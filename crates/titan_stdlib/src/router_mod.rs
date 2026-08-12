//! URL router (`std::router::*`) powered by `matchit` 0.8 — the same
//! high-performance radix-tree router that lives at the heart of axum.
//!
//! Titan code creates one or more routers, inserts patterns with
//! optional named / catch-all parameters, and then matches an incoming
//! path to recover the pattern name plus a `{name: value, …}` map of
//! extracted parameters.
//!
//! ```titan
//! let r = std::router::new()
//! std::router::insert(r, "/users/{id}",         "show_user")
//! std::router::insert(r, "/posts/{id}/edit",    "edit_post")
//! std::router::insert(r, "/static/{*rest}",     "static_files")
//!
//! let m = std::router::at(r, "/users/42")
//! # -> {"pattern": "show_user", "params": {"id": "42"}}
//! ```
//!
//! Handles are opaque `i64` values so multiple routers can coexist per
//! process (one for HTTP routes, one for WS routes, one for admin, …).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

use matchit::Router;
use thiserror::Error;

const MAX_ROUTER_HANDLES: usize = 64;
const MAX_ROUTES_PER_ROUTER: usize = 4_096;
const MAX_ROUTER_BYTES: usize = 1024 * 1024;
const MAX_RUNTIME_ROUTER_BYTES: usize = 8 * 1024 * 1024;
const MAX_ROUTE_PATTERN_BYTES: usize = 8 * 1024;
const MAX_ROUTE_VALUE_BYTES: usize = 64 * 1024;
const MAX_ROUTE_PATH_BYTES: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("router error: {0}")]
    Insert(String),
    #[error("unknown router handle {0}")]
    UnknownHandle(i64),
    #[error("{resource} exceeds limit {limit}")]
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    #[error("router handle space exhausted")]
    HandleSpaceExhausted,
}

struct RouterEntry {
    router: Router<String>,
    routes: usize,
    bytes: usize,
}

struct Registry {
    routers: HashMap<(u64, i64), RouterEntry>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| {
        Mutex::new(Registry {
            routers: HashMap::new(),
            next_id: 1,
        })
    })
}

fn handle_key(handle: i64) -> (u64, i64) {
    crate::native::runtime_handle_key(handle)
}

/// Create a fresh empty router. Returns an opaque handle.
pub fn new() -> Result<i64, RouterError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(registry());
    let active = registry
        .routers
        .keys()
        .filter(|(owner, _)| *owner == runtime_id)
        .count();
    if active >= MAX_ROUTER_HANDLES {
        return Err(RouterError::ResourceLimit {
            resource: "router handles",
            limit: MAX_ROUTER_HANDLES,
        });
    }
    let id = registry.next_id;
    registry.next_id = id
        .checked_add(1)
        .ok_or(RouterError::HandleSpaceExhausted)?;
    registry.routers.insert(
        (runtime_id, id),
        RouterEntry {
            router: Router::new(),
            routes: 0,
            bytes: 0,
        },
    );
    Ok(id)
}

/// Drop a router. Idempotent.
pub fn drop_router(handle: i64) {
    crate::native::lock_recover(registry())
        .routers
        .remove(&handle_key(handle));
}

/// Register a route. `pattern` follows matchit syntax:
///
/// * `/users` — static segment
/// * `/users/{id}` — named parameter
/// * `/files/{*rest}` — catch-all (must be the last segment)
///
/// `value` is an arbitrary string tag returned by `at()` when the
/// pattern matches — typically a handler name.
pub fn insert(handle: i64, pattern: &str, value: &str) -> Result<(), RouterError> {
    if pattern.len() > MAX_ROUTE_PATTERN_BYTES {
        return Err(RouterError::ResourceLimit {
            resource: "route pattern bytes",
            limit: MAX_ROUTE_PATTERN_BYTES,
        });
    }
    if value.len() > MAX_ROUTE_VALUE_BYTES {
        return Err(RouterError::ResourceLimit {
            resource: "route value bytes",
            limit: MAX_ROUTE_VALUE_BYTES,
        });
    }
    let added_bytes = pattern
        .len()
        .checked_add(value.len())
        .ok_or(RouterError::ResourceLimit {
            resource: "route bytes",
            limit: MAX_ROUTER_BYTES,
        })?;
    let key = handle_key(handle);
    let mut registry = crate::native::lock_recover(registry());
    let entry = registry
        .routers
        .get(&key)
        .ok_or(RouterError::UnknownHandle(handle))?;
    if entry.routes >= MAX_ROUTES_PER_ROUTER {
        return Err(RouterError::ResourceLimit {
            resource: "routes per router",
            limit: MAX_ROUTES_PER_ROUTER,
        });
    }
    if entry.bytes.saturating_add(added_bytes) > MAX_ROUTER_BYTES {
        return Err(RouterError::ResourceLimit {
            resource: "router bytes",
            limit: MAX_ROUTER_BYTES,
        });
    }
    let runtime_bytes = registry
        .routers
        .iter()
        .filter(|((owner, _), _)| *owner == key.0)
        .try_fold(0usize, |total, (_, entry)| total.checked_add(entry.bytes))
        .unwrap_or(usize::MAX);
    if runtime_bytes.saturating_add(added_bytes) > MAX_RUNTIME_ROUTER_BYTES {
        return Err(RouterError::ResourceLimit {
            resource: "runtime router bytes",
            limit: MAX_RUNTIME_ROUTER_BYTES,
        });
    }
    let entry = registry
        .routers
        .get_mut(&key)
        .ok_or(RouterError::UnknownHandle(handle))?;
    entry
        .router
        .insert(pattern.to_string(), value.to_string())
        .map_err(|error| RouterError::Insert(error.to_string()))?;
    entry.routes += 1;
    entry.bytes += added_bytes;
    Ok(())
}

/// Look up `path` in the router. Returns `Some((tag, params))` if a
/// route matched, `None` otherwise. `params` is an ordered map of
/// extracted names to values (percent-decoded by matchit).
pub fn at(
    handle: i64,
    path: &str,
) -> Result<Option<(String, BTreeMap<String, String>)>, RouterError> {
    if path.len() > MAX_ROUTE_PATH_BYTES {
        return Err(RouterError::ResourceLimit {
            resource: "route lookup path bytes",
            limit: MAX_ROUTE_PATH_BYTES,
        });
    }
    let registry = crate::native::lock_recover(registry());
    let router = registry
        .routers
        .get(&handle_key(handle))
        .ok_or(RouterError::UnknownHandle(handle))?;
    match router.router.at(path) {
        Ok(m) => {
            let mut params = BTreeMap::new();
            for (k, v) in m.params.iter() {
                params.insert(k.to_string(), v.to_string());
            }
            Ok(Some((m.value.to_string(), params)))
        }
        Err(_) => Ok(None),
    }
}

/// Convenience: match `path` and return `true` iff any route matches
/// (no parameters returned). Handy for feature flags.
pub fn matches(handle: i64, path: &str) -> Result<bool, RouterError> {
    if path.len() > MAX_ROUTE_PATH_BYTES {
        return Err(RouterError::ResourceLimit {
            resource: "route lookup path bytes",
            limit: MAX_ROUTE_PATH_BYTES,
        });
    }
    let registry = crate::native::lock_recover(registry());
    let router = registry
        .routers
        .get(&handle_key(handle))
        .ok_or(RouterError::UnknownHandle(handle))?;
    Ok(router.router.at(path).is_ok())
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let mut reg = crate::native::lock_recover(registry());
    crate::native::remove_runtime_entries(&mut reg.routers, runtime_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_match_static_and_named() {
        let r = new().unwrap();
        insert(r, "/", "root").unwrap();
        insert(r, "/users/{id}", "show_user").unwrap();
        insert(r, "/files/{*rest}", "files").unwrap();

        let (tag, p) = at(r, "/").unwrap().expect("root matches");
        assert_eq!(tag, "root");
        assert!(p.is_empty());

        let (tag, p) = at(r, "/users/42").unwrap().expect("user matches");
        assert_eq!(tag, "show_user");
        assert_eq!(p.get("id").map(String::as_str), Some("42"));

        let (tag, p) = at(r, "/files/a/b/c.txt")
            .unwrap()
            .expect("catch-all matches");
        assert_eq!(tag, "files");
        assert_eq!(p.get("rest").map(String::as_str), Some("a/b/c.txt"));

        assert!(at(r, "/nope").unwrap().is_none());
        drop_router(r);
    }

    #[test]
    fn duplicate_pattern_reports_typed_error() {
        let r = new().unwrap();
        insert(r, "/x", "one").unwrap();
        assert!(insert(r, "/x", "two").is_err());
        drop_router(r);
    }

    #[test]
    fn handle_route_and_byte_limits_reject_growth_and_recover() {
        let runtime_id = 8_300_002;
        crate::native::with_runtime_context(runtime_id, || {
            let mut handles = (0..MAX_ROUTER_HANDLES)
                .map(|_| new().unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                new(),
                Err(RouterError::ResourceLimit {
                    resource: "router handles",
                    ..
                })
            ));
            drop_router(handles.pop().unwrap());
            let router = new().unwrap();
            assert!(matches!(
                insert(router, "/too-large", &"x".repeat(MAX_ROUTE_VALUE_BYTES + 1)),
                Err(RouterError::ResourceLimit { .. })
            ));
            for index in 0..MAX_ROUTES_PER_ROUTER {
                insert(router, &format!("/route/{index}"), "handler").unwrap();
            }
            assert!(matches!(
                insert(router, "/overflow", "handler"),
                Err(RouterError::ResourceLimit {
                    resource: "routes per router",
                    ..
                })
            ));
            assert!(matches!(
                at(router, &"x".repeat(MAX_ROUTE_PATH_BYTES + 1)),
                Err(RouterError::ResourceLimit { .. })
            ));
        });
        assert_eq!(cleanup_runtime(runtime_id), MAX_ROUTER_HANDLES);
    }

    #[test]
    fn per_router_and_runtime_byte_quotas_are_enforced() {
        let runtime_id = 8_300_006;
        crate::native::with_runtime_context(runtime_id, || {
            let routers = (0..9).map(|_| new().unwrap()).collect::<Vec<_>>();
            let value = "x".repeat(MAX_ROUTE_VALUE_BYTES);

            for (router_index, router) in routers.iter().take(8).enumerate() {
                for route_index in 0..15 {
                    insert(
                        *router,
                        &format!("/bytes/{router_index}/{route_index}"),
                        &value,
                    )
                    .unwrap();
                }
            }
            assert!(matches!(
                insert(routers[0], "/bytes/per-router-overflow", &value),
                Err(RouterError::ResourceLimit {
                    resource: "router bytes",
                    ..
                })
            ));

            let mut runtime_limit_seen = false;
            for route_index in 0..16 {
                match insert(
                    routers[8],
                    &format!("/runtime-bytes/{route_index}"),
                    &value,
                ) {
                    Ok(()) => {}
                    Err(RouterError::ResourceLimit {
                        resource: "runtime router bytes",
                        ..
                    }) => {
                        runtime_limit_seen = true;
                        break;
                    }
                    Err(error) => panic!("unexpected router error: {error}"),
                }
            }
            assert!(runtime_limit_seen);
        });
        assert_eq!(cleanup_runtime(runtime_id), 9);
    }

    #[test]
    fn unknown_handle_reports_typed_error() {
        assert!(matches!(
            at(999_999, "/"),
            Err(RouterError::UnknownHandle(_))
        ));
    }
}

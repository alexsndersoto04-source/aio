//! Blocking Redis client (`std::redis::*`) via the `redis` crate.
//!
//! Real Redis wire protocol over plain TCP. Connections cross the .titan
//! boundary as opaque `i64` handles kept in a process-wide registry that is
//! partitioned by VM runtime ownership.
//!
//! We use the synchronous side of the `redis` crate to avoid pulling in
//! Tokio for network-bound work (Phase 3's `std::http_full`/`std::dns`
//! already offer async). Perfect for scripts, workers and CLIs.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use redis::{Client, Commands, Connection};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RedisError {
    #[error("redis error: {0}")]
    Backend(#[from] redis::RedisError),
    #[error("unknown connection handle {0}")]
    UnknownHandle(i64),
}

struct Registry { conns: HashMap<(u64, i64), Connection>, next_id: i64 }

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry { conns: HashMap::new(), next_id: 1 }))
}

fn handle_key(handle: i64) -> (u64, i64) { crate::native::runtime_handle_key(handle) }

fn with_conn<F, R>(handle: i64, action: F) -> Result<R, RedisError>
where F: FnOnce(&mut Connection) -> Result<R, RedisError> {
    let mut reg = registry().lock().expect("redis registry poisoned");
    let conn = reg.conns.get_mut(&handle_key(handle)).ok_or(RedisError::UnknownHandle(handle))?;
    action(conn)
}

/// Open a blocking connection to `url` (e.g. `redis://localhost/`,
/// `redis://user:pass@host:6379/0`). Returns an opaque handle.
pub fn connect(url: &str) -> Result<i64, RedisError> {
    let client = Client::open(url)?;
    let conn = client.get_connection()?;
    let mut reg = registry().lock().expect("redis registry poisoned");
    let id = reg.next_id;
    reg.next_id += 1;
    reg.conns.insert(handle_key(id), conn);
    Ok(id)
}

/// Close a connection. Idempotent.
pub fn close(handle: i64) {
    if let Ok(mut reg) = registry().lock() { reg.conns.remove(&handle_key(handle)); }
}

/// PING → OK ("PONG" or the string returned by the server).
pub fn ping(handle: i64) -> Result<String, RedisError> {
    with_conn(handle, |conn| Ok(redis::cmd("PING").query::<String>(conn)?))
}

// ---------------- Strings ---------------------------------------------

pub fn set(handle: i64, key: &str, value: &str) -> Result<(), RedisError> {
    with_conn(handle, |conn| { let _: () = conn.set(key, value)?; Ok(()) })
}

pub fn set_ex(handle: i64, key: &str, value: &str, seconds: u64) -> Result<(), RedisError> {
    with_conn(handle, |conn| { let _: () = conn.set_ex(key, value, seconds)?; Ok(()) })
}

pub fn get(handle: i64, key: &str) -> Result<Option<String>, RedisError> {
    with_conn(handle, |conn| Ok(conn.get::<_, Option<String>>(key)?))
}

pub fn del(handle: i64, key: &str) -> Result<u64, RedisError> {
    with_conn(handle, |conn| Ok(conn.del::<_, u64>(key)?))
}

pub fn exists(handle: i64, key: &str) -> Result<bool, RedisError> {
    with_conn(handle, |conn| Ok(conn.exists::<_, bool>(key)?))
}

pub fn expire(handle: i64, key: &str, seconds: i64) -> Result<bool, RedisError> {
    with_conn(handle, |conn| Ok(conn.expire::<_, bool>(key, seconds)?))
}

pub fn ttl(handle: i64, key: &str) -> Result<i64, RedisError> {
    with_conn(handle, |conn| Ok(conn.ttl::<_, i64>(key)?))
}

pub fn incr(handle: i64, key: &str, delta: i64) -> Result<i64, RedisError> {
    with_conn(handle, |conn| Ok(conn.incr::<_, _, i64>(key, delta)?))
}

pub fn keys(handle: i64, pattern: &str) -> Result<Vec<String>, RedisError> {
    with_conn(handle, |conn| Ok(conn.keys::<_, Vec<String>>(pattern)?))
}

// ---------------- Lists (LPUSH/RPUSH/LRANGE) --------------------------

pub fn lpush(handle: i64, key: &str, value: &str) -> Result<u64, RedisError> {
    with_conn(handle, |conn| Ok(conn.lpush::<_, _, u64>(key, value)?))
}
pub fn rpush(handle: i64, key: &str, value: &str) -> Result<u64, RedisError> {
    with_conn(handle, |conn| Ok(conn.rpush::<_, _, u64>(key, value)?))
}
pub fn lrange(handle: i64, key: &str, start: i64, stop: i64) -> Result<Vec<String>, RedisError> {
    with_conn(handle, |conn| Ok(conn.lrange::<_, Vec<String>>(key, start as isize, stop as isize)?))
}
pub fn llen(handle: i64, key: &str) -> Result<u64, RedisError> {
    with_conn(handle, |conn| Ok(conn.llen::<_, u64>(key)?))
}

// ---------------- Hashes ----------------------------------------------

pub fn hset(handle: i64, key: &str, field: &str, value: &str) -> Result<(), RedisError> {
    with_conn(handle, |conn| { let _: () = conn.hset(key, field, value)?; Ok(()) })
}
pub fn hget(handle: i64, key: &str, field: &str) -> Result<Option<String>, RedisError> {
    with_conn(handle, |conn| Ok(conn.hget::<_, _, Option<String>>(key, field)?))
}
pub fn hdel(handle: i64, key: &str, field: &str) -> Result<u64, RedisError> {
    with_conn(handle, |conn| Ok(conn.hdel::<_, _, u64>(key, field)?))
}
pub fn hgetall(handle: i64, key: &str) -> Result<Vec<(String, String)>, RedisError> {
    with_conn(handle, |conn| {
        let map: std::collections::BTreeMap<String, String> = conn.hgetall(key)?;
        Ok(map.into_iter().collect())
    })
}

/// Raw command: `command_and_args` is split by whitespace. Use for
/// commands not covered by the wrappers above.
pub fn raw(handle: i64, command_and_args: &str) -> Result<String, RedisError> {
    with_conn(handle, |conn| {
        let parts: Vec<&str> = command_and_args.split_whitespace().collect();
        if parts.is_empty() { return Ok(String::new()); }
        let mut cmd = redis::cmd(parts[0]);
        for arg in &parts[1..] { cmd.arg(*arg); }
        // Best-effort: bring back the reply as a string.
        let value: redis::Value = cmd.query(conn)?;
        Ok(format!("{value:?}"))
    })
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let mut reg = crate::native::lock_recover(registry());
    crate::native::remove_runtime_entries(&mut reg.conns, runtime_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_handle_reports_typed_error() {
        assert!(matches!(ping(999_999), Err(RedisError::UnknownHandle(_))));
        assert!(matches!(get(999_999, "x"), Err(RedisError::UnknownHandle(_))));
    }

    #[test]
    fn connect_errors_on_bad_url() {
        // We don't have a real server here; just check it fails cleanly.
        assert!(connect("redis://127.0.0.1:1/").is_err());
    }

    /// Live tests are opt-in: set TITAN_REDIS_TEST_URL=redis://localhost/.
    #[test]
    fn live_round_trip_when_configured() {
        let Ok(url) = std::env::var("TITAN_REDIS_TEST_URL") else { return; };
        let handle = connect(&url).expect("connect");
        assert_eq!(ping(handle).unwrap(), "PONG");
        set(handle, "titan:test", "hola").unwrap();
        assert_eq!(get(handle, "titan:test").unwrap(), Some("hola".into()));
        del(handle, "titan:test").unwrap();
        close(handle);
    }
}

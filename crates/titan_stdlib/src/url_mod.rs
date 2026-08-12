//! URL parsing and construction (`std::url::*`) backed by the `url` crate.
//!
//! All operations are memory-safe and reject invalid input with `UrlError`
//! rather than panicking. Query-string builders/parsers preserve RFC 3986
//! percent-encoding.

use std::collections::BTreeMap;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum UrlError {
    #[error("invalid URL '{url}': {source}")]
    Parse {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("URL has no host: '{0}'")]
    NoHost(String),
}

fn parse(text: &str) -> Result<Url, UrlError> {
    Url::parse(text).map_err(|source| UrlError::Parse {
        url: text.into(),
        source,
    })
}

/// Returns `true` when `text` is an absolute URL.
pub fn is_valid(text: &str) -> bool {
    Url::parse(text).is_ok()
}

/// Scheme (e.g. `"https"`).
pub fn scheme(text: &str) -> Result<String, UrlError> {
    Ok(parse(text)?.scheme().to_string())
}

/// Host name; returns [`UrlError::NoHost`] when the URL has no authority.
pub fn host(text: &str) -> Result<String, UrlError> {
    let url = parse(text)?;
    url.host_str()
        .map(|s| s.to_string())
        .ok_or_else(|| UrlError::NoHost(text.into()))
}

/// Port explicitly present in the URL, or the default for the scheme (443 for https, 80 for http, ...).
pub fn port(text: &str) -> Result<Option<u16>, UrlError> {
    Ok(parse(text)?.port_or_known_default())
}

/// Path component (`/foo/bar`).
pub fn path(text: &str) -> Result<String, UrlError> {
    Ok(parse(text)?.path().to_string())
}

/// Query string without the leading `?`, or empty string.
pub fn query(text: &str) -> Result<String, UrlError> {
    Ok(parse(text)?.query().unwrap_or("").to_string())
}

/// Fragment (`#foo`) without the `#`, or empty string.
pub fn fragment(text: &str) -> Result<String, UrlError> {
    Ok(parse(text)?.fragment().unwrap_or("").to_string())
}

/// Parses `?a=1&b=2` (with or without the `?`) into a map. When the same key
/// appears more than once, the last value wins — call `query_pairs` if you
/// need every value.
pub fn parse_query(text: &str) -> BTreeMap<String, String> {
    let trimmed = text.strip_prefix('?').unwrap_or(text);
    let mut out = BTreeMap::new();
    for (key, value) in url::form_urlencoded::parse(trimmed.as_bytes()) {
        out.insert(key.into_owned(), value.into_owned());
    }
    out
}

/// Parses `?a=1&a=2` preserving every occurrence.
pub fn query_pairs(text: &str) -> Vec<(String, String)> {
    let trimmed = text.strip_prefix('?').unwrap_or(text);
    url::form_urlencoded::parse(trimmed.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

/// Builds an `application/x-www-form-urlencoded` query string from key/value pairs.
pub fn build_query(pairs: &[(String, String)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .finish()
}

/// Resolves `relative` against `base`, e.g. `join("https://a.com/x/", "../y")`.
pub fn join(base: &str, relative: &str) -> Result<String, UrlError> {
    Ok(parse(base)?
        .join(relative)
        .map_err(|source| UrlError::Parse {
            url: relative.into(),
            source,
        })?
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_components() {
        let text = "https://user:pw@example.com:8443/api/v1?foo=bar&baz=1#frag";
        assert_eq!(scheme(text).unwrap(), "https");
        assert_eq!(host(text).unwrap(), "example.com");
        assert_eq!(port(text).unwrap(), Some(8443));
        assert_eq!(path(text).unwrap(), "/api/v1");
        assert_eq!(query(text).unwrap(), "foo=bar&baz=1");
        assert_eq!(fragment(text).unwrap(), "frag");
    }

    #[test]
    fn default_ports() {
        assert_eq!(port("https://example.com/").unwrap(), Some(443));
        assert_eq!(port("http://example.com/").unwrap(), Some(80));
        assert_eq!(port("ftp://example.com/").unwrap(), Some(21));
    }

    #[test]
    fn query_helpers() {
        let map = parse_query("?a=1&b=2&c=hola%20mundo");
        assert_eq!(map.get("a").map(String::as_str), Some("1"));
        assert_eq!(map.get("c").map(String::as_str), Some("hola mundo"));

        let pairs = query_pairs("x=1&x=2&y=3");
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], ("x".into(), "1".into()));
        assert_eq!(pairs[1], ("x".into(), "2".into()));

        let built = build_query(&[("q".into(), "hola mundo".into()), ("n".into(), "42".into())]);
        assert_eq!(built, "q=hola+mundo&n=42");
    }

    #[test]
    fn join_relative_urls() {
        assert_eq!(
            join("https://a.com/x/y", "../z").unwrap(),
            "https://a.com/z"
        );
        assert_eq!(
            join("https://a.com/", "https://b.com/other").unwrap(),
            "https://b.com/other"
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(!is_valid("not a url"));
        assert!(scheme("not a url").is_err());
    }
}

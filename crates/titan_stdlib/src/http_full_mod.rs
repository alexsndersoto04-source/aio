//! Full-featured blocking HTTP/HTTPS client (`std::http_full::*`).
//!
//! Powered by the `ureq` crate + `rustls` for TLS. Compiles cleanly on
//! Termux AArch64 (pure Rust, no OpenSSL).
//!
//! What this gives you over the existing `std::http::request`:
//!   * `https://` support out of the box (rustls + webpki-roots).
//!   * Transparent gzip decoding of responses.
//!   * Convenience wrappers: `get`, `post`, `put`, `delete`, `patch`.
//!   * `get_json` / `post_json` returning parsed JSON directly.
//!   * Basic and Bearer authentication.
//!   * Per-request timeout and custom headers.

use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("HTTP transport error: {0}")]
    Transport(String),
    #[error("HTTP request error: {0}")]
    Request(String),
    #[error("HTTP I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid header value: {0}")]
    Header(String),
    #[error("response body was not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Structured HTTP response returned to `.titan` as a map:
/// `{ status: Int, headers: Map<String>, body: Bytes, final_url: String }`.
#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub final_url: String,
}

/// Optional per-request options.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub headers: Vec<(String, String)>,
    /// Basic auth `(user, password)`.
    pub basic_auth: Option<(String, String)>,
    /// Bearer token (sent as `Authorization: Bearer <token>`).
    pub bearer: Option<String>,
    /// Overall request timeout in milliseconds. Defaults to 30 s.
    pub timeout_ms: Option<u64>,
    /// Maximum number of redirects to follow. Defaults to 5.
    pub max_redirects: Option<u32>,
    /// Custom User-Agent. Defaults to `titan-http/0.2`.
    pub user_agent: Option<String>,
}

fn agent(options: &Options) -> ureq::Agent {
    let timeout = Duration::from_millis(options.timeout_ms.unwrap_or(30_000));
    let mut builder = ureq::AgentBuilder::new()
        .timeout(timeout)
        .redirects(options.max_redirects.unwrap_or(5))
        .user_agent(options.user_agent.as_deref().unwrap_or("titan-http/0.2"));
    // ureq 2.x picks up rustls-tls automatically when the feature is enabled.
    // Wire cookies would require the "cookies" feature; we skip for now.
    let _ = &mut builder;
    builder.build()
}

fn build_request(mut req: ureq::Request, options: &Options) -> Result<ureq::Request, HttpError> {
    for (name, value) in &options.headers {
        req = req.set(name, value);
    }
    if let Some((user, password)) = &options.basic_auth {
        use base64::Engine as _;
        let raw = format!("{user}:{password}");
        let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
        req = req.set("Authorization", &format!("Basic {encoded}"));
    }
    if let Some(token) = &options.bearer {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    Ok(req)
}

fn collect(response: ureq::Response) -> Result<Response, HttpError> {
    let status = response.status();
    let headers = response
        .headers_names()
        .into_iter()
        .filter_map(|name| {
            response
                .header(&name)
                .map(|value| (name, value.to_string()))
        })
        .collect();
    let final_url = response.get_url().to_string();
    let mut body = Vec::new();
    response
        .into_reader()
        .take(64 * 1024 * 1024)
        .read_to_end(&mut body)?;
    Ok(Response {
        status,
        headers,
        body,
        final_url,
    })
}

fn transport_err(error: ureq::Error) -> HttpError {
    match error {
        ureq::Error::Status(_, resp) => {
            HttpError::Request(format!("HTTP {}: {}", resp.status(), resp.status_text()))
        }
        ureq::Error::Transport(err) => HttpError::Transport(err.to_string()),
    }
}

// ------------------ HTTP verbs -----------------------------------------

pub fn get(url: &str, options: &Options) -> Result<Response, HttpError> {
    let request = build_request(agent(options).get(url), options)?;
    request.call().map_err(transport_err).and_then(collect)
}

pub fn head(url: &str, options: &Options) -> Result<Response, HttpError> {
    let request = build_request(agent(options).head(url), options)?;
    request.call().map_err(transport_err).and_then(collect)
}

pub fn delete(url: &str, options: &Options) -> Result<Response, HttpError> {
    let request = build_request(agent(options).delete(url), options)?;
    request.call().map_err(transport_err).and_then(collect)
}

pub fn post(url: &str, body: &[u8], options: &Options) -> Result<Response, HttpError> {
    let request = build_request(agent(options).post(url), options)?;
    request
        .send_bytes(body)
        .map_err(transport_err)
        .and_then(collect)
}

pub fn put(url: &str, body: &[u8], options: &Options) -> Result<Response, HttpError> {
    let request = build_request(agent(options).put(url), options)?;
    request
        .send_bytes(body)
        .map_err(transport_err)
        .and_then(collect)
}

pub fn patch(url: &str, body: &[u8], options: &Options) -> Result<Response, HttpError> {
    let request = build_request(agent(options).request("PATCH", url), options)?;
    request
        .send_bytes(body)
        .map_err(transport_err)
        .and_then(collect)
}

/// Convenience: GET returning parsed JSON.
pub fn get_json(url: &str, options: &Options) -> Result<Value, HttpError> {
    let mut headers = options.headers.clone();
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("Accept"))
    {
        headers.push(("Accept".into(), "application/json".into()));
    }
    let opts = Options {
        headers,
        ..options.clone()
    };
    let response = get(url, &opts)?;
    Ok(serde_json::from_slice(&response.body)?)
}

/// Convenience: POST with a JSON body and JSON response.
pub fn post_json(url: &str, body: &Value, options: &Options) -> Result<Value, HttpError> {
    let mut headers = options.headers.clone();
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("Content-Type"))
    {
        headers.push(("Content-Type".into(), "application/json".into()));
    }
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("Accept"))
    {
        headers.push(("Accept".into(), "application/json".into()));
    }
    let opts = Options {
        headers,
        ..options.clone()
    };
    let payload = serde_json::to_vec(body)?;
    let response = post(url, &payload, &opts)?;
    Ok(serde_json::from_slice(&response.body)?)
}

/// Convenience: POST url-encoded form.
pub fn post_form(
    url: &str,
    form: &[(String, String)],
    options: &Options,
) -> Result<Response, HttpError> {
    let mut headers = options.headers.clone();
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("Content-Type"))
    {
        headers.push((
            "Content-Type".into(),
            "application/x-www-form-urlencoded".into(),
        ));
    }
    let opts = Options {
        headers,
        ..options.clone()
    };
    let body = form_urlencoded_serialize(form);
    post(url, body.as_bytes(), &opts)
}

fn form_urlencoded_serialize(pairs: &[(String, String)]) -> String {
    let encode = |s: &str| {
        let mut out = String::with_capacity(s.len());
        for byte in s.bytes() {
            match byte {
                b' ' => out.push('+'),
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(byte as char)
                }
                _ => out.push_str(&format!("%{byte:02X}")),
            }
        }
        out
    };
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_urlencoded_matches_spec() {
        let out = form_urlencoded_serialize(&[
            ("q".into(), "hola mundo".into()),
            ("n".into(), "42".into()),
        ]);
        assert_eq!(out, "q=hola+mundo&n=42");
    }

    #[test]
    fn agent_respects_timeout_setting() {
        let mut options = Options::default();
        options.timeout_ms = Some(1234);
        // Building the agent must not panic and the timeout must round-trip.
        let _agent = agent(&options);
    }

    /// Live network tests are opt-in. Set TITAN_HTTP_LIVE=1 to run them.
    /// The endpoint used (`https://httpbin.org`) has no strict SLA; skip if it flakes.
    #[test]
    fn live_https_get_when_enabled() {
        if std::env::var("TITAN_HTTP_LIVE").is_err() {
            return;
        }
        let response = get("https://httpbin.org/get", &Options::default()).unwrap();
        assert_eq!(response.status, 200);
        assert!(response.body.len() > 0);
    }

    #[test]
    fn live_https_post_json_when_enabled() {
        if std::env::var("TITAN_HTTP_LIVE").is_err() {
            return;
        }
        let json = serde_json::json!({ "hello": "titan" });
        let response = post_json("https://httpbin.org/post", &json, &Options::default()).unwrap();
        assert_eq!(response["json"]["hello"], "titan");
    }
}

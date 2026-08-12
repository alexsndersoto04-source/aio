//! HTTP/1.1 server (`std::server::*`) powered by `tiny_http` 0.12.
//!
//! `tiny_http` is 100% pure Rust, blocking, and works everywhere Rust
//! builds — no OpenSSL, no async runtime, no C shims. It handles the
//! wire (accept + parse + response encoding), and Titan code sits on
//! top with a synchronous event-loop model:
//!
//! ```titan
//! let s = std::server::start("0.0.0.0:8080")
//! while true {
//!     let req = std::server::accept(s, 5000)   // -1 on timeout
//!     if req >= 0 {
//!         let path = std::server::path(req)
//!         std::server::respond(req, 200, "Hola desde Titan " + path)
//!     }
//! }
//! ```
//!
//! Requests, servers and upgraded WebSocket streams all cross the
//! `.titan` boundary as opaque `i64` handles kept in a process-wide
//! registry so several servers/connections can coexist.

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use thiserror::Error;
use tiny_http::{Header, HeaderField, ListenAddr, Request, Response, Server};

use crate::websocket::{self as ws_codec, Message, MessageDecoder};

// WebSocket opcode constants (RFC 6455 §5.2)
const WS_OP_TEXT: u8 = 0x1;
const WS_OP_BINARY: u8 = 0x2;
const WS_OP_CLOSE: u8 = 0x8;
const WS_OP_PONG: u8 = 0xA;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("http server error: {0}")]
    Http(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unknown server handle {0}")]
    UnknownServer(i64),
    #[error("unknown request handle {0}")]
    UnknownRequest(i64),
    #[error("unknown websocket handle {0}")]
    UnknownWebSocket(i64),
    #[error("request body is not valid UTF-8")]
    Utf8,
    #[error("invalid header value")]
    InvalidHeader,
    #[error("request is missing the '{0}' header")]
    MissingHeader(&'static str),
    #[error("websocket upgrade failed: {0}")]
    Upgrade(&'static str),
    #[error("websocket protocol error: {0}")]
    WebSocket(#[from] ws_codec::WebSocketError),
}

// ---- Server / request registry --------------------------------------

struct Registry {
    servers: HashMap<(u64, i64), Server>,
    requests: HashMap<(u64, i64), Request>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| {
        Mutex::new(Registry {
            servers: HashMap::new(),
            requests: HashMap::new(),
            next_id: 1,
        })
    })
}

fn handle_key(handle: i64) -> (u64, i64) {
    crate::native::runtime_handle_key(handle)
}

fn insert_server(server: Server) -> i64 {
    let mut r = registry().lock().expect("server registry poisoned");
    let id = r.next_id;
    r.next_id += 1;
    r.servers.insert(handle_key(id), server);
    id
}
fn insert_request(req: Request) -> i64 {
    let mut r = registry().lock().expect("server registry poisoned");
    let id = r.next_id;
    r.next_id += 1;
    r.requests.insert(handle_key(id), req);
    id
}

// ---- Server lifecycle -----------------------------------------------

/// Bind and start serving on `addr` (e.g. `"0.0.0.0:8080"` or
/// `"127.0.0.1:0"` for an ephemeral port).
pub fn start(addr: &str) -> Result<i64, ServerError> {
    let server = Server::http(addr).map_err(|e| ServerError::Http(e.to_string()))?;
    Ok(insert_server(server))
}

/// Return the local socket address the server is bound to, e.g.
/// `"127.0.0.1:8080"`. Useful with `start("127.0.0.1:0")`.
pub fn local_addr(handle: i64) -> Result<String, ServerError> {
    let r = registry().lock().expect("server registry poisoned");
    let s = r
        .servers
        .get(&handle_key(handle))
        .ok_or(ServerError::UnknownServer(handle))?;
    Ok(match s.server_addr() {
        ListenAddr::IP(addr) => addr.to_string(),
        // Los sockets Unix no existen en Windows: tiny_http solo define
        // esta variante del enum en plataformas Unix.
        #[cfg(unix)]
        ListenAddr::Unix(addr) => format!("unix:{addr:?}"),
    })
}

/// Blocking accept with a timeout in milliseconds. Returns the request
/// handle, or `-1` when the timeout elapses without a request.
pub fn accept(handle: i64, timeout_ms: u64) -> Result<i64, ServerError> {
    let received = {
        let r = registry().lock().expect("server registry poisoned");
        let s = r
            .servers
            .get(&handle_key(handle))
            .ok_or(ServerError::UnknownServer(handle))?;
        s.recv_timeout(Duration::from_millis(timeout_ms))
            .map_err(|e| ServerError::Http(e.to_string()))?
    };
    match received {
        Some(req) => Ok(insert_request(req)),
        None => Ok(-1),
    }
}

/// Stop a server: drops the listener and any queued connections.
pub fn stop(handle: i64) {
    if let Ok(mut r) = registry().lock() {
        r.servers.remove(&handle_key(handle));
    }
}

// ---- Request accessors ----------------------------------------------

fn with_request<F, R>(handle: i64, action: F) -> Result<R, ServerError>
where
    F: FnOnce(&Request) -> R,
{
    let r = registry().lock().expect("server registry poisoned");
    let req = r
        .requests
        .get(&handle_key(handle))
        .ok_or(ServerError::UnknownRequest(handle))?;
    Ok(action(req))
}

pub fn method(handle: i64) -> Result<String, ServerError> {
    with_request(handle, |r| r.method().as_str().to_string())
}
pub fn url(handle: i64) -> Result<String, ServerError> {
    with_request(handle, |r| r.url().to_string())
}
pub fn path(handle: i64) -> Result<String, ServerError> {
    with_request(handle, |r| {
        r.url().split('?').next().unwrap_or("/").to_string()
    })
}
pub fn query(handle: i64) -> Result<String, ServerError> {
    with_request(handle, |r| {
        let u = r.url();
        u.find('?')
            .map(|i| u[i + 1..].to_string())
            .unwrap_or_default()
    })
}
pub fn remote_addr(handle: i64) -> Result<String, ServerError> {
    with_request(handle, |r| {
        r.remote_addr().map(|a| a.to_string()).unwrap_or_default()
    })
}

pub fn header(handle: i64, name: &str) -> Result<Option<String>, ServerError> {
    with_request(handle, |r| {
        let target = HeaderField::from_bytes(name.as_bytes()).ok()?;
        r.headers()
            .iter()
            .find(|h| h.field == target)
            .map(|h| h.value.as_str().to_string())
    })
}

pub fn headers(handle: i64) -> Result<BTreeMap<String, String>, ServerError> {
    with_request(handle, |r| {
        r.headers()
            .iter()
            .map(|h| {
                (
                    h.field.as_str().as_str().to_string(),
                    h.value.as_str().to_string(),
                )
            })
            .collect()
    })
}

/// Read the entire request body into memory. Consumes the reader, so
/// call this at most once per request.
pub fn body(handle: i64) -> Result<Vec<u8>, ServerError> {
    let mut r = registry().lock().expect("server registry poisoned");
    let req = r
        .requests
        .get_mut(&handle_key(handle))
        .ok_or(ServerError::UnknownRequest(handle))?;
    let mut buf = Vec::new();
    req.as_reader().read_to_end(&mut buf)?;
    Ok(buf)
}
pub fn body_text(handle: i64) -> Result<String, ServerError> {
    let bytes = body(handle)?;
    String::from_utf8(bytes).map_err(|_| ServerError::Utf8)
}

// ---- Response helpers ----------------------------------------------

fn take_request(handle: i64) -> Result<Request, ServerError> {
    let mut r = registry().lock().expect("server registry poisoned");
    r.requests
        .remove(&handle_key(handle))
        .ok_or(ServerError::UnknownRequest(handle))
}

fn make_header(name: &str, value: &str) -> Option<Header> {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).ok()
}

/// Send a plain-text response and consume the request.
pub fn respond(handle: i64, status: u16, body: &str) -> Result<(), ServerError> {
    let req = take_request(handle)?;
    let mut resp = Response::from_string(body).with_status_code(status);
    if let Some(h) = make_header("Content-Type", "text/plain; charset=utf-8") {
        resp.add_header(h);
    }
    req.respond(resp)?;
    Ok(())
}

pub fn respond_html(handle: i64, status: u16, html: &str) -> Result<(), ServerError> {
    let req = take_request(handle)?;
    let mut resp = Response::from_string(html).with_status_code(status);
    if let Some(h) = make_header("Content-Type", "text/html; charset=utf-8") {
        resp.add_header(h);
    }
    req.respond(resp)?;
    Ok(())
}

pub fn respond_json(handle: i64, status: u16, body: &str) -> Result<(), ServerError> {
    let req = take_request(handle)?;
    let mut resp = Response::from_string(body).with_status_code(status);
    if let Some(h) = make_header("Content-Type", "application/json; charset=utf-8") {
        resp.add_header(h);
    }
    req.respond(resp)?;
    Ok(())
}

pub fn respond_bytes(
    handle: i64,
    status: u16,
    content_type: &str,
    data: Vec<u8>,
) -> Result<(), ServerError> {
    let req = take_request(handle)?;
    let mut resp = Response::from_data(data).with_status_code(status);
    if let Some(h) = make_header("Content-Type", content_type) {
        resp.add_header(h);
    }
    req.respond(resp)?;
    Ok(())
}

/// Send a response with arbitrary headers.
pub fn respond_full(
    handle: i64,
    status: u16,
    content_type: &str,
    extra_headers: &[(String, String)],
    data: Vec<u8>,
) -> Result<(), ServerError> {
    let req = take_request(handle)?;
    let mut resp = Response::from_data(data).with_status_code(status);
    if let Some(h) = make_header("Content-Type", content_type) {
        resp.add_header(h);
    }
    for (k, v) in extra_headers {
        if k.contains(['\r', '\n']) || v.contains(['\r', '\n']) {
            return Err(ServerError::InvalidHeader);
        }
        if let Some(h) = make_header(k, v) {
            resp.add_header(h);
        }
    }
    req.respond(resp)?;
    Ok(())
}

// ---- WebSocket upgrade (RFC 6455) -----------------------------------

/// Upgraded WebSocket connection. We keep the underlying reader/writer
/// (a `tiny_http::ReadWrite` boxed trait object over the original TCP
/// stream) plus a `MessageDecoder` that reassembles fragmented text and
/// binary messages according to RFC 6455.
struct WsConn {
    stream: Box<dyn tiny_http::ReadWrite + Send>,
    decoder: MessageDecoder,
}

struct WsRegistry {
    entries: HashMap<(u64, i64), WsConn>,
    next_id: i64,
}
fn ws_registry() -> &'static Mutex<WsRegistry> {
    static REG: OnceLock<Mutex<WsRegistry>> = OnceLock::new();
    REG.get_or_init(|| {
        Mutex::new(WsRegistry {
            entries: HashMap::new(),
            next_id: 1_000_000_000,
        })
    })
}
fn insert_ws(ws: WsConn) -> i64 {
    let mut r = ws_registry().lock().expect("ws registry poisoned");
    let id = r.next_id;
    r.next_id += 1;
    r.entries.insert(handle_key(id), ws);
    id
}

/// Upgrade an HTTP request to a WebSocket connection (RFC 6455). Returns
/// an opaque WS handle. After a successful upgrade the request is
/// consumed; use `ws_recv` / `ws_send_text` / `ws_send_binary` /
/// `ws_close` to drive the conversation.
///
/// `max_message` is the largest reassembled payload we'll accept from
/// the peer (in bytes) — set generously (e.g. 1 MiB) or defensively.
pub fn upgrade_websocket(handle: i64, max_message: usize) -> Result<i64, ServerError> {
    let req = take_request(handle)?;
    // Extract Sec-WebSocket-Key.
    let key = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("Sec-WebSocket-Key"))
        .map(|h| h.value.as_str().to_string())
        .ok_or(ServerError::MissingHeader("Sec-WebSocket-Key"))?;
    let accept = ws_codec::accept_key(&key)
        .map_err(|_| ServerError::Upgrade("invalid Sec-WebSocket-Key"))?;

    // Build the 101 response and hand it to tiny_http's `upgrade` API,
    // which returns the raw duplex stream after having written the
    // response headers. We do NOT send anything else — the peer speaks
    // WebSocket frames from now on.
    let resp = Response::empty(101)
        .with_header(Header::from_bytes(b"Upgrade".as_slice(), b"websocket".as_slice()).unwrap())
        .with_header(Header::from_bytes(b"Connection".as_slice(), b"Upgrade".as_slice()).unwrap())
        .with_header(
            Header::from_bytes(b"Sec-WebSocket-Accept".as_slice(), accept.as_bytes()).unwrap(),
        );

    let stream = req.upgrade("websocket", resp);
    let ws = WsConn {
        stream,
        decoder: MessageDecoder::new(max_message),
    };
    Ok(insert_ws(ws))
}

/// Read one *message* from the client (frames are reassembled). Returns
/// a triple `(kind, text_or_empty, bytes)` where `kind` is one of
/// `"text"`, `"binary"`, `"ping"`, `"pong"`, `"close"`.
///
/// Ping frames are answered automatically before this call returns, but
/// the ping payload is still surfaced so the app can log it. Close
/// frames are echoed back; after receiving a `"close"`, call `ws_close`
/// to release the handle.
pub fn ws_recv(handle: i64) -> Result<(String, String, Vec<u8>), ServerError> {
    let mut r = ws_registry().lock().expect("ws registry poisoned");
    let ws = r
        .entries
        .get_mut(&handle_key(handle))
        .ok_or(ServerError::UnknownWebSocket(handle))?;

    // Drive the decoder: pull bytes until a full message pops out. We
    // require peer frames to be masked (`Some(true)`) as RFC 6455 §5.1
    // mandates for server-side reads.
    loop {
        match ws.decoder.next(Some(true))? {
            Some(Message::Text(text)) => {
                return Ok(("text".into(), text.clone(), text.into_bytes()));
            }
            Some(Message::Binary(bytes)) => {
                return Ok(("binary".into(), String::new(), bytes));
            }
            Some(Message::Ping(payload)) => {
                // Auto-reply with a Pong echoing the same payload.
                let pong = ws_codec::encode_frame(true, WS_OP_PONG, &payload, None)?;
                ws.stream.write_all(&pong)?;
                return Ok(("ping".into(), String::new(), payload));
            }
            Some(Message::Pong(payload)) => {
                return Ok(("pong".into(), String::new(), payload));
            }
            Some(Message::Close { code, reason }) => {
                // Echo the Close frame back.
                let mut payload = Vec::new();
                if let Some(code) = code {
                    payload.extend_from_slice(&code.to_be_bytes());
                }
                payload.extend_from_slice(reason.as_bytes());
                let close = ws_codec::encode_frame(true, WS_OP_CLOSE, &payload, None)?;
                let _ = ws.stream.write_all(&close);
                return Ok(("close".into(), reason, payload));
            }
            None => {
                // Need more bytes.
                let mut buf = [0u8; 4096];
                let n = ws.stream.read(&mut buf)?;
                if n == 0 {
                    return Err(ServerError::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "websocket peer closed",
                    )));
                }
                ws.decoder.push(&buf[..n])?;
            }
        }
    }
}

/// Send a UTF-8 text message. Server frames are unmasked per RFC 6455.
pub fn ws_send_text(handle: i64, text: &str) -> Result<(), ServerError> {
    let mut r = ws_registry().lock().expect("ws registry poisoned");
    let ws = r
        .entries
        .get_mut(&handle_key(handle))
        .ok_or(ServerError::UnknownWebSocket(handle))?;
    let frame = ws_codec::encode_frame(true, WS_OP_TEXT, text.as_bytes(), None)?;
    ws.stream.write_all(&frame)?;
    Ok(())
}

/// Send a binary message.
pub fn ws_send_binary(handle: i64, data: &[u8]) -> Result<(), ServerError> {
    let mut r = ws_registry().lock().expect("ws registry poisoned");
    let ws = r
        .entries
        .get_mut(&handle_key(handle))
        .ok_or(ServerError::UnknownWebSocket(handle))?;
    let frame = ws_codec::encode_frame(true, WS_OP_BINARY, data, None)?;
    ws.stream.write_all(&frame)?;
    Ok(())
}

/// Send a Close frame (optionally with code+reason) and drop the handle.
pub fn ws_close(handle: i64, code: Option<u16>, reason: &str) -> Result<(), ServerError> {
    let mut r = ws_registry().lock().expect("ws registry poisoned");
    if let Some(mut ws) = r.entries.remove(&handle_key(handle)) {
        let mut payload = Vec::new();
        if let Some(code) = code {
            payload.extend_from_slice(&code.to_be_bytes());
        }
        payload.extend_from_slice(reason.as_bytes());
        if let Ok(frame) = ws_codec::encode_frame(true, WS_OP_CLOSE, &payload, None) {
            let _ = ws.stream.write_all(&frame);
        }
    }
    Ok(())
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let mut released = {
        let mut reg = crate::native::lock_recover(registry());
        let requests = crate::native::remove_runtime_entries(&mut reg.requests, runtime_id);
        requests + crate::native::remove_runtime_entries(&mut reg.servers, runtime_id)
    };
    let mut websockets = crate::native::lock_recover(ws_registry());
    released += crate::native::remove_runtime_entries(&mut websockets.entries, runtime_id);
    released
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_bind_ephemeral_and_stop() {
        let h = start("127.0.0.1:0").expect("bind ephemeral");
        let addr = local_addr(h).expect("addr");
        assert!(addr.starts_with("127.0.0.1:"), "got {addr}");
        // No client → timeout returns -1.
        assert_eq!(accept(h, 50).unwrap(), -1);
        stop(h);
    }

    #[test]
    fn unknown_handles_report_typed_errors() {
        assert!(matches!(
            method(999_999),
            Err(ServerError::UnknownRequest(_))
        ));
        assert!(matches!(
            local_addr(999_999),
            Err(ServerError::UnknownServer(_))
        ));
        assert!(matches!(
            ws_recv(999_999),
            Err(ServerError::UnknownWebSocket(_))
        ));
    }
}

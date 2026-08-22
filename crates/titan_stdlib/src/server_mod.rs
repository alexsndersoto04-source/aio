//! Bounded HTTP/1.1 and WebSocket server (`std::server::*`).
//!
//! The original implementation delegated request parsing to `tiny_http`, whose
//! request-line and header readers grow `Vec`s without a configurable limit.
//! This module owns the small HTTP/1.1 transport instead, so hostile lengths are
//! rejected before allocation. Each accepted TCP connection serves one request
//! (or is transferred to WebSocket) and responses explicitly close HTTP
//! connections; this intentionally simple lifecycle is portable to Android and
//! keeps memory, blocking work, and cleanup deterministic.

use std::collections::{BTreeMap, HashMap};
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::websocket::{self as ws_codec, Message, MessageDecoder};

const MAX_SERVERS_PER_RUNTIME: usize = 8;
const MAX_REQUESTS_PER_RUNTIME: usize = 256;
const MAX_WEBSOCKETS_PER_RUNTIME: usize = 64;
const MAX_REQUEST_METADATA_PER_RUNTIME: usize = 16 * 1024 * 1024;
const MAX_CONCURRENT_OPERATIONS: usize = 8;
const MAX_BIND_ADDRESS_BYTES: usize = 4 * 1024;
const MAX_ACCEPT_TIMEOUT_MS: u64 = 30_000;
const MAX_REQUEST_HEAD_BYTES: usize = 64 * 1024;
const MAX_REQUEST_TARGET_BYTES: usize = 16 * 1024;
const MAX_REQUEST_HEADERS: usize = 128;
const MAX_HEADER_NAME_BYTES: usize = 256;
const MAX_HEADER_VALUE_BYTES: usize = 8 * 1024;
const MAX_HEADER_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CONTENT_TYPE_BYTES: usize = 1024;
const MAX_CHUNK_LINE_BYTES: usize = 8 * 1024;
const MAX_TRAILER_BYTES: usize = 16 * 1024;
const MAX_TRAILERS: usize = 32;
const MAX_WEBSOCKET_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(5);
#[cfg(not(test))]
const IO_DEADLINE: Duration = Duration::from_secs(5);
// The test deadline stays below the production one so the timeout tests do
// not stall the suite, but it must still be generous enough for the slowest
// hosted CI runner. It used to be 300ms, which is ample on Linux and on a
// developer machine yet too tight on GitHub's macOS runners: `accept` parses
// the request head under this deadline regardless of the caller's own accept
// timeout, so a client thread that is simply slow to be scheduled turned a
// healthy round-trip test into an intermittent `ServerError::Timeout`.
// Tests that deliberately wait for the deadline derive their sleeps from this
// constant instead of hardcoding a matching literal.
#[cfg(test)]
const IO_DEADLINE: Duration = Duration::from_secs(2);

const WS_OP_TEXT: u8 = 0x1;
const WS_OP_BINARY: u8 = 0x2;
const WS_OP_CLOSE: u8 = 0x8;
const WS_OP_PONG: u8 = 0xA;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("HTTP server error: {0}")]
    Http(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unknown server handle {0}")]
    UnknownServer(i64),
    #[error("unknown request handle {0}")]
    UnknownRequest(i64),
    #[error("unknown websocket handle {0}")]
    UnknownWebSocket(i64),
    #[error("server handle {0} is closing or closed")]
    ClosedServer(i64),
    #[error("request handle {0} is closing or closed")]
    ClosedRequest(i64),
    #[error("websocket handle {0} is closing or closed")]
    ClosedWebSocket(i64),
    #[error("request body has already been consumed")]
    BodyConsumed,
    #[error("request body is not valid UTF-8")]
    Utf8,
    #[error("invalid server argument: {0}")]
    InvalidArgument(&'static str),
    #[error("invalid HTTP request: {0}")]
    InvalidRequest(&'static str),
    #[error("invalid HTTP response: {0}")]
    InvalidResponse(&'static str),
    #[error("websocket upgrade failed: {0}")]
    Upgrade(&'static str),
    #[error("websocket protocol error: {0}")]
    WebSocket(#[from] ws_codec::WebSocketError),
    #[error("{operation} timed out")]
    Timeout { operation: &'static str },
    #[error("{resource} is busy")]
    Busy { resource: &'static str },
    #[error("{resource} exceeds limit {limit}")]
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    #[error("server handle space exhausted")]
    HandleSpaceExhausted,
    #[error("server runtime ownership ended while a resource was being created")]
    RuntimeClosed,
}

fn map_io(error: std::io::Error, operation: &'static str) -> ServerError {
    if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) {
        ServerError::Timeout { operation }
    } else {
        ServerError::Io(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum HandleKind {
    Server,
    Request,
    WebSocket,
}

struct ServerEntry {
    listener: TcpListener,
    closed: AtomicBool,
}

impl ServerEntry {
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
}

#[derive(Clone, Copy)]
enum BodyMode {
    Empty,
    Fixed(usize),
    Chunked,
}

struct RequestIo {
    stream: Option<TcpStream>,
    prefetched: Vec<u8>,
    prefix_offset: usize,
    body_consumed: bool,
}

struct RequestEntry {
    method: String,
    target: String,
    version: String,
    headers: Vec<(String, String)>,
    remote_addr: String,
    body_mode: BodyMode,
    expect_continue: bool,
    metadata_bytes: usize,
    io: Mutex<RequestIo>,
    shutdown: TcpStream,
    closed: AtomicBool,
}

impl RequestEntry {
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        let _ = self.shutdown.shutdown(Shutdown::Both);
    }
}

struct WsConn {
    stream: TcpStream,
    decoder: MessageDecoder,
    prefetched: Vec<u8>,
}

struct WsEntry {
    conn: Mutex<WsConn>,
    shutdown: TcpStream,
    closed: AtomicBool,
}

impl WsEntry {
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        let _ = self.shutdown.shutdown(Shutdown::Both);
    }
}

struct Registry {
    servers: HashMap<(u64, i64), Arc<ServerEntry>>,
    requests: HashMap<(u64, i64), Arc<RequestEntry>>,
    websockets: HashMap<(u64, i64), Arc<WsEntry>>,
    reserved: HashMap<(u64, HandleKind), usize>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            servers: HashMap::new(),
            requests: HashMap::new(),
            websockets: HashMap::new(),
            reserved: HashMap::new(),
            next_id: 1,
        })
    })
}

#[derive(Default)]
struct RuntimeOperations {
    active: usize,
}

fn operation_usage() -> &'static Mutex<HashMap<u64, RuntimeOperations>> {
    static USAGE: OnceLock<Mutex<HashMap<u64, RuntimeOperations>>> = OnceLock::new();
    USAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

struct OperationPermit {
    runtime_id: u64,
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        let mut usage = crate::native::lock_recover(operation_usage());
        if let Some(runtime) = usage.get_mut(&self.runtime_id) {
            runtime.active = runtime.active.saturating_sub(1);
            if runtime.active == 0 {
                usage.remove(&self.runtime_id);
            }
        }
    }
}

fn reserve_operation() -> Result<OperationPermit, ServerError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut usage = crate::native::lock_recover(operation_usage());
    let runtime = usage.entry(runtime_id).or_default();
    if runtime.active >= MAX_CONCURRENT_OPERATIONS {
        return Err(ServerError::ResourceLimit {
            resource: "concurrent server operations",
            limit: MAX_CONCURRENT_OPERATIONS,
        });
    }
    runtime.active += 1;
    Ok(OperationPermit { runtime_id })
}

fn handle_key(handle: i64) -> (u64, i64) {
    crate::native::runtime_handle_key(handle)
}

fn kind_limit(kind: HandleKind) -> usize {
    match kind {
        HandleKind::Server => MAX_SERVERS_PER_RUNTIME,
        HandleKind::Request => MAX_REQUESTS_PER_RUNTIME,
        HandleKind::WebSocket => MAX_WEBSOCKETS_PER_RUNTIME,
    }
}

fn kind_name(kind: HandleKind) -> &'static str {
    match kind {
        HandleKind::Server => "HTTP server handles",
        HandleKind::Request => "pending HTTP request handles",
        HandleKind::WebSocket => "server WebSocket handles",
    }
}

fn active_handles(registry: &Registry, runtime_id: u64, kind: HandleKind) -> usize {
    match kind {
        HandleKind::Server => registry
            .servers
            .keys()
            .filter(|(owner, _)| *owner == runtime_id)
            .count(),
        HandleKind::Request => registry
            .requests
            .keys()
            .filter(|(owner, _)| *owner == runtime_id)
            .count(),
        HandleKind::WebSocket => registry
            .websockets
            .keys()
            .filter(|(owner, _)| *owner == runtime_id)
            .count(),
    }
}

fn release_reservation(registry: &mut Registry, runtime_id: u64, kind: HandleKind) {
    let key = (runtime_id, kind);
    if let Some(reserved) = registry.reserved.get_mut(&key) {
        *reserved = reserved.saturating_sub(1);
        if *reserved == 0 {
            registry.reserved.remove(&key);
        }
    }
}

struct HandleReservation {
    runtime_id: u64,
    kind: HandleKind,
    committed: bool,
}

fn reserve_handle(kind: HandleKind) -> Result<HandleReservation, ServerError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(registry());
    let active = active_handles(&registry, runtime_id, kind);
    let reserved = registry
        .reserved
        .get(&(runtime_id, kind))
        .copied()
        .unwrap_or(0);
    let limit = kind_limit(kind);
    if active.saturating_add(reserved) >= limit {
        return Err(ServerError::ResourceLimit {
            resource: kind_name(kind),
            limit,
        });
    }
    *registry.reserved.entry((runtime_id, kind)).or_default() += 1;
    Ok(HandleReservation {
        runtime_id,
        kind,
        committed: false,
    })
}

enum NewHandle {
    Server(Arc<ServerEntry>),
    Request(Arc<RequestEntry>),
    WebSocket(Arc<WsEntry>),
}

impl HandleReservation {
    fn commit(mut self, resource: NewHandle) -> Result<i64, ServerError> {
        let resource_kind = match &resource {
            NewHandle::Server(_) => HandleKind::Server,
            NewHandle::Request(_) => HandleKind::Request,
            NewHandle::WebSocket(_) => HandleKind::WebSocket,
        };
        if resource_kind != self.kind {
            return Err(ServerError::InvalidArgument(
                "server handle reservation kind mismatch",
            ));
        }

        let mut registry = crate::native::lock_recover(registry());
        if registry
            .reserved
            .get(&(self.runtime_id, self.kind))
            .copied()
            .unwrap_or(0)
            == 0
        {
            return Err(ServerError::RuntimeClosed);
        }
        if let NewHandle::Request(request) = &resource {
            let current_bytes = registry
                .requests
                .iter()
                .filter(|((owner, _), _)| *owner == self.runtime_id)
                .try_fold(0usize, |total, (_, request)| {
                    total.checked_add(request.metadata_bytes)
                })
                .ok_or(ServerError::ResourceLimit {
                    resource: "HTTP request metadata bytes per runtime",
                    limit: MAX_REQUEST_METADATA_PER_RUNTIME,
                })?;
            let new_bytes = current_bytes.checked_add(request.metadata_bytes).ok_or(
                ServerError::ResourceLimit {
                    resource: "HTTP request metadata bytes per runtime",
                    limit: MAX_REQUEST_METADATA_PER_RUNTIME,
                },
            )?;
            if new_bytes > MAX_REQUEST_METADATA_PER_RUNTIME {
                return Err(ServerError::ResourceLimit {
                    resource: "HTTP request metadata bytes per runtime",
                    limit: MAX_REQUEST_METADATA_PER_RUNTIME,
                });
            }
        }

        let id = registry.next_id;
        registry.next_id = id
            .checked_add(1)
            .ok_or(ServerError::HandleSpaceExhausted)?;
        release_reservation(&mut registry, self.runtime_id, self.kind);
        let key = (self.runtime_id, id);
        match resource {
            NewHandle::Server(server) => {
                registry.servers.insert(key, server);
            }
            NewHandle::Request(request) => {
                registry.requests.insert(key, request);
            }
            NewHandle::WebSocket(websocket) => {
                registry.websockets.insert(key, websocket);
            }
        }
        self.committed = true;
        Ok(id)
    }
}

impl Drop for HandleReservation {
    fn drop(&mut self) {
        if !self.committed {
            let mut registry = crate::native::lock_recover(registry());
            release_reservation(&mut registry, self.runtime_id, self.kind);
        }
    }
}

fn get_server(handle: i64) -> Result<Arc<ServerEntry>, ServerError> {
    crate::native::lock_recover(registry())
        .servers
        .get(&handle_key(handle))
        .cloned()
        .ok_or(ServerError::UnknownServer(handle))
}

fn get_request(handle: i64) -> Result<Arc<RequestEntry>, ServerError> {
    crate::native::lock_recover(registry())
        .requests
        .get(&handle_key(handle))
        .cloned()
        .ok_or(ServerError::UnknownRequest(handle))
}

fn take_request(handle: i64) -> Result<Arc<RequestEntry>, ServerError> {
    crate::native::lock_recover(registry())
        .requests
        .remove(&handle_key(handle))
        .ok_or(ServerError::UnknownRequest(handle))
}

fn get_websocket(handle: i64) -> Result<Arc<WsEntry>, ServerError> {
    crate::native::lock_recover(registry())
        .websockets
        .get(&handle_key(handle))
        .cloned()
        .ok_or(ServerError::UnknownWebSocket(handle))
}

fn take_websocket(handle: i64) -> Option<Arc<WsEntry>> {
    crate::native::lock_recover(registry())
        .websockets
        .remove(&handle_key(handle))
}

fn validate_size(
    value: &str,
    resource: &'static str,
    limit: usize,
) -> Result<(), ServerError> {
    if value.len() > limit {
        return Err(ServerError::ResourceLimit { resource, limit });
    }
    Ok(())
}

fn remaining(deadline: Instant, operation: &'static str) -> Result<Duration, ServerError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(ServerError::Timeout { operation })
}

fn read_deadline(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    deadline: Instant,
    operation: &'static str,
) -> Result<usize, ServerError> {
    stream
        .set_read_timeout(Some(remaining(deadline, operation)?))
        .map_err(|error| map_io(error, operation))?;
    stream
        .read(buffer)
        .map_err(|error| map_io(error, operation))
}

fn write_all_deadline(
    stream: &mut TcpStream,
    mut bytes: &[u8],
    deadline: Instant,
    operation: &'static str,
) -> Result<(), ServerError> {
    while !bytes.is_empty() {
        stream
            .set_write_timeout(Some(remaining(deadline, operation)?))
            .map_err(|error| map_io(error, operation))?;
        let written = stream
            .write(bytes)
            .map_err(|error| map_io(error, operation))?;
        if written == 0 {
            return Err(ServerError::Io(std::io::Error::new(
                ErrorKind::WriteZero,
                "socket wrote zero bytes",
            )));
        }
        bytes = &bytes[written..];
    }
    Ok(())
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn is_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
        })
}

fn is_header_value(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte == b'\t' || (0x20..=0x7e).contains(&byte))
}

fn parse_content_length(value: &str) -> Result<usize, ServerError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ServerError::InvalidRequest("invalid Content-Length"));
    }
    let length = value
        .parse::<u64>()
        .map_err(|_| ServerError::InvalidRequest("invalid Content-Length"))?;
    let length = usize::try_from(length).map_err(|_| ServerError::ResourceLimit {
        resource: "HTTP request body bytes",
        limit: MAX_BODY_BYTES,
    })?;
    if length > MAX_BODY_BYTES {
        return Err(ServerError::ResourceLimit {
            resource: "HTTP request body bytes",
            limit: MAX_BODY_BYTES,
        });
    }
    Ok(length)
}

fn header_values<'a>(headers: &'a [(String, String)], name: &str) -> Vec<&'a str> {
    headers
        .iter()
        .filter(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
        .collect()
}

fn parse_request(mut stream: TcpStream, remote_addr: String) -> Result<RequestEntry, ServerError> {
    stream
        .set_nodelay(true)
        .map_err(|error| map_io(error, "configure accepted HTTP connection"))?;
    let deadline = Instant::now() + IO_DEADLINE;
    let mut raw = Vec::with_capacity(4096);
    let header_end = loop {
        if let Some(end) = find_header_end(&raw) {
            if end + 4 > MAX_REQUEST_HEAD_BYTES {
                return Err(ServerError::ResourceLimit {
                    resource: "HTTP request head bytes",
                    limit: MAX_REQUEST_HEAD_BYTES,
                });
            }
            break end;
        }
        if raw.len() >= MAX_REQUEST_HEAD_BYTES {
            return Err(ServerError::ResourceLimit {
                resource: "HTTP request head bytes",
                limit: MAX_REQUEST_HEAD_BYTES,
            });
        }
        let mut buffer = [0u8; 4096];
        let capacity = (MAX_REQUEST_HEAD_BYTES - raw.len()).min(buffer.len());
        let read = read_deadline(
            &mut stream,
            &mut buffer[..capacity],
            deadline,
            "read HTTP request headers",
        )?;
        if read == 0 {
            return Err(ServerError::InvalidRequest(
                "connection closed before request headers completed",
            ));
        }
        raw.extend_from_slice(&buffer[..read]);
    };

    let prefetched = raw[header_end + 4..].to_vec();
    let head = std::str::from_utf8(&raw[..header_end])
        .map_err(|_| ServerError::InvalidRequest("request headers are not UTF-8/ASCII"))?;
    if !head.is_ascii() {
        return Err(ServerError::InvalidRequest(
            "request headers contain non-ASCII bytes",
        ));
    }
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or(ServerError::InvalidRequest("missing request line"))?;
    let mut parts = request_line.split(' ');
    let method = parts
        .next()
        .filter(|method| is_header_name(method) && method.len() <= 32)
        .ok_or(ServerError::InvalidRequest("invalid HTTP method"))?;
    let target = parts
        .next()
        .filter(|target| !target.is_empty())
        .ok_or(ServerError::InvalidRequest("invalid request target"))?;
    let version = parts
        .next()
        .filter(|version| matches!(*version, "HTTP/1.0" | "HTTP/1.1"))
        .ok_or(ServerError::InvalidRequest("unsupported HTTP version"))?;
    if parts.next().is_some() {
        return Err(ServerError::InvalidRequest("malformed request line"));
    }
    validate_size(
        target,
        "HTTP request target bytes",
        MAX_REQUEST_TARGET_BYTES,
    )?;
    if target.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(ServerError::InvalidRequest(
            "request target contains control bytes",
        ));
    }

    let mut headers = Vec::new();
    for line in lines {
        if headers.len() >= MAX_REQUEST_HEADERS {
            return Err(ServerError::ResourceLimit {
                resource: "HTTP request headers",
                limit: MAX_REQUEST_HEADERS,
            });
        }
        if line.starts_with([' ', '\t']) {
            return Err(ServerError::InvalidRequest(
                "folded HTTP headers are not accepted",
            ));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or(ServerError::InvalidRequest("malformed HTTP header"))?;
        let value = value.trim_matches([' ', '\t']);
        validate_size(name, "HTTP header name bytes", MAX_HEADER_NAME_BYTES)?;
        validate_size(value, "HTTP header value bytes", MAX_HEADER_VALUE_BYTES)?;
        if !is_header_name(name) || !is_header_value(value) {
            return Err(ServerError::InvalidRequest("invalid HTTP header"));
        }
        headers.push((name.to_string(), value.to_string()));
    }

    let hosts = header_values(&headers, "Host");
    if version == "HTTP/1.1" && (hosts.len() != 1 || hosts[0].is_empty()) {
        return Err(ServerError::InvalidRequest(
            "HTTP/1.1 requires exactly one nonempty Host header",
        ));
    }
    let content_lengths = header_values(&headers, "Content-Length");
    let transfer_encodings = header_values(&headers, "Transfer-Encoding");
    if content_lengths.len() > 1 {
        return Err(ServerError::InvalidRequest(
            "multiple Content-Length headers are not accepted",
        ));
    }
    if transfer_encodings.len() > 1 || (!content_lengths.is_empty() && !transfer_encodings.is_empty())
    {
        return Err(ServerError::InvalidRequest(
            "ambiguous request body framing",
        ));
    }
    let body_mode = if let Some(value) = transfer_encodings.first() {
        if !value.eq_ignore_ascii_case("chunked") {
            return Err(ServerError::InvalidRequest(
                "only chunked Transfer-Encoding is supported",
            ));
        }
        BodyMode::Chunked
    } else if let Some(value) = content_lengths.first() {
        let length = parse_content_length(value)?;
        if length == 0 {
            BodyMode::Empty
        } else {
            BodyMode::Fixed(length)
        }
    } else {
        BodyMode::Empty
    };

    let expects = header_values(&headers, "Expect");
    if expects.len() > 1
        || expects
            .first()
            .is_some_and(|value| !value.eq_ignore_ascii_case("100-continue"))
    {
        return Err(ServerError::InvalidRequest("unsupported Expect header"));
    }
    let expect_continue = !expects.is_empty() && !matches!(body_mode, BodyMode::Empty);
    let metadata_bytes = header_end
        .checked_add(4)
        .and_then(|bytes| bytes.checked_add(remote_addr.len()))
        .ok_or(ServerError::ResourceLimit {
            resource: "HTTP request metadata bytes",
            limit: MAX_REQUEST_HEAD_BYTES + 128,
        })?;
    let shutdown = stream
        .try_clone()
        .map_err(|error| map_io(error, "clone HTTP connection for cleanup"))?;

    Ok(RequestEntry {
        method: method.to_string(),
        target: target.to_string(),
        version: version.to_string(),
        headers,
        remote_addr,
        body_mode,
        expect_continue,
        metadata_bytes,
        io: Mutex::new(RequestIo {
            stream: Some(stream),
            prefetched,
            prefix_offset: 0,
            body_consumed: false,
        }),
        shutdown,
        closed: AtomicBool::new(false),
    })
}

/// Start an HTTP/1.1 server on a numeric IP address and port.
pub fn start(addr: &str) -> Result<i64, ServerError> {
    let _operation = reserve_operation()?;
    validate_size(
        addr,
        "HTTP bind address bytes",
        MAX_BIND_ADDRESS_BYTES,
    )?;
    let address = addr.parse::<SocketAddr>().map_err(|_| {
        ServerError::InvalidArgument(
            "bind address must be a numeric IP and port, such as 127.0.0.1:8080 or [::1]:8080",
        )
    })?;
    let reservation = reserve_handle(HandleKind::Server)?;
    let listener = TcpListener::bind(address)?;
    listener.set_nonblocking(true)?;
    reservation.commit(NewHandle::Server(Arc::new(ServerEntry {
        listener,
        closed: AtomicBool::new(false),
    })))
}

pub fn local_addr(handle: i64) -> Result<String, ServerError> {
    let _operation = reserve_operation()?;
    let server = get_server(handle)?;
    if server.closed.load(Ordering::Acquire) {
        return Err(ServerError::ClosedServer(handle));
    }
    Ok(server.listener.local_addr()?.to_string())
}

/// Accept one request. The timeout applies to waiting for a TCP connection;
/// once connected, the bounded request head has its own fixed total deadline.
pub fn accept(handle: i64, timeout_ms: u64) -> Result<i64, ServerError> {
    let _operation = reserve_operation()?;
    if timeout_ms > MAX_ACCEPT_TIMEOUT_MS {
        return Err(ServerError::ResourceLimit {
            resource: "HTTP accept timeout milliseconds",
            limit: MAX_ACCEPT_TIMEOUT_MS as usize,
        });
    }
    let reservation = reserve_handle(HandleKind::Request)?;
    let server = get_server(handle)?;
    if server.closed.load(Ordering::Acquire) {
        return Err(ServerError::ClosedServer(handle));
    }
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let (stream, remote) = loop {
        if server.closed.load(Ordering::Acquire) {
            return Err(ServerError::ClosedServer(handle));
        }
        match server.listener.accept() {
            Ok(connection) => break connection,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Ok(-1);
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                thread::sleep(ACCEPT_POLL_INTERVAL.min(remaining));
            }
            Err(error) => return Err(ServerError::Io(error)),
        }
    };
    if server.closed.load(Ordering::Acquire) {
        return Err(ServerError::ClosedServer(handle));
    }
    let request = Arc::new(parse_request(stream, remote.to_string())?);
    reservation.commit(NewHandle::Request(request))
}

/// Stop a server. Idempotent and never performs network I/O under the registry.
pub fn stop(handle: i64) {
    let server = crate::native::lock_recover(registry())
        .servers
        .remove(&handle_key(handle));
    if let Some(server) = server {
        server.close();
    }
}

fn ensure_request_open(request: &RequestEntry, handle: i64) -> Result<(), ServerError> {
    if request.closed.load(Ordering::Acquire) {
        Err(ServerError::ClosedRequest(handle))
    } else {
        Ok(())
    }
}

pub fn method(handle: i64) -> Result<String, ServerError> {
    let _operation = reserve_operation()?;
    let request = get_request(handle)?;
    ensure_request_open(&request, handle)?;
    Ok(request.method.clone())
}

pub fn url(handle: i64) -> Result<String, ServerError> {
    let _operation = reserve_operation()?;
    let request = get_request(handle)?;
    ensure_request_open(&request, handle)?;
    Ok(request.target.clone())
}

pub fn path(handle: i64) -> Result<String, ServerError> {
    let _operation = reserve_operation()?;
    let request = get_request(handle)?;
    ensure_request_open(&request, handle)?;
    Ok(request
        .target
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string())
}

pub fn query(handle: i64) -> Result<String, ServerError> {
    let _operation = reserve_operation()?;
    let request = get_request(handle)?;
    ensure_request_open(&request, handle)?;
    Ok(request
        .target
        .find('?')
        .map(|index| request.target[index + 1..].to_string())
        .unwrap_or_default())
}

pub fn remote_addr(handle: i64) -> Result<String, ServerError> {
    let _operation = reserve_operation()?;
    let request = get_request(handle)?;
    ensure_request_open(&request, handle)?;
    Ok(request.remote_addr.clone())
}

pub fn header(handle: i64, name: &str) -> Result<Option<String>, ServerError> {
    let _operation = reserve_operation()?;
    validate_size(name, "HTTP header lookup name bytes", MAX_HEADER_NAME_BYTES)?;
    if !is_header_name(name) {
        return Err(ServerError::InvalidArgument("invalid HTTP header name"));
    }
    let request = get_request(handle)?;
    ensure_request_open(&request, handle)?;
    Ok(request
        .headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone()))
}

pub fn headers(handle: i64) -> Result<BTreeMap<String, String>, ServerError> {
    let _operation = reserve_operation()?;
    let request = get_request(handle)?;
    ensure_request_open(&request, handle)?;
    let mut output = BTreeMap::<String, String>::new();
    let mut output_bytes = 0usize;
    for (name, value) in &request.headers {
        output_bytes = output_bytes
            .checked_add(name.len())
            .and_then(|bytes| bytes.checked_add(value.len()))
            .and_then(|bytes| bytes.checked_add(2))
            .ok_or(ServerError::ResourceLimit {
                resource: "rendered HTTP request header bytes",
                limit: MAX_HEADER_OUTPUT_BYTES,
            })?;
        if output_bytes > MAX_HEADER_OUTPUT_BYTES {
            return Err(ServerError::ResourceLimit {
                resource: "rendered HTTP request header bytes",
                limit: MAX_HEADER_OUTPUT_BYTES,
            });
        }
        output
            .entry(name.to_ascii_lowercase())
            .and_modify(|existing| {
                existing.push_str(", ");
                existing.push_str(value);
            })
            .or_insert_with(|| value.clone());
    }
    Ok(output)
}

struct BodyReader<'a> {
    stream: &'a mut TcpStream,
    prefetched: &'a [u8],
    prefix_offset: &'a mut usize,
    deadline: Instant,
}

impl BodyReader<'_> {
    fn read_some(
        &mut self,
        output: &mut [u8],
        operation: &'static str,
    ) -> Result<usize, ServerError> {
        if *self.prefix_offset < self.prefetched.len() {
            let available = &self.prefetched[*self.prefix_offset..];
            let count = available.len().min(output.len());
            output[..count].copy_from_slice(&available[..count]);
            *self.prefix_offset += count;
            return Ok(count);
        }
        read_deadline(self.stream, output, self.deadline, operation)
    }

    fn read_exact(
        &mut self,
        output: &mut [u8],
        operation: &'static str,
    ) -> Result<(), ServerError> {
        let mut offset = 0;
        while offset < output.len() {
            let read = self.read_some(&mut output[offset..], operation)?;
            if read == 0 {
                return Err(ServerError::InvalidRequest(
                    "connection closed before request body completed",
                ));
            }
            offset += read;
        }
        Ok(())
    }

    fn read_line(
        &mut self,
        limit: usize,
        operation: &'static str,
    ) -> Result<Vec<u8>, ServerError> {
        let mut line = Vec::new();
        loop {
            if line.len() >= limit {
                return Err(ServerError::ResourceLimit {
                    resource: "HTTP chunk or trailer line bytes",
                    limit,
                });
            }
            let mut byte = [0u8; 1];
            self.read_exact(&mut byte, operation)?;
            line.push(byte[0]);
            if line.ends_with(b"\r\n") {
                line.truncate(line.len() - 2);
                return Ok(line);
            }
        }
    }
}

fn read_fixed_body(reader: &mut BodyReader<'_>, length: usize) -> Result<Vec<u8>, ServerError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| ServerError::ResourceLimit {
            resource: "HTTP request body bytes",
            limit: MAX_BODY_BYTES,
        })?;
    output.resize(length, 0);
    reader.read_exact(&mut output, "read HTTP request body")?;
    Ok(output)
}

fn read_chunked_body(reader: &mut BodyReader<'_>) -> Result<Vec<u8>, ServerError> {
    let mut output = Vec::new();
    loop {
        let line = reader.read_line(MAX_CHUNK_LINE_BYTES, "read HTTP chunk header")?;
        let line = std::str::from_utf8(&line)
            .map_err(|_| ServerError::InvalidRequest("chunk length is not ASCII"))?;
        let size_text = line.split(';').next().unwrap_or("").trim();
        if size_text.is_empty()
            || size_text.len() > 16
            || !size_text.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(ServerError::InvalidRequest("invalid HTTP chunk length"));
        }
        let chunk = usize::from_str_radix(size_text, 16)
            .map_err(|_| ServerError::InvalidRequest("invalid HTTP chunk length"))?;
        let new_length = output
            .len()
            .checked_add(chunk)
            .ok_or(ServerError::ResourceLimit {
                resource: "HTTP request body bytes",
                limit: MAX_BODY_BYTES,
            })?;
        if new_length > MAX_BODY_BYTES {
            return Err(ServerError::ResourceLimit {
                resource: "HTTP request body bytes",
                limit: MAX_BODY_BYTES,
            });
        }
        if chunk == 0 {
            let mut trailer_bytes = 0usize;
            for _ in 0..=MAX_TRAILERS {
                let trailer = reader.read_line(MAX_CHUNK_LINE_BYTES, "read HTTP trailers")?;
                if trailer.is_empty() {
                    return Ok(output);
                }
                trailer_bytes = trailer_bytes.checked_add(trailer.len()).ok_or(
                    ServerError::ResourceLimit {
                        resource: "HTTP trailer bytes",
                        limit: MAX_TRAILER_BYTES,
                    },
                )?;
                if trailer_bytes > MAX_TRAILER_BYTES {
                    return Err(ServerError::ResourceLimit {
                        resource: "HTTP trailer bytes",
                        limit: MAX_TRAILER_BYTES,
                    });
                }
                let trailer = std::str::from_utf8(&trailer)
                    .map_err(|_| ServerError::InvalidRequest("trailer is not ASCII"))?;
                let (name, value) = trailer
                    .split_once(':')
                    .ok_or(ServerError::InvalidRequest("malformed HTTP trailer"))?;
                if !is_header_name(name) || !is_header_value(value.trim()) {
                    return Err(ServerError::InvalidRequest("invalid HTTP trailer"));
                }
            }
            return Err(ServerError::ResourceLimit {
                resource: "HTTP trailers",
                limit: MAX_TRAILERS,
            });
        }
        output
            .try_reserve(chunk)
            .map_err(|_| ServerError::ResourceLimit {
                resource: "HTTP request body bytes",
                limit: MAX_BODY_BYTES,
            })?;
        let old_length = output.len();
        output.resize(new_length, 0);
        reader.read_exact(&mut output[old_length..], "read HTTP request chunk")?;
        let mut terminator = [0u8; 2];
        reader.read_exact(&mut terminator, "read HTTP chunk terminator")?;
        if terminator != *b"\r\n" {
            return Err(ServerError::InvalidRequest(
                "HTTP chunk is missing CRLF terminator",
            ));
        }
    }
}

pub fn body(handle: i64) -> Result<Vec<u8>, ServerError> {
    let _operation = reserve_operation()?;
    let request = get_request(handle)?;
    ensure_request_open(&request, handle)?;
    let mut io = crate::native::lock_recover(&request.io);
    if io.body_consumed {
        return Err(ServerError::BodyConsumed);
    }
    io.body_consumed = true;
    let mut stream = io
        .stream
        .take()
        .ok_or(ServerError::ClosedRequest(handle))?;
    if request.expect_continue {
        if let Err(error) = write_all_deadline(
            &mut stream,
            b"HTTP/1.1 100 Continue\r\n\r\n",
            Instant::now() + IO_DEADLINE,
            "write HTTP 100 Continue response",
        ) {
            io.stream = Some(stream);
            return Err(error);
        }
    }
    let prefetched = std::mem::take(&mut io.prefetched);
    let mut prefix_offset = io.prefix_offset;
    let mut reader = BodyReader {
        stream: &mut stream,
        prefetched: &prefetched,
        prefix_offset: &mut prefix_offset,
        deadline: Instant::now() + IO_DEADLINE,
    };
    let result = match request.body_mode {
        BodyMode::Empty => Ok(Vec::new()),
        BodyMode::Fixed(length) => read_fixed_body(&mut reader, length),
        BodyMode::Chunked => read_chunked_body(&mut reader),
    };
    io.prefix_offset = prefix_offset;
    io.prefetched = prefetched;
    io.stream = Some(stream);
    result
}

pub fn body_text(handle: i64) -> Result<String, ServerError> {
    String::from_utf8(body(handle)?).map_err(|_| ServerError::Utf8)
}

fn validate_status(status: u16) -> Result<(), ServerError> {
    if !(100..=599).contains(&status) || status == 101 {
        return Err(ServerError::InvalidResponse(
            "status must be between 100 and 599; 101 is reserved for WebSocket upgrade",
        ));
    }
    Ok(())
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Content",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Status",
    }
}

fn validate_response_parts(
    status: u16,
    content_type: &str,
    extra_headers: &[(String, String)],
    body_length: usize,
) -> Result<(), ServerError> {
    validate_status(status)?;
    if body_length > MAX_RESPONSE_BYTES {
        return Err(ServerError::ResourceLimit {
            resource: "HTTP response body bytes",
            limit: MAX_RESPONSE_BYTES,
        });
    }
    if matches!(status, 100..=199 | 204 | 304) && body_length != 0 {
        return Err(ServerError::InvalidResponse(
            "this HTTP status cannot carry a response body",
        ));
    }
    validate_size(
        content_type,
        "HTTP Content-Type bytes",
        MAX_CONTENT_TYPE_BYTES,
    )?;
    if !is_header_value(content_type) {
        return Err(ServerError::InvalidResponse("invalid Content-Type"));
    }
    if extra_headers.len() > MAX_REQUEST_HEADERS {
        return Err(ServerError::ResourceLimit {
            resource: "HTTP response headers",
            limit: MAX_REQUEST_HEADERS,
        });
    }
    let mut total = content_type.len();
    for (name, value) in extra_headers {
        validate_size(name, "HTTP response header name bytes", MAX_HEADER_NAME_BYTES)?;
        validate_size(
            value,
            "HTTP response header value bytes",
            MAX_HEADER_VALUE_BYTES,
        )?;
        if !is_header_name(name) || !is_header_value(value) {
            return Err(ServerError::InvalidResponse("invalid HTTP response header"));
        }
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "content-length" | "transfer-encoding" | "connection" | "content-type"
        ) {
            return Err(ServerError::InvalidResponse(
                "framing and Content-Type headers are managed by TITAN",
            ));
        }
        total = total
            .checked_add(name.len())
            .and_then(|bytes| bytes.checked_add(value.len()))
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or(ServerError::ResourceLimit {
                resource: "HTTP response header bytes",
                limit: MAX_HEADER_OUTPUT_BYTES,
            })?;
        if total > MAX_HEADER_OUTPUT_BYTES {
            return Err(ServerError::ResourceLimit {
                resource: "HTTP response header bytes",
                limit: MAX_HEADER_OUTPUT_BYTES,
            });
        }
    }
    Ok(())
}

fn send_response(
    handle: i64,
    status: u16,
    content_type: &str,
    extra_headers: &[(String, String)],
    body: &[u8],
) -> Result<(), ServerError> {
    validate_response_parts(status, content_type, extra_headers, body.len())?;
    let request = take_request(handle)?;
    ensure_request_open(&request, handle)?;
    let mut io = crate::native::lock_recover(&request.io);
    let mut stream = io
        .stream
        .take()
        .ok_or(ServerError::ClosedRequest(handle))?;
    let payload_length = if matches!(status, 100..=199 | 204 | 304) {
        0
    } else {
        body.len()
    };
    let mut head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Length: {payload_length}\r\nConnection: close\r\n",
        reason_phrase(status)
    );
    if !content_type.is_empty() {
        head.push_str("Content-Type: ");
        head.push_str(content_type);
        head.push_str("\r\n");
    }
    for (name, value) in extra_headers {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    if head.len() > MAX_HEADER_OUTPUT_BYTES {
        return Err(ServerError::ResourceLimit {
            resource: "HTTP response header bytes",
            limit: MAX_HEADER_OUTPUT_BYTES,
        });
    }
    let deadline = Instant::now() + IO_DEADLINE;
    write_all_deadline(
        &mut stream,
        head.as_bytes(),
        deadline,
        "write HTTP response headers",
    )?;
    if request.method != "HEAD" && payload_length != 0 {
        write_all_deadline(&mut stream, body, deadline, "write HTTP response body")?;
    }
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

pub fn respond(handle: i64, status: u16, body: &str) -> Result<(), ServerError> {
    let _operation = reserve_operation()?;
    send_response(
        handle,
        status,
        "text/plain; charset=utf-8",
        &[],
        body.as_bytes(),
    )
}

pub fn respond_html(handle: i64, status: u16, html: &str) -> Result<(), ServerError> {
    let _operation = reserve_operation()?;
    send_response(
        handle,
        status,
        "text/html; charset=utf-8",
        &[],
        html.as_bytes(),
    )
}

pub fn respond_json(handle: i64, status: u16, body: &str) -> Result<(), ServerError> {
    let _operation = reserve_operation()?;
    send_response(
        handle,
        status,
        "application/json; charset=utf-8",
        &[],
        body.as_bytes(),
    )
}

pub fn respond_bytes(
    handle: i64,
    status: u16,
    content_type: &str,
    data: Vec<u8>,
) -> Result<(), ServerError> {
    let _operation = reserve_operation()?;
    send_response(handle, status, content_type, &[], &data)
}

pub fn respond_full(
    handle: i64,
    status: u16,
    content_type: &str,
    extra_headers: &[(String, String)],
    data: Vec<u8>,
) -> Result<(), ServerError> {
    let _operation = reserve_operation()?;
    send_response(handle, status, content_type, extra_headers, &data)
}

fn one_header<'a>(request: &'a RequestEntry, name: &str) -> Result<&'a str, ServerError> {
    let values = header_values(&request.headers, name);
    if values.len() != 1 {
        return Err(ServerError::Upgrade(
            "required WebSocket header must appear exactly once",
        ));
    }
    Ok(values[0])
}

fn contains_token(value: &str, expected: &str) -> bool {
    value
        .split(',')
        .any(|token| token.trim().eq_ignore_ascii_case(expected))
}

pub fn upgrade_websocket(handle: i64, max_message: usize) -> Result<i64, ServerError> {
    let _operation = reserve_operation()?;
    if max_message == 0 || max_message > MAX_WEBSOCKET_MESSAGE_BYTES {
        return Err(ServerError::ResourceLimit {
            resource: "server WebSocket message bytes",
            limit: MAX_WEBSOCKET_MESSAGE_BYTES,
        });
    }
    let request = get_request(handle)?;
    ensure_request_open(&request, handle)?;
    if request.method != "GET" || request.version != "HTTP/1.1" {
        return Err(ServerError::Upgrade(
            "WebSocket upgrade requires GET over HTTP/1.1",
        ));
    }
    if !matches!(request.body_mode, BodyMode::Empty) {
        return Err(ServerError::Upgrade(
            "WebSocket upgrade request must not contain a body",
        ));
    }
    if !one_header(&request, "Upgrade")?.eq_ignore_ascii_case("websocket")
        || !contains_token(one_header(&request, "Connection")?, "upgrade")
        || one_header(&request, "Sec-WebSocket-Version")? != "13"
    {
        return Err(ServerError::Upgrade("invalid WebSocket upgrade headers"));
    }
    let key = one_header(&request, "Sec-WebSocket-Key")?;
    validate_size(key, "Sec-WebSocket-Key bytes", 128)?;
    let accept = ws_codec::accept_key(key)
        .map_err(|_| ServerError::Upgrade("invalid Sec-WebSocket-Key"))?;
    let reservation = reserve_handle(HandleKind::WebSocket)?;
    let request = take_request(handle)?;
    let mut io = crate::native::lock_recover(&request.io);
    let mut stream = io
        .stream
        .take()
        .ok_or(ServerError::ClosedRequest(handle))?;
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    write_all_deadline(
        &mut stream,
        response.as_bytes(),
        Instant::now() + IO_DEADLINE,
        "write WebSocket upgrade response",
    )?;
    let shutdown = stream
        .try_clone()
        .map_err(|error| map_io(error, "clone WebSocket for cleanup"))?;
    let websocket = Arc::new(WsEntry {
        conn: Mutex::new(WsConn {
            stream,
            decoder: MessageDecoder::new(max_message),
            prefetched: std::mem::take(&mut io.prefetched),
        }),
        shutdown,
        closed: AtomicBool::new(false),
    });
    drop(io);
    reservation.commit(NewHandle::WebSocket(websocket))
}

fn ensure_websocket_open(websocket: &WsEntry, handle: i64) -> Result<(), ServerError> {
    if websocket.closed.load(Ordering::Acquire) {
        Err(ServerError::ClosedWebSocket(handle))
    } else {
        Ok(())
    }
}

fn receive_websocket(conn: &mut WsConn) -> Result<(String, String, Vec<u8>), ServerError> {
    if !conn.prefetched.is_empty() {
        conn.decoder.push(&conn.prefetched)?;
        conn.prefetched.clear();
    }
    let deadline = Instant::now() + IO_DEADLINE;
    loop {
        match conn.decoder.next(Some(true))? {
            Some(Message::Text(text)) => {
                let bytes = text.as_bytes().to_vec();
                return Ok(("text".into(), text, bytes));
            }
            Some(Message::Binary(bytes)) => return Ok(("binary".into(), String::new(), bytes)),
            Some(Message::Ping(payload)) => {
                let pong = ws_codec::encode_frame(true, WS_OP_PONG, &payload, None)?;
                write_all_deadline(
                    &mut conn.stream,
                    &pong,
                    deadline,
                    "write WebSocket pong",
                )?;
                return Ok(("ping".into(), String::new(), payload));
            }
            Some(Message::Pong(payload)) => {
                return Ok(("pong".into(), String::new(), payload));
            }
            Some(Message::Close { code, reason }) => {
                let mut payload = Vec::new();
                if let Some(code) = code {
                    payload.extend_from_slice(&code.to_be_bytes());
                }
                payload.extend_from_slice(reason.as_bytes());
                let close = ws_codec::encode_frame(true, WS_OP_CLOSE, &payload, None)?;
                let _ = write_all_deadline(
                    &mut conn.stream,
                    &close,
                    deadline,
                    "echo WebSocket close",
                );
                return Ok(("close".into(), reason, payload));
            }
            None => {
                let mut buffer = [0u8; 16 * 1024];
                let read = read_deadline(
                    &mut conn.stream,
                    &mut buffer,
                    deadline,
                    "read WebSocket message",
                )?;
                if read == 0 {
                    return Err(ServerError::Io(std::io::Error::new(
                        ErrorKind::UnexpectedEof,
                        "WebSocket peer closed",
                    )));
                }
                conn.decoder.push(&buffer[..read])?;
            }
        }
    }
}

pub fn ws_recv(handle: i64) -> Result<(String, String, Vec<u8>), ServerError> {
    let _operation = reserve_operation()?;
    let websocket = get_websocket(handle)?;
    ensure_websocket_open(&websocket, handle)?;
    let result = {
        let mut conn = websocket
            .conn
            .try_lock()
            .map_err(|_| ServerError::Busy {
                resource: "server WebSocket connection",
            })?;
        receive_websocket(&mut conn)
    };
    if result.is_err() {
        if let Some(websocket) = take_websocket(handle) {
            websocket.close();
        }
    }
    result
}

fn send_websocket(handle: i64, opcode: u8, payload: &[u8]) -> Result<(), ServerError> {
    if payload.len() > MAX_WEBSOCKET_MESSAGE_BYTES {
        return Err(ServerError::ResourceLimit {
            resource: "server WebSocket outbound message bytes",
            limit: MAX_WEBSOCKET_MESSAGE_BYTES,
        });
    }
    let websocket = get_websocket(handle)?;
    ensure_websocket_open(&websocket, handle)?;
    let frame = ws_codec::encode_frame(true, opcode, payload, None)?;
    let result = {
        let mut conn = websocket
            .conn
            .try_lock()
            .map_err(|_| ServerError::Busy {
                resource: "server WebSocket connection",
            })?;
        write_all_deadline(
            &mut conn.stream,
            &frame,
            Instant::now() + IO_DEADLINE,
            "write WebSocket message",
        )
    };
    if result.is_err() {
        if let Some(websocket) = take_websocket(handle) {
            websocket.close();
        }
    }
    result
}

pub fn ws_send_text(handle: i64, text: &str) -> Result<(), ServerError> {
    let _operation = reserve_operation()?;
    send_websocket(handle, WS_OP_TEXT, text.as_bytes())
}

pub fn ws_send_binary(handle: i64, data: &[u8]) -> Result<(), ServerError> {
    let _operation = reserve_operation()?;
    send_websocket(handle, WS_OP_BINARY, data)
}

fn valid_websocket_close_code(code: u16) -> bool {
    matches!(code, 1000..=1003 | 1007..=1014 | 3000..=4999)
}

pub fn ws_close(handle: i64, code: Option<u16>, reason: &str) -> Result<(), ServerError> {
    let _operation = reserve_operation()?;
    if code.is_some_and(|code| !valid_websocket_close_code(code)) {
        return Err(ServerError::InvalidArgument("invalid WebSocket close code"));
    }
    if code.is_none() && !reason.is_empty() {
        return Err(ServerError::InvalidArgument(
            "WebSocket close reason requires a close code",
        ));
    }
    if reason.len() > 123 {
        return Err(ServerError::ResourceLimit {
            resource: "WebSocket close reason bytes",
            limit: 123,
        });
    }
    let mut payload = Vec::with_capacity(reason.len() + usize::from(code.is_some()) * 2);
    if let Some(code) = code {
        payload.extend_from_slice(&code.to_be_bytes());
    }
    payload.extend_from_slice(reason.as_bytes());
    let frame = ws_codec::encode_frame(true, WS_OP_CLOSE, &payload, None)?;
    if let Some(websocket) = take_websocket(handle) {
        websocket.closed.store(true, Ordering::Release);
        if let Ok(mut conn) = websocket.conn.try_lock() {
            let _ = write_all_deadline(
                &mut conn.stream,
                &frame,
                Instant::now() + IO_DEADLINE,
                "write WebSocket close",
            );
        }
        websocket.close();
    }
    Ok(())
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let (servers, requests, websockets) = {
        let mut registry = crate::native::lock_recover(registry());
        registry
            .reserved
            .retain(|(owner, _), _| *owner != runtime_id);
        let servers = registry
            .servers
            .extract_if(|(owner, _), _| *owner == runtime_id)
            .map(|(_, server)| server)
            .collect::<Vec<_>>();
        let requests = registry
            .requests
            .extract_if(|(owner, _), _| *owner == runtime_id)
            .map(|(_, request)| request)
            .collect::<Vec<_>>();
        let websockets = registry
            .websockets
            .extract_if(|(owner, _), _| *owner == runtime_id)
            .map(|(_, websocket)| websocket)
            .collect::<Vec<_>>();
        (servers, requests, websockets)
    };
    let released = servers.len() + requests.len() + websockets.len();
    for server in servers {
        server.close();
    }
    for request in requests {
        request.close();
    }
    for websocket in websockets {
        websocket.close();
    }
    released
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_RUNTIME: AtomicU64 = AtomicU64::new(40_000);

    fn in_test_runtime<R>(test: impl FnOnce(u64) -> R) -> R {
        let runtime_id = NEXT_TEST_RUNTIME.fetch_add(1, Ordering::Relaxed);
        crate::native::with_runtime_context(runtime_id, || test(runtime_id))
    }

    fn client(addr: &str, request: Vec<u8>) -> thread::JoinHandle<Vec<u8>> {
        let addr = addr.to_string();
        thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            stream.write_all(&request).unwrap();
            let _ = stream.shutdown(Shutdown::Write);
            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            response
        })
    }

    #[test]
    fn real_http_round_trip_covers_metadata_body_and_response() {
        in_test_runtime(|runtime_id| {
            let server = start("127.0.0.1:0").unwrap();
            let addr = local_addr(server).unwrap();
            let peer = client(
                &addr,
                b"POST /hello?q=1 HTTP/1.1\r\nHost: localhost\r\nX-Test: yes\r\nContent-Length: 5\r\n\r\nhello"
                    .to_vec(),
            );
            let request = accept(server, 10_000).unwrap();
            assert_eq!(method(request).unwrap(), "POST");
            assert_eq!(url(request).unwrap(), "/hello?q=1");
            assert_eq!(path(request).unwrap(), "/hello");
            assert_eq!(query(request).unwrap(), "q=1");
            assert_eq!(header(request, "x-test").unwrap().as_deref(), Some("yes"));
            assert_eq!(headers(request).unwrap().get("host").unwrap(), "localhost");
            assert_eq!(body_text(request).unwrap(), "hello");
            assert!(matches!(body(request), Err(ServerError::BodyConsumed)));
            respond_full(
                request,
                201,
                "text/plain",
                &[("X-Reply".into(), "ok".into())],
                b"world".to_vec(),
            )
            .unwrap();
            let response = String::from_utf8(peer.join().unwrap()).unwrap();
            assert!(response.starts_with("HTTP/1.1 201 Created\r\n"));
            assert!(response.contains("Content-Length: 5\r\n"));
            assert!(response.contains("X-Reply: ok\r\n"));
            assert!(response.ends_with("\r\n\r\nworld"));
            stop(server);
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn chunked_request_body_is_decoded_with_trailers() {
        in_test_runtime(|runtime_id| {
            let server = start("127.0.0.1:0").unwrap();
            let addr = local_addr(server).unwrap();
            let peer = client(
                &addr,
                b"POST /chunked HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5\r\npedia\r\n0\r\nX-End: yes\r\n\r\n"
                    .to_vec(),
            );
            let request = accept(server, 10_000).unwrap();
            assert_eq!(body_text(request).unwrap(), "Wikipedia");
            respond(request, 200, "ok").unwrap();
            assert!(String::from_utf8(peer.join().unwrap())
                .unwrap()
                .ends_with("\r\n\r\nok"));
            stop(server);
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn framing_header_and_response_limits_reject_hostile_inputs() {
        in_test_runtime(|runtime_id| {
            let server = start("127.0.0.1:0").unwrap();
            let addr = local_addr(server).unwrap();
            let peer = client(
                &addr,
                format!(
                    "POST / HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n",
                    MAX_BODY_BYTES + 1
                )
                .into_bytes(),
            );
            assert!(matches!(
                accept(server, 10_000),
                Err(ServerError::ResourceLimit {
                    resource: "HTTP request body bytes",
                    ..
                })
            ));
            peer.join().unwrap();

            let peer = client(
                &addr,
                b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 1\r\nTransfer-Encoding: chunked\r\n\r\n"
                    .to_vec(),
            );
            assert!(matches!(
                accept(server, 10_000),
                Err(ServerError::InvalidRequest(
                    "ambiguous request body framing"
                ))
            ));
            peer.join().unwrap();

            let peer = client(&addr, b"GET / HTTP/1.1\r\nHost: x\r\n\r\n".to_vec());
            let request = accept(server, 10_000).unwrap();
            assert!(matches!(
                respond_full(
                    request,
                    200,
                    "text/plain",
                    &[("Content-Length".into(), "999".into())],
                    Vec::new()
                ),
                Err(ServerError::InvalidResponse(_))
            ));
            respond(request, 200, "safe").unwrap();
            peer.join().unwrap();
            stop(server);
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn stalled_request_body_hits_a_total_deadline_without_global_lock() {
        in_test_runtime(|runtime_id| {
            let server = start("127.0.0.1:0").unwrap();
            let addr = local_addr(server).unwrap();
            let stalled = thread::spawn({
                let addr = addr.clone();
                move || {
                    let mut stream = TcpStream::connect(addr).unwrap();
                    stream
                        .write_all(
                            b"POST /slow HTTP/1.1\r\nHost: x\r\nContent-Length: 10\r\n\r\nx",
                        )
                        .unwrap();
                    // Outlive the server deadline so the read fails as a
                    // timeout rather than as an early end of stream.
                    thread::sleep(IO_DEADLINE + Duration::from_millis(500));
                }
            });
            let request = accept(server, 10_000).unwrap();
            let started = Instant::now();
            let body_reader = thread::spawn(move || {
                crate::native::with_runtime_context(runtime_id, || body(request))
            });
            thread::sleep(Duration::from_millis(50));
            let other_server = start("127.0.0.1:0").unwrap();
            assert!(local_addr(other_server).is_ok());
            assert!(matches!(
                body_reader.join().unwrap(),
                Err(ServerError::Timeout {
                    operation: "read HTTP request body"
                })
            ));
            assert!(started.elapsed() < IO_DEADLINE + Duration::from_millis(750));
            stop(other_server);
            stop(server);
            stalled.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 1);
        });
    }

    // Real-network round-trip integration test, kept on Linux only.
    // The original reason was the 300ms cfg(test) IO_DEADLINE, which is now
    // 2s, so the margin that made this red on macOS/Windows is gone. The gate
    // stays until a green macOS/Windows run confirms it; re-enabling it is a
    // deliberate follow-up, not something to assume.
    #[cfg(target_os = "linux")]
    #[test]
    fn real_websocket_upgrade_and_masked_frame_round_trip() {
        in_test_runtime(|runtime_id| {
            let server = start("127.0.0.1:0").unwrap();
            let addr = local_addr(server).unwrap();
            let peer = thread::spawn(move || {
                let mut stream = TcpStream::connect(addr).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                stream
                    .write_all(
                        b"GET /chat HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: keep-alive, Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
                    )
                    .unwrap();
                let mut handshake = Vec::new();
                let mut byte = [0u8; 1];
                while !handshake.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    handshake.push(byte[0]);
                }
                assert!(String::from_utf8(handshake)
                    .unwrap()
                    .starts_with("HTTP/1.1 101 Switching Protocols"));
                let frame = ws_codec::encode_frame(true, WS_OP_TEXT, b"hello", Some([1, 2, 3, 4]))
                    .unwrap();
                stream.write_all(&frame).unwrap();
                let mut response = [0u8; 7];
                stream.read_exact(&mut response).unwrap();
                let frame = ws_codec::parse_frame(&response, Some(false), 1024)
                    .unwrap()
                    .unwrap();
                assert_eq!(frame.payload, b"world");
            });
            let request = accept(server, 10_000).unwrap();
            let websocket = upgrade_websocket(request, 1024).unwrap();
            assert_eq!(
                ws_recv(websocket).unwrap(),
                ("text".into(), "hello".into(), b"hello".to_vec())
            );
            ws_send_text(websocket, "world").unwrap();
            ws_close(websocket, Some(1000), "done").unwrap();
            peer.join().unwrap();
            stop(server);
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn handle_operation_and_message_quotas_recover() {
        in_test_runtime(|runtime_id| {
            let reservations = (0..MAX_SERVERS_PER_RUNTIME)
                .map(|_| reserve_handle(HandleKind::Server).unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                reserve_handle(HandleKind::Server),
                Err(ServerError::ResourceLimit {
                    resource: "HTTP server handles",
                    ..
                })
            ));
            drop(reservations);

            let permits = (0..MAX_CONCURRENT_OPERATIONS)
                .map(|_| reserve_operation().unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                reserve_operation(),
                Err(ServerError::ResourceLimit {
                    resource: "concurrent server operations",
                    ..
                })
            ));
            drop(permits);

            assert!(matches!(
                upgrade_websocket(123, MAX_WEBSOCKET_MESSAGE_BYTES + 1),
                Err(ServerError::ResourceLimit {
                    resource: "server WebSocket message bytes",
                    ..
                })
            ));

            let mut large = dummy_request();
            large.metadata_bytes = MAX_REQUEST_METADATA_PER_RUNTIME;
            reserve_handle(HandleKind::Request)
                .unwrap()
                .commit(NewHandle::Request(Arc::new(large)))
                .unwrap();
            let mut extra = dummy_request();
            extra.metadata_bytes = 1;
            assert!(matches!(
                reserve_handle(HandleKind::Request)
                    .unwrap()
                    .commit(NewHandle::Request(Arc::new(extra))),
                Err(ServerError::ResourceLimit {
                    resource: "HTTP request metadata bytes per runtime",
                    ..
                })
            ));
            assert_eq!(cleanup_runtime(runtime_id), 1);
            assert!(!crate::native::lock_recover(operation_usage()).contains_key(&runtime_id));
        });
    }

    #[test]
    fn runtime_ownership_cleanup_and_inflight_reservations_are_safe() {
        in_test_runtime(|runtime_id| {
            let server = start("127.0.0.1:0").unwrap();
            let reservation = reserve_handle(HandleKind::Request).unwrap();
            let other_runtime = NEXT_TEST_RUNTIME.fetch_add(1, Ordering::Relaxed);
            crate::native::with_runtime_context(other_runtime, || {
                assert!(matches!(
                    local_addr(server),
                    Err(ServerError::UnknownServer(_))
                ));
                assert_eq!(cleanup_runtime(other_runtime), 0);
            });
            assert_eq!(cleanup_runtime(runtime_id), 1);

            let late = reservation.commit(NewHandle::Request(Arc::new(dummy_request())));
            assert!(matches!(late, Err(ServerError::RuntimeClosed)));
        });
    }

    fn dummy_request() -> RequestEntry {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).unwrap();
        let (server, remote) = listener.accept().unwrap();
        drop(client);
        parse_request_for_dummy(server, remote.to_string())
    }

    fn parse_request_for_dummy(stream: TcpStream, remote_addr: String) -> RequestEntry {
        let shutdown = stream.try_clone().unwrap();
        RequestEntry {
            method: "GET".into(),
            target: "/".into(),
            version: "HTTP/1.1".into(),
            headers: Vec::new(),
            remote_addr,
            body_mode: BodyMode::Empty,
            expect_continue: false,
            metadata_bytes: 64,
            io: Mutex::new(RequestIo {
                stream: Some(stream),
                prefetched: Vec::new(),
                prefix_offset: 0,
                body_consumed: false,
            }),
            shutdown,
            closed: AtomicBool::new(false),
        }
    }

    #[test]
    fn unknown_handles_and_numeric_address_validation_are_typed() {
        in_test_runtime(|runtime_id| {
            assert!(matches!(
                start("localhost:0"),
                Err(ServerError::InvalidArgument(_))
            ));
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
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }
}

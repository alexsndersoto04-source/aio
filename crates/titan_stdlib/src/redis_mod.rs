//! Bounded blocking Redis client (`std::redis::*`) over RESP2/TCP.
//!
//! URLs are parsed by the `redis` crate, but replies are decoded here so an
//! untrusted server cannot make that crate materialise an unbounded value
//! before TITAN gets a chance to inspect it. Every socket has hard deadlines,
//! every request and reply is bounded, and the process-wide registry is held
//! only for short handle lookups (never while network I/O is in progress).

use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{mpsc, Arc, Mutex, OnceLock, TryLockError};
use std::thread;
use std::time::{Duration, Instant};

use redis::{Client, ConnectionAddr, ProtocolVersion};
use thiserror::Error;

const MAX_CONNECTIONS_PER_RUNTIME: usize = 8;
const MAX_CONCURRENT_OPERATIONS: usize = 4;
const MAX_DNS_RESOLVERS_PER_RUNTIME: usize = 2;
const MAX_DNS_RESOLVERS_GLOBAL: usize = 16;
const MAX_RESOLVED_ADDRESSES: usize = 32;
const MAX_URL_BYTES: usize = 16 * 1024;
const MAX_KEY_BYTES: usize = 64 * 1024;
const MAX_VALUE_BYTES: usize = 8 * 1024 * 1024;
const MAX_RAW_COMMAND_BYTES: usize = 64 * 1024;
const MAX_COMMAND_ARGUMENTS: usize = 32;
const MAX_REQUEST_BYTES: usize = 9 * 1024 * 1024;
const MAX_RESPONSE_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESPONSE_WIRE_BYTES: usize = 9 * 1024 * 1024;
const MAX_RESPONSE_ELEMENTS: usize = 65_536;
const MAX_RESPONSE_DEPTH: usize = 16;
const MAX_RESPONSE_LINE_BYTES: usize = 64 * 1024;
const MAX_COLLECTION_ITEMS: usize = 65_536;
const MAX_COLLECTION_BYTES: usize = 8 * 1024 * 1024;
const MAX_SCAN_ROUNDS: usize = 256;

#[cfg(not(test))]
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
#[cfg(test)]
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(not(test))]
const OPERATION_TIMEOUT: Duration = Duration::from_secs(5);
// Same reasoning as `server_mod::IO_DEADLINE`: the test timeouts were 500ms,
// which is comfortable on Linux but marginal on hosted macOS/Windows runners
// under parallel load, where a healthy local round-trip occasionally exceeded
// it and failed as `RedisError::Timeout`. Two seconds keeps the timeout tests
// fast while removing the false negatives; tests that wait for the deadline
// derive their sleeps from these constants.
#[cfg(test)]
const OPERATION_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum RedisError {
    #[error("invalid Redis URL or configuration: {0}")]
    Configuration(#[from] redis::RedisError),
    #[error("Redis I/O error: {0}")]
    Io(#[source] std::io::Error),
    #[error("Redis operation timed out")]
    Timeout,
    #[error("Redis server error: {0}")]
    Server(String),
    #[error("invalid RESP2 reply: {0}")]
    Protocol(&'static str),
    #[error("unexpected Redis reply: {0}")]
    UnexpectedResponse(&'static str),
    #[error("unknown connection handle {0}")]
    UnknownHandle(i64),
    #[error("Redis connection handle {0} is busy")]
    ConnectionBusy(i64),
    #[error("invalid Redis argument: {0}")]
    InvalidArgument(&'static str),
    #[error("{resource} exceeds limit {limit}")]
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    #[error("Redis handle space exhausted")]
    HandleSpaceExhausted,
    #[error("Redis runtime ownership ended while a connection was opening")]
    RuntimeClosed,
}

#[derive(Debug)]
enum RespValue {
    Simple(Vec<u8>),
    Bulk(Vec<u8>),
    Integer(i64),
    Array(Vec<RespValue>),
    Null,
    Error(Vec<u8>),
}

struct Connection {
    socket: BufReader<TcpStream>,
    broken: bool,
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.socket.get_ref().shutdown(Shutdown::Both);
    }
}

type SharedConnection = Arc<Mutex<Connection>>;

struct Registry {
    conns: HashMap<(u64, i64), SharedConnection>,
    reserved: HashMap<u64, usize>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            conns: HashMap::new(),
            reserved: HashMap::new(),
            next_id: 1,
        })
    })
}

#[derive(Default)]
struct RuntimeUsage {
    active_operations: usize,
}

fn runtime_usage() -> &'static Mutex<HashMap<u64, RuntimeUsage>> {
    static USAGE: OnceLock<Mutex<HashMap<u64, RuntimeUsage>>> = OnceLock::new();
    USAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

struct OperationPermit {
    runtime_id: u64,
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        let mut usage = crate::native::lock_recover(runtime_usage());
        if let Some(runtime) = usage.get_mut(&self.runtime_id) {
            runtime.active_operations = runtime.active_operations.saturating_sub(1);
            if runtime.active_operations == 0 {
                usage.remove(&self.runtime_id);
            }
        }
    }
}

fn reserve_operation() -> Result<OperationPermit, RedisError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut usage = crate::native::lock_recover(runtime_usage());
    let runtime = usage.entry(runtime_id).or_default();
    if runtime.active_operations >= MAX_CONCURRENT_OPERATIONS {
        return Err(RedisError::ResourceLimit {
            resource: "concurrent Redis operations",
            limit: MAX_CONCURRENT_OPERATIONS,
        });
    }
    runtime.active_operations += 1;
    Ok(OperationPermit { runtime_id })
}

#[derive(Default)]
struct ResolverUsage {
    active_global: usize,
    active_by_runtime: HashMap<u64, usize>,
}

fn resolver_usage() -> &'static Mutex<ResolverUsage> {
    static USAGE: OnceLock<Mutex<ResolverUsage>> = OnceLock::new();
    USAGE.get_or_init(|| Mutex::new(ResolverUsage::default()))
}

struct ResolverPermit {
    runtime_id: u64,
}

impl Drop for ResolverPermit {
    fn drop(&mut self) {
        let mut usage = crate::native::lock_recover(resolver_usage());
        usage.active_global = usage.active_global.saturating_sub(1);
        if let Some(active) = usage.active_by_runtime.get_mut(&self.runtime_id) {
            *active = active.saturating_sub(1);
            if *active == 0 {
                usage.active_by_runtime.remove(&self.runtime_id);
            }
        }
    }
}

fn reserve_resolver(runtime_id: u64) -> Result<ResolverPermit, RedisError> {
    let mut usage = crate::native::lock_recover(resolver_usage());
    let active_for_runtime = usage
        .active_by_runtime
        .get(&runtime_id)
        .copied()
        .unwrap_or(0);
    if active_for_runtime >= MAX_DNS_RESOLVERS_PER_RUNTIME {
        return Err(RedisError::ResourceLimit {
            resource: "concurrent Redis DNS resolvers per runtime",
            limit: MAX_DNS_RESOLVERS_PER_RUNTIME,
        });
    }
    if usage.active_global >= MAX_DNS_RESOLVERS_GLOBAL {
        return Err(RedisError::ResourceLimit {
            resource: "concurrent Redis DNS resolvers",
            limit: MAX_DNS_RESOLVERS_GLOBAL,
        });
    }
    usage.active_global += 1;
    *usage.active_by_runtime.entry(runtime_id).or_default() += 1;
    Ok(ResolverPermit { runtime_id })
}

struct HandleReservation {
    runtime_id: u64,
    committed: bool,
}

fn active_handles(registry: &Registry, runtime_id: u64) -> usize {
    registry
        .conns
        .keys()
        .filter(|(owner, _)| *owner == runtime_id)
        .count()
}

fn release_reservation(registry: &mut Registry, runtime_id: u64) {
    if let Some(count) = registry.reserved.get_mut(&runtime_id) {
        *count = count.saturating_sub(1);
        if *count == 0 {
            registry.reserved.remove(&runtime_id);
        }
    }
}

fn reserve_handle() -> Result<HandleReservation, RedisError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(registry());
    let active = active_handles(&registry, runtime_id);
    let reserved = registry.reserved.get(&runtime_id).copied().unwrap_or(0);
    if active.saturating_add(reserved) >= MAX_CONNECTIONS_PER_RUNTIME {
        return Err(RedisError::ResourceLimit {
            resource: "Redis connection handles",
            limit: MAX_CONNECTIONS_PER_RUNTIME,
        });
    }
    *registry.reserved.entry(runtime_id).or_default() += 1;
    Ok(HandleReservation {
        runtime_id,
        committed: false,
    })
}

impl HandleReservation {
    fn commit(mut self, connection: Connection) -> Result<i64, RedisError> {
        let mut registry = crate::native::lock_recover(registry());
        if registry
            .reserved
            .get(&self.runtime_id)
            .copied()
            .unwrap_or(0)
            == 0
        {
            return Err(RedisError::RuntimeClosed);
        }
        let id = registry.next_id;
        registry.next_id = id.checked_add(1).ok_or(RedisError::HandleSpaceExhausted)?;
        release_reservation(&mut registry, self.runtime_id);
        registry
            .conns
            .insert((self.runtime_id, id), Arc::new(Mutex::new(connection)));
        self.committed = true;
        Ok(id)
    }
}

impl Drop for HandleReservation {
    fn drop(&mut self) {
        if !self.committed {
            let mut registry = crate::native::lock_recover(registry());
            release_reservation(&mut registry, self.runtime_id);
        }
    }
}

fn handle_key(handle: i64) -> (u64, i64) {
    crate::native::runtime_handle_key(handle)
}

fn remove_if_same(key: (u64, i64), connection: &SharedConnection) {
    let mut registry = crate::native::lock_recover(registry());
    if registry
        .conns
        .get(&key)
        .is_some_and(|registered| Arc::ptr_eq(registered, connection))
    {
        registry.conns.remove(&key);
    }
}

fn with_conn<F, R>(handle: i64, action: F) -> Result<R, RedisError>
where
    F: FnOnce(&mut Connection, Instant) -> Result<R, RedisError>,
{
    let _permit = reserve_operation()?;
    let key = handle_key(handle);
    let connection = {
        let registry = crate::native::lock_recover(registry());
        Arc::clone(
            registry
                .conns
                .get(&key)
                .ok_or(RedisError::UnknownHandle(handle))?,
        )
    };
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    let mut connection_guard = match connection.try_lock() {
        Ok(guard) => guard,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return Err(RedisError::ConnectionBusy(handle)),
    };
    let result = action(&mut connection_guard, deadline);
    let broken = connection_guard.broken;
    drop(connection_guard);
    if broken {
        remove_if_same(key, &connection);
    }
    result
}

fn remaining(deadline: Instant, ceiling: Duration) -> Result<Duration, RedisError> {
    let left = deadline
        .checked_duration_since(Instant::now())
        .ok_or(RedisError::Timeout)?;
    if left.is_zero() {
        return Err(RedisError::Timeout);
    }
    Ok(left.min(ceiling))
}

fn map_io(error: std::io::Error) -> RedisError {
    match error.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => RedisError::Timeout,
        _ => RedisError::Io(error),
    }
}

fn validate_size(value: &str, resource: &'static str, limit: usize) -> Result<(), RedisError> {
    if value.len() > limit {
        return Err(RedisError::ResourceLimit { resource, limit });
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), RedisError> {
    validate_size(key, "Redis key bytes", MAX_KEY_BYTES)
}

fn validate_value(value: &str) -> Result<(), RedisError> {
    validate_size(value, "Redis value bytes", MAX_VALUE_BYTES)
}

fn decimal_len(value: usize) -> usize {
    value.to_string().len()
}

fn request_size(args: &[&[u8]]) -> Result<usize, RedisError> {
    if args.is_empty() {
        return Err(RedisError::InvalidArgument("Redis command cannot be empty"));
    }
    if args.len() > MAX_COMMAND_ARGUMENTS {
        return Err(RedisError::ResourceLimit {
            resource: "Redis command arguments",
            limit: MAX_COMMAND_ARGUMENTS,
        });
    }
    let mut bytes = 1usize
        .checked_add(decimal_len(args.len()))
        .and_then(|value| value.checked_add(2))
        .ok_or(RedisError::ResourceLimit {
            resource: "Redis request bytes",
            limit: MAX_REQUEST_BYTES,
        })?;
    for argument in args {
        bytes = bytes
            .checked_add(1)
            .and_then(|value| value.checked_add(decimal_len(argument.len())))
            .and_then(|value| value.checked_add(2))
            .and_then(|value| value.checked_add(argument.len()))
            .and_then(|value| value.checked_add(2))
            .ok_or(RedisError::ResourceLimit {
                resource: "Redis request bytes",
                limit: MAX_REQUEST_BYTES,
            })?;
    }
    if bytes > MAX_REQUEST_BYTES {
        return Err(RedisError::ResourceLimit {
            resource: "Redis request bytes",
            limit: MAX_REQUEST_BYTES,
        });
    }
    Ok(bytes)
}

impl Connection {
    fn new(stream: TcpStream) -> Result<Self, RedisError> {
        stream.set_nodelay(true).map_err(map_io)?;
        Ok(Self {
            socket: BufReader::new(stream),
            broken: false,
        })
    }

    fn write_bytes(&mut self, mut bytes: &[u8], deadline: Instant) -> Result<(), RedisError> {
        while !bytes.is_empty() {
            let timeout = remaining(deadline, OPERATION_TIMEOUT)?;
            self.socket
                .get_ref()
                .set_write_timeout(Some(timeout))
                .map_err(map_io)?;
            match self.socket.get_mut().write(bytes) {
                Ok(0) => {
                    return Err(map_io(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "failed to write Redis request",
                    )));
                }
                Ok(written) => bytes = &bytes[written..],
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(map_io(error)),
            }
        }
        Ok(())
    }

    fn write_request(&mut self, args: &[&[u8]], deadline: Instant) -> Result<(), RedisError> {
        let _ = request_size(args)?;
        self.write_bytes(format!("*{}\r\n", args.len()).as_bytes(), deadline)?;
        for argument in args {
            self.write_bytes(format!("${}\r\n", argument.len()).as_bytes(), deadline)?;
            self.write_bytes(argument, deadline)?;
            self.write_bytes(b"\r\n", deadline)?;
        }
        Ok(())
    }

    fn execute(&mut self, args: &[&[u8]], deadline: Instant) -> Result<RespValue, RedisError> {
        if self.broken {
            return Err(RedisError::Protocol("connection is already broken"));
        }
        if !self.socket.buffer().is_empty() {
            self.broken = true;
            return Err(RedisError::Protocol(
                "unsolicited bytes were buffered before a command",
            ));
        }

        // Validate the complete request before the first byte is sent. A local
        // limit error therefore leaves the connection synchronised.
        request_size(args)?;
        if let Err(error) = self.write_request(args, deadline) {
            self.broken = true;
            return Err(error);
        }

        let value = match RespDecoder::new(&mut self.socket, deadline).decode() {
            Ok(value) => value,
            Err(error) => {
                self.broken = true;
                return Err(error);
            }
        };
        if !self.socket.buffer().is_empty() {
            self.broken = true;
            return Err(RedisError::Protocol(
                "server sent bytes beyond one RESP2 reply",
            ));
        }
        match value {
            RespValue::Error(message) => Err(RedisError::Server(
                String::from_utf8_lossy(&message).into_owned(),
            )),
            value => Ok(value),
        }
    }
}

struct RespDecoder<'a> {
    reader: &'a mut BufReader<TcpStream>,
    deadline: Instant,
    wire_bytes: usize,
    payload_bytes: usize,
    elements: usize,
}

impl<'a> RespDecoder<'a> {
    fn new(reader: &'a mut BufReader<TcpStream>, deadline: Instant) -> Self {
        Self {
            reader,
            deadline,
            wire_bytes: 0,
            payload_bytes: 0,
            elements: 0,
        }
    }

    fn decode(mut self) -> Result<RespValue, RedisError> {
        self.decode_value(0)
    }

    fn add_wire(&mut self, amount: usize) -> Result<(), RedisError> {
        self.wire_bytes = self
            .wire_bytes
            .checked_add(amount)
            .ok_or(RedisError::ResourceLimit {
                resource: "Redis response wire bytes",
                limit: MAX_RESPONSE_WIRE_BYTES,
            })?;
        if self.wire_bytes > MAX_RESPONSE_WIRE_BYTES {
            return Err(RedisError::ResourceLimit {
                resource: "Redis response wire bytes",
                limit: MAX_RESPONSE_WIRE_BYTES,
            });
        }
        Ok(())
    }

    fn add_payload(&mut self, amount: usize) -> Result<(), RedisError> {
        self.payload_bytes =
            self.payload_bytes
                .checked_add(amount)
                .ok_or(RedisError::ResourceLimit {
                    resource: "Redis response payload bytes",
                    limit: MAX_RESPONSE_PAYLOAD_BYTES,
                })?;
        if self.payload_bytes > MAX_RESPONSE_PAYLOAD_BYTES {
            return Err(RedisError::ResourceLimit {
                resource: "Redis response payload bytes",
                limit: MAX_RESPONSE_PAYLOAD_BYTES,
            });
        }
        Ok(())
    }

    fn add_element(&mut self) -> Result<(), RedisError> {
        self.elements = self
            .elements
            .checked_add(1)
            .ok_or(RedisError::ResourceLimit {
                resource: "Redis response elements",
                limit: MAX_RESPONSE_ELEMENTS,
            })?;
        if self.elements > MAX_RESPONSE_ELEMENTS {
            return Err(RedisError::ResourceLimit {
                resource: "Redis response elements",
                limit: MAX_RESPONSE_ELEMENTS,
            });
        }
        Ok(())
    }

    fn prepare_read(&self) -> Result<(), RedisError> {
        let timeout = remaining(self.deadline, OPERATION_TIMEOUT)?;
        self.reader
            .get_ref()
            .set_read_timeout(Some(timeout))
            .map_err(map_io)
    }

    fn read_byte(&mut self) -> Result<u8, RedisError> {
        let mut byte = [0u8; 1];
        loop {
            self.prepare_read()?;
            match self.reader.read(&mut byte) {
                Ok(0) => {
                    return Err(RedisError::Protocol(
                        "connection closed in the middle of a reply",
                    ));
                }
                Ok(_) => {
                    self.add_wire(1)?;
                    return Ok(byte[0]);
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(map_io(error)),
            }
        }
    }

    fn read_line(&mut self) -> Result<Vec<u8>, RedisError> {
        let mut line = Vec::new();
        loop {
            let byte = self.read_byte()?;
            if byte == b'\r' {
                if self.read_byte()? != b'\n' {
                    return Err(RedisError::Protocol("RESP line did not end with CRLF"));
                }
                return Ok(line);
            }
            if line.len() >= MAX_RESPONSE_LINE_BYTES {
                return Err(RedisError::ResourceLimit {
                    resource: "Redis response line bytes",
                    limit: MAX_RESPONSE_LINE_BYTES,
                });
            }
            line.push(byte);
        }
    }

    fn read_exact_bounded(&mut self, length: usize) -> Result<Vec<u8>, RedisError> {
        self.add_payload(length)?;
        let remaining_wire = MAX_RESPONSE_WIRE_BYTES.saturating_sub(self.wire_bytes);
        if length.saturating_add(2) > remaining_wire {
            return Err(RedisError::ResourceLimit {
                resource: "Redis response wire bytes",
                limit: MAX_RESPONSE_WIRE_BYTES,
            });
        }
        let mut value = vec![0u8; length];
        let mut offset = 0;
        while offset < value.len() {
            self.prepare_read()?;
            match self.reader.read(&mut value[offset..]) {
                Ok(0) => {
                    return Err(RedisError::Protocol(
                        "connection closed in the middle of a bulk reply",
                    ));
                }
                Ok(read) => {
                    self.add_wire(read)?;
                    offset += read;
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(map_io(error)),
            }
        }
        if self.read_byte()? != b'\r' || self.read_byte()? != b'\n' {
            return Err(RedisError::Protocol("bulk response did not end with CRLF"));
        }
        Ok(value)
    }

    fn parse_i64(line: &[u8]) -> Result<i64, RedisError> {
        let text = std::str::from_utf8(line)
            .map_err(|_| RedisError::Protocol("RESP integer was not ASCII"))?;
        text.parse::<i64>()
            .map_err(|_| RedisError::Protocol("invalid RESP integer"))
    }

    fn parse_length(line: &[u8]) -> Result<Option<usize>, RedisError> {
        let value = Self::parse_i64(line)?;
        if value == -1 {
            return Ok(None);
        }
        if value < -1 {
            return Err(RedisError::Protocol("invalid negative RESP length"));
        }
        usize::try_from(value)
            .map(Some)
            .map_err(|_| RedisError::Protocol("RESP length is out of range"))
    }

    fn decode_value(&mut self, depth: usize) -> Result<RespValue, RedisError> {
        if depth > MAX_RESPONSE_DEPTH {
            return Err(RedisError::ResourceLimit {
                resource: "Redis response nesting depth",
                limit: MAX_RESPONSE_DEPTH,
            });
        }
        self.add_element()?;
        let prefix = self.read_byte()?;
        match prefix {
            b'+' => {
                let line = self.read_line()?;
                self.add_payload(line.len())?;
                Ok(RespValue::Simple(line))
            }
            b'-' => {
                let line = self.read_line()?;
                self.add_payload(line.len())?;
                Ok(RespValue::Error(line))
            }
            b':' => {
                let line = self.read_line()?;
                Ok(RespValue::Integer(Self::parse_i64(&line)?))
            }
            b'$' => {
                let line = self.read_line()?;
                match Self::parse_length(&line)? {
                    Some(length) => Ok(RespValue::Bulk(self.read_exact_bounded(length)?)),
                    None => Ok(RespValue::Null),
                }
            }
            b'*' => {
                let line = self.read_line()?;
                let Some(length) = Self::parse_length(&line)? else {
                    return Ok(RespValue::Null);
                };
                if length > MAX_RESPONSE_ELEMENTS
                    || self.elements.saturating_add(length) > MAX_RESPONSE_ELEMENTS
                {
                    return Err(RedisError::ResourceLimit {
                        resource: "Redis response elements",
                        limit: MAX_RESPONSE_ELEMENTS,
                    });
                }
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    values.push(self.decode_value(depth + 1)?);
                }
                Ok(RespValue::Array(values))
            }
            _ => Err(RedisError::Protocol("unsupported RESP2 type prefix")),
        }
    }
}

fn resolve_addresses(
    host: &str,
    port: u16,
    deadline: Instant,
) -> Result<Vec<SocketAddr>, RedisError> {
    if let Ok(address) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(address, port)]);
    }

    // `ToSocketAddrs` has no timeout API. Resolve on a separately quota-bound
    // worker so the VM still observes its deadline. If the platform resolver
    // itself wedges, at most two workers per runtime and sixteen process-wide
    // can remain until the operating system releases them.
    let runtime_id = crate::native::current_runtime_id();
    let permit = reserve_resolver(runtime_id)?;
    let host = host.to_owned();
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("titan-redis-dns".into())
        .spawn(move || {
            let result = (host.as_str(), port)
                .to_socket_addrs()
                .map_err(map_io)
                .and_then(|addresses| {
                    let addresses = addresses
                        .take(MAX_RESOLVED_ADDRESSES + 1)
                        .collect::<Vec<_>>();
                    if addresses.len() > MAX_RESOLVED_ADDRESSES {
                        return Err(RedisError::ResourceLimit {
                            resource: "resolved Redis addresses",
                            limit: MAX_RESOLVED_ADDRESSES,
                        });
                    }
                    if addresses.is_empty() {
                        return Err(RedisError::InvalidArgument(
                            "Redis host resolved to no addresses",
                        ));
                    }
                    Ok(addresses)
                });
            drop(permit);
            let _ = sender.send(result);
        })
        .map_err(map_io)?;

    let wait = remaining(deadline, CONNECT_TIMEOUT)?;
    match receiver.recv_timeout(wait) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(RedisError::Timeout),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(RedisError::Io(std::io::Error::other(
            "Redis DNS resolver stopped without a result",
        ))),
    }
}

fn connect_tcp(host: &str, port: u16, deadline: Instant) -> Result<TcpStream, RedisError> {
    let addresses = resolve_addresses(host, port, deadline)?;
    let mut last_error = None;
    for address in addresses {
        let timeout = remaining(deadline, CONNECT_TIMEOUT)?;
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }
    Err(map_io(last_error.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "no Redis address connected")
    })))
}

fn command<'a>(name: &'a str, arguments: &'a [&'a str]) -> Vec<&'a [u8]> {
    let mut command = Vec::with_capacity(arguments.len() + 1);
    command.push(name.as_bytes());
    command.extend(arguments.iter().map(|argument| argument.as_bytes()));
    command
}

fn execute_text(
    connection: &mut Connection,
    deadline: Instant,
    name: &str,
    arguments: &[&str],
) -> Result<RespValue, RedisError> {
    let command = command(name, arguments);
    connection.execute(&command, deadline)
}

fn into_bytes(value: RespValue) -> Result<Vec<u8>, RedisError> {
    match value {
        RespValue::Simple(value) | RespValue::Bulk(value) => Ok(value),
        _ => Err(RedisError::UnexpectedResponse("expected a string")),
    }
}

fn into_string(value: RespValue) -> Result<String, RedisError> {
    String::from_utf8(into_bytes(value)?)
        .map_err(|_| RedisError::UnexpectedResponse("response string was not UTF-8"))
}

fn into_optional_string(value: RespValue) -> Result<Option<String>, RedisError> {
    match value {
        RespValue::Null => Ok(None),
        value => into_string(value).map(Some),
    }
}

fn into_integer(value: RespValue) -> Result<i64, RedisError> {
    match value {
        RespValue::Integer(value) => Ok(value),
        _ => Err(RedisError::UnexpectedResponse("expected an integer")),
    }
}

fn into_u64(value: RespValue) -> Result<u64, RedisError> {
    u64::try_from(into_integer(value)?)
        .map_err(|_| RedisError::UnexpectedResponse("expected a nonnegative integer"))
}

fn into_bool(value: RespValue) -> Result<bool, RedisError> {
    match into_integer(value)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(RedisError::UnexpectedResponse(
            "expected integer zero or one",
        )),
    }
}

fn into_ok(value: RespValue) -> Result<(), RedisError> {
    if into_bytes(value)? == b"OK" {
        Ok(())
    } else {
        Err(RedisError::UnexpectedResponse("expected OK"))
    }
}

fn into_array(value: RespValue) -> Result<Vec<RespValue>, RedisError> {
    match value {
        RespValue::Array(values) => Ok(values),
        _ => Err(RedisError::UnexpectedResponse("expected an array")),
    }
}

fn into_string_array(value: RespValue) -> Result<Vec<String>, RedisError> {
    into_array(value)?.into_iter().map(into_string).collect()
}

/// Open a bounded blocking RESP2 connection to a plain `redis://` URL.
/// Authentication and database selection from the URL are performed before
/// the opaque handle is published.
pub fn connect(url: &str) -> Result<i64, RedisError> {
    validate_size(url, "Redis URL bytes", MAX_URL_BYTES)?;
    let reservation = reserve_handle()?;
    let _permit = reserve_operation()?;
    let client = Client::open(url)?;
    let info = client.get_connection_info();
    if info.redis.protocol != ProtocolVersion::RESP2 {
        return Err(RedisError::InvalidArgument(
            "only RESP2 Redis URLs are supported",
        ));
    }
    let (host, port) = match &info.addr {
        ConnectionAddr::Tcp(host, port) => (host.clone(), *port),
        ConnectionAddr::TcpTls { .. } => {
            return Err(RedisError::InvalidArgument(
                "TLS Redis URLs are not enabled in this build",
            ));
        }
        ConnectionAddr::Unix(_) => {
            return Err(RedisError::InvalidArgument(
                "Unix-socket Redis URLs are not supported",
            ));
        }
    };
    if info.redis.username.is_some() && info.redis.password.is_none() {
        return Err(RedisError::InvalidArgument(
            "Redis username requires a password",
        ));
    }
    if info.redis.db < 0 {
        return Err(RedisError::InvalidArgument(
            "Redis database number must be nonnegative",
        ));
    }

    let deadline = Instant::now() + OPERATION_TIMEOUT;
    let stream = connect_tcp(&host, port, deadline)?;
    let mut connection = Connection::new(stream)?;

    if let Some(password) = &info.redis.password {
        let reply = if let Some(username) = &info.redis.username {
            execute_text(&mut connection, deadline, "AUTH", &[username, password])?
        } else {
            execute_text(&mut connection, deadline, "AUTH", &[password])?
        };
        into_ok(reply)?;
    }
    if info.redis.db != 0 {
        let database = info.redis.db.to_string();
        into_ok(execute_text(
            &mut connection,
            deadline,
            "SELECT",
            &[&database],
        )?)?;
    }
    reservation.commit(connection)
}

/// Close a connection. This is idempotent. An operation already in progress
/// retains its socket only until its deadline expires.
pub fn close(handle: i64) {
    let connection = {
        let mut registry = crate::native::lock_recover(registry());
        registry.conns.remove(&handle_key(handle))
    };
    drop(connection);
}

/// PING -> PONG (or the custom string returned by the server).
pub fn ping(handle: i64) -> Result<String, RedisError> {
    with_conn(handle, |connection, deadline| {
        into_string(execute_text(connection, deadline, "PING", &[])?)
    })
}

// ---------------- Strings ---------------------------------------------

pub fn set(handle: i64, key: &str, value: &str) -> Result<(), RedisError> {
    validate_key(key)?;
    validate_value(value)?;
    with_conn(handle, |connection, deadline| {
        into_ok(execute_text(connection, deadline, "SET", &[key, value])?)
    })
}

pub fn set_ex(handle: i64, key: &str, value: &str, seconds: u64) -> Result<(), RedisError> {
    validate_key(key)?;
    validate_value(value)?;
    if seconds > i64::MAX as u64 {
        return Err(RedisError::InvalidArgument(
            "expiration seconds are out of range",
        ));
    }
    let seconds = seconds.to_string();
    with_conn(handle, |connection, deadline| {
        into_ok(execute_text(
            connection,
            deadline,
            "SETEX",
            &[key, &seconds, value],
        )?)
    })
}

pub fn get(handle: i64, key: &str) -> Result<Option<String>, RedisError> {
    validate_key(key)?;
    with_conn(handle, |connection, deadline| {
        into_optional_string(execute_text(connection, deadline, "GET", &[key])?)
    })
}

pub fn del(handle: i64, key: &str) -> Result<u64, RedisError> {
    validate_key(key)?;
    with_conn(handle, |connection, deadline| {
        into_u64(execute_text(connection, deadline, "DEL", &[key])?)
    })
}

pub fn exists(handle: i64, key: &str) -> Result<bool, RedisError> {
    validate_key(key)?;
    with_conn(handle, |connection, deadline| {
        into_bool(execute_text(connection, deadline, "EXISTS", &[key])?)
    })
}

pub fn expire(handle: i64, key: &str, seconds: i64) -> Result<bool, RedisError> {
    validate_key(key)?;
    let seconds = seconds.to_string();
    with_conn(handle, |connection, deadline| {
        into_bool(execute_text(
            connection,
            deadline,
            "EXPIRE",
            &[key, &seconds],
        )?)
    })
}

pub fn ttl(handle: i64, key: &str) -> Result<i64, RedisError> {
    validate_key(key)?;
    with_conn(handle, |connection, deadline| {
        into_integer(execute_text(connection, deadline, "TTL", &[key])?)
    })
}

pub fn incr(handle: i64, key: &str, delta: i64) -> Result<i64, RedisError> {
    validate_key(key)?;
    let delta = delta.to_string();
    with_conn(handle, |connection, deadline| {
        into_integer(execute_text(
            connection,
            deadline,
            "INCRBY",
            &[key, &delta],
        )?)
    })
}

/// Return matching keys through bounded SCAN pages. This deliberately never
/// sends the blocking, all-at-once KEYS command.
pub fn keys(handle: i64, pattern: &str) -> Result<Vec<String>, RedisError> {
    validate_size(pattern, "Redis pattern bytes", MAX_KEY_BYTES)?;
    with_conn(handle, |connection, deadline| {
        let mut cursor = "0".to_string();
        let mut keys = Vec::new();
        let mut key_bytes = 0usize;
        for _ in 0..MAX_SCAN_ROUNDS {
            let reply = execute_text(
                connection,
                deadline,
                "SCAN",
                &[&cursor, "MATCH", pattern, "COUNT", "256"],
            )?;
            let mut page = into_array(reply)?;
            if page.len() != 2 {
                return Err(RedisError::UnexpectedResponse(
                    "SCAN reply must contain cursor and keys",
                ));
            }
            let page_keys = into_string_array(page.pop().expect("length checked"))?;
            let next_cursor = into_string(page.pop().expect("length checked"))?;
            next_cursor
                .parse::<u64>()
                .map_err(|_| RedisError::UnexpectedResponse("SCAN cursor was not an integer"))?;

            if keys.len().saturating_add(page_keys.len()) > MAX_COLLECTION_ITEMS {
                return Err(RedisError::ResourceLimit {
                    resource: "Redis collection items",
                    limit: MAX_COLLECTION_ITEMS,
                });
            }
            for key in page_keys {
                key_bytes = key_bytes
                    .checked_add(key.len())
                    .ok_or(RedisError::ResourceLimit {
                        resource: "Redis collection bytes",
                        limit: MAX_COLLECTION_BYTES,
                    })?;
                if key_bytes > MAX_COLLECTION_BYTES {
                    return Err(RedisError::ResourceLimit {
                        resource: "Redis collection bytes",
                        limit: MAX_COLLECTION_BYTES,
                    });
                }
                keys.push(key);
            }
            cursor = next_cursor;
            if cursor == "0" {
                keys.sort_unstable();
                keys.dedup();
                return Ok(keys);
            }
            remaining(deadline, OPERATION_TIMEOUT)?;
        }
        Err(RedisError::ResourceLimit {
            resource: "Redis SCAN rounds",
            limit: MAX_SCAN_ROUNDS,
        })
    })
}

// ---------------- Lists ------------------------------------------------

pub fn lpush(handle: i64, key: &str, value: &str) -> Result<u64, RedisError> {
    validate_key(key)?;
    validate_value(value)?;
    with_conn(handle, |connection, deadline| {
        into_u64(execute_text(connection, deadline, "LPUSH", &[key, value])?)
    })
}

pub fn rpush(handle: i64, key: &str, value: &str) -> Result<u64, RedisError> {
    validate_key(key)?;
    validate_value(value)?;
    with_conn(handle, |connection, deadline| {
        into_u64(execute_text(connection, deadline, "RPUSH", &[key, value])?)
    })
}

fn normalise_list_index(index: i64, length: i64) -> i64 {
    if index < 0 {
        length.saturating_add(index)
    } else {
        index
    }
}

pub fn lrange(handle: i64, key: &str, start: i64, stop: i64) -> Result<Vec<String>, RedisError> {
    validate_key(key)?;
    with_conn(handle, |connection, deadline| {
        // Resolve negative indexes first and send a fixed, bounded interval.
        // A concurrent mutation can shift values, but cannot increase the
        // number of values returned by the final LRANGE.
        let length = into_integer(execute_text(connection, deadline, "LLEN", &[key])?)?;
        if length < 0 {
            return Err(RedisError::UnexpectedResponse(
                "LLEN returned a negative length",
            ));
        }
        if length == 0 {
            return Ok(Vec::new());
        }
        let start = normalise_list_index(start, length).max(0);
        let stop = normalise_list_index(stop, length).min(length - 1);
        if stop < 0 || stop < start || start >= length {
            return Ok(Vec::new());
        }
        let count = stop
            .checked_sub(start)
            .and_then(|value| value.checked_add(1))
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(RedisError::ResourceLimit {
                resource: "Redis collection items",
                limit: MAX_COLLECTION_ITEMS,
            })?;
        if count > MAX_COLLECTION_ITEMS {
            return Err(RedisError::ResourceLimit {
                resource: "Redis collection items",
                limit: MAX_COLLECTION_ITEMS,
            });
        }
        let start = start.to_string();
        let stop = stop.to_string();
        into_string_array(execute_text(
            connection,
            deadline,
            "LRANGE",
            &[key, &start, &stop],
        )?)
    })
}

pub fn llen(handle: i64, key: &str) -> Result<u64, RedisError> {
    validate_key(key)?;
    with_conn(handle, |connection, deadline| {
        into_u64(execute_text(connection, deadline, "LLEN", &[key])?)
    })
}

// ---------------- Hashes -----------------------------------------------

pub fn hset(handle: i64, key: &str, field: &str, value: &str) -> Result<(), RedisError> {
    validate_key(key)?;
    validate_size(field, "Redis hash field bytes", MAX_KEY_BYTES)?;
    validate_value(value)?;
    with_conn(handle, |connection, deadline| {
        let changed = into_integer(execute_text(
            connection,
            deadline,
            "HSET",
            &[key, field, value],
        )?)?;
        if matches!(changed, 0 | 1) {
            Ok(())
        } else {
            Err(RedisError::UnexpectedResponse(
                "HSET returned an invalid count",
            ))
        }
    })
}

pub fn hget(handle: i64, key: &str, field: &str) -> Result<Option<String>, RedisError> {
    validate_key(key)?;
    validate_size(field, "Redis hash field bytes", MAX_KEY_BYTES)?;
    with_conn(handle, |connection, deadline| {
        into_optional_string(execute_text(connection, deadline, "HGET", &[key, field])?)
    })
}

pub fn hdel(handle: i64, key: &str, field: &str) -> Result<u64, RedisError> {
    validate_key(key)?;
    validate_size(field, "Redis hash field bytes", MAX_KEY_BYTES)?;
    with_conn(handle, |connection, deadline| {
        into_u64(execute_text(connection, deadline, "HDEL", &[key, field])?)
    })
}

pub fn hgetall(handle: i64, key: &str) -> Result<Vec<(String, String)>, RedisError> {
    validate_key(key)?;
    with_conn(handle, |connection, deadline| {
        let values = into_string_array(execute_text(connection, deadline, "HGETALL", &[key])?)?;
        if values.len() % 2 != 0 {
            return Err(RedisError::UnexpectedResponse(
                "HGETALL returned an odd number of values",
            ));
        }
        let mut pairs = Vec::with_capacity(values.len() / 2);
        let mut values = values.into_iter();
        while let (Some(field), Some(value)) = (values.next(), values.next()) {
            pairs.push((field, value));
        }
        Ok(pairs)
    })
}

const RAW_ALLOWLIST: &[&str] = &[
    "APPEND",
    "DEL",
    "DECR",
    "DECRBY",
    "EXISTS",
    "EXPIRE",
    "GET",
    "GETDEL",
    "GETEX",
    "GETRANGE",
    "GETSET",
    "HDEL",
    "HEXISTS",
    "HGET",
    "HGETALL",
    "HSET",
    "HLEN",
    "HSTRLEN",
    "INCR",
    "INCRBY",
    "INCRBYFLOAT",
    "LINDEX",
    "LLEN",
    "LPOP",
    "LPUSH",
    "LRANGE",
    "MGET",
    "MSET",
    "MSETNX",
    "PERSIST",
    "PEXPIRE",
    "PING",
    "PTTL",
    "PUBLISH",
    "RPOP",
    "RPUSH",
    "SCAN",
    "SCARD",
    "SET",
    "SETEX",
    "SETNX",
    "SISMEMBER",
    "SREM",
    "STRLEN",
    "TOUCH",
    "TTL",
    "TYPE",
    "UNLINK",
    "ZCARD",
    "ZREM",
    "ZSCORE",
];

fn push_rendered(output: &mut String, text: &str) -> Result<(), RedisError> {
    if output.len().saturating_add(text.len()) > MAX_COLLECTION_BYTES {
        return Err(RedisError::ResourceLimit {
            resource: "Redis raw output bytes",
            limit: MAX_COLLECTION_BYTES,
        });
    }
    output.push_str(text);
    Ok(())
}

fn render_bytes(bytes: &[u8], output: &mut String) -> Result<(), RedisError> {
    push_rendered(output, "\"")?;
    for &byte in bytes {
        match byte {
            b'"' => push_rendered(output, "\\\"")?,
            b'\\' => push_rendered(output, "\\\\")?,
            b'\n' => push_rendered(output, "\\n")?,
            b'\r' => push_rendered(output, "\\r")?,
            b'\t' => push_rendered(output, "\\t")?,
            0x20..=0x7e => {
                let character = [byte];
                push_rendered(output, std::str::from_utf8(&character).expect("ASCII"))?;
            }
            _ => push_rendered(output, &format!("\\x{byte:02x}"))?,
        }
    }
    push_rendered(output, "\"")
}

fn render_raw(value: RespValue, output: &mut String) -> Result<(), RedisError> {
    match value {
        RespValue::Simple(bytes) | RespValue::Bulk(bytes) => render_bytes(&bytes, output),
        RespValue::Integer(value) => push_rendered(output, &value.to_string()),
        RespValue::Null => push_rendered(output, "null"),
        RespValue::Array(values) => {
            push_rendered(output, "[")?;
            for (index, value) in values.into_iter().enumerate() {
                if index != 0 {
                    push_rendered(output, ", ")?;
                }
                render_raw(value, output)?;
            }
            push_rendered(output, "]")
        }
        RespValue::Error(_) => Err(RedisError::Protocol(
            "server errors must be handled before rendering",
        )),
    }
}

/// Execute one explicitly allowlisted, single-reply Redis command.
///
/// Stateful/protocol-changing, blocking, administrative and script commands
/// are rejected so a raw call cannot desynchronise or indefinitely occupy the
/// connection. Arguments retain the historical whitespace-splitting syntax.
pub fn raw(handle: i64, command_and_args: &str) -> Result<String, RedisError> {
    validate_size(
        command_and_args,
        "Redis raw command bytes",
        MAX_RAW_COMMAND_BYTES,
    )?;
    let parts = command_and_args.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(RedisError::InvalidArgument(
            "raw Redis command cannot be empty",
        ));
    }
    if parts.len() > MAX_COMMAND_ARGUMENTS {
        return Err(RedisError::ResourceLimit {
            resource: "Redis command arguments",
            limit: MAX_COMMAND_ARGUMENTS,
        });
    }
    let name = parts[0].to_ascii_uppercase();
    if !RAW_ALLOWLIST.contains(&name.as_str()) {
        return Err(RedisError::InvalidArgument(
            "raw Redis command is not in the safe allowlist",
        ));
    }
    with_conn(handle, |connection, deadline| {
        let value = execute_text(connection, deadline, &name, &parts[1..])?;
        let mut output = String::new();
        render_raw(value, &mut output)?;
        Ok(output)
    })
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let released = {
        let mut registry = crate::native::lock_recover(registry());
        let released = crate::native::remove_runtime_entries(&mut registry.conns, runtime_id);
        registry.reserved.remove(&runtime_id);
        released
    };
    let mut usage = crate::native::lock_recover(runtime_usage());
    if usage
        .get(&runtime_id)
        .is_some_and(|runtime| runtime.active_operations == 0)
    {
        usage.remove(&runtime_id);
    }
    released
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;

    static NEXT_TEST_RUNTIME: AtomicU64 = AtomicU64::new(10_000);

    fn in_test_runtime<R>(test: impl FnOnce(u64) -> R) -> R {
        let runtime_id = NEXT_TEST_RUNTIME.fetch_add(1, Ordering::Relaxed);
        crate::native::with_runtime_context(runtime_id, || test(runtime_id))
    }

    fn spawn_server(
        handler: impl FnOnce(TcpStream) + Send + 'static,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let join = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handler(stream);
        });
        (format!("redis://{address}/"), join)
    }

    fn server_read_line(reader: &mut BufReader<TcpStream>) -> Vec<u8> {
        let mut line = Vec::new();
        reader.read_until(b'\n', &mut line).unwrap();
        assert!(line.ends_with(b"\r\n"));
        line.truncate(line.len() - 2);
        line
    }

    fn server_read_command(reader: &mut BufReader<TcpStream>) -> Vec<String> {
        let header = server_read_line(reader);
        assert_eq!(header.first(), Some(&b'*'));
        let count = std::str::from_utf8(&header[1..])
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let mut command = Vec::with_capacity(count);
        for _ in 0..count {
            let header = server_read_line(reader);
            assert_eq!(header.first(), Some(&b'$'));
            let length = std::str::from_utf8(&header[1..])
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let mut bytes = vec![0; length];
            reader.read_exact(&mut bytes).unwrap();
            let mut crlf = [0; 2];
            reader.read_exact(&mut crlf).unwrap();
            assert_eq!(&crlf, b"\r\n");
            command.push(String::from_utf8(bytes).unwrap());
        }
        command
    }

    fn scripted_server(
        script: Vec<(Vec<&'static str>, &'static [u8])>,
    ) -> (String, thread::JoinHandle<()>) {
        spawn_server(move |stream| {
            let mut reader = BufReader::new(stream);
            for (expected, response) in script {
                assert_eq!(server_read_command(&mut reader), expected);
                reader.get_mut().write_all(response).unwrap();
            }
        })
    }

    #[test]
    fn real_resp2_round_trip_covers_wrappers() {
        in_test_runtime(|runtime_id| {
            let (url, server) = scripted_server(vec![
                (vec!["PING"], b"+PONG\r\n"),
                (vec!["SET", "key", "value"], b"+OK\r\n"),
                (vec!["GET", "key"], b"$5\r\nvalue\r\n"),
                (vec!["EXISTS", "key"], b":1\r\n"),
                (vec!["INCRBY", "counter", "2"], b":7\r\n"),
                (vec!["HSET", "hash", "field", "value"], b":1\r\n"),
                (
                    vec!["HGETALL", "hash"],
                    b"*2\r\n$5\r\nfield\r\n$5\r\nvalue\r\n",
                ),
                (vec!["DEL", "key"], b":1\r\n"),
            ]);
            let handle = connect(&url).unwrap();
            assert_eq!(ping(handle).unwrap(), "PONG");
            set(handle, "key", "value").unwrap();
            assert_eq!(get(handle, "key").unwrap(), Some("value".into()));
            assert!(exists(handle, "key").unwrap());
            assert_eq!(incr(handle, "counter", 2).unwrap(), 7);
            hset(handle, "hash", "field", "value").unwrap();
            assert_eq!(
                hgetall(handle, "hash").unwrap(),
                vec![("field".into(), "value".into())]
            );
            assert_eq!(del(handle, "key").unwrap(), 1);
            close(handle);
            server.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn authentication_and_database_selection_are_real_commands() {
        in_test_runtime(|runtime_id| {
            let (url, server) = scripted_server(vec![
                (vec!["AUTH", "user", "secret"], b"+OK\r\n"),
                (vec!["SELECT", "3"], b"+OK\r\n"),
                (vec!["PING"], b"+PONG\r\n"),
            ]);
            let address = url.strip_prefix("redis://").unwrap();
            let url = format!("redis://user:secret@{address}3");
            let handle = connect(&url).unwrap();
            assert_eq!(ping(handle).unwrap(), "PONG");
            close(handle);
            server.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn keys_uses_bounded_scan_pages_instead_of_keys() {
        in_test_runtime(|runtime_id| {
            let (url, server) = scripted_server(vec![
                (
                    vec!["SCAN", "0", "MATCH", "user:*", "COUNT", "256"],
                    b"*2\r\n$2\r\n17\r\n*1\r\n$6\r\nuser:1\r\n",
                ),
                (
                    vec!["SCAN", "17", "MATCH", "user:*", "COUNT", "256"],
                    b"*2\r\n$1\r\n0\r\n*2\r\n$6\r\nuser:1\r\n$6\r\nuser:2\r\n",
                ),
            ]);
            let handle = connect(&url).unwrap();
            assert_eq!(
                keys(handle, "user:*").unwrap(),
                vec!["user:1".to_string(), "user:2".to_string()]
            );
            close(handle);
            server.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn lrange_resolves_negative_indexes_and_caps_the_interval() {
        in_test_runtime(|runtime_id| {
            let (url, server) = scripted_server(vec![
                (vec!["LLEN", "items"], b":3\r\n"),
                (
                    vec!["LRANGE", "items", "0", "2"],
                    b"*3\r\n$1\r\na\r\n$1\r\nb\r\n$1\r\nc\r\n",
                ),
            ]);
            let handle = connect(&url).unwrap();
            assert_eq!(lrange(handle, "items", 0, -1).unwrap(), ["a", "b", "c"]);
            close(handle);
            server.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn server_error_is_typed_and_does_not_desynchronise_connection() {
        in_test_runtime(|runtime_id| {
            let (url, server) = scripted_server(vec![
                (vec!["GET", "wrong-type"], b"-WRONGTYPE not a string\r\n"),
                (vec!["PING"], b"+PONG\r\n"),
            ]);
            let handle = connect(&url).unwrap();
            assert!(matches!(
                get(handle, "wrong-type"),
                Err(RedisError::Server(message)) if message == "WRONGTYPE not a string"
            ));
            assert_eq!(ping(handle).unwrap(), "PONG");
            close(handle);
            server.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn safe_raw_command_executes_and_renders_a_bounded_reply() {
        in_test_runtime(|runtime_id| {
            let (url, server) = scripted_server(vec![(vec!["STRLEN", "key"], b":5\r\n")]);
            let handle = connect(&url).unwrap();
            assert_eq!(raw(handle, "strlen key").unwrap(), "5");
            close(handle);
            server.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    // Windows resolves `localhost` to IPv6 (::1); the test server binds IPv4
    // only, so this real-hostname-resolution check is Unix-only.
    #[cfg(not(windows))]
    #[test]
    fn hostname_resolution_is_real_and_releases_its_quota() {
        in_test_runtime(|runtime_id| {
            let (url, server) = scripted_server(vec![(vec!["PING"], b"+PONG\r\n")]);
            let url = url.replacen("127.0.0.1", "localhost", 1);
            let handle = connect(&url).unwrap();
            assert_eq!(ping(handle).unwrap(), "PONG");
            close(handle);
            server.join().unwrap();
            assert!(!crate::native::lock_recover(resolver_usage())
                .active_by_runtime
                .contains_key(&runtime_id));
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn malformed_and_over_nested_replies_close_the_connection() {
        in_test_runtime(|runtime_id| {
            let (url, server) = spawn_server(|stream| {
                let mut reader = BufReader::new(stream);
                assert_eq!(server_read_command(&mut reader), vec!["GET", "key"]);
                reader.get_mut().write_all(b"$3\r\nabcX\n").unwrap();
            });
            let handle = connect(&url).unwrap();
            assert!(matches!(get(handle, "key"), Err(RedisError::Protocol(_))));
            assert!(matches!(ping(handle), Err(RedisError::UnknownHandle(_))));
            server.join().unwrap();

            let (url, server) = spawn_server(|stream| {
                let mut reader = BufReader::new(stream);
                assert_eq!(server_read_command(&mut reader), vec!["GET", "key"]);
                let mut reply = b"*1\r\n".repeat(MAX_RESPONSE_DEPTH + 2);
                reply.extend_from_slice(b"+value\r\n");
                reader.get_mut().write_all(&reply).unwrap();
            });
            let handle = connect(&url).unwrap();
            assert!(matches!(
                get(handle, "key"),
                Err(RedisError::ResourceLimit {
                    resource: "Redis response nesting depth",
                    ..
                })
            ));
            assert!(matches!(ping(handle), Err(RedisError::UnknownHandle(_))));
            server.join().unwrap();

            let (url, server) = spawn_server(|stream| {
                let mut reader = BufReader::new(stream);
                assert_eq!(server_read_command(&mut reader), vec!["GET", "key"]);
                write!(reader.get_mut(), "*{}\r\n", MAX_RESPONSE_ELEMENTS).unwrap();
            });
            let handle = connect(&url).unwrap();
            assert!(matches!(
                get(handle, "key"),
                Err(RedisError::ResourceLimit {
                    resource: "Redis response elements",
                    ..
                })
            ));
            assert!(matches!(ping(handle), Err(RedisError::UnknownHandle(_))));
            server.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn oversized_reply_is_rejected_before_bulk_allocation_and_closes_handle() {
        in_test_runtime(|runtime_id| {
            let (url, server) = spawn_server(|stream| {
                let mut reader = BufReader::new(stream);
                assert_eq!(server_read_command(&mut reader), vec!["GET", "key"]);
                write!(reader.get_mut(), "${}\r\n", MAX_RESPONSE_PAYLOAD_BYTES + 1).unwrap();
            });
            let handle = connect(&url).unwrap();
            assert!(matches!(
                get(handle, "key"),
                Err(RedisError::ResourceLimit {
                    resource: "Redis response payload bytes",
                    ..
                })
            ));
            assert!(matches!(ping(handle), Err(RedisError::UnknownHandle(_))));
            server.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn stalled_server_hits_deadline_and_closes_handle() {
        in_test_runtime(|runtime_id| {
            let (url, server) = spawn_server(|stream| {
                let mut reader = BufReader::new(stream);
                assert_eq!(server_read_command(&mut reader), vec!["PING"]);
                thread::sleep(OPERATION_TIMEOUT + Duration::from_millis(150));
            });
            let handle = connect(&url).unwrap();
            assert!(matches!(ping(handle), Err(RedisError::Timeout)));
            assert!(matches!(ping(handle), Err(RedisError::UnknownHandle(_))));
            server.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    // Kept on Linux only, and not because of OPERATION_TIMEOUT: the
    // assertion below requires an unrelated command to complete in under
    // 250ms while another connection is stalled, which measures host speed
    // rather than the property under test. Hosted macOS/Windows runners
    // cannot honour that budget reliably.
    #[cfg(target_os = "linux")]
    #[test]
    fn slow_connection_does_not_hold_the_global_registry_lock() {
        in_test_runtime(|runtime_id| {
            let (request_started_tx, request_started_rx) = std::sync::mpsc::channel();
            let (slow_url, slow_server) = spawn_server(move |stream| {
                let mut reader = BufReader::new(stream);
                assert_eq!(server_read_command(&mut reader), vec!["PING"]);
                request_started_tx.send(()).unwrap();
                thread::sleep(Duration::from_millis(400));
                reader.get_mut().write_all(b"+PONG\r\n").unwrap();
            });
            let (fast_url, fast_server) = scripted_server(vec![(vec!["PING"], b"+PONG\r\n")]);
            let slow = connect(&slow_url).unwrap();
            let fast = connect(&fast_url).unwrap();
            let slow_thread = thread::spawn(move || {
                crate::native::with_runtime_context(runtime_id, || ping(slow))
            });
            request_started_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap();
            let started = Instant::now();
            assert_eq!(ping(fast).unwrap(), "PONG");
            assert!(started.elapsed() < Duration::from_millis(250));
            assert_eq!(slow_thread.join().unwrap().unwrap(), "PONG");
            close(slow);
            close(fast);
            slow_server.join().unwrap();
            fast_server.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn raw_rejects_stateful_blocking_and_unknown_commands() {
        in_test_runtime(|runtime_id| {
            for command in [
                "SUBSCRIBE channel",
                "BLPOP list 0",
                "MULTI",
                "CLIENT REPLY OFF",
                "HELLO 3",
                "EVAL return 1 0",
                "CUSTOM anything",
            ] {
                assert!(matches!(
                    raw(999_999, command),
                    Err(RedisError::InvalidArgument(
                        "raw Redis command is not in the safe allowlist"
                    ))
                ));
            }
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn handles_are_runtime_owned_and_cleanup_closes_the_socket() {
        in_test_runtime(|runtime_id| {
            let (url, server) = spawn_server(|mut stream| {
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let mut byte = [0u8; 1];
                assert_eq!(stream.read(&mut byte).unwrap(), 0);
            });
            let handle = connect(&url).unwrap();
            let other_runtime = NEXT_TEST_RUNTIME.fetch_add(1, Ordering::Relaxed);
            crate::native::with_runtime_context(other_runtime, || {
                assert!(matches!(ping(handle), Err(RedisError::UnknownHandle(_))));
                assert_eq!(cleanup_runtime(other_runtime), 0);
            });
            assert_eq!(cleanup_runtime(runtime_id), 1);
            server.join().unwrap();
        });
    }

    #[test]
    fn cleanup_invalidates_an_inflight_handle_reservation() {
        in_test_runtime(|runtime_id| {
            let reservation = reserve_handle().unwrap();
            let (url, server) = spawn_server(|mut stream| {
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let mut byte = [0u8; 1];
                assert_eq!(stream.read(&mut byte).unwrap(), 0);
            });
            let address = url.strip_prefix("redis://").unwrap().trim_end_matches('/');
            let connection = Connection::new(TcpStream::connect(address).unwrap()).unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
            assert!(matches!(
                reservation.commit(connection),
                Err(RedisError::RuntimeClosed)
            ));
            server.join().unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn runtime_ownership_cleanup_and_quotas_are_enforced() {
        in_test_runtime(|runtime_id| {
            let reservations = (0..MAX_CONNECTIONS_PER_RUNTIME)
                .map(|_| reserve_handle().unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                reserve_handle(),
                Err(RedisError::ResourceLimit {
                    resource: "Redis connection handles",
                    ..
                })
            ));
            drop(reservations);

            let permits = (0..MAX_CONCURRENT_OPERATIONS)
                .map(|_| reserve_operation().unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                reserve_operation(),
                Err(RedisError::ResourceLimit {
                    resource: "concurrent Redis operations",
                    ..
                })
            ));
            drop(permits);

            let resolvers = (0..MAX_DNS_RESOLVERS_PER_RUNTIME)
                .map(|_| reserve_resolver(runtime_id).unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                reserve_resolver(runtime_id),
                Err(RedisError::ResourceLimit {
                    resource: "concurrent Redis DNS resolvers per runtime",
                    ..
                })
            ));
            drop(resolvers);
            assert_eq!(cleanup_runtime(runtime_id), 0);
            assert!(!crate::native::lock_recover(runtime_usage()).contains_key(&runtime_id));
            assert!(!crate::native::lock_recover(resolver_usage())
                .active_by_runtime
                .contains_key(&runtime_id));
        });
    }

    #[test]
    fn argument_and_protocol_limits_fail_cleanly() {
        in_test_runtime(|runtime_id| {
            let oversized_key = "k".repeat(MAX_KEY_BYTES + 1);
            assert!(matches!(
                get(123, &oversized_key),
                Err(RedisError::ResourceLimit {
                    resource: "Redis key bytes",
                    ..
                })
            ));
            let resp3 = "redis://127.0.0.1:1/?protocol=resp3";
            assert!(matches!(
                connect(resp3),
                Err(RedisError::InvalidArgument(
                    "only RESP2 Redis URLs are supported"
                ))
            ));
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn unknown_handle_reports_typed_error() {
        in_test_runtime(|runtime_id| {
            assert!(matches!(ping(999_999), Err(RedisError::UnknownHandle(_))));
            assert!(matches!(
                get(999_999, "x"),
                Err(RedisError::UnknownHandle(_))
            ));
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    /// Optional interoperability check against an actual Redis deployment.
    #[test]
    fn live_round_trip_when_configured() {
        let Ok(url) = std::env::var("TITAN_REDIS_TEST_URL") else {
            return;
        };
        in_test_runtime(|runtime_id| {
            let handle = connect(&url).expect("connect");
            assert_eq!(ping(handle).unwrap(), "PONG");
            set(handle, "titan:test", "hola").unwrap();
            assert_eq!(get(handle, "titan:test").unwrap(), Some("hola".into()));
            del(handle, "titan:test").unwrap();
            close(handle);
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }
}

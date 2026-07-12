//! Small, dependency-free HTTP/1.1 and TCP helpers.
//!
//! HTTPS is rejected explicitly because silently sending plaintext to a TLS
//! endpoint is unsafe. Applications needing HTTPS should use a TLS-enabled
//! host binding until Titan's native TLS module lands.

use std::io;
use std::net::{TcpListener, TcpStream};

pub struct TcpServer { listener: TcpListener }
impl TcpServer {
    pub fn bind(addr: &str) -> io::Result<Self> { Ok(Self { listener: TcpListener::bind(addr)? }) }
    pub fn accept(&self) -> io::Result<(TcpStream, String)> { let (stream, address) = self.listener.accept()?; Ok((stream, address.to_string())) }
    pub fn local_addr(&self) -> io::Result<String> { self.listener.local_addr().map(|a| a.to_string()) }
}

pub struct TcpClient { stream: TcpStream }
impl TcpClient {
    pub fn connect(addr: &str) -> io::Result<Self> { Ok(Self { stream: TcpStream::connect(addr)? }) }
    pub fn send(&mut self, data: &[u8]) -> io::Result<usize> { use std::io::Write; self.stream.write(data) }
    pub fn send_str(&mut self, data: &str) -> io::Result<()> { use std::io::Write; self.stream.write_all(data.as_bytes()) }
    pub fn recv_str(&mut self, max: usize) -> io::Result<String> { use std::io::Read; let mut buffer = vec![0; max]; let count = self.stream.read(&mut buffer)?; Ok(String::from_utf8_lossy(&buffer[..count]).into()) }
    pub fn peer_addr(&self) -> io::Result<String> { self.stream.peer_addr().map(|a| a.to_string()) }
}

#[derive(Debug, Clone)]
pub struct HttpResponse { pub status: u16, pub headers: Vec<(String, String)>, pub body: Vec<u8> }
impl HttpResponse {
    pub fn is_success(&self) -> bool { (200..300).contains(&self.status) }
    pub fn text(&self) -> String { String::from_utf8_lossy(&self.body).into() }
}

pub fn http_get(url: &str) -> io::Result<HttpResponse> {
    if url.starts_with("https://") { return Err(io::Error::new(io::ErrorKind::InvalidInput, "HTTPS requires a TLS-enabled client")); }
    let raw = url.strip_prefix("http://").ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "URL must start with http://"))?;
    let (authority, path) = raw.split_once('/').map(|(a, p)| (a, format!("/{p}"))).unwrap_or((raw, "/".into()));
    let (host, port) = authority.rsplit_once(':').map(|(h, p)| (h, p)).unwrap_or((authority, "80"));
    if host.is_empty() { return Err(io::Error::new(io::ErrorKind::InvalidInput, "URL has no host")); }

    use std::io::{Read, Write};
    let mut connection = TcpStream::connect((host, port.parse::<u16>().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid port"))?))?;
    connection.write_all(format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\nUser-Agent: Titan/0.2\r\nAccept: */*\r\nConnection: close\r\n\r\n").as_bytes())?;
    let mut bytes = Vec::new(); connection.read_to_end(&mut bytes)?;
    let separator = bytes.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed HTTP response"))?;
    let head = String::from_utf8_lossy(&bytes[..separator]);
    let mut lines = head.split("\r\n");
    let status = lines.next().and_then(|line| line.split_whitespace().nth(1)).and_then(|s| s.parse().ok()).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid HTTP status"))?;
    let headers = lines.filter_map(|line| line.split_once(':')).map(|(name, value)| (name.trim().into(), value.trim().into())).collect();
    Ok(HttpResponse { status, headers, body: bytes[separator + 4..].to_vec() })
}

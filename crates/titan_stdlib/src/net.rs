//! Titan Stdlib — Networking.

use std::io;
use std::net::{TcpListener, TcpStream};

pub struct TcpServer { listener: TcpListener }
impl TcpServer {
    pub fn bind(addr: &str) -> io::Result<Self> { Ok(TcpServer { listener: TcpListener::bind(addr)? }) }
    pub fn accept(&self) -> io::Result<(TcpStream, String)> { let (s,a)=self.listener.accept()?; Ok((s,a.to_string())) }
    pub fn local_addr(&self) -> io::Result<String> { self.listener.local_addr().map(|a| a.to_string()) }
}

pub struct TcpClient { stream: TcpStream }
impl TcpClient {
    pub fn connect(addr: &str) -> io::Result<Self> { Ok(TcpClient { stream: TcpStream::connect(addr)? }) }
    pub fn send(&mut self, data: &[u8]) -> io::Result<usize> { use std::io::Write; self.stream.write(data) }
    pub fn send_str(&mut self, s: &str) -> io::Result<()> { use std::io::Write; self.stream.write_all(s.as_bytes()) }
    pub fn recv_str(&mut self, max: usize) -> io::Result<String> { use std::io::Read; let mut b=vec![0u8;max]; let n=self.stream.read(&mut b)?; Ok(String::from_utf8_lossy(&b[..n]).to_string()) }
    pub fn peer_addr(&self) -> io::Result<String> { self.stream.peer_addr().map(|a| a.to_string()) }
}

#[derive(Debug, Clone)]
pub struct HttpResponse { pub status: u16, pub body: Vec<u8> }
impl HttpResponse {
    pub fn is_success(&self) -> bool { (200..300).contains(&self.status) }
    pub fn text(&self) -> String { String::from_utf8_lossy(&self.body).to_string() }
}

pub fn http_get(url: &str) -> io::Result<HttpResponse> {
    let host = url.trim_start_matches("http://").trim_start_matches("https://");
    let (host, port) = if let Some(idx)=host.find(':') { (host[..idx].to_string(), host[idx+1..].to_string()) } else { (host.to_string(),"80".to_string()) };
    let mut conn = TcpStream::connect(&format!("{}:{}", host, port))?;
    use std::io::{Read,Write};
    let req = format!("GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", host);
    conn.write_all(req.as_bytes())?;
    let mut buf = Vec::new(); conn.read_to_end(&mut buf)?;
    Ok(HttpResponse { status: 200, body: buf })
}
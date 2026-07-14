//! TLS 1.2/1.3 transport powered by rustls and WebPKI roots.

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned};
pub use rustls::ServerConfig as RustlsServerConfig;
use std::io::{self, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TlsError {
    #[error("TLS I/O error: {0}")] Io(#[from] io::Error),
    #[error("TLS protocol error: {0}")] Protocol(#[from] rustls::Error),
    #[error("invalid DNS server name '{0}'")] ServerName(String),
    #[error("certificate file contains no certificates")]
    NoCertificates,
    #[error("private key file contains no supported private key")]
    NoPrivateKey,
}

pub enum TlsStream {
    Client(StreamOwned<ClientConnection, TcpStream>),
    Server(StreamOwned<ServerConnection, TcpStream>),
}
impl Read for TlsStream { fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> { match self { Self::Client(stream) => stream.read(buffer), Self::Server(stream) => stream.read(buffer) } } }
impl Write for TlsStream { fn write(&mut self, buffer: &[u8]) -> io::Result<usize> { match self { Self::Client(stream) => stream.write(buffer), Self::Server(stream) => stream.write(buffer) } } fn flush(&mut self) -> io::Result<()> { match self { Self::Client(stream) => stream.flush(), Self::Server(stream) => stream.flush() } } }

pub fn client_config() -> Arc<ClientConfig> {
    let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Arc::new(ClientConfig::builder().with_root_certificates(roots).with_no_client_auth())
}
pub fn client_config_with_ca(pem: &[u8]) -> Result<Arc<ClientConfig>, TlsError> {
    let mut reader = BufReader::new(pem); let certificates = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    if certificates.is_empty() { return Err(TlsError::NoCertificates); }
    let mut roots = RootCertStore::empty(); for certificate in certificates { roots.add(certificate)?; }
    Ok(Arc::new(ClientConfig::builder().with_root_certificates(roots).with_no_client_auth()))
}

pub fn connect(address: &str, server_name: &str, config: Arc<ClientConfig>) -> Result<TlsStream, TlsError> { connect_with_timeout(address, server_name, config, std::time::Duration::from_secs(10)) }
pub fn connect_with_timeout(address: &str, server_name: &str, config: Arc<ClientConfig>, timeout: std::time::Duration) -> Result<TlsStream, TlsError> {
    let name = ServerName::try_from(server_name.to_owned()).map_err(|_| TlsError::ServerName(server_name.into()))?;
    let socket = TcpStream::connect(address)?; socket.set_nodelay(true)?; socket.set_read_timeout(Some(timeout))?; socket.set_write_timeout(Some(timeout))?;
    let connection = ClientConnection::new(config, name)?;
    let mut stream = StreamOwned::new(connection, socket);
    stream.conn.complete_io(&mut stream.sock)?;
    Ok(TlsStream::Client(stream))
}

pub fn server_config(cert_path: impl AsRef<Path>, key_path: impl AsRef<Path>) -> Result<Arc<ServerConfig>, TlsError> {
    let mut certificates = BufReader::new(std::fs::File::open(cert_path)?);
    let certificates = rustls_pemfile::certs(&mut certificates).collect::<Result<Vec<_>, _>>()?;
    if certificates.is_empty() { return Err(TlsError::NoCertificates); }
    let mut keys = BufReader::new(std::fs::File::open(key_path)?);
    let key = rustls_pemfile::private_key(&mut keys)?.ok_or(TlsError::NoPrivateKey)?;
    Ok(Arc::new(ServerConfig::builder().with_no_client_auth().with_single_cert(certificates, key)?))
}

pub fn accept(socket: TcpStream, config: Arc<ServerConfig>) -> Result<TlsStream, TlsError> {
    let connection = ServerConnection::new(config)?;
    let mut stream = StreamOwned::new(connection, socket);
    stream.conn.complete_io(&mut stream.sock)?;
    Ok(TlsStream::Server(stream))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn builds_webpki_client_config_and_rejects_bad_names() { let config = client_config(); assert!(connect("127.0.0.1:1", "not a dns name!", config).is_err()); }
    #[test] fn rejects_empty_server_credentials() { let root=std::env::temp_dir().join(format!("titan-tls-{}",std::process::id()));let _=std::fs::create_dir_all(&root);let cert=root.join("cert.pem");let key=root.join("key.pem");std::fs::write(&cert,"").unwrap();std::fs::write(&key,"").unwrap();assert!(server_config(&cert,&key).is_err());let _=std::fs::remove_dir_all(root); }
    #[test] fn performs_verified_local_tls_handshake() { let certified=rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();let cert_pem=certified.cert.pem();let key_pem=certified.key_pair.serialize_pem();let root=std::env::temp_dir().join(format!("titan-tls-handshake-{}",std::process::id()));let _=std::fs::create_dir_all(&root);let cert=root.join("cert.pem");let key=root.join("key.pem");std::fs::write(&cert,&cert_pem).unwrap();std::fs::write(&key,key_pem).unwrap();let server_config=server_config(&cert,&key).unwrap();let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();let server=std::thread::spawn(move||{let(socket,_)=listener.accept().unwrap();let mut stream=accept(socket,server_config).unwrap();let mut data=[0;4];stream.read_exact(&mut data).unwrap();assert_eq!(&data,b"ping");stream.write_all(b"pong").unwrap();});let config=client_config_with_ca(cert_pem.as_bytes()).unwrap();let mut client=connect(&address.to_string(),"localhost",config).unwrap();client.write_all(b"ping").unwrap();let mut response=[0;4];client.read_exact(&mut response).unwrap();assert_eq!(&response,b"pong");server.join().unwrap();std::fs::remove_dir_all(root).unwrap(); }
}

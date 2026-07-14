# TITAN TLS

TITAN uses rustls 0.23 and WebPKI roots; it does not implement cryptography itself. TLS 1.2/1.3 streams are VM-managed handles and require the Network capability. Loading server credentials additionally requires Filesystem.

```titan
let stream = std::tls::connect("example.com:443", "example.com")
std::tls::write(stream, std::encoding::utf8_encode("GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n"))
let response = std::tls::read(stream, 65536)
std::tls::close(stream)
```

Server side:

```titan
let listener = std::net::tcp_listen("0.0.0.0:443")
let config = std::tls::server_config("cert.pem", "key.pem")
let accepted = std::tls::accept(listener, config)
let stream = accepted[0]
```

Features include WebPKI certificate-chain/hostname validation, SNI, PEM certificate chains, PKCS#8/PKCS#1/SEC1 private-key parsing, complete handshake before returning a stream, binary reads/writes, explicit close, and sandbox enforcement. A local integration test generates a certificate, performs a verified client/server handshake, and exchanges ping/pong over encrypted loopback.

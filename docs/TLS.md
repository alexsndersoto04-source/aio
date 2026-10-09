# TITAN TLS

> **Nota:** este documento se escribió para el prototipo en Rust. La API de Titan que describe (funciones, argumentos, resultados)
> sigue siendo la del lenguaje, pero las referencias a la VM, a hilos del sistema, a «capabilities» o a bibliotecas de Rust ya no aplican:
> hoy todo está en Titan y no hay VM al ejecutar. Lo vigente y verificado está en `selfhost/ESTADO.md`.

TITAN implements TLS 1.2/1.3 itself, in Titan (`selfhost/native/std_tls.titan`), with its own X.509 validation. TLS streams are handles managed by the runtime. There is no capability system in native executables: any program can open connections and read credential files.

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

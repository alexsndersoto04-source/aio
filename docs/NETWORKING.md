# TITAN Networking

TITAN exposes real TCP sockets as VM-managed handles guarded by the Network runtime capability.

```titan
let listener = std::net::tcp_listen("127.0.0.1:8080")
let local = std::net::tcp_local_addr(listener)

loop {
    let accepted = std::net::tcp_accept(listener)
    let stream = accepted[0]
    let peer = accepted[1]

    spawn || {
        std::net::tcp_set_timeout(stream, 5000)
        let request = std::net::tcp_read(stream, 65536)
        std::net::tcp_write(stream, request)
        std::net::tcp_close(stream)
    }
}
```

Implemented operations:

- bind listeners, including ephemeral port `:0`;
- query the actual local address;
- blocking accept with peer address;
- DNS/address resolution through `TcpStream::connect`;
- bounded binary reads (hard capped at 16 MiB per operation);
- complete writes via `write_all`;
- read/write timeouts;
- deterministic close of streams/listeners;
- shared handles usable by spawned tasks;
- typed errors and sandbox denial.

The runtime registry stores listeners in `Arc<TcpListener>` and streams in `Arc<Mutex<TcpStream>>`, avoiding holding the global registry lock during blocking I/O. This layer is the transport foundation for HTTP, TLS and WebSockets; those protocols are not claimed by the raw socket API.

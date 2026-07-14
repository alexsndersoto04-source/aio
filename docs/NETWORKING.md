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

The runtime registry stores listeners in `Arc<TcpListener>` and streams in `Arc<Mutex<TcpStream>>`, avoiding holding the global registry lock during blocking I/O.

## HTTP/1.1 codec

`std::http::parse_request(bytes)` incrementally parses one request and returns `Option::Some(request)` only when headers and the declared body are complete. The request map includes method, target, path, query, version, normalized headers, bytes body, keep-alive and consumed byte count. Remaining bytes can be retained for pipelined requests.

`std::http::build_response(status, headers, body, keep_alive)` generates a complete response with canonical status reason, trusted `Content-Length`, controlled `Connection` and CR/LF injection rejection.

The codec enforces request-line/header/body/count limits, exactly one HTTP/1.1 Host header, consistent duplicate Content-Length, no obsolete folding, and rejects Transfer-Encoding to prevent CL/TE request-smuggling ambiguity.

## Routing and query data

`std::http::route_match(pattern, path)` supports static segments, named `:parameters`, and a final `*wildcard`. It returns `Option::Some(params)` or `Option::None`; duplicate/empty parameters and non-final wildcards are rejected. Captured values use strict UTF-8 percent-decoding.

`std::http::parse_query(query, max_pairs)` preserves repeated keys as arrays, decodes `%XX` and form-style `+`, and enforces an explicit pair limit.

## High-level connection server

`std::http::serve_connection(listener, handler, max_requests)` accepts one TCP connection, incrementally buffers requests, invokes a TITAN closure for each request, writes validated responses, and honors keep-alive up to the configured request limit. Run it inside `spawn` for concurrent connections.

The handler receives a map containing method, target, path, query, version, headers, body, keep-alive and peer. It returns a map with status, headers, body and optional keep-alive. The server enforces a 10,000-request maximum per connection and a bounded connection buffer. Handler/runtime errors close that task and propagate through `join`.

## Composed router and middleware

`std::http::router()` creates a VM-managed router. `route(router, method, pattern, handler)` registers ordered handlers; `middleware(router, closure)` registers request-transforming closures; and `dispatch(router, request)` runs middleware, matches method/path, injects `request.params`, and invokes the handler. Missing routes produce a safe 404 response map.

Routers and their captured closures are shared safely across server tasks. Registration validates patterns before accepting them, dispatch releases router locks before executing user code, and middleware must return a request map. `after(router, closure)` adds response middleware executed in reverse registration order.

Production helpers include `security_headers(response)` (nosniff, frame denial, strict referrer policy and CSP), validated `cors(response, origin, methods)`, monotonic process-wide `request_id(request)`, and stateful fixed-window `rate_limit(key, maximum, window_ms)`. Header helpers reject CR/LF injection, and rate-limit state is mutex protected.

`on_error(router, |request, error| response)` recovers handler failures into HTTP responses while preserving structured error kind/message; failures in the recovery closure still propagate. `json_response(status, value)` and `error_response(status, message)` generate UTF-8 JSON response maps suitable for handlers. Response middleware still runs on recovered responses.

TLS remains the next transport layer.

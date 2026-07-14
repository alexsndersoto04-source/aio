# TITAN HTTP/HTTPS Client

`std::http::request(method, url, headers, body, maximum_body, redirects, timeout_ms)` performs a bounded blocking HTTP/1.1 request over TCP or validated rustls TLS and returns a map with `status`, normalized `headers`, binary `body`, and `final_url`.

The client validates URL scheme/host/port, methods and header tokens, prevents caller overrides of framing headers, writes exact Content-Length, applies read/write timeouts, caps headers at 64 KiB and body at the caller limit, follows a bounded number of absolute or relative redirects, and applies GET semantics to 303.

Response framing supports strict Content-Length, chunked transfer with extensions, and connection-close bodies. Conflicting lengths, CL/TE ambiguity, unsupported transfer codings, malformed chunks, oversized bodies, invalid redirects and incomplete responses are errors. HTTPS uses WebPKI chain/hostname validation and SNI through `titan_tls`.

Example:

```titan
let response = std::http::request(
    "GET",
    "https://example.com/api",
    std::json::parse("{\"Accept\":\"application/json\"}"),
    std::encoding::utf8_encode(""),
    1048576,
    5,
    10000
)
```

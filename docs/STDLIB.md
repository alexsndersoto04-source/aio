# Titan Standard Library Host API

The `titan_stdlib` crate provides memory-safe host capabilities for the compiler, VM embedders, and future native builtin bridge. APIs return `Result`/`Option` where operations can fail; malformed input is not silently accepted.

> The native bridge is active. Registered functions are called from `.titan` with qualified names such as `std::text::reverse("Titan")`. The shared registry currently contains 115 functions; the type checker validates their arity/types, codegen emits `CallNative`, and the VM converts values and returns structured errors.

## Modules

| Module | Capabilities |
|---|---|
| `bytes` | Bounds-checked endian binary reader/writer, numeric and length-prefixed string formats |
| `cache` | Capacity-bounded LRU cache with deterministic eviction |
| `checksum` | FNV-1a, CRC-32, constant-time byte comparison |
| `collections` | Vec/map/set/deque/heap aliases; deduplication, frequency, grouping, partitioning, chunks, windows, zip, search |
| `csv` | Quoted CSV parser, serializer and header-based table access |
| `encoding` | Strict hex, Base64 and UTF-8 percent encoding/decoding |
| `io` | Text/bytes, bounded reads, lines, append, atomic writes, sorted directory listing and depth-limited walking |
| `http` | Incremental HTTP/1.1 request parsing, anti-smuggling validation, keep-alive metadata and safe response construction |
| `json` | Parsing, compact/pretty output, JSON Pointer, path queries, merge patch behavior and flattening |
| `math` | Common floating-point functions and generic min/max |
| `net` | TCP client/server and parsed plaintext HTTP/1.1 responses |
| `path` | Join, components, normalization, absolute/canonical paths and containment checks |
| `process` | Shell-free command construction, environment/cwd, output capture and timeout with concurrent pipe draining |
| `stats` | Welford streaming mean/variance, standard deviation, min/max, median and quantiles |
| `sync` | Bounded thread pool, panic isolation, graceful join, channels and poison-aware shared state |
| `testing` | Assertions for host-side library tests |
| `text` | Unicode-scalar length/reverse, codepoints, truncation, padding, HTML escaping, slugification and Levenshtein distance |
| `time` | Unix timestamps, checked duration construction, stopwatch and monotonic deadlines |

## Security boundaries

- `checksum` is **not cryptography**. It must not be used for passwords, authentication or signatures.
- HTTPS is rejected by `net::http_get`; the library never pretends that plaintext TCP is TLS.
- `process::CommandSpec` invokes a program directly and never passes input through a shell, reducing command-injection risk.
- `io::read_limited` and VM range/instruction limits exist to bound untrusted input.
- `path::is_within` canonicalizes both paths and requires them to exist; lexical normalization alone is not a sandbox.
- Text operations count Unicode scalar values, not user-perceived grapheme clusters. The API states this explicitly.

## Examples (Rust host)

```rust
use titan_stdlib::{csv, encoding, stats};

let rows = csv::parse("name,score\nAda,10")?;
let token = encoding::base64_encode(b"Titan");

let mut summary = stats::Summary::new();
summary.extend([10.0, 20.0, 30.0]);
assert_eq!(summary.mean(), Some(20.0));
```

```rust
use std::time::Duration;
use titan_stdlib::process::CommandSpec;

let output = CommandSpec::new("git")
    .args(["--version"])
    .output_timeout(Duration::from_secs(2))?;
```

## Calling from `.titan`

```titan
fn main() {
    let encoded = std::encoding::base64_encode(
        std::encoding::utf8_encode("Titan")
    )
    let document = std::json::parse("{\"answer\":42}")
    print(std::text::uppercase(encoded))
    print(document.answer)
}
```

Native results include regular values plus VM `bytes` and `map` values. Map entries can be read with field syntax (`result.stdout`, `document.name`). Bytes can be created with `std::encoding::utf8_encode`, decoded from hex/Base64, or read from files/network.

### Effects and capabilities

Pure functions need no capability. The registry marks effectful calls as one of:

- `Filesystem`: `std::fs::*` and canonical/absolute path operations;
- `Process`: `std::process::*`;
- `Network`: `std::net::*`;
- `Environment`: `std::env::*`.

`Vm::new` enables standard desktop capabilities so CLI programs can use the complete library. Embedders can use `Vm::sandboxed` or `with_capabilities` to deny effects. A denied call returns `PermissionDenied`; it is never silently executed.

The authoritative metadata is `titan_stdlib::native::NATIVES`. Every registered name has a VM dispatch implementation, and registry/dispatch parity is checked during development.

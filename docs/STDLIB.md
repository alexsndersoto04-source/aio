# Titan Standard Library Host API

The `titan_stdlib` crate provides memory-safe host capabilities for the compiler, VM embedders, and future native builtin bridge. APIs return `Result`/`Option` where operations can fail; malformed input is not silently accepted.

> Current boundary: these Rust modules are implemented as host APIs. Only `print`, `println`, and `len` are directly emitted as VM intrinsics today. Exposing every module to `.titan` source requires the planned native-function registry and type signatures.

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

## Planned `.titan` bridge

The next integration layer should expose these modules through a native registry rather than hard-coding every function in the VM. Each native entry needs:

1. module-qualified name;
2. Titan function signature;
3. arity/type validation;
4. conversion between VM and host values;
5. structured errors;
6. capability policy for filesystem, process and network access;
7. deterministic tests from `.titan` source.

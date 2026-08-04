# Titan Standard Library Host API

The `titan_stdlib` crate provides memory-safe host capabilities for the compiler, VM embedders, and future native builtin bridge. APIs return `Result`/`Option` where operations can fail; malformed input is not silently accepted.

> The native bridge is active. Registered functions are called from `.titan` with qualified names such as `std::text::reverse("Titan")`. The shared registry currently contains 694 functions; the type checker validates their arity/types, codegen emits `CallNative`, and the VM converts values and returns structured errors.

## Modules

| Module | Capabilities |
|---|---|
| `bytes` | Bounds-checked endian binary reader/writer, numeric and length-prefixed string formats |
| `cache` | Capacity-bounded LRU cache with deterministic eviction |
| `checksum` | FNV-1a, CRC-32, constant-time byte comparison |
| `clipboard` | **Real** system clipboard via `termux-clipboard-set/get` (Termux), `wl-copy`/`wl-paste` (Wayland), `xclip`/`xsel` (X11), `pbcopy`/`pbpaste` (macOS), `clip`/PowerShell (Windows). Typed error when no backend exists — never a fake in-memory copy. Also hosts `std::notify` (real notifications via `termux-notification`, `notify-send`, `osascript`) |
| `collections` | Vec/map/set/deque/heap aliases; deduplication, frequency, grouping, partitioning, chunks, windows, zip, search |
| `csv` | Quoted CSV parser, serializer and header-based table access |
| `encoding` | Strict hex, Base64 and UTF-8 percent encoding/decoding |
| `freestanding` | **Real** bare-metal *build helpers*: validates target triples and generates GNU ld linker scripts + assembly `_start` stubs (aarch64/x86_64/riscv64-unknown-none). Deliberately **not** a kernel/hardware simulation — the fake exception/MMIO/frame-allocator "bare-metal" was removed in Phase 41 |
| `game` | Headless 2D frame loop with measured delta-time/FPS and AABB collision detection (Fase 1 graduated) |
| `gui` | Retained-mode widget tree: containers, labels, buttons, text, click state and child traversal (Fase 2 graduated) |
| `gui_raster` | Pure-Rust software rasterizer rendering the `gui` tree to RGBA buffers and PNG (Fase 2 graduated) |
| `input` | Real keyboard/mouse/multi-touch hardware state for games and GUI (Fase 1 graduated) |
| `io` | Text/bytes, bounded reads, lines, append, atomic writes, sorted directory listing and depth-limited walking |
| `http` | Incremental HTTP/1.1 request parsing, anti-smuggling validation, keep-alive metadata and safe response construction |
| `http_client` | Bounded HTTP/HTTPS requests, redirects, chunked transfer, timeouts and WebPKI validation |
| `json` | Parsing, compact/pretty output, JSON Pointer, path queries, merge patch behavior and flattening |
| `math` | Common floating-point functions and generic min/max |
| `metrics` | Thread-safe counters, gauges, aggregate histograms and snapshots |
| `mobile` | Android-style app lifecycle state machine: foreground/background, pause/resume (Fase 1 graduated) |
| `multipart` | Bounded multipart/form-data parsing with safe upload metadata |
| `net` | TCP client/server and parsed plaintext HTTP/1.1 responses |
| `path` | Join, components, normalization, absolute/canonical paths and containment checks |
| `process` | Shell-free command construction, environment/cwd, output capture and timeout with concurrent pipe draining |
| `stats` | Welford streaming mean/variance, standard deviation, min/max, median and quantiles |
| `sync` | Bounded thread pool, panic isolation, graceful join, channels and poison-aware shared state |
| `testing` | Assertions for host-side library tests |
| `text` | Unicode-scalar length/reverse, codepoints, truncation, padding, HTML escaping, slugification and Levenshtein distance |
| `time` | Unix timestamps, checked duration construction, stopwatch and monotonic deadlines |
| `window` | Logical window abstraction with a typed event queue and shared event formatting (Fase 2 graduated) |
| `window_live` | Real OS windows at 60 fps via pure-Rust minifb (X11/Wayland/Win32/Cocoa), bridging real input into `std::input`; on headless boxes it honestly reports `-1`. **Fase 2 graduated 2026-07-31**: first live window ran 3,601 frames on a real 32-bit Android phone (armv7l, proot + Termux:X11) and closed cleanly |
| `websocket` | RFC 6455 handshake, secure masking, incremental frame codec and protocol validation |

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

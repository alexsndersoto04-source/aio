# TITAN / Zett

[![cross-platform CI](https://github.com/alexsndersoto04-source/aio/actions/workflows/cross-platform.yml/badge.svg)](https://github.com/alexsndersoto04-source/aio/actions/workflows/cross-platform.yml)

TITAN is a small, statically checked programming language implemented in Rust. Source files use the **`.titan`** extension and run on Titan's safe stack-based bytecode VM. On Termux, the compiler ships as the **`zett`** binary.

> **Project status.** Version **0.34.0**. The core (lexer → parser → typechecker → HIR → bytecode codegen → VM) compiles and runs end-to-end, and the full **438-test** suite passes on real **Ubuntu, Windows and macOS** runners plus **Android (Termux)** before every release (see [Releases](https://github.com/alexsndersoto04-source/aio/releases)). There is a real WebAssembly backend (`zett wasm`), and the standard library spans **72 `std::*` namespaces with 694 registered native functions**, covering regex, hashing, cryptography, HTTPS, DNS, SMTP email, JWT, YAML/XML, gzip/zstd, tar/zip, **terminal/TUI** (colors, cursor, keys, animated bars, readline with history), **image processing** (PNG/JPEG/WebP/BMP/GIF), **QR codes** (ASCII/Unicode/SVG/PNG), **system info** (CPU %, memory, load average, processes, disks, networks), **file-system watcher** (inotify), **Unix signals**, **audio** (real WAV I/O and synthesis + playback/recording via Termux:API), a headless **2D game engine** (real delta-time frame loop with measured FPS + AABB collision), **hardware input state** (keyboard / mouse / multi-touch) and an Android-style **app lifecycle** state machine, a retained-mode **GUI toolkit** (`std::gui`: containers, labels, buttons) drawn by a pure-Rust **software rasterizer**, plus **real OS windows** (`std::window::live_*`, pure-Rust minifb: X11/Wayland/Win32/Cocoa) pumping at 60 fps with the machine's real keyboard/mouse bridged into `std::input` — **Fase 2 graduated 2026-07-31**, when the first live TITAN window ever existed ran 3,601 frames on a real 32-bit Android phone (armv7l) through proot-distro + Termux:X11 and closed cleanly; on headless boxes it honestly reports `-1` instead of pretending a window exists, **NoSQL storage** (embedded ACID key-value store via sled + blocking Redis client), a pure-Rust **HTTP/1.1 web server** with a radix-tree **URL router** (tiny_http + matchit, the same router axum uses) supporting named / catch-all path parameters, JSON responses and RFC 6455 WebSocket upgrades, **SVG charts** (line / multi-line / bar / scatter / histogram via plotters, no C-deps), **HuggingFace tokenizers** (BPE / WordPiece / Unigram via the official `tokenizers` crate in pure-Rust mode), **on-device ONNX inference** (via `tract-onnx`, Sonos' production Rust inference engine — load `.onnx` models and run them entirely on the phone's CPU, no CUDA / cuDNN / BLAS / ONNX Runtime C++), and — uniquely — **direct access to Android hardware** via Termux:API (battery, GPS, sensors, camera, SMS, clipboard, vibrate, notifications, TTS).


## Install

Prebuilt binaries ship on the [**Releases** page](https://github.com/alexsndersoto04-source/aio/releases/latest) for Linux, macOS and Windows; Android/Termux installs from our own APT repo.

### 🐧 Linux (x86-64)

```bash
curl -L https://github.com/alexsndersoto04-source/aio/releases/download/v0.34.0/zett-linux-x86_64.tar.gz | tar xz
./zett version
```

### 🍎 macOS (Apple Silicon)

```bash
curl -L https://github.com/alexsndersoto04-source/aio/releases/download/v0.34.0/zett-macos-arm64.tar.gz | tar xz
xattr -d com.apple.quarantine zett 2>/dev/null; true   # unsigned binary: clear the quarantine flag once
./zett version
```

### 🪟 Windows (x86-64)

Download `zett-windows-x86_64.zip` from [Releases](https://github.com/alexsndersoto04-source/aio/releases/latest), unzip it, then in PowerShell:

```powershell
.\zett.exe version
```

### 🤖 Android / Termux (one-liner via our APT repo)

```bash
echo 'deb [trusted=yes] https://raw.githubusercontent.com/alexsndersoto04-source/aio/zett-repo ./ ' \
  > $PREFIX/etc/apt/sources.list.d/zett.list
pkg update && pkg install zett
zett --help
```

Optional: for the Android integrations (`std::termux::*`), also install
the Termux:API app from F-Droid and:

```bash
pkg install termux-api
```

### Quick smoke test (any OS)

```bash
echo 'fn main() { let n = std::procfs::cpu_count() print("TITAN sees {n} CPUs here") }' > hi.titan
zett run hi.titan        # on Linux/macOS use ./zett
```

## Build from source

Prerequisite: current stable Rust from <https://rustup.rs>.

```bash
git clone https://github.com/alexsndersoto04-source/aio
cd aio
cargo build --release -p titan_cli
target/release/titan run examples/hello.titan
target/release/titan run examples/fibonacci.titan
target/release/titan run examples/extras.titan     # Phase 1
target/release/titan run examples/formats.titan    # Phase 2
target/release/titan run examples/network.titan    # Phase 3 (needs internet)
target/release/titan run examples/security.titan   # Phase 4
target/release/titan run examples/android.titan    # Phase 5 (needs Termux:API)
target/release/titan run examples/tui.titan        # Phase 6
target/release/titan run examples/images.titan     # Phase 7 (images + QR)
target/release/titan run examples/system.titan     # Phase 8 (procfs / system info)
target/release/titan run examples/audio.titan      # Phase 9 (WAV synth + playback)
target/release/titan run examples/database.titan   # Phase 10 (embedded key-value)
target/release/titan run examples/webserver.titan  # Phase 11 (HTTP server + router)
target/release/titan run examples/charts.titan     # Phase 14 (SVG line/bar/scatter/histogram)
target/release/titan run examples/tokenizer.titan  # Phase 12 pt.1 (HuggingFace tokenizers)
target/release/titan run examples/onnx.titan       # Phase 12 pt.2 (ONNX inference, MNIST)
target/release/titan run examples/sentiment.titan  # Phase 12 pt.3 (DistilBERT sentiment)
target/release/titan run examples/wifi.titan       # Phase 13' (Wi-Fi scanning via Termux:API)
target/release/titan run examples/vector_search.titan  # Phase 12 pt.4 (vector math demo, runs on any device)
target/release/titan run examples/search.titan         # Phase 12 pt.4 full pipeline (needs 4+ GB RAM)
target/release/titan run examples/invoice.titan        # Phase 16 (PDF invoice generation)
target/release/titan run examples/game_engine.titan      # Fase 1 (headless game loop, collisions, input)
target/release/titan run examples/mobile_lifecycle.titan # Fase 1 (Android-style lifecycle machine)
target/release/titan run examples/gui_screenshot.titan   # Fase 2 pt.1 (GUI tree + software raster -> PNG)
target/release/titan run examples/gui_live_window.titan  # Fase 2 pt.2 (REAL live OS window @60fps; needs a display, or proot+Termux:X11 on a phone)
```

Install the CLI locally:

```bash
cargo install --path crates/titan_cli

titan new hello
cd hello
titan check
titan run
titan test
titan build                             # writes target/hello.tbc
titan repl
```

Single-file programs remain supported with `titan run examples/hello.titan`. Projects use `Titan.toml`, recursive source imports, deterministic local path dependencies and `Titan.lock`; see [`docs/PROJECTS.md`](docs/PROJECTS.md).

## The language

```titan
fn factorial(n: int) -> int {
    if n <= 1 { return 1 }
    n * factorial(n - 1)
}

fn main() {
    let total = 0
    for i in 1..=5 {
        total += factorial(i)
    }
    print("total = {total}")
}
```

### Supported end-to-end

- `int`, `float`, `bool`, `char`, `string`, `nil`, arrays and tuples.
- Typed functions, default parameter syntax, recursion and checked arity.
- First-class named functions and closures with deterministic lexical capture.
- Functional array pipelines through `map`, `filter`, and `fold` (function or method syntax).
- Built-in `Option::Some`/`None`, `Result::Ok`/`Err`, and propagating `?` execution.
- Lexical variables, assignment and compound arithmetic assignment.
- Arithmetic, comparison, logical and integer bitwise operators.
- `if`/`else`, `while`, `loop`, `for` over arrays/ranges, `break`, `continue`, `return`.
- Struct construction and field reads.
- Enums with zero or one payload and matching enum variants.
- Literal, binding and wildcard `match` arms; boolean exhaustiveness checking.
- Constants and nested module declarations.
- String interpolation for variables and simple named calls, such as `{fib(i)}`.
- Runtime errors for bad types, divide-by-zero, overflow, bounds, runaway execution and excessive recursion.
- A CLI with `run`, `build`, `repl`, and `version`.
- Editor diagnostics through the `titan_lsp` language-service crate.

### Parsed/checked language surface

Traits, impl blocks, references, slices, generic type syntax, `spawn` and advanced destructuring exist in the AST or parser. Some require runtime/linker work and produce an explicit “unsupported construct” error instead of silently generating incorrect code. See [the specification](docs/SPEC.md) for exact status.

## Commands

```text
titan new <directory>        Create Titan.toml and src/main.titan
titan add <name> <range>     Add remote dependency
titan fetch [--offline]      Resolve/install dependencies
titan update                 Re-resolve remote dependencies
titan keygen <path>          Generate package signing key
titan pack --key K --output P  Build signed .tpkg
titan publish --key K        Upload signed package over HTTPS
titan check [file|project]   Resolve imports and type-check
titan run [file|project]     Compile, type-check and execute
titan run --sandbox [path]   Deny filesystem/process/network/environment
titan build [file|project]   Write validated .tbc bytecode
titan wasm [file|project]    Compile supported code to .wasm
titan debug [path] -b file:line  Interactive source debugger
titan exec <file.tbc>        Validate and execute bytecode without source
titan test [project]         Run all tests/*.titan programs
titan repl                   Interactive expressions/statements
titan version                Print compiler version
```

The build artifact uses a portable, versioned JSON bytecode container with a magic header and CRC-32 integrity check. `titan exec` loads it without source compilation and rejects incompatible versions, corruption, invalid jumps/locals/functions, unknown natives, wrong arity, excessive metadata and unsafe references before VM execution.

## Architecture

```text
.titan source
  → titan_lexer
  → titan_parser / titan_ast
  → titan_typechecker
  → titan_codegen
  → titan_vm
```

Additional crates provide HIR/MIR data structures, tracing GC metadata, scheduling, package manifests, standard-library host functions, macros, and editor services. They are kept separate so the executable core does not depend on unfinished optimization passes.

The standard library includes checked binary I/O, LRU caching, collections algorithms, CSV, strict hex/Base64/percent encoding, bounded and atomic filesystem operations, JSON querying/merge, paths, process execution with timeouts, streaming statistics, Unicode-scalar text operations, clocks/deadlines and checksums. A shared native registry exposes 694 functions directly to `.titan`; effectful calls are controlled by VM capabilities. See [`docs/STDLIB.md`](docs/STDLIB.md).

Advanced subsystems are documented separately instead of overcrowding this overview: [projects/packages](docs/PROJECTS.md), [validated bytecode/debug source maps](docs/DEBUGGER.md), [LSP](docs/LSP.md), [DAP](docs/DAP.md), [threaded tasks and channels](docs/CONCURRENCY.md), and [TCP/HTTP networking](docs/NETWORKING.md), and [TLS](docs/TLS.md), and [WebSockets](docs/WEBSOCKET.md), and the [HTTP/HTTPS client](docs/HTTP_CLIENT.md), and [multipart uploads](docs/MULTIPART.md), and [metrics](docs/METRICS.md), and [server lifecycle/backpressure](docs/SERVER_LIFECYCLE.md), and [SQLite](docs/SQLITE.md), and [PostgreSQL](docs/POSTGRESQL.md), and [MySQL](docs/MYSQL.md), and the [common database API](docs/DATABASE_API.md), and [remote registry](docs/PACKAGE_REGISTRY.md), and [WebAssembly](docs/WASM.md).

## Development quality gates

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

The repository includes tests for malformed lexer input, parsing, semantic errors, recursion, loops/ranges, VM runtime failures, GC tracing and editor diagnostics.

## Safety and limits

The VM does not execute native pointers or Rust `unsafe`. It enforces instruction, recursion and range-allocation limits. Titan's dependency-free HTTP helper supports plain `http://` and rejects `https://` rather than sending insecure plaintext; use a TLS-enabled host integration where HTTPS is required.

## License

MIT. See [LICENSE](LICENSE).

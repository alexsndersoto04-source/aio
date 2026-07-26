# TITAN / Zett

TITAN is a small, statically checked programming language implemented in Rust. Source files use the **`.titan`** extension and run on Titan's safe stack-based bytecode VM. On Termux, the compiler ships as the **`zett`** binary.

> **Project status.** Version **0.7.0**. The core (lexer → parser → typechecker → HIR → bytecode codegen → VM) compiles and runs end-to-end. There is a real WebAssembly backend (`zett wasm`), and the standard library has grown to **27 optional modules** covering regex, hashing, cryptography, HTTPS, DNS, SMTP email, JWT, YAML/XML, gzip/zstd, tar/zip, **terminal/TUI** (colors, cursor, keys, animated bars, readline with history), **image processing** (PNG/JPEG/WebP/BMP/GIF), **QR codes** (ASCII/Unicode/SVG/PNG), **system info** (CPU %, memory, load average, processes, disks, networks), **file-system watcher** (inotify), **Unix signals**, **audio** (real WAV I/O and synthesis + playback/recording via Termux:API), and — uniquely — **direct access to Android hardware** via Termux:API (battery, GPS, sensors, camera, SMS, clipboard, vibrate, notifications, TTS).
>
> **Not yet functional (experimental scaffolding).** The `titan_mir` crate and the `zett native` / `zett mobile` CLI commands are placeholders: `lower_hir_to_mir` is a no-op, and the ELF/APK writers emit an incomplete header. Both commands print a warning at runtime. Use `zett build` (portable bytecode) or `zett wasm` (WebAssembly) for real artifacts.

## Install on Termux (one-liner)

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
titan native [file|project]  EXPERIMENTAL — emits an incomplete ELF stub (not loadable)
titan mobile [file|project]  EXPERIMENTAL — does NOT produce a real Android APK
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

The standard library includes checked binary I/O, LRU caching, collections algorithms, CSV, strict hex/Base64/percent encoding, bounded and atomic filesystem operations, JSON querying/merge, paths, process execution with timeouts, streaming statistics, Unicode-scalar text operations, clocks/deadlines and checksums. A shared native registry exposes 128 functions directly to `.titan`; effectful calls are controlled by VM capabilities. See [`docs/STDLIB.md`](docs/STDLIB.md).

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

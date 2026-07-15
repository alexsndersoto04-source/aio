# TITAN

TITAN is a small, statically checked programming language implemented in Rust. Source files use the **`.titan`** extension and run on Titan's safe stack-based bytecode VM.

> Project status: the supported core compiles and runs end to end. On Termux ARM with Rust 1.96.1, all 19 crates pass `cargo check`, Clippy passes with `-D warnings`, and the growing suite contains 116 passing tests across compiler, VM, tooling, concurrency, artifacts and TCP. See [`docs/VALIDATION.md`](docs/VALIDATION.md). Experimental syntax is identified separately rather than advertised as complete.

## Quick start

Prerequisite: current stable Rust from <https://rustup.rs>.

```bash
git clone https://github.com/alexsndersoto04-source/aio
cd aio
cargo test --workspace --all-targets
cargo run -p titan_cli -- run examples/hello.titan
cargo run -p titan_cli -- run examples/fibonacci.titan
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
titan check [file|project]   Resolve imports and type-check
titan run [file|project]     Compile, type-check and execute
titan run --sandbox [path]   Deny filesystem/process/network/environment
titan build [file|project]   Write validated .tbc bytecode
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

Advanced subsystems are documented separately instead of overcrowding this overview: [projects/packages](docs/PROJECTS.md), [validated bytecode/debug source maps](docs/DEBUGGER.md), [LSP](docs/LSP.md), [DAP](docs/DAP.md), [threaded tasks and channels](docs/CONCURRENCY.md), and [TCP/HTTP networking](docs/NETWORKING.md), and [TLS](docs/TLS.md), and [WebSockets](docs/WEBSOCKET.md), and the [HTTP/HTTPS client](docs/HTTP_CLIENT.md), and [multipart uploads](docs/MULTIPART.md), and [metrics](docs/METRICS.md), and [server lifecycle/backpressure](docs/SERVER_LIFECYCLE.md), and [SQLite](docs/SQLITE.md), and [PostgreSQL](docs/POSTGRESQL.md).

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

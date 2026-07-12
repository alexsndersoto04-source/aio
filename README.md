# TITAN

TITAN is a small, statically checked programming language implemented in Rust. Source files use the **`.titan`** extension and run on Titan's safe stack-based bytecode VM.

> Project status: active language implementation. The supported core below is executable end to end; experimental syntax is identified separately rather than advertised as complete.

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

titan run examples/hello.titan
titan build examples/hello.titan       # writes hello.tbc
titan repl
```

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

Traits, impl blocks, imports, references, slices, generic type syntax, `spawn`, closures and advanced destructuring exist in the AST or parser. Some require runtime/linker work and produce an explicit “unsupported construct” error instead of silently generating incorrect code. See [the specification](docs/SPEC.md) for exact status.

## Commands

```text
titan run <file.titan>       Compile, type-check and execute
titan run --sandbox <file>   Execute while denying filesystem/process/network/environment
titan build <file.titan>     Write inspectable .tbc bytecode
titan repl                   Interactive expressions/statements
titan version                Print compiler version
```

The build artifact currently uses a versioned, inspectable textual bytecode format. `run` compiles from source; loading precompiled artifacts is planned for the binary bytecode container.

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

The standard library includes checked binary I/O, LRU caching, collections algorithms, CSV, strict hex/Base64/percent encoding, bounded and atomic filesystem operations, JSON querying/merge, paths, process execution with timeouts, streaming statistics, worker pools/channels, Unicode-scalar text operations, clocks/deadlines, checksums, TCP and HTTP/1.1. A shared native registry exposes 104 functions directly to `.titan` through names such as `std::text::reverse`, `std::json::parse` and `std::fs::read_text`; effectful calls are controlled by VM capabilities. See [`docs/STDLIB.md`](docs/STDLIB.md).

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

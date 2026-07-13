# Validation Record

## Termux ARM validation — 12 July 2026

TITAN 0.2 was compiled and exercised from commit `1d336f0` and its preceding fixes on Android/Termux.

### Environment

```text
rustc 1.96.1 (31fca3adb 2026-06-26)
cargo 1.96.1 (356927216 2026-06-26)
git 2.55.0
clang 21.1.8-3
Target environment: Termux, ARM Android
```

### Workspace compilation

```bash
cargo check --workspace --all-targets
```

Result: success for all 15 workspace crates and all targets.

### Static quality gate

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Result: success, zero Clippy warnings.

### Automated tests

```bash
cargo test --workspace --all-targets
```

Result: **53 passed, 0 failed, 0 ignored**.

Verified test areas include:

- Unicode lexing, escapes, invalid input and ranges;
- parsing functions, declarations, match, closures, `?`, JSON braces and malformed programs;
- recursive imports, import-cycle detection and local path dependencies;
- type errors, recursive signatures, native signatures and generic native arrays;
- arithmetic, recursion, loops, structs, enums, closures, first-class functions, functional array pipelines, JSON maps, interpolation, native encoding/statistics, sandbox permissions, runtime errors and `Result` propagation;
- binary readers/writers, LRU cache, checksums, collections, CSV, strict encodings, JSON merge/query, paths, process capture, statistics, text, synchronization and time;
- GC transitive tracing, editor diagnostics and scheduler FIFO/parent validation.

### End-to-end language examples

```bash
cargo run -p titan_cli -- run examples/hello.titan
cargo run -p titan_cli -- run examples/fibonacci.titan
cargo run -p titan_cli -- run examples/stdlib.titan
```

Observed results:

- Hello World printed correctly.
- Fibonacci printed complete interpolated lines from `fib(0) = 0` through `fib(19) = 4181`.
- Standard-library example successfully exercised UTF-8, Base64, JSON map fields, statistics and slugification.

### Project workflow

The following commands were exercised successfully:

```bash
titan new <project>
titan check <project>
titan run <project>
titan build <project>
titan test <project>
```

Confirmed behavior:

- generated `Titan.toml` and `src/main.titan`;
- discovered and checked a project;
- executed its main function;
- wrote a physical `TITAN-BYTECODE 1` `.tbc` artifact;
- discovered and passed a `tests/arithmetic.titan` test;
- loaded a two-file project with `import math`, reporting 2 sources and 3 functions;
- executed imported `double` and `square` functions correctly.

## Scope of this validation

This record establishes compilation, lint cleanliness, automated tests and end-to-end operation on one real ARM/Termux environment. It does not by itself certify Windows, macOS, desktop Linux, iOS, browser/Wasm, native-code generation, security auditing or production suitability. Those require their own target-specific records.

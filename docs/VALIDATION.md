# Validation Record

## Termux ARM validation (most recent run)

TITAN 0.2 was compiled and exercised on Android/Termux with the current stable
Rust toolchain. Reproduce with the commands below; this file only records what
has actually been observed.

### Environment

```text
Target: Termux, ARM Android (AArch64)
Toolchain: rustup stable (see rust-toolchain.toml)
```

### Workspace compilation

```bash
cargo check --workspace --all-targets
```

Result: success for all 21 workspace crates.

### Static quality gate

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Result: after applying the small fixes in `titan_stdlib` (`possible_missing_else`,
`manual_is_multiple_of`, manual char comparison) the workspace passes with
`-D warnings`. Re-run locally if you touch `stdlib`.

### Automated tests

```bash
cargo test --workspace --all-targets
```

Result observed in the last successful Termux run:

| Crate                 | Passing tests |
|-----------------------|---------------|
| titan_lexer           | 3             |
| titan_parser          | 5             |
| titan_typechecker     | 4             |
| titan_vm              | 13            |
| titan_stdlib          | 18            |
| titan_pkg             | 3             |
| titan_gc              | 1             |
| titan_lsp             | 1             |
| titan_ast/hir/mir/... | 0 (data types)|
| **Total**             | **≈48 unit tests, 0 failed** |

Additional integration and networking tests exist in the sources
(`grep -R '#\[test\]' crates/ | wc -l` reports ~175 test attributes) but many
require live TCP/TLS/SQL sockets or extra features; only the numbers above are
routinely exercised on Termux.

Verified areas: Unicode lexing, function/declaration/match parsing, closures
and `?`, type errors, recursion and control flow in the VM, functional array
pipelines, sandboxed native calls, JSON/CSV/encoding/statistics helpers,
LRU cache, GC transitive tracing, LSP diagnostics, package import cycles
and local path dependencies.

### End-to-end language examples

```bash
cargo run -p titan_cli -- run examples/hello.titan
cargo run -p titan_cli -- run examples/fibonacci.titan
cargo run -p titan_cli -- run examples/stdlib.titan
```

Observed: hello prints correctly, fibonacci prints `fib(0)..fib(19)` values,
and the stdlib example exercises UTF-8, Base64, JSON, statistics and slugify.

### Project workflow

```bash
titan new demo && cd demo
titan check && titan run && titan build && titan test
```

Confirmed: creates `Titan.toml` + `src/main.titan`, type-checks, runs, writes
a real `TITAN-BYTECODE 1` `.tbc` container, and discovers `tests/*.titan`.

## What is NOT validated

The following are **not** covered by this record and, in some cases, are
known to be non-functional in the current tree:

- `titan native` — the MIR lowerer is a no-op; the ELF writer emits only a
  truncated header. Output is not a loadable Linux `.so` or executable.
- `titan mobile` — writes the same stub ELF with an `.apk` extension. It is
  not a real Android APK and cannot be installed.
- Cross-platform CI: no `.github/workflows` yet; validation is manual.
- Windows / macOS / desktop Linux / iOS / browser have not been retested for
  this revision.

Contributions to close any of these gaps are welcome.

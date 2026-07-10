# ⚔️ TITAN — The Executioner of Programming Languages

> *"The executioner does not ask permission. It executes."*

---

## 📊 Project Status

| | |
|---|---|
| **Crates** | 15 |
| **Rust Lines** | ~2,600 |
| **Compilation** | ✅ Verified (Termux, Rust 1.96) |
| **License** | MIT |

---

## 🧬 What is TITAN?

TITAN is a **systems programming language** that combines:

- **Rust-level safety** — no null, no undefined behavior, exhaustive pattern matching
- **Go-level simplicity** — clean syntax, fast compilation, minimal keywords
- **C-level performance** — AOT compilation, zero-cost abstractions, stack by default

Titan compiles to **bytecode** that runs on its own **stack-based VM**, with an optional **generational garbage collector**.

---

## 🏗️ Architecture

```
Source (.tt)
  → Lexer      (titan_lexer)
  → Parser     (titan_parser) — recursive descent + Pratt
  → Type Checker (titan_typechecker)
  → HIR        (titan_hir)
  → MIR + Opt  (titan_mir)
  → Codegen    (titan_codegen)
  → Bytecode
  → VM         (titan_vm) — stack-based interpreter
  → GC         (titan_gc) — generational mark-and-sweep
```

---

## 📦 15 Crates

| # | Crate | Purpose |
|---|-------|---------|
| 1 | `titan_lexer` | Tokenizer |
| 2 | `titan_ast` | Abstract Syntax Tree |
| 3 | `titan_parser` | Recursive descent + Pratt parser |
| 4 | `titan_typechecker` | Type inference & checking |
| 5 | `titan_hir` | High-level IR |
| 6 | `titan_mir` | Mid-level IR + optimizations |
| 7 | `titan_codegen` | Bytecode compiler |
| 8 | `titan_vm` | Stack-based virtual machine |
| 9 | `titan_gc` | Generational garbage collector |
| 10 | `titan_macros` | Macro expansion engine |
| 11 | `titan_runtime` | Fiber scheduler + channels |
| 12 | `titan_stdlib` | Standard library |
| 13 | `titan_cli` | CLI: build, run, repl, version |
| 14 | `titan_lsp` | Language Server Protocol |
| 15 | `titan_pkg` | Package manager |

---

## 📝 Quick Example

```titan
// Hello World in Titan
fn main() {
    let name = "Developer"
    print("Hello, {name} from TITAN!")
}
```

```titan
// Fibonacci
fn fib(n: int) -> int {
    if n <= 1 { return n }
    fib(n - 1) + fib(n - 2)
}

fn main() {
    for i in 0..20 {
        print("fib({i}) = {fib(i)}")
    }
}
```

---

## 🔨 How to Compile

### Prerequisites
- **Rust** (install from https://rustup.rs)

### Build
```bash
git clone https://github.com/alexsndersoto04-source/aio
cd aio
git checkout arena/019f4510-aio
cargo build --workspace --lib
```

### On Android (Termux)
```bash
pkg install rust git
git clone https://github.com/alexsndersoto04-source/aio
cd aio
git checkout arena/019f4510-aio
cargo build --workspace --lib
```

### On GitHub Codespaces
1. Open your repo on GitHub
2. Click **Code → Codespaces → Create codespace**
3. Run: `cargo build --workspace --lib`

---

## 📁 Project Structure

```
aio/
├── Cargo.toml              # Workspace root
├── LICENSE                 # MIT
├── README.md
├── crates/
│   ├── titan_lexer/        # Tokenizer
│   ├── titan_ast/          # AST types
│   ├── titan_parser/       # Parser
│   ├── titan_typechecker/  # Type system
│   ├── titan_hir/          # High IR
│   ├── titan_mir/          # Mid IR
│   ├── titan_codegen/      # Bytecode compiler
│   ├── titan_vm/           # Virtual machine
│   ├── titan_gc/           # Garbage collector
│   ├── titan_macros/       # Macro engine
│   ├── titan_runtime/      # Concurrency runtime
│   ├── titan_stdlib/       # Standard library
│   ├── titan_cli/          # Command-line interface
│   ├── titan_lsp/          # LSP server
│   └── titan_pkg/          # Package manager
```

---

## ⚡ Features

- Full type inference
- Algebraic data types (enums with payloads)
- Pattern matching (exhaustive by default)
- Zero-cost abstractions
- Generational garbage collector (optional)
- Fiber-based concurrency
- Channel communication (Go-style)
- Standard library: IO, networking, JSON, crypto, math
- Package manager with Titan.toml manifests
- Language Server Protocol for IDE integration
- Compiles on Linux, macOS, Android (Termux)

---

## 🎯 Language Level

TITAN is a **systems programming language** — same category as Rust, Go, Zig, and C. It is NOT a scripting language. It is designed for building:

- Web servers
- APIs and microservices
- CLI tools
- Game engines
- Databases
- Infrastructure software

---

## 📄 License

MIT — Free for everyone. Forever.

---

<div align="center">

**⚔️ TITAN — The Executioner**

*"All other languages fall."*

</div>

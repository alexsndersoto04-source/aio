# Titan Architecture

## Executable pipeline

```text
.titan source
  → titan_lexer          tokens, byte-accurate spans, lexical diagnostics
  → titan_parser         recursive descent + precedence parsing
  → titan_ast            declarations, statements, expressions and patterns
  → titan_typechecker    scopes, signatures, aggregate and control-flow checks
  → titan_codegen        AST to versioned stack bytecode
  → titan_vm             checked frame-based interpreter
```

For project commands, `titan_pkg::SourceProject` first discovers `Titan.toml`, resolves canonical local dependencies and recursively loads imports. It rejects missing files, path escapes, duplicate dependency traversal, import cycles and dependency cycles. The resulting unified `Program` enters the pipeline above. The CLI stops at the first failed phase and never executes a partial program.

## Workspace crates

| Crate | Responsibility | Pipeline status |
|---|---|---|
| `titan_lexer` | Unicode-aware tokenization | Connected |
| `titan_ast` | Source-level representation | Connected |
| `titan_parser` | Syntax and recovery diagnostics | Connected |
| `titan_typechecker` | Semantic/type checks | Connected |
| `titan_codegen` | Bytecode generation | Connected |
| `titan_vm` | Safe bytecode execution | Connected |
| `titan_hir` | Future desugared high IR | Library layer |
| `titan_mir` | Future optimization IR | Library layer |
| `titan_gc` | Tracing object graph metadata | Tested library; VM values currently use Rust ownership |
| `titan_macros` | Macro registry foundation | Experimental |
| `titan_runtime` | Fiber scheduling foundation | Experimental |
| `titan_stdlib` | Rust host helpers | Library layer |
| `titan_cli` | `build`, `run`, `repl`, `version` | Connected |
| `titan_lsp` | Document diagnostics core | Connected as a library |
| `titan_pkg` | Manifest and lockfile model | Local library layer |

HIR and MIR are deliberately not shown in the active path until their lowering passes preserve all executable semantics. Documentation must not claim an optimization stage is active merely because a crate with that name exists.

## VM model

Each call creates an isolated vector of locals and an operand stack. `Call` contains a resolved function index and argument count. Values include numbers, booleans, chars, strings, arrays, tuples, structs, enums and `nil`.

The VM reports errors for stack corruption, invalid locals/functions, arity, bad operand types, division by zero, overflow, bounds, missing fields, instruction exhaustion and excessive call depth.

## Native standard library

`titan_stdlib::native::NATIVES` is the authoritative registry of qualified names, parameter types, return types and required capabilities. The type checker consumes this metadata, codegen emits `CallNative`, and `titan_vm::native` dispatches the call. This avoids treating host calls as unresolved globals and keeps all compiler stages aligned.

The VM adds `Bytes` and `Map` runtime values for binary I/O, JSON, processes and HTTP. Effectful native calls are guarded by `RuntimeCapabilities`; embedders can construct a sandboxed VM while the CLI offers `titan run --sandbox`.

## Artifact model

`build` writes an inspectable `TITAN-BYTECODE 1` artifact. It is currently intended for diagnostics and tooling; source execution remains the stable interface. A future binary container must include format versioning, integrity validation and a loader before artifacts are advertised as independently executable.

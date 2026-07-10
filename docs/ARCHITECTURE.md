# Titan Architecture

## Pipeline

```
Source (.tt)
  -> Lexer (titan_lexer)
  -> Parser (titan_parser)
  -> Type Checker (titan_typechecker)
  -> HIR (titan_hir)
  -> MIR + Optimizations (titan_mir)
  -> Codegen (titan_codegen)
  -> Bytecode
  -> VM (titan_vm)
```

## 15 Crates

| Crate | Function |
|---|---|
| titan_lexer | Tokenizer |
| titan_ast | AST types |
| titan_parser | Parser |
| titan_typechecker | Type system |
| titan_hir | High IR |
| titan_mir | Mid IR |
| titan_codegen | Bytecode compiler |
| titan_vm | Virtual machine |
| titan_gc | GC |
| titan_macros | Macros |
| titan_runtime | Concurrency |
| titan_stdlib | Std lib |
| titan_cli | CLI |
| titan_lsp | LSP |
| titan_pkg | Package mgr |
# TITAN Language Server

`titan-lsp` is a real stdio Language Server Protocol process. It uses JSON-RPC 2.0 messages framed with `Content-Length`, caps inbound messages at 16 MiB, never writes logs to stdout, and flushes every response/notification.

Implemented protocol surface:

- `initialize`, `initialized`, `shutdown`, `exit`;
- full and incremental UTF-16 document synchronization;
- version ordering and stale-change rejection;
- `publishDiagnostics` after open/change and clearing after close;
- completion for keywords, workspace declarations and all registered natives;
- Markdown hover;
- go to definition;
- cross-document references;
- validated workspace rename;
- document symbols and workspace symbol search.

Positions are converted between UTF-8 byte offsets used by the compiler and UTF-16 code units required by LSP clients. Symbol/reference discovery is token based, so strings and comments are not renamed accidentally.

Run directly:

```bash
cargo run -p titan_lsp --bin titan-lsp
```

Editor configuration must launch `titan-lsp` as a subprocess and communicate over stdin/stdout. The server advertises its capabilities during `initialize`.

Semantic diagnostics for expressions, functions, declarations, members, parameters, type annotations, aliases, and trait implementations carry their real AST spans and are converted from compiler byte offsets to LSP UTF-16 ranges. The diagnostic API keeps spans optional only for synthetic ASTs that provide no originating source construct; in that defensive case the server falls back to the document origin rather than fabricating a location.

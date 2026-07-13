# TITAN Debug Adapter Protocol

`titan-dap` is a standalone Debug Adapter Protocol process using stdio and `Content-Length` framed JSON. It launches either a Titan project/source entry or a validated `.tbc` artifact and drives the real channel-based VM debugger.

Implemented requests: `initialize`, `launch`, `setBreakpoints`, `configurationDone`, `threads`, `stackTrace`, `scopes`, `variables`, `continue`, `next`, `stepIn`, `stepOut`, `pause`, `terminate`, and `disconnect`.

Implemented events: `initialized`, `stopped`, `continued`, `output`, and `terminated`. Program output is routed through a VM output channel and emitted as DAP `output` events, never mixed directly into protocol stdout.

Breakpoints are verified against canonical source paths and executable source-map lines before being accepted. Frames expose source path, line, column, function and call depth. Scopes expose locals/captures and the operand stack.

Launch configuration example:

```json
{
  "type": "titan",
  "request": "launch",
  "name": "Debug Titan project",
  "program": "${workspaceFolder}",
  "sandbox": false
}
```

Build and run the adapter:

```bash
cargo build -p titan_dap --bin titan-dap --release
./target/release/titan-dap
```

Editors must launch it as a subprocess and communicate only through stdin/stdout. Diagnostic logs go to stderr.

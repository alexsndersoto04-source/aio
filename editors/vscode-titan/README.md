# TITAN Language Tools for VS Code and code-server

Support for `.titan` files: highlighting, LSP diagnostics/completion/navigation, run/check/build/WASM/test commands, project creation, and DAP debugging.

## Browser editors

`vscode.dev` cannot start compiler processes on Android. Use **Termux + code-server** (or a remote environment such as Codespaces): Chrome is still the interface, while code-server can run TITAN. See [Android setup](../../docs/ANDROID_CODE_SERVER.md).

## Package

```sh
npm install
npm run package
code-server --install-extension titan-language-tools-0.1.0.vsix
```

The host environment must provide `titan` (or configure `titan.compiler.path` as `zett`), `titan-lsp`, and `titan-dap`. Change their paths in the TITAN settings.

**Build bytecode** produces portable `.tbc` files run through `titan exec`. **Build WebAssembly** produces deployable `.wasm` plus source maps. TITAN currently does not claim to emit standalone native `.apk`, `.exe`, or ELF binaries.

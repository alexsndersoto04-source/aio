#!/usr/bin/env bash
# Runs once when GitHub Codespaces creates the remote development machine.
set -euo pipefail

cd "${WORKSPACE_FOLDER:-$PWD}"
echo 'Building the complete TITAN developer toolchain (compiler, language server and debugger)...'
cargo build --release -p titan_cli -p titan_lsp -p titan_dap

mkdir -p "$HOME/.local/bin"
for program in titan titan-lsp titan-dap; do
    ln -sf "$PWD/target/release/$program" "$HOME/.local/bin/$program"
done

echo 'TITAN is ready:'
titan version
printf '\nInstall the bundled VS Code extension once from the Command Palette:\n'
printf 'Extensions: Install from VSIX → editors/vscode-titan/titan-language-tools-0.1.0.vsix\n'

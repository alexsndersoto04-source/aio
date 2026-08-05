#!/usr/bin/env bash
# Runs after the browser client is attached to a Codespace.  A VSIX is a ZIP
# package, not a source file; install it with the VS Code CLI instead of
# opening it in the editor.
set -euo pipefail

extension_id='titan-lang.titan-language-tools'
vsix="$PWD/editors/vscode-titan/titan-language-tools-0.1.0.vsix"

if ! command -v code >/dev/null 2>&1; then
    echo "VS Code CLI is not ready yet. Reopen the Codespace once; TITAN tools are already built."
    exit 0
fi

if code --list-extensions | grep -Fxq "$extension_id"; then
    echo 'TITAN VS Code extension is already installed.'
else
    echo 'Installing TITAN VS Code extension...'
    code --install-extension "$vsix" --force
fi

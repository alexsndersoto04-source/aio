#!/bin/bash
# Descarga el binario zett (linux x86_64) de la rama tools-zett-x86_64
# del propio repo (la lo publica la CI) y lo deja en projects/moon/bin/zett.
set -e
OPS="$(cd "$(dirname "$0")" && pwd)"
MOON="$OPS/.."
REPO_DIR="$(cd "$MOON/../.." && pwd)"
mkdir -p "$MOON/bin"
cd "$REPO_DIR"
git fetch -q origin 'refs/heads/tools-zett-x86_64:refs/remotes/origin/tools-zett-x86_64'
git show origin/tools-zett-x86_64:tools/zett-linux-x86_64 > "$MOON/bin/zett"
chmod +x "$MOON/bin/zett"
"$MOON/bin/zett" --version 2>/dev/null || true
echo "OK: $MOON/bin/zett"

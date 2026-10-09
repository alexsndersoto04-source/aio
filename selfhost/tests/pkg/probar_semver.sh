#!/bin/bash
# Compila y ejecuta semver_match.titan con el compilador dado (por defecto selfhost/titanc3).
set -e
AQUI="$(cd "$(dirname "$0")" && pwd)"
SELFHOST="$(cd "$AQUI/../.." && pwd)"
COMPILADOR="${COMPILER:-$SELFHOST/titanc3}"
T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT
cp "$SELFHOST/semver.titan" "$AQUI/semver_match.titan" "$T/"
"$COMPILADOR" "$T/semver_match.titan" "$T/semver_match"
"$T/semver_match"

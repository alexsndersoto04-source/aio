#!/data/data/com.termux/files/usr/bin/bash
set -euo pipefail

ZETT="${1:-zett}"

if [[ "$ZETT" == */* ]]; then
    if [[ ! -f "$ZETT" ]]; then
        echo "FAIL: Zett binary not found: $ZETT" >&2
        exit 1
    fi
    ZETT="$(cd -- "$(dirname -- "$ZETT")" && pwd)/$(basename -- "$ZETT")"
else
    ZETT="$(command -v "$ZETT" || true)"
    if [[ -z "$ZETT" ]]; then
        echo "FAIL: zett is not installed or available in PATH" >&2
        exit 1
    fi
fi

if [[ ! -x "$ZETT" ]]; then
    echo "FAIL: Zett binary is not executable: $ZETT" >&2
    exit 1
fi

WORK="$(mktemp -d "${TMPDIR:-$HOME}/zett-smoke.XXXXXXXX")"
trap 'rm -rf "$WORK"' EXIT
SOURCE="$WORK/hello.titan"
ARTIFACT="$WORK/hello.tbc"

cat > "$SOURCE" <<'EOF'
fn main() {
    print("TERMUX_SMOKE_OK")
}
EOF

echo "== Device =="
echo "uname: $(uname -m)"
echo "package architecture: $(dpkg --print-architecture 2>/dev/null || echo unknown)"
echo "binary: $ZETT"

echo "== Version =="
VERSION_OUTPUT="$($ZETT version)"
printf '%s\n' "$VERSION_OUTPUT"
grep -Fq "TITAN Language Compiler" <<<"$VERSION_OUTPUT"

# Clap must be able to load the complete executable before any language test.
"$ZETT" --help >/dev/null

echo "== Check =="
CHECK_OUTPUT="$($ZETT check "$SOURCE")"
printf '%s\n' "$CHECK_OUTPUT"
grep -Fq "CHECK OK" <<<"$CHECK_OUTPUT"

echo "== Run =="
RUN_OUTPUT="$($ZETT run "$SOURCE")"
printf '%s\n' "$RUN_OUTPUT"
grep -Fq "TERMUX_SMOKE_OK" <<<"$RUN_OUTPUT"

echo "== Sandboxed pure program =="
SANDBOX_OUTPUT="$($ZETT run "$SOURCE" --sandbox)"
printf '%s\n' "$SANDBOX_OUTPUT"
grep -Fq "TERMUX_SMOKE_OK" <<<"$SANDBOX_OUTPUT"

echo "== Build bytecode =="
"$ZETT" build "$SOURCE" --output "$ARTIFACT"
test -s "$ARTIFACT"

echo "== Execute bytecode =="
EXEC_OUTPUT="$($ZETT exec "$ARTIFACT")"
printf '%s\n' "$EXEC_OUTPUT"
grep -Fq "TERMUX_SMOKE_OK" <<<"$EXEC_OUTPUT"

echo "TERMUX RELEASE SMOKE TEST: PASS"

#!/usr/bin/env bash
# E2E del runtime Titan nativo contra servicios del sistema reales: X11/Xvfb
# con xclip y una sesión D-Bus con dunst. No ejecuta la VM del seed (3231a66
# conserva un backend Rust de portapapeles en memoria) ni compila Titan con Cargo.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
COMPILER="${COMPILER:-selfhost/titanc1}"
BUILD_TIMEOUT="${BUILD_TIMEOUT:-300}"
TEST_TIMEOUT="${TEST_TIMEOUT:-60}"

if [[ -z "${DISPLAY:-}" || -z "${DBUS_SESSION_BUS_ADDRESS:-}" ]]; then
  echo "requires an active X11 DISPLAY and a D-Bus session" >&2
  exit 2
fi
for program in xclip dunst notify-send dbus-send; do
  if ! command -v "$program" >/dev/null 2>&1; then
    echo "clipboard/notification integration requires $program" >&2
    exit 2
  fi
done
if [[ ! -x "$COMPILER" ]]; then
  echo "native Titan compiler not found or not executable: $COMPILER" >&2
  exit 2
fi

unset WAYLAND_DISPLAY
export TITAN_REQUIRE_SYSTEM_CLIPBOARD=1
export TITAN_REQUIRE_SYSTEM_NOTIFICATION=1

tmp="$(mktemp -d)"
dunst_log="$tmp/dunst.log"
# Start the real notification daemon under the workflow's D-Bus/X11 session.
dunst >"$dunst_log" 2>&1 &
dunst_pid=$!
cleanup() {
  kill "$dunst_pid" 2>/dev/null || true
  wait "$dunst_pid" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT

ready=0
for _ in $(seq 1 50); do
  # Query the bus daemon directly; using notify-send here would activate a
  # competing Notifications service before dunst claims the well-known name.
  if dbus-send --session --dest=org.freedesktop.DBus --print-reply \
      /org/freedesktop/DBus org.freedesktop.DBus.NameHasOwner \
      string:org.freedesktop.Notifications 2>/dev/null | grep -Fq "boolean true"; then
    ready=1
    break
  fi
  if ! kill -0 "$dunst_pid" 2>/dev/null; then
    cat "$dunst_log" >&2
    echo "dunst exited before registering its notification service" >&2
    exit 1
  fi
  sleep 0.1
done
if [[ "$ready" != "1" ]]; then
  cat "$dunst_log" >&2
  echo "dunst did not acquire org.freedesktop.Notifications on the D-Bus session" >&2
  exit 1
fi

program="$tmp/clipboard-notify"
if ! timeout "$BUILD_TIMEOUT" "$COMPILER" selfhost/tests/native/clipboard_notify.titan "$program" >"$tmp/build.log" 2>&1; then
  cat "$tmp/build.log" >&2
  echo "Titan-native clipboard/notification test did not compile" >&2
  exit 1
fi
if [[ ! -x "$program" ]]; then
  cat "$tmp/build.log" >&2
  echo "Titan compiler did not produce an executable" >&2
  exit 1
fi
if ! timeout "$TEST_TIMEOUT" "$program" >"$tmp/stdout" 2>"$tmp/stderr"; then
  cat "$tmp/stdout" >&2
  cat "$tmp/stderr" >&2
  echo "Titan-native real clipboard/notification test failed" >&2
  exit 1
fi
for expected in \
  "clipboard write true" \
  "clipboard round-trip true" \
  "notification delivered true"; do
  if ! grep -Fqx "$expected" "$tmp/stdout"; then
    cat "$tmp/stdout" >&2
    cat "$tmp/stderr" >&2
    echo "Titan-native integration did not confirm: $expected" >&2
    exit 1
  fi
done
printf '%s\n' "Titan-native system integration: xclip clipboard read/write and dunst notification succeeded" "--- stdout ---"
cat "$tmp/stdout"
if [[ -s "$tmp/stderr" ]]; then
  echo "--- stderr ---"
  cat "$tmp/stderr"
fi

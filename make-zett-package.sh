#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="/data/data/com.termux/files/usr"
BINARY=""
ARCH=""
VERSION=""
OUTPUT_DIR="$ROOT_DIR"
BUILD_LOCAL=false

usage() {
    cat <<'EOF'
Build a native Termux .deb for Zett.

Usage:
  ./make-zett-package.sh [--build-local]
  ./make-zett-package.sh --binary PATH --arch ARCH [options]

Options:
  --binary PATH       Prebuilt Android/Termux titan binary to package.
  --arch ARCH         Termux package architecture: arm, aarch64, i686, or x86_64.
  --version VERSION   Debian package version. Defaults to the workspace version.
  --output-dir PATH   Destination directory. Defaults to the repository root.
  --build-local       Build titan on the current Termux device before packaging.
  -h, --help          Show this help.

With no --binary argument, local compilation is used for backwards compatibility.
EOF
}

workspace_version() {
    awk '
        /^\[workspace\.package\]$/ { in_workspace_package = 1; next }
        /^\[/ { in_workspace_package = 0 }
        in_workspace_package && /^version[[:space:]]*=/ {
            value = $0
            sub(/^[^=]*=[[:space:]]*/, "", value)
            gsub(/^[\"[:space:]]+|[\"[:space:]]+$/, "", value)
            print value
            exit
        }
    ' "$ROOT_DIR/Cargo.toml"
}

while (($# > 0)); do
    case "$1" in
        --binary)
            [[ $# -ge 2 ]] || { echo "error: --binary requires a path" >&2; exit 2; }
            BINARY="$2"
            shift 2
            ;;
        --arch)
            [[ $# -ge 2 ]] || { echo "error: --arch requires a value" >&2; exit 2; }
            ARCH="$2"
            shift 2
            ;;
        --version)
            [[ $# -ge 2 ]] || { echo "error: --version requires a value" >&2; exit 2; }
            VERSION="$2"
            shift 2
            ;;
        --output-dir)
            [[ $# -ge 2 ]] || { echo "error: --output-dir requires a path" >&2; exit 2; }
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --build-local)
            BUILD_LOCAL=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown argument '$1'" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ -n "$BINARY" && "$BUILD_LOCAL" == true ]]; then
    echo "error: --binary and --build-local cannot be used together" >&2
    exit 2
fi

if [[ -z "$VERSION" ]]; then
    VERSION="$(workspace_version)"
fi
if [[ -z "$VERSION" || ! "$VERSION" =~ ^[0-9A-Za-z.+:~-]+$ ]]; then
    echo "error: invalid Debian package version '$VERSION'" >&2
    exit 2
fi

if [[ -z "$ARCH" ]]; then
    if command -v dpkg >/dev/null 2>&1; then
        ARCH="$(dpkg --print-architecture)"
    else
        echo "error: --arch is required when dpkg is unavailable" >&2
        exit 2
    fi
fi
case "$ARCH" in
    arm|aarch64|i686|x86_64) ;;
    *)
        echo "error: unsupported Termux architecture '$ARCH'" >&2
        exit 2
        ;;
esac

if [[ -z "$BINARY" ]]; then
    if [[ "$(uname -o 2>/dev/null || true)" != "Android" && -z "${TERMUX_VERSION:-}" ]]; then
        echo "error: local builds are only supported inside Termux; use --binary for a cross-compiled executable" >&2
        exit 2
    fi
    echo "Building the complete Zett binary locally for Termux ($ARCH)..."
    (
        cd "$ROOT_DIR"
        CARGO_INCREMENTAL=0 cargo build --locked --release -p titan_cli
    )
    BINARY="$ROOT_DIR/target/release/titan"
fi

if [[ ! -f "$BINARY" ]]; then
    echo "error: binary does not exist: $BINARY" >&2
    exit 2
fi

BINARY="$(cd -- "$(dirname -- "$BINARY")" && pwd)/$(basename -- "$BINARY")"
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd -- "$OUTPUT_DIR" && pwd)"

STAGING="$(mktemp -d "${TMPDIR:-/tmp}/zett-package.XXXXXXXX")"
trap 'rm -rf "$STAGING"' EXIT
PACKAGE_ROOT="$STAGING/root"
PACKAGE_TMP="$STAGING/zett.deb"

mkdir -p \
    "$PACKAGE_ROOT/DEBIAN" \
    "$PACKAGE_ROOT$PREFIX/bin" \
    "$PACKAGE_ROOT$PREFIX/share/zett/stdlib" \
    "$PACKAGE_ROOT$PREFIX/share/zett/source"

install -m 0755 "$BINARY" "$PACKAGE_ROOT$PREFIX/bin/zett"
cp -a "$ROOT_DIR/stdlib/." "$PACKAGE_ROOT$PREFIX/share/zett/stdlib/"
cp -a "$ROOT_DIR/crates" "$ROOT_DIR/docs" "$ROOT_DIR/examples" \
    "$PACKAGE_ROOT$PREFIX/share/zett/source/"
cp -a \
    "$ROOT_DIR/Cargo.toml" \
    "$ROOT_DIR/Cargo.lock" \
    "$ROOT_DIR/README.md" \
    "$ROOT_DIR/LICENSE" \
    "$ROOT_DIR/rust-toolchain.toml" \
    "$PACKAGE_ROOT$PREFIX/share/zett/source/"

find "$PACKAGE_ROOT" -type d -exec chmod 0755 {} +
find "$PACKAGE_ROOT" -type f -exec chmod a+r {} +
chmod 0755 "$PACKAGE_ROOT$PREFIX/bin/zett"

INSTALLED_SIZE="$(du -sk "$PACKAGE_ROOT" | awk '{print $1}')"
cat > "$PACKAGE_ROOT/DEBIAN/control" <<EOF
Package: zett
Version: $VERSION
Architecture: $ARCH
Section: devel
Priority: optional
Installed-Size: $INSTALLED_SIZE
Maintainer: Alex Sanders Soto
Description: TITAN language compiler and integrated runtime
 Zett is the single-binary TITAN compiler, bytecode VM, package tooling,
 WebAssembly backend, and standard library distribution for Termux.
EOF
chmod 0644 "$PACKAGE_ROOT/DEBIAN/control"

if dpkg-deb --help 2>&1 | grep -q -- '--root-owner-group'; then
    dpkg-deb --build --root-owner-group "$PACKAGE_ROOT" "$PACKAGE_TMP"
else
    dpkg-deb --build "$PACKAGE_ROOT" "$PACKAGE_TMP"
fi

OUTPUT="$OUTPUT_DIR/zett_${VERSION}_${ARCH}.deb"
mv -f "$PACKAGE_TMP" "$OUTPUT"
echo "Package created: $OUTPUT"
echo "Architecture: $ARCH"
echo "Version: $VERSION"

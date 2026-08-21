#!/usr/bin/env bash
set -euo pipefail

# Build a native Termux/Android binary + .deb package for titan_cli (zett).
#
# Two release lanes are supported via TERMUX_ARCH:
#   arm     -> armv7-linux-androideabi  (ELF32, 32-bit; dispositivos antiguos)
#   aarch64 -> aarch64-linux-android    (ELF64, 64-bit; Redmi 9C y Android moderno)
#
# The Redmi 9C (MediaTek Helio G22) is aarch64, so that lane produces the
# .deb that installs on the phone. Both lanes reuse the same packaging logic.

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TERMUX_ARCH="${TERMUX_ARCH:-arm}"
ANDROID_API="${ANDROID_API:-24}"
NDK_VERSION="${NDK_VERSION:-27.2.12479018}"
ANDROID_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"

case "$TERMUX_ARCH" in
    arm)
        TERMUX_TARGET="armv7-linux-androideabi"
        CLANG_PREFIX="armv7a-linux-androideabi${ANDROID_API}"
        EXPECTED_ELF_CLASS="ELF32"
        EXPECTED_ELF_MACHINE="ARM"
        ;;
    aarch64)
        TERMUX_TARGET="aarch64-linux-android"
        CLANG_PREFIX="aarch64-linux-android${ANDROID_API}"
        EXPECTED_ELF_CLASS="ELF64"
        EXPECTED_ELF_MACHINE="AArch64"
        ;;
    *)
        echo "error: unsupported TERMUX_ARCH '$TERMUX_ARCH' (expected: arm, aarch64)" >&2
        exit 2
        ;;
esac
export TERMUX_TARGET

if [[ -z "$ANDROID_ROOT" ]]; then
    echo "error: ANDROID_SDK_ROOT or ANDROID_HOME is required" >&2
    exit 2
fi
if ! command -v sdkmanager >/dev/null 2>&1; then
    echo "error: sdkmanager is unavailable" >&2
    exit 2
fi
if ! command -v rustup >/dev/null 2>&1; then
    echo "error: rustup is required to install the Rust target" >&2
    exit 2
fi

sdkmanager "ndk;${NDK_VERSION}"
rustup target add "$TERMUX_TARGET"

toolchain="${ANDROID_ROOT}/ndk/${NDK_VERSION}/toolchains/llvm/prebuilt/linux-x86_64"
clang="${toolchain}/bin/${CLANG_PREFIX}-clang"
clangxx="${toolchain}/bin/${CLANG_PREFIX}-clang++"
llvm_ar="${toolchain}/bin/llvm-ar"
llvm_ranlib="${toolchain}/bin/llvm-ranlib"

for tool in "$clang" "$clangxx" "$llvm_ar" "$llvm_ranlib"; do
    if [[ ! -x "$tool" ]]; then
        echo "error: required Android NDK tool is unavailable: $tool" >&2
        exit 2
    fi
done

# Cross-check the OTHER Android architecture so both ABIs are exercised on
# every run. The arm lane verifies aarch64 (as before); the aarch64 lane is
# itself a real release build, so no extra check is needed.
if [[ "$TERMUX_ARCH" == "arm" ]]; then
    ANDROID_API="$ANDROID_API" NDK_VERSION="$NDK_VERSION" \
        bash scripts/check-android-aarch64-ci.sh
fi

export CARGO_INCREMENTAL=0

# Cargo/cc variable names derive from the target: dashes become underscores,
# lowercase becomes uppercase for CARGO_TARGET_*.
cargo_target_var="$(printf '%s' "$TERMUX_TARGET" | tr '[:lower:]-' '[:upper:]_')"
cc_env="CC_$(printf '%s' "$TERMUX_TARGET" | tr '-' '_')"
cxx_env="CXX_$(printf '%s' "$TERMUX_TARGET" | tr '-' '_')"
ar_env="AR_$(printf '%s' "$TERMUX_TARGET" | tr '-' '_')"
ranlib_env="RANLIB_$(printf '%s' "$TERMUX_TARGET" | tr '-' '_')"

export "CARGO_TARGET_${cargo_target_var}_LINKER=$clang"
export "CARGO_TARGET_${cargo_target_var}_AR=$llvm_ar"
export "$cc_env=$clang"
export "$cxx_env=$clangxx"
export "$ar_env=$llvm_ar"
export "$ranlib_env=$llvm_ranlib"

cargo build --locked --release -p titan_cli --target "$TERMUX_TARGET"

binary="$ROOT_DIR/target/${TERMUX_TARGET}/release/titan"
if [[ ! -s "$binary" ]]; then
    echo "error: Cargo did not produce the expected Termux binary: $binary" >&2
    exit 1
fi

for command in file readelf dpkg-deb sha256sum; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "error: required verification command is unavailable: $command" >&2
        exit 2
    fi
done

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR/elf"

file "$binary" | tee "$DIST_DIR/elf/file.txt"
readelf -h "$binary" | tee "$DIST_DIR/elf/header.txt"
readelf -l "$binary" | tee "$DIST_DIR/elf/program-headers.txt"
grep -Eq "Class:[[:space:]]+${EXPECTED_ELF_CLASS}" "$DIST_DIR/elf/header.txt"
grep -Eq "Machine:[[:space:]]+${EXPECTED_ELF_MACHINE}" "$DIST_DIR/elf/header.txt"

workspace_version="$(awk '
    /^\[workspace\.package\]$/ { active=1; next }
    /^\[/ { active=0 }
    active && /^version[[:space:]]*=/ {
        value=$0
        sub(/^[^=]*=[[:space:]]*/, "", value)
        gsub(/^[\\"[:space:]]+|[\\"[:space:]]+$/, "", value)
        print value
        exit
    }
' Cargo.toml)"
if [[ -z "$workspace_version" ]]; then
    echo "error: workspace package version was not found" >&2
    exit 1
fi

run_number="${GITHUB_RUN_NUMBER:-0}"
run_attempt="${GITHUB_RUN_ATTEMPT:-1}"
package_revision="${TERMUX_PACKAGE_REVISION:-2}"
package_version="${workspace_version}-${package_revision}.ci${run_number}.${run_attempt}"

cp "$binary" "$DIST_DIR/zett"
chmod 0755 "$DIST_DIR/zett"
cp scripts/test-termux-release.sh "$DIST_DIR/"
chmod 0755 "$DIST_DIR/test-termux-release.sh"

bash make-zett-package.sh \
    --binary "$binary" \
    --arch "$TERMUX_ARCH" \
    --version "$package_version" \
    --output-dir "$DIST_DIR"

package="$DIST_DIR/zett_${package_version}_${TERMUX_ARCH}.deb"
test -s "$package"
test "$(dpkg-deb --field "$package" Package)" = "zett"
test "$(dpkg-deb --field "$package" Architecture)" = "$TERMUX_ARCH"
test "$(dpkg-deb --field "$package" Version)" = "$package_version"
dpkg-deb --info "$package" | tee "$DIST_DIR/package-info.txt"
dpkg-deb --contents "$package" | tee "$DIST_DIR/package-contents.txt"

extracted="$(mktemp -d "${TMPDIR:-/tmp}/zett-package-check.XXXXXXXX")"
trap 'rm -rf "$extracted"' EXIT
dpkg-deb --extract "$package" "$extracted"
packaged_binary="$extracted/data/data/com.termux/files/usr/bin/zett"
test -x "$packaged_binary"
cmp "$binary" "$packaged_binary"

(
    cd "$DIST_DIR"
    sha256sum "$(basename -- "$package")" zett > SHA256SUMS
)
printf '%s\n' "$package_version" > "$DIST_DIR/VERSION"
git rev-parse HEAD > "$DIST_DIR/COMMIT"

echo "Termux ${TERMUX_ARCH} release candidate created and structurally verified:"
echo "  package: $package"
echo "  binary:  $DIST_DIR/zett"
echo "  version: $package_version"
echo "  arch:    ${TERMUX_ARCH} (${TERMUX_TARGET})"

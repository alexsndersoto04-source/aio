#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TERMUX_TARGET="${TERMUX_TARGET:-armv7-linux-androideabi}"
TERMUX_ARCH="${TERMUX_ARCH:-arm}"
ANDROID_API="${ANDROID_API:-24}"
NDK_VERSION="${NDK_VERSION:-27.2.12479018}"
ANDROID_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"

if [[ "$TERMUX_TARGET" != "armv7-linux-androideabi" || "$TERMUX_ARCH" != "arm" ]]; then
    echo "error: this release lane is intentionally restricted to native Termux ARM 32-bit" >&2
    exit 2
fi
if [[ -z "$ANDROID_ROOT" ]]; then
    echo "error: ANDROID_SDK_ROOT or ANDROID_HOME is required" >&2
    exit 2
fi
if ! command -v sdkmanager >/dev/null 2>&1; then
    echo "error: sdkmanager is unavailable" >&2
    exit 2
fi

sdkmanager "ndk;${NDK_VERSION}"

toolchain="${ANDROID_ROOT}/ndk/${NDK_VERSION}/toolchains/llvm/prebuilt/linux-x86_64"
clang="${toolchain}/bin/armv7a-linux-androideabi${ANDROID_API}-clang"
clangxx="${toolchain}/bin/armv7a-linux-androideabi${ANDROID_API}-clang++"
llvm_ar="${toolchain}/bin/llvm-ar"
llvm_ranlib="${toolchain}/bin/llvm-ranlib"

for tool in "$clang" "$clangxx" "$llvm_ar" "$llvm_ranlib"; do
    if [[ ! -x "$tool" ]]; then
        echo "error: required Android NDK tool is unavailable: $tool" >&2
        exit 2
    fi
done

export CARGO_INCREMENTAL=0
export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$clang"
export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_AR="$llvm_ar"
export CC_armv7_linux_androideabi="$clang"
export CXX_armv7_linux_androideabi="$clangxx"
export AR_armv7_linux_androideabi="$llvm_ar"
export RANLIB_armv7_linux_androideabi="$llvm_ranlib"

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
grep -Eq 'Class:[[:space:]]+ELF32' "$DIST_DIR/elf/header.txt"
grep -Eq 'Machine:[[:space:]]+ARM' "$DIST_DIR/elf/header.txt"

workspace_version="$(awk '
    /^\[workspace\.package\]$/ { active=1; next }
    /^\[/ { active=0 }
    active && /^version[[:space:]]*=/ {
        value=$0
        sub(/^[^=]*=[[:space:]]*/, "", value)
        gsub(/^[\"[:space:]]+|[\"[:space:]]+$/, "", value)
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
package_version="${workspace_version}~ci${run_number}.${run_attempt}"

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

sha256sum "$package" "$DIST_DIR/zett" > "$DIST_DIR/SHA256SUMS"
printf '%s\n' "$package_version" > "$DIST_DIR/VERSION"

echo "Termux ARM release candidate created and structurally verified:"
echo "  package: $package"
echo "  binary:  $DIST_DIR/zett"
echo "  version: $package_version"

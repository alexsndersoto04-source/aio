#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TARGET="aarch64-linux-android"
ANDROID_API="${ANDROID_API:-24}"
NDK_VERSION="${NDK_VERSION:-27.2.12479018}"
ANDROID_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}"

if [[ -z "$ANDROID_ROOT" ]]; then
    echo "error: ANDROID_SDK_ROOT or ANDROID_HOME is required" >&2
    exit 2
fi
if ! command -v rustup >/dev/null 2>&1; then
    echo "error: rustup is required to install the AArch64 Rust target" >&2
    exit 2
fi
if ! command -v sdkmanager >/dev/null 2>&1; then
    echo "error: sdkmanager is unavailable" >&2
    exit 2
fi

sdkmanager "ndk;${NDK_VERSION}"
rustup target add "$TARGET"

toolchain="${ANDROID_ROOT}/ndk/${NDK_VERSION}/toolchains/llvm/prebuilt/linux-x86_64"
clang="${toolchain}/bin/aarch64-linux-android${ANDROID_API}-clang"
clangxx="${toolchain}/bin/aarch64-linux-android${ANDROID_API}-clang++"
llvm_ar="${toolchain}/bin/llvm-ar"
llvm_ranlib="${toolchain}/bin/llvm-ranlib"

for tool in "$clang" "$clangxx" "$llvm_ar" "$llvm_ranlib"; do
    if [[ ! -x "$tool" ]]; then
        echo "error: required Android NDK tool is unavailable: $tool" >&2
        exit 2
    fi
done

export CARGO_INCREMENTAL=0
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$clang"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_AR="$llvm_ar"
export CC_aarch64_linux_android="$clang"
export CXX_aarch64_linux_android="$clangxx"
export AR_aarch64_linux_android="$llvm_ar"
export RANLIB_aarch64_linux_android="$llvm_ranlib"

# This is a real Android cross-check: build scripts for bundled C components
# such as SQLite receive the NDK compiler instead of accidentally searching for
# a nonexistent host aarch64-linux-android-gcc.
cargo check --locked --workspace --all-targets --target "$TARGET"

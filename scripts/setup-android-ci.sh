#!/usr/bin/env bash
set -euo pipefail

# Android SDK setup for the Termux build lanes (armv7 / aarch64).
#
# Explicit replacement for android-actions/setup-android@v3: since 2026-09-14
# Google removed the legacy "tools" package from the SDK repository, and the
# action's default package list ("tools platform-tools") makes sdkmanager
# exit 1 before the action exports the SDK to PATH. Every downstream
# workflow using the action broke that day (see android-actions/setup-android
# issue #513).
#
# This script:
#   1. Uses the SDK preinstalled on ubuntu-latest when present, otherwise
#      downloads command-line-tools from dl.google.com.
#   2. Removes the legacy "tools" directory the image ships: sdkmanager
#      tries to validate it against the remote repository (which no longer
#      carries the package) and hard-fails.
#   3. Accepts licenses and installs platform-tools.
#   4. Exports the same contract the action exported: ANDROID_SDK_ROOT and
#      ANDROID_HOME via GITHUB_ENV, sdkmanager on PATH via GITHUB_PATH.
#
# Usage (from a GitHub Actions step):  bash scripts/setup-android-ci.sh

# 1. SDK location: prefer what the runner image already provides.
export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/.android/sdk}}"
export ANDROID_HOME="$ANDROID_SDK_ROOT"
mkdir -p "$ANDROID_SDK_ROOT"

# 2. Drop the legacy "tools" package if the image ships it.
rm -rf "$ANDROID_SDK_ROOT/tools"

SDKMGR=""
for candidate in \
    "$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager" \
    "$ANDROID_SDK_ROOT/cmdline-tools/16.0/bin/sdkmanager"; do
    if [[ -x "$candidate" ]]; then
        SDKMGR="$candidate"
        break
    fi
done

if [[ -z "$SDKMGR" ]]; then
    echo "cmdline-tools not preinstalled; downloading from dl.google.com..."
    curl -fsSL --retry 3 -o /tmp/cmdline-tools.zip \
        "https://dl.google.com/android/repository/commandlinetools-linux-13114758_latest.zip"
    unzip -q /tmp/cmdline-tools.zip -d /tmp/cmdline-tools
    mkdir -p "$ANDROID_SDK_ROOT/cmdline-tools"
    rm -rf "$ANDROID_SDK_ROOT/cmdline-tools/latest"
    mv /tmp/cmdline-tools/cmdline-tools "$ANDROID_SDK_ROOT/cmdline-tools/latest"
    SDKMGR="$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager"
fi

echo "sdkmanager: $("$SDKMGR" --version)"

# 3. Accept licenses. Bounded input (here-string) instead of `yes |` to
# avoid SIGPIPE killing the pipeline under `set -o pipefail`.
LICENSES_INPUT="$(printf 'y\n%.0s' {1..20})"
"$SDKMGR" --sdk_root="$ANDROID_SDK_ROOT" --licenses <<< "$LICENSES_INPUT"

"$SDKMGR" --sdk_root="$ANDROID_SDK_ROOT" platform-tools

# 4. Same contract the old action exported.
if [[ -n "${GITHUB_ENV:-}" ]]; then
    echo "ANDROID_SDK_ROOT=$ANDROID_SDK_ROOT" >> "$GITHUB_ENV"
    echo "ANDROID_HOME=$ANDROID_HOME" >> "$GITHUB_ENV"
fi
if [[ -n "${GITHUB_PATH:-}" ]]; then
    echo "$(dirname "$SDKMGR")" >> "$GITHUB_PATH"
fi

echo "Android SDK ready at: $ANDROID_SDK_ROOT"

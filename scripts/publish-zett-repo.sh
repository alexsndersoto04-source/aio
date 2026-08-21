#!/usr/bin/env bash
# Publish a Zett .deb to the zett-repo APT branch (the Termux package store),
# regenerating the Packages / Packages.gz / Release indexes so `pkg install
# zett` resolves the new release. Run on Termux (or any host with
# dpkg-scanpackages and apt-ftparchive, e.g. a Debian/Ubuntu runner).
#
# Usage:
#   ./scripts/publish-zett-repo.sh /path/to/zett_1.0.0_arm.deb
#
# After publishing, users refresh and install with:
#   pkg update && pkg install zett
set -euo pipefail

DEB="${1:?usage: publish-zett-repo.sh <zett.deb>}"
if [[ ! -f "$DEB" ]]; then
    echo "error: deb not found: $DEB" >&2
    exit 2
fi
for tool in dpkg-scanpackages apt-ftparchive git gzip; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "error: required tool is unavailable: $tool" >&2
        echo "       (on Termux: pkg install dpkg apt)" >&2
        exit 2
    fi
done

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

# Materialize the zett-repo branch in a temporary worktree so the main checkout
# (and the arena session branch) is left completely untouched.
git fetch origin zett-repo
WORKTREE="$(mktemp -d "${TMPDIR:-/tmp}/zett-repo.XXXXXX")"
trap 'git worktree remove --force "$WORKTREE" 2>/dev/null || true; rm -rf "$WORKTREE"' EXIT
git worktree add --detach "$WORKTREE" FETCH_HEAD
cd "$WORKTREE"

# Drop the new .deb into the store.
DEB_NAME="$(basename "$DEB")"
cp "$DEB" "./$DEB_NAME"

# Regenerate the APT indexes. --multiversion keeps every published release
# listed so `pkg install zett=1.0.0` and the legacy versions both resolve.
dpkg-scanpackages --arch arm --multiversion . > Packages
gzip -9c Packages > Packages.gz
apt-ftparchive release . -c release.conf > Release

git add -A
git commit -m "Publish $DEB_NAME" >/dev/null
git push origin HEAD:zett-repo

echo "Published $DEB_NAME to zett-repo."
echo "Users refresh and install with:  pkg update && pkg install zett"

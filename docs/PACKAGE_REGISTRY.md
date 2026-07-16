# TITAN Remote Package Registry

`RegistryClient` requires an HTTPS base URL. It fetches `/v1/packages/{percent-encoded-name}`, parses version metadata, resolves the highest release matching a SemVer requirement, downloads a bounded archive, verifies lowercase/uppercase SHA-256, and writes it atomically into a content-addressed cache.

Metadata records version, HTTPS archive URL, SHA-256 and dependency ranges. Registry/package-name mismatch, insecure initial/redirect URL, malformed metadata/ranges/hashes, non-200 status, body limits and checksum mismatches are errors.

Cache layout is `<cache>/<name>/<version>/<sha256>.tpkg`. Existing entries are rehashed before use. Temporary writes include process ID and are renamed only after verification, so interrupted downloads are never treated as packages.

Every release includes an Ed25519 public key/signature over the 32-byte SHA-256 digest. Download requires both digest and signature verification.

`.tpkg` is gzip-compressed tar. Safe extraction allows only regular files/directories with relative normal path components, rejects duplicates, symlinks/hardlinks/devices and traversal, enforces file/count/total limits, requires root `Titan.toml`, extracts into staging, and atomically renames on success.

The graph resolver collects every constraint per package and performs highest-version-first backtracking when transitive constraints conflict. It validates every range, limits package/decision counts, caches index fetches, supports dependency cycles through selected-version consistency checks, and emits a deterministic sorted lockfile containing archive, hash, signing key and dependency ranges.

CLI commands are `titan add <name> <range>`, `titan fetch [--offline]`, and `titan update`. Add rewrites `Titan.toml` atomically; fetch resolves/downloads/verifies/extracts and writes `Titan.remote.lock`; offline requires the lock and verified cache. The project loader maps remote aliases to `.titan/packages/<name>/<version>/src` so fetched dependencies participate in normal imports.

Publisher key ownership/publishing is the next registry block.

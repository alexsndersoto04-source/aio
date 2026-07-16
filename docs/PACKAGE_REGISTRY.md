# TITAN Remote Package Registry

`RegistryClient` requires an HTTPS base URL. It fetches `/v1/packages/{percent-encoded-name}`, parses version metadata, resolves the highest release matching a SemVer requirement, downloads a bounded archive, verifies lowercase/uppercase SHA-256, and writes it atomically into a content-addressed cache.

Metadata records version, HTTPS archive URL, SHA-256 and dependency ranges. Registry/package-name mismatch, insecure initial/redirect URL, malformed metadata/ranges/hashes, non-200 status, body limits and checksum mismatches are errors.

Cache layout is `<cache>/<name>/<version>/<sha256>.tpkg`. Existing entries are rehashed before use. Temporary writes include process ID and are renamed only after verification, so interrupted downloads are never treated as packages.

Archive extraction, full dependency-graph solving, signatures and CLI integration are the next registry blocks.

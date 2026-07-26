# Zett / TITAN — Changelog

## 0.3.0 — Phase 5: real Android hardware & OS bindings

### Added
- **`std::termux::*`** — 23 native functions that shell out to the
  Termux:API CLI shipped by the Termux:API Android app. Everything is
  real, nothing is simulated:
  - Device state: `battery_status`, `wifi_info`, `telephony_info`.
  - Location: `location(provider, request)`.
  - Sensors: `sensor_list`, `sensor_read`.
  - Real system clipboard: `clipboard_get`, `clipboard_set`.
  - Feedback: `vibrate`, `torch`, `toast`, `notify`, `notify_remove`,
    `tts_speak`, `brightness`.
  - Communications: `sms_list`, `sms_send`, `contacts`.
  - Camera: `camera_info`, `camera_photo`.
  - Dialog & sharing: `dialog`, `share`.
  - Availability probe: `is_available` for cross-platform code.
- All Phase-5 natives are gated behind the `Process` capability, so
  `zett run --sandbox` blocks them consistently with `std::process::*`.
- `examples/android.titan` demonstrating battery, toast, vibration,
  clipboard, notification, sensors and WiFi.

### Requirements (on-device)
- **Termux:API** app from F-Droid or Play Store.
- `pkg install termux-api` inside Termux.

If the CLI is missing, every helper returns a typed
`TermuxError::MissingCli` so `.titan` programs can degrade gracefully.

---

## 0.2.0 — Phases 1-4

### Added

**Phase 1 — Fundamentals**
- `std::regex` — Unicode-aware pattern matching (crate `regex`).
- `std::uuid` — UUID v4 and v7 (crate `uuid`).
- `std::hash` — SHA-256/384/512, SHA-3, BLAKE3, HMAC (RustCrypto).
- `std::random` — OS entropy + reproducible ChaCha20 (crate `rand`).
- `std::datetime` — dates, RFC 3339/2822, offsets (crate `chrono`).
- `std::url` — parse/build URLs and query strings (crate `url`).
- `std::dirs` — HOME/config/cache/downloads (crate `dirs`).

**Phase 2 — Formats & compression**
- `std::compress` — Gzip, Zlib, Deflate, Zstandard.
- `std::archive` — tar & zip pack/unpack, zip-slip safe.
- `std::yaml` — parse, stringify, multi-document.
- `std::xml` — quick-xml tree parsing + escapes.

**Phase 3 — Advanced networking**
- `std::http_full` — blocking HTTPS client (ureq + rustls-ring).
- `std::dns` — hickory-resolver lookups (A, AAAA, MX, TXT, CNAME, PTR).
- `std::email` — SMTP with STARTTLS / implicit TLS (lettre).

**Phase 4 — Modern cryptography**
- `std::crypto` — ChaCha20-Poly1305, AES-256-GCM AEAD.
- `std::password` — Argon2id, bcrypt.
- `std::jwt` — HS256 / RS256 JSON Web Tokens.

### Fixed
- Cleaned up hallucinated artifacts left by an earlier AI-assisted
  session (zero-byte binaries, imaginary informe docs, broken fix
  scripts). Marked `titan native` / `titan mobile` as experimental
  with runtime warnings; they don't produce loadable ELF or APK yet.
- Forced the whole workspace onto rustls' `ring` backend to keep the
  Termux build small and avoid aws-lc-sys (~300 C files).
- `titan_tls::ensure_default_crypto_provider()` installs a default
  rustls CryptoProvider exactly once per process; fixes the crash in
  `titan_postgres::builds_rustls_connector` after Phase 3 landed.

### Notes
- All new stdlib modules live behind Cargo features; the `extras`
  meta-feature bundles them all and is on by default.
- Docs: `docs/EXTRAS.md` walks through every module and the Termux
  build recipe.

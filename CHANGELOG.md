# Zett / TITAN — Changelog

## 0.8.0 — Phase 10: NoSQL (embedded KV + Redis)

### Added
- **`std::kv::*`** — real embedded key-value database via `sled` 0.34.
  Pure Rust, ACID, persists a whole database to a single directory on
  disk. Multiple databases and named sub-buckets ("trees") can coexist
  through opaque `i64` handles.
  - Lifecycle: `open(path)`, `close`, `flush`.
  - Default tree: `insert`, `get`, `remove`, `contains`, `len`, `clear`,
    `keys`, `compare_and_swap(key, expected, new)` — pass empty bytes
    for `None`.
  - Named trees (buckets): `open_tree(db, name)`, `tree_insert`,
    `tree_get`, `tree_remove`, `tree_len`, `tree_keys`.
- **`std::redis::*`** — blocking Redis client via `redis` 0.27.
  Connections are opaque handles.
  - Lifecycle: `connect(url)`, `close`, `ping`.
  - Strings: `set`, `set_ex`, `get`, `del`, `exists`, `expire`, `ttl`,
    `incr`, `keys(pattern)`.
  - Lists: `lpush`, `rpush`, `lrange`, `llen`.
  - Hashes: `hset`, `hget`, `hdel`, `hgetall`.
  - Escape hatch: `raw(command_and_args)` for anything else.
- `examples/database.titan` opens a sled database in `$HOME`, writes
  three users, reads one back, walks all keys, uses a "sessions"
  sub-bucket for tokens, exercises compare-and-swap, flushes and
  closes. Runs offline (no Redis required).

### Nothing removed
All Phases 1-9 remain untouched.

---

## 0.7.0 — Phase 9: Audio

### Added
- **`std::audio::*`** — real WAV I/O and synthesis (crate `hound`, pure
  Rust, no native audio deps), plus playback and recording delegated to
  the Termux:API binaries so the compile never breaks on Android.
  - Read: `read_wav(path)`, `read_wav_bytes(bytes)` — both return
    `{ samples, sample_rate, channels, bits_per_sample }` with the
    samples normalized to floats in `[-1.0, 1.0]`.
  - Write: `write_wav(path, samples, sample_rate, channels)` and
    in-memory `encode_wav(samples, sample_rate, channels)`.
  - Synthesis: `sine_wave`, `square_wave`, `saw_wave`, `white_noise` —
    each returns a float sample array for the requested duration/rate.
  - Playback (via `termux-media-player`): `play(path)`, `pause`,
    `resume`, `stop`, `info`, `is_termux_media_available`.
  - Recording (via `termux-microphone-record`): `record_start(path,
    seconds)`, `record_stop`, `record_info`.
- `examples/audio.titan` synthesises a 500 ms A4 tone, writes and
  re-reads the WAV, tries to play it via Termux:API, and stitches
  Do-Re-Mi-Fa-Sol into a scale WAV.

### Nothing removed
All Phases 1-8 remain untouched.

---

## 0.6.0 — Phase 8: System & OS

### Added
- **`std::procfs::*`** — cross-platform system information via `sysinfo`.
  Works on Termux/Android, Linux and macOS.
  - Identity: `hostname`, `kernel`, `os_name`, `os_version`, `uptime`.
  - CPU: `cpu_usage` (global %), `cpu_count`, `cpus()` (per-core map).
  - Memory: `total_memory`, `used_memory`, `available_memory`,
    `total_swap`, `used_swap`.
  - `load_average()` returning `{one, five, fifteen}`.
  - Processes: `process_count`, `top_processes(limit)` sorted by CPU %.
  - `disks()` and `networks()` with usage counters.
- **`std::fswatch::*`** — file-system watcher powered by `notify`
  (inotify on Linux/Android).
  - `watch_once(path, timeout_ms, recursive)` — one-shot blocking watch.
  - Handle-based `open(path, recursive)` + `next_event(handle, timeout_ms)`
    + `close(handle)` for long-lived daemons.
- **`std::signals::*`** — Unix signals via `signal-hook`.
  - `install("SIGINT")` (idempotent), `pending("SIGINT")` for counter
    polling, `wait_any(timeout_ms)` returning the first fired signal.
  - Names accepted with or without `SIG` prefix.
- `examples/system.titan` demoing hostname, OS, CPU %, memory, load
  average, top processes, disks and network counters.

### Nothing removed
All Phases 1-7 stay exactly as they were in 0.5.0.

---

## 0.5.0 — Phase 7: Images & QR codes

### Added
- **`std::image::*`** — real image processing via the `image` crate.
  Supports PNG, JPEG, WebP, BMP, GIF. Images are managed through opaque
  `i64` handles kept in a process-wide registry.
  - I/O: `load(path)`, `load_bytes(bytes)`, `save(handle, path)`,
    `encode(handle, format)`, `close(handle)`.
  - Metadata: `width`, `height`, `color_type`.
  - Transforms (return new handles): `resize`, `resize_exact`,
    `thumbnail`, `crop`, `grayscale`, `blur`, `brighten`,
    `rotate90`/`180`/`270`, `flip_horizontal`, `flip_vertical`.
  - Named filters: `nearest`, `triangle`, `catmullrom`, `gaussian`,
    `lanczos3`.
- **`std::qrcode::*`** — QR code generation via the `qrcode` crate.
  - `to_ascii(text, level, dark, light)` — printable text.
  - `to_unicode(text, level)` — dense Unicode block art.
  - `to_svg(text, level, module_pixels)` — SVG bytes.
  - `to_png(text, level, side_pixels)` — PNG bytes.
  - `save_png(text, level, side_pixels, path)` — write PNG to disk.
  - Error-correction levels: `L`, `M`, `Q`, `H`.
- `examples/images.titan` demoing a QR encoded as ASCII + Unicode + PNG,
  then reloading the PNG and creating a 100×100 thumbnail and a
  grayscale version.

### Combines beautifully with earlier phases
- Take a photo with `std::termux::camera_photo` (Phase 5), resize it
  with `std::image::resize` (Phase 7), hash it with `std::hash::sha256`
  (Phase 1), generate a QR of the hash with `std::qrcode::to_ansi`
  (Phase 7), and share it via `std::termux::share` (Phase 5).

### Nothing removed
All Phases 1-6 remain untouched.

---

## 0.4.0 — Phase 6: Terminal & TUI

### Added
- **`std::term::*`** — real terminal control powered by `crossterm`:
  - `print_colored`, `print_styled`, `print_attr` (bold/italic/underline).
  - Named colours plus custom `rgb:R,G,B` and `#RRGGBB`.
  - `clear_screen`, `clear_line`, `move_to`, `hide_cursor`, `show_cursor`,
    `size`, `flush`.
  - Alt-screen / raw-mode switches: `enter_alt_screen`, `leave_alt_screen`,
    `enable_raw`, `disable_raw`.
  - `read_key(timeout_ms)` returning normalized names like `Enter`,
    `Ctrl+c`, `Shift+F1`, `Up`.
- **`std::readline::*`** — GNU-Readline-style line editing via `rustyline`:
  - `prompt`, `prompt_with_history`, `prompt_persistent(prompt, path)`,
    `prompt_secret` (input hidden).
- **`std::progress::*`** — animated progress via `indicatif`:
  - `bar_new(total)`, `spinner_new()`, `set_message`, `set_position`,
    `increment`, `finish`, `abandon`.
- `examples/tui.titan` demoing colors, terminal size, an animated
  progress bar and a spinner.

### Nothing removed
All Phase 1-5 modules from 0.3.0 remain exactly as they were.

---

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

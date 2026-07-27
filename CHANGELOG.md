# Zett / TITAN — Changelog

## 0.14.0 — Phase 13': Wi-Fi introspection (termux-wifi-*)

### Added
- **`std::wifi::*`** — real bindings to the `termux-wifi-*` CLIs shipped
  by the official Termux:API package. Nothing is simulated: every call
  spawns the matching binary and surfaces exactly what Android's
  `WifiManager` reports.
    * `scan()` → `[{ ssid, bssid, rssi, frequency_mhz, timestamp,
      channel_bandwidth_mhz, center_frequency_mhz }, ...]`
      — nearby access points from the last cached scan.
    * `connection_info()` → `{ ssid, bssid, ip, mac_address,
      link_speed_mbps, rssi, frequency_mhz, network_id,
      supplicant_state, hidden_ssid }` or `nil` when not connected.
    * `set_enabled(bool)` — toggle the Wi-Fi radio (may silently
      no-op on Android ≥ 10 with the screen locked; upstream Android
      restriction, not a bug).
    * `signal_bars(rssi_dbm)` → 0..=4 — pure Rust, no CLI, matches
      Android's `WifiManager.calculateSignalLevel` heuristic. Safe to
      call anywhere for UI rendering.

### Why Wi-Fi and not Bluetooth
Termux:API does **not** expose Bluetooth scanning / BLE. `bluetoothctl`
/ BlueZ / `hcitool` all require Linux's BlueZ stack, which Android
doesn't use (it uses BlueDroid). Confirmed by the Termux maintainer
(Grimler91, 2022): *"android doesn't use bluez, so bluetoothctl cannot
work. What you need is a termux-api 'bluetoothAPI', but no one has
worked on writing such an API at the moment."*

Rather than ship a fake `std::bluetooth::*` module (which would
violate the project's zero-simulations rule), this release replaces
what was going to be Phase 13 with **Phase 13'**: real Wi-Fi
introspection using `termux-wifi-scaninfo`, `termux-wifi-connectioninfo`
and `termux-wifi-enable` — all of which are confirmed present in the
official termux-api package.

### 📦 Ejemplo
`examples/wifi.titan` — escanea redes cercanas, imprime SSID / RSSI /
frecuencia / bars por cada AP, después muestra el estado de la conexión
actual (SSID, IP, MAC, link speed).

## 0.13.0 — Phase 12 (part 3): BERT-family multi-input inference

### Added
- **`std::onnx::load_bert(path, batch, seq_len)`** — pin both input
  tensors (`input_ids`, `attention_mask`) to `[batch, seq_len]` of
  `i64` before optimize. Matches the shape 99% of HuggingFace exports
  use for DistilBERT / MiniLM / RoBERTa classifiers and encoders.
- **`std::onnx::load_bert3(path, batch, seq_len)`** — same but pins
  three inputs (`input_ids`, `attention_mask`, `token_type_ids`).
  Classic BERT-base-uncased needs this third tensor.
- **`std::onnx::run_bert(handle, shape, input_ids, attention_mask)`**
  → `{values, shape}` — feeds a text sample through the model in one
  call. Combined with `std::tokenize::encode()` from Phase 12 pt.1,
  a text-to-logits pipeline fits in ~10 lines of `.titan`.
- **`std::onnx::run_bert3(handle, shape, input_ids, attention_mask,
  token_type_ids)`** — three-input equivalent.
- **`std::math::exp(x)`**, **`std::math::log(x, base)`**,
  **`std::math::to_float(int)`**, **`std::math::to_int(float)`** —
  small additions needed to build a real softmax + int/float
  arithmetic on top of tokenizer/model outputs without leaving Titan.
- **`std::tokenize::encode_padded(handle, text, max_length, pad_id, add_special_tokens)`**
  — encode + pad-to-max_length or truncate. Necessary when the ONNX
  transformer graph was compiled with a fixed `[batch, seq_len]` input
  shape (which is the norm — MiniLM's tokenizer.json ships with
  padding baked in, but DistilBERT's doesn't). Uses `pad_id` for
  `ids` / `type_ids` / `special_tokens_mask` and `0` for `attention_mask`
  so downstream transformers correctly ignore padded positions.
- **`examples/sentiment.titan`** — end-to-end demo: loads a real
  DistilBERT sentiment classifier (SST-2, 2 classes: NEGATIVE /
  POSITIVE), tokenizes English text, runs the ONNX forward pass on
  device, applies a numerically-stable 2-class softmax and prints
  the sentiment label with its confidence — 100% offline, no cloud,
  no API keys, no Python interpreter.

### Notes
- The Rust API additions are non-breaking: `load`, `load_shape`,
  `run_f32`, `run_ids` from v0.12.0 still work unchanged.
- Suggested model for the demo:
  `Xenova/distilbert-base-uncased-finetuned-sst-2-english` — use the
  **FP32 `model.onnx`** (~260 MB), NOT `model_quantized.onnx`
  (~65 MB). The quantized export uses INT8-specific ops that
  onnxruntime handles in the browser but tract-onnx cannot analyse.
  Download instructions printed by the example already point at the
  right one.

## 0.12.0 — Phase 12 (part 2): ONNX inference on-device

### Added
- **`std::onnx::*`** — real ONNX model inference via `tract-onnx` 0.21,
  the pure-Rust runtime Sonos uses in production for wake-word and
  streaming speech recognition on their smart speakers. **No CUDA, no
  cuDNN, no BLAS, no ONNX Runtime C++.** Runs anywhere Rust compiles,
  including armv7-linux-androideabi (your Termux ARM phone).
- API (opaque `i64` handles; multiple models can coexist):
    * `load(path)` — parse → optimize → make runnable in one shot.
    * `load_shape(path, shape)` — same, but pin the first input's shape
      before optimizing (needed for models with dynamic axes, e.g. BERT
      that leaves batch/seq-len symbolic).
    * `close(handle)`.
    * `input_count(handle)` / `output_count(handle)`.
    * `input_shape(handle, i)` / `output_shape(handle, i)` — return an
      `[Int]` shape (may contain -1 for symbolic dims tract couldn't
      resolve statically).
    * `run_f32(handle, shape, data)` — flat f32 input, returns
      `{values: [Float], shape: [Int]}` (first output). Perfect for
      MNIST, MobileNet, image classifiers, VAD, etc.
    * `run_ids(handle, shape, ids)` — same but for i64 token-id inputs
      (BERT / MiniLM / DistilBERT and other transformers), so you can
      pipe the output of `std::tokenize::encode()` straight in.
- **Combines** with Fase 12 pt.1 (`std::tokenize::*`): tokenize text →
  feed ids to an ONNX transformer → get embeddings back. All on-device,
  offline, no cloud, no API key.

### Notes
- `tract-onnx` 0.21 build takes 8–12 min on Termux the first time
  (~50 crates in the dep graph — `prost` protobuf, `tract-hir`,
  `tract-nnef`, `tract-onnx-opl`, `tract-core`, `tract-linalg`,
  `smallvec`, `num-integer`, `memmap2`, ...). All pure Rust, no C.
- Suggested first model: MNIST-8 (~26 KB) or MobileNet-v2 (~14 MB).
  Both are on the ONNX model zoo.
- For LLM-family models (BERT, MiniLM, DistilBERT), use `load_shape`
  and pass `[1, seq_len]` as input shape before you feed ids.

## 0.11.0 — Phase 12 (part 1): HuggingFace tokenizers

### Added
- **`std::tokenize::*`** — real HuggingFace `tokenizers` crate 0.22
  built in a **pure-Rust configuration**. Defaults are deliberately
  turned off (`default-features = false`) to avoid three C/C++ deps
  that would break Termux builds:
    * `esaxx_fast`  → skipped (C++ suffix-array; pure-Rust fallback works)
    * `onig`        → skipped (C Oniguruma regex; replaced by `fancy-regex`)
    * `progressbar` → skipped (Phase 6 already ships `indicatif`)
  Only `fancy-regex` is enabled. **v0.22 is the first release that
  properly gates `SysRegex` on `fancy-regex XOR onig`** — v0.20/0.21
  hardcoded `mod onig;` and refused to compile without the C library.
- API (opaque `i64` handles from a process-wide registry, so multiple
  tokenizers can coexist):
    * `load(path)` — open a HuggingFace `tokenizer.json` from disk.
    * `from_json(text)` — same but from an in-memory JSON string.
    * `close(handle)` — release.
    * `vocab_size(handle)` — total vocab (incl. added tokens).
    * `encode(handle, text, add_special_tokens)` → map with
      `ids`, `tokens`, `type_ids`, `attention_mask`, `special_tokens_mask`.
    * `encode_batch(handle, texts, add_special_tokens)` — same but
      returns an array of maps (uses rayon internally for parallelism).
    * `decode(handle, ids, skip_special_tokens)` → string.
    * `token_to_id(handle, token)` / `id_to_token(handle, id)` — lookups
      that return `nil` when the token/id is absent.

### Coming next in Phase 12
- `std::onnx::*` via `tract-onnx` (pure-Rust ONNX inference). Kept as a
  separate patch so `tract`'s 8-12 min compile doesn't hold this shipment
  back if something needs adjusting.

## 0.10.0 — Phase 14: SVG charts (plotters)

### Added
- **`std::plot::*`** — real, pure-Rust charts via `plotters` 0.3.
  Deliberately built **without** `ttf` / `font-kit` (which pull in
  `freetype-sys` / `expat-sys` / `fontconfig`, all C-deps that break or
  bloat Termux builds). Every function writes a standalone `.svg` file;
  text is rendered by whatever viewer opens the file.
  - `line(path, title, x_axis, y_axis, xs, ys)` — single line chart
    with a marker on every sample.
  - `multi_line(path, title, x_axis, y_axis, labels, xs_of_series, ys_of_series)`
    — 3 parallel arrays (a triple-of-arrays per series would be a
    heterogeneous literal, which Titan's typechecker rejects). Each
    series gets a stable colour from an 8-slot palette + a legend entry.
  - `bar(path, title, y_axis, labels, values)` — bar chart.
  - `scatter(path, title, x_axis, y_axis, xs, ys)` — scatter plot.
  - `histogram(path, title, x_axis, values, bins)` — auto-binned
    histogram.
- **`examples/charts.titan`** — writes 5 SVGs to `$HOME` (line, bar,
  scatter, histogram, multi-line) and prints their paths so you can
  `termux-open` them or `rsvg-convert` them to PNG.

### Notes
- SVGs are ~5-15 KB each — safe to commit to a repo, e-mail, or
  attach to WhatsApp.
- For PNG output on Termux, install `librsvg`: `pkg install librsvg`
  and then `rsvg-convert chart.svg -o chart.png`.
- Combines beautifully with `std::procfs::*` (Fase 8) to build live
  system dashboards, and with `std::server::respond_bytes` (Fase 11)
  to serve charts straight from an HTTP endpoint.

## 0.9.0 — Phase 11: Web server (tiny_http + matchit, axum-style)

### Added
- **`std::server::*`** — real pure-Rust HTTP/1.1 server via `tiny_http`
  0.12. No async runtime, no OpenSSL, no C shims. Blocking event-loop
  model that fits Titan's synchronous VM perfectly.
  - Lifecycle: `start(addr)`, `local_addr(server)`, `stop(server)`.
  - Accept: `accept(server, timeout_ms) → request | -1`.
  - Introspection: `method`, `url`, `path`, `query`, `remote_addr`,
    `header(name)`, `headers()` (whole map), `body()` (raw bytes),
    `body_text()` (UTF-8).
  - Responses: `respond` (text/plain), `respond_html`, `respond_json`,
    `respond_bytes(content_type, bytes)`, `respond_full(status,
    content_type, headers-map, body-bytes)`.
  - **WebSocket upgrade (RFC 6455):**
    `upgrade_websocket(request, max_message) → ws_handle`,
    `ws_recv(ws) → [kind, text, bytes]` (kind is one of `"text"`,
    `"binary"`, `"ping"`, `"pong"`, `"close"`; pings are auto-ponged),
    `ws_send_text`, `ws_send_binary`, `ws_close(ws, code, reason)`.
- **`std::router::*`** — high-performance radix-tree URL router via
  `matchit` 0.8 (the same crate axum uses internally).
  - `new()`, `drop(router)`.
  - `insert(router, pattern, tag)` — pattern syntax:
    * `/users` — static
    * `/users/{id}` — named parameter
    * `/files/{*rest}` — catch-all (must be last segment)
  - `at(router, path) → { pattern: tag, params: {name: value, ...} }`
    or `nil` when nothing matches.
  - `matches(router, path) → bool` for quick feature-flag style checks.
- **`examples/webserver.titan`** — end-to-end demo: binds a port,
  installs 4 routes with matchit, decodes path params for
  `GET /users/{id}` and `GET /files/{*rest}`, and returns JSON,
  HTML and plain text responses.

### Notes
- No TLS in the server itself (keeps the Termux build lean and avoids
  the `aws-lc-sys` C-dep trap). Put nginx / Caddy / stunnel in front
  for public HTTPS, or use the existing `std::http` client (which does
  use rustls) for outbound HTTPS.
- `std::ws::*` (RFC 6455 codec primitives) from Phase 3 stays available
  and is what `std::server::ws_*` builds upon.

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

#!/data/data/com.termux/files/usr/bin/bash
set -e

PREFIX="/data/data/com.termux/files/usr"
ROOT="packaging/zett-root"
OUT="zett_0.17.0_arm.deb"

rm -rf "$ROOT" "$OUT"

mkdir -p "$ROOT/DEBIAN"
mkdir -p "$ROOT$PREFIX/bin"
mkdir -p "$ROOT$PREFIX/share/zett/source"

cat > "$ROOT/DEBIAN/control" <<'EOF'
Package: zett
Version: 0.17.0
Architecture: arm
Maintainer: Alex Sanders Soto
Description: TITAN compiler: HTTPS, crypto, Android, TUI, images, QR, system, audio, NoSQL, web server, charts, HF tokenizers, ONNX inference (BERT-family multi-input, sentence-transformer pooling), Wi-Fi scanning, vector math (semantic search), PDF generation
 Zett installs the TITAN Language Compiler, examples and documentation.
 Includes a modern standard library with real (non-simulated) modules:
 regex, uuid, hash (SHA/BLAKE3/HMAC), random, datetime, url, dirs,
 compress (gzip/zstd), archive (tar/zip), yaml, xml, blocking HTTPS
 client, DNS resolver, SMTP email, ChaCha20-Poly1305 & AES-GCM AEAD,
 Argon2id/bcrypt password hashing, JWT signing/verification, direct
 access to Termux:API for Android hardware/OS integration (battery,
 GPS, sensors, camera, SMS, clipboard, vibrate, notify, TTS),
 terminal/TUI helpers (colors, cursor, raw mode, keyboard events,
 readline with history, animated progress bars and spinners), image
 processing (PNG/JPEG/WebP/BMP/GIF: load, save, encode, resize, crop,
 rotate, grayscale, blur, brighten), QR code generation
 (ASCII/Unicode/SVG/PNG at every error-correction level), system
 tools (procfs CPU/memory/processes/disks/networks, file-system watcher
 via inotify, Unix signal handling), and audio (WAV I/O and synthesis
 via hound: sine, square, sawtooth, white noise, fades; playback and
 recording through termux-media-player and termux-microphone-record), and
 NoSQL storage (embedded ACID key-value database via sled with named
 sub-buckets and compare-and-swap, plus a blocking Redis client with
 strings, lists, hashes, TTL and raw commands), and a pure-Rust HTTP/1.1
 web server (tiny_http) with a radix-tree URL router (matchit, the same
 router axum uses under the hood) supporting named and catch-all path
 parameters, JSON/HTML/bytes responses and RFC 6455 WebSocket upgrades,
 and pure-Rust SVG charts via plotters (line, multi-line, bar, scatter
 and histogram outputs to standalone .svg files that any viewer can
 render without shipping a font), and HuggingFace text tokenizers
 (BPE / WordPiece / Unigram) via the official `tokenizers` crate in a
 pure-Rust configuration — reads any HF `tokenizer.json` and exposes
 encode / encode_batch / decode / vocab_size / token_to_id lookups,
 and on-device ONNX inference via `tract-onnx` (Sonos' production
 inference engine, 100% Rust, no CUDA / cuDNN / BLAS / ONNX Runtime
 C++), able to load .onnx models, inspect their input/output shapes
 and run f32 or i64 tensor inputs entirely on the phone's CPU, plus
 multi-input BERT-family loaders (input_ids + attention_mask, with
 or without token_type_ids) so DistilBERT / MiniLM / classic BERT
 classifiers and embedding models can be tokenized with std::tokenize
 and forwarded through std::onnx in a single pipeline on the device,
 and Wi-Fi introspection (std::wifi::scan / connection_info / set_enabled
 / signal_bars) via the official termux-wifi-* CLIs shipped by the
 Termux:API package, plus sentence-transformer style embedding
 pooling (std::onnx::run_bert_pooled — attention-mask-weighted mean
 pool of the encoder's last_hidden_state) and pure-Rust vector math
 (std::vector::dot / norm / cosine_similarity / normalize / add / sub
 / scale / argmax) for on-device semantic search over MiniLM
 embeddings, and PDF generation (std::pdf::new / add_page / add_text /
 set_color / add_line / add_rect / save) built on printpdf 0.7 in a
 no-defaults pure-Rust config (no azul-layout, no rust-fontconfig, no
 HTML rendering — just the core PDF writer with the 14 built-in fonts).
EOF

install -m 755 target/release/titan "$ROOT$PREFIX/bin/zett"

cp -a crates docs examples "$ROOT$PREFIX/share/zett/source/"
cp -a Cargo.toml Cargo.lock README.md LICENSE rust-toolchain.toml \
  "$ROOT$PREFIX/share/zett/source/"

rm -f "$ROOT$PREFIX/share/zett/source/crates/titan_codegen/src/artifact.rs.bak"

find "$ROOT" -type d -exec chmod 755 {} +
find "$ROOT" -type f -exec chmod a+r {} +
chmod 755 "$ROOT$PREFIX/bin/zett"

dpkg-deb --build "$ROOT" "$OUT"

echo "Paquete creado: $OUT"

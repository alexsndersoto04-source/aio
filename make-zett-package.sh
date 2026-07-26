#!/data/data/com.termux/files/usr/bin/bash
set -e

PREFIX="/data/data/com.termux/files/usr"
ROOT="packaging/zett-root"
OUT="zett_0.6.0_arm.deb"

rm -rf "$ROOT" "$OUT"

mkdir -p "$ROOT/DEBIAN"
mkdir -p "$ROOT$PREFIX/bin"
mkdir -p "$ROOT$PREFIX/share/zett/source"

cat > "$ROOT/DEBIAN/control" <<'EOF'
Package: zett
Version: 0.6.0
Architecture: arm
Maintainer: Alex Sanders Soto
Description: TITAN language compiler: HTTPS, crypto, Android, TUI, images, QR, system info
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
 (ASCII/Unicode/SVG/PNG at every error-correction level), and system
 tools: procfs (CPU, memory, load average, processes, disks, networks),
 file-system watcher (inotify) and Unix signal handling.
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

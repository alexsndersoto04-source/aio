#!/data/data/com.termux/files/usr/bin/bash
set -e

PREFIX="/data/data/com.termux/files/usr"
ROOT="packaging/zett-root"
OUT="zett_0.3.0_arm.deb"

rm -rf "$ROOT" "$OUT"

mkdir -p "$ROOT/DEBIAN"
mkdir -p "$ROOT$PREFIX/bin"
mkdir -p "$ROOT$PREFIX/share/zett/source"

cat > "$ROOT/DEBIAN/control" <<'EOF'
Package: zett
Version: 0.3.0
Architecture: arm
Maintainer: Alex Sanders Soto
Description: TITAN language compiler with real HTTPS, crypto and Android bindings
 Zett installs the TITAN Language Compiler, examples and documentation.
 Includes a modern standard library with real (non-simulated) modules:
 regex, uuid, hash (SHA/BLAKE3/HMAC), random, datetime, url, dirs,
 compress (gzip/zstd), archive (tar/zip), yaml, xml, blocking HTTPS
 client, DNS resolver, SMTP email, ChaCha20-Poly1305 & AES-GCM AEAD,
 Argon2id/bcrypt password hashing, JWT signing/verification, and
 direct access to Termux:API for Android hardware/OS integration
 (battery, GPS, sensors, camera, SMS, clipboard, vibrate, notify, TTS).
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

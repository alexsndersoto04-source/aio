#!/data/data/com.termux/files/usr/bin/bash
set -e

PREFIX="/data/data/com.termux/files/usr"
ROOT="packaging/zett-root"
OUT="zett_0.2.0_arm.deb"

rm -rf "$ROOT" "$OUT"

mkdir -p "$ROOT/DEBIAN"
mkdir -p "$ROOT$PREFIX/bin"
mkdir -p "$ROOT$PREFIX/share/zett/source"

cat > "$ROOT/DEBIAN/control" <<'EOF'
Package: zett
Version: 0.2.0
Architecture: arm
Maintainer: Alex Sanders Soto
Description: Zett package containing the TITAN language compiler and ecosystem
 Zett installs the TITAN Language Compiler, examples, documentation,
 and the complete TITAN source ecosystem.
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

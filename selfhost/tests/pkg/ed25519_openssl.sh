#!/bin/bash
# Compara std::crypto::ed25519_* con OpenSSL: claves, firmas (Ed25519 es determinista) y verificación
# cruzada con mensajes de varias longitudes.   Uso: bash selfhost/tests/pkg/ed25519_openssl.sh [compilador]
set -u
COMPILER=${1:-selfhost/titanc3}
W=$(mktemp -d)
cat > $W/p.titan <<'T'
fn main() {
    let seed = std::encoding::hex_decode(std::env::get("SEED"))
    let msg = std::encoding::hex_decode(std::env::get("MSG"))
    let pk = std::crypto::ed25519_public_key(seed)
    let sig = std::crypto::ed25519_sign(seed, msg)
    print(std::encoding::hex_encode(pk))
    print(std::encoding::hex_encode(sig))
    print(std::crypto::ed25519_verify(pk, msg, std::encoding::hex_decode(std::env::get("OSIG"))))
}
T
"$COMPILER" $W/p.titan $W/p >/dev/null 2>&1 || { echo "no compila"; exit 1; }
pass=0; fail=0
for n in 1 2 31 32 33 63 64 65 100 127 128 129 300 1000 5000; do
  openssl genpkey -algorithm ed25519 -out $W/k.pem 2>/dev/null
  seed=$(openssl pkey -in $W/k.pem -outform DER 2>/dev/null | tail -c 32 | od -An -tx1 | tr -d ' \n')
  pub=$(openssl pkey -in $W/k.pem -pubout -outform DER 2>/dev/null | tail -c 32 | od -An -tx1 | tr -d ' \n')
  head -c $n /dev/urandom > $W/m.bin
  openssl pkeyutl -sign -rawin -inkey $W/k.pem -in $W/m.bin -out $W/osig.bin 2>/dev/null
  osig=$(od -An -tx1 $W/osig.bin | tr -d ' \n')
  msg=$(od -An -tx1 $W/m.bin | tr -d ' \n')
  out=$(SEED=$seed MSG=$msg OSIG=$osig $W/p)
  mine_pub=$(echo "$out" | sed -n 1p); mine_sig=$(echo "$out" | sed -n 2p); ver=$(echo "$out" | sed -n 3p)
  echo "$mine_sig" | xxd -r -p > $W/mine.bin 2>/dev/null || python3 -c "import sys;sys.stdout.buffer.write(bytes.fromhex(sys.argv[1]))" "$mine_sig" > $W/mine.bin
  openssl pkey -in $W/k.pem -pubout -out $W/pub.pem 2>/dev/null
  cross=$(openssl pkeyutl -verify -rawin -pubin -inkey $W/pub.pem -in $W/m.bin -sigfile $W/mine.bin 2>&1)
  if [ "$mine_pub" = "$pub" ] && [ "$mine_sig" = "$osig" ] && [ "$ver" = true ] && echo "$cross" | grep -q Successfully; then pass=$((pass+1)); else fail=$((fail+1)); echo "FALLA con $n bytes"; fi
done
rm -rf $W
echo "pasan $pass, fallan $fail"
[ $fail = 0 ]

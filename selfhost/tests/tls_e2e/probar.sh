#!/bin/bash
# Pruebas de extremo a extremo de TLS nativo: servidor frente a openssl s_client
# (EC P-256, P-384, RSA PKCS#1/PKCS#8, cadena de dos certificados) y cliente frente a
# pypi.org / registry.npmjs.org (este último solo habla TLS 1.2).
# Uso: probar.sh [compilador]   (desde la raíz del repositorio)
set -u
COMPILER=${1:-selfhost/titanc8}
W=$(mktemp -d)
pass=0; fail=0
ok() { echo "OK   $1"; pass=$((pass+1)); }
ko() { echo "FALLA $1"; fail=$((fail+1)); }
cd "$W"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -keyout ec.key -out ec.crt -days 30 -subj /CN=localhost -addext subjectAltName=DNS:localhost 2>/dev/null
openssl ecparam -name secp384r1 -genkey -noout -out p384.key 2>/dev/null
openssl req -x509 -new -key p384.key -out p384.crt -days 30 -subj /CN=localhost -addext subjectAltName=DNS:localhost 2>/dev/null
openssl req -x509 -newkey rsa:2048 -nodes -keyout rsa8.key -out rsa.crt -days 30 -subj /CN=localhost -addext subjectAltName=DNS:localhost 2>/dev/null
openssl rsa -traditional -in rsa8.key -out rsa1.key 2>/dev/null
cat rsa.crt ec.crt > chain.crt
cd - >/dev/null
port=45300
for caso in "ec.crt ec.key" "p384.crt p384.key" "rsa.crt rsa8.key" "rsa.crt rsa1.key" "chain.crt rsa8.key"; do
  set -- $caso; port=$((port+1))
  sed "s#@CERT@#$W/$1#;s#@KEY@#$W/$2#;s#@PORT@#$port#" selfhost/tests/tls_e2e/servidor.titan > $W/s$port.titan
  "$COMPILER" $W/s$port.titan $W/s$port >/dev/null 2>&1
  $W/s$port >$W/s$port.log 2>&1 &
  pid=$!
  sleep 1
  out=$(printf ping | timeout 10 openssl s_client -connect 127.0.0.1:$port -quiet 2>/dev/null)
  [ "$out" = "ping hello" ] && ok "servidor $caso (TLS 1.3)" || ko "servidor $caso: '$out'"
  out=$(printf ping | timeout 10 openssl s_client -connect 127.0.0.1:$port -groups P-256 -quiet 2>/dev/null)
  [ "$out" = "ping hello" ] && ok "servidor $caso (grupo P-256)" || ko "servidor $caso P-256: '$out'"
  err=$(printf x | timeout 10 openssl s_client -connect 127.0.0.1:$port -tls1_2 -quiet 2>&1 >/dev/null | grep -c "alert protocol version")
  [ "$err" = 1 ] && ok "servidor $caso rechaza TLS 1.2 con protocol_version" || ko "servidor $caso TLS 1.2"
  out=$(printf again | timeout 10 openssl s_client -connect 127.0.0.1:$port -quiet 2>/dev/null)
  [ "$out" = "again hello" ] && ok "servidor $caso sobrevive a un error" || ko "servidor $caso tras error: '$out'"
  kill $pid 2>/dev/null; wait $pid 2>/dev/null
done
"$COMPILER" selfhost/tests/tls_e2e/cliente.titan $W/cliente >/dev/null 2>&1
res=$(timeout 60 $W/cliente 2>&1)
echo "$res"
for h in pypi.org registry.npmjs.org files.pythonhosted.org; do
  echo "$res" | grep -q "^$h: HTTP/1.1 .* true$" && ok "cliente $h" || ko "cliente $h"
done
rm -rf "$W"
echo "pasan $pass, fallan $fail"
[ $fail = 0 ]

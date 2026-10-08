#!/bin/bash
# Compara std::net::http_get de la VM (glibc real) con el nativo (Titan) en
# varias configuraciones del sistema. Necesita sudo, los servidores de
# servidores.py y dns_falso.py (127.0.0.53) en marcha y el ejecutable nativo
# de prueba.titan en $1. Restaura los archivos de /etc al terminar.
set -u
NAT="$1"
ZETT="${ZETT:-zett}"
DIR="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d)"
for f in resolv.conf hosts nsswitch.conf host.conf gai.conf; do
  if [ -e /etc/$f ]; then sudo cp -p /etc/$f "$tmp/$f.orig"; fi
done
restore() {
  for f in resolv.conf hosts nsswitch.conf host.conf gai.conf; do
    if [ -e "$tmp/$f.orig" ]; then sudo cp -p "$tmp/$f.orig" /etc/$f; else sudo rm -f /etc/$f; fi
  done
  rm -rf "$tmp"
}
trap restore EXIT
HOSTS_BASE="$(cat "$tmp/hosts.orig")
127.0.0.2 doble.hosts
127.0.0.3 doble.hosts
127.0.0.3 v4v6.hosts
::1 v4v6.hosts"
printf 'alias1 a1.test\n' > "$tmp/aliases"
pass=0; fail=0
run() {  # nombre, resolv.conf, nsswitch, host.conf, gai.conf, env...
  local name="$1" resolv="$2" nss="$3" hconf="$4" gai="$5"; shift 5
  printf '%b' "$resolv" | sudo tee /etc/resolv.conf >/dev/null
  printf '%s\n' "$HOSTS_BASE" | sudo tee /etc/hosts >/dev/null
  if [ -n "$nss" ]; then printf '%b' "$nss" | sudo tee /etc/nsswitch.conf >/dev/null; else sudo cp -p "$tmp/nsswitch.conf.orig" /etc/nsswitch.conf; fi
  if [ -n "$hconf" ]; then printf '%b' "$hconf" | sudo tee /etc/host.conf >/dev/null; else sudo rm -f /etc/host.conf; [ -e "$tmp/host.conf.orig" ] && sudo cp -p "$tmp/host.conf.orig" /etc/host.conf; fi
  if [ -n "$gai" ]; then printf '%b' "$gai" | sudo tee /etc/gai.conf >/dev/null; else sudo rm -f /etc/gai.conf; fi
  env -u GITHUB_ACTIONS HOSTALIASES="$tmp/aliases" "$@" timeout 300 "$ZETT" run "$DIR/prueba.titan" "$DIR/urls.txt" > "$tmp/vm.out" 2>&1; local a=$?
  env HOSTALIASES="$tmp/aliases" "$@" timeout 300 "$NAT" "$DIR/urls.txt" > "$tmp/nat.out" 2>&1; local b=$?
  if cmp -s "$tmp/vm.out" "$tmp/nat.out" && [ $a = $b ]; then
    pass=$((pass + 1)); echo "igual: $name"
  else
    fail=$((fail + 1)); echo "DISTINTO: $name ($a/$b)"; diff "$tmp/vm.out" "$tmp/nat.out" | head -20
  fi
  [ -n "${VER:-}" ] && cat "$tmp/nat.out"
}
NS='nameserver 127.0.0.53\n'
run base "${NS}search lista.test buscar\noptions timeout:1 attempts:2\n" "" "" "" true
run sin-search "${NS}options timeout:1\n" "" "" "" true
run ndots3 "${NS}search lista.test\noptions ndots:3 timeout:1\n" "" "" "" true
run single "${NS}options single-request timeout:1\n" "" "" "" true
run reopen "${NS}options single-request-reopen timeout:1\n" "" "" "" true
run usevc "${NS}options use-vc timeout:1\n" "" "" "" true
run edns "${NS}options edns0 trust-ad timeout:1\n" "" "" "" true
run noaaaa "${NS}options no-aaaa timeout:1\n" "" "" "" true
run rotate "nameserver 127.0.0.54\n${NS}options rotate timeout:1 attempts:1\n" "" "" "" true
run cerrado "nameserver 127.0.0.54\noptions timeout:1 attempts:1\n" "" "" "" true
run dos-ns "nameserver 127.0.0.54\n${NS}options timeout:1\n" "" "" "" true
run domain "${NS}domain otra.test\noptions timeout:1\n" "" "" "" true
run no-tld "${NS}search lista.test\noptions no-tld-query timeout:1\n" "" "" "" true
run localdomain "${NS}options timeout:1\n" "" "" "" LOCALDOMAIN="otra.test lista.test"
run res-options "${NS}options timeout:1\n" "" "" "" RES_OPTIONS="ndots:2 attempts:1"
run noaliases "${NS}options timeout:1\n" "" "" "" RES_OPTIONS="no-aaaa"
run dns-files "${NS}options timeout:1\n" "hosts: dns files\n" "" "" true
run solo-files "${NS}options timeout:1\n" "hosts: files\n" "" "" true
run notfound-return "${NS}options timeout:1\n" "hosts: files [NOTFOUND=return] dns\n" "" "" true
run unavail "nameserver 127.0.0.54\noptions timeout:1 attempts:1\n" "hosts: dns [!UNAVAIL=return] files\n" "" "" true
run modulo-raro "${NS}options timeout:1\n" "hosts: raro files dns\n" "" "" true
run sin-hosts-linea "${NS}options timeout:1\n" "passwd: files\n" "" "" true
run hostconf-multi "${NS}options timeout:1\n" "" "multi on\nreorder on\n" "" true
run hostconf-malo "${NS}options timeout:1\n" "" "multi talvez\nnospoof on\ntrim .test\n" "" true
run gai-v4 "${NS}options timeout:1\n" "" "" "precedence ::ffff:0:0/96 100\n" true
run gai-label "${NS}options timeout:1\n" "" "" "label ::1/128 5\nlabel ::/0 1\nscopev4 ::ffff:127.0.0.0/104 14\n" true
run gai-reload "${NS}options timeout:1\n" "" "" "reload yes\nprecedence ::1/128 5\nprecedence ::ffff:0:0/96 60\n" true
echo "configuraciones: $((pass + fail))  iguales: $pass  distintas: $fail"
[ "$fail" -eq 0 ]

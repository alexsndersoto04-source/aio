#!/bin/bash
# Pruebas de regresión para Genéricos definidos por el usuario (<T>) en Titan.
set -eu
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
TITAN="${1:-${TITAN:-./selfhost/titan}}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "== 1. Chequeo y ejecución de examples/genericos.titan (VM + Nativo) =="
"$TITAN" check examples/genericos.titan
"$TITAN" run examples/genericos.titan
"$TITAN" compile examples/genericos.titan --output "$tmp/genericos_bin"
"$tmp/genericos_bin"

echo "== 2. Verificación de rechazos de tipos genéricos inválidos =="

cat << 'EOF' > "$tmp/rechazo_campo_generico.titan"
struct Caja<T> { valor: T }
fn main() {
    let c = Caja { valor: 42 }
    let mal: string = c.valor
}
EOF

cat << 'EOF' > "$tmp/rechazo_fn_generica.titan"
fn identidad<T>(x: T) -> T { x }
fn main() {
    let mal: string = identidad(99)
}
EOF

cat << 'EOF' > "$tmp/rechazo_mismo_par.titan"
struct MismoPar<T> { a: T, b: T }
fn main() {
    let m = MismoPar { a: 1, b: "no_es_int" }
}
EOF

cat << 'EOF' > "$tmp/rechazo_aridad_tipo.titan"
struct Caja<T> { valor: T }
fn main() {
    let c: Caja<int, string> = Caja { valor: 1 }
}
EOF

cat << 'EOF' > "$tmp/rechazo_cuerpo_generico.titan"
fn falso_int<T>(x: T) -> int {
    x
}
fn main() {}
EOF

for caso in \
    "rechazo_campo_generico.titan:type mismatch: expected String, found Int" \
    "rechazo_fn_generica.titan:type mismatch: expected String, found Int" \
    "rechazo_mismo_par.titan:type mismatch: expected Int, found String" \
    "rechazo_aridad_tipo.titan:type 'Caja' expects 1 type arguments, found 2" \
    "rechazo_cuerpo_generico.titan:type mismatch: expected Int, found TypeParam(\"T\")"
do
    archivo="${caso%%:*}"
    esperado="${caso#*:}"
    salida="$("$TITAN" check "$tmp/$archivo" 2>&1 || true)"
    if printf '%s\n' "$salida" | grep -Fq "$esperado"; then
        echo "ok: $archivo -> $esperado"
    else
        echo "FALLO en $archivo: se esperaba '$esperado', salió:" >&2
        echo "$salida" >&2
        exit 1
    fi
done

echo "probar.sh (genericos): todo bien"

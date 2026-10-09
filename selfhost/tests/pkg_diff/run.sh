#!/bin/bash
# Prueba diferencial del cargador con Titan.toml: crea proyectos en /tmp/pkg_diff y
# compara `zett wasm` (Rust, titan_pkg) con selfhost/wasm_cli.titan (loader.titan):
# el .wasm, los mapas, Titan.lock, la salida y el código de salida. En los casos de
# manifiesto inválido solo se exige el mismo código de salida y el mismo prefijo
# («invalid manifest at <ruta>: invalid Titan.toml»): el texto de TOML es del crate
# `toml` y el port no lo reproduce.
# Uso: selfhost/tests/pkg_diff/run.sh
set -u
cd "$(dirname "$0")/../../.."
ZETT="${ZETT:-zett}"
BIN="${WASM_TITAN:-/tmp/wasm_diff/wasm_titan}"
if [ ! -x "$BIN" ]; then mkdir -p "$(dirname "$BIN")"; ./selfhost/titanc3 selfhost/wasm_cli.titan "$BIN" || exit 1; fi
B=/tmp/pkg_diff; rm -rf $B; mkdir -p $B
mk() { mkdir -p "$B/$1"; }
w() { mkdir -p "$(dirname "$B/$1")"; printf '%b' "$2" > "$B/$1"; }
toml() { printf '[package]\nname = "%s"\nversion = "%s"\nedition = "2021"\nlicense = "MIT"\n' "$1" "${2:-0.1.0}"; }

# 1. app con dependencia por ruta (tabla [dependencies.x]) + módulo propio + lib de la dependencia
w ok_deps/app/Titan.toml "$(toml app)\n[dependencies.mathlib]\npath = \"../mathlib\"\n"
w ok_deps/app/src/main.titan 'import mathlib\nimport util\nfn main() { triple(14) + twice(4) }\n'
w ok_deps/app/src/util.titan 'fn twice(x: int) -> int { x * 2 }\n'
w ok_deps/mathlib/Titan.toml "$(toml mathlib 1.2.3-beta.1+build.5)"
w ok_deps/mathlib/src/lib.titan 'import helpers\nfn triple(x: int) -> int { helper_mul(x, 3) }\n'
w ok_deps/mathlib/src/helpers.titan 'fn helper_mul(a: int, b: int) -> int { a * b }\n'
# 2. dependencia transitiva, tabla en línea, módulo con mod.titan, alias con submódulo
w ok_nested/app/Titan.toml "$(toml app)\n[dependencies]\nlib_a = { path = \"../a\" } # comentario\n"
w ok_nested/app/src/main.titan 'import lib_a::geometry\nfn main() { area(3, 4) + from_b() }\n'
w ok_nested/a/Titan.toml "$(toml a)\n[dependencies]\nlib_b = { path = \"../b\", version = \"0.1.0\" }\n"
w ok_nested/a/src/geometry.titan 'import lib_b\nfn area(w: int, h: int) -> int { w * h }\nfn from_b() -> int { b_value() }\n'
w ok_nested/b/Titan.toml "$(toml b)"
w ok_nested/b/src/lib.titan 'fn b_value() -> int { 7 }\n'
# 3. proyecto sin dependencias y entrada por directorio
w ok_plain/Titan.toml "$(toml plain)"
w ok_plain/src/main.titan 'import sub\nfn main() { sub_value() }\n'
w ok_plain/src/sub/mod.titan 'fn sub_value() -> int { 42 }\n'
# 4. errores de proyecto
w err_missing_path/app/Titan.toml "$(toml app)\n[dependencies.gone]\npath = \"../gone\"\n"
w err_missing_path/app/src/main.titan 'fn main() { 1 }\n'
w err_no_path/app/Titan.toml "$(toml app)\n[dependencies.remote]\nversion = \"1.0.0\"\n"
w err_no_path/app/src/main.titan 'fn main() { 1 }\n'
w err_dep_cycle/a/Titan.toml "$(toml a)\n[dependencies.b]\npath = \"../b\"\n"
w err_dep_cycle/a/src/main.titan 'fn main() { 1 }\n'
w err_dep_cycle/b/Titan.toml "$(toml b)\n[dependencies.a]\npath = \"../a\"\n"
w err_dep_cycle/b/src/lib.titan 'fn x() { 1 }\n'
w err_import_cycle/app/Titan.toml "$(toml app)"
w err_import_cycle/app/src/main.titan 'import a\nfn main() { a() }\n'
w err_import_cycle/app/src/a.titan 'import b\nfn a() { 1 }\n'
w err_import_cycle/app/src/b.titan 'import a\nfn b() { 2 }\n'
w err_unresolved/app/Titan.toml "$(toml app)"
w err_unresolved/app/src/main.titan 'import nothing\nfn main() { 1 }\n'
w err_no_src/app/Titan.toml "$(toml app)"
w err_no_src/app/main.titan 'fn main() { 1 }\n'
# 5. manifiestos inválidos (mensaje de TOML distinto: solo prefijo)
w bad_version/app/Titan.toml "$(toml app 1.0)"
w bad_version/app/src/main.titan 'fn main() { 1 }\n'
w bad_missing_name/app/Titan.toml '[package]\nversion = "0.1.0"\nedition = "2021"\n'
w bad_missing_name/app/src/main.titan 'fn main() { 1 }\n'
w bad_syntax/app/Titan.toml '[package\nname = "x"\n'
w bad_syntax/app/src/main.titan 'fn main() { 1 }\n'
w bad_string_dep/app/Titan.toml "$(toml app)\n[dependencies]\nfoo = \"1.0\"\n"
w bad_string_dep/app/src/main.titan 'fn main() { 1 }\n'

# 6. sintaxis TOML variada que Rust acepta
w ok_toml_syntax/app/Titan.toml '# manifiesto con sintaxis variada\n[package]\nname = "app"\nversion = "10.20.30"\nedition = "2021"\nlicense = "MIT"\ndescription = """\nlinea 1\nlinea \\u00e9 "dos"\n"""\nkeywords = ["a", "b", [1, 2],\n  { x = 1 }]\nflag = true\nratio = 1.5e3\n[ dependencies . "dep-x" ]\npath = "../depx"\n'
w ok_toml_syntax/app/src/main.titan 'import dep_x_unused_guard\nfn main() { 1 }\n'
w ok_toml_syntax/app/src/dep_x_unused_guard.titan 'fn guard() -> int { 0 }\n'
w ok_toml_syntax/depx/Titan.toml "$(toml depx)"
w ok_toml_syntax/depx/src/lib.titan 'fn d() -> int { 1 }\n'
w ok_dotted/app/Titan.toml '[package]\nname = "app"\nversion = "0.0.1"\nedition = "2021"\n[dependencies]\nz.path = "../z"\na = { path = "../z" }\n'
w ok_dotted/app/src/main.titan 'import z\nimport a\nfn main() { zed() }\n'
w ok_dotted/z/Titan.toml "$(toml z)"
w ok_dotted/z/src/lib.titan 'fn zed() -> int { 26 }\n'
w bad_dup_key/app/Titan.toml '[package]\nname = "a"\nname = "b"\nversion = "0.1.0"\nedition = "2021"\n'
w bad_dup_key/app/src/main.titan 'fn main() { 1 }\n'
w bad_dup_table/app/Titan.toml '[package]\nname = "a"\nversion = "0.1.0"\nedition = "2021"\n[package]\nlicense = "x"\n'
w bad_dup_table/app/src/main.titan 'fn main() { 1 }\n'
w bad_unterminated/app/Titan.toml '[package]\nname = "a\nversion = "0.1.0"\nedition = "2021"\n'
w bad_unterminated/app/src/main.titan 'fn main() { 1 }\n'
w bad_type/app/Titan.toml '[package]\nname = 5\nversion = "0.1.0"\nedition = "2021"\n'
w bad_type/app/src/main.titan 'fn main() { 1 }\n'
w bad_remote_lock/app/Titan.toml "$(toml app)\n[dependencies.netlib]\nversion = \"2.0.0\"\n"
w bad_remote_lock/app/Titan.remote.lock '{"version":1,"packages":[{"name":"netlib","version":"2.0.0","dependencies":{}}]}'
w bad_remote_lock/app/src/main.titan 'fn main() { 1 }\n'
# 7. dependencia instalada (Titan.remote.lock + .titan/packages)
w ok_remote/app/Titan.toml "$(toml app)\n[dependencies.netlib]\nversion = \"2.0.0\"\n"
w ok_remote/app/Titan.remote.lock '{"version":1,"packages":[{"name":"netlib","version":"2.0.0","archive":"a.tar.gz","sha256":"00","signing_key":"k","signature":"s","dependencies":{}}]}'
w ok_remote/app/.titan/packages/netlib/2.0.0/Titan.toml "$(toml netlib 2.0.0)"
w ok_remote/app/.titan/packages/netlib/2.0.0/src/lib.titan 'fn net() -> int { 80 }\n'
w ok_remote/app/src/main.titan 'import netlib\nfn main() { net() }\n'

run_case() { # nombre entrada [prefijo-solo]
    local name=$1 entry=$2 mode=${3:-exact}
    local d=$B/$name
    rm -f $d/*/Titan.lock $d/Titan.lock
    local e="$d/$entry"
    mkdir -p $d/rust $d/titan
    "$ZETT" wasm "$e" --output "$d/rust/x.wasm" > "$d/rust.out" 2>&1; local rc1=$?
    local lock1=$(cat $(dirname "$(find $d -name Titan.lock | head -1)" 2>/dev/null)/Titan.lock 2>/dev/null)
    find $d -name Titan.lock -delete
    "$BIN" "$e" --output "$d/titan/x.wasm" > "$d/titan.out" 2>&1; local rc2=$?
    local lock2=$(cat $(dirname "$(find $d -name Titan.lock | head -1)" 2>/dev/null)/Titan.lock 2>/dev/null)
    sed -i "s#$d/rust/#OUT/#g" $d/rust.out; sed -i "s#$d/titan/#OUT/#g" $d/titan.out
    local ok=1
    [ $rc1 -eq $rc2 ] || ok=0
    if [ "$mode" = exact ]; then
        cmp -s $d/rust.out $d/titan.out || ok=0
        if [ $rc1 -eq 0 ]; then
            cmp -s $d/rust/x.wasm $d/titan/x.wasm || ok=0
            cmp -s $d/rust/x.wasm.map $d/titan/x.wasm.map || ok=0
            cmp -s $d/rust/x.wasm.map.json $d/titan/x.wasm.map.json || ok=0
            [ "$lock1" = "$lock2" ] || ok=0
        fi
    else
        local p1=$(head -c 60 $d/rust.out | head -1 | sed 's/: invalid Titan.toml.*//'); local p2=$(head -1 $d/titan.out | sed 's/: invalid Titan.toml.*//')
        [ "$(head -1 $d/rust.out | sed 's/invalid Titan.toml.*/invalid Titan.toml/;s/Not found.*/Not found/;s/` at line.*/`/')" = "$(head -1 $d/titan.out | sed 's/invalid Titan.toml.*/invalid Titan.toml/;s/Not found.*/Not found/;s/` at line.*/`/')" ] || ok=0
    fi
    if [ $ok -eq 1 ]; then echo "IGUAL   $name (rc=$rc1)"; else echo "DIFIERE $name (rc rust=$rc1 titan=$rc2)"; echo "  rust : $(head -c 300 $d/rust.out | head -2)"; echo "  titan: $(head -c 300 $d/titan.out | head -2)"; fail=1; fi
}
fail=0
run_case ok_deps app/src/main.titan
run_case ok_deps app
run_case ok_nested app/src/main.titan
run_case ok_plain src/main.titan
run_case ok_plain .
run_case err_missing_path app/src/main.titan
run_case err_no_path app/src/main.titan
run_case err_dep_cycle a/src/main.titan
run_case err_import_cycle app/src/main.titan
run_case err_unresolved app/src/main.titan
run_case err_no_src app/main.titan
run_case bad_version app/src/main.titan prefijo
run_case bad_missing_name app/src/main.titan prefijo
run_case bad_syntax app/src/main.titan prefijo
run_case bad_string_dep app/src/main.titan prefijo
run_case ok_toml_syntax app/src/main.titan
run_case ok_dotted app/src/main.titan
run_case ok_remote app/src/main.titan
run_case bad_remote_lock app/src/main.titan prefijo
run_case bad_dup_key app/src/main.titan prefijo
run_case bad_dup_table app/src/main.titan prefijo
run_case bad_unterminated app/src/main.titan prefijo
run_case bad_type app/src/main.titan prefijo
[ $fail -eq 0 ] && echo "TODO IGUAL" || echo "HAY DIFERENCIAS"
exit $fail

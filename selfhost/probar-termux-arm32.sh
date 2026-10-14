#!/data/data/com.termux/files/usr/bin/bash
# Prueba Titan en un Termux de ARM de 32 bits (armv7l / armv8l) y deja un informe para pegar.
#
#   bash probar-termux-arm32.sh            # instala lo que falte y prueba todo
#   bash probar-termux-arm32.sh --sin-instalar
#
# Qué hace, en orden (cada paso dice [OK], [FALLA] o [LIMITACIÓN]; nunca se detiene en el primero):
#   0. datos del aparato (modelo, Android, núcleo, ABIs, memoria);
#   1. instala clang + lld y el paquete zett_arm.termux.deb de la release (si no hay `titan`);
#   2. `titan version` y `zett version`, y comprueba que el ejecutable sea ELF de 32 bits ARM;
#   3. `titan check` (solo el front-end: no necesita clang);
#   4. `titan run hola.titan` (la primera compilación es la lenta: genera la caché del runtime);
#   5. `titan compile kit.titan` + ejecutar: enteros de 64 bits, coma flotante, enums, match,
#      Result, closures, mapas, Unicode, SHA-256, JSON y tareas con canales; se compara la salida
#      byte a byte con la de x86-64;
#   6. `titan run metodos.titan`: `p.nombre()` (CallMethod) todavía NO se admite en el backend LLVM
#      (ARM64/ARM32); se espera el error y se anota como limitación conocida;
#   7. proyecto con `titan new` + `titan run`.
# Al final escribe ~/titan-arm32-informe.txt con todo (incluidas las salidas y los tiempos).
#
# Variables: TITAN=/ruta/a/titan (usar otro ejecutable), VERSION=v1.3.0 (release a instalar),
#            DIR=carpeta de trabajo (por omisión ~/titan-arm32-prueba).
set -u

VERSION="${VERSION:-latest}"
if [ "$VERSION" = latest ]; then
  URL_BASE="https://github.com/alexsndersoto04-source/aio/releases/latest/download"
else
  URL_BASE="https://github.com/alexsndersoto04-source/aio/releases/download/$VERSION"
fi
PAQUETE="zett_arm.termux.deb"
DIR="${DIR:-$HOME/titan-arm32-prueba}"
INFORME="$HOME/titan-arm32-informe.txt"
instalar=1
[ "${1:-}" = "--sin-instalar" ] && instalar=0

mkdir -p "$DIR"
cd "$DIR"
: > "$INFORME"

ok=0; falla=0; limit=0
resumen=""
log() { printf '%s\n' "$*" | tee -a "$INFORME"; }
titulo() { log ""; log "== $* =="; }
paso_ok() { ok=$((ok + 1)); resumen="$resumen
[OK]         $*"; log "[OK] $*"; }
paso_falla() { falla=$((falla + 1)); resumen="$resumen
[FALLA]      $*"; log "[FALLA] $*"; }
paso_limit() { limit=$((limit + 1)); resumen="$resumen
[LIMITACIÓN] $*"; log "[LIMITACIÓN] $*"; }
# Ejecuta un comando con límite de tiempo, guarda su salida en $1 y devuelve su código.
# Imprime el tiempo que tardó.
correr() {
  local salida="$1"; shift
  local t0 t1 codigo
  t0=$(date +%s)
  if command -v timeout > /dev/null 2>&1; then
    timeout 1800 "$@" > "$salida" 2>&1
  else
    "$@" > "$salida" 2>&1
  fi
  codigo=$?
  t1=$(date +%s)
  log "   ($* : ${codigo} en $((t1 - t0)) s)"
  return $codigo
}
mostrar() { sed 's/^/   | /' "$1" | head -n "${2:-40}" | tee -a "$INFORME"; }
# ¿Dos archivos iguales? (sin depender de diffutils). diferencias() las enseña si hay `diff`.
iguales() { [ "$(cat "$1")" = "$(cat "$2")" ]; }
diferencias() {
  if command -v diff > /dev/null 2>&1; then diff "$1" "$2" | tee -a "$INFORME"
  else log "   esperado:"; mostrar "$1"; log "   obtenido:"; mostrar "$2"; fi
}

# ---------------------------------------------------------------- 0. el aparato
titulo "0. Aparato"
log "fecha:        $(date -u +%Y-%m-%dT%H:%M:%SZ)"
log "uname -a:     $(uname -a 2> /dev/null)"
log "uname -m:     $(uname -m 2> /dev/null)"
if command -v getprop > /dev/null 2>&1; then
  log "modelo:       $(getprop ro.product.manufacturer 2> /dev/null) $(getprop ro.product.model 2> /dev/null)"
  log "android:      $(getprop ro.build.version.release 2> /dev/null) (API $(getprop ro.build.version.sdk 2> /dev/null))"
  log "abilist:      $(getprop ro.product.cpu.abilist 2> /dev/null)"
  log "abilist32:    $(getprop ro.product.cpu.abilist32 2> /dev/null)"
  log "abilist64:    $(getprop ro.product.cpu.abilist64 2> /dev/null)"
fi
log "memoria:      $(grep -E 'MemTotal|MemAvailable' /proc/meminfo 2> /dev/null | tr -s ' ' | tr '\n' ' ')"
log "cpu:          $(grep -m1 -E 'Hardware|model name' /proc/cpuinfo 2> /dev/null | cut -d: -f2- | sed 's/^ *//') ($(grep -c ^processor /proc/cpuinfo 2> /dev/null) núcleos)"
log "features:     $(grep -m1 -E '^Features' /proc/cpuinfo 2> /dev/null | cut -d: -f2- | sed 's/^ *//')"
log "PREFIX:       ${PREFIX:-(sin PREFIX: esto no parece Termux)}"
log "TMPDIR:       ${TMPDIR:-(sin TMPDIR)}"
if command -v dpkg > /dev/null 2>&1; then log "dpkg arch:    $(dpkg --print-architecture 2> /dev/null)"; fi
case "$(uname -m)" in
  armv7l | armv8l | arm) log "=> Termux de 32 bits: es justo lo que queremos probar." ;;
  aarch64) log "=> AVISO: esto es un Termux de 64 bits; el paquete de 32 bits no es el adecuado aquí (sigue igual, pero el resultado no vale como prueba de ARM32)." ;;
  *) log "=> AVISO: arquitectura $(uname -m); el script está pensado para Termux ARM de 32 bits." ;;
esac

# ---------------------------------------------------------------- 1. instalar
titulo "1. Instalación"
TITAN="${TITAN:-}"
if [ -z "$TITAN" ] && command -v titan > /dev/null 2>&1; then TITAN="$(command -v titan)"; fi
if [ "$instalar" = 1 ] && command -v pkg > /dev/null 2>&1; then
  if ! command -v clang > /dev/null 2>&1 || ! command -v ld.lld > /dev/null 2>&1; then
    log "instalando clang y lld (Titan en ARM compila con LLVM)…"
    pkg install -y clang lld >> "$INFORME" 2>&1 || log "   aviso: pkg install clang lld devolvió error (mira el informe)"
  fi
fi
if [ -z "$TITAN" ] && [ "$instalar" = 1 ]; then
  if command -v pkg > /dev/null 2>&1; then
    log "descargando $URL_BASE/$PAQUETE …"
    rm -f "$PAQUETE"
    if curl -fL --retry 3 -o "$PAQUETE" "$URL_BASE/$PAQUETE" >> "$INFORME" 2>&1 || wget -O "$PAQUETE" "$URL_BASE/$PAQUETE" >> "$INFORME" 2>&1; then
      log "   $(ls -la "$PAQUETE" | awk '{print $5}') bytes; sha256: $(sha256sum "$PAQUETE" | cut -c1-16)…"
      if pkg install -y "./$PAQUETE" >> "$INFORME" 2>&1; then
        paso_ok "paquete $PAQUETE instalado con pkg"
      elif dpkg -i "./$PAQUETE" >> "$INFORME" 2>&1; then
        paso_ok "paquete $PAQUETE instalado con dpkg -i"
      else
        paso_falla "no se pudo instalar $PAQUETE (últimas líneas del informe):"
        tail -n 15 "$INFORME" | sed 's/^/   | /'
      fi
    else
      paso_falla "no se pudo descargar $URL_BASE/$PAQUETE"
    fi
    command -v titan > /dev/null 2>&1 && TITAN="$(command -v titan)"
  else
    log "no hay 'pkg': no es Termux, no instalo nada (usa TITAN=/ruta/a/titan)."
  fi
fi
if [ -z "$TITAN" ]; then
  paso_falla "no encuentro 'titan' en el PATH (¿se instaló el paquete?)"
  log "pkg list-installed | grep zett:"; (pkg list-installed 2> /dev/null | grep -i zett || true) | tee -a "$INFORME"
  log ""; log "Sin titan no se puede seguir. Informe en $INFORME"; exit 1
fi
log "titan:        $TITAN -> $(readlink -f "$TITAN" 2> /dev/null)"
carpeta="$(dirname "$(readlink -f "$TITAN" 2> /dev/null || echo "$TITAN")")"
log "carpeta:      $carpeta ($(ls "$carpeta" 2> /dev/null | tr '\n' ' '))"
log "runtime:      $([ -f "$carpeta/native/runtime.titan" ] && echo "native/runtime.titan presente ($(ls "$carpeta/native" | wc -l) archivos)" || echo "FALTA $carpeta/native/runtime.titan")"
log "clang:        $(command -v clang 2> /dev/null || echo 'NO ESTÁ') $(clang --version 2> /dev/null | head -1)"
log "ld.lld:       $(command -v ld.lld 2> /dev/null || echo 'NO ESTÁ') $(ld.lld --version 2> /dev/null | head -1)"
command -v clang > /dev/null 2>&1 || log "   AVISO: sin clang, 'titan run' y 'titan compile' van a fallar en ARM (pkg install clang lld)."

# ---------------------------------------------------------------- 2. version + ELF
titulo "2. titan version"
if correr salida_version.txt "$TITAN" version; then
  mostrar salida_version.txt 3
  if grep -q "TITAN Language Compiler" salida_version.txt; then paso_ok "titan version: $(head -1 salida_version.txt)"; else paso_falla "titan version respondió algo raro"; fi
else
  mostrar salida_version.txt 10
  paso_falla "titan version no arranca (si dice «Exec format error» o «cannot execute binary file», el binario no corre en este núcleo)"
fi
ejecutable="$(readlink -f "$TITAN" 2> /dev/null || echo "$TITAN")"
cab="$(od -An -tx1 -N20 "$ejecutable" 2> /dev/null | tr -d ' \n')"
# 7f454c46 01 = ELF32, 01 = little-endian; bytes 18-19 = máquina (28 00 = ARM, b7 00 = AArch64, 3e 00 = x86-64)
case "$cab" in
  7f454c4601*2800) paso_ok "el ejecutable es ELF de 32 bits ARM (EM_ARM): es el paquete de 32 bits" ;;
  7f454c4602*b700) paso_limit "el ejecutable es ELF de 64 bits AArch64: estás probando el paquete de 64 bits, no el de 32" ;;
  7f454c46*) paso_falla "el ejecutable es ELF pero no ARM32 (cabecera $cab)" ;;
  *) paso_falla "no pude leer la cabecera ELF de $ejecutable" ;;
esac
if command -v zett > /dev/null 2>&1; then
  if correr salida_zett.txt zett version; then paso_ok "zett version: $(head -1 salida_zett.txt)"; else paso_falla "zett version falló"; fi
else
  paso_falla "no hay enlace 'zett' en el PATH (el paquete debería traerlo)"
fi

# ---------------------------------------------------------------- programas de prueba
cat > hola.titan <<'EOF'
fn main() {
    print("¡Hola desde Titan en ARM32!")
    let x = 42
    let y = 58
    print("Suma: {x} + {y} = {x + y}")
}
EOF
cat > hola.esperado.txt <<'EOF'
¡Hola desde Titan en ARM32!
Suma: 42 + 58 = 100
EOF

cat > kit.titan <<'EOF'
// Prueba de Titan en ARM de 32 bits: lenguaje + runtime (sin red ni archivos).
struct Punto { x: int, y: int }

enum Figura {
    Circulo(float),
    Rect(float),
    Vacio,
}

// Nota: p.nombre() (CallMethod) todavía no se admite en el backend LLVM (ARM64/ARM32);
// por eso aquí es una función normal. Ver metodos.titan.
fn nombre(p: Punto) -> string { "P({p.x}, {p.y})" }

fn area(f: Figura) -> float {
    match f {
        Figura::Circulo(r) => 3.14159 * r * r,
        Figura::Rect(l) => l * l,
        Figura::Vacio => 0.0,
    }
}

fn dividir(a: int, b: int) -> Result {
    if b == 0 { return Result::Err("división por cero") }
    Result::Ok(a / b)
}

fn main() {
    // enteros de 64 bits en una máquina de 32
    let grande = 9007199254740993
    print("int64: {grande} * 3 = {grande * 3}")
    print("división/módulo 64: {grande / 7} {grande % 7}")
    print("negativos: {0 - grande / 1000}")

    // coma flotante
    let pi = 3.14159
    print("float: {pi * 2.0} {1.0 / 3.0} {std::math::sqrt(2.0)}")
    let pf = std::text::parse_float("1")
    let pi2 = std::text::parse_int("-42")
    print("parse: {pf} {pi2}")

    // structs, enums, match
    let p = Punto { x: 3, y: 4 }
    print(nombre(p))
    let a1 = area(Figura::Circulo(1.0))
    let a2 = area(Figura::Rect(3.0))
    let a3 = area(Figura::Vacio)
    print("áreas: {a1} {a2} {a3}")

    // Result / Option
    match dividir(10, 0) {
        Result::Ok(v) => print("ok {v}"),
        Result::Err(e) => print("error: {e}"),
    }
    match dividir(10, 3) {
        Result::Ok(v) => print("ok {v}"),
        Result::Err(e) => print("error: {e}"),
    }

    // arrays, mapas, closures
    let xs = [5, 3, 9, 1]
    let dobles = map(xs, |v| v * 2)
    let grandes = filter(xs, |v| v > 2)
    let suma = fold(xs, 0, |acc, v| acc + v)
    print("arrays: {dobles} {grandes} suma={suma}")
    let mut m = std::map::new()
    m = std::map::insert(m, "uno", 1)
    m = std::map::insert(m, "dos", 2)
    let tiene = std::map::contains(m, "dos")
    print("mapa: {m} tiene dos: {tiene}")

    // texto y unicode
    let s = "ñandú ARM32 ✓"
    print("texto: {std::text::uppercase(s)} chars={len(std::text::chars(s))} al revés={std::text::reverse(s)}")

    // hash y json
    let h = std::hash::sha256(std::encoding::utf8_encode("abc"))
    print("sha256(abc) = {h}")
    let j = std::json::parse("{\"a\": [1, 2, {\"b\": true}], \"c\": \"ñ\"}")
    print("json: {std::json::stringify(j)}")

    // tareas (fibras) y canales
    let (tx, rx) = channel(4)
    let t = spawn || {
        let mut i = 0
        while i < 4 { send(tx, i * i); i += 1 }
        4
    }
    let mut total = 0
    let mut k = 0
    while k < 4 { total += recv(rx); k += 1 }
    print("tareas: suma={total} enviados={join(t)}")

    print("ARM32 OK")
}
EOF
# Salida de este mismo programa en Linux x86-64 (backend propio) y en el emulador ARM32 (idénticas).
cat > kit.esperado.txt <<'EOF'
int64: 9007199254740993 * 3 = 27021597764222979
división/módulo 64: 1286742750677284 5
negativos: -9007199254740
float: 6.28318 0.3333333333333333 1.4142135623730951
parse: Option::Some(1) Option::Some(-42)
P(3, 4)
áreas: 3.14159 9 0
error: división por cero
ok 3
arrays: [10, 6, 18, 2] [5, 3, 9] suma=18
mapa: {dos: 2, uno: 1} tiene dos: true
texto: ÑANDÚ ARM32 ✓ chars=13 al revés=✓ 23MRA údnañ
sha256(abc) = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
json: {"a":[1,2,{"b":true}],"c":"ñ"}
tareas: suma=14 enviados=4
ARM32 OK
EOF

cat > metodos.titan <<'EOF'
struct Punto { x: int, y: int }
impl Punto {
    fn nombre(self) -> string { "P({self.x}, {self.y})" }
}
fn main() {
    let p = Punto { x: 3, y: 4 }
    print(p.nombre())
}
EOF

# ---------------------------------------------------------------- 3. check
titulo "3. titan check (solo front-end, sin clang)"
if correr salida_check.txt "$TITAN" check kit.titan; then
  mostrar salida_check.txt 3
  paso_ok "titan check kit.titan"
else
  mostrar salida_check.txt 20
  paso_falla "titan check kit.titan"
fi

# ---------------------------------------------------------------- 4. run hola
titulo "4. titan run hola.titan (la primera vez es la lenta: compila el runtime; paciencia, pueden ser minutos)"
if correr salida_hola.txt "$TITAN" run hola.titan; then
  mostrar salida_hola.txt 5
  if iguales hola.esperado.txt salida_hola.txt; then
    paso_ok "titan run hola.titan: salida idéntica a la esperada"
  else
    paso_falla "titan run hola.titan: corrió pero la salida difiere"; diferencias hola.esperado.txt salida_hola.txt
  fi
  titulo "4b. titan run hola.titan otra vez (con la caché del runtime ya hecha)"
  correr salida_hola2.txt "$TITAN" run hola.titan > /dev/null && paso_ok "segunda ejecución" || paso_falla "segunda ejecución"
else
  mostrar salida_hola.txt 30
  paso_falla "titan run hola.titan"
fi

# ---------------------------------------------------------------- 5. compile kit
titulo "5. titan compile kit.titan -o kit && ./kit"
rm -f kit
if correr salida_compile.txt "$TITAN" compile kit.titan -o kit; then
  mostrar salida_compile.txt 3
  log "   ejecutable: $(ls -la kit 2> /dev/null | awk '{print $5}') bytes; cabecera: $(od -An -tx1 -N20 kit 2> /dev/null | tr -d ' \n')"
  if correr salida_kit.txt ./kit; then
    mostrar salida_kit.txt 20
    if iguales kit.esperado.txt salida_kit.txt; then
      paso_ok "kit: las 16 líneas son idénticas a las de x86-64"
    else
      paso_falla "kit: corrió pero hay diferencias con x86-64:"; diferencias kit.esperado.txt salida_kit.txt
    fi
  else
    mostrar salida_kit.txt 30
    paso_falla "el ejecutable kit murió (código distinto de 0)"
  fi
else
  mostrar salida_compile.txt 30
  paso_falla "titan compile kit.titan"
fi

# ---------------------------------------------------------------- 6. métodos (limitación conocida)
titulo "6. titan run metodos.titan (p.nombre(): CallMethod en el backend LLVM)"
if correr salida_metodos.txt "$TITAN" run metodos.titan; then
  mostrar salida_metodos.txt 5
  paso_ok "¡métodos de impl funcionan en ARM32! (no se esperaba: CallMethod no estaba en el backend LLVM)"
else
  mostrar salida_metodos.txt 5
  if grep -q "CallMethod" salida_metodos.txt; then
    paso_limit "métodos de impl/trait (p.nombre()) no compilan en ARM: «CallMethod is not supported». Conocida; pendiente de arreglar en selfhost/native/llvm.titan"
  else
    paso_falla "metodos.titan falló por otra razón (mira arriba)"
  fi
fi

# ---------------------------------------------------------------- 7. proyecto
titulo "7. titan new proyecto && titan run"
rm -rf mi_app
if correr salida_new.txt "$TITAN" new mi_app; then
  if (cd mi_app && correr ../salida_proyecto.txt "$TITAN" run); then
    mostrar salida_proyecto.txt 5
    paso_ok "proyecto nuevo compilado y ejecutado"
  else
    mostrar salida_proyecto.txt 30
    paso_falla "titan run dentro del proyecto"
  fi
else
  mostrar salida_new.txt 10
  paso_falla "titan new mi_app"
fi

# ---------------------------------------------------------------- resumen
titulo "RESUMEN"
log "$resumen"
log ""
log "OK: $ok   FALLA: $falla   LIMITACIÓN CONOCIDA: $limit"
log "aparato: $(uname -m) · $(command -v getprop > /dev/null 2>&1 && getprop ro.product.model) · Android $(command -v getprop > /dev/null 2>&1 && getprop ro.build.version.release) · $(head -1 salida_version.txt 2> /dev/null)"
log ""
log "Informe completo: $INFORME  (pégalo entero en el chat o en un issue)"
log "Archivos de la prueba: $DIR"
[ "$falla" = 0 ]

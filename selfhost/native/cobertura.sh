#!/bin/bash
# Cobertura de la biblioteca estándar en el runtime nativo (fase 6).
# Compara la lista completa de nativas (selfhost/natives.titan, 813) con las
# funciones `std__modulo__nombre` alcanzables desde los imports de
# native/runtime.titan. Uso: bash selfhost/native/cobertura.sh [-v]
#   -v: lista también las que faltan, por módulo.
# Una función solo cuenta si el runtime COMPILA: antes de contar se ejecuta
# selfhost/revisar_runtime.titan (cargador + typechecker de Titan sobre
# native/runtime.titan). Si hay errores, el script termina con código 3 y no
# imprime ninguna cifra. SIN_REVISION=1 se salta la revisión (solo para
# depurar; esa cifra no se debe publicar).
#
# Con qué ejecutable se revisa: por defecto la CLI nativa selfhost/titan, que
# construye `bash selfhost/bootstrap.sh` (el `zett` de Rust ya no está en el
# repositorio, así que no se puede dar por sentado en el PATH). Se puede elegir
# otro con TITAN=/ruta/al/ejecutable; ZETT=… sigue aceptándose por compatibilidad
# con los guiones y la documentación que todavía usan ese nombre.
set -u
cd "$(dirname "$0")/.."
REVISOR="${TITAN:-${ZETT:-}}"
if [ -z "$REVISOR" ]; then
  if [ -x ./titan ]; then
    REVISOR=./titan
  else
    echo "cobertura: falta selfhost/titan: ejecuta antes bash selfhost/bootstrap.sh" >&2
    echo "           (o indica otro ejecutable con TITAN=/ruta/al/ejecutable)" >&2
    exit 2
  fi
fi
if [ "${SIN_REVISION:-0}" != "1" ]; then
  # La ruta del runtime se pasa explícita: revisar_runtime.titan ya no puede
  # deducirla de ar_program() (en un ejecutable nativo eso es el binario temporal).
  if ! out=$("$REVISOR" run revisar_runtime.titan "$PWD/native/runtime.titan" 2>&1) || [ "$out" != "ok" ]; then
    echo "cobertura: el runtime nativo NO compila; no se cuenta nada:" >&2
    echo "$out" | head -40 >&2
    exit 3
  fi
fi
all=$(grep -o '"std::[a-z_0-9]*::[a-z_0-9]*' natives.titan | tr -d '"' | sort -u)
have=$(python3 - "$PWD/native/runtime.titan" <<'PY'
import re
import sys
from pathlib import Path

root = Path("native").resolve()
start = Path(sys.argv[1]).resolve()
pending = [start]
seen = set()
while pending:
    source = pending.pop().resolve()
    if source in seen:
        continue
    seen.add(source)
    text = source.read_text(encoding="utf-8")
    for module in re.findall(r"^\s*import\s+([A-Za-z_][A-Za-z_0-9]*(?:::[A-Za-z_][A-Za-z_0-9]*)*)", text, re.M):
        relative = Path(*module.split("::")).with_suffix(".titan")
        candidates = (source.parent / relative, root / relative)
        imported = next((path.resolve() for path in candidates if path.is_file()), None)
        if imported is None:
            print(f"cobertura: no se encontró el import {module!r} de {source}", file=sys.stderr)
            raise SystemExit(2)
        pending.append(imported)

functions = set()
for source in seen:
    text = source.read_text(encoding="utf-8")
    functions.update(re.findall(r"^fn\s+(std__[a-z0-9_]+)\s*\(", text, re.M))
print("\n".join(sorted(functions)))
PY
) || exit $?
# Nativas que el compilador convierte en una instrucción propia (no en una
# función del runtime): cuentan si los DOS backends traducen esa instrucción.
for pair in std__try__catch:TryCall; do
  f=${pair%%:*}; op=${pair#*:}
  if grep -q "k == \"$op\"" native/backend.titan && grep -q "k == \"$op\"" native/llvm.titan; then
    have=$(printf '%s\n%s\n' "$have" "$f" | sort -u)
  fi
done
total=0; done=0
declare -A mt md
for n in $all; do
  m=${n#std::}; m=${m%%::*}
  f=$(echo "$n" | sed 's/::/__/g')
  total=$((total+1)); mt[$m]=$(( ${mt[$m]:-0} + 1 ))
  if echo "$have" | grep -qx "$f"; then done=$((done+1)); md[$m]=$(( ${md[$m]:-0} + 1 )); fi
done
for m in $(printf '%s\n' "${!mt[@]}" | sort); do
  printf '%-22s %3d / %3d\n' "$m" "${md[$m]:-0}" "${mt[$m]}"
done
echo "TOTAL: $done / $total nativas escritas en Titan"
if [ "${1:-}" = "-v" ]; then
  echo "--- faltan:"
  for n in $all; do f=$(echo "$n" | sed 's/::/__/g'); echo "$have" | grep -qx "$f" || echo "  $n"; done
fi

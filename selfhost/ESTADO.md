# Self-hosting de Titan — estado

Objetivo: que **todo** Titan (compilador, runtime y biblioteca estándar) esté
escrito en Titan y se compile a sí mismo, sin Rust. Reglas: nada falso, nada
simulado; cada paso se verifica con pruebas que cualquiera puede repetir.

## Fases

| # | Fase | Estado |
|---|------|--------|
| 0 | Preparar la VM de Rust para poder ejecutar un compilador escrito en Titan | ✅ hecho |
| 1 | **Lexer** en Titan (`selfhost/lexer.titan`) | ✅ idéntico al de Rust |
| 2 | **Parser** en Titan (`selfhost/parser.titan`) | ✅ idéntico al de Rust |
| 3 | **Typechecker** en Titan (`selfhost/typechecker.titan`) | ✅ idéntico al de Rust |
| 4a | **Cargador de `import`** y **generador de bytecode** en Titan (`selfhost/loader.titan`, `selfhost/codegen.titan`) | ✅ CI `37187789306` (`f7747b0`): compilador nativo = Zett precompilado, byte a byte, en 911/911 archivos `.titan` (incluidos diagnósticos) |
| 4b | bytecode → **ejecutable nativo** x86-64 + runtime en Titan (sin VM en Rust) (`selfhost/build.titan`, `selfhost/native/`) | ✅ funciona (con conteo de referencias y floats); la paridad funcional se registra aparte por batería y arquitectura |
| 5 | Titan se compila a sí mismo (punto fijo: etapa1 == etapa2 == etapa3 byte a byte) | ✅ CI `37187789306` (`f7747b0`): tres generaciones idénticas; etapas 2 y 3 sin Rust ni `PATH` (`selfhost/verify_fixpoint.sh`) |
| 6 | Biblioteca estándar y runtime en Titan; borrar el último `.rs` | ✅ **812/812** funciones `std::*` con cuerpo en Titan (`bash selfhost/native/cobertura.sh -v`); el código Rust se borró del repositorio (última versión con Rust: etiqueta `ultimo-con-rust`, `5dbb238`) |
| L | **Backend LLVM** en Titan: bytecode → LLVM IR → clang/llc (LLVM real) (`selfhost/native/llvm.titan`, `selfhost/build_llvm.titan`) | ✅ CI `37396772922` (`b30105d`): LLVM x86-64 y ARM64 en hardware real dieron **313/313 idénticos**, sin diferencias, timeouts ni casos no admitidos; se generaron los 313 objetos ARM64 sin rechazos. `audio_synthesis` es una prueba positiva: requiere código 0 en VM y nativo. La matriz también contiene las pruebas WAV; esto valida esos casos, no todas las ramas ni paridad global. El run anterior `37272142744` detectó un defecto del arnés: `image_gif_validate` y `image_webp_validate` recibían carpetas separadas de sus productores y fallaban ambos lados con código 1. `7a67de3` corrigió el directorio compartido y el orden determinista; el run nuevo pasó. El punto fijo por LLVM (`native/llvm/punto_fijo.sh`) también se ha verificado anteriormente. |
| O | **Optimizador propio** en Titan, estilo `opt` de LLVM, sobre el bytecode (la representación intermedia de Titan) (`selfhost/opt.titan`): cálculo de constantes, saltos encadenados, valores descartados, código inalcanzable. Lo usan los dos backends | ✅ en marcha: el compilador pasa de 74 550 a 69 358 instrucciones (−7 %); punto fijo propio y por LLVM ✅; `selfhost/opt_ver.titan ARCHIVO [--dump]` muestra el antes/después; `TITAN_OPT=0` lo apaga. Integración de funciones pequeñas (inlining) hecha y probada, cuenta la profundidad de llamadas igual que una llamada real; va apagada salvo con `TITAN_INLINE=1` porque medida sobre el compilador no lo acelera (ni con el backend propio ni con LLVM) y agranda el ejecutable |
| M | **Memoria del runtime**: medido con un perfilador por muestreo propio (`selfhost/native/perfil.py`, sin perf), el compilador pasaba casi la mitad del tiempo pidiendo y devolviendo bloques grandes al sistema. Ahora esos bloques se reutilizan (listas por tamaño, potencias de 2) | ✅ compilador hecho por LLVM: 6,5 s → 4,3 s (−34 %); con el backend propio: 41,4 s → 31,0 s (−25 %); misma salida byte a byte; puntos fijos ✅; prueba `memoria_grande.titan` |
| U | **Última lectura (moves)** en el optimizador: análisis de vida de las variables; la última lectura entrega el valor sin copiar (`TakeLocal`), así arrays/mapas/textos quedan con un solo dueño y se amplían en el sitio. Se aplica a los programas, no al runtime (que guarda direcciones en enteros). El perfilador ahora sube por la cadena de llamadas (`--llamador`, `--solo`) y el backend propio escribe en el mapa sus rutinas internas. Con eso se arreglaron las copias enteras en `pc_add`/`pc_ref`/`pc_label`, `cg_emit`/`cg_patch`/`cg_intern` y el optimizador; y el backend propio compara en línea la longitud de dos strings | ✅ compilador con el backend propio: 42 s → 26 s (−38 %); con LLVM: 4,3 s → 4,0 s; mismo binario byte a byte; bytecode idéntico al de Rust en 771/771 archivos; puntos fijos ✅; prueba `ultima_lectura.titan` |
| S | **Sin sistema operativo** (objetivo `aarch64-none`): el programa arranca solo en una máquina ARM64 (la "virt" de QEMU): `_start` de `std::freestanding`, coma flotante, **MMU con tabla de páginas de verdad** (bloques de 1 GiB: dispositivos sin ejecutar y RAM normal con caché), puerto serie PL011 para la salida y la entrada, memoria propia desde el final del programa, reloj con el contador del procesador y apagado por PSCI (con el código de salida). Llamadas sin equivalente sin sistema operativo (archivos, procesos, red, azar): -ENOSYS de verdad (`native/sys_bare.titan`). Primitivas nuevas del runtime: `std::raw::mmio_load32/mmio_store32` (volátiles), `sysreg/set_sysreg`, `hvc`, `bare` | ✅ CI `37182816733`: QEMU ARM64 `virt`, 137/137 idénticos en salida y código de salida; Unicorn, 136/137 ejecutados idénticos y un caso `.solo-qemu` omitido porque prueba fallos reales de memoria. No hubo diferencias entre los casos ejecutados. |

## Cómo verificar

```bash
# `scripts/sandbox-zett.sh` no existe en el repo: baja `zett-linux-x86_64` (+ .sha256) de la rama `binaries`
# (git clone --depth 1 -b binaries ...), verifica el sha256 y ponlo en ~/.local/bin/zett. Necesita libasound.so.2.
# Comprobación rápida de que el runtime nativo compila (~90 s en la VM): zett run selfhost/revisar_runtime.titan
export PATH="$HOME/.local/bin:$PATH"
bash selfhost/verify_lexer.sh            # lexer Titan vs lexer Rust, byte a byte
bash selfhost/verify_parser.sh           # parser Titan vs parser Rust, byte a byte
bash selfhost/verify_typechecker.sh      # typechecker Titan vs Rust, byte a byte
bash selfhost/verify_codegen.sh          # cargador + codegen Titan vs Rust, byte a byte
bash selfhost/native/verify_x64.sh       # codificador x86-64 en Titan vs el ensamblador GNU
bash selfhost/native/verify_native.sh    # nativo vs `zett run` (stdout, stderr, código y archivos de imagen)
bash selfhost/native/verify_native.sh selfhost/tests/native/redis.titan # arranca un Redis real si redis-server/redis-cli están instalados
bash selfhost/native/verify_json.sh      # std::json::parse nativo vs VM en 97 casos (errores con línea/columna)
bash selfhost/native/cobertura.sh [-v]   # definiciones Titan frente al inventario registrado
python3 selfhost/native/auditar_pruebas.py # cruce de firmas con nombres citados en la suite (no mide ramas)
ZETT=/ruta/al/zett COMPILER=/ruta/titanc1 python3 selfhost/native/readline/verificar.py # PTY real
ZETT=/ruta/al/zett COMPILER=/ruta/titanc1 bash selfhost/native/term/verificar.sh # PTY real
ZETT=/ruta/al/zett COMPILER=/ruta/titanc1 python3 selfhost/native/http_full/verificar.py # HTTP/TLS local
# El compilador Titan convertido en ejecutable nativo, contra el de Rust:
zett run selfhost/build.titan selfhost/bytecode.titan /tmp/bytecode_nativo
SELF=/tmp/bytecode_nativo bash selfhost/verify_codegen.sh
bash selfhost/verify_fixpoint.sh         # punto fijo: titanc1 == titanc2 == titanc3
bash selfhost/native/llvm/verificar.sh   # backend LLVM: ejecutables por LLVM vs `zett run` (ver native/llvm/NOTAS.md)
bash selfhost/native/llvm/punto_fijo.sh  # el compilador hecho por LLVM da el mismo compilador, byte a byte
# Perfil de un ejecutable nativo (herramienta de desarrollo):
zett run selfhost/build.titan PROG.titan /tmp/prog /tmp/prog.map
python3 selfhost/native/profile.py /tmp/prog.map /tmp/prog ARGS...
zett run ci-bench/bench.titan            # rendimiento de la VM
```

`zett tokens ARCHIVO` (comando oculto) imprime el volcado canónico del lexer
de Rust; `zett run selfhost/tokens.titan ARCHIVO` el del lexer en Titan.
`zett ast ARCHIVO` (oculto) imprime el AST del parser de Rust en un formato
propio y determinista (`crates/titan_parser/src/dump.rs`, no el `Debug` de
Rust); `zett run selfhost/ast.titan ARCHIVO` el del parser en Titan.

El workflow `.github/workflows/rust-check-logs.yml` compila y prueba todo el
workspace en cada push a `arena/**` y deja la salida real en `ci-logs/`
(los logs de Actions no son descargables desde el sandbox; git sí).

## Fase 0 — defectos reales corregidos en la VM (Rust)

Medido con `ci-bench/bench.titan`, 20.000 elementos:

| Operación | Antes | Después |
|---|---|---|
| `acc = std::array::push(acc, i)` | 18.143 ms | 13 ms |
| leer `acc[i]` | 15.340 ms | 10 ms |
| pasar un array a una función | 33.745 ms | 14 ms |

1. **Cada lectura de una variable copiaba el valor entero** (arrays, mapas,
   structs). Ahora `Value::Array/Tuple/Map/Struct` usan `Shared<T>`
   (`crates/titan_vm/src/shared.rs`): `Arc` + copy-on-write. Misma semántica de
   valor, lectura O(1).
2. **Cada llamada clonaba el bytecode completo de la función.** El módulo
   vive ahora en un `Arc` y se toma prestado.
3. **`x = std::array::push(x, v)` / `s += v` copiaban el contenedor.** Nueva
   instrucción `TakeLocal` (mueve el valor; solo se emite si el resto de la
   expresión no lee la variable, análisis conservador `expr_mentions`).
4. **Comparar enteros con `<`/`>` los convertía a `f64`**: resultados
   incorrectos por encima de 2^53. Ahora se comparan como enteros; además se
   admiten `char` y `string` (antes el typechecker lo aceptaba y la VM fallaba).
5. **Un `if/else` en posición de sentencia fallaba si sus ramas terminaban en
   tipos distintos**, contra lo que dice `docs/TITAN_SYNTAX.md`. Corregido.
6. Nuevas nativas para el lexer: `std::text::chars`, `char_code`,
   `from_char_code`, `is_alphabetic`, `is_alphanumeric`, `is_whitespace`.

Todo con tests en `crates/titan_vm` y `crates/titan_typechecker`; CI verde.

## Fase 1 — lexer

- `selfhost/lexer.titan`: traducción fiel de `crates/titan_lexer/src/lib.rs`.
- Verificación: 89 archivos (todo `.titan` del repo + casos límite de
  `selfhost/tests/lexer/`), 67.681 líneas de tokens, **idénticos**.
- Prueba de mutación: dos errores introducidos a propósito fueron detectados.

## Fase 2 — parser

- `selfhost/parser.titan`: traducción fiel de `crates/titan_parser/src/lib.rs`
  (Pratt + descenso recursivo), con todas sus desazucaraciones (`let (a, b)`,
  `let P { x }`, `for (a, b) in`, `|>`, `<=>`, `[..a, b]`, `#{ k: v }`), los
  mismos mensajes de error y la misma recuperación (sigue en la siguiente
  declaración y reporta todos los errores).
- Verificación: 98 archivos, 58.971 líneas de AST, **idénticos**. Incluye
  `selfhost/tests/parser/` (todas las construcciones, 256 pares de
  precedencia, errores con Unicode, recuperación).
- Prueba de mutación: 4 errores introducidos a propósito. El primero (cambiar
  la precedencia de `^`) **no** se detectó al principio: ningún archivo lo
  ejercitaba. Se añadió `precedence.titan` y ahora los 4 se detectan.
- El parser en Titan analiza su propio código (2.273 líneas) en ~1 s.

Defectos reales del Titan en Rust encontrados en esta fase (corregidos):

1. **La recursión abortaba el proceso hacia las ~450 llamadas anidadas**
   ("stack overflow"), aunque el límite documentado era 4096 con un error
   limpio. Ahora la pila nativa crece (`stacker`, lo mismo que usa rustc) y
   el límite de 4096 es real. Test: `deep_recursion_reaches_the_call_depth_limit_without_crashing`.
2. **Todo `titan run` se cortaba tras 10 millones de instrucciones** (~0,5 s
   de cálculo). Ese presupuesto es para código no confiable: ahora se aplica
   solo con `--sandbox` (documentado en `docs/SPEC.md` §6).
3. Nueva nativa `std::text::is_uppercase` (el parser la necesita para
   distinguir `Punto { x: 1 }` de un bloque).

## Fase 3 — typechecker

- `selfhost/typechecker.titan` (~4.700 líneas): traducción fiel de
  `crates/titan_typechecker/src/lib.rs`: declaraciones duplicadas, tipos
  desconocidos, alias recursivos, traits/impls, inferencia del tipo de
  retorno (por rondas, igual que Rust), closures y callbacks, interpolación
  de texto, llamadas a nativas/variantes/métodos, cobertura de `match`
  (enums, bool, literales), análisis de "siempre devuelve"/"puede salir del
  bucle" y los mismos mensajes en el mismo orden.
- `zett typecheck ARCHIVO` (oculto) imprime `ok` o una línea `diag:` por
  diagnóstico (typechecker de Rust); `zett run selfhost/check.titan ARCHIVO`
  lo mismo con el de Titan.
- Firmas de las 816 nativas: `selfhost/natives.titan`, generado por
  `selfhost/gen_natives.sh` desde `zett natives`. **Es temporal**: cuando la
  biblioteca estándar esté en Titan (fase 6) las firmas saldrán de ella.
- Verificación: 448 archivos, 2.737 líneas de diagnósticos, **idénticos**.
  Incluye `selfhost/tests/typechecker/de_rust/` (los 341 programas de los
  tests de Rust, guardados para cuando se borre Rust) y 7 archivos propios.
- Fuzzing diferencial: 2.100 programas mutados al azar (que Rust acepta o
  rechaza con diagnósticos): **todos idénticos**.
- Prueba de mutación: se plantaron errores en el typechecker de Titan. Con
  solo los tests de Rust, 4 de 6 **no** se detectaban; se escribieron
  `colecciones`, `flujo`, `nativas`, `patrones`, `plantillas`, `llamadas` y
  `varios.titan` y ahora se detectan todos. Tres errores plantados resultaron
  ser "equivalentes" (no cambian nunca el resultado, p. ej. un `loop` sin
  `break` ya es de tipo `Never` por otro camino) y se sustituyeron.

Defectos reales del Titan en Rust encontrados en esta fase:

1. (corregido) Un `impl` de un método que no existe en el trait daba un error
   equivocado ("unknown variable"); ahora `MissingTraitMethod`.
2. (corregido) `impl Nope for A` con un trait inexistente decía
   "unknown variable or function 'trait 'Nope''"; ahora `unknown trait 'Nope'`.
3. (corregido) Los tipos de un `impl` inválido se mostraban con el `Debug`
   de Rust (`Array(Named(...))`); ahora en sintaxis Titan (`[int]`).
4. (corregido) Leer de un mapa copiaba el mapa entero: 20.000 lecturas
   tardaban 31,9 s; ahora 13 ms.
5. (corregido) El orden de los errores cambiaba entre ejecuciones.
6. (corregido 2026-10-10) Un `if` con ramas de tipos distintos, como **última**
   expresión del cuerpo de un `for`, daba "type mismatch"; en cualquier otro
   sitio se aceptaba. Ahora el valor del cuerpo de un `for`/`while`/`loop` se
   tipa en posición de sentencia (se descarta): aceptado y con pruebas en
   `selfhost/prueba_arreglos.titan`. El `zett` congelado sigue con el defecto
   (divergencia deliberada, ver verify_typechecker.sh).
7. (pendiente) Varios errores dentro de `main` salen con posición `1:1` o la
   de la función en lugar de la línea real.
8. (pendiente) `zett check`/`zett run` imprimen cada error de compilación dos
   veces.

## Fase 4a — cargador de `import` y generador de bytecode

- `selfhost/codegen.titan` (~1.560 líneas): traducción fiel de
  `crates/titan_codegen/src/lib.rs`: resolución de nombres (local, constante
  con detección de ciclos, variante, función), llamadas (nativas, variantes,
  métodos, las 120 instrucciones dedicadas, `std::try::catch`), asignaciones
  compuestas con `TakeLocal`, `for`/`while`/`loop` con `break`/`continue`,
  `match` con patrones anidados, plantillas `"{x}"`, closures con capturas y
  la tabla de métodos/structs/enums del módulo.
- `selfhost/codegen_tables.titan`: las 120 instrucciones dedicadas, generada
  por `selfhost/gen_codegen_tables.py` a partir del código Rust.
- `selfhost/loader.titan`: traducción de `crates/titan_pkg/src/project.rs`
  (`import a::b` → `a/b.titan` o `a/b/mod.titan`, cada archivo una vez,
  ciclos, módulos de `stdlib/`, mismos mensajes de error).
- `zett bytecode ARCHIVO` (oculto, `crates/titan_cli/src/bytecode_dump.rs`)
  imprime el módulo compilado por Rust en un formato estable;
  `zett run selfhost/bytecode.titan ARCHIVO` lo mismo con el cargador,
  typechecker y generador de Titan.
- El typechecker de Titan ahora sabe en qué archivo está cada error (como
  Rust cuando carga un proyecto): `archivo:l:c: mensaje`.
- Verificación reproducible más reciente: en CI `37187789306` para `f7747b0`, el compilador nativo coincidió con el Zett precompilado en los **911 archivos `.titan`** del repositorio, incluidos los diagnósticos de error. El paso verifica el seed precompilado y usa `SELF` nativo; no invoca Cargo.
- **El compilador en Titan se compila a sí mismo**: `selfhost/bytecode.titan`
  (con todo lo que importa) da 53.845 líneas de bytecode, idénticas a las de
  Rust.
- Prueba de mutación: 8 errores plantados. 7 se detectaban; el que no (el
  archivo de los errores de declaración) llevó a añadir
  `tests/codegen/decl/` y ahora se detectan los 8.

Defectos reales del Titan en Rust encontrados en esta fase (pendientes; el
código en Titan los reproduce a propósito para que la comparación sea exacta
y se corregirán en los dos a la vez):

9. Los errores de declaración (p. ej. función duplicada) salen con el archivo
   de la **última** función del programa, no con el suyo:
   `tests/codegen/decl_wrong_file/` dice `main.titan:5:1` cuando el duplicado
   está en `dup.titan:5:1`.
10. (Resuelto el 2026-10-08) El cargador de Titan ya admite proyectos con
    `Titan.toml`; ver «Titan.toml en el cargador».
11. `std::uuid::nil()` no se podía llamar desde Titan: `nil` es palabra clave
    y el parser la rechazaba después de `::` ("expected an identifier, found
    Nil"). La nativa existía pero era inalcanzable.

    **Corregido en los dos lados** (junto con el 16): después de `::` una
    palabra clave no puede empezar otra cosa, así que ahora se lee como
    nombre. Rust: `expect_path_segment` en `titan_parser/src/lib.rs` (con
    test); Titan: lo mismo en `selfhost/parser.titan`. Fuera de una ruta
    siguen siendo palabras clave (`let nil = 1` sigue siendo un error).
    Prueba: `tests/native_pendiente_vm/ruta_palabra_clave.titan`.
12. **`std::datetime` hacía *panic* (la VM se caía con un mensaje interno de
    Rust)** en dos casos, en vez de dar un error normal:
    - `format` / `format_offset` con un formato no válido (`%Q`, `%-D`,
      `%.4f`, `%` al final, o `%#z`, que solo sirve para leer): "a Display
      implementation returned an error unexpectedly".
    - `to_rfc2822` con años fuera de 0..=9999: "date cannot be represented by
      RFC 2822".

    **Corregido en los dos lados.** En Rust (`datetime_mod.rs`) ahora son
    errores: `invalid format string '<fmt>': bad or unsupported format string`
    (el tipo de error ya existía y no se usaba) y `timestamp <ts> cannot be
    represented by RFC 2822 (years 0 to 9999 only)`, con tests. En Titan
    (`std_datetime.titan`) se dan los mismos mensajes. Los programas que lo
    prueban están en `tests/native/` (comprobados con el `zett`
    reconstruido, ver abajo).
13. **`std::dirs` hacía *panic*** al pedir cualquier carpeta de usuario
    (`desktop`, `documents`, `music`…) si `~/.config/user-dirs.dirs` tenía
    una línea `XDG_DIR=...` o un valor que era solo `"`: el crate dirs-sys
    0.4.1 corta la cadena con índices al revés (`xdg_user_dirs.rs:35` y
    `:54`). Una sola línea así hacía caer el programa.

    **Corregido en los dos lados.** En Rust (`dirs_mod.rs`) la lectura de
    `user-dirs.dirs` ya no usa el crate en Linux/Android: mismas reglas, pero
    esas líneas se ignoran como cualquier otra que no se entiende (con tests).
    En Titan (`std_dirs.titan`) igual. La prueba está en
    `tests/native/dirs_lineas_malas.titan`.
14. **`std::xml::parse` perdía datos sin avisar** si el documento terminaba
    con etiquetas abiertas: quick-xml no da error en ese caso y `xml_mod`
    devolvía solo la última etiqueta abierta (`parse("<a><x/><b>hola")` daba
    `{tag: b, text: hola}`: se perdían `<a>` y `<x/>`).

    **Corregido en los dos lados.** Ahora es un error, con el mensaje que
    quick-xml usa para ese caso: `ill-formed document: start tag not closed:
    `</b>` not found before end of input` (con test en `xml_mod.rs`). La
    prueba está en `tests/native/std_error_xml_abierta.titan`.
15. **`std::term::print_colored("#a€bc", …)` hacía *panic*** (corte de
    string dentro de un carácter). Corregido en `term_mod.rs` y en
    `std_term.titan` (ver la fila de std::term en la fase 6).
16. **`std::process::spawn` no se podía llamar desde Titan**: `spawn` es
    palabra clave (para `spawn f()`), igual que el 11. Sin `spawn`, las
    funciones `spawn_wait`, `spawn_poll`, `spawn_kill` y `spawn_pid` tampoco
    servían (no hay forma de conseguir un número de proceso). Corregido con
    el 11.
17. **`std::process::env_set` / `env_unset` hacían *panic*** (la VM se caía)
    con un nombre vacío, con `=` o con un byte NUL, o con un valor con NUL:
    `std::env::set_var` de Rust hace panic si el sistema lo rechaza.
    **Corregido en los dos lados**: ahora es un error normal (`invalid
    environment variable: name must be non-empty and must not contain '=' or
    NUL` / `... value must not contain NUL`), con test en `process_mod.rs`.
18. **`std::process::env_vars` hacía *panic*** si alguna variable de entorno
    no era UTF-8 válido (`std::env::vars()` de Rust). **Corregido en los dos
    lados**: esas variables se saltan, igual que `env_get` da None para ellas
    (test en `process_mod.rs`).

    Los tres (16, 17, 18) se comprobaron con el `zett` reconstruido por CI
    (commits 9a7d45d y 343f52f): la VM de Rust y el nativo dan lo mismo.

**Comprobación con Rust reconstruido (2026-09-28).** El workflow *Publish
Linux x86_64 binary* recompila `zett` desde el código Rust en cada push, y
`scripts/sandbox-zett.sh` descarga ese binario (rama `binaries`). Con el
compilado del commit 9c3b3cb, las pruebas de los bugs 12, 13 y 14 dan
**idéntico** entre la VM de Rust corregida y el ejecutable nativo (5 de 5;
la del 13 también con `XDG_CONFIG_HOME` apuntando al archivo malo), y el
error del 15 sale igual en los dos. Esas pruebas pasaron de
`tests/native_pendiente_vm/` a `tests/native/`.

## Fase 4b — compilador nativo (bytecode → ejecutable x86-64)

`zett run selfhost/build.titan PROGRAMA.titan SALIDA` genera un ejecutable
ELF de Linux x86-64 que no usa Rust ni la VM. Piezas, todas en Titan:

- `native/x64.titan`: codificador de instrucciones x86-64 (verificado contra
  el ensamblador GNU: 1412 instrucciones idénticas).
- `native/elf.titan`: el formato de archivo ejecutable ELF.
- `native/backend.titan`: traduce cada instrucción del bytecode a código
  máquina (pila de valores de 16 bytes: etiqueta + contenido).
- `native/runtime.titan`: el "motor" que va dentro de cada ejecutable
  (memoria, strings, arrays, mapas, comparaciones, impresión, errores y las
  nativas usadas hasta ahora: `std::map`, `std::array`, `std::text`,
  `std::fs`, `std::path`, `std::env`, `sort_by`), escrito en Titan y
  compilado por el mismo compilador. Es el único que puede usar las
  operaciones de bajo nivel `std::raw::` (leer/escribir memoria, llamadas
  al sistema Linux).
- `native/unicode.titan`: tablas Unicode (letras, dígitos, mayúsculas,
  espacios) **generadas** por `native/gen_unicode.titan` preguntando al
  Titan actual por cada uno de los 1,1 millones de caracteres; así el
  ejecutable nativo responde exactamente igual. Son datos, no lógica.
- `args.titan`: las herramientas leen sus argumentos igual con la VM que
  como ejecutable nativo.

Resultados (sesión 4 de la fase 4b):

- `verify_native.sh`: 11/11 programas de `tests/native/` idénticos a la VM
  (salida, errores de ejecución —desbordamiento, índice, división por cero,
  recursión, archivo inexistente, carácter inválido— y código de salida).
- **El compilador Titan (`bytecode.titan`: cargador + lexer + parser +
  typechecker + generador) compilado a ejecutable nativo** (1,7 MB) produce
  un resultado idéntico al de Rust en **433 de 484** archivos.
- (Sesión 5) **Conteo de referencias** como el `Rc` de la VM: cada lugar
  que guarda un valor tiene una referencia; al llegar a 0 la memoria se
  libera y se reutiliza (listas de bloques libres por tamaño), y con un solo
  dueño `push`/`set`/`insert`/`+=` modifican en el sitio (`Rc::make_mut`).
  Reglas de las operaciones `std::raw::` (tag/bits/keep/value/adopt/
  retain/release) al principio de `native/runtime.titan`.
- Rutas rápidas en línea para `x.campo` y `a[i]`; perfilador por muestreo
  (`native/profile.py`, con ptrace) y mapa de símbolos opcional
  (`build.titan PROG SALIDA MAPA`).
- Velocidad: el compilador Titan nativo compila `selfhost/bytecode.titan`
  (todo el compilador) en 6,7 s; el mismo compilador en la VM tarda 14,2 s.
  (Medido en otra sesión; en la máquina de la sesión de floats tanto la
  versión anterior como la nueva tardan ~25 s: la máquina era más lenta,
  no el código.)
- **Floats** (`native/float.titan`, todo con enteros exactos, sin tablas de
  Rust):
  - texto → float como `str::parse::<f64>` (redondeo al más cercano, empate
    al par; subnormales, inf, nan). Camino rápido de Clinger (≤15 dígitos,
    |exp| ≤ 22: una sola operación SSE exacta) y si no, división exacta con
    enteros grandes;
  - float → texto como `f64::to_string`: Grisu con números de 62 bits
    (tabla de potencias de diez `native/float_table.titan`, generada por
    `native/gen_float_table.titan` con enteros exactos) y, cuando Grisu no
    puede garantizar el resultado (~1,4 %), Dragon exacto con las mismas
    reglas de límites que `core::num::flt2dec`;
  - aritmética (`addsd`/`subsd`/`mulsd`/`divsd`), comparaciones (`ucomisd`;
    NaN nunca es igual ni ordenado), negación, división por 0.0/-0.0 = error.
  - Comprobado: 20 000 textos aleatorios (VM contra float.titan) y 3 000×5
    valores en un ejecutable nativo contra la VM: 0 diferencias.
  - Velocidad: 20 000 conversiones ida y vuelta en 0,43 s (la VM: 0,03 s);
    lo que queda es el coste general de listas/textos del runtime.
- `verify_native.sh`: las primeras corridas encontraron diferencias reales en WebP (tamaños RIFF y entradas truncadas; corregidas en `18c2b3b`) y en la expectativa de una cabecera `JUNK` (actualizada en `046a8c8`). La corrida más reciente, `37396772665` (`b30105d`), usó el Zett precompilado verificado y terminó con bootstrap, codegen, los cuatro shards nativos, clipboard y Readline exitosos. Los 313 programas de la matriz dieron salida, errores y código idénticos; `audio_synthesis` exige código 0 y prueba `saw_wave`, incluida en los casos de onda cuadrada, períodos fraccionarios/cortos/subnormales y frecuencias cero/negativa. Esto valida los casos probados, no todas las ramas ni entrada física de dispositivos.
- Termux AArch64 del PR `37396776106`, job `112054523721`: falló el paso `Build, verify, and package`; setup, checkout, Rust/NDK y caché habían pasado. El push del mismo SHA sí pasó (job `112054512006`). Las anotaciones solo muestran código 1 y la descarga del log falla con EOF incluso al reintentarla, así que la causa sigue desconocida; no atribuyo el fallo al código ni a la infraestructura. `scripts/build-termux-ci.sh` ejecuta `cargo build`; no se reejecutó, de acuerdo con la regla de no compilar Titan con Cargo.
- El generador de bytecode con el compilador nativo coincidió con el seed de Rust en **911/911** archivos `.titan` en CI `37187789306` (`f7747b0`), con comparaciones byte a byte.

## Fase 5 — punto fijo del bootstrap ✅

`bash selfhost/verify_fixpoint.sh`:

1. La VM de Rust ejecuta `selfhost/build.titan` (el compilador en Titan)
   sobre sí mismo → `selfhost/titanc1` (ejecutable x86-64 de 5,6 MB).
2. `titanc1`, **sin Rust ni VM y con el entorno vacío** (sin PATH: no puede
   llamar a `zett` ni a ningún otro programa), se compila → `titanc2`.
3. `titanc2` se compila → `titanc3`.

Resultado más reciente: CI `37278754059` para `7a67de3` pasó el bootstrap de tres generaciones y la comparación byte a byte. La primera etapa usa el Zett precompilado verificado; las etapas 2 y 3 ejecutan con `PATH=/nonexistent`, sin Rust, VM ni `std::process`. No reutilizo el hash de una sesión anterior porque el compilador ha cambiado desde entonces.

La fase 6 sigue en curso: `bash selfhost/native/cobertura.sh` cuenta ahora 740/812 firmas con cuerpo Titan/intrínseco (solo si el runtime compila); quedan 116 atendidas por Rust. Este recuento es estático: no certifica paridad conductual ni equivale a self-hosting completo; la matriz nativa y sus resultados por arquitectura se anotan por separado. También quedan componentes Rust del CLI por sustituir.

- Limitaciones anotadas: `std::path::parent`/`canonical` siguen las reglas
  de `Path` de Rust para los casos habituales; casos raros (p. ej. rutas
  con bytes no UTF-8) no están probados.

## Fase 6 — biblioteca estándar en Titan (en curso)

Cada módulo se escribe en Titan dentro del runtime (`selfhost/native/`) y se
compara con la VM de Rust con programas de `selfhost/tests/native/` (salida,
mensaje de error y código de salida idénticos). El enlazador solo mete en cada
ejecutable las funciones del runtime que usa (lista de trabajo en `bk_build`).

**Cruce estático actual** (`cobertura.sh` + `auditar_pruebas.py`): 816 firmas
nativas registradas; 668 tienen definición Titan o intrínseco; la matriz
contiene 326 programas `.titan` y ocho pruebas suplementarias (PTY, terminal,
HTTP/TLS y las cinco pruebas bare-metal). Los archivos de prueba citan 649
nombres de API, todos con definición; 167 firmas registradas no tienen mención
directa y 42 cuerpos Titan no aparecen citados. Esto mide inventario y
referencias, no ramas ejecutadas ni equivalencia de comportamiento. No se
afirma paridad para las funciones solo cubiertas estáticamente.

| Módulo | Archivo | Notas |
|---|---|---|
| std::text (28) | `std_text.titan` | mayúsculas/minúsculas Unicode con tablas generadas (`gen_unicode_case.titan`), sigma final |
| array, map, bytes, encoding, checksum | `std_core.titan` | hex/base64/percent/utf-8 con los mensajes de Rust |
| math exacto (sqrt, floor, ceil, round, abs, to_int/to_float) | `std_core.titan` | instrucciones SSE `sqrtsd`/`cvttsd2si` |
| time, path, fs | `std_core.titan` | llamadas al sistema directas; errores con el texto de `strerror` de glibc (`errno.titan`, generado por `gen_errno.py`) |
| std::hash (SHA-256/384/512, SHA3-256/512, BLAKE3, HMAC) | `std_hash.titan` | desde las especificaciones; constantes calculadas por `gen_hash_consts.py` (raíces de primos, LFSR de Keccak) |
| std::random | `std_random.titan` | con semilla: idéntico a rand 0.9 + rand_chacha (PCG32 → ChaCha20, método de Canon); sin semilla: `getrandom` |
| std::json | `std_json.titan` | el mismo analizador que serde_json 1.0.150 (mensajes y línea/columna; números como serde, que no siempre redondea exacto: `9007199254740993.0` → `…994`); floats de salida como zmij (`1e+21`) |
| std::csv, std::uuid, std::stats | `std_misc.titan` | csv como el crate `csv` (comillas, CRLF, filas vacías); uuid v4/v7 con `getrandom` y el reloj; mean/median/quantile/variance/stddev con los mismos mensajes |
| std::collections (57) | `std_collections.titan` | set, deque, cola de prioridad, mapa ordenado, contador y grafo con número identificador; las mismas cuotas que la VM (256 identificadores, 65536 entradas, 16 MiB, 4096 por estructura, 64 KiB por elemento) y los mismos órdenes de salida (Dijkstra con los desempates del `BinaryHeap` de Rust) |
| std::datetime (49) | `std_datetime.titan` | calendario gregoriano proléptico (−262143 a 262142), formatos `%…` con el mismo tokenizador que `StrftimeItems` de chrono 0.4.45, lectura con el mismo algoritmo de `Parsed` (semanas ISO, `%U`/`%W`, `%s`, segundos intercalares, `%C`/`%y`) y los mismos 7 errores; RFC 3339/2822; aritmética de datetime_ext (con vuelta en el desborde, como Rust en release). Comparado además con 12.000 casos aleatorios y 89 casos límite escritos a mano: 0 diferencias. |
| zonas horarias: `to_timezone`, `timezone_offset_seconds` | `std_tz.titan`, `std_tz_data.titan` (generado por `native/tz/gen_tz.py`) | lo mismo que chrono-tz 0.10.4: su base es tzdb 2025b sin backzone. Se parte del `tzdata.zi` 2026b del sistema (guardado en `native/tz/`), se deshacen los 3 cambios de datos de 2025c–2026b (Tijuana, Chisinau, Vancouver) y se añaden los enlaces de `backward` de 2025b: 597 nombres, los mismos que acepta la VM. El runtime guarda solo las reglas (~160 KB, que solo entran en los ejecutables que usan zonas) y calcula la tabla de cada zona la primera vez con el mismo algoritmo que parse-zoneinfo 0.5 (años 1800–2099, mismos desempates, fusión "optimise", primer tramo). `gen_tz.py` contiene también ese algoritmo en Python como referencia. Comparado con la VM: 223.599 consultas desde Python y 131.937 desde un programa nativo (todos los nombres, instantes aleatorios, extremos de i64, horas LMT con segundos como Caracas −4:27:44): 0 diferencias |
| std::vector (8) | `std_vector.titan` | aritmética en f32 como la VM (suma que empieza en −0,0); comparado con 6 semillas de casos aleatorios |
| std::metrics (8), std::testing (2) | `std_metrics.titan` | nombres, cuotas (4096) y mensajes iguales; contador u64 con saturación; salida Prometheus e instantánea como la VM |
| std::dirs (18) | `std_dirs.titan` | reglas de dirs 5.0.1 / dirs-sys 0.4.1 en Linux (variables XDG absolutas, `user-dirs.dirs` con el mismo analizador), `temp_dir` y `current_dir` de Rust, rutas no UTF-8 con U+FFFD como `to_string_lossy`. Sin `HOME` se lee `/etc/passwd` (la fuente "files" de glibc; LDAP/sssd no se consultan). Comparado en 250 entornos aleatorios (sin HOME, HOME vacío o no UTF-8, XDG relativos, varios `user-dirs.dirs`, TMPDIR vacío): 0 diferencias |
| std::xml (4) | `std_xml.titan` | el mismo lector que quick-xml 0.36.2 (BOM, texto recortado, comentarios, CDATA, DOCTYPE con balance de `<`/`>`, instrucciones `<?…?>`, `>` dentro de comillas, nombres de cierre literales, atributos con los mismos errores y posiciones, entidades y `&#…;` con los mismos mensajes) y el mismo escritor; comparado con 6.300 documentos aleatorios (válidos, rotos y cortados): 0 diferencias. Un XML de 2,3 MB tarda ~0,95 s frente a ~0,49 s en la VM (casi todo es el asignador de memoria) |
| std::jwt (5) | `std_jwt.titan`, `std_rsa.titan` | lo mismo que jsonwebtoken 9.3.1: base64url (con los mismos errores y posiciones que base64 0.22), lector JSON al estilo serde (cabecera con `Jwk`, enums, `untagged`, mismos mensajes y columnas), validación de `exp`/`aud`/`iss`; PEM (pem 3.0), ASN.1 (simple_asn1 0.6) y RSA escrito desde cero en Titan: comprobación de claves como ring 0.17 (mismos errores: `InconsistentComponents`, `TooLarge`, `PrivateModulusLenNotMultipleOf512Bits`…), firma PKCS#1 v1.5 + SHA-256 con CRT y verificación. Comparado con ~19.000 tokens HS256 y ~7.000 casos RS256 (claves de openssl de 1024 a 8192 bits, PKCS#1/PKCS#8/certificado/EC/Ed25519, DER alterado): 0 diferencias; openssl confirma que las firmas son válidas |
| std::math trascendentes (7: exp, ln, log, sin, cos, tan, pow) | `std_libm.titan` (generado desde `libm/std_libm.titan.in` por `libm/gen_libm.py`), `std_libm_tab.titan` | port bit a bit de la libm de glibc 2.36 (`e_exp.c`, `e_log.c`, `e_pow.c`, `s_sin.c`, `s_tan.c`, `branred.c`) tal como la compila glibc para CPUs con FMA (la variante que usa la VM en esta máquina): cada `fma` en el mismo sitio que el compilador de glibc (leído de sus volcados `widening_mul`), `branred` sin fusionar. Las tablas (2.080 valores) las extrae el generador de `libm.so.6` buscándolas por contenido y comprobándolas. FMA por hardware (`vfmadd231sd`, detectada con `cpuid`/`xgetbv`) o, si la CPU no la tiene, FMA por software exacta (enteros de 30 bits) que da los mismos bits. Comparado con la libm del sistema en 3,2 millones de casos (y 2 millones más forzando la FMA por software): 0 diferencias; la FMA por software y la de hardware contra un cálculo exacto con fracciones en 900.000 tríos difíciles: 0 diferencias. Límite honesto: en CPUs sin FMA la glibc de Rust usa otra variante (SSE2/FMA4) y Android usa otra libm (bionic); ahí la VM puede diferir en el último bit y el nativo seguirá dando el resultado de la variante FMA |
| std::crypto (4: ChaCha20-Poly1305, AES-256-GCM, sellar/abrir) | `std_crypto.titan` | desde RFC 8439 y NIST SP 800-38D; AES con tablas T calculadas al arrancar y GHASH con tablas de 4 bits; nonce aleatorio con `getrandom`; mismos mensajes que la VM. Comparado con los vectores de RFC/NIST y 3.000 casos aleatorios contra la VM |
| std::password (4: hash/verify de Argon2 y bcrypt) | `std_password.titan`, `std_blowfish_tab.titan` (generado por `native/gen_blowfish.py`, que calcula los dígitos de pi con la fórmula de Machin y los compara con los valores publicados) | lo mismo que password-hash 0.5.0 + argon2 0.5.3 + bcrypt 0.15.1: BLAKE2b y Argon2d/i/id (RFC 9106, versiones 16 y 19, `p` > 1, `data`, `keyid`), el analizador PHC y base64ct con los mismos errores, Eksblowfish y los errores de bcrypt (prefijos 2y/2b/2a/2x, costo como `u32` de Rust, base64 0.22 con alfabeto bcrypt). Comparado: 30 cadenas Argon2 con parámetros variados generadas por una implementación independiente en Python (`native/argon2_ref.py`), aceptadas por la VM y por el nativo; hashes nativos verificados por la VM y al revés; 118 casos de error o borde con mensaje y código de salida idénticos |
| std::procfs (18: nombre del equipo, núcleo, sistema, CPUs, memoria, procesos, discos, redes) | `std_procfs.titan`, `std_procfs_arm.titan` (generado por `native/sysinfo/gen_arm.py` con las tablas de fabricantes y modelos ARM de sysinfo) | lo mismo que sysinfo 0.32.1 en Linux (notas de su código en `native/sysinfo/NOTAS.md`): lee `/proc/stat`, `/proc/meminfo`, `/proc/cpuinfo`, `/proc/[pid]/stat` y `statm` (con los hilos), `/proc/mounts` + `statfs`, `/dev/disk/by-id/usb-*`, `/sys/class/net/*/statistics`, `/etc/os-release`, `uname`; porcentajes en `f32` con las mismas fórmulas, la regla de 200 ms entre lecturas de `/proc/stat`, detección de PID reutilizado por hora de inicio, y `available_parallelism` de Rust (sched_getaffinity + cuotas de cgroups v1/v2 + `get_nprocs` de glibc) cuando no hay CPUs. Comparado en esta máquina con la VM: mismos nombres, núcleo, sistema, CPUs (marca, fabricante, frecuencia), memoria, discos y redes salvo lo que cambia entre una ejecución y otra; un proceso ocupado (`yes`) aparece arriba con su % de CPU en ambos. Prueba `tests/native/procfs_std.titan` (forma de los datos y relaciones que valen siempre) idéntica. Diferencias honestas: (1) con empates de % de CPU la VM devuelve los procesos en un orden aleatorio (sysinfo los guarda en un `HashMap` con semilla al azar; dos ejecuciones de la VM tampoco coinciden); el nativo usa el orden de `/proc`. (2) Donde sysinfo haría `panic!` por un archivo de `/proc` con un formato que Linux no produce, la VM se cae con un pánico de Rust y el nativo termina con un error "sysinfo would panic: …" |
| std::signals (3: install, pending, wait_any) | `std_signals.titan` | lo mismo que el módulo de la VM sobre signal-hook 0.3: los mismos 8 nombres (con o sin "SIG", mayúsculas ASCII), el mismo error para nombres desconocidos, contadores por señal que se ponen en 0 al consultarlos, `wait_any` con plazo (negativo = 1000 ms) que devuelve "timeout". Mecanismo: el runtime nativo no tiene hilos ni punteros a funciones para un manejador como el de signal-hook; usa lo que Linux ofrece para lo mismo: las señales instaladas se bloquean (`rt_sigprocmask`) y se leen desde un `signalfd`. La señal deja de terminar el proceso y queda contada, igual que en la VM. Comparado: un programa que instala USR1/TERM/HUP recibe señales enviadas con `kill` desde fuera; la VM y el nativo imprimen exactamente lo mismo (USR1 despierta a `wait_any`, TERM queda pendiente sin matar el proceso, HUP sale después). Pruebas `senales_std`, `error_senal_desconocida`, `error_senal_no_ascii` idénticas. Diferencias honestas: (1) si hay varias señales pendientes a la vez, la VM elige en qué orden devolverlas según un `HashMap` con semilla al azar; el nativo sigue siempre el orden de la tabla (INT, TERM, HUP, USR1, USR2, QUIT, PIPE, CHLD). (2) La VM revisa cada 20 ms; el nativo despierta en cuanto llega la señal. (3) Pendiente para `std::process`: la máscara de señales bloqueadas se hereda al lanzar otro programa; el hijo debe desbloquearlas antes de `execve` (`sg_mask()`) |
| std::progress (7: bar_new, spinner_new, set_message, set_position, increment, finish, abandon) | `std_progress.titan`, `std_uwidth.titan`, `std_uwidth_tab.titan` (generado por `native/uwidth/gen_uwidth.py`) | lo mismo que progress_mod sobre indicatif 0.17.11 + console 0.15.11 + unicode-width 0.2.2, traducido de su código original (descargado de crates.io por el workflow `fuentes-crates.yml` y comprobado contra las sumas de `Cargo.lock`; notas en `native/progress/NOTAS.md`): registro de 64, límite de 4096 bytes, mismos errores; plantillas `{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}` y `{spinner:.green} {msg}`, barra con aritmética f32, colores según `colors_enabled()` (TERM, NO_COLOR, CLICOLOR, CLICOLOR_FORCE, stdout terminal), el autómata de `strip_ansi_codes`, el ancho Unicode de unicode-width 0.2.2 (tablas copiadas byte a byte), los limitadores de frecuencia (20 Hz con ráfaga 20; posición 1 ms con ráfaga 10), el dibujo de `draw_to_term` y el borrado de las barras sin terminar al salir. Mecanismo: el hilo que avanza el spinner cada 100 ms es aquí un proceso hijo (fork) que recibe las órdenes por un socketpair y confirma cada una, solo si stderr es una terminal; muere con el padre (PR_SET_PDEATHSIG). Comparado: 6 pruebas sin terminal idénticas (ids, cupo, mensajes, errores); dentro de una pseudoterminal (`native/progress/pty_run.py`) las barras dan exactamente los mismos bytes que la VM con y sin colores, y el spinner los mismos cuadros, ticks, final y limpieza. El ancho de texto se comprobó con la batería de pruebas original de unicode-width (sus aserciones, los 3.953 emoji de `emoji-test.txt` y sus bucles sobre todos los caracteres): 5.564.649 comprobaciones, 0 fallos (`native/uwidth/verificar.sh`, con el compilador de pruebas `prueba_interna.titan`). Diferencia honesta: en Rust el primer tick del hilo del spinner compite con lo que hace el programa justo después de crearlo (en la prueba, la VM dibujó primero el mensaje y el nativo primero el tick); es una carrera real en los dos, solo cambia el primer cuadro |
| std::term (15: print_colored, print_styled, print_attr, clear_screen, clear_line, move_to, hide_cursor, show_cursor, size, flush, enter_alt_screen, leave_alt_screen, enable_raw, disable_raw, read_key) | `std_term.titan`, `std_term_keys.titan` | lo mismo que term_mod sobre crossterm 0.28.1 + rustix 0.38.44 + mio 1.2.2, traducido de su código original (notas en `native/term/NOTAS.md`): las mismas secuencias de escape (NO_COLOR memorizado, tabla SGR, MoveTo con desborde u16), el modo crudo con TCGETS2/TCSETS2 y su plan B, `size()` con TIOCGWINSZ y, si falla, `tput cols`/`tput lines` lanzados de verdad (búsqueda en PATH de glibc y el errno que el hijo deja en el padre), y `read_key` con epoll por flanco, SIGWINCH (bloqueada y leída por signalfd, porque el runtime no tiene punteros a funciones) y el analizador `parse.rs` completo (SS3, CSI, ratón, pegado, foco, protocolo de kitty, UTF-8 partido). Comparado: 9 pruebas sin terminal en la batería general y `native/term/verificar.sh` (11 de 11 iguales byte a byte): tamaño con 7 entornos y en una pseudoterminal, 40 teclas en modo crudo con la configuración de la terminal antes y después, plazos, cambio de tamaño de la ventana, modo normal y NO_COLOR. **Defecto de Rust corregido:** `print_colored("#a€bc", …)` hacía pánico (corte dentro de un carácter); corregido en `term_mod.rs` con `str::get`, el runtime en Titan da el error correcto. Diferencia honesta: sin TERM y sin terminal, el errno del error de `size()` depende de una carrera dentro del propio Rust (medido: Rust 19 de 20 veces "os error 11" y 1 "os error 2"; el nativo 18 y 2), por eso ese caso queda fuera de la batería |
| std::email (3/3 cuerpos Titan; transporte real aún sin prueba diferencial) | `std_email.titan`, importado por `native/runtime.titan` | Construye mensajes MIME de texto, alternativa texto/HTML y adjuntos binarios; entrega real mediante `curl`/libcurl. Port 25 explícitamente sin TLS, 465 con TLS implícito y otros puertos con STARTTLS obligatorio; la verificación TLS de curl permanece activa. Las credenciales se escriben temporalmente con `O_EXCL` y modo 0600, nunca como argumentos del proceso, y el archivo se elimina al terminar. `email_validation.titan` comprueba rechazo previo a red de remitente y MIME inválidos, inyección de cabeceras y puertos fuera de rango; el módulo y esa prueba aún no se han compilado con Zett ni ejecutado diferencialmente, y falta probar un envío contra un SMTP real. |
| std::kv (17 definiciones Titan; interoperabilidad Rust/Sled pendiente) | `std_kv.titan`, importado por `native/runtime.titan` | Implementación persistente real con snapshots JSON versionados, claves/valores binarios codificados sin pérdida, escritura atómica con `fsync` del archivo y del directorio, y bloqueo entre procesos mediante `flock` con plazo; cubre CRUD, CAS, límites y árboles nombrados. En una ejecución real previa, `kv_persistence.titan` terminó 1/1 en la comparación VM/nativo, incluidos persistencia, CAS, bytes binarios, orden y cierre/reapertura; esa batería no prueba el intercambio con Sled. La validación canónica añadida después aún no se ha reejecutado. Se añadió `kv_interop.titan` de tres fases VM/Sled → Titan nativo → VM/Sled; ahora también cubre `open_tree("__sled__default")` como alias real del árbol por defecto, con longitudes compartidas en ambos backends. Se corrigió Rust para contabilizar ese alias como `TreeId::Default`. Con el Zett precompilado de origen `3231a660c5d9e5dd646c83a3b4ed8e9e69dbbcc0` falló porque la fase VM/Sled no publicó `.titan-kv-v1.json`; no se cuenta como aprobada. `crates/titan_stdlib/src/kv_mod.rs` tiene ahora un puente inicial con snapshot versionado, CRC32 de Sled, `flock`, marcadores de recuperación y sincronización al abrir, hacer flush y cerrar; no interpreta el pagecache privado ni usa `Db::import`. El puente Rust todavía no se compiló ni validó con un Zett actualizado. `.github/workflows/selfhost.yml` registra el origen/SHA del seed y se activa ante cambios del puente. El backend Titan reescribe el snapshot completo en cada mutación; no se presenta como equivalente en rendimiento. |
| std::audio (62/62 cuerpos Titan; `cloud_*` eliminado; ver «Audio: decodificador y engine en Titan») | `std_audio.titan`, `std_audio_tags.titan`, `std_process.titan` | La síntesis de seno, onda cuadrada, diente de sierra (`saw_wave`) y ruido blanco, además de reproducción/grabación por Termux, usan implementaciones reales; Titan también implementa `encode_wav`, `write_wav`, `read_wav` y `read_wav_bytes` para PCM RIFF/WAVE. `std_audio_tags.titan` contiene parsers y wrappers Titan para `tags`, `duration`, `cover` y `scan_library`: `tags`/`duration` leen una cabecera y cola acotadas; `cover` busca la portada por rangos y no carga el audio completo; limita a 64 MiB la metadata que recorre en Rust y Titan y devuelve un error explícito al superar ese presupuesto. Se corrigió en Rust y Titan el escaneo de nombres POSIX no UTF-8: se abre usando los bytes originales y se convierte a UTF-8 con reemplazo solo al exponer `TrackInfo.path`, conservando entradas distintas con la misma ruta visible. También se corrigió la lectura del sufijo del archivo cuando este cabe por completo en la cabecera: así `scan_library` puede encontrar ID3v1 y no cuenta sus 128 bytes como audio en MP3 cortos. Ahora `scan_library` limita el recorrido a 5.000 entradas totales —directorios, archivos no compatibles y archivos de audio—, no solo pistas válidas; Rust cuenta cada resultado del iterador y Titan procesa `getdents64` en streaming. La regresión crea 2.501 álbumes con un WAV válido cada uno y espera exactamente 2.500 pistas; aún falta ejecutarla con el seed precompilado. `audio_tags.titan` cubre ahora ese caso junto con dos WAV PCM completos en nombres de archivo reales, un MP3 con footer ID3v2.4 válido y un APIC v2.4 con descripción UTF-16BE para detectar truncamiento al localizar el terminador. Las nuevas regresiones ID3v1 corto/APIC UTF-16BE aún no se han ejecutado; también añade fixtures en disco con PNG de más de 1 MiB incrustado en MP3/ID3, FLAC PICTURE y WAV/ID3 después del chunk de audio, incluido un caso con 80 MiB de muestras dispersas antes de la portada, además de fixtures dispersos de ID3 (incluido un APIC válido antes del límite), FLAC y RIFF para comprobar el límite común. La ruta Titan de `cover` siempre usa el lector acotado, incluso si el APIC completo cabe en la cabecera inicial. Los verificadores nativo, LLVM y ARM64 preparan esos fixtures. En la ejecución real previa de `verify_native.sh audio_tags.titan`, el lado nativo terminó con código 0, incluidos `scan_library`, las portadas grandes y el límite de 64 MiB; el Zett VM precompilado terminó con código 1 en la aserción ID3v2.4 UTF-16BE (`left=bytes[11], right=bytes[8]`). La comparación completa falla y no se cuenta como aprobada. Los fuentes Rust y Titan recorren el terminador UTF-16 en límites de unidades, pero el Zett de `binaries` (origen `3231a660c5d9e5dd646c83a3b4ed8e9e69dbbcc0`, SHA-256 `76eaf82ab0388f25f802bc7a263563e96a10ba0d8e1049bfcbbe58fad533ef40`) es anterior a esa corrección. En el intento más reciente el manifiesto volvió a verificarse, pero faltaba la biblioteca real `libasound.so.2`; `apt-get update` no pudo conectar a Debian y no se usó ningún reemplazo. Por tanto, falta repetir la comparación con un seed actualizado y ALSA real. `audio_wav_io.titan`, `audio_wav_read.titan` y `audio_synthesis.titan` están en las matrices; `audio_synthesis` exige código 0 y cubre `saw_wave`. Self-host CI `37396772665`: 313/313 idénticos entre los cuatro shards. LLVM CI `37396772922`: 313/313 idénticos en x86-64 y en ARM64 real; ambas corridas son anteriores a `audio_tags.titan` y no la validan. Los CLI Termux son `termux-media-player` / `termux-microphone-record`. Titan implementa ahora `player_backend` mediante búsqueda real en `PATH` y descarta archivos sin bit ejecutable; `audio_player_backend.titan` añade una regresión con un archivo regular no ejecutable que nunca se lanza, pero la comparación diferencial todavía no se ejecutó. Las seis `sim_*` ya tienen una ruta de salida real en fuente: Rust usa CPAL en escritorio y Termux:API en Android; el cuerpo Titan genera/escribe PCM16 WAV y lo reproduce con un ejecutable real (el bucle requiere mpv; ajustar volumen en una onda activa reinicia el archivo en esta ruta). No se han compilado ni comparado todavía y no se declara paridad: falta ejecutar la regresión con el seed actualizado y salida/dispositivo real. Las demás APIs `player_*` Titan están completas. El motor `engine_*` (22 funciones) y el decodificador propio (WAV, PCM, ADPCM, FLAC, Ogg/Vorbis) están descritos en «Audio: decodificador y engine en Titan». `cloud_*` se eliminó (Telegram/MTProto no es verificable sin servicio real). |
| std::onnx (9/14 cuerpos Titan; faltan las 5 `*_bert*`) | `std_onnx.titan`, `std_onnx_engine.titan` | motor propio de inferencia (subconjunto de 39 operadores, rechazo explícito del resto); comparado con onnxruntime y con tract; ver «std::onnx en Titan» |
| std::termux (23/23 cuerpos Titan; paridad conductual aún parcial) | `std_termux.titan`, `std_process.titan` | `is_available` busca el CLI real en `PATH` y, si hace falta, intenta iniciarlo sin esperar. Las otras 22 funciones invocan los `termux-*` reales con argv directo y stdin real cuando corresponde; no se simulan servicios Android. `termux_missing.titan` compara el error de batería cuando falta el CLI; todavía no demuestra el comportamiento de cada servicio en un dispositivo Termux. |
| std::clipboard (2) y std::notify (1) | `std_clipboard.titan` y `crates/titan_stdlib/src/clipboard.rs` | El adaptador Rust usa servicios reales en Termux/macOS/Linux/Windows para portapapeles y Termux/macOS/Linux para notificaciones (Windows devuelve `false`, sin backend). `std_clipboard.titan` usa comandos reales en los runtimes Unix que soporta; si no hay servicio devuelve `""`/`false`, sin fallback en memoria. El seed precompilado `3231a66` aún simula el portapapeles; su salida no cuenta como paridad. El job dedicado ejecuta el ELF Titan contra Xvfb/xclip y D-Bus/dunst/notify-send. En `37189547454`, la sonda `notify-send` activó un proveedor antes de que dunst reclamara el nombre; se cambió a `NameHasOwner` en `626d7f9`. En `37190391495` el ELF corrió, pero el job terminó con fallo. `4fe7b84` evita capturar stdout/stderr de clientes que se bifurcan (como xclip); el job real de portapapeles/notificación pasó en `37192097995`. El job dedicado pasó de nuevo en `37192244855`. |
| std::readline (4 de 4 funciones) | `std_readline.titan` | lectura real desde la terminal: edición y movimiento del cursor, borrado, historial en memoria y archivo compatible con el formato `#V2`, búsqueda inversa con Ctrl+R y entrada secreta sin mostrar las teclas; si no se puede activar el modo protegido, rechaza leer la clave. También acepta entrada por tubería y comunica Ctrl+C/Ctrl+D. La prueba `native/readline/verificar.py` incluye comparación VM/nativo en pseudoterminal, historial entre dos llamadas, escapes `#V2` de barras y saltos de línea, conservación del archivo cuando no hay una entrada nueva, clave UTF-8 y rechazo de claves sin terminal. En CI `37278754059`, el job dedicado con el binario precompilado, una pseudoterminal real y ALSA real pasó. No se aceptan resultados con ALSA simulada |
| std::redis (21/21 funciones) | `std_redis.titan` | TCP real y RESP2 con sockets, AUTH percent-decoded, SELECT, tiempos límite por operación, parser acotado, handles que limpian sockets y comandos `raw` con allowlist; `keys` usa SCAN, ordena y deduplica sin bloquear con KEYS. `redis.titan` fuerza Redis real, prueba los 21 endpoints, tres handles simultáneos, fallo de AUTH, error WRONGTYPE sin perder el socket, UTF-8 y 54 claves ordenadas. La comparación VM/nativo pasó contra Redis 7.2.4 real; el arnés arranca y apaga un servidor local auténtico si no se proporciona `TITAN_TEST_REDIS_URL`. |
| std::server (23/23 funciones) | `std_server.titan` | Port de `server_mod.rs`: TCP real por syscalls (socket/bind/listen/accept4/poll/read/sendto), parseo estricto de request-line y cabeceras (límites de 64 KiB de cabecera, 128 cabeceras, nombres de 256 y valores de 8192 bytes), cuerpo fijo y chunked con trailers, `Expect`, respuestas validadas (reserva del 101, Content-Type y framing gestionados), `respond*`, upgrade WebSocket (SHA-1 + Base64 de `std_ws.titan`), decodificador de mensajes (fragmentación, UTF-8, máscara obligatoria, límites), cierre y eco de cierre. Handles, límites (8 servidores, 256 peticiones, 64 WebSockets vivos) y mensajes de error iguales a la VM. Pruebas: `server_std.titan` (curl) y `selfhost/tests/server_diff/` (`bash selfhost/tests/server_diff/run.sh`): servidor Titan + cliente Python de guion fijo contra VM y nativo; el cliente y lo que imprime el servidor coinciden línea a línea. Desviaciones: ver «Corrección de cifras». |
| std::process (22: run, run_with_input, shell, pipe, run_timeout, spawn, spawn_wait, spawn_poll, spawn_kill, spawn_pid, env_get, env_set, env_unset, env_vars, working_dir, set_working_dir, self_pid, hostname, username, args, send_signal, exit) | `std_process.titan` (+ `rt_environ`/`rt_getenv` en `runtime.titan`) | lo mismo que process_mod.rs y `run_timeout` de process.rs (notas en `native/process/NOTAS.md`): los programas se lanzan como `posix_spawnp` de glibc (fork, redirecciones, máscara de señales vacía, SIGPIPE por defecto, la búsqueda en PATH de `__execvpex` con su regla del último errno, el errno del exec fallido pasado al padre); la salida y la entrada se leen/escriben a la vez con `poll` (cupo compartido de 4 MiB, EPIPE en vez de morir por SIGPIPE); los procesos en segundo plano tienen un proceso ayudante que lee sus tubos mientras corren (lo que en Rust hacen dos hilos; el runtime no tiene hilos) y al terminar el programa se matan y se esperan como en el `Drop` de Rust (pero no con `exit`, igual que Rust). `env_set`/`env_unset` cambian un entorno propio del runtime que ven `std::env`, `std::dirs`, el `tput` de `std::term` y los hijos. Comparado: 66 programas idénticos (6 normales y 58 de errores, más `ruta_palabra_clave` y `dirs_lineas_malas`, que ahora pone `XDG_CONFIG_HOME` con `env_set`) y variables no UTF-8 a mano. **Defectos de Rust corregidos:** 16 (`spawn` no se podía escribir), 17 (panic en `env_set`/`env_unset`) y 18 (panic en `env_vars`). Diferencia honesta: el ayudante aparece como un proceso más en `ps` |
| std::url (10: is_valid, scheme, host, port, path, query, fragment, join, parse_query, build_query) | `std_url.titan`, `std_idna.titan`, `std_icu.titan`, `std_icu_tab.titan` (generado por `native/icu/gen_icu.py`) | lo mismo que url_mod.rs sobre url 2.5.8 + form_urlencoded 1.2.2 + percent-encoding 2.3.2 + idna 1.1.0 + idna_adapter 1.2.2 + ICU4X 2.2.0, traducido de su código original (descargado por `fuentes-crates.yml`, sumas de `Cargo.lock`): el analizador WHATWG de `parser.rs` (esquemas especiales, `file:`, rutas opacas, `.`/`..` y `%2e`, IPv4 con octal/hex, IPv6 con `::` y IPv4 dentro, puertos, usuario/contraseña, tabuladores y saltos quitados, URL base para `join`), los mismos mensajes de error, `form_urlencoded` (`+`, UTF-8 con U+FFFD, último valor gana, mapa ordenado). Los dominios no ASCII pasan por UTS 46 como `idna` (lista de caracteres prohibidos de URL, punycode en los dos sentidos, CONTEXTJ con virama y tipos de unión, reglas bidi, marca al principio, largo máximo) y la normalización de ICU4X: los iteradores `Decomposition`/`Composition` traducidos paso a paso (incluidos los casos raros de U+0345, U+0F73 y U+FDFA) sobre sus propias tablas: el `CodePointTrie` de UTS 46, las tablas NFD/NFKD, el `Char16Trie` de composiciones y los tries de Bidi_Class, Joining_Type y General_Category, copiados byte a byte de `icu_normalizer_data`/`icu_properties_data` (Unicode de ICU4X 2.2, no el de Python). Comparado con la VM: 3 pruebas (`url_basico`, `url_errores` y `url_casos` con 272 URL, 60 `join`, 24 consultas y 10 `build_query`), 38.000 casos aleatorios (`native/url/fuzz.py`, 13 semillas) y todos los puntos de código de Unicode dentro de un dominio, solos y con una marca combinante detrás (`native/url/todos.py`, 2,2 millones de casos): 0 diferencias |
| std::window (12) | `std_window.titan` | Las seis funciones lógicas (`create`, `is_open`, `close`, `set_title`, `resize`, `poll_events`) ya gestionan handles, límites y eventos de resize en memoria real, igual que `window.rs`; `window_state.titan` da 1/1 idéntico. Las seis `live_*` implementan X11 real; la salida actual sin servidor X solo valida sus rutas de rechazo, no una ventana visible. |
| std::regex (7: is_match, find, find_all, captures, replace_all, split, is_valid) | `std_regex_parse.titan` (capa 1: sintaxis), `std_regex_tab.titan` (tablas Unicode 16, generado por `native/regex/gen_tab.py`), `std_regex_hir.titan` (capa 2: traducción, clases, plegado), `std_regex_nfa.titan` (capa 3: NFA de Thompson y límite de tamaño), `std_regex_run.titan` (capa 4: búsqueda) | lo mismo que regex_mod.rs sobre regex 1.13.0 + regex-syntax 0.8.11 + regex-automata 0.4.15, traducido de su código original: el analizador de `ast/parse.rs` con los mismos mensajes y marcas `^^^`, la traducción a HIR (clases Unicode, `(?i)` con la tabla de plegado, `\b` ASCII/Unicode, banderas), el compilador de Thompson (secuencias UTF-8, compartir sufijos, NFA inverso, límite de 10 MiB con el mismo error) y la búsqueda PikeVM (prioridad de hilos, `\b` sobre UTF-8 inválido, no partir caracteres, coincidencias vacías, `$1`/`${nombre}`/`$$`). Además guarda las 8 últimas expresiones compiladas (la VM compila en cada llamada): en un bucle típico el nativo va ~2× más rápido que la VM. Comparado con la VM: 3 pruebas (`regex_basico`, `regex_cache`, `regex_errores`, también por LLVM), 27.000+ casos aleatorios × 6 funciones (`native/regex/busca.py`) y 66.000 análisis/traducciones (`native/regex/fuzz_parse.py`): 0 diferencias. Con miles de expresiones distintas llenas de clases Unicode el nativo aún es ~2,3× más lento que la VM (compilar el NFA) |
| std::router (5: new, drop, insert, at, matches) | `std_router.titan` | lo mismo que router_mod.rs sobre matchit 0.8.6, traducido de su código original (`tree.rs`, `escape.rs`, `params.rs`, `error.rs`): escapes `{{`/`}}`, normalización de parámetros, sufijos (`{x}.png`), comodín final `{*resto}`, prioridades de los hijos, retroceso en la búsqueda y los mismos mensajes de conflicto; registro con los mismos límites (64 routers, 4096 rutas, 1 MiB por router, 8 MiB en total, tamaños de patrón/valor/camino). Comparado con la VM: 2 pruebas (`router_basico`, `router_limites`, también por LLVM) y 20.000 casos aleatorios (`native/router/fuzz.py`, ~280.000 resultados): 0 diferencias |
| std::freestanding (6: init, validate_target_spec, generate_linker_script, generate_startup_asm, get_active_target, shutdown) | `std_freestanding.titan` | los mismos textos que freestanding.rs letra por letra (ya corregidos, ver abajo); el objetivo activo en las globales. Comparado: `freestanding_textos` (3 objetivos, errores, símbolos); los guiones y arranques de los 3 objetivos se ensamblan y enlazan con ld.lld y GNU ld de verdad, y los 3 `_start` se ejecutaron en procesadores emulados (llegan a la función de entrada con la pila en `_stack_top`); el compilador los usa para `aarch64-none` |
| std::compress, descompresión (3 de 8: gzip_decode, zlib_decode, deflate_decode) | `std_compress.titan` | lo mismo que compress_mod.rs sobre flate2 1.1.9 + miniz_oxide 0.8.9, traducido de su código original (`inflate/core.rs`: `init_tree` con sus tablas de 10 bits y árbol, `decode_huffman_code`, `read_bits`, la máquina de estados de `decompress`; `gz/mod.rs` y `gz/bufread.rs`): mismas reglas de aceptación (cabecera zlib, bloques almacenados, fijos y dinámicos, códigos incompletos de un solo símbolo que miniz acepta, incluida su rareza de decodificar los caminos no usados como símbolo 0), cabecera gzip con FEXTRA/FNAME/FCOMMENT/FHCRC y el límite de 65535, CRC-32 y longitud, solo el primer miembro, basura detrás ignorada, y los mismos mensajes. Con los dos defectos de abajo corregidos. Comparado: un modelo de referencia en Python con dos interruptores (antes/después de la corrección) tiene que dar exactamente lo mismo que la VM anterior y que el nativo en cada caso: 70.500 casos aleatorios (`native/compress/fuzz.py`: flujos del zlib de C con todos los niveles y estrategias, bloques dinámicos hechos bit a bit con árboles completos, incompletos y de un código, mutados y cortados): 0 diferencias. Pruebas `compress_inflate`, `compress_codigos` (todos los códigos de longitud y distancia con extra mínimo y máximo) y `compress_defectos`, generadas por `native/compress/gen_tests.py`, también por LLVM. 20 MB de gzip: 173 ms con LLVM (la VM 57 ms). `std::checksum::crc32` pasa a usar la misma tabla (63 ms frente a 2,7 s bit a bit; la VM 89 ms). Los compresores, en la fila siguiente; zstd, en las de después |
| std::compress, compresión (3 de 8 más: gzip_encode, zlib_encode, deflate_encode; niveles 0..9) | `std_compress_enc.titan` | lo mismo que compress_mod.rs (`GzEncoder`/`ZlibEncoder`/`DeflateEncoder` de flate2 1.1.9 con `Compression::new(nivel)`, `write_all` + `finish`), traducido del código original de miniz_oxide 0.8.9 (`deflate/core.rs`: `compress_normal` con búsqueda perezosa, `compress_fast` del nivel 1, `find_match`, `flush_block` con la vuelta a bloque almacenado si no ahorra, `optimize_table`, `calculate_minimum_redundancy`, `enforce_max_code_size`, el RLE de `start_dynamic_block`; `stored.rs` del nivel 0; la cabecera de `zlib.rs`) y del camino de llamadas de flate2 (`zio::Writer` con su búfer de 32 KiB, `stream::deflate`, `compress_inner`), porque importa: cuando un bloque no cabe en lo que queda del búfer, miniz se para y sigue en la llamada siguiente, y en el nivel 1 eso cambia la salida (se encontró así: un caso de 135 KB salía distinto hasta reproducirlo). Cabecera gzip de flate2 (XFL 2/4/0, SO 255) y cola CRC-32 + longitud; niveles fuera de 0..9 (o que no caben en i32) dan el mismo error. Salida idéntica byte a byte a la VM: `native/compress/deflate_ref.py` es el mismo algoritmo en Python (tablas leídas del core.rs original), y `native/compress/fuzz_enc.py` compara VM, modelo y nativo: 10.000 casos aleatorios (263 MB: aleatorio, alfabetos pequeños, corridas, texto, copias lejanas, periodos que envuelven el diccionario, entradas de 0 a 300 KB, todos los niveles y formatos): 0 diferencias; también con el binario de LLVM. Prueba `compress_encode` (incluye el caso de nivel 1 que se para a mitad de bloque, comprobado con el modelo). 5 MB de texto a nivel 6: 0,39 s con LLVM -O2 (la VM 0,19 s); con el backend propio sin optimizar ~3,3 s |
| std::compress, zstd: descompresión (1 de 8 más: zstd_decode) | `std_compress_zstd.titan` | lo mismo que compress_mod.rs (`zstd::stream::decode_all` de la crate zstd 0.13.3 sobre libzstd 1.5.7, la versión que lleva la VM), traducido línea a línea del C original (`zstd_decompress.c`: `ZSTD_decompressStream` con su búfer circular, el paso único cuando el marco entero cabe, `ZSTD_decompressContinue`, marcos saltables, tamaño de ventana máximo, checksum XXH64; `zstd_decompress_block.c`: literales con el búfer partido de 64 KiB, tablas FSE, las tres variantes de ejecución de secuencias incluida la de historia >16 MB y las copias "salvajes" de 16/32 bytes; `huf_decompress.c`: tablas X1 y X2, el camino rápido 4X que usa la VM en x86-64/aarch64 y el genérico; `entropy_common.c`, `fse_decompress.c`, `bitstream.h`) y del conductor de Rust (`io::copy` con búfer de 8192 bytes, `BufReader` de 131075 y cómo la crate reinicia entre marcos y decide "incomplete frame"), porque con datos corruptos cambian qué errores salen (p. ej. el camino rápido de Huffman no comprueba el final del flujo y acepta datos que el genérico rechaza). Mismos mensajes (`ZSTD_getErrorName`). Comparado: `native/compress/zstd_ref.py` es el mismo algoritmo en Python y `native/compress/fuzz_zstd.py` compara VM, modelo y nativo: 10.300 casos aleatorios (marcos de la libzstd real con parámetros variados, varios marcos, saltables, y mutados/cortados; la mitad dan error): 0 diferencias, también con el binario de LLVM; además 10.300 con el Huffman genérico y la variante de secuencias "long" forzadas en modelo y nativo, 300 con la cabecera del segundo marco partida en el borde del `BufReader`, y marcos de 400 KB (literales partidos) y 19 MB (secuencias "long" de verdad). Prueba `compress_zstd` (generada por `native/compress/gen_zstd_test.py`). 19 MB: 0,59 s con LLVM -O2 (la VM 0,69 s; los dos contando el paso a hexadecimal del arnés), ~4,9 s con el backend propio sin optimizar. La compresión, en la fila siguiente |
| std::compress, zstd: compresión (1 de 8 más: zstd_encode; niveles 1..22; std::compress completo, 8 de 8) | `std_compress_zstd_enc.titan` (entropía, bloques, divisores, marco, flujo), `std_compress_zstd_mf.titan` (buscadores de coincidencias, analizador óptimo, LDM), `std_compress_zstd_enc_tab.titan` (tablas, generado por `native/compress/gen_zstd_enc_tab.py` desde el C original) | lo mismo que compress_mod.rs (`zstd::stream::encode_all` de la crate zstd 0.13.3: toda la entrada en un `write` y luego `finish`, es decir `ZSTD_e_continue` + `ZSTD_e_end` con tamaño desconocido), traducido del C de libzstd 1.5.7: el flujo con su búfer circular de ventana + 128 KiB y el reinicio de índices por desbordamiento, el divisor previo de bloques (`zstd_preSplit.c`), literales Huffman (tablas óptimas por profundidad desde btultra), FSE con sus tres modos y repetición, los buscadores fast y dfast, el de filas (greedy/lazy/lazy2, niveles 5-12), el árbol binario DUBT (btlazy2, 13-15), el analizador óptimo con hash3 (btopt/btultra/btultra2, 16-22, incluida la pasada doble de btultra2 en el primer bloque), el divisor posterior por secuencias (`ZSTD_compressBlock_splitBlock`, 16-22) y las coincidencias a larga distancia del nivel 22 (`zstd_ldm.c`: hash de engranaje, XXH64, cubetas, y su uso como candidatas en el analizador óptimo). Se reproduce también una rareza del original: `ZSTD_ldm_gear_reset` calcula el hash en una variable local y no lo guarda, así que no reinicia nada; sin copiarla, los cortes y la salida serían otros. Comparado byte a byte con la libzstd real (`native/compress/fuzz_zstd_enc.py`, python-zstandard 0.25.0 = libzstd 1.5.7, o la biblioteca C compilada): unos 4.000 casos aleatorios de todos los niveles, 0 diferencias; archivos de 6 y 14 MB en varios niveles y uno de 150 MB en el nivel 22 (más que la ventana de 128 MiB: búfer circular y diccionario externo), idénticos. Prueba `compress_zstd_enc` (todos los niveles, errores de nivel, entrada string), también por LLVM. El runtime gana `rt_alloc_zero`: memoria a cero con `madvise(MADV_DONTNEED)` en bloques grandes (en ARM64 se traduce a su número 233; sin sistema operativo, bucle), porque poner a cero con un bucle las tablas del nivel 22 (~200 MB) costaba >1 s por llamada |
| std::archive (5 de 5: tar_pack, tar_unpack, zip_pack, zip_unpack, zip_list) | `std_archive.titan` | lo mismo que archive_mod.rs sobre tar 0.4.46 y zip 2.4.2 (con la compresión de miniz_oxide de la fila de std::compress), traducido de su código original. tar: `Builder` con cabeceras GNU (nombres largos con `././@LongLink`, modo 0644, suma de control) y `Archive::entries` con todas sus reglas: cabeceras GNU, ustar y antiguas, números octales y binarios (base 256), extensiones pax (`path`, `size`), nombres largos GNU, archivos dispersos GNU con sus bloques extendidos, fin por bloque vacío, y los mismos mensajes (\"numeric field did not have utf-8 text…\", \"sparse file consumed more data than the header listed\"…). zip: `ZipWriter` (deflate nivel 6, fechas y permisos, zip64 automático a partir de 65535 entradas) y `ZipArchive`: búsqueda del EOCD hacia atrás con el reintento de `get_metadata`, EOCD64 y su localizador, desplazamiento del archivo, el directorio central con los campos extra zip64, marca de tiempo (0x5455), Unicode (0x7075, con su CRC), AES (0x9901), nombres CP437 o UTF-8, el mapa de nombres (duplicados: posición del primero, datos del último) y las comprobaciones de `by_index` (cifrado, método, AES) en el mismo orden. Comparado: `native/archive/fuzz.py` genera archivos con el tarfile/zipfile de Python y a mano (campos extra aleatorios, EOCD64 con relleno, directorios rotos) y compara VM y nativo: 14.800 casos aleatorios tras la última corrección (la mitad dan error; salen casi todos los mensajes de error de zip), 0 diferencias reales. Pruebas `archive_tar`, `archive_zip`, `archive_zip64` (65.540 entradas) y `archive_defectos`, generadas por `native/archive/gen_tests.py`. Límite nuevo, igual en las dos versiones: un archivo disperso de tar de más de 4 GiB reales da \"sparse entry too large to unpack in memory\" en vez de intentar reservar esa memoria. `deflate_encode` de entradas pequeñas ya no pone a cero 1,1 MB en cada llamada (usa `rt_alloc_zero`). Diferencia honesta: con el backend propio sin optimizar, `archive_zip64` tarda ~17 s (la VM 1,5 s): cada entrada vacía cuesta ~0,1 ms de montar tablas de Huffman |
| std::yaml (3 de 3: parse, parse_multi, stringify) | `std_yaml.titan`, `std_yaml_parse.titan`, `std_yaml_emit.titan` | lo mismo que yaml_mod.rs sobre serde_yaml 0.9.34 y unsafe-libyaml 0.2.11, traducido de su código original: el lector (UTF-8/UTF-16 con BOM, trozos de 16 KB, \"control characters are not allowed\", octetos UTF-8 incorrectos), el escáner completo de libyaml (claves simples, sangrías, bloques literales y plegados con sus indicadores, escalares planos y entre comillas con todos los escapes, etiquetas y directivas %YAML/%TAG, anclas y alias), el parser con sus 24 estados (incluido un fallo real de libyaml 0.2.5 que serde_yaml hereda: en `[? x : y]` se salta una ficha, reproducido igual), el cargador de serde_yaml (anclas redefinidas, \"unknown anchor\", documentos) y el deserializador hacia serde_json::Value: resolución de escalares sin etiqueta (nulos, booleanos, enteros decimales/0x/0o/0b de 64 y 128 bits con \"JSON number out of range\", decimales, .inf/.nan), etiquetas !!bool/!!int/!!float/!!null y las de enum, claves que no son texto, límites de recursión (128) y de repetición de alias, rutas en los mensajes (`a[1]: …`) y marcas de línea y columna. Mensajes idénticos a los de libyaml y serde. stringify: el serializador de serde_yaml (estilo pedido para cada texto: comillas simples si parece número, booleano o nulo; literal `|` si tiene saltos de línea) sobre el emisor de libyaml con los ajustes de serde_yaml (unicode, sin partir líneas, sangría 2): análisis de cada escalar (indicadores, espacios al principio o al final, saltos, caracteres especiales), elección final del estilo (plano, comillas simples, dobles con todos los escapes `\x`/`\u`/`\U`/`\N`/`\L`/`\P`, literal con sus indicaciones de sangría y de final `-`/`+`), secuencias sin sangría dentro de mapas, claves complejas `? ` (varias líneas o más de 128 bytes), `[]` y `{}`, y los decimales con el formato de ryu (`1e21`, `1e-7`, sin `+`); antes pasa por la misma conversión to_json de la VM (bytes, structs, enums, \"non-finite JSON number\"). Comparado: `native/yaml/fuzz.py` genera YAML válido, mutado, sopa de caracteres especiales, documentos grandes y anidados, bombas de alias, valores JSON aleatorios para stringify e idas y vueltas (parse y luego stringify), y compara VM y nativo: 40.000 casos aleatorios, 0 diferencias. Pruebas `yaml_parse`, `yaml_errores`, `yaml_stringify` y `error_yaml` |
| std::net (1 de 1: http_get) | `std_net.titan`, `std_net_gai.titan`, `std_net_dns.titan` | lo mismo que net.rs y TcpStream::connect de Rust (IPv4/IPv6 estrictos de Rust, cada dirección en orden, mensajes de io::Error) sobre getaddrinfo de glibc 2.39 (la del ubuntu-24.04 de CI) traducido de su código: nsswitch.conf (acciones `[!STATUS=accion]`, módulos files y dns), host.conf (multi, reorder, trim y sus avisos), /etc/hosts con los límites de búfer de glibc, resolv.conf (nameserver, search, domain, options: ndots, timeout, attempts, rotate, edns0, single-request, single-request-reopen, use-vc, trust-ad, no-aaaa, no-tld-query, no-reload; LOCALDOMAIN, RES_OPTIONS, HOSTALIASES; recarga cuando cambia el archivo), el cliente DNS real (consultas A y AAAA con sendmmsg por UDP, paso a TCP si viene truncada, reintentos y tiempos de espera, respuestas con otro ID o pregunta distinta, SERVFAIL/REFUSED/NXDOMAIN, CNAME, compresión), gai.conf (label, precedence, scopev4, reload) y el orden RFC 3484 con check_pf (netlink) y la prueba de conexión UDP. Comparado con glibc real: `native/net/comparar.sh` monta un servidor DNS falso con 30 casos raros (`dns_falso.py`) y servidores HTTP locales y compara VM y nativo en 27 configuraciones distintas del sistema: 0 diferencias. Prueba `net_http` |
| std::qrcode (5 de 5: to_ascii, to_unicode, to_svg, to_png, save_png) | `std_qrcode.titan`, `std_qrcode_png.titan`, `std_qrcode_tab.titan` (generado por `native/qrcode/gen_tab.py` desde qrcode 0.14.1 y fdeflate 0.3.7) | lo mismo que qrcode_mod.rs sobre qrcode 0.14.1 (matriz, modos numérico/alfanumérico/byte/kanji, Reed-Solomon, las 8 máscaras y su puntuación) y, para PNG, image 0.25.10 + png 0.18.1 con fdeflate 0.3.7 (compresión Fast y filtro Adaptive). Mensajes idénticos (`unknown error-correction level`, `QR encode error: data too long`, `module_pixels`/`side_pixels out of range`, `image write error: … (os error N)`). Comparado: pruebas `qrcode_std` y `qrcode_errores` idénticas a la VM (también por LLVM y objetos ARM64); `native/qrcode/fuzz.py` 350 textos aleatorios (números, ASCII, Unicode, niveles L/M/Q/H y errores): 0 diferencias |
| std::plot (5 de 5: line, multi_line, bar, scatter, histogram) | `std_plot.titan` | lo mismo que plot_mod.rs sobre plotters 0.3.7 (solo SVG, sin TTF): lienzo 900×500, márgenes, título, malla clara y gruesa, ejes, etiquetas, barras, puntos y leyenda. El archivo SVG sale idéntico byte a byte al de la VM. Mensajes `plot input error: …`. Comparado: pruebas `plot_std` (CRC de los 5 gráficos) y `plot_errores` idénticas a la VM |
| std::fswatch (4 de 4: watch_once, open, next_event, close) | `std_fswatch.titan` | lo mismo que fswatch_mod.rs sobre notify 6 (inotify en Linux): hasta 32 vigilantes, ruta ≤16384 B, espera ≤86400000 ms, cola de 1024, texto de evento `{tipo}:{ruta}` truncado a 64 KiB. Vigilancia recursiva de subcarpetas y de carpetas nuevas. Mensajes idénticos (`watcher error: … about […]`, `watch path bytes exceeds limit 16384`, `no watcher registered under handle N`). Comparado: pruebas `fswatch_std` (timeout, create de archivo, recursive) y `fswatch_errores` idénticas a la VM |
| std::ws (6 de 6: accept_key, upgrade_response, validate_upgrade, validate_accept, encode, parse) | `std_ws.titan` | lo mismo que websocket.rs (RFC 6455): SHA-1 + Base64 para la clave de aceptación (vector `dGhlIHNhbXBsZSBub25jZQ==` → `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`), respuesta 101, marcos con máscara aleatoria de getrandom, longitud mínima, bits reservados, UTF-8 en texto. Mensajes idénticos (`invalid Sec-WebSocket-Key`, `unsupported WebSocket opcode N`, `WebSocket masking policy violation`, …). Comparado: pruebas `ws_std` y `ws_errores` idénticas a la VM |
| std::http (13 de 13: parse_request, build_response, reason_phrase, route_match, parse_query, security_headers, cors, request_id, rate_limit, json_response, error_response, request, parse_multipart) | `std_http.titan` | lo mismo que http.rs, multipart.rs y el cliente de http_client.rs: petición HTTP/1.1, Host obligatorio, Content-Length, rutas `:id`/`*path`, query con `+` y percent-encoding, cabeceras de seguridad y CORS, `titan-{id:016x}`, límite de ritmo, JSON compacto, multipart/form-data, cliente HTTP/HTTPS real (TCP y `openssl s_client`, redirecciones, chunked, SNI y verificación de CA y nombre). HTTPS depende de `openssl` en PATH y del almacén de certificados del sistema (`SSL_CERT_FILE`/`SSL_CERT_DIR` también se respetan); no se convierte a HTTP sin cifrar. `std::net::http_get` separado aún no ofrece HTTPS. Comparado: pruebas `http_std` y `http_errores` idénticas a la VM |
| std::http_full (4 de 4 funciones) | `std_http_full.titan` | HTTP/HTTPS real: solicitudes, JSON y formularios; agente, autenticación básica y Bearer, límites de espera y redirecciones; descompresión gzip. Reutiliza `ht_single` de `std_http.titan`: HTTP por TCP y HTTPS por el TLS nativo (`std_tls.titan`: SNI, validación de cadena y nombre contra las anclas embebidas de `std_roots.titan`), sin subprocesos ni `openssl`. Comparada con la VM mediante un servidor HTTP local real (JSON, gzip, POST JSON, formulario, cabeceras, autenticación, redirección, límite de redirecciones, 404); HTTPS comprobado contra `pypi.org`, `registry.npmjs.org` y `files.pythonhosted.org` (`tests/tls_e2e/probar.sh`). |
| std::wifi (4 de 4: scan, connection_info, set_enabled, signal_bars) | `std_wifi.titan` | lo mismo que wifi_mod.rs: lanza de verdad `termux-wifi-scaninfo` / `connectioninfo` / `enable` (como `Command` de Rust). Si no está el CLI: `termux-api CLI '…' is not installed. Run: pkg install termux-api`. Barras 0–4 como WifiManager. Comparado: pruebas `wifi_std` y `wifi_errores` idénticas a la VM |
| std::dns (7 de 7: resolve, resolve_ipv4, resolve_ipv6, resolve_mx, resolve_txt, resolve_cname, reverse) | `std_dns.titan` | lo mismo que dns_mod.rs sobre hickory-resolver 0.24.4: `/etc/resolv.conf`, `/etc/hosts`, zonas especiales `localhost` / `127.in-addr.arpa` / `::1`, consultas DNS reales por UDP. Comparado: pruebas `dns_std` y `dns_errores` idénticas a la VM |

| std::pdf (9) | `native/pdf/std_pdf.titan` | implementación Titan de registro acotado y escritura real de PDF 1.4 (páginas, Helvetica/WinAnsi, texto, color, líneas, rectángulos, xref y guardado con reemplazo atómico). La prueba acepta el whitespace PDF válido y comprueba `/Type /Pages`, `/Count 2`, xref y EOF. CI `37278754059` y `37278754144` pasó `pdf_smoke` con código 0; es validación de esa prueba, no de todas las operaciones PDF. |
| std::game (5) | `std_game.titan` | reloj con `CLOCK_MONOTONIC` para medir tiempo real entre cuadros y FPS; colisiones AABB con comprobación de valores finitos y tamaños válidos. `game_std.titan` compara inicio, medición, límites y colisiones con la VM |
| std::image (21) | `native/image/` | lectura y escritura real de BMP, PNG, JPEG, GIF y WebP; módulos incluidos en `native/runtime.titan`. Las pruebas `image_*` incluyen round-trip y entradas malformadas y exigen código 0 en los casos positivos. Se corrigieron dos divergencias WebP: VP8L acepta símbolos duplicados en árboles Huffman simples (RFC 9649; aceptados por `image-webp`) y, en contenedores extendidos, VP8X determina el tipo de color y el presupuesto de bytes aunque discrepe el bit alfa de VP8L. Se añadieron dos fixtures diferenciales 1x1, pendientes de ejecutar con el seed precompilado. CI `37396772665` pasó nativo vs VM; `37396772922` pasó LLVM x86-64 y ARM64 real (313/313 idénticos en cada batería). En el primer run ARM64 con guard positivo (`37272142744`), `image_gif_validate` y `image_webp_validate` fallaron porque el arnés no compartía los archivos de sus productores; `7a67de3` corrigió las rutas y la repetición pasó. Detalles: `native/image/NOTAS.md` |
| std::input (8/8) | `std_input.titan` | estado persistente de teclado, ratón y multitáctil; casts enteros como Rust, límite de 256 bytes UTF-8 por tecla, 256 teclas y 32 puntos táctiles, liberación y reutilización de slots. `input_std.titan` cubre esos límites, botones, coordenadas y retirada de puntos. CI `37278754059`: shard 2, 77/77 idénticos con el Zett precompilado; el guard exige que `input_std` termine con código 0. La implementación guarda y consulta estado: no genera eventos ni afirma captura de entrada física. |

Defectos reales de la versión en Rust encontrados al hacer de verdad
`std::freestanding` y corregidos en las dos versiones:
- `generate_linker_script` escribía `OUTPUT_FORMAT("elf64-i386:x86-64")` para
  x86-64 y `OUTPUT_FORMAT("elf64-riscv:rv64")` para RISC-V: ningún enlazador
  los acepta (probado: ld.lld "unknown output format name" y GNU ld "target
  not found"). Ahora `elf64-x86-64`, `elf64-littleriscv` y
  `elf64-littleaarch64` (los aceptan los dos enlazadores).
- Con un objetivo desconocido los dos generadores devolvían en silencio el
  texto de aarch64 (con el nombre pedido en el comentario): ahora es un error
  (`unsupported freestanding target '…'`), igual que con dirección o pila
  negativas o un símbolo de entrada que no es un nombre (evita meter texto en
  el ensamblador).
- RISC-V figuraba como admitido, pero su `_start` era `b .` (que ni siquiera
  es una instrucción de RISC-V): ahora pila, `.bss` a cero y llamada, de
  verdad. Las comparaciones de direcciones del arranque eran con signo
  (`b.ge`, `jge`): ahora sin signo (`b.hs`, `jae`).
- `std::metrics` (histogramas): el mínimo y el máximo usaban `f64::min/max`,
  que con 0.0 y -0.0 pueden devolver cualquiera de los dos; salía distinto en
  x86-64 (`maxsd`) y en ARM64 (`fmaxnm`), el mismo programa daba otro
  resultado según la máquina. Ahora -0 < +0 (IEEE 754-2019) en todas.

`std::freestanding_memory` (7), `std::freestanding_cpu` (7) y
`std::freestanding_mmio` (7) — `std_freestanding_hw.titan` — ya son de verdad
(eran simulaciones en Rust: `dispatch_exception` devolvía
`manejador ^ dirección ^ código`, `invoke_syscall` sumaba los argumentos con el
comentario "Simular respuesta", `map_page` guardaba un `HashMap`, los
registros MMIO eran otro `HashMap`). Ahora, en un programa sin sistema
operativo (`aarch64-none`):
- `map_page` escribe las tablas de páginas del procesador (TTBR0_EL1, 4 KiB,
  39 bits): recorre niveles 1-3, parte bloques de 1 GiB / 2 MiB en tablas
  nuevas con los mismos atributos, escribe la entrada (AF, SH, AttrIdx normal
  o dispositivo con el flag 8, AP solo lectura sin el flag 1, EL0 con el 4,
  PXN/UXN sin el 2) e invalida la TLB. `translate_page` recorre las tablas de
  verdad (bloques incluidos).
- La tabla de vectores es real (`lv_vectors` en `llvm.titan`: 16 entradas de
  128 bytes; guardan x0-x30, ELR, SPSR y q0-q31 en un marco de 800 bytes y
  vuelven con `eret`). El arranque la pone en VBAR_EL1 desde el principio: un
  fallo del programa da `unhandled CPU exception (vector N): ESR=… [FAR=…]
  ELR=…` en vez de colgar la máquina. `init_exception_table` la copia a la
  dirección pedida (caché de datos a memoria, caché de instrucciones
  invalidada) y la pone en VBAR_EL1. Las excepciones llaman a las funciones
  Titan registradas (closures, con capturas): `manejador(far, esr)` distinto
  de 0 salta la instrucción; `invoke_syscall` ejecuta `svc #0` de verdad (x8 =
  número) y `manejador(a0, a1, a2)` deja el resultado en x0 (-38 si no hay).
  Las IRQ siguen enmascaradas.
- `read/write_mmio_u32`: accesos volátiles de 32 bits (región registrada,
  alineados a 4; si el acceso falla y un manejador lo salta, dicen que
  falló). `serial_init` programa un PL011 de verdad (reloj de 24 MHz: IBRD,
  FBRD, 8 bits con FIFO, TX/RX); `serial_write_str` espera a que haya sitio
  (FR.TXFF) y escribe en DR; el búfer guarda los bytes exactos.
Con sistema operativo (Linux y la VM de Rust) lo que toca el hardware da el
error honesto `requires a bare-metal program (build with target
aarch64-none): …` (el sistema es el dueño de las tablas, los vectores y los
registros; la VM no puede llamar a closures desde una nativa); el reparto de
marcos y la lista de regiones funcionan igual en los dos. Pruebas:
`tests/bare/cpu_llamadas`, `memoria_paginas` (lo escrito por la dirección
virtual nueva se lee por la física del marco), `uart`, y `memoria_fallos`
(solo QEMU: escribir en solo lectura y leer sin mapear dan un *data abort*
real que atiende un manejador Titan; en unicorn se comprobó que el fallo
ocurre justo ahí, pero unicorn no lo entrega al programa);
`tests/native/freestanding_hw_hosted` (idéntico con el backend propio y con
LLVM). `native/bare/correr.py` hace la entrada a excepción de la arquitectura
para `svc`/`brk`/instrucción no definida (unicorn no la hace) y guarda los
registros de control del PL011.

Defectos reales de la versión en Rust encontrados aquí y corregidos:
- La tabla de vectores se aceptaba alineada a 1 KiB; VBAR_EL1 exige 2 KiB
  (bits 10:0 a cero). Ahora 2048.
- El búfer del UART convertía cada byte en un carácter latin-1: "ñ" salía
  "Ã±". Ahora son los bytes exactos (leídos como UTF-8, U+FFFD si no vale).
- La VM truncaba en silencio a 32 bits el vector, los flags, el número de
  llamada, el valor MMIO y los baudios (`as u32`: el vector 4294967296 era el
  0) y convertía direcciones negativas en enormes (`as u64`). Ahora las
  direcciones y tamaños negativos son un error y los valores fuera de rango
  dan `false`.

Defecto real de la versión en Rust encontrado con std::router y corregido en
las dos versiones: una ruta con 26 o más parámetros hacía `panic!` dentro de
matchit ("Too many route parameters.") y terminaba el programa entero (código
101; `try::catch` no lo atrapa). Ahora es un error normal
`router error: Too many route parameters.` (`router_mod.rs` repite el
recorrido de `normalize_params` antes de insertar, con su prueba).

Defectos reales de la versión en Rust encontrados con std::compress y
corregidos en las dos versiones (datos corruptos que se daban por buenos):
- Un flujo deflate o zlib cortado devolvía `Ok` con lo que hubiera salido
  (flate2 convierte el "faltan datos" de miniz en un fin de archivo normal):
  `deflate_decode` de medio flujo daba `Ok("hola mundo, hola T")` y el de
  `"xyz"` daba `Ok` vacío. Ahora es `compression I/O error: unexpected end of
  file`, como ya pasaba con gzip.
- Una referencia a antes del principio de la salida (distancia mayor que lo
  escrito) no daba error: flate2 usa miniz con un diccionario circular de
  32 KiB lleno de ceros y copiaba esos ceros inventados (`Ok(61006100)` donde
  zlib dice "invalid distance too far back"). Ahora es `corrupt deflate
  stream`.
`compress_mod.rs` ya no usa los lectores de flate2 para descomprimir: llama a
miniz_oxide sobre toda la entrada en modo no circular (que comprueba las
distancias y distingue "faltan datos") y lee la cabecera y la cola gzip con
las mismas reglas y mensajes que flate2; con sus pruebas.

Defecto real de la versión en Rust encontrado con std::archive y corregido
en las dos versiones: `zip_unpack` usaba el lector de la crate zip, que lee
el deflate con flate2 y tenía los dos defectos de std::compress. Un zip con
el deflate cortado, sin bloque final o con una distancia a antes del
principio daba `Ok` con los datos a medias o con ceros inventados si la CRC
coincidía (un zip hecho así lo pasa). Ahora `zip_unpack` lee los datos en
bruto (`by_index_raw`), descomprime con miniz_oxide en modo estricto y
comprueba la CRC: esos casos dan `archive I/O error: unexpected end of file`
o `corrupt deflate stream`, y un almacenado al que le faltan bytes da
"unexpected end of file" en vez de "Invalid checksum". Con sus pruebas en
`archive_mod.rs` y en `archive_defectos`.

Defecto real encontrado y corregido: `std::array::set` fuera de rango decía
"index out of bounds" en el runtime nativo; la VM dice "array index out of
bounds".

Defecto del runtime nativo encontrado con datetime y corregido: al imprimir un
error sin capturar, `rt_flatten`/`rt_trim` solo reconocían espacios ASCII; el
CLI de Rust usa `split_whitespace`/`trim`, que reconocen todos los espacios
Unicode (por ejemplo U+3000). Ahora se usan los mismos.

Pendiente en esta fase:
- Rendimiento: `std::json` sobre 1,2 MB tarda ~0,42 s en nativo frente a
  ~0,11 s en la VM (serde en Rust optimizado). El tiempo se reparte entre el
  asignador de memoria y la conversión de floats; se mejorará junto con el
  asignador.
- Zonas horarias: calcular las 597 tablas tarda ~3 s en total (~5 ms la
  primera consulta de cada zona); la VM las trae ya calculadas.
- Criptografía lenta: el generador de código aún no guarda variables en
  registros, así que los bucles de cálculo intenso van 15-45 veces más lentos
  que la VM (Rust optimizado): `std::crypto` sobre los casos de prueba 2,6 s
  frente a 0,06 s; `hash_argon2` (m=19456, t=2) 1,3 s frente a 0,04 s;
  `hash_bcrypt` costo 10: 1,1 s frente a 0,08 s. Los resultados son idénticos;
  se resolverá con la asignación de registros.
- El resto de módulos (net,
  http, process…) y las herramientas del CLI.

## Corrección de cifras (2026-10-08)

Al compilar de verdad el runtime nativo se vio que `main` no compilaba **ningún**
programa nativo (ni un «hola mundo»): 63 errores en `native/std_audio.titan` y
`native/std_tokenize.titan`, que usaban funciones inexistentes
(`std::array::add`, `std::array::length`, `std::raw::cast`,
`std::raw::float_bits_to_float`, `std::text::char`, `std::process::current_pid`,
un método `.length`) y nunca se habían compilado. Esas cifras no eran reales.
Se hizo esto, sin simular nada:

- **Eliminado `std_tokenize.titan`** (10 funciones). Era un BPE de juguete (corta
  por espacios, BPE por carácter) que no se parece al crate `tokenizers` de
  HuggingFace que usa Rust (normalizadores Unicode, WordPiece, Unigram,
  ByteLevel, plantillas…). Queda **pendiente** como `0/10`.
- **Eliminada la sección `engine_*` de `std_audio.titan`** (22 funciones). Estaba
  descrita como «motor sobre mpv», pero el Rust real no usa mpv (decodifica
  MP3/FLAC/Vorbis/Opus por su cuenta y sale por `cpal`), guardaba crossfade,
  gapless y ecualizador sin aplicarlos y compartía el bloque de estado
  (`3544`) con `player_*`. Queda **pendiente**.
- **`std::server`**: el fallo «`accept` devuelve −1» no era de `accept`; era
  `sv_to_str`, que creaba *bytes* en vez de *string*. Después se **reescribió
  el módulo entero** como port de `server_mod.rs` (2412 líneas) y se probó con
  `selfhost/tests/server_diff/`: el mismo guion de cliente contra la VM de Rust
  (oráculo) y contra el binario nativo; lo que ve el cliente (153 líneas) y lo
  que imprime el servidor (641 líneas) es idéntico, en dos ejecuciones
  seguidas. Cubre errores de argumentos, cabeceras y cuerpos malformados,
  trailers, chunked, límites de tamaño y de handles (256 peticiones, 64 ws),
  upgrade con cabeceras inválidas, ping/pong, fragmentación, cierres e inactividad.
  **Desviaciones conocidas que no se reproducen:**
  1. el límite de 8 operaciones concurrentes (`reserve_operation`): el nativo
     no tiene hilos que se solapen;
  2. las peticiones nunca pasan a estado «closed» (en Rust solo ocurre en
     `cleanup_runtime`);
  3. tras un error de escritura en un WebSocket, la entrada queda registrada
     (cerrada) hasta `ws_close`.
  No se ha probado más allá de ese guion: IPv6, clientes lentos con muchas
  conexiones y concurrencia real no están en la prueba.
- **Bug del backend nativo encontrado con esa prueba y corregido**: un string
  literal con forma `r<cifras>` (p. ej. `"r70000"`) se confundía con la etiqueta
  de una función del runtime al enlazar, y el compilador nativo moría con
  `index 70000 out of bounds`. `bk_build` ahora solo mira referencias de código
  (`rel`/`abs`).
- `selfhost/revisar_runtime.titan` revisa el runtime con el typechecker de
  Titan; `cobertura.sh` lo ejecuta primero y **no imprime cifras** si el runtime
  no compila (`SIN_REVISION=1` lo salta, solo para depurar).
- Punto fijo verificado de nuevo tras la corrección: `titanc1 == titanc2 ==
  titanc3`, SHA-256 `73a2f3a4be5e1e48147e33470b0df63276524dbe14bc9bce82c2a3456b30cdc0` (2026-10-08, tras TCP, Server, Runtime, Ws y Http; antes `d881b9ee…` tras añadir tareas y canales; antes, con `CallMethod` y las instrucciones de colección al backend; antes `394f88d6…cbb798`, tras corregir el desempate de floats en JSON/YAML; antes `0959bdb9…e2be089`, tras añadir la caché del runtime y separar `native_build`/`wasm_cmd`; antes `83f0c30f…da82c` con Titan.toml en el cargador, y `6953bd38…78cf0` con la corrección del backend).

## Inventario de lo que sigue en Rust (2026-10-08)

Líneas de `.rs` por crate (`crates/`, 19 crates con código; ≈70 000 líneas) y
estado real frente a Titan. «Hecho» solo significa lo que está verificado arriba.

| Crate | Líneas | Estado |
|---|---:|---|
| `titan_stdlib` | 33 875 | 740/812 firmas con cuerpo Titan en el runtime nativo. Faltan 72: `web` 53, `wasm` 14, `onnx` 5 (`load_bert`, `load_bert3`, `run_bert`, `run_bert3`, `run_bert_pooled`: eliminadas del nativo, ver «std::onnx en Titan»). La VM sigue usando estas funciones en Rust. |
| `titan_vm` | 13 036 | Sin sustituto: el intérprete/VM que ejecuta `zett run`. El compilador nativo lo reemplaza para programas compilados, pero la VM sigue siendo el oráculo de las pruebas y arranca la etapa 1 del bootstrap. |
| `titan_typechecker` | 7 280 | Hecho en Titan (`typechecker.titan`, fase 3, verificado contra Rust). |
| `titan_wasm` | 6 176 | Portado a Titan: `wasm.titan` (3 858 líneas) + `wasm_cli.titan`; salida idéntica byte a byte a `zett wasm` en el corpus de `tests/wasm_diff` (ver «Fase W»). Ya es el subcomando `titan wasm` de la CLI única (ver «CLI única»). Faltan las pruebas de los 58 tests unitarios de Rust. |
| `titan_codegen` | 2 234 | Hecho en Titan (`codegen.titan`, fase 4a). |
| `titan_parser` | 2 145 | Hecho en Titan (fase 2). |
| `titan_pkg` | 1 835 | Hecho en Titan: `loader.titan` (proyectos), `semver.titan`, `pkg.titan` (registro HTTPS, resolución, tar.gz, `add`, `fetch`, `update`, `keygen`, `pack`, `publish`) y Ed25519 nativo (ver «Gestor de paquetes en Titan»). |
| `titan_cli` | 1 237 | Parcial: `selfhost/titan.titan` cubre `new`, `check`, `run`, `test`, `wasm`, `version` y añade `compile` (nativo), `bytecode`, `build` (artefacto `.tbc`) y `exec`. `add`, `fetch`, `update`, `keygen`, `pack` y `publish` ya están (ver «Gestor de paquetes en Titan»). `debug`, `repl` y `dap` ya están (ver «Depurador, DAP y REPL en Titan»). |
| `titan_lsp` | 916 | Hecho en Titan: `lsp.titan` + `titan lsp` (ver «LSP en Titan»). |
| `titan_lexer` | 621 | Hecho en Titan (fase 1). |
| `titan_postgres` | 554 | Sustituido por `std_postgres.titan` (ver «PostgreSQL nativo»); la VM de Rust lo sigue usando. |
| `titan_dap` | 547 | Hecho en Titan: `dap.titan`, `native/dap_core.titan` y `native/std_dap.titan` + `titan dap` (ver «Depurador, DAP y REPL en Titan»). |
| `titan_sqlite` | 544 | Sin sustituto (`std::sqlite`): exigiría un motor SQL completo en Titan. |
| `titan_ast` | 459 | Definiciones compartidas; reemplazadas por `ast.titan`. |
| `titan_mysql` | 455 | Sustituido por `std_mysql.titan` (ver «MySQL nativo»); la VM de Rust lo sigue usando. |
| `titan_tls` | 206 | Sustituido por `std_tls.titan` (ver «TLS nativo»). Ya no se necesita para el nativo; la VM sigue usándolo. |
| `titan_gc` | 124 | Metadatos del GC de la VM; el nativo tiene su propio gestor de memoria (fase M). |
| `titan_runtime` | 110 | Tareas y canales de la VM. Sustituido en el backend nativo x86-64 por fibras cooperativas (`native/std_task.titan`, ver «Tareas y canales nativos»); falta `SpawnQuota` y los contadores `Runtime*`. |
| `titan_macros` | 26 | Macros de Rust. |

Para poder borrar los `.rs` falta, como mínimo: las 116 firmas pendientes,
un sustituto
de la VM (o aceptar que `zett run` pase a compilar y ejecutar en nativo).

## Titan.toml en el cargador (2026-10-08)

`loader.titan` ahora implementa `SourceProject::load` completo: `ld_load_project`
(raíz = el `Titan.toml` más cercano hacia arriba, fuentes en `<raíz>/src`),
dependencias por ruta (también transitivas, con detección de ciclos), dependencias
instaladas (`Titan.remote.lock` → `.titan/packages/<nombre>/<versión>`), `lib.titan`
para `import alias`, raíces permitidas, `ld_default_entry` (directorio →
`src/main.titan`) y `ld_write_lock` (`Titan.lock` con el mismo JSON que serde).
El TOML lo lee un analizador propio (`ld_toml_parse`) y la versión se valida con
reglas de semver. `ld_load` sigue devolviendo solo los elementos, así que los
compiladores no cambian de interfaz.

Verificación: `selfhost/tests/pkg_diff/run.sh` crea 24 proyectos en `/tmp/pkg_diff`
y compara `zett wasm` con `wasm_cli` (`.wasm`, mapas, `Titan.lock`, salida y código
de salida). Los 24 coinciden: proyectos con dependencias anidadas, tablas en línea,
claves con puntos, TOML con comentarios/multilínea/arreglos, dependencia instalada,
y errores (ruta inexistente, dependencia sin ruta, ciclo de dependencias, ciclo de
imports, import sin resolver, sin `src/`).

Regresión tras el cambio: punto fijo ✅ (`83f0c30f…da82c`, etapas 1=2=3) y
`verify_codegen.sh` con el compilador nativo: 954/954 archivos con bytecode idéntico a Rust.

Diferencias conocidas:
- Los errores de **manifiesto inválido** tienen el mismo código de salida y el mismo
  prefijo (`invalid manifest at <ruta>: invalid Titan.toml…`), pero no el texto del
  crate `toml` (que incluye línea/columna y un recuadro). Lo mismo con
  `Titan.remote.lock` (`missing field` sin «at line … column …»).
- TOML no soportado, con error explícito: arreglos de tablas `[[x]]`. Los valores
  escalares no cadena (números, fechas, booleanos) se aceptan comprobando solo su
  forma, no su valor; el manifiesto no los usa.
- `titan_pkg` (registro, `publish`, `add`/`sync`, tar.gz, ed25519) se portó después: ver «Gestor de paquetes en Titan».

## CLI única `titan` (2026-10-08)

`selfhost/titan.titan` es el ejecutable de línea de comandos escrito en Titan
(`./selfhost/titanc3 selfhost/titan.titan selfhost/titan`). Reparte en subcomandos
lo que antes eran programas sueltos:

| Comando | Qué hace | Equivalente en Rust |
|---|---|---|
| `titan new <ruta>` | crea `Titan.toml` + `src/main.titan` | `zett new` (mismos mensajes y errores) |
| `titan check [entrada]` | carga, revisa tipos y genera bytecode; imprime `CHECK OK: N files, M functions` | `zett check` |
| `titan run [entrada] [args…]` | compila a un ejecutable nativo temporal, lo ejecuta, borra el temporal y devuelve su código de salida | `zett run` (que usa la VM) |
| `titan test [entrada]` | compila y ejecuta cada `tests/**/*.titan` y resume `test result: …` | `zett test` |
| `titan compile [entrada] [-o salida]` | ejecutable nativo x86-64 (por omisión `<raíz>/target/<nombre>`) | no existe |
| `titan build [entrada] [-o salida]` | artefacto `.tbc` (por omisión `<raíz>/target/<nombre>.tbc`) idéntico byte a byte al de `zett build`, con el mismo resumen | `zett build` |
| `titan exec [artefacto.tbc]` | decodifica y valida el `.tbc`, lo compila a nativo (temporal) y lo ejecuta | `zett exec` (que usa la VM) |
| `titan wasm [entrada] [-o salida]` | WebAssembly + mapas de fuente | `zett wasm` |
| `titan bytecode <entrada>` | volcado del bytecode | `zett bytecode` |
| `titan version` | `TITAN Language Compiler v1.2.0` | `zett version` |
| `titan add/fetch/update/keygen/pack/publish` | gestor de paquetes (ver «Gestor de paquetes en Titan») | los mismos de `zett` |

Los errores salen por stderr con el formato de Rust (`ETIQUETA: causa` aplanada, el
mensaje multilínea repetido debajo y, bajo GitHub Actions, la anotación `::error`)
y código de salida 1. Todos los comandos de `zett` están portados; solo `titan debug --sandbox` y `titan dap` con
`sandbox: true` se rechazan (la VM sandboxeada no existe en los ejecutables nativos).

Verificación contra `zett` (el oráculo):
- `titan check` sobre 419 archivos (`examples/*.titan` y `selfhost/tests/*/*.titan`,
  correctos y con errores): stdout + stderr + código de salida idénticos en los 419
  (`async_demo` solo coincide con `ZETT_STDLIB_DIR` apuntando a `stdlib/`; el cargador
  de Titan busca `stdlib/` subiendo directorios y el `zett` precompilado no la encuentra).
- `titan new`: mismos archivos; `titan wasm`: `.wasm` y mapas idénticos a `zett wasm`
  (mismo nombre de salida en directorios distintos); `wasm_diff` y `pkg_diff` siguen en
  «TODO IGUAL».
- `titan test` sobre un proyecto con 2 pruebas correctas (una en subdirectorio), una
  con fallo en ejecución (`index 7 out of bounds`) y una con error de tipos: el mismo
  resultado `2 passed; 2 failed` y código 1 que `zett test`.
- Punto fijo del bootstrap ✅ con el código nuevo (`0959bdb9…2be089`).

Diferencias con la VM que hay que conocer (no están ocultas):
1. **`run` y `test` compilan de verdad un ejecutable nativo**; no hay intérprete.
   Eso cuesta ≈6 s por programa con la caché del runtime caliente (ver abajo) y ≈45 s
   la primera vez con un compilador nuevo.
2. El programa hijo se ejecuta con su salida **capturada** (`std::process::run_timeout`):
   stdout/stderr aparecen al terminar, no en vivo, y **no hereda stdin**. Un
   programa interactivo no funciona con `titan run`; compílese con `titan compile` y
   ejecútese directamente.
3. La VM imprime `=> valor` cuando `main` devuelve algo; el ejecutable nativo no.
4. En `titan test` la salida de cada prueba se imprime antes de su línea
   `test … ok/FAILED` (Rust la mezcla dentro de la línea) y el mensaje de un fallo en
   ejecución lleva el prefijo `RUNTIME ERROR:`.
5. `--sandbox` se rechaza (el nativo no tiene modo sandbox). `run` reenvía los
   argumentos al programa; Rust los ignoraba.
6. El ejecutable busca `native/runtime.titan` junto a sí mismo: debe estar en
   `selfhost/` (o con un directorio `native/` al lado).
7. stderr se escribe abriendo `/dev/stderr` (no hay un nativo para el descriptor 2);
   si no se puede abrir, se usa stdout.

### Caché del frente del runtime (`rtcache.titan`)

Medición: compilar `hello.titan` tardaba ≈41 s y el 98 % era cargar (11,7 s),
revisar tipos (18,0 s), generar (6,1 s) y optimizar (4,4 s) `native/runtime.titan` +
`std_*` (4 707 elementos) en cada compilación; el programa son 0,16 s y el backend 2 s.
`native_build.titan` ahora guarda lo único que el backend lee del runtime (nombre,
aridad, capturas, locales y operaciones de cada función, más la tabla de cadenas) en
`native/.runtime-cache-<clave>.json` (≈8 MB, ignorado por git). La clave es el
SHA-512 del propio ejecutable del compilador y el de cada `.titan` bajo `native/`:
si cambia cualquiera, la caché no se usa. Bajo la VM (`zett run build.titan`) no hay
ejecutable que identificar y no se usa; `TITAN_CACHE=0` la apaga. Se conservan como
mucho 4 archivos de caché.

Resultado: `hello.titan` 41 s → 5,9 s; `bytecode.titan` 58 s → 15,8 s. Los ejecutables
producidos son **idénticos byte a byte** con y sin caché (comparados `hello` y
`bytecode.titan` con los de `titanc3` antes del cambio). La primera compilación con un
ejecutable nuevo sigue costando ≈41 s (genera la caché).

Refactor asociado: `native_build.titan` (`build`, `build_runtime`), `wasm_cmd.titan`
(`wa_run`, antes en `wasm_cli.titan`) y `cli_util.titan` son bibliotecas; `build.titan` y
`wasm_cli.titan` quedan como `main` finos. `LdProject` gana el campo `files`.

## Fase W — backend WebAssembly en Titan (2026-10-08)

`selfhost/wasm.titan` es una traducción de `crates/titan_wasm/src/lib.rs`
(secciones, análisis de pila, inferencia de tipos para `+`, regiones `if`/`loop`
directas o despachador `br_table`, mapas/arrays/strings en memoria lineal, las
53 importaciones de `std::web`, los 14 `std::wasm::heap_*`, mapa lógico
`.map.json`, mapa estándar `.map`). `selfhost/wasm_cli.titan` lo envuelve como
`wasm <archivo.titan> [--output salida.wasm]` (loader + typechecker + codegen de
Titan; imprime las mismas tres líneas que `zett wasm`).

Verificación (`selfhost/tests/wasm_diff/run.sh`, ≈10 s con el ejecutable ya
compilado): para cada programa compara con `zett wasm` el `.wasm`, el `.map`, el
`.map.json`, la salida y el código de salida; después ejecuta el `.wasm` del port
en Node 22 y compara `main()` con la VM, y corre `examples/browser/host.js` (el
real) sobre un DOM mínimo y compara el registro de DOM con el del `.wasm` de Rust.

| Programa | Qué ejercita | Resultado |
|---|---|---|
| `examples/browser/main.titan` | DOM, canvas, fetch, eventos, animación, strings de ida y vuelta | idéntico (38 528 B); host.js real: mismo registro (423 líneas) |
| `web_all.titan` | las 53 funciones de `std::web` | idéntico; `WebAssembly.validate` |
| `heap.titan` | los 14 `std::wasm::heap_*` | idéntico; ejecutado en Node (la VM no ejecuta `std::wasm`) |
| `arith`, `control` | recursión, `while`/`loop`/`break`/`continue`, `match`, ramas anidadas | idéntico; Node = VM |
| `arrays`, `arrays2` | `set/push/pop/slice/concat`, índices, tuplas, arrays en structs | idéntico; Node = VM |
| `maps` | `insert/insert_new/get/contains/remove/keys/values/length` | idéntico; Node = VM |
| `strings`, `textops` | concatenación, `equals`, `hash64`, claves de mapa, UTF-8 | idéntico; Node = VM |
| `structs_enums` | structs anidados, enums con carga | idéntico; Node = VM |
| `err_range`, `err_ambiguous_add` | rutas de error | mismo mensaje y código de salida |

Cobertura medida con una copia instrumentada: 105 de las 109 funciones del port
con cuerpo propio se ejecutan en este corpus (a todas las `wa_i32_*`/`wa_i64_*`
generadas no se les cuenta). Sin ejecutar: `wa_drop`, `wa_select`, `wa_hex`,
`wa_hex64` (el último solo se usa en el error de colisión de etiquetas de enum).

Desviaciones y límites, sin maquillar:
- **`sourcesContent`** se lee del disco (Rust lo toma de `project.sources`); con
  `Titan.toml` ya funciona (ver «Titan.toml»).
- **`TakeLocal`.** `codegen.titan` emite `TakeLocal` (mover); el `zett` publicado
  (compilado desde `3231a66`) lo trata como `PushLocal` en Wasm. El código de
  `crates/` en `main` ya no tiene `TakeLocal` en ningún crate, así que `main` y
  `3231a66` difieren; el port normaliza `TakeLocal` a `PushLocal` para el análisis
  y la emisión, y el mapa lógico conserva el texto original (`TakeLocal(n)`), igual
  que el binario publicado. **El oráculo es ese binario, no un `cargo build` de `main`**
  (no hay cargo en el entorno).
- **`==` entre strings en Wasm compara handles**, no contenido (también en Rust). Con
  strings construidos en ejecución da falso; hay que usar `std::text::equals`. El port
  lo reproduce byte a byte; `textops.titan` usa `equals` por eso.
- Los mensajes de error de `StringDataTooLarge`, colisión de etiquetas de enum,
  `InvalidString` y similares no se comparan con Rust (no hay programa que los dispare).
- No se portaron los 58 tests unitarios de Rust (construyen `Op` a mano).
- La cobertura de `cobertura.sh` no cuenta `std::web`/`std::wasm`: `std::web`/`std::wasm` no son
  funciones del runtime nativo, son importaciones/globales del módulo Wasm. Su medida
  es la de esta sección: 53/53 importaciones en `web_all` y 14/14 funciones de heap en
  `heap`, con bytes idénticos a Rust. Que `host.js` siga siendo JavaScript es parte del
  diseño del navegador, no una dependencia de Rust.

## std::onnx en Titan (2026-10-08)

`std::onnx::{load, load_shape, close, input_count, output_count, input_shape,
output_shape, run_f32, run_ids}` (9 de 14 firmas) tienen ahora cuerpo Titan en el
runtime nativo; no hay tract ni ningún crate detrás.

**Dónde está.** `selfhost/native/std_onnx_engine.titan` (motor, Titan sin `std::raw`,
así que corre igual bajo la VM de Rust) y `selfhost/native/std_onnx.titan` (la API:
registro de handles en las ranuras globales +3536/+3544, límites y errores).

**Qué hace de verdad.**
- Lee el protobuf (`ModelProto`, `GraphProto`, `NodeProto`, `TensorProto`,
  `AttributeProto`, `ValueInfoProto`, opset) a mano. Tensores FLOAT, INT32, INT64,
  con `raw_data` o campos tipados.
- 39 operadores: Add Sub Mul Div (con broadcasting), MatMul (por lotes), Gemm, Conv
  (1-D/2-D, grupos, dilatación, `pads`, `auto_pad`), MaxPool, AveragePool
  (`ceil_mode`, `count_include_pad`), GlobalAveragePool, GlobalMaxPool,
  BatchNormalization, Relu, LeakyRelu, Sigmoid, Tanh, Exp, Log, Sqrt, Neg, Abs, Clip
  (atributos y entradas), Softmax y LogSoftmax (semántica anterior y posterior al
  opset 13), Flatten, Reshape, Transpose, Concat, Gather, Squeeze, Unsqueeze, Shape,
  Cast, Constant, Identity, Dropout, ReduceMean, ReduceSum, ReduceMax.
- Un operador fuera de esa lista se rechaza **al cargar**: `onnx error: unsupported
  operator 'X'`. No hay ruta que lo ignore o lo aproxime.
- `input_shape`/`output_shape` devuelven -1 en las dimensiones dinámicas, igual que
  tract; la forma de salida se deduce ejecutando el modelo con unos con las
  dimensiones dinámicas valiendo 1 y 2 (lo que cambia es dinámico) y se guarda en caché.
- Aritmética en f64 con redondeo a f32 al salir de cada operador. tract acumula en
  f32, así que los números coinciden en ≈1e-6 relativo, no bit a bit.

**Cómo se verificó** (`bash selfhost/tests/onnx/run.sh`; necesita `numpy`, `onnx` y
`onnxruntime` en `OXPY`, p. ej. un venv): 25 modelos generados con `onnx.helper`
(MLP con Gemm y con MatMul+Add, CNN con BatchNorm, red tipo MNIST 1×28×28, Conv con
grupos/dilatación/SAME/1-D, pooling con relleno y `ceil_mode`, broadcasting, MatMul por
lotes, reshape/transpose/concat/gather/reduce, Softmax antiguo y nuevo, entrada INT64
para `run_ids`), cada uno con y sin `load_shape`. El ejecutable nativo se compara con
**onnxruntime** y con **tract** (el `zett` de Rust): formas y número de entradas/salidas
idénticas; error máximo relativo de valores 3.4e-07 contra onnxruntime y 3.6e-07
contra tract en las 50 comprobaciones (tolerancia 1e-5). Los casos de error de la API
(14 líneas de salida: archivo inexistente,
directorio, handle desconocido, dimensión 0/negativa, rango 9, desajuste de elementos,
dimensión > 4 194 304, handle tras `close`, ids 1-5 sin reutilizar, límite de 4 handles)
dan texto **idéntico** al de Rust (tract). Archivos truncados, vacíos, basura y con operador
no soportado fallan siempre con error (nunca abortan ni devuelven handle). Una red
tipo MNIST tarda 0,1 s en el nativo.

**Diferencias con la versión de Rust, a propósito:**
- Archivo de modelo ≤ 64 MiB (Rust: 256 MiB): los tensores son arrays de Titan, 16
  bytes por elemento.
- Los textos de error de un modelo mal formado o incompatible con la entrada no son
  los de tract (los de límites, handles y formas sí).
- `run_ids` sirve para modelos con una entrada INT64 y salida f32. (Los operadores
  de transformer —Where, Erf, LayerNorm…— se añadieron después: ver «std::onnx: BERT».)
- Un solo tensor de entrada por `run_*` (igual que la API de Rust).
- tract entra en pánico si se pide `input_shape(h, i)` con `i` fuera de rango; el nativo
  devuelve un error normal.

**Punto fijo** reverificado después de este cambio: `titanc1 == titanc2 == titanc3`,
SHA-256 `0959bdb9…e089` (el mismo: el compilador no incrusta el runtime, que se lee de
`native/runtime.titan` al compilar cada programa).

**BERT** (`load_bert`, `load_bert3`, `run_bert`, `run_bert3`, `run_bert_pooled`): ver «std::onnx: BERT»
más abajo (el párrafo que decía que se había eliminado quedó obsoleto).

## Pendientes conocidos (anotados para no olvidarlos)

- **Asignación al final de una rama `if`** (corregido 2026-10-10 en las
  posiciones de sentencia): si la rama `then` terminaba en una llamada y la
  `else` en `x = [..]`, el verificador exigía que las dos ramas tuvieran el
  mismo tipo y fallaba con "expected Nil, found Array". Ahora una asignación
  en posición de sentencia vale `()` (el cuerpo de un bucle y las sentencias
  sueltas se tipan así), que era lo prescrito aquí. Queda **igual a propósito**
  en posición de valor: las colas de función y los `let` con el valor en uso
  siguen exigiendo ramas del mismo tipo (igual que `if c { 1 } else { "a" }`),
  y `let y = (x = 5)` / `fn f() -> int { x = 5 }` siguen valiendo el valor
  asignado. Pruebas: `selfhost/prueba_arreglos.titan` y los rechazos de
  `selfhost/tests/arreglos/`.
- **Tupla al inicio de línea tras un bloque** se parsea como llamada:
  `loop { ... }` + salto de línea + `(a, b)` ⇒ `loop{...}(a, b)`. En el
  código Titan se usa `return (a, b)`. El parser en Titan debe reproducir el
  comportamiento actual (el oráculo es el parser de Rust); se decidirá aparte
  si se cambia el lenguaje.
- Indexar un `string` (`s[i]`) sigue siendo O(i). El lexer usa
  `std::text::chars` una vez y luego indexa el array (O(1)).
- La clasificación Unicode usa hoy las tablas de Rust (nativas
  `is_alphabetic`, `is_alphanumeric`, `is_whitespace`, `is_uppercase`). En la
  fase 6 deben reemplazarse por tablas escritas en Titan.
- ~~parse_float / float→texto de Rust~~: hechos en Titan (`native/float.titan`)
  para los ejecutables nativos.
- Los mensajes de error del parser citan el token con el formato `{:?}` de
  Rust. Para caracteres Unicode exóticos Rust consulta su tabla de
  "imprimibles"; el parser en Titan aproxima esa tabla con los rangos
  habituales (controles, marcas combinantes, espacios de ancho cero, uso
  privado). Solo afecta al texto de errores con esos caracteres.
- No existe `print` sin salto de línea (útil para herramientas).
- Fase 2 necesita un oráculo del AST: un volcado canónico (`titan ast`) en
  Rust, como `titan_lexer::dump_tokens`, para comparar byte a byte.

## std::try::catch en ejecutables nativos

- Instrucción `TryCall` en los dos backends (propio x86-64 y LLVM x86-64/ARM64),
  con la misma semántica que la VM: `Result::Ok(valor)` o
  `Result::Err(mensaje)` sin imprimir nada; los catch se pueden anidar.
- Todo error pasa por `rt_fatal`; si hay un catch activo (`G+24`), salta a él
  con `std::raw::throw` en vez de terminar.
- LLVM: `t_setjmp`/`t_longjmp` escritos a mano para cada arquitectura.
  `llvm.eh.sjlj.setjmp` NO sirve en AArch64: allí devuelve siempre 0 y el
  longjmp no hace nada (comprobado con clang).
- Prueba: `selfhost/tests/native/try_catch.titan`.
- Incidencia concreta de LLVM detectada en `095995b`, run `37256171189`:
  el job de objetos ARM64 aceptó 308/310 programas; `termux_missing.titan` y
  `audio_termux_missing.titan` tenían `TryCall` dentro de funciones del
  runtime, que `llvm.titan` rechazaba explícitamente. Se amplió `TryCall` al
  runtime con el mismo `t_setjmp`/`t_longjmp`; ahí se omite guardar/restaurar
  la profundidad porque esas funciones no mantienen `%dp`. La corrección aún
  debe validarse con el siguiente run de LLVM; no se cuenta como paridad hasta
  que compile y pase las comparaciones reales.

## `titan build` / `titan exec` y el formato `.tbc` (2026-10-08)

`selfhost/artifact.titan` (prefijo `ac_`) es la traducción de `crates/titan_codegen/src/artifact.rs`:

- **Codificación** (`ac_encode`): valida el módulo con las mismas comprobaciones y mensajes que
  `validate()` de Rust, arma el módulo con mapas (claves en orden alfabético, igual que
  `serde_json::Value`), calcula el CRC-32 sobre su JSON compacto y escribe
  `TITAN-BYTECODE 1\n` + sobre pretty (`format_version`, `compiler_version`, `checksum_crc32`,
  `module`, en el orden de campos del struct de Rust, escrito a mano) + `\n`.
  `Call { function: usize::MAX }` (la intrínseca de rangos) no cabe en un entero de Titan: viaja
  como `-1` y se sustituye por `18446744073709551615` al escribir y al revés al leer; un salto sin
  parchear se rechaza como en Rust. Los tipos (`param_types`, `return_type`) se guardan en Titan
  como texto Debug de Rust y se convierten a/desde el JSON de serde (`ac_ty_json`, `ac_ty_text`).
- **Decodificación** (`ac_decode`): cabecera, JSON, versión de formato, CRC recalculado sobre el
  módulo y las mismas validaciones. Devuelve el `G` del generador de código, así que un `.tbc`
  puede pasar por `ot_optimize` y `bk_build`.
- **`titan exec`**: no hay intérprete de bytecode. El `.tbc` se decodifica, se optimiza, se enlaza
  con el runtime (`bk_build`) y se ejecuta como ejecutable temporal, con las mismas reglas que
  `titan run` (`=> valor` si `main` devuelve algo, código de salida del programa).

Verificación (`selfhost/tests/tbc_diff/run.sh`, contra `zett` v1.0.0):
- `build`: .tbc y salida idénticos byte a byte en 7 programas propios (floats extremos, chars,
  strings con escapes y unicode, structs/enums/métodos, closures, rangos, `try::catch`) y en
  422 archivos de `examples/*.titan` + `selfhost/tests/*/*.titan`: 401 compilan y dan el mismo
  `.tbc` (`async_demo` con `ZETT_STDLIB_DIR=stdlib/`, como en `check`) y los 21 con error de
  compilación dan el mismo mensaje y código.
- `exec` de un `.tbc` hecho por `zett`: misma salida y código en los programas ejecutables
  (28 de `tests/native` de muestra, salvo `gui_std` y `kv_persistence`, que también difieren con
  `run` porque el `zett` v1.0.0 es anterior a esas pruebas).
- 22 artefactos inválidos (cabecera, versión, CRC, truncado, entrada, saltos, locales, strings,
  llamadas, nativas, closures, rangos…): mismo mensaje y código de salida. Los errores de serde
  (JSON truncado, variante desconocida) coinciden solo en la etiqueta `INVALID ARTIFACT:
  malformed bytecode JSON:`, no en la descripción de serde.

Límites conocidos:
- Un `.tbc` válido pero con la pila desbalanceada (la VM da `stack underflow`) termina con código
  1 sin mensaje al ejecutarse en nativo: el código compilado no lleva comprobación de pila.
- `CallMethod` y las instrucciones de colección ya se compilan a nativo (ver «Métodos de `impl`…»);
  siguen sin soporte las instrucciones propias de la VM (tareas, canales, red, bases de datos).

### Corrección: floats en JSON y YAML

Al comparar los `.tbc` salió un error del runtime nativo: `std::json::stringify` y
`std::yaml::stringify` desempataban un valor exactamente a mitad entre dos candidatos redondeando
hacia arriba (`1936443305115367.25` → `…367.3`), mientras que serde_json (zmij) y serde_yaml
(ryu) eligen el último dígito par (`…367.2`); `print` sí usa el criterio hacia arriba y no cambia.
`fl_shortest_even` (en `native/float.titan`) lo corrige: el empate solo es posible con 1 a 26 bits
fraccionarios tras quitar los ceros finales de la mantisa, y solo entonces se salta Grisu y se
usa Dragon con desempate a par. Prueba: `tests/native/json_ties.titan` (JSON y YAML idénticos
a `zett`).

## Métodos de `impl` y operaciones de colección en el backend nativo (2026-10-08)

Hasta ahora el backend x86-64 rechazaba `Op::CallMethod` y las seis instrucciones de
colección (`ArrayMap`, `ArrayFilter`, `ArrayFold`, `ArrayFind`, `ArrayAny`, `ArrayAll`): cualquier
programa con un método de `impl` (`p.suma()`), con `xs.map(f)` o con `map(xs, f)` no compilaba
con `titan run`/`exec`. Ahora sí (`native/backend.titan`, `bk_call_method`; `native/runtime.titan`,
`rt_m_*`):

- **Struct**: se compara el nombre del struct (string estático único) con cada implementación
  `"<Struct>::<método>"` de la tabla de métodos que existe en tiempo de compilación y se llama a
  la función con el receptor como primer argumento. Una aridad distinta da el mismo error que la
  VM (`function 'Circle::area' expected 1 arguments, found 2`); un método inexistente,
  `undefined method 'Point::nope'`.
- **Otros valores**: `len`, `map`, `filter`, `fold`, `find`, `any`, `all`, `sort_by` con la misma
  semántica y los mismos mensajes (`map requires a function`, `filter predicate must return bool`,
  `operation requires an array`, `no method 'x' for value ...`).
- Pruebas: `tests/native/metodos_impl.titan` y `coleccion_llamadas.titan` (salida y errores
  idénticos a `zett run`); `examples/` (`traits`, `impl_structs`, `qol_higher_order`,
  `pipeline_spaceship`, `aliases_spread`, `const_and_maps`, `for_destructuring`) también.
- Backend LLVM: `ArrayMap/Filter/Fold/Find/Any/All` (las llamadas `map(xs, f)`) ya se admiten (2026-10-09, mismas rutinas del runtime; `coleccion_llamadas` idéntico). `CallMethod` sigue sin admitirse en LLVM.

## Instrucciones de la VM sin equivalente nativo (2026-10-08)

Al probar `examples/` salió que la cobertura «700/816» cuenta funciones nativas (`std::x::f`), pero
el generador de código traduce varias familias a **instrucciones propias de la VM**
(`selfhost/codegen_tables.titan`), y el backend nativo no las tiene. Un programa que las usa
termina con `instruction X is not supported by the native backend yet`. Eran 113; quedan 17 (`SpawnQuota` y `Sqlite*`) tras tareas/canales, TCP, Server, Runtime, Ws, Http, Tls, Postgres, MySQL y Db (hechos el 2026-10-08; ver «TCP, Server, Runtime, WebSocket y HTTP nativos», «TLS nativo», «PostgreSQL nativo» y «MySQL nativo»):

| Familia | Instrucciones | Qué falta |
|---|---:|---|
| Tareas y canales | `SpawnQuota` (las otras nueve ya están: ver «Tareas y canales nativos») | el tope de memoria por tarea no se puede aplicar en el runtime nativo |
| TCP | `Tcp*` (8) | ✅ hecho |
| TLS | `Tls*` (6) | ✅ hecho (ver «TLS nativo») |
| WebSocket cliente | `Ws*` (10) | ✅ hecho (`ws://`, `wss://` y `attach_tls`) |
| HTTP | `Http*` (7): router, middleware, `serve_connection`, `dispatch` | ✅ hecho |
| Servidor | `Server*` (6) | ✅ hecho |
| SQLite | `Sqlite*` (16) | motor SQL |
| PostgreSQL | `Postgres*` (16) | ✅ hecho (ver «PostgreSQL nativo») |
| MySQL | `Mysql*` (15) | ✅ hecho (ver «MySQL nativo»; probado contra un servidor de protocolo de pruebas, no contra MySQL real) |
| Base de datos genérica | `Db*` (8) | ✅ hecho sobre los handles PostgreSQL y MySQL (SQLite no existe en el nativo) |
| Introspección del runtime | `Runtime*` (11) | ✅ hecho (valores propios del runtime nativo; ver sección) |

Estas instrucciones **no** estaban en el inventario de 816 firmas, así que «700/816» no las incluye.

## Tareas y canales nativos (2026-10-08)

`spawn`, `join`, `join_timeout`, `cancel`, `channel`, `send`, `recv`, `recv_timeout` y `select`
funcionan en el backend x86-64 propio (`Op::Spawn`, `JoinTask`, `JoinTaskTimeout`, `CancelTask`,
`NewChannel`, `ChannelSend/Recv/RecvTimeout/Select`).

- **Cómo**: la VM usa un hilo del sistema por tarea; el runtime nativo no tiene hilos, así que cada
  tarea es una **fibra cooperativa** con pila propia (`mmap` de 32 MiB reservada con página de
  guarda) en el mismo hilo. El cambio de contexto es real: `std::raw::fiber_switch` →
  `g_switch` en `backend.titan` (guarda rbp/rsp, carga los de la otra pila). Cada fibra conserva su
  cadena de `std::try::catch` (`[globals+24]`) y su profundidad de llamadas (`[globals+16]`).
  Código en `native/std_task.titan`; estado en `globals+3552`.
- **Valores**: `Task`, `Sender` y `Receiver` son objetos contados con etiquetas nuevas 13/14/15
  (`<task:N>`, `<sender:N>`, `<receiver:N>`, igualdad por id).
- **Semántica copiada de la VM**: ids desde 1; máx. 256 tareas registradas (`runtime task limit
  exceeded (256)`), 1024 canales, capacidad ≤ 65 536 y ≥ 0; `join` retira la tarea (segundo `join` →
  `unknown or already joined task N`) y propaga el error de la tarea tal cual; `join_timeout`
  devuelve `Option` y deja la tarea registrada si expira; `cancel` devuelve `true` si la tarea está
  registrada; `select` sondea en orden y devuelve `Some((índice, valor))`; los canales nunca se
  desconectan; capacidad 0 = `send` espera a que alguien reciba.
- **Diferencias reales con la VM** (no hay paralelismo):
  - Una tarea solo cede en `join`, `join_timeout`, `recv`, `recv_timeout`, `select`, `send`
    bloqueado y `std::time::sleep_ms`. Una tarea que calcula sin bloquearse no deja avanzar a las
    demás hasta que termine, y un bucle de espera activa sin esas llamadas no termina nunca (en la
    VM sí, por los hilos). Las lecturas bloqueantes (red, archivos, stdin) tampoco ceden.
  - `spawn` no ejecuta nada al instante: la tarea arranca cuando alguien se bloquea.
  - `cancel` se ve cuando la tarea arranca o vuelve de un punto de cesión (la VM lo mira en cada
    instrucción); además despierta a las tareas bloqueadas (la VM no).
  - Si todas las tareas están bloqueadas y ninguna tiene plazo, es un bloqueo mutuo real: la VM
    se colgaría; aquí termina con `RUNTIME ERROR: deadlock: every task is blocked and none can ever
    wake up` (código 1). Con plazos se duerme hasta el más próximo.
  - Al salir `main`, las tareas pendientes no se ejecutan ni se esperan (como la VM).
- **Backend LLVM / ARM64**: desde 2026-10-09 las tareas y canales también funcionan por LLVM (x86-64 y ARM64): `Spawn`,
  `JoinTask`, `ChannelSend/Recv/Select`… llaman a las mismas rutinas de `std_task.titan`, y `std::raw::fiber_switch` es
  `lv_fiber_switch`, escrito en ensamblador para cada arquitectura (registros que conserva una llamada, pila y salto a la
  entrada). Verificado en un ARM64 real (ver «Linux x86-64 y ARM64/Termux» al final).
- **Sin sustituto**: `SpawnQuota` (límite de memoria por tarea).
- **Pruebas** (idénticas a `zett run`, salida, errores y código de salida): `tareas_basico`,
  `tareas_canales`, `tareas_select`, `tareas_estres` (ping-pong de 2000 viajes sin capacidad, 256
  tareas vivas, recursión de 3000 niveles en una tarea) y diez `error_tarea_*`/`error_canal_*`.
  El bloqueo mutuo se probó a mano (la VM no termina, no se puede comparar).

## TCP, Server, Runtime, WebSocket y HTTP nativos (2026-10-08)

Código: `native/std_tcp.titan`, `std_runtime.titan`, `std_ws_conn.titan`, `std_http_router.titan`
(más el refactor de `std_http.titan` en núcleos con parámetro `function`). Cableado en `backend.titan`
con `bk_call_rt`. Las esperas de fd se integran con el planificador de fibras (`tk_idle` con `poll`).

- **TCP** (`Tcp*`, 8 ops): handles con etiquetas 16/17 (listener/stream), tabla de 1024. Límites: `connect`
  y la resolución DNS bloquean sin ceder; soltar un handle no lo cierra (igual que la VM salvo `close`).
- **Server\*/Runtime\***: estadísticas (`accepted, active, completed, healthy, maximum, ready, rejected,
  shutting_down`), `health_response` 200/503, límites 1..64 MiB. Los valores de `Runtime*` son los del
  runtime nativo (`allocated_bytes` real, no la estimación de la VM; `gc_collect` → 0 porque hay conteo de
  referencias; `memory_limit` −1).
- **WebSocket** (`Ws*`): cliente `ws://` real (DNS, handshake, subprotocolo, límites), decodificador y
  conexión. `wss://` y `attach_tls` funcionan sobre el TLS nativo (ver «TLS nativo»).
- **HTTP** (`Http*`, 7 ops): `router`, `route`, `middleware`, `after`, `on_error`, `dispatch` y
  `serve_connection` (keep-alive, límite de peticiones, cuerpos partidos, peticiones mal formadas, errores
  del manejador). El router es un objeto de etiqueta 18 (`<http-router:N>`); la lista de rutas no libera
  memoria al crecer. Un router gasta un handle permanente.
- **Pruebas** idénticas a `zett run` (salida y errores): `tcp_*` (6), `server_std`, `runtime_info`, `ws_*` (5),
  `http_router`, `http_serve` (+ `http_std`, `http_errores`).
- TLS (`Tls*`, 6 ops) está hecho: ver «TLS nativo». `std::http_full` usa `ht_single`, que ya habla HTTPS por el TLS nativo (sin `openssl s_client`).

## TLS nativo (2026-10-08)

`std_tls.titan` (+ `std_crypto`, `std_hash`, `std_rsa`, `std_ec`, `std_x25519`, `std_x509`, `std_roots`) sustituye a
`titan_tls` en el runtime nativo: las seis instrucciones `Tls*` y los handles con etiqueta 19
(`<tls-stream:N>`) y 20 (`<tls-server-config:N>`).

- **Cliente:** TLS 1.3 (suites 1301/1302/1303; grupos X25519 y P-256) y TLS 1.2 con ECDHE
  (c02b, c02c, c02f, c030, cca8, cca9). Valida cadena, caducidad, nombre (DNS e IP) y firma
  (RSA PKCS#1, RSA-PSS, ECDSA P-256/P-384) contra 121 anclas embebidas (`gen_roots.py`).
- **Servidor:** TLS 1.3 con claves EC P-256/P-384 y RSA (PKCS#1/PKCS#8), cadenas de varios certificados.
- **Verificado:** `tests/tls_e2e/probar.sh` (20/20: servidor frente a `openssl s_client`, cliente frente a hosts
  públicos), `tests/tls_prims/` (X25519, ECDSA/ECDH, X.509) y TLS 1.3 con `openssl s_server` forzando cada suite
  (1301, 1302, 1303).
- **Bug encontrado y corregido** al probar PostgreSQL por TLS: la cadena de certificados apuntaba dentro del
  mensaje `Certificate`, que se liberaba al final de cada vuelta del bucle del handshake; la verificación de
  `CertificateVerify` (o de `ServerKeyExchange` en 1.2) leía memoria liberada. Según el patrón de asignaciones
  fallaba siempre con las suites AES-GCM y pasaba con ChaCha20 en el `openssl s_server` local. Ahora el
  mensaje se conserva hasta terminar el handshake (`cert_keep`).
- **Límites:** sin renegociación ni reanudación de sesión; sin autenticación de cliente (certificado vacío si
  el servidor lo pide); sin TLS 1.2 en servidor.

## PostgreSQL nativo (2026-10-08)

`std_postgres.titan` implementa el protocolo v3 (consulta extendida, sentencias preparadas con parámetros
tipados, filas como mapas) con las 16 instrucciones `Postgres*` y las 8 `Db*` (estas despachan por la etiqueta del handle:
PostgreSQL y, desde «MySQL nativo», también MySQL). Es un port de `crates/titan_postgres` (crate `postgres`).

- Autenticación cleartext, md5 y SCRAM-SHA-256; `SSLRequest` + TLS nativo (`connect_tls`, pool con TLS,
  `sslmode` disable/prefer/require); cadenas `postgresql://` y `clave=valor`; varios `host` y `port` separados
  por comas, sockets Unix (`host=/ruta`, `%2F`), usuario por defecto del proceso; cancelación (`cancel`) por
  conexión nueva con la clave del backend.
- Tipos: int2/4/8, float4/8, bool, text/varchar/bpchar/name, bytea, json/jsonb. `numeric`, `timestamptz`, arrays
  y `uuid` dan «unsupported PostgreSQL column type '…'», igual que la VM.
- Transacciones, migraciones, pool (`acquire` con plazo, `pool_stats`, `pool_health`, `pool_close`), límites y
  mensajes de error idénticos a la VM.
- **Verificación** contra un PostgreSQL 16.2 real: `tests/postgres/basico.titan`, `auth_pool.titan` y
  `socket_unix.titan` dan salida **idéntica** a `zett run`. TLS se probó con un proxy TLS y una CA de prueba
  (el PostgreSQL disponible no está compilado con SSL, y la VM no admite CA propias, así que esa parte no tiene
  comparación diferencial: solo se comprobó que conecta, consulta 5000 filas, cancela, valida el nombre y usa el
  pool); el programa de esa prueba no está en el repositorio porque exige un runtime con anclas propias.
- Las pruebas exigen un servidor en `127.0.0.1:55432` (y el socket en `/tmp`), con usuarios `md5user`,
  `clearuser`, `scramuser`; no se ejecutan en `verify_native.sh`.
- **Sin hacer:** SQLite (`Sqlite*`, requiere un motor SQL). Los handles `Db*` de SQLite no existen en el nativo.
  (MySQL se hizo después: ver «MySQL nativo».)

## MySQL nativo (2026-10-08)

`std_mysql.titan` (≈3 100 líneas) implementa el cliente del protocolo MySQL con las 15 instrucciones
`Mysql*`, y las 8 `Db*` genéricas ya reparten por la etiqueta del handle (PostgreSQL 26, MySQL 28). Es un
port de `crates/titan_mysql`, que usa el crate `mysql` 26.0.1 (`mysql_common` 0.35.5); el comportamiento se
copió de las fuentes de esos dos crates.

- **Protocolo:** saludo v10 y negociación de capacidades como `get_client_flags`; paquetes partidos en
  trozos de 16 MiB − 1; `COM_QUERY` (texto), `COM_STMT_PREPARE`/`EXECUTE`/`CLOSE`/`SEND_LONG_DATA`
  (los parámetros pasan a datos largos si la petición no cabe en un paquete) y filas binarias; `COM_QUIT`.
  Siempre sentencias preparadas con parámetros posicionales, y los `:nombre` se reescriben a `?` con el
  mismo analizador que el crate (comillas, comentarios, `MixedParams`). Caché LRU de sentencias
  (`stmt_cache_size`, 32 por defecto; 0 la desactiva) que cierra la menos usada al desbordarse.
- **Autenticación:** `mysql_native_password`; `caching_sha2_password` (camino rápido, contraseña
  cifrada con la clave pública RSA del servidor —OAEP con SHA-1— sobre TCP, y en claro por socket Unix);
  `mysql_clear_password` (solo con `enable_cleartext_plugin=true`); `mysql_old_password` (solo con
  `secure_auth=false`); cambio de plugin pedido por el servidor. `client_ed25519` da el mismo fallo que la VM
  (el crate está compilado sin esa opción).
- **URL:** esquema `mysql`, usuario/clave/base con percent-decode, y todas las claves de
  `Opts::from_url` con sus errores (`UnknownParameter`, `InvalidValue`, `pool_min`/`pool_max`);
  `prefer_socket` con host de loopback pregunta `@@socket` y reconecta por el socket Unix si existe (si
  falla se queda en TCP); `tcp_connect_timeout_ms`, `tcp_keepalive_*`, `tcp_user_timeout_ms`, `socket`,
  `max_allowed_packet` (si falta, `SELECT @@max_allowed_packet`). El análisis de la URL reutiliza el de
  `std_url.titan` (el de `url` 2.5.8).
- **Valores:** enteros con y sin signo (un `BIGINT UNSIGNED` que no cabe en i64 sale como string), `FLOAT`
  → float64, `DOUBLE`, `DATE`/`DATETIME`/`TIMESTAMP` como `AAAA-MM-DD HH:MM:SS.ffffff`, `TIME` como
  `D HH:MM:SS.ffffff`, el resto como string si es UTF-8 y como bytes si no. Las filas son mapas (si dos columnas
  se llaman igual gana la última, como el `BTreeMap` de la VM).
- **Resto:** transacciones, migraciones con `GET_LOCK('titan_migrations',30)` (el SQL de cada migración se
  manda en texto, como el crate, con lo que los errores de sentencias posteriores de un mismo texto se
  pierden igual que en la VM), pool (`acquire` con plazo, `pool_stats`, `pool_health`, `pool_close`), límites
  (256 handles, pool ≤ 64) y mensajes de error idénticos a la VM, incluidos los de red
  (`Could not connect to address …`, `server disconnected`, `Broken pipe`).
- **Diferencias conocidas:** (1) **no hay compresión**: `compress=...` se valida pero la conexión va sin
  comprimir (el servidor no comprime si el cliente no lo pide; ningún resultado cambia, solo el ancho de
  banda); (2) **no hay TLS**, igual que en la VM (`Opts::from_url` no puede activarlo); (3) `LOCAL INFILE`
  responde con un archivo vacío (la VM no instala manejador); (4) los atributos de conexión que ve el
  servidor son `_client_name=titan-mysql`, `_client_version`, `_os`, `_pid`, `program_name`; (5) un
  `SELECT @@socket` que devuelve NULL (la VM entra en pánico) se trata como «sin socket».
- **Verificación — leer con cuidado:** en este entorno no hay MySQL ni MariaDB y no se pueden descargar. Las
  pruebas son **diferenciales contra un servidor de protocolo de terceros**, no contra MySQL real:
  `tests/mysql/servidor.py` usa `mysql-mimic` 3.0.5 (otra implementación del protocolo, independiente de las
  dos que se comparan) con SQLite como motor SQL, ampliado con `caching_sha2_password` (RSA real),
  `mysql_old_password`, columnas de todos los tipos binarios, sockets Unix y cortes de conexión.
  `tests/mysql/comparar.sh` arranca un servidor nuevo para cada cliente y compara la salida de
  `zett run` (la VM de Rust, crate `mysql`) con la del nativo: `basico` (94 líneas), `conexion` (83), `sha2` (16),
  `pool` (50), `grande` (7: parámetro y fila de 17 MB), `errores` (20: cortes, cupo de handles) y `red` (5:
  plazo de conexión y esperas cooperativas) dan salida **idéntica**. Lo que SQLite no puede imitar no está
  probado contra ningún servidor: varios conjuntos de resultados (`CALL`, varias sentencias), `LOCAL INFILE`,
  `DECIMAL`/`BIT`/`GEOMETRY` reales, tipos temporales con fracción de MySQL 8 y el comportamiento exacto de
  un MySQL/MariaDB real ante `GET_LOCK`. Hasta probarlo contra un servidor real, la pieza se considera
  verificada solo en lo que cubre `mimic`.
- Para ejecutarlas: `pip install --target DIR mysql-mimic cryptography` (y `sqlglot`), y
  `PYLIB=DIR COMPILER=selfhost/titanc3 bash selfhost/tests/mysql/comparar.sh`. No corren en `verify_native.sh`.

**Punto fijo tras MySQL (2026-10-08):** `SEMILLA=selfhost/titanc6 bash selfhost/verify_fixpoint.sh` →
`titanc1 == titanc2 == titanc3`, SHA-256 de la última verificación: ver «std::tokenize en Titan».
La etapa 1 ya no la hace la VM de Rust: con el runtime actual la VM necesita más de los 4 GB de esta
máquina y el sistema la mata (`Killed`); la semilla es un compilador nativo ya construido (`titanc6`, hecho
por el `titanc3` anterior). Muestra del corpus nativo (68 programas, sin `redis`/`postgres`/`audio`/`gui`):
67 idénticos a la VM; el único distinto es `email_validation`, que ya lo era (el oráculo `zett` es anterior
a esa prueba).

**Punto fijo tras TLS y PostgreSQL (2026-10-08):** `selfhost/verify_fixpoint.sh` →
`titanc1 == titanc2 == titanc3`, SHA-256 `9f10e286dc044737634efaece2e1b8cc21ba1a8f3dc640819c92c472188c4d71`.
Regresión del corpus nativo (357 programas, `titanc8`, antes del último arreglo de TLS): 349 idénticos, 7 con
diferencia (`audio_player_backend`, `audio_tags`, `email_validation`, `gui_std`, `kv_interop`,
`kv_persistence`, `kv_symlink_canonical`: el oráculo `zett` disponible es anterior a esas pruebas o no está en
el PATH del subproceso), 1 no admitido (`diag_accept`, usa intrínsecos internos), 0 timeouts.

**`std::redis` junto a TCP (2026-10-08):** el estado de Redis usaba la misma posición de globals (3560) que la
tabla TCP: un programa que abría un socket TCP (también HTTP, WS, TLS o PostgreSQL) y luego una segunda
conexión Redis terminaba en segfault. Ahora cuelga de la cabecera de `tcp_st()` (+40). Además los errores de
conexión de Redis llevan el prefijo `Redis I/O error: ` como en la VM y los hosts IPv6 entre corchetes
(`redis://[::1]:…`) conectan. Pruebas: `selfhost/tests/redis_tcp/` (`redis_tcp.titan` exige
`TITAN_TEST_REDIS_URL` y `TITAN_TEST_REDIS_ADDR`; `redis_conexion.titan` no necesita servidor), idénticas a
`zett run`, y `tests/native/redis.titan` sigue idéntica contra un Redis 6.2.14 real con contraseña.

## Gestor de paquetes en Titan (2026-10-08)

**Zett (v1.2.0).** El gestor se llama `zett`: es el mismo ejecutable que `titan` invocado con ese nombre (el paquete trae el
enlace `zett → titan`; `selfhost/titan.titan`, `cl_zett`). Ofrece `add`, `fetch`/`install`, `update`, `keygen`, `pack`, `publish` y
`version`; `titan add` etc. siguen funcionando igual. Probado: `selfhost/tests/pkg/registro/probar.sh` (40/40, contra un registro HTTPS
local de prueba, con la CLI `titan`); con el ejecutable `zett` empaquetado se comprobaron a mano `version`, `help`, `add`, `keygen`,
`pack` (dos empaquetados idénticos byte a byte), los mensajes de uso con el nombre `zett` y que `titan run` funciona invocado solo por
el `PATH`. El registro público es ahora estático (`registro/` del repositorio, servido por GitHub; `selfhost/registro_agregar.titan` añade versiones; prueba local `selfhost/tests/pkg/registro/estatico.sh`); antes la dirección por defecto (`registry.titan-lang.org`) no existía. Sin probar en ARM64 real. Guía: `docs/ZETT.md`.

**ARM de 32 bits no soportado.** Un Termux de 32 bits (`armv7l`/`armv8l`, ABIs `armeabi-v7a`) no puede usar los paquetes: solo hay generador de código para x86-64 y ARM64. El Zett anterior (Rust, paquete `.deb` `arm` de la rama `zett-repo`) sí era de 32 bits y ya no se reconstruye. Caso real: teléfono con `abilist` = `armeabi-v7a,armeabi`.

**Corrección (v1.2.0):** `titan` calculaba la ruta de `native/` a partir de `argv[0]`; invocado solo por nombre (`titan run x.titan`
con el ejecutable en el `PATH`) fallaba con `std::path::canonical … No such file or directory`. Desde v1.2.0 usa `/proc/self/exe`.
**La release v1.1.0 tiene ese fallo**: con ella, `titan run` por el `PATH` no funciona (con la ruta completa sí).

Port completo de `crates/titan_pkg` y de los subcomandos `add`, `fetch`, `update`, `keygen`, `pack` y
`publish` de `titan_cli`. Piezas:

- `selfhost/semver.titan`: versiones y requisitos del crate `semver` 1.x (análisis con los mismos mensajes de
  error, precedencia, `matches`, reglas de prerelease; números u64 completos).
- `selfhost/pkg.titan` (~1 900 líneas): lector/escritor de `Titan.toml` con las reglas de cadenas de `toml_edit`
  (cuatro tipos de comillas), cliente del registro (`GET /v1/packages/<nombre>`, `POST …/versions` con
  `Authorization: Bearer`, solo HTTPS, redirecciones, límites), resolución con retroceso, `Titan.remote.lock`,
  caché `.titan/cache` e instalación en `.titan/packages`, tar.gz determinista (`pack`) y extracción segura
  (rutas, enlaces, duplicados, 32 MiB por archivo, 128 MiB y 10 000 archivos en total, formatos GNU/ustar/pax),
  verificación SHA-256 y firma Ed25519 de cada paquete.
- Nativas nuevas en el runtime (solo Titan, sin equivalente en la VM): `std::crypto::ed25519_public_key`,
  `ed25519_sign`, `ed25519_verify` (`native/std_ed25519.titan`, RFC 8032 + OpenSSL), `std::fs::is_symlink` y
  `write_private` (`native/std_fs_extra.titan`; ver `natives_titan.txt`).
- `titan.titan` reparte los comandos a `pk_run`.

Verificación (`selfhost/tests/pkg/`; todo repetible):
- `paquetes.py CLI`: compara la CLI con `paquetes_esperado.json`, que es la salida de `zett` sobre exactamente los
  mismos casos: 1 638 de `add` (1 327 requisitos semver generados al azar, manifiestos, 300 cadenas para las
  reglas de comillas TOML), 59 de extracción `fetch --offline` (formatos, rutas peligrosas, enlaces, truncados,
  límites exactos) y 31 de `keygen`/`pack`/ayuda/errores (el `.tpkg` coincide en entradas, modos, contenido
  y mtime/uid/gid; la firma se comprueba con OpenSSL). Regenerar: `paquetes.py --generar ORACULO`.
- `probar_semver.sh`: 96 comprobaciones de `matches` y orden (ejemplos de la documentación del crate y de
  semver.org; esas no vienen del oráculo, que no expone `matches`, sino de las reglas publicadas).
- `registro/probar.sh`: 40 comprobaciones de red contra un registro HTTPS local (`registry.py`) con CA propia:
  resolución con retroceso, prerelease, descarga, caché, `--offline`, `update`, 11 errores de registro y 11 de
  `publish`. El cliente TLS nativo solo confía en los roots embebidos, así que el script compila una CLI de
  prueba con una copia de `native/` cuyo `std_roots.titan` contiene solo la CA de prueba.
- El test con casos guardados encontró y arregló un fallo real: tras una coma, un comodín seguido de otra cosa
  (`6,x5496`) da el error normal de «carácter inesperado», no el de «comodín único».

Regresión: punto fijo ✅ (`titanc1 == titanc2 == titanc3`, SHA-256 `2dd9d0c3e043314cf807dbe8a2d7b712870d473eba4c1c1583a58b63fbab00bb`;
el compilador no importa el gestor, así que no cambia) y 26 programas del corpus nativo sin diferencias nuevas
(`gui_std` ya difería por el oráculo antiguo; `redis.titan` exige servidor).

Diferencias conocidas (no ocultas):
1. Con manifiestos TOML inválidos 4 diagnósticos difieren del crate `toml` (el texto con línea/columna).
2. El registro público (GitHub) no es accesible desde el sandbox: las pruebas de red locales usan un servidor HTTPS propio; la lectura real desde GitHub la comprueba el CI. Para el cliente Rust también valen solo los roots embebidos.
3. Los `/tmp/titan-publish-*.tpkg` temporales quedan tras un error de `publish`, igual que en Rust.

## LSP en Titan (2026-10-08)

`selfhost/lsp.titan` reemplaza el crate `titan_lsp` (916 líneas de Rust) y se usa con `titan lsp` (el CLI
`titan.titan` lo importa). Habla LSP por stdin/stdout con cabeceras `Content-Length` (máximo 16 MiB).
Para eso hay dos nativas nuevas, solo-Titan (sin equivalente en la VM de Rust): `std::io::read_stdin(n)` y
`std::io::write_stdout(bytes)` (`native/std_fs_extra.titan`). La tabla pasó a **823** firmas, así que hubo que
recompilar el compilador (`build.titan`) antes de usarlas.

Cubre: `initialize` (capacidades), `didOpen/didChange/didClose` (cambios incrementales y totales, posiciones
en UTF-16), diagnósticos (léxico, parseo, tipos, duplicados), símbolos de documento y de espacio de trabajo,
definición, referencias, hover, `rename` (también entre documentos), tokens semánticos, `signatureHelp` y
completado (20 palabras + símbolos + las 823 nativas).

Pruebas (`selfhost/tests/lsp/`):
- `probar.py`: **109 comprobaciones, 0 fallos** (~1 min por el documento de 40 000 funciones). Los esperados
  salen de los 7 tests de Rust portados y de leer `crates/titan_lsp`; no existe un oráculo ejecutable del LSP
  (el `zett` v1.0.0 no tiene `lsp`).
- `contraste_zett.py`: compara los diagnósticos (mensaje y línea:columna) con `zett check` sobre **466 archivos
  `.titan`** del repositorio (24 con errores): **466 idénticos**. Los archivos con `import` o `mod` se omiten
  (`check` carga el proyecto; el LSP revisa el documento solo) y los de `tests/pkg` también (usan Ed25519, que
  `zett` no conoce).
- Rendimiento medido: 1000 funciones → 0,36 s al abrir; 5000 → 1,9 s.

Diferencias conocidas (no ocultas):
1. `rename` responde `newText` (el campo correcto de LSP); el Rust escribía `new_text`.
2. Método desconocido → error -32602 «method not found», como el Rust.
3. El hover de nativas (`std::x::f`) no se puede alcanzar en el Rust (la palabra se corta en `:`), así que no se portó.
4. `signatureHelp` de funciones propias da `nombre(…)` sin parámetros, igual que el Rust.

Regresión: punto fijo ✅ (`titanc1 == titanc2 == titanc3`, SHA-256
`fa57a027…` al cerrar el LSP).

## Depurador, DAP y REPL en Titan (2026-10-08)

Los tres sustituyen a `cmd_debug`/`cmd_repl` de `titan_cli`, a `titan_dap` y a la parte de
`titan_vm` que los soportaba (`src/debug.rs`).

**Cómo se depura sin VM.** `titan debug`/`titan dap` compilan el programa con un *gancho* antes
de cada instrucción de bytecode de las funciones del usuario (`bk_build_with` en
`native/backend.titan`). Como la pila de operandos del bytecode es la pila de la máquina y las
variables locales están en el marco (`[rbp-16(i+1)]`), el gancho (`native/std_debug.titan`) lee de
ahí exactamente lo que la VM de Rust entrega en `DebugFrame`: función, instrucción, profundidad,
posición, locales y pila. Las decisiones de parada (puntos de interrupción por línea o por
instrucción, paso adentro/sobre/afuera) son las de `Debugger`. La tabla de funciones y posiciones
la genera `debug_table.titan`; el proceso compilador se reemplaza por el ejecutable de depuración
con `std::process::exec_once` (execve + borrado del archivo, sin dejar temporales).

- **`titan debug [-b ruta:línea]… <entrada>`**: depurador interactivo (`c s n o p q`), mismo texto
  que `zett debug`. Probado con `tests/debug/probar.sh`: 40 guiones (pasos ×600, `next` ×200,
  paso+`p`, mezclas, salir, puntos de interrupción `-b`/`-bX`/`--breakpoints`, errores de línea
  de comandos) sobre 6 programas, **salida, stderr y código idénticos a `zett debug`** (40/0).
  Desviación: con la entrada agotada antes de terminar el programa `zett debug` repite «unknown
  debugger command» sin fin (consume CPU y disco); aquí se trata como `q`.
- **`titan dap`** (equivale al binario `titan-dap`): servidor DAP por stdin/stdout.
  Antes de `configurationDone` lo atiende `dap.titan` (initialize, launch con `.tbc` o proyecto,
  setBreakpoints, threads, scopes…); en `configurationDone` compila el ejecutable de depuración y
  se reemplaza por él: desde ahí `native/std_dap.titan` atiende el protocolo con el programa en
  marcha (revisa la entrada con `poll` sin bloquear cada 32 instrucciones, emite `stopped`,
  `continued`, `output`, `terminated`). La salida del programa (`print`, `write_stdout`) va a una
  tubería (fd 1 redirigido) y sale como eventos `output` línea a línea; los mensajes DAP salen
  por una copia del stdout original. Respuestas, eventos, claves y errores son los de
  `crates/titan_dap/src/lib.rs` (`native/dap_core.titan` tiene lo común).
  Probado con `tests/dap/probar.py` (20 casos, cliente real por tuberías): handshake y números de
  secuencia, errores de cada petición, sesión completa (parada, `stackTrace`, `variables`,
  `stepIn`/`stepOut`/`next`/`continue`, `terminated`), error del programa (evento stderr),
  `terminate`/`disconnect` parado, `pause` con el programa corriendo, peticiones pipelinadas,
  300 líneas de salida, `.tbc`, proyecto sin escribir `titan.lock`, errores de marco/JSON
  (`titan-dap: …`, código 1) antes y después de arrancar.
  **El oráculo `zett` v1.0.0 no trae `dap`**, así que las expectativas salen de leer el código de
  `titan_dap`, no de ejecutarlo.
  Diferencias conocidas:
  - `launch` con `sandbox: true` → error (los ejecutables nativos no se sandboxean).
  - `launch` una vez arrancado el programa → error (en Rust recompilaría y relanzaría).
  - Con el programa parado y la entrada cerrada, sale con 0 (Rust se queda esperando para siempre).
  - Los mensajes de «invalid DAP JSON» llevan el texto del analizador de Titan, no el de serde.
  - Un único `print` de más de 1 MiB entre dos revisiones bloquearía hasta que el cliente lea
    (tubería de 1 MiB); en Rust el canal es ilimitado.
  - Salida por `write_stdout`/`/dev/stdout` del programa llega como evento `output`; en Rust
    corrompería el flujo DAP.
  - Primera compilación de un programa a depurar: ~1 min (se llena la caché del runtime); después ~8 s.
- **`titan repl`**: idéntico a `zett repl` en 11 sesiones (ver `titan.titan`, `cl_repl`).

Nativas nuevas (solo Titan, no existen en la VM): `std::process::exec_once` (824 en total).

Regresión: punto fijo ✅ (`titanc1 == titanc2 == titanc3`, SHA-256
`824bcfdc70e16bbb9b048070c88e65d29e915a04cef7aa3d33daecc5c3dbe03f`; cambió la tabla de nativas y el
runtime). Muestra del corpus nativo (1 de cada 6, 59 programas menos `redis`/`postgres`, que piden
servidor): idénticos a la VM salvo `audio_player_backend` y `gui_std`, que fallan del lado de la VM
(sin `zett` en el PATH del subproceso y sin pantalla), no del nativo.

## Tabla de avance hacia el self-hosting total (2026-10-08)

| Pieza | Estado |
|---|---|
| Compilador (lexer, parser, tipos, bytecode, nativo, LLVM, WASM) | ✅ punto fijo |
| Biblioteca estándar nativa | **812/812 firmas** (medido con `bash selfhost/native/cobertura.sh -v`, 2026-10-08; el script necesita `bash`). `std::web` (53) y `std::wasm` (14) existen en el nativo pero dan el mismo error de ejecución que la VM (ver «std::web y std::wasm en el nativo») |
| Red: TCP/HTTP/WS/TLS/servidor, Postgres, Redis | ✅ |
| **Gestor de paquetes** | ✅ (esta sección) |
| **LSP** (`titan_lsp`, 916 líneas Rust) | ✅ `titan lsp` (ver «LSP en Titan») |
| **DAP, `debug`, `repl`** (`titan_dap`, parte de `titan_cli`) | ✅ `titan dap`, `titan debug`, `titan repl` (ver «Depurador, DAP y REPL en Titan») |
| MySQL | ✅ `std_mysql.titan` (ver «MySQL nativo»; verificado contra un servidor de protocolo de pruebas, **no** contra MySQL ni MariaDB reales) |
| SQLite | ✅ ver «SQLite en Titan» |
| Audio | ✅ ver «Audio: decodificador y engine en Titan» |
| **tokenize** (10 nativas) | ✅ ver «std::tokenize en Titan» (sin `Precompiled`, `UnicodeScripts` ni BPE con dropout) |
| Borrar el Rust (`titan_vm`, `titan_stdlib`, …) | ✅ ver «Rust borrado» (se arranca desde una semilla de 650 KB) |
| Página oficial con GitHub Pages | ✅ hecha (`site/` + `.github/workflows/pages.yml`); se publica en `https://alexsndersoto04-source.github.io/aio/` al llegar a `main` (Pages ya está activado con «GitHub Actions»); **aún no se ha desplegado** |


## Audio: decodificador y engine en Titan

**Dónde está.** `selfhost/native/std_audio_decode.titan` (WAV/PCM/ADPCM/FLAC),
`std_audio_ogg.titan` (contenedor Ogg: páginas, CRC32, flujos por serie),
`std_audio_vorbis.titan` (decodificador Vorbis completo) y
`std_audio_engine.titan` (las 22 funciones `engine_*`).

**Cómo se verificó.**
- `selfhost/tests/audio/decode_all.titan` decodifica 36 ficheros de
  `gen_fixtures.py` y su salida es idéntica (`cmp`) al oráculo generado con la VM
  de Rust (symphonia). Vorbis comparado muestra a muestra con libvorbis: ≤ 3e-7.
- `engine_flow.titan`: misma salida que `zett run` salvo las líneas que dependen
  del hardware (nombre del dispositivo ALSA).
- DSP (volumen, remuestreo 22050→48000, EQ de 3 bandas) comparado con una
  referencia numpy independiente (`engine_sink.titan` + `TITAN_AUDIO_SINK=file:`).

**Limitaciones declaradas (no se ocultan).**
- Sin MP3, Opus, AAC, AIFF ni CAF (tampoco los tiene el oráculo Rust).
- Floor0 de Vorbis está portado pero **sin verificar** (no hay ficheros de prueba).
- La salida a dispositivo es un pipe de PCM f32 a `pw-cat`/`paplay`/`aplay`/`mpv`/
  `ffplay`; **no se ha probado** (el sandbox no tiene tarjeta de sonido). La salida
  `TITAN_AUDIO_SINK=file:<ruta>` sí está probada.
- `seek` reabre el fichero y descarta paquetes: lento en ficheros largos.
- Ogg-FLAC y MKV: no soportados todavía (se tratan como no reconocidos).
- El estado de audio (simulador, reproductor, engine) vive en el bloque `rtm_state()`
  del runtime (+16, +24, +32), no en las globales de 4096 bytes (estaban llenas y
  chocaban con ONNX).
- Los hilos nativos son fibras cooperativas: el trabajador del engine avanza en
  cada llamada a `engine_*`.
- `cloud_*` (12 funciones, Telegram/MTProto) **eliminado** del registro de nativas
  (`gen_natives.sh` lo filtra): no se puede implementar ni verificar sin servicio real.

**Semilla.** `titanc0` es una semilla «lean» (runtime sin MySQL) generada con
`bootstrap_semilla.sh` para máquinas de 4 GB; punto fijo `titanc1==2==3`,
SHA-256 de la última verificación: ver «std::tokenize en Titan».


## std::tokenize en Titan

**Dónde está.** `selfhost/native/std_tokenize_{tab,uni,rx,eng,mod,dec,unigram,load}.titan`
(motor) y `std_tokenize.titan` (las 10 nativas: `load`, `from_json`, `close`,
`vocab_size`, `encode`, `encode_padded`, `encode_batch`, `decode`, `token_to_id`,
`id_to_token`). Es una traducción del crate `tokenizers` 0.22.2 (el mismo que usa
el Rust de `tokenize_mod.rs`): normalizadores (NFC/NFD/NFKC/NFKD con las tablas
Unicode 9 de `unicode-normalization-alignments`, Lowercase, Strip, StripAccents,
BertNormalizer, Replace, Prepend, Nmt, ByteLevel, Sequence), pre-tokenizadores
(Whitespace, WhitespaceSplit, BertPreTokenizer, ByteLevel, Metaspace, Punctuation,
Digits, Split, CharDelimiterSplit, FixedLength, Sequence), modelos WordLevel, WordPiece, BPE y
Unigram, post-procesadores (Bert, Roberta, ByteLevel, Template, Sequence),
decoders (WordPiece, ByteLevel, Metaspace, BPEDecoder, CTC, Fuse, Replace,
ByteFallback, Strip, Sequence), tokens añadidos, truncado y relleno. El motor de
regex del pre-tokenizador (`std_tokenize_rx.titan`) y las tablas
(`std_tokenize_tab.titan`, generadas por `native/tokenize/gen_tab.py` a partir de
`regex-syntax` 0.8.11, `unicode_categories` y `unicode-normalization-alignments`)
están incluidos; las fuentes de esos crates están en `selfhost/fuentes/`.

**Cómo se verificó.**
- `selfhost/tests/native/tokenize_dev/` (`gen_cfgs.py`, `build_cmp.sh`,
  `cmp_main.titan`): 64 tokenizers generados con `tokenizers` 0.22.2 de Python ×
  262 textos = **40 960 casos, 0 diferencias** contra `std::tokenize` de la VM de Rust
  (ids, tokens, type_ids, máscaras).
- `tests/native/tokenize_std.titan` (BERT, BPE de bytes y Unigram; encode,
  encode_padded, encode_batch, decode, vocabulario, `load` de archivo) y
  `tokenize_errores.titan` (límites, handles, UTF-8, errores): compilados con
  `titanc3` e **idénticos** a la VM con `verify_native.sh`.

**Rendimiento.** El runtime se compila sin «moves» (su código maneja referencias a
mano). El motor no usa `std::raw`, y sin moves cada `push`/`set` sobre un array que
pasa de función en función copiaba el array entero: 200 s para 60 KB. `opt.titan`
(`ot_moves_for`) activa los moves solo para las funciones de `std_tokenize_*.titan`;
ahora 60 KB tardan ≈0,6–1,4 s (BERT/GPT-2/Unigram/Llama). La VM de Rust es más rápida
(milisegundos); `tokenize_dev/perf_dev.titan` mide esto.

**Limitaciones declaradas (no se ocultan).**
- `Precompiled` (normalizador de T5, ALBERT, XLM-R), `UnicodeScripts` y BPE con
  `dropout > 0` se rechazan al cargar con «unsupported …» (Rust los acepta).
  `Precompiled` exige segmentación de grafemas Unicode y el trie de `spm_precompiled`.
- Un JSON mal formado o con tipos erróneos da un mensaje propio, no el de serde
  (sin «at line N column M»).
- Stride ≥ max_length con truncado real hace *panic* en Rust; aquí es un error.
  `TemplateProcessing` con piezas inexistentes y el decoder `Strip` con tokens cortos
  también hacen *panic* en Rust; aquí se rechazan/acotan.
- No hay el límite de 2 operaciones concurrentes por runtime.
- BPE en palabras enormes puede ser O(n²).

**Punto fijo** (con todo lo anterior, incl. audio y tokenize):
`titanc1 == titanc2 == titanc3`, SHA-256
`cb652b3ce760f068faba3f131cd0657f819a1b376316bf308f87b168994ffe67`.


## SQLite en Titan

**Dónde está.** `selfhost/native/std_sqlite_*.titan` (analizador SQL, planificador/evaluador, almacenamiento
en páginas con formato de archivo propio, fechas, JSON, funciones de ventana) y `std_sqlite.titan` (las 16
instrucciones `Sqlite*` y los handles `Db*`). Ni libsqlite3 ni ningún crate: un motor SQL escrito en Titan.

**Cómo se verificó.**
- Diferencial con el oráculo (la VM de Rust con SQLite 3.46.0 integrado): `selfhost/tests/native/sqlite_dev/`
  (cientos de consultas: DDL, DML, joins, subconsultas, CTE recursivas, ventanas con marcos/EXCLUDE/FILTER,
  JSON/JSON5, fechas, triggers, vistas, índices, `pragma`, migraciones).
- `sqlite_api.titan` y `sqlite_conc.titan` (API y concurrencia) idénticos entre la VM y el ejecutable nativo.

**Limitaciones declaradas (no se ocultan).**
- Sin FTS, RTREE, `ATTACH`, `EXPLAIN` ni `jsonb`; sin `WITHOUT ROWID`, `STRICT`, columnas generadas ni tablas
  temporales; `UPDATE … FROM` no soportado; `RIGHT`/`FULL JOIN` con `json_each` no soportados.
- Diario `delete`, pero los lectores no bloquean: se emula con candados OFD de Linux (**kernel ≥ 3.15**).
  Sin recuperación de WAL.
- `localtime` = UTC; fechas inválidas con algunos quirks; `-0.0` se guarda como `0.0`; `VACUUM`/`ANALYZE` no hacen
  nada; las claves foráneas diferidas se comprueban como inmediatas.
- Sin `ORDER BY` el orden de filas puede diferir del de SQLite real.
- Un caso de mensaje distinto: `insert into c values (?, ?)` con 1 parámetro da «Wrong number of parameters»
  en lugar de «table c has 1 columns but 2 values were supplied».

## std::web y std::wasm en el nativo

Las 53 + 14 firmas están registradas y compiladas en el runtime nativo, pero **producen el mismo error de
ejecución que la VM de Rust** en un entorno sin host JS (no hay navegador ni Node en el sandbox). La emisión de
Wasm real a partir de Titan es la Fase W (`wasm_diff/`); `std::web` con un host JS real no se ha probado aquí.

## std::onnx: BERT

**Dónde está.** `selfhost/native/std_onnx_ops.titan` (operadores nuevos), parches en `std_onnx_engine.titan`
(BOOL, Split, formas fijadas) y `std_onnx.titan` (`load_bert`, `load_bert3`, `run_bert`, `run_bert3`,
`run_bert_pooled`, y `ox_mm_kernel`, el núcleo de MatMul en f64 crudo con `std::raw`).

**Qué hace de verdad.** Además de los 39 operadores anteriores: Where, And/Or/Not/Equal/Less/Greater…,
Expand, Slice, Range, GatherND, Split, Min/Max, Erf, Gelu, LayerNormalization, Pow y reducciones adicionales,
dtypes FLOAT, INT32, INT64 y BOOL. `load_bert(path, batch, seq)` fija las entradas 0 y 1 a i64 `[batch, seq]`
(`load_bert3` también la 2); `run_bert`/`run_bert3` devuelven `{values, shape}` de la primera salida y
`run_bert_pooled` la media enmascarada `[batch, hidden]`.

**Cómo se verificó.**
- `bash selfhost/tests/onnx/run.sh` (con `COMPILER=selfhost/titanc3` si no existe `selfhost/titan`): 10 casos
  `op_*` nuevos (opset 17/20) además de los 25 anteriores, cada uno con y sin `load_shape`: error máximo
  3e-07 contra onnxruntime y contra tract (tract no ejecuta `op_erf_gelu`; ahí solo onnxruntime). `ONNX: TODO OK`.
- `bash selfhost/tests/onnx/run_bert.sh` (necesita torch + transformers + onnx + onnxruntime + onnxscript en
  `OXPY`): genera 4 BERT reales de HuggingFace (legado 2 y 3 entradas con ejes dinámicos, dynamo, hidden 128 con 3
  capas); error máximo 1.2e-06 contra onnxruntime (y 7.2e-07 contra tract en el único que tract carga);
  `run_bert_pooled` comparado con la media enmascarada de la referencia; 10 casos de error de la API idénticos a tract
  (los textos que dependen del grafo se enmascaran).
- Un modelo de 4 capas, hidden 256, 3,2 M de parámetros y seq 16 (9,8 MB) coincide con tract en ≤ 1.4e-06.

**Rendimiento.** ≈3 s por pasada con ese modelo de 3,2 M de parámetros (leer y analizar 9,8 MB: ≈1,5 s). Antes del
kernel de MatMul eran ≈8 s. MatMul ≈17 ns por multiplicación-suma; Erf ≈70 ms por 16 384 elementos.

**Limitaciones declaradas.**
- Solo modelos pequeños: archivo ≤ 64 MiB, ≤ 4 194 304 elementos por tensor, rango ≤ 8, ≤ 4 handles. Verificado con
  hidden 64–256. Un BERT-base de verdad (≈440 MB) **no** carga ni cabe.
- Precisión ≈1e-6 absoluto (acumula en f64 y redondea a f32 por operador; tract acumula en f32).
- Los errores de modelos mal formados no coinciden con los de tract; tract se cae con *panic* en dos casos que aquí
  son errores normales (`load_bert` con seq incompatible y `load_bert3` en un modelo de 2 entradas).
- Máscaras con `-inf` rechazadas (HuggingFace usa `finfo.min`). El nativo carga modelos dynamo (GatherND, Gelu) que
  tract rechaza.
- Gemm no usa el kernel rápido (solo MatMul).
- `opt.titan` activa los moves también para `std_onnx_engine`/`std_onnx_ops`. Punto fijo tras ese cambio:
  `titanc1 == titanc2 == titanc3`, SHA-256 `f4943da32549139149a6e8765887c42c3610da9073eeb4b7b6364c781492bc3d`.


## Rust borrado

**Qué se borró.** `crates/` (19 crates, 114 archivos `.rs`, ≈70 000 líneas), `Cargo.toml`, `Cargo.lock`,
`rust-toolchain.toml`, `.cargo/`, `make-zett-package.sh`, `verify_moon.py`, `verify_phase34.py`, los scripts de
Android/Termux que compilaban Rust, y los flujos de GitHub Actions que usaban `cargo` (cross-platform, termux-*,
update-cargo-lock, publish-linux-binary, ci-diag-annotations, diag-titan). `selfhost/gen_natives.sh` y
`gen_codegen_tables.py` (leían el registro/código de Rust) también: `natives.titan` y `codegen_tables.titan` son
ahora la fuente y se editan a mano. El Rust sigue en el historial: etiqueta local **`ultimo-con-rust`**
(commit `5dbb238`).

**Cómo se arranca ahora.** `bash selfhost/bootstrap.sh`:
1. desempaqueta la **semilla** (`selfhost/semilla/titanc-linux-x86_64.gz`, 650 KB; 7,7 MB descomprimida; SHA-256
   `f4943da3…bc3d`) — el compilador nativo x86-64 construido con la última VM de Rust;
2. `selfhost/verify_fixpoint.sh`: semilla → titanc1 → titanc2 → titanc3, sin `PATH`, idénticos byte a byte;
3. compila la CLI `selfhost/titan` con el resultado.
Verificado desde cero con `PATH=/usr/bin:/bin` (sin `zett`): hash `f4943da32549139149a6e8765887c42c3610da9073eeb4b7b6364c781492bc3d`
en las tres etapas, ≈3 min. Ver `selfhost/semilla/LEEME.md`.

**Última pasada con la VM de Rust antes de borrarla** (VM de la rama `binaries`, de 2026-10-04, algo anterior a
la del commit): muestra de 59 programas de `selfhost/tests/native` (1 de cada 6, sin redis/postgres/mysql ni los
que dependen de pantalla/audio): **59 idénticos, 0 distintos**. `titan check projects/moon/src/main.titan`:
17 archivos, 308 funciones, OK. Cobertura medida: 812/812.

**Lo que se pierde, dicho claro.**
- **Oráculo de pruebas:** la VM de Rust ya no está. Las pruebas diferenciales (`verify_*.sh`, `native/verify_native.sh`,
  `tests/**`) quedan como registro; para repetirlas hay que construir la etiqueta `ultimo-con-rust` (ver
  `selfhost/tests/LEEME.md`). Los tests ONNX comparan además con onnxruntime, que sí sigue sirviendo.
- **Plataformas:** hoy hay paquetes para Linux x86-64 y Linux ARM64 (Termux); macOS y Windows no tienen paquete nuevo
  (las releases hasta v1.0.26 son de la versión en Rust; desde la v1.1.0 se construyen sin Rust). Ver «Linux x86-64 y ARM64/Termux» más abajo.
- **Flujos de GitHub Actions**: `ci.yml` (bootstrap sin Rust) y `arm64.yml` se ejecutaron en GitHub y pasan (ver más abajo);
  `check-moon.yml` y `pages.yml` aún no se han ejecutado allí.
- `zett run` como intérprete: `titan run` compila a nativo y ejecuta (no hay VM).
- Los textos de arriba de este documento que dicen «la VM de Rust lo sigue usando» describen la situación anterior.


## Linux x86-64 y ARM64/Termux (2026-10-09)

**Qué hay.** Dos paquetes, `titan-v1.2.0-linux-x86_64.tar.gz` y `titan-v1.2.0-linux-aarch64.tar.gz` (los fabrica
`selfhost/empaquetar.sh`; cada uno lleva el ejecutable `titan`, la carpeta `native/` con el runtime en Titan y un
`LEEME.txt`; ≈2,7 MB). Para Termux, `selfhost/instalar-termux.sh`. Los fabrica el flujo `.github/workflows/arm64.yml` y
quedan como artefacto `paquetes` de cada ejecución; el paso de publicación como release solo corre en etiquetas `v*` y
**todavía no se ha ejecutado**.

**Cómo se hace el ejecutable ARM64.** El compilador x86-64 construido por el punto fijo compila la CLI (`selfhost/titan.titan`) con
el backend LLVM: `titan compile selfhost/titan.titan -o titan-aarch64 --target aarch64` (LLVM lo pone `clang`; la CLI
no incluye LLVM, lo invoca). El resultado es un ELF estático ARM64 sin libc: habla con el núcleo Linux por llamadas al
sistema, igual que en x86-64. En ARM64, `titan run/compile` genera LLVM IR y llama a `clang` (con `lld`); por eso en Termux
hace falta `pkg install clang`. En una máquina x86-64, `--target aarch64` compila para ARM64 (también por `clang` o por `zig cc`
como alternativa vía `TITAN_CC`).

**Qué se verificó, en un ARM64 real** (runner `ubuntu-24.04-arm` de GitHub Actions; ejecución `37890535640`, commit `7deb0fc`):
- el ejecutable ARM64 arranca y `titan version` responde;
- compila y ejecuta un programa en la propia máquina ARM64;
- procesos hijos (`std::process::spawn`) y servidor HTTP (`std::server`) con un cliente `curl` real;
- **diferencial: 95 programas de `selfhost/tests/native` compilados y ejecutados con el backend x86-64 y con LLVM/ARM64: 95 idénticos
  (stdout, stderr y código de salida), 0 distintos, 0 no admitidos.** La muestra es 1 de cada 4 de las pruebas que no necesitan red
  externa, pantalla ni audio, más todas las de tareas/canales, `coleccion_llamadas`, `server_std` y `window_state`;
- el propio `titan` ARM64 compila `selfhost/titan.titan` en ARM64 (2 min 17 s). Ese ejecutable **no** es idéntico byte a byte al
  cruzado (clang distinto en cada caso): no se afirma punto fijo en ARM64.

**Fallos reales encontrados y corregidos al hacerlo.**
- `std::server` guardaba su puntero de estado en `globals+3592`, que en ARM64 es la zona de trabajo de `rt_sys_arm64`;
  `ppoll` escribe ahí el tiempo restante y corrompía el puntero (SIGSEGV al recibir la primera petición). Ahora el estado vive en
  `rtm_state()+64`. `std::window` (Wayland) guardaba el suyo en `globals+3552`, el mismo hueco de las tareas; ahora en `rtm_state()+72`.
- El backend LLVM no admitía `map(xs, f)`/`filter`/`fold`/`find`/`any`/`all` como llamadas, ni tareas ni canales: ahora sí.
- `ci.yml` tenía un error de YAML que lo hacía fallar en 0 s; corregido.

**Límites que quedan, dichos claro.**
- No está probado en un teléfono: el runner ARM64 es un servidor Linux, no Android. Termux usa el mismo núcleo, pero Android puede
  filtrar algunas llamadas al sistema; si algo falla en un dispositivo, hay que mirarlo ahí.
- En ARM64 hace falta `clang` + `lld` (Termux: `pkg install clang`) para compilar programas; el paquete no lo trae.
- `titan debug`/DAP: su estado vivía en `globals+3600…3728`, que en ARM64 es zona de trabajo del runtime. Se movió a un bloque propio (`rtm_state()+80`, `dbg_g()` en `std_debug.titan`). En x86-64: `selfhost/tests/debug/probar.sh` 40/40 con salida idéntica a la versión anterior y `selfhost/tests/dap/probar.py` 20/20. **Sigue sin probarse en ARM64.**
- `std_audio_engine`, `gui`/ventanas, `fswatch`, Redis/Postgres/MySQL, TLS y `ws_*` no están en el diferencial (necesitan servicios o
  dispositivos externos): se compilan por LLVM, pero su paridad en ARM64 no está medida.
- macOS y Windows: sin paquete (el runtime usa llamadas al sistema de Linux).

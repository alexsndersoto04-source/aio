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
| 4a | **Cargador de `import`** y **generador de bytecode** en Titan (`selfhost/loader.titan`, `selfhost/codegen.titan`) | ✅ idéntico al de Rust |
| 4b | bytecode → **ejecutable nativo** x86-64 + runtime en Titan (sin VM en Rust) (`selfhost/build.titan`, `selfhost/native/`) | ✅ funciona (con conteo de referencias y floats) |
| 5 | Titan se compila a sí mismo (punto fijo: etapa1 == etapa2 byte a byte) | ✅ **logrado** (`selfhost/verify_fixpoint.sh`) |
| 6 | Biblioteca estándar y runtime en Titan; borrar el último `.rs` | en curso: 473 / 816 nativas (`selfhost/native/cobertura.sh`) |
| L | **Backend LLVM** en Titan: bytecode → LLVM IR → clang/llc (LLVM real) (`selfhost/native/llvm.titan`, `selfhost/build_llvm.titan`) | ✅ x86-64: 241 / 241 idénticos a la VM con -O2 (LLVM 22 local y clang 18 en la CI) · ✅ punto fijo por LLVM (`native/llvm/punto_fijo.sh`; el compilador hecho por LLVM es ~5× más rápido) · ✅ ARM64 (AArch64): 241 / 241 idénticos en una máquina ARM64 real (CI `ubuntu-24.04-arm`, contra la VM de Rust compilada para ARM64) |
| O | **Optimizador propio** en Titan, estilo `opt` de LLVM, sobre el bytecode (la representación intermedia de Titan) (`selfhost/opt.titan`): cálculo de constantes, saltos encadenados, valores descartados, código inalcanzable. Lo usan los dos backends | ✅ en marcha: el compilador pasa de 74 550 a 69 358 instrucciones (−7 %); punto fijo propio y por LLVM ✅; `selfhost/opt_ver.titan ARCHIVO [--dump]` muestra el antes/después; `TITAN_OPT=0` lo apaga. Integración de funciones pequeñas (inlining) hecha y probada, cuenta la profundidad de llamadas igual que una llamada real; va apagada salvo con `TITAN_INLINE=1` porque medida sobre el compilador no lo acelera (ni con el backend propio ni con LLVM) y agranda el ejecutable |
| M | **Memoria del runtime**: medido con un perfilador por muestreo propio (`selfhost/native/perfil.py`, sin perf), el compilador pasaba casi la mitad del tiempo pidiendo y devolviendo bloques grandes al sistema. Ahora esos bloques se reutilizan (listas por tamaño, potencias de 2) | ✅ compilador hecho por LLVM: 6,5 s → 4,3 s (−34 %); con el backend propio: 41,4 s → 31,0 s (−25 %); misma salida byte a byte; puntos fijos ✅; prueba `memoria_grande.titan` |
| U | **Última lectura (moves)** en el optimizador: análisis de vida de las variables; la última lectura entrega el valor sin copiar (`TakeLocal`), así arrays/mapas/textos quedan con un solo dueño y se amplían en el sitio. Se aplica a los programas, no al runtime (que guarda direcciones en enteros). El perfilador ahora sube por la cadena de llamadas (`--llamador`, `--solo`) y el backend propio escribe en el mapa sus rutinas internas. Con eso se arreglaron las copias enteras en `pc_add`/`pc_ref`/`pc_label`, `cg_emit`/`cg_patch`/`cg_intern` y el optimizador; y el backend propio compara en línea la longitud de dos strings | ✅ compilador con el backend propio: 42 s → 26 s (−38 %); con LLVM: 4,3 s → 4,0 s; mismo binario byte a byte; bytecode idéntico al de Rust en 771/771 archivos; puntos fijos ✅; prueba `ultima_lectura.titan` |
| S | **Sin sistema operativo** (objetivo `aarch64-none`): el programa arranca solo en una máquina ARM64 (la "virt" de QEMU): `_start` de `std::freestanding`, coma flotante, **MMU con tabla de páginas de verdad** (bloques de 1 GiB: dispositivos sin ejecutar y RAM normal con caché), puerto serie PL011 para la salida y la entrada, memoria propia desde el final del programa, reloj con el contador del procesador y apagado por PSCI (con el código de salida). Llamadas sin equivalente sin sistema operativo (archivos, procesos, red, azar): -ENOSYS de verdad (`native/sys_bare.titan`). Primitivas nuevas del runtime: `std::raw::mmio_load32/mmio_store32` (volátiles), `sysreg/set_sysreg`, `hvc`, `bare` | ✅ 118 / 120 programas idénticos a la VM (salida y código de salida) en un ARM64 emulado con su MMU (`native/bare/probar.sh`, `native/bare/correr.py` con unicorn); las 2 diferencias eran defectos de Rust, corregidos (ver Fase 6). En la CI también en `qemu-system-aarch64 -machine virt` real (job `sin-so`) |

## Cómo verificar

```bash
bash scripts/sandbox-zett.sh            # binario de la rama `binaries`
export PATH="$HOME/.local/bin:$PATH"
bash selfhost/verify_lexer.sh            # lexer Titan vs lexer Rust, byte a byte
bash selfhost/verify_parser.sh           # parser Titan vs parser Rust, byte a byte
bash selfhost/verify_typechecker.sh      # typechecker Titan vs Rust, byte a byte
bash selfhost/verify_codegen.sh          # cargador + codegen Titan vs Rust, byte a byte
bash selfhost/native/verify_x64.sh       # codificador x86-64 en Titan vs el ensamblador GNU
bash selfhost/native/verify_native.sh    # ejecutables nativos vs `zett run` (salida, errores, código)
bash selfhost/native/verify_json.sh      # std::json::parse nativo vs VM en 97 casos (errores con línea/columna)
bash selfhost/native/cobertura.sh [-v]   # cuántas nativas de la biblioteca ya están en Titan
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
6. (pendiente) Un `if` con ramas de tipos distintos, como **última**
   expresión del cuerpo de un `for`, da "type mismatch"; en cualquier otro
   sitio se acepta. El código Titan lo esquiva con `continue`.
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
- Verificación: 461 archivos, 105.746 líneas, **idénticos** (215 programas
  compilados y 234 mensajes de error). Incluye `selfhost/tests/codegen/`.
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
10. El cargador de Titan todavía no admite proyectos con `Titan.toml`
    (manifiesto y dependencias): los rechaza con un error explícito. En el
    repositorio no hay ninguno.
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
- `verify_native.sh`: 18/18 idénticos (incluye memoria, archivos, bytes,
  decimales y sus errores).
- El compilador nativo produce lo mismo que Rust en **495 de 495** archivos
  (`SELF=bytecode-nativo bash selfhost/verify_codegen.sh`).

## Fase 5 — punto fijo del bootstrap ✅

`bash selfhost/verify_fixpoint.sh`:

1. La VM de Rust ejecuta `selfhost/build.titan` (el compilador en Titan)
   sobre sí mismo → `selfhost/titanc1` (ejecutable x86-64 de 5,6 MB).
2. `titanc1`, **sin Rust ni VM y con el entorno vacío** (sin PATH: no puede
   llamar a `zett` ni a ningún otro programa), se compila → `titanc2`.
3. `titanc2` se compila → `titanc3`.

Resultado: las tres etapas son **idénticas byte a byte**
(sha256 `2261f1a6…` desde la sesión de std::datetime); cada etapa nativa tarda ~24 s. El compilador no usa
`std::process` ni FFI. Esta es la única vez que se usa el Titan de Rust
(el arranque); a partir de `titanc1`, Titan se compila solo.

Lo que falta para borrar Rust (fase 6): el resto de nativas de la biblioteca estándar (~800; hoy el runtime tiene las
que usa el compilador), las herramientas del CLI (`titan run`, `fmt`,
etc.) escritas en Titan, y ARM64 para Termux.

- Limitaciones anotadas: `std::path::parent`/`canonical` siguen las reglas
  de `Path` de Rust para los casos habituales; casos raros (p. ej. rutas
  con bytes no UTF-8) no están probados.

## Fase 6 — biblioteca estándar en Titan (en curso)

Cada módulo se escribe en Titan dentro del runtime (`selfhost/native/`) y se
compara con la VM de Rust con programas de `selfhost/tests/native/` (salida,
mensaje de error y código de salida idénticos). El enlazador solo mete en cada
ejecutable las funciones del runtime que usa (lista de trabajo en `bk_build`).

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
| std::process (22: run, run_with_input, shell, pipe, run_timeout, spawn, spawn_wait, spawn_poll, spawn_kill, spawn_pid, env_get, env_set, env_unset, env_vars, working_dir, set_working_dir, self_pid, hostname, username, args, send_signal, exit) | `std_process.titan` (+ `rt_environ`/`rt_getenv` en `runtime.titan`) | lo mismo que process_mod.rs y `run_timeout` de process.rs (notas en `native/process/NOTAS.md`): los programas se lanzan como `posix_spawnp` de glibc (fork, redirecciones, máscara de señales vacía, SIGPIPE por defecto, la búsqueda en PATH de `__execvpex` con su regla del último errno, el errno del exec fallido pasado al padre); la salida y la entrada se leen/escriben a la vez con `poll` (cupo compartido de 4 MiB, EPIPE en vez de morir por SIGPIPE); los procesos en segundo plano tienen un proceso ayudante que lee sus tubos mientras corren (lo que en Rust hacen dos hilos; el runtime no tiene hilos) y al terminar el programa se matan y se esperan como en el `Drop` de Rust (pero no con `exit`, igual que Rust). `env_set`/`env_unset` cambian un entorno propio del runtime que ven `std::env`, `std::dirs`, el `tput` de `std::term` y los hijos. Comparado: 66 programas idénticos (6 normales y 58 de errores, más `ruta_palabra_clave` y `dirs_lineas_malas`, que ahora pone `XDG_CONFIG_HOME` con `env_set`) y variables no UTF-8 a mano. **Defectos de Rust corregidos:** 16 (`spawn` no se podía escribir), 17 (panic en `env_set`/`env_unset`) y 18 (panic en `env_vars`). Diferencia honesta: el ayudante aparece como un proceso más en `ps` |
| std::url (10: is_valid, scheme, host, port, path, query, fragment, join, parse_query, build_query) | `std_url.titan`, `std_idna.titan`, `std_icu.titan`, `std_icu_tab.titan` (generado por `native/icu/gen_icu.py`) | lo mismo que url_mod.rs sobre url 2.5.8 + form_urlencoded 1.2.2 + percent-encoding 2.3.2 + idna 1.1.0 + idna_adapter 1.2.2 + ICU4X 2.2.0, traducido de su código original (descargado por `fuentes-crates.yml`, sumas de `Cargo.lock`): el analizador WHATWG de `parser.rs` (esquemas especiales, `file:`, rutas opacas, `.`/`..` y `%2e`, IPv4 con octal/hex, IPv6 con `::` y IPv4 dentro, puertos, usuario/contraseña, tabuladores y saltos quitados, URL base para `join`), los mismos mensajes de error, `form_urlencoded` (`+`, UTF-8 con U+FFFD, último valor gana, mapa ordenado). Los dominios no ASCII pasan por UTS 46 como `idna` (lista de caracteres prohibidos de URL, punycode en los dos sentidos, CONTEXTJ con virama y tipos de unión, reglas bidi, marca al principio, largo máximo) y la normalización de ICU4X: los iteradores `Decomposition`/`Composition` traducidos paso a paso (incluidos los casos raros de U+0345, U+0F73 y U+FDFA) sobre sus propias tablas: el `CodePointTrie` de UTS 46, las tablas NFD/NFKD, el `Char16Trie` de composiciones y los tries de Bidi_Class, Joining_Type y General_Category, copiados byte a byte de `icu_normalizer_data`/`icu_properties_data` (Unicode de ICU4X 2.2, no el de Python). Comparado con la VM: 3 pruebas (`url_basico`, `url_errores` y `url_casos` con 272 URL, 60 `join`, 24 consultas y 10 `build_query`), 38.000 casos aleatorios (`native/url/fuzz.py`, 13 semillas) y todos los puntos de código de Unicode dentro de un dominio, solos y con una marca combinante detrás (`native/url/todos.py`, 2,2 millones de casos): 0 diferencias |
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
| std::http (13 de 13: parse_request, build_response, reason_phrase, route_match, parse_query, security_headers, cors, request_id, rate_limit, json_response, error_response, request, parse_multipart) | `std_http.titan` | lo mismo que http.rs, multipart.rs y el cliente de http_client.rs: petición HTTP/1.1, Host obligatorio, Content-Length, rutas `:id`/`*path`, query con `+` y percent-encoding, cabeceras de seguridad y CORS, `titan-{id:016x}`, límite de ritmo, JSON compacto, multipart/form-data, cliente `http://` (TCP, redirecciones, chunked). HTTPS del cliente aún pide TLS, igual que `std::net::http_get`. Comparado: pruebas `http_std` y `http_errores` idénticas a la VM |

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

## Pendientes conocidos (anotados para no olvidarlos)

- **Asignación al final de una rama `if`** (VM de Rust y compilador en Titan,
  igual en los dos): si la rama `then` termina en una llamada y la `else` en
  `x = [..]`, el verificador exige que las dos ramas tengan el mismo tipo y
  falla con "expected Nil, found Array". Una asignación en posición de
  sentencia debería valer `()`. Se mantiene igual en los dos mientras la VM
  de Rust exista; en el código se evita con un `continue`/sentencia al final.
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

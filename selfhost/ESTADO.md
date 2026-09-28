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
| 6 | Biblioteca estándar y runtime en Titan; borrar el último `.rs` | en curso: 336 / 816 nativas (`selfhost/native/cobertura.sh`) |

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
11. `std::uuid::nil()` no se puede llamar desde Titan: `nil` es palabra clave
    y el parser la rechaza después de `::` ("expected an identifier, found
    Nil"). La nativa existe pero es inalcanzable. Pendiente de decidir si se
    cambia el parser o el nombre de la función.
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
    prueban están en `tests/native_pendiente_vm/`: coincidirán cuando se use
    un `zett` reconstruido con la corrección (el precompilado es anterior).
13. **`std::dirs` hacía *panic*** al pedir cualquier carpeta de usuario
    (`desktop`, `documents`, `music`…) si `~/.config/user-dirs.dirs` tenía
    una línea `XDG_DIR=...` o un valor que era solo `"`: el crate dirs-sys
    0.4.1 corta la cadena con índices al revés (`xdg_user_dirs.rs:35` y
    `:54`). Una sola línea así hacía caer el programa.

    **Corregido en los dos lados.** En Rust (`dirs_mod.rs`) la lectura de
    `user-dirs.dirs` ya no usa el crate en Linux/Android: mismas reglas, pero
    esas líneas se ignoran como cualquier otra que no se entiende (con tests).
    En Titan (`std_dirs.titan`) igual. La prueba está en
    `tests/native_pendiente_vm/dirs_lineas_malas.titan`.
14. **`std::xml::parse` perdía datos sin avisar** si el documento terminaba
    con etiquetas abiertas: quick-xml no da error en ese caso y `xml_mod`
    devolvía solo la última etiqueta abierta (`parse("<a><x/><b>hola")` daba
    `{tag: b, text: hola}`: se perdían `<a>` y `<x/>`).

    **Corregido en los dos lados.** Ahora es un error, con el mensaje que
    quick-xml usa para ese caso: `ill-formed document: start tag not closed:
    `</b>` not found before end of input` (con test en `xml_mod.rs`). La
    prueba está en `tests/native_pendiente_vm/std_error_xml_abierta.titan`.

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
- `std::url` (pospuesto): el crate `url` convierte los dominios con IDNA
  (UTS 46), que necesita las tablas Unicode de ICU4X 2.2 (Unicode ~17:
  mapeo, normalización NFC, reglas bidi). En este entorno no hay de dónde
  sacarlas (Python trae Unicode 14 y no hay internet desde la terminal); sin
  ellas el resultado no sería idéntico con dominios no ASCII.
- El resto de módulos (url, regex, net,
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

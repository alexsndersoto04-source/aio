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
| 6 | Biblioteca estándar y runtime en Titan; borrar el último `.rs` | en curso: 240 / 816 nativas (`selfhost/native/cobertura.sh`) |

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

| std::datetime (47 de 49) | `std_datetime.titan` | calendario gregoriano proléptico (−262143 a 262142), formatos `%…` con el mismo tokenizador que `StrftimeItems` de chrono 0.4.45, lectura con el mismo algoritmo de `Parsed` (semanas ISO, `%U`/`%W`, `%s`, segundos intercalares, `%C`/`%y`) y los mismos 7 errores; RFC 3339/2822; aritmética de datetime_ext (con vuelta en el desborde, como Rust en release). Comparado además con 12.000 casos aleatorios y 89 casos límite escritos a mano: 0 diferencias. Faltan `to_timezone` y `timezone_offset_seconds` (necesitan la base de datos de zonas horarias de IANA, sesión propia) |

Defecto real encontrado y corregido: `std::array::set` fuera de rango decía
"index out of bounds" en el runtime nativo; la VM dice "array index out of
bounds".

Defecto del runtime nativo encontrado con datetime y corregido: al imprimir un
error sin capturar, `rt_flatten`/`rt_trim` solo reconocían espacios ASCII; el
CLI de Rust usa `split_whitespace`/`trim`, que reconocen todos los espacios
Unicode (por ejemplo U+3000). Ahora se usan los mismos.

Pendiente en esta fase:
- **Funciones trascendentes** (`sin`, `cos`, `tan`, `exp`, `ln`, `log`,
  `pow`…): la VM usa la libm de glibc; para dar los mismos bits hay que portar
  sus algoritmos. Se hará en una sesión dedicada.
- Rendimiento: `std::json` sobre 1,2 MB tarda ~0,42 s en nativo frente a
  ~0,11 s en la VM (serde en Rust optimizado). El tiempo se reparte entre el
  asignador de memoria y la conversión de floats; se mejorará junto con el
  asignador.
- Zonas horarias de `std::datetime` (`to_timezone`,
  `timezone_offset_seconds`, chrono-tz 0.10.4): hay que llevar a Titan la base
  de datos de IANA con sus reglas.
- El resto de módulos (url, regex, net,
  http, process…) y las herramientas del CLI.

## Pendientes conocidos (anotados para no olvidarlos)

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

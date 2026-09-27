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
| 4b | Siguiente: bytecode → **ejecutable nativo** x86-64 + runtime en Titan (sin VM en Rust) | ⏳ siguiente |
| 5 | Titan se compila a sí mismo (punto fijo: etapa1 == etapa2 byte a byte) | pendiente |
| 6 | Biblioteca estándar y runtime en Titan; borrar el último `.rs` | pendiente |

## Cómo verificar

```bash
bash scripts/sandbox-zett.sh            # binario de la rama `binaries`
export PATH="$HOME/.local/bin:$PATH"
bash selfhost/verify_lexer.sh            # lexer Titan vs lexer Rust, byte a byte
bash selfhost/verify_parser.sh           # parser Titan vs parser Rust, byte a byte
bash selfhost/verify_typechecker.sh      # typechecker Titan vs Rust, byte a byte
bash selfhost/verify_codegen.sh          # cargador + codegen Titan vs Rust, byte a byte
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
- El parser usa la nativa `std::text::parse_float` (conversión decimal→binario
  de Rust, correctamente redondeada) y el volcado usa la conversión
  float→texto de la VM (dígitos mínimos que reproducen el valor). En la fase 6
  hay que escribir ambos algoritmos en Titan.
- Los mensajes de error del parser citan el token con el formato `{:?}` de
  Rust. Para caracteres Unicode exóticos Rust consulta su tabla de
  "imprimibles"; el parser en Titan aproxima esa tabla con los rangos
  habituales (controles, marcas combinantes, espacios de ancho cero, uso
  privado). Solo afecta al texto de errores con esos caracteres.
- No existe `print` sin salto de línea (útil para herramientas).
- Fase 2 necesita un oráculo del AST: un volcado canónico (`titan ast`) en
  Rust, como `titan_lexer::dump_tokens`, para comparar byte a byte.

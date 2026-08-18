# Titan Language Reference — Programación Real

Una guía práctica basada en 15 releases de escribir código Titan.
Cubre los patrones, los gotchas conocidos, y cómo escribir programas
que realmente funcionen.

## Índice

1. [Tipos básicos](#tipos-básicos)
2. [Variables y mutabilidad](#variables-y-mutabilidad)
3. [Strings y concatenación](#strings-y-concatenación)
4. [Arrays](#arrays)
5. [Maps (diccionarios)](#maps-diccionarios)
6. [Funciones](#funciones)
7. [Control de flujo](#control-de-flujo)
8. [Interpolación de strings](#interpolación-de-strings)
9. [Anotaciones de tipo](#anotaciones-de-tipo)
10. [Llamadas a la stdlib](#llamadas-a-la-stdlib)
11. [Gotchas conocidos sin fix](#gotchas-conocidos-sin-fix)

---

## Tipos básicos

Titan tiene los siguientes tipos primitivos:

| Alias         | Tipo interno       | Ejemplo                    |
|---------------|--------------------|----------------------------|
| `int`         | Int (i64)          | `42`, `-7`, `0`            |
| `float`       | Float (f64)        | `3.14`, `-0.5`, `2.0`      |
| `bool`        | Bool               | `true`, `false`            |
| `string`      | String             | `"hola"`, `"multi líneas"` |
| `char`        | Char (Unicode)     | `'a'`, `'ñ'`               |
| `nil`         | Nil                | `nil`                      |
| `array`       | Array(Unknown)     | `[1, 2, 3]`, `["a", 2]`    |
| `[T]`         | Array(T)           | `[float]`, `[int]`         |
| `map`         | Named("map")       | `std::map::new()`          |
| `any`         | Unknown            | acepta cualquier cosa      |

---

## Variables y mutabilidad

```titan
let x = 42              // inmutable
let mut y = 10          // mutable
y = 20                  // OK
// x = 100              // ERROR
```

Los `let` en un `for` **no crean shadowing persistente**:

```titan
let mut acc = []
for i in 0..5 {
    acc = std::array::push(acc, i)   // OK: modifica el mut exterior
    // let acc = ...                  // BAD: shadowing local, se pierde al salir
}
// acc == [0, 1, 2, 3, 4]
```

---

## Strings y concatenación

Desde **v0.16.0**, `+` acepta String con cualquier tipo (coerción automática):

```titan
let n = 42
let msg = "cantidad: " + n            // "cantidad: 42"
let m = 100 + " puntos"               // "100 puntos"  (simétrico)
let out = "arr: " + [1, 2, 3]         // "arr: [1, 2, 3]"
```

Encadenamiento libre:

```titan
let user = "juan"
let score = 95
print("El usuario " + user + " tiene " + score + " puntos")
```

---

## Arrays

Arrays **homogéneos y heterogéneos** ambos válidos desde **v0.16.0**:

```titan
let a = [1, 2, 3]                       // Array(Int)
let b = ["hola", "mundo"]               // Array(String)
let c = ["cpu", [1.0, 2.0], [3.0, 4.0]] // Array(Unknown) — mixto OK
```

Operaciones comunes:

```titan
let xs = [1, 2, 3]
let n = std::collections::length(xs)        // 3
let ys = std::array::push(xs, 4)            // [1,2,3,4]  (xs sin cambiar)
let zs = std::array::filled(10, 0.0)        // [0.0, 0.0, ..., 0.0]  (10 elementos)
let idx = xs[0]                             // 1  (indexación por int)
let updated = std::array::set(xs, 1, 99)    // [1, 99, 3]
```

Push en un loop (patrón muy común):

```titan
let mut acc = []
for i in 0..n {
    acc = std::array::push(acc, transform(i))
}
```

---

## Maps (diccionarios)

Los maps son "estructura genérica de string → any". Se construyen paso a paso:

```titan
let cfg = std::map::new()
let cfg = std::map::insert(cfg, "port", 8080)
let cfg = std::map::insert(cfg, "host", "0.0.0.0")

let port = cfg.port                         // 8080  (acceso por dot)
let host = std::map::get(cfg, "host")       // "0.0.0.0"
```

**Importante:** `map.field` en Titan devuelve `Unknown`, así que no
podés usar el resultado en un `require_compatible` estricto. Está OK
si lo pasás a otras funciones que también acepten Unknown.

---

## Funciones

```titan
fn suma(a: int, b: int) -> int {
    a + b
}

fn saludar(nombre: string) {          // sin -> ⇒ retorna Unit
    print("Hola " + nombre)
}

fn generico(x) -> any {               // x sin anotación ⇒ Unknown
    x
}
```

**Parámetros sin anotación** son `Unknown` — pueden recibir cualquier
tipo. Útil cuando el argumento viene de un native que devolvió `Array`
(genérico) o de un `map.field` (Unknown).

**Regla práctica:** si el arg viene de una fn nativa, **no lo anotes**:

```titan
// Bien
fn print_top(items) {                 // items sin tipo
    for i in 0..std::collections::length(items) {
        print(items[i])
    }
}

// Mal (falla si items viene de un native "Array" genérico)
fn print_top(items: [map]) { ... }    // fuerza Array(Named("map"))
```

---

## Control de flujo

### if/else

Ambas ramas pueden tener **tipos distintos** desde **v0.16.0**:

```titan
if condicion {
    print("A")            // Nil
} else {
    do_something()        // Unit
}
// Ahora es válido — el tipo del if-expr degrada a Unknown
```

Si querés usar el valor del if-expr como una expresión concreta,
ambas ramas tienen que devolver lo mismo:

```titan
let x = if cond { 10 } else { 20 }   // int
let s = if cond { "sí" } else { "no" } // string
```

### for

```titan
for i in 0..10 {                 // 0..10 exclusivo del final
    print(i)
}

for item in [1, 2, 3] {
    print(item)
}
```

### while

```titan
let mut n = 10
while n > 0 {
    print(n)
    n = n - 1
}
```

### match

```titan
match tag {
    "hello" => print("saludo"),
    "bye"   => print("despedida"),
    _       => print("otro"),
}
```

---

## Interpolación de strings

La gramática de interpolación es deliberadamente pequeña: admite un identificador local, una constante global declarada o una llamada nombrada cuyos argumentos sean identificadores locales o enteros literales. Las llamadas usan la misma resolución que fuera del string, por lo que una closure local o una constante invocable puede ser el destino.

```titan
const LIMIT: int = 20

fn main() {
    let x = 42
    let arr = [1, 2, 3]
    let siguiente = |value: int| value + 1
    print("x = {x}")                       // "x = 42"
    print("límite = {LIMIT}")               // "límite = 20"
    print("arr = {arr}")                   // "arr = [1, 2, 3]"
    print("siguiente = {siguiente(x)}")     // "siguiente = 43"
    print("home: {std::dirs::home()}")     // llamadas también
}
```

Una expresión como `{x + 10}` todavía no pertenece a esa gramática. Debe calcularse primero en un local y luego interpolarse.

Contenidos válidos entre `{...}`:
- Identificadores locales (`x`, `foo`) y constantes globales declaradas (`LIMIT`)
- Llamadas simples (`fn()`, `fn(arg)`, `std::dirs::home()`), con argumentos locales o enteros

Contenidos NO válidos:
- Aritmética: `{a + b * c}` → **NO soportada**
- Acceso a campos o índices: `{user.name}`, `{arr[0]}` → **NO soportado**

Si necesitás algo complejo, extraelo a una variable primero:

```titan
let total = a + b * c
print("total: {total}")
```

---

## Anotaciones de tipo

### Aliases nuevos desde v0.16.0

```titan
fn foo() -> array   { [1, 2, 3] }        // Array(Unknown)
fn bar() -> map     { std::map::new() }   // Named("map")
fn baz(x: any) -> any { x }               // Unknown
```

Estos son atajos convenientes para no escribir `[any]` o
`std::map::new` genérico.

### Sigils tradicionales

```titan
fn a() -> [int]    { [1, 2, 3] }
fn b() -> [float]  { [1.0, 2.0] }
fn c() -> [string] { ["a", "b"] }
```

Ambos estilos coexisten.

---

## Structs y métodos (`impl`) — v0.19.0

Un struct declara los campos. Un bloque `impl` le agrega métodos:

```titan
struct Point { x: float, y: float }

impl Point {
    // Estático: se llama `Point::origin()`.
    fn origin() -> Point { Point { x: 0.0, y: 0.0 } }

    // Estático con args.
    fn new(x: float, y: float) -> Point { Point { x: x, y: y } }

    // Instancia: `self` como primer parámetro. Tipo inferido = Point.
    fn distance_sq(self, other: Point) -> float {
        let dx = self.x - other.x
        let dy = self.y - other.y
        dx * dx + dy * dy
    }

    fn magnitude_sq(self) -> float {
        self.x * self.x + self.y * self.y
    }
}

fn main() {
    let o = Point::origin()
    let p = Point::new(3.0, 4.0)
    print(p.distance_sq(o))        // 25.0
    print(p.magnitude_sq())        // 25.0
}
```

Reglas:

- **Métodos estáticos**: se llaman con `Tipo::nombre(args)` — sin
  receiver. Útiles para constructores / factories.
- **Métodos de instancia**: primer parámetro se llama `self`. No
  hace falta anotar su tipo: se infiere como el tipo del `impl`.
- **Dispatch dinámico**: `p.metodo(args)` mira el tipo real del
  valor `p` en runtime. Dos structs distintos pueden compartir
  nombre de método sin colisión.
- **Encadenable**: `p.translated(1.0, 2.0).magnitude_sq()` funciona
  siempre que cada método devuelva un valor con métodos.

Ejemplo completo verificable: `examples/impl_structs.titan`.

---

## Fixes de limpieza — v0.31.0

### Tipos función como anotaciones
```titan
type Callback = fn(int) -> int
type Predicate = fn(int) -> bool
type Producer = fn() -> string

fn aplicar(x: int, cb: Callback) -> int { cb(x) }
```

### `let _ = expr` para descartar
```titan
let _ = calcular()          // side effect, resultado descartado
let _ = 1 + 2 + 3
```

### Sintaxis `.N` para tuplas
```titan
let t = (10, 20, 30)
print(t.0)                  // 10 — igual que t[0]
print(t.1)                  // 20
print(t.2)                  // 30
```

### Parse y substring en `std::text`
```titan
match std::text::parse_int("42") {
    Option::Some(n) => print(n),
    Option::None    => print("no es int"),
}
std::text::parse_float("3.14")           // Option::Some(3.14)
std::text::substring("año", 0, 2)        // "añ" (Unicode chars, no bytes)
```

Ejemplo completo: `examples/fixes_v032.titan`.

---

## `std::async` — utilidades de reintento y tiempo — v0.30.0

Primer módulo `std::` escrito en Titan (no en Rust). Se importa
con `import std::async` — el loader lo carga desde el `.deb`.

```titan
import std::async

delay(500)                              // sleep bloqueante ms

let (r, ms) = measure(|| foo())         // (resultado, ms transcurridos)

let r = retry(|| fetch(url), 3, 500)    // 3 intentos, 500ms entre cada
let r = retry_backoff(|| fetch(url), 5, 100)  // 100→200→400→800ms

let r = timeout(|| lenta(), 2000)       // Err si excede 2000ms
```

Todas devuelven `Result::Ok(v)` o `Result::Err(msg)`, combinable
con `std::try::catch` (Fase 18).

**Cómo se agrega otro módulo `std::` en Titan puro:**
Poner `<nombre>.titan` en `stdlib/` del repositorio; el `.deb`
lo empaqueta automáticamente en `$PREFIX/share/zett/stdlib/`. El
loader lo encuentra al hacer `import std::<nombre>`.

Ejemplo completo: `examples/async_demo.titan`.

---

## `const` con expresiones + maps literales `#{...}` — v0.29.0

Const acepta cualquier expresión (arrays, calls, higher-order):

```titan
const NUMEROS   = [1, 2, 3, 4, 5]
const CUADRADOS = map(NUMEROS, |n| n * n)
```

Maps literales con `#{...}`:

```titan
const CONFIG = #{
    "puerto": 8080,           // key con comillas
    host:     "127.0.0.1",    // key como identificador (equivalente)
    debug:    true,
}

// Acceso natural
print(CONFIG.puerto)          // 8080
print(CONFIG.host)            // "127.0.0.1"

// Anidable
const NESTED = #{
    outer: #{
        inner: #{ deep: 42 },
    },
}
print(NESTED.outer.inner.deep)   // 42

// Dinámico (dentro de fns), no solo const
fn perfil(nombre: string) -> map {
    return #{ nombre: nombre, creado: std::datetime::now() }
}
```

Detalles:
- Keys pueden ser `"string"` o `identificador` (equivalente).
- Values son cualquier expresión válida.
- Desazucar puro en el parser → `std::map::insert(std::map::new(), "k", v)`.
- Const con expresiones se re-evalúa lazy en cada uso.

Ejemplo completo: `examples/const_and_maps.titan`.

---

## `for` con destructuring — v0.28.0

Extiende Fase 22 (destructuring en `let`) al loop `for`:

```titan
for (a, b) in pares { ... }                    // tupla
for Point { x, y } in puntos { ... }           // struct
for Point { x: cx, y: cy } in puntos { ... }   // rename
for (first, _, last) in triples { ... }        // wildcard
for (id, (a, b)) in anidados { ... }           // anidado
for (id, Point { x, y }) in etiquetados { ... } // combinado
```

Reglas: mismas que Fase 22. El azúcar se resuelve en el parser
como un `for __item in xs { let PATTERN = __item; body }`. Cero
cambios en runtime.

Ejemplo completo: `examples/for_destructuring.titan`.

---

## Type aliases + spread `..` — v0.27.0

Aliases:

```titan
type UserId = string
type Score  = int
type Handler = fn(int) -> string

fn find(id: UserId) -> Player { ... }   // igual que `id: string`
```

Cero costo runtime. Cambiás la definición y todas las firmas se
adaptan. Chain permitido: `type A = B; type B = int` funciona.

Spread en literales de array:

```titan
let a = [1, 2, 3]
let b = [10, 20, 30]

[..a, ..b]                  // [1,2,3,10,20,30]
[0, ..a, 99, ..b, 100]      // [0,1,2,3,99,10,20,30,100]
[..map(a, |x| x*x), 999]    // spread + higher-order
```

`..` **al inicio** de un elemento indica spread. Los ranges
`0..10` no se ven afectados (van entre dos expresiones).

Ejemplo completo: `examples/aliases_spread.titan`.

---

## Errores custom con enums — v0.26.0

```titan
enum ApiError {
    NotFound(string),
    BadInput(string),
    Unauthorized,               // sin payload
    RateLimited(int),
}

fn find_user(id: string) -> any {
    if id == "" { return Result::Err(ApiError::BadInput("id")) }
    return Result::Ok(build_user(id))
}

// El operador `?` propaga el error tipado hacia arriba.
fn describe(id: string) -> any {
    let u = find_user(id)?      // si Err, salimos con ese Err
    return Result::Ok(u.name)
}

// Extraer contexto con match:
match describe("99") {
    Result::Ok(msg) => print(msg),
    Result::Err(err) => match err {
        ApiError::NotFound(id)      => print("no existe: " + id),
        ApiError::BadInput(field)   => print("campo: " + field),
        ApiError::Unauthorized      => print("sin auth"),
        ApiError::RateLimited(secs) => print("esperar " + secs),
    }
}
```

Reglas:

- `enum X { ... }` en top-level; variantes con `,` como separador.
- Variantes con payload: `Name(tipo)` — 0 o 1 payload por variante.
- Variantes sin payload: solo el nombre.
- Se construyen con `X::Variant` (sin payload) o `X::Variant(val)`.
- `Result::Err(cualquierEnum)` es idiomático para errores tipados.
- El operador `?` funciona con cualquier `Result` — propaga limpio.
- El pattern del inner en match debe ser `Ident` o `_` (no anidado).

Ejemplo completo: `examples/custom_errors.titan`.

---

## JSON completo (`std::json`) — v0.24.0

```titan
// Parsear -> Value nativo (objects son Map, arrays son Array)
let obj = std::json::parse(text)
print(obj.nombre)                          // acceso con .
print(obj.hobbies[0])                      // acceso con [i]
print(obj.direccion.ciudad)                // anidado

// Serializar
std::json::stringify(x)                    // compacto
std::json::pretty(x)                       // indentado

// JSON Pointer (paths estilo XPath)
std::json::pointer(x, "/direccion/cp")
std::json::pointer(x, "/hobbies/1")

// Merge Patch (b sobre a; null en b borra la clave)
std::json::merge(base, patch)

// Aplanar -> [(path, valor)]
std::json::flatten(x)
```

Integración con HTTP:

```titan
// GET con parse automático
let user = std::http_full::get_json(url, headers, opts)
print(user.login)

// POST con serialización automática del payload
let resp = std::http_full::post_json(url, payload, headers, opts)
```

Conversiones: Object↔Map, Array↔Array, Number→Int o Float según,
String↔String, Bool↔Bool, null↔Nil. Los structs de Titan también
se pueden pasar directo — sus campos son las keys del JSON.

Ejemplo completo: `examples/json_api.titan`.

---

## Operadores `|>` pipeline y `<=>` spaceship — v0.23.0

### Pipeline

```titan
x |> f              // == f(x)
x |> f(a, b)        // == f(x, a, b) — LHS va como PRIMER arg
x |> f |> g         // == g(f(x)) — encadenable, left-assoc

// Combina brutal con higher-order:
[1, 2, 3] |> map(|x| x*x) |> fold(0, |acc, x| acc + x)
```

Precedencia mínima: `a + 1 |> print` agrupa como `(a+1) |> print`.

### Spaceship

```titan
a <=> b             // -1 si a<b, 0 si a==b, 1 si a>b (solo int/float)

// Reemplaza el comparador manual en sort_by:
sort_by(xs, |a, b| a <=> b)              // asc
sort_by(xs, |a, b| b <=> a)              // desc
sort_by(items, |x, y| x.precio <=> y.precio)   // por campo
```

Cada lado se evalúa una sola vez (usa dos temps internos).

Ejemplo completo: `examples/pipeline_spaceship.titan`.

---

## Destructuring en `let` — v0.22.0

Para desempacar tuplas y structs directamente:

```titan
// Tupla
let (a, b) = par
let (lo, hi) = min_max(xs)
let (first, _) = par                       // wildcard descarta

// Struct
let Point { x, y } = p                     // igual nombre
let Point { x: cx, y: cy } = p             // con rename

// Anidado
let (id, Point { x, y }) = combo
let Person { name, home: Address { city } } = ana
```

Reglas:

- Después de `let`, `(` inicia patrón tupla.
- `Ident { ... }` seguido de `=` es patrón struct.
- `_` descarta.
- La RHS se evalúa **una sola vez**.
- Recursivo sin límite.
- Enum patterns (`let Some(x) = opt`) NO — usar `match`.

Ejemplo completo verificable: `examples/destructuring.titan`.

---

## Traits con métodos default — v0.21.0

Un `trait` define un contrato: una lista de métodos que un tipo debe
implementar. Cada método puede tener body default o solo firma.

```titan
trait Greetable {
    fn name(self) -> string;              // requerido
    fn greet(self) -> string {            // default
        "Hola, " + self.name() + "!"
    }
}

struct Person { first: string }

impl Greetable for Person {
    fn name(self) -> string { self.first }
    // greet se hereda del trait automáticamente.
}

fn main() {
    let p = Person { first: "Ana" }
    print(p.greet())    // "Hola, Ana!"
}
```

Reglas:

- Trait declarado con `trait Name { ... }`, dentro solo van `fn` con
  o sin body.
- `fn foo();` (con `;`) es **obligatorio** — el impl debe proveerlo.
- `fn foo() { ... }` (con body) es **default** — el impl puede
  omitirlo y hereda ese body.
- El impl se conecta con `impl Trait for Type { ... }`.
- Los defaults pueden llamarse entre sí via `self.otro()` — el
  dispatch es dinámico (Fase 20).
- Un mismo struct puede implementar múltiples traits sin colisión.
- Si un método requerido falta en el impl, error en compilación.

Ejemplo completo verificable: `examples/traits.titan`.

---

## Módulos custom con `import` — v0.20.0

Un proyecto Titan puede tener varios archivos `.titan`. Un archivo
carga a otro con `import`:

```titan
// examples/modules/main.titan
import geometry           // carga geometry.titan
import util::text         // carga util/text.titan
import util::math         // carga util/math.titan

fn main() {
    let p = Point::new(3.0, 4.0)   // Point definido en geometry.titan
    print(greet("mundo"))           // greet definido en util/text.titan
    print(sum([1, 2, 3]))           // sum definido en util/math.titan
}
```

Estructura:

```
examples/modules/
├── main.titan
├── geometry.titan
└── util/
    ├── math.titan
    └── text.titan
```

Reglas:

- `import a::b` busca `a/b.titan` o `a/b/mod.titan`.
- Sin `Titan.toml`, el "source root" es la carpeta del entry.
  Con `Titan.toml`, es `<root>/src/` (estilo Cargo).
- Todos los items (structs, impls, funciones, consts, enums) van a
  un **namespace plano**: `Point::new()` funciona directo,
  no hace falta `geometry::Point::new()`.
- **Ciclos** (a importa b, b importa a) → error claro.
- **Escape** (`import ../../secreto`) → error.
- **Sin doble carga**: importar el mismo archivo desde dos módulos
  distintos no lo duplica.
- `import std::...` se ignora (la stdlib es nativa, no `.titan`).

Ejemplo completo verificable: `examples/modules/main.titan`.

---

## Llamadas a la stdlib

Las natives están organizadas por módulo:

```titan
// Filesystem
let bytes = std::fs::read_bytes("/path")
std::fs::write_text("/out.txt", "hola")

// Números
let r = std::random::int(1, 100)
let e = std::math::exp(2.0)

// Strings
let up = std::text::uppercase("titan")

// Colecciones
let n = std::collections::length(xs)

// Y muchos más (regex, hash, url, jwt, crypto, http, sql, etc.)
```

Consultar `crates/titan_stdlib/src/native.rs` para el registro
completo (~410 funciones al momento de v0.16.0).

---

## Gotchas conocidos sin fix

Estos son casos donde el parser/typechecker sigue siendo estricto y
hay que usar workarounds:

### `[` al inicio de línea = indexación

```titan
let sum = a + b
[1, 2, 3]                     // ← esto es sum[1,2,3], NO un array nuevo
                              // → ERROR de parseo
```

**Workaround:** asignar el array a una variable en la misma línea:

```titan
let sum = a + b
let arr = [1, 2, 3]           // OK
```

### Handles usados después de close

```titan
std::pdf::close(doc)
std::pdf::page_count(doc)     // → runtime ERROR: unknown handle
```

**Workaround:** consultar antes de cerrar.

### Errores runtime son fatales

Titan **no tiene try/catch**. Un error de un native (archivo no
existe, conexión rechazada, etc.) mata todo el programa.

**Workaround:** verificar precondiciones antes:

```titan
if std::fs::exists(path) {
    let bytes = std::fs::read_bytes(path)
    ...
} else {
    print("Archivo no existe")
}
```

En una futura Fase 17 pt.5 podría agregarse `try?` para expresiones
que devuelvan nil ante fallo.

---

## Ejemplos completos

Ver el directorio `examples/` — cada `.titan` demuestra un patrón
funcional distinto:

- `hello.titan`, `fibonacci.titan`     — básicos
- `extras.titan`, `formats.titan`      — regex, hash, yaml, xml, etc.
- `network.titan`, `security.titan`    — HTTPS, DNS, JWT, crypto
- `android.titan`, `tui.titan`         — Termux hardware + TUI
- `images.titan`, `audio.titan`        — imágenes + WAV
- `system.titan`, `database.titan`     — procfs, sled, redis
- `webserver.titan`                    — HTTP server con router
- `charts.titan`                       — SVG plots
- `tokenizer.titan`, `onnx.titan`      — Fase 12 pt.1-2
- `sentiment.titan`, `vector_search.titan`  — Fase 12 pt.3-4
- `wifi.titan`                         — Fase 13'
- `qol_*.titan`                        — demos de Fase 17 QoL

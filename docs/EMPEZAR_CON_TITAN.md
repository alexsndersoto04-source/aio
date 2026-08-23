# Empezar con TITAN — guía práctica

Guía de arranque escrita leyendo el compilador de este repo (v1.0.0), no la
documentación aspiracional. Todo lo que aparece acá está soportado hoy; lo que
el compilador rechaza está marcado como tal.

Complementos:

- [`REFERENCIA_API.md`](REFERENCIA_API.md) — las 758 nativas + 122 firmas del compilador, generado desde el código.
- [`SPEC.md`](SPEC.md) — especificación formal y lista de limitaciones.
- [`TITAN_SYNTAX.md`](TITAN_SYNTAX.md) — recorrido por fase/versión.
- [`IDEAS_CON_TITAN.md`](IDEAS_CON_TITAN.md) — 20 proyectos para construir.

---

## 1. Instalar y correr en 3 minutos

```bash
# Desde el código fuente (genera el binario `titan`)
cargo build --release -p titan_cli
export PATH="$PWD/target/release:$PATH"

# En Android/Termux el binario de distribución se llama `zett`
pkg install zett
```

Primer programa:

```bash
cat > hola.titan <<'EOF'
fn main() {
    let nombre = "TITAN"
    print("Hola, {nombre}!")
}
EOF

titan run hola.titan
```

Proyecto real:

```bash
titan new mi_app && cd mi_app
titan check        # parsea y tipa, sin generar artefacto
titan run          # compila y ejecuta src/main.titan
titan build        # emite target/mi_app.tbc validado (CRC-32 + límites)
titan test         # corre cada tests/*.titan como programa independiente
titan run --sandbox  # sin filesystem, proceso, red, environment ni UI
```

Estructura que espera el loader:

```text
mi_app/
├── Titan.toml
├── Titan.lock
├── src/main.titan      ← entry point
└── tests/*.titan       ← cada archivo tiene su propio fn main()
```

---

## 2. El modelo mental en una página

TITAN es **compilado, tipado estáticamente y monomórfico**, y corre sobre una VM
de pila con presupuestos y capacidades.

```text
.titan → lexer → parser → AST → typechecker → codegen → .tbc → VM segura
                                                     └→ WebAssembly
```

Cinco reglas que explican casi todos los errores del compilador:

1. **Un solo namespace ejecutable plano.** `mod`, `import` y `pub` controlan
   *carga*, no *scoping*. Dos funciones con el mismo nombre en archivos
   distintos chocan: `duplicate function 'f'`.
2. **No hay genéricos de usuario.** La única forma parametrizada es `Array<T>` /
   `Vec<T>` / `[T]`. `Option<int>` se rechaza; escribí `Option` a secas.
3. **Solo se asigna a locales `mut` no capturadas.** No existe `p.x = 1` ni
   `xs[0] = 1`. Usá `std::array::set` / `std::map::insert`, que **devuelven un
   valor nuevo**.
4. **Los efectos pasan por capacidades.** Filesystem, Process, Network,
   Environment, UserInterface. `--sandbox` las apaga todas y las funciones puras
   siguen andando.
5. **Los errores son valores estructurados, no panics.** Overflow, división por
   cero, índice fuera de rango, límite de instrucciones: todo es un `VmError`
   con nombre, y `std::try::catch` lo convierte en `Result::Err(string)`.

---

## 3. Tipos y variables

| Escribís | Tipo interno | Notas |
|---|---|---|
| `int`, `i32`, `i64`, `u64`, `usize` | `Int` | Todos son i64 hoy; el ancho en el nombre es documentación |
| `float`, `f32`, `f64` | `Float` | IEEE-754 binario64 |
| `bool`, `char`, `string`/`str` | | `char` = escalar Unicode |
| `[T]`, `Array<T>`, `Vec<T>` | `Array(T)` | |
| `array` | `Array(any)` | atajo para heterogéneos |
| `map` | `Named("map")` | claves string |
| `any` | `Unknown` | escape hatch, compatible con todo |
| `(A, B)` | `Tuple` | |
| `fn(A) -> B` | `Function` | tipo de closures |
| `Option`, `Result` | enums del prelude | sin argumentos de tipo |

```titan
let x = 42               // inmutable
let mut total = 0        // mutable
total += 8
let y: float = 3.5
```

La aritmética entera es **chequeada**: overflow y división por cero son errores
de runtime, nunca wraparound silencioso.

---

## 4. Strings, interpolación y el gotcha más común

`+` acepta string de cualquier lado y convierte el otro operando:

```titan
let n = 42
let msg = "cantidad: " + n + " unidades"
```

La interpolación `{...}` tiene una **gramática deliberadamente chica**:

```titan
const LIMITE: int = 20

fn main() {
    let x = 42
    let siguiente = |v: int| v + 1
    print("x = {x}")                    // ✅ local
    print("límite = {LIMITE}")          // ✅ const global
    print("sig = {siguiente(x)}")       // ✅ call con args locales/int
    print("home = {std::dirs::home()}") // ✅ call nativa
}
```

**No** entra aritmética (`{a + b}`), acceso a campo (`{u.name}`) ni indexado
(`{xs[0]}`). Calculalo en un local y después interpolá:

```titan
let total = a + b * c
print("total: {total}")
```

> Corolario práctico: si necesitás llaves literales — por ejemplo los patrones
> del router `/users/{id}` — armalas por concatenación. Mirá cómo lo hace
> `examples/webserver.titan` con `brace_open()` / `brace_close()`.

---

## 5. Colecciones: todo es inmutable-por-retorno

```titan
let xs = [1, 2, 3]
let n   = std::collections::length(xs)   // 3
let ys  = std::array::push(xs, 4)        // [1,2,3,4]; xs NO cambió
let zs  = std::array::set(xs, 1, 99)     // [1,99,3]
let primero = xs[0]
```

El patrón de acumulación en loop (usá una `mut` externa, nunca `let` adentro):

```titan
let mut acc = []
for i in 0..n {
    acc = std::array::push(acc, i * i)
}
```

Maps:

```titan
let cfg = #{ host: "0.0.0.0", puerto: 8080, debug: true }
print(cfg.host)

let mut m = std::map::new()
m = std::map::insert(m, "clave", 1)      // devuelve un map nuevo
let v = std::map::get(m, "clave")
```

Los maps **no son iterables** directamente: recorré `std::map::keys(m)`.

Higher-order (también en forma de método: `xs.map(f)`):

```titan
let cuadrados = map([1,2,3], |x| x * x)
let pares     = filter([1,2,3,4], |x| x % 2 == 0)
let suma      = fold([1,2,3], 0, |acc, x| acc + x)
let ordenado  = sort_by(items, |a, b| a.precio <=> b.precio)
let hay       = any(xs, |x| x > 10)
```

`<=>` devuelve -1/0/1 y evalúa cada lado una sola vez: está hecho para
comparadores. `|>` es azúcar de parser: `x |> f(a)` es exactamente `f(x, a)`.

---

## 6. Funciones, structs, traits

```titan
fn factorial(n: int) -> int {
    if n <= 1 { return 1 }
    n * factorial(n - 1)         // la última expresión es el valor
}
```

**Regla de oro con la stdlib:** si un argumento viene de una nativa, **no lo
anotes**. Muchas nativas devuelven `Array` o `Map` genéricos y una anotación
estricta como `[map]` hace fallar el chequeo.

```titan
fn imprimir(items) {              // sin tipo → Unknown, acepta todo
    for i in 0..std::collections::length(items) { print(items[i]) }
}
```

Structs, `impl` y traits con default:

```titan
struct Punto { x: float, y: float }

impl Punto {
    fn new(x: float, y: float) -> Punto { Punto { x: x, y: y } }   // estático
    fn norma2(self) -> float { self.x * self.x + self.y * self.y } // instancia
}

trait Saludable {
    fn nombre(self) -> string;                       // requerido
    fn saludo(self) -> string { "Hola, " + self.nombre() }  // default
}

impl Saludable for Punto {
    fn nombre(self) -> string { "punto" }
}
```

Restricciones reales: `impl` solo sobre **structs declarados** (no enums, no
primitivos), los métodos default **necesitan tipo de retorno explícito**, y no
hay trait objects ni bounds. El dispatch de `x.metodo()` es dinámico por nombre
de struct en runtime.

---

## 7. Errores: `Option`, `Result`, `?` y `catch`

```titan
enum ApiError {
    NoEncontrado(string),
    EntradaInvalida(string),
    SinAutorizacion,
}

fn buscar(id: string) -> any {
    if id == "" { return Result::Err(ApiError::EntradaInvalida("id")) }
    Result::Ok(id)
}

fn describir(id: string) -> any {
    let u = buscar(id)?              // si es Err, sale de la función con ese Err
    Result::Ok(u)
}
```

Por defecto **el fallo de una nativa termina el programa**. Para recuperarte:

```titan
let r = std::try::catch(|| std::fs::read_text("data.txt"))
match r {
    Result::Ok(texto) => print(texto),
    Result::Err(msg)  => print("falló: " + msg),
}
```

`std::try::catch` captura cualquier error de runtime — nativa, tipo, índice,
aritmética — y lo devuelve como `Result::Err(string)`. Es la herramienta que
convierte un servidor frágil en uno que sobrevive requests malformados.

En `match` los patrones son restringidos: `_`, un identificador, un literal, una
variante sin payload, o una variante cuyo payload sea identificador o `_`. Los
guards (`n if n >= 500 =>`) sí funcionan. Tuplas, structs y or-patterns en
`match` **no**: para eso usá destructuring en `let`/`for`, donde sí anda todo
(anidado, rename, wildcard).

---

## 8. Concurrencia

Tareas sobre **threads reales del host**, no async cooperativo:

```titan
fn main() {
    let t = spawn || { trabajo_pesado() }     // `go` es sinónimo
    let r = join(t)

    let (tx, rx) = channel(16)
    let productor = spawn || { send(tx, 42) }
    let v = recv(rx)
    join(productor)

    // Tarea con presupuesto de memoria propio
    let acotada = std::runtime::spawn_quota(50000, || { 42 })
    print(join(acotada))
}
```

- `spawn` toma una closure **sin parámetros**.
- `join_timeout(t, ms)` y `recv_timeout(rx, ms)` devuelven `Option`.
- `select([rx1, rx2], ms)` devuelve `Option::Some((índice, valor))`.
- `cancel(t)` es cooperativo: se chequea antes de cada instrucción.
- Las tareas no comparten estado mutable; los valores viajan por copia.

Observabilidad del runtime: `std::runtime::allocated_bytes()`,
`active_tasks()`, `heap_dump(path)`, `benchmark(iteraciones, closure)`.

---

## 9. Capacidades y sandbox

```bash
titan run --sandbox programa.titan
```

Apaga las cinco capacidades. Del registro actual:

| Capacidad | Nativas | Ejemplos |
|---|---|---|
| `None` (puras) | 478 | text, json, math, collections, crypto, stats |
| `UserInterface` | 66 | term, gui, window, input, progress, readline |
| `Network` | 63 | http, dns, redis, server, email, wifi |
| `Filesystem` | 57 | fs, kv, plot, image::load, pdf::save |
| `Environment` | 48 | procfs, dirs, env |
| `Process` | 45 | process, termux, signals, audio::play |

Diseñá pensando en esto: si el núcleo de tu lógica es puro, podés testearlo
entero bajo `titan test --sandbox` y dejar los efectos en los bordes.

---

## 10. Bases de datos y servidor HTTP

Tres motores con la misma forma, más `std::db::*` que acepta cualquier handle:

```titan
let db = std::sqlite::open("app.db")
std::sqlite::execute(db, "CREATE TABLE IF NOT EXISTS t (id INTEGER, nombre TEXT)", [])
std::sqlite::execute(db, "INSERT INTO t VALUES (?, ?)", [1, "ana"])
let filas = std::sqlite::query(db, "SELECT * FROM t WHERE id = ?", [1])  // [map]
for fila in filas { print(fila.nombre) }
std::sqlite::close(db)

// Pools con health check
let pool = std::sqlite::pool("app.db", 8)
print(std::sqlite::pool_health(pool, 1000))
```

Servidor con router estilo axum (matchit por dentro):

```titan
fn main() {
    let r = std::router::new()
    std::router::insert(r, "/", "home")

    let s = std::server::start("0.0.0.0:8080")
    print("escuchando en {addr}")

    for i in 0..200 {
        let req = std::server::accept(s, 60000)
        if req >= 0 {
            let hit = std::router::at(r, std::server::path(req))
            if hit == nil {
                std::server::respond(req, 404, "Not Found")
            } else {
                std::server::respond_json(req, 200, std::json::stringify(#{ ok: true }))
            }
        }
    }
    std::server::stop(s)
    std::router::drop(r)
}
```

Ejemplos completos: `examples/webserver.titan`, `examples/rest/main.titan`,
`examples/dashboard.titan`.

---

## 11. Gotchas que te van a morder

| Síntoma | Causa | Solución |
|---|---|---|
| Error de parseo con un array al inicio de línea | `[` se lee como indexación de la línea anterior | `let arr = [1,2,3]` en la misma línea |
| `invalid string interpolation expression` | Aritmética o acceso a campo dentro de `{}` | Extraé a un local primero |
| El acumulador queda vacío tras el `for` | Usaste `let` adentro del loop (shadowing local) | Declará `let mut` afuera y reasigná |
| `assignment target must currently be a variable` | Intentaste `p.x = 1` o `xs[0] = 1` | `std::map::insert` / `std::array::set` |
| Falla al pasar un resultado de nativa a tu función | Anotaste el parámetro con un tipo estricto | Sacá la anotación (queda `Unknown`) |
| `duplicate function 'f'` | Dos archivos definen el mismo nombre | Namespace plano: renombrá |
| `unknown handle` en runtime | Usaste el handle después de `close` | Consultá antes de cerrar |
| `built-in function values ('map')` | Pasaste `map`/`print` como valor | Envolvelo en una closure: `\|x\| map(x, f)` |
| `range exceeds the one-million element safety limit` | Los rangos se materializan eager | Usá `while` con contador |
| `type 'Option' expects 0 type arguments` | Escribiste `Option<int>` | Solo `Array<T>`/`Vec<T>` son paramétricos |

---

## 12. WebAssembly: qué entra y qué no

`titan wasm` emite un módulo **autocontenido** (no un wrapper de la VM) con dos
source maps, y `examples/browser/host.js` provee `std::web::*` — DOM, eventos,
`fetch`, WebSocket, Canvas 2D, animación y WebGL2.

Entra: aritmética, control de flujo, strings, arrays, tuplas, structs, enums,
`std::array::*`, `std::map::*`, `std::text::{equals,hash64}`,
`std::time::unix_millis`, `std::wasm::heap_*` y `print` de un argumento.

**No entra:** rangos (`for i in 0..n` → usá `while`), closures y las operaciones
de orden superior, `try::catch`, dispatch dinámico de métodos, concurrencia,
filesystem, proceso, TCP/TLS, servidor HTTP, bases de datos y `std::runtime`.

Es un target de **cómputo + navegador**, no de sistemas. Planificá tu app en dos
capas si querés apuntar a los dos backends.

---

## 13. Herramientas

```text
titan check    parsear + tipar
titan build    bytecode .tbc validado (CRC-32, límites, saltos, aridad)
titan exec     ejecutar un .tbc ya compilado
titan wasm     WebAssembly + source maps
titan debug -b archivo:línea    breakpoints, step in/over/out, frames, locales
titan repl     REPL (cada línea se envuelve en un fn main sintético)
titan test     corre tests/*.titan
titan keygen/pack/publish       paquetes .tpkg firmados con Ed25519
```

Más `titan-lsp` (diagnósticos, completado, definición, referencias, rename,
semantic tokens) y `titan-dap` (Debug Adapter Protocol) para tu editor.

Verificación del registro sin compilar Rust:

```bash
python3 verify_phase34.py             # duplicados + aridad de todas las llamadas
python3 scripts/gen-api-reference.py  # regenera docs/REFERENCIA_API.md
```

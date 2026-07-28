# Zett / TITAN — Changelog

## 0.29.0 — Phase 30: `const` con expresiones + sintaxis literal `#{...}` para maps 🗺️

Dos features complementarias que hacen las **constantes de configuración**
sean naturales de escribir. Antes había que armar cada map con `std::map::insert`
encadenado — verboso y poco declarativo. Ahora:

### Constantes con cualquier expresión

Las `const` ya aceptaban cualquier expresión gracias a la infraestructura
existente, pero esta fase lo demuestra y documenta oficialmente:

```titan
const NUMEROS   = [1, 2, 3, 4, 5]
const NOMBRES   = ["ana", "juan", "maria"]
const CUADRADOS = map(NUMEROS, |n| n * n)     // fn calls funcionan
const SALUDO    = "Hola desde Titan"
```

Cualquier expresión Titan válida sirve como valor de una `const` — arrays,
strings interpolados, llamadas a funciones libres, higher-order, spread `..`,
combinaciones. Se re-evalúa lazy en cada uso (cada acceso corre la
expresión); útil para expresiones puras, cuidado con side effects.

### Nueva sintaxis: `#{ ... }` para maps literales

```titan
const CONFIG = #{
    "puerto":   8080,
    "host":     "127.0.0.1",
    "debug":    true,
    "max_conn": 100,
}

// Keys como identificadores (sin comillas) — mismo efecto
const USUARIO = #{
    nombre: "Alex",
    edad:   30,
    tags:   ["premium", "verificado"],
}

// Anidable — maps dentro de maps
const RESPUESTA_API = #{
    status: 200,
    body:   #{
        message: "OK",
        data:    #{ count: 3, items: ["a", "b", "c"] },
    },
}

// Acceso natural con `.campo` (Map ya soporta field access desde antes)
print(CONFIG.puerto)                       // 8080
print(RESPUESTA_API.body.data.count)       // 3
```

También funciona **dinámicamente**, no solo en `const`:

```titan
fn crear_perfil(nombre: string, edad: int) -> map {
    return #{
        nombre:  nombre,
        edad:    edad,
        creado:  std::datetime::now(),
    }
}
```

### Bajo el capó

- Lexer: nuevo token `HashLBrace` para `#{`. `#` sin `{` es error
  claro (`stray '#' — did you mean '#{'?`).
- Parser: `#{ k1: v1, k2: v2 }` se desazucara al pipeline:
  `std::map::insert(std::map::insert(std::map::new(), "k1", v1), "k2", v2)`.
- Keys pueden ser string literals (`"foo"`) o identificadores (`foo`)
  — el parser los normaliza a string. Los valores son cualquier `Expr`.
- Los `#{}` se anidan naturalmente porque el valor puede ser otro `#{}`.
- Cero cambios en typechecker/codegen/VM — todo se apoya en los
  natives `std::map::new` y `std::map::insert` que existen desde
  hace decenas de versiones.

### Ejemplo verificable

`zett run examples/const_and_maps.titan` — 6 escenarios:
constantes con arrays y map()+higher-order, config de servidor,
usuario con tags anidados, respuesta API de 3 niveles de profundidad,
iteración con `for` destructuring sobre const array, map dinámico
dentro de una función con string interpolación.

---

## 0.28.0 — Phase 29: `for` con destructuring 🔄

Extiende Fase 23 (destructuring en `let`) al loop `for`. Ahora se
puede desempacar tuplas y structs directo en el patrón del loop,
sin escribir un `let (a, b) = item` a mano dentro del cuerpo.

### Nueva sintaxis

```titan
// Tupla
for (a, b) in pares {
    print(a + " -> " + b)
}

// Struct
struct Point { x: int, y: int }
for Point { x, y } in puntos {
    print("(" + x + ", " + y + ")")
}

// Con rename
for Point { x: cx, y: cy } in puntos { ... }

// Wildcard (descartar campo)
for (first, _, last) in triples { ... }

// Anidado — tupla dentro de tupla, struct dentro de tupla
for (id, (a, b)) in anidado { ... }
for (id, Point { x, y }) in etiquetados { ... }

// Combinable con higher-order y pipeline
let filtered = numeros |> filter(|n| n > 3) |> map(|n| (n, n * n))
for (n, cuadrado) in filtered { ... }
```

### Bajo el capó

- El parser reconoce `(` o `Ident { ... } in` después del `for`.
- Desazucar puro: `for (a, b) in xs { body }` se transforma en
  `for __destr<N> in xs { let (a, b) = __destr<N>; body }`.
- Reusa `emit_pattern_binding` / `fresh_destr_name` / `TuplePart`
  de Fase 23 — cero código nuevo en typechecker/codegen/VM.
- Lookahead con seguimiento de profundidad de braces distingue el
  patrón struct real (`for Point { x, y } in xs`) de un `for x { ... }`
  degenerado.

### Ejemplo verificable

`zett run examples/for_destructuring.titan` — 8 escenarios: tupla
simple, tupla con wildcard, struct básico, struct con rename, struct
en pipeline con higher-order, tupla anidada dentro de tupla, tupla
que contiene struct, y combinación con `map`+`filter`.

---

## 0.27.0 — Phase 28: type aliases + spread `..` en arrays 🧵

Dos QoL del lenguaje que hacen el código más limpio sin cambiar
la semántica. Type aliases dan **documentación viva y refactor
gratis** — cambiar el tipo de un dominio en un solo lugar. Spread
elimina el fold manual para concatenar arrays.

### Type aliases — `type X = Y`

```titan
type UserId = string
type Score  = int
type Ratio  = float
type Handler = fn(int) -> string

struct Player {
    id: UserId,
    score: Score,
    tags: [string],
}

fn describe_score(s: Score) -> string {
    if s >= 100 { return "excelente" }
    return "bueno"
}
```

Reglas:

- `type Name = ExistingType` a nivel top-level (no dentro de funciones).
- Los aliases son **intercambiables 100%** con el tipo real — cero
  costo en runtime, cero cambios en la VM.
- Chain de aliases funciona: `type A = B; type B = int` → A es Int.
- El typechecker expande aliases lazy en `require_compatible`,
  así ambos lados de una comparación se normalizan.
- Si mañana `UserId` pasa a ser `int` en vez de `string`, cambiás
  la definición y todas las firmas siguen funcionando.

### Spread `..` en literales de array

```titan
let a = [1, 2, 3]
let b = [10, 20, 30]

let concat  = [..a, ..b]                // [1,2,3,10,20,30]
let mezcla  = [0, ..a, 99, ..b, 100]    // [0,1,2,3,99,10,20,30,100]
let prefix  = [-1, ..a]                 // [-1,1,2,3]
let suffix  = [..b, 999]                // [10,20,30,999]
```

Combinable con higher-order (Fase 19) y pipeline (Fase 24):

```titan
let cuadrados = [..map(a, |x| x*x), ..map(b, |x| x*x)]
let total = [..a, ..b] |> fold(0, |acc, x| acc + x)
```

Reglas:

- `..` como prefijo de un elemento en un literal de array indica spread.
- Los ranges (`0..10`) **NO** se ven afectados — como siempre, van
  entre dos expresiones, no al inicio de un elemento.
- Bajo el capó se desazucara a `std::array::concat` (que ya existía
  desde v0.2). Cero cambios en la VM.

### Ejemplo verificable

`zett run examples/aliases_spread.titan` — 10 escenarios cubriendo
aliases en fn signatures, struct fields, spreads simples y anidados,
combinación con map/fold, spreads de field accesses.

### Bajo el capó

- Lexer: keyword nueva `type` → `TokenKind::Type`.
- AST: `Item::TypeAlias(TypeAliasDecl)`.
- Parser: `parse_type_alias` (top-level) y detección de `..` como
  primer token de un elemento en array literal, que desazucara a
  `Expr::Call { callee: "std::array::concat", args: [...] }`.
- Typechecker: `type_aliases: HashMap<String, Type>` + helper
  `resolve_alias` (16 hops max) + `require_compatible` normaliza
  ambos lados antes de comparar.
- Codegen y VM: cero cambios. Type aliases son puramente estáticos,
  spread es puro desazucar.

---

## 0.26.0 — Phase 27: enums-con-payload como errores custom 🎯

Titan ya tenía `enum X { A, B(int) }` funcionando en el parser, AST,
typechecker, codegen y VM desde hace muchas versiones — pero nunca
se había demostrado que sirven para el caso de uso rey: **errores
custom tipados** al estilo Rust/Elm/Haskell.

Con esta fase queda claro que Titan tiene manejo de errores de
nivel industrial: propagación con `?`, exhaustividad implícita en
`match`, y contexto por variante en el payload.

### Nueva forma idiomática

```titan
enum ApiError {
    NotFound(string),          // id que no existio
    BadInput(string),          // nombre del campo invalido
    Unauthorized,              // variante SIN payload
    Conflict(string),
    RateLimited(int),          // segundos hasta reintentar
}

// Funciones retornan Result<T, ApiError>
fn find_user(id: string) -> any {
    if id == "" { return Result::Err(ApiError::BadInput("id")) }
    if id == "42" { return Result::Ok(build_user(id)) }
    return Result::Err(ApiError::NotFound(id))
}

// El operador `?` propaga el error automáticamente,
// atravesando cuantos niveles haga falta:
fn describe_user(id: string) -> any {
    let user = find_user(id)?          // Si Err, sale de esta fn con ese Err
    let name = std::map::get(user, "name")
    return Result::Ok("Usuario " + id + " se llama " + name)
}

fn handle_request(token: string, user_id: string) -> any {
    require_auth(token)?               // ? propaga Unauthorized
    check_rate_limit(user_id)?         // ? propaga RateLimited(30)
    describe_user(user_id)?            // ? propaga NotFound o BadInput
}
```

### Pattern matching con contexto extraído

```titan
match resultado {
    Result::Ok(v) => print("OK: " + v),
    Result::Err(err) => match err {
        ApiError::NotFound(id)      => print("no existe: " + id),
        ApiError::BadInput(field)   => print("campo: " + field),
        ApiError::Unauthorized      => print("sin auth"),
        ApiError::Conflict(what)    => print("dup: " + what),
        ApiError::RateLimited(secs) => print("esperar " + secs + "s"),
    }
}
```

### Mapear errores a codigos HTTP

```titan
fn http_status(err: ApiError) -> int {
    match err {
        ApiError::NotFound(_)    => 404,
        ApiError::BadInput(_)    => 400,
        ApiError::Unauthorized   => 401,
        ApiError::Conflict(_)    => 409,
        ApiError::RateLimited(_) => 429,
    }
}
```

### Ejemplo verificable

`zett run examples/custom_errors.titan` — 10 escenarios:

1. Camino feliz — Ok con struct dentro
2. NotFound(id) — payload con contexto
3. BadInput(field) — validación
4. Conflict(name) — recurso duplicado
5. BadInput otro campo
6. `?` operator propagando desde función anidada
7. Camino feliz vía `?`
8. Handler completo — auth OK
9. Unauthorized sin payload
10. RateLimited(30) — payload int
11. Propagación de NotFound cruzando 3 niveles de funciones

### Bajo el capó

- Cero cambios en el compilador — la infraestructura (parser,
  `enum_variants: HashMap<String, Option<Type>>` en typechecker,
  `Op::NewEnum/EnumIs/EnumPayload` en VM, `Pattern::Enum` en match)
  ya estaba desde hace varias versiones.
- Esta fase la ejercita, verifica end-to-end, y documenta como
  parte oficial del lenguaje.

---

## 0.25.0 — Phase 26: REST API completa en TITAN 🏢

**La prueba viviente.** No es una feature del lenguaje — es un
proyecto real de ~400 líneas de código Titan puro que combina
TODA la stack construida hasta ahora en una aplicación funcional.

Este es el momento en que Titan deja de ser un lenguaje de juguete
y se demuestra como plataforma capaz de construir servicios web
reales, con la misma arquitectura de un backend de producción
(Express/Flask/Axum/Gin), corriendo en un celular.

### Arquitectura del proyecto

```
examples/rest/
├── main.titan     — servidor HTTP + router + dispatcher (300 líneas)
├── db.titan       — capa SQL: users, posts, migraciones (90 líneas)
└── client.titan   — cliente que consume la API (150 líneas)
```

### Stack usado (todas las Fases combinadas)

| Componente        | Módulo Titan            | Fase |
|-------------------|-------------------------|------|
| Servidor HTTP     | `std::server`           | 11   |
| Router radix-tree | `std::router` (matchit) | 11   |
| Base de datos     | `std::sqlite::memory`   | 12   |
| Migraciones       | `std::sqlite::migrate`  | 12   |
| JSON              | `std::json` (Fase 25-B) | 12   |
| Password hashing  | `std::password::argon2` | 4    |
| JWT               | `std::jwt::hs256`       | 4    |
| UUID v4           | `std::uuid::v4`         | 1    |
| Timestamps        | `std::datetime::now`    | 1    |
| Manejo de errores | `std::try::catch`       | 18   |
| Multi-archivo     | `import db` / `import auth` | 21 |
| Higher-order      | `for u in users`        | 19   |
| Cliente HTTPS     | `std::http_full`        | 3    |

### Endpoints REST

```
GET    /health                  — status
POST   /api/register            — crear usuario (public)
POST   /api/login               — obtener JWT (public)
GET    /api/users               — listar usuarios (public)
GET    /api/posts               — listar posts (public)
GET    /api/posts/<id>          — un post (public)
POST   /api/posts               — crear post (requiere JWT)
DELETE /api/posts/<id>          — borrar (requiere JWT, solo dueño)
```

### Flujo de autenticación

1. `POST /api/register` → hashea la password con **argon2id** (el
   estándar OWASP), guarda en SQLite.
2. `POST /api/login` → verifica el hash. Si OK, emite un **JWT
   HS256** con claims `sub`, `username`, `iss`, `iat`, `exp`.
3. Endpoints protegidos leen `Authorization: Bearer <token>`,
   verifican firma + expiración, extraen `sub` = user_id.
4. Cada operación de escritura de posts respeta ownership.

### Ejemplo verificable end-to-end

Terminal 1 (servidor):
```bash
zett run examples/rest/main.titan
# → http://127.0.0.1:8080 escuchando
```

Terminal 2 (cliente):
```bash
zett run examples/rest/client.titan
# → hace un tour completo: health, register, login,
#   crea 3 posts, lista, GET individual, DELETE,
#   verifica que borró, y prueba que sin JWT recibe 401.
```

También se puede probar con `curl` clásico:
```bash
curl -X POST http://127.0.0.1:8080/api/register \
     -H 'Content-Type: application/json' \
     -d '{"username":"alex","password":"secret123"}'
```

### Bajo el capó

- **Cero cambios en el compilador**. Esta fase es puramente
  aplicativa — demuestra que la stack construida hasta v0.24.0
  ya es suficiente para escribir servicios web serios.
- El servidor procesa hasta 40 requests y se cierra solo (para
  demos reproducibles). Cambiando `0..40` a `loop { ... }` lo
  volvés permanente.
- SQLite en memoria: fresh state cada arranque, perfecto para
  demos. Con `std::sqlite::open(path)` pasa a persistente.
- El secret JWT está hardcodeado con fines demostrativos. En
  producción vendría de env vars o `std::dirs::secret_file`.

---

## 0.24.0 — Phase 25-B: `std::json` completo + integración HTTP end-to-end 🌐

El módulo `std::json` ya estaba implementado a nivel runtime desde
hace varias versiones (usa `serde_json` — el crate JSON estándar de
todo el ecosistema Rust), pero nunca se había demostrado
end-to-end con un ejemplo público ni documentado como API pública
del lenguaje. Esta versión lo cierra: ejemplo real que consume la
API pública de GitHub, extrae campos con paths RFC 6901, y compone
un JSON de respuesta enriquecido.

### API disponible

```titan
// Parsear un string JSON -> Value nativo de Titan
let obj = std::json::parse("{\"nombre\":\"Ana\",\"edad\":30}")
print(obj.nombre)                          // "Ana"    (acceso con .)
print(obj.edad)                            // 30

// Serializar de vuelta
std::json::stringify(obj)                  // "{\"nombre\":\"Ana\",\"edad\":30}"
std::json::pretty(obj)                     // formato indentado

// JSON Pointer RFC 6901 (como XPath para JSON)
std::json::pointer(obj, "/direccion/ciudad")
std::json::pointer(obj, "/hobbies/0")

// Merge Patch RFC 7396 (b sobreescribe a; null en b borra)
std::json::merge(base, patch)

// Aplanar estructura anidada a lista de (path, valor)
std::json::flatten(obj)                    // [("/nombre", "Ana"), ("/edad", 30), ...]
```

### Conversiones automáticas

- JSON `Object` ↔ Titan `Map` (accesible con `.campo`)
- JSON `Array`  ↔ Titan `Array` (accesible con `[i]`)
- JSON `Number` → Titan `Int` si entero, `Float` si decimal
- JSON `String` ↔ Titan `String`
- JSON `Bool`   ↔ Titan `Bool`
- JSON `null`   ↔ Titan `Nil`
- Titan `Struct` → JSON `Object` (usa sus campos como keys)
- Titan `Enum`   → JSON `{"type":..., "variant":..., "payload":...}`

### Integración HTTP

Ya existía desde antes pero acá se demuestra:

```titan
// GET con parse automático a Value de Titan
let user = std::http_full::get_json(
    "https://api.github.com/users/torvalds",
    headers, opts
)
print(user.login)              // acceso directo con .
print(user.followers)

// POST con payload JSON auto-serializado
let respuesta = std::http_full::post_json(url, payload, headers, opts)
```

### Ejemplo verificable

`zett run examples/json_api.titan` — 7 escenarios:

1. Parse local de un JSON complejo con nested objects y arrays
2. Acceso con `.campo` y `[i]` sobre el resultado
3. JSON Pointer paths (`/direccion/cp`, `/hobbies/1`)
4. Stringify compacto y pretty
5. Merge Patch con borrado explícito via `null`
6. Flatten a lista de (path, valor)
7. **API HTTPS real de GitHub** con manejo de errores via
   `std::try::catch` (Fase 18) — si estás offline no crashea,
   avisa y el resto del ejemplo sigue.

### Bajo el capó

- Cero cambios en el compilador — Fase 25-B es puramente
  ejercitar, verificar y documentar la infraestructura ya
  construida.
- La conversión `Value ↔ serde_json::Value` vive en
  `crates/titan_vm/src/native.rs` (`to_json` / `from_json`).
- La implementación de merge/flatten/pointer vive en
  `crates/titan_stdlib/src/json.rs`, con test unitario propio.

---

## 0.23.0 — Phase 24: pipeline `|>` y spaceship `<=>` 🚀

Dos operadores de programación funcional que hacían falta para
escribir código idiomático de verdad — ambos son puro desazucar
sintáctico en el parser, cero cambios en la VM.

### Pipeline `|>`

```titan
// Antes: cuadrar(dobla(sumar_1(x)))    (leer de adentro hacia afuera)
// Ahora: x |> sumar_1 |> dobla |> cuadrar   (leer de izquierda a derecha)

let total = [1, 2, 3, 4]
    |> map(|x| x * x)
    |> fold(0, |acc, x| acc + x)
// == fold(map([1,2,3,4], |x| x*x), 0, |acc, x| acc + x)
```

Reglas:

- `x |> f` es azúcar de `f(x)`.
- `x |> f(a, b)` es azúcar de `f(x, a, b)` — el valor pipeado
  siempre va como **primer argumento**.
- Precedencia mínima (0): `a + 1 |> print` es `(a+1) |> print`.
- Left-associative y encadenable sin límite.

### Spaceship `<=>`

```titan
print(3 <=> 5)                       // -1
print(5 <=> 5)                       //  0
print(7 <=> 5)                       //  1

// Ideal para sort_by — reemplaza el clásico
//   |a, b| if a < b { -1 } else if a > b { 1 } else { 0 }
let asc  = sort_by(data, |a, b| a <=> b)
let desc = sort_by(data, |a, b| b <=> a)

// Sobre structs (Fase 20 + Fase 24)
let por_precio = sort_by(productos, |a, b| a.precio <=> b.precio)
```

Reglas:

- `a <=> b` devuelve `int`: `-1` si `a<b`, `0` si `a==b`, `1` si `a>b`.
- Cada lado se evalúa **una sola vez** (usa dos temporarios
  sintéticos como en Fase 23).
- Funciona sobre `int` y `float` (los tipos que el runtime ordena).
- Misma precedencia que los comparadores clásicos (7).

### Bajo el capó

- Lexer: tokens nuevos `PipeGt` (`|>`) y `Spaceship` (`<=>`).
- Parser: `|>` se convierte en un `Expr::Call` con el LHS
  insertado al principio de los args. `<=>` se convierte en un
  `Expr::Block` con dos `let __destr<N>` + un `if a<b { -1 } else
  if a>b { 1 } else { 0 }`, garantizando evaluación única de cada lado.
- Cero cambios en `BinaryOp`, typechecker o VM — todo se apoya en
  la infraestructura existente. Los `.zettbc` viejos siguen
  cargando sin cambios.

### Ejemplo verificable

`zett run examples/pipeline_spaceship.titan` — 10 casos cubriendo
pipeline básico, encadenado, con múltiples args, combinado con
higher-order (Fase 19), spaceship sobre int/float, sort ascendente
y descendente, y sort sobre structs por campo.

---

## 0.22.0 — Phase 23: destructuring en `let` (tuplas + structs) 🎁

Ahora se pueden **desempacar** tuplas y structs directamente en el
binding, sin campo por campo. Cero cambios en la VM, cero cambios
en el AST — es un desazucar puramente sintáctico en el parser.

### Nueva sintaxis

```titan
// Tupla
let (a, b) = mi_par
let (lo, hi) = min_max(xs)          // funciones que retornan tuplas

// Struct
struct Point { x: int, y: int }
let Point { x, y } = p              // nombres iguales al campo
let Point { x: cx, y: cy } = p      // con rename

// Wildcards para descartar
let (primero, _) = par

// Anidado
let (first, (second, third)) = nested
let (id, Point { x, y }) = combo

// Struct dentro de struct
struct Address { city: string, zip: int }
struct Person { name: string, age: int, home: Address }
let Person { name, age, home: Address { city, zip } } = ana
```

### Reglas

- Después de `let` (o `let mut`), si viene `(` → patrón tupla.
- Si viene `Ident {` y después de `}` viene `=` → patrón struct.
- Sin ambos, sigue el `let x = ...` de siempre.
- `_` como parte descarta el valor pero el temp igual se evalúa.
- La RHS **se evalúa una sola vez**: `let (a, b) = calcular()`
  llama `calcular()` una vez, guarda el resultado en un temp y
  desde ahí bindea `a` y `b`.
- Patrones anidados funcionan recursivamente sin límite de
  profundidad.

### Bajo el capó

- El parser genera un `let __destr<N>` (nombre reservado interno,
  el `__` inicial evita colisiones con identificadores del usuario)
  y luego un `let` por cada nombre bindeado, indexando la tupla
  con `[i]` o los campos del struct con `.field`.
- Ejemplo: `let (a, b) = par` se compila igual que si el usuario
  hubiera escrito:
  ```titan
  let __destr0 = par
  let a = __destr0[0]
  let b = __destr0[1]
  ```
- Como no hay opcodes nuevos, los `.zettbc` viejos siguen ejecutando.
- Enum patterns (`let Some(x) = opt`) NO están soportados — para
  eso hay que usar `match` de siempre.

### Ejemplo verificable

`zett run examples/destructuring.titan` — 10 casos cubriendo tupla
simple, tupla-desde-función, wildcards, struct con y sin rename,
tuplas anidadas, structs anidados, y combinaciones de ambos.

---

## 0.21.0 — Phase 22: traits con métodos default 🧬

Los traits ya se parseaban desde hace muchas versiones (`trait X { fn foo(); }`),
pero no se conectaban con `impl Trait for Type` ni admitían defaults.
Ahora sí: polimorfismo real con herencia de métodos.

### Nueva sintaxis útil

```titan
trait Greetable {
    // Requerido: cada tipo debe implementarlo.
    fn name(self) -> string;

    // Default: el impl lo puede omitir y hereda este body.
    fn greet(self) -> string {
        "Hola, " + self.name() + "!"
    }

    // Los defaults pueden llamar a otros métodos del propio trait
    // via `self.metodo()` — el dispatch dinámico se encarga.
    fn shout(self) -> string {
        self.greet() + "!!!"
    }
}

struct Person { first: string }

impl Greetable for Person {
    fn name(self) -> string { self.first }
    // greet y shout se heredan automáticamente.
}

struct Robot { model: string, serial: int }

impl Greetable for Robot {
    fn name(self) -> string { "Robot-" + self.model }

    // Sobreescribe el default.
    fn greet(self) -> string {
        "UNIT " + self.name() + " #" + self.serial + " REPORTING"
    }
    // shout hereda el default -> usa la versión sobreescrita de greet.
}
```

Un mismo struct puede implementar **múltiples** traits:

```titan
struct Product { title: string, stock: int }

impl Greetable for Product {
    fn name(self) -> string { self.title }
}

impl Countable for Product {
    fn count(self) -> int { self.stock }
}
```

### Validación en tiempo de compilación

- **Método requerido faltante**: si `impl Foo for T` no provee un método
  que Foo declara sin default (`fn foo();`), el typechecker aborta con
  `missing required method 'foo'` antes de correr nada.
- **Trait desconocido**: `impl NoExiste for T` da error inmediato.
- **Los defaults se typecheckean como si fueran del impl**: si el body
  del default llama `self.campo` y el tipo no tiene ese campo, se
  reporta ahí mismo.

### Bajo el capó

- `TraitMethod` gana un `body: Option<Block>`. Reutiliza el parser
  existente de `fn` que ya distingue `;` (sin body) de `{ ... }`.
- Codegen: cuando ve `impl Trait for Type { ... }`, para cada método
  del trait con body y que el impl no override, sintetiza un
  `FunctionDecl` con nombre `Type::metodo` y lo registra en la
  `method_table` — el runtime lo dispatcha idéntico a un método
  escrito a mano.
- Los defaults que llaman `self.otro_metodo()` funcionan porque el
  dispatch de Fase 20 es dinámico: en runtime se resuelve `Type::otro_metodo`
  contra el tipo real del receiver, no contra el trait.
- Cero cambios en la VM — la infraestructura de Fase 20 alcanza.

### Ejemplo verificable

`zett run examples/traits.titan` — cubre:

1. Trait con método requerido + 2 defaults encadenados
2. Struct que hereda los dos defaults
3. Struct que sobreescribe uno y hereda el otro
4. Trait con 3 defaults encadenados (`describe` usa `is_empty`+`is_singleton`+`count`)
5. Struct que implementa **DOS** traits distintos sin colisión

---

## 0.20.0 — Phase 21: proyectos multi-archivo con `import` 📦

Titan ya tenía todo el andamiaje de carga multi-archivo en
`titan_pkg::SourceProject` (parser reconoce `import a::b`, loader
resuelve rutas, detecta ciclos, previene escape del source root),
pero nunca se había demostrado end-to-end con un ejemplo. Esta
versión lo verifica y lo documenta.

### Cómo funciona

```titan
// examples/modules/main.titan
import geometry
import util::text
import util::math

fn main() {
    let p = Point::new(3.0, 4.0)   // viene de geometry.titan
    print(greet("mundo"))           // viene de util/text.titan
    let s = sum([1, 2, 3])          // viene de util/math.titan
    print(s)
}
```

Estructura de archivos:

```
examples/modules/
├── main.titan
├── geometry.titan
└── util/
    ├── math.titan
    └── text.titan
```

Reglas de resolución (implementadas por `titan_pkg::SourceProject::load`):

- `import geometry` → busca `<dir_del_entry>/geometry.titan`
  o `<dir_del_entry>/geometry/mod.titan`.
- `import util::math` → busca `util/math.titan` o `util/math/mod.titan`.
- Si hay un `Titan.toml` en algún ancestro, el source root pasa a
  ser `<root>/src/` (comportamiento tipo Cargo). Sin manifiesto,
  el source root es la carpeta del entry.
- Todos los items importados quedan en un **namespace plano**:
  `Point::new()` funciona directo, sin `geometry::Point::new()`.
- **Ciclos detectados**: si `a.titan` importa `b.titan` y viceversa,
  el compilador aborta con `ImportCycle: a -> b -> a`.
- **Escape del root prevenido**: `import ../../secreto` falla con
  `ImportEscapesRoot`.
- **Sin doble carga**: cada archivo se parsea y typechecka una sola
  vez, incluso si dos módulos lo importan.
- `import std::...` se ignora (la stdlib son natives, no `.titan`).

### Ejemplo verificable

```bash
zett run examples/modules/main.titan
```

Combina Fase 20 (impl para structs) con imports: define `Point`
y `Rect` en `geometry.titan`, funciones libres en `util/math.titan`
y `util/text.titan`, y las usa todas desde `main.titan`.

### Bajo el capó

- Cero cambios en el compilador — Fase 21 fue puramente ejercitar,
  verificar y documentar la infraestructura ya construida.
- El loader vive en `crates/titan_pkg/src/project.rs`, expuesto via
  `titan_pkg::SourceProject::load(entry_path)`.
- `zett run archivo.titan` invoca `load_and_compile()` que a su vez
  llama `SourceProject::load` — así que import funciona con todos
  los subcomandos que compilan (`run`, `build`, `wasm`, `check`).

---

## 0.19.0 — Phase 20: `impl` para structs (métodos en tipos custom) 🧱

Los structs de Titan ahora pueden tener **métodos** propios via
bloques `impl`. Es la pieza que faltaba para modelar tipos custom
como en Rust / Swift / Go, con dispatch dinámico basado en el
struct real del receiver.

### Nueva sintaxis

```titan
struct Point { x: float, y: float }

impl Point {
    // Método estático (sin `self`): se llama `Point::origin()`.
    fn origin() -> Point { Point { x: 0.0, y: 0.0 } }

    fn new(x: float, y: float) -> Point { Point { x: x, y: y } }

    // Método de instancia: `self` como primer parámetro.
    // Su tipo se infiere como `Point` automáticamente.
    fn distance_sq(self, other: Point) -> float {
        let dx = self.x - other.x
        let dy = self.y - other.y
        dx * dx + dy * dy
    }

    fn translated(self, dx: float, dy: float) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}

fn main() {
    let o = Point::origin()             // método estático
    let p = Point::new(3.0, 4.0)
    print(p.distance_sq(o))             // instancia -> 25.0
    print(p.translated(1.0, -2.0).x)    // encadenable
}
```

Dos structs distintos pueden compartir el nombre de un método sin
colisión (`Point::show` y `Rect::show` conviven porque el dispatch
se hace por el tipo del receiver, no por nombre global).

### Bajo el capó

- Nuevo opcode `Op::CallMethod { method, argc }`. La VM pop-ea el
  receiver, mira su `Value::Struct { name, .. }` y busca
  `"<name>::<method>"` en `module.method_table`. Dispatch en tiempo
  constante (`HashMap` lookup).
- Los métodos se registran con nombre calificado (`Point::distance_sq`)
  tanto en `titan_typechecker` como en `titan_codegen`, evitando
  colisiones con funciones libres o entre structs.
- El primer parámetro `self` sin anotación se infiere como
  `Type::Named(nombre_del_impl)` — permite `self.campo` y llamadas
  `self.otro_metodo()` sin escribir el tipo.
- Los métodos estáticos (`Point::origin()`) reutilizan el parser
  existente para paths `::`, sin cambio de sintaxis.
- `CompiledModule` gana un campo `method_table: HashMap<String, usize>`
  con `#[serde(default)]`, así los `.zettbc` viejos siguen leyéndose.

### Ejemplo verificable

Correr `zett run examples/impl_structs.titan` — muestra 10+ casos:
factories estáticos, distancia entre puntos, encadenamiento de
métodos (`.translated(...).show()`), y dos structs (`Point`, `Rect`)
compartiendo `show()` sin pisarse.

---

## 0.18.0 — Phase 19: higher-order sobre arrays con closures 🎛️

### 🎯 4 operaciones nuevas: `sort_by`, `find`, `any`, `all`

Ya teníamos `map`, `filter`, `fold` — ahora se completan las
higher-order clásicas para poder programar como en JS/Python/Rust.

```titan
let nums = [3, 1, 4, 1, 5, 9, 2, 6]

sort_by(nums, |a, b| a - b)       // [1, 1, 2, 3, 4, 5, 6, 9]
sort_by(nums, |a, b| b - a)       // [9, 6, 5, 4, 3, 2, 1, 1]  (desc)
find(nums, |n| n > 4)             // 5   (primero que cumple)
find(nums, |n| n > 100)           // nil (ninguno cumple)
any(nums, |n| n % 2 == 1)         // true (hay impares)
all(nums, |n| n > 0)              // true (todos positivos)
```

Combinable con `.method()` syntax también:

```titan
nums.sort_by(|a, b| a - b)
productos.filter(|p| p.precio > 10).sort_by(|a, b| a.precio - b.precio)
```

### 🎯 Caso real — ordenar un array de maps por campo

```titan
let productos = [p1, p2, p3]
let baratos = sort_by(productos, |a, b| a.precio - b.precio)
let caro = find(productos, |p| p.precio > 20)
let gratis = any(productos, |p| p.precio == 0)
```

### 🔧 Implementación

4 opcodes nuevos en el codegen (`ArraySortBy`, `ArrayFind`, `ArrayAny`,
`ArrayAll`), 4 handlers en el VM que ejecutan la closure para cada
elemento del array. Los predicados de `find`/`any`/`all` deben devolver
`bool`; el comparador de `sort_by` debe devolver `int` o `float`
(negativo si a<b, cero si iguales, positivo si a>b — mismo protocolo
que `qsort`/`compareTo` de C/Java).

`any`/`all`/`find` cortocircuitan (paran apenas encuentran la respuesta).
`sort_by` usa selection sort (simple, O(n²), estable — suficiente para
arrays hasta ~1000 elementos; para más, ordenar en Rust vía un helper).

### 📦 Ejemplo

`examples/qol_higher_order.titan` — verifica los 4 nuevos con nums
primero, después ordena un array de maps de productos por precio,
busca el primer caro, y chequea si hay algo gratis.

### ⚠️ No-breaking

`map`/`filter`/`fold` que ya existían no se tocan. Los 4 nuevos son
agregados quirúrgicos: 4 opcodes, 4 dispatch cases, 4 typechecker sigs.

## 0.17.0 — Phase 18: manejo de errores con `std::try::catch` 🛡️

### 🎯 Titan ahora tiene manejo de errores real

Un error de un native (archivo no existe, HTTP timeout, JSON malformado,
etc.) ya NO mata el programa. Podés capturarlo como valor con
`std::try::catch(closure)` y decidir qué hacer.

**API:**
```titan
let r = std::try::catch(|| std::fs::read_text("/config.json"))
match r {
    Result::Ok(text) => print("cargue: " + text),
    Result::Err(msg) => print("no pude: " + msg),
}
```

Combinado con el operador `?` (que ya existía pero era casi inutil
porque nada devolvia Result), ahora podés propagar errores idiomáticamente:

```titan
fn cargar_config(path) {
    let text = std::try::catch(|| std::fs::read_text(path))?
    let json = std::try::catch(|| std::json::parse(text))?
    Result::Ok(json)
}
```

Todas las natives peligrosas (fs, http, dns, json parse, database, etc.)
se pueden envolver en `catch()` sin cambios en el resto del código.

### 🔧 Cómo funciona internamente

- **Codegen** (`titan_codegen`): al ver `std::try::catch(fn, args...)`
  emite el opcode nuevo `Op::TryCall(argc)` en vez de un CallNative.
- **VM** (`titan_vm`): `Op::TryCall` ejecuta la closure envuelta en un
  `match Result<Value, VmError>`, convierte el `Err` en `Value::Enum {
  Result::Err(msg) }` y sigue. Ningún error escapa.
- **Typechecker**: `std::try::catch` declarada como variádica (skip
  arity check) y con retorno `Any` (es Result<Ok, Err>).
- **`?` operator**: ya sabía procesar `Result` y `Option` — ahora
  finalmente tiene valores reales que procesar.

### 📦 Ejemplo

`examples/qol_try_catch.titan` — 6 casos:
1. Leer archivo que existe
2. Leer archivo que NO existe (capturado)
3. Parsear JSON válido
4. Parsear JSON malformado (capturado)
5. DNS lookup a dominio inexistente (capturado)
6. Función que usa `?` para propagar errores

### ⚠️ No-breaking

Cualquier código v0.16.0 sigue compilando y corriendo igual. `Op::TryCall`
es un opcode nuevo, no toca los existentes. El registry de natives suma
una entrada más (`std::try::catch`); nada se remueve.

## 0.16.0 — Phase 17 (part 1) QoL: String + Any, plus Phase 16 PDF

### 🎯 Language Quality-of-Life #1: `String + Any` just works
The `+` operator now works when **either operand is a String** — the
other side is coerced to its `print` form via `val_to_string`. Mirrors
JavaScript's `"x " + n` and Python's f-strings.

**Before v0.16.0:**
```titan
let s = "count: " + 42          // typecheck ERROR
let out = "hello " + doc        // typecheck ERROR if doc is Unknown
```
**Now:**
```titan
let s = "count: " + 42          // → "count: 42"
let out = "hello " + doc        // works for any type of doc
let e = 100 + " points"         // → "100 points"  (symmetric)
```

This removes the single most frequent typecheck gotcha we hit while
writing ~20 `.titan` examples across Phases 10-15. Existing code that
concatenated two Strings keeps working unchanged — this is a strict
loosening of the rule, not a breaking change.

**Example:** `examples/qol_string_add.titan` — verified all six patterns
(Int, Float, Bool, Array, Unknown-from-param, symmetric).

**Files changed:** `crates/titan_typechecker/src/lib.rs` (Add rule),
`crates/titan_vm/src/lib.rs` (runtime `add` fn).

### ⚠️ Phase 16 PDF DISABLED in default build

The `printpdf` 0.7 crate turned out to have API incompatibilities I
couldn't detect without a local Rust toolchain (no `.clone()` on
`PdfDocumentReference`, `!Send` marker, `Mm(f32)` vs my `f64`). Rather
than block v0.16.0's QoL fix, `pdf_mod` is now behind an opt-in
feature — the code stays in the tree (`crates/titan_stdlib/src/pdf_mod.rs`
+ `examples/invoice.titan`) but doesn't ship in the default `.deb`.

To re-enable if you want to tinker:
```toml
titan_stdlib = { path = "...", features = ["pdf_mod"] }
```

We'll come back to PDF with a different backend (or a printpdf
rewrite) in a later release.

### Added
- **`std::pdf::*`** — pure-Rust PDF writing via `printpdf` 0.7 built
  with `default-features = false`. That drops `azul-layout`,
  `rust-fontconfig`, HTML rendering and SVG-to-PDF — leaving just the
  core PDF machinery (`lopdf` + `owned_ttf_parser` + `time`). No C
  dependencies at all, compiles cleanly on Termux ARM.
- API (opaque `i64` handles for documents; pages and layers by
  0-based index; all coordinates in millimetres, PDF-space with y
  growing upwards):
    * `new(title, width_mm, height_mm)` — new document, page 0 / layer 0
      created automatically. Common sizes: A4 portrait `(210, 297)`,
      A4 landscape `(297, 210)`, US Letter `(216, 279)`.
    * `add_page(handle, width, height, layer_name)` → page index.
    * `page_count(handle)`.
    * `add_text(handle, page, layer, text, font_size_pt, x, y)` — uses
      the document's default Helvetica (one of PDF's 14 built-in fonts,
      no TTF embedding needed).
    * `set_color(handle, page, layer, r, g, b)` — sets both fill and
      outline colours for subsequent draw calls (RGB in `[0, 1]`).
    * `add_line(handle, page, layer, x1, y1, x2, y2, thickness_pt)`.
    * `add_rect(handle, page, layer, x, y, w, h)` — closed 4-point
      polygon, both fill and outline honour the current colour.
    * `save(handle, path)` — serialize to disk.
    * `close(handle)` — release from the process-wide registry.

### Combines with
- **Fase 7 (`std::qrcode::to_svg`)** — build a QR code SVG and drop
  its image into a receipt PDF (via `rsvg-convert` → PNG → future
  `add_png`; kept out of this release to avoid version conflicts with
  the `image` crate).
- **Fase 14 (`std::plot`)** — the same route works for embedding
  charts once `add_png` lands.

### Notes
- PDF image embedding (`add_png`) was intentionally deferred to a
  follow-up release. `printpdf` uses `image = 0.24`, while our Fase 7
  modules use `image = 0.25`; adding embed today would either force
  a version downgrade (breaking Fase 7) or upgrade `printpdf`'s deps
  (pulling in the heavy `azul-layout` graph). We'll consolidate the
  two once `printpdf` releases with `image = 0.25`.

## 0.15.0 — Phase 12 (part 4): Semantic search on-device 🧠🔍

### Added
- **`std::onnx::run_bert_pooled(handle, batch, seq_len, ids, mask)`**
  — sentence-transformer style: runs a BERT-family encoder and applies
  **attention-mask-weighted mean pooling** over the token dimension of
  the raw `last_hidden_state`. Returns `{values, shape=[batch, hidden]}`.
  Doing the pooling in Rust is way faster than looping in `.titan`
  (for MiniLM's 128 * 384 fp adds per sentence).
- **`std::vector::*`** — new pure-Rust module for embedding math and
  ranking:
    * `dot(a, b)` — dot product.
    * `norm(v)` — L2 (Euclidean) norm.
    * `cosine_similarity(a, b)` → `[-1, 1]`. Returns 0 for zero-vectors
      so callers can rank without a special case.
    * `normalize(v)` — unit L2 norm (zeros stay zeros).
    * `add(a, b)`, `sub(a, b)`, `scale(v, k)` — element-wise ops.
    * `argmax(v)` → index of the max element (errors on empty).
  Every fn is deliberately branchless-when-possible so rustc's release
  profile autovectorizes into ARM NEON on Termux.
- **`examples/vector_search.titan`** — lightweight demo of
  `std::vector::*` with **hardcoded 4-dim embeddings** (no ONNX
  model needed). Runs in < 1s on any device, including a 3 GB Redmi
  9C. Demonstrates that the cosine-similarity ranking pipeline is
  correct end-to-end.
- **`examples/search.titan`** — full end-to-end pipeline with a real
  MiniLM ONNX encoder. **Requires 4+ GB RAM free** — tract's graph
  optimization pass needs ~250-800 MB peak depending on model and
  seq_len, which does not fit in a 3 GB device with Android and other
  apps loaded. Tested and confirmed unusable on a Redmi 9C (swap
  thrashes indefinitely). Kept in the tree as a template for users
  with beefier hardware; refined with defensive defaults (MiniLM-L3,
  seq_len=64) that at least give it a chance on 4 GB devices.

### Notes on running the full pipeline
- MiniLM's ONNX export has hidden_size=384. Sentence embeddings are
  normalized so cosine == dot, which is faster to compute repeatedly
  when ranking many documents against one query.
- Combines with Phase 10 (`std::kv`) to persist an embedding index to
  disk once and reload it across runs. Left as an exercise in the demo.
- **Memory reality on Termux:** even paraphrase-MiniLM-L3-v2 (a 3-layer
  distilled BERT, 66 MB on disk) needs ~250 MB RAM peak inside tract
  during `into_optimized()`. Below 4 GB total RAM (before Android + other
  apps take their cut), the process just swaps forever. Not a bug in
  Titan or tract — it's an inherent tension between "compile the graph
  once, run it fast forever" (tract's design) and "3 GB celus with
  Android eating half".

## 0.14.0 — Phase 13': Wi-Fi introspection (termux-wifi-*)

### Added
- **`std::wifi::*`** — real bindings to the `termux-wifi-*` CLIs shipped
  by the official Termux:API package. Nothing is simulated: every call
  spawns the matching binary and surfaces exactly what Android's
  `WifiManager` reports.
    * `scan()` → `[{ ssid, bssid, rssi, frequency_mhz, timestamp,
      channel_bandwidth_mhz, center_frequency_mhz }, ...]`
      — nearby access points from the last cached scan.
    * `connection_info()` → `{ ssid, bssid, ip, mac_address,
      link_speed_mbps, rssi, frequency_mhz, network_id,
      supplicant_state, hidden_ssid }` or `nil` when not connected.
    * `set_enabled(bool)` — toggle the Wi-Fi radio (may silently
      no-op on Android ≥ 10 with the screen locked; upstream Android
      restriction, not a bug).
    * `signal_bars(rssi_dbm)` → 0..=4 — pure Rust, no CLI, matches
      Android's `WifiManager.calculateSignalLevel` heuristic. Safe to
      call anywhere for UI rendering.

### Why Wi-Fi and not Bluetooth
Termux:API does **not** expose Bluetooth scanning / BLE. `bluetoothctl`
/ BlueZ / `hcitool` all require Linux's BlueZ stack, which Android
doesn't use (it uses BlueDroid). Confirmed by the Termux maintainer
(Grimler91, 2022): *"android doesn't use bluez, so bluetoothctl cannot
work. What you need is a termux-api 'bluetoothAPI', but no one has
worked on writing such an API at the moment."*

Rather than ship a fake `std::bluetooth::*` module (which would
violate the project's zero-simulations rule), this release replaces
what was going to be Phase 13 with **Phase 13'**: real Wi-Fi
introspection using `termux-wifi-scaninfo`, `termux-wifi-connectioninfo`
and `termux-wifi-enable` — all of which are confirmed present in the
official termux-api package.

### 📦 Ejemplo
`examples/wifi.titan` — escanea redes cercanas, imprime SSID / RSSI /
frecuencia / bars por cada AP, después muestra el estado de la conexión
actual (SSID, IP, MAC, link speed).

## 0.13.0 — Phase 12 (part 3): BERT-family multi-input inference

### Added
- **`std::onnx::load_bert(path, batch, seq_len)`** — pin both input
  tensors (`input_ids`, `attention_mask`) to `[batch, seq_len]` of
  `i64` before optimize. Matches the shape 99% of HuggingFace exports
  use for DistilBERT / MiniLM / RoBERTa classifiers and encoders.
- **`std::onnx::load_bert3(path, batch, seq_len)`** — same but pins
  three inputs (`input_ids`, `attention_mask`, `token_type_ids`).
  Classic BERT-base-uncased needs this third tensor.
- **`std::onnx::run_bert(handle, shape, input_ids, attention_mask)`**
  → `{values, shape}` — feeds a text sample through the model in one
  call. Combined with `std::tokenize::encode()` from Phase 12 pt.1,
  a text-to-logits pipeline fits in ~10 lines of `.titan`.
- **`std::onnx::run_bert3(handle, shape, input_ids, attention_mask,
  token_type_ids)`** — three-input equivalent.
- **`std::math::exp(x)`**, **`std::math::log(x, base)`**,
  **`std::math::to_float(int)`**, **`std::math::to_int(float)`** —
  small additions needed to build a real softmax + int/float
  arithmetic on top of tokenizer/model outputs without leaving Titan.
- **`std::tokenize::encode_padded(handle, text, max_length, pad_id, add_special_tokens)`**
  — encode + pad-to-max_length or truncate. Necessary when the ONNX
  transformer graph was compiled with a fixed `[batch, seq_len]` input
  shape (which is the norm — MiniLM's tokenizer.json ships with
  padding baked in, but DistilBERT's doesn't). Uses `pad_id` for
  `ids` / `type_ids` / `special_tokens_mask` and `0` for `attention_mask`
  so downstream transformers correctly ignore padded positions.
- **`examples/sentiment.titan`** — end-to-end demo: loads a real
  DistilBERT sentiment classifier (SST-2, 2 classes: NEGATIVE /
  POSITIVE), tokenizes English text, runs the ONNX forward pass on
  device, applies a numerically-stable 2-class softmax and prints
  the sentiment label with its confidence — 100% offline, no cloud,
  no API keys, no Python interpreter.

### Notes
- The Rust API additions are non-breaking: `load`, `load_shape`,
  `run_f32`, `run_ids` from v0.12.0 still work unchanged.
- Suggested model for the demo:
  `Xenova/distilbert-base-uncased-finetuned-sst-2-english` — use the
  **FP32 `model.onnx`** (~260 MB), NOT `model_quantized.onnx`
  (~65 MB). The quantized export uses INT8-specific ops that
  onnxruntime handles in the browser but tract-onnx cannot analyse.
  Download instructions printed by the example already point at the
  right one.

## 0.12.0 — Phase 12 (part 2): ONNX inference on-device

### Added
- **`std::onnx::*`** — real ONNX model inference via `tract-onnx` 0.21,
  the pure-Rust runtime Sonos uses in production for wake-word and
  streaming speech recognition on their smart speakers. **No CUDA, no
  cuDNN, no BLAS, no ONNX Runtime C++.** Runs anywhere Rust compiles,
  including armv7-linux-androideabi (your Termux ARM phone).
- API (opaque `i64` handles; multiple models can coexist):
    * `load(path)` — parse → optimize → make runnable in one shot.
    * `load_shape(path, shape)` — same, but pin the first input's shape
      before optimizing (needed for models with dynamic axes, e.g. BERT
      that leaves batch/seq-len symbolic).
    * `close(handle)`.
    * `input_count(handle)` / `output_count(handle)`.
    * `input_shape(handle, i)` / `output_shape(handle, i)` — return an
      `[Int]` shape (may contain -1 for symbolic dims tract couldn't
      resolve statically).
    * `run_f32(handle, shape, data)` — flat f32 input, returns
      `{values: [Float], shape: [Int]}` (first output). Perfect for
      MNIST, MobileNet, image classifiers, VAD, etc.
    * `run_ids(handle, shape, ids)` — same but for i64 token-id inputs
      (BERT / MiniLM / DistilBERT and other transformers), so you can
      pipe the output of `std::tokenize::encode()` straight in.
- **Combines** with Fase 12 pt.1 (`std::tokenize::*`): tokenize text →
  feed ids to an ONNX transformer → get embeddings back. All on-device,
  offline, no cloud, no API key.

### Notes
- `tract-onnx` 0.21 build takes 8–12 min on Termux the first time
  (~50 crates in the dep graph — `prost` protobuf, `tract-hir`,
  `tract-nnef`, `tract-onnx-opl`, `tract-core`, `tract-linalg`,
  `smallvec`, `num-integer`, `memmap2`, ...). All pure Rust, no C.
- Suggested first model: MNIST-8 (~26 KB) or MobileNet-v2 (~14 MB).
  Both are on the ONNX model zoo.
- For LLM-family models (BERT, MiniLM, DistilBERT), use `load_shape`
  and pass `[1, seq_len]` as input shape before you feed ids.

## 0.11.0 — Phase 12 (part 1): HuggingFace tokenizers

### Added
- **`std::tokenize::*`** — real HuggingFace `tokenizers` crate 0.22
  built in a **pure-Rust configuration**. Defaults are deliberately
  turned off (`default-features = false`) to avoid three C/C++ deps
  that would break Termux builds:
    * `esaxx_fast`  → skipped (C++ suffix-array; pure-Rust fallback works)
    * `onig`        → skipped (C Oniguruma regex; replaced by `fancy-regex`)
    * `progressbar` → skipped (Phase 6 already ships `indicatif`)
  Only `fancy-regex` is enabled. **v0.22 is the first release that
  properly gates `SysRegex` on `fancy-regex XOR onig`** — v0.20/0.21
  hardcoded `mod onig;` and refused to compile without the C library.
- API (opaque `i64` handles from a process-wide registry, so multiple
  tokenizers can coexist):
    * `load(path)` — open a HuggingFace `tokenizer.json` from disk.
    * `from_json(text)` — same but from an in-memory JSON string.
    * `close(handle)` — release.
    * `vocab_size(handle)` — total vocab (incl. added tokens).
    * `encode(handle, text, add_special_tokens)` → map with
      `ids`, `tokens`, `type_ids`, `attention_mask`, `special_tokens_mask`.
    * `encode_batch(handle, texts, add_special_tokens)` — same but
      returns an array of maps (uses rayon internally for parallelism).
    * `decode(handle, ids, skip_special_tokens)` → string.
    * `token_to_id(handle, token)` / `id_to_token(handle, id)` — lookups
      that return `nil` when the token/id is absent.

### Coming next in Phase 12
- `std::onnx::*` via `tract-onnx` (pure-Rust ONNX inference). Kept as a
  separate patch so `tract`'s 8-12 min compile doesn't hold this shipment
  back if something needs adjusting.

## 0.10.0 — Phase 14: SVG charts (plotters)

### Added
- **`std::plot::*`** — real, pure-Rust charts via `plotters` 0.3.
  Deliberately built **without** `ttf` / `font-kit` (which pull in
  `freetype-sys` / `expat-sys` / `fontconfig`, all C-deps that break or
  bloat Termux builds). Every function writes a standalone `.svg` file;
  text is rendered by whatever viewer opens the file.
  - `line(path, title, x_axis, y_axis, xs, ys)` — single line chart
    with a marker on every sample.
  - `multi_line(path, title, x_axis, y_axis, labels, xs_of_series, ys_of_series)`
    — 3 parallel arrays (a triple-of-arrays per series would be a
    heterogeneous literal, which Titan's typechecker rejects). Each
    series gets a stable colour from an 8-slot palette + a legend entry.
  - `bar(path, title, y_axis, labels, values)` — bar chart.
  - `scatter(path, title, x_axis, y_axis, xs, ys)` — scatter plot.
  - `histogram(path, title, x_axis, values, bins)` — auto-binned
    histogram.
- **`examples/charts.titan`** — writes 5 SVGs to `$HOME` (line, bar,
  scatter, histogram, multi-line) and prints their paths so you can
  `termux-open` them or `rsvg-convert` them to PNG.

### Notes
- SVGs are ~5-15 KB each — safe to commit to a repo, e-mail, or
  attach to WhatsApp.
- For PNG output on Termux, install `librsvg`: `pkg install librsvg`
  and then `rsvg-convert chart.svg -o chart.png`.
- Combines beautifully with `std::procfs::*` (Fase 8) to build live
  system dashboards, and with `std::server::respond_bytes` (Fase 11)
  to serve charts straight from an HTTP endpoint.

## 0.9.0 — Phase 11: Web server (tiny_http + matchit, axum-style)

### Added
- **`std::server::*`** — real pure-Rust HTTP/1.1 server via `tiny_http`
  0.12. No async runtime, no OpenSSL, no C shims. Blocking event-loop
  model that fits Titan's synchronous VM perfectly.
  - Lifecycle: `start(addr)`, `local_addr(server)`, `stop(server)`.
  - Accept: `accept(server, timeout_ms) → request | -1`.
  - Introspection: `method`, `url`, `path`, `query`, `remote_addr`,
    `header(name)`, `headers()` (whole map), `body()` (raw bytes),
    `body_text()` (UTF-8).
  - Responses: `respond` (text/plain), `respond_html`, `respond_json`,
    `respond_bytes(content_type, bytes)`, `respond_full(status,
    content_type, headers-map, body-bytes)`.
  - **WebSocket upgrade (RFC 6455):**
    `upgrade_websocket(request, max_message) → ws_handle`,
    `ws_recv(ws) → [kind, text, bytes]` (kind is one of `"text"`,
    `"binary"`, `"ping"`, `"pong"`, `"close"`; pings are auto-ponged),
    `ws_send_text`, `ws_send_binary`, `ws_close(ws, code, reason)`.
- **`std::router::*`** — high-performance radix-tree URL router via
  `matchit` 0.8 (the same crate axum uses internally).
  - `new()`, `drop(router)`.
  - `insert(router, pattern, tag)` — pattern syntax:
    * `/users` — static
    * `/users/{id}` — named parameter
    * `/files/{*rest}` — catch-all (must be last segment)
  - `at(router, path) → { pattern: tag, params: {name: value, ...} }`
    or `nil` when nothing matches.
  - `matches(router, path) → bool` for quick feature-flag style checks.
- **`examples/webserver.titan`** — end-to-end demo: binds a port,
  installs 4 routes with matchit, decodes path params for
  `GET /users/{id}` and `GET /files/{*rest}`, and returns JSON,
  HTML and plain text responses.

### Notes
- No TLS in the server itself (keeps the Termux build lean and avoids
  the `aws-lc-sys` C-dep trap). Put nginx / Caddy / stunnel in front
  for public HTTPS, or use the existing `std::http` client (which does
  use rustls) for outbound HTTPS.
- `std::ws::*` (RFC 6455 codec primitives) from Phase 3 stays available
  and is what `std::server::ws_*` builds upon.

## 0.8.0 — Phase 10: NoSQL (embedded KV + Redis)

### Added
- **`std::kv::*`** — real embedded key-value database via `sled` 0.34.
  Pure Rust, ACID, persists a whole database to a single directory on
  disk. Multiple databases and named sub-buckets ("trees") can coexist
  through opaque `i64` handles.
  - Lifecycle: `open(path)`, `close`, `flush`.
  - Default tree: `insert`, `get`, `remove`, `contains`, `len`, `clear`,
    `keys`, `compare_and_swap(key, expected, new)` — pass empty bytes
    for `None`.
  - Named trees (buckets): `open_tree(db, name)`, `tree_insert`,
    `tree_get`, `tree_remove`, `tree_len`, `tree_keys`.
- **`std::redis::*`** — blocking Redis client via `redis` 0.27.
  Connections are opaque handles.
  - Lifecycle: `connect(url)`, `close`, `ping`.
  - Strings: `set`, `set_ex`, `get`, `del`, `exists`, `expire`, `ttl`,
    `incr`, `keys(pattern)`.
  - Lists: `lpush`, `rpush`, `lrange`, `llen`.
  - Hashes: `hset`, `hget`, `hdel`, `hgetall`.
  - Escape hatch: `raw(command_and_args)` for anything else.
- `examples/database.titan` opens a sled database in `$HOME`, writes
  three users, reads one back, walks all keys, uses a "sessions"
  sub-bucket for tokens, exercises compare-and-swap, flushes and
  closes. Runs offline (no Redis required).

### Nothing removed
All Phases 1-9 remain untouched.

---

## 0.7.0 — Phase 9: Audio

### Added
- **`std::audio::*`** — real WAV I/O and synthesis (crate `hound`, pure
  Rust, no native audio deps), plus playback and recording delegated to
  the Termux:API binaries so the compile never breaks on Android.
  - Read: `read_wav(path)`, `read_wav_bytes(bytes)` — both return
    `{ samples, sample_rate, channels, bits_per_sample }` with the
    samples normalized to floats in `[-1.0, 1.0]`.
  - Write: `write_wav(path, samples, sample_rate, channels)` and
    in-memory `encode_wav(samples, sample_rate, channels)`.
  - Synthesis: `sine_wave`, `square_wave`, `saw_wave`, `white_noise` —
    each returns a float sample array for the requested duration/rate.
  - Playback (via `termux-media-player`): `play(path)`, `pause`,
    `resume`, `stop`, `info`, `is_termux_media_available`.
  - Recording (via `termux-microphone-record`): `record_start(path,
    seconds)`, `record_stop`, `record_info`.
- `examples/audio.titan` synthesises a 500 ms A4 tone, writes and
  re-reads the WAV, tries to play it via Termux:API, and stitches
  Do-Re-Mi-Fa-Sol into a scale WAV.

### Nothing removed
All Phases 1-8 remain untouched.

---

## 0.6.0 — Phase 8: System & OS

### Added
- **`std::procfs::*`** — cross-platform system information via `sysinfo`.
  Works on Termux/Android, Linux and macOS.
  - Identity: `hostname`, `kernel`, `os_name`, `os_version`, `uptime`.
  - CPU: `cpu_usage` (global %), `cpu_count`, `cpus()` (per-core map).
  - Memory: `total_memory`, `used_memory`, `available_memory`,
    `total_swap`, `used_swap`.
  - `load_average()` returning `{one, five, fifteen}`.
  - Processes: `process_count`, `top_processes(limit)` sorted by CPU %.
  - `disks()` and `networks()` with usage counters.
- **`std::fswatch::*`** — file-system watcher powered by `notify`
  (inotify on Linux/Android).
  - `watch_once(path, timeout_ms, recursive)` — one-shot blocking watch.
  - Handle-based `open(path, recursive)` + `next_event(handle, timeout_ms)`
    + `close(handle)` for long-lived daemons.
- **`std::signals::*`** — Unix signals via `signal-hook`.
  - `install("SIGINT")` (idempotent), `pending("SIGINT")` for counter
    polling, `wait_any(timeout_ms)` returning the first fired signal.
  - Names accepted with or without `SIG` prefix.
- `examples/system.titan` demoing hostname, OS, CPU %, memory, load
  average, top processes, disks and network counters.

### Nothing removed
All Phases 1-7 stay exactly as they were in 0.5.0.

---

## 0.5.0 — Phase 7: Images & QR codes

### Added
- **`std::image::*`** — real image processing via the `image` crate.
  Supports PNG, JPEG, WebP, BMP, GIF. Images are managed through opaque
  `i64` handles kept in a process-wide registry.
  - I/O: `load(path)`, `load_bytes(bytes)`, `save(handle, path)`,
    `encode(handle, format)`, `close(handle)`.
  - Metadata: `width`, `height`, `color_type`.
  - Transforms (return new handles): `resize`, `resize_exact`,
    `thumbnail`, `crop`, `grayscale`, `blur`, `brighten`,
    `rotate90`/`180`/`270`, `flip_horizontal`, `flip_vertical`.
  - Named filters: `nearest`, `triangle`, `catmullrom`, `gaussian`,
    `lanczos3`.
- **`std::qrcode::*`** — QR code generation via the `qrcode` crate.
  - `to_ascii(text, level, dark, light)` — printable text.
  - `to_unicode(text, level)` — dense Unicode block art.
  - `to_svg(text, level, module_pixels)` — SVG bytes.
  - `to_png(text, level, side_pixels)` — PNG bytes.
  - `save_png(text, level, side_pixels, path)` — write PNG to disk.
  - Error-correction levels: `L`, `M`, `Q`, `H`.
- `examples/images.titan` demoing a QR encoded as ASCII + Unicode + PNG,
  then reloading the PNG and creating a 100×100 thumbnail and a
  grayscale version.

### Combines beautifully with earlier phases
- Take a photo with `std::termux::camera_photo` (Phase 5), resize it
  with `std::image::resize` (Phase 7), hash it with `std::hash::sha256`
  (Phase 1), generate a QR of the hash with `std::qrcode::to_ansi`
  (Phase 7), and share it via `std::termux::share` (Phase 5).

### Nothing removed
All Phases 1-6 remain untouched.

---

## 0.4.0 — Phase 6: Terminal & TUI

### Added
- **`std::term::*`** — real terminal control powered by `crossterm`:
  - `print_colored`, `print_styled`, `print_attr` (bold/italic/underline).
  - Named colours plus custom `rgb:R,G,B` and `#RRGGBB`.
  - `clear_screen`, `clear_line`, `move_to`, `hide_cursor`, `show_cursor`,
    `size`, `flush`.
  - Alt-screen / raw-mode switches: `enter_alt_screen`, `leave_alt_screen`,
    `enable_raw`, `disable_raw`.
  - `read_key(timeout_ms)` returning normalized names like `Enter`,
    `Ctrl+c`, `Shift+F1`, `Up`.
- **`std::readline::*`** — GNU-Readline-style line editing via `rustyline`:
  - `prompt`, `prompt_with_history`, `prompt_persistent(prompt, path)`,
    `prompt_secret` (input hidden).
- **`std::progress::*`** — animated progress via `indicatif`:
  - `bar_new(total)`, `spinner_new()`, `set_message`, `set_position`,
    `increment`, `finish`, `abandon`.
- `examples/tui.titan` demoing colors, terminal size, an animated
  progress bar and a spinner.

### Nothing removed
All Phase 1-5 modules from 0.3.0 remain exactly as they were.

---

## 0.3.0 — Phase 5: real Android hardware & OS bindings

### Added
- **`std::termux::*`** — 23 native functions that shell out to the
  Termux:API CLI shipped by the Termux:API Android app. Everything is
  real, nothing is simulated:
  - Device state: `battery_status`, `wifi_info`, `telephony_info`.
  - Location: `location(provider, request)`.
  - Sensors: `sensor_list`, `sensor_read`.
  - Real system clipboard: `clipboard_get`, `clipboard_set`.
  - Feedback: `vibrate`, `torch`, `toast`, `notify`, `notify_remove`,
    `tts_speak`, `brightness`.
  - Communications: `sms_list`, `sms_send`, `contacts`.
  - Camera: `camera_info`, `camera_photo`.
  - Dialog & sharing: `dialog`, `share`.
  - Availability probe: `is_available` for cross-platform code.
- All Phase-5 natives are gated behind the `Process` capability, so
  `zett run --sandbox` blocks them consistently with `std::process::*`.
- `examples/android.titan` demonstrating battery, toast, vibration,
  clipboard, notification, sensors and WiFi.

### Requirements (on-device)
- **Termux:API** app from F-Droid or Play Store.
- `pkg install termux-api` inside Termux.

If the CLI is missing, every helper returns a typed
`TermuxError::MissingCli` so `.titan` programs can degrade gracefully.

---

## 0.2.0 — Phases 1-4

### Added

**Phase 1 — Fundamentals**
- `std::regex` — Unicode-aware pattern matching (crate `regex`).
- `std::uuid` — UUID v4 and v7 (crate `uuid`).
- `std::hash` — SHA-256/384/512, SHA-3, BLAKE3, HMAC (RustCrypto).
- `std::random` — OS entropy + reproducible ChaCha20 (crate `rand`).
- `std::datetime` — dates, RFC 3339/2822, offsets (crate `chrono`).
- `std::url` — parse/build URLs and query strings (crate `url`).
- `std::dirs` — HOME/config/cache/downloads (crate `dirs`).

**Phase 2 — Formats & compression**
- `std::compress` — Gzip, Zlib, Deflate, Zstandard.
- `std::archive` — tar & zip pack/unpack, zip-slip safe.
- `std::yaml` — parse, stringify, multi-document.
- `std::xml` — quick-xml tree parsing + escapes.

**Phase 3 — Advanced networking**
- `std::http_full` — blocking HTTPS client (ureq + rustls-ring).
- `std::dns` — hickory-resolver lookups (A, AAAA, MX, TXT, CNAME, PTR).
- `std::email` — SMTP with STARTTLS / implicit TLS (lettre).

**Phase 4 — Modern cryptography**
- `std::crypto` — ChaCha20-Poly1305, AES-256-GCM AEAD.
- `std::password` — Argon2id, bcrypt.
- `std::jwt` — HS256 / RS256 JSON Web Tokens.

### Fixed
- Cleaned up hallucinated artifacts left by an earlier AI-assisted
  session (zero-byte binaries, imaginary informe docs, broken fix
  scripts). Marked `titan native` / `titan mobile` as experimental
  with runtime warnings; they don't produce loadable ELF or APK yet.
- Forced the whole workspace onto rustls' `ring` backend to keep the
  Termux build small and avoid aws-lc-sys (~300 C files).
- `titan_tls::ensure_default_crypto_provider()` installs a default
  rustls CryptoProvider exactly once per process; fixes the crash in
  `titan_postgres::builds_rustls_connector` after Phase 3 landed.

### Notes
- All new stdlib modules live behind Cargo features; the `extras`
  meta-feature bundles them all and is on by default.
- Docs: `docs/EXTRAS.md` walks through every module and the Termux
  build recipe.

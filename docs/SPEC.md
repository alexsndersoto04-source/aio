# Titan Language Specification

**Document version:** 1.0 — 2026-08-21
**Applies to:** TITAN / Zett compiler `1.0.0` (workspace version in `Cargo.toml`)
**Canonical source extension:** `.titan`
**Compiled artifact:** `TITAN-BYTECODE 1` container (`.tbc`)

---

## 0. Scope and reading conventions

This document describes the language **as the current implementation accepts it**. It is
descriptive, not aspirational: every construct listed as supported is executable today, and
every construct that is parsed but refused is listed explicitly, together with the diagnostic
the compiler emits.

Three status labels are used throughout:

| Label | Meaning |
|---|---|
| **Supported** | Parsed, type-checked, lowered to bytecode, and executable in the VM. |
| **Parsed, rejected** | The grammar accepts the syntax so that tooling can read it, but the typechecker or code generator refuses it with a specific error. Nothing is silently miscompiled. |
| **Not in the grammar** | The parser fails. The token may exist in the lexer, but no production consumes it. |

Section [§18](#18-consolidated-list-of-current-limitations) consolidates every limitation in one
table. Section [§19](#19-how-this-document-was-verified) records how each claim was checked
against the source tree.

Two backends exist and they do **not** accept the same programs. Unless a section says
otherwise, its contents describe the bytecode VM. The WebAssembly backend and its narrower
subset are specified in [§16](#16-webassembly-backend).

---

## 1. Program structure and entry point

A program is a sequence of declarations. Execution begins at `fn main()`.

```titan
const LIMIT: int = 20

fn main() {
    print("limit={LIMIT}")
}
```

Rules enforced by the implementation:

- `main` must take **zero parameters**. A parameter list is rejected at semantic time
  (`entry point 'main' must take no parameters, but N were declared`) and again in code
  generation, because the runtime always invokes the entry point with no arguments.
- All declarations from all loaded files enter **one flat executable namespace**. Duplicate
  names are rejected (`duplicate function 'name'`) rather than shadowed.
- Top-level item kinds: `fn`, `struct`, `enum`, `trait`, `impl`, `mod`, `import`, `const`,
  `type`.

---

## 2. Lexical structure

### 2.1 Identifiers, comments, literals

- Identifiers begin with a Unicode alphabetic character, followed by Unicode alphanumerics or `_`.
- `//` starts a line comment. `/* ... */` block comments **nest**; an unterminated block comment
  is a lexical error.
- String escapes: `\n`, `\r`, `\t`, `\0`, `\"`, `\'`, `\\`. Any other escape is a lexical error.
- Integer literals accept `_` separators (`1_000_000`).
- Character literals are single Unicode scalars: `'a'`.
- Statements are separated by newlines or `;`.

### 2.2 Operators and punctuation

```
+  -  *  /  %       &  |  ^  ~  !
+= -= *= /= %=      == != <= >= < >
&& ||               ::  =  .  ,  :  ;  _
->  =>  ?           ..  ..=
|>  <=>             #{  (  )  {  }  [  ]
```

`|>` is the pipeline operator and `<=>` is the three-way comparison operator; both are
described in [§4.3](#43-pipeline-and-three-way-comparison). `#{` opens a map literal
([§7.4](#74-maps)).

### 2.3 Keywords

**Active:** `let` `mut` `fn` `return` `if` `else` `match` `for` `while` `loop` `break`
`continue` `in` `struct` `enum` `trait` `impl` `mod` `import` `const` `spawn` `go` `true`
`false` `nil` `self` `type` `pub` `extern`.

- `go` is an accepted synonym of `spawn`.
- `pub` is accepted and ignored: there is a single executable namespace, so no visibility
  distinction is applied yet.
- `extern` parses, but an `extern fn` has no runtime linkage and is rejected
  (`extern function 'name' without runtime linkage`).

**Reserved tokens with no production:** `as`, `unsafe`. The lexer produces them, but no grammar
rule consumes them, so any use is a parse error. They are reserved for future casts and unsafe
blocks.

---

## 3. Types

### 3.1 Built-in type names

| Written as | Internal type | Notes |
|---|---|---|
| `int`, `i32`, `i64`, `u64`, `usize` | `Int` | All map to the VM's signed 64-bit integer today. The width in the name is documentation, not a guarantee. |
| `float`, `f32`, `f64` | `Float` | All map to IEEE-754 binary64. |
| `bool` | `Bool` | |
| `char` | `Char` | Unicode scalar value. |
| `string`, `str` | `String` | UTF-8. |
| `[T]` | `Array(T)` | Slice syntax is the array type. |
| `Array<T>`, `Vec<T>` | `Array(T)` | The only named types that accept a type argument. |
| `array` | `Array(any)` | Convenience alias for a heterogeneous array. |
| `map` | `Named("map")` | String-keyed map; see [§7.4](#74-maps). |
| `any` | `Unknown` | Dynamic escape hatch; compatible with every type. |
| `(A, B)` | `Tuple` | |
| `fn(A) -> B` | `Function` | Type of closures and named functions used as values. |
| `Option`, `Result` | prelude enums | See [§11](#11-option-result-and-the--operator). |
| `Name` | `Named` | User struct, enum, or handle type. |
| `nil` | `Nil` | Runtime absence value; compatible with unit positions. |

Runtime handle types produced by intrinsics (`TcpListener`, `TcpStream`, `TlsStream`,
`HttpRouter`, `Sender`, `Receiver`, `Task`, database handles, and similar) are opaque named
types. They can be passed to the intrinsics that accept them and stored in locals; they have no
fields and no user-visible representation.

Integer arithmetic is **checked**: overflow, division by zero, and out-of-range indexing are
structured runtime errors, never silent wraparound or undefined behavior.

### 3.2 Generics

Titan is **monomorphic**. Exactly one parameterized shape exists: `Array<T>` / `Vec<T>`, which
take one argument. Every other named type must be written with zero type arguments; writing
`Option<int>` or `Map<K, V>` is rejected with
`type 'Name' expects 0 type arguments, found N`.

This is deliberate. Accepting and discarding type arguments would advertise generic guarantees
the checker does not enforce. User-defined generic functions and generic structs are **not in
the grammar**.

### 3.3 Type aliases

```titan
type UserId = string
type Score  = int
```

Aliases are resolved before compatibility checks and cost nothing at runtime. Recursive aliases
are rejected (`type alias 'T' is recursive`). Aliases of **function types**
(`type Handler = fn(int) -> string`) are not accepted by the type parser.

### 3.4 Type forms that are parsed and rejected

| Form | Diagnostic |
|---|---|
| `&T`, `&mut T` | `unsupported language feature: reference types` |
| `[T; N]` (fixed size) | `unsupported language feature: fixed-size array types` |

---

## 4. Variables, operators, precedence

### 4.1 Bindings and assignment

```titan
let value: int = 10          // immutable
let mut total = 0            // mutable
total += 2
total = total * 3
```

Type annotations are optional where inference succeeds. Assignment rules enforced by codegen:

- The target must be a **local variable of the current function**
  (`assignment target must currently be a variable`).
- The local must be declared `mut` and must not be a closure capture
  (`assignment to immutable or captured local 'name'`).
- Fields, array elements, and map entries are **not** assignable through `=`. Use the
  corresponding standard-library call (for example `std::array::set`, `std::map::insert`),
  which returns an updated value.

Compound assignment supports `+= -= *= /= %=`.

### 4.2 Precedence

From tightest to loosest binding:

| Level | Operators |
|---|---|
| 9 | `*` `/` `%` |
| 8 | `+` `-` |
| 7 | `<` `>` `<=` `>=` `<=>` |
| 6 | `==` `!=` |
| 5 | `&` |
| 4 | `^` |
| 3 | `\|` |
| 2 | `&&` |
| 1 | `\|\|` |
| — | ranges `..` `..=`, then assignment, then `\|>` (loosest) |

Unary operators `-`, `!`, `~` bind tighter than any binary operator. `&&` and `||` short-circuit.

### 4.3 Pipeline and three-way comparison

Both operators are **pure syntactic sugar expanded by the parser**. No opcode, VM change, or
runtime cost is involved.

```titan
x |> f              //  f(x)
x |> f(a, b)        //  f(x, a, b)   — the piped value is always the FIRST argument
a <=> b             //  -1 if a < b, 0 if equal, 1 if a > b
```

`|>` has the loosest precedence, so `x + 1 |> print` groups as `(x + 1) |> print`, and it is
left-associative, so pipelines chain. The right operand must be an identifier or a call.

`<=>` expands to an `if`/`else` chain over temporaries, so each side is evaluated **exactly
once**. It is designed for `sort_by` comparators.

---

## 5. Control flow

```titan
if condition { ... } else { ... }
while condition { ... }
loop { ... }
for item in collection { ... }
match value { ... }
```

- `if` is an expression; both branches must produce compatible types when the value is used.
- `break` and `continue` are legal only inside loops (`break/continue used outside a loop`).
- **`break` with a value is parsed and rejected** (`unsupported language feature: values carried
  by break`). `loop` therefore cannot yield a value; use a mutable local.
- `return` is checked against the declared return type. A function that can finish without
  returning its declared type is rejected (`function 'f' can finish without returning T`), as
  are inconsistent return types across paths.

### 5.1 Iterable receivers of `for`

| Receiver | Item type |
|---|---|
| `[T]` | `T` |
| `string` | `char` |
| `bytes` | `int` |
| tuple | the common type of its elements |
| range (`a..b`, `a..=b`) | `int` |

Anything else is rejected (`value of type T is not iterable`). Maps are not directly iterable;
iterate `std::map::keys(m)` (or `std::map::values(m)`) and read entries with
`std::map::get(m, key)`.

### 5.2 Ranges

`a..b` and `a..=b` are **materialized eagerly into an array of integers** by the VM. This is
observable and bounded: a range longer than **1,000,000 elements** fails with
`range exceeds the one-million element safety limit`. Ranges are not lazy iterators and are not
first-class range objects.

---

## 6. Functions, closures, recursion

```titan
fn fib(n: int) -> int {
    if n <= 1 { return n }
    fib(n - 1) + fib(n - 2)
}
```

- The last expression of a block is its value; `return` is optional.
- Calls are arity-checked at compile time and again in the VM.
- Recursion is bounded by a call-depth limit (default **4096** frames,
  `call depth limit exceeded`) and by a total instruction budget (default **10,000,000**,
  `instruction limit exceeded`). Both are configurable when embedding the VM.

### 6.1 Closures

```titan
let offset = 10
let add = |value: int| -> int value + offset
print(add(5))
```

Closures capture visible lexical bindings **by value**, in deterministic name order. Captured
locals are read-only inside the closure. Named functions are also first-class values and can be
stored, passed, and returned.

**Exception:** the compiler's built-in operations (`print`, `len`, `map`, `filter`, `fold`,
`sort_by`, `find`, `any`, `all`, and the concurrency and intrinsic families of
[§13.1](#131-built-in-and-intrinsic-call-surface)) exist only as direct-call bytecode
operations. Referring to one as a value is rejected
(`unsupported language feature: built-in function values ('map')`).

### 6.2 Function declaration forms that are parsed and rejected

| Form | Diagnostic |
|---|---|
| Default parameter values | `default parameter 'f::p'` |
| `extern fn` | `extern function 'f' without runtime linkage` |
| A body-less `fn` outside a `trait` | `bodyless function 'f' outside a trait declaration` |
| Declarations nested inside a block | `nested declarations are not executable yet` |

---

## 7. Aggregate values

### 7.1 Structs

```titan
struct Point { x: int, y: int }
let point = Point { x: 2, y: 3 }
print(point.x)
```

Struct literals are validated for missing, unknown, and duplicated fields.

### 7.2 Enums

```titan
enum Maybe { None, Some(int) }
let value = Maybe::Some(42)
```

A variant carries **zero or one** payload value. Multi-payload variants
(`Variant(int, string)`) are not in the grammar; use a tuple or a struct as the single payload.

### 7.3 Tuples and arrays

```titan
let pair = (1, "one")
let numbers = [1, 2, 3]
let combined = [..numbers, 99, ..more]     // spread
```

The spread form `..expr` inside an array literal is expanded by the parser into
`std::array::concat` calls. Arrays may be heterogeneous; the checker tracks a mixed-element type
that satisfies `any`/`[any]` but does not silently satisfy `[int]` or `[string]`.

### 7.4 Maps

```titan
let config = #{"host": "0.0.0.0", "port": 8080}
let m = std::map::new()
let m = std::map::insert(m, "key", value)
print(config.host)
```

`#{ ... }` is expanded by the parser into `std::map::new()` plus `std::map::insert` calls. Keys
are strings. Field-access syntax works on maps as well as structs: at runtime, `x.field` reads a
struct field **or** a map entry, and a missing key raises `unknown field 'name'`.

Maps are values, not references. `std::map::insert` returns a new map; it does not mutate in
place.

---

## 8. Pattern matching

Patterns appear in three positions, and the supported set is **not** the same in each.

### 8.1 `let` and `for` destructuring — fully supported

```titan
let (a, b) = pair
let Point { x, y } = point
let Point { x: cx, y: cy } = point       // rename
let (a, _) = pair                        // wildcard
let (a, (b, c)) = nested                 // recursive
for (a, b) in pairs { ... }              // array of tuples
for Product { title, price } in inventory { ... }
```

These are expanded **by the parser** into a temporary plus one ordinary `let` per bound name.
The VM never sees a pattern, which is why nesting, renaming, and wildcards all work here.

### 8.2 `match` arms — restricted

```titan
match value {
    Maybe::Some(n) => n,
    Maybe::None    => 0,
}

match code {
    200 => "ok",
    n if n >= 500 => "server error",
    _ => "other",
}
```

Supported arm patterns: wildcard `_`, an identifier binding, a literal, an enum variant without
payload, and an enum variant whose inner pattern is an identifier or `_`. Guards (`if cond`) are
supported.

Rejected in `match` (`unsupported language feature: or-patterns and nested destructuring in
match`, and the corresponding codegen errors `or-pattern bytecode`,
`destructuring pattern bytecode`, `nested enum destructuring pattern`):

- or-patterns `A | B`
- tuple patterns `(a, b)`
- struct patterns `Point { x, y }`
- an enum payload pattern that is itself a tuple, struct, literal, or nested enum

### 8.3 Exhaustiveness

- A `match` over a `bool` must cover `true` and `false` or provide a catch-all.
- A `match` over a declared enum must cover every variant or provide a catch-all; the error
  names the missing ones (`non-exhaustive match for enum 'E'; missing ["None"]`).
- Arms that can never be reached are reported (`match arm N has an unreachable pattern`).

---

## 9. Strings and interpolation

```titan
print("fib({i}) = {fib(i)}")
```

Interpolation uses a **deliberately restricted grammar**, unchanged in 1.0:

- a local binding, or
- a declared global constant, or
- a call whose callee is a name and whose arguments are local identifiers or integer literals.

Arbitrary expressions, field access, indexing, and arithmetic inside `{ ... }` are rejected with
`invalid string interpolation expression '...'`. Call resolution inside a template follows
ordinary rules, including the priority of a local closure or callable constant over a static
function or native of the same name.

String concatenation with `+` accepts a string on either side and stringifies the other operand.

---

## 10. Modules, imports, and projects

### 10.1 Declaration and import syntax

```titan
mod geometry { ... }
import geometry::Point
import std::async
```

Import resolution, performed by the project loader:

- `a::b` resolves to `a/b.titan` or `a/b/mod.titan`, relative to the source root.
- Local path dependencies declared in `Titan.toml` are resolved as additional roots. Remote
  dependencies must be fetched first; a dependency without a resolvable path fails with
  `dependency 'name' must specify a local path in this compiler version`.
- `import std::x` additionally searches the installed Titan-level standard library
  (`$ZETT_STDLIB_DIR`, `<prefix>/share/zett/stdlib`, or `stdlib/` next to the executable). Parts
  of the standard library are written in Titan itself; `std::async` is the current example.
- Every path is canonicalized, loaded once, and constrained to an authorized source root
  (`import 'x' resolves outside source root ...`). Import cycles are detected and reported with
  the full chain.

`mod` blocks group declarations for readability. Their items are collected into the same single
executable namespace as everything else — module paths are not yet a namespacing mechanism, so
two functions with the same name in different modules collide.

### 10.2 Project layout

```
Titan.toml          manifest: name, version, dependencies
Titan.lock          generated lockfile
src/main.titan      default entry point
tests/*.titan       test programs discovered by `titan test`
```

`titan test` compiles and runs every `.titan` file under `tests/`, reporting one line per file
and exiting non-zero on any failure. Assertions come from `std::testing::assert` and
`std::testing::assert_eq`; there is no separate test-function attribute.

---

## 11. `Option`, `Result`, and the `?` operator

The prelude provides `Option::None`, `Option::Some(value)`, `Result::Ok(value)`, and
`Result::Err(error)`.

Postfix `?` unwraps `Some`/`Ok`. On `None`/`Err` it **returns that value immediately from the
current function**. Applying `?` to anything else is a runtime type error
(`operator ? requires Result or Option`) and is flagged by the checker
(`operator ? requires an Option or Result value`).

### 11.1 Errors as values

`std::try::catch(callable)` calls a closure under the `TryCall` operation: any runtime failure —
native error, type mismatch, index out of bounds, arithmetic error — is captured and returned as
`Result::Err(string)`, while success returns `Result::Ok(value)`. This is the mechanism that
makes failures inspectable from Titan code instead of terminating the program.

---

## 12. Traits, `impl`, and methods

```titan
trait Greetable {
    fn name(self) -> string;                       // required
    fn greet(self) -> string {                     // default body
        "Hola, " + self.name() + "!"
    }
}

struct Person { first: string }

impl Greetable for Person {
    fn name(self) -> string { self.first }
}
```

Supported:

- Traits with required methods and **default method bodies**. A default body may call other
  methods through `self`, including other defaults.
- `impl Trait for Type` and inherent `impl Type` blocks.
- Method-call syntax `receiver.method(args)`, dispatched dynamically by the VM
  (`CallMethod`). A declared struct method takes priority over a built-in collection method of
  the same name.
- Missing required methods are a compile error; an extra method not declared by the trait is
  rejected (`method 'm' is not declared by trait 'T'`), as is a signature mismatch.

Constraints:

- **`impl` targets must be declared structs** (`impl target 'X' is not a declared struct`).
  There are no impls on enums, primitives, or aliases.
- A trait default method **must declare an explicit return type**
  (`trait default method 'T::m' without an explicit return type`).
- There are no trait objects, no generic bounds, and no static dispatch tables: a method is
  resolved at runtime from the receiver's struct name.

### 12.1 Built-in methods on collections

When no user method matches, these intrinsic methods are available:
`len()`, `map(f)`, `filter(f)`, `fold(init, f)`, `sort_by(cmp)`, `find(f)`, `any(f)`, `all(f)`.
Each also exists in call form (`map(xs, f)`).

---

## 13. Standard library and the capability model

### 13.1 Built-in and intrinsic call surface

The compiler knows **122 signatures** that are not registry natives:

- 18 global built-ins: `print`, `println`, `len`, `map`, `filter`, `fold`, `sort_by`, `find`,
  `any`, `all`, `join`, `join_timeout`, `cancel`, `channel`, `send`, `recv`, `recv_timeout`,
  `select`.
- 104 qualified intrinsics lowered to dedicated opcodes rather than a generic native call:
  `std::net` (8), `std::tls` (6), `std::ws` (10), `std::http` (7), `std::server` (6),
  `std::sqlite` (16), `std::postgres` (16), `std::mysql` (15), `std::db` (8),
  `std::runtime` (12).

### 13.2 The native registry

`crates/titan_stdlib/src/native.rs` declares **758 unique native functions across 71 `std::*`
namespaces**, each with a fixed parameter list, a result type, and an optional required
capability. The compiler resolves qualified names through this registry and checks arity and
parameter types **before** code generation. A native failure becomes a runtime error that names
the function.

Values crossing the native boundary are ints, floats, bools, chars, strings, bytes, arrays,
tuples, maps, structs, and enums.

The registry is verifiable without compiling Rust: `python3 verify_phase34.py` re-derives the
table, checks for duplicate names, and validates every `std::*` call site in `examples/` and
`stdlib/` against the declared arity.

### 13.3 Capabilities

The VM carries five capability flags: `filesystem`, `process`, `network`, `environment`,
`user_interface`. Each guarded native and intrinsic checks its flag at dispatch time and fails
with `native function 'f' requires capability 'C'` when it is denied.

`titan run --sandbox` (also available on `exec` and `debug`) clears all five. Pure computation —
text, encoding, JSON, collections, math, statistics — keeps working; anything touching the
outside world does not.

### 13.4 Modules that are simulations, not OS integration

Two families are in-process **emulators** and are documented as such so their names are not
mistaken for hardware or platform bindings:

- `std::freestanding`, `std::freestanding_cpu`, `std::freestanding_memory`,
  `std::freestanding_mmio` — a bare-metal *model*: frame allocator, page mapping, exception
  table, and MMIO registers kept in host data structures. They generate linker scripts and
  startup assembly as text; they do not execute privileged instructions.
- `std::mobile` — an application-lifecycle state machine (`Running`, `Paused`, `Stopped`,
  `Destroyed`) with an event history. It is not bound to the Android activity lifecycle.

Actual Android integration is `std::termux::*`, which shells out to the `termux-api` commands.

---

## 14. Execution model

### 14.1 Compilation pipeline

```
source (.titan)
   → lexer → parser → AST
   → typechecker → code generator
   → bytecode module ──► TITAN-BYTECODE 1 container (.tbc) ──► Titan VM
                     └─► WebAssembly module (.wasm + source maps)
```

### 14.2 Bytecode container

`titan build` emits a portable container:

- 17-byte magic header `TITAN-BYTECODE 1\n`
- an envelope recording `format_version` (currently `1`), the producing `compiler_version`, a
  **CRC-32** of the module payload, and the module itself as JSON

`titan exec` validates before anything enters the VM: container size (≤ 64 MiB), function count
(≤ 100,000), instruction count (≤ 10,000,000), string-table size (≤ 1,000,000), then every jump
target, local index, string index, call target, arity, closure capture, and native signature.
A checksum mismatch or an unsupported format version is refused. Malformed artifacts never
execute.

### 14.3 Runtime guarantees

The VM is a stack machine with isolated locals per call frame, no pointer instructions, and no
`unsafe` execution path. Failures are structured values, not panics:

| Category | Errors |
|---|---|
| Arithmetic | `division by zero`, `integer overflow` |
| Memory / bounds | `index out of bounds`, `unknown field`, `stack underflow`, `invalid local` |
| Contracts | arity mismatch, `type error`, `invalid function index` |
| Budgets | `instruction limit exceeded`, `call depth limit exceeded`, `task memory limit exceeded`, `runtime <resource> limit exceeded` |
| Security | `native function 'f' requires capability 'C'` |
| Concurrency | `unknown or already joined task`, `task panicked`, `task cancelled`, `unknown channel`, `channel disconnected`, `invalid timeout` |
| Tooling | `execution terminated by debugger` |

Default budgets: 10,000,000 instructions, 4,096 call frames, 1,024 network handles, 256 database
handles, 64 connections per pool, 1,024 channels, 65,536 slots per channel.

### 14.4 Memory and garbage collection

`titan_gc` implements deterministic mark-and-sweep bookkeeping over runtime allocations, with a
configurable threshold (default 1 MiB) that triggers a collection on allocation. Programs can
introspect and control it through `std::runtime`: `allocated_bytes`, `memory_limit`,
`gc_live_count`, `gc_collect`, `gc_threshold`, `gc_set_threshold`, `active_tasks`,
`heap_dump(path)` (JSON), `optimize_level`, `fast_path_enabled`, `benchmark`.

---

## 15. Concurrency

```titan
let task = spawn || { work() }        // `go` is a synonym
let result = join(task)

let (tx, rx) = channel(16)
send(tx, value)
let received = recv(rx)
```

- `spawn` / `go` take a **zero-argument closure** and run it on a real host thread. Passing a
  closure with parameters is an arity error; passing a non-closure is a type error.
- `join(task)`, `join_timeout(task, ms)`, `cancel(task)` — cancellation is cooperative.
- `channel(capacity)` creates a bounded channel (capacity ≤ 65,536); `send`, `recv`,
  `recv_timeout(rx, ms)`, and `select(receivers, ms)` return `Option` where a timeout is
  possible.
- `std::runtime::spawn_quota(closure, bytes)` spawns a task with its **own memory budget**;
  exceeding it fails that task with `task memory limit exceeded` instead of destabilizing the
  process.

Tasks do not share mutable state: values are passed by value through channels and captures.

---

## 16. WebAssembly backend

`titan wasm <project>` emits a **self-contained WebAssembly module** — not a wrapper around the
VM — plus two source maps: a Titan-specific one in the custom section `titan.source_map`, and a
standard source map referenced by a `sourceMappingURL` custom section, so browser devtools can
step through `.titan` sources.

The backend implements its own runtime inside linear memory: a bump heap with checkpoints and
scopes, UTF-8 strings, arrays, string-keyed hash maps, structs, and enums, all emitted as raw
WebAssembly. Titan's control-flow graph, including backward branches, is reproduced with a
structured dispatch loop, so arbitrary validated bytecode compiles without relying on the host.
The emitter validates operand-stack balance, jump targets, local indices, and call arity while
generating code.

### 16.1 Supported in WebAssembly

Integer and float arithmetic and comparisons, bitwise operators, locals, branches and loops,
calls and returns, string literals and concatenation, `len`, arrays, tuples, structs, enums
(construction, tag tests, payload extraction), indexing, `std::array::{push, pop, set, slice,
concat}`, `std::map::{new, insert, insert_new, get, contains, remove, keys, values, length}`,
`std::text::{equals, hash64}`, `std::time::unix_millis`, the `std::wasm::heap_*` introspection
API, and `print` with exactly one argument routed to the host.

Browser integration is provided by host imports declared by the module and implemented in
JavaScript (`examples/browser/host.js`): the `std::web::*` family covering DOM manipulation,
events, `fetch`, WebSocket, Canvas 2D, animation frames, and WebGL2.

### 16.2 Rejected by the WebAssembly backend

Every rejection is an explicit diagnostic naming the function and the operation:

- **Ranges** — `start..end` / `start..=end` are not lowered yet, so `for i in 0..n` does not
  compile to WebAssembly. Use a `while` loop with a counter.
- Closures and the higher-order array operations (`map`, `filter`, `fold`, `sort_by`, `find`,
  `any`, `all`), `TryCall`, and dynamic method dispatch.
- Concurrency (`spawn`, channels, `select`) and every task operation.
- All host-facing intrinsic families: filesystem, process, TCP, TLS, WebSocket, HTTP server,
  SQLite, PostgreSQL, MySQL, `std::db`, `std::runtime`.
- Any registry native not in the browser host list above (`CallNative(std::x::y)`).
- String indexing, and collections or structs whose size exceeds WebAssembly's 32-bit memory.

In short: the WebAssembly target is a **compute-and-browser** target, not a systems target.

---

## 17. Tooling surface

The reference implementation ships one binary — `titan` when built from source, `zett` in
distribution packages — plus two standalone protocol servers.

### 17.1 Commands

| Command | Purpose |
|---|---|
| `run <input> [--sandbox] [args…]` | Compile and execute a file or project |
| `check <input>` | Parse and type-check only; reports files and function count |
| `build <input> [-o out.tbc]` | Emit the bytecode container and list every function with its op and local counts |
| `exec <file.tbc> [--sandbox]` | Validate and execute an existing artifact |
| `wasm <input> [-o out.wasm]` | Emit a WebAssembly module plus both source maps |
| `debug <input> [-b file:line…] [--sandbox]` | Interactive debugger; breakpoints resolve to source lines |
| `test <input> [--sandbox]` | Run every `.titan` file under `tests/` |
| `repl` | Read-eval-print loop; each line is wrapped in a synthetic `fn main` |
| `new <path>` | Create `Titan.toml` and `src/main.titan` |
| `add`, `fetch`, `update` | Declare, resolve, download, and verify dependencies |
| `keygen`, `pack`, `publish` | Ed25519 key generation, deterministic signed `.tpkg`, upload |
| `version` | Print version details |

`--sandbox` on `run`, `exec`, and `debug` applies the capability restrictions of
[§13.3](#133-capabilities).

### 17.2 Packages

`pack` produces a deterministic archive together with its SHA-256 digest, the signing public
key, and an Ed25519 signature. `fetch` verifies the signature before installing, and writes
`Titan.lock`. `publish` uploads using the `TITAN_REGISTRY_TOKEN` environment variable.

The registry host defaults to `https://registry.titan-lang.org`, which is a configurable default
rather than an operated public service; every packaging operation except `publish`/`fetch`
against a remote works fully offline, and local path dependencies need no registry at all.

### 17.3 Editor and debugger protocols

- `titan-lsp` — a stdio Language Server providing diagnostics on open and change, completion,
  signature help, hover, go-to-definition, references, rename, document and workspace symbols,
  and semantic tokens.
- `titan-dap` — a stdio Debug Adapter supporting `launch`, `setBreakpoints`, `stackTrace`,
  `scopes`, `variables`, `continue`, `next`, `stepIn`, `stepOut`, `pause`, and `terminate`.

---

## 18. Consolidated list of current limitations

| Area | Limitation | Diagnostic |
|---|---|---|
| Generics | Only `Array<T>` / `Vec<T>` are parameterized; no user generics | `type 'N' expects 0 type arguments, found K` |
| Namespaces | One flat executable namespace; `mod` does not scope names; `pub` is ignored | `duplicate function 'f'` |
| References | `&T`, `&mut T`, `*x`, `&x` | `references and dereferencing` |
| Arrays | Fixed-size array types `[T; N]` | `fixed-size array types` |
| Loops | `break value` | `values carried by break` |
| `match` | or-patterns, tuple patterns, struct patterns, nested enum payload patterns | `or-patterns and nested destructuring in match` |
| Functions | Default parameter values | `default parameter 'f::p'` |
| Functions | `extern fn` has no linkage | `extern function 'f' without runtime linkage` |
| Functions | Declarations nested inside a block | `nested declarations are not executable yet` |
| Values | Built-ins (`map`, `print`, …) used as values | `built-in function values ('map')` |
| Traits | `impl` only on declared structs; no trait objects, no bounds | `impl target 'X' is not a declared struct` |
| Traits | Default methods need an explicit return type | `trait default method 'T::m' without an explicit return type` |
| Aliases | No function-type aliases | parse error |
| Assignment | Only to mutable, non-captured locals; no field/index assignment | `assignment target must currently be a variable` |
| Strings | Restricted interpolation grammar | `invalid string interpolation expression` |
| Ranges | Eagerly materialized, capped at 1,000,000 elements | `range exceeds the one-million element safety limit` |
| Entry point | `main` takes no parameters | `entry point 'main' must take no parameters` |
| Keywords | `as` and `unsafe` are reserved with no grammar | parse error |
| WebAssembly | See [§16.2](#162-rejected-by-the-webassembly-backend) | `unsupported WebAssembly operation: …` |
| Stdlib | `std::freestanding*` and `std::mobile` are in-process simulations | — |
| Packages | The default registry host `registry.titan-lang.org` is a CLI default; local packing, signing, verification and path dependencies work offline | — |

---

## 19. How this document was verified

Every statement above was checked against the source tree at compiler version `1.0.0`:

| Claim | Source of truth |
|---|---|
| Tokens, keywords, comments, escapes | `crates/titan_lexer/src/lib.rs` |
| Grammar, desugaring of `\|>`, `<=>`, `#{}`, spread, destructuring | `crates/titan_parser/src/lib.rs` |
| Item and expression shapes | `crates/titan_ast/src/lib.rs`, `crates/titan_ast/src/expr.rs` |
| Types, inference, exhaustiveness, every rejection | `crates/titan_typechecker/src/lib.rs` (`TypeError`, `UnsupportedFeature` sites) |
| Opcodes, lowering, codegen rejections | `crates/titan_codegen/src/lib.rs` (`Op`, `CodegenError::Unsupported` sites) |
| Container format, limits, validation | `crates/titan_codegen/src/artifact.rs` |
| Runtime semantics, budgets, capabilities, concurrency | `crates/titan_vm/src/lib.rs` (`VmError`, `RuntimeCapabilities`, `make_range`) |
| Collector behavior | `crates/titan_gc/src/lib.rs` |
| Native registry contents | `crates/titan_stdlib/src/native.rs`, cross-checked with `verify_phase34.py` |
| WebAssembly subset and rejections | `crates/titan_wasm/src/lib.rs` (`WasmError`, `emit_operation`) |
| Project loading and imports | `crates/titan_pkg/src/project.rs` |
| CLI surface | `crates/titan_cli/src/main.rs` |

Reproduce the checks with:

```bash
python3 verify_phase34.py                  # native table and call sites, no Rust build needed
cargo test --workspace --all-targets       # 637 declared Rust tests
cargo clippy --workspace --all-targets -- -D warnings
```

Related documents: [`ARCHITECTURE.md`](ARCHITECTURE.md) (compiler internals),
[`TITAN_SYNTAX.md`](TITAN_SYNTAX.md) (practical guide with worked examples),
[`STDLIB.md`](STDLIB.md) (full native API), [`WASM.md`](WASM.md),
[`CONCURRENCY.md`](CONCURRENCY.md), [`PROJECTS.md`](PROJECTS.md),
[`VALIDATION.md`](VALIDATION.md) (verification log).

When the implementation and this document disagree, the implementation is correct and this
document is a bug. Report it as such.

# Titan Language Specification

**Language version:** 0.2

**Canonical source extension:** `.titan`

## 1. Program structure

A program consists of declarations. Execution begins at `fn main()`.

```titan
const LIMIT: int = 20

fn main() {
    print("limit={LIMIT}")
}
```

Functions may declare typed parameters and a return type. Types may be omitted where the checker can safely retain an unknown/inferred type. Function names must be unique in the current executable module.

## 2. Lexical rules

- Identifiers are Unicode alphabetic characters followed by Unicode alphanumeric characters or `_`.
- `//` starts a line comment; `/* ... */` comments may nest.
- Strings support `\n`, `\r`, `\t`, `\0`, `\"`, `\'`, and `\\`.
- Statements may be separated by semicolons or by syntax boundaries/newlines.
- Integer separators such as `1_000` are accepted.
- Ranges use `start..end` or inclusive `start..=end`.

## 3. Built-in types

| Type | Meaning |
|---|---|
| `int`, `i32`, `i64`, `u64`, `usize` | VM signed 64-bit integer today |
| `float`, `f32`, `f64` | VM IEEE-754 64-bit float today |
| `bool` | `true` or `false` |
| `char` | Unicode scalar |
| `string` | UTF-8 string |
| `[T]` | array/slice type syntax |
| `(A, B)` | tuple |
| `Name` | user-defined type |
| `nil` | absence/unit-compatible runtime value |

Integer arithmetic is checked. Division by zero, overflow and bad indexing are runtime errors rather than silent undefined behavior.

## 4. Variables and operators

```titan
let value: int = 10
value += 2
value = value * 3
```

Precedence, high to low: unary; `* / %`; `+ -`; ordered comparisons; equality; bitwise `& ^ |`; logical `&& ||`; ranges; assignment.

## 5. Control flow

```titan
if condition { ... } else { ... }
while condition { ... }
loop { ... }
for item in 0..10 { ... }
match value { true => 1, false => 0 }
```

`break` and `continue` are only legal inside loops. `return` is checked against the function return type. Boolean matches without a wildcard must cover both values.

## 6. Functions and recursion

```titan
fn fib(n: int) -> int {
    if n <= 1 { return n }
    fib(n - 1) + fib(n - 2)
}
```

Calls have checked arity and execute with isolated VM frames. Recursion is limited to prevent host stack exhaustion.

## 7. Aggregate values

```titan
struct Point { x: int, y: int }
let point = Point { x: 2, y: 3 }
print(point.x)

enum Maybe { None, Some(int) }
let value = Maybe::Some(42)
match value {
    Maybe::Some(n) => n,
    Maybe::None => 0,
}
```

Struct construction validates required/unknown fields. Enums currently support unit variants and variants carrying one value. Enum, literal, wildcard and binding patterns execute in the bytecode VM. Nested tuple/struct and or-pattern execution remains reserved and is rejected explicitly.

## 8. Strings

Strings interpolate variables and simple named calls:

```titan
print("fib({i}) = {fib(i)}")
```

Interpolation expressions intentionally use a restricted grammar in version 0.2: an identifier, or a named function call whose arguments are local identifiers or integer literals.

## 9. Modules, traits and imports

The parser and AST define:

```titan
mod geometry { ... }
import geometry::Point
trait Display { fn display(self) -> string; }
impl Display for Point { ... }
```

Nested module declarations and impl methods are collected by the compiler. Cross-file imports resolve `.titan` files, directory `mod.titan` files, and local path dependencies from `Titan.toml`; imports are canonicalized, loaded once, constrained to authorized source roots, and checked for cycles. Declarations currently enter one executable namespace, so duplicate names are rejected rather than silently shadowed.

Trait dispatch, qualified symbol namespaces and generic monomorphization remain **reserved features**: valid syntax is preserved, but code requiring unavailable runtime behavior receives an explicit compile error. This is preferable to silently miscompiling it.

## 10. Concurrency, closures and references

`spawn`, closure, reference and advanced generic syntax have AST representation for forward compatibility. Their ownership/runtime semantics are not standardized in language version 0.2 and executable use is rejected. They must not be treated as stable features yet.

## 11. Native standard-library calls

Registered host functions use qualified names:

```titan
let bytes = std::encoding::utf8_encode("Titan")
let encoded = std::encoding::base64_encode(bytes)
let data = std::json::parse("{\"ok\":true}")
print(data.ok)
```

The compiler resolves these names through a shared registry. Arity and parameter types are checked before code generation. Native failures become runtime errors that include the function name. Values exchanged with natives include strings, arrays, tuples, bytes, maps and scalar values.

Filesystem, process, network and environment calls require VM capabilities. Normal CLI execution enables them; `titan run --sandbox` denies them. Pure text, encoding, JSON, collection, math and statistics calls remain available in sandbox mode.

The complete registered API is documented in `STDLIB.md`.

## 12. Execution model

Titan compiles to versioned stack bytecode. The VM has:

- isolated locals per call frame;
- checked operations and structured runtime errors;
- call-depth and instruction limits;
- bounded range materialization;
- no native pointer instructions or Rust `unsafe` execution.

`build` emits inspectable `TITAN-BYTECODE 1` text. `run` compiles and executes source. A portable binary loader is not part of version 0.2.

# Titan Projects, Modules, and Tests

> **Nota:** este documento se escribió para el prototipo en Rust. La API de Titan que describe (funciones, argumentos, resultados)
> sigue siendo la del lenguaje, pero las referencias a la VM, a hilos del sistema, a «capabilities» o a bibliotecas de Rust ya no aplican:
> hoy todo está en Titan y no hay VM al ejecutar. Lo vigente y verificado está en `selfhost/ESTADO.md`.

## Project layout

```text
hello/
├── Titan.toml
├── Titan.lock
├── src/
│   ├── main.titan
│   ├── math.titan
│   └── models/
│       └── mod.titan
└── tests/
    └── arithmetic.titan
```

Create and use a project:

```bash
titan new hello
titan new hello --path /tmp/apps   # el proyecto queda en /tmp/apps/hello
cd hello
titan check
titan run
titan build
titan test
```

`build`, `check`, and `run` accept either a `.titan` entry file or a project directory. From a project directory they select `src/main.titan`. Build artifacts are written to `target/<project>.tbc` unless `--output` is supplied.

## Imports

Given `src/math.titan`:

```titan
fn double(value: int) -> int { value * 2 }
```

`src/main.titan` can load it once with:

```titan
import math
fn main() { print(double(21)) }
```

Nested paths resolve in this order:

- `import models` → `src/models.titan` or `src/models/mod.titan`;
- `import models::user` → `src/models/user.titan` or `src/models/user/mod.titan`;
- if the complete path is not a file, the longest file prefix is used, allowing a trailing imported symbol.

Files are canonicalized before loading. Duplicate imports are loaded once. Circular imports report the complete cycle. A resolved import must remain inside the project source root or an explicitly declared dependency source root, preventing `..`/symlink escapes.

`std::...` calls are native registry names and do not cause source-file loading.

## Local dependencies

Version 0.2 resolves local path dependencies. Network registries are deliberately rejected rather than silently ignored.

```toml
[package]
name = "web_app"
version = "0.1.0"
edition = "2021"

[dependencies.shared]
path = "../shared"
version = "0.3.0"
```

Import the dependency alias:

```titan
import shared
```

A dependency entry loads `shared/src/lib.titan`; `import shared::models` loads a nested module. Transitive local dependencies are collected, package cycles are detected, and every dependency must have a valid `Titan.toml` with a semantic version.

`Titan.lock` is deterministic JSON. It records sorted dependency aliases, versions, and canonical `path+...` sources. Commands regenerate it from the resolved graph.

## Tests

Every `.titan` file under `tests/` is an independent test program with its own `fn main()`. A successful return is a pass; a compile error, a runtime error, a non-zero exit code, or a failed `std::testing` assertion is a failure.

```titan
import math

fn main() {
    std::testing::assert_eq(double(21), 42, "double should multiply by two")
}
```

Run recursively and deterministically:

```bash
titan test
```

```text
test tests/math.titan ... ok

test result: ok. 1 passed; 0 failed
```

Two rules that follow from "every file is a program":

- A file with **no `fn main()`** is not a test — it is an auxiliary module meant to be imported by
  the others. `titan test` reports it as `skipped (no entry point 'main'; auxiliary module)` and it
  does not count as a failure; the summary then adds `; N skipped`.
- A test may import **both** the project's `src/` modules and its siblings in `tests/`. Both
  directories are import roots, and the most specific one containing the importing file wins: from
  `tests/math.titan`, `import util` resolves to `tests/util.titan` if it exists and to
  `src/util.titan` otherwise. Neither root lets an import escape the project.

`std::testing::assert(condición, mensaje)` and `std::testing::assert_eq(izq, der, mensaje)` abort the
program with a runtime error that names the values, so the failing test reports which assertion
broke:

```text
test tests/math.titan ... FAILED
  RUNTIME ERROR: native function 'std::testing::assert_eq' failed: assertion failed: double should multiply by two; left=21, right=42
```

`titan test` exits 0 only when no test failed.

## Current namespace rule

Loaded declarations currently share the executable module namespace. Duplicate function names are rejected by codegen. Imports control loading and dependency boundaries, but aliases do not yet qualify declarations (`shared::function`) at runtime. Full symbol namespaces require HIR name resolution and are tracked as a separate language feature; the loader does not pretend they already exist.

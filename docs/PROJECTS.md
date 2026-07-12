# Titan Projects, Modules, and Tests

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

Every `.titan` file under `tests/` is an independent test program with its own `fn main()`. A successful return is a pass; a compile error, runtime error, failed native assertion, or denied capability is a failure.

```titan
import math

fn main() {
    std::testing::assert_eq(double(21), 42, "double should multiply by two")
}
```

Run recursively and deterministically:

```bash
titan test
titan test --sandbox
```

Sandbox mode permits pure standard-library functions but rejects filesystem, process, network, and environment natives.

## Current namespace rule

Loaded declarations currently share the executable module namespace. Duplicate function names are rejected by codegen. Imports control loading and dependency boundaries, but aliases do not yet qualify declarations (`shared::function`) at runtime. Full symbol namespaces require HIR name resolution and are tracked as a separate language feature; the loader does not pretend they already exist.

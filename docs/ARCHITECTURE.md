# Arquitectura de Titan

Todo el compilador, el runtime y la biblioteca estándar están escritos en Titan y viven en `selfhost/`. No hay código
Rust en el repositorio (el prototipo original sigue en el historial de git, etiqueta `ultimo-con-rust`).

## Del código fuente al ejecutable

```text
programa.titan
  → lexer.titan          tokens con posiciones
  → parser.titan         sintaxis → AST (ast.titan)
  → typechecker.titan    ámbitos, firmas, tipos
  → codegen.titan        AST → bytecode de pila (la representación intermedia)
  → opt.titan            optimizador sobre el bytecode
        ┌──────────────────────────┼──────────────────────────┐
        ▼                          ▼                          ▼
  native/backend.titan      native/llvm.titan            wasm.titan
  x86-64 directo (ELF)      LLVM IR (x86-64 y ARM64)     WebAssembly
```

- **Proyectos:** `loader.titan` y `pkg.titan` descubren `Titan.toml`, resuelven dependencias locales y remotas, siguen los
  `import` (detectan ciclos y rutas que escapan del árbol) y entregan un único programa a la tubería.
- **Bytecode:** `titan build` escribe un artefacto `TITAN-BYTECODE 1` (cabecera, versión, CRC-32) y `titan exec` lo valida
  (funciones, saltos, locales, llamadas) antes de ejecutarlo (`artifact.titan`, `bytecode.titan`).
- **No hay máquina virtual al ejecutar.** `titan run` compila el programa a código nativo y lo ejecuta; el bytecode es solo
  la representación intermedia entre el front-end y los backends.

## Backends

| Backend | Archivo | Qué produce | Notas |
|---|---|---|---|
| x86-64 propio | `native/backend.titan`, `native/x64.titan`, `native/elf.titan` | ELF estático x86-64, sin libc ni enlazador | Es el que usa `titan run/compile` en x86-64. |
| LLVM | `native/llvm.titan`, `llvm_build.titan` | LLVM IR, que `clang` + `lld` convierten en ELF estático (x86-64 o ARM64) | Es el que usa `titan` en ARM64 y `--target aarch64`. También `aarch64-none` (sin sistema operativo). |
| WebAssembly | `wasm.titan`, `wasm_cmd.titan` | módulo `.wasm` con source maps | El host JS de `std::web` está en `native/`. |

Los dos backends nativos comparten el **runtime** (`native/runtime.titan` y los `native/std_*.titan`): asignador con
conteo de referencias, cadenas, arrays, mapas, tareas, red, etc. Está escrito en Titan y se compila junto con cada programa
(con una caché `.runtime-cache-*.json` dentro de `native/`). Habla con el sistema operativo por llamadas al sistema de
Linux, sin libc. En ARM64, `native/sys_arm64.titan` traduce las llamadas al sistema de x86-64 a las de ARM64.

## Biblioteca estándar

`natives.titan` lista las firmas de las funciones `std::*` (el typechecker las consulta) y `codegen_tables.titan` dice cuáles
se convierten en instrucciones propias del bytecode. El cuerpo de cada función está en `selfhost/native/` (SQLite, TLS,
regex, compresión, audio, ONNX, tokenizadores, imágenes, PDF…). Hoy son 812 funciones en 73 espacios de nombres; la medición
se repite con `bash selfhost/native/cobertura.sh -v`. Ver [STDLIB.md](STDLIB.md).

## Herramientas

`titan.titan` es la CLI (`new`, `check`, `run`, `build`, `exec`, `compile`, `wasm`, `test`, `repl`, `debug`, `add`, `fetch`,
`update`, `keygen`, `pack`, `publish`, `lsp`, `dap`, `version`). Servidor de lenguaje: `lsp.titan`. Adaptador de depuración:
`dap.titan` y `native/dap_core.titan`.

## Cómo se construye a sí mismo

`bash selfhost/bootstrap.sh`: una **semilla** (el compilador x86-64 ya compilado, `selfhost/semilla/`) compila el compilador
escrito en Titan (`titanc1`); éste se compila (`titanc2`) y otra vez (`titanc3`); las tres tienen que ser idénticas byte a
byte. Después se construye la CLI `selfhost/titan`. Ver `selfhost/semilla/LEEME.md`.

## Estado y límites

El estado detallado, con lo verificado y lo que no, está en [`selfhost/ESTADO.md`](../selfhost/ESTADO.md).

# TITAN / Zett

[![CI](https://github.com/alexsndersoto04-source/aio/actions/workflows/ci.yml/badge.svg)](https://github.com/alexsndersoto04-source/aio/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/alexsndersoto04-source/aio?label=release\&color=brightgreen)](https://github.com/alexsndersoto04-source/aio/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**TITAN** es un lenguaje de programación compilado y verificado estáticamente, **escrito en sí mismo**: el compilador, la biblioteca estándar y las herramientas están en Titan y no queda código Rust en el repositorio (Titan nació de un prototipo en Rust, que sigue en el historial de git). Los programas usan la extensión **`.titan`** y se compilan a ejecutables nativos x86-64 (también a WebAssembly). **Zett** es el nombre de distribución del compilador, especialmente en Android/Termux; ambos nombres se refieren al mismo ecosistema.

> **Estado:** self-hosting completo. El compilador se compila a sí mismo (punto fijo verificado) y la biblioteca estándar (812 funciones nativas en 72 namespaces `std::*`) está escrita en Titan. Los binarios antiguos de la release [v1.0.0](https://github.com/alexsndersoto04-source/aio/releases/tag/v1.0.0) (Linux, macOS, Windows, Termux) son de la versión en Rust y no se actualizan.

```text
TITAN source (.titan)
        │
        ▼
 lexer → parser → AST → typechecker → codegen → bytecode
                                                │
                              ┌─────────────────┴──────────────┐
                              ▼                                ▼
                  ejecutable nativo x86-64               WebAssembly
```

## ¿Qué incluye?

TITAN es más que un intérprete de ejemplos. El repositorio reúne un lenguaje, runtime, tooling, backend web y una plataforma estándar para programas de sistema, datos, web y dispositivos móviles.

- **Lenguaje tipado:** funciones, closures, structs, enums, `match`, módulos, imports, constantes, aliases, arrays, mapas, pipelines, rangos, interpolación y manejo de `Option` / `Result`.
- **Bytecode validado:** artefactos `.tbc` versionados con cabecera, CRC-32, límites de tamaño y validación de saltos, aridad, locales, capturas y llamadas nativas antes de ejecutar.
- **Ejecutables nativos:** `titan compile`/`titan run` generan código x86-64 directamente (backend propio) o vía LLVM (x86-64 y ARM64); los errores de ejecución (overflow, división por cero, índices, aridad…) se comunican con los mismos mensajes que definía la VM original.
- **Runtime concurrente:** `spawn`, `join`, cancelación cooperativa, canales acotados, timeouts y `select`, sobre fibras cooperativas del runtime nativo.
- **Runtime operativo:** cuotas de memoria por tarea, recolección manual, umbral de GC configurable, heap dump JSON, tareas activas, fast-paths enteros y benchmark integrado.
- **WebAssembly real:** `titan wasm` emite módulos WASM con source maps, memoria lineal, strings UTF-8, arrays, mapas, structs, enums y control de flujo nativo.
- **Navegador:** integración opcional con DOM, eventos, `fetch`, WebSocket, Canvas 2D, animación y WebGL2 mediante un host JavaScript real.
- **Herramientas:** CLI, REPL, proyectos multiarchivo, paquetes firmados, LSP, DAP y depurador interactivo por línea fuente.
- **Biblioteca estándar amplia:** texto, JSON, bytes, archivos, procesos, HTTP/HTTPS, TLS, WebSockets, bases de datos, métricas, IA local, GUI, audio y Android/Termux.

## Instalación

Titan se construye desde las fuentes, sin Rust ni ninguna otra herramienta, a partir de una **semilla**: el
compilador nativo ya compilado (650 KB) que está en `selfhost/semilla/` (ver `selfhost/semilla/LEEME.md`).
Hoy el compilador es para **Linux x86-64**. macOS, Windows y ARM64/Termux todavía no tienen empaquetado (el
backend ARM64 existe vía LLVM, pero no está empaquetado).

```bash
git clone https://github.com/alexsndersoto04-source/aio.git
cd aio
bash selfhost/bootstrap.sh      # desempaqueta la semilla, repite el punto fijo y construye selfhost/titan
selfhost/titan version
```

### Primera ejecución

Crea un programa pequeño:

```bash
cat > hi.titan <<'EOF'
fn main() {
    let cpus = std::procfs::cpu_count()
    print("TITAN detecta {cpus} CPU(s)")
}
EOF

selfhost/titan run hi.titan
```

## Compilar el compilador

`bash selfhost/bootstrap.sh` ejecuta el **punto fijo**: la semilla compila el compilador escrito en Titan
(`titanc1`), éste se compila a sí mismo (`titanc2`) y otra vez (`titanc3`); las tres tienen que ser idénticas byte a
byte. Después se construye la CLI `selfhost/titan` (`titan new`, `check`, `run`, `test`, `compile`, `wasm`,
`build`, `exec`, `add`, `fetch`, `lsp`, `dap`, `repl`, `debug`...).

```bash
selfhost/titan new hola_titan
cd hola_titan
../selfhost/titan check
../selfhost/titan run
```

## Un programa TITAN

```titan
fn factorial(n: int) -> int {
    if n <= 1 { return 1 }
    n * factorial(n - 1)
}

fn main() {
    let total = 0
    for i in 1..=5 {
        total += factorial(i)
    }
    print("total = {total}")
}
```

El núcleo ejecutable soporta, entre otras capacidades:

- `int`, `float`, `bool`, `char`, `string`, `nil`, bytes, arrays, tuplas y mapas;
- variables, asignación y operadores aritméticos, lógicos, bitwise y de comparación;
- `if`, `match`, `while`, `loop`, `for`, rangos, `break`, `continue` y `return`;
- funciones tipadas, parámetros por defecto, recursión y aridad comprobada;
- closures y capturas léxicas deterministas;
- `Option::Some` / `None`, `Result::Ok` / `Err`, `?` y `std::try::catch`;
- structs, enums con payload, métodos `impl`, traits con métodos por defecto y type aliases;
- imports recursivos, dependencias locales y proyectos con `Titan.toml`;
- arrays funcionales: `map`, `filter`, `fold`, `sort_by`, `find`, `any` y `all`.

Algunas construcciones con sintaxis reservada —por ejemplo, determinadas formas de destructuring, or-patterns, referencias y genéricos— se rechazan explícitamente cuando todavía no tienen semántica completa en el codegen o la VM. TITAN prefiere un error claro antes que generar código incorrecto. Consulta la [especificación](docs/SPEC.md) y la [referencia de sintaxis](docs/TITAN_SYNTAX.md).

## Runtime para aplicaciones concurrentes y operativas

La Fase 36–40 incorporó herramientas que hacen visible el estado de una aplicación TITAN en ejecución.

```titan
fn main() {
    let task = std::runtime::spawn_quota(50000, || {
        // Esta tarea posee una cuota de memoria independiente.
        42
    })

    let result = join(task)
    let metrics = std::runtime::benchmark(1000, || { 20 * 21 })

    print("resultado = {result}")
    print("ops/s = {metrics.ops_per_sec}")
}
```

### Capacidades de las fases enterprise

| Fase | Capacidades reales |
|---|---|
| **36** | Métricas thread-safe: contadores, gauges, histogramas, snapshots y exportación Prometheus/OpenMetrics. |
| **37** | Pools y health checks para SQLite, PostgreSQL y MySQL; API común `std::db`. |
| **38** | Cuotas de memoria por tarea, memoria asignada, objetos vivos y recolección explícita. |
| **39** | Umbral de GC configurable, tareas activas y `heap_dump(path)` en JSON. |
| **40** | Fast-paths de enteros en la VM y `std::runtime::benchmark`. |

El runtime también incluye `spawn`, `join`, `join_timeout`, `cancel`, `channel`, `send`, `recv`, `recv_timeout` y `select`. Las tareas usan threads reales del host y aislación de VM; no se presentan como async cooperativo cuando no lo son. Más detalles: [concurrencia y runtime](docs/CONCURRENCY.md) y [métricas](docs/METRICS.md).

## Biblioteca estándar

La biblioteca estándar ofrece **812 funciones en 72 namespaces `std::*`, todas con cuerpo escrito en Titan** (medido con `selfhost/native/cobertura.sh`). Las limitaciones de cada pieza están declaradas en `selfhost/ESTADO.md`.

| Área | Incluye |
|---|---|
| Texto, datos y formatos | Unicode, regex, encoding, bytes, checksum, JSON, CSV, YAML, XML, URL, UUID, gzip/zstd y TAR/ZIP. |
| Seguridad | SHA, SHA-3, BLAKE3, HMAC, ChaCha20-Poly1305, AES-GCM, Argon2id, bcrypt y JWT. |
| Red | HTTP/HTTPS, TLS 1.2/1.3 propio con validación X.509, DNS, SMTP, multipart, WebSocket, servidor HTTP y router. |
| Datos | SQLite (motor SQL propio), PostgreSQL, MySQL, migraciones, pools, KV y Redis. |
| Sistema | Archivos, paths, procesos, señales POSIX, filesystem watcher, procfs, cache, métricas y variables de entorno. |
| Terminal y multimedia | TUI, colores, teclado, readline, progreso, imágenes PNG/JPEG/WebP/BMP/GIF, QR, SVG charts y WAV. |
| IA local | Tokenizers HuggingFace, motor ONNX propio, BERT multi-input (modelos pequeños), embeddings y matemáticas vectoriales. |
| UI y dispositivos | Motor 2D, GUI retenida con rasterizador software, ventanas live, entrada, lifecycle móvil y Termux:API. |
| WebAssembly | Heap WASM, source maps, DOM, eventos, fetch, WebSocket, Canvas 2D, animación y WebGL2 mediante host web. |

Funciones con efectos se protegen mediante capacidades del runtime:

```text
Filesystem · Process · Network · Environment
```

La opción `--sandbox` pertenecía a la VM de Rust y **no existe** en los ejecutables nativos. Consulta la [referencia de stdlib](docs/STDLIB.md).

## Proyectos, paquetes y CLI

Un proyecto TITAN tiene una estructura simple:

```text
mi_app/
├── Titan.toml
├── Titan.lock
├── src/
│   ├── main.titan
│   └── util.titan
└── tests/
    └── suma.titan
```

Comandos principales:

```text
titan new <directorio>                 Crear un proyecto
titan check [archivo|proyecto]         Parsear y comprobar tipos
titan run [archivo|proyecto]           Compilar y ejecutar
titan build [archivo|proyecto]         Crear bytecode .tbc validado
titan exec <archivo.tbc>               Validar y ejecutar bytecode existente
titan wasm [archivo|proyecto]          Generar WebAssembly
titan test [proyecto]                  Ejecutar tests/*.titan
titan debug [ruta] -b archivo:línea    Depurador interactivo
titan repl                             REPL
titan add/fetch/update                 Dependencias remotas
titan keygen/pack/publish              Paquetes .tpkg firmados con Ed25519
titan version                          Versión del compilador
```

`build`, `check` y `run` aceptan un archivo `.titan` o una carpeta de proyecto. Los imports se canonicalizan, se detectan ciclos y no pueden escapar del árbol de fuentes autorizado. Los paquetes remotos se resuelven por HTTPS, verifican SHA-256 y firmas Ed25519. Lee [proyectos y módulos](docs/PROJECTS.md) y el [registro de paquetes](docs/PACKAGE_REGISTRY.md).

## Bytecode, depuración y herramientas

`build` produce un contenedor `TITAN-BYTECODE 1`. Antes de ejecutar con `titan exec`, el runtime valida formato, checksum, tamaño, funciones, instrucciones, strings, locales, saltos, llamadas, capturas y nativas.

El depurador de terminal permite:

- breakpoints por instrucción o `archivo:línea`;
- continuar, pausar, step in, step over y step out;
- inspección de frames, locales, captures y pila;
- source maps preservados en bytecode.

El workspace también incluye:

- **LSP:** diagnósticos, símbolos, definición, referencias, rename, semantic tokens y signature help;
- **DAP:** base de Debug Adapter Protocol para clientes compatibles;
- **WebAssembly source maps:** mapa TITAN propio y formato estándar para host/browser.

Documentación: [debugger](docs/DEBUGGER.md), [LSP](docs/LSP.md), [DAP](docs/DAP.md) y [WASM](docs/WASM.md).

## Arquitectura

Todo está en `selfhost/`: `lexer.titan`, `parser.titan`, `typechecker.titan`, `codegen.titan` (bytecode),
`opt.titan` (optimizador), `native/` (backend x86-64, runtime y biblioteca estándar: 812 funciones `std::*`
escritas en Titan, más SQLite, TLS, audio, ONNX...), `native/llvm.titan` (backend LLVM), `wasm.titan`,
`loader.titan`/`pkg.titan` (proyectos y paquetes), `lsp.titan`, `dap.titan` y `titan.titan` (la CLI).
El estado detallado, con lo verificado y las limitaciones declaradas, está en `selfhost/ESTADO.md`.

Consulta [la arquitectura completa](docs/ARCHITECTURE.md) (describe el diseño original en Rust).

## Ejemplos incluidos

```bash
# Núcleo y lenguaje
titan run examples/hello.titan
titan run examples/fibonacci.titan
titan run examples/impl_structs.titan
titan run examples/pipeline_spaceship.titan

# Runtime y operación
titan run examples/enterprise_metrics.titan
titan run examples/enterprise_pool.titan
titan run examples/enterprise_runtime.titan
titan run examples/enterprise_profiler.titan
titan run examples/enterprise_benchmark.titan

# Capacidades de la stdlib
titan run examples/security.titan
titan run examples/database.titan
titan run examples/webserver.titan
titan run examples/charts.titan
titan run examples/tokenizer.titan
titan run examples/onnx.titan
titan run examples/vector_search.titan
```

Algunos ejemplos requieren recursos externos o del sistema: internet, un servidor de base de datos, Termux:API, una pantalla para ventana live, modelos ONNX o memoria adicional. Revísalos antes de ejecutarlos en producción.

## Desarrollo y validación

```bash
bash selfhost/verify_fixpoint.sh               # punto fijo del compilador (con la semilla)
bash selfhost/native/cobertura.sh -v           # cuántas funciones std:: tienen cuerpo en Titan
selfhost/titan test                            # tests de un proyecto
```

Las pruebas de `selfhost/tests/` se escribieron comparando contra la implementación original en Rust; ese
oráculo ya no está en el repositorio. Para repetirlas hay que construir la última versión con Rust
(`git checkout ultimo-con-rust`, commit `5dbb238`) y ver `selfhost/tests/LEEME.md`.

## Documentación

- [Arquitectura](docs/ARCHITECTURE.md)
- [Especificación del lenguaje](docs/SPEC.md)
- [Referencia de sintaxis](docs/TITAN_SYNTAX.md)
- [Biblioteca estándar](docs/STDLIB.md)
- [Proyectos, módulos y tests](docs/PROJECTS.md)
- [Concurrencia y runtime](docs/CONCURRENCY.md)
- [WebAssembly](docs/WASM.md)
- [Networking, HTTP, TLS y WebSockets](docs/NETWORKING.md)
- [Bases de datos](docs/DATABASE_API.md)
- [Paquetes y registry](docs/PACKAGE_REGISTRY.md)
- [LSP, DAP y debugger](docs/LSP.md)

## Licencia

TITAN/Zett se distribuye bajo la licencia [MIT](LICENSE).

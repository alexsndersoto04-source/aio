# TITAN / Zett

[![CI](https://github.com/alexsndersoto04-source/aio/actions/workflows/ci.yml/badge.svg)](https://github.com/alexsndersoto04-source/aio/actions/workflows/ci.yml)
[![cross-platform CI](https://github.com/alexsndersoto04-source/aio/actions/workflows/cross-platform.yml/badge.svg)](https://github.com/alexsndersoto04-source/aio/actions/workflows/cross-platform.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**TITAN** es un lenguaje de programación compilado y verificado estáticamente, implementado en Rust. Los programas usan la extensión **`.titan`**, se compilan a bytecode portable y se ejecutan en una máquina virtual de pila segura. **Zett** es el nombre de distribución del compilador, especialmente en Android/Termux; ambos nombres se refieren al mismo ecosistema.

> **Estado del código fuente:** Fase 40. El núcleo conecta lexer, parser, AST, comprobación de tipos, generación de bytecode, VM, biblioteca estándar, herramientas de desarrollo y backend WebAssembly. El registro contiene **758 funciones nativas únicas en 72 namespaces `std::*`**, además de primitivas especializadas del runtime. Los binarios publicados actualmente están disponibles en la serie **v0.35.0**; la metadata de la próxima distribución se alineará con las fases posteriores.

```text
TITAN source (.titan)
        │
        ▼
 lexer → parser → AST → typechecker → codegen → bytecode / WebAssembly
                                                │
                                                ▼
                                           safe Titan VM
```

## ¿Qué incluye?

TITAN es más que un intérprete de ejemplos. El repositorio reúne un lenguaje, runtime, tooling, backend web y una plataforma estándar para programas de sistema, datos, web y dispositivos móviles.

- **Lenguaje tipado:** funciones, closures, structs, enums, `match`, módulos, imports, constantes, aliases, arrays, mapas, pipelines, rangos, interpolación y manejo de `Option` / `Result`.
- **Bytecode validado:** artefactos `.tbc` versionados con cabecera, CRC-32, límites de tamaño y validación de saltos, aridad, locales, capturas y llamadas nativas antes de ejecutar.
- **VM segura:** errores tipados para overflow, división por cero, índices, pila, aridad, recursión, límites de instrucciones y permisos.
- **Sandbox por capacidades:** `--sandbox` bloquea filesystem, procesos, red y environment sin desactivar las funciones puras.
- **Runtime concurrente:** tareas sobre threads del host, `spawn`, `join`, cancelación cooperativa, canales acotados, timeouts y `select`.
- **Runtime operativo:** cuotas de memoria por tarea, recolección manual, umbral de GC configurable, heap dump JSON, tareas activas, fast-paths enteros y benchmark integrado.
- **WebAssembly real:** `titan wasm` emite módulos WASM con source maps, memoria lineal, strings UTF-8, arrays, mapas, structs, enums y control de flujo nativo.
- **Navegador:** integración opcional con DOM, eventos, `fetch`, WebSocket, Canvas 2D, animación y WebGL2 mediante un host JavaScript real.
- **Herramientas:** CLI, REPL, proyectos multiarchivo, paquetes firmados, LSP, DAP y depurador interactivo por línea fuente.
- **Biblioteca estándar amplia:** texto, JSON, bytes, archivos, procesos, HTTP/HTTPS, TLS, WebSockets, bases de datos, métricas, IA local, GUI, audio y Android/Termux.

## Instalación

Los binarios precompilados se publican en [**Releases**](https://github.com/alexsndersoto04-source/aio/releases/latest). En paquetes de distribución el ejecutable se llama `zett`; al compilar directamente desde esta fuente, Cargo genera el binario `titan`.

### Linux x86-64

```bash
curl -L https://github.com/alexsndersoto04-source/aio/releases/download/v0.35.0/zett-linux-x86_64.tar.gz | tar xz
./zett version
```

### Linux ARM64 y ARMv7

```bash
# ARM de 64 bits
curl -L https://github.com/alexsndersoto04-source/aio/releases/download/v0.35.0/zett-linux-aarch64.tar.gz | tar xz

# ARM de 32 bits hard-float; útil, por ejemplo, en proot Debian armhf + Termux:X11
curl -L https://github.com/alexsndersoto04-source/aio/releases/download/v0.35.0/zett-linux-armv7hf.tar.gz | tar xz

./zett version
```

### macOS Apple Silicon

```bash
curl -L https://github.com/alexsndersoto04-source/aio/releases/download/v0.35.0/zett-macos-arm64.tar.gz | tar xz
xattr -d com.apple.quarantine zett 2>/dev/null || true
./zett version
```

### Windows x86-64

Descarga `zett-windows-x86_64.zip` desde [Releases](https://github.com/alexsndersoto04-source/aio/releases/latest), descomprímelo y ejecuta:

```powershell
.\zett.exe version
```

### Android / Termux

Zett también se distribuye mediante el repositorio APT del proyecto:

```bash
echo 'deb [trusted=yes] https://raw.githubusercontent.com/alexsndersoto04-source/aio/zett-repo ./ ' \
  > "$PREFIX/etc/apt/sources.list.d/zett.list"
pkg update
pkg install zett
zett --help
```

Para las integraciones Android de `std::termux::*`, instala también la app **Termux:API** y su paquete de comandos:

```bash
pkg install termux-api
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

zett run hi.titan       # Usa ./titan si compilaste desde el código fuente.
```

## Compilar desde el código fuente

Requisitos: Rust estable reciente, `rustfmt` y `clippy`.

```bash
git clone https://github.com/alexsndersoto04-source/aio.git
cd aio
cargo build --release -p titan_cli

# Cargo genera target/release/titan.
target/release/titan version
target/release/titan run examples/hello.titan
```

Instalación local opcional:

```bash
cargo install --path crates/titan_cli

titan new hola_titan
cd hola_titan
titan check
titan run
titan build
titan test
```

> El build completo con las features predeterminadas incorpora ONNX, tokenizers, imágenes, bases de datos, TLS y otras dependencias grandes. En Termux o dispositivos ARM con poco espacio, libera almacenamiento antes de ejecutar todas las pruebas; `target/` puede ocupar varios GB durante compilación y enlace.

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

La biblioteca estándar ofrece **758 funciones nativas registradas en 72 namespaces**. Las features opcionales se agrupan bajo `extras` y están activadas por defecto en la CLI de distribución.

| Área | Incluye |
|---|---|
| Texto, datos y formatos | Unicode, regex, encoding, bytes, checksum, JSON, CSV, YAML, XML, URL, UUID, gzip/zstd y TAR/ZIP. |
| Seguridad | SHA, SHA-3, BLAKE3, HMAC, ChaCha20-Poly1305, AES-GCM, Argon2id, bcrypt y JWT. |
| Red | HTTP/HTTPS, TLS con rustls/WebPKI, DNS, SMTP, multipart, WebSocket, servidor HTTP y router. |
| Datos | SQLite, PostgreSQL, MySQL, migraciones, pools, KV ACID mediante sled y Redis. |
| Sistema | Archivos, paths, procesos, señales POSIX, filesystem watcher, procfs, cache, métricas y variables de entorno. |
| Terminal y multimedia | TUI, colores, teclado, readline, progreso, imágenes PNG/JPEG/WebP/BMP/GIF, QR, SVG charts y WAV. |
| IA local | Tokenizers HuggingFace, ONNX por `tract-onnx`, BERT multi-input, embeddings y matemáticas vectoriales. |
| UI y dispositivos | Motor 2D, GUI retenida con rasterizador software, ventanas live, entrada, lifecycle móvil y Termux:API. |
| WebAssembly | Heap WASM, source maps, DOM, eventos, fetch, WebSocket, Canvas 2D, animación y WebGL2 mediante host web. |

Funciones con efectos se protegen mediante capacidades del runtime:

```text
Filesystem · Process · Network · Environment
```

Por ejemplo, `titan run --sandbox programa.titan` conserva funciones puras de texto, JSON, math o colecciones, pero deniega operaciones de archivos, proceso, red y environment. Consulta la [referencia de stdlib](docs/STDLIB.md).

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
titan run --sandbox [ruta]             Ejecutar sin capacidades de efectos
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

## Arquitectura del workspace

| Crate | Responsabilidad |
|---|---|
| `titan_lexer` | Tokenización Unicode, spans y diagnósticos léxicos. |
| `titan_ast` / `titan_parser` | Árbol sintáctico, precedencia y parser. |
| `titan_typechecker` | Scopes, firmas, tipos, structs, enums, traits y control de flujo. |
| `titan_codegen` | Bytecode, source locations y artefactos `.tbc`. |
| `titan_vm` | Ejecución segura, nativas, sandbox, concurrencia, DBs y runtime operativo. |
| `titan_wasm` | Emisión WebAssembly, memoria administrada, host imports y source maps. |
| `titan_stdlib` | Implementaciones Rust y metadata de la API estándar. |
| `titan_pkg` | Manifiestos, imports, lockfiles, registry, archivos y firma de paquetes. |
| `titan_lsp` / `titan_dap` | Servicios para editores y depuradores. |
| `titan_tls` | TLS basado en rustls y WebPKI. |
| `titan_sqlite` / `titan_postgres` / `titan_mysql` | Adaptadores y pools de bases de datos. |
| `titan_gc` / `titan_runtime` | Primitivas de GC y scheduling usadas como capas de runtime. |

Consulta [la arquitectura completa](docs/ARCHITECTURE.md).

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
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

El repositorio contiene más de **400 pruebas Rust declaradas** distribuidas entre lexer, parser, typechecker, VM, WebAssembly, stdlib, paquetes, TLS, LSP, DAP y conectores de base de datos. GitHub Actions incluye comprobaciones del workspace, build sin features predeterminadas, comprobación Android AArch64 y una matriz de test/build para Linux, macOS y Windows.

En dispositivos con almacenamiento limitado, especialmente ARM/Termux, el comando completo puede agotar espacio durante el enlace aunque la compilación del código haya avanzado correctamente. Para reducir el pico de disco:

```bash
cargo clean
CARGO_INCREMENTAL=0 cargo test --workspace --all-targets -j 1
```

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

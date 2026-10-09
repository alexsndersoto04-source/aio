# TITAN / Zett

[![CI](https://github.com/alexsndersoto04-source/aio/actions/workflows/ci.yml/badge.svg)](https://github.com/alexsndersoto04-source/aio/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/alexsndersoto04-source/aio?label=release\&color=brightgreen)](https://github.com/alexsndersoto04-source/aio/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**TITAN** es un lenguaje de programación compilado y con tipos comprobados antes de ejecutar. El compilador, el runtime y la biblioteca estándar están escritos en Titan; el repositorio no contiene código Rust (Titan nació como un prototipo en Rust, que sigue en el historial de git, etiqueta `ultimo-con-rust`). Los programas usan la extensión **`.titan`** y se compilan a ejecutables nativos (o a WebAssembly). **Zett** es el nombre con el que se distribuyó antes, sobre todo en Android/Termux.

> **Estado (octubre de 2026).** Plataformas con ejecutable probado: **Linux x86-64** y **Linux ARM64 (Termux)**. En ARM64 el ejecutable se probó en un servidor ARM64 real (GitHub Actions), no en un teléfono. macOS y Windows todavía no tienen ejecutable. Los binarios de la release [v1.0.0](https://github.com/alexsndersoto04-source/aio/releases/tag/v1.0.0) son de la versión antigua en Rust y no se actualizan. Lo que está verificado y lo que no, con detalle: [`selfhost/ESTADO.md`](selfhost/ESTADO.md).

```text
código fuente (.titan)
   → lexer → parser → comprobación de tipos → bytecode → optimizador
   → generador de código:  x86-64 directo   |   LLVM (x86-64 y ARM64)   |   WebAssembly
   → ejecutable nativo (sin máquina virtual al ejecutar)
```

## Qué incluye

- **Lenguaje:** funciones, closures, structs, enums con datos, `match`, traits, módulos e imports, constantes, alias de tipos, arrays, mapas, rangos, interpolación de strings, `Option` / `Result` y el operador `?`.
- **Ejecutables nativos:** `titan run` y `titan compile` generan código de máquina. El backend propio produce x86-64; el backend LLVM (necesita `clang` y `lld`) produce x86-64 o ARM64 (`--target aarch64`).
- **Concurrencia:** `spawn`, `join`, canales, `select` y timeouts, sobre fibras cooperativas (no hilos del sistema). Ver [`docs/CONCURRENCY.md`](docs/CONCURRENCY.md).
- **WebAssembly:** `titan wasm` genera módulos con memoria lineal, strings, arrays, mapas, structs y enums, más un host JavaScript para DOM, eventos, `fetch`, WebSocket y Canvas.
- **Herramientas:** CLI, REPL, proyectos con `Titan.toml`, paquetes firmados con Ed25519, servidor LSP, servidor DAP y depurador interactivo.
- **Biblioteca estándar:** 812 funciones en 73 espacios de nombres `std::*`, escritas en Titan (ver abajo).

## Instalación

Titan se construye desde las fuentes con `bash` y `gzip`. El repositorio incluye una **semilla**: el compilador ya compilado (7,7 MB, `selfhost/semilla/`) que compila el compilador escrito en Titan. Después el compilador se compila a sí mismo dos veces y se comprueba que el resultado sea idéntico byte a byte (punto fijo). Tarda unos 2–3 minutos.

```bash
git clone https://github.com/alexsndersoto04-source/aio.git
cd aio
bash selfhost/bootstrap.sh      # construye selfhost/titan
selfhost/titan version
```

Para usarlo desde cualquier carpeta, enlaza el ejecutable (necesita su carpeta `native/` al lado; un enlace simbólico funciona):

```bash
ln -s "$PWD/selfhost/titan" ~/.local/bin/titan
```

### ARM64 y Termux

El flujo `Linux ARM64 (Termux)` de GitHub Actions fabrica `titan-v1.0.0-linux-aarch64.tar.gz` y lo prueba en una máquina ARM64 real: 95 programas dan la misma salida que con x86-64. En ARM64, `titan` necesita `clang` y `lld` instalados. Guía y límites conocidos: [`docs/GUIA_TERMUX.md`](docs/GUIA_TERMUX.md); instalador: [`selfhost/instalar-termux.sh`](selfhost/instalar-termux.sh).

### Cruzar de arquitectura

```bash
titan compile programa.titan --target aarch64   # desde x86-64 a ARM64 (usa LLVM)
```

## Primer programa

```titan
fn factorial(n: int) -> int {
    if n <= 1 { return 1 }
    n * factorial(n - 1)
}

fn main() {
    let mut total = 0
    for i in 1..=5 {
        total += factorial(i)
    }
    print("total = {total}")
}
```

```bash
titan run hola.titan
```

Para crear un proyecto: `titan new mi_app && cd mi_app && titan run`.

## Biblioteca estándar

812 funciones en 73 espacios de nombres, todas con cuerpo en Titan (se mide con `bash selfhost/native/cobertura.sh -v`). Las limitaciones de cada pieza están declaradas en [`selfhost/ESTADO.md`](selfhost/ESTADO.md). Referencia: [`docs/STDLIB.md`](docs/STDLIB.md).

| Área | Incluye |
|---|---|
| Texto y formatos | Unicode, regex, encoding, bytes, checksum, JSON, CSV, YAML, XML, URL, UUID, gzip/zstd, TAR/ZIP |
| Criptografía | SHA-2, SHA-3, BLAKE3, HMAC, ChaCha20-Poly1305, AES-GCM, Argon2id, bcrypt, JWT |
| Red | HTTP/HTTPS, TLS 1.2/1.3 con validación X.509, DNS, SMTP, WebSocket, servidor HTTP con router |
| Datos | SQLite (motor SQL propio), PostgreSQL, MySQL, migraciones, pools, Redis |
| Sistema | Archivos, procesos, señales, vigilancia de archivos, variables de entorno, procfs |
| Terminal y multimedia | TUI, imágenes PNG/JPEG/WebP/BMP/GIF, QR, gráficos SVG, audio WAV/FLAC/Vorbis |
| IA local | Tokenizadores HuggingFace, ONNX propio, BERT (modelos pequeños), vectores |
| Interfaz y dispositivos | Motor 2D, GUI con rasterizador por software, ventana en Wayland, Termux:API |
| Web | WebAssembly con host JavaScript: DOM, eventos, `fetch`, Canvas 2D, WebGL2 |

Limitaciones conocidas que conviene saber antes de usarla:

- No hay sistema de permisos ni `--sandbox` en los ejecutables nativos: un programa puede usar archivos, procesos y red.
- Audio: no decodifica MP3, Opus, AAC, AIFF ni CAF; la salida a altavoces va por un reproductor del sistema y no se ha probado en hardware real.
- SQLite: sin FTS, RTREE, ATTACH, EXPLAIN ni tablas temporales; `VACUUM` y `ANALYZE` no hacen nada.
- `std::web` y `std::wasm` solo funcionan con un host JavaScript (fuera de él devuelven error).
- La memoria no se puede limitar por tarea (`spawn_quota` no existe en nativo).

## Comandos de la CLI

```text
titan new <directorio>               Crear un proyecto
titan check [archivo|proyecto]       Comprobar sintaxis y tipos
titan run [archivo|proyecto]         Compilar y ejecutar
titan compile <archivo> [-o salida] [--target x86_64|aarch64]
titan build / exec                   Escribir y ejecutar bytecode .tbc validado
titan wasm [archivo|proyecto]        Generar WebAssembly
titan test [proyecto]                Ejecutar tests/*.titan
titan debug [ruta] -b archivo:línea  Depurador interactivo
titan repl                           REPL
titan add / fetch / update           Dependencias remotas
titan keygen / pack / publish        Paquetes .tpkg firmados con Ed25519
titan lsp / dap                      Servidores para editores
titan version
```

Un proyecto tiene `Titan.toml`, `src/main.titan` y opcionalmente `tests/*.titan`. Los imports se canonicalizan, se detectan ciclos y no pueden salir del árbol de fuentes. Ver [`docs/PROJECTS.md`](docs/PROJECTS.md).

## Estructura del repositorio

```text
selfhost/          el compilador, el runtime y la biblioteca estándar, todo en Titan
  native/            runtime y las 812 funciones std::* (más backends x86-64, LLVM y WebAssembly)
  semilla/           compilador ya compilado para arrancar (bootstrap.sh)
  tests/             pruebas del compilador, del runtime, del depurador y de ARM64
  ESTADO.md          estado verificado, pruebas y limitaciones
docs/              documentación (docs/historico/ guarda informes de la etapa en Rust)
examples/          programas de ejemplo
stdlib/            módulos de la biblioteca estándar que viven en archivos .titan
projects/moon/     Moon, una aplicación completa (API en Titan + interfaz web)
site/              página oficial (generada desde site-src/)
.github/workflows/ CI: pruebas, ARM64 real, comprobaciones de Moon y página
Dockerfile, render.yaml, wrangler.jsonc   despliegue de Moon
```

## Ejemplos y proyectos

La carpeta [`examples/`](examples) tiene más de 50 programas (todos pasan `titan check`): lenguaje, servidor web, bases de datos, criptografía, tokenizador, ONNX, gráficos, TUI, etc. Algunos necesitan recursos externos (internet, un servidor de base de datos, una pantalla, un modelo ONNX).

```bash
titan run examples/hello.titan
titan run examples/webserver.titan
titan run examples/tokenizer.titan
```

[`projects/moon`](projects/moon) es una aplicación completa (API web con base de datos, cuentas, subida de archivos, ≈4 700 líneas de Titan).

## Desarrollo y validación

```bash
bash selfhost/verify_fixpoint.sh       # el compilador se compila a sí mismo: etapas 1, 2 y 3 idénticas
bash selfhost/native/cobertura.sh -v   # funciones std:: con cuerpo en Titan
bash selfhost/tests/arm64_diff.sh      # diferencial x86-64 / ARM64 (en CI de ARM64)
```

Las pruebas de `selfhost/tests/` se escribieron comparando contra la implementación original en Rust; ese oráculo ya no está en el repositorio. Para repetirlas hay que construir la versión con Rust (`git checkout ultimo-con-rust`) y leer [`selfhost/tests/LEEME.md`](selfhost/tests/LEEME.md).

## Documentación

- [Estado verificado y limitaciones](selfhost/ESTADO.md)
- [Arquitectura](docs/ARCHITECTURE.md)
- [Especificación del lenguaje](docs/SPEC.md) y [sintaxis](docs/TITAN_SYNTAX.md)
- [Biblioteca estándar](docs/STDLIB.md) y [extras](docs/EXTRAS.md)
- [Proyectos, módulos y tests](docs/PROJECTS.md)
- [Concurrencia](docs/CONCURRENCY.md)
- [WebAssembly](docs/WASM.md)
- [Red, HTTP, TLS y WebSocket](docs/NETWORKING.md)
- [Bases de datos](docs/DATABASE_API.md)
- [Paquetes](docs/PACKAGE_REGISTRY.md)
- [LSP](docs/LSP.md), [DAP](docs/DAP.md) y [depurador](docs/DEBUGGER.md)
- [Guía de Termux](docs/GUIA_TERMUX.md)
- [Historial de cambios](CHANGELOG.md)

## Licencia

TITAN se distribuye bajo la licencia [MIT](LICENSE).

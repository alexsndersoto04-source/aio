# Ejemplos

Programas de Titan para aprender y para probar piezas de la biblioteca. Se
ejecutan con `titan run <archivo>`; todos pasan `titan check`.

> Algunos comentarios del principio de los archivos vienen de la etapa en Rust
> (mencionan `cargo` o «Fase N»). El código es el actual y se ejecuta con
> `titan`.

## Para empezar

Si es tu primera vez, empieza por la [guía rápida](../docs/GUIA_RAPIDA.md). Sus
programas están en [`guia/`](guia):

| Archivo | Tema |
|---|---|
| `guia/01_variables.titan` | Variables y tipos |
| `guia/02_control.titan` | Funciones, `if`, `for`, `while` |
| `guia/03_colecciones.titan` | Arrays, mapas y closures |
| `guia/04_structs_traits.titan` | `struct`, `impl` y `trait` |
| `guia/05_enums_match.titan` | Enums con datos y `match` |
| `guia/06_errores.titan` | `Result`, `?` y `std::try::catch` |
| `guia/07_tareas.titan` | `spawn`, canales y `join` |
| `guia/08_datos.titan` | JSON, base64 y SHA-256 |

## Lenguaje

| Archivo | Muestra |
|---|---|
| `hello.titan`, `fibonacci.titan` | Lo mínimo |
| `impl_structs.titan`, `traits.titan` | Métodos, métodos estáticos, traits con métodos por defecto |
| `custom_errors.titan` | Enums con datos como errores propios |
| `destructuring.titan`, `for_destructuring.titan` | Destructuring en `let` y en `for` |
| `aliases_spread.titan`, `qol_type_aliases.titan` | Alias de tipos y `..` en arrays |
| `const_and_maps.titan` | `const` con arrays y mapas |
| `pipeline_spaceship.titan` | Operadores `\|>` y `<=>` |
| `qol_higher_order.titan` | `map`, `filter`, `fold` con closures |
| `qol_try_catch.titan` | Manejo de errores con `std::try::catch` y `?` |
| `qol_hetero_arrays.titan`, `qol_if_branches.titan`, `qol_string_add.titan` | Detalles de comodidad |
| `fixes_v032.titan`, `phase34_demo.titan`, `async_demo.titan` | Procesos, colecciones, fechas y `std::async` |
| `modules/` | Un proyecto con varios archivos e `import` |

## Biblioteca estándar

| Archivo | Muestra | Necesita |
|---|---|---|
| `stdlib.titan`, `extras.titan`, `formats.titan` | Texto, compresión, archivos, YAML, XML | — |
| `security.titan` | Criptografía moderna | — |
| `json_api.titan` | JSON y HTTP | internet |
| `network.titan` | HTTPS y DNS | internet |
| `webserver.titan` | Servidor HTTP | un puerto libre |
| `rest/` | API REST con rutas, autenticación y SQLite en memoria | un puerto libre |
| `database.titan`, `enterprise_pool.titan` | Bases de datos y pools de conexiones | — |
| `tokenizer.titan` | Tokenizadores de Hugging Face | `~/tokenizer.json` (el programa imprime de dónde bajarlo) |
| `onnx.titan`, `sentiment.titan`, `search.titan` | Inferencia ONNX (MNIST), análisis de sentimiento, búsqueda semántica | un modelo y su tokenizador en `~/` (cada programa indica cuáles; `sentiment` usa uno de ~65–260 MB, más grande que el límite de 64 MiB de ONNX/BERT descrito en `ESTADO.md`) |
| `vector_search.titan` | Búsqueda por vectores | — |
| `images.titan`, `charts.titan`, `invoice.titan` | Imágenes y QR, gráficos SVG, PDF | — |
| `audio.titan`, `reproductor_*.titan` | Audio WAV/FLAC/Vorbis, reproductor | altavoz (salida por un reproductor del sistema) |
| `tui.titan`, `dashboard.titan`, `system.titan` | Terminal, panel de sistema, procesos y señales | terminal |
| `game_engine.titan`, `gui_*.titan` | Motor 2D, GUI por software | `gui_live_window`: Wayland |
| `android.titan`, `wifi.titan`, `mobile_lifecycle.titan` | Termux:API, Wi-Fi, ciclo de vida móvil | Android con Termux |
| `enterprise_*.titan` | Métricas, perfilador, runtime | — |
| `browser/` | Titan en el navegador (WebAssembly) | `titan wasm` y un servidor web estático |

Las columnas «Necesita» son orientativas: un programa que usa red, pantalla o
dispositivos fallará si no los tiene. Para ver el comportamiento real de cada
pieza, lee [`docs/STDLIB.md`](../docs/STDLIB.md).

# Documentación de Titan

Índice de todo lo que hay en esta carpeta, ordenado por lo que quieres hacer.
La misma documentación está publicada, con menú lateral y buscador, en la
[página oficial](https://alexsndersoto04-source.github.io/aio/documentacion.html).

## Empezar

| Documento | Para qué sirve |
|---|---|
| [`GUIA_RAPIDA.md`](GUIA_RAPIDA.md) | Recorrido por el lenguaje en 20 minutos, con programas que se ejecutan |
| [`PROJECTS.md`](PROJECTS.md) | Proyectos con `Titan.toml`, módulos, imports y tests |
| [`GUIA_TERMUX.md`](GUIA_TERMUX.md) | Instalar y usar Titan en Android (Termux, ARM64) |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Cómo está hecho: etapas del compilador y generadores de código |

## El lenguaje

| Documento | Para qué sirve |
|---|---|
| [`TITAN_SYNTAX.md`](TITAN_SYNTAX.md) | Referencia de sintaxis con ejemplos (todas las construcciones) |
| [`SPEC.md`](SPEC.md) | Especificación formal: gramática y reglas de tipos (en inglés) |
| [`CONCURRENCY.md`](CONCURRENCY.md) | Tareas, canales, `select` y tiempos límite |
| [`WASM.md`](WASM.md) | WebAssembly: qué se genera y qué ofrece el host JavaScript |

## Biblioteca estándar

| Documento | Para qué sirve |
|---|---|
| [`STDLIB.md`](STDLIB.md) | Visión general de los espacios de nombres `std::*` |
| [`EXTRAS.md`](EXTRAS.md) | Módulos adicionales |
| [`NETWORKING.md`](NETWORKING.md), [`HTTP_CLIENT.md`](HTTP_CLIENT.md), [`TLS.md`](TLS.md), [`WEBSOCKET.md`](WEBSOCKET.md), [`MULTIPART.md`](MULTIPART.md) | Red |
| [`SERVER_LIFECYCLE.md`](SERVER_LIFECYCLE.md), [`METRICS.md`](METRICS.md) | Servidor HTTP: ciclo de vida y métricas |
| [`SQLITE.md`](SQLITE.md), [`POSTGRESQL.md`](POSTGRESQL.md), [`MYSQL.md`](MYSQL.md), [`DATABASE_API.md`](DATABASE_API.md) | Bases de datos |
| [`AUDIO.md`](AUDIO.md), [`AUDIO_ENGINE.md`](AUDIO_ENGINE.md) | Audio: decodificación y motor de reproducción |

## Herramientas

| Documento | Para qué sirve |
|---|---|
| [`DEBUGGER.md`](DEBUGGER.md) | Depurador interactivo (`titan debug`) |
| [`DAP.md`](DAP.md), [`LSP.md`](LSP.md) | Servidores para editores (depuración y lenguaje) |
| [`PACKAGE_REGISTRY.md`](PACKAGE_REGISTRY.md) | Paquetes `.tpkg` firmados y registro |

## Estado del proyecto

| Documento | Para qué sirve |
|---|---|
| [`../selfhost/ESTADO.md`](../selfhost/ESTADO.md) | **Fuente de la verdad**: qué está verificado, cómo y qué límites tiene |
| [`VALIDATION.md`](VALIDATION.md) | Registro de validaciones de la etapa anterior |
| [`../CHANGELOG.md`](../CHANGELOG.md) | Historial de cambios |
| [`historico/`](historico) | Informes de la etapa en Rust, conservados como registro |

> **Si dos documentos se contradicen**, manda `selfhost/ESTADO.md`: es el que
> se actualiza con cada verificación. Los documentos de dependencias externas
> (por ejemplo, el de SQLite) describen lo que Titan implementa, no el producto
> original.

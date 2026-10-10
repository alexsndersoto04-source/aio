# Guía de Contribución a Titan

Gracias por tu interés en contribuir a **Titan / Zett**. Este documento describe cómo configurar el entorno de desarrollo, la arquitectura general del repositorio, las pruebas de verificación y el flujo recomendado para enviar mejoras o correcciones.

---

## 1. Configuración del entorno de desarrollo

Titan es un lenguaje autoalojado (*self-hosted*): el compilador, el runtime y las 812 funciones de la biblioteca estándar están escritos íntegramente en Titan.

### Requisitos básicos (Linux x86-64)
- `bash`, `gzip` y utilidades estándar de sistema (`coreutils`, `tar`, `sha256sum`).
- **Opcional (Backend LLVM y compilación cruzada ARM64):** `clang` y `lld`.
- **Opcional (Generador del sitio web y documentación):** `python3` y el paquete `markdown` (`pip install markdown`).

### Construcción desde las fuentes (*Bootstrap*)

```bash
git clone https://github.com/alexsndersoto04-source/aio.git
cd aio
bash selfhost/bootstrap.sh
selfhost/titan version
```

El script `selfhost/bootstrap.sh` descomprime la semilla inicial (`selfhost/semilla/titanc-linux-x86_64.gz`), compila el compilador en tres etapas sucesivas (`titanc1`, `titanc2`, `titanc3`) comprobando que produzcan el mismo binario byte a byte (punto fijo), y finalmente genera la herramienta completa `selfhost/titan`.

### Instalación desde paquetes precompilados (Linux x86-64 y ARM64)
Si prefieres instalar directamente los binarios oficiales precompilados para **Linux x86-64** o **Linux ARM64 (`aarch64` / Termux)**, puedes descargarlos desde [GitHub Releases](https://github.com/alexsndersoto04-source/aio/releases/latest):

- **Linux x86-64:** `titan-linux-x86_64.tar.gz`
- **Linux ARM64 / Termux:** `titan-linux-aarch64.tar.gz`

---

## 2. Estructura del repositorio

| Directorio | Descripción |
|---|---|
| `selfhost/` | Código fuente del compilador (`lexer`, `parser`, `typechecker`, `codegen`, `opt`, `wasm`), la CLI unificada (`titan.titan`) y herramientas (LSP, DAP, Zett). |
| `selfhost/native/` | Runtime nativo, generadores de código (`x64.titan`, `llvm.titan`, `elf.titan`) y la biblioteca estándar (`std_*.titan`). |
| `selfhost/tests/` | Suites de pruebas automatizadas del compilador, biblioteca estándar, depurador y validación diferencial ARM64. |
| `stdlib/` | Módulos adicionales de la biblioteca estándar en archivos `.titan`. |
| `examples/` | Más de 50 programas de ejemplo que muestran el uso del lenguaje y de su biblioteca estándar. |
| `docs/` | Documentación técnica del lenguaje, la biblioteca estándar, la arquitectura y las herramientas. |
| `site-src/` | Plantillas, hojas de estilo y generador (`generar.py`) del sitio web oficial publicado en `site/`. |

Para más detalles sobre las etapas internas del compilador, consulta [`selfhost/README.md`](selfhost/README.md) y [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

---

## 3. Verificación y pruebas de calidad

Antes de enviar un *Pull Request*, ejecuta las comprobaciones correspondientes al área que hayas modificado:

| Área modificada | Comando de verificación |
|---|---|
| **Compilador (`selfhost/*.titan`)** | `bash selfhost/verify_fixpoint.sh` (verifica la autocompilación en 3 etapas) |
| **Genéricos y sistema de tipos** | `bash selfhost/tests/genericos/probar.sh ./selfhost/titan` y `bash selfhost/tests/errores/probar.sh ./selfhost/titan` |
| **Biblioteca estándar (`selfhost/native/`)** | `bash selfhost/native/cobertura.sh -v` y ejecución de un programa de prueba relacionado |
| **Ejemplos o proyectos (`examples/`, `projects/`)** | `./selfhost/titan check <archivo.titan>` |
| **Sitio web y documentación (`site-src/`, `docs/`)** | `python3 site-src/generar.py claro` |

Todas las pruebas se ejecutan automáticamente mediante **GitHub Actions** en cada *Pull Request*, tanto en entornos `Linux x86-64` como en servidores `Linux ARM64`.

---

## 4. Buenas prácticas de desarrollo

- **Documentación y diagnósticos:** Los mensajes de usuario de la CLI, los diagnósticos del compilador y la documentación principal se mantienen en español claro y técnico.
- **Estado técnico:** Si agregas una funcionalidad nueva al lenguaje o a la biblioteca estándar, actualiza la documentación correspondiente en `docs/` y el registro técnico en [`selfhost/ESTADO.md`](selfhost/ESTADO.md).
- **Sitio web oficial:** Los archivos dentro de `site/` se generan automáticamente a partir de `site-src/` y `docs/`. Realiza siempre los cambios en `site-src/` o `docs/` y ejecuta `python3 site-src/generar.py claro` para actualizar `site/`.
- **Control de versiones:** Evita incluir binarios compilados o artefactos temporales en los commits; el repositorio mantiene únicamente el código fuente y la semilla verificada en `selfhost/semilla/`.

---

## 5. Cómo enviar un Pull Request

1. Crea una rama descriptiva a partir de `main` (por ejemplo, `feat/nueva-funcion`, `fix/correccion-parser` o `docs/mejora-guia`).
2. Realiza commits claros y enfocados que expliquen el propósito del cambio.
3. Verifica que las pruebas locales pasen sin errores.
4. Abre un **Pull Request** detallando:
   - Qué problema resuelve o qué funcionalidad incorpora.
   - Qué pruebas ejecutaste para validarlo.

---

## 6. Reporte de errores y sugerencias (Issues)

Si encuentras un error o deseas proponer una mejora, abre un [Issue en GitHub](https://github.com/alexsndersoto04-source/aio/issues) incluyendo, en la medida de lo posible:

- La salida de `titan version` y la arquitectura del sistema (`uname -a`).
- Un fragmento mínimo de código `.titan` que permita reproducir el comportamiento.
- El resultado esperado y el resultado obtenido.

---

## 7. Licencia

Al contribuir a este repositorio, aceptas que tus aportaciones se distribuyan bajo los términos de la licencia [MIT](LICENSE) del proyecto.

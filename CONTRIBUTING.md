# Guía de Contribución a Titan

Gracias por tu interés en contribuir a **Titan / Zett**. Damos la bienvenida a colaboraciones de la comunidad en el compilador, el runtime, la biblioteca estándar, las herramientas de desarrollo (LSP, DAP, Zett) y la documentación.

---

## 1. Configuración del entorno de desarrollo

Titan es un lenguaje compilado y autoalojado (*self-hosted*): el compilador, el runtime y la biblioteca estándar están implementados en Titan.

### Requisitos del sistema (Linux x86-64)
- `bash`, `gzip` y utilidades estándar de sistema (`coreutils`, `tar`, `sha256sum`).
- **Opcional (Backend LLVM y compilación cruzada ARM64):** `clang` y `lld`.
- **Opcional (Generación del sitio web estático):** `python3` y el paquete `markdown` (`pip install markdown`).

### Construcción desde el código fuente (*Bootstrap*)

```bash
git clone https://github.com/alexsndersoto04-source/aio.git
cd aio
bash selfhost/bootstrap.sh
selfhost/titan version
```

El script `selfhost/bootstrap.sh` construye el compilador en tres etapas sucesivas (`titanc1`, `titanc2`, `titanc3`), verifica la reproducibilidad determinista del binario (punto fijo) y genera el ejecutable unificado `selfhost/titan`.

### Instalación mediante paquetes precompilados
También puedes descargar los binarios oficiales precompilados para **Linux x86-64** y **Linux ARM64 (`aarch64` / Termux)** desde [GitHub Releases](https://github.com/alexsndersoto04-source/aio/releases/latest):

- **Linux x86-64:** `titan-linux-x86_64.tar.gz`
- **Linux ARM64 / Termux:** `titan-linux-aarch64.tar.gz`

---

## 2. Arquitectura del repositorio

| Directorio | Descripción |
|---|---|
| `selfhost/` | Compilador principal (`lexer`, `parser`, `typechecker`, `codegen`, `opt`, `wasm`), CLI (`titan.titan`) y servidores LSP/DAP. |
| `selfhost/native/` | Runtime nativo, generadores de código (`x64.titan`, `llvm.titan`, `elf.titan`) y los 73 módulos de la biblioteca estándar (`std_*.titan`). |
| `selfhost/tests/` | Suites de pruebas automatizadas del compilador, biblioteca estándar, depurador y validación multiplataforma. |
| `stdlib/` | Módulos adicionales de la biblioteca estándar en archivos `.titan`. |
| `examples/` | Programas de ejemplo que demuestran las capacidades del lenguaje y su biblioteca estándar. |
| `docs/` | Documentación técnica del lenguaje, especificación formal, arquitectura y guías de uso. |
| `site-src/` | Plantillas, estilos y generador (`generar.py`) del sitio web oficial publicado en `site/`. |

Para profundizar en el diseño interno del compilador, consulta [`selfhost/README.md`](selfhost/README.md) y [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

---

## 3. Estándares de ingeniería y código

Para mantener la estabilidad, el rendimiento y la mantenibilidad del proyecto, recomendamos seguir estos principios al desarrollar cambios:

1. **Diseño modular y claridad:** Escribe código legible, bien estructurado y fácil de mantener, respetando las convenciones de estilo existentes en cada módulo.
2. **Reproducibilidad del compilador:** Cualquier cambio en las etapas del compilador (`lexer`, `parser`, `typechecker`, `codegen`, `opt`) debe mantener la propiedad de punto fijo determinista verificada por `verify_fixpoint.sh`.
3. **Cobertura de pruebas:** Acompaña las nuevas características o correcciones de errores con casos de prueba automatizados dentro de `selfhost/tests/` o `examples/`.
4. **Compatibilidad multiplataforma:** Asegúrate de que los cambios en el runtime o en la generación de código sean compatibles tanto con `x86-64` como con `ARM64` (`aarch64`) y `WebAssembly` cuando corresponda.
5. **Documentación sincronizada:** Cuando incorpores nuevas funciones, comandos o sintaxis, actualiza la documentación técnica correspondiente en `docs/` y regenera el sitio estático con `python3 site-src/generar.py claro`.

---

## 4. Verificación y suite de pruebas

Antes de enviar un *Pull Request*, ejecuta las pruebas correspondientes a los componentes modificados:

| Componente modificado | Comando de verificación |
|---|---|
| **Compilador (`selfhost/*.titan`)** | `bash selfhost/verify_fixpoint.sh` |
| **Sistema de tipos y genéricos** | `bash selfhost/tests/genericos/probar.sh ./selfhost/titan` y `bash selfhost/tests/errores/probar.sh ./selfhost/titan` |
| **Biblioteca estándar (`selfhost/native/`)** | `bash selfhost/native/cobertura.sh -v` |
| **Ejemplos o proyectos (`examples/`, `projects/`)** | `./selfhost/titan check <archivo.titan>` |
| **Sitio web y documentación (`site-src/`, `docs/`)** | `python3 site-src/generar.py claro` |

El sistema de integración continua (**GitHub Actions**) ejecuta automáticamente estas validaciones en cada *Pull Request*.

---

## 5. Proceso para enviar un Pull Request

1. Crea una rama descriptiva a partir de `main` (por ejemplo, `feat/nueva-funcionalidad`, `fix/correccion-tipos` o `docs/actualizacion-guia`).
2. Realiza commits atómicos y descriptivos que expliquen claramente el propósito técnico del cambio.
3. Comprueba que la suite de pruebas local finalice correctamente.
4. Abre un **Pull Request** incluyendo:
   - Una descripción clara de la mejora o del problema resuelto.
   - Los pasos o comandos utilizados para verificar el cambio.

---

## 6. Reporte de errores y propuestas de mejora (Issues)

Para reportar un defecto o proponer una nueva característica, abre un [Issue en GitHub](https://github.com/alexsndersoto04-source/aio/issues) indicando:

- Versión de la herramienta (`titan version`) y plataforma (`uname -a`).
- Un ejemplo mínimo reproducible en código `.titan`.
- El comportamiento esperado y el resultado observado.

---

## 7. Código de conducta y Licencia

Fomentamos un entorno de colaboración respetuoso, constructivo y orientado a la excelencia técnica. Al contribuir a este repositorio, aceptas que tus aportaciones se distribuyan bajo los términos de la licencia [MIT](LICENSE).

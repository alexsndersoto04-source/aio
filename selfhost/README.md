# selfhost/ — el compilador, el runtime y la biblioteca estándar

Todo Titan vive aquí y está escrito en Titan. Esta carpeta explica **qué es cada
cosa** y **cómo se construye y se comprueba**. Para saber qué está verificado y
qué límites tiene cada pieza, lee [`ESTADO.md`](ESTADO.md) (es la fuente de la
verdad; este archivo es solo un mapa).

## Construir

Necesitas Linux x86-64, `bash` y `gzip`:

```bash
bash selfhost/bootstrap.sh     # unos 2–3 minutos
selfhost/titan version
```

`bootstrap.sh` hace tres cosas:

1. Descomprime la **semilla** (`semilla/titanc-linux-x86_64.gz`, el compilador ya
   compilado) y comprueba su SHA-256.
2. Con la semilla compila el compilador (`titanc1`), con ese compila otro
   (`titanc2`) y con ese otro más (`titanc3`). Comprueba que los tres sean
   idénticos byte a byte (**punto fijo**): `verify_fixpoint.sh`.
3. Con `titanc3` compila la herramienta `titan` (`titan.titan`).

La semilla es un ejecutable x86-64: en ARM64 no se puede ejecutar el
`bootstrap.sh`; se usa el paquete ya compilado o se compila `titan.titan` desde
x86-64 con `--target aarch64` (ver [`../docs/GUIA_TERMUX.md`](../docs/GUIA_TERMUX.md)).

## Mapa de archivos

### Las etapas del compilador

| Archivo | Qué hace |
|---|---|
| `lexer.titan` | Convierte el texto en palabras (tokens) |
| `parser.titan` | Construye el árbol sintáctico |
| `typechecker.titan` | Comprueba tipos y reglas antes de generar nada |
| `loader.titan` | Carga el proyecto y resuelve los `import` |
| `frontend.titan` | Parte común de los dos compiladores nativos: cargar, revisar y generar el bytecode |
| `codegen.titan`, `codegen_tables.titan`, `bytecode.titan` | Genera el bytecode (representación intermedia) |
| `opt.titan`, `opt_ver.titan` | Optimizador: constantes, saltos, código muerto, última lectura |
| `build.titan`, `native_build.titan`, `native/x64.titan`, `native/elf.titan` | Generador propio de x86-64 y escritura del ELF |
| `build_llvm.titan`, `llvm_build.titan`, `native/llvm.titan` | Generador por LLVM (x86-64 y ARM64; necesita `clang` y `lld`) |
| `wasm.titan`, `wasm_cmd.titan`, `wasm_cli.titan` | Generador de WebAssembly |
| `artifact.titan` | Formato del bytecode `.tbc` validado (`titan build` / `exec`) |

### La herramienta `titan`

`titan.titan` es el programa principal; el resto son sus piezas:
`args.titan` y `cli_util.titan` (argumentos), `pkg.titan` y `semver.titan`
(dependencias y paquetes; es lo que usa **Zett**, el gestor de paquetes: ver [`../docs/ZETT.md`](../docs/ZETT.md)), `lsp.titan` (servidor de lenguaje), `dap.titan` y
`debug_table.titan` (depuración), `rtcache.titan` (caché de la parte del runtime que no cambia entre programas).

Herramientas auxiliares para verificar las etapas: `tokens.titan`, `ast.titan` y `check.titan` imprimen los tokens, el árbol y los diagnósticos de un archivo (las usan los `verify_*.sh`); `revisar_runtime.titan` revisa el runtime sin generar código; `prueba_interna.titan` es un compilador de pruebas que expone funciones internas y no se usa para programas normales.

### El runtime y la biblioteca estándar: `native/`

Contiene el runtime (`runtime.titan`, `std_core.titan`, `std_task.titan`…) y los
archivos `std_*.titan`, uno o varios por espacio de nombres (`std_json`,
`std_http`, `std_sqlite_*`, `std_tls`, `std_audio_*`…). Los subdirectorios y
los `gen_*.py`/`gen_*.titan` generan tablas (Unicode, errno, constantes de
hash, floats). El compilador lee esta carpeta cada vez que compila un programa,
por eso el ejecutable `titan` debe tener `native/` a su lado.

```bash
bash selfhost/native/cobertura.sh -v    # cuántas de las 812 funciones std:: tienen cuerpo en Titan
```

### Otras carpetas

| Carpeta | Contenido |
|---|---|
| `semilla/` | El compilador ya compilado que arranca todo (ver `semilla/LEEME.md`) |
| `tests/` | Pruebas por etapa y pieza (ver `tests/LEEME.md`) |
| `fuentes/` | Copias de las bibliotecas de Rust de las que se portó comportamiento, con su SHA-256 (`VERIFICADO.txt`) |
| `native/llvm/` | Scripts para verificar el backend LLVM (incluido ARM64 real) |

### Scripts

| Script | Para qué |
|---|---|
| `bootstrap.sh` | Construye `selfhost/titan` desde la semilla |
| `verify_fixpoint.sh` | Comprueba que el compilador se compila a sí mismo (etapas idénticas) |
| `verify_lexer.sh`, `verify_parser.sh`, `verify_typechecker.sh`, `verify_codegen.sh` | Verificaciones por etapa |
| `empaquetar.sh` | Arma `titan-<versión>-linux-<arquitectura>.tar.gz` (con `titan`, el enlace `zett → titan` y `native/`) |
| `instalar-termux.sh` | Instalador para Android (Termux) |
| `tests/arm64_diff.sh` | Compara x86-64 con ARM64 programa a programa (en la CI de ARM64) |

## Verificar

```bash
bash selfhost/verify_fixpoint.sh                      # punto fijo
bash selfhost/native/cobertura.sh -v                  # biblioteca estándar
bash selfhost/tests/debug/probar.sh selfhost/titan <titan de referencia>   # depurador (x86-64)
```

Muchas pruebas de `tests/` comparaban contra la implementación original en Rust,
que ya no está en el repositorio; `tests/LEEME.md` explica cuáles se pueden
repetir tal cual y cómo reconstruir el resto.

## Si algo falla

- *«la semilla no coincide con su SHA-256»*: el archivo `semilla/*.gz` está
  dañado; vuelve a clonar el repositorio.
- *«la semilla es un ejecutable Linux x86-64»*: estás en otra plataforma.
- La primera compilación con LLVM tarda más (≈1 minuto 20 s) y necesita `clang` y `lld`.
- Los errores del compilador apuntan a la sentencia que contiene el problema,
  no siempre a la línea exacta.

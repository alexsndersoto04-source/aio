# Pruebas de `selfhost/tests/`

Casi todas las pruebas de esta carpeta son **diferenciales**: ejecutan el mismo programa con la VM de Rust
(`zett run`, el oráculo) y con el compilador nativo en Titan y exigen salida idéntica. Desde que se borró el Rust
(ver `selfhost/ESTADO.md`, «Rust borrado») el oráculo ya no está en el repositorio; los scripts `verify_*.sh`,
`native/verify_native.sh` y los `run*.sh`/`*.py` que llaman a `zett` siguen aquí como **registro de cómo se
verificó** cada pieza, y se pueden volver a ejecutar así:

```bash
git worktree add /tmp/titan-rust ultimo-con-rust     # commit 5dbb238: la última versión con Rust
cd /tmp/titan-rust && cargo build --release -p titan_cli && cp target/release/titan ~/bin/zett
cd <este repo> && ZETT=~/bin/zett COMPILER=selfhost/titanc3 bash selfhost/native/verify_native.sh
```

Lo que **no** necesita Rust y sigue funcionando tal cual:
- `bash selfhost/native/verify_native.sh` (sin argumentos: todo `tests/native`): compara el código nativo
  con la VM de bytecode del propio port (`titan build` + `titan exec`). Con un `zett` de Rust en el PATH,
  o con `VM=zett`, vuelve a comparar contra la VM de Rust (la comparación histórica). Hoy pasa el corpus
  completo que no necesita servicios externos: 221 programas, 221 idénticos.

Los que **sí** siguen necesitando el `zett` de Rust (porque su oráculo son los volcados de Rust o porque
lo usan para construir la herramienta): `verify_lexer.sh`, `verify_parser.sh`, `verify_typechecker.sh`,
`verify_codegen.sh`, `tests/tbc_diff`, `tests/server_diff`, `tests/repl`, `tests/debug`,
`tests/lsp/contraste_zett.py`, `tests/mysql/comparar.sh`, `tests/onnx/*`, `tests/audio/verificar.sh`,
`tests/native/tokenize_dev/*`, `tests/pkg_diff` y `native/net/comparar.sh`.
(`native/profile.py` solo lo nombra en un comentario: es una herramienta de desarrollo.)
- `bash selfhost/native/verify_json.sh` (97 casos de JSON, errores con línea y columna incluidos) y
  `bash selfhost/tests/vm_diff.sh` (muestra del corpus; `TITAN_VM_MUESTRA=1` para la amplia): el mismo
  tipo de comparación, con la ventaja de que el lado VM es el artefacto `.tbc` de verdad.
- `bash selfhost/verify_fixpoint.sh` (punto fijo, con la semilla) y `bash selfhost/native/cobertura.sh -v`
  (la cobertura revisa el runtime con la CLI nativa `selfhost/titan`; ya no pide el `zett` de Rust).
- Los arneses que solo usan `selfhost/titan`: `errores/probar.sh` (posiciones y mensajes de los
  diagnósticos), `arreglos/probar.sh` (tipos de bucles y rechazos), `genericos/probar.sh`
  (genéricos `<T>` y sus rechazos), `test_runner/probar.sh` (`titan test`: módulos auxiliares
  omitidos, imports desde `src/` y `tests/`, fallo reportado) y `pkg/registro/estatico.sh`
  (registro estático de paquetes por HTTPS: publicar, resolver, verificar la firma y rechazar un
  archivo alterado; necesita `openssl` y `python3`).
- `onnx/run.sh` y `onnx/run_bert.sh` (comparan con onnxruntime y con el tract de la VM; sin la VM comparan solo
  con onnxruntime si se desactiva la parte `ZETT`).
- Los tests de `tests/native/*.titan` como programas: `selfhost/titan run <prueba>`.
- Los fuzzers de `native/` (regex, url, router, QR, archive, compress y yaml): comparan el nativo contra la
  VM de bytecode del port con el ayudante `native/vm_port.py`. Antes comparaban contra la VM de Rust;
  detalles, comandos y resultados en `selfhost/ESTADO.md`, «Los fuzzers diferenciales sin el Rust».
  `fuzz_enc.py` necesita el `core.rs` de miniz_oxide 0.8.9 (`CORE_RS=`) y `fuzz_zstd.py` el módulo
  `zstandard` de Python. `regex/umbral.py` se corre aparte (tarda ~20 minutos) y su lado nativo es
  `nat_hir`, no `umbral.titan`.
- `bash native/verify_x64.sh` (codificador x86-64 contra `objdump`), `bash native/image/verificar.sh`
  (12 pruebas de imagen) y `bash native/bare/probar.sh [programas...]` (ARM64 sin sistema operativo, con
  zig + unicorn): los tres comparan contra el propio port o contra herramientas externas, sin Rust.
- Los arneses de LLVM: `bash native/llvm/punto_fijo.sh` (mismo compilador y mismo IR byte a byte),
  `bash native/llvm/verificar.sh [archivos]` (ejecutables por LLVM frente al backend nativo del port),
  `bash native/llvm/arm64_objetos.sh SALIDA [archivos]` (objetos AArch64 reales en una máquina x86-64) y
  `bash native/llvm/arm64_referencias.sh OBJETOS [archivos]` (salidas de referencia x86-64). `build_llvm` lo
  rehace `selfhost/titanc3`; `arm64_comparar.sh` sigue pidiendo una máquina ARM64 real.
- `bash native/term/verificar.sh` (pseudoterminal; 11 de 11 con `titan run --tty`, que deja al programa la
  terminal) y `python3 native/readline/verificar.py` (la misma idea con `titan exec --tty`: edición, historial,
  búsqueda inversa, clave secreta, Ctrl-C/Ctrl-D, tubería y rechazo sin terminal, comparados con la VM del
  port). `python3 native/http_full/verificar.py` prueba HTTP contra esa VM y HTTPS con una CA de prueba
  generada con `openssl`, en un cliente compilado con `std_roots.titan` regenerado, como en
  `tests/pkg/registro/estatico.sh`.
- `./selfhost/titanc3 native/libm/h.titan /tmp/h` y `python3 native/libm/verify_libm.py SEMILLA CASOS /tmp/h`
  (bit a bit contra `libm.so.6` del sistema).
- `bash native/uwidth/verificar.sh` (ancho de texto; arma el compilador de pruebas con `titan compile`).
- `bash tests/vm_diff.sh` y `bash native/verify_json.sh`: el corpus y el analizador de JSON comparados
  entre las dos formas de ejecutar del port (`titan run` y `titan build` + `titan exec`).
- `bash tests/wasm_diff/run.sh`: funciona sin `zett`; si lo encuentra, compara además el `.wasm` byte a
  byte con `zett wasm`.

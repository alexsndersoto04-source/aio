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
`tests/lsp/contraste_zett.py`, `tests/mysql/comparar.sh`, `tests/audio/verificar.sh`,
`tests/native/tokenize_dev/*` y `tests/pkg_diff`.
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
- `onnx/run.sh`: el oráculo principal es **onnxruntime**, que no es Rust, así que corre sin `zett` (compara
  el motor propio con onnxruntime y avisa de que no hay tract). Con `zett` en `ZETT=` o en el PATH compara
  además con tract (la implementación de Rust de siempre) y exige que los mensajes de `errors.titan` sean
  idénticos a los suyos. `run_bert.sh` (BERT) sí sigue pidiendo el `zett` de Rust para su parte de tract,
  además de `torch`/`transformers`/`onnxscript`.
  En un entorno sin `libasound.so.2` (el `zett` la pide al cargar) se puede arrancar igual con una biblioteca
  de prueba que no hace nada de sonido: `readelf --dyn-syms -W zett | awk '$7=="UND"' | grep -o 'snd_[a-z0-9_]*@[A-Za-z0-9_.]*' | sort -u`
  da los 48 símbolos con sus versiones (`ALSA_0.9`, `ALSA_0.9.0rc4`, `ALSA_0.9.0rc8`); se compila una
  `libasound.so.2` con esas firmas devolviendo -1 y una `--version-script` con esos nodos, y se corre con
  `LD_LIBRARY_PATH=`. No cambia ningún resultado: aquí el sonido no se usa. Con eso,
  `LD_LIBRARY_PATH=/tmp/al ZETT=/ruta/a/zett bash tests/onnx/run.sh` sobre 35 modelos da `ONNX: TODO OK`
  (70 comprobaciones idénticas a onnxruntime, 3 de ellas sin tract porque no las ejecuta).
- `bash native/net/comparar.sh EJECUTABLE` (`std::net::http_get` frente a glibc de verdad): la referencia es
  `native/net/oraculo_glibc.py` (getaddrinfo por ctypes y sockets reales), no la VM de Rust, porque `std::net`
  de Rust llamaba a esas mismas funciones de glibc. Necesita `sudo`, `native/net/dns_falso.py` en 127.0.0.53 y
  `native/net/servidores.py`; restaura los archivos de `/etc` al terminar. Con `ZETT=/ruta/a/zett` compara
  contra la VM de Rust (la comparación histórica). El ejecutable de prueba es
  `selfhost/titan compile native/net/prueba.titan -o /tmp/prueba_net`.
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

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
- `bash selfhost/verify_fixpoint.sh` (punto fijo, con la semilla) y `bash selfhost/native/cobertura.sh -v`.
- `onnx/run.sh` y `onnx/run_bert.sh` (comparan con onnxruntime y con el tract de la VM; sin la VM comparan solo
  con onnxruntime si se desactiva la parte `ZETT`).
- Los tests de `tests/native/*.titan` como programas: `selfhost/titan run <prueba>`.

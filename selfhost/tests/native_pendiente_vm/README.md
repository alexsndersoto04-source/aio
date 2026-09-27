# Pruebas que esperan una VM de Rust corregida

Estos programas prueban el **bug 12** (ver `selfhost/ESTADO.md`): la VM de Rust
hacía *panic* (se caía con un mensaje interno) en vez de dar un error normal.

La versión en Titan ya da el error correcto, y la corrección en Rust está en
`crates/titan_stdlib/src/datetime_mod.rs`. Pero el binario `zett` que se usa
para comparar es el precompilado, **anterior a la corrección**. Por eso estos
programas darían «distintos» en `verify_native.sh` hasta reconstruir la VM.

Cuando haya un `zett` nuevo:

    bash selfhost/native/verify_native.sh selfhost/tests/native_pendiente_vm/*.titan

y, si coinciden, se mueven a `selfhost/tests/native/`.

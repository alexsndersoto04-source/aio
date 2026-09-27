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

## Bug 13 (`std::dirs`)

`dirs_lineas_malas.titan` necesita la variable `XDG_CONFIG_HOME` (el archivo
`user-dirs.dirs` se lee al pedir la carpeta, pero Titan no puede cambiar el
entorno del propio proceso):

    XDG_CONFIG_HOME=/tmp/titan_bug13 bash selfhost/native/verify_native.sh selfhost/tests/native_pendiente_vm/dirs_lineas_malas.titan

Con el `zett` precompilado la VM se cae (panic en dirs-sys); la versión en
Titan y la VM corregida escriben `/escritorio` y una línea vacía para música.

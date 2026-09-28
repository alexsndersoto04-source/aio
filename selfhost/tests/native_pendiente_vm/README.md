# Pruebas que esperan una VM de Rust reconstruida

Cuando se corrige un defecto en el código Rust, el `zett` que se usa para
comparar solo lo tiene después de que el workflow *Publish Linux x86_64
binary* lo recompile (unos minutos tras el push) y `scripts/sandbox-zett.sh`
lo descargue de la rama `binaries`. Mientras tanto, las pruebas de ese defecto
esperan aquí:

    bash scripts/sandbox-zett.sh
    bash selfhost/native/verify_native.sh selfhost/tests/native_pendiente_vm/*.titan

y, si coinciden, se mueven a `selfhost/tests/native/`.

Historial: las pruebas de los bugs 12, 13 y 14 pasaron aquí y se movieron el
2026-09-28, con el `zett` compilado del commit 9c3b3cb (5/5 idénticos).
`dirs_lineas_malas.titan` (bug 13) necesita `XDG_CONFIG_HOME=/tmp/titan_bug13`
para probar el caso del defecto; sin esa variable prueba el caso normal.

## Bugs 11 y 16 (palabras clave después de `::`)

`ruta_palabra_clave.titan`: `std::uuid::nil()` (y `std::process::spawn`) no se
podían escribir porque `nil` y `spawn` son palabras clave.

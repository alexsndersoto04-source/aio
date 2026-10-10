# Titan en Termux (Android, ARM64)

Titan funciona en Linux ARM64. Termux usa el mismo núcleo Linux, así que el ejecutable ARM64 de Titan está pensado para correr ahí.

> **Estado honesto.** El ejecutable ARM64 se probó en un servidor Linux ARM64 (GitHub Actions, `ubuntu-24.04-arm`): arranca,
> compila y ejecuta programas, y 95 programas de prueba dieron exactamente la misma salida que con el compilador x86-64
> (ver «Linux x86-64 y ARM64/Termux» en [`selfhost/ESTADO.md`](../selfhost/ESTADO.md)). En un teléfono Android (Redmi 9C,
> Termux de 32 bits) el binario aarch64 arranca (`titan version` sí corre). El 2026-10-10 se corrigió la detección de
> arquitectura para esos Termux de 32 bits (antes el compilador creía estar en una PC y pedía un binario que no era).

## Antes de instalar: ¿tu Termux corre el binario de 64 bits?

Titan solo tiene versión para **ARM de 64 bits** (`aarch64`); no existe versión para ARM de 32 bits. Pero cuidado:
**un Termux de 32 bits puede estar en un teléfono con núcleo de 64 bits**, y ahí el binario aarch64 corre igual.
La prueba de verdad es intentarlo (el instalador lo hace al final):

```sh
uname -m                              # a | aarch64 → estupendo; armv8l / armv7l → mira abajo
getprop ro.product.cpu.abilist        # dato orientativo, no definitivo
```

- Si `uname -m` dice `aarch64`, sigue con el paso 1.
- Si dice `armv7l` o `armv8l` (Termux de 32 bits): sigue igual. Si al final `titan version` imprime la versión,
  tu teléfono ejecuta los binarios de 64 bits y **todo funciona** (caso del Redmi 9C, aunque `abilist` solo
  traiga `armeabi-v7a` y `armeabi`). Si ves «Exec format error», tu teléfono es de 32 bits de verdad: instala
  el Termux de 64 bits (APK `arm64-v8a`, de F-Droid o del GitHub oficial de Termux) **si tu teléfono lo admite**;
  si no lo admite, no hay versión posible (hace falta un generador de código para ARM de 32 bits, que no existe).
- Alternativa recomendada si puedes: instalar el Termux de 64 bits desde el principio. Desinstalar el Termux
  actual borra sus datos: guarda antes lo que quieras conservar.

## 1. Instalar

Necesitas el paquete `titan-v1.2.0-linux-aarch64.tar.gz`. Lo fabrica el flujo `Linux ARM64 (Termux)` de GitHub Actions
(artefacto `paquetes` de la ejecución; también se publica como release cuando se crea una etiqueta `v*`).

Con la release publicada:

```sh
pkg install -y curl
curl -fLO https://raw.githubusercontent.com/alexsndersoto04-source/aio/main/selfhost/instalar-termux.sh
bash instalar-termux.sh
```

Con un paquete que ya descargaste (por ejemplo el artefacto de Actions):

```sh
bash selfhost/instalar-termux.sh /ruta/a/titan-v1.2.0-linux-aarch64.tar.gz
```

Desde la versión 1.2.0 el paquete trae también `zett`, el gestor de paquetes (ver [`ZETT.md`](ZETT.md)); el script lo deja en el `PATH` junto a `titan`.

El script instala `clang` (Titan en ARM64 genera LLVM IR y llama a `clang`/`lld` para producir el ejecutable) y deja `titan`
en `$PREFIX/bin`. Comprueba:

```sh
titan version
printf 'fn main() {\n    println("hola desde Termux")\n}\n' > hola.titan
titan run hola.titan
```

La primera compilación tarda más: genera una caché del runtime dentro de `native/` (junto al ejecutable).

## 2. Qué esperar

- `titan run` y `titan compile` están verificados en ARM64 Linux. Los demás subcomandos (`check`, `test`, `new`, `build`, `exec`, `repl`…) usan
  el mismo código, pero no se probaron uno por uno en ARM64.
- Tareas (`spawn`), canales, servidor HTTP, procesos hijos, SQLite, TLS, regex, compresión: pasan las pruebas en ARM64 Linux.
- `std::termux` (Termux:API), portapapeles, notificaciones, Wi-Fi: dependen de tener instalada la app y el paquete `termux-api`;
  sin ellos dan el error correspondiente.
- Ventanas y GUI en vivo, audio con salida a dispositivo y `titan debug`/DAP: sin probar en ARM64.

## 3. Compilar Titan desde las fuentes

`bash selfhost/bootstrap.sh` usa una semilla x86-64, así que **no sirve en ARM64**. Para tener `titan` en un teléfono:

1. Usa el paquete ARM64 ya fabricado, o
2. en un PC x86-64, haz el bootstrap y compila la CLI para ARM64: `selfhost/titan compile selfhost/titan.titan -o titan-aarch64 --target aarch64`
   (necesita `clang` y `lld`, o `TITAN_CC="python3 -m ziglang cc"`). Después, en ARM64, `./titan-aarch64 compile selfhost/titan.titan -o titan`
   compila la CLI en el propio dispositivo (en el runner ARM64 de pruebas tardó 2 min 17 s).

## 4. Si algo falla

Abre un issue con la salida de estos comandos:

```sh
uname -a
titan version
titan run hola.titan
```

# Guía: compilar TITAN/Zett en Termux (Android aarch64)

Para Redmi 9C (Helio G22, ~2-3 GB RAM). Esta guía te deja compilar y
testear el proyecto localmente para validar el fix del offline-fetch y
confirmar que el binario `zett` compila en el target real de Android.

---

## 1. Instalar Rust y el toolchain de build

En Termux:

```sh
pkg update -y
pkg install -y rust clang make perl pkg-config git
```

Por qué cada paquete:

| Paquete      | Para qué                                                   |
|--------------|------------------------------------------------------------|
| `rust`       | `rustc` + `cargo` (la versión de Termux es reciente)       |
| `clang`      | Compila SQLite desde C (`rusqlite` con feature `bundled`)  |
| `make`       | Lo usan los build scripts de algunas crates                |
| `perl`       | Lo necesita el build de `libsqlite3-sys`                   |
| `pkg-config` | Detección de libs (por si alguna dep lo pide)              |
| `git`        | Clonar/actualizar el repo                                  |

Comprueba que cargo responde:

```sh
cargo --version
rustc --version
```

## 2. Traer el código (rama con el fix del offline-fetch)

```sh
git clone https://github.com/alexsndersoto04-source/aio
cd aio
git fetch origin arena/01a01dfc-aio
git checkout arena/01a01dfc-aio
git pull origin arena/01a01dfc-aio
```

Si ya tenías el repo clonado antes, simplemente:

```sh
cd aio
git fetch origin
git checkout arena/01a01dfc-aio
git pull origin arena/01a01dfc-aio
```

El commit con el fix es `4b4f063` (*Make offline package fetch never hit
the network on a cache miss*).

## 3. Compilar (cuidado con la RAM del Redmi 9C)

El Redmi 9C tiene poca RAM, así que limitamos los jobs para no quedarnos
sin memoria (OOM kill):

```sh
# Primera compilación: solo verificar que arma (puede tardar 15-40 min)
cargo build --workspace -j2
```

Si se cae por memoria (ves `Killed`), baja a un solo job:

```sh
cargo build --workspace -j1
```

> Truco: si Termux se cierra al compilar, activa el "acquire wakelock"
> (`termux-wake-lock`) para que Android no mate el proceso en background,
> y mantén la pantalla con la app de Termux abierta.

## 4. Probar el fix del offline-fetch

El fix está en `crates/titan_pkg`. Para validarlo rápido:

```sh
cargo test -p titan_pkg -j2
```

Esto compila solo el crate `titan_pkg` y corre sus tests. Debería pasar
`resolves_highest_matching_semver`, `verifies_sha256_and_rejects_insecure_registry`
y `verifies_ed25519_release_signature`.

## 5. Tests de todo el workspace

Cuando quieras la validación completa (tarda bastante en el Redmi 9C):

```sh
cargo test --workspace -j2
```

Si `titan_sqlite` (que compila SQLite en C) da problemas, prueba:

```sh
cargo test --workspace -j1 --exclude titan_sqlite
cargo test -p titan_sqlite -j1
```

## 6. Cazar tests intermitentes desde el teléfono

Algunos tests dependen del reloj: abren sockets locales, esperan a que salte
un deadline o coordinan hilos. Esos tests pasan siempre en una máquina rápida
y descansada, y fallan **de vez en cuando** en un runner de CI lento y
cargado. Es el fallo más difícil de diagnosticar, porque el código está bien:
lo que falla es el margen de tiempo.

Aquí el Redmi 9C deja de ser una limitación y pasa a ser la mejor herramienta
del proyecto. Un runner de macOS en GitHub es lento; **un Helio G22 con 2 GB
de RAM lo es mucho más**. Si un test sobrevive treinta repeticiones seguidas
en el teléfono, sus márgenes aguantan cualquier cosa que GitHub le eche
encima.

```sh
# 10 repeticiones del grupo sensible al tiempo
scripts/flaky-check.sh

# 30 repeticiones, compilando con un solo job (menos RAM)
scripts/flaky-check.sh -n 30 -j 1

# la suite completa, 5 veces (tarda mucho en el teléfono)
scripts/flaky-check.sh -a -n 5
```

El script compila los binarios de test una sola vez y luego repite solo la
ejecución, así que la segunda repetición y las siguientes son rápidas. Si
algo falla te imprime **el nombre exacto del test** y guarda el log completo
en `flaky-check-failure-*.log`, que es justo lo que hace falta para
arreglarlo sin adivinar.

Para forzar aún más la situación, corre el script con el teléfono ocupado:
otra app pesada abierta, o una segunda sesión de Termux compilando. Esa es la
condición que reproduce el CI cargado.

## 7. Compilar el binario `zett`

El CLI se llama `zett`:

```sh
cargo build -p titan_cli --release -j2
```

El binario queda en `target/release/zett` (o `target/release/titan` según
el nombre del binario en `crates/titan_cli/Cargo.toml`).

## 8. Qué estamos validando y por qué

| Objetivo                          | Comando                          | Estado                 |
|-----------------------------------|----------------------------------|------------------------|
| Fix offline-fetch compila         | `cargo test -p titan_pkg`        | ✅ fix re-aplicado     |
| Workspace arma en aarch64         | `cargo build --workspace`        | ⏳ a validar en Termux |
| Tests pasan en Android real       | `cargo test --workspace`         | ⏳ a validar en Termux |
| Binario `zett` funcional          | `cargo build -p titan_cli`       | ⏳ a validar en Termux |
| Márgenes de tiempo sin flakes     | `scripts/flaky-check.sh -n 30`   | ⏳ a validar en Termux |

> **Nota sobre cross-platform (Windows/macOS):** Termux es Linux, así que no
> reproduce un fallo *específico* de Windows o macOS (rutas, resolución de
> `localhost` a IPv6, APIs del sistema). Para esos casos hacen falta los
> **logs de CI de GitHub** (Actions → el run que falla → copiar el error).
> Pero sí sirve, y muy bien, para la otra familia de fallos de esa matriz:
> los **intermitentes por tiempo**. Ahí el teléfono es más exigente que
> cualquier runner de GitHub; ver la sección 6.

## 9. Si algo falla: qué pegarme

Cuando un comando falle, copiame **desde la primera línea de error** hasta
el final. Concretamente:

- `cargo build` falla → las líneas con `error[E...]` y el contexto.
- `cargo test` falla → el `panicked at` y qué test fue.
- CI de Windows/macOS falla → el error del job en la pestaña Actions.

Con eso puedo arreglarlo sin adivinar.

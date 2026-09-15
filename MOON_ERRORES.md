# Moon — los 19 errores de tipo: **arreglados** (2026-09-12)

> **Estado final (12-sep-2026, commit `bb4f99c`):** `zett check` verde y el
> **E2E completo verde en CI** — todo el recorrido contra PostgreSQL 16 real
> (5 checks del run en `success`: CI `cargo test`, *titan-check*,
> *frontend-build*, Termux AArch64 y Termux ARM 32-bit). El backend arranca,
> aplica las 15 migraciones y atiende todo el recorrido de la aplicación,
> incluido el WebSocket en vivo. Detalle del recorrido y de los arreglos:
> [`INFORME_PROYECTOS.md`](INFORME_PROYECTOS.md) §4.3 y §7.

`zett check projects/moon/src/main.titan` **pasa**. Evidencia: workflow
*Moon checks*, job `titan-check`, paso «Compilar el backend Titan (zett check)»
= **success** en el commit `070b556` (run 34707099648). En ese mismo commit
están verdes además CI (formato + compilación + pruebas), Termux ARM 32-bit
y Termux AArch64.

Reproducible con: `zett check projects/moon/src/main.titan` (Termux) o
`cargo run -p titan_cli -- check projects/moon/src/main.titan`.

## Cómo se obtuvieron los 19 diagnósticos

El workflow publicaba solo la primera línea («CHECK FAILED:»). Ahora el
comprobador imprime `archivo:línea:columna: mensaje`, y la CLI lo publica
como anotación de GitHub cuando corre en Actions. Con eso se vio la lista
completa en el commit `7188757`; los 19 salieron del comprobador, ninguno
del lexer ni del parser de sintaxis.

## Los 19, uno por uno, y qué los arregló

Cuatro reglas del comprobador y un detalle del parser explicaban 17 de los
19; los otros dos eran código de Moon que pedía un ajuste.

| # | Ubicación (antes) | Diagnóstico | Arreglo |
|---|---|---|---|
| 1 | `db.titan:94` | `expected PostgresPool, found Nil` | `std::process::exit` es `Never` |
| 2 | `db.titan:122` | `invalid operands for Sub: Nil and Int` | parser: `-1` en su línea |
| 3 | `validate.titan:66` | `non-exhaustive match` | brazo `_` en Moon |
| 4 | `realtime.titan:16` | `expected Int, found Sender` | Moon: el hub es `Sender` |
| 5 | `realtime.titan:31` | `expected Unit, found map` | unión de ramas en sentencia |
| 6 | `realtime.titan:58` | `expected Unit, found map` | unión de ramas en sentencia |
| 7 | `realtime.titan:104` | `expected map, found Unit` | unión de ramas en sentencia |
| 8 | `realtime.titan:120` | `expected Sender, found Int` | Moon: el hub es `Sender` |
| 9 | `realtime.titan:124` | `expected Sender, found Int` | Moon: el hub es `Sender` |
| 10 | `realtime.titan:198` | `expected Bool, found Unit` | unión de ramas + inferencia |
| 11 | `realtime.titan:222` | `expected Bool, found Unit` | unión de ramas + inferencia |
| 12 | `h_messages.titan:13` | `invalid operands for Sub: Unit and Int` | parser: `-1` en su línea |
| 13 | `h_notifications.titan:108` | `expected Unit, found map` | unión de ramas en sentencia |
| 14 | `h_notifications.titan:125` | `expected Unit, found map` | unión de ramas en sentencia |
| 15 | `h_admin.titan:60` | `expected Array(Unknown), found Nil` | `valor == nil` siempre vale |
| 16 | `h_media.titan:54` | `expected Nil, found map` | contexto `any` no exige tipo común |
| 17 | `h_media.titan:116` | `expected Int, found Unit` | unión de ramas en sentencia |
| 18 | `h_media.titan:192` | `non-exhaustive match` | brazo `_` en Moon |
| 19 | `main.titan:529` | `expected Task, found Unit` | unión de ramas en sentencia |

## Las cinco reglas del lenguaje que estaban mal (y ya no)

1. **Un `if`/`match` usado como sentencia descarta su valor**, pero el
   comprobador exigía que las dos ramas tuvieran el mismo tipo. Así,
   `if flag { act() } else { n = 1 }` era un error aunque nadie use el
   resultado. Ahora, sin tipo esperado, una rama que solo actúa convive con
   otra que produce un valor: la expresión vale `Unit`. (Errores 5, 6, 7,
   10, 11, 13, 14, 17, 19.)
2. **En un contexto gradual (`-> any`) tampoco se exige tipo común**: cada
   rama ya se comprobó contra el contrato declarado. (Error 16.)
3. **La inferencia de retorno** de una función que solo devuelve valor en
   algunas rutas ya no falla con `InconsistentReturns`: infiere `Unit`,
   que es lo honesto (no hay valor que usar). (Errores 10 y 11.)
4. **`valor == nil` / `!= nil` vale para cualquier tipo**. Antes solo valía
   si el comprobador ya sabía que el valor podía ser nil, así que comparar
   un `array` anotado con `nil` —la forma normal de preguntar «¿está
   vacío/no vino?»— era un error. (Error 15.)
5. **`std::process::exit` se tipa `Never`** (no vuelve al que lo llama),
   igual que `abort()` en C. Una rama que termina ahí no tiene que producir
   el valor de las demás ramas. (Error 1.)

Y un detalle del **parser**: un operador escrito pegado a su operando
(`-1`, `*p`, `&x`) y con espacio antes es un operador **prefijo**, no la
continuación de la línea anterior. Antes, `print(msg)` seguido de `-1` en
la línea siguiente se analizaba como `print(msg) - 1`: de ahí los errores 2
y 12 (`Sub: Nil and Int`, `Sub: Unit and Int`) y el riesgo de aritmética
silenciosa sobre el resultado de la sentencia anterior. La regla es la de
Swift, y no rompe nada del repositorio: en todo el código `.titan` de
`examples/`, `docs/` y `projects/` solo había esas dos líneas afectadas
(las continuaciones con `+ "texto"` llevan espacio después del operador y
siguen siendo binarias).

## Los dos ajustes de Moon

- `realtime.titan`: `fn rt_start() -> Sender` (era `-> int`) y `hub: Sender`
  en `rt_emit`, `rt_broadcast`, `rt_handle_message`, `n_notify` y
  `hp_process_mentions`. El hub es un canal, no un número: decirlo en el
  tipo elimina los errores 4, 8 y 9.
- `validate.titan` y `h_media.titan`: brazo `_` en los dos `match` sobre
  valores dinámicos. La regla del lenguaje (documentada y con prueba) es
  que un `match` sobre `any` necesita un caso final; se respeta la regla en
  vez de relajarla.

## Pruebas nuevas

- Comprobador (6): ramas en sentencia, ramas en contexto `any`, función que
  solo a veces devuelve valor, `exit` que no vuelve, `== nil` con `array` y
  con `map`, y `-1` en su propia línea.
- Parser (2): el `-` pegado a su operando empieza expresión nueva, y el `+`
  con espacio a los dos lados sigue continuando la expresión.

## Siguiente hito (lo único que falta para «funcional»)

Compilar ya compila; ahora hay que **verlo correr contra Postgres real**. El
arnés completo está en esta misma rama:

- `projects/moon/LOCAL.md` — guía paso a paso (Postgres, API, frontend).
- `projects/moon/ops/setup-local.sh` — crea un Postgres embebido (npm
  `@embedded-postgres/linux-x64`), idempotente.
- `projects/moon/ops/start-api.sh` — arranca la API con la BD local, el
  `JWT_SECRET` y el puerto.
- `projects/moon/ops/reset-db.sh` — vuelve a dejar la base vacía.
- `projects/moon/test/e2e.mjs` — 467 líneas, ~90 comprobaciones: registro,
  login, 2FA, refresh/rotación, posts, feed, comentarios, likes, guards,
  follows/blocks, mensajería por WebSocket, notificaciones, subida de
  imágenes, reportes, admin y seguridad (404/405/rate limit/CORS).

En este entorno de trabajo no hay `zett` compilado, ni Postgres, ni Docker,
así que ese paso hay que correrlo en la máquina del usuario (o en Render):

```sh
cd projects/moon
bash ops/setup-local.sh          # Postgres embebido + base de datos
bash ops/start-api.sh &          # API en :3000 (usa bin/zett)
API_BASE=http://127.0.0.1:3000 MOON_LOG=/tmp/moon-server.log \
  node test/e2e.mjs              # debe imprimir el resumen PASS/FAIL
```

Y si quieres que ese E2E corra solo en cada push (es un archivo de workflow, y
desde esta sesión no se pueden subir; pégalo tú en
`.github/workflows/check-moon.yml`):

```yaml
  moon-e2e:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16
        env:
          POSTGRES_USER: moon
          POSTGRES_PASSWORD: moon
          POSTGRES_DB: moon
        ports: ["5432:5432"]
        options: >-
          --health-cmd "pg_isready -U moon" --health-interval 5s
          --health-timeout 5s --health-retries 20
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Compilar el compilador
        run: cargo build --release -p titan_cli
      - name: Arrancar Moon contra Postgres
        env:
          DATABASE_URL: postgres://moon:moon@127.0.0.1:5432/moon
          JWT_SECRET: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
          PORT: "3000"
        run: |
          ./target/release/titan run projects/moon/src/main.titan > /tmp/moon.log 2>&1 &
          for _ in $(seq 1 60); do
            curl -sf http://127.0.0.1:3000/health >/dev/null && break
            sleep 1
          done
      - name: E2E de 467 líneas
        env:
          API_BASE: http://127.0.0.1:3000
          MOON_LOG: /tmp/moon.log
        run: node projects/moon/test/e2e.mjs
```

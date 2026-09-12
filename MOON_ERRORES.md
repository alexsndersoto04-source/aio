# Moon — Los 19 errores que impiden compilar el backend

**Cómo se obtuvo:** `zett check projects/moon/src/main.titan`, ejecutado en CI
(workflow *Moon checks*, job `titan-check`). Hasta el commit `64e5a53` ese
chequeo solo publicaba la primera línea del error («CHECK FAILED:»), así que la
causa era invisible. Ahora cada diagnóstico nombra su archivo.
Reproducible con: `zett check projects/moon/src/main.titan` (Termux) o
`cargo run -p titan_cli -- check projects/moon/src/main.titan`.

Ya no hay **errores de sintaxis**: el parseo pasa entero (los dos que había se
corrigieron: ver commits `b142ba2` y `3992c82`). Lo que queda son 19 errores de
**tipos**, repartidos en 6 archivos:

| # | Ubicación | Diagnóstico |
|---|---|---|
| 1 | `db.titan:94:9` | `type mismatch: expected Named("PostgresPool"), found Nil` |
| 2 | `db.titan:122:13` | `invalid operands for Sub: Nil and Int` |
| 3 | `validate.titan:66:9` | `non-exhaustive match; add a catch-all arm` |
| 4 | `realtime.titan:16:1` | `type mismatch: expected Int, found Named("Sender")` |
| 5 | `realtime.titan:31:17` | `type mismatch: expected Unit, found Named("map")` |
| 6 | `realtime.titan:58:21` | `type mismatch: expected Unit, found Named("map")` |
| 7 | `realtime.titan:104:21` | `type mismatch: expected Named("map"), found Unit` |
| 8 | `realtime.titan:120:32` | `type mismatch: expected Named("Sender"), found Int` |
| 9 | `realtime.titan:124:32` | `type mismatch: expected Named("Sender"), found Int` |
| 10 | `realtime.titan:198:25` | `type mismatch: expected Bool, found Unit` |
| 11 | `realtime.titan:222:13` | `type mismatch: expected Bool, found Unit` |
| 12 | `h_messages.titan:13:5` | `invalid operands for Sub: Unit and Int` |
| 13 | `h_notifications.titan:108:5` | `type mismatch: expected Unit, found Named("map")` |
| 14 | `h_notifications.titan:125:5` | `type mismatch: expected Unit, found Named("map")` |
| 15 | `h_admin.titan:60:8` | `type mismatch: expected Array(Unknown), found Nil` |
| 16 | `h_media.titan:54:1` | `type mismatch: expected Nil, found Named("map")` |
| 17 | `h_media.titan:116:13` | `type mismatch: expected Int, found Unit` |
| 18 | `h_media.titan:192:13` | `non-exhaustive match; add a catch-all arm` |
| 19 | `main.titan:529:5` | `type mismatch: expected Named("Task"), found Unit` |

## Patrones que se repiten

- **Comparar con `nil`** (`if conn != nil`, `if rows == nil`, `if current == nil`).
  El comprobador espera el tipo del valor y encuentra `Nil`, o al revés.
  Errores 5, 6, 7, 10, 11, 13, 14, 15.
- **`Option` / `Result` con `match` incompleto**: falta el brazo final
  (errores 3 y 18) — el lenguaje exige cubrir todos los casos.
- **Concurrencia**: `spawn` (que devuelve `Task`) usado como sentencia y por
  tanto tipado como `Unit` (errores 4, 19); el canal (`channel`) comparado con
  `Sender` (8, 9).
- **Funciones que devuelven `nil` en una rama y un `map` en otra** con el tipo
  de retorno declarado `any` (error 16, en `hm_process`).
- **Aritmética sobre valores que pueden ser `nil`** (errores 2, 12, 17).

Cada uno hay que decidirlo en su sitio: unos son el código de Moon pidiendo un
ajuste (por ejemplo, cubrir el `match`), y otros apuntan a reglas del
comprobador que conviene relajar o corregir en el lenguaje —arreglarlo en el
lenguaje beneficia a cualquier programa, no solo a Moon—.

## Siguiente paso

1. Ir error por error, empezando por los dos `match` no exhaustivos (3 y 18):
   son los más mecánicos.
2. Revisar el tratamiento de `nil` en comparaciones: si el comprobador está
   siendo más estricto de lo razonable con `any` / `Option` / `Result`, el
   arreglo va en `crates/titan_typechecker` y desaparecen varios de golpe.
3. Repetir `zett check` tras cada bloque (CI tarda ~2,5 min por vuelta).
4. Cuando el chequeo pase: levantar Postgres local con el `ops/setup-local.sh`
   que ya existe y correr el E2E de 467 líneas (`projects/moon/test/e2e.mjs`,
   hoy solo en la rama `arena/01a040eb-aio`) — ese es el hito que convierte
   Moon en «producto que funciona».

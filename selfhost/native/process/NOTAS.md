# std::process en Titan — notas de diseño

Código: `selfhost/native/std_process.titan` (22 funciones) y, en
`runtime.titan`, `rt_environ` / `rt_getenv` (el entorno que ven todos).

Referencia: `crates/titan_stdlib/src/process_mod.rs` (21 funciones, llamadas
desde `crates/titan_vm/src/native.rs`) y `run_timeout` de
`crates/titan_stdlib/src/process.rs` (`CommandSpec::output_timeout`). Rust
lanza los programas con `std::process::Command`, que en Linux usa
`posix_spawnp` de glibc.

## Entorno

Rust cambia el entorno del proceso (`setenv`/`unsetenv` de glibc) y todo lo
demás lo ve: `std::env::get`, `std::dirs` (HOME, XDG_*), el `tput` de
`std::term::size` y los procesos hijos. En el runtime nativo el entorno
empieza siendo el de la pila inicial; con el primer `env_set`/`env_unset` se
hace una copia propia de la lista de punteros (`globals + 2576`), y
`rt_environ()` devuelve la que toque. Todos los lectores pasan por ahí.

- `env_set`: como `setenv(n, v, 1)` de glibc: cambia la **primera** entrada
  con ese nombre o añade una al final (el orden importa en `env_vars`).
- `env_unset`: como `unsetenv`: quita **todas** las entradas con ese nombre.
- `env_get("")` y nombres con NUL: "no está", igual que Rust.
- `env_vars`: `vars_os()` de Rust: se parte en el primer `=` que no sea el
  primer carácter; entradas sin `=` se saltan.
- Las cadenas viejas no se liberan (glibc tampoco lo hace con `setenv`).

## Lanzar un programa (`ps_spawn`)

Lo mismo que hace `posix_spawnp` con los ajustes que pone Rust:

1. `fork`.
2. En el hijo: `dup2` de entrada, salida y error a 0, 1, 2 en ese orden (si
   el descriptor ya es el de destino se le quita `O_CLOEXEC`, como glibc).
3. Máscara de señales vacía (`POSIX_SPAWN_SETSIGMASK`); importa porque
   `std::term` bloquea SIGWINCH y `std::signals` bloquea las que instala.
4. SIGPIPE por defecto (`POSIX_SPAWN_SETSIGDEF`).
5. `__execvpex` de glibc:
   - Con `/` en el nombre, exec directo. Nombre vacío: ENOENT.
   - Si no, cada entrada de `PATH` del entorno actual (vacía = carpeta
     actual). Sin `PATH`: `/bin:/usr/bin`.
   - ENOENT, ESTALE, ENOTDIR, ENODEV, ETIMEDOUT y EACCES pasan a la
     siguiente entrada; cualquier otro error (p. ej. ENOEXEC) para.
   - Si nada funciona, el error es el del **último** intento, o EACCES si
     alguno lo dio. Comprobado con `error_process_path_enotdir` (la última
     entrada es un archivo, así que el error es "Not a directory") y
     `error_process_path_eacces`.
   - Sin plan B con `sh` para ENOEXEC (posix_spawnp no lo tiene):
     `error_process_formato_exec`.
6. Si exec falla, el hijo escribe el errno en un tubo `O_CLOEXEC` y termina
   con 127. El padre lee ese tubo:
   - 0 bytes: exec funcionó.
   - 8 bytes: recoge al hijo y devuelve el error (como `posix_spawnp`, que
     devuelve el errno del hijo).

Un NUL en el programa o los argumentos: Rust lo rechaza antes de crear nada
("nul byte found in provided data").

Todos los tubos y descriptores del runtime llevan `O_CLOEXEC`, así que los
hijos no heredan nada más (Rust igual).

## Leer y escribir a la vez (`ps_pump`)

Rust usa un hilo por cada tubo de salida (y otro para escribir la entrada).
Para las funciones que esperan al hijo (`run`, `run_with_input`, `shell`,
`pipe`, `run_timeout`) basta con un bucle con `poll` sobre todos los tubos,
con el mismo resultado: nadie se bloquea con un tubo lleno.

- Cupo compartido de 4 MiB entre todas las salidas de la llamada
  (`CaptureBudget`). Pasado el cupo se sigue leyendo y se descarta, y al
  final da el error.
- La entrada se escribe sin bloquear. Rust ignora SIGPIPE siempre, así que
  escribir a un hijo que ya cerró su entrada da EPIPE, no la muerte. Aquí
  SIGPIPE se ignora solo mientras se escribe, y después se deja como estaba.
  Caso determinista: `error_process_epipe` (1 MiB a `true`).
- Qué error gana si hay varios, como en `finish_process`:
  1. esperar al hijo;
  2. escribir la entrada;
  3. leer stdout;
  4. leer stderr;
  5. cupo excedido.

  En `pipe`:
  1. esperar a la última etapa;
  2. esperar a las demás;
  3. leer stdout;
  4. el primer stderr que falló;
  5. cupo excedido.
- `pipe`, si una etapa no se puede lanzar: SIGKILL a las anteriores, esperar
  a todas, leer sus stderr hasta el final y dar el error de lanzamiento.
- `run_timeout`: Rust comprueba cada 5 ms (`try_wait`, ¿pasó el plazo?,
  dormir 5 ms). Aquí igual, pero esos 5 ms se pasan leyendo los tubos.

## Procesos en segundo plano (`spawn`)

Aquí sí hace falta algo que lea mientras el programa hace otra cosa: si el
hijo escribe más de 64 KiB y nadie lee, se queda bloqueado. En Rust lo hacen
dos hilos. El runtime nativo no tiene hilos, porque `std::raw` no tiene
punteros a funciones ni un `clone` con pila nueva. Por eso lo hace un
**proceso ayudante**:

- Es un fork del programa, hecho después de cerrar en el padre los extremos
  de escritura; así el final de los tubos llega cuando el hijo termina.
- Se queda solo con sus tres descriptores: stdout y stderr del hijo, y un
  socket con el programa. Todos los demás los cierra.
- `PR_SET_PDEATHSIG = SIGKILL`: muere si el programa muere, como los hilos
  mueren con el proceso.
- Lee los dos tubos con el mismo cupo de 4 MiB. Al llegar al final de los
  dos, manda por el socket:
  - excedido;
  - errno de stdout;
  - errno de stderr;
  - largo de stdout;
  - largo de stderr;
  - los bytes.

  Después termina.
- `spawn_wait` espera al hijo, lee eso del socket y recoge al ayudante.

Diferencia honesta: el ayudante se ve como un proceso más (en `ps`), donde
Rust tiene dos hilos. El resultado para el programa es el mismo: salida,
códigos, errores y tiempos.

Tabla (32 filas, como el límite de Rust): número, pid, estado recogido
(`try_wait` lo guarda, y `kill` después de recogerlo no hace nada, como
`Child::kill`), pid del ayudante, socket e inicio. Los números empiezan en 1
y cada `spawn` que pasa de `parse_cmd` y del límite gasta uno, aunque luego
falle al lanzar (`next_handle()` va antes del `spawn` en Rust).

### Al terminar el programa

Medido con la VM de Rust (`sleep 7.77` en segundo plano):

- **Fin normal y error de ejecución**: el `Drop` de `RuntimeState` llama a
  `cleanup_runtime` (primero las barras de progreso, después los procesos).
  A cada uno que siga vivo le manda SIGKILL, lo espera y espera a sus
  lectores. Aquí `ps_cleanup()` hace lo mismo en `rt_finish` y `rt_fatal`,
  después de `pg_cleanup()`.
- **`std::process::exit`**: Rust no ejecuta destructores; el hijo **sigue
  vivo** y los hilos lectores mueren con el proceso. Aquí es `exit_group`
  directo: el hijo sigue vivo y el ayudante muere por `PDEATHSIG`.
  Comprobado: 0 ayudantes quedan vivos.

## Límites y mensajes

| Qué | Límite | Mensaje |
|---|---|---|
| procesos a la vez | 32 | `process count exceeds limit 32` |
| largo del comando | 65536 | `command bytes exceeds limit 65536` (run_timeout: `exceed`) |
| entrada | 8 MiB | `process input bytes exceeds limit 8388608` |
| salida capturada | 4 MiB | `captured process output bytes exceeds limit 4194304` (run_timeout: `exceed`) |
| etapas de pipe | 8 | `pipeline commands exceeds limit 8` |

Prefijos: `spawn error: ` al lanzar y `process io error: ` al leer, escribir
o esperar (`run_timeout` no pone prefijo). `send_signal` usa el formato de
nix (`signal error: ESRCH: No such process`). Solo acepta las señales 1 a 31
(`Signal::try_from`).

## Defectos de Rust encontrados (corregidos en los dos lados)

- **Bug 16**: `std::process::spawn` no se podía escribir (`spawn` es
  palabra clave), así que las otras cuatro funciones `spawn_*` no servían.
  Arreglado en el parser (Rust y Titan) junto con el 11 (`std::uuid::nil`).
- **Bug 17**: `env_set`/`env_unset` con nombre vacío, con `=` o con NUL (o
  un valor con NUL) hacían *panic* y la VM se caía. Ahora es un error:
  `invalid environment variable: name must be non-empty and must not
  contain '=' or NUL` / `... value must not contain NUL`.
- **Bug 18**: `env_vars` hacía *panic* si alguna variable no era UTF-8. Ahora
  esas se saltan (como `env_get`, que da None para ellas).

## Lo que queda abierto

- `std::process::args` / `std::env::args` con argumentos que no son UTF-8:
  la VM nunca llega a verlos, porque la línea de comandos de `zett` los
  rechaza antes ("invalid UTF-8 was detected in one or more arguments"). En
  el ejecutable nativo se devuelven tal cual. No hay comportamiento de Rust
  con el que comparar.
- `hostname`, `username`, `self_pid` y `args` dependen de la máquina y de
  cómo se lanza el programa. Las pruebas comparan lo que es igual en los dos
  (hostname y usuario sí; el pid y el nombre del programa no).

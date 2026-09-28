# std::term en Titan: notas

Reproduce `crates/titan_stdlib/src/term_mod.rs`, que usa crossterm 0.28.1
(características por defecto: bracketed-paste, events; sin libc, así que
debajo están rustix 0.38.44 con llamadas directas al núcleo y mio 1.2.2).
El código original está en `selfhost/fuentes/*.crate`.

Archivos: `std_term.titan` (salida, colores, termios, tamaño, tput) y
`std_term_keys.titan` (analizador de teclas y lector de eventos).

## Salida

- `execute!` escribe en `io::stdout()` y vacía. El runtime no tiene búfer,
  así que cada llamada escribe todo de una vez: los bytes salen iguales y
  en el mismo orden respecto de `print`.
- El stdout de Rust trata EBADF (descriptor 1 cerrado) como éxito.
- `Colored` no escribe nada si `NO_COLOR` no está vacío (se mira una sola vez
  en todo el proceso). Queda `ESC[m`, incluso para "reset".
- `SetAttribute` usa la tabla SGR por posición del enum: bold 1, dim 2,
  italic 3, underlined 4, reverse 7, hidden 8, reset 0.
- `MoveTo(c, r)` escribe `r + 1` y `c + 1` como u16: con 65535 da 0 (Rust
  compilado en release no revisa el desborde).
- `"#rrggbb"` acepta `+` delante de cada par (`from_str_radix`), igual que
  `rgb:` acepta `+` en cada número.

**Defecto de Rust corregido:** `"#a€bc"` mide 6 bytes pero cortar en el byte 2
cae dentro de `€`, y Rust hacía pánico (el programa moría con código 101).
Corregido en `term_mod.rs` con `str::get` (y una prueba). El runtime en
Titan da el error correcto (`unknown color: '#a€bc'`). El `zett` precompilado
sigue teniendo el pánico hasta que se recompile.

## Modo crudo (rustix)

- La terminal: stdin si es terminal (`isatty` de rustix = TIOCGWINSZ
  funciona); si no, `/dev/tty` con O_RDWR|O_CLOEXEC (se cierra al final).
- `tcgetattr`: TCGETS2 (termios2, 44 bytes). Si da ENOTTY o EACCES, TCGETS y
  las velocidades se deducen de c_cflag (ERANGE si no son conocidas).
- `cfmakeraw`: igual que glibc. `tcsetattr(TCSANOW)`: TCSETS2, con el mismo
  plan B (TCSETS no acepta BOTHER: ERANGE).
- El termios original se guarda solo si todo salió bien; activar dos veces no
  hace nada; desactivar sin haber activado tampoco.

## Tamaño

1. `/dev/tty` de solo lectura (o stdout si no se puede abrir) y TIOCGWINSZ;
   cualquier valor sirve, incluso 0.
2. Si falla: `tput cols` y `tput lines` (los dos, siempre). Se lanzan de
   verdad: fork, stdin en /dev/null, salidas en tubos, máscara de señales
   vacía y la búsqueda en PATH de `__execvpe` de glibc (entrada vacía = el
   directorio actual; EACCES gana si apareció; CS_PATH `/bin:/usr/bin` si no
   hay PATH). Después, `read2` de Rust: tubos sin bloqueo y poll.
3. Se toman los dígitos de la salida (u16 con desborde); tiene que ser > 0.
4. Si alguno falla: `io::Error::last_os_error()`, o sea el errno que haya
   dejado la última llamada fallida. Rust lanza tput con `posix_spawnp`; el
   hijo de glibc comparte la memoria con el padre, así que los `execve`
   fallidos del hijo dejan su errno en el padre. El hijo nativo se lo cuenta
   al padre por un tubo con O_CLOEXEC, uno por intento.

**Carrera (en el propio Rust):** sin TERM, tput falla y el errno final depende
de si una lectura del tubo encontró EAGAIN antes de que tput terminara.
Medido con la salida a archivos: Rust dio "os error 11" 19 de 20 veces y
"os error 2" una; el nativo 18 y 2. Por eso ese caso no está en la batería
general (no es determinista ni en Rust).

## read_key (mio + signal-hook + parse.rs)

- El lector se crea una vez (la terminal, epoll_create1, registro con
  EPOLLIN|EPOLLRDHUP|EPOLLET, SIGWINCH). Si falla, queda sin fuente para
  siempre: "Failed to initialize input reader".
- signal-hook instala un manejador de SIGWINCH. El runtime no tiene punteros a
  funciones: bloquea SIGWINCH y la lee de un signalfd (como std::signals).
  SIGWINCH se ignora por defecto, así que no cambia nada para el programa; los
  hijos (tput) empiezan con la máscara vacía, como con Rust.
- `try_read`: primero la cola del analizador; si no, epoll_wait con el plazo
  redondeado hacia arriba a ms (máximo 3 eventos). La terminal se lee de a
  1024 bytes hasta que el analizador produce un evento (la lectura bloquea:
  en Rust también). SIGWINCH produce Resize (y llama a size()).
- `poll` con EventFilter: los eventos internos (posición del cursor,
  capacidades del teclado) se saltan, pero cuentan como una vuelta: si el
  plazo ya pasó, `poll` devuelve false aunque haya una tecla detrás.
- El analizador es `parse.rs` línea por línea: ESC solo (si no hay más bytes),
  SS3, CSI, ratón (normal, SGR, rxvt), pegado entre corchetes, foco,
  protocolo de kitty (CSI u), teclas especiales (~), modificadores y UTF-8
  partido. Los errores descartan el búfer.
- `\n` es Enter fuera del modo crudo y Ctrl+j dentro (mira el estado del modo
  crudo en el momento de analizar).
- `format_key`: Ctrl, Alt, Shift (Shift no se muestra en caracteres); BackTab
  ya lleva SHIFT, así que sale "Shift+Shift+Tab". Otras teclas: su nombre
  Debug (`CapsLock`, `Media(Play)`, `Modifier(LeftShift)`...).

## Verificación

- `selfhost/tests/native/term_std.titan` y `error_term_*.titan`: en la
  batería general.
- `bash selfhost/native/term/verificar.sh`: tamaño con 7 entornos distintos y
  en una pseudoterminal, 40 teclas en modo crudo (con la configuración de la
  terminal antes y después), plazos, cambio de tamaño de la ventana, modo
  normal y NO_COLOR. 11 de 11 iguales byte a byte.
- `pty_keys.py` teclea en una pseudoterminal: espera "LISTO" y ejecuta pasos
  (esperar, escribir bytes, cambiar el tamaño, anotar termios).

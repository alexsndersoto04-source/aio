# Registro visible de validación

Este documento registra evidencia comprobable. Un cambio no se marca como
validado solamente porque el código parezca correcto: debe tener pruebas y una
ejecución externa en GitHub Actions. Las pruebas físicas en el Redmi se anotan
por separado porque CI no puede sustituir un teléfono real.

## Estado actual

- Rama de trabajo: `arena/019ff232-aio`
- Commit validado: `c21517cb28c666308a83264a9e456af230c0a830`
- Alcance: seguridad de capacidades, aislamiento y cuotas por runtime para tareas,
  canales, red y bases de datos; checks Android ARM de 32 y 64 bits
- Fecha: 2026-08-11

### Evidencia automatizada más reciente

| Comprobación | Resultado | Evidencia |
|---|---:|---|
| Formato completo, `cargo fmt --check` | Aprobado | [CI 31555988990](https://github.com/alexsndersoto04-source/aio/actions/runs/31555988990) |
| `cargo check` con características normales | Aprobado | [CI 31555988990](https://github.com/alexsndersoto04-source/aio/actions/runs/31555988990) |
| Tests del workspace, todos los targets | Aprobado | [CI 31555988990](https://github.com/alexsndersoto04-source/aio/actions/runs/31555988990) |
| `cargo check --no-default-features` | Aprobado | [CI 31555988990](https://github.com/alexsndersoto04-source/aio/actions/runs/31555988990) |
| Cross-check Android AArch64 | Aprobado | [CI 31555988990](https://github.com/alexsndersoto04-source/aio/actions/runs/31555988990) |
| AArch64 con compiladores reales de Android NDK y warnings estrictos | Aprobado | [Termux ARM 31555989007](https://github.com/alexsndersoto04-source/aio/actions/runs/31555989007) |
| Compilación y enlace Android/Bionic ARMv7 | Aprobado | [Termux ARM 31555989007](https://github.com/alexsndersoto04-source/aio/actions/runs/31555989007) |
| ELF32 ARM y paquete Debian `Architecture: arm` | Aprobado | [Termux ARM 31555989007](https://github.com/alexsndersoto04-source/aio/actions/runs/31555989007) |
| Artefacto Termux subido por GitHub | Aprobado | [Termux ARM 31555989007](https://github.com/alexsndersoto04-source/aio/actions/runs/31555989007) |

Los dos fallos advisory anteriores ya fueron corregidos:

- Se aplicó el `rustfmt` estable de CI a los 106 archivos Rust que no cumplían
  el formato. El verificador Phase 34 se hizo compatible con macros formateadas
  en varias líneas y volvió a confirmar 758 nativas únicas y 837 llamadas.
- AArch64 ahora encuentra automáticamente clang, clang++, `llvm-ar` y
  `llvm-ranlib` del Android NDK. Además, la ruta oficial ejecuta un segundo
  check AArch64 real con NDK, `--all-targets`, lockfile y warnings tratados como
  errores antes de construir el paquete ARMv7.

El workflow ARMv7 oficial usa Android NDK y `armv7-linux-androideabi`. Además de
compilar y enlazar, comprueba con `readelf` que el binario sea ELF32/ARM,
verifica los metadatos del `.deb`, extrae el paquete y compara byte por byte el
ejecutable empaquetado con el ejecutable construido.

### Aislamiento de estado mutable por VM

El commit validado elimina el estado compartido entre VMs en audio, juego, GUI,
input, portapapeles/notificaciones, ciclo de vida móvil y métricas. También
separa los cuatro emuladores freestanding (plataforma, CPU, memoria y MMIO), los
contadores de señales y los límites de frecuencia HTTP. Las métricas creadas
internamente por el dispatcher HTTP reciben de forma explícita el identificador
del runtime correcto; no caen en el runtime global por defecto.

Las regresiones crean dos dominios de runtime simultáneos y comprueban que el
segundo no puede leer handles, clicks, teclas, coordenadas, texto, eventos,
métricas, registros MMIO, mapas de páginas ni fallos de CPU del primero. Luego
reinician y limpian selectivamente el segundo y demuestran que el estado del
primero sigue intacto. Señales y rate limits tienen pruebas específicas de
consumo y cleanup independientes.

El asignador freestanding ya no crea un `Vec` proporcional a toda la memoria
física simulada. Usa asignación dispersa O(1) y valida overflow de direcciones.
Una regresión inicializa una región modelada de 1 TiB, asigna dos frames y
comprueba el conteo sin reservar cientos de millones de entradas host.

Evidencia externa:

- [CI 31555029376](https://github.com/alexsndersoto04-source/aio/actions/runs/31555029376): formato, checks normales y sin características por defecto, todos
  los tests y cross-check AArch64 aprobados.
- [Termux ARM 31555029202](https://github.com/alexsndersoto04-source/aio/actions/runs/31555029202): AArch64 NDK estricto, build ARMv7, verificación, paquete y artefacto
  aprobados.

### Primera capa de cuotas por runtime

Las tareas ya no pueden crear hilos del sistema sin límite. Cada runtime admite
por defecto 256 tareas pendientes de `join`; el embedder puede reducir esa
cantidad mediante `with_task_limit`. La reserva se realiza bajo el mismo lock
que el registro, antes de crear el hilo, por lo que dos tareas concurrentes no
pueden saltarse la cuota. Un `join` libera el slot. Los hijos heredan los límites
de instrucciones, profundidad, memoria y GC del padre, y `spawn_quota` solo
puede reducir la memoria disponible, nunca elevar el límite del padre.

Los canales tienen un máximo predeterminado de 1.024 por runtime y su capacidad
individual se rechaza antes de reservar memoria si supera 65.536 mensajes.
También se añadieron límites finitos para el estado nativo aislado:

- 256 buffers de audio, ondas de hasta 60 segundos y cinco minutos de muestras
  acumuladas;
- 1.024 widgets y 64 KiB por texto GUI;
- 256 teclas simultáneas y 32 puntos táctiles;
- portapapeles de 1 MiB, 256 notificaciones y 1.024 eventos móviles;
- 4.096 métricas y 4.096 claves de rate limit por runtime;
- 1.024 handlers de syscall freestanding;
- 16.384 frames host activos, mappings de página y registros MMIO;
- 256 regiones MMIO y 256 KiB de salida UART.

Las regresiones llenan cada cuota, comprueban el rechazo de una entrada extra y,
donde existe una operación de liberación o reset, demuestran que el slot vuelve
a estar disponible. Los límites se aplican antes de construir vectores grandes,
crear hilos o insertar nuevas entradas.

Evidencia externa de este bloque:

- [CI 31555029376](https://github.com/alexsndersoto04-source/aio/actions/runs/31555029376): formato, checks, tests, no-default-features y AArch64 aprobados.
- [Termux ARM 31555029202](https://github.com/alexsndersoto04-source/aio/actions/runs/31555029202): check NDK estricto, build ARMv7, verificación, paquete y artefacto aprobados.

### Cuotas de red y bases de datos

El commit `c21517c` añade dos contadores atómicos compartidos por todas las
tareas de una VM. El límite predeterminado es de 1.024 handles de red y 256
handles de base de datos. Quien integra la VM puede reducirlos con
`with_network_handle_limit` y `with_database_handle_limit`.

La cuota de red cubre listeners y streams TCP, routers HTTP, configuraciones y
streams TLS, decoders y conexiones WebSocket, y controles de servidor. La cuota
de datos cubre conexiones directas, pools y conexiones adquiridas de SQLite,
PostgreSQL y MySQL. Además, el tamaño solicitado para cualquiera de esos pools
queda limitado específicamente a 64 conexiones; ya no reutiliza el techo
genérico de 67.108.864.

Cada operación reserva su slot antes de abrir un socket, aceptar una conexión,
realizar un handshake o abrir/adquirir una base de datos. El permiso RAII
deshace automáticamente la reserva si la operación falla. El cierre o retirada
del registro libera el slot. Convertir un stream TCP/TLS existente en WebSocket
transfiere el mismo slot, en vez de cobrar dos handles por un único transporte.

Las regresiones verificadas por CI demuestran:

1. saturación y recuperación del permiso, además de una carrera de 32 hilos en
   la que un límite de cuatro nunca admite un quinto;
2. rechazo del segundo router con límite uno y reutilización después de cerrar
   un listener TCP local;
3. rechazo de la segunda base SQLite en memoria y reutilización después de
   cerrar la primera;
4. rechazo de un pool de 65 conexiones con el máximo específico de 64;
5. un eco WebSocket real sobre listener y streams locales con límite tres, lo
   que prueba que las conversiones TCP a WebSocket no cuentan dos veces.

Evidencia externa de este bloque:

- [CI 31555988990](https://github.com/alexsndersoto04-source/aio/actions/runs/31555988990): `cargo fmt --check`, checks normal y
  `--no-default-features`, todos los tests y cross-check AArch64 aprobados.
- [Termux ARM 31555989007](https://github.com/alexsndersoto04-source/aio/actions/runs/31555989007): check AArch64 NDK estricto, compilación y enlace Android/Bionic
  ARMv7, verificaciones del ELF y paquete, y subida del artefacto aprobados.

No fue necesario levantar servicios externos para estas regresiones: se usaron
sockets loopback y SQLite en memoria. PostgreSQL y MySQL sí compilaron en las
rutas host, AArch64 y ARMv7, pero este bloque no afirma haber conectado contra
servidores reales de esas dos bases.

La ejecución intermedia [CI 31555890863](https://github.com/alexsndersoto04-source/aio/actions/runs/31555890863) detectó un error de ownership en el código de la nueva prueba
concurrente; formato y cross-check AArch64 sí habían pasado. El commit
`c21517c` corrigió el test y la ejecución final anterior aprobó todos los pasos.
La ejecución intermedia no se cuenta como validación verde.

### Qué prueban las regresiones de limpieza de recursos

1. Al destruir el último estado de un runtime, sus handles de colecciones dejan
   de existir.
2. La limpieza no borra handles pertenecientes a otra VM viva.
3. Al destruir la VM principal, una tarea no unida que ejecuta un bucle infinito
   recibe cancelación.
4. Después de que la tarea observa la cancelación, el estado compartido del
   runtime se destruye; la prueba falla si queda retenido.
5. Existe una barrera de apagado para impedir que una tarea cree otra tarea
   después de que haya comenzado la destrucción del runtime.

La limpieza está conectada a los registros aislados de UI/móvil/audio,
métricas, señales, emuladores freestanding, límites HTTP, ventanas, KV/sled,
watchers, Redis, imágenes, tokenizadores, ONNX, PDF, barras de progreso,
routers, servidores HTTP, solicitudes pendientes, WebSockets, procesos en
segundo plano y las seis familias de colecciones. La ruta de procesos mata y
recolecta al hijo; KV intenta vaciar escrituras antes de cerrar; las ventanas
reales se liberan en el mismo hilo que las creó.

### Límites de esta validación

- Las cuotas de tareas, canales, red y bases de datos ya están conectadas. El
  bloque de crecimiento controlado todavía debe cubrir colecciones persistentes
  con handles, procesos en background y capturas de salida de procesos externos.
- CI prueba directamente la destrucción con handles de colecciones y tareas.
  Las demás rutas de limpieza compilan y son revisadas por Rust, pero este run
  no levanta un servidor Redis externo ni carga un modelo ONNX real para luego
  destruirlo.
- Los comandos de formato y del antiguo job AArch64 ya pasan, pero sus dos
  líneas `continue-on-error` permanecen en el workflow hasta aplicar desde
  GitHub web la plantilla corregida `docs/CI_WORKFLOW_TEMPLATE.yml`. La GitHub
  App de Arena no tiene permiso para modificar `.github/workflows`.
- Una ejecución intermedia de tests, [CI 31552230343](https://github.com/alexsndersoto04-source/aio/actions/runs/31552230343),
  falló de forma transitoria. No se reprodujo en las dos ejecuciones siguientes,
  incluida la ejecución final sin instrumentación temporal. Se conserva como
  señal para la auditoría de concurrencia; no se oculta ni se contabiliza como
  una validación aprobada.
- El candidato de este commit todavía no se ha instalado en el Redmi 9C. No se
  requiere instalar cada cambio interno; habrá una prueba física agrupada al
  cerrar un milestone importante.
- Un resultado verde no significa que TITAN 1.0 esté terminado. Solo valida el
  commit y el alcance descrito aquí.
- El workflow histórico `android-apk - NEON FRACTURE (FIXED)` no forma parte de
  la validación oficial y sus resultados se ignoran.

## Bloque de capacidades

El commit `ca7e3e7` protege terminal, readline, información de
procesos/directorios, señales y detectores del entorno, incluida la exigencia
combinada de filesystem e interfaz de usuario para el historial persistente de
readline.

- [CI 31548478710](https://github.com/alexsndersoto04-source/aio/actions/runs/31548478710)
- [Termux ARM 31548478652](https://github.com/alexsndersoto04-source/aio/actions/runs/31548478652)

## Ceremonia para candidatos físicos

Cuando se cierre un milestone:

1. Se elige un commit que tenga CI y Termux ARM verdes.
2. Se descarga el `.deb` producido por ese run, junto con `SHA256SUMS`.
3. En el Redmi se verifica el hash y la arquitectura del paquete.
4. El paquete nuevo sustituye al anterior; no se mantienen instalaciones por
   fase.
5. Se ejecuta `test-termux-release.sh` y los casos manuales específicos del
   milestone.
6. Se registra aquí el commit, paquete, comandos y resultado observado.

Hasta completar esos seis pasos, el estado se describe como **validado en CI y
empaquetado para ARMv7**, no como **validado físicamente en el Redmi**.

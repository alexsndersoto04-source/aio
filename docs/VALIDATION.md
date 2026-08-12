# Registro visible de validación

Este documento registra evidencia comprobable. Un cambio no se marca como
validado solamente porque el código parezca correcto: debe tener pruebas y una
ejecución externa en GitHub Actions. Las pruebas físicas en el Redmi se anotan
por separado porque CI no puede sustituir un teléfono real.

## Estado actual

- Rama de trabajo: `arena/019ff232-aio`
- Commit validado: `7c25b2368cee657236fa2e57daf89e9e8b046371`
- Alcance: seguridad de capacidades, aislamiento y cuotas por runtime para tareas,
  canales, red, bases de datos, procesos, colecciones, juego, señales, watchers,
  progreso, routers y ventanas virtuales/reales; checks Android ARM de 32 y 64 bits
- Fecha: 2026-08-11

### Evidencia automatizada más reciente

| Comprobación | Resultado | Evidencia |
|---|---:|---|
| Formato completo, `cargo fmt --check` | Aprobado | [CI 31559566600](https://github.com/alexsndersoto04-source/aio/actions/runs/31559566600) |
| `cargo check` con características normales | Aprobado | [CI 31559566600](https://github.com/alexsndersoto04-source/aio/actions/runs/31559566600) |
| Tests del workspace, todos los targets | Aprobado | [CI 31559566600](https://github.com/alexsndersoto04-source/aio/actions/runs/31559566600) |
| `cargo check --no-default-features` | Aprobado | [CI 31559566600](https://github.com/alexsndersoto04-source/aio/actions/runs/31559566600) |
| Cross-check Android AArch64 | Aprobado | [CI 31559566600](https://github.com/alexsndersoto04-source/aio/actions/runs/31559566600) |
| AArch64 con compiladores reales de Android NDK y warnings estrictos | Aprobado | [Termux ARM 31559566583](https://github.com/alexsndersoto04-source/aio/actions/runs/31559566583) |
| Compilación y enlace Android/Bionic ARMv7 | Aprobado | [Termux ARM 31559566583](https://github.com/alexsndersoto04-source/aio/actions/runs/31559566583) |
| ELF32 ARM y paquete Debian `Architecture: arm` | Aprobado | [Termux ARM 31559566583](https://github.com/alexsndersoto04-source/aio/actions/runs/31559566583) |
| Artefacto Termux subido por GitHub | Aprobado | [Termux ARM 31559566583](https://github.com/alexsndersoto04-source/aio/actions/runs/31559566583) |

Los fallos advisory registrados ya fueron corregidos:

- Se aplicó el `rustfmt` estable de CI a los 106 archivos Rust que no cumplían
  el formato. El verificador Phase 34 se hizo compatible con macros formateadas
  en varias líneas y volvió a confirmar 758 nativas únicas y 837 llamadas.
- AArch64 ahora encuentra automáticamente clang, clang++, `llvm-ar` y
  `llvm-ranlib` del Android NDK. Además, la ruta oficial ejecuta un segundo
  check AArch64 real con NDK, `--all-targets`, lockfile y warnings tratados como
  errores antes de construir el paquete ARMv7.
- Las modificaciones posteriores de red, procesos y colecciones introdujeron
  nuevas diferencias de `rustfmt`. El workflow histórico las dejó pasar porque
  ese paso conserva `continue-on-error`. El commit `819f4c9` aplicó exactamente
  el formato señalado por Rust estable; CI 31558196407 y la ejecución final
  31558531732 no tienen la anotación de fallo advisory y aprueban el formato.

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

- [CI 31555988990](https://github.com/alexsndersoto04-source/aio/actions/runs/31555988990): checks normal y `--no-default-features`, todos los tests y
  cross-check AArch64 aprobados. Su paso advisory de formato sí reportó
  diferencias; el formato completo quedó aprobado posteriormente en
  [CI 31558196407](https://github.com/alexsndersoto04-source/aio/actions/runs/31558196407).
- [Termux ARM 31555989007](https://github.com/alexsndersoto04-source/aio/actions/runs/31555989007): check AArch64 NDK estricto, compilación y enlace Android/Bionic
  ARMv7, verificaciones del ELF y paquete, y subida del artefacto aprobados.

No fue necesario levantar servicios externos para estas regresiones: se usaron
sockets loopback y SQLite en memoria. PostgreSQL y MySQL sí compilaron en las
rutas host, AArch64 y ARMv7, pero este bloque no afirma haber conectado contra
servidores reales de esas dos bases.

La ejecución intermedia [CI 31555890863](https://github.com/alexsndersoto04-source/aio/actions/runs/31555890863) detectó un error de ownership en el código de la nueva prueba
concurrente; el cross-check AArch64 sí había pasado, mientras el paso advisory
de formato también reportó diferencias. El commit `c21517c` corrigió el test y
la ejecución funcional posterior aprobó checks y tests; `819f4c9` cerró después
el formato pendiente.
La ejecución intermedia no se cuenta como validación verde.

### Procesos externos con memoria y concurrencia acotadas

El commit `3b046e3` conecta una cuota común de 32 procesos hijo por runtime. La
cuota incluye ejecuciones sincrónicas, `run_timeout`, procesos background y
cada hijo que compone un pipeline. La reserva se realiza antes de `spawn` y un
permiso RAII la devuelve tras `wait`, error de creación o cleanup. Un handle
background terminado continúa ocupando su slot hasta que se recolecta con
`spawn_wait`, evitando acumular zombies y resultados abandonados.

También se aplican estos límites antes o durante la operación:

- 64 KiB para comando y argumentos;
- 8 MiB para la entrada enviada por stdin;
- 4 MiB combinados entre stdout y stderr;
- ocho comandos por pipeline.

La salida ya no se recoge con vectores que crecen hasta agotar la memoria. Dos
lectores concurrentes comparten el presupuesto de 4 MiB; cuando se llena,
continúan drenando y descartan el exceso para que el hijo no quede bloqueado por
un pipe lleno. `run_with_input` escribe stdin en paralelo con esos lectores.
Los pipelines drenan el stderr de todos los procesos intermedios mientras leen
el stdout final. Los procesos background empiezan a drenar inmediatamente, no
solo cuando alguien llama posteriormente a `spawn_wait`.

Las regresiones ejecutan procesos reales y comprueban:

1. stdin, stdout y stderr simultáneos de 128 KiB, mayores que un pipe típico,
   sin interbloqueo;
2. stderr intermedio de 128 KiB en un pipeline, conservando además su stdout;
3. un proceso background que produce 128 KiB termina antes de `spawn_wait`;
4. salida de 4 MiB más un byte rechazada como `ResourceLimit`;
5. rechazo previo de comandos, entradas y pipelines sobredimensionados;
6. saturación de los 32 permisos, liberación y reutilización;
7. cleanup de runtime que mata, espera y elimina un `sleep` background real.

Evidencia externa de este bloque:

- [CI 31557012296](https://github.com/alexsndersoto04-source/aio/actions/runs/31557012296): checks normal y sin features por defecto, todos los tests y
  cross-check AArch64 aprobados. Su formato advisory reportó diferencias que
  quedaron corregidas y aprobadas en
  [CI 31558196407](https://github.com/alexsndersoto04-source/aio/actions/runs/31558196407).
- [Termux ARM 31557012301](https://github.com/alexsndersoto04-source/aio/actions/runs/31557012301): check NDK estricto, build y enlace Android/Bionic ARMv7,
  verificaciones, paquete y artefacto aprobados.

La ejecución intermedia [CI 31556925627](https://github.com/alexsndersoto04-source/aio/actions/runs/31556925627) señaló que un inspector usado solo por tests seguía compilándose
en producción y activaba `dead_code` bajo warnings estrictos. El commit
`3b046e3` lo restringió a tests y la ejecución final aprobó. Esa ejecución
intermedia no se cuenta como validación verde.

### Colecciones persistentes con cuotas y aritmética segura

El commit `5d16dda` conecta una cuota común por runtime para set, deque, cola de
prioridad, mapa ordenado, contador y grafo. Los máximos predeterminados son:

- 256 handles entre las seis familias;
- 65.536 entradas agregadas;
- 4.096 entradas dentro de un único handle;
- 16 MiB de strings y JSON serializado;
- 64 KiB por elemento.

Cada constructor y mutación reserva antes de insertar. Una reserva que falla no
modifica el registro ni consume cuota; retirar elementos, reemplazar un JSON
por otro menor y destruir handles devuelven entradas y bytes. El cleanup del
runtime elimina tanto los registros como la contabilidad residual. El mapa
ordenado mide JSON mediante un escritor contador acotado, sin construir una
segunda copia serializada potencialmente grande solo para conocer su tamaño.

También se cerraron errores aritméticos y algorítmicos: la cola mínima rechaza
`i64::MIN` en vez de desbordar al negarlo, la secuencia estable y los contadores
usan operaciones comprobadas, y el total del contador se calcula en `i128`
antes de convertirlo. Los grafos rechazan pesos negativos porque el camino más
corto usa Dijkstra. La detección de ciclos dirigida ahora es iterativa, por lo
que una cadena profunda controlada por un programa TITAN no depende de la pila
nativa de Rust.

Las regresiones ejecutadas por CI demuestran:

1. saturación compartida de los 256 handles, recuperación tras liberar uno y
   cleanup completo;
2. rechazo y reutilización del límite de 4.096 entradas por handle;
3. llenado exacto de los 16 MiB, rechazo del siguiente elemento y recuperación
   después de retirar uno;
4. atomicidad y recuperación de la cuota agregada de entradas;
5. devolución de bytes al extraer de deque y cola, reemplazar/retirar del mapa,
   y destruir las demás estructuras;
6. rechazo de elementos sobredimensionados, overflows de prioridad, valor y
   total, y pesos negativos;
7. detección de un ciclo al final de una cadena dirigida de 1.500 nodos sin DFS
   recursiva.

Evidencia externa de este bloque:

- [CI 31558531732](https://github.com/alexsndersoto04-source/aio/actions/runs/31558531732): formato completo, checks con características normales y sin
  características por defecto, todos los tests y cross-check AArch64
  aprobados.
- [Termux ARM 31558531725](https://github.com/alexsndersoto04-source/aio/actions/runs/31558531725): check AArch64 con NDK obligatorio, compilación y enlace
  Android/Bionic ARMv7, verificación ELF32/ARM, paquete y artefacto aprobados.

La ejecución funcional original [CI 31557774013](https://github.com/alexsndersoto04-source/aio/actions/runs/31557774013) ya aprobaba compilación y las regresiones de colecciones, pero su
paso de formato era advisory y reportó diferencias. No se presenta ese paso
como verde: `819f4c9` aplicó el formato y la ejecución final anterior lo aprobó.

Estas pruebas no requieren servicios externos ni una instalación física. El
candidato quedó validado y empaquetado por GitHub Actions, pero aún no se ha
ejecutado en el Redmi 9C; esa prueba se agrupará con el siguiente milestone.

### Estado de juego acotado y validado

Los commits `f3e300f` y `e31bb4d` cierran el crecimiento controlable de
`std::game`. Sigue existiendo como máximo un estado por runtime, pero ahora el
título se rechaza antes de copiarlo si supera 64 KiB. El ancho y alto deben ser
positivos y no superar 16.384. `step`, `fps` y `shutdown` ya no crean un estado
vacío por el solo hecho de consultar un runtime que nunca inicializó el motor.
Al apagar, el título se sustituye por un string vacío para liberar también su
capacidad reservada, y se reinician tiempo, frames y FPS.

La colisión AABB conserva los puntos de tamaño cero ya soportados, pero rechaza
tamaños negativos, NaN, infinitos y sumas geométricas que desborden a infinito.
Las regresiones verifican esos bordes, el límite exacto de título y dimensión,
el rechazo previo sin crear estado, la liberación del título y el cleanup del
runtime.

Evidencia externa:

- [CI 31558531732](https://github.com/alexsndersoto04-source/aio/actions/runs/31558531732): formato, checks, 281 tests de stdlib —incluidas las nuevas
  regresiones de juego—, el resto del workspace, no-default-features y AArch64
  aprobados.
- [Termux ARM 31558531725](https://github.com/alexsndersoto04-source/aio/actions/runs/31558531725): AArch64 NDK estricto, build Android/Bionic ARMv7, ELF, paquete y
  artefacto aprobados.

La ejecución intermedia [CI 31558364517](https://github.com/alexsndersoto04-source/aio/actions/runs/31558364517) encontró que una prueba antigua todavía esperaba que `shutdown`
creara implícitamente un estado en un segundo runtime. Las 16 pruebas directas
de juego pasaron, pero esa regresión de integración falló; `e31bb4d` actualizó
la expectativa y el conteo de cleanup, y la ejecución final aprobó. El run
intermedio no se cuenta como verde.

### Señales, watchers, progreso, routers y ventanas acotados

El commit `7c25b23` cierra el crecimiento sin límite de varios registros nativos
ligeros y de sus colas. Todos los máximos se aplican por runtime antes de crear
o insertar el recurso:

- señales: los ocho tipos admitidos siguen teniendo un único dispatcher real
  por señal de proceso, pero el contador pendiente ahora se satura en
  `usize::MAX` en vez de volver a cero por overflow;
- filesystem watchers: 32 watchers activos entre operaciones persistentes y
  `watch_once`, paths de 16 KiB, timeout máximo de 24 horas, 1.024 eventos
  pendientes y descripción de evento de 64 KiB;
- progreso: 64 barras o spinners y mensajes de 4 KiB;
- routers: 64 handles, 4.096 rutas y 1 MiB por router, además de 8 MiB entre
  todos los routers del runtime; patrón de 8 KiB y valor/path de 64 KiB;
- ventanas virtuales: 64 handles, 1.024 eventos por ventana, título de 64 KiB y
  dimensiones entre 1 y 16.384;
- ventanas reales: 16 handles, 1.024 eventos por ventana, título de 4 KiB,
  dimensión máxima de 4.096 y 16.777.216 píxeles agregados por runtime.

Los watchers usan ahora un canal sincronizado de capacidad fija. Cuando el
backend del sistema operativo produce eventos más rápido que el programa, se
descarta el exceso en vez de hacer crecer memoria. `next_event` clona solamente
la referencia a su receptor y libera el lock del registro antes de esperar, así
que una espera larga no impide cerrar, limpiar ni crear otros watchers. El
permiso RAII se devuelve al fallar la creación, cerrar el handle, terminar
`watch_once` o limpiar el runtime.

Las ventanas reales conservan el requisito de vivir en el hilo del sistema
operativo que las creó, pero su permiso y su presupuesto de píxeles son globales
al runtime: abrir ventanas desde tareas en hilos distintos no multiplica la
cuota. Las colas virtuales y reales dejan de crecer al alcanzar 1.024 eventos;
un cierre real conserva prioridad para que `CloseRequested` no se pierda. Cerrar
una ventana virtual elimina el handle inmediatamente y recupera su slot.

Todos los generadores de handles modificados usan incremento comprobado. El
puente de la VM propaga los errores de creación de progreso, router y ventana,
y convierte handles y dimensiones con `try_from`; enteros negativos o mayores
que `u32` ya no se transforman silenciosamente mediante `as`.

Las regresiones ejecutadas por CI demuestran:

1. saturación y recuperación de handles de progreso, routers, ventanas
   virtuales, ventanas reales y watchers;
2. rechazo de mensajes, títulos, paths, timeouts y dimensiones
   sobredimensionados, y truncado seguro de descripciones al límite exacto;
3. límite de 4.096 rutas, 1 MiB por router y 8 MiB agregado, sin cargar bytes por
   una inserción rechazada;
4. llenado y drenaje de una cola virtual de 1.024 eventos, y conservación del
   cierre prioritario en la cola real;
5. saturación del contador de señales sin wrap;
6. rechazo desde la VM de anchos y handles negativos o fuera de rango;
7. cleanup que libera todos los slots del runtime usado por cada prueba.

Evidencia externa de este bloque:

- [CI 31559566600](https://github.com/alexsndersoto04-source/aio/actions/runs/31559566600): `cargo fmt --check`, checks con características normales y sin
  características por defecto, todos los tests del workspace y cross-check
  AArch64 aprobados.
- [Termux ARM 31559566583](https://github.com/alexsndersoto04-source/aio/actions/runs/31559566583): check AArch64 con Android NDK y warnings estrictos, compilación y
  enlace Android/Bionic ARMv7, verificación ELF32/ARM, paquete Debian y subida
  del artefacto aprobados.
- `verify_phase34.py`: 758 nativas únicas, 837 llamadas verificadas y 110 brazos
  Phase 34 conectados.

No hizo falta una validación física para probar estas cuotas y rechazos. La
ventana real continúa respaldada por la prueba física X11 ya registrada, pero
este commit no afirma haber repetido esa ceremonia en el Redmi. El paquete
producido se reserva para una validación física agrupada del próximo milestone.

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

- Las cuotas de tareas, canales, red, bases de datos, procesos externos,
  colecciones y los recursos ligeros descritos arriba ya están conectadas. Esto
  no cierra toda la auditoría de crecimiento: imagen, KV, Redis, ONNX, PDF,
  tokenizadores y servidores conservan registros duraderos que deben revisarse
  individualmente.
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

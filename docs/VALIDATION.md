# Registro visible de validación

Este documento registra evidencia comprobable. Un cambio no se marca como
validado solamente porque el código parezca correcto: debe tener pruebas y una
ejecución externa en GitHub Actions. Las pruebas físicas en el Redmi se anotan
por separado porque CI no puede sustituir un teléfono real.

## Estado actual

- Rama de trabajo: `arena/019ff232-aio`
- Commit funcional validado: `1875e3a2b0d7386c5027f4e5f46909f5800e97e9`
- Alcance: cierre de la fase 2 de sandbox y seguridad; capacidades, aislamiento,
  cuotas y cleanup por runtime para tareas, canales, red, bases de datos,
  procesos, colecciones y recursos stdlib; servidor HTTP/WebSocket acotado,
  KV/sled, Redis, ONNX y generación PDF real; checks Android ARM de 32 y 64 bits
- Fecha: 2026-08-12

### Evidencia automatizada más reciente

| Comprobación | Resultado | Evidencia |
|---|---:|---|
| Formato completo, `cargo fmt --check` | Aprobado, sin fallo oculto | [CI 31633367279](https://github.com/alexsndersoto04-source/aio/actions/runs/31633367279) |
| `cargo check` con características normales | Aprobado | [CI 31633367279](https://github.com/alexsndersoto04-source/aio/actions/runs/31633367279) |
| Tests del workspace, todos los targets | Aprobado | [CI 31633367279](https://github.com/alexsndersoto04-source/aio/actions/runs/31633367279) |
| `cargo check --no-default-features` | Aprobado | [CI 31633367279](https://github.com/alexsndersoto04-source/aio/actions/runs/31633367279) |
| Cross-check Android AArch64 | Aprobado | [CI 31633367279](https://github.com/alexsndersoto04-source/aio/actions/runs/31633367279) |
| AArch64 con compiladores reales de Android NDK y warnings estrictos | Aprobado | [Termux ARM 31633367369](https://github.com/alexsndersoto04-source/aio/actions/runs/31633367369) |
| Compilación y enlace Android/Bionic ARMv7 | Aprobado | [Termux ARM 31633367369](https://github.com/alexsndersoto04-source/aio/actions/runs/31633367369) |
| ELF32 ARM y paquete Debian `Architecture: arm` | Aprobado | [Termux ARM 31633367369](https://github.com/alexsndersoto04-source/aio/actions/runs/31633367369) |
| Artefacto `zett-termux-arm-81`, 26.718.677 bytes | Aprobado | [Termux ARM 31633367369](https://github.com/alexsndersoto04-source/aio/actions/runs/31633367369) |

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

### Imágenes decodificadas y transformaciones con memoria acotada

Los commits `82ea1ab` y `57b2b84` endurecen el registro de imágenes y todas las
operaciones que pueden decodificar, copiar o producir píxeles. Cada runtime
admite como máximo:

- 64 handles de imagen;
- 8.192 píxeles de ancho o alto y 64 MiB decodificados por imagen;
- 128 MiB decodificados entre todas sus imágenes persistentes;
- 32 MiB de entrada codificada y 64 MiB de salida codificada;
- cuatro operaciones simultáneas y 128 MiB de presupuesto transitorio. Una
  operación que reserva el máximo de 64 MiB permite, por tanto, como máximo dos
  transformaciones pesadas simultáneas.

`load` y `load_bytes` configuran los límites del decodificador de `image` antes
de leer los píxeles, y vuelven a medir el buffer decodificado exacto antes de
insertarlo. `encode` ya no escribe en un `Vec` libre: usa un writer con `Write +
Seek` que devuelve error antes de cruzar 64 MiB. Los paths se rechazan por
encima de 16 KiB.

El registro guarda cada `DynamicImage` detrás de `Arc`. Una transformación puede
liberar el lock global y trabajar sin clonar todo el origen. El resultado sigue
cubierto por un permiso RAII transitorio hasta que la inserción termina o falla.
Cerrar o limpiar devuelve inmediatamente handles y bytes persistentes; cualquier
error devuelve también el permiso transitorio.

Se validan dimensiones positivas antes de reservar. Los recortes comprueban
sumas de coordenadas y bordes antes de llamar al codec, y `blur` rechaza NaN,
infinito, negativos y sigma mayor que 100. En el puente VM, `from_rgba` dejó de
convertir enteros mediante `as`, y brillo dejó de convertir silenciosamente un
entero fuera de `i32` en cero.

Las regresiones ejecutadas externamente demuestran:

1. saturación de 64 handles, rechazo del siguiente, liberación de uno,
   reutilización y cleanup completo;
2. rechazo de dimensiones, recortes y sigma inválidos;
3. rechazo en el borde de 128 MiB persistentes sin insertar el resultado;
4. saturación independiente del número de operaciones y del presupuesto
   transitorio, con recuperación RAII;
5. writer codificado que conserva exactamente el límite y falla antes del byte
   siguiente;
6. carga, resize, transformaciones y encode PNG reales conservando las pruebas
   funcionales existentes.

Evidencia externa:

- [CI 31560352682](https://github.com/alexsndersoto04-source/aio/actions/runs/31560352682): formato con Rust 1.97, checks normal y sin características por
  defecto, todos los tests del workspace y AArch64 aprobados.
- [Termux ARM 31560352721](https://github.com/alexsndersoto04-source/aio/actions/runs/31560352721): check NDK estricto, compilación y enlace Android/Bionic ARMv7,
  verificación ELF32/ARM, paquete y artefacto aprobados.

La ejecución intermedia [CI 31560187616](https://github.com/alexsndersoto04-source/aio/actions/runs/31560187616) aprobó compilación, tests y AArch64, pero el paso advisory de formato
detectó cambios exigidos por el `rustfmt` estable recién actualizado a Rust
1.97. El commit `57b2b84` aplicó exactamente ese diff, incluidos los archivos
del bloque ligero anterior, y la ejecución final aprobó formato. El run
intermedio no se presenta como completamente verde.

No se necesita una prueba física para estos límites. Los codecs y las rutas
ARMv7 sí quedaron compilados dentro del binario Android/Bionic; este bloque no
afirma que el Redmi haya decodificado manualmente cada formato.

### Tokenizadores y encodings con entradas y salidas acotadas

El commit `14268e2` limita tanto los modelos HuggingFace persistentes como las
operaciones de tokenización. Cada runtime admite:

- 16 handles de tokenizer;
- 16 MiB de JSON por modelo y 64 MiB de fuentes JSON agregadas;
- vocabularios de hasta 262.144 entradas y tokens de vocabulario de hasta 256
  bytes;
- textos individuales de 64 KiB;
- batches de 64 textos y 256 KiB agregados;
- 131.072 tokens por encoding, 262.144 por batch, 8 MiB de strings de tokens por
  encoding y 16 MiB por batch;
- padding entre 1 y 65.536 tokens;
- 131.072 ids por decode y 1 MiB de texto decodificado;
- dos operaciones pesadas simultáneas.

`load` ya no usa una lectura de archivo que crece hasta EOF. Lee como máximo el
límite más un byte, rechaza el exceso y valida UTF-8 antes de parsear. Tanto
`load` como `from_json` hacen una comprobación previa de capacidad y repiten la
comprobación bajo el lock al insertar, cerrando carreras entre tareas. Los IDs
usan incremento comprobado.

El tokenizer persistente se guarda en `Arc`. Encode, batch y decode clonan solo
esa referencia y liberan el registro global antes del trabajo costoso, por lo
que una tokenización lenta no bloquea close ni el cleanup de otros handles. Un
permiso RAII limita la concurrencia y se devuelve también ante errores.

Antes de copiar un encoding hacia la VM se miden cantidad de tokens y bytes de
sus strings. `encode_padded` rechaza longitud cero —que antes podía llegar a
`max_length - 1`— y cualquier padding mayor al máximo. Los batches validan
cantidad y bytes antes de activar el paralelismo interno del tokenizer. Decode,
`token_to_id` e `id_to_token` tienen límites de entrada y salida propios.

Las regresiones verificadas por CI usan un tokenizer WordLevel real y prueban:

1. creación de 16 modelos, rechazo del siguiente, recuperación de un slot y
   cleanup completo;
2. encode con padding real y longitud exacta;
3. rechazo de longitud de padding cero, texto y batch sobredimensionados;
4. límites agregado de fuentes y vocabulario;
5. saturación y recuperación de los dos permisos de operación;
6. conservación de las pruebas opcionales para tokenizadores HuggingFace
   externos.

Evidencia externa:

- [CI 31560961245](https://github.com/alexsndersoto04-source/aio/actions/runs/31560961245): formato, checks normal y sin características por defecto, todos los
  tests del workspace y AArch64 aprobados.
- [Termux ARM 31560961244](https://github.com/alexsndersoto04-source/aio/actions/runs/31560961244): NDK estricto, compilación y enlace Android/Bionic ARMv7,
  verificación ELF32/ARM, paquete y artefacto aprobados.

La contabilidad persistente usa los bytes del JSON, no intenta fingir una medida
exacta de las estructuras internas del crate `tokenizers`. Ese posible factor
de expansión queda además acotado por vocabulario, longitud de token, handles y
concurrencia. No se ejecutó un modelo HuggingFace externo en este run; la prueba
funcional integrada usa un modelo WordLevel pequeño y real, sin simulación.

No hace falta validación física individual para este bloque. El código sí quedó
compilado y enlazado dentro del binario Android/Bionic ARMv7.

### Modelos ONNX e inferencia con cuotas por runtime

Los commits `788720b` y `f0b8f61` sustituyen el registro ONNX sin límites por
un registro aislado y acotado. Cada runtime admite como máximo:

- cuatro modelos;
- 256 MiB por archivo principal y 512 MiB agregados de fuentes persistentes;
- paths de 16 KiB;
- 65.536 nodos, ocho entradas y 32 salidas por modelo;
- rango ocho, 4.194.304 por dimensión y 4.194.304 elementos por tensor;
- 64 MiB agregados de entrada por inferencia;
- 4.194.304 elementos y 64 MiB agregados de salida de ancho fijo;
- una carga o inferencia pesada simultánea.

La carga adquiere primero un permiso RAII, abre una sola vez el archivo, exige
que sea regular y mide ese mismo descriptor. Tract recibe un `Read::take`
limitado al tamaño inspeccionado mediante `model_for_read`; ya no se vuelve a
abrir el path con `model_for_path`. La capacidad se comprueba antes de parsear y
otra vez bajo el lock al insertar. Los IDs usan incremento comprobado.

El modelo persistente queda en `Arc<Mutex<_>>`. El lock global solo sirve para
buscar o modificar handles, no permanece tomado durante optimización ni
inferencia. `close` y cleanup devuelven el slot y los bytes de fuente, y los
handles continúan siendo privados del runtime que los creó.

Antes de optimizar se limitan entradas, salidas y nodos. Antes de insertar se
repiten esos límites sobre el grafo ejecutable y se inspeccionan formas, tipos y
presupuestos estáticos. Las rutas `run_f32`, `run_ids`, BERT de dos y tres
entradas y pooling usan productos y conversiones comprobados. Después de
`tract::run`, todas las salidas se validan por cantidad, rango, dimensión,
elementos y bytes antes de copiar la primera hacia la VM. Tensores String/Blob,
cuyo contenido no puede acotarse por `size_of`, se rechazan.

El puente VM rechaza arrays de forma antes de convertir más de ocho elementos,
valida longitudes y bytes antes de duplicar arrays `Value` en buffers f32/i64,
y rechaza NaN, infinito y valores f64 que no caben en f32.

Las regresiones verificadas por CI prueban:

1. cuatro modelos ejecutables de identidad hechos con tract real, rechazo del
   quinto, inferencia f32 real, `close`, reemplazo, aislamiento y cleanup;
2. rangos, dimensiones, productos, bytes de entrada, longitudes y salidas
   adversariales;
3. saturación y recuperación del permiso de operación;
4. rechazo de directorios, de un archivo sparse de 256 MiB más un byte y de un
   protobuf ONNX malformado, sin filtrar el permiso ni insertar un handle;
5. conservación del test opt-in que permite cargar un `.onnx` externo válido.

Evidencia externa final para `f0b8f61882b57305906d9c97c2aceb080344765c`:

- [CI 31561938930](https://github.com/alexsndersoto04-source/aio/actions/runs/31561938930): `cargo fmt --check`, checks con características normales y sin
  características por defecto, todos los tests del workspace y cross-check
  AArch64 aprobaron.
- [Termux ARM 31561938779](https://github.com/alexsndersoto04-source/aio/actions/runs/31561938779): check NDK estricto, compilación y enlace Android/Bionic ARMv7,
  verificación ELF32/ARM, paquete Debian y artefacto
  `zett-termux-arm-61` aprobaron.
- `verify_phase34.py`: 758 nativas únicas, 837 llamadas verificadas y 110 brazos
  Phase 34 conectados.

La primera ejecución funcional, [CI 31561809762](https://github.com/alexsndersoto04-source/aio/actions/runs/31561809762), detectó que `TDim::to_i64` devuelve `Result` y no `Option` en
tract 0.21.14; por eso no se cuenta como verde. `f0b8f61` corrigió esa
conversión, añadió el límite de nodos y aplicó exactamente el formato exigido
por Rust 1.97. El Termux correspondiente a aquella revisión también falló y no
se usa como evidencia.

Límites declarados, sin fingir más cobertura: la contabilidad persistente mide
bytes del archivo ONNX principal, no el heap exacto del plan optimizado. Una
salida dinámica solo puede medirse después de que tract la produjo, aunque se
rechaza antes de expandirla a valores TITAN. La ruta acotada no resuelve pesos
ONNX guardados en archivos externos, para no abrir datos laterales fuera de la
cuota. Tampoco hay todavía cancelación segura de una operación nativa tract que
se quede computando; la cuota impide otra operación ONNX simultánea en ese
runtime, pero no es un timeout.

CI ejecutó inferencia real sobre un grafo tract construido en memoria y ejercitó
el parser con entrada malformada; no se suministró un `.onnx` válido de terceros
automáticamente. El test opt-in sigue disponible para eso. No hace falta una
prueba física individual: el módulo completo quedó compilado y enlazado en el
ELF Android/Bionic ARMv7, y el paquete se reserva para el siguiente milestone
físico agrupado.

### KV/sled con handles, memoria y crecimiento lógico acotados

Los commits `acdeeac`, `dd9eacb`, `4033708` y `b807666` reemplazan el
registro sled global y sin cuotas por estados de base compartidos, aislados por
runtime y con contabilidad persistente. Cada runtime admite como máximo:

- ocho bases de datos y 32 handles de árboles;
- 256 MiB y 524.288 entradas lógicas entre todas sus bases;
- cuatro operaciones KV simultáneas.

Cada base admite:

- un path de 16 KiB y cache sled configurado a 16 MiB;
- 64 árboles nombrados, con nombres de hasta 1 KiB;
- 128 MiB y 262.144 entradas lógicas entre el árbol principal y los nombrados;
- claves de 64 KiB y valores de 8 MiB;
- listados de hasta 65.536 claves UTF-8 y 8 MiB de strings agregados.

`open` reserva el slot antes de tocar el filesystem, abre sled con cache
explícita y recorre de forma incremental el árbol principal y todos los árboles
existentes. Rechaza una base preexistente si cualquiera de sus claves, valores,
árboles, bytes o entradas ya excede los límites. Solo después reserva la cuota
agregada e inserta el handle mediante un ID con incremento comprobado. Una
apertura fallida devuelve tanto la reserva del handle como la operación.

Cada base usa `Arc<DatabaseState>` y un mutex propio para serializar mutaciones
y su contabilidad. El registro global se libera antes de `get`, iteraciones,
flush, escrituras o compare-and-swap; una base lenta ya no bloquea el I/O de
otras bases o runtimes. Reservas separadas cierran la carrera entre aperturas
concurrentes y el último slot disponible.

Insert, overwrite, remove, clear y compare-and-swap actualizan exactamente los
bytes `clave + valor` y las entradas del árbol, la base y el runtime. Un
crecimiento se reserva antes de escribir y se revierte si sled falla o el CAS
pierde. Una reducción se descuenta solo después del éxito. La misma ruta cubre
árboles nombrados. `keys` ya no ignora errores de iteración de sled y valida el
presupuesto antes de construir el array de salida.

`close` retira primero la base y todos sus handles de árboles, marca el estado
cerrado, espera cualquier mutación ya iniciada y hace flush. Un árbol ya no
queda vivo durante todo el proceso después de cerrar su base padre. Cleanup
realiza la misma invalidación por runtime y un flush de mejor esfuerzo. El
puente VM también dejó de convertir sin comprobación el contador de bytes de
`flush` a `i64`.

Las regresiones ejecutadas por CI verifican:

1. persistencia real después de flush, cierre y reapertura de sled;
2. insert, overwrite, get, remove y CAS reales;
3. aislamiento de árboles y su invalidación al cerrar la base padre;
4. saturación de ocho bases y 32 árboles, rechazo del siguiente, reemplazo de
   una base cerrada y cleanup;
5. saturación y recuperación de cuatro permisos de operación;
6. contabilidad exacta al crecer, reducir y borrar datos del árbol principal y
   uno nombrado;
7. rechazo durante `open` de una base sled real que ya contiene un valor de más
   de 8 MiB.

Evidencia externa final para `b8076664d85357fa0127f2204429caebe130fe6c`:

- [CI 31563194128](https://github.com/alexsndersoto04-source/aio/actions/runs/31563194128): formato sin anotaciones ocultas de fallo, checks normal y sin
  características por defecto, todos los tests del workspace y AArch64
  aprobaron.
- [Termux ARM 31563194228](https://github.com/alexsndersoto04-source/aio/actions/runs/31563194228): check NDK estricto, compilación y enlace Android/Bionic ARMv7,
  verificación ELF32/ARM, paquete y artefacto `zett-termux-arm-66` aprobaron.
- `verify_phase34.py`: 758 nativas únicas, 837 llamadas verificadas y 110 brazos
  Phase 34 conectados.

Las ejecuciones intermedias no se ocultan: [CI 31562783972](https://github.com/alexsndersoto04-source/aio/actions/runs/31562783972) encontró cuatro argumentos de test omitidos y el diff inicial de
rustfmt. Más tarde [CI 31563012653](https://github.com/alexsndersoto04-source/aio/actions/runs/31563012653) aprobó compilación y tests, pero una anotación reveló que el paso advisory de
formato aún pedía dos cambios; por eso tampoco se usa como validación final. El
commit `b807666` aplicó ese diff y el run final solo conserva las advertencias de
Node.js 20 de actions de terceros.

Límites declarados: la cuota persistente mide bytes lógicos de claves y valores,
no el tamaño físico exacto del log, índices o archivos temporales de compactación
de sled. Por ello protege la cantidad de datos aceptada y la cache, pero no
promete que el directorio ocupe exactamente 128 MiB. Abrir una base existente
requiere recorrerla hasta medirla o encontrar el primer exceso, y una operación
sled bloqueada en el filesystem todavía no tiene timeout seguro.

No hace falta validación física individual. Las rutas sled y sus regresiones
corrieron en Linux, y el módulo completo quedó compilado y enlazado dentro del
ELF Android/Bionic ARMv7. El paquete se reserva para el siguiente milestone
físico agrupado.

### Redis/RESP2 con red, respuestas y ciclo de vida acotados

Los commits `e37f146`, `1ee1f57`, `ffd323a`, `f024ac8` y `1af829b`
sustituyen la conexión bloqueante del crate `redis` bajo un mutex global por
un cliente RESP2 acotado sobre sockets TCP reales. El crate `redis 0.27.6`
continúa parseando y decodificando de forma segura la URL, usuario, contraseña
y base; TITAN realiza directamente el intercambio RESP2 para poder rechazar una
longitud hostil **antes** de reservar el cuerpo anunciado.

Cada runtime admite como máximo:

- ocho conexiones Redis publicadas;
- cuatro operaciones Redis simultáneas;
- dos resoluciones DNS activas, con un techo adicional de 16 en todo el proceso.

Los límites por petición y respuesta son:

- URL de 16 KiB, claves/campos/patrones de 64 KiB y valores de 8 MiB;
- comando `raw` de 64 KiB, 32 argumentos y petición RESP completa de 9 MiB;
- 8 MiB de payload y 9 MiB de bytes de protocolo por respuesta;
- 65.536 nodos RESP, profundidad 16 y líneas de cabecera de 64 KiB;
- 65.536 elementos y 8 MiB acumulados al devolver colecciones;
- 256 páginas `SCAN`, solicitadas en lotes indicativos de 256.

Una operación normal tiene un deadline total de cinco segundos; cada intento de
conexión usa como máximo tres segundos sin superar ese deadline. Escritura,
lectura, autenticación, selección de base y las páginas sucesivas de `SCAN`
comparten el presupuesto total correspondiente. Como la biblioteca estándar no
ofrece timeout para `ToSocketAddrs`, los nombres se resuelven en workers
separados que conservan su permiso hasta terminar. Así una resolución del SO
atascada no retiene a la VM ni permite acumular más de dos hilos por runtime o
16 globales.

El registro global solo se toma para reservar, publicar, buscar o retirar un
handle. El socket vive en un `Arc<Mutex<_>>` por conexión; nunca se hace I/O
bajo el registro global. Una segunda llamada simultánea al mismo handle recibe
un error `busy`, mientras otras conexiones y runtimes continúan. Los IDs usan
incremento comprobado y una reserva previa evita sobrepasar el último slot. Si
cleanup ocurre durante `connect`, invalida también la reserva para impedir que
la conexión aparezca después de destruir su runtime.

Toda conexión configura timeouts de lectura y escritura. Un timeout, error de
transporte, respuesta incompleta, prefijo RESP3, cabecera inválida, profundidad,
elementos o bytes excesivos marca el transporte como roto, lo retira del
registro y cierra el socket: nunca se reutiliza una conexión desincronizada. Un
error normal `-ERR` del servidor sí queda tipado y la conexión sigue utilizable
porque esa respuesta se consumió completa.

`connect` ejecuta de verdad `AUTH` —con usuario cuando corresponde— y `SELECT`
antes de publicar el handle. Una URL que solicita RESP3 se rechaza. Esta build
no habilita TLS en `redis`, por lo que `rediss://` y sockets Unix se rechazan en
vez de fingir soporte; la superficie validada es `redis://` RESP2 sobre TCP.

La API histórica `keys` ya no envía `KEYS`: recorre páginas `SCAN`, valida
cursor, elementos y bytes, y elimina duplicados permitidos por SCAN. `LRANGE`
consulta primero `LLEN`, normaliza índices negativos y envía un intervalo fijo
de como máximo 65.536 entradas, por lo que una mutación concurrente no puede
agrandar la respuesta solicitada. `GET`, `HGETALL` y todas las demás respuestas
pasan por el mismo decoder acotado.

`raw` conserva la separación histórica por espacios, pero ya no acepta cualquier
comando. Solo permite una lista explícita de comandos de una respuesta. Rechaza
suscripciones, variantes bloqueantes, `MULTI`, cambios de protocolo/sesión,
`CLIENT`, scripts, administración y comandos desconocidos que podrían bloquear
o desincronizar el transporte. Su salida escapada también se detiene en 8 MiB.

Las 18 regresiones del módulo incluyen 17 casos deterministas con servidores
TCP loopback reales y un caso de interoperabilidad externo opt-in. Verifican:

1. codificación y respuestas RESP2 de wrappers de strings, listas y hashes;
2. `AUTH`, `SELECT`, errores normales y reutilización posterior del socket;
3. `SCAN` multipágina sin `KEYS`, deduplicación y `LRANGE` fijo;
4. ejecución de un `raw` permitido y rechazo de comandos bloqueantes,
   stateful, scripts o desconocidos;
5. DNS real de `localhost` y devolución de su permiso;
6. rechazo previo a la asignación de un bulk mayor de 8 MiB, un agregado con
   demasiados nodos, profundidad excesiva, CRLF malformado y RESP3;
7. timeout de un peer que no responde y cierre obligatorio del handle;
8. que un peer lento no bloquea el registro ni otra conexión rápida;
9. aislamiento entre runtimes, cleanup del socket, cuotas recuperables y la
   carrera cleanup/reserva durante `connect`.

Estos peers de prueba no sustituyen la implementación: son servidores TCP que
leen los comandos RESP enviados por el cliente de producción y contestan bytes
reales, incluidos casos adversariales difíciles de pedir a un Redis normal. El
test opt-in `TITAN_REDIS_TEST_URL` se conserva, pero este run no recibió una
instancia Redis externa; por tanto no se afirma una prueba contra una versión
concreta del servidor Redis.

Evidencia externa final para `1af829becaf931a90d3756967a1601f3f6f6b9b6`:

- [CI 31565041845](https://github.com/alexsndersoto04-source/aio/actions/runs/31565041845): formato, check normal, las 18 regresiones Redis dentro de todos
  los tests del workspace, no-default-features y AArch64 aprobaron. Las
  annotations finales solo contienen la advertencia externa de Node.js 20.
- [Termux ARM 31565041835](https://github.com/alexsndersoto04-source/aio/actions/runs/31565041835): check NDK estricto, compilación y enlace
  Android/Bionic ARMv7, verificación ELF32/ARM, paquete y upload aprobaron;
  produjo `zett-termux-arm-72` de 25.286.749 bytes.
- `verify_phase34.py`: 758 nativas únicas, 837 llamadas verificadas y 110 brazos
  Phase 34 conectados.

Los fallos intermedios permanecen visibles. [CI 31564268476](https://github.com/alexsndersoto04-source/aio/actions/runs/31564268476) y [Termux ARM 31564268480](https://github.com/alexsndersoto04-source/aio/actions/runs/31564268480) detectaron un import `BufRead` no usado y el primer diff de rustfmt;
no cuentan como verdes. [CI 31564766319](https://github.com/alexsndersoto04-source/aio/actions/runs/31564766319) y [CI 31564905958](https://github.com/alexsndersoto04-source/aio/actions/runs/31564905958) aprobaron compilación y tests, pero las annotations revelaron dos y una
diferencias de formato, respectivamente. `1af829b` aplicó el último resultado
de Rust 1.97 y cerró el fallo advisory.

Límites declarados: RESP2/TCP no ofrece confidencialidad; para un servidor
remoto debe usarse una red protegida o túnel externo hasta que la build integre
un transporte `rediss://` acotado. El deadline sí devuelve el control a la VM
durante DNS, pero la API del SO no permite cancelar el worker ya iniciado; las
cuotas de dos y 16 limitan ese residuo. `SCAN` puede ser no atómico frente a
mutaciones del servidor, aunque la memoria, rondas y duración del cliente sí
quedan acotadas.

No hace falta validación física individual para este bloque. El cliente quedó
compilado y enlazado dentro del ELF Android/Bionic ARMv7; una instalación en el
Redmi se agrupará con el siguiente milestone físico.

### Servidor HTTP/WebSocket real, acotado y aislado

Los commits `a323391` y `4b71994` sustituyen `tiny_http` por un backend propio
sobre `std::net`. La razón no fue cosmética: `tiny_http 0.12` acumulaba líneas y
cabeceras antes de entregar la petición y no exponía el socket aceptado para
fijar deadlines a tiempo. Aplicar límites después de `recv` habría sido una
protección tardía. La dependencia y sus paquetes exclusivos también fueron
retirados de `Cargo.toml` y `Cargo.lock`; el producto continúa siendo un solo
binario.

Antes de publicar un handle, el parser HTTP/1.1 limita:

- 64 KiB de cabecera completa, 128 cabeceras, target de 16 KiB, nombre de 256
  bytes y valor de 8 KiB;
- exactamente un `Host` no vacío en HTTP/1.1;
- un único `Content-Length` decimal o `Transfer-Encoding: chunked`, nunca ambos;
- cuerpos fijos o chunked de hasta 8 MiB, líneas chunk de 1 KiB y 32 trailers;
- deadline total de 5 segundos para cabeceras y, en producción, 30 segundos para
  cada lectura o escritura completa. El timeout solicitado a `accept` también
  está limitado a 30 segundos.

Las cuotas por runtime son ocho listeners, 256 peticiones pendientes, 64
WebSockets, 16 MiB de metadatos de peticiones y ocho operaciones concurrentes.
Las reservas se realizan bajo el mismo registro antes de aceptar o publicar el
recurso; el asignador de IDs usa suma comprobada. Listeners, peticiones y
WebSockets llevan ownership de runtime y locks propios. El registro global solo
se usa para localizar o retirar un `Arc`: nunca permanece tomado durante red,
espera, parser ni cierre. Cleanup retira primero los recursos y después usa
clones de los sockets para interrumpir I/O bloqueado sin afectar otra VM; una
reserva concurrente no puede revivir un runtime ya limpiado.

Las respuestas validan status, content type y cabeceras antes de consumir la
petición. Se rechazan CR/LF, nombres inválidos, framing reservado y cuerpos de
más de 8 MiB. Cada conexión HTTP procesa una petición y responde con
`Connection: close`, decisión deliberada que elimina ambigüedad de pipelining en
esta superficie de bajo nivel.

El upgrade WebSocket valida GET/HTTP/1.1, tokens `Connection`/`Upgrade`, versión
13 y una clave base64 de 16 bytes. Conserva bytes de frames que llegaron junto
con el handshake. Los frames se procesan con el codec RFC 6455 ya integrado:
mask obligatorio del cliente, control frames, fragmentación, ping/pong, UTF-8,
códigos de cierre válidos y máximo configurable con techo de 4 MiB. Enviar,
recibir y cerrar operan sobre locks por conexión, no sobre el registro global.

Ocho regresiones loopback reales comprueban HTTP con metadatos/cuerpo/respuesta,
chunked y trailers, upgrade y frame WebSocket enmascarado, framing ambiguo,
límites de cabeceras/cuerpo/respuesta, deadlines totales, ausencia de lock global
mientras un cliente se estanca, ownership entre dos runtimes, cleanup,
reservas en vuelo y recuperación de cuotas. En el primer run del backend,
[CI 31632116052](https://github.com/alexsndersoto04-source/aio/actions/runs/31632116052), las ocho regresiones de servidor, formato, compilación y AArch64
aprobaron; el run quedó rojo únicamente porque una prueba nueva de metadatos de
capacidades usó dos nombres WebSocket inexistentes. El nombre se corrigió sin
ocultar ese fallo. [CI 31632346274](https://github.com/alexsndersoto04-source/aio/actions/runs/31632346274) aprobó formato, check, todos los tests, no-default-features y
AArch64 con el backend final y sin `tiny_http`. [Termux ARM 31632346324](https://github.com/alexsndersoto04-source/aio/actions/runs/31632346324) compiló y enlazó el mismo código para Android/Bionic ARMv7.

Límites declarados: este listener es HTTP plano; TLS debe terminar en el backend
TLS del producto o en un reverse proxy. No ofrece keep-alive, HTTP/2,
compresión WebSocket ni negociación de subprotocolos en esta API. No se afirma
resistencia a una saturación del backlog del sistema operativo anterior a
`accept`; sí se acotan los recursos una vez aceptados. No hace falta validación
física aislada para este bloque: protocolo, sockets y frames se ejercitaron en
loopback real, y las dos arquitecturas Android compilaron dentro del binario. Se
reserva la instalación en el Redmi para el candidato agrupado del milestone.

### Barrido transversal final de sandbox y handles

El commit `1875e3a2b0d7386c5027f4e5f46909f5800e97e9` cerró el barrido posterior al
servidor:

- las 23 nativas `std::server::*` exigen `Network`, incluidas consultas de
  metadatos y cierre; las 21 de Redis también;
- las 17 operaciones KV/sled y las cuatro de watchers exigen `Filesystem`, no
  solo `open`; las siete barras de progreso exigen `UserInterface` porque
  escriben en el terminal;
- una regresión de la VM demuestra que handles enteros forjados no permiten
  saltarse esas capacidades: el rechazo ocurre antes de buscar el recurso;
- IDs de runtimes, tareas, canales, sockets, TLS, WebSockets, routers, bases de
  datos, pools, request IDs, procesos, colecciones y temporales PDF dejaron de
  usar incremento con wrap. Al agotarse fallan cerrados; los contadores de
  estadísticas saturan en vez de volver a cero;
- handles de audio y GUI ya no vuelven a 1 tras `shutdown`, por lo que un handle
  viejo no puede convertirse en alias de un objeto creado después del reinicio;
- PID, señal, estado de salida y cantidad de `most_common` usan conversiones
  comprobadas en la VM en vez de casts que envolvían valores negativos o
  demasiado grandes;
- procesos en background usan `Arc<Mutex<Option<_>>>` por recurso. `poll`,
  `kill`, PID, espera y cleanup sueltan el registro global antes de tocar el
  proceso del sistema operativo; cleanup mata y recolecta fuera del registro.

La revisión de registros duraderos confirmó cleanup para estado UI/móvil/audio,
emuladores, métricas, señales, rate limits, watchers, ventanas, KV, Redis,
imágenes, tokenizadores, ONNX, PDF, progreso, routers, servidor, procesos y las
seis familias de colecciones. Las estructuras internas de red y bases de datos
de la VM viven dentro de `RuntimeState`; este estado se destruye únicamente
cuando root y todas las tareas han soltado su `Arc`. Por eso una operación de una
tarea en vuelo impide que el cleanup de su runtime empiece, además de la barrera
`shutting_down` que impide crear tareas hijas durante cierre.

Evidencia final:

- [CI 31633367279](https://github.com/alexsndersoto04-source/aio/actions/runs/31633367279): todos los pasos individuales aprobaron, incluidos formato,
  check normal, workspace tests, no-default-features y AArch64;
- [Termux ARM 31633367369](https://github.com/alexsndersoto04-source/aio/actions/runs/31633367369): check NDK estricto, build/enlace Android/Bionic ARMv7,
  ELF32/ARM, paquete y upload aprobaron;
- artefacto `zett-termux-arm-81`, id `9156036875`, 26.718.677 bytes, digest
  `sha256:28cee74a0bdb810bd7b2ccd12c075ab65b65a84d92284bec1cacef517cd21cd5`;
- `verify_phase34.py`: 758 nativas únicas, 837 llamadas verificadas y 110 brazos
  Phase 34 conectados.

Con esta evidencia queda cerrada la fase 2 de sandbox y seguridad. Esto no
convierte automáticamente las fases restantes en terminadas: el siguiente
bloque es la fase 3, typechecker y reglas del lenguaje.

### PDF real con memoria, ownership y publicación acotados

Los commits `6c54c9b`, `64a508c` y `0d10e16` reemplazan el registro PDF sin
cuotas por un modelo de dibujo propio y acotado. La característica `pdf_mod`
ya forma parte de `extras`, por lo que las nueve nativas `std::pdf::*` quedan
compiladas y conectadas por defecto dentro del único binario final.

`printpdf 0.7.0` representa un documento vivo con `Rc` y `Weak<RefCell<_>>`;
esos tipos no son `Send` y no pueden compartirse de forma segura entre tareas.
Por eso el registro global de TITAN **no almacena ningún objeto de printpdf**.
Conserva solamente páginas y comandos de texto, color, línea y rectángulo bajo
un lock individual por documento. Al guardar, copia una instantánea consistente,
suelta todos los locks y construye localmente el documento real. Otros handles
y runtimes pueden seguir avanzando durante la serialización y el I/O.

Los límites persistentes son:

- ocho documentos por runtime;
- 256 páginas por documento y 512 por runtime;
- 16.000 comandos por documento y 32.000 por runtime;
- 8 MiB lógicos por documento y 16 MiB por runtime;
- coste fijo de 1.024 bytes por documento, 512 por página y 512 por comando,
  además de los strings, para que miles de elementos vacíos también consuman
  cuota;
- título y nombre de capa de 4 KiB, texto de 256 KiB por comando y path de
  salida de 16 KiB.

Cada runtime admite cuatro operaciones PDF simultáneas y una sola serialización;
además hay un máximo global de dos serializaciones para acotar los buffers del
backend entre VMs. Un PDF serializado no puede superar 64 MiB. `printpdf`
construye internamente todo el archivo en un `Vec<u8>` antes de devolverlo, así
que el límite final se comprueba antes de publicar el archivo, mientras las
cuotas de páginas, comandos, strings y bytes lógicos acotan la entrada que puede
provocar esa asignación. No se afirma que el backend haga streaming: esa sería
una propiedad falsa de esta versión de printpdf.

Toda entrada numérica se valida antes de convertir `f64` de TITAN a los `f32`
que usa `Mm`: no se admiten `NaN` ni infinitos. Las páginas quedan entre 1 y
5.000 mm, las coordenadas dentro de +/-10.000 mm, las fuentes entre 0,1 y 1.000
puntos, los trazos entre 0,01 y 1.000 puntos y RGB entre 0 y 1. Rectángulos
requieren dimensiones positivas y extremos dentro del rango. Páginas, capas e
índices inválidos devuelven errores tipados sin consumir cuota.

La fuente integrada es Helvetica con `WinAnsiEncoding`. Los caracteres
representables —incluidos acentos españoles— se conservan; controles, saltos de
línea y caracteres no representables como emoji se rechazan explícitamente en
vez de permitir que printpdf los descarte en silencio. Los rectángulos se
materializan como polígonos reales con relleno y contorno; texto, colores y
líneas usan las operaciones reales de la capa PDF.

`save` serializa una instantánea y solo después crea un temporal exclusivo en el
mismo directorio, escribe todos los bytes, ejecuta `flush` y `sync_all`, y lo
publica mediante `rename`. Cualquier error elimina el temporal y no trunca el
destino antes de tener un PDF completo. El registro global nunca permanece
tomado durante serialización ni filesystem. `close` y cleanup retiran el handle,
marcan el estado cerrado y devuelven páginas, comandos y bytes; una reserva de
creación que estaba en vuelo no puede revivir el runtime después del cleanup.

Las diez regresiones del módulo y la regresión adicional del dispatcher VM
verifican:

1. un archivo con dos páginas, texto, color, línea y rectángulo, reabierto
   estructuralmente con `printpdf::lopdf`;
2. validación de números no finitos, rangos, texto WinAnsi, páginas y capas;
3. saturación y recuperación de handles, páginas, comandos, bytes, operaciones
   y slots de serialización por runtime y globales;
4. aislamiento de ownership, cleanup y la carrera cleanup/reserva;
5. reemplazo atómico, preservación ante un path fallido y eliminación del
   temporal;
6. paso real por las nueve nativas de la VM, incluido rechazo de `save` sin la
   capacidad de filesystem y generación correcta al concederla.

Evidencia externa final para `0d10e162f936d7f71bba2be0134f35d3b276161e`:

- [CI 31566525679](https://github.com/alexsndersoto04-source/aio/actions/runs/31566525679): `cargo fmt --check`, check normal, todos los tests —incluidas
  las once regresiones PDF—, no-default-features y AArch64 aprobaron. Las únicas
  annotations son avisos externos sobre Node.js 20.
- [Termux ARM 31566525670](https://github.com/alexsndersoto04-source/aio/actions/runs/31566525670): check NDK estricto, compilación y enlace Android/Bionic ARMv7,
  verificación ELF32/ARM, paquete y upload aprobaron; produjo
  `zett-termux-arm-76` de 26.849.784 bytes, digest de artifact
  `sha256:a7fa3fcdd58ce65dafd455128008401d2117c3bb65683b8eaf6abb597590073d`.
- `verify_phase34.py`: 758 nativas únicas, 837 llamadas verificadas y 110 brazos
  Phase 34 conectados.

El primer run [CI 31566175798](https://github.com/alexsndersoto04-source/aio/actions/runs/31566175798) compiló correctamente el backend, aprobó formato y AArch64, y
nueve de las diez regresiones PDF, pero falló porque el test exigía un salto de
línea concreto después de `%%EOF`. El archivo ya se reabría correctamente; el
commit `64a508c` retiró esa suposición textual y mantuvo la validación
estructural. [CI 31566327093](https://github.com/alexsndersoto04-source/aio/actions/runs/31566327093) quedó verde antes de añadir la prueba adicional del dispatcher. El fallo
intermedio se conserva y no se cuenta como validación aprobada.

Límites declarados: esta superficie usa una fuente integrada WinAnsi y no
soporta aún tipografías Unicode externas, imágenes, cifrado ni firmas PDF. El
límite de 64 MiB se comprueba al recibir el `Vec` de printpdf; las cuotas lógicas
y los dos slots globales son la defensa que acota el trabajo previo porque esta
versión del backend no ofrece serialización streaming. `rename` sustituye
atómicamente un destino existente en Linux y Android; en plataformas donde el
SO no permite reemplazar con `rename`, la operación falla conservando el archivo
anterior.

No hace falta una validación física individual para este bloque: el PDF se creó
y reabrió realmente en CI, toda la ruta quedó compilada y enlazada dentro del
ELF Android/Bionic ARMv7, y el paquete se reserva para el próximo milestone
físico agrupado.

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
  colecciones, PDF, HTTP/WebSocket y los recursos ligeros descritos arriba ya
  están conectadas. La fase 2 queda cerrada para ese alcance; los límites de
  memoria y concurrencia propios de fases posteriores siguen auditándose en sus
  bloques correspondientes.
- CI prueba directamente la destrucción con handles de colecciones, tareas,
  tokenizadores, planes ONNX de tract, bases/árboles sled, sockets Redis loopback
  y documentos PDF. Las demás rutas de limpieza compilan y son revisadas por
  Rust. Este run no conecta un Redis externo ni carga un `.onnx` válido de
  terceros.
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

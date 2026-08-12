# Registro visible de validación

Este documento registra evidencia comprobable. Un cambio no se marca como
validado solamente porque el código parezca correcto: debe tener pruebas y una
ejecución externa en GitHub Actions. Las pruebas físicas en el Redmi se anotan
por separado porque CI no puede sustituir un teléfono real.

## Estado actual

- Rama de trabajo: `arena/019ff232-aio`
- Commit validado: `1dce945e37dba4a1d5604ef8e3db1931a0145c9f`
- Cambio: liberación automática de recursos nativos al terminar una VM
- Fecha: 2026-08-11

### Evidencia automatizada

| Comprobación | Resultado | Evidencia |
|---|---:|---|
| Formato Rust (`cargo fmt --check`) | **Fallo advisory (exit 1)** | [CI 31549327056](https://github.com/alexsndersoto04-source/aio/actions/runs/31549327056) |
| `cargo check` con características normales | Aprobado | [CI 31549327056](https://github.com/alexsndersoto04-source/aio/actions/runs/31549327056) |
| Tests del workspace, todos los targets | Aprobado | [CI 31549327056](https://github.com/alexsndersoto04-source/aio/actions/runs/31549327056) |
| `cargo check --no-default-features` | Aprobado | [CI 31549327056](https://github.com/alexsndersoto04-source/aio/actions/runs/31549327056) |
| Cross-check Android AArch64 | **Fallo advisory (exit 101)** | [CI 31549327056](https://github.com/alexsndersoto04-source/aio/actions/runs/31549327056) |
| Compilación y enlace Android/Bionic ARMv7 | Aprobado | [Termux ARM 31549327029](https://github.com/alexsndersoto04-source/aio/actions/runs/31549327029) |
| ELF32 ARM y paquete Debian `Architecture: arm` | Aprobado | [Termux ARM 31549327029](https://github.com/alexsndersoto04-source/aio/actions/runs/31549327029) |
| Artefacto Termux subido por GitHub | Aprobado | [Termux ARM 31549327029](https://github.com/alexsndersoto04-source/aio/actions/runs/31549327029) |

La página general de CI aparece verde porque actualmente formato y cross-check
AArch64 tienen `continue-on-error`. Las anotaciones del job registran sus fallos,
por lo que **no se contabilizan como aprobados**. Los pasos obligatorios de
`cargo check`, tests y `--no-default-features` sí terminaron correctamente.
Esta distinción evita confundir un workflow verde con que absolutamente todos
sus comandos hayan pasado.

El workflow ARMv7 oficial no tiene esa excepción: usa Android NDK y
`armv7-linux-androideabi`. Además de compilar y enlazar, comprueba con `readelf`
que el binario sea ELF32/ARM, verifica los metadatos del `.deb`, extrae el
paquete y compara byte por byte el ejecutable empaquetado con el ejecutable
construido. Ese workflow sí terminó completamente aprobado.

### Qué prueban las regresiones nuevas

1. Al destruir el último estado de un runtime, sus handles de colecciones dejan
   de existir.
2. La limpieza no borra handles pertenecientes a otra VM viva.
3. Al destruir la VM principal, una tarea no unida que ejecuta un bucle infinito
   recibe cancelación.
4. Después de que la tarea observa la cancelación, el estado compartido del
   runtime se destruye; la prueba falla si queda retenido.
5. Existe una barrera de apagado para impedir que una tarea cree otra tarea
   después de que haya comenzado la destrucción del runtime.

La limpieza está conectada a los registros de ventanas, KV/sled, watchers,
Redis, imágenes, tokenizadores, ONNX, PDF, barras de progreso, routers,
servidores HTTP, solicitudes pendientes, WebSockets, procesos en segundo plano
y las seis familias de colecciones. La ruta de procesos mata y recolecta al
hijo; KV intenta vaciar escrituras antes de cerrar; las ventanas reales se
liberan en el mismo hilo que las creó.

### Límites de esta validación

- CI prueba directamente la destrucción con handles de colecciones y tareas.
- El formato completo del repositorio y el cross-check AArch64 continúan
  pendientes; sus fallos están en modo advisory y no invalidan el paquete
  ARMv7 oficial, pero tampoco se presentan como aprobados.
  Las demás rutas de limpieza compilan y son revisadas por el typechecker de
  Rust, pero este run no levanta servicios externos reales de Redis ni carga un
  modelo ONNX para destruirlos.
- El candidato de este commit todavía no se ha instalado en el Redmi 9C. No se
  requiere instalar cada cambio interno; habrá una prueba física agrupada al
  cerrar un milestone importante.
- Un resultado verde no significa que TITAN 1.0 esté terminado. Solo valida el
  commit y el alcance descrito aquí.
- El workflow histórico `android-apk - NEON FRACTURE (FIXED)` no forma parte de
  la validación oficial y sus resultados se ignoran.

## Bloque de capacidades anterior

El commit `ca7e3e7` también quedó validado externamente:

- [CI 31548478710](https://github.com/alexsndersoto04-source/aio/actions/runs/31548478710): aprobado.
- [Termux ARM 31548478652](https://github.com/alexsndersoto04-source/aio/actions/runs/31548478652): aprobado.

Ese bloque protege terminal, readline, información de procesos/directorios,
señales y detectores del entorno, incluida la exigencia combinada de filesystem
e interfaz de usuario para el historial persistente de readline.

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

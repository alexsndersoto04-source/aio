# Registro visible de validación

Este documento registra evidencia comprobable. Un cambio no se marca como
validado solamente porque el código parezca correcto: debe tener pruebas y una
ejecución externa en GitHub Actions. Las pruebas físicas en el Redmi se anotan
por separado porque CI no puede sustituir un teléfono real.

## Estado actual

- Rama de trabajo: `arena/019ff232-aio`
- Commit validado: `c694874e9c3ddc52039384b40527428494815cdf`
- Alcance: seguridad de capacidades, limpieza de recursos, formato y checks
  Android ARM de 32 y 64 bits
- Fecha: 2026-08-11

### Evidencia automatizada más reciente

| Comprobación | Resultado | Evidencia |
|---|---:|---|
| Formato completo, `cargo fmt --check` | Aprobado | [CI 31552463594](https://github.com/alexsndersoto04-source/aio/actions/runs/31552463594) |
| `cargo check` con características normales | Aprobado | [CI 31552463594](https://github.com/alexsndersoto04-source/aio/actions/runs/31552463594) |
| Tests del workspace, todos los targets | Aprobado | [CI 31552463594](https://github.com/alexsndersoto04-source/aio/actions/runs/31552463594) |
| `cargo check --no-default-features` | Aprobado | [CI 31552463594](https://github.com/alexsndersoto04-source/aio/actions/runs/31552463594) |
| Cross-check Android AArch64 | Aprobado | [CI 31552463594](https://github.com/alexsndersoto04-source/aio/actions/runs/31552463594) |
| AArch64 con compiladores reales de Android NDK y warnings estrictos | Aprobado | [Termux ARM 31552463503](https://github.com/alexsndersoto04-source/aio/actions/runs/31552463503) |
| Compilación y enlace Android/Bionic ARMv7 | Aprobado | [Termux ARM 31552463503](https://github.com/alexsndersoto04-source/aio/actions/runs/31552463503) |
| ELF32 ARM y paquete Debian `Architecture: arm` | Aprobado | [Termux ARM 31552463503](https://github.com/alexsndersoto04-source/aio/actions/runs/31552463503) |
| Artefacto Termux subido por GitHub | Aprobado | [Termux ARM 31552463503](https://github.com/alexsndersoto04-source/aio/actions/runs/31552463503) |

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

La limpieza está conectada a los registros de ventanas, KV/sled, watchers,
Redis, imágenes, tokenizadores, ONNX, PDF, barras de progreso, routers,
servidores HTTP, solicitudes pendientes, WebSockets, procesos en segundo plano
y las seis familias de colecciones. La ruta de procesos mata y recolecta al
hijo; KV intenta vaciar escrituras antes de cerrar; las ventanas reales se
liberan en el mismo hilo que las creó.

### Límites de esta validación

- CI prueba directamente la destrucción con handles de colecciones y tareas.
  Las demás rutas de limpieza compilan y son revisadas por Rust, pero este run
  no levanta un servidor Redis externo ni carga un modelo ONNX real para luego
  destruirlo.
- Los comandos de formato y del antiguo job AArch64 ya pasan, pero sus dos
  líneas `continue-on-error` permanecen en el workflow hasta aplicar desde
  GitHub web la plantilla corregida `docs/CI_WORKFLOW_TEMPLATE.yml`. La GitHub
  App de Arena no tiene permiso para modificar `.github/workflows`.
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

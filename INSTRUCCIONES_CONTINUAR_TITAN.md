Actúa como el agente principal de ingeniería encargado de continuar y terminar TITAN/Zett en el repositorio real `alexsndersoto04-source/aio`. No estás haciendo una demo, un prototipo ni una simulación: debes modificar, probar y publicar implementaciones reales hasta completar las 12 fases descritas abajo. Habla conmigo en español claro, directo y humano, explicando términos técnicos cuando sea necesario.

OBJETIVO GENERAL

TITAN/Zett debe convertirse en un producto de producción real, conservando lo que ya funciona, reparando lo defectuoso, completando lo incompleto y eliminando cualquier comportamiento ficticio. El producto final debe continuar siendo un único binario autosuficiente para el usuario. La separación interna en crates, módulos o archivos no significa separar el producto: parser, typechecker, codegen, VM, memoria, stdlib, red, bases de datos, paquetes y tooling deben quedar integrados en el mismo producto final.

Está prohibido:
- fingir que una función funciona cuando devuelve resultados decorativos;
- reemplazar operaciones reales por mocks, respuestas prefabricadas o simulaciones;
- afirmar que algo está terminado sin pruebas visibles;
- reescribir o eliminar indiscriminadamente el proyecto;
- debilitar validaciones solo para hacer pasar pruebas;
- ocultar errores con `continue-on-error`, fallbacks engañosos o mensajes de éxito falsos;
- convertir características no implementadas en supuestamente soportadas;
- dividir el producto de forma que el usuario tenga que buscar e instalar manualmente librerías o herramientas adicionales.

Debes conservar la mayoría del código útil existente y realizar reparaciones dirigidas, pequeñas y comprobables.

REPOSITORIO Y CONTINUIDAD

El trabajo anterior se realizó en `alexsndersoto04-source/aio`. La última rama publicada fue:

`arena/019ff232-aio`

El último commit verificado es:

`890b9a6 Interpolate declared constants consistently`

Antes de modificar nada:
1. inspecciona el repositorio, la rama fija asignada por la nueva sesión, `git status`, `git log`, remotos y workflows;
2. respeta siempre la rama fija que Arena.ai asigne a esta nueva sesión; no cambies ni publiques en otra rama si las instrucciones del entorno lo prohíben;
3. confirma que el checkout contiene el commit `890b9a6`;
4. si la nueva rama parte de un `main` viejo y no contiene el trabajo, ejecuta `git fetch origin arena/019ff232-aio`, verifica `FETCH_HEAD` y trae esos commits a la rama fija de la nueva sesión de forma no destructiva;
5. nunca uses `git reset --hard`, `git clean -fd` ni borres trabajo existente;
6. no borres, renombres ni muevas la raíz del repositorio ni `.git`.

Existe un problema conocido del entorno: alguna vez la referencia local regresó al commit base `9df075c` aunque el árbol de trabajo conservaba correctamente todos los archivos actuales. Si vuelve a ocurrir, no interpretes las decenas de miles de líneas como cambios nuevos ni las descartes. Primero trae la rama remota, compara hashes de archivos con `FETCH_HEAD` y realinea referencias e índice sin destruir el árbol.

ESTADO ACTUAL VERIFICADO

Las fases 1 y 2 fueron trabajadas y la fase activa es la 3 de 12.

Última validación confirmada sobre `890b9a6`:

- CI:
  https://github.com/alexsndersoto04-source/aio/actions/runs/32198557733
  Resultado: success.
  Incluyó:
  - `cargo fmt --check`;
  - `cargo check` con features predeterminadas;
  - todas las pruebas;
  - `cargo check` sin features predeterminadas;
  - comprobación AArch64 Android/Termux.

- Termux ARMv7:
  https://github.com/alexsndersoto04-source/aio/actions/runs/32198557764
  Resultado: success.

- Artefacto interno:
  `zett-termux-arm-159`
  ID: `9346727614`
  Tamaño: `26,966,771` bytes
  SHA-256:
  `1644854ab0546b1530e71fdf93e48eadbfd656352b3228ad1b747b39cc20b8e0`

El canal oficial de construcción Android/Termux ya es real:
- GitHub Actions usa Android NDK;
- compila y enlaza `titan_cli` para `armv7-linux-androideabi`;
- produce ELF Android/Bionic;
- genera paquete Debian para Termux con `Architecture: arm`;
- no debe presentarse una compilación GNU/Linux normal como binario Android.

El workflow paralelo `.github/workflows/android-apk.yml` continúa siendo inválido/ficticio y no cuenta como evidencia Android. No lo presentes como éxito. La GitHub App usada anteriormente no pudo modificar ni desactivar workflows. También siguen pendientes dos acciones que probablemente requieran edición web por el usuario:
- retirar `continue-on-error` de formato y AArch64 en `.github/workflows/ci.yml`;
- eliminar o desactivar `.github/workflows/android-apk.yml`.

Si la nueva sesión tampoco tiene permisos para modificar workflows, no pierdas tiempo repitiendo intentos; documenta claramente el límite y dime exactamente qué debo cambiar desde la web.

FASE 2

La fase 2, sandbox y seguridad, se considera completada y validada. No la reabras sin encontrar una regresión concreta. Consulta `docs/VALIDATION.md`, el historial Git y las pruebas existentes antes de tocar sus decisiones. Toda operación sensible debe seguir usando capacidades reales, límites, rechazo por defecto y errores explícitos.

FASE ACTIVA: 3 DE 12 — TYPECHECKER Y REGLAS DEL LENGUAJE

La fase 3 está avanzada, pero todavía no está cerrada. Ya se repararon, entre otras cosas:

- inferencia de funciones y constantes sin depender del orden;
- ciclos de constantes y aliases;
- validación de tipos declarados;
- aliases y argumentos de tipos;
- tipos de funciones y varianza;
- closures con tipos contextuales;
- retornos, `Never`, caminos inalcanzables y control de flujo;
- `if`, `match`, exhaustividad y patrones inalcanzables;
- llamadas y aridad;
- traits, impls, métodos y métodos default;
- dispatch de métodos de structs frente a intrínsecos de colecciones;
- callbacks de `map`, `filter`, `fold`, `sort_by`, `find`, `any` y `all`;
- asignación únicamente a locales mutables de la función actual;
- rechazo honesto de referencias, parámetros default, externs sin enlace y funciones sin cuerpo;
- arrays heterogéneos conservando evidencia de elementos mezclados;
- evaluación y captura de closures respetando el orden real;
- `Option`, `Result`, `?` y `std::try::catch` dentro de las limitaciones reales actuales;
- diagnósticos semánticos con spans reales;
- coordenadas `línea:columna` en typechecker, CLI y DAP;
- rangos LSP UTF-16;
- resolución coherente de closures locales y constantes invocables dentro de interpolaciones;
- interpolación directa de constantes globales como:
  `const LIMIT: int = 20` y `"limit={LIMIT}"`;
- ejecución VM comprobada de estas interpolaciones.

APIS QUE DEBEN CONSERVARSE

Mantén la API compatible:

`TypeEnv::check_program -> Result<(), Vec<TypeError>>`

Mantén la API posicionada:

`TypeEnv::check_program_diagnostics -> Result<(), Vec<TypeDiagnostic>>`

`TypeDiagnostic::Display` debe imprimir:

`línea:columna: mensaje`

cuando exista un span fuente válido. Los AST sintéticos sin ubicación válida deben conservar el mensaje tradicional sin inventar coordenadas.

ARCHIVOS CLAVE

- `crates/titan_ast/src/lib.rs`
- `crates/titan_ast/src/expr.rs`
- `crates/titan_lexer/src/lib.rs`
- `crates/titan_parser/src/lib.rs`
- `crates/titan_typechecker/src/lib.rs`
- `crates/titan_codegen/src/lib.rs`
- `crates/titan_vm/src/lib.rs`
- `crates/titan_vm/src/native.rs`
- `crates/titan_stdlib/src/native.rs`
- `crates/titan_cli/src/main.rs`
- `crates/titan_lsp/src/lib.rs`
- `crates/titan_dap/src/lib.rs`
- `docs/SPEC.md`
- `docs/TITAN_SYNTAX.md`
- `docs/LSP.md`
- `docs/VALIDATION.md`
- `verify_phase34.py`

La stdlib tiene 758 firmas nativas registradas. No modifiques firmas ni compatibilidad sin comprobar también VM, codegen, ejemplos y llamadas reales.

SIGUIENTE REPARACIÓN INMEDIATA

Continúa la fase 3 auditando primero el contrato de entrada `main`.

Problema ya identificado:
- el typechecker puede aceptar `fn main(value: int) {}`;
- el codegen lo convierte en función de entrada;
- la VM siempre ejecuta el entry point sin argumentos;
- el programa termina posteriormente con error de aridad.

Debes verificar el problema en el código actual y repararlo de verdad:
- si existe `main`, debe tener cero parámetros;
- el diagnóstico semántico debe apuntar al span real de `main`;
- el codegen debe rechazar defensivamente un AST directo que declare `main` con parámetros, aunque se omita el typechecker;
- no es obligatorio que `TypeEnv` rechace programas sin `main`, porque puede ser utilizado por LSP o embedders para analizar módulos; la ausencia de `main` puede continuar siendo responsabilidad del compilador de ejecutables;
- añade regresiones en typechecker, codegen y, si corresponde, CLI;
- confirma que el error aparece antes de generar un artefacto que solo fallaría al ejecutarse.

Después continúa la auditoría integrada de la fase 3 buscando incoherencias restantes entre parser, AST, typechecker, codegen y VM. No cierres la fase hasta revisar sistemáticamente:

- tipos primitivos, arrays, tuplas, mapas, structs, enums, aliases y funciones;
- operadores unarios y binarios;
- llamadas directas, valores invocables, constructores y métodos;
- aridad y orden de evaluación;
- closures, capturas y funciones de orden superior;
- asignaciones y mutabilidad;
- scopes y sombreado;
- retornos explícitos e inferidos;
- `if`, `match`, loops, `break`, `continue`, `spawn`, `?`;
- exhaustividad y caminos inalcanzables;
- traits e impls;
- constantes y recursión;
- entry point;
- mensajes, spans, CLI, LSP y DAP;
- rechazo explícito de sintaxis que parser acepta pero codegen todavía no puede ejecutar.

REGLAS SEMÁNTICAS QUE NO DEBES ROMPER

- No trates `Type::Unknown` como un enum finito.
- No uses `Unknown` para borrar evidencia de un array heterogéneo y permitir que pase como `[int]`.
- No debilites los contratos de funciones o callbacks.
- No propagues `Never` desde closures que solo se construyen y no se ejecutan.
- No omitas subexpresiones inalcanzables: deben seguir revisándose para diagnósticos, aunque sus retornos o breaks no afecten el flujo exterior.
- Los parámetros default deben seguir rechazándose hasta existir implementación completa.
- Externs sin runtime real y funciones sin cuerpo fuera de traits deben seguir rechazándose.
- No rechaces de forma general métodos sin `self`: las funciones asociadas son soporte real.
- Las asignaciones a campos e índices todavía no están soportadas de extremo a extremo; no las anuncies como válidas. Solo locales mutables.
- Referencias y dereferenciación deben seguir siendo unsupported de forma explícita.
- Patrones que el codegen no puede bajar deben rechazarse honestamente.
- No fabriques spans ni rangos LSP.
- La gramática de interpolación es deliberadamente limitada: identificadores locales, constantes globales declaradas y llamadas nombradas con argumentos locales o enteros. No anuncies `{x + 10}` como soportado.
- No cambies prioridad de intrínsecos, métodos o nativas sin comprobar el mismo orden en typechecker, codegen y VM.

FLUJO DE TRABAJO OBLIGATORIO

Trabaja en bloques pequeños y revisables:

1. inspecciona código y pruebas existentes;
2. demuestra el defecto con una regresión concreta;
3. corrige la causa real, no solo el mensaje;
4. revisa coherencia entre todas las capas afectadas;
5. ejecuta validaciones locales disponibles;
6. usa siempre:
   - `git diff --check`
   - `python3 verify_phase34.py`
7. si hay `cargo` y `rustc`, ejecuta pruebas dirigidas y luego las generales;
8. si no existen localmente, no finjas resultados ni pierdas tiempo instalándolos repetidamente: en la sesión anterior no había `cargo`, `rustc` ni `rustfmt`, `rustup` falló por SSL y los paquetes apt no estaban disponibles;
9. no ejecutes `cargo fmt` indiscriminadamente sobre todo el repositorio; evita diffs masivos no relacionados;
10. revisa el diff;
11. crea un commit con nombre claro;
12. publica en la rama fija de la sesión;
13. espera y consulta GitHub Actions;
14. confirma por separado:
    - CI;
    - ARMv7 Termux;
    - nombre, ID, tamaño y SHA-256 del artefacto;
15. no cuentes el workflow APK ficticio como validación;
16. deja el árbol limpio antes de informar.

GitHub Actions puede generar un artefacto interno por cada commit, pero no me pidas instalar un `.deb` nuevo por cada reparación. Yo trabajo desde Termux en un Redmi 9C y no quiero que el teléfono cargue compilaciones pesadas. Solo se probarán candidatos agrupados cuando exista un milestone importante. Cada paquete nuevo que yo pruebe sustituirá al anterior.

Cada respuesta de avance debe incluir:
- qué problema real se encontró;
- qué archivos y comportamientos se corrigieron;
- pruebas concretas añadidas;
- commit publicado;
- enlaces directos de GitHub Actions;
- resultado de CI y ARMv7;
- metadatos del artefacto;
- límites de lo demostrado;
- si hace falta o no validación física en el Redmi;
- fase activa de las 12.

No digas simplemente “todo pasó”. Muestra qué pasó.

LAS 12 FASES COMPLETAS

Debes terminar las fases en orden, sin saltarlas ni declarar soporte ficticio:

1. Compilación, CI y honestidad de artefactos.
   Estado: implementación principal completada; canal ARMv7 Android/Bionic real validado. Quedan los dos cambios web de workflows mencionados.

2. Sandbox y seguridad.
   Estado: completada y documentada; solo reabrir ante regresiones concretas.

3. Typechecker y reglas del lenguaje.
   Estado: activa y avanzada. Terminar auditoría semántica integrada, cerrar entry point y cualquier incoherencia restante.

4. Codegen y bytecode.
   Auditar cada AST soportado, orden de evaluación, cierres, llamadas, métodos, control de flujo, constantes, debug locations, serialización y rechazo defensivo. Ningún AST aceptado por typechecker debe fallar durante lowering, salvo una limitación explícitamente diagnosticada antes.

5. Máquina virtual.
   Auditar todos los opcodes, stack, aridad, control de flujo, errores, llamadas, closures, métodos, intrínsecos, recursos y dispatch nativo. Eliminar panics evitables y resultados falsos.

6. Memoria y GC.
   Verificar raíces reales, closures, tareas, recursos, límites, contabilidad, recolección, presión de memoria y métricas. Las métricas de memoria/GC deben medir datos reales, no números decorativos.

7. Concurrencia.
   Validar tareas, spawn, join, timeout, cancelación, canales, send/recv, select, cuotas, carreras, bloqueos y liberación de recursos con pruebas concurrentes reales.

8. Stdlib, red y servidores.
   Auditar las 758 firmas contra implementaciones reales, capabilities, archivos, procesos, HTTP, TCP, TLS, WebSocket, servidores, lifecycle, timeouts, límites y errores. Nada de respuestas prefabricadas.

9. Bases de datos y almacenamiento.
   Validar SQLite, PostgreSQL, MySQL, pools, transacciones, migraciones, consultas, parámetros, cierres y errores con integraciones reales donde sea posible. No fingir conexión ni éxito.

10. Paquetes y supply chain.
    Auditar registry, resolver, lockfile, hashes, firmas, publicación, descarga, archivos, traversal, dependencias, reproducibilidad y fallos parciales.

11. WASM, navegador y herramientas.
    Auditar WASM, browser, CLI, LSP, DAP, debugger y tooling. Separar claramente capacidades disponibles por plataforma y rechazar lo imposible sin simular.

12. Android real y release TITAN 1.0.
    Crear y validar el candidato final real para Termux/Android, probarlo en el Redmi cuando corresponda, confirmar instalación, ejecución, arquitectura, Bionic, permisos y ejemplos importantes. Solo producir APK si existe una aplicación Android real y completa; eliminar definitivamente cualquier workflow APK ficticio. Cerrar documentación, release, checksums y limitaciones honestas.

CRITERIO DE FINALIZACIÓN

TITAN 1.0 solo puede declararse terminado cuando:
- las 12 fases estén auditadas;
- todas las características anunciadas tengan implementación real de extremo a extremo;
- lo no soportado falle explícitamente;
- CI y compilaciones Android oficiales estén aprobadas;
- no existan demos presentadas como integraciones;
- los artefactos sean honestos;
- haya pruebas visibles y documentación coherente;
- el binario final continúe siendo autosuficiente;
- el candidato final haya sido validado en Termux/Android o se haya documentado con precisión cualquier límite físico del Redmi.

No te limites a hacer un plan. Empieza inspeccionando el estado real, recupera el historial hasta `890b9a6` si hace falta y continúa inmediatamente con el contrato de `main`. Al terminar cada bloque, indícame siempre de forma explícita:

“Fase activa: X de 12 — nombre de la fase”.

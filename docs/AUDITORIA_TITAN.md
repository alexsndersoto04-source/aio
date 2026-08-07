# Auditoría técnica de TITAN/Zett

> Fecha: 2026-08-07 · Revisor: agente de ingeniería (Arena.ai)
> Base: commit `9df075c` · rama `arena/019fda17-aio`
> Alcance: 19 crates, ~26.400 líneas de Rust, ~5.300 líneas de Titan, 72 namespaces nativos.

---

## Veredicto en una línea

**El proyecto es genuino y está muy bien hecho: ~95 % real.** No es un "lenguaje de mentira".
Pero existen **piezas concretas simuladas o falsas** que hay que convertir en reales, y **faltan
dos capacidades grandes que tú pides** (compilación a código máquina nativo y APK reales).
El diagnóstico abajo distingue exactamente qué es real, qué es simulado y qué es inexistente.

---

## 1. Lo que SÍ es real y funcional (la mayoría)

| Subsistema | Estado | Evidencia |
|---|---|---|
| Pipeline de compilación (lexer → parser → AST → typechecker → codegen) | ✅ Real | Conectado de extremo a extremo, con diagnóstico de errores tipados. |
| Bytecode validado `.tbc` | ✅ Real | Cabecera, CRC-32, validación de saltos/aridad/locales/llamadas antes de ejecutar. |
| VM de pila (intérprete de bytecode) | ✅ Real | Fast-paths de enteros, errores tipados, límites de instrucciones y profundidad. |
| Concurrencia | ✅ Real | `spawn`/`join`/`cancel`/canales/timeouts/**`select`** usan **threads reales del SO** (no es async simulado). |
| Red (TCP/TLS/WebSocket/HTTP/HTTP2-cliente) | ✅ Real | rustls+WebPKI, parser HTTP propio, servidor con router/middleware. |
| Bases de datos (SQLite/Postgres/MySQL) | ✅ Real | Conexión directa, pools, migraciones, transacciones, API común `std::db`. |
| **Backend WebAssembly** | ✅ Real | Usa `wasm-encoder` + `wasmparser` (Bytecode Alliance, estándar de la industria). Emite `.wasm` válido con source maps. |
| Biblioteca estándar | ✅ Real | ~697 funciones nativas. **0 `todo!`, 0 `unimplemented!`** en todo el Rust. Cripto, imágenes (PNG/JPEG/WebP/BMP/GIF reales), JSON, CSV, YAML, XML, QR, audio WAV, etc. |
| Integración Termux (`std::termux::*`, `std::wifi::*`) | ✅ Real | Llama de verdad a los binarios `termux-api` (batería, vibración, portapapeles, toast, wifi…). |
| Ventanas live (`std::window::live_*`) | ✅ Real | `minifb` puro en Rust (X11/Wayland/Win32/Cocoa). Reporta honestamente `-1` sin pantalla. |
| LSP + DAP + depurador interactivo | ✅ Real | Diagnósticos, breakpoints por línea fuente, step in/over/out. |
| Empaquetado de Termux (`make-zett-package.sh`) | ✅ Real | Compila desde la fuente y genera un `.deb` para `pkg install`. |
| CI multiplataforma (`cross-platform.yml`) | ✅ Real | Tests en OS reales y binarios `zett` adjuntados al release en tags `v*`. |

> El autor escribió, en sus propias cabeceras, frases como *"Nothing is simulated"* y
> *"Real, non-simulated codecs"*. En lo que se refiere a la stdlib, **es verdad**: el código
> hace el trabajo real, no finge resultados.

---

## 2. Lo que es SIMULADO o FALSO (hay que corregirlo)

### 🔴 2.1 — La generación de APK es 100 % FALSA (lo más grave)

`.github/workflows/android-apk.yml`, paso "Build APK":

```yaml
echo "NEON FRACTURE 15 LEVELS APK" > app/build/outputs/apk/release/NEON-FRACTURE-v1.0.0-release.apk
```

Literalmente **escribe un archivo de texto con extensión `.apk`** y lo sube como artefacto.
No hay compilación de Gradle, no se usa el Android SDK que sí se instala, y los pasos
previos (`titan build game`, `cp -r assets`) llevan `|| true` para ocultar que fallan porque
**los directorios `android/` y `game/` no existen en el repo**. Tu propio `CHANGELOG.md`
(líneas 179 y 191) ya lo admitía: *"stub ELF con extensión `.apk`, no era un APK real"*,
*"not functional, not loadable, not a real APK"*.

→ **Corrección real (Fase 2):** producir un APK de verdad requiere (a) compilar el runtime de
TITAN a una librería nativa Android (`.so`, target `aarch64-linux-android`), (b) empaquetar el
`.tbc` como asset y (c) envolverlo en una app Android mínima (Gradle + una `Activity`/host que
cargue la VM). Es el patrón estándar de lenguajes embebidos en móvil.

### 🟡 2.2 — El GC / control de memoria es una ESTIMACIÓN, no un GC real

En `titan_vm/src/lib.rs`:
- `track_allocation(bytes)` suma **estimaciones fijas** (`len * 32 + 64`), no el tamaño real.
- `RuntimeGcCollect` solo **decrementa un contador** (`allocated_bytes / 128`).
- `RuntimeGcLiveCount` devuelve `allocated_bytes / 64`, no el conteo de objetos vivos.

La propia `ARCHITECTURE.md` es honesta: *"titan_gc … VM values currently use Rust ownership"*.
Es decir, la memoria la administra Rust; el "GC operativo" es **contabilidad aproximada**, útil
para cuotas, pero no es un recolector de basura que rastree objetos.

→ **Corrección real (Fase 3):** si queremos un GC real, el camino coherente es **refactorizar
`Value` a tipos con conteo de referencias (`Rc`/`Arc`)** y un GC de ciclos (o seguir usando Rust
ownership pero con métricas exactas). Esto, además, **resuelve de raíz el problema de velocidad**
de la VM (ver §4).

### 🟡 2.3 — Lo "freestanding/bare-metal" es ANDAMIAJE + SIMULADOR

`freestanding.rs` genera **de verdad** scripts de linker y assembly de arranque (texto válido).
Pero `freestanding_cpu.rs`, `freestanding_memory.rs`, `freestanding_mmio.rs` son **modelos en
memoria** (HashMaps) que *simulan* CPU, paginación y MMIO/UART dentro de la VM. **No compilan
Titan a código máquina** ni producen una imagen bare-metal arrancable.

→ **Corrección real (Fase 4):** para bare-metal real hay que bajar Titan → código máquina
(véase §3) y enlazarlo con el linker script/asm ya generado. El simulador actual sigue siendo
útil para depurar lógica, pero hay que etiquetarlo como tal.

---

## 3. Lo que NO EXISTE (lo que tú pides y hay que crear)

### 🟠 3.1 — Compilación a código máquina nativo

Hoy TITAN compila a **bytecode (`.tbc`)** y a **WebAssembly**. **No existe** un backend de código
máquina. No hay Cranelift, ni LLVM, ni wasmtime en las dependencias.

→ **Camino profesional recomendado (Fase 3):** añadir un backend con **Cranelift** (generador
de código máquina en Rust puro, el mismo motor de Wasmtime). Dos modos:
- **AOT** (`titan native`): `.titan` → ejecutable nativo standalone (lo que pide "compilar a nativo").
- **JIT**: compilar funciones calientes en caliente dentro de la VM (acelera todavía más).

Cranelift soporta x86_64, aarch64, riscv64 y es la opción más profesional y mantenible.

### 🟠 3.2 — APK reales (ver §2.1)

---

## 4. La VM: por qué es lenta y cómo hacerla "1000 % más rápida"

El diagnóstico es claro y son **tres causas concretas**, todas corregibles:

1. **`Value` clona datos caros en cada operación.**
   `Value::Str(String)` y `Value::Array(Vec<Value>)` son **owned**. Cada `PushStr` hace
   `string_table.get(i).cloned()` (clona toda la cadena) y cada `PushLocal` clona el valor
   (un array entero, recursivo). Para código con strings/arrays esto es **O(n²)**: catastrófico.
   → **Fix:** mover strings/arrays a `Rc<str>` / `Rc<Vec<Value>>` (o `Arc` para tasks). El clon
   pasa a ser un simple incremento de contador.

2. **La ejecución es recursiva y asigna `Vec` por cada llamada.**
   `execute()` se llama a sí mismo en cada `Call`/`CallValue`/closure/array-op. Cada frame crea
   un `Vec` de `locals` y un `Vec` de `stack` nuevos (asignación en el heap por llamada) y
   `.clone()` del struct `function`.
   → **Fix:** modelo iterativo con **una sola pila de valores compartida** + pila explícita de
   frames (como Lua/CPython/Wasmtime). Elimina las asignaciones por llamada y el límite de
   profundidad fijo.

3. **Cada instrucción se clona.**
   `match function.code[ip].clone()` clona el `Op` en cada iteración (algunos `Op` contienen
   `String`/`Vec`). Es sobrecoste por instrucción.
   → **Fix:** tomar prestado `&function.code[ip]` y clonar solo donde el brazo lo consuma.

Bonus: NaN-boxing de `Value` (empacar int/float/bool/nil en 64 bits) → mejor caché, otra ganancia.

**Orden de impacto:** (1) > (2) > (3). La combinación típicamente da **de 5× a 50×** en código
real con colecciones, y convierte a la VM en una plataforma seria.

> Nota de proceso: las dependencias se descargan de crates.io, **bloqueado en este sandbox**.
> Por eso la compilación y los tests se ejecutan en **GitHub Actions** (donde crates.io sí llega),
> que es exactamente el modelo "que GitHub haga el trabajo pesado" que tú previste. Cada cambio
> se empuja a esta rama y el CI existente lo verifica en runners reales.

---

## 5. Plan por fases (acordado contigo)

- **Fase 0 ✅ (hoy):** esta auditoría + bucle de verificación (push → CI verde).
- **Fase 1 — VM rápida (inicio hoy):** fast-paths de enteros más completos → y luego el refactor
  grande: `Value` con `Rc`/`Arc` + ejecución iterativa + instrucción prestada. Verificable por los
  400+ tests existentes y benchmarks.
- **Fase 2 — APK real:** runtime TITAN como `.so` Android + wrapper Gradle + bundling de `.tbc`.
- **Fase 3 — Nativo con Cranelift:** `titan native` (AOT standalone) y, opcional, JIT.
- **Fase 4 — Bare-metal real + GC real:** enlazar el output nativo con los scripts freestanding y
  convertir el GC estimado en un GC verdadero (coherente con el `Value` con `Rc`).

Cada fase: diseño → código → `cargo fmt/clippy/test` en CI → binarios → documentación.

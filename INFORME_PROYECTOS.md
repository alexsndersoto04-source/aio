# Informe de magnitud y estado — TITAN/Zett + Moon

**Fecha:** 12 de septiembre de 2026
**Repositorio:** `alexsndersoto04-source/aio` (commit base `966e7d0`, rama de trabajo `arena/01a09451-aio`)
**Método:** medición directa sobre el código (LOC, tablas de símbolos, dependencias, workflows, releases vía API de GitHub), más ejecución real de lo que sí se podía ejecutar en este entorno (build del frontend, verificador estático del propio repo, consultas a la API de GitHub).

> **Nota de honestidad:** este sandbox **no tiene toolchain de Rust ni acceso a crates.io**, y las descargas de *release assets* y *artifacts* de GitHub están bloqueadas por red. Por eso **no pude compilar ni ejecutar TITAN**, ni bajar los binarios oficiales. Todo lo que digo aquí es o medido sobre el código, o verificado en GitHub. Lo que no pude verificar, lo digo explícitamente.

---

## 1. Resumen ejecutivo

| | **TITAN / Zett** (tu lenguaje) | **Moon** (tu red social) |
|---|---|---|
| Qué es | Lenguaje compilado + VM + stdlib + tooling, escrito en Rust | Red social web completa (backend en TITAN + Postgres, frontend React) |
| Tamaño | **64.937 líneas de Rust** en 108 archivos, 19 crates | **4.582 líneas** de TITAN (17 módulos) + **3.669 líneas** JS/JSX |
| Superficie | **880 funciones nativas** `std::*` en **78 namespaces**; **170 opcodes** | **69 rutas** REST + WebSocket `/ws`; **21 tablas**; **11 vistas** |
| Pruebas | **637 tests** Rust en CI (verde en `main`) | **0 pruebas en `main`**; el check de CI **nunca ha pasado** (0/16) |
| Documentación | 28 docs (≈3.800 líneas) + README + CHANGELOG de 37 fases | SPEC + STATUS + DEPLOY (muy buenos) |
| Releases | 45 releases, tags hasta **v1.0.28**; última release **con binarios: v1.0.26** | Sin release propio (se despliega junto al repo) |
| Estado real | **Producto maduro y verificable**, con deuda de versionado y de higiene | **Código muy completo pero NO compila** (no pasa el propio checker de TITAN) y no está desplegado |
| Riesgo nº1 | Los tags de release apuntan a commits de diagnóstico, no a `main` | El backend nunca se ha ejecutado con éxito; el error exacto está oculto por un bug de CI |

**La frase que resume todo:** TITAN es un proyecto **grande y real** (nivel "lenguaje pequeño-medio con ecosistema"), y Moon es una aplicación **grande en ambición y muy completa en código, pero aún no verificada en ejecución**: no hay una sola evidencia de que el backend haya arrancado alguna vez.

---

## 2. Mapa del repositorio

```
aio/
├── crates/                 19 crates Rust  → el lenguaje TITAN (64.937 LOC)
├── examples/               59 programas .titan (5.169 líneas)
├── docs/                   28 documentos (SPEC, ARCHITECTURE, STDLIB, TERMUX…)
├── stdlib/async.titan      módulo de la stdlib escrito en el propio lenguaje
├── projects/moon/          MOON: backend (17 módulos .titan) + SPEC/STATUS/DEPLOY
├── frontend/               MOON: SPA React 18 + Vite (11 vistas, 24 archivos)
├── scripts/                packaging Termux, NDK, flaky-check, publish APT
├── .github/workflows/      6 workflows (CI, cross-platform, Termux ×2, Moon, Diag)
├── Dockerfile + render.yaml  despliegue de Moon (Render)
├── CHANGELOG.md            80 KB, 37 versiones documentadas (hasta 0.40.0)
└── README.md               16 KB de presentación de TITAN/Zett
```

Totales del repo (excluyendo `.git` y `node_modules`): **≈86.600 líneas** de código y documentación.

| Tipo | Archivos | Líneas |
|---|---:|---:|
| Rust (`.rs`) | 108 | 64.937 |
| TITAN (`.titan`) | 77 | 9.856 |
| React/JSX | 20 | 2.480 |
| JavaScript | 6 | 1.189 |
| Markdown | 31 | 6.643 |
| Workflows YAML | 9 | 581 |

---

## 3. Proyecto 1 — TITAN / Zett (tu lenguaje de programación)

### 3.1 Qué es

Un lenguaje compilado y verificado estáticamente, implementado en Rust: código fuente `.titan` → lexer → parser → AST → typechecker → codegen → **bytecode `.tbc` validado** (cabecera + CRC-32 + límites) → **VM de pila segura**. También genera **WebAssembly**. "Zett" es solo el nombre del binario en Android/Termux; es el mismo proyecto.

### 3.2 Magnitud real, crate por crate

| Crate | LOC | Qué hace |
|---|---:|---|
| `titan_stdlib` | 27.661 | 758 funciones nativas en 72 namespaces (77 archivos) |
| `titan_vm` | 12.698 | VM de pila, concurrencia, GC, sandbox, debugger |
| `titan_typechecker` | 7.071 | Chequeo de tipos + 122 firmas nativas extra (DBs, WS…) |
| `titan_wasm` | 6.176 | Backend WebAssembly (memoria lineal, strings, structs…) |
| `titan_codegen` | 2.234 | AST → bytecode (**170 opcodes**) + artefacto `.tbc` |
| `titan_parser` | 2.003 | Parser del lenguaje |
| `titan_pkg` | 1.835 | Proyectos multiarchivo, paquetes firmados, registry |
| `titan_lsp` | 916 | Servidor de lenguaje (LSP) |
| `titan_cli` | 751 | 16 subcomandos (`run`, `build`, `check`, `wasm`, `debug`, `repl`, `pack`, `publish`…) |
| `titan_lexer` | 621 | Lexer |
| `titan_dap` | 541 | Protocolo de depuración (DAP) |
| `titan_sqlite` / `titan_postgres` / `titan_mysql` | 1.505 | Conectores de base de datos reales |
| `titan_ast`, `titan_tls`, `titan_gc`, `titan_runtime`, `titan_macros` | 925 | Base del lenguaje y soporte |
| **Total** | **64.937** | |

**Ecosistema:** 758 nativas en 72 namespaces (`std::text`, `std::image`, `std::server`, `std::postgres`, `std::jwt`, `std::onnx`, `std::gui`, `std::termux`…), más 122 firmas del registro del typechecker (SQLite/Postgres/MySQL, WebSocket, etc.) → **880 funciones nativas distintas, sin solapamientos**, en **78 namespaces**. El verificador del propio repo (`verify_phase34.py`) lo confirma: *"758 entradas, 758 únicas; 837 llamadas `std::*` verificadas"*.

**Calidad e ingeniería real:**
- **637 tests Rust** (`titan_stdlib` 330, `titan_vm` 125, `codegen` 13, `pkg` 10, `lsp` 6, `dap` 3).
- **CI verde en `main`** en la última subida: `CI` ✅, `cross-platform` ✅, `Termux AArch64` ✅, `Termux ARM 32-bit` ✅.
- Histórico: `ci.yml` 160/200 ejecuciones exitosas; `cross-platform` 82/113; `termux-arm` 149/200.
- **Sin `TODO`, `FIXME`, `todo!()` ni `unimplemented!()`** en todo `crates/` (buena señal).
- Dependencias de terceros serias y bien elegidas: `rusqlite`, `postgres`, `mysql`, `rustls`, `lettre` (email), `jsonwebtoken`, `argon2`, `image`, `printpdf`, `tokenizers` + `tract-onnx` (IA local), `minifb` (ventanas reales), `sled`, `redis`, `plotters`, `wasm-encoder`… 51 dependencias, casi todas opcionales tras features (meta-feature `extras`).
- Detalles de producto que casi nadie tiene: sandbox por capacidades, cuotas de memoria por tarea, heap dump JSON, export Prometheus, LSP, DAP, debugger, paquetes firmados con `ed25519`, APT repo propio para Termux.

**Escala comparable (aproximada):** el intérprete completo de Lua 5.4 ronda las **30.000 líneas de C**; `clox` de *Crafting Interpreters* son ~4.000; CPython pasa de las **500.000**. Con ~65.000 líneas de Rust, TITAN está en la liga de **lenguaje real de tamaño medio**, no de juguete — pero tampoco es aún un compilador industrial.

### 3.3 Lo que está probado y lo que no

| ✅ Verificado | ⚠️ No verificado / incompleto |
|---|---|
| Compila y pasa 637 tests en 3 SO + 2 arquitecturas ARM en CI | **No hay evidencia de usuarios reales ni de proyectos de terceros** en TITAN |
| 758 nativas sin duplicados, llamadas de ejemplos/stdlib con aridad correcta | `titan native` y `titan mobile` (ELF/APK propios) son **experimentales** y no generan artefactos cargables (lo dice el propio CHANGELOG) |
| WASM, DBs, red, TLS, imágenes, PDF, ONNX: módulos reales, no stubs | `std::audio::play` no usa dispositivo de audio: delega en `termux-media-player` (honesto, pero **solo funciona en Termux**) |
| Windows/macOS/Linux + Android 32/64 bits | Ejemplos que necesitan recursos externos (ONNX, pantalla, DB) no se ejecutan en CI |

### 3.4 ⚠️ Problema grave de versionado y releases (lo más importante de esta sección)

1. **La versión del binario no coincide con el tag.** `titan_cli` imprime `env!("CARGO_PKG_VERSION")` = **1.0.0 siempre**, incluso en el binario del tag `v1.0.26`. No puedes saber qué release estás ejecutando con `--version`.
2. **El CHANGELOG se quedó en 0.40.0** mientras `Cargo.toml` dice 1.0.0 y los tags llegan a **v1.0.28**. Tres numeraciones distintas conviviendo.
3. **Los tags de release no están en `main`.** Verificado por API: los tags **v1.0.20 → v1.0.28** apuntan a commits de la rama de diagnóstico `arena/01a040eb-aio`, con mensajes como *"diag(mirror): probe 2 — medir GITHUB_TOKEN en el build script"*, *"v13 — panic probe en la LINEA 1 del build script"*, *"self-test v11 — canal de diagnóstico por RELEASE"*. **Ninguno es un commit de `main`.**
4. **v1.0.27 y v1.0.28 existen como tag pero NO tienen release publicado** (sus workflows fallaron a propósito: el `build.rs` de prueba hacía `exit(3)`). La última release con binarios es **v1.0.26 (11-sep)**, con 5 artefactos (~17,9 MB el de linux-x86_64).
5. **Ese mecanismo de "mirror" es código que empuja al repo usando el token de CI.** En la rama de diagnóstico hay un `crates/titan_parser/build.rs` (603 líneas) que, al compilar en CI, **lee `GITHUB_TOKEN` y publica el binario en ramas `tools-*`** mediante API/push. No es malicioso (era un truco para sacar binarios del sandbox, que no puede bajar *release assets*), pero es exactamente el patrón que un revisor de seguridad marca como riesgo: *un build script que usa el token del CI para escribir en el repositorio*. Y salió en **releases publicadas** (v1.0.24–v1.0.26 se construyeron desde esos commits).
6. La rama `tools-zett-x86_64` que ese mecanismo usa contiene hoy un fichero de **30 bytes** que dice `#!/bin/bash\necho zett-fake-v2` — es un **artefacto falso** de las pruebas con compilador simulado. Cualquiera que encuentre esa rama (es pública) pensará que tus binarios son falsos.

**Recomendación P0:** cortar un release limpio desde `main` (p. ej. `v1.0.29`), borrar/archivar las ramas `tools-*` y `arena/*` viejas, quitar el `build.rs` espejo de cualquier rama desde la que se etiquete, y hacer que la versión del binario salga del tag (`build.rs` con `ZETT_VERSION` o `git describe`).

---

## 4. Proyecto 2 — Moon (tu red social)

### 4.1 Qué es y qué pretende

Una red social web "de nivel startup": registro/login con 2FA, posts con fotos, feed, explorar con *trending*, follows/bloqueos, mensajería directa en tiempo real, notificaciones en vivo, panel de administración, moderación y auditoría. Principio declarado en su SPEC: **"CERO SIMULACIÓN"** — todo contra base de datos real.

### 4.2 Magnitud

| Componente | Medida |
|---|---|
| Backend TITAN | **17 módulos, 4.582 líneas** (`main.titan` 615, `h_auth.titan` 612, `h_posts.titan` 514…) |
| API | **69 rutas** registradas en el router (30 GET, 25 POST, 5 DELETE, 2 PATCH explícitos + ramas multi-método) + WebSocket `/ws` |
| Base de datos | **21 tablas**, **14 migraciones versionadas**, 20 índices explícitos |
| Uso del lenguaje | **1.076 llamadas `std::*`** a **85 funciones distintas** |
| Frontend | **24 archivos JS/JSX, 3.669 líneas**, 11 vistas, router por hash |
| Build del frontend | **✅ verificado por mí**: 53 módulos → 210,55 KB JS (64,17 KB gzip) + 13,21 KB CSS, en 1,18 s |
| Despliegue | `Dockerfile` (Ubuntu 24.04 + binario `zett` v1.0.0) + `render.yaml` (API + Postgres + web) |

### 4.3 Lo que verifiqué yo mismo sobre Moon (con resultado)

1. **✅ Build del frontend:** `npm ci && npm run build` funciona limpio. Los números coinciden exactamente con lo que dice tu `STATUS.md` (53 módulos, 210 KB / 64 KB gzip).
2. **✅ Todas las llamadas `std::*` existen y con la aridad correcta.** Escribí un verificador que cruza las **1.076 llamadas** de los 17 módulos contra los **dos registros nativos** del compilador (758 + 122 = 880 firmas): **0 funciones desconocidas, 0 errores de aridad**. Es la primera vez que esta comprobación se hace incluyendo el registro del typechecker (el script del repo solo cubría `examples/` y `stdlib/`).
3. **✅ Datos coherentes:** las 21 tablas que crean las migraciones son exactamente las que consultan los handlers (`FROM`/`JOIN`/`INTO`/`UPDATE`); no hay referencias a tablas fantasma.
4. **✅ Contrato frontend ↔ backend consistente al 100 %.** Cruzé las **52 rutas distintas** que llama el frontend contra las **69 rutas registradas** en el router del backend: **0 desajustes**. La única dinámica (`/api/feed/${tab}`) resuelve a las tres rutas declaradas (`/api/feed/latest`, `/for-you`, `/trending`). Es decir: no hay pantallas llamando a endpoints que no existen.
5. **✅ Configuración defensiva real:** `config.titan` aborta el arranque si falta `DATABASE_URL` o si `JWT_SECRET` mide <32 caracteres.
6. **❌ El backend NO pasa el chequeo del propio lenguaje.** Ver apartado 4.4.
7. **❌ Nunca se ha ejecutado.** No hay ninguna evidencia (release, log, captura, CI) de que el backend haya arrancado. Tu propio `STATUS.md` lo admite: la verificación fue **estática**.

### 4.4 🔴 El problema central: `zett check` lleva **rojo desde el día 1** y nadie ha visto por qué

- El workflow `Moon checks` se añadió el **26-ago** y acumula **16 ejecuciones, 0 exitosas**. Falla siempre en el paso `cargo run -p titan_cli -- check projects/moon/src/main.titan`.
- Fui a buscar el diagnóstico exacto y encontré esto: **la anotación de GitHub del fallo contiene literalmente 13 caracteres: `CHECK FAILED:`**. Nada más. Ni una línea de error. He comprobado las 15 ejecuciones del workflow de diagnóstico: **todas devuelven el mismo texto vacío**.
- **La causa es un bug del propio workflow**: el script codifica los saltos de línea con `sed 's/\n/%0A/g'`, pero **GNU sed procesa línea por línea y nunca puede casar `\n`** — lo he reproducido en este entorno. Resultado: el `::error::` lleva el texto multilínea crudo, GitHub solo conserva la **primera línea** (`CHECK FAILED:`) como mensaje de la anotación, y los diagnósticos reales se pierden en un log que nadie mira.
- En otras palabras: **tienes un backend que no compila y un CI que no te dice por qué.** Es lo primero que hay que arreglar.

**Arreglo del workflow (sustituye el paso de `check-moon.yml`):**

```yaml
      - name: Compilar el backend Titan (zett check)
        run: |
          set +e
          OUT=$(cargo run --quiet -p titan_cli -- check projects/moon/src/main.titan 2>&1)
          CODE=$?
          set -e
          printf '%s\n' "$OUT"                      # al log
          { echo '### `zett check`'; echo '```'; printf '%s\n' "$OUT"; echo '```'; } >> "$GITHUB_STEP_SUMMARY"
          if [ "$CODE" -ne 0 ]; then
            ONE=$(printf '%s' "$OUT" | tr -d '\r' | awk 'NF' | paste -sd'‖' - | cut -c1-3000)
            echo "::error title=ZETT_CHECK_FAILED::$ONE"   # una sola línea
          fi
          exit "$CODE"
```

Con eso, el error completo aparece en la pestaña **Summary** del job y en la anotación.

**Para ver el error hoy mismo, sin esperar a CI:**
```bash
# En tu teléfono (Termux), con el paquete instalado:
pkg install zett && zett check projects/moon/src/main.titan
# En una máquina con Rust:
cargo run -p titan_cli -- check projects/moon/src/main.titan
```
Ojo: el chequeo **compila el proyecto completo siguiendo los `import`**, así que el fallo puede estar en cualquiera de los 17 módulos, no solo en `main.titan`. Y como el error es del *typechecker/codegen*, es plausible que sea un puñado de sitios concretos (el historial de commits muestra que ya se pelearon con esto: *"fix: let mut for reassignments"*, *"no Bool vs Nil comparisons"*, *"Fix Moon backend compilation for Render"*).

### 4.5 Frontend (React)

- 11 vistas: Auth, Reset, Feed (infinito + pestañas), Explore (búsqueda + hashtags), Profile, User, Post (comentarios), Messages (chat WS con reacciones y leídos), Notifications, Settings (perfil/privacidad/2FA/sesiones/export/borrado), Admin (dashboard/usuarios/reportes/palabras/actividad).
- Cliente HTTP propio con **refresh con rotación y cola**, 401→reintento único, subida por XHR con progreso; WebSocket con **reconexión exponencial, latido y cola de eventos**; `wsUrl()` deriva `wss://` cuando el API es `https://` (correcto).
- Sin tests (no hay framework en `package.json`), aunque sí compila y sus 52 rutas están 100 % alineadas con el backend (verificado).

### 4.6 🟠 Riesgos de producción que debes conocer (Render, plan free)

Tu `render.yaml` usa el plan **free** para API, web y base de datos. Eso implica, según la documentación pública de Render en 2026:

| Hecho | Consecuencia para Moon |
|---|---|
| La **Postgres free expira 30 días después de crearla** (tras 14 días de gracia, se borra con todo dentro) | Tu red social **pierde todos los datos cada mes** salvo que migres a un plan pago o a Neon/Supabase (free sin caducidad). Hay que planificarlo desde ya. |
| El **filesystem es efímero**; no se pueden montar discos en free | Moon guarda las imágenes en **`uploads/` local** (lo confirma `h_media.titan` y tu propio `STATUS.md` §5). **Cada redeploy, reinicio o spin-down borra todas las fotos subidas.** El SPEC preveía Cloudinary; el código no lo usa. Es el fallo de datos más probable que verás. |
| Los servicios web free **se duermen a los 15 min** y tardan ~1 min en despertar | El primer request del día tarda un minuto; los WebSocket se cortan (el cliente reconecta, bien) y la sensación de "app muerta" es inevitable en demostraciones. |
| **750 horas/mes** por workspace y **1 hub WS por instancia** | Si el hub se reinicia, presence/typing se pierde (los no-leídos sí se recalculan en DB). Multi-instancia necesitaría pub/sub (Redis) — ya está en tu plan v2. |
| Sin SMTP configurado | Los códigos de 2FA/recuperación salen **en los logs** (documentado en `DEPLOY.md`); funciona para probar, no para usuarios reales. |
| `Dockerfile` fija **TITAN v1.0.0 (21-ago)** | Es un binario de hace ~3 semanas. Entre v1.0.0 y `main` hay **50 commits**, con arreglos en `server_mod.rs`, `process_mod.rs`, `redis_mod.rs`, `sqlite` y `titan_vm` — justo el módulo `std::server` del que depende Moon. **Sube el pin a v1.0.26** (o mejor, al release limpio nuevo). Nota positiva: comprobé por hash que `native.rs` y `typechecker/lib.rs` en v1.0.0 son **idénticos** a `main`, así que la superficie `std::*` que usa Moon no cambia. |

### 4.7 Lo que existe pero **no está en `main`** (sorpresa importante)

En `main` solo están `projects/moon/{SPEC,STATUS,DEPLOY}.md` y `src/*.titan`, más `frontend/`. En la rama de diagnóstico `arena/01a040eb-aio` hay trabajo de Moon que **nunca se mergeó**:

- **`projects/moon/test/e2e.mjs` (467 líneas)** — tu suite E2E (registro → login → posts → DM → 2FA)… fuera de `main`.
- **`projects/moon/ops/`** — `setup-local.sh`, `start-api.sh`, `reset-db.sh`, `fetch-zett.sh`, `db.mjs` (cluster Postgres local en Node).
- **`projects/moon/LOCAL.md` (197 líneas)** — el flujo de desarrollo local.

Traducción: **Moon tiene tests end-to-end y un entorno local reproducible, pero viven en una rama de trabajo que además está contaminada con scripts de diagnóstico.** Merece la pena rescatar esos tres archivos a `main` (sin los `build.rs` de prueba) y conectar el E2E a CI.

---

## 5. Hallazgos transversales (higiene y seguridad del repo)

1. **Basura de sesión commiteada en `main`:** `debug.txt`, `debug2.txt`, `error.txt`, `probe.txt`, `selftest.txt` y `diag/{error,probe}.txt`. Contienen rutas del sandbox, nombres de host, y cosas como `GITHUB_TOKEN=<set, len=24>`. No hay secretos en claro, pero es ruido y filtra cómo se trabaja. Además el commit `05e7ee3` ("cleanup: quitar probe.txt/error.txt escritos por error desde el sandbox") demuestra que **ya se borraron una vez y volvieron**. → Añade esos nombres a `.gitignore` (o mejor: que las sesiones escriban en `/tmp`, nunca dentro del repo).
2. **Trabajo de diagnóstico dentro de `main`:** `diag-titan.yml` es un workflow *de diagnóstico* en `main` (con anotación de errores) y arrastra 15/15 ejecuciones en rojo. Debería vivir en una rama, no en la rama principal: un `main` con dos workflows rojos permanentes (Moon + Diag) entrena a ignorar el rojo — exactamente lo que tu propio `docs/VALIDATION.md` dice que hay que evitar.
3. **32 ramas remotas, 23 de ellas `arena/*`** y 61 tags. Hay ramas `tools-*` con artefactos falsos (`tools-zett-x86_64` → `echo zett-fake-v2`, `tools-rustc-diag`, `tools-zett-probe-v14`). Limpieza pendiente.
4. **Un PR cerrado sin merge** (#4, "TIERRAS DE TITAN + TitanForge: juegos e infraestructura") — si ese trabajo te interesa, está en riesgo de perderse entre la maraña de ramas.
5. **Buena práctica que sí existe:** `docs/VALIDATION.md` con reglas explícitas contra tests que dependen del reloj y `scripts/flaky-check.sh`. Eso está por encima de la media.

---

## 6. Magnitud comparada y madurez

| Dimensión | TITAN/Zett | Moon |
|---|---|---|
| Código | ✅ Grande (65k Rust) | ✅ Medio-grande (8.2k entre TITAN y React) |
| Arquitectura | ✅ Clara, en capas, con docs | ✅ Clara (handlers/módulos/vistas) y SPEC excelente |
| Pruebas | ✅ 637 en CI, 3 SO + ARM | ❌ 0 en `main`; E2E existe en otra rama |
| CI | ✅ Verde en `main` | ❌ Rojo desde su creación, sin diagnóstico visible |
| Ejecución real probada | ✅ (CI compila y testea de verdad) | ❌ **Nunca ejecutado** |
| Versionado/releases | ⚠️ Inconsistente, tags fuera de `main` | ⚠️ Sin versión propia |
| Datos persistentes | n/a | ⚠️ Postgres free caduca a los 30 días; imágenes en disco efímero |
| Documentación | ✅ 28 docs | ✅ SPEC + STATUS + DEPLOY (mejor que el promedio) |
| Riesgo principal | Trazabilidad de releases y "mirror" con token | Que el backend no compile y nadie lo sepa |

**Valoración honesta de esfuerzo:** por volumen, TITAN equivale a **meses de trabajo de un equipo pequeño** (y el repo lo confirma: creado el 3-jun, 45 releases, 37 fases documentadas). Moon es una aplicación que, *si compila y arranca*, está por encima del 90 % de los proyectos "de portafolio": 69 endpoints, 2FA, refresh rotativo, rate limiting, moderación, auditoría, tiempo real. Pero hoy su estado real es **"código escrito, no producto funcionando"**.

---

## 7. Plan priorizado

### P0 — Desbloquear (esta semana)
1. **Ver el error real de `zett check`** (comando de §4.4 en Termux o local) y **arreglar Moon hasta que compile**. Nada más importa hasta que esto pase.
2. **Parchear el workflow `Moon checks`** con el snippet de §4.4 para que el fallo sea legible para siempre.
3. **Levantar el backend localmente** con Postgres (usa el `ops/setup-local.sh` que ya tienes en la rama de diagnóstico) y correr el **E2E de 467 líneas**. Ese es el hito que convierte Moon en "producto".
4. **Cortar un release limpio de TITAN desde `main`** (v1.0.29) y **apuntar el `Dockerfile` de Moon a él** en vez de v1.0.0.

### P1 — Ordenar (dos semanas)
5. Limpiar `main`: borrar `debug*.txt`, `error.txt`, `probe.txt`, `selftest.txt`, `diag/`; sacar `diag-titan.yml` de `main`; ignorar esos patrones.
6. Traer a `main` `projects/moon/{LOCAL.md, ops/, test/e2e.mjs}` (sin los `build.rs` de diagnóstico) y **añadir el E2E a CI** con un Postgres de servicio.
7. Unificar el versionado: que `titan version` imprima la versión real del release (del tag), y alinear CHANGELOG (`1.0.x`) con los tags.
8. Retirar el mecanismo "mirror" (`crates/titan_parser/build.rs` + `scripts/zett-mirror.sh` + `cargo-diag-wrapper*`) de cualquier rama desde la que se etiquete; archivar ramas `tools-*`.

### P2 — Endurecer para usuarios reales
9. **Imágenes:** pasar de `uploads/` local a almacenamiento externo (Cloudinary/S3/Backblaze) — si no, **pierdes todas las fotos en el próximo redeploy**.
10. **Base de datos:** mover el free de Render a **Neon o Supabase** (free sin caducidad) o presupuestar el pago; automatizar `pg_dump` antes del día 30.
11. Tests en el frontend (aunque sea smoke con Vitest) y **un test de humo del backend por endpoint** en CI.
12. Multi-instancia del hub WS con Redis pub/sub (ya está en tu v2) y vigilancia de las cuotas (750 h/mes).

---

## 8. Cómo reproducir estas cifras

```bash
# Tamaño por crate
for d in crates/*/; do echo -n "$d "; find $d -name '*.rs' -exec cat {} + | wc -l; done

# Nativas registradas y namespaces (758 en 72)
grep -c 'native!(' crates/titan_stdlib/src/native.rs

# Verificador propio del repo (758 únicas / 837 llamadas)
python3 verify_phase34.py

# Verificador de Moon que escribí para este informe
# (1.076 llamadas std::* contra los DOS registros nativos + contrato de rutas)
python3 verify_moon.py

# Frontend de Moon (build real)
cd frontend && npm ci && npm run build

# Tests declarados
grep -rc '#\[test\]' crates --include=*.rs | awk -F: '{s+=$2} END {print s}'   # 637

# Estado de CI / releases
gh run list -L 10
gh release list -L 5
```

---

## 9. Anexo — cifras crudas

- Rust: **108 archivos / 64.937 líneas / 19 crates**; stdlib 77 archivos / 27.661 líneas.
- Nativas: **758 (stdlib, únicas) + 122 (typechecker) = 880**, en **78 namespaces**, sin solapamientos. Opcodes: **170**.
- Tests Rust: **637** (330 stdlib, 125 vm, 13 codegen, 10 pkg, 6 lsp, 3 dap).
- Ejemplos: **59** programas / 5.169 líneas. Docs: **28** archivos / ≈3.800 líneas. CHANGELOG: 37 versiones (0.2.0 → 0.40.0).
- Moon: **17 módulos / 4.582 líneas**; **69 endpoints**; **21 tablas / 14 migraciones / 20 índices**; **1.076 llamadas `std::*` a 85 funciones**; frontend **24 archivos / 3.669 líneas / 11 vistas**; build 210,55 KB JS (64,17 KB gzip).
- GitHub: **45 releases** (última con binarios **v1.0.26**, 11-sep), **61 tags**, **32 ramas** (23 `arena/*`), **11 PRs** (10 mergeados).
- CI (histórico): `ci.yml` 160/200 ✅, `cross-platform` 82/113 ✅, `termux-arm` 149/200 ✅, `termux-aarch64` 80/118 ✅, **`Moon checks` 0/16 ❌**, **`Diag titan` 0/15 ❌**.

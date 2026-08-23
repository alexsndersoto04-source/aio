# 20 ideas potentes para construir con TITAN

Cada idea usa **solo APIs que existen hoy** en este repo (verificadas contra
`crates/titan_stdlib/src/native.rs` y `crates/titan_typechecker/src/lib.rs`).
Para cada una: qué es, por qué TITAN es buena elección, qué módulos usa, cómo
arrancar y qué gotcha te va a morder.

Leyenda de dificultad: 🟢 fin de semana · 🟡 una o dos semanas · 🔴 proyecto serio

Índice rápido:

| # | Proyecto | Nivel | Núcleo |
|---|---|---|---|
| 1 | Monitor de servidor con dashboard web | 🟢 | procfs + server + plot |
| 2 | API REST con JWT y SQLite | 🟡 | server + sqlite + jwt + argon2 |
| 3 | Buscador semántico local | 🟡 | onnx + tokenize + vector + kv |
| 4 | Gestor de secretos cifrado | 🟢 | crypto + argon2 + kv |
| 5 | Motor de facturación PDF | 🟡 | pdf + sqlite + qrcode |
| 6 | Sonda de uptime distribuida | 🟡 | http_full + metrics + channels |
| 7 | Pipeline ETL con sandbox | 🟡 | csv/json/yaml/xml + stats + db |
| 8 | Bot de automatización Android | 🟢 | termux + kv + datetime |
| 9 | Gateway de API con rate limiting | 🔴 | http router + redis + tls |
| 10 | Cliente Git-like de backups | 🟡 | hash + compress + archive + fswatch |
| 11 | Servidor de chat WebSocket | 🟡 | server + ws + channels |
| 12 | Analizador de logs en streaming | 🟡 | regex + collections + plot |
| 13 | Juego 2D en ventana nativa | 🟡 | window::live_* + input + game + audio |
| 14 | Panel de red y Wi-Fi | 🟢 | wifi + dns + net + term |
| 15 | Generador de sitios estáticos | 🟢 | fs + regex + text + fswatch |
| 16 | Cola de trabajos con reintentos | 🔴 | kv/redis + runtime + async |
| 17 | Visualizador de datos en el navegador | 🟡 | wasm + web (Canvas/WebGL2) |
| 18 | Herramienta de migración de bases | 🟡 | db + sqlite/postgres/mysql |
| 19 | Servidor de imágenes on-the-fly | 🟡 | image + server + cache |
| 20 | Runner de tests y benchmarks | 🔴 | process + runtime + metrics |

---

## 1. Monitor de servidor con dashboard web en vivo 🟢

**Qué:** un `htop` gráfico que se sirve por HTTP. CPU, RAM, swap, discos, red y
top de procesos, con gráficos SVG que se regeneran en cada request y HTML con
auto-refresh.

**Por qué TITAN:** `std::procfs` te da 18 lecturas del sistema sin dependencias,
`std::plot` renderiza SVG en Rust puro y `std::server` levanta HTTP sin
framework. Un binario, cero runtime instalado — corre igual en un VPS que en un
teléfono con Termux.

**Módulos:** `std::procfs::{cpu_usage, cpus, total_memory, used_memory, disks, networks, top_processes, load_average, uptime}` · `std::plot::{bar, multi_line, histogram}` · `std::server` · `std::router`

**Arranque:** `examples/dashboard.titan` ya hace la versión mínima. Extendelo
con historial: guardá muestras en un array circular y graficá con
`std::plot::multi_line` para ver tendencia, no solo el instante.

**Gotcha:** `std::procfs::*` requiere capacidad `Environment` — no corre bajo
`--sandbox`. `std::plot::*` escribe a disco (`Filesystem`), así que el chart va
a un archivo y después lo servís con `std::fs::read_bytes`.

---

## 2. API REST con autenticación JWT y SQLite 🟡

**Qué:** el backend clásico: registro, login, CRUD con permisos, paginación,
respuestas JSON coherentes y errores tipados.

**Por qué TITAN:** tenés Argon2id **y** bcrypt para passwords, JWT HS256/RS256
firmado y verificado, SQLite con transacciones y migraciones, y un router
matchit igual al de axum. Todo compilado a un `.tbc` de tamaño acotado.

**Módulos:** `std::server` · `std::router` · `std::sqlite::{open, execute, query, begin, commit, migrate}` · `std::password::{hash_argon2, verify_argon2}` · `std::jwt::{sign_hs256, verify_hs256}` · `std::uuid::v4` · `std::json` · `std::try::catch`

**Arranque:** `examples/rest/` es un esqueleto completo con `main.titan`,
`db.titan` y `auth.titan` separados por `import`. Copialo y cambiale el modelo.

**Gotcha:** envolvé **cada** handler en `std::try::catch`. Sin eso, un body JSON
malformado mata el proceso entero. Y recordá que los patrones del router llevan
llaves: armalos con concatenación porque el interpolador se las come.

---

## 3. Buscador semántico local, sin nube 🟡

**Qué:** indexás tus documentos, notas o código, generás embeddings con un
modelo ONNX local (MiniLM y similares) y buscás por significado, no por
palabras. Todo offline.

**Por qué TITAN:** poquísimos lenguajes traen tokenizers de HuggingFace,
inferencia ONNX (vía `tract`, Rust puro, sin libtorch), matemática vectorial y
un KV store ACID **en la biblioteca estándar**. Acá es un solo binario.

**Módulos:** `std::tokenize::{load, encode, encode_padded}` · `std::onnx::{load_bert, run_bert_pooled, input_shape}` · `std::vector::{normalize, cosine_similarity, argmax, dot}` · `std::kv::{open, insert, get}` · `std::fs`

**Arranque:** `examples/vector_search.titan` corre la parte de ranking con
vectores a mano (sin descargar modelos, arranca en un segundo). `examples/search.titan`
hace el pipeline completo con MiniLM. Empezá por el primero, cambiale la fuente
de embeddings después.

**Gotcha:** el modelo real necesita RAM. En un teléfono de 3 GB usá la variante
liviana. `std::onnx::run_bert_pooled` espera arrays ya paddeados: mirá
`std::tokenize::encode_padded`.

---

## 4. Gestor de secretos cifrado en disco 🟢

**Qué:** un `pass` propio. Master password derivada con Argon2id, secretos
cifrados con ChaCha20-Poly1305 o AES-GCM, almacenados en un KV transaccional,
con TOTP-like y copia al portapapeles.

**Por qué TITAN:** criptografía moderna auditada (`chacha20poly1305`, `aes-gcm`,
`argon2`) expuesta directo, más `std::kv` con compare-and-swap para que dos
procesos no se pisen. Y podés correr la lógica pura bajo `--sandbox` para
demostrar que no filtra nada por red.

**Módulos:** `std::password::{hash_argon2, verify_argon2}` · `std::crypto::{generate_key_32, generate_nonce, chacha20_seal, chacha20_open, aes_gcm_seal, aes_gcm_open}` · `std::kv` · `std::clipboard` · `std::readline::prompt_secret` · `std::checksum::constant_time_eq`

**Arranque:** `examples/security.titan` muestra las primitivas. El diseño clave:
derivá la clave de 32 bytes del master password, nunca la guardes, y usá un
nonce nuevo por escritura.

**Gotcha:** `std::checksum::*` **no es criptografía** — está documentado así en
`docs/STDLIB.md`. Para comparar tokens usá `constant_time_eq`, para integridad
usá `std::hash::blake3` o HMAC.

---

## 5. Motor de facturación con PDF y QR 🟡

**Qué:** generás facturas en PDF con logo, tablas, totales e IVA, cada una con
un QR de verificación, y las archivás en SQLite con su hash.

**Por qué TITAN:** `std::pdf` construye display lists acotadas y las materializa
con `printpdf` solo al guardar; `std::qrcode` emite PNG/SVG/ASCII. Es un
generador de documentos sin LaTeX, sin headless Chrome, sin Node.

**Módulos:** `std::pdf::{new, add_page, add_text, add_line, add_rect, set_color, save, close}` · `std::qrcode::{to_png, to_svg, save_png}` · `std::sqlite` · `std::datetime::{now, format, to_rfc3339}` · `std::hash::sha256`

**Arranque:** `examples/invoice.titan`. Convertilo en servicio agregándole
`std::server` y un endpoint `POST /facturas` que devuelva el PDF como bytes.

**Gotcha:** después de `std::pdf::close(doc)` el handle muere — llamar
`page_count` sobre él es un error de runtime `unknown handle`. Consultá antes de
cerrar. Y los colores van en floats 0.0–1.0.

---

## 6. Sonda de uptime distribuida con métricas Prometheus 🟡

**Qué:** chequeás N endpoints en paralelo, medís latencia y código de estado,
exponés `/metrics` en formato Prometheus y alertás por email o notificación.

**Por qué TITAN:** `spawn` da concurrencia con threads reales, `std::metrics`
exporta OpenMetrics nativo (contadores, gauges, histogramas con `_count`,
`_sum`, `_min`, `_max`) y `std::runtime::spawn_quota` acota cada sonda para que
una respuesta gigante no tumbe el proceso.

**Módulos:** `std::http_full::{request, get_json}` · `std::metrics::{counter_add, gauge_set, histogram_record, prometheus_export}` · `spawn`/`join_timeout`/`channel`/`select` · `std::email::send_simple` · `std::server`

**Arranque:** un `spawn` por endpoint, resultados por `channel(N)`, y
`select(receptores, timeout)` para consumir a medida que llegan.
`examples/enterprise_metrics.titan` muestra el export.

**Gotcha:** `join` consume el handle **exactamente una vez**; un segundo `join`
da `unknown or already joined task`. Usá `join_timeout` si no querés bloquear
para siempre.

---

## 7. Pipeline ETL multiformato con núcleo sandboxeado 🟡

**Qué:** lee CSV/JSON/YAML/XML, valida, normaliza, agrega estadísticas y
escribe a base de datos o a un reporte gráfico.

**Por qué TITAN:** los cuatro parsers están en la stdlib, `std::stats` hace
media/mediana/cuantiles/desvío en streaming (Welford), y el modelo de
capacidades te deja **probar toda la transformación bajo `--sandbox`**: si un
test pasa sandboxeado, ese código no toca red ni disco. Garantía verificable,
no promesa.

**Módulos:** `std::csv` · `std::json::{parse, pointer, merge, flatten}` · `std::yaml` · `std::xml` · `std::stats::{mean, median, stddev, quantile}` · `std::collections::{counter_from, counter_most_common, set_*, omap_*}` · `std::db` · `std::plot`

**Arranque:** `examples/formats.titan`. Estructurá el proyecto como
`src/extract.titan` / `src/transform.titan` / `src/load.titan` y poné los tests
de `transform` bajo `titan test --sandbox`.

**Gotcha:** `std::json::flatten` devuelve pares `(path, valor)` — ideal para
diffear dos payloads. Y no anotes los parámetros que reciben resultados de
parsers: vienen como `Array`/`Map` genéricos.

---

## 8. Bot de automatización para Android 🟢

**Qué:** un daemon en tu teléfono que reacciona al mundo: batería baja →
notificación, llegaste a casa (GPS) → apagar datos, SMS de cierto número →
webhook, foto periódica con la cámara, lectura de sensores a CSV.

**Por qué TITAN:** `std::termux::*` expone 23 integraciones reales de Android
(batería, sensores, GPS, SMS, cámara, TTS, linterna, vibración, notificaciones,
diálogos, contactos, portapapeles, compartir) y Zett se instala con
`pkg install zett`. Es Tasker programable en un lenguaje tipado.

**Módulos:** `std::termux::{battery_status, sensor_read, sensor_list, location, sms_list, sms_send, notify, toast, tts_speak, camera_photo, torch, vibrate, dialog, share}` · `std::datetime` · `std::kv` · `std::http_full`

**Arranque:** `examples/android.titan`. Necesitás la app **Termux:API** más
`pkg install termux-api`; chequeá con `std::termux::is_available()` antes de
cada llamada.

**Gotcha:** todo `std::termux::*` requiere capacidad `Process` porque hace shell
out a los comandos `termux-*`. Son lentos (cientos de ms): no los pongas en un
loop cerrado, y cacheá lo que no cambia.

---

## 9. API gateway con rate limiting y TLS 🔴

**Qué:** un proxy inverso que autentica, aplica cuotas por cliente, agrega
cabeceras de seguridad y CORS, enruta a backends y registra todo.

**Por qué TITAN:** `std::http` trae piezas de gateway ya hechas —
`rate_limit(clave, límite, ventana)`, `security_headers`, `cors`, `request_id`,
`route_match`, parsing anti-smuggling — y `std::tls` con rustls/WebPKI da TLS
sin OpenSSL. El middleware chain (`middleware`, `after`, `on_error`) está en el
router intrínseco.

**Módulos:** `std::http::{router, route, middleware, after, on_error, dispatch, rate_limit, security_headers, cors, request_id, serve_connection}` · `std::tls::{server_config, accept, read, write}` · `std::redis` (cuotas compartidas) · `std::jwt` · `std::metrics`

**Arranque:** empezá con `std::http::router()` + `std::http::serve_connection`,
que es el camino con middleware. `std::server::*` es la vía más simple sin
cadena de middleware.

**Gotcha:** los presupuestos del runtime son reales — 1.024 handles de red por
defecto. Cerrá conexiones (`std::tls::close`, `std::net::tcp_close`) o te
quedás sin descriptores bajo carga.

---

## 10. Sistema de backups incremental con deduplicación 🟡

**Qué:** watchea directorios, hashea archivos con BLAKE3, guarda solo los
bloques nuevos comprimidos con zstd, y arma snapshots restaurables en tar/zip.

**Por qué TITAN:** BLAKE3 (rapidísimo), zstd y gzip, tar y zip, más un
filesystem watcher real — todo en la stdlib. El resultado es un binario único
que corre igual en tu servidor y en tu teléfono.

**Módulos:** `std::hash::{blake3, blake3_bytes}` · `std::compress::{zstd_encode, zstd_decode, gzip_encode}` · `std::archive::{tar_pack, tar_unpack, zip_pack, zip_list}` · `std::fswatch::{open, watch_once, next_event}` · `std::fs::{list_dir, read_bytes, atomic_write, file_size}` · `std::kv` (índice de bloques) · `std::progress`

**Arranque:** el índice va en `std::kv` — clave = hash del bloque, valor = ruta
del blob. `std::kv::compare_and_swap` te da escrituras idempotentes.

**Gotcha:** usá `std::fs::atomic_write` para el manifiesto, nunca `write_bytes`:
si el proceso muere a mitad, un manifiesto corrupto te arruina todos los
snapshots.

---

## 11. Servidor de chat con WebSockets 🟡

**Qué:** salas, presencia, historial persistido, broadcast; cliente web servido
por el mismo binario.

**Por qué TITAN:** implementación RFC 6455 completa — handshake, masking seguro,
codec incremental, validación de protocolo — más upgrade directo desde el
servidor HTTP (`std::server::upgrade_websocket`). Los canales acotados y `select`
son exactamente lo que necesita un hub de broadcast.

**Módulos:** `std::server::{accept, upgrade_websocket, ws_send_text, ws_recv, ws_close}` · `std::ws::{connect, send_text, receive, decoder, decoder_push, decoder_next}` · `channel`/`select`/`spawn` · `std::sqlite` (historial) · `std::json`

**Arranque:** una tarea por conexión con `spawn`, un canal por cliente, y un
hub central que hace `select` sobre los receptores. `docs/WEBSOCKET.md` tiene el
detalle del codec.

**Gotcha:** el cliente **debe** enmascarar, el servidor **no debe** —
`std::ws::encode(opcode, payload, mask)` te lo deja explícito. Y `channel` tiene
capacidad máxima de 65.536: si el productor va más rápido, `send` bloquea.

---

## 12. Analizador de logs en streaming con alertas 🟡

**Qué:** seguís archivos de log en vivo, extraés campos con regex, contás por
dimensión, detectás picos de error y sacás gráficos de tendencia.

**Por qué TITAN:** regex Unicode, `std::collections::counter_*` (frecuencias y
top-N ya implementados), `std::stats` para percentiles de latencia y
`std::fswatch` para el tail. Es `awk | sort | uniq -c` pero tipado y con
gráficos.

**Módulos:** `std::regex::{captures, find_all, is_match}` · `std::collections::{counter_from, counter_add, counter_most_common, deque_*}` · `std::stats::quantile` · `std::fswatch` · `std::plot::{line, histogram}` · `std::term::print_colored` · `std::datetime::parse`

**Arranque:** un deque acotado (`deque_push_back` + `pop_front`) te da la
ventana deslizante de los últimos N eventos sin que la memoria crezca.

**Gotcha:** compilar la regex en cada línea es caro; `std::regex::is_valid`
primero y reutilizá el patrón como constante. Para p95 de latencia,
`std::stats::quantile(muestras, 0.95)`.

---

## 13. Juego 2D en ventana nativa real 🟡

**Qué:** un arcade — Snake, Breakout, un shooter — en una ventana del sistema
operativo a 60 fps, con teclado, mouse, colisiones y efectos de sonido.

**Por qué TITAN:** `std::window::live_*` abre ventanas reales vía minifb
(X11/Wayland/Win32/Cocoa, Rust puro), `std::input` trae el estado real de
teclado/mouse/multi-touch, `std::game` da el frame loop con delta-time medido y
AABB, y `std::audio` sintetiza ondas y escribe WAV. Un ejemplo real corrió 3.601
frames en un Android de 32 bits.

**Módulos:** `std::window::{live_open, live_poll_events, live_pump, live_is_open, live_set_title, live_close}` · `std::input::{is_key_pressed, mouse_pos, is_mouse_button_pressed, touch_pos}` · `std::game::{init, step, check_collision, fps, shutdown}` · `std::gui::{init, create_container, add_button, add_label, is_clicked, render}` · `std::audio::{sine_wave, square_wave, saw_wave, write_wav, sim_play}`

**Arranque:** `examples/gui_live_window.titan` y `examples/game_engine.titan`.
`std::game::step()` devuelve el delta en segundos: multiplicá toda velocidad por
él o el juego cambia de ritmo según la máquina.

**Gotcha:** requiere capacidad `UserInterface` y una pantalla. En headless las
funciones reportan `-1` honestamente en vez de fingir — chequealo.

---

## 14. Panel de diagnóstico de red y Wi-Fi 🟢

**Qué:** escaneás redes cercanas con señal y canal, resolvés DNS (A, AAAA, MX,
TXT, CNAME, reverse), probás puertos TCP y medís latencia, todo en una TUI.

**Por qué TITAN:** DNS completo con hickory-resolver, TCP crudo, introspección
Wi-Fi vía Termux:API y una TUI con colores, alt-screen, raw mode y lectura de
teclas. Es un multiherramienta de red portátil.

**Módulos:** `std::dns::{resolve, resolve_ipv4, resolve_mx, resolve_txt, reverse}` · `std::net::{tcp_connect, tcp_set_timeout, tcp_close}` · `std::wifi::{scan, connection_info, signal_bars}` · `std::term::{enter_alt_screen, print_colored, move_to, read_key, size}` · `std::procfs::networks`

**Arranque:** `examples/wifi.titan` y `examples/tui.titan`. El escaneo de puertos
en paralelo con `spawn` + `join_timeout` es trivial y baja el tiempo total de
minutos a segundos.

**Gotcha:** siempre `std::term::leave_alt_screen()` y `disable_raw()` al salir, o
dejás la terminal rota. Instalá un handler con `std::signals::install("SIGINT")`
para limpiar en Ctrl+C.

---

## 15. Generador de sitios estáticos con live reload 🟢

**Qué:** convertís Markdown ligero + front-matter YAML en HTML con plantillas,
generás índice y feed, y servís con recarga al detectar cambios.

**Por qué TITAN:** regex + `std::text` (slugify, escape_html, truncate, words,
lines) + YAML + fswatch + servidor HTTP. Cero `node_modules`, arranque
instantáneo, y el binario se lleva a cualquier lado.

**Módulos:** `std::fs::{list_dir, read_text, write_text, create_dir}` · `std::regex::{replace_all, captures}` · `std::text::{slugify, escape_html, truncate, lines, words}` · `std::yaml::parse` · `std::fswatch` · `std::server` · `std::path::{join, stem, extension}`

**Arranque:** empezá con un solo transform (encabezados, links, negritas) y
crecé. `std::text::slugify` ya te da las URLs limpias.

**Gotcha:** `std::text::*` cuenta escalares Unicode, no grafemas — un emoji con
modificador cuenta más de uno. Para truncar títulos usá `std::text::truncate`,
que respeta límites de carácter.

---

## 16. Cola de trabajos persistente con reintentos 🔴

**Qué:** infraestructura tipo Sidekiq: encolás trabajos, N workers los consumen
en paralelo, reintentos con backoff exponencial, dead-letter queue, métricas y
panel de estado.

**Por qué TITAN:** cuotas de memoria **por tarea** (`spawn_quota`) — un trabajo
que se descontrola muere solo con `VmError::MemoryLimit` sin tumbar el proceso.
Sumale `std::async` (retry, retry_backoff, timeout, measure), KV ACID o Redis
para la persistencia, y métricas Prometheus. Ese aislamiento por tarea es difícil
de conseguir en la mayoría de los runtimes.

**Módulos:** `std::runtime::{spawn_quota, active_tasks, allocated_bytes, heap_dump}` · `import std::async` (`retry`, `retry_backoff`, `timeout`, `measure`, `delay`) · `std::kv` o `std::redis::{lpush, lrange, expire}` · `channel`/`select` · `std::metrics` · `std::try::catch`

**Arranque:** `examples/enterprise_runtime.titan` y `stdlib/async.titan`. El
patrón: un canal de trabajos, N tareas consumidoras con cuota, y estado
persistido antes y después de cada intento.

**Gotcha:** la cancelación es cooperativa (se chequea antes de cada
instrucción), así que una nativa bloqueante larga no se interrumpe a la mitad.
Usá timeouts en las nativas que los aceptan.

---

## 17. Visualizador de datos que corre en el navegador 🟡

**Qué:** cargás un dataset, lo procesás y lo dibujás en Canvas 2D o WebGL2 —
compilado a WebAssembly desde el mismo `.titan`.

**Por qué TITAN:** `titan wasm` emite un módulo **autocontenido** (no un wrapper
de la VM) con su propio heap en memoria lineal, más source maps estándar para
debuggear `.titan` en las devtools del navegador. El host JS te da DOM, eventos,
fetch, WebSocket, Canvas 2D, animación y WebGL2.

**Módulos:** `std::web::{query_exists, set_text, set_html, listen, event_*, canvas_fill_rect, canvas_line, canvas_text, animation_start, frame_delta_ms, fetch, webgl_*}` · `std::array` · `std::map` · `std::wasm::heap_*`

**Arranque:** `examples/browser/` tiene `main.titan`, `host.js` e `index.html`
listos. `titan wasm examples/browser/main.titan` y servís la carpeta.

**Gotcha:** el backend WASM es un subconjunto estricto. **No hay rangos**
(`for i in 0..n` no compila: usá `while` con contador), no hay closures ni
`map`/`filter`/`fold`, ni concurrencia, ni nativas de sistema. Diseñá el núcleo
de cómputo con `while` y arrays desde el día uno si querés apuntar a los dos
backends. Lista completa en `docs/SPEC.md` §16.2.

---

## 18. Herramienta de migración entre bases de datos 🟡

**Qué:** copiás esquema y datos entre SQLite, PostgreSQL y MySQL, con
verificación por checksum, migraciones versionadas y reporte de diferencias.

**Por qué TITAN:** los tres motores exponen **la misma forma de API**, y
`std::db::*` acepta cualquier handle indistintamente: escribís la lógica una vez
y funciona con los tres. Con pools, health checks y transacciones reales.

**Módulos:** `std::db::{query, execute, begin, commit, rollback, migrate, ping, close}` · `std::sqlite::*` · `std::postgres::{connect, connect_tls, pool, pool_health}` · `std::mysql::*` · `std::hash::sha256` · `std::progress::bar_new`

**Arranque:** `examples/enterprise_pool.titan`. Copiá por lotes dentro de una
transacción y verificá con un hash de cada lote antes de hacer commit.

**Gotcha:** hay 256 handles de base y 64 conexiones por pool como presupuesto
por defecto. Usá el pool y devolvé las conexiones; no abras una por fila.

---

## 19. Servidor de imágenes con transformaciones on-the-fly 🟡

**Qué:** `GET /img/foto.jpg?w=300&blur=2` y devolvés la imagen redimensionada,
recortada, rotada o en escala de grises, con cache LRU y thumbnails.

**Por qué TITAN:** `std::image` trae 21 operaciones (resize, crop, rotate, flip,
blur, brighten, grayscale, thumbnail, encode) sobre PNG/JPEG/WebP/BMP/GIF, todo
en Rust puro. Sin ImageMagick, sin sidecar, sin binarios externos.

**Módulos:** `std::image::{load, load_bytes, resize, resize_exact, crop, thumbnail, blur, grayscale, rotate90, encode, width, height, close}` · `std::server::respond_bytes` · `std::router` · `std::kv` (cache en disco) · `std::checksum::crc32` (clave de cache) · `std::collections::omap_*` (índice LRU en memoria)

**Arranque:** la clave de cache = CRC-32 de la ruta + parámetros normalizados;
guardá el resultado en `std::kv` para que sobreviva reinicios.
`std::http::parse_query` te parsea el query string.

**Gotcha:** cada `std::image::load` devuelve un handle que hay que `close`. En
un servidor de larga vida, olvidarlo es una fuga garantizada — cerralo en el
mismo bloque, incluso en el camino de error.

---

## 20. Runner de tests y benchmarks para tu equipo 🔴

**Qué:** descubrís suites, las corrés en paralelo con timeouts y aislamiento,
medís tiempo y memoria por caso, detectás flakies repitiendo, y publicás un
reporte HTML con tendencia histórica.

**Por qué TITAN:** `std::process` ejecuta comandos **sin shell** (nada de
inyección) con timeout y drenaje concurrente de pipes; `std::runtime::benchmark`
te da `ns_per_op` y `ops_per_sec` con precisión; `spawn_quota` aísla cada caso; y
`heap_dump` exporta el estado del heap en JSON para diagnosticar un caso que se
descontroló.

**Módulos:** `std::process::{run, run_timeout, run_with_input, spawn, spawn_poll, spawn_kill}` · `std::runtime::{benchmark, allocated_bytes, heap_dump, active_tasks}` · `std::metrics` · `std::plot::multi_line` · `std::sqlite` (histórico) · `std::term::print_colored` · `std::progress`

**Arranque:** `examples/enterprise_benchmark.titan` y `scripts/flaky-check.sh`,
que ya hace detección de flakies en este mismo repo. Guardá cada corrida en
SQLite y graficá la tendencia para cazar regresiones de performance.

**Gotcha:** `std::process::run` **no pasa por shell** — pasá el programa y los
argumentos por separado, no un string con pipes. Si necesitás pipeline de shell
hay `std::process::shell`, pero ahí volvés a tener el riesgo de inyección.

---

## Cómo elegir por dónde empezar

**Si querés resultado visible hoy:** #1 (dashboard), #8 (bot Android), #14
(panel de red) o #15 (sitio estático). Todos son un archivo y una tarde.

**Si querés aprender el lenguaje a fondo:** #2 (API REST) te obliga a pasar por
structs, enums, `Result`, `?`, `try::catch`, módulos e imports — que es
básicamente el 80% de TITAN.

**Si querés mostrar algo que otros lenguajes no hacen fácil:** #3 (IA local sin
dependencias), #13 (ventana nativa desde un teléfono) o #16 (cuotas de memoria
por tarea). Ahí es donde este stack se distingue de verdad.

**Si querés contribuir al lenguaje:** elegí cualquiera y anotá cada vez que el
compilador te rechace algo razonable. `docs/SPEC.md` §18 lista las limitaciones
conocidas; los huecos más útiles hoy son namespaces reales para `mod`, patrones
de tuple/struct en `match`, y rangos en el backend WebAssembly.

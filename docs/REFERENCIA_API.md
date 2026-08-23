# Referencia de la API de TITAN — generada desde el código fuente

> Extraída automáticamente de `crates/titan_stdlib/src/native.rs` (registro de
> nativas) y de `crates/titan_typechecker/src/lib.rs` (built-ins globales e
> intrínsecos con opcode dedicado). Regenerar con:
> `python3 scripts/gen-api-reference.py`.
> Si esta tabla y el compilador difieren, el compilador tiene razón.

- **758 funciones nativas** repartidas en **72 namespaces `std::*`**.
- **122 firmas conocidas directamente por el compilador** (18 built-ins globales + 104 intrínsecos).

**Capacidades.** `None` = función pura, sigue funcionando con
`titan run --sandbox`. `Filesystem`, `Process`, `Network`, `Environment` y
`UserInterface` requieren esa capacidad; si está denegada la llamada falla con
`native function 'f' requires capability 'C'` — nunca se ejecuta en silencio.

**Tipos de handle** (`Sqlite`, `TcpStream`, `Task`, `HttpRouter`, `Postgres`, …)
son opacos: se guardan en variables y se pasan a los intrínsecos que los
aceptan, pero no tienen campos ni representación visible.

Varios módulos devuelven `Int` como handle de recurso (`std::kv::open`,
`std::image::load`, `std::server::start`, `std::router::new`, `std::pdf::new`,
`std::onnx::load`, …). Ese entero es un descriptor: pásalo a las funciones del
mismo namespace y ciérralo cuando termines.

---

## 1. Built-ins globales

Sin prefijo `std::`, compilados a opcodes dedicados. **No se pueden usar como
valores**: pasarlos a otra función da
`unsupported language feature: built-in function values ('map')`.

| Función | Firma |
|---|---|
| `all` | `(any, any) -> bool` |
| `any` | `(any, any) -> bool` |
| `cancel` | `(Task) -> bool` |
| `channel` | `(int) -> (Sender, Receiver)` |
| `filter` | `(any, any) -> [any]` |
| `find` | `(any, any) -> any` |
| `fold` | `(any, any, any) -> any` |
| `join` | `(Task) -> any` |
| `join_timeout` | `(Task, int) -> Option` |
| `len` | `(any) -> int` |
| `map` | `(any, any) -> [any]` |
| `print` | `(any) -> nil` |
| `println` | `(any) -> nil` |
| `recv` | `(Receiver) -> any` |
| `recv_timeout` | `(Receiver, int) -> Option` |
| `select` | `([Receiver], int) -> Option` |
| `send` | `(Sender, any) -> nil` |
| `sort_by` | `(any, any) -> [any]` |

## 2. Intrínsecos `std::*` (opcode dedicado, fuera del registro)

### `std::db`

| Función | Firma |
|---|---|
| `begin` | `(any) -> nil` |
| `close` | `(any) -> bool` |
| `commit` | `(any) -> nil` |
| `execute` | `(any, string, any) -> int` |
| `migrate` | `(any, any) -> int` |
| `ping` | `(any) -> bool` |
| `query` | `(any, string, any) -> [map]` |
| `rollback` | `(any) -> nil` |

### `std::http`

| Función | Firma |
|---|---|
| `after` | `(HttpRouter, fn(map) -> map) -> nil` |
| `dispatch` | `(HttpRouter, map) -> any` |
| `middleware` | `(HttpRouter, fn(map) -> map) -> nil` |
| `on_error` | `(HttpRouter, fn(map, map) -> map) -> nil` |
| `route` | `(HttpRouter, string, string, fn(map) -> any) -> nil` |
| `router` | `() -> HttpRouter` |
| `serve_connection` | `(TcpListener, fn(map) -> map, int) -> string` |

### `std::mysql`

| Función | Firma |
|---|---|
| `acquire` | `(MysqlPool, int) -> Option` |
| `begin` | `(Mysql) -> nil` |
| `close` | `(Mysql) -> bool` |
| `commit` | `(Mysql) -> nil` |
| `connect` | `(string) -> Mysql` |
| `execute` | `(Mysql, string, any) -> int` |
| `last_insert_id` | `(Mysql) -> int` |
| `migrate` | `(Mysql, any) -> int` |
| `ping` | `(Mysql) -> bool` |
| `pool` | `(string, int) -> MysqlPool` |
| `pool_close` | `(MysqlPool) -> nil` |
| `pool_health` | `(MysqlPool, int) -> bool` |
| `pool_stats` | `(MysqlPool) -> map` |
| `query` | `(Mysql, string, any) -> [map]` |
| `rollback` | `(Mysql) -> nil` |

### `std::net`

| Función | Firma |
|---|---|
| `tcp_accept` | `(TcpListener) -> (TcpStream, string)` |
| `tcp_close` | `(any) -> bool` |
| `tcp_connect` | `(string) -> TcpStream` |
| `tcp_listen` | `(string) -> TcpListener` |
| `tcp_local_addr` | `(TcpListener) -> string` |
| `tcp_read` | `(TcpStream, int) -> bytes` |
| `tcp_set_timeout` | `(TcpStream, int) -> nil` |
| `tcp_write` | `(TcpStream, bytes) -> int` |

### `std::postgres`

| Función | Firma |
|---|---|
| `acquire` | `(PostgresPool, int) -> Option` |
| `begin` | `(Postgres) -> nil` |
| `cancel` | `(Postgres) -> nil` |
| `close` | `(Postgres) -> bool` |
| `commit` | `(Postgres) -> nil` |
| `connect` | `(string) -> Postgres` |
| `connect_tls` | `(string) -> Postgres` |
| `execute` | `(Postgres, string, any) -> int` |
| `migrate` | `(Postgres, any) -> int` |
| `ping` | `(Postgres) -> bool` |
| `pool` | `(string, int, bool) -> PostgresPool` |
| `pool_close` | `(PostgresPool) -> nil` |
| `pool_health` | `(PostgresPool, int) -> bool` |
| `pool_stats` | `(PostgresPool) -> map` |
| `query` | `(Postgres, string, any) -> [map]` |
| `rollback` | `(Postgres) -> nil` |

### `std::runtime`

| Función | Firma |
|---|---|
| `active_tasks` | `() -> int` |
| `allocated_bytes` | `() -> int` |
| `benchmark` | `(int, fn() -> any) -> map` |
| `fast_path_enabled` | `() -> bool` |
| `gc_collect` | `() -> int` |
| `gc_live_count` | `() -> int` |
| `gc_set_threshold` | `(int) -> nil` |
| `gc_threshold` | `() -> int` |
| `heap_dump` | `(string) -> bool` |
| `memory_limit` | `() -> int` |
| `optimize_level` | `() -> int` |
| `spawn_quota` | `(int, fn() -> any) -> Task` |

### `std::server`

| Función | Firma |
|---|---|
| `control` | `(int) -> ServerControl` |
| `health_response` | `(ServerControl) -> map` |
| `release` | `(ServerControl) -> bool` |
| `shutdown` | `(ServerControl) -> nil` |
| `stats` | `(ServerControl) -> map` |
| `try_acquire` | `(ServerControl) -> bool` |

### `std::sqlite`

| Función | Firma |
|---|---|
| `acquire` | `(SqlitePool, int) -> Option` |
| `begin` | `(Sqlite) -> nil` |
| `close` | `(Sqlite) -> bool` |
| `commit` | `(Sqlite) -> nil` |
| `execute` | `(Sqlite, string, any) -> int` |
| `last_insert_id` | `(Sqlite) -> int` |
| `memory` | `() -> Sqlite` |
| `migrate` | `(Sqlite, any) -> int` |
| `open` | `(string) -> Sqlite` |
| `ping` | `(Sqlite) -> bool` |
| `pool` | `(string, int) -> SqlitePool` |
| `pool_close` | `(SqlitePool) -> nil` |
| `pool_health` | `(SqlitePool, int) -> bool` |
| `pool_stats` | `(SqlitePool) -> map` |
| `query` | `(Sqlite, string, any) -> [map]` |
| `rollback` | `(Sqlite) -> nil` |

### `std::tls`

| Función | Firma |
|---|---|
| `accept` | `(TcpListener, TlsServerConfig) -> (TlsStream, string)` |
| `close` | `(TlsStream) -> bool` |
| `connect` | `(string, string) -> TlsStream` |
| `read` | `(TlsStream, int) -> bytes` |
| `server_config` | `(string, string) -> TlsServerConfig` |
| `write` | `(TlsStream, bytes) -> int` |

### `std::ws`

| Función | Firma |
|---|---|
| `attach_tcp` | `(TcpStream, bool, int) -> WebSocket` |
| `attach_tls` | `(TlsStream, bool, int) -> WebSocket` |
| `close` | `(WebSocket, int, string) -> nil` |
| `connect` | `(string, string, int) -> WebSocket` |
| `decoder` | `(int) -> WebSocketDecoder` |
| `decoder_next` | `(WebSocketDecoder, bool) -> Option` |
| `decoder_push` | `(WebSocketDecoder, bytes) -> nil` |
| `receive` | `(WebSocket) -> any` |
| `send_binary` | `(WebSocket, bytes) -> nil` |
| `send_text` | `(WebSocket, string) -> nil` |

---

## 3. Registro de nativas `std::*`

**Índice:** [`archive`](#stdarchive--5-funciones) · [`array`](#stdarray--7-funciones) · [`audio`](#stdaudio--23-funciones) · [`bytes`](#stdbytes--7-funciones) · [`checksum`](#stdchecksum--3-funciones) · [`clipboard`](#stdclipboard--2-funciones) · [`collections`](#stdcollections--57-funciones) · [`compress`](#stdcompress--8-funciones) · [`crypto`](#stdcrypto--10-funciones) · [`csv`](#stdcsv--2-funciones) · [`datetime`](#stddatetime--49-funciones) · [`dirs`](#stddirs--18-funciones) · [`dns`](#stddns--7-funciones) · [`email`](#stdemail--3-funciones) · [`encoding`](#stdencoding--8-funciones) · [`env`](#stdenv--3-funciones) · [`freestanding`](#stdfreestanding--6-funciones) · [`freestanding_cpu`](#stdfreestandingcpu--7-funciones) · [`freestanding_memory`](#stdfreestandingmemory--7-funciones) · [`freestanding_mmio`](#stdfreestandingmmio--7-funciones) · [`fs`](#stdfs--16-funciones) · [`fswatch`](#stdfswatch--4-funciones) · [`game`](#stdgame--5-funciones) · [`gui`](#stdgui--11-funciones) · [`hash`](#stdhash--11-funciones) · [`http`](#stdhttp--13-funciones) · [`http_full`](#stdhttpfull--4-funciones) · [`image`](#stdimage--21-funciones) · [`input`](#stdinput--8-funciones) · [`json`](#stdjson--6-funciones) · [`jwt`](#stdjwt--5-funciones) · [`kv`](#stdkv--17-funciones) · [`map`](#stdmap--9-funciones) · [`math`](#stdmath--14-funciones) · [`metrics`](#stdmetrics--8-funciones) · [`mobile`](#stdmobile--3-funciones) · [`net`](#stdnet--1-funciones) · [`notify`](#stdnotify--1-funciones) · [`onnx`](#stdonnx--14-funciones) · [`password`](#stdpassword--4-funciones) · [`path`](#stdpath--8-funciones) · [`pdf`](#stdpdf--9-funciones) · [`plot`](#stdplot--5-funciones) · [`process`](#stdprocess--22-funciones) · [`procfs`](#stdprocfs--18-funciones) · [`progress`](#stdprogress--7-funciones) · [`qrcode`](#stdqrcode--5-funciones) · [`random`](#stdrandom--8-funciones) · [`readline`](#stdreadline--4-funciones) · [`redis`](#stdredis--21-funciones) · [`regex`](#stdregex--7-funciones) · [`router`](#stdrouter--5-funciones) · [`server`](#stdserver--23-funciones) · [`signals`](#stdsignals--3-funciones) · [`stats`](#stdstats--5-funciones) · [`term`](#stdterm--15-funciones) · [`termux`](#stdtermux--23-funciones) · [`testing`](#stdtesting--2-funciones) · [`text`](#stdtext--21-funciones) · [`time`](#stdtime--3-funciones) · [`tokenize`](#stdtokenize--10-funciones) · [`try`](#stdtry--1-funciones) · [`url`](#stdurl--10-funciones) · [`uuid`](#stduuid--5-funciones) · [`vector`](#stdvector--8-funciones) · [`wasm`](#stdwasm--14-funciones) · [`web`](#stdweb--53-funciones) · [`wifi`](#stdwifi--4-funciones) · [`window`](#stdwindow--12-funciones) · [`ws`](#stdws--6-funciones) · [`xml`](#stdxml--4-funciones) · [`yaml`](#stdyaml--3-funciones)

### `std::archive` — 5 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `tar_pack` | `Array` | `Bytes` | None |
| `tar_unpack` | `Bytes` | `Array` | None |
| `zip_list` | `Bytes` | `Array` | None |
| `zip_pack` | `Array` | `Bytes` | None |
| `zip_unpack` | `Bytes` | `Array` | None |

### `std::array` — 7 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `concat` | `Array, Array` | `Array` | None |
| `filled` | `Int, Any` | `Array` | None |
| `pop` | `Array` | `Array` | None |
| `push` | `Array, Any` | `Array` | None |
| `range` | `Int, Int` | `Array` | None |
| `set` | `Array, Int, Any` | `Array` | None |
| `slice` | `Array, Int, Int` | `Array` | None |

### `std::audio` — 23 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `encode_wav` | `Array, Int, Int` | `Bytes` | None |
| `info` | `—` | `String` | Process |
| `is_termux_media_available` | `—` | `Bool` | Environment |
| `pause` | `—` | `String` | Process |
| `play` | `String` | `String` | Process |
| `read_wav` | `String` | `Map` | Filesystem |
| `read_wav_bytes` | `Bytes` | `Map` | None |
| `record_info` | `—` | `String` | Process |
| `record_start` | `String, Int` | `String` | Process |
| `record_stop` | `—` | `String` | Process |
| `resume` | `—` | `String` | Process |
| `saw_wave` | `Float, Int, Int, Float` | `Array` | None |
| `sim_init` | `—` | `Bool` | None |
| `sim_load_wave` | `Float, Int` | `Int` | None |
| `sim_play` | `Int, Bool` | `Bool` | None |
| `sim_sample_count` | `Int` | `Int` | None |
| `sim_set_volume` | `Int, Float` | `Bool` | None |
| `sim_stop` | `Int` | `Bool` | None |
| `sine_wave` | `Float, Int, Int, Float` | `Array` | None |
| `square_wave` | `Float, Int, Int, Float` | `Array` | None |
| `stop` | `—` | `String` | Process |
| `white_noise` | `Int, Int, Float` | `Array` | None |
| `write_wav` | `String, Array, Int, Int` | `Nil` | Filesystem |

### `std::bytes` — 7 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `concat` | `Bytes, Bytes` | `Bytes` | None |
| `from_array` | `Array` | `Bytes` | None |
| `length` | `Bytes` | `Int` | None |
| `read_u32_le` | `Bytes, Int` | `Int` | None |
| `slice` | `Bytes, Int, Int` | `Bytes` | None |
| `to_array` | `Bytes` | `Array` | None |
| `write_u32_le` | `Int` | `Bytes` | None |

### `std::checksum` — 3 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `constant_time_eq` | `Bytes, Bytes` | `Bool` | None |
| `crc32` | `Bytes` | `Int` | None |
| `fnv1a64` | `Bytes` | `Int` | None |

### `std::clipboard` — 2 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `get_text` | `—` | `String` | UserInterface |
| `set_text` | `String` | `Bool` | UserInterface |

### `std::collections` — 57 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `chunk` | `Array, Int` | `Array` | None |
| `contains` | `Array, Any` | `Bool` | None |
| `counter_add` | `Int, String, Int` | `Nil` | None |
| `counter_count` | `Int, String` | `Int` | None |
| `counter_drop` | `Int` | `Bool` | None |
| `counter_from` | `Array` | `Int` | None |
| `counter_most_common` | `Int, Int` | `Array` | None |
| `counter_total` | `Int` | `Int` | None |
| `deduplicate` | `Array` | `Array` | None |
| `deque_drop` | `Int` | `Bool` | None |
| `deque_len` | `Int` | `Int` | None |
| `deque_new` | `—` | `Int` | None |
| `deque_pop_back` | `Int` | `Option` | None |
| `deque_pop_front` | `Int` | `Option` | None |
| `deque_push_back` | `Int, String` | `Nil` | None |
| `deque_push_front` | `Int, String` | `Nil` | None |
| `deque_to_array` | `Int` | `Array` | None |
| `graph_add_edge` | `Int, String, String, Int` | `Nil` | None |
| `graph_add_node` | `Int, String` | `Nil` | None |
| `graph_bfs` | `Int, String` | `Array` | None |
| `graph_dfs` | `Int, String` | `Array` | None |
| `graph_drop` | `Int` | `Bool` | None |
| `graph_has_cycle` | `Int` | `Bool` | None |
| `graph_neighbors` | `Int, String` | `Array` | None |
| `graph_new` | `Bool` | `Int` | None |
| `graph_nodes` | `Int` | `Array` | None |
| `graph_shortest_path` | `Int, String, String` | `Array` | None |
| `graph_topological_sort` | `Int` | `Array` | None |
| `join` | `Array, String` | `String` | None |
| `length` | `Any` | `Int` | None |
| `omap_drop` | `Int` | `Bool` | None |
| `omap_get` | `Int, String` | `Option` | None |
| `omap_insert` | `Int, String, Any` | `Nil` | None |
| `omap_keys` | `Int` | `Array` | None |
| `omap_len` | `Int` | `Int` | None |
| `omap_new` | `—` | `Int` | None |
| `omap_remove` | `Int, String` | `Bool` | None |
| `pq_drop` | `Int` | `Bool` | None |
| `pq_len` | `Int` | `Int` | None |
| `pq_new_max` | `—` | `Int` | None |
| `pq_new_min` | `—` | `Int` | None |
| `pq_peek` | `Int` | `Option` | None |
| `pq_pop` | `Int` | `Option` | None |
| `pq_push` | `Int, String, Int` | `Nil` | None |
| `reverse` | `Array` | `Array` | None |
| `set_add` | `Int, String` | `Bool` | None |
| `set_contains` | `Int, String` | `Bool` | None |
| `set_difference` | `Int, Int` | `Int` | None |
| `set_drop` | `Int` | `Bool` | None |
| `set_from` | `Array` | `Int` | None |
| `set_intersect` | `Int, Int` | `Int` | None |
| `set_is_subset` | `Int, Int` | `Bool` | None |
| `set_len` | `Int` | `Int` | None |
| `set_new` | `—` | `Int` | None |
| `set_remove` | `Int, String` | `Bool` | None |
| `set_to_array` | `Int` | `Array` | None |
| `set_union` | `Int, Int` | `Int` | None |

### `std::compress` — 8 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `deflate_decode` | `Bytes` | `Bytes` | None |
| `deflate_encode` | `Bytes, Int` | `Bytes` | None |
| `gzip_decode` | `Bytes` | `Bytes` | None |
| `gzip_encode` | `Bytes, Int` | `Bytes` | None |
| `zlib_decode` | `Bytes` | `Bytes` | None |
| `zlib_encode` | `Bytes, Int` | `Bytes` | None |
| `zstd_decode` | `Bytes` | `Bytes` | None |
| `zstd_encode` | `Bytes, Int` | `Bytes` | None |

### `std::crypto` — 10 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `aes_gcm_decrypt` | `Bytes, Bytes, Bytes, Bytes` | `Bytes` | None |
| `aes_gcm_encrypt` | `Bytes, Bytes, Bytes, Bytes` | `Bytes` | None |
| `aes_gcm_open` | `Bytes, Bytes, Bytes` | `Bytes` | None |
| `aes_gcm_seal` | `Bytes, Bytes, Bytes` | `Bytes` | None |
| `chacha20_decrypt` | `Bytes, Bytes, Bytes, Bytes` | `Bytes` | None |
| `chacha20_encrypt` | `Bytes, Bytes, Bytes, Bytes` | `Bytes` | None |
| `chacha20_open` | `Bytes, Bytes, Bytes` | `Bytes` | None |
| `chacha20_seal` | `Bytes, Bytes, Bytes` | `Bytes` | None |
| `generate_key_32` | `—` | `Bytes` | None |
| `generate_nonce` | `—` | `Bytes` | None |

### `std::csv` — 2 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `parse` | `String` | `Array` | None |
| `serialize` | `Array` | `String` | None |

### `std::datetime` — 49 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `add_days` | `Int, Int` | `Int` | None |
| `add_days_ext` | `Int, Int` | `Int` | None |
| `add_hours` | `Int, Int` | `Int` | None |
| `add_minutes` | `Int, Int` | `Int` | None |
| `add_months` | `Int, Int` | `Int` | None |
| `add_seconds` | `Int, Int` | `Int` | None |
| `add_weeks` | `Int, Int` | `Int` | None |
| `add_years` | `Int, Int` | `Int` | None |
| `business_days_between` | `Int, Int` | `Int` | None |
| `common_timezones` | `—` | `Array` | None |
| `day` | `Int` | `Int` | None |
| `day_of_week` | `Int` | `Int` | None |
| `day_of_year` | `Int` | `Int` | None |
| `days_in_month` | `Int, Int` | `Int` | None |
| `diff_days` | `Int, Int` | `Int` | None |
| `diff_hours` | `Int, Int` | `Int` | None |
| `diff_minutes` | `Int, Int` | `Int` | None |
| `diff_seconds` | `Int, Int` | `Int` | None |
| `format` | `Int, String` | `String` | None |
| `format_offset` | `Int, String, Int` | `String` | None |
| `from_iso` | `String` | `Int` | None |
| `from_ymd` | `Int, Int, Int` | `Int` | None |
| `from_ymd_hms` | `Int, Int, Int, Int, Int, Int` | `Int` | None |
| `hour` | `Int` | `Int` | None |
| `humanize` | `Int, Int` | `String` | None |
| `is_after` | `Int, Int` | `Bool` | None |
| `is_before` | `Int, Int` | `Bool` | None |
| `is_leap_year` | `Int` | `Bool` | None |
| `is_same_day` | `Int, Int` | `Bool` | None |
| `is_weekend` | `Int` | `Bool` | None |
| `minute` | `Int` | `Int` | None |
| `month` | `Int` | `Int` | None |
| `next_weekday` | `Int, Int` | `Int` | None |
| `now` | `—` | `Int` | None |
| `now_iso` | `—` | `String` | None |
| `parse` | `String, String` | `Int` | None |
| `parse_rfc3339` | `String` | `Int` | None |
| `quarter` | `Int` | `Int` | None |
| `range_ext` | `Int, Int, Int` | `Array` | None |
| `second` | `Int` | `Int` | None |
| `timezone_offset_seconds` | `Int, String` | `Int` | None |
| `to_iso` | `Int` | `String` | None |
| `to_rfc2822` | `Int` | `String` | None |
| `to_rfc3339` | `Int` | `String` | None |
| `to_timezone` | `Int, String` | `String` | None |
| `utc_ymd_hms` | `Int, Int, Int, Int, Int, Int` | `Int` | None |
| `week_of_year` | `Int` | `Int` | None |
| `weekday` | `Int` | `Int` | None |
| `year` | `Int` | `Int` | None |

### `std::dirs` — 18 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `cache` | `—` | `String` | Environment |
| `config` | `—` | `String` | Environment |
| `current` | `—` | `String` | Environment |
| `data` | `—` | `String` | Environment |
| `data_local` | `—` | `String` | Environment |
| `desktop` | `—` | `String` | Environment |
| `documents` | `—` | `String` | Environment |
| `downloads` | `—` | `String` | Environment |
| `executable` | `—` | `String` | Environment |
| `home` | `—` | `String` | Environment |
| `music` | `—` | `String` | Environment |
| `pictures` | `—` | `String` | Environment |
| `preference` | `—` | `String` | Environment |
| `public` | `—` | `String` | Environment |
| `runtime` | `—` | `String` | Environment |
| `state` | `—` | `String` | Environment |
| `temp` | `—` | `String` | Environment |
| `videos` | `—` | `String` | Environment |

### `std::dns` — 7 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `resolve` | `String` | `Array` | Network |
| `resolve_cname` | `String` | `Array` | Network |
| `resolve_ipv4` | `String` | `Array` | Network |
| `resolve_ipv6` | `String` | `Array` | Network |
| `resolve_mx` | `String` | `Array` | Network |
| `resolve_txt` | `String` | `Array` | Network |
| `reverse` | `String` | `Array` | Network |

### `std::email` — 3 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `send_html` | `String, Int, String, String, String, String, String, String, String` | `String` | Network |
| `send_simple` | `String, Int, String, String, String, String, String, String` | `String` | Network |
| `send_with_attachment` | `String, Int, String, String, String, String, String, String, String, String, Bytes` | `String` | Network |

### `std::encoding` — 8 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `base64_decode` | `String` | `Bytes` | None |
| `base64_encode` | `Bytes` | `String` | None |
| `hex_decode` | `String` | `Bytes` | None |
| `hex_encode` | `Bytes` | `String` | None |
| `percent_decode` | `String` | `String` | None |
| `percent_encode` | `String` | `String` | None |
| `utf8_decode` | `Bytes` | `String` | None |
| `utf8_encode` | `String` | `Bytes` | None |

### `std::env` — 3 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `args` | `—` | `Array` | Environment |
| `current_dir` | `—` | `String` | Environment |
| `get` | `String` | `String` | Environment |

### `std::freestanding` — 6 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `generate_linker_script` | `String, Int, Int` | `String` | None |
| `generate_startup_asm` | `String, String` | `String` | None |
| `get_active_target` | `—` | `String` | None |
| `init` | `String` | `Bool` | None |
| `shutdown` | `—` | `Bool` | None |
| `validate_target_spec` | `String` | `Bool` | None |

### `std::freestanding_cpu` — 7 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `dispatch_exception` | `Int, Int, Int` | `Int` | None |
| `get_last_fault_addr` | `—` | `Int` | None |
| `init_exception_table` | `Int` | `Bool` | None |
| `invoke_syscall` | `Int, Int, Int, Int` | `Int` | None |
| `register_exception_handler` | `Int, Int` | `Bool` | None |
| `register_syscall_handler` | `Int, Int` | `Bool` | None |
| `shutdown` | `—` | `Bool` | None |

### `std::freestanding_memory` — 7 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `allocate_frame` | `—` | `Int` | None |
| `deallocate_frame` | `Int` | `Bool` | None |
| `free_frames_count` | `—` | `Int` | None |
| `init_frame_allocator` | `Int, Int` | `Bool` | None |
| `map_page` | `Int, Int, Int` | `Bool` | None |
| `shutdown` | `—` | `Bool` | None |
| `translate_page` | `Int` | `Int` | None |

### `std::freestanding_mmio` — 7 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `init_mmio_region` | `Int, Int` | `Bool` | None |
| `read_mmio_u32` | `Int` | `Int` | None |
| `serial_get_buffer` | `—` | `String` | None |
| `serial_init` | `Int, Int` | `Bool` | None |
| `serial_write_str` | `String` | `Int` | None |
| `shutdown` | `—` | `Bool` | None |
| `write_mmio_u32` | `Int, Int` | `Bool` | None |

### `std::fs` — 16 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `append` | `String, Bytes` | `Nil` | Filesystem |
| `atomic_write` | `String, Bytes` | `Nil` | Filesystem |
| `copy` | `String, String` | `Int` | Filesystem |
| `create_dir` | `String` | `Nil` | Filesystem |
| `exists` | `String` | `Bool` | Filesystem |
| `file_size` | `String` | `Int` | Filesystem |
| `is_dir` | `String` | `Bool` | Filesystem |
| `is_file` | `String` | `Bool` | Filesystem |
| `list_dir` | `String` | `Array` | Filesystem |
| `read_bytes` | `String` | `Bytes` | Filesystem |
| `read_text` | `String` | `String` | Filesystem |
| `remove_dir` | `String` | `Nil` | Filesystem |
| `remove_file` | `String` | `Nil` | Filesystem |
| `rename` | `String, String` | `Nil` | Filesystem |
| `write_bytes` | `String, Bytes` | `Nil` | Filesystem |
| `write_text` | `String, String` | `Nil` | Filesystem |

### `std::fswatch` — 4 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `close` | `Int` | `Nil` | Filesystem |
| `next_event` | `Int, Int` | `String` | Filesystem |
| `open` | `String, Bool` | `Int` | Filesystem |
| `watch_once` | `String, Int, Bool` | `String` | Filesystem |

### `std::game` — 5 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `check_collision` | `Float, Float, Float, Float, Float, Float, Float, Float` | `Bool` | None |
| `fps` | `—` | `Int` | UserInterface |
| `init` | `String, Int, Int` | `Bool` | UserInterface |
| `shutdown` | `—` | `Bool` | UserInterface |
| `step` | `—` | `Float` | UserInterface |

### `std::gui` — 11 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `add_button` | `Int, String, Int, Int, Int, Int` | `Int` | UserInterface |
| `add_label` | `Int, String, Int, Int` | `Int` | UserInterface |
| `child_count` | `Int` | `Int` | UserInterface |
| `create_container` | `String, Int, Int` | `Int` | UserInterface |
| `get_text` | `Int` | `String` | UserInterface |
| `init` | `—` | `Bool` | UserInterface |
| `is_clicked` | `Int` | `Bool` | UserInterface |
| `render` | `Int` | `Any` | UserInterface |
| `set_text` | `Int, String` | `Bool` | UserInterface |
| `shutdown` | `—` | `Bool` | UserInterface |
| `trigger_click` | `Int` | `Bool` | UserInterface |

### `std::hash` — 11 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `blake3` | `Bytes` | `String` | None |
| `blake3_bytes` | `Bytes` | `Bytes` | None |
| `hmac_sha256` | `Bytes, Bytes` | `String` | None |
| `hmac_sha512` | `Bytes, Bytes` | `String` | None |
| `sha256` | `Bytes` | `String` | None |
| `sha256_bytes` | `Bytes` | `Bytes` | None |
| `sha384` | `Bytes` | `String` | None |
| `sha3_256` | `Bytes` | `String` | None |
| `sha3_512` | `Bytes` | `String` | None |
| `sha512` | `Bytes` | `String` | None |
| `sha512_bytes` | `Bytes` | `Bytes` | None |

### `std::http` — 13 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `build_response` | `Int, Map, Bytes, Bool` | `Bytes` | None |
| `cors` | `Map, String, String` | `Map` | None |
| `error_response` | `Int, String` | `Map` | None |
| `json_response` | `Int, Any` | `Map` | None |
| `parse_multipart` | `String, Bytes, Int, Int` | `Array` | None |
| `parse_query` | `String, Int` | `Map` | None |
| `parse_request` | `Bytes` | `Option` | None |
| `rate_limit` | `String, Int, Int` | `Bool` | None |
| `reason_phrase` | `Int` | `String` | None |
| `request` | `String, String, Map, Bytes, Int, Int, Int` | `Map` | Network |
| `request_id` | `Map` | `Map` | None |
| `route_match` | `String, String` | `Option` | None |
| `security_headers` | `Map` | `Map` | None |

### `std::http_full` — 4 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `get_json` | `String, Map, Map` | `Any` | Network |
| `post_form` | `String, Array, Map, Map` | `Map` | Network |
| `post_json` | `String, Any, Map, Map` | `Any` | Network |
| `request` | `String, String, Map, Bytes, Map` | `Map` | Network |

### `std::image` — 21 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `blur` | `Int, Float` | `Int` | None |
| `brighten` | `Int, Int` | `Int` | None |
| `close` | `Int` | `Nil` | None |
| `color_type` | `Int` | `String` | None |
| `crop` | `Int, Int, Int, Int, Int` | `Int` | None |
| `encode` | `Int, String` | `Bytes` | None |
| `flip_horizontal` | `Int` | `Int` | None |
| `flip_vertical` | `Int` | `Int` | None |
| `from_rgba` | `Int, Int, Bytes` | `Int` | None |
| `grayscale` | `Int` | `Int` | None |
| `height` | `Int` | `Int` | None |
| `load` | `String` | `Int` | Filesystem |
| `load_bytes` | `Bytes` | `Int` | None |
| `resize` | `Int, Int, Int, String` | `Int` | None |
| `resize_exact` | `Int, Int, Int, String` | `Int` | None |
| `rotate180` | `Int` | `Int` | None |
| `rotate270` | `Int` | `Int` | None |
| `rotate90` | `Int` | `Int` | None |
| `save` | `Int, String` | `Nil` | Filesystem |
| `thumbnail` | `Int, Int, Int` | `Int` | None |
| `width` | `Int` | `Int` | None |

### `std::input` — 8 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `is_key_pressed` | `String` | `Bool` | UserInterface |
| `is_mouse_button_pressed` | `Int` | `Bool` | UserInterface |
| `mouse_pos` | `—` | `Array` | UserInterface |
| `set_key_state` | `String, Bool` | `Bool` | UserInterface |
| `set_mouse_button` | `Int, Bool` | `Bool` | UserInterface |
| `set_mouse_pos` | `Int, Int` | `Bool` | UserInterface |
| `set_touch_point` | `Int, Int, Int, Bool` | `Bool` | UserInterface |
| `touch_pos` | `Int` | `Array` | UserInterface |

### `std::json` — 6 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `flatten` | `Any` | `Array` | None |
| `merge` | `Any, Any` | `Any` | None |
| `parse` | `String` | `Any` | None |
| `pointer` | `Any, String` | `Any` | None |
| `pretty` | `Any` | `String` | None |
| `stringify` | `Any` | `String` | None |

### `std::jwt` — 5 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `peek_header` | `String` | `Any` | None |
| `sign_hs256` | `Any, Bytes` | `String` | None |
| `sign_rs256` | `Any, Bytes` | `String` | None |
| `verify_hs256` | `String, Bytes, String, String` | `Any` | None |
| `verify_rs256` | `String, Bytes, String, String` | `Any` | None |

### `std::kv` — 17 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `clear` | `Int` | `Nil` | Filesystem |
| `close` | `Int` | `Nil` | Filesystem |
| `compare_and_swap` | `Int, Bytes, Bytes, Bytes` | `Bool` | Filesystem |
| `contains` | `Int, Bytes` | `Bool` | Filesystem |
| `flush` | `Int` | `Int` | Filesystem |
| `get` | `Int, Bytes` | `Any` | Filesystem |
| `insert` | `Int, Bytes, Bytes` | `Any` | Filesystem |
| `keys` | `Int` | `Array` | Filesystem |
| `len` | `Int` | `Int` | Filesystem |
| `open` | `String` | `Int` | Filesystem |
| `open_tree` | `Int, String` | `Int` | Filesystem |
| `remove` | `Int, Bytes` | `Any` | Filesystem |
| `tree_get` | `Int, Bytes` | `Any` | Filesystem |
| `tree_insert` | `Int, Bytes, Bytes` | `Any` | Filesystem |
| `tree_keys` | `Int` | `Array` | Filesystem |
| `tree_len` | `Int` | `Int` | Filesystem |
| `tree_remove` | `Int, Bytes` | `Any` | Filesystem |

### `std::map` — 9 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `contains` | `Map, String` | `Bool` | None |
| `get` | `Map, String` | `Any` | None |
| `insert` | `Map, String, Any` | `Map` | None |
| `insert_new` | `Map, String, Any` | `Map` | None |
| `keys` | `Map` | `Array` | None |
| `length` | `Map` | `Int` | None |
| `new` | `—` | `Map` | None |
| `remove` | `Map, String` | `Map` | None |
| `values` | `Map` | `Array` | None |

### `std::math` — 14 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `abs` | `Float` | `Float` | None |
| `ceil` | `Float` | `Float` | None |
| `cos` | `Float` | `Float` | None |
| `exp` | `Float` | `Float` | None |
| `floor` | `Float` | `Float` | None |
| `ln` | `Float` | `Float` | None |
| `log` | `Float, Float` | `Float` | None |
| `pow` | `Float, Float` | `Float` | None |
| `round` | `Float` | `Float` | None |
| `sin` | `Float` | `Float` | None |
| `sqrt` | `Float` | `Float` | None |
| `tan` | `Float` | `Float` | None |
| `to_float` | `Int` | `Float` | None |
| `to_int` | `Float` | `Int` | None |

### `std::metrics` — 8 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `counter_add` | `String, Int` | `Int` | None |
| `counter_get` | `String` | `Int` | None |
| `gauge_get` | `String` | `Float` | None |
| `gauge_set` | `String, Float` | `Nil` | None |
| `histogram_record` | `String, Float` | `Nil` | None |
| `prometheus_export` | `—` | `String` | None |
| `reset` | `—` | `Nil` | None |
| `snapshot` | `—` | `Map` | None |

### `std::mobile` — 3 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `poll_events` | `—` | `Array` | UserInterface |
| `state` | `—` | `String` | UserInterface |
| `trigger` | `String` | `Bool` | UserInterface |

### `std::net` — 1 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `http_get` | `String` | `Map` | Network |

### `std::notify` — 1 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `send` | `String, String` | `Bool` | UserInterface |

### `std::onnx` — 14 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `close` | `Int` | `Nil` | None |
| `input_count` | `Int` | `Int` | None |
| `input_shape` | `Int, Int` | `Array` | None |
| `load` | `String` | `Int` | Filesystem |
| `load_bert` | `String, Int, Int` | `Int` | Filesystem |
| `load_bert3` | `String, Int, Int` | `Int` | Filesystem |
| `load_shape` | `String, Array` | `Int` | Filesystem |
| `output_count` | `Int` | `Int` | None |
| `output_shape` | `Int, Int` | `Array` | None |
| `run_bert` | `Int, Array, Array, Array` | `Map` | None |
| `run_bert3` | `Int, Array, Array, Array, Array` | `Map` | None |
| `run_bert_pooled` | `Int, Int, Int, Array, Array` | `Map` | None |
| `run_f32` | `Int, Array, Array` | `Map` | None |
| `run_ids` | `Int, Array, Array` | `Map` | None |

### `std::password` — 4 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `hash_argon2` | `String` | `String` | None |
| `hash_bcrypt` | `String, Int` | `String` | None |
| `verify_argon2` | `String, String` | `Bool` | None |
| `verify_bcrypt` | `String, String` | `Bool` | None |

### `std::path` — 8 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `absolute` | `String` | `String` | Filesystem |
| `canonical` | `String` | `String` | Filesystem |
| `extension` | `String` | `String` | None |
| `file_name` | `String` | `String` | None |
| `join` | `String, String` | `String` | None |
| `normalize` | `String` | `String` | None |
| `parent` | `String` | `String` | None |
| `stem` | `String` | `String` | None |

### `std::pdf` — 9 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `add_line` | `Int, Int, Int, Float, Float, Float, Float, Float` | `Nil` | None |
| `add_page` | `Int, Float, Float, String` | `Int` | None |
| `add_rect` | `Int, Int, Int, Float, Float, Float, Float` | `Nil` | None |
| `add_text` | `Int, Int, Int, String, Float, Float, Float` | `Nil` | None |
| `close` | `Int` | `Nil` | None |
| `new` | `String, Float, Float` | `Int` | None |
| `page_count` | `Int` | `Int` | None |
| `save` | `Int, String` | `Nil` | Filesystem |
| `set_color` | `Int, Int, Int, Float, Float, Float` | `Nil` | None |

### `std::plot` — 5 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `bar` | `String, String, String, Array, Array` | `Nil` | Filesystem |
| `histogram` | `String, String, String, Array, Int` | `Nil` | Filesystem |
| `line` | `String, String, String, String, Array, Array` | `Nil` | Filesystem |
| `multi_line` | `String, String, String, String, Array, Array, Array` | `Nil` | Filesystem |
| `scatter` | `String, String, String, String, Array, Array` | `Nil` | Filesystem |

### `std::process` — 22 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `args` | `—` | `Array` | Environment |
| `env_get` | `String` | `Option` | Environment |
| `env_set` | `String, String` | `Nil` | Environment |
| `env_unset` | `String` | `Nil` | Environment |
| `env_vars` | `—` | `Array` | Environment |
| `exit` | `Int` | `Nil` | Process |
| `hostname` | `—` | `String` | Environment |
| `pipe` | `Array` | `Map` | Process |
| `run` | `String` | `Map` | Process |
| `run_timeout` | `String, Array, Int` | `Map` | Process |
| `run_with_input` | `String, Bytes` | `Map` | Process |
| `self_pid` | `—` | `Int` | None |
| `send_signal` | `Int, Int` | `Nil` | Process |
| `set_working_dir` | `String` | `Nil` | Filesystem |
| `shell` | `String` | `Map` | Process |
| `spawn` | `String` | `Int` | Process |
| `spawn_kill` | `Int` | `Nil` | Process |
| `spawn_pid` | `Int` | `Int` | Process |
| `spawn_poll` | `Int` | `Option` | Process |
| `spawn_wait` | `Int` | `Map` | Process |
| `username` | `—` | `String` | Environment |
| `working_dir` | `—` | `String` | Filesystem |

### `std::procfs` — 18 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `available_memory` | `—` | `Int` | Environment |
| `cpu_count` | `—` | `Int` | Environment |
| `cpu_usage` | `—` | `Float` | Environment |
| `cpus` | `—` | `Array` | Environment |
| `disks` | `—` | `Array` | Environment |
| `hostname` | `—` | `String` | Environment |
| `kernel` | `—` | `String` | Environment |
| `load_average` | `—` | `Map` | Environment |
| `networks` | `—` | `Map` | Environment |
| `os_name` | `—` | `String` | Environment |
| `os_version` | `—` | `String` | Environment |
| `process_count` | `—` | `Int` | Environment |
| `top_processes` | `Int` | `Array` | Environment |
| `total_memory` | `—` | `Int` | Environment |
| `total_swap` | `—` | `Int` | Environment |
| `uptime` | `—` | `Int` | Environment |
| `used_memory` | `—` | `Int` | Environment |
| `used_swap` | `—` | `Int` | Environment |

### `std::progress` — 7 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `abandon` | `Int` | `Nil` | UserInterface |
| `bar_new` | `Int` | `Int` | UserInterface |
| `finish` | `Int, String` | `Nil` | UserInterface |
| `increment` | `Int, Int` | `Nil` | UserInterface |
| `set_message` | `Int, String` | `Nil` | UserInterface |
| `set_position` | `Int, Int` | `Nil` | UserInterface |
| `spinner_new` | `—` | `Int` | UserInterface |

### `std::qrcode` — 5 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `save_png` | `String, String, Int, String` | `Nil` | Filesystem |
| `to_ascii` | `String, String, String, String` | `String` | None |
| `to_png` | `String, String, Int` | `Bytes` | None |
| `to_svg` | `String, String, Int` | `Bytes` | None |
| `to_unicode` | `String, String` | `String` | None |

### `std::random` — 8 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `bool` | `—` | `Bool` | None |
| `bytes` | `Int` | `Bytes` | None |
| `float` | `—` | `Float` | None |
| `int` | `—` | `Int` | None |
| `range` | `Int, Int` | `Int` | None |
| `seeded_bytes` | `Int, Int` | `Bytes` | None |
| `seeded_float` | `Int` | `Float` | None |
| `seeded_int` | `Int, Int, Int` | `Int` | None |

### `std::readline` — 4 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `prompt` | `String` | `String` | UserInterface |
| `prompt_persistent` | `String, String` | `String` | FilesystemUserInterface |
| `prompt_secret` | `String` | `String` | UserInterface |
| `prompt_with_history` | `String` | `String` | UserInterface |

### `std::redis` — 21 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `close` | `Int` | `Nil` | Network |
| `connect` | `String` | `Int` | Network |
| `del` | `Int, String` | `Int` | Network |
| `exists` | `Int, String` | `Bool` | Network |
| `expire` | `Int, String, Int` | `Bool` | Network |
| `get` | `Int, String` | `Any` | Network |
| `hdel` | `Int, String, String` | `Int` | Network |
| `hget` | `Int, String, String` | `Any` | Network |
| `hgetall` | `Int, String` | `Array` | Network |
| `hset` | `Int, String, String, String` | `Nil` | Network |
| `incr` | `Int, String, Int` | `Int` | Network |
| `keys` | `Int, String` | `Array` | Network |
| `llen` | `Int, String` | `Int` | Network |
| `lpush` | `Int, String, String` | `Int` | Network |
| `lrange` | `Int, String, Int, Int` | `Array` | Network |
| `ping` | `Int` | `String` | Network |
| `raw` | `Int, String` | `String` | Network |
| `rpush` | `Int, String, String` | `Int` | Network |
| `set` | `Int, String, String` | `Nil` | Network |
| `set_ex` | `Int, String, String, Int` | `Nil` | Network |
| `ttl` | `Int, String` | `Int` | Network |

### `std::regex` — 7 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `captures` | `String, String` | `Array` | None |
| `find` | `String, String` | `String` | None |
| `find_all` | `String, String` | `Array` | None |
| `is_match` | `String, String` | `Bool` | None |
| `is_valid` | `String` | `Bool` | None |
| `replace_all` | `String, String, String` | `String` | None |
| `split` | `String, String` | `Array` | None |

### `std::router` — 5 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `at` | `Int, String` | `Any` | None |
| `drop` | `Int` | `Nil` | None |
| `insert` | `Int, String, String` | `Nil` | None |
| `matches` | `Int, String` | `Bool` | None |
| `new` | `—` | `Int` | None |

### `std::server` — 23 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `accept` | `Int, Int` | `Int` | Network |
| `body` | `Int` | `Bytes` | Network |
| `body_text` | `Int` | `String` | Network |
| `header` | `Int, String` | `Any` | Network |
| `headers` | `Int` | `Map` | Network |
| `local_addr` | `Int` | `String` | Network |
| `method` | `Int` | `String` | Network |
| `path` | `Int` | `String` | Network |
| `query` | `Int` | `String` | Network |
| `remote_addr` | `Int` | `String` | Network |
| `respond` | `Int, Int, String` | `Nil` | Network |
| `respond_bytes` | `Int, Int, String, Bytes` | `Nil` | Network |
| `respond_full` | `Int, Int, String, Map, Bytes` | `Nil` | Network |
| `respond_html` | `Int, Int, String` | `Nil` | Network |
| `respond_json` | `Int, Int, String` | `Nil` | Network |
| `start` | `String` | `Int` | Network |
| `stop` | `Int` | `Nil` | Network |
| `upgrade_websocket` | `Int, Int` | `Int` | Network |
| `url` | `Int` | `String` | Network |
| `ws_close` | `Int, Int, String` | `Nil` | Network |
| `ws_recv` | `Int` | `Array` | Network |
| `ws_send_binary` | `Int, Bytes` | `Nil` | Network |
| `ws_send_text` | `Int, String` | `Nil` | Network |

### `std::signals` — 3 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `install` | `String` | `Nil` | Process |
| `pending` | `String` | `Int` | Process |
| `wait_any` | `Int` | `String` | Process |

### `std::stats` — 5 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `mean` | `Array` | `Float` | None |
| `median` | `Array` | `Float` | None |
| `quantile` | `Array, Float` | `Float` | None |
| `stddev` | `Array` | `Float` | None |
| `variance` | `Array` | `Float` | None |

### `std::term` — 15 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `clear_line` | `—` | `Nil` | UserInterface |
| `clear_screen` | `—` | `Nil` | UserInterface |
| `disable_raw` | `—` | `Nil` | UserInterface |
| `enable_raw` | `—` | `Nil` | UserInterface |
| `enter_alt_screen` | `—` | `Nil` | UserInterface |
| `flush` | `—` | `Nil` | UserInterface |
| `hide_cursor` | `—` | `Nil` | UserInterface |
| `leave_alt_screen` | `—` | `Nil` | UserInterface |
| `move_to` | `Int, Int` | `Nil` | UserInterface |
| `print_attr` | `String, String` | `Nil` | UserInterface |
| `print_colored` | `String, String` | `Nil` | UserInterface |
| `print_styled` | `String, String, String` | `Nil` | UserInterface |
| `read_key` | `Int` | `String` | UserInterface |
| `show_cursor` | `—` | `Nil` | UserInterface |
| `size` | `—` | `Array` | UserInterface |

### `std::termux` — 23 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `battery_status` | `—` | `Any` | Process |
| `brightness` | `Int` | `Nil` | Process |
| `camera_info` | `—` | `Any` | Process |
| `camera_photo` | `String, String` | `Nil` | Process |
| `clipboard_get` | `—` | `String` | Process |
| `clipboard_set` | `String` | `Nil` | Process |
| `contacts` | `—` | `Any` | Process |
| `dialog` | `String, String` | `Any` | Process |
| `is_available` | `—` | `Bool` | Environment |
| `location` | `String, String` | `Any` | Process |
| `notify` | `String, String, Int` | `Nil` | Process |
| `notify_remove` | `Int` | `Nil` | Process |
| `sensor_list` | `—` | `Array` | Process |
| `sensor_read` | `String` | `Any` | Process |
| `share` | `String` | `Nil` | Process |
| `sms_list` | `Int` | `Any` | Process |
| `sms_send` | `String, String` | `Nil` | Process |
| `telephony_info` | `—` | `Any` | Process |
| `toast` | `String` | `Nil` | Process |
| `torch` | `Bool` | `Nil` | Process |
| `tts_speak` | `String` | `Nil` | Process |
| `vibrate` | `Int, Bool` | `Nil` | Process |
| `wifi_info` | `—` | `Any` | Process |

### `std::testing` — 2 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `assert` | `Bool, String` | `Nil` | None |
| `assert_eq` | `Any, Any, String` | `Nil` | None |

### `std::text` — 21 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `capitalize` | `String` | `String` | None |
| `contains` | `String, String` | `Bool` | None |
| `ends_with` | `String, String` | `Bool` | None |
| `equals` | `String, String` | `Bool` | None |
| `escape_html` | `String` | `String` | None |
| `hash64` | `String` | `Int` | None |
| `length` | `String` | `Int` | None |
| `levenshtein` | `String, String` | `Int` | None |
| `lines` | `String` | `Array` | None |
| `lowercase` | `String` | `String` | None |
| `parse_float` | `String` | `Option` | None |
| `parse_int` | `String` | `Option` | None |
| `replace` | `String, String, String` | `String` | None |
| `reverse` | `String` | `String` | None |
| `slugify` | `String` | `String` | None |
| `starts_with` | `String, String` | `Bool` | None |
| `substring` | `String, Int, Int` | `String` | None |
| `trim` | `String` | `String` | None |
| `truncate` | `String, Int, String` | `String` | None |
| `uppercase` | `String` | `String` | None |
| `words` | `String` | `Array` | None |

### `std::time` — 3 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `sleep_ms` | `Int` | `Nil` | None |
| `unix_millis` | `—` | `Int` | None |
| `unix_seconds` | `—` | `Int` | None |

### `std::tokenize` — 10 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `close` | `Int` | `Nil` | None |
| `decode` | `Int, Array, Bool` | `String` | None |
| `encode` | `Int, String, Bool` | `Map` | None |
| `encode_batch` | `Int, Array, Bool` | `Array` | None |
| `encode_padded` | `Int, String, Int, Int, Bool` | `Map` | None |
| `from_json` | `String` | `Int` | None |
| `id_to_token` | `Int, Int` | `Any` | None |
| `load` | `String` | `Int` | Filesystem |
| `token_to_id` | `Int, String` | `Any` | None |
| `vocab_size` | `Int` | `Int` | None |

### `std::try` — 1 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `catch` | `Any` | `Any` | None |

### `std::url` — 10 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `build_query` | `Array` | `String` | None |
| `fragment` | `String` | `String` | None |
| `host` | `String` | `String` | None |
| `is_valid` | `String` | `Bool` | None |
| `join` | `String, String` | `String` | None |
| `parse_query` | `String` | `Map` | None |
| `path` | `String` | `String` | None |
| `port` | `String` | `Int` | None |
| `query` | `String` | `String` | None |
| `scheme` | `String` | `String` | None |

### `std::uuid` — 5 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `is_valid` | `String` | `Bool` | None |
| `nil` | `—` | `String` | None |
| `normalize` | `String` | `String` | None |
| `v4` | `—` | `String` | None |
| `v7` | `—` | `String` | None |

### `std::vector` — 8 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `add` | `Array, Array` | `Array` | None |
| `argmax` | `Array` | `Int` | None |
| `cosine_similarity` | `Array, Array` | `Float` | None |
| `dot` | `Array, Array` | `Float` | None |
| `norm` | `Array` | `Float` | None |
| `normalize` | `Array` | `Array` | None |
| `scale` | `Array, Float` | `Array` | None |
| `sub` | `Array, Array` | `Array` | None |

### `std::wasm` — 14 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `heap_allocated_bytes` | `—` | `Int` | None |
| `heap_allocations` | `—` | `Int` | None |
| `heap_capacity` | `—` | `Int` | None |
| `heap_checkpoint` | `—` | `Int` | None |
| `heap_limit` | `—` | `Int` | None |
| `heap_peak_used` | `—` | `Int` | None |
| `heap_reclaimed_bytes` | `—` | `Int` | None |
| `heap_reset_counters` | `—` | `Bool` | None |
| `heap_restore` | `Int` | `Bool` | None |
| `heap_restores` | `—` | `Int` | None |
| `heap_scope_begin` | `—` | `Int` | None |
| `heap_scope_end` | `Int` | `Bool` | None |
| `heap_set_limit` | `Int` | `Bool` | None |
| `heap_used` | `—` | `Int` | None |

### `std::web` — 53 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `add_class` | `String, String` | `Nil` | None |
| `animation_cancel` | `Int` | `Bool` | None |
| `animation_start` | `String` | `Int` | None |
| `canvas_clear` | `String, String` | `Nil` | None |
| `canvas_fill_rect` | `String, Int, Int, Int, Int, String` | `Nil` | None |
| `canvas_line` | `String, Int, Int, Int, Int, String, Int` | `Nil` | None |
| `canvas_resize` | `String, Int, Int` | `Nil` | None |
| `canvas_stroke_rect` | `String, Int, Int, Int, Int, String, Int` | `Nil` | None |
| `canvas_text` | `String, String, Int, Int, String, String` | `Nil` | None |
| `event_checked` | `—` | `Bool` | None |
| `event_key` | `—` | `String` | None |
| `event_target_id` | `—` | `String` | None |
| `event_type` | `—` | `String` | None |
| `event_value` | `—` | `String` | None |
| `event_x` | `—` | `Int` | None |
| `event_y` | `—` | `Int` | None |
| `fetch` | `String, Int, Int, String` | `Int` | None |
| `fetch_body` | `—` | `String` | None |
| `fetch_cancel` | `Int` | `Bool` | None |
| `fetch_error` | `—` | `String` | None |
| `fetch_headers` | `—` | `String` | None |
| `fetch_ok` | `—` | `Bool` | None |
| `fetch_status` | `—` | `Int` | None |
| `fetch_url` | `—` | `String` | None |
| `focus` | `String` | `Nil` | None |
| `frame_count` | `—` | `Int` | None |
| `frame_delta_ms` | `—` | `Int` | None |
| `frame_id` | `—` | `Int` | None |
| `frame_time_ms` | `—` | `Int` | None |
| `listen` | `String, String, String` | `Int` | None |
| `query_exists` | `String` | `Bool` | None |
| `remove_class` | `String, String` | `Nil` | None |
| `request` | `String, String, String, String, Int, Int, String` | `Int` | None |
| `set_attribute` | `String, String, String` | `Nil` | None |
| `set_html` | `String, String` | `Nil` | None |
| `set_text` | `String, String` | `Nil` | None |
| `set_title` | `String` | `Nil` | None |
| `unlisten` | `Int` | `Bool` | None |
| `webgl_create` | `String, String, String, String, String` | `Int` | None |
| `webgl_delete` | `Int` | `Bool` | None |
| `webgl_draw` | `Int, String` | `Bool` | None |
| `webgl_supported` | `String` | `Bool` | None |
| `webgl_uniform_f32` | `Int, String, Int, Int` | `Bool` | None |
| `ws_close` | `Int, Int, String` | `Bool` | None |
| `ws_close_code` | `—` | `Int` | None |
| `ws_close_reason` | `—` | `String` | None |
| `ws_connect` | `String, String, Int, String, String, String, String` | `Int` | None |
| `ws_error` | `—` | `String` | None |
| `ws_id` | `—` | `Int` | None |
| `ws_message` | `—` | `String` | None |
| `ws_protocol` | `—` | `String` | None |
| `ws_send` | `Int, String` | `Bool` | None |
| `ws_was_clean` | `—` | `Bool` | None |

### `std::wifi` — 4 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `connection_info` | `—` | `Any` | Network |
| `scan` | `—` | `Array` | Network |
| `set_enabled` | `Bool` | `Nil` | Network |
| `signal_bars` | `Int` | `Int` | None |

### `std::window` — 12 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `close` | `Int` | `Bool` | UserInterface |
| `create` | `String, Int, Int` | `Int` | UserInterface |
| `is_open` | `Int` | `Bool` | UserInterface |
| `live_close` | `Int` | `Bool` | UserInterface |
| `live_is_open` | `Int` | `Bool` | UserInterface |
| `live_open` | `String, Int, Int` | `Int` | UserInterface |
| `live_poll_events` | `Int` | `Array` | UserInterface |
| `live_pump` | `Int, Int` | `Int` | UserInterface |
| `live_set_title` | `Int, String` | `Bool` | UserInterface |
| `poll_events` | `Int` | `Array` | UserInterface |
| `resize` | `Int, Int, Int` | `Bool` | UserInterface |
| `set_title` | `Int, String` | `Bool` | UserInterface |

### `std::ws` — 6 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `accept_key` | `String` | `String` | None |
| `encode` | `Int, Bytes, Bool` | `Bytes` | None |
| `parse` | `Bytes, Bool, Int` | `Option` | None |
| `upgrade_response` | `String, String` | `Bytes` | None |
| `validate_accept` | `Bytes, String` | `Bool` | None |
| `validate_upgrade` | `Map, String` | `Bytes` | None |

### `std::xml` — 4 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `escape_attr` | `String` | `String` | None |
| `escape_text` | `String` | `String` | None |
| `parse` | `String` | `Any` | None |
| `stringify` | `Any` | `String` | None |

### `std::yaml` — 3 funciones

| Función | Parámetros | Devuelve | Capacidad |
|---|---|---|---|
| `parse` | `String` | `Any` | None |
| `parse_multi` | `String` | `Array` | None |
| `stringify` | `Any` | `String` | None |


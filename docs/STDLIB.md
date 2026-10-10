# Biblioteca estándar

Las funciones `std::*` están escritas en Titan (cuerpos en `selfhost/native/`) y se compilan junto con el programa; no hay
código de otro lenguaje ni bibliotecas externas. La tabla de firmas es `selfhost/natives.titan` (la consulta el typechecker).
Hoy son **812 funciones en 73 espacios de nombres**. Cuántas tienen cuerpo en Titan se mide con
`bash selfhost/native/cobertura.sh -v`.

Además de estas, `std::sqlite`, `std::postgres`, `std::mysql`, `std::db`, `std::tls`, `std::runtime` y parte de `std::ws`, `std::net`,
`std::http` y `std::server` se compilan a instrucciones propias del bytecode (`selfhost/codegen_tables.titan`).

| Espacio de nombres | Funciones | Contenido |
|---|---|---|
| `std::archive` | 5 | TAR y ZIP (leer y escribir) |
| `std::array` | 7 | operaciones sobre arrays |
| `std::audio` | 62 | WAV, decodificación (WAV, FLAC, Ogg Vorbis), etiquetas, biblioteca, síntesis y motor de reproducción |
| `std::bytes` | 7 | lector/escritor binario con límites |
| `std::checksum` | 3 | FNV-1a, CRC-32, comparación en tiempo constante |
| `std::clipboard` | 2 | portapapeles (Termux/Linux) |
| `std::collections` | 57 | listas, conjuntos, agrupar, particionar, ventanas… |
| `std::compress` | 8 | gzip, zlib, deflate y zstd (comprimir y descomprimir) |
| `std::crypto` | 13 | ChaCha20-Poly1305, AES-GCM, Argon2id, bcrypt |
| `std::csv` | 2 | CSV con comillas |
| `std::datetime` | 49 | fechas, zonas horarias, formatos RFC 3339/2822 |
| `std::dirs` | 18 | carpetas del usuario (home, config, caché…) |
| `std::dns` | 7 | consultas DNS |
| `std::email` | 3 | SMTP |
| `std::encoding` | 8 | hex, Base64, UTF-8, porcentaje |
| `std::env` | 3 | variables de entorno |
| `std::freestanding` | 6 | programas sin sistema operativo (aarch64-none) |
| `std::freestanding_cpu` | 7 | CPU en modo sin sistema operativo |
| `std::freestanding_memory` | 7 | memoria en modo sin sistema operativo |
| `std::freestanding_mmio` | 7 | MMIO en modo sin sistema operativo |
| `std::fs` | 18 | archivos y carpetas |
| `std::fswatch` | 4 | vigilar cambios en archivos |
| `std::game` | 5 | bucle de juego 2D y colisiones |
| `std::gui` | 11 | árbol de widgets y rasterizador por software |
| `std::hash` | 11 | SHA-2, SHA-3, BLAKE3, HMAC |
| `std::http` | 13 | cliente HTTP/HTTPS |
| `std::http_full` | 4 | HTTP (funciones extendidas) |
| `std::image` | 21 | PNG, JPEG, WebP, BMP, GIF (leer y escribir) |
| `std::input` | 8 | estado de teclado, ratón y táctil |
| `std::io` | 2 | lectura limitada de texto/bytes |
| `std::json` | 6 | JSON (parse, pretty, pointer, merge patch) |
| `std::jwt` | 5 | JWT |
| `std::kv` | 17 | almacén clave-valor |
| `std::map` | 9 | mapas |
| `std::math` | 14 | funciones matemáticas |
| `std::metrics` | 8 | contadores, gauges, histogramas |
| `std::mobile` | 3 | ciclo de vida de apps móviles |
| `std::net` | 1 | TCP |
| `std::notify` | 1 | notificaciones |
| `std::onnx` | 14 | motor ONNX propio (BERT y modelos pequeños) |
| `std::password` | 4 | hash y verificación de contraseñas |
| `std::path` | 8 | rutas |
| `std::pdf` | 9 | generar PDF |
| `std::plot` | 5 | gráficos SVG |
| `std::process` | 23 | procesos hijos, pipes, tiempo límite |
| `std::procfs` | 18 | información del sistema (/proc) |
| `std::progress` | 7 | barras de progreso |
| `std::qrcode` | 5 | códigos QR |
| `std::random` | 8 | aleatorios y generadores deterministas |
| `std::readline` | 4 | línea de comandos con historial |
| `std::redis` | 21 | cliente Redis |
| `std::regex` | 7 | expresiones regulares |
| `std::router` | 5 | enrutador de rutas HTTP |
| `std::server` | 23 | servidor HTTP |
| `std::signals` | 3 | señales POSIX |
| `std::stats` | 5 | estadística |
| `std::term` | 15 | terminal (colores, teclas, TUI) |
| `std::termux` | 23 | Termux:API |
| `std::testing` | 2 | aserciones |
| `std::text` | 28 | texto y Unicode |
| `std::time` | 3 | tiempo y pausas |
| `std::tokenize` | 10 | tokenizadores estilo HuggingFace |
| `std::try` | 1 | `std::try::catch` |
| `std::url` | 10 | URLs |
| `std::uuid` | 5 | UUID v4 y v7 |
| `std::vector` | 8 | vectores numéricos |
| `std::wasm` | 14 | WebAssembly: memoria y source maps |
| `std::web` | 53 | navegador: DOM, eventos, fetch, WebSocket, Canvas 2D, WebGL2 (solo con host JS) |
| `std::wifi` | 4 | Wi-Fi (Termux) |
| `std::window` | 12 | ventanas |
| `std::ws` | 6 | WebSocket |
| `std::xml` | 4 | XML |
| `std::yaml` | 3 | YAML |

## Cosas que hay que saber

- Las limitaciones concretas de cada pieza (qué formatos no se soportan, qué cosas no se han probado en dispositivos) están en
  [`selfhost/ESTADO.md`](../selfhost/ESTADO.md). Algunos ejemplos: SQLite no tiene FTS, RTREE ni `ATTACH`; el audio no decodifica
  MP3/AAC/Opus; `std::web` solo funciona con un host JavaScript (WebAssembly en el navegador).
- **No hay sandbox ni sistema de permisos** en los ejecutables nativos: un programa Titan puede usar archivos, procesos y red
  como cualquier otro programa. La opción `--sandbox` pertenecía al prototipo en Rust y ya no existe.
- `std::checksum` no es criptografía. Para contraseñas, firmas y autenticación hay `std::crypto`, `std::hash`, `std::password` y `std::jwt`.
- El texto cuenta valores escalares Unicode, no grupos de caracteres percibidos por el usuario.

## Llamarlas desde Titan

```titan
fn main() {
    let codificado = std::encoding::base64_encode(std::encoding::utf8_encode("Titan"))
    let doc = std::json::parse("{\"respuesta\":42}")
    print(std::text::uppercase(codificado))
    print(doc.respuesta)
}
```

Los resultados que no son números ni textos son `bytes` y `map`; los campos de un mapa se leen con la sintaxis `resultado.campo`.

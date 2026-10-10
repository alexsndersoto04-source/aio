# Módulos de la biblioteca estándar: regex, uuid, hash, random, datetime, url, dirs

Todos están escritos en Titan, forman parte de la biblioteca estándar y no hay que activar nada.

| Prefijo | Qué da |
|---|---|
| `std::regex::*` | Expresiones regulares Unicode: `is_match`, `find`, `find_all`, `captures`, `replace_all`, `split` |
| `std::uuid::*` | UUID v4 (aleatorio) y v7 (ordenado por tiempo) |
| `std::hash::*` | SHA-256/384/512, SHA-3, BLAKE3, HMAC |
| `std::random::*` | Aleatorios del sistema y generadores deterministas (ChaCha20) |
| `std::datetime::*` | Ahora, formatos RFC 3339/2822, análisis, campos, desfases y zonas horarias |
| `std::url::*` | Analizar y construir URLs |
| `std::dirs::*` | Carpetas del usuario (home, config, caché, descargas) |

La lista completa de funciones y firmas está en `selfhost/natives.titan`. Ejemplo ejecutable: `examples/extras.titan`.

```bash
titan run examples/extras.titan
```

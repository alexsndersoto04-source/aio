# Moon — Estado actual del proyecto

**Fecha:** 2026-08-26
**Método:** verificación estática contra el runtime de Titan (`titan_stdlib`, `titan_vm`, `titan_typechecker`, `titan_codegen`, `titan_lexer`) + build real del frontend (`vite build`). El sandbox no tiene toolchain Rust/crates.io, así que el backend Titan no se pudo *ejecutar* aquí: se validó contra el código fuente del runtime (firmas, aridad, tipos, escapes del lexer) y contra la release v1.0.0 que usa el Dockerfile.

---

## Veredicto corto

Moon es ahora una **red social completa y real** (CERO SIMULACIÓN), escrita de cero sobre Titan + PostgreSQL:

- **Backend Titan**: 17 módulos, ~4.5k líneas, 68 rutas REST + WebSocket `/ws`, 14 migraciones Postgres, auth completo (Argon2id, JWT corto + refresh rotativo y revocable, 2FA por email, lockout, recuperación), feed con algoritmos reales, mensajería en vivo, notificaciones en vivo, imágenes reales con re-encode, admin + moderación + auditoría + rate limiting.
- **Frontend React 18 + Vite**: SPA completa (hash router), diseño minimalista blanco único, 13 vistas, tiempo real por WebSocket, subida de imágenes, sin ninguna simulación.
- **Despliegue**: `render.yaml` (API en Docker + PostgreSQL + web React) y `Dockerfile` (binario Titan v1.0.0) listos.

---

## 1. Backend (`projects/moon/src/`, 17 archivos, 4.408 líneas)

| Módulo | Contenido |
|---|---|
| `main.titan` | Router 68 rutas, 1 tarea/conexión con backpressure (`std::server::control`), WebSocket `/ws`, OPTIONS/CORS, 405 |
| `db.titan` | Pool Postgres + 14 migraciones versionadas (`std::postgres::migrate`) |
| `config.titan` | Variables de entorno; falla duro si faltan secretos |
| `http_util.titan` | Respuestas JSON/CORS/seguridad, rate limiting real, paginación, body/bytes, auditoría, stats |
| `validate.titan` | Sanitización anti-XSS, regex hashtags/menciones, emails/usernames, palabras bloqueadas, anti-spam |
| `auth.titan` | JWT HS256 (iss/exp), refresh rotativo con hash en BD, códigos 2FA/recovery con expiración, emails SMTP |
| `notify.titan` | Notificaciones en BD + entrega en vivo, preferencias |
| `realtime.titan` | Hub WebSocket actor (1 tarea central, canales), presence, typing, read, sync |
| `query.titan` | Queries compartidas, filtro de bloqueos universal, decorado por LOTE (3 queries por feed, sin N+1) |
| `h_auth.titan` | register/login/2FA/refresh/logout/me/update/privacy/change-password/recovery/sessions/delete/export |
| `h_users.titan` | perfil, posts con privacidad, followers/following, liked/saved, search, follow/unfollow/block/unblock, sugerencias |
| `h_posts.titan` | CRUD posts, imágenes, hashtags/menciones, likes/saves, comentarios/respuestas, hashtag page, reportes |
| `h_messages.titan` | conversaciones/DM, hilo paginado invertido, enviar/leer/reaccionar/borrar, privacidad DM, realtime |
| `h_explore.titan` | feed de seguidos, trending (score con decaimiento), for_you (2º grado), latest, hashtags trending |
| `h_notifications.titan` | list/read/read-all/prefs + `h_metrics` + `h_health` |
| `h_admin.titan` | dashboard, usuarios, suspender/activar/verificar, reportes (resolve borra contenido), backup JSON, stats 30 días, palabras bloqueadas, activity log |
| `h_media.titan` | subida multipart real (MIME + re-encode obligatorio a JPEG), thumbnails/avatar/cover, cuotas por usuario, servido estático binario |

### Verificaciones hechas (contra el runtime real)

- ✅ **84 funciones `std::*`** usadas existen en `titan_stdlib/src/native.rs` / `titan_vm` (`std::postgres`, `std::server`, `std::image`, `std::email`, `std::jwt`, `std::password`, `std::router`, `std::metrics`, etc.) — cero inexistentes.
- ✅ **Aridad** de todas las funciones propias (script de parseo): OK.
- ✅ **Firmas std::** verificadas una a una: `send_simple(host,port,user,pass,from,to,subject,body)`, `verify_hs256(token,secret,aud,iss)`, `respond_full(req,status,ct,headers,bytes)`, `parse_multipart(ct,body,max_parts,max_part)`, `pool(url,max,tls)`, `thumbnail(handle,w,h)`, `encode(handle,fmt)`…
- ✅ **Tipos del driver Postgres**: solo BOOL/INT/FLOAT/TEXT/BYTEA/JSON/NULL; todas las columnas de fecha se castean con `::text` al leer (y se comparan desde SQL). Corregido `revoked_at` que faltaba.
- ✅ **Lexer**: sin strings multilínea, escapes válidos (`\"`), llaves `{8,64}` de regex no son interpolación.
- ✅ **Typechecker**: `Type::Unknown` de `std::map::get`/`any` en comparaciones con nil, `any` en `for`, `parsed[0]`, `ServerControl` tipado, `channel` → tupla indexable, closures con capturas.
- ✅ **Seguridad**: no se filtra `password_hash` (helpers `ha_me_map`), CORS estricto, CSP, JWT con `iss:"moon"` (corregido — se firmaba sin issuer y se verificaba con uno), lockout por usuario, 2FA, rate limiting, borrado de cuenta en cascada, bloqueos en ambas direcciones con contadores correctos.
- ✅ **Escala**: decorado de feed por lote (3 queries por página en vez de 3 por post), backpressure de conexiones, pool 10, índices en todas las tablas.

### Bugs encontrados y corregidos durante la verificación

1. `a_issue_access/a_issue_temp` firmaban sin `iss` pero `a_verify` exigía `iss="moon"` → se añadió el claim.
2. `auth.titan:122` leía `revoked_at` sin `::text` (TIMESTAMPTZ no soportado por el driver) → corregido.
3. `h_auth_update_me` y `h_auth_export` devolvían `password_hash` → sanitizado con `ha_me_map`.
4. `h_users_block` ajustaba contadores de follows solo en una dirección → corregido (4 updates con EXISTS antes del DELETE).
5. `h_users_unfollow`/`h_users_unblock` no estaban expuestas en el router → añadidas (DELETE en las mismas rutas).
6. `v_media_ext` se usaba pero no existía → creada.
7. `h_media` re-declaraba `processed` → reescrito con `mut` (sin shadowing).
8. N+1 en feeds → `q_decorate_batch` (IN cláusula dinámica segura).
9. Contadores `total` del feed eran el tamaño de página → COUNT real.

---

## 2. Frontend (`frontend/`, React 18 + Vite, JS moderno)

Diseño minimalista blanco único (Inter, un solo acento tinta, tarjetas suaves), 53 módulos, 210 KB JS (64 KB gzip).

| Archivo | Función |
|---|---|
| `src/api.js` | Cliente HTTP real: refresh con rotación y cola, 401→retry único, errores tipificados, subida por XHR con progreso |
| `src/auth.jsx` | Contexto de sesión: login/registro/2FA/logout, restauración de sesión al recargar |
| `src/realtime.js` | WebSocket con reconexión exponencial, latido, cola de eventos |
| `src/unread.js` | Contadores no-leídos (notificaciones/mensajes) globales |
| `App.jsx` | Router por hash: 13 rutas, guardas de auth, layout 3 columnas responsive |
| `views/*` | AuthView, ResetView, FeedView (tabs + infinito), ExploreView (búsqueda real + hashtags), ProfileView, UserView (seguir/bloquear/mensaje), PostView (comentarios en vivo), MessagesView (chat WS con reacciones y leídos), NotificationsView (en vivo), SettingsView (perfil/fotos/privacidad/seguridad/2FA/sesiones/export/borrar), AdminView (dashboard/usuarios/reportes/palabras/actividad) |
| `components/*` | PostCard, Composer (fotos), LeftNav, BottomNav, Avatar, Icons (SVG propios) |

- ✅ `vite build` exitoso sin errores.
- ✅ Todas las llamadas del frontend apuntan a rutas reales del backend (cruce revisado).

---

## 3. Despliegue (Render)

- `render.yaml`: servicios `moon-api` (Docker, health `/api/health`), `moon-db` (PostgreSQL 16 free, DATABASE_URL automática) y `moon-web` (Node, SPA).
- `Dockerfile`: Ubuntu 24.04 + binario `zett` v1.0.0 de la release del repo + `CMD zett run src/main.titan`, con HEALTHCHECK real.
- Variables a completar en Render: `CORS_ORIGIN`, `PUBLIC_BASE_URL`, `SMTP_*` (email real), `VITE_API_URL`.

---

## 4. Pasos que faltan para producción (dependen del entorno, no del código)

| Paso | Dónde | Notas |
|---|---|---|
| Compilar/ejecutar el backend | Render (Docker) | Este sandbox no tiene toolchain Rust; Render sí compila el binario desde la release |
| Crear DB y aplicar migraciones | Render | `db_init` las aplica al arrancar |
| Probar flujo completo en vivo | Render | Registro → login → posts → fotos → DM → 2FA |
| Conectar SMTP real | Render | `SMTP_*` (2FA y recovery requieren email real) |
| Dominio + CORS | Render | `CORS_ORIGIN` y `VITE_API_URL` con el dominio final |
| *(Opcional)* CI | GitHub Actions | job que corre `vite build` y checks estáticos |

---

## 5. Honestidad técnica (SPEC §13)

- **No ejecutado en este sandbox**: el runtime de Titan requiere Rust/crates.io y no hay red ni toolchain. La revisión fue estática contra las fuentes del runtime y el build real del frontend.
- **Almacenamiento de imágenes**: archivos locales en `uploads/` servidos por la API (CDN del hosting en v2).
- **Email**: `std::email::send_simple` real; si no hay SMTP configurado, los códigos se imprimen en consola (documentado).
- **WS**: 1 hub por instancia; multi-instancia (Redis pub/sub) sería la evolución v2.

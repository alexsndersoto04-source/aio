# Moon — Especificación técnica: Red social profesional (v1)

**Estado:** en revisión del dueño del proyecto
**Fecha:** 2026-08-25
**Stack:** Titan (backend concurrente) · Postgres (datos) · Cloudinary (imágenes) · React (frontend) · Render (hosting free)
**Principio rector: CERO SIMULACIÓN.** Toda funcionalidad funciona contra base de datos real.

---

## 0. Qué es este proyecto (y qué no es)

Una red social web **real y profesional** que usuarios reales pueden usar a diario:
datos persistentes, seguridad real, moderación real, tiempo real real, panel de
administración completo. Nivel de referencia: el tier en el que startups lanzan
productos serios. **No es una demo y no es un esqueleto.**

Qué NO es (honesto, fuera de v1 y por qué):

| Fuera de v1 | Por qué |
|---|---|
| Video en posts | Pipeline pesado; v1 = texto + fotos |
| App nativa de teléfono (iOS/Android instalable) | v1 = web, 100% funcional en móvil |
| Push al teléfono con la app cerrada | v1 = tiempo real con la app abierta (web) |
| Recomendaciones por IA | v1 = algoritmo real por actividad y grafo social |

---

## 1. Arquitectura

```
[Frontend React] --HTTPS--> [Backend Titan (concurrente: 1 task por conexión)]
                                |--- Postgres (Supabase/Neon free)  = datos permanentes
                                |--- Cloudinary (free + CDN)        = imágenes
                                |--- WebSocket nativo de Titan      = tiempo real
```

- **Backend:** un solo servicio Titan. Concurrencia real (cada conexión en su own
  task). Manejo de errores estricto en todos los handlers. Rate limiting aplicado
  de verdad por endpoint y por usuario.
- **Datos:** Postgres con **migraciones versionadas** (nunca "cambios a mano"),
  foreign keys, índices en todas las queries calientes.
- **Crecimiento:** los planes free tienen techo. Cuando la app crezca se sube de
  plan **sin reescribir código**.

---

## 2. Modelo de datos completo (v1)

| Tabla | Qué guarda |
|---|---|
| `users` | Cuenta, hash Argon2id, perfil (display_name, bio, avatar, cover, link), contadores, estado (activo/suspendido/eliminado), rol (user/admin) |
| `refresh_tokens` | Sesiones reales: dispositivo, user-agent, expiración, revocable → permite "ver y cerrar sesiones activas" |
| `follows` | Grafo social (seguidor → seguido), con índice compuesto |
| `blocks` | Bloqueos: A bloquea B → no interactúan, no ven contenido, no aparecen |
| `reports` | Reportes: tipo (user/post/comment), motivo, detalle, estado (abierto/resuelto/descartado), resolución |
| `posts` | Post: contenido, autor, contadores (likes/comentarios), `edited_at`, estado (activo/eliminado) |
| `post_images` | Fotos del post: orden, URL original, URLs de variaciones |
| `likes` | Likes únicos por (usuario, post) |
| `comments` | Comentarios con **respuestas** (`parent_id`), autor, estado |
| `saves` | Guardados reales, únicos por (usuario, post) |
| `notifications` | Por tipo (follow, like, comment, mention, message), estado leído, origen, referencia |
| `notification_prefs` | Por usuario: qué notificaciones recibe (toggle por tipo) |
| `conversations` | Hilo 1:1: 2 usuarios, último mensaje, última actividad, no leídos (data model preparada para grupos en v2) |
| `messages` | Mensaje: remitente, receptor, texto/foto, estado (enviado/leído/eliminado), reacciones |
| `hashtags` | Hashtag + contador de uso |
| `post_hashtags` | Post ↔ hashtag (índices para explorar) |
| `activity_log` | Auditoría: logins, cambios de seguridad, acciones de admin (fecha, IP, detalle) |
| `app_stats` | Contadores diarios (usuarios nuevos, posts, mensajes) → dashboard y gráficas |

---

## 3. API (≈65 endpoints, v1)

**Cuenta y seguridad**
`register` · `login` · `logout` · `refresh` · `me` · `update_me` · `change_password` ·
`request_password_recovery` · `verify_password_recovery` · `delete_account` ·
`export_my_data` · `my_sessions` · `revoke_session`

**Perfil**
`user_profile` · `user_posts` · `user_liked` · `user_saved` · `user_followers` ·
`user_following` · `search_users` · `upload_avatar` · `update_profile` · `set_privacy`

**Grafo social**
`follow` · `unfollow` · `block` · `unblock` · `suggested_users`

**Posts (contenido)**
`create_post` (texto + N fotos) · `edit_post` (con marca "editado") · `delete_post` ·
`get_post` · `like` · `unlike` · `add_comment` · `reply` · `delete_comment` ·
`get_comments` · `posts_by_hashtag`

**Explorar**
`trending` (algoritmo real por actividad) · `for_you` (grafo + actividad) · `latest` ·
`hashtag_page`

**Mensajería (tiempo real)**
`conversations` · `start_conversation` · `thread` · `send_message` · `mark_read` ·
`delete_message` · `react_message`

**Notificaciones (tiempo real)**
`list` · `mark_read` · `mark_all_read` · `get_prefs` · `set_prefs`

**Tiempo real**
`ws /realtime` → canales: `messages`, `notifications`, `typing`, `presence`

**Admin (rol admin)**
`dashboard` · `list_users` · `suspend_user` · `activate_user` · `list_reports` ·
`resolve_report` · `backup_export` · `app_stats_range`

**Sistema**
`health` (con chequeo de DB) · `metrics` · `upload_image` (pipeline completo)

**Contrato de la API:** todo endpoint valida y sanea input; errores con código y
mensaje (nunca se fuga SQL); paginación `?page=&limit=` donde hay listados largos.

---

## 4. Seguridad (estándar empresarial)

1. **Contraseñas:** Argon2id. Nunca en claro. Validación de fuerza.
2. **Sesiones:** JWT corto (15 min) + refresh token en DB (7 días, rotación,
   revocable individual). "Cerrar sesión en todos los dispositivos" = real.
3. **Rate limiting REAL (aplicado, no declarado):**
   - login: 5 intentos / 15 min (con lockout progresivo)
   - posts: 30 / hora
   - mensajes: 60 / minuto
   - uploads: 10 / hora
   - registro: 3 / día / IP
4. **Validación:** longitud, tipo y contenido en CADA endpoint (sanitización de HTML).
5. **Imágenes:** validación real (MIME real, no extensión; tamaño máx; re-encode
   obligatoria) → ningún archivo ejecutable disfrazado.
6. **CORS** estricto a tu origen (no `*`). **Headers de seguridad** (CSP, HSTS,
   X-Content-Type-Options, Referrer-Policy).
7. **Auditoría:** logins, cambios de contraseña, acciones de admin → `activity_log`.
8. **Secretos:** si falta `JWT_SECRET` o `DATABASE_URL`, el servicio **no arranca**
   (cero valores por defecto).
9. **Bloqueos:** un usuario bloqueado no ve nada del que lo bloqueó y viceversa
   (se aplica en TODAS las queries, no solo en la UI).

---

## 5. Tiempo real (WebSocket nativo de Titan)

- **Eventos:** `new_message`, `message_read`, `message_deleted`, `typing`,
  `new_notification`, `presence`.
- **Recorte:** reconexión automática (backoff exponencial) + **sync de estado al
  reconectar** (ningún mensaje se pierde si se corta la red).
- **Heartbeat** (ping/pong) para detectar conexiones muertas.
- Los no-leídos se calculan en la DB (fuente de verdad), no solo en memoria.

---

## 6. Pipeline de imágenes

`upload` → validación (MIME real, tamaño) → original + variaciones generadas
(`image_mod`: avatar 100/400px, post 640/1080px) → Cloudinary → URLs de CDN.
- **Cotas por usuario** (imágenes y GB) contadas en DB.
- Las miniaturas se generan en el servidor (no en el teléfono del usuario).

---

## 7. Moderación real (lo que separa una red social real de una demo)

- **Reporte** de usuario / post / comentario con motivo (spam, acoso, contenido,
  otro + detalle).
- **Cola de moderación** (admin): ver contexto, suspender, eliminar, resolver con
  nota. El reportero puede ver el estado.
- **Suspensión** temporal/permanente: el usuario no puede iniciar sesión, con
  motivo real.
- **Filtros automáticos** (reglas reales, no IA): contenido repetido, velocidad de
  publicación, palabras bloqueadas (lista administrable).
- **Anti-spam** = el rate limiting de la sección 4 de verdad aplicado.

---

## 8. Ajustes y privacidad (usuario)

- Tema claro/oscuro · Idioma ES/EN · Preferencias de notificación por tipo.
- **Privacidad:** ¿quién puede escribirte (todos / seguidores / nadie)?
  Perfil público o privado (privado = posts visibles solo para seguidores).
- **Sesiones activas:** ver dispositivo, fecha, IP; cerrar individual o todas.
- **Cuenta:** cambiar contraseña · **exportar mis datos** (JSON) ·
  **eliminar mi cuenta** (borrado real de mis datos, no "cuenta fantasma").

---

## 9. Panel de administración (web, solo rol admin)

- **Dashboard:** usuarios totales y nuevos (hoy/7d/30d), posts, mensajes,
  gráficas de crecimiento.
- **Usuarios:** buscar, ver detalle + actividad, suspender/activar, eliminar.
- **Reportes:** cola abierta, resolver con nota, estadísticas de reportes.
- **Backup:** exportar TODOS los datos (JSON) con un clic.
  *(Backup programado automático aparece cuando se tenga un plan con cron;
  en free el servicio duerme y no hay reloj garantizado.)*

---

## 10. Testing y calidad (definición de "listo")

Antes de publicar CADA fase:
1. **Tests unitarios** de la lógica pura (con `titan test`).
2. **Tests de integración:** endpoints reales contra Postgres real
   (flujo completo: registro → post → seguir → comentar → mensaje → admin).
3. **Prueba de carga:** N usuarios simulados concurrentes antes de publicar.
4. **Definición de "listo":** funciona de punta a punta contra BD real, sin errores
   en consola, funciona en móvil, seguridad aplicada, documentado.
   *"Simulado" no es una categoría que exista en este proyecto.*

---

## 11. Fases de construcción (cada una: demo en tu navegador → tests → tu aprobación)

| Fase | Contenido |
|---|---|
| **P0 — Cimientos de datos** | Postgres + migraciones + **rescate del dato existente** (SQLite actual) |
| **P1 — Cuenta y seguridad** | Auth completo, recuperación de contraseña por email, 2FA por email, sesiones gestionables, auditoría |
| **P2 — Perfil completo** | Editar todo, avatar, cover, tabs (posts/me gusta/guardados), seguidores/seguidos, público/privado |
| **P3 — Contenido** | Posts con varias fotos, edición (marca "editado"), hashtags, menciones, comentarios con respuestas, guardados |
| **P4 — Grafo social** | Seguir/dejar de seguir, bloquear, sugerencias basadas en tu grafo |
| **P5 — Mensajería en tiempo real** | Instantánea, ticks de leído (1/2), "escribiendo…", fotos en chat, reacciones, eliminar, no-leídos |
| **P6 — Notificaciones en tiempo real** | Todas las tipos + preferencias por tipo + marcar leídas |
| **P7 — Explorar y búsqueda** | Tendencias (algoritmo real), Para ti, páginas de hashtag, búsqueda instantánea de personas y posts |
| **P8 — Moderación y abuso** | Reportes, cola, suspensiones, filtros automáticos, anti-spam |
| **P9 — Ajustes, privacidad y admin** | Todo lo de las secciones 8 y 9 |
| **P10 — Rendimiento y operación** | Prueba de carga final, monitoreo, endurecimiento, backup real |

**Frontend (rediseño React):** consume el 100% de esta API (cada botón cableado a
un endpoint real). El diseño se define en una sesión aparte (lo que tú discutas);
la construcción corre en paralelo desde P2.

---

## 12. Techos del plan free (honesto) y crecimiento

| Límite free | Cuándo aprieta | Solución (sin reescribir código) |
|---|---|---|
| Servicio duerme ~15 min de inactividad | Con tráfico real constante | Plan pago (~$7/mes) |
| Postgres ~0.5 GB | Miles de posts + fotos | Subir plan de BD |
| Cloudinary 25 GB | Catálogo de medios grande | Subir plan |
| Sin cron garantizado | Backup automático programado | Plan pago |

---

## 13. Compromiso (escrito)

1. Las fases P0–P10 **son el proyecto**. No un subconjunto.
2. Ninguna fase avanza sin la aprobación del dueño.
3. Si algo resulta imposible en Titan, se documenta **aquí, antes de intentarlo**,
   con el motivo y la alternativa — nunca se descubre después.
4. Cada entrega: funcionando en vivo + tests + sin errores en consola.

# Moon — Servidor real (API + WebSocket + PostgreSQL)

Este es el servidor de Moon: **no hay nada simulado**. Cada cuenta, publicación,
mensaje, notificación o reporte que se ve en la aplicación vive en una base de
datos PostgreSQL de verdad.

Está escrito en Node.js (22) y usa **el mismo esquema** que el servidor Titan que
se despliega en producción: las migraciones no se escriben aquí, se extraen de
`projects/moon/src/db.titan` con `extraer-esquema.mjs`. Así los dos servidores
hablan con las mismas tablas.

## Puesta en marcha (un solo comando)

```bash
cd projects/moon/server
npm install          # dependencias (incluye el motor PostgreSQL para desarrollo)
npm run dev          # arranca la base de datos, aplica el esquema y levanta la API
```

Eso deja:

| Servicio | Dirección | Qué es |
|---|---|---|
| API | http://127.0.0.1:3000 | Rutas `/api/*` y WebSocket `/ws` |
| PostgreSQL | 127.0.0.1:55432 | Base de datos real (carpeta `/tmp/moon-pgdata`) |

Opciones útiles:

```bash
node dev.mjs --reset        # borra todos los datos y empieza de cero
node dev.mjs --solo-bd      # solo la base de datos
```

## La aplicación, contra este servidor

```bash
cd frontend
API_PROXY_TARGET=http://127.0.0.1:3000 npm run dev
```

El servidor de desarrollo de Vite reenvía `/api` y `/ws` a la API, así que la
interfaz queda en http://localhost:5173 llamando a datos reales.

## Comprobaciones

```bash
# 59 comprobaciones contra la API y la base de datos reales
node prueba-api.mjs

# La aplicación de verdad, manejada desde la interfaz (registro, publicar…)
cd ../../frontend && npm i --no-save jsdom && API=http://127.0.0.1:3000 node scripts/prueba-real.mjs
```

## Qué hay dentro

| Archivo | Contenido |
|---|---|
| `dev.mjs` | Arranca PostgreSQL (motor incluido) + migraciones + API |
| `extraer-esquema.mjs` | Lee las migraciones de `src/db.titan` y genera `esquema.json` |
| `src/index.mjs` | Servidor HTTP, CORS, límites, salud y métricas |
| `src/nucleo.mjs` | Enrutador, contexto de petición y errores |
| `src/auth.mjs` | Contraseñas Argon2id, JWT HS256, sesiones, códigos de un solo uso |
| `src/rutas-auth.mjs` | Cuenta, acceso (con 2FA), perfil, privacidad, recuperación |
| `src/rutas-social.mjs` | Inicio, publicaciones, comentarios, personas, búsqueda, etiquetas, notificaciones, reportes |
| `src/rutas-mensajes.mjs` | Conversaciones, mensajes, no leídos, reacciones, borrado |
| `src/rutas-admin.mjs` | Resumen, serie diaria, moderación, palabras bloqueadas, actividad |
| `src/rutas-historias.mjs` | Historias de 24 horas, visitas y presencia |
| `src/rutas-grupos.mjs` | Grupos: crear, entrar, salir, publicar dentro, miembros |
| `src/rutas-media.mjs` | Subida y entrega de imágenes (multipart) |
| `src/estatico.mjs` | Entrega la aplicación web compilada en el mismo puerto |
| `src/ws.mjs` | Tiempo real: notificaciones, mensajes, «escribiendo…», contadores |
| `src/correo.mjs` | Correo por SMTP (2FA y recuperación de contraseña) |
| `src/limites.mjs` | Límite de peticiones por ventana deslizante |
| `prueba-api.mjs` | Prueba de extremo a extremo de la API |

## Variables de entorno

| Variable | Obligatoria | Para qué |
|---|---|---|
| `DATABASE_URL` | Sí (salvo `npm run dev`) | Conexión a PostgreSQL |
| `JWT_SECRET` | Sí | Firma de tokens (mínimo 32 caracteres) |
| `PORT` | No (3000) | Puerto de escucha |
| `CORS_ORIGIN` | No | Orígenes permitidos, separados por comas |
| `PUBLIC_BASE_URL` | No | Enlaces en los correos |
| `MOON_UPLOADS` | No (`../uploads`) | Carpeta de imágenes subidas |
| `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASS`, `SMTP_FROM` | No | Envío de correo |

Sin SMTP configurado, el código de verificación y el enlace de recuperación se
devuelven en la propia respuesta (`dev_code`, `dev_token`) para poder terminar el
proceso en desarrollo. **Con SMTP configurado eso no ocurre.**

## Publicarlo en internet (plan gratuito)

`render.yaml` de esta carpeta describe el servicio tal cual, para Render:

1. Crea una cuenta gratuita en [Neon](https://neon.tech) y copia la cadena de
   conexión de PostgreSQL (permanente y gratuita).
2. Crea en Render un «Blueprint» apuntando a este repositorio y usa
   `projects/moon/server/render.yaml`.
3. Rellena `DATABASE_URL` (la de Neon) y `CORS_ORIGIN` (la dirección del frontend).

El almacenamiento de imágenes en el plan gratuito es temporal: para fotos que
duren, usa Cloudinary (`CLOUDINARY_URL`) o cualquier servicio de objetos.

## Detalles que conviene conocer

- **La primera cuenta registrada es administradora.** Así una instalación nueva
  tiene quien modere sin tocar la base de datos.
- Los contadores (me gusta, comentarios, seguidores, guardados) se actualizan con
  SQL, no en memoria: lo que se ve es lo que hay.
- Las sesiones se guardan como hash y se rotan en cada renovación; cerrar sesión
  revoca la sesión concreta.
- Límites: 10 publicaciones y 30 mensajes por minuto y cuenta; 300 peticiones por
  minuto y dirección.
- `/api/metrics` requiere cuenta de administración (antes era pública).

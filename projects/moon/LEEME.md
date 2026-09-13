# Moon — la red social (LEEME principal)

Moon es una red social **real**: cuentas, publicaciones, fotos, historias de 24
horas, grupos, mensajes, notificaciones, contactos y administración. Todo vive
en una base de datos PostgreSQL de verdad; no hay nada simulado ni de ejemplo.

## Dónde está

| Cosa | Dónde |
|---|---|
| Aplicación publicada | la dirección del servicio en Render (ver `DESPLEGAR.md`) |
| Base de datos | PostgreSQL (en producción, la nube; en local, el motor que trae `dev.mjs`) |
| Servidor | `projects/moon/server` (Node 22, sin dependencias de pago) |
| Interfaz | `frontend/` (React + Vite) |

## Cómo se arranca en tu computadora

```bash
cd projects/moon/server
npm install
node dev.mjs          # arranca PostgreSQL, aplica el esquema y levanta la API
```

Queda la aplicación completa en **http://localhost:3000** (la API y la web en el
mismo puerto). La primera cuenta que se registra queda como administradora.

Para trabajar en la interfaz con recarga instantánea:

```bash
cd frontend
npm install
npm run dev           # http://localhost:5173, con el proxy hacia la API
```

## Cómo se comprueba que funciona

```bash
# 59 comprobaciones contra la API y la base de datos reales
cd projects/moon/server && node prueba-api.mjs

# 51 comprobaciones desde la interfaz, en un navegador simulado con red real
cd frontend && npm i --no-save jsdom && API=http://127.0.0.1:3000 node scripts/prueba-real.mjs
```

Las dos pruebas crean cuentas nuevas con nombres únicos y dejan a la vista lo
que hicieron, para poder comprobarlo a mano después.

## Qué sabe hacer (todo real)

| Área | Qué incluye |
|---|---|
| Cuentas | Registro, acceso, 2FA, recuperación de contraseña, privacidad, bloqueos |
| Publicaciones | Texto, hasta 4 fotos, edición, borrado, me gusta, comentarios, guardados |
| Historias | Foto con pie, caducan a las 24 horas, visitas contadas, borrado |
| Grupos | Crear, entrar, salir, publicar dentro, miembros con su papel, privados |
| Mensajes | Conversaciones, tiempo real, reacciones, borrar, «escribiendo…» |
| Gente | Seguir, dejar de seguir, contactos con presencia (quién está en línea) |
| Avisos | Notificaciones en vivo, contadores, marcar leídas |
| Moderación | Reportes, palabras bloqueadas, actividad, métricas (solo administración) |

## Organización del proyecto

```
projects/moon/
├── src/db.titan          Esquema de la base de datos (fuente de la verdad)
├── server/               Servidor real (API + WebSocket + web compilada)
│   ├── dev.mjs           Arranque en un comando (bd + migraciones + API)
│   ├── esquema.json      Migraciones extraídas de db.titan
│   ├── prueba-api.mjs    Pruebas de la API (59)
│   └── src/              Rutas, autenticación, tiempo real, límites, correo
└── uploads/              Fotos subidas en desarrollo

frontend/
├── src/styles.css        Sistema de diseño completo (secciones §1 a §29)
├── src/components/       Barra superior, paneles, publicaciones, historias, avisos
├── src/views/            Pantallas: inicio, explorar, perfil, mensajes, grupos…
└── scripts/              Construcción y pruebas desde la interfaz
```

Las migraciones **no** se escriben a mano en el servidor: se extraen del esquema
`src/db.titan` con `node extraer-esquema.mjs`, de modo que el servidor de Node y
el servidor en Titan aplican exactamente las mismas tablas (van por la v17).

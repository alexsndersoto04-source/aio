# 🌙 Moon — Red Social Moderna y Minimalista

Backend completo en **Titan** + Frontend en **React**. Desplegado 100% gratis.

---

## 🎯 Stack Tecnológico

| Capa | Tecnología | Hosting Gratuito |
|------|-----------|------------------|
| **Backend API** | Titan (lenguaje compilado) | Koyeb (siempre activo) |
| **Base de datos** | PostgreSQL | Neon.tech (500MB gratis) |
| **Cache** | Redis | Upstash (10K cmds/día) |
| **Imágenes** | Cloudinary | Cloudinary (25GB gratis) |
| **Frontend** | React + Vite + Tailwind | Vercel (ilimitado) |
| **Código** | Git | GitHub (ilimitado) |

**Costo total: $0.00 | Tarjeta de crédito: NO | PC: NO**

---

## 🚀 Deploy Paso a Paso (Desde tu Teléfono)

### PASO 1: Crear cuenta en Neon.tech (PostgreSQL)

1. Abre tu navegador y ve a **https://neon.tech**
2. Click en **"Sign Up"**
3. Selecciona **"Sign up with GitHub"** (un solo click)
4. Autoriza a Neon
5. Verás tu dashboard. Click en **"Create Project"**
6. Name: `moon-db`
7. Region: `South America (São Paulo)` (más cercano a ti)
8. Click **"Create Project"**
9. **COPIA la URL de conexión** (algo como):
   ```
   postgresql://user:pass@ep-xxx.region.aws.neon.tech/db
   ```
10. **GUÁRDALA** en notas de tu teléfono

### PASO 2: Crear cuenta en Upstash (Redis)

1. Ve a **https://upstash.com**
2. Click **"Sign Up"** → **"Sign up with GitHub"**
3. Click **"Create Database"**
4. Name: `moon-redis`
5. Region: `sa-east-1` (São Paulo)
6. Click **"Create"**
7. **COPIA la URL de conexión** (algo como):
   ```
   redis://default:xxx@xxx.upstash.io:6379
   ```
8. **GUÁRDALA** en notas

### PASO 3: Crear cuenta en Cloudinary (Imágenes)

1. Ve a **https://cloudinary.com**
2. Click **"Sign Up"** → **"Sign up with GitHub"**
3. En el dashboard, busca:
   - **Cloud Name:** `xxx` (guárdalo)
   - **API Key:** `123456789` (guárdalo)
   - **API Secret:** `xxx` (guárdalo)
4. **GUÁRDALOS** en notas

### PASO 4: Subir código a GitHub

1. Ve a **https://github.com**
2. Click en el **"+"** → **"New repository"**
3. Repository name: `moon`
4. Visibility: **Private**
5. Click **"Create repository"**
6. Ahora ve a la carpeta del proyecto en tu teléfono
7. Sube los archivos:
   - Todo el contenido de `projects/moon/`
8. Alternativa rápida desde el navegador:
   - Ve a tu repo en GitHub
   - Click **"uploading an existing file"**
   - Sube todos los archivos de `projects/moon/`

### PASO 5: Deploy Backend en Koyeb

1. Ve a **https://www.koyeb.com**
2. Click **"Sign up"** → **"Sign up with GitHub"**
3. Click **"Create App"**
4. Selecciona tu repo `moon`
5. Configura:
   - **Name:** `moon-api`
   - **Region:** `fra` (Frankfurt) o el más cercano
   - **Instance type:** `Free`
   - **Port:** `3000`
6. En **"Environment Variables"**, agrega:
   ```
   DATABASE_URL=postgresql://user:pass@ep-xxx.neon.tech/db
   REDIS_URL=redis://default:xxx@xxx.upstash.io:6379
   JWT_SECRET=genera-un-secreto-largo-aleatorio-aqui-64-caracteres
   ```
7. Click **"Deploy"**
8. Espera 2-3 minutos
9. Tu API estará en: `https://moon-api-xxx.koyeb.app`
10. **COPIA esta URL**

### PASO 6: Configurar Frontend

1. Edita `projects/moon/frontend/.env.production`
2. Cambia la URL:
   ```
   VITE_API_URL=https://moon-api-xxx.koyeb.app/api
   ```
3. Sube el cambio a GitHub

### PASO 7: Deploy Frontend en Vercel

1. Ve a **https://vercel.com**
2. Click **"Sign Up"** → **"Sign up with GitHub"**
3. Click **"Add New Project"**
4. Importa tu repo `moon`
5. Selecciona **"moon/frontend"** como root directory
6. Framework Preset: **Vite**
7. Click **"Deploy"**
8. Espera 1-2 minutos
9. Tu frontend estará en: `https://moon-xxx.vercel.app`
10. **¡LISTO! Tu red social está online**

---

## 🧪 Probar Localmente

Si quieres probar antes de desplegar:

```bash
# Backend
cd projects/moon
titan run src/main.titan

# Frontend (en otra terminal)
cd projects/moon/frontend
npm install
npm run dev
```

Abre `http://localhost:3000` en tu navegador.

---

## 📁 Estructura del Proyecto

```
projects/moon/
├── src/                      # Backend en Titan
│   ├── main.titan           # Entry point (40+ endpoints)
│   ├── config.titan         # Configuración
│   ├── database.titan       # Migraciones PostgreSQL
│   ├── auth.titan           # JWT + Argon2id
│   ├── users.titan          # Perfiles, follow/unfollow
│   ├── posts.titan          # Posts, likes, comentarios
│   ├── feed.titan           # Timeline + trending
│   ├── messages.titan       # Mensajería directa
│   ├── notifications.titan  # Notificaciones
│   ├── upload.titan         # Upload de imágenes
│   └── search.titan         # Búsqueda global
│
├── frontend/                 # Frontend en React
│   ├── src/
│   │   ├── pages/           # Login, Register, Feed, Profile, Messages
│   │   ├── components/      # Sidebar, PostCard, PostComposer
│   │   └── context/         # AuthContext
│   ├── package.json
│   └── tailwind.config.js
│
└── scripts/
    └── deploy_oracle.sh     # Script de deployment alternativo
```

---

## 🎨 Características

### Backend
- ✅ Autenticación JWT con Argon2id
- ✅ Sistema de follow/unfollow
- ✅ Posts con likes, comentarios, @mentions
- ✅ Feed cronológico + trending
- ✅ Mensajería directa
- ✅ Notificaciones en tiempo real
- ✅ Upload de imágenes con thumbnails
- ✅ Búsqueda global (usuarios, posts, hashtags)
- ✅ Rate limiting
- ✅ CORS + Security headers
- ✅ Concurrencia real (threads del OS)

### Frontend
- ✅ Diseño moderno y minimalista
- ✅ Mobile-first (responsive)
- ✅ Dark mode por defecto
- ✅ Animaciones suaves
- ✅ Accesibilidad (focus states)
- ✅ React Router
- ✅ Estado global con Context
- ✅ Axios para API calls

---

## 🔒 Seguridad

- Passwords: **Argon2id** (el más seguro)
- Tokens: **JWT** con expiración de 24h
- Rate limiting: Protección contra brute force
- CORS: Configurado correctamente
- SQL injection: Queries parametrizados
- File upload: Validación de tipo y tamaño
- HTTPS: Automático en Koyeb y Vercel

---

## 📊 Endpoints API

```
POST   /api/auth/register
POST   /api/auth/login
POST   /api/auth/refresh
POST   /api/auth/logout
GET    /api/auth/me

GET    /api/users/:id
GET    /api/users/name/:username
PUT    /api/users/me
POST   /api/follow/:user_id
POST   /api/unfollow/:user_id
GET    /api/users/:id/followers
GET    /api/users/:id/following
GET    /api/users/search

POST   /api/posts
GET    /api/posts/:id
DELETE /api/posts/:id
POST   /api/posts/:id/like
POST   /api/posts/:id/unlike
GET    /api/posts/:id/comments
POST   /api/posts/:id/comment

GET    /api/feed
GET    /api/feed/trending
GET    /api/users/:id/posts
GET    /api/users/:id/likes

GET    /api/messages/conversations
GET    /api/messages/:user_id
POST   /api/messages/send
DELETE /api/messages/del/:message_id
GET    /api/messages/unread/count

GET    /api/notifications
GET    /api/notifications/unread/count
POST   /api/notifications/read-all
POST   /api/notifications/:id/read

POST   /api/upload/image
POST   /api/upload/avatar

GET    /api/search
GET    /api/search/trending

GET    /api/health
```

---

## 🎯 Próximos Pasos

Una vez desplegado:

1. **Dominio personalizado** (opcional):
   - Compra en Namecheap (~$10/año)
   - Configura en Koyeb y Vercel

2. **Más características**:
   - Stories (como Instagram)
   - Video uploads
   - Live streaming
   - Grupos/Comunidades

3. **Monetización** (si quieres):
   - Suscripciones premium
   - Publicidad
   - Donaciones

---

## 💡 ¿Por qué Moon?

- **Construido con Titan** — un lenguaje compilado, verificado estáticamente, con 758 funciones nativas
- **Backend completo** — no necesitas 50 dependencias
- **Frontend moderno** — React + Tailwind, minimalista y profesional
- **100% gratis** — sin tarjeta de crédito
- **Escalable** — diseñado para crecer

---

## 📄 Licencia

MIT

---

**Hecho con 🌙 por Alex Soto**

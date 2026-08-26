# Moon — Guía de despliegue en Render (paso a paso)

## 1. Cuando el blueprint termine, abre tu dashboard de Render

Verás **3 servicios** (los nombres son los que puse en `render.yaml`):

| Servicio | Qué es | Su URL aparece en |
|---|---|---|
| `moon-api` | Backend Titan (Docker) | Dashboard → clic en `moon-api` → sección **"URL"** en la parte de arriba |
| `moon-db` | PostgreSQL | Dashboard → clic en `moon-db` |
| `moon-web` | Frontend React | Dashboard → clic en `moon-web` → sección **"URL"** |

> ⚠️ Copia las URLs **exactas** como aparecen (sin `/` al final). Ejemplo típico:
> - API: `https://moon-api.onrender.com`
> - Web: `https://moon-web.onrender.com`

---

## 2. Variable por variable (esto es TODO lo que hay que llenar)

### En el servicio `moon-api` → pestaña **Environment**

Entra a `moon-api` → **Environment** → verás las variables creadas por el blueprint. Llena solo estas:

| Variable | Qué poner exactamente | Ejemplo |
|---|---|---|
| `CORS_ORIGIN` | La URL del **frontend** (`moon-web`) | `https://moon-web.onrender.com` |
| `PUBLIC_BASE_URL` | La **misma** que `CORS_ORIGIN` (es para los enlaces de los emails) | `https://moon-web.onrender.com` |
| `SMTP_HOST` | El servidor de correo que elijas (ver sección 3) | `smtp-relay.brevo.com` |
| `SMTP_PORT` | Puerto del servidor de correo | `587` |
| `SMTP_USER` | Tu usuario de correo | `tucorreo@example.com` |
| `SMTP_PASS` | La contraseña o clave SMTP | `xxxxxxxxxxxx` |
| `SMTP_FROM` | La dirección "De:" que verá el receptor | `tucorreo@example.com` |

✅ **No toques** `DATABASE_URL` (la llenó Render automáticamente desde `moon-db`) ni `JWT_SECRET` (se generó solo).

### En el servicio `moon-web` → pestaña **Environment**

| Variable | Qué poner exactamente | Ejemplo |
|---|---|---|
| `VITE_API_URL` | La URL de la **API** (`moon-api`) — SIN `/api` al final | `https://moon-api.onrender.com` |

✅ `API_PROXY_TARGET` solo se usa en desarrollo local; en Render puedes dejarla vacía o poner la misma URL de la API.

### En el servicio `moon-db`

No hay que tocar nada. Si quieres conectarte con un cliente (opcional): Dashboard → `moon-db` → **Connect** → copias la cadena `postgres://...`.

---

## 3. Cómo conseguir los datos SMTP (para que lleguen los emails de 2FA y recuperación)

Elige UNA opción:

### Opción A — Brevo (recomendada, gratis y fácil) ⭐
1. Crea cuenta en https://www.brevo.com (gratis, 300 emails/día).
2. Ve a **Settings → SMTP & API → SMTP** y genera una **SMTP key**.
3. Valores:
   - `SMTP_HOST`: `smtp-relay.brevo.com`
   - `SMTP_PORT`: `587`
   - `SMTP_USER`: el correo con el que te registraste
   - `SMTP_PASS`: la SMTP key
   - `SMTP_FROM`: tu correo (o el remitente verificado que Brevo te dé)

### Opción B — Gmail
1. Activa la verificación en 2 pasos en tu cuenta Google.
2. Ve a https://myaccount.google.com/apppasswords → genera una **contraseña de aplicación**.
3. Valores:
   - `SMTP_HOST`: `smtp.gmail.com`
   - `SMTP_PORT`: `587`
   - `SMTP_USER`: tu correo de Gmail
   - `SMTP_PASS`: la contraseña de aplicación (16 caracteres)
   - `SMTP_FROM`: tu correo de Gmail

> 💡 Si **no** llenas SMTP: la app sigue funcionando, pero los códigos de 2FA/recovery se imprimen en los **logs** de `moon-api` (en vez de llegar por correo). Sirve para probar.

---

## 4. Guardar y esperar el redeploy

1. Después de llenar cada variable: **Save Changes** (abajo).
2. Render **redepliega automáticamente** el servicio (verás un spinner en la pestaña **Events**).
3. Cuando el estado pase a **Live**:
   - Abre `https://moon-web.onrender.com` → verás la página de login.
   - Crea una cuenta y prueba: publicar, subir foto, mandar mensaje, activar 2FA.

### Cómo saber que el backend arrancó bien
- Abre `https://moon-api.onrender.com/api/health` → debe responder `{"status":"ok","db":true}`.
- Si responde `db:false`, revisa la variable `DATABASE_URL` (debe ser la de `moon-db`).
- Si la página da error, mira los **logs**: `moon-api` → **Logs** (los errores de Titan aparecen ahí).

---

## 5. Avisos importantes (plan free)

- **El plan free de Render duerme los servicios** después de ~15 min sin uso; la primera visita tras dormir tarda ~30-60 s en despertar.
- **Las imágenes subidas viven en el disco del contenedor**: en el plan free el disco es temporal y **se borra cuando el servicio se reinicia**. Para guardar fotos para siempre hace falta disco persistente (plan pago) o un bucket (S3/Cloudinary en v2). Para probar, está perfecto.
- La base de datos free de Render **expira a los 30 días** (te avisan; los datos se pierden al vencer).
- Si ya tenías otra base de datos free en tu cuenta, Render puede rechazar crear `moon-db` — borra la anterior o usa el plan pago.

---

## 6. Después de probar

Cuando lo hayas probado, dime si quieres que ajustemos algo. Ideas que podemos hacer después:
- Convertir tu cuenta en **admin** (para usar el panel `/admin`).
- CI en GitHub Actions (build del frontend + checks).
- Cambios de diseño.

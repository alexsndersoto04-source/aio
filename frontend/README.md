# Moon Frontend

Frontend React para la red social Moon.

## Desarrollo local

```bash
npm install
npm run dev
```

El proxy de Vite reenvía `/api` al backend en `http://localhost:3000`.

## Revisar el diseño (sin backend)

El sistema de diseño «Órbita» tiene una galería que monta los componentes
reales con datos de ejemplo. Hay dos formas de mirarla:

```bash
# 1) En el servidor de desarrollo: http://localhost:5173/design.html
#    (o en la raíz, con MOON_VISTA=diseno npm run dev)
MOON_VISTA=diseno npm run dev

# 2) En un único archivo HTML, sin servidor ni conexión:
#    genera `moon-diseno.html` (CSS y JS incrustados) y ábrelo con el navegador
node scripts/diseno-suelto.mjs
```

La galería también se compila en `npm run build` (sale como `dist/design.html`).

## Deploy en Render

1. Crear un **Web Service** con runtime Node.
2. **Root Directory**: `frontend`
3. **Build Command**: `npm ci && npm run build`
4. **Start Command**: `npm start`
5. **Health Check Path**: `/`
6. Variable de entorno: `VITE_API_URL=https://TU-SERVICIO-DE-API.onrender.com`

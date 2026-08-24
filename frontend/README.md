# Moon Frontend

Frontend React para la red social Moon.

## Desarrollo local

```bash
npm install
npm run dev
```

El proxy de Vite reenvía `/api` al backend en `http://localhost:3000`.

## Deploy en Render

1. Crear un **Web Service** con runtime Node.
2. **Root Directory**: `frontend`
3. **Build Command**: `npm ci && npm run build`
4. **Start Command**: `npm start`
5. **Health Check Path**: `/`
6. Variable de entorno: `VITE_API_URL=https://TU-SERVICIO-DE-API.onrender.com`

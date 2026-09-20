import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App.jsx';
import { instalarDemo, esDemo } from './demo.js';
import './styles.css';
import './aurora.css';
import './llamada.css';
import './tema.css';
import { registrarServicio } from './push.js';

// Sin servidor (página de demostración): responde la API con datos de ejemplo.
instalarDemo();

// Servicio en segundo plano: hace falta para los avisos al teléfono y para la
// capa sin conexión. Solo existe en la build de producción y fuera del modo
// demostración (en desarrollo o en la demo estorbaría con la caché).
if (import.meta.env.PROD && !esDemo() && 'serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    registrarServicio().then((reg) => {
      if (reg) reg.update().catch(() => {});
    });
  });
  // Si un service worker nuevo toma el control, recargar suavemente para que
  // todo el usuario vea los cambios nuevos inmediatamente sin quedarse pegado.
  let recargando = false;
  navigator.serviceWorker.addEventListener('controllerchange', () => {
    if (!recargando) {
      recargando = true;
      window.location.reload();
    }
  });
}

// Si se toca un aviso, el servicio pide abrir una pantalla concreta.
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.addEventListener('message', (ev) => {
    const destino = ev.data && ev.data.tipo === 'ir-a' ? ev.data.url : '';
    if (destino) window.location.hash = destino;
  });
}

createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

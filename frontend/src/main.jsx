import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App.jsx';
import { instalarDemo, esDemo } from './demo.js';
import './styles.css';
import './aurora.css';
import './refinar.css';
import './acabado.css';
import './identidad.css';
import './minimal.css';
import { registrarServicio } from './push.js';

// Sin servidor (página de demostración): responde la API con datos de ejemplo.
instalarDemo();

// Servicio en segundo plano: hace falta para los avisos al teléfono y para la
// capa sin conexión. Solo existe en la build de producción y fuera del modo
// demostración (en desarrollo o en la demo estorbaría con la caché).
if (import.meta.env.PROD && !esDemo() && 'serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    registrarServicio();
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

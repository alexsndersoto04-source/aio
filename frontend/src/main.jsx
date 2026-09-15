import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App.jsx';
import { instalarDemo } from './demo.js';
import './styles.css';
import './aurora.css';
import { registrarServicio } from './push.js';

// Sin servidor (página de demostración): responde la API con datos de ejemplo.
instalarDemo();

// Servicio en segundo plano: necesario para los avisos al teléfono.
registrarServicio();

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

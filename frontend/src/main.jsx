import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App.jsx';
import { instalarDemo, esDemo } from './demo.js';
import './styles.css';
import './aurora.css';
import './refinar.css';

// Sin servidor (página de demostración): responde la API con datos de ejemplo.
instalarDemo();

// PWA: el service worker solo existe en la build de producción y fuera del
// modo demostración (en desarrollo estorbaría con la caché).
if (import.meta.env.PROD && !esDemo() && 'serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/sw.js').catch(() => { /* sin capa offline */ });
  });
}

createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App.jsx';
import { instalarDemo } from './demo.js';
import './styles.css';
import './aurora.css';

// Sin servidor (página de demostración): responde la API con datos de ejemplo.
instalarDemo();

createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

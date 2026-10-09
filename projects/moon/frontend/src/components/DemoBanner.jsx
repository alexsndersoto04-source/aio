// Moon — Aviso de modo demostración
// ============================================================
// Solo aparece cuando la aplicación se abre sin servidor (datos de ejemplo).

import React, { useState } from 'react';
import { esDemo } from '../demo.js';

export default function DemoBanner() {
  const [visible, setVisible] = useState(true);
  if (!esDemo() || !visible) return null;
  return (
    <div className="demo-banner" role="status">
      <span>
        <strong>Modo demostración.</strong> Datos de ejemplo que viven solo en tu
        navegador: nada se guarda y nada sale de tu dispositivo.
      </span>
      <button type="button" className="btn btn-outline btn-sm" onClick={() => setVisible(false)}>
        Entendido
      </button>
    </div>
  );
}

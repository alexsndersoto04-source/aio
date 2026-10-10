// Moon — Candado del PIN
// ============================================================
// Si en Ajustes → Seguridad pusiste un PIN y dejaste encendido
// «Pedir el PIN al abrir», esta pantalla aparece antes que nada:
// Moon no enseña nada hasta que se teclea bien el PIN.
//
// El PIN no viaja en claro ni una vez: se manda al servidor y él dice
// si es el correcto. Al acertar se recuerda solo en esta pestaña.

import React, { useEffect, useRef, useState } from 'react';
import { api } from '../api.js';
import { useAuth } from '../auth.jsx';
import { IconLock } from './Icons.jsx';

export const PIN_ABIERTO = 'moon_pin_abierto';

export function desbloqueado() {
  try { return sessionStorage.getItem(PIN_ABIERTO) === 'si'; } catch { return true; }
}

export default function BloqueoPin({ onAbierto }) {
  const { logout } = useAuth();
  const [pin, setPin] = useState('');
  const [error, setError] = useState('');
  const [probando, setProbando] = useState(false);
  const campo = useRef(null);

  useEffect(() => { campo.current?.focus(); }, []);

  async function probar(valor) {
    if (valor.length < 4 || probando) return;
    setProbando(true);
    setError('');
    try {
      await api.post('/api/me/ajustes/pin/verificar', { pin: valor });
      try { sessionStorage.setItem(PIN_ABIERTO, 'si'); } catch { /* sin almacén */ }
      onAbierto();
    } catch (e) {
      setPin('');
      setError(e?.status === 403 ? 'Ese no es el PIN. Prueba otra vez.' : (e?.message || 'No se pudo comprobar el PIN'));
      campo.current?.focus();
    } finally {
      setProbando(false);
    }
  }

  return (
    <div className="bloqueo-pin" role="dialog" aria-modal="true" aria-label="Moon está bloqueada">
      <div className="bloqueo-caja">
        <span className="bloqueo-icono"><IconLock /></span>
        <h1>Moon está bloqueada</h1>
        <p className="muted">Escribe tu PIN para entrar.</p>

        <form
          onSubmit={(e) => { e.preventDefault(); probar(pin); }}
          className="bloqueo-forma"
        >
          <input
            ref={campo}
            className="input pin-campo bloqueo-campo"
            type="password"
            inputMode="numeric"
            autoComplete="off"
            maxLength={8}
            aria-label="PIN"
            value={pin}
            onChange={(e) => {
              const limpio = e.target.value.replace(/\D/g, '');
              setPin(limpio);
              setError('');
              // Con 4 números ya se puede probar: si el PIN es más largo, sigue escribiendo.
              if (limpio.length === 4) probar(limpio);
            }}
          />
          <button className="btn btn-primary" disabled={pin.length < 4 || probando}>
            {probando ? 'Comprobando…' : 'Entrar'}
          </button>
        </form>

        {error ? <p className="error-nota">{error}</p> : null}

        <button
          type="button"
          className="enlace-suave"
          onClick={async () => { await logout(); }}
        >
          No soy yo: cerrar la sesión
        </button>
        <p className="muted bloqueo-ayuda">
          ¿Olvidaste el PIN? Cierra la sesión y entra con tu contraseña: Moon se abre igual.
        </p>
      </div>
    </div>
  );
}

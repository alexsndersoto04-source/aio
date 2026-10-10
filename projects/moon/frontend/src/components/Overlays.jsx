// Moon — Avisos flotantes y diálogos (montaje único en la aplicación)
// ============================================================
// `Overlays` escucha la cola de `ui.js` y dibuja:
//   · los avisos (toasts) de éxito / error / información
//   · los diálogos de confirmación y de petición de texto
// Sin ventanas nativas del navegador y accesibles por teclado (Escape,
// foco atrapado en el diálogo y `role="dialog"`).

import React, { useEffect, useRef, useState } from 'react';
import { subscribe } from '../ui.js';
import {
  IconCheck, IconAlert, IconInfo, IconX, IconLock, IconWarning,
} from './Icons.jsx';

const DURACION = 5200;

function IconoNivel({ level }) {
  if (level === 'ok') return <IconCheck />;
  if (level === 'err') return <IconAlert />;
  return <IconInfo />;
}

function Avisos({ avisos, quitar }) {
  if (avisos.length === 0) return null;
  return (
    <div className="toast-stack" role="status" aria-live="polite">
      {avisos.map((a) => (
        <div key={a.id} className={`toast ${a.level}`}>
          <IconoNivel level={a.level} />
          <div>{a.message}</div>
          <button className="close" onClick={() => quitar(a.id)} aria-label="Cerrar aviso">
            <IconX />
          </button>
        </div>
      ))}
    </div>
  );
}

function Dialogo({ peticion, cerrar }) {
  const { dialogo } = peticion;
  const [texto, setTexto] = useState(dialogo.value || '');
  const [password, setPassword] = useState('');
  const primero = useRef(null);
  const caja = useRef(null);

  useEffect(() => {
    if (primero.current) primero.current.focus();
    function onKey(e) {
      if (e.key === 'Escape') { e.preventDefault(); cerrar(null); }
      if (e.key === 'Tab' && caja.current) {
        const foco = caja.current.querySelectorAll('input, textarea, button');
        if (foco.length === 0) return;
        const lista = Array.from(foco);
        const i = lista.indexOf(document.activeElement);
        if (e.shiftKey && i <= 0) { e.preventDefault(); lista[lista.length - 1].focus(); }
        else if (!e.shiftKey && i === lista.length - 1) { e.preventDefault(); lista[0].focus(); }
      }
    }
    document.addEventListener('keydown', onKey);
    const anterior = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    return () => {
      document.removeEventListener('keydown', onKey);
      document.body.style.overflow = anterior;
    };
  }, [cerrar]);

  const esTexto = dialogo.tipo === 'texto';
  const valido = esTexto
    ? (!dialogo.requerido || texto.trim().length > 0)
    : (!dialogo.requerirPassword || password.length > 0);

  function aceptar() {
    if (!valido) return;
    cerrar(esTexto ? texto.trim() : (dialogo.requerirPassword ? password : true));
  }

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => { if (e.target === e.currentTarget) cerrar(null); }}
    >
      <div className="modal" role="dialog" aria-modal="true" aria-labelledby="dlg-title" ref={caja}>
        <h2 id="dlg-title">{dialogo.title}</h2>
        {dialogo.message ? <p className="sub">{dialogo.message}</p> : null}

        {esTexto ? (
          <div className="field">
            {dialogo.label ? <label htmlFor="dlg-text">{dialogo.label}</label> : null}
            {dialogo.multiline ? (
              <textarea
                id="dlg-text" className="textarea" rows={3} ref={primero}
                placeholder={dialogo.placeholder}
                value={texto}
                onChange={(e) => setTexto(e.target.value)}
              />
            ) : (
              <input
                id="dlg-text" className="input" ref={primero}
                placeholder={dialogo.placeholder}
                value={texto}
                onChange={(e) => setTexto(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') aceptar(); }}
              />
            )}
          </div>
        ) : null}

        {!esTexto && dialogo.requerirPassword ? (
          <div className="field">
            <label htmlFor="dlg-pass">
              <span className="row" style={{ gap: 6 }}>
                <IconLock /> Confirma tu contraseña
              </span>
            </label>
            <input
              id="dlg-pass" className="input" type="password" ref={primero}
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') aceptar(); }}
            />
          </div>
        ) : null}

        {dialogo.tipo === 'confirmar' && dialogo.danger && !dialogo.requerirPassword ? (
          <div className="alert warn">
            <IconWarning />
            <span>Esta acción no se puede deshacer.</span>
          </div>
        ) : null}

        <div className="modal-actions">
          <button className="btn btn-ghost" onClick={() => cerrar(null)}>
            {dialogo.cancelText || 'Cancelar'}
          </button>
          <button
            className={`btn ${dialogo.danger ? 'btn-danger' : 'btn-primary'}`}
            onClick={aceptar}
            disabled={!valido}
          >
            {dialogo.confirmText || 'Confirmar'}
          </button>
        </div>
      </div>
    </div>
  );
}

export default function Overlays() {
  const [avisos, setAvisos] = useState([]);
  const [cola, setCola] = useState([]);
  const temporizadores = useRef(new Map());

  useEffect(() => subscribe((ev) => {
    if (ev.kind === 'toast') {
      setAvisos((lista) => [...lista.slice(-3), { id: ev.id, level: ev.level, message: ev.message }]);
      const t = setTimeout(() => {
        setAvisos((lista) => lista.filter((a) => a.id !== ev.id));
        temporizadores.current.delete(ev.id);
      }, DURACION);
      temporizadores.current.set(ev.id, t);
    } else if (ev.kind === 'toast-cerrar') {
      setAvisos((lista) => lista.filter((a) => a.id !== ev.id));
    } else if (ev.kind === 'dialogo') {
      setCola((lista) => [...lista, ev]);
    }
  }), []);

  useEffect(() => () => {
    for (const t of temporizadores.current.values()) clearTimeout(t);
  }, []);

  function quitar(id) {
    setAvisos((lista) => lista.filter((a) => a.id !== id));
    const t = temporizadores.current.get(id);
    if (t) { clearTimeout(t); temporizadores.current.delete(id); }
  }

  function responder(peticion, valor) {
    setCola((lista) => lista.filter((p) => p.id !== peticion.id));
    peticion.resolver(valor);
  }

  const actual = cola[0];

  return (
    <>
      <Avisos avisos={avisos} quitar={quitar} />
      {actual ? <Dialogo key={actual.id} peticion={actual} cerrar={(v) => responder(actual, v)} /> : null}
    </>
  );
}

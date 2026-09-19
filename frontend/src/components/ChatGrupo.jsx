// Moon — Chat del grupo
// ============================================================
// Mensajes cortos entre los miembros: texto o nota de voz, en vivo por
// WebSocket, con «escribiendo…». Cada mensaje tiene su botón de opciones
// (responder, copiar, borrar) y el chat respeta las preferencias de cada
// persona (tamaño de texto, fondo y densidad) guardadas en su teléfono.
// La barra de escritura va siempre abajo, pegada al borde de la pantalla.

import React, { useEffect, useRef, useState } from 'react';
import { api, uploadMedia, imgUrl } from '../api.js';
import { useAuth } from '../auth.jsx';
import { realtime } from '../realtime.js';
import { horaMensaje } from '../utils.js';
import { avisoError, toast } from '../ui.js';
import { IconSend, IconTrash, IconChat, IconMore, IconResponder, IconCopy } from './Icons.jsx';
import { Grabador, AudioMensaje, puedeGrabar } from './NotaVoz.jsx';

const CLAVE_OPS = 'moon_chat_grupo_ops';

export function leerOpsChat() {
  try {
    return { texto: 'normal', fondo: 'neutro', denso: 'no', ...JSON.parse(localStorage.getItem(CLAVE_OPS) || '{}') };
  } catch {
    return { texto: 'normal', fondo: 'neutro', denso: 'no' };
  }
}

export function guardarOpsChat(ops) {
  try { localStorage.setItem(CLAVE_OPS, JSON.stringify(ops)); } catch { /* sin almacén */ }
}

function iniciales(u) {
  return (u.display_name || u.username || '?').slice(0, 2).toUpperCase();
}

export default function ChatGrupo({ grupo, esMiembro, onNecesitaEntrar, onVistos }) {
  const { user } = useAuth();
  const [mensajes, setMensajes] = useState([]);
  const [cargando, setCargando] = useState(true);
  const [hayMas, setHayMas] = useState(false);
  const [borrador, setBorrador] = useState('');
  const [enviando, setEnviando] = useState(false);
  const [subiendo, setSubiendo] = useState(false);
  const [otroEscribe, setOtroEscribe] = useState(false);
  const [menuMsg, setMenuMsg] = useState(null);
  const [ops] = useState(leerOpsChat);
  const fin = useRef(null);
  const caja = useRef(null);
  const area = useRef(null);
  const escribiendoTimer = useRef(null);

  async function cargar(antes = 0) {
    try {
      const r = await api.get(`/api/groups/${grupo.id}/messages${antes ? `?antes=${antes}` : ''}`);
      const lista = r.mensajes || [];
      setHayMas(!!r.hay_mas);
      setMensajes((prev) => (antes ? [...lista, ...prev] : lista));
      const ultimo = lista[lista.length - 1]?.id;
      if (!antes && ultimo && onVistos) onVistos(ultimo);
    } catch (e) {
      if (e.status === 403) onNecesitaEntrar();
      else avisoError(e);
    } finally {
      setCargando(false);
    }
  }

  useEffect(() => {
    setCargando(true);
    if (esMiembro) cargar();
    else setCargando(false);
  }, [grupo.id, esMiembro]);

  useEffect(() => {
    if (!esMiembro) return undefined;
    const off = realtime.on((ev) => {
      if (Number(ev.group_id) !== Number(grupo.id)) return;
      if (ev.type === 'group_message') {
        if (ev.message && Number(ev.message.user_id) === Number(user?.id)) return;
        setMensajes((prev) => (prev.some((m) => m.id === ev.message.id) ? prev : [...prev, ev.message]));
        if (onVistos) onVistos(ev.message.id);
        setOtroEscribe(false);
      } else if (ev.type === 'group_message_deleted') {
        setMensajes((prev) => prev.filter((m) => m.id !== ev.message_id));
      } else if (ev.type === 'typing' && Number(ev.user_id) !== Number(user?.id)) {
        setOtroEscribe(true);
        clearTimeout(escribiendoTimer.current);
        escribiendoTimer.current = setTimeout(() => setOtroEscribe(false), 2500);
      }
    });
    return () => { off(); clearTimeout(escribiendoTimer.current); };
  }, [grupo.id, esMiembro, user?.id]);

  useEffect(() => {
    if (fin.current) fin.current.scrollIntoView({ behavior: 'smooth', block: 'end' });
  }, [mensajes.length, otroEscribe]);

  async function mandar(e) {
    if (e) e.preventDefault();
    const limpio = borrador.trim();
    if (!limpio || enviando) return;
    setEnviando(true);
    try {
      const creado = await api.post(`/api/groups/${grupo.id}/messages`, { content: limpio });
      setMensajes((prev) => [...prev, creado]);
      if (onVistos) onVistos(creado.id);
      setBorrador('');
    } catch (err) {
      avisoError(err);
    } finally {
      setEnviando(false);
    }
  }

  async function enviarNota(blob, duracionMs) {
    setSubiendo(true);
    try {
      const subida = await uploadMedia('audio', blob);
      const creado = await api.post(`/api/groups/${grupo.id}/messages`, {
        audio_url: subida.url,
        duracion_ms: duracionMs,
      });
      setMensajes((prev) => [...prev, creado]);
      if (onVistos) onVistos(creado.id);
    } catch (err) {
      avisoError(err);
    } finally {
      setSubiendo(false);
    }
  }

  async function borrar(m) {
    const ok = await window.confirm('¿Borrar este mensaje del chat?');
    if (!ok) return;
    try {
      await api.del(`/api/groups/${grupo.id}/messages/${m.id}`);
      setMensajes((prev) => prev.filter((x) => x.id !== m.id));
      setMenuMsg(null);
    } catch (e) {
      toast.err(e.message || 'No se pudo borrar');
    }
  }

  function responder(m) {
    setBorrador((b) => `${b ? `${b} ` : ''}@${m.username} `);
    setMenuMsg(null);
    if (area.current) area.current.focus();
  }

  async function copiar(m) {
    try {
      await navigator.clipboard.writeText(m.content || '');
      toast.ok('Mensaje copiado');
    } catch {
      toast.info(m.content || '');
    }
    setMenuMsg(null);
  }

  if (!esMiembro) {
    return (
      <div className="empty">
        <IconChat />
        <h3>El chat es para los miembros</h3>
        <p>Entra al grupo y podrás leer y escribir en el chat con los demás.</p>
      </div>
    );
  }

  const clases = [
    'chat-grupo',
    ops.texto === 'grande' ? 'chat-texto-grande' : '',
    ops.fondo === 'suave' ? 'chat-fondo-suave' : '',
    ops.denso === 'si' ? 'chat-denso' : '',
  ].filter(Boolean).join(' ');

  return (
    <div className={clases}>
      <div className="chat-grupo-mensajes" ref={caja}>
        {cargando ? <div className="spinner" /> : null}

        {!cargando && mensajes.length === 0 ? (
          <div className="empty">
            <IconChat />
            <h3>Empieza el chat</h3>
            <p>Escribe el primer mensaje o manda una nota de voz a {grupo.name}.</p>
          </div>
        ) : null}

        {hayMas && !cargando ? (
          <button type="button" className="btn btn-ghost btn-sm ver-mas-chat" onClick={() => cargar(mensajes[0]?.id)}>
            Ver mensajes anteriores
          </button>
        ) : null}

        {mensajes.map((m, i) => {
          const mio = Number(m.user_id) === Number(user?.id);
          const anterior = mensajes[i - 1];
          const mismo = anterior && Number(anterior.user_id) === Number(m.user_id)
            && (new Date(m.created_at) - new Date(anterior.created_at)) < 5 * 60 * 1000;
          const puedoBorrar = mio || !!grupo.mando;
          return (
            <div className={`fila-chat${mio ? ' mia' : ''}${mismo ? ' seguida' : ''}`} key={m.id}>
              {!mio && !mismo ? (
                <a className="avatar-chat" href={`#/user/${m.username}`} title={m.display_name || m.username}>
                  {m.avatar_url ? <img src={imgUrl(m.avatar_url)} alt="" /> : iniciales(m)}
                </a>
              ) : null}
              {!mio && mismo ? <span className="hueco-avatar" aria-hidden="true" /> : null}
              <div className="burbuja-chat">
                {!mio && !mismo ? <span className="quien">{m.display_name || m.username}</span> : null}
                {m.audio_url ? <AudioMensaje url={m.audio_url} duracionMs={m.duracion_ms} mio={mio} /> : null}
                {m.content ? <span className="texto-chat">{m.content}</span> : null}
                <span className="pie-burbuja">
                  <span className="hora-chat">{horaMensaje(m.created_at)}</span>
                  <button
                    type="button"
                    className="mas-burbuja"
                    aria-label="Opciones del mensaje"
                    onClick={() => setMenuMsg(menuMsg === m.id ? null : m.id)}
                  >
                    <IconMore />
                  </button>
                </span>
                {menuMsg === m.id ? (
                  <div className="acciones-burbuja" role="menu">
                    <button type="button" onClick={() => responder(m)}>
                      <IconResponder /> Responder
                    </button>
                    {m.content ? (
                      <button type="button" onClick={() => copiar(m)}>
                        <IconCopy /> Copiar
                      </button>
                    ) : null}
                    {puedoBorrar ? (
                      <button type="button" className="peligro" onClick={() => borrar(m)}>
                        <IconTrash /> Borrar
                      </button>
                    ) : null}
                  </div>
                ) : null}
              </div>
            </div>
          );
        })}

        {otroEscribe ? (
          <span className="typing" aria-live="polite">escribiendo<i /><i /><i /></span>
        ) : null}
        <div ref={fin} />
      </div>

      <form className="chat-input chat-grupo-input" onSubmit={mandar}>
        <textarea
          ref={area}
          className="textarea"
          rows={1}
          placeholder="Escribe en el chat del grupo…"
          aria-label="Escribe un mensaje para el grupo"
          value={borrador}
          maxLength={2000}
          onChange={(e) => {
            setBorrador(e.target.value);
            realtime.send({ type: 'typing', group_id: grupo.id });
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); mandar(e); }
          }}
        />
        {puedeGrabar() ? (
          <Grabador onListo={enviarNota} disabled={subiendo || enviando} />
        ) : null}
        <button className="btn btn-primary" disabled={enviando || subiendo || !borrador.trim()} aria-label="Enviar mensaje">
          <IconSend />
        </button>
      </form>
      {subiendo ? <p className="muted subiendo-nota">Subiendo la nota de voz…</p> : null}
    </div>
  );
}

/* ------------------------------------------------- pantalla de ajustes chat */

export function AjustesChatGrupo() {
  const [ops, setOps] = useState(leerOpsChat);

  function fijar(clave, valor) {
    const nuevas = { ...ops, [clave]: valor };
    setOps(nuevas);
    guardarOpsChat(nuevas);
  }

  return (
    <>
      <section className="card ajustes-bloque">
        <div className="titulo">Tamaño del texto</div>
        <div className="fila-ajuste">
          <span className="texto">
            <b>Letra de los mensajes</b>
            <small>Elige cómo de grande quieres leer el chat en este teléfono.</small>
          </span>
          <div className="tabs">
            <button type="button" className={ops.texto === 'normal' ? 'active' : ''} onClick={() => fijar('texto', 'normal')}>
              Normal
            </button>
            <button type="button" className={ops.texto === 'grande' ? 'active' : ''} onClick={() => fijar('texto', 'grande')}>
              Grande
            </button>
          </div>
        </div>
      </section>

      <section className="card ajustes-bloque">
        <div className="titulo">Fondo del chat</div>
        <div className="fila-ajuste">
          <span className="texto">
            <b>Color de fondo</b>
            <small>Neutro para menos distracción, suave para diferenciar el chat.</small>
          </span>
          <div className="tabs">
            <button type="button" className={ops.fondo === 'neutro' ? 'active' : ''} onClick={() => fijar('fondo', 'neutro')}>
              Neutro
            </button>
            <button type="button" className={ops.fondo === 'suave' ? 'active' : ''} onClick={() => fijar('fondo', 'suave')}>
              Suave
            </button>
          </div>
        </div>
      </section>

      <section className="card ajustes-bloque">
        <div className="titulo">Densidad</div>
        <div className="fila-ajuste">
          <span className="texto">
            <b>Espacio entre mensajes</b>
            <small>Compacto muestra más mensajes en la misma pantalla.</small>
          </span>
          <div className="tabs">
            <button type="button" className={ops.denso === 'no' ? 'active' : ''} onClick={() => fijar('denso', 'no')}>
              Cómodo
            </button>
            <button type="button" className={ops.denso === 'si' ? 'active' : ''} onClick={() => fijar('denso', 'si')}>
              Compacto
            </button>
          </div>
        </div>
      </section>

      <p className="muted small">
        Estas preferencias se guardan solo en este teléfono y aplican al chat de todos los grupos.
      </p>
    </>
  );
}

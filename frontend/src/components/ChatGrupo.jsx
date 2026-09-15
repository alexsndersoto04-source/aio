// Moon — Chat del grupo
// ============================================================
// Mensajes cortos entre los miembros del grupo: texto o nota de voz, en vivo
// por WebSocket, con «escribiendo…» y borrado (quien lo escribió o quien creó
// el grupo). Solo entran los miembros, igual que en la vida real.

import React, { useEffect, useRef, useState } from 'react';
import { api, uploadMedia, imgUrl } from '../api.js';
import { useAuth } from '../auth.jsx';
import { realtime } from '../realtime.js';
import { horaMensaje } from '../utils.js';
import { avisoError, toast } from '../ui.js';
import { IconSend, IconTrash, IconChat } from './Icons.jsx';
import { Grabador, AudioMensaje, puedeGrabar } from './NotaVoz.jsx';


function iniciales(u) {
  return (u.display_name || u.username || '?').slice(0, 2).toUpperCase();
}

export default function ChatGrupo({ grupo, esMiembro, onNecesitaEntrar }) {
  const { user } = useAuth();
  const [mensajes, setMensajes] = useState([]);
  const [cargando, setCargando] = useState(true);
  const [hayMas, setHayMas] = useState(false);
  const [escribiendo, setEscribiendo] = useState(false);
  const [borrador, setBorrador] = useState('');
  const [enviando, setEnviando] = useState(false);
  const [subiendo, setSubiendo] = useState(false);
  const [otroEscribe, setOtroEscribe] = useState(false);
  const fin = useRef(null);
  const caja = useRef(null);
  const escribiendoTimer = useRef(null);

  async function cargar(antes = 0) {
    try {
      const r = await api.get(`/api/groups/${grupo.id}/messages${antes ? `?antes=${antes}` : ''}`);
      const lista = r.mensajes || [];
      setHayMas(!!r.hay_mas);
      setMensajes((prev) => (antes ? [...lista, ...prev] : lista));
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

  // En vivo: mensajes nuevos, borrados y «escribiendo…» de los demás.
  useEffect(() => {
    if (!esMiembro) return undefined;
    const off = realtime.on((ev) => {
      if (Number(ev.group_id) !== Number(grupo.id)) return;
      if (ev.type === 'group_message') {
        if (ev.message && Number(ev.message.user_id) === Number(user?.id)) return;
        setMensajes((prev) => (prev.some((m) => m.id === ev.message.id) ? prev : [...prev, ev.message]));
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
    } catch (e) {
      toast.err(e.message || 'No se pudo borrar');
    }
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

  return (
    <div className="chat-grupo">
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
                <span className="hora-chat">{horaMensaje(m.created_at)}</span>
                {mio ? (
                  <button type="button" className="borrar-chat" onClick={() => borrar(m)} aria-label="Borrar mensaje" title="Borrar">
                    <IconTrash />
                  </button>
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

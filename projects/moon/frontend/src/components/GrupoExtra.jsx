// Moon — Lo que le faltaba al grupo (tanda 3)
// ============================================================
// Seis piezas, cada una en su sitio:
//   · Anuncio: una franja fina arriba del grupo, como el mensaje fijado.
//   · Eventos: día, hora, lugar y «voy / quizás / no voy» con quién va.
//   · Archivos: lo que se comparte, con quién lo subió y cuánto pesa.
//   · Reglas: el texto del grupo y las preguntas para entrar.
//   · Solicitudes: quien administra acepta o rechaza, viendo las respuestas.
//   · Sanciones y registro: avisos, silencios, expulsiones y qué ha pasado.
//
// Todo sigue el mismo lenguaje: superficie continua, filos de 1 px y el
// acento aurora solo donde hay algo vivo.

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api, uploadMedia } from '../api.js';
import { toast, avisoError, confirmar, pedirTexto } from '../ui.js';
import Avatar from './Avatar.jsx';
import {
  IconPin, IconCalendar, IconFile, IconTrash, IconCheck, IconX, IconPlus,
  IconShield, IconSend, IconEdit, IconUsers, IconClock,
} from './Icons.jsx';

const fechaLarga = (iso) => {
  try {
    return new Date(iso).toLocaleString('es', { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });
  } catch { return ''; }
};
const pesoLegible = (bytes) => {
  const n = Number(bytes || 0);
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
};
const hace = (iso) => {
  const ms = Date.now() - new Date(iso).getTime();
  const min = Math.round(ms / 60000);
  if (min < 1) return 'ahora';
  if (min < 60) return `hace ${min} min`;
  const h = Math.round(min / 60);
  if (h < 24) return `hace ${h} h`;
  const d = Math.round(h / 24);
  return d === 1 ? 'ayer' : `hace ${d} días`;
};

// ---------- Anuncio fijado ----------
export function AnuncioGrupo({ texto, cuando, puedoQuitar, onQuitar }) {
  if (!texto) return null;
  return (
    <div className="anuncio-grupo">
      <IconPin />
      <span className="texto">
        <b>Anuncio del grupo{cuando ? ` · ${hace(cuando)}` : ''}</b>
        <span className="cuerpo">{texto}</span>
      </span>
      {puedoQuitar ? (
        <button type="button" className="icon-btn" onClick={onQuitar} aria-label="Quitar el anuncio" title="Quitar el anuncio">
          <IconX />
        </button>
      ) : null}
    </div>
  );
}

// ---------- Eventos ----------
export function EventosGrupo({ grupoId, mando, soyMiembro }) {
  const [eventos, setEventos] = useState(null);
  const [creando, setCreando] = useState(false);
  const [quien, setQuien] = useState(null);
  const [form, setForm] = useState({ title: '', about: '', place: '', dia: '', hora: '' });

  const cargar = useCallback(async () => {
    try {
      const r = await api.get(`/api/groups/${grupoId}/events`);
      setEventos(r.eventos || []);
    } catch (e) {
      avisoError(e);
      setEventos([]);
    }
  }, [grupoId]);

  useEffect(() => { cargar(); }, [cargar]);

  async function crear() {
    if (!form.title.trim() || !form.dia) { toast.err('Ponle título y día al evento'); return; }
    try {
      const starts = new Date(`${form.dia}T${form.hora || '18:00'}`).toISOString();
      await api.post(`/api/groups/${grupoId}/events`, {
        title: form.title, about: form.about, place: form.place, starts_at: starts,
      });
      setForm({ title: '', about: '', place: '', dia: '', hora: '' });
      setCreando(false);
      toast.ok('Evento creado');
      cargar();
    } catch (e) { avisoError(e); }
  }

  async function responder(ev, estado) {
    try {
      const listo = await api.post(`/api/groups/${grupoId}/events/${ev.id}/asistir`, { estado });
      setEventos((prev) => prev.map((x) => (x.id === listo.id ? listo : x)));
    } catch (e) { avisoError(e); }
  }

  async function borrar(ev) {
    const ok = await confirmar({ title: '¿Borrar el evento?', message: `«${ev.title}» desaparecerá para todos.`, confirmText: 'Borrar', danger: true });
    if (!ok) return;
    try {
      await api.del(`/api/groups/${grupoId}/events/${ev.id}`);
      setEventos((prev) => prev.filter((x) => x.id !== ev.id));
      toast.ok('Evento borrado');
    } catch (e) { avisoError(e); }
  }

  const proximos = (eventos || []).filter((e) => !e.pasado);
  const pasados = (eventos || []).filter((e) => e.pasado);

  return (
    <div className="bloque-grupo">
      <div className="cabecera-bloque">
        <h2><IconCalendar /> Eventos</h2>
        {mando && soyMiembro ? (
          <button type="button" className="btn btn-outline btn-sm" onClick={() => setCreando((v) => !v)}>
            <IconPlus /> {creando ? 'Cancelar' : 'Nuevo evento'}
          </button>
        ) : null}
      </div>

      {creando ? (
        <div className="form-evento">
          <input className="input" placeholder="Título del evento" value={form.title} maxLength={120}
            onChange={(e) => setForm({ ...form, title: e.target.value })} />
          <div className="row" style={{ gap: 8 }}>
            <input className="input" type="date" value={form.dia} onChange={(e) => setForm({ ...form, dia: e.target.value })} />
            <input className="input" type="time" value={form.hora} onChange={(e) => setForm({ ...form, hora: e.target.value })} />
          </div>
          <input className="input" placeholder="Dónde (opcional)" value={form.place} maxLength={160}
            onChange={(e) => setForm({ ...form, place: e.target.value })} />
          <textarea className="textarea" rows={2} placeholder="De qué se trata (opcional)" value={form.about} maxLength={600}
            onChange={(e) => setForm({ ...form, about: e.target.value })} />
          <button type="button" className="btn btn-primary btn-sm" onClick={crear}>
            <IconCheck /> Crear evento
          </button>
        </div>
      ) : null}

      {eventos === null ? <p className="muted small" style={{ padding: 12 }}>Cargando eventos…</p> : null}
      {eventos && eventos.length === 0 ? (
        <div className="empty">
          <IconCalendar />
          <h3>No hay eventos todavía</h3>
          <p>{mando ? 'Crea el primero: día, hora y para qué.' : 'Cuando se organice algo, aparecerá aquí.'}</p>
        </div>
      ) : null}

      {proximos.map((ev) => (
        <article className="evento" key={ev.id}>
          <div className="fecha">
            <b>{new Date(ev.starts_at).toLocaleDateString('es', { day: 'numeric' })}</b>
            <span>{new Date(ev.starts_at).toLocaleDateString('es', { month: 'short' })}</span>
          </div>
          <div className="cuerpo">
            <b className="titulo">{ev.title}</b>
            <span className="muted small">
              {fechaLarga(ev.starts_at)}{ev.place ? ` · ${ev.place}` : ''} · lo propuso {ev.autor}
            </span>
            {ev.about ? <p className="sobre">{ev.about}</p> : null}
            <div className="respuestas">
              <button type="button" className={ev.mi_estado === 'voy' ? 'activa' : ''} onClick={() => responder(ev, 'voy')}>
                <IconCheck /> Voy {ev.voy > 0 ? <i>{ev.voy}</i> : null}
              </button>
              <button type="button" className={ev.mi_estado === 'quizas' ? 'activa' : ''} onClick={() => responder(ev, 'quizas')}>
                Quizás {ev.quizas > 0 ? <i>{ev.quizas}</i> : null}
              </button>
              <button type="button" className={ev.mi_estado === 'no' ? 'activa' : ''} onClick={() => responder(ev, 'no')}>
                No voy {ev.no_van > 0 ? <i>{ev.no_van}</i> : null}
              </button>
              {ev.voy + ev.quizas > 0 ? (
                <button type="button" className="btn-ghost" onClick={() => setQuien(ev)}>Ver quién va</button>
              ) : null}
              {mando || ev.mio ? (
                <button type="button" className="icon-btn peligro" onClick={() => borrar(ev)} aria-label="Borrar el evento">
                  <IconTrash />
                </button>
              ) : null}
            </div>
          </div>
        </article>
      ))}

      {pasados.length > 0 ? (
        <>
          <h3 className="titulo-pequeno"><IconClock /> Ya pasaron</h3>
          {pasados.map((ev) => (
            <article className="evento pasado" key={ev.id}>
              <div className="fecha"><b>{new Date(ev.starts_at).toLocaleDateString('es', { day: 'numeric' })}</b>
                <span>{new Date(ev.starts_at).toLocaleDateString('es', { month: 'short' })}</span></div>
              <div className="cuerpo">
                <b className="titulo">{ev.title}</b>
                <span className="muted small">{fechaLarga(ev.starts_at)}{ev.place ? ` · ${ev.place}` : ''} · fueron {ev.voy}</span>
              </div>
            </article>
          ))}
        </>
      ) : null}

      {quien ? <QuienVa grupoId={grupoId} evento={quien} onCerrar={() => setQuien(null)} /> : null}
    </div>
  );
}

function QuienVa({ grupoId, evento, onCerrar }) {
  const [gente, setGente] = useState(null);
  useEffect(() => {
    api.get(`/api/groups/${grupoId}/events/${evento.id}/asistentes`)
      .then((r) => setGente(r.asistentes || []))
      .catch(() => setGente([]));
  }, [grupoId, evento.id]);
  const grupos = [['voy', 'Van'], ['quizas', 'Quizás'], ['no', 'No van']];
  return (
    <>
      <span className="hoja-fondo" role="presentation" onClick={onCerrar} />
      <div className="hoja-reenviar" role="dialog" aria-label="Quién va al evento">
        <div className="cabecera">
          <b>{evento.title}</b>
          <button type="button" className="icon-btn" onClick={onCerrar} aria-label="Cerrar"><IconX /></button>
        </div>
        {gente === null ? <p className="muted small">Cargando…</p> : null}
        {gente && gente.length === 0 ? <p className="muted small">Todavía no ha respondido nadie.</p> : null}
        <div className="lista">
          {grupos.map(([clave, titulo]) => {
            const delGrupo = (gente || []).filter((g) => g.estado === clave);
            if (delGrupo.length === 0) return null;
            return (
              <div key={clave} className="grupo-asistentes">
                <span className="titulo-pequeno">{titulo} · {delGrupo.length}</span>
                {delGrupo.map((g) => (
                  <a key={g.id} href={`#/user/${g.id}`} className="fila-persona" onClick={onCerrar}>
                    <Avatar user={g} size="sm" />
                    <span className="quien"><b>{g.display_name}</b><small>@{g.username}</small></span>
                  </a>
                ))}
              </div>
            );
          })}
        </div>
      </div>
    </>
  );
}

// ---------- Archivos ----------
export function ArchivosGrupo({ grupoId, soyMiembro }) {
  const [archivos, setArchivos] = useState(null);
  const [subiendo, setSubiendo] = useState(false);
  const entrada = useRef(null);

  const cargar = useCallback(async () => {
    try {
      const r = await api.get(`/api/groups/${grupoId}/files`);
      setArchivos(r.archivos || []);
    } catch (e) { avisoError(e); setArchivos([]); }
  }, [grupoId]);

  useEffect(() => { cargar(); }, [cargar]);

  async function subir(ev) {
    const archivo = ev.target.files?.[0];
    if (!archivo) return;
    setSubiendo(true);
    try {
      const subido = await uploadMedia(archivo, 'post');
      const url = subido?.url || subido?.original_url;
      if (!url) throw new Error('No se pudo subir el archivo');
      const f = await api.post(`/api/groups/${grupoId}/files`, {
        nombre: archivo.name, url, tipo: archivo.type, peso: archivo.size,
      });
      setArchivos((prev) => [f, ...(prev || [])]);
      toast.ok('Archivo compartido');
    } catch (e) {
      avisoError(e);
    } finally {
      setSubiendo(false);
      if (entrada.current) entrada.current.value = '';
    }
  }

  async function borrar(f) {
    try {
      await api.del(`/api/groups/${grupoId}/files/${f.id}`);
      setArchivos((prev) => prev.filter((x) => x.id !== f.id));
      toast.ok('Archivo quitado');
    } catch (e) { avisoError(e); }
  }

  return (
    <div className="bloque-grupo">
      <div className="cabecera-bloque">
        <h2><IconFile /> Archivos</h2>
        {soyMiembro ? (
          <>
            <input ref={entrada} type="file" accept="image/*,audio/*" hidden onChange={subir} />
            <button type="button" className="btn btn-outline btn-sm" disabled={subiendo} onClick={() => entrada.current?.click()}>
              <IconPlus /> {subiendo ? 'Subiendo…' : 'Compartir'}
            </button>
          </>
        ) : null}
      </div>

      {archivos === null ? <p className="muted small" style={{ padding: 12 }}>Cargando archivos…</p> : null}
      {archivos && archivos.length === 0 ? (
        <div className="empty">
          <IconFile />
          <h3>Nadie ha compartido archivos</h3>
          <p>{soyMiembro ? 'Sube una imagen o una nota de voz para que quede a mano.' : 'Entra al grupo para compartir.'}</p>
        </div>
      ) : null}

      {(archivos || []).map((f) => (
        <div className="fila-archivo" key={f.id}>
          {f.tipo?.startsWith('image/') ? (
            <a className="miniatura" href={f.url} target="_blank" rel="noreferrer"><img src={f.url} alt="" /></a>
          ) : (
            <span className="miniatura icono"><IconFile /></span>
          )}
          <span className="texto">
            <b className="ellipsis">{f.nombre}</b>
            <small className="muted">{f.autor} · {pesoLegible(f.peso)} · {hace(f.created_at)}</small>
          </span>
          <a className="btn btn-ghost btn-sm" href={f.url} target="_blank" rel="noreferrer">Abrir</a>
          {f.puedo_borrar ? (
            <button type="button" className="icon-btn peligro" onClick={() => borrar(f)} aria-label="Quitar el archivo" title="Quitar">
              <IconTrash />
            </button>
          ) : null}
        </div>
      ))}
    </div>
  );
}

// ---------- Reglas y preguntas para entrar ----------
export function ReglasGrupo({ grupoId, grupo, mando, onCambio }) {
  const [reglas, setReglas] = useState(grupo?.rules || '');
  const [guardando, setGuardando] = useState(false);
  const [preguntas, setPreguntas] = useState(grupo?.join_questions || []);
  const [nueva, setNueva] = useState('');

  async function guardar(patch) {
    setGuardando(true);
    try {
      const r = await api.patch(`/api/groups/${grupoId}/rules`, patch);
      setReglas(r.rules || '');
      setPreguntas(r.join_questions || []);
      onCambio?.(r);
      toast.ok('Guardado');
    } catch (e) { avisoError(e); } finally { setGuardando(false); }
  }

  return (
    <div className="bloque-grupo">
      <div className="cabecera-bloque">
        <h2><IconShield /> Reglas del grupo</h2>
      </div>
      {mando ? (
        <>
          <textarea
            className="textarea" rows={6} value={reglas} maxLength={2000}
            placeholder={'1. Respeto por encima de todo\n2. Nada de spam\n3. Un tema por publicación'}
            onChange={(e) => setReglas(e.target.value)}
          />
          <div className="row" style={{ gap: 8, justifyContent: 'flex-end' }}>
            <button type="button" className="btn btn-primary btn-sm" disabled={guardando} onClick={() => guardar({ rules: reglas })}>
              <IconCheck /> Guardar reglas
            </button>
          </div>
        </>
      ) : reglas ? (
        <p className="texto-reglas">{reglas}</p>
      ) : (
        <div className="empty">
          <IconShield />
          <h3>Este grupo no ha escrito sus reglas</h3>
          <p>Lo normal: respeto, nada de spam y un tema por publicación.</p>
        </div>
      )}

      {mando ? (
        <section className="sub-bloque">
          <h3 className="titulo-pequeno">Preguntas para entrar</h3>
          <p className="muted small">
            Si pones preguntas, quien quiera entrar deja de entrar solo: se le piden y tú decides.
          </p>
          {preguntas.map((q, i) => (
            <div className="fila-pregunta" key={`${q}-${i}`}>
              <span className="numero">{i + 1}</span>
              <span className="texto ellipsis">{q}</span>
              <button
                type="button" className="icon-btn" aria-label="Quitar la pregunta"
                onClick={() => guardar({ join_questions: preguntas.filter((_, j) => j !== i) })}
              >
                <IconX />
              </button>
            </div>
          ))}
          <div className="row" style={{ gap: 8 }}>
            <input
              className="input" placeholder="Nueva pregunta (por ejemplo: ¿Por qué quieres entrar?)"
              value={nueva} maxLength={160} onChange={(e) => setNueva(e.target.value)}
            />
            <button
              type="button" className="btn btn-outline btn-sm" disabled={!nueva.trim() || preguntas.length >= 5}
              onClick={() => { guardar({ join_questions: [...preguntas, nueva.trim()] }); setNueva(''); }}
            >
              <IconPlus /> Añadir
            </button>
          </div>
          {preguntas.length >= 5 ? <p className="muted small">Máximo cinco preguntas.</p> : null}
        </section>
      ) : null}
    </div>
  );
}

// ---------- Solicitudes para entrar ----------
export function SolicitudesGrupo({ grupoId }) {
  const [solicitudes, setSolicitudes] = useState(null);

  const cargar = useCallback(async () => {
    try {
      const r = await api.get(`/api/groups/${grupoId}/solicitudes`);
      setSolicitudes(r.solicitudes || []);
    } catch { setSolicitudes([]); }
  }, [grupoId]);

  useEffect(() => { cargar(); }, [cargar]);

  async function decidir(s, aprobar) {
    try {
      await api.post(`/api/groups/${grupoId}/solicitudes/${s.id}`, { aprobar });
      setSolicitudes((prev) => prev.filter((x) => x.id !== s.id));
      toast.ok(aprobar ? `Aceptaste a ${s.display_name}` : 'Solicitud rechazada');
    } catch (e) { avisoError(e); }
  }

  if (solicitudes === null || solicitudes.length === 0) return null;

  return (
    <section className="sub-bloque">
      <h3 className="titulo-pequeno"><IconUsers /> Quieren entrar · {solicitudes.length}</h3>
      {solicitudes.map((s) => (
        <div className="solicitud" key={s.id}>
          <div className="quien">
            <Avatar user={s} size="sm" />
            <span className="texto">
              <b>{s.display_name}</b>
              <small className="muted">@{s.username} · {hace(s.created_at)}</small>
            </span>
          </div>
          <ol className="respuestas">
            {(s.answers || []).map((a, i) => <li key={i}>{a}</li>)}
          </ol>
          <div className="acciones">
            <button type="button" className="btn btn-primary btn-sm" onClick={() => decidir(s, true)}>
              <IconCheck /> Aceptar
            </button>
            <button type="button" className="btn btn-ghost btn-sm" onClick={() => decidir(s, false)}>
              <IconX /> Rechazar
            </button>
          </div>
        </div>
      ))}
    </section>
  );
}

// ---------- Sanciones y registro ----------
export function SancionesGrupo({ grupoId, miembros }) {
  const [sanciones, setSanciones] = useState(null);
  const [registro, setRegistro] = useState(null);
  const [eligiendo, setEligiendo] = useState(false);

  const cargar = useCallback(async () => {
    try {
      const r = await api.get(`/api/groups/${grupoId}/sanciones`);
      setSanciones(r.sanciones || []);
      const g = await api.get(`/api/groups/${grupoId}/registro`);
      setRegistro(g.registro || []);
    } catch { /* si no hay mando, no se muestra nada */ }
  }, [grupoId]);

  useEffect(() => { cargar(); }, [cargar]);

  async function poner(m, tipo) {
    const motivo = await pedirTexto({
      title: tipo === 'aviso' ? 'Aviso' : tipo === 'silencio' ? 'Silencio temporal' : 'Expulsar del grupo',
      label: 'Motivo (lo verá la persona)',
      placeholder: 'Por ejemplo: publicó dos veces lo mismo',
    });
    if (motivo === null) return;
    try {
      await api.post(`/api/groups/${grupoId}/sanciones`, {
        user_id: m.id, tipo, motivo: motivo || '', minutos: tipo === 'silencio' ? 60 : 0,
      });
      setEligiendo(false);
      toast.ok(tipo === 'aviso' ? 'Aviso enviado' : tipo === 'silencio' ? 'Queda en silencio una hora' : 'Fuera del grupo');
      cargar();
    } catch (e) { avisoError(e); }
  }

  async function levantar(s) {
    try {
      await api.del(`/api/groups/${grupoId}/sanciones/${s.id}`);
      setSanciones((prev) => prev.filter((x) => x.id !== s.id));
      toast.ok('Sanción levantada');
    } catch (e) { avisoError(e); }
  }

  const etiqueta = { aviso: 'Aviso', silencio: 'Silencio', expulsion: 'Expulsión' };

  return (
    <section className="sub-bloque">
      <h3 className="titulo-pequeno"><IconShield /> Sanciones</h3>
      <button type="button" className="btn btn-outline btn-sm" onClick={() => setEligiendo((v) => !v)}>
        <IconPlus /> {eligiendo ? 'Cancelar' : 'Sancionar a alguien'}
      </button>
      {eligiendo ? (
        <div className="lista-miembros-sancion">
          {(miembros || []).filter((m) => m.papel !== 'owner').map((m) => (
            <div className="fila-persona" key={m.id}>
              <Avatar user={m} size="sm" />
              <span className="quien"><b>{m.display_name}</b><small>@{m.username}</small></span>
              <button type="button" className="btn btn-ghost btn-sm" onClick={() => poner(m, 'aviso')}>Aviso</button>
              <button type="button" className="btn btn-ghost btn-sm" onClick={() => poner(m, 'silencio')}>Silencio</button>
              <button type="button" className="btn btn-ghost btn-sm peligro" onClick={() => poner(m, 'expulsion')}>Fuera</button>
            </div>
          ))}
        </div>
      ) : null}

      {(sanciones || []).length === 0 ? (
        <p className="muted small">Nadie tiene sanciones en este grupo.</p>
      ) : (
        (sanciones || []).map((s) => (
          <div className="fila-sancion" key={s.id}>
            <Avatar user={s} size="sm" />
            <span className="texto">
              <b>{s.display_name} <i className={`etiqueta ${s.tipo}`}>{etiqueta[s.tipo]}</i></b>
              <small className="muted">
                {s.motivo || 'sin motivo escrito'} · {hace(s.created_at)}
                {s.tipo === 'silencio' && s.vigente ? ' · sigue en silencio' : ''}
              </small>
            </span>
            {s.vigente && s.tipo !== 'expulsion' ? (
              <button type="button" className="btn btn-ghost btn-sm" onClick={() => levantar(s)}>Levantar</button>
            ) : null}
          </div>
        ))
      )}

      <h3 className="titulo-pequeno"><IconClock /> Registro</h3>
      {(registro || []).length === 0 ? <p className="muted small">Todavía no hay nada anotado.</p> : null}
      <ul className="lista-registro">
        {(registro || []).slice(0, 25).map((r) => (
          <li key={r.id}>
            <span className="punto" aria-hidden="true" />
            <span className="texto">
              <b>{r.display_name}</b> {r.detalle || r.accion.replace(/_/g, ' ')}
              <small className="muted"> · {hace(r.created_at)}</small>
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}

// ---------- Hoja para pedir entrar con preguntas ----------
export function HojaEntrar({ grupo, onCerrar, onDentro }) {
  const [respuestas, setRespuestas] = useState(() => (grupo.join_questions || []).map(() => ''));
  const [enviando, setEnviando] = useState(false);
  const preguntas = grupo.join_questions || [];

  async function enviar() {
    setEnviando(true);
    try {
      const r = await api.post(`/api/groups/${grupo.id}/solicitar`, { answers: respuestas });
      if (r.estado === 'dentro') {
        toast.ok('Ya estás dentro');
        onDentro?.();
      } else {
        toast.info('Tu solicitud quedó pendiente: te avisamos');
        onDentro?.();
      }
      onCerrar();
    } catch (e) { avisoError(e); } finally { setEnviando(false); }
  }

  return (
    <>
      <span className="hoja-fondo" role="presentation" onClick={onCerrar} />
      <div className="hoja-reenviar" role="dialog" aria-label="Preguntas para entrar al grupo">
        <div className="cabecera">
          <b>Para entrar en {grupo.name}</b>
          <button type="button" className="icon-btn" onClick={onCerrar} aria-label="Cerrar"><IconX /></button>
        </div>
        <p className="muted small">Contesta y quien administra el grupo decide.</p>
        {preguntas.map((q, i) => (
          <label className="campo" key={`${q}-${i}`}>
            <span className="etiqueta-pregunta">{q}</span>
            <textarea
              className="textarea" rows={2} maxLength={400} value={respuestas[i]}
              onChange={(e) => setRespuestas((prev) => prev.map((v, j) => (j === i ? e.target.value : v)))}
            />
          </label>
        ))}
        <button type="button" className="btn btn-primary" disabled={enviando} onClick={enviar}>
          <IconSend /> {enviando ? 'Enviando…' : 'Pedir entrar'}
        </button>
      </div>
    </>
  );
}

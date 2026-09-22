// Moon — Panel de administración (moderación real)

import React, { useEffect, useRef, useState } from 'react';
import { api, getAccessToken, API_URL } from '../api.js';
import { toast, confirmar, pedirTexto, avisoError } from '../ui.js';
import { useAuth } from '../auth.jsx';
import Avatar from '../components/Avatar.jsx';
import { IconShield, IconBell, IconCheck } from '../components/Icons.jsx';
import { timeAgo } from '../utils.js';

const STATUS_PILL = { active: 'ok', suspended: 'err', deleted: 'info' };
const REPORT_PILL = { open: 'warn', resolved: 'ok', dismissed: 'info' };

function SeccionCopias() {
  const [busy, setBusy] = useState('');
  const [hechas, setHechas] = useState([]);
  const [migrado, setMigrado] = useState(null); // informe de la migración
  const [direccion, setDireccion] = useState(''); // dirección de la base nueva (pegada)
  const [estadoMudanza, setEstadoMudanza] = useState(''); // estado en claro para el usuario

  async function descargar() {
    setBusy('descargar');
    try {
      const res = await fetch(`${API_URL}/api/admin/backup`, {
        headers: { Authorization: `Bearer ${getAccessToken()}` },
      });
      if (!res.ok) throw new Error('El servidor no pudo preparar la copia');
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `copia-moon-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
      setHechas((l) => [new Date().toLocaleString('es-VE'), ...l].slice(0, 5));
      toast.ok('Copia descargada. Guárdala en un sitio seguro.');
    } catch (e) {
      avisoError(e);
    } finally {
      setBusy('');
    }
  }

  async function porCorreo() {
    const ok = await confirmar({
      title: '¿Enviar la copia por correo?',
      message: 'Se enviará el archivo completo a los correos de administración.',
      confirmText: 'Enviar',
    });
    if (!ok) return;
    setBusy('correo');
    try {
      const r = await api.post('/api/admin/backup/correo', {});
      toast.ok(`Copia enviada a ${r.enviadas} correo(s)`);
      setHechas((l) => [new Date().toLocaleString('es-VE') + ' (correo)', ...l].slice(0, 5));
    } catch (e) {
      avisoError(e);
    } finally {
      setBusy('');
    }
  }

  async function migrar() {
    const destino = direccion.trim();
    if (!destino) {
      toast.err('Pega primero la dirección de la base nueva (la de Neon).');
      return;
    }
    const ok = await confirmar({
      title: '¿Migrar TODO y pasar a la base nueva?',
      message:
        'Se copian todos los datos (cuentas, publicaciones, mensajes, grupos y fotos) a la base nueva que pegaste, y la app pasa a usarla. La base nueva debe estar vacía. No se borra nada de la base actual.',
      confirmText: 'Migrar todo',
    });
    if (!ok) return;
    setBusy('migrar');
    setEstadoMudanza('');
    try {
      let host = '';
      try { host = new URL(destino).hostname; } catch { host = ''; }
      setEstadoMudanza('Copiando todo a la base nueva… puede tardar un par de minutos.');
      const r = await api.post('/api/admin/migrate/activar', { destination: destino });
      setMigrado(r);
      setEstadoMudanza(
        `Todo copiado y verificado (${r.total_filas ?? 'ya estaba'} filas). La app se está cambiando a la base nueva…`
      );
      // El servicio se reinicia solo; se espera a que vuelva a reportar la base nueva.
      if (host) {
        for (let i = 0; i < 30; i += 1) {
          await new Promise((res) => setTimeout(res, 4000));
          try {
            const h = await fetch(`${API_URL}/api/health`);
            if (!h.ok) continue;
            const j = await h.json();
            if (j.base && j.base.startsWith(host)) {
              setEstadoMudanza('Listo: la app ya usa la base nueva.');
              toast.ok('Mudanza completa: la app ya usa la base nueva.');
              break;
            }
          } catch { /* el servicio sigue reiniciando */ }
        }
      } else {
        setEstadoMudanza('Migración lista. La app pasará a usar la base nueva al reiniciarse.');
      }
    } catch (e) {
      setMigrado(null);
      setEstadoMudanza('');
      avisoError(e);
    } finally {
      setBusy('');
    }
  }

  return (
    <>
      <div className="card ajustes-bloque">
        <div className="titulo">Mudanza a la base nueva (una sola vez)</div>
        <div className="fila-ajuste">
          <span className="icono"><IconShield /></span>
          <span className="texto">
            <b>Usar una base de datos nueva</b>
            <small>Copia todo (cuentas, publicaciones, mensajes, grupos y fotos) a una base de datos nueva —por ejemplo Neon— y la app pasa a usarla sin perder nada. Se hace una sola vez.</small>
          </span>
        </div>
        <div className="fila-ajuste">
          <input
            type="password"
            className="input"
            placeholder="Pega aquí la dirección de la base nueva (empieza por postgres://)"
            value={direccion}
            onChange={(e) => setDireccion(e.target.value)}
            autoComplete="off"
            spellCheck="false"
          />
          <button className="btn btn-primary btn-sm" onClick={migrar} disabled={busy === 'migrar'}>
            {busy === 'migrar' ? 'Copiando todo…' : 'Migrar y usar la base nueva'}
          </button>
        </div>
        {estadoMudanza ? (
          <div className="fila-ajuste">
            <span className="icono">{busy === 'migrar' ? <IconShield /> : <IconCheck />}</span>
            <span className="texto">
              <b>{estadoMudanza}</b>
            </span>
          </div>
        ) : null}
        {migrado ? (
          <div className="fila-ajuste">
            <span className="icono"><IconCheck /></span>
            <span className="texto">
              <b>Migración completada y verificada</b>
              <small>
                {migrado.total_filas ?? 'los datos ya estaban'} filas copiadas · {Object.keys(migrado.tablas || {}).length || '—'} tablas
                {migrado.fotos && migrado.fotos.origen > 0
                  ? ` · fotos ${migrado.fotos.destino === migrado.fotos.origen ? '✓ intactas' : '⚠ revisa'}`
                  : ''}
              </small>
            </span>
          </div>
        ) : null}
      </div>
      <div className="card ajustes-bloque">
        <div className="titulo">Copia de seguridad</div>
        <div className="fila-ajuste">
          <span className="icono"><IconShield /></span>
          <span className="texto">
            <b>Descargar todo ahora</b>
            <small>Un archivo con usuarios, publicaciones, comentarios, mensajes, grupos y estadísticas. Las contraseñas nunca se incluyen.</small>
          </span>
          <button className="btn btn-primary btn-sm" onClick={descargar} disabled={busy === 'descargar'}>
            {busy === 'descargar' ? 'Preparando…' : 'Descargar'}
          </button>
        </div>
        <div className="fila-ajuste">
          <span className="icono"><IconBell /></span>
          <span className="texto">
            <b>Enviarla a mi correo</b>
            <small>Llega como archivo adjunto a las cuentas de administración. Todos los días a las 4 de la mañana se envía sola (si el correo está configurado).</small>
          </span>
          <button className="btn btn-outline btn-sm" onClick={porCorreo} disabled={busy === 'correo'}>
            {busy === 'correo' ? 'Enviando…' : 'Enviar por correo'}
          </button>
        </div>
        {hechas.length > 0 ? (
          <div className="fila-ajuste">
            <span className="icono"><IconCheck /></span>
            <span className="texto">
              <b>Últimas copias de esta sesión</b>
              <small>{hechas.join(' · ')}</small>
            </span>
          </div>
        ) : null}
      </div>
    </>
  );
}

export default function AdminView({ tab }) {
  const { isAdmin } = useAuth();
  const [section, setSection] = useState(tab || 'dashboard');
  const migas = useRef(null);

  // La pestaña activa se trae a la vista (en el teléfono hay que deslizar).
  useEffect(() => {
    const activa = migas.current?.querySelector('button.active');
    if (activa && activa.scrollIntoView) {
      activa.scrollIntoView({ inline: 'center', block: 'nearest', behavior: 'smooth' });
    }
  }, [section]);

  if (!isAdmin) {
    return <div className="card empty"><h3>Acceso restringido</h3><p>Necesitas rol de administrador.</p></div>;
  }

  return (
    <>
      <div className="topbar"><h1>Panel de administración</h1></div>
      <div className="tabs" ref={migas}>
        {[['dashboard', 'Resumen'], ['users', 'Usuarios'], ['reports', 'Reportes'], ['words', 'Palabras'], ['activity', 'Actividad'], ['copias', 'Copias'], ['boveda', 'Bóveda Telegram']].map(([id, label]) => (
          <button key={id} className={section === id ? 'active' : ''} onClick={() => setSection(id)}>{label}</button>
        ))}
      </div>
      {section === 'dashboard' ? <Dashboard /> : null}
      {section === 'users' ? <UsersAdmin /> : null}
      {section === 'reports' ? <ReportsAdmin /> : null}
      {section === 'words' ? <WordsAdmin /> : null}
      {section === 'activity' ? <ActivityAdmin /> : null}
      {section === 'copias' ? <SeccionCopias /> : null}
      {section === 'boveda' ? <SeccionBovedaTelegram /> : null}
    </>
  );
}

function Dashboard() {
  const [d, setD] = useState(null);
  useEffect(() => {
    api.get('/api/admin/dashboard').then(setD).catch(avisoError);
  }, []);
  if (!d) return <div className="spinner" />;
  const stats = [
    ['Usuarios', d.users_total],
    ['Hoy', d.users_new_today],
    ['Últimos 7d', d.users_new_7d],
    ['Publicaciones', d.posts_total],
    ['Posts hoy', d.posts_today],
    ['Comentarios', d.comments_total],
    ['Mensajes', d.messages_total],
    ['Seguimientos', d.follows_total],
    ['Reportes abiertos', d.reports_open],
    ['Suspendidos', d.suspended_users],
  ];
  return (
    <div>
      <div className="stat-grid">
        {stats.map(([label, value]) => (
          <div key={label} className="stat"><b>{value}</b><span>{label}</span></div>
        ))}
      </div>
      <div className="card" style={{ padding: 16 }}>
        <h3 style={{ marginTop: 0 }}>Últimos 30 días</h3>
        <MiniChart />
      </div>
    </div>
  );
}

function MiniChart() {
  const [rows, setRows] = useState([]);
  useEffect(() => {
    api.get('/api/admin/stats').then(setRows).catch(() => {});
  }, []);
  if (rows.length === 0) return <p className="muted">Aún no hay datos diarios.</p>;
  const max = Math.max(1, ...rows.map((r) => r.new_users + r.new_posts));
  return (
    <div style={{ display: 'flex', alignItems: 'flex-end', gap: 3, height: 90 }}>
      {rows.map((r) => (
        <div key={r.stat_date} title={`${r.stat_date}: ${r.new_users} usuarios, ${r.new_posts} posts`}
          style={{ flex: 1, background: 'var(--ink)', opacity: 0.15 + 0.85 * ((r.new_users + r.new_posts) / max), borderRadius: '4px 4px 0 0', height: `${Math.max(6, 90 * ((r.new_users + r.new_posts) / max))}px` }} />
      ))}
    </div>
  );
}

function UsersAdmin() {
  const [q, setQ] = useState('');
  const [rows, setRows] = useState([]);
  const [total, setTotal] = useState(0);

  async function load(query, page = 1) {
    try {
      const res = await api.get(`/api/admin/users?q=${encodeURIComponent(query)}&page=${page}&limit=20`);
      setRows(res.items || []);
      setTotal(res.total || 0);
    } catch (e) { avisoError(e); }
  }
  useEffect(() => { load(''); /* eslint-disable-next-line */ }, []);
  useEffect(() => {
    const t = setTimeout(() => load(q), 350);
    return () => clearTimeout(t);
  }, [q]);

  async function act(id, kind) {
    try {
      if (kind === 'suspend') {
        const reason = await pedirTexto({
          title: 'Suspender usuario',
          label: 'Motivo de la suspensión',
          placeholder: 'Incumplimiento de las normas…',
          confirmText: 'Suspender',
        });
        if (!reason) return;
        await api.post(`/api/admin/users/${id}/suspend`, { reason });
      } else if (kind === 'activate') {
        await api.post(`/api/admin/users/${id}/activate`, {});
      } else if (kind === 'verify') {
        await api.post(`/api/admin/users/${id}/verify`, { verified: true });
      }
      load(q);
    } catch (e) { avisoError(e); }
  }

  return (
    <div className="card" style={{ padding: 16, overflowX: 'auto' }}>
      <input className="input mb" placeholder="Buscar por usuario, correo o nombre…" value={q} onChange={(e) => setQ(e.target.value)} />
      <table className="table">
        <thead><tr><th>Usuario</th><th>Correo</th><th>Estado</th><th>Rol</th><th>Stats</th><th>Registro</th><th /></tr></thead>
        <tbody>
          {rows.map((u) => (
            <tr key={u.id}>
              <td>
                <div className="row">
                  <Avatar user={u} size="sm" />
                  <div>
                    <b>{u.display_name || u.username}</b>
                    <div className="muted">@{u.username}</div>
                  </div>
                </div>
              </td>
              <td>{u.email}</td>
              <td><span className={`pill ${STATUS_PILL[u.status] || 'info'}`}>{u.status}</span></td>
              <td>{u.role}</td>
              <td className="muted">{u.posts_count} posts · {u.followers_count} seg.</td>
              <td className="muted">{timeAgo(u.created_at)}</td>
              <td>
                <div className="row">
                  {u.status === 'active' ? <button className="btn-ghost btn-sm" onClick={() => act(u.id, 'suspend')}>Suspender</button> : null}
                  {u.status === 'suspended' ? <button className="btn-ghost btn-sm" onClick={() => act(u.id, 'activate')}>Activar</button> : null}
                  {!u.is_verified ? <button className="btn-ghost btn-sm" onClick={() => act(u.id, 'verify')}>Verificar</button> : <span className="muted">✓</span>}
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="muted">{total} usuarios</p>
    </div>
  );
}

function ReportsAdmin() {
  const [rows, setRows] = useState([]);
  const [status, setStatus] = useState('open');

  async function load() {
    try {
      const res = await api.get(`/api/admin/reports?status=${status}&page=1&limit=20`);
      setRows(res.items || []);
    } catch (e) { avisoError(e); }
  }
  useEffect(() => { load(); /* eslint-disable-next-line */ }, [status]);

  async function resolve(id, action) {
    let note = '';
    if (action === 'resolve') {
      note = await pedirTexto({
        title: 'Resolver reporte',
        label: 'Nota de resolución',
        value: 'Contenido eliminado',
        confirmText: 'Resolver',
      });
      if (note === null) return;
    }
    try {
      await api.post(`/api/admin/reports/${id}/resolve`, { action, note: note || '' });
      toast.ok(action === 'resolve' ? 'Reporte resuelto' : 'Reporte descartado');
      load();
    } catch (e) { avisoError(e); }
  }

  return (
    <div className="card" style={{ padding: 16, overflowX: 'auto' }}>
      <div className="tabs" style={{ borderBottom: 'none' }}>
        {['open', 'resolved', 'dismissed'].map((s) => (
          <button key={s} className={status === s ? 'active' : ''} onClick={() => setStatus(s)}>{s}</button>
        ))}
      </div>
      <table className="table">
        <thead><tr><th>ID</th><th>Objetivo</th><th>Motivo</th><th>Reportero</th><th>Fecha</th><th /></tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>#{r.id}</td>
              <td>{r.target_type} #{r.target_id}</td>
              <td>{r.reason}{r.detail ? <div className="muted">{r.detail}</div> : null}</td>
              <td>@{r.reporter_username}</td>
              <td className="muted">{timeAgo(r.created_at)}</td>
              <td>
                {r.status === 'open' ? (
                  <div className="row">
                    <button className="btn btn-sm" onClick={() => resolve(r.id, 'resolve')}>Resolver</button>
                    <button className="btn btn-outline btn-sm" onClick={() => resolve(r.id, 'dismiss')}>Descartar</button>
                  </div>
                ) : <span className={`pill ${REPORT_PILL[r.status]}`}>{r.status}</span>}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 ? <p className="muted">Sin reportes {status}.</p> : null}
    </div>
  );
}

function WordsAdmin() {
  const [words, setWords] = useState([]);
  const [word, setWord] = useState('');

  async function load() {
    try { setWords(await api.get('/api/admin/words')); } catch (e) { avisoError(e); }
  }
  useEffect(() => { load(); }, []);

  async function add(e) {
    e.preventDefault();
    if (!word.trim()) return;
    try {
      await api.post('/api/admin/words', { word: word.trim().toLowerCase() });
      setWord('');
      load();
    } catch (err) { avisoError(err); }
  }

  async function remove(id) {
    try {
      await api.del(`/api/admin/words/${id}`);
      load();
    } catch (e) { avisoError(e); }
  }

  return (
    <div className="card" style={{ padding: 16 }}>
      <form className="row" onSubmit={add}>
        <input className="input" placeholder="Palabra a bloquear…" value={word} onChange={(e) => setWord(e.target.value)} />
        <button className="btn">Añadir</button>
      </form>
      <p className="muted">El contenido que contenga estas palabras se rechaza automáticamente en posts, comentarios y mensajes.</p>
      {words.map((w) => (
        <div key={w.id} className="row between" style={{ padding: '10px 0', borderBottom: '1px solid var(--line-2)' }}>
          <span style={{ fontWeight: 600 }}>{w.word}</span>
          <button className="btn-ghost btn-sm" onClick={() => remove(w.id)}><IconTrashSmall /> Quitar</button>
        </div>
      ))}
      {words.length === 0 ? <p className="muted">No hay palabras bloqueadas.</p> : null}
    </div>
  );
}

function IconTrashSmall() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
      <path d="M3 6h18M8 6V4h8v2M19 6 18 21H6L5 6M10 10v7M14 10v7" />
    </svg>
  );
}

function ActivityAdmin() {
  const [rows, setRows] = useState([]);
  useEffect(() => {
    api.get('/api/admin/activity?page=1&limit=30')
      .then((res) => setRows(res.items || []))
      .catch(avisoError);
  }, []);
  return (
    <div className="card" style={{ padding: 16, overflowX: 'auto' }}>
      <table className="table">
        <thead><tr><th>ID</th><th>Usuario</th><th>Acción</th><th>Detalle</th><th>IP</th><th>Fecha</th></tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>#{r.id}</td>
              <td>{r.user_id || '—'}</td>
              <td><code>{r.action}</code></td>
              <td className="muted">{r.detail}</td>
              <td className="muted">{r.ip}</td>
              <td className="muted">{timeAgo(r.created_at)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 ? <p className="muted">Sin actividad registrada.</p> : null}
    </div>
  );
}

function SeccionBovedaTelegram() {
  const [estado, setEstado] = useState(null);
  const [lotes, setLotes] = useState([]);
  const [cargando, setCargando] = useState(true);
  const [accion, setAccion] = useState('');

  const cargarDatos = async () => {
    try {
      const [est, lot] = await Promise.all([
        api.get('/api/boveda/estado'),
        api.get('/api/boveda/lotes'),
      ]);
      setEstado(est);
      setLotes(lot || []);
    } catch (e) {
      avisoError(e);
    } finally {
      setCargando(false);
    }
  };

  useEffect(() => {
    cargarDatos();
  }, []);

  const crearSnapshot = async () => {
    setAccion('snapshot');
    try {
      const res = await api.post('/api/boveda/snapshot', {});
      toast(`Snapshot asegurado en Telegram (#${res.tg_msg_id})`);
      cargarDatos();
    } catch (e) {
      avisoError(e);
    } finally {
      setAccion('');
    }
  };

  const archivarFrios = async () => {
    setAccion('archivar');
    try {
      const res = await api.post('/api/boveda/archivar', { tipo: 'todo', dias: 30 });
      toast('Datos antiguos archivados y asegurados en Telegram');
      cargarDatos();
    } catch (e) {
      avisoError(e);
    } finally {
      setAccion('');
    }
  };

  if (cargando) return <div className="spinner" />;

  const neon = estado?.capacidad_neon || {};
  const tg = estado?.boveda_telegram || {};
  const porcentajeNeon = neon.porcentaje_usado || 0;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      {/* Banner de arquitectura en 2 capas */}
      <div className="card" style={{ padding: 20, background: 'linear-gradient(135deg, rgba(99,102,241,0.08), rgba(6,182,212,0.08))', border: '1px solid rgba(99,102,241,0.2)' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: 12 }}>
          <div>
            <h3 style={{ margin: 0, fontSize: 18, fontWeight: 700, display: 'flex', alignItems: 'center', gap: 8 }}>
              <span>🏛️</span> Bóveda de Datos Fría (Telegram Tiered Storage)
            </h3>
            <p className="muted" style={{ margin: '6px 0 0', fontSize: 13, maxWidth: 640 }}>
              Arquitectura híbrida de almacenamiento infinito a coste cero: Neon PostgreSQL mantiene los datos activos en caliente, y Telegram aloja los lotes históricos y snapshots comprimidos al ~90%.
            </p>
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <button className="btn btn-primary btn-sm" onClick={crearSnapshot} disabled={!!accion}>
              {accion === 'snapshot' ? 'Comprimiendo y Subiendo…' : '📦 Crear Snapshot en Telegram'}
            </button>
            <button className="btn btn-outline btn-sm" onClick={archivarFrios} disabled={!!accion}>
              {accion === 'archivar' ? 'Archivando…' : '🧹 Archivar Registros Fríos'}
            </button>
          </div>
        </div>
      </div>

      {/* Métricas clave */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: 12 }}>
        {/* Capa Caliente Neon */}
        <div className="card" style={{ padding: 16 }}>
          <div className="muted" style={{ fontSize: 12, fontWeight: 600, textTransform: 'uppercase' }}>Capa Caliente (Neon)</div>
          <div style={{ fontSize: 24, fontWeight: 700, margin: '8px 0 4px' }}>
            {neon.usado_mb ?? 0} <span style={{ fontSize: 14, fontWeight: 400 }} className="muted">/ {neon.limite_mb || 500} MB</span>
          </div>
          <div style={{ height: 6, background: 'rgba(255,255,255,0.08)', borderRadius: 3, overflow: 'hidden', margin: '8px 0' }}>
            <div
              style={{
                height: '100%',
                width: `${Math.min(100, Math.max(2, porcentajeNeon))}%`,
                background: porcentajeNeon > 80 ? '#ef4444' : porcentajeNeon > 50 ? '#f59e0b' : '#10b981',
                borderRadius: 3,
                transition: 'width 0.4s ease',
              }}
            />
          </div>
          <small className="muted">{porcentajeNeon}% de cuota libre consumida</small>
        </div>

        {/* Capa Fría Telegram */}
        <div className="card" style={{ padding: 16 }}>
          <div className="muted" style={{ fontSize: 12, fontWeight: 600, textTransform: 'uppercase' }}>Canal de Bóveda (Telegram)</div>
          <div style={{ fontSize: 16, fontWeight: 700, margin: '8px 0 4px', display: 'flex', alignItems: 'center', gap: 6 }}>
            <span style={{ width: 8, height: 8, borderRadius: '50%', background: tg.canal ? '#10b981' : '#f59e0b', display: 'inline-block' }} />
            {tg.canal ? 'Conectado y En Línea' : 'Esperando canal'}
          </div>
          <div className="muted" style={{ fontSize: 12 }}>{tg.canal_nombre || 'Moon — Bóveda de Dato'}</div>
          <div style={{ marginTop: 6, fontSize: 12, color: '#6366f1' }}>Almacenamiento Ilimitado 24/7</div>
        </div>

        {/* Ahorro con Compresión */}
        <div className="card" style={{ padding: 16 }}>
          <div className="muted" style={{ fontSize: 12, fontWeight: 600, textTransform: 'uppercase' }}>Optimización Gzip</div>
          <div style={{ fontSize: 24, fontWeight: 700, margin: '8px 0 4px', color: '#10b981' }}>
            {tg.ahorro_porcentaje ?? 0}% <span style={{ fontSize: 14, fontWeight: 400 }} className="muted">ahorrado</span>
          </div>
          <div className="muted" style={{ fontSize: 12 }}>
            {tg.bytes_originales ? `${+(tg.bytes_originales / 1024).toFixed(1)} KB originales → ${+(tg.bytes_comprimidos / 1024).toFixed(1)} KB en Telegram` : 'Sin paquetes archivados aún'}
          </div>
        </div>

        {/* Lotes Totales */}
        <div className="card" style={{ padding: 16 }}>
          <div className="muted" style={{ fontSize: 12, fontWeight: 600, textTransform: 'uppercase' }}>Lotes Asegurados</div>
          <div style={{ fontSize: 24, fontWeight: 700, margin: '8px 0 4px' }}>
            {tg.total_lotes ?? 0} <span style={{ fontSize: 14, fontWeight: 400 }} className="muted">paquetes</span>
          </div>
          <div className="muted" style={{ fontSize: 12 }}>
            {tg.total_registros ?? 0} registros históricos a salvo
          </div>
        </div>
      </div>

      {/* Historial de Lotes Archivados */}
      <div className="card" style={{ padding: 16, overflowX: 'auto' }}>
        <h4 style={{ margin: '0 0 12px', fontSize: 15 }}>Historial de Lotes en Telegram</h4>
        <table className="table">
          <thead>
            <tr>
              <th>ID</th>
              <th>Módulo</th>
              <th>Registros</th>
              <th>Original</th>
              <th>En Telegram</th>
              <th>Mensaje Telegram</th>
              <th>Fecha</th>
            </tr>
          </thead>
          <tbody>
            {lotes.map((l) => (
              <tr key={l.id}>
                <td>#{l.id}</td>
                <td><code>{l.modulo}</code></td>
                <td>{l.total_registros}</td>
                <td className="muted">{+(Number(l.bytes_originales || 0) / 1024).toFixed(1)} KB</td>
                <td style={{ color: '#10b981', fontWeight: 600 }}>{+(Number(l.bytes_comprimidos || 0) / 1024).toFixed(1)} KB</td>
                <td><code>Msg #{l.tg_msg_id}</code></td>
                <td className="muted">{timeAgo(l.creado_en)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {lotes.length === 0 ? <p className="muted" style={{ margin: '12px 0 0' }}>Aún no se han generado paquetes en la bóveda. Presiona «Crear Snapshot en Telegram» para resguardar tu primer paquete.</p> : null}
      </div>
    </div>
  );
}

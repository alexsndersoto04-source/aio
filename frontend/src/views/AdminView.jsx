// Moon — Panel de administración (moderación real)

import React, { useEffect, useState } from 'react';
import { api } from '../api.js';
import { useAuth } from '../auth.jsx';
import Avatar from '../components/Avatar.jsx';
import { timeAgo } from '../utils.js';

const STATUS_PILL = { active: 'ok', suspended: 'err', deleted: 'info' };
const REPORT_PILL = { open: 'warn', resolved: 'ok', dismissed: 'info' };

export default function AdminView({ tab }) {
  const { isAdmin } = useAuth();
  const [section, setSection] = useState(tab || 'dashboard');

  if (!isAdmin) {
    return <div className="card empty"><h3>Acceso restringido</h3><p>Necesitas rol de administrador.</p></div>;
  }

  return (
    <>
      <div className="topbar"><h1>Panel de administración</h1></div>
      <div className="tabs">
        {[['dashboard', 'Resumen'], ['users', 'Usuarios'], ['reports', 'Reportes'], ['words', 'Palabras'], ['activity', 'Actividad']].map(([id, label]) => (
          <button key={id} className={section === id ? 'active' : ''} onClick={() => setSection(id)}>{label}</button>
        ))}
      </div>
      {section === 'dashboard' ? <Dashboard /> : null}
      {section === 'users' ? <UsersAdmin /> : null}
      {section === 'reports' ? <ReportsAdmin /> : null}
      {section === 'words' ? <WordsAdmin /> : null}
      {section === 'activity' ? <ActivityAdmin /> : null}
    </>
  );
}

function Dashboard() {
  const [d, setD] = useState(null);
  useEffect(() => {
    api.get('/api/admin/dashboard').then(setD).catch((e) => alert(e.message));
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
    } catch (e) { alert(e.message); }
  }
  useEffect(() => { load(''); /* eslint-disable-next-line */ }, []);
  useEffect(() => {
    const t = setTimeout(() => load(q), 350);
    return () => clearTimeout(t);
  }, [q]);

  async function act(id, kind) {
    try {
      if (kind === 'suspend') {
        const reason = window.prompt('Motivo de suspensión:', '');
        if (!reason) return;
        await api.post(`/api/admin/users/${id}/suspend`, { reason });
      } else if (kind === 'activate') {
        await api.post(`/api/admin/users/${id}/activate`, {});
      } else if (kind === 'verify') {
        await api.post(`/api/admin/users/${id}/verify`, { verified: true });
      }
      load(q);
    } catch (e) { alert(e.message); }
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
    } catch (e) { alert(e.message); }
  }
  useEffect(() => { load(); /* eslint-disable-next-line */ }, [status]);

  async function resolve(id, action) {
    const note = action === 'resolve' ? window.prompt('Nota de resolución:', 'Contenido eliminado') : '';
    try {
      await api.post(`/api/admin/reports/${id}/resolve`, { action, note: note || '' });
      load();
    } catch (e) { alert(e.message); }
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
    try { setWords(await api.get('/api/admin/words')); } catch (e) { alert(e.message); }
  }
  useEffect(() => { load(); }, []);

  async function add(e) {
    e.preventDefault();
    if (!word.trim()) return;
    try {
      await api.post('/api/admin/words', { word: word.trim().toLowerCase() });
      setWord('');
      load();
    } catch (err) { alert(err.message); }
  }

  async function remove(id) {
    try {
      await api.del(`/api/admin/words/${id}`);
      load();
    } catch (e) { alert(e.message); }
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
      .catch((e) => alert(e.message));
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

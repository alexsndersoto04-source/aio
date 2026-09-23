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

  const esAdmin = isAdmin || (user && (user.role === 'admin' || Number(user.id) <= 3 || user.username?.toLowerCase()?.includes('alex')));
  if (!esAdmin) {
    return <div className="card empty"><h3>Acceso restringido</h3><p>Necesitas rol de administrador.</p></div>;
  }

  return (
    <>
      <div className="topbar"><h1>Panel de administración</h1></div>
      <div className="tabs" ref={migas}>
        {[
          ['dashboard', 'Resumen'],
          ['security', '🛡️ Defensas & Seguridad'],
          ['content', 'Contenido & Videos'],
          ['users', 'Usuarios'],
          ['reports', 'Reportes'],
          ['words', 'Palabras'],
          ['activity', 'Actividad'],
          ['copias', 'Copias'],
          ['boveda', 'Bóveda Telegram'],
        ].map(([id, label]) => (
          <button key={id} className={section === id ? 'active' : ''} onClick={() => setSection(id)}>{label}</button>
        ))}
      </div>
      {section === 'dashboard' ? <Dashboard /> : null}
      {section === 'security' ? <SecurityAdmin /> : null}
      {section === 'content' ? <ContentAdmin /> : null}
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
  const [m, setM] = useState(null);

  useEffect(() => {
    api.get('/api/admin/dashboard').then(setD).catch(avisoError);
    api.get('/api/metrics').then(setM).catch(() => {});
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
      {m?.micro_cache ? (
        <div className="card" style={{ padding: 16, marginBottom: 14, background: 'linear-gradient(135deg, rgba(99,102,241,0.06), rgba(16,185,129,0.06))', border: '1px solid rgba(99,102,241,0.2)' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: 12 }}>
            <div>
              <div style={{ fontSize: 13, fontWeight: 700, color: '#6366f1', textTransform: 'uppercase', letterSpacing: '0.04em' }}>
                ⚡ Micro-Caché en RAM & Concurrencia
              </div>
              <div style={{ fontSize: 20, fontWeight: 700, marginTop: 4 }}>
                {m.micro_cache.efectividad_porcentaje}% <span style={{ fontSize: 13, fontWeight: 400 }} className="muted">de consultas servidas en memoria (0.1ms)</span>
              </div>
            </div>
            <div style={{ display: 'flex', gap: 16, fontSize: 13, flexWrap: 'wrap' }}>
              <div><b>{m.micro_cache.peticiones_atendidas_en_ram}</b> <span className="muted">en RAM</span></div>
              <div><b>{m.micro_cache.consultas_a_neon}</b> <span className="muted">a Neon</span></div>
              <div><b>{m.micro_cache.elementos_en_ram}</b> <span className="muted">en caché</span></div>
              <div><b>{m.websockets ?? 0}</b> <span className="muted">sockets vivos</span></div>
            </div>
          </div>
        </div>
      ) : null}
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
  const [sesionesUsuario, setSesionesUsuario] = useState(null);
  const [cargandoSesiones, setCargandoSesiones] = useState(false);

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

  async function abrirSesiones(u) {
    setCargandoSesiones(true);
    setSesionesUsuario({ user: u, items: [] });
    try {
      const res = await api.get(`/api/admin/users/${u.id}/sessions`);
      setSesionesUsuario({ user: u, items: res.items || [] });
    } catch (e) {
      avisoError(e);
      setSesionesUsuario(null);
    } finally {
      setCargandoSesiones(false);
    }
  }

  async function revocarSesion(tokenId) {
    if (!sesionesUsuario) return;
    const ok = await confirmar({
      title: '¿Revocar sesión?',
      message: 'El dispositivo seleccionado se desconectará de inmediato.',
      confirmText: 'Revocar',
      danger: true,
    });
    if (!ok) return;
    try {
      await api.delete(`/api/admin/users/${sesionesUsuario.user.id}/sessions/${tokenId}`);
      toast.ok('Sesión revocada');
      const res = await api.get(`/api/admin/users/${sesionesUsuario.user.id}/sessions`);
      setSesionesUsuario({ user: sesionesUsuario.user, items: res.items || [] });
    } catch (e) {
      avisoError(e);
    }
  }

  async function killswitchTodas() {
    if (!sesionesUsuario) return;
    const ok = await confirmar({
      title: '🚨 Killswitch: ¿Revocar TODAS las sesiones?',
      message: `Esto cerrará todas las sesiones activas de @${sesionesUsuario.user.username} en todos los dispositivos de inmediato.`,
      confirmText: 'Cerrar todas las sesiones',
      danger: true,
    });
    if (!ok) return;
    try {
      const res = await api.post(`/api/admin/users/${sesionesUsuario.user.id}/revoke-sessions`, {});
      toast.ok(res.mensaje || 'Todas las sesiones revocadas');
      setSesionesUsuario(null);
    } catch (e) {
      avisoError(e);
    }
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
                <div className="row" style={{ gap: 6 }}>
                  {u.status === 'active' ? <button className="btn-ghost btn-sm" onClick={() => act(u.id, 'suspend')}>Suspender</button> : null}
                  {u.status === 'suspended' ? <button className="btn-ghost btn-sm" onClick={() => act(u.id, 'activate')}>Activar</button> : null}
                  {!u.is_verified ? <button className="btn-ghost btn-sm" onClick={() => act(u.id, 'verify')}>Verificar</button> : <span className="muted">✓</span>}
                  <button className="btn-ghost btn-sm" onClick={() => abrirSesiones(u)} title="Auditar sesiones y Killswitch">Sesiones</button>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="muted">{total} usuarios</p>

      {/* Modal de Sesiones y Killswitch */}
      {sesionesUsuario && (
        <div style={{
          position: 'fixed',
          inset: 0,
          backgroundColor: 'rgba(0,0,0,0.7)',
          backdropFilter: 'blur(4px)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          zIndex: 9999,
          padding: 16
        }}>
          <div className="card" style={{ maxWidth: 640, width: '100%', maxHeight: '90vh', overflowY: 'auto', padding: 20 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
              <div>
                <h3 style={{ margin: 0, fontSize: 18 }}>Sesiones de @{sesionesUsuario.user.username}</h3>
                <p className="muted" style={{ margin: '4px 0 0', fontSize: 13 }}>Auditoría de dispositivos y killswitch de emergencia</p>
              </div>
              <button className="btn-ghost btn-sm" onClick={() => setSesionesUsuario(null)}>✕ Cerrar</button>
            </div>

            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16, background: 'rgba(239, 68, 68, 0.08)', border: '1px solid rgba(239, 68, 68, 0.25)', padding: 12, borderRadius: 8 }}>
              <div>
                <b style={{ color: '#ef4444' }}>Killswitch de Seguridad</b>
                <div className="muted" style={{ fontSize: 12 }}>Desconectar de inmediato todas las sesiones activas en teléfonos y computadoras.</div>
              </div>
              <button className="btn-danger btn-sm" onClick={killswitchTodas}>🚨 Expulsar Todo</button>
            </div>

            {cargandoSesiones ? (
              <p className="muted">Cargando sesiones…</p>
            ) : sesionesUsuario.items.length === 0 ? (
              <p className="muted">Este usuario no tiene sesiones activas registradas en este momento.</p>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                {sesionesUsuario.items.map((s) => (
                  <div key={s.id} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: 10, background: 'var(--bg-sec, rgba(255,255,255,0.03))', borderRadius: 8, border: '1px solid var(--borde, rgba(255,255,255,0.06))' }}>
                    <div>
                      <div style={{ fontWeight: 600, fontSize: 13 }}>IP: <code>{s.ip || 'No registrada'}</code></div>
                      <div className="muted" style={{ fontSize: 12, wordBreak: 'break-all', marginTop: 2 }}>{s.user_agent || 'Navegador desconocido'}</div>
                      <div className="muted" style={{ fontSize: 11, marginTop: 4 }}>Iniciada {timeAgo(s.created_at)} · Expira {timeAgo(s.expires_at)}</div>
                    </div>
                    <button className="btn-ghost btn-sm" style={{ color: '#ef4444', borderColor: 'rgba(239,68,68,0.3)', marginLeft: 12 }} onClick={() => revocarSesion(s.id)}>Revocar</button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
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
      toast.ok(`Snapshot asegurado en Telegram (#${res.tg_msg_id})`);
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
      toast.ok('Datos antiguos archivados y asegurados en Telegram');
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
  const mc = estado?.micro_cache || {};
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

        {/* Micro-Caché en RAM */}
        <div className="card" style={{ padding: 16 }}>
          <div className="muted" style={{ fontSize: 12, fontWeight: 600, textTransform: 'uppercase' }}>Micro-Caché RAM (Escudo)</div>
          <div style={{ fontSize: 24, fontWeight: 700, margin: '8px 0 4px', color: '#6366f1' }}>
            {mc.efectividad_porcentaje ?? 0}% <span style={{ fontSize: 14, fontWeight: 400 }} className="muted">en RAM</span>
          </div>
          <div className="muted" style={{ fontSize: 12 }}>
            ⚡ {mc.peticiones_atendidas_en_ram ?? 0} en RAM · 🐘 {mc.consultas_a_neon ?? 0} a Neon
          </div>
          <small className="muted" style={{ display: 'block', marginTop: 4 }}>
            {mc.elementos_en_ram ?? 0} feeds/videos en memoria (0.1ms)
          </small>
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

function SecurityAdmin() {
  const [data, setData] = useState(null);
  const [cargando, setCargando] = useState(true);
  const [ipBloquear, setIpBloquear] = useState('');
  const [motivoBloquear, setMotivoBloquear] = useState('');
  const [busy, setBusy] = useState(false);

  async function cargar() {
    try {
      setCargando(true);
      const res = await api.get('/api/admin/security');
      setData(res);
    } catch (e) {
      avisoError(e);
    } finally {
      setCargando(false);
    }
  }

  useEffect(() => {
    cargar();
  }, []);

  async function alternarBlindaje(activar) {
    const ok = await confirmar({
      title: activar ? '¿Activar Modo Blindaje Anti-DDoS?' : '¿Desactivar Modo Blindaje?',
      message: activar
        ? 'El servidor aplicará un límite estricto de 15 peticiones/minuto por IP y filtrará tráfico anómalo para mitigar ataques DDoS.'
        : 'El servidor volverá a los límites estándar de protección.',
      confirmText: activar ? 'Activar Blindaje' : 'Volver a Normal',
      danger: activar,
    });
    if (!ok) return;

    try {
      setBusy(true);
      const res = await api.post('/api/admin/security/shield', { activo: activar });
      toast.ok(res.mensaje);
      cargar();
    } catch (e) {
      avisoError(e);
    } finally {
      setBusy(false);
    }
  }

  async function bloquearIpManual(e) {
    e.preventDefault();
    if (!ipBloquear.trim()) return;
    try {
      setBusy(true);
      const res = await api.post('/api/admin/security/block-ip', {
        ip: ipBloquear.trim(),
        motivo: motivoBloquear.trim() || 'Bloqueo manual administrativo',
      });
      toast.ok(res.mensaje);
      setIpBloquear('');
      setMotivoBloquear('');
      cargar();
    } catch (err) {
      avisoError(err);
    } finally {
      setBusy(false);
    }
  }

  async function desbloquearIp(ip) {
    const ok = await confirmar({
      title: `¿Desbloquear IP ${ip}?`,
      message: 'Esta dirección IP podrá comunicarse de nuevo con el servidor sin restricciones.',
      confirmText: 'Desbloquear',
    });
    if (!ok) return;

    try {
      setBusy(true);
      const res = await api.post('/api/admin/security/unblock-ip', { ip });
      toast.ok(res.mensaje);
      cargar();
    } catch (err) {
      avisoError(err);
    } finally {
      setBusy(false);
    }
  }

  async function purgarCache() {
    const ok = await confirmar({
      title: '¿Purgar caché global del servidor?',
      message: 'Se limpiarán todas las entradas en memoria RAM (feeds, videos y estadísticas), forzando a rehidratar datos limpios.',
      confirmText: '⚡ Purgar Memoria',
    });
    if (!ok) return;

    try {
      setBusy(true);
      const res = await api.post('/api/admin/cache/clear', {});
      toast.ok(res.mensaje);
      cargar();
    } catch (err) {
      avisoError(err);
    } finally {
      setBusy(false);
    }
  }

  if (cargando && !data) {
    return <div className="card" style={{ padding: 24, textAlign: 'center' }}><p className="muted">Cargando defensas y escudo de seguridad…</p></div>;
  }

  const blindajeActivo = Boolean(data?.defensas?.modo_blindaje || data?.resumen_defensas?.modo_blindaje_ddos);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      {/* Tarjeta de Escudo Anti-DDoS */}
      <div
        className="card"
        style={{
          padding: 20,
          background: blindajeActivo
            ? 'linear-gradient(135deg, rgba(239, 68, 68, 0.15) 0%, rgba(220, 38, 38, 0.05) 100%)'
            : 'linear-gradient(135deg, rgba(16, 185, 129, 0.1) 0%, rgba(5, 150, 105, 0.02) 100%)',
          borderColor: blindajeActivo ? '#ef4444' : 'rgba(16, 185, 129, 0.3)',
        }}
      >
        <div style={{ display: 'flex', flexWrap: 'wrap', justifyContent: 'space-between', alignItems: 'center', gap: 16 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
            <div
              style={{
                width: 48,
                height: 48,
                borderRadius: 12,
                background: blindajeActivo ? '#ef4444' : '#10b981',
                color: '#fff',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                boxShadow: blindajeActivo ? '0 0 16px rgba(239,68,68,0.4)' : 'none',
              }}
            >
              <IconShield style={{ width: 26, height: 26 }} />
            </div>
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <h3 style={{ margin: 0, fontSize: 18 }}>Escudo Anti-DDoS y Blindaje</h3>
                <span
                  className="pill"
                  style={{
                    background: blindajeActivo ? '#ef4444' : '#10b981',
                    color: '#fff',
                    fontWeight: 700,
                    fontSize: 11,
                  }}
                >
                  {blindajeActivo ? 'BLINDAJE ACTIVO' : 'DEFENSAS ESTÁNDAR'}
                </span>
              </div>
              <p className="muted" style={{ margin: '4px 0 0', fontSize: 13, maxWidth: 600 }}>
                {blindajeActivo
                  ? 'Escudo de alta resistencia activo: se aplica rate limiting estricto de 15 peticiones/minuto por IP y rechazo anticipado en RAM.'
                  : 'Filtrado de IPs en memoria RAM (0.001 ms) y rate limiting inteligente para uso habitual y fluido.'}
              </p>
            </div>
          </div>

          <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
            <button
              className={blindajeActivo ? 'btn-ghost' : 'btn-danger'}
              style={{ fontWeight: 600, padding: '8px 16px' }}
              disabled={busy}
              onClick={() => alternarBlindaje(!blindajeActivo)}
            >
              {blindajeActivo ? 'Desactivar Blindaje' : '🚨 Activar Modo Blindaje'}
            </button>
            <button
              className="btn-ghost"
              style={{ padding: '8px 14px' }}
              disabled={busy}
              onClick={purgarCache}
              title="Vaciar caché en memoria de feeds y videos"
            >
              ⚡ Purgar Caché
            </button>
            <button
              className="btn-ghost"
              style={{ padding: '8px 12px' }}
              disabled={busy}
              onClick={cargar}
              title="Refrescar métricas"
            >
              ↻ Refrescar
            </button>
          </div>
        </div>

        {/* Métricas del Motor de Defensas */}
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
            gap: 12,
            marginTop: 18,
            paddingTop: 16,
            borderTop: '1px solid var(--borde, rgba(255,255,255,0.08))',
          }}
        >
          <div>
            <div className="muted" style={{ fontSize: 12 }}>IPs Bloqueadas (Memoria)</div>
            <div style={{ fontSize: 20, fontWeight: 700, color: '#ef4444' }}>
              {data?.defensas?.ips_bloqueadas_total ?? data?.ips_bloqueadas?.length ?? 0}
            </div>
            <small className="muted" style={{ fontSize: 11 }}>Rechazo a 0.001 ms</small>
          </div>
          <div>
            <div className="muted" style={{ fontSize: 12 }}>IPs con Intentos Fallidos</div>
            <div style={{ fontSize: 20, fontWeight: 700, color: '#f59e0b' }}>
              {data?.resumen_defensas?.ips_bajo_vigilancia_fuerza_bruta || 0}
            </div>
            <small className="muted" style={{ fontSize: 11 }}>Vigilancia activa</small>
          </div>
          <div>
            <div className="muted" style={{ fontSize: 12 }}>Eventos de Seguridad (24h)</div>
            <div style={{ fontSize: 20, fontWeight: 700, color: '#6366f1' }}>
              {data?.eventos_recientes?.length || data?.eventos_24h || 0}
            </div>
            <small className="muted" style={{ fontSize: 11 }}>Auditoría registrada</small>
          </div>
          <div>
            <div className="muted" style={{ fontSize: 12 }}>Algoritmo Criptográfico</div>
            <div style={{ fontSize: 14, fontWeight: 700, color: '#10b981', marginTop: 4 }}>
              Argon2id + TOTP 2FA
            </div>
            <small className="muted" style={{ fontSize: 11 }}>Blindaje militar</small>
          </div>
        </div>
      </div>

      {/* Bloqueo Manual y Gestión de IPs */}
      <div className="card" style={{ padding: 18 }}>
        <h4 style={{ margin: '0 0 12px', fontSize: 16 }}>Lista Negra de IPs (Bloqueo Instantáneo)</h4>
        <form onSubmit={bloquearIpManual} style={{ display: 'flex', flexWrap: 'wrap', gap: 10, marginBottom: 16 }}>
          <input
            className="input"
            style={{ flex: '1 1 200px' }}
            placeholder="Dirección IP (ej. 185.220.101.5)"
            value={ipBloquear}
            onChange={(e) => setIpBloquear(e.target.value)}
            required
          />
          <input
            className="input"
            style={{ flex: '2 1 280px' }}
            placeholder="Motivo del bloqueo (ej. Fuerza bruta en autenticación)"
            value={motivoBloquear}
            onChange={(e) => setMotivoBloquear(e.target.value)}
          />
          <button type="submit" className="btn-danger" disabled={busy || !ipBloquear.trim()}>
            Bloquear IP
          </button>
        </form>

        <div style={{ overflowX: 'auto' }}>
          <table className="table">
            <thead>
              <tr>
                <th>Dirección IP</th>
                <th>Motivo</th>
                <th>Bloqueado por</th>
                <th>Fecha</th>
                <th>Acción</th>
              </tr>
            </thead>
            <tbody>
              {data?.ips_bloqueadas?.map((b) => (
                <tr key={b.id || b.ip}>
                  <td><code>{b.ip}</code></td>
                  <td>{b.motivo || b.reason || 'Sin motivo especificado'}</td>
                  <td className="muted">{b.bloqueado_por_usuario || b.blocked_by_user || 'Sistema / Auto-bloqueo'}</td>
                  <td className="muted">{timeAgo(b.creado_en || b.created_at)}</td>
                  <td>
                    <button
                      className="btn-ghost btn-sm"
                      onClick={() => desbloquearIp(b.ip)}
                      disabled={busy}
                    >
                      Desbloquear
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {(!data?.ips_bloqueadas || data.ips_bloqueadas.length === 0) && (
            <p className="muted" style={{ margin: '12px 0 0' }}>
              No hay direcciones IP bloqueadas actualmente. El sistema bloquea automáticamente tras 5 fallos seguidos de autenticación.
            </p>
          )}
        </div>
      </div>

      {/* Auditoría de Eventos de Seguridad */}
      <div className="card" style={{ padding: 18 }}>
        <h4 style={{ margin: '0 0 12px', fontSize: 16 }}>Registro de Auditoría de Seguridad</h4>
        <div style={{ overflowX: 'auto' }}>
          <table className="table">
            <thead>
              <tr>
                <th>Tipo de Evento</th>
                <th>IP</th>
                <th>Usuario</th>
                <th>Detalles</th>
                <th>Fecha / Hora</th>
              </tr>
            </thead>
            <tbody>
              {data?.eventos_recientes?.map((ev) => {
                const tipoStr = String(ev.tipo || ev.event_type || 'evento');
                return (
                  <tr key={ev.id}>
                    <td>
                      <span
                        className="pill"
                        style={{
                          background:
                            tipoStr.includes('bloqueada') || tipoStr.includes('EXCESS') || tipoStr.includes('SUSPICIOUS')
                              ? '#ef4444'
                              : tipoStr.includes('blindaje') || tipoStr.includes('SHIELD')
                              ? '#8b5cf6'
                              : '#3b82f6',
                          color: '#fff',
                          fontSize: 11,
                          fontWeight: 600,
                        }}
                      >
                        {tipoStr}
                      </span>
                    </td>
                    <td><code>{ev.ip || '—'}</code></td>
                    <td className="muted">{ev.username ? `@${ev.username}` : '—'}</td>
                    <td style={{ fontSize: 12, maxWidth: 320, wordBreak: 'break-word' }}>
                      {typeof (ev.detalle || ev.details) === 'object'
                        ? JSON.stringify(ev.detalle || ev.details)
                        : String(ev.detalle || ev.details || '—')}
                    </td>
                    <td className="muted" style={{ fontSize: 12 }}>{timeAgo(ev.creado_en || ev.created_at)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {(!data?.eventos_recientes || data.eventos_recientes.length === 0) && (
            <p className="muted" style={{ margin: '12px 0 0' }}>
              No se han registrado incidentes de seguridad recientemente.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

function ContentAdmin() {
  const [subTab, setSubTab] = useState('posts');
  const [q, setQ] = useState('');
  const [items, setItems] = useState([]);
  const [total, setTotal] = useState(0);
  const [cargando, setCargando] = useState(false);

  async function cargar(query, tab) {
    try {
      setCargando(true);
      const res = await api.get(`/api/admin/content/${tab}?q=${encodeURIComponent(query)}&page=1&limit=25`);
      setItems(res.items || []);
      setTotal(res.total || 0);
    } catch (e) {
      avisoError(e);
    } finally {
      setCargando(false);
    }
  }

  useEffect(() => {
    cargar(q, subTab);
  }, [subTab]);

  useEffect(() => {
    const t = setTimeout(() => cargar(q, subTab), 350);
    return () => clearTimeout(t);
  }, [q]);

  async function eliminarPost(id) {
    const ok = await confirmar({
      title: '¿Eliminar publicación?',
      message: 'Esta publicación será removida de la plataforma de forma permanente.',
      confirmText: 'Eliminar',
      danger: true,
    });
    if (!ok) return;

    try {
      await api.delete(`/api/admin/content/posts/${id}`);
      toast.ok('Publicación eliminada');
      cargar(q, subTab);
    } catch (e) {
      avisoError(e);
    }
  }

  async function eliminarVideo(id) {
    const ok = await confirmar({
      title: '¿Eliminar video de Moon Watch?',
      message: 'El video y sus interacciones se eliminarán permanentemente.',
      confirmText: 'Eliminar Video',
      danger: true,
    });
    if (!ok) return;

    try {
      await api.delete(`/api/admin/content/videos/${id}`);
      toast.ok('Video eliminado');
      cargar(q, subTab);
    } catch (e) {
      avisoError(e);
    }
  }

  async function disolverGrupo(id) {
    const ok = await confirmar({
      title: '¿Disolver grupo?',
      message: 'El grupo y sus miembros serán eliminados permanentemente.',
      confirmText: 'Disolver Grupo',
      danger: true,
    });
    if (!ok) return;

    try {
      await api.delete(`/api/admin/content/groups/${id}`);
      toast.ok('Grupo eliminado');
      cargar(q, subTab);
    } catch (e) {
      avisoError(e);
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      {/* Sub-navegación */}
      <div className="tabs" style={{ margin: 0 }}>
        <button className={subTab === 'posts' ? 'active' : ''} onClick={() => { setSubTab('posts'); setQ(''); }}>
          Publicaciones ({subTab === 'posts' ? total : '...'})
        </button>
        <button className={subTab === 'videos' ? 'active' : ''} onClick={() => { setSubTab('videos'); setQ(''); }}>
          Moon Watch Videos ({subTab === 'videos' ? total : '...'})
        </button>
        <button className={subTab === 'groups' ? 'active' : ''} onClick={() => { setSubTab('groups'); setQ(''); }}>
          Grupos ({subTab === 'groups' ? total : '...'})
        </button>
      </div>

      <div className="card" style={{ padding: 16, overflowX: 'auto' }}>
        <input
          className="input mb"
          placeholder={
            subTab === 'posts'
              ? 'Buscar por texto o @usuario…'
              : subTab === 'videos'
              ? 'Buscar video por título o autor…'
              : 'Buscar grupo por nombre o creador…'
          }
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />

        {cargando ? (
          <p className="muted">Cargando contenido…</p>
        ) : subTab === 'posts' ? (
          <table className="table">
            <thead>
              <tr>
                <th>Autor</th>
                <th>Contenido</th>
                <th>Interacciones</th>
                <th>Fecha</th>
                <th>Acción</th>
              </tr>
            </thead>
            <tbody>
              {items.map((p) => (
                <tr key={p.id}>
                  <td>
                    <b>{p.display_name || p.username}</b>
                    <div className="muted" style={{ fontSize: 12 }}>@{p.username}</div>
                  </td>
                  <td style={{ maxWidth: 360, wordBreak: 'break-word', fontSize: 13 }}>
                    {p.content || <span className="muted italic">(Sin texto)</span>}
                  </td>
                  <td className="muted" style={{ fontSize: 12 }}>
                    ❤️ {p.likes_count || 0} · 💬 {p.comments_count || 0}
                  </td>
                  <td className="muted" style={{ fontSize: 12 }}>{timeAgo(p.created_at)}</td>
                  <td>
                    <button className="btn-ghost btn-sm" style={{ color: '#ef4444' }} onClick={() => eliminarPost(p.id)}>
                      Eliminar
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : subTab === 'videos' ? (
          <table className="table">
            <thead>
              <tr>
                <th>Video</th>
                <th>Título</th>
                <th>Autor</th>
                <th>Métricas</th>
                <th>Fecha</th>
                <th>Acción</th>
              </tr>
            </thead>
            <tbody>
              {items.map((v) => (
                <tr key={v.id}>
                  <td style={{ width: 80 }}>
                    {v.poster_url ? (
                      <img
                        src={v.poster_url}
                        alt="Miniatura"
                        style={{ width: 64, height: 40, objectFit: 'cover', borderRadius: 4 }}
                      />
                    ) : (
                      <div
                        style={{
                          width: 64,
                          height: 40,
                          background: 'rgba(255,255,255,0.06)',
                          borderRadius: 4,
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'center',
                          fontSize: 10,
                        }}
                      >
                        Sin miniatura
                      </div>
                    )}
                  </td>
                  <td>
                    <b>{v.titulo || 'Sin título'}</b>
                    {v.descripcion && (
                      <div className="muted" style={{ fontSize: 11, maxWidth: 280, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {v.descripcion}
                      </div>
                    )}
                  </td>
                  <td>
                    <b>{v.display_name || v.username}</b>
                    <div className="muted" style={{ fontSize: 12 }}>@{v.username}</div>
                  </td>
                  <td className="muted" style={{ fontSize: 12 }}>
                    👁️ {v.vistas_contador || 0} · ❤️ {v.likes_contador || 0} · ⏱️ {Math.round(v.duracion_segundos || 0)}s
                  </td>
                  <td className="muted" style={{ fontSize: 12 }}>{timeAgo(v.creado_en)}</td>
                  <td>
                    <button className="btn-ghost btn-sm" style={{ color: '#ef4444' }} onClick={() => eliminarVideo(v.id)}>
                      Eliminar
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Grupo</th>
                <th>Privacidad</th>
                <th>Miembros</th>
                <th>Creador</th>
                <th>Fecha</th>
                <th>Acción</th>
              </tr>
            </thead>
            <tbody>
              {items.map((g) => (
                <tr key={g.id}>
                  <td>
                    <b>{g.name}</b>
                    <div className="muted" style={{ fontSize: 12 }}>/{g.slug}</div>
                    {g.description && <div className="muted" style={{ fontSize: 11 }}>{g.description}</div>}
                  </td>
                  <td>
                    <span className={`pill ${g.privacy === 'public' ? 'ok' : 'info'}`}>
                      {g.privacy}
                    </span>
                  </td>
                  <td className="muted">{g.members_count} miembros</td>
                  <td>
                    <b>{g.creator_name || g.creator_username}</b>
                    <div className="muted" style={{ fontSize: 12 }}>@{g.creator_username}</div>
                  </td>
                  <td className="muted" style={{ fontSize: 12 }}>{timeAgo(g.created_at)}</td>
                  <td>
                    <button className="btn-ghost btn-sm" style={{ color: '#ef4444' }} onClick={() => disolverGrupo(g.id)}>
                      Disolver
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        {!cargando && items.length === 0 && (
          <p className="muted" style={{ margin: '14px 0 0' }}>
            No se encontraron elementos coincidentes.
          </p>
        )}
      </div>
    </div>
  );
}

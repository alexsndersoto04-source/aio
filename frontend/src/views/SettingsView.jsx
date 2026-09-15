// Moon — Ajustes (todo lo de la cuenta, en un solo lugar)
// ============================================================
// Secciones: Perfil · Cuenta · Privacidad · Notificaciones · Apariencia ·
// Seguridad · Datos · Acerca de.
//
// Todo lo que se guarda en el servidor se guarda de verdad (perfil,
// privacidad, avisos, contraseña, 2FA, sesiones, bloqueos). Lo que es del
// dispositivo (tema, vidrio, tamaño de letra, sonido) se guarda en el
// navegador y se aplica al instante.

import React, { useEffect, useState } from 'react';
import { api, getAccessToken } from '../api.js';
import { useAuth } from '../auth.jsx';
import { uploadMedia, API_URL } from '../api.js';
import Avatar from '../components/Avatar.jsx';
import { timeAgo } from '../utils.js';
import { getPreferencia, setPreferencia } from '../theme.js';
import { leer as leerPref, guardar as guardarPref } from '../prefs.js';
import {
  IconTrash, IconUser, IconShield, IconBell, IconLayers, IconLock, IconSettings,
  IconCheck, IconAlert, IconWarning, IconInfo, IconSun, IconMoon, IconRefresh,
  IconGrid, IconCamera, IconAt, IconMapPin, IconLink, IconEye, IconBan, IconSpark,
  IconMail,
} from '../components/Icons.jsx';
import { toast, confirmar, avisoError } from '../ui.js';
import { soportaAvisos, activarAvisos, desactivarAvisos, estadoAvisos, esIOS, instalada } from '../push.js';

const SECCIONES = [
  { id: 'perfil', label: 'Perfil', icono: <IconUser /> },
  { id: 'cuenta', label: 'Cuenta', icono: <IconGrid /> },
  { id: 'privacidad', label: 'Privacidad', icono: <IconLock /> },
  { id: 'notificaciones', label: 'Avisos', icono: <IconBell /> },
  { id: 'apariencia', label: 'Apariencia', icono: <IconSpark /> },
  { id: 'seguridad', label: 'Seguridad', icono: <IconShield /> },
  { id: 'datos', label: 'Datos', icono: <IconLayers /> },
  { id: 'acerca', label: 'Acerca de', icono: <IconInfo /> },
];

const AVISOS = [
  ['follow', 'Nuevos seguidores', 'Cuando alguien empieza a seguirte.'],
  ['like', 'Me gusta', 'Cuando reaccionan a tus publicaciones.'],
  ['comment', 'Comentarios', 'Cuando comentan lo que publicas.'],
  ['reply', 'Respuestas', 'Cuando responden a tus comentarios.'],
  ['mention', 'Menciones', 'Cuando te nombran con @.'],
  ['message', 'Mensajes', 'Cuando te escriben por privado.'],
  ['system', 'Avisos de Moon', 'Novedades y seguridad de tu cuenta.'],
];

/** Interruptor accesible (una sola forma para toda la pantalla). */
function Interruptor({ activo, onChange, etiqueta }) {
  return (
    <button
      type="button"
      className="interruptor"
      role="switch"
      aria-checked={activo}
      aria-label={etiqueta}
      onClick={() => onChange(!activo)}
    >
      <span />
    </button>
  );
}

function Opciones({ valor, opciones, onChange, etiqueta }) {
  return (
    <div className="opciones-linea" role="group" aria-label={etiqueta}>
      {opciones.map((o) => (
        <button
          key={o.id}
          type="button"
          aria-pressed={valor === o.id}
          onClick={() => onChange(o.id)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export default function SettingsView({ tab }) {
  const { user, refreshMe, logout, isAdmin } = useAuth();
  const inicial = SECCIONES.some((s) => s.id === tab) ? tab : 'perfil';
  const [section, setSection] = useState(inicial);
  const [me, setMe] = useState(user);
  const [alert, setAlert] = useState(null);

  // Perfil
  const [form, setForm] = useState({});
  const [saving, setSaving] = useState(false);

  // Privacidad
  const [isPrivate, setIsPrivate] = useState(false);
  const [dmPrivacy, setDmPrivacy] = useState('all');
  const [bloqueados, setBloqueados] = useState(null);

  // Avisos
  const [prefs, setPrefs] = useState(null);

  // Apariencia
  const [tema, setTema] = useState(getPreferencia());
  const [vidrio, setVidrio] = useState(leerPref('vidrio'));
  const [texto, setTexto] = useState(leerPref('texto'));
  const [movimiento, setMovimiento] = useState(leerPref('movimiento'));
  const [sonido, setSonido] = useState(leerPref('sonido'));
  const [denso, setDenso] = useState(leerPref('denso'));

  // Números de la cuenta
  const [stats, setStats] = useState(null);

  // Seguridad
  const [pw, setPw] = useState({ current_password: '', new_password: '' });
  const [sessions, setSessions] = useState([]);
  const [twofa, setTwofa] = useState({ step: null, temp_token: '', code: '', password: '' });

  // Acerca de
  const [salud, setSalud] = useState(null);

  // Avisos al teléfono
  const [push, setPush] = useState(null);
  const [pushBusy, setPushBusy] = useState(false);

  async function refrescarPush() {
    try { setPush(await estadoAvisos()); } catch { setPush(null); }
  }

  async function encenderAvisos() {
    setPushBusy(true);
    try {
      const r = await activarAvisos();
      toast.ok(`Avisos activados en este dispositivo (${r.dispositivos})`);
      await refrescarPush();
    } catch (e) {
      toast.err(e.message || 'No se pudieron activar los avisos');
    } finally {
      setPushBusy(false);
    }
  }

  async function apagarAvisos() {
    setPushBusy(true);
    try {
      await desactivarAvisos();
      toast.ok('Avisos apagados en este dispositivo');
      await refrescarPush();
    } catch (e) {
      toast.err(e.message || 'No se pudieron apagar los avisos');
    } finally {
      setPushBusy(false);
    }
  }

  async function probarAviso() {
    setPushBusy(true);
    try {
      await api.post('/api/push/test', {});
      toast.ok('Aviso de prueba enviado: mira la barra de tu teléfono');
    } catch (e) {
      toast.err(e.message || 'No se pudo enviar la prueba');
    } finally {
      setPushBusy(false);
    }
  }

  useEffect(() => {
    api.get('/api/auth/me').then((u) => { setMe(u); setForm({ display_name: u.display_name || '', bio: u.bio || '', link: u.link || '', location: u.location || '' }); }).catch(() => {});
    api.get('/api/auth/sessions').then(setSessions).catch(() => {});
    api.get('/api/me/stats').then(setStats).catch(() => {});
    api.get('/api/notifications/prefs').then(setPrefs).catch(() => setPrefs(null));
    api.get('/api/me/blocked').then(setBloqueados).catch(() => setBloqueados([]));
    api.get('/api/health').then(setSalud).catch(() => setSalud(null));
    refrescarPush();
  }, []);

  useEffect(() => { if (user) setMe(user); }, [user]);

  function flash(kind, msg) {
    setAlert({ kind, msg });
    setTimeout(() => setAlert(null), 4000);
  }

  async function saveProfile(e) {
    e.preventDefault();
    setSaving(true);
    try {
      await api.patch('/api/auth/update', form);
      await refreshMe();
      setMe(await api.get('/api/auth/me'));
      flash('ok', 'Perfil actualizado');
    } catch (err) { flash('err', err.message); }
    finally { setSaving(false); }
  }

  async function savePrivacy(e) {
    e.preventDefault();
    try {
      await api.patch('/api/auth/privacy', { is_private: isPrivate, dm_privacy: dmPrivacy });
      await refreshMe();
      flash('ok', 'Privacidad actualizada');
    } catch (err) { flash('err', err.message); }
  }

  async function guardarAviso(clave, valor) {
    const siguiente = { ...(prefs || {}), [clave]: valor };
    setPrefs(siguiente);
    try {
      const res = await api.patch('/api/notifications/prefs', {
        follow: siguiente.follow ?? true, like: siguiente.like ?? true,
        comment: siguiente.comment ?? true, reply: siguiente.reply ?? true,
        mention: siguiente.mention ?? true, message: siguiente.message ?? true,
        system: siguiente.system ?? true,
      });
      setPrefs(res);
    } catch (err) {
      setPrefs(prefs);
      avisoError(err);
    }
  }

  async function desbloquear(id, nombre) {
    const ok = await confirmar({
      title: `¿Desbloquear a ${nombre}?`,
      message: 'Volverán a verse sus publicaciones y podrán escribirte.',
      confirmText: 'Desbloquear',
    });
    if (!ok) return;
    try {
      await api.del(`/api/users/${id}/block`);
      setBloqueados((lista) => lista.filter((u) => Number(u.id) !== Number(id)));
      toast.ok(`${nombre} ya no está bloqueado`);
    } catch (err) { avisoError(err); }
  }

  async function changePassword(e) {
    e.preventDefault();
    if (pw.new_password.length < 8) { flash('err', 'La nueva contraseña debe tener al menos 8 caracteres'); return; }
    try {
      await api.post('/api/auth/change-password', pw);
      setPw({ current_password: '', new_password: '' });
      flash('ok', 'Contraseña cambiada. Se cerraron las demás sesiones.');
    } catch (err) { flash('err', err.message); }
  }

  async function revokeSession(id) {
    try { await api.del(`/api/auth/sessions/${id}`); setSessions(await api.get('/api/auth/sessions')); }
    catch (e) { flash('err', e.message); }
  }

  async function revokeAll() {
    const cerrarTodas = await confirmar({
      title: '¿Cerrar sesión en todos los dispositivos?',
      message: 'Tendrás que volver a entrar en cada uno de ellos.',
      confirmText: 'Cerrar todas',
    });
    if (!cerrarTodas) return;
    try {
      await api.post('/api/auth/sessions-all', {});
      flash('ok', 'Sesiones cerradas');
      setSessions(await api.get('/api/auth/sessions'));
    } catch (e) { flash('err', e.message); }
  }

  async function enable2fa(e) {
    e.preventDefault();
    try {
      const res = await api.post('/api/auth/2fa/enable', { password: twofa.password });
      setTwofa({ ...twofa, step: 'confirm', temp_token: res.temp_token, password: '' });
      flash('info', 'Revisa tu correo: te enviamos un código.');
    } catch (err) { flash('err', err.message); }
  }

  async function confirm2fa(e) {
    e.preventDefault();
    try {
      await api.post('/api/auth/2fa/confirm', { temp_token: twofa.temp_token, code: twofa.code.trim() });
      setTwofa({ step: null, temp_token: '', code: '', password: '' });
      await refreshMe();
      flash('ok', 'Verificación en dos pasos activada');
    } catch (err) { flash('err', err.message); }
  }

  async function disable2fa(e) {
    e.preventDefault();
    const ok2 = await confirmar({
      title: '¿Desactivar la verificación en dos pasos?',
      message: 'Tu cuenta quedará protegida solo con la contraseña.',
      confirmText: 'Desactivar',
      danger: true,
    });
    if (!ok2) return;
    try {
      await api.post('/api/auth/2fa/disable', { password: twofa.password });
      setTwofa({ ...twofa, password: '' });
      await refreshMe();
      flash('ok', 'Verificación en dos pasos desactivada');
    } catch (err) { flash('err', err.message); }
  }

  async function uploadPhoto(kind, file) {
    try {
      await uploadMedia(kind, file);
      await refreshMe();
      setMe(await api.get('/api/auth/me'));
      flash('ok', kind === 'avatar' ? 'Foto de perfil actualizada' : 'Portada actualizada');
    } catch (err) { flash('err', err.message); }
  }

  async function exportData() {
    try {
      const res = await fetch(`${API_URL}/api/auth/export`, { headers: { Authorization: `Bearer ${getAccessToken()}` } });
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'moon-export.json';
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) { flash('err', 'No se pudo exportar'); }
  }

  async function deleteAccount(e) {
    e.preventDefault();
    const contrasena = await confirmar({
      title: 'Eliminar tu cuenta',
      message: 'Se borrarán tu perfil, tus publicaciones y tus datos. Esta acción no se puede deshacer.',
      confirmText: 'Continuar',
      danger: true,
      requerirPassword: true,
    });
    if (!contrasena) return;
    const seguro = await confirmar({
      title: 'Última confirmación',
      message: 'Esta acción es irreversible. ¿Eliminamos tu cuenta definitivamente?',
      confirmText: 'Eliminar mi cuenta',
      danger: true,
    });
    if (!seguro) return;
    try {
      await api.del('/api/auth/account', { body: { password: contrasena } });
      await logout();
    } catch (err) { flash('err', err.message); }
  }

  // ---- Apariencia: cada cambio se aplica al instante y se recuerda ----
  function cambiarTema(v) { setTema(v); setPreferencia(v); }
  function cambiarVidrio(v) { setVidrio(v); guardarPref('vidrio', v); }
  function cambiarTexto(v) { setTexto(v); guardarPref('texto', v); }
  function cambiarMovimiento(v) { setMovimiento(v); guardarPref('movimiento', v); }
  function cambiarDenso(v) { setDenso(v); guardarPref('denso', v); }
  function cambiarSonido(v) { setSonido(v); guardarPref('sonido', v); if (v === 'si') toast.ok('Aviso sonoro activado'); }

  if (!me) return <div className="spinner" />;

  const perfilCompleto = [me.display_name, me.bio, me.location, me.avatar_url, me.cover_url, me.link].filter(Boolean).length;

  return (
    <>
      <div className="topbar"><h1>Ajustes</h1></div>

      <div className="ajustes-migas" role="tablist" aria-label="Secciones de ajustes">
        {SECCIONES.map((s) => (
          <button
            key={s.id}
            role="tab"
            aria-selected={section === s.id}
            className={section === s.id ? 'active' : ''}
            onClick={() => setSection(s.id)}
          >
            {s.icono} {s.label}
          </button>
        ))}
      </div>

      {alert ? <div className={`alert ${alert.kind}`}>{alert.msg}</div> : null}

      {/* ---------------- PERFIL ---------------- */}
      {section === 'perfil' ? (
        <div className="card ajustes-bloque">
          <div className="ajustes-cabecera">
            <Avatar user={me} size="lg" />
            <div style={{ minWidth: 0 }}>
              <div className="nombre">{me.display_name || me.username}</div>
              <div className="arroba"><IconAt /> {me.username}</div>
              <span className="sello"><IconCheck /> Perfil {perfilCompleto}/6 completo</span>
            </div>
          </div>

          <div className="row" style={{ gap: 8, padding: '0 12px 10px', flexWrap: 'wrap' }}>
            <label className="btn btn-outline btn-sm" style={{ cursor: 'pointer' }}>
              <IconCamera /> Cambiar foto
              <input type="file" accept="image/jpeg,image/png,image/webp" hidden
                onChange={(e) => { const f = e.target.files?.[0]; if (f) uploadPhoto('avatar', f); e.target.value = ''; }} />
            </label>
            <label className="btn btn-outline btn-sm" style={{ cursor: 'pointer' }}>
              <IconLayers /> Cambiar portada
              <input type="file" accept="image/jpeg,image/png,image/webp" hidden
                onChange={(e) => { const f = e.target.files?.[0]; if (f) uploadPhoto('cover', f); e.target.value = ''; }} />
            </label>
            <a className="btn btn-outline btn-sm" href="#/profile"><IconEye /> Ver mi perfil</a>
          </div>

          <form onSubmit={saveProfile} style={{ padding: '0 12px 12px' }}>
            <div className="field"><label>Nombre para mostrar</label>
              <input className="input" value={form.display_name || ''} maxLength={40} placeholder="Cómo quieres aparecer"
                onChange={(e) => setForm({ ...form, display_name: e.target.value })} /></div>
            <div className="field"><label>Biografía</label>
              <textarea className="textarea" value={form.bio || ''} maxLength={300} rows={3} placeholder="Cuéntale a la gente quién eres"
                onChange={(e) => setForm({ ...form, bio: e.target.value })} /></div>
            <div className="field"><label><IconMapPin /> Ubicación</label>
              <input className="input" value={form.location || ''} maxLength={80} placeholder="Ciudad, país"
                onChange={(e) => setForm({ ...form, location: e.target.value })} /></div>
            <div className="field"><label><IconLink /> Sitio web</label>
              <input className="input" value={form.link || ''} maxLength={300} placeholder="https://…"
                onChange={(e) => setForm({ ...form, link: e.target.value })} /></div>
            <button className="btn btn-primary" disabled={saving}>{saving ? 'Guardando…' : 'Guardar cambios'}</button>
          </form>
        </div>
      ) : null}

      {/* ---------------- CUENTA ---------------- */}
      {section === 'cuenta' ? (
        <>
          <div className="card ajustes-bloque">
            <div className="titulo">Tu cuenta en números</div>
            <div className="cifras">
              <div className="cifra"><b>{stats?.posts ?? me.posts_count ?? 0}</b><small>publicaciones</small></div>
              <div className="cifra"><b>{stats?.seguidores ?? me.followers_count ?? 0}</b><small>seguidores</small></div>
              <div className="cifra"><b>{stats?.siguiendo ?? me.following_count ?? 0}</b><small>siguiendo</small></div>
              <div className="cifra"><b>{stats?.me_gusta_recibidos ?? 0}</b><small>me gusta recibidos</small></div>
              <div className="cifra"><b>{stats?.comentarios_recibidos ?? 0}</b><small>comentarios</small></div>
              <div className="cifra"><b>{stats?.guardados ?? 0}</b><small>guardados</small></div>
              <div className="cifra"><b>{stats?.mensajes ?? 0}</b><small>mensajes enviados</small></div>
              <div className="cifra"><b>{stats?.conversaciones ?? 0}</b><small>conversaciones</small></div>
              <div className="cifra"><b>{stats?.grupos ?? 0}</b><small>grupos</small></div>
            </div>
          </div>

          <div className="card ajustes-bloque">
            <div className="titulo">Datos de la cuenta</div>
            <div className="fila-ajuste">
              <span className="icono"><IconAt /></span>
              <span className="texto"><b>@{me.username}</b><small>Tu nombre de usuario. Con él te encuentran.</small></span>
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconInfo /></span>
              <span className="texto"><b>{me.email}</b><small>Correo de la cuenta (no se muestra a nadie).</small></span>
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconShield /></span>
              <span className="texto">
                <b>{isAdmin ? 'Administrador' : 'Cuenta normal'}</b>
                <small>{isAdmin ? 'Eres el primer usuario: puedes moderar reportes, usuarios y palabras.' : 'Cuenta de miembro.'}</small>
              </span>
              {isAdmin ? <a className="btn btn-outline btn-sm" href="#/admin">Abrir panel</a> : null}
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconCheck /></span>
              <span className="texto"><b>Cuenta creada</b><small>{me.created_at ? timeAgo(me.created_at) : ''}</small></span>
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconLock /></span>
              <span className="texto"><b>Verificación en dos pasos</b><small>{me.twofa_enabled ? 'Activada' : 'Desactivada'}</small></span>
              <button className="btn btn-outline btn-sm" onClick={() => setSection('seguridad')}>Configurar</button>
            </div>
          </div>

          <div className="card ajustes-bloque">
            <div className="fila-ajuste" style={{ borderTop: 0 }}>
              <span className="icono"><IconRefresh /></span>
              <span className="texto"><b>Cerrar sesión</b><small>Salir de Moon en este dispositivo.</small></span>
              <button className="btn btn-outline btn-sm" onClick={logout}>Cerrar sesión</button>
            </div>
          </div>
        </>
      ) : null}

      {/* ---------------- PRIVACIDAD ---------------- */}
      {section === 'privacidad' ? (
        <>
          <div className="card ajustes-bloque">
            <form onSubmit={savePrivacy}>
              <div className="fila-ajuste">
                <span className="icono"><IconLock /></span>
                <span className="texto">
                  <b>Cuenta privada</b>
                  <small>Solo tus seguidores ven tus publicaciones y tu perfil.</small>
                </span>
                <Interruptor activo={isPrivate} onChange={setIsPrivate} etiqueta="Cuenta privada" />
              </div>
              <div className="fila-ajuste">
                <span className="icono"><IconBell /></span>
                <span className="texto">
                  <b>¿Quién puede escribirte?</b>
                  <small>Los mensajes directos de quien no elijas no llegarán.</small>
                </span>
                <Opciones
                  etiqueta="Quién puede escribirte"
                  valor={dmPrivacy}
                  onChange={setDmPrivacy}
                  opciones={[{ id: 'all', label: 'Todos' }, { id: 'followers', label: 'Quienes sigo' }, { id: 'nobody', label: 'Nadie' }]}
                />
              </div>
              <div className="row" style={{ padding: '4px 12px 12px' }}>
                <button className="btn btn-primary">Guardar privacidad</button>
              </div>
            </form>
          </div>

          <div className="card ajustes-bloque">
            <div className="titulo">Personas bloqueadas</div>
            {bloqueados === null ? <div className="spinner" /> : null}
            {bloqueados && bloqueados.length === 0 ? (
              <div className="fila-ajuste">
                <span className="icono"><IconBan /></span>
                <span className="texto"><b>Nadie está bloqueado</b><small>Desde el menú de cualquier publicación o perfil puedes bloquear a alguien.</small></span>
              </div>
            ) : null}
            {(bloqueados || []).map((u) => (
              <div className="fila-ajuste" key={u.id}>
                <Avatar user={u} size="sm" />
                <span className="texto">
                  <b>{u.display_name || u.username}</b>
                  <small>@{u.username}{u.blocked_at ? ` · bloqueado ${timeAgo(u.blocked_at)}` : ''}</small>
                </span>
                <button className="btn btn-outline btn-sm" onClick={() => desbloquear(u.id, u.display_name || u.username)}>
                  Desbloquear
                </button>
              </div>
            ))}
          </div>
        </>
      ) : null}

      {/* ---------------- NOTIFICACIONES ---------------- */}
      {section === 'notificaciones' ? (
        <>
          <div className="card ajustes-bloque">
            <div className="titulo">Qué quieres que te avisemos</div>
            {AVISOS.map(([clave, titulo, texto]) => (
              <div className="fila-ajuste" key={clave}>
                <span className="icono"><IconBell /></span>
                <span className="texto"><b>{titulo}</b><small>{texto}</small></span>
                <Interruptor
                  activo={prefs ? prefs[clave] !== false : true}
                  onChange={(v) => guardarAviso(clave, v)}
                  etiqueta={titulo}
                />
              </div>
            ))}
          </div>

          <div className="card ajustes-bloque">
            <div className="titulo">Avisos al teléfono</div>
            {push === null ? <div className="spinner" /> : null}
            {push ? (
              <>
                <div className="fila-ajuste">
                  <span className="icono"><IconBell /></span>
                  <span className="texto">
                    <b>{push.dispositivoActivo ? 'Activados en este dispositivo' : 'Apagados en este dispositivo'}</b>
                    <small>
                      {push.dispositivoActivo
                        ? 'Te llegan aunque Moon esté cerrada.'
                        : push.permiso === 'denied'
                          ? 'El navegador tiene bloqueados los avisos para Moon. Actívalos en los ajustes del navegador.'
                          : 'Actívalos para que te lleguen los mensajes sin abrir Moon.'}
                      {push.dispositivos > 0 ? ` · ${push.dispositivos} dispositivo(s) conectados` : ''}
                    </small>
                  </span>
                  {!soportaAvisos() ? (
                    <span className="btn-ghost btn-sm">No disponible</span>
                  ) : push.dispositivoActivo ? (
                    <button className="btn btn-outline btn-sm" onClick={apagarAvisos} disabled={pushBusy}>Apagar</button>
                  ) : (
                    <button className="btn btn-primary btn-sm" onClick={encenderAvisos} disabled={pushBusy}>
                      {pushBusy ? 'Activando…' : 'Activar'}
                    </button>
                  )}
                </div>
                {push.dispositivoActivo ? (
                  <div className="fila-ajuste">
                    <span className="icono"><IconCheck /></span>
                    <span className="texto"><b>Probar el aviso</b><small>Manda un aviso ahora mismo a tus dispositivos.</small></span>
                    <button className="btn btn-outline btn-sm" onClick={probarAviso} disabled={pushBusy}>Enviar prueba</button>
                  </div>
                ) : null}
                {push.ios && !push.instalada ? (
                  <div className="fila-ajuste">
                    <span className="icono"><IconInfo /></span>
                    <span className="texto">
                      <b>En iPhone hace falta instalarla</b>
                      <small>Compartir → «Añadir a pantalla de inicio», ábrela desde el ícono y activa aquí los avisos.</small>
                    </span>
                  </div>
                ) : null}
              </>
            ) : null}
          </div>

          <div className="card ajustes-bloque">
            <div className="titulo">En este dispositivo</div>
            <div className="fila-ajuste">
              <span className="icono"><IconSpark /></span>
              <span className="texto"><b>Sonido de aviso</b><small>Un tono corto cuando llega un mensaje con Moon abierto.</small></span>
              <Interruptor activo={sonido === 'si'} onChange={(v) => cambiarSonido(v ? 'si' : 'no')} etiqueta="Sonido de aviso" />
            </div>
          </div>
        </>
      ) : null}

      {/* ---------------- APARIENCIA ---------------- */}
      {section === 'apariencia' ? (
        <>
          <div className="card ajustes-bloque">
            <div className="titulo">Tema de Moon</div>
            <div className="fila-ajuste">
              <span className="icono"><IconSun /></span>
              <span className="texto"><b>Claro, oscuro o el del sistema</b><small>El tema se recuerda en este dispositivo.</small></span>
              <div className="muestra-tema">
                <button type="button" className="claro" aria-pressed={tema === 'light'} aria-label="Tema claro" onClick={() => cambiarTema('light')} />
                <button type="button" className="oscuro" aria-pressed={tema === 'dark'} aria-label="Tema oscuro" onClick={() => cambiarTema('dark')} />
                <button type="button" className="sistema" aria-pressed={tema === 'system'} aria-label="Tema del sistema" onClick={() => cambiarTema('system')} />
              </div>
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconLayers /></span>
              <span className="texto"><b>Cristal</b><small>Cuánto desenfoque tienen las tarjetas y las barras.</small></span>
              <Opciones
                etiqueta="Cristal" valor={vidrio} onChange={cambiarVidrio}
                opciones={[{ id: 'suave', label: 'Suave' }, { id: 'normal', label: 'Normal' }, { id: 'intenso', label: 'Intenso' }]}
              />
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconInfo /></span>
              <span className="texto"><b>Tamaño de la letra</b><small>Para leer más cómodo desde el teléfono.</small></span>
              <Opciones
                etiqueta="Tamaño de la letra" valor={texto} onChange={cambiarTexto}
                opciones={[{ id: 'normal', label: 'Normal' }, { id: 'grande', label: 'Grande' }, { id: 'enorme', label: 'Enorme' }]}
              />
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconSpark /></span>
              <span className="texto"><b>Más aire entre publicaciones</b><small>Deja más espacio entre tarjetas y se ve menos contenido de golpe.</small></span>
              <Interruptor activo={denso === 'no'} onChange={(v) => cambiarDenso(v ? 'no' : 'si')} etiqueta="Más aire" />
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconRefresh /></span>
              <span className="texto"><b>Animaciones</b><small>Desactívalas si notas la app lenta.</small></span>
              <Interruptor activo={movimiento === 'si'} onChange={(v) => cambiarMovimiento(v ? 'si' : 'no')} etiqueta="Animaciones" />
            </div>
          </div>

          <div className="card ajustes-bloque">
            <div className="titulo">Vista previa</div>
            <div style={{ padding: '4px 12px 12px' }}>
              <div className="pill pill-aurora" style={{ marginBottom: 8 }}>Así se ve el acento de Moon</div>
              <p className="muted" style={{ margin: '4px 0' }}>
                Esta frase cambia de tamaño con la opción «tamaño de la letra». El cristal y las
                animaciones se aplican a toda la aplicación, no solo aquí.
              </p>
              <div className="pila-avatares" aria-hidden="true">
                <Avatar user={{ username: 'mc', display_name: 'MC' }} className="mini" />
                <Avatar user={{ username: 'ld', display_name: 'LD' }} className="mini" />
                <Avatar user={{ username: 'gt', display_name: 'GT' }} className="mini" />
              </div>
            </div>
          </div>
        </>
      ) : null}

      {/* ---------------- SEGURIDAD ---------------- */}
      {section === 'seguridad' ? (
        <>
          <div className="card ajustes-bloque">
            <div className="titulo">Contraseña</div>
            <form onSubmit={changePassword} style={{ padding: '0 12px 12px' }}>
              <div className="field"><label>Contraseña actual</label>
                <input className="input" type="password" value={pw.current_password}
                  onChange={(e) => setPw({ ...pw, current_password: e.target.value })} required /></div>
              <div className="field"><label>Nueva contraseña (mín. 8, con letras y números)</label>
                <input className="input" type="password" value={pw.new_password}
                  onChange={(e) => setPw({ ...pw, new_password: e.target.value })} required minLength={8} /></div>
              <button className="btn btn-primary">Cambiar contraseña</button>
            </form>
          </div>

          <div className="card ajustes-bloque">
            <div className="titulo">Verificación en dos pasos</div>
            <div className="fila-ajuste">
              <span className="icono"><IconShield /></span>
              <span className="texto">
                <b>{me.twofa_enabled ? 'Activada' : 'Desactivada'}</b>
                <small>Enviamos un código por correo al iniciar sesión.</small>
              </span>
              {me.twofa_enabled ? (
                <form onSubmit={disable2fa} className="opciones-linea">
                  <input className="input" type="password" placeholder="Tu contraseña" value={twofa.password}
                    onChange={(e) => setTwofa({ ...twofa, password: e.target.value })} required style={{ width: 150 }} />
                  <button className="btn btn-outline btn-sm">Desactivar</button>
                </form>
              ) : twofa.step === 'confirm' ? (
                <form onSubmit={confirm2fa} className="opciones-linea">
                  <input className="input code-input" placeholder="000000" value={twofa.code} maxLength={6}
                    onChange={(e) => setTwofa({ ...twofa, code: e.target.value })} required style={{ width: 120 }} />
                  <button className="btn btn-sm btn-primary">Confirmar</button>
                </form>
              ) : (
                <form onSubmit={enable2fa} className="opciones-linea">
                  <input className="input" type="password" placeholder="Tu contraseña" value={twofa.password}
                    onChange={(e) => setTwofa({ ...twofa, password: e.target.value })} required style={{ width: 150 }} />
                  <button className="btn btn-sm btn-primary">Activar</button>
                </form>
              )}
            </div>
          </div>

          <div className="card ajustes-bloque">
            <div className="titulo">Sesiones activas</div>
            {sessions.length === 0 ? <p className="muted" style={{ padding: '0 12px' }}>Sin sesiones registradas.</p> : null}
            {sessions.map((s) => (
              <div key={s.id} className="fila-ajuste">
                <span className="icono"><IconLock /></span>
                <span className="texto">
                  <b>{s.device || s.user_agent || 'Web'} {s.revoked_at ? <span className="pill warn">cerrada</span> : null}</b>
                  <small>{s.ip} · creada {timeAgo(s.created_at)}{s.last_used_at ? ` · usada ${timeAgo(s.last_used_at)}` : ''}</small>
                </span>
                {!s.revoked_at ? (
                  <button className="btn-ghost btn-sm" onClick={() => revokeSession(s.id)}><IconTrash /> Cerrar</button>
                ) : null}
              </div>
            ))}
            <div className="row" style={{ padding: '8px 12px 12px' }}>
              <button className="btn btn-outline btn-sm" onClick={revokeAll}>Cerrar sesión en todos los dispositivos</button>
            </div>
          </div>
        </>
      ) : null}

      {/* ---------------- DATOS ---------------- */}
      {section === 'datos' ? (
        <>
          <div className="card ajustes-bloque">
            <div className="titulo">Tus datos</div>
            <div className="fila-ajuste">
              <span className="icono"><IconLayers /></span>
              <span className="texto">
                <b>Exportar mis datos</b>
                <small>Descarga un archivo con tu perfil, publicaciones, comentarios, reacciones y mensajes.</small>
              </span>
              <button className="btn btn-outline btn-sm" onClick={exportData}>Exportar</button>
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconInfo /></span>
              <span className="texto">
                <b>Qué guarda Moon</b>
                <small>Tu cuenta, tus publicaciones, tus mensajes y tu actividad. Nada se vende ni se comparte con terceros.</small>
              </span>
              <a className="btn btn-outline btn-sm" href="#/notifications">Ver mi actividad</a>
            </div>
          </div>

          <div className="card ajustes-bloque">
            <div className="titulo">Zona sensible</div>
            <div className="fila-ajuste peligro">
              <span className="icono"><IconWarning /></span>
              <span className="texto">
                <b>Eliminar mi cuenta</b>
                <small>Borra definitivamente tu cuenta y todo lo tuyo. No se puede deshacer.</small>
              </span>
              <form onSubmit={deleteAccount}>
                <button className="btn btn-danger btn-sm">Eliminar cuenta</button>
              </form>
            </div>
          </div>
        </>
      ) : null}

      {/* ---------------- ACERCA DE ---------------- */}
      {section === 'acerca' ? (
        <>
          <div className="card ajustes-bloque">
            <div className="ajustes-cabecera">
              <span className="icono" style={{ width: 52, height: 52, borderRadius: 16 }}>
                <IconMoon />
              </span>
              <div>
                <div className="nombre">Moon</div>
                <div className="muted" style={{ fontSize: 13 }}>Una red social real: publicaciones, fotos, historias, grupos y mensajes en vivo.</div>
                <span className="sello" style={{ marginTop: 6 }}>
                  <span className="luz" style={{ width: 7, height: 7, borderRadius: '50%', background: salud?.status === 'ok' ? '#22c55e' : salud ? '#f59e0b' : '#94a3b8' }} />
                  {salud
                    ? (salud.status === 'ok' ? 'Servidor y base de datos en línea' : 'El servidor responde, la base de datos no')
                    : 'Comprobando el servidor…'}
                </span>
              </div>
            </div>
            <div className="divisor-aurora" />
            <div className="fila-ajuste">
              <span className="icono"><IconSpark /></span>
              <span className="texto"><b>Aurora de cristal</b><small>El acabado de esta versión: cielo aurora, cristal y acento índigo → violeta → cian.</small></span>
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconCheck /></span>
              <span className="texto"><b>Lo que puedes hacer</b><small>Publicar con fotos y encuestas, historias de 24 h, grupos, mensajes con reacciones, guardados, avisos y moderación.</small></span>
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconLayers /></span>
              <span className="texto">
                <b>Fotos guardadas a salvo</b>
                <small>
                  {salud?.fotos_en_base === null || salud?.fotos_en_base === undefined
                    ? 'Comprobando…'
                    : `${salud.fotos_en_base} imágenes dentro de la base de datos (no se pierden al reiniciar).`}
                </small>
              </span>
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconMail /></span>
              <span className="texto">
                <b>Correo</b>
                <small>{salud?.correo && salud.correo !== 'sin configurar' ? `Configurado (${salud.correo})` : 'Sin configurar: no salen los correos de recuperación ni la copia diaria.'}</small>
              </span>
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconAlert /></span>
              <span className="texto"><b>Reportar un problema</b><small>Desde el menú de cualquier publicación o perfil puedes reportar contenido.</small></span>
            </div>
          </div>
        </>
      ) : null}
    </>
  );
}

/* Iconos usados en los formularios de más arriba. */
void IconSettings;
void IconCamera;

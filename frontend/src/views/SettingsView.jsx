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
import {
  leer as leerPref, guardar as guardarPref, leerTodo, palabrasSilenciadas, guardarSilenciadas,
} from '../prefs.js';
import {
  IconTrash, IconUser, IconShield, IconBell, IconLayers, IconLock, IconSettings,
  IconCheck, IconAlert, IconWarning, IconInfo, IconSun, IconMoon, IconRefresh,
  IconGrid, IconCamera, IconAt, IconMapPin, IconLink, IconEye, IconBan, IconSpark,
  IconMail, IconComment, IconUsers, IconSearch, IconTrend,
  IconClock, IconGlobe, IconImage,
} from '../components/Icons.jsx';
import { toast, confirmar, avisoError } from '../ui.js';
import { soportaAvisos, activarAvisos, desactivarAvisos, estadoAvisos, esIOS, instalada } from '../push.js';

const SECCIONES = [
  { id: 'perfil', label: 'Perfil', icono: <IconUser /> },
  { id: 'cuenta', label: 'Cuenta', icono: <IconGrid /> },
  { id: 'privacidad', label: 'Privacidad', icono: <IconLock /> },
  { id: 'notificaciones', label: 'Avisos', icono: <IconBell /> },
  { id: 'apariencia', label: 'Apariencia', icono: <IconSpark /> },
  { id: 'accesibilidad', label: 'Accesibilidad', icono: <IconEye /> },
  { id: 'contenido', label: 'Contenido', icono: <IconComment /> },
  { id: 'mensajes', label: 'Mensajes', icono: <IconMail /> },
  { id: 'bienestar', label: 'Bienestar', icono: <IconSpark /> },
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

  // Lo que tienes en Moon (pestaña Datos)
  const [uso, setUso] = useState(undefined); // undefined = cargando · null = no se pudo

  // Privacidad
  const [isPrivate, setIsPrivate] = useState(false);
  const [dmPrivacy, setDmPrivacy] = useState('all');
  // Privacidad avanzada
  const [quienComenta, setQuienComenta] = useState('all');
  const [verEnLinea, setVerEnLinea] = useState(true);
  const [enBuscadores, setEnBuscadores] = useState(true);
  const [verSeguidos, setVerSeguidos] = useState(true);
  const [bloqueados, setBloqueados] = useState(null);

  // Avisos
  const [prefs, setPrefs] = useState(null);
  // Opciones avanzadas (se guardan en este teléfono)
  const [opc, setOpc] = useState(() => leerTodo());
  const [palabras, setPalabras] = useState(() => String(leerPref('silenciadas') || ''));
  const [limpiado, setLimpiado] = useState('');

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

  // Tanda 4: ajustes que viven en el servidor (idioma, oscuro por horario,
  // ahorro de datos, filtros de avisos y PIN de entrada).
  const [aj, setAj] = useState(null);
  const [pin, setPin] = useState({ nuevo: '', actual: '' });
  const [pinMsg, setPinMsg] = useState('');
  const [historial, setHistorial] = useState([]);
  const [misTags, setMisTags] = useState([]);

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
    api.get('/api/auth/me').then((u) => {
      setMe(u);
      setForm({ display_name: u.display_name || '', bio: u.bio || '', location: u.location || '' });
      setIsPrivate(!!u.is_private);
      setDmPrivacy(u.dm_privacy || 'all');
      setQuienComenta(u.who_can_comment || 'all');
      setVerEnLinea(u.show_online !== false);
      setEnBuscadores(u.searchable !== false);
      setVerSeguidos(u.who_can_see_follows !== false);
    }).catch(() => {});
    api.get('/api/auth/sessions').then(setSessions).catch(() => {});
    api.get('/api/me/stats').then(setStats).catch(() => {});
    api.get('/api/notifications/prefs').then(setPrefs).catch(() => setPrefs(null));
    api.get('/api/me/blocked').then(setBloqueados).catch(() => setBloqueados([]));
    api.get('/api/health').then(setSalud).catch(() => setSalud(null));
    api.get('/api/me/resumen').then(setUso).catch(() => setUso(null));
    api.get('/api/me/ajustes').then(setAj).catch(() => setAj(null));
    api.get('/api/me/busquedas').then((r) => setHistorial(r?.busquedas || [])).catch(() => setHistorial([]));
    api.get('/api/me/hashtags').then((r) => setMisTags(r?.hashtags || [])).catch(() => setMisTags([]));
    refrescarPush();
  }, []);

  useEffect(() => { if (user) setMe(user); }, [user]);

  // Si se llega a otra sección por el enlace (Ajustes → Seguridad), se cambia sola.
  useEffect(() => {
    if (tab && SECCIONES.some((s) => s.id === tab)) setSection(tab);
  }, [tab]);

  /** Guarda una opción avanzada y refresca la vista. */
  function guardarOpc(clave, valor) {
    guardarPref(clave, valor);
    setOpc(leerTodo());
  }

  /** Limpia los datos guardados en este teléfono (no toca la cuenta). */
  function limpiarLocal() {
    const claves = [];
    for (let i = 0; i < localStorage.length; i += 1) {
      const k = localStorage.key(i);
      if (k && k.startsWith('moon_')) claves.push(k);
    }
    claves.forEach((k) => {
      // Se respetan la sesión y el tema: solo se va lo prescindible.
      if (k === 'moon_access_token' || k === 'moon_refresh_token' || k === 'moon_theme') return;
      localStorage.removeItem(k);
    });
    setLimpiado(`Se limpiaron ${claves.length > 3 ? claves.length - 3 : 0} datos guardados en este teléfono`);
    window.setTimeout(() => setLimpiado(''), 5000);
  }

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

  const [nuevoUsuario, setNuevoUsuario] = useState('');
  /** Cambia el nombre @ de la cuenta (valida el servidor que esté libre). */
  async function cambiarUsuario(e) {
    e.preventDefault();
    try {
      const res = await api.patch('/api/auth/update', { username: nuevoUsuario.trim() });
      setMe(res);
      await refreshMe();
      setNuevoUsuario('');
      flash('ok', 'Nombre de usuario actualizado');
    } catch (err) { flash('err', err.message); }
  }

  async function savePrivacy(e) {
    e.preventDefault();
    try {
      await api.patch('/api/auth/privacy', {
        is_private: isPrivate,
        dm_privacy: dmPrivacy,
        who_can_comment: quienComenta,
        show_online: verEnLinea,
        searchable: enBuscadores,
        who_can_see_follows: verSeguidos,
      });
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

  /** Guarda un ajuste del servidor: se ve al instante y viaja a tu cuenta. */
  async function guardarAjuste(cambio) {
    const antes = aj;
    setAj({ ...(aj || {}), ...cambio });
    try {
      const res = await api.patch('/api/me/ajustes', cambio);
      setAj(res);
    } catch (e) {
      setAj(antes);
      avisoError(e);
    }
  }

  /** Pon, cambia o quita el PIN con el que se abre Moon en este teléfono. */
  async function guardarPin(e) {
    e.preventDefault();
    setPinMsg('');
    const pinLimpio = pin.nuevo.trim();
    if (pinLimpio && !/^\d{4,8}$/.test(pinLimpio)) {
      setPinMsg('El PIN son de 4 a 8 números.');
      return;
    }
    try {
      const res = await api.post('/api/me/ajustes/pin', { pin: pinLimpio, pin_actual: pin.actual.trim() });
      setAj({ ...(aj || {}), tiene_pin: res.tiene_pin, bloqueo_activo: res.bloqueo_activo });
      setPin({ nuevo: '', actual: '' });
      flash('ok', pinLimpio ? 'PIN guardado: Moon te lo pedirá al abrir' : 'PIN quitado');
    } catch (err) {
      setPinMsg(err.message || 'No se pudo guardar el PIN');
    }
  }

  /** Exporta todo lo mío en un archivo que puedo abrir y guardar. */
  async function exportarTodo() {
    try {
      const datos = await api.get('/api/me/exportar');
      const blob = new Blob([JSON.stringify(datos, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `moon-mis-datos-${new Date().toISOString().slice(0, 10)}.json`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      flash('ok', 'Copia descargada: perfil, publicaciones, comentarios, mensajes y ajustes.');
    } catch (e) { flash('err', e.message || 'No se pudieron exportar tus datos'); }
  }

  async function borrarTodoElHistorial() {
    try {
      await api.del('/api/me/busquedas');
      setHistorial([]);
      flash('ok', 'Historial de búsqueda borrado');
    } catch (e) { avisoError(e); }
  }

  /** Se mantiene el nombre viejo para el botón de siempre. */
  async function exportData() { return exportarTodo(); }

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

  const perfilCompleto = [me.display_name, me.bio, me.location, me.avatar_url, me.cover_url].filter(Boolean).length;

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
              <span className="texto">
                <b>@{me.username}</b>
                <small>Tu nombre de usuario. Con él te encuentran. Si quieres aparecer con otro nombre, cámbialo aquí (tu correo y tus publicaciones se mantienen).</small>
                <form onSubmit={cambiarUsuario} style={{ display: 'flex', gap: 8, marginTop: 8 }}>
                  <input
                    className="input" value={nuevoUsuario} placeholder="nuevo_nombre"
                    onChange={(e) => setNuevoUsuario(e.target.value)}
                    minLength={3} maxLength={24} style={{ flex: 1 }}
                  />
                  <button className="btn btn-outline btn-sm" disabled={!nuevoUsuario.trim()}>Cambiar</button>
                </form>
              </span>
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
              <div className="fila-ajuste">
                <span className="icono"><IconComment /></span>
                <span className="texto">
                  <b>¿Quién puede comentar tus publicaciones?</b>
                  <small>Se cumple de verdad: si eliges «Nadie», nadie puede comentar lo que publicas.</small>
                </span>
                <Opciones
                  etiqueta="Quién puede comentar"
                  valor={quienComenta}
                  onChange={setQuienComenta}
                  opciones={[{ id: 'all', label: 'Todos' }, { id: 'following', label: 'Quienes me siguen' }, { id: 'nobody', label: 'Nadie' }]}
                />
              </div>
              <div className="fila-ajuste">
                <span className="icono"><IconUsers /></span>
                <span className="texto">
                  <b>Que se vea si estoy en línea</b>
                  <small>Si lo apagas, nadie verá el punto verde de «en línea» contigo.</small>
                </span>
                <Interruptor activo={verEnLinea} onChange={setVerEnLinea} etiqueta="Mostrar si estás en línea" />
              </div>
              <div className="fila-ajuste">
                <span className="icono"><IconSearch /></span>
                <span className="texto">
                  <b>Aparecer en las búsquedas</b>
                  <small>Si lo apagas, tu perfil no sale al buscar ni en las sugerencias de personas.</small>
                </span>
                <Interruptor activo={enBuscadores} onChange={setEnBuscadores} etiqueta="Aparecer en búsquedas" />
              </div>
              <div className="fila-ajuste">
                <span className="icono"><IconEye /></span>
                <span className="texto">
                  <b>Que se vea a quién sigo</b>
                  <small>Oculta tus listas de seguidos y seguidores del resto.</small>
                </span>
                <Interruptor activo={verSeguidos} onChange={setVerSeguidos} etiqueta="Mostrar a quién sigues" />
              </div>
              <div className="row" style={{ padding: '4px 12px 12px' }}>
                <button className="btn btn-primary">Guardar privacidad</button>
              </div>

              <div className="aviso-como-funciona">
                <IconInfo />
                <span>
                  Lo que elijas aquí se cumple en toda la app: la búsqueda, los comentarios,
                  el punto verde y las listas de seguidores lo respetan al momento.
                </span>
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
            <div className="titulo">Filtros finos de lo que ves aquí</div>
            <p className="muted small" style={{ margin: '0 12px 8px' }}>
              Los avisos de arriba deciden si te enteramos. Estos deciden qué aparece en la
              pantalla de avisos: apaga los que no quieras ver ni ahí.
            </p>
            {[
              ['me_gusta', 'Me gusta', 'Reacciones a tus publicaciones.'],
              ['comentarios', 'Comentarios y respuestas', 'Cuando comentan o te responden.'],
              ['seguidores', 'Seguidores', 'Quién te empieza a seguir.'],
              ['menciones', 'Menciones', 'Cuando te nombran con @.'],
              ['mensajes', 'Mensajes', 'Avisos de conversaciones.'],
              ['grupos', 'Grupos y eventos', 'Lo que pasa en tus grupos.'],
              ['solicitudes', 'Solicitudes de permiso', 'Cuentas privadas que piden seguirte.'],
            ].map(([clave, titulo, texto]) => (
              <div className="fila-ajuste" key={clave}>
                <span className="icono"><IconBell /></span>
                <span className="texto"><b>{titulo}</b><small>{texto}</small></span>
                <Interruptor
                  activo={(aj?.avisos_tipos || {})[clave] !== false}
                  onChange={(v) => guardarAjuste({ avisos_tipos: { ...(aj?.avisos_tipos || {}), [clave]: v } })}
                  etiqueta={titulo}
                />
              </div>
            ))}
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
            <div className="titulo">Idioma, horario y datos</div>
            <div className="fila-ajuste">
              <span className="icono"><IconGlobe /></span>
              <span className="texto">
                <b>Idioma</b>
                <small>El que prefieres para Moon. Se guarda en tu cuenta.</small>
              </span>
              <Opciones
                etiqueta="Idioma"
                valor={aj?.idioma || 'es'}
                onChange={(v) => guardarAjuste({ idioma: v })}
                opciones={[{ id: 'es', label: 'Español' }, { id: 'en', label: 'English' }, { id: 'pt', label: 'Português' }]}
              />
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconMoon /></span>
              <span className="texto">
                <b>Oscuro por horario</b>
                <small>De noche cambia sola al tema oscuro y de día vuelve al claro.</small>
              </span>
              <Interruptor
                activo={!!aj?.tema_auto}
                onChange={(v) => guardarAjuste({ tema_auto: v })}
                etiqueta="Oscuro por horario"
              />
            </div>
            {aj?.tema_auto ? (
              <div className="fila-ajuste fila-horario">
                <span className="icono"><IconClock /></span>
                <span className="texto"><b>Desde y hasta</b><small>Fuera de ese rango se ve el tema claro.</small></span>
                <div className="horas">
                  <input
                    type="time"
                    className="input hora-input"
                    aria-label="Oscuro desde"
                    value={aj?.tema_desde || '20:00'}
                    onChange={(e) => setAj({ ...aj, tema_desde: e.target.value })}
                    onBlur={(e) => guardarAjuste({ tema_desde: e.target.value })}
                  />
                  <span className="muted">a</span>
                  <input
                    type="time"
                    className="input hora-input"
                    aria-label="Oscuro hasta"
                    value={aj?.tema_hasta || '07:00'}
                    onChange={(e) => setAj({ ...aj, tema_hasta: e.target.value })}
                    onBlur={(e) => guardarAjuste({ tema_hasta: e.target.value })}
                  />
                </div>
              </div>
            ) : null}
            <div className="fila-ajuste">
              <span className="icono"><IconImage /></span>
              <span className="texto">
                <b>Ahorro de datos</b>
                <small>Baja la calidad de las fotos y no carga videos solos.</small>
              </span>
              <Interruptor
                activo={!!aj?.ahorro_datos}
                onChange={(v) => guardarAjuste({ ahorro_datos: v })}
                etiqueta="Ahorro de datos"
              />
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
            <div className="titulo">PIN para abrir Moon</div>
            <div className="fila-ajuste">
              <span className="icono"><IconLock /></span>
              <span className="texto">
                <b>{aj?.tiene_pin ? 'PIN puesto' : 'Sin PIN'}</b>
                <small>
                  {aj?.tiene_pin
                    ? 'Además de tu contraseña, Moon pide este PIN para abrir la aplicación.'
                    : 'Un PIN de 4 a 8 números para que nadie entre si te prestan el teléfono.'}
                </small>
              </span>
              <span className={`pill ${aj?.tiene_pin ? 'pill-viva' : ''}`}>
                {aj?.tiene_pin ? (aj?.bloqueo_activo ? 'Activo' : 'Guardado') : 'Apagado'}
              </span>
            </div>
            <form onSubmit={guardarPin} className="pin-form" style={{ padding: '0 12px 12px' }}>
              <div className="field">
                <label>{aj?.tiene_pin ? 'Nuevo PIN (déjalo vacío para quitarlo)' : 'Elige tu PIN (4 a 8 números)'}</label>
                <input
                  className="input pin-campo"
                  inputMode="numeric"
                  autoComplete="off"
                  maxLength={8}
                  placeholder="••••"
                  value={pin.nuevo}
                  onChange={(e) => setPin({ ...pin, nuevo: e.target.value.replace(/\D/g, '') })}
                />
              </div>
              {aj?.tiene_pin ? (
                <div className="field">
                  <label>PIN actual</label>
                  <input
                    className="input pin-campo"
                    inputMode="numeric"
                    autoComplete="off"
                    maxLength={8}
                    placeholder="••••"
                    value={pin.actual}
                    onChange={(e) => setPin({ ...pin, actual: e.target.value.replace(/\D/g, '') })}
                  />
                </div>
              ) : null}
              <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
                <button className="btn btn-primary btn-sm">{aj?.tiene_pin ? 'Cambiar el PIN' : 'Poner el PIN'}</button>
                {aj?.tiene_pin ? (
                  <button
                    type="button"
                    className="btn btn-outline btn-sm"
                    onClick={async () => {
                      try {
                        await api.post('/api/me/ajustes/pin', { pin: '', pin_actual: pin.actual.trim() });
                        setAj({ ...(aj || {}), tiene_pin: false, bloqueo_activo: false });
                        setPin({ nuevo: '', actual: '' });
                        flash('ok', 'PIN quitado');
                      } catch (err) { setPinMsg(err.message || 'No se pudo quitar el PIN'); }
                    }}
                  >
                    Quitar el PIN
                  </button>
                ) : null}
              </div>
              {pinMsg ? <p className="error-nota">{pinMsg}</p> : null}
            </form>
            {aj?.tiene_pin ? (
              <div className="fila-ajuste">
                <span className="icono"><IconShield /></span>
                <span className="texto">
                  <b>Pedir el PIN al abrir</b>
                  <small>Si lo apagas, el PIN queda guardado pero Moon no lo pide.</small>
                </span>
                <Interruptor
                  activo={!!aj?.bloqueo_activo}
                  onChange={async (v) => {
                    try {
                      const r = await api.post('/api/me/ajustes/pin/activar', { activo: v });
                      setAj({ ...(aj || {}), bloqueo_activo: r.bloqueo_activo });
                    } catch (e) { avisoError(e); }
                  }}
                  etiqueta="Pedir el PIN al abrir"
                />
              </div>
            ) : null}
          </div>

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

      {/* ---------------- ACCESIBILIDAD ---------------- */}
      {section === 'accesibilidad' ? (
        <>
          <div className="card ajustes-bloque">
            <div className="titulo">Que se lea y se sienta bien</div>
            <div className="fila-ajuste">
              <span className="icono"><IconEye /></span>
              <span className="texto">
                <b>Contraste alto</b>
                <small>Texto y bordes con más fuerza, para verlo sin esfuerzo.</small>
              </span>
              <Opciones
                etiqueta="Contraste"
                valor={opc.contraste}
                onChange={(v) => guardarOpc('contraste', v)}
                opciones={[{ id: 'normal', label: 'Normal' }, { id: 'alto', label: 'Alto' }]}
              />
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconAt /></span>
              <span className="texto">
                <b>Tamaño de la letra</b>
                <small>Se aplica a toda la app al instante.</small>
              </span>
              <Opciones
                etiqueta="Tamaño de la letra"
                valor={opc.texto}
                onChange={(v) => guardarOpc('texto', v)}
                opciones={[{ id: 'normal', label: 'Normal' }, { id: 'grande', label: 'Grande' }, { id: 'enorme', label: 'Enorme' }]}
              />
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconSpark /></span>
              <span className="texto">
                <b>Más contenido por pantalla</b>
                <small>Menos aire entre cosas: se ve más de una vez.</small>
              </span>
              <Interruptor activo={opc.denso === 'si'} onChange={(v) => guardarOpc('denso', v ? 'si' : 'no')} etiqueta="Más contenido por pantalla" />
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconRefresh /></span>
              <span className="texto">
                <b>Reducir el movimiento</b>
                <small>Menos animaciones: la app se queda quieta.</small>
              </span>
              <Interruptor activo={opc.movimiento === 'no'} onChange={(v) => guardarOpc('movimiento', v ? 'no' : 'si')} etiqueta="Reducir el movimiento" />
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconLock /></span>
              <span className="texto">
                <b>Cristal de las tarjetas</b>
                <small>Cuánto desenfoque llevan las superficies.</small>
              </span>
              <Opciones
                etiqueta="Cristal"
                valor={opc.vidrio}
                onChange={(v) => guardarOpc('vidrio', v)}
                opciones={[{ id: 'suave', label: 'Suave' }, { id: 'normal', label: 'Normal' }, { id: 'intenso', label: 'Intenso' }]}
              />
            </div>
          </div>
        </>
      ) : null}

      {/* ---------------- CONTENIDO ---------------- */}
      {section === 'contenido' ? (
        <>
          <div className="card ajustes-bloque">
            <div className="titulo">Palabras silenciadas</div>
            <p className="muted small" style={{ margin: '0 2px 10px' }}>
              Las publicaciones que traigan estas palabras no aparecerán en tu Inicio ni en tus búsquedas.
              Una por línea (o separadas por comas).
            </p>
            <textarea
              className="textarea"
              rows={5}
              placeholder="Una palabra por línea: spoilers, política, lo que no quieras ver"

              value={palabras}
              onChange={(e) => setPalabras(e.target.value)}
            />
            <div className="row" style={{ gap: 8, marginTop: 10, flexWrap: 'wrap' }}>
              <button
                className="btn btn-primary btn-sm"
                onClick={() => {
                  guardarSilenciadas(palabras);
                  toast.ok('Palabras guardadas');
                }}
              >
                Guardar palabras
              </button>
              <button
                className="btn btn-outline btn-sm"
                onClick={() => {
                  setPalabras('');
                  guardarSilenciadas('');
                  toast.ok('Lista vacía: vuelves a ver todo');
                }}
              >
                Borrar la lista
              </button>
              <span className="muted small">
                {palabrasSilenciadas().length > 0
                  ? `Ahora mismo se silencian ${palabrasSilenciadas().length} palabra(s).`
                  : 'No hay ninguna palabra silenciada.'}
              </span>
            </div>
          </div>

          <div className="card ajustes-bloque">
            <div className="titulo">Lo que ves al entrar</div>
            <div className="fila-ajuste">
              <span className="icono"><IconBell /></span>
              <span className="texto">
                <b>Sonido al recibir un mensaje</b>
                <small>Un aviso corto cuando llega algo nuevo.</small>
              </span>
              <Interruptor activo={opc.sonido === 'si'} onChange={(v) => guardarOpc('sonido', v ? 'si' : 'no')} etiqueta="Sonido de mensajes" />
            </div>
          </div>
        </>
      ) : null}

      {/* ---------------- MENSAJES ---------------- */}
      {section === 'mensajes' ? (
        <>
          <div className="card ajustes-bloque">
            <div className="titulo">Cómo se comportan tus mensajes</div>
            <div className="fila-ajuste">
              <span className="icono"><IconMail /></span>
              <span className="texto">
                <b>Reproducir las notas de voz solas</b>
                <small>La última nota se escucha al abrir la conversación.</small>
              </span>
              <Interruptor activo={opc.notas === 'si'} onChange={(v) => guardarOpc('notas', v ? 'si' : 'no')} etiqueta="Reproducir notas de voz solas" />
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconLock /></span>
              <span className="texto">
                <b>Quién puede escribirte</b>
                <small>Lo mismo que en Privacidad, aquí a mano.</small>
              </span>
              <Opciones
                etiqueta="Quién puede escribirte"
                valor={dmPrivacy}
                onChange={setDmPrivacy}
                opciones={[{ id: 'all', label: 'Todos' }, { id: 'following', label: 'Quienes sigo' }, { id: 'nobody', label: 'Nadie' }]}
              />
            </div>
            <div className="row" style={{ padding: '4px 12px 12px' }}>
              <button
                className="btn btn-primary btn-sm"
                onClick={async () => {
                  try {
                    await api.patch('/api/auth/privacy', { dm_privacy: dmPrivacy });
                    await refreshMe();
                    flash('ok', 'Listo: así se queda quién puede escribirte');
                  } catch (err) { flash('err', err.message); }
                }}
              >
                Guardar
              </button>
              <a className="btn btn-outline btn-sm" href="#/messages">Ir a Mensajes</a>
            </div>
          </div>

          <div className="card ajustes-bloque">
            <div className="titulo">Tus conversaciones</div>
            <p className="muted small" style={{ margin: '0 2px 8px' }}>
              Silenciar, archivar u ocultar se hace desde los tres puntos de cada conversación.
              Lo que archives queda en la pestaña «Archivadas».
            </p>
          </div>
        </>
      ) : null}

      {/* ---------------- BIENESTAR ---------------- */}
      {section === 'bienestar' ? (
        <>
          <div className="card ajustes-bloque">
            <div className="titulo">Tu tiempo en Moon</div>
            <div className="fila-ajuste">
              <span className="icono"><IconBell /></span>
              <span className="texto">
                <b>Recordarme descansar</b>
                <small>Un aviso suave cuando llevas mucho rato seguido.</small>
              </span>
              <Opciones
                etiqueta="Recordatorio de descanso"
                valor={opc.bienestar}
                onChange={(v) => guardarOpc('bienestar', v)}
                opciones={[
                  { id: '0', label: 'No' },
                  { id: '20', label: '20 min' },
                  { id: '40', label: '40 min' },
                  { id: '60', label: '1 hora' },
                ]}
              />
            </div>
            <p className="muted small" style={{ margin: '6px 2px 0' }}>
              {opc.bienestar === '0'
                ? 'Ahora mismo no te avisamos de nada.'
                : `Te avisaremos cuando lleves ${opc.bienestar} minutos seguidos con la app abierta.`}
            </p>
          </div>
        </>
      ) : null}

      {/* ---------------- DATOS ---------------- */}
      {section === 'datos' ? (
        <>
          <div className="card ajustes-bloque">
            <div className="titulo">Lo que tienes en Moon</div>
            {uso ? (
              <>
                <div className="rejilla-uso">
                  {[
                    ['Publicaciones', uso.publicaciones],
                    ['Fotos guardadas', uso.fotos],
                    ['Comentarios', uso.comentarios],
                    ['Mensajes enviados', uso.mensajes],
                    ['Grupos', uso.grupos],
                    ['Siguiendo', uso.siguiendo],
                    ['Seguidores', uso.seguidores],
                    ['Espacio usado', `${uso.megabytes} MB`],
                  ].map(([etiqueta, valor]) => (
                    <div className="dato-uso" key={etiqueta}>
                      <b>{valor}</b>
                      <small>{etiqueta}</small>
                    </div>
                  ))}
                </div>
                <p className="muted small" style={{ margin: '10px 2px 0' }}>
                  Tu cuenta se guarda en la base de datos de Moon. Las fotos ya no se pierden cuando el servidor se reinicia.
                </p>
              </>
            ) : uso === undefined ? (
              <div className="spinner" />
            ) : (
              <p className="muted small" style={{ margin: '2px 2px 0' }}>
                No se pudo leer el resumen ahora mismo. Vuelve a entrar dentro de un momento.
              </p>
            )}
          </div>

          <div className="card ajustes-bloque">
            <div className="titulo">Tus datos</div>
            <div className="fila-ajuste">
              <span className="icono"><IconLayers /></span>
              <span className="texto">
                <b>Exportar mis datos</b>
                <small>Descarga un archivo con tu perfil, publicaciones, comentarios, mensajes, me gusta, seguidores y ajustes.</small>
              </span>
              <button className="btn btn-outline btn-sm" onClick={exportarTodo}>Descargar copia</button>
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconSearch /></span>
              <span className="texto">
                <b>Historial de búsqueda</b>
                <small>
                  {historial.length === 0
                    ? 'No hay búsquedas guardadas.'
                    : `${historial.length} ${historial.length === 1 ? 'búsqueda guardada' : 'búsquedas guardadas'}: ${historial.slice(0, 3).map((b) => b.termino).join(' · ')}`}
                </small>
              </span>
              <button className="btn btn-outline btn-sm" disabled={historial.length === 0} onClick={borrarTodoElHistorial}>
                Borrar historial
              </button>
            </div>
            <div className="fila-ajuste">
              <span className="icono"><IconTrend /></span>
              <span className="texto">
                <b>Etiquetas que sigo</b>
                <small>
                  {misTags.length === 0
                    ? 'No sigues ninguna etiqueta todavía.'
                    : `${misTags.length} ${misTags.length === 1 ? 'etiqueta' : 'etiquetas'}: ${misTags.slice(0, 4).map((h) => `#${h.tag}`).join(' ')}`}
                </small>
              </span>
              <a className="btn btn-outline btn-sm" href="#/explore">Ver en Explorar</a>
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

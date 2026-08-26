// Moon — Ajustes: perfil, privacidad, seguridad, sesiones, 2FA, datos

import React, { useEffect, useState } from 'react';
import { api, getAccessToken } from '../api.js';
import { useAuth } from '../auth.jsx';
import { uploadMedia, API_URL } from '../api.js';
import Avatar from '../components/Avatar.jsx';
import { timeAgo } from '../utils.js';
import { IconTrash } from '../components/Icons.jsx';

export default function SettingsView({ tab }) {
  const { user, refreshMe, logout } = useAuth();
  const [section, setSection] = useState(tab === 'security' ? 'security' : 'profile');
  const [me, setMe] = useState(user);

  // Formulario perfil
  const [form, setForm] = useState({});
  useEffect(() => { if (me) setForm({ display_name: me.display_name || '', bio: me.bio || '', link: me.link || '', location: me.location || '' }); }, [me]);
  const [saving, setSaving] = useState(false);

  // Privacidad
  const [isPrivate, setIsPrivate] = useState(false);
  const [dmPrivacy, setDmPrivacy] = useState('all');
  useEffect(() => { if (me) { setIsPrivate(!!me.is_private); setDmPrivacy(me.dm_privacy || 'all'); } }, [me]);

  // Seguridad
  const [pw, setPw] = useState({ current_password: '', new_password: '' });
  const [sessions, setSessions] = useState([]);
  const [twofa, setTwofa] = useState({ step: null, temp_token: '', code: '', password: '' });
  const [alert, setAlert] = useState(null);

  useEffect(() => {
    api.get('/api/auth/me').then(setMe).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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

  async function changePassword(e) {
    e.preventDefault();
    if (pw.new_password.length < 8) { flash('err', 'La nueva contraseña debe tener al menos 8 caracteres'); return; }
    try {
      await api.post('/api/auth/change-password', pw);
      setPw({ current_password: '', new_password: '' });
      flash('ok', 'Contraseña cambiada. Se cerraron las demás sesiones.');
    } catch (err) { flash('err', err.message); }
  }

  async function loadSessions() {
    try { setSessions(await api.get('/api/auth/sessions')); } catch (e) { flash('err', e.message); }
  }

  async function revokeSession(id) {
    try { await api.del(`/api/auth/sessions/${id}`); loadSessions(); } catch (e) { flash('err', e.message); }
  }

  async function revokeAll() {
    if (!window.confirm('¿Cerrar sesión en todos los dispositivos?')) return;
    try { await api.post('/api/auth/sessions-all', {}); flash('ok', 'Sesiones cerradas'); loadSessions(); } catch (e) { flash('err', e.message); }
  }

  // 2FA
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
    if (!window.confirm('¿Desactivar la verificación en dos pasos?')) return;
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
      flash('ok', 'Foto actualizada');
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
    const pw = window.prompt('Escribe tu contraseña para confirmar la ELIMINACIÓN de tu cuenta:');
    if (!pw) return;
    if (!window.confirm('Esta acción es irreversible. Se borrarán tu perfil, publicaciones y datos. ¿Continuar?')) return;
    try {
      await api.del('/api/auth/account', { body: { password: pw } });
      await logout();
    } catch (err) { flash('err', err.message); }
  }

  if (!me) return <div className="spinner" />;

  return (
    <>
      <div className="topbar"><h1>Ajustes</h1></div>
      <div className="tabs">
        <button className={section === 'profile' ? 'active' : ''} onClick={() => setSection('profile')}>Perfil</button>
        <button className={section === 'privacy' ? 'active' : ''} onClick={() => setSection('privacy')}>Privacidad</button>
        <button className={section === 'security' ? 'active' : ''} onClick={() => { setSection('security'); loadSessions(); }}>Seguridad</button>
        <button className={section === 'data' ? 'active' : ''} onClick={() => setSection('data')}>Datos</button>
      </div>

      {alert ? <div className={`alert ${alert.kind}`}>{alert.msg}</div> : null}

      {section === 'profile' ? (
        <div className="card" style={{ padding: 20 }}>
          <div className="row" style={{ marginBottom: 20 }}>
            <Avatar user={me} size="lg" />
            <div>
              <div className="muted mb">Foto de perfil</div>
              <label className="btn btn-outline btn-sm" style={{ cursor: 'pointer' }}>
                Subir foto
                <input type="file" accept="image/jpeg,image/png,image/webp" hidden
                  onChange={(e) => { const f = e.target.files?.[0]; if (f) uploadPhoto('avatar', f); e.target.value = ''; }} />
              </label>
            </div>
            <div style={{ marginLeft: 'auto' }}>
              <div className="muted mb">Portada</div>
              <label className="btn btn-outline btn-sm" style={{ cursor: 'pointer' }}>
                Subir portada
                <input type="file" accept="image/jpeg,image/png,image/webp" hidden
                  onChange={(e) => { const f = e.target.files?.[0]; if (f) uploadPhoto('cover', f); e.target.value = ''; }} />
              </label>
            </div>
          </div>
          <form onSubmit={saveProfile}>
            <div className="field"><label>Nombre para mostrar</label>
              <input className="input" value={form.display_name || ''} maxLength={40}
                onChange={(e) => setForm({ ...form, display_name: e.target.value })} /></div>
            <div className="field"><label>Biografía</label>
              <textarea className="textarea" value={form.bio || ''} maxLength={300} rows={3}
                onChange={(e) => setForm({ ...form, bio: e.target.value })} /></div>
            <div className="field"><label>Sitio web</label>
              <input className="input" value={form.link || ''} maxLength={300}
                onChange={(e) => setForm({ ...form, link: e.target.value })} /></div>
            <div className="field"><label>Ubicación</label>
              <input className="input" value={form.location || ''} maxLength={80}
                onChange={(e) => setForm({ ...form, location: e.target.value })} /></div>
            <button className="btn" disabled={saving}>{saving ? 'Guardando…' : 'Guardar cambios'}</button>
          </form>
        </div>
      ) : null}

      {section === 'privacy' ? (
        <div className="card" style={{ padding: 20 }}>
          <form onSubmit={savePrivacy}>
            <div className="field row" style={{ justifyContent: 'space-between' }}>
              <div>
                <b>Cuenta privada</b>
                <div className="muted">Solo tus seguidores pueden ver tus publicaciones.</div>
              </div>
              <input type="checkbox" checked={isPrivate} onChange={(e) => setIsPrivate(e.target.checked)} />
            </div>
            <div className="field">
              <label>¿Quién puede enviarte mensajes?</label>
              <select className="select" value={dmPrivacy} onChange={(e) => setDmPrivacy(e.target.value)}>
                <option value="all">Todos</option>
                <option value="followers">Solo personas que sigues</option>
                <option value="nobody">Nadie</option>
              </select>
            </div>
            <button className="btn">Guardar privacidad</button>
          </form>
        </div>
      ) : null}

      {section === 'security' ? (
        <>
          <div className="card" style={{ padding: 20, marginBottom: 16 }}>
            <h3 style={{ marginTop: 0 }}>Cambiar contraseña</h3>
            <form onSubmit={changePassword}>
              <div className="field"><label>Contraseña actual</label>
                <input className="input" type="password" value={pw.current_password}
                  onChange={(e) => setPw({ ...pw, current_password: e.target.value })} required /></div>
              <div className="field"><label>Nueva contraseña (mín. 8 caracteres, letras y números)</label>
                <input className="input" type="password" value={pw.new_password}
                  onChange={(e) => setPw({ ...pw, new_password: e.target.value })} required minLength={8} /></div>
              <button className="btn">Cambiar contraseña</button>
            </form>
          </div>

          <div className="card" style={{ padding: 20, marginBottom: 16 }}>
            <div className="between">
              <div>
                <b>Verificación en dos pasos</b>
                <div className="muted">Enviamos un código por correo al iniciar sesión.</div>
              </div>
              {me.twofa_enabled ? (
                <form onSubmit={disable2fa}>
                  <input className="input" type="password" placeholder="Tu contraseña" value={twofa.password}
                    onChange={(e) => setTwofa({ ...twofa, password: e.target.value })} required
                    style={{ width: 180, marginRight: 8 }} />
                  <button className="btn btn-outline btn-sm">Desactivar</button>
                </form>
              ) : (
                twofa.step === 'confirm' ? (
                  <form onSubmit={confirm2fa}>
                    <input className="input code-input" placeholder="000000" value={twofa.code} maxLength={6}
                      onChange={(e) => setTwofa({ ...twofa, code: e.target.value })} required
                      style={{ width: 140, marginRight: 8 }} />
                    <button className="btn btn-sm">Confirmar</button>
                  </form>
                ) : (
                  <form onSubmit={enable2fa}>
                    <input className="input" type="password" placeholder="Tu contraseña" value={twofa.password}
                      onChange={(e) => setTwofa({ ...twofa, password: e.target.value })} required
                      style={{ width: 180, marginRight: 8 }} />
                    <button className="btn btn-sm">Activar</button>
                  </form>
                )
              )}
            </div>
          </div>

          <div className="card" style={{ padding: 20 }}>
            <div className="between">
              <b>Sesiones activas</b>
              <button className="btn-ghost btn-sm" onClick={revokeAll}>Cerrar todas</button>
            </div>
            {sessions.length === 0 ? <p className="muted">Cargando…</p> : (
              sessions.map((s) => (
                <div key={s.id} className="row between" style={{ padding: '10px 0', borderBottom: '1px solid var(--line-2)' }}>
                  <div>
                    <div style={{ fontWeight: 600 }}>{s.device || 'Web'} {s.revoked_at ? <span className="pill warn">cerrada</span> : null}</div>
                    <div className="muted">{s.ip} · creada {timeAgo(s.created_at)}{s.last_used_at ? ` · usada ${timeAgo(s.last_used_at)}` : ''}</div>
                  </div>
                  {!s.revoked_at ? (
                    <button className="btn-ghost btn-sm" onClick={() => revokeSession(s.id)}>
                      <IconTrash /> Cerrar
                    </button>
                  ) : null}
                </div>
              ))
            )}
          </div>
        </>
      ) : null}

      {section === 'data' ? (
        <div className="card" style={{ padding: 20 }}>
          <div className="between" style={{ padding: '8px 0' }}>
            <div>
              <b>Exportar mis datos</b>
              <div className="muted">Descarga un JSON con tu perfil, publicaciones, comentarios, likes y mensajes.</div>
            </div>
            <button className="btn btn-outline btn-sm" onClick={exportData}>Exportar</button>
          </div>
          <div className="between" style={{ padding: '8px 0', borderTop: '1px solid var(--line-2)' }}>
            <div>
              <b style={{ color: 'var(--danger)' }}>Eliminar mi cuenta</b>
              <div className="muted">Borra definitivamente tu cuenta y todos tus datos. Irreversible.</div>
            </div>
            <form onSubmit={deleteAccount}>
              <button className="btn btn-danger btn-sm">Eliminar cuenta</button>
            </form>
          </div>
        </div>
      ) : null}
    </>
  );
}

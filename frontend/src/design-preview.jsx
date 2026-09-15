// Moon — Galería del sistema de diseño («Órbita»)
// ============================================================
// Página de referencia con datos de ejemplo: sirve para revisar y ajustar
// la interfaz sin backend (abre /design.html en el servidor de desarrollo).
// Monta los componentes REALES de la aplicación, no copias.

import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';
import './styles.css';
import { AuthContext } from './auth.jsx';
import { ThemeToggle } from './components/LeftNav.jsx';
import PostCard from './components/PostCard.jsx';
import Avatar, { VerifiedBadge } from './components/Avatar.jsx';
import Composer from './components/Composer.jsx';
import Overlays from './components/Overlays.jsx';
import { PostSkeleton, ListSkeleton, ProfileSkeleton } from './components/Skeleton.jsx';
import { toast, confirmar, pedirTexto } from './ui.js';
import {
  IconHeart, IconComment, IconBookmark, IconHome, IconExplore, IconBell, IconMail,
  IconUser, IconSettings, IconShield, IconSpark, IconTrend, IconUsers, IconCheck,
  IconAlert, IconInfo, IconWarning, IconLock, IconImage, IconSend, IconTrend as IconT,
  IconGlobe, IconGrid, IconAt, IconPlus,
} from './components/Icons.jsx';

const usuarioDemo = {
  id: 1,
  username: 'alice',
  display_name: 'Alice Márquez',
  email: 'alice@moon.test',
  role: 'admin',
  avatar_url: '',
  cover_url: '',
  is_verified: true,
};

const authDemo = {
  user: usuarioDemo,
  setUser: () => {},
  loading: false,
  login: async () => ({}),
  register: async () => ({}),
  logout: () => {},
  verify2fa: async () => {},
  refreshMe: () => {},
  isAdmin: true,
};

const postDemo = {
  id: 101,
  content: 'Acabo de mudar el diseño de Moon al sistema «Órbita»: menos ruido, más foco. ¿Qué os parece el modo oscuro? 🌙 #diseño #moon',
  created_at: new Date(Date.now() - 1000 * 60 * 42).toISOString(),
  likes_count: 128,
  comments_count: 14,
  saves_count: 6,
  is_liked: false,
  is_saved: false,
  is_mine: true,
  author_username: 'alice',
  author_display_name: 'Alice Márquez',
  author_avatar_url: '',
  author_is_verified: true,
  images: [],
};

function Seccion({ id, titulo, children, descripcion }) {
  return (
    <section id={id} style={{ marginBottom: 34 }}>
      <h2 style={{ fontSize: 18, marginBottom: 4 }}>{titulo}</h2>
      {descripcion ? <p className="sub" style={{ marginBottom: 12 }}>{descripcion}</p> : null}
      {children}
    </section>
  );
}

function App() {
  const [post, setPost] = useState(postDemo);
  const [nombre, setNombre] = useState('');

  return (
    <AuthContext.Provider value={authDemo}>
      <div className="app" style={{ paddingTop: 26 }}>
        <div style={{ gridColumn: '1 / -1', maxWidth: 900 }}>
          <div className="between" style={{ marginBottom: 6 }}>
            <div className="brand" style={{ padding: 0 }}>
              <span className="dot" aria-hidden="true" />
              <span>Moon<small>Sistema de diseño · Órbita</small></span>
            </div>
            <div className="row">
              <ThemeToggle />
              <a className="btn btn-outline btn-sm" href="/">Ir a la aplicación</a>
            </div>
          </div>
          <p className="sub" style={{ marginBottom: 26 }}>
            Página de referencia con datos de ejemplo. Aquí se revisan los componentes reales
            (los mismos que usa la aplicación) antes de tocar nada.
          </p>

          <Seccion id="sec-acciones" titulo="Acciones" descripcion="Un solo acento para lo importante; el resto, contorno o texto.">
            <div className="row" style={{ flexWrap: 'wrap' }}>
              <button className="btn btn-primary"><IconPlus /> Principal</button>
              <button className="btn">Neutro</button>
              <button className="btn btn-outline">Contorno</button>
              <button className="btn btn-ghost">Texto</button>
              <button className="btn btn-danger">Peligro</button>
              <button className="btn btn-primary" disabled>Deshabilitado</button>
              <span className="icon-btn on"><IconHeart /></span>
              <span className="icon-btn"><IconBookmark /></span>
            </div>
            <div className="row" style={{ flexWrap: 'wrap', marginTop: 12 }}>
              <span className="pill">Píldora</span>
              <span className="pill accent"><IconSpark /> Acento</span>
              <span className="pill ok"><IconCheck /> Correcto</span>
              <span className="pill warn"><IconWarning /> Aviso</span>
              <span className="pill danger"><IconAlert /> Error</span>
              <span className="badge">12</span>
              <span className="badge quiet">3</span>
              <span className="verified"><span className="sr-only">verificado</span><IconCheck /></span>
            </div>
          </Seccion>

          <Seccion id="sec-formularios" titulo="Formularios">
            <div className="card card-pad" style={{ maxWidth: 520 }}>
              <div className="field">
                <label htmlFor="demo-1">Usuario</label>
                <input id="demo-1" className="input" placeholder="@alice" value={nombre} onChange={(e) => setNombre(e.target.value)} />
                <span className="hint">Entre 3 y 24 caracteres.</span>
              </div>
              <div className="field">
                <label htmlFor="demo-2">Código de verificación</label>
                <input id="demo-2" className="input code-input" placeholder="000000" maxLength={6} />
              </div>
              <div className="field">
                <label className="switch">
                  <input type="checkbox" defaultChecked />
                  <span className="track" />
                  <span>Recibir notificaciones</span>
                </label>
              </div>
              <div className="row"><button className="btn btn-primary btn-block">Guardar cambios</button></div>
            </div>
          </Seccion>

          <Seccion id="sec-publicacion" titulo="Publicación" descripcion="Reacciones, guardado, menú y estado de edición.">
            <div className="card" style={{ padding: 0, maxWidth: 640 }}>
              <PostCard post={post} onChanged={setPost} />
            </div>
          </Seccion>

          <Seccion id="sec-redactor" titulo="Redactor" descripcion="Anillo de caracteres, adjuntos por clic o arrastrando.">
            <div className="card" style={{ overflow: 'hidden', maxWidth: 640, padding: 0 }}>
              <Composer onCreated={(p) => toast.ok('Publicación creada (demo)')} />
            </div>
          </Seccion>

          <Seccion id="sec-avisos" titulo="Avisos y diálogos" descripcion="Sin ventanas del navegador: misma estética en todos los sistemas.">
            <div className="row" style={{ flexWrap: 'wrap' }}>
              <button className="btn btn-outline" onClick={() => toast.ok('Publicación guardada')}>Aviso correcto</button>
              <button className="btn btn-outline" onClick={() => toast.err('No se pudo conectar con el servidor')}>Aviso de error</button>
              <button className="btn btn-outline" onClick={() => toast.info('Te enviamos un código por correo')}>Aviso informativo</button>
              <button className="btn btn-outline" onClick={async () => {
                const ok = await confirmar({ title: '¿Eliminar esta publicación?', message: 'Se borrarán también sus comentarios.', confirmText: 'Eliminar', danger: true });
                toast.info(ok ? 'Confirmado' : 'Cancelado');
              }}>Diálogo de confirmación</button>
              <button className="btn btn-outline" onClick={async () => {
                const t = await pedirTexto({ title: 'Reportar publicación', label: '¿Por qué la reportas?', placeholder: 'Spam, acoso…', confirmText: 'Enviar' });
                if (t) toast.ok(`Reporte: ${t}`);
              }}>Diálogo de texto</button>
            </div>
          </Seccion>

          <Seccion id="sec-mensajes" titulo="Conversación" descripcion="Burbujas, reacciones, escritura en vivo y confirmación de lectura.">
            <div className="card card-pad" style={{ maxWidth: 640 }}>
              <div className="chat-messages" style={{ padding: 0 }}>
                <div className="msg">¿Viste el nuevo diseño? <span className="time">10:24</span></div>
                <div className="msg mine">Sí, acabo de subirlo 🌙 <span className="time">10:25 · leído</span></div>
                <div className="msg">Está mucho más limpio <span className="react">👍</span><span className="time">10:26</span></div>
                <span className="typing">escribiendo<i /><i /><i /></span>
              </div>
            </div>
          </Seccion>

          <Seccion id="sec-cargas" titulo="Carga y estados vacíos" descripcion="Nunca una pantalla en blanco: la forma de lo que va a llegar.">
            <div className="card" style={{ maxWidth: 640, padding: 0, overflow: 'hidden' }}>
              <PostSkeleton />
              <PostSkeleton lines={2} />
            </div>
            <div className="card mt" style={{ maxWidth: 640, padding: 0, overflow: 'hidden' }}>
              <ListSkeleton rows={2} />
            </div>
            <div className="card mt" style={{ maxWidth: 640, padding: 0, overflow: 'hidden' }}>
              <ProfileSkeleton />
            </div>
            <div className="card mt empty" style={{ maxWidth: 640 }}>
              <IconMail />
              <h3>Sin conversaciones</h3>
              <p>Busca a alguien en Explorar y toca «Mensaje» para empezar.</p>
              <button className="btn btn-primary btn-sm">Explorar personas</button>
            </div>
          </Seccion>

          <Seccion id="sec-admin" titulo="Estadísticas y tablas (administración)">
            <div className="stat-grid" style={{ padding: 0, marginBottom: 14 }}>
              {[['Usuarios', '1.284'], ['Publicaciones', '9.731'], ['Mensajes', '42.118'], ['Reportes abiertos', '3']].map(([k, v]) => (
                <div className="stat" key={k}><b>{v}</b><span>{k}</span></div>
              ))}
            </div>
            <div className="card" style={{ padding: 0, overflow: 'hidden' }}>
              <table className="table">
                <thead>
                  <tr><th>Usuario</th><th>Estado</th><th>Registro</th><th /></tr>
                </thead>
                <tbody>
                  <tr>
                    <td><span className="row"><Avatar user={usuarioDemo} size="sm" /> @alice <VerifiedBadge show /></span></td>
                    <td><span className="pill ok">activo</span></td>
                    <td className="muted">hoy</td>
                    <td><div className="actions"><button className="btn btn-outline btn-sm">Suspender</button></div></td>
                  </tr>
                  <tr>
                    <td><span className="row"><Avatar user={{ username: 'bob', display_name: 'Bob' }} size="sm" /> @bob</span></td>
                    <td><span className="pill warn">suspendido</span></td>
                    <td className="muted">ayer</td>
                    <td><div className="actions"><button className="btn btn-outline btn-sm">Activar</button></div></td>
                  </tr>
                </tbody>
              </table>
            </div>
          </Seccion>

          <Seccion id="sec-iconos" titulo="Iconografía" descripcion="Un solo trazo (1,8 px) en toda la aplicación.">
            <div className="row" style={{ flexWrap: 'wrap', gap: 14 }}>
              {[IconHome, IconExplore, IconBell, IconMail, IconUser, IconSettings, IconShield, IconSpark, IconTrend, IconT, IconUsers, IconCheck, IconAlert, IconInfo, IconWarning, IconLock, IconImage, IconSend, IconGlobe, IconGrid, IconAt, IconHeart, IconComment, IconBookmark].map((I, i) => (
                <span className="icon-btn" key={i}><I /></span>
              ))}
            </div>
          </Seccion>

          <Seccion id="sec-colores" titulo="Marcas de color" descripcion="Superficies, tinta y el acento aurora.">
            <div className="row" style={{ flexWrap: 'wrap', gap: 10 }}>
              {['--bg', '--surface', '--surface-2', '--surface-3', '--line', '--ink', '--ink-2', '--ink-3', '--accent', '--accent-2', '--accent-3', '--ok', '--warn', '--danger'].map((v) => (
                <div key={v} style={{ width: 120 }}>
                  <div style={{ height: 46, borderRadius: 12, background: `var(${v})`, border: '1px solid var(--line)' }} />
                  <small className="muted">{v}</small>
                </div>
              ))}
            </div>
          </Seccion>
        </div>
      </div>
      <Overlays />
    </AuthContext.Provider>
  );
}

createRoot(document.getElementById('root')).render(<App />);

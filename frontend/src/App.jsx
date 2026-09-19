// Moon — Aplicación (armazón + navegación por hash)
// ============================================================
// Estructura de tres columnas en escritorio (menú, contenido y descubrir),
// una sola columna con barra inferior en móvil. Todo el «chrome» de la
// aplicación vive aquí: avisos flotantes, diálogos, tema y contadores.

import React, { useEffect, useState } from 'react';
import { AuthProvider, useAuth } from './auth.jsx';
import { api } from './api.js';
import { parseHash } from './utils.js';
import TopNav, { AccesosRapidos } from './components/TopNav.jsx';
import LeftRail from './components/LeftRail.jsx';
import BottomNav from './components/BottomNav.jsx';
import RightRail from './components/RightRail.jsx';
import Overlays from './components/Overlays.jsx';
import DemoBanner from './components/DemoBanner.jsx';
import { PostSkeleton } from './components/Skeleton.jsx';
import AuthView from './views/AuthView.jsx';
import ResetView from './views/ResetView.jsx';
import FeedView from './views/FeedView.jsx';
import ExploreView from './views/ExploreView.jsx';
import ProfileView from './views/ProfileView.jsx';
import UserView from './views/UserView.jsx';
import PostView from './views/PostView.jsx';
import MessagesView from './views/MessagesView.jsx';
import NotificationsView from './views/NotificationsView.jsx';
import SettingsView from './views/SettingsView.jsx';
import AdminView from './views/AdminView.jsx';
import ContactosView from './views/ContactosView.jsx';
import GruposView from './views/GruposView.jsx';
import GrupoView from './views/GrupoView.jsx';
import BloqueoPin, { desbloqueado } from './components/BloqueoPin.jsx';
import { realtime } from './realtime.js';
import { setUnread, bump, useUnread } from './unread.js';
import { aplicarTema } from './theme.js';
import { aplicar as aplicarPrefs, sonar, leer as leerPref } from './prefs.js';
import { toast } from './ui.js';

function useRoute() {
  const [route, setRoute] = useState(() => parseHash(window.location.hash));
  useEffect(() => {
    const handler = () => setRoute(parseHash(window.location.hash));
    window.addEventListener('hashchange', handler);
    return () => window.removeEventListener('hashchange', handler);
  }, []);
  return route;
}

/** Barra fina de progreso mientras cambia de pantalla. */
function BarraProgreso() {
  const route = useRoute();
  const [visible, setVisible] = useState(false);
  const clave = route.parts.join('/');

  useEffect(() => {
    setVisible(true);
    const t = setTimeout(() => setVisible(false), 420);
    return () => clearTimeout(t);
  }, [clave]);

  if (!visible) return null;
  return (
    <div className="barra-progreso" aria-hidden="true"><span /></div>
  );
}

/* El cielo aurora: vive detrás de todo y no intercepta toques. */
function Cielo() {
  return (
    <div className="cielo" aria-hidden="true">
      <span className="cielo-estrellas" />
      <span className="cielo-estrellas-2" />
    </div>
  );
}

// Secciones que en el teléfono se comportan como pantalla completa: cada una
// trae su propia cabecera y su propio scroll.
const SECCIONES_PANTALLA = [
  'feed', 'explore', 'grupos', 'grupo', 'messages', 'profile', 'user',
  'notifications', 'settings', 'admin', 'amigos', 'contactos', 'contacts',
];

/** Aviso suave de descanso, según lo que se elija en Ajustes → Bienestar. */
function RecordatorioDescanso() {
  useEffect(() => {
    let temporizador = null;
    const armar = () => {
      if (temporizador) clearTimeout(temporizador);
      const minutos = Number(leerPref('bienestar') || 0);
      if (!minutos) return;
      temporizador = setTimeout(() => {
        toast.info(`Llevas ${minutos} minutos en Moon: estírate y mira lejos un momento`);
        armar();
      }, minutos * 60 * 1000);
    };
    armar();
    window.addEventListener('moon:prefs', armar);
    return () => {
      if (temporizador) clearTimeout(temporizador);
      window.removeEventListener('moon:prefs', armar);
    };
  }, []);
  return null;
}

function Shell({ children }) {
  const route = useRoute();
  const seccion = route.parts[0] || 'feed';
  const enHilo = seccion === 'messages' && !!route.parts[1];
  const completa = SECCIONES_PANTALLA.includes(seccion);
  return (
    <div
      className="marco"
      data-seccion={seccion}
      data-pantalla={completa ? 'completa' : 'normal'}
      data-hilo={enHilo ? 'si' : 'no'}
    >
      <Cielo />
      <TopNav />
      <div className="app">
        <LeftRail />
        <main className="main">
          <DemoBanner />
          {children}
        </main>
        <RightRail />
      </div>
      <BottomNav />
      <RecordatorioDescanso />
    </div>
  );
}

function UnreadProvider({ children }) {
  const { user } = useAuth();
  useEffect(() => {
    if (!user) return;
    realtime.start();
    const off = realtime.on((ev) => {
      if (ev.type === 'notification') {
        bump('notification');
      } else if (ev.type === 'message') {
        bump('message');
        sonar();
      } else if (ev.type === 'sync') {
        const data = ev.data || {};
        setUnread({ notifications: data.unread_notifications || 0, messages: data.unread_messages || 0 });
      }
    });
    realtime.send({ type: 'sync' });
    return off;
  }, [user]);

  // Al cambiar de ruta, pedir los contadores reales al servidor.
  useEffect(() => {
    if (!user) return;
    realtime.send({ type: 'sync' });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [window.location.hash]);

  // El título de la pestaña avisa de lo pendiente, como en las apps grandes.
  const unread = useUnread();
  useEffect(() => {
    const total = (unread.notifications || 0) + (unread.messages || 0);
    document.title = total > 0 ? `(${total > 9 ? '9+' : total}) Moon — red social` : 'Moon — red social';
  }, [unread.notifications, unread.messages]);

  return children;
}

function Router() {
  const route = useRoute();
  const { parts, params } = route;

  if (parts.length === 0) return <Navigate to="#/feed" />;
  switch (parts[0]) {
    case 'login':
    case 'register':
      return <AuthView mode={parts[0]} />;
    case 'reset':
      return <ResetView token={params.token} />;
    case 'feed':
      return <FeedView />;
    case 'explore':
      return <ExploreView initialQ={params.q} initialType={params.type} />;
    case 'post':
      return <PostView id={parts[1]} />;
    case 'user':
      return <UserView id={parts[1]} />;
    case 'profile':
      return <ProfileView tab={parts[1]} />;
    case 'messages':
      return <MessagesView conversationId={parts[1]} />;
    case 'notifications':
      return <NotificationsView />;
    case 'settings':
      return <SettingsView tab={parts[1]} />;
    case 'amigos':
    case 'contactos':
    case 'contacts':
      return <ContactosView />;
    case 'grupos':
      return <GruposView />;
    case 'grupo':
      return <GrupoView id={parts[1]} seccion={parts[2]} sub={parts[3]} />;
    case 'admin':
      return <AdminView tab={parts[1]} />;
    default:
      return <Navigate to="#/feed" />;
  }
}

function Navigate({ to }) {
  useEffect(() => { window.location.hash = to; }, [to]);
  return null;
}

function Cargando() {
  return (
    <div aria-busy="true" aria-live="polite">
      <span className="sr-only">Cargando Moon…</span>
      <PostSkeleton etiqueta="Abriendo Moon…" alto="60vh" />
    </div>
  );
}

function Gate() {
  const { user, loading } = useAuth();
  const route = useRoute();
  const { parts } = route;
  const isAuthPage = ['login', 'register', 'reset'].includes(parts[0] || '');
  const ruta = parts.join('/') || 'feed';
  const [candado, setCandado] = useState(() => (desbloqueado() ? 'no' : 'comprobando'));

  useEffect(() => { aplicarTema(); aplicarPrefs(); }, []);

  // ¿Hay PIN puesto y encendido? Entonces Moon no enseña nada hasta teclearlo.
  useEffect(() => {
    if (!user) { setCandado('no'); return; }
    if (desbloqueado()) { setCandado('no'); return; }
    let vivo = true;
    api.get('/api/me/ajustes')
      .then((aj) => { if (vivo) setCandado(aj?.tiene_pin && aj?.bloqueo_activo ? 'si' : 'no'); })
      .catch(() => { if (vivo) setCandado('no'); });
    return () => { vivo = false; };
  }, [user]);

  if (loading) return <Cargando />;

  if (!user) {
    if (isAuthPage) {
      return <div className="route-fade" key={ruta}><Router /></div>;
    }
    return <Navigate to="#/login" />;
  }

  if (isAuthPage) return <Navigate to="#/feed" />;

  if (candado === 'si') {
    return <BloqueoPin onAbierto={() => setCandado('no')} />;
  }

  if (candado === 'comprobando') return <Cargando />;

  return (
    <UnreadProvider>
      <Shell>
        <div className="route-fade" key={ruta}>
          <Router />
        </div>
      </Shell>
    </UnreadProvider>
  );
}

export default function App() {
  return (
    <AuthProvider>
      <BarraProgreso />
      <Gate />
      <Overlays />
    </AuthProvider>
  );
}

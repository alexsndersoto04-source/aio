// Moon — Aplicación (armazón + navegación por hash)
// ============================================================
// Estructura de tres columnas en escritorio (menú, contenido y descubrir),
// una sola columna con barra inferior en móvil. Todo el «chrome» de la
// aplicación vive aquí: avisos flotantes, diálogos, tema y contadores.

import React, { useEffect, useState } from 'react';
import { AuthProvider, useAuth } from './auth.jsx';
import { parseHash } from './utils.js';
import LeftNav from './components/LeftNav.jsx';
import BottomNav, { FloatingCompose } from './components/BottomNav.jsx';
import RightRail from './components/RightRail.jsx';
import Overlays from './components/Overlays.jsx';
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
import { realtime } from './realtime.js';
import { setUnread, bump } from './unread.js';
import { aplicarTema } from './theme.js';

function useRoute() {
  const [route, setRoute] = useState(() => parseHash(window.location.hash));
  useEffect(() => {
    const handler = () => setRoute(parseHash(window.location.hash));
    window.addEventListener('hashchange', handler);
    return () => window.removeEventListener('hashchange', handler);
  }, []);
  return route;
}

function Shell({ children }) {
  return (
    <div className="app">
      <LeftNav />
      <main className="main">{children}</main>
      <RightRail />
      <BottomNav />
      <FloatingCompose />
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
      <PostSkeleton />
      <PostSkeleton lines={2} />
      <PostSkeleton lines={2} />
    </div>
  );
}

function Gate() {
  const { user, loading } = useAuth();
  const route = useRoute();
  const { parts } = route;
  const isAuthPage = ['login', 'register', 'reset'].includes(parts[0] || '');
  const ruta = parts.join('/') || 'feed';

  useEffect(() => { aplicarTema(); }, []);

  if (loading) return <Cargando />;

  if (!user) {
    if (isAuthPage) {
      return <div className="route-fade" key={ruta}><Router /></div>;
    }
    return <Navigate to="#/login" />;
  }

  if (isAuthPage) return <Navigate to="#/feed" />;

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
      <Gate />
      <Overlays />
    </AuthProvider>
  );
}

// Moon — Aplicación (shell + routing por hash)
// ============================================================

import React, { useEffect } from 'react';
import { AuthProvider, useAuth } from './auth.jsx';
import { parseHash } from './utils.js';
import LeftNav from './components/LeftNav.jsx';
import BottomNav from './components/BottomNav.jsx';
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
      <div className="rail" />
      <BottomNav />
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
  // Cuando cambia la ruta, actualizar contadores desde el servidor.
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

function Gate() {
  const { user, loading } = useAuth();
  const route = useRoute();
  const { parts } = route;
  const isAuthPage = ['login', 'register', 'reset'].includes(parts[0] || '');

  if (loading) return <div className="spinner" />;

  if (!user) {
    if (isAuthPage) return <Router />;
    return <Navigate to="#/login" />;
  }

  if (isAuthPage) return <Navigate to="#/feed" />;

  return (
    <UnreadProvider>
      <Shell><Router /></Shell>
    </UnreadProvider>
  );
}

export default function App() {
  return (
    <AuthProvider>
      <Gate />
    </AuthProvider>
  );
}

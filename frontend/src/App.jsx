import React, { useState, useEffect } from 'react';
import { useAuth } from './auth.jsx';
import LeftNav from './components/LeftNav.jsx';
import RightRail from './components/RightRail.jsx';
import TopBar from './components/TopBar.jsx';
import BottomNav from './components/BottomNav.jsx';
import AuthView from './views/AuthView.jsx';
import FeedView from './views/FeedView.jsx';
import ExploreView from './views/ExploreView.jsx';
import NotificationsView from './views/NotificationsView.jsx';
import MessagesView from './views/MessagesView.jsx';
import ProfileView from './views/ProfileView.jsx';
import SavedView from './views/SavedView.jsx';
import SettingsView from './views/SettingsView.jsx';

function useHash() {
  const [hash, setHash] = useState(window.location.hash || '#/feed');
  useEffect(() => {
    const handler = () => setHash(window.location.hash || '#/feed');
    window.addEventListener('hashchange', handler);
    return () => window.removeEventListener('hashchange', handler);
  }, []);
  return hash;
}

const TITLES = {
  feed: 'Inicio',
  explore: 'Explorar',
  notifications: 'Notificaciones',
  messages: 'Mensajes',
  profile: 'Perfil',
  saved: 'Guardados',
  settings: 'Ajustes',
};

export default function App() {
  const { user, loading } = useAuth();
  const hash = useHash();

  if (loading) {
    return (
      <div className="boot-screen">
        <p>Cargando Moon…</p>
      </div>
    );
  }

  if (!user) return <AuthView />;

  const parts = hash.replace('#/', '').split('/');
  const route = parts[0] || 'feed';
  const param = parts[1] || null;

  let view;
  switch (route) {
    case 'feed':
      view = <FeedView />;
      break;
    case 'explore':
      view = <ExploreView />;
      break;
    case 'notifications':
      view = <NotificationsView />;
      break;
    case 'messages':
      view = <MessagesView partnerId={param} />;
      break;
    case 'profile':
      view = <ProfileView userId={param || user.id} />;
      break;
    case 'saved':
      view = <SavedView />;
      break;
    case 'settings':
      view = <SettingsView />;
      break;
    default:
      view = <FeedView />;
  }

  return (
    <div className="shell">
      <LeftNav />
      <div className="shell-main">
        <TopBar title={TITLES[route] || 'Moon'} />
        <main className="shell-content">{view}</main>
      </div>
      <RightRail />
      <BottomNav />
    </div>
  );
}

import React, { useState, useEffect } from 'react';
import { useAuth } from './auth.jsx';
import Sidebar from './Sidebar.jsx';
import AuthView from './AuthView.jsx';
import FeedView from './FeedView.jsx';
import TrendingView from './TrendingView.jsx';
import ProfileView from './ProfileView.jsx';
import NotificationsView from './NotificationsView.jsx';
import MessagesView from './MessagesView.jsx';

function useHash() {
  const [hash, setHash] = useState(window.location.hash || '#/feed');
  useEffect(() => {
    const handler = () => setHash(window.location.hash || '#/feed');
    window.addEventListener('hashchange', handler);
    return () => window.removeEventListener('hashchange', handler);
  }, []);
  return hash;
}

export default function App() {
  const { user, loading } = useAuth();
  const hash = useHash();

  if (loading) return <div className="loading">Cargando...</div>;
  if (!user) return <AuthView />;

  const parts = hash.replace('#/', '').split('/');
  const route = parts[0] || 'feed';
  const param = parts[1] || null;

  let view;
  switch (route) {
    case 'feed':
      view = <FeedView />;
      break;
    case 'trending':
      view = <TrendingView />;
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
    default:
      view = <FeedView />;
  }

  return (
    <div className="app-layout">
      <Sidebar />
      <main className="main-content">
        {view}
      </main>
    </div>
  );
}

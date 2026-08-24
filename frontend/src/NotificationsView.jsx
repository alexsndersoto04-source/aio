import React, { useState, useEffect } from 'react';
import { api } from './api.js';

function notifText(n) {
  const u = n.from_username || 'Alguien';
  switch (n.type) {
    case 'follow': return `${u} te empezo a seguir`;
    case 'like': return `${u} le dio me gusta a tu publicacion`;
    case 'comment': return `${u} comento tu publicacion`;
    case 'message': return `${u} te envio un mensaje`;
    default: return `${u}: ${n.type}`;
  }
}

export default function NotificationsView() {
  const [notifs, setNotifs] = useState([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api('/api/notifications')
      .then(setNotifs)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <p className="muted">Cargando...</p>;

  return (
    <div>
      <h2>Notificaciones</h2>
      {notifs.length === 0 ? <p className="muted">Sin notificaciones</p> : (
        <div className="notif-list">
          {notifs.map(n => (
            <div key={n.id} className="notif-item">
              <span>{notifText(n)}</span>
              <span className="post-date">{new Date(n.created_at).toLocaleString()}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

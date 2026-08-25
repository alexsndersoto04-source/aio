import React, { useState, useEffect } from 'react';
import { api } from '../api.js';
import EmptyState from '../components/EmptyState.jsx';
import { BellIcon, HeartIcon, CommentIcon, ChatIcon, UserIcon } from '../components/Icons.jsx';
import { timeAgo } from '../utils.js';

const TYPE_META = {
  follow: { Icon: UserIcon, text: u => `@${u} te empezó a seguir` },
  like: { Icon: HeartIcon, text: u => `@${u} le dio me gusta a tu publicación` },
  comment: { Icon: CommentIcon, text: u => `@${u} comentó tu publicación` },
  message: { Icon: ChatIcon, text: u => `@${u} te envió un mensaje` },
};

export default function NotificationsView() {
  const [list, setList] = useState(null);

  useEffect(() => {
    api('/api/notifications')
      .then(setList)
      .catch(() => setList([]));
  }, []);

  if (list === null) {
    return (
      <div className="skeleton-list">
        <div className="card sk" />
        <div className="card sk" />
      </div>
    );
  }

  return (
    <>
      <div className="page-head">
        <h2>Notificaciones</h2>
      </div>
      {list.length === 0 ? (
        <EmptyState
          icon={BellIcon}
          title="Sin notificaciones"
          sub="Cuando alguien interactúe contigo, lo verás aquí."
        />
      ) : (
        <div className="card notif-list">
          {list.map(n => {
            const meta = TYPE_META[n.type] || {
              Icon: BellIcon,
              text: u => `${u}: ${n.type}`,
            };
            return (
              <div key={n.id} className={`notif ${n.is_read == 0 ? 'notif-new' : ''}`}>
                <span className="notif-icon">
                  <meta.Icon size={17} />
                </span>
                <div className="notif-text">
                  {meta.text(n.from_username || 'Alguien')}
                  <span className="muted sm">{timeAgo(n.created_at)}</span>
                </div>
                {n.is_read == 0 && <span className="notif-dot" />}
              </div>
            );
          })}
        </div>
      )}
    </>
  );
}

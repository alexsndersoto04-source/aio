import React from 'react';
import Imagen from './Imagen.jsx';
import { IconVerified } from './Icons.jsx';

export default function Avatar({ user, size = '', verified = false, className = '' }) {
  // En los avatares diminutos (pilas de «les gusta», presencia) dos iniciales
  // no caben y se leen cortadas: ahí va una sola letra.
  const mini = size === 'mini' || /(^|\s)mini(\s|$)/.test(className);
  const initials = (user?.display_name || user?.username || '?').slice(0, mini ? 1 : 2).toUpperCase();
  const cls = `avatar ${size} ${className}`.trim();
  return (
    <div className={cls} title={user?.username || ''}>
      {user?.avatar_url
        ? <Imagen src={user.avatar_url} alt={user.username} ratio="1 / 1" />
        : <span>{initials}</span>}
    </div>
  );
}

export function VerifiedBadge({ show }) {
  if (!show) return null;
  return <span className="verified"><IconVerified /></span>;
}

import React from 'react';
import { imgUrl } from '../api.js';
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
        ? <img src={imgUrl(user.avatar_url)} alt={user.username} loading="lazy" />
        : <span>{initials}</span>}
    </div>
  );
}

export function VerifiedBadge({ show }) {
  if (!show) return null;
  return <span className="verified"><IconVerified /></span>;
}

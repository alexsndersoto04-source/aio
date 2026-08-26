import React from 'react';
import { IconVerified } from './Icons.jsx';

export default function Avatar({ user, size = '', verified = false, className = '' }) {
  const initials = (user?.display_name || user?.username || '?').slice(0, 2).toUpperCase();
  const cls = `avatar ${size} ${className}`.trim();
  return (
    <div className={cls} title={user?.username || ''}>
      {user?.avatar_url
        ? <img src={user.avatar_url} alt={user.username} loading="lazy" />
        : <span>{initials}</span>}
    </div>
  );
}

export function VerifiedBadge({ show }) {
  if (!show) return null;
  return <span className="verified"><IconVerified /></span>;
}

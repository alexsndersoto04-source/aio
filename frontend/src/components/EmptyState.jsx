import React from 'react';
import { MoonLogo } from './Icons.jsx';

export default function EmptyState({ icon: Icon = MoonLogo, title, sub }) {
  return (
    <div className="card empty">
      <Icon size={34} />
      <h3>{title}</h3>
      {sub && <p className="muted">{sub}</p>}
    </div>
  );
}

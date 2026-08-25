import React from 'react';
import { useTheme } from '../theme.jsx';
import { SunIcon, MoonIcon, MoonLogo } from './Icons.jsx';

export default function TopBar({ title }) {
  const { theme, toggle } = useTheme();
  return (
    <header className="topbar">
      <a className="topbar-brand" href="#/feed">
        <MoonLogo size={20} />
        <h1>{title}</h1>
      </a>
      <button className="icon-btn" onClick={toggle} title="Cambiar tema">
        {theme === 'light' ? <MoonIcon size={18} /> : <SunIcon size={18} />}
      </button>
    </header>
  );
}

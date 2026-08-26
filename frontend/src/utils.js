// Moon — Utilidades compartidas

export function timeAgo(iso) {
  if (!iso) return '';
  const d = new Date(iso.includes('T') ? iso : iso.replace(' ', 'T') + (iso.includes('+') ? '' : 'Z'));
  const s = Math.floor((Date.now() - d.getTime()) / 1000);
  if (isNaN(s) || s < 0) return '';
  if (s < 60) return 'ahora';
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h} h`;
  const days = Math.floor(h / 24);
  if (days < 7) return `${days} d`;
  return d.toLocaleDateString('es-VE', { day: 'numeric', month: 'short' });
}

export function parseHash(hash) {
  const h = (hash || '#/feed').replace(/^#/, '');
  const [path, query = ''] = h.split('?');
  const parts = path.split('/').filter(Boolean);
  const params = {};
  for (const pair of query.split('&').filter(Boolean)) {
    const [k, v] = pair.split('=');
    params[decodeURIComponent(k)] = decodeURIComponent(v || '');
  }
  return { parts, params };
}

export function linkify(text) {
  if (!text) return text;
  const esc = text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  return esc
    .replace(/(https?:\/\/[^\s]+)/g, '<a href="$1" target="_blank" rel="noopener noreferrer">$1</a>')
    .replace(/#([A-Za-z0-9_]+)/g, '<a class="hash" href="#/explore?q=$1&type=posts">#$1</a>')
    .replace(/@([A-Za-z0-9_.-]+)/g, '<a href="#/user/$1">@$1</a>');
}

export function debounce(fn, ms = 350) {
  let t;
  return (...args) => {
    clearTimeout(t);
    t = setTimeout(() => fn(...args), ms);
  };
}

export function plural(n, singular, pluralForm) {
  return `${n} ${n === 1 ? singular : (pluralForm || singular + 's')}`;
}

export function humanNumber(n) {
  const num = Number(n) || 0;
  if (num >= 1_000_000) return (num / 1_000_000).toFixed(1).replace('.0', '') + 'M';
  if (num >= 1_000) return (num / 1_000).toFixed(1).replace('.0', '') + 'K';
  return String(num);
}

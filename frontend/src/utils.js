const MESES = ['ene', 'feb', 'mar', 'abr', 'may', 'jun', 'jul', 'ago', 'sep', 'oct', 'nov', 'dic'];

export function timeAgo(iso) {
  if (!iso) return '';
  const d = new Date(iso);
  if (isNaN(d.getTime())) return '';
  const diff = (Date.now() - d.getTime()) / 1000;
  if (diff < 60) return 'ahora';
  if (diff < 3600) return `hace ${Math.floor(diff / 60)} min`;
  if (diff < 86400) return `hace ${Math.floor(diff / 3600)} h`;
  if (diff < 172800) return 'ayer';
  const fecha = `${d.getDate()} ${MESES[d.getMonth()]}`;
  return d.getFullYear() === new Date().getFullYear() ? fecha : `${fecha} ${d.getFullYear()}`;
}

const SAVED_KEY = 'moon_saved_v1';

export function loadSaved() {
  try {
    return JSON.parse(localStorage.getItem(SAVED_KEY)) || [];
  } catch {
    return [];
  }
}

// Devuelve true si quedó guardada, false si se quitó.
export function toggleSaved(post) {
  const list = loadSaved();
  const i = list.findIndex(s => s.post.id === post.id);
  if (i >= 0) list.splice(i, 1);
  else list.unshift({ post, saved_at: new Date().toISOString() });
  localStorage.setItem(SAVED_KEY, JSON.stringify(list));
  return i < 0;
}

export function isSaved(post) {
  return loadSaved().some(s => s.post.id === post.id);
}

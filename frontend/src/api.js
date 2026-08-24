const BASE = import.meta.env.VITE_API_URL || '';

export async function api(path, opts = {}) {
  const token = localStorage.getItem('moon_token');
  const headers = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const res = await fetch(`${BASE}${path}`, {
    ...opts,
    headers: { ...headers, ...opts.headers },
  });

  const data = await res.json();
  if (!res.ok) {
    throw new Error(data.error || `Error ${res.status}`);
  }
  return data;
}

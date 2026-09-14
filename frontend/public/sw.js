/* Moon — Service worker: capa offline de la aplicación instalada
 * ================================================================
 * Estrategia por tipo de petición:
 *   · navegación   → red primero; sin red, el shell cacheado (la app abre
 *                     aunque el teléfono esté sin señal y muestra lo último)
 *   · assets con hash y estáticos → caché primero (son inmutables)
 *   · /api/* y /ws → siempre red (datos vivos; nunca cachear)
 */

const VERSION = 'moon-shell-v1';
const SHELL = ['/', '/index.html', '/manifest.webmanifest', '/icono-192.png', '/icono-512.png'];

self.addEventListener('install', (ev) => {
  ev.waitUntil(
    caches.open(VERSION).then((c) => c.addAll(SHELL)).then(() => self.skipWaiting())
  );
});

self.addEventListener('activate', (ev) => {
  ev.waitUntil(
    caches.keys()
      .then((claves) => Promise.all(claves.filter((k) => k !== VERSION).map((k) => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', (ev) => {
  const req = ev.request;
  if (req.method !== 'GET') return;

  const url = new URL(req.url);
  if (url.origin !== self.location.origin) return;

  // Datos y tiempo real: nunca se cachean.
  if (url.pathname.startsWith('/api/') || url.pathname === '/ws') return;

  // Navegación: red primero, shell cacheado como respaldo.
  if (req.mode === 'navigate') {
    ev.respondWith(
      fetch(req)
        .then((res) => {
          const copia = res.clone();
          caches.open(VERSION).then((c) => c.put('/index.html', copia));
          return res;
        })
        .catch(() => caches.match('/index.html'))
    );
    return;
  }

  // Estáticos: caché primero; lo que no esté, se pide y se guarda.
  ev.respondWith(
    caches.match(req).then(
      (enCache) =>
        enCache ||
        fetch(req).then((res) => {
          if (res.ok && (url.pathname.startsWith('/assets/') || /\.(png|svg|webmanifest|css|js)$/.test(url.pathname))) {
            const copia = res.clone();
            caches.open(VERSION).then((c) => c.put(req, copia));
          }
          return res;
        })
    )
  );
});

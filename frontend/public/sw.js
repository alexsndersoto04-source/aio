/* Moon — Servicio en segundo plano (avisos al teléfono + capa sin conexión)
 * ======================================================================
 * Hace dos cosas:
 *   1. Recibe los avisos que manda el servidor y los muestra como
 *      notificación del sistema, aunque Moon esté cerrada; al tocar el aviso
 *      abre Moon en la pantalla correcta.
 *   2. Deja la aplicación abrible sin señal (shell cacheado). Los datos
 *      vivos (/api/*, /ws) NUNCA se guardan: nada de información vieja.
 *
 * Estrategia por tipo de petición:
 *   · /api/* y /ws  → siempre red (datos vivos)
 *   · navegación    → red primero; si no hay red, el shell cacheado
 *   · estáticos     → caché primero (los /assets/ llevan hash e son inmutables)
 */

const VERSION = 'moon-shell-v2';
const SHELL = ['/', '/index.html', '/manifest.webmanifest', '/iconos/icono-192.png', '/iconos/icono-512.png'];

self.addEventListener('install', (ev) => {
  ev.waitUntil(
    caches.open(VERSION)
      .then((c) => Promise.all(SHELL.map((u) => c.add(u).catch(() => undefined))))
      .then(() => self.skipWaiting())
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
          if (res.ok && (url.pathname.startsWith('/assets/') || /\.(png|svg|jpg|jpeg|webp|ico|webmanifest|woff2?|css|js)$/.test(url.pathname))) {
            const copia = res.clone();
            caches.open(VERSION).then((c) => c.put(req, copia));
          }
          return res;
        })
    )
  );
});

/* ------------------------ Avisos al teléfono ------------------------ */

self.addEventListener('push', (evento) => {
  let datos = {};
  try {
    datos = evento.data ? evento.data.json() : {};
  } catch {
    datos = { title: 'Moon', body: (evento.data && evento.data.text && evento.data.text()) || '' };
  }
  const titulo = datos.title || 'Moon';
  const opciones = {
    body: datos.body || 'Tienes algo nuevo en Moon',
    icon: datos.icon || '/iconos/icono-192.png',
    badge: datos.badge || '/iconos/icono-192.png',
    tag: datos.etiqueta || 'moon',
    renotify: true,
    data: { url: datos.url || '#/feed' },
    vibrate: Array.isArray(datos.vibrar) && datos.vibrar.length ? datos.vibrar : [40, 30, 40],
    // Las llamadas se quedan en pantalla hasta que la persona las atienda o
    // las quite, para que no se pierdan mientras suena el teléfono.
    requireInteraction: datos.quedarse === true,
  };
  evento.waitUntil(self.registration.showNotification(titulo, opciones));
});

self.addEventListener('notificationclick', (evento) => {
  evento.notification.close();
  const destino = (evento.notification.data && evento.notification.data.url) || '#/feed';
  evento.waitUntil((async () => {
    const clientes = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
    for (const cliente of clientes) {
      if ('focus' in cliente) {
        cliente.postMessage({ tipo: 'ir-a', url: destino });
        return cliente.focus();
      }
    }
    if (self.clients.openWindow) return self.clients.openWindow(`/${destino}`);
    return undefined;
  })());
});

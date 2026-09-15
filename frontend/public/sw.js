// Moon — Servicio en segundo plano (avisos al teléfono)
// ============================================================
// Sirve para dos cosas:
//   1. Recibir los avisos que manda el servidor y mostrarlos como
//      notificación del sistema, aunque Moon esté cerrado.
//   2. Al tocar el aviso, abrir Moon en la pantalla correcta.
//
// A propósito NO guarda copias de la aplicación: así nunca se ve una versión
// vieja después de una actualización.

self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', (evento) => evento.waitUntil(self.clients.claim()));

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
    vibrate: [40, 30, 40],
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

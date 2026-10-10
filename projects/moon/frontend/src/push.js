// Moon — Avisos al teléfono (Web Push)
// ============================================================
// Activa las notificaciones del sistema para este dispositivo. Funciona en
// Android y en computadora con Chrome, Edge o Firefox. En iPhone solo a
// partir de iOS 16.4 y con Moon instalada en la pantalla de inicio; si no se
// puede, se informa en lenguaje llano y no se rompe nada.

import { api } from './api.js';

export function soportaAvisos() {
  return typeof navigator !== 'undefined'
    && 'serviceWorker' in navigator
    && typeof window !== 'undefined'
    && 'PushManager' in window
    && 'Notification' in window;
}

export function esIOS() {
  const ua = navigator.userAgent || '';
  return /iPad|iPhone|iPod/.test(ua) || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);
}

export function instalada() {
  return (window.matchMedia && window.matchMedia('(display-mode: standalone)').matches)
    || navigator.standalone === true;
}

export function permisoActual() {
  if (typeof Notification === 'undefined') return 'no-soportado';
  return Notification.permission; // 'granted' | 'denied' | 'default'
}

function claveABytes(base64) {
  const relleno = '='.repeat((4 - (base64.length % 4)) % 4);
  const limpio = (base64 + relleno).replace(/-/g, '+').replace(/_/g, '/');
  const crudo = atob(limpio);
  const bytes = new Uint8Array(crudo.length);
  for (let i = 0; i < crudo.length; i += 1) bytes[i] = crudo.charCodeAt(i);
  return bytes;
}

/**
 * Espera a que el servicio en segundo plano esté listo, pero sin quedarse
 * colgado: si no arranca (navegador en privado, permiso bloqueado, demo…),
 * a los 3 segundos se sigue igual y se mira lo que haya registrado.
 */
async function registroActivo(esperaMs = 3000) {
  if (!('serviceWorker' in navigator)) return null;
  let porTiempo = false;
  const listo = await Promise.race([
    navigator.serviceWorker.ready.catch(() => null),
    new Promise((resolver) => setTimeout(() => { porTiempo = true; resolver(null); }, esperaMs)),
  ]);
  if (listo && listo.active) return listo;
  if (!porTiempo && listo) return listo;
  try {
    return (await navigator.serviceWorker.getRegistration()) || listo;
  } catch {
    return listo;
  }
}

/** Registra el servicio en segundo plano (se llama al arrancar la app). */
export async function registrarServicio() {
  if (!('serviceWorker' in navigator)) return null;
  try {
    return await navigator.serviceWorker.register('/sw.js');
  } catch (e) {
    console.warn('[push] no se pudo registrar el servicio:', e.message);
    return null;
  }
}

/** Enciende los avisos en ESTE dispositivo. Devuelve cuántos quedan activos. */
export async function activarAvisos() {
  if (!soportaAvisos()) {
    throw new Error(esIOS()
      ? 'En iPhone hay que instalar Moon primero: Compartir → «Añadir a pantalla de inicio», y luego activarlo desde ahí.'
      : 'Este navegador no admite los avisos al teléfono.');
  }
  const permiso = await Notification.requestPermission();
  if (permiso !== 'granted') throw new Error('No se concedió el permiso de avisos');

  const registro = await registroActivo();
  if (!registro) throw new Error('Este navegador no tiene listo el servicio de avisos. Prueba a recargar la página.');
  const { key } = await api.get('/api/push/public-key');
  if (!key) throw new Error('El servidor no tiene listos los avisos');

  let suscripcion = await registro.pushManager.getSubscription();
  if (!suscripcion) {
    suscripcion = await registro.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: claveABytes(key),
    });
  }
  const res = await api.post('/api/push/subscribe', {
    endpoint: suscripcion.endpoint,
    keys: {
      p256dh: suscripcion.toJSON().keys.p256dh,
      auth: suscripcion.toJSON().keys.auth,
    },
    device: navigator.userAgent,
  });
  return res;
}

/** Apaga los avisos en ESTE dispositivo. */
export async function desactivarAvisos() {
  if (!('serviceWorker' in navigator)) return { ok: true };
  const registro = await registroActivo();
  const suscripcion = registro && registro.pushManager ? await registro.pushManager.getSubscription().catch(() => null) : null;
  if (suscripcion) {
    await api.del('/api/push/subscribe', { body: { endpoint: suscripcion.endpoint } }).catch(() => {});
    await suscripcion.unsubscribe().catch(() => {});
  } else {
    await api.del('/api/push/subscribe', { body: {} }).catch(() => {});
  }
  return { ok: true };
}

export async function estadoAvisos() {
  const soporte = soportaAvisos();
  const permiso = permisoActual();
  let dispositivoActivo = false;
  if (soporte) {
    const registro = await registroActivo();
    const suscripcion = registro && registro.pushManager ? await registro.pushManager.getSubscription().catch(() => null) : null;
    dispositivoActivo = !!suscripcion;
  }
  let servidor = { dispositivos: 0, ultimo: null };
  try { servidor = await api.get('/api/push/estado'); } catch { /* sin conexión */ }
  return { soporte, permiso, dispositivoActivo, ...servidor, ios: esIOS(), instalada: instalada() };
}

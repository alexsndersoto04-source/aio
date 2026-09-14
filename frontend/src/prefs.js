// Moon — Preferencias de la persona (apariencia y avisos)
// ============================================================
// Cada ajuste se guarda en el dispositivo (localStorage) y se aplica al
// documento como atributo `data-*`, para que el CSS reaccione sin tocar
// nada más. `index.html` aplica el tema antes del primer pintado; el resto
// se aplica al arrancar la aplicación.
//
//   · tema     → claro / oscuro / sistema   (lo aplica theme.js)
//   · vidrio   → suave / normal / intenso   (cuánto desenfoque y brillo)
//   · texto    → normal / grande / enorme   (tamaño base de la letra)
//   · movimiento → sí / no                  (animaciones cortas o nada)
//   · sonido   → sí / no                    (aviso sonoro de mensajes)
//   · denso    → sí / no                    (más contenido por pantalla)

import { almacen } from './almacen.js';

export const CLAVES = {
  vidrio: 'moon_vidrio',
  texto: 'moon_texto',
  movimiento: 'moon_movimiento',
  sonido: 'moon_sonido',
  denso: 'moon_denso',
};

export const OPCIONES = {
  vidrio: ['suave', 'normal', 'intenso'],
  texto: ['normal', 'grande', 'enorme'],
  movimiento: ['si', 'no'],
  sonido: ['si', 'no'],
  denso: ['no', 'si'],
};

export const DEFECTOS = {
  vidrio: 'normal',
  texto: 'normal',
  movimiento: 'si',
  sonido: 'si',
  denso: 'no',
};

const escuchas = new Set();

export function leer(clave) {
  const v = almacen.leer(CLAVES[clave]);
  return OPCIONES[clave].includes(v) ? v : DEFECTOS[clave];
}

export function leerTodo() {
  const out = {};
  for (const clave of Object.keys(CLAVES)) out[clave] = leer(clave);
  return out;
}

/** Aplica al <html> todas las preferencias de apariencia. */
export function aplicar() {
  const raiz = document.documentElement;
  raiz.dataset.vidrio = leer('vidrio');
  raiz.dataset.texto = leer('texto');
  raiz.dataset.movimiento = leer('movimiento');
  raiz.dataset.denso = leer('denso');
}

export function guardar(clave, valor) {
  if (!OPCIONES[clave] || !OPCIONES[clave].includes(valor)) return;
  almacen.escribir(CLAVES[clave], valor);
  aplicar();
  escuchas.forEach((fn) => fn(leerTodo()));
  window.dispatchEvent(new CustomEvent('moon:prefs', { detail: leerTodo() }));
}

export function suscribir(fn) {
  escuchas.add(fn);
  return () => escuchas.delete(fn);
}

/** Aviso sonoro corto (mensajes nuevos). Sin archivos: dos notas suaves. */
export function sonar() {
  if (leer('sonido') !== 'si') return;
  try {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    const ctx = new AC();
    const t0 = ctx.currentTime;
    [880, 1320].forEach((f, i) => {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = 'sine';
      osc.frequency.value = f;
      gain.gain.setValueAtTime(0.0001, t0 + i * 0.11);
      gain.gain.exponentialRampToValueAtTime(0.06, t0 + i * 0.11 + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, t0 + i * 0.11 + 0.22);
      osc.connect(gain).connect(ctx.destination);
      osc.start(t0 + i * 0.11);
      osc.stop(t0 + i * 0.11 + 0.24);
    });
    setTimeout(() => ctx.close().catch(() => {}), 700);
  } catch { /* sin audio disponible: se ignora */ }
}

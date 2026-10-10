// Moon — Tema (claro / oscuro / sistema)
// ============================================================
// El tema se guarda en `localStorage` y se aplica en el atributo
// `data-theme` del <html>. `index.html` trae un script mínimo que lo
// aplica antes del primer pintado: sin eso, la pantalla aparece clara y
// salta a oscura (el destello blanco clásico).

import { almacen } from './almacen.js';

const CLAVE = 'moon_theme';
const OPCIONES = ['light', 'dark', 'system'];

export function prefersDark() {
  return window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
}

export function getPreferencia() {
  const guardado = almacen.leer(CLAVE);
  return OPCIONES.includes(guardado) ? guardado : 'system';
}

/** Tema realmente aplicado: 'light' o 'dark'. */
export function getTemaAplicado() {
  const pref = getPreferencia();
  if (pref === 'system') return prefersDark() ? 'dark' : 'light';
  return pref;
}

export function aplicarTema(pref = getPreferencia()) {
  const tema = pref === 'system' ? (prefersDark() ? 'dark' : 'light') : pref;
  document.documentElement.dataset.theme = tema;
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.setAttribute('content', tema === 'dark' ? '#08090e' : '#ffffff');
  return tema;
}

export function setPreferencia(pref) {
  if (!OPCIONES.includes(pref)) return;
  almacen.escribir(CLAVE, pref);
  aplicarTema(pref);
  window.dispatchEvent(new CustomEvent('moon:tema', { detail: pref }));
}

/** Sigue los cambios del sistema mientras la preferencia sea «sistema». */
export function observarSistema() {
  if (!window.matchMedia) return () => {};
  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  const handler = () => { if (getPreferencia() === 'system') aplicarTema('system'); };
  mq.addEventListener('change', handler);
  return () => mq.removeEventListener('change', handler);
}

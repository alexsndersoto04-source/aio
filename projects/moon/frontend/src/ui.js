// Moon — Avisos y diálogos con la identidad de la aplicación
// ============================================================
// Sustituye a `alert`, `window.confirm` y `window.prompt`: son ventanas del
// navegador, no se pueden diseñar, bloquean la pestaña y se ven distintas en
// cada sistema. Aquí todo pasa por el mismo lenguaje visual de Moon.
//
//   import { toast, confirmar, pedirTexto } from '../ui.js';
//
//   toast.ok('Publicación creada');
//   if (await confirmar({ title: '¿Eliminar?', danger: true })) ...
//   const motivo = await pedirTexto({ title: 'Motivo del reporte' });

const listeners = new Set();
let seq = 0;

export function subscribe(fn) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function emit(event) {
  for (const fn of Array.from(listeners)) fn(event);
}

// ---------- Avisos ----------
export const toast = {
  ok: (message) => emit({ kind: 'toast', level: 'ok', message, id: ++seq }),
  err: (message) => emit({ kind: 'toast', level: 'err', message, id: ++seq }),
  info: (message) => emit({ kind: 'toast', level: 'info', message, id: ++seq }),
  cerrar: (id) => emit({ kind: 'toast-cerrar', id }),
};

/** Convierte cualquier error en un aviso legible. */
export function avisoError(error) {
  const texto = (error && error.message) || 'Algo no salió bien. Inténtalo de nuevo.';
  toast.err(texto);
}

// ---------- Diálogos ----------
/**
 * Diálogo de confirmación. Devuelve `true` si la persona confirma.
 * `danger: true` pinta la acción principal en rojo (borrados, bloqueos).
 */
export function confirmar({
  title = '¿Continuar?',
  message = '',
  confirmText = 'Confirmar',
  cancelText = 'Cancelar',
  danger = false,
  requerirPassword = false,
} = {}) {
  return new Promise((resolve) => {
    emit({
      kind: 'dialogo',
      id: ++seq,
      dialogo: { tipo: 'confirmar', title, message, confirmText, cancelText, danger, requerirPassword },
      resolver: resolve,
    });
  });
}

/**
 * Diálogo que pide un texto (motivos de reporte, notas de resolución…).
 * Devuelve el texto o `null` si se cancela.
 */
export function pedirTexto({
  title = 'Escribe el texto',
  label = '',
  placeholder = '',
  value = '',
  confirmText = 'Enviar',
  multiline = false,
  requerido = true,
} = {}) {
  return new Promise((resolve) => {
    emit({
      kind: 'dialogo',
      id: ++seq,
      dialogo: { tipo: 'texto', title, label, placeholder, value, confirmText, multiline, requerido },
      resolver: resolve,
    });
  });
}

// Moon — Almacenamiento a prueba de bloqueos
// ============================================================
// Algunos navegadores no dejan usar `localStorage` (modo privado, archivos
// abiertos con doble clic, permisos restringidos). Si eso pasa, se guarda en
// memoria: la sesión funciona igual durante esa visita.

const memoria = new Map();

function conRespaldo(accion, alternativa) {
  try {
    return accion();
  } catch {
    return alternativa();
  }
}

export const almacen = {
  leer(clave) {
    return conRespaldo(
      () => window.localStorage.getItem(clave),
      () => (memoria.has(clave) ? memoria.get(clave) : null)
    );
  },
  escribir(clave, valor) {
    conRespaldo(
      () => window.localStorage.setItem(clave, valor),
      () => { memoria.set(clave, valor); return null; }
    );
  },
  borrar(clave) {
    conRespaldo(
      () => window.localStorage.removeItem(clave),
      () => { memoria.delete(clave); return null; }
    );
  },
};

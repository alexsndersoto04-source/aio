// Moon — Límite de peticiones
// ============================================================
// Ventana deslizante en memoria. Es suficiente para un único proceso; si el
// servicio se escala a varias instancias, el contador debería vivir en la
// base de datos (Redis o una tabla).

const registros = new Map();

export function demasiadoRapido(clave, maximo, ventanaMs) {
  const ahora = Date.now();
  const previos = (registros.get(clave) || []).filter((t) => ahora - t < ventanaMs);
  if (previos.length >= maximo) {
    registros.set(clave, previos);
    return true;
  }
  previos.push(ahora);
  registros.set(clave, previos);
  return false;
}

// Limpieza periódica para no crecer sin control.
setInterval(() => {
  const ahora = Date.now();
  for (const [clave, marcas] of registros) {
    const vigentes = marcas.filter((t) => ahora - t < 3_600_000);
    if (vigentes.length === 0) registros.delete(clave);
    else registros.set(clave, vigentes);
  }
}, 600_000).unref?.();

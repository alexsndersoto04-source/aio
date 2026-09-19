// Moon — menús que siempre caen dentro de la pantalla
// ============================================================
// Un menú colocado «pegado» a su botón puede quedar fuera de la vista cuando
// la publicación está al final de la lista o la barra queda contra el borde:
// se abre el menú pero no se ve nada. Aquí se mide el botón y el menú al
// abrirse, y se coloca con coordenadas de pantalla: arriba del botón; si no
// cabe, debajo; y siempre arrimado a los bordes. Se dibuja fuera de la
// publicación (createPortal sobre el body) para que ningún contenedor lo
// recorte ni lo deje debajo del fondo oscurecido.

import { useCallback, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

export function useFlotante(abierto, { margen = 8, distancia = 8 } = {}) {
  const anclaRef = useRef(null);
  const cajaRef = useRef(null);
  const [estilo, setEstilo] = useState(null);

  const colocar = useCallback(() => {
    const ancla = anclaRef.current;
    const caja = cajaRef.current;
    if (!ancla || !caja) return;
    const a = ancla.getBoundingClientRect();
    const c = caja.getBoundingClientRect();
    const ancho = c.width || caja.offsetWidth || 0;
    const alto = c.height || caja.offsetHeight || 0;

    // Vertical: primero arriba del botón; si no cabe, debajo; si tampoco,
    // pegado al borde (nunca fuera de la vista).
    let arriba = a.top - distancia - alto;
    if (arriba < margen) arriba = a.bottom + distancia;
    arriba = Math.max(margen, Math.min(arriba, window.innerHeight - margen - alto));

    // Horizontal: alineado al botón, arrimado a los bordes si se sale.
    let izquierda = a.left;
    izquierda = Math.max(margen, Math.min(izquierda, window.innerWidth - margen - ancho));

    setEstilo({
      left: Math.round(izquierda) + 'px',
      top: Math.round(arriba) + 'px',
    });
  }, [margen, distancia]);

  // Se mide antes de pintar: el menú nunca aparece en el sitio equivocado.
  useLayoutEffect(() => {
    if (!abierto) { setEstilo(null); return undefined; }
    colocar();
    const alMoverse = () => colocar();
    window.addEventListener('resize', alMoverse);
    window.addEventListener('scroll', alMoverse, true);
    return () => {
      window.removeEventListener('resize', alMoverse);
      window.removeEventListener('scroll', alMoverse, true);
    };
  }, [abierto, colocar]);

  const menu = (contenido) => (abierto
    ? createPortal(
      <span
        ref={cajaRef}
        className="reacciones-menu"
        role="menu"
        aria-label="Reacciones"
        // Hasta tener la medida, queda listo pero invisible: dura un parpadeo.
        style={estilo ? estilo : { visibility: 'hidden', left: 0, top: 0 }}
      >
        {contenido}
      </span>,
      document.body,
    )
    : null);

  return { anclaRef, menu };
}

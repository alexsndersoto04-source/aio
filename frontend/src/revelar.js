// Moon — Entrada en escena de los elementos
// ============================================================
// Lo que entra en el campo de vista aparece con un gesto corto, en vez de
// estar ya ahí cuando uno baja. Se observa una sola vez por elemento y se
// apaga por completo si el sistema pide menos movimiento.

import { useEffect, useRef } from 'react';

export function useRevelar() {
  const ref = useRef(null);

  useEffect(() => {
    const nodo = ref.current;
    if (!nodo) return undefined;

    if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
      nodo.classList.add('aparece-visible');
      return undefined;
    }
    // Si ya está dentro del campo de vista, no hay nada que animar.
    const arriba = nodo.getBoundingClientRect().top;
    if (arriba < window.innerHeight * 0.9) {
      nodo.classList.add('aparece-visible');
      return undefined;
    }

    const obs = new IntersectionObserver(
      (entradas) => {
        for (const e of entradas) {
          if (e.isIntersecting) {
            e.target.classList.add('aparece-visible');
            obs.unobserve(e.target);
          }
        }
      },
      { rootMargin: '0px 0px -6% 0px', threshold: 0.04 },
    );
    obs.observe(nodo);
    return () => obs.disconnect();
  }, []);

  return ref;
}

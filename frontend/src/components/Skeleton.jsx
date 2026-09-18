// Moon — Estados de carga
// ============================================================
// Giros finos y discretos, con una frase corta que explica qué se está
// cargando. Sin bloques grises: la pantalla nunca enseña una maqueta del
// contenido, solo dice que está trabajando.

import React from 'react';

function Cargando({ etiqueta, alto }) {
  return (
    <div className="cargando" role="status" aria-live="polite" style={alto ? { minHeight: alto } : undefined}>
      <span className="giro" aria-hidden="true" />
      <span>{etiqueta}</span>
    </div>
  );
}

export function PostSkeleton({ etiqueta = 'Cargando publicaciones…', alto }) {
  return <Cargando etiqueta={etiqueta} alto={alto} />;
}

export function ListSkeleton({ etiqueta = 'Cargando…', alto }) {
  return <Cargando etiqueta={etiqueta} alto={alto} />;
}

export function ProfileSkeleton({ etiqueta = 'Cargando perfil…', alto }) {
  return <Cargando etiqueta={etiqueta} alto={alto} />;
}

export default Cargando;

// Moon — Esqueletos de carga
// ============================================================
// Mientras llegan los datos se dibuja la forma de lo que va a aparecer,
// en vez de un girador o un salto de contenido. Se siente mucho más
// rápido y evita el «parpadeo» clásico.

import React from 'react';

export function SkeletonLine({ width = '100%', height = 12, style }) {
  return <div className="skeleton skeleton-line" style={{ width, height, ...style }} />;
}

export function PostSkeleton({ lines = 3 }) {
  return (
    <div className="skeleton-post" aria-hidden="true">
      <div className="skeleton" style={{ width: 44, height: 44, borderRadius: '50%', flex: 'none' }} />
      <div className="lines">
        <SkeletonLine width="38%" />
        <SkeletonLine width="92%" />
        {lines > 2 ? <SkeletonLine width="76%" /> : null}
        {lines > 3 ? <SkeletonLine width="54%" /> : null}
        <div className="row" style={{ gap: 18, marginTop: 6 }}>
          <SkeletonLine width={54} height={10} />
          <SkeletonLine width={54} height={10} />
          <SkeletonLine width={54} height={10} />
        </div>
      </div>
    </div>
  );
}

export function ListSkeleton({ rows = 6 }) {
  return (
    <div aria-hidden="true">
      {Array.from({ length: rows }).map((_, i) => (
        <div className="skeleton-post" key={i}>
          <div className="skeleton" style={{ width: 44, height: 44, borderRadius: '50%', flex: 'none' }} />
          <div className="lines">
            <SkeletonLine width="30%" />
            <SkeletonLine width="84%" />
          </div>
        </div>
      ))}
    </div>
  );
}

export function ProfileSkeleton() {
  return (
    <div aria-hidden="true">
      <div className="skeleton" style={{ height: 190, borderRadius: 0 }} />
      <div style={{ padding: '0 20px 20px' }}>
        <div className="skeleton" style={{ width: 108, height: 108, borderRadius: '50%', marginTop: -54, border: '4px solid var(--surface)' }} />
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10, marginTop: 14 }}>
          <SkeletonLine width="34%" height={16} />
          <SkeletonLine width="22%" />
          <SkeletonLine width="64%" />
        </div>
      </div>
      <PostSkeleton />
      <PostSkeleton lines={2} />
    </div>
  );
}

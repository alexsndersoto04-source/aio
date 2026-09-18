// Moon — Reacciones
// ============================================================
// Una reacción por persona y publicación (la anterior se reemplaza).
// El resumen se pega a cada publicación al listarla, con el mismo formato
// para el feed, el perfil y la vista de una sola publicación.
// ============================================================

export const TIPOS = ['me_gusta', 'me_encanta', 'risa', 'sorpresa', 'triste', 'enojo'];

export const EMOJI = {
  me_gusta: '👍',
  me_encanta: '❤️',
  risa: '😂',
  sorpresa: '😮',
  triste: '😢',
  enojo: '😠',
};

export function tipoValido(t) {
  return TIPOS.includes(String(t || ''));
}

/** Resumen de reacciones de varias publicaciones (una sola consulta). */
export async function conReacciones(pool, posts, yoId) {
  if (!posts || posts.length === 0) return posts;
  const ids = posts.map((p) => Number(p.id));
  const conteos = (await pool.query(
    'SELECT post_id, tipo, COUNT(*)::int AS n FROM likes WHERE post_id = ANY($1::bigint[]) GROUP BY post_id, tipo',
    [ids]
  )).rows;
  const mias = yoId
    ? (await pool.query('SELECT post_id, tipo FROM likes WHERE post_id = ANY($1::bigint[]) AND user_id = $2', [ids, yoId])).rows
    : [];

  for (const p of posts) {
    const id = Number(p.id);
    const dePost = conteos.filter((c) => Number(c.post_id) === id);
    const total = dePost.reduce((n, c) => n + Number(c.n), 0);
    const conteo = {};
    for (const c of dePost) conteo[c.tipo] = Number(c.n);
    const mi = mias.find((m) => Number(m.post_id) === id);
    // Se ordenan de mayor a menor para pintar las tres primeras y «+N».
    const orden = Object.entries(conteo)
      .sort((a, b) => b[1] - a[1])
      .map(([tipo, n]) => ({ tipo, emoji: EMOJI[tipo] || '👍', n }));
    p.reacciones = { total, mi: mi ? mi.tipo : null, conteo, orden };
  }
  return posts;
}

/** Resumen de reacciones de varios comentarios. */
export async function conReaccionesComentarios(pool, comentarios, yoId) {
  if (!comentarios || comentarios.length === 0) return comentarios;
  const ids = comentarios.map((x) => Number(x.id));
  const conteos = (await pool.query(
    'SELECT comment_id, tipo, COUNT(*)::int AS n FROM comment_likes WHERE comment_id = ANY($1::bigint[]) GROUP BY comment_id, tipo',
    [ids]
  )).rows;
  const mias = yoId
    ? (await pool.query('SELECT comment_id, tipo FROM comment_likes WHERE comment_id = ANY($1::bigint[]) AND user_id = $2', [ids, yoId])).rows
    : [];
  for (const x of comentarios) {
    const id = Number(x.id);
    const deEste = conteos.filter((c) => Number(c.comment_id) === id);
    const total = deEste.reduce((n, c) => n + Number(c.n), 0);
    const conteo = {};
    for (const c of deEste) conteo[c.tipo] = Number(c.n);
    const mi = mias.find((m) => Number(m.comment_id) === id);
    const orden = Object.entries(conteo).sort((a, b) => b[1] - a[1]).map(([tipo, n]) => ({ tipo, emoji: EMOJI[tipo] || '👍', n }));
    x.reacciones = { total, mi: mi ? mi.tipo : null, conteo, orden };
  }
  return comentarios;
}

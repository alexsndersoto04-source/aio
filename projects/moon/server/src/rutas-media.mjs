// Moon — Imágenes
// ============================================================
// Subida (multipart) y entrega de archivos. Los bytes viven en la base de
// datos (`media_blobs`), con una copia en disco como caché: así una foto no
// se pierde cuando el servidor se reinicia.
//
// Al subir avatar o portada se actualiza el perfil en el momento.

import Busboy from 'busboy';
import { ApiErr } from './util.mjs';
import { uno, auditar } from './db.mjs';
import { guardarImagen, leerImagen, servirDeDisco, asegurarCarpeta } from './medios.mjs';

const TIPOS = new Set(['image/jpeg', 'image/png', 'image/webp', 'image/gif', 'image/avif']);
const MAX_BYTES = 8 * 1024 * 1024;

/** Lee el formulario completo en memoria (con tope de tamaño). */
function leerFormulario(req) {
  return new Promise((resolver, rechazar) => {
    const bb = Busboy({ headers: req.headers, limits: { files: 1, fileSize: MAX_BYTES } });
    let clase = 'post';
    let trozos = [];
    let mime = '';
    let nombreOriginal = '';
    let truncado = false;
    let fallo = null;

    bb.on('field', (nombre, valor) => {
      if (nombre === 'name' && ['avatar', 'cover', 'post', 'story'].includes(valor)) clase = valor;
    });

    bb.on('file', (_campo, stream, info) => {
      mime = String(info.mimeType || '').toLowerCase();
      nombreOriginal = info.filename || '';
      stream.on('data', (t) => trozos.push(t));
      stream.on('limit', () => { truncado = true; });
      stream.on('error', (e) => { fallo = e; });
    });

    bb.on('error', (e) => { fallo = e; });
    bb.on('close', () => {
      if (fallo) return rechazar(new ApiErr(`No se pudo leer el archivo: ${fallo.message}`, 400));
      if (truncado) return rechazar(new ApiErr('La imagen supera los 8 MB', 413));
      const bytes = Buffer.concat(trozos);
      if (bytes.length === 0) return rechazar(new ApiErr('No se recibió ninguna imagen', 400));
      if (mime && !TIPOS.has(mime)) return rechazar(new ApiErr('Formato de imagen no admitido (JPEG, PNG, WebP, GIF o AVIF)', 400));
      return resolver({ clase, bytes, mime: mime || 'image/jpeg', nombreOriginal });
    });

    req.pipe(bb);
  });
}

export function registrarRutasMedia(router) {
  router.post('/api/upload', async (c) => {
    const yo = await c.exigir();
    const tipo = String(c.req.headers['content-type'] || '');
    if (!tipo.startsWith('multipart/form-data')) throw new ApiErr('Se esperaba un formulario con el archivo', 400);

    asegurarCarpeta();
    const { clase, bytes, mime } = await leerFormulario(c.req);
    const guardada = await guardarImagen(c.pool, { userId: yo.id, clase, bytes, mime });

    if (clase === 'avatar') {
      await c.pool.query('UPDATE users SET avatar_url = $1 WHERE id = $2', [guardada.url, yo.id]);
    } else if (clase === 'cover') {
      await c.pool.query('UPDATE users SET cover_url = $1 WHERE id = $2', [guardada.url, yo.id]);
    }
    await c.pool.query(
      'UPDATE users SET images_count = images_count + 1, images_bytes = images_bytes + $1 WHERE id = $2',
      [guardada.bytes, yo.id]
    );
    await auditar(c.pool, Number(yo.id), 'imagen_subida', clase, c.ip);

    return guardada;
  });

  // Entrega de la imagen: primero la caché en disco (rápida) y si no está,
  // la base de datos (permanente).
  router.get('/api/media/:archivo', async (c) => {
    const nombre = String(c.params.archivo || '');
    if (!/^[A-Za-z0-9._-]+$/.test(nombre)) throw new ApiErr('Nombre inválido', 400);

    if (servirDeDisco(c.res, nombre)) return undefined;

    const fila = await leerImagen(c.pool, nombre);
    if (!fila) throw new ApiErr('Archivo no encontrado', 404);

    const bytes = fila.bytes;
    c.res.writeHead(200, {
      'Content-Type': fila.mime || 'image/jpeg',
      'Content-Length': bytes.length,
      'Cache-Control': 'public, max-age=31536000, immutable',
      'X-Content-Type-Options': 'nosniff',
    });
    c.res.end(bytes);
    return undefined;
  });
}

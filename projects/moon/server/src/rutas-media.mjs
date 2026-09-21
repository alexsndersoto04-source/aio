// Moon — Archivos (imágenes y notas de voz)
// ============================================================
// Subida (multipart) y entrega. Los bytes viven en la base de datos
// (`media_blobs`), con una copia en disco como caché: así una foto —o una
// nota de voz— no se pierde cuando el servidor se reinicia.
//
// Las imágenes se giran y se reducen al subirlas; las notas de voz se guardan
// tal cual. La entrega admite peticiones por trozos (Range), que es lo que
// usan los reproductores del teléfono.
//
// Al subir avatar o portada se actualiza el perfil en el momento.

import Busboy from 'busboy';
import { ApiErr } from './util.mjs';
import { uno, auditar } from './db.mjs';
import {
  guardarImagen, leerImagen, servirDeDisco, asegurarCarpeta,
  esAudio, esVideo, mimeDeNombre, rangoDe,
} from './medios.mjs';

const TIPOS = new Set(['image/jpeg', 'image/png', 'image/webp', 'image/gif', 'image/avif']);
const MAX_BYTES_VIDEO = 120 * 1024 * 1024; // Hasta 120 MB para videos
const MAX_BYTES_IMAGEN = 15 * 1024 * 1024; // Hasta 15 MB para fotos
const aceptado = (mime) => TIPOS.has(mime) || esAudio(mime) || esVideo(mime);

/** Lee el formulario completo en memoria (con tope de tamaño). */
function leerFormulario(req) {
  return new Promise((resolver, rechazar) => {
    const bb = Busboy({ headers: req.headers, limits: { files: 1, fileSize: MAX_BYTES_VIDEO } });
    let clase = 'post';
    let trozos = [];
    let mime = '';
    let nombreOriginal = '';
    let truncado = false;
    let fallo = null;

    bb.on('field', (nombre, valor) => {
      if (nombre === 'name' && ['avatar', 'cover', 'post', 'story', 'audio', 'video'].includes(valor)) clase = valor;
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
      if (truncado) return rechazar(new ApiErr('El archivo supera los 120 MB permitidos', 413));
      const bytes = Buffer.concat(trozos);
      if (bytes.length === 0) return rechazar(new ApiErr('No se recibió ningún archivo', 400));
      const limpio = String(mime || '').split(';')[0].trim();
      if (limpio && !aceptado(limpio)) {
        return rechazar(new ApiErr('Formato no admitido: imágenes (JPEG, PNG, WebP, GIF), videos (MP4, WebM, MOV) o notas de voz', 400));
      }
      if (!esVideo(limpio) && !esAudio(limpio) && bytes.length > MAX_BYTES_IMAGEN) {
        return rechazar(new ApiErr('La imagen supera los 15 MB permitidos', 413));
      }
      if (esVideo(limpio)) clase = 'video';
      return resolver({ clase, bytes, mime: limpio || (esVideo(limpio) ? 'video/mp4' : 'image/jpeg'), nombreOriginal });
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

    const cabeceraRango = String(c.req.headers.range || '');
    if (servirDeDisco(c.res, nombre, cabeceraRango)) return undefined;

    const fila = await leerImagen(c.pool, nombre);
    if (!fila) throw new ApiErr('Archivo no encontrado', 404);

    const bytes = fila.bytes;
    const mime = fila.mime || mimeDeNombre(nombre);
    const comun = {
      'Content-Type': mime,
      'Cache-Control': 'public, max-age=31536000, immutable',
      'Accept-Ranges': 'bytes',
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Headers': 'Range, Content-Type',
      'Access-Control-Expose-Headers': 'Content-Range, Accept-Ranges, Content-Length',
      'X-Content-Type-Options': 'nosniff',
    };

    // Los reproductores del teléfono piden trozos (Range) para poder avanzar
    // dentro de una nota de voz. Sin esto, en iPhone no suena.
    const r = rangoDe(cabeceraRango, bytes.length);
    if (r && r.invalido) {
      c.res.writeHead(416, { ...comun, 'Content-Range': `bytes */${bytes.length}` });
      c.res.end();
      return undefined;
    }
    if (r) {
      const trozo = bytes.subarray(r.inicio, r.fin + 1);
      c.res.writeHead(206, {
        ...comun,
        'Content-Length': trozo.length,
        'Content-Range': `bytes ${r.inicio}-${r.fin}/${bytes.length}`,
      });
      c.res.end(trozo);
      return undefined;
    }
    c.res.writeHead(200, { ...comun, 'Content-Length': bytes.length });
    c.res.end(bytes);
    return undefined;
  });
}

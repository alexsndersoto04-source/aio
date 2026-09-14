// Moon — Imágenes
// ============================================================
// Subida (multipart) y entrega de archivos. Las imágenes viven EN la base de
// datos (columna BYTEA de la tabla `media`), no en el disco del contenedor:
// en los planes gratuitos el disco es temporal y cada redeploy borraba todas
// las fotos subidas. La ficha de cada imagen sigue en `media`, y al subir
// avatar o portada se actualiza el perfil.
//
// Compatibilidad: las fichas antiguas (subidas cuando se guardaba en disco)
// se siguen sirviendo desde el archivo si todavía existe; si no, 404 honesto.

import Busboy from 'busboy';
import { existsSync, statSync, createReadStream } from 'node:fs';
import { randomBytes } from 'node:crypto';
import { resolve, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { ApiErr } from './util.mjs';
import { uno } from './db.mjs';
import { auditar } from './db.mjs';

const aqui = dirname(fileURLToPath(import.meta.url));
// Solo para servir fichas antiguas que aún tengan archivo en disco.
const CARPETA = process.env.MOON_UPLOADS || resolve(aqui, '../../uploads');

const TIPOS = {
  'image/jpeg': '.jpg',
  'image/png': '.png',
  'image/webp': '.webp',
  'image/gif': '.gif',
  'image/avif': '.avif',
};
const MAX_BYTES = 8 * 1024 * 1024;

function extensionDe(nombre, mime) {
  if (TIPOS[mime]) return TIPOS[mime];
  const ext = extname(nombre || '').toLowerCase();
  return ['.jpg', '.jpeg', '.png', '.webp', '.gif', '.avif'].includes(ext) ? ext : '.jpg';
}

function mimeDe(nombre, guardado) {
  if (guardado) return guardado;
  const ext = extname(nombre).toLowerCase();
  return (
    ext === '.png' ? 'image/png'
    : ext === '.webp' ? 'image/webp'
    : ext === '.gif' ? 'image/gif'
    : ext === '.avif' ? 'image/avif'
    : 'image/jpeg'
  );
}

export function registrarRutasMedia(router) {
  router.post('/api/upload', async (c) => {
    const yo = await c.exigir();

    const tipo = String(c.req.headers['content-type'] || '');
    if (!tipo.startsWith('multipart/form-data')) throw new ApiErr('Se esperaba un formulario con el archivo', 400);

    const bb = Busboy({ headers: c.req.headers, limits: { files: 1, fileSize: MAX_BYTES } });
    let clase = 'post';
    let archivo = null;
    let error = null;

    const terminado = new Promise((resolver) => {
      bb.on('field', (nombre, valor) => {
        if (nombre === 'name' && ['avatar', 'cover', 'post', 'story'].includes(valor)) clase = valor;
      });

      bb.on('file', (_campo, stream, info) => {
        const ext = extensionDe(info.filename, info.mimeType);
        if (!TIPOS[info.mimeType] && !info.filename) {
          error = new ApiErr('Formato de imagen no admitido', 400);
          stream.resume();
          return;
        }
        const nombreFichero = `${Date.now()}-${randomBytes(8).toString('hex')}${ext}`;
        const trozos = [];
        let bytes = 0;
        let truncado = false;
        stream.on('data', (t) => {
          bytes += t.length;
          if (bytes > MAX_BYTES) {
            truncado = true;
            stream.resume();
            return;
          }
          trozos.push(t);
        });
        stream.on('limit', () => { truncado = true; });
        stream.on('end', () => {
          if (truncado) {
            error = new ApiErr('La imagen supera los 8 MB', 413);
          } else if (bytes > 0) {
            archivo = {
              nombre: nombreFichero,
              datos: Buffer.concat(trozos),
              bytes,
              mime: TIPOS[info.mimeType] ? info.mimeType : 'image/jpeg',
            };
          }
        });
        stream.on('error', () => { error = new ApiErr('No se pudo leer la imagen', 400); });
        stream.resume();
      });
      bb.on('error', (e) => { error = new ApiErr(`No se pudo leer el archivo: ${e.message}`, 400); });
      bb.on('close', () => resolver());
    });

    c.req.pipe(bb);
    await terminado;

    if (error) throw error;
    if (!archivo) throw new ApiErr('No se recibió ninguna imagen', 400);

    const url = `/api/media/${archivo.nombre}`;
    const media = await uno(
      c.pool,
      `INSERT INTO media (user_id, kind, original_path, thumb_path, url, bytes, original_data, mime)
       VALUES ($1, $2, $3, $3, $4, $5, $6, $7) RETURNING id`,
      [yo.id, clase, archivo.nombre, url, archivo.bytes, archivo.datos, archivo.mime]
    );

    if (clase === 'avatar') {
      await c.pool.query('UPDATE users SET avatar_url = $1 WHERE id = $2', [url, yo.id]);
    } else if (clase === 'cover') {
      await c.pool.query('UPDATE users SET cover_url = $1 WHERE id = $2', [url, yo.id]);
    }
    await c.pool.query('UPDATE users SET images_count = images_count + 1, images_bytes = images_bytes + $1 WHERE id = $2', [archivo.bytes, yo.id]);
    await auditar(c.pool, Number(yo.id), 'imagen_subida', clase, c.ip);

    return { id: Number(media.id), url, bytes: archivo.bytes, kind: clase };
  });

  // Entrega de la imagen: desde la base de datos; si la ficha es anterior al
  // almacenamiento en BD y el archivo sigue en disco, se sirve desde ahí.
  router.get('/api/media/:archivo', async (c) => {
    const nombre = String(c.params.archivo || '');
    if (!/^[A-Za-z0-9._-]+$/.test(nombre)) throw new ApiErr('Nombre inválido', 400);

    const ficha = await uno(
      c.pool,
      'SELECT original_data, mime, original_path FROM media WHERE url = $1',
      [`/api/media/${nombre}`]
    );
    if (!ficha) throw new ApiErr('Archivo no encontrado', 404);

    const cabecerasBase = {
      'Cache-Control': 'public, max-age=31536000, immutable',
      'X-Content-Type-Options': 'nosniff',
    };

    if (ficha.original_data) {
      const buf = Buffer.isBuffer(ficha.original_data)
        ? ficha.original_data
        : Buffer.from(ficha.original_data);
      c.res.writeHead(200, {
        ...cabecerasBase,
        'Content-Type': mimeDe(nombre, ficha.mime),
        'Content-Length': String(buf.length),
      });
      c.res.end(c.req.method === 'HEAD' ? undefined : buf);
      return;
    }

    // Ficha antigua: intentar el disco (solo mientras el archivo exista).
    const candidato = ficha.original_path && existsSync(ficha.original_path)
      ? ficha.original_path
      : resolve(CARPETA, nombre);
    const seguro = candidato.startsWith(CARPETA) && existsSync(candidato) && statSync(candidato).isFile();
    if (!seguro) throw new ApiErr('Archivo no encontrado', 404);
    c.res.writeHead(200, {
      ...cabecerasBase,
      'Content-Type': mimeDe(nombre, ficha.mime),
      'Content-Length': String(statSync(candidato).size),
    });
    if (c.req.method === 'HEAD') { c.res.end(); return; }
    const lectura = createReadStream(candidato);
    lectura.on('error', (e) => {
      console.error('[api] no se pudo leer la imagen:', e.message);
      if (!c.res.writableEnded) c.res.end();
    });
    c.res.on('close', () => lectura.destroy());
    lectura.pipe(c.res);
  });
}

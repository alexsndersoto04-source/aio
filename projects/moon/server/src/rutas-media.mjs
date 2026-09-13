// Moon — Imágenes
// ============================================================
// Subida (multipart) y entrega de archivos. Los archivos viven en disco y su
// ficha en la tabla `media`. Al subir avatar o portada se actualiza el perfil.

import Busboy from 'busboy';
import { createWriteStream, mkdirSync, existsSync, statSync, createReadStream } from 'node:fs';
import { randomBytes } from 'node:crypto';
import { resolve, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { ApiErr } from './util.mjs';
import { uno } from './db.mjs';
import { auditar } from './db.mjs';

const aqui = dirname(fileURLToPath(import.meta.url));
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

export function registrarRutasMedia(router) {
  router.post('/api/upload', async (c) => {
    const yo = await c.exigir();
    if (!existsSync(CARPETA)) mkdirSync(CARPETA, { recursive: true });

    const tipo = String(c.req.headers['content-type'] || '');
    if (!tipo.startsWith('multipart/form-data')) throw new ApiErr('Se esperaba un formulario con el archivo', 400);

    const bb = Busboy({ headers: c.req.headers, limits: { files: 1, fileSize: MAX_BYTES } });
    let clase = 'post';
    let archivo = null;
    let error = null;
    // La escritura en disco puede terminar después de que Busboy avise del
    // final del formulario: hay que esperar las dos cosas, no solo una.
    let escritura = null;

    bb.on('field', (nombre, valor) => {
      if (nombre === 'name' && ['avatar', 'cover', 'post', 'story'].includes(valor)) clase = valor;
    });

    const terminado = new Promise((resolver) => {
      bb.on('file', (_campo, stream, info) => {
        const ext = extensionDe(info.filename, info.mimeType);
        if (!TIPOS[info.mimeType] && !info.filename) {
          error = new ApiErr('Formato de imagen no admitido', 400);
          stream.resume();
          return;
        }
        const nombreFichero = `${Date.now()}-${randomBytes(8).toString('hex')}${ext}`;
        const destino = resolve(CARPETA, nombreFichero);
        let bytes = 0;
        let truncado = false;
        const salida = createWriteStream(destino);
        stream.on('data', (t) => { bytes += t.length; });
        stream.on('limit', () => { truncado = true; });
        escritura = new Promise((listo) => {
          salida.on('close', () => {
            if (truncado) {
              error = new ApiErr('La imagen supera los 8 MB', 413);
            } else if (bytes > 0) {
              archivo = { nombre: nombreFichero, ruta: destino, bytes, mime: info.mimeType };
            }
            listo();
          });
          salida.on('error', () => {
            error = new ApiErr('No se pudo guardar la imagen', 500);
            listo();
          });
        });
        stream.pipe(salida);
      });
      bb.on('error', (e) => { error = new ApiErr(`No se pudo leer el archivo: ${e.message}`, 400); });
      bb.on('close', () => resolver());
    });

    c.req.pipe(bb);
    await terminado;
    if (escritura) await escritura;

    if (error) throw error;
    if (!archivo) throw new ApiErr('No se recibió ninguna imagen', 400);

    const url = `/api/media/${archivo.nombre}`;
    const media = await uno(
      c.pool,
      `INSERT INTO media (user_id, kind, original_path, thumb_path, url, bytes)
       VALUES ($1, $2, $3, $3, $4, $5) RETURNING id`,
      [yo.id, clase, archivo.ruta, url, archivo.bytes]
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

  // Entrega del archivo. Se comprueba que nadie intente salir de la carpeta.
  router.get('/api/media/:archivo', async (c) => {
    const nombre = String(c.params.archivo || '');
    if (!/^[A-Za-z0-9._-]+$/.test(nombre)) throw new ApiErr('Nombre inválido', 400);
    const ruta = resolve(CARPETA, nombre);
    if (!ruta.startsWith(CARPETA) || !existsSync(ruta) || !statSync(ruta).isFile()) {
      throw new ApiErr('Archivo no encontrado', 404);
    }
    const ext = extname(nombre).toLowerCase();
    const mime =
      ext === '.png' ? 'image/png'
      : ext === '.webp' ? 'image/webp'
      : ext === '.gif' ? 'image/gif'
      : ext === '.avif' ? 'image/avif'
      : 'image/jpeg';
    c.res.writeHead(200, {
      'Content-Type': mime,
      'Content-Length': statSync(ruta).size,
      'Cache-Control': 'public, max-age=31536000, immutable',
      'X-Content-Type-Options': 'nosniff',
    });
    const lectura = createReadStream(ruta);
    lectura.on('error', (e) => {
      console.error('[api] no se pudo leer la imagen:', e.message);
      if (!c.res.writableEnded) c.res.end();
    });
    c.res.on('close', () => lectura.destroy());
    lectura.pipe(c.res);
  });
}

// Moon — Almacén de imágenes (dentro de la base de datos)
// ============================================================
// El problema que resuelve: en el plan gratuito el disco del servidor es
// temporal (se borra al reiniciar), así que las fotos se perdían. Ahora los
// bytes de cada imagen viven en la tabla `media_blobs`, dentro de PostgreSQL
// (Supabase), que sí es permanente.
//
// Al guardar:
//   · se gira según el EXIF (las fotos del teléfono salen derechas),
//   · se reduce a 1600 px y se recomprime (una foto de 5 MB baja a ~250 KB),
//   · se guarda una copia en disco como caché rápida (si el disco se borra,
//     la base de datos sigue teniendo el original).
//
// La entrega (`GET /api/media/<nombre>`) busca primero en disco y, si no está,
// lo saca de la base de datos. Por eso la misma dirección sigue funcionando
// aunque el servidor se haya reiniciado.

import sharp from 'sharp';
import { randomBytes } from 'node:crypto';
import { createWriteStream, mkdirSync, existsSync, statSync, createReadStream, readdirSync } from 'node:fs';
import { extname, resolve } from 'node:path';
import { uno } from './db.mjs';

export const CARPETA = process.env.MOON_UPLOADS || resolve(process.cwd(), 'uploads');

const LADO_MAXIMO = 1600;
const CALIDAD = 82;

export function asegurarCarpeta() {
  try {
    if (!existsSync(CARPETA)) mkdirSync(CARPETA, { recursive: true });
  } catch (e) {
    console.error('[medios] no se pudo crear la carpeta de caché:', e.message);
  }
}

function nombrePara(ext) {
  return `${Date.now()}-${randomBytes(8).toString('hex')}${ext}`;
}

/**
 * Optimiza la imagen. Devuelve `{ bytes, mime, ext, ancho, alto }`.
 *
 * Reglas (pensadas para no estropear nada de lo que sube la gente):
 *   · Las animaciones (GIF o WebP animado) se guardan tal cual: recortarlas
 *     perdería el movimiento.
 *   · Las imágenes con transparencia (PNG con canal alfa) se quedan en PNG,
 *     porque pasar a JPEG las volvería negras por detrás.
 *   · WebP se queda en WebP; lo demás (fotos de teléfono) pasa a JPEG.
 *   · Todo se gira según el EXIF y se reduce a 1600 px: una foto de 5 MB
 *     baja a unos 250 KB, que es lo que permite que la base gratuita aguante.
 * Si algo falla, se guarda el original: nunca se pierde una subida.
 */
export async function optimizar(original, mime) {
  const LADO = { width: LADO_MAXIMO, height: LADO_MAXIMO, fit: 'inside', withoutEnlargement: true };

  if (mime === 'image/gif') {
    return { bytes: original, mime, ext: '.gif', ancho: 0, alto: 0 };
  }

  try {
    const meta = await sharp(original, { failOn: 'none', animated: true }).metadata();
    if ((meta.pages || 1) > 1) {
      // Animación (WebP o GIF): intacta.
      const ext = mime === 'image/webp' ? '.webp' : mime === 'image/gif' ? '.gif' : '.png';
      return { bytes: original, mime, ext, ancho: 0, alto: 0 };
    }

    const tubo = sharp(original, { failOn: 'none' }).rotate().resize(LADO);
    const jpeg = () => tubo.clone().jpeg({ quality: CALIDAD, progressive: true, mozjpeg: true }).toBuffer({ resolveWithObject: true });
    const enJpeg = (salida) => ({
      bytes: salida.data, mime: 'image/jpeg', ext: '.jpg',
      ancho: salida.info.width || 0, alto: salida.info.height || 0,
    });

    if (mime === 'image/png') {
      const png = await tubo.clone().png({ compressionLevel: 9, palette: true }).toBuffer({ resolveWithObject: true });
      // Con transparencia, PNG es obligatorio (si no, el fondo sale negro).
      if (meta.hasAlpha) {
        return { bytes: png.data, mime: 'image/png', ext: '.png', ancho: png.info.width || 0, alto: png.info.height || 0 };
      }
      // Sin transparencia (capturas, dibujos): se queda el más liviano de los
      // dos. Un plano de color pesa menos en PNG; una foto, menos en JPEG.
      const jpg = await jpeg();
      if (png.data.length <= jpg.data.length) {
        return { bytes: png.data, mime: 'image/png', ext: '.png', ancho: png.info.width || 0, alto: png.info.height || 0 };
      }
      return enJpeg(jpg);
    }
    if (mime === 'image/webp') {
      const salida = await tubo.clone().webp({ quality: 80 }).toBuffer({ resolveWithObject: true });
      return { bytes: salida.data, mime: 'image/webp', ext: '.webp', ancho: salida.info.width || 0, alto: salida.info.height || 0 };
    }
    return enJpeg(await jpeg());
  } catch (e) {
    console.error('[medios] no se pudo optimizar, se guarda el original:', e.message);
    const ext = mime === 'image/png' ? '.png' : mime === 'image/webp' ? '.webp' : mime === 'image/avif' ? '.avif' : '.jpg';
    return { bytes: original, mime, ext, ancho: 0, alto: 0 };
  }
}

/** Escribe la copia de caché en disco (si se puede; si no, no pasa nada). */
function copiaEnDisco(nombre, bytes) {
  try {
    asegurarCarpeta();
    const salida = createWriteStream(resolve(CARPETA, nombre));
    salida.on('error', () => {});
    salida.end(bytes);
  } catch { /* sin caché: la base de datos es la que manda */ }
}

/**
 * Guarda una imagen: optimiza, registra la ficha y mete los bytes en la base
 * de datos. Devuelve la fila creada con su dirección pública.
 */
export async function guardarImagen(pool, { userId, clase, bytes, mime }) {
  const listo = await optimizar(bytes, mime);
  const nombre = nombrePara(listo.ext);
  const url = `/api/media/${nombre}`;

  const media = await uno(
    pool,
    `INSERT INTO media (user_id, kind, original_path, thumb_path, url, bytes)
     VALUES ($1, $2, $3, $3, $4, $5) RETURNING id`,
    [userId, clase, nombre, url, listo.bytes.length]
  );

  try {
    await pool.query(
      `INSERT INTO media_blobs (media_id, mime, bytes, ancho, alto) VALUES ($1, $2, $3, $4, $5)`,
      [Number(media.id), listo.mime, listo.bytes, listo.ancho, listo.alto]
    );
  } catch (e) {
    // Si la base de datos no acepta los bytes (por ejemplo, se llenó el
    // espacio), al menos queda la copia en disco: mejor eso que nada.
    console.error('[medios] no se pudieron guardar los bytes en la base:', e.message);
  }

  copiaEnDisco(nombre, listo.bytes);

  return { id: Number(media.id), url, bytes: listo.bytes.length, kind: clase, mime: listo.mime, ancho: listo.ancho, alto: listo.alto };
}

/** Devuelve la imagen guardada en la base de datos, o `null`. */
export async function leerImagen(pool, nombre) {
  const url = `/api/media/${nombre}`;
  const fila = await uno(
    pool,
    `SELECT b.mime, b.bytes FROM media m JOIN media_blobs b ON b.media_id = m.id WHERE m.url = $1`,
    [url]
  );
  if (fila) return fila;

  // Compatibilidad: las fotos que se subieron con la versión anterior quedaron
  // en la columna `original_data` de la tabla `media`. Se siguen sirviendo
  // exactamente igual, sin pedirle a nadie que vuelva a subirlas.
  try {
    const vieja = await uno(
      pool,
      `SELECT COALESCE(NULLIF(mime, ''), 'image/jpeg') AS mime, original_data AS bytes
         FROM media WHERE url = $1 AND original_data IS NOT NULL`,
      [url]
    );
    if (vieja && vieja.bytes) return { mime: vieja.mime, bytes: vieja.bytes };
  } catch { /* la columna no existe todavía: nada que recuperar */ }

  return null;
}

/** Sirve un archivo de la caché en disco. Devuelve `false` si no existe. */
export function servirDeDisco(res, nombre) {
  if (!/^[A-Za-z0-9._-]+$/.test(nombre)) return false;
  const ruta = resolve(CARPETA, nombre);
  if (!ruta.startsWith(CARPETA)) return false;
  try {
    if (!existsSync(ruta) || !statSync(ruta).isFile()) return false;
  } catch {
    return false;
  }
  const ext = extname(nombre).toLowerCase();
  const mime =
    ext === '.png' ? 'image/png'
    : ext === '.webp' ? 'image/webp'
    : ext === '.gif' ? 'image/gif'
    : ext === '.avif' ? 'image/avif'
    : 'image/jpeg';
  res.writeHead(200, {
    'Content-Type': mime,
    'Content-Length': statSync(ruta).size,
    'Cache-Control': 'public, max-age=31536000, immutable',
    'X-Content-Type-Options': 'nosniff',
  });
  const lectura = createReadStream(ruta);
  lectura.on('error', () => { if (!res.writableEnded) res.end(); });
  res.on('close', () => lectura.destroy());
  lectura.pipe(res);
  return true;
}

/**
 * Al arrancar: mete en la base de datos las imágenes antiguas que quedaran en
 * el disco (de antes de este cambio). Se hace poco a poco y sin bloquear.
 */
export async function importarDelDisco(pool, limite = 400) {
  try {
    asegurarCarpeta();
    const archivos = readdirSync(CARPETA).filter((n) => /\.(jpg|jpeg|png|webp|gif|avif)$/i.test(n));
    if (archivos.length === 0) return 0;
    let importadas = 0;
    for (const nombre of archivos.slice(0, limite)) {
      const url = `/api/media/${nombre}`;
      const ya = await uno(
        pool,
        `SELECT 1 FROM media m JOIN media_blobs b ON b.media_id = m.id WHERE m.url = $1`,
        [url]
      );
      if (ya) continue;
      const ruta = resolve(CARPETA, nombre);
      const bytes = statSync(ruta).size;
      if (bytes <= 0 || bytes > 12 * 1024 * 1024) continue;
      const { readFileSync } = await import('node:fs');
      const datos = readFileSync(ruta);
      const ext = extname(nombre).toLowerCase();
      const mime = ext === '.png' ? 'image/png' : ext === '.webp' ? 'image/webp' : ext === '.gif' ? 'image/gif' : ext === '.avif' ? 'image/avif' : 'image/jpeg';
      const ficha = await uno(pool, 'SELECT id FROM media WHERE url = $1', [url]);
      let mediaId = ficha ? Number(ficha.id) : null;
      if (!mediaId) {
        const creada = await uno(
          pool,
          `INSERT INTO media (user_id, kind, original_path, thumb_path, url, bytes)
           VALUES ((SELECT id FROM users ORDER BY id LIMIT 1), 'post', $1, $1, $2, $3) RETURNING id`,
          [nombre, url, bytes]
        );
        mediaId = Number(creada.id);
      }
      await pool.query(
        `INSERT INTO media_blobs (media_id, mime, bytes) VALUES ($1, $2, $3) ON CONFLICT (media_id) DO NOTHING`,
        [mediaId, mime, datos]
      );
      importadas += 1;
    }
    if (importadas > 0) console.log(`[medios] ${importadas} imágenes antiguas guardadas en la base de datos`);
    return importadas;
  } catch (e) {
    console.error('[medios] no se pudieron importar las imágenes del disco:', e.message);
    return 0;
  }
}

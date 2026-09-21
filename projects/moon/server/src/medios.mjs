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
import { subirATelegram, descargarDeTelegram } from './almacen-telegram.mjs';

export const CARPETA = process.env.MOON_UPLOADS || resolve(process.cwd(), 'uploads');

const LADO_MAXIMO = 1600;
const CALIDAD = 82;

// Formatos de video admitidos para publicaciones de Moon
export const TIPOS_VIDEO = new Set([
  'video/mp4', 'video/webm', 'video/quicktime', 'video/ogg', 'video/3gpp', 'video/x-matroska',
]);

export const EXT_DE_VIDEO = {
  'video/mp4': '.mp4',
  'video/webm': '.webm',
  'video/quicktime': '.mov',
  'video/ogg': '.ogv',
  'video/3gpp': '.3gp',
  'video/x-matroska': '.mkv',
};

// Notas de voz: se guardan tal cual (comprimirlas otra vez estropearía el
// audio). Estos son los formatos que graban los teléfonos.
export const TIPOS_AUDIO = new Set([
  'audio/webm', 'audio/ogg', 'audio/mp4', 'audio/mpeg', 'audio/wav', 'audio/x-wav',
  'audio/aac', 'audio/m4a', 'audio/x-m4a', 'audio/3gpp',
]);

const EXT_DE_MIME = {
  'audio/webm': '.webm',
  'audio/ogg': '.ogg',
  'audio/mp4': '.m4a', 'audio/m4a': '.m4a', 'audio/x-m4a': '.m4a',
  'audio/mpeg': '.mp3',
  'audio/wav': '.wav', 'audio/x-wav': '.wav',
  'audio/aac': '.aac', 'audio/3gpp': '.3gp',
};

const MIME_DE_EXT = {
  '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.png': 'image/png', '.webp': 'image/webp',
  '.gif': 'image/gif', '.avif': 'image/avif',
  '.mp4': 'video/mp4', '.mov': 'video/quicktime', '.webm': 'video/webm',
  '.ogv': 'video/ogg', '.3gp': 'video/3gpp', '.mkv': 'video/x-matroska',
  '.ogg': 'audio/ogg', '.oga': 'audio/ogg', '.m4a': 'audio/mp4',
  '.mp3': 'audio/mpeg', '.wav': 'audio/wav', '.aac': 'audio/aac',
};

/** ¿Es un video? */
export function esVideo(mime) {
  const m = String(mime || '').toLowerCase().split(';')[0].trim();
  return TIPOS_VIDEO.has(m) || m.startsWith('video/');
}

/** ¿Es una nota de voz (o audio)? */
export function esAudio(mime) {
  const m = String(mime || '').toLowerCase().split(';')[0].trim();
  return TIPOS_AUDIO.has(m) || m.startsWith('audio/');
}

/** Tipo de archivo a partir de su nombre. */
export function mimeDeNombre(nombre) {
  const ext = extname(String(nombre || '')).toLowerCase();
  return MIME_DE_EXT[ext] || 'application/octet-stream';
}

/**
 * Bytes como parámetro bytea que aceptan los dos drivers: el TCP clásico y el
 * HTTP de Neon (que no entiende objetos Buffer y los rompería en JSON).
 */
function paramBytes(bytes) {
  return `\\x${Buffer.from(bytes).toString('hex')}`;
}

/**
 * Lo contrario: normaliza lo que devuelve la base (Buffer por TCP, o texto
 * «\x…» en hex por HTTP) a un Buffer de verdad para poder servirlo.
 */
export function aBuffer(v) {
  if (Buffer.isBuffer(v)) return v;
  if (v instanceof Uint8Array) return Buffer.from(v);
  const s = String(v || '');
  if (s.startsWith('\\x')) return Buffer.from(s.slice(2), 'hex');
  return Buffer.from(s, 'base64');
}

/**
 * Lee la cabecera `Range` de una petición (la usan los reproductores de audio
 * del teléfono para poder avanzar dentro de la nota de voz).
 */
export function rangoDe(cabecera, total) {
  const m = /^bytes=(\d*)-(\d*)$/.exec(String(cabecera || '').trim());
  if (!m) return null;
  let inicio = m[1] === '' ? null : Number(m[1]);
  let fin = m[2] === '' ? null : Number(m[2]);
  if (inicio === null && fin === null) return null;
  if (inicio === null) { inicio = Math.max(0, total - fin); fin = total - 1; }
  if (fin === null || fin >= total) fin = total - 1;
  if (inicio > fin || inicio >= total) return { invalido: true };
  return { inicio, fin };
}

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
  const esVid = esVideo(mime);
  const esAud = esAudio(mime);

  let listo;
  if (esAud) {
    const limpio = String(mime).split(';')[0].trim();
    listo = { bytes, mime: limpio, ext: EXT_DE_MIME[limpio] || '.webm', ancho: 0, alto: 0 };
  } else if (esVid) {
    const limpio = String(mime).split(';')[0].trim();
    listo = { bytes, mime: limpio, ext: EXT_DE_VIDEO[limpio] || '.mp4', ancho: 0, alto: 0 };
  } else {
    listo = await optimizar(bytes, mime);
  }

  const nombre = nombrePara(listo.ext);
  const url = `/api/media/${nombre}`;
  const tipoKind = esVid ? 'video' : (esAud ? 'audio' : clase);

  const media = await uno(
    pool,
    `INSERT INTO media (user_id, kind, original_path, thumb_path, url, bytes)
     VALUES ($1, $2, $3, $3, $4, $5) RETURNING id`,
    [userId, tipoKind, nombre, url, listo.bytes.length]
  );

  // Si el archivo pesa hasta 30 MB (o es imagen/audio), se guarda en Postgres
  // para que esté 100% permanente y no se pierda al reiniciar Render.
  const guardarEnBlobs = !esVid || (listo.bytes.length <= 30 * 1024 * 1024);
  if (guardarEnBlobs) {
    try {
      await pool.query(
        `INSERT INTO media_blobs (media_id, mime, bytes, ancho, alto) VALUES ($1, $2, $3, $4, $5)`,
        [Number(media.id), listo.mime, paramBytes(listo.bytes), listo.ancho, listo.alto]
      );
    } catch (e) {
      // Si la base de datos no acepta los bytes (por ejemplo, se llenó el
      // espacio), al menos queda la copia en disco: mejor eso que nada.
      console.error('[medios] no se pudieron guardar los bytes en la base:', e.message);
    }
  }

  copiaEnDisco(nombre, listo.bytes);

  // Enviar a la bodega del canal privado en Telegram (de fondo y protegido contra cualquier fallo)
  try {
    subirATelegram(listo.bytes, { nombre, tipo: listo.mime })
      .then(async (resTg) => {
        if (resTg && resTg.tg_id) {
          await pool.query('UPDATE media SET thumb_path = $1 WHERE id = $2', [`tg:${resTg.tg_id}`, Number(media.id)]).catch(() => {});
        }
      })
      .catch((err) => {
        console.error('[medios] error enviando a telegram:', err?.message || err);
      });
  } catch (errTg) {
    console.error('[medios] error iniciando subida a telegram:', errTg?.message || errTg);
  }

  return { id: Number(media.id), url, bytes: listo.bytes.length, kind: tipoKind, mime: listo.mime, ancho: listo.ancho, alto: listo.alto };
}

/** Devuelve la imagen guardada en la base de datos o recuperada de Telegram, o `null`. */
export async function leerImagen(pool, nombre) {
  const url = `/api/media/${nombre}`;
  const fila = await uno(
    pool,
    `SELECT b.mime, b.bytes FROM media m JOIN media_blobs b ON b.media_id = m.id WHERE m.url = $1`,
    [url]
  );
  if (fila) return { mime: fila.mime, bytes: aBuffer(fila.bytes) };

  // Si no está en media_blobs (por ejemplo videos que van directo a Telegram):
  try {
    const med = await uno(
      pool,
      `SELECT id, kind, thumb_path, original_path FROM media WHERE url = $1`,
      [url]
    );
    if (med && med.thumb_path && String(med.thumb_path).startsWith('tg:')) {
      const tgId = String(med.thumb_path).slice(3);
      const bajado = await descargarDeTelegram(tgId, { tipo: med.kind || 'video' });
      if (bajado) {
        copiaEnDisco(nombre, bajado);
        const mime = mimeDeNombre(nombre);
        return { mime, bytes: bajado };
      }
    }
  } catch (errTg) {
    console.error('[medios] error recuperando de telegram:', errTg?.message || errTg);
  }

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
    if (vieja && vieja.bytes) return { mime: vieja.mime, bytes: aBuffer(vieja.bytes) };
  } catch { /* la columna no existe todavía: nada que recuperar */ }

  return null;
}

/** Sirve un archivo de la caché en disco. Devuelve `false` si no existe. */
export function servirDeDisco(res, nombre, cabeceraRango = '') {
  if (!/^[A-Za-z0-9._-]+$/.test(nombre)) return false;
  const ruta = resolve(CARPETA, nombre);
  if (!ruta.startsWith(CARPETA)) return false;
  let tamano = 0;
  try {
    if (!existsSync(ruta) || !statSync(ruta).isFile()) return false;
    tamano = statSync(ruta).size;
  } catch {
    return false;
  }
  const mime = mimeDeNombre(nombre);
  const comun = {
    'Content-Type': mime,
    'Cache-Control': 'public, max-age=31536000, immutable',
    'Accept-Ranges': 'bytes',
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Headers': 'Range, Content-Type',
    'Access-Control-Expose-Headers': 'Content-Range, Accept-Ranges, Content-Length',
    'X-Content-Type-Options': 'nosniff',
  };

  const r = rangoDe(cabeceraRango, tamano);
  if (r && r.invalido) {
    res.writeHead(416, { ...comun, 'Content-Range': `bytes */${tamano}` });
    res.end();
    return true;
  }
  const desde = r ? r.inicio : 0;
  const hasta = r ? r.fin : tamano - 1;
  res.writeHead(r ? 206 : 200, {
    ...comun,
    'Content-Length': hasta - desde + 1,
    ...(r ? { 'Content-Range': `bytes ${desde}-${hasta}/${tamano}` } : {}),
  });
  const lectura = createReadStream(ruta, { start: desde, end: hasta });
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
    const archivos = readdirSync(CARPETA).filter((n) => /\.(jpg|jpeg|png|webp|gif|avif|webm|ogg|m4a|mp3|wav|aac|3gp)$/i.test(n));
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
      const mime = mimeDeNombre(nombre);
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
        [mediaId, mime, paramBytes(datos)]
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

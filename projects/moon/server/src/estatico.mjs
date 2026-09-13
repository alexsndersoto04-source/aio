// Moon — Servidor de archivos estáticos
// ============================================================
// Entrega la aplicación ya compilada (frontend/dist) desde el mismo puerto
// que la API. Así la red social y su servidor viven en una sola dirección:
// un único enlace que abre todo, sin depender de dos servicios distintos.

import { createReadStream, statSync, existsSync } from 'node:fs';
import { join, resolve, extname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const AQUI = fileURLToPath(new URL('.', import.meta.url));
// src → server → moon → projects → raíz del repositorio
const RAIZ_REPO = resolve(AQUI, '..', '..', '..', '..');

export const CARPETA_WEB = process.env.MOON_WEB_DIR
  ? resolve(process.env.MOON_WEB_DIR)
  : join(RAIZ_REPO, 'frontend', 'dist');

const TIPOS = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.gif': 'image/gif',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.txt': 'text/plain; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
  '.webmanifest': 'application/manifest+json',
};

function tipoDe(ruta) {
  return TIPOS[extname(ruta).toLowerCase()] || 'application/octet-stream';
}

function esArchivo(ruta) {
  try {
    return statSync(ruta).isFile();
  } catch {
    return false;
  }
}

/** Traduce una ruta de URL a una ruta de disco segura dentro de la carpeta web. */
function rutaEnDisco(camino) {
  const limpio = camino.replace(/^\/+/, '');
  const destino = resolve(CARPETA_WEB, limpio);
  if (destino !== CARPETA_WEB && !destino.startsWith(CARPETA_WEB + sep)) return null;
  return destino;
}

/**
 * ¿Debe servirse el index? Sí para la raíz, para cualquier ruta sin extensión
 * (páginas del tipo #/… o /messages/3) y cuando el archivo pedido no existe.
 */
function decidir(camino) {
  if (camino === '/' || camino === '') return { archivo: join(CARPETA_WEB, 'index.html') };
  const destino = rutaEnDisco(camino);
  if (destino && esArchivo(destino)) return { archivo: destino };
  const tieneExtension = extname(camino) !== '';
  if (!tieneExtension) return { archivo: join(CARPETA_WEB, 'index.html') };
  return null;
}

/**
 * Intenta responder con un archivo de la aplicación web.
 * Devuelve true si ya respondió; false si la petición debe seguir a la API.
 */
export function servirWeb(req, res, camino) {
  if (camino.startsWith('/api/') || camino === '/ws') return false;
  if (req.method !== 'GET' && req.method !== 'HEAD') return false;
  if (!existsSync(CARPETA_WEB)) return false;

  const elegido = decidir(camino);
  if (!elegido) return false;

  const { archivo } = elegido;
  const esIndex = archivo.endsWith(`${sep}index.html`);
  const cabeceras = {
    'Content-Type': tipoDe(archivo),
    'Cache-Control': esIndex
      ? 'no-cache'
      : 'public, max-age=31536000, immutable',
  };

  if (esIndex) cabeceras['Content-Type'] = TIPOS['.html'];

  try {
    const info = statSync(archivo);
    cabeceras['Content-Length'] = String(info.size);
    res.writeHead(200, cabeceras);
    if (req.method === 'HEAD') {
      res.end();
      return true;
    }
    createReadStream(archivo).pipe(res);
    return true;
  } catch {
    return false;
  }
}

/** Estado de la carpeta web, para mostrarlo al arrancar. */
export function estadoWeb() {
  if (!existsSync(CARPETA_WEB)) return `sin compilar (${CARPETA_WEB})`;
  const index = join(CARPETA_WEB, 'index.html');
  if (!esArchivo(index)) return `incompleta (${CARPETA_WEB})`;
  return `lista en / (${CARPETA_WEB})`;
}

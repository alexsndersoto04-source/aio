// Moon — Generador de íconos PWA y tarjeta para compartir
// =========================================================
// Dibuja los PNG de la aplicación (luna creciente sobre el degradado de la
// casa) sin dependencias: el PNG se escribe a mano con `zlib`. Uso:
//   node frontend/scripts/generar-iconos.mjs
// y escribe en frontend/public/: icono-192.png, icono-512.png,
// icono-maskable-512.png y og.png (tarjeta de 1200×630 para WhatsApp y demás).

import { deflateSync } from 'node:zlib';
import { writeFileSync, mkdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const aqui = dirname(fileURLToPath(import.meta.url));
const publico = resolve(aqui, '../public');
mkdirSync(publico, { recursive: true });

// ---------- CRC32 y PNG ----------
const TABLA_CRC = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();

function crc32(buf) {
  let c = -1;
  for (let i = 0; i < buf.length; i++) c = TABLA_CRC[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
}

function chunk(tipo, datos) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(datos.length);
  const cuerpo = Buffer.concat([Buffer.from(tipo, 'ascii'), datos]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(cuerpo));
  return Buffer.concat([len, cuerpo, crc]);
}

function png(ancho, alto, pixel) {
  const crudo = Buffer.alloc(alto * (ancho * 4 + 1));
  let p = 0;
  for (let y = 0; y < alto; y++) {
    crudo[p++] = 0; // filtro «none» por línea
    for (let x = 0; x < ancho; x++) {
      const [r, g, b, a] = pixel(x, y);
      crudo[p++] = r; crudo[p++] = g; crudo[p++] = b; crudo[p++] = a;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(ancho, 0);
  ihdr.writeUInt32BE(alto, 4);
  ihdr[8] = 8;  // profundidad
  ihdr[9] = 6;  // RGBA
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(crudo, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

// ---------- Paleta de la casa ----------
const A = [99, 102, 241];   // índigo
const B = [168, 85, 247];   // violeta
const C = [34, 211, 238];   // cian

function degradado(t) {
  // t en [0,1]: índigo → violeta → cian
  const mezclar = (p, q, u) => p.map((v, i) => Math.round(v + (q[i] - v) * u));
  if (t < 0.52) return mezclar(A, B, t / 0.52);
  return mezclar(B, C, (t - 0.52) / 0.48);
}

/** Ícono: fondo degradado diagonal + luna creciente blanca. */
function icono(tam, margenLuna = 0.30) {
  return (x, y) => {
    const t = (x + y) / (2 * tam);
    const [r, g, b] = degradado(t);
    // luna: círculo blanco menos círculo desplazado (creciente)
    const cx = tam / 2, cy = tam / 2;
    const d1 = Math.hypot(x - cx, y - cy);
    const d2 = Math.hypot(x - (cx + tam * 0.16), y - (cy - tam * 0.13));
    if (d1 < tam * margenLuna && d2 > tam * margenLuna * 0.92) return [255, 255, 255, 255];
    return [r, g, b, 255];
  };
}

/** Tarjeta og: degradado + luna grande a la izquierda. */
function og() {
  const W = 1200, H = 630;
  return (x, y) => {
    const t = (x / W) * 0.7 + (y / H) * 0.3;
    const [r, g, b] = degradado(t);
    const cx = W * 0.24, cy = H * 0.5;
    const d1 = Math.hypot(x - cx, y - cy);
    const d2 = Math.hypot(x - (cx + 62), y - (cy - 50));
    if (d1 < 150 && d2 > 138) return [255, 255, 255, 255];
    return [r, g, b, 255];
  };
}

writeFileSync(`${publico}/icono-192.png`, png(192, 192, icono(192)));
writeFileSync(`${publico}/icono-512.png`, png(512, 512, icono(512)));
// maskable: luna más pequeña para respetar la zona segura del 80 %
writeFileSync(`${publico}/icono-maskable-512.png`, png(512, 512, icono(512, 0.24)));
writeFileSync(`${publico}/og.png`, png(1200, 630, og()));

console.log('Íconos escritos en frontend/public/: 192, 512, maskable-512 y og.png');

// Sirve la web compilada y hace de puente hacia la API (igual que Cloudflare).
// Uso: node servidor.cjs <carpeta-dist> <puerto>
const http = require('http');
const https = require('https');
const fs = require('fs');
const path = require('path');

const RAIZ = path.resolve(process.argv[2] || 'frontend/dist');
const PUERTO = Number(process.argv[3] || 4173);
const API = 'https://moon-dal0.onrender.com';

const TIPOS = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2',
};

function puente(req, res) {
  const cabeceras = Object.assign({}, req.headers, { host: 'moon-dal0.onrender.com' });
  const salida = https.request(API + req.url, { method: req.method, headers: cabeceras }, (r) => {
    res.writeHead(r.statusCode || 502, r.headers);
    r.pipe(res);
  });
  salida.on('error', (e) => { res.writeHead(502, { 'Content-Type': 'text/plain' }); res.end('puente: ' + e.message); });
  req.pipe(salida);
}

http.createServer((req, res) => {
  if (req.url.startsWith('/api')) return puente(req, res);
  if (req.url.startsWith('/ws')) { res.writeHead(404); return res.end(); }
  let ruta = decodeURIComponent(req.url.split('?')[0]);
  if (ruta === '/') ruta = '/index.html';
  let archivo = path.join(RAIZ, ruta);
  if (!fs.existsSync(archivo) || fs.statSync(archivo).isDirectory()) archivo = path.join(RAIZ, 'index.html');
  let cuerpo = fs.readFileSync(archivo);
  if (archivo.endsWith('index.html')) {
    const datos = fs.existsSync(path.join(RAIZ, 'diag-datos.js'))
      ? fs.readFileSync(path.join(RAIZ, 'diag-datos.js'), 'utf8') : '';
    cuerpo = cuerpo.toString().replace('</body>', `<script>${datos}</script>\n<script src="/diag.js"></script>\n</body>`);
  }
  res.writeHead(200, {
    'Content-Type': TIPOS[path.extname(archivo)] || 'application/octet-stream',
    'Cache-Control': 'no-store',
  });
  res.end(cuerpo);
}).listen(PUERTO, '127.0.0.1', () => console.log('servidor listo en http://127.0.0.1:' + PUERTO));

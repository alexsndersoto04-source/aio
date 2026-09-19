// Sirve la web compilada y hace de puente hacia la API (igual que Cloudflare).
// Uso: node servidor.cjs <carpeta-dist> <puerto> [direccion-de-la-api]
const http = require('http');
const https = require('https');
const fs = require('fs');
const path = require('path');

const RAIZ = path.resolve(process.argv[2] || 'frontend/dist');
const PUERTO = Number(process.argv[3] || 4173);
const API = process.argv[4] || 'https://moon-dal0.onrender.com';

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
  '.webmanifest': 'application/manifest+json',
};

// Todo lo que empieza por /api viaja a la API de verdad (mismo camino que en
// el sitio en vivo): la web compilada no sabe que está en una prueba.
function puente(req, res) {
  const cabeceras = Object.assign({}, req.headers, { host: 'moon-dal0.onrender.com' });
  const salida = https.request(API + req.url, { method: req.method, headers: cabeceras }, (r) => {
    res.writeHead(r.statusCode || 502, r.headers);
    r.pipe(res);
  });
  salida.on('error', (e) => {
    res.writeHead(502, { 'Content-Type': 'text/plain' });
    res.end('puente: ' + e.message);
  });
  req.pipe(salida);
}

// El tubo del tiempo real (las llamadas y el «escribiendo…» viajan por aquí):
// se pasa tal cual hacia la API, como hace Cloudflare en el sitio en vivo.
const servidor = http.createServer((req, res) => {
  if (req.url.startsWith('/api')) return puente(req, res);
  if (req.url.startsWith('/ws')) { res.writeHead(404); return res.end(); }
  let ruta = decodeURIComponent(req.url.split('?')[0]);
  if (ruta === '/') ruta = '/index.html';
  let archivo = path.join(RAIZ, ruta);
  if (!fs.existsSync(archivo) || fs.statSync(archivo).isDirectory()) archivo = path.join(RAIZ, 'index.html');
  res.writeHead(200, {
    'Content-Type': TIPOS[path.extname(archivo)] || 'application/octet-stream',
    'Cache-Control': 'no-store',
  });
  res.end(fs.readFileSync(archivo));
});

servidor.on('upgrade', (req, socket, cabeza) => {
  if (!req.url.startsWith('/ws')) { socket.destroy(); return; }
  const peticion = https.request({
    host: 'moon-dal0.onrender.com',
    port: 443,
    path: req.url,
    method: 'GET',
    headers: Object.assign({}, req.headers, { host: 'moon-dal0.onrender.com' }),
  });
  peticion.on('upgrade', (respuesta, socketRemoto) => {
    const cabeceras = Object.entries(respuesta.headers)
      .map(([k, v]) => `${k}: ${v}`)
      .join('\r\n');
    socket.write('HTTP/1.1 101 Switching Protocols\r\n' + cabeceras + '\r\n\r\n');
    socketRemoto.pipe(socket);
    socket.pipe(socketRemoto);
    socket.on('error', () => socketRemoto.destroy());
    socketRemoto.on('error', () => socket.destroy());
  });
  peticion.on('error', () => socket.destroy());
  peticion.end();
  void cabeza;
});

servidor.listen(PUERTO, '127.0.0.1', () => console.log('servidor listo (web + api + tubo) en http://127.0.0.1:' + PUERTO));

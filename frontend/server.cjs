// Moon — Servidor estático de producción (SPA + assets con caché)
const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = process.env.PORT || 3000;
const DIST = path.join(__dirname, 'dist');

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js':   'application/javascript; charset=utf-8',
  '.css':  'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg':  'image/svg+xml',
  '.png':  'image/png',
  '.ico':  'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
};

function send(res, status, headers, body) {
  res.writeHead(status, headers);
  res.end(body);
}

const server = http.createServer((req, res) => {
  // Solo GET/HEAD (REST de la API va directo a moon-api, no aquí)
  if (req.method !== 'GET' && req.method !== 'HEAD') {
    send(res, 405, { 'Content-Type': 'text/plain; charset=utf-8', Allow: 'GET, HEAD' }, 'Método no permitido');
    return;
  }

  let urlPath = req.url.split('?')[0];
  let filePath = path.join(DIST, urlPath);

  // Path traversal protection
  const normalized = path.normalize(filePath);
  if (!normalized.startsWith(DIST)) {
    send(res, 403, { 'Content-Type': 'text/plain; charset=utf-8' }, 'Forbidden');
    return;
  }

  fs.stat(normalized, (err, stats) => {
    if (!err && stats.isFile()) {
      const ext = path.extname(normalized);
      const mime = MIME[ext] || 'application/octet-stream';
      const headers = {
        'Content-Type': mime,
        'X-Content-Type-Options': 'nosniff',
      };
      // Assets con hash (dist/assets/*) son inmutables; el resto no.
      if (normalized.includes(`${path.sep}assets${path.sep}`)) {
        headers['Cache-Control'] = 'public, max-age=31536000, immutable';
      } else {
        headers['Cache-Control'] = 'no-cache';
      }
      res.writeHead(200, headers);
      if (req.method === 'HEAD') { res.end(); return; }
      fs.createReadStream(normalized).pipe(res);
      return;
    }

    // SPA fallback para rutas del router (hash router: solo raíz).
    const index = path.join(DIST, 'index.html');
    fs.readFile(index, (e, data) => {
      if (e) {
        send(res, 404, { 'Content-Type': 'text/plain; charset=utf-8' }, 'Not found');
      } else {
        send(res, 200, {
          'Content-Type': 'text/html; charset=utf-8',
          'Cache-Control': 'no-cache',
          'X-Content-Type-Options': 'nosniff',
        }, req.method === 'HEAD' ? '' : data);
      }
    });
  });
});

server.listen(PORT, '0.0.0.0', () => {
  console.log(`Moon frontend en http://0.0.0.0:${PORT}`);
});

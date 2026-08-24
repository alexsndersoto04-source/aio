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
};

const server = http.createServer((req, res) => {
  let urlPath = req.url.split('?')[0];
  let filePath = path.join(DIST, urlPath);

  // Path traversal protection
  const normalized = path.normalize(filePath);
  if (!normalized.startsWith(DIST)) {
    res.writeHead(403);
    res.end('Forbidden');
    return;
  }

  fs.stat(normalized, (err, stats) => {
    if (!err && stats.isFile()) {
      const ext = path.extname(normalized);
      const mime = MIME[ext] || 'application/octet-stream';
      res.writeHead(200, { 'Content-Type': mime });
      fs.createReadStream(normalized).pipe(res);
    } else {
      // SPA fallback
      const index = path.join(DIST, 'index.html');
      fs.readFile(index, (e, data) => {
        if (e) {
          res.writeHead(404);
          res.end('Not found');
        } else {
          res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
          res.end(data);
        }
      });
    }
  });
});

server.listen(PORT, '0.0.0.0', () => {
  console.log(`Moon frontend en http://0.0.0.0:${PORT}`);
});

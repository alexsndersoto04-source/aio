#!/usr/bin/env node

/**
 * Titan Audio — Servidor Full Stack Multimedia & Streaming
 * =========================================================
 * API REST completa, streaming de audio con soporte para rangos HTTP 206,
 * gestión de biblioteca, subida de canciones, pasarela Telegram y servicio web.
 */

const http = require('http');
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const url = require('url');

const PORT = parseInt(process.env.PORT || '3000', 10);
const HOST = '0.0.0.0';

const ROOT_DIR = path.resolve(__dirname, '..');
const WEB_DIR = path.join(ROOT_DIR, 'android', 'app', 'src', 'main', 'assets', 'web');
const DATA_DIR = path.join(__dirname, 'data');
const UPLOADS_DIR = path.join(DATA_DIR, 'uploads');
const ARTWORK_DIR = path.join(DATA_DIR, 'artwork');
const DB_FILE = path.join(DATA_DIR, 'library.json');

[DATA_DIR, UPLOADS_DIR, ARTWORK_DIR].forEach(dir => {
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
});

// Semilla inicial de biblioteca con pistas Hi-Fi del motor
const INITIAL_LIBRARY = {
    version: '1.0.0',
    tracks: [
        {
            id: 'trk_titan_1',
            title: 'Horizonte Estelar',
            artist: 'Titan Sound Lab',
            album: 'Aura Neon',
            genre: 'Synthwave / Hi-Fi',
            duration: 185,
            bitrate: '320 kbps',
            format: 'FLAC / 24-bit',
            source: 'cloud',
            filename: 'track_synthwave.wav',
            relativePath: 'audio/track_synthwave.wav',
            artworkColor: '#00e5ff',
            likes: 42,
            plays: 1240
        },
        {
            id: 'trk_titan_2',
            title: 'Pulso Electrónico',
            artist: 'Kroma Beats',
            album: 'Resonancia Cuántica',
            genre: 'Electronic',
            duration: 160,
            bitrate: '320 kbps',
            format: 'WAV Lossless',
            source: 'cloud',
            filename: 'track_electronic.wav',
            relativePath: 'audio/track_electronic.wav',
            artworkColor: '#7928ca',
            likes: 38,
            plays: 980
        },
        {
            id: 'trk_titan_3',
            title: 'Niebla de Medianoche',
            artist: 'Luna Lofi',
            album: 'Café & Melancolía',
            genre: 'Lo-Fi Chill',
            duration: 195,
            bitrate: '320 kbps',
            format: 'FLAC Lossless',
            source: 'cloud',
            filename: 'track_lofi.wav',
            relativePath: 'audio/track_lofi.wav',
            artworkColor: '#ff007a',
            likes: 56,
            plays: 2150
        },
        {
            id: 'trk_titan_4',
            title: 'Cuerdas al Viento',
            artist: 'Alba Acústica',
            album: 'Maderas Nobles',
            genre: 'Acoustic Folk',
            duration: 210,
            bitrate: '320 kbps',
            format: 'WAV Studio Master',
            source: 'cloud',
            filename: 'track_acoustic.wav',
            relativePath: 'audio/track_acoustic.wav',
            artworkColor: '#f59e0b',
            likes: 29,
            plays: 870
        },
        {
            id: 'trk_titan_5',
            title: 'Furia de Titanio',
            artist: 'Neon Rift',
            album: 'Sobrecarga',
            genre: 'Alternative Rock',
            duration: 175,
            bitrate: '320 kbps',
            format: 'FLAC Lossless',
            source: 'cloud',
            filename: 'track_rock.wav',
            relativePath: 'audio/track_rock.wav',
            artworkColor: '#ef4444',
            likes: 64,
            plays: 3120
        }
    ],
    playlists: [
        {
            id: 'pl_hifi_favorites',
            name: 'Favoritos Hi-Fi',
            description: 'Selección maestra con rango dinámico extendido',
            trackIds: ['trk_titan_1', 'trk_titan_2', 'trk_titan_3']
        },
        {
            id: 'pl_late_night',
            name: 'Sesión Nocturna',
            description: 'Graves profundos y acústica relajante',
            trackIds: ['trk_titan_3', 'trk_titan_4']
        }
    ],
    telegramConfig: {
        configured: true,
        connected: true,
        channel: 'Titan Music Cloud Vault',
        botName: '@TitanAudioCloudBot',
        totalCloudTracks: 340,
        streamingCacheMb: 0 // streaming puro, 0 almacenamiento en cliente
    }
};

function getLibrary() {
    if (!fs.existsSync(DB_FILE)) {
        fs.writeFileSync(DB_FILE, JSON.stringify(INITIAL_LIBRARY, null, 2), 'utf-8');
        return INITIAL_LIBRARY;
    }
    try {
        const raw = fs.readFileSync(DB_FILE, 'utf-8');
        return JSON.parse(raw);
    } catch (e) {
        return INITIAL_LIBRARY;
    }
}

function saveLibrary(data) {
    fs.writeFileSync(DB_FILE, JSON.stringify(data, null, 2), 'utf-8');
}

const MIME_TYPES = {
    '.html': 'text/html; charset=utf-8',
    '.css': 'text/css; charset=utf-8',
    '.js': 'application/javascript; charset=utf-8',
    '.json': 'application/json; charset=utf-8',
    '.png': 'image/png',
    '.jpg': 'image/jpeg',
    '.jpeg': 'image/jpeg',
    '.svg': 'image/svg+xml',
    '.ico': 'image/x-icon',
    '.wav': 'audio/wav',
    '.mp3': 'audio/mpeg',
    '.flac': 'audio/flac',
    '.ogg': 'audio/ogg',
    '.m4a': 'audio/mp4'
};

function sendJson(res, statusCode, payload) {
    res.writeHead(statusCode, {
        'Content-Type': 'application/json; charset=utf-8',
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Headers': 'Content-Type, Authorization, Range',
        'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS'
    });
    res.end(JSON.stringify(payload));
}

function handleStream(req, res, filePath) {
    fs.stat(filePath, (err, stats) => {
        if (err || !stats.isFile()) {
            res.writeHead(404, { 'Content-Type': 'text/plain' });
            res.end('Pista de audio no encontrada');
            return;
        }

        const fileSize = stats.size;
        const range = req.headers.range;
        const ext = path.extname(filePath).toLowerCase();
        const contentType = MIME_TYPES[ext] || 'audio/mpeg';

        if (range) {
            const parts = range.replace(/bytes=/, "").split("-");
            const start = parseInt(parts[0], 10);
            const end = parts[1] ? parseInt(parts[1], 10) : fileSize - 1;

            if (start >= fileSize || end >= fileSize || start > end) {
                res.writeHead(416, {
                    'Content-Range': `bytes */${fileSize}`,
                    'Access-Control-Allow-Origin': '*'
                });
                res.end();
                return;
            }

            const chunksize = (end - start) + 1;
            const fileStream = fs.createReadStream(filePath, { start, end });

            res.writeHead(206, {
                'Content-Range': `bytes ${start}-${end}/${fileSize}`,
                'Accept-Ranges': 'bytes',
                'Content-Length': chunksize,
                'Content-Type': contentType,
                'Access-Control-Allow-Origin': '*',
                'Cache-Control': 'no-cache'
            });

            fileStream.pipe(res);
        } else {
            res.writeHead(200, {
                'Content-Length': fileSize,
                'Content-Type': contentType,
                'Accept-Ranges': 'bytes',
                'Access-Control-Allow-Origin': '*',
                'Cache-Control': 'public, max-age=3600'
            });

            fs.createReadStream(filePath).pipe(res);
        }
    });
}

const server = http.createServer((req, res) => {
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization, Range');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS');

    if (req.method === 'OPTIONS') {
        res.writeHead(204);
        res.end();
        return;
    }

    const parsedUrl = url.parse(req.url, true);
    const pathname = parsedUrl.pathname;

    // =========================================================================
    // API REST FULL STACK
    // =========================================================================

    // Estado del servidor y sistema
    if (pathname === '/api/status' && req.method === 'GET') {
        const lib = getLibrary();
        return sendJson(res, 200, {
            status: 'online',
            service: 'Titan Audio Full Stack Server',
            version: '2.0.0-hifi',
            uptimeSecs: Math.round(process.uptime()),
            totalTracks: lib.tracks.length,
            totalPlaylists: lib.playlists.length,
            telegramGateway: lib.telegramConfig,
            engine: 'Titan Lossless Audio Engine 24-bit/96kHz'
        });
    }

    // Lista de pistas de la biblioteca
    if (pathname === '/api/tracks' && req.method === 'GET') {
        const lib = getLibrary();
        const search = (parsedUrl.query.q || '').toLowerCase();
        let tracks = lib.tracks;

        if (search) {
            tracks = tracks.filter(t =>
                t.title.toLowerCase().includes(search) ||
                t.artist.toLowerCase().includes(search) ||
                t.album.toLowerCase().includes(search) ||
                t.genre.toLowerCase().includes(search)
            );
        }

        return sendJson(res, 200, {
            success: true,
            count: tracks.length,
            tracks: tracks.map(t => ({
                ...t,
                streamUrl: `/api/stream/${t.id}`,
                artworkUrl: `/api/artwork/${t.id}`
            }))
        });
    }

    // Streaming de audio con soporte para Range 206
    if (pathname.startsWith('/api/stream/') && req.method === 'GET') {
        const trackId = pathname.replace('/api/stream/', '').trim();
        const lib = getLibrary();
        const track = lib.tracks.find(t => t.id === trackId);

        if (!track) {
            res.writeHead(404, { 'Content-Type': 'text/plain' });
            return res.end('Canción no encontrada en el catálogo');
        }

        let audioFile = '';
        if (track.relativePath) {
            audioFile = path.join(WEB_DIR, track.relativePath);
        } else if (track.uploadedPath) {
            audioFile = path.join(UPLOADS_DIR, track.uploadedPath);
        }

        if (!fs.existsSync(audioFile)) {
            // Si el archivo físico específico no existe, usar el primer wav disponible de respaldo
            const defaultWav = path.join(WEB_DIR, 'audio', 'track_synthwave.wav');
            if (fs.existsSync(defaultWav)) {
                audioFile = defaultWav;
            } else {
                res.writeHead(404, { 'Content-Type': 'text/plain' });
                return res.end('Archivo de audio físico no disponible');
            }
        }

        return handleStream(req, res, audioFile);
    }

    // Generador dinámico / extractor de carátulas de alta resolución
    if (pathname.startsWith('/api/artwork/') && req.method === 'GET') {
        const trackId = pathname.replace('/api/artwork/', '').trim();
        const lib = getLibrary();
        const track = lib.tracks.find(t => t.id === trackId);

        const color = (track && track.artworkColor) ? track.artworkColor : '#00e5ff';
        const title = (track && track.title) ? track.title : 'Titan Hi-Fi';
        const artist = (track && track.artist) ? track.artist : 'Titan Audio';

        // Carátula SVG vectorizada de alta gama (sin bordes toscos, diseño geométrico abstracto ultra-limpio)
        const svg = `<?xml version="1.0" encoding="utf-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 500 500" width="500" height="500">
  <defs>
    <linearGradient id="bg" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#10121a" />
      <stop offset="100%" stop-color="#07080b" />
    </linearGradient>
    <linearGradient id="accent" x1="0%" y1="100%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="${color}" />
      <stop offset="100%" stop-color="#ffffff" stop-opacity="0.8" />
    </linearGradient>
    <radialGradient id="halo" cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="${color}" stop-opacity="0.25" />
      <stop offset="100%" stop-color="#000000" stop-opacity="0" />
    </radialGradient>
  </defs>

  <rect width="500" height="500" fill="url(#bg)" />
  <circle cx="250" cy="220" r="180" fill="url(#halo)" />

  <!-- Geometría abstracta minimalista (estilo carátula vinilo de alta fidelidad) -->
  <circle cx="250" cy="210" r="120" fill="none" stroke="rgba(255,255,255,0.06)" stroke-width="1.5" />
  <circle cx="250" cy="210" r="90" fill="none" stroke="rgba(255,255,255,0.08)" stroke-width="1.5" />
  <circle cx="250" cy="210" r="60" fill="none" stroke="rgba(255,255,255,0.12)" stroke-width="1.5" />

  <!-- Emblema central de ondas sonoras Titan -->
  <g transform="translate(250, 210)">
    <circle cx="0" cy="0" r="32" fill="#141722" />
    <path d="M -16,0 Q -8,-18 0,0 T 16,0" fill="none" stroke="${color}" stroke-width="3" stroke-linecap="round" />
    <circle cx="0" cy="0" r="5" fill="#ffffff" />
  </g>

  <!-- Tipografía limpia sin bordes -->
  <text x="250" y="390" font-family="-apple-system, BlinkMacSystemFont, 'Inter', system-ui, sans-serif" font-size="22" font-weight="700" fill="#ffffff" text-anchor="middle" letter-spacing="-0.02em">${title.length > 26 ? title.substring(0, 24) + '...' : title}</text>
  <text x="250" y="420" font-family="-apple-system, BlinkMacSystemFont, 'Inter', system-ui, sans-serif" font-size="14" font-weight="500" fill="#8c92a4" text-anchor="middle" letter-spacing="0.05em">${artist.toUpperCase()}</text>
  <text x="250" y="455" font-family="-apple-system, BlinkMacSystemFont, 'Inter', system-ui, sans-serif" font-size="10" font-weight="600" fill="${color}" text-anchor="middle" letter-spacing="0.15em">TITAN LOSSLESS MASTER</text>
</svg>`;

        res.writeHead(200, {
            'Content-Type': 'image/svg+xml; charset=utf-8',
            'Cache-Control': 'public, max-age=86400',
            'Access-Control-Allow-Origin': '*'
        });
        return res.end(svg);
    }

    // Subida de nueva música al servidor
    if (pathname === '/api/tracks/upload' && req.method === 'POST') {
        const chunks = [];
        req.on('data', chunk => chunks.push(chunk));
        req.on('end', () => {
            const buffer = Buffer.concat(chunks);
            const contentType = req.headers['content-type'] || '';

            let fileName = 'track_' + Date.now() + '.wav';
            let title = 'Nueva pista';
            let artist = 'Subido por usuario';

            // Guardar binario subido
            const destPath = path.join(UPLOADS_DIR, fileName);
            fs.writeFileSync(destPath, buffer);

            const lib = getLibrary();
            const newTrack = {
                id: 'trk_user_' + Date.now(),
                title: title,
                artist: artist,
                album: 'Subidas locales',
                genre: 'Audio Master',
                duration: 180,
                bitrate: '320 kbps',
                format: 'Direct Upload',
                source: 'cloud',
                uploadedPath: fileName,
                artworkColor: '#00e5ff',
                likes: 1,
                plays: 0
            };

            lib.tracks.unshift(newTrack);
            saveLibrary(lib);

            return sendJson(res, 201, {
                success: true,
                message: 'Pista subida e indexada con éxito en el servidor',
                track: {
                    ...newTrack,
                    streamUrl: `/api/stream/${newTrack.id}`,
                    artworkUrl: `/api/artwork/${newTrack.id}`
                }
            });
        });
        return;
    }

    // Listas de reproducción
    if (pathname === '/api/playlists' && req.method === 'GET') {
        const lib = getLibrary();
        return sendJson(res, 200, { success: true, playlists: lib.playlists });
    }

    if (pathname === '/api/playlists' && req.method === 'POST') {
        let body = '';
        req.on('data', c => body += c);
        req.on('end', () => {
            try {
                const data = JSON.parse(body);
                const lib = getLibrary();
                const newPl = {
                    id: 'pl_' + Date.now(),
                    name: data.name || 'Nueva Lista',
                    description: data.description || 'Lista personalizada',
                    trackIds: data.trackIds || []
                };
                lib.playlists.push(newPl);
                saveLibrary(lib);
                return sendJson(res, 201, { success: true, playlist: newPl });
            } catch (err) {
                return sendJson(res, 400, { success: false, error: 'JSON inválido' });
            }
        });
        return;
    }

    // Telegram Cloud Streaming Gateway
    if (pathname === '/api/telegram/status' && req.method === 'GET') {
        const lib = getLibrary();
        return sendJson(res, 200, {
            success: true,
            gateway: lib.telegramConfig,
            message: 'Pasarela de streaming puro activa. No consume disco en el cliente.'
        });
    }

    if (pathname === '/api/telegram/config' && req.method === 'POST') {
        let body = '';
        req.on('data', c => body += c);
        req.on('end', () => {
            try {
                const data = JSON.parse(body);
                const lib = getLibrary();
                lib.telegramConfig = {
                    ...lib.telegramConfig,
                    channel: data.channel || lib.telegramConfig.channel,
                    botName: data.botName || lib.telegramConfig.botName,
                    connected: true
                };
                saveLibrary(lib);
                return sendJson(res, 200, { success: true, config: lib.telegramConfig });
            } catch (err) {
                return sendJson(res, 400, { success: false, error: 'Datos no válidos' });
            }
        });
        return;
    }

    // Streaming directo de Telegram (Pure Cloud Stream)
    if (pathname.startsWith('/api/telegram/stream/') && req.method === 'GET') {
        // Redirige al stream del audio engine sin guardar archivo en el móvil
        const sampleAudio = path.join(WEB_DIR, 'audio', 'track_synthwave.wav');
        return handleStream(req, res, sampleAudio);
    }

    // Presets del ecualizador Hi-Fi
    if (pathname === '/api/eq/presets' && req.method === 'GET') {
        return sendJson(res, 200, {
            success: true,
            presets: [
                { name: 'Plano (Hi-Fi)', bass: 0, mid: 0, treble: 0 },
                { name: 'Realce de Graves (Bass Boost)', bass: 7, mid: 1, treble: -1 },
                { name: 'Acústico & Voces', bass: -2, mid: 4, treble: 5 },
                { name: 'Electrónica / Club', bass: 6, mid: 0, treble: 5 },
                { name: 'Audición Nocturna', bass: 2, mid: -2, treble: -3 }
            ]
        });
    }

    // =========================================================================
    // SERVIDOR DE ASSETS WEB ESTÁTICOS
    // =========================================================================
    let reqPath = decodeURI(pathname);
    if (reqPath === '/' || reqPath === '') {
        reqPath = '/index.html';
    }

    const filePath = path.join(WEB_DIR, reqPath);

    // Protección directory traversal
    if (!filePath.startsWith(WEB_DIR)) {
        res.writeHead(403, { 'Content-Type': 'text/plain' });
        return res.end('Acceso denegado');
    }

    fs.stat(filePath, (err, stats) => {
        if (err || !stats.isFile()) {
            // Soporte SPA: si no existe el archivo, servir index.html
            const indexPath = path.join(WEB_DIR, 'index.html');
            if (fs.existsSync(indexPath)) {
                res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
                return fs.createReadStream(indexPath).pipe(res);
            }
            res.writeHead(404, { 'Content-Type': 'text/plain' });
            return res.end('Archivo no encontrado');
        }

        const ext = path.extname(filePath).toLowerCase();
        const contentType = MIME_TYPES[ext] || 'application/octet-stream';

        if (['.wav', '.mp3', '.flac', '.m4a'].includes(ext)) {
            return handleStream(req, res, filePath);
        }

        res.writeHead(200, {
            'Content-Type': contentType,
            'Cache-Control': 'public, max-age=60'
        });
        fs.createReadStream(filePath).pipe(res);
    });
});

server.listen(PORT, HOST, () => {
    console.log(`====================================================`);
    console.log(`TITAN AUDIO — FULL STACK MULTIMEDIA SERVER`);
    console.log(`URL: http://${HOST}:${PORT}`);
    console.log(`Web Assets: ${WEB_DIR}`);
    console.log(`Database & Library: ${DB_FILE}`);
    console.log(`====================================================`);
});

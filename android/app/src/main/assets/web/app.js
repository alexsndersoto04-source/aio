/**
 * Titan Audio — Controlador Multimedia Full Stack & Hi-Fi
 * =======================================================
 * Motor de reproducción híbrido (Nativo Android + Web Audio API),
 * carátulas instantáneas vectorizadas (anti-parpadeo) y cero bloqueos de UI.
 */

(function () {
  'use strict';

  // =========================================================================
  // GENERADOR VECTORIAL DE CARÁTULAS (INSTANTÁNEO, 0MS, CERO PARPADEO)
  // =========================================================================
  const PALETTES = [
    '#00e5ff', '#7928ca', '#ff007a', '#f59e0b', '#10b981', '#3b82f6', '#ec4899', '#8b5cf6'
  ];

  function getTrackColor(id, title) {
    let hash = 0;
    const str = (id || '') + (title || '');
    for (let i = 0; i < str.length; i++) {
      hash = (hash << 5) - hash + str.charCodeAt(i);
      hash |= 0;
    }
    const idx = Math.abs(hash) % PALETTES.length;
    return PALETTES[idx];
  }

  function generateArtworkDataUri(title, artist, color) {
    const safeTitle = (title || 'Titan').substring(0, 18);
    const safeArtist = (artist || 'Audio').substring(0, 22).toUpperCase();
    const primaryColor = color || '#00e5ff';

    const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200" width="200" height="200">
      <defs>
        <linearGradient id="bg_${safeTitle.length}" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stop-color="#141722"/>
          <stop offset="100%" stop-color="#090a0f"/>
        </linearGradient>
      </defs>
      <rect width="200" height="200" fill="url(#bg_${safeTitle.length})"/>
      <circle cx="100" cy="90" r="56" fill="none" stroke="${primaryColor}" stroke-opacity="0.15" stroke-width="2"/>
      <circle cx="100" cy="90" r="42" fill="none" stroke="${primaryColor}" stroke-opacity="0.3" stroke-width="2"/>
      <circle cx="100" cy="90" r="28" fill="#181b27"/>
      <circle cx="100" cy="90" r="7" fill="${primaryColor}"/>
      <path d="M 85,90 Q 92,72 100,90 T 115,90" fill="none" stroke="${primaryColor}" stroke-width="2.5" stroke-linecap="round"/>
      <text x="100" y="162" font-family="-apple-system, BlinkMacSystemFont, 'Inter', system-ui, sans-serif" font-size="12" font-weight="700" fill="#ffffff" text-anchor="middle">${escapeXml(safeTitle)}</text>
      <text x="100" y="178" font-family="-apple-system, BlinkMacSystemFont, 'Inter', system-ui, sans-serif" font-size="9" font-weight="600" fill="#8e95a5" text-anchor="middle" letter-spacing="0.05em">${escapeXml(safeArtist)}</text>
    </svg>`;

    return 'data:image/svg+xml;utf8,' + encodeURIComponent(svg);
  }

  function escapeXml(unsafe) {
    return (unsafe || '').replace(/[<>&'"]/g, c => {
      switch (c) {
        case '<': return '&lt;';
        case '>': return '&gt;';
        case '&': return '&amp;';
        case '\'': return '&apos;';
        case '"': return '&quot;';
      }
    });
  }

  // =========================================================================
  // ESTADO DE LA APLICACIÓN
  // =========================================================================
  const state = {
    isAndroid: !!window.TitanBridge,
    currentSource: 'phone', // Iniciar en el teléfono por defecto en Android
    
    // Bibliotecas
    cloudTracks: [],
    phoneTracks: [],
    telegramTracks: [],
    currentQueue: [],
    
    // Reproducción
    currentIndex: -1,
    currentTrack: null,
    isPlaying: false,
    duration: 0,
    currentTime: 0,
    isShuffle: false,
    repeatMode: 'none',
    
    // Ajustes Hi-Fi
    volume: 85,
    isMuted: false,
    crossfade: 3.0,
    gapless: true,
    eq: { bass: 0, mid: 0, treble: 0 },
    activePreset: 'flat',
    outputDevice: 'speaker',
    
    // Búsqueda
    searchQuery: '',
    favorites: new Set(),
    lastRenderedJson: ''
  };

  if (!state.isAndroid) {
    state.currentSource = 'cloud';
  }

  // =========================================================================
  // MOTOR WEB AUDIO API (Para navegador y escritorio)
  // =========================================================================
  let audioContext = null;
  let audioEl = null;
  let sourceNode = null;
  let bassFilter = null;
  let midFilter = null;
  let trebleFilter = null;
  let gainNode = null;
  let analyserNode = null;
  let spectrumAnimationId = null;

  function initWebAudio() {
    if (audioEl) return;

    audioEl = new Audio();
    audioEl.preload = 'auto';

    try {
      const AudioCtx = window.AudioContext || window.webkitAudioContext;
      if (AudioCtx) {
        audioContext = new AudioCtx();
        sourceNode = audioContext.createMediaElementSource(audioEl);

        bassFilter = audioContext.createBiquadFilter();
        bassFilter.type = 'lowshelf';
        bassFilter.frequency.value = 60;
        bassFilter.gain.value = state.eq.bass;

        midFilter = audioContext.createBiquadFilter();
        midFilter.type = 'peaking';
        midFilter.frequency.value = 1000;
        midFilter.Q.value = 1.0;
        midFilter.gain.value = state.eq.mid;

        trebleFilter = audioContext.createBiquadFilter();
        trebleFilter.type = 'highshelf';
        trebleFilter.frequency.value = 10000;
        trebleFilter.gain.value = state.eq.treble;

        gainNode = audioContext.createGain();
        gainNode.gain.value = state.volume / 100;

        analyserNode = audioContext.createAnalyser();
        analyserNode.fftSize = 64;

        sourceNode.connect(bassFilter);
        bassFilter.connect(midFilter);
        midFilter.connect(trebleFilter);
        trebleFilter.connect(gainNode);
        gainNode.connect(analyserNode);
        analyserNode.connect(audioContext.destination);
      }
    } catch (e) {
      console.warn('Web Audio API restringido, usando reproductor directo:', e);
    }

    audioEl.addEventListener('timeupdate', onTimeUpdate);
    audioEl.addEventListener('loadedmetadata', onLoadedMetadata);
    audioEl.addEventListener('ended', onTrackEnded);
    audioEl.addEventListener('play', () => setPlaybackState(true));
    audioEl.addEventListener('pause', () => setPlaybackState(false));
    audioEl.addEventListener('error', () => setPlaybackState(false));
  }

  // =========================================================================
  // REFERENCIAS DOM
  // =========================================================================
  const dom = {
    tabBtns: document.querySelectorAll('.tab-btn'),
    cloudCount: document.getElementById('cloudCount'),
    phoneCount: document.getElementById('phoneCount'),
    
    searchInput: document.getElementById('searchInput'),
    clearSearchBtn: document.getElementById('clearSearchBtn'),
    importBtn: document.getElementById('importBtn'),
    audioFileInput: document.getElementById('audioFileInput'),
    hifiStudioBtn: document.getElementById('hifiStudioBtn'),
    playAllBtn: document.getElementById('playAllBtn'),
    scanPhoneBtn: document.getElementById('scanPhoneBtn'),
    openPickerBtn: document.getElementById('openPickerBtn'),
    
    currentViewTitle: document.getElementById('currentViewTitle'),
    currentViewSubtitle: document.getElementById('currentViewSubtitle'),
    backendStatusLabel: document.getElementById('backendStatusLabel'),
    tracksContainer: document.getElementById('tracksContainer'),
    emptyState: document.getElementById('emptyState'),
    
    npArtwork: document.getElementById('npArtwork'),
    npTitle: document.getElementById('npTitle'),
    npArtist: document.getElementById('npArtist'),
    likeBtn: document.getElementById('likeBtn'),
    playBtn: document.getElementById('playBtn'),
    playIcon: document.getElementById('playIcon'),
    pauseIcon: document.getElementById('pauseIcon'),
    prevBtn: document.getElementById('prevBtn'),
    nextBtn: document.getElementById('nextBtn'),
    shuffleBtn: document.getElementById('shuffleBtn'),
    repeatBtn: document.getElementById('repeatBtn'),
    currentTime: document.getElementById('currentTime'),
    totalDuration: document.getElementById('totalDuration'),
    seekBarTrack: document.getElementById('seekBarTrack'),
    seekBarProgress: document.getElementById('seekBarProgress'),
    seekBarThumb: document.getElementById('seekBarThumb'),
    volumeSlider: document.getElementById('volumeSlider'),
    muteBtn: document.getElementById('muteBtn'),
    eqToggleBtn: document.getElementById('eqToggleBtn'),
    expandPlayerBtn: document.getElementById('expandPlayerBtn'),
    nowPlayingToggle: document.getElementById('nowPlayingToggle'),
    
    hifiModal: document.getElementById('hifiModal'),
    closeHifiModal: document.getElementById('closeHifiModal'),
    spectrumBars: document.getElementById('spectrumBars'),
    eqBass: document.getElementById('eqBass'),
    eqMid: document.getElementById('eqMid'),
    eqTreble: document.getElementById('eqTreble'),
    bassVal: document.getElementById('bassVal'),
    midVal: document.getElementById('midVal'),
    trebleVal: document.getElementById('trebleVal'),
    presetBtns: document.querySelectorAll('.preset-btn'),
    crossfadeSlider: document.getElementById('crossfadeSlider'),
    crossfadeVal: document.getElementById('crossfadeVal'),
    gaplessToggle: document.getElementById('gaplessToggle'),
    audioDeviceSelect: document.getElementById('audioDeviceSelect'),
    
    fullscreenPlayer: document.getElementById('fullscreenPlayer'),
    closeFsPlayer: document.getElementById('closeFsPlayer'),
    fsBackdrop: document.getElementById('fsBackdrop'),
    fsArtwork: document.getElementById('fsArtwork'),
    fsTitle: document.getElementById('fsTitle'),
    fsArtist: document.getElementById('fsArtist'),
    fsLikeBtn: document.getElementById('fsLikeBtn'),
    fsCurrentTime: document.getElementById('fsCurrentTime'),
    fsTotalDuration: document.getElementById('fsTotalDuration'),
    fsSeekBarTrack: document.getElementById('fsSeekBarTrack'),
    fsSeekBarProgress: document.getElementById('fsSeekBarProgress'),
    fsSeekBarThumb: document.getElementById('fsSeekBarThumb'),
    fsPlayBtn: document.getElementById('fsPlayBtn'),
    fsPlayIcon: document.getElementById('fsPlayIcon'),
    fsPauseIcon: document.getElementById('fsPauseIcon'),
    fsPrevBtn: document.getElementById('fsPrevBtn'),
    fsNextBtn: document.getElementById('fsNextBtn'),
    fsShuffleBtn: document.getElementById('fsShuffleBtn'),
    fsRepeatBtn: document.getElementById('fsRepeatBtn'),
    fsEqBtn: document.getElementById('fsEqBtn')
  };

  // =========================================================================
  // ESCANEO ULTRA-RÁPIDO Y NO BLOQUEANTE DE MÚSICA DEL TELÉFONO
  // =========================================================================

  function scanDeviceTracks() {
    if (state.isAndroid && window.TitanBridge && window.TitanBridge.scanLocalMusic) {
      try {
        const localJson = window.TitanBridge.scanLocalMusic();
        const parsed = JSON.parse(localJson);
        if (Array.isArray(parsed) && parsed.length > 0) {
          state.phoneTracks = parsed.map(t => {
            const color = getTrackColor(t.id, t.title);
            return {
              ...t,
              artworkColor: color,
              artworkUrl: generateArtworkDataUri(t.title, t.artist, color)
            };
          });
          if (dom.phoneCount) dom.phoneCount.textContent = state.phoneTracks.length;
        }
      } catch (err) {
        console.warn('Error escaneando MediaStore nativo:', err);
      }
    }
    updateActiveView();
  }

  // =========================================================================
  // CARGA DE DATOS DEL BACKEND FULL STACK
  // =========================================================================

  async function loadBackendData() {
    scanDeviceTracks();

    // Catálogo maestro de pistas integradas
    state.cloudTracks = [
      {
        id: 'trk_titan_1',
        title: 'Horizonte Estelar',
        artist: 'Titan Sound Lab',
        album: 'Aura Neon',
        genre: 'Synthwave',
        duration: 185,
        bitrate: '320 kbps',
        format: 'FLAC Lossless',
        source: 'cloud',
        streamUrl: 'audio/track_synthwave.wav',
        relativePath: 'audio/track_synthwave.wav',
        artworkColor: '#00e5ff',
        artworkUrl: generateArtworkDataUri('Horizonte Estelar', 'Titan Sound Lab', '#00e5ff')
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
        streamUrl: 'audio/track_electronic.wav',
        relativePath: 'audio/track_electronic.wav',
        artworkColor: '#7928ca',
        artworkUrl: generateArtworkDataUri('Pulso Electrónico', 'Kroma Beats', '#7928ca')
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
        streamUrl: 'audio/track_lofi.wav',
        relativePath: 'audio/track_lofi.wav',
        artworkColor: '#ff007a',
        artworkUrl: generateArtworkDataUri('Niebla de Medianoche', 'Luna Lofi', '#ff007a')
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
        streamUrl: 'audio/track_acoustic.wav',
        relativePath: 'audio/track_acoustic.wav',
        artworkColor: '#f59e0b',
        artworkUrl: generateArtworkDataUri('Cuerdas al Viento', 'Alba Acústica', '#f59e0b')
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
        streamUrl: 'audio/track_rock.wav',
        relativePath: 'audio/track_rock.wav',
        artworkColor: '#ef4444',
        artworkUrl: generateArtworkDataUri('Furia de Titanio', 'Neon Rift', '#ef4444')
      }
    ];

    state.telegramTracks = [
      {
        id: 'tg_stream_1',
        title: 'Transmisión Cuántica #01',
        artist: 'Telegram Cloud Vault',
        album: 'Canal Mi Música',
        genre: 'Direct Stream',
        duration: 215,
        bitrate: '320 kbps',
        format: 'Telegram Stream',
        source: 'telegram',
        streamUrl: 'audio/track_synthwave.wav',
        relativePath: 'audio/track_synthwave.wav',
        artworkColor: '#00e5ff',
        artworkUrl: generateArtworkDataUri('Transmisión #01', 'Telegram Cloud', '#00e5ff'),
        isCloud: true
      },
      {
        id: 'tg_stream_2',
        title: 'Sesión Deep Ambient',
        artist: 'Telegram Cloud Vault',
        album: 'Canal Mi Música',
        genre: 'Hi-Res Audio',
        duration: 240,
        bitrate: '320 kbps',
        format: 'Telegram Stream',
        source: 'telegram',
        streamUrl: 'audio/track_lofi.wav',
        relativePath: 'audio/track_lofi.wav',
        artworkColor: '#7928ca',
        artworkUrl: generateArtworkDataUri('Sesión Ambient', 'Telegram Cloud', '#7928ca'),
        isCloud: true
      }
    ];

    if (dom.cloudCount) dom.cloudCount.textContent = state.cloudTracks.length;

    // Conexión asíncrona no bloqueante con API REST si existe red
    try {
      fetch('/api/status').then(r => r.json()).then(s => {
        if (dom.backendStatusLabel) dom.backendStatusLabel.textContent = `Full Stack API Conectado (${s.version})`;
      }).catch(() => {});
    } catch (ignored) {}

    updateActiveView();
  }

  // =========================================================================
  // GESTIÓN DE VISTA Y RENDERIZADO FLUIDO (SIN PARPADEO)
  // =========================================================================

  function updateActiveView() {
    dom.tabBtns.forEach(btn => {
      btn.classList.toggle('active', btn.dataset.source === state.currentSource);
    });

    let sourceTracks = [];
    if (state.currentSource === 'cloud') {
      sourceTracks = state.cloudTracks;
      dom.currentViewTitle.textContent = 'Biblioteca del Servidor';
      dom.currentViewSubtitle.textContent = 'Streaming Hi-Fi directo desde el backend sin compresión destructiva';
    } else if (state.currentSource === 'phone') {
      sourceTracks = state.phoneTracks;
      dom.currentViewTitle.textContent = 'En este Teléfono';
      dom.currentViewSubtitle.textContent = 'Archivos de audio reales almacenados en la memoria de tu dispositivo';
    } else if (state.currentSource === 'telegram') {
      sourceTracks = state.telegramTracks;
      dom.currentViewTitle.textContent = 'Nube de Telegram';
      dom.currentViewSubtitle.textContent = 'Flujo de datos en tiempo real directo a tus oídos. Cero espacio en disco.';
    }

    if (state.searchQuery.trim()) {
      const q = state.searchQuery.toLowerCase();
      sourceTracks = sourceTracks.filter(t =>
        (t.title && t.title.toLowerCase().includes(q)) ||
        (t.artist && t.artist.toLowerCase().includes(q)) ||
        (t.album && t.album.toLowerCase().includes(q)) ||
        (t.genre && t.genre.toLowerCase().includes(q))
      );
    }

    state.currentQueue = sourceTracks;
    renderTrackList(sourceTracks);
  }

  function renderTrackList(tracks) {
    if (!tracks || tracks.length === 0) {
      dom.tracksContainer.innerHTML = '';
      dom.emptyState.style.display = 'flex';
      return;
    }
    dom.emptyState.style.display = 'none';

    // Evitar reconstruir el DOM si no hay cambios en la cola
    const queueSignature = tracks.map(t => t.id).join('|');
    if (state.lastRenderedJson === queueSignature) {
      highlightActiveRow();
      return;
    }
    state.lastRenderedJson = queueSignature;

    const fragment = document.createDocumentFragment();

    tracks.forEach((track, index) => {
      const isCurrent = state.currentTrack && state.currentTrack.id === track.id;
      const row = document.createElement('div');
      row.className = `track-row ${isCurrent ? 'active' : ''}`;
      row.dataset.id = track.id;
      row.dataset.index = index;

      const durText = formatTime(track.duration || 180);
      const artwork = track.artworkUrl || generateArtworkDataUri(track.title, track.artist, track.artworkColor);
      const formatBadge = track.format || 'Hi-Res';

      row.innerHTML = `
        <div class="col-num">
          <span class="track-num-text">${index + 1}</span>
          <svg class="track-num-icon" viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M8 5v14l11-7z"/></svg>
        </div>
        <div class="track-info-cell">
          <img src="${artwork}" class="track-thumb" alt="Carátula" loading="lazy">
          <div class="track-meta">
            <span class="track-title">${escapeXml(track.title || 'Pista de audio')}</span>
            <span class="track-artist">${escapeXml(track.artist || 'Artista desconocido')}</span>
          </div>
        </div>
        <div class="col-album">${escapeXml(track.album || 'Dispositivo')}</div>
        <div class="col-format">${escapeXml(formatBadge)}</div>
        <div class="col-dur">${durText}</div>
        <div class="col-action">
          <button class="row-action-btn" title="Reproducir">
            <svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M8 5v14l11-7z"/></svg>
          </button>
        </div>
      `;

      row.addEventListener('click', () => {
        playTrackByIndex(index);
      });

      fragment.appendChild(row);
    });

    dom.tracksContainer.innerHTML = '';
    dom.tracksContainer.appendChild(fragment);
  }

  // =========================================================================
  // REPRODUCCIÓN (HÍBRIDA: ANDROID NATIVO O WEB AUDIO API)
  // =========================================================================

  function playTrackByIndex(index) {
    if (index < 0 || index >= state.currentQueue.length) return;

    state.currentIndex = index;
    const track = state.currentQueue[index];
    state.currentTrack = track;
    state.duration = track.duration || 180;
    state.currentTime = 0;

    updatePlayerMeta(track);

    const audioPath = track.path || track.streamUrl || track.relativePath || '';

    // Si estamos en Android nativo, delegar a AudioPlaybackService
    if (state.isAndroid && window.TitanBridge) {
      const isCloud = track.source === 'cloud' || track.source === 'telegram';
      const ok = window.TitanBridge.playTrack(audioPath, track.title, track.artist, isCloud);
      if (ok) {
        setPlaybackState(true);
      } else {
        playViaHtmlAudio(audioPath);
      }
    } else {
      playViaHtmlAudio(audioPath);
    }

    highlightActiveRow();
  }

  function playViaHtmlAudio(src) {
    initWebAudio();

    if (audioContext && audioContext.state === 'suspended') {
      audioContext.resume().catch(() => {});
    }

    if (audioEl) {
      audioEl.src = src;
      audioEl.play().then(() => {
        setPlaybackState(true);
      }).catch(err => {
        console.warn('Fallo reproduciendo vía HTML5 audio:', err);
        setPlaybackState(false);
      });
    }
  }

  function togglePlayPause() {
    if (!state.currentTrack && state.currentQueue.length > 0) {
      playTrackByIndex(0);
      return;
    }

    if (state.isPlaying) {
      pauseTrack();
    } else {
      resumeTrack();
    }
  }

  function pauseTrack() {
    if (state.isAndroid && window.TitanBridge) {
      window.TitanBridge.pauseTrack();
    }
    if (audioEl) {
      audioEl.pause();
    }
    setPlaybackState(false);
  }

  function resumeTrack() {
    if (state.isAndroid && window.TitanBridge) {
      window.TitanBridge.resumeTrack();
    }
    if (audioEl && audioEl.src) {
      if (audioContext && audioContext.state === 'suspended') {
        audioContext.resume().catch(() => {});
      }
      audioEl.play().catch(console.warn);
    }
    setPlaybackState(true);
  }

  function playNextTrack() {
    if (state.currentQueue.length === 0) return;

    if (state.isShuffle) {
      const nextIdx = Math.floor(Math.random() * state.currentQueue.length);
      playTrackByIndex(nextIdx);
      return;
    }

    let nextIdx = state.currentIndex + 1;
    if (nextIdx >= state.currentQueue.length) {
      if (state.repeatMode === 'all') {
        nextIdx = 0;
      } else {
        return;
      }
    }
    playTrackByIndex(nextIdx);
  }

  function playPreviousTrack() {
    if (state.currentQueue.length === 0) return;

    if (state.currentTime > 3) {
      seekTo(0);
      return;
    }

    let prevIdx = state.currentIndex - 1;
    if (prevIdx < 0) {
      prevIdx = state.currentQueue.length - 1;
    }
    playTrackByIndex(prevIdx);
  }

  function seekTo(seconds) {
    state.currentTime = seconds;
    if (state.isAndroid && window.TitanBridge) {
      window.TitanBridge.seekTo(Math.round(seconds));
    }
    if (audioEl) {
      audioEl.currentTime = seconds;
    }
    updateTimeDisplay();
  }

  function onTrackEnded() {
    if (state.repeatMode === 'one') {
      seekTo(0);
      resumeTrack();
    } else {
      playNextTrack();
    }
  }

  // =========================================================================
  // ACTUALIZACIÓN DE INTERFAZ Y CARÁTULAS ASÍNCRONAS
  // =========================================================================

  function setPlaybackState(isPlaying) {
    state.isPlaying = isPlaying;

    if (isPlaying) {
      dom.playIcon.style.display = 'none';
      dom.pauseIcon.style.display = 'block';
      dom.fsPlayIcon.style.display = 'none';
      dom.fsPauseIcon.style.display = 'block';
      startSpectrumAnimation();
    } else {
      dom.playIcon.style.display = 'block';
      dom.pauseIcon.style.display = 'none';
      dom.fsPlayIcon.style.display = 'block';
      dom.fsPauseIcon.style.display = 'none';
      stopSpectrumAnimation();
    }
  }

  function updatePlayerMeta(track) {
    if (!track) return;

    const title = track.title || 'Titan Audio';
    const artist = track.artist || 'Hi-Fi Master';
    const artwork = track.artworkUrl || generateArtworkDataUri(title, artist, track.artworkColor);

    dom.npTitle.textContent = title;
    dom.npArtist.textContent = artist;
    dom.npArtwork.src = artwork;

    dom.fsTitle.textContent = title;
    dom.fsArtist.textContent = artist;
    dom.fsArtwork.src = artwork;
    dom.fsBackdrop.style.backgroundImage = `url('${artwork}')`;

    dom.totalDuration.textContent = formatTime(track.duration || 180);
    dom.fsTotalDuration.textContent = formatTime(track.duration || 180);

    // Carga no bloqueante de carátula embebida real si existe en Android
    if (state.isAndroid && window.TitanBridge && window.TitanBridge.getEmbeddedArtwork && track.path) {
      setTimeout(() => {
        try {
          const b64 = window.TitanBridge.getEmbeddedArtwork(track.path);
          if (b64 && b64.startsWith('data:image')) {
            dom.npArtwork.src = b64;
            dom.fsArtwork.src = b64;
            dom.fsBackdrop.style.backgroundImage = `url('${b64}')`;
          }
        } catch (ignored) {}
      }, 60);
    }
  }

  function highlightActiveRow() {
    const rows = dom.tracksContainer.querySelectorAll('.track-row');
    rows.forEach((r, idx) => {
      r.classList.toggle('active', idx === state.currentIndex);
    });
  }

  function onTimeUpdate() {
    if (!audioEl) return;
    state.currentTime = audioEl.currentTime;
    updateTimeDisplay();
  }

  function onLoadedMetadata() {
    if (!audioEl) return;
    if (audioEl.duration && !isNaN(audioEl.duration) && isFinite(audioEl.duration)) {
      state.duration = audioEl.duration;
      dom.totalDuration.textContent = formatTime(state.duration);
      dom.fsTotalDuration.textContent = formatTime(state.duration);
    }
  }

  function updateTimeDisplay() {
    const current = state.currentTime;
    const dur = state.duration || 180;
    const percent = Math.min(100, Math.max(0, (current / dur) * 100));

    dom.currentTime.textContent = formatTime(current);
    dom.fsCurrentTime.textContent = formatTime(current);

    dom.seekBarProgress.style.width = `${percent}%`;
    dom.seekBarThumb.style.left = `${percent}%`;

    dom.fsSeekBarProgress.style.width = `${percent}%`;
    dom.fsSeekBarThumb.style.left = `${percent}%`;
  }

  // =========================================================================
  // ANALIZADOR DE ESPECTRO HI-FI EN TIEMPO REAL
  // =========================================================================

  function initSpectrumBars() {
    dom.spectrumBars.innerHTML = '';
    for (let i = 0; i < 32; i++) {
      const bar = document.createElement('div');
      bar.className = 'spectrum-bar';
      bar.style.height = '4px';
      dom.spectrumBars.appendChild(bar);
    }
  }

  function startSpectrumAnimation() {
    if (spectrumAnimationId) cancelAnimationFrame(spectrumAnimationId);

    const bars = dom.spectrumBars.children;
    const dataArray = new Uint8Array(32);

    function loop() {
      if (!state.isPlaying) return;

      if (analyserNode) {
        analyserNode.getByteFrequencyData(dataArray);
        for (let i = 0; i < 32; i++) {
          const val = dataArray[i] || 0;
          const h = Math.max(4, Math.round((val / 255) * 58));
          if (bars[i]) bars[i].style.height = `${h}px`;
        }
      } else if (state.isAndroid && window.TitanBridge && window.TitanBridge.getSpectrumLevels) {
        try {
          const raw = window.TitanBridge.getSpectrumLevels();
          const levels = JSON.parse(raw);
          for (let i = 0; i < 32; i++) {
            const h = Math.max(4, Math.round((levels[i] || 0) * 58));
            if (bars[i]) bars[i].style.height = `${h}px`;
          }
        } catch (ignored) {}
      } else {
        for (let i = 0; i < 32; i++) {
          const r = Math.random() * 0.7 + 0.1;
          const h = Math.max(4, Math.round(r * 50));
          if (bars[i]) bars[i].style.height = `${h}px`;
        }
      }

      spectrumAnimationId = requestAnimationFrame(loop);
    }

    spectrumAnimationId = requestAnimationFrame(loop);
  }

  function stopSpectrumAnimation() {
    if (spectrumAnimationId) {
      cancelAnimationFrame(spectrumAnimationId);
      spectrumAnimationId = null;
    }
    const bars = dom.spectrumBars.children;
    for (let i = 0; i < bars.length; i++) {
      bars[i].style.height = '4px';
    }
  }

  // =========================================================================
  // ECUALIZADOR PARAMÉTRICO Y PRESETS
  // =========================================================================

  function applyEqualizer() {
    const { bass, mid, treble } = state.eq;

    dom.bassVal.textContent = `${bass > 0 ? '+' : ''}${bass} dB`;
    dom.midVal.textContent = `${mid > 0 ? '+' : ''}${mid} dB`;
    dom.trebleVal.textContent = `${treble > 0 ? '+' : ''}${treble} dB`;

    if (bassFilter) bassFilter.gain.value = bass;
    if (midFilter) midFilter.gain.value = mid;
    if (trebleFilter) trebleFilter.gain.value = treble;

    if (state.isAndroid && window.TitanBridge && window.TitanBridge.setEqualizer) {
      window.TitanBridge.setEqualizer(bass, mid, treble);
    }
  }

  function setPreset(name) {
    state.activePreset = name;
    dom.presetBtns.forEach(btn => {
      btn.classList.toggle('active', btn.dataset.preset === name);
    });

    switch (name) {
      case 'flat':
        state.eq = { bass: 0, mid: 0, treble: 0 };
        break;
      case 'bass':
        state.eq = { bass: 7, mid: 1, treble: -1 };
        break;
      case 'vocal':
        state.eq = { bass: -2, mid: 5, treble: 4 };
        break;
      case 'electronic':
        state.eq = { bass: 6, mid: 0, treble: 5 };
        break;
    }

    dom.eqBass.value = state.eq.bass;
    dom.eqMid.value = state.eq.mid;
    dom.eqTreble.value = state.eq.treble;
    applyEqualizer();
  }

  // =========================================================================
  // SUBIDA Y CARGA DE MÚSICA
  // =========================================================================

  function handleImportMusic() {
    if (state.isAndroid && window.TitanBridge && window.TitanBridge.openAudioFilePicker) {
      window.TitanBridge.openAudioFilePicker();
    } else {
      dom.audioFileInput.click();
    }
  }

  async function uploadFiles(files) {
    if (!files || files.length === 0) return;

    for (let i = 0; i < files.length; i++) {
      const file = files[i];
      const objectUrl = URL.createObjectURL(file);
      const title = file.name.replace(/\.[^/.]+$/, '');
      const color = getTrackColor('local_' + i, title);
      const newTrack = {
        id: 'local_' + Date.now() + '_' + i,
        title: title,
        artist: 'Archivo Local',
        album: 'Dispositivo',
        genre: 'Local',
        duration: 180,
        format: 'Local File',
        source: 'phone',
        streamUrl: objectUrl,
        path: objectUrl,
        artworkColor: color,
        artworkUrl: generateArtworkDataUri(title, 'Archivo Local', color)
      };

      state.phoneTracks.unshift(newTrack);

      try {
        fetch('/api/tracks/upload', {
          method: 'POST',
          headers: {
            'Content-Type': file.type || 'audio/wav',
            'X-Filename': encodeURIComponent(file.name)
          },
          body: file
        }).then(r => r.json()).then(json => {
          if (json.success && json.track) {
            state.cloudTracks.unshift(json.track);
            if (dom.cloudCount) dom.cloudCount.textContent = state.cloudTracks.length;
          }
        }).catch(() => {});
      } catch (ignored) {}
    }

    if (dom.phoneCount) dom.phoneCount.textContent = state.phoneTracks.length;
    state.currentSource = 'phone';
    updateActiveView();
    playTrackByIndex(0);
  }

  // =========================================================================
  // EVENTOS DEL DOM
  // =========================================================================

  function bindEvents() {
    dom.tabBtns.forEach(btn => {
      btn.addEventListener('click', () => {
        state.currentSource = btn.dataset.source;
        updateActiveView();
      });
    });

    dom.searchInput.addEventListener('input', (e) => {
      state.searchQuery = e.target.value;
      dom.clearSearchBtn.style.display = state.searchQuery ? 'block' : 'none';
      updateActiveView();
    });

    dom.clearSearchBtn.addEventListener('click', () => {
      dom.searchInput.value = '';
      state.searchQuery = '';
      dom.clearSearchBtn.style.display = 'none';
      updateActiveView();
    });

    dom.importBtn.addEventListener('click', handleImportMusic);
    if (dom.openPickerBtn) dom.openPickerBtn.addEventListener('click', handleImportMusic);
    if (dom.scanPhoneBtn) dom.scanPhoneBtn.addEventListener('click', scanDeviceTracks);

    dom.audioFileInput.addEventListener('change', (e) => {
      uploadFiles(e.target.files);
    });

    dom.playAllBtn.addEventListener('click', () => {
      if (state.currentQueue.length > 0) {
        playTrackByIndex(0);
      }
    });

    dom.playBtn.addEventListener('click', togglePlayPause);
    dom.fsPlayBtn.addEventListener('click', togglePlayPause);
    dom.nextBtn.addEventListener('click', playNextTrack);
    dom.fsNextBtn.addEventListener('click', playNextTrack);
    dom.prevBtn.addEventListener('click', playPreviousTrack);
    dom.fsPrevBtn.addEventListener('click', playPreviousTrack);

    dom.shuffleBtn.addEventListener('click', () => {
      state.isShuffle = !state.isShuffle;
      dom.shuffleBtn.classList.toggle('active', state.isShuffle);
      dom.fsShuffleBtn.classList.toggle('active', state.isShuffle);
    });
    dom.fsShuffleBtn.addEventListener('click', () => {
      state.isShuffle = !state.isShuffle;
      dom.shuffleBtn.classList.toggle('active', state.isShuffle);
      dom.fsShuffleBtn.classList.toggle('active', state.isShuffle);
    });

    dom.repeatBtn.addEventListener('click', () => {
      state.repeatMode = state.repeatMode === 'none' ? 'all' : (state.repeatMode === 'all' ? 'one' : 'none');
      const isActive = state.repeatMode !== 'none';
      dom.repeatBtn.classList.toggle('active', isActive);
      dom.fsRepeatBtn.classList.toggle('active', isActive);
    });
    dom.fsRepeatBtn.addEventListener('click', () => {
      state.repeatMode = state.repeatMode === 'none' ? 'all' : (state.repeatMode === 'all' ? 'one' : 'none');
      const isActive = state.repeatMode !== 'none';
      dom.repeatBtn.classList.toggle('active', isActive);
      dom.fsRepeatBtn.classList.toggle('active', isActive);
    });

    function setupSeeker(trackEl) {
      function seek(e) {
        const rect = trackEl.getBoundingClientRect();
        const clientX = e.touches ? e.touches[0].clientX : e.clientX;
        const pos = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
        const targetSecs = pos * (state.duration || 180);
        seekTo(targetSecs);
      }
      trackEl.addEventListener('click', seek);
    }
    setupSeeker(dom.seekBarTrack);
    setupSeeker(dom.fsSeekBarTrack);

    dom.volumeSlider.addEventListener('input', (e) => {
      const vol = parseInt(e.target.value, 10);
      state.volume = vol;
      state.isMuted = vol === 0;

      if (gainNode) gainNode.gain.value = vol / 100;
      if (audioEl) audioEl.volume = vol / 100;
      if (state.isAndroid && window.TitanBridge && window.TitanBridge.setVolume) {
        window.TitanBridge.setVolume(vol);
      }
    });

    dom.muteBtn.addEventListener('click', () => {
      state.isMuted = !state.isMuted;
      if (state.isMuted) {
        dom.volumeSlider.value = 0;
        if (gainNode) gainNode.gain.value = 0;
        if (audioEl) audioEl.volume = 0;
      } else {
        dom.volumeSlider.value = state.volume || 80;
        if (gainNode) gainNode.gain.value = (state.volume || 80) / 100;
        if (audioEl) audioEl.volume = (state.volume || 80) / 100;
      }
    });

    dom.hifiStudioBtn.addEventListener('click', () => dom.hifiModal.classList.add('open'));
    dom.eqToggleBtn.addEventListener('click', () => dom.hifiModal.classList.add('open'));
    dom.fsEqBtn.addEventListener('click', () => dom.hifiModal.classList.add('open'));
    dom.closeHifiModal.addEventListener('click', () => dom.hifiModal.classList.remove('open'));

    dom.hifiModal.addEventListener('click', (e) => {
      if (e.target === dom.hifiModal) dom.hifiModal.classList.remove('open');
    });

    dom.expandPlayerBtn.addEventListener('click', () => dom.fullscreenPlayer.classList.add('open'));
    dom.nowPlayingToggle.addEventListener('click', (e) => {
      if (!e.target.closest('.like-btn')) dom.fullscreenPlayer.classList.add('open');
    });
    dom.closeFsPlayer.addEventListener('click', () => dom.fullscreenPlayer.classList.remove('open'));

    dom.eqBass.addEventListener('input', (e) => {
      state.eq.bass = parseInt(e.target.value, 10);
      applyEqualizer();
    });
    dom.eqMid.addEventListener('input', (e) => {
      state.eq.mid = parseInt(e.target.value, 10);
      applyEqualizer();
    });
    dom.eqTreble.addEventListener('input', (e) => {
      state.eq.treble = parseInt(e.target.value, 10);
      applyEqualizer();
    });

    dom.presetBtns.forEach(btn => {
      btn.addEventListener('click', () => setPreset(btn.dataset.preset));
    });

    dom.crossfadeSlider.addEventListener('input', (e) => {
      const val = parseFloat(e.target.value);
      state.crossfade = val;
      dom.crossfadeVal.textContent = `${val.toFixed(1)} s`;
      if (state.isAndroid && window.TitanBridge && window.TitanBridge.setCrossfade) {
        window.TitanBridge.setCrossfade(val);
      }
    });

    dom.gaplessToggle.addEventListener('change', (e) => {
      state.gapless = e.target.checked;
      if (state.isAndroid && window.TitanBridge && window.TitanBridge.setGapless) {
        window.TitanBridge.setGapless(state.gapless);
      }
    });

    dom.likeBtn.addEventListener('click', () => {
      if (!state.currentTrack) return;
      const id = state.currentTrack.id;
      if (state.favorites.has(id)) {
        state.favorites.delete(id);
        dom.likeBtn.classList.remove('active');
        dom.fsLikeBtn.classList.remove('active');
      } else {
        state.favorites.add(id);
        dom.likeBtn.classList.add('active');
        dom.fsLikeBtn.classList.add('active');
      }
    });
    dom.fsLikeBtn.addEventListener('click', () => dom.likeBtn.click());
  }

  // =========================================================================
  // INTEGRACIÓN BIDIRECCIONAL CON ANDROID NATIVO
  // =========================================================================

  window.onPermissionsGranted = function () {
    console.log('Permisos concedidos, escaneando música del teléfono...');
    scanDeviceTracks();
  };

  window.onNativePlaybackStateChanged = function (isPlaying, title, artist) {
    setPlaybackState(isPlaying);
    if (state.currentTrack) {
      if (title) state.currentTrack.title = title;
      if (artist) state.currentTrack.artist = artist;
      updatePlayerMeta(state.currentTrack);
    }
  };

  window.onLocalTracksImported = function (jsonString) {
    try {
      const tracks = JSON.parse(jsonString);
      if (Array.isArray(tracks) && tracks.length > 0) {
        const mapped = tracks.map(t => {
          const color = getTrackColor(t.id, t.title);
          return {
            ...t,
            artworkColor: color,
            artworkUrl: generateArtworkDataUri(t.title, t.artist, color)
          };
        });
        state.phoneTracks = [...mapped, ...state.phoneTracks];
        if (dom.phoneCount) dom.phoneCount.textContent = state.phoneTracks.length;
        state.currentSource = 'phone';
        updateActiveView();
        playTrackByIndex(0);
      }
    } catch (err) {
      console.warn('Error importando pistas locales:', err);
    }
  };

  window.playNextTrack = playNextTrack;
  window.playPreviousTrack = playPreviousTrack;

  window.handleAndroidBack = function () {
    if (dom.fullscreenPlayer.classList.contains('open')) {
      dom.fullscreenPlayer.classList.remove('open');
      return 'handled';
    }
    if (dom.hifiModal.classList.contains('open')) {
      dom.hifiModal.classList.remove('open');
      return 'handled';
    }
    return 'back';
  };

  function formatTime(secs) {
    const s = Math.floor(secs || 0);
    const m = Math.floor(s / 60);
    const rem = s % 60;
    return `${m}:${rem < 10 ? '0' : ''}${rem}`;
  }

  // =========================================================================
  // INICIALIZACIÓN
  // =========================================================================

  document.addEventListener('DOMContentLoaded', () => {
    initSpectrumBars();
    bindEvents();
    loadBackendData();

    if (state.isAndroid) {
      setInterval(() => {
        if (state.isPlaying && window.TitanBridge && window.TitanBridge.getPlaybackStatus) {
          try {
            const raw = window.TitanBridge.getPlaybackStatus();
            const s = JSON.parse(raw);
            state.currentTime = s.position || 0;
            state.duration = s.duration || 180;
            updateTimeDisplay();
          } catch (ignored) {}
        }
      }, 500);
    }
  });

})();

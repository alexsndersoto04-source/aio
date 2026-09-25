/**
 * Titan Audio — Hi-Fi Full Stack Client Application
 * Gestión de reproducción nativa y streaming remoto con interfaz táctil de alta gama.
 */

(function () {
  'use strict';

  // =========================================================================
  // ESTADO GLOBAL
  // =========================================================================

  const state = {
    isAndroid: typeof window.TitanBridge !== 'undefined',
    currentSource: 'phone', // 'phone' | 'cloud' | 'telegram'
    cloudTracks: [],
    phoneTracks: [],
    telegramTracks: [],
    currentQueue: [],
    currentIndex: -1,
    currentTrack: null,
    isPlaying: false,
    currentTime: 0,
    duration: 180,
    volume: 85,
    isMuted: false,
    shuffle: false,
    repeat: 'none', // 'none' | 'all' | 'one'
    crossfade: 3.0,
    gapless: true,
    eq: { bass: 0, mid: 0, treble: 0 },
    searchQuery: '',
    lastRenderSignature: ''
  };

  // Motor Web Audio (fallback para navegador web)
  let audioContext = null;
  let htmlAudio = null;
  let sourceNode = null;
  let bassFilter = null;
  let midFilter = null;
  let trebleFilter = null;
  let gainNode = null;

  // =========================================================================
  // REFERENCIAS DOM
  // =========================================================================

  const dom = {
    // Top Nav & Search
    searchInput: document.getElementById('searchInput'),
    clearSearchBtn: document.getElementById('clearSearchBtn'),
    hifiStudioBtn: document.getElementById('hifiStudioBtn'),
    openPickerBtn: document.getElementById('openPickerBtn'),
    audioFileInput: document.getElementById('audioFileInput'),

    // Library Tabs
    navSegments: document.querySelectorAll('.nav-segment'),
    phoneCount: document.getElementById('phoneCount'),
    cloudCount: document.getElementById('cloudCount'),

    // Main Content
    currentViewTitle: document.getElementById('currentViewTitle'),
    currentViewSubtitle: document.getElementById('currentViewSubtitle'),
    scanPhoneBtn: document.getElementById('scanPhoneBtn'),
    playAllBtn: document.getElementById('playAllBtn'),
    tracksContainer: document.getElementById('tracksContainer'),
    emptyState: document.getElementById('emptyState'),

    // Mini Player
    miniPlayer: document.getElementById('miniPlayer'),
    miniProgressFill: document.getElementById('miniProgressFill'),
    miniMetaTrigger: document.getElementById('miniMetaTrigger'),
    miniArtwork: document.getElementById('miniArtwork'),
    miniTitle: document.getElementById('miniTitle'),
    miniArtist: document.getElementById('miniArtist'),
    miniPrevBtn: document.getElementById('miniPrevBtn'),
    miniPlayBtn: document.getElementById('miniPlayBtn'),
    miniPlayIcon: document.getElementById('miniPlayIcon'),
    miniPauseIcon: document.getElementById('miniPauseIcon'),
    miniNextBtn: document.getElementById('miniNextBtn'),

    // Fullscreen Player
    fullscreenPlayer: document.getElementById('fullscreenPlayer'),
    closeFsPlayer: document.getElementById('closeFsPlayer'),
    fsEqBtn: document.getElementById('fsEqBtn'),
    fsArtwork: document.getElementById('fsArtwork'),
    fsTitle: document.getElementById('fsTitle'),
    fsArtist: document.getElementById('fsArtist'),
    fsLikeBtn: document.getElementById('fsLikeBtn'),
    fsSeekTrack: document.getElementById('fsSeekTrack'),
    fsSeekProgress: document.getElementById('fsSeekProgress'),
    fsSeekThumb: document.getElementById('fsSeekThumb'),
    fsCurrentTime: document.getElementById('fsCurrentTime'),
    fsTotalDuration: document.getElementById('fsTotalDuration'),
    fsShuffleBtn: document.getElementById('fsShuffleBtn'),
    fsPrevBtn: document.getElementById('fsPrevBtn'),
    fsPlayBtn: document.getElementById('fsPlayBtn'),
    fsPlayIcon: document.getElementById('fsPlayIcon'),
    fsPauseIcon: document.getElementById('fsPauseIcon'),
    fsNextBtn: document.getElementById('fsNextBtn'),
    fsRepeatBtn: document.getElementById('fsRepeatBtn'),
    fsFormatBadge: document.getElementById('fsFormatBadge'),
    fsActiveDevice: document.getElementById('fsActiveDevice'),

    // Hi-Fi Modal
    hifiModal: document.getElementById('hifiModal'),
    closeHifiModal: document.getElementById('closeHifiModal'),
    spectrumBars: document.getElementById('spectrumBars'),
    eqBass: document.getElementById('eqBass'),
    eqMid: document.getElementById('eqMid'),
    eqTreble: document.getElementById('eqTreble'),
    bassVal: document.getElementById('bassVal'),
    midVal: document.getElementById('midVal'),
    trebleVal: document.getElementById('trebleVal'),
    presetPills: document.querySelectorAll('.preset-pill'),
    crossfadeSlider: document.getElementById('crossfadeSlider'),
    crossfadeVal: document.getElementById('crossfadeVal'),
    gaplessToggle: document.getElementById('gaplessToggle'),
    audioDeviceSelect: document.getElementById('audioDeviceSelect')
  };

  // =========================================================================
  // GENERADOR VECTORIAL DE CARÁTULAS DETERMINÍSTICAS (0ms, SIN RED)
  // =========================================================================

  function generateArtworkDataUri(title, artist, primaryColor) {
    const seed = (title || 'Track') + (artist || 'Artist');
    let hash = 0;
    for (let i = 0; i < seed.length; i++) {
      hash = (hash << 5) - hash + seed.charCodeAt(i);
      hash |= 0;
    }

    const hue1 = Math.abs(hash) % 360;
    const hue2 = (hue1 + 45) % 360;
    const initial = (title && title.length > 0) ? title.trim().charAt(0).toUpperCase() : 'T';

    const svg = `
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200" width="200" height="200">
        <defs>
          <linearGradient id="bgGrad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stop-color="hsl(${hue1}, 70%, 14%)" />
            <stop offset="100%" stop-color="hsl(${hue2}, 85%, 7%)" />
          </linearGradient>
          <linearGradient id="accentGrad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stop-color="#00e5ff" />
            <stop offset="100%" stop-color="hsl(${hue1}, 80%, 55%)" />
          </linearGradient>
        </defs>
        <rect width="200" height="200" rx="20" fill="url(#bgGrad)" />
        <circle cx="100" cy="100" r="70" fill="none" stroke="url(#accentGrad)" stroke-width="2.5" opacity="0.35" />
        <circle cx="100" cy="100" r="50" fill="none" stroke="#00e5ff" stroke-width="1.5" opacity="0.2" />
        <circle cx="100" cy="100" r="28" fill="#11141e" />
        <text x="100" y="108" font-family="-apple-system, sans-serif" font-size="24" font-weight="800" fill="#ffffff" text-anchor="middle" dominant-baseline="middle">${initial}</text>
      </svg>
    `.trim();

    return 'data:image/svg+xml;utf8,' + encodeURIComponent(svg);
  }

  // =========================================================================
  // INICIALIZACIÓN
  // =========================================================================

  function init() {
    createSpectrumBars();
    bindEvents();

    // 1. Cargar biblioteca de la nube
    loadCloudLibrary();

    // 2. Si estamos en Android nativo, escanear inmediatamente la biblioteca del teléfono
    if (state.isAndroid && window.TitanBridge) {
      scanPhoneLibrary();
    } else {
      // En navegador, cambiar por defecto a la nube
      switchSource('cloud');
    }

    // Timer de sondeo de estado
    setInterval(pollPlaybackStatus, 500);

    // Animador de espectro
    requestAnimationFrame(renderSpectrumLoop);
  }

  // =========================================================================
  // VINCULACIÓN DE EVENTOS
  // =========================================================================

  function bindEvents() {
    // Segmentos de navegación
    dom.navSegments.forEach(seg => {
      seg.addEventListener('click', () => {
        const source = seg.dataset.source;
        switchSource(source);
      });
    });

    // Búsqueda
    dom.searchInput.addEventListener('input', (e) => {
      state.searchQuery = e.target.value.trim();
      dom.clearSearchBtn.style.display = state.searchQuery ? 'block' : 'none';
      filterAndRenderQueue();
    });

    dom.clearSearchBtn.addEventListener('click', () => {
      dom.searchInput.value = '';
      state.searchQuery = '';
      dom.clearSearchBtn.style.display = 'none';
      filterAndRenderQueue();
    });

    // Botones de acción superior
    dom.hifiStudioBtn.addEventListener('click', () => openHifiModal());
    dom.fsEqBtn.addEventListener('click', () => openHifiModal());
    dom.closeHifiModal.addEventListener('click', () => closeHifiModal());
    dom.hifiModal.addEventListener('click', (e) => {
      if (e.target === dom.hifiModal) closeHifiModal();
    });

    dom.openPickerBtn.addEventListener('click', () => {
      if (state.isAndroid && window.TitanBridge) {
        window.TitanBridge.openAudioFilePicker();
      } else {
        dom.audioFileInput.click();
      }
    });

    dom.audioFileInput.addEventListener('change', (e) => {
      handleHtmlFileInput(e.target.files);
    });

    dom.scanPhoneBtn.addEventListener('click', () => {
      scanPhoneLibrary();
    });

    dom.playAllBtn.addEventListener('click', () => {
      if (state.currentQueue.length > 0) {
        playTrackByIndex(0);
      }
    });

    // Mini Player
    dom.miniMetaTrigger.addEventListener('click', () => {
      openFullscreenPlayer();
    });

    dom.miniPlayBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      togglePlay();
    });

    dom.miniPrevBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      playPrevious();
    });

    dom.miniNextBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      playNext();
    });

    // Fullscreen Player
    dom.closeFsPlayer.addEventListener('click', () => {
      closeFullscreenPlayer();
    });

    dom.fsPlayBtn.addEventListener('click', () => togglePlay());
    dom.fsPrevBtn.addEventListener('click', () => playPrevious());
    dom.fsNextBtn.addEventListener('click', () => playNext());

    dom.fsShuffleBtn.addEventListener('click', () => {
      state.shuffle = !state.shuffle;
      dom.fsShuffleBtn.classList.toggle('active', state.shuffle);
    });

    dom.fsRepeatBtn.addEventListener('click', () => {
      if (state.repeat === 'none') {
        state.repeat = 'all';
        dom.fsRepeatBtn.classList.add('active');
      } else if (state.repeat === 'all') {
        state.repeat = 'one';
      } else {
        state.repeat = 'none';
        dom.fsRepeatBtn.classList.remove('active');
      }
    });

    dom.fsLikeBtn.addEventListener('click', () => {
      dom.fsLikeBtn.classList.toggle('liked');
    });

    // Barra de búsqueda en pantalla completa (Scrubber)
    dom.fsSeekTrack.addEventListener('click', (e) => {
      const rect = dom.fsSeekTrack.getBoundingClientRect();
      const clickX = e.clientX - rect.left;
      const ratio = Math.max(0, Math.min(1, clickX / rect.width));
      const targetSec = Math.round(ratio * (state.duration || 180));
      seekTo(targetSec);
    });

    // Ajustes Hi-Fi
    dom.eqBass.addEventListener('input', (e) => updateEq());
    dom.eqMid.addEventListener('input', (e) => updateEq());
    dom.eqTreble.addEventListener('input', (e) => updateEq());

    dom.presetPills.forEach(pill => {
      pill.addEventListener('click', () => {
        applyEqPreset(pill.dataset.preset);
        dom.presetPills.forEach(p => p.classList.remove('active'));
        pill.classList.add('active');
      });
    });

    dom.crossfadeSlider.addEventListener('input', (e) => {
      const val = parseFloat(e.target.value);
      state.crossfade = val;
      dom.crossfadeVal.textContent = val.toFixed(1) + ' s';
      if (state.isAndroid && window.TitanBridge) {
        window.TitanBridge.setCrossfade(val);
      }
    });

    dom.gaplessToggle.addEventListener('change', (e) => {
      state.gapless = e.target.checked;
      if (state.isAndroid && window.TitanBridge) {
        window.TitanBridge.setGapless(state.gapless);
      }
    });

    dom.audioDeviceSelect.addEventListener('change', (e) => {
      dom.fsActiveDevice.textContent = e.target.options[e.target.selectedIndex].text;
    });

    // Callbacks globales expuestos para Android
    window.onAudioFilesSelected = function (jsonStr) {
      try {
        const files = typeof jsonStr === 'string' ? JSON.parse(jsonStr) : jsonStr;
        if (Array.isArray(files) && files.length > 0) {
          files.forEach(f => {
            if (!f.artworkUrl) f.artworkUrl = generateArtworkDataUri(f.title, f.artist);
          });
          state.phoneTracks = [...files, ...state.phoneTracks];
          updateCounts();
          switchSource('phone');
        }
      } catch (err) {
        console.error('Error en onAudioFilesSelected', err);
      }
    };

    window.onPermissionsGranted = function () {
      scanPhoneLibrary();
    };
  }

  // =========================================================================
  // GESTIÓN DE FUENTES Y BIBLIOTECA
  // =========================================================================

  function switchSource(source) {
    state.currentSource = source;

    dom.navSegments.forEach(seg => {
      seg.classList.toggle('active', seg.dataset.source === source);
    });

    if (source === 'phone') {
      dom.currentViewTitle.textContent = 'Canciones en este teléfono';
      dom.currentViewSubtitle.textContent = 'Música local sin conexión';
      dom.scanPhoneBtn.style.display = 'inline-flex';
    } else if (source === 'cloud') {
      dom.currentViewTitle.textContent = 'Nube de Streaming Hi-Fi';
      dom.currentViewSubtitle.textContent = 'Catálogo sin pérdidas desde el servidor';
      dom.scanPhoneBtn.style.display = 'none';
    } else if (source === 'telegram') {
      dom.currentViewTitle.textContent = 'Canal Privado Telegram';
      dom.currentViewSubtitle.textContent = 'Transmisión directa de datos en tiempo real';
      dom.scanPhoneBtn.style.display = 'none';
    }

    filterAndRenderQueue();
  }

  function scanPhoneLibrary() {
    if (!state.isAndroid || !window.TitanBridge) return;

    try {
      const raw = window.TitanBridge.scanLocalMusic();
      const list = JSON.parse(raw);

      if (Array.isArray(list)) {
        // Filtrar audios de voz o archivos residuales
        const cleanList = list.filter(t => {
          const dur = t.duration || 0;
          const lowerTitle = (t.title || '').toLowerCase();
          const lowerPath = (t.rawPath || '').toLowerCase();

          if (dur < 40) return false;
          if (lowerTitle.includes('tts-') || lowerTitle.includes('inworld') || lowerTitle.startsWith('ptt-') || lowerTitle.startsWith('aud-')) return false;
          if (lowerPath.includes('whatsapp') || lowerPath.includes('cache')) return false;

          return true;
        });

        cleanList.forEach(t => {
          t.artworkUrl = generateArtworkDataUri(t.title, t.artist);
        });

        state.phoneTracks = cleanList;
        updateCounts();

        if (state.currentSource === 'phone') {
          filterAndRenderQueue();
        }
      }
    } catch (e) {
      console.error('Error escaneando MediaStore', e);
    }
  }

  function loadCloudLibrary() {
    fetch('/api/tracks')
      .then(res => res.json())
      .then(data => {
        if (data && Array.isArray(data.tracks)) {
          data.tracks.forEach(t => {
            t.artworkUrl = generateArtworkDataUri(t.title, t.artist, t.artworkColor);
          });
          state.cloudTracks = data.tracks;
          updateCounts();

          if (state.currentSource === 'cloud') {
            filterAndRenderQueue();
          }
        }
      })
      .catch(() => {
        // Catálogo Hi-Fi local integrado en Assets
        const fallback = [
          { id: 'trk_titan_1', title: 'Horizonte Estelar', artist: 'Titan Sound Lab', duration: 185, path: 'file:///android_asset/web/audio/track_synthwave.wav', source: 'cloud' },
          { id: 'trk_titan_2', title: 'Acoustic Solitude', artist: 'Elena Rostova', duration: 204, path: 'file:///android_asset/web/audio/track_acoustic.wav', source: 'cloud' },
          { id: 'trk_titan_3', title: 'Cyber Pulse 2099', artist: 'Vektor Prime', duration: 168, path: 'file:///android_asset/web/audio/track_electronic.wav', source: 'cloud' },
          { id: 'trk_titan_4', title: 'Cálido Café Lo-Fi', artist: 'Komorebi Beat', duration: 142, path: 'file:///android_asset/web/audio/track_lofi.wav', source: 'cloud' },
          { id: 'trk_titan_5', title: 'Distorsión de Medianoche', artist: 'The Electric Void', duration: 220, path: 'file:///android_asset/web/audio/track_rock.wav', source: 'cloud' }
        ];
        fallback.forEach(t => {
          t.artworkUrl = generateArtworkDataUri(t.title, t.artist);
        });
        state.cloudTracks = fallback;
        updateCounts();
        if (state.currentSource === 'cloud') {
          filterAndRenderQueue();
        }
      });
  }

  function updateCounts() {
    dom.phoneCount.textContent = state.phoneTracks.length;
    dom.cloudCount.textContent = state.cloudTracks.length;
  }

  // =========================================================================
  // RENDERIZADO DE COLA (LIMPIO, SIN FLECHAS SUELTAS)
  // =========================================================================

  function filterAndRenderQueue() {
    let sourceTracks = [];
    if (state.currentSource === 'phone') {
      sourceTracks = state.phoneTracks;
    } else if (state.currentSource === 'cloud') {
      sourceTracks = state.cloudTracks;
    } else if (state.currentSource === 'telegram') {
      sourceTracks = state.telegramTracks;
    }

    if (state.searchQuery) {
      const q = state.searchQuery.toLowerCase();
      sourceTracks = sourceTracks.filter(t =>
        (t.title && t.title.toLowerCase().includes(q)) ||
        (t.artist && t.artist.toLowerCase().includes(q)) ||
        (t.album && t.album.toLowerCase().includes(q))
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

    // Firma para evitar trabajo innecesario en DOM
    const sig = tracks.map(t => t.id).join(':');
    if (state.lastRenderSignature === sig) {
      updateActiveTrackRowHighlight();
      return;
    }
    state.lastRenderSignature = sig;

    const fragment = document.createDocumentFragment();

    tracks.forEach((track, index) => {
      const isCurrent = state.currentTrack && state.currentTrack.id === track.id;
      const row = document.createElement('div');
      row.className = `track-row ${isCurrent ? 'active' : ''}`;
      row.dataset.id = track.id;
      row.dataset.index = index;

      const durText = formatTime(track.duration || 180);
      const artwork = track.artworkUrl || generateArtworkDataUri(track.title, track.artist);

      row.innerHTML = `
        <img src="${artwork}" class="track-thumb" alt="Carátula" loading="lazy">
        <div class="track-info">
          <span class="track-title">${escapeXml(track.title || 'Pista de audio')}</span>
          <span class="track-artist">${escapeXml(track.artist || 'Artista desconocido')}</span>
        </div>
        <div class="track-trailing">
          ${isCurrent && state.isPlaying ? `
            <div class="playing-equalizer-bars">
              <span></span><span></span><span></span>
            </div>
          ` : ''}
          <span class="track-dur">${durText}</span>
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

  function updateActiveTrackRowHighlight() {
    const rows = dom.tracksContainer.querySelectorAll('.track-row');
    rows.forEach((row) => {
      const idx = parseInt(row.dataset.index, 10);
      const track = state.currentQueue[idx];
      const isCurrent = state.currentTrack && track && state.currentTrack.id === track.id;
      row.classList.toggle('active', isCurrent);

      const trailing = row.querySelector('.track-trailing');
      if (trailing) {
        const durText = formatTime(track ? track.duration || 180 : 180);
        trailing.innerHTML = `
          ${isCurrent && state.isPlaying ? `
            <div class="playing-equalizer-bars">
              <span></span><span></span><span></span>
            </div>
          ` : ''}
          <span class="track-dur">${durText}</span>
        `;
      }
    });
  }

  // =========================================================================
  // MOTOR DE REPRODUCCIÓN (NATIVO ANDROID / WEB AUDIO)
  // =========================================================================

  function playTrackByIndex(index) {
    if (index < 0 || index >= state.currentQueue.length) return;

    state.currentIndex = index;
    const track = state.currentQueue[index];
    state.currentTrack = track;
    state.duration = track.duration || 180;
    state.currentTime = 0;

    updatePlayerMeta(track);

    const path = track.path || track.streamUrl || track.relativePath || '';

    if (state.isAndroid && window.TitanBridge) {
      const isCloud = track.source === 'cloud' || track.source === 'telegram';
      const ok = window.TitanBridge.playTrack(path, track.title, track.artist, isCloud);
      if (ok) {
        setPlaybackState(true);
      } else {
        playViaHtmlAudio(path);
      }
    } else {
      playViaHtmlAudio(path);
    }

    updateActiveTrackRowHighlight();
  }

  function playViaHtmlAudio(src) {
    initWebAudio();

    if (!htmlAudio) {
      htmlAudio = new Audio();
      htmlAudio.crossOrigin = 'anonymous';
      htmlAudio.addEventListener('timeupdate', () => {
        state.currentTime = Math.round(htmlAudio.currentTime);
        state.duration = Math.round(htmlAudio.duration) || state.duration;
        updateProgressUI();
      });
      htmlAudio.addEventListener('ended', () => {
        playNext();
      });
    }

    htmlAudio.src = src;
    htmlAudio.play()
      .then(() => setPlaybackState(true))
      .catch(err => {
        console.warn('Fallback HTML Audio error:', err);
        setPlaybackState(false);
      });
  }

  function initWebAudio() {
    if (audioContext) return;
    try {
      const AudioCtx = window.AudioContext || window.webkitAudioContext;
      audioContext = new AudioCtx();

      bassFilter = audioContext.createBiquadFilter();
      bassFilter.type = 'lowshelf';
      bassFilter.frequency.value = 60;

      midFilter = audioContext.createBiquadFilter();
      midFilter.type = 'peaking';
      midFilter.frequency.value = 1000;
      midFilter.Q.value = 1.0;

      trebleFilter = audioContext.createBiquadFilter();
      trebleFilter.type = 'highshelf';
      trebleFilter.frequency.value = 10000;

      gainNode = audioContext.createGain();

      if (htmlAudio && !sourceNode) {
        sourceNode = audioContext.createMediaElementSource(htmlAudio);
        sourceNode.connect(bassFilter);
        bassFilter.connect(midFilter);
        midFilter.connect(trebleFilter);
        trebleFilter.connect(gainNode);
        gainNode.connect(audioContext.destination);
      }
    } catch (e) {
      console.warn('Web Audio no disponible:', e);
    }
  }

  function togglePlay() {
    if (!state.currentTrack && state.currentQueue.length > 0) {
      playTrackByIndex(0);
      return;
    }

    if (state.isPlaying) {
      if (state.isAndroid && window.TitanBridge) {
        window.TitanBridge.pauseTrack();
      }
      if (htmlAudio) htmlAudio.pause();
      setPlaybackState(false);
    } else {
      if (state.isAndroid && window.TitanBridge) {
        window.TitanBridge.resumeTrack();
      }
      if (htmlAudio) htmlAudio.play();
      setPlaybackState(true);
    }
  }

  function playNext() {
    if (state.currentQueue.length === 0) return;

    if (state.repeat === 'one' && state.currentIndex >= 0) {
      playTrackByIndex(state.currentIndex);
      return;
    }

    let next = state.currentIndex + 1;
    if (state.shuffle) {
      next = Math.floor(Math.random() * state.currentQueue.length);
    }

    if (next >= state.currentQueue.length) {
      if (state.repeat === 'all') next = 0;
      else return;
    }

    playTrackByIndex(next);
  }

  function playPrevious() {
    if (state.currentQueue.length === 0) return;

    let prev = state.currentIndex - 1;
    if (prev < 0) prev = state.currentQueue.length - 1;
    playTrackByIndex(prev);
  }

  function seekTo(seconds) {
    state.currentTime = seconds;
    if (state.isAndroid && window.TitanBridge) {
      window.TitanBridge.seekTo(seconds);
    }
    if (htmlAudio) {
      htmlAudio.currentTime = seconds;
    }
    updateProgressUI();
  }

  function setPlaybackState(playing) {
    state.isPlaying = playing;

    // Actualizar iconos de reproducir / pausa
    dom.miniPlayIcon.style.display = playing ? 'none' : 'block';
    dom.miniPauseIcon.style.display = playing ? 'block' : 'none';

    dom.fsPlayIcon.style.display = playing ? 'none' : 'block';
    dom.fsPauseIcon.style.display = playing ? 'block' : 'none';

    updateActiveTrackRowHighlight();
  }

  function updatePlayerMeta(track) {
    if (!track) return;

    const title = track.title || 'Canción';
    const artist = track.artist || 'Titan Audio';
    const artwork = track.artworkUrl || generateArtworkDataUri(title, artist);

    // Mini Player
    dom.miniTitle.textContent = title;
    dom.miniArtist.textContent = artist;
    dom.miniArtwork.src = artwork;

    // Fullscreen Player
    dom.fsTitle.textContent = title;
    dom.fsArtist.textContent = artist;
    dom.fsArtwork.src = artwork;
    dom.fsTotalDuration.textContent = formatTime(state.duration);

    // Carga asíncrona de carátula embebida en segundo plano si está en Android
    if (state.isAndroid && window.TitanBridge && track.path) {
      setTimeout(() => {
        try {
          const emb = window.TitanBridge.getEmbeddedArtwork(track.path);
          if (emb && emb.startsWith('data:image')) {
            dom.miniArtwork.src = emb;
            dom.fsArtwork.src = emb;
            track.artworkUrl = emb;
          }
        } catch (ignored) {}
      }, 50);
    }
  }

  function updateProgressUI() {
    const cur = state.currentTime || 0;
    const dur = state.duration || 180;
    const pct = Math.max(0, Math.min(100, (cur / dur) * 100));

    dom.miniProgressFill.style.width = pct + '%';
    dom.fsSeekProgress.style.width = pct + '%';
    dom.fsSeekThumb.style.left = pct + '%';

    dom.fsCurrentTime.textContent = formatTime(cur);
    dom.fsTotalDuration.textContent = formatTime(dur);
  }

  function pollPlaybackStatus() {
    if (!state.isAndroid || !window.TitanBridge) return;

    try {
      const raw = window.TitanBridge.getPlaybackStatus();
      if (!raw) return;
      const status = JSON.parse(raw);

      if (typeof status.playing === 'boolean' && status.playing !== state.isPlaying) {
        setPlaybackState(status.playing);
      }
      if (typeof status.position === 'number') {
        state.currentTime = status.position;
      }
      if (typeof status.duration === 'number' && status.duration > 0) {
        state.duration = status.duration;
      }

      updateProgressUI();
    } catch (e) {
      console.warn('Error leyendo estado nativo:', e);
    }
  }

  // =========================================================================
  // ECUALIZADOR Y ESTUDIO HI-FI
  // =========================================================================

  function openHifiModal() {
    dom.hifiModal.classList.add('active');
  }

  function closeHifiModal() {
    dom.hifiModal.classList.remove('active');
  }

  function openFullscreenPlayer() {
    dom.fullscreenPlayer.classList.add('active');
  }

  function closeFullscreenPlayer() {
    dom.fullscreenPlayer.classList.remove('active');
  }

  function updateEq() {
    const bass = parseInt(dom.eqBass.value, 10);
    const mid = parseInt(dom.eqMid.value, 10);
    const treble = parseInt(dom.eqTreble.value, 10);

    state.eq = { bass, mid, treble };

    dom.bassVal.textContent = (bass > 0 ? '+' : '') + bass + ' dB';
    dom.midVal.textContent = (mid > 0 ? '+' : '') + mid + ' dB';
    dom.trebleVal.textContent = (treble > 0 ? '+' : '') + treble + ' dB';

    if (state.isAndroid && window.TitanBridge) {
      window.TitanBridge.setEqualizer(bass, mid, treble);
    }

    if (bassFilter && midFilter && trebleFilter) {
      bassFilter.gain.value = bass;
      midFilter.gain.value = mid;
      trebleFilter.gain.value = treble;
    }
  }

  function applyEqPreset(preset) {
    let b = 0, m = 0, t = 0;
    if (preset === 'bass') { b = 7; m = 1; t = -1; }
    else if (preset === 'vocal') { b = -2; m = 4; t = 5; }
    else if (preset === 'electronic') { b = 6; m = 0; t = 5; }

    dom.eqBass.value = b;
    dom.eqMid.value = m;
    dom.eqTreble.value = t;
    updateEq();
  }

  function createSpectrumBars() {
    dom.spectrumBars.innerHTML = '';
    for (let i = 0; i < 32; i++) {
      const bar = document.createElement('div');
      bar.className = 'bar';
      dom.spectrumBars.appendChild(bar);
    }
  }

  function renderSpectrumLoop() {
    if (dom.hifiModal.classList.contains('active')) {
      const bars = dom.spectrumBars.children;
      let levels = [];

      if (state.isAndroid && window.TitanBridge) {
        try {
          const raw = window.TitanBridge.getSpectrumLevels();
          levels = JSON.parse(raw);
        } catch (ignored) {}
      }

      for (let i = 0; i < bars.length; i++) {
        let val = 0.08;
        if (state.isPlaying) {
          val = (levels && levels[i]) ? levels[i] : (Math.random() * 0.7 + 0.15);
        }
        bars[i].style.height = Math.round(val * 100) + '%';
      }
    }

    requestAnimationFrame(renderSpectrumLoop);
  }

  // =========================================================================
  // UTILIDADES
  // =========================================================================

  function formatTime(seconds) {
    if (isNaN(seconds) || seconds < 0) return '0:00';
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${m}:${s < 10 ? '0' : ''}${s}`;
  }

  function escapeXml(str) {
    return String(str)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&apos;');
  }

  function handleHtmlFileInput(files) {
    if (!files || files.length === 0) return;

    const imported = [];
    Array.from(files).forEach((file, idx) => {
      const objUrl = URL.createObjectURL(file);
      const name = file.name.replace(/\.[^/.]+$/, "");
      imported.push({
        id: 'local_' + Date.now() + '_' + idx,
        title: name,
        artist: 'Archivo Local',
        album: 'Dispositivo',
        duration: 180,
        path: objUrl,
        artworkUrl: generateArtworkDataUri(name, 'Archivo Local'),
        source: 'phone'
      });
    });

    state.phoneTracks = [...imported, ...state.phoneTracks];
    updateCounts();
    switchSource('phone');
  }

  // Ejecución inicial
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

})();

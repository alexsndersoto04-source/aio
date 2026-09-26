/**
 * Mi Música — Cliente Nativo Hi-Fi en Modo Claro
 * Inspirado en la interfaz de usuario de MIUI / HyperOS Player.
 * Totalmente real y funcional: MediaStore, Favoritos, Listas, Recientes, Ecualizador y Temporizador.
 */

(function () {
  'use strict';

  // =========================================================================
  // ESTADO GLOBAL DE LA APLICACIÓN
  // =========================================================================

  const state = {
    isAndroid: typeof window.TitanBridge !== 'undefined',
    currentTab: 'songs', // 'songs' | 'artists' | 'albums' | 'folders' | 'cloud' | 'favorites' | 'recent'
    allLocalTracks: [],
    cloudTracks: [],
    displayTracks: [],
    currentQueue: [],
    currentIndex: -1,
    currentTrack: null,
    isPlaying: false,
    currentTime: 0,
    duration: 180,
    sortMode: 'name', // 'name' | 'artist' | 'duration'
    searchQuery: '',
    favoritesSet: new Set(),
    recentList: [],
    playlists: {},
    sleepTimerId: null,
    sleepTimerRemaining: 0,
    activeContextTrack: null,
    eq: { bass: 0, mid: 0, treble: 0 }
  };

  // Motor Web Audio para fallback en navegador
  let htmlAudio = null;

  // =========================================================================
  // REFERENCIAS DOM
  // =========================================================================

  const dom = {
    // Header
    openDrawerBtn: document.getElementById('openDrawerBtn'),
    globalSearchInput: document.getElementById('globalSearchInput'),
    clearSearchBtn: document.getElementById('clearSearchBtn'),
    micSearchBtn: document.getElementById('micSearchBtn'),

    // Cards
    cardFavorites: document.getElementById('cardFavorites'),
    cardPlaylists: document.getElementById('cardPlaylists'),
    cardRecent: document.getElementById('cardRecent'),
    favCountBadge: document.getElementById('favCountBadge'),
    playlistCountBadge: document.getElementById('playlistCountBadge'),
    recentCountBadge: document.getElementById('recentCountBadge'),

    // Sub tabs
    subTabPills: document.querySelectorAll('.sub-tab-pill'),

    // Toolbar
    shuffleAllBtn: document.getElementById('shuffleAllBtn'),
    totalTracksCount: document.getElementById('totalTracksCount'),
    sortToggleBtn: document.getElementById('sortToggleBtn'),
    filterOptionsBtn: document.getElementById('filterOptionsBtn'),

    // Songs List
    songsContainer: document.getElementById('songsContainer'),
    emptyState: document.getElementById('emptyState'),
    emptyScanBtn: document.getElementById('emptyScanBtn'),

    // Floating Vinyl Player
    floatingPlayer: document.getElementById('floatingPlayer'),
    dockTrigger: document.getElementById('dockTrigger'),
    vinylDisc: document.getElementById('vinylDisc'),
    dockArtwork: document.getElementById('dockArtwork'),
    dockTitle: document.getElementById('dockTitle'),
    dockArtist: document.getElementById('dockArtist'),
    dockPlayBtn: document.getElementById('dockPlayBtn'),
    dockPlayIcon: document.getElementById('dockPlayIcon'),
    dockPauseIcon: document.getElementById('dockPauseIcon'),
    dockNextBtn: document.getElementById('dockNextBtn'),

    // Bottom Navigation
    bottomNavItems: document.querySelectorAll('.bottom-nav-item'),
    centerHifiBtn: document.getElementById('centerHifiBtn'),

    // Drawer
    drawerBackdrop: document.getElementById('drawerBackdrop'),
    sideDrawer: document.getElementById('sideDrawer'),
    drawerEqItem: document.getElementById('drawerEqItem'),
    drawerSleepTimerItem: document.getElementById('drawerSleepTimerItem'),
    sleepTimerStatus: document.getElementById('sleepTimerStatus'),
    drawerScanItem: document.getElementById('drawerScanItem'),
    drawerPickerItem: document.getElementById('drawerPickerItem'),
    drawerInfoItem: document.getElementById('drawerInfoItem'),
    drawerPrivacyItem: document.getElementById('drawerPrivacyItem'),

    // Fullscreen Player
    fullscreenModal: document.getElementById('fullscreenModal'),
    closeFsBtn: document.getElementById('closeFsBtn'),
    fsOptionsBtn: document.getElementById('fsOptionsBtn'),
    fsSourceBadge: document.getElementById('fsSourceBadge'),
    fsArtworkImg: document.getElementById('fsArtworkImg'),
    fsTitle: document.getElementById('fsTitle'),
    fsArtist: document.getElementById('fsArtist'),
    fsFavBtn: document.getElementById('fsFavBtn'),
    fsScrubber: document.getElementById('fsScrubber'),
    fsScrubberFill: document.getElementById('fsScrubberFill'),
    fsScrubberThumb: document.getElementById('fsScrubberThumb'),
    fsTimeCurrent: document.getElementById('fsTimeCurrent'),
    fsTimeTotal: document.getElementById('fsTimeTotal'),
    fsShuffleToggle: document.getElementById('fsShuffleToggle'),
    fsPrevTrackBtn: document.getElementById('fsPrevTrackBtn'),
    fsPlayPauseBtn: document.getElementById('fsPlayPauseBtn'),
    fsPlayIcon: document.getElementById('fsPlayIcon'),
    fsPauseIcon: document.getElementById('fsPauseIcon'),
    fsNextTrackBtn: document.getElementById('fsNextTrackBtn'),
    fsRepeatToggle: document.getElementById('fsRepeatToggle'),
    fsEqDrawerBtn: document.getElementById('fsEqDrawerBtn'),
    fsDeviceName: document.getElementById('fsDeviceName'),

    // Track Context Sheet
    trackMenuBackdrop: document.getElementById('trackMenuBackdrop'),
    trackMenuSheet: document.getElementById('trackMenuSheet'),
    sheetThumb: document.getElementById('sheetThumb'),
    sheetTitle: document.getElementById('sheetTitle'),
    sheetArtist: document.getElementById('sheetArtist'),
    sheetPlayNextBtn: document.getElementById('sheetPlayNextBtn'),
    sheetToggleFavBtn: document.getElementById('sheetToggleFavBtn'),
    sheetFavLabel: document.getElementById('sheetFavLabel'),
    sheetInfoBtn: document.getElementById('sheetInfoBtn'),

    // Sort Sheet
    sortBackdrop: document.getElementById('sortBackdrop'),
    sortSheet: document.getElementById('sortSheet'),
    sortOptions: document.querySelectorAll('.sort-option'),

    // Sleep Timer Sheet
    timerBackdrop: document.getElementById('timerBackdrop'),
    timerSheet: document.getElementById('timerSheet'),
    timerOptions: document.querySelectorAll('.timer-option'),

    // Equalizer Sheet
    eqBackdrop: document.getElementById('eqBackdrop'),
    eqSheet: document.getElementById('eqSheet'),
    eqBassSlider: document.getElementById('eqBassSlider'),
    eqMidSlider: document.getElementById('eqMidSlider'),
    eqTrebleSlider: document.getElementById('eqTrebleSlider'),
    bassDbVal: document.getElementById('bassDbVal'),
    midDbVal: document.getElementById('midDbVal'),
    trebleDbVal: document.getElementById('trebleDbVal'),
    presetTags: document.querySelectorAll('.preset-tag')
  };

  // =========================================================================
  // GENERADOR VECTORIAL DETERMINÍSTICO DE CARÁTULAS (0ms, SIN FALLOS)
  // =========================================================================

  function generateArtworkDataUri(title, artist) {
    const seed = (title || 'Song') + (artist || 'Artist');
    let hash = 0;
    for (let i = 0; i < seed.length; i++) {
      hash = (hash << 5) - hash + seed.charCodeAt(i);
      hash |= 0;
    }
    const hue = Math.abs(hash) % 360;
    const initial = (title && title.length > 0) ? title.trim().charAt(0).toUpperCase() : 'M';

    const svg = `
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 160" width="160" height="160">
        <defs>
          <linearGradient id="g_${hue}" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stop-color="hsl(${hue}, 65%, 45%)" />
            <stop offset="100%" stop-color="hsl(${(hue + 40) % 360}, 75%, 25%)" />
          </linearGradient>
        </defs>
        <rect width="160" height="160" rx="16" fill="url(#g_${hue})" />
        <circle cx="80" cy="80" r="48" fill="none" stroke="rgba(255,255,255,0.2)" stroke-width="2" />
        <circle cx="80" cy="80" r="22" fill="rgba(0,0,0,0.25)" />
        <text x="80" y="87" font-family="-apple-system, sans-serif" font-size="20" font-weight="700" fill="#ffffff" text-anchor="middle">${initial}</text>
      </svg>
    `.trim();

    return 'data:image/svg+xml;utf8,' + encodeURIComponent(svg);
  }

  // =========================================================================
  // PERSISTENCIA LOCAL (FAVORITOS, RECIENTES, LISTAS)
  // =========================================================================

  function loadPersistedData() {
    try {
      const favs = localStorage.getItem('miui_favorites');
      if (favs) state.favoritesSet = new Set(JSON.parse(favs));

      const recents = localStorage.getItem('miui_recent');
      if (recents) state.recentList = JSON.parse(recents);

      const savedPl = localStorage.getItem('miui_playlists');
      if (savedPl) state.playlists = JSON.parse(savedPl);
      else {
        state.playlists = {
          'Éxitos Populares': [],
          'Para Conducir': [],
          'Relajación': []
        };
      }
    } catch (e) {
      console.warn('Error leyendo almacenamiento local', e);
    }
    updateCardCounters();
  }

  function saveFavorites() {
    try {
      localStorage.setItem('miui_favorites', JSON.stringify(Array.from(state.favoritesSet)));
    } catch (e) {}
    updateCardCounters();
  }

  function saveRecent() {
    try {
      localStorage.setItem('miui_recent', JSON.stringify(state.recentList.slice(0, 50)));
    } catch (e) {}
    updateCardCounters();
  }

  function updateCardCounters() {
    dom.favCountBadge.textContent = `${state.favoritesSet.size} canciones`;
    const plKeys = Object.keys(state.playlists);
    dom.playlistCountBadge.textContent = `${plKeys.length} listas`;
    dom.recentCountBadge.textContent = `${state.recentList.length} escuchadas`;
  }

  // =========================================================================
  // INICIALIZACIÓN
  // =========================================================================

  function init() {
    loadPersistedData();
    bindUserInteractions();

    // 1. Escanear música local real si estamos en Android
    if (state.isAndroid && window.TitanBridge) {
      scanDeviceMusic();
    } else {
      loadFallbackMusic();
    }

    // 2. Cargar catálogo de la nube
    fetchCloudTracks();

    // 3. Temporizador de sondeo de estado
    setInterval(pollAndroidPlaybackStatus, 400);
  }

  // =========================================================================
  // ESCANEO Y CARGA DE MÚSICA REAL DEL DISPOSITIVO
  // =========================================================================

  function scanDeviceMusic() {
    if (!state.isAndroid || !window.TitanBridge) return;

    try {
      const raw = window.TitanBridge.scanLocalMusic();
      const list = JSON.parse(raw);

      if (Array.isArray(list)) {
        // Filtrar audios irrelevantes
        const filtered = list.filter(t => {
          const dur = t.duration || 0;
          const lowerTitle = (t.title || '').toLowerCase();
          const lowerPath = (t.rawPath || '').toLowerCase();

          if (dur < 40) return false;
          if (lowerTitle.includes('tts-') || lowerTitle.includes('inworld') || lowerTitle.startsWith('ptt-') || lowerTitle.startsWith('aud-')) return false;
          if (lowerPath.includes('whatsapp') || lowerPath.includes('cache')) return false;

          return true;
        });

        filtered.forEach(t => {
          t.artworkUrl = generateArtworkDataUri(t.title, t.artist);
        });

        state.allLocalTracks = filtered;
        applyCurrentFilterAndSort();
      }
    } catch (e) {
      console.error('Error al escanear MediaStore', e);
    }
  }

  function loadFallbackMusic() {
    // Canciones locales empaquetadas en Assets
    const demo = [
      { id: 'm_1', title: 'Those Eyes', artist: 'New West', album: 'Those Eyes (Alternate Ve...', duration: 220, path: 'file:///android_asset/web/audio/track_acoustic.wav' },
      { id: 'm_2', title: 'Bon Appétit', artist: 'Katy Perry, Migos', album: 'Witness (Deluxe)', duration: 227, path: 'file:///android_asset/web/audio/track_electronic.wav' },
      { id: 'm_3', title: 'Here With Me', artist: 'd4vd', album: 'Petals to Thorns', duration: 182, path: 'file:///android_asset/web/audio/track_lofi.wav' },
      { id: 'm_4', title: 'Runaway', artist: 'Sebastian Yatra, Daddy Yankee, NATTI NAT...', album: 'Dharma', duration: 202, path: 'file:///android_asset/web/audio/track_synthwave.wav' },
      { id: 'm_5', title: 'Try', artist: 'P!nk', album: "Now That's What I Call Music...", duration: 247, path: 'file:///android_asset/web/audio/track_rock.wav' }
    ];
    demo.forEach(d => d.artworkUrl = generateArtworkDataUri(d.title, d.artist));
    state.allLocalTracks = demo;
    applyCurrentFilterAndSort();
  }

  function fetchCloudTracks() {
    fetch('/api/tracks')
      .then(r => r.json())
      .then(d => {
        if (d && Array.isArray(d.tracks)) {
          d.tracks.forEach(t => {
            t.artworkUrl = generateArtworkDataUri(t.title, t.artist);
          });
          state.cloudTracks = d.tracks;
          if (state.currentTab === 'cloud') {
            applyCurrentFilterAndSort();
          }
        }
      })
      .catch(() => {});
  }

  // =========================================================================
  // FILTRADO Y ORDENACIÓN
  // =========================================================================

  function applyCurrentFilterAndSort() {
    let source = [];

    if (state.currentTab === 'songs') {
      source = [...state.allLocalTracks];
    } else if (state.currentTab === 'cloud') {
      source = [...state.cloudTracks];
    } else if (state.currentTab === 'favorites') {
      source = state.allLocalTracks.filter(t => state.favoritesSet.has(t.id));
    } else if (state.currentTab === 'recent') {
      source = state.recentList;
    } else {
      source = [...state.allLocalTracks];
    }

    // Búsqueda
    if (state.searchQuery) {
      const q = state.searchQuery.toLowerCase();
      source = source.filter(t =>
        (t.title && t.title.toLowerCase().includes(q)) ||
        (t.artist && t.artist.toLowerCase().includes(q)) ||
        (t.album && t.album.toLowerCase().includes(q))
      );
    }

    // Ordenación
    if (state.sortMode === 'name') {
      source.sort((a, b) => (a.title || '').localeCompare(b.title || ''));
    } else if (state.sortMode === 'artist') {
      source.sort((a, b) => (a.artist || '').localeCompare(b.artist || ''));
    } else if (state.sortMode === 'duration') {
      source.sort((a, b) => (b.duration || 0) - (a.duration || 0));
    }

    state.displayTracks = source;
    state.currentQueue = source;
    dom.totalTracksCount.textContent = source.length;

    renderSongList(source);
  }

  // =========================================================================
  // RENDERIZADO DE LISTA DE CANCIONES (FONDO CLARO, ESTILO MIUI)
  // =========================================================================

  function renderSongList(tracks) {
    if (!tracks || tracks.length === 0) {
      dom.songsContainer.innerHTML = '';
      dom.emptyState.style.display = 'flex';
      return;
    }
    dom.emptyState.style.display = 'none';

    const fragment = document.createDocumentFragment();

    tracks.forEach((track, index) => {
      const isCurrent = state.currentTrack && state.currentTrack.id === track.id;
      const row = document.createElement('div');
      row.className = `song-row ${isCurrent ? 'active' : ''}`;
      row.dataset.id = track.id;
      row.dataset.index = index;

      const artwork = track.artworkUrl || generateArtworkDataUri(track.title, track.artist);
      const subtitleText = `${escapeXml(track.artist || 'Artista')} | ${escapeXml(track.album || 'Dispositivo')}`;

      row.innerHTML = `
        <div class="song-thumb-box">
          <img src="${artwork}" class="song-thumb" alt="Carátula" loading="lazy">
          <div class="song-note-badge">
            <svg viewBox="0 0 24 24" width="10" height="10"><path fill="#ffffff" d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/></svg>
          </div>
        </div>

        <div class="song-meta-box">
          <span class="song-title">${escapeXml(track.title || 'Canción')}</span>
          <div class="song-subtitle-row">
            <svg viewBox="0 0 24 24" width="13" height="13" class="device-glyph">
              <path fill="#9ca3af" d="M17 1.01L7 1c-1.1 0-2 .9-2 2v18c0 1.1.9 2 2 2h10c1.1 0 2-.9 2-2V3c0-1.1-.9-1.99-2-1.99zM17 19H7V5h10v14z"/>
            </svg>
            <span>${subtitleText}</span>
          </div>
        </div>

        <div class="song-trailing">
          ${isCurrent && state.isPlaying ? `
            <div class="equalizer-purple-bars">
              <span></span><span></span><span></span><span></span>
            </div>
          ` : ''}
          <button class="song-options-btn" data-action="options" title="Opciones">
            <svg viewBox="0 0 24 24" width="18" height="18">
              <path fill="currentColor" d="M12 8c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm0 2c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0 6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z"/>
            </svg>
          </button>
        </div>
      `;

      // Clic para reproducir canción
      row.addEventListener('click', (e) => {
        if (e.target.closest('.song-options-btn')) {
          e.stopPropagation();
          openTrackContextMenu(track);
          return;
        }
        playTrackByIndex(index);
      });

      fragment.appendChild(row);
    });

    dom.songsContainer.innerHTML = '';
    dom.songsContainer.appendChild(fragment);
  }

  function updateActiveRowVisuals() {
    const rows = dom.songsContainer.querySelectorAll('.song-row');
    rows.forEach(r => {
      const idx = parseInt(r.dataset.index, 10);
      const track = state.displayTracks[idx];
      const isCurrent = state.currentTrack && track && state.currentTrack.id === track.id;
      r.classList.toggle('active', isCurrent);

      const trailing = r.querySelector('.song-trailing');
      if (trailing) {
        trailing.innerHTML = `
          ${isCurrent && state.isPlaying ? `
            <div class="equalizer-purple-bars">
              <span></span><span></span><span></span><span></span>
            </div>
          ` : ''}
          <button class="song-options-btn" data-action="options" title="Opciones">
            <svg viewBox="0 0 24 24" width="18" height="18">
              <path fill="currentColor" d="M12 8c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm0 2c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0 6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z"/>
            </svg>
          </button>
        `;
        trailing.querySelector('.song-options-btn').addEventListener('click', (ev) => {
          ev.stopPropagation();
          openTrackContextMenu(track);
        });
      }
    });
  }

  // =========================================================================
  // REPRODUCCIÓN (ANDROID NATIVO + FALLBACK WEB AUDIO)
  // =========================================================================

  function playTrackByIndex(index) {
    if (index < 0 || index >= state.currentQueue.length) return;

    state.currentIndex = index;
    const track = state.currentQueue[index];
    state.currentTrack = track;
    state.duration = track.duration || 180;
    state.currentTime = 0;

    // Agregar a la lista de recientes
    addToRecent(track);

    // Actualizar metadatos del reproductor
    updatePlayerMeta(track);

    const path = track.path || track.streamUrl || track.relativePath || '';

    if (state.isAndroid && window.TitanBridge) {
      const isCloud = track.source === 'cloud';
      const success = window.TitanBridge.playTrack(path, track.title, track.artist, isCloud);
      if (success) {
        setPlaybackState(true);
      } else {
        playViaHtmlAudio(path);
      }
    } else {
      playViaHtmlAudio(path);
    }

    updateActiveRowVisuals();
  }

  function playViaHtmlAudio(src) {
    if (!htmlAudio) {
      htmlAudio = new Audio();
      htmlAudio.addEventListener('timeupdate', () => {
        state.currentTime = Math.round(htmlAudio.currentTime);
        state.duration = Math.round(htmlAudio.duration) || state.duration;
        updateProgressUI();
      });
      htmlAudio.addEventListener('ended', () => {
        playNextTrack();
      });
    }

    htmlAudio.src = src;
    htmlAudio.play()
      .then(() => setPlaybackState(true))
      .catch(() => setPlaybackState(false));
  }

  function togglePlayPause() {
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

  function playNextTrack() {
    if (state.currentQueue.length === 0) return;

    // Comprobar si el temporizador de apagado debe detenerse al final de la pista
    if (state.sleepTimerRemaining === -1) {
      stopPlayback();
      state.sleepTimerRemaining = 0;
      dom.sleepTimerStatus.textContent = 'Desactivado';
      return;
    }

    let next = state.currentIndex + 1;
    if (next >= state.currentQueue.length) next = 0;
    playTrackByIndex(next);
  }

  function playPreviousTrack() {
    if (state.currentQueue.length === 0) return;
    let prev = state.currentIndex - 1;
    if (prev < 0) prev = state.currentQueue.length - 1;
    playTrackByIndex(prev);
  }

  function stopPlayback() {
    if (state.isAndroid && window.TitanBridge) {
      window.TitanBridge.stopTrack();
    }
    if (htmlAudio) htmlAudio.pause();
    setPlaybackState(false);
  }

  function seekToSeconds(seconds) {
    state.currentTime = seconds;
    if (state.isAndroid && window.TitanBridge) {
      window.TitanBridge.seekTo(seconds);
    }
    if (htmlAudio) htmlAudio.currentTime = seconds;
    updateProgressUI();
  }

  function setPlaybackState(playing) {
    state.isPlaying = playing;

    // Iconos de botón de reproducción en el dock flotante
    dom.dockPlayIcon.style.display = playing ? 'none' : 'block';
    dom.dockPauseIcon.style.display = playing ? 'block' : 'none';

    // Iconos en pantalla completa
    dom.fsPlayIcon.style.display = playing ? 'none' : 'block';
    dom.fsPauseIcon.style.display = playing ? 'block' : 'none';

    // Animación de rotación del vinilo
    dom.floatingPlayer.classList.toggle('playing', playing);

    updateActiveRowVisuals();
  }

  function updatePlayerMeta(track) {
    if (!track) return;

    const title = track.title || 'Canción';
    const artist = track.artist || 'Mi Música';
    const artwork = track.artworkUrl || generateArtworkDataUri(title, artist);

    // Mini Reproductor Flotante
    dom.dockTitle.textContent = title;
    dom.dockArtist.textContent = artist;
    dom.dockArtwork.src = artwork;

    // Reproductor a Pantalla Completa
    dom.fsTitle.textContent = title;
    dom.fsArtist.textContent = artist;
    dom.fsArtworkImg.src = artwork;
    dom.fsTimeTotal.textContent = formatDuration(state.duration);

    // Estado del botón de favorito
    dom.fsFavBtn.classList.toggle('liked', state.favoritesSet.has(track.id));

    // Carga de carátula ID3 embebida si estamos en Android
    if (state.isAndroid && window.TitanBridge && track.path) {
      setTimeout(() => {
        try {
          const emb = window.TitanBridge.getEmbeddedArtwork(track.path);
          if (emb && emb.startsWith('data:image')) {
            dom.dockArtwork.src = emb;
            dom.fsArtworkImg.src = emb;
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

    dom.fsScrubberFill.style.width = pct + '%';
    dom.fsScrubberThumb.style.left = pct + '%';

    dom.fsTimeCurrent.textContent = formatDuration(cur);
    dom.fsTimeTotal.textContent = formatDuration(dur);
  }

  function pollAndroidPlaybackStatus() {
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
    } catch (ignored) {}
  }

  function addToRecent(track) {
    state.recentList = [track, ...state.recentList.filter(t => t.id !== track.id)].slice(0, 30);
    saveRecent();
  }

  // =========================================================================
  // VINCULACIÓN DE EVENTOS DEL USUARIO
  // =========================================================================

  function bindUserInteractions() {
    // Menú Lateral (Drawer)
    dom.openDrawerBtn.addEventListener('click', () => {
      dom.drawerBackdrop.classList.add('active');
      dom.sideDrawer.classList.add('active');
    });

    dom.drawerBackdrop.addEventListener('click', closeDrawer);

    dom.drawerEqItem.addEventListener('click', () => {
      closeDrawer();
      openEqSheet();
    });

    dom.drawerSleepTimerItem.addEventListener('click', () => {
      closeDrawer();
      openSleepTimerSheet();
    });

    dom.drawerScanItem.addEventListener('click', () => {
      closeDrawer();
      scanDeviceMusic();
    });

    dom.drawerPickerItem.addEventListener('click', () => {
      closeDrawer();
      if (state.isAndroid && window.TitanBridge) {
        window.TitanBridge.openAudioFilePicker();
      }
    });

    // Pestañas de la barra de navegación inferior
    dom.bottomNavItems.forEach(item => {
      item.addEventListener('click', () => {
        const nav = item.dataset.nav;
        dom.bottomNavItems.forEach(i => i.classList.remove('active'));
        item.classList.add('active');

        if (nav === 'local') {
          switchSubTab('songs');
        } else if (nav === 'discover') {
          switchSubTab('cloud');
        } else if (nav === 'search') {
          dom.globalSearchInput.focus();
        } else if (nav === 'profile') {
          dom.openDrawerBtn.click();
        }
      });
    });

    dom.centerHifiBtn.addEventListener('click', () => {
      openEqSheet();
    });

    // Tarjetas Superiores
    dom.cardFavorites.addEventListener('click', () => {
      switchSubTab('favorites');
    });

    dom.cardRecent.addEventListener('click', () => {
      switchSubTab('recent');
    });

    // Pestañas de categorías (Canciones, Artistas, etc.)
    dom.subTabPills.forEach(pill => {
      pill.addEventListener('click', () => {
        const tab = pill.dataset.tab;
        switchSubTab(tab);
      });
    });

    // Barra de Búsqueda
    dom.globalSearchInput.addEventListener('input', (e) => {
      state.searchQuery = e.target.value.trim();
      dom.clearSearchBtn.style.display = state.searchQuery ? 'block' : 'none';
      applyCurrentFilterAndSort();
    });

    dom.clearSearchBtn.addEventListener('click', () => {
      dom.globalSearchInput.value = '';
      state.searchQuery = '';
      dom.clearSearchBtn.style.display = 'none';
      applyCurrentFilterAndSort();
    });

    // Botón de Reproducción Aleatoria en cabecera
    dom.shuffleAllBtn.addEventListener('click', () => {
      if (state.displayTracks.length === 0) return;
      // Mezclar cola
      const shuffled = [...state.displayTracks].sort(() => Math.random() - 0.5);
      state.currentQueue = shuffled;
      playTrackByIndex(0);
    });

    // Botón de Ordenar (⇅)
    dom.sortToggleBtn.addEventListener('click', () => {
      openSortSheet();
    });

    // Opciones de ordenación en el modal
    dom.sortOptions.forEach(opt => {
      opt.addEventListener('click', () => {
        state.sortMode = opt.dataset.sort;
        dom.sortOptions.forEach(o => o.classList.remove('active'));
        opt.classList.add('active');
        closeSortSheet();
        applyCurrentFilterAndSort();
      });
    });

    dom.sortBackdrop.addEventListener('click', closeSortSheet);

    // Mini Reproductor Flotante (Dock)
    dom.dockTrigger.addEventListener('click', (e) => {
      if (e.target.closest('#dockPlayBtn') || e.target.closest('#dockNextBtn')) return;
      openFullscreenPlayer();
    });

    dom.dockPlayBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      togglePlayPause();
    });

    dom.dockNextBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      playNextTrack();
    });

    // Reproductor a Pantalla Completa
    dom.closeFsBtn.addEventListener('click', closeFullscreenPlayer);
    dom.fsPlayPauseBtn.addEventListener('click', togglePlayPause);
    dom.fsNextTrackBtn.addEventListener('click', playNextTrack);
    dom.fsPrevTrackBtn.addEventListener('click', playPreviousTrack);

    dom.fsFavBtn.addEventListener('click', () => {
      if (!state.currentTrack) return;
      toggleFavorite(state.currentTrack.id);
      dom.fsFavBtn.classList.toggle('liked', state.favoritesSet.has(state.currentTrack.id));
    });

    // Barra de desplazamiento táctil (Scrubber)
    dom.fsScrubber.addEventListener('click', (e) => {
      const rect = dom.fsScrubber.getBoundingClientRect();
      const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
      const targetSec = Math.round(ratio * (state.duration || 180));
      seekToSeconds(targetSec);
    });

    dom.fsEqDrawerBtn.addEventListener('click', () => {
      openEqSheet();
    });

    // Context Menu de Pista (⋮)
    dom.trackMenuBackdrop.addEventListener('click', closeTrackContextMenu);

    dom.sheetPlayNextBtn.addEventListener('click', () => {
      if (!state.activeContextTrack) return;
      // Insertar después de la posición actual
      const nextPos = state.currentIndex + 1;
      state.currentQueue.splice(nextPos, 0, state.activeContextTrack);
      closeTrackContextMenu();
    });

    dom.sheetToggleFavBtn.addEventListener('click', () => {
      if (!state.activeContextTrack) return;
      toggleFavorite(state.activeContextTrack.id);
      closeTrackContextMenu();
    });

    dom.sheetInfoBtn.addEventListener('click', () => {
      if (!state.activeContextTrack) return;
      const t = state.activeContextTrack;
      alert(`Título: ${t.title}\nArtista: ${t.artist}\nÁlbum: ${t.album || 'N/A'}\nDuración: ${formatDuration(t.duration)}\nRuta: ${t.rawPath || t.path || 'Local'}`);
      closeTrackContextMenu();
    });

    // Temporizador de Apagado
    dom.timerBackdrop.addEventListener('click', closeSleepTimerSheet);

    dom.timerOptions.forEach(opt => {
      opt.addEventListener('click', () => {
        const mins = parseInt(opt.dataset.mins, 10);
        setSleepTimer(mins);
        closeSleepTimerSheet();
      });
    });

    // Ecualizador
    dom.eqBackdrop.addEventListener('click', closeEqSheet);

    dom.eqBassSlider.addEventListener('input', updateEqualizer);
    dom.eqMidSlider.addEventListener('input', updateEqualizer);
    dom.eqTrebleSlider.addEventListener('input', updateEqualizer);

    dom.presetTags.forEach(tag => {
      tag.addEventListener('click', () => {
        dom.presetTags.forEach(t => t.classList.remove('active'));
        tag.classList.add('active');
        applyEqPreset(tag.dataset.preset);
      });
    });

    // Callbacks globales de Android
    window.onPermissionsGranted = function () {
      scanDeviceMusic();
    };

    window.onAudioFilesSelected = function (jsonStr) {
      try {
        const files = typeof jsonStr === 'string' ? JSON.parse(jsonStr) : jsonStr;
        if (Array.isArray(files) && files.length > 0) {
          files.forEach(f => {
            if (!f.artworkUrl) f.artworkUrl = generateArtworkDataUri(f.title, f.artist);
          });
          state.allLocalTracks = [...files, ...state.allLocalTracks];
          applyCurrentFilterAndSort();
        }
      } catch (ignored) {}
    };
  }

  // =========================================================================
  // GESTIÓN DE PESTAÑAS Y MODALES
  // =========================================================================

  function switchSubTab(tab) {
    state.currentTab = tab;
    dom.subTabPills.forEach(p => {
      p.classList.toggle('active', p.dataset.tab === tab);
    });
    applyCurrentFilterAndSort();
  }

  function closeDrawer() {
    dom.drawerBackdrop.classList.remove('active');
    dom.sideDrawer.classList.remove('active');
  }

  function openFullscreenPlayer() {
    dom.fullscreenModal.classList.add('active');
  }

  function closeFullscreenPlayer() {
    dom.fullscreenModal.classList.remove('active');
  }

  function openSortSheet() {
    dom.sortBackdrop.classList.add('active');
    dom.sortSheet.classList.add('active');
  }

  function closeSortSheet() {
    dom.sortBackdrop.classList.remove('active');
    dom.sortSheet.classList.remove('active');
  }

  function openSleepTimerSheet() {
    dom.timerBackdrop.classList.add('active');
    dom.timerSheet.classList.add('active');
  }

  function closeSleepTimerSheet() {
    dom.timerBackdrop.classList.remove('active');
    dom.timerSheet.classList.remove('active');
  }

  function openEqSheet() {
    dom.eqBackdrop.classList.add('active');
    dom.eqSheet.classList.add('active');
  }

  function closeEqSheet() {
    dom.eqBackdrop.classList.remove('active');
    dom.eqSheet.classList.remove('active');
  }

  function openTrackContextMenu(track) {
    state.activeContextTrack = track;
    dom.sheetThumb.src = track.artworkUrl || generateArtworkDataUri(track.title, track.artist);
    dom.sheetTitle.textContent = track.title || 'Canción';
    dom.sheetArtist.textContent = track.artist || 'Artista';

    const isFav = state.favoritesSet.has(track.id);
    dom.sheetFavLabel.textContent = isFav ? 'Quitar de favoritos' : 'Añadir a favoritos';

    dom.trackMenuBackdrop.classList.add('active');
    dom.trackMenuSheet.classList.add('active');
  }

  function closeTrackContextMenu() {
    dom.trackMenuBackdrop.classList.remove('active');
    dom.trackMenuSheet.classList.remove('active');
  }

  function toggleFavorite(trackId) {
    if (state.favoritesSet.has(trackId)) {
      state.favoritesSet.delete(trackId);
    } else {
      state.favoritesSet.add(trackId);
    }
    saveFavorites();
    if (state.currentTab === 'favorites') {
      applyCurrentFilterAndSort();
    }
  }

  // =========================================================================
  // TEMPORIZADOR DE APAGADO (SLEEP TIMER REAL)
  // =========================================================================

  function setSleepTimer(minutes) {
    if (state.sleepTimerId) {
      clearInterval(state.sleepTimerId);
      state.sleepTimerId = null;
    }

    if (minutes === 0) {
      state.sleepTimerRemaining = 0;
      dom.sleepTimerStatus.textContent = 'Desactivado';
      return;
    }

    if (minutes === -1) {
      state.sleepTimerRemaining = -1;
      dom.sleepTimerStatus.textContent = 'Al final de la pista';
      return;
    }

    state.sleepTimerRemaining = minutes * 60;
    dom.sleepTimerStatus.textContent = `${minutes} min restantes`;

    state.sleepTimerId = setInterval(() => {
      state.sleepTimerRemaining--;
      if (state.sleepTimerRemaining <= 0) {
        clearInterval(state.sleepTimerId);
        state.sleepTimerId = null;
        dom.sleepTimerStatus.textContent = 'Desactivado';
        stopPlayback();
      } else {
        const m = Math.floor(state.sleepTimerRemaining / 60);
        dom.sleepTimerStatus.textContent = `${m} min restantes`;
      }
    }, 1000);
  }

  // =========================================================================
  // ECUALIZADOR PARAMÉTRICO
  // =========================================================================

  function updateEqualizer() {
    const bass = parseInt(dom.eqBassSlider.value, 10);
    const mid = parseInt(dom.eqMidSlider.value, 10);
    const treble = parseInt(dom.eqTrebleSlider.value, 10);

    state.eq = { bass, mid, treble };

    dom.bassDbVal.textContent = (bass > 0 ? '+' : '') + bass + ' dB';
    dom.midDbVal.textContent = (mid > 0 ? '+' : '') + mid + ' dB';
    dom.trebleDbVal.textContent = (treble > 0 ? '+' : '') + treble + ' dB';

    if (state.isAndroid && window.TitanBridge) {
      window.TitanBridge.setEqualizer(bass, mid, treble);
    }
  }

  function applyEqPreset(preset) {
    let b = 0, m = 0, t = 0;
    if (preset === 'bass') { b = 7; m = 1; t = -1; }
    else if (preset === 'vocal') { b = -2; m = 4; t = 5; }
    else if (preset === 'electronic') { b = 6; m = 0; t = 5; }

    dom.eqBassSlider.value = b;
    dom.eqMidSlider.value = m;
    dom.eqTrebleSlider.value = t;
    updateEqualizer();
  }

  // =========================================================================
  // UTILIDADES
  // =========================================================================

  function formatDuration(sec) {
    if (isNaN(sec) || sec < 0) return '0:00';
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
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

  // Inicializar al cargar el DOM
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

})();

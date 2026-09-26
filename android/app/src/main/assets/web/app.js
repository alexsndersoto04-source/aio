/**
 * Mi Música — Cliente Nativo Hi-Fi en Modo Claro
 * Inspirado en la interfaz de usuario limpia y moderna de MIUI / HyperOS Music Player.
 * 100% Funcional y Real:
 * - Agrupación por Canciones, Artistas, Álbumes y Carpetas (MediaStore nativo).
 * - Listas de reproducción completas (Crear, añadir canciones, eliminar, reproducir).
 * - Favoritos y Recientes persistentes.
 * - Modal nativo de Detalles de Archivo (Ruta, tamaño MB, álbum, duración) sin alert() de navegador.
 * - Ecualizador de hardware nativo y Temporizador de apagado real.
 */

(function () {
  'use strict';

  // =========================================================================
  // 1. ESTADO GLOBAL
  // =========================================================================

  const state = {
    isAndroid: typeof window.TitanBridge !== 'undefined',
    activeView: 'songs', // 'songs' | 'artists' | 'albums' | 'folders' | 'playlists' | 'favorites' | 'recent' | 'cloud'
    activeGroup: null,   // null | { type: 'artist'|'album'|'folder'|'playlist', name: string, tracks: [] }
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
    playlists: {}, // { [name]: track[] }
    sleepTimerId: null,
    sleepTimerRemaining: 0,
    activeContextTrack: null,
    eq: { bass: 0, mid: 0, treble: 0 }
  };

  let htmlAudio = null;

  // =========================================================================
  // 2. REFERENCIAS AL DOM
  // =========================================================================

  const dom = {
    // Header
    openDrawerBtn: document.getElementById('openDrawerBtn'),
    globalSearchInput: document.getElementById('globalSearchInput'),
    clearSearchBtn: document.getElementById('clearSearchBtn'),

    // Top Cards
    cardFavorites: document.getElementById('cardFavorites'),
    cardPlaylists: document.getElementById('cardPlaylists'),
    cardRecent: document.getElementById('cardRecent'),
    favCountBadge: document.getElementById('favCountBadge'),
    playlistCountBadge: document.getElementById('playlistCountBadge'),
    recentCountBadge: document.getElementById('recentCountBadge'),

    // Sub Tabs Bar
    subTabsBar: document.getElementById('subTabsBar'),
    subTabPills: document.querySelectorAll('.sub-tab-pill'),

    // Toolbar
    shuffleAllBtn: document.getElementById('shuffleAllBtn'),
    totalTracksCount: document.getElementById('totalTracksCount'),
    sortToggleBtn: document.getElementById('sortToggleBtn'),

    // Container & Empty State
    songsContainer: document.getElementById('songsContainer'),
    emptyState: document.getElementById('emptyState'),
    emptyScanBtn: document.getElementById('emptyScanBtn'),

    // Floating Vinyl Player Dock
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

    // Side Drawer
    drawerBackdrop: document.getElementById('drawerBackdrop'),
    sideDrawer: document.getElementById('sideDrawer'),
    drawerEqItem: document.getElementById('drawerEqItem'),
    drawerSleepTimerItem: document.getElementById('drawerSleepTimerItem'),
    sleepTimerStatus: document.getElementById('sleepTimerStatus'),
    drawerScanItem: document.getElementById('drawerScanItem'),
    drawerPickerItem: document.getElementById('drawerPickerItem'),
    drawerStatsItem: document.getElementById('drawerStatsItem'),
    drawerStatsText: document.getElementById('drawerStatsText'),

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

    // Track Context Menu Sheet
    trackMenuBackdrop: document.getElementById('trackMenuBackdrop'),
    trackMenuSheet: document.getElementById('trackMenuSheet'),
    sheetThumb: document.getElementById('sheetThumb'),
    sheetTitle: document.getElementById('sheetTitle'),
    sheetArtist: document.getElementById('sheetArtist'),
    sheetPlayNextBtn: document.getElementById('sheetPlayNextBtn'),
    sheetToggleFavBtn: document.getElementById('sheetToggleFavBtn'),
    sheetFavLabel: document.getElementById('sheetFavLabel'),
    sheetAddToPlaylistBtn: document.getElementById('sheetAddToPlaylistBtn'),
    sheetInfoBtn: document.getElementById('sheetInfoBtn'),

    // File Details Sheet (Modal nativo real, cero alerts)
    fileInfoBackdrop: document.getElementById('fileInfoBackdrop'),
    fileInfoSheet: document.getElementById('fileInfoSheet'),
    infoTitle: document.getElementById('infoTitle'),
    infoArtist: document.getElementById('infoArtist'),
    infoAlbum: document.getElementById('infoAlbum'),
    infoDuration: document.getElementById('infoDuration'),
    infoSize: document.getElementById('infoSize'),
    infoFolder: document.getElementById('infoFolder'),
    infoPath: document.getElementById('infoPath'),

    // Add to Playlist Sheet
    addPlaylistBackdrop: document.getElementById('addPlaylistBackdrop'),
    addPlaylistSheet: document.getElementById('addPlaylistSheet'),
    playlistPickList: document.getElementById('playlistPickList'),
    openCreatePlModalBtn: document.getElementById('openCreatePlModalBtn'),

    // Create New Playlist Modal
    newPlaylistBackdrop: document.getElementById('newPlaylistBackdrop'),
    newPlaylistSheet: document.getElementById('newPlaylistSheet'),
    newPlaylistInput: document.getElementById('newPlaylistInput'),
    confirmCreatePlaylistBtn: document.getElementById('confirmCreatePlaylistBtn'),
    cancelCreatePlaylistBtn: document.getElementById('cancelCreatePlaylistBtn'),

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
  // 3. PERSISTENCIA LOCAL (FAVORITOS, RECIENTES, LISTAS)
  // =========================================================================

  function loadPersistedData() {
    try {
      const favs = localStorage.getItem('miui_favorites');
      if (favs) state.favoritesSet = new Set(JSON.parse(favs));

      const recents = localStorage.getItem('miui_recent');
      if (recents) state.recentList = JSON.parse(recents);

      const savedPl = localStorage.getItem('miui_playlists');
      if (savedPl) {
        state.playlists = JSON.parse(savedPl);
      } else {
        state.playlists = {
          'Favoritas del Verano': [],
          'Música para Entrenar': [],
          'Clásicos Inolvidables': []
        };
      }
    } catch (e) {
      console.warn('Error leyendo almacenamiento local', e);
    }
    updateCounters();
  }

  function saveFavorites() {
    try {
      localStorage.setItem('miui_favorites', JSON.stringify(Array.from(state.favoritesSet)));
    } catch (e) {}
    updateCounters();
  }

  function saveRecent() {
    try {
      localStorage.setItem('miui_recent', JSON.stringify(state.recentList.slice(0, 50)));
    } catch (e) {}
    updateCounters();
  }

  function savePlaylists() {
    try {
      localStorage.setItem('miui_playlists', JSON.stringify(state.playlists));
    } catch (e) {}
    updateCounters();
  }

  function updateCounters() {
    if (dom.favCountBadge) {
      dom.favCountBadge.textContent = `${state.favoritesSet.size} canciones`;
    }
    if (dom.playlistCountBadge) {
      const plCount = Object.keys(state.playlists).length;
      dom.playlistCountBadge.textContent = `${plCount} listas`;
    }
    if (dom.recentCountBadge) {
      dom.recentCountBadge.textContent = `${state.recentList.length} escuchadas`;
    }
    if (dom.drawerStatsText) {
      dom.drawerStatsText.textContent = `${state.allLocalTracks.length} canciones locales`;
    }
  }

  // =========================================================================
  // 4. GENERADOR DETERMINÍSTICO DE CARÁTULAS VECTORIALES
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
  // 5. INICIALIZACIÓN Y ESCANEO DE MÚSICA
  // =========================================================================

  function init() {
    loadPersistedData();
    bindEvents();

    if (state.isAndroid && window.TitanBridge) {
      scanDeviceMusic();
    } else {
      loadFallbackMusic();
    }

    fetchCloudTracks();
    setInterval(pollAndroidPlaybackStatus, 400);
  }

  function scanDeviceMusic() {
    if (!state.isAndroid || !window.TitanBridge) return;

    try {
      const raw = window.TitanBridge.scanLocalMusic();
      const list = JSON.parse(raw);

      if (Array.isArray(list)) {
        const filtered = list.filter(t => {
          const dur = t.duration || 0;
          const lowerTitle = (t.title || '').toLowerCase();
          const lowerPath = (t.rawPath || t.path || '').toLowerCase();

          if (dur < 30) return false; // Ignorar notificaciones cortas
          if (lowerTitle.includes('tts-') || lowerTitle.includes('inworld') || lowerTitle.startsWith('ptt-') || lowerTitle.startsWith('aud-')) return false;
          if (lowerPath.includes('whatsapp') || lowerPath.includes('cache')) return false;

          return true;
        });

        filtered.forEach(t => {
          t.artworkUrl = generateArtworkDataUri(t.title, t.artist);
          if (!t.folder) {
            t.folder = extractFolderName(t.rawPath || t.path);
          }
        });

        state.allLocalTracks = filtered;
        renderCurrentView();
      }
    } catch (e) {
      console.error('Error al escanear MediaStore', e);
    }
  }

  function extractFolderName(path) {
    if (!path) return 'Música interna';
    const parts = path.split('/');
    if (parts.length > 1) {
      return parts[parts.length - 2] || 'Música interna';
    }
    return 'Música interna';
  }

  function loadFallbackMusic() {
    const demo = [
      { id: 'm_1', title: 'Those Eyes', artist: 'New West', album: 'Those Eyes (Alternate Version)', duration: 220, size: 5410000, folder: 'Music', path: 'file:///android_asset/web/audio/track_acoustic.wav' },
      { id: 'm_2', title: 'Bon Appétit', artist: 'Katy Perry, Migos', album: 'Witness (Deluxe)', duration: 227, size: 7850000, folder: 'Download', path: 'file:///android_asset/web/audio/track_electronic.wav' },
      { id: 'm_3', title: 'Here With Me', artist: 'd4vd', album: 'Petals to Thorns', duration: 182, size: 4200000, folder: 'Music', path: 'file:///android_asset/web/audio/track_lofi.wav' },
      { id: 'm_4', title: 'Runaway', artist: 'Sebastian Yatra, Daddy Yankee', album: 'Dharma', duration: 202, size: 6150000, folder: 'TitanMusic', path: 'file:///android_asset/web/audio/track_synthwave.wav' },
      { id: 'm_5', title: 'Try', artist: 'P!nk', album: "Now That's What I Call Music", duration: 247, size: 8900000, folder: 'Music', path: 'file:///android_asset/web/audio/track_rock.wav' }
    ];
    demo.forEach(d => {
      d.artworkUrl = generateArtworkDataUri(d.title, d.artist);
    });
    state.allLocalTracks = demo;
    renderCurrentView();
  }

  function fetchCloudTracks() {
    fetch('/api/tracks')
      .then(r => r.json())
      .then(d => {
        if (d && Array.isArray(d.tracks)) {
          d.tracks.forEach(t => {
            t.artworkUrl = generateArtworkDataUri(t.title, t.artist);
            t.folder = 'Telegram Cloud';
          });
          state.cloudTracks = d.tracks;
          if (state.activeView === 'cloud') {
            renderCurrentView();
          }
        }
      })
      .catch(() => {});
  }

  // =========================================================================
  // 6. MOTOR DE RENDERIZADO PRINCIPAL (VISTAS Y AGRUPACIONES)
  // =========================================================================

  function renderCurrentView() {
    // Si estamos dentro de un grupo abierto (Artista, Álbum, Carpeta, Lista)
    if (state.activeGroup) {
      renderGroupDetailView(state.activeGroup);
      return;
    }

    // Vistas principales según pestaña activa
    switch (state.activeView) {
      case 'songs':
        renderSongsListView(state.allLocalTracks);
        break;
      case 'artists':
        renderArtistsGridView();
        break;
      case 'albums':
        renderAlbumsGridView();
        break;
      case 'folders':
        renderFoldersListView();
        break;
      case 'playlists':
        renderPlaylistsListView();
        break;
      case 'favorites':
        renderFavoritesView();
        break;
      case 'recent':
        renderRecentView();
        break;
      case 'cloud':
        renderSongsListView(state.cloudTracks);
        break;
      default:
        renderSongsListView(state.allLocalTracks);
        break;
    }
  }

  // --- 6.1 VISTA DE CANCIONES (CON BÚSQUEDA Y ORDENACIÓN) ---
  function renderSongsListView(sourceTracks) {
    let tracks = [...sourceTracks];

    // Aplicar búsqueda en tiempo real
    if (state.searchQuery) {
      const q = state.searchQuery.toLowerCase();
      tracks = tracks.filter(t =>
        (t.title && t.title.toLowerCase().includes(q)) ||
        (t.artist && t.artist.toLowerCase().includes(q)) ||
        (t.album && t.album.toLowerCase().includes(q)) ||
        (t.folder && t.folder.toLowerCase().includes(q))
      );
    }

    // Aplicar ordenación
    sortTrackArray(tracks, state.sortMode);

    state.displayTracks = tracks;
    state.currentQueue = tracks;
    dom.totalTracksCount.textContent = tracks.length;

    if (tracks.length === 0) {
      dom.songsContainer.innerHTML = '';
      dom.emptyState.style.display = 'flex';
      return;
    }
    dom.emptyState.style.display = 'none';

    dom.songsContainer.innerHTML = '';
    const frag = document.createDocumentFragment();

    tracks.forEach((track, index) => {
      const row = createSongRowElement(track, index);
      frag.appendChild(row);
    });

    dom.songsContainer.appendChild(frag);
  }

  // --- 6.2 VISTA DE ARTISTAS REAL (AGRUPADOS POR ARTISTA) ---
  function renderArtistsGridView() {
    dom.emptyState.style.display = 'none';
    dom.songsContainer.innerHTML = '';

    const map = {};
    state.allLocalTracks.forEach(t => {
      const artist = (t.artist || 'Artista desconocido').trim();
      if (!map[artist]) map[artist] = [];
      map[artist].push(t);
    });

    let artistNames = Object.keys(map).sort((a, b) => a.localeCompare(b));

    if (state.searchQuery) {
      const q = state.searchQuery.toLowerCase();
      artistNames = artistNames.filter(name => name.toLowerCase().includes(q));
    }

    dom.totalTracksCount.textContent = `${artistNames.length} artistas`;

    if (artistNames.length === 0) {
      dom.songsContainer.innerHTML = `
        <div class="empty-state-card">
          <p class="empty-title">No se encontraron artistas</p>
        </div>
      `;
      return;
    }

    const frag = document.createDocumentFragment();

    artistNames.forEach(artist => {
      const tracks = map[artist];
      const card = document.createElement('div');
      card.className = 'group-card-row';

      const initial = artist.charAt(0).toUpperCase();

      card.innerHTML = `
        <div class="group-icon-avatar">${initial}</div>
        <div class="group-meta">
          <span class="group-name">${escapeXml(artist)}</span>
          <span class="group-sub">${tracks.length} ${tracks.length === 1 ? 'canción' : 'canciones'}</span>
        </div>
        <div class="group-arrow">
          <svg viewBox="0 0 24 24" width="20" height="20"><path fill="currentColor" d="M8.59 16.59L13.17 12 8.59 7.41 10 6l6 6-6 6-1.41-1.41z"/></svg>
        </div>
      `;

      card.addEventListener('click', () => {
        state.activeGroup = {
          type: 'artist',
          name: artist,
          tracks: tracks
        };
        renderCurrentView();
      });

      frag.appendChild(card);
    });

    dom.songsContainer.appendChild(frag);
  }

  // --- 6.3 VISTA DE ÁLBUMES REAL (AGRUPADOS POR ÁLBUM) ---
  function renderAlbumsGridView() {
    dom.emptyState.style.display = 'none';
    dom.songsContainer.innerHTML = '';

    const map = {};
    state.allLocalTracks.forEach(t => {
      const album = (t.album || 'Álbum desconocido').trim();
      if (!map[album]) map[album] = [];
      map[album].push(t);
    });

    let albumNames = Object.keys(map).sort((a, b) => a.localeCompare(b));

    if (state.searchQuery) {
      const q = state.searchQuery.toLowerCase();
      albumNames = albumNames.filter(name => name.toLowerCase().includes(q));
    }

    dom.totalTracksCount.textContent = `${albumNames.length} álbumes`;

    if (albumNames.length === 0) {
      dom.songsContainer.innerHTML = `
        <div class="empty-state-card">
          <p class="empty-title">No se encontraron álbumes</p>
        </div>
      `;
      return;
    }

    const frag = document.createDocumentFragment();

    albumNames.forEach(album => {
      const tracks = map[album];
      const firstArtist = tracks[0].artist || 'Varios Artistas';
      const cover = tracks[0].artworkUrl || generateArtworkDataUri(album, firstArtist);

      const card = document.createElement('div');
      card.className = 'group-card-row';

      card.innerHTML = `
        <img class="group-album-cover" src="${cover}" alt="Álbum" loading="lazy">
        <div class="group-meta">
          <span class="group-name">${escapeXml(album)}</span>
          <span class="group-sub">${escapeXml(firstArtist)} • ${tracks.length} ${tracks.length === 1 ? 'canción' : 'canciones'}</span>
        </div>
        <div class="group-arrow">
          <svg viewBox="0 0 24 24" width="20" height="20"><path fill="currentColor" d="M8.59 16.59L13.17 12 8.59 7.41 10 6l6 6-6 6-1.41-1.41z"/></svg>
        </div>
      `;

      card.addEventListener('click', () => {
        state.activeGroup = {
          type: 'album',
          name: album,
          artist: firstArtist,
          artworkUrl: cover,
          tracks: tracks
        };
        renderCurrentView();
      });

      frag.appendChild(card);
    });

    dom.songsContainer.appendChild(frag);
  }

  // --- 6.4 VISTA DE CARPETAS REAL (POR DIRECTORIO DE ALMACENAMIENTO) ---
  function renderFoldersListView() {
    dom.emptyState.style.display = 'none';
    dom.songsContainer.innerHTML = '';

    const map = {};
    state.allLocalTracks.forEach(t => {
      const folder = t.folder || extractFolderName(t.rawPath || t.path);
      if (!map[folder]) map[folder] = [];
      map[folder].push(t);
    });

    let folderNames = Object.keys(map).sort((a, b) => a.localeCompare(b));

    if (state.searchQuery) {
      const q = state.searchQuery.toLowerCase();
      folderNames = folderNames.filter(name => name.toLowerCase().includes(q));
    }

    dom.totalTracksCount.textContent = `${folderNames.length} carpetas`;

    if (folderNames.length === 0) {
      dom.songsContainer.innerHTML = `
        <div class="empty-state-card">
          <p class="empty-title">No se encontraron carpetas con música</p>
        </div>
      `;
      return;
    }

    const frag = document.createDocumentFragment();

    folderNames.forEach(folder => {
      const tracks = map[folder];
      const card = document.createElement('div');
      card.className = 'group-card-row';

      card.innerHTML = `
        <div class="group-icon-avatar" style="background:#e0f2fe; color:#0284c7;">
          <svg viewBox="0 0 24 24" width="22" height="22"><path fill="currentColor" d="M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/></svg>
        </div>
        <div class="group-meta">
          <span class="group-name">${escapeXml(folder)}</span>
          <span class="group-sub">${tracks.length} ${tracks.length === 1 ? 'archivo de audio' : 'archivos de audio'}</span>
        </div>
        <div class="group-arrow">
          <svg viewBox="0 0 24 24" width="20" height="20"><path fill="currentColor" d="M8.59 16.59L13.17 12 8.59 7.41 10 6l6 6-6 6-1.41-1.41z"/></svg>
        </div>
      `;

      card.addEventListener('click', () => {
        state.activeGroup = {
          type: 'folder',
          name: folder,
          tracks: tracks
        };
        renderCurrentView();
      });

      frag.appendChild(card);
    });

    dom.songsContainer.appendChild(frag);
  }

  // --- 6.5 VISTA DE LISTAS DE REPRODUCCIÓN (CRUD REAL) ---
  function renderPlaylistsListView() {
    dom.emptyState.style.display = 'none';
    dom.songsContainer.innerHTML = '';

    const plNames = Object.keys(state.playlists);
    dom.totalTracksCount.textContent = `${plNames.length} listas`;

    const frag = document.createDocumentFragment();

    // Botón para crear nueva lista directamente arriba
    const createBtnRow = document.createElement('div');
    createBtnRow.style.padding = '4px 0 12px 0';
    createBtnRow.innerHTML = `
      <button class="create-pl-inline-btn" style="margin-top:0;">
        + Crear nueva lista de reproducción
      </button>
    `;
    createBtnRow.querySelector('button').addEventListener('click', () => {
      openNewPlaylistModal();
    });
    frag.appendChild(createBtnRow);

    plNames.forEach(plName => {
      const tracks = state.playlists[plName] || [];
      const card = document.createElement('div');
      card.className = 'group-card-row';

      card.innerHTML = `
        <div class="group-icon-avatar" style="background:#fef3c7; color:#d97706;">
          <svg viewBox="0 0 24 24" width="22" height="22"><path fill="currentColor" d="M15 6H3v2h12V6zm0 4H3v2h12v-2zM3 16h8v-2H3v2zM17 6v8.18c-.31-.11-.65-.18-1-.18-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3V8h3V6h-5z"/></svg>
        </div>
        <div class="group-meta">
          <span class="group-name">${escapeXml(plName)}</span>
          <span class="group-sub">${tracks.length} ${tracks.length === 1 ? 'canción' : 'canciones'}</span>
        </div>
        <div style="display:flex; align-items:center; gap:8px;">
          <button class="icon-tool-btn delete-pl-btn" title="Eliminar lista" style="color:#ef4444;">
            <svg viewBox="0 0 24 24" width="18" height="18"><path fill="currentColor" d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/></svg>
          </button>
          <div class="group-arrow">
            <svg viewBox="0 0 24 24" width="20" height="20"><path fill="currentColor" d="M8.59 16.59L13.17 12 8.59 7.41 10 6l6 6-6 6-1.41-1.41z"/></svg>
          </div>
        </div>
      `;

      card.querySelector('.delete-pl-btn').addEventListener('click', (e) => {
        e.stopPropagation();
        deletePlaylist(plName);
      });

      card.addEventListener('click', () => {
        state.activeGroup = {
          type: 'playlist',
          name: plName,
          tracks: tracks
        };
        renderCurrentView();
      });

      frag.appendChild(card);
    });

    dom.songsContainer.appendChild(frag);
  }

  // --- 6.6 VISTA DE FAVORITOS ---
  function renderFavoritesView() {
    const favTracks = state.allLocalTracks.filter(t => state.favoritesSet.has(t.id));

    dom.totalTracksCount.textContent = `${favTracks.length} favoritas`;

    if (favTracks.length === 0) {
      dom.songsContainer.innerHTML = `
        <div class="empty-state-card">
          <div class="empty-icon-circle" style="color:#ef4444;">
            <svg viewBox="0 0 24 24" width="32" height="32"><path fill="currentColor" d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z"/></svg>
          </div>
          <p class="empty-title">Aún no tienes favoritos</p>
          <p class="empty-desc">Toca el corazón en cualquier canción para guardarla aquí.</p>
        </div>
      `;
      return;
    }

    renderSongsListView(favTracks);
  }

  // --- 6.7 VISTA DE RECIENTES ---
  function renderRecentView() {
    dom.totalTracksCount.textContent = `${state.recentList.length} reproducidas`;

    if (state.recentList.length === 0) {
      dom.songsContainer.innerHTML = `
        <div class="empty-state-card">
          <div class="empty-icon-circle" style="color:var(--primary-accent);">
            <svg viewBox="0 0 24 24" width="32" height="32"><path fill="currentColor" d="M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8zm.5-13H11v6l5.25 3.15.75-1.23-4.5-2.67z"/></svg>
          </div>
          <p class="empty-title">Historial vacío</p>
          <p class="empty-desc">Las canciones que reproduzcas aparecerán organizadas aquí.</p>
        </div>
      `;
      return;
    }

    renderSongsListView(state.recentList);
  }

  // --- 6.8 VISTA DETALLE DE GRUPO (ARTISTA, ÁLBUM, CARPETA, LISTA ABIERTA) ---
  function renderGroupDetailView(group) {
    dom.emptyState.style.display = 'none';
    dom.songsContainer.innerHTML = '';

    const frag = document.createDocumentFragment();

    // Barra de navegación hacia atrás
    const backBar = document.createElement('div');
    backBar.className = 'group-back-bar';
    backBar.innerHTML = `
      <svg viewBox="0 0 24 24" width="20" height="20"><path fill="currentColor" d="M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z"/></svg>
      <span>Volver a ${getGroupNameByType(group.type)}</span>
    `;
    backBar.addEventListener('click', () => {
      state.activeGroup = null;
      renderCurrentView();
    });
    frag.appendChild(backBar);

    // Encabezado descriptivo del grupo
    const header = document.createElement('div');
    header.style.display = 'flex';
    header.style.alignItems = 'center';
    header.style.gap = '14px';
    header.style.padding = '8px 0 16px 0';
    header.style.borderBottom = '1px solid var(--border-light)';
    header.style.marginBottom = '12px';

    const groupTitle = escapeXml(group.name);
    const countText = `${group.tracks.length} ${group.tracks.length === 1 ? 'canción' : 'canciones'}`;

    header.innerHTML = `
      <div style="flex:1; min-width:0;">
        <h2 style="font-size:18px; font-weight:800; color:var(--text-main); white-space:nowrap; overflow:hidden; text-overflow:ellipsis;">${groupTitle}</h2>
        <span style="font-size:13px; color:var(--text-muted);">${countText}</span>
      </div>
      <button class="shuffle-play-all-btn" id="playGroupAllBtn" style="background:#000000; color:#ffffff; padding:7px 14px; border-radius:18px;">
        <span>Reproducir</span>
      </button>
    `;

    header.querySelector('#playGroupAllBtn').addEventListener('click', () => {
      if (group.tracks.length > 0) {
        state.currentQueue = [...group.tracks];
        playTrackByIndex(0);
      }
    });

    frag.appendChild(header);

    // Listado de canciones del grupo
    state.displayTracks = group.tracks;
    state.currentQueue = group.tracks;
    dom.totalTracksCount.textContent = group.tracks.length;

    group.tracks.forEach((track, index) => {
      const row = createSongRowElement(track, index);
      frag.appendChild(row);
    });

    dom.songsContainer.appendChild(frag);
  }

  function getGroupNameByType(type) {
    if (type === 'artist') return 'Artistas';
    if (type === 'album') return 'Álbumes';
    if (type === 'folder') return 'Carpetas';
    if (type === 'playlist') return 'Listas';
    return 'Mi Música';
  }

  // --- 6.9 CREADOR DE FILA DE CANCIÓN (CON DETALLES Y ECUALIZADOR PÚRPURA) ---
  function createSongRowElement(track, index) {
    const isCurrent = state.currentTrack && state.currentTrack.id === track.id;
    const row = document.createElement('div');
    row.className = `song-row ${isCurrent ? 'active' : ''}`;
    row.dataset.id = track.id;
    row.dataset.index = index;

    const artwork = track.artworkUrl || generateArtworkDataUri(track.title, track.artist);
    const subtitleText = `${escapeXml(track.artist || 'Artista')} | ${escapeXml(track.album || track.folder || 'Dispositivo')}`;

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

    row.addEventListener('click', (e) => {
      if (e.target.closest('.song-options-btn')) {
        e.stopPropagation();
        openTrackContextMenu(track);
        return;
      }
      playTrackByIndex(index);
    });

    return row;
  }

  function sortTrackArray(arr, mode) {
    if (mode === 'name') {
      arr.sort((a, b) => (a.title || '').localeCompare(b.title || ''));
    } else if (mode === 'artist') {
      arr.sort((a, b) => (a.artist || '').localeCompare(b.artist || ''));
    } else if (mode === 'duration') {
      arr.sort((a, b) => (b.duration || 0) - (a.duration || 0));
    }
  }

  // =========================================================================
  // 7. GESTIÓN DE LISTAS DE REPRODUCCIÓN (CRUD REAL)
  // =========================================================================

  function openNewPlaylistModal() {
    dom.newPlaylistInput.value = '';
    dom.newPlaylistBackdrop.classList.add('active');
    dom.newPlaylistSheet.classList.add('active');
    setTimeout(() => dom.newPlaylistInput.focus(), 150);
  }

  function closeNewPlaylistModal() {
    dom.newPlaylistBackdrop.classList.remove('active');
    dom.newPlaylistSheet.classList.remove('active');
  }

  function createNewPlaylist(name) {
    const trimmed = (name || '').trim();
    if (!trimmed) return;

    if (!state.playlists[trimmed]) {
      state.playlists[trimmed] = [];
      savePlaylists();
    }

    closeNewPlaylistModal();

    // Si había una canción seleccionada para añadir, guardarla
    if (state.activeContextTrack) {
      addTrackToPlaylist(trimmed, state.activeContextTrack);
      closeAddToPlaylistSheet();
    }

    if (state.activeView === 'playlists') {
      renderCurrentView();
    }
  }

  function deletePlaylist(name) {
    if (state.playlists[name]) {
      delete state.playlists[name];
      savePlaylists();
      renderCurrentView();
    }
  }

  function openAddToPlaylistSheet(track) {
    state.activeContextTrack = track;
    dom.playlistPickList.innerHTML = '';

    const plNames = Object.keys(state.playlists);
    if (plNames.length === 0) {
      dom.playlistPickList.innerHTML = `
        <div style="padding:10px 0; color:var(--text-muted); font-size:13px;">
          No tienes listas de reproducción creadas.
        </div>
      `;
    } else {
      plNames.forEach(name => {
        const item = document.createElement('button');
        item.className = 'sheet-item';
        item.innerHTML = `
          <svg viewBox="0 0 24 24" width="20" height="20" style="color:var(--primary-accent);"><path fill="currentColor" d="M15 6H3v2h12V6zm0 4H3v2h12v-2zM3 16h8v-2H3v2zM17 6v8.18c-.31-.11-.65-.18-1-.18-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3V8h3V6h-5z"/></svg>
          <span style="flex:1;">${escapeXml(name)}</span>
          <span style="font-size:12px; color:var(--text-muted);">${state.playlists[name].length} canciones</span>
        `;
        item.addEventListener('click', () => {
          addTrackToPlaylist(name, track);
          closeAddToPlaylistSheet();
        });
        dom.playlistPickList.appendChild(item);
      });
    }

    dom.addPlaylistBackdrop.classList.add('active');
    dom.addPlaylistSheet.classList.add('active');
  }

  function closeAddToPlaylistSheet() {
    dom.addPlaylistBackdrop.classList.remove('active');
    dom.addPlaylistSheet.classList.remove('active');
  }

  function addTrackToPlaylist(plName, track) {
    if (!state.playlists[plName]) state.playlists[plName] = [];

    const exists = state.playlists[plName].some(t => t.id === track.id);
    if (!exists) {
      state.playlists[plName].push(track);
      savePlaylists();
    }
  }

  // =========================================================================
  // 8. MODAL REAL DE DETALLES DEL ARCHIVO (SIN ALERTS)
  // =========================================================================

  function openFileDetailsModal(track) {
    if (!track) return;

    dom.infoTitle.textContent = track.title || 'Desconocido';
    dom.infoArtist.textContent = track.artist || 'Desconocido';
    dom.infoAlbum.textContent = track.album || 'Desconocido';
    dom.infoDuration.textContent = formatDuration(track.duration);
    dom.infoSize.textContent = formatFileSize(track.size);
    dom.infoFolder.textContent = track.folder || extractFolderName(track.rawPath || track.path);
    dom.infoPath.textContent = track.rawPath || track.path || 'Almacenamiento del dispositivo';

    dom.fileInfoBackdrop.classList.add('active');
    dom.fileInfoSheet.classList.add('active');
  }

  function closeFileDetailsModal() {
    dom.fileInfoBackdrop.classList.remove('active');
    dom.fileInfoSheet.classList.remove('active');
  }

  function formatFileSize(bytes) {
    if (!bytes || bytes <= 0) return 'Desconocido';
    const mb = bytes / (1024 * 1024);
    if (mb < 1) {
      const kb = bytes / 1024;
      return `${kb.toFixed(1)} KB`;
    }
    return `${mb.toFixed(2)} MB`;
  }

  // =========================================================================
  // 9. REPRODUCCIÓN (ANDROID NATIVO + FALLBACK WEB AUDIO)
  // =========================================================================

  function playTrackByIndex(index) {
    if (index < 0 || index >= state.currentQueue.length) return;

    state.currentIndex = index;
    const track = state.currentQueue[index];
    state.currentTrack = track;
    state.duration = track.duration || 180;
    state.currentTime = 0;

    addToRecent(track);
    updatePlayerMeta(track);

    const path = track.path || track.streamUrl || track.rawPath || '';

    if (state.isAndroid && window.TitanBridge) {
      const isCloud = track.source === 'cloud' || (path.startsWith('http://') || path.startsWith('https://'));
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
      window.TitanBridge.pauseTrack();
    }
    if (htmlAudio) htmlAudio.pause();
    setPlaybackState(false);
  }

  function seekToSeconds(sec) {
    state.currentTime = sec;
    if (state.isAndroid && window.TitanBridge) {
      window.TitanBridge.seekTo(sec * 1000);
    }
    if (htmlAudio) {
      htmlAudio.currentTime = sec;
    }
    updateProgressUI();
  }

  function setPlaybackState(playing) {
    state.isPlaying = playing;
    dom.floatingPlayer.classList.toggle('playing', playing);

    dom.dockPlayIcon.style.display = playing ? 'none' : 'block';
    dom.dockPauseIcon.style.display = playing ? 'block' : 'none';

    dom.fsPlayIcon.style.display = playing ? 'none' : 'block';
    dom.fsPauseIcon.style.display = playing ? 'block' : 'none';

    updateActiveRowVisuals();
  }

  function updatePlayerMeta(track) {
    const artwork = track.artworkUrl || generateArtworkDataUri(track.title, track.artist);

    dom.dockTitle.textContent = track.title || 'Música';
    dom.dockArtist.textContent = track.artist || 'Artista';
    dom.dockArtwork.src = artwork;

    dom.fsTitle.textContent = track.title || 'Música';
    dom.fsArtist.textContent = track.artist || 'Artista';
    dom.fsArtworkImg.src = artwork;

    dom.fsSourceBadge.textContent = (track.folder || 'LOCAL').toUpperCase();
    dom.fsFavBtn.classList.toggle('liked', state.favoritesSet.has(track.id));

    dom.fsTimeTotal.textContent = formatDuration(state.duration);
    updateProgressUI();
  }

  function updateProgressUI() {
    const dur = state.duration || 1;
    const progressPct = Math.min(100, Math.max(0, (state.currentTime / dur) * 100));

    dom.fsScrubberFill.style.width = `${progressPct}%`;
    dom.fsScrubberThumb.style.left = `${progressPct}%`;
    dom.fsTimeCurrent.textContent = formatDuration(state.currentTime);
  }

  function pollAndroidPlaybackStatus() {
    if (!state.isAndroid || !window.TitanBridge) return;

    try {
      const raw = window.TitanBridge.getPlaybackStatus();
      if (!raw) return;
      const data = JSON.parse(raw);

      if (typeof data.isPlaying === 'boolean' && data.isPlaying !== state.isPlaying) {
        setPlaybackState(data.isPlaying);
      }
      if (typeof data.currentPositionMs === 'number') {
        state.currentTime = Math.round(data.currentPositionMs / 1000);
        updateProgressUI();
      }
      if (typeof data.durationMs === 'number' && data.durationMs > 0) {
        state.duration = Math.round(data.durationMs / 1000);
        dom.fsTimeTotal.textContent = formatDuration(state.duration);
      }
    } catch (e) {}
  }

  function addToRecent(track) {
    state.recentList = state.recentList.filter(t => t.id !== track.id);
    state.recentList.unshift(track);
    saveRecent();
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
  // 10. INTERACCIONES Y EVENTOS
  // =========================================================================

  function bindEvents() {
    // Drawer lateral
    dom.openDrawerBtn.addEventListener('click', () => {
      dom.drawerBackdrop.classList.add('active');
      dom.sideDrawer.classList.add('active');
    });

    dom.drawerBackdrop.addEventListener('click', closeDrawer);

    dom.drawerScanItem.addEventListener('click', () => {
      closeDrawer();
      if (state.isAndroid && window.TitanBridge) {
        window.TitanBridge.scanLocalMusic();
        scanDeviceMusic();
      }
    });

    dom.drawerSleepTimerItem.addEventListener('click', () => {
      closeDrawer();
      openSleepTimerSheet();
    });

    dom.drawerEqItem.addEventListener('click', () => {
      closeDrawer();
      openEqSheet();
    });

    dom.drawerPickerItem.addEventListener('click', () => {
      closeDrawer();
      if (state.isAndroid && window.TitanBridge) {
        window.TitanBridge.openAudioFilePicker();
      }
    });

    // Pestañas inferiores (Bottom Nav)
    dom.bottomNavItems.forEach(item => {
      item.addEventListener('click', () => {
        const nav = item.dataset.nav;
        dom.bottomNavItems.forEach(i => i.classList.remove('active'));
        item.classList.add('active');

        state.activeGroup = null;

        if (nav === 'local') {
          switchCategoryTab('songs');
        } else if (nav === 'discover') {
          state.activeView = 'cloud';
          renderCurrentView();
        } else if (nav === 'playlists') {
          state.activeView = 'playlists';
          renderCurrentView();
        } else if (nav === 'profile') {
          dom.openDrawerBtn.click();
        }
      });
    });

    dom.centerHifiBtn.addEventListener('click', () => {
      openEqSheet();
    });

    // Tarjetas destacadas superiores
    dom.cardFavorites.addEventListener('click', () => {
      state.activeGroup = null;
      state.activeView = 'favorites';
      renderCurrentView();
    });

    dom.cardPlaylists.addEventListener('click', () => {
      state.activeGroup = null;
      state.activeView = 'playlists';
      renderCurrentView();
    });

    dom.cardRecent.addEventListener('click', () => {
      state.activeGroup = null;
      state.activeView = 'recent';
      renderCurrentView();
    });

    // Sub-pestañas horizontales (Canciones, Artistas, Álbumes, Carpetas)
    dom.subTabPills.forEach(pill => {
      pill.addEventListener('click', () => {
        const tab = pill.dataset.tab;
        switchCategoryTab(tab);
      });
    });

    // Barra de búsqueda
    dom.globalSearchInput.addEventListener('input', (e) => {
      state.searchQuery = e.target.value.trim();
      dom.clearSearchBtn.style.display = state.searchQuery ? 'block' : 'none';
      renderCurrentView();
    });

    dom.clearSearchBtn.addEventListener('click', () => {
      dom.globalSearchInput.value = '';
      state.searchQuery = '';
      dom.clearSearchBtn.style.display = 'none';
      renderCurrentView();
    });

    // Reproducir todo / Aleatorio
    dom.shuffleAllBtn.addEventListener('click', () => {
      if (state.displayTracks.length === 0) return;
      const shuffled = [...state.displayTracks].sort(() => Math.random() - 0.5);
      state.currentQueue = shuffled;
      playTrackByIndex(0);
    });

    // Modal de Ordenación
    dom.sortToggleBtn.addEventListener('click', openSortSheet);
    dom.sortBackdrop.addEventListener('click', closeSortSheet);

    dom.sortOptions.forEach(opt => {
      opt.addEventListener('click', () => {
        state.sortMode = opt.dataset.sort;
        dom.sortOptions.forEach(o => o.classList.remove('active'));
        opt.classList.add('active');
        closeSortSheet();
        renderCurrentView();
      });
    });

    // Mini Reproductor Flotante
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

    // Pantalla Completa
    dom.closeFsBtn.addEventListener('click', closeFullscreenPlayer);
    dom.fsPlayPauseBtn.addEventListener('click', togglePlayPause);
    dom.fsNextTrackBtn.addEventListener('click', playNextTrack);
    dom.fsPrevTrackBtn.addEventListener('click', playPreviousTrack);

    dom.fsFavBtn.addEventListener('click', () => {
      if (!state.currentTrack) return;
      toggleFavorite(state.currentTrack.id);
      dom.fsFavBtn.classList.toggle('liked', state.favoritesSet.has(state.currentTrack.id));
    });

    dom.fsScrubber.addEventListener('click', (e) => {
      const rect = dom.fsScrubber.getBoundingClientRect();
      const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
      const targetSec = Math.round(ratio * (state.duration || 180));
      seekToSeconds(targetSec);
    });

    dom.fsEqDrawerBtn.addEventListener('click', openEqSheet);

    // Menú contextual de pista (⋮)
    dom.trackMenuBackdrop.addEventListener('click', closeTrackContextMenu);

    dom.sheetPlayNextBtn.addEventListener('click', () => {
      if (!state.activeContextTrack) return;
      const nextPos = state.currentIndex + 1;
      state.currentQueue.splice(nextPos, 0, state.activeContextTrack);
      closeTrackContextMenu();
    });

    dom.sheetToggleFavBtn.addEventListener('click', () => {
      if (!state.activeContextTrack) return;
      toggleFavorite(state.activeContextTrack.id);
      closeTrackContextMenu();
    });

    dom.sheetAddToPlaylistBtn.addEventListener('click', () => {
      if (!state.activeContextTrack) return;
      const track = state.activeContextTrack;
      closeTrackContextMenu();
      openAddToPlaylistSheet(track);
    });

    dom.sheetInfoBtn.addEventListener('click', () => {
      if (!state.activeContextTrack) return;
      const track = state.activeContextTrack;
      closeTrackContextMenu();
      openFileDetailsModal(track);
    });

    // Modal de Detalles de Archivo
    dom.fileInfoBackdrop.addEventListener('click', closeFileDetailsModal);

    // Modal de Añadir a Lista
    dom.addPlaylistBackdrop.addEventListener('click', closeAddToPlaylistSheet);

    dom.openCreatePlModalBtn.addEventListener('click', () => {
      closeAddToPlaylistSheet();
      openNewPlaylistModal();
    });

    // Modal de Crear Lista
    dom.newPlaylistBackdrop.addEventListener('click', closeNewPlaylistModal);
    dom.cancelCreatePlaylistBtn.addEventListener('click', closeNewPlaylistModal);
    dom.confirmCreatePlaylistBtn.addEventListener('click', () => {
      createNewPlaylist(dom.newPlaylistInput.value);
    });

    dom.newPlaylistInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        createNewPlaylist(dom.newPlaylistInput.value);
      }
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
            if (!f.folder) f.folder = extractFolderName(f.rawPath || f.path);
          });
          state.allLocalTracks = [...files, ...state.allLocalTracks];
          renderCurrentView();
        }
      } catch (ignored) {}
    };
  }

  // =========================================================================
  // 11. FUNCIONES AUXILIARES DE NAVEGACIÓN Y MODALES
  // =========================================================================

  function switchCategoryTab(tab) {
    state.activeView = tab;
    state.activeGroup = null;

    dom.subTabPills.forEach(p => {
      p.classList.toggle('active', p.dataset.tab === tab);
    });

    renderCurrentView();
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

    if (state.activeView === 'favorites') {
      renderCurrentView();
    }
  }

  // =========================================================================
  // 12. TEMPORIZADOR DE APAGADO (SLEEP TIMER REAL)
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
  // 13. ECUALIZADOR PARAMÉTRICO DE HARDWARE
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
  // 14. UTILIDADES
  // =========================================================================

  function formatDuration(sec) {
    if (isNaN(sec) || sec < 0) return '0:00';
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${s < 10 ? '0' : ''}${s}`;
  }

  function escapeXml(str) {
    return String(str || '')
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

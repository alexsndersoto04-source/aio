// =============================================================================
// TITAN AUDIO — MOTOR Y CONTROLADOR PRINCIPAL (100% REAL)
// =============================================================================

(function () {
    'use strict';

    // ESTADO DE LA APLICACIÓN
    const state = {
        currentTab: 'tabHome',
        currentTrack: null,
        isPlaying: false,
        position: 0,
        duration: 180,
        volume: 85,
        crossfade: 0.0,
        gapless: true,
        eq: { bass: 0, mid: 0, treble: 0 },
        shuffle: false,
        repeat: false,
        songs: [],
        filteredSongs: [],
        artworksCache: {} // Cache de carátulas embebidas Base64
    };

    // ELEMENTOS DOM
    const el = {};

    function cacheDom() {
        // Header & Tools
        el.importFilesBtn = document.getElementById('importFilesBtn');
        el.refreshScanBtn = document.getElementById('refreshScanBtn');
        el.webAudioFileInput = document.getElementById('webAudioFileInput');
        el.tabPanes = document.querySelectorAll('.tab-pane');
        el.navItems = document.querySelectorAll('.nav-item');

        // Home
        el.heroGreeting = document.getElementById('heroGreeting');
        el.heroScanActionBtn = document.getElementById('heroScanActionBtn');
        el.heroPickerActionBtn = document.getElementById('heroPickerActionBtn');
        el.libraryCountText = document.getElementById('libraryCountText');
        el.totalTracksBadge = document.getElementById('totalTracksBadge');
        el.homeTracksList = document.getElementById('homeTracksList');

        // Library
        el.libSearchInput = document.getElementById('libSearchInput');
        el.libSearchClearBtn = document.getElementById('libSearchClearBtn');
        el.fullLibraryList = document.getElementById('fullLibraryList');
        el.sortChips = document.querySelectorAll('.filter-chip');

        // Settings
        el.sliderVolume = document.getElementById('sliderVolume');
        el.valVolume = document.getElementById('valVolume');
        el.sliderCrossfade = document.getElementById('sliderCrossfade');
        el.valCrossfade = document.getElementById('valCrossfade');
        el.cfChips = document.querySelectorAll('.cf-chip');
        el.resetEqBtn = document.getElementById('resetEqBtn');
        el.sliderEqBass = document.getElementById('sliderEqBass');
        el.sliderEqMid = document.getElementById('sliderEqMid');
        el.sliderEqTreble = document.getElementById('sliderEqTreble');
        el.valEqBass = document.getElementById('valEqBass');
        el.valEqMid = document.getElementById('valEqMid');
        el.valEqTreble = document.getElementById('valEqTreble');
        el.toggleGapless = document.getElementById('toggleGapless');

        // Mini player
        el.miniPlayer = document.getElementById('miniPlayer');
        el.miniProgressFill = document.getElementById('miniProgressFill');
        el.miniPlayerContent = document.getElementById('miniPlayerContent');
        el.miniCoverImg = document.getElementById('miniCoverImg');
        el.miniCoverFallback = document.getElementById('miniCoverFallback');
        el.miniTrackTitle = document.getElementById('miniTrackTitle');
        el.miniTrackArtist = document.getElementById('miniTrackArtist');
        el.miniPlayPauseBtn = document.getElementById('miniPlayPauseBtn');
        el.miniPlayIcon = document.getElementById('miniPlayIcon');
        el.miniNextBtn = document.getElementById('miniNextBtn');

        // Full player
        el.fullPlayerModal = document.getElementById('fullPlayerModal');
        el.fullCloseBtn = document.getElementById('fullCloseBtn');
        el.fullSettingsBtn = document.getElementById('fullSettingsBtn');
        el.ambientGlow = document.getElementById('ambientGlow');
        el.fullCoverImg = document.getElementById('fullCoverImg');
        el.fullCoverFallback = document.getElementById('fullCoverFallback');
        el.fullTrackTitle = document.getElementById('fullTrackTitle');
        el.fullTrackArtist = document.getElementById('fullTrackArtist');
        el.fullLikeBtn = document.getElementById('fullLikeBtn');
        el.fullScrubber = document.getElementById('fullScrubber');
        el.fullCurrentTime = document.getElementById('fullCurrentTime');
        el.fullTotalTime = document.getElementById('fullTotalTime');
        el.fullShuffleBtn = document.getElementById('fullShuffleBtn');
        el.fullPrevBtn = document.getElementById('fullPrevBtn');
        el.fullPlayPauseBtn = document.getElementById('fullPlayPauseBtn');
        el.fullPlayPauseIcon = document.getElementById('fullPlayPauseIcon');
        el.fullNextBtn = document.getElementById('fullNextBtn');
        el.fullRepeatBtn = document.getElementById('fullRepeatBtn');

        // Toast & Fallback HTML5 Audio
        el.toastNotification = document.getElementById('toastNotification');
        el.toastMessage = document.getElementById('toastMessage');
        el.htmlAudioPlayer = document.getElementById('htmlAudioPlayer');
    }

    // INICIALIZACIÓN
    function init() {
        cacheDom();
        bindEvents();
        updateGreeting();

        // Escanear música real del teléfono
        syncLocalMusic();

        // Ticker de progreso
        setInterval(updatePlaybackProgress, 350);
    }

    function updateGreeting() {
        const h = new Date().getHours();
        let greeting = 'NOCHE';
        if (h >= 5 && h < 12) greeting = 'BUENOS DÍAS';
        else if (h >= 12 && h < 19) greeting = 'BUENAS TARDES';
        el.heroGreeting.textContent = `${greeting} • TITAN AUDIO`;
    }

    // GESTIÓN DE PESTAÑAS
    function switchTab(tabId) {
        state.currentTab = tabId;
        el.tabPanes.forEach(p => p.classList.toggle('active', p.id === tabId));
        el.navItems.forEach(n => n.classList.toggle('active', n.dataset.tab === tabId));
        const main = document.getElementById('mainScrollArea');
        if (main) main.scrollTop = 0;
    }

    // EVENTOS
    function bindEvents() {
        // Navegación
        el.navItems.forEach(item => {
            item.addEventListener('click', () => switchTab(item.dataset.tab));
        });

        // Acciones de importación y escaneo
        el.importFilesBtn.addEventListener('click', openFilePicker);
        el.heroPickerActionBtn.addEventListener('click', openFilePicker);
        el.refreshScanBtn.addEventListener('click', () => {
            syncLocalMusic();
            showToast('Escaneando archivos del teléfono...');
        });
        el.heroScanActionBtn.addEventListener('click', () => {
            syncLocalMusic();
            showToast('Escaneando archivos del teléfono...');
        });

        el.webAudioFileInput.addEventListener('change', handleWebFileInput);

        // Búsqueda en biblioteca
        el.libSearchInput.addEventListener('input', handleSearch);
        el.libSearchClearBtn.addEventListener('click', () => {
            el.libSearchInput.value = '';
            el.libSearchClearBtn.style.display = 'none';
            handleSearch();
        });

        // Ordenación de biblioteca
        el.sortChips.forEach(chip => {
            chip.addEventListener('click', () => {
                el.sortChips.forEach(c => c.classList.remove('active'));
                chip.classList.add('active');
                sortSongs(chip.dataset.sort);
            });
        });

        // Mini reproductor
        el.miniPlayerContent.addEventListener('click', (e) => {
            if (!e.target.closest('.mini-btn-play') && !e.target.closest('.mini-btn-ctrl')) {
                openFullPlayer();
            }
        });

        el.miniPlayPauseBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            togglePlayPause();
        });

        el.miniNextBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            playNext();
        });

        // Full reproductor
        el.fullCloseBtn.addEventListener('click', closeFullPlayer);
        el.fullSettingsBtn.addEventListener('click', () => {
            closeFullPlayer();
            switchTab('tabSettings');
        });
        el.fullPlayPauseBtn.addEventListener('click', togglePlayPause);
        el.fullPrevBtn.addEventListener('click', playPrevious);
        el.fullNextBtn.addEventListener('click', playNext);

        el.fullLikeBtn.addEventListener('click', () => {
            const active = el.fullLikeBtn.classList.toggle('active');
            showToast(active ? 'Añadida a favoritos' : 'Eliminada de favoritos');
        });

        el.fullShuffleBtn.addEventListener('click', () => {
            state.shuffle = !state.shuffle;
            el.fullShuffleBtn.classList.toggle('active', state.shuffle);
            showToast(state.shuffle ? 'Modo aleatorio activado' : 'Modo aleatorio desactivado');
        });

        el.fullRepeatBtn.addEventListener('click', () => {
            state.repeat = !state.repeat;
            el.fullRepeatBtn.classList.toggle('active', state.repeat);
            showToast(state.repeat ? 'Repetición activada' : 'Repetición desactivada');
        });

        // Scrubber
        el.fullScrubber.addEventListener('input', (e) => {
            const pct = parseFloat(e.target.value);
            const targetSec = (pct / 100) * state.duration;
            el.fullCurrentTime.textContent = formatTime(targetSec);
        });

        el.fullScrubber.addEventListener('change', (e) => {
            const pct = parseFloat(e.target.value);
            seekToSeconds((pct / 100) * state.duration);
        });

        // Ajustes Hi-Fi
        el.sliderVolume.addEventListener('input', (e) => {
            const val = parseInt(e.target.value, 10);
            setVolume(val);
        });

        el.sliderCrossfade.addEventListener('input', (e) => {
            const val = parseFloat(e.target.value);
            setCrossfade(val);
        });

        el.cfChips.forEach(chip => {
            chip.addEventListener('click', () => {
                el.cfChips.forEach(c => c.classList.remove('active'));
                chip.classList.add('active');
                const val = parseFloat(chip.dataset.cf);
                el.sliderCrossfade.value = val;
                setCrossfade(val);
            });
        });

        el.sliderEqBass.addEventListener('input', updateEq);
        el.sliderEqMid.addEventListener('input', updateEq);
        el.sliderEqTreble.addEventListener('input', updateEq);
        el.resetEqBtn.addEventListener('click', resetEq);

        el.toggleGapless.addEventListener('change', (e) => {
            state.gapless = e.target.checked;
            if (window.TitanBridge && typeof window.TitanBridge.setGapless === 'function') {
                window.TitanBridge.setGapless(state.gapless);
            }
            showToast(state.gapless ? 'Gapless activado' : 'Gapless desactivado');
        });

        // HTML5 Audio para entorno desktop
        el.htmlAudioPlayer.addEventListener('timeupdate', () => {
            if (el.htmlAudioPlayer.duration && !isNaN(el.htmlAudioPlayer.duration)) {
                state.position = Math.floor(el.htmlAudioPlayer.currentTime);
                state.duration = Math.floor(el.htmlAudioPlayer.duration);
                updateSeekDisplay();
            }
        });

        el.htmlAudioPlayer.addEventListener('ended', () => {
            playNext();
        });
    }

    // SELECTOR DE ARCHIVOS REAL DE ANDROID
    function openFilePicker() {
        if (window.TitanBridge && typeof window.TitanBridge.openAudioFilePicker === 'function') {
            window.TitanBridge.openAudioFilePicker();
            showToast('Abriendo selector de archivos del teléfono...');
        } else {
            el.webAudioFileInput.click();
        }
    }

    function handleWebFileInput(e) {
        const files = e.target.files;
        if (!files || files.length === 0) return;

        let added = 0;
        for (let i = 0; i < files.length; i++) {
            const file = files[i];
            const url = URL.createObjectURL(file);
            const title = file.name.replace(/\.[^/.]+$/, "");

            const track = {
                id: 'web_' + Date.now() + '_' + i,
                title: title,
                artist: 'Archivo importado',
                album: 'Navegador',
                duration: 180,
                path: url,
                blobUrl: url
            };

            state.songs.unshift(track);
            added++;
            if (i === 0) {
                playTrack(track);
            }
        }

        renderLibrary();
        showToast(`${added} canción(es) importadas`);
    }

    // CALLBACK INVOCADO DESDE ANDROID CUANDO SE SELECCIONAN ARCHIVOS
    window.onLocalTracksImported = function (jsonString) {
        try {
            const imported = JSON.parse(jsonString);
            if (Array.isArray(imported) && imported.length > 0) {
                // Agregar al inicio de la lista
                state.songs = [...imported, ...state.songs];
                renderLibrary();
                // Reproducir la primera canción importada inmediatamente
                playTrack(imported[0]);
                showToast(`${imported.length} canción(es) agregadas`);
            }
        } catch (e) {
            console.error('Error parseando canciones importadas:', e);
        }
    };

    window.onPermissionsGranted = function () {
        syncLocalMusic();
        showToast('Permisos concedidos: analizando música');
    };

    // CALLBACKS DESDE NOTIFICACIONES DEL SISTEMA Y BOTONES DE AURICULARES
    window.onNativePlaybackStateChanged = function (playing, title, artist) {
        state.isPlaying = playing;
        updatePlayIcons(playing);
        if (title && el.miniTrackTitle) {
            el.miniTrackTitle.textContent = title;
            el.fullTrackTitle.textContent = title;
        }
        if (artist && el.miniTrackArtist) {
            el.miniTrackArtist.textContent = artist;
            el.fullTrackArtist.textContent = artist;
        }
    };

    window.playNextTrack = function () {
        playNext();
    };

    window.playPreviousTrack = function () {
        playPrevious();
    };

    // =========================================================================
    // REPRODUCCIÓN REAL DE AUDIO
    // =========================================================================

    function playTrack(track) {
        state.currentTrack = track;
        state.duration = track.duration || 180;
        state.position = 0;
        state.isPlaying = true;

        // Metadatos en UI
        el.miniTrackTitle.textContent = track.title;
        el.miniTrackArtist.textContent = track.artist;
        el.fullTrackTitle.textContent = track.title;
        el.fullTrackArtist.textContent = track.artist;
        el.fullTotalTime.textContent = formatTime(state.duration);
        el.fullCurrentTime.textContent = '0:00';
        el.fullScrubber.value = 0;

        // Cargar carátula real
        loadArtwork(track);

        el.miniPlayer.style.display = 'flex';
        updatePlayIcons(true);
        renderActiveTrackHighlight();

        // 1. Reproducir en Android nativo
        if (window.TitanBridge && typeof window.TitanBridge.playTrack === 'function') {
            try {
                window.TitanBridge.playTrack(track.path, track.title, track.artist, false);
            } catch (err) {
                console.warn('Error en TitanBridge.playTrack:', err);
            }
        }

        // 2. Si es en navegador web (preview de escritorio)
        if (!window.TitanBridge) {
            try {
                if (track.blobUrl) {
                    el.htmlAudioPlayer.src = track.blobUrl;
                } else {
                    el.htmlAudioPlayer.src = 'audio/track_rock.wav';
                }
                el.htmlAudioPlayer.volume = state.volume / 100.0;
                el.htmlAudioPlayer.play().catch(e => console.warn('HTML5 Audio:', e));
            } catch (e) {
                console.error('Error en HTML5 Audio:', e);
            }
        }

        showToast(track.title);
    }

    function loadArtwork(track) {
        // Comprobar si ya la tenemos en cache
        if (state.artworksCache[track.path]) {
            setArtworkDisplay(state.artworksCache[track.path]);
            return;
        }

        // Consultar carátula embebida al sistema nativo
        if (window.TitanBridge && typeof window.TitanBridge.getEmbeddedArtwork === 'function') {
            try {
                const b64 = window.TitanBridge.getEmbeddedArtwork(track.path);
                if (b64 && b64.length > 30) {
                    state.artworksCache[track.path] = b64;
                    setArtworkDisplay(b64);
                    return;
                }
            } catch (ignored) {}
        }

        // Fallback a icono vectorial
        setArtworkDisplay(null);
    }

    function setArtworkDisplay(imgData) {
        if (imgData) {
            el.miniCoverImg.src = imgData;
            el.miniCoverImg.style.display = 'block';
            el.miniCoverFallback.style.display = 'none';

            el.fullCoverImg.src = imgData;
            el.fullCoverImg.style.display = 'block';
            el.fullCoverFallback.style.display = 'none';
        } else {
            el.miniCoverImg.style.display = 'none';
            el.miniCoverFallback.style.display = 'flex';

            el.fullCoverImg.style.display = 'none';
            el.fullCoverFallback.style.display = 'flex';
        }
    }

    function togglePlayPause() {
        if (!state.currentTrack && state.songs.length > 0) {
            playTrack(state.songs[0]);
            return;
        }

        state.isPlaying = !state.isPlaying;
        updatePlayIcons(state.isPlaying);

        if (state.isPlaying) {
            if (window.TitanBridge && typeof window.TitanBridge.resumeTrack === 'function') {
                window.TitanBridge.resumeTrack();
            } else {
                el.htmlAudioPlayer.play().catch(() => {});
            }
        } else {
            if (window.TitanBridge && typeof window.TitanBridge.pauseTrack === 'function') {
                window.TitanBridge.pauseTrack();
            } else {
                el.htmlAudioPlayer.pause();
            }
        }
    }

    function updatePlayIcons(playing) {
        const playSvg = '<path d="M8 5v14l11-7z"/>';
        const pauseSvg = '<path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/>';

        el.miniPlayIcon.innerHTML = playing ? pauseSvg : playSvg;
        el.fullPlayPauseIcon.innerHTML = playing ? pauseSvg : playSvg;
    }

    function playNext() {
        if (state.songs.length === 0) return;
        const currentIdx = state.songs.findIndex(s => s.id === (state.currentTrack ? state.currentTrack.id : ''));
        let nextIdx = 0;
        if (state.shuffle) {
            nextIdx = Math.floor(Math.random() * state.songs.length);
        } else if (currentIdx >= 0 && currentIdx < state.songs.length - 1) {
            nextIdx = currentIdx + 1;
        }
        playTrack(state.songs[nextIdx]);
    }

    function playPrevious() {
        if (state.position > 3) {
            seekToSeconds(0);
            return;
        }
        if (state.songs.length === 0) return;
        const currentIdx = state.songs.findIndex(s => s.id === (state.currentTrack ? state.currentTrack.id : ''));
        let prevIdx = state.songs.length - 1;
        if (currentIdx > 0) {
            prevIdx = currentIdx - 1;
        }
        playTrack(state.songs[prevIdx]);
    }

    function seekToSeconds(seconds) {
        state.position = Math.floor(seconds);
        if (window.TitanBridge && typeof window.TitanBridge.seekTo === 'function') {
            window.TitanBridge.seekTo(state.position);
        }
        if (!window.TitanBridge && el.htmlAudioPlayer.duration) {
            el.htmlAudioPlayer.currentTime = seconds % el.htmlAudioPlayer.duration;
        }
        updateSeekDisplay();
    }

    function updatePlaybackProgress() {
        if (!state.isPlaying) return;

        if (window.TitanBridge && typeof window.TitanBridge.getPlaybackStatus === 'function') {
            try {
                const st = JSON.parse(window.TitanBridge.getPlaybackStatus());
                if (st.duration && st.duration > 0) {
                    state.duration = st.duration;
                    state.position = st.position;
                    state.isPlaying = st.playing;
                }
            } catch (ignored) {}
        } else if (!window.TitanBridge && el.htmlAudioPlayer.duration) {
            state.position = Math.floor(el.htmlAudioPlayer.currentTime);
        } else if (state.isPlaying) {
            state.position += 1;
            if (state.position >= state.duration) {
                state.position = 0;
                playNext();
            }
        }

        updateSeekDisplay();
    }

    function updateSeekDisplay() {
        const pct = state.duration > 0 ? (state.position / state.duration) * 100 : 0;
        el.miniProgressFill.style.width = `${pct}%`;
        el.fullScrubber.value = pct;
        el.fullCurrentTime.textContent = formatTime(state.position);
        el.fullTotalTime.textContent = formatTime(state.duration);
    }

    function openFullPlayer() {
        el.fullPlayerModal.classList.add('open');
    }

    function closeFullPlayer() {
        el.fullPlayerModal.classList.remove('open');
    }

    // =========================================================================
    // AJUSTES HI-FI
    // =========================================================================

    function setVolume(val) {
        state.volume = val;
        el.valVolume.textContent = `${val}%`;
        el.sliderVolume.value = val;
        el.htmlAudioPlayer.volume = val / 100.0;
        if (window.TitanBridge && typeof window.TitanBridge.setVolume === 'function') {
            window.TitanBridge.setVolume(val);
        }
    }

    function setCrossfade(seconds) {
        state.crossfade = seconds;
        el.valCrossfade.textContent = seconds === 0 ? 'Desactivado' : `${seconds.toFixed(1)}s`;
        if (window.TitanBridge && typeof window.TitanBridge.setCrossfade === 'function') {
            window.TitanBridge.setCrossfade(seconds);
        }
    }

    function updateEq() {
        const bass = parseInt(el.sliderEqBass.value, 10);
        const mid = parseInt(el.sliderEqMid.value, 10);
        const treble = parseInt(el.sliderEqTreble.value, 10);

        state.eq = { bass, mid, treble };
        el.valEqBass.textContent = `${bass > 0 ? '+' : ''}${bass} dB`;
        el.valEqMid.textContent = `${mid > 0 ? '+' : ''}${mid} dB`;
        el.valEqTreble.textContent = `${treble > 0 ? '+' : ''}${treble} dB`;

        if (window.TitanBridge && typeof window.TitanBridge.setEqualizer === 'function') {
            window.TitanBridge.setEqualizer(bass, mid, treble);
        }
    }

    function resetEq() {
        el.sliderEqBass.value = 0;
        el.sliderEqMid.value = 0;
        el.sliderEqTreble.value = 0;
        updateEq();
        showToast('Ecualizador restablecido');
    }

    // =========================================================================
    // LECTURA DE CANCIONES DEL DISPOSITIVO (MEDIASTORE)
    // =========================================================================

    function syncLocalMusic() {
        if (!window.TitanBridge || typeof window.TitanBridge.scanLocalMusic !== 'function') {
            el.libraryCountText.textContent = 'Modo Web: Importa tus canciones con «Abrir archivos»';
            el.totalTracksBadge.textContent = state.songs.length;
            renderLibrary();
            return;
        }

        try {
            const jsonStr = window.TitanBridge.scanLocalMusic();
            const localTracks = JSON.parse(jsonStr);

            if (Array.isArray(localTracks) && localTracks.length > 0) {
                state.songs = localTracks;
                el.libraryCountText.textContent = `${localTracks.length} pistas detectadas en el teléfono`;
                el.totalTracksBadge.textContent = localTracks.length;
                // Si no hay canción en curso, preparar la primera
                if (!state.currentTrack) {
                    state.currentTrack = localTracks[0];
                    el.miniTrackTitle.textContent = localTracks[0].title;
                    el.miniTrackArtist.textContent = localTracks[0].artist;
                    loadArtwork(localTracks[0]);
                    el.miniPlayer.style.display = 'flex';
                }
            } else {
                el.libraryCountText.textContent = 'Sin canciones detectadas. Pulsa «Abrir archivos» para seleccionar tus MP3';
                el.totalTracksBadge.textContent = '0';
            }
            renderLibrary();
        } catch (e) {
            console.error('Error al sincronizar música local:', e);
            el.libraryCountText.textContent = 'Pulsa «Abrir archivos» para cargar música';
        }
    }

    // =========================================================================
    // RENDERIZADO DE BIBLIOTECA
    // =========================================================================

    function renderLibrary() {
        renderTrackList(el.homeTracksList, state.songs.slice(0, 8));
        renderTrackList(el.fullLibraryList, state.filteredSongs.length > 0 ? state.filteredSongs : state.songs);
        el.totalTracksBadge.textContent = state.songs.length;
    }

    function renderTrackList(container, tracks) {
        if (!container) return;
        container.innerHTML = '';

        if (!tracks || tracks.length === 0) {
            container.innerHTML = `
                <div style="padding: 32px 16px; text-align: center; color: var(--text-muted);">
                    <p style="font-size: 14px; font-weight: 600;">No hay pistas para mostrar</p>
                    <p style="font-size: 12px; margin-top: 4px;">Pulsa «Abrir archivos» para agregar canciones desde tu teléfono</p>
                </div>
            `;
            return;
        }

        tracks.forEach(track => {
            const isPlayingThis = state.currentTrack && state.currentTrack.id === track.id;
            const row = document.createElement('div');
            row.className = `track-row ${isPlayingThis ? 'playing' : ''}`;
            row.dataset.id = track.id;

            row.innerHTML = `
                <div class="track-cover-box">
                    <div class="track-cover-fallback">
                        <svg viewBox="0 0 24 24" width="20" height="20" fill="${isPlayingThis ? '#00e5ff' : '#8f9cae'}">
                            <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
                        </svg>
                    </div>
                </div>
                <div class="track-meta">
                    <div class="track-title">${escapeHtml(track.title)}</div>
                    <div class="track-sub">${escapeHtml(track.artist)} • ${escapeHtml(track.album || 'Dispositivo')}</div>
                </div>
                <div class="track-time">${formatTime(track.duration)}</div>
            `;

            row.addEventListener('click', () => playTrack(track));
            container.appendChild(row);
        });
    }

    function renderActiveTrackHighlight() {
        document.querySelectorAll('.track-row').forEach(row => {
            const isCurrent = state.currentTrack && row.dataset.id === state.currentTrack.id;
            row.classList.toggle('playing', isCurrent);
        });
    }

    function handleSearch() {
        const q = el.libSearchInput.value.toLowerCase().trim();
        if (!q) {
            state.filteredSongs = [];
            el.libSearchClearBtn.style.display = 'none';
        } else {
            el.libSearchClearBtn.style.display = 'block';
            state.filteredSongs = state.songs.filter(s =>
                s.title.toLowerCase().includes(q) ||
                s.artist.toLowerCase().includes(q) ||
                (s.album && s.album.toLowerCase().includes(q))
            );
        }
        renderTrackList(el.fullLibraryList, state.filteredSongs.length > 0 ? state.filteredSongs : (q ? [] : state.songs));
    }

    function sortSongs(criteria) {
        if (criteria === 'title') {
            state.songs.sort((a, b) => a.title.localeCompare(b.title));
        } else if (criteria === 'artist') {
            state.songs.sort((a, b) => a.artist.localeCompare(b.artist));
        } else if (criteria === 'duration') {
            state.songs.sort((a, b) => (b.duration || 0) - (a.duration || 0));
        }
        renderLibrary();
    }

    // BOTÓN ATRÁS EN ANDROID
    window.handleAndroidBack = function () {
        if (el.fullPlayerModal.classList.contains('open')) {
            closeFullPlayer();
            return 'handled';
        }
        if (state.currentTab !== 'tabHome') {
            switchTab('tabHome');
            return 'handled';
        }
        return 'back';
    };

    // UTILIDADES
    function formatTime(secs) {
        if (!secs || isNaN(secs)) return '0:00';
        const m = Math.floor(secs / 60);
        const s = Math.floor(secs % 60);
        return `${m}:${s < 10 ? '0' : ''}${s}`;
    }

    function escapeHtml(str) {
        if (!str) return '';
        return String(str)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    let toastTimer = null;
    function showToast(msg) {
        el.toastMessage.textContent = msg;
        el.toastNotification.classList.add('show');
        if (toastTimer) clearTimeout(toastTimer);
        toastTimer = setTimeout(() => {
            el.toastNotification.classList.remove('show');
        }, 2200);
    }

    document.addEventListener('DOMContentLoaded', init);

})();

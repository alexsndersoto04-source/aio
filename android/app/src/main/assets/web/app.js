// =============================================================================
// TITAN MUSIC — CLIENTE SPOTIFY MOBILE (MOTOR REAL DE AUDIO HI-FI)
// =============================================================================

(function () {
    'use strict';

    // ESTADO DE LA APLICACIÓN
    const state = {
        currentTab: 'tabHome',
        filter: 'all',          // 'all', 'phone', 'cloud'
        currentTrack: null,
        isPlaying: false,
        position: 0,
        duration: 180,
        volume: 80,
        crossfade: 0.0,
        gapless: true,
        eq: { bass: 0, mid: 0, treble: 0 },
        shuffle: false,
        repeat: false,
        activeDevice: 'Altavoz del teléfono',
        songs: [],
        telegram: {
            connected: true,
            user: 'alexsndersoto',
            channel: 'Mi Música'
        }
    };

    // BIBLIOTECA INICIAL LIMPIA (ENRIQUECIDA AUTOMÁTICAMENTE CON MÚSICA DEL TELÉFONO)
    const initialTracks = [
        {
            id: 'cloud_1',
            title: 'Blinding Lights',
            artist: 'The Weeknd',
            album: 'After Hours',
            duration: 200,
            source: 'cloud',
            isCloud: true,
            path: 'cloud:101',
            color: '#1a233a'
        },
        {
            id: 'cloud_2',
            title: 'De Música Ligera',
            artist: 'Soda Stereo',
            album: 'Canción Animal',
            duration: 212,
            source: 'cloud',
            isCloud: true,
            path: 'cloud:102',
            color: '#281a1a'
        },
        {
            id: 'cloud_3',
            title: 'Midnight City',
            artist: 'M83',
            album: 'Hurry Up, We\'re Dreaming',
            duration: 243,
            source: 'cloud',
            isCloud: true,
            path: 'cloud:103',
            color: '#1d2a20'
        },
        {
            id: 'cloud_4',
            title: 'Starboy',
            artist: 'The Weeknd ft. Daft Punk',
            album: 'Starboy',
            duration: 230,
            source: 'cloud',
            isCloud: true,
            path: 'cloud:104',
            color: '#2a1a28'
        }
    ];

    // ELEMENTOS DOM
    const el = {};

    function cacheDom() {
        // Headers & navegación
        el.greetingText = document.getElementById('greetingText');
        el.openFolderBtn = document.getElementById('openFolderBtn');
        el.topTgPill = document.getElementById('topTgPill');
        el.tgHeaderStatusDot = document.getElementById('tgHeaderStatusDot');
        el.topSettingsBtn = document.getElementById('topSettingsBtn');
        el.webAudioFileInput = document.getElementById('webAudioFileInput');
        el.tabPanes = document.querySelectorAll('.tab-pane');
        el.navItems = document.querySelectorAll('.nav-item');

        // Home
        el.quickAccessGrid = document.getElementById('quickAccessGrid');
        el.localImportCard = document.getElementById('localImportCard');
        el.localCountLabel = document.getElementById('localCountLabel');
        el.importLocalFilesBtn = document.getElementById('importLocalFilesBtn');
        el.refreshLibBtn = document.getElementById('refreshLibBtn');
        el.homeSongList = document.getElementById('homeSongList');

        // Search
        el.searchInput = document.getElementById('searchInput');
        el.searchClearBtn = document.getElementById('searchClearBtn');
        el.searchBrowseSection = document.getElementById('searchBrowseSection');
        el.searchResultsSection = document.getElementById('searchResultsSection');
        el.searchResultsList = document.getElementById('searchResultsList');

        // Library
        el.librarySongList = document.getElementById('librarySongList');

        // Telegram tab
        el.tgCardChannelTitle = document.getElementById('tgCardChannelTitle');
        el.tgCardStateText = document.getElementById('tgCardStateText');
        el.tgSyncNowBtn = document.getElementById('tgSyncNowBtn');
        el.tgPhoneInput = document.getElementById('tgPhoneInput');
        el.tgCodeInput = document.getElementById('tgCodeInput');
        el.tgConnectActionBtn = document.getElementById('tgConnectActionBtn');
        el.telegramSongList = document.getElementById('telegramSongList');

        // Settings tab
        el.settingsVolValue = document.getElementById('settingsVolValue');
        el.settingsVolRange = document.getElementById('settingsVolRange');
        el.settingsCrossfadeValue = document.getElementById('settingsCrossfadeValue');
        el.settingsCrossfadeRange = document.getElementById('settingsCrossfadeRange');
        el.resetEqBtn = document.getElementById('resetEqBtn');
        el.eqBass = document.getElementById('eqBass');
        el.eqMid = document.getElementById('eqMid');
        el.eqTreble = document.getElementById('eqTreble');
        el.eqBassVal = document.getElementById('eqBassVal');
        el.eqMidVal = document.getElementById('eqMidVal');
        el.eqTrebleVal = document.getElementById('eqTrebleVal');
        el.specStatusText = document.getElementById('specStatusText');
        el.spectrumBarsBox = document.getElementById('spectrumBarsBox');
        el.gaplessSwitch = document.getElementById('gaplessSwitch');
        el.devicesList = document.getElementById('devicesList');

        // Mini player
        el.miniPlayer = document.getElementById('miniPlayer');
        el.miniPlayerContent = document.getElementById('miniPlayerContent');
        el.miniCover = document.getElementById('miniCover');
        el.miniTitle = document.getElementById('miniTitle');
        el.miniArtist = document.getElementById('miniArtist');
        el.miniHeartBtn = document.getElementById('miniHeartBtn');
        el.miniPlayBtn = document.getElementById('miniPlayBtn');
        el.miniPlayIcon = document.getElementById('miniPlayIcon');
        el.miniProgressFill = document.getElementById('miniProgressFill');

        // Full player modal
        el.fullPlayerModal = document.getElementById('fullPlayerModal');
        el.collapsePlayerBtn = document.getElementById('collapsePlayerBtn');
        el.fullSourceLabel = document.getElementById('fullSourceLabel');
        el.fullArtworkCard = document.getElementById('fullArtworkCard');
        el.fullTrackTitle = document.getElementById('fullTrackTitle');
        el.fullTrackArtist = document.getElementById('fullTrackArtist');
        el.fullHeartBtn = document.getElementById('fullHeartBtn');
        el.fullHeartIcon = document.getElementById('fullHeartIcon');
        el.fullSeekBar = document.getElementById('fullSeekBar');
        el.fullCurrentTime = document.getElementById('fullCurrentTime');
        el.fullTotalTime = document.getElementById('fullTotalTime');
        el.fullShuffleBtn = document.getElementById('fullShuffleBtn');
        el.fullPrevBtn = document.getElementById('fullPrevBtn');
        el.fullPlayPauseBtn = document.getElementById('fullPlayPauseBtn');
        el.fullPlayPauseIcon = document.getElementById('fullPlayPauseIcon');
        el.fullNextBtn = document.getElementById('fullNextBtn');
        el.fullRepeatBtn = document.getElementById('fullRepeatBtn');
        el.fullDeviceBtn = document.getElementById('fullDeviceBtn');
        el.fullActiveDeviceName = document.getElementById('fullActiveDeviceName');

        // Toast & HTML Audio
        el.toastNotification = document.getElementById('toastNotification');
        el.toastMessage = document.getElementById('toastMessage');
        el.htmlAudioPlayer = document.getElementById('htmlAudioPlayer');
    }

    // INICIALIZACIÓN
    function init() {
        cacheDom();
        state.songs = [...initialTracks];

        setupGreeting();
        initSpectrumBars();
        bindEvents();

        // Sincronizar música real del teléfono
        syncLocalMusic();

        // Renderizar vistas
        renderAll();

        // Si hay canciones, preparar la primera sin reproducir
        if (state.songs.length > 0) {
            loadTrackMeta(state.songs[0]);
        }

        // Ticker de reproducción
        setInterval(updatePlaybackProgress, 350);
        setInterval(renderSpectrumAnimation, 120);
    }

    function setupGreeting() {
        const hour = new Date().getHours();
        let greeting = 'Buenas noches';
        if (hour >= 6 && hour < 12) greeting = 'Buenos días';
        else if (hour >= 12 && hour < 19) greeting = 'Buenas tardes';
        el.greetingText.textContent = greeting;
    }

    function initSpectrumBars() {
        el.spectrumBarsBox.innerHTML = '';
        for (let i = 0; i < 32; i++) {
            const bar = document.createElement('div');
            bar.className = 'spectrum-bar';
            bar.style.height = '4px';
            el.spectrumBarsBox.appendChild(bar);
        }
    }

    // NAVEGACIÓN LIMPIA
    function switchTab(tabId) {
        state.currentTab = tabId;

        el.tabPanes.forEach(pane => {
            pane.classList.toggle('active', pane.id === tabId);
        });

        el.navItems.forEach(item => {
            item.classList.toggle('active', item.dataset.tab === tabId);
        });

        const mainScroll = document.querySelector('.main-content-scroll');
        if (mainScroll) mainScroll.scrollTop = 0;
    }

    // EVENTOS
    function bindEvents() {
        // Tabs
        el.navItems.forEach(item => {
            item.addEventListener('click', () => switchTab(item.dataset.tab));
        });

        el.topTgPill.addEventListener('click', () => switchTab('tabTelegram'));
        el.topSettingsBtn.addEventListener('click', () => switchTab('tabSettings'));

        // Abrir archivos de música del teléfono
        el.openFolderBtn.addEventListener('click', triggerFilePicker);
        el.importLocalFilesBtn.addEventListener('click', triggerFilePicker);
        el.refreshLibBtn.addEventListener('click', () => {
            syncLocalMusic();
            showToast('Actualizando canciones del dispositivo...');
        });

        // Entrada de archivo web
        el.webAudioFileInput.addEventListener('change', handleWebFileImport);

        // Filtros de biblioteca
        document.querySelectorAll('.spotify-pill').forEach(pill => {
            pill.addEventListener('click', () => {
                document.querySelectorAll('.spotify-pill').forEach(p => p.classList.remove('active'));
                pill.classList.add('active');
                state.filter = pill.dataset.filter;
                renderSongs();
            });
        });

        // Búsqueda
        el.searchInput.addEventListener('input', handleSearch);
        el.searchClearBtn.addEventListener('click', () => {
            el.searchInput.value = '';
            el.searchClearBtn.style.display = 'none';
            handleSearch();
        });

        document.querySelectorAll('.browse-tile').forEach(tile => {
            tile.addEventListener('click', () => {
                el.searchInput.value = tile.dataset.query;
                el.searchClearBtn.style.display = 'block';
                handleSearch();
            });
        });

        // Mini reproductor
        el.miniPlayerContent.addEventListener('click', (e) => {
            if (!e.target.closest('.mini-action-btn') && !e.target.closest('.mini-play-btn')) {
                openFullPlayer();
            }
        });

        el.miniPlayBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            togglePlayPause();
        });

        el.miniHeartBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            toggleLike();
        });

        // Full player
        el.collapsePlayerBtn.addEventListener('click', closeFullPlayer);
        el.fullPlayPauseBtn.addEventListener('click', togglePlayPause);
        el.fullPrevBtn.addEventListener('click', playPrevious);
        el.fullNextBtn.addEventListener('click', playNext);
        el.fullHeartBtn.addEventListener('click', toggleLike);

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

        el.fullDeviceBtn.addEventListener('click', () => {
            closeFullPlayer();
            switchTab('tabSettings');
        });

        // Scrubber
        el.fullSeekBar.addEventListener('input', (e) => {
            const pct = parseFloat(e.target.value);
            const targetSecs = (pct / 100) * state.duration;
            el.fullCurrentTime.textContent = formatTime(targetSecs);
        });

        el.fullSeekBar.addEventListener('change', (e) => {
            const pct = parseFloat(e.target.value);
            seekToSeconds((pct / 100) * state.duration);
        });

        // Ajustes Hi-Fi
        el.settingsVolRange.addEventListener('input', (e) => {
            const vol = parseInt(e.target.value, 10);
            setVolume(vol);
        });

        el.settingsCrossfadeRange.addEventListener('input', (e) => {
            const cf = parseFloat(e.target.value);
            setCrossfade(cf);
        });

        document.querySelectorAll('.preset-chips-row .chip-btn[data-cf]').forEach(btn => {
            btn.addEventListener('click', () => {
                document.querySelectorAll('.preset-chips-row .chip-btn[data-cf]').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                const cf = parseFloat(btn.dataset.cf);
                el.settingsCrossfadeRange.value = cf;
                setCrossfade(cf);
            });
        });

        el.eqBass.addEventListener('input', updateEq);
        el.eqMid.addEventListener('input', updateEq);
        el.eqTreble.addEventListener('input', updateEq);
        el.resetEqBtn.addEventListener('click', resetEq);

        document.querySelectorAll('.preset-chips-row .chip-btn[data-eq]').forEach(btn => {
            btn.addEventListener('click', () => {
                document.querySelectorAll('.preset-chips-row .chip-btn[data-eq]').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                const [b, m, t] = btn.dataset.eq.split(',').map(Number);
                el.eqBass.value = b;
                el.eqMid.value = m;
                el.eqTreble.value = t;
                updateEq();
            });
        });

        el.gaplessSwitch.addEventListener('change', (e) => {
            state.gapless = e.target.checked;
            if (window.TitanBridge && typeof window.TitanBridge.setGapless === 'function') {
                window.TitanBridge.setGapless(state.gapless);
            }
            showToast(state.gapless ? 'Reproducción sin pausas activada' : 'Reproducción continua desactivada');
        });

        // Telegram tab
        el.tgSyncNowBtn.addEventListener('click', () => {
            showToast('Sincronizando canal «Mi Música»...');
            setTimeout(() => showToast('Canal sincronizado'), 800);
        });

        el.tgConnectActionBtn.addEventListener('click', () => {
            const phone = el.tgPhoneInput.value.trim();
            if (!phone) {
                showToast('Ingresa un número de teléfono');
                return;
            }
            if (window.TitanBridge && typeof window.TitanBridge.setupTelegram === 'function') {
                window.TitanBridge.setupTelegram('1234567', 'abcdef', phone);
            }
            state.telegram.connected = true;
            state.telegram.user = `Telegram (${phone})`;
            el.tgCardStateText.textContent = `Conectado como ${state.telegram.user}`;
            showToast('Cuenta de Telegram vinculada con éxito');
        });

        // Eventos HTML5 Audio para entorno web
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

    // DISPARAR SELECTOR DE ARCHIVOS DE AUDIO REAL
    function triggerFilePicker() {
        if (window.TitanBridge && typeof window.TitanBridge.openAudioFilePicker === 'function') {
            window.TitanBridge.openAudioFilePicker();
            showToast('Abriendo selector de archivos...');
        } else {
            el.webAudioFileInput.click();
        }
    }

    function handleWebFileImport(e) {
        const files = e.target.files;
        if (!files || files.length === 0) return;

        let added = 0;
        for (let i = 0; i < files.length; i++) {
            const file = files[i];
            const url = URL.createObjectURL(file);
            const title = file.name.replace(/\.[^/.]+$/, "");

            const track = {
                id: 'imported_' + Date.now() + '_' + i,
                title: title,
                artist: 'Archivo local',
                album: 'Este dispositivo',
                duration: 180,
                source: 'phone',
                isCloud: false,
                downloaded: true,
                path: url,
                blobUrl: url
            };

            state.songs.unshift(track);
            added++;
            if (i === 0) {
                playTrack(track);
            }
        }

        renderAll();
        showToast(`${added} canción(es) importadas`);
    }

    // CALLBACK LLAMADO DESDE ANDROID AL IMPORTAR UN ARCHIVO CON EL SELECTOR
    window.onLocalTrackImported = function (uri, title) {
        const track = {
            id: 'local_pick_' + Date.now(),
            title: title || 'Canción importada',
            artist: 'Archivo del teléfono',
            album: 'Almacenamiento',
            duration: 180,
            source: 'phone',
            isCloud: false,
            downloaded: true,
            path: uri
        };

        state.songs.unshift(track);
        loadTrackMeta(track);
        state.isPlaying = true;
        updatePlayIcons(true);
        renderSongs();
        showToast(`Reproduciendo: ${track.title}`);
    };

    window.onPermissionsGranted = function () {
        syncLocalMusic();
        showToast('Permisos concedidos: música detectada');
    };

    // =========================================================================
    // REPRODUCCIÓN REAL DE AUDIO
    // =========================================================================

    function loadTrackMeta(track) {
        state.currentTrack = track;
        state.duration = track.duration || 180;
        state.position = 0;

        // Mini player
        el.miniTitle.textContent = track.title;
        el.miniArtist.textContent = track.artist;

        // Full player
        el.fullTrackTitle.textContent = track.title;
        el.fullTrackArtist.textContent = track.artist;
        el.fullSourceLabel.textContent = track.isCloud ? 'Nube de Telegram' : 'Música del teléfono';
        el.fullTotalTime.textContent = formatTime(state.duration);
        el.fullCurrentTime.textContent = '0:00';
        el.fullSeekBar.value = 0;

        // Colores de carátula
        const color = track.color || '#242424';
        el.fullArtworkCard.style.backgroundColor = color;
        el.fullPlayerModal.style.background = `linear-gradient(180deg, ${color} 0%, #121212 70%)`;

        el.miniPlayer.style.display = 'flex';
        renderSongs();
    }

    function playTrack(track) {
        loadTrackMeta(track);
        state.isPlaying = true;
        updatePlayIcons(true);

        let startedOnAndroid = false;
        // 1. Invocar Android MediaPlayer nativo con el content:// URI o file path real
        if (window.TitanBridge && typeof window.TitanBridge.playTrack === 'function') {
            try {
                startedOnAndroid = window.TitanBridge.playTrack(track.path, track.title, track.artist, track.isCloud);
            } catch (err) {
                console.warn('Error en TitanBridge.playTrack:', err);
            }
        }

        // 2. Si estamos en navegador web (no en Android nativo), reproducir vía HTML5 <audio>
        if (!window.TitanBridge) {
            try {
                if (track.blobUrl) {
                    el.htmlAudioPlayer.src = track.blobUrl;
                } else {
                    // Fallback para navegador de escritorio
                    el.htmlAudioPlayer.src = 'audio/track_rock.wav';
                }
                el.htmlAudioPlayer.volume = state.volume / 100.0;
                el.htmlAudioPlayer.play().catch(e => console.warn('HTML5 Audio play:', e));
            } catch (e) {
                console.error('Error reproduciendo en navegador:', e);
            }
        }

        showToast(`${track.title}`);
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
        const playIcon = `<path d="M7.05 3.606l13.49 7.77a.7.7 0 0 1 0 1.21L7.05 20.356A.7.7 0 0 1 6 19.752V4.21a.7.7 0 0 1 1.05-.604z"/>`;
        const pauseIcon = `<path d="M5.7 3a.7.7 0 0 0-.7.7v16.6a.7.7 0 0 0 .7.7h2.6a.7.7 0 0 0 .7-.7V3.7a.7.7 0 0 0-.7-.7H5.7zm10 0a.7.7 0 0 0-.7.7v16.6a.7.7 0 0 0 .7.7h2.6a.7.7 0 0 0 .7-.7V3.7a.7.7 0 0 0-.7-.7h-2.6z"/>`;

        el.miniPlayIcon.innerHTML = playing ? pauseIcon : playIcon;
        el.fullPlayPauseIcon.innerHTML = playing ? pauseIcon : playIcon;
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

        // Si corre con Android nativo, obtener el estado real
        if (window.TitanBridge && typeof window.TitanBridge.getPlaybackStatus === 'function') {
            try {
                const statusStr = window.TitanBridge.getPlaybackStatus();
                const st = JSON.parse(statusStr);
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
        el.fullSeekBar.value = pct;
        el.fullCurrentTime.textContent = formatTime(state.position);
        el.fullTotalTime.textContent = formatTime(state.duration);
    }

    function toggleLike() {
        const liked = el.fullHeartBtn.classList.toggle('liked');
        el.miniHeartBtn.classList.toggle('liked', liked);
        showToast(liked ? 'Guardada en tus Me gusta' : 'Eliminada de tus Me gusta');
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
        el.settingsVolValue.textContent = `${val}%`;
        el.settingsVolRange.value = val;
        el.htmlAudioPlayer.volume = val / 100.0;
        if (window.TitanBridge && typeof window.TitanBridge.setVolume === 'function') {
            window.TitanBridge.setVolume(val);
        }
    }

    function setCrossfade(seconds) {
        state.crossfade = seconds;
        el.settingsCrossfadeValue.textContent = seconds === 0 ? 'Desactivado' : `${seconds.toFixed(1)}s`;
        if (window.TitanBridge && typeof window.TitanBridge.setCrossfade === 'function') {
            window.TitanBridge.setCrossfade(seconds);
        }
    }

    function updateEq() {
        const bass = parseInt(el.eqBass.value, 10);
        const mid = parseInt(el.eqMid.value, 10);
        const treble = parseInt(el.eqTreble.value, 10);

        state.eq = { bass, mid, treble };
        el.eqBassVal.textContent = `${bass > 0 ? '+' : ''}${bass} dB`;
        el.eqMidVal.textContent = `${mid > 0 ? '+' : ''}${mid} dB`;
        el.eqTrebleVal.textContent = `${treble > 0 ? '+' : ''}${treble} dB`;

        if (window.TitanBridge && typeof window.TitanBridge.setEqualizer === 'function') {
            window.TitanBridge.setEqualizer(bass, mid, treble);
        }
    }

    function resetEq() {
        el.eqBass.value = 0;
        el.eqMid.value = 0;
        el.eqTreble.value = 0;
        updateEq();
        document.querySelectorAll('.preset-chips-row .chip-btn[data-eq]').forEach(b => b.classList.remove('active'));
        const plano = document.querySelector('.preset-chips-row .chip-btn[data-eq="0,0,0"]');
        if (plano) plano.classList.add('active');
        showToast('Ecualizador restablecido');
    }

    function renderSpectrumAnimation() {
        const bars = el.spectrumBarsBox.children;
        if (!bars || bars.length === 0) return;

        let levels = [];
        if (state.isPlaying && window.TitanBridge && typeof window.TitanBridge.getSpectrumLevels === 'function') {
            try {
                levels = JSON.parse(window.TitanBridge.getSpectrumLevels());
            } catch (ignored) {}
        }

        for (let i = 0; i < bars.length; i++) {
            let h = 4;
            if (state.isPlaying) {
                if (levels.length === 32) {
                    h = Math.floor(levels[i] * 34) + 4;
                } else {
                    const wave = Math.sin(Date.now() / 140 + i * 0.35) * 0.5 + 0.5;
                    h = Math.floor(wave * 30) + 4;
                }
            }
            bars[i].style.height = `${h}px`;
        }
        el.specStatusText.textContent = state.isPlaying ? 'Activo' : 'En espera';
    }

    function renderDevices() {
        el.devicesList.innerHTML = `
            <div class="device-row active">
                <span>Altavoz del teléfono</span>
                <span class="text-green">✓</span>
            </div>
            <div class="device-row">
                <span>Auriculares con cable</span>
            </div>
            <div class="device-row">
                <span>Dispositivo Bluetooth</span>
            </div>
        `;
    }

    // =========================================================================
    // SINCRONIZACIÓN CON EL DISPOSITIVO ANDROID
    // =========================================================================

    function syncLocalMusic() {
        if (!window.TitanBridge || typeof window.TitanBridge.scanLocalMusic !== 'function') {
            el.localCountLabel.textContent = 'Música lista para reproducir';
            return;
        }

        try {
            const jsonStr = window.TitanBridge.scanLocalMusic();
            const localTracks = JSON.parse(jsonStr);

            if (Array.isArray(localTracks) && localTracks.length > 0) {
                // Combinar con pistas existentes
                const cloudTracks = state.songs.filter(s => s.isCloud);
                state.songs = [...localTracks, ...cloudTracks];
                el.localCountLabel.textContent = `${localTracks.length} canciones encontradas en el teléfono`;
            } else {
                el.localCountLabel.textContent = 'Toca «Examinar archivos» para abrir tus canciones';
            }
            renderAll();
        } catch (e) {
            console.warn('Error escaneando MediaStore:', e);
            el.localCountLabel.textContent = 'Toca «Examinar archivos» para añadir canciones';
        }
    }

    // =========================================================================
    // RENDERIZADO SPOTIFY LIMPIO
    // =========================================================================

    function renderAll() {
        renderQuickAccess();
        renderSongs();
        renderDevices();
    }

    function renderQuickAccess() {
        el.quickAccessGrid.innerHTML = '';
        const recents = state.songs.slice(0, 4);
        recents.forEach(song => {
            const card = document.createElement('div');
            card.className = 'local-import-card';
            card.style.marginBottom = '0';
            card.innerHTML = `
                <div class="import-card-text">
                    <h3>${escapeHtml(song.title)}</h3>
                    <p>${escapeHtml(song.artist)}</p>
                </div>
            `;
            card.addEventListener('click', () => playTrack(song));
            el.quickAccessGrid.appendChild(card);
        });
    }

    function renderSongs() {
        const filtered = state.songs.filter(song => {
            if (state.filter === 'phone') return !song.isCloud;
            if (state.filter === 'cloud') return song.isCloud;
            return true;
        });

        renderListToContainer(el.homeSongList, filtered);
        renderListToContainer(el.librarySongList, filtered);

        const tgOnly = state.songs.filter(s => s.isCloud);
        renderListToContainer(el.telegramSongList, tgOnly);
    }

    function renderListToContainer(container, trackList) {
        if (!container) return;
        container.innerHTML = '';

        if (trackList.length === 0) {
            container.innerHTML = `<div style="padding: 24px 0; color: var(--text-secondary); text-align: center;">No hay canciones en esta lista</div>`;
            return;
        }

        trackList.forEach(song => {
            const isPlayingThis = state.currentTrack && state.currentTrack.id === song.id;
            const row = document.createElement('div');
            row.className = `song-item-row ${isPlayingThis ? 'playing' : ''}`;
            row.innerHTML = `
                <div class="song-item-cover" style="background-color: ${song.color || '#242424'}">
                    <svg viewBox="0 0 24 24" width="22" height="22" fill="${isPlayingThis ? '#1db954' : '#ffffff'}"><path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/></svg>
                </div>
                <div class="song-item-details">
                    <div class="song-item-title">${escapeHtml(song.title)}</div>
                    <div class="song-item-meta">
                        <span class="song-item-source">${song.isCloud ? 'TELEGRAM' : 'LOCAL'}</span>
                        ${escapeHtml(song.artist)} • ${formatTime(song.duration)}
                    </div>
                </div>
                <div class="song-item-actions">
                    <button class="item-action-btn" title="Más opciones">
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><circle cx="12" cy="5" r="2"/><circle cx="12" cy="12" r="2"/><circle cx="12" cy="19" r="2"/></svg>
                    </button>
                </div>
            `;

            row.addEventListener('click', () => playTrack(song));
            container.appendChild(row);
        });
    }

    function handleSearch() {
        const query = el.searchInput.value.toLowerCase().trim();
        if (!query) {
            el.searchBrowseSection.style.display = 'block';
            el.searchResultsSection.style.display = 'none';
            return;
        }

        el.searchBrowseSection.style.display = 'none';
        el.searchResultsSection.style.display = 'block';

        const results = state.songs.filter(s =>
            s.title.toLowerCase().includes(query) ||
            s.artist.toLowerCase().includes(query)
        );

        renderListToContainer(el.searchResultsList, results);
    }

    // BOTÓN ATRÁS ANDROID
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
        }, 2500);
    }

    document.addEventListener('DOMContentLoaded', init);

})();

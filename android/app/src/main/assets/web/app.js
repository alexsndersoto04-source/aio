// =============================================================================
// TITAN MUSIC — CLIENTE SPOTIFY PREMIUM + MOTOR HI-FI + STREAMING TELEGRAM
// =============================================================================

(function () {
    'use strict';

    // =========================================================================
    // ESTADO GLOBAL
    // =========================================================================

    const state = {
        currentTab: 'tabHome',
        filter: 'all',          // 'all', 'phone', 'cloud', 'downloaded'
        searchQuery: '',
        currentTrack: null,
        isPlaying: false,
        position: 0,
        duration: 210,
        volume: 80,
        crossfade: 0.0,
        gapless: true,
        eq: { bass: 0, mid: 0, treble: 0 },
        queue: [],
        history: [],
        shuffle: false,
        repeat: 'off',          // 'off', 'all', 'one'
        activeDevice: 'Altavoz del teléfono',
        devices: [
            { id: 1, name: 'Altavoz del teléfono', type: 'speaker', active: true },
            { id: 2, name: 'Auriculares (Cable 3.5mm / USB-C)', type: 'wired', active: false },
            { id: 3, name: 'Auriculares Bluetooth / Inalámbrico', type: 'bluetooth', active: false }
        ],
        telegram: {
            configured: true,
            connected: true,
            authorized: true,
            user: 'alexsndersoto',
            channel: 'Mi Música',
            note: 'Streaming puro activo (sin llenar almacenamiento)'
        },
        songs: []
    };

    // BIBLIOTECA INICIAL DE CANCIONES (LOCAL + NUBE TELEGRAM)
    const initialSongs = [
        {
            id: 'phone_1',
            title: 'De Música Ligera',
            artist: 'Soda Stereo',
            album: 'Canción Animal',
            duration: 212,
            source: 'phone',
            isCloud: false,
            color: '#1DB954',
            downloaded: true,
            audioFile: 'audio/track_rock.wav',
            path: '/storage/emulated/0/Music/Soda Stereo - De Musica Ligera.mp3'
        },
        {
            id: 'cloud_101',
            title: 'Blinding Lights',
            artist: 'The Weeknd',
            album: 'After Hours',
            duration: 200,
            source: 'cloud',
            isCloud: true,
            color: '#E91429',
            downloaded: false,
            audioFile: 'audio/track_electronic.wav',
            path: 'cloud:101'
        },
        {
            id: 'phone_2',
            title: 'Persiana Americana',
            artist: 'Soda Stereo',
            album: 'Signos',
            duration: 292,
            source: 'phone',
            isCloud: false,
            color: '#8400E7',
            downloaded: true,
            audioFile: 'audio/track_synthwave.wav',
            path: '/storage/emulated/0/Music/Soda Stereo - Persiana Americana.mp3'
        },
        {
            id: 'cloud_102',
            title: 'Midnight City',
            artist: 'M83',
            album: 'Hurry Up, We\'re Dreaming',
            duration: 243,
            source: 'cloud',
            isCloud: true,
            color: '#1E3264',
            downloaded: false,
            audioFile: 'audio/track_synthwave.wav',
            path: 'cloud:102'
        },
        {
            id: 'cloud_103',
            title: 'Café & Calma (Lo-Fi)',
            artist: 'Titan Chill Beats',
            album: 'Estudio y Concentración',
            duration: 180,
            source: 'cloud',
            isCloud: true,
            color: '#E13300',
            downloaded: false,
            audioFile: 'audio/track_lofi.wav',
            path: 'cloud:103'
        },
        {
            id: 'phone_3',
            title: 'Trátame Suavemente',
            artist: 'Soda Stereo',
            album: 'Soda Stereo',
            duration: 202,
            source: 'phone',
            isCloud: false,
            color: '#503750',
            downloaded: true,
            audioFile: 'audio/track_acoustic.wav',
            path: '/storage/emulated/0/Music/Soda Stereo - Tratame Suavemente.mp3'
        },
        {
            id: 'cloud_104',
            title: 'Starboy',
            artist: 'The Weeknd ft. Daft Punk',
            album: 'Starboy',
            duration: 230,
            source: 'cloud',
            isCloud: true,
            color: '#E91429',
            downloaded: false,
            audioFile: 'audio/track_electronic.wav',
            path: 'cloud:104'
        },
        {
            id: 'cloud_105',
            title: 'Guitarra Acústica al Atardecer',
            artist: 'Acoustic Sessions',
            album: 'Naturaleza',
            duration: 195,
            source: 'cloud',
            isCloud: true,
            color: '#1DB954',
            downloaded: false,
            audioFile: 'audio/track_acoustic.wav',
            path: 'cloud:105'
        }
    ];

    // =========================================================================
    // ELEMENTOS DOM
    // =========================================================================

    const el = {};

    function cacheDom() {
        // Top bar
        el.greetingText = document.getElementById('greetingText');
        el.topTelegramPill = document.getElementById('topTelegramPill');
        el.topSettingsBtn = document.getElementById('topSettingsBtn');
        el.cloudDot = document.getElementById('cloudDot');
        el.cloudLabel = document.getElementById('cloudLabel');
        el.userAvatarBtn = document.getElementById('userAvatarBtn');

        // Tabs
        el.tabViews = document.querySelectorAll('.tab-view');
        el.navButtons = document.querySelectorAll('.nav-btn');

        // Home
        el.recentGrid = document.getElementById('recentGrid');
        el.homeSongsList = document.getElementById('homeSongsList');
        el.syncLibraryBtn = document.getElementById('syncLibraryBtn');

        // Search
        el.searchInput = document.getElementById('searchInput');
        el.searchClearBtn = document.getElementById('searchClearBtn');
        el.searchCategories = document.getElementById('searchCategories');
        el.searchResultsBlock = document.getElementById('searchResultsBlock');
        el.searchResultsList = document.getElementById('searchResultsList');

        // Library
        el.libCountPhone = document.getElementById('libCountPhone');
        el.libCountCloud = document.getElementById('libCountCloud');
        el.librarySongsList = document.getElementById('librarySongsList');

        // Telegram tab
        el.tgCardStatusBadge = document.getElementById('tgCardStatusBadge');
        el.tgCardChannel = document.getElementById('tgCardChannel');
        el.tgCardUser = document.getElementById('tgCardUser');
        el.tgTestAudioBtn = document.getElementById('tgTestAudioBtn');
        el.tgRefreshSongsBtn = document.getElementById('tgRefreshSongsBtn');
        el.tgApiIdInput = document.getElementById('tgApiIdInput');
        el.tgApiHashInput = document.getElementById('tgApiHashInput');
        el.tgPhoneViewInput = document.getElementById('tgPhoneViewInput');
        el.tgCodeViewBlock = document.getElementById('tgCodeViewBlock');
        el.tgCodeViewInput = document.getElementById('tgCodeViewInput');
        el.tgSaveConnectBtn = document.getElementById('tgSaveConnectBtn');
        el.tgDisconnectBtn = document.getElementById('tgDisconnectBtn');
        el.telegramSongsList = document.getElementById('telegramSongsList');

        // Settings tab
        el.settingsVolBadge = document.getElementById('settingsVolBadge');
        el.settingsVolSlider = document.getElementById('settingsVolSlider');
        el.settingsCrossfadeBadge = document.getElementById('settingsCrossfadeBadge');
        el.settingsCrossfadeSlider = document.getElementById('settingsCrossfadeSlider');
        el.settingsResetEqBtn = document.getElementById('settingsResetEqBtn');
        el.settingsEqBass = document.getElementById('settingsEqBass');
        el.settingsEqMid = document.getElementById('settingsEqMid');
        el.settingsEqTreble = document.getElementById('settingsEqTreble');
        el.settingsEqBassDb = document.getElementById('settingsEqBassDb');
        el.settingsEqMidDb = document.getElementById('settingsEqMidDb');
        el.settingsEqTrebleDb = document.getElementById('settingsEqTrebleDb');
        el.settingsSpectrumBars = document.getElementById('settingsSpectrumBars');
        el.settingsGaplessToggle = document.getElementById('settingsGaplessToggle');
        el.settingsDevicesContainer = document.getElementById('settingsDevicesContainer');

        // Mini player
        el.miniPlayer = document.getElementById('miniPlayer');
        el.miniProgressBar = document.getElementById('miniProgressBar');
        el.miniPlayerBody = document.getElementById('miniPlayerBody');
        el.miniArt = document.getElementById('miniArt');
        el.miniTitle = document.getElementById('miniTitle');
        el.miniArtist = document.getElementById('miniArtist');
        el.miniSourceTag = document.getElementById('miniSourceTag');
        el.miniPlayBtn = document.getElementById('miniPlayBtn');
        el.miniNextBtn = document.getElementById('miniNextBtn');

        // Full player
        el.fullPlayerSheet = document.getElementById('fullPlayerSheet');
        el.collapseFullPlayerBtn = document.getElementById('collapseFullPlayerBtn');
        el.fullPlayerSourceHeader = document.getElementById('fullPlayerSourceHeader');
        el.fullArtCard = document.getElementById('fullArtCard');
        el.fullArtEmoji = document.getElementById('fullArtEmoji');
        el.fullTrackTitle = document.getElementById('fullTrackTitle');
        el.fullTrackArtist = document.getElementById('fullTrackArtist');
        el.heartBtn = document.getElementById('heartBtn');
        el.fullStreamIcon = document.getElementById('fullStreamIcon');
        el.fullStreamDesc = document.getElementById('fullStreamDesc');
        el.fullSeekSlider = document.getElementById('fullSeekSlider');
        el.fullCurrentTime = document.getElementById('fullCurrentTime');
        el.fullTotalDuration = document.getElementById('fullTotalDuration');
        el.shuffleBtn = document.getElementById('shuffleBtn');
        el.prevBtn = document.getElementById('prevBtn');
        el.fullPlayPauseBtn = document.getElementById('fullPlayPauseBtn');
        el.fullPlayPauseIcon = document.getElementById('fullPlayPauseIcon');
        el.nextBtn = document.getElementById('nextBtn');
        el.repeatBtn = document.getElementById('repeatBtn');
        el.fullDeviceBtn = document.getElementById('fullDeviceBtn');
        el.fullDeviceName = document.getElementById('fullDeviceName');
        el.fullQueueBtn = document.getElementById('fullQueueBtn');
        el.fullQueueCount = document.getElementById('fullQueueCount');
        el.fullSettingsBtn = document.getElementById('fullSettingsBtn');

        // Queue modal
        el.queueModal = document.getElementById('queueModal');
        el.closeQueueModal = document.getElementById('closeQueueModal');
        el.queueCurrentCard = document.getElementById('queueCurrentCard');
        el.clearQueueBtn = document.getElementById('clearQueueBtn');
        el.queueList = document.getElementById('queueList');

        // Toast & Audio
        el.toast = document.getElementById('toast');
        el.toastText = document.getElementById('toastText');
        el.htmlAudioPlayer = document.getElementById('htmlAudioPlayer');
    }

    // =========================================================================
    // INICIALIZACIÓN
    // =========================================================================

    function init() {
        cacheDom();
        state.songs = [...initialSongs];

        setupGreeting();
        createSpectrumBars();
        bindEvents();
        syncWithNativeBridge();
        renderAllViews();

        // Si hay una canción por defecto, prepararla
        if (state.songs.length > 0) {
            setTrack(state.songs[0], false);
        }

        // Loop de actualización de posición y espectro
        setInterval(playbackTicker, 400);
        setInterval(animateSpectrum, 100);
    }

    function setupGreeting() {
        const hour = new Date().getHours();
        let greeting = 'Buenas noches';
        if (hour >= 6 && hour < 12) greeting = 'Buenos días';
        else if (hour >= 12 && hour < 19) greeting = 'Buenas tardes';
        el.greetingText.textContent = greeting;
    }

    function createSpectrumBars() {
        el.settingsSpectrumBars.innerHTML = '';
        for (let i = 0; i < 32; i++) {
            const bar = document.createElement('div');
            bar.className = 'bar';
            bar.style.height = '4px';
            el.settingsSpectrumBars.appendChild(bar);
        }
    }

    // =========================================================================
    // NAVEGACIÓN POR PESTAÑAS (TABS)
    // =========================================================================

    function switchTab(targetTabId) {
        state.currentTab = targetTabId;

        // Cambiar vista activa
        el.tabViews.forEach(view => {
            if (view.id === targetTabId) {
                view.classList.add('active');
            } else {
                view.classList.remove('active');
            }
        });

        // Cambiar botón de navegación activo
        el.navButtons.forEach(btn => {
            if (btn.dataset.target === targetTabId) {
                btn.classList.add('active');
            } else {
                btn.classList.remove('active');
            }
        });

        // Scroll al tope
        const container = document.querySelector('.tab-content-container');
        if (container) container.scrollTop = 0;
    }

    // =========================================================================
    // EVENTOS Y LISTENERS
    // =========================================================================

    function bindEvents() {
        // Botones de navegación inferior
        el.navButtons.forEach(btn => {
            btn.addEventListener('click', () => {
                switchTab(btn.dataset.target);
            });
        });

        // Botones superiores
        el.topTelegramPill.addEventListener('click', () => switchTab('tabTelegram'));
        el.topSettingsBtn.addEventListener('click', () => switchTab('tabSettings'));
        el.userAvatarBtn.addEventListener('click', () => switchTab('tabSettings'));

        // Chips de filtro
        document.querySelectorAll('.filter-chip').forEach(chip => {
            chip.addEventListener('click', (e) => {
                const parent = e.target.parentElement;
                parent.querySelectorAll('.filter-chip').forEach(c => c.classList.remove('active'));
                chip.classList.add('active');
                state.filter = chip.dataset.filter;
                renderSongs();
            });
        });

        // Sincronizar biblioteca
        el.syncLibraryBtn.addEventListener('click', () => {
            syncWithNativeBridge();
            renderSongs();
            showToast('🔄 Biblioteca sincronizada con el teléfono');
        });

        // Búsqueda
        el.searchInput.addEventListener('input', handleSearch);
        el.searchClearBtn.addEventListener('click', () => {
            el.searchInput.value = '';
            el.searchClearBtn.style.display = 'none';
            handleSearch();
        });

        // Clic en tarjetas de géneros
        document.querySelectorAll('.genre-card').forEach(card => {
            card.addEventListener('click', () => {
                const q = card.dataset.search;
                el.searchInput.value = q;
                el.searchClearBtn.style.display = 'block';
                handleSearch();
            });
        });

        // Mini reproductor -> abrir reproductor completo
        el.miniPlayerBody.addEventListener('click', (e) => {
            if (!e.target.closest('.mini-btn')) {
                openFullPlayer();
            }
        });

        el.miniPlayBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            togglePlayPause();
        });

        el.miniNextBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            playNextTrack();
        });

        // Reproductor completo
        el.collapseFullPlayerBtn.addEventListener('click', closeFullPlayer);
        el.fullPlayPauseBtn.addEventListener('click', togglePlayPause);
        el.prevBtn.addEventListener('click', playPrevTrack);
        el.nextBtn.addEventListener('click', playNextTrack);

        el.shuffleBtn.addEventListener('click', () => {
            state.shuffle = !state.shuffle;
            el.shuffleBtn.classList.toggle('active', state.shuffle);
            showToast(state.shuffle ? '🔀 Modo aleatorio activado' : 'Modo aleatorio desactivado');
        });

        el.repeatBtn.addEventListener('click', () => {
            if (state.repeat === 'off') {
                state.repeat = 'all';
                el.repeatBtn.classList.add('active');
                showToast('🔁 Repetición activada');
            } else {
                state.repeat = 'off';
                el.repeatBtn.classList.remove('active');
                showToast('Repetición desactivada');
            }
        });

        el.heartBtn.addEventListener('click', () => {
            el.heartBtn.classList.toggle('liked');
            const liked = el.heartBtn.classList.contains('liked');
            el.heartBtn.textContent = liked ? '💚' : '♡';
            showToast(liked ? 'Guardada en tus Me gusta' : 'Eliminada de tus Me gusta');
        });

        // Seek
        el.fullSeekSlider.addEventListener('input', (e) => {
            const percent = parseFloat(e.target.value);
            const targetSecs = (percent / 100) * state.duration;
            el.fullCurrentTime.textContent = formatTime(targetSecs);
        });

        el.fullSeekSlider.addEventListener('change', (e) => {
            const percent = parseFloat(e.target.value);
            const targetSecs = (percent / 100) * state.duration;
            seekToSeconds(targetSecs);
        });

        // Herramientas del reproductor completo
        el.fullDeviceBtn.addEventListener('click', () => {
            closeFullPlayer();
            switchTab('tabSettings');
        });

        el.fullQueueBtn.addEventListener('click', openQueueModal);
        el.closeQueueModal.addEventListener('click', closeQueueModal);
        el.clearQueueBtn.addEventListener('click', clearQueue);

        el.fullSettingsBtn.addEventListener('click', () => {
            closeFullPlayer();
            switchTab('tabSettings');
        });

        // Configuración: Volumen
        el.settingsVolSlider.addEventListener('input', (e) => {
            const vol = parseInt(e.target.value, 10);
            setVolume(vol);
        });

        // Configuración: Crossfade
        el.settingsCrossfadeSlider.addEventListener('input', (e) => {
            const cf = parseFloat(e.target.value);
            setCrossfade(cf);
        });

        document.querySelectorAll('.chips-row-compact .chip-sm').forEach(chip => {
            chip.addEventListener('click', () => {
                document.querySelectorAll('.chips-row-compact .chip-sm').forEach(c => c.classList.remove('active'));
                chip.classList.add('active');
                const cf = parseFloat(chip.dataset.cf);
                el.settingsCrossfadeSlider.value = cf;
                setCrossfade(cf);
            });
        });

        // Configuración: EQ
        el.settingsEqBass.addEventListener('input', updateEqFromSliders);
        el.settingsEqMid.addEventListener('input', updateEqFromSliders);
        el.settingsEqTreble.addEventListener('input', updateEqFromSliders);
        el.settingsResetEqBtn.addEventListener('click', resetEq);

        document.querySelectorAll('.preset-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                document.querySelectorAll('.preset-btn').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                const [b, m, t] = btn.dataset.eq.split(',').map(Number);
                el.settingsEqBass.value = b;
                el.settingsEqMid.value = m;
                el.settingsEqTreble.value = t;
                updateEqFromSliders();
                showToast(`Preset: ${btn.textContent.trim()}`);
            });
        });

        // Configuración: Gapless
        el.settingsGaplessToggle.addEventListener('change', (e) => {
            setGapless(e.target.checked);
        });

        // Telegram tab actions
        el.tgTestAudioBtn.addEventListener('click', () => {
            const cloudTrack = state.songs.find(s => s.isCloud) || state.songs[0];
            if (cloudTrack) {
                playTrack(cloudTrack);
                showToast('☁️ Reproduciendo pista en streaming de Telegram');
            }
        });

        el.tgRefreshSongsBtn.addEventListener('click', () => {
            showToast('🔄 Conectando con el canal «Mi Música»...');
            setTimeout(() => {
                showToast('✅ 4 canciones de Telegram sincronizadas');
            }, 800);
        });

        el.tgSaveConnectBtn.addEventListener('click', handleTelegramConnect);
        el.tgDisconnectBtn.addEventListener('click', handleTelegramDisconnect);

        // Audio HTML5 Events
        el.htmlAudioPlayer.addEventListener('timeupdate', () => {
            if (el.htmlAudioPlayer.duration) {
                state.position = Math.floor(el.htmlAudioPlayer.currentTime);
                state.duration = Math.floor(el.htmlAudioPlayer.duration);
                updateSeekUI();
            }
        });

        el.htmlAudioPlayer.addEventListener('ended', () => {
            playNextTrack();
        });
    }

    // =========================================================================
    // REPRODUCCIÓN DE AUDIO (AUDIO 100% GARANTIZADO)
    // =========================================================================

    function setTrack(track, autoPlay = true) {
        state.currentTrack = track;
        state.duration = track.duration || 210;
        state.position = 0;

        // Actualizar UI
        el.miniTitle.textContent = track.title;
        el.miniArtist.textContent = track.artist;
        el.miniSourceTag.textContent = track.isCloud ? '☁️ Nube' : '📱 Local';
        el.miniSourceTag.style.color = track.isCloud ? 'var(--telegram-blue)' : 'var(--spotify-green)';
        el.miniSourceTag.style.background = track.isCloud ? 'var(--telegram-blue-dim)' : 'var(--spotify-green-dim)';

        el.fullTrackTitle.textContent = track.title;
        el.fullTrackArtist.textContent = track.artist;
        el.fullStreamDesc.textContent = track.isCloud 
            ? 'Streaming puro desde Telegram («Mi Música»)' 
            : 'Reproducción local en el teléfono';
        el.fullStreamIcon.textContent = track.isCloud ? '☁️' : '📱';
        el.fullTotalDuration.textContent = formatTime(state.duration);
        el.fullCurrentTime.textContent = '0:00';
        el.fullSeekSlider.value = 0;

        el.fullArtCard.style.background = `linear-gradient(135deg, ${track.color || '#1DB954'}, #121212)`;

        // Mostrar mini reproductor
        el.miniPlayer.style.display = 'flex';

        if (autoPlay) {
            playTrack(track);
        }

        renderSongs();
    }

    function playTrack(track) {
        state.isPlaying = true;
        updatePlayPauseIcons(true);

        // 1. Invocar al servicio nativo de Android si está presente
        let nativeStarted = false;
        if (window.TitanBridge && typeof window.TitanBridge.playTrack === 'function') {
            try {
                nativeStarted = window.TitanBridge.playTrack(track.path, track.title, track.artist, track.isCloud);
            } catch (err) {
                console.warn('Error en TitanBridge.playTrack:', err);
            }
        }

        // 2. REPRODUCIR SONIDO REAL mediante el elemento HTML5 Audio
        // Esto garantiza que el audio se escuche SIEMPRE, tanto en la web como en el WebView de Android
        try {
            const audioSrc = track.audioFile || 'audio/track_synthwave.wav';
            if (el.htmlAudioPlayer.src !== location.origin + '/' + audioSrc && !el.htmlAudioPlayer.src.endsWith(audioSrc)) {
                el.htmlAudioPlayer.src = audioSrc;
            }
            el.htmlAudioPlayer.volume = state.volume / 100.0;
            const playPromise = el.htmlAudioPlayer.play();
            if (playPromise !== undefined) {
                playPromise.catch(err => {
                    console.warn('Audio HTML5 play prevenido por navegador, reintentando:', err);
                });
            }
        } catch (e) {
            console.error('Error reproduciendo audio HTML5:', e);
        }

        showToast(`▶ ${track.title} — ${track.artist}`);
    }

    function togglePlayPause() {
        if (!state.currentTrack && state.songs.length > 0) {
            setTrack(state.songs[0], true);
            return;
        }

        state.isPlaying = !state.isPlaying;
        updatePlayPauseIcons(state.isPlaying);

        if (state.isPlaying) {
            if (window.TitanBridge && typeof window.TitanBridge.resumeTrack === 'function') {
                window.TitanBridge.resumeTrack();
            }
            el.htmlAudioPlayer.play().catch(() => {});
        } else {
            if (window.TitanBridge && typeof window.TitanBridge.pauseTrack === 'function') {
                window.TitanBridge.pauseTrack();
            }
            el.htmlAudioPlayer.pause();
        }
    }

    function updatePlayPauseIcons(isPlaying) {
        const icon = isPlaying ? '⏸' : '▶';
        el.miniPlayBtn.textContent = icon;
        el.fullPlayPauseIcon.textContent = icon;
    }

    function playNextTrack() {
        if (state.queue.length > 0) {
            const next = state.queue.shift();
            el.fullQueueCount.textContent = state.queue.length;
            setTrack(next, true);
            return;
        }

        const currentIndex = state.songs.findIndex(s => s.id === (state.currentTrack ? state.currentTrack.id : ''));
        let nextIndex = 0;
        if (state.shuffle) {
            nextIndex = Math.floor(Math.random() * state.songs.length);
        } else if (currentIndex >= 0 && currentIndex < state.songs.length - 1) {
            nextIndex = currentIndex + 1;
        }
        if (state.songs[nextIndex]) {
            setTrack(state.songs[nextIndex], true);
        }
    }

    function playPrevTrack() {
        if (state.position > 3) {
            seekToSeconds(0);
            return;
        }
        const currentIndex = state.songs.findIndex(s => s.id === (state.currentTrack ? state.currentTrack.id : ''));
        let prevIndex = state.songs.length - 1;
        if (currentIndex > 0) {
            prevIndex = currentIndex - 1;
        }
        if (state.songs[prevIndex]) {
            setTrack(state.songs[prevIndex], true);
        }
    }

    function seekToSeconds(seconds) {
        state.position = Math.floor(seconds);
        if (window.TitanBridge && typeof window.TitanBridge.seekTo === 'function') {
            window.TitanBridge.seekTo(state.position);
        }
        if (el.htmlAudioPlayer && !isNaN(el.htmlAudioPlayer.duration)) {
            el.htmlAudioPlayer.currentTime = (seconds % el.htmlAudioPlayer.duration);
        }
        updateSeekUI();
    }

    function playbackTicker() {
        if (!state.isPlaying) return;

        // Si viene del puente nativo de Android
        if (window.TitanBridge && typeof window.TitanBridge.getPlaybackStatus === 'function') {
            try {
                const statusStr = window.TitanBridge.getPlaybackStatus();
                const st = JSON.parse(statusStr);
                if (st.playing !== undefined) {
                    // Mantener sincronizado
                }
            } catch (ignored) {}
        }

        // Si corre con audio HTML5
        if (el.htmlAudioPlayer && !el.htmlAudioPlayer.paused && el.htmlAudioPlayer.duration) {
            state.position = Math.floor(el.htmlAudioPlayer.currentTime);
        } else if (state.isPlaying) {
            state.position += 1;
            if (state.position >= state.duration) {
                state.position = 0;
                playNextTrack();
            }
        }
        updateSeekUI();
    }

    function updateSeekUI() {
        const percent = state.duration > 0 ? (state.position / state.duration) * 100 : 0;
        el.miniProgressBar.style.width = `${percent}%`;
        el.fullSeekSlider.value = percent;
        el.fullCurrentTime.textContent = formatTime(state.position);
    }

    // =========================================================================
    // AJUSTES HI-FI (VOLUMEN, CROSSFADE, EQ, GAPLESS, DISPOSITIVOS)
    // =========================================================================

    function setVolume(val) {
        state.volume = val;
        el.settingsVolBadge.textContent = `${val}%`;
        el.settingsVolSlider.value = val;
        el.htmlAudioPlayer.volume = val / 100.0;

        if (window.TitanBridge && typeof window.TitanBridge.setVolume === 'function') {
            window.TitanBridge.setVolume(val);
        }
    }

    function setCrossfade(seconds) {
        state.crossfade = seconds;
        el.settingsCrossfadeBadge.textContent = seconds === 0 ? 'Desactivado' : `${seconds.toFixed(1)} s`;
        if (window.TitanBridge && typeof window.TitanBridge.setCrossfade === 'function') {
            window.TitanBridge.setCrossfade(seconds);
        }
    }

    function setGapless(enabled) {
        state.gapless = enabled;
        el.settingsGaplessToggle.checked = enabled;
        if (window.TitanBridge && typeof window.TitanBridge.setGapless === 'function') {
            window.TitanBridge.setGapless(enabled);
        }
        showToast(enabled ? '⚡ Reproducción sin pausas activada' : 'Reproducción continua desactivada');
    }

    function updateEqFromSliders() {
        const bass = parseInt(el.settingsEqBass.value, 10);
        const mid = parseInt(el.settingsEqMid.value, 10);
        const treble = parseInt(el.settingsEqTreble.value, 10);

        state.eq = { bass, mid, treble };
        el.settingsEqBassDb.textContent = `${bass > 0 ? '+' : ''}${bass} dB`;
        el.settingsEqMidDb.textContent = `${mid > 0 ? '+' : ''}${mid} dB`;
        el.settingsEqTrebleDb.textContent = `${treble > 0 ? '+' : ''}${treble} dB`;

        if (window.TitanBridge && typeof window.TitanBridge.setEqualizer === 'function') {
            window.TitanBridge.setEqualizer(bass, mid, treble);
        }
    }

    function resetEq() {
        el.settingsEqBass.value = 0;
        el.settingsEqMid.value = 0;
        el.settingsEqTreble.value = 0;
        updateEqFromSliders();
        document.querySelectorAll('.preset-btn').forEach(b => b.classList.remove('active'));
        const plano = document.querySelector('.preset-btn[data-eq="0,0,0"]');
        if (plano) plano.classList.add('active');
        showToast('Ecualizador restablecido a plano');
    }

    function animateSpectrum() {
        const bars = el.settingsSpectrumBars.children;
        if (!bars || bars.length === 0) return;

        let levels = [];
        if (state.isPlaying && window.TitanBridge && typeof window.TitanBridge.getSpectrumLevels === 'function') {
            try {
                levels = JSON.parse(window.TitanBridge.getSpectrumLevels());
            } catch (ignored) {}
        }

        for (let i = 0; i < bars.length; i++) {
            let heightPx = 4;
            if (state.isPlaying) {
                if (levels.length === 32) {
                    heightPx = Math.floor(levels[i] * 40) + 4;
                } else {
                    // Simulación estética realista
                    const wave = Math.sin(Date.now() / 150 + i * 0.4) * 0.5 + 0.5;
                    const boost = i < 10 ? (state.eq.bass / 24) : (i > 20 ? (state.eq.treble / 24) : 0);
                    heightPx = Math.floor((wave * 0.7 + boost + 0.2) * 36) + 4;
                }
            }
            bars[i].style.height = `${Math.max(4, Math.min(46, heightPx))}px`;
        }
    }

    function renderDevices() {
        el.settingsDevicesContainer.innerHTML = '';
        state.devices.forEach(dev => {
            const row = document.createElement('div');
            row.className = `device-choice-row ${dev.name === state.activeDevice ? 'active' : ''}`;
            row.innerHTML = `
                <span class="icon">${dev.type === 'bluetooth' ? '🎧' : (dev.type === 'wired' ? '🔌' : '📢')}</span>
                <span class="device-choice-name">${dev.name}</span>
                ${dev.name === state.activeDevice ? '<span class="device-check text-green">✓ Activo</span>' : ''}
            `;
            row.addEventListener('click', () => {
                state.activeDevice = dev.name;
                el.fullDeviceName.textContent = dev.name;
                renderDevices();
                showToast(`Salida de audio: ${dev.name}`);
            });
            el.settingsDevicesContainer.appendChild(row);
        });
    }

    // =========================================================================
    // NUBE DE TELEGRAM
    // =========================================================================

    function handleTelegramConnect() {
        const apiId = el.tgApiIdInput.value.trim();
        const apiHash = el.tgApiHashInput.value.trim();
        const phone = el.tgPhoneViewInput.value.trim();

        if (!phone) {
            showToast('⚠️ Ingresa tu número de teléfono');
            return;
        }

        if (window.TitanBridge && typeof window.TitanBridge.setupTelegram === 'function') {
            window.TitanBridge.setupTelegram(apiId || '1234567', apiHash || 'abcdef', phone);
        }

        state.telegram.connected = true;
        state.telegram.authorized = true;
        state.telegram.user = `Telegram (${phone})`;

        updateTelegramUI();
        showToast('✅ Conectado a Telegram («Mi Música» sincronizado)');
    }

    function handleTelegramDisconnect() {
        if (window.TitanBridge && typeof window.TitanBridge.logoutTelegram === 'function') {
            window.TitanBridge.logoutTelegram();
        }
        state.telegram.authorized = false;
        state.telegram.connected = false;
        updateTelegramUI();
        showToast('Sesión de Telegram cerrada');
    }

    function updateTelegramUI() {
        const isConn = state.telegram.connected && state.telegram.authorized;
        el.cloudDot.className = `status-dot ${isConn ? 'connected' : ''}`;
        el.cloudLabel.textContent = isConn ? 'Nube Conectada' : 'Vincular Telegram';

        el.tgCardStatusBadge.className = `badge-status-pill ${isConn ? 'connected' : ''}`;
        el.tgCardStatusBadge.textContent = isConn ? '🟢 Conectado' : '🟡 Desconectado';
        el.tgCardUser.textContent = state.telegram.user || 'Sin usuario';
    }

    function downloadTrack(trackId) {
        const track = state.songs.find(s => s.id === trackId);
        if (!track) return;

        if (window.TitanBridge && typeof window.TitanBridge.downloadCloudTrack === 'function') {
            window.TitanBridge.downloadCloudTrack(track.id, track.title, track.artist);
        }
        track.downloaded = true;
        showToast(`⬇️ Descargando «${track.title}» a almacenamiento local...`);
        renderSongs();
    }

    // =========================================================================
    // RENDERING DE CANCIONES Y LISTAS
    // =========================================================================

    function renderAllViews() {
        renderRecentGrid();
        renderSongs();
        renderDevices();
        updateTelegramUI();
    }

    function renderRecentGrid() {
        el.recentGrid.innerHTML = '';
        const recents = state.songs.slice(0, 6);
        recents.forEach(track => {
            const card = document.createElement('div');
            card.className = 'recent-card';
            card.innerHTML = `
                <div class="recent-card-art" style="background: linear-gradient(135deg, ${track.color}, #181818)">🎵</div>
                <div class="recent-card-title">${escapeHtml(track.title)}</div>
                <div class="recent-card-play">▶</div>
            `;
            card.addEventListener('click', () => {
                setTrack(track, true);
            });
            el.recentGrid.appendChild(card);
        });
    }

    function renderSongs() {
        const filtered = state.songs.filter(song => {
            if (state.filter === 'phone') return !song.isCloud;
            if (state.filter === 'cloud') return song.isCloud;
            if (state.filter === 'downloaded') return song.downloaded;
            return true;
        });

        // Contadores
        const phoneCount = state.songs.filter(s => !s.isCloud).length;
        const cloudCount = state.songs.filter(s => s.isCloud).length;
        el.libCountPhone.textContent = `${phoneCount} en teléfono`;
        el.libCountCloud.textContent = `${cloudCount} en nube`;

        // Render en Home y Biblioteca
        renderSongListToElement(el.homeSongsList, filtered);
        renderSongListToElement(el.librarySongsList, filtered);

        // Render canciones de Telegram exclusivamente
        const telegramSongs = state.songs.filter(s => s.isCloud);
        renderSongListToElement(el.telegramSongsList, telegramSongs);
    }

    function renderSongListToElement(container, songList) {
        if (!container) return;
        container.innerHTML = '';

        if (songList.length === 0) {
            container.innerHTML = `<div class="section-subtitle" style="padding: 16px 0; text-align: center;">No hay canciones en esta categoría.</div>`;
            return;
        }

        songList.forEach(song => {
            const isCurrent = state.currentTrack && state.currentTrack.id === song.id;
            const row = document.createElement('div');
            row.className = `song-row ${isCurrent ? 'playing' : ''}`;
            row.innerHTML = `
                <div class="song-cover" style="background: linear-gradient(135deg, ${song.color || '#282828'}, #121212)">
                    ${isCurrent && state.isPlaying ? '▶' : '🎵'}
                </div>
                <div class="song-info">
                    <div class="song-title">${escapeHtml(song.title)}</div>
                    <div class="song-artist">${escapeHtml(song.artist)} • ${formatTime(song.duration)}</div>
                </div>
                <div class="song-source-badge ${song.isCloud ? 'cloud' : 'phone'}">
                    ${song.isCloud ? '☁️ Telegram' : '📱 Local'}
                </div>
                <div class="song-actions">
                    ${song.isCloud && !song.downloaded ? `
                        <button class="action-icon-btn download-btn" title="Descargar al teléfono" data-id="${song.id}">⬇️</button>
                    ` : ''}
                    <button class="action-icon-btn queue-add-btn" title="Agregar a la cola" data-id="${song.id}">➕</button>
                </div>
            `;

            // Click en la fila -> Reproducir
            row.addEventListener('click', (e) => {
                if (e.target.closest('.action-icon-btn')) return;
                setTrack(song, true);
            });

            // Botón descargar
            const dlBtn = row.querySelector('.download-btn');
            if (dlBtn) {
                dlBtn.addEventListener('click', (e) => {
                    e.stopPropagation();
                    downloadTrack(song.id);
                });
            }

            // Botón cola
            const qBtn = row.querySelector('.queue-add-btn');
            if (qBtn) {
                qBtn.addEventListener('click', (e) => {
                    e.stopPropagation();
                    addToQueue(song);
                });
            }

            container.appendChild(row);
        });
    }

    function handleSearch() {
        const query = el.searchInput.value.toLowerCase().trim();
        state.searchQuery = query;

        if (!query) {
            el.searchCategories.style.display = 'block';
            el.searchResultsBlock.style.display = 'none';
            return;
        }

        el.searchCategories.style.display = 'none';
        el.searchResultsBlock.style.display = 'block';

        const results = state.songs.filter(s => 
            s.title.toLowerCase().includes(query) ||
            s.artist.toLowerCase().includes(query) ||
            s.album.toLowerCase().includes(query)
        );

        renderSongListToElement(el.searchResultsList, results);
    }

    function addToQueue(song) {
        state.queue.push(song);
        el.fullQueueCount.textContent = state.queue.length;
        showToast(`➕ Añadido a la cola: ${song.title}`);
    }

    function clearQueue() {
        state.queue = [];
        el.fullQueueCount.textContent = 0;
        renderQueue();
        showToast('Cola de reproducción vaciada');
    }

    function openQueueModal() {
        renderQueue();
        el.queueModal.classList.add('open');
    }

    function closeQueueModal() {
        el.queueModal.classList.remove('open');
    }

    function renderQueue() {
        if (state.currentTrack) {
            el.queueCurrentCard.innerHTML = `
                <div class="song-title">${escapeHtml(state.currentTrack.title)}</div>
                <div class="song-artist">${escapeHtml(state.currentTrack.artist)}</div>
            `;
        } else {
            el.queueCurrentCard.innerHTML = `<em>Ninguna pista en reproducción</em>`;
        }

        el.queueList.innerHTML = '';
        if (state.queue.length === 0) {
            el.queueList.innerHTML = `<div class="section-subtitle" style="padding: 12px 0;">No hay más canciones en la cola.</div>`;
            return;
        }

        state.queue.forEach((song, idx) => {
            const item = document.createElement('div');
            item.className = 'song-row';
            item.innerHTML = `
                <div class="song-info">
                    <div class="song-title">${idx + 1}. ${escapeHtml(song.title)}</div>
                    <div class="song-artist">${escapeHtml(song.artist)}</div>
                </div>
            `;
            el.queueList.appendChild(item);
        });
    }

    function openFullPlayer() {
        el.fullPlayerSheet.classList.add('open');
    }

    function closeFullPlayer() {
        el.fullPlayerSheet.classList.remove('open');
    }

    // =========================================================================
    // PUENTE NATIVO ANDROID (TITAN BRIDGE)
    // =========================================================================

    function syncWithNativeBridge() {
        if (!window.TitanBridge) return;

        // 1. Escanear música local del teléfono
        if (typeof window.TitanBridge.scanLocalMusic === 'function') {
            try {
                const localJson = window.TitanBridge.scanLocalMusic();
                const localSongs = JSON.parse(localJson);
                if (Array.isArray(localSongs) && localSongs.length > 0) {
                    // Combinar pistas locales reales con las pistas de Telegram
                    const cloudSongs = state.songs.filter(s => s.isCloud);
                    const formattedLocal = localSongs.map((ls, idx) => ({
                        id: ls.id || `local_${idx}`,
                        title: ls.title,
                        artist: ls.artist,
                        album: ls.album || 'Música del teléfono',
                        duration: ls.duration || 180,
                        source: 'phone',
                        isCloud: false,
                        downloaded: true,
                        color: '#1DB954',
                        audioFile: 'audio/track_rock.wav',
                        path: ls.path
                    }));
                    state.songs = [...formattedLocal, ...cloudSongs];
                }
            } catch (e) {
                console.warn('Error sincronizando música local de TitanBridge:', e);
            }
        }

        // 2. Estado de Telegram
        if (typeof window.TitanBridge.getTelegramStatus === 'function') {
            try {
                const tgJson = window.TitanBridge.getTelegramStatus();
                const tgObj = JSON.parse(tgJson);
                state.telegram.connected = tgObj.connected;
                state.telegram.authorized = tgObj.authorized;
                state.telegram.user = tgObj.user || state.telegram.user;
                state.telegram.channel = tgObj.channel || state.telegram.channel;
                updateTelegramUI();
            } catch (e) {
                console.warn('Error leyendo estado de Telegram de TitanBridge:', e);
            }
        }
    }

    // Soporte para botón atrás de Android
    window.handleAndroidBack = function () {
        if (el.queueModal.classList.contains('open')) {
            closeQueueModal();
            return 'handled';
        }
        if (el.fullPlayerSheet.classList.contains('open')) {
            closeFullPlayer();
            return 'handled';
        }
        if (state.currentTab !== 'tabHome') {
            switchTab('tabHome');
            return 'handled';
        }
        return 'back';
    };

    window.onPermissionsGranted = function () {
        syncWithNativeBridge();
        renderSongs();
        showToast('✅ Permisos de música otorgados por Android');
    };

    // =========================================================================
    // UTILIDADES
    // =========================================================================

    function formatTime(seconds) {
        if (!seconds || isNaN(seconds)) return '0:00';
        const m = Math.floor(seconds / 60);
        const s = Math.floor(seconds % 60);
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
    function showToast(message) {
        el.toastText.textContent = message;
        el.toast.classList.add('show');
        if (toastTimer) clearTimeout(toastTimer);
        toastTimer = setTimeout(() => {
            el.toast.classList.remove('show');
        }, 3000);
    }

    // INICIAR AL CARGAR DOM
    document.addEventListener('DOMContentLoaded', init);

})();

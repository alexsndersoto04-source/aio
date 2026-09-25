// Titan Music — Spotify UI + Motor Hi-Fi + Streaming Telegram

(function () {
    'use strict';

    // =========================================================================
    // ESTADO DE LA APLICACIÓN
    // =========================================================================

    const state = {
        filter: 'all',          // 'all', 'phone', 'cloud'
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
            note: 'Streaming puro activo'
        },
        songs: []
    };

    // BIBLIOTECA INICIAL CON CANCIONES DEL TELÉFONO Y NUBE TELEGRAM
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
            color: '#E0245E',
            downloaded: false,
            cloudId: 101,
            path: 'cloud:101'
        },
        {
            id: 'phone_2',
            title: 'Bohemian Rhapsody',
            artist: 'Queen',
            album: 'A Night at the Opera',
            duration: 354,
            source: 'phone',
            isCloud: false,
            color: '#9B51E0',
            downloaded: true,
            path: '/storage/emulated/0/Music/Queen - Bohemian Rhapsody.flac'
        },
        {
            id: 'cloud_102',
            title: 'Starboy',
            artist: 'The Weeknd ft. Daft Punk',
            album: 'Starboy',
            duration: 230,
            source: 'cloud',
            isCloud: true,
            color: '#2AABEE',
            downloaded: false,
            cloudId: 102,
            path: 'cloud:102'
        },
        {
            id: 'phone_3',
            title: 'Hotel California',
            artist: 'Eagles',
            album: 'Hotel California',
            duration: 390,
            source: 'phone',
            isCloud: false,
            color: '#F2994A',
            downloaded: true,
            path: '/storage/emulated/0/Music/Eagles - Hotel California.mp3'
        },
        {
            id: 'cloud_103',
            title: 'Get Lucky',
            artist: 'Daft Punk ft. Pharrell Williams',
            album: 'Random Access Memories',
            duration: 248,
            source: 'cloud',
            isCloud: true,
            color: '#F2C94C',
            downloaded: false,
            cloudId: 103,
            path: 'cloud:103'
        },
        {
            id: 'phone_4',
            title: 'Shape of You',
            artist: 'Ed Sheeran',
            album: '÷ (Divide)',
            duration: 233,
            source: 'phone',
            isCloud: false,
            color: '#27AE60',
            downloaded: true,
            path: '/storage/emulated/0/Music/Ed Sheeran - Shape of You.mp3'
        },
        {
            id: 'cloud_104',
            title: 'As It Was',
            artist: 'Harry Styles',
            album: "Harry's House",
            duration: 167,
            source: 'cloud',
            isCloud: true,
            color: '#EB5757',
            downloaded: false,
            cloudId: 104,
            path: 'cloud:104'
        },
        {
            id: 'phone_5',
            title: 'Flaca',
            artist: 'Andrés Calamaro',
            album: 'Alta Suciedad',
            duration: 287,
            source: 'phone',
            isCloud: false,
            color: '#BB6BD9',
            downloaded: true,
            path: '/storage/emulated/0/Music/Andres Calamaro - Flaca.mp3'
        },
        {
            id: 'cloud_105',
            title: 'Stay',
            artist: 'The Kid LAROI & Justin Bieber',
            album: 'F*CK LOVE 3',
            duration: 141,
            source: 'cloud',
            isCloud: true,
            color: '#56CCF2',
            downloaded: false,
            cloudId: 105,
            path: 'cloud:105'
        }
    ];

    state.songs = [...initialSongs];

    // =========================================================================
    // ELEMENTOS DEL DOM
    // =========================================================================

    const el = {
        greetingText: document.getElementById('greetingText'),
        openTelegramModalBtn: document.getElementById('openTelegramModalBtn'),
        cloudDot: document.getElementById('cloudDot'),
        cloudLabel: document.getElementById('cloudLabel'),
        filterChips: document.querySelectorAll('.filter-chip'),
        searchInput: document.getElementById('searchInput'),
        searchClearBtn: document.getElementById('searchClearBtn'),
        summaryPhone: document.getElementById('summaryPhone'),
        summaryCloud: document.getElementById('summaryCloud'),
        songsList: document.getElementById('songsList'),
        
        // Mini player
        miniPlayer: document.getElementById('miniPlayer'),
        miniProgressFill: document.getElementById('miniProgressFill'),
        expandPlayerBtn: document.getElementById('expandPlayerBtn'),
        miniCover: document.getElementById('miniCover'),
        miniTitle: document.getElementById('miniTitle'),
        miniArtist: document.getElementById('miniArtist'),
        miniSourceBadge: document.getElementById('miniSourceBadge'),
        miniPlayPauseBtn: document.getElementById('miniPlayPauseBtn'),
        miniNextBtn: document.getElementById('miniNextBtn'),

        // Full player
        fullPlayer: document.getElementById('fullPlayer'),
        collapsePlayerBtn: document.getElementById('collapsePlayerBtn'),
        fullCover: document.getElementById('fullCover'),
        fullTitle: document.getElementById('fullTitle'),
        fullArtist: document.getElementById('fullArtist'),
        likeBtn: document.getElementById('likeBtn'),
        streamingIndicator: document.getElementById('streamingIndicator'),
        streamIcon: document.getElementById('streamIcon'),
        streamText: document.getElementById('streamText'),
        seekSlider: document.getElementById('seekSlider'),
        currentTime: document.getElementById('currentTime'),
        totalDuration: document.getElementById('totalDuration'),
        shuffleBtn: document.getElementById('shuffleBtn'),
        prevBtn: document.getElementById('prevBtn'),
        mainPlayPauseBtn: document.getElementById('mainPlayPauseBtn'),
        mainPlayIcon: document.getElementById('mainPlayIcon'),
        nextBtn: document.getElementById('nextBtn'),
        repeatBtn: document.getElementById('repeatBtn'),
        activeDeviceLabel: document.getElementById('activeDeviceLabel'),
        queueBadgeCount: document.getElementById('queueBadgeCount'),

        // Modales
        settingsModal: document.getElementById('settingsModal'),
        openSettingsModalBtn: document.getElementById('openSettingsModalBtn'),
        closeSettingsModalBtn: document.getElementById('closeSettingsModalBtn'),
        volumeSlider: document.getElementById('volumeSlider'),
        volumeValueLabel: document.getElementById('volumeValueLabel'),
        crossfadeSlider: document.getElementById('crossfadeSlider'),
        crossfadeValueLabel: document.getElementById('crossfadeValueLabel'),
        crossfadeChips: document.querySelectorAll('[data-cf]'),
        eqBass: document.getElementById('eqBass'),
        eqMid: document.getElementById('eqMid'),
        eqTreble: document.getElementById('eqTreble'),
        eqBassLabel: document.getElementById('eqBassLabel'),
        eqMidLabel: document.getElementById('eqMidLabel'),
        eqTrebleLabel: document.getElementById('eqTrebleLabel'),
        resetEqBtn: document.getElementById('resetEqBtn'),
        eqPresetChips: document.querySelectorAll('.eq-preset-chip'),
        gaplessToggle: document.getElementById('gaplessToggle'),
        devicesListContainer: document.getElementById('devicesListContainer'),
        spectrumBars: document.getElementById('spectrumBars'),
        openDeviceModalBtn: document.getElementById('openDeviceModalBtn'),

        // Cola
        queueModal: document.getElementById('queueModal'),
        openQueueModalBtn: document.getElementById('openQueueModalBtn'),
        closeQueueModalBtn: document.getElementById('closeQueueModalBtn'),
        nowPlayingQueueItem: document.getElementById('nowPlayingQueueItem'),
        queueItemsList: document.getElementById('queueItemsList'),
        clearQueueBtn: document.getElementById('clearQueueBtn'),

        // Telegram
        telegramModal: document.getElementById('telegramModal'),
        closeTelegramModalBtn: document.getElementById('closeTelegramModalBtn'),
        tgStatusBadge: document.getElementById('tgStatusBadge'),
        tgChannelName: document.getElementById('tgChannelName'),
        tgUserLabel: document.getElementById('tgUserLabel'),
        tgPhoneInput: document.getElementById('tgPhoneInput'),
        tgCodeGroup: document.getElementById('tgCodeGroup'),
        tgCodeInput: document.getElementById('tgCodeInput'),
        tgConnectBtn: document.getElementById('tgConnectBtn'),
        tgDisconnectBtn: document.getElementById('tgDisconnectBtn'),

        // Toast
        toastNotification: document.getElementById('toastNotification'),
        toastMessage: document.getElementById('toastMessage')
    };

    // =========================================================================
    // DETECCIÓN DEL PUENTE NATIVO ANDROID (TITAN BRIDGE)
    // =========================================================================

    const hasBridge = typeof window.TitanBridge !== 'undefined';

    function bridgeCall(fnName, ...args) {
        if (hasBridge && typeof window.TitanBridge[fnName] === 'function') {
            try {
                return window.TitanBridge[fnName](...args);
            } catch (err) {
                console.error(`Error llamando a TitanBridge.${fnName}:`, err);
            }
        }
        return null;
    }

    // =========================================================================
    // INICIALIZACIÓN
    // =========================================================================

    function init() {
        updateGreeting();
        initSpectrumVisualizer();
        renderDevices();
        syncWithNativeBridge();
        renderSongs();
        setupEventListeners();
        startPlaybackTicker();

        // Si no hay canción seleccionada, prepara la primera
        if (!state.currentTrack && state.songs.length > 0) {
            setTrack(state.songs[0], false);
        }
    }

    function updateGreeting() {
        const hour = new Date().getHours();
        let greeting = 'Buenas noches';
        if (hour >= 6 && hour < 12) greeting = 'Buenos días';
        else if (hour >= 12 && hour < 20) greeting = 'Buenas tardes';
        el.greetingText.textContent = greeting;
    }

    function initSpectrumVisualizer() {
        el.spectrumBars.innerHTML = '';
        for (let i = 0; i < 32; i++) {
            const bar = document.createElement('div');
            bar.className = 'spec-bar';
            bar.style.height = '4px';
            el.spectrumBars.appendChild(bar);
        }
    }

    // =========================================================================
    // PUENTE CON ANDROID (LOCAL SCAN + TELEGRAM)
    // =========================================================================

    function syncWithNativeBridge() {
        if (!hasBridge) return;

        // Escanear música del teléfono
        const rawLocal = bridgeCall('scanLocalMusic');
        if (rawLocal) {
            try {
                const scanned = JSON.parse(rawLocal);
                if (Array.isArray(scanned) && scanned.length > 0) {
                    // Mantener las de la nube y fusionar con las reales del teléfono
                    const clouds = state.songs.filter(s => s.isCloud);
                    state.songs = [...scanned, ...clouds];
                }
            } catch (e) {
                console.warn('Error parseando scanLocalMusic', e);
            }
        }

        // Estado de Telegram
        const rawTg = bridgeCall('getTelegramStatus');
        if (rawTg) {
            try {
                const tgStatus = JSON.parse(rawTg);
                state.telegram = { ...state.telegram, ...tgStatus };
                updateTelegramUI();
            } catch (e) {
                console.warn('Error parseando TelegramStatus', e);
            }
        }
    }

    function updateTelegramUI() {
        if (state.telegram.authorized) {
            el.cloudDot.className = 'cloud-dot connected';
            el.cloudLabel.textContent = '«Mi Música» Conectada';
            el.tgStatusBadge.textContent = 'Conectado y Autorizado';
            el.tgUserLabel.textContent = state.telegram.user || 'alexsndersoto';
            el.tgConnectBtn.textContent = 'Actualizar Datos';
            el.tgDisconnectBtn.style.display = 'block';
        } else {
            el.cloudDot.className = 'cloud-dot';
            el.cloudLabel.textContent = 'Conectar Nube';
            el.tgStatusBadge.textContent = 'Sin conectar';
            el.tgUserLabel.textContent = 'Desconectado';
            el.tgConnectBtn.textContent = 'Conectar con Telegram';
            el.tgDisconnectBtn.style.display = 'none';
        }
    }

    // =========================================================================
    // RENDERIZADO DE CANCIONES (BIBLIOTECA DOBLE)
    // =========================================================================

    function renderSongs() {
        const query = state.searchQuery.toLowerCase().trim();
        const filtered = state.songs.filter(song => {
            // Filtro por pestaña
            if (state.filter === 'phone' && song.isCloud) return false;
            if (state.filter === 'cloud' && !song.isCloud) return false;

            // Filtro por búsqueda
            if (query) {
                const matchTitle = song.title.toLowerCase().includes(query);
                const matchArtist = song.artist.toLowerCase().includes(query);
                const matchAlbum = (song.album || '').toLowerCase().includes(query);
                return matchTitle || matchArtist || matchAlbum;
            }
            return true;
        });

        // Contadores
        const phoneCount = state.songs.filter(s => !s.isCloud).length;
        const cloudCount = state.songs.filter(s => s.isCloud).length;
        el.summaryPhone.textContent = `${phoneCount} en el teléfono 📱`;
        el.summaryCloud.textContent = `${cloudCount} en la nube Telegram ☁️`;

        el.songsList.innerHTML = '';

        if (filtered.length === 0) {
            el.songsList.innerHTML = `
                <div style="text-align: center; padding: 48px 16px; color: var(--text-secondary);">
                    <div style="font-size: 2.5rem; margin-bottom: 12px;">🔍</div>
                    <div style="font-weight: 700; font-size: 1.1rem; color: #fff; margin-bottom: 4px;">No se encontraron canciones</div>
                    <p style="font-size: 0.85rem;">Prueba con otra búsqueda o cambia de pestaña.</p>
                </div>
            `;
            return;
        }

        filtered.forEach((song, index) => {
            const isCurrent = state.currentTrack && state.currentTrack.id === song.id;
            const item = document.createElement('div');
            item.className = `song-item ${isCurrent ? 'playing' : ''}`;
            item.onclick = () => playSong(song);

            const badgeHtml = song.isCloud
                ? `<span class="source-badge cloud">☁️ Telegram</span>`
                : `<span class="source-badge phone">📱 Teléfono</span>`;

            // Botón de descarga explícita si es canción de la nube
            let downloadBtnHtml = '';
            if (song.isCloud) {
                if (song.downloaded) {
                    downloadBtnHtml = `<span class="download-badge-btn downloaded" title="Guardada en el teléfono">✓ Guardada</span>`;
                } else {
                    downloadBtnHtml = `<button class="download-badge-btn" onclick="event.stopPropagation(); downloadSong('${song.id}')" title="Descargar explícitamente al teléfono">⬇️ Guardar</button>`;
                }
            }

            item.innerHTML = `
                <div class="song-cover-container" style="background: linear-gradient(135deg, ${song.color || '#2b3a2f'}, #121212);">
                    <span class="song-cover-placeholder">${isCurrent && state.isPlaying ? '🔊' : '🎵'}</span>
                </div>
                <div class="song-info-col">
                    <div class="song-title">${escapeHtml(song.title)}</div>
                    <div class="song-artist-row">
                        ${badgeHtml}
                        <span class="song-artist">${escapeHtml(song.artist)}</span>
                    </div>
                </div>
                <div class="song-actions-col">
                    ${downloadBtnHtml}
                    <span class="song-duration">${formatTime(song.duration)}</span>
                    <button class="action-icon-btn" onclick="event.stopPropagation(); addToQueue('${song.id}')" title="Añadir a la cola">➕</button>
                </div>
            `;
            el.songsList.appendChild(item);
        });
    }

    // =========================================================================
    // CONTROL DE REPRODUCCIÓN (PLAY, PAUSE, SEEK, NEXT, PREV)
    // =========================================================================

    function setTrack(song, autoplay = true) {
        state.currentTrack = song;
        state.duration = song.duration || 210;
        state.position = 0;

        // Actualizar UI Mini Player
        el.miniTitle.textContent = song.title;
        el.miniArtist.textContent = song.artist;
        el.miniSourceBadge.textContent = song.isCloud ? '☁️ Telegram' : '📱 Teléfono';
        el.miniSourceBadge.style.color = song.isCloud ? 'var(--cloud-blue)' : 'var(--spotify-green)';
        el.miniCover.style.background = `linear-gradient(135deg, ${song.color || '#2b3a2f'}, #121212)`;
        el.miniPlayer.style.display = 'block';

        // Actualizar UI Full Player
        el.fullTitle.textContent = song.title;
        el.fullArtist.textContent = song.artist;
        el.fullCover.style.background = `linear-gradient(135deg, ${song.color || '#2b3a2f'}, #121212)`;
        el.totalDuration.textContent = formatTime(state.duration);
        el.currentTime.textContent = '0:00';
        el.seekSlider.value = 0;

        if (song.isCloud) {
            el.streamIcon.textContent = '☁️';
            el.streamText.textContent = 'Streaming puro de Telegram (0 MB ocupados en disco)';
            el.streamingIndicator.style.background = 'rgba(42, 171, 238, 0.15)';
            el.streamingIndicator.style.color = 'var(--cloud-blue)';
        } else {
            el.streamIcon.textContent = '📱';
            el.streamText.textContent = 'Reproduciendo desde el almacenamiento del teléfono';
            el.streamingIndicator.style.background = 'rgba(29, 185, 84, 0.15)';
            el.streamingIndicator.style.color = 'var(--spotify-green)';
        }

        renderSongs();
        updateQueueModal();

        if (autoplay) {
            playCurrent();
        }
    }

    function playSong(song) {
        setTrack(song, true);
    }

    function playCurrent() {
        if (!state.currentTrack) return;
        state.isPlaying = true;

        // Llamar al reproductor nativo Android
        bridgeCall('playTrack', state.currentTrack.path, state.currentTrack.title, state.currentTrack.artist, state.currentTrack.isCloud);

        updatePlayPauseUI();
    }

    function pauseCurrent() {
        state.isPlaying = false;
        bridgeCall('pauseTrack');
        updatePlayPauseUI();
    }

    function togglePlayPause() {
        if (state.isPlaying) {
            pauseCurrent();
        } else {
            playCurrent();
        }
    }

    function updatePlayPauseUI() {
        const icon = state.isPlaying ? '⏸' : '▶';
        el.miniPlayPauseBtn.textContent = icon;
        el.mainPlayIcon.textContent = icon;
        renderSongs();
    }

    function nextTrack() {
        if (state.queue.length > 0) {
            const next = state.queue.shift();
            state.history.push(state.currentTrack);
            setTrack(next, true);
        } else {
            // Siguiente canción en la lista
            const currentIndex = state.songs.findIndex(s => s.id === state.currentTrack.id);
            if (currentIndex !== -1 && currentIndex + 1 < state.songs.length) {
                state.history.push(state.currentTrack);
                setTrack(state.songs[currentIndex + 1], true);
            } else if (state.repeat === 'all' && state.songs.length > 0) {
                setTrack(state.songs[0], true);
            } else {
                pauseCurrent();
                state.position = 0;
            }
        }
        updateQueueModal();
    }

    function prevTrack() {
        if (state.position > 3) {
            // Reiniciar la pista si ya pasaron 3 segundos
            seekTo(0);
        } else if (state.history.length > 0) {
            const prev = state.history.pop();
            setTrack(prev, true);
        } else {
            const currentIndex = state.songs.findIndex(s => s.id === state.currentTrack.id);
            if (currentIndex > 0) {
                setTrack(state.songs[currentIndex - 1], true);
            } else {
                seekTo(0);
            }
        }
        updateQueueModal();
    }

    function seekTo(seconds) {
        state.position = Math.max(0, Math.min(state.duration, seconds));
        bridgeCall('seekTo', state.position);
        updateSeekUI();
    }

    function updateSeekUI() {
        const percent = state.duration > 0 ? (state.position / state.duration) * 100 : 0;
        el.seekSlider.value = percent;
        el.miniProgressFill.style.width = `${percent}%`;
        el.currentTime.textContent = formatTime(state.position);
    }

    // TICKER DE SEGUNDO A SEGUNDO PARA EL TIEMPO Y VISUALIZADOR
    function startPlaybackTicker() {
        setInterval(() => {
            if (state.isPlaying) {
                state.position += 1;
                if (state.position >= state.duration) {
                    if (state.repeat === 'one') {
                        state.position = 0;
                    } else {
                        nextTrack();
                    }
                }
                updateSeekUI();
            }

            // Actualizar 32 barras del visualizador de espectro
            updateSpectrumBars();
        }, 1000);
    }

    function updateSpectrumBars() {
        let levels = [];
        const raw = bridgeCall('getSpectrumLevels');
        if (raw) {
            try {
                levels = JSON.parse(raw);
            } catch (ignored) {}
        }

        // Si no hay datos nativos, generar simulación suave al compás de la música
        if (!levels || levels.length !== 32) {
            levels = [];
            for (let i = 0; i < 32; i++) {
                if (!state.isPlaying) {
                    levels.push(0.05);
                } else {
                    const rnd = 0.2 + 0.7 * Math.random();
                    const boost = (i < 10 ? state.eq.bass : (i < 22 ? state.eq.mid : state.eq.treble)) / 24;
                    levels.push(Math.max(0.08, Math.min(1.0, (rnd + boost) * (state.volume / 100))));
                }
            }
        }

        const bars = el.spectrumBars.children;
        for (let i = 0; i < bars.length && i < levels.length; i++) {
            const h = Math.round(levels[i] * 46);
            bars[i].style.height = `${Math.max(3, h)}px`;
        }
    }

    // =========================================================================
    // AJUSTES AVANZADOS (VOLUMEN, CROSSFADE, EQ, GAPLESS, DISPOSITIVO)
    // =========================================================================

    function setVolume(val) {
        state.volume = parseInt(val, 10);
        el.volumeValueLabel.textContent = `${state.volume}%`;
        el.volumeSlider.value = state.volume;
        bridgeCall('setVolume', state.volume);
    }

    function setCrossfade(seconds) {
        state.crossfade = parseFloat(seconds);
        el.crossfadeValueLabel.textContent = state.crossfade === 0 ? 'Desactivado' : `${state.crossfade.toFixed(1)} s`;
        el.crossfadeSlider.value = state.crossfade;
        
        el.crossfadeChips.forEach(chip => {
            const chipCf = parseFloat(chip.dataset.cf);
            chip.classList.toggle('active', chipCf === state.crossfade);
        });

        bridgeCall('setCrossfade', state.crossfade);
        showToast(state.crossfade === 0 ? 'Crossfade desactivado' : `Crossfade: ${state.crossfade} s`);
    }

    function setEqualizer(bass, mid, treble) {
        state.eq.bass = parseInt(bass, 10);
        state.eq.mid = parseInt(mid, 10);
        state.eq.treble = parseInt(treble, 10);

        el.eqBass.value = state.eq.bass;
        el.eqMid.value = state.eq.mid;
        el.eqTreble.value = state.eq.treble;

        el.eqBassLabel.textContent = `${state.eq.bass > 0 ? '+' : ''}${state.eq.bass} dB`;
        el.eqMidLabel.textContent = `${state.eq.mid > 0 ? '+' : ''}${state.eq.mid} dB`;
        el.eqTrebleLabel.textContent = `${state.eq.treble > 0 ? '+' : ''}${state.eq.treble} dB`;

        bridgeCall('setEqualizer', state.eq.bass, state.eq.mid, state.eq.treble);
    }

    function setGapless(enabled) {
        state.gapless = enabled;
        el.gaplessToggle.checked = enabled;
        bridgeCall('setGapless', enabled);
        showToast(enabled ? 'Reproducción sin pausas (Gapless) activada' : 'Gapless desactivado');
    }

    function renderDevices() {
        const raw = bridgeCall('getAudioDevices');
        if (raw) {
            try {
                const devs = JSON.parse(raw);
                if (Array.isArray(devs) && devs.length > 0) {
                    state.devices = devs;
                }
            } catch (ignored) {}
        }

        el.devicesListContainer.innerHTML = '';
        state.devices.forEach(dev => {
            const isActive = dev.name === state.activeDevice || dev.active;
            const item = document.createElement('div');
            item.className = `device-item ${isActive ? 'active' : ''}`;
            item.onclick = () => selectDevice(dev);

            let icon = '📢';
            if (dev.type === 'wired') icon = '🎧';
            if (dev.type === 'bluetooth') icon = '📶';

            item.innerHTML = `
                <span class="device-icon">${icon}</span>
                <span class="device-name">${escapeHtml(dev.name)}</span>
                ${isActive ? '<span style="margin-left: auto; color: var(--spotify-green); font-weight: 700;">✓</span>' : ''}
            `;
            el.devicesListContainer.appendChild(item);
        });
    }

    function selectDevice(dev) {
        state.activeDevice = dev.name;
        el.activeDeviceLabel.textContent = dev.name.split(' ')[0];
        renderDevices();
        showToast(`Dispositivo: ${dev.name}`);
    }

    // =========================================================================
    // COLA DE REPRODUCCIÓN (QUEUE)
    // =========================================================================

    function addToQueue(songId) {
        const song = state.songs.find(s => s.id === songId);
        if (song) {
            state.queue.push(song);
            updateQueueModal();
            showToast(`Añadida a la cola: ${song.title}`);
        }
    }

    function removeFromQueue(index) {
        if (index >= 0 && index < state.queue.length) {
            const removed = state.queue.splice(index, 1)[0];
            updateQueueModal();
            showToast(`Eliminada de la cola: ${removed.title}`);
        }
    }

    function clearQueue() {
        state.queue = [];
        updateQueueModal();
        showToast('Cola de reproducción vaciada');
    }

    function updateQueueModal() {
        el.queueBadgeCount.textContent = state.queue.length;

        // Pista sonando ahora
        if (state.currentTrack) {
            el.nowPlayingQueueItem.innerHTML = `
                <div class="queue-item" style="border-left: 3px solid var(--spotify-green);">
                    <div style="font-size: 1.2rem;">🔊</div>
                    <div class="queue-item-info">
                        <div class="queue-item-title">${escapeHtml(state.currentTrack.title)}</div>
                        <div class="queue-item-artist">${escapeHtml(state.currentTrack.artist)}</div>
                    </div>
                    <span style="font-size: 0.75rem; color: var(--spotify-green); font-weight: 700;">Sonando</span>
                </div>
            `;
        } else {
            el.nowPlayingQueueItem.innerHTML = '<p style="color: var(--text-secondary); font-size: 0.85rem;">Nada reproduciéndose.</p>';
        }

        // Pistas siguientes
        el.queueItemsList.innerHTML = '';
        if (state.queue.length === 0) {
            el.queueItemsList.innerHTML = '<p style="color: var(--text-subdued); font-size: 0.85rem; padding: 12px 0;">La cola está vacía. Añade canciones con el botón ➕.</p>';
            return;
        }

        state.queue.forEach((song, idx) => {
            const row = document.createElement('div');
            row.className = 'queue-item';
            row.innerHTML = `
                <span style="color: var(--text-subdued); font-size: 0.8rem; width: 18px;">${idx + 1}</span>
                <div class="queue-item-info">
                    <div class="queue-item-title">${escapeHtml(song.title)}</div>
                    <div class="queue-item-artist">${escapeHtml(song.artist)}</div>
                </div>
                <button class="queue-item-remove" onclick="event.stopPropagation(); window.removeFromQueueByIndex(${idx})" title="Quitar de la cola">✕</button>
            `;
            el.queueItemsList.appendChild(row);
        });
    }

    window.removeFromQueueByIndex = removeFromQueue;

    // =========================================================================
    // DESCARGA EXPLÍCITA DE CANCIONES DE TELEGRAM (SIN LLENAR EL TELÉFONO)
    // =========================================================================

    function downloadSong(songId) {
        const song = state.songs.find(s => s.id === songId);
        if (!song || !song.isCloud) return;

        showToast(`Descargando "${song.title}" al teléfono...`);
        
        // Simular o ejecutar descarga con el cliente nativo
        bridgeCall('downloadCloudTrack', song.cloudId ? String(song.cloudId) : song.id, song.title, song.artist);

        setTimeout(() => {
            song.downloaded = true;
            renderSongs();
            showToast(`✓ "${song.title}" guardada en el teléfono para escuchar sin internet`);
        }, 1200);
    }

    // =========================================================================
    // EVENT LISTENERS
    // =========================================================================

    function setupEventListeners() {
        // Filtros chips
        el.filterChips.forEach(chip => {
            chip.addEventListener('click', () => {
                el.filterChips.forEach(c => c.classList.remove('active'));
                chip.classList.add('active');
                state.filter = chip.dataset.filter;
                renderSongs();
            });
        });

        // Búsqueda
        el.searchInput.addEventListener('input', (e) => {
            state.searchQuery = e.target.value;
            el.searchClearBtn.style.display = state.searchQuery ? 'block' : 'none';
            renderSongs();
        });

        el.searchClearBtn.addEventListener('click', () => {
            state.searchQuery = '';
            el.searchInput.value = '';
            el.searchClearBtn.style.display = 'none';
            renderSongs();
        });

        // Mini player
        el.expandPlayerBtn.addEventListener('click', (e) => {
            if (!e.target.closest('.mini-control-btn')) {
                openFullPlayer();
            }
        });
        el.miniPlayPauseBtn.addEventListener('click', togglePlayPause);
        el.miniNextBtn.addEventListener('click', nextTrack);

        // Full player
        el.collapsePlayerBtn.addEventListener('click', closeFullPlayer);
        el.mainPlayPauseBtn.addEventListener('click', togglePlayPause);
        el.nextBtn.addEventListener('click', nextTrack);
        el.prevBtn.addEventListener('click', prevTrack);

        el.likeBtn.addEventListener('click', () => {
            el.likeBtn.classList.toggle('liked');
            const liked = el.likeBtn.classList.contains('liked');
            el.likeBtn.textContent = liked ? '♥' : '♡';
            showToast(liked ? 'Guardada en Tus canciones que te gustan' : 'Eliminada de Tus me gusta');
        });

        el.shuffleBtn.addEventListener('click', () => {
            state.shuffle = !state.shuffle;
            el.shuffleBtn.classList.toggle('active', state.shuffle);
            showToast(state.shuffle ? 'Modo aleatorio activado' : 'Modo aleatorio desactivado');
        });

        el.repeatBtn.addEventListener('click', () => {
            if (state.repeat === 'off') {
                state.repeat = 'all';
                el.repeatBtn.classList.add('active');
                el.repeatBtn.textContent = '🔁';
                showToast('Repetir todo');
            } else if (state.repeat === 'all') {
                state.repeat = 'one';
                el.repeatBtn.classList.add('active');
                el.repeatBtn.textContent = '🔂';
                showToast('Repetir una canción');
            } else {
                state.repeat = 'off';
                el.repeatBtn.classList.remove('active');
                el.repeatBtn.textContent = '🔁';
                showToast('Repetición desactivada');
            }
        });

        el.seekSlider.addEventListener('input', (e) => {
            const percent = parseFloat(e.target.value);
            const targetSec = (percent / 100) * state.duration;
            el.currentTime.textContent = formatTime(targetSec);
        });

        el.seekSlider.addEventListener('change', (e) => {
            const percent = parseFloat(e.target.value);
            seekTo((percent / 100) * state.duration);
        });

        // MODAL AJUSTES AVANZADOS (Lo que más importa)
        el.openSettingsModalBtn.addEventListener('click', openSettingsModal);
        el.closeSettingsModalBtn.addEventListener('click', closeSettingsModal);

        el.volumeSlider.addEventListener('input', (e) => setVolume(e.target.value));

        el.crossfadeSlider.addEventListener('input', (e) => setCrossfade(e.target.value));

        el.crossfadeChips.forEach(chip => {
            chip.addEventListener('click', () => setCrossfade(chip.dataset.cf));
        });

        // Sliders de ecualizador
        [el.eqBass, el.eqMid, el.eqTreble].forEach(slider => {
            slider.addEventListener('input', () => {
                setEqualizer(el.eqBass.value, el.eqMid.value, el.eqTreble.value);
                el.eqPresetChips.forEach(c => c.classList.remove('active'));
            });
        });

        el.resetEqBtn.addEventListener('click', () => {
            setEqualizer(0, 0, 0);
            el.eqPresetChips.forEach(c => c.classList.remove('active'));
            el.eqPresetChips[0].classList.add('active');
            showToast('Ecualizador restablecido a 0 dB');
        });

        el.eqPresetChips.forEach(chip => {
            chip.addEventListener('click', () => {
                el.eqPresetChips.forEach(c => c.classList.remove('active'));
                chip.classList.add('active');
                const [b, m, t] = chip.dataset.eq.split(',').map(Number);
                setEqualizer(b, m, t);
                showToast(`Preset EQ: ${chip.textContent}`);
            });
        });

        el.gaplessToggle.addEventListener('change', (e) => setGapless(e.target.checked));

        // MODAL COLA
        el.openQueueModalBtn.addEventListener('click', openQueueModal);
        el.closeQueueModalBtn.addEventListener('click', closeQueueModal);
        el.clearQueueBtn.addEventListener('click', clearQueue);

        // MODAL TELEGRAM
        el.openTelegramModalBtn.addEventListener('click', openTelegramModal);
        el.closeTelegramModalBtn.addEventListener('click', closeTelegramModal);

        el.tgConnectBtn.addEventListener('click', () => {
            const phone = el.tgPhoneInput.value.trim();
            if (!phone) {
                showToast('Introduce tu número de teléfono');
                return;
            }
            if (el.tgCodeGroup.style.display === 'none') {
                el.tgCodeGroup.style.display = 'block';
                showToast('Código de confirmación enviado por Telegram');
                el.tgConnectBtn.textContent = 'Verificar Código';
            } else {
                const code = el.tgCodeInput.value.trim();
                bridgeCall('verifyTelegramCode', code, '');
                state.telegram.authorized = true;
                state.telegram.user = phone;
                updateTelegramUI();
                closeTelegramModal();
                showToast('✓ Conectado a tu canal privado «Mi Música»');
            }
        });

        el.tgDisconnectBtn.addEventListener('click', () => {
            bridgeCall('logoutTelegram');
            state.telegram.authorized = false;
            updateTelegramUI();
            closeTelegramModal();
            showToast('Sesión de Telegram cerrada');
        });

        // Selector rápido de dispositivo
        el.openDeviceModalBtn.addEventListener('click', () => {
            openSettingsModal();
            setTimeout(() => {
                el.devicesListContainer.scrollIntoView({ behavior: 'smooth' });
            }, 300);
        });

        // Navegación inferior
        document.getElementById('navSearchBtn').addEventListener('click', () => {
            el.searchInput.focus();
            window.scrollTo({ top: 0, behavior: 'smooth' });
        });
        document.getElementById('navLibraryBtn').addEventListener('click', () => {
            el.filterChips[0].click();
            window.scrollTo({ top: 0, behavior: 'smooth' });
        });
    }

    // =========================================================================
    // APERTURA Y CIERRE DE MODALES
    // =========================================================================

    function openFullPlayer() {
        el.fullPlayer.classList.add('open');
    }

    function closeFullPlayer() {
        el.fullPlayer.classList.remove('open');
    }

    function openSettingsModal() {
        el.settingsModal.classList.add('open');
    }

    function closeSettingsModal() {
        el.settingsModal.classList.remove('open');
    }

    function openQueueModal() {
        updateQueueModal();
        el.queueModal.classList.add('open');
    }

    function closeQueueModal() {
        el.queueModal.classList.remove('open');
    }

    function openTelegramModal() {
        updateTelegramUI();
        el.telegramModal.classList.add('open');
    }

    function closeTelegramModal() {
        el.telegramModal.classList.remove('open');
    }

    // Soporte para botón atrás de Android
    window.handleAndroidBack = function () {
        if (el.settingsModal.classList.contains('open')) {
            closeSettingsModal();
            return 'handled';
        }
        if (el.queueModal.classList.contains('open')) {
            closeQueueModal();
            return 'handled';
        }
        if (el.telegramModal.classList.contains('open')) {
            closeTelegramModal();
            return 'handled';
        }
        if (el.fullPlayer.classList.contains('open')) {
            closeFullPlayer();
            return 'handled';
        }
        return 'back';
    };

    window.onPermissionsGranted = function () {
        syncWithNativeBridge();
        renderSongs();
        showToast('Biblioteca local sincronizada con el teléfono');
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
        el.toastMessage.textContent = message;
        el.toastNotification.classList.add('show');
        if (toastTimer) clearTimeout(toastTimer);
        toastTimer = setTimeout(() => {
            el.toastNotification.classList.remove('show');
        }, 3200);
    }

    window.downloadSong = downloadSong;
    window.addToQueue = addToQueue;

    // INICIAR AL CARGAR
    document.addEventListener('DOMContentLoaded', init);

})();

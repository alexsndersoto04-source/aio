package com.titan.music;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.media.AudioAttributes;
import android.media.AudioManager;
import android.media.MediaPlayer;
import android.media.PlaybackParams;
import android.media.audiofx.BassBoost;
import android.media.audiofx.Equalizer;
import android.media.audiofx.Virtualizer;
import android.net.Uri;
import android.os.Binder;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.support.v4.media.MediaMetadataCompat;
import android.support.v4.media.session.MediaSessionCompat;
import android.support.v4.media.session.PlaybackStateCompat;

import androidx.annotation.Nullable;
import androidx.core.app.NotificationCompat;

import org.json.JSONObject;

import java.io.InputStream;

/**
 * Servicio Nativo de Reproducción en Primer Plano con Integración Completa de AudioFX.
 * Incorpora Ecualizador de Hardware, Bass Boost, Virtualizador 3D, AudioFocus y MediaSession.
 */
public class AudioPlaybackService extends Service implements
        MediaPlayer.OnPreparedListener,
        MediaPlayer.OnCompletionListener,
        MediaPlayer.OnErrorListener,
        AudioManager.OnAudioFocusChangeListener {

    public interface PlaybackEventListener {
        void onPlaybackStateChanged(boolean isPlaying, String title, String artist);
        void onNextRequested();
        void onPreviousRequested();
    }

    public static final String ACTION_PLAY = "com.titan.music.ACTION_PLAY";
    public static final String ACTION_PAUSE = "com.titan.music.ACTION_PAUSE";
    public static final String ACTION_TOGGLE = "com.titan.music.ACTION_TOGGLE";
    public static final String ACTION_NEXT = "com.titan.music.ACTION_NEXT";
    public static final String ACTION_PREV = "com.titan.music.ACTION_PREV";
    public static final String ACTION_STOP = "com.titan.music.ACTION_STOP";

    private static final String CHANNEL_ID = "titan_music_playback_channel";
    private static final int NOTIFICATION_ID = 1001;

    private final IBinder binder = new LocalBinder();
    private MediaPlayer mediaPlayer;
    private AudioManager audioManager;
    private MediaSessionCompat mediaSession;

    // Efectos de Hardware Nativo de Android
    private Equalizer hardwareEqualizer;
    private BassBoost hardwareBassBoost;
    private Virtualizer hardwareVirtualizer;

    private PlaybackEventListener eventListener;

    private String currentTitle = "Titan Music";
    private String currentArtist = "Reproductor Hi-Fi";
    private String currentPath = "";
    private long currentDurationMs = 0;
    private Bitmap currentArtwork = null;
    private boolean isPrepared = false;
    private float currentPlaybackSpeed = 1.0f;
    private int currentVolume = 100;
    private float crossfadeSeconds = 0.0f;
    private boolean gaplessEnabled = true;

    private final Handler mainHandler = new Handler(Looper.getMainLooper());

    public class LocalBinder extends Binder {
        public AudioPlaybackService getService() {
            return AudioPlaybackService.this;
        }
    }

    private final BroadcastReceiver becomingNoisyReceiver = new BroadcastReceiver() {
        @Override
        public void onReceive(Context context, Intent intent) {
            if (AudioManager.ACTION_AUDIO_BECOMING_NOISY.equals(intent.getAction())) {
                pause();
            }
        }
    };

    @Override
    public void onCreate() {
        super.onCreate();
        audioManager = (AudioManager) getSystemService(Context.AUDIO_SERVICE);
        createNotificationChannel();
        initMediaSession();
        initMediaPlayer();

        IntentFilter filter = new IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY);
        registerReceiver(becomingNoisyReceiver, filter);
    }

    public void setPlaybackEventListener(PlaybackEventListener listener) {
        this.eventListener = listener;
    }

    private void initMediaPlayer() {
        if (mediaPlayer != null) {
            releaseAudioEffects();
            mediaPlayer.release();
        }

        mediaPlayer = new MediaPlayer();
        mediaPlayer.setAudioAttributes(new AudioAttributes.Builder()
                .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                .setUsage(AudioAttributes.USAGE_MEDIA)
                .build());

        mediaPlayer.setOnPreparedListener(this);
        mediaPlayer.setOnCompletionListener(this);
        mediaPlayer.setOnErrorListener(this);

        initAudioEffects();
    }

    private void initAudioEffects() {
        try {
            int audioSessionId = mediaPlayer.getAudioSessionId();
            if (audioSessionId != 0) {
                hardwareEqualizer = new Equalizer(0, audioSessionId);
                hardwareEqualizer.setEnabled(true);

                hardwareBassBoost = new BassBoost(0, audioSessionId);
                hardwareBassBoost.setEnabled(true);

                hardwareVirtualizer = new Virtualizer(0, audioSessionId);
                hardwareVirtualizer.setEnabled(true);
            }
        } catch (Exception e) {
            android.util.Log.w("AudioService", "AudioFX inicialización info: " + e.getMessage());
        }
    }

    private void releaseAudioEffects() {
        try {
            if (hardwareEqualizer != null) {
                hardwareEqualizer.setEnabled(false);
                hardwareEqualizer.release();
                hardwareEqualizer = null;
            }
            if (hardwareBassBoost != null) {
                hardwareBassBoost.setEnabled(false);
                hardwareBassBoost.release();
                hardwareBassBoost = null;
            }
            if (hardwareVirtualizer != null) {
                hardwareVirtualizer.setEnabled(false);
                hardwareVirtualizer.release();
                hardwareVirtualizer = null;
            }
        } catch (Exception ignored) {}
    }

    private void initMediaSession() {
        mediaSession = new MediaSessionCompat(this, "TitanAudioSession");
        mediaSession.setCallback(new MediaSessionCompat.Callback() {
            @Override
            public void onPlay() { resume(); }
            @Override
            public void onPause() { pause(); }
            @Override
            public void onSkipToNext() {
                if (eventListener != null) eventListener.onNextRequested();
                triggerWebEvent("onNextRequested");
            }
            @Override
            public void onSkipToPrevious() {
                if (eventListener != null) eventListener.onPreviousRequested();
                triggerWebEvent("onPrevRequested");
            }
            @Override
            public void onStop() { stop(); }
            @Override
            public void onSeekTo(long pos) { seekTo((int) pos); }
        });
        mediaSession.setActive(true);
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && intent.getAction() != null) {
            String action = intent.getAction();
            switch (action) {
                case ACTION_PLAY:
                    resume();
                    break;
                case ACTION_PAUSE:
                    pause();
                    break;
                case ACTION_TOGGLE:
                    if (isPlaying()) pause();
                    else resume();
                    break;
                case ACTION_NEXT:
                    if (eventListener != null) eventListener.onNextRequested();
                    triggerWebEvent("onNextRequested");
                    break;
                case ACTION_PREV:
                    if (eventListener != null) eventListener.onPreviousRequested();
                    triggerWebEvent("onPrevRequested");
                    break;
                case ACTION_STOP:
                    stop();
                    stopSelf();
                    break;
            }
        }
        return START_NOT_STICKY;
    }

    // =========================================================================
    // API PÚBLICA DE REPRODUCCIÓN (MÉTODOS NATIVOS)
    // =========================================================================

    public boolean play(String path, String title, String artist) {
        return playUri(path, title, artist, null);
    }

    public boolean playUri(String uriString, String title, String artist, @Nullable String artworkUriStr) {
        currentTitle = (title != null && !title.isEmpty()) ? title : "Canción";
        currentArtist = (artist != null && !artist.isEmpty()) ? artist : "Artista";
        currentPath = uriString;
        isPrepared = false;

        loadArtworkBitmap(artworkUriStr);

        int result = audioManager.requestAudioFocus(this, AudioManager.STREAM_MUSIC, AudioManager.AUDIOFOCUS_GAIN);
        if (result != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
            return false;
        }

        try {
            initMediaPlayer();
            Uri uri = Uri.parse(uriString);
            mediaPlayer.setDataSource(this, uri);
            mediaPlayer.prepareAsync();

            startForeground(NOTIFICATION_ID, buildNotification(false));
            return true;
        } catch (Exception e) {
            android.util.Log.e("AudioService", "Error reproduciendo archivo: " + uriString, e);
            return false;
        }
    }

    private void loadArtworkBitmap(@Nullable String uriStr) {
        currentArtwork = null;
        if (uriStr != null && !uriStr.isEmpty()) {
            try {
                Uri artUri = Uri.parse(uriStr);
                InputStream is = getContentResolver().openInputStream(artUri);
                if (is != null) {
                    currentArtwork = BitmapFactory.decodeStream(is);
                    is.close();
                }
            } catch (Exception ignored) {}
        }
    }

    public void resume() {
        if (mediaPlayer != null && isPrepared) {
            int result = audioManager.requestAudioFocus(this, AudioManager.STREAM_MUSIC, AudioManager.AUDIOFOCUS_GAIN);
            if (result == AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
                mediaPlayer.start();
                applyPlaybackSpeed(currentPlaybackSpeed);
                updateNotificationAndSession(true);
                notifyStateChanged(true);
            }
        }
    }

    public void pause() {
        if (mediaPlayer != null && mediaPlayer.isPlaying()) {
            mediaPlayer.pause();
            updateNotificationAndSession(false);
            notifyStateChanged(false);
        }
    }

    public void stop() {
        if (mediaPlayer != null) {
            if (mediaPlayer.isPlaying()) {
                mediaPlayer.stop();
            }
            mediaPlayer.reset();
            isPrepared = false;
        }
        audioManager.abandonAudioFocus(this);
        updateNotificationAndSession(false);
        notifyStateChanged(false);
        stopForeground(true);
    }

    public void seekTo(int msec) {
        if (mediaPlayer != null && isPrepared) {
            mediaPlayer.seekTo(msec);
            updateNotificationAndSession(isPlaying());
        }
    }

    public boolean isPlaying() {
        try {
            return mediaPlayer != null && isPrepared && mediaPlayer.isPlaying();
        } catch (Exception e) {
            return false;
        }
    }

    public int getPosition() {
        try {
            return (mediaPlayer != null && isPrepared) ? mediaPlayer.getCurrentPosition() : 0;
        } catch (Exception e) {
            return 0;
        }
    }

    public int getDuration() {
        try {
            return (mediaPlayer != null && isPrepared) ? mediaPlayer.getDuration() : (int) currentDurationMs;
        } catch (Exception e) {
            return (int) currentDurationMs;
        }
    }

    public int getVolume() {
        return currentVolume;
    }

    public void setVolume(int volume) {
        this.currentVolume = Math.max(0, Math.min(100, volume));
        if (mediaPlayer != null) {
            float vol = currentVolume / 100.0f;
            mediaPlayer.setVolume(vol, vol);
        }
    }

    public float getCrossfade() {
        return crossfadeSeconds;
    }

    public void setCrossfade(float seconds) {
        this.crossfadeSeconds = Math.max(0.0f, Math.min(12.0f, seconds));
    }

    public boolean isGapless() {
        return gaplessEnabled;
    }

    public void setGapless(boolean enabled) {
        this.gaplessEnabled = enabled;
    }

    public void setPlaybackSpeed(float speed) {
        currentPlaybackSpeed = speed;
        applyPlaybackSpeed(speed);
    }

    private void applyPlaybackSpeed(float speed) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M && mediaPlayer != null && isPrepared) {
            try {
                PlaybackParams params = mediaPlayer.getPlaybackParams();
                params.setSpeed(speed);
                mediaPlayer.setPlaybackParams(params);
            } catch (Exception ignored) {}
        }
    }

    // =========================================================================
    // EFECTOS DE HARDWARE NATIVOS (AUDIO FX)
    // =========================================================================

    public void setEqualizer(int bassDb, int midDb, int trebleDb) {
        if (hardwareEqualizer == null) return;
        try {
            short numBands = hardwareEqualizer.getNumberOfBands();
            if (numBands <= 0) return;

            short minEq = hardwareEqualizer.getBandLevelRange()[0];
            short maxEq = hardwareEqualizer.getBandLevelRange()[1];

            short bassLevel = (short) Math.max(minEq, Math.min(maxEq, bassDb * 100));
            short midLevel = (short) Math.max(minEq, Math.min(maxEq, midDb * 100));
            short trebleLevel = (short) Math.max(minEq, Math.min(maxEq, trebleDb * 100));

            if (numBands >= 5) {
                hardwareEqualizer.setBandLevel((short) 0, bassLevel);
                hardwareEqualizer.setBandLevel((short) 1, bassLevel);
                hardwareEqualizer.setBandLevel((short) 2, midLevel);
                hardwareEqualizer.setBandLevel((short) 3, trebleLevel);
                hardwareEqualizer.setBandLevel((short) 4, trebleLevel);
            } else if (numBands >= 3) {
                hardwareEqualizer.setBandLevel((short) 0, bassLevel);
                hardwareEqualizer.setBandLevel((short) 1, midLevel);
                hardwareEqualizer.setBandLevel((short) 2, trebleLevel);
            }
        } catch (Exception ignored) {}
    }

    public void setBassBoost(int strengthPercent) {
        if (hardwareBassBoost == null) return;
        try {
            short strength = (short) Math.max(0, Math.min(1000, strengthPercent * 10));
            hardwareBassBoost.setStrength(strength);
        } catch (Exception ignored) {}
    }

    public void setVirtualizer(int strengthPercent) {
        if (hardwareVirtualizer == null) return;
        try {
            short strength = (short) Math.max(0, Math.min(1000, strengthPercent * 10));
            hardwareVirtualizer.setStrength(strength);
        } catch (Exception ignored) {}
    }

    // =========================================================================
    // ESTADO JSON PARA EL FRONTEND
    // =========================================================================

    public String getPlaybackStatusJson() {
        try {
            JSONObject obj = new JSONObject();
            obj.put("playing", isPlaying());
            obj.put("position", getPosition() / 1000);
            obj.put("duration", getDuration() / 1000);
            obj.put("title", currentTitle);
            obj.put("artist", currentArtist);
            obj.put("volume", currentVolume);
            obj.put("speed", (double) currentPlaybackSpeed);
            obj.put("crossfade", (double) crossfadeSeconds);
            obj.put("gapless", gaplessEnabled);
            return obj.toString();
        } catch (Exception e) {
            return "{\"playing\":false,\"position\":0,\"duration\":0}";
        }
    }

    // =========================================================================
    // CALLBACKS DE MEDIAPLAYER
    // =========================================================================

    @Override
    public void onPrepared(MediaPlayer mp) {
        isPrepared = true;
        currentDurationMs = mp.getDuration();
        mp.start();
        applyPlaybackSpeed(currentPlaybackSpeed);
        updateNotificationAndSession(true);
        notifyStateChanged(true);
        triggerWebEvent("onTrackPrepared");
    }

    @Override
    public void onCompletion(MediaPlayer mp) {
        updateNotificationAndSession(false);
        notifyStateChanged(false);
        if (eventListener != null) eventListener.onNextRequested();
        triggerWebEvent("onTrackEnded");
    }

    @Override
    public boolean onError(MediaPlayer mp, int what, int extra) {
        isPrepared = false;
        triggerWebEvent("onPlaybackError");
        return true;
    }

    @Override
    public void onAudioFocusChange(int focusChange) {
        switch (focusChange) {
            case AudioManager.AUDIOFOCUS_LOSS:
            case AudioManager.AUDIOFOCUS_LOSS_TRANSIENT:
                pause();
                break;
            case AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK:
                if (mediaPlayer != null && isPlaying()) {
                    mediaPlayer.setVolume(0.2f, 0.2f);
                }
                break;
            case AudioManager.AUDIOFOCUS_GAIN:
                if (mediaPlayer != null) {
                    setVolume(currentVolume);
                    resume();
                }
                break;
        }
    }

    private void notifyStateChanged(boolean playing) {
        if (eventListener != null) {
            eventListener.onPlaybackStateChanged(playing, currentTitle, currentArtist);
        }
        triggerWebEvent("onPlaybackStateChanged");
    }

    private void updateNotificationAndSession(boolean playing) {
        long stateActions = PlaybackStateCompat.ACTION_PLAY | PlaybackStateCompat.ACTION_PAUSE |
                PlaybackStateCompat.ACTION_SKIP_TO_NEXT | PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS |
                PlaybackStateCompat.ACTION_SEEK_TO;

        int state = playing ? PlaybackStateCompat.STATE_PLAYING : PlaybackStateCompat.STATE_PAUSED;
        mediaSession.setPlaybackState(new PlaybackStateCompat.Builder()
                .setActions(stateActions)
                .setState(state, getPosition(), currentPlaybackSpeed)
                .build());

        MediaMetadataCompat.Builder metaBuilder = new MediaMetadataCompat.Builder()
                .putString(MediaMetadataCompat.METADATA_KEY_TITLE, currentTitle)
                .putString(MediaMetadataCompat.METADATA_KEY_ARTIST, currentArtist)
                .putLong(MediaMetadataCompat.METADATA_KEY_DURATION, currentDurationMs);

        if (currentArtwork != null) {
            metaBuilder.putBitmap(MediaMetadataCompat.METADATA_KEY_ALBUM_ART, currentArtwork);
        }

        mediaSession.setMetadata(metaBuilder.build());

        NotificationManager nm = (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
        if (nm != null) {
            nm.notify(NOTIFICATION_ID, buildNotification(playing));
        }
    }

    private Notification buildNotification(boolean playing) {
        Intent contentIntent = new Intent(this, MainActivity.class);
        PendingIntent pContent = PendingIntent.getActivity(this, 0, contentIntent, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);

        PendingIntent pPrev = PendingIntent.getService(this, 1, new Intent(this, AudioPlaybackService.class).setAction(ACTION_PREV), PendingIntent.FLAG_IMMUTABLE);
        PendingIntent pToggle = PendingIntent.getService(this, 2, new Intent(this, AudioPlaybackService.class).setAction(ACTION_TOGGLE), PendingIntent.FLAG_IMMUTABLE);
        PendingIntent pNext = PendingIntent.getService(this, 3, new Intent(this, AudioPlaybackService.class).setAction(ACTION_NEXT), PendingIntent.FLAG_IMMUTABLE);

        NotificationCompat.Builder builder = new NotificationCompat.Builder(this, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_stat_music)
                .setContentTitle(currentTitle)
                .setContentText(currentArtist)
                .setContentIntent(pContent)
                .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
                .setOnlyAlertOnce(true)
                .setShowWhen(false)
                .addAction(R.drawable.ic_skip_previous, "Anterior", pPrev)
                .addAction(playing ? R.drawable.ic_pause : R.drawable.ic_play_arrow, playing ? "Pausa" : "Reproducir", pToggle)
                .addAction(R.drawable.ic_skip_next, "Siguiente", pNext)
                .setStyle(new androidx.media.app.NotificationCompat.MediaStyle()
                        .setMediaSession(mediaSession.getSessionToken())
                        .setShowActionsInCompactView(0, 1, 2));

        if (currentArtwork != null) {
            builder.setLargeIcon(currentArtwork);
        }

        return builder.build();
    }

    private void createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            NotificationChannel channel = new NotificationChannel(
                    CHANNEL_ID,
                    "Reproducción de Música",
                    NotificationManager.IMPORTANCE_LOW
            );
            channel.setDescription("Controles multimedia en barra de estado y pantalla de bloqueo");
            channel.setShowBadge(false);
            NotificationManager nm = getSystemService(NotificationManager.class);
            if (nm != null) nm.createNotificationChannel(channel);
        }
    }

    private void triggerWebEvent(String eventName) {
        mainHandler.post(() -> {
            try {
                MainActivity.dispatchWebEvent(eventName);
            } catch (Exception ignored) {}
        });
    }

    @Override
    public void onDestroy() {
        super.onDestroy();
        try {
            unregisterReceiver(becomingNoisyReceiver);
        } catch (Exception ignored) {}

        releaseAudioEffects();
        if (mediaPlayer != null) {
            mediaPlayer.release();
            mediaPlayer = null;
        }
        if (mediaSession != null) {
            mediaSession.release();
            mediaSession = null;
        }
    }

    @Nullable
    @Override
    public IBinder onBind(Intent intent) {
        return binder;
    }
}

package com.titan.music;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.content.res.AssetFileDescriptor;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.media.AudioManager;
import android.media.MediaMetadataRetriever;
import android.media.MediaPlayer;
import android.media.audiofx.Equalizer;
import android.net.Uri;
import android.os.Binder;
import android.os.Build;
import android.os.IBinder;
import android.support.v4.media.MediaMetadataCompat;
import android.support.v4.media.session.MediaSessionCompat;
import android.support.v4.media.session.PlaybackStateCompat;
import android.util.Base64;
import android.util.Log;

import androidx.core.app.NotificationCompat;
import androidx.core.app.NotificationManagerCompat;
import androidx.core.app.ServiceCompat;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.File;
import java.io.FileInputStream;
import java.util.Random;

/**
 * Servicio de reproducción de audio multimedia Titan Audio.
 *
 * Integra MediaSessionCompat nativo de Android, NotificationCompat.MediaStyle,
 * extracción real de carátulas embebidas ID3 y compatibilidad completa con Android 10-15.
 */
public class AudioPlaybackService extends Service {
    private static final String TAG = "TitanAudioService";
    public static final String CHANNEL_ID = "titan_audio_channel";
    public static final int NOTIFICATION_ID = 1001;

    // Acciones de control desde notificación y widgets
    public static final String ACTION_TOGGLE = "com.titan.music.ACTION_TOGGLE";
    public static final String ACTION_PLAY = "com.titan.music.ACTION_PLAY";
    public static final String ACTION_PAUSE = "com.titan.music.ACTION_PAUSE";
    public static final String ACTION_NEXT = "com.titan.music.ACTION_NEXT";
    public static final String ACTION_PREV = "com.titan.music.ACTION_PREV";
    public static final String ACTION_STOP = "com.titan.music.ACTION_STOP";

    private final IBinder binder = new LocalBinder();
    private MediaPlayer mediaPlayer;
    private MediaSessionCompat mediaSession;
    private Equalizer equalizer;

    private String currentTitle = "Sin reproducción";
    private String currentArtist = "Titan Audio";
    private String currentAlbum = "Biblioteca local";
    private String currentPath = "";
    private Bitmap currentArtwork = null;

    private boolean isPlaying = false;
    private float currentVolume = 0.85f;
    private float crossfadeSecs = 0.0f;
    private boolean gaplessEnabled = true;
    private int eqBass = 0;
    private int eqMid = 0;
    private int eqTreble = 0;

    public interface PlaybackEventListener {
        void onPlaybackStateChanged(boolean isPlaying, String title, String artist);
        void onNextRequested();
        void onPreviousRequested();
    }

    private PlaybackEventListener eventListener;

    public void setPlaybackEventListener(PlaybackEventListener listener) {
        this.eventListener = listener;
    }

    public class LocalBinder extends Binder {
        public AudioPlaybackService getService() {
            return AudioPlaybackService.this;
        }
    }

    @Override
    public void onCreate() {
        super.onCreate();
        createNotificationChannel();
        initMediaSession();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && intent.getAction() != null) {
            String action = intent.getAction();
            Log.d(TAG, "onStartCommand acción recibida: " + action);
            switch (action) {
                case ACTION_TOGGLE:
                    if (isPlaying) {
                        pause();
                    } else {
                        resume();
                    }
                    break;
                case ACTION_PLAY:
                    resume();
                    break;
                case ACTION_PAUSE:
                    pause();
                    break;
                case ACTION_NEXT:
                    if (eventListener != null) {
                        eventListener.onNextRequested();
                    }
                    break;
                case ACTION_PREV:
                    if (eventListener != null) {
                        eventListener.onPreviousRequested();
                    }
                    break;
                case ACTION_STOP:
                    stop();
                    break;
            }
        }
        return START_NOT_STICKY;
    }

    private void initMediaSession() {
        mediaSession = new MediaSessionCompat(this, "TitanAudioMediaSession");
        mediaSession.setActive(true);

        mediaSession.setCallback(new MediaSessionCompat.Callback() {
            @Override
            public void onPlay() {
                resume();
            }

            @Override
            public void onPause() {
                pause();
            }

            @Override
            public void onSkipToNext() {
                if (eventListener != null) eventListener.onNextRequested();
            }

            @Override
            public void onSkipToPrevious() {
                if (eventListener != null) eventListener.onPreviousRequested();
            }

            @Override
            public void onSeekTo(long pos) {
                seekTo((int) pos);
            }

            @Override
            public void onStop() {
                stop();
            }
        });

        updatePlaybackState(PlaybackStateCompat.STATE_NONE, 0);
    }

    private void updatePlaybackState(int state, long position) {
        if (mediaSession == null) return;

        long actions = PlaybackStateCompat.ACTION_PLAY
                | PlaybackStateCompat.ACTION_PAUSE
                | PlaybackStateCompat.ACTION_PLAY_PAUSE
                | PlaybackStateCompat.ACTION_SKIP_TO_NEXT
                | PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS
                | PlaybackStateCompat.ACTION_SEEK_TO;

        PlaybackStateCompat.Builder stateBuilder = new PlaybackStateCompat.Builder()
                .setActions(actions)
                .setState(state, position, 1.0f);

        mediaSession.setPlaybackState(stateBuilder.build());
    }

    private void updateSessionMetadata() {
        if (mediaSession == null) return;

        MediaMetadataCompat.Builder metaBuilder = new MediaMetadataCompat.Builder()
                .putString(MediaMetadataCompat.METADATA_KEY_TITLE, currentTitle)
                .putString(MediaMetadataCompat.METADATA_KEY_ARTIST, currentArtist)
                .putString(MediaMetadataCompat.METADATA_KEY_ALBUM, currentAlbum)
                .putLong(MediaMetadataCompat.METADATA_KEY_DURATION, getDuration());

        if (currentArtwork != null) {
            metaBuilder.putBitmap(MediaMetadataCompat.METADATA_KEY_ALBUM_ART, currentArtwork);
            metaBuilder.putBitmap(MediaMetadataCompat.METADATA_KEY_ART, currentArtwork);
        }

        mediaSession.setMetadata(metaBuilder.build());
    }

    private void createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            NotificationChannel channel = new NotificationChannel(
                    CHANNEL_ID,
                    "Titan Reproductor Multimedia",
                    NotificationManager.IMPORTANCE_LOW
            );
            channel.setDescription("Controles interactivos de reproducción y carátula");
            channel.setShowBadge(true);
            channel.setLockscreenVisibility(Notification.VISIBILITY_PUBLIC);

            NotificationManager manager = getSystemService(NotificationManager.class);
            if (manager != null) {
                manager.createNotificationChannel(channel);
            }
        }
    }

    private Notification buildNotification() {
        Intent contentIntent = new Intent(this, MainActivity.class);
        contentIntent.setFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP | Intent.FLAG_ACTIVITY_CLEAR_TOP);
        PendingIntent piContent = PendingIntent.getActivity(
                this, 0, contentIntent,
                PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M ? PendingIntent.FLAG_IMMUTABLE : 0)
        );

        PendingIntent piPrev = PendingIntent.getService(
                this, 1, new Intent(this, AudioPlaybackService.class).setAction(ACTION_PREV),
                PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M ? PendingIntent.FLAG_IMMUTABLE : 0)
        );

        PendingIntent piToggle = PendingIntent.getService(
                this, 2, new Intent(this, AudioPlaybackService.class).setAction(ACTION_TOGGLE),
                PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M ? PendingIntent.FLAG_IMMUTABLE : 0)
        );

        PendingIntent piNext = PendingIntent.getService(
                this, 3, new Intent(this, AudioPlaybackService.class).setAction(ACTION_NEXT),
                PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M ? PendingIntent.FLAG_IMMUTABLE : 0)
        );

        androidx.media.app.NotificationCompat.MediaStyle mediaStyle = new androidx.media.app.NotificationCompat.MediaStyle()
                .setMediaSession(mediaSession.getSessionToken())
                .setShowActionsInCompactView(0, 1, 2);

        NotificationCompat.Builder builder = new NotificationCompat.Builder(this, CHANNEL_ID)
                .setContentTitle(currentTitle)
                .setContentText(currentArtist)
                .setSubText(currentAlbum)
                .setSmallIcon(R.drawable.ic_notif_music)
                .setContentIntent(piContent)
                .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
                .setPriority(NotificationCompat.PRIORITY_HIGH)
                .setOngoing(isPlaying)
                .setShowWhen(false)
                .setStyle(mediaStyle)
                .addAction(R.drawable.ic_notif_previous, "Anterior", piPrev)
                .addAction(isPlaying ? R.drawable.ic_notif_pause : R.drawable.ic_notif_play, isPlaying ? "Pausa" : "Reproducir", piToggle)
                .addAction(R.drawable.ic_notif_next, "Siguiente", piNext);

        if (currentArtwork != null) {
            builder.setLargeIcon(currentArtwork);
        }

        return builder.build();
    }

    private void updateNotification() {
        try {
            Notification notification = buildNotification();
            NotificationManagerCompat.from(this).notify(NOTIFICATION_ID, notification);
        } catch (Exception e) {
            Log.w(TAG, "No se pudo refrescar la notificación: " + e.getMessage());
        }
    }

    /**
     * Carga y reproduce el archivo real de audio.
     */
    public synchronized boolean play(String path, String title, String artist) {
        this.currentTitle = (title != null && !title.isEmpty()) ? title : "Canción";
        this.currentArtist = (artist != null && !artist.isEmpty()) ? artist : "Titan Audio";
        this.currentPath = path != null ? path : "";

        // Extraer carátula embebida real del archivo
        extractArtwork(this.currentPath);

        Log.i(TAG, "Iniciando pista real: " + this.currentTitle + " (" + this.currentPath + ")");

        try {
            if (mediaPlayer != null) {
                try {
                    mediaPlayer.stop();
                    mediaPlayer.reset();
                    mediaPlayer.release();
                } catch (Exception ignored) {}
                mediaPlayer = null;
            }

            if (equalizer != null) {
                try {
                    equalizer.release();
                } catch (Exception ignored) {}
                equalizer = null;
            }

            mediaPlayer = new MediaPlayer();
            mediaPlayer.setAudioStreamType(AudioManager.STREAM_MUSIC);

            boolean loaded = false;

            // 1. Content URI de MediaStore (content://...)
            if (path != null && path.startsWith("content://")) {
                Uri contentUri = Uri.parse(path);
                try {
                    AssetFileDescriptor afd = getContentResolver().openAssetFileDescriptor(contentUri, "r");
                    if (afd != null) {
                        mediaPlayer.setDataSource(afd.getFileDescriptor(), afd.getStartOffset(), afd.getLength());
                        afd.close();
                        loaded = true;
                        Log.i(TAG, "Pista cargada mediante openAssetFileDescriptor");
                    }
                } catch (Exception e1) {
                    Log.w(TAG, "openAssetFileDescriptor fallo, usando context/uri", e1);
                }

                if (!loaded) {
                    try {
                        mediaPlayer.setDataSource(getApplicationContext(), contentUri);
                        loaded = true;
                        Log.i(TAG, "Pista cargada mediante setDataSource(context, uri)");
                    } catch (Exception e2) {
                        Log.e(TAG, "Error final cargando content URI: " + path, e2);
                    }
                }
            }
            // 2. Stream HTTP remoto
            else if (path != null && (path.startsWith("http://") || path.startsWith("https://"))) {
                mediaPlayer.setDataSource(path);
                loaded = true;
            }
            // 3. Ruta directa en almacenamiento (/storage/... o /sdcard/...)
            else if (path != null && !path.isEmpty()) {
                File file = new File(path);
                if (file.exists() && file.canRead()) {
                    try (FileInputStream fis = new FileInputStream(file)) {
                        mediaPlayer.setDataSource(fis.getFD());
                        loaded = true;
                        Log.i(TAG, "Pista cargada mediante FileInputStream");
                    } catch (Exception e) {
                        Log.w(TAG, "Error leyendo archivo: " + path, e);
                    }
                }
            }

            if (!loaded) {
                Log.e(TAG, "No se pudo cargar la fuente de audio real: " + path);
                return false;
            }

            mediaPlayer.setVolume(currentVolume, currentVolume);

            mediaPlayer.setOnPreparedListener(mp -> {
                try {
                    mp.start();
                    isPlaying = true;
                    applyEqualizer();

                    updateSessionMetadata();
                    updatePlaybackState(PlaybackStateCompat.STATE_PLAYING, mp.getCurrentPosition());

                    Notification notif = buildNotification();
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                        ServiceCompat.startForeground(this, NOTIFICATION_ID, notif, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK);
                    } else {
                        startForeground(NOTIFICATION_ID, notif);
                    }

                    if (eventListener != null) {
                        eventListener.onPlaybackStateChanged(true, currentTitle, currentArtist);
                    }
                    Log.i(TAG, "Reproducción en curso con notificación activa");
                } catch (Exception err) {
                    Log.e(TAG, "Error al arrancar MediaPlayer", err);
                }
            });

            mediaPlayer.setOnCompletionListener(mp -> {
                Log.i(TAG, "Canción completada");
                isPlaying = false;
                updatePlaybackState(PlaybackStateCompat.STATE_PAUSED, mp.getDuration());
                updateNotification();
                if (eventListener != null) {
                    eventListener.onNextRequested();
                }
            });

            mediaPlayer.setOnErrorListener((mp, what, extra) -> {
                Log.e(TAG, "Error en MediaPlayer: what=" + what + ", extra=" + extra);
                isPlaying = false;
                updatePlaybackState(PlaybackStateCompat.STATE_ERROR, 0);
                return false;
            });

            mediaPlayer.prepareAsync();
            return true;
        } catch (Exception e) {
            Log.e(TAG, "Excepción iniciando reproducción", e);
            isPlaying = false;
            return false;
        }
    }

    private void extractArtwork(String path) {
        currentArtwork = null;
        if (path == null || path.isEmpty()) return;

        MediaMetadataRetriever mmr = new MediaMetadataRetriever();
        try {
            if (path.startsWith("content://")) {
                mmr.setDataSource(this, Uri.parse(path));
            } else {
                File f = new File(path);
                if (f.exists()) mmr.setDataSource(path);
            }
            byte[] art = mmr.getEmbeddedPicture();
            if (art != null && art.length > 0) {
                currentArtwork = BitmapFactory.decodeByteArray(art, 0, art.length);
            }
        } catch (Exception ignored) {
        } finally {
            try { mmr.release(); } catch (Exception ignored) {}
        }
    }

    public void pause() {
        if (mediaPlayer != null && isPlaying) {
            mediaPlayer.pause();
            isPlaying = false;
            updatePlaybackState(PlaybackStateCompat.STATE_PAUSED, mediaPlayer.getCurrentPosition());
            updateNotification();
            if (eventListener != null) {
                eventListener.onPlaybackStateChanged(false, currentTitle, currentArtist);
            }
        }
    }

    public void resume() {
        if (mediaPlayer != null && !isPlaying) {
            mediaPlayer.start();
            isPlaying = true;
            updatePlaybackState(PlaybackStateCompat.STATE_PLAYING, mediaPlayer.getCurrentPosition());

            Notification notif = buildNotification();
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                ServiceCompat.startForeground(this, NOTIFICATION_ID, notif, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK);
            } else {
                startForeground(NOTIFICATION_ID, notif);
            }

            if (eventListener != null) {
                eventListener.onPlaybackStateChanged(true, currentTitle, currentArtist);
            }
        }
    }

    public void stop() {
        if (mediaPlayer != null) {
            try {
                mediaPlayer.stop();
            } catch (Exception ignored) {}
            isPlaying = false;
            updatePlaybackState(PlaybackStateCompat.STATE_STOPPED, 0);
            stopForeground(true);
            if (eventListener != null) {
                eventListener.onPlaybackStateChanged(false, currentTitle, currentArtist);
            }
        }
    }

    public void seekTo(int msec) {
        if (mediaPlayer != null) {
            mediaPlayer.seekTo(msec);
            updatePlaybackState(isPlaying ? PlaybackStateCompat.STATE_PLAYING : PlaybackStateCompat.STATE_PAUSED, msec);
        }
    }

    public boolean isPlaying() {
        return mediaPlayer != null && mediaPlayer.isPlaying();
    }

    public int getPosition() {
        if (mediaPlayer != null) {
            try {
                return mediaPlayer.getCurrentPosition();
            } catch (Exception ignored) {}
        }
        return 0;
    }

    public int getDuration() {
        if (mediaPlayer != null) {
            try {
                int dur = mediaPlayer.getDuration();
                return dur > 0 ? dur : 180000;
            } catch (Exception ignored) {}
        }
        return 180000;
    }

    public void setVolume(int percent) {
        float vol = Math.max(0.0f, Math.min(1.0f, percent / 100.0f));
        this.currentVolume = vol;
        if (mediaPlayer != null) {
            mediaPlayer.setVolume(vol, vol);
        }
    }

    public int getVolume() {
        return Math.round(currentVolume * 100);
    }

    public void setCrossfade(float seconds) {
        this.crossfadeSecs = Math.max(0.0f, Math.min(12.0f, seconds));
    }

    public float getCrossfade() {
        return this.crossfadeSecs;
    }

    public void setGapless(boolean enabled) {
        this.gaplessEnabled = enabled;
    }

    public boolean isGapless() {
        return this.gaplessEnabled;
    }

    public void setEqualizer(int bassDb, int midDb, int trebleDb) {
        this.eqBass = Math.max(-12, Math.min(12, bassDb));
        this.eqMid = Math.max(-12, Math.min(12, midDb));
        this.eqTreble = Math.max(-12, Math.min(12, trebleDb));
        applyEqualizer();
    }

    public int[] getEqualizer() {
        return new int[]{eqBass, eqMid, eqTreble};
    }

    private void applyEqualizer() {
        if (mediaPlayer == null) return;
        try {
            if (equalizer == null) {
                equalizer = new Equalizer(0, mediaPlayer.getAudioSessionId());
                equalizer.setEnabled(true);
            }
            short bands = equalizer.getNumberOfBands();
            if (bands >= 3) {
                short minLevel = equalizer.getBandLevelRange()[0];
                short maxLevel = equalizer.getBandLevelRange()[1];

                short bassMilli = (short) ((eqBass / 12.0f) * maxLevel);
                short midMilli = (short) ((eqMid / 12.0f) * maxLevel);
                short trebleMilli = (short) ((eqTreble / 12.0f) * maxLevel);

                equalizer.setBandLevel((short) 0, bassMilli);
                equalizer.setBandLevel((short) (bands / 2), midMilli);
                equalizer.setBandLevel((short) (bands - 1), trebleMilli);
            }
        } catch (Exception e) {
            Log.w(TAG, "Equalizer no disponible por hardware: " + e.getMessage());
        }
    }

    public JSONArray getAvailableDevices() {
        JSONArray arr = new JSONArray();
        try {
            JSONObject dev1 = new JSONObject();
            dev1.put("id", 1);
            dev1.put("name", "Altavoz del dispositivo");
            dev1.put("type", "speaker");
            dev1.put("active", true);
            arr.put(dev1);

            JSONObject dev2 = new JSONObject();
            dev2.put("id", 2);
            dev2.put("name", "Auriculares con cable");
            dev2.put("type", "wired");
            dev2.put("active", false);
            arr.put(dev2);

            JSONObject dev3 = new JSONObject();
            dev3.put("id", 3);
            dev3.put("name", "Audio Bluetooth");
            dev3.put("type", "bluetooth");
            dev3.put("active", false);
            arr.put(dev3);
        } catch (Exception ignored) {}
        return arr;
    }

    public float[] getSpectrumLevels() {
        float[] levels = new float[32];
        if (isPlaying) {
            Random r = new Random();
            for (int i = 0; i < 32; i++) {
                float factor = (i < 10) ? (1.0f + eqBass / 24.0f) : (i > 20 ? (1.0f + eqTreble / 24.0f) : (1.0f + eqMid / 24.0f));
                levels[i] = Math.max(0.05f, Math.min(1.0f, (r.nextFloat() * 0.7f + 0.2f) * factor));
            }
        }
        return levels;
    }

    @Override
    public IBinder onBind(Intent intent) {
        return binder;
    }

    @Override
    public void onDestroy() {
        if (mediaPlayer != null) {
            try {
                mediaPlayer.stop();
                mediaPlayer.release();
            } catch (Exception ignored) {}
            mediaPlayer = null;
        }
        if (equalizer != null) {
            try {
                equalizer.release();
            } catch (Exception ignored) {}
            equalizer = null;
        }
        if (mediaSession != null) {
            mediaSession.setActive(false);
            mediaSession.release();
            mediaSession = null;
        }
        super.onDestroy();
    }
}

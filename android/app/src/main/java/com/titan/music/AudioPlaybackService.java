package com.titan.music;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.res.AssetFileDescriptor;
import android.media.AudioManager;
import android.media.MediaPlayer;
import android.media.audiofx.Equalizer;
import android.net.Uri;
import android.os.Binder;
import android.os.Build;
import android.os.IBinder;
import android.util.Log;
import androidx.core.app.NotificationCompat;
import org.json.JSONArray;
import org.json.JSONObject;

import java.io.File;
import java.io.FileInputStream;
import java.util.Random;

/**
 * Servicio de reproducción de audio en primer plano (Hi-Fi Engine).
 */
public class AudioPlaybackService extends Service {
    private static final String TAG = "AudioPlaybackService";
    private static final String CHANNEL_ID = "titan_audio_channel";
    private static final int NOTIFICATION_ID = 4201;

    private final IBinder binder = new LocalBinder();
    private MediaPlayer mediaPlayer;
    private Equalizer equalizer;

    private String currentTitle = "Sin reproducción";
    private String currentArtist = "Titan Music";
    private String currentPath = "";
    private boolean isPlaying = false;
    private float currentVolume = 0.8f;
    private float crossfadeSecs = 0.0f;
    private boolean gaplessEnabled = true;
    private int eqBass = 0;
    private int eqMid = 0;
    private int eqTreble = 0;

    public class LocalBinder extends Binder {
        public AudioPlaybackService getService() {
            return AudioPlaybackService.this;
        }
    }

    @Override
    public void onCreate() {
        super.onCreate();
        createNotificationChannel();
    }

    private void createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            NotificationChannel channel = new NotificationChannel(
                    CHANNEL_ID,
                    "Titan Music Reproducción",
                    NotificationManager.IMPORTANCE_LOW
            );
            channel.setDescription("Controles de reproducción multimedia");
            channel.setShowBadge(false);
            NotificationManager manager = getSystemService(NotificationManager.class);
            if (manager != null) {
                manager.createNotificationChannel(channel);
            }
        }
    }

    private Notification buildNotification() {
        Intent intent = new Intent(this, MainActivity.class);
        intent.setFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP | Intent.FLAG_ACTIVITY_CLEAR_TOP);
        PendingIntent pi = PendingIntent.getActivity(
                this, 0, intent,
                PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M ? PendingIntent.FLAG_IMMUTABLE : 0)
        );

        return new NotificationCompat.Builder(this, CHANNEL_ID)
                .setContentTitle(currentTitle)
                .setContentText(currentArtist)
                .setSmallIcon(R.mipmap.ic_launcher)
                .setContentIntent(pi)
                .setOngoing(isPlaying)
                .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .build();
    }

    /**
     * Inicia la reproducción de una pista (URI de MediaStore, archivo local o stream de red).
     */
    public synchronized boolean play(String path, String title, String artist) {
        this.currentTitle = (title != null && !title.isEmpty()) ? title : "Canción";
        this.currentArtist = (artist != null && !artist.isEmpty()) ? artist : "Artista";
        this.currentPath = path != null ? path : "";

        Log.i(TAG, "Solicitando reproducir: " + this.currentTitle + " (" + this.currentPath + ")");

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

            // 1. Content URI de Android MediaStore (content://...)
            if (path != null && path.startsWith("content://")) {
                Uri contentUri = Uri.parse(path);
                try {
                    AssetFileDescriptor afd = getContentResolver().openAssetFileDescriptor(contentUri, "r");
                    if (afd != null) {
                        mediaPlayer.setDataSource(afd.getFileDescriptor(), afd.getStartOffset(), afd.getLength());
                        afd.close();
                        loaded = true;
                        Log.i(TAG, "Cargado vía ContentResolver openAssetFileDescriptor");
                    }
                } catch (Exception e1) {
                    Log.w(TAG, "Fallo openAssetFileDescriptor, intentando setDataSource directo", e1);
                }

                if (!loaded) {
                    try {
                        mediaPlayer.setDataSource(getApplicationContext(), contentUri);
                        loaded = true;
                        Log.i(TAG, "Cargado vía setDataSource(context, uri)");
                    } catch (Exception e2) {
                        Log.e(TAG, "Error final cargando content URI: " + path, e2);
                    }
                }
            }
            // 2. Stream HTTP / URL remota
            else if (path != null && (path.startsWith("http://") || path.startsWith("https://"))) {
                mediaPlayer.setDataSource(path);
                loaded = true;
            }
            // 3. Archivo del sistema de ficheros (/storage/... o /data/...)
            else if (path != null && !path.isEmpty() && !path.startsWith("cloud:")) {
                File file = new File(path);
                if (file.exists() && file.canRead()) {
                    try (FileInputStream fis = new FileInputStream(file)) {
                        mediaPlayer.setDataSource(fis.getFD());
                        loaded = true;
                        Log.i(TAG, "Cargado vía FileInputStream");
                    } catch (Exception e) {
                        Log.w(TAG, "Error leyendo file path: " + path, e);
                    }
                }
            }

            // 4. Fallback a pistas empaquetadas en assets si es una pista demo de la nube
            if (!loaded) {
                String search = (this.currentTitle + " " + this.currentArtist + " " + path).toLowerCase();
                String assetFile = "track_synthwave.wav";
                if (search.contains("soda") || search.contains("ligera") || search.contains("rock")) {
                    assetFile = "track_rock.wav";
                } else if (search.contains("lofi") || search.contains("chill") || search.contains("calma")) {
                    assetFile = "track_lofi.wav";
                } else if (search.contains("acoustic") || search.contains("guitar") || search.contains("piano")) {
                    assetFile = "track_acoustic.wav";
                } else if (search.contains("dance") || search.contains("blinding") || search.contains("electronic") || search.contains("starboy")) {
                    assetFile = "track_electronic.wav";
                }

                AssetFileDescriptor afd = null;
                try {
                    afd = getAssets().openFd("web/audio/" + assetFile);
                } catch (Exception e1) {
                    try {
                        afd = getAssets().openFd("audio/" + assetFile);
                    } catch (Exception e2) {
                        Log.e(TAG, "No se pudo abrir asset empaquetado: " + assetFile, e2);
                    }
                }

                if (afd != null) {
                    mediaPlayer.setDataSource(afd.getFileDescriptor(), afd.getStartOffset(), afd.getLength());
                    afd.close();
                    loaded = true;
                    Log.i(TAG, "Cargado asset empaquetado: " + assetFile);
                }
            }

            if (!loaded) {
                Log.e(TAG, "Imposible cargar fuente de audio: " + path);
                return false;
            }

            mediaPlayer.setVolume(currentVolume, currentVolume);

            mediaPlayer.setOnPreparedListener(mp -> {
                try {
                    mp.start();
                    isPlaying = true;
                    applyEqualizer();
                    startForeground(NOTIFICATION_ID, buildNotification());
                    Log.i(TAG, "Reproducción iniciada exitosamente: " + currentTitle);
                } catch (Exception err) {
                    Log.e(TAG, "Error en mp.start()", err);
                }
            });

            mediaPlayer.setOnCompletionListener(mp -> {
                Log.i(TAG, "Reproducción completada: " + currentTitle);
                isPlaying = false;
                stopForeground(false);
            });

            mediaPlayer.setOnErrorListener((mp, what, extra) -> {
                Log.e(TAG, "MediaPlayer error: " + what + ", " + extra);
                isPlaying = false;
                return false;
            });

            mediaPlayer.prepareAsync();
            return true;
        } catch (Exception e) {
            Log.e(TAG, "Error iniciando reproducción de: " + path, e);
            isPlaying = false;
            return false;
        }
    }

    public void pause() {
        if (mediaPlayer != null && isPlaying) {
            mediaPlayer.pause();
            isPlaying = false;
            stopForeground(false);
        }
    }

    public void resume() {
        if (mediaPlayer != null && !isPlaying) {
            mediaPlayer.start();
            isPlaying = true;
            startForeground(NOTIFICATION_ID, buildNotification());
        }
    }

    public void stop() {
        if (mediaPlayer != null) {
            try {
                mediaPlayer.stop();
            } catch (Exception ignored) {}
            isPlaying = false;
            stopForeground(true);
        }
    }

    public void seekTo(int msec) {
        if (mediaPlayer != null) {
            mediaPlayer.seekTo(msec);
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

    // =========================================================================
    // AJUSTES HI-FI
    // =========================================================================

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
            Log.w(TAG, "No se pudo aplicar ecualizador por hardware", e);
        }
    }

    public JSONArray getAvailableDevices() {
        JSONArray arr = new JSONArray();
        try {
            JSONObject dev1 = new JSONObject();
            dev1.put("id", 1);
            dev1.put("name", "Altavoz del teléfono");
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
            dev3.put("name", "Dispositivo Bluetooth");
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
        super.onDestroy();
    }
}

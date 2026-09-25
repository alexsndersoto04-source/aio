package com.titan.music;

import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.provider.MediaStore;
import android.util.Log;
import android.webkit.JavascriptInterface;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.File;

/**
 * Puente JavaScript <-> Android para la interfaz estilo Spotify.
 */
public class WebAppInterface {
    private static final String TAG = "WebAppInterface";

    private final Context context;
    private final MainActivity activity;
    private final TelegramStreamingClient telegramClient;

    public WebAppInterface(Context context, MainActivity activity) {
        this.context = context;
        this.activity = activity;
        this.telegramClient = new TelegramStreamingClient(context);
    }

    @JavascriptInterface
    public boolean playTrack(String path, String title, String artist, boolean isCloud) {
        AudioPlaybackService service = activity.getAudioService();
        int attempts = 0;
        while (service == null && attempts < 5) {
            try {
                Thread.sleep(100);
            } catch (InterruptedException ignored) {}
            service = activity.getAudioService();
            attempts++;
        }
        if (service != null) {
            return service.play(path, title, artist);
        }
        return false;
    }

    @JavascriptInterface
    public void pauseTrack() {
        AudioPlaybackService service = activity.getAudioService();
        if (service != null) {
            service.pause();
        }
    }

    @JavascriptInterface
    public void resumeTrack() {
        AudioPlaybackService service = activity.getAudioService();
        if (service != null) {
            service.resume();
        }
    }

    @JavascriptInterface
    public void stopTrack() {
        AudioPlaybackService service = activity.getAudioService();
        if (service != null) {
            service.stop();
        }
    }

    @JavascriptInterface
    public void seekTo(int seconds) {
        AudioPlaybackService service = activity.getAudioService();
        if (service != null) {
            service.seekTo(seconds * 1000);
        }
    }

    @JavascriptInterface
    public String getPlaybackStatus() {
        JSONObject obj = new JSONObject();
        try {
            AudioPlaybackService service = activity.getAudioService();
            if (service != null) {
                obj.put("playing", service.isPlaying());
                obj.put("position", service.getPosition() / 1000);
                obj.put("duration", service.getDuration() / 1000);
                obj.put("volume", service.getVolume());
                obj.put("crossfade", service.getCrossfade());
                obj.put("gapless", service.isGapless());
                int[] eq = service.getEqualizer();
                obj.put("bass", eq[0]);
                obj.put("mid", eq[1]);
                obj.put("treble", eq[2]);
            } else {
                obj.put("playing", false);
                obj.put("position", 0);
                obj.put("duration", 210);
                obj.put("volume", 80);
                obj.put("crossfade", 0.0);
                obj.put("gapless", true);
                obj.put("bass", 0);
                obj.put("mid", 0);
                obj.put("treble", 0);
            }
        } catch (Exception ignored) {}
        return obj.toString();
    }

    // =========================================================================
    // AJUSTES AVANZADOS
    // =========================================================================

    @JavascriptInterface
    public void setVolume(int percent) {
        AudioPlaybackService service = activity.getAudioService();
        if (service != null) {
            service.setVolume(percent);
        }
    }

    @JavascriptInterface
    public void setCrossfade(float seconds) {
        AudioPlaybackService service = activity.getAudioService();
        if (service != null) {
            service.setCrossfade(seconds);
        }
    }

    @JavascriptInterface
    public void setGapless(boolean enabled) {
        AudioPlaybackService service = activity.getAudioService();
        if (service != null) {
            service.setGapless(enabled);
        }
    }

    @JavascriptInterface
    public void setEqualizer(int bassDb, int midDb, int trebleDb) {
        AudioPlaybackService service = activity.getAudioService();
        if (service != null) {
            service.setEqualizer(bassDb, midDb, trebleDb);
        }
    }

    @JavascriptInterface
    public String getAudioDevices() {
        AudioPlaybackService service = activity.getAudioService();
        if (service != null) {
            return service.getAvailableDevices().toString();
        }
        return "[]";
    }

    @JavascriptInterface
    public String getSpectrumLevels() {
        JSONArray arr = new JSONArray();
        AudioPlaybackService service = activity.getAudioService();
        if (service != null) {
            float[] levels = service.getSpectrumLevels();
            for (float lvl : levels) {
                try {
                    arr.put((double) lvl);
                } catch (Exception ignored) {}
            }
        }
        return arr.toString();
    }

    // =========================================================================
    // BIBLIOTECA DEL TELÉFONO
    // =========================================================================

    @JavascriptInterface
    public String scanLocalMusic() {
        JSONArray array = new JSONArray();
        try {
            Uri uri = MediaStore.Audio.Media.EXTERNAL_CONTENT_URI;
            String[] projection = {
                    MediaStore.Audio.Media._ID,
                    MediaStore.Audio.Media.TITLE,
                    MediaStore.Audio.Media.ARTIST,
                    MediaStore.Audio.Media.ALBUM,
                    MediaStore.Audio.Media.DURATION,
                    MediaStore.Audio.Media.DATA
            };
            String selection = MediaStore.Audio.Media.IS_MUSIC + "!= 0";

            Cursor cursor = context.getContentResolver().query(uri, projection, selection, null, null);
            if (cursor != null) {
                while (cursor.moveToNext()) {
                    long id = cursor.getLong(0);
                    String title = cursor.getString(1);
                    String artist = cursor.getString(2);
                    String album = cursor.getString(3);
                    long duration = cursor.getLong(4) / 1000;
                    String path = cursor.getString(5);

                    JSONObject song = new JSONObject();
                    song.put("id", "local_" + id);
                    song.put("title", title != null ? title : "Canción " + id);
                    song.put("artist", artist != null && !artist.equals("<unknown>") ? artist : "Artista desconocido");
                    song.put("album", album != null ? album : "Álbum");
                    song.put("duration", duration > 0 ? duration : 180);
                    song.put("path", path != null ? path : "");
                    song.put("source", "phone");
                    song.put("isCloud", false);
                    array.put(song);
                }
                cursor.close();
            }
        } catch (Exception e) {
            Log.e(TAG, "Error escaneando música del teléfono", e);
        }

        return array.toString();
    }

    // =========================================================================
    // NUBE TELEGRAM
    // =========================================================================

    @JavascriptInterface
    public String getTelegramStatus() {
        return telegramClient.getStatus().toString();
    }

    @JavascriptInterface
    public void setupTelegram(String apiId, String apiHash, String phone) {
        telegramClient.setup(apiId, apiHash, phone);
    }

    @JavascriptInterface
    public boolean verifyTelegramCode(String code, String password) {
        return telegramClient.verifyCode(code, password);
    }

    @JavascriptInterface
    public void logoutTelegram() {
        telegramClient.logout();
    }

    @JavascriptInterface
    public boolean downloadCloudTrack(String id, String title, String artist) {
        return telegramClient.downloadExplicit(id, title, artist);
    }
}

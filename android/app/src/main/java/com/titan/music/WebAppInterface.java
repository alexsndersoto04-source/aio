package com.titan.music;

import android.content.ContentUris;
import android.content.Context;
import android.database.Cursor;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.media.MediaMetadataRetriever;
import android.net.Uri;
import android.provider.MediaStore;
import android.util.Base64;
import android.util.Log;
import android.webkit.JavascriptInterface;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.util.Locale;

/**
 * Interfaz de comunicación bidireccional entre la interfaz web y el sistema Android nativo.
 * Proporciona acceso a MediaStore, AudioFX (Equalizer, BassBoost, Virtualizer) y sistema de archivos.
 */
public class WebAppInterface {
    private static final String TAG = "TitanWebAppInterface";

    private final MainActivity activity;
    private final Context context;

    public WebAppInterface(MainActivity activity) {
        this.activity = activity;
        this.context = activity.getApplicationContext();
    }

    private AudioPlaybackService getService() {
        return activity.getAudioService();
    }

    // =========================================================================
    // REPRODUCCIÓN NATIVA
    // =========================================================================

    @JavascriptInterface
    public boolean playTrack(String path, String title, String artist, boolean isCloud) {
        AudioPlaybackService service = getService();
        if (service != null) {
            return service.play(path, title, artist);
        }
        Log.e(TAG, "AudioPlaybackService no disponible en playTrack");
        return false;
    }

    @JavascriptInterface
    public void pauseTrack() {
        AudioPlaybackService service = getService();
        if (service != null) {
            service.pause();
        }
    }

    @JavascriptInterface
    public void resumeTrack() {
        AudioPlaybackService service = getService();
        if (service != null) {
            service.resume();
        }
    }

    @JavascriptInterface
    public void stopTrack() {
        AudioPlaybackService service = getService();
        if (service != null) {
            service.stop();
        }
    }

    @JavascriptInterface
    public void seekTo(int milliseconds) {
        AudioPlaybackService service = getService();
        if (service != null) {
            service.seekTo(milliseconds);
        }
    }

    @JavascriptInterface
    public String getPlaybackStatus() {
        AudioPlaybackService service = getService();
        if (service != null) {
            return service.getPlaybackStatusJson();
        }
        return "{\"playing\":false,\"position\":0,\"duration\":0}";
    }

    @JavascriptInterface
    public void openAudioFilePicker() {
        activity.runOnUiThread(activity::launchAudioFilePicker);
    }

    // =========================================================================
    // EXTRACCIÓN REAL DE METADATOS Y CARÁTULAS EMBEBIDAS
    // =========================================================================

    @JavascriptInterface
    public String getEmbeddedArtwork(String pathOrUri) {
        if (pathOrUri == null || pathOrUri.isEmpty()) return "";

        MediaMetadataRetriever mmr = new MediaMetadataRetriever();
        try {
            if (pathOrUri.startsWith("content://")) {
                mmr.setDataSource(context, Uri.parse(pathOrUri));
            } else {
                File f = new File(pathOrUri.replace("file://", ""));
                if (f.exists() && f.canRead()) {
                    mmr.setDataSource(f.getAbsolutePath());
                } else {
                    return "";
                }
            }

            byte[] art = mmr.getEmbeddedPicture();
            if (art != null && art.length > 0) {
                Bitmap bitmap = BitmapFactory.decodeByteArray(art, 0, art.length);
                if (bitmap != null) {
                    int maxDimension = 320;
                    float scale = Math.min((float) maxDimension / bitmap.getWidth(), (float) maxDimension / bitmap.getHeight());
                    if (scale < 1.0f) {
                        int scaledW = Math.round(bitmap.getWidth() * scale);
                        int scaledH = Math.round(bitmap.getHeight() * scale);
                        bitmap = Bitmap.createScaledBitmap(bitmap, scaledW, scaledH, true);
                    }

                    ByteArrayOutputStream baos = new ByteArrayOutputStream();
                    bitmap.compress(Bitmap.CompressFormat.JPEG, 85, baos);
                    byte[] compressed = baos.toByteArray();
                    return "data:image/jpeg;base64," + Base64.encodeToString(compressed, Base64.NO_WRAP);
                }
            }
        } catch (Exception e) {
            Log.d(TAG, "Sin carátula embebida en: " + pathOrUri);
        } finally {
            try { mmr.release(); } catch (Exception ignored) {}
        }
        return "";
    }

    // =========================================================================
    // EFECTOS DE AUDIO Y HARDWARE (AUDIO FX)
    // =========================================================================

    @JavascriptInterface
    public void setVolume(int volume) {
        AudioPlaybackService service = getService();
        if (service != null) {
            service.setVolume(volume);
        }
    }

    @JavascriptInterface
    public void setCrossfade(float seconds) {
        AudioPlaybackService service = getService();
        if (service != null) {
            service.setCrossfade(seconds);
        }
    }

    @JavascriptInterface
    public void setGapless(boolean enabled) {
        AudioPlaybackService service = getService();
        if (service != null) {
            service.setGapless(enabled);
        }
    }

    @JavascriptInterface
    public void setEqualizer(int bass, int mid, int treble) {
        AudioPlaybackService service = getService();
        if (service != null) {
            service.setEqualizer(bass, mid, treble);
        }
    }

    @JavascriptInterface
    public void setBassBoost(int strengthPercent) {
        AudioPlaybackService service = getService();
        if (service != null) {
            service.setBassBoost(strengthPercent);
        }
    }

    @JavascriptInterface
    public void setVirtualizer(int strengthPercent) {
        AudioPlaybackService service = getService();
        if (service != null) {
            service.setVirtualizer(strengthPercent);
        }
    }

    @JavascriptInterface
    public void setPlaybackSpeed(float speed) {
        AudioPlaybackService service = getService();
        if (service != null) {
            service.setPlaybackSpeed(speed);
        }
    }

    // =========================================================================
    // BIBLIOTECA DEL TELÉFONO (MEDIASTORE REAL CON METADATOS COMPLETOS)
    // =========================================================================

    @JavascriptInterface
    public String scanLocalMusic() {
        JSONArray array = new JSONArray();

        try {
            Uri audioUri = MediaStore.Audio.Media.EXTERNAL_CONTENT_URI;
            String[] projection = {
                    MediaStore.Audio.Media._ID,
                    MediaStore.Audio.Media.TITLE,
                    MediaStore.Audio.Media.ARTIST,
                    MediaStore.Audio.Media.ALBUM,
                    MediaStore.Audio.Media.ALBUM_ID,
                    MediaStore.Audio.Media.DURATION,
                    MediaStore.Audio.Media.DATA,
                    MediaStore.Audio.Media.SIZE,
                    MediaStore.Audio.Media.YEAR,
                    MediaStore.Audio.Media.DATE_ADDED
            };

            String selection = MediaStore.Audio.Media.IS_MUSIC + " != 0 AND " + MediaStore.Audio.Media.DURATION + " >= 20000";
            String sortOrder = MediaStore.Audio.Media.TITLE + " COLLATE NOCASE ASC";

            try (Cursor cursor = context.getContentResolver().query(audioUri, projection, selection, null, sortOrder)) {
                if (cursor == null) {
                    return array.toString();
                }

                int idIdx = cursor.getColumnIndex(MediaStore.Audio.Media._ID);
                int titleIdx = cursor.getColumnIndex(MediaStore.Audio.Media.TITLE);
                int artistIdx = cursor.getColumnIndex(MediaStore.Audio.Media.ARTIST);
                int albumIdx = cursor.getColumnIndex(MediaStore.Audio.Media.ALBUM);
                int albumIdIdx = cursor.getColumnIndex(MediaStore.Audio.Media.ALBUM_ID);
                int durIdx = cursor.getColumnIndex(MediaStore.Audio.Media.DURATION);
                int dataIdx = cursor.getColumnIndex(MediaStore.Audio.Media.DATA);
                int sizeIdx = cursor.getColumnIndex(MediaStore.Audio.Media.SIZE);
                int yearIdx = cursor.getColumnIndex(MediaStore.Audio.Media.YEAR);
                int dateAddedIdx = cursor.getColumnIndex(MediaStore.Audio.Media.DATE_ADDED);

                while (cursor.moveToNext()) {
                    long id = cursor.getLong(idIdx);
                    String title = titleIdx >= 0 ? cursor.getString(titleIdx) : "Sin título";
                    String artist = artistIdx >= 0 ? cursor.getString(artistIdx) : "Artista desconocido";
                    String album = albumIdx >= 0 ? cursor.getString(albumIdx) : "Álbum desconocido";
                    long albumId = albumIdIdx >= 0 ? cursor.getLong(albumIdIdx) : -1;
                    long durSecs = durIdx >= 0 ? cursor.getLong(durIdx) / 1000 : 180;
                    String rawPath = dataIdx >= 0 ? cursor.getString(dataIdx) : "";
                    long sizeBytes = sizeIdx >= 0 ? cursor.getLong(sizeIdx) : 0;
                    int year = yearIdx >= 0 ? cursor.getInt(yearIdx) : 0;
                    long dateAdded = dateAddedIdx >= 0 ? cursor.getLong(dateAddedIdx) : 0;

                    String lowerPath = (rawPath != null ? rawPath : "").toLowerCase(Locale.ROOT);
                    String lowerTitle = (title != null ? title : "").toLowerCase(Locale.ROOT);

                    if (lowerPath.contains("whatsapp") ||
                            lowerPath.contains("telegram/telegram audio") ||
                            lowerPath.contains("/cache") ||
                            lowerPath.contains("/notifications") ||
                            lowerPath.contains("/ringtones") ||
                            lowerTitle.startsWith("ptt-") ||
                            lowerTitle.startsWith("aud-") ||
                            lowerTitle.contains("tts-") ||
                            lowerTitle.contains("inworld")) {
                        continue;
                    }

                    String folder = "Música";
                    if (rawPath != null && rawPath.contains("/")) {
                        int lastSlash = rawPath.lastIndexOf('/');
                        if (lastSlash > 0) {
                            int prevSlash = rawPath.lastIndexOf('/', lastSlash - 1);
                            if (prevSlash >= 0) {
                                folder = rawPath.substring(prevSlash + 1, lastSlash);
                            }
                        }
                    }

                    Uri itemContentUri = ContentUris.withAppendedId(MediaStore.Audio.Media.EXTERNAL_CONTENT_URI, id);
                    String albumArtUri = "";
                    if (albumId >= 0) {
                        Uri artworkUri = ContentUris.withAppendedId(Uri.parse("content://media/external/audio/albumart"), albumId);
                        albumArtUri = artworkUri.toString();
                    }

                    JSONObject song = new JSONObject();
                    song.put("id", "phone_" + id);
                    song.put("title", (title != null && !title.isEmpty()) ? title : "Pista " + id);
                    song.put("artist", (artist != null && !artist.equals("<unknown>")) ? artist : "Artista desconocido");
                    song.put("album", (album != null && !album.equals("<unknown>")) ? album : "Dispositivo");
                    song.put("duration", durSecs);
                    song.put("path", itemContentUri.toString());
                    song.put("rawPath", rawPath != null ? rawPath : "");
                    song.put("size", sizeBytes);
                    song.put("year", year);
                    song.put("folder", folder);
                    song.put("dateAdded", dateAdded);
                    song.put("artworkUri", albumArtUri);
                    song.put("source", "phone");
                    song.put("isCloud", false);
                    song.put("downloaded", true);

                    array.put(song);
                }
            }
        } catch (Exception e) {
            Log.e(TAG, "Error consultando MediaStore", e);
        }

        return array.toString();
    }
}

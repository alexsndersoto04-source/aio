package com.titan.music;

import android.Manifest;
import android.annotation.SuppressLint;
import android.content.ClipData;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.IBinder;
import android.provider.OpenableColumns;
import android.util.Log;
import android.webkit.WebChromeClient;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

import androidx.annotation.NonNull;
import androidx.appcompat.app.AppCompatActivity;
import androidx.core.app.ActivityCompat;
import androidx.core.content.ContextCompat;

import org.json.JSONArray;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.List;

public class MainActivity extends AppCompatActivity {
    private static final String TAG = "TitanMainActivity";
    private static final int PERMISSION_REQ_CODE = 101;
    private static final int FILE_PICKER_REQ_CODE = 202;

    private WebView webView;
    private AudioPlaybackService audioService;
    private boolean isBound = false;

    private final ServiceConnection serviceConnection = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName name, IBinder binder) {
            AudioPlaybackService.LocalBinder localBinder = (AudioPlaybackService.LocalBinder) binder;
            audioService = localBinder.getService();
            isBound = true;
            Log.i(TAG, "AudioPlaybackService conectado con éxito");

            audioService.setPlaybackEventListener(new AudioPlaybackService.PlaybackEventListener() {
                @Override
                public void onPlaybackStateChanged(boolean isPlaying, String title, String artist) {
                    runOnUiThread(() -> {
                        if (webView != null) {
                            String escapedTitle = (title != null ? title : "").replace("'", "\\'");
                            String escapedArtist = (artist != null ? artist : "").replace("'", "\\'");
                            webView.evaluateJavascript(
                                    String.format("if (window.onNativePlaybackStateChanged) window.onNativePlaybackStateChanged(%b, '%s', '%s');",
                                            isPlaying, escapedTitle, escapedArtist),
                                    null
                            );
                        }
                    });
                }

                @Override
                public void onNextRequested() {
                    runOnUiThread(() -> {
                        if (webView != null) {
                            webView.evaluateJavascript("if (window.playNextTrack) window.playNextTrack();", null);
                        }
                    });
                }

                @Override
                public void onPreviousRequested() {
                    runOnUiThread(() -> {
                        if (webView != null) {
                            webView.evaluateJavascript("if (window.playPreviousTrack) window.playPreviousTrack();", null);
                        }
                    });
                }
            });
        }

        @Override
        public void onServiceDisconnected(ComponentName name) {
            audioService = null;
            isBound = false;
        }
    };

    @SuppressLint("SetJavaScriptEnabled")
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        // Iniciar y conectar el servicio de audio
        Intent serviceIntent = new Intent(this, AudioPlaybackService.class);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(serviceIntent);
        } else {
            startService(serviceIntent);
        }
        bindService(serviceIntent, serviceConnection, Context.BIND_AUTO_CREATE);

        // Crear WebView de alto rendimiento a pantalla completa
        webView = new WebView(this);
        setContentView(webView);

        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setAllowFileAccess(true);
        settings.setAllowContentAccess(true);
        settings.setMediaPlaybackRequiresUserGesture(false);

        webView.setWebViewClient(new WebViewClient());
        webView.setWebChromeClient(new WebChromeClient());

        // Inyectar puente JavaScript con la app nativa
        webView.addJavascriptInterface(new WebAppInterface(this), "TitanBridge");

        // Cargar interfaz de Titan Audio
        webView.loadUrl("file:///android_asset/web/index.html");

        // Solicitar permisos necesarios
        checkAndRequestPermissions();
    }

    public AudioPlaybackService getAudioService() {
        return audioService;
    }

    /**
     * Abre el selector del sistema de Android permitiendo seleccionar una o múltiples canciones.
     */
    public void launchAudioFilePicker() {
        try {
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            intent.addCategory(Intent.CATEGORY_OPENABLE);
            intent.setType("audio/*");
            intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
            intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
            startActivityForResult(intent, FILE_PICKER_REQ_CODE);
        } catch (Exception e) {
            Log.w(TAG, "ACTION_OPEN_DOCUMENT fallo, intentando ACTION_GET_CONTENT", e);
            try {
                Intent intent = new Intent(Intent.ACTION_GET_CONTENT);
                intent.setType("audio/*");
                intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                startActivityForResult(Intent.createChooser(intent, "Selecciona canciones"), FILE_PICKER_REQ_CODE);
            } catch (Exception e2) {
                Log.e(TAG, "No se pudo abrir selector de archivos", e2);
            }
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == FILE_PICKER_REQ_CODE && resultCode == RESULT_OK && data != null) {
            List<Uri> selectedUris = new ArrayList<>();

            if (data.getClipData() != null) {
                ClipData clipData = data.getClipData();
                for (int i = 0; i < clipData.getItemCount(); i++) {
                    Uri u = clipData.getItemAt(i).getUri();
                    if (u != null) selectedUris.add(u);
                }
            } else if (data.getData() != null) {
                selectedUris.add(data.getData());
            }

            JSONArray importedTracks = new JSONArray();

            for (Uri uri : selectedUris) {
                try {
                    getContentResolver().takePersistableUriPermission(
                            uri,
                            Intent.FLAG_GRANT_READ_URI_PERMISSION
                    );
                } catch (Exception ignored) {}

                String displayName = "Pista de audio";
                try (Cursor c = getContentResolver().query(uri, null, null, null, null)) {
                    if (c != null && c.moveToFirst()) {
                        int nameIdx = c.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                        if (nameIdx >= 0) {
                            displayName = c.getString(nameIdx);
                        }
                    }
                } catch (Exception ignored) {}

                String cleanTitle = displayName.replaceAll("\\.[a-zA-Z0-9]+$", "");
                JSONObject trackObj = new JSONObject();
                try {
                    trackObj.put("id", "import_" + System.currentTimeMillis() + "_" + importedTracks.length());
                    trackObj.put("title", cleanTitle);
                    trackObj.put("artist", "Archivo local");
                    trackObj.put("album", "Dispositivo");
                    trackObj.put("duration", 180);
                    trackObj.put("path", uri.toString());
                    trackObj.put("source", "phone");
                    trackObj.put("isCloud", false);
                    importedTracks.put(trackObj);
                } catch (Exception ignored) {}
            }

            if (importedTracks.length() > 0 && webView != null) {
                webView.post(() -> {
                    String jsonString = importedTracks.toString().replace("'", "\\'");
                    webView.evaluateJavascript(
                            String.format("if (window.onLocalTracksImported) window.onLocalTracksImported('%s');", jsonString),
                            null
                    );
                });
            }
        }
    }

    private void checkAndRequestPermissions() {
        List<String> needed = new ArrayList<>();

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            // Android 13+ (API 33+)
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.READ_MEDIA_AUDIO)
                    != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.READ_MEDIA_AUDIO);
            }
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS)
                    != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.POST_NOTIFICATIONS);
            }
        } else {
            // Android 12 y anteriores
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.READ_EXTERNAL_STORAGE)
                    != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.READ_EXTERNAL_STORAGE);
            }
        }

        if (!needed.isEmpty()) {
            ActivityCompat.requestPermissions(
                    this,
                    needed.toArray(new String[0]),
                    PERMISSION_REQ_CODE
            );
        }
    }

    @Override
    public void onRequestPermissionsResult(int requestCode, @NonNull String[] permissions, @NonNull int[] grantResults) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults);
        if (requestCode == PERMISSION_REQ_CODE && webView != null) {
            webView.post(() -> webView.evaluateJavascript("if (window.onPermissionsGranted) window.onPermissionsGranted();", null));
        }
    }

    @Override
    public void onBackPressed() {
        if (webView != null) {
            webView.evaluateJavascript("if (window.handleAndroidBack) { window.handleAndroidBack(); } else { 'back'; }", value -> {
                if (value != null && value.contains("handled")) {
                    return;
                }
                super.onBackPressed();
            });
        } else {
            super.onBackPressed();
        }
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        if (isBound) {
            unbindService(serviceConnection);
            isBound = false;
        }
    }
}

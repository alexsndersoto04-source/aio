package com.titan.music;

import android.Manifest;
import android.annotation.SuppressLint;
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

/**
 * Actividad principal con WebView acelerada por hardware y puente nativo a Android MediaPlayer.
 */
public class MainActivity extends AppCompatActivity {
    private static final String TAG = "MainActivity";
    private static final int PERMISSION_REQ_CODE = 1001;
    public static final int FILE_PICKER_REQ_CODE = 2001;

    private WebView webView;
    private AudioPlaybackService audioService;
    private boolean isBound = false;

    private final ServiceConnection serviceConnection = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName name, IBinder binder) {
            AudioPlaybackService.LocalBinder b = (AudioPlaybackService.LocalBinder) binder;
            audioService = b.getService();
            isBound = true;
            Log.d(TAG, "AudioPlaybackService conectado");
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

        // Iniciar y enlazar el servicio de audio en primer plano
        Intent serviceIntent = new Intent(this, AudioPlaybackService.class);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(serviceIntent);
        } else {
            startService(serviceIntent);
        }
        bindService(serviceIntent, serviceConnection, Context.BIND_AUTO_CREATE);

        // Crear WebView a pantalla completa
        webView = new WebView(this);
        setContentView(webView);

        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setDatabaseEnabled(true);
        settings.setMediaPlaybackRequiresUserGesture(false);
        settings.setAllowFileAccess(true);
        settings.setAllowContentAccess(true);
        settings.setLoadWithOverviewMode(true);
        settings.setUseWideViewPort(true);

        // Aceleración por hardware para 60fps
        webView.setLayerType(WebView.LAYER_TYPE_HARDWARE, null);

        // Registrar puente JavaScript -> Android
        webView.addJavascriptInterface(new WebAppInterface(this, this), "TitanBridge");

        webView.setWebViewClient(new WebViewClient());
        webView.setWebChromeClient(new WebChromeClient());

        // Cargar interfaz Spotify
        webView.loadUrl("file:///android_asset/web/index.html");

        checkAndRequestPermissions();
    }

    public AudioPlaybackService getAudioService() {
        return audioService;
    }

    public void launchAudioFilePicker() {
        try {
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            intent.addCategory(Intent.CATEGORY_OPENABLE);
            intent.setType("audio/*");
            intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
            startActivityForResult(intent, FILE_PICKER_REQ_CODE);
        } catch (Exception e) {
            Log.w(TAG, "ACTION_OPEN_DOCUMENT fallo, intentando ACTION_GET_CONTENT", e);
            try {
                Intent intent = new Intent(Intent.ACTION_GET_CONTENT);
                intent.setType("audio/*");
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                startActivityForResult(Intent.createChooser(intent, "Selecciona una canción"), FILE_PICKER_REQ_CODE);
            } catch (Exception e2) {
                Log.e(TAG, "No se pudo abrir selector de archivos", e2);
            }
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == FILE_PICKER_REQ_CODE && resultCode == RESULT_OK && data != null) {
            Uri uri = data.getData();
            if (uri != null) {
                try {
                    getContentResolver().takePersistableUriPermission(
                            uri,
                            Intent.FLAG_GRANT_READ_URI_PERMISSION
                    );
                } catch (Exception ignored) {}

                String displayName = "Canción seleccionada";
                try (Cursor c = getContentResolver().query(uri, null, null, null, null)) {
                    if (c != null && c.moveToFirst()) {
                        int nameIdx = c.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                        if (nameIdx >= 0) {
                            displayName = c.getString(nameIdx);
                        }
                    }
                } catch (Exception ignored) {}

                final String finalUri = uri.toString();
                final String finalTitle = displayName.replaceAll("\\.[a-zA-Z0-9]+$", "");

                if (audioService != null) {
                    audioService.play(finalUri, finalTitle, "Almacenamiento del teléfono");
                }

                if (webView != null) {
                    webView.post(() -> {
                        String escapedUri = finalUri.replace("'", "\\'");
                        String escapedTitle = finalTitle.replace("'", "\\'");
                        webView.evaluateJavascript(
                                String.format("if (window.onLocalTrackImported) window.onLocalTrackImported('%s', '%s');", escapedUri, escapedTitle),
                                null
                        );
                    });
                }
            }
        }
    }

    private void checkAndRequestPermissions() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.READ_MEDIA_AUDIO)
                    != PackageManager.PERMISSION_GRANTED) {
                ActivityCompat.requestPermissions(
                        this,
                        new String[]{
                                Manifest.permission.READ_MEDIA_AUDIO,
                                Manifest.permission.POST_NOTIFICATIONS
                        },
                        PERMISSION_REQ_CODE
                );
            }
        } else {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.READ_EXTERNAL_STORAGE)
                    != PackageManager.PERMISSION_GRANTED) {
                ActivityCompat.requestPermissions(
                        this,
                        new String[]{
                                Manifest.permission.READ_EXTERNAL_STORAGE,
                                Manifest.permission.WRITE_EXTERNAL_STORAGE
                        },
                        PERMISSION_REQ_CODE
                );
            }
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
                    // Manejado por la web
                } else if (webView.canGoBack()) {
                    webView.goBack();
                } else {
                    super.onBackPressed();
                }
            });
        } else {
            super.onBackPressed();
        }
    }

    @Override
    protected void onDestroy() {
        if (isBound) {
            unbindService(serviceConnection);
            isBound = false;
        }
        if (webView != null) {
            webView.destroy();
        }
        super.onDestroy();
    }
}

package com.titan.music;

import android.Manifest;
import android.annotation.SuppressLint;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Bundle;
import android.os.IBinder;
import android.webkit.WebChromeClient;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import androidx.annotation.NonNull;
import androidx.appcompat.app.AppCompatActivity;
import androidx.core.app.ActivityCompat;
import androidx.core.content.ContextCompat;

/**
 * Actividad principal con interfaz web WebView acelerada por hardware (UI Spotify).
 */
public class MainActivity extends AppCompatActivity {
    private static final int PERMISSION_REQ_CODE = 1001;

    private WebView webView;
    private AudioPlaybackService audioService;
    private boolean isBound = false;

    private final ServiceConnection serviceConnection = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName name, IBinder binder) {
            AudioPlaybackService.LocalBinder b = (AudioPlaybackService.LocalBinder) binder;
            audioService = b.getService();
            isBound = true;
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

        // Iniciar y enlazar el servicio de audio
        Intent serviceIntent = new Intent(this, AudioPlaybackService.class);
        startService(serviceIntent);
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
            // Notificar a la web que refresque la biblioteca si se otorgaron permisos
            webView.post(() -> webView.evaluateJavascript("if (window.onPermissionsGranted) window.onPermissionsGranted();", null));
        }
    }

    @Override
    public void onBackPressed() {
        if (webView != null) {
            // Si la web tiene un modal abierto (reproductor grande o ajustes), se cierra primero
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

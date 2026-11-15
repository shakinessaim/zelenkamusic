package com.zelenka.music;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.ServiceInfo;
import android.net.Uri;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.PowerManager;

/** A user-started data-sync service keeps long downloads alive across screen changes. */
public final class ContentDownloadService extends Service {
    static final String CANCEL = "com.zelenka.music.CANCEL_DOWNLOAD";
    private static final String CHANNEL = "library-download";
    private static final int NOTIFICATION = 2;
    private static volatile boolean running;
    private final RepositoryDownload downloader = new RepositoryDownload();
    private final Handler main = new Handler(Looper.getMainLooper());
    private SharedPreferences preferences;
    private Thread worker;
    private boolean finishing;
    private PowerManager.WakeLock wakeLock;
    static boolean isRunning() { return running; }
    @Override public void onCreate() {
        super.onCreate(); preferences = getSharedPreferences("zelenka", MODE_PRIVATE);
        ((NotificationManager) getSystemService(NOTIFICATION_SERVICE)).createNotificationChannel(
                new NotificationChannel(CHANNEL, "Загрузка музыки и текстов", NotificationManager.IMPORTANCE_LOW));
    }
    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && CANCEL.equals(intent.getAction())) { downloader.cancel(); if (worker == null) stopSelf(); return START_NOT_STICKY; }
        if (worker != null) return START_NOT_STICKY;
        String songs = preferences.getString("music_tree", ""), lyrics = preferences.getString("lyrics_tree", "");
        if (songs.isEmpty() || lyrics.isEmpty()) { stopSelf(); return START_NOT_STICKY; }
        if (Build.VERSION.SDK_INT >= 29) startForeground(NOTIFICATION, notification("Подготовка загрузки…"), ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC);
        else startForeground(NOTIFICATION, notification("Подготовка загрузки…"));
        running = true;
        wakeLock = ((PowerManager) getSystemService(POWER_SERVICE)).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "ZelenkaMusic:ContentDownload");
        wakeLock.acquire(6L * 60 * 60 * 1000);
        worker = new Thread(() -> {
            String result;
            try { result = downloader.download(getContentResolver(), Uri.parse(songs), Uri.parse(lyrics), this::report); }
            catch (Exception error) { result = "Загрузка не завершена: " + (error.getMessage() == null ? "Проверьте интернет и доступ к папкам." : error.getMessage()); }
            final String message = result;
            preferences.edit().putString("download_status", message).putBoolean("download_rescan", true).putString("download_music_tree", songs).apply();
            main.post(() -> { finishing = true; running = false; stopForeground(true); stopSelf(); });
        }, "zelenka-content-download");
        worker.start();
        return START_NOT_STICKY;
    }
    private void report(String status) {
        preferences.edit().putString("download_status", status).apply();
        ((NotificationManager) getSystemService(NOTIFICATION_SERVICE)).notify(NOTIFICATION, notification(status));
    }
    private Notification notification(String status) {
        PendingIntent open = PendingIntent.getActivity(this, 20, new Intent(this, MainActivity.class), PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        PendingIntent cancel = PendingIntent.getService(this, 21, new Intent(this, ContentDownloadService.class).setAction(CANCEL), PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        return new Notification.Builder(this, CHANNEL).setSmallIcon(R.drawable.ic_notification)
                .setContentTitle("Загрузка музыки и текстов").setContentText(status).setStyle(new Notification.BigTextStyle().bigText(status))
                .setContentIntent(open).setOnlyAlertOnce(true).setOngoing(true).setProgress(0, 0, true)
                .addAction(new Notification.Action.Builder(R.drawable.ic_close, "Отмена", cancel).build()).build();
    }
    @Override public IBinder onBind(Intent intent) { return null; }
    @Override public void onTimeout(int startId, int fgsType) { downloader.cancel(); stopSelf(); }
    @Override public void onDestroy() {
        downloader.cancel(); if (worker != null) worker.interrupt(); running = false;
        if (wakeLock != null && wakeLock.isHeld()) wakeLock.release();
        if (!finishing) preferences.edit().putString("download_status", "Загрузка прервана. Повторите загрузку; готовые файлы сохранены.").putBoolean("download_rescan", true).apply();
        super.onDestroy();
    }
}

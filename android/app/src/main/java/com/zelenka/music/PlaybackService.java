package com.zelenka.music;

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
import android.media.AudioFocusRequest;
import android.media.AudioManager;
import android.media.MediaMetadata;
import android.media.MediaPlayer;
import android.media.session.MediaSession;
import android.media.session.PlaybackState;
import android.net.Uri;
import android.os.Binder;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.PowerManager;
import android.util.LruCache;

import org.json.JSONArray;

import java.util.ArrayList;
import java.util.List;

public final class PlaybackService extends Service {
    public interface Listener { void changed(); }
    public final class LocalBinder extends Binder {
        public PlaybackService service() { return PlaybackService.this; }
    }
    private static final String CHANNEL = "playback";
    private static final String PREFIX = "com.zelenka.music.";
    private static final int NOTIFICATION = 1;
    private final LocalBinder binder = new LocalBinder();
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final ArrayList<Listener> listeners = new ArrayList<>();
    private final PlaybackQueue queue = new PlaybackQueue();
    private LibraryRepository library;
    private MediaPlayer player;
    private MediaSession session;
    private AudioManager audioManager;
    private AudioFocusRequest focusRequest;
    private boolean prepared, playWhenReady, resumeOnFocus, foreground;
    private boolean stopped, noisyRegistered;
    private int pendingSeek, consecutiveErrors;
    private float volume = 0.75f, duck = 1f;
    private String notice = "";
    private long lastSave;
    private final LruCache<String, Bitmap> artworkCache = new LruCache<String, Bitmap>(8 * 1024) {
        @Override protected int sizeOf(String key, Bitmap value) { return Math.max(1, value.getByteCount() / 1024); }
    };

    private final Runnable ticker = new Runnable() {
        @Override public void run() {
            if (System.currentTimeMillis() - lastSave > 5000) save();
            fireChanged();
            handler.postDelayed(this, 500);
        }
    };
    private final BroadcastReceiver noisy = new BroadcastReceiver() {
        @Override public void onReceive(Context context, Intent intent) { pause(false); }
    };

    @Override public void onCreate() {
        super.onCreate();
        library = new LibraryRepository(this);
        volume = library.preferences().getFloat("volume", 0.75f);
        queue.setRepeat(library.preferences().getInt("repeat", PlaybackQueue.REPEAT_ALL));
        audioManager = (AudioManager) getSystemService(AUDIO_SERVICE);
        AudioAttributes attributes = new AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA)
                .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build();
        focusRequest = new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                .setAudioAttributes(attributes).setOnAudioFocusChangeListener(this::focusChanged, handler).build();
        NotificationManager notifications = (NotificationManager) getSystemService(NOTIFICATION_SERVICE);
        notifications.createNotificationChannel(new NotificationChannel(CHANNEL, "Воспроизведение музыки",
                NotificationManager.IMPORTANCE_LOW));
        session = new MediaSession(this, "ZelenkaMusic");
        session.setFlags(MediaSession.FLAG_HANDLES_MEDIA_BUTTONS | MediaSession.FLAG_HANDLES_TRANSPORT_CONTROLS);
        session.setCallback(new MediaSession.Callback() {
            @Override public void onPlay() { resume(); }
            @Override public void onPause() { pause(false); }
            @Override public void onStop() { stopPlayback(); }
            @Override public void onSkipToNext() { next(false); }
            @Override public void onSkipToPrevious() { previous(); }
            @Override public void onSeekTo(long position) { seek((int) position); }
        }, handler);
        session.setActive(true);
        if (Build.VERSION.SDK_INT >= 33)
            registerReceiver(noisy, new IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY), RECEIVER_NOT_EXPORTED);
        else registerReceiver(noisy, new IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY));
        noisyRegistered = true;
        restore();
        handler.post(ticker);
    }

    @Override public IBinder onBind(Intent intent) { return binder; }
    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null) {
            String action = intent.getAction();
            if ((PREFIX + "PLAY").equals(action)) resume();
            else if ((PREFIX + "PAUSE").equals(action)) pause(false);
            else if ((PREFIX + "NEXT").equals(action)) next(false);
            else if ((PREFIX + "PREVIOUS").equals(action)) previous();
            else if ((PREFIX + "STOP").equals(action)) stopPlayback();
        }
        return START_NOT_STICKY;
    }

    public LibraryRepository library() { return library; }
    public PlaybackQueue queue() { return queue; }
    public Track current() { return library.find(queue.current()); }
    public boolean isPlaying() { return prepared && player != null && player.isPlaying(); }
    public boolean isPreparing() { return player != null && !prepared; }
    public int position() { return prepared && player != null ? player.getCurrentPosition() : pendingSeek; }
    public int duration() { return prepared && player != null ? player.getDuration() :
            current() == null ? 0 : (int) Math.min(Integer.MAX_VALUE, current().duration); }
    public float volume() { return volume; }
    public String notice() { return notice; }
    public void setNotice(String value) { notice = value == null ? "" : value; fireChanged(); }
    public void addListener(Listener listener) { if (!listeners.contains(listener)) listeners.add(listener); }
    public void removeListener(Listener listener) { listeners.remove(listener); }
    private void fireChanged() { for (Listener listener : new ArrayList<>(listeners)) listener.changed(); }

    public Bitmap artwork(Track track) {
        String key = track == null || track.artwork.isEmpty() ? "logo" : track.artwork;
        Bitmap bitmap = artworkCache.get(key);
        if (bitmap == null) {
            if (!key.equals("logo")) bitmap = BitmapFactory.decodeFile(key);
            if (bitmap == null) {
                BitmapFactory.Options options = new BitmapFactory.Options();
                options.inSampleSize = 1;
                bitmap = BitmapFactory.decodeResource(getResources(), R.drawable.logo, options);
            }
            if (bitmap != null) artworkCache.put(key, bitmap);
        }
        return bitmap;
    }
    public void libraryChanged() {
        artworkCache.evictAll();
        if (queue.current() != null && current() == null) {
            // Keep the saved URI so a temporarily disconnected folder can be restored.
            pause(false);
            releasePlayer();
            notice = "Текущая композиция отсутствует в выбранной папке.";
        }
        publish();
    }

    public void scanMusic(Uri tree, LibraryRepository.ScanCallback callback) {
        // Retain the service during a scan even if the activity rotates or is closed.
        startService(new Intent(this, PlaybackService.class).setAction(PREFIX + "KEEP"));
        library.scan(tree, (error, skipped) -> {
            libraryChanged();
            setNotice(error == null ? "Медиатека обновлена: " + library.tracks().size()
                    + " композиций" + (skipped == 0 ? "" : ". Не прочитано файлов: " + skipped) : error);
            callback.complete(error, skipped);
            if (!foreground && !isPlaying()) stopSelf();
        });
    }

    public void play(List<Track> tracks, String first, boolean shuffle, boolean continuous) {
        if (tracks.isEmpty()) { setNotice("Добавьте MP3 или FLAC в выбранную папку."); return; }
        stopped = false;
        consecutiveErrors = 0;
        queue.set(LibraryRepository.uris(tracks), first, shuffle, continuous);
        loadCurrent(true, 0);
    }

    public void toggle() {
        if (isPreparing()) {
            if (playWhenReady) pause(false);
            else { playWhenReady = true; promote(); publish(); }
        }
        else if (isPlaying()) pause(false);
        else resume();
    }

    private void promote() {
        stopped = false;
        if (!foreground) startForegroundService(new Intent(this, PlaybackService.class).setAction(PREFIX + "KEEP"));
        startForeground(NOTIFICATION, notification());
        foreground = true;
    }

    public void resume() {
        stopped = false;
        if (queue.current() == null) { play(library.tracks(), null, true, true); return; }
        if (player == null) { loadCurrent(true, pendingSeek); return; }
        if (!prepared) { playWhenReady = true; promote(); return; }
        try {
            // Android 15 grants focus only to a visible app or a foreground service.
            promote();
            if (audioManager.requestAudioFocus(focusRequest) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
                notice = "Звук сейчас занят другим приложением. Попробуйте ещё раз.";
                demote();
                publish();
                return;
            }
            resumeOnFocus = false;
            duck = 1f;
            player.setVolume(volume, volume);
            if (duration() > 0 && position() >= duration() - 150) player.seekTo(0);
            player.start();
            notice = "";
            publish();
        } catch (Exception error) { playbackError(); }
    }

    public void pause(boolean keepForeground) {
        playWhenReady = false;
        if (prepared && player != null && player.isPlaying()) player.pause();
        if (!keepForeground) {
            resumeOnFocus = false;
            audioManager.abandonAudioFocusRequest(focusRequest);
            demote();
        }
        save();
        publish();
    }

    private void demote() {
        if (foreground) { stopForeground(STOP_FOREGROUND_DETACH); foreground = false; }
    }

    public void next(boolean completion) {
        if (queue.next(completion) == null) { pause(false); return; }
        loadCurrent(true, 0);
    }
    public void previous() {
        if (position() > 3000) { seek(0); return; }
        queue.previous();
        if (queue.current() != null) loadCurrent(true, 0);
    }
    public void seek(int position) {
        pendingSeek = Math.max(0, Math.min(position, duration()));
        if (prepared && player != null) player.seekTo(pendingSeek);
        save();
        publish();
    }
    public void setVolume(float value) {
        volume = Math.max(0, Math.min(1, value));
        if (player != null) player.setVolume(volume * duck, volume * duck);
        library.preferences().edit().putFloat("volume", volume).apply();
        fireChanged();
    }
    public void toggleShuffle() {
        queue.setShuffle(!queue.isShuffle());
        save(); publish();
    }
    public void cycleRepeat() {
        queue.setRepeat((queue.repeat() + 1) % 3);
        save(); publish();
    }

    private void loadCurrent(boolean autoplay, int position) {
        releasePlayer();
        Track track = current();
        pendingSeek = Math.max(0, position);
        playWhenReady = autoplay;
        if (track == null) { playbackError(); return; }
        try {
            if (autoplay) promote();
            MediaPlayer loading = new MediaPlayer();
            player = loading;
            loading.setAudioAttributes(new AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA)
                    .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build());
            loading.setWakeMode(this, PowerManager.PARTIAL_WAKE_LOCK);
            loading.setDataSource(this, Uri.parse(track.uri));
            loading.setVolume(volume, volume);
            loading.setOnPreparedListener(ready -> {
                if (player != ready) return;
                prepared = true;
                consecutiveErrors = 0;
                if (pendingSeek > 0) ready.seekTo(Math.min(pendingSeek, ready.getDuration()));
                if (playWhenReady) resume();
                else publish();
            });
            loading.setOnCompletionListener(done -> { if (player == done) next(true); });
            loading.setOnSeekCompleteListener(done -> { if (player == done) { save(); publish(); } });
            loading.setOnErrorListener((failed, what, extra) -> {
                if (player == failed) playbackError();
                return true;
            });
            loading.prepareAsync();
            publish();
        } catch (Exception error) { playbackError(); }
    }

    private void playbackError() {
        boolean skip = playWhenReady || foreground;
        releasePlayer();
        notice = "Не удалось открыть композицию. Проверьте файл и доступ к папке.";
        consecutiveErrors++;
        if (skip && consecutiveErrors < queue.source().size()) {
            handler.post(() -> next(false));
        } else {
            playWhenReady = false;
            audioManager.abandonAudioFocusRequest(focusRequest);
            demote();
        }
        publish();
    }

    private void focusChanged(int change) {
        if (change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK) {
            duck = 0.2f;
            if (player != null) player.setVolume(volume * duck, volume * duck);
        } else if (change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT) {
            resumeOnFocus = isPlaying();
            pause(true);
        } else if (change == AudioManager.AUDIOFOCUS_LOSS) pause(false);
        else if (change == AudioManager.AUDIOFOCUS_GAIN) {
            duck = 1f;
            if (player != null) player.setVolume(volume, volume);
            if (resumeOnFocus) resume();
        }
    }

    private void releasePlayer() {
        prepared = false;
        if (player != null) { player.release(); player = null; }
    }

    private void publish() {
        Track track = current();
        if (track != null) session.setMetadata(new MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, track.title)
                .putString(MediaMetadata.METADATA_KEY_ARTIST, track.artist)
                .putString(MediaMetadata.METADATA_KEY_ALBUM, track.album)
                .putLong(MediaMetadata.METADATA_KEY_DURATION, duration())
                .putBitmap(MediaMetadata.METADATA_KEY_ALBUM_ART, artwork(track)).build());
        int state = track == null ? PlaybackState.STATE_NONE : isPreparing() ? PlaybackState.STATE_BUFFERING
                : isPlaying() ? PlaybackState.STATE_PLAYING : PlaybackState.STATE_PAUSED;
        session.setPlaybackState(new PlaybackState.Builder().setActions(PlaybackState.ACTION_PLAY
                | PlaybackState.ACTION_PAUSE | PlaybackState.ACTION_PLAY_PAUSE | PlaybackState.ACTION_STOP
                | PlaybackState.ACTION_SKIP_TO_NEXT | PlaybackState.ACTION_SKIP_TO_PREVIOUS
                | PlaybackState.ACTION_SEEK_TO).setState(state, position(), isPlaying() ? 1f : 0f).build());
        if (!stopped && (foreground || track != null)) {
            NotificationManager notifications = (NotificationManager) getSystemService(NOTIFICATION_SERVICE);
            notifications.notify(NOTIFICATION, notification());
        }
        fireChanged();
    }

    private PendingIntent action(String action, int request) {
        return PendingIntent.getService(this, request, new Intent(this, PlaybackService.class)
                .setAction(PREFIX + action), PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
    }
    private Notification notification() {
        Track track = current();
        PendingIntent open = PendingIntent.getActivity(this, 0, new Intent(this, MainActivity.class),
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        return new Notification.Builder(this, CHANNEL).setSmallIcon(R.drawable.ic_notification)
                .setContentTitle(track == null ? "ZelenkaMusic" : track.title)
                .setContentText(track == null ? "Локальная музыка" : track.artist)
                .setLargeIcon(artwork(track)).setContentIntent(open).setColor(0xff7bce9a)
                .setVisibility(Notification.VISIBILITY_PUBLIC).setOnlyAlertOnce(true)
                .setOngoing(isPlaying()).setCategory(Notification.CATEGORY_TRANSPORT)
                .addAction(new Notification.Action.Builder(R.drawable.ic_previous, "Предыдущая", action("PREVIOUS", 1)).build())
                .addAction(new Notification.Action.Builder(isPlaying() ? R.drawable.ic_pause : R.drawable.ic_play,
                        isPlaying() ? "Пауза" : "Слушать", action(isPlaying() ? "PAUSE" : "PLAY", 2)).build())
                .addAction(new Notification.Action.Builder(R.drawable.ic_next, "Следующая", action("NEXT", 3)).build())
                .addAction(new Notification.Action.Builder(R.drawable.ic_close, "Остановить", action("STOP", 4)).build())
                .setDeleteIntent(action("STOP", 4))
                .setStyle(new Notification.MediaStyle().setMediaSession(session.getSessionToken())
                        .setShowActionsInCompactView(0, 1, 2)).build();
    }

    private void save() {
        lastSave = System.currentTimeMillis();
        JSONArray uris = new JSONArray(queue.source());
        library.preferences().edit().putString("queue", uris.toString())
                .putString("current_uri", queue.current()).putInt("position", position())
                .putBoolean("shuffle", queue.isShuffle()).putBoolean("continuous", queue.isContinuous())
                .putInt("repeat", queue.repeat()).apply();
    }
    private void restore() {
        try {
            JSONArray json = new JSONArray(library.preferences().getString("queue", "[]"));
            List<String> uris = new ArrayList<>();
            for (int i = 0; i < json.length(); i++) uris.add(json.getString(i));
            queue.set(uris, library.preferences().getString("current_uri", null),
                    library.preferences().getBoolean("shuffle", true),
                    library.preferences().getBoolean("continuous", false));
            pendingSeek = library.preferences().getInt("position", 0);
            if (current() != null) loadCurrent(false, pendingSeek);
        } catch (Exception error) { notice = "Выберите композицию, чтобы начать воспроизведение."; }
    }
    public void stopPlayback() {
        pause(false);
        stopped = true;
        stopForeground(STOP_FOREGROUND_REMOVE);
        ((NotificationManager) getSystemService(NOTIFICATION_SERVICE)).cancel(NOTIFICATION);
        stopSelf();
    }

    @Override public void onDestroy() {
        handler.removeCallbacksAndMessages(null);
        save();
        releasePlayer();
        audioManager.abandonAudioFocusRequest(focusRequest);
        session.release();
        if (noisyRegistered) unregisterReceiver(noisy);
        library.close();
        artworkCache.evictAll();
        ((NotificationManager) getSystemService(NOTIFICATION_SERVICE)).cancel(NOTIFICATION);
        super.onDestroy();
    }
}

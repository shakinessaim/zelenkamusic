package com.zelenka.music;

import android.Manifest;
import android.app.Activity;
import android.app.AlertDialog;
import android.content.ComponentName;
import android.content.Intent;
import android.content.ServiceConnection;
import android.content.pm.PackageManager;
import android.content.res.ColorStateList;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.IBinder;
import android.provider.DocumentsContract;
import android.text.Editable;
import android.text.TextWatcher;
import android.text.TextUtils;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.WindowManager;
import android.widget.BaseAdapter;
import android.widget.Button;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.ImageButton;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.ListView;
import android.widget.ScrollView;
import android.widget.SeekBar;
import android.widget.Switch;
import android.widget.TextView;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;
import java.util.Map;

public final class MainActivity extends Activity implements PlaybackService.Listener {
    private static final int BACKGROUND = 0xff101512, SURFACE = 0xff1c251f;
    private static final int GREEN = 0xff7bce9a, TEXT = 0xfff1f5f2, MUTED = 0xffa4b0a8;
    private static final int PICK_FOLDER = 20, NOTIFICATIONS = 21;
    private PlaybackService service;
    private boolean bound, started, seeking;
    private int page, libraryTab;
    private String detailKey, detailPlaylist, search = "", displayedArtwork;
    private LinearLayout root, pageLayout;
    private FrameLayout content;
    private SnowView snow;
    private TextView summary, notice, playerTitle, playerArtist, elapsed, homeTitle, homeArtist;
    private ImageView playerArt, homeArt;
    private ImageButton playButton;
    private Button shuffleButton, repeatButton, homePlay, refreshButton;
    private SeekBar progress;
    private final Button[] navigation = new Button[3];

    private final ServiceConnection connection = new ServiceConnection() {
        @Override public void onServiceConnected(ComponentName name, IBinder binder) {
            service = ((PlaybackService.LocalBinder) binder).service();
            service.addListener(MainActivity.this);
            buildPage();
            changed();
            if (service.library().loadError() != null) service.setNotice(service.library().loadError());
        }
        @Override public void onServiceDisconnected(ComponentName name) { service = null; }
    };

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        if (Build.VERSION.SDK_INT >= 30) getWindow().setDecorFitsSystemWindows(false);
        else getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        if (state != null) {
            page = state.getInt("page"); libraryTab = state.getInt("tab");
            detailKey = state.getString("album"); detailPlaylist = state.getString("playlist");
            search = state.getString("search", "");
        }
        buildShell();
    }
    @Override protected void onStart() {
        super.onStart();
        started = true;
        bound = bindService(new Intent(this, PlaybackService.class), connection, BIND_AUTO_CREATE);
    }
    @Override protected void onStop() {
        started = false;
        snow.setEnabledSnow(false);
        if (service != null) service.removeListener(this);
        if (bound) { unbindService(connection); bound = false; }
        service = null;
        super.onStop();
    }
    @Override protected void onSaveInstanceState(Bundle state) {
        state.putInt("page", page); state.putInt("tab", libraryTab);
        state.putString("album", detailKey); state.putString("playlist", detailPlaylist);
        state.putString("search", search);
        super.onSaveInstanceState(state);
    }

    private int dp(float value) { return Math.round(value * getResources().getDisplayMetrics().density); }
    private LinearLayout column() {
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        return layout;
    }
    private LinearLayout row() {
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.HORIZONTAL);
        layout.setGravity(Gravity.CENTER_VERTICAL);
        return layout;
    }
    private TextView text(String value, int size, int color) {
        TextView view = new TextView(this);
        view.setText(value); view.setTextSize(size); view.setTextColor(color);
        view.setPadding(dp(2), dp(4), dp(2), dp(4));
        return view;
    }
    private GradientDrawable background(int color, int radius) {
        GradientDrawable drawable = new GradientDrawable();
        drawable.setColor(color); drawable.setCornerRadius(dp(radius));
        return drawable;
    }
    private Button button(String label, boolean primary, Runnable action) {
        Button button = new Button(this);
        button.setText(label); button.setAllCaps(false); button.setTextSize(14);
        button.setTextColor(primary ? BACKGROUND : TEXT);
        button.setBackgroundTintList(ColorStateList.valueOf(primary ? GREEN : SURFACE));
        button.setMinHeight(dp(48)); button.setMinimumHeight(dp(48));
        button.setPadding(dp(10), 0, dp(10), 0);
        button.setOnClickListener(view -> { if (service != null) action.run(); });
        return button;
    }
    private ImageButton icon(int resource, String label, Runnable action) {
        ImageButton button = new ImageButton(this);
        button.setImageResource(resource); button.setContentDescription(label);
        button.setBackground(background(SURFACE, 24));
        button.setPadding(dp(12), dp(12), dp(12), dp(12));
        button.setOnClickListener(view -> { if (service != null) action.run(); });
        return button;
    }
    private void weighted(LinearLayout row, View view) {
        row.addView(view, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1));
    }

    private void buildShell() {
        root = column();
        root.setBackgroundColor(BACKGROUND);
        root.setPadding(dp(16), dp(8), dp(16), 0);
        root.setOnApplyWindowInsetsListener((view, insets) -> {
            int left, top, right, bottom;
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets bars = insets.getInsets(WindowInsets.Type.systemBars()
                        | WindowInsets.Type.displayCutout() | WindowInsets.Type.ime());
                left = bars.left; top = bars.top; right = bars.right; bottom = bars.bottom;
            } else {
                left = insets.getSystemWindowInsetLeft(); top = insets.getSystemWindowInsetTop();
                right = insets.getSystemWindowInsetRight(); bottom = insets.getSystemWindowInsetBottom();
            }
            view.setPadding(dp(16) + left, dp(8) + top, dp(16) + right, bottom);
            return Build.VERSION.SDK_INT >= 30 ? WindowInsets.CONSUMED : insets.consumeSystemWindowInsets();
        });
        LinearLayout header = row();
        ImageView logo = new ImageView(this);
        logo.setImageResource(R.drawable.logo); logo.setScaleType(ImageView.ScaleType.CENTER_CROP);
        logo.setBackground(background(SURFACE, 12)); logo.setClipToOutline(true);
        header.addView(logo, new LinearLayout.LayoutParams(dp(38), dp(38)));
        TextView name = text("ZelenkaMusic", 23, TEXT);
        name.setPadding(dp(12), 0, 0, 0); name.setTypeface(null, Typeface.BOLD);
        weighted(header, name);
        root.addView(header, new LinearLayout.LayoutParams(-1, dp(56)));
        summary = text("Подключение…", 12, GREEN);
        root.addView(summary);
        content = new FrameLayout(this);
        root.addView(content, new LinearLayout.LayoutParams(-1, 0, 1));
        notice = text("", 12, GREEN);
        notice.setMaxLines(3); notice.setPadding(dp(10), dp(6), dp(10), dp(6));
        notice.setBackground(background(SURFACE, 10)); notice.setVisibility(View.GONE);
        notice.setContentDescription("Сообщение. Нажмите, чтобы закрыть.");
        notice.setOnClickListener(view -> { if (service != null) service.setNotice(""); });
        root.addView(notice);
        buildPlayer();
        LinearLayout nav = row();
        String[] titles = {"Главная", "Медиатека", "Настройки"};
        for (int i = 0; i < titles.length; i++) {
            final int selected = i;
            navigation[i] = button(titles[i], false, () -> {
                page = selected; detailKey = null; detailPlaylist = null; search = "";
                buildPage(); changed();
            });
            weighted(nav, navigation[i]);
        }
        root.addView(nav);
        snow = new SnowView(this);
        setContentView(root);
        root.requestApplyInsets();
    }

    private void buildPlayer() {
        LinearLayout player = column();
        player.setPadding(dp(10), dp(7), dp(10), dp(2));
        player.setBackground(background(SURFACE, 16));
        LinearLayout description = row();
        playerArt = new ImageView(this);
        playerArt.setScaleType(ImageView.ScaleType.CENTER_CROP);
        playerArt.setBackground(background(BACKGROUND, 8)); playerArt.setClipToOutline(true);
        description.addView(playerArt, new LinearLayout.LayoutParams(dp(42), dp(42)));
        LinearLayout labels = column(); labels.setPadding(dp(10), 0, 0, 0);
        playerTitle = text("Ничего не играет", 14, TEXT);
        playerTitle.setSingleLine(true); playerTitle.setEllipsize(TextUtils.TruncateAt.END);
        playerArtist = text("Выберите папку с музыкой", 11, MUTED);
        playerArtist.setSingleLine(true); playerArtist.setEllipsize(TextUtils.TruncateAt.END);
        labels.addView(playerTitle); labels.addView(playerArtist);
        weighted(description, labels);
        player.addView(description);
        LinearLayout timeline = row();
        progress = new SeekBar(this); progress.setMax(1000);
        progress.setProgressTintList(ColorStateList.valueOf(GREEN));
        progress.setThumbTintList(ColorStateList.valueOf(GREEN));
        progress.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onStartTrackingTouch(SeekBar view) { seeking = true; }
            @Override public void onProgressChanged(SeekBar view, int value, boolean fromUser) {
                if (fromUser && service != null)
                    elapsed.setText(time((int) (service.duration() * (value / 1000f))) + " / " + time(service.duration()));
            }
            @Override public void onStopTrackingTouch(SeekBar view) {
                seeking = false;
                if (service != null) service.seek((int) (service.duration() * (view.getProgress() / 1000f)));
            }
        });
        timeline.addView(progress, new LinearLayout.LayoutParams(0, dp(36), 1));
        elapsed = text("0:00 / 0:00", 10, MUTED);
        timeline.addView(elapsed);
        player.addView(timeline);
        LinearLayout controls = row();
        shuffleButton = button("Случайно", false, () -> service.toggleShuffle());
        shuffleButton.setTextSize(11); weighted(controls, shuffleButton);
        controls.addView(icon(R.drawable.ic_previous, "Предыдущая композиция", () -> service.previous()),
                new LinearLayout.LayoutParams(dp(48), dp(48)));
        playButton = icon(R.drawable.ic_play, "Слушать", () -> { requestNotifications(); service.toggle(); });
        playButton.setBackground(background(0xff285c42, 24));
        controls.addView(playButton, new LinearLayout.LayoutParams(dp(48), dp(48)));
        controls.addView(icon(R.drawable.ic_next, "Следующая композиция", () -> service.next(false)),
                new LinearLayout.LayoutParams(dp(48), dp(48)));
        repeatButton = button("Повтор", false, () -> service.cycleRepeat());
        repeatButton.setTextSize(11); weighted(controls, repeatButton);
        player.addView(controls);
        root.addView(player);
    }

    private void buildPage() {
        if (service == null) return;
        content.removeAllViews();
        homeTitle = null; homeArtist = null; homeArt = null; homePlay = null; refreshButton = null;
        pageLayout = column();
        pageLayout.setPadding(0, dp(8), 0, dp(8));
        content.addView(pageLayout, new FrameLayout.LayoutParams(-1, -1));
        if (page == 0) buildHome();
        else if (page == 1) buildLibrary();
        else buildSettings();
        content.addView(snow, new FrameLayout.LayoutParams(-1, -1));
        for (int i = 0; i < navigation.length; i++) {
            navigation[i].setTextColor(i == page ? GREEN : MUTED);
            navigation[i].setSelected(i == page);
        }
        snow.setEnabledSnow(service.library().preferences().getBoolean("snow", false));
        displayedArtwork = null;
    }

    private void buildHome() {
        ScrollView scroll = new ScrollView(this);
        LinearLayout home = column(); home.setGravity(Gravity.CENTER_HORIZONTAL);
        TextView heading = text("Ваша музыка. Ваш ритм.", 24, TEXT);
        heading.setTypeface(null, Typeface.BOLD); heading.setGravity(Gravity.CENTER);
        home.addView(heading);
        TextView subtitle = text("Любимые альбомы всегда с вами", 13, MUTED);
        subtitle.setGravity(Gravity.CENTER); home.addView(subtitle);
        homeArt = new ImageView(this);
        homeArt.setScaleType(ImageView.ScaleType.CENTER_CROP);
        homeArt.setBackground(background(SURFACE, 24)); homeArt.setClipToOutline(true);
        int size = Math.min(280, Math.max(140, (int) (getResources().getDisplayMetrics().widthPixels
                / getResources().getDisplayMetrics().density) - 64));
        LinearLayout.LayoutParams artParams = new LinearLayout.LayoutParams(dp(size), dp(size));
        artParams.topMargin = dp(16); artParams.bottomMargin = dp(12); home.addView(homeArt, artParams);
        homeTitle = text("", 21, TEXT); homeTitle.setGravity(Gravity.CENTER);
        homeTitle.setMaxLines(3); homeTitle.setEllipsize(TextUtils.TruncateAt.END); home.addView(homeTitle);
        homeArtist = text("", 13, MUTED); homeArtist.setGravity(Gravity.CENTER); home.addView(homeArtist);
        homePlay = button("Слушать", true, () -> { requestNotifications(); service.toggle(); });
        home.addView(homePlay, new LinearLayout.LayoutParams(-1, dp(56)));
        home.addView(button("Случайная подборка", false, () -> {
            requestNotifications(); service.play(service.library().tracks(), null, true, true);
        }), new LinearLayout.LayoutParams(-1, dp(52)));
        home.addView(button(service.library().tracks().isEmpty() ? "Выбрать папку с музыкой" : "Открыть медиатеку",
                false, () -> {
                    if (service.library().tracks().isEmpty()) pickFolder();
                    else { page = 1; buildPage(); changed(); }
                }), new LinearLayout.LayoutParams(-1, dp(52)));
        scroll.addView(home); pageLayout.addView(scroll, new LinearLayout.LayoutParams(-1, -1));
    }

    private void buildLibrary() {
        if (detailKey != null || detailPlaylist != null) { buildDetail(); return; }
        LinearLayout tools = row();
        weighted(tools, button("Папка с музыкой", false, this::pickFolder));
        refreshButton = button("Обновить", false, this::rescan);
        weighted(tools, refreshButton); pageLayout.addView(tools);
        LinearLayout tabs = row();
        Button albums = button("Альбомы", libraryTab == 0, () -> { libraryTab = 0; search = ""; buildPage(); changed(); });
        Button playlists = button("Плейлисты", libraryTab == 1, () -> { libraryTab = 1; search = ""; buildPage(); changed(); });
        weighted(tabs, albums); weighted(tabs, playlists); pageLayout.addView(tabs);
        if (libraryTab == 1) pageLayout.addView(button("+ Создать плейлист", true, () -> editPlaylist(null, null)));
        EditText filter = new EditText(this);
        filter.setSingleLine(true); filter.setTextColor(TEXT); filter.setHintTextColor(MUTED);
        filter.setHint(libraryTab == 0 ? "Найти альбом или исполнителя" : "Найти плейлист");
        filter.setText(search); filter.setTextSize(14);
        pageLayout.addView(filter);
        ListView list = list();
        ArrayList<CollectionRow> all = new ArrayList<>();
        if (libraryTab == 0) {
            for (Map.Entry<String, ArrayList<Track>> entry : service.library().albums().entrySet()) {
                Track first = entry.getValue().get(0);
                all.add(new CollectionRow(entry.getKey(), first.album,
                        first.artist + " · " + entry.getValue().size() + " композиций", first));
            }
        } else {
            JSONObject saved = service.library().playlists();
            ArrayList<String> names = new ArrayList<>();
            Iterator<String> keys = saved.keys(); while (keys.hasNext()) names.add(keys.next());
            Collections.sort(names, String.CASE_INSENSITIVE_ORDER);
            for (String name : names) {
                List<Track> tracks = service.library().playlistTracks(name);
                int total = saved.optJSONArray(name) == null ? 0 : saved.optJSONArray(name).length();
                all.add(new CollectionRow(name, name, tracks.size() + " из " + total + " композиций доступны",
                        tracks.isEmpty() ? null : tracks.get(0)));
            }
        }
        CollectionAdapter adapter = new CollectionAdapter(all);
        list.setAdapter(adapter);
        list.setOnItemClickListener((parent, view, index, id) -> {
            CollectionRow selected = adapter.rows.get(index);
            if (libraryTab == 0) detailKey = selected.key; else detailPlaylist = selected.key;
            buildPage(); changed();
        });
        filter.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence value, int start, int count, int after) { }
            @Override public void onTextChanged(CharSequence value, int start, int before, int count) {
                search = value.toString(); adapter.filter(search);
            }
            @Override public void afterTextChanged(Editable value) { }
        });
        adapter.filter(search);
        if (all.isEmpty()) {
            TextView empty = text(libraryTab == 0 ? "Выберите папку с MP3 и FLAC. Подпапки альбомов будут найдены автоматически."
                    : "Создайте плейлист, затем добавьте композиции кнопкой + в альбоме.", 15, MUTED);
            empty.setPadding(dp(8), dp(20), dp(8), dp(20)); pageLayout.addView(empty);
        }
        pageLayout.addView(list, new LinearLayout.LayoutParams(-1, 0, 1));
    }

    private void buildDetail() {
        boolean playlist = detailPlaylist != null;
        List<Track> tracks = playlist ? service.library().playlistTracks(detailPlaylist)
                : service.library().albums().get(detailKey);
        if (tracks == null) tracks = new ArrayList<>();
        final List<Track> collection = tracks;
        pageLayout.addView(button("‹ Медиатека", false, () -> {
            detailKey = null; detailPlaylist = null; buildPage(); changed();
        }));
        String title = playlist ? detailPlaylist : tracks.isEmpty() ? "Альбом недоступен" : tracks.get(0).album;
        TextView heading = text(title, 22, TEXT); heading.setMaxLines(2);
        heading.setEllipsize(TextUtils.TruncateAt.END); pageLayout.addView(heading);
        if (playlist) {
            LinearLayout tools = row();
            weighted(tools, button("Переименовать", false, () -> editPlaylist(detailPlaylist, null)));
            weighted(tools, button("Удалить", false, this::deletePlaylist)); pageLayout.addView(tools);
        }
        LinearLayout actions = row();
        weighted(actions, button("Слушать", true, () -> {
            requestNotifications(); service.play(collection, null, false, false);
        }));
        weighted(actions, button("Перемешать", false, () -> {
            requestNotifications(); service.play(collection, null, true, false);
        })); pageLayout.addView(actions);
        if (collection.isEmpty()) pageLayout.addView(text("Нет доступных композиций. Добавьте музыку или обновите папку.", 14, MUTED));
        ListView list = list();
        list.setAdapter(new TrackAdapter(collection, playlist));
        list.setOnItemClickListener((parent, view, index, id) -> {
            requestNotifications(); service.play(collection, collection.get(index).uri, false, false);
        });
        pageLayout.addView(list, new LinearLayout.LayoutParams(-1, 0, 1));
    }

    private void buildSettings() {
        ScrollView scroll = new ScrollView(this);
        LinearLayout settings = column();
        settings.addView(text("Настройки", 26, TEXT));
        Switch snowfall = new Switch(this);
        snowfall.setText("Снегопад"); snowfall.setTextColor(TEXT); snowfall.setTextSize(16);
        snowfall.setPadding(dp(8), dp(12), dp(8), dp(12)); snowfall.setMinHeight(dp(56));
        snowfall.setChecked(service.library().preferences().getBoolean("snow", false));
        snowfall.setOnCheckedChangeListener((view, checked) -> {
            service.library().preferences().edit().putBoolean("snow", checked).apply();
            snow.setEnabledSnow(checked);
        }); settings.addView(snowfall);
        settings.addView(text("Громкость проигрывателя", 16, TEXT));
        SeekBar volume = new SeekBar(this); volume.setMax(100); volume.setProgress(Math.round(service.volume() * 100));
        volume.setProgressTintList(ColorStateList.valueOf(GREEN)); volume.setThumbTintList(ColorStateList.valueOf(GREEN));
        volume.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar view, int value, boolean fromUser) {
                if (fromUser && service != null) service.setVolume(value / 100f);
            }
            @Override public void onStartTrackingTouch(SeekBar view) { }
            @Override public void onStopTrackingTouch(SeekBar view) { }
        }); settings.addView(volume, new LinearLayout.LayoutParams(-1, dp(48)));
        settings.addView(text("Музыка на устройстве", 18, TEXT));
        String tree = service.library().preferences().getString("music_tree", "");
        String location = tree.isEmpty() ? "Папка ещё не выбрана" : DocumentsContract.getTreeDocumentId(Uri.parse(tree));
        settings.addView(text(location, 13, MUTED));
        settings.addView(button("Выбрать папку с музыкой", true, this::pickFolder));
        refreshButton = button("Обновить медиатеку", false, this::rescan); settings.addView(refreshButton);
        settings.addView(text("Скопируйте альбомы на телефон и выберите их общую папку. Поддерживаются MP3 и FLAC, "
                + "встроенные обложки и cover.jpg / cover.png / cover.webp. Музыкальные файлы остаются в выбранной папке.", 14, MUTED));
        settings.addView(text("ZelenkaMusic 0.1.2 · Android", 15, GREEN));
        settings.addView(text("Работает без интернета. Плейлисты и настройки сохраняются на этом устройстве. "
                + "После перезапуска композиция восстанавливается на паузе.", 13, MUTED));
        scroll.addView(settings); pageLayout.addView(scroll, new LinearLayout.LayoutParams(-1, -1));
    }

    private ListView list() {
        ListView list = new ListView(this);
        list.setDividerHeight(dp(6)); list.setDivider(new android.graphics.drawable.ColorDrawable(BACKGROUND));
        list.setClipToPadding(false); list.setPadding(0, dp(8), 0, dp(8));
        return list;
    }

    private static final class CollectionRow {
        final String key, title, subtitle;
        final Track artwork;
        CollectionRow(String key, String title, String subtitle, Track artwork) {
            this.key = key; this.title = title; this.subtitle = subtitle; this.artwork = artwork;
        }
    }
    private static final class RowViews {
        final LinearLayout root;
        final ImageView art;
        final TextView title, subtitle;
        final Button action;
        RowViews(LinearLayout root, ImageView art, TextView title, TextView subtitle, Button action) {
            this.root = root; this.art = art; this.title = title; this.subtitle = subtitle; this.action = action;
        }
    }
    private RowViews makeRow(boolean withAction) {
        LinearLayout layout = row(); layout.setPadding(dp(8), dp(8), dp(8), dp(8));
        layout.setBackground(background(SURFACE, 14)); layout.setMinimumHeight(dp(76));
        ImageView art = new ImageView(this); art.setScaleType(ImageView.ScaleType.CENTER_CROP);
        art.setBackground(background(BACKGROUND, 8)); art.setClipToOutline(true);
        layout.addView(art, new LinearLayout.LayoutParams(dp(56), dp(56)));
        LinearLayout labels = column(); labels.setPadding(dp(12), 0, dp(4), 0);
        TextView title = text("", 15, TEXT); title.setSingleLine(true); title.setEllipsize(TextUtils.TruncateAt.END);
        TextView subtitle = text("", 12, MUTED); subtitle.setMaxLines(2); subtitle.setEllipsize(TextUtils.TruncateAt.END);
        labels.addView(title); labels.addView(subtitle); weighted(layout, labels);
        Button action = null;
        if (withAction) {
            action = button("+", false, () -> {}); action.setTextSize(20); action.setFocusable(false);
            layout.addView(action, new LinearLayout.LayoutParams(dp(48), dp(48)));
        }
        RowViews views = new RowViews(layout, art, title, subtitle, action); layout.setTag(views);
        return views;
    }
    private final class CollectionAdapter extends BaseAdapter {
        final ArrayList<CollectionRow> all, rows = new ArrayList<>();
        CollectionAdapter(ArrayList<CollectionRow> all) { this.all = all; rows.addAll(all); }
        void filter(String query) {
            rows.clear(); String lower = query.toLowerCase(Locale.ROOT).trim();
            for (CollectionRow row : all) if ((row.title + " " + row.subtitle).toLowerCase(Locale.ROOT).contains(lower)) rows.add(row);
            notifyDataSetChanged();
        }
        @Override public int getCount() { return rows.size(); }
        @Override public Object getItem(int index) { return rows.get(index); }
        @Override public long getItemId(int index) { return index; }
        @Override public View getView(int index, View recycled, ViewGroup parent) {
            RowViews view = recycled == null ? makeRow(false) : (RowViews) recycled.getTag();
            CollectionRow row = rows.get(index);
            view.title.setText(row.title); view.subtitle.setText(row.subtitle);
            if (service != null) view.art.setImageBitmap(service.artwork(row.artwork));
            return view.root;
        }
    }
    private final class TrackAdapter extends BaseAdapter {
        final List<Track> tracks;
        final boolean playlist;
        TrackAdapter(List<Track> tracks, boolean playlist) { this.tracks = tracks; this.playlist = playlist; }
        @Override public int getCount() { return tracks.size(); }
        @Override public Object getItem(int index) { return tracks.get(index); }
        @Override public long getItemId(int index) { return index; }
        @Override public View getView(int index, View recycled, ViewGroup parent) {
            RowViews view = recycled == null ? makeRow(true) : (RowViews) recycled.getTag();
            Track track = tracks.get(index);
            view.title.setText(track.title); view.subtitle.setText(track.artist + " · " + time(track.duration));
            if (service != null) view.art.setImageBitmap(service.artwork(track));
            view.action.setText(playlist ? "−" : "+");
            view.action.setContentDescription(playlist ? "Убрать из плейлиста" : "Добавить в плейлист");
            view.action.setOnClickListener(button -> {
                if (service == null) return;
                if (playlist) removeFromPlaylist(track); else choosePlaylist(track);
            });
            return view.root;
        }
    }

    private void pickFolder() {
        if (service.library().isScanning()) { service.setNotice("Дождитесь завершения поиска музыки."); return; }
        Intent pick = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        pick.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION
                | Intent.FLAG_GRANT_PREFIX_URI_PERMISSION);
        try { startActivityForResult(pick, PICK_FOLDER); }
        catch (Exception error) { service.setNotice("На устройстве недоступен выбор папок."); }
    }
    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != PICK_FOLDER || result != RESULT_OK || data == null || data.getData() == null) return;
        Uri tree = data.getData();
        try {
            getContentResolver().takePersistableUriPermission(tree, data.getFlags() & Intent.FLAG_GRANT_READ_URI_PERMISSION);
            // onActivityResult can precede reconnection to the service after the system picker.
            pendingFolder = tree;
            if (service != null) scanFolder(tree);
        } catch (Exception error) {
            if (service != null) service.setNotice("Не удалось сохранить доступ. Выберите папку в памяти телефона.");
        }
    }
    private Uri pendingFolder;
    private void rescan() {
        String saved = service.library().preferences().getString("music_tree", "");
        if (saved.isEmpty()) pickFolder(); else scanFolder(Uri.parse(saved));
    }
    private void scanFolder(Uri tree) {
        if (service == null || service.library().isScanning()) return;
        pendingFolder = null;
        PlaybackService connected = service;
        connected.setNotice("");
        connected.scanMusic(tree, (error, skipped) -> {
            if (started && service == connected && !isDestroyed()) { buildPage(); changed(); }
        });
        changed();
    }

    private void requestNotifications() {
        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
                != PackageManager.PERMISSION_GRANTED
                && !service.library().preferences().getBoolean("notifications_requested", false)) {
            service.library().preferences().edit().putBoolean("notifications_requested", true).apply();
            requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS}, NOTIFICATIONS);
        }
    }

    private void editPlaylist(String original, Track add) {
        EditText input = new EditText(this); input.setSingleLine(true); input.setMaxLines(1);
        input.setHint("Название плейлиста"); input.setText(original == null ? "" : original);
        input.setSelectAllOnFocus(true); input.setPadding(dp(20), dp(12), dp(20), dp(12));
        AlertDialog dialog = new AlertDialog.Builder(this).setTitle(original == null ? "Создать плейлист" : "Переименовать плейлист")
                .setView(input).setNegativeButton("Отмена", null).setPositiveButton("Сохранить", null).create();
        dialog.setOnShowListener(ignored -> dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(view -> {
            if (service == null) return;
            String name = input.getText().toString().trim();
            JSONObject playlists = service.library().playlists();
            if (name.isEmpty() || name.length() > 80) { input.setError("Введите название от 1 до 80 символов"); return; }
            if (!name.equals(original) && playlists.has(name)) { input.setError("Такой плейлист уже существует"); return; }
            try {
                JSONArray uris = original == null ? new JSONArray() : playlists.optJSONArray(original);
                if (uris == null) uris = new JSONArray();
                if (original != null && !original.equals(name)) playlists.remove(original);
                if (add != null) uris.put(add.uri);
                playlists.put(name, uris); service.library().savePlaylists(playlists);
                if (original != null) detailPlaylist = name;
                dialog.dismiss(); buildPage(); changed();
                if (add != null) service.setNotice("Добавлено в «" + name + "»");
            } catch (JSONException error) { input.setError("Не удалось сохранить плейлист"); }
        })); dialog.show();
    }
    private void choosePlaylist(Track track) {
        JSONObject playlists = service.library().playlists();
        ArrayList<String> names = new ArrayList<>();
        Iterator<String> keys = playlists.keys(); while (keys.hasNext()) names.add(keys.next());
        Collections.sort(names, String.CASE_INSENSITIVE_ORDER);
        names.add("+ Создать плейлист");
        new AlertDialog.Builder(this).setTitle("Добавить в плейлист")
                .setItems(names.toArray(new String[0]), (dialog, index) -> {
                    if (service == null) return;
                    if (index == names.size() - 1) { editPlaylist(null, track); return; }
                    String name = names.get(index);
                    JSONObject current = service.library().playlists();
                    JSONArray uris = current.optJSONArray(name); if (uris == null) uris = new JSONArray();
                    for (int i = 0; i < uris.length(); i++) if (track.uri.equals(uris.optString(i))) {
                        service.setNotice("Композиция уже в этом плейлисте."); return;
                    }
                    try {
                        uris.put(track.uri); current.put(name, uris); service.library().savePlaylists(current);
                        service.setNotice("Добавлено в «" + name + "»");
                    } catch (JSONException error) { service.setNotice("Не удалось сохранить плейлист."); }
                }).setNegativeButton("Отмена", null).show();
    }
    private void removeFromPlaylist(Track track) {
        JSONObject playlists = service.library().playlists();
        JSONArray current = playlists.optJSONArray(detailPlaylist), updated = new JSONArray();
        if (current != null) for (int i = 0; i < current.length(); i++)
            if (!track.uri.equals(current.optString(i))) updated.put(current.optString(i));
        try {
            playlists.put(detailPlaylist, updated); service.library().savePlaylists(playlists);
            buildPage(); changed();
        } catch (JSONException error) { service.setNotice("Не удалось сохранить плейлист."); }
    }
    private void deletePlaylist() {
        final String name = detailPlaylist;
        new AlertDialog.Builder(this).setTitle("Удалить «" + name + "»?")
                .setMessage("Музыкальные файлы останутся в папке.").setNegativeButton("Отмена", null)
                .setPositiveButton("Удалить", (dialog, which) -> {
                    if (service == null) return;
                    JSONObject playlists = service.library().playlists(); playlists.remove(name);
                    service.library().savePlaylists(playlists); detailPlaylist = null; buildPage(); changed();
                }).show();
    }

    @Override public void changed() {
        if (service == null || !started) return;
        if (pendingFolder != null) { scanFolder(pendingFolder); return; }
        Track track = service.current();
        String title = track == null ? "Ничего не играет" : track.title;
        String artist = track == null ? "Выберите папку с музыкой" : track.artist;
        playerTitle.setText(title); playerArtist.setText(artist);
        if (homeTitle != null) homeTitle.setText(title);
        if (homeArtist != null) homeArtist.setText(track == null ? "MP3 и FLAC · без интернета" : track.artist + " · " + track.album);
        if (homePlay != null) homePlay.setText(service.isPreparing() ? "Подготовка…" : service.isPlaying() ? "Пауза" : "Слушать");
        playButton.setImageResource(service.isPlaying() ? R.drawable.ic_pause : R.drawable.ic_play);
        playButton.setContentDescription(service.isPlaying() ? "Пауза" : "Слушать");
        shuffleButton.setTextColor(service.queue().isShuffle() ? GREEN : MUTED);
        shuffleButton.setContentDescription(service.queue().isShuffle() ? "Случайный порядок включён" : "Включить случайный порядок");
        int repeat = service.queue().repeat();
        repeatButton.setText(repeat == PlaybackQueue.REPEAT_ONE ? "Повтор 1" : "Повтор");
        repeatButton.setTextColor(repeat == PlaybackQueue.REPEAT_OFF ? MUTED : GREEN);
        repeatButton.setContentDescription(repeat == PlaybackQueue.REPEAT_OFF ? "Повтор выключен"
                : repeat == PlaybackQueue.REPEAT_ALL ? "Повтор всей подборки" : "Повтор одной композиции");
        progress.setEnabled(track != null && !service.isPreparing());
        if (!seeking) {
            progress.setProgress(service.duration() <= 0 ? 0 : (int) (service.position() * 1000L / service.duration()));
            elapsed.setText(time(service.position()) + " / " + time(service.duration()));
        }
        String art = track == null ? "logo" : track.uri + "|" + track.artwork;
        if (!art.equals(displayedArtwork)) {
            playerArt.setImageBitmap(service.artwork(track));
            if (homeArt != null) homeArt.setImageBitmap(service.artwork(track));
            displayedArtwork = art;
        }
        boolean scanning = service.library().isScanning();
        summary.setText(scanning ? "Поиск музыки…" : service.library().trackCount() + " композиций · "
                + service.library().albumCount() + " альбомов");
        if (refreshButton != null) { refreshButton.setEnabled(!scanning); refreshButton.setText(scanning ? "Поиск…" : "Обновить"); }
        notice.setText(service.notice()); notice.setVisibility(service.notice().isEmpty() ? View.GONE : View.VISIBLE);
    }

    private static String time(long millis) {
        long seconds = Math.max(0, millis / 1000);
        return String.format(Locale.ROOT, "%d:%02d", seconds / 60, seconds % 60);
    }
    @Override public void onBackPressed() {
        if (page == 1 && (detailKey != null || detailPlaylist != null)) {
            detailKey = null; detailPlaylist = null; buildPage(); changed();
        } else if (page != 0) { page = 0; buildPage(); changed(); }
        else super.onBackPressed();
    }
}

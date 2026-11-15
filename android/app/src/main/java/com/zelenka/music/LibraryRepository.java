package com.zelenka.music;

import android.content.Context;
import android.content.SharedPreferences;
import android.database.Cursor;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.media.MediaMetadataRetriever;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.provider.DocumentsContract;
import android.util.AtomicFile;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Only scans a user-granted document tree. Music files are never modified. */
public final class LibraryRepository {
    public interface ScanCallback { void complete(String error, int skipped); }
    private final Context context;
    private final SharedPreferences prefs;
    private final AtomicFile libraryFile;
    private final ExecutorService scanner = Executors.newSingleThreadExecutor();
    private final Handler main = new Handler(Looper.getMainLooper());
    private final ArrayList<Track> tracks = new ArrayList<>();
    private final LinkedHashMap<String, Track> tracksByUri = new LinkedHashMap<>();
    private final LinkedHashMap<String, ArrayList<Track>> albums = new LinkedHashMap<>();
    private boolean scanning;
    private String loadError;

    public LibraryRepository(Context context) {
        this.context = context.getApplicationContext();
        prefs = context.getSharedPreferences("zelenka", Context.MODE_PRIVATE);
        libraryFile = new AtomicFile(new File(context.getFilesDir(), "library.json"));
        load();
    }

    public SharedPreferences preferences() { return prefs; }
    public List<Track> tracks() { return new ArrayList<>(tracks); }
    public int trackCount() { return tracks.size(); }
    public int albumCount() { return albums.size(); }
    public Map<String, ArrayList<Track>> albums() { return new LinkedHashMap<>(albums); }
    public boolean isScanning() { return scanning; }
    public String loadError() { return loadError; }
    public Track find(String uri) {
        return tracksByUri.get(uri);
    }

    private void load() {
        if (!libraryFile.getBaseFile().exists()) return;
        try (FileInputStream input = libraryFile.openRead()) {
            JSONArray json = new JSONArray(new String(readAll(input), StandardCharsets.UTF_8));
            for (int i = 0; i < json.length(); i++) tracks.add(Track.fromJson(json.getJSONObject(i)));
            indexAlbums();
        } catch (Exception error) {
            tracks.clear();
            loadError = "Не удалось прочитать медиатеку. Обновите папку с музыкой.";
        }
    }

    private void indexAlbums() {
        albums.clear();
        tracksByUri.clear();
        Collections.sort(tracks, (a, b) -> {
            int albumOrder = a.album.compareToIgnoreCase(b.album);
            if (albumOrder != 0) return albumOrder;
            int folderOrder = a.albumKey.compareTo(b.albumKey);
            if (folderOrder != 0) return folderOrder;
            int numberOrder = Integer.compare(a.number, b.number);
            return numberOrder == 0 ? a.title.compareToIgnoreCase(b.title) : numberOrder;
        });
        for (Track track : tracks) {
            tracksByUri.put(track.uri, track);
            if (!albums.containsKey(track.albumKey)) albums.put(track.albumKey, new ArrayList<>());
            albums.get(track.albumKey).add(track);
        }
    }

    public void scan(Uri tree, ScanCallback callback) {
        if (scanning) return;
        scanning = true;
        scanner.execute(() -> {
            ArrayList<Track> found = new ArrayList<>();
            int[] skipped = {0};
            String failure = null;
            try {
                String rootId = DocumentsContract.getTreeDocumentId(tree);
                String rootName = documentName(DocumentsContract.buildDocumentUriUsingTree(tree, rootId));
                walk(tree, rootId, rootName, "", found, skipped, new HashSet<>(), 0);
                JSONArray json = new JSONArray();
                for (Track track : found) json.put(track.toJson());
                FileOutputStream output = null;
                try {
                    output = libraryFile.startWrite();
                    output.write(json.toString().getBytes(StandardCharsets.UTF_8));
                    libraryFile.finishWrite(output);
                } catch (Exception error) {
                    if (output != null) libraryFile.failWrite(output);
                    throw error;
                }
            } catch (Exception error) {
                failure = "Не удалось прочитать папку. Выберите доступную папку с музыкой ещё раз.";
            }
            final String result = failure;
            main.post(() -> {
                scanning = false;
                if (result == null) {
                    tracks.clear();
                    tracks.addAll(found);
                    indexAlbums();
                    loadError = null;
                    prefs.edit().putString("music_tree", tree.toString()).apply();
                }
                callback.complete(result, skipped[0]);
            });
        });
    }

    private String documentName(Uri document) {
        try (Cursor cursor = context.getContentResolver().query(document,
                new String[]{DocumentsContract.Document.COLUMN_DISPLAY_NAME}, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) return cursor.getString(0);
        }
        return "Музыка";
    }

    private void walk(Uri tree, String parentId, String folder, String relativeFolder, List<Track> found, int[] skipped,
                      Set<String> visited, int depth) throws Exception {
        if (depth > 64 || !visited.add(parentId)) return;
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parentId);
        String[] columns = {DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE};
        ArrayList<String[]> files = new ArrayList<>();
        Uri cover = null;
        int coverPriority = Integer.MAX_VALUE;
        try (Cursor cursor = context.getContentResolver().query(children, columns, null, null, null)) {
            if (cursor == null) throw new IllegalStateException("Document provider unavailable");
            while (cursor.moveToNext()) {
                String id = cursor.getString(0), name = cursor.getString(1), mime = cursor.getString(2);
                files.add(new String[]{id, name, mime});
                String lower = name.toLowerCase(Locale.ROOT);
                if (lower.matches(".*\\.(jpe?g|png|webp)$")) {
                    int priority = lower.startsWith("cover.") ? 0 : lower.startsWith("folder.") ? 1 : 2;
                    if (priority < coverPriority) {
                        cover = DocumentsContract.buildDocumentUriUsingTree(tree, id);
                        coverPriority = priority;
                    }
                }
            }
        }
        String folderArt = cover == null ? "" : cacheArtwork(cover);
        for (String[] file : files) {
            if (DocumentsContract.Document.MIME_TYPE_DIR.equals(file[2])) {
                // A failed subfolder aborts the scan, preserving the last complete library.
                walk(tree, file[0], file[1], relativeFolder.isEmpty() ? file[1] : relativeFolder + "/" + file[1], found, skipped, visited, depth + 1);
            } else {
                String lower = file[1].toLowerCase(Locale.ROOT);
                if (!lower.endsWith(".mp3") && !lower.endsWith(".flac")) continue;
                Uri document = DocumentsContract.buildDocumentUriUsingTree(tree, file[0]);
                try { found.add(metadata(document, file[1], folder, parentId, folderArt, relativeFolder.isEmpty() ? file[1] : relativeFolder + "/" + file[1])); }
                catch (Exception error) { skipped[0]++; }
            }
        }
    }

    private Track metadata(Uri uri, String filename, String folder, String parentId, String folderArt, String relativePath)
            throws Exception {
        MediaMetadataRetriever reader = new MediaMetadataRetriever();
        try {
            reader.setDataSource(context, uri);
            String title = fallback(reader.extractMetadata(MediaMetadataRetriever.METADATA_KEY_TITLE),
                    filename.substring(0, filename.lastIndexOf('.')));
            String artist = fallback(reader.extractMetadata(MediaMetadataRetriever.METADATA_KEY_ARTIST),
                    "Неизвестный исполнитель");
            String album = fallback(reader.extractMetadata(MediaMetadataRetriever.METADATA_KEY_ALBUM), folder);
            long duration = parseLong(reader.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION));
            String trackNumber = reader.extractMetadata(MediaMetadataRetriever.METADATA_KEY_CD_TRACK_NUMBER);
            int number = (int) parseLong(trackNumber == null ? "0" : trackNumber.split("/")[0]);
            String art = folderArt;
            byte[] embedded = reader.getEmbeddedPicture();
            if (embedded != null) {
                String embeddedArt = cacheArtwork(uri.toString(), embedded);
                if (!embeddedArt.isEmpty()) art = embeddedArt;
            }
            return new Track(uri.toString(), title, artist, album, parentId + "|" + album, art, duration, number, relativePath);
        } finally { reader.release(); }
    }

    private String cacheArtwork(Uri uri) {
        try (InputStream input = context.getContentResolver().openInputStream(uri)) {
            if (input == null) return "";
            return cacheArtwork(uri.toString(), readLimited(input, 16 * 1024 * 1024));
        } catch (Exception error) { return ""; }
    }

    private String cacheArtwork(String key, byte[] data) {
        try {
            BitmapFactory.Options options = new BitmapFactory.Options();
            options.inJustDecodeBounds = true;
            BitmapFactory.decodeByteArray(data, 0, data.length, options);
            options.inSampleSize = 1;
            while (Math.max(options.outWidth, options.outHeight) / options.inSampleSize > 512)
                options.inSampleSize *= 2;
            options.inJustDecodeBounds = false;
            Bitmap bitmap = BitmapFactory.decodeByteArray(data, 0, data.length, options);
            if (bitmap == null) return "";
            File folder = new File(context.getCacheDir(), "artwork");
            if (!folder.isDirectory() && !folder.mkdirs()) { bitmap.recycle(); return ""; }
            byte[] hash = MessageDigest.getInstance("SHA-256").digest(key.getBytes(StandardCharsets.UTF_8));
            StringBuilder name = new StringBuilder();
            for (byte value : hash) name.append(String.format(Locale.ROOT, "%02x", value & 255));
            File file = new File(folder, name + ".jpg");
            try (FileOutputStream output = new FileOutputStream(file)) {
                bitmap.compress(Bitmap.CompressFormat.JPEG, 85, output);
            } finally { bitmap.recycle(); }
            return file.getAbsolutePath();
        } catch (Exception error) { return ""; }
    }

    private static String fallback(String value, String alternate) {
        return value == null || value.trim().isEmpty() ? alternate : value.trim();
    }
    private static long parseLong(String value) {
        try { return Long.parseLong(value); } catch (Exception error) { return 0; }
    }
    private static byte[] readAll(InputStream input) throws Exception {
        return readLimited(input, 32 * 1024 * 1024);
    }
    private static byte[] readLimited(InputStream input, int limit) throws Exception {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        byte[] buffer = new byte[8192];
        int count;
        while ((count = input.read(buffer)) != -1) {
            if (output.size() + count > limit) throw new IllegalStateException("File too large");
            output.write(buffer, 0, count);
        }
        return output.toByteArray();
    }

    public JSONObject playlists() {
        try { return new JSONObject(prefs.getString("playlists", "{}")); }
        catch (JSONException error) { return new JSONObject(); }
    }
    public void savePlaylists(JSONObject playlists) {
        prefs.edit().putString("playlists", playlists.toString()).apply();
    }
    public List<Track> playlistTracks(String name) {
        ArrayList<Track> result = new ArrayList<>();
        JSONArray uris = playlists().optJSONArray(name);
        if (uris != null) for (int i = 0; i < uris.length(); i++) {
            Track track = find(uris.optString(i));
            if (track != null) result.add(track);
        }
        return result;
    }

    public static List<String> uris(List<Track> tracks) {
        ArrayList<String> result = new ArrayList<>();
        for (Track track : tracks) result.add(track.uri);
        return result;
    }

    public void close() { scanner.shutdown(); }
}

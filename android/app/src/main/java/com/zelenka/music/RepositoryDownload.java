package com.zelenka.music;

import android.content.ContentResolver;
import android.net.Uri;
import android.provider.DocumentsContract;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.UUID;

/** Downloads only songs/ and lyrics/ from one pinned public GitHub commit. */
final class RepositoryDownload {
    static final String REPOSITORY = "shakinessaim/zelenkamusic";
    interface Progress { void update(String message); }
    private volatile boolean cancelled;
    private volatile HttpURLConnection connection;
    void cancel() { cancelled = true; HttpURLConnection current = connection; if (current != null) current.disconnect(); }
    private void check() throws IOException { if (cancelled || Thread.currentThread().isInterrupted()) throw new IOException("Загрузка отменена. Готовые файлы сохранены."); }
    String download(ContentResolver resolver, Uri musicTree, Uri lyricsTree, Progress progress) throws Exception {
        progress.update("Проверка репозитория GitHub…");
        String api = "https://api.github.com/repos/" + REPOSITORY;
        JSONObject repository = json(api);
        String branch = repository.getString("default_branch");
        String commit = json(api + "/branches/" + Uri.encode(branch)).getJSONObject("commit").getString("sha");
        JSONObject manifest = json(api + "/git/trees/" + commit + "?recursive=1");
        if (manifest.optBoolean("truncated")) throw new IOException("GitHub вернул неполный список файлов. Загрузка не начата.");
        JSONArray tree = manifest.getJSONArray("tree");
        List<JSONObject> files = new ArrayList<>();
        for (int i = 0; i < tree.length(); i++) {
            JSONObject entry = tree.getJSONObject(i);
            String path = entry.getString("path");
            if (entry.getString("type").equals("blob") && !entry.getString("mode").equals("120000") && RepositoryPath.downloadable(path)) files.add(entry);
        }
        if (files.isEmpty()) throw new IOException("В репозитории пока нет файлов songs или lyrics.");
        FolderAccess songs = new FolderAccess(resolver, musicTree), lyrics = new FolderAccess(resolver, lyricsTree);
        int downloaded = 0, existing = 0;
        for (JSONObject file : files) {
            check();
            String path = file.getString("path"), relative = path.substring(path.indexOf('/') + 1);
            FolderAccess access = path.startsWith("songs/") ? songs : lyrics;
            int slash = relative.lastIndexOf('/');
            String name = relative.substring(slash + 1), parent = slash < 0 ? "" : relative.substring(0, slash);
            FolderAccess.Directory directory = access.folder(parent, true);
            String label = String.format(Locale.ROOT, "Файл %d/%d: %s", downloaded + existing + 1, files.size(), path);
            progress.update(label);
            if (directory.entry(name) != null) {
                if (directory.entry(name).directory) throw new IOException("Папка мешает загрузить файл: " + relative);
                existing++; continue;
            }
            Uri temporary = DocumentsContract.createDocument(resolver, access.document(directory.id), "application/octet-stream",
                    ".zelenka-download-" + UUID.randomUUID() + ".part");
            if (temporary == null) throw new IOException("Нет доступа для записи. Выберите папки повторно.");
            try {
                String encoded = Uri.encode(path, "/");
                String raw = "https://raw.githubusercontent.com/" + REPOSITORY + "/" + commit + "/" + encoded;
                long expected = file.getLong("size");
                byte[] pointer = null;
                if (expected < 1024) {
                    pointer = bytes(raw, 1024);
                    if (pointer.length != expected) throw new IOException("GitHub вернул неполный файл: " + path);
                }
                String small = pointer == null ? "" : new String(pointer, StandardCharsets.UTF_8);
                if (small.startsWith("version https://git-lfs.github.com/spec/v1\n")) {
                    long lfsSize = -1; String hash = null;
                    for (String line : small.split("\n")) {
                        if (line.startsWith("size ")) lfsSize = Long.parseLong(line.substring(5).trim());
                        if (line.startsWith("oid sha256:")) hash = line.substring(11).trim();
                    }
                    if (lfsSize < 0 || hash == null || !hash.matches("[0-9a-f]{64}")) throw new IOException("Некорректный Git LFS файл.");
                    copy("https://media.githubusercontent.com/media/" + REPOSITORY + "/" + commit + "/" + encoded,
                            resolver, temporary, lfsSize, hash, label, progress);
                } else if (pointer != null) {
                    try (OutputStream output = resolver.openOutputStream(temporary, "wt")) {
                        if (output == null) throw new IOException("Не удалось записать файл."); output.write(pointer);
                    }
                } else copy(raw, resolver, temporary, expected, null, label, progress);
                check();
                access.refresh(directory);
                if (directory.entry(name) != null) { existing++; continue; }
                Uri saved = DocumentsContract.renameDocument(resolver, temporary, name);
                if (saved == null) throw new IOException("Папка не поддерживает сохранение файлов. Выберите папку в памяти телефона.");
                temporary = saved;
                if (!access.name(saved).equalsIgnoreCase(name)) throw new IOException("Папка изменила имя файла. Выберите другую папку.");
                directory.entries.put(name, new FolderAccess.Entry(DocumentsContract.getDocumentId(saved), name, false));
                temporary = null; downloaded++;
            } finally { if (temporary != null) try { DocumentsContract.deleteDocument(resolver, temporary); } catch (Exception ignored) { } }
        }
        return String.format(Locale.ROOT, "Загружено файлов: %d. Уже существовали: %d.", downloaded, existing);
    }
    private JSONObject json(String url) throws Exception { return new JSONObject(new String(bytes(url, 32 * 1024 * 1024), StandardCharsets.UTF_8)); }
    private HttpURLConnection open(String url) throws Exception {
        check();
        HttpURLConnection request = (HttpURLConnection) new URL(url).openConnection(); connection = request;
        request.setConnectTimeout(30000); request.setReadTimeout(30000);
        request.setRequestProperty("User-Agent", "ZelenkaMusic/0.1.3");
        request.setRequestProperty("Accept-Encoding", "identity");
        int status = request.getResponseCode();
        if (status != 200) {
            request.disconnect(); connection = null;
            throw new IOException(status == 404 ? "Репозиторий или файл недоступен. Репозиторий должен быть публичным."
                    : status == 403 || status == 429 ? "GitHub ограничил загрузку. Попробуйте позже." : "Ошибка GitHub: HTTP " + status);
        }
        return request;
    }
    private byte[] bytes(String url, int limit) throws Exception {
        HttpURLConnection request = open(url);
        try (InputStream input = request.getInputStream(); ByteArrayOutputStream output = new ByteArrayOutputStream()) {
            byte[] buffer = new byte[8192]; int read;
            while ((read = input.read(buffer)) != -1) { check(); output.write(buffer, 0, read); if (output.size() > limit) throw new IOException("Ответ GitHub слишком большой."); }
            return output.toByteArray();
        } finally { request.disconnect(); connection = null; }
    }
    private void copy(String url, ContentResolver resolver, Uri target, long expected, String expectedHash, String label, Progress progress) throws Exception {
        HttpURLConnection request = open(url);
        try (InputStream input = request.getInputStream(); OutputStream output = resolver.openOutputStream(target, "wt")) {
            if (output == null) throw new IOException("Нет доступа для записи.");
            MessageDigest hash = expectedHash == null ? null : MessageDigest.getInstance("SHA-256");
            byte[] buffer = new byte[65536]; int read; long count = 0, lastReport = 0;
            while ((read = input.read(buffer)) != -1) {
                check(); output.write(buffer, 0, read); count += read;
                if (hash != null) hash.update(buffer, 0, read);
                if (count > expected) throw new IOException("Размер файла отличается от репозитория.");
                long now = System.currentTimeMillis();
                if (now - lastReport >= 1000) { progress.update(label + String.format(Locale.ROOT, " · %.1f / %.1f МБ", count / 1048576.0, expected / 1048576.0)); lastReport = now; }
            }
            if (count != expected) throw new IOException("Загрузка файла прервана. Повторите загрузку.");
            if (hash != null) {
                StringBuilder actual = new StringBuilder(); for (byte value : hash.digest()) actual.append(String.format(Locale.ROOT, "%02x", value & 255));
                if (!actual.toString().equals(expectedHash)) throw new IOException("Контрольная сумма Git LFS файла не совпала.");
            }
        } finally { request.disconnect(); connection = null; }
    }
}

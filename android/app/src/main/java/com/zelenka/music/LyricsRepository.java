package com.zelenka.music;

import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.io.IOException;

final class LyricsRepository {
    static final class Result {
        final Lyrics lyrics; final String source;
        Result(Lyrics lyrics, String source) { this.lyrics = lyrics; this.source = source; }
    }
    static Result load(Context context, String folder, Track track) throws Exception {
        if (folder.isEmpty() || track == null) return null;
        FolderAccess access = new FolderAccess(context.getContentResolver(), Uri.parse(folder));
        String relative = track.relativePath;
        if (relative.isEmpty()) {
            try (Cursor cursor = context.getContentResolver().query(Uri.parse(track.uri),
                    new String[]{DocumentsContract.Document.COLUMN_DISPLAY_NAME}, null, null, null)) {
                if (cursor == null || !cursor.moveToFirst()) return null;
                relative = cursor.getString(0);
            }
        }
        int slash = relative.lastIndexOf('/');
        String name = relative.substring(slash + 1);
        int extension = name.lastIndexOf('.');
        String stem = extension < 0 ? name : name.substring(0, extension);
        String mirrored = slash < 0 ? "" : relative.substring(0, slash);
        for (String location : mirrored.isEmpty() ? new String[]{""} : new String[]{mirrored, ""}) {
            FolderAccess.Directory directory = access.folder(location, false);
            if (directory == null) continue;
            for (String ext : new String[]{".lrc", ".txt"}) {
                FolderAccess.Entry entry = directory.entries.get(stem + ext);
                if (entry == null) for (FolderAccess.Entry candidate : directory.entries.values())
                    if (candidate.name.equalsIgnoreCase(stem + ext)) { entry = candidate; break; }
                if (entry == null || entry.directory) continue;
                try (InputStream input = context.getContentResolver().openInputStream(access.document(entry.id));
                     ByteArrayOutputStream output = new ByteArrayOutputStream()) {
                    if (input == null) throw new IOException("Файл текста недоступен.");
                    byte[] buffer = new byte[8192]; int read;
                    while ((read = input.read(buffer)) != -1) { output.write(buffer, 0, read); if (output.size() > Lyrics.MAX_BYTES) throw new IOException("Файл текста больше 1 МБ."); }
                    return new Result(Lyrics.parse(Lyrics.decode(output.toByteArray())), (location.isEmpty() ? "" : location + "/") + entry.name);
                }
            }
        }
        return null;
    }
}

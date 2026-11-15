package com.zelenka.music;

import android.content.ContentResolver;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import java.io.IOException;
import java.util.HashMap;
import java.util.Map;

/** Uses provider document IDs, never assumes that a URI is a filesystem path. */
final class FolderAccess {
    final ContentResolver resolver;
    final Uri tree;
    private final Map<String, Directory> directories = new HashMap<>();
    static final class Entry {
        final String id, name; final boolean directory;
        Entry(String id, String name, boolean directory) { this.id = id; this.name = name; this.directory = directory; }
    }
    static final class Directory {
        final String id; final Map<String, Entry> entries;
        Directory(String id, Map<String, Entry> entries) { this.id = id; this.entries = entries; }
        Entry entry(String name) {
            Entry exact = entries.get(name); if (exact != null) return exact;
            for (Entry candidate : entries.values()) if (candidate.name.equalsIgnoreCase(name)) return candidate;
            return null;
        }
    }
    FolderAccess(ContentResolver resolver, Uri tree) { this.resolver = resolver; this.tree = tree; }
    Uri document(String id) { return DocumentsContract.buildDocumentUriUsingTree(tree, id); }
    Directory folder(String relative, boolean create) throws Exception {
        if (directories.containsKey(relative)) return directories.get(relative);
        String id = DocumentsContract.getTreeDocumentId(tree), prefix = "";
        Directory current = directory(id);
        directories.put("", current);
        if (relative.isEmpty()) return current;
        for (String name : relative.split("/", -1)) {
            if (!RepositoryPath.validComponent(name)) throw new IOException("Некорректное имя папки.");
            prefix = prefix.isEmpty() ? name : prefix + "/" + name;
            if (directories.containsKey(prefix)) { current = directories.get(prefix); continue; }
            Entry child = current.entry(name);
            if (child == null && create) {
                Uri created = DocumentsContract.createDocument(resolver, document(current.id), DocumentsContract.Document.MIME_TYPE_DIR, name);
                if (created == null) throw new IOException("Не удалось создать папку: " + name);
                child = new Entry(DocumentsContract.getDocumentId(created), name, true); current.entries.put(name, child);
            }
            if (child == null) return null;
            if (!child.directory) throw new IOException("Файл мешает создать папку: " + name);
            current = directory(child.id); directories.put(prefix, current);
        }
        return current;
    }
    void refresh(Directory folder) throws Exception {
        Map<String, Entry> updated = directory(folder.id).entries;
        folder.entries.clear(); folder.entries.putAll(updated);
    }
    String name(Uri document) throws Exception {
        try (Cursor cursor = resolver.query(document, new String[]{DocumentsContract.Document.COLUMN_DISPLAY_NAME}, null, null, null)) {
            if (cursor == null || !cursor.moveToFirst()) throw new IOException("Сохранённый файл недоступен.");
            return cursor.getString(0);
        }
    }
    private Directory directory(String id) throws Exception {
        Map<String, Entry> entries = new HashMap<>();
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, id);
        try (Cursor cursor = resolver.query(children, new String[]{DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE}, null, null, null)) {
            if (cursor == null) throw new IOException("Папка недоступна. Выберите её ещё раз.");
            while (cursor.moveToNext()) entries.put(cursor.getString(1), new Entry(cursor.getString(0), cursor.getString(1),
                    DocumentsContract.Document.MIME_TYPE_DIR.equals(cursor.getString(2))));
        }
        return new Directory(id, entries);
    }
}

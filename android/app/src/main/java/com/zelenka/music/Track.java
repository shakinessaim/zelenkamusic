package com.zelenka.music;

import org.json.JSONException;
import org.json.JSONObject;

public final class Track {
    public final String uri, title, artist, album, albumKey, artwork;
    public final long duration;
    public final int number;
    public final String relativePath;

    public Track(String uri, String title, String artist, String album, String albumKey,
                 String artwork, long duration, int number) {
        this(uri, title, artist, album, albumKey, artwork, duration, number, "");
    }
    public Track(String uri, String title, String artist, String album, String albumKey,
                 String artwork, long duration, int number, String relativePath) {
        this.uri = uri;
        this.title = title;
        this.artist = artist;
        this.album = album;
        this.albumKey = albumKey;
        this.artwork = artwork;
        this.duration = duration;
        this.number = number;
        this.relativePath = relativePath;
    }

    public JSONObject toJson() throws JSONException {
        return new JSONObject().put("uri", uri).put("title", title).put("artist", artist)
                .put("album", album).put("albumKey", albumKey).put("artwork", artwork)
                .put("duration", duration).put("number", number).put("relativePath", relativePath);
    }

    public static Track fromJson(JSONObject json) throws JSONException {
        return new Track(json.getString("uri"), json.getString("title"), json.optString("artist"),
                json.optString("album"), json.getString("albumKey"), json.optString("artwork"),
                json.optLong("duration"), json.optInt("number"), json.optString("relativePath"));
    }
}

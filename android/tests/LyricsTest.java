package com.zelenka.music;

public final class LyricsTest {
    private static void equal(Object expected, Object actual) {
        if (!expected.equals(actual)) throw new AssertionError("Expected " + expected + ", got " + actual);
    }
    public static void main(String[] args) throws Exception {
        equal("Первая\n\nВторая", Lyrics.parse("\ufeffПервая\r\n\r\nВторая").text);
        equal("П", Lyrics.decode(new byte[]{(byte) 255, (byte) 254, 31, 4}));
        try { Lyrics.decode(new byte[]{(byte) 255, (byte) 254, 0}); throw new AssertionError("Bad UTF-16 accepted"); }
        catch (java.nio.charset.CharacterCodingException expected) { }
        Lyrics lyrics = Lyrics.parse("[ar:Artist]\n[00:10.50][00:30.500]Chorus\n[00:20]Verse\n[00:20]Harmony\n[00:25]\n[offset:500]");
        equal(true, lyrics.synchronizedLyrics());
        equal("", lyrics.currentLine(9900)); equal("Chorus", lyrics.currentLine(10000));
        equal("Verse\nHarmony", lyrics.currentLine(19500)); equal("", lyrics.currentLine(24500));
        equal("Chorus", lyrics.currentLine(30000)); equal("Chorus", lyrics.currentLine(11000));
        equal("[00:99]Invalid\n[00:NaN]Literal", Lyrics.parse("[00:99]Invalid\n[00:NaN]Literal").text);
        equal(false, RepositoryPath.downloadable("songs/../private.txt"));
        equal(false, RepositoryPath.downloadable("songs//song.mp3"));
        equal(false, RepositoryPath.downloadable("lyrics/..\\private.txt"));
        equal(false, RepositoryPath.downloadable("source/Main.java"));
        equal(true, RepositoryPath.downloadable("songs/Альбом/01 - Песня.flac"));
        System.out.println("Lyrics decoding, LRC timing/seek, and repository path checks passed.");
    }
}

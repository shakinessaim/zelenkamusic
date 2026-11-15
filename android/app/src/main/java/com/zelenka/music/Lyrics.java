package com.zelenka.music;

import java.nio.ByteBuffer;
import java.nio.charset.Charset;
import java.nio.charset.CodingErrorAction;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** Plain text and LRC parsing without Android dependencies. */
public final class Lyrics {
    public static final int MAX_BYTES = 1024 * 1024;
    public final String text;
    private final List<Cue> cues;
    private static final class Cue {
        double time; final String text;
        Cue(double time, String text) { this.time = time; this.text = text; }
    }
    private Lyrics(String text, List<Cue> cues) { this.text = text; this.cues = cues; }
    public boolean synchronizedLyrics() { return !cues.isEmpty(); }
    public String currentLine(long millis) {
        double seconds = millis / 1000.0;
        int low = 0, high = cues.size();
        while (low < high) { int middle = (low + high) / 2; if (cues.get(middle).time <= seconds) low = middle + 1; else high = middle; }
        if (low == 0) return "";
        double time = cues.get(low - 1).time;
        int start = low - 1;
        while (start > 0 && cues.get(start - 1).time == time) start--;
        StringBuilder line = new StringBuilder();
        for (int i = start; i < low; i++) { if (i > start) line.append('\n'); line.append(cues.get(i).text); }
        return line.toString();
    }
    public static Lyrics parse(String source) {
        if (source.startsWith("\ufeff")) source = source.substring(1);
        List<Cue> cues = new ArrayList<>();
        List<String> lines = new ArrayList<>();
        double offset = 0;
        for (String original : source.split("\\r?\\n", -1)) {
            String remaining = original.replaceAll("\\s+$", "");
            List<Double> times = new ArrayList<>();
            while (remaining.startsWith("[") && remaining.indexOf(']') > 0) {
                int end = remaining.indexOf(']'); String tag = remaining.substring(1, end);
                Double time = timestamp(tag);
                if (time != null) { times.add(time); remaining = remaining.substring(end + 1); }
                else if (times.isEmpty() && tag.startsWith("offset:")) {
                    try { double value = Double.parseDouble(tag.substring(7).trim()); offset = Double.isFinite(value) ? value / 1000 : 0; }
                    catch (NumberFormatException ignored) { offset = 0; }
                    remaining = ""; break;
                } else if (times.isEmpty() && tag.matches("^(ar|ti|al|by|re|ve|length):.*")) { remaining = ""; break; }
                else break;
            }
            if (times.isEmpty()) { if (!remaining.isEmpty() || original.trim().isEmpty()) lines.add(remaining); }
            else { String line = remaining.trim(); lines.add(line); for (double time : times) cues.add(new Cue(time, line)); }
        }
        for (Cue cue : cues) cue.time = Math.max(0, cue.time - offset);
        Collections.sort(cues, (a, b) -> Double.compare(a.time, b.time));
        return new Lyrics(String.join("\n", lines).trim(), cues);
    }
    private static Double timestamp(String tag) {
        if (!tag.matches("[0-9]+:[0-9]+(?:\\.[0-9]+)?")) return null;
        try {
            String[] parts = tag.split(":");
            long minutes = Long.parseLong(parts[0]); double seconds = Double.parseDouble(parts[1]);
            if (!Double.isFinite(seconds) || seconds >= 60 || minutes > Integer.MAX_VALUE) return null;
            return minutes * 60.0 + seconds;
        } catch (NumberFormatException error) { return null; }
    }
    public static String decode(byte[] bytes) throws Exception {
        String encoding = "UTF-8"; int start = 0;
        if (bytes.length >= 2 && (bytes[0] & 255) == 255 && (bytes[1] & 255) == 254) { encoding = "UTF-16LE"; start = 2; }
        else if (bytes.length >= 2 && (bytes[0] & 255) == 254 && (bytes[1] & 255) == 255) { encoding = "UTF-16BE"; start = 2; }
        String value = Charset.forName(encoding).newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes, start, bytes.length - start)).toString();
        return value.startsWith("\ufeff") ? value.substring(1) : value;
    }
}

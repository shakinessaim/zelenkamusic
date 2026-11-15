package com.zelenka.music;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.LinkedHashSet;
import java.util.Random;

/** URI-based playback order; independent of Android so its transitions can be tested. */
public final class PlaybackQueue {
    public static final int REPEAT_OFF = 0;
    public static final int REPEAT_ALL = 1;
    public static final int REPEAT_ONE = 2;
    private final Random random;
    private final ArrayList<String> source = new ArrayList<>();
    private final ArrayList<String> order = new ArrayList<>();
    private final ArrayList<String> history = new ArrayList<>();
    private int index = -1;
    private boolean shuffle;
    private boolean continuous;
    private int repeat = REPEAT_OFF;

    public PlaybackQueue() { this(new Random()); }
    public PlaybackQueue(Random random) { this.random = random; }

    public void set(List<String> tracks, String first, boolean shuffled, boolean continuous) {
        source.clear();
        LinkedHashSet<String> unique = new LinkedHashSet<>(tracks);
        unique.remove(null);
        source.addAll(unique);
        shuffle = shuffled;
        this.continuous = continuous;
        history.clear();
        rebuild(first, null);
    }

    private void rebuild(String first, String previous) {
        order.clear();
        order.addAll(source);
        if (shuffle) Collections.shuffle(order, random);
        if (shuffle && first != null && order.remove(first)) order.add(0, first);
        else if (shuffle && order.size() > 1 && order.get(0).equals(previous))
            Collections.swap(order, 0, 1 + random.nextInt(order.size() - 1));
        index = order.isEmpty() ? -1 : first != null && !shuffle && order.contains(first)
                ? order.indexOf(first) : 0;
    }

    public String current() { return index < 0 ? null : order.get(index); }
    public List<String> source() { return new ArrayList<>(source); }
    public boolean isShuffle() { return shuffle; }
    public boolean isContinuous() { return continuous; }
    public int repeat() { return repeat; }
    public void setRepeat(int mode) {
        if (mode < REPEAT_OFF || mode > REPEAT_ONE) throw new IllegalArgumentException("Repeat mode");
        repeat = mode;
    }

    public void setShuffle(boolean enabled) {
        String selected = current();
        shuffle = enabled;
        if (enabled) rebuild(selected, null);
        else {
            order.clear();
            order.addAll(source);
            index = selected == null ? -1 : order.indexOf(selected);
        }
    }

    public String next(boolean completion) {
        String previous = current();
        if (previous == null) return null;
        if (completion && repeat == REPEAT_ONE) return previous;
        if (index + 1 < order.size()) index++;
        else if (repeat == REPEAT_ALL || continuous) rebuild(null, previous);
        else return null;
        history.add(previous);
        if (history.size() > 1000) history.remove(0);
        return current();
    }

    public String previous() {
        if (history.isEmpty()) return current();
        String selected = history.remove(history.size() - 1);
        index = order.indexOf(selected);
        return current();
    }
}

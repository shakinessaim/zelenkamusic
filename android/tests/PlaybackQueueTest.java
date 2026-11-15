package com.zelenka.music;

import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.Random;
import java.util.Set;

/** Run using scripts/Test-Android.ps1. No Gradle, Android device, or test dependencies needed. */
public final class PlaybackQueueTest {
    private static int checks;
    private static void check(boolean condition, String message) {
        checks++;
        if (!condition) throw new AssertionError(message);
    }
    private static PlaybackQueue queue(boolean shuffle, boolean continuous) {
        PlaybackQueue queue = new PlaybackQueue(new Random(17));
        queue.set(Arrays.asList("a", "b", "c", "d"), null, shuffle, continuous);
        return queue;
    }
    public static void main(String[] args) {
        PlaybackQueue empty = new PlaybackQueue();
        empty.set(Collections.emptyList(), null, true, true);
        check(empty.current() == null && empty.next(true) == null && empty.previous() == null, "Empty library");

        PlaybackQueue sequential = queue(false, false);
        check("a".equals(sequential.current()), "Start at first song");
        check("b".equals(sequential.next(true)), "Sequential completion");
        check("a".equals(sequential.previous()), "Previous returns to history");
        sequential.next(true); sequential.next(true); sequential.next(true);
        check(sequential.next(true) == null && "d".equals(sequential.current()), "Repeat off stops at end");
        sequential.setRepeat(PlaybackQueue.REPEAT_ALL);
        check("a".equals(sequential.next(true)), "Repeat all wraps to start");
        check("d".equals(sequential.previous()), "History survives a repeated cycle");

        PlaybackQueue selected = new PlaybackQueue();
        selected.set(Arrays.asList("a", "b", "c", "d"), "c", false, false);
        check("c".equals(selected.current()) && "d".equals(selected.next(true)), "Selected song preserves album order");
        check(selected.next(true) == null, "Selection stops at end of album");

        PlaybackQueue repeatOne = queue(false, false);
        repeatOne.setRepeat(PlaybackQueue.REPEAT_ONE);
        check("a".equals(repeatOne.next(true)), "Repeat one on completion");
        check("b".equals(repeatOne.next(false)), "Manual next escapes repeat one");

        PlaybackQueue shuffled = queue(true, true);
        String last = null;
        for (int cycle = 0; cycle < 200; cycle++) {
            Set<String> heard = new HashSet<>();
            String first = shuffled.current();
            check(!first.equals(last), "No adjacent duplicate between shuffle cycles");
            for (int song = 0; song < 4; song++) {
                String current = shuffled.current();
                check(heard.add(current), "Each song occurs once in a shuffle cycle");
                last = current;
                shuffled.next(true);
            }
            check(heard.size() == 4, "Shuffle cycle contains the entire library");
        }

        PlaybackQueue single = new PlaybackQueue();
        single.set(Collections.singletonList("only"), null, true, true);
        check("only".equals(single.next(true)), "A single song can repeat in continuous mode");

        PlaybackQueue deduplicated = new PlaybackQueue();
        deduplicated.set(Arrays.asList("a", "a", null, "b"), null, false, false);
        check(deduplicated.source().size() == 2, "Duplicate and null URIs are removed");
        deduplicated.source().clear();
        check(deduplicated.source().size() == 2, "Caller cannot mutate queue storage");

        PlaybackQueue toggle = queue(false, false);
        toggle.next(true); toggle.next(true);
        toggle.setShuffle(true);
        check("c".equals(toggle.current()), "Enabling shuffle retains current song");
        toggle.setShuffle(false);
        check("c".equals(toggle.current()) && "d".equals(toggle.next(true)), "Disabling shuffle resumes album order");

        boolean invalid = false;
        try { toggle.setRepeat(9); } catch (IllegalArgumentException expected) { invalid = true; }
        check(invalid, "Invalid repeat mode is rejected");
        System.out.println("PlaybackQueue: " + checks + " checks passed (including 200 shuffle cycles).");
    }
}

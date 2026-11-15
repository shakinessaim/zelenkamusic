package com.zelenka.music;

final class RepositoryPath {
    static boolean validComponent(String value) {
        return !value.isEmpty() && !value.equals(".") && !value.equals("..")
                && value.indexOf('/') < 0 && value.indexOf('\\') < 0 && value.indexOf('\0') < 0;
    }
    static boolean downloadable(String path) {
        if (!path.startsWith("songs/") && !path.startsWith("lyrics/")) return false;
        for (String component : path.split("/", -1)) if (!validComponent(component)) return false;
        return true;
    }
}

# Lyrics

Add UTF-8 `.txt` files or timed `.lrc` files manually. UTF-16 with a BOM also works.
Use the exact audio filename with its extension replaced, preferably with the same album folders:

```text
songs/Album/01 - Song.flac
lyrics/Album/01 - Song.txt
```

Flat files such as `lyrics/01 - Song.lrc` also work. Album-specific files take precedence;
within a location, `.lrc` takes precedence over `.txt`. Press **Refresh** on the lyrics page
after editing a file. Nothing is fetched while listening.

LRC supports `[mm:ss.xx]`, multiple timestamps per line, metadata, and `[offset:milliseconds]`.
A positive offset displays lines earlier. The current timed line follows playback and seeking.
Each file must be at most 1 MiB. Blank lines in plain text preserve paragraph breaks.

On Android, select separate music and lyrics folders in Settings. Downloading from GitHub
uses those selected folders and skips files that already exist.

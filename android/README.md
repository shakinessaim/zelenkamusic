# ZelenkaMusic for Android

Additional native Android edition of the Windows player, for Android 8.0 (API 26) and newer. It keeps the green theme, logo, Russian interface, albums, editable playlists, optional snowfall, and local MP3/FLAC playback. The APK supports all Android CPU architectures because the app uses Android's media APIs and has no native libraries or external runtime dependencies.

Music and lyrics are selected with Android's folder picker (Storage Access Framework), with persistent access and no broad storage permission. Internet permission is used only for the optional GitHub download in Settings. The app writes missing downloaded files to the selected folders and preserves existing files. Nothing is uploaded or bundled in the APK. Metadata and playlists stay in app-private storage. Removing the app clears those settings, while music and lyrics in the selected folders remain.

Playback uses a foreground media service, a media session, wake locks while playing, notification and lock-screen controls, audio focus, and pause on headphone disconnection. Home playback shuffles without repeating a song within a cycle. Albums and playlists support sequential playback, shuffle, and repeat off/all/one. A saved song and position restore on pause after a process restart. Desktop and Android libraries/settings are separate; copy music to the phone yourself.

## Lyrics and GitHub downloads

In **Settings**, choose separate folders for music and `lyrics`. To download the public
`shakinessaim/zelenkamusic` repository's `songs` and `lyrics` folders, allow read/write access
to both folders and tap **Download music and lyrics**. The data-sync foreground service
shows progress and a cancel action, and continues while the activity is closed or rotated.
Existing files are skipped; retrying adds missing files. Partial downloads are removed.
Public Git LFS files are supported and their SHA-256 checksums are verified. Only the two
content folders are downloaded, from one pinned commit on the repository's default branch.
The library refreshes when the app is open after completion or cancellation.

Add UTF-8 `.txt` or timed `.lrc` files manually, using the audio filename with its extension
replaced, for example `songs/Album/01 - Song.flac` and `lyrics/Album/01 - Song.txt`.
Matching subfolders take precedence over flat files; `.lrc` takes precedence over `.txt`
within a location. UTF-16 with a BOM is also supported. Open the **Lyrics** tab and tap
**Refresh lyrics** after editing. LRC supports multiple timestamps, metadata and millisecond
offsets (positive values display lines earlier). Each file is limited to 1 MiB. Playback,
lyrics display and seeking work offline. Rescan an older library once to enable matching
album subfolders for tracks cached by version 0.1.2.

## Build the APK on Windows

Install Android Studio, then open **SDK Manager** and install:

- Android SDK Platform 35 (or newer).
- Android SDK Build-Tools 35.0.0 (or newer).
- Android SDK Platform-Tools if installing through USB or an emulator.

Android Studio includes the required Java JDK. The standalone script uses the SDK directly, so building the APK does not require a Gradle download or Maven dependencies once those tools are installed. Run from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\Build-Android.ps1
```

For a custom installation path:

```powershell
.\scripts\Build-Android.ps1 -SdkPath 'D:\Android\Sdk' -JavaHome 'D:\Android Studio\jbr'
```

The script tests playback ordering, compiles resources and Java, generates DEX bytecode, aligns and signs the APK, and verifies the signature and alignment. Results:

- `dist/ZelenkaMusic-Android-0.1.3.apk` — signed installer.
- `dist/ZelenkaMusic-Android-0.1.3.apk.sha256` — checksum.
- `dist/ZelenkaMusic-Android-INSTALL.txt` — phone installation instructions.

Build and install on an attached phone with USB debugging enabled or a running emulator:

```powershell
.\scripts\Build-Android.ps1 -Install
# If multiple devices are connected:
.\scripts\Build-Android.ps1 -Install -DeviceSerial emulator-5554
```

The default signing key is generated once in `.tools/android-signing/zelenka-local.jks`, alias `zelenka-local`, with the development password `android`. Keep that file for compatible updates. It is excluded from source control. For distribution using your own release key, supply `-KeystorePath` and `-KeyAlias`, with passwords in `ZELENKA_KEYSTORE_PASSWORD` and optionally `ZELENKA_KEY_PASSWORD`; the build script does not print these environment variables. A build made with a new signing key requires uninstalling an earlier installation, which clears app settings and playlists.

## Android Studio

Open the `android` directory as a project. Gradle sync requires internet access for Android Gradle Plugin 8.7.3 and Gradle 8.9. The repository includes no Gradle wrapper binary; use Android Studio's local Gradle distribution or install Gradle 8.9. The PowerShell build above bypasses Gradle entirely. Gradle's `preBuild` task copies the shared desktop logo into generated resources. Android Studio debug builds use its own debug signing key, so use the standalone build consistently for sideloaded updates.

## Checks

The built APK and completed checks are recorded in [VERIFICATION.md](VERIFICATION.md).

```powershell
.\scripts\Test-Android.ps1
```

The Java queue checks cover selection and album order, empty libraries, previous/next, all repeat modes, shuffle toggling, duplicate filtering, and 200 consecutive shuffle cycles without adjacent repeats. They run with a Java JDK and require no emulator.

On a phone/emulator, verify folder access after restarting, MP3 and FLAC playback, corrupt-file handling, cover art, album and playlist edits, seeking, shuffle/repeat, pause on unplug, interruptions from another audio app, background playback with the screen off, notification controls, rotation, and snowfall. Android's file picker restricts access to some system folders; select your Music folder or an SD-card subfolder. Platform MP3/FLAC decoding is used, so uncommon FLAC encodings depend on the device's decoder.

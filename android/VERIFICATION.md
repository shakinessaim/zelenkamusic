# Android APK verification

Built on Windows on 15 November 2026 with Android Studio's Java 21.0.10, Android SDK Platform 36.1, and Build-Tools 37.0.0 using `scripts/Build-Android.ps1`.

- Java compilation completed successfully with explicit compiler diagnostic and exit-status checks.
- Playback queue: 1,217 checks passed, including 200 shuffle cycles, repeat modes, previous/next, selection order, and empty libraries.
- Android resource compilation and linking passed.
- APK signature verification passed for APK Signature Scheme v2 and v3.
- APK alignment and SHA-256 checksum checks passed.
- Package: `com.zelenka.music`, version `0.1.2`, version code `102`.
- Minimum Android API: `26` (Android 8.0). Target API: `35`.
- Launcher: `com.zelenka.music.MainActivity`.
- DEX validation confirmed the activity, playback service, library repository, playback queue, track model, and snowfall view.
- No architecture-specific native libraries are included; the APK works with Android's platform media APIs on supported CPU architectures.

Installer: `dist/ZelenkaMusic-Android-0.1.2.apk` (54,329 bytes).

SHA-256:

```text
11710CD1A3090D6F0426F347D5F51A97D2E8F7268C9755BE9D032E0A62E334F6
```

The APK uses the reusable local development signing key described in [README.md](README.md). Keep that key for compatible updates.

No phone or emulator runtime test was performed. Folder selection, playback on device, media controls, interruptions, rotation, and visual behavior still require device verification using the checklist in [README.md](README.md).

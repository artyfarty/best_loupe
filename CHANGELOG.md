# Changelog

Versions follow [SemVer](https://semver.org/); each release is a git tag `vX.Y.Z` matching
`Cargo.toml`. The version and the exact `git describe` are burned into the exe (VERSIONINFO
resource, log, tray tooltip).

## v0.2.0 — 2026-10-24

First public release.

- Installer (`setup.exe`, English/Russian): Program Files install, optional autostart, Start-menu
  entry, uninstaller. Plus a portable exe.
- uiAccess signing is now machine-local: the installer creates a certificate on the target
  machine, signs the exe and deletes the private key (no shared publisher certificate). Replaces
  the v0.1.0 shared certificate on reinstall.
- Settings window (tray icon → Settings, or start the exe again): lens size, rounding, border
  width/colour, shadow; start/max zoom, wheel step, animation, smoothing; hotkey (any combination
  of Ctrl/Alt/Shift), activation delay, fade-in, autostart, update checks; About tab. All changes
  apply live. Large-print UI (1.5× the standard dialog scale), English/Russian.
- Update check against GitHub releases (daily, can be disabled; tray balloon + download link).
- App icon (exe, tray, window).
- Shift (when not part of the hotkey) now counts as "another key" and cancels the lens, like any
  other shortcut key.
- GitHub Actions: build + test on every push, releases published from tags.

## v0.1.0 — 2026-10-24

First working version.

- Hold Ctrl+Alt → lens around the cursor; wheel zooms (smoothly, wheel swallowed); release hides.
- Desktop Duplication capture + DirectComposition click-through overlay + HLSL lens shader
  (rounded rect, border, shadow), vsync-paced, cursor sampled per frame.
- Overlay/capture/mouse hook exist only while the chord is held.
- Chord tainting (other keys) and activation delay; `GetAsyncKeyState` safety net.
- Output under the cursor is picked at activation (multi-monitor: one output at a time).
- TOML config, file log, tray icon with Exit, single instance.
- `uiaccess` cargo feature, self-signed code signing, Program Files install with autostart
  (`scripts/deploy.sh`), static CRT, version resource.

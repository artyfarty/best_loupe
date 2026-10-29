# Changelog

Versions follow [SemVer](https://semver.org/); each release is a git tag `vX.Y.Z` matching
`Cargo.toml`. The version and the exact `git describe` are burned into the exe (VERSIONINFO
resource, log, tray tooltip).

## v0.3.1 — 2026-10-29

- Fix: pressing the hotkey and the freeze key at the same moment showed a black lens. The first
  Desktop Duplication frame is empty and was used as the picture; it is now skipped (this also
  removes a one-frame black flash when the lens appears).

## v0.3.0 — 2026-10-29

- Freeze frame: hold the freeze key (Shift by default, selectable in settings) together with the
  hotkey — the lens keeps a still picture while following the cursor and zooming, e.g. to read
  tooltips that vanish when the mouse moves. A frozen lens has a blue border.
- Tray menu: "Pause the lens" (greyed tray icon while paused; not persisted).
- `best_loupe.exe --quit` stops a running instance gracefully (the installed uiAccess copy cannot
  be killed from a normal process); the installer uses it before upgrading.
- Fix: modifiers released over an elevated window could stay "stuck" in the hotkey state.

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

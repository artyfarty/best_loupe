# Changelog

Versions follow [SemVer](https://semver.org/); each release is a git tag `vX.Y.Z` matching
`Cargo.toml`. The version and the exact `git describe` are burned into the exe (VERSIONINFO
resource, log, tray tooltip).

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

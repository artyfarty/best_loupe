# best_loupe

**The magnifier you have deserved for years — and were never given.**

A macOS-style hold-to-zoom lens for Windows. Hold **Ctrl+Alt** and a lens appears around the
cursor and follows it; **scroll** to zoom; let go and it is gone.

Windows' own Magnifier lens is laggy (it copies the screen on a timer and the picture "swims"
behind the cursor), and it falls apart around elevated windows and games. best_loupe takes the
frame DWM has already composed (Desktop Duplication), and draws the lens with a pixel shader into
a click-through topmost overlay that is re-rendered every vsync at the freshly sampled cursor
position. The lens itself never lags the cursor; its content is at most one frame behind.

## Features

- Hold Ctrl+Alt (configurable) → lens; release → gone. Mouse wheel while held → smooth zoom
  (up to 32×), the wheel does not reach the app below. Zoom is remembered between uses.
- Zoom about the cursor: the point under the cursor stays under the cursor; near screen edges the
  lens slides inside the screen instead of showing garbage.
- Runs at the display refresh rate (tested at 164 Hz), ~1 ms to appear.
- Works over elevated (admin) windows, other users' windows, Start/search, and windowed /
  borderless / FSO games (with the signed uiAccess install).
- Costs nothing while idle: no capture, no overlay, no mouse hook until Ctrl+Alt is held — games
  keep their independent-flip fast path.
- Ctrl+Alt+<other key> shortcuts do not flash the lens.
- Add Shift while holding the hotkey to **freeze** the picture: the lens keeps following the
  cursor over a still image — read tooltips and other things that vanish when the mouse moves.
- Pause/resume from the tray menu.
- Rounded lens with border and soft shadow; size, rounding, border, zoom steps, animation,
  smoothing, hotkey — all live-adjustable in a large-print settings window (English / Russian).
- Checks GitHub releases for updates (can be turned off).

Known limits: no lens on the secure desktop (UAC prompt, Ctrl+Alt+Del, lock screen), DRM-protected
video shows black inside the lens, legacy true-exclusive-fullscreen games are not covered. The
lens is invisible to screenshots and screen recordings by design (otherwise it would magnify
itself).

## Requirements

Windows 10 2004+ or Windows 11, a D3D11 GPU. No runtime dependencies.

## Install

Download from [Releases](https://github.com/artyfarty/best_loupe/releases):

- **`best_loupe-vX.Y.Z-setup.exe`** — the normal way. Installs to Program Files, optionally
  starts with Windows, adds a Start-menu entry and an uninstaller. Windows SmartScreen will warn
  about an unknown publisher (the installer is not code-signed): *More info → Run anyway*.
- **`best_loupe-vX.Y.Z-portable-x64.exe`** — just run it; no install. Works everywhere except over
  admin windows and the Start menu.

Why the installer matters: to draw over elevated windows and Start, Windows requires the
`uiAccess` privilege, which only a *trusted-signed* exe in Program Files gets. There is no
publisher certificate to trust: the installer creates a certificate on your machine, signs the
installed exe with it and immediately deletes the private key, so that certificate can never sign
anything else. The uninstaller removes it.

Settings: click the tray icon (or start best_loupe again). Config file:
`%APPDATA%\best_loupe\config.toml` ([all keys](config.example.toml)); log:
`%LOCALAPPDATA%\best_loupe\best_loupe.log`.

## Building

Rust (MSVC toolchain) + Visual Studio Build Tools + Windows SDK; Inno Setup 6 for the installer.
CI builds everything on GitHub Actions and publishes releases from tags. Developed in WSL, built
on the Windows host: `scripts/win-build.sh` (build), `scripts/win-run.sh` (run a dev build),
`scripts/win-installer.sh` (installer), `scripts/deploy.sh` (install a uiAccess build locally or
over SSH). See [docs/build.md](docs/build.md), [docs/release.md](docs/release.md).

## License

[MIT](LICENSE)

## Docs

- [docs/design.md](docs/design.md) — architecture, render pipeline, roadmap
- [docs/research-approaches.md](docs/research-approaches.md) — why this approach (vs Magnification
  API, DWM injection, drivers), existing tools
- [docs/deploy.md](docs/deploy.md) — uiAccess, machine-local signing, install
- [docs/release.md](docs/release.md) — CI, releases, update check
- [CHANGELOG.md](CHANGELOG.md)

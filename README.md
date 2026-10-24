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

- Hold Ctrl+Alt → lens; release → gone. Mouse wheel while held → smooth zoom (1.25×–16×), the
  wheel does not reach the app below. Zoom is remembered between uses.
- Zoom about the cursor: the point under the cursor stays under the cursor; near screen edges the
  lens slides inside the screen instead of showing garbage.
- Runs at the display refresh rate (tested at 164 Hz), ~1 ms to appear.
- Works over elevated (admin) windows, other users' windows, Start/search, and windowed /
  borderless / FSO games (with the signed uiAccess install).
- Costs nothing while idle: no capture, no overlay, no mouse hook until Ctrl+Alt is held — games
  keep their independent-flip fast path.
- Ctrl+Alt+<other key> shortcuts do not flash the lens.
- Rounded lens with border and soft shadow; size, shape, zoom steps, smoothing etc. configurable.

Known limits: no lens on the secure desktop (UAC prompt, Ctrl+Alt+Del, lock screen), DRM-protected
video shows black inside the lens, legacy true-exclusive-fullscreen games are not covered. The
lens is invisible to screenshots and screen recordings by design (otherwise it would magnify
itself).

## Requirements

Windows 10 2004+ or Windows 11, a D3D11 GPU. No runtime dependencies.

## Install

Signed uiAccess build into `Program Files`, with autostart:

```bash
scripts/deploy.sh local          # this PC, one UAC prompt
scripts/deploy.sh <ssh-host>     # a Windows box reachable over OpenSSH (elevated PowerShell)
```

One-time before the first deploy: `scripts/windows/new-signing-cert.ps1` on the build PC (creates
a self-signed code-signing certificate; `install.ps1` trusts it on the target). Details and
uninstall: [docs/deploy.md](docs/deploy.md).

Quit from the tray icon.

## Configuration

`%APPDATA%\best_loupe\config.toml`, created with defaults on first run; restart to apply. All
keys are documented in [config.example.toml](config.example.toml). Log:
`%LOCALAPPDATA%\best_loupe\best_loupe.log`.

## Building

Rust (MSVC toolchain) + Visual Studio Build Tools + Windows SDK. Developed in WSL, built on the
Windows host: `scripts/win-build.sh` (build), `scripts/win-run.sh` (run a dev build). See
[docs/build.md](docs/build.md).

## Docs

- [docs/design.md](docs/design.md) — architecture, render pipeline, roadmap
- [docs/research-approaches.md](docs/research-approaches.md) — why this approach (vs Magnification
  API, DWM injection, drivers), existing tools
- [docs/deploy.md](docs/deploy.md) — signing, uiAccess, install
- [CHANGELOG.md](CHANGELOG.md)

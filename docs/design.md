# Design — best_loupe

Hold-to-zoom lens magnifier for Windows 10 2004+ / Windows 11 (variant **A** from
[research-approaches.md](research-approaches.md): DWM output capture + topmost click-through
overlay rendered by a shader).

## UX

- Hold the hotkey (**Ctrl+Alt** by default; any non-empty combination of Ctrl / Alt / Shift) →
  a rounded lens appears around the cursor and follows it. Win is deliberately not offered:
  releasing it alone opens Start.
- **Mouse wheel** while held → zoom in/out (multiplicative steps, smooth animation). The wheel
  event is swallowed so the app below does not scroll/zoom.
- Release any hotkey modifier → lens disappears instantly.
- Pressing any other key — including a modifier that is not part of the hotkey (Shift with
  Ctrl+Alt) — while the hotkey is held "taints" the chord: the lens hides and stays hidden until a
  hotkey modifier is released. This keeps shortcuts like Ctrl+Alt+Del / Ctrl+Alt+T from flashing
  the lens. A short activation delay (`activation_delay_ms`) further hides it from fast
  shortcuts; a wheel event during the delay activates immediately.
- Zoom level is remembered between activations (in memory).
- Tray icon: left click → settings; right click → Settings / About / Download update / Exit.
  Starting the exe again while it runs opens the settings of the running instance.

## Settings window

`settings.rs` — one native Win32 window (no GUI framework) with a tab control: **Lens** (size,
rounding, border width/colour, shadow), **Zoom** (start/max zoom, wheel step, animation,
smoothing), **General** (hotkey modifiers, activation delay, fade-in, autostart, update checks,
open config folder, reset) and **About** (icon, version + build, update status, check now /
download, project page, log).

- Every control applies immediately via `config::update` (sanitize → bump generation → save
  `config.toml` → push the hotkey mask into `input`); the render thread re-reads the config when
  the generation changes, so the next lens already uses the new values.
- Laid out in DIPs × monitor DPI × `UI_SCALE` (1.5): the UI is deliberately larger than standard
  dialogs. `WM_DPICHANGED` rebuilds all controls for the new DPI.
- Strings in `i18n.rs`: Russian when the Windows UI language is Russian, else English.
- Common Controls v6 via the manifest (themed controls).

## Update check

`update.rs`, own thread; see [release.md](release.md#update-check-in-the-app). Notifies the tray
(balloon) and the settings window via a posted `WM_UPDATE_STATUS`.

## Magnification mapping

The lens is a "zoom about the cursor": lens pixel `p` shows desktop pixel
`s = c + (p - c) / zoom`, where `c` is the cursor. The desktop point under the cursor stays under
the cursor, and as long as both `p` and `c` are on the monitor `s` is on the monitor too — so the
lens rect is simply clamped to the monitor near edges and the source never goes out of bounds.

The real (hardware) cursor is composed by DWM on top of the overlay, i.e. it stays visible
unmagnified at the lens center. Desktop Duplication frames do not contain the cursor.

## Threads

| Thread | Job |
| --- | --- |
| main | config, logging, single-instance mutex, tray icon + settings window message loop |
| input | `WH_KEYBOARD_LL` always; `WH_MOUSE_LL` installed **only while the chord is held** (zero mouse-latency overhead otherwise). Updates atomics, signals the render thread event. |
| update | sleeps; checks GitHub releases ~30 s after start, then daily or on demand |
| render | owns D3D11 device, overlay window, DirectComposition, swapchain and the duplication. Sleeps on the event when idle, runs a vsync-paced loop while active. |

Shared state is lock-free atomics (`input.rs`). Hook procs only touch atomics and `SetEvent`, so
they never risk `LowLevelHooksTimeout`.

Safety net: while active, the render thread also checks `GetAsyncKeyState` for the hotkey
modifiers each frame; if one is up it deactivates. This covers key-ups the hook never saw (e.g. foreground is
an elevated window and we are not uiAccess yet).

## Render pipeline (per frame while active)

1. Wait on the swapchain frame-latency waitable object (`SetMaximumFrameLatency(1)`), pumping
   window messages.
2. `AcquireNextFrame(0)`: if a new desktop image arrived (`LastPresentTime != 0`), `CopyResource`
   into our own SRV texture, `ReleaseFrame`. Otherwise keep the last copy (DDA only delivers frames
   on change, so a static desktop costs nothing).
3. Sample `GetCursorPos` as late as possible, compute lens rect / constants.
4. Clear the full-monitor composition swapchain to transparent, draw one quad (vertex shader
   builds it from `SV_VertexID`), pixel shader: rounded-rect SDF mask, border, soft shadow,
   bilinear (or point) sample of the desktop copy, premultiplied alpha output.
5. `Present(1, 0)`.

The swapchain covers the whole monitor and the window never moves: moving a window
(`SetWindowPos`) or a DComp visual per frame can desync position vs content by a frame. Clearing
and composing a mostly transparent 2560×1440 surface is negligible on any discrete GPU.

Expected latency: lens position matches the cursor sampled ~1 frame before scanout; lens content
is ≤1 extra frame behind the real desktop.

## Show / hide

- Activation: find the DXGI output under the cursor (rebuild the lens if the output changed),
  create the duplication, wait up to ~50 ms for its first full frame, render the first frame,
  **then** show the window (no stale-frame flash). Re-assert `HWND_TOPMOST` on each show.
- Deactivation: present a transparent frame, hide the window, release the duplication.
  The overlay and the capture exist only while the key is held, so games keep independent
  flip / MPO the rest of the time (research §A).
- Overlay window styles: `WS_POPUP`, `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE |
  WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP`; `SetWindowDisplayAffinity(
  WDA_EXCLUDEFROMCAPTURE)` set before the first show so the lens never captures itself.

## Error handling

- `DXGI_ERROR_ACCESS_LOST` (mode change, secure desktop, fullscreen transition): drop the
  duplication and recreate it on the next frame; if the output geometry changed, rebuild the lens.
  While it cannot be recreated (UAC / lock screen) the lens just shows the last good frame.
- `DXGI_ERROR_DEVICE_REMOVED/RESET`: rebuild everything.
- `DuplicateOutput1` → `DXGI_ERROR_UNSUPPORTED` (hybrid GPU laptops: output not on our adapter):
  logged; Windows.Graphics.Capture fallback is a TODO.

## Configuration

`%APPDATA%\best_loupe\config.toml` (written with defaults on first run), edited live by the
settings window. See `config.example.toml` for all keys. Values are clamped to sane ranges on
load and on every change.

## Elevation / z-order

To work over elevated windows and above Start / search, the exe needs `uiAccess="true"` in its
manifest, an Authenticode signature trusted by the machine (self-signed cert imported into
Trusted Root) and installation under `Program Files`. Implemented behind the `uiaccess` cargo
feature + `install.ps1`, which signs with a machine-local certificate; see [deploy.md](deploy.md). Dev builds (no feature) run from
anywhere with `uiAccess="false"` and do not work while an elevated window is focused.

## Known limits (by design of variant A)

- Secure desktop (UAC prompt, Ctrl+Alt+Del, lock screen): not reachable.
- HW-DRM protected video: black inside the lens.
- Legacy *true* exclusive fullscreen (FSO disabled): no topmost window possible; capture is lost.
  FSO / borderless / windowed games work; they lose iflip only while the lens is visible.

## Roadmap

1. ~~MVP: single-output-at-a-time lens, Ctrl+Alt hold, wheel zoom, tray exit, config.~~ v0.1.0.
2. ~~uiAccess manifest + self-sign + installer script; autostart.~~ v0.1.0.
2a. ~~Tray icon, settings/About window, configurable hotkey, GitHub releases + update check.~~ v0.2.0.
3. Magnified cursor drawn from `GetFramePointerShape`; lens spanning two monitors.
4. WGC fallback (hybrid GPUs), HDR (FP16 duplication + scRGB swapchain), rotated outputs.
5. Optional variant B: whole-screen zoom while held via `MagSetFullscreenTransform`.

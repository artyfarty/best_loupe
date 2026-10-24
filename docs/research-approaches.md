# Research: approaches to a fast lens magnifier on Windows 11

Date: 2026-10-24. Target UX (macOS Zoom-like): while a hotkey is held, a small region around the
cursor is magnified and follows the cursor; the scroll wheel changes zoom while held; releasing the
key hides the lens.

## Why the built-in Windows Magnifier lens lags

- Lens/docked modes use the windowed magnifier control (`WC_MAGNIFIER` + `MagSetWindowSource`):
  source rect is copied to an offscreen bitmap, scaled, painted into a layered window, refreshed on a
  timer (`InvalidateRect`) — not vsync/cursor driven, so content "swims" behind the cursor.
- Widely reported to be worse after 24H2 (lens "unusable", docked ~20 fps).
- Full-screen mode does not lag: it is applied inside DWM as a transform.

## Options considered

| Option | Speed | Coverage | Fragility | Verdict |
| --- | --- | --- | --- | --- |
| A. Capture (DDA / WGC) + topmost click-through D3D11 overlay + shader | lens position same-frame, content +~1 frame | elevated windows, other users, Start (with uiAccess), FSO/borderless games, HDR | low (documented APIs) | **chosen** |
| B. `MagSetFullscreenTransform` | perfect (DWM transform) | everything DWM composes | low | full-screen zoom only, not a lens; possible secondary mode |
| C. Inject into `dwm.exe`, hook `COverlayContext::Present` (dwm_lut style), post-process shader | best (zero copy) | everything composed | very high: offsets per build, can crash DWM | experiment only |
| D. GPU vendor / ReShade / Freestyle | — | per-game only | — | dead end |
| E. Indirect Display Driver / kernel | mediocre | — | high | not worth it |

### A. Capture + overlay — details and gotchas

- Both DDA (`IDXGIOutputDuplication`) and WGC capture the DWM-composed output, so window
  integrity level does not matter (elevated / other users' windows are captured).
- Independent-flip / MPO content bypasses composition; any visible window on that output (even
  1x1 at alpha 1/255, alpha 0 is culled) forces composition again — our overlay itself does that
  while shown. Consequence: game loses iflip/MPO only while the lens is visible → show overlay and
  run capture **only while the key is held**.
- Not capturable: secure desktop (UAC, Ctrl+Alt+Del — separate desktop, only system Magnifier via
  ATBroker works there), hardware DRM content (black), legacy exclusive fullscreen.
- DDA: frames only on change, dirty/move rects, pointer shape separately
  (`GetFramePointerShape`), only on the adapter driving the output (hybrid laptops →
  `DXGI_ERROR_UNSUPPORTED`). HDR needs `IDXGIOutput5::DuplicateOutput1` with
  `R16G16B16A16_FLOAT`. First frame arrives immediately (good for hold-to-show).
- WGC: cross-adapter, `IsCursorCaptureEnabled(false)`, `IsBorderRequired(false)` (Win11),
  `MinUpdateInterval` (24H2+, default throttles — set ~1–4 ms). Starts a few frames slower.
- 24H2 notes (Lossless Scaling patch notes): DDA strongly tied to MPO support; WGC without MPO with
  HW cursor + VRR disables iflip globally.
- Self-exclusion: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` (Win10 2004+), set before
  first show. 24H2 regression made DDA wake on every repaint of an excluded window (fixed in
  KB5046617) — guard against spurious frames anyway.
- Z-order: plain `WS_EX_TOPMOST` loses to Start/search/notification center. `uiAccess="true"`
  manifest puts the window in `ZBID_UIACCESS`; requires Authenticode signature (self-signed cert
  in Trusted Root is fine) and install in a secure location (Program Files). `CreateWindowInBand`
  with higher bands requires Microsoft signature — not an option. (Unverified details, check.)
- Input: `WH_MOUSE_LL` / `WH_KEYBOARD_LL` from medium IL do not see input destined to elevated
  windows (UIPI); uiAccess or elevation fixes it. Swallowing the wheel requires `WH_MOUSE_LL`
  (return non-zero). Hook on its own thread, respond fast (`LowLevelHooksTimeout` silently unhooks).
  `RegisterHotKey` cannot do modifier-only holds.
- Rendering: one click-through window per monitor
  (`WS_EX_LAYERED|WS_EX_TRANSPARENT|WS_EX_NOACTIVATE|WS_EX_TOOLWINDOW|WS_EX_NOREDIRECTIONBITMAP`),
  `CreateSwapChainForComposition` flip-model + premultiplied alpha + DirectComposition,
  `SetMaximumFrameLatency(1)` + waitable object. Do **not** move the window per frame
  (`SetWindowPos` lags); keep it full-monitor and draw the lens at the freshly sampled cursor
  position in the shader. Per-Monitor DPI Aware V2, physical pixels, handle output rotation.
  Lens crossing monitors needs two captures.
- Cursor: hardware cursor is drawn by DWM on top of our overlay at the real position (= lens
  center), which is usually desired; optionally draw a magnified cursor from the pointer shape.

### B. Magnification API full-screen

- `MagSetFullscreenTransform` (scale 1..4096, offsets in unscaled coords) and
  `MagSetFullscreenColorEffect` are applied by DWM. `MagSetInputTransform` needs uiAccess.
- Undocumented `SetFullscreenMagnifierOffsetsDWMUpdated(TRUE,0,0)` lets DWM follow the cursor
  in the same frame (project Maxaubert/Wind): ~0.2 px offset vs 18–96 px with timer updates.
- Input transform is system-global; if our process crashes it stays applied.

### C. DWM injection (dwm_lut and forks)

- `CreateRemoteThread` inject, MinHook `COverlayContext::Present` in `dwmcore.dll`, draw into the
  backbuffer obtained via the `IOverlaySwapChain` vtable. Requires admin; autostart via scheduled
  task. Offsets/signatures per build; forks: lauralex/dwm_lut, GianSegugio/dwm_lut_is_back,
  0x401gg/dwm_lut-windows-25h2. Must disable MPO/DirectFlip (`OverlayTestMode=5`, hook
  `IsCandidateDirectFlipCompatible`). Conflicts with other DWM mods crash DWM.

## Existing tools (none match the UX)

- Windows Magnifier: lens mode, no hold-to-zoom. Registry `HKCU\Software\Microsoft\ScreenMagnifier`
  (`UseBitmapSmoothing=0` sometimes helps).
- ZoomIt (PowerToys, MIT): Live Zoom is full-screen via Magnification API; WGC for recording.
- PowerToys lens requests (#34841, #24026, ...) closed as "built into Windows".
- sergiiopm/loupe (MIT, C#): closest UX (Ctrl+Alt+wheel), GDI `StretchBlt` capture, closes on 1x/Esc
  not on key release, excluded via `WDA_EXCLUDEFROMCAPTURE`.
- almakarem/Magnifier (MIT, C++): Mag API lens with `DwmFlush` vsync pacing.
- mertemr/BetterMagnifier (Apache-2.0, C++): D3D11 + DDA full-screen zoom, own cursor rendering.
- xxyu-1145/screen-magnifier: D3D11 + DDA + DirectComposition, click-through region magnifier.
- Magpie (Blinue/Magpie): window upscaler; good reference for capture methods (wiki compares them).
- robmikh/Win32CaptureSample: reference WGC/DDA sample.
- Commercial ZoomText / SuperNova: lens exists, no hold-to-zoom; heavy, paid.

## Decision

Option A in C++ (Win32, D3D11, DirectComposition, DDA primary / WGC fallback), uiAccess-signed and
installed to Program Files. Optional option B as a secondary "zoom whole screen while held" mode.
Option C only as a side experiment.

# Build & run

The code lives in WSL; it is built and run on the Windows host.

## Toolchain (Windows host)

- Rust via rustup, MSVC host (`x86_64-pc-windows-msvc`), pinned to an explicit stable version
  (installed: 1.99.1, `--profile minimal`).
- MSVC linker from Visual Studio Build Tools (2019 / 14.29 is enough) + a Windows 10 SDK.
- No other SDK tools are needed: shaders are compiled at runtime with `D3DCompile`
  (`d3dcompiler_47.dll` ships with Windows), the manifest is embedded by the MSVC linker via
  `build.rs` (`/MANIFEST:EMBED /MANIFESTINPUT:app.manifest`).

## Scripts

- `scripts/win-build.sh [cargo args]` — rsyncs the tree (minus `target/`, `.git/`, `.claude/`) to
  `C:\Users\<user>\dev\best_loupe` and runs `cargo.exe` there (default `build --release`). Cargo on
  Windows does not cope well with `\\wsl.localhost` paths, and building from NTFS is faster.
  `Cargo.lock` is copied back so it can be committed.
- `scripts/win-run.sh` — kills a running instance and starts the release exe detached (PowerShell
  `Start-Process`; `cmd /c start` hangs from WSL). `scripts/win-run.sh --stop` only kills it.
- Log: `%LOCALAPPDATA%\best_loupe\best_loupe.log` (truncated on each start).
- Debug builds have a console; release builds use the windows subsystem.
- The CRT is linked statically (`.cargo/config.toml`, `+crt-static`) so the exe runs without the
  VC++ redistributable.
- Deployable (signed, uiAccess) builds: `scripts/deploy.sh`, see [deploy.md](deploy.md).

## Testing notes

- The overlay is excluded from capture (`WDA_EXCLUDEFROMCAPTURE`), so screenshots/recordings
  never show the lens — verify visually on the physical screen, or via the log.
- A synthetic smoke test (Ctrl+Alt hold + one wheel notch) can be injected with `keybd_event` /
  `mouse_event` from PowerShell; LL hooks see injected input.

## Dependency policy

- Every crate version must be at least 3 days old at the time it is added. Cargo has no
  declarative age gate, so direct dependencies are pinned with `=x.y.z` and `Cargo.lock` is
  committed; when adding/updating crates, check the publish date of every changed lock entry
  (crates.io API `/api/v1/crates/<name>/<version>` → `created_at`).

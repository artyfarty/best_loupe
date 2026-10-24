# Releases & CI

Source of truth: `Cargo.toml` `version` + annotated tag `vX.Y.Z` (see CLAUDE.md → Versioning).

## CI (`.github/workflows/build.yml`)

Runs on every push to `main`, every PR and every `v*` tag, on `windows-latest`:

1. Rust toolchain pinned (`RUST_TOOLCHAIN`), `cargo test --release --locked`.
2. Portable build (`--target-dir target/portable`) and uiAccess build (`--features uiaccess
   --target-dir target/dist`).
3. Inno Setup downloaded from its GitHub release, pinned by version **and SHA256**, installed
   per-user; `ISCC /DAppVersion=<Cargo version>` → `dist/best_loupe-vX.Y.Z-setup.exe`.
4. `dist/` = setup, `best_loupe-vX.Y.Z-portable-x64.exe`, `SHA256SUMS` → uploaded as a workflow
   artifact.
5. On a tag: the job fails if the tag ≠ `v<Cargo version>`; otherwise `gh release create` with
   those three files and the version's `CHANGELOG.md` section as notes.

Third-party actions are pinned to commit SHAs (checked to be ≥ 3 days old, like crates).
`fetch-depth: 0` so `git describe` (burned into the exe) sees the tags.

## Cutting a release

1. Bump `version` in `Cargo.toml`, add a `## vX.Y.Z — date` section to `CHANGELOG.md`, update docs.
2. Commit, `git tag -a vX.Y.Z -m "best_loupe vX.Y.Z"`.
3. Push commit + tag to `origin` (Gitea) and `github` (YubiKey touch). CI publishes the release.

Locally, `scripts/win-installer.sh` builds the same installer on the Windows host (Inno Setup 6
per-user install expected at `%LOCALAPPDATA%\Programs\InnoSetup6`, or `ISCC=` override).

## Release artifacts

| File | What |
| --- | --- |
| `best_loupe-vX.Y.Z-setup.exe` | Inno Setup installer (en/ru): Program Files, machine-local uiAccess signing ([deploy.md](deploy.md)), optional autostart, Start-menu entry, uninstaller (also removes the cert). The installer itself is not code-signed → SmartScreen "unknown publisher" warning. |
| `best_loupe-vX.Y.Z-portable-x64.exe` | No install, runs from anywhere; no elevated-window / Start-menu support (no uiAccess). |
| `SHA256SUMS` | Checksums of the above. |

## Update check (in the app)

`src/update.rs`: ~30 s after start and then every 24 h (if `check_updates` is on; "Check now" in
About forces it) it GETs `https://api.github.com/repos/<update_repo>/releases/latest` via WinHTTP
(system proxy, 10–15 s timeouts, `User-Agent: best_loupe/<version>`). `tag_name` newer than the
running version (numeric `major.minor.patch` compare) → status "available": a tray balloon once
per version per run (click → release page), a "Download update" tray menu item and a Download
button in About. 404 (no releases yet) = up to date. Nothing is downloaded or installed
automatically. Unauthenticated GitHub API limit is 60 requests/h per IP — far above one per day.

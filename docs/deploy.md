# Deploy / install (uiAccess build)

To work over elevated windows, above Start/search and with the hotkey while an elevated app is
focused, the exe needs `uiAccess="true"`. Windows only honours it when the binary has a
**trusted Authenticode signature** and lives in a **secure location** (`Program Files`), and it
must be started via ShellExecute (not plain `CreateProcess`). Otherwise the process does not
start at all (`0xC0000361` / "A referral was returned from the server") — so a running
Program Files instance proves uiAccess was granted.

## Signing model: machine-local, no shared publisher cert

There is no project-wide signing certificate. After the exe is copied to
`%ProgramFiles%\best_loupe\` (by `setup.exe` or `install.ps1`), `scripts/windows/uiaccess-sign.ps1`
does what AutoHotkey's UIA mode does:

1. remove any previous best_loupe cert;
2. create a fresh self-signed code-signing cert `CN=best_loupe local uiAccess signing (<computer>)`
   in `LocalMachine\My` (RSA 3072, EKU Code Signing only, not a CA);
3. sign the installed exe with it (SHA256);
4. add the **public** part to `LocalMachine\Root` + `TrustedPublisher`;
5. delete the cert *with its private key* from `LocalMachine\My`.

Nothing else can ever be signed with that cert, and nobody has to trust a key that exists on
another machine. Each (re)install repeats this (also removing the shared `CN=best_loupe code
signing` cert used by v0.1.0 deploys); `-Remove` (uninstall) only removes.

## Pieces

| File | Role |
| --- | --- |
| `Cargo.toml` feature `uiaccess` | `build.rs` writes `uiAccess="true"` into the embedded manifest. Builds without it keep `false` and run from anywhere (the portable release). |
| `build.rs` | Embeds `app.manifest` via `/MANIFEST:EMBED /MANIFESTINPUT` plus `/MANIFESTUAC:NO` (otherwise the linker adds its own `uiAccess='false'` fragment and `mt.exe` fails with c1010001). Also icon + VERSIONINFO via `rc.exe`. |
| `installer/best_loupe.iss` | Inno Setup installer for end users (see [release.md](release.md)): fixed `{autopf}\best_loupe` dir, runs `uiaccess-sign.ps1` after copying, optional autostart task (HKCU Run), Start-menu shortcut, launches via `shellexec runasoriginaluser` (non-elevated ShellExecute), stops a running instance before install/uninstall, uninstall runs `uiaccess-sign.ps1 -Remove`. |
| `scripts/windows/uiaccess-sign.ps1` | The signing step above; installed next to the exe so the uninstaller can use it. |
| `scripts/windows/install.ps1` | Headless script installer for dev deploys (e.g. over SSH): copy, sign, autostart (`-NoAutostart`), launch in the user's interactive session (`-NoLaunch`), `-Uninstall`. |
| `scripts/deploy.sh local\|<ssh-host> [install args]` | Dev deploy: builds with `--features uiaccess --target-dir target/dist` (a running dev build in `target\release` does not lock it), copies exe + `install.ps1` + `uiaccess-sign.ps1`, runs it — locally through one UAC prompt, remotely over SSH (OpenSSH whose default shell is an elevated PowerShell). |

## Launch details

- SSH sessions and elevated shells are not the user's desktop session. `install.ps1` registers a
  one-shot scheduled task (`LogonType Interactive`, `RunLevel Limited`, user = target user) whose
  action is `explorer.exe "<exe>"` — explorer ShellExecutes it — runs it and deletes the task.
- Autostart uses `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`; Explorer launches those
  via ShellExecute as well. Both installers write it into the HKCU of the *elevated* account; if
  a standard user elevates with a different admin account, autostart lands in that admin's HKCU —
  the "Start with Windows" checkbox in settings fixes it from the right account.
- Starting the exe again while it runs opens the running instance's settings window
  (`--about` → About tab). The tray window allows that message through UIPI
  (`ChangeWindowMessageFilterEx`), since the running copy is uiAccess.

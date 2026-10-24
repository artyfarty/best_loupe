# Deploy (signed uiAccess build)

To work over elevated windows, above Start/search and with hotkeys while an elevated app is
focused, the exe needs `uiAccess="true"`. Windows only honours it when the binary has a
**trusted Authenticode signature** and lives in a **secure location** (`Program Files`), and it
must be started via ShellExecute (not plain `CreateProcess`).

## Pieces

| File | Role |
| --- | --- |
| `Cargo.toml` feature `uiaccess` | `build.rs` writes `uiAccess="true"` into the embedded manifest. Dev builds (no feature) keep `false` so they run from `target\`. |
| `build.rs` | Embeds `app.manifest` via `/MANIFEST:EMBED /MANIFESTINPUT`, plus `/MANIFESTUAC:NO` — otherwise the linker adds its own UAC fragment (`uiAccess='false'`) and `mt.exe` fails with c1010001. |
| `scripts/windows/new-signing-cert.ps1` | One-time, on the build PC: self-signed code-signing cert `CN=best_loupe code signing` (RSA 3072, 10 years, EKU Code Signing only, not a CA), **non-exportable** key in `Cert:\CurrentUser\My`. |
| `scripts/windows/sign.ps1` | `Set-AuthenticodeSignature -HashAlgorithm SHA256`, exports the public `.cer`. Status `UnknownError` on the build PC is expected (its own root is not trusted there). |
| `scripts/windows/install.ps1` | Elevated, on the target: imports the `.cer` into `LocalMachine\Root` + `TrustedPublisher`, verifies the exe signature is `Valid`, stops the running instance, copies to `%ProgramFiles%\best_loupe\`, sets `HKCU\...\Run\best_loupe` (autostart), launches it in the user's interactive session. `-NoAutostart`, `-NoLaunch`, `-Uninstall` (also removes the cert). |
| `scripts/deploy.sh local\|<ssh-host> [install args]` | Builds with `--features uiaccess --target-dir target/dist` (so a running dev build in `target\release` does not lock the output), signs, stages, runs `install.ps1` — locally through one UAC prompt, remotely over SSH (OpenSSH with an elevated PowerShell default shell). |

## Launch details

- SSH sessions and elevated shells are not the user's desktop session. `install.ps1` registers a
  one-shot scheduled task (`LogonType Interactive`, `RunLevel Limited`, user = target user) whose
  action is `explorer.exe "<exe>"` — explorer ShellExecutes it, which is what uiAccess binaries
  need — runs it, then deletes the task.
- Autostart uses `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (Explorer launches those
  via ShellExecute as well).
- If uiAccess requirements are not met, the process does not start at all (ERROR 0xC0000361 /
  "A referral was returned from the server") — so a running Program Files instance proves
  uiAccess was granted.

## Rollout log

- 2026-10-24: first uiAccess deploy to the second home PC (Windows 10 22H2, 19045) over SSH —
  cert imported, installed under `%ProgramFiles%\best_loupe`, autostart on, running in session 1.

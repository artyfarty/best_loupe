#!/usr/bin/env bash
# Start the last release build on the Windows desktop (detached), replacing a running instance.
# scripts/win-run.sh --stop only stops it.
set -euo pipefail
WIN_USER="$(cmd.exe /c 'echo %USERNAME%' 2>/dev/null | tr -d '\r')"
cd /mnt/c
EXE_W="C:\\Users\\${WIN_USER}\\dev\\best_loupe\\target\\release\\best_loupe.exe"
# Ask the running instance to quit (an installed uiAccess copy cannot be killed from a normal
# process), then force-stop whatever is left.
powershell.exe -NoProfile -Command "if (Get-Process best_loupe -ErrorAction SilentlyContinue) { Start-Process -Wait '${EXE_W}' -ArgumentList '--quit'; Start-Sleep -Milliseconds 700 }; Get-Process best_loupe -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; if (Get-Process best_loupe -ErrorAction SilentlyContinue) { Write-Host 'WARNING: an instance is still running (installed copy older than v0.3.0?): exit it from the tray' }"
[ "${1:-}" = "--stop" ] && exit 0
powershell.exe -NoProfile -Command "Start-Process -FilePath '${EXE_W}'"

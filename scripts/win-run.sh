#!/usr/bin/env bash
# Start the last release build on the Windows desktop (detached). Stop with the tray icon -> Exit,
# or scripts/win-run.sh --stop.
set -euo pipefail
WIN_USER="$(cmd.exe /c 'echo %USERNAME%' 2>/dev/null | tr -d '\r')"
cd /mnt/c
powershell.exe -NoProfile -Command "Get-Process best_loupe -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep -Milliseconds 300"
[ "${1:-}" = "--stop" ] && exit 0
powershell.exe -NoProfile -Command "Start-Process -FilePath 'C:\\Users\\${WIN_USER}\\dev\\best_loupe\\target\\release\\best_loupe.exe'"

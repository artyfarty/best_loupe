#!/usr/bin/env bash
# Build on the Windows host from WSL: mirror sources to a Windows-local dir and run cargo.exe there.
# Usage: scripts/win-build.sh [cargo args...]   (default: build --release)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WIN_USER="$(cmd.exe /c 'echo %USERNAME%' 2>/dev/null | tr -d '\r')"
WIN_DIR="/mnt/c/Users/${WIN_USER}/dev/best_loupe"
WIN_DIR_W="C:\\Users\\${WIN_USER}\\dev\\best_loupe"
CARGO_W="C:\\Users\\${WIN_USER}\\.cargo\\bin\\cargo.exe"

mkdir -p "$WIN_DIR"
rsync -a --delete --exclude target --exclude .git --exclude .claude "$ROOT/" "$WIN_DIR/"
# The mirror has no .git: hand the version string to build.rs (burned into the exe).
git -C "$ROOT" describe --tags --always --dirty > "$WIN_DIR/.build-describe" 2>/dev/null || rm -f "$WIN_DIR/.build-describe"
[ $# -eq 0 ] && set -- build --release
cd /mnt/c
# A dev instance running out of target\ locks the exe; stop it (installed copies are left alone).
powershell.exe -NoProfile -Command "Get-Process best_loupe -ErrorAction SilentlyContinue | Where-Object { \$_.Path -like '${WIN_DIR_W}\target\*' } | Stop-Process -Force" || true
cmd.exe /c "cd /d ${WIN_DIR_W} && ${CARGO_W} $*" 2>&1 | tr -d '\r'
status=${PIPESTATUS[0]}
# Bring the lockfile back so it can be committed.
cp "$WIN_DIR/Cargo.lock" "$ROOT/Cargo.lock" 2>/dev/null || true
exit "$status"

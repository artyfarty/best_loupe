#!/usr/bin/env bash
# Build the uiAccess exe and the Inno Setup installer on the Windows host (local counterpart of
# the GitHub Actions release job). Output: C:\Users\<user>\dev\best_loupe\dist\best_loupe-vX.Y.Z-setup.exe
# Needs Inno Setup 6 (ISCC.exe); override its location with ISCC=<windows path>.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WIN_USER="$(cmd.exe /c 'echo %USERNAME%' 2>/dev/null | tr -d '\r')"
ISCC="${ISCC:-C:\\Users\\${WIN_USER}\\AppData\\Local\\Programs\\InnoSetup6\\ISCC.exe}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"

"$ROOT/scripts/win-build.sh" build --release --features uiaccess --target-dir target/dist
cd /mnt/c
cmd.exe /c "cd /d C:\\Users\\${WIN_USER}\\dev\\best_loupe\\installer && ${ISCC} /Q /DAppVersion=${VERSION} best_loupe.iss" 2>&1 | tr -d '\r'
ls -la "/mnt/c/Users/${WIN_USER}/dev/best_loupe/dist/"

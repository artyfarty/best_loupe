#!/usr/bin/env bash
# Build a uiAccess release and install it on a Windows machine (install.ps1 signs it there with a
# machine-local certificate, see docs/deploy.md).
#   scripts/deploy.sh local          # this PC (pops one UAC prompt)
#   scripts/deploy.sh <ssh-host>     # remote Windows box with an elevated OpenSSH PowerShell
# Extra args are passed to install.ps1 (e.g. -NoAutostart, -NoLaunch).
set -euo pipefail
TARGET="${1:?usage: deploy.sh local|<ssh-host> [install.ps1 args]}"
shift
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WIN_USER="$(cmd.exe /c 'echo %USERNAME%' 2>/dev/null | tr -d '\r')"
STAGE="/mnt/c/Users/${WIN_USER}/dev/best_loupe-dist"
STAGE_W="C:\\Users\\${WIN_USER}\\dev\\best_loupe-dist"

# Separate target dir: a running dev build in target\release must not block the deploy build.
"$ROOT/scripts/win-build.sh" build --release --features uiaccess --target-dir target/dist

rm -rf "$STAGE" && mkdir -p "$STAGE"
cp "/mnt/c/Users/${WIN_USER}/dev/best_loupe/target/dist/release/best_loupe.exe" "$STAGE/"
cp "$ROOT/scripts/windows/install.ps1" "$ROOT/scripts/windows/uiaccess-sign.ps1" "$STAGE/"

if [ "$TARGET" = local ]; then
    LOG_W="${STAGE_W}\\install.log"
    echo ">>> confirm the UAC prompt on this PC"
    cd /mnt/c
    powershell.exe -NoProfile -Command "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-Command','& ''${STAGE_W}\\install.ps1'' -SourceDir ''${STAGE_W}'' -User ${WIN_USER} $* *> ''${LOG_W}'''"
    tr -d '\r' < "$STAGE/install.log"
else
    REMOTE_TMP="$(ssh -o BatchMode=yes "$TARGET" '$env:TEMP' | tr -d '\r')\\best_loupe-dist"
    ssh -o BatchMode=yes "$TARGET" "New-Item -ItemType Directory -Force -Path '$REMOTE_TMP' | Out-Null"
    REMOTE_SCP="/$(echo "$REMOTE_TMP" | tr '\\' '/')"
    scp -q "$STAGE"/best_loupe.exe "$STAGE"/install.ps1 "$STAGE"/uiaccess-sign.ps1 "$TARGET:$REMOTE_SCP/"
    ssh -o BatchMode=yes "$TARGET" "powershell -NoProfile -ExecutionPolicy Bypass -File '$REMOTE_TMP\\install.ps1' -SourceDir '$REMOTE_TMP' $*" | tr -d '\r'
fi

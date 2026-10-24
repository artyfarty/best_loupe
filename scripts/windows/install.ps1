# Script installer used by scripts/deploy.sh (headless, e.g. over SSH). End users get setup.exe,
# which does the same. Must run elevated.
#   install.ps1 [-SourceDir <dir with best_loupe.exe + uiaccess-sign.ps1>] [-User name] [-NoAutostart] [-NoLaunch]
#   install.ps1 -Uninstall
param(
    [string]$SourceDir = $PSScriptRoot,
    [string]$User = $env:USERNAME,
    [switch]$NoAutostart,
    [switch]$NoLaunch,
    [switch]$Uninstall
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'must run elevated' }

$dest = Join-Path $env:ProgramFiles 'best_loupe'
$target = Join-Path $dest 'best_loupe.exe'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$signer = Join-Path $SourceDir 'uiaccess-sign.ps1'

function Stop-Loupe {
    Get-Process best_loupe -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500
}

if ($Uninstall) {
    Stop-Loupe
    Remove-ItemProperty $runKey -Name best_loupe -ErrorAction SilentlyContinue
    Remove-Item $dest -Recurse -Force -ErrorAction SilentlyContinue
    & $signer -Remove
    'uninstalled'
    exit 0
}

$exe = Join-Path $SourceDir 'best_loupe.exe'
if (-not (Test-Path $exe)) { throw "not found: $exe" }

Stop-Loupe
New-Item -ItemType Directory -Force $dest | Out-Null
Copy-Item $exe $target -Force
"installed $target"

& $signer -Exe $target

if ($NoAutostart) {
    Remove-ItemProperty $runKey -Name best_loupe -ErrorAction SilentlyContinue
} else {
    Set-ItemProperty $runKey -Name best_loupe -Value "`"$target`""
    'autostart: HKCU Run'
}

if (-not $NoLaunch) {
    # Start in the user's interactive session (SSH / elevated shells are not it). Going through
    # explorer.exe makes it a ShellExecute launch, which is what uiAccess binaries require.
    $name = 'best_loupe_launch'
    $action = New-ScheduledTaskAction -Execute "$env:WINDIR\explorer.exe" -Argument "`"$target`""
    $taskPrincipal = New-ScheduledTaskPrincipal -UserId "$env:COMPUTERNAME\$User" -LogonType Interactive -RunLevel Limited
    Register-ScheduledTask -TaskName $name -Action $action -Principal $taskPrincipal -Force | Out-Null
    Start-ScheduledTask -TaskName $name
    Start-Sleep -Seconds 3
    Unregister-ScheduledTask -TaskName $name -Confirm:$false
    $p = Get-Process best_loupe -ErrorAction SilentlyContinue
    if ($p) { $p | ForEach-Object { "running: pid $($_.Id), session $($_.SessionId)" } } else { 'WARNING: not running after launch' }
}

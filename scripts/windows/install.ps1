# Installs (or uninstalls) a signed uiAccess build of best_loupe. Must run elevated.
#   install.ps1 -SourceDir <dir with best_loupe.exe + best_loupe-codesign.cer> [-User name] [-NoAutostart] [-NoLaunch]
#   install.ps1 -Uninstall
# uiAccess requirements: trusted Authenticode signature (our self-signed cert goes into
# LocalMachine Root + TrustedPublisher) and a secure location (Program Files).
param(
    [string]$SourceDir,
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

function Stop-Loupe {
    Get-Process best_loupe -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500
}

if ($Uninstall) {
    Stop-Loupe
    Remove-ItemProperty $runKey -Name best_loupe -ErrorAction SilentlyContinue
    Remove-Item $dest -Recurse -Force -ErrorAction SilentlyContinue
    foreach ($store in 'Root', 'TrustedPublisher') {
        Get-ChildItem "Cert:\LocalMachine\$store" | Where-Object Subject -eq 'CN=best_loupe code signing' |
            ForEach-Object { Remove-Item $_.PSPath; "removed cert from $store" }
    }
    'uninstalled'
    exit 0
}

if (-not $SourceDir) { throw '-SourceDir is required' }
$cer = Join-Path $SourceDir 'best_loupe-codesign.cer'
$exe = Join-Path $SourceDir 'best_loupe.exe'

$cert = New-Object Security.Cryptography.X509Certificates.X509Certificate2 $cer
foreach ($store in 'Root', 'TrustedPublisher') {
    $s = New-Object Security.Cryptography.X509Certificates.X509Store $store, 'LocalMachine'
    $s.Open('ReadWrite')
    if (-not ($s.Certificates | Where-Object Thumbprint -eq $cert.Thumbprint)) {
        $s.Add($cert)
        "cert $($cert.Thumbprint) added to LocalMachine\$store"
    }
    $s.Close()
}

$sig = Get-AuthenticodeSignature $exe
if ($sig.Status -ne 'Valid') { throw "exe signature not valid: $($sig.Status) $($sig.StatusMessage)" }
if ($sig.SignerCertificate.Thumbprint -ne $cert.Thumbprint) { throw 'exe is signed by a different cert' }

Stop-Loupe
New-Item -ItemType Directory -Force $dest | Out-Null
Copy-Item $exe $target -Force
"installed $target"

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

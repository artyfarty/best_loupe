# Makes an installed best_loupe.exe runnable with uiAccess on THIS machine. Must run elevated.
#   uiaccess-sign.ps1 -Exe "<Program Files>\best_loupe\best_loupe.exe"
#   uiaccess-sign.ps1 -Remove
#
# uiAccess needs a trusted Authenticode signature (plus a Program Files location). There is no
# shared publisher certificate: like AutoHotkey's UIA mode, this creates a certificate on this
# machine, signs the exe, trusts the public part (LocalMachine Root + TrustedPublisher) and
# deletes the private key right away, so nothing else can ever be signed with it. Every run
# replaces the previous certificate. Used by the setup.exe installer and by install.ps1.
param(
    [string]$Exe,
    [switch]$Remove
)
$ErrorActionPreference = 'Stop'

$subject = "CN=best_loupe local uiAccess signing ($env:COMPUTERNAME)"
# Also clean up the shared build-machine cert used by v0.1.0 deploys.
$known = @($subject, 'CN=best_loupe code signing')

foreach ($store in 'Root', 'TrustedPublisher', 'My') {
    Get-ChildItem "Cert:\LocalMachine\$store" | Where-Object { $known -contains $_.Subject } | ForEach-Object {
        Remove-Item $_.PSPath -DeleteKey -ErrorAction SilentlyContinue
        Remove-Item $_.PSPath -ErrorAction SilentlyContinue
        "removed cert $($_.Thumbprint) from LocalMachine\$store"
    }
}
if ($Remove) { exit 0 }
if (-not $Exe -or -not (Test-Path $Exe)) { throw "exe not found: $Exe" }

$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $subject `
    -CertStoreLocation Cert:\LocalMachine\My -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 `
    -NotAfter (Get-Date).AddYears(20)
try {
    $public = New-Object Security.Cryptography.X509Certificates.X509Certificate2(, $cert.RawData)
    foreach ($store in 'Root', 'TrustedPublisher') {
        $s = New-Object Security.Cryptography.X509Certificates.X509Store $store, 'LocalMachine'
        $s.Open('ReadWrite')
        $s.Add($public)
        $s.Close()
    }
    $sig = Set-AuthenticodeSignature -FilePath $Exe -Certificate $cert -HashAlgorithm SHA256
} finally {
    Remove-Item "Cert:\LocalMachine\My\$($cert.Thumbprint)" -DeleteKey
}
if ($sig.Status -ne 'Valid') { throw "signing failed: $($sig.Status) $($sig.StatusMessage)" }
"signed $Exe with machine-local cert $($cert.Thumbprint) (private key deleted)"

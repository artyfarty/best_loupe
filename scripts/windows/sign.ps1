# Signs an exe with the best_loupe code-signing cert and exports the public cert next to it.
param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$OutCer
)
$ErrorActionPreference = 'Stop'
$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Where-Object Subject -eq 'CN=best_loupe code signing' |
    Sort-Object NotAfter -Descending | Select-Object -First 1
if (-not $cert) { throw 'signing cert not found; run scripts\windows\new-signing-cert.ps1 first' }
$r = Set-AuthenticodeSignature -FilePath $Exe -Certificate $cert -HashAlgorithm SHA256
# 'UnknownError' = signed fine, but this machine does not trust the self-signed root (yet).
if ($r.Status -eq 'NotSigned' -or $r.SignerCertificate.Thumbprint -ne $cert.Thumbprint) {
    throw "signing failed: $($r.Status) $($r.StatusMessage)"
}
Export-Certificate -Cert $cert -FilePath $OutCer -Type CERT | Out-Null
"signed $Exe with $($cert.Thumbprint) (status here: $($r.Status))"

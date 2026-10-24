# Creates the self-signed code-signing certificate used to sign uiAccess builds.
# Runs on the build machine, no elevation needed. The private key is non-exportable and stays in
# Cert:\CurrentUser\My; only the public .cer is distributed (by install.ps1) to target machines.
$ErrorActionPreference = 'Stop'
$subject = 'CN=best_loupe code signing'
$existing = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object Subject -eq $subject
if ($existing) {
    "already exists: $($existing.Thumbprint) (valid until $($existing.NotAfter))"
    exit 0
}
$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $subject `
    -CertStoreLocation Cert:\CurrentUser\My -KeyExportPolicy NonExportable `
    -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 -NotAfter (Get-Date).AddYears(10)
"created: $($cert.Thumbprint) (valid until $($cert.NotAfter))"

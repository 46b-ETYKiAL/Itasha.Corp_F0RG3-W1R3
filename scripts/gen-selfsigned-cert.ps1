#!/usr/bin/env pwsh
# ============================================================================
# gen-selfsigned-cert.ps1 — free self-signed Windows code-signing cert
# ============================================================================
# Generates a self-signed Authenticode code-signing certificate (no cost, no
# CA). Use it to:
#   - Sign installers so an ENTERPRISE can allow-list the publisher via Group
#     Policy / Intune (import the exported .cer into Trusted Publishers).
#   - Give the binary a stable publisher identity in its Authenticode chain.
#
# IMPORTANT: a self-signed cert is NOT trusted by the public Windows
# SmartScreen reputation system — general (non-enterprise) users will still see
# a one-time warning. Warning-free public installs require a real OV/EV cert
# (a paid dependency; see docs/adr/0003-signing-posture.md). This script is the
# free tier, honestly scoped.
#
# Usage:  ./scripts/gen-selfsigned-cert.ps1 [-Subject "CN=Itasha.Corp"] [-ExportCer .\itasha-publisher.cer]
# Outputs: the certificate thumbprint (set it as WINDOWS_CERT_THUMBPRINT).
# ----------------------------------------------------------------------------
[CmdletBinding()]
param(
    [string]$Subject = "CN=Itasha.Corp, O=Itasha.Corp",
    [string]$ExportCer = "",
    [int]$Years = 3
)
$ErrorActionPreference = "Stop"

$cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $Subject `
    -KeyAlgorithm RSA -KeyLength 3072 `
    -HashAlgorithm SHA256 `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -NotAfter (Get-Date).AddYears($Years) `
    -KeyUsage DigitalSignature `
    -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3")

Write-Host "==> Self-signed code-signing cert created."
Write-Host "    Subject:    $($cert.Subject)"
Write-Host "    Thumbprint: $($cert.Thumbprint)"
Write-Host ""
Write-Host "Set this for signing (local or CI WINDOWS_CERT_THUMBPRINT secret):"
Write-Host "    `$env:WINDOWS_CERT_THUMBPRINT = '$($cert.Thumbprint)'"

if ($ExportCer) {
    Export-Certificate -Cert $cert -FilePath $ExportCer | Out-Null
    Write-Host ""
    Write-Host "Exported PUBLIC cert (safe to distribute for enterprise trust): $ExportCer"
    Write-Host "Enterprises import it into 'Trusted Publishers' via GPO to allow-list installs."
}
Write-Host ""
Write-Host "NOTE: self-signed != public trust. SmartScreen still warns general users."
Write-Host "      Warning-free public installs need a paid OV/EV cert (ADR-0003)."

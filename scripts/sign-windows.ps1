#!/usr/bin/env pwsh
# ============================================================================
# sign-windows.ps1 — Authenticode-sign Windows installers (free or paid cert)
# ============================================================================
# Signs dist/*.exe (and *.msi) with the certificate identified by
# WINDOWS_CERT_THUMBPRINT, using Set-AuthenticodeSignature + an RFC-3161
# timestamp (free public timestamp authority, so signatures outlive the cert).
#
# The cert can be EITHER a free self-signed cert (scripts/gen-selfsigned-cert.ps1,
# for enterprise allow-listing) OR a paid OV/EV cert (warning-free public
# installs). The thumbprint plumbing is identical — only the trust level differs.
#
# NEVER fakes a signature: if WINDOWS_CERT_THUMBPRINT is absent or the cert is
# not found, it prints an honest warning and exits 0 (artifacts stay unsigned).
#
# Usage:  ./scripts/sign-windows.ps1 [-DistDir dist]
# ----------------------------------------------------------------------------
[CmdletBinding()]
param([string]$DistDir = "dist")
$ErrorActionPreference = "Stop"

$thumb = $env:WINDOWS_CERT_THUMBPRINT
if ([string]::IsNullOrWhiteSpace($thumb)) {
    Write-Host "::warning::WINDOWS_CERT_THUMBPRINT absent — Windows installers UNSIGNED (Authenticode tier skipped). Generate a free self-signed cert with scripts/gen-selfsigned-cert.ps1, or supply a paid OV/EV cert. Never faked."
    exit 0
}
$cert = Get-ChildItem -Path Cert:\CurrentUser\My, Cert:\LocalMachine\My -CodeSigningCert -ErrorAction SilentlyContinue |
    Where-Object { $_.Thumbprint -eq $thumb } | Select-Object -First 1
if ($null -eq $cert) {
    Write-Host "::warning::No code-signing cert with thumbprint $thumb found — skipping Authenticode signing."
    exit 0
}
if (-not (Test-Path $DistDir)) { Write-Host "No $DistDir/ — nothing to sign."; exit 0 }

$tsUrl = "http://timestamp.digicert.com"  # free public RFC-3161 TSA
$signed = 0
Get-ChildItem -Path $DistDir -Include *.exe, *.msi -Recurse | ForEach-Object {
    $res = Set-AuthenticodeSignature -FilePath $_.FullName -Certificate $cert `
        -HashAlgorithm SHA256 -TimestampServer $tsUrl
    if ($res.Status -eq 'Valid') {
        Write-Host "  signed: $($_.Name)"
        $signed++
    } else {
        Write-Host "::error::Authenticode signing failed for $($_.Name): $($res.StatusMessage)"
        exit 1
    }
}
Write-Host "==> Authenticode: signed $signed file(s) with cert $thumb."
if ($cert.Issuer -eq $cert.Subject) {
    Write-Host "    (self-signed cert — enterprise allow-listing only; SmartScreen still warns public users)"
}

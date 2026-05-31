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

# Shared key-handle resolver (also used by sign-cloud.ps1). Dot-sourcing only
# defines functions; it runs no signing logic. NEVER touches a key value.
. "$PSScriptRoot/_sign-key-resolver.ps1"

# BYO cloud/HSM signing takes precedence when ITASHA_CLOUD_SIGNING=1 (opt-in,
# default-OFF). Delegate to sign-cloud.ps1 (CA/B-Forum 2023 hardware-key
# mandate path) and stop — the thumbprint path below is the free default.
if ($env:ITASHA_CLOUD_SIGNING -eq "1") {
    Write-Host "==> ITASHA_CLOUD_SIGNING=1 — delegating to scripts/sign-cloud.ps1 (BYO HSM/KMS)."
    & "$PSScriptRoot/sign-cloud.ps1" -DistDir $DistDir
    exit $LASTEXITCODE
}

# Default free path: resolve the Authenticode cert by WINDOWS_CERT_THUMBPRINT
# via the shared resolver (absent thumbprint / no matching cert => $null =>
# honest unsigned exit 0, never faked).
$cert = Resolve-ThumbprintIdentity
if ($null -eq $cert) { exit 0 }
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
Write-Host "==> Authenticode: signed $signed file(s) with cert $($cert.Thumbprint)."
if ($cert.Issuer -eq $cert.Subject) {
    Write-Host "    (self-signed cert — enterprise allow-listing only; SmartScreen still warns public users)"
}

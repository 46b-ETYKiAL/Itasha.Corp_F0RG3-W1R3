#!/usr/bin/env pwsh
# ============================================================================
# sign-cloud.ps1 — BYO cloud / HSM Authenticode signing (OPT-IN, default-OFF)
# ============================================================================
# Signs dist/*.exe and *.msi using a code-signing key that lives in a cloud HSM
# / KMS / hardware token, referenced BY HANDLE — never as a key value. This is
# the path for an Organization OV/EV certificate under the CA/B-Forum 2023
# hardware-key mandate (private keys MUST be in a FIPS-140-2 L2+ / Common
# Criteria EAL4+ module — they can no longer be exported to a .pfx). See
# docs/key-custody.md.
#
# DEFAULT-OFF: this script does nothing unless ITASHA_CLOUD_SIGNING=1 is set.
# It NEVER fakes a signature: if it is enabled but the backend/handle/tool is
# absent, it prints an honest ::warning:: and exits 0 (artifacts stay unsigned,
# the free minisign + cosign-keyless tiers still cover integrity). No paid
# service is REQUIRED — this is a documented option for orgs that have one.
#
# Free OSS backends (referenced by handle, nothing vendored, nothing installed
# by default):
#   * osslsigncode  — PKCS#11 engine (any HSM/token exposing a PKCS#11 module).
#   * jsign         — cloud-KMS native (Azure Trusted Signing, AWS KMS, GCP KMS,
#                     DigiCert ONE, etc.) by key reference.
#
# Required env (referenced BY NAME; supplied from the secret store at run time):
#   ITASHA_CLOUD_SIGNING=1            enable this path (else no-op, exit 0)
#   ITASHA_SIGN_BACKEND               "osslsigncode" | "jsign"
#   ITASHA_SIGN_KEY_HANDLE            the key HANDLE / URI (NOT a key value), e.g.
#                                       pkcs11:token=…;object=…  (osslsigncode)
#                                       or the jsign --storetype/--alias ref.
#   ITASHA_SIGN_CERT                  path/URL to the PUBLIC cert chain (.pem/.cer)
#   ITASHA_SIGN_TIMESTAMP_URL         RFC-3161 TSA (default: free DigiCert TSA)
#   (backend-specific auth — e.g. AZURE_* / PKCS11_MODULE — are read by the
#    backend tool itself from its own documented env, never echoed here.)
#
# Usage:  ITASHA_CLOUD_SIGNING=1 ITASHA_SIGN_BACKEND=jsign … ./scripts/sign-cloud.ps1 [-DistDir dist]
# ----------------------------------------------------------------------------
[CmdletBinding()]
param([string]$DistDir = "dist")
$ErrorActionPreference = "Stop"

# Shared key-handle resolver (also used by sign-windows.ps1). Returns a
# structured object describing the resolved signing identity, or $null when
# cloud signing is not enabled / not fully configured. NEVER returns or logs a
# key VALUE — only handles/URIs/cert paths.
. "$PSScriptRoot/_sign-key-resolver.ps1"

$resolved = Resolve-CloudSigningHandle
if ($null -eq $resolved) {
    # Resolve-CloudSigningHandle already emitted the honest reason (disabled or
    # incomplete config). Default-OFF / gated-not-faked: exit 0, unsigned.
    exit 0
}

if (-not (Test-Path $DistDir)) {
    Write-Host "No $DistDir/ — nothing to sign."
    exit 0
}

$tsUrl = if ([string]::IsNullOrWhiteSpace($env:ITASHA_SIGN_TIMESTAMP_URL)) {
    "http://timestamp.digicert.com"
} else { $env:ITASHA_SIGN_TIMESTAMP_URL }

$targets = Get-ChildItem -Path $DistDir -Include *.exe, *.msi -Recurse
if (-not $targets) {
    Write-Host "No .exe/.msi under $DistDir/ — nothing to sign."
    exit 0
}

$signed = 0
foreach ($file in $targets) {
    $path = $file.FullName
    switch ($resolved.Backend) {
        "osslsigncode" {
            if (-not (Get-Command osslsigncode -ErrorAction SilentlyContinue)) {
                Write-Host "::warning::ITASHA_SIGN_BACKEND=osslsigncode but osslsigncode is not on PATH — UNSIGNED (not faked). Install it (free OSS) or use jsign."
                exit 0
            }
            $tmp = "$path.signed"
            # PKCS#11 engine signing by handle; no key value is ever passed.
            & osslsigncode sign `
                -pkcs11engine "$($resolved.Pkcs11Engine)" `
                -pkcs11module "$($resolved.Pkcs11Module)" `
                -key "$($resolved.KeyHandle)" `
                -certs "$($resolved.CertPath)" `
                -h sha256 -ts "$tsUrl" `
                -in "$path" -out "$tmp"
            if ($LASTEXITCODE -ne 0) {
                Write-Host "::error::osslsigncode failed for $($file.Name)."
                exit 1
            }
            Move-Item -Force "$tmp" "$path"
        }
        "jsign" {
            if (-not (Get-Command jsign -ErrorAction SilentlyContinue)) {
                Write-Host "::warning::ITASHA_SIGN_BACKEND=jsign but jsign is not on PATH — UNSIGNED (not faked). Install it (free OSS) or use osslsigncode."
                exit 0
            }
            # jsign references a cloud-KMS key by its --storetype/--alias handle;
            # backend auth is read by jsign from its own documented env.
            & jsign `
                --storetype "$($resolved.StoreType)" `
                --alias "$($resolved.KeyHandle)" `
                --certfile "$($resolved.CertPath)" `
                --tsaurl "$tsUrl" `
                --tsmode RFC3161 `
                "$path"
            if ($LASTEXITCODE -ne 0) {
                Write-Host "::error::jsign failed for $($file.Name)."
                exit 1
            }
        }
        default {
            Write-Host "::warning::Unknown ITASHA_SIGN_BACKEND '$($resolved.Backend)' — UNSIGNED (not faked). Use 'osslsigncode' or 'jsign'."
            exit 0
        }
    }
    Write-Host "  cloud-signed: $($file.Name)  (backend=$($resolved.Backend))"
    $signed++
}

Write-Host "==> Cloud signing: signed $signed file(s) via $($resolved.Backend) by key handle (no key value touched)."

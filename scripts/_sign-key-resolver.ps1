#!/usr/bin/env pwsh
# ============================================================================
# _sign-key-resolver.ps1 — shared key-HANDLE resolver for the signing scripts
# ============================================================================
# Dot-sourced by sign-cloud.ps1 (BYO cloud/HSM) and sign-windows.ps1 (thumbprint
# default). Resolves a signing identity from env vars referenced BY NAME/HANDLE
# — it NEVER reads, returns, or logs a key VALUE, only handles / URIs / cert
# paths / thumbprints. Dot-sourcing this file ONLY defines functions; it runs no
# signing logic. See docs/key-custody.md.
# ----------------------------------------------------------------------------

function Resolve-CloudSigningHandle {
    <#
    .SYNOPSIS
      Resolve the BYO cloud/HSM signing identity from env (handles only).
    .OUTPUTS
      A hashtable describing the backend + key handle + cert path, or $null when
      cloud signing is disabled (default) or incompletely configured. On $null
      it emits an honest ::warning::/notice and the caller exits 0 unsigned
      (gated-not-faked). NEVER returns a key value.
    #>
    [CmdletBinding()]
    param()

    # DEFAULT-OFF master switch.
    if ($env:ITASHA_CLOUD_SIGNING -ne "1") {
        Write-Host "::notice::Cloud signing is OFF (set ITASHA_CLOUD_SIGNING=1 to enable BYO HSM/KMS signing). Using the default thumbprint path / free tiers."
        return $null
    }

    $backend = $env:ITASHA_SIGN_BACKEND
    $handle = $env:ITASHA_SIGN_KEY_HANDLE
    $cert = $env:ITASHA_SIGN_CERT

    $missing = @()
    if ([string]::IsNullOrWhiteSpace($backend)) { $missing += "ITASHA_SIGN_BACKEND" }
    if ([string]::IsNullOrWhiteSpace($handle)) { $missing += "ITASHA_SIGN_KEY_HANDLE" }
    if ([string]::IsNullOrWhiteSpace($cert)) { $missing += "ITASHA_SIGN_CERT" }
    if ($missing.Count -gt 0) {
        Write-Host "::warning::Cloud signing enabled but incompletely configured (missing: $($missing -join ', ')) — UNSIGNED (not faked). See docs/key-custody.md."
        return $null
    }

    return @{
        Backend      = $backend
        KeyHandle    = $handle            # a HANDLE/URI/alias — never a key value
        CertPath     = $cert              # PUBLIC cert chain only
        # osslsigncode PKCS#11 plumbing (read by handle; backend tool reads PINs
        # from its own documented env, never echoed here).
        Pkcs11Engine = $env:ITASHA_PKCS11_ENGINE
        Pkcs11Module = $env:ITASHA_PKCS11_MODULE
        # jsign cloud-KMS store type (e.g. AZUREKEYVAULT, AWS, GOOGLECLOUD).
        StoreType    = if ([string]::IsNullOrWhiteSpace($env:ITASHA_SIGN_STORETYPE)) { "AZUREKEYVAULT" } else { $env:ITASHA_SIGN_STORETYPE }
    }
}

function Resolve-ThumbprintIdentity {
    <#
    .SYNOPSIS
      Resolve the default Windows Authenticode cert by WINDOWS_CERT_THUMBPRINT.
    .OUTPUTS
      The X509 cert object, or $null when the thumbprint is absent or no matching
      cert is found (caller exits 0 unsigned — never faked).
    #>
    [CmdletBinding()]
    param()

    $thumb = $env:WINDOWS_CERT_THUMBPRINT
    if ([string]::IsNullOrWhiteSpace($thumb)) {
        Write-Host "::warning::WINDOWS_CERT_THUMBPRINT absent — Windows installers UNSIGNED (Authenticode tier skipped). Generate a free self-signed cert with scripts/gen-selfsigned-cert.ps1, or supply a paid OV/EV cert, or enable cloud signing (ITASHA_CLOUD_SIGNING=1). Never faked."
        return $null
    }
    $cert = Get-ChildItem -Path Cert:\CurrentUser\My, Cert:\LocalMachine\My -CodeSigningCert -ErrorAction SilentlyContinue |
        Where-Object { $_.Thumbprint -eq $thumb } | Select-Object -First 1
    if ($null -eq $cert) {
        Write-Host "::warning::No code-signing cert with thumbprint $thumb found — skipping Authenticode signing."
        return $null
    }
    return $cert
}

#!/usr/bin/env pwsh
# ============================================================================
# verify.ps1 — verify a downloaded F0RG3-W1R3 artifact on Windows (free)
# ============================================================================
# Windows counterpart to verify.sh. Checks SHA-256 against checksum.sha256 and,
# when minisign is installed, the detached minisign signature against the
# published public key. Also reports the Authenticode status (Valid / signed
# by a self-signed/enterprise cert / UnknownError = unsigned dev build).
#
# Usage:  ./scripts/verify.ps1 -Artifact <path> [-PublicKey keys/minisign.pub]
# ----------------------------------------------------------------------------
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Artifact,
    [string]$PublicKey = "keys/minisign.pub"
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Artifact)) { Write-Error "artifact not found: $Artifact"; exit 2 }
$rc = 0

# --- SHA-256 ---
if (Test-Path "checksum.sha256") {
    $base = Split-Path -Leaf $Artifact
    $have = (Get-FileHash -Algorithm SHA256 $Artifact).Hash.ToLower()
    $line = Select-String -Path "checksum.sha256" -Pattern ([regex]::Escape($base)) | Select-Object -First 1
    $want = if ($line) { ($line.Line -split '\s+')[0].ToLower() } else { "" }
    if ($want -and $have -eq $want) { Write-Host "  OK  sha256 matches checksum.sha256" }
    else { Write-Host "  FAIL sha256 mismatch (have $have, want $want)"; $rc = 1 }
} else { Write-Host "  --  checksum.sha256 not present; skipping hash check" }

# --- minisign ---
if (Get-Command minisign -ErrorAction SilentlyContinue) {
    if ((Test-Path "$Artifact.minisig") -and (Test-Path $PublicKey)) {
        & minisign -Vm $Artifact -p $PublicKey | Out-Null
        if ($LASTEXITCODE -eq 0) { Write-Host "  OK  minisign signature valid ($PublicKey)" }
        else { Write-Host "  FAIL minisign signature INVALID"; $rc = 1 }
    } else { Write-Host "  --  no $Artifact.minisig or $PublicKey; signature check skipped" }
} else { Write-Host "  --  minisign not installed (https://jedisct1.github.io/minisign/)" }

# --- Authenticode (informational) ---
if ($Artifact -match '\.(exe|msi)$') {
    $sig = Get-AuthenticodeSignature $Artifact
    Write-Host ("  ii  Authenticode: {0} (signer: {1})" -f $sig.Status, $sig.SignerCertificate.Subject)
    if ($sig.Status -ne 'Valid') {
        Write-Host "      (unsigned or self-signed/enterprise cert — SmartScreen may warn for general users)"
    }
}

if ($rc -eq 0) { Write-Host "==> VERIFIED" } else { Write-Host "==> VERIFICATION FAILED" }
exit $rc

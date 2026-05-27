#!/usr/bin/env pwsh
# ============================================================================
# build.ps1 — Itasha.Corp installer framework build wrapper (PowerShell)
# ============================================================================
# Windows-native counterpart to build.sh. Merges packager.template.toml with a
# per-app override and invokes cargo-packager. If cargo-packager is NOT
# installed, prints the exact install command and exits non-zero (no silent
# success).
#
# Usage:
#   ./scripts/build.ps1 -App c0pl4nd [-Binary <path>] [-DryRun]
# ----------------------------------------------------------------------------
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$App,
    [string]$Binary = "",
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
$CargoPackagerVersion = "0.11.8"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root      = (Resolve-Path (Join-Path $ScriptDir "..")).Path
$Template  = Join-Path $Root "packager.template.toml"
$Override  = Join-Path $Root "apps/$App.toml"

if (-not (Test-Path $Template)) {
    Write-Error "template not found: $Template"
    exit 1
}
if (-not (Test-Path $Override)) {
    Write-Error "per-app override not found: $Override"
    exit 1
}

Write-Host "==> Validating template + override for app '$App'"
& python (Join-Path $Root "tests/validate_config.py") $Template $Override
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Resolve the binary input (binary-input-contract.md).
$ResolvedBinary = $Binary
if ([string]::IsNullOrEmpty($ResolvedBinary)) { $ResolvedBinary = $env:ITASHA_BINARY_PATH }

if ($DryRun) {
    Write-Host "==> Dry run: config resolves; skipping cargo-packager invocation."
    if (-not [string]::IsNullOrEmpty($ResolvedBinary)) {
        Write-Host "    binary input: $ResolvedBinary"
    } else {
        Write-Host "    binary input: (none supplied — dry run uses a placeholder)"
    }
    exit 0
}

if ([string]::IsNullOrEmpty($ResolvedBinary)) {
    Write-Error "no binary input. Pass -Binary <path> or set ITASHA_BINARY_PATH. See docs/binary-input-contract.md."
    exit 1
}
if (-not (Test-Path $ResolvedBinary)) {
    Write-Error "binary input does not exist: $ResolvedBinary"
    exit 1
}

# Require cargo-packager; fail loudly if missing.
$cp = Get-Command cargo-packager -ErrorAction SilentlyContinue
if ($null -eq $cp) {
    Write-Host "ERROR: cargo-packager is not installed." -ForegroundColor Red
    Write-Host ""
    Write-Host "Install the pinned version with:"
    Write-Host "    cargo install cargo-packager --version $CargoPackagerVersion --locked"
    Write-Host ""
    Write-Host "Then re-run: ./scripts/build.ps1 -App $App -Binary $ResolvedBinary"
    exit 127
}

Write-Host "==> Invoking cargo-packager for app '$App'"
Push-Location $Root
try {
    $env:ITASHA_BINARY_PATH = $ResolvedBinary
    & cargo-packager --config "apps/$App.toml"
    exit $LASTEXITCODE
} finally {
    Pop-Location
}

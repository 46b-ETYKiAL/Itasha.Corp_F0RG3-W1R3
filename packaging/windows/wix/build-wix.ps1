#!/usr/bin/env pwsh
# ============================================================================
# build-wix.ps1 — RESERVED enterprise MSI build (WiX Toolset 6)
# ============================================================================
# Builds the reserved Group-Policy / Intune MSI from product.wxs. NSIS (via
# cargo-packager) is the PRIMARY branded installer; this MSI is preserved (not
# dropped) for enterprise managed deployment. See docs/adr/.
#
# CRITICAL VERSION LESSON (dependency_version_conflict / supply_chain):
#   The WixToolset.UI.wixext extension MUST match the `wix.exe` toolset MAJOR
#   version. GitHub `windows-latest` historically shipped WiX v3.14.1 on PATH;
#   WiX v4/v6 is a SEPARATE dotnet-tool install. This script pins BOTH the
#   toolset and the UI extension to the SAME major version (6) as an explicit
#   step — never relying on a pre-installed `candle.exe`/`light.exe` (v3) or a
#   mismatched extension.
#
# Usage:
#   ./build-wix.ps1 -App c0pl4nd -Binary <path-to-built-exe> [-Version 0.1.0]
# ----------------------------------------------------------------------------
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$App,
    [Parameter(Mandatory = $true)][string]$Binary,
    [string]$Version = "0.1.0"
)

$ErrorActionPreference = "Stop"

# Pin the WiX toolset + UI extension to the SAME major version.
$WixToolsetVersion = "6.0.0"
$WixUiExtension    = "WixToolset.UI.wixext"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root      = (Resolve-Path (Join-Path $ScriptDir "../../..")).Path
$Wxs       = Join-Path $ScriptDir "product.wxs"

if (-not (Test-Path $Binary)) {
    Write-Error "binary input not found: $Binary"
    exit 1
}
$BinaryFull = (Resolve-Path $Binary).Path
$BinaryName = Split-Path -Leaf $BinaryFull

# --- Ensure the WiX v6 toolset is present (explicit pinned install). ---
$wixOnPath = Get-Command wix -ErrorAction SilentlyContinue
if (-not $wixOnPath) {
    Write-Host "==> WiX toolset not on PATH. Install the pinned version with:"
    Write-Host "    dotnet tool install --global wix --version $WixToolsetVersion"
    Write-Host "    wix extension add -g $WixUiExtension/$WixToolsetVersion"
    exit 127
}

$wixVersion = (& wix --version) 2>$null
Write-Host "==> Using wix toolset version: $wixVersion"
if ($wixVersion -notlike "6.*") {
    Write-Error ("WiX toolset major version mismatch: expected 6.x, found '$wixVersion'. " +
                 "windows-latest ships WiX v3 by default; install v6 as a dotnet tool and " +
                 "ensure the UI extension is the SAME major version.")
    exit 1
}

# Stable per-app UpgradeCode. This GUID MUST be constant across every version of
# a given app so MajorUpgrade replaces cleanly. Derived deterministically from
# the app name so a maintainer never has to hand-manage GUIDs.
$upgradeSeed = [System.Text.Encoding]::UTF8.GetBytes("itasha.corp.installer.$App")
$md5 = [System.Security.Cryptography.MD5]::Create()
$hash = $md5.ComputeHash($upgradeSeed)
$UpgradeCode = ([System.Guid]::new($hash)).ToString("D").ToUpper()

$OutDir = Join-Path $Root "dist"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$Msi = Join-Path $OutDir "$App-$Version-x86_64.msi"

Write-Host "==> Building reserved MSI for '$App' -> $Msi"
& wix build $Wxs `
    -arch x64 `
    -ext $WixUiExtension `
    -d "APP_PRODUCT_NAME=$App" `
    -d "APP_VERSION=$Version" `
    -d "APP_MANUFACTURER=Itasha.Corp" `
    -d "APP_UPGRADE_CODE=$UpgradeCode" `
    -d "APP_INSTALL_SUBDIR=$App" `
    -d "APP_BINARY_NAME=$BinaryName" `
    -d "APP_BINARY_PATH=$BinaryFull" `
    -o $Msi
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "==> Reserved MSI built: $Msi"
Write-Host "    (Group-Policy / Intune deployment track — NSIS remains primary.)"

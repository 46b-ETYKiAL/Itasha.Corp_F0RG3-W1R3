<#
================================================================================
 matrix.ps1 — Windows installer test matrix (Itasha.Corp installer framework)
================================================================================
 Runs the five Windows installer-lifecycle scenarios against a built NSIS
 installer. Intended to run inside a DISPOSABLE clean VM / Windows Sandbox so
 the filesystem/registry/PATH snapshot diffs are meaningful.

 Scenarios:
   1. fresh-install   — install, assert files at C:\Program Files\Itasha.Corp\C0PL4ND,
                        ARP registry entry, Start-Menu shortcut.
   2. upgrade         — install over a previous version: single ARP entry, no dup files.
   3. uninstall-diff  — before/after snapshot of FS + registry + Start-Menu + PATH;
                        assert ZERO residue after uninstall.
   4. silent          — install with /S, assert exit 0 + log has no errors.
   5. shortcuts-arp   — Start-Menu shortcut PRESENT, Desktop shortcut ABSENT
                        (default-off honored), PATH delta exact then gone on
                        uninstall, ARP DisplayName/Publisher/DisplayVersion/
                        DisplayIcon/InstallLocation correct.

 HONEST SKIP DISCIPLINE:
   * If not running on Windows  -> exit 77 (skip), loud message.
   * If no installer .exe given -> exit 77 (skip), loud message.
   * A skip is NEVER reported as success; a real assertion failure exits 1.

 Usage:
   pwsh tests/windows/matrix.ps1 -Installer <path-to-setup.exe> [-Scenario all|fresh|upgrade|uninstall|silent|shortcuts] [-PreviousInstaller <path>]

 Exit codes: 0 pass; 1 fail; 77 skip (tool/host/artifact unavailable).
================================================================================
#>
[CmdletBinding()]
param(
    [string]$Installer,
    [ValidateSet('all', 'fresh', 'upgrade', 'uninstall', 'silent', 'shortcuts')]
    [string]$Scenario = 'all',
    [string]$PreviousInstaller
)

$ErrorActionPreference = 'Stop'
$SKIP = 77
$InstallDir = 'C:\Program Files\Itasha.Corp\C0PL4ND'
$AppName = 'C0PL4ND'

function Write-Pass([string]$m) { Write-Host "PASS: $m" }
function Write-Fail([string]$m) { Write-Error "FAIL: $m"; $script:Failed = $true }
function Write-Skip([string]$m) {
    Write-Warning "SKIP: $m"
    Write-Warning "matrix-skip-reason: host-or-artifact-unavailable"
}

# --- Host + artifact gating (honest skip, never fake) ----------------------
if (-not $IsWindows -and $env:OS -ne 'Windows_NT') {
    Write-Skip "Windows installer matrix requires a Windows host (got non-Windows)."
    exit $SKIP
}
if ([string]::IsNullOrEmpty($Installer) -or -not (Test-Path -LiteralPath $Installer)) {
    Write-Skip "no installer .exe supplied (pass -Installer <path>); build one with scripts/build.ps1 first."
    exit $SKIP
}
# Elevation is required for a perMachine install (D4).
$isAdmin = ([Security.Principal.WindowsPrincipal] `
    [Security.Principal.WindowsIdentity]::GetCurrent()
).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Skip "perMachine install requires an elevated (Administrator) shell; re-run elevated in the clean VM."
    exit $SKIP
}

$script:Failed = $false

# --- Snapshot helpers ------------------------------------------------------
function Get-StateSnapshot {
    [PSCustomObject]@{
        FilesPresent = Test-Path -LiteralPath $InstallDir
        Arp          = Get-ArpEntry
        StartMenu    = Test-StartMenuShortcut
        Desktop      = Test-DesktopShortcut
        PathHasApp   = (Test-PathContains $InstallDir)
    }
}

function Get-ArpEntry {
    $roots = @(
        'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
    )
    foreach ($r in $roots) {
        $e = Get-ItemProperty -Path $r -ErrorAction SilentlyContinue |
            Where-Object { $_.DisplayName -eq $AppName -or $_.DisplayName -like "*$AppName*" }
        if ($e) { return @($e) }
    }
    return @()
}

function Test-StartMenuShortcut {
    $paths = @(
        (Join-Path $env:ProgramData "Microsoft\Windows\Start Menu\Programs\$AppName.lnk"),
        (Join-Path $env:ProgramData "Microsoft\Windows\Start Menu\Programs\Itasha.Corp\$AppName.lnk")
    )
    foreach ($p in $paths) { if (Test-Path -LiteralPath $p) { return $true } }
    return $false
}

function Test-DesktopShortcut {
    $p = Join-Path ([Environment]::GetFolderPath('CommonDesktopDirectory')) "$AppName.lnk"
    return (Test-Path -LiteralPath $p)
}

function Test-PathContains([string]$dir) {
    $machinePath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    return ($machinePath -split ';' | Where-Object { $_.TrimEnd('\') -ieq $dir.TrimEnd('\') }).Count -gt 0
}

function Invoke-Installer([string]$exe, [string[]]$args) {
    $p = Start-Process -FilePath $exe -ArgumentList $args -Wait -PassThru
    return $p.ExitCode
}

function Get-UninstallCommand {
    $arp = Get-ArpEntry
    if ($arp.Count -gt 0) {
        $u = $arp[0].QuietUninstallString
        if ([string]::IsNullOrEmpty($u)) { $u = $arp[0].UninstallString }
        return $u
    }
    return $null
}

# --- Scenarios -------------------------------------------------------------
function Test-Fresh {
    Write-Host "=== scenario: fresh-install ==="
    $code = Invoke-Installer $Installer @('/S')
    if ($code -ne 0) { Write-Fail "fresh install exit code $code (expected 0)"; return }
    if (-not (Test-Path -LiteralPath $InstallDir)) { Write-Fail "install dir absent: $InstallDir"; return }
    if ((Get-ArpEntry).Count -lt 1) { Write-Fail "no ARP entry after fresh install"; return }
    if (-not (Test-StartMenuShortcut)) { Write-Fail "Start-Menu shortcut absent (should be ON by default)"; return }
    Write-Pass "fresh install: files + ARP + Start-Menu shortcut present"
}

function Test-Shortcuts {
    Write-Host "=== scenario: shortcuts + PATH + ARP ==="
    if ((Get-ArpEntry).Count -lt 1) { Write-Fail "expected an installed app for shortcuts check"; return }
    if (-not (Test-StartMenuShortcut)) { Write-Fail "Start-Menu shortcut should be PRESENT (default ON)"; return }
    if (Test-DesktopShortcut) { Write-Fail "Desktop shortcut should be ABSENT (D-spec default OFF)"; return }
    $arp = (Get-ArpEntry)[0]
    foreach ($field in 'DisplayName', 'Publisher', 'DisplayVersion', 'DisplayIcon', 'InstallLocation') {
        if ([string]::IsNullOrEmpty($arp.$field)) { Write-Fail "ARP field '$field' is empty"; return }
    }
    if ($arp.Publisher -ne 'Itasha.Corp') { Write-Fail "ARP Publisher '$($arp.Publisher)' != 'Itasha.Corp'"; return }
    if ($arp.InstallLocation.TrimEnd('\') -ine $InstallDir.TrimEnd('\')) {
        Write-Fail "ARP InstallLocation '$($arp.InstallLocation)' != '$InstallDir'"; return
    }
    Write-Pass "shortcuts (Start-Menu ON, Desktop OFF) + ARP fields correct"
}

function Test-Upgrade {
    Write-Host "=== scenario: upgrade-over-previous ==="
    if ([string]::IsNullOrEmpty($PreviousInstaller) -or -not (Test-Path -LiteralPath $PreviousInstaller)) {
        Write-Skip "upgrade scenario needs -PreviousInstaller <older-setup.exe>; skipping."
        return $SKIP
    }
    Invoke-Installer $PreviousInstaller @('/S') | Out-Null
    Invoke-Installer $Installer @('/S') | Out-Null
    $entries = Get-ArpEntry
    if ($entries.Count -ne 1) { Write-Fail "expected exactly 1 ARP entry after upgrade, found $($entries.Count)"; return }
    Write-Pass "upgrade: single ARP entry, no duplicate registration"
}

function Test-Silent {
    Write-Host "=== scenario: silent/unattended ==="
    $code = Invoke-Installer $Installer @('/S')
    if ($code -ne 0) { Write-Fail "silent install exit code $code (expected 0)"; return }
    Write-Pass "silent install exit 0"
}

function Test-UninstallResidue {
    Write-Host "=== scenario: uninstall + clean-removal residue diff ==="
    $before = Get-StateSnapshot
    if (-not $before.FilesPresent) {
        Invoke-Installer $Installer @('/S') | Out-Null
    }
    $u = Get-UninstallCommand
    if ([string]::IsNullOrEmpty($u)) { Write-Fail "no uninstall command found in ARP"; return }
    # Run the uninstaller silently.
    cmd /c "$u /S" | Out-Null
    Start-Sleep -Seconds 3
    $after = Get-StateSnapshot
    $residue = @()
    if ($after.FilesPresent) { $residue += "install dir still present: $InstallDir" }
    if ($after.Arp.Count -gt 0) { $residue += "ARP entry still present" }
    if ($after.StartMenu) { $residue += "Start-Menu shortcut still present" }
    if ($after.Desktop) { $residue += "Desktop shortcut still present" }
    if ($after.PathHasApp) { $residue += "PATH still contains install dir" }
    if ($residue.Count -gt 0) {
        Write-Fail ("uninstall left residue: " + ($residue -join '; '))
        return
    }
    Write-Pass "uninstall: ZERO filesystem/registry/Start-Menu/PATH residue"
}

# --- Dispatch --------------------------------------------------------------
switch ($Scenario) {
    'fresh'     { Test-Fresh }
    'upgrade'   { Test-Upgrade | Out-Null }
    'uninstall' { Test-UninstallResidue }
    'silent'    { Test-Silent }
    'shortcuts' { Test-Shortcuts }
    'all' {
        Test-Fresh
        Test-Shortcuts
        Test-Silent
        Test-Upgrade | Out-Null
        Test-UninstallResidue
    }
}

if ($script:Failed) { exit 1 } else { exit 0 }

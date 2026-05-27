# Installer Test Matrix

Runnable lifecycle tests for the produced installers, plus the public-repo
content-safety gate. Every test follows **honest detect-and-skip**: when a
required tool, host OS, or built artifact is unavailable, the script prints a
**loud structured skip** and exits `77` (the GNU-automake "skipped" convention)
— it is **never** silent and **never** faked as success. A real assertion
failure exits `1`.

## Layout

| Path | What it covers |
|---|---|
| `content_safety_audit.py` | Public-repo IP-boundary gate (secrets / app source / internal paths / agent-system refs). Exit 0 = clean. |
| `validate_config.py` | TOML parse + template/override schema + resolved Windows install dir. |
| `matrix/_lib.sh` | Shared `require_tool` / `require_os` / `skip` helpers for the shell harnesses. |
| `windows/matrix.ps1` | The five Windows scenarios (fresh / upgrade / uninstall-residue-diff / silent / shortcuts+PATH+ARP). |
| `windows/sandbox.wsb` | Windows Sandbox config for a disposable clean-VM run. |
| `macos/verify.sh` | codesign + spctl (Gatekeeper) + stapler (notarization) verification. |
| `linux/verify.sh` | desktop-file-validate + lintian + AppImage self-check. |

## The 14 test types → where they live

1. fresh-install → `windows/matrix.ps1 -Scenario fresh`
2. upgrade-over-previous → `windows/matrix.ps1 -Scenario upgrade -PreviousInstaller <older.exe>`
3. uninstall + clean-removal residue diff → `windows/matrix.ps1 -Scenario uninstall`
4. silent/unattended (`/S`) → `windows/matrix.ps1 -Scenario silent`
5. shortcut presence (Start-Menu ON) → `windows/matrix.ps1 -Scenario shortcuts`
6. Desktop shortcut ABSENT (D-spec default OFF) → `windows/matrix.ps1 -Scenario shortcuts`
7. PATH delta exact-then-gone → `windows/matrix.ps1 -Scenario uninstall`
8. ARP DisplayName/Publisher/Version/Icon/InstallLocation → `windows/matrix.ps1 -Scenario shortcuts`
9. clean-VM / Windows Sandbox isolation → `windows/sandbox.wsb`
10. macOS signing verification (codesign) → `macos/verify.sh`
11. macOS Gatekeeper acceptance (spctl) → `macos/verify.sh`
12. macOS notarization staple (stapler) → `macos/verify.sh`
13. Linux .deb lint (lintian) + .desktop validity → `linux/verify.sh`
14. Linux AppImage integrity + menu integration → `linux/verify.sh`

## Running

### Windows (in a disposable clean VM / Windows Sandbox, elevated)

```powershell
# 1. build an installer:  .\scripts\build.ps1 -App c0pl4nd
# 2. run the full matrix (perMachine install needs an elevated shell):
pwsh tests\windows\matrix.ps1 -Installer dist\C0PL4ND-setup.exe -Scenario all
```

Without `-Installer`, on a non-Windows host, or in a non-elevated shell, the
harness exits `77` (skip) with a loud reason — never a false pass.

### macOS

```sh
./tests/macos/verify.sh --app dist/C0PL4ND.app --dmg dist/C0PL4ND.dmg
```

An intentionally-unsigned dev build records an honest "UNSIGNED (dev build)"
note and exits 0 (the documented dev-unsigned-until-creds posture, ADR-0003) —
it is never reported as notarized.

### Linux

```sh
./tests/linux/verify.sh --appimage dist/C0PL4ND.AppImage --deb dist/c0pl4nd.deb
```

Missing `lintian` / `appimagetool` / `desktop-file-validate` → that check is
skipped loudly; the script still runs everything it can.

### Content-safety gate (runs anywhere with Python 3.11+)

```sh
python tests/content_safety_audit.py   # exit 0 = no IP-boundary leakage
```

## Exit-code contract

| Code | Meaning |
|---|---|
| 0 | scenario passed (or an intentionally-unsigned dev build verified honestly) |
| 1 | a real assertion failed |
| 77 | skipped — required tool / host OS / built artifact unavailable (loud, documented) |
| 2 | usage / IO error |

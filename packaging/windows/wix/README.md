# Reserved Enterprise MSI Track (WiX)

This directory is the **reserved, documented enterprise MSI track**. The
primary branded installer is **NSIS** (via cargo-packager); the WiX/Burn MSI is
**preserved, not dropped**, for Group-Policy / Intune enterprise deployment.
This is the "recommend, do not silently drop" outcome from decision **D2** — the
existing working MSI is kept as a parallel, buildable track.

## Files

| File | Purpose |
|------|---------|
| `product.wxs` | WiX v6 MSI definition. perMachine install to `<ProgramFiles>\Itasha.Corp\<AppFolder>` (D4 — matches the NSIS target). MajorUpgrade for clean version replacement; ARP `InstallLocation` + icon; Start Menu shortcut (default ON, mirrors the NSIS D-spec). |
| `build-wix.ps1` | Build wrapper. Pins the WiX toolset + UI extension to the **same major version** (6), derives a stable per-app `UpgradeCode`, and fails loudly if `wix` is missing or is the wrong major version. |
| `license.rtf` | License shown in the WixUI dialog. Provided per-app or as a shared default. |

## Build (reserved track)

```powershell
# 1. Install the pinned WiX v6 toolset + the version-matched UI extension:
dotnet tool install --global wix --version 6.0.0
wix extension add -g WixToolset.UI.wixext/6.0.0

# 2. Build the MSI from a compiled binary:
./build-wix.ps1 -App c0pl4nd -Binary C:\path\to\c0pl4nd.exe -Version 0.1.0
# -> dist/c0pl4nd-0.1.0-x86_64.msi
```

## The WiX-version-match lesson (load-bearing)

> **`WixUIExtension` MUST be the same major version as the `wix.exe` toolset.**

GitHub `windows-latest` runners historically ship **WiX v3.14.1** on `PATH`
(the `candle.exe` / `light.exe` toolchain). WiX **v4/v6** is a *separate*
`dotnet tool` install with a *different* CLI (`wix build`) and *namespaced*
extensions (`WixToolset.UI.wixext`). Mixing a v3 toolset with a v4/v6 UI
extension — or assuming the pre-installed v3 is what you want — is the classic
MSI build failure.

`build-wix.ps1` closes this by:

1. Requiring `wix` (v6 CLI) on `PATH` and **asserting** the major version is
   `6.x` (it exits non-zero on a v3 mismatch rather than producing a broken
   MSI).
2. Pinning both the toolset (`6.0.0`) and the UI extension
   (`WixToolset.UI.wixext/6.0.0`) to the same major version in the documented
   install commands.
3. In CI, the v6 toolset is therefore an **explicit pinned install step**, never
   a reliance on the runner's default v3.

The engine ADRs (`docs/adr/0002-reserved-wix-msi.md`) record the
recommend-not-drop rationale and this version-match constraint.

## Why MSI is reserved, not primary

| Aspect | NSIS (primary) | WiX/MSI (reserved) |
|--------|----------------|--------------------|
| Branded first-install UX | Full custom UI, options screen, brand bitmaps | Standard WixUI dialogs |
| Authoring burden | NSIS hook include (`.nsh`) | XML (`.wxs`) |
| Enterprise managed deploy | Limited | Group Policy + Intune native (`.msi`) |
| Use when | Direct end-user download | IT-managed fleet deployment |

NSIS wins the branded direct-download experience; MSI wins managed enterprise
deployment. Both install to the same D4 target so a fleet and a direct user end
up with an identical install layout.

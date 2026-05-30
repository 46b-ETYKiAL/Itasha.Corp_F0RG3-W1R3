# Itasha.Corp Native Installer — CRT Provisioning Console

A bespoke, **reusable** Windows installer for every Itasha.Corp app: a brand-
themed **CRT provisioning console** (Rust / `eframe`/`egui`) instead of a dated
wizard. One generic crate; the app name, binary, vendor, install sub-dir,
version, voice colour, kanji, tagline **and the payload** are injected at build
time, so the same installer serves C0PL4ND, SCR1B3, N4V1, … with no per-app fork.

## What it does

- **Per-machine install** to `C:\Program Files\Itasha.Corp\<App>` — the default
  path is **pre-filled** on the location step (editable + Browse). Elevation via a
  `requireAdministrator` manifest.
- Brand identity from the influences (Lain / GitS / Akira / Eva / Gundam / JDM /
  Gharliera / antireal): drifting perspective grid, Eva-style **sync gauge**,
  terminal provisioning log, per-app **voice colour** accent, kanji watermark,
  mecha-HUD brackets. Akira-red reserved for faults only.
- Engine: extract payload → ARP/registry → Start-Menu / optional desktop
  shortcut → optional system PATH → self-uninstaller. System ops go through
  `reg.exe` / PowerShell (no `unsafe`).
- **Modes:** GUI (default) · `--silent [--dir <path>] [--no-start-menu]
  [--desktop] [--add-path]` (winget / enterprise / unattended) · `--uninstall`.

## Build a per-app installer

```sh
python scripts/build_native_installer.py \
  --app <app> \                     # apps/<app>.toml
  --binary-dir <dir/with/app.exe+assets> \
  --version v1.2.3 \
  --out dist/<app>-v1.2.3-x86_64-setup.exe
```

The wrapper reads `apps/<app>.toml` (`[itasha.app]` + optional `[itasha.brand]`),
zips the payload, sets the `ITASHA_*` build config, and `cargo build --release`s
the crate into one signed-ready `setup.exe`.

## Onboard a new app

1. You already have `apps/<app>.toml` (`[itasha.app]`) from the cargo-packager
   track. Add an `[itasha.brand]` table:
   ```toml
   [itasha.brand]
   voice_hex = "#34E0D0"   # the app's banner voice colour (installer accent)
   kanji     = "端末"        # CRT kanji watermark
   tagline   = "one line, shown on the console"
   ```
2. In the app's release workflow, build the Windows binary, assemble the payload
   (`<app>.exe` + `assets/` + licenses), and call the wrapper (see C0PL4ND's
   `.github/workflows/release.yml` `windows-installer` job for the reference).

## Code signing

The installer ships **unsigned** by default (SmartScreen "More info → Run anyway";
SHA-256 + SLSA build-provenance attestation provided). The release workflow wires
**SignPath Foundation** (free OSS signing), gated on the `SIGNPATH_ORG_ID` repo
variable — inactive until a SignPath project is approved. Apply at
<https://signpath.org/>; set `SIGNPATH_ORG_ID` (var) + `SIGNPATH_API_TOKEN`
(secret) and the release signs automatically. (Self-branded paid alternative:
Azure Trusted Signing, ~$10/mo.)

## Crate layout

| File | Purpose |
|---|---|
| `src/main.rs` | provisioning-console GUI + `--silent`/`--uninstall` dispatch |
| `src/theme.rs` | wired-noir brand palette + CRT chrome painter |
| `src/engine.rs` | install/uninstall engine (extract · ARP · shortcuts · PATH) |
| `build.rs` | elevation manifest (`elevate` feature) · `ITASHA_*` config · payload embed |

Build a non-elevated visual-QA binary with `cargo build --no-default-features`.

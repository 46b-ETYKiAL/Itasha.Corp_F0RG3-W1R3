# ADR-0001 — Installer Engine: cargo-packager

- **Status:** Accepted
- **Date:** 2026-05-26
- **Deciders:** Itasha.Corp installer framework

## Context

The framework must turn one shared config + a small per-app override into
branded installers for Windows, macOS, and Linux, reusable across all
Itasha.Corp Rust apps, with the smallest possible per-app surface (the D1
reusability goal).

## Decision

Use **cargo-packager (CrabNebula)** as the cross-platform engine, pinned to
**v0.11.8**.

- One `Packager.toml` / `[package.metadata.packager]` block produces Windows
  **NSIS** (+ MSI), macOS **DMG** (+ `.app`), and Linux **AppImage** + **.deb**.
- Built-in code signing + macOS notarization hooks + `cargo-packager-updater`.
- Rust-native and free (no paid tier); runs locally and on free GitHub Actions.
- A per-app override is ~15-20 lines (`apps/<app>.toml`), satisfying D1.

### Alternatives rejected

| Option | Why rejected |
|---|---|
| Velopack | No branded first-install GUI on Windows. |
| cargo-dist | No branded GUI installer (archive/updater focused). |
| Tauri bundler | Assumes a Tauri webview app; wrong runtime model. |
| electron-builder | Electron runtime; wrong for Rust-native binaries. |

### Honest limitations (documented, not faked)

cargo-packager does not expose the NSIS options-screen checkbox **default
states** (Start Menu ON / Desktop OFF / Launch ON) or the ARP `InstallLocation`
+ `DisplayIcon` writes as first-class keys. These are implemented via the
**supported** `installer_hooks` NSIS include at `packaging/windows/options.nsh`
— this is the documented extension point, not a workaround.

## Consequences

- Pinned `0.11.8`; a major config-schema break would require template
  adaptation (mitigated by the pin + CI guard).
- The framework's own validator (`tests/validate_config.py`) strips the
  `[itasha]` convention table before cargo-packager consumes the merged config.

## References

best-in-class-installer research §5 (recommended stack) + §1 (NSIS branded
default).

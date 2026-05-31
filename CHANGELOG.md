# Changelog

All notable changes to the Itasha.Corp Installer Framework are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Reusable, config-driven cross-platform installer framework scaffold.
- Shared `packager.template.toml` company-default template (publisher, install
  mode, ARP metadata, option-screen defaults, signing identity placeholders).
- Per-app override mechanism under `apps/<appname>.toml`, demonstrated with the
  `c0pl4nd.toml` override (Windows NSIS + macOS dmg + Linux AppImage/deb).
- Binary-input contract: the framework consumes a compiled app binary as a
  build input, never vendoring app source.
- IP-safety boundary checklist (`ships-publicly-vs-never.md`).
- Build wrapper scripts (POSIX `sh` + PowerShell) that invoke cargo-packager
  when present and fail loudly with an install hint when absent.
- Framework CI `validate.yml` (actionlint, shellcheck, TOML/manifest schema lint).

### Added (production hardening)
- Wired **gitleaks** secret-hygiene gate (CI job over working tree + full
  history) plus a hardened `.gitignore` excluding every key-shaped extension.
- Complete **SCR1B3** packaging parity: winget (3-file set), Homebrew cask, and
  Scoop manifest, plus an app-aware `update_manifests.py` resolver.
- A tested **onboarding-a-new-app** template (`docs/onboarding-a-new-app.md`).
- **WiX-MSI** track wired into `release.yml` alongside the NSIS default
  (honest-skip when the WiX v6 toolset is unavailable).
- **Binary-input acquisition** wired in `release.yml` (`gh release download` from
  the app's own repo by handle, dry-run fallback preserved).
- **release-verify** CI gate: per-OS install/verify (Windows Sandbox + matrix,
  macOS staple/spctl, Linux AppImage/.deb) plus an artifact-shape assertion that
  fails a release with a missing per-OS artifact.
- **reproducible-build** CI gate: double-builds the unsigned payload and asserts
  byte-identical SHA-256.
- **Headless native-installer crate** CI build + test + `cargo-audit`.
- BYO **cloud/HSM code-signing** option (`scripts/sign-cloud.ps1`, default-OFF)
  with a shared key-handle resolver and `docs/key-custody.md`.
- Operator **release/rollback/cert-rotation runbook** (`docs/release-runbook.md`)
  and a README documentation index.

### Changed (production hardening)
- macOS `sign-notarize-staple.sh`: the post-staple `stapler validate` +
  `spctl --assess` verdict is now **load-bearing** (the `|| true` swallow is
  removed); the credential-absent honest-skip path is unchanged.
- `content_safety_audit.py`: also asserts no git-**tracked** key-shaped path
  exists, and exempts detector-config files (`.gitleaks.toml`).

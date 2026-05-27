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

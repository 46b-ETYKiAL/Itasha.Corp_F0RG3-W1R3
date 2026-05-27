# Contributing to the Itasha.Corp Installer Framework

Thanks for helping build the installer framework. This document covers how the
framework is structured, how to add a new app, and the boundaries that keep the
public repository safe.

## What this framework is

A **reusable, config-driven** installer engine that packages any Itasha.Corp
application's **compiled binary** into branded, cross-platform installers
(Windows / macOS / Linux) from one shared template plus a small per-app
override. It is not a per-app one-off and it is not built into any application.

## The IP-safety boundary (read this first)

Before contributing anything, read [`ships-publicly-vs-never.md`](./ships-publicly-vs-never.md).
In short:

- **Ships publicly:** packaging config, installer UI text/scripts, branding
  assets, docs, CI workflow definitions that reference secrets *by name only*.
- **Never ships:** application source code, signing private keys
  (`.p12`/`.pfx`/`.p8`), API tokens, internal absolute paths, or any internal
  tooling reference.

The `tests/content_safety_audit` script enforces this boundary and runs in CI.
A pull request that fails the content-safety audit cannot merge.

## How to add a new app

1. Copy `apps/c0pl4nd.toml` to `apps/<yourapp>.toml`.
2. Edit the override (keep it small — roughly 15–20 lines):
   - `app.name`, `app.binary`, `app.identifier` (reverse-DNS bundle ID)
   - `app.icon`, `app.install_subdir` (the folder name under the company root)
   - per-target enable flags (`windows`, `macos`, `linux`)
3. Provide the app's brand assets under `branding/` (icon SVG, NSIS header /
   sidebar, dmg background) or point the override at shared defaults.
4. Validate locally: `python tests/validate_config.py apps/<yourapp>.toml`.
5. The shared `packager.template.toml` supplies every company default; you only
   override what differs.

## How signing works (dev-unsigned-until-creds)

The framework ships builds **unsigned** in development. Real signing requires
credentials the maintainer must acquire:

- **Windows:** an OV code-signing certificate in a cloud HSM / Key Vault.
- **macOS:** an Apple Developer Program account ($99/yr) for Developer ID
  signing + notarization.

Until those credentials exist, signing steps are **gated and skipped honestly**
— the build produces an unsigned dev artifact and prints the SmartScreen /
Gatekeeper warning explanation. Notarization is never faked. Secrets are
referenced **by name** from CI secrets / HSM only and never written to the repo.

## Local checks before opening a PR

```sh
# Lint workflows (if actionlint is installed)
actionlint .github/workflows/*.yml

# Shell sanity (if shellcheck is installed)
shellcheck packaging/**/*.sh scripts/*.sh

# Config + manifest schema validation
python tests/validate_config.py packager.template.toml apps/*.toml

# IP-safety boundary audit (must exit 0)
python tests/content_safety_audit.py
```

## Conventions

- Files / folders: kebab-case, lowercase.
- No tool versions left unpinned — pin every external tool referenced in CI.
- No deferred-work markers (`TODO`, `FIXME`, "coming soon"). If it is not done,
  it is descoped, not deferred.

## License

Contributions are dual-licensed under **MIT OR Apache-2.0** (see `LICENSE-MIT`
and `LICENSE-APACHE`). By contributing you agree your work is offered under both.

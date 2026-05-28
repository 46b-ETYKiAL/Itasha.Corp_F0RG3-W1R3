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
2. Edit the `[itasha.app]` table (keep it small — roughly 15–20 lines):
   - `product_name`, `binary`, `identifier` (reverse-DNS bundle ID)
   - `icon_ico` / `icon_png` / `icon_icns`, `install_subdir` (the folder name
     under the company root, e.g. `SCR1B3`)
   - `formats` (the subset of `nsis` / `dmg` / `appimage` / `deb` this app ships)
   - `desktop_file` for the Linux `.desktop` entry
3. Provide the app's brand assets under `branding/<yourapp>/` (icons, NSIS
   header / sidebar, dmg background) or point the override at shared defaults.
4. Validate locally:
   `python tests/validate_config.py packager.template.toml apps/<yourapp>.toml`.
   The resolved Windows install dir is printed as
   `<windows_install_root>\<install_subdir>`; confirm it lands under
   `C:\Program Files\Itasha.Corp\`.
5. The shared `packager.template.toml` supplies every company default; you only
   override what differs.

`apps/scribe.toml` (the SCR1B3 app) is a worked second example proving the
template is genuinely reusable across apps, not C0PL4ND-shaped.

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

## Local checks before opening a PR (pre-push equivalent)

Run these from the framework root (`apps/itasha-installer/`). They are the exact
local equivalent of the `lint-and-validate` job in `.github/workflows/ci.yml`;
the last two are **required gates** that block a PR from merging.

```sh
# 1. Lint the workflow definitions (CI runs actionlint 1.7.12, pinned).
actionlint .github/workflows/*.yml

# 2. Lint shell scripts (CI runs shellcheck + shfmt on packaging/scripts/tests).
shellcheck packaging/**/*.sh scripts/*.sh tests/**/*.sh

# 3. Schema-validate the shared template + every per-app override.
#    Prints each app's resolved Windows install dir; confirm it is under
#    C:\Program Files\Itasha.Corp\ .  REQUIRED gate.
python tests/validate_config.py packager.template.toml apps/*.toml

# 4. IP-safety / content-safety boundary audit — must exit 0.  REQUIRED gate.
python tests/content_safety_audit.py
```

**CI parity (confirmed):** `.github/workflows/ci.yml` `lint-and-validate` job
runs `python tests/validate_config.py packager.template.toml apps/*.toml`
followed by `python tests/content_safety_audit.py` as a REQUIRED gate on every
`pull_request`, alongside `actionlint`, `shellcheck`, `shfmt`, and `ruff`. A PR
that fails either of the two Python checks cannot merge. `actionlint` and
`shellcheck` may be absent on a local machine — they always run in CI, so an
honest local skip is acceptable; the two `python tests/...` checks have no
third-party dependency (`tomllib` is stdlib on Python 3.11+) and should always
pass locally before pushing.

## Conventions

- Files / folders: kebab-case, lowercase.
- No tool versions left unpinned — pin every external tool referenced in CI.
- No deferred-work markers (`TODO`, `FIXME`, "coming soon"). If it is not done,
  it is descoped, not deferred.

## License

Contributions are dual-licensed under **MIT OR Apache-2.0** (see `LICENSE-MIT`
and `LICENSE-APACHE`). By contributing you agree your work is offered under both.

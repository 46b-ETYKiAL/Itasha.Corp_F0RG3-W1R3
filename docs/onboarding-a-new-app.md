# Onboarding a New App

How to add a new application to the F0RG3-W1R3 installer framework. A new app is
a **small override**, not a fork — the framework's company defaults live in
`packager.template.toml`, and each app supplies only what differs. The two
reference apps are **C0PL4ND** (terminal) and **SCR1B3** (notes); copy whichever
is closer to your app and edit the values.

The framework consumes your app's **compiled binary** as a build input and never
vendors its source (see `binary-input-contract.md`). The content-safety audit
fails the build if an `apps/<app>/src/` tree is ever found.

---

## Files a new app needs

For an app named `<app>` (lowercase, e.g. `myapp`):

| # | File | Required? | Purpose |
|---|------|-----------|---------|
| 1 | `apps/<app>.toml` | Yes | The per-app override (merges over the template). |
| 2 | `branding/<app>/icon.svg` | Yes | Source icon (vector). |
| 3 | `branding/<app>/icon-256.png` | Yes | 256px PNG (Linux menu icon, NSIS). |
| 4 | `branding/<app>/icon.ico` | Yes (Windows) | Windows icon. |
| 5 | `branding/<app>/icon.icns` | Yes (macOS) | macOS bundle icon. |
| 6 | `packaging/linux/<app>.desktop` | Yes (Linux) | Freedesktop menu entry. |
| 7 | `packaging/manifests/winget/ItashaCorp.<APP>.installer.yaml` | For winget | winget installer manifest. |
| 8 | `packaging/manifests/winget/ItashaCorp.<APP>.locale.en-US.yaml` | For winget | winget default-locale manifest. |
| 9 | `packaging/manifests/winget/ItashaCorp.<APP>.yaml` | For winget | winget version manifest. |
| 10 | `packaging/manifests/homebrew/<app>.rb` | For Homebrew | Homebrew cask. |
| 11 | `packaging/manifests/scoop/<app>.json` | For Scoop | Scoop manifest. |

winget requires ALL THREE of files 7-9 (a single-file submission is schema-
incomplete). All three MUST carry the same `PackageIdentifier`.

Nothing else changes: `scripts/build.{sh,ps1}`, `release.yml`,
`packaging/windows/options.nsh`, the signing scripts, and the verify scripts are
shared and app-agnostic. `packaging/manifests/update_manifests.py` discovers
every app's manifest set automatically (it globs the winget version manifests +
scoop `*.json` + homebrew `*.rb`).

---

## 1. The per-app override (`apps/<app>.toml`)

Copy `apps/scribe.toml` and edit. Keep it SMALL — only what differs from the
template. The resolved Windows install dir = `windows_install_root` (template) +
`install_subdir`.

```toml
[itasha.app]
product_name    = "MYAPP"
identifier      = "corp.itasha.myapp"
description     = "Short one-line description"
long_description = "MYAPP is a … from Itasha.Corp."
homepage        = "https://github.com/itasha-corp/myapp"
install_subdir  = "MYAPP"
# Compiled binary consumed as a build input (never vendored source).
binary    = "myapp"
icon_ico  = "branding/myapp/icon.ico"
icon_png  = "branding/myapp/icon-256.png"
icon_icns = "branding/myapp/icon.icns"
formats   = ["nsis", "dmg", "appimage", "deb"]
desktop_file = "myapp.desktop"
# Linux .deb runtime deps (GUI apps): add as needed.
# deb_depends = ["libc6", "libxkbcommon0"]

# --- Optional native-installer brand accent ---
[itasha.brand]
voice_hex = "#8CE8B4"
kanji     = "記録"
tagline   = "A short brand line."
```

Validate the override merges cleanly:

```bash
python tests/validate_config.py packager.template.toml apps/myapp.toml
# OK  myapp.toml   (resolved install dir = C:\Program Files\Itasha.Corp\MYAPP)
```

---

## 2. Branding

Drop the icons into `branding/<app>/`. The 256px PNG is the canonical Linux menu
icon; the `.ico`/`.icns` are the Windows/macOS bundle icons. SCR1B3 ships
`icon.svg` + `icon-mono.svg` as the source set; export the raster forms from
those.

---

## 3. Package-manager manifests

Copy the SCR1B3 set and rename, replacing the identity:

```bash
# winget (THREE files — all required, matching PackageIdentifier):
cp packaging/manifests/winget/ItashaCorp.SCR1B3.installer.yaml \
   packaging/manifests/winget/ItashaCorp.MYAPP.installer.yaml
cp packaging/manifests/winget/ItashaCorp.SCR1B3.locale.en-US.yaml \
   packaging/manifests/winget/ItashaCorp.MYAPP.locale.en-US.yaml
cp packaging/manifests/winget/ItashaCorp.SCR1B3.yaml \
   packaging/manifests/winget/ItashaCorp.MYAPP.yaml
# Homebrew + Scoop:
cp packaging/manifests/homebrew/scr1b3.rb packaging/manifests/homebrew/myapp.rb
cp packaging/manifests/scoop/scr1b3.json  packaging/manifests/scoop/myapp.json
```

Then replace `SCR1B3` -> `MYAPP`, `scr1b3` -> `myapp`, and the description /
homepage in each. Leave the `${VERSION}` / `__VERSION__` / `__SHA256__`
placeholders intact — CI substitutes them from the actual artifact bytes
(`update_manifests.py`); hashes are NEVER hand-edited (WezTerm #7713 drift).

---

## 4. Build, verify, release

```bash
# Local dry-run (no binary, no cargo-packager needed):
./scripts/build.sh --app myapp --dry-run

# Real build (supply the compiled binary):
ITASHA_BINARY_PATH=/path/to/myapp ./scripts/build.sh --app myapp

# Release: tag the framework; CI builds + signs + verifies (see release-runbook.md).
```

The release pipeline and the `release-verify` / `reproducible-build` gates are
shared, so a new app gets the full cross-platform build + install-verify +
artifact-shape + reproducibility coverage with zero workflow edits.

---

## Reference examples

- **C0PL4ND** — `apps/c0pl4nd.toml` (terminal; ships the `zip` portable lane,
  `deb_depends`, updater feed, and the `[itasha.brand]` accent).
- **SCR1B3** — `apps/scribe.toml` (notes; the minimal second-consumer override
  that proves the template's reusability).

A future Itasha.Corp app (for example a desktop onboarding app) follows this
exact template — a small override plus branding plus manifests, no fork.

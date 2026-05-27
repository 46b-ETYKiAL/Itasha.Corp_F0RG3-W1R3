# Binary-Input Contract

How the installer framework consumes an application's **compiled binary** as a
build input — **without ever vendoring the application's source**.

## Principle

An installer packages an already-built executable. The framework therefore
treats each app's binary as an *external input*, resolved at build time from one
of the sources below. There is no `src/` of any packaged app in this
repository, and the content-safety audit (`tests/content_safety_audit.py`)
fails the build if an app source tree is ever found.

## The contract

The per-app override (`apps/<appname>.toml`) declares a logical binary **name**
(e.g. `binary = "c0pl4nd"`). The build wrapper resolves that name to a concrete
path at build time using exactly one of these resolution sources, in priority
order:

| # | Source | How it is supplied | When used |
|---|--------|--------------------|-----------|
| 1 | `ITASHA_BINARY_PATH` env var | Absolute path to a pre-built binary | Local maintainer builds, manual packaging |
| 2 | `--binary <path>` wrapper flag | CLI argument to `scripts/build.sh` / `build.ps1` | Scripted / one-off builds |
| 3 | CI release artifact | A prior CI job builds the app in **its own repo** and uploads the binary; the packaging job downloads it into a scratch `binaries/` dir (git-ignored) | CI release pipeline (cross-repo) |

The resolved path is substituted into the `${APP_BINARY_PATH}` placeholder of
the merged cargo-packager config. The framework copies that single file into the
package; it never reads, imports, or commits any surrounding source.

## What crosses the boundary

```
  App repo (source, possibly private)              Installer framework (public)
  ┌─────────────────────────────────┐              ┌──────────────────────────┐
  │  src/  Cargo.toml  tests/        │              │  packager.template.toml  │
  │            │                     │              │  apps/<app>.toml         │
  │            ▼  cargo build        │   BINARY     │  packaging/  branding/   │
  │   target/release/<app>(.exe) ────┼── only ─────▶│  scripts/build.{sh,ps1}  │
  └─────────────────────────────────┘   crosses    └──────────────────────────┘
```

Only the compiled binary (plus its public name/version/icon) crosses. Source
stays in the app's own repository (two-repo split, `ships-publicly-vs-never.md`
rule 2).

## Dry-run resolution

`scripts/build.sh --dry-run --app c0pl4nd` (and the PowerShell equivalent)
performs the full merge + placeholder resolution against a **placeholder** binary
path and prints the resolved config + the resolved Windows install directory,
**without** requiring the real binary or cargo-packager to be installed. This is
how the engine phase verifies the contract end-to-end locally:

```
$ python tests/validate_config.py packager.template.toml apps/c0pl4nd.toml
OK  packager.template.toml        (valid TOML, [itasha] marker present)
OK  apps/c0pl4nd.toml             (valid override; resolved install dir =
                                   C:\Program Files\Itasha.Corp\C0PL4ND)
```

No reference to any app's `src/` appears anywhere in the resolution path.

## Why not vendor the binary into the repo?

Committing binaries bloats history, defeats the two-repo split, and risks
shipping a pre-release/internal build publicly. Binaries are **release
artifacts**: downloaded fresh per build, verified by `checksum.sha256`, and
discarded. The `.gitignore` blocks `binaries/`, `inputs/`, and bare `*.exe` /
`*.dll` / `*.dylib` / `*.so` for exactly this reason.

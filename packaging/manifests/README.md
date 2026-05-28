# Package-Manager Manifests

Templates that point package managers at the **signed release artifacts** for
the three install one-liners:

```sh
winget install ItashaCorp.C0PL4ND
brew install --cask itasha-corp/tap/c0pl4nd
scoop bucket add itasha-corp https://github.com/itasha-corp/scoop-bucket
scoop install itasha-corp/c0pl4nd
```

| Manager | File | Bucket / tap | Install scope / placement |
|---|---|---|---|
| winget | `winget/ItashaCorp.C0PL4ND*.yaml` | `microsoft/winget-pkgs` (via bump PR) | per-machine (`Scope: machine`, D4) |
| Homebrew cask | `homebrew/c0pl4nd.rb` | `itasha-corp/homebrew-tap` | `.app` → `/Applications/C0PL4ND.app` |
| Scoop | `scoop/c0pl4nd.json` | `itasha-corp/scoop-bucket` | per-user (NSIS `.exe`) |

## Hashes are never hand-edited (closed structurally)

The WezTerm #7713 hash-drift class — a hand-typed `InstallerSha256` no longer
matching the published binary — is closed two ways, neither of which ever
hand-edits a hash:

1. **Managed auto-bump (preferred)** — `.github/workflows/package-bump.yml`
   runs on `release: published`. **WinGet Releaser** (invoking Komac) and
   **Homebrew bump-cask** each download the signed asset and compute the hash
   *themselves* before opening a bump PR. **Scoop** self-bumps: its manifest
   carries `checkver` + `autoupdate`, so `scoop update` resolves the new
   version and recomputes the hash autonomously from the asset's `.sha256`
   sidecar. Every step is gated on the signed asset existing — an absent asset
   emits a `::warning::` and is skipped (no hash is fabricated).
2. **Local fallback** — `update_manifests.py` derives every hash from the
   artifact's own bytes on disk (preferring the release `checksum.sha256` else
   recomputing) for offline inspection or a manual, user-authorized submission.

## Submission flow

The auto-bump workflow **opens PRs** to the package-manager repos
(`microsoft/winget-pkgs` via WinGet Releaser; the Homebrew tap via bump-cask).
Those PRs are reviewed/merged by the package-manager maintainers — opening a
bump PR is the start of that review flow, not an unattended package deploy.
Tokens for the fork/tap PRs are repo secrets referenced **by name only**
(`WINGET_PKGS_TOKEN`, `HOMEBREW_TAP_TOKEN`); none are stored in this repo.

The local `update_manifests.py` path instead emits **resolved** manifests as a
build artifact (`resolved-manifests`) and does NOT auto-submit anything; that
manual path stays gated on `website_publish_consent_required`.

## Run locally

```sh
python packaging/manifests/update_manifests.py \
  --version 1.2.3 \
  --release-dir dist \
  --manifest-root packaging/manifests \
  --out-dir resolved-manifests \
  --repo itasha-corp/c0pl4nd
```

A platform with no artifact in `--release-dir` is skipped with a loud message
(exit 0). Only a usage/IO error exits non-zero.

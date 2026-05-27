# Package-Manager Manifests

Templates that point package managers at the **signed release artifacts**.
They are parameterised; the CI release job (`.github/workflows/release.yml`)
fills version / URL / hash placeholders from the actually-uploaded artifacts
via `update_manifests.py`.

| Manager | File | Placeholder style | Install scope |
|---|---|---|---|
| winget | `winget/ItashaCorp.C0PL4ND*.yaml` | `${VERSION}` `${INSTALLER_URL}` `${INSTALLER_SHA256}` | per-machine (`Scope: machine`, D4) |
| Homebrew cask | `homebrew/c0pl4nd.rb` | `__VERSION__` `__SHA256__` `__URL__` | macOS `.dmg` |
| Scoop | `scoop/c0pl4nd.json` | `__VERSION__` `__INSTALLER_URL__` `__INSTALLER_SHA256__` | per-user (NSIS `.exe`) |

## Hashes are never hand-edited

`update_manifests.py` derives every hash from the artifact's own bytes
(preferring the release `checksum.sha256` else recomputing). This closes the
WezTerm #7713 hash-drift failure mode where a hand-typed `InstallerSha256` no
longer matched the published binary.

## Submission is a separate, user-authorized step

CI emits **resolved** manifests as a build artifact (`resolved-manifests`). It
does NOT auto-submit them to `microsoft/winget-pkgs`, a Homebrew tap, or a
Scoop bucket. Publishing is gated on `website_publish_consent_required`.

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

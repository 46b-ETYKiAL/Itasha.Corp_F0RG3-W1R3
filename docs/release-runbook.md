# Release Runbook — F0RG3-W1R3 Installer Framework

Operator-facing procedure for cutting, verifying, yanking, and rolling back a
release, plus signing-credential rotation and the gated-not-faked degradation
matrix. This is the on-call reference for the release pipeline
(`.github/workflows/release.yml` + `release-verify.yml`).

The pipeline is **tag-gated**: a release fires only on a `v*` tag push (never on
an ordinary push). The `publish` job is the sole holder of `contents: write`.
No package-manager submission is automated — winget/Homebrew/Scoop manifests are
emitted as artifacts for manual maintainer submission.

---

## 1. Pre-release checklist

Run locally before tagging. All must be green.

| Check | Command | Pass condition |
|---|---|---|
| Config merges | `python tests/validate_config.py packager.template.toml apps/*.toml` | exit 0 for every app |
| Content-safety / IP-boundary | `python tests/content_safety_audit.py` | exit 0, "0 leakage findings" |
| Secret hygiene (tree) | `gitleaks detect --source . --no-git --config .gitleaks.toml --redact` | exit 0, "no leaks found" |
| Secret hygiene (history) | `gitleaks detect --source . --config .gitleaks.toml --redact` | exit 0, "no leaks found" |
| Shell lint | `shellcheck --severity=warning packaging/**/*.sh scripts/*.sh tests/**/*.sh` | exit 0 |
| Shell format | `shfmt -d -i 2 -ci <each .sh>` | empty diff |
| Workflow lint | `actionlint -color` | exit 0 |
| Reproducible payload | `./scripts/reproducible-build.sh --app <app>` | "REPRODUCIBLE" (or honest tool-absent skip) |
| Native crate (headless) | `cargo build --manifest-path installer/Cargo.toml --no-default-features` | exit 0 |

---

## 2. Cut a release (real, signed)

```bash
# 1. Decide the version (semver, no leading v in Cargo, leading v on the tag).
VERSION=1.2.3

# 2. Ensure the per-app binary input is published on the APP's own repo release.
#    The framework consumes the compiled binary as a BUILD INPUT (it never
#    vendors app source). See docs/binary-input-contract.md.

# 3. Tag and push. This is the only trigger.
git tag -a "v${VERSION}" -m "Release v${VERSION}"
git push origin "v${VERSION}"
```

The `release.yml` matrix then builds Windows (NSIS .exe + WiX .msi), macOS
(.dmg + notarize + staple), and Linux (.AppImage + .deb) on native runners,
signs each tier when its credential is present, emits checksums + CycloneDX SBOM
+ SLSA-L2 provenance + minisign + cosign-keyless signatures, and the `publish`
job attaches everything to the GitHub Release. `release-verify.yml` then runs the
per-OS install/uninstall + artifact-shape gate.

### 2a. Dry-run (no binary input, no secrets)

```bash
gh workflow run release.yml -f app=c0pl4nd   # workflow_dispatch
```

With no `ITASHA_BINARY_PATH`, the build runs in config-dry-run mode
(`dry_run=1`): it validates the packaging config path and exits without
producing or signing artifacts. Honest `::notice::` is emitted; nothing is
published.

---

## 3. Verify a published release (any user, offline)

```bash
# Download the artifact + its checksum.sha256 + .minisig + the public key.
./scripts/verify.sh <artifact> keys/minisign.pub
# => "==> VERIFIED" on a match; non-zero on mismatch.
```

Per-OS deeper verification mirrors `release-verify.yml`:

- **Windows:** `tests/windows/matrix.ps1` (+ `sandbox.wsb` install/uninstall).
- **macOS:** `tests/macos/verify.sh --app <App.app> --dmg <Disk.dmg>` (asserts
  `codesign --verify`, `spctl --assess`, `stapler validate`).
- **Linux:** `tests/linux/verify.sh --appimage <f> --deb <f>` (AppImage smoke,
  `dpkg` install/remove, checksum + minisign).

---

## 4. Yank / rollback a bad release

A GitHub Release is the unit of distribution. To withdraw one:

```bash
VERSION=1.2.3

# 1. Delete the release (removes the download surface immediately).
gh release delete "v${VERSION}" --yes

# 2. Delete the tag locally and on the remote (so the version is reusable
#    only after a re-cut from a corrected commit; never reuse a tag for
#    different bytes — that breaks the checksum/minisign trust chain).
git tag -d "v${VERSION}"
git push origin ":refs/tags/v${VERSION}"

# 3. If a package-manager manifest was already submitted, withdraw it:
#    - winget:    open a removal PR against microsoft/winget-pkgs
#    - Homebrew:  bump the cask back / open a removal PR on the tap
#    - Scoop:     revert the bucket manifest commit
#    (No submission is automated, so a manifest is withdrawn the same way it
#     was submitted — by a maintainer PR.)
```

After withdrawal, re-cut from a corrected commit with the NEXT patch version
(e.g. `v1.2.4`). Do not re-publish different bytes under an already-distributed
tag.

The codebase itself is rolled back by `git revert` of the offending commit(s);
no pipeline change deletes any existing script or workflow, so a branch revert
always restores the prior state.

---

## 5. Signing-credential rotation

All signing material is referenced BY NAME from the CI secret store / Keychain /
HSM handle — never committed (enforced by `.gitleaks.toml` + the content-safety
audit). Rotation replaces the stored value behind the same name.

| Credential | Secret name(s) | Rotation |
|---|---|---|
| Windows Authenticode | `WINDOWS_CERT_THUMBPRINT` (+ HSM handle for cloud-signing, see `docs/key-custody.md`) | Issue/import the new cert; update the secret to the new thumbprint/handle. Always sign with the SAME publisher identity to preserve SmartScreen reputation (ADR-0003). |
| Apple Developer ID | `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` (or `APPLE_API_KEY_ID`/`APPLE_API_ISSUER`/`APPLE_API_KEY_PATH`) | Regenerate the app-specific password or ASC API key in App Store Connect; update the matching secret. |
| minisign | `MINISIGN_SECRET_KEY` (secret) + `keys/minisign.pub` (public, committed at release) | Generate a new keypair (`scripts/gen-minisign-key.sh`); update the secret; commit the new public key; announce the key change in the release notes. |
| cosign keyless | none (OIDC, ephemeral) | No stored key to rotate — identity is the workflow's OIDC subject. |

After rotating, cut a throwaway dry-run (§2a) and confirm the signing step emits
the credential-present `::notice::` rather than the absent `::warning::`.

---

## 6. Gated-not-faked degradation matrix

Signing is GATED on credential presence and is NEVER faked. When a credential is
absent, the pipeline degrades to an honest, documented state — it does not break
and it does not fabricate a signature, ticket, or attestation.

| Credential absent | Build outcome | What ships | User-visible consequence |
|---|---|---|---|
| `WINDOWS_CERT_THUMBPRINT` | builds UNSIGNED, `::warning::` | .exe/.msi + minisign + cosign-keyless + checksum | SmartScreen warns on first run until a stable signing identity earns reputation (ADR-0003). |
| Apple Developer ID secrets | builds UNSIGNED + ad-hoc, `::warning::` | .dmg + minisign + cosign-keyless + checksum | Gatekeeper hard-blocks on Sequoia+ (no "Open Anyway"); user must self-authorize. Notarization is NEVER faked. The `release-verify` staple assertion is **skipped-not-failed** with a structured reason. |
| `MINISIGN_SECRET_KEY` | builds, `::warning::` | artifacts + checksum + cosign-keyless (no .minisig, no updater feed) | The auto-update feed (`latest.json`) is skipped, not faked. |
| cosign OIDC token | builds, `::warning::` | artifacts + checksum + minisign (no cosign sig) | One fewer transparency signature; integrity still verifiable via minisign + checksum. |
| `dist/` empty on a runner | SLSA provenance honest-skip, `::warning::` | nothing on that runner | The `release-verify` artifact-shape assertion turns the missing per-OS artifact into a hard `::error::` — the release does not silently publish a partial set. |

The single golden signal is the `ci-gate` / `release-verify` roll-up: a skipped
job can never report success (explicit-`if` + `always()` per
`merge-queue-check-gating`). A release that fails install-verify or artifact-shape
never reaches `publish`.

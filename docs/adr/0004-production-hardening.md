# ADR-0004 — Production Hardening (12-Gap Closure)

- **Status:** Accepted
- **Date:** 2026-05-31
- **Deciders:** Itasha.Corp installer framework
- **Supersedes:** none (extends ADR-0001 engine, ADR-0002 reserved MSI, ADR-0003 signing posture)

## Context

The framework already built Windows NSIS + WiX-MSI, macOS `.dmg` + notarize +
staple, and Linux AppImage + `.deb` from one cargo-packager config; signed
Authenticode + macOS notarize + minisign + cosign-keyless; and emitted CycloneDX
SBOM + SLSA-L2 provenance + checksums. A read-only audit surfaced 12
production-readiness gaps (G1-G12) where the surface was present but unwired,
non-load-bearing, or incomplete. This ADR records the architecture of the
hardening that closes them. The framework is **not rebuilt** — every change is
additive and extends an existing surface.

## Decision

Close the 12 gaps through wiring, assertion, and documentation, preserving the
**gated-not-faked** invariant throughout.

### 1. Secret hygiene (G5)

A hardened `.gitleaks.toml` (org key-shape rules + template-placeholder
allowlist) is wired as a required CI job over both the working tree and full
history, backed by a `.gitignore` that excludes every key-shaped extension and a
content-safety audit that fails on any tracked key-shaped path. Detector-config
files (`.gitleaks.toml`) are exempt from the content-pattern scan exactly as the
auditor exempts itself.

### 2. App onboarding (G1)

SCR1B3 gains full winget (3-file set) / Homebrew / Scoop parity, and
`update_manifests.py` is now app-aware (globs every manifest set). A tested
onboarding template (`docs/onboarding-a-new-app.md`) makes a new app a small
override, not a fork.

### 3. Build-track wiring (G2, G3, G7)

The reserved WiX-MSI track is wired into `release.yml` (honest-skip when the WiX
v6 toolset is unavailable; ADR-0002 version-match lesson preserved). Binary-input
acquisition resolves the app binary from the app's own repo by handle
(`gh release download`), with the dry-run fallback preserved. The native
installer crate gets a headless `cargo build --no-default-features` + `cargo test`
+ `cargo-audit` CI job.

### 4. The gated-not-faked invariant + the load-bearing macOS verdict (G8)

Signing is GATED on credential presence and NEVER faked. The root-cause fix in
`sign-notarize-staple.sh` removes the `|| true` that swallowed the post-staple
`spctl --assess` / `stapler validate` verdict: on the creds-present path a failed
Gatekeeper assertion now hard-fails the build (Apple's "notarization passing is
not Gatekeeper passing" guidance). The credential-absent path is unchanged —
honest unsigned + minisign + cosign-keyless, never a fabricated ticket. The CI
`|| true` that re-swallowed the verdict at the workflow level is also removed.

### 5. Release-verify gate topology (G4, G6)

`release-verify.yml` is a per-OS matrix (fail-fast:false) that asserts
ARTIFACT-SHAPE — every expected `.exe`/`.msi`/`.dmg`/`.AppImage`/`.deb` is present
and non-empty — and invokes the now-wired per-OS verify scripts. A missing per-OS
artifact on a real build is a hard `::error::`; the roll-up uses explicit-`if` +
`always()` so skipped is not success.

### 6. Reproducible-build gate (G11)

`reproducible-build.yml` double-builds the unsigned payload via
`reproducible-build.sh` (root-cause-fixed to use build.sh's supported flags) and
asserts byte-identical SHA-256 under a pinned `SOURCE_DATE_EPOCH`.

### 7. BYO key-custody abstraction (G9)

`scripts/sign-cloud.ps1` (default-OFF) signs through a cloud HSM/KMS by key
HANDLE — never a value — via free OSS osslsigncode (PKCS#11) or jsign (cloud-KMS),
per the CA/B-Forum 2023 hardware-key mandate. A shared `_sign-key-resolver.ps1`
serves both it and the default thumbprint path. `docs/key-custody.md` records the
handle discipline and the cross-platform-signer coordination note (an OPTIONAL
future integration, NOT a dependency).

### 8. Operability (G10, G12)

An operator `release-runbook.md` (cut / verify / yank / rollback / cert-rotation
/ degradation matrix), a README documentation index, and CHANGELOG parity.

## Consequences

- A release that fails install-verify, artifact-shape, the secret-hygiene gate,
  or (creds-present) the macOS Gatekeeper assertion never publishes.
- No existing script or workflow is deleted; the change set is fully
  `git revert`-able.
- cargo-packager stays pinned at 0.11.8 (no bump); no new required third-party
  dependency is introduced (BYO cloud signing references free OSS by handle).
- The gated-not-faked invariant is now load-bearing rather than advisory: a
  credential-absent run degrades honestly; it never fabricates a signature,
  notarization ticket, or attestation.

## References

- ADR-0001 (engine), ADR-0002 (reserved MSI), ADR-0003 (signing posture).
- `docs/release-runbook.md`, `docs/key-custody.md`, `docs/onboarding-a-new-app.md`.
- `.github/workflows/release-verify.yml`, `reproducible-build.yml`, `ci.yml`, `release.yml`.
- CA/B-Forum Baseline Requirements (2023-06-01 hardware-key mandate).

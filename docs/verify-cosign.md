# Verifying a Release — cosign + GitHub Artifact Attestations

This page documents the two **supply-chain provenance** verification paths the
release pipeline produces in 2026:

1. **`gh attestation verify`** — confirms an artifact was built from this
   source, in this workflow (SLSA build provenance, keyless Sigstore).
2. **`cosign verify-blob`** — an OS-agnostic, GitHub-independent integrity +
   signer check over the native installer (`.exe` / `.dmg` / `.AppImage`).

Both are **free** and **keyless** (no long-lived signing key). They complement
— they do not replace — the SHA-256 + **minisign** checks documented in
`scripts/verify.sh` / `scripts/verify.ps1` (see ADR-0003 §5, Tier 0). Use
whichever your tooling supports; for the strongest 2026 guarantee use the
attestation path.

> Honest note: when the release ran without an OIDC token (e.g. a fork build
> with no `id-token: write`), the pipeline emits a LOUD warning and ships the
> artifact **without** a `.sig`/`.pem` rather than faking one. In that case only
> the SHA-256 + minisign checks are available. Nothing signed is ever
> fabricated. See ADR-0003 §4 (the gating rule).

---

## 1. `gh attestation verify` (SLSA build provenance)

Every release artifact carries a build-provenance attestation produced by
`actions/attest-build-provenance` (SLSA v1.0 build level 2, keyless via the
workflow's ambient OIDC identity and the public Sigstore transparency log).

### Online

```bash
# Replace <org> with the GitHub organization that owns the release repo.
gh attestation verify <downloaded-artifact> --owner <org>
```

A green result means: this exact file was produced by a workflow in `<org>`,
and the provenance is recorded in the public transparency log.

You can also pin to the specific repository instead of the owner:

```bash
gh attestation verify <downloaded-artifact> --repo <org>/<repo>
```

### Offline (air-gapped)

First, on a networked machine, download the attestation bundle for the
artifact, then verify with the bundle on the offline host:

```bash
# (networked) fetch the bundle next to the artifact:
gh attestation download <downloaded-artifact> --owner <org>

# (offline) verify against the local bundle — no network call:
gh attestation verify <downloaded-artifact> \
  --owner <org> \
  --bundle <downloaded-artifact>.sigstore.json \
  --offline
```

`--offline` skips the live transparency-log lookup and trusts the embedded,
already-fetched bundle. Use it on air-gapped or egress-restricted hosts.

---

## 2. `cosign verify-blob` (keyless OIDC, GitHub-independent)

The pipeline also runs `cosign sign-blob` (keyless) over each native installer,
publishing `<artifact>.sig` (signature) and `<artifact>.pem` (the short-lived
Fulcio certificate) next to `checksum.sha256`. This lets users verify with the
Sigstore toolchain without depending on the GitHub API.

Install cosign: <https://docs.sigstore.dev/cosign/installation/>

```bash
cosign verify-blob <downloaded-artifact> \
  --signature   <downloaded-artifact>.sig \
  --certificate <downloaded-artifact>.pem \
  --certificate-identity-regexp   "^https://github\.com/<org>/.+" \
  --certificate-oidc-issuer        "https://token.actions.githubusercontent.com"
```

What each flag asserts:

| Flag | Asserts |
|---|---|
| `--signature` / `--certificate` | the detached signature + the cert that produced it |
| `--certificate-identity-regexp` | the signer was a workflow under your `<org>` (pin to the exact workflow ref for a stricter check) |
| `--certificate-oidc-issuer` | the cert was minted by GitHub Actions OIDC, not some other issuer |

A green result means: this file was signed by a GitHub Actions workflow in
`<org>` whose identity matches, and the signature is valid.

> Pin tighter when you can. Replace the regexp with the full workflow identity,
> e.g. `https://github.com/<org>/<repo>/.github/workflows/release.yml@refs/tags/<tag>`,
> to bind the signature to one specific workflow + tag.

---

## 3. Which check should I run?

| You have… | Use |
|---|---|
| `gh` CLI + network | `gh attestation verify` (§1) — strongest, simplest |
| `gh` CLI + air-gapped host | `gh attestation verify --offline --bundle …` (§1) |
| cosign, no GitHub dependency | `cosign verify-blob` (§2) |
| nothing but a checksum file | SHA-256 + minisign via `scripts/verify.sh` (ADR-0003 §5) |

All four are independent and free. Running more than one is strictly better.

---

## References

- best-in-class-installer-2026-05-27.md §provenance (P0-1) and §cosign (P1-5).
- GitHub Artifact Attestations — concepts + offline verification (`gh attestation verify`).
- Sigstore cosign — `verify-blob`, keyless OIDC signing.
- ADR-0003 §4 (gating rule), §5 (free-signing tiers: minisign / self-signed / ad-hoc).

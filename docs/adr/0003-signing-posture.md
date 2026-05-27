# ADR-0003 — Code-Signing Posture (Windows / macOS / Linux)

- **Status:** Accepted
- **Date:** 2026-05-26
- **Deciders:** Itasha.Corp installer framework
- **Supersedes:** none

## Context

The framework produces installers that public users download and run. The two
desktop platforms that gate execution on a publisher signature are Windows
(SmartScreen) and macOS (Gatekeeper). Signing is therefore a **release**
concern, not a build-correctness concern: an unsigned installer still
*builds*, it just earns a friction warning on first run.

This ADR records the signing posture, the real external dependencies it
implies, and the non-negotiable rule that **signing is gated on credential
presence — it is never faked**.

## Decision

### 1. Windows — OV code-signing certificate via cloud HSM

- Use an **Organization Validation (OV)** certificate, not Extended Validation
  (EV). Since **2024 Microsoft no longer grants instant SmartScreen reputation
  to EV-signed binaries**; reputation now builds organically for any stable
  signing identity. The EV premium buys no reputation head-start, so OV is the
  decision.
- Reputation is earned by **always signing with the same identity**. A new
  certificate (OV or EV) starts at zero reputation and incurs a SmartScreen
  "unrecognized app" prompt for roughly the first few weeks / download-volume
  of each new version until reputation accrues.
- The private key MUST live in a **cloud HSM / Key Vault** and never touch a
  repo or a build-runner disk in plaintext. This is also the **post-June-2023
  CA/Browser Forum requirement**: code-signing private keys must be generated
  and stored on FIPS 140-2 Level 2 (or Common Criteria EAL 4+) hardware — file-
  based `.pfx` issuance for new OV/EV certs ended in June 2023.
- Provider selection (in preference order, all acceptable):
  1. **Azure Trusted Signing** (formerly Azure Code Signing) — cheapest, fully
     HSM-backed, but eligibility is restricted: the signing org must be a
     US/Canada legal entity ≥ 3 years old. If eligible, use it.
  2. **DigiCert / SSL.com / Certum** OV via their cloud-HSM/KSP offering, if the
     org does not meet the Azure eligibility bar.
  3. **SignPath Foundation** free certificate **for OSS projects only** — note
     the publisher then displays as "SignPath Foundation", not "Itasha.Corp".
- Signing tool: `signtool` (Windows SDK) or the provider's cloud-signing CLI,
  driven by env vars **read by name** (`WINDOWS_CERT_THUMBPRINT`,
  `AZURE_*` / provider credentials) from CI secrets.

### 2. macOS — Apple Developer ID + notarize + staple ($99/yr REAL dependency)

- Sign the `.app` with a **Developer ID Application** certificate, submit the
  `.dmg` to **Apple notarization** (`notarytool`), then **staple** the ticket
  to the `.dmg` so Gatekeeper passes **offline**.
- This requires an **Apple Developer Program membership ($99/yr)**. There is
  **no free or self-hosted alternative** that clears Gatekeeper for
  distribution — ad-hoc signing only suppresses the dialog on the building
  machine. **This is a real, non-deferrable external dependency.**
- Credentials are read **by name** from CI secrets: `APPLE_SIGNING_IDENTITY`,
  `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` (or the App Store Connect
  API key trio `APPLE_API_KEY_ID` / `APPLE_API_ISSUER` / `APPLE_API_KEY_PATH`).
- The pipeline lives at `packaging/macos/sign-notarize-staple.sh`.

### 3. Linux — no mandatory signing surface

- AppImage and `.deb` are not gated on a publisher signature by the OS.
- Integrity is provided by the **published `checksum.sha256` + minisign public
  key** (the private minisign key follows the same HSM/secret rule). `.deb`
  repository signing (`dpkg-sig` / `debsign`) is an optional, separate
  publish-time step and is out of scope for v1 build correctness.

### 4. The gating rule (HARD)

> **Signing is gated on credential presence. When the signing credentials are
> absent, the build ships an UNSIGNED dev-only artifact with a documented
> SmartScreen/Gatekeeper warning. Notarization is NEVER faked, and a
> dev-unsigned artifact is NEVER presented as signed.**

This rule is enforced structurally:
- `packaging/macos/sign-notarize-staple.sh` detects absent credentials, prints
  the dev-unsigned warning, and exits 0 without producing a fake ticket.
- The release workflow (`.github/workflows/release.yml`) checks each platform's
  secret presence and emits a **loud "BUILDING UNSIGNED — credentials absent"**
  notice when signing is skipped.
- No signing secret is ever stored in the repository (`secret_leakage_to_public_repo`
  fork + `tests/content_safety_audit.py` + `.gitignore`).

### 5. Free-signing tiers (the wired default — no paid dependency)

Before any paid certificate exists, the framework signs **as much as is
possible for free**, so every release carries cryptographic authenticity even
without OV/Apple credentials. These tiers are wired into `release.yml` and run
on every build; each is honest about what it does and does not buy.

| Tier | Tool | Cost | Covers | Buys you | Does NOT buy |
|---|---|---|---|---|---|
| **0 — minisign** (universal) | minisign (Ed25519) | free, no CA | Win + macOS + Linux artifacts | cryptographic authenticity + integrity any downloader can verify offline with the public key | OS publisher trust (not an Authenticode/Gatekeeper identity) |
| **1a — Windows self-signed** | `signtool` / `Set-AuthenticodeSignature` | free | `.exe` / `.msi` | a stable Authenticode identity + **enterprise allow-listing** (import the `.cer` into Trusted Publishers via GPO/Intune) | public SmartScreen trust (general users still warned) |
| **1b — macOS ad-hoc** | `codesign --sign -` | free | `.app` | local-machine integrity + hardened-runtime/Apple-silicon execution | Gatekeeper clearance for downloaded apps |
| **2 — paid** | OV/EV cert + Apple Developer ID | paid (§1, §2) | `.exe` / `.dmg` | warning-free public installs | — |

Tooling (all free, no service):
- `scripts/gen-minisign-key.sh` — one-time keypair; public half committed to
  `keys/minisign.pub`, secret half → `MINISIGN_SECRET_KEY` CI secret.
- `scripts/sign-artifacts.sh` — signs every `dist/` artifact to `.minisig`.
- `scripts/gen-selfsigned-cert.ps1` + `scripts/sign-windows.ps1` — free Windows
  Authenticode (self-signed) with the SAME thumbprint plumbing a paid cert uses.
- `scripts/sign-macos-adhoc.sh` — ad-hoc `.app` signing when no Developer ID.
- `scripts/verify.sh` / `scripts/verify.ps1` — downloader verification
  (sha256 + minisign + Authenticode status).

The free tiers and the paid tiers share the same secret-by-name plumbing:
acquiring a paid cert later is a secret swap (`WINDOWS_CERT_THUMBPRINT` → the OV
cert's thumbprint; add `APPLE_*`) with **no code change** — Tier 2 simply
supersedes Tier 1 for that platform. The minisign tier always runs regardless.

This honours the project's **no-paid-services** default: the installer is fully
functional and cryptographically verifiable for free; paid certs are an opt-in
upgrade that only removes the OS first-run warning, never a functional gate.

## Consequences

- Until the user acquires the Apple Developer Program membership and a Windows
  OV cert, public releases ship unsigned-dev with documented first-run friction.
  This is honest and reversible — acquiring the credentials and re-running the
  tag-gated release upgrades to signed with no code change.
- The README carries a "why the warning / how to proceed" section for end users
  hitting SmartScreen/Gatekeeper on an unsigned build.
- Secrets-by-name + HSM storage keeps the public repo safe by construction.

## References

- best-in-class-installer research §1/§4/§6 (cargo-packager, signing, ARP).
- installer-ip-safety research §A.2-A.4 (secrets-by-name, two-repo split,
  SHA-pinned Actions).
- Microsoft SmartScreen reputation change (2024); CA/Browser Forum code-signing
  key-storage requirement (June 2023); Apple `notarytool`/`stapler`/`spctl`
  documentation.

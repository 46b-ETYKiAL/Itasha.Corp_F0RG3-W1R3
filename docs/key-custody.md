# Key Custody

How the F0RG3-W1R3 installer framework handles signing-key material. The one
load-bearing rule: **the framework references keys by HANDLE, never by VALUE.**
No private key is ever committed, embedded, echoed, or logged. The wired
gitleaks gate + hardened `.gitignore` + `tests/content_safety_audit.py` enforce
this; the BYO cloud-signing path (`scripts/sign-cloud.ps1`) is built around it.

---

## The hardware-key mandate (why handles, not files)

Since **2023-06-01** the CA/B-Forum Baseline Requirements **mandate** that
publicly-trusted code-signing private keys live in a certified hardware module —
a FIPS 140-2 Level 2+ / Common Criteria EAL4+ HSM, hardware token, or cloud KMS.
A `.pfx`/`.p12` file with an exportable private key is **no longer issuable** for
an OV/EV code-signing certificate. The key cannot be exported; you sign by
asking the module to sign, addressing the key by a **handle / URI / alias**.

This framework is built for that world:

| Tier | Key location | How it is addressed | Cost |
|---|---|---|---|
| minisign (default, all platforms) | secret stored in `MINISIGN_SECRET_KEY` CI secret | by secret name | Free |
| cosign keyless (transparency) | ephemeral OIDC identity (no stored key) | the workflow's OIDC subject | Free |
| Windows self-signed (enterprise allow-list) | local cert store | `WINDOWS_CERT_THUMBPRINT` | Free |
| Windows OV/EV (public, warning-free) | cloud HSM / KMS / token | PKCS#11 URI or KMS alias HANDLE via `sign-cloud.ps1` | BYO (paid cert) |
| macOS Developer ID (notarization) | Apple-managed identity | `APPLE_SIGNING_IDENTITY` + notarytool creds | BYO ($99/yr) |

No paid service is REQUIRED to build or release: the free minisign + cosign-
keyless + checksum tiers cover integrity. The HSM/KMS tier is the OPT-IN path for
an org that holds an OV/EV certificate.

---

## Handle discipline (the rules)

1. **Never commit a private key.** Not a `.pfx`, `.p12`, `.pem`, `.key`, `.p8`,
   `.jks`, `.keystore`, or any private-key block. `.gitignore` excludes every
   such extension; `gitleaks` (CI + the runbook checklist) scans the working
   tree AND full history; the content-safety audit fails on any tracked
   key-shaped path.
2. **Reference by handle.** A signing identity is addressed by:
   - a **CI-secret name** (`MINISIGN_SECRET_KEY`, `APPLE_APP_PASSWORD`, …) —
     never the secret's value in source;
   - a **PKCS#11 URI** (`pkcs11:token=…;object=…`) for osslsigncode;
   - a **cloud-KMS alias / key reference** (Azure Trusted Signing account +
     certificate name, AWS/GCP KMS key id) for jsign;
   - a **certificate thumbprint** (`WINDOWS_CERT_THUMBPRINT`) for the local
     store path.
   The PUBLIC half (cert chain `.cer`/`.pem`, `keys/minisign.pub`) MAY be
   committed — it is public by design.
3. **Backend auth stays in the backend's own env.** PKCS#11 PINs, Azure
   service-principal creds, AWS credentials, etc. are read by the signing tool
   (osslsigncode / jsign) from ITS documented env at run time. The framework
   never reads, forwards, or echoes them.
4. **Rotation replaces the value behind the handle.** See the cert-rotation
   table in `docs/release-runbook.md`. Always sign Windows builds with the SAME
   publisher identity to preserve SmartScreen reputation (ADR-0003).
5. **Gated, never faked.** Absent a handle/credential, signing degrades to the
   free tiers with a loud `::warning::` — it never fabricates a signature,
   notarization ticket, or attestation.

---

## BYO cloud-signing (the opt-in path)

`scripts/sign-cloud.ps1` is **default-OFF**. It activates only when
`ITASHA_CLOUD_SIGNING=1`, and resolves the signing identity through the shared
`scripts/_sign-key-resolver.ps1` from these env vars (handles only):

| Env var | Meaning |
|---|---|
| `ITASHA_CLOUD_SIGNING=1` | enable the path (else no-op, exit 0) |
| `ITASHA_SIGN_BACKEND` | `osslsigncode` (PKCS#11) or `jsign` (cloud-KMS) |
| `ITASHA_SIGN_KEY_HANDLE` | the PKCS#11 URI / KMS alias — a HANDLE, never a value |
| `ITASHA_SIGN_CERT` | path/URL to the PUBLIC cert chain |
| `ITASHA_SIGN_TIMESTAMP_URL` | RFC-3161 TSA (free public default) |

Free OSS backends (referenced by handle, nothing vendored, nothing installed by
default): **osslsigncode** (PKCS#11 engine, any HSM/token) and **jsign**
(cloud-KMS native — Azure Trusted Signing, AWS KMS, GCP KMS, DigiCert ONE).

---

## Coordination note: the cross-platform signer (separate effort)

A sibling cross-platform code-signing service is a SEPARATE, future effort in
its OWN repository. Integrating it into F0RG3-W1R3 would be an OPTIONAL
future-integration — it is **NOT a dependency** of this framework. F0RG3-W1R3's
embedded signing (Authenticode thumbprint / cloud-HSM, macOS notarize+staple,
minisign, cosign-keyless) is and remains the shipped path. This note exists so a
future integrator knows the handle abstraction here is the seam to plug into;
nothing in this repo blocks on that effort.

---

## See also

- `docs/adr/0003-signing-posture.md` — the free→paid signing tier ladder.
- `docs/release-runbook.md` — cert-rotation procedure + degradation matrix.
- `keys/README.md` — the public-key directory contract.
- `scripts/sign-cloud.ps1` / `scripts/_sign-key-resolver.ps1` — the
  handle-resolver implementation.

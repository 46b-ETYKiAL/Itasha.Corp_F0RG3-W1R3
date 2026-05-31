# Signing keys (public material only)

This directory holds the **public** half of the project's free signing identity.
Private keys NEVER live here — the content-safety audit rejects them.

## `minisign.pub` (free, cross-platform artifact signing)

The minisign Ed25519 **public** key. Downloaders verify every release artifact
against it (`scripts/verify.sh` / `scripts/verify.ps1`), free and offline, with
no certificate authority and no cost.

It is created once by [`scripts/gen-minisign-key.sh`](../scripts/gen-minisign-key.sh):

```sh
./scripts/gen-minisign-key.sh
# → writes keys/minisign.pub (commit it) and a secret key (store, then delete)
```

- **Commit** `keys/minisign.pub`.
- **Store** the secret key's full contents in the GitHub Actions secret
  `MINISIGN_SECRET_KEY` (and `MINISIGN_PASSWORD` if you set one), then delete
  the local secret file.
- The release workflow signs each artifact into a detached `<artifact>.minisig`.

Until the key exists, releases ship **checksum-verified but unsigned** — the
honest state, never faked.

## What is intentionally NOT here

**No private key material is ever committed to this directory or anywhere in the
repo** — the hardened `.gitignore`, the wired gitleaks gate, and
`tests/content_safety_audit.py` (which fails on any tracked key-shaped path) all
enforce this. Keys are referenced BY HANDLE, never by value:

| Lives in a CI secret / HSM handle / never committed | Why |
|---|---|
| `MINISIGN_SECRET_KEY` | minisign signing private key |
| `WINDOWS_CERT_THUMBPRINT` (+ the cert in a hardware module) | Authenticode signing |
| `ITASHA_SIGN_KEY_HANDLE` (PKCS#11 URI / KMS alias) | BYO cloud/HSM OV/EV signing (opt-in) |
| `APPLE_*` (signing identity, app password / ASC API key) | macOS notarization (paid tier) |

For the full handle-not-value discipline, the CA/B-Forum 2023 hardware-key
mandate, the BYO cloud-signing option, and the cross-platform-signer coordination
note, see [`docs/key-custody.md`](../docs/key-custody.md). For the free→paid
signing tier ladder, see
[`docs/adr/0003-signing-posture.md`](../docs/adr/0003-signing-posture.md).

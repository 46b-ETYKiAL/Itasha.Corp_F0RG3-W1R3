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

| Lives in a CI secret / never committed | Why |
|---|---|
| `MINISIGN_SECRET_KEY` | signing private key |
| `WINDOWS_CERT_THUMBPRINT` + the cert's private key | Authenticode signing |
| `APPLE_*` (signing identity, app password) | macOS notarization (paid tier) |

See [`docs/adr/0003-signing-posture.md`](../docs/adr/0003-signing-posture.md) for
the full free→paid signing tier ladder.

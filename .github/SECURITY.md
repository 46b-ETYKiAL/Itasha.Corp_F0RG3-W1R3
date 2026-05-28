# Security Policy

The Itasha.Corp Installer Framework packages already-compiled application
binaries into branded, cross-platform installers. Because installers run with
elevated privileges and establish a publisher's signing identity, we take
reports about this repository seriously.

## Reporting a vulnerability

**Do not open a public issue for a security vulnerability.** Instead, use one of:

- **GitHub private vulnerability reporting** — open the repository's **Security**
  tab and click **Report a vulnerability** (preferred; keeps the report private).
- If private reporting is unavailable, contact the maintainers through the
  repository owner's published contact channel.

Please include:

- The affected file(s) or workflow and a description of the issue.
- A reproduction (a minimal manifest/config/workflow snippet, the command run,
  and the observed vs. expected behavior).
- The impact you believe it has (e.g. arbitrary code execution during install,
  signing-chain weakening, supply-chain tampering).

We aim to acknowledge a report within a few days and to agree on a coordinated
disclosure timeline before any public discussion.

## Reporting a leaked secret

If you find a **secret of any kind** committed to this repository — a signing
private key (`.p12`/`.pfx`/`.p8`/`.pem`/`.key`), an Apple notarization
credential, an API token, or a code-signing certificate — treat it as a
vulnerability and report it privately using the channel above. Do **not** post
the secret in a public issue or pull request. The credential should be revoked
and rotated as part of remediation.

## The IP / secret boundary (what this repo is allowed to contain)

This framework enforces a hard content boundary, documented in
[`../ships-publicly-vs-never.md`](../ships-publicly-vs-never.md). In short:

- **Secrets never live in the repo.** Signing private keys and notarization
  credentials live **only** in CI secrets, a cloud HSM, or a Key Vault, and are
  referenced **by name** in workflow files (e.g. `${{ secrets.NAME }}`). A
  literal key, password, token, or certificate in the tree is a security
  incident, not a configuration choice.
- **Binary-as-artifact, never source.** The framework consumes an application's
  **compiled binary** as a build input (a path or a downloaded release
  artifact). It never vendors, imports, or commits any application's **source
  tree**. There is no `src/` of any packaged app in this repository.
- **Public verification surface only.** We publish `checksum.sha256` and the
  **public** half of any signing/minisign identity so downloaders can verify
  artifacts. The **private** half never leaves the HSM/Key Vault.

These boundaries are enforced automatically: `tests/content_safety_audit.py`
runs as a **required CI gate** and fails any change that introduces a secret
file, an embedded private key, a vendored app-source tree, or an internal path.
GitHub secret-scanning and push-protection should be enabled on the repository
to catch leaks at push time.

## Supply-chain integrity

- Every third-party GitHub Action is pinned by full commit SHA.
- Release artifacts ship with a `checksum.sha256` and (where configured) build
  provenance attestations and a code-signature, so consumers can verify what
  they download.
- The default `GITHUB_TOKEN` permission is least-privilege (`contents: read`),
  elevated only on the jobs that must publish.

## Supported versions

The framework is developed on the default branch; security fixes land there and
in the most recent tagged release. Older tags are not separately maintained.

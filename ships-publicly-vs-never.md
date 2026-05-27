# Ships Publicly vs Never — IP-Safety Boundary Checklist

This is the **authoritative content-safety boundary** for the Itasha.Corp
Installer Framework. The framework is structured to live in its **own public
GitHub repository**. Anything in the public repo is world-readable forever
(including git history). This checklist is the contract that keeps the public
surface safe, and it is the source-of-truth allowlist that the automated
`tests/content_safety_audit` script enforces.

> **Core principle:** an installer packages an *already-compiled binary*. The
> packaging config, UI, scripts, and branding are safe to publish. The
> application's **source code** and all **signing secrets** are never published.

---

## The boundary at a glance

| SHIPS PUBLICLY (safe in the installer repo) | STAYS SECRET (never in the installer repo) |
|---|---|
| Packaging config (`packager.template.toml`, per-app `*.toml`) | Application **source code** (no `src/` trees vendored) |
| Installer UI text, NSIS `.nsh` hooks, `.desktop` files | Signing **private keys** (`.p12`, `.pfx`, `.p8`, `.pem`, `.key`) |
| Branding assets (SVG banner/footer, icons, dmg backgrounds) | Apple notarization credentials (`APPLE_APP_PASSWORD`, App Store Connect API key) |
| Build wrapper scripts (`sh` / PowerShell) | Windows code-signing cert + HSM/Key Vault access tokens |
| CI workflow definitions referencing secrets **by name only** | Actual secret **values** of any kind (API tokens, passwords) |
| Docs (README, CONTRIBUTING, ADRs, this checklist) | Internal absolute user paths (a Windows `Users` home dir, a POSIX `home` dir) |
| Package-manager manifests pointing at signed release artifacts | Internal tooling references (agent-system dirs, internal repo names) |
| `checksum.sha256` + minisign **public** key for verification | minisign / signing **private** keys |
| Compiled binaries consumed as **release artifacts** (downloaded, not committed) | Pre-release / internal binaries committed into the tree |

---

## The seven boundary rules

1. **Binary-as-artifact, never-vendor-source.** The framework consumes a
   compiled binary as a build *input* (a path or a downloaded release
   artifact). It never copies, imports, or commits the application's source
   tree. There is no `src/` of any packaged app in this repo.

2. **Two-repo split.** Application source lives in its own (potentially
   private) repository. This installer framework is a *separate* repository.
   The only thing that crosses the boundary is the **compiled binary** plus its
   public metadata (name, version, icon).

3. **Secrets live in CI / HSM / Key Vault only — referenced by name.** Workflow
   files may say `${{ secrets.APPLE_APP_PASSWORD }}` or read
   `$WINDOWS_CERT_THUMBPRINT` from the environment. They must **never** contain
   a literal key, password, token, or certificate. Signing private keys live in
   a cloud HSM or Key Vault and never touch the repo or a build runner's disk
   in plaintext.

4. **Public license independent of the app's license.** The framework is
   licensed `MIT OR Apache-2.0` so that packaging IP is decoupled from each
   packaged application's own license terms.

5. **No internal-system leakage.** Nothing referencing the internal agent
   system (its config/hook directories, internal repo names, internal plan
   identifiers, internal absolute user paths, or internal lore not approved for
   public release) may appear in any file destined for the public repo.

6. **Public verification surface only.** Publish `checksum.sha256` and the
   **public** half of any minisign/code-signing identity so downloaders can
   verify artifacts. The **private** half never leaves the HSM/Key Vault.

7. **Publication is a separate, authorized action.** Developing the framework
   inside the monorepo is *not* publication. Pushing to the public repo, or
   signing and distributing a release, is a distinct step that requires explicit
   owner authorization, and the content-safety audit must pass first.

---

## What the content-safety audit checks (`tests/content_safety_audit`)

The audit fails (non-zero exit) on **any** of the following found anywhere in
the framework tree:

- A vendored application source directory (e.g. an `apps/*/src/` tree).
- A signing private key or secret pattern: `.p12`, `.pfx`, `.p8`, `.pem`,
  `.key`, `.keystore`, `BEGIN ... PRIVATE KEY`, `AuthKey_*.p8`.
- An embedded secret value (long base64 tokens, `password = "..."` with a
  literal, AWS/Apple/GitHub token shapes).
- An internal absolute user path (a Windows per-user home directory, or a POSIX
  `home` / macOS `Users` per-user home directory).
- An internal plan-identifier token (`plan-<digits>`).
- An internal agent-system reference (the internal config-directory names or
  the internal system brand token).

The audit's allowlist is defined to match this checklist: only the categories
in the **SHIPS PUBLICLY** column are permitted. If you add a new file type to
the public surface, update both this checklist and the audit allowlist together.

> Note: the audit script and this checklist are *internal* development artifacts
> that guard the boundary. When the framework is mirrored to its public repo,
> the test harness ships too (it scans only for leakage; it contains no secrets).

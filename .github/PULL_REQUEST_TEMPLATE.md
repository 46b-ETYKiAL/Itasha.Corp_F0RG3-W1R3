<!--
Thanks for contributing to the Itasha.Corp Installer Framework.
Read CONTRIBUTING.md and ../ships-publicly-vs-never.md before opening this PR.
-->

## What this PR does

A short description of the change and why.

## Type of change

- [ ] New per-app override (`apps/*.toml`)
- [ ] Packaging config / template change
- [ ] New or changed packaging target (Windows / macOS / Linux)
- [ ] Signing / provenance / SBOM
- [ ] CI / workflow
- [ ] Docs only
- [ ] Bug fix

## Local checks (run before opening the PR)

Paste the output, or confirm each ran clean:

- [ ] `actionlint .github/workflows/*.yml`
- [ ] `shellcheck packaging/**/*.sh scripts/*.sh` (if shell scripts changed)
- [ ] `python tests/validate_config.py packager.template.toml apps/*.toml`
- [ ] `python tests/content_safety_audit.py` — **exits 0** (required gate)

## IP / secret boundary (required)

- [ ] No application **source** added (no `apps/*/src/` tree).
- [ ] No secret committed — no `.p12`/`.pfx`/`.p8`/`.pem`/`.key`, no token,
      no certificate, no embedded private key.
- [ ] Workflow secret references are **by name only** (`${{ secrets.NAME }}`).
- [ ] No internal absolute user path and no internal-system reference added.
- [ ] Any new third-party GitHub Action is pinned by full commit SHA.

## Reusability (D1)

- [ ] Per-app overrides stay small; company-wide defaults live in
      `packager.template.toml`.

## Notes for reviewers

Anything that needs attention (honest-skips where a tool/credential is absent,
follow-up considerations, etc.).

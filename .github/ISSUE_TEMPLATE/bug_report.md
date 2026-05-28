---
name: Bug report
about: Report a problem with the installer framework (packaging, build, or install)
title: "[bug] "
labels: ["bug"]
assignees: []
---

<!--
SECURITY: Do NOT report vulnerabilities or leaked secrets here. Use private
vulnerability reporting instead — see .github/SECURITY.md.

Do NOT paste any secret, signing key, certificate, or internal absolute path
into this issue.
-->

## What happened

A clear description of the bug.

## What you expected

What you expected to happen instead.

## Reproduction

Steps to reproduce, including the exact command(s) you ran:

```sh
# e.g. python tests/validate_config.py packager.template.toml apps/<app>.toml
```

## Scope

- [ ] Packaging config (`packager.template.toml` / a per-app `apps/*.toml`)
- [ ] Windows / NSIS installer
- [ ] macOS / DMG
- [ ] Linux (AppImage / `.deb` / Flatpak)
- [ ] CI workflow
- [ ] Docs

## Environment

- OS + version:
- Which app override (e.g. `apps/c0pl4nd.toml`):
- Framework version / commit:
- Relevant tool versions (cargo-packager, NSIS, etc.):

## Logs / output

Paste the relevant build or installer log output. **Redact any absolute user
path, secret, or token before pasting.**

```
<output here>
```

## Additional context

Anything else that helps reproduce or diagnose the problem.

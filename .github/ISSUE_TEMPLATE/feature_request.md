---
name: Feature request
about: Suggest a packaging capability, target, or workflow improvement
title: "[feature] "
labels: ["enhancement"]
assignees: []
---

<!--
Keep requests within the framework's purpose: it packages already-compiled
binaries into branded cross-platform installers. It does NOT vendor application
source and does NOT store secrets — see .github/SECURITY.md and
../ships-publicly-vs-never.md.
-->

## Problem / motivation

What problem would this solve? Which app(s) or target(s) does it affect?

## Proposed capability

What should the framework do? If it adds a packaging target, signing path, or
workflow step, describe the expected output artifact.

## Affected surface

- [ ] Shared template (`packager.template.toml`)
- [ ] Per-app override shape (`apps/*.toml`)
- [ ] Windows / NSIS
- [ ] macOS / DMG
- [ ] Linux (AppImage / `.deb` / Flatpak / zsync)
- [ ] Signing / provenance / SBOM
- [ ] CI / release workflow
- [ ] Docs

## Reusability check (D1)

Does this keep the per-app override small and the company defaults shared? A
feature that would make `apps/*.toml` larger or app-specific should explain why.

## Alternatives considered

Other approaches, and why this one is preferred.

## Additional context

Links to tool docs, prior art, or related issues.

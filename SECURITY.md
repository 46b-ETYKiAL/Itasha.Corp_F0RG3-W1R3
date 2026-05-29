# Security Policy

F0RG3-W1R3 is an in-house, cross-platform mobile build-platform installer
(Python + PowerShell + GitHub Actions). We take the security of the installer
and its supply chain seriously. This document explains how to report a
vulnerability, which versions receive fixes, and how we handle disclosure.

## Supported Versions

F0RG3-W1R3 follows a rolling-release model: the most recent release on the
default branch is the supported version. Security fixes are applied to the
latest release only; there is no long-term-support backport branch.

| Version            | Supported          |
| ------------------ | ------------------ |
| Latest release     | :white_check_mark: |
| Older releases     | :x:                |

If you are running an older build, the remediation for any reported issue is
to upgrade to the latest release.

## Reporting a Vulnerability

**Please do not open a public issue for security vulnerabilities.**

Report security issues through one of the following private channels:

1. **GitHub Private Vulnerability Reporting** (preferred) — use the
   **Security → Report a vulnerability** button on this repository. This opens
   a private advisory visible only to the maintainer and you.
2. **Email** — `46b.itasha.corp@pm.me` with the subject line
   `[SECURITY] F0RG3-W1R3: <short summary>`.

When reporting, please include:

- A description of the vulnerability and its impact.
- Steps to reproduce (proof-of-concept, affected file/workflow, or command).
- The version / commit SHA you tested against.
- Any suggested remediation, if you have one.

## Disclosure Process

We follow a **coordinated disclosure** model with a **90-day** disclosure
window:

| Stage | Target |
| ----- | ------ |
| Acknowledge receipt | within **3 business days** |
| Initial assessment & severity triage | within **7 days** |
| Fix developed and released | as soon as practical, and within **90 days** of the report |
| Public disclosure (advisory + CVE if warranted) | after a fix ships, or at 90 days, whichever comes first — coordinated with the reporter |

If a fix cannot be shipped within 90 days, we will coordinate an extended
timeline with the reporter rather than disclose without a remediation path.

We credit reporters in the published advisory unless you request otherwise.

## Scope

In scope:

- The installer code (Python, PowerShell) in this repository.
- The release and CI/CD GitHub Actions workflows in `.github/workflows/`.
- Supply-chain integrity of the published installer artifacts.

Out of scope:

- Vulnerabilities in upstream third-party tools the installer downloads or
  invokes (report those to their respective projects).
- Issues requiring a compromised local machine or privileged local account as
  a precondition.

## Supply-Chain Hardening

This project applies the following supply-chain controls, which are continuously
measured by [OpenSSF Scorecard](https://github.com/ossf/scorecard) (see the
`scorecard.yml` workflow and the dashboard for the live score):

- **Pinned GitHub Actions** — every `uses:` reference in
  `.github/workflows/*.yml` is pinned to a full-length commit SHA with a
  human-readable `# vX.Y.Z` version comment for Dependabot tracking.
- **Hardened runners** — `step-security/harden-runner` enforces egress policy
  on CI jobs.
- **Dependency review** and **CodeQL** static analysis run on pull requests.
- **Signed, attested releases** — release artifacts are accompanied by an
  SBOM (`anchore/sbom-action`), Cosign signatures, and SLSA build-provenance
  attestations.

## Fuzzing

F0RG3-W1R3 is an installer/orchestration tool. It does **not** implement a
parser, network service, or any component that consumes untrusted, adversarial
input across a trust boundary, so continuous fuzzing (OSS-Fuzz/ClusterFuzzLite)
is **not applicable**. See `.github/scorecard-notes.md` for the full rationale
behind this and other heuristic-only Scorecard findings.

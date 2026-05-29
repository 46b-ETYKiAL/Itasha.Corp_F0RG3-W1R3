# OpenSSF Scorecard — Finding Notes

This file documents the disposition of every [OpenSSF Scorecard](https://github.com/ossf/scorecard)
check for F0RG3-W1R3, with particular attention to checks that are
**heuristic-only** or require **manual / external action** and therefore
cannot be resolved purely by a code change in this repository.

Last reviewed: 2026-05-29.

## Code-fixed checks

| Check | Status | What was done |
| ----- | ------ | ------------- |
| `Security-Policy` (`SecurityPolicyID`) | Resolved | Added `SECURITY.md` with a private reporting channel, supported-versions table, and a 90-day coordinated-disclosure window. |
| `Pinned-Dependencies` (`PinnedDependenciesID`) | Resolved | Every `uses:` in `.github/workflows/*.yml` is pinned to a full-length commit SHA with a `# vX.Y.Z` Dependabot-readable version comment. |
| `Code-Review` (`CodeReviewID`) | Resolved via branch protection | The default branch requires a pull request with 1 approving review. As a solo-maintained repository, the owner is permitted to bypass the review requirement when necessary; all changes still flow through a PR for an auditable history. |

## Heuristic-only / external-action checks

These checks cannot be closed by editing code in this repo. They resolve either
automatically over time, or only after a human performs a one-time external
action.

### `Fuzzing` (`FuzzingID`) — Not Applicable

**Disposition: not applicable; no action taken (by design).**

Scorecard's `Fuzzing` check looks for integration with a fuzzing framework
(OSS-Fuzz, ClusterFuzzLite, Go native fuzzing, etc.). F0RG3-W1R3 is an
**installer / build-orchestration tool** written in Python and PowerShell. It:

- does not implement a parser for an untrusted wire format or file format,
- does not expose a network service or RPC surface,
- does not process adversarial, attacker-controlled input across a trust
  boundary.

Fuzzing is a technique for finding memory-safety and input-handling bugs in
code that consumes untrusted input. Scaffolding a fuzzing harness for a tool
with no such surface would add maintenance burden and CI cost with no security
benefit, and would itself be an anti-pattern. We therefore intentionally do
**not** integrate fuzzing. If a future release adds a parser or an
untrusted-input surface, this decision will be revisited.

### `CII-Best-Practices` (`CIIBestPracticesID`) — Deferred (external signup)

**Disposition: deferred; requires a manual, human, external action.**

This check awards points only when the project holds an
[OpenSSF Best Practices Badge](https://www.bestpractices.dev/) (formerly the
CII Best Practices Badge). Obtaining the badge requires the maintainer to:

1. Sign in at **https://www.bestpractices.dev/** with the GitHub account.
2. Register this project.
3. Self-certify against the passing-level criteria questionnaire.

Because this is an account-bound, human self-attestation step that cannot be
performed by an automated code change, it is deferred to the maintainer. Once
the badge is obtained, the badge markdown can be added to `README.md` and this
check will pass on the next scheduled scan.

### `Maintained` (`MaintainedID`) — Resolves automatically

**Disposition: no action; resolves over time.**

`Maintained` is a pure activity heuristic: it rewards repositories with commits
and/or merged pull requests within the last 90 days. There is nothing to "fix"
in code — the check improves automatically as routine commits and releases land
on the default branch. The remediation work in this repository (and ongoing
development) keeps this check satisfied.

## When do these alerts close?

Scorecard alerts are produced by the scheduled `scorecard.yml` workflow and
uploaded to GitHub code scanning as SARIF. They do **not** close instantly when
a fix merges. The `SecurityPolicyID`, `PinnedDependenciesID`, and
`CodeReviewID` alerts will re-evaluate and close on the **next scheduled
Scorecard run** (or on a manual `workflow_dispatch` of `scorecard.yml`). The
`FuzzingID`, `CIIBestPracticesID`, and `MaintainedID` findings are documented
here as their permanent disposition.

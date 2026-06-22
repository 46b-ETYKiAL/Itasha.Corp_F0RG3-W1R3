# Privacy

F0RG3-W1R3 is a local-first installer framework. This document states, in plain
language, what data the produced installer collects and transmits. The short
answer: **nothing, by default.**

---

## The short version

- **Telemetry-free by default.** The installer does not phone home. There is no
  telemetry, no analytics, no ads, and no account or login. Nothing about how
  you use it is collected or transmitted in the background — ever.
- **Crash/issue reporting is opt-in and default-OFF.** F0RG3-W1R3 integrates the
  in-house **W1TN3SS** reporting SDK
  (`installer/src/reporting.rs`, `panic_hook.rs`, `issue_intake.rs`), but
  **nothing is ever sent without you first opting in and consenting to the
  specific report.** With the defaults, the installer transmits no reports of
  any kind.

---

## Opt-in crash & issue reporting (W1TN3SS)

If a build fails or you want to report a problem, F0RG3-W1R3 can — *only if you
choose to* — help you send a report. The design follows one rule: **nothing is
transmitted without your explicit, per-report consent.**

- **Default-OFF.** Every reporting stream defaults to off/inert. The installer
  is a short-lived, one-shot process with no persistent config; consent is a
  per-event runtime decision, and nothing is captured-for-send unless you tick
  the opt-in box on the failure screen.
- **Consent-gated, per report.** A crash report is only ever sent after you
  agree on the failure screen (the consent token that authorizes a send only
  exists once you do). No consent token means no send.
- **Captured locally first, and previewable.** A crash report is built from a
  panic's short message plus its `file:line` site, sanitized, and spooled
  locally; capture transmits nothing. Before anything leaves your machine you
  can review the report text and cancel.
- **Two independent consent streams**, each separately opt-in:
  1. a **crash-report** stream — a scrubbed, stack-only text report (a panic's
     short message plus its `file:line` site; your home directory is normalized
     to `<HOME>` by the SDK's sanitizer). Because a backtrace can be tied back to
     a single report, this stream is honestly **pseudonymous, not anonymous**,
     under GDPR — we label it that way on purpose; and
  2. a **manual "Report an issue"** stream — covered below.
- **No persistent identifier.** Reports carry no account, install ID, or machine
  fingerprint — only an ephemeral per-report nonce.
- **Self-hosted ingest only — no third parties.** A crash report is delivered
  by a hardened HTTPS transport (no redirects, bounded timeout) to a **single,
  config-injected self-hosted** W1TN3SS endpoint, and only when one is
  configured. There is no default endpoint, and no data is ever sent to any
  third-party crash or analytics vendor.
- **Manual "Report an issue" stays manual.** The issue-intake helper only opens
  a *prefilled, editable* GitHub Issue Form when **you** press Open / Copy /
  Email on the failure screen — there is no background or default-on path.

In short: F0RG3-W1R3 is **telemetry-free by default**, and offers **opt-in,
default-OFF** crash/issue reporting that never sends without your consent.

---

## Signing telemetry

The signing posture (Windows OV / macOS notarization) is documented in
[SECURITY.md](SECURITY.md). Signing is a build-time step performed by the
maintainer; it is not a runtime data path and collects nothing from end users.

---

## Questions or concerns

Privacy or security questions can be raised through the repository's
**Security → Advisories** form for anything sensitive, or as a regular GitHub
issue for general questions. See [SECURITY.md](SECURITY.md) for the full
security posture.

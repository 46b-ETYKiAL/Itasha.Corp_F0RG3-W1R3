# Coverage — W1TN3SS reporting integration (`installer/src/reporting.rs`)

The opt-in W1TN3SS crash/error reporting integration (`installer/src/reporting.rs`)
is the installer's host glue over the first-party `itasha-report-core` SDK +
`itasha-report-transport-tor` crate. It owns the privacy-critical seams: the
default-OFF consent gate, the capture-and-spool panic hook, the transport
selection (clearnet vs. opt-in Tor onion), and the consent-gated send.

Because these seams are load-bearing for the **default-OFF / consent-gated**
privacy invariant, the module is covered to **100% of the testable surface** and
guarded by a `cargo llvm-cov --fail-under-lines` gate (see below).

## Coverage gate

The CI `native-installer` job and the local `scripts/coverage.sh` run:

```bash
cargo llvm-cov --manifest-path installer/Cargo.toml --no-default-features \
  --fail-under-lines 94
```

The floor is **94**, set just below the achieved **95.48%** line coverage to
absorb the small, deterministic cross-platform variance described below (the
`data_dir()` resolver has a `#[cfg(windows)]` / `#[cfg(not(windows))]` split, so
exactly one of two lines is reachable on any single platform). The achieved
testable coverage is 100% — every line not covered is enumerated and justified
below as genuinely uncoverable in a unit test.

## What the tests cover (the privacy-critical surface)

| Invariant | Test(s) |
|---|---|
| **Consent default-OFF — nothing transmits without an endpoint AND a consent token** | `send_with_neither_env_set_refuses_default_off`, `send_without_endpoint_refuses_and_transmits_nothing`, `dispatch_send_none_choice_refuses_and_transmits_nothing` |
| Capture-and-spool panic hook spools locally, transmits nothing | `capture_panic_spools_into_the_global_data_dir`, `capture_and_drain_round_trips_in_temp_dir` |
| Telemetry suppression (`S4F3_DISABLE_TELEMETRY=1`) silences the log but never the capture | `capture_panic_is_silent_under_disable_telemetry_but_still_spools` |
| Spool-open failure surfaces a structured outcome (never a silent swallow) | `capture_panic_surfaces_spool_open_failure` |
| No-data-dir surfaces a structured failure (unix `data_dir()` shape) | `capture_panic_without_a_data_dir_fails_no_data_dir` |
| Transport selection: valid onion > clearnet; malformed onion refuses (no silent clearnet downgrade); port override; default-clearnet; nothing-configured | the `select_transport_*` suite |
| Tor-without-data-dir fails (never a silent clearnet downgrade, never a fake Sent) | `dispatch_send_tor_without_data_dir_fails_not_silently_clearnet` |
| Crash-only stream filter — a spooled manual-issue is never surfaced on the crash dialog | `load_from_spool_keeps_only_crash_stream_reports` |
| Corrupt spool entries are skipped, never crashed-on or surfaced | `load_from_spool_skips_a_corrupt_spool_file_without_crashing`, `advance_skips_a_corrupt_entry_queued_after_a_valid_one` |
| Multi-report queue advance + per-decline single-file removal | `load_from_spool_presents_multiple_crashes_in_sequence` |
| Send with no endpoint keeps the file spooled + advances (no fake Sent, no silent drop) | `consent_and_send_without_endpoint_advances_to_next_and_keeps_both_spooled`, `crash_consent_send_without_endpoint_keeps_file_spooled` |
| Unbound-spool paths clear the dialog without panicking | `decline_with_unbound_spool_clears_current_without_panicking`, `advance_with_unbound_spool_and_nonempty_queue_presents_nothing`, `load_from_spool_with_unbound_spool_presents_nothing` |
| Editable preview round-trips user redactions into the sent body | `edited_preview_text_round_trips_user_redactions_into_body` |
| Outcome log details are stable + non-identifying (no PII leak) | `outcome_log_details_are_stable_and_non_identifying` |

## Genuinely-uncoverable lines (NOT faked, NOT excluded by suppression)

These lines cannot be exercised by a hermetic unit test. They are covered by the
SDK's own crate tests and the `#[ignore]`'d live E2E placeholder, or are
single-platform `cfg` branches. They are **not** silenced with coverage-ignore
pragmas — they remain visible in the report, and the gate floor accounts for
them honestly.

| Line(s) | Code | Why uncoverable in a unit test |
|---|---|---|
| 321–341 | `dispatch_send` — `TorOnion` live Arti connect + `Clearnet` live `LeanPipelineBackend.send` | Requires a running Tor network + a real onion service / a live ingest endpoint. The **selection + dispatch wiring** is fully covered (the `None`, malformed-onion, and Tor-without-data-dir refusal arms); only the actual network connect is the SDK transport crate's responsibility. Covered there + by the `#[ignore]`'d `onion_send_e2e_placeholder`. |
| 481–483 | `consent_and_send` — remove the spooled file **after a successful `Sent`** | The `Sent` outcome only occurs after a real network send (see above). The non-Sent retain path is covered; the post-Sent removal needs a live endpoint. |
| 196 | `capture_panic` — `spool.enqueue` error arm | A spool-OPEN failure is fault-injected and covered (line 199, `capture_panic_surfaces_spool_open_failure`); an enqueue failure *after* a successful open requires making the reports dir read-only mid-operation, which is not portably reproducible across Windows/Linux test runners. |
| 202 | `capture_panic` — `data_dir() == None` arm | Reachable only when no platform per-user data dir resolves. On Windows `LOCALAPPDATA` always resolves in CI, so this is exercised via the `#[cfg(not(windows))]` `capture_panic_without_a_data_dir_fails_no_data_dir` test on Linux CI. Exactly one of {201-region, 202} is reachable per platform. |
| 436 | `load_from_spool` — `spool.list()` error arm | `list()` only fails if `read_dir` fails on a dir `Spool::open` just `create_dir_all`'d — not portably reproducible. |
| 498 | `decline_and_discard` — `spool.remove` region exit | Best-effort remove; the spool-None and remove-success paths are both covered; this residual region is a non-load-bearing brace exit. |
| 855, 857 | `onion_send_e2e_placeholder` | `#[ignore]`'d by design — a CI run must never attempt a live network send. |
| 1164 | `capture_panic_surfaces_spool_open_failure` panic arm | Only executes if the fault-injection assertion fails (i.e. a regression). |

None of the privacy-critical logic (consent gate, default-OFF, opt-in flow,
transport selection, error/refusal paths) is in the uncoverable set — every one
of those is exercised by a hermetic test above.

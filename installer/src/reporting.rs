//! W1TN3SS opt-in crash/error reporting — the F0RG3-W1R3 installer host glue
//! (Tier-1).
//!
//! This module is thin host glue over the in-house `itasha-report-core` SDK
//! (pinned git tag). The installer implements NO SDK behaviour — the report
//! model, sanitizer, spool, transport, preview API and consent gate all live in
//! the SDK and are CALLED here. The two seams this module owns are:
//!
//! 1. **Capture** ([`capture_panic`]) — builds a Tier-1 report from a panic's
//!    `&'static str` message + our own `file:line` SITE, sanitizes it, and
//!    SPOOLS it locally. It transmits NOTHING — local-first, offline-safe,
//!    consent comes later.
//! 2. **Consent-gated send** ([`send_report`]) — given a host-minted
//!    [`ConsentToken`] (which only exists after the user agreed), transmit one
//!    spooled report through the SDK's hardened transport, then log the outcome.
//!
//! The installer is a short-lived, one-shot process with no persistent config
//! file, so there is NO config-schema migrate here (the per-app config migrate
//! in the fleet-rollout plan applies to apps that own a persistent config; the
//! installer owns none). Consent is a per-event runtime decision and DEFAULTS
//! OFF: nothing is captured-for-send unless the user explicitly ticks the
//! opt-in box on the failure screen, and nothing transmits without a
//! [`ConsentToken`].
//!
//! Privacy invariants (inherited from the SDK, asserted at this surface):
//! - default-OFF (the failure-screen opt-in box defaults unchecked),
//! - consent-gated (no [`ConsentToken`] => no send — enforced at the type level
//!   by the SDK's `IngestBackend::send` signature),
//! - previewable+editable before send (the failure screen shows [`preview_text`]),
//! - no persistent identifier (only the consent token's ephemeral nonce),
//! - the panic `&'static str` discipline (a `String` payload — which could embed
//!   environment fragments or a path — is deliberately suppressed at capture).

use std::path::{Path, PathBuf};

use itasha_report_core::backend::{
    IngestBackend, LeanPipelineBackend, SendOutcome, TransportConfig,
};
use itasha_report_core::consent::ConsentToken;
use itasha_report_core::preview::Preview;
use itasha_report_core::report::Report;
use itasha_report_core::sanitize::Sanitizer;
use itasha_report_core::spool::Spool;

/// The env var that injects the self-hosted ingest endpoint. There is NO
/// hardcoded URL in the installer and NO default — a build with this unset can
/// spool locally but can NEVER transmit (a mis-build cannot phone home). Until
/// the server endpoint is configured, a consented send returns the structured
/// `RefusedNoEndpoint` outcome (never a silent drop, never a fake success).
pub const REPORT_ENDPOINT_ENV: &str = "F0RG3_REPORT_ENDPOINT";

/// The structured result of attempting a report, logged counts/enums only
/// (never PII). A report is either captured-and-spooled, sent, refused for want
/// of an endpoint, or failed in transport — never silently dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportOutcome {
    /// The panic was captured and written to the local spool. Nothing sent.
    Spooled,
    /// A consented report was transmitted and accepted by the endpoint.
    Sent,
    /// Consent was present but no endpoint is configured — the report stays
    /// spooled for a later, configured send.
    RefusedNoEndpoint,
    /// The transport failed (offline, TLS, status). The report is retained.
    Failed(String),
}

impl ReportOutcome {
    /// The stable, non-identifying log-detail string for this outcome.
    pub fn log_detail(&self) -> &'static str {
        match self {
            ReportOutcome::Spooled => "spooled",
            ReportOutcome::Sent => "sent",
            ReportOutcome::RefusedNoEndpoint => "refused-no-endpoint",
            ReportOutcome::Failed(_) => "failed",
        }
    }
}

/// Log a report outcome counts/enums only (no PII — the `Failed` reason is
/// NEVER inlined). Honours `S4F3_DISABLE_TELEMETRY=1` by emitting nothing.
/// Best-effort; never blocks. The installer has no `tracing` dependency, so the
/// outcome is written to stderr only (de-elevated, no console in the GUI build
/// — this is a no-op there, which is fine: the structured enum is the contract,
/// the sink is best-effort).
fn log_outcome(outcome: &ReportOutcome) {
    if std::env::var_os("S4F3_DISABLE_TELEMETRY").is_some() {
        return;
    }
    eprintln!("f0rg3::report detail={}", outcome.log_detail());
}

/// The per-user data directory the W1TN3SS report spool (`reports/`) is created
/// under. The installer has no config file, so this is resolved directly from
/// the platform per-user data dir: `%LOCALAPPDATA%\itasha-installer` on Windows,
/// `$XDG_DATA_HOME`/`$HOME/.local/share/itasha-installer` elsewhere. Returns
/// `None` when no such dir resolves (no `%LOCALAPPDATA%` / `$HOME`).
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("itasha-installer"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
            })
            .map(|p| p.join("itasha-installer"))
    }
}

/// Build a sanitized Tier-1 crash report from the panic's STATIC message and our
/// own panic SITE. Only a source-literal message (e.g. an `expect("…")` string)
/// and the `file:line` of our own code enter the report. A runtime `String`
/// payload (which could embed environment fragments or a user's path) is the
/// caller's responsibility to keep out (the hook passes `&'static str` only);
/// the SDK's [`Sanitizer`] is the second line of defense (home/username/host
/// scrub).
pub fn build_crash_report(static_msg: &'static str, location: &str) -> Report {
    let raw = Report::crash(format!("panic: {static_msg} (at {location})"))
        .with_metadata("app_version", env!("CARGO_PKG_VERSION"))
        .with_metadata("app", "itasha-installer")
        .with_metadata("os", std::env::consts::OS);
    Sanitizer::new().sanitize(raw)
}

/// The literal, editable Tier-1 preview text the failure screen shows the user
/// BEFORE any send. This is the transparency primitive — the user sees exactly
/// what would leave the machine.
#[must_use]
pub fn preview_text(report: &Report) -> String {
    Preview::of(report).text().to_string()
}

/// Rebuild a [`Report`] from the user-edited preview text, preserving the
/// original report's stream, title, metadata, and attachments. The preview text
/// renders as `title\n\nbody[\n\n--- metadata ---\n…]`; this extracts the BODY
/// span so the user's edits/redactions to the body are what gets sent.
#[must_use]
pub fn edited_report_from_preview_text(edited_text: &str, original: &Report) -> Report {
    let body = edited_text
        // Drop the title line: everything after the first blank-line separator.
        .split_once("\n\n")
        .map(|(_title, rest)| rest)
        .unwrap_or(edited_text)
        // Drop the metadata footer if present.
        .split("\n\n--- metadata ---\n")
        .next()
        .unwrap_or(edited_text)
        .to_string();
    Report {
        stream: original.stream,
        title: original.title.clone(),
        body,
        metadata: original.metadata.clone(),
        attachments: original.attachments.clone(),
    }
}

/// Capture a panic into the local spool. Builds the sanitized Tier-1 report,
/// then enqueues it to `<data_dir>/reports/` via the SDK's atomic spool. This
/// is the panic-hook seam: it CAPTURES + SPOOLS but transmits NOTHING — consent
/// is sought later, never inside the panic hook. Returns the outcome (for
/// logging).
///
/// Best-effort and panic-safe: a spool failure inside an already-panicking
/// thread must not re-panic. The outcome is logged either way.
pub fn capture_panic(static_msg: &'static str, location: &str) -> ReportOutcome {
    let outcome = match data_dir() {
        Some(dir) => match Spool::open(&dir) {
            Ok(spool) => {
                let report = build_crash_report(static_msg, location);
                match spool.enqueue(&report) {
                    Ok(_path) => ReportOutcome::Spooled,
                    Err(e) => ReportOutcome::Failed(format!("spool: {e}")),
                }
            }
            Err(e) => ReportOutcome::Failed(format!("spool-open: {e}")),
        },
        // No data dir => nowhere to spool. Surface it rather than swallow.
        None => ReportOutcome::Failed("no-data-dir".to_string()),
    };
    log_outcome(&outcome);
    outcome
}

/// Transmit ONE report through the SDK's hardened transport, consent-gated.
///
/// The `consent` argument is mandatory — there is no send path without it (the
/// SDK enforces this at the type level). The host mints the [`ConsentToken`]
/// ONLY after the user explicitly opted in on the failure screen. The transport
/// is the SDK's [`LeanPipelineBackend`]: a static User-Agent, zero redirects,
/// bounded timeout, size-capped, NO persistent identifier (only the token's
/// ephemeral nonce). The outcome is logged.
///
/// If no endpoint is configured (the `F0RG3_REPORT_ENDPOINT` env is unset), this
/// returns [`ReportOutcome::RefusedNoEndpoint`] and transmits nothing — the
/// report stays in the spool for a later, configured send.
pub fn send_report(report: &Report, consent: &ConsentToken) -> ReportOutcome {
    let outcome = match endpoint_from_env() {
        Some(endpoint) => {
            let backend = LeanPipelineBackend::new(TransportConfig::new(endpoint));
            match backend.send(report, consent) {
                Ok(SendOutcome::Sent) => ReportOutcome::Sent,
                Ok(SendOutcome::Failed(reason)) => ReportOutcome::Failed(reason),
                Err(e) => ReportOutcome::Failed(e.to_string()),
            }
        }
        None => ReportOutcome::RefusedNoEndpoint,
    };
    log_outcome(&outcome);
    outcome
}

/// Read the ingest endpoint from the env var, treating an empty value as unset.
fn endpoint_from_env() -> Option<String> {
    std::env::var(REPORT_ENDPOINT_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Open the local spool rooted at an EXPLICIT data dir so callers (and tests)
/// can drain pending crash reports. The dir is always passed by the caller (a
/// temp dir under test) so no spool I/O ever silently hits the GLOBAL
/// [`data_dir`].
pub fn open_spool_in(dir: &Path) -> Option<Spool> {
    Spool::open(dir).ok()
}

/// The per-launch crash-consent dialog state, owned by the app. On launch the
/// host loads any crash reports spooled by a PRIOR crashed run; the dialog
/// presents them with an EDITABLE preview and equal-weight Send / Don't-send.
/// Nothing transmits without a consent token minted on the Send press.
///
/// This holds NO SDK transport state — only the spooled paths, the currently-
/// presented report + its editable preview text.
#[derive(Debug, Default)]
pub struct CrashConsentState {
    /// The EXPLICIT data dir this dialog's spool I/O is rooted at. `None` until
    /// the host binds it via [`CrashConsentState::set_data_dir`]; while `None`
    /// every spool operation is a no-op (so a default-constructed state touches
    /// NO real data dir).
    data_dir: Option<PathBuf>,
    /// Remaining spooled report paths to present (oldest first).
    queue: Vec<PathBuf>,
    /// The report currently shown in the dialog (loaded from `queue`'s head).
    current: Option<(PathBuf, Report)>,
    /// The editable preview text the user sees and may modify before sending.
    edited_text: String,
}

impl CrashConsentState {
    /// Bind the explicit data dir whose `reports/` spool this dialog drains.
    pub fn set_data_dir(&mut self, dir: Option<PathBuf>) {
        self.data_dir = dir;
    }

    /// Open this dialog's spool at its bound data dir, if any is set.
    fn spool(&self) -> Option<Spool> {
        self.data_dir.as_deref().and_then(open_spool_in)
    }

    /// Load the spooled CRASH reports into the dialog queue. Returns the number
    /// queued. Best-effort: a spool error yields an empty queue.
    pub fn load_from_spool(&mut self) -> usize {
        use itasha_report_core::report::Stream;
        self.queue.clear();
        self.current = None;
        if let Some(spool) = self.spool() {
            if let Ok(paths) = spool.list() {
                for path in paths {
                    if let Ok(report) = spool.load(&path) {
                        if report.stream == Stream::CrashReports {
                            self.queue.push(path);
                        }
                    }
                }
            }
        }
        self.advance();
        self.queue.len() + usize::from(self.current.is_some())
    }

    /// Whether the dialog has a report to present this frame.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        self.current.is_some()
    }

    /// The editable preview text (mutable so the dialog can bind a `TextEdit`).
    pub fn edited_text_mut(&mut self) -> &mut String {
        &mut self.edited_text
    }

    /// Pop the next report off the queue and load it as `current` + its preview
    /// text. Clears `current` when the queue is empty.
    fn advance(&mut self) {
        self.current = None;
        self.edited_text.clear();
        if self.queue.is_empty() {
            return;
        }
        let path = self.queue.remove(0);
        if let Some(spool) = self.spool() {
            if let Ok(report) = spool.load(&path) {
                self.edited_text = preview_text(&report);
                self.current = Some((path, report));
            }
        }
    }

    /// The user pressed SEND on the current crash report. Rebuild the (possibly
    /// edited/redacted) report body from the editable preview text, mint a
    /// consent token, transmit, and — on a successful send — remove the spooled
    /// file. Returns the outcome. Advances to the next queued report regardless
    /// of outcome.
    pub fn consent_and_send(&mut self) -> Option<ReportOutcome> {
        let (path, original) = self.current.take()?;
        let report = edited_report_from_preview_text(&self.edited_text, &original);
        let token = ConsentToken::granted();
        let outcome = send_report(&report, &token);
        if outcome == ReportOutcome::Sent {
            if let Some(spool) = self.spool() {
                let _ = spool.remove(&path);
            }
        }
        // Not sent (offline / no endpoint / failed): keep the file spooled so a
        // later configured/online send can retry.
        self.advance();
        Some(outcome)
    }

    /// The user pressed DON'T-SEND on the current crash report. Discard the
    /// spooled file (the user declined to send it) and advance.
    pub fn decline_and_discard(&mut self) {
        if let Some((path, _)) = self.current.take() {
            if let Some(spool) = self.spool() {
                let _ = spool.remove(&path);
            }
        }
        self.advance();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itasha_report_core::report::Stream;

    /// A scoped guard that sets an env var and restores it on drop.
    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }
    impl EnvGuard {
        fn unset(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, prev }
        }
        fn set(key: &'static str, val: &str) -> Self {
            let prev = std::env::var(key).ok();
            std::env::set_var(key, val);
            Self { key, prev }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    use std::sync::Mutex;
    static ENDPOINT_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn crash_report_is_crash_stream_and_carries_static_message() {
        let r = build_crash_report("called `Option::unwrap()` on a `None`", "src/foo.rs:42");
        assert_eq!(r.stream, Stream::CrashReports);
        assert!(r.body.contains("called `Option::unwrap()`"));
        assert!(r.body.contains("src/foo.rs:42"));
        assert!(r.metadata.iter().any(|(k, _)| k == "app_version"));
        assert!(r
            .metadata
            .iter()
            .any(|(k, v)| k == "app" && v == "itasha-installer"));
        assert!(r.metadata.iter().any(|(k, _)| k == "os"));
    }

    #[test]
    fn preview_text_shows_the_literal_payload() {
        let r = build_crash_report("boom", "src/x.rs:1");
        let text = preview_text(&r);
        assert!(text.contains("boom"));
        assert!(text.contains("src/x.rs:1"));
    }

    #[test]
    fn send_without_endpoint_refuses_and_transmits_nothing() {
        let _lock = ENDPOINT_LOCK.lock().unwrap();
        let _g = EnvGuard::unset(REPORT_ENDPOINT_ENV);
        // Even WITH a consent token, an unset endpoint cannot transmit — the
        // report stays spooled and the outcome is the structured refusal (never
        // a fake Sent, never a silent drop).
        let r = build_crash_report("boom", "src/x.rs:1");
        let token = ConsentToken::granted();
        let outcome = send_report(&r, &token);
        assert_eq!(outcome, ReportOutcome::RefusedNoEndpoint);
    }

    #[test]
    fn empty_endpoint_env_is_treated_as_unset() {
        let _lock = ENDPOINT_LOCK.lock().unwrap();
        let _g = EnvGuard::set(REPORT_ENDPOINT_ENV, "   ");
        assert!(
            endpoint_from_env().is_none(),
            "a whitespace-only endpoint must be treated as unset (cannot phone home)"
        );
    }

    #[test]
    fn outcome_log_details_are_stable_and_non_identifying() {
        assert_eq!(ReportOutcome::Spooled.log_detail(), "spooled");
        assert_eq!(ReportOutcome::Sent.log_detail(), "sent");
        assert_eq!(
            ReportOutcome::RefusedNoEndpoint.log_detail(),
            "refused-no-endpoint"
        );
        // The Failed reason is NOT inlined into the log detail (no PII leak).
        assert_eq!(
            ReportOutcome::Failed("transport error: https://secret".to_string()).log_detail(),
            "failed"
        );
    }

    #[test]
    fn edited_preview_text_round_trips_user_redactions_into_body() {
        let original = Report::crash("panic: boom (at src/x.rs:1)")
            .with_metadata("os", "linux")
            .with_metadata("app_version", "9.9.9");
        let preview = preview_text(&original);
        assert!(preview.contains("boom"));
        let edited_text = preview.replace("boom", "[redacted]");
        let edited = edited_report_from_preview_text(&edited_text, &original);
        assert!(edited.body.contains("[redacted]"));
        assert!(!edited.body.contains("boom"));
        assert!(!edited.body.contains("--- metadata ---"));
        assert_eq!(edited.stream, Stream::CrashReports);
        assert_eq!(edited.title, original.title);
        assert_eq!(edited.metadata, original.metadata);
    }

    #[test]
    fn default_crash_consent_state_touches_no_real_data_dir() {
        // A default-constructed state with no bound data dir is fully inert:
        // load_from_spool returns 0 and nothing is presented.
        let mut st = CrashConsentState::default();
        assert_eq!(st.load_from_spool(), 0);
        assert!(!st.has_pending());
    }

    #[test]
    fn crash_consent_load_and_decline_round_trip_in_temp_dir() {
        // Capture into a temp data dir, then a bound consent dialog drains it and
        // DECLINE removes the spooled file (the user declined to send).
        let dir = std::env::temp_dir().join(format!("f0rg3-consent-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let report = build_crash_report("boom", "src/x.rs:1");
        let spool = open_spool_in(&dir).expect("open spool");
        spool.enqueue(&report).expect("enqueue");

        let mut st = CrashConsentState::default();
        st.set_data_dir(Some(dir.clone()));
        assert!(st.load_from_spool() >= 1, "the queued crash must load");
        assert!(st.has_pending());
        assert!(st.edited_text_mut().contains("boom"), "preview is editable");
        st.decline_and_discard();
        assert!(!st.has_pending(), "declining clears the presented report");
        // The declined file is removed from the spool.
        let remaining = open_spool_in(&dir)
            .and_then(|s| s.list().ok())
            .unwrap_or_default();
        assert!(
            remaining.is_empty(),
            "declined report is discarded from the spool"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crash_consent_send_without_endpoint_keeps_file_spooled() {
        // With no endpoint configured, Send returns RefusedNoEndpoint and the
        // file STAYS spooled (never a fake Sent, never a silent drop).
        let _lock = ENDPOINT_LOCK.lock().unwrap();
        let _g = EnvGuard::unset(REPORT_ENDPOINT_ENV);
        let dir = std::env::temp_dir().join(format!("f0rg3-send-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let report = build_crash_report("boom", "src/x.rs:1");
        open_spool_in(&dir)
            .unwrap()
            .enqueue(&report)
            .expect("enqueue");

        let mut st = CrashConsentState::default();
        st.set_data_dir(Some(dir.clone()));
        st.load_from_spool();
        let outcome = st.consent_and_send().expect("a report was pending");
        assert_eq!(outcome, ReportOutcome::RefusedNoEndpoint);
        let remaining = open_spool_in(&dir)
            .and_then(|s| s.list().ok())
            .unwrap_or_default();
        assert_eq!(
            remaining.len(),
            1,
            "an un-sent report stays spooled for retry"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn capture_and_drain_round_trips_in_temp_dir() {
        // Capture into a temp data dir, then read the spooled crash back — proves
        // the panic-hook seam spools a real, loadable crash report locally.
        let dir = std::env::temp_dir().join(format!("f0rg3-report-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let report = build_crash_report("boom", "src/x.rs:1");
        let spool = open_spool_in(&dir).expect("open spool");
        spool.enqueue(&report).expect("enqueue");

        let paths = spool.list().expect("list");
        assert!(!paths.is_empty(), "the queued crash must be listed");
        let loaded = spool.load(&paths[0]).expect("load");
        assert_eq!(loaded.stream, Stream::CrashReports);
        assert!(loaded.body.contains("boom"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

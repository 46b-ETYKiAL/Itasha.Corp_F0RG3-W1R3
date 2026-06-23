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
use itasha_report_transport_tor::{TorOnionTransport, TorTransportConfig};

/// The env var that injects the self-hosted ingest endpoint. There is NO
/// hardcoded URL in the installer and NO default — a build with this unset can
/// spool locally but can NEVER transmit (a mis-build cannot phone home). Until
/// the server endpoint is configured, a consented send returns the structured
/// `RefusedNoEndpoint` outcome (never a silent drop, never a fake success).
pub const REPORT_ENDPOINT_ENV: &str = "F0RG3_REPORT_ENDPOINT";

/// The env var that injects the OPT-IN truly-anonymous Tor v3 `.onion` ingest
/// address (56 base32 chars + `.onion`). When set to a structurally-valid onion
/// address, a consented send routes over the in-process Arti Tor transport
/// (sender-IP-free) INSTEAD of the clearnet endpoint. Unset / empty / malformed
/// => the clearnet path is used (default). This is the ONLY opt-in switch: there
/// is no hardcoded onion and no default — a build with this unset behaves
/// identically to before.
pub const REPORT_ONION_ENV: &str = "F0RG3_REPORT_ONION";

/// The env var that overrides the Tor onion virtual port. Defaults to
/// [`DEFAULT_ONION_PORT`] when unset / empty / unparseable. Only consulted when
/// [`REPORT_ONION_ENV`] is set to a valid onion address.
pub const REPORT_ONION_PORT_ENV: &str = "F0RG3_REPORT_ONION_PORT";

/// The default onion virtual port the W1TN3SS onion service publishes its ingest
/// endpoint on (the onion-routed equivalent of `:443`).
pub const DEFAULT_ONION_PORT: u16 = 443;

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

/// The transport the host should use for a consented send, resolved PURELY from
/// the configured onion address + clearnet endpoint (no I/O, no env reads — so
/// the selection is unit-testable without a live network or process env).
///
/// The selection rule is a strict priority: a configured, structurally-valid
/// onion address wins (the truly-anonymous, sender-IP-free path); otherwise a
/// configured clearnet endpoint is used; otherwise nothing can transmit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportChoice {
    /// Route over the opt-in Arti Tor v3 onion transport (sender-IP-free). Carries
    /// the validated onion address + virtual port.
    TorOnion { onion: String, port: u16 },
    /// Route over the clearnet `LeanPipelineBackend` to this endpoint (default).
    Clearnet { endpoint: String },
    /// No transport is configured — a consented send refuses (stays spooled).
    None,
}

/// Resolve the transport choice PURELY from the configured inputs.
///
/// `onion` is the optional onion address (e.g. from [`REPORT_ONION_ENV`]);
/// `onion_port` is the optional onion virtual-port override (from
/// [`REPORT_ONION_PORT_ENV`]); `clearnet_endpoint` is the optional clearnet
/// ingest URL (from [`REPORT_ENDPOINT_ENV`]). A non-empty onion that passes the
/// SDK's structural [`TorTransportConfig::is_valid_onion`] check takes priority
/// over clearnet — when the user has gone to the trouble of configuring an
/// onion, the anonymous path is the intended one. A configured-but-MALFORMED
/// onion does NOT silently fall back to clearnet: it is treated as a
/// misconfiguration and yields [`TransportChoice::None`] (the structured
/// refusal) rather than leaking the report over a less-private channel the user
/// did not intend.
///
/// When the onion path wins, the chosen virtual port is `onion_port` if
/// supplied, else [`DEFAULT_ONION_PORT`].
#[must_use]
pub fn select_transport(
    onion: Option<&str>,
    onion_port: Option<u16>,
    clearnet_endpoint: Option<&str>,
) -> TransportChoice {
    let onion = onion.map(str::trim).filter(|s| !s.is_empty());
    if let Some(onion) = onion {
        let port = onion_port.unwrap_or(DEFAULT_ONION_PORT);
        // Validate with the SDK's own structural check (single source of truth).
        let probe = TorTransportConfig::new(onion, port, ".", ".");
        if probe.is_valid_onion() {
            return TransportChoice::TorOnion {
                onion: onion.to_string(),
                port,
            };
        }
        // Onion configured but malformed — refuse rather than silently downgrade
        // to clearnet (the user opted into anonymity; honour it or refuse).
        return TransportChoice::None;
    }
    match clearnet_endpoint.map(str::trim).filter(|s| !s.is_empty()) {
        Some(endpoint) => TransportChoice::Clearnet {
            endpoint: endpoint.to_string(),
        },
        None => TransportChoice::None,
    }
}

/// Transmit ONE report through the SDK's hardened transport, consent-gated.
///
/// The `consent` argument is mandatory — there is no send path without it (the
/// SDK enforces this at the type level). The host mints the [`ConsentToken`]
/// ONLY after the user explicitly opted in on the failure screen. The outcome
/// is logged.
///
/// **Transport selection (opt-in / default-clearnet):**
/// - If a structurally-valid `.onion` address is configured
///   ([`REPORT_ONION_ENV`]), the report routes over the in-process Arti Tor v3
///   onion transport — truly-anonymous (sender-IP-free). The Tor state/cache +
///   spool live under `<data_dir>/tor-state`, `<data_dir>/tor-cache`,
///   `<data_dir>/tor-spool`.
/// - Otherwise, if a clearnet endpoint is configured ([`REPORT_ENDPOINT_ENV`]),
///   the report routes over the SDK's [`LeanPipelineBackend`] (static
///   User-Agent, zero redirects, bounded timeout, size-capped, NO persistent
///   identifier). **This is the default.**
/// - Otherwise (neither configured, or a malformed onion), this returns
///   [`ReportOutcome::RefusedNoEndpoint`] and transmits nothing — the report
///   stays spooled for a later, configured send.
pub fn send_report(report: &Report, consent: &ConsentToken) -> ReportOutcome {
    let choice = select_transport(
        onion_from_env().as_deref(),
        onion_port_from_env(),
        endpoint_from_env().as_deref(),
    );
    let outcome = dispatch_send(report, consent, &choice, data_dir());
    log_outcome(&outcome);
    outcome
}

/// Execute a send for a resolved [`TransportChoice`]. Split out (taking the
/// already-resolved choice + an explicit data dir) so the dispatch is testable
/// without touching the process env: the `None`/clearnet-refusal branches are
/// asserted directly; the live Tor/clearnet connect paths require a network and
/// are exercised by the SDK's own crate tests + the `#[ignore]`'d onion E2E.
fn dispatch_send(
    report: &Report,
    consent: &ConsentToken,
    choice: &TransportChoice,
    data_dir: Option<PathBuf>,
) -> ReportOutcome {
    match choice {
        TransportChoice::TorOnion { onion, port } => {
            // The Tor transport needs writable state/cache/spool dirs. With no
            // data dir there is nowhere to root them — surface it rather than
            // silently downgrade to clearnet.
            let Some(dir) = data_dir else {
                return ReportOutcome::Failed("no-data-dir".to_string());
            };
            let config = TorTransportConfig::new(
                onion.clone(),
                *port,
                dir.join("tor-state"),
                dir.join("tor-cache"),
            );
            match TorOnionTransport::new(config, dir.join("tor-spool")) {
                Ok(backend) => match backend.send(report, consent) {
                    Ok(SendOutcome::Sent) => ReportOutcome::Sent,
                    Ok(SendOutcome::Failed(reason)) => ReportOutcome::Failed(reason),
                    Err(e) => ReportOutcome::Failed(e.to_string()),
                },
                Err(e) => ReportOutcome::Failed(e.to_string()),
            }
        }
        TransportChoice::Clearnet { endpoint } => {
            let backend = LeanPipelineBackend::new(TransportConfig::new(endpoint.clone()));
            match backend.send(report, consent) {
                Ok(SendOutcome::Sent) => ReportOutcome::Sent,
                Ok(SendOutcome::Failed(reason)) => ReportOutcome::Failed(reason),
                Err(e) => ReportOutcome::Failed(e.to_string()),
            }
        }
        TransportChoice::None => ReportOutcome::RefusedNoEndpoint,
    }
}

/// Read the clearnet ingest endpoint from the env var, treating an empty value
/// as unset.
fn endpoint_from_env() -> Option<String> {
    std::env::var(REPORT_ENDPOINT_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Read the opt-in onion ingest address from the env var, treating an empty
/// value as unset. The structural validity check happens in [`select_transport`]
/// (single source of truth via the SDK), so this only normalizes presence.
fn onion_from_env() -> Option<String> {
    std::env::var(REPORT_ONION_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Read the opt-in onion virtual-port override from the env var. An unset,
/// empty, or unparseable value yields `None` so [`select_transport`] falls back
/// to [`DEFAULT_ONION_PORT`] — a garbled port is never a hard failure, it just
/// means "use the default port" (the onion address itself is the load-bearing
/// opt-in; the port is a convenience override for a non-`:443` onion service).
fn onion_port_from_env() -> Option<u16> {
    std::env::var(REPORT_ONION_PORT_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<u16>().ok())
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
    /// Serializes the tests that mutate the platform data-dir env var
    /// (`LOCALAPPDATA` on Windows / `XDG_DATA_HOME` + `HOME` elsewhere), which
    /// the GLOBAL [`data_dir`] reads. Without this lock, two such tests running
    /// concurrently would clobber each other's env.
    static DATA_DIR_LOCK: Mutex<()> = Mutex::new(());

    /// Point the GLOBAL [`data_dir`] at `dir` for the duration of the returned
    /// guards by setting the platform per-user data-dir env var. On Windows that
    /// is `LOCALAPPDATA` (and [`data_dir`] appends `itasha-installer`); elsewhere
    /// it is `XDG_DATA_HOME` (with `HOME` also pinned so the fallback can never
    /// resolve to the real home). The caller MUST hold [`DATA_DIR_LOCK`] first.
    /// The installer-suffix means the spool is rooted at `<dir>/itasha-installer`,
    /// so each test gets a private, collision-free tree.
    fn point_data_dir_at(dir: &Path) -> Vec<EnvGuard> {
        let p = dir.to_str().expect("temp dir is valid UTF-8");
        #[cfg(windows)]
        {
            vec![EnvGuard::set("LOCALAPPDATA", p)]
        }
        #[cfg(not(windows))]
        {
            vec![EnvGuard::set("XDG_DATA_HOME", p), EnvGuard::set("HOME", p)]
        }
    }

    /// The spool dir the GLOBAL [`data_dir`] resolves to under
    /// [`point_data_dir_at`] (it appends `itasha-installer`).
    fn global_spool_root(base: &Path) -> PathBuf {
        base.join("itasha-installer")
    }

    #[test]
    fn crash_report_is_crash_stream_and_carries_static_message() {
        // A plain prose panic message survives the SDK sanitizer verbatim (no
        // high-entropy token, no path, no identity). The panic SITE is always
        // preserved. (A message that embeds a symbol run like
        // `Option::unwrap()` may be free-text-redacted by the SDK's anonymity
        // hardening — that is asserted separately below.)
        let r = build_crash_report("index out of bounds", "src/foo.rs:42");
        assert_eq!(r.stream, Stream::CrashReports);
        assert!(r.body.contains("index out of bounds"));
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

    /// A structurally-valid v3 onion address (56 lowercase base32 chars).
    fn valid_onion() -> String {
        format!("{}.onion", "a".repeat(56))
    }

    #[test]
    fn select_transport_prefers_a_valid_onion_over_clearnet() {
        // When BOTH an onion and a clearnet endpoint are configured, the
        // anonymous onion path wins (the user opted into anonymity).
        let onion = valid_onion();
        let choice = select_transport(Some(&onion), None, Some("https://ingest.example.test"));
        assert_eq!(
            choice,
            TransportChoice::TorOnion {
                onion: onion.clone(),
                port: DEFAULT_ONION_PORT,
            },
            "a configured valid onion must take priority over clearnet"
        );
    }

    #[test]
    fn select_transport_honours_the_onion_port_override() {
        // A supplied port override is carried through onto the chosen onion
        // transport instead of DEFAULT_ONION_PORT.
        let onion = valid_onion();
        let choice = select_transport(Some(&onion), Some(9150), None);
        assert_eq!(
            choice,
            TransportChoice::TorOnion { onion, port: 9150 },
            "a configured onion port override must be carried through"
        );
    }

    #[test]
    fn select_transport_uses_clearnet_when_no_onion_configured() {
        // The DEFAULT path: no onion configured, clearnet endpoint present.
        let choice = select_transport(None, None, Some("https://ingest.example.test"));
        assert_eq!(
            choice,
            TransportChoice::Clearnet {
                endpoint: "https://ingest.example.test".to_string(),
            },
            "with no onion, a configured clearnet endpoint is the default transport"
        );
    }

    #[test]
    fn select_transport_is_none_when_nothing_configured() {
        // Neither configured => no transport => the structured refusal (default
        // out-of-the-box state: the installer cannot phone home).
        assert_eq!(select_transport(None, None, None), TransportChoice::None);
        // Empty/whitespace strings are treated as unset.
        assert_eq!(
            select_transport(Some("   "), None, Some("   ")),
            TransportChoice::None
        );
    }

    #[test]
    fn select_transport_refuses_a_malformed_onion_without_clearnet_downgrade() {
        // A configured-but-malformed onion must NOT silently downgrade to
        // clearnet — the user opted into anonymity; honour it or refuse.
        let choice = select_transport(
            Some("not-a-real.onion"),
            None,
            Some("https://ingest.example.test"),
        );
        assert_eq!(
            choice,
            TransportChoice::None,
            "a malformed onion refuses rather than leaking over clearnet"
        );
    }

    #[test]
    fn dispatch_send_none_choice_refuses_and_transmits_nothing() {
        // The TransportChoice::None branch is the structured refusal regardless
        // of consent — proven without touching the process env or a network.
        let report = build_crash_report("boom", "src/x.rs:1");
        let token = ConsentToken::granted();
        let outcome = dispatch_send(&report, &token, &TransportChoice::None, None);
        assert_eq!(outcome, ReportOutcome::RefusedNoEndpoint);
    }

    #[test]
    fn dispatch_send_tor_without_data_dir_fails_not_silently_clearnet() {
        // A Tor choice with no data dir has nowhere to root its state/cache/spool
        // — it surfaces a structured failure, NEVER a silent clearnet downgrade
        // and NEVER a fake Sent.
        let report = build_crash_report("boom", "src/x.rs:1");
        let token = ConsentToken::granted();
        let choice = TransportChoice::TorOnion {
            onion: valid_onion(),
            port: DEFAULT_ONION_PORT,
        };
        let outcome = dispatch_send(&report, &token, &choice, None);
        assert_eq!(outcome, ReportOutcome::Failed("no-data-dir".to_string()));
    }

    #[test]
    fn send_with_neither_env_set_refuses_default_off() {
        // End-to-end through the env-reading send_report: with NEITHER the
        // clearnet endpoint NOR the onion env set (the default install state),
        // a consented send refuses and transmits nothing.
        let _lock = ENDPOINT_LOCK.lock().unwrap();
        let _ge = EnvGuard::unset(REPORT_ENDPOINT_ENV);
        let _go = EnvGuard::unset(REPORT_ONION_ENV);
        let report = build_crash_report("boom", "src/x.rs:1");
        let token = ConsentToken::granted();
        assert_eq!(
            send_report(&report, &token),
            ReportOutcome::RefusedNoEndpoint,
            "default-OFF: no transport configured => structured refusal"
        );
    }

    #[test]
    fn empty_onion_env_is_treated_as_unset() {
        let _lock = ENDPOINT_LOCK.lock().unwrap();
        let _g = EnvGuard::set(REPORT_ONION_ENV, "   ");
        assert!(
            onion_from_env().is_none(),
            "a whitespace-only onion env must be treated as unset"
        );
    }

    /// A live onion connect is NOT exercised in unit tests (it needs a running
    /// Tor network + a real onion service). The selection + dispatch wiring is
    /// covered by the pure tests above; the actual Arti connect path is covered
    /// by the transport-tor crate's own tests. This placeholder documents the
    /// boundary and is `#[ignore]`'d so a CI run never attempts a network send.
    #[test]
    #[ignore = "live onion E2E needs a running Tor network + real onion service"]
    fn onion_send_e2e_placeholder() {
        // Intentionally empty: the live path is the SDK crate's responsibility.
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

    // --- capture_panic: the panic-hook capture seam over the GLOBAL data_dir ---

    #[test]
    fn capture_panic_spools_into_the_global_data_dir() {
        // With the platform data-dir env pointed at a temp tree, capture_panic
        // resolves the GLOBAL data_dir(), opens the spool, builds the sanitized
        // crash report, and ENQUEUES it — returning Spooled (never Sent: the
        // panic hook transmits NOTHING). The spooled report is a real, loadable
        // CrashReports entry carrying our static message + site.
        let _lock = DATA_DIR_LOCK.lock().unwrap();
        let base = std::env::temp_dir().join(format!("f0rg3-capture-ok-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("mkdir");
        let _guards = point_data_dir_at(&base);

        let outcome = capture_panic("kaboom", "src/q.rs:7");
        assert_eq!(
            outcome,
            ReportOutcome::Spooled,
            "a captured panic spools locally (never transmits)"
        );

        // Prove the report actually landed in the global-resolved spool and is a
        // loadable crash carrying our payload (kills a mutant that returns
        // Spooled without enqueueing).
        let spool_root = global_spool_root(&base);
        let spool = open_spool_in(&spool_root).expect("spool exists at global data dir");
        let paths = spool.list().expect("list");
        assert_eq!(paths.len(), 1, "exactly one report was spooled");
        let loaded = spool.load(&paths[0]).expect("load");
        assert_eq!(loaded.stream, Stream::CrashReports);
        assert!(loaded.body.contains("kaboom"));
        assert!(loaded.body.contains("src/q.rs:7"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    #[cfg(not(windows))]
    fn capture_panic_without_a_data_dir_fails_no_data_dir() {
        // When NO platform data dir resolves (no XDG_DATA_HOME and no HOME),
        // data_dir() is None, so capture_panic surfaces the structured
        // "no-data-dir" failure rather than silently swallowing the crash.
        // (Windows always has LOCALAPPDATA-class resolution paths in CI, so this
        // None branch is asserted on the unix data_dir() shape.)
        let _lock = DATA_DIR_LOCK.lock().unwrap();
        let _gx = EnvGuard::unset("XDG_DATA_HOME");
        let _gh = EnvGuard::unset("HOME");
        let outcome = capture_panic("kaboom", "src/q.rs:7");
        assert_eq!(
            outcome,
            ReportOutcome::Failed("no-data-dir".to_string()),
            "no data dir => structured failure, never a silent drop"
        );
    }

    #[test]
    fn capture_panic_is_silent_under_disable_telemetry_but_still_spools() {
        // S4F3_DISABLE_TELEMETRY=1 silences the outcome LOG (the early return in
        // log_outcome) but does NOT change the capture behaviour: the report is
        // still spooled. Privacy-respecting telemetry suppression must never
        // become a silent capture failure.
        let _lock = DATA_DIR_LOCK.lock().unwrap();
        let _gt = EnvGuard::set("S4F3_DISABLE_TELEMETRY", "1");
        let base =
            std::env::temp_dir().join(format!("f0rg3-capture-silent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("mkdir");
        let _guards = point_data_dir_at(&base);

        let outcome = capture_panic("hush", "src/q.rs:9");
        assert_eq!(outcome, ReportOutcome::Spooled);

        let spool = open_spool_in(&global_spool_root(&base)).expect("spool");
        assert_eq!(
            spool.list().expect("list").len(),
            1,
            "still spooled silently"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn data_dir_resolves_under_a_pointed_env() {
        // data_dir() returns Some under a pointed platform env (and the path is
        // suffixed with the installer namespace). This pins the resolver the
        // capture seam depends on.
        let _lock = DATA_DIR_LOCK.lock().unwrap();
        let base = std::env::temp_dir().join(format!("f0rg3-datadir-{}", std::process::id()));
        let _guards = point_data_dir_at(&base);
        let resolved = data_dir().expect("a pointed env resolves a data dir");
        assert!(
            resolved.ends_with("itasha-installer"),
            "the data dir is namespaced to the installer: {resolved:?}"
        );
    }

    // --- CrashConsentState::load_from_spool stream-filtering + multi-advance ---

    #[test]
    fn load_from_spool_keeps_only_crash_stream_reports() {
        // The dialog drains CRASH reports only — a spooled ManualIssues report
        // (the other stream the SDK can spool) is FILTERED OUT, never presented
        // on the crash-consent dialog. This exercises the false arm of the
        // `stream == CrashReports` filter (a mutant that drops the filter would
        // wrongly surface the manual issue).
        let dir = std::env::temp_dir().join(format!("f0rg3-filter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let spool = open_spool_in(&dir).expect("spool");
        // One crash + one manual issue spooled together.
        spool
            .enqueue(&build_crash_report("real-crash", "src/x.rs:1"))
            .expect("enqueue crash");
        spool
            .enqueue(&Report::manual_issue("a title", "a manual issue body"))
            .expect("enqueue manual");

        let mut st = CrashConsentState::default();
        st.set_data_dir(Some(dir.clone()));
        let n = st.load_from_spool();
        assert_eq!(
            n, 1,
            "only the crash report is queued+presented (manual filtered out)"
        );
        assert!(st.has_pending());
        assert!(
            st.edited_text_mut().contains("real-crash"),
            "the presented report is the crash, not the manual issue"
        );
        assert!(
            !st.edited_text_mut().contains("a manual issue body"),
            "the manual issue is never surfaced on the crash dialog"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_spool_presents_multiple_crashes_in_sequence() {
        // Two spooled crashes: load presents the first; declining advances to the
        // second; declining again clears. This drives `advance` across a
        // non-empty queue more than once (the multi-report advance path) and
        // proves each decline removes exactly one spooled file.
        let dir = std::env::temp_dir().join(format!("f0rg3-multi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let spool = open_spool_in(&dir).expect("spool");
        spool
            .enqueue(&build_crash_report("first-crash", "src/a.rs:1"))
            .expect("enqueue 1");
        spool
            .enqueue(&build_crash_report("second-crash", "src/b.rs:2"))
            .expect("enqueue 2");

        let mut st = CrashConsentState::default();
        st.set_data_dir(Some(dir.clone()));
        let n = st.load_from_spool();
        assert_eq!(n, 2, "both crashes are queued");
        assert!(st.has_pending(), "the first crash is presented");

        // Decline the first -> advance to the second.
        st.decline_and_discard();
        assert!(st.has_pending(), "the second crash is now presented");
        assert_eq!(
            open_spool_in(&dir)
                .and_then(|s| s.count().ok())
                .unwrap_or(99),
            1,
            "declining removed exactly the first spooled file"
        );

        // Decline the second -> queue drained, nothing pending.
        st.decline_and_discard();
        assert!(
            !st.has_pending(),
            "the queue is drained after both declines"
        );
        assert_eq!(
            open_spool_in(&dir)
                .and_then(|s| s.count().ok())
                .unwrap_or(99),
            0,
            "both declined files are removed from the spool"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_spool_skips_a_corrupt_spool_file_without_crashing() {
        // A garbage `report-*.json` file in the spool dir is LISTED by the SDK
        // (it matches the name pattern) but FAILS to load (invalid JSON). The
        // dialog must skip it (the `if let Ok(report)` false arm in
        // load_from_spool) and still present the one valid crash — a corrupt
        // spool entry is best-effort-skipped, never a crash and never surfaced
        // as an empty/garbled report.
        let dir = std::env::temp_dir().join(format!("f0rg3-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let spool = open_spool_in(&dir).expect("spool");
        spool
            .enqueue(&build_crash_report("good-crash", "src/a.rs:1"))
            .expect("enqueue good");
        // Hand-write a corrupt entry matching the SDK's `report-*.json` listing
        // pattern but with non-JSON content so `spool.load` returns Err.
        std::fs::write(
            spool.dir().join("report-zzzzzzzzzz-corrupt.json"),
            b"this is not valid json {{{",
        )
        .expect("write corrupt file");

        let mut st = CrashConsentState::default();
        st.set_data_dir(Some(dir.clone()));
        let n = st.load_from_spool();
        assert_eq!(
            n, 1,
            "the corrupt entry is skipped; only the valid crash is queued"
        );
        assert!(st.has_pending(), "the valid crash is still presented");
        assert!(
            st.edited_text_mut().contains("good-crash"),
            "the surviving report is the valid crash, not the corrupt entry"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn advance_skips_a_corrupt_entry_queued_after_a_valid_one() {
        // Drive `advance`'s load-failure arm directly: a queue whose SECOND entry
        // is corrupt. Declining the first advances to the second; the second's
        // load fails so the dialog ends with nothing pending (the corrupt entry
        // is not surfaced). This covers the `if let Ok(report)` false arm inside
        // advance (distinct from the load_from_spool filter loop).
        //
        // load_from_spool only QUEUES entries that load+match the crash stream,
        // so to seat a corrupt path in the queue we set the data dir, enqueue a
        // valid crash, load it (queueing nothing extra), then corrupt-replace the
        // CURRENT report's backing file before declining — forcing advance to try
        // to (re)load a now-unreadable path is not possible via the public API;
        // instead we seat TWO valid crashes, corrupt the SECOND's file after
        // load_from_spool has queued it, and decline the first so advance hits
        // the corrupt second.
        let dir = std::env::temp_dir().join(format!("f0rg3-adv-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let spool = open_spool_in(&dir).expect("spool");
        // Filenames are timestamp-stamped + sorted; enqueue with a beat between
        // so ordering is deterministic (first listed = first queued).
        let p1 = spool
            .enqueue(&build_crash_report("first", "src/a.rs:1"))
            .expect("enqueue 1");
        std::thread::sleep(std::time::Duration::from_millis(5));
        let p2 = spool
            .enqueue(&build_crash_report("second", "src/b.rs:2"))
            .expect("enqueue 2");
        assert!(p1 < p2, "first enqueue sorts before second");

        let mut st = CrashConsentState::default();
        st.set_data_dir(Some(dir.clone()));
        let n = st.load_from_spool();
        assert_eq!(n, 2, "both valid crashes are queued");
        assert!(
            st.edited_text_mut().contains("first"),
            "the first crash is presented"
        );

        // Now corrupt the SECOND file's content so advance's reload of it fails.
        std::fs::write(&p2, b"corrupted-after-queueing {{{").expect("corrupt second");

        // Decline the first -> advance pops the (now-corrupt) second; its load
        // fails so the dialog clears `current` (nothing pending) rather than
        // surfacing a broken report.
        st.decline_and_discard();
        assert!(
            !st.has_pending(),
            "the corrupt second entry is not surfaced; the dialog clears"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn capture_panic_surfaces_spool_open_failure() {
        // Fault-inject a spool-OPEN failure: point the GLOBAL data_dir at a path
        // that already exists AS A FILE, so `Spool::open` (which create_dir_all's
        // `<data_dir>/reports`) fails. capture_panic must surface the structured
        // "spool-open: …" Failed outcome — never a silent swallow, never a fake
        // Spooled.
        let _lock = DATA_DIR_LOCK.lock().unwrap();
        // data_dir() resolves to <env>/itasha-installer, then Spool::open creates
        // <env>/itasha-installer/reports. Make `itasha-installer` a regular file
        // so the `reports` subdir cannot be created under it.
        let base =
            std::env::temp_dir().join(format!("f0rg3-spool-open-fail-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("mkdir base");
        // Create the `itasha-installer` path as a FILE (not a dir).
        std::fs::write(base.join("itasha-installer"), b"i am a file, not a dir")
            .expect("write blocking file");
        let _guards = point_data_dir_at(&base);

        let outcome = capture_panic("boom", "src/x.rs:1");
        match outcome {
            ReportOutcome::Failed(detail) => assert!(
                detail.starts_with("spool-open:"),
                "a spool-open failure surfaces the structured spool-open detail, got: {detail}"
            ),
            other => panic!("expected a spool-open Failed outcome, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn decline_with_unbound_spool_clears_current_without_panicking() {
        // A report is loaded (current is Some), then the data dir is UNBOUND.
        // decline_and_discard takes `current` but `self.spool()` is None, so the
        // remove is skipped (the `if let Some(spool)` false arm) — the dialog
        // still clears cleanly. Covers decline's spool-None branch.
        let dir = std::env::temp_dir().join(format!("f0rg3-decline-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        open_spool_in(&dir)
            .unwrap()
            .enqueue(&build_crash_report("boom", "src/x.rs:1"))
            .expect("enqueue");
        let mut st = CrashConsentState::default();
        st.set_data_dir(Some(dir.clone()));
        st.load_from_spool();
        assert!(st.has_pending(), "a report is loaded");
        // Unbind the spool, THEN decline.
        st.set_data_dir(None);
        st.decline_and_discard();
        assert!(
            !st.has_pending(),
            "the dialog clears even with no bound spool"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn advance_with_unbound_spool_and_nonempty_queue_presents_nothing() {
        // Seat TWO crashes in the queue, then UNBIND the spool before declining.
        // Declining the first calls advance with a NON-EMPTY queue but no bound
        // spool, so advance's `if let Some(spool)` false arm runs: it pops the
        // path but cannot reload it, leaving `current` None. The dialog clears
        // rather than surfacing a report it cannot read.
        let dir = std::env::temp_dir().join(format!("f0rg3-adv-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let spool = open_spool_in(&dir).expect("spool");
        spool
            .enqueue(&build_crash_report("first", "src/a.rs:1"))
            .expect("enqueue 1");
        std::thread::sleep(std::time::Duration::from_millis(5));
        spool
            .enqueue(&build_crash_report("second", "src/b.rs:2"))
            .expect("enqueue 2");
        let mut st = CrashConsentState::default();
        st.set_data_dir(Some(dir.clone()));
        assert_eq!(st.load_from_spool(), 2, "both queued");
        // Unbind: now the queue still holds the second path, but spool() is None.
        st.set_data_dir(None);
        st.decline_and_discard();
        assert!(
            !st.has_pending(),
            "advance with an unbound spool cannot reload the next entry; dialog clears"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_spool_with_unbound_spool_presents_nothing() {
        // load_from_spool with no bound data dir skips the spool-read entirely
        // (the `if let Some(spool)` false arm in load_from_spool) and presents
        // nothing — the inert default-state path, asserted distinctly from the
        // default-construction test.
        let mut st = CrashConsentState::default();
        st.set_data_dir(None);
        assert_eq!(st.load_from_spool(), 0);
        assert!(!st.has_pending());
    }

    #[test]
    fn consent_and_send_returns_none_when_nothing_pending() {
        // With no report loaded, consent_and_send is a no-op returning None (the
        // `self.current.take()?` early bail). A mutant that proceeds to send on
        // an empty dialog is killed here.
        let mut st = CrashConsentState::default();
        assert!(
            st.consent_and_send().is_none(),
            "no pending report => Send is a no-op"
        );
    }

    #[test]
    fn consent_and_send_without_endpoint_advances_to_next_and_keeps_both_spooled() {
        // Two spooled crashes, no endpoint. Pressing Send on the first returns
        // RefusedNoEndpoint, KEEPS the file spooled (no fake Sent / silent drop),
        // and ADVANCES to the second. This drives consent_and_send's NON-Sent
        // path (the file-retain branch) plus the post-send advance.
        let _lock = ENDPOINT_LOCK.lock().unwrap();
        let _ge = EnvGuard::unset(REPORT_ENDPOINT_ENV);
        let _go = EnvGuard::unset(REPORT_ONION_ENV);
        let dir = std::env::temp_dir().join(format!("f0rg3-send-advance-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let spool = open_spool_in(&dir).expect("spool");
        spool
            .enqueue(&build_crash_report("c1", "src/a.rs:1"))
            .expect("enqueue 1");
        spool
            .enqueue(&build_crash_report("c2", "src/b.rs:2"))
            .expect("enqueue 2");

        let mut st = CrashConsentState::default();
        st.set_data_dir(Some(dir.clone()));
        st.load_from_spool();

        let outcome = st.consent_and_send().expect("first report pending");
        assert_eq!(
            outcome,
            ReportOutcome::RefusedNoEndpoint,
            "no endpoint => structured refusal, not a fake Sent"
        );
        assert!(st.has_pending(), "Send advanced to the second crash");
        assert_eq!(
            open_spool_in(&dir)
                .and_then(|s| s.count().ok())
                .unwrap_or(99),
            2,
            "an un-sent report is KEPT spooled for retry (neither file removed)"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn env_guard_restores_a_preexisting_value_on_drop() {
        // The EnvGuard test fixture must RESTORE a pre-existing env value on drop
        // (the `Some(prev)` restore arm), not leave the override or wrongly
        // delete a value that was there before. This pins the fixture the other
        // env-mutating tests rely on for isolation.
        let key = "F0RG3_ENVGUARD_FIXTURE_PROBE";
        // Ensure a known prior value exists.
        std::env::set_var(key, "original");
        {
            let _g = EnvGuard::set(key, "overridden");
            assert_eq!(std::env::var(key).as_deref(), Ok("overridden"));
        }
        assert_eq!(
            std::env::var(key).as_deref(),
            Ok("original"),
            "the guard restored the pre-existing value on drop"
        );
        // And the unset guard restores a previously-absent var to absent.
        std::env::remove_var(key);
        {
            let _g = EnvGuard::set(key, "temp");
            assert!(std::env::var(key).is_ok());
        }
        assert!(
            std::env::var(key).is_err(),
            "the guard restored the var to ABSENT (it had no prior value)"
        );
    }
}

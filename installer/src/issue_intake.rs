//! W1TN3SS manual "Report an issue" intake — the F0RG3-W1R3 installer host glue.
//!
//! This is the user-initiated counterpart to [`crate::reporting`] (the opt-in
//! CRASH path). It owns NO transport: it builds a prefilled GitHub **Issue-Form
//! deep link** and hands it to the user's browser, or — when the URL would
//! exceed the helper's `GITHUB_URL_LENGTH_THRESHOLD` (the HTTP-414 ceiling) —
//! yields a clipboard body the host surfaces, or offers a `mailto:` fallback to
//! a support alias. All of the URL / clipboard-body / mailto building is
//! DELEGATED to `itasha_report_core::intake` (the consumed seam, pinned by git
//! tag); this module only wires the dialog UX state and the launch decision.
//!
//! The installer has no clipboard crate, so the clipboard fallback returns the
//! body text for the host to surface (the failure screen shows it for a manual
//! copy) — keeping this module pure + headlessly testable.
//!
//! Privacy invariants (asserted by the tests in this module):
//! - **User-initiated only.** Nothing happens until the user presses Open /
//!   Copy / Email on the failure screen. There is no background or default-on
//!   path.
//! - **Previewable + editable.** The prefilled body is shown in an editable
//!   field BEFORE any browser / mail client opens.
//! - **Diagnostics OFF by default.** No app version / OS / target line is
//!   included unless the user explicitly ticks the diagnostics toggle.
//! - **No persistent identifier.** No install-id / fingerprint / session-id is
//!   ever built into the title, body, query string, or mailto.

use itasha_report_core::intake::{
    clipboard_fallback_body, mailto_url, IssueFormRequest, GITHUB_URL_LENGTH_THRESHOLD,
};

/// The GitHub `owner/repo` the prefilled Issue-Form deep link targets — the
/// installer's own public repo.
pub const ISSUE_REPO: &str = "46b-ETYKiAL/Itasha.Corp_F0RG3-W1R3";

/// The support email alias the `mailto:` fallback addresses.
pub const MAILTO_ALIAS: &str = "46b.AbandonSomething@proton.me";

/// The kind of issue the user is filing. Each maps to a shared Issue-Form
/// template filename in the public W1TN3SS repo and the server-side label that
/// template applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IssueKind {
    /// A bug report → `bug.yml` (label `bug`).
    #[default]
    Bug,
    /// A feature request → `feature.yml` (label `enhancement`).
    Feature,
    /// A question or anything else → `other.yml` (label `question`).
    Other,
}

impl IssueKind {
    /// All kinds, in display order (for the dialog's selector).
    pub const ALL: [IssueKind; 3] = [IssueKind::Bug, IssueKind::Feature, IssueKind::Other];

    /// The Issue-Form template filename this kind targets.
    #[must_use]
    pub fn template(self) -> &'static str {
        match self {
            IssueKind::Bug => "bug.yml",
            IssueKind::Feature => "feature.yml",
            IssueKind::Other => "other.yml",
        }
    }

    /// The server-side label this kind applies.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            IssueKind::Bug => "bug",
            IssueKind::Feature => "enhancement",
            IssueKind::Other => "question",
        }
    }

    /// A human label for the dialog selector.
    #[must_use]
    pub fn display(self) -> &'static str {
        match self {
            IssueKind::Bug => "Bug",
            IssueKind::Feature => "Feature request",
            IssueKind::Other => "Question / other",
        }
    }

    /// The default issue title prefix for this kind.
    #[must_use]
    pub fn title_prefix(self) -> &'static str {
        match self {
            IssueKind::Bug => "bug: ",
            IssueKind::Feature => "feat: ",
            IssueKind::Other => "other: ",
        }
    }
}

/// The path the intake took, logged counts/enums only (never the body content).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntakeOutcome {
    /// The prefilled Issue-Form URL was opened in the browser.
    OpenedDeepLink,
    /// The URL exceeded the length ceiling (or the browser could not launch),
    /// so the body was surfaced for a manual copy.
    CopiedToClipboard,
    /// The `mailto:` fallback opened a mail client.
    OpenedMailto,
    /// The action could not be completed. The reason is non-identifying.
    Failed(String),
}

impl IntakeOutcome {
    /// The stable, non-identifying log detail for this outcome.
    #[must_use]
    pub fn log_detail(&self) -> &'static str {
        match self {
            IntakeOutcome::OpenedDeepLink => "deep-link",
            IntakeOutcome::CopiedToClipboard => "clipboard",
            IntakeOutcome::OpenedMailto => "mailto",
            IntakeOutcome::Failed(_) => "failed",
        }
    }
}

/// The decision [`open_or_copy`] reached for the host to act on. The host opens
/// the browser / surfaces the clipboard body accordingly — the module itself
/// never touches the OS clipboard (the installer has no clipboard crate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntakeAction {
    /// The SDK already launched the browser — the URL fit. Report the outcome.
    Opened(IntakeOutcome),
    /// Surface this text for the user to copy manually, then the host reports
    /// [`IntakeOutcome::CopiedToClipboard`].
    CopyToClipboard(String),
}

/// Build the host-provided diagnostics block: app version, OS, and target.
/// NON-identifying and only present when the user explicitly opts in. NO
/// install-id / fingerprint / session-id — compile-time + `std::env::consts`
/// values only.
#[must_use]
pub fn diagnostics_block() -> String {
    format!(
        "\n\n---\nApp version: {}\nOS: {}\nArch: {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// Build the prefilled issue BODY from the user's description plus an OPTIONAL,
/// opt-in diagnostics block. The body carries NO persistent identifier — only
/// the user's own text and (if toggled) the non-identifying diagnostics.
#[must_use]
pub fn build_body(description: &str, include_diagnostics: bool) -> String {
    let mut body = description.to_string();
    if include_diagnostics {
        body.push_str(&diagnostics_block());
    }
    body
}

/// Build the [`IssueFormRequest`] for the consumed SDK helper from the dialog's
/// current state.
#[must_use]
pub fn build_request(
    repo: &str,
    kind: IssueKind,
    title_tail: &str,
    body: &str,
) -> IssueFormRequest {
    let title = format!("{}{}", kind.title_prefix(), title_tail.trim());
    IssueFormRequest {
        repo: repo.to_string(),
        title,
        body: body.to_string(),
        template: Some(kind.template().to_string()),
        labels: vec![kind.label().to_string()],
    }
}

/// Derive the one-line issue title tail from a free-form description: its first
/// non-empty line, capped so the title is not the whole body. Pure.
#[must_use]
pub fn title_tail_from(description: &str) -> String {
    let line = description
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if line.chars().count() > 80 {
        line.chars().take(80).collect()
    } else {
        line.to_string()
    }
}

/// The dialog state owned by the app. Default-constructed state is inert: the
/// dialog is closed, diagnostics are OFF, and the description is empty.
#[derive(Debug, Clone, Default)]
pub struct IssueIntakeState {
    /// Whether the modal is currently shown.
    pub open: bool,
    /// The selected issue kind (Bug / Feature / Other).
    pub kind: IssueKind,
    /// The user's free-form description (bound to a `TextEdit`).
    pub description: String,
    /// Whether to include the non-identifying diagnostics block. **OFF by
    /// default** — the user must explicitly opt in.
    pub include_diagnostics: bool,
    /// The last outcome (for a small status line), if any.
    pub last_outcome: Option<IntakeOutcome>,
}

impl IssueIntakeState {
    /// Open the dialog fresh: clear the previous description + outcome and reset
    /// diagnostics to OFF (so reopening never silently re-enables diagnostics).
    pub fn open_fresh(&mut self) {
        self.open = true;
        self.kind = IssueKind::default();
        self.description.clear();
        self.include_diagnostics = false;
        self.last_outcome = None;
    }

    /// Seed the dialog from a failed install: prefill the description with the
    /// installer error text so the user can file it with one edit. Diagnostics
    /// still default OFF (the user opts in). The error text is the installer's
    /// OWN structured `engine` error string (a controlled message), shown to
    /// the user in the editable preview before any send.
    pub fn open_from_failure(&mut self, error: &str) {
        self.open_fresh();
        self.kind = IssueKind::Bug;
        self.description = format!("Install failed.\n\nError: {error}\n\nWhat I was doing:\n");
    }

    /// The EXACT body text that will be sent, given the current state.
    #[must_use]
    pub fn preview_body(&self) -> String {
        build_body(&self.description, self.include_diagnostics)
    }

    /// Build the [`IssueFormRequest`] for the current state against `repo`.
    #[must_use]
    pub fn request(&self, repo: &str) -> IssueFormRequest {
        let body = self.preview_body();
        let title_tail = title_tail_from(&self.description);
        build_request(repo, self.kind, &title_tail, &body)
    }

    /// Whether the current request's deep-link URL fits under the helper's
    /// length ceiling (browser path viable) or must use the clipboard fallback.
    #[must_use]
    pub fn fits_url_length(&self, repo: &str) -> bool {
        self.request(repo).fits_url_length()
    }
}

/// Decide the GitHub deep-link path for `req`: if the URL fits the length
/// ceiling, open it in the browser (via the SDK launcher) and return
/// [`IntakeAction::Opened`]; otherwise (or if the browser cannot launch) return
/// [`IntakeAction::CopyToClipboard`] with the body the host surfaces.
///
/// Split out (taking an already-built [`IssueFormRequest`]) so it is testable
/// without a live browser: the length decision is asserted directly and the
/// clipboard body is produced deterministically.
#[must_use]
pub fn open_or_copy(req: &IssueFormRequest) -> IntakeAction {
    let url = req.to_url();
    if url.len() <= GITHUB_URL_LENGTH_THRESHOLD {
        match itasha_report_core::intake::launch(&url) {
            Ok(()) => IntakeAction::Opened(IntakeOutcome::OpenedDeepLink),
            // Browser could not launch (headless / offline): fall back to the
            // surfaced body so the user never loses their report.
            Err(_) => IntakeAction::CopyToClipboard(clipboard_fallback_body(req)),
        }
    } else {
        // URL would 414 — surface the body for a manual paste.
        IntakeAction::CopyToClipboard(clipboard_fallback_body(req))
    }
}

/// Execute the `mailto:` fallback: build a `mailto:` to `alias` with the issue
/// title as subject and the body, then open it. Returns the outcome.
#[must_use]
pub fn open_mailto(alias: &str, subject: &str, body: &str) -> IntakeOutcome {
    let url = mailto_url(alias, subject, body);
    match itasha_report_core::intake::launch(&url) {
        Ok(()) => IntakeOutcome::OpenedMailto,
        Err(e) => IntakeOutcome::Failed(format!("could not open mail client: {e}")),
    }
}

/// Record the intake outcome counts/enums only (the stable `log_detail`, NEVER
/// the body text, the URL, the repo, or any persistent identifier). Honours
/// `S4F3_DISABLE_TELEMETRY=1` by emitting nothing. Best-effort; never blocks.
pub fn log_outcome(outcome: &IntakeOutcome) {
    if std::env::var_os("S4F3_DISABLE_TELEMETRY").is_some() {
        return;
    }
    eprintln!("f0rg3::issue_intake detail={}", outcome.log_detail());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;

    /// A string that must NEVER appear in any built URL or body.
    fn assert_no_persistent_id(haystack: &str) {
        for forbidden in [
            "install_id",
            "install-id",
            "fingerprint",
            "session_id",
            "session-id",
            "machine_id",
            "device_id",
            "client_id",
            "uuid",
        ] {
            assert!(
                !haystack.to_ascii_lowercase().contains(forbidden),
                "built text must carry no persistent identifier, found {forbidden:?}"
            );
        }
    }

    #[test]
    fn kinds_map_to_templates_and_labels() {
        assert_eq!(IssueKind::Bug.template(), "bug.yml");
        assert_eq!(IssueKind::Bug.label(), "bug");
        assert_eq!(IssueKind::Feature.template(), "feature.yml");
        assert_eq!(IssueKind::Feature.label(), "enhancement");
        assert_eq!(IssueKind::Other.template(), "other.yml");
        assert_eq!(IssueKind::Other.label(), "question");
        assert_eq!(IssueKind::ALL.len(), 3);
    }

    #[test]
    fn diagnostics_off_by_default() {
        let st = IssueIntakeState::default();
        assert!(
            !st.include_diagnostics,
            "diagnostics MUST default OFF (privacy-conservative)"
        );
        assert!(!st.open, "dialog defaults closed");
    }

    #[test]
    fn open_fresh_resets_diagnostics_off() {
        let mut st = IssueIntakeState::default();
        st.include_diagnostics = true;
        st.description = "old text".into();
        st.open_fresh();
        assert!(
            !st.include_diagnostics,
            "reopening must reset diagnostics to OFF (no silent re-enable)"
        );
        assert!(st.description.is_empty());
        assert!(st.open);
    }

    #[test]
    fn open_from_failure_prefills_bug_and_keeps_diagnostics_off() {
        let mut st = IssueIntakeState::default();
        st.open_from_failure("mkdir C:\\Program Files\\x: access denied");
        assert!(st.open);
        assert_eq!(st.kind, IssueKind::Bug);
        assert!(st.description.contains("Install failed"));
        assert!(st.description.contains("access denied"));
        assert!(
            !st.include_diagnostics,
            "a failure-seeded dialog still defaults diagnostics OFF"
        );
    }

    #[test]
    fn body_excludes_diagnostics_when_toggle_off() {
        let body = build_body("my description", false);
        assert_eq!(body, "my description");
        assert!(!body.contains("App version"));
        assert!(!body.contains("Arch"));
    }

    #[test]
    fn body_includes_diagnostics_only_when_toggle_on() {
        let body = build_body("my description", true);
        assert!(body.starts_with("my description"));
        assert!(body.contains("App version:"));
        assert!(body.contains("OS:"));
        assert!(body.contains("Arch:"));
    }

    #[test]
    fn preview_body_is_exactly_what_request_carries() {
        let mut st = IssueIntakeState::default();
        st.description = "a bug".into();
        st.include_diagnostics = true;
        let preview = st.preview_body();
        let req = st.request("o/r");
        assert_eq!(req.body, preview, "preview must equal the sent body");
    }

    #[test]
    fn request_targets_correct_template_and_label() {
        let mut st = IssueIntakeState::default();
        st.kind = IssueKind::Feature;
        st.description = "please add silent retry".into();
        let req = st.request("owner/repo");
        assert_eq!(req.template.as_deref(), Some("feature.yml"));
        assert_eq!(req.labels, vec!["enhancement".to_string()]);
        assert!(req.title.starts_with("feat: "));
        assert!(req.title.contains("please add silent retry"));
        assert_eq!(req.repo, "owner/repo");
    }

    #[test]
    fn title_tail_takes_first_nonempty_line_capped() {
        assert_eq!(
            title_tail_from("\n\n  hello world  \nsecond"),
            "hello world"
        );
        assert_eq!(title_tail_from(""), "");
        let long = "x".repeat(200);
        assert_eq!(title_tail_from(&long).chars().count(), 80);
    }

    #[test]
    fn no_persistent_id_in_any_built_url_or_body() {
        let mut st = IssueIntakeState::default();
        st.kind = IssueKind::Bug;
        st.description = "crash on open".into();
        st.include_diagnostics = true;
        let req = st.request("o/r");
        assert_no_persistent_id(&req.to_url());
        assert_no_persistent_id(&req.body);
        assert_no_persistent_id(&clipboard_fallback_body(&req));
        let mailto = mailto_url("a@b.test", &req.title, &req.body);
        assert_no_persistent_id(&mailto);
    }

    #[test]
    fn url_under_ceiling_uses_deep_link_decision() {
        let mut st = IssueIntakeState::default();
        st.description = "short".into();
        assert!(
            st.fits_url_length("o/r"),
            "a short report must fit the URL ceiling"
        );
    }

    #[test]
    fn url_over_ceiling_falls_back_to_clipboard() {
        let mut st = IssueIntakeState::default();
        st.description = "y".repeat(GITHUB_URL_LENGTH_THRESHOLD + 500);
        assert!(
            !st.fits_url_length("o/r"),
            "an over-length report must NOT fit the URL ceiling"
        );
        let req = st.request("o/r");
        // open_or_copy on an over-length request never opens the browser — it
        // yields a clipboard action with the fallback body.
        match open_or_copy(&req) {
            IntakeAction::CopyToClipboard(body) => {
                assert!(!body.is_empty(), "clipboard body must carry the report");
                assert_no_persistent_id(&body);
            }
            IntakeAction::Opened(_) => {
                panic!("an over-length URL must never take the deep-link path");
            }
        }
    }

    #[test]
    fn outcome_log_details_are_stable_and_non_identifying() {
        assert_eq!(IntakeOutcome::OpenedDeepLink.log_detail(), "deep-link");
        assert_eq!(IntakeOutcome::CopiedToClipboard.log_detail(), "clipboard");
        assert_eq!(IntakeOutcome::OpenedMailto.log_detail(), "mailto");
        assert_eq!(
            IntakeOutcome::Failed("clipboard unavailable: secret/path".into()).log_detail(),
            "failed"
        );
    }

    #[test]
    fn mailto_carries_subject_and_body_no_id() {
        let url = mailto_url("support@example.test", "bug: it broke", "details & more");
        assert!(url.starts_with("mailto:support@example.test?"));
        assert!(url.contains("subject="));
        assert!(url.contains("body="));
        assert_no_persistent_id(&url);
    }

    #[test]
    fn default_intake_coords_target_the_installer_repo() {
        assert_eq!(ISSUE_REPO, "46b-ETYKiAL/Itasha.Corp_F0RG3-W1R3");
        assert!(MAILTO_ALIAS.contains('@'));
    }
}

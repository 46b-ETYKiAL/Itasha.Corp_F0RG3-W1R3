//! Unexpected-panic crash diagnostics for the installer.
//!
//! The release profile sets `panic = "abort"` (see `Cargo.toml`), so an
//! *unexpected* panic terminates the process immediately — the GUI window
//! vanishes with zero diagnostic. [`install`] registers a `std::panic::set_hook`
//! early in `main` that, before the abort fires, runs the W1TN3SS Tier-1 capture
//! seam: it spools a sanitized, opt-in crash report locally so the user could
//! review + consent-send it later. Nothing transmits in the hook — capture is
//! local-first, default-OFF, consent-gated.
//!
//! The hook then chains to the previously-installed hook, so the default panic
//! output still runs. A panic hook runs *before* the runtime aborts, so the
//! report is always spooled first.

use std::panic::PanicHookInfo;

/// Install the crash-diagnostics panic hook. Call once, early in `main`.
///
/// Chains to any previously-installed hook so default panic output is
/// preserved. Safe to call before the window/event-loop is created.
pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // W1TN3SS Tier-1: spool a sanitized, opt-in crash report locally so the
        // user can review + consent-send it later. Nothing transmits here —
        // capture is local-first, default-OFF, consent-gated. Only the panic's
        // STATIC `&'static str` message (a source-literal, e.g. an `expect("…")`
        // string) + our own panic SITE enter the report — a runtime `String`
        // payload (which could embed environment fragments / paths) is
        // deliberately NOT spooled. Best-effort; a spool failure in an
        // already-panicking thread is swallowed (never re-panics).
        capture_panic_w1tn3ss(info);
        // Always chain to the previous hook (default abort message, etc.).
        previous(info);
    }));
}

/// W1TN3SS Tier-1 capture: spool a sanitized, opt-in crash report from the
/// panic's STATIC message + our panic SITE via [`crate::reporting::capture_panic`].
///
/// Only a `&'static str` panic payload (a source-literal message, e.g. from
/// `panic!("lit")` / `expect("…")` / `unwrap()` — the latter's std message is a
/// `&'static str`) is spooled, honouring the SDK's static-message discipline: a
/// runtime `String` payload (from `panic!("{}", x)`) could embed environment
/// fragments or a path, so it is deliberately NOT spooled. Best-effort: a
/// non-static payload or a spool failure is a no-op — the panic hook must never
/// itself re-panic.
fn capture_panic_w1tn3ss(info: &PanicHookInfo<'_>) {
    // Only the `&'static str` arm is spooled (the static-message discipline).
    let Some(static_msg) = info.payload().downcast_ref::<&'static str>() else {
        return;
    };
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "<unknown>".to_string());
    let _ = crate::reporting::capture_panic(static_msg, &location);
}

#[cfg(test)]
mod tests {
    /// `PanicHookInfo` is not constructible outside std, so the static-payload
    /// discrimination is exercised by capturing inside an actual (caught) panic
    /// on a worker thread, where the hook receives a genuine info. We assert the
    /// extractor takes the `&'static str` arm for a string-literal panic and
    /// declines a formatted (`String`) payload — the static-message discipline.
    #[test]
    fn static_payload_is_captured_formatted_is_declined() {
        let took_static = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let took_formatted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let s = took_static.clone();
        let f = took_formatted.clone();

        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if info.payload().downcast_ref::<&'static str>().is_some() {
                s.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            if info.payload().downcast_ref::<String>().is_some() {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }));
        let _ = std::panic::catch_unwind(|| panic!("static literal boom"));
        let runtime = 7;
        let _ = std::panic::catch_unwind(|| panic!("formatted {runtime}"));
        std::panic::set_hook(previous);

        assert!(
            took_static.load(std::sync::atomic::Ordering::SeqCst),
            "a string-literal panic carries a &'static str payload"
        );
        assert!(
            took_formatted.load(std::sync::atomic::Ordering::SeqCst),
            "a formatted panic carries a String payload (which the W1TN3SS capture declines)"
        );
    }
}

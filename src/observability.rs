//! Lightweight observability primitives shared across commands.
//!
//! The `tracing` crate is wired up as of S-1.03 — see `src/main.rs` for
//! subscriber initialization. This module provides shared helper utilities
//! (e.g. `log_parse_failure_once`) that CLI handlers use independently of
//! the HTTP-layer tracing events in `src/api/client.rs` and `src/api/auth.rs`.

use std::sync::atomic::{AtomicBool, Ordering};

/// Log a parse-failure once per `flag` per process, gated on `verbose`.
///
/// `flag` is typically a function-local `static AtomicBool`, one per
/// call-site, so each formatter fires at most one line per run. The
/// `site` argument is a short human label (e.g. `"changelog"`,
/// `"date"`) included in the message for disambiguation.
pub(crate) fn log_parse_failure_once(flag: &AtomicBool, site: &str, iso: &str, verbose: bool) {
    if verbose && !flag.swap(true, Ordering::Relaxed) {
        eprintln!("[verbose] {site} timestamp failed to parse: {iso}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbose_false_leaves_flag_untouched() {
        // Pins the short-circuit order: the `verbose` check must happen
        // BEFORE `flag.swap(...)`. Reversing them would silently burn the
        // flag on a non-verbose run and suppress the first verbose log
        // later in the same process.
        let flag = AtomicBool::new(false);
        log_parse_failure_once(&flag, "test", "not-a-date", false);
        assert!(
            !flag.load(Ordering::Relaxed),
            "verbose=false must not flip the gate flag"
        );
    }
}

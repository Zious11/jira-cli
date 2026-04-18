//! Lightweight observability primitives shared across commands.
//!
//! Intentionally tiny: the project has no tracing/log crate, and a
//! single `--verbose`-gated `eprintln!` is the established pattern
//! (see `src/api/client.rs` for HTTP-request logging). Expand to a
//! real tracing layer when there is cross-subsystem need.

use std::sync::atomic::{AtomicBool, Ordering};

/// Log a parse-failure once per `flag` per process, gated on `verbose`.
///
/// `flag` is typically a function-local `static AtomicBool`, one per
/// call-site, so each formatter fires at most one line per run. The
/// `site` argument is a short human label (e.g. `"changelog"`,
/// `"comment"`) included in the message for disambiguation.
pub(crate) fn log_parse_failure_once(flag: &AtomicBool, site: &str, iso: &str, verbose: bool) {
    if verbose && !flag.swap(true, Ordering::Relaxed) {
        eprintln!("[verbose] {site} timestamp failed to parse: {iso}");
    }
}

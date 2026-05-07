//! Red Gate integration tests for S-1.03: tracing crate + structured logging wire-up.
//!
//! # Approach: source-grep (Option A)
//!
//! All tests here perform static inspection of source files and `Cargo.toml`
//! using `std::fs::read_to_string`. No `tracing` types are imported into this
//! test file, so the test binary compiles today (pre-fix) without the tracing
//! dep being present. The assertions are written as postconditions that FAIL
//! before the implementation and PASS after it.
//!
//! # Red Gate expectations (pre-fix state)
//!
//! | Test                                           | Failure reason                                     |
//! |------------------------------------------------|----------------------------------------------------|
//! | test_s_1_03_cargo_toml_has_tracing_dep         | `tracing = ` absent from Cargo.toml                |
//! | test_s_1_03_cargo_toml_has_tracing_subscriber  | `tracing-subscriber` absent from Cargo.toml        |
//! | test_s_1_03_main_initializes_tracing_subscriber| `tracing_subscriber::` absent from main.rs         |
//! | test_s_1_03_main_uses_env_filter               | `EnvFilter` absent from main.rs                    |
//! | test_s_1_03_main_subscriber_not_in_lib         | (vacuous pass pre-fix; tightened post-fix)         |
//! | test_s_1_03_client_uses_tracing_debug          | `tracing::debug!` absent from client.rs            |
//! | test_s_1_03_client_no_verbose_eprintln         | `[verbose]` eprintln still present in client.rs    |
//! | test_s_1_03_auth_has_tracing_entry_points      | `tracing::` calls absent from auth.rs              |
//! | test_s_1_03_auth_no_secret_in_tracing_fields   | (vacuous pass pre-fix; structural post-fix guard)  |
//!
//! # AC-004 / SD-003 note
//!
//! SD-003 regression (AC-004) is verified by `tests/verbose_bodies.rs` which
//! was introduced by S-0.06. No duplicate tests are written here.

// ─── helpers ────────────────────────────────────────────────────────────────

/// Read a source file relative to the crate root (CARGO_MANIFEST_DIR).
fn read_src(relative_path: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let full_path = std::path::Path::new(manifest_dir).join(relative_path);
    std::fs::read_to_string(&full_path).unwrap_or_else(|err| {
        panic!(
            "Could not read {full_path:?}: {err}\n\
             (This likely means the file does not exist yet — expected pre-fix.)"
        )
    })
}

// ─── AC-001: Cargo.toml has pinned tracing deps ──────────────────────────────

/// AC-001 (NFR-O-A): `Cargo.toml` [dependencies] must list `tracing` with an
/// explicit version pin (not `*`).
///
/// Pre-fix FAILS: `tracing = ` is absent from Cargo.toml — the crate has no
/// tracing dependency at the pre-fix HEAD.
/// Post-fix PASSES: implementer adds `tracing = "0.1"` (or similar pin).
#[test]
fn test_s_1_03_cargo_toml_has_tracing_dep() {
    let cargo_toml = read_src("Cargo.toml");

    // Must contain a tracing dep line in [dependencies]. Accept both:
    //   tracing = "0.1"
    //   tracing = { version = "0.1", ... }
    let has_tracing = cargo_toml
        .lines()
        .any(|line| line.trim_start().starts_with("tracing ") && line.contains('='));

    assert!(
        has_tracing,
        "Cargo.toml [dependencies] must contain a `tracing = ...` entry with an explicit \
         version pin. Got Cargo.toml:\n{cargo_toml}"
    );

    // Must NOT use a wildcard version.
    let has_wildcard = cargo_toml.lines().any(|line| {
        let t = line.trim_start();
        t.starts_with("tracing ") && t.contains("= \"*\"")
    });
    assert!(
        !has_wildcard,
        "Cargo.toml `tracing` dep must not use a wildcard version `*`; \
         use an explicit semver pin like \"0.1\"."
    );
}

/// AC-001 (NFR-O-A): `Cargo.toml` [dependencies] must list `tracing-subscriber`
/// with an explicit version pin.
///
/// Pre-fix FAILS: `tracing-subscriber` is absent from Cargo.toml.
/// Post-fix PASSES: implementer adds `tracing-subscriber = { version = "0.3", features = ["env-filter"] }`.
#[test]
fn test_s_1_03_cargo_toml_has_tracing_subscriber_dep() {
    let cargo_toml = read_src("Cargo.toml");

    // Must contain a tracing-subscriber dep line. Accept:
    //   tracing-subscriber = "0.3"
    //   tracing-subscriber = { version = "0.3", features = [...] }
    let has_subscriber = cargo_toml
        .lines()
        .any(|line| line.trim_start().starts_with("tracing-subscriber") && line.contains('='));

    assert!(
        has_subscriber,
        "Cargo.toml [dependencies] must contain a `tracing-subscriber = ...` entry \
         with an explicit version pin (e.g. \"0.3\"). \
         Got Cargo.toml:\n{cargo_toml}"
    );

    // The env-filter feature must be enabled (AC-002 requires EnvFilter).
    let has_env_filter_feature = cargo_toml.contains("env-filter");
    assert!(
        has_env_filter_feature,
        "Cargo.toml `tracing-subscriber` dep must enable the `env-filter` feature \
         so EnvFilter is available. Add: features = [\"env-filter\"]"
    );
}

// ─── AC-002: main.rs initializes tracing-subscriber with EnvFilter ───────────

/// AC-002 (NFR-O-A): `src/main.rs` must initialize a `tracing_subscriber` by
/// calling a method from the `tracing_subscriber` crate.
///
/// Pre-fix FAILS: `tracing_subscriber::` does not appear in main.rs.
/// Post-fix PASSES: implementer adds `tracing_subscriber::fmt()...init()` (or
/// `try_init().ok()`) in main.rs.
#[test]
fn test_s_1_03_main_initializes_tracing_subscriber() {
    let main_rs = read_src("src/main.rs");

    let has_subscriber_call = main_rs.contains("tracing_subscriber::");
    assert!(
        has_subscriber_call,
        "src/main.rs must initialize tracing_subscriber (e.g. \
         `tracing_subscriber::fmt()...init()`). \
         `tracing_subscriber::` not found in main.rs."
    );
}

/// AC-002 (NFR-O-A): The subscriber initialization in `src/main.rs` must use
/// `EnvFilter` to set default level (WARN) and escalate on --verbose / --verbose-bodies.
///
/// Pre-fix FAILS: `EnvFilter` does not appear in main.rs.
/// Post-fix PASSES: implementer adds `.with_max_level(...)` or
/// `.with_env_filter(EnvFilter::...)`.
#[test]
fn test_s_1_03_main_uses_env_filter() {
    let main_rs = read_src("src/main.rs");

    // Accept both a direct EnvFilter usage and a with_max_level usage.
    // The story allows either approach; what matters is that the verbose flags
    // gate the log level rather than always emitting at a fixed level.
    let has_level_gate = main_rs.contains("EnvFilter")
        || main_rs.contains("with_max_level")
        || main_rs.contains("with_env_filter");

    assert!(
        has_level_gate,
        "src/main.rs must use EnvFilter or with_max_level to gate log level based on \
         --verbose / --verbose-bodies flags (AC-002). \
         Neither `EnvFilter`, `with_max_level`, nor `with_env_filter` found in main.rs."
    );
}

/// AC-006 guard (architecture rule): `src/lib.rs` must NOT initialize a
/// tracing subscriber. Subscriber init belongs only in `main.rs`.
///
/// Pre-fix PASSES vacuously (no subscriber anywhere).
/// Post-fix PASSES if implementer correctly placed init only in main.rs.
/// Post-fix FAILS if implementer accidentally called `.init()` in lib.rs,
/// which would cause double-init panics in test mode.
#[test]
fn test_s_1_03_lib_rs_does_not_init_subscriber() {
    let lib_rs = read_src("src/lib.rs");

    // Allow tracing re-exports or use statements, but not a subscriber .init() call.
    // The forbidden pattern is `tracing_subscriber::...init()` or `.init()` after
    // a subscriber builder chain.
    let has_subscriber_init = lib_rs.contains("tracing_subscriber::") && lib_rs.contains(".init()");

    assert!(
        !has_subscriber_init,
        "src/lib.rs must NOT initialize a tracing subscriber (only main.rs should). \
         Double-init panics in test mode. Found `tracing_subscriber::` and `.init()` \
         in lib.rs — move the init call to main.rs only."
    );
}

// ─── AC-003: client.rs uses tracing::debug!, not eprintln! for verbose lines ─

/// AC-003 (NFR-O-A): `src/api/client.rs` must use `tracing::debug!` (or
/// `debug!` via a use-import) for request/response header logging, replacing
/// the ad-hoc `eprintln!("[verbose] ...")` calls.
///
/// Pre-fix FAILS: `tracing::debug!` does not appear in client.rs — the file
/// uses only raw `eprintln!` for verbose output.
/// Post-fix PASSES: implementer replaces the eprintln! verbose sites with
/// tracing::debug! / tracing::trace! structured events.
#[test]
fn test_s_1_03_client_uses_tracing_debug() {
    let client_rs = read_src("src/api/client.rs");

    // Accept either fully-qualified `tracing::debug!` or macro imported as `debug!`
    // with a `use tracing` statement present.
    let has_tracing_debug = client_rs.contains("tracing::debug!")
        || client_rs.contains("tracing::trace!")
        || (client_rs.contains("use tracing") && client_rs.contains("debug!"));

    assert!(
        has_tracing_debug,
        "src/api/client.rs must use `tracing::debug!` (or imported `debug!` from tracing) \
         for request/response logging (AC-003). \
         Currently only eprintln! is used. The implementer must replace verbose eprintln! \
         sites with structured tracing events."
    );
}

/// AC-003 (NFR-O-A): The `[verbose]` prefixed `eprintln!` calls for method+URL
/// and rate-limit lines in `client.rs` must be replaced by tracing macros.
/// After replacement, the pattern `eprintln!("[verbose]` must not appear for
/// those lines.
///
/// NOTE: The `[jr] WARNING:` PII banner (for --verbose-bodies) is intentional
/// user-facing output, NOT a logging call — it stays as eprintln! or println!.
/// This test only checks that the HTTP-level verbose output (`[verbose] GET ...`,
/// `[verbose] Rate limited`) is replaced.
///
/// Pre-fix FAILS: client.rs has many `eprintln!("[verbose] %s %s"` lines for
/// method+URL and rate-limit output.
/// Post-fix PASSES: those specific sites are replaced with tracing calls.
#[test]
fn test_s_1_03_client_no_verbose_request_eprintln() {
    let client_rs = read_src("src/api/client.rs");

    // Count `eprintln!("[verbose]` occurrences that represent HTTP-level events
    // (method+URL, headers, rate-limit). The body suppression hint may remain
    // as an eprintln! if the implementer chooses; the story spec says to replace
    // the header/URL ones, not necessarily the suppression hint. We check that
    // the *request/rate-limit* verbose eprintln patterns are gone.
    //
    // Patterns that MUST be replaced:
    //   eprintln!("[verbose] {} {}", r.method(), r.url())   → tracing::debug!
    //   eprintln!("[verbose] Rate limited                   → tracing::debug!
    //
    // Pattern that MAY remain (user-visible hint):
    //   eprintln!("[verbose] body suppressed ...")           → MAY stay
    //   eprintln!("[jr] WARNING: ...")                       → MUST stay (PII banner)

    let request_eprintln_count = client_rs
        .lines()
        .filter(|line| {
            let t = line.trim();
            // Match the specific request-logging eprintln patterns
            (t.contains("eprintln!(\"[verbose]") || t.contains("eprintln!(\"[verbose]"))
                && (t.contains("method()") || t.contains("Rate limited"))
        })
        .count();

    assert_eq!(
        request_eprintln_count, 0,
        "src/api/client.rs must NOT use eprintln! for method+URL and rate-limit verbose \
         output (AC-003). Found {request_eprintln_count} occurrences of \
         `eprintln!(\"[verbose]` with method/rate-limit content — \
         replace these with `tracing::debug!` structured events."
    );
}

// ─── AC-005: auth.rs has tracing entries at OAuth function entry points ───────

/// AC-005 (NFR-O-A): `src/api/auth.rs` must emit `tracing::info!` or
/// `tracing::debug!` events at key OAuth flow entry points.
///
/// The story requires traces at:
///   - `oauth_login` entry (OAuth flow start)
///   - `exchange_code_for_token` call site (token exchange)
///   - `refresh_oauth_token` entry (token refresh)
///
/// Pre-fix FAILS: `tracing::` calls do not appear in auth.rs at all.
/// Post-fix PASSES: implementer adds structured trace events at these points.
#[test]
fn test_s_1_03_auth_has_tracing_entry_points() {
    let auth_rs = read_src("src/api/auth.rs");

    // Accept either fully-qualified or use-imported tracing calls.
    let has_tracing_call = auth_rs.contains("tracing::info!")
        || auth_rs.contains("tracing::debug!")
        || (auth_rs.contains("use tracing")
            && (auth_rs.contains("info!") || auth_rs.contains("debug!")));

    assert!(
        has_tracing_call,
        "src/api/auth.rs must emit tracing events at OAuth flow entry points \
         (AC-005, NFR-O-A). `tracing::info!` / `tracing::debug!` not found in auth.rs. \
         Add structured trace events at `oauth_login`, token exchange, and \
         `refresh_oauth_token` entry."
    );
}

/// AC-005 secret-safety guard: tracing events in `auth.rs` must NOT directly
/// reference secret variable values in their field lists.
///
/// Specifically, the tracing field arguments must not include `client_secret`,
/// `access_token` (as a value), or `refresh_token` (as a value). Profile name
/// and flow type are acceptable fields.
///
/// Pre-fix PASSES vacuously (no tracing:: calls in auth.rs yet).
/// Post-fix PASSES if implementer follows the architecture rule:
///   "Do not log client_secret, access_token, or refresh_token values."
/// Post-fix FAILS if implementer accidentally includes secret values in fields.
///
/// Static check: we look for the anti-pattern of `tracing::*!(...client_secret...`
/// or `tracing::*!(...access_token = %access_token...` in the same macro call.
#[test]
fn test_s_1_03_auth_no_client_secret_in_tracing_fields() {
    let auth_rs = read_src("src/api/auth.rs");

    // We scan each line that contains a tracing macro call and check that
    // it does not pass secret variables as field values.
    // The anti-patterns (logging the VALUE of a secret):
    //   tracing::debug!(client_secret = %client_secret, ...)
    //   tracing::info!(access_token = %access_token, ...)
    //   tracing::debug!(refresh_token = %refresh_token, ...)
    //
    // Acceptable (logging the profile name, not the secret):
    //   tracing::debug!(profile = %profile, "refreshing token")
    //
    // Strategy: look for tracing macro invocations that contain both a secret
    // variable name AND a `%` or `?` formatter on the same line (direct value log).

    let secret_leak_patterns = [
        // Field assignments using Display/Debug formatters on secret vars
        "client_secret = %",
        "client_secret = ?",
        "access_token = %access_token",
        "access_token = ?access_token",
        "refresh_token = %refresh_token",
        "refresh_token = ?refresh_token",
        // Simple bare variable in tracing macro after a comma (logged as-is)
        // e.g. tracing::debug!("...", client_secret)
        // These are harder to catch statically; we focus on the field-assign form.
    ];

    for pattern in &secret_leak_patterns {
        // Only flag if the line also contains a tracing macro.
        let leaking_lines: Vec<&str> = auth_rs
            .lines()
            .filter(|line| {
                let contains_tracing = line.contains("tracing::debug!")
                    || line.contains("tracing::info!")
                    || line.contains("tracing::trace!")
                    || line.contains("tracing::warn!");
                contains_tracing && line.contains(pattern)
            })
            .collect();

        assert!(
            leaking_lines.is_empty(),
            "src/api/auth.rs has a tracing event that may log a secret value \
             (pattern: `{pattern}`). Secret values (client_secret, access_token, \
             refresh_token) must NEVER appear in tracing field lists. \
             Offending lines:\n{leaking_lines:#?}"
        );
    }
}

// ─── AC-006: subscriber init is in main only (observability.rs does not init) ─

/// AC-006 guard: `src/observability.rs` must NOT initialize a tracing subscriber.
/// It may extend helpers, but the `.init()` / `.try_init()` calls belong only in
/// `main.rs`.
///
/// Pre-fix PASSES vacuously (observability.rs has no tracing at all).
/// Post-fix PASSES if implementer leaves subscriber init in main.rs only.
#[test]
fn test_s_1_03_observability_rs_does_not_init_subscriber() {
    let obs_rs = read_src("src/observability.rs");

    // The forbidden pattern: tracing_subscriber with .init() being called
    // (not just referenced or used as a type).
    let calls_init = obs_rs.contains("tracing_subscriber::") && obs_rs.contains(".init()");

    assert!(
        !calls_init,
        "src/observability.rs must NOT call tracing_subscriber::...init() — \
         subscriber initialization belongs exclusively in src/main.rs to prevent \
         double-init panics when the lib is used in test mode (AC-006)."
    );
}

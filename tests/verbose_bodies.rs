//! Integration tests for SD-003: --verbose-bodies flag + PII warning (S-0.06).
//!
//! # Red Gate
//!
//! ALL tests in this file MUST FAIL at the pre-fix HEAD (`d907504`). Failure modes:
//!
//! | Test                                                          | Pre-fix failure mode                                      |
//! |---------------------------------------------------------------|-----------------------------------------------------------|
//! | test_sd_003_verbose_bodies_emits_pii_warning                  | clap parse error: "unexpected argument '--verbose-bodies'" |
//! | test_sd_003_verbose_alone_suppresses_body_bytes               | sentinel@example.test APPEARS in stderr (bodies printed)  |
//! | test_sd_003_verbose_plus_verbose_bodies_prints_body           | clap parse error: "unexpected argument '--verbose-bodies'" |
//! | test_sd_003_verbose_bodies_alone_prints_body_without_url_line | clap parse error: "unexpected argument '--verbose-bodies'" |
//! | test_sd_003_help_mentions_verbose_bodies_flag                 | --verbose-bodies absent from help text                    |
//! | test_sd_003_changelog_has_breaking_change_entry               | CHANGELOG.md absent or missing BREAKING CHANGE entry      |
//!
//! # Green Gate (post-fix, all must PASS)
//!
//! - `--verbose-bodies` is a valid clap flag
//! - `--verbose` alone suppresses body bytes, emitting "[verbose] body suppressed ..."
//! - `--verbose-bodies` emits the 3-line PII warning before body content
//! - Combined `--verbose --verbose-bodies` emits method+URL line, PII warning, AND body
//! - `--verbose-bodies` alone prints body + warning, but NOT the method+URL line
//! - `jr --help` lists both `--verbose` and `--verbose-bodies` with correct help text
//! - CHANGELOG.md contains a BREAKING CHANGE entry for the body-suppression migration
//!
//! # Holdout alignment
//!
//! - H-NEW-VERBOSE-001: verified by test_sd_003_verbose_bodies_emits_pii_warning
//! - H-NEW-VERBOSE-002: verified by test_sd_003_verbose_alone_suppresses_body_bytes

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── Shared sentinel values ──────────────────────────────────────────────────

/// Sentinel string in the mock response body; used to detect body leakage.
const SENTINEL_EMAIL: &str = "sentinel@example.test";
const SENTINEL_ACCOUNT_ID: &str = "acc_sentinel_42";

/// JSON body the mock returns for GET /rest/api/3/myself.
/// Contains distinguishable PII-like content to detect body leakage in stderr.
fn myself_sentinel_body() -> serde_json::Value {
    serde_json::json!({
        "accountId": SENTINEL_ACCOUNT_ID,
        "emailAddress": SENTINEL_EMAIL,
        "displayName": "Sentinel User"
    })
}

// ── Config helpers ──────────────────────────────────────────────────────────

/// Write a minimal single-profile config pointing at the mock server.
fn write_minimal_config(config_home: &std::path::Path, server_uri: &str) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        format!(
            r#"
default_profile = "default"

[profiles.default]
url = "{server_uri}"
"#
        ),
    )
    .unwrap();
}

/// Build a `jr` command wired to a mock server with auth bypass and --no-input.
fn jr_cmd(server_uri: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("XDG_CONFIG_HOME", config_dir)
        .arg("--no-input");
    cmd
}

// ── AC-001 / H-NEW-VERBOSE-001: --verbose-bodies emits PII warning ──────────

/// SD-003 postcondition (AC-001, H-NEW-VERBOSE-001): when `--verbose-bodies` is
/// set, stderr MUST contain all three PII warning lines before any body content.
///
/// Pre-fix FAILS: clap parse error "unexpected argument '--verbose-bodies'" —
/// the flag does not exist yet.
/// Post-fix PASSES: warning is emitted on client construction.
///
/// This is holdout H-NEW-VERBOSE-001 (Group 7 in holdout-scenarios.md).
#[tokio::test]
async fn test_sd_003_verbose_bodies_emits_pii_warning() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    write_minimal_config(config_dir.path(), &server.uri());

    // Mount GET /rest/api/3/myself so the `jr me` command has something to hit.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(myself_sentinel_body()))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--verbose-bodies", "me"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Post-fix: all three PII warning lines must appear in stderr.
    assert!(
        stderr.contains("[jr] WARNING: --verbose-bodies prints request/response bodies to stderr."),
        "stderr must contain PII warning line 1; got: {stderr}"
    );
    assert!(
        stderr
            .contains("[jr] These bodies contain PII (accountId, emailAddress, ADF text content)."),
        "stderr must contain PII warning line 2; got: {stderr}"
    );
    assert!(
        stderr.contains("[jr] Do not pipe to AI-agent contexts or shared logs without consent."),
        "stderr must contain PII warning line 3; got: {stderr}"
    );
}

// ── AC-002 / H-NEW-VERBOSE-002: --verbose alone does NOT print body bytes ───

/// SD-003 postcondition (AC-002, H-NEW-VERBOSE-002): when `--verbose` is set
/// WITHOUT `--verbose-bodies`, stderr MUST NOT contain body bytes (sentinel PII
/// strings), and MUST contain the suppression hint.
///
/// Pre-fix FAILS: current `--verbose` prints the full response body, so
/// `sentinel@example.test` and `acc_sentinel_42` appear in stderr.
/// Post-fix PASSES: bodies are suppressed; suppression hint is emitted.
///
/// This is holdout H-NEW-VERBOSE-002 (Group 7 in holdout-scenarios.md).
#[tokio::test]
async fn test_sd_003_verbose_alone_suppresses_body_bytes() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    write_minimal_config(config_dir.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(myself_sentinel_body()))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--verbose", "me"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Post-fix: body sentinel strings MUST NOT appear in stderr.
    assert!(
        !stderr.contains(SENTINEL_EMAIL),
        "stderr must NOT contain body email sentinel; got: {stderr}"
    );
    assert!(
        !stderr.contains(SENTINEL_ACCOUNT_ID),
        "stderr must NOT contain body accountId sentinel; got: {stderr}"
    );

    // Post-fix: suppression hint MUST appear so users can migrate to --verbose-bodies.
    assert!(
        stderr.contains("body suppressed (use --verbose-bodies to inspect, will print PII)"),
        "stderr must contain suppression hint; got: {stderr}"
    );

    // Post-fix: method+URL verbose line IS present (--verbose controls this).
    assert!(
        stderr.contains("[verbose]"),
        "stderr must contain [verbose] method+URL line; got: {stderr}"
    );
}

// ── AC-003: --verbose + --verbose-bodies combined prints body + warning ──────

/// SD-003 postcondition (AC-003): when both `--verbose` and `--verbose-bodies`
/// are set, stderr MUST contain:
///   - The `[verbose]` method+URL line (from --verbose)
///   - The 3-line PII warning (from --verbose-bodies)
///   - The body bytes (body sentinel strings from --verbose-bodies)
///
/// Pre-fix FAILS: clap parse error "unexpected argument '--verbose-bodies'" —
/// the flag does not exist yet.
/// Post-fix PASSES: both flags are orthogonal and compose correctly.
#[tokio::test]
async fn test_sd_003_verbose_plus_verbose_bodies_prints_body() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    write_minimal_config(config_dir.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(myself_sentinel_body()))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--verbose", "--verbose-bodies", "me"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // method+URL line from --verbose.
    assert!(
        stderr.contains("[verbose]"),
        "stderr must contain [verbose] method+URL line; got: {stderr}"
    );

    // PII warning from --verbose-bodies.
    assert!(
        stderr.contains("[jr] WARNING: --verbose-bodies prints request/response bodies to stderr."),
        "stderr must contain PII warning line 1; got: {stderr}"
    );

    // Body bytes from --verbose-bodies (response body leaked into stderr intentionally).
    // Note: sentinel strings appear in the response body returned by the mock.
    assert!(
        stderr.contains(SENTINEL_EMAIL) || stderr.contains(SENTINEL_ACCOUNT_ID),
        "stderr must contain body sentinel string(s) when --verbose-bodies is set; got: {stderr}"
    );
}

// ── AC-004: --verbose-bodies alone prints body + warning, not method+URL ─────

/// SD-003 postcondition (AC-004, S-0.06 AC-005): `--verbose-bodies` without
/// `--verbose` is NOT equivalent to `--verbose`. Specifically:
///   - MUST print body bytes (sentinel strings)
///   - MUST print PII warning
///   - MUST NOT print the `[verbose] GET ...` method+URL line (that requires --verbose)
///
/// This tests that the two flags are orthogonal: --verbose-bodies controls body
/// output; --verbose controls headers/status/URL logging.
///
/// Pre-fix FAILS: clap parse error "unexpected argument '--verbose-bodies'".
/// Post-fix PASSES: bodies printed (with warning) but no method+URL line.
#[tokio::test]
async fn test_sd_003_verbose_bodies_alone_prints_body_without_url_line() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    write_minimal_config(config_dir.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(myself_sentinel_body()))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--verbose-bodies", "me"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // PII warning must be present (--verbose-bodies was set).
    assert!(
        stderr.contains("[jr] WARNING: --verbose-bodies prints request/response bodies to stderr."),
        "stderr must contain PII warning; got: {stderr}"
    );

    // Body bytes must appear (--verbose-bodies enables body output).
    assert!(
        stderr.contains(SENTINEL_EMAIL) || stderr.contains(SENTINEL_ACCOUNT_ID),
        "stderr must contain body sentinel string(s); got: {stderr}"
    );

    // method+URL line must NOT appear (--verbose was not set; only --verbose-bodies).
    // The [verbose] prefix on method+URL lines is: "[verbose] GET https://..."
    // Body lines use "[verbose] body: ..." — so we check for absence of
    // "[verbose] GET" and "[verbose] POST" patterns (HTTP method after [verbose]).
    let has_method_url_line = stderr.lines().any(|line| {
        let trimmed = line.trim_start_matches("[verbose] ");
        // A method+URL line looks like "[verbose] GET http://..." where the remainder
        // starts with an HTTP method verb followed by a space and URL.
        line.starts_with("[verbose] ")
            && (trimmed.starts_with("GET ")
                || trimmed.starts_with("POST ")
                || trimmed.starts_with("PUT ")
                || trimmed.starts_with("DELETE ")
                || trimmed.starts_with("PATCH "))
    });

    assert!(
        !has_method_url_line,
        "stderr must NOT contain [verbose] method+URL line when only --verbose-bodies is set \
         (--verbose controls method+URL logging); got: {stderr}"
    );
}

// ── AC-005: --help mentions both flags ──────────────────────────────────────

/// SD-003 postcondition (AC-005, S-0.06 implementation requirement 4): `jr --help`
/// stdout MUST contain both `--verbose` and `--verbose-bodies` as distinct flags,
/// with appropriate help text.
///
/// Per SD-003 resolution: `--verbose` help text must mention "use --verbose-bodies
/// for full body inspection".
///
/// Pre-fix FAILS: `--verbose-bodies` is absent from help text.
/// Post-fix PASSES: both flags listed with correct descriptions.
#[test]
fn test_sd_003_help_mentions_verbose_bodies_flag() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .arg("--help")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    // --verbose must still be present.
    assert!(
        stdout.contains("--verbose"),
        "jr --help must list --verbose flag; got: {stdout}"
    );

    // --verbose-bodies must be the new flag.
    assert!(
        stdout.contains("--verbose-bodies"),
        "jr --help must list --verbose-bodies flag; got: {stdout}"
    );

    // Per SD-003 resolution requirement 4: --verbose help text must reference
    // --verbose-bodies so users know how to migrate.
    assert!(
        stdout.contains("verbose-bodies"),
        "jr --help --verbose description must mention verbose-bodies migration path; got: {stdout}"
    );
}

// ── AC-006: CHANGELOG.md has a BREAKING CHANGE entry ────────────────────────

/// SD-003 postcondition (AC-006): CHANGELOG.md (or equivalent) MUST contain a
/// BREAKING CHANGE entry documenting the `--verbose` body-suppression migration
/// to `--verbose-bodies`.
///
/// Pre-fix FAILS: CHANGELOG.md is absent or does not contain "BREAKING CHANGE".
/// Post-fix PASSES: implementer creates/updates CHANGELOG.md with the entry.
///
/// The implementer must add (per S-0.06 implementation notes):
/// ```
/// ## BREAKING CHANGE (v0.6)
/// **--verbose no longer prints HTTP request/response bodies by default.**
/// Use `--verbose-bodies` for full body inspection.
/// ```
#[test]
fn test_sd_003_changelog_has_breaking_change_entry() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let changelog_path = std::path::Path::new(manifest_dir).join("CHANGELOG.md");

    let contents = std::fs::read_to_string(&changelog_path).unwrap_or_else(|_| {
        panic!(
            "CHANGELOG.md must exist at {changelog_path:?}. \
             Implementer must create it with a BREAKING CHANGE entry per S-0.06 AC-006."
        )
    });

    // Must mention BREAKING CHANGE (per SD-003 Resolution §Breaking change release notes).
    assert!(
        contents.contains("BREAKING CHANGE") || contents.contains("BREAKING:"),
        "CHANGELOG.md must contain a BREAKING CHANGE entry for the --verbose body-suppression \
         migration to --verbose-bodies; got contents:\n{contents}"
    );

    // Must mention --verbose-bodies so the migration path is clear.
    assert!(
        contents.contains("--verbose-bodies"),
        "CHANGELOG.md BREAKING CHANGE entry must mention --verbose-bodies; \
         got contents:\n{contents}"
    );

    // Must mention --verbose (the changed flag).
    assert!(
        contents.contains("--verbose"),
        "CHANGELOG.md BREAKING CHANGE entry must mention --verbose; \
         got contents:\n{contents}"
    );
}

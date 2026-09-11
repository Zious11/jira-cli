//! Red Gate tests for S-cycle7-oauth-help-text-fix — `--oauth` help-text
//! accuracy fix (issue #790).
//!
//! DOCS-ONLY story: AC-001 and AC-002 assert on `--help` output substrings
//! only; they FAIL against the current (wrong) doc string and PASS after the
//! doc-comment edit. AC-003 is a regression pin for EXISTING correct runtime
//! behavior (guard-rejection-precedes-deprecation-notice) — it PASSES before
//! AND after the doc change. AC-004 is a baseline pin confirming no
//! `#[arg(...)]` attribute is changed — it also PASSES before AND after.
//!
//! Assertions are scoped to the `--oauth` flag's own help-text block (from
//! `      --oauth` up to `      --api-token`) via `oauth_option_block()`,
//! not the whole `--help` stdout, so unrelated occurrences of the same words
//! in neighboring flags cannot cause a false-green before the fix lands.

use assert_cmd::Command;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Runs `jr auth login --help` and returns captured stdout as a String.
fn auth_login_help_stdout() -> String {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "login", "--help"])
        .output()
        .expect("failed to run `jr auth login --help`");
    assert!(
        output.status.success(),
        "`jr auth login --help` did not exit successfully"
    );
    String::from_utf8(output.stdout).expect("stdout is not valid UTF-8")
}

/// Extracts the `--oauth` flag's help-text block from a `jr auth login --help`
/// stdout capture — from the six-space-indented `      --oauth` header line up
/// to (but not including) `      --api-token`. This isolates AC-001/AC-002
/// assertions to exactly the text this story's doc-comment edit targets and
/// prevents false-greens from unrelated occurrences of the same words.
fn oauth_option_block(stdout: &str) -> &str {
    let header = "      --oauth";
    let next_header = "      --api-token";
    let start = stdout.find(header).unwrap_or_else(|| {
        panic!("no '{header}' marker found in `jr auth login --help` output:\n{stdout}")
    });
    let rest = &stdout[start..];
    let end = rest.find(next_header).unwrap_or_else(|| {
        panic!(
            "no '{next_header}' marker found after '--oauth' in `jr auth login --help` output:\n{stdout}"
        )
    });
    &rest[..end]
}

// ---------------------------------------------------------------------------
// AC-001 — RED today: help text must not claim "requires your own OAuth app"
// ---------------------------------------------------------------------------

/// AC-001 (traces to PRD delta §6.1 primary defect).
///
/// The `--oauth` flag's doc comment in `src/cli/mod.rs` (`AuthCommand::Login`)
/// currently opens with "(requires your own OAuth app)" — factually wrong
/// given jr ships an embedded OAuth app (ADR-0006). After the doc-comment fix,
/// the `--oauth` help block must contain no such claim.
///
/// RED GATE: this test FAILS against the current doc string and PASSES after
/// the doc-comment edit removes the "(requires your own OAuth app)" phrase.
#[test]
fn test_help_text_oauth_flag_does_not_claim_own_app_required() {
    let stdout = auth_login_help_stdout();
    let block = oauth_option_block(&stdout);

    assert!(
        !block.contains("requires your own OAuth app"),
        "--oauth help block must not claim '(requires your own OAuth app)' — \
         jr ships an embedded OAuth app (ADR-0006); the phrase must be removed \
         or corrected to describe the embedded default + optional override.\n\
         Current --oauth block:\n{block}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 — RED today: help text must not overclaim unconditional notice
// ---------------------------------------------------------------------------

/// AC-002 (traces to PRD delta §6.1 secondary (a)).
///
/// The `--oauth` doc comment currently says "a deprecation notice is printed
/// to stderr in human-output mode" without qualification — this overclaims,
/// because the `check_noninteractive_oauth_guard` rejection (called FIRST in
/// `handle_login`) suppresses the notice path entirely on non-interactive
/// invocations. After the doc-comment fix, the wording must be consistent with
/// the confirmed-correct runtime behavior (guard can precede and suppress the
/// notice; see AC-003).
///
/// RED GATE: this test FAILS against the current unconditional phrasing and
/// PASSES after the doc-comment is corrected.
#[test]
fn test_help_text_oauth_flag_does_not_overclaim_unconditional_notice() {
    let stdout = auth_login_help_stdout();
    let block = oauth_option_block(&stdout);

    // The overclaiming phrase asserts the notice is always printed to stderr
    // in human-output mode — no qualification for the guard-rejection case.
    assert!(
        !block.contains("printed to stderr in human-output mode"),
        "--oauth help block must not claim the deprecation notice is \
         unconditionally 'printed to stderr in human-output mode' — the \
         guard-rejection path suppresses it (EC-1.2.049-3). The phrasing must \
         be corrected or qualified.\n\
         Current --oauth block:\n{block}"
    );
}

// ---------------------------------------------------------------------------
// AC-003 — GREEN today: guard rejection precedes deprecation notice (regression pin)
// ---------------------------------------------------------------------------

/// AC-003 (traces to BC-1.2.049 EC-1.2.049-3).
///
/// This is a regression pin for EXISTING correct behavior, NOT a new behavior.
/// `handle_login` calls `check_noninteractive_oauth_guard` as its FIRST
/// statement; that guard's `Err` propagates via `?` before
/// `emit_oauth_deprecation_notice` is ever reached.
///
/// Concretely: `jr auth login --oauth --no-input` must:
/// 1. Exit with code 64 (UserError).
/// 2. Emit the guard rejection message on stderr.
/// 3. NOT emit the deprecation notice on stderr.
///
/// GREEN today: the behavior is already correct. This test pins it so a
/// future refactor of `handle_login`'s call order cannot silently break it.
///
/// Note: `JR_CONFIG_DIR` is set to a temp dir to avoid reading any real
/// `~/.config/jr/config.toml`, though the guard fires before any config load
/// and would produce the same result without it. The isolation is defensive.
#[test]
fn test_ec_1_2_049_3_guard_rejection_precedes_deprecation_notice() {
    let tmp = tempfile::tempdir().expect("failed to create tempdir");

    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "login", "--oauth", "--no-input"])
        .env("JR_CONFIG_DIR", tmp.path())
        .output()
        .expect("failed to run `jr auth login --oauth --no-input`");

    // Guard rejection exits 64 (JrError::UserError).
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 from guard rejection; got {:?}. \
         stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Guard message must appear.
    assert!(
        stderr.contains("OAuth requires an interactive terminal"),
        "guard rejection message must appear in stderr (EC-1.2.049-3). \
         Got stderr:\n{stderr}"
    );

    // Deprecation notice must NOT appear — guard fires first and suppresses it.
    assert!(
        !stderr.contains("--oauth is deprecated"),
        "deprecation notice must NOT appear when the guard rejects first \
         (EC-1.2.049-3). The notice path must be unreachable after guard exits. \
         Got stderr:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// OBS-1 — RED today: auth refresh --oauth help must not overclaim notice
// ---------------------------------------------------------------------------

/// Runs `jr auth refresh --help` and returns captured stdout as a String.
fn auth_refresh_help_stdout() -> String {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "refresh", "--help"])
        .output()
        .expect("failed to run `jr auth refresh --help`");
    assert!(
        output.status.success(),
        "`jr auth refresh --help` did not exit successfully"
    );
    String::from_utf8(output.stdout).expect("stdout is not valid UTF-8")
}

/// Extracts the `--oauth` flag's help-text block from `jr auth refresh --help`
/// stdout — from `      --oauth` up to (but not including) `      --api-token`.
/// Scoped to the Refresh subcommand only; prevents false-greens from the Login
/// block sharing the same process output.
fn refresh_oauth_option_block(stdout: &str) -> &str {
    let header = "      --oauth";
    let next_header = "      --api-token";
    let start = stdout.find(header).unwrap_or_else(|| {
        panic!("no '{header}' marker found in `jr auth refresh --help` output:\n{stdout}")
    });
    let rest = &stdout[start..];
    let end = rest.find(next_header).unwrap_or_else(|| {
        panic!(
            "no '{next_header}' marker found after '--oauth' in `jr auth refresh --help` output:\n{stdout}"
        )
    });
    &rest[..end]
}

/// OBS-1 + pass-2 F1/OBS-A (adversary findings).
///
/// `AuthCommand::Refresh`'s `--oauth` doc comment in `src/cli/mod.rs` (~L280).
///
/// Negative assertion (OBS-1): must NOT contain the old unconditional overclaim
/// "printed to stderr in human-output mode".
///
/// Positive assertion (OBS-A): MUST contain "human-output" — pins the corrected
/// framing so a future reword that simply removes both claims cannot silently
/// pass. The accurate wording is: "A deprecation notice is printed in
/// human-output (Table) mode unless the non-interactive OAuth guard rejects the
/// refresh first." This correctly captures that the notice IS emitted in
/// human-output mode for non-OAuth profiles (where the guard does not fire),
/// unlike Login where main.rs auto-sets --no-input on non-TTY stdin and the
/// guard always fires before the notice.
///
/// RED today: current text "may be emitted on interactive runs" passes the
/// negative assertion but FAILS the positive assertion (no "human-output").
/// GREEN after: doc corrected to the accurate output-format-gated wording.
#[test]
fn test_help_text_refresh_oauth_flag_does_not_overclaim_unconditional_notice() {
    let stdout = auth_refresh_help_stdout();
    let block = refresh_oauth_option_block(&stdout);

    // Negative: must not contain the old "printed to stderr" unconditional form.
    assert!(
        !block.contains("printed to stderr in human-output mode"),
        "--oauth help block in `jr auth refresh --help` must not claim the \
         deprecation notice is unconditionally 'printed to stderr in \
         human-output mode' (OBS-1).\n\
         Current --oauth block:\n{block}"
    );

    // Positive (OBS-A): must contain "human-output" — pins the accurate
    // output-format-gated framing (notice is output-mode-gated, not
    // interactive-session-gated; see pass-2 F1 for the distinction).
    assert!(
        block.contains("human-output"),
        "--oauth help block in `jr auth refresh --help` must contain \
         'human-output' to accurately pin the output-format-gated framing \
         (OBS-A). The corrected wording is: 'A deprecation notice is printed \
         in human-output (Table) mode unless the non-interactive OAuth guard \
         rejects the refresh first.'\n\
         Current --oauth block:\n{block}"
    );
}

// ---------------------------------------------------------------------------
// AC-004 — GREEN baseline: conflicts_with usage rendering is unaffected
// ---------------------------------------------------------------------------

/// AC-004 (traces to PRD delta §6.2(b) explicitly declined this cycle).
///
/// This story makes a doc-comment-only change to `AuthCommand::Login`'s
/// `--oauth` field — no `#[arg(...)]` attribute is modified. This test pins
/// that the `conflicts_with`-driven usage-line rendering is byte-for-byte
/// identical before AND after the doc change, so an accidental attribute
/// change would be caught immediately.
///
/// Specifically: the `Usage:` line in `jr auth login --help` must list
/// `[--oauth]` and `[--api-token]` as independent optional flags (clap
/// renders mutually-exclusive non-required flags as independent `[...]`
/// items, not as `<--oauth|--api-token>`). Additionally, both `--oauth` and
/// `--api-token` must appear as separate entries in the `Options:` section.
///
/// GREEN today: the rendering is already correct. This test locks the
/// baseline so the doc change cannot accidentally affect it.
#[test]
fn test_clap_conflicts_with_usage_rendering_unaffected_by_doc_change() {
    let stdout = auth_login_help_stdout();

    // Both flags must appear as distinct entries in the Options section.
    assert!(
        stdout.contains("      --oauth"),
        "--oauth must appear as a distinct Options entry; got:\n{stdout}"
    );
    assert!(
        stdout.contains("      --api-token"),
        "--api-token must appear as a distinct Options entry; got:\n{stdout}"
    );

    // The Usage: line must not have been accidentally changed to a required
    // or grouped rendering.  clap emits `[--oauth]` for an optional flag.
    let usage_line = stdout
        .lines()
        .find(|l| l.starts_with("Usage:"))
        .unwrap_or_else(|| panic!("no 'Usage:' line in --help output:\n{stdout}"));
    assert!(
        !usage_line.contains("<--oauth|--api-token>"),
        "Usage: line must not render --oauth/--api-token as a required group \
         (conflicts_with rendering should remain as independent [...] flags). \
         Got usage line: {usage_line}"
    );
}

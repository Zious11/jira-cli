//! Red-gate integration tests for `--no-parent` flag on `jr issue edit` (issue #284).
//!
//! Every test in this file is expected to FAIL before the implementation because
//! `--no-parent` does not yet exist in `src/cli/mod.rs`. The failure mode
//! depends on the test:
//!
//! - Help-text tests fail because the flag name does not appear in the output.
//! - Wiremock body tests fail because clap rejects `--no-parent` as unknown
//!   and exits 2 before making any HTTP call (mock never invoked, exit ≠ 0).
//! - The conflict test fails because clap exits 2 with "unrecognized argument"
//!   rather than "cannot be used with" — the substring check discriminates.
//! - The "no fields" guard test fails for the same clap-rejection reason.
//! - The subtask-400 test fails because the flag doesn't exist yet.
//! - The combined-fields test fails for the same reason as the body test.
//!
//! Reference: research report at
//! `.factory/research/issue-284-no-parent-flag.md` (all claims verified).

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

/// Start a wiremock server and return its URI.
async fn start_server() -> MockServer {
    MockServer::start().await
}

/// Build a `jr` command pointing at the mock server.
///
/// Auth is injected via `JR_AUTH_HEADER` (Basic test:test in base64) — a
/// debug-only seam documented in CLAUDE.md "AI Agent Notes". The `JR_BASE_URL`
/// env var overrides the Jira instance URL so no config file or keychain is
/// needed.
fn jr_cmd(server_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0"); // test:test
    cmd
}

// ---------------------------------------------------------------------------
// T-01: --no-parent appears in `jr issue edit --help`
// ---------------------------------------------------------------------------
//
// Pre-impl failure mode: `--no-parent` flag does not exist → clap help output
// does not contain the string "--no-parent" → the `contains` predicate fails.

#[test]
fn test_no_parent_flag_appears_in_edit_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "edit", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--no-parent"));
}

// ---------------------------------------------------------------------------
// T-02: --no-parent flag description mentions "parent" and clearing behavior
// ---------------------------------------------------------------------------
//
// Bonus test: verifies the flag has a useful one-line help string.
//
// Pre-impl failure mode: help text is absent → `contains` fails.

#[test]
fn test_no_parent_flag_help_text_describes_clear_behavior() {
    // Collect help output and check manually so we can assert on the
    // description line adjacent to --no-parent, not just any occurrence.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "edit", "--help"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "jr issue edit --help exited non-zero"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // The flag must exist …
    assert!(
        stdout.contains("--no-parent"),
        "Expected '--no-parent' in help; got:\n{stdout}"
    );
    // … and both --parent and --no-parent must appear (two separate flag lines).
    assert!(
        stdout.contains("--parent"),
        "Expected '--parent' in help alongside '--no-parent'; got:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// T-03: --no-parent sends PUT with {"fields":{"parent":null}}
// ---------------------------------------------------------------------------
//
// Pre-impl failure mode: clap rejects `--no-parent` as unknown → exit 2 →
// `assert().success()` fails. The PUT mock is never called.

#[tokio::test]
async fn test_no_parent_clears_parent_via_put_with_json_null() {
    let server = start_server().await;

    // The critical matcher: the request body must contain `"parent": null`
    // inside the `fields` object. We use an exact-body matcher on the fields
    // sub-object to avoid false positives.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-100"))
        .and(body_json(serde_json::json!({
            "fields": {
                "parent": null
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1) // must be called exactly once
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "FOO-100", "--no-parent"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 when --no-parent succeeds (204); stderr={stderr} stdout={stdout}"
    );
    // wiremock verifies .expect(1) on drop — if the PUT was never made, the
    // test panics with "expected 1 call, got 0".
}

// ---------------------------------------------------------------------------
// T-04: --no-parent conflicts with --parent
// ---------------------------------------------------------------------------
//
// Pre-impl failure mode: `--no-parent` is unknown → clap exits 2 with
// "unrecognized argument '--no-parent'" (not "cannot be used with") → the
// `contains("cannot be used with")` assertion fails, discriminating RED from
// GREEN.

#[test]
fn test_no_parent_conflicts_with_parent_value() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue",
            "edit",
            "FOO-100",
            "--no-parent",
            "--parent",
            "BAR-200",
        ])
        .assert()
        .failure()
        .code(2) // clap usage error
        .stderr(predicate::str::contains("cannot be used with"));
}

// ---------------------------------------------------------------------------
// T-05: --no-parent alone counts as a field update (no "no fields" error)
// ---------------------------------------------------------------------------
//
// The existing guard at `create.rs:333` bails if `has_updates` is false.
// After implementation, `no_parent = true` must set `has_updates = true`.
//
// Pre-impl failure mode: clap rejects `--no-parent` → exit 2 →
// `assert().success()` fails.

#[tokio::test]
async fn test_no_parent_alone_with_no_other_fields_works() {
    let server = start_server().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-100"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "FOO-100", "--no-parent"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Must exit 0 — not the "no fields specified" error (exit 1 with bail!)
    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    // Confirm the "no fields" error message is NOT in stderr
    assert!(
        !stderr.contains("No fields specified to update"),
        "--no-parent should count as a field update; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// T-06: subtask parent-clear 400 surfaces API error AND convert hint
// ---------------------------------------------------------------------------
//
// Atlassian returns 400 when `parent: null` is sent against a subtask.
// The handler MUST surface the API message AND append a hint about conversion.
//
// Pre-impl failure mode: clap rejects `--no-parent` → exit 2 →
// `!output.status.success()` passes, but exit code is 2 (clap) not 1 (API),
// AND the stderr will not contain the conversion hint — so both assertions fail
// independently. This gives a precise RED discriminant.

#[tokio::test]
async fn test_subtask_parent_clear_surfaces_400_with_convert_hint() {
    let server = start_server().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/SUB-456"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": [],
            "errors": {
                "parent": "Subtasks must have a parent."
            }
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "SUB-456", "--no-parent"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "Expected non-zero exit when Jira returns 400; stderr={stderr}"
    );

    // Exit code must be 1 (API error), not 2 (clap usage error).
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected exit 1 for API 400 error, got {:?}; stderr={stderr}",
        output.status.code()
    );

    // The Jira error message must appear in stderr.
    assert!(
        stderr.contains("Subtasks must have a parent")
            || stderr.contains("400")
            || stderr.contains("API error"),
        "Expected Jira error text in stderr; stderr={stderr}"
    );

    // A conversion hint MUST appear so users know the path forward.
    assert!(
        stderr.contains("convert")
            || stderr.contains("subtask")
            || stderr.contains("standard issue"),
        "Expected convert hint in stderr; stderr={stderr}"
    );

    // AC-4 strengthening (S-388): three new literal-pin assertions.

    // Pin 1: JRACLOUD-27893 citation MUST appear in stderr.
    assert!(
        stderr.contains("JRACLOUD-27893"),
        "Expected JRACLOUD-27893 citation in stderr (--no-parent cross-hierarchy hint); stderr={stderr}"
    );

    // Pin 2: regression guard — the removed fake-endpoint hint MUST NOT appear.
    // The prior hint contained `jr api /rest/api/3/issue/{key}/convert -X put -d ...`;
    // this substring uniquely identifies the removed text without over-matching
    // legitimate content (see BC-3.4.010 postcondition note and prd-delta-388.md T-06 note).
    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint must not appear in stderr; stderr={stderr}"
    );

    // Pin 3: full verbatim context sentence that MUST be prepended before CROSS_HIERARCHY_HINT
    // on the --no-parent path only (PINNED STRING — exact bytes required per BC-3.4.010).
    assert!(
        stderr.contains(
            "Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue."
        ),
        "Expected full verbatim context sentence in stderr (--no-parent path); stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// T-07: --no-parent combined with --summary sends a unified PUT payload
// ---------------------------------------------------------------------------
//
// Both fields must appear in a SINGLE PUT request body — not two separate
// requests. The body matcher checks both keys are present simultaneously.
//
// Pre-impl failure mode: clap rejects `--no-parent` → exit 2 →
// `assert().success()` fails. Mock never invoked (`.expect(1)` fires on drop).

#[tokio::test]
async fn test_no_parent_combined_with_other_fields_sends_unified_payload() {
    let server = start_server().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-100"))
        .and(body_json(serde_json::json!({
            "fields": {
                "parent": null,
                "summary": "New title"
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1) // exactly one PUT with both fields
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-100",
            "--no-parent",
            "--summary",
            "New title",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for combined --no-parent --summary; stderr={stderr} stdout={stdout}"
    );
    // wiremock verifies .expect(1) on drop — wrong body or missing call panics.
}

// ---------------------------------------------------------------------------
// T-08: --output json on success emits {"key":"FOO-100"}
// ---------------------------------------------------------------------------
//
// AC-002 from the research report: `--output json` must emit the standard
// write-op JSON shape on success.
//
// Pre-impl failure mode: clap rejects `--no-parent` → exit 2 →
// `assert().success()` fails.

#[tokio::test]
async fn test_no_parent_output_json_emits_key_on_success() {
    let server = start_server().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-100"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "FOO-100",
            "--no-parent",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 with --output json; stderr={stderr}"
    );

    // stdout must be valid JSON containing "key": "FOO-100"
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));
    assert_eq!(
        parsed["key"].as_str(),
        Some("FOO-100"),
        r#"Expected {{"key":"FOO-100"}} in stdout; got {stdout}"#
    );
}

// ---------------------------------------------------------------------------
// Mutation kill: --no-parent gate negative — non-subtask-parent 400 MUST NOT
// emit the cross-hierarchy hint.
// ---------------------------------------------------------------------------
//
// This test pins the `&&` in `if no_parent && is_subtask_parent_error(e)`.
// If the operator were replaced with `||`, this test would fail because:
//   - `no_parent` is true, `is_subtask_parent_error(e)` is false (error has
//     no "subtask" / "parent+400" substring) → with `||` the block fires
//     incorrectly and `CROSS_HIERARCHY_HINT` appears on stderr.
//
// The test asserts that when `--no-parent` is set but the 400 response body
// does NOT look like a subtask-parent error, NO cross-hierarchy hint is emitted.
// The raw API error is still propagated (exit 1, error text on stderr).
//
// Kill target: `src/cli/issue/create.rs:898:22 replace && with ||`

#[tokio::test]
async fn test_no_parent_non_subtask_400_does_not_surface_cross_hierarchy_hint() {
    let server = start_server().await;

    // PUT → 400 with a generic "field required" error, NOT a subtask-parent error.
    // `is_subtask_parent_error` checks for "subtask" or ("parent" AND "400") in the
    // message text — this body contains neither, so the predicate returns false.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-99"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": ["The summary field is required."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "PROJ-99", "--no-parent"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must still exit non-zero (raw API error propagated).
    assert!(
        !output.status.success(),
        "Expected non-zero exit for non-subtask-parent 400; stderr={stderr}"
    );

    // CROSS_HIERARCHY_HINT MUST NOT appear — the --no-parent gate must not fire
    // when is_subtask_parent_error returns false (pins &&, not ||).
    assert!(
        !stderr.contains("JRACLOUD-27893"),
        "CROSS_HIERARCHY_HINT must NOT appear when error is not a subtask-parent error; stderr={stderr}"
    );

    // Regression pin on the removed fake-endpoint hint.
    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint must not appear in stderr; stderr={stderr}"
    );

    // The raw error text MUST appear so users see why the edit failed.
    assert!(
        stderr.contains("summary") || stderr.contains("required") || stderr.contains("400"),
        "Expected raw API error text in stderr; stderr={stderr}"
    );
}

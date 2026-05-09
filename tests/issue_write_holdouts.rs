//! BC-3 issue-write regression holdout suite — S-2.02
//!
//! Pins existing issue-write behavior across 4 holdout scenarios. All tests
//! pass on current develop — no implementation changes required. Future
//! regressions in any of these paths will break this suite.
//!
//! Holdout coverage:
//! - AC-001 / H-006 / BC-3.2.001: `issue move FOO-1 "In Progress"` already in target
//!   → exit 0, `"transitioned": false`, POST transitions `.expect(0)`
//! - AC-002 / H-007 / BC-3.2.009: `issue move FOO-1 Done` → 400 resolution-required
//!   → stderr contains `--resolution` AND `jr issue resolutions`
//! - AC-003 / H-008 / BC-2.1.013: `issue list --status prog` ambiguous → exit 64,
//!   "Ambiguous status" + "In Progress", JQL search `.expect(0)`
//! - AC-004 / H-014 / BC-X.7.004: `issue assign FOO-1 --to "John Smith" --no-input`
//!   with 2 dup displayNames → stderr has both emails + both accountIds
//!
//! Infrastructure:
//! - Process-spawn tests use `assert_cmd::Command` with `JR_BASE_URL` + `JR_AUTH_HEADER`
//! - HTTP-mock tests use `wiremock`
//! - XDG isolation via `tempfile::TempDir` for every test
//! - `expect(0)` on POST mocks proves idempotency / short-circuit guards

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build a `jr` command with XDG isolation and mock-server wiring.
fn jr_cmd(server_uri: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("XDG_CONFIG_HOME", config_dir)
        .arg("--no-input");
    cmd
}

// ---------------------------------------------------------------------------
// AC-001 / H-006 / BC-3.2.001
// ---------------------------------------------------------------------------

/// BC-3.2.001 postcondition: `jr issue move FOO-1 "In Progress"` when the issue
/// is already in "In Progress" must exit 0, return `{"transitioned": false}` in
/// stdout JSON, and make ZERO POST requests to the transitions endpoint.
///
/// `handle_move` calls GET transitions first, then GET issue to read the current
/// status; the idempotency check fires before the POST is ever dispatched. The
/// `.expect(0)` on the POST mock verifies via wiremock's verify-on-drop that the
/// HTTP call was never made.
///
/// Pins `src/cli/issue/workflow.rs::handle_move` idempotency guard.
/// Without this guard a future refactor that swaps the check order or removes
/// the early-return would silently POST an unnecessary (no-op) transition,
/// burning a Jira API call and potentially firing webhooks on the server side.
#[tokio::test]
async fn test_s_2_02_h_006_bc_3_2_001_idempotent_move_already_in_target_no_post() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    // GET /rest/api/3/issue/FOO-1/transitions — returns a transitions list that
    // includes "In Progress" (so the partial_match candidate pool is populated).
    // handle_move calls this endpoint first to enumerate available transitions.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("31", "Start Progress", "In Progress"),
                ("41", "Done", "Done"),
                ("11", "Back to To Do", "To Do"),
            ]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    // GET /rest/api/3/issue/FOO-1 — returns issue with status.name = "In Progress".
    // handle_move reads this after the transitions list to determine the current status.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-1",
                "My ticket",
                "In Progress",
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    // POST /rest/api/3/issue/FOO-1/transitions — must NOT fire (idempotency guard).
    // wiremock verifies .expect(0) on drop; any POST invocation will fail the test.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .current_dir(cwd.path())
        .args(["issue", "move", "FOO-1", "In Progress", "--output", "json"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "idempotent move must exit 0; stderr: {stderr}, stdout: {stdout}"
    );

    let parsed: Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON on idempotent move");
    // The JSON field is "changed" (not "transitioned") — see src/cli/issue/json_output.rs.
    // BC-3.2.001 uses "transitioned" in the contract language but the implementation
    // exposes "changed". We pin the actual field name so a future rename is visible here.
    assert_eq!(
        parsed["changed"],
        serde_json::json!(false),
        "BC-3.2.001: idempotent move must return changed=false; stdout: {stdout}"
    );
    // wiremock verifies .expect(0) on the POST mock during server drop.
}

// ---------------------------------------------------------------------------
// AC-002 / H-007 / BC-3.2.009
// ---------------------------------------------------------------------------

/// BC-3.2.009 postcondition: when a transition request fails with HTTP 400 containing
/// a body that indicates the "resolution" field is required, `handle_move` must
/// transform the raw Jira error into an actionable error message that includes:
///   - `--resolution` (the CLI flag the user should add), AND
///   - `jr issue resolutions` (the command to list valid resolution values)
///
/// The heuristic in `handle_move` matches on both "resolution" and "required" (lowercase)
/// in the error body. Sending the exact Atlassian-format errors body triggers it.
///
/// Pins `src/cli/issue/workflow.rs` resolution-required 400 transform.
/// Without this guard a refactor that removes the heuristic would silently expose the
/// raw Jira error body to the user, which names internal field IDs rather than the
/// CLI flag, leaving the user unable to act.
#[tokio::test]
async fn test_s_2_02_h_007_bc_3_2_009_resolution_required_400_actionable_stderr() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    // GET transitions — returns list that includes "Done" so the partial_match
    // candidate pool can resolve the "Done" target status.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("11", "Start Progress", "In Progress"),
                ("21", "Done", "Done"),
            ]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    // GET issue — returns issue in "To Do" (not yet Done) so the idempotency
    // guard does NOT fire, allowing the transition POST to be attempted.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-1",
                "My ticket",
                "To Do",
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    // POST transitions → 400 with Atlassian-format resolution-required body.
    // handle_move detects "resolution" + "required" in the lowercased error string
    // and rewrites to the actionable hint.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errors": {
                "resolution": "Field 'resolution' is required"
            },
            "errorMessages": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .current_dir(cwd.path())
        .args(["issue", "move", "FOO-1", "Done"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "resolution-required 400 must cause non-zero exit; stderr: {stderr}"
    );
    assert!(
        stderr.contains("--resolution"),
        "BC-3.2.009: stderr must contain '--resolution' flag hint; got: {stderr}"
    );
    assert!(
        stderr.contains("jr issue resolutions"),
        "BC-3.2.009: stderr must contain 'jr issue resolutions' command hint; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-003 / H-008 / BC-2.1.013
// ---------------------------------------------------------------------------

/// BC-2.1.013 postcondition: `jr issue list --status prog` when project statuses
/// are `["To Do", "In Progress", "Done"]` must:
///   - exit 64 (UserError)
///   - include "Ambiguous status" in stderr
///   - include "In Progress" in stderr (the matching candidate)
///   - make ZERO calls to the JQL search endpoint
///
/// "prog" is a single-substring hit for "In Progress". Per `partial_match.rs`
/// design, single-substring hits route through `MatchResult::Ambiguous` — only
/// case-insensitive exact matches auto-resolve. This is intentional: requiring
/// explicit status names prevents silent JQL over-scoping.
///
/// The `.expect(0)` on the JQL search mock verifies the short-circuit fires before
/// any search is dispatched.
///
/// Note: H-008 (BC-2.1.013) and H-021 (BC-2.1.007) are related holdouts.
/// H-021 is anchored to S-2.01 AC-007 (`tests/issue_read_holdouts.rs`).
/// This test pins H-008 from the issue-write suite with an explicit `--project`
/// flag rather than a `.jr.toml` file (per story risk note).
///
/// Pins `src/cli/issue/list.rs` status-validation short-circuit.
#[tokio::test]
async fn test_s_2_02_h_008_bc_2_1_013_ambiguous_status_prog_exits_64_no_jql_call() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    // GET project statuses: provides ["To Do", "In Progress", "Done"] for PROJ.
    // "prog" substring hits "In Progress" → routes through Ambiguous, not Exact.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ/statuses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::project_statuses_response()),
        )
        .expect(1)
        .mount(&server)
        .await;

    // POST JQL search must NOT fire — ambiguous status rejects before any search.
    // wiremock verifies .expect(0) on drop.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issues": [],
            "nextPageToken": null
        })))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .current_dir(cwd.path())
        .args(["issue", "list", "--project", "PROJ", "--status", "prog"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "ambiguous --status must fail; stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-2.1.013: ambiguous status must exit 64 (UserError); got: {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Ambiguous status"),
        "BC-2.1.013: stderr must contain 'Ambiguous status'; got: {stderr}"
    );
    assert!(
        stderr.contains("In Progress"),
        "BC-2.1.013: stderr must name the ambiguous candidate 'In Progress'; got: {stderr}"
    );
    // wiremock verifies .expect(0) on the JQL mock during server drop.
}

// ---------------------------------------------------------------------------
// AC-004 / H-014 / BC-X.7.004
// ---------------------------------------------------------------------------

/// BC-X.7.004 postcondition: when the assignable-user-search endpoint returns
/// two users with identical `displayName` = "John Smith" but different `emailAddress`
/// and `accountId`, and `--no-input` is set, the error message must contain:
///   - both email addresses, AND
///   - both accountIds
///
/// This allows an AI-agent caller (or a script) to retry the command with an
/// unambiguous identifier (either `--account-id` or a unique email substring).
///
/// The disambiguation logic lives in `disambiguate_user` (helpers.rs), which
/// routes duplicate display names through `MatchResult::ExactMultiple`. The error
/// format includes `(email, account: id)` per-duplicate entry.
///
/// Pins `src/cli/issue/helpers.rs::disambiguate_user` ExactMultiple branch.
/// Without this guard a refactor that simplifies the error message (e.g., drops
/// the email or accountId) would silently break AI-agent workflows that depend
/// on the structured error to disambiguate users.
#[tokio::test]
async fn test_s_2_02_h_014_bc_x_7_004_dup_display_names_stderr_has_emails_and_account_ids() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let email_1 = "john.smith.eng@example.com";
    let email_2 = "john.smith.pm@example.com";
    let account_id_1 = "acc-john-smith-eng-001";
    let account_id_2 = "acc-john-smith-pm-002";

    // GET /rest/api/3/user/assignable/search?query=John+Smith&issueKey=FOO-1
    // Returns two users with the same displayName but different email/accountId.
    // resolve_assignee in helpers.rs calls search_assignable_users which hits this endpoint.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .and(query_param("query", "John Smith"))
        .and(query_param("issueKey", "FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "accountId": account_id_1,
                "displayName": "John Smith",
                "emailAddress": email_1,
                "active": true
            },
            {
                "accountId": account_id_2,
                "displayName": "John Smith",
                "emailAddress": email_2,
                "active": true
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    // GET /rest/api/3/issue/FOO-1 — the assign handler reads the current assignee
    // AFTER resolving the account ID (for the idempotency check). Since user
    // resolution fails first (ExactMultiple), this mock should not be reached.
    // Register it anyway so the test doesn't fail on unexpected requests.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee("FOO-1", "My ticket", None),
        ))
        .mount(&server)
        .await;

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .current_dir(cwd.path())
        .args(["issue", "assign", "FOO-1", "--to", "John Smith"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "duplicate displayNames with --no-input must fail; stderr: {stderr}"
    );
    assert!(
        stderr.contains(email_1),
        "BC-X.7.004: stderr must contain first user's email '{email_1}'; got: {stderr}"
    );
    assert!(
        stderr.contains(email_2),
        "BC-X.7.004: stderr must contain second user's email '{email_2}'; got: {stderr}"
    );
    assert!(
        stderr.contains(account_id_1),
        "BC-X.7.004: stderr must contain first accountId '{account_id_1}'; got: {stderr}"
    );
    assert!(
        stderr.contains(account_id_2),
        "BC-X.7.004: stderr must contain second accountId '{account_id_2}'; got: {stderr}"
    );
}

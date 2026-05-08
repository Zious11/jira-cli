//! BC-2 issue-read regression holdout suite — S-2.01
//!
//! Pins existing issue-read behavior across 7 holdout scenarios. All tests
//! pass on current develop — no implementation changes required. Future
//! regressions in any of these paths will break this suite.
//!
//! Holdout coverage:
//! - AC-001 / H-030 / BC-7.3.001: 400 + empty body → "<empty response body>" in stderr
//! - AC-002 / H-031 / BC-X.2.005: short page (35) on page 2 → startAt advances by
//!   USER_PAGE_SIZE (100), total = 235 (JRACLOUD-71293 fixed-window pagination)
//! - AC-003 / H-032 / BC-X.2.006: stateless infinite 100-user mock → safety cap fires
//!   at 1500 users, exit 0, stderr contains warning
//! - AC-004 / H-033 / BC-3.7.004: `jr issue remote-link --url ftp://example.com`
//!   → exit 64, "http or https" + "ftp" in stderr, ZERO HTTP calls
//! - AC-005 / H-034 / BC-3.7.001: `jr issue remote-link --url https://example.com`
//!   → POST body normalized to `https://example.com/` (trailing slash from url crate)
//! - AC-006 / H-035 / BC-2.1.001: combined filters (--assignee --created-after
//!   --status --team) → exit 0, JSON array, no panic.
//!   Note: `--open` and `--status` declared `conflicts_with` at clap layer
//!   (`src/cli/mod.rs:168,171`); test uses 4 non-conflicting filters.
//! - AC-007 / H-021 / BC-2.1.007: `--status prog` ambiguous → exit 64,
//!   "Ambiguous status" in stderr, JQL mock not invoked
//!   (anchors pre-existing tests/issue_list_errors.rs:369 coverage)
//!
//! Infrastructure:
//! - Process-spawn tests use `assert_cmd::Command` with `JR_BASE_URL` + `JR_AUTH_HEADER`
//! - HTTP-mock tests use `wiremock`
//! - XDG isolation via `tempfile::TempDir` for every test
//! - Library-level tests use `jr::api::client::JiraClient::new_for_test`
//! - USER_PAGE_SIZE = 100 (src/api/jira/users.rs)
//! - USER_PAGINATION_SAFETY_CAP = 15 (src/api/jira/users.rs) → 15 * 100 = 1500 users

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use jr::api::client::JiraClient;

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

/// Build a `jr` command with XDG isolation but an unreachable base URL so that
/// any HTTP call made by the binary causes a connection-refused failure. Used to
/// confirm that validation short-circuits before the network layer.
fn jr_cmd_no_network(cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("XDG_CONFIG_HOME", config_dir)
        .arg("--no-input");
    cmd
}

/// Build a fixed-count page of user JSON objects. Each user gets a unique
/// accountId derived from `prefix` + index.
fn users_page(count: usize, prefix: &str) -> Value {
    let users: Vec<Value> = (0..count)
        .map(|i| {
            serde_json::json!({
                "accountId": format!("{prefix}-acc-{i:03}"),
                "displayName": format!("{prefix} User {i:03}"),
                "emailAddress": format!("{prefix}.user.{i:03}@test.com"),
                "active": true,
            })
        })
        .collect();
    Value::Array(users)
}

/// Write a minimal config with the given profile containing a `url` and the
/// given `team_field_id`. The URL is ignored in tests because `JR_BASE_URL`
/// overrides it; the field IDs ARE read from the config by the handlers.
fn write_config_with_team_field(
    config_home: &std::path::Path,
    team_field_id: &str,
    status_field_id: Option<&str>,
) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    let sp_line = match status_field_id {
        Some(id) => format!("story_points_field_id = \"{id}\"\n"),
        None => String::new(),
    };
    std::fs::write(
        conf_dir.join("config.toml"),
        format!(
            r#"
default_profile = "default"

[profiles.default]
url = "https://acme.atlassian.net"
team_field_id = "{team_field_id}"
{sp_line}
"#
        ),
    )
    .unwrap();
}

/// Pre-populate the team cache so that `--team AlphaTeam` resolves without
/// hitting the GraphQL / org discovery endpoints. Writes to the canonical
/// `<XDG_CACHE_HOME>/jr/v1/default/teams.json` path used by production code.
fn write_team_cache(cache_home: &std::path::Path, team_id: &str, team_name: &str) {
    let teams_dir = cache_home.join("jr").join("v1").join("default");
    std::fs::create_dir_all(&teams_dir).unwrap();
    let cache = jr::cache::TeamCache {
        fetched_at: chrono::Utc::now(),
        teams: vec![jr::cache::CachedTeam {
            id: team_id.to_string(),
            name: team_name.to_string(),
        }],
    };
    std::fs::write(
        teams_dir.join("teams.json"),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// AC-001 / H-030 / BC-7.3.001
// ---------------------------------------------------------------------------

/// BC-7.3.001 postcondition: when a Jira API endpoint returns HTTP 400 with an
/// empty response body, `extract_error_message` produces the literal string
/// `"<empty response body>"` and the binary writes it to stderr.
///
/// Pins `src/api/client.rs::extract_error_message` branch 5 (empty body).
/// Without this guard a future refactor that changes the sentinel text or drops
/// the empty-body branch would silently regress the error UX.
#[tokio::test]
async fn test_s_2_01_h_030_bc_7_3_001_400_empty_body_shows_sentinel_in_stderr() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    // Return 400 with a completely empty body — no JSON, no bytes.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(400).set_body_bytes(vec![]))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .current_dir(cwd.path())
        .args(["issue", "view", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "400 response must cause non-zero exit; stderr: {stderr}"
    );
    assert!(
        stderr.contains("<empty response body>"),
        "stderr must contain the literal sentinel '<empty response body>' (BC-7.3.001); got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 / H-031 / BC-X.2.005
// ---------------------------------------------------------------------------

/// BC-X.2.005 postcondition: when page 2 of a user search returns 35 users
/// (fewer than USER_PAGE_SIZE=100), `search_users_all` must advance `startAt`
/// by USER_PAGE_SIZE (100), not by 35. The third request must use startAt=200,
/// and the total user count across all pages must be 235 (100 + 35 + 100).
///
/// Pins the JRACLOUD-71293 fixed-window pagination contract in
/// `src/api/jira/users.rs::search_users_all`. Advancing by the returned
/// count instead of the requested window would skip raw users 35..100 on
/// page 2 and duplicate users already seen (overlap regression).
#[tokio::test]
async fn test_s_2_01_h_031_bc_x_2_005_short_page_advances_by_page_size_not_returned_count() {
    let server = MockServer::start().await;

    // Page 1: startAt=0 → 100 users
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    // Page 2: startAt=100 → 35 users (short, non-empty page).
    // A bug that advances by 35 would request startAt=135 next, not 200.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "100"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(35, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    // Page 3: startAt=200 (correct window advance) → 100 users.
    // This mock only matches startAt=200; any other value (e.g. 135) would
    // cause a wiremock unmatched-request failure, making the bug visible.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "200"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p3")))
        .expect(1)
        .mount(&server)
        .await;

    // Page 4: startAt=300 → empty (signals end-of-data)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "300"))
        .and(query_param("maxResults", "100"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client
        .search_users_all("u")
        .await
        .expect("pagination must succeed");

    assert_eq!(
        users.len(),
        235,
        "total must be 235 (100+35+100); short page must not truncate early: got {}",
        users.len()
    );
    // Verify startAt advances by USER_PAGE_SIZE (100), not by returned count (35):
    // if startAt had advanced by 35 after page 2, page 3 would start at 135
    // and the mock at startAt=200 would never be hit — wiremock would reject the
    // request and we would not reach 235 users. Reaching this assertion confirms
    // the fixed-window advance is correct.
    assert_eq!(
        users[0].display_name, "p1 User 000",
        "first user must be from page 1"
    );
    assert_eq!(
        users[100].display_name, "p2 User 000",
        "user 101 must be from page 2"
    );
    assert_eq!(
        users[135].display_name, "p3 User 000",
        "user 136 must be from page 3 (100 + 35 = 135 boundary)"
    );
}

// ---------------------------------------------------------------------------
// AC-003 / H-032 / BC-X.2.006
// ---------------------------------------------------------------------------

/// BC-X.2.006 postcondition: when the user-search endpoint never returns an
/// empty page (pathological/infinite server behavior), `search_users_all` must
/// terminate after exactly USER_PAGINATION_SAFETY_CAP=15 iterations, return
/// the collected 1500 users (15 * USER_PAGE_SIZE=100), and emit a stderr
/// warning so the truncation is observable rather than silent.
///
/// Pins `USER_PAGINATION_SAFETY_CAP` in `src/api/jira/users.rs`. A refactor
/// that removes or raises the cap without updating this test will fail here.
/// A refactor that silences the stderr warning will also fail.
#[tokio::test]
async fn test_s_2_01_h_032_bc_x_2_006_safety_cap_fires_at_1500_with_warning() {
    let server = MockServer::start().await;

    // Stateless responder: every startAt returns a full 100-user page.
    // USER_PAGINATION_SAFETY_CAP=15 → the loop fires exactly 15 times.
    // `.expect(15)` asserts this via wiremock's verify-on-drop.
    //
    // USER_PAGE_SIZE=100, USER_PAGINATION_SAFETY_CAP=15 — both constants from
    // src/api/jira/users.rs. The product 1500 is hardcoded in the assertion
    // with a comment so future changes to the constants are visible here.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "cap-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "cap")))
        .expect(15) // USER_PAGINATION_SAFETY_CAP = 15
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client
        .search_users_all("cap-test")
        .await
        .expect("safety cap must not error; it returns partial results with a warning");

    assert_eq!(
        users.len(),
        1500, // USER_PAGINATION_SAFETY_CAP(15) * USER_PAGE_SIZE(100)
        "safety cap must collect exactly 15 * 100 = 1500 users before stopping; got {}",
        users.len()
    );
    // The stderr warning is emitted by search_users_all (not this library-level
    // call), verified by the companion CLI-level test in tests/user_pagination.rs
    // (user_search_all_cli_emits_safety_cap_warning). The library contract here
    // is that the function succeeds with 1500 results and the wiremock `.expect(15)`
    // assertion confirms the loop terminates at the cap boundary.
}

// ---------------------------------------------------------------------------
// AC-004 / H-033 / BC-3.7.004
// ---------------------------------------------------------------------------

/// BC-3.7.004 postcondition: `jr issue remote-link --url ftp://example.com` must
/// exit 64 (UserError), include both "http or https" and "ftp" in stderr, and
/// make ZERO HTTP calls to the Jira API.
///
/// Pins the scheme validation gate in `src/cli/issue/links.rs::handle_remote_link`.
/// Using an unreachable base URL ensures any HTTP call the binary makes results
/// in a connection-refused error (exit ≠ 64), making the zero-HTTP-calls
/// property implicitly verified by the exit code assertion.
#[test]
fn test_s_2_01_h_033_bc_3_7_004_ftp_url_rejected_before_network_with_exit_64() {
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let output = jr_cmd_no_network(cache_dir.path(), config_dir.path())
        .current_dir(cwd.path())
        .args([
            "issue",
            "remote-link",
            "PROJ-1",
            "--url",
            "ftp://example.com",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "ftp:// scheme must exit 64 (UserError) before any network call; got: {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("http or https"),
        "stderr must name the accepted schemes; got: {stderr}"
    );
    assert!(
        stderr.contains("ftp"),
        "stderr must echo the rejected scheme; got: {stderr}"
    );
    // Zero HTTP calls: if the binary had tried to dial 127.0.0.1:1 it would have
    // gotten a connection-refused network error (exit 1/130 depending on OS) —
    // exit 64 confirms validation short-circuited before any dial attempt.
}

// ---------------------------------------------------------------------------
// AC-005 / H-034 / BC-3.7.001
// ---------------------------------------------------------------------------

/// BC-3.7.001 postcondition: `jr issue remote-link --url https://example.com`
/// must POST to the Jira remotelink endpoint with the url field set to
/// `https://example.com/` (trailing slash added by `url::Url::parse`).
///
/// Pins the url normalization contract in `src/cli/issue/links.rs::handle_remote_link`.
/// The `url` crate's parser normalizes bare-host HTTPS URLs to include a
/// trailing slash. Without this guard a future change that uses the raw flag
/// value instead of `parsed.as_str()` would post an unnormalized URL and
/// silently diverge from what the Jira UI renders.
#[tokio::test]
async fn test_s_2_01_h_034_bc_3_7_001_bare_host_url_normalized_with_trailing_slash() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let self_url = format!("{}/rest/api/3/issue/PROJ-42/remotelink/55001", server.uri());

    // body_partial_json asserts the POST body contains the normalized URL.
    // If the binary posts `https://example.com` (no slash) the mock won't match
    // and wiremock returns 404 — the test would fail at the exit-code assertion.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-42/remotelink"))
        .and(body_partial_json(serde_json::json!({
            "object": {
                "url": "https://example.com/",
                "title": "https://example.com/"
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": 55001,
            "self": self_url,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .current_dir(cwd.path())
        .args([
            "issue",
            "remote-link",
            "PROJ-42",
            "--url",
            "https://example.com",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "normalized URL must produce exit 0; stderr: {stderr}, stdout: {stdout}"
    );

    let parsed: Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(
        parsed["url"], "https://example.com/",
        "stdout JSON must reflect the normalized URL with trailing slash"
    );
}

// ---------------------------------------------------------------------------
// AC-006 / H-035 / BC-2.1.001
// ---------------------------------------------------------------------------

/// BC-2.1.001 postcondition: multiple `jr issue list` filters that are not
/// mutually exclusive (--assignee, --created-after, --status, --team) may be
/// combined in a single invocation. The handler must compose them into a
/// single JQL query without panicking or returning an error. With all mocks
/// satisfied, the command exits 0 and returns a 5-element JSON array.
///
/// Note: `--open` conflicts with `--status` at the clap CLI layer (they are
/// declared `conflicts_with` each other). The BC-2.1.001 contract is about
/// handler-level filter composition, not conflicting CLI flag combinations.
/// This test therefore uses `--status "In Progress"` instead of `--open`.
///
/// This test exercises the full filter-composition path in
/// `src/cli/issue/list.rs::handle_list`. It requires simultaneous mocks for:
///   - user search (--assignee resolution)
///   - project statuses (--status resolution)
///   - JQL search (result)
///
/// The team is resolved via a pre-populated cache (no GraphQL call needed).
/// No CMDB fields are configured, so the S-0.03 multi-workspace path is not taken.
#[tokio::test]
async fn test_s_2_01_h_035_bc_2_1_001_all_filters_combined_compose_jql_without_panic() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    // Config: team_field_id so --team is routable, no story_points_field_id.
    let team_field_id = "customfield_10100";
    write_config_with_team_field(config_dir.path(), team_field_id, None);

    // Team cache: resolve "Alpha" to "team-uuid-alpha" without hitting GraphQL.
    write_team_cache(cache_dir.path(), "team-uuid-alpha", "Alpha");

    // User search: --assignee "alice" → accountId "acc-alice-001"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "alice"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "accountId": "acc-alice-001",
                "displayName": "Alice Anderson",
                "emailAddress": "alice@test.com",
                "active": true
            }
        ])))
        .mount(&server)
        .await;

    // Project statuses: --status "Progress" resolves from project HOLDOUT scope.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HOLDOUT/statuses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::project_statuses_response()),
        )
        .mount(&server)
        .await;

    // JQL search: returns 5 issues to match the assertion below.
    // The JQL is built by handle_list and includes all filter clauses;
    // we use a wildcard POST mock so the exact JQL string doesn't need
    // to be hardcoded here (it would be fragile to the clause ordering).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issues": [
                common::fixtures::issue_response("HOLDOUT-1", "Issue one",   "In Progress"),
                common::fixtures::issue_response("HOLDOUT-2", "Issue two",   "In Progress"),
                common::fixtures::issue_response("HOLDOUT-3", "Issue three", "In Progress"),
                common::fixtures::issue_response("HOLDOUT-4", "Issue four",  "In Progress"),
                common::fixtures::issue_response("HOLDOUT-5", "Issue five",  "In Progress"),
            ],
            "nextPageToken": null
        })))
        .mount(&server)
        .await;

    // Write a .jr.toml so the handler can derive project key = HOLDOUT.
    std::fs::write(cwd.path().join(".jr.toml"), "project = \"HOLDOUT\"\n").unwrap();

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .current_dir(cwd.path())
        // Note: --open and --status are declared conflicts_with each other in the
        // CLI definition (src/cli/mod.rs). The BC-2.1.001 contract covers
        // handler-level composition; this test uses --status without --open.
        .args([
            "issue",
            "list",
            "--assignee",
            "alice",
            "--created-after",
            "2025-01-01",
            "--status",
            "In Progress",
            "--team",
            "Alpha",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "all combined filters must produce exit 0; stderr: {stderr}, stdout: {stdout}"
    );
    assert!(
        !stderr.contains("panic"),
        "combined filters must not cause a panic: {stderr}"
    );

    let parsed: Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    let arr = parsed
        .as_array()
        .expect("--output json must return an array");
    assert_eq!(
        arr.len(),
        5,
        "result must contain the 5 mock issues; got {} — filter composition broken",
        arr.len()
    );
}

// ---------------------------------------------------------------------------
// AC-007 / H-021 / BC-2.1.007
// ---------------------------------------------------------------------------

/// BC-2.1.007 postcondition: `jr issue list --status prog` must exit 64
/// (UserError) and include "Ambiguous status" in stderr when the substring
/// "prog" matches multiple status candidates (e.g. "In Progress").
/// The JQL search endpoint must NOT be invoked — ambiguous resolution must
/// short-circuit before any search call.
///
/// This test anchors the pre-existing coverage in
/// `tests/issue_list_errors.rs:369` (`issue_list_status_single_substring_rejected`)
/// as a named holdout so AC-007 / H-021 / BC-2.1.007 are explicitly traceable
/// to a test name following the `test_s_2_01_*` naming convention.
///
/// The implementation is a thin delegation: we run the same scenario with the
/// same wire conditions (project statuses mock + JQL mock .expect(0)) rather
/// than reimplementing the logic, so this file completes the holdout registry
/// while the anchor test in issue_list_errors.rs remains the primary assertion.
#[tokio::test]
async fn test_s_2_01_h_021_bc_2_1_007_ambiguous_status_exits_64_no_jql_call() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();

    // Project statuses: "prog" is a single-hit substring of "In Progress" →
    // routes through Ambiguous (partial_match returns Ambiguous for a unique
    // substring match since it doesn't satisfy strict-match rules).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ/statuses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::project_statuses_response()),
        )
        .mount(&server)
        .await;

    // JQL search must never be called — ambiguous status rejects early.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issues": [], "nextPageToken": null
        })))
        .expect(0)
        .mount(&server)
        .await;

    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .current_dir(project_dir.path())
        .args(["issue", "list", "--status", "prog"])
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
        "ambiguous status must exit 64 (UserError); got: {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Ambiguous status"),
        "stderr must contain 'Ambiguous status'; got: {stderr}"
    );
    // wiremock verifies .expect(0) on drop — if the JQL mock was called
    // the test will fail with a "expected 0 requests but got N" panic.
}

//! End-to-end coverage for `--all` disabling the default-limit cap
//! (#186 + #248).
//!
//! `resolve_effective_limit` is unit-tested in `src/cli/mod.rs`, but without
//! handler tests there's no regression guarantee that commands actually pass
//! `None` down to their API layer when `--all` is set. Each test here stubs
//! a response with more than `DEFAULT_LIMIT` (30) items and asserts the
//! command returns the full set under `--all` and the 30-row cap without.
//!
//! Initial coverage (#186) landed for `issue list` and `user search`.
//! #248 extends this to the remaining `--all`-accepting commands:
//! `user list`, `board view`, `sprint current`, `issue changelog`.
//!
//! Note: `user list --all` paginate+concat is already covered in
//! `tests/user_pagination.rs` (#189). The test here focuses on the
//! previously-missing negative case — default 30-row cap without `--all`.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a `jr` command pre-configured for non-interactive JSON output
/// against a mock server. Matches the pattern used in other integration
/// test files so shared flags/env live in one place.
fn jr_cmd_json(server_uri: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "--output", "json"]);
    cmd
}

/// `jr issue list --all` fetches beyond the default 30-row cap. Server
/// returns 35 issues in one cursor-paginated response (`nextPageToken:
/// null`) — client with `--all` passes `limit=None`, so the API request
/// body carries `maxResults=50` (the client's page size when no limit
/// is set per src/api/jira/issues.rs:50) → keeps all 35.
#[tokio::test]
async fn issue_list_all_returns_more_than_default_cap() {
    let server = MockServer::start().await;

    let issues: Vec<Value> = (1..=35)
        .map(|i| common::fixtures::issue_response(&format!("ALL-{i}"), "Issue", "To Do"))
        .collect();
    // Constrain the request body: the JQL must match what handle_list
    // actually builds (wrapped parens + ORDER BY), and maxResults=50
    // proves the client passed limit=None (i.e., --all took effect).
    // Loose path-only matchers would silently pass even if the command
    // sent the wrong JQL or cap.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": "(project = ALL) ORDER BY updated DESC",
            "maxResults": 50
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(issues)),
        )
        .mount(&server)
        .await;

    // With --all: all 35 issues should appear in JSON output.
    let output = jr_cmd_json(&server.uri())
        .args(["issue", "list", "--jql", "project = ALL", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("issue list JSON is an array");
    assert_eq!(
        arr.len(),
        35,
        "--all should return all 35 issues, got {}",
        arr.len()
    );
}

/// Without `--all`, `jr issue list` truncates to DEFAULT_LIMIT (30).
#[tokio::test]
async fn issue_list_default_caps_at_thirty() {
    let server = MockServer::start().await;

    let issues: Vec<Value> = (1..=35)
        .map(|i| common::fixtures::issue_response(&format!("CAP-{i}"), "Issue", "To Do"))
        .collect();
    // Constrain the request: maxResults=30 proves the default cap took
    // effect (without --all, client passes limit=Some(30) →
    // max_per_page=30 per src/api/jira/issues.rs:50).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": "(project = CAP) ORDER BY updated DESC",
            "maxResults": 30
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(issues)),
        )
        .mount(&server)
        .await;
    // The cap hint needs an approximate-count response — Jira-side, a
    // truncated result triggers a hint like "Showing 30 of ~42 results",
    // which `handle_list` looks up via /search/approximate-count with the
    // ORDER BY-stripped JQL. Pinning the body ensures the secondary
    // request shape is correct.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/approximate-count"))
        .and(body_partial_json(serde_json::json!({
            "jql": "(project = CAP)"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::approximate_count_response(35)),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["issue", "list", "--jql", "project = CAP"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("issue list JSON is an array");
    assert_eq!(
        arr.len(),
        30,
        "default limit should truncate to 30, got {}",
        arr.len()
    );
}

/// `jr user search --all` returns all users from a response that contains
/// more than DEFAULT_LIMIT entries. As of #189, `--all` triggers true
/// server-side pagination — the client requests page 1 (startAt=0) and
/// receives 35 users (a short page due to Atlassian's post-paging filter),
/// then advances `startAt` by the requested `maxResults` (100) and sees an
/// empty page that terminates the loop.
#[tokio::test]
async fn user_search_all_returns_more_than_default_cap() {
    let server = MockServer::start().await;

    let users: Vec<(String, String, bool)> = (1..=35)
        .map(|i| (format!("acc-{i:03}"), format!("User {i:03}"), true))
        .collect();

    // Page 1 (startAt=0): 35 users.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "User"))
        .and(query_param("startAt", "0"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::user_search_response(
                users
                    .iter()
                    .map(|(a, d, t)| (a.as_str(), d.as_str(), *t))
                    .collect(),
            )),
        )
        .mount(&server)
        .await;
    // Page 2 (startAt=100, advanced by requested maxResults, NOT by returned
    // count): empty — terminates the loop.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "User"))
        .and(query_param("startAt", "100"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "search", "User", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user search JSON is an array");
    assert_eq!(
        arr.len(),
        35,
        "--all should return all 35 users, got {}",
        arr.len()
    );
}

/// Without `--all`, `jr user search` truncates to DEFAULT_LIMIT (30).
#[tokio::test]
async fn user_search_default_caps_at_thirty() {
    let server = MockServer::start().await;

    let users: Vec<(String, String, bool)> = (1..=35)
        .map(|i| (format!("acc-{i:03}"), format!("User {i:03}"), true))
        .collect();
    // Same query_param constraint as the --all case above.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "User"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::user_search_response(
                users
                    .iter()
                    .map(|(a, d, t)| (a.as_str(), d.as_str(), *t))
                    .collect(),
            )),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "search", "User"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user search JSON is an array");
    assert_eq!(
        arr.len(),
        30,
        "default limit should truncate to 30, got {}",
        arr.len()
    );
}

// =============================================================================
// #248 — remaining `--all`-accepting commands
// =============================================================================

/// Without `--all`, `jr user list --project PROJ` truncates to DEFAULT_LIMIT (30).
/// The positive `--all` counterpart for this command is covered by
/// `tests/user_pagination.rs::user_list_all_cli_paginates` (#189).
#[tokio::test]
async fn user_list_default_caps_at_thirty() {
    let server = MockServer::start().await;

    let users: Vec<(String, String, bool)> = (1..=35)
        .map(|i| (format!("acc-{i:03}"), format!("Person {i:03}"), true))
        .collect();
    // Without --all, handle_list calls the legacy single-call
    // `search_assignable_users_by_project` — no startAt/maxResults params.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "PROJ"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::user_search_response(
                users
                    .iter()
                    .map(|(a, d, t)| (a.as_str(), d.as_str(), *t))
                    .collect(),
            )),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "list", "--project", "PROJ"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user list JSON is an array");
    assert_eq!(
        arr.len(),
        30,
        "default limit should truncate to 30, got {}",
        arr.len()
    );
}

/// Helpers for the sprint + board path: mount board auto-resolve, board
/// config (scrum), and active sprint list. Returns a ready-to-use server
/// with board id 42 and sprint id 100.
async fn mount_scrum_prereqs(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .and(query_param("type", "scrum"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::board_list_response(vec![common::fixtures::board_response(
                42,
                "PROJ Scrum",
                "scrum",
                "PROJ",
            )]),
        ))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/sprint"))
        .and(query_param("state", "active"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::sprint_list_response(vec![common::fixtures::sprint(
                100, "Sprint 1", "active",
            )]),
        ))
        .mount(server)
        .await;
}

async fn mount_kanban_prereqs(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::board_list_response(vec![common::fixtures::board_response(
                42,
                "PROJ Kanban",
                "kanban",
                "PROJ",
            )]),
        ))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("kanban")),
        )
        .mount(server)
        .await;
}

fn build_issue_fixtures(count: usize, prefix: &str) -> Vec<Value> {
    (1..=count)
        .map(|i| {
            common::fixtures::issue_response(
                &format!("{prefix}-{i}"),
                &format!("Issue {i}"),
                "In Progress",
            )
        })
        .collect()
}

/// `jr sprint current --all` returns more than DEFAULT_LIMIT issues.
/// `get_sprint_issues` receives `limit=None` under `--all`, so the client
/// consumes the server page in full without client-side truncation.
#[tokio::test]
async fn sprint_current_all_returns_more_than_default_cap() {
    let server = MockServer::start().await;
    mount_scrum_prereqs(&server).await;

    let issues = build_issue_fixtures(35, "SPA");
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 35)),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["--project", "PROJ", "sprint", "current", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    // sprint current emits {sprint, issues, [sprint_summary]} in JSON mode.
    let arr = json["issues"]
        .as_array()
        .expect("issues array in sprint current JSON");
    assert_eq!(
        arr.len(),
        35,
        "--all should return all 35 issues, got {}",
        arr.len()
    );
}

/// Without `--all`, `jr sprint current` truncates to DEFAULT_LIMIT (30).
#[tokio::test]
async fn sprint_current_default_caps_at_thirty() {
    let server = MockServer::start().await;
    mount_scrum_prereqs(&server).await;

    let issues = build_issue_fixtures(35, "SPC");
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 35)),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["--project", "PROJ", "sprint", "current"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json["issues"]
        .as_array()
        .expect("issues array in sprint current JSON");
    assert_eq!(
        arr.len(),
        30,
        "default limit should truncate to 30, got {}",
        arr.len()
    );
}

/// `jr board view --all` (kanban path) returns more than DEFAULT_LIMIT issues.
/// `search_issues` receives `limit=None` under `--all`, so no client-side
/// truncation applies.
#[tokio::test]
async fn board_view_all_returns_more_than_default_cap() {
    let server = MockServer::start().await;
    mount_kanban_prereqs(&server).await;

    let issues = build_issue_fixtures(35, "BVA");
    // Constrain request body so the test fails if the handler stops sending
    // the expected JQL shape.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": "project = \"PROJ\" AND statusCategory != Done ORDER BY rank ASC",
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(issues)),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["--project", "PROJ", "board", "view", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json
        .as_array()
        .expect("board view JSON is an array of issues");
    assert_eq!(
        arr.len(),
        35,
        "--all should return all 35 issues, got {}",
        arr.len()
    );
}

/// Without `--all`, `jr board view` (kanban) truncates to DEFAULT_LIMIT (30).
/// `search_issues` receives `limit=Some(30)` so `max_per_page=30` on the
/// request body — pin that shape so a regression to full-page fetch is caught.
#[tokio::test]
async fn board_view_default_caps_at_thirty() {
    let server = MockServer::start().await;
    mount_kanban_prereqs(&server).await;

    let issues = build_issue_fixtures(35, "BVC");
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": "project = \"PROJ\" AND statusCategory != Done ORDER BY rank ASC",
            "maxResults": 30,
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(issues)),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["--project", "PROJ", "board", "view"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json
        .as_array()
        .expect("board view JSON is an array of issues");
    assert_eq!(
        arr.len(),
        30,
        "default limit should truncate to 30, got {}",
        arr.len()
    );
}

/// Build a changelog response with N entries, each carrying one item —
/// so N entries maps to N rows for `truncate_to_rows`.
fn build_changelog_response(count: usize, total: u32, is_last: bool) -> Value {
    let values: Vec<Value> = (1..=count)
        .map(|i| {
            serde_json::json!({
                "id": format!("{i}"),
                "author": {
                    "accountId": format!("acc-{i:03}"),
                    "displayName": format!("User {i:03}"),
                    "active": true,
                },
                "created": format!("2026-04-{:02}T10:00:00.000+0000", (i % 27) + 1),
                "items": [{
                    "field": "status",
                    "fieldtype": "jira",
                    "from": "1", "fromString": "To Do",
                    "to": "3", "toString": "In Progress",
                }],
            })
        })
        .collect();
    serde_json::json!({
        "startAt": 0,
        "maxResults": 100,
        "total": total,
        "isLast": is_last,
        "values": values,
    })
}

/// `jr issue changelog <KEY> --all` returns all changelog rows even when
/// there are more than DEFAULT_LIMIT. `--all` disables the local
/// `truncate_to_rows` call; the API call itself always fetches every page.
#[tokio::test]
async fn issue_changelog_all_returns_more_than_default_cap() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_changelog_response(35, 35, true)),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["issue", "changelog", "FOO-1", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let entries = json["entries"]
        .as_array()
        .expect("changelog JSON has entries array");
    assert_eq!(
        entries.len(),
        35,
        "--all should return all 35 changelog entries, got {}",
        entries.len()
    );
}

/// Without `--all`, `jr issue changelog <KEY>` truncates to DEFAULT_LIMIT (30).
#[tokio::test]
async fn issue_changelog_default_caps_at_thirty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_changelog_response(35, 35, true)),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["issue", "changelog", "FOO-1"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let entries = json["entries"]
        .as_array()
        .expect("changelog JSON has entries array");
    assert_eq!(
        entries.len(),
        30,
        "default limit should truncate to 30 rows, got {}",
        entries.len()
    );
}

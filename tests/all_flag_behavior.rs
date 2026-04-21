//! End-to-end coverage for `--all` disabling the default-limit cap (#186).
//!
//! `resolve_effective_limit` is unit-tested in `src/cli/mod.rs`, but without
//! handler tests there's no regression guarantee that commands actually pass
//! `None` down to their API layer when `--all` is set. Each test here stubs
//! a response with more than `DEFAULT_LIMIT` (30) items and asserts the
//! command returns the full set under `--all` and the 30-row cap without.
//!
//! Scope note: this PR covers `issue list` and `user search`. The other four
//! `--all` commands (`user list`, `board view`, `sprint current`,
//! `issue changelog`) are deferred to a follow-up issue.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{method, path};
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
/// returns 35 issues in one cursor-paginated response (`nextPageToken`
/// absent) — client with `--all` passes `limit=None` → keeps all 35.
/// Client without `--all` passes `limit=Some(30)` → truncates to 30.
#[tokio::test]
async fn issue_list_all_returns_more_than_default_cap() {
    let server = MockServer::start().await;

    let issues: Vec<Value> = (1..=35)
        .map(|i| common::fixtures::issue_response(&format!("ALL-{i}"), "Issue", "To Do"))
        .collect();
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
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
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(issues)),
        )
        .mount(&server)
        .await;
    // The cap hint needs an approximate-count response too — Jira-side, a
    // truncated result triggers a hint like "Showing 30 of ~42 results",
    // which `handle_list` looks up via /search/approximate-count. Stub it
    // so the command doesn't error on the secondary call.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/approximate-count"))
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
/// more than DEFAULT_LIMIT entries. `search_users` is flat (no pagination),
/// so truncation is purely client-side.
#[tokio::test]
async fn user_search_all_returns_more_than_default_cap() {
    let server = MockServer::start().await;

    let users: Vec<(String, String, bool)> = (1..=35)
        .map(|i| (format!("acc-{i:03}"), format!("User {i:03}"), true))
        .collect();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
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
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
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

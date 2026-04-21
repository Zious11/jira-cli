//! End-to-end coverage for `--all` true pagination on `user search` and
//! `user list` (#189). Library-level tests assert that `_all` variants loop
//! the endpoint until an empty page is returned and that pages are
//! concatenated in order. CLI-level tests verify the flag wiring in handlers.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use jr::api::client::JiraClient;

/// Build a `jr` command pre-configured for non-interactive JSON output
/// against a mock server. Matches the pattern used in tests/all_flag_behavior.rs.
#[allow(dead_code)]
fn jr_cmd_json(server_uri: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "--output", "json"]);
    cmd
}

/// Build a user-search fixture of `count` users with names/ids derived from `prefix`.
/// Constructs the JSON response directly from owned `String`s to avoid leaking
/// memory just to satisfy the borrowed-string fixture signature.
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

/// `search_users_all` paginates three sequential pages (100 + 100 + 27)
/// and returns 227 users concatenated in order. `startAt` advances by the
/// requested page size (100) each iteration regardless of returned count —
/// Jira uses fixed-window pagination, so advancing by the short page's
/// length would re-scan already-seen raw users.
#[tokio::test]
async fn search_users_all_paginates_and_concatenates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "100"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "200"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(27, "p3")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
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
    assert_eq!(users.len(), 227, "expected 227 users across 3 pages");
    assert_eq!(users[0].display_name, "p1 User 000");
    assert_eq!(users[100].display_name, "p2 User 000");
    assert_eq!(users[200].display_name, "p3 User 000");
    assert_eq!(users[226].display_name, "p3 User 026");
}

/// Loop stops as soon as a page comes back empty; subsequent startAt
/// windows are not requested. The strict `.expect(1)` on each mock,
/// combined with wiremock rejecting unmatched requests, asserts that
/// any fourth request would fail.
#[tokio::test]
async fn search_users_all_stops_on_empty_page() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "100"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users_all("u").await.expect("must succeed");
    assert_eq!(users.len(), 100);
}

/// If the API never returns an empty page (pathological behavior), the loop
/// stops at USER_PAGINATION_SAFETY_CAP iterations = 15 requests.
#[tokio::test]
async fn search_users_all_respects_safety_cap() {
    let server = MockServer::start().await;

    // Unbounded responder for any startAt; .expect(15) pins the iteration cap.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "cap")))
        .expect(15)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users_all("u").await.expect("must succeed");
    assert_eq!(users.len(), 1500, "15 iterations * 100 per page = 1500");
}

/// If a page request fails mid-pagination, the error is propagated and the
/// loop does not silently return partial results.
#[tokio::test]
async fn search_users_all_propagates_error_mid_pagination() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.search_users_all("u").await;
    let err = result.expect_err("500 on page 2 must propagate");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("500"),
        "error must surface the 500 status, got: {msg}"
    );
}

/// Atlassian docs warn that the user-search endpoint "usually returns fewer
/// users than specified in maxResults" due to post-page filtering. A short
/// non-empty page is NOT end-of-data; the loop must keep paginating until it
/// sees a truly empty page. Pins two contracts at once: (a) short response
/// doesn't trigger early termination, and (b) `startAt` advances by the
/// requested window size (100) regardless of returned count — advancing by
/// 35 would re-scan users[35..100] and produce duplicates per JRACLOUD-71293.
#[tokio::test]
async fn search_users_all_continues_past_short_non_empty_page() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(35, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "200"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p3")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "300"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users_all("u").await.expect("must succeed");
    assert_eq!(
        users.len(),
        235,
        "must keep paginating past a short non-empty page (100 + 35 + 100)"
    );
}

/// `search_assignable_users_by_project_all` paginates the assignable-users
/// endpoint and concatenates pages in order.
#[tokio::test]
async fn search_assignable_users_by_project_all_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("query", ""))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "100"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(40, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "200"))
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
        .search_assignable_users_by_project_all("", "FOO")
        .await
        .expect("pagination must succeed");
    assert_eq!(users.len(), 140);
    assert_eq!(users[0].display_name, "p1 User 000");
    assert_eq!(users[100].display_name, "p2 User 000");
}

/// End-to-end: `jr user search --all` paginates and emits all users as JSON.
#[tokio::test]
async fn user_search_all_cli_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(50, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "200"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "search", "u", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user search --all JSON is an array");
    assert_eq!(arr.len(), 150, "--all should paginate to 150 users");
}

/// End-to-end: `jr user list --all --project FOO` paginates.
#[tokio::test]
async fn user_list_all_cli_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(35, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "200"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "list", "--project", "FOO", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user list --all JSON is an array");
    assert_eq!(arr.len(), 135);
}

/// Without `--all`, `jr user search` must still make exactly one API request
/// (the existing single-call path) — no accidental pagination.
/// Asserts via `received_requests()` that the request query string contains
/// no `startAt` or `maxResults` — a loose matcher like `query_param("query", "u")`
/// would still match a paginated request, so the post-hoc inspection is
/// required to actually guard against accidental pagination regression.
#[tokio::test]
async fn user_search_no_all_issues_single_request() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(50, "u")))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "search", "u"])
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
        "default cap should truncate to 30, got {}",
        arr.len()
    );

    // Primary guard: verify the actual request the binary sent contains no
    // pagination parameters. Without `--all` the single-call path must not
    // send `startAt` or `maxResults`.
    let requests = server
        .received_requests()
        .await
        .expect("received_requests must be recording");
    assert_eq!(requests.len(), 1, "expected exactly one API request");
    let query = requests[0].url.query().unwrap_or("");
    assert!(
        !query.contains("startAt"),
        "single-call path must not send startAt; got query: {query}"
    );
    assert!(
        !query.contains("maxResults"),
        "single-call path must not send maxResults; got query: {query}"
    );
}

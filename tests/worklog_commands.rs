#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_add_worklog() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "12345",
            "timeSpentSeconds": 7200,
            "timeSpent": "2h",
            "author": {"accountId": "abc", "displayName": "Test User"}
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let worklog = client.add_worklog("FOO-1", "2h", None).await.unwrap();
    assert_eq!(worklog.time_spent_seconds, Some(7200));
}

#[tokio::test]
async fn test_list_worklogs() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/worklog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "startAt": 0, "maxResults": 50, "total": 1,
            "worklogs": [{
                "id": "12345",
                "timeSpentSeconds": 3600,
                "timeSpent": "1h",
                "author": {"accountId": "abc", "displayName": "Test User"},
                "started": "2026-03-21T10:00:00.000+0000"
            }]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let worklogs = client.list_worklogs("FOO-1").await.unwrap();
    assert_eq!(worklogs.len(), 1);
    assert_eq!(worklogs[0].time_spent_seconds, Some(3600));
}

// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn worklog_list_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["worklog", "list", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "5xx should exit 1, got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("API error (500)"),
        "Expected 'API error (500)' in stderr, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn worklog_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["worklog", "list", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "401 should exit 2, got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Not authenticated"),
        "Expected 'Not authenticated' in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "Expected 'jr auth login' suggestion in stderr, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn worklog_list_network_drop_surfaces_reach_error() {
    // Privileged port 1 — connect-refused from any unprivileged process.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["worklog", "list", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "Net-drop should exit 1, got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Could not reach"),
        "Expected 'Could not reach' in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("check your connection"),
        "Expected 'check your connection' in stderr, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

// ─── BC-X.5.002: Pagination tests (S-0.02, H-045) ──────────────────────────

/// Build a worklog JSON item with a unique-enough ID for test bodies.
fn make_worklog_item(id: u32) -> serde_json::Value {
    serde_json::json!({
        "id": id.to_string(),
        "timeSpentSeconds": 3600,
        "timeSpent": "1h",
        "author": {"accountId": "abc", "displayName": "Test User"},
        "started": "2026-03-21T10:00:00.000+0000"
    })
}

/// Build a worklogs page body containing `count` items.
fn make_worklogs_page(start_at: u32, total: u32, count: u32) -> serde_json::Value {
    let worklogs: Vec<serde_json::Value> = (0..count)
        .map(|i| make_worklog_item(start_at + i + 1))
        .collect();
    serde_json::json!({
        "startAt": start_at,
        "maxResults": 50,
        "total": total,
        "worklogs": worklogs
    })
}

/// AC-001 (BC-X.5.002 postcondition): two-page result MUST return all 80 items.
///
/// The current single-fetch implementation returns only 50 (page 1). This test
/// MUST FAIL before the pagination loop is implemented (Red Gate, H-045).
#[tokio::test]
async fn test_bc_x_5_002_two_page_result_returns_all_80_items() {
    let server = MockServer::start().await;

    // Page 1: startAt=0, 50 items, total=80
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_worklogs_page(0, 80, 50)))
        .expect(1)
        .mount(&server)
        .await;

    // Page 2: startAt=50, 30 items, total=80
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .and(query_param("startAt", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_worklogs_page(50, 80, 30)))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let worklogs = client.list_worklogs("PROJ-1").await.unwrap();

    assert_eq!(
        worklogs.len(),
        80,
        "AC-001 FAIL (H-045): expected 80 items from two pages, got {}",
        worklogs.len()
    );
    // wiremock verifies both mocks had exactly 1 call each on server drop
}

/// AC-002 (BC-X.5.002 postcondition): both pages MUST be fetched from the server.
///
/// The second mock has `.expect(1)` — wiremock panics on drop if it was never
/// called. This MUST FAIL before the pagination loop is in place (Red Gate, H-045).
#[tokio::test]
async fn test_bc_x_5_002_both_pages_fetched() {
    let server = MockServer::start().await;

    // Page 1
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_worklogs_page(0, 80, 50)))
        .expect(1)
        .mount(&server)
        .await;

    // Page 2 — MUST be called exactly once; wiremock fails on drop if skipped.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .and(query_param("startAt", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_worklogs_page(50, 80, 30)))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let worklogs = client.list_worklogs("PROJ-1").await.unwrap();

    // Both pages together must produce exactly 80 items.
    let page1_items: Vec<_> = worklogs
        .iter()
        .filter(|w| {
            w.id.as_deref()
                .and_then(|id| id.parse::<u32>().ok())
                .is_some_and(|n| n <= 50)
        })
        .collect();
    let page2_items: Vec<_> = worklogs
        .iter()
        .filter(|w| {
            w.id.as_deref()
                .and_then(|id| id.parse::<u32>().ok())
                .is_some_and(|n| n > 50)
        })
        .collect();

    assert_eq!(
        page1_items.len(),
        50,
        "AC-002: expected 50 items from page 1"
    );
    assert_eq!(
        page2_items.len(),
        30,
        "AC-002: expected 30 items from page 2"
    );
    // wiremock verifies both mocks had exactly 1 call each on server drop
}

/// AC-003 (BC-X.5.002 regression guard): single-page result MUST NOT trigger a
/// second request.
///
/// This is the over-fetch regression guard. It is expected to PASS pre-fix
/// (the current single-fetch implementation never makes a second call).
/// It MUST also PASS post-fix (pagination must stop when total is reached).
///
/// Uses a path-only matcher for the first mock so it matches both the current
/// (no-query-param) path and the post-fix (?startAt=0) path. The second mock
/// uses `query_param("startAt", "30")` with `.expect(0)` to catch spurious
/// over-fetches in the post-fix implementation.
#[tokio::test]
async fn test_bc_x_5_002_single_page_no_extra_fetch() {
    let server = MockServer::start().await;

    // Only page: 30 items, total=30 — all items fit in one page.
    // Path-only matcher works for both pre-fix (no query params) and post-fix
    // (?startAt=0) call patterns.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_worklogs_page(0, 30, 30)))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let worklogs = client.list_worklogs("PROJ-1").await.unwrap();

    assert_eq!(
        worklogs.len(),
        30,
        "AC-003: expected exactly 30 items from single page, got {}",
        worklogs.len()
    );
    // wiremock verifies endpoint was called exactly once on drop (no over-fetch)
}

/// AC-004 (BC-X.5.002 loop-termination guard): empty issue MUST return zero items
/// without entering an infinite loop.
///
/// This is expected to PASS pre-fix and MUST also PASS post-fix.
/// Path-only matcher so it works for both pre-fix and post-fix call patterns.
#[tokio::test]
async fn test_bc_x_5_002_empty_issue_returns_zero_items() {
    let server = MockServer::start().await;

    // Empty result: total=0, no items.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "startAt": 0,
            "maxResults": 50,
            "total": 0,
            "worklogs": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let worklogs = client.list_worklogs("PROJ-1").await.unwrap();

    assert_eq!(
        worklogs.len(),
        0,
        "AC-004: expected 0 items for empty issue, got {}",
        worklogs.len()
    );
    // wiremock verifies the endpoint was called exactly once on drop
}

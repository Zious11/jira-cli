#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_comments_returns_all_comments() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10001",
                    "author": { "accountId": "abc", "displayName": "Alice", "emailAddress": "a@test.com", "active": true },
                    "body": { "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "First comment" }] }] },
                    "created": "2026-03-20T10:00:00.000+0000"
                },
                {
                    "id": "10002",
                    "author": { "accountId": "def", "displayName": "Bob", "emailAddress": "b@test.com", "active": true },
                    "body": { "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Second comment" }] }] },
                    "created": "2026-03-21T11:00:00.000+0000"
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 2
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let comments = client.list_comments("FOO-1", None).await.unwrap();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].id.as_deref(), Some("10001"));
    assert_eq!(comments[1].id.as_deref(), Some("10002"));
}

#[tokio::test]
async fn list_comments_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-2/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [],
            "startAt": 0,
            "maxResults": 100,
            "total": 0
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let comments = client.list_comments("FOO-2", None).await.unwrap();
    assert!(comments.is_empty());
}

#[tokio::test]
async fn list_comments_with_limit() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-3/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "1"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10001",
                    "author": { "accountId": "abc", "displayName": "Alice", "emailAddress": "a@test.com", "active": true },
                    "body": null,
                    "created": "2026-03-20T10:00:00.000+0000"
                }
            ],
            "startAt": 0,
            "maxResults": 1,
            "total": 2
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let comments = client.list_comments("FOO-3", Some(1)).await.unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id.as_deref(), Some("10001"));
}

#[tokio::test]
async fn list_comments_paginated() {
    let server = MockServer::start().await;

    // Page 1
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-4/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10001",
                    "author": { "accountId": "abc", "displayName": "Alice", "emailAddress": "a@test.com", "active": true },
                    "body": null,
                    "created": "2026-03-20T10:00:00.000+0000"
                }
            ],
            "startAt": 0,
            "maxResults": 1,
            "total": 2
        })))
        .mount(&server)
        .await;

    // Page 2
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-4/comment"))
        .and(query_param("startAt", "1"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10002",
                    "author": { "accountId": "def", "displayName": "Bob", "emailAddress": "b@test.com", "active": true },
                    "body": null,
                    "created": "2026-03-21T11:00:00.000+0000"
                }
            ],
            "startAt": 1,
            "maxResults": 1,
            "total": 2
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let comments = client.list_comments("FOO-4", None).await.unwrap();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].id.as_deref(), Some("10001"));
    assert_eq!(comments[1].id.as_deref(), Some("10002"));
}

// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn issue_comments_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
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
        .args(["issue", "comments", "PROJ-1"])
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
async fn issue_comments_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
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
        .args(["issue", "comments", "PROJ-1"])
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
async fn issue_comments_network_drop_surfaces_reach_error() {
    // Privileged port 1 — connect-refused from any unprivileged process.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "PROJ-1"])
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

#[tokio::test]
async fn comments_verbose_logs_parse_failure_once() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/BAD-1/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10001",
                    "author": {
                        "accountId": "u1", "displayName": "Alice",
                        "emailAddress": "a@test.com", "active": true
                    },
                    "body": { "type": "doc", "version": 1, "content": [
                        { "type": "paragraph", "content": [
                            { "type": "text", "text": "first" }
                        ]}
                    ]},
                    "created": "not-a-date"
                },
                {
                    "id": "10002",
                    "author": {
                        "accountId": "u1", "displayName": "Alice",
                        "emailAddress": "a@test.com", "active": true
                    },
                    "body": { "type": "doc", "version": 1, "content": [
                        { "type": "paragraph", "content": [
                            { "type": "text", "text": "second" }
                        ]}
                    ]},
                    "created": "still-not-a-date"
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 2
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "BAD-1", "--verbose"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "jr exited non-zero ({:?}). stdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );
    let count = stderr.matches("timestamp failed to parse").count();
    assert_eq!(
        count, 1,
        "expected exactly one parse-failure log across 2 bad comments, got {count}. stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("[verbose] date"),
        "expected [verbose] date prefix in stderr, got:\n{stderr}"
    );
}

#[tokio::test]
async fn comments_parse_failure_silent_without_verbose() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/BAD-2/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10001",
                    "author": {
                        "accountId": "u1", "displayName": "Alice",
                        "emailAddress": "a@test.com", "active": true
                    },
                    "body": { "type": "doc", "version": 1, "content": [
                        { "type": "paragraph", "content": [
                            { "type": "text", "text": "first" }
                        ]}
                    ]},
                    "created": "not-a-date"
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 1
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "BAD-2"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "jr exited non-zero ({:?}). stdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("failed to parse"),
        "expected no verbose parse-failure output without --verbose, got:\n{stderr}"
    );
}

// ─── BC-2.4.043 — anti-stall guard ──────────────────────────────────────────

/// AC-002 / BC-2.4.043: non-advancing startAt must return Err (not hang).
///
/// Simulates a malformed Jira response where `total > len(comments)` (has_more=true)
/// but startAt stays at 0 on the first page, which would cause an infinite loop
/// without the guard. The guard fires after page 1 and returns Err.
#[tokio::test]
async fn test_list_comments_stall_guard_returns_error_when_start_at_does_not_advance() {
    let server = MockServer::start().await;

    // Page 1: startAt=0, maxResults=0 in response, total=5.
    // next_start = startAt + maxResults = 0 + 0 = 0.
    // has_more = 0 + 0 < 5 = true.
    // Guard fires: next (0) <= start_at (0) → Err.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10001",
                    "author": {
                        "accountId": "abc", "displayName": "Alice",
                        "emailAddress": "a@test.com", "active": true
                    },
                    "body": null,
                    "created": "2026-03-20T10:00:00.000+0000"
                }
            ],
            "startAt": 0,
            "maxResults": 0,
            "total": 5
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.list_comments("TEST-1", None).await;

    assert!(result.is_err(), "expected Err from stall guard, got Ok");
    let err = result.unwrap_err();
    assert!(
        err.to_string()
            .contains("aborting to prevent infinite loop"),
        "expected 'aborting to prevent infinite loop' in error, got: {err}"
    );
}

/// AC-002 / BC-2.4.043 (normal path): multi-page pagination with advancing startAt
/// must return Ok with all comments collected.
///
/// The existing `list_comments_paginated` test covers this code path — this test
/// is an explicit named regression pin that exercices the guard skip (guard does
/// NOT fire when next > start_at) to confirm normal pagination works correctly.
#[tokio::test]
async fn test_list_comments_paginates_correctly_when_offset_advances() {
    let server = MockServer::start().await;

    // Page 1: startAt=0, total=2, one comment — next_start = 0+1 = 1 > 0 (guard skipped).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ADV-1/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "20001",
                    "author": {
                        "accountId": "u1", "displayName": "Alice",
                        "emailAddress": "a@test.com", "active": true
                    },
                    "body": null,
                    "created": "2026-03-20T10:00:00.000+0000"
                }
            ],
            "startAt": 0,
            "maxResults": 1,
            "total": 2
        })))
        .mount(&server)
        .await;

    // Page 2: startAt=1, total=2, one comment — isLast (next_start == 1+1 == 2 >= total 2).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ADV-1/comment"))
        .and(query_param("startAt", "1"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "20002",
                    "author": {
                        "accountId": "u2", "displayName": "Bob",
                        "emailAddress": "b@test.com", "active": true
                    },
                    "body": null,
                    "created": "2026-03-21T11:00:00.000+0000"
                }
            ],
            "startAt": 1,
            "maxResults": 1,
            "total": 2
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let comments = client.list_comments("ADV-1", None).await.unwrap();

    assert_eq!(comments.len(), 2, "expected 2 comments from two pages");
    assert_eq!(comments[0].id.as_deref(), Some("20001"));
    assert_eq!(comments[1].id.as_deref(), Some("20002"));
}

/// AC-002 / BC-2.4.043 (mutation kill, deterministic): bounds the stall-guard call
/// with an explicit wall-clock timeout so the guard-boundary mutant
/// (`next <= start_at` → `next > start_at`, src/api/jira/issues.rs) is caught as a
/// FAST FAILURE rather than relying on cargo-mutants' multi-minute global timeout.
///
/// Same malformed response as the stall-guard test above: every page returns
/// startAt=0/maxResults=0 with total=5, so the offset NEVER advances. With the
/// correct `<=` guard, `list_comments` returns Err on the first iteration (well
/// under the 5s bound). With the mutated `>` guard, the loop never terminates —
/// the timeout elapses and this test fails immediately, killing the mutant.
#[tokio::test]
async fn test_list_comments_stall_guard_terminates_within_bounded_time() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-2/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "30001",
                    "author": {
                        "accountId": "abc", "displayName": "Alice",
                        "emailAddress": "a@test.com", "active": true
                    },
                    "body": null,
                    "created": "2026-03-20T10:00:00.000+0000"
                }
            ],
            "startAt": 0,
            "maxResults": 0,
            "total": 5
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let bounded = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.list_comments("TEST-2", None),
    )
    .await;

    // The outer Result is the timeout boundary: Err(Elapsed) means the loop never
    // terminated (mutant alive). Ok means list_comments returned in time.
    let result = bounded.expect(
        "list_comments did not terminate within 5s — anti-stall guard is broken \
         (non-advancing startAt must return Err, not loop forever)",
    );
    assert!(
        result.is_err(),
        "expected Err from stall guard within bounded time, got Ok"
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("aborting to prevent infinite loop"),
        "expected 'aborting to prevent infinite loop' in the bounded-time error"
    );
}

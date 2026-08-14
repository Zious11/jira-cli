#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_queues_returns_all_queues() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .and(query_param("includeCount", "true"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "10", "name": "Triage", "jql": "project = HELPDESK AND status = New", "issueCount": 12 },
                { "id": "20", "name": "In Progress", "jql": "project = HELPDESK AND status = \"In Progress\"", "issueCount": 7 }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let queues = client.list_queues("15").await.unwrap();
    assert_eq!(queues.len(), 2);
    assert_eq!(queues[0].name, "Triage");
    assert_eq!(queues[0].issue_count, Some(12));
    assert_eq!(queues[1].name, "In Progress");
}

#[tokio::test]
async fn list_queues_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 0,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": []
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let queues = client.list_queues("15").await.unwrap();
    assert!(queues.is_empty());
}

#[tokio::test]
async fn get_queue_issue_keys_returns_keys() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                {
                    "key": "HELPDESK-42",
                    "fields": {
                        "summary": "VPN not working",
                        "status": { "name": "New", "statusCategory": { "name": "To Do", "key": "new" } },
                        "issuetype": { "name": "Service Request" },
                        "priority": { "name": "High" },
                        "assignee": null
                    }
                },
                {
                    "key": "HELPDESK-41",
                    "fields": {
                        "summary": "Need license renewal",
                        "status": { "name": "New", "statusCategory": { "name": "To Do", "key": "new" } },
                        "issuetype": { "name": "Service Request" },
                        "assignee": { "accountId": "abc", "displayName": "Jane D." }
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let keys = client.get_queue_issue_keys("15", "10", None).await.unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0], "HELPDESK-42");
    assert_eq!(keys[1], "HELPDESK-41");
}

#[tokio::test]
async fn get_queue_issue_keys_with_limit() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 1,
            "isLastPage": false,
            "values": [
                {
                    "key": "HELPDESK-42",
                    "fields": {
                        "summary": "VPN not working"
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let keys = client
        .get_queue_issue_keys("15", "10", Some(1))
        .await
        .unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0], "HELPDESK-42");
}

#[tokio::test]
async fn get_queue_issue_keys_paginated() {
    let server = MockServer::start().await;

    // Page 1
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 1,
            "isLastPage": false,
            "values": [
                { "key": "HELPDESK-2", "fields": { "summary": "Issue A" } }
            ]
        })))
        .mount(&server)
        .await;

    // Page 2
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "1"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 1,
            "limit": 1,
            "isLastPage": true,
            "values": [
                { "key": "HELPDESK-1", "fields": { "summary": "Issue B" } }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let keys = client.get_queue_issue_keys("15", "10", None).await.unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0], "HELPDESK-2");
    assert_eq!(keys[1], "HELPDESK-1");
}

#[tokio::test]
async fn resolve_queue_duplicate_names_error_message() {
    let server = MockServer::start().await;

    // Two queues with the same name but different IDs
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .and(query_param("includeCount", "true"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "10", "name": "Triage", "issueCount": 5 },
                { "id": "20", "name": "Triage", "issueCount": 3 }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let result = jr::cli::queue::resolve_queue_by_name("15", "Triage", &client).await;

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Multiple queues named \"Triage\""),
        "Expected queue name in error, got: {msg}"
    );
    assert!(
        msg.contains("10, 20"),
        "Expected both queue IDs in error, got: {msg}"
    );
    assert!(
        msg.contains("Use --id 10 to specify"),
        "Expected --id suggestion in error, got: {msg}"
    );
}

/// G-H2: BC-X.10.001 EC-1 call-count pin — ambiguous partial_match short-circuits
/// with exit 64 and fires the queue-list endpoint EXACTLY ONCE (no follow-on HTTP).
///
/// The `.expect(1)` on the list mock verifies that (a) the list fetch ran (the
/// subprocess did not bail before hitting the network), and (b) no follow-on
/// request (e.g. a spurious queue-view GET or a second list fetch) was made —
/// because no other endpoint is mounted, any extra call would hit an unregistered
/// route and wiremock would return 404 (surfaced as a non-zero exit), but more
/// importantly `.expect(1)` on the list mock would panic on server drop if any
/// extra call matched it.
///
/// Non-tautology: would fail if partial_match stopped short-circuiting on
/// Ambiguous (a follow-on GET would hit an unmounted endpoint / exceed expect(1)).
///
/// BC anchor: BC-X.10.001 EC-1 (call-count pin recommended in BC text).
#[tokio::test]
async fn test_resolve_queue_ambiguous_fires_list_exactly_once_no_followon_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Serve a project-meta response so require_service_desk passes (queue handler
    // needs this before it reaches list_queues; must be service_desk type).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELPDESK"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "10000",
            "key": "HELPDESK",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .mount(&server)
        .await;

    // Service-desk meta (require_service_desk fetches /rest/servicedeskapi/servicedesk
    // to obtain the numeric service_desk_id for the project).
    // ServiceDesk struct requires: id, projectId, projectName (serde rename).
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "15", "projectId": "10000", "projectName": "HELPDESK Service Desk" }
            ]
        })))
        .mount(&server)
        .await;

    // The queue list: mount with .expect(1) so wiremock panics on server drop if
    // the endpoint is called 0 times (command bailed early) or >1 times (retry/loop).
    // "esc" matches "Escalations" as a single-substring → Ambiguous → exit 64.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .and(query_param("includeCount", "true"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "10", "name": "Escalations", "issueCount": 5 },
                { "id": "20", "name": "General Requests", "issueCount": 3 }
            ]
        })))
        .expect(1) // BC-X.10.001 EC-1: the list fetch fires exactly once
        .mount(&server)
        .await;

    // No queue-view endpoint is mounted — any follow-on HTTP (e.g. GET queue issues)
    // would hit an unregistered route, which in wiremock returns 404 and causes the
    // command to exit non-zero; the exit-64 assertion below would then fail and the
    // disambiguation-message assertion would also fail, surfacing the defect.

    write_minimal_config(config_dir.path(), &server.uri());

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "queue",
            "view",
            "--project",
            "HELPDESK",
            "--no-input",
            "esc",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit 64 (UserError / disambiguation)
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for ambiguous queue name; stderr={stderr}"
    );

    // Must emit the disambiguation phrase (Ambiguous path in partial_match).
    // The real message at queue.rs::resolve_queue_by_name is:
    //   "\"{}\" matches multiple queues: …"
    // The lowercase "ambiguous" alternative never appears in subprocess stderr —
    // it only surfaces in the inline unit-test assertions in queue.rs itself.
    assert!(
        stderr.contains("matches multiple queues"),
        "Expected disambiguation phrase in stderr; stderr={stderr}"
    );

    // Must name the matching queue in the message
    assert!(
        stderr.contains("Escalations"),
        "Expected matched queue 'Escalations' in stderr; stderr={stderr}"
    );
    // wiremock verifies .expect(1) on server drop — fires if list was called 0 or 2+ times
}

/// Single-substring hit on a queue name must route through Ambiguous and
/// error with the disambiguation message + UserError (exit 64 in the
/// binary). Complements the ExactMultiple coverage above and locks the
/// behavior at queue.rs:169 from the #193 strict-matching rollout.
#[tokio::test]
async fn resolve_queue_single_substring_is_ambiguous() {
    let server = MockServer::start().await;

    // "escal" is a single-substring of "Escalations" only — "General Requests"
    // shares no substring. The input is neither exact nor a multi-hit, which
    // is the exact scenario the #193 strict-matching rollout now routes
    // through Ambiguous.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .and(query_param("includeCount", "true"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "10", "name": "Escalations", "issueCount": 5 },
                { "id": "20", "name": "General Requests", "issueCount": 3 }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let result = jr::cli::queue::resolve_queue_by_name("15", "escal", &client).await;

    let err = result.unwrap_err();
    assert!(
        err.downcast_ref::<jr::error::JrError>()
            .is_some_and(|e| matches!(e, jr::error::JrError::UserError(_))),
        "Expected JrError::UserError, got: {err}"
    );

    let msg = err.to_string();
    assert!(
        msg.contains("matches multiple queues"),
        "Expected disambiguation phrase in error, got: {msg}"
    );
    assert!(
        msg.contains("Escalations"),
        "Expected matched queue 'Escalations' in error, got: {msg}"
    );
}

#[tokio::test]
async fn resolve_queue_mixed_case_duplicate_names_error_message() {
    let server = MockServer::start().await;

    // Two queues whose names differ only in casing — unlike the exact-duplicate
    // test above, this exercises the to_lowercase() normalization path
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .and(query_param("includeCount", "true"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "30", "name": "Triage", "issueCount": 5 },
                { "id": "40", "name": "TRIAGE", "issueCount": 3 }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    // Lowercase input — differs in casing from both stored names,
    // so to_lowercase() must normalize both input and candidates
    let result = jr::cli::queue::resolve_queue_by_name("15", "triage", &client).await;

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Multiple queues named \"Triage\""),
        "Expected queue name in error, got: {msg}"
    );
    assert!(
        msg.contains("30, 40"),
        "Expected both queue IDs in error, got: {msg}"
    );
    assert!(
        msg.contains("Use --id 30 to specify"),
        "Expected --id suggestion in error, got: {msg}"
    );
}

// ─── Error-path coverage (#187) ─────────────────────────────────────────────

/// Write a minimal jr config to a temp XDG_CONFIG_HOME so the subprocess
/// finds a URL while JR_BASE_URL / JR_AUTH_HEADER override the real values.
fn write_minimal_config(config_home: &std::path::Path, url: &str) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        format!("[instance]\nurl = \"{url}\"\n"),
    )
    .unwrap();
}

#[tokio::test]
async fn queue_list_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // Fail the FIRST call in the queue-list chain:
    // require_service_desk → get_or_fetch_project_meta → GET /rest/api/3/project/{key}
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
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
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args(["queue", "list", "--project", "PROJ"])
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
async fn queue_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
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
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args(["queue", "list", "--project", "PROJ"])
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

/// AC-010 (S-288-pr2-cli) + BC-X.8.004: verifies the queue caller of
/// `require_service_desk` produces the canonical capitalised plural-agreement
/// error message. Regression guard for the pre-adv-01 lowercase/singular-verb
/// drift ("queue commands requires" → "Queue commands (`jr queue`) require").
#[tokio::test]
async fn test_queue_list_non_jsm_project_emits_canonical_callsite_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // Project meta for "DEV" returns a software project — NOT service_desk.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "100",
            "key": "DEV",
            "projectTypeKey": "software",
            "simplified": true
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args(["queue", "list", "--project", "DEV", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for non-JSM project, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    // BC-X.8.004 prefix pin: "Project " prefix (C-1 fix mirrored here for queue).
    assert!(
        stderr.contains("Project \"DEV\" is a"),
        "BC-X.8.004: error must start with 'Project \"DEV\" is a'; got: {stderr}"
    );

    // BC-X.8.004 verbatim phrase — capitalised, plural noun, plural-agreement verb.
    assert!(
        stderr.contains("Queue commands (`jr queue`) require a Jira Service Management project"),
        "BC-X.8.004: stderr must contain the verbatim canonical phrase; got: {stderr}"
    );

    // C-2: BC-X.8.004 closing sentence — BC-verbatim "find a JSM project".
    assert!(
        stderr.contains("Run \"jr project list\" to find a JSM project."),
        "BC-X.8.004: closing must use 'find a JSM project'; got: {stderr}"
    );
    assert!(
        !stderr.contains("see available projects"),
        "Old drifted closing 'see available projects' must not appear; got: {stderr}"
    );

    // Regression guard: the pre-adv-01 lowercase/singular-verb form must never appear.
    assert!(
        !stderr.contains("queue commands requires"),
        "Regression: lowercase singular-verb form 'queue commands requires' must not appear; got: {stderr}"
    );
}

#[tokio::test]
async fn queue_list_network_drop_surfaces_reach_error() {
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), "http://127.0.0.1:1");

    // Privileged port 1 — connect-refused from any unprivileged process.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args(["queue", "list", "--project", "PROJ"])
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

// ─── S-693-1 (#693): `queue view` threads queue-declared customfield_*
// columns into search_issues extra_fields (BC-X.8.009 AMENDED) ────────────
//
// RED GATE — these tests pin AC-1 through AC-8 from
// .factory/stories/S-693-1-queue-view-custom-fields.md against BC-X.8.009.
// They are written to exercise the full CLI (subprocess) pipeline rather
// than a not-yet-existing pure helper, so the crate keeps compiling (and
// every pre-existing queue test above keeps passing) whether or not the
// story's implementation has landed yet.
//
// Test-fn naming intentionally uses `test_bc_x_8_009_...` (lowercase) per
// this repo's snake_case test-naming convention (CLAUDE.md), rather than
// the literal `test_BC_X_8_009_...` spelling used in the story's AC
// headings — the story names are shorthand, not a literal fn-name mandate,
// and lowercase avoids a `non_snake_case` warning under `-D warnings`.
//
// Fixed fixture identity used throughout: project key "HELPDESK", service
// desk id "15", queue id "10" (name "Triage") — mirrors the existing
// conventions already established elsewhere in this file.

/// Mounts the two prerequisite mocks `require_service_desk` needs before
/// `handle_view` is ever entered: the project-meta lookup (confirms a JSM
/// `service_desk` project) and the service-desk list (resolves the numeric
/// `service_desk_id` for the project).
async fn mount_jsm_prereqs(server: &MockServer, project_key: &str, service_desk_id: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/project/{project_key}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "10000",
            "key": project_key,
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": service_desk_id, "projectId": "10000", "projectName": "Service Desk" }
            ]
        })))
        .mount(server)
        .await;
}

/// Mounts `GET .../servicedesk/{sdId}/queue` (the `list_queues` endpoint —
/// used both by `resolve_queue_by_name` on the name path, and by the new
/// `--id`-path auxiliary lookup this story adds) returning a single queue
/// with the given `fields[]` declaration (`None` omits the `fields` key
/// entirely, matching a queue with no configured columns).
async fn mount_queue_list(
    server: &MockServer,
    service_desk_id: &str,
    queue_id: &str,
    queue_name: &str,
    fields: Option<Vec<&str>>,
) {
    let mut queue = json!({
        "id": queue_id,
        "name": queue_name,
        "issueCount": 1
    });
    if let Some(f) = fields {
        queue["fields"] = json!(f);
    }
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/servicedeskapi/servicedesk/{service_desk_id}/queue"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [queue]
        })))
        .mount(server)
        .await;
}

/// Mounts `GET .../servicedesk/{sdId}/queue/{queueId}/issue` (the
/// `get_queue_issue_keys` endpoint) returning the given keys in order.
async fn mount_queue_issue_keys(
    server: &MockServer,
    service_desk_id: &str,
    queue_id: &str,
    keys: &[&str],
) {
    let values: Vec<serde_json::Value> = keys
        .iter()
        .map(|k| json!({ "key": k, "fields": {} }))
        .collect();
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/servicedeskapi/servicedesk/{service_desk_id}/queue/{queue_id}/issue"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": keys.len(),
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": values
        })))
        .mount(server)
        .await;
}

/// Mounts `POST /rest/api/3/search/jql` returning a single issue carrying
/// `field_id: value` in addition to the standard base fields.
async fn mount_search_issues_with_customfield(
    server: &MockServer,
    key: &str,
    field_id: &str,
    value: serde_json::Value,
) {
    let mut issue = json!({
        "key": key,
        "fields": {
            "summary": "Test issue",
            "status": {"name": "New"},
            "issuetype": {"name": "Task"},
            "priority": {"name": "Medium"},
            "assignee": serde_json::Value::Null
        }
    });
    issue["fields"][field_id] = value;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [issue],
            "nextPageToken": serde_json::Value::Null
        })))
        .mount(server)
        .await;
}

/// Mounts `POST /rest/api/3/search/jql` returning a single issue with only
/// the standard base fields (no custom field) — used for the "nothing
/// matches the allow-list" scenarios.
async fn mount_search_issues_plain(server: &MockServer, key: &str) {
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [{
                "key": key,
                "fields": {
                    "summary": "Test issue",
                    "status": {"name": "New"},
                    "issuetype": {"name": "Task"},
                    "priority": {"name": "Medium"},
                    "assignee": serde_json::Value::Null
                }
            }],
            "nextPageToken": serde_json::Value::Null
        })))
        .mount(server)
        .await;
}

/// Runs `jr queue view --project HELPDESK --no-input <extra_args>` against
/// `server_uri`, with the given XDG cache/config temp dirs.
fn run_jr_queue_view(
    server_uri: &str,
    cache_dir: &std::path::Path,
    config_dir: &std::path::Path,
    extra_args: &[&str],
) -> std::process::Output {
    write_minimal_config(config_dir, server_uri);
    let mut args: Vec<&str> = vec!["queue", "view", "--project", "HELPDESK", "--no-input"];
    args.extend_from_slice(extra_args);
    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("JR_CACHE_DIR", cache_dir.join("jr"))
        .env("XDG_CONFIG_HOME", config_dir)
        .env("JR_CONFIG_DIR", config_dir.join("jr"))
        .args(args)
        .output()
        .unwrap()
}

/// Finds the (first) captured `POST /rest/api/3/search/jql` request and
/// returns its `fields` array as owned strings.
async fn captured_search_fields(server: &MockServer) -> Vec<String> {
    let reqs = server
        .received_requests()
        .await
        .expect("wiremock must record requests");
    let search_req = reqs
        .iter()
        .find(|r| {
            r.method == wiremock::http::Method::POST && r.url.path() == "/rest/api/3/search/jql"
        })
        .expect("must have hit /rest/api/3/search/jql");
    let body: serde_json::Value =
        serde_json::from_slice(&search_req.body).expect("search body must be valid JSON");
    body["fields"]
        .as_array()
        .expect("fields must be an array")
        .iter()
        .map(|v| {
            v.as_str()
                .expect("field entries must be strings")
                .to_string()
        })
        .collect()
}

/// Counts captured requests matching an exact method + path.
fn count_requests_to(reqs: &[wiremock::Request], m: wiremock::http::Method, p: &str) -> usize {
    reqs.iter()
        .filter(|r| r.method == m && r.url.path() == p)
        .count()
}

/// AC-1 (BC-X.8.009 Issue fetch pipeline step 3, happy path): name-path
/// custom fields surface in JSON, with zero additional `list_queues` calls
/// beyond the one `resolve_queue_by_name` already makes.
#[tokio::test]
async fn test_bc_x_8_009_queue_view_name_path_surfaces_declared_customfield_in_json() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_jsm_prereqs(&server, "HELPDESK", "15").await;
    mount_queue_list(
        &server,
        "15",
        "10",
        "Triage",
        Some(vec!["issuekey", "summary", "customfield_10050"]),
    )
    .await;
    mount_queue_issue_keys(&server, "15", "10", &["HELPDESK-42"]).await;
    mount_search_issues_with_customfield(
        &server,
        "HELPDESK-42",
        "customfield_10050",
        json!("Acme Corp"),
    )
    .await;

    let output = run_jr_queue_view(
        &server.uri(),
        cache_dir.path(),
        config_dir.path(),
        &["Triage", "--output", "json"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");

    // Primary pin: the search request's `fields` must include the queue's
    // declared customfield_10050 token.
    let fields = captured_search_fields(&server).await;
    assert!(
        fields.iter().any(|f| f == "customfield_10050"),
        "AC-1: search request `fields` must include queue-declared customfield_10050; got: {fields:?}"
    );

    // --output json must surface the custom field value.
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let issues = stdout.as_array().expect("json output must be an array");
    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0]["fields"]["customfield_10050"],
        json!("Acme Corp"),
        "AC-1: customfield_10050 must surface in the JSON `fields` object; got: {}",
        issues[0]["fields"]
    );

    // Name path incurs no additional list_queues call beyond the one
    // resolve_queue_by_name already makes.
    let reqs = server.received_requests().await.unwrap();
    let queue_list_calls = count_requests_to(
        &reqs,
        wiremock::http::Method::GET,
        "/rest/servicedeskapi/servicedesk/15/queue",
    );
    assert_eq!(
        queue_list_calls, 1,
        "AC-1: name path must make exactly ONE list_queues call (no extra aux lookup); got {queue_list_calls}"
    );
}

/// AC-2 (BC-X.8.009 Queue ID resolution item 1, cost asymmetry): the `--id`
/// path incurs the `list_queues` endpoint exactly once (the new auxiliary
/// lookup this story adds — pre-story the `--id` path never calls this
/// endpoint at all), the same absolute count as the `<name>` path's
/// pre-existing resolution call, but for a different reason.
#[tokio::test]
async fn test_bc_x_8_009_queue_view_id_path_incurs_one_additional_list_queues_call() {
    // --- Name path: baseline is exactly ONE list_queues call (via resolve_queue_by_name) ---
    let name_server = MockServer::start().await;
    let name_cache = tempfile::tempdir().unwrap();
    let name_config = tempfile::tempdir().unwrap();
    mount_jsm_prereqs(&name_server, "HELPDESK", "15").await;
    mount_queue_list(
        &name_server,
        "15",
        "10",
        "Triage",
        Some(vec!["customfield_10050"]),
    )
    .await;
    mount_queue_issue_keys(&name_server, "15", "10", &["HELPDESK-42"]).await;
    mount_search_issues_with_customfield(
        &name_server,
        "HELPDESK-42",
        "customfield_10050",
        json!("v"),
    )
    .await;

    let name_output = run_jr_queue_view(
        &name_server.uri(),
        name_cache.path(),
        name_config.path(),
        &["Triage", "--output", "json"],
    );
    assert!(
        name_output.status.success(),
        "name path must exit 0; stderr={}",
        String::from_utf8_lossy(&name_output.stderr)
    );

    let name_reqs = name_server.received_requests().await.unwrap();
    let name_calls = count_requests_to(
        &name_reqs,
        wiremock::http::Method::GET,
        "/rest/servicedeskapi/servicedesk/15/queue",
    );
    assert_eq!(
        name_calls, 1,
        "name path baseline must be exactly 1 list_queues call; got {name_calls}"
    );

    // --- --id path: incurs the SAME endpoint exactly once — the aux lookup
    // this story adds — since --id bypasses resolve_queue_by_name entirely
    // and therefore has no OTHER reason to call list_queues. Pre-story, this
    // count would be 0 (no aux lookup exists yet), so asserting 1 here is
    // the AC-2 pin.
    let id_server = MockServer::start().await;
    let id_cache = tempfile::tempdir().unwrap();
    let id_config = tempfile::tempdir().unwrap();
    mount_jsm_prereqs(&id_server, "HELPDESK", "15").await;
    mount_queue_list(
        &id_server,
        "15",
        "10",
        "Triage",
        Some(vec!["customfield_10050"]),
    )
    .await;
    mount_queue_issue_keys(&id_server, "15", "10", &["HELPDESK-42"]).await;
    mount_search_issues_with_customfield(
        &id_server,
        "HELPDESK-42",
        "customfield_10050",
        json!("v"),
    )
    .await;

    let id_output = run_jr_queue_view(
        &id_server.uri(),
        id_cache.path(),
        id_config.path(),
        &["--id", "10", "--output", "json"],
    );
    assert!(
        id_output.status.success(),
        "id path must exit 0; stderr={}",
        String::from_utf8_lossy(&id_output.stderr)
    );

    // S5 (pr-reviewer suggestion): the --id HAPPY path (aux lookup succeeds,
    // queue declares a customfield) must surface it in --output json too —
    // AC-1 only pins this for the name path.
    let id_stdout: serde_json::Value = serde_json::from_slice(&id_output.stdout).unwrap();
    let id_issues = id_stdout.as_array().expect("json output must be an array");
    assert_eq!(id_issues.len(), 1);
    assert_eq!(
        id_issues[0]["fields"]["customfield_10050"],
        json!("v"),
        "id path happy case: customfield_10050 must surface in the JSON `fields` \
         object same as the name path; got: {}",
        id_issues[0]["fields"]
    );

    let id_reqs = id_server.received_requests().await.unwrap();
    let id_calls = count_requests_to(
        &id_reqs,
        wiremock::http::Method::GET,
        "/rest/servicedeskapi/servicedesk/15/queue",
    );
    assert_eq!(
        id_calls, 1,
        "AC-2: --id path must incur exactly ONE list_queues call (the new aux \
         lookup for queue.fields); pre-story this endpoint is never called on \
         the --id path at all (0), so this pins the 0→1 cost asymmetry; got {id_calls}"
    );
}

/// AC-3 (BC-X.8.009 EC-X.8.009-1, MEDIUM-3/LOW-1 fail-open degrade): the
/// `--id` path's auxiliary `list_queues` lookup failing (5xx) OR succeeding
/// with no id match must degrade to `extra_fields = &[]`, exit 0, and emit
/// the canonical stderr warning — never hard-fail.
#[tokio::test]
async fn test_bc_x_8_009_queue_view_id_path_aux_lookup_failure_degrades_with_warning_exit_0() {
    // --- Sub-case (a) / EC-3: aux list_queues lookup 5xxs ---
    let server_a = MockServer::start().await;
    let cache_a = tempfile::tempdir().unwrap();
    let config_a = tempfile::tempdir().unwrap();
    mount_jsm_prereqs(&server_a, "HELPDESK", "15").await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(json!({"errorMessages": ["boom"], "errors": {}})),
        )
        .mount(&server_a)
        .await;
    mount_queue_issue_keys(&server_a, "15", "999", &["HELPDESK-1"]).await;
    mount_search_issues_with_customfield(
        &server_a,
        "HELPDESK-1",
        "customfield_10050",
        json!("unused"),
    )
    .await;

    let output_a = run_jr_queue_view(
        &server_a.uri(),
        cache_a.path(),
        config_a.path(),
        &["--id", "999", "--output", "json"],
    );
    let stderr_a = String::from_utf8_lossy(&output_a.stderr);

    assert!(
        output_a.status.success(),
        "AC-3(a)/EC-3: a failed aux lookup must NOT hard-fail the command; stderr={stderr_a}"
    );
    assert!(
        stderr_a.contains(
            "warning: could not fetch queue field configuration for --id 999 (API error (500)); showing base fields only."
        ),
        "AC-3(a)/EC-3: expected canonical degrade warning in stderr; got: {stderr_a}"
    );
    let stdout_a: serde_json::Value = serde_json::from_slice(&output_a.stdout).unwrap();
    assert!(
        stdout_a[0]["fields"].get("customfield_10050").is_none(),
        "AC-3(a): degraded --id path must show base fields only, no custom field; got: {}",
        stdout_a[0]["fields"]
    );

    // --- Sub-case (b) / EC-4: aux lookup succeeds (HTTP 200) but no entry's
    // id matches the requested --id ---
    let server_b = MockServer::start().await;
    let cache_b = tempfile::tempdir().unwrap();
    let config_b = tempfile::tempdir().unwrap();
    mount_jsm_prereqs(&server_b, "HELPDESK", "15").await;
    mount_queue_list(
        &server_b,
        "15",
        "10",
        "Triage",
        Some(vec!["customfield_10050"]),
    )
    .await; // id "10", not "999"
    mount_queue_issue_keys(&server_b, "15", "999", &["HELPDESK-2"]).await;
    mount_search_issues_with_customfield(
        &server_b,
        "HELPDESK-2",
        "customfield_10050",
        json!("unused"),
    )
    .await;

    let output_b = run_jr_queue_view(
        &server_b.uri(),
        cache_b.path(),
        config_b.path(),
        &["--id", "999", "--output", "json"],
    );
    let stderr_b = String::from_utf8_lossy(&output_b.stderr);

    assert!(
        output_b.status.success(),
        "AC-3(b)/EC-4: a no-id-match aux lookup must NOT hard-fail the command; stderr={stderr_b}"
    );
    assert!(
        stderr_b.contains(
            "warning: could not fetch queue field configuration for --id 999 (no matching queue); showing base fields only."
        ),
        "AC-3(b)/EC-4: expected canonical degrade warning in stderr; got: {stderr_b}"
    );
    let stdout_b: serde_json::Value = serde_json::from_slice(&output_b.stdout).unwrap();
    assert!(
        stdout_b[0]["fields"].get("customfield_10050").is_none(),
        "AC-3(b): degraded --id path must show base fields only, no custom field; got: {}",
        stdout_b[0]["fields"]
    );
}

/// AC-4 (BC-X.8.009 Issue fetch pipeline step 3, allow-list pin): only
/// `^customfield_\d+$`-shaped tokens are kept; pseudo-columns, base fields,
/// and malformed near-misses are all dropped.
#[tokio::test]
async fn test_bc_x_8_009_extra_fields_allow_list_rejects_non_matching_tokens() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_jsm_prereqs(&server, "HELPDESK", "15").await;
    mount_queue_list(
        &server,
        "15",
        "10",
        "Triage",
        Some(vec![
            "issuekey",
            "summary",
            "status",
            "customfield_10050",
            "customfield_",
            "customfield_10050_x",
            "Customfield_99",
        ]),
    )
    .await;
    mount_queue_issue_keys(&server, "15", "10", &["HELPDESK-42"]).await;
    mount_search_issues_with_customfield(
        &server,
        "HELPDESK-42",
        "customfield_10050",
        json!("kept"),
    )
    .await;

    let output = run_jr_queue_view(
        &server.uri(),
        cache_dir.path(),
        config_dir.path(),
        &["Triage", "--output", "json"],
    );
    assert!(
        output.status.success(),
        "expected exit 0; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let fields = captured_search_fields(&server).await;

    assert_eq!(
        fields.iter().filter(|f| *f == "customfield_10050").count(),
        1,
        "AC-4: customfield_10050 must be kept exactly once; got: {fields:?}"
    );
    for rejected in [
        "customfield_",
        "customfield_10050_x",
        "Customfield_99",
        "issuekey",
    ] {
        assert!(
            !fields.iter().any(|f| f == rejected),
            "AC-4: '{rejected}' must be dropped by the allow-list, never sent to search_issues; got: {fields:?}"
        );
    }
    // BASE_ISSUE_FIELDS members declared by the queue (summary/status) must
    // not be duplicated by the extra_fields pass — they're already in the
    // base set requested unconditionally.
    assert_eq!(
        fields.iter().filter(|f| *f == "summary").count(),
        1,
        "AC-4: 'summary' must appear exactly once (base field, not re-added); got: {fields:?}"
    );
    assert_eq!(
        fields.iter().filter(|f| *f == "status").count(),
        1,
        "AC-4: 'status' must appear exactly once (base field, not re-added); got: {fields:?}"
    );
}

/// AC-5 (BC-X.8.009 EC-X.8.009-2): a queue whose declared `fields[]`
/// entirely fails the allow-list produces `extra_fields = &[]`,
/// byte-identical to a queue with `fields: null`.
#[tokio::test]
async fn test_bc_x_8_009_extra_fields_all_filtered_out_yields_empty_slice_no_regression() {
    const BASE_ISSUE_FIELDS: &[&str] = &[
        "summary",
        "status",
        "issuetype",
        "priority",
        "assignee",
        "reporter",
        "project",
        "description",
        "created",
        "updated",
        "duedate",
        "resolution",
        "components",
        "fixVersions",
        "labels",
        "parent",
        "issuelinks",
    ];

    // --- Scenario A: queue declares fields, none match the allow-list ---
    let server_a = MockServer::start().await;
    let cache_a = tempfile::tempdir().unwrap();
    let config_a = tempfile::tempdir().unwrap();
    mount_jsm_prereqs(&server_a, "HELPDESK", "15").await;
    mount_queue_list(
        &server_a,
        "15",
        "10",
        "Triage",
        Some(vec!["issuekey", "summary", "status"]),
    )
    .await;
    mount_queue_issue_keys(&server_a, "15", "10", &["HELPDESK-42"]).await;
    mount_search_issues_plain(&server_a, "HELPDESK-42").await;

    let output_a = run_jr_queue_view(
        &server_a.uri(),
        cache_a.path(),
        config_a.path(),
        &["Triage", "--output", "json"],
    );
    assert!(
        output_a.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output_a.stderr)
    );
    let fields_a = captured_search_fields(&server_a).await;
    assert_eq!(
        fields_a, BASE_ISSUE_FIELDS,
        "AC-5: no allow-list matches → fields must be BYTE-IDENTICAL to BASE_ISSUE_FIELDS \
         (empty extra_fields); got: {fields_a:?}"
    );

    // --- Scenario B: queue declares fields: null (baseline, pre-#693 identical) ---
    let server_b = MockServer::start().await;
    let cache_b = tempfile::tempdir().unwrap();
    let config_b = tempfile::tempdir().unwrap();
    mount_jsm_prereqs(&server_b, "HELPDESK", "15").await;
    mount_queue_list(&server_b, "15", "10", "Triage", None).await;
    mount_queue_issue_keys(&server_b, "15", "10", &["HELPDESK-42"]).await;
    mount_search_issues_plain(&server_b, "HELPDESK-42").await;

    let output_b = run_jr_queue_view(
        &server_b.uri(),
        cache_b.path(),
        config_b.path(),
        &["Triage", "--output", "json"],
    );
    assert!(
        output_b.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output_b.stderr)
    );
    let fields_b = captured_search_fields(&server_b).await;

    assert_eq!(
        fields_a, fields_b,
        "AC-5/EC-8: fields:[non-matching] and fields:null must produce byte-identical \
         search requests; got a={fields_a:?} b={fields_b:?}"
    );
}

/// AC-6 (BC-X.8.009 Output/Table-output clause, regression pin): table
/// output is unaffected by queue-configured custom fields — no new column,
/// byte-identical headers to pre-#693.
#[tokio::test]
async fn test_bc_x_8_009_queue_view_table_output_unaffected_by_custom_field_extra_fields() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_jsm_prereqs(&server, "HELPDESK", "15").await;
    mount_queue_list(
        &server,
        "15",
        "10",
        "Triage",
        Some(vec!["customfield_10050"]),
    )
    .await;
    mount_queue_issue_keys(&server, "15", "10", &["HELPDESK-42"]).await;
    mount_search_issues_with_customfield(
        &server,
        "HELPDESK-42",
        "customfield_10050",
        json!("Acme"),
    )
    .await;

    // Default output (table) — no --output flag.
    let output = run_jr_queue_view(
        &server.uri(),
        cache_dir.path(),
        config_dir.path(),
        &["Triage"],
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("Key")
            && stdout.contains("Type")
            && stdout.contains("Status")
            && stdout.contains("Priority")
            && stdout.contains("Assignee")
            && stdout.contains("Summary"),
        "AC-6: expected standard 6-column headers (Key/Type/Status/Priority/Assignee/Summary); \
         got: {stdout}"
    );
    assert!(
        !stdout.contains("customfield_10050") && !stdout.contains("Acme"),
        "AC-6: table output must NOT surface the custom field id or value \
         (render-side work tracked separately as #575); got: {stdout}"
    );
}

/// AC-7 (BC-X.8.009 Issue fetch pipeline item 2, regression pin): a
/// zero-issue queue short-circuits before any `search_issues` call, and
/// before any aux `list_queues` lookup — nothing to fetch fields for.
#[tokio::test]
async fn test_bc_x_8_009_queue_view_zero_issues_short_circuits_no_extra_fields_lookup() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_jsm_prereqs(&server, "HELPDESK", "15").await;
    // Deliberately do NOT mount GET .../servicedesk/15/queue (list_queues) or
    // POST /rest/api/3/search/jql — if handle_view attempts either for a
    // zero-issue --id-path queue, the request hits an unmounted route (404)
    // and the command exits non-zero, failing the assertions below.
    mount_queue_issue_keys(&server, "15", "999", &[]).await;

    let output = run_jr_queue_view(
        &server.uri(),
        cache_dir.path(),
        config_dir.path(),
        &["--id", "999", "--output", "json"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-7: zero-issue queue must exit 0; stderr={stderr}"
    );

    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        stdout,
        json!([]),
        "AC-7: zero-issue queue must produce an empty JSON array; got: {stdout}"
    );

    let reqs = server.received_requests().await.unwrap();
    let list_calls = count_requests_to(
        &reqs,
        wiremock::http::Method::GET,
        "/rest/servicedeskapi/servicedesk/15/queue",
    );
    assert_eq!(
        list_calls, 0,
        "AC-7: zero-issue queue must NOT attempt the aux list_queues lookup; got {list_calls}"
    );
    let search_calls = count_requests_to(
        &reqs,
        wiremock::http::Method::POST,
        "/rest/api/3/search/jql",
    );
    assert_eq!(
        search_calls, 0,
        "AC-7: zero-issue queue must NOT call search_issues; got {search_calls}"
    );
}

/// AC-8 (BC-X.8.009 Errors clause, MEDIUM-3 scope note): a REAL failure of
/// the primary pipeline (`search_issues` itself returning 401) is NOT
/// degraded by the story's fail-open scope — it surfaces via the ordinary
/// Errors clause exactly as before #693, and DOES affect the exit code.
#[tokio::test]
async fn test_bc_x_8_009_primary_pipeline_failure_still_hard_fails_unaffected_by_aux_lookup_scope()
{
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_jsm_prereqs(&server, "HELPDESK", "15").await;
    mount_queue_list(
        &server,
        "15",
        "10",
        "Triage",
        Some(vec!["customfield_10050"]),
    )
    .await;
    mount_queue_issue_keys(&server, "15", "10", &["HELPDESK-42"]).await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = run_jr_queue_view(
        &server.uri(),
        cache_dir.path(),
        config_dir.path(),
        &["Triage", "--output", "json"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(2),
        "AC-8: a real search_issues 401 must exit 2 (ordinary Errors clause), unaffected by \
         aux-lookup scope; stderr={stderr}"
    );
    assert!(
        stderr.contains("Not authenticated"),
        "AC-8: expected 'Not authenticated' in stderr; got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "AC-8: expected 'jr auth login' suggestion in stderr; got: {stderr}"
    );
}

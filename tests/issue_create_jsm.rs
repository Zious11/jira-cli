//! Integration tests for `jr issue create --request-type` dispatch fork.
//!
//! Covers AC-001..AC-015 from story S-288-pr4-dispatch
//! (`.factory/code-delivery/issue-288-pr4-dispatch/story.md`).
//!
//! All HTTP tests use subprocess + wiremock + assert_cmd, matching the pattern
//! established in `tests/requesttype_commands.rs`. Each test runs the `jr`
//! binary via `assert_cmd::Command::cargo_bin("jr")` with:
//!   `JR_BASE_URL=<wiremock url>` `JR_AUTH_HEADER=Basic dGVzdDp0ZXN0`
//!
//! AC-016 (OAuth scope pin) lives in `src/cli/auth/tests/mod.rs`.
//! AC-013 proptest properties live in `src/cli/issue/create.rs::mod parse_field_kv_proptests`.
//! AC-014 proptest properties live in `src/api/jsm/requests.rs::mod proptests`.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{body_partial_json, method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ─── Shared mock fixture helpers ──────────────────────────────────────────────

/// Mount project-meta GET for project "HELP" returning a service_desk project.
/// The project_id "99" is matched by the service desk list mock below.
async fn mount_project_meta_help(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELP"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "99",
            "key": "HELP",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .mount(server)
        .await;
}

/// Mount project-meta GET for project "SW" returning a software project.
async fn mount_project_meta_sw_software(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/SW"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "100",
            "key": "SW",
            "projectTypeKey": "software",
            "simplified": false
        })))
        .mount(server)
        .await;
}

/// Mount the service desk list GET, returning service desk id "10" for project id "99".
async fn mount_service_desk_list(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "_links": {},
            "values": [
                {
                    "id": "10",
                    "projectId": "99",
                    "projectKey": "HELP",
                    "projectName": "Help Desk"
                }
            ]
        })))
        .mount(server)
        .await;
}

/// Two request types used across multiple tests.
fn two_request_types_body() -> Value {
    json!({
        "size": 2,
        "start": 0,
        "limit": 50,
        "isLastPage": true,
        "_links": {},
        "values": [
            {
                "id": "11001",
                "name": "Get IT Help",
                "description": "Get IT help for hardware, software, or other issues",
                "helpText": "Please describe the issue in detail",
                "issueTypeId": "12345",
                "serviceDeskId": "10",
                "portalId": "2",
                "groupIds": ["12"]
            },
            {
                "id": "11002",
                "name": "Password Reset",
                "description": "Reset your password",
                "helpText": "Provide your username",
                "issueTypeId": "12346",
                "serviceDeskId": "10",
                "portalId": "2",
                "groupIds": ["12", "13"]
            }
        ]
    })
}

/// Mount the request type list for service desk 10.
async fn mount_request_type_list(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(two_request_types_body()))
        .mount(server)
        .await;
}

/// Mount the request type list for service desk 10 with a single "Password Reset" type.
///
/// Used by tests that only need one type to avoid ambiguous-match complications.
async fn mount_request_types_password_reset(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "_links": {},
            "values": [
                {
                    "id": "11002",
                    "name": "Password Reset",
                    "description": "Reset your password",
                    "helpText": "Provide your username",
                    "issueTypeId": "12346",
                    "serviceDeskId": "10",
                    "portalId": "2",
                    "groupIds": ["12", "13"]
                }
            ]
        })))
        .mount(server)
        .await;
}

/// Successful JSM create response for "HELP-42".
fn jsm_created_response() -> Value {
    json!({
        "issueId": "107001",
        "issueKey": "HELP-42",
        "requestTypeId": "11002",
        "serviceDeskId": "10",
        "_links": {
            "self": "https://example.atlassian.net/rest/servicedeskapi/request/107001",
            "web": "https://example.atlassian.net/servicedesk/customer/portal/10/HELP-42"
        }
    })
}

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

// ─── AC-001: dispatch routes to servicedeskapi, NOT platform endpoint ─────────

/// AC-001 (BC-3.8.001, H-NEW-JSM-RT-001): `jr issue create --request-type` fires
/// exactly ONE POST to `/rest/servicedeskapi/request` and ZERO POSTs to
/// `/rest/api/3/issue`. Output contains the issue key; exit 0.
///
/// The `expect(0)` on the platform endpoint is the holdout-H-NEW-JSM-RT-001
/// regression guard.
#[tokio::test]
async fn test_jsm_create_happy_path_routes_to_servicedeskapi() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    // CRITICAL: JSM endpoint must be called exactly once.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
        .mount(&server)
        .await;

    // CRITICAL: Platform endpoint must NEVER be called (H-NEW-JSM-RT-001 guard).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "My issue",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.001: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    // Assert issue key appears in output.
    assert!(
        stdout.contains("HELP-42"),
        "BC-3.8.001: expected issue key 'HELP-42' in output, got: {stdout}"
    );
    // The .expect(0) on the platform mock is enforced automatically by wiremock on server drop.
}

// ─── AC-002: platform path unchanged when --request-type absent ───────────────

/// AC-002 (BC-3.3.001): Without `--request-type`, platform POST fires exactly
/// once and the servicedeskapi POST is never called. Regression guard for
/// the dispatch-fork conditionality.
#[tokio::test]
async fn test_jsm_create_without_request_type_uses_platform_path() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // Platform endpoint must be called exactly once.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    // JSM endpoint must NEVER be called (regression guard for BC-3.3.001).
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    // GET /rest/api/3/field — for CMDB discovery on the platform path.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
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
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Platform issue",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "BC-3.3.001: expected exit 0 on platform path, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("PROJ-123"),
        "BC-3.3.001: platform create must emit issue key 'PROJ-123'; got stdout: {stdout}, stderr: {stderr}"
    );
    // The .expect(0) on the servicedeskapi mock is enforced on server drop.
}

// ─── AC-003: non-JSM project exits 64, zero HTTP POST ────────────────────────

/// AC-003 (BC-3.8.002, H-NEW-JSM-RT-002): `--request-type` on a software project
/// exits 64 with a verbatim BC-mandated message. ZERO POSTs to either endpoint.
#[tokio::test]
async fn test_jsm_create_non_jsm_project_exits_64_zero_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_sw_software(&server).await;

    // Neither endpoint should receive a POST.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "SW",
            "--request-type",
            "Bug Report",
            "--summary",
            "test",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.002 / H-NEW-JSM-RT-002: expected exit 64 for non-JSM project, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    // BC-3.8.002: verbatim error phrase with call-site label "`jr issue create --request-type`".
    assert!(
        stderr.contains(
            "`jr issue create --request-type` requires a Jira Service Management project"
        ),
        "BC-3.8.002: stderr must contain verbatim BC phrase with call-site label; got: {stderr}"
    );
}

// ─── AC-004: ambiguous request-type exits 64 with hint ───────────────────────

/// AC-004 (BC-3.8.003): When `--request-type "Bug"` matches two request types,
/// exits 64 with "Ambiguous request type" + candidate names + actionable hint.
#[tokio::test]
async fn test_jsm_create_ambiguous_request_type_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    // Two request types both containing "Bug".
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "_links": {},
            "values": [
                {
                    "id": "11001",
                    "name": "Bug Report",
                    "description": "Report a bug",
                    "helpText": null,
                    "issueTypeId": "12345",
                    "serviceDeskId": "10",
                    "portalId": "2",
                    "groupIds": []
                },
                {
                    "id": "11002",
                    "name": "Bug Fix Request",
                    "description": "Request a bug fix",
                    "helpText": null,
                    "issueTypeId": "12346",
                    "serviceDeskId": "10",
                    "portalId": "2",
                    "groupIds": []
                }
            ]
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Bug",
            "--summary",
            "test",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.003: expected exit 64 for ambiguous request type, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    // BC-3.8.003: verbatim prefix.
    assert!(
        stderr.contains("Ambiguous request type \"Bug\" matches:"),
        "BC-3.8.003: stderr must contain 'Ambiguous request type \"Bug\" matches:'; got: {stderr}"
    );
    // Both candidate names must appear.
    assert!(
        stderr.contains("Bug Report"),
        "BC-3.8.003: stderr must list candidate 'Bug Report'; got: {stderr}"
    );
    assert!(
        stderr.contains("Bug Fix Request"),
        "BC-3.8.003: stderr must list candidate 'Bug Fix Request'; got: {stderr}"
    );
    // Actionable hint with verbatim command form.
    assert!(
        stderr.contains("Run `jr requesttype list --project HELP`"),
        "BC-3.8.003: hint must use 'Run `jr requesttype list --project HELP`'; got: {stderr}"
    );
    assert!(
        stderr.contains("to see all request types"),
        "BC-3.8.003: hint must end with 'to see all request types'; got: {stderr}"
    );
    // Negative: old drift forms must not appear.
    assert!(
        !stderr.contains("to see available types") && !stderr.contains("to see current types"),
        "Old drift wording must not appear; got: {stderr}"
    );
}

// ─── AC-005: numeric request-type ID bypasses name resolution ────────────────

/// AC-005 (BC-3.8.004): When `--request-type` is all-digits, the handler uses
/// it directly as `requestTypeId` without calling the request-type list endpoint.
/// The list endpoint mock has `expect(0)` as the regression guard.
#[tokio::test]
async fn test_jsm_create_numeric_id_bypasses_name_lookup() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    // List endpoint MUST NOT be called for a numeric ID.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .respond_with(ResponseTemplate::new(200).set_body_json(two_request_types_body()))
        .expect(0)
        .mount(&server)
        .await;

    // JSM create endpoint must be called exactly once with the numeric request type ID.
    // M-02a (adversary pass-03): pin top-level serviceDeskId and requestTypeId in the
    // POST body — they must NOT be inside requestFieldValues (BC-3.8.001).
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .and(body_partial_json(json!({
            "serviceDeskId": "10",       // top-level, NOT in requestFieldValues
            "requestTypeId": "11002",    // top-level, NOT in requestFieldValues (the literal --request-type arg)
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "issueId": "107002",
            "issueKey": "HELP-55",
            "requestTypeId": "11002",
            "serviceDeskId": "10",
            "_links": {}
        })))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "11002",
            "--summary",
            "test numeric id",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.004: expected exit 0 for numeric ID bypass, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("HELP-55"),
        "BC-3.8.004: expected issue key in output, got: {stdout}"
    );
    // The .expect(0) on the list mock is enforced on server drop.
}

// ─── AC-006: summary required in requestFieldValues ──────────────────────────

/// AC-006 (BC-3.8.005): The POST body to `/rest/servicedeskapi/request` must
/// contain `requestFieldValues.summary` equal to the `--summary` flag value.
#[tokio::test]
async fn test_jsm_create_summary_in_requestfieldvalues() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    // Mount JSM create and capture request body via received_requests.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "Reset my password",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.005: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    // Verify the POST body contained requestFieldValues.summary via received_requests.
    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("BC-3.8.005: JSM POST must have been made");

    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("BC-3.8.005: POST body must be valid JSON");

    assert_eq!(
        body["requestFieldValues"]["summary"].as_str(),
        Some("Reset my password"),
        "BC-3.8.005: requestFieldValues.summary must equal --summary value; got body: {body}"
    );
}

// ─── AC-007: description → ADF with isAdfRequest: true ───────────────────────

/// AC-007 (BC-3.8.006): With `--description`, the POST body contains
/// `isAdfRequest: true` and `requestFieldValues.description` is a JSON object
/// (ADF root node, NOT a bare string).
#[tokio::test]
async fn test_jsm_create_description_is_adf_with_is_adf_request_true() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--description",
            "**Bold** text",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.006: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("BC-3.8.006: JSM POST must have been made");

    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("BC-3.8.006: POST body must be valid JSON");

    // BC-3.8.006: isAdfRequest must be true when description is set.
    assert_eq!(
        body.get("isAdfRequest").and_then(Value::as_bool),
        Some(true),
        "BC-3.8.006: isAdfRequest must be true when description is set; got body: {body}"
    );

    // BC-3.8.006: requestFieldValues.description must be a JSON object (ADF root node).
    let desc = body
        .get("requestFieldValues")
        .and_then(|rfv| rfv.get("description"));
    assert!(
        desc.map(|d| d.is_object()).unwrap_or(false),
        "BC-3.8.006: requestFieldValues.description must be a JSON object (ADF root), not a bare string; got: {:?}",
        desc
    );

    // BC-3.8.006: ADF root MUST be `{"type":"doc","version":N,"content":[...]}`.
    // Pin both required keys strictly to catch any ADF-shape drift.
    let desc_obj = desc.unwrap();
    assert_eq!(
        desc_obj.get("type").and_then(Value::as_str),
        Some("doc"),
        "BC-3.8.006: ADF root type must be \"doc\"; got: {desc_obj}"
    );
    assert!(
        desc_obj
            .get("content")
            .map(Value::is_array)
            .unwrap_or(false),
        "BC-3.8.006: ADF root content must be an array; got: {desc_obj}"
    );
}

/// AC-007 sibling (BC-3.8.006): Without `--description`, the POST body does NOT
/// contain `requestFieldValues.description` and does NOT contain `isAdfRequest: true`.
#[tokio::test]
async fn test_jsm_create_plain_description_absent_when_no_description_flag() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test no description",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.006: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("BC-3.8.006: JSM POST must have been made");

    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("BC-3.8.006: POST body must be valid JSON");

    // BC-3.8.006: isAdfRequest must be absent or false when description is absent.
    let is_adf = body
        .get("isAdfRequest")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    assert!(
        !is_adf,
        "BC-3.8.006: isAdfRequest must be absent or false when --description not set; got body: {body}"
    );

    // BC-3.8.006: description key must be absent from requestFieldValues.
    let rfv_desc = body
        .get("requestFieldValues")
        .and_then(|rfv| rfv.get("description"));
    assert!(
        rfv_desc.is_none(),
        "BC-3.8.006: requestFieldValues.description must be absent when --description not set; got: {:?}",
        rfv_desc
    );
}

// ─── AC-008: priority and labels in requestFieldValues ───────────────────────

/// AC-008 (BC-3.8.007): `--priority High` → `requestFieldValues.priority = {"name": "High"}`.
/// `--label alpha --label beta` → `requestFieldValues.labels = ["alpha", "beta"]`
/// (plain string array, NOT object array).
#[tokio::test]
async fn test_jsm_create_priority_and_labels_mapped() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--priority",
            "High",
            "--label",
            "alpha",
            "--label",
            "beta",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.007: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("BC-3.8.007: JSM POST must have been made");

    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("BC-3.8.007: POST body must be valid JSON");

    let rfv = body
        .get("requestFieldValues")
        .expect("BC-3.8.007: requestFieldValues must be present");

    // BC-3.8.007: priority must be {"name": "High"}.
    assert_eq!(
        rfv.get("priority")
            .and_then(|p| p.get("name"))
            .and_then(Value::as_str),
        Some("High"),
        "BC-3.8.007: priority must be {{\"name\": \"High\"}}; got rfv: {rfv}"
    );

    // BC-3.8.007: labels must be a plain string array ["alpha", "beta"].
    let labels = rfv
        .get("labels")
        .and_then(Value::as_array)
        .expect("BC-3.8.007: labels must be a JSON array");

    assert_eq!(
        labels.len(),
        2,
        "BC-3.8.007: expected 2 labels, got {}; labels: {labels:?}",
        labels.len()
    );
    // Labels must be strings, NOT objects.
    assert!(
        labels[0].is_string(),
        "BC-3.8.007: labels must be plain strings, not objects; got: {:?}",
        labels[0]
    );
    assert_eq!(
        labels.iter().filter_map(Value::as_str).collect::<Vec<_>>(),
        vec!["alpha", "beta"],
        "BC-3.8.007: labels must be ['alpha', 'beta'] in order; got: {labels:?}"
    );

    // Negative: labels must NOT be an object array like [{"name": "alpha"}].
    assert!(
        labels.iter().all(|l| l.is_string()),
        "BC-3.8.007: all label entries must be plain strings, not objects; got: {labels:?}"
    );
}

// ─── AC-009: --field NAME=VALUE parsing ──────────────────────────────────────

/// AC-009 (BC-3.8.008): `--field` custom fields are merged into requestFieldValues.
/// First-equals split: `desc=bar=baz` → key="desc", value="bar=baz".
/// Duplicate: last value wins.
#[tokio::test]
async fn test_jsm_create_field_first_equals_split_and_duplicate_last_wins() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_10200=foo",
            "--field",
            "desc=bar=baz",
            "--field",
            "customfield_10200=overridden", // duplicate — last wins
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.008: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("BC-3.8.008: JSM POST must have been made");

    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("BC-3.8.008: POST body must be valid JSON");

    let rfv = body
        .get("requestFieldValues")
        .expect("BC-3.8.008: requestFieldValues must be present");

    // BC-3.8.008: first-equals split — desc=bar=baz → value "bar=baz".
    assert_eq!(
        rfv.get("desc").and_then(Value::as_str),
        Some("bar=baz"),
        "BC-3.8.008: first-equals split: 'desc=bar=baz' must yield value 'bar=baz'; got rfv: {rfv}"
    );

    // BC-3.8.008: duplicate last-wins — customfield_10200 should be "overridden".
    assert_eq!(
        rfv.get("customfield_10200").and_then(Value::as_str),
        Some("overridden"),
        "BC-3.8.008: duplicate key last-wins: customfield_10200 must be 'overridden'; got rfv: {rfv}"
    );
}

/// AC-009 (BC-3.8.008): Missing `=` in `--field` argument exits 64 with a
/// descriptive error message.
#[tokio::test]
async fn test_jsm_create_field_missing_equals_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    // M-02 (adversary pass-02-retry): regression guard — `--field nokvinthis`
    // must exit 64 BEFORE the POST is attempted. A future refactor moving
    // parse_field_kv after create_jsm_request would silently pass without
    // this guard (exit-64 would still come from JSM 5xx fallback).
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "nokvinthis",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.008: expected exit 64 for missing '=', got {:?}. stderr: {stderr}",
        output.status.code()
    );
    // BC-3.8.008: error must identify the malformed pair.
    assert!(
        stderr.contains("nokvinthis"),
        "BC-3.8.008: error must mention the malformed pair 'nokvinthis'; got: {stderr}"
    );
    assert!(
        stderr.contains("NAME=VALUE"),
        "BC-3.8.008: error must mention NAME=VALUE format requirement; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// S-578-3: real `:kind` dispatch has landed on the JSM create path,
// superseding the S-578-1 interim guard this test previously pinned (the
// guard call site and its underlying `reject_unsupported_hint_kinds` helper
// have both been removed — see `tests/issue_create_jsm.rs`'s AC-001..010
// block further down in this file for the full new-behavior coverage). This
// test is flipped, not deleted, to keep asserting the end-to-end outcome for
// this exact `--field cf:id=10042` input: it now dispatches through
// `JsmRequestBuilder::build()`'s kind-aware match and succeeds (exit 0),
// producing `{"id": "10042"}` on `requestFieldValues.cf` (by analogy to the
// platform-path shape; VP-578-016 parity-PENDING).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_jsm_create_field_kind_hint_dispatches_real_id_shape_s578_3() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "cf:id=10042",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "S-578-3: '--field cf:id=10042' must dispatch through real :kind handling and \
         succeed (exit 0) — the S-578-1 interim guard has been removed. \
         stderr={stderr}"
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("S-578-3: JSM POST must have been made");
    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("S-578-3: POST body must be valid JSON");
    assert_eq!(
        body["requestFieldValues"]["cf"],
        json!({"id": "10042"}),
        "S-578-3: ':id' hint must produce {{\"id\": \"10042\"}} on requestFieldValues; \
         got body: {body}"
    );
}

/// S-578-1 regression pin (paired with the test above, which now dispatches
/// through real `:kind` handling with the interim guard removed): a BARE
/// `--field NAME=VALUE` pair (`kind: None`) must keep working exactly as
/// before — hinted (`kind: Some(_)`) and unhinted pairs must never interfere
/// with each other. This test is a restatement of the existing last-wins
/// coverage above, scoped narrowly to that non-interference property.
#[tokio::test]
async fn test_jsm_create_field_bare_pair_unaffected_by_kind_hint_guard_s578_1() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "cf=10042",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "S-578-1 regression pin: a bare (unhinted) '--field cf=10042' pair must keep succeeding \
         (kind: None must never trip the interim ':kind'-hint guard); \
         stderr={stderr} stdout={stdout}"
    );
}

// ─── AC-010: --on-behalf-of → raiseOnBehalfOf at top level ──────────────────

/// AC-010 (BC-3.8.009): `--on-behalf-of` maps to top-level `raiseOnBehalfOf`
/// in the POST body, NOT inside `requestFieldValues`.
#[tokio::test]
async fn test_jsm_create_on_behalf_of_injected_at_top_level() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--on-behalf-of",
            "557058:abc123",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.009: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("BC-3.8.009: JSM POST must have been made");

    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("BC-3.8.009: POST body must be valid JSON");

    // BC-3.8.009: raiseOnBehalfOf must be at TOP level.
    assert_eq!(
        body.get("raiseOnBehalfOf").and_then(Value::as_str),
        Some("557058:abc123"),
        "BC-3.8.009: raiseOnBehalfOf must be at top level with value '557058:abc123'; got body: {body}"
    );

    // BC-3.8.009: raiseOnBehalfOf must NOT be inside requestFieldValues.
    let rfv_obo = body
        .get("requestFieldValues")
        .and_then(|rfv| rfv.get("raiseOnBehalfOf"));
    assert!(
        rfv_obo.is_none(),
        "BC-3.8.009: raiseOnBehalfOf must NOT be inside requestFieldValues; got rfv: {:?}",
        body.get("requestFieldValues")
    );
}

/// AC-010 sibling (BC-3.8.009): Without `--on-behalf-of`, the `raiseOnBehalfOf`
/// key must be completely absent from the POST body (NOT null).
#[tokio::test]
async fn test_jsm_create_on_behalf_of_absent_when_not_set() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test no obo",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.009: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("BC-3.8.009: JSM POST must have been made");

    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("BC-3.8.009: POST body must be valid JSON");

    // BC-3.8.009: raiseOnBehalfOf key must be completely absent, not null.
    assert!(
        body.get("raiseOnBehalfOf").is_none(),
        "BC-3.8.009: raiseOnBehalfOf must be completely absent when --on-behalf-of not set; got body: {body}"
    );
}

// ─── AC-011: --type flag emits warning to stderr, still exits 0 ──────────────

/// AC-011 (BC-3.8.010, H-NEW-JSM-RT-004): When both `--request-type` and `--type`
/// are set, a warning is emitted to stderr and the command succeeds (exit 0).
/// The warning must use the verbatim BC-3.8.010 string.
#[tokio::test]
async fn test_jsm_create_type_flag_ignored_with_warning() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--type",
            "Task",
            "--summary",
            "test",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // BC-3.8.010, H-NEW-JSM-RT-004: must exit 0 despite --type being set.
    assert!(
        output.status.success(),
        "BC-3.8.010 / H-NEW-JSM-RT-004: expected exit 0 (warning, not error), got {:?}. stderr: {stderr}",
        output.status.code()
    );

    // BC-3.8.010: verbatim warning string must appear on stderr.
    assert!(
        stderr.contains("warning: --type is ignored when --request-type is set"),
        "BC-3.8.010: stderr must contain verbatim warning; got: {stderr}"
    );
    assert!(
        stderr.contains("request type encodes the issue type"),
        "BC-3.8.010: warning must include 'request type encodes the issue type'; got: {stderr}"
    );
}

// ─── AC-012 / AC-5 (BC-3.8.014): Basic-auth 401 on JSM POST → API-token-expiry hint ─

/// AC-012 / AC-5 (BC-3.8.014, repurposed in place from S-384): When the JSM POST
/// returns HTTP 401 with a generic-expiry body and the active auth is Basic
/// (`JR_AUTH_HEADER=Basic ...`), the `handle_jsm_create` map_err MUST rewrite any
/// incoming variant to `JrError::NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }`.
///
/// Fixture stays Basic per adversary-pass-9 C-01 correction: a Bearer + generic-expiry
/// 401 routes through the auto-refresh coordinator in `send_inner`, fails with raw anyhow
/// via `JR_AUTH_HEADER` seam, and the hint is never injected — making a Bearer test
/// non-deterministic. This test is a BC-3.8.014 (Basic-auth API-token expiry) pin.
///
/// Assertions flipped from `write:servicedesk-request` (pre-S-384 behavior) to
/// API-token-expiry hint — the pre-S-384 behavior was the bug (O-08-01 CONFIRMED).
#[tokio::test]
async fn test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    // JSM POST returns 401 with generic-expiry body (plausible Atlassian shape).
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": [
                "The access token provided is expired, revoked, malformed, or invalid for other reasons."
            ],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        // Basic auth fixture — stays Basic per BC-3.8.014 / adversary-pass-9 C-01.
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test 401",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit non-zero (exit code 2 for NotAuthenticated).
    assert!(
        !output.status.success(),
        "BC-3.8.014: expected non-zero exit for Basic-auth 401, got exit 0. stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "BC-3.8.014: expected exit code 2 (NotAuthenticated) for Basic-auth 401; stderr: {stderr}"
    );

    // BC-3.8.014 postcondition 1: API-token-expiry hint must be present.
    // (L-288-pr2-02 strict split: three separate assertions, no `||`.)
    assert!(
        stderr.contains("expired or revoked"),
        "BC-3.8.014: stderr must contain 'expired or revoked' from API_TOKEN_EXPIRY_HINT; got: {stderr}"
    );
    assert!(
        stderr.contains("id.atlassian.com/manage-profile/security/api-tokens"),
        "BC-3.8.014: stderr must contain the api-tokens URL; got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "BC-3.8.014: stderr must contain 'jr auth login' actionable recovery; got: {stderr}"
    );

    // BC-3.8.014 postcondition 3: OAuth scope hint must NOT appear for Basic-auth users.
    assert!(
        !stderr.contains("write:servicedesk-request"),
        "BC-3.8.014: Basic-auth 401 hint must NOT mention 'write:servicedesk-request'; got: {stderr}"
    );
}

/// AC-012 sibling: Platform POST returning 401 must NOT emit the
/// `write:servicedesk-request` scope hint (regression guard against false-positive
/// scope hint on non-JSM 401s).
#[tokio::test]
async fn test_platform_create_401_no_jsm_scope_hint() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // Platform POST returns 401.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": [
                "The access token provided is expired, revoked, malformed, or invalid for other reasons."
            ],
            "errors": {}
        })))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "platform 401 test",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit non-zero.
    assert!(
        !output.status.success(),
        "Expected non-zero exit for platform 401, got exit 0. stderr: {stderr}"
    );

    // Regression guard: platform 401 must NOT mention the JSM-specific scope.
    assert!(
        !stderr.contains("write:servicedesk-request"),
        "Platform 401 must NOT mention 'write:servicedesk-request' scope; got: {stderr}"
    );
}

// ─── AC-015: --output json shape matches platform create ─────────────────────

/// AC-015 (BC-3.8.001): `jr issue create --request-type ... --output json`
/// emits `{"key": "<issue_key>"}` — identical shape to platform create.
/// No additional fields beyond `key`.
#[tokio::test]
async fn test_jsm_create_output_json_shape_matches_platform() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "json shape test",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.001 / AC-015: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    // Must be valid JSON.
    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("AC-015: stdout must be valid JSON; got: {stdout}\nError: {e}"));

    // BC-3.8.001 / AC-015: JSON shape must be {{"key": "<issue_key>"}}.
    assert_eq!(
        parsed.get("key").and_then(Value::as_str),
        Some("HELP-42"),
        "AC-015: JSON output must contain key='HELP-42'; got: {parsed}"
    );

    // The shape should be minimal — just {"key": "..."}.
    // (The platform also adds "url" and "fields" in json mode; for JSM we expect
    // the simpler shape per AC-015. If the impl adds these later, update the test.)
    let obj = parsed
        .as_object()
        .expect("AC-015: stdout must be a JSON object");
    assert!(
        obj.contains_key("key"),
        "AC-015: JSON output must contain 'key' field; got: {parsed}"
    );
}

// ─── C-01: OAuth InsufficientScope 401 surfaces write:servicedesk-request ────

/// C-01 (adversary pass-01): OAuth scope-mismatch 401 must surface the
/// write:servicedesk-request hint via JrError::InsufficientScope dispatch.
///
/// The existing `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`
/// uses Basic auth which hits the `NotAuthenticated` branch; this test uses
/// Bearer auth + body "scope does not match" which hits the `InsufficientScope`
/// branch (the `"scope does not match"` body check in `send_inner`). Regression guard for the C-01 fix in
/// `src/cli/issue/jsm_create.rs::handle_jsm_create map_err`.
///
/// // H-NEW-JSM-RT-003 + BC-3.8.015 anchor
/// This test IS H-NEW-JSM-RT-003 (re-bound per F2 adversary-pass-9 C-01).
/// Logic, fixture, and assertions MUST remain unmodified — this test pins
/// the only deterministic OAuth→JrError→write:servicedesk-request path via
/// the JR_AUTH_HEADER seam (Bearer + scope-mismatch body short-circuits to
/// InsufficientScope via the `"scope does not match"` body check in `send_inner`
/// BEFORE the auto-refresh coordinator).
#[tokio::test]
async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    // Bearer auth (Authorization: Bearer ...) plus a 401 body containing the
    // exact Atlassian phrase that triggers InsufficientScope dispatch.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        // Bearer, not Basic — triggers InsufficientScope branch in client.rs.
        .env("JR_AUTH_HEADER", "Bearer test-oauth-token")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "Reset my password",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "C-01: OAuth scope mismatch must exit non-zero; stderr: {stderr}"
    );
    // Per L-288-pr2-02: three separate strict assertions, no `||` accept-either.
    assert!(
        stderr.contains("write:servicedesk-request"),
        "C-01 / BC-X.3.005: hint must mention `write:servicedesk-request` scope; got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth refresh"),
        "C-01 / BC-X.3.005: hint must include `jr auth refresh` actionable recovery; got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "C-01 / BC-X.3.005: hint must include `jr auth login` actionable recovery; got: {stderr}"
    );
}

// ─── C-02: Per-flag warnings for platform-only flags on JSM path ──────────────

/// C-02 (adversary pass-01) + BC-3.8.011: `--team` is ignored with a verbatim
/// warning when `--request-type` is set. The JSM POST must still succeed (exit 0).
#[tokio::test]
async fn test_jsm_create_team_flag_emits_warning_with_request_type() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "X",
            "--team",
            "some-team-name",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "C-02: --team warning must not block success; exit {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains(
            "warning: --team is ignored when --request-type is set; teams are managed by the request type's workflow"
        ),
        "C-02 / BC-3.8.011: verbatim --team warning must appear on stderr; got: {stderr}"
    );
}

/// C-02 (adversary pass-01) + BC-3.8.011: `--points` is ignored with a verbatim
/// warning when `--request-type` is set. The JSM POST must still succeed (exit 0).
#[tokio::test]
async fn test_jsm_create_points_flag_emits_warning_with_request_type() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "X",
            "--points",
            "5.0",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "C-02: --points warning must not block success; exit {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains(
            "warning: --points is ignored when --request-type is set; story points are not part of JSM request schema"
        ),
        "C-02 / BC-3.8.011: verbatim --points warning must appear on stderr; got: {stderr}"
    );
}

/// C-02 (adversary pass-01) + BC-3.8.011: `--parent` is ignored with a verbatim
/// warning when `--request-type` is set. The JSM POST must still succeed (exit 0).
#[tokio::test]
async fn test_jsm_create_parent_flag_emits_warning_with_request_type() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "X",
            "--parent",
            "HELP-1",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "C-02: --parent warning must not block success; exit {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains(
            "warning: --parent is ignored when --request-type is set; JSM requests cannot be sub-tasks"
        ),
        "C-02 / BC-3.8.011: verbatim --parent warning must appear on stderr; got: {stderr}"
    );
}

/// C-02 (adversary pass-01) + BC-3.8.011: `--to` is ignored with a verbatim
/// warning when `--request-type` is set. The JSM POST must still succeed (exit 0).
#[tokio::test]
async fn test_jsm_create_to_flag_emits_warning_with_request_type() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "X",
            "--to",
            "jsmith",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "C-02: --to warning must not block success; exit {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains(
            "warning: --to is ignored when --request-type is set; use --on-behalf-of to set the requester"
        ),
        "C-02 / BC-3.8.011: verbatim --to warning must appear on stderr; got: {stderr}"
    );
}

/// C-02 (adversary pass-01) + BC-3.8.011: `--account-id` is ignored with a verbatim
/// warning when `--request-type` is set. The JSM POST must still succeed (exit 0).
#[tokio::test]
async fn test_jsm_create_account_id_flag_emits_warning_with_request_type() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "X",
            "--account-id",
            "557058:abc123",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "C-02: --account-id warning must not block success; exit {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains(
            "warning: --account-id is ignored when --request-type is set; use --on-behalf-of to set the requester"
        ),
        "C-02 / BC-3.8.011: verbatim --account-id warning must appear on stderr; got: {stderr}"
    );
}

// ─── H-02: Missing project on JSM path exits 64 with JSM-specific hint ────────

/// H-02 (adversary pass-01) + BC-3.8.002 (O-08-02 harmonized string): missing project
/// on JSM path exits 64 with the harmonized verbatim string carrying --project /
/// .jr.toml / jr project list affordances. Regression guard for the impl change in
/// `src/cli/issue/jsm_create.rs::handle_jsm_create`.
///
/// UPDATED by S-385 (O-08-02): assertion updated from the terse pre-#385 string
/// ("project is required for JSM request creation") to the harmonized form
/// (BC-3.8.002 CANONICAL SOURCE — same affordances as the platform path while
/// preserving the JSM-specific context label).
#[tokio::test]
async fn test_jsm_create_missing_project_exits_64_with_jsm_specific_hint() {
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    // Write config WITHOUT a project field so there is no fallback project.
    let dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        r#"default_profile = "default"
[profiles.default]
url = "https://example.atlassian.net"
auth_method = "api_token"
"#,
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "create",
            "--request-type",
            "Password Reset",
            "--summary",
            "X",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "H-02 / BC-3.8.002: expected exit 64 for missing project on JSM path; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    // BC-3.8.002 (O-08-02 harmonized string — CANONICAL SOURCE in bc-3-issue-write.md).
    // The terse pre-#385 string "project is required for JSM request creation" is REPLACED
    // by the harmonized form below. Copy verbatim; any deviation causes adversarial failure.
    assert!(
        stderr.contains(
            "Project key is required for JSM request creation. \
             Use --project or configure .jr.toml. \
             Run \"jr project list\" to see available JSM projects."
        ),
        "H-02 / BC-3.8.002: harmonized verbatim missing-project hint must appear; got: {stderr}"
    );
}

// ─── H-03: Missing summary on JSM path exits 64 ───────────────────────────────

/// H-03 (adversary pass-01) + BC-3.8.005: `jr issue create --project HELP
/// --request-type "Password Reset" --no-input` (no --summary) exits 64 and
/// emits the BC-mandated verbatim string. The POST to /rest/servicedeskapi/request
/// must NEVER be called.
#[tokio::test]
async fn test_jsm_create_missing_summary_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    // POST must never be called when summary is missing.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "H-03 / BC-3.8.005: expected exit 64 for missing summary; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("summary is required for JSM request submission"),
        "H-03 / BC-3.8.005: verbatim missing-summary hint must appear; got: {stderr}"
    );
    // The .expect(0) on the POST mock is enforced on server drop.
}

// ─── H-04: Request type not found exits 64 with cache-deletion hint ───────────

/// H-04 (adversary pass-01): When `--request-type "Zebra"` does not match any
/// request type in the list, exits 64 with a "not found" message + hint to
/// list types + cache-deletion suggestion (BC-X.12.008-style pattern).
#[tokio::test]
async fn test_jsm_create_request_type_not_found_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    // Only "Password Reset" exists; "Zebra" will not match.
    mount_request_types_password_reset(&server).await;

    // POST must never be called when request type resolution fails.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Zebra",
            "--summary",
            "test not found",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "H-04: expected exit 64 for request type not found; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Request type \"Zebra\" not found"),
        "H-04: stderr must contain 'Request type \"Zebra\" not found'; got: {stderr}"
    );
    assert!(
        stderr.contains("Run `jr requesttype list --project HELP`"),
        "H-04: stderr must contain actionable hint to list request types; got: {stderr}"
    );
    // Cache-deletion path hint: assert the structural parts that are stable across
    // platforms. The full path prefix (e.g. ~/.cache/jr/v1/ on Unix or
    // %LOCALAPPDATA%\jr\v1\ on Windows) uses the OS separator, so we do NOT
    // assert "/jr/v1/" — that would fail on Windows CI (backslash separators).
    // Instead, assert the separator-agnostic prefix phrase and the filename suffix
    // (same hardening as tests/requesttype_commands.rs::test_bc_x_12_008_*).
    assert!(
        stderr.contains("or delete the cache file at "),
        "H-04: cache-deletion hint must contain 'or delete the cache file at'; got: {stderr}"
    );
    assert!(
        stderr.contains("request_types_10.json"),
        "H-04: cache-deletion hint must contain 'request_types_10.json' filename (sid=10 from fixture); got: {stderr}"
    );
    // The .expect(0) on the POST mock is enforced on server drop.
}

// ─── M-02: --field summary=X overrides --summary X ───────────────────────────

/// M-02 (adversary pass-01) + BC-3.8.008: when `--summary X` and `--field summary=Y`
/// are BOTH set, `--field` wins (extra_fields override base fields per
/// JsmRequestBuilder insertion order). Regression guard for any refactor that
/// moves extra_fields merge before the summary insert.
#[tokio::test]
async fn test_jsm_create_field_summary_overrides_summary_flag() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    // The POST body matcher requires summary="from-field", not "from-flag".
    // body_partial_json fails the mock if summary is "from-flag" instead of "from-field".
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .and(body_partial_json(json!({
            "requestFieldValues": {
                "summary": "from-field"
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "from-flag",
            "--field",
            "summary=from-field",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "M-02 / BC-3.8.008: expected exit 0; --field summary must override --summary. exit {:?}, stderr: {stderr}",
        output.status.code()
    );
    // The .expect(1) on the body_partial_json mock enforces the override semantics on server drop.
}

// ─── M-03: --markdown + --description on JSM path produces ADF ───────────────

/// M-03 (adversary pass-02-retry) + BC-3.8.006: `--markdown` with `--description`
/// on JSM path produces an ADF document (`isAdfRequest: true`, description.type ==
/// "doc"). Pins the markdown_to_adf path through JsmRequestBuilder::build()
/// lines 94-104. Regression guard for any change that drops the markdown branch.
///
/// The body_partial_json matcher verifies `isAdfRequest: true` and that
/// `requestFieldValues.description` is an ADF doc object. The POST body is also
/// inspected via received_requests to assert at least one text node carries a
/// "strong" mark (from the `**bold**` input), confirming markdown_to_adf ran.
#[tokio::test]
async fn test_jsm_create_markdown_description_yields_adf_with_strong_marks() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    // Match: body has isAdfRequest: true AND description is an ADF doc object.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .and(body_partial_json(json!({
            "isAdfRequest": true,
            "requestFieldValues": {
                "description": {
                    "type": "doc",
                    "version": 1
                }
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"issueKey": "HELP-1"})))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "Reset",
            "--description",
            "**bold** text with `code`",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "M-03 / BC-3.8.006: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    // Inspect the actual POST body to verify markdown_to_adf produced a "strong" mark.
    // This distinguishes markdown_to_adf (produces marks) from text_to_adf (plain text).
    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("M-03: JSM POST must have been made");

    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("M-03: POST body must be valid JSON");

    let desc = body
        .get("requestFieldValues")
        .and_then(|rfv| rfv.get("description"))
        .expect("M-03 / BC-3.8.006: requestFieldValues.description must be present");

    // Walk content to find any text node with a "strong" mark — produced by **bold**.
    fn has_strong_mark(node: &Value) -> bool {
        if let Some(marks) = node.get("marks").and_then(Value::as_array) {
            if marks
                .iter()
                .any(|m| m.get("type").and_then(Value::as_str) == Some("strong"))
            {
                return true;
            }
        }
        if let Some(children) = node.get("content").and_then(Value::as_array) {
            return children.iter().any(has_strong_mark);
        }
        false
    }

    assert!(
        has_strong_mark(desc),
        "M-03 / BC-3.8.006: description ADF must contain a node with mark type 'strong' \
         (from **bold** input via markdown_to_adf); got description: {desc}"
    );
    // The .expect(1) on the body_partial_json mock enforces isAdfRequest + doc shape on server drop.
}

// ─── M-01 sanity: --markdown without --description exits 64 on JSM path ───────

/// M-01 (adversary pass-02-retry): `--markdown` without `--description` or
/// `--description-stdin` on the JSM path errors with a JSM-specific message.
/// No platform-path equivalent exists (S-639-1, EC-3.8.012-5 — see the
/// correction in `jsm_create.rs::handle_jsm_create` step 3): on the platform
/// path, `--markdown` with no description is simply a no-op. Regression guard
/// for the validation block added in handle_jsm_create at b35bc1a.
///
/// No HTTP mocks are mounted — the validation fires before any HTTP is made.
/// If a future refactor moves the validation after HTTP, the test will fail
/// because wiremock has no matching mock (returns 404 → JSM error that does
/// not contain the expected message).
#[tokio::test]
async fn test_jsm_create_markdown_without_description_exits_64_with_platform_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // No HTTP mocks mounted — the validation fires before any HTTP is made.
    // If a future refactor moves the validation after HTTP, the test will
    // fail because wiremock has no matching mock (returns 404 → JSM error).

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "Reset",
            "--markdown", // No --description, no --description-stdin
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "M-01 / BC-3.8.006: expected exit 64 for --markdown without --description; stderr: {stderr}"
    );
    // JSM-specific message (jsm_create.rs::handle_jsm_create step 3) — no
    // platform-path equivalent exists (S-639-1, EC-3.8.012-5).
    assert!(
        stderr.contains("--markdown requires --description or --description-stdin to take effect"),
        "M-01 / BC-3.8.006: expected JSM-path validation message; got: {stderr}"
    );
}

// ─── S-639-1: Platform-path pre-flight exit-64 guards (BC-3.8.012 / BC-3.8.013,
//     DEC-188) ─────────────────────────────────────────────────────────────────
//
// These tests live in `issue_create_jsm.rs` by the explicit decision in the
// S-383 story file (`.factory/stories/S-383-platform-inverse-warnings.md`
// §"Test File Decision"), carried forward by S-639-1
// (`.factory/stories/S-639-1.md`). They are PLATFORM-PATH tests — no
// `--request-type` flag — co-located here because they cover the inverse
// symmetry of the BC-3.8.011 forward-direction warnings already in this file.
//
// **IMPLEMENTING SUCCESSOR to S-383 (DEC-188, 2026-07-25):** the S-383
// warn-and-proceed contract (exit 0 + issue created despite the stray flag)
// is SUPERSEDED by a pre-flight `JrError::UserError` exit-64 guard that fires
// BEFORE any HTTP call. AC-1/AC-2/AC-3/AC-5/AC-7 below are INVERTED from
// exit-0 to exit-64 (renamed per the story's "Superseded Tests" table);
// AC-4/AC-6 are vacuity→non-vacuity transitions (same names, updated bodies).
// AC-8 through AC-21 are new tests added by S-639-1.
//
// Red Gate: every test below asserting exit-64 + a new verbatim error string
// MUST fail against the unmodified (S-383-era) implementation in
// `src/cli/issue/create.rs`, which still warns-and-proceeds (exit 0) for
// `--field` / `--on-behalf-of` without `--request-type`. The guard
// implementation (3-branch pre-flight check after the JSM dispatch fork) is
// implemented in `src/cli/issue/create.rs` on this branch (S-639-1 Task 3).

/// Helper: mount the two stubs the platform path needs (POST /rest/api/3/issue
/// + GET /rest/api/3/field for CMDB discovery) and return the key "PROJ-123".
async fn mount_platform_create_stubs(server: &wiremock::MockServer) {
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::Value::Array(vec![])))
        .mount(server)
        .await;
}

// ─── AC-1: --field on platform path exits 64 pre-flight (BC-3.8.012) ─────────

/// AC-1 (BC-3.8.012, [mode: human]): `jr issue create --field NAME=VALUE`
/// WITHOUT `--request-type` exits 64 BEFORE any HTTP, with the verbatim
/// BC-3.8.012 single-flag error on stderr. INVERTED from the S-383 exit-0
/// warn-and-proceed contract (DEC-188). Renamed from
/// `test_platform_create_field_flag_emits_warning_without_request_type`.
///
/// Pairing: symmetric twin of AC-10 ([mode: --output json]) for the same
/// invocation class.
#[tokio::test]
async fn test_platform_create_field_flag_exits_64_without_request_type() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    common::fixtures::write_profile_config(config_dir.path(), &server.uri());

    // Would-otherwise-succeed precondition — proves the guard fires pre-flight,
    // not merely that the platform POST happens to be unreachable.
    mount_platform_create_stubs(&server).await;

    // JSM endpoint must NEVER be called.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "a=b",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.012 / AC-1: expected exit 64; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Error: "),
        "BC-3.8.012 / AC-1: human-mode 'Error: ' prefix must appear; got: {stderr}"
    );
    assert!(
        stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-1: prefix pin must appear on stderr; got: {stderr}"
    );
    assert!(
        stderr.contains(
            "--field is only valid with --request-type (JSM service-desk requests). Add --request-type <NAME> to submit a JSM request with custom fields, or drop --field to create a standard platform issue."
        ),
        "BC-3.8.012 / AC-1: FULL-STRING verbatim single-flag error must appear on stderr; got: {stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "BC-3.8.012 / AC-1: stdout must be empty (HYGIENE); got: {stdout}"
    );
    assert!(
        !stderr.contains("Created issue"),
        "BC-3.8.012 / AC-1: DISCRIMINATING — no success path must have executed; got: {stderr}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.012 / AC-1: REGRESSION PIN — old S-383 warn string must not appear; got: {stderr}"
    );
    // The .expect(0) on the JSM mock is enforced on server drop. The
    // NORMATIVE zero-HTTP proof (received_requests().is_empty()) is covered
    // by AC-8, which uses an isolated MockServer specifically for that check.
}

// ─── AC-2: --on-behalf-of on platform path exits 64 pre-flight (BC-3.8.013) ──

/// AC-2 (BC-3.8.013, [mode: --output json]): `jr issue create --on-behalf-of
/// <ID>` WITHOUT `--request-type` exits 64 with a JSON error envelope on
/// stderr. INVERTED from the S-383 exit-0 warn-and-proceed contract
/// (DEC-188). Renamed from
/// `test_platform_create_on_behalf_of_flag_emits_warning_without_request_type`.
#[tokio::test]
async fn test_platform_create_on_behalf_of_flag_exits_64_without_request_type() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    // Pre-migrated config REQUIRED — assert_json_error_envelope strict-parses
    // stderr as JSON; the legacy [instance] shape triggers a migration line
    // that would poison the parse.
    common::fixtures::write_profile_config(config_dir.path(), &server.uri());

    mount_platform_create_stubs(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--on-behalf-of",
            "X",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    common::assertions::assert_json_error_envelope(&output, 64, "BC-3.8.013 / AC-2");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert!(
        parsed["error"]
            .as_str()
            .is_some_and(|s| s.contains("--on-behalf-of is only valid with")),
        "BC-3.8.013 / AC-2: error field must contain the single-flag prefix pin; got: {parsed}"
    );
    assert!(
        stdout.trim().is_empty(),
        "BC-3.8.013 / AC-2: DISCRIMINATING — stdout must be empty; got: {stdout}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.013 / AC-2: REGRESSION PIN — old S-383 warn string must not appear; got: {stderr}"
    );
}

// ─── AC-3: Both --field + --on-behalf-of exit 64 with ONE combined error ─────

/// AC-3 (BC-3.8.012 combined postcondition, [mode: human]): When both
/// `--field NAME=VALUE` and `--on-behalf-of <ID>` are supplied WITHOUT
/// `--request-type`, exactly ONE combined `JrError::UserError` fires
/// (exit 64) — NOT two independent single-flag errors. INVERTED from the
/// S-383 exit-0 warn-and-proceed contract (DEC-188). Renamed from
/// `test_platform_create_both_inverse_flags_emit_independent_warnings`.
#[tokio::test]
async fn test_platform_create_both_inverse_flags_exit_64_combined_error() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_platform_create_stubs(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "a=b",
            "--on-behalf-of",
            "X",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.012 / AC-3: expected exit 64; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("--field and --on-behalf-of are only valid with"),
        "BC-3.8.012 / AC-3: combined-error prefix pin must appear on stderr; got: {stderr}"
    );
    assert!(
        stderr.contains(
            "--field and --on-behalf-of are only valid with --request-type (JSM service-desk requests). Add --request-type <NAME> to use these flags, or drop them to create a standard platform issue."
        ),
        "BC-3.8.012 / AC-3: FULL-STRING verbatim combined error must appear on stderr; got: {stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "BC-3.8.012 / AC-3: stdout must be empty (HYGIENE); got: {stdout}"
    );
    assert!(
        !stderr.contains("Created issue"),
        "BC-3.8.012 / AC-3: DISCRIMINATING — no success path must have executed; got: {stderr}"
    );
    assert!(
        !stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-3: FALSIFIABLE-COARSE — single-flag guard must NOT fire instead of combined; got: {stderr}"
    );
    assert!(
        !stderr.contains("--on-behalf-of is only valid with"),
        "BC-3.8.013 / AC-3: FALSIFIABLE-COARSE — single-flag guard must NOT fire instead of combined; got: {stderr}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.012+013 / AC-3: REGRESSION PIN — old S-383 warn strings must not appear; got: {stderr}"
    );
}

// ─── AC-4: No inverse flags → no pre-flight errors (regression baseline) ─────

/// AC-4 (BC-3.8.012 negative postcondition — clean-path regression baseline,
/// [mode: --output json]): `jr issue create --project PROJ --summary "test"`
/// WITHOUT `--field` AND WITHOUT `--on-behalf-of` AND WITHOUT `--request-type`
/// must NOT trip any of the three new pre-flight guard error strings. Exit
/// code stays 0 (BREAKING-CHANGE REGRESSION PIN, H-NEW-PREFLIGHT-004).
///
/// **AC-4 VACUITY→NON-VACUITY TRANSITION (DEC-188):** the old assertions
/// (`!stderr.contains("--field is ignored")` / the `--on-behalf-of` twin) are
/// vacuously true post-DEC-188 — those substrings no longer exist ANYWHERE in
/// the codebase, so they would pass even if the guard fired unconditionally.
/// Replaced with FALSIFIABLE-COARSE negatives on the three NEW error
/// substrings, which DO catch an unconditionally-firing guard.
/// Renamed from `test_platform_create_without_inverse_flags_emits_no_new_warnings`.
#[tokio::test]
async fn test_platform_create_without_inverse_flags_emits_no_errors() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_platform_create_stubs(&server).await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.012+013 / AC-4: expected exit 0 (H-NEW-PREFLIGHT-004 regression pin); got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-4: FALSIFIABLE-COARSE — single-flag guard must NOT fire on a clean invocation; got: {stderr}"
    );
    assert!(
        !stderr.contains("--on-behalf-of is only valid with"),
        "BC-3.8.013 / AC-4: FALSIFIABLE-COARSE — single-flag guard must NOT fire on a clean invocation; got: {stderr}"
    );
    assert!(
        !stderr.contains("--field and --on-behalf-of are only valid with"),
        "BC-3.8.012 / AC-4: FALSIFIABLE-COARSE — combined guard must NOT fire on a clean invocation; got: {stderr}"
    );
}

// ─── AC-5: Multiple --field occurrences → exactly ONE idempotent error ────────

/// AC-5 (BC-3.8.012 idempotency postcondition, [mode: human]): `--field a=b`
/// (ONE occurrence) and `--field a=b --field c=d` (TWO occurrences) WITHOUT
/// `--request-type` both exit 64 with the SAME single-flag error — the guard
/// fires on `!field_pairs.is_empty()` (presence-only), so `--field` is one
/// logical flag regardless of how many NAME=VALUE pairs are supplied.
/// INVERTED from the S-383 exit-0 warn-and-proceed contract (DEC-188).
/// Renamed from `test_platform_create_field_idempotent_one_warning_per_logical_flag`.
///
/// Two-invocation comparison test — deliberately separate from AC-1.
#[tokio::test]
async fn test_platform_create_field_idempotent_one_error_per_logical_flag() {
    // Invocation (i): exactly ONE --field.
    let server_i = MockServer::start().await;
    let cache_dir_i = tempfile::tempdir().unwrap();
    let config_dir_i = tempfile::tempdir().unwrap();
    common::fixtures::write_profile_config(config_dir_i.path(), &server_i.uri());
    mount_platform_create_stubs(&server_i).await;

    let output_i = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server_i.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir_i.path())
        .env("JR_CACHE_DIR", cache_dir_i.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir_i.path())
        .env("JR_CONFIG_DIR", config_dir_i.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "a=b",
            "--no-input",
        ])
        .output()
        .unwrap();

    // Invocation (ii): exactly TWO --field occurrences.
    let server_ii = MockServer::start().await;
    let cache_dir_ii = tempfile::tempdir().unwrap();
    let config_dir_ii = tempfile::tempdir().unwrap();
    common::fixtures::write_profile_config(config_dir_ii.path(), &server_ii.uri());
    mount_platform_create_stubs(&server_ii).await;

    let output_ii = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server_ii.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir_ii.path())
        .env("JR_CACHE_DIR", cache_dir_ii.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir_ii.path())
        .env("JR_CONFIG_DIR", config_dir_ii.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "a=b",
            "--field",
            "c=d",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr_i = String::from_utf8_lossy(&output_i.stderr).to_string();
    let stderr_ii = String::from_utf8_lossy(&output_ii.stderr).to_string();

    for (label, output, stderr) in [
        ("AC-5(i, n=1)", &output_i, &stderr_i),
        ("AC-5(ii, n=2)", &output_ii, &stderr_ii),
    ] {
        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.8.012 / {label}: expected exit 64; got {:?}. stderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("--field is only valid with"),
            "BC-3.8.012 / {label}: anchor — single-flag error must appear; got: {stderr}"
        );
        assert!(
            !stderr.contains("is ignored on the platform create path"),
            "BC-3.8.012 / {label}: REGRESSION PIN — old S-383 warn string must not appear; got: {stderr}"
        );
        assert!(
            !stderr.contains("Created issue"),
            "BC-3.8.012 / {label}: DISCRIMINATING — no success path must have executed; got: {stderr}"
        );
    }

    // Byte-identity: ONE error regardless of --field count (idempotent per-flag,
    // not per-value). The anchor assertions above guarantee this isn't merely
    // two identical "Created issue" success paths.
    assert_eq!(
        stderr_i, stderr_ii,
        "BC-3.8.012 / AC-5: stderr must be byte-identical for n=1 and n=2 --field occurrences \
         (idempotent, presence-only guard); i={stderr_i} ii={stderr_ii}"
    );
}

// ─── AC-6: JSM path + --field does NOT fire BC-3.8.012 (regression gate) ─────

/// AC-6 (BC-3.8.012 JSM-path non-mis-fire, BC-3.3.001 regression baseline,
/// [mode: --output json]): When `--request-type` IS set alongside `--field
/// NAME=VALUE`, the command takes the JSM path and neither the BC-3.8.012
/// single-flag nor combined guard may fire. Exit code stays 0. The
/// `expect(1)` POST stub below is KEPT (load-bearing).
///
/// **AC-6 VACUITY→NON-VACUITY TRANSITION (DEC-188):** the old assertion
/// (`!stderr.contains("--field is ignored on the platform create path")`) is
/// vacuously true post-DEC-188 — that substring no longer exists anywhere in
/// the codebase. Replaced with DISCRIMINATING + FALSIFIABLE-COARSE negatives
/// on the new guard error substrings.
#[tokio::test]
async fn test_jsm_create_with_field_and_request_type_does_not_fire_bc_3_8_012() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "NAME=VALUE",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.012 / AC-6: expected exit 0 on JSM path; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-6: DISCRIMINATING — single-flag guard must NOT fire on JSM path; got: {stderr}"
    );
    assert!(
        !stderr.contains("--field and --on-behalf-of are only valid with"),
        "BC-3.8.012 / AC-6: FALSIFIABLE-COARSE — combined guard must NOT fire when only --field is present; got: {stderr}"
    );
}

// ─── AC-7: Malformed --field on platform path exits 64 (EC-3.8.012-3) ────────

/// AC-7 (BC-3.8.012 EC-3.8.012-3 — malformed --field edge case,
/// [mode: --output json]): `--field bareflagnoequals` (no `=`) WITHOUT
/// `--request-type` exits 64 with the BC-3.8.012 single-flag error. The
/// guard fires on `!field_pairs.is_empty()` BEFORE value parsing — malformed
/// format does not affect guard activation. INVERTED from the S-383 exit-0
/// warn-and-proceed contract (DEC-188). Renamed from
/// `test_platform_create_malformed_field_one_warning_no_exit_64`.
#[tokio::test]
async fn test_platform_create_malformed_field_without_request_type_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    common::fixtures::write_profile_config(config_dir.path(), &server.uri());

    mount_platform_create_stubs(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "bareflagnoequals",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    common::assertions::assert_json_error_envelope(&output, 64, "BC-3.8.012 / AC-7");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert!(
        parsed["error"]
            .as_str()
            .is_some_and(|s| s.contains("--field is only valid with")),
        "BC-3.8.012 / AC-7: error field must contain the single-flag prefix pin; got: {parsed}"
    );
    assert!(
        stdout.trim().is_empty(),
        "BC-3.8.012 / AC-7: DISCRIMINATING — stdout must be empty; got: {stdout}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.012 / AC-7: REGRESSION PIN — old S-383 warn string must not appear; got: {stderr}"
    );
}

// ─── AC-8 (NEW): --field / --on-behalf-of + helper flags → zero HTTP ─────────

/// AC-8 (BC-3.8.012 + BC-3.8.013 zero-HTTP guarantee, [mode: human]): Even
/// when `--team`/`--to` (or `--field`/`--on-behalf-of`'s sibling) would
/// normally trigger pre-POST helper HTTP (team resolution, assignee
/// resolution), the pre-flight guard suppresses ALL of it — zero HTTP of any
/// kind. Two sub-invocations, each against its own dedicated isolated
/// `MockServer` (no `mount_platform_create_stubs` — wiremock 0.6 FIFO:
/// free-fire mocks registered first would defeat the `expect(0)` mocks here).
#[tokio::test]
async fn test_platform_create_field_with_helpers_exits_64_zero_http() {
    // Sub-invocation (i): --field + --team + --to.
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        for (m, p) in [
            ("GET", "/rest/api/3/myself"),
            ("POST", "/gateway/api/graphql"),
        ] {
            Mock::given(method(m))
                .and(path(p))
                .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
                .expect(0)
                .mount(&server)
                .await;
        }
        Mock::given(method("GET"))
            .and(path_regex("/gateway/api/public/teams/v1/org/.*/teams"))
            .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/rest/api/3/field"))
            .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue"))
            .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
            .expect(0)
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
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "a=b",
                "--team",
                "X",
                "--to",
                "me",
                "--no-input",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.8.012 / AC-8(i): expected exit 64; got {:?}. stderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("--field is only valid with"),
            "BC-3.8.012 / AC-8(i): prefix pin must appear; got: {stderr}"
        );
        assert!(
            !stderr.contains("is ignored on the platform create path"),
            "BC-3.8.012 / AC-8(i): REGRESSION PIN; got: {stderr}"
        );
        assert!(
            !stderr.contains("Created issue"),
            "BC-3.8.012 / AC-8(i): HYGIENE — structurally unreachable on an isolated server; got: {stderr}"
        );
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "BC-3.8.012 / AC-8(i): NORMATIVE zero-HTTP proof — no request of any kind must reach the server"
        );
        // All five .expect(0) mocks are additionally enforced on server drop.
    }

    // Sub-invocation (ii): --on-behalf-of + --team + --to (BC-3.8.013 mirror).
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        for (m, p) in [
            ("GET", "/rest/api/3/myself"),
            ("POST", "/gateway/api/graphql"),
        ] {
            Mock::given(method(m))
                .and(path(p))
                .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
                .expect(0)
                .mount(&server)
                .await;
        }
        Mock::given(method("GET"))
            .and(path_regex("/gateway/api/public/teams/v1/org/.*/teams"))
            .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/rest/api/3/field"))
            .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue"))
            .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
            .expect(0)
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
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--on-behalf-of",
                "X",
                "--team",
                "X",
                "--to",
                "me",
                "--no-input",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.8.013 / AC-8(ii): expected exit 64; got {:?}. stderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("--on-behalf-of is only valid with"),
            "BC-3.8.013 / AC-8(ii): prefix pin must appear; got: {stderr}"
        );
        assert!(
            !stderr.contains("is ignored on the platform create path"),
            "BC-3.8.013 / AC-8(ii): REGRESSION PIN; got: {stderr}"
        );
        assert!(
            !stderr.contains("Created issue"),
            "BC-3.8.013 / AC-8(ii): HYGIENE — structurally unreachable on an isolated server; got: {stderr}"
        );
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "BC-3.8.013 / AC-8(ii): NORMATIVE zero-HTTP proof — no request of any kind must reach the server"
        );
    }
}

// ─── AC-9 (NEW): --field without --project exits 64, not a project error ────

/// AC-9 (BC-3.8.012 EC-3.8.012-4, [mode: human]): `--field a=b` WITHOUT
/// `--project` and WITHOUT `--request-type` exits 64 with the BC-3.8.012
/// error — NOT the "Project key is required" error. Proves the guard fires
/// at step 2, BEFORE project-key resolution at step 3.
#[tokio::test]
async fn test_platform_create_field_without_project_exits_64_not_project_error() {
    let cwd_dir = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    // Config lacks a project key — write_minimal_config writes only [instance] url.
    write_minimal_config(config_dir.path(), &server.uri());

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd_dir.path())
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args(["issue", "create", "--field", "a=b", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.012 / AC-9: expected exit 64; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-9: positive guard assertion; got: {stderr}"
    );
    assert!(
        !stderr.contains("Project key"),
        "BC-3.8.012 / AC-9: DISCRIMINATING — guard must fire BEFORE project-key resolution; got: {stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "BC-3.8.012 / AC-9: stdout must be empty (HYGIENE); got: {stdout}"
    );
    assert!(
        !stderr.contains("Created issue"),
        "BC-3.8.012 / AC-9: HYGIENE — structurally unreachable without a project; got: {stderr}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.012 / AC-9: REGRESSION PIN; got: {stderr}"
    );
}

// ─── AC-10 (NEW): --field --output json error-envelope shape ────────────────

/// AC-10 (BC-3.8.012 `--output json` envelope shape, [mode: --output json]):
/// Pairing/symmetric twin of AC-1 ([mode: human]) for the same invocation
/// class.
#[tokio::test]
async fn test_platform_create_field_without_request_type_json_error_shape() {
    let cwd_dir = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    common::fixtures::write_profile_config(config_dir.path(), &server.uri());

    mount_platform_create_stubs(&server).await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd_dir.path())
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "a=b",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    common::assertions::assert_json_error_envelope(&output, 64, "BC-3.8.012 / AC-10");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(
        parsed["code"].as_i64(),
        Some(64),
        "BC-3.8.012 / AC-10: code field must be 64; got: {parsed}"
    );
    assert!(
        parsed["error"]
            .as_str()
            .is_some_and(|s| s.contains("--field is only valid with")),
        "BC-3.8.012 / AC-10: error field must contain the single-flag prefix pin; got: {parsed}"
    );
    assert!(
        stdout.trim().is_empty(),
        "BC-3.8.012 / AC-10: DISCRIMINATING — stdout must be empty; got: {stdout}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.012 / AC-10: REGRESSION PIN; got: {stderr}"
    );
}

// ─── AC-11 (NEW): --field in TTY/interactive mode exits 64 before prompt ────

/// AC-11 (BC-3.8.012 mode-agnosticism, [mode: human/TTY]): `--field a=b`
/// WITHOUT `--project`, WITHOUT `--request-type`, and WITHOUT `--no-input`
/// exits 64 BEFORE any interactive prompt fires, even with
/// `JR_STDIN_IS_TTY=1` (debug seam suppressing the auto-`--no-input` flip on
/// non-TTY stdin).
///
/// Non-goal: dialoguer 0.12 `interact_text()` short-circuits on non-TTY
/// stderr under `assert_cmd`; the true PTY-interactive branch is untestable
/// without a PTY harness. AC-11's unique value is exercising the
/// `JR_STDIN_IS_TTY=1` no-auto-flip code path itself.
#[tokio::test]
async fn test_platform_create_field_interactive_tty_exits_64_before_prompt() {
    let cwd_dir = tempfile::tempdir().unwrap();
    // Bare MockServer, no registered handlers — expect(0) mocks are
    // NON-DISCRIMINATING here (guard-absent also fails before reaching HTTP,
    // via the project-resolution or prompt path); the discriminating proof
    // is the "Project key" absence + presence of the guard string below.
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd_dir.path())
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .env("JR_STDIN_IS_TTY", "1")
        .args(["issue", "create", "--field", "a=b"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-11: positive guard assertion; got: {stderr}"
    );
    assert!(
        !stderr.contains("Project key"),
        "BC-3.8.012 / AC-11: DISCRIMINATING — guard must fire BEFORE project-key resolution; got: {stderr}"
    );
    assert!(
        !stderr.contains("Created issue"),
        "BC-3.8.012 / AC-11: HYGIENE — structurally unreachable without a project; got: {stderr}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.012 / AC-11: REGRESSION PIN — old S-383 warn string must not appear; got: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.012 / AC-11: HYGIENE (guard-absent also exits 64 on the eventual project error; \
         items 1+2 above are what discriminate); got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.trim().is_empty(),
        "BC-3.8.012 / AC-11: output-channel hygiene; got: {stdout}"
    );
}

// ─── AC-12 (NEW): --help pins "requires --request-type" on BOTH flags ───────

/// AC-12 (BC-3.8.012 delivery item (d) — help text first-line update,
/// [mode: human help]): `jr issue create --help` must contain
/// "requires --request-type" for BOTH the `--field` and `--on-behalf-of`
/// entries. A single `stdout.contains(…)` would pass with only one flag
/// updated — the `.count() == 2` form is required to pin BOTH.
#[tokio::test]
async fn test_platform_create_help_flags_requires_request_type_in_help() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "create", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Normalization is MANDATORY — clap 4 next-line layout may wrap long doc
    // strings, causing the substring to straddle a newline.
    let normalized = stdout.split_whitespace().collect::<Vec<_>>().join(" ");

    assert_eq!(
        normalized.matches("requires --request-type").count(),
        2,
        "BC-3.8.012 / AC-12: 'requires --request-type' must appear exactly twice \
         (once for --field, once for --on-behalf-of) in whitespace-normalized help; \
         got normalized help: {normalized}"
    );
}

// ─── AC-13 (NEW): empty --on-behalf-of + --field → combined, not two singles ─

/// AC-13 (BC-3.8.012 EC-3.8.012-1 — combined-check ordering with empty
/// `--on-behalf-of`, [mode: human]): `--on-behalf-of "" --field a=b` WITHOUT
/// `--request-type` fires the COMBINED error, not two independent single-flag
/// errors — `""` is still `Some("")`, i.e. `is_some()` is true. Dedicated
/// isolated `MockServer` (not `mount_platform_create_stubs`) so the
/// zero-HTTP proof is DISCRIMINATING against the would-otherwise-succeed
/// guard-absent path.
#[tokio::test]
async fn test_platform_create_combined_empty_on_behalf_with_field_exits_64_combined_error() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // Would-otherwise-succeed precondition (inlined, not via
    // mount_platform_create_stubs, to keep this MockServer dedicated).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::Value::Array(vec![])))
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
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--on-behalf-of",
            "",
            "--field",
            "a=b",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("--field and --on-behalf-of are only valid with"),
        "BC-3.8.012 / AC-13: combined error must be present; got: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.012 / AC-13: expected exit 64; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-13: FALSIFIABLE-COARSE — single-flag guard must NOT fire instead of combined; got: {stderr}"
    );
    assert!(
        !stderr.contains("--on-behalf-of is only valid with"),
        "BC-3.8.013 / AC-13: FALSIFIABLE-COARSE — single-flag guard must NOT fire instead of combined; got: {stderr}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.012+013 / AC-13: REGRESSION PIN — DISCRIMINATING (this invocation previously \
         emitted BOTH old S-383 warn strings); got: {stderr}"
    );
    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "BC-3.8.012 / AC-13: NORMATIVE zero-HTTP proof — would-otherwise-succeed, so a \
         guard-absent implementation would have reached HTTP"
    );
}

// ─── AC-14 (NEW): empty --request-type routes to JSM, not BC-3.8.012 ────────

/// AC-14 (BC-3.8.012 EC-3.8.012-2 — routing guard is JSM-fork-agnostic,
/// [mode: human]): `--project PROJ --field a=b --request-type ""` routes to
/// the JSM dispatch fork (since `request_type.is_some()` is true for `""`)
/// and fires the BC-3.8.016 empty-request-type guard, NOT BC-3.8.012.
/// `--project PROJ` is REQUIRED: `handle_jsm_create` resolves the project key
/// BEFORE the empty-request-type guard.
#[tokio::test]
async fn test_platform_create_empty_request_type_routes_jsm_not_bc_3_8_012() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
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
            "issue",
            "create",
            "--project",
            "PROJ",
            "--field",
            "a=b",
            "--request-type",
            "",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("request type cannot be empty"),
        "BC-3.8.012 / AC-14: POSITIVE — BC-3.8.016 empty-request-type guard must fire; got: {stderr}"
    );
    assert!(
        !stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-14: DISCRIMINATING — BC-3.8.012 would fire here if the platform-path \
         guard preceded the JSM dispatch fork; got: {stderr}"
    );
}

// ─── AC-15 (NEW): clap conflicts_with exits 2, not 64 ────────────────────────

/// AC-15 (BC-3.8.012 EC-3.8.012-8 — clap parse-level rejection precedes
/// `handle_create`, [mode: human]): `--field a=b --to me --account-id X`
/// (clap `conflicts_with` pair: `--to` conflicts with `--account-id`) exits 2
/// (clap parse error), NOT 64. The guard is structurally unreachable on any
/// clap-rejected invocation.
#[tokio::test]
async fn test_platform_create_conflicting_flags_exit_2_not_64_clap_precedence() {
    let server = MockServer::start().await;
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "create",
            "--field",
            "a=b",
            "--to",
            "me",
            "--account-id",
            "X",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(2),
        "BC-3.8.012 / AC-15: expected clap exit 2 (not 64); got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-15: HYGIENE — handle_create is never entered on a clap-rejected \
         invocation; got: {stderr}"
    );
}

// ─── AC-16 (NEW): --on-behalf-of "" alone fires BC-3.8.013 ──────────────────

/// AC-16 (BC-3.8.013 EC-3.8.013-1 — empty string value is still `is_some()`,
/// [mode: human]): `--on-behalf-of ""` alone (no `--field`, no
/// `--request-type`) exits 64 with the verbatim BC-3.8.013 single-flag error.
#[tokio::test]
async fn test_platform_create_on_behalf_empty_string_exits_64_013_error() {
    let cwd_dir = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd_dir.path())
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args(["issue", "create", "--on-behalf-of", "", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.013 / AC-16: expected exit 64; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("--on-behalf-of is only valid with"),
        "BC-3.8.013 / AC-16: prefix pin must appear; got: {stderr}"
    );
    assert!(
        stderr.contains(
            "--on-behalf-of is only valid with --request-type (JSM service-desk requests). Add --request-type <NAME> to raise a request on behalf of another user, or drop --on-behalf-of to create a standard platform issue."
        ),
        "BC-3.8.013 / AC-16: FULL-STRING verbatim single-flag error must appear on stderr; got: {stderr}"
    );
    assert!(
        !stderr.contains("--field and --on-behalf-of are only valid with"),
        "BC-3.8.013 / AC-16: FALSIFIABLE-COARSE — combined guard must NOT mis-fire when --field is absent; got: {stderr}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.013 / AC-16: REGRESSION PIN — DISCRIMINATING (this invocation previously \
         emitted the old S-383 warn string); got: {stderr}"
    );
}

// ─── AC-17 (NEW): --markdown + --field exits 64 (BC-3.8.012), not markdown err ─

/// AC-17 (BC-3.8.012 EC-3.8.012-5 — guard fires before `--markdown`→ADF
/// conversion, [mode: human]): `--markdown --field description=x` WITHOUT
/// `--request-type` exits 64 with the BC-3.8.012 error, NOT the JSM-path
/// `--markdown` conflict error (that string lives only inside
/// `handle_jsm_create`, structurally unreachable without `--request-type`
/// routing).
#[tokio::test]
async fn test_platform_create_markdown_with_field_exits_64_bc_3_8_012_not_markdown_error() {
    let cwd_dir = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd_dir.path())
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--markdown",
            "--field",
            "description=x",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.012 / AC-17: expected exit 64; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-17: positive guard assertion; got: {stderr}"
    );
    assert!(
        !stderr.contains("Project key"),
        "BC-3.8.012 / AC-17: DISCRIMINATING — guard must fire BEFORE project-key resolution; got: {stderr}"
    );
    assert!(
        !stderr.contains("cannot be combined with `--markdown`"),
        "BC-3.8.012 / AC-17: HYGIENE — the JSM-path --markdown conflict string is structurally \
         unreachable without --request-type routing; got: {stderr}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.012 / AC-17: REGRESSION PIN; got: {stderr}"
    );
}

// ─── AC-18 (NEW): --description-stdin + --field exits 64, stdin not consumed ─

/// AC-18 (BC-3.8.012 EC-3.8.012-7 — guard fires before the
/// `--description-stdin` blocking read, [mode: human]): `--field a=b
/// --description-stdin` WITHOUT `--request-type` exits 64 before the blocking
/// stdin read at step 4a. Stdin is piped (non-TTY) with some content; if the
/// guard did NOT fire first, the read would consume it and the
/// would-otherwise-succeed platform POST would proceed.
#[tokio::test]
async fn test_platform_create_description_stdin_with_field_exits_64_stdin_not_consumed() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_platform_create_stubs(&server).await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "a=b",
            "--description-stdin",
            "--no-input",
        ])
        .write_stdin("some description content\n")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.012 / AC-18: expected exit 64; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-18: positive guard assertion; got: {stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "BC-3.8.012 / AC-18: stdout must be empty (HYGIENE); got: {stdout}"
    );
    assert!(
        !stderr.contains("Created issue"),
        "BC-3.8.012 / AC-18: DISCRIMINATING — would-otherwise-succeed, so a guard-absent \
         implementation would have reached the success path; got: {stderr}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.012 / AC-18: REGRESSION PIN; got: {stderr}"
    );
}

// ─── AC-19 (NEW): --field a= (empty value) still fires BC-3.8.012 ───────────

/// AC-19 (BC-3.8.012 EC-3.8.012-9 — key-present empty-value still triggers
/// guard, [mode: human]): `--field a=` (key present, empty value after `=`)
/// WITHOUT `--request-type` exits 64. The guard fires on
/// `!field_pairs.is_empty()` (presence-only) — value contents are never
/// inspected at guard stage. Distinct from EC-3.8.012-3's malformed-no-equals
/// class (AC-7).
#[tokio::test]
async fn test_platform_create_field_empty_value_exits_64_bc_3_8_012() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_platform_create_stubs(&server).await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "a=",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.012 / AC-19: expected exit 64; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-19: positive guard assertion; got: {stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "BC-3.8.012 / AC-19: stdout must be empty (HYGIENE); got: {stdout}"
    );
    assert!(
        !stderr.contains("Created issue"),
        "BC-3.8.012 / AC-19: DISCRIMINATING — no success path must have executed; got: {stderr}"
    );
    assert!(
        !stderr.contains("is ignored on the platform create path"),
        "BC-3.8.012 / AC-19: REGRESSION PIN — DISCRIMINATING (this invocation previously \
         triggered the old S-383 warn string); got: {stderr}"
    );
}

// ─── AC-20 (NEW): JSM path + --on-behalf-of does NOT fire BC-3.8.013 ────────

/// AC-20 (BC-3.8.013 JSM-path non-mis-fire, [mode: --output json]): When
/// `--request-type` IS set alongside `--on-behalf-of <ID>`, the command takes
/// the JSM path and neither guard may fire.
#[tokio::test]
async fn test_jsm_create_with_on_behalf_of_and_request_type_does_not_fire_bc_3_8_013() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--summary",
            "test",
            "--on-behalf-of",
            "X",
            "--request-type",
            "Password Reset",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.013 / AC-20: expected exit 0 on JSM path; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("--on-behalf-of is only valid with"),
        "BC-3.8.013 / AC-20: DISCRIMINATING — single-flag guard must NOT fire on JSM path; got: {stderr}"
    );
    assert!(
        !stderr.contains("--field is only valid with"),
        "BC-3.8.013 / AC-20: HYGIENE — invocation has no --field; got: {stderr}"
    );
    assert!(
        !stderr.contains("--field and --on-behalf-of are only valid with"),
        "BC-3.8.013 / AC-20: FALSIFIABLE-COARSE — combined guard must NOT fire when only \
         --on-behalf-of is present; got: {stderr}"
    );
}

// ─── AC-21 (NEW): JSM path + BOTH flags does NOT fire either guard ──────────

/// AC-21 (BC-3.8.012 + BC-3.8.013 combined JSM-path non-mis-fire,
/// [mode: --output json]): The ONLY invocation falsifying the COMBINED guard
/// on the JSM path — `--project HELP --summary test --field a=b
/// --on-behalf-of X --request-type "Password Reset"` (BOTH `--field` AND
/// `--on-behalf-of` with `--request-type`) must not fire ANY of the three
/// guard error strings.
#[tokio::test]
async fn test_jsm_create_with_both_flags_and_request_type_does_not_fire_guards() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--summary",
            "test",
            "--field",
            "a=b",
            "--on-behalf-of",
            "X",
            "--request-type",
            "Password Reset",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.012+013 / AC-21: expected exit 0 on JSM path; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("--field is only valid with"),
        "BC-3.8.012 / AC-21: DISCRIMINATING — single-flag guard must NOT fire; got: {stderr}"
    );
    assert!(
        !stderr.contains("--on-behalf-of is only valid with"),
        "BC-3.8.013 / AC-21: DISCRIMINATING — single-flag guard must NOT fire; got: {stderr}"
    );
    assert!(
        !stderr.contains("--field and --on-behalf-of are only valid with"),
        "BC-3.8.012+013 / AC-21: DISCRIMINATING — combined guard must NOT fire; this is the \
         discriminating negative AC-6 and AC-20 cannot provide; got: {stderr}"
    );
}

// ─── S-384 AC-4: Basic-auth scope-mismatch-body 401 on JSM POST → API-token hint ─

/// AC-4 (BC-3.8.014 postcondition 2 — HIGHEST regression risk): When
/// `POST /rest/servicedeskapi/request` returns HTTP 401 with a scope-mismatch
/// body (`{"errorMessages": ["Unauthorized; scope does not match"]}`) and the
/// active auth is Basic, the `handle_jsm_create` map_err MUST REWRITE the
/// incoming `JrError::InsufficientScope` to
/// `JrError::NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }`.
///
/// This pins the non-obvious ordering in `send_inner`: the `"scope does not match"`
/// body check fires BEFORE the `Bearer`-scheme guard, so a Basic-auth 401 with a
/// scope-mismatch body lands as `InsufficientScope` in the `map_err` WITHOUT the
/// rewrite, exposing misleading OAuth language to Basic-auth users. The rewrite
/// suppresses this. This test MUST NOT be skipped.
///
/// Assertions: same as AC-3 — API-token hint present, "write:servicedesk-request"
/// absent, "Insufficient token scope" preamble absent (variant rewritten).
#[tokio::test]
async fn test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_types_password_reset(&server).await;

    // JSM POST returns 401 with scope-mismatch body — the `"scope does not match"`
    // body check in `send_inner` fires BEFORE the `Bearer`-scheme guard, so this body
    // produces `InsufficientScope` even for Basic-auth clients. The map_err
    // rewrite MUST convert it to `NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }`.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        // Basic auth — NOT Bearer. Scope-mismatch body + Basic auth = InsufficientScope
        // in the client, which must be REWRITTEN to NotAuthenticated by the map_err.
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test scope-mismatch rewrite",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit non-zero with exit code 2 (NotAuthenticated after rewrite).
    assert!(
        !output.status.success(),
        "BC-3.8.014 AC-4: expected non-zero exit for Basic-auth scope-mismatch 401; got exit 0. stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "BC-3.8.014 AC-4: expected exit code 2 (NotAuthenticated after InsufficientScope rewrite); stderr: {stderr}"
    );

    // BC-3.8.014 postcondition 2: InsufficientScope is rewritten → API-token hint present.
    assert!(
        stderr.contains("expired or revoked"),
        "BC-3.8.014 AC-4: rewritten hint must contain 'expired or revoked'; got: {stderr}"
    );
    assert!(
        stderr.contains("id.atlassian.com/manage-profile/security/api-tokens"),
        "BC-3.8.014 AC-4: rewritten hint must contain the api-tokens URL; got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "BC-3.8.014 AC-4: rewritten hint must contain 'jr auth login'; got: {stderr}"
    );

    // BC-3.8.014 postcondition 2: OAuth scope language must be absent after rewrite.
    assert!(
        !stderr.contains("write:servicedesk-request"),
        "BC-3.8.014 AC-4: rewritten Basic-auth hint must NOT contain 'write:servicedesk-request'; got: {stderr}"
    );
    // The InsufficientScope Display preamble must be absent (variant was rewritten).
    assert!(
        !stderr.contains("Insufficient token scope"),
        "BC-3.8.014 AC-4: InsufficientScope preamble must be absent after rewrite to NotAuthenticated; got: {stderr}"
    );
}

// ─── S-384 AC-7: require_service_desk Basic-auth 401 (cache miss) → API-token hint ─

/// AC-7 (BC-X.8.006 postconditions 1-3): When `require_service_desk` calls
/// `get_or_fetch_project_meta` on a cache miss and the project GET returns HTTP 401,
/// a NEW `map_err` in `require_service_desk` MUST rewrite any incoming variant to
/// `JrError::NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }` for Basic-auth clients.
///
/// Test setup: isolated `XDG_CACHE_HOME` tempdir (forces cache miss so the live
/// project GET fires); `JR_AUTH_HEADER=Basic <b64>`; project GET returns HTTP 401
/// with the standard expired-token body.
///
/// All three JSM callers (handle_jsm_create, jr queue, jr requesttype) benefit from
/// the map_err in require_service_desk; this test pins the `create` caller path.
#[tokio::test]
async fn test_require_service_desk_basic_auth_401_surfaces_api_token_hint() {
    let server = MockServer::start().await;
    // Isolated XDG_CACHE_HOME — forces cache miss so the live project GET fires.
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // Project GET returns 401 — the canonical pinned arm per BC-X.8.006 Setup.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELP"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": [
                "The access token provided is expired, revoked, malformed, or invalid for other reasons."
            ],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        // Basic auth — deterministic, never enters refresh coordinator.
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        // Fresh isolated cache dir — ensures cache miss and live project GET fires.
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test require_service_desk basic 401",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit non-zero with exit code 2 (NotAuthenticated — require_service_desk errors before JSM POST).
    assert!(
        !output.status.success(),
        "BC-X.8.006 AC-7: expected non-zero exit for Basic-auth project GET 401; got exit 0. stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "BC-X.8.006 AC-7: expected exit code 2 (NotAuthenticated) for Basic-auth project GET 401; stderr: {stderr}"
    );

    // BC-X.8.006 postcondition 1: API-token-expiry hint present (L-288-pr2-02 strict).
    assert!(
        stderr.contains("expired or revoked"),
        "BC-X.8.006 AC-7: stderr must contain 'expired or revoked'; got: {stderr}"
    );
    assert!(
        stderr.contains("id.atlassian.com/manage-profile/security/api-tokens"),
        "BC-X.8.006 AC-7: stderr must contain the api-tokens management URL; got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "BC-X.8.006 AC-7: stderr must contain 'jr auth login' recovery step; got: {stderr}"
    );

    // BC-X.8.006 postcondition 3: OAuth scope language must NOT appear.
    assert!(
        !stderr.contains("write:servicedesk-request"),
        "BC-X.8.006 AC-7: Basic-auth 401 from require_service_desk must NOT surface 'write:servicedesk-request'; got: {stderr}"
    );
}

// ─── S-384 AC-8: require_service_desk OAuth 401 (cache miss, scope-mismatch) → read-scope hint ─

/// AC-8 (BC-X.8.007 postconditions 1-2): When `require_service_desk` calls
/// `get_or_fetch_project_meta` on a cache miss and the project GET returns HTTP 401
/// with a scope-mismatch body, a NEW `map_err` in `require_service_desk` MUST
/// rewrite BOTH `InsufficientScope` and `NotAuthenticated` arms to
/// `JrError::NotAuthenticated { hint }` with the read-side scope hint for OAuth
/// clients (`read:jira-work` + `read:servicedesk-request`).
///
/// WHY scope-mismatch body required (BC-X.8.007 Setup): A Bearer client with a
/// generic-expiry 401 body enters the auto-refresh coordinator in `send_inner`, fails
/// with raw anyhow (not a `JrError`) via the `JR_AUTH_HEADER` seam — the map_err
/// never fires. Scope-mismatch body short-circuits to `InsufficientScope` via the
/// `"scope does not match"` body check in `send_inner` BEFORE the auto-refresh
/// coordinator, deterministically reaching the map_err.
///
/// Assertions: stderr contains `read:jira-work` AND `read:servicedesk-request`;
/// does NOT contain `write:servicedesk-request`.
#[tokio::test]
async fn test_require_service_desk_oauth_401_surfaces_read_scope_hint() {
    let server = MockServer::start().await;
    // Isolated XDG_CACHE_HOME — forces cache miss so the live project GET fires.
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // Project GET returns 401 with scope-mismatch body — the only deterministic
    // Bearer→JrError path via JR_AUTH_HEADER seam (the `"scope does not match"`
    // body check in `send_inner` short-circuits before the auto-refresh coordinator).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELP"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        // Bearer auth — triggers InsufficientScope via scope-mismatch body check.
        .env("JR_AUTH_HEADER", "Bearer test-oauth-token")
        // Fresh isolated cache dir — ensures cache miss and live project GET fires.
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test require_service_desk oauth read-scope hint",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit non-zero with exit code 2 (NotAuthenticated — require_service_desk errors before JSM POST).
    assert!(
        !output.status.success(),
        "BC-X.8.007 AC-8: expected non-zero exit for Bearer scope-mismatch project GET 401; got exit 0. stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "BC-X.8.007 AC-8: expected exit code 2 (NotAuthenticated) for Bearer scope-mismatch project GET 401; stderr: {stderr}"
    );

    // BC-X.8.007 postcondition 1: read-side scope hint present (L-288-pr2-02 strict).
    assert!(
        stderr.contains("read:jira-work"),
        "BC-X.8.007 AC-8: stderr must contain 'read:jira-work' scope hint; got: {stderr}"
    );
    assert!(
        stderr.contains("read:servicedesk-request"),
        "BC-X.8.007 AC-8: stderr must contain 'read:servicedesk-request' scope hint; got: {stderr}"
    );

    // BC-X.8.007 postcondition 2: write scope must NOT appear (write applies to the
    // subsequent POST, not the require_service_desk GET path).
    assert!(
        !stderr.contains("write:servicedesk-request"),
        "BC-X.8.007 AC-8: read-scope hint must NOT contain 'write:servicedesk-request'; got: {stderr}"
    );
}

// ─── S-385 O-08-04: BC-3.8.016 — empty/whitespace --request-type exits 64 ────

/// BC-3.8.016 / H-NEW-JSM-RT-006 (Required Test Deliverable item 1 — S-385):
/// `jr issue create --request-type ""` (empty string) and `--request-type "   "`
/// (whitespace-only) both exit 64 with "request type cannot be empty" on stderr.
///
/// Zero HTTP mocks are mounted. The guard fires at Canonical Guard Ordering step 1,
/// BEFORE `require_service_desk` (step 4). Because `handle_jsm_create` returns before
/// issuing any HTTP call, the binary never contacts the mock server. Ordering regressions
/// (guard moved below step 4) are detected by the exit-code and stderr message assertions:
/// a regression would produce a 404 error or a "requires a Jira Service Management
/// project" message instead of "request type cannot be empty".
///
/// Both the empty-string and whitespace-only inputs are MANDATORY in this test —
/// the whitespace-only case specifically pins the `.trim().is_empty()` guard
/// implementation (EC-3.8.016-1). A guard using `.is_empty()` alone would pass
/// `""` but fail `"   "`.
#[tokio::test]
async fn test_jsm_create_empty_request_type_exits_64() {
    // ── Sub-case A: --request-type "" (empty string) ─────────────────────────
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        // ZERO HTTP mocks mounted. The step-1 guard fires before any HTTP.
        // A regression moving the guard below require_service_desk (step 4) would cause
        // the binary to issue a GET to the mock server; wiremock returns 404, and the
        // test would then fail on the exit-code or stderr message assertions rather than
        // silently passing — so those assertions are the regression detectors here.

        let output = Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("XDG_CACHE_HOME", cache_dir.path())
            .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
            .env("XDG_CONFIG_HOME", config_dir.path())
            .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
            .args([
                "issue",
                "create",
                "--project",
                "HELP",
                "--request-type",
                "",
                "--summary",
                "Test",
                "--no-input",
            ])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Exit-code precondition (BC-3.8.016 postcondition 2).
        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.8.016 (empty string): expected exit 64; got {:?}. stderr: {stderr}",
            output.status.code()
        );

        // Stderr must contain the CANONICAL message (BC-3.8.016 CANONICAL SOURCE).
        assert!(
            stderr.contains("request type cannot be empty"),
            "BC-3.8.016 (empty string): stderr must contain 'request type cannot be empty'; got: {stderr}"
        );

        // Stdout must be empty (BC-3.8.016 postcondition 3).
        assert!(
            stdout.is_empty(),
            "BC-3.8.016 (empty string): stdout must be empty; got: {stdout}"
        );
    }

    // ── Sub-case B: --request-type "   " (whitespace-only) ───────────────────
    // Pins the .trim() call specifically (EC-3.8.016-1). A guard using only
    // .is_empty() would pass sub-case A but fail here.
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        // ZERO HTTP mocks mounted. Guard fires at step 1 before any HTTP. A regression
        // moving the guard below step 4 would produce a 404 from the unmatched HTTP call,
        // which the exit-code and message assertions would catch.

        let output = Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("XDG_CACHE_HOME", cache_dir.path())
            .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
            .env("XDG_CONFIG_HOME", config_dir.path())
            .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
            .args([
                "issue",
                "create",
                "--project",
                "HELP",
                "--request-type",
                "   ",
                "--summary",
                "Test",
                "--no-input",
            ])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Exit-code precondition (BC-3.8.016 postcondition 2).
        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.8.016 (whitespace-only): expected exit 64; got {:?}. stderr: {stderr}",
            output.status.code()
        );

        // Stderr must contain the CANONICAL message (BC-3.8.016 postcondition 1).
        assert!(
            stderr.contains("request type cannot be empty"),
            "BC-3.8.016 (whitespace-only): stderr must contain 'request type cannot be empty'; got: {stderr}"
        );

        // Stdout must be empty (BC-3.8.016 postcondition 3).
        assert!(
            stdout.is_empty(),
            "BC-3.8.016 (whitespace-only): stdout must be empty; got: {stdout}"
        );
    }
}

// ─── S-385 O-08-06: BC-3.8.017 — --markdown + --field description= conflict ──

/// BC-3.8.017 / H-NEW-JSM-RT-007 (Required Test Deliverable item 2 — S-385):
/// `jr issue create --markdown --field description=<value>` exits 64 with the
/// canonical single-sentence conflict message BEFORE `require_service_desk` (step 4).
///
/// Three `contains` checks are performed — they are substring slices of the ONE
/// canonical sentence emitted by the implementation, NOT three separate messages.
/// The full canonical sentence lives in bc-3-issue-write.md BC-3.8.017 body
/// (CANONICAL SOURCE).
///
/// Zero HTTP mocks are mounted. Guard fires at Canonical Guard Ordering step 2,
/// before `require_service_desk`. Because the binary exits before any HTTP call,
/// the mock server is never contacted. Ordering regressions (guard moved below step 4)
/// are detected by the exit-code and stderr message assertions: a regression would
/// produce a 404 or require_service_desk error instead of the conflict message.
///
/// The guard uses a case-SENSITIVE, no-trim raw-key match: the key substring before
/// the first `=` must be exactly `"description"`. `--field Description=X` does NOT
/// trigger it (EC-3.8.017-3).
#[tokio::test]
async fn test_jsm_create_markdown_field_description_conflict_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // ZERO HTTP mocks mounted. Guard fires at step 2 before any HTTP.

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "17",
            "--summary",
            "Reset please",
            "--markdown",
            "--field",
            "description=plain text override",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Exit-code precondition (BC-3.8.017 postcondition 1).
    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.017: expected exit 64 for --markdown + --field description= conflict; got {:?}. stderr: {stderr}",
        output.status.code()
    );

    // Three `contains` assertions covering the ONE canonical sentence from BC-3.8.017
    // body CANONICAL SOURCE. These are test-assertion slices — do NOT assemble the
    // implementation string from these fragments.

    // Slice (a) — identifies the conflicting flag pair.
    assert!(
        stderr.contains("`--field description=...` cannot be combined with `--markdown`"),
        "BC-3.8.017 (slice a): stderr must contain conflict identification; got: {stderr}"
    );

    // Slice (b) — explains the potential harm (may result in desync).
    assert!(
        stderr.contains("may result in a JSM 400 error or silently dropped ADF formatting"),
        "BC-3.8.017 (slice b): stderr must contain desync-harm explanation; got: {stderr}"
    );

    // Slice (c) — remediation clause (pins "errors always suggest what to do next").
    assert!(
        stderr.contains("Pass `--description` with `--markdown`, or omit `--markdown`"),
        "BC-3.8.017 (slice c): stderr must contain remediation clause; got: {stderr}"
    );

    // Stdout must be empty (BC-3.8.017 postcondition 3).
    assert!(
        stdout.is_empty(),
        "BC-3.8.017: stdout must be empty; got: {stdout}"
    );
}

// ─── S-385 BC-3.8.017 negative cases: EC-3.8.017-5 (no-= token) and EC-3.8.017-3 (capital-D) ──

/// BC-3.8.017 EC-3.8.017-5 / EC-3.8.017-3 negative-case regression pins (adversary pass-1 H-1).
///
/// The step-2 conflict guard uses `pair.find('=').is_some_and(|pos| &pair[..pos] == "description")`.
/// Two sub-cases pin the non-triggering boundaries:
///
/// Sub-case A — EC-3.8.017-5: `--field description` (NO `=` in token). The guard must
/// NOT fire because there is no extractable key. No HTTP mocks are mounted; if the
/// step-2 guard wrongly fires, the binary returns exit 64 with the conflict message
/// before issuing any HTTP — which the assertion below will catch. If the guard correctly
/// does NOT fire, the binary proceeds to step 4 (`require_service_desk`) which issues a
/// GET to the mock server; wiremock returns 404, causing the binary to exit with a
/// non-conflict error. The test only asserts absence of the conflict-identification slice,
/// so it passes in either the 404 or later-step-error case. No specific exit code is
/// asserted here (the actual error depends on which step fails first).
///
/// Sub-case B — EC-3.8.017-3: `--field Description=X` (capital D). Guard must NOT fire
/// (case-SENSITIVE exact match — raw key `Description` != `description`). The command
/// will proceed past step 2 and fail elsewhere. Assert stderr does NOT contain the
/// conflict-identification slice.
#[tokio::test]
async fn test_jsm_create_markdown_field_description_conflict_negative_cases() {
    // ── Sub-case A: EC-3.8.017-5 — --field description (no =) must NOT trigger guard ─
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        // No HTTP mocks. If the step-2 conflict guard WRONGLY fires for a no-= token,
        // the binary exits 64 with the conflict message before any HTTP — the assertion
        // below catches this. If the guard correctly does NOT fire, the binary issues
        // a GET to the mock server which returns 404; the binary exits with a different
        // error and the absence assertion still passes.

        let output = Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("XDG_CACHE_HOME", cache_dir.path())
            .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
            .env("XDG_CONFIG_HOME", config_dir.path())
            .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
            .args([
                "issue",
                "create",
                "--project",
                "HELP",
                "--request-type",
                "17",
                "--summary",
                "Reset please",
                "--markdown",
                "--description",
                "some description",
                "--field",
                "description",
                "--no-input",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        // The BC-3.8.017 conflict guard MUST NOT fire for a no-= token (EC-3.8.017-5).
        // The conflict-identification slice must be absent from stderr.
        assert!(
            !stderr.contains("`--field description=...` cannot be combined with `--markdown`"),
            "EC-3.8.017-5: guard must NOT fire for --field token with no '='; \
             conflict message wrongly appeared. stderr: {stderr}"
        );
    }

    // ── Sub-case B: EC-3.8.017-3 — --field Description=X (capital D) must NOT trigger ─
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        // No HTTP mocks. Guard fires at step 2 if it triggers; assert it does not.

        let output = Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("XDG_CACHE_HOME", cache_dir.path())
            .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
            .env("XDG_CONFIG_HOME", config_dir.path())
            .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
            .args([
                "issue",
                "create",
                "--project",
                "HELP",
                "--request-type",
                "17",
                "--summary",
                "Reset please",
                "--markdown",
                "--description",
                "some description",
                "--field",
                "Description=some value",
                "--no-input",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        // The BC-3.8.017 conflict guard MUST NOT fire for capital-D key (EC-3.8.017-3).
        assert!(
            !stderr.contains("`--field description=...` cannot be combined with `--markdown`"),
            "EC-3.8.017-3: guard must NOT fire for --field Description=X (capital D); \
             conflict message wrongly appeared. stderr: {stderr}"
        );
    }
}

// ─── S-385 O-08-07: BC-3.8.010/011 — platform-flag warning suppressed on non-JSM ─

/// BC-3.8.010 / BC-3.8.011 (O-08-07 warning-position) / Required Test Deliverable
/// item 3 — S-385: When a non-JSM project is used with `--request-type <non-empty>`
/// and the `--type` platform-only flag, the warning MUST NOT appear on stderr.
/// Only the `require_service_desk` non-JSM project error is emitted (exit 64).
///
/// Mock topology: H-NEW-JSM-RT-002 (non-JSM project meta, software typeKey, service
/// desk list NOT called because project meta check short-circuits first).
///
/// Exit-code precondition: assert exit 64 FIRST. This verifies the test reached
/// `require_service_desk` (step 4) and failed there — not at step 1 (empty RT)
/// or step 2/3 (other guards). If the test exits at step 1, the warning-suppression
/// assertion would be trivially true for the wrong reason.
///
/// A non-empty `--request-type` value is used ("Get IT Help") so the BC-3.8.016
/// step-1 guard does NOT fire. The step-4 `require_service_desk` guard fires on
/// the non-JSM project check.
///
/// After O-08-07: the entire pre-dispatch warning block (lines ~64-96 in pre-#385
/// code) is removed. Warnings exist at exactly ONE site — step 5 inside
/// `handle_jsm_create` AFTER `require_service_desk` returns Ok. On a non-JSM
/// project, `require_service_desk` returns Err at step 4, so step 5 is never
/// reached — warnings are suppressed.
#[tokio::test]
async fn test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // H-NEW-JSM-RT-002 mock topology: non-JSM project (software typeKey).
    // require_service_desk checks projectTypeKey first and returns UserError immediately
    // for non-service_desk projects — no service desk list GET is issued.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "100",
            "key": "PROJ",
            "projectTypeKey": "software",
            "simplified": false
        })))
        .mount(&server)
        .await;

    // POST endpoints MUST NOT be called (expect(0)).
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--request-type",
            "Get IT Help",
            "--type",
            "Bug",
            "--summary",
            "VPN broken",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Exit-code precondition (MUST assert exit 64 FIRST — verifies step-4 was reached).
    // This precondition ensures the warning-suppression assertion is non-trivial.
    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.8.010 O-08-07: expected exit 64 from require_service_desk on non-JSM project; \
         got {:?}. stderr: {stderr}",
        output.status.code()
    );

    // Non-JSM project error MUST appear on stderr (verifies the correct failure site).
    assert!(
        stderr.contains(
            "`jr issue create --request-type` requires a Jira Service Management project"
        ),
        "BC-3.8.010 O-08-07: stderr must contain the non-JSM project error; got: {stderr}"
    );

    // Platform-flag warning MUST NOT appear (warning-suppression pin).
    // After O-08-07: warnings fire at step 5 (AFTER require_service_desk succeeds).
    // On a non-JSM project, require_service_desk fails at step 4 → step 5 never reached.
    let warning_count = stderr.matches("warning: --type is ignored").count();
    assert_eq!(
        warning_count, 0,
        "BC-3.8.010 O-08-07: --type warning must NOT appear on stderr for non-JSM project \
         (warning-suppression pin); found {warning_count} occurrence(s). stderr: {stderr}"
    );
}

// ─── S-385 O-08-07: BC-3.8.010/011 single-site — no double-emission on success ─

/// BC-3.8.010 / BC-3.8.011 single-site requirement (F-02) / Required Test
/// Deliverable item 7 — S-385: On a successful JSM create path, each platform-only
/// flag warning MUST appear EXACTLY ONCE on stderr. Double-emission from two code
/// sites is a defect pinned here.
///
/// `--to` and `--account-id` are clap-mutually-exclusive on `issue create`, so all
/// six flags cannot appear in a single invocation. Two invocations are required:
///
/// Invocation A: carries --type --team --points --parent --to (5 flags)
///   → assert exit 0, then assert each of the 5 warning substrings count == 1
/// Invocation B: carries --account-id (1 flag)
///   → assert exit 0, then assert --account-id warning count == 1
///
/// CRITICAL: exit-code precondition (exit 0) MUST be asserted BEFORE warning counts.
/// A non-zero exit means step 5 (warnings) was never reached — the count assertions
/// would then be trivially 0 and silently void the double-emission pin.
///
/// CRITICAL: assertion mechanism is occurrence COUNT, NOT plain `contains`.
/// A plain `contains` passes whether a warning appears once or twice, making it
/// unable to detect the double-emission defect this test exists to catch.
///
/// Full JSM success-path mock set is required for both invocations (same topology
/// as H-NEW-JSM-RT-004 / test_jsm_create_type_flag_ignored_with_warning).
#[tokio::test]
async fn test_jsm_create_platform_flag_warnings_emit_once_on_success() {
    // ── Invocation A: --type --team --points --parent --to (5 flags) ──────────
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        // Full JSM success-path mocks (same topology as H-NEW-JSM-RT-004).
        mount_project_meta_help(&server).await;
        mount_service_desk_list(&server).await;
        mount_request_type_list(&server).await;

        Mock::given(method("POST"))
            .and(path("/rest/servicedeskapi/request"))
            .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
            .expect(1)
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
            .args([
                "issue",
                "create",
                "--project",
                "HELP",
                "--request-type",
                "Password Reset",
                "--summary",
                "test",
                "--no-input",
                "--output",
                "json",
                "--type",
                "Bug",
                "--team",
                "team-abc",
                "--points",
                "3",
                "--parent",
                "HELP-1",
                "--to",
                "account-id-xyz",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Exit-code precondition: MUST assert exit 0 BEFORE warning counts.
        // Non-zero exit means step 5 never reached → counts would be trivially 0.
        assert!(
            output.status.success(),
            "BC-3.8.010/011 F-02 invocation-A: expected exit 0 (success path); \
             got {:?}. stderr: {stderr}",
            output.status.code()
        );

        // Occurrence count assertions — NOT plain `contains`.
        // Each warning MUST appear EXACTLY ONCE (single-site requirement F-02).

        let count_type = stderr
            .matches("warning: --type is ignored when --request-type is set")
            .count();
        assert_eq!(
            count_type, 1,
            "BC-3.8.010 F-02 invocation-A: expected exactly 1 --type warning; got {count_type}. stderr: {stderr}"
        );

        let count_team = stderr
            .matches("warning: --team is ignored when --request-type is set")
            .count();
        assert_eq!(
            count_team, 1,
            "BC-3.8.011 F-02 invocation-A: expected exactly 1 --team warning; got {count_team}. stderr: {stderr}"
        );

        let count_points = stderr
            .matches("warning: --points is ignored when --request-type is set")
            .count();
        assert_eq!(
            count_points, 1,
            "BC-3.8.011 F-02 invocation-A: expected exactly 1 --points warning; got {count_points}. stderr: {stderr}"
        );

        let count_parent = stderr
            .matches("warning: --parent is ignored when --request-type is set")
            .count();
        assert_eq!(
            count_parent, 1,
            "BC-3.8.011 F-02 invocation-A: expected exactly 1 --parent warning; got {count_parent}. stderr: {stderr}"
        );

        let count_to = stderr
            .matches("warning: --to is ignored when --request-type is set")
            .count();
        assert_eq!(
            count_to, 1,
            "BC-3.8.011 F-02 invocation-A: expected exactly 1 --to warning; got {count_to}. stderr: {stderr}"
        );
    }

    // ── Invocation B: --account-id (1 flag, clap-exclusive with --to) ─────────
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        // Full JSM success-path mocks.
        mount_project_meta_help(&server).await;
        mount_service_desk_list(&server).await;
        mount_request_type_list(&server).await;

        Mock::given(method("POST"))
            .and(path("/rest/servicedeskapi/request"))
            .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
            .expect(1)
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
            .args([
                "issue",
                "create",
                "--project",
                "HELP",
                "--request-type",
                "Password Reset",
                "--summary",
                "test",
                "--no-input",
                "--output",
                "json",
                "--account-id",
                "account-id-xyz",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Exit-code precondition: MUST assert exit 0 BEFORE warning count.
        assert!(
            output.status.success(),
            "BC-3.8.011 F-02 invocation-B: expected exit 0 (success path); \
             got {:?}. stderr: {stderr}",
            output.status.code()
        );

        // --account-id warning MUST appear EXACTLY ONCE (single-site requirement F-02).
        let count_account_id = stderr
            .matches("warning: --account-id is ignored when --request-type is set")
            .count();
        assert_eq!(
            count_account_id, 1,
            "BC-3.8.011 F-02 invocation-B: expected exactly 1 --account-id warning; \
             got {count_account_id}. stderr: {stderr}"
        );
    }
}

// ─── H-NEW-ADF-010 Call E: JSM path parity (BC-7.2.015 EC-4) ─────────────────

/// H-NEW-ADF-010 Call E (BC-7.2.015 JSM path parity): `^\`code\`^` submitted
/// via `handle_jsm_create` — `subsup` must be stripped from the code text node
/// in `requestFieldValues.description` exactly as it is in `fields.description`
/// on the platform path.
///
/// `markdown_to_adf` and `push_code` are the single shared conversion engine
/// invoked by both `handle_create` (platform, ADR-0014 upstream fork) and
/// `handle_jsm_create` (ADR-0014 JSM fork). This test confirms the exclusivity
/// invariant holds regardless of which downstream endpoint receives the POST.
///
/// Five mocks:
///   1. GET /rest/api/3/project/HELPDESK — `require_service_desk` project-meta fetch.
///   2. GET /rest/servicedeskapi/servicedesk — service desk list; matches on projectId "77".
///   3. GET /rest/servicedeskapi/servicedesk/3/requesttype?start=0&limit=50 — RT discovery.
///   4. POST /rest/servicedeskapi/request `.expect(1)` — JSM submit.
///   5. POST /rest/api/3/issue `.expect(0)` — platform endpoint must NOT be called.
///
/// BC-7.2.015 EC-4, H-NEW-ADF-010 Call E.
#[tokio::test]
async fn test_bc_7_2_015_call_e_jsm_path_subsup_code_mark_stripped() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // Mock 1: project meta — HELPDESK is a service_desk project with id "77".
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELPDESK"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "77",
            "key": "HELPDESK",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .mount(&server)
        .await;

    // Mock 2: service desk list — entry for project_id "77" has service desk id "3".
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "_links": {},
            "values": [
                {
                    "id": "3",
                    "projectId": "77",
                    "projectName": "Help Desk"
                }
            ]
        })))
        .mount(&server)
        .await;

    // Mock 3: request type list for service desk 3.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/3/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "_links": {},
            "values": [
                {
                    "id": "5",
                    "name": "Get IT Help",
                    "description": "IT support"
                }
            ]
        })))
        .mount(&server)
        .await;

    // Mock 4: JSM POST — exactly once; body is captured via received_requests below.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "issueId": "10042",
            "issueKey": "HELP-42"
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Mock 5: platform endpoint — must NEVER be called (H-NEW-ADF-010 Call E guard).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
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
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "HELPDESK",
            "--request-type",
            "Get IT Help",
            "--summary",
            "jsm-code",
            "--markdown",
            "--description",
            "^`code`^",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Call E: expected exit 0; stderr={stderr:?} stdout={stdout:?}"
    );

    // Capture the POST body to /rest/servicedeskapi/request and inspect
    // requestFieldValues.description for code-mark exclusivity.
    let captured = server
        .received_requests()
        .await
        .expect("wiremock must record received requests");
    let jsm_post = captured
        .iter()
        .find(|r| {
            r.url.path() == "/rest/servicedeskapi/request"
                && r.method == wiremock::http::Method::POST
        })
        .expect("Call E: POST to /rest/servicedeskapi/request must have been captured");

    let body_str =
        std::str::from_utf8(&jsm_post.body).expect("Call E: POST body must be valid UTF-8");
    let body_json: serde_json::Value = serde_json::from_str(body_str)
        .unwrap_or_else(|e| panic!("Call E: POST body must be valid JSON: {e}; body={body_str}"));

    let desc = &body_json["requestFieldValues"]["description"];
    assert!(
        !desc.is_null(),
        "Call E: requestFieldValues.description must be present in POST body; body={body_str}"
    );

    // BC-7.2.015 exclusivity invariant over the JSM ADF.
    assert_code_mark_exclusivity_local(desc);

    // Specific assertion: text "code" carries marks [code] ONLY — NOT [subsup, code].
    let mut text_nodes: Vec<&serde_json::Value> = Vec::new();
    collect_text_nodes_local(desc, &mut text_nodes);
    let code_node = text_nodes
        .iter()
        .find(|n| n["text"].as_str() == Some("code"))
        .unwrap_or_else(|| {
            panic!(
                "Call E: expected text node 'code' in requestFieldValues.description; desc={desc}"
            )
        });
    let marks = &code_node["marks"];
    assert!(
        has_mark_local(marks, "code"),
        "Call E: 'code' node must carry `code` mark; marks={marks}"
    );
    assert!(
        !has_mark_local(marks, "subsup"),
        "Call E (issue #571 JSM-path regression guard): 'code' node must NOT carry \
         `subsup` mark (stripped by push_code allowlist filter regardless of \
         which endpoint the POST targets); marks={marks}"
    );
    assert_eq!(
        marks.as_array().map(|a| a.len()).unwrap_or(0),
        1,
        "Call E: 'code' node must carry exactly 1 mark ([{{\"type\":\"code\"}}]); marks={marks}"
    );
    // The .expect(0) on the platform mock fires on server drop (enforced by wiremock).
}

// ADF-walking helpers local to Call E (defined here to avoid a cross-module
// helper dependency; mirrors the helpers in `tests/adf_code_mark_exclusivity.rs`).

fn collect_text_nodes_local<'a>(node: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
    if node.get("type").and_then(|t| t.as_str()) == Some("text") {
        out.push(node);
    }
    if let Some(children) = node.get("content").and_then(|c| c.as_array()) {
        for child in children {
            collect_text_nodes_local(child, out);
        }
    }
}

fn has_mark_local(marks: &serde_json::Value, mark_type: &str) -> bool {
    marks
        .as_array()
        .is_some_and(|arr| arr.iter().any(|m| m["type"].as_str() == Some(mark_type)))
}

fn assert_code_mark_exclusivity_local(adf: &serde_json::Value) {
    const FORBIDDEN: &[&str] = &[
        "strong",
        "em",
        "strike",
        "subsup",
        "underline",
        "textColor",
        "backgroundColor",
    ];
    let mut text_nodes = Vec::new();
    collect_text_nodes_local(adf, &mut text_nodes);
    for tn in &text_nodes {
        let marks = &tn["marks"];
        if has_mark_local(marks, "code") {
            for ftype in FORBIDDEN {
                assert!(
                    !has_mark_local(marks, ftype),
                    "BC-7.2.015 (Call E JSM path): text node {:?} carries both `code` \
                     mark and forbidden typographic mark {ftype:?}. marks={marks}",
                    tn["text"]
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// S-578-3: JSM `issue create --field` hint-kind uniformity
// (BC-3.8.008 "Hint-kind uniformity" amendment, VP-578-015/016/022)
//
// Historical (RED gate, now closed): the S-578-1 interim
// `reject_unsupported_hint_kinds` guard that used to reject every `--field
// NAME:kind=VALUE` hinted pair with exit 64 has been removed — both the
// guard call site and its underlying helper are gone from `jsm_create.rs`.
// `JsmRequestBuilder::build()`'s kind-aware dispatch (`compose_id_wire`/
// `compose_name_wire`/`compose_asset_wire` in `src/api/jsm/requests.rs`) and
// `resolve_asset_field_l2` (`src/cli/issue/jsm_create.rs`) are real,
// implemented logic, not `todo!()` stubs. Every test below — HINTED
// (`:id`/`:name`/`:asset`/`:option`) and bare alike — now exercises that
// real dispatch end-to-end and is expected to PASS (GREEN), pinning the
// merged behavior rather than describing a pending Red Gate.
//
// VP-578-016 PARITY-PENDING NOTE: the `:id`/`:name`/`:asset`
// `requestFieldValues` wire shapes asserted below are implemented BY
// ANALOGY to the platform-path shapes (`field_resolve.rs::compose_id_hint`/
// `compose_name_hint`/`compose_asset_hint`) per BC-3.8.008's own explicit
// caveat — this parity is NOT research-confirmed for the JSM
// `requestFieldValues` target. A green run of these tests, once the guard
// is removed and dispatch lands, is NOT proof of live-JSM parity; treat
// VP-578-016 as parity-PENDING until F4/live-JSM validation.
// ═══════════════════════════════════════════════════════════════════════════

// ─── AC-001 (BC-3.8.008 amendment): extra_fields type is FieldValueSpec map ──

/// AC-001: `JsmRequestBuilder.extra_fields` is `&'a HashMap<String,
/// FieldValueSpec>` (not the old `&'a HashMap<String, String>`) — the SAME
/// `parse_field_kv` parser used by every `--field`-accepting call site now
/// feeds this builder directly (no per-call-site parsing divergence).
///
/// `FieldValueSpec`/`FieldValueKind` are `pub(crate)` (crate-internal), so
/// this cannot be asserted via a direct Rust type check from an external
/// integration test — it is exercised behaviorally: a BARE (unhinted)
/// `--field` pair must flow end-to-end through `parse_field_kv` ->
/// `JsmRequestBuilder.extra_fields` -> `build()` successfully (`kind: None`
/// is the `FieldValueSpec` variant the bare form always produces).
///
/// PRE-SATISFIED GREEN at time of writing: the `extra_fields` type change
/// (Task 2) already landed as part of the compilable-stub commit
/// (`7eb89fd6`) that precedes this Red Gate — `src/api/jsm/requests.rs`'s
/// `JsmRequestBuilder.extra_fields` field is already `&'a HashMap<String,
/// FieldValueSpec>`, and `jsm_create.rs` already constructs that map
/// directly from `parse_field_kv`'s output (no intermediate `.value`-only
/// unwrap). This test is a regression pin locking in the already-landed
/// type change, not a new Red Gate failure.
#[tokio::test]
async fn test_bc_3_8_008_bare_field_flows_through_spec_typed_extra_fields() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_20000=plain",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-001: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("AC-001: JSM POST must have been made");
    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("AC-001: POST body must be valid JSON");
    assert_eq!(
        body["requestFieldValues"]["customfield_20000"].as_str(),
        Some("plain"),
        "AC-001: a bare --field pair must flow through the FieldValueSpec-typed \
         extra_fields map end-to-end; got body: {body}"
    );
}

// ─── AC-002 (BC-3.8.008 amendment): build()'s kind-aware dispatch ────────────

/// AC-002: `:id` dispatches through `JsmRequestBuilder::build()`'s
/// kind-aware match to `compose_id_wire`, producing `{"id": "10042"}` on
/// `requestFieldValues` (by analogy to the platform-path shape,
/// `field_resolve.rs::compose_id_hint` — VP-578-016 parity-PENDING).
#[tokio::test]
async fn test_bc_3_8_008_build_kind_aware_dispatch_id() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_30000:id=10042",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-002: ':id' hint must dispatch through build() and exit 0. \
         got exit {:?}. \
         stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("AC-002: JSM POST must have been made");
    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("AC-002: POST body must be valid JSON");
    assert_eq!(
        body["requestFieldValues"]["customfield_30000"],
        json!({"id": "10042"}),
        "AC-002: ':id' hint must produce {{\"id\": \"10042\"}} on requestFieldValues \
         (by analogy to the platform-path shape; VP-578-016 parity-PENDING); \
         got body: {body}"
    );
}

/// AC-002: `:name` dispatches through `build()`'s kind-aware match to
/// `compose_name_wire`, producing `{"name": "High"}` on `requestFieldValues`
/// (by analogy to `field_resolve.rs::compose_name_hint` — VP-578-016
/// parity-PENDING).
#[tokio::test]
async fn test_bc_3_8_008_build_kind_aware_dispatch_name() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_30001:name=High",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-002: ':name' hint must dispatch through build() and exit 0. \
         got exit {:?}. \
         stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("AC-002: JSM POST must have been made");
    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("AC-002: POST body must be valid JSON");
    assert_eq!(
        body["requestFieldValues"]["customfield_30001"],
        json!({"name": "High"}),
        "AC-002: ':name' hint must produce {{\"name\": \"High\"}} on requestFieldValues \
         (by analogy to the platform-path shape; VP-578-016 parity-PENDING); \
         got body: {body}"
    );
}

/// AC-002 / VP-578-015: `kind: None` (bare) and `kind: Some(Option)`
/// (`:option`, non-cascading) both dispatch to the SAME plain-string wrap —
/// `build()`'s match arm is `None | Some(FieldValueKind::Option) =>
/// serde_json::Value::String(spec.value.clone())` (already-landed, real
/// logic per the rustdoc in `src/api/jsm/requests.rs`, not a stub). Bare and
/// `:option`-hinted pairs on DIFFERENT field names must therefore produce
/// byte-identical (plain string) wire values.
#[tokio::test]
async fn test_bc_3_8_008_build_kind_aware_dispatch_option_bare_parity() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "bare_field=BareValue",
            "--field",
            "hinted_field:option=HintedValue",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-002 VP-578-015: bare/:option parity must exit 0. \
         got exit {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("AC-002: JSM POST must have been made");
    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("AC-002: POST body must be valid JSON");
    assert_eq!(
        body["requestFieldValues"]["bare_field"],
        json!("BareValue"),
        "AC-002 VP-578-015: bare form must remain a plain string; got body: {body}"
    );
    assert_eq!(
        body["requestFieldValues"]["hinted_field"],
        json!("HintedValue"),
        "AC-002 VP-578-015: ':option' non-cascading hint must produce the SAME \
         plain-string wrap as the bare form (byte-identical parity, not an \
         object wrap); got body: {body}"
    );
}

// ─── AC-003 (EC-3.8.008-1): cascading '>' is opaque literal on JSM ───────────

/// EC-3.8.008-1: `--field cf:option=Parent>Child` on the JSM path is treated
/// as an OPAQUE literal — JSM has no `>`-split site anywhere in its
/// dispatch (`parse_field_kv` itself never splits on `>`; that split lives
/// only at platform-path call sites, per ADR-0019 §Amendment D3). The whole
/// `"Parent>Child"` substring, `>` included, is wrapped verbatim by the SAME
/// `None | Some(Option) => Value::String(...)` non-cascading arm AC-002
/// pins — i.e. a PLAIN STRING `"Parent>Child"`, not a `{"value": ...}`
/// object.
///
/// NOTE on the story text: S-578-3's own AC-003 prose describes the
/// resulting shape as `{"cf": {"value": "Parent>Child"}}` (an object wrap),
/// which is inconsistent with AC-002's own `{"cf": "V"}` pin for the
/// identical match arm and with the already-landed (non-stub)
/// `src/api/jsm/requests.rs` `build()` code (which this story explicitly
/// forbids modifying) — that arm performs a plain `Value::String` wrap for
/// BOTH `None` and `Some(Option)`, with no object-wrap branch anywhere.
/// This test follows the landed source code (source of truth) rather than
/// the apparently-erroneous story example.
#[tokio::test]
async fn test_ec_3_8_008_1_cascading_greater_than_treated_as_opaque_literal_on_jsm() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_40000:option=Parent>Child",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "EC-3.8.008-1: expected exit 0. got exit {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("EC-3.8.008-1: JSM POST must have been made");
    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("EC-3.8.008-1: POST body must be valid JSON");
    assert_eq!(
        body["requestFieldValues"]["customfield_40000"],
        json!("Parent>Child"),
        "EC-3.8.008-1: the entire 'Parent>Child' substring must be wrapped \
         verbatim as a PLAIN STRING (no '>' split, no object wrap) — matches \
         the landed non-cascading Option match arm; got body: {body}"
    );
}

// ─── AC-004 (EC-3.8.008-2): missing '=' is the pre-existing error ────────────

/// EC-3.8.008-2: `--field cf:option` (no `=` at all) never reaches
/// `parse_field_kv`'s step-2 `:kind` extraction — step 1 (split on the
/// first `=`) fails to find any `=` first, so this resolves to the SAME
/// pre-existing "missing '='" exit-64 error BC-3.8.008's own Errors line
/// documents, NOT a hint-syntax parse error. Applies identically on the
/// platform path (this is `parse_field_kv`'s own step-1 behavior,
/// unaffected by call site or by this story's S-578-3 dispatch amendment).
///
/// PRE-SATISFIED GREEN: `parse_field_kv`'s step-1 "missing '='" check is
/// pre-existing, unrelated to the hint-kind dispatch this story adds — this
/// test is a regression pin, not a Red Gate failure.
#[tokio::test]
async fn test_ec_3_8_008_2_missing_equals_is_preexisting_error_not_hint_parse_error() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "cf:option",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "EC-3.8.008-2: expected exit 64 for missing '=', got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("cf:option"),
        "EC-3.8.008-2: error must mention the malformed pair 'cf:option'; got: {stderr}"
    );
    assert!(
        stderr.contains("NAME=VALUE"),
        "EC-3.8.008-2: error must mention NAME=VALUE format requirement, confirming \
         this is the pre-existing missing-'=' error, not a hint-parse error; got: {stderr}"
    );
    assert!(
        !stderr.contains("unknown field-value kind"),
        "EC-3.8.008-2: must NOT be routed through the ':kind' catalog (BC-3.4.031) — \
         it never reaches step 2; got: {stderr}"
    );
}

// ─── AC-005 (EC-3.8.008-3): malformed-hint catalog fires before any POST ─────

/// EC-3.8.008-3: `parse_field_kv`'s shared unknown-`:kind` exit-64 catalog
/// (BC-3.4.031) fires on the JSM path BEFORE any HTTP POST — `--field
/// cf:bogus=X` (unknown kind tag) exits 64 with ZERO POST to
/// `/rest/servicedeskapi/request`, identically to the platform-path shape.
/// This is a direct consequence of `parse_field_kv` running as a single,
/// request-type-agnostic parse pass before `handle_jsm_create` ever
/// constructs the request body — no separate JSM-specific pre-flight check
/// is needed.
///
/// PRE-SATISFIED GREEN: the unknown-`:kind` catalog check is `parse_field_kv`
/// step 3, pre-existing and unaffected by this story's dispatch amendment —
/// this test is a regression pin, not a Red Gate failure.
#[tokio::test]
async fn test_ec_3_8_008_3_malformed_hint_exits_64_zero_post_on_jsm_path() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "cf:bogus=X",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "EC-3.8.008-3: expected exit 64 for unknown ':kind' tag, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("unknown field-value kind"),
        "EC-3.8.008-3: stderr must route through the BC-3.4.031 unknown-kind catalog; \
         got: {stderr}"
    );
    assert!(
        stderr.contains("option, id, name, asset"),
        "EC-3.8.008-3: stderr must list the closed set of valid kinds; got: {stderr}"
    );
    // The .expect(0) on the POST mock is enforced on server drop — zero HTTP
    // POST must occur before this exit-64.
}

// ─── AC-006 (BC-3.8.008 amendment ':asset' arm): L2 workspace resolution ─────

/// AC-006: an EXPLICIT `WORKSPACE:OBJECTID` `:asset` value composes
/// directly at the L2 call site (`jsm_create.rs`) — NO cache lookup, NO
/// call to `get_or_fetch_workspace_id` — mirroring `edit.rs`'s S-578-2
/// precedent for the platform path. `build()`'s `Some(Asset)` arm then
/// performs PURE array-wrapping of the already-qualified value:
/// `[{"workspaceId":"WS-9","id":"WS-9:777","objectId":"777"}]` (by analogy
/// to `field_resolve.rs::compose_asset_hint`'s platform-path shape —
/// VP-578-016 parity-PENDING). The workspace-discovery GET mock below must
/// receive ZERO hits — the explicit form skips the cache lookup entirely
/// per AC-006.
#[tokio::test]
async fn test_bc_3_8_008_asset_explicit_workspace_l2_composes_no_cache_lookup() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    // Mounted but must receive ZERO hits — explicit WORKSPACE:OBJECTID form
    // must never trigger a cache/API workspace lookup (AC-006).
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{"workspaceId": "should-not-be-fetched"}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_50000:asset=WS-9:777",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-006: expected exit 0. got exit {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let workspace_hits = requests
        .iter()
        .filter(|r| r.url.path() == "/rest/servicedeskapi/assets/workspace")
        .count();
    assert_eq!(
        workspace_hits, 0,
        "AC-006: explicit WORKSPACE:OBJECTID form must NEVER call \
         get_or_fetch_workspace_id (no cache/API lookup); got {workspace_hits} hits"
    );

    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("AC-006: JSM POST must have been made");
    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("AC-006: POST body must be valid JSON");
    assert_eq!(
        body["requestFieldValues"]["customfield_50000"],
        json!([{"workspaceId": "WS-9", "id": "WS-9:777", "objectId": "777"}]),
        "AC-006: explicit :asset form must produce a pure array-wrap of the \
         already-qualified WORKSPACE:OBJECTID pair; got body: {body}"
    );
}

/// AC-006: a BARE `<objectId>` `:asset` value (no `:`) requires the L2 call
/// site to call `get_or_fetch_workspace_id` FIRST (AT MOST ONCE per
/// invocation, mirroring the platform-path invariant) before the array can
/// be composed — `build()` never sees a bare `:asset` value, only the
/// L2-resolved, fully-composed result. The workspace-discovery mock must
/// receive exactly 1 hit.
#[tokio::test]
async fn test_bc_3_8_008_asset_bare_form_l2_resolves_workspace_before_build() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{"workspaceId": "ws-42"}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_50001:asset=888",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-006: expected exit 0. got exit {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let workspace_hits = requests
        .iter()
        .filter(|r| r.url.path() == "/rest/servicedeskapi/assets/workspace")
        .count();
    assert_eq!(
        workspace_hits, 1,
        "AC-006: bare :asset form must call get_or_fetch_workspace_id EXACTLY \
         ONCE; got {workspace_hits} hits"
    );

    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("AC-006: JSM POST must have been made");
    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("AC-006: POST body must be valid JSON");
    assert_eq!(
        body["requestFieldValues"]["customfield_50001"],
        json!([{"workspaceId": "ws-42", "id": "ws-42:888", "objectId": "888"}]),
        "AC-006: bare :asset form must resolve workspaceId via cache/API before \
         composing the array; got body: {body}"
    );
}

// ─── ADV-S578-3-P1-002: malformed `:asset` value negative coverage ──────────
//
// GAP (adversary Pass-1 finding ADV-S578-3-P1-002): the two AC-006 tests
// above cover only WELL-FORMED `:asset` values (explicit
// `WORKSPACE:OBJECTID` and bare `<objectId>`). The malformed-shape catalog
// that BC-3.4.030 EC-3.4.030-3 + BC-3.4.031 EC-2a/EC-2b/EC-2d mandate for the
// platform path — `src/cli/issue/field_resolve.rs::compose_asset_hint`,
// mirrored by `tests/issue_field_hint_kinds.rs::test_bc_3_4_031_ec2a/ec2b/
// ec2c/ec2d/ec3` — was never exercised on the JSM path via BC-3.8.008's
// shared malformed-hint exit-64 catalog. This let a real HIGH impl gap ship:
// `resolve_asset_field_l2` (`jsm_create.rs`) and `compose_asset_wire`
// (`requests.rs`) perform ZERO validation today — a malformed value sails
// straight through the L2 workspace fetch and/or the JSM POST instead of
// being rejected pre-flight, exactly mirroring the platform path's four
// `compose_asset_hint` checks.
//
// `resolve_asset_field_l2` mirrors `compose_asset_hint`'s four checks (empty
// value, empty workspace segment, extra colon, non-numeric/empty objectId)
// BEFORE either the L2 workspace fetch or `build()` — every test below pins
// that pre-flight rejection: exit 64, zero workspace-discovery GET hits, and
// zero JSM POST hits.

/// EC-2a (via BC-3.8.008's shared malformed-hint catalog): `--field
/// cf:asset=` (empty value) must exit 64 with the exact "asset reference
/// cannot be empty" message `compose_asset_hint` uses on the platform path —
/// BEFORE any workspace-discovery GET or JSM POST.
#[tokio::test]
async fn test_ec_3_8_008_asset_empty_value_exits_64_zero_post() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{"workspaceId": "should-not-be-fetched"}]
        })))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_54001:asset=",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "ADV-S578-3-P1-002 EC-2a: expected exit 64 for empty :asset value; \
         got exit {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("asset reference cannot be empty"),
        "ADV-S578-3-P1-002 EC-2a: message must match compose_asset_hint's \
         platform-path wording verbatim; stderr={stderr}"
    );
}

/// EC-2c/EC-2b (via BC-3.8.008's shared malformed-hint catalog): `--field
/// cf:asset=:777` (colon present, empty workspace segment) must exit 64 with
/// the exact "workspace segment cannot be empty" message `compose_asset_hint`
/// uses on the platform path — BEFORE any workspace-discovery GET or JSM
/// POST. This value has a numeric objectId segment ("777"), so the
/// empty-workspace check must fire and take PRECEDENCE over the generic
/// numeric check, exactly as the platform sibling's EC-2c precedence test
/// asserts.
#[tokio::test]
async fn test_ec_3_8_008_asset_empty_workspace_segment_exits_64_zero_post() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{"workspaceId": "should-not-be-fetched"}]
        })))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_54002:asset=:777",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "ADV-S578-3-P1-002 EC-2c/EC-2b: expected exit 64 for empty workspace \
         segment; got exit {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("workspace segment cannot be empty"),
        "ADV-S578-3-P1-002 EC-2c/EC-2b: message must match compose_asset_hint's \
         platform-path wording verbatim; stderr={stderr}"
    );
}

/// EC-2d (via BC-3.8.008's shared malformed-hint catalog): `--field
/// cf:asset=W:Y:Z` (extra colon) must exit 64 with the exact "unexpected
/// extra ':'" message `compose_asset_hint` uses on the platform path —
/// BEFORE any workspace-discovery GET or JSM POST. This must be a DISTINCT
/// message from the generic "objectId must be numeric" error, mirroring the
/// platform sibling's EC-2d precedence test.
#[tokio::test]
async fn test_ec_3_8_008_asset_extra_colon_exits_64_zero_post() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{"workspaceId": "should-not-be-fetched"}]
        })))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_54003:asset=W:Y:Z",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "ADV-S578-3-P1-002 EC-2d: expected exit 64 for extra ':' in :asset \
         value; got exit {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("unexpected extra ':'"),
        "ADV-S578-3-P1-002 EC-2d: message must name the extra-colon mistake \
         specifically (compose_asset_hint's platform-path wording verbatim), \
         not the generic numeric-objectId message; stderr={stderr}"
    );
}

/// EC-3 (via BC-3.8.008's shared malformed-hint catalog): `--field
/// cf:asset=abc` (bare, non-numeric objectId) and `--field
/// cf:asset=WS:abc` (explicit workspace, non-numeric objectId) must both
/// exit 64 with the exact "objectId must be numeric" message
/// `compose_asset_hint` uses on the platform path — BEFORE any
/// workspace-discovery GET or JSM POST.
#[tokio::test]
async fn test_ec_3_8_008_asset_non_numeric_objectid_exits_64_zero_post() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{"workspaceId": "should-not-be-fetched"}]
        })))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(0)
        .mount(&server)
        .await;

    for value in ["abc", "WS:abc"] {
        let field_arg = format!("customfield_54004:asset={value}");
        let output = Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("XDG_CACHE_HOME", cache_dir.path())
            .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
            .env("XDG_CONFIG_HOME", config_dir.path())
            .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
            .args([
                "issue",
                "create",
                "--project",
                "HELP",
                "--request-type",
                "Password Reset",
                "--summary",
                "test",
                "--field",
                &field_arg,
                "--no-input",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "ADV-S578-3-P1-002 EC-3: expected exit 64 for non-numeric \
             objectId; value={value:?}; got exit {:?}. stderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("objectId must be numeric"),
            "ADV-S578-3-P1-002 EC-3: message must match compose_asset_hint's \
             platform-path wording verbatim; value={value:?}; stderr={stderr}"
        );
    }
}

/// EC-2b (adversarial Pass-2 finding P2-001, MEDIUM — mutation-survivability):
/// `--field cf:asset=ws:` (colon present, objectId segment EMPTY, distinct
/// from `WS:abc`'s non-empty-but-non-numeric case above) must exit 64 with
/// the SAME "objectId must be numeric" message `compose_asset_hint` uses on
/// the platform path — BEFORE any workspace-discovery GET or JSM POST. This
/// pins the load-bearing `object_id.is_empty()` half of
/// `resolve_asset_field_l2`'s combined `object_id.is_empty() ||
/// !object_id.chars().all(|c| c.is_ascii_digit())` check (`jsm_create.rs`) —
/// without a test exercising an explicit-workspace value whose objectId
/// segment is empty (as opposed to merely non-numeric), a mutant dropping
/// the `is_empty()` conjunct would let `ws:` fall through to
/// `format!("{workspace_id}:{object_id}")` and POST a malformed
/// `{"objectId":""}` array on `requestFieldValues`, undetected.
#[tokio::test]
async fn test_ec_3_8_008_asset_empty_objectid_with_colon_exits_64_zero_post() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{"workspaceId": "should-not-be-fetched"}]
        })))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_54005:asset=ws:",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "P2-001: expected exit 64 for 'ws:' (empty objectId segment with \
         colon present); got exit {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("objectId must be numeric"),
        "P2-001: message must match compose_asset_hint's platform-path \
         wording verbatim (the object_id.is_empty() conjunct must fire, not \
         fall through to a malformed POST); stderr={stderr}"
    );
}

// ─── AC-007 (BC-3.4.030 taxonomy, VP-578-022): JSM-path independent assertion ─

/// AC-007 (VP-578-022 — 1 of 3 shared call sites; this is `jsm_create.rs`'s
/// OWN independent assertion, NOT "already covered" by S-578-2's edit-path
/// test or S-578-4's create-path test of the same VP): 403/404 from `GET
/// /rest/servicedeskapi/assets/workspace` -> exit 64, "Assets is not
/// available on this Jira site..." (the SAME `get_or_fetch_workspace_id`
/// error mapping every call site shares — `src/api/assets/workspace.rs`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_jsm_path_asset_cold_cache_403_404_assets_unavailable() {
    for status in [403u16, 404u16] {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        mount_project_meta_help(&server).await;
        mount_service_desk_list(&server).await;
        mount_request_type_list(&server).await;

        let _guard = Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/assets/workspace"))
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({
                "errorMessages": ["nope"], "errors": {}
            })))
            .mount_as_scoped(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/rest/servicedeskapi/request"))
            .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
            .expect(0)
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
            .args([
                "issue",
                "create",
                "--project",
                "HELP",
                "--request-type",
                "Password Reset",
                "--summary",
                "test",
                "--field",
                "customfield_60000:asset=456",
                "--no-input",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "AC-007 status={status}: expected exit 64; stderr={stderr}"
        );
        assert!(
            stderr.contains(
                "Assets is not available on this Jira site. Assets requires \
                 Jira Service Management Premium or Enterprise."
            ),
            "AC-007 status={status}: message must match this taxonomy row's \
             specific wording; stderr={stderr}"
        );
    }
}

/// AC-007: `GET /rest/servicedeskapi/assets/workspace` returning 200 with
/// zero entries -> exit 64, "No Assets workspace found on this Jira
/// site...".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_jsm_path_asset_cold_cache_empty_workspace() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    let _guard = Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 0, "start": 0, "limit": 25, "isLastPage": true, "values": []
        })))
        .mount_as_scoped(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_60001:asset=456",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-007 empty-workspace: expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains(
            "No Assets workspace found on this Jira site. Assets requires \
             Jira Service Management Premium or Enterprise."
        ),
        "AC-007 empty-workspace: message must match this taxonomy row's \
         specific wording; stderr={stderr}"
    );
}

/// AC-007: `GET /rest/servicedeskapi/assets/workspace` returning 401 must
/// use the STANDARD `JrError::NotAuthenticated` mapping (exit 2) — not a
/// bespoke Assets-specific mapping.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_jsm_path_asset_cold_cache_401_standard_auth_mapping() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    let _guard = Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount_as_scoped(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(0)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "customfield_60002:asset=456",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "AC-007 401: 401 must use the standard NotAuthenticated mapping \
         (exit 2); stderr={stderr}"
    );
    assert!(stderr.contains("Not authenticated"), "stderr={stderr}");
}

/// AC-007: `GET /rest/servicedeskapi/assets/workspace` returning 5xx, and a
/// network-unreachable base URL, both use the STANDARD `ApiError`/
/// `NetworkError` mapping (exit 1).
///
/// Sub-case (a) 5xx: the workspace-discovery GET returns 500 and the
/// command must exit 1 via the standard `ApiError` mapping.
///
/// Sub-case (b) network error: uses a connect-refused base URL
/// (`http://127.0.0.1:1`, matching the established convention in
/// `tests/assets_errors.rs` and the S-578-2 edit-path taxonomy test). This
/// necessarily exercises the FIRST HTTP call the JSM create flow makes
/// (`require_service_desk`'s project-meta lookup), not exclusively the
/// workspace-discovery GET, since `jr`'s single `JR_BASE_URL` applies to
/// every call — so this sub-case is PRE-SATISFIED GREEN today (the failure
/// occurs before field-hint dispatch is ever reached, identically with or
/// without this story's implementation), demonstrating the same standard
/// NetworkError/exit-1 mapping the underlying `get_or_fetch_workspace_id`
/// machinery shares. This mirrors the identical precedent and caveat in
/// `tests/issue_field_hint_kinds.rs`'s
/// `test_bc_3_4_030_edit_path_asset_cold_cache_5xx_network_standard_mapping`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_jsm_path_asset_cold_cache_5xx_network_standard_mapping() {
    // (a) 5xx.
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        mount_project_meta_help(&server).await;
        mount_service_desk_list(&server).await;
        mount_request_type_list(&server).await;

        Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/assets/workspace"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({
                "errorMessages": ["Internal server error"], "errors": {}
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/rest/servicedeskapi/request"))
            .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
            .expect(0)
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
            .args([
                "issue",
                "create",
                "--project",
                "HELP",
                "--request-type",
                "Password Reset",
                "--summary",
                "test",
                "--field",
                "customfield_60003:asset=456",
                "--no-input",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "AC-007 5xx: 5xx must use the standard ApiError mapping (exit 1); \
             stderr={stderr}"
        );
        assert!(stderr.contains("API error (500)"), "stderr={stderr}");
    }

    // (b) network error — connect-refused (see doc comment above): failure
    // occurs at the FIRST HTTP call, before any field-hint dispatch is
    // reached.
    {
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), "http://127.0.0.1:1");

        let output = Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", "http://127.0.0.1:1")
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("XDG_CACHE_HOME", cache_dir.path())
            .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
            .env("XDG_CONFIG_HOME", config_dir.path())
            .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
            .args([
                "issue",
                "create",
                "--project",
                "HELP",
                "--request-type",
                "Password Reset",
                "--summary",
                "test",
                "--field",
                "customfield_60004:asset=456",
                "--no-input",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "AC-007 network: network error must use the standard NetworkError \
             mapping (exit 1); stderr={stderr}"
        );
        assert!(stderr.contains("Could not reach"), "stderr={stderr}");
    }
}

// ─── AC-008 (VP-578-015): bare-field byte-identity regression pin ────────────

/// AC-008 / VP-578-015: a bare (unhinted) `--field NAME=VALUE` on the JSM
/// create path produces BYTE-IDENTICAL `requestFieldValues` wire output
/// before and after the S-578-3 amendment — the kind-aware dispatch is
/// purely additive for `kind: None`. `summary`/`description`/`priority`/
/// `labels` (BC-3.8.005..007) sit in the SAME `rfv` map and are untouched
/// by this amendment.
///
/// PRE-SATISFIED GREEN: the bare-form arm (`None | Some(Option) =>
/// Value::String(...)`) is unchanged, pre-existing logic — this test is a
/// regression pin, not a Red Gate failure.
#[tokio::test]
async fn test_vp_578_015_bare_field_byte_identical_pre_post_amendment() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
        .expect(1)
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--priority",
            "High",
            "--label",
            "alpha",
            "--field",
            "customfield_70000=BareUnhintedValue",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-008: expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("AC-008: JSM POST must have been made");
    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("AC-008: POST body must be valid JSON");
    let rfv = body
        .get("requestFieldValues")
        .expect("AC-008: requestFieldValues must be present");

    // COMPLETE-MAP equality (VP-578-015 review fix B2): assert the entire
    // requestFieldValues object against the full expected wire shape in one
    // shot, so an added/removed/renamed key OR a wrong value on any existing
    // key (including the exact `labels` contents, not just its length) fails
    // this test. This makes the "BYTE-IDENTICAL" claim in this test's name
    // real rather than a per-key spot-check that an added key could slip
    // past silently.
    assert_eq!(
        rfv,
        &json!({
            "summary": "test",
            "priority": {"name": "High"},
            "labels": ["alpha"],
            "customfield_70000": "BareUnhintedValue"
        }),
        "AC-008 VP-578-015: bare --field must produce a BYTE-IDENTICAL \
         requestFieldValues map to pre-amendment behavior — no added, \
         removed, or changed keys; got rfv: {rfv}"
    );
}

// ─── AC-009 (VP-578-016): :id/:name/:asset wire shapes by analogy ────────────

/// AC-009 / VP-578-016 (DOWNGRADED status per the story: "NOT
/// research-confirmed for any of the three kinds... `:asset` in particular
/// is at least as likely to diverge as `:option` — Assets attribute
/// payloads are the least standardized of the four across Atlassian's JSM
/// vs platform surfaces"). This test asserts the IMPLEMENTED shape (by
/// analogy to the platform-path `:id`/`:name`/`:asset` shapes in
/// `field_resolve.rs::compose_id_hint`/`compose_name_hint`/
/// `compose_asset_hint`) with wiremock.
///
/// **A green run of this test is NOT proof of live-JSM parity.** VP-578-016
/// remains parity-PENDING until F4/live-JSM validation runs against a real
/// JSM instance — do NOT read this test passing as a settled guarantee of
/// Atlassian's actual `requestFieldValues` schema for these three hint
/// kinds.
#[tokio::test]
async fn test_vp_578_016_id_name_asset_jsm_wire_shapes_by_analogy_flagged_unverified() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;
    mount_request_type_list(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(jsm_created_response()))
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
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "test",
            "--field",
            "f_id:id=90001",
            "--field",
            "f_name:name=Urgent",
            "--field",
            "f_asset:asset=WSX:5001",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-009/VP-578-016: expected exit 0 (by-analogy shapes, \
         parity-PENDING). got exit {:?}. \
         stderr: {stderr}",
        output.status.code()
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let jsm_post = requests
        .iter()
        .find(|r| r.url.path() == "/rest/servicedeskapi/request" && r.method.as_str() == "POST")
        .expect("AC-009: JSM POST must have been made");
    let body: Value =
        serde_json::from_slice(&jsm_post.body).expect("AC-009: POST body must be valid JSON");
    let rfv = body
        .get("requestFieldValues")
        .expect("AC-009: requestFieldValues must be present");

    assert_eq!(
        rfv.get("f_id"),
        Some(&json!({"id": "90001"})),
        "AC-009/VP-578-016 (by analogy, parity-PENDING): ':id' shape; got rfv: {rfv}"
    );
    assert_eq!(
        rfv.get("f_name"),
        Some(&json!({"name": "Urgent"})),
        "AC-009/VP-578-016 (by analogy, parity-PENDING): ':name' shape; got rfv: {rfv}"
    );
    assert_eq!(
        rfv.get("f_asset"),
        Some(&json!([{"workspaceId": "WSX", "id": "WSX:5001", "objectId": "5001"}])),
        "AC-009/VP-578-016 (by analogy, parity-PENDING): ':asset' shape; got rfv: {rfv}"
    );
}

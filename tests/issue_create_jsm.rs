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

use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{body_partial_json, method, path, query_param};
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
/// `src/cli/issue/create.rs handle_jsm_create map_err`.
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
/// `src/cli/issue/create.rs handle_jsm_create`.
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
        .env("XDG_CACHE_HOME", cache_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
    // Cache-deletion path hint: the path contains /jr/v1/ and request_types_10.json.
    assert!(
        stderr.contains("/jr/v1/"),
        "H-04: cache-deletion hint must contain '/jr/v1/' path segment; got: {stderr}"
    );
    assert!(
        stderr.contains("request_types_10.json"),
        "H-04: cache-deletion hint must contain 'request_types_10.json' filename; got: {stderr}"
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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

/// M-01 (adversary pass-02-retry) + platform-parity: `--markdown` without
/// `--description` or `--description-stdin` on the JSM path errors with the
/// same verbatim message as the platform path (mirrors lines 333-343 of
/// handle_create). Regression guard for the validation block added in
/// handle_jsm_create at b35bc1a.
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
    // Verbatim match against the platform path's error text (verify against
    // create.rs lines 333-343 if this assertion drifts).
    assert!(
        stderr.contains("--markdown requires --description or --description-stdin to take effect"),
        "M-01 / BC-3.8.006: expected platform-parity validation message; got: {stderr}"
    );
}

// ─── S-383: Platform-path inverse warnings (BC-3.8.012 / BC-3.8.013) ─────────
//
// These tests live in `issue_create_jsm.rs` by the explicit decision in the
// S-383 story file (`.factory/stories/S-383-platform-inverse-warnings.md`
// §"Test File Decision").  They are PLATFORM-PATH tests — no `--request-type`
// flag — co-located here because they cover the inverse symmetry of the
// BC-3.8.011 forward-direction warnings already in this file.
//
// Red Gate: all 7 tests MUST fail against the unmodified implementation
// in `src/cli/issue/create.rs`.  The implementation change (2 `eprintln!`
// guards) is introduced in a subsequent commit.

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

// ─── AC-1: --field on platform path emits BC-3.8.012 warning ─────────────────

/// AC-1 (BC-3.8.012 postcondition 1): `jr issue create --field NAME=VALUE`
/// WITHOUT `--request-type` emits exactly the verbatim BC-3.8.012 warning on
/// stderr.  The platform POST to `/rest/api/3/issue` proceeds; exit code 0.
/// The JSM endpoint is never called.
#[tokio::test]
async fn test_platform_create_field_flag_emits_warning_without_request_type() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
            "NAME=VALUE",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "BC-3.8.012 / AC-1: expected exit 0; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("warning: --field is ignored on the platform create path; it only applies with --request-type (JSM service-desk requests). To pass custom fields to a JSM request type, also supply --request-type."),
        "BC-3.8.012 / AC-1: verbatim warning must appear on stderr; got: {stderr}"
    );
    assert!(
        stdout.contains("PROJ-123"),
        "BC-3.8.012 / AC-1: platform issue key must appear on stdout; got: {stdout}"
    );
    // Warning must NOT bleed onto stdout.
    assert!(
        !stdout.contains("warning: --field is ignored"),
        "BC-3.8.012 / AC-1: warning must be on stderr only, not stdout; got: {stdout}"
    );
    // The .expect(0) on the JSM mock is enforced on server drop.
}

// ─── AC-2: --on-behalf-of on platform path emits BC-3.8.013 warning ──────────

/// AC-2 (BC-3.8.013 postcondition 1): `jr issue create --on-behalf-of <ID>`
/// WITHOUT `--request-type` emits exactly the verbatim BC-3.8.013 warning on
/// stderr.  The platform POST proceeds; exit code 0.  The JSM endpoint is
/// never called.
#[tokio::test]
async fn test_platform_create_on_behalf_of_flag_emits_warning_without_request_type() {
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
            "fake-account-id",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "BC-3.8.013 / AC-2: expected exit 0; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("warning: --on-behalf-of is ignored on the platform create path; it only applies with --request-type (JSM service-desk requests). To raise a request on behalf of another user, also supply --request-type."),
        "BC-3.8.013 / AC-2: verbatim warning must appear on stderr; got: {stderr}"
    );
    assert!(
        stdout.contains("PROJ-123"),
        "BC-3.8.013 / AC-2: platform issue key must appear on stdout; got: {stdout}"
    );
    assert!(
        !stdout.contains("warning: --on-behalf-of is ignored"),
        "BC-3.8.013 / AC-2: warning must be on stderr only, not stdout; got: {stdout}"
    );
}

// ─── AC-3: Both --field + --on-behalf-of emit independent warnings ────────────

/// AC-3 (BC-3.8.012 postcondition 3 + BC-3.8.013 postcondition 3): When both
/// `--field NAME=VALUE` and `--on-behalf-of <ID>` are supplied WITHOUT
/// `--request-type`, BOTH verbatim warnings fire independently on stderr.
/// Each appears at least once.  Ordering is not asserted.  Platform POST
/// proceeds normally; exit code 0.
#[tokio::test]
async fn test_platform_create_both_inverse_flags_emit_independent_warnings() {
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
            "A=1",
            "--on-behalf-of",
            "fake-id",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.012+013 / AC-3: expected exit 0; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("warning: --field is ignored on the platform create path; it only applies with --request-type (JSM service-desk requests). To pass custom fields to a JSM request type, also supply --request-type."),
        "BC-3.8.012 / AC-3: BC-3.8.012 warning must appear on stderr; got: {stderr}"
    );
    assert!(
        stderr.contains("warning: --on-behalf-of is ignored on the platform create path; it only applies with --request-type (JSM service-desk requests). To raise a request on behalf of another user, also supply --request-type."),
        "BC-3.8.013 / AC-3: BC-3.8.013 warning must appear on stderr; got: {stderr}"
    );
}

// ─── AC-4: No inverse flags → no new warnings ────────────────────────────────

/// AC-4 (BC-3.8.012 postcondition 4 + BC-3.8.013 postcondition 4 — negative
/// case): `jr issue create --project PROJ --summary "Foo"` WITHOUT `--field`
/// AND WITHOUT `--on-behalf-of` AND WITHOUT `--request-type` must NOT emit
/// either inverse warning.  Stderr is byte-identical to pre-issue-#383 behavior.
#[tokio::test]
async fn test_platform_create_without_inverse_flags_emits_no_new_warnings() {
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
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Foo",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.012+013 / AC-4: expected exit 0; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("--field is ignored"),
        "BC-3.8.012 / AC-4: BC-3.8.012 warning must NOT appear when --field is absent; got: {stderr}"
    );
    assert!(
        !stderr.contains("--on-behalf-of is ignored"),
        "BC-3.8.013 / AC-4: BC-3.8.013 warning must NOT appear when --on-behalf-of is absent; got: {stderr}"
    );
}

// ─── AC-5: Multiple --field occurrences emit exactly ONE warning ──────────────

/// AC-5 (BC-3.8.012 postcondition 2 — idempotency): `--field A=1 --field A=2
/// --field B=3` WITHOUT `--request-type` emits the BC-3.8.012 warning EXACTLY
/// ONCE — the per-logical-flag-NAME rule means `--field` is one logical flag
/// regardless of how many NAME=VALUE pairs are supplied.
#[tokio::test]
async fn test_platform_create_field_idempotent_one_warning_per_logical_flag() {
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
            "A=1",
            "--field",
            "A=2",
            "--field",
            "B=3",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.012 / AC-5: expected exit 0; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert_eq!(
        stderr
            .matches("warning: --field is ignored on the platform create path")
            .count(),
        1,
        "BC-3.8.012 / AC-5: warning must appear EXACTLY ONCE regardless of --field count; got: {stderr}"
    );
}

// ─── AC-6: JSM path + --field does NOT fire BC-3.8.012 (regression gate) ─────

/// AC-6 (BC-3.8.011 invariant — forward-path regression gate): When
/// `--request-type` IS set alongside `--field NAME=VALUE`, the command takes
/// the JSM path and BC-3.8.012 must NOT fire.  The existing BC-3.8.011
/// forward-direction warning tests remain unaffected by the S-383 change.
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        !stderr.contains("--field is ignored on the platform create path"),
        "BC-3.8.012 / AC-6: BC-3.8.012 warning must NOT fire on JSM path; got: {stderr}"
    );
}

// ─── AC-7: Malformed --field on platform path → one warning, no exit-64 ──────

/// AC-7 (BC-3.8.012 postcondition 5 — malformed --field edge case): When
/// `--field bare-name-no-equals` is supplied WITHOUT `--request-type`, the
/// platform path emits the BC-3.8.012 warning EXACTLY ONCE and proceeds to
/// the platform POST (no exit-64).  Format validation (BC-3.8.008) applies
/// only on the JSM path, not the platform path.
#[tokio::test]
async fn test_platform_create_malformed_field_one_warning_no_exit_64() {
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-3.8.012 / AC-7: expected exit 0 (not 64) for malformed --field on platform path; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert_eq!(
        stderr
            .matches("warning: --field is ignored on the platform create path")
            .count(),
        1,
        "BC-3.8.012 / AC-7: warning must appear EXACTLY ONCE for malformed --field; got: {stderr}"
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
            .env("XDG_CONFIG_HOME", config_dir.path())
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
            .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
            .env("XDG_CONFIG_HOME", config_dir.path())
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
            .env("XDG_CONFIG_HOME", config_dir.path())
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
        .env("XDG_CONFIG_HOME", config_dir.path())
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
            .env("XDG_CONFIG_HOME", config_dir.path())
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
            .env("XDG_CONFIG_HOME", config_dir.path())
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

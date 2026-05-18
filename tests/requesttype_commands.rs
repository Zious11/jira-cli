//! Integration tests for `jr requesttype list` and `jr requesttype fields` commands.
//!
//! Covers AC-001..AC-011 from story S-288-pr2-cli
//! (`.factory/code-delivery/issue-288-pr2-cli/story.md`).
//!
//! Pattern matches `tests/queue.rs`: subprocess + wiremock + assert_cmd.
//! Each test runs the `jr` binary via `assert_cmd::Command::cargo_bin("jr")`,
//! sets `JR_BASE_URL=<wiremock url>`, `JR_AUTH_HEADER=Basic dGVzdDp0ZXN0`.

use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ─── Shared mock fixture helpers ─────────────────────────────────────────────

/// Mount the project-meta GET for project "HELP" returning a service_desk project.
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

/// Mount the project-meta GET for project "SW" returning a software project.
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

/// Fields response for request type 11002 (Password Reset).
fn fields_response_body() -> Value {
    json!({
        "canRaiseOnBehalfOf": true,
        "canAddRequestParticipants": true,
        "requestTypeFields": [
            {
                "fieldId": "summary",
                "name": "What do you need?",
                "description": null,
                "required": true,
                "jiraSchema": { "system": "summary", "type": "string" },
                "validValues": [],
                "visible": true
            },
            {
                "fieldId": "customfield_10000",
                "name": "Nominee",
                "description": null,
                "required": false,
                "jiraSchema": {
                    "custom": "com.atlassian.jira.plugin.system.customfieldtypes:userpicker",
                    "customId": 10000,
                    "type": "user"
                },
                "validValues": [],
                "visible": true
            }
        ]
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

// ─── AC-001: list returns types table ────────────────────────────────────────

/// AC-001: `jr requesttype list --project HELP` calls the correct endpoint and
/// renders a table with Name and Description columns.
///
/// Traces: BC-X.12.001
#[tokio::test]
async fn test_requesttype_list_returns_types_table() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(two_request_types_body()))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["requesttype", "list", "--project", "HELP", "--no-input"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("Get IT Help"),
        "Expected 'Get IT Help' in stdout table, got: {stdout}"
    );
    assert!(
        stdout.contains("Password Reset"),
        "Expected 'Password Reset' in stdout table, got: {stdout}"
    );
    assert!(
        stdout.contains("Name") || stdout.contains("name"),
        "Expected 'Name' column header in stdout table, got: {stdout}"
    );
    assert!(
        stdout.contains("Description") || stdout.contains("description"),
        "Expected 'Description' column header in stdout table, got: {stdout}"
    );
}

// ─── AC-002: --search forwarded as searchQuery param ─────────────────────────

/// AC-002: `jr requesttype list --search password` sends `?searchQuery=password`
/// to the server.
///
/// Traces: BC-X.12.002
#[tokio::test]
async fn test_requesttype_list_search_forwarded_as_query_param() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("searchQuery", "password"))
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
                    "groupIds": ["12"]
                }
            ]
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
            "requesttype",
            "list",
            "--project",
            "HELP",
            "--search",
            "password",
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
        "Expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("Expected valid JSON in stdout, got: {stdout}\nError: {e}"));
    let arr = parsed
        .as_array()
        .expect("Expected JSON array from --output json");
    assert_eq!(
        arr.len(),
        1,
        "Expected 1 result (filtered by search), got {}",
        arr.len()
    );
}

/// AC-002 (negative): When `--search` is NOT passed, the HTTP request MUST NOT
/// include `searchQuery` as a query parameter.
///
/// Per L-288-pr1-01: uses `query_param_is_missing` as the strict matcher.
///
/// Traces: BC-X.12.002
#[tokio::test]
async fn test_requesttype_list_search_omitted_when_not_set() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    // query_param_is_missing enforces the param is truly absent —
    // the mock will NOT match if the implementation sends searchQuery=... as an extra param.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .and(query_param_is_missing("searchQuery"))
        .respond_with(ResponseTemplate::new(200).set_body_json(two_request_types_body()))
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
            "requesttype",
            "list",
            "--project",
            "HELP",
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
        "Expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("Expected valid JSON in stdout, got: {stdout}\nError: {e}"));
    let arr = parsed.as_array().expect("Expected JSON array");
    assert_eq!(
        arr.len(),
        2,
        "Expected 2 results (no filter), got {}",
        arr.len()
    );
}

// ─── AC-003: non-JSM project exits 64 with call-site-specific message ────────

/// AC-003: `jr requesttype list --project SW` on a software project exits 64
/// with a message containing the call-site-specific label for requesttype
/// (NOT "Queue commands require").
///
/// Traces: BC-X.12.003, BC-X.8.004
#[tokio::test]
async fn test_requesttype_list_non_jsm_project_exits_64_with_callsite_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_sw_software(&server).await;
    // No request-type mocks — the command must abort before reaching that endpoint.

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["requesttype", "list", "--project", "SW", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for non-JSM project, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    // H-2 fix: assert the verbatim BC-X.12.003 phrase rather than a tautological
    // substring that would pass even if the BC-mandated wording were absent.
    assert!(
        stderr.contains("`jr requesttype` commands require a Jira Service Management project"),
        "AC-003 / BC-X.12.003: stderr must contain the verbatim BC phrase; got: {stderr}"
    );
    // Negative assertion: the old missing-verb form must not appear.
    assert!(
        !stderr.contains("jr requesttype requires"),
        "Old missing-verb form must not appear (BC mandates 'commands require'); got: {stderr}"
    );
    // AC-003 specifically asserts the call-site label is NOT the queue label.
    assert!(
        !stderr.contains("Queue commands require"),
        "Unexpected 'Queue commands require' in stderr — wrong call-site label, got: {stderr}"
    );
}

// ─── AC-004: --output json shape for list ────────────────────────────────────

/// AC-004: `jr requesttype list --project HELP --output json` returns a JSON array
/// where each element has the required keys with correct types.
///
/// Traces: BC-X.12.004
#[tokio::test]
async fn test_requesttype_list_output_json_shape() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(two_request_types_body()))
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
            "requesttype",
            "list",
            "--project",
            "HELP",
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
        "Expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("Expected valid JSON in stdout, got: {stdout}\nError: {e}"));
    let arr = parsed
        .as_array()
        .expect("Expected JSON array from --output json");
    assert!(
        !arr.is_empty(),
        "Expected at least one request type in output"
    );

    let first = &arr[0];
    assert!(
        first.get("id").and_then(Value::as_str).is_some(),
        "Expected 'id' as string in first element, got: {first}"
    );
    assert!(
        first.get("name").and_then(Value::as_str).is_some(),
        "Expected 'name' as string in first element, got: {first}"
    );
    // description and helpText may be null but key must be present
    assert!(
        first.get("description").is_some(),
        "Expected 'description' key in first element, got: {first}"
    );
    assert!(
        first.get("helpText").is_some(),
        "Expected 'helpText' key in first element, got: {first}"
    );
    assert!(
        first.get("issueTypeId").is_some(),
        "Expected 'issueTypeId' key in first element, got: {first}"
    );
    assert!(
        first.get("groupIds").and_then(Value::as_array).is_some(),
        "Expected 'groupIds' as array in first element, got: {first}"
    );
}

// ─── AC-005: fields command resolves name and returns table ──────────────────

/// AC-005: `jr requesttype fields "Password Reset" --project HELP` resolves the
/// name to request type ID 11002 via partial_match, then calls
/// `GET .../servicedesk/10/requesttype/11002/field`.
/// Default table shows Field Name, Required, Type columns.
///
/// Traces: BC-X.12.005
#[tokio::test]
async fn test_requesttype_fields_resolves_name_and_returns_table() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    // Request types list: cache miss on first call, used for name resolution.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(two_request_types_body()))
        .mount(&server)
        .await;

    // Fields endpoint for request type 11002 (Password Reset).
    Mock::given(method("GET"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/10/requesttype/11002/field",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(fields_response_body()))
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
            "requesttype",
            "fields",
            "Password Reset",
            "--project",
            "HELP",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    // Table should have a row for the summary field
    assert!(
        stdout.contains("summary") || stdout.contains("What do you need"),
        "Expected summary field row in stdout table, got: {stdout}"
    );
    // Table should have column headers
    assert!(
        stdout.contains("Field") || stdout.contains("field"),
        "Expected 'Field' column header in stdout table, got: {stdout}"
    );
    assert!(
        stdout.contains("Required") || stdout.contains("required"),
        "Expected 'Required' column header in stdout table, got: {stdout}"
    );
}

// ─── AC-006: ambiguous name exits 64 with hint ───────────────────────────────

/// AC-006: `jr requesttype fields "Password" --project HELP` when two types both
/// match "Password" (Password Reset, Password Change) exits 64 with stderr listing
/// both candidates and the hint `jr requesttype list --project HELP`.
///
/// In --no-input mode, exits 64 cleanly (does NOT prompt).
///
/// Traces: BC-X.12.006
#[tokio::test]
async fn test_requesttype_fields_ambiguous_exits_64_with_hint() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    // Two request types both containing "Password".
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
                    "name": "Password Reset",
                    "description": "Reset your password",
                    "helpText": null,
                    "issueTypeId": "12345",
                    "serviceDeskId": "10",
                    "portalId": "2",
                    "groupIds": []
                },
                {
                    "id": "11002",
                    "name": "Password Change",
                    "description": "Change your password",
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
            "requesttype",
            "fields",
            "Password",
            "--project",
            "HELP",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for ambiguous name, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Password Reset"),
        "Expected candidate 'Password Reset' in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("Password Change"),
        "Expected candidate 'Password Change' in stderr, got: {stderr}"
    );
    // M-2 fix: BC-X.12.006 (as updated 2026-05-18) mandates the "Run" imperative verb.
    // Tightened from substring-only check so both the verb and the full hint phrase are pinned.
    assert!(
        stderr.contains("Run `jr requesttype list --project HELP`"),
        "AC-006 / BC-X.12.006: hint must use 'Run' imperative; got: {stderr}"
    );
    // Negative: old "Use" verb form must not appear after BC alignment to "Run".
    assert!(
        !stderr.contains("Use `jr requesttype list"),
        "Old 'Use' verb form must not appear after BC alignment to 'Run'; got: {stderr}"
    );
    // The hint MUST use the canonical single-word `requesttype` subcommand
    // (pinned via `#[command(name = "requesttype")]` in `cli/mod.rs`). The kebab-case
    // form `request-type` is not a valid subcommand and would mis-direct users.
    assert!(
        !stderr.contains("request-type"),
        "Stderr must not contain kebab-case 'request-type' (canonical form is single-word 'requesttype'). Got: {stderr}"
    );
}

// ─── H-4: not-found error includes BC-X.12.008 cache-deletion hint ───────────

/// BC-X.12.008 §Stale-cache window: when `jr requesttype fields <NAME>` cannot
/// find the request type via partial_match, the error message MUST contain both
/// the standard not-found hint AND the manual cache-deletion sentence:
///   '...or delete the cache file at ~/.cache/jr/v1/<profile>/request_types_<sid>.json
///    if a recent admin change is suspected.'
///
/// This test decomposes the sentence into four overlapping substring assertions
/// to tolerate the dynamic `<profile>` interpolation while still pinning each
/// literal fragment that the BC mandates.
///
/// Traces: BC-X.12.008 §Stale-cache window
#[tokio::test]
async fn test_requesttype_fields_not_found_error_includes_cache_deletion_hint() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    // Request types list: "NonExistentType" will not match any of these two types.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(two_request_types_body()))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "requesttype",
            "fields",
            "NonExistentType",
            "--project",
            "HELP",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for not-found request type, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    // BC-X.12.008: not-found phrase.
    assert!(
        stderr.contains("Request type \"NonExistentType\" not found"),
        "BC-X.12.008: stderr must contain not-found phrase; got: {stderr}"
    );

    // BC-X.12.006 / BC-X.12.008: Run hint with "current types" wording.
    assert!(
        stderr.contains("Run `jr requesttype list --project HELP`"),
        "BC-X.12.008: stderr must contain 'Run `jr requesttype list --project HELP`'; got: {stderr}"
    );

    // BC-X.12.008 §Stale-cache window: cache-deletion prefix (profile name is dynamic).
    assert!(
        stderr.contains("or delete the cache file at ~/.cache/jr/v1/"),
        "BC-X.12.008: stderr must contain cache-deletion prefix; got: {stderr}"
    );

    // BC-X.12.008 §Stale-cache window: filename suffix with the service desk ID "10".
    assert!(
        stderr.contains("/request_types_10.json"),
        "BC-X.12.008: stderr must contain '/request_types_10.json' (sid=10 from fixture); got: {stderr}"
    );

    // BC-X.12.008 §Stale-cache window: full closing phrase.
    assert!(
        stderr.contains("if a recent admin change is suspected"),
        "BC-X.12.008: stderr must contain closing phrase; got: {stderr}"
    );
}

// ─── AC-007: --output json shape for fields ───────────────────────────────────

/// AC-007: `jr requesttype fields "Password Reset" --project HELP --output json`
/// returns a JSON object with the required top-level keys and field array shape.
///
/// Traces: BC-X.12.007
#[tokio::test]
async fn test_requesttype_fields_output_json_shape() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    // Request types list for name resolution.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(two_request_types_body()))
        .mount(&server)
        .await;

    // Fields endpoint for request type 11002.
    Mock::given(method("GET"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/10/requesttype/11002/field",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(fields_response_body()))
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
            "requesttype",
            "fields",
            "Password Reset",
            "--project",
            "HELP",
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
        "Expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("Expected valid JSON object in stdout, got: {stdout}\nError: {e}")
    });
    let obj = parsed
        .as_object()
        .expect("Expected JSON object for fields output");

    assert!(
        obj.contains_key("canRaiseOnBehalfOf"),
        "Expected 'canRaiseOnBehalfOf' key in fields JSON output, got: {parsed}"
    );
    assert!(
        obj.contains_key("canAddRequestParticipants"),
        "Expected 'canAddRequestParticipants' key in fields JSON output, got: {parsed}"
    );
    // The story normalizes requestTypeFields → fields in the CLI output.
    // Accept either "fields" (normalized) or "requestTypeFields" (raw API camelCase).
    let field_arr = obj
        .get("fields")
        .or_else(|| obj.get("requestTypeFields"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "Expected 'fields' or 'requestTypeFields' as array in JSON output, got: {parsed}"
            )
        });

    assert!(
        !field_arr.is_empty(),
        "Expected at least one field in the fields array, got empty"
    );

    let first_field = &field_arr[0];
    assert!(
        first_field.get("fieldId").and_then(Value::as_str).is_some(),
        "Expected 'fieldId' as string in first field, got: {first_field}"
    );
    assert!(
        first_field.get("name").and_then(Value::as_str).is_some(),
        "Expected 'name' as string in first field, got: {first_field}"
    );
    assert!(
        first_field
            .get("required")
            .and_then(Value::as_bool)
            .is_some(),
        "Expected 'required' as bool in first field, got: {first_field}"
    );
    assert!(
        first_field.get("jiraSchema").is_some(),
        "Expected 'jiraSchema' key in first field, got: {first_field}"
    );
}

// ─── AC-008: list cache hit — no second HTTP call ────────────────────────────

/// AC-008: The second invocation of `jr requesttype list --project HELP` reads
/// from cache and makes NO second HTTP call to the request types endpoint.
///
/// Verified with `expect(1)` on the request types mock across two binary invocations.
///
/// Traces: BC-X.12.008
#[tokio::test]
async fn test_requesttype_list_cache_hit_no_second_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // Project meta and service desk list are also cached per-profile after the first
    // call. Mount without expect() so wiremock doesn't complain about >1 call if they
    // aren't cached, and the test still validates the critical invariant (request types
    // endpoint fires exactly once).
    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    // CRITICAL: expect(1) — must fire EXACTLY ONCE across BOTH invocations.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(two_request_types_body()))
        .expect(1)
        .mount(&server)
        .await;

    // Invocation 1: populates the cache.
    let out1 = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "requesttype",
            "list",
            "--project",
            "HELP",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    assert!(
        out1.status.success(),
        "Invocation 1 expected exit 0, got {:?}. stderr: {stderr1}",
        out1.status.code()
    );

    let stdout1 = String::from_utf8_lossy(&out1.stdout);
    let parsed1: Value = serde_json::from_str(&stdout1)
        .unwrap_or_else(|e| panic!("Invocation 1 expected valid JSON, got: {stdout1}\nError: {e}"));

    // Invocation 2: must hit cache — the request types mock must NOT fire again.
    let out2 = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "requesttype",
            "list",
            "--project",
            "HELP",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(
        out2.status.success(),
        "Invocation 2 expected exit 0, got {:?}. stderr: {stderr2}",
        out2.status.code()
    );

    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    let parsed2: Value = serde_json::from_str(&stdout2)
        .unwrap_or_else(|e| panic!("Invocation 2 expected valid JSON, got: {stdout2}\nError: {e}"));

    // Both outputs must be equivalent arrays.
    assert_eq!(
        parsed1.as_array().map(|a| a.len()),
        parsed2.as_array().map(|a| a.len()),
        "Invocation 1 and 2 produced different output lengths: {parsed1} vs {parsed2}"
    );
    // M-6 fix: full-JSON equivalence pin — cache hit must produce byte-identical output.
    assert_eq!(
        parsed1, parsed2,
        "cache hit must return byte-identical JSON output across invocations"
    );
    // The expect(1) mock constraint is verified automatically by wiremock when the
    // MockServer drops — if the endpoint was called twice, the test panics here.
}

// ─── AC-009: fields cache hit — no second HTTP call (holdout H-NEW-JSM-RT-005) ──

/// AC-009 (holdout H-NEW-JSM-RT-005): The second invocation of
/// `jr requesttype fields "Password Reset" --project HELP` reads from the fields
/// cache and makes NO second HTTP call to the fields endpoint.
///
/// Verified with `expect(1)` on the fields mock across two binary invocations.
///
/// Traces: BC-X.12.005, BC-X.12.008, holdout H-NEW-JSM-RT-005
#[tokio::test]
async fn test_requesttype_fields_cache_hit_no_second_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    // Request types list: needed for name resolution on both invocations unless
    // also cached. Mount without expect() — the critical invariant is fields endpoint.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(two_request_types_body()))
        .mount(&server)
        .await;

    // CRITICAL: expect(1) — fields endpoint must fire EXACTLY ONCE across BOTH invocations.
    Mock::given(method("GET"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/10/requesttype/11002/field",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(fields_response_body()))
        .expect(1)
        .mount(&server)
        .await;

    // Invocation 1: populates the fields cache.
    let out1 = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "requesttype",
            "fields",
            "Password Reset",
            "--project",
            "HELP",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    assert!(
        out1.status.success(),
        "Invocation 1 expected exit 0, got {:?}. stderr: {stderr1}",
        out1.status.code()
    );

    let stdout1 = String::from_utf8_lossy(&out1.stdout);
    let parsed1: Value = serde_json::from_str(&stdout1)
        .unwrap_or_else(|e| panic!("Invocation 1 expected valid JSON, got: {stdout1}\nError: {e}"));

    // Invocation 2: must hit fields cache — fields endpoint must NOT fire again.
    let out2 = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "requesttype",
            "fields",
            "Password Reset",
            "--project",
            "HELP",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(
        out2.status.success(),
        "Invocation 2 expected exit 0, got {:?}. stderr: {stderr2}",
        out2.status.code()
    );

    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    let parsed2: Value = serde_json::from_str(&stdout2)
        .unwrap_or_else(|e| panic!("Invocation 2 expected valid JSON, got: {stdout2}\nError: {e}"));

    // Both outputs must agree on canRaiseOnBehalfOf (smoke check for equivalence).
    assert_eq!(
        parsed1.get("canRaiseOnBehalfOf"),
        parsed2.get("canRaiseOnBehalfOf"),
        "Expected both invocations to produce equivalent canRaiseOnBehalfOf"
    );
    // M-6 fix: full-JSON equivalence pin — cache hit must produce byte-identical output.
    assert_eq!(
        parsed1, parsed2,
        "cache hit must return byte-identical JSON output across invocations"
    );
    // The expect(1) mock constraint is verified automatically by wiremock when the
    // MockServer drops — if the endpoint was called twice, the test panics here.
}

// ─── AC-011: uses profile project when no --project flag ─────────────────────

/// AC-011: When no `--project` flag is given but the active profile has a project
/// configured, `jr requesttype list` uses the profile project.
///
/// Traces: BC-X.12.003
#[tokio::test]
async fn test_requesttype_list_uses_profile_project_when_no_flag() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Write a config with project = "HELP" in the default profile.
    let config_jr_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&config_jr_dir).unwrap();
    std::fs::write(
        config_jr_dir.join("config.toml"),
        format!(
            "[instance]\nurl = \"{}\"\n\n[profiles.default]\nproject = \"HELP\"\n",
            server.uri()
        ),
    )
    .unwrap();

    mount_project_meta_help(&server).await;
    mount_service_desk_list(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(two_request_types_body()))
        .expect(1)
        .mount(&server)
        .await;

    // Run WITHOUT --project flag — must pick up "HELP" from profile config.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["requesttype", "list", "--no-input", "--output", "json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Expected exit 0 (profile project used), got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("Expected valid JSON in stdout, got: {stdout}\nError: {e}"));
    let arr = parsed.as_array().expect("Expected JSON array");
    assert_eq!(
        arr.len(),
        2,
        "Expected 2 request types (HELP project used from profile), got {}",
        arr.len()
    );
}

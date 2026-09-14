//! RED-phase failing tests for ADF auto-conversion on the platform CREATE path.
//!
//! Covers VP-FIELD-ADF-002/003/004 Axes G/h1 plus AC-004 Axis B, AC-005,
//! AC-006, AC-012, AC-013 from S-cycle12-platform-adf-autoconvert.
//!
//! RED expectations:
//! - AC-006: NET-NEW guard does not exist; command proceeds past where it
//!   should exit 64 with "cannot be combined with `--markdown`".
//! - AC-005/Axis G: create empty ADF field sends plain string in POST body
//!   instead of omitting the field entirely.
//! - AC-012: create table shows raw value instead of "(adf)".
//! - AC-013: createmeta with absent system/custom fields may deserialize
//!   them as empty strings rather than None, causing is_adf_field to miss.
//! - AC-004 Axis B: create empty ADF field includes field in POST body
//!   instead of omitting it.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Shared scaffolding
// ---------------------------------------------------------------------------

fn write_minimal_config(config_home: &std::path::Path, url: &str) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        format!("[instance]\nurl = \"{url}\"\n"),
    )
    .unwrap();
}

fn write_fields_cache(cache_home: &std::path::Path, profile: &str, fields: &[(&str, &str)]) {
    let dir = cache_home.join("jr").join("v1").join(profile);
    std::fs::create_dir_all(&dir).unwrap();
    let tuples: Vec<Value> = fields.iter().map(|(id, name)| json!([id, name])).collect();
    let cache = json!({
        "fields": tuples,
        "fetched_at": chrono::Utc::now().to_rfc3339()
    });
    std::fs::write(
        dir.join("fields.json"),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();
}

struct Harness {
    server: MockServer,
    cache_dir: tempfile::TempDir,
    config_dir: tempfile::TempDir,
}

impl Harness {
    async fn new() -> Self {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());
        Self {
            server,
            cache_dir,
            config_dir,
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("jr").unwrap();
        cmd.env("JR_BASE_URL", self.server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("XDG_CACHE_HOME", self.cache_dir.path())
            .env("JR_CACHE_DIR", self.cache_dir.path().join("jr"))
            .env("XDG_CONFIG_HOME", self.config_dir.path())
            .env("JR_CONFIG_DIR", self.config_dir.path().join("jr"));
        cmd
    }
}

/// Mount `GET /rest/api/3/issue/createmeta/{project}/issuetypes` returning
/// a single issue type.
async fn mount_issue_types(server: &MockServer, project: &str) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/api/3/issue/createmeta/{project}/issuetypes"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issueTypes": [{"id": "10001", "name": "Task", "subtask": false}]
        })))
        .mount(server)
        .await;
}

/// Mount `GET /rest/api/3/issue/createmeta/{project}/issuetypes/{itid}`
/// returning a textarea custom field and a summary system field as an array.
async fn mount_createmeta_textarea(
    server: &MockServer,
    project: &str,
    it_id: &str,
    field_id: &str,
    field_name: &str,
) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/api/3/issue/createmeta/{project}/issuetypes/{it_id}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": [
                {
                    "fieldId": "summary",
                    "name": "Summary",
                    "required": true,
                    "schema": {"type": "string", "system": "summary"},
                    "operations": ["set"]
                },
                {
                    "fieldId": field_id,
                    "name": field_name,
                    "required": false,
                    "schema": {
                        "type": "string",
                        "custom": "com.atlassian.jira.plugin.system.customfieldtypes:textarea"
                    },
                    "operations": ["set"]
                }
            ],
            "total": 2,
            "maxResults": 50,
            "startAt": 0
        })))
        .mount(server)
        .await;
}

/// Descriptor for a single ADF field in the mixed-fields createmeta mock.
struct AdfFieldDesc<'a> {
    id: &'a str,
    name: &'a str,
    schema_system: Option<&'a str>,
    schema_custom: Option<&'a str>,
}

/// Mount `GET .../createmeta/.../issuetypes/{it_id}` with BOTH a textarea
/// field AND a system field (description/environment) as an array, to test
/// multi-field ADF detection with the correct Jira Cloud REST API v3 shape.
async fn mount_createmeta_mixed_adf_fields(
    server: &MockServer,
    project: &str,
    it_id: &str,
    textarea_field: AdfFieldDesc<'_>,
    system_field: AdfFieldDesc<'_>,
) {
    let ta_id = textarea_field.id;
    let ta_name = textarea_field.name;
    let ta_schema = json!({
        "type": "string",
        "system": textarea_field.schema_system,
        "custom": textarea_field.schema_custom,
    });
    let sys_id = system_field.id;
    let sys_name = system_field.name;
    let sys_schema = json!({
        "type": "string",
        "system": system_field.schema_system,
        "custom": system_field.schema_custom,
    });
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/api/3/issue/createmeta/{project}/issuetypes/{it_id}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": [
                {
                    "fieldId": "summary",
                    "name": "Summary",
                    "required": true,
                    "schema": {"type": "string", "system": "summary"},
                    "operations": ["set"]
                },
                {
                    "fieldId": ta_id,
                    "name": ta_name,
                    "required": false,
                    "schema": ta_schema,
                    "operations": ["set"]
                },
                {
                    "fieldId": sys_id,
                    "name": sys_name,
                    "required": false,
                    "schema": sys_schema,
                    "operations": ["set"]
                }
            ],
            "total": 3,
            "maxResults": 50,
            "startAt": 0
        })))
        .mount(server)
        .await;
}

/// Mount `GET /rest/api/3/field` list.
async fn mount_list_fields(server: &MockServer, fields: &[(&str, &str)]) {
    let body: Vec<Value> = fields
        .iter()
        .map(|(id, name)| json!({"id": id, "name": name, "custom": true}))
        .collect();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

/// Mount `POST /rest/api/3/issue` returning a created issue key.
async fn mount_post_issue(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(json!({"id": "10100", "key": "TEST-100"})),
        )
        .mount(server)
        .await;
}

// ---------------------------------------------------------------------------
// AC-006 / VP-FIELD-ADF-004 Axis h1
// ---------------------------------------------------------------------------

/// AC-006: NET-NEW guard on create path: `--markdown + --field description=VALUE`
/// must exit 64 with message containing `"cannot be combined with \`--markdown\`"`.
///
/// RED: the guard does not exist; the D2 collision guard fires first (for
/// `--field description=X`) and exits 64 with a DIFFERENT message.
/// The assertion on the required substring fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_3_014_5_markdown_field_description_conflict_exits_64_create() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"));

    let out = cmd
        .args([
            "issue",
            "create",
            "--project",
            "TEST",
            "--type",
            "Task",
            "--summary",
            "Test issue",
            "--field",
            "description=Some description",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(64),
        "AC-006: must exit 64; stderr: {stderr}"
    );
    assert!(
        stderr.contains("cannot be combined with `--markdown`"),
        "AC-006: stderr must contain 'cannot be combined with `--markdown`'; got: {stderr}"
    );
    assert!(
        out.stdout.is_empty(),
        "AC-006: stdout must be empty; got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

// ---------------------------------------------------------------------------
// F-1 negative anchor (ADR-0024 §uniform-exit-64): guard is case-SENSITIVE
// ---------------------------------------------------------------------------

/// F-1 negative anchor (create path): `--field Description=VALUE` (capital D)
/// with `--markdown` must NOT fire the guard — only the exact raw token
/// `"description"` (lowercase, no `:kind` suffix) matches.
///
/// RED: the current guard uses `eq_ignore_ascii_case("description")` on the
/// PARSED map key, so capital-D fires the guard spuriously.
/// GREEN after fix: guard iterates raw CLI tokens, matching only
/// substring-before-first-`=` == `"description"` (case-sensitive).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_3_014_5_markdown_field_description_guard_is_case_sensitive_create() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"));

    let out = cmd
        .args([
            "issue",
            "create",
            "--project",
            "TEST",
            "--type",
            "Task",
            "--summary",
            "Test issue",
            "--field",
            "Description=Some description",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    // The guard MUST NOT fire for capital-D. The command may fail for other
    // reasons (HTTP, missing mocks), but not with the "cannot be combined" message.
    assert!(
        !stderr.contains("cannot be combined with `--markdown`"),
        "F-1 create: capital-D 'Description' must NOT trigger the guard; stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-005 + AC-013 Axis G / VP-FIELD-ADF-003 Axis G
// ---------------------------------------------------------------------------

/// AC-005 / AC-013b (same test): platform create with empty bare ADF-backed
/// `--field NAME=` must NOT include the field in the POST body.
///
/// RED: current code includes the field as `Value::String("")` in the POST body
/// → assertion that the field is absent from the POST body fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_3_015_create_empty_adf_field_omitted_from_post_body() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_12345";
    const FIELD_NAME: &str = "My Textarea";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_list_fields(&h.server, &[(FIELD_ID, FIELD_NAME)]).await;
    mount_issue_types(&h.server, "TEST").await;
    mount_createmeta_textarea(&h.server, "TEST", "10001", FIELD_ID, FIELD_NAME).await;
    mount_post_issue(&h.server).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "TEST",
            "--type",
            "Task",
            "--summary",
            "Test issue",
            "--field",
            &format!("{FIELD_NAME}="),
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-005: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Intercept POST body and assert the ADF field is absent.
    let reqs = h.server.received_requests().await.unwrap();
    let post_req = reqs
        .iter()
        .find(|r| r.method == wiremock::http::Method::POST)
        .expect("AC-005: POST request must have been made");
    let post_body: Value =
        serde_json::from_slice(&post_req.body).expect("AC-005: POST body must be valid JSON");

    assert!(
        post_body["fields"].get(FIELD_ID).is_none(),
        "AC-005: POST body must NOT contain field '{FIELD_ID}' for empty ADF-backed field; got: {:?}",
        post_body["fields"]
    );
}

// ---------------------------------------------------------------------------
// AC-012 / VP-FIELD-ADF-002
// ---------------------------------------------------------------------------

/// AC-012: platform create table-mode success echo must show `(adf)` for an
/// ADF-backed field, NOT the raw value or ADF JSON.
///
/// RED: current code stores `Value::String("Hello")` in fields; `field_markers`
/// is never populated → table shows raw value "Hello", not "(adf)".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_3_013_create_table_shows_adf_marker() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_12345";
    const FIELD_NAME: &str = "My Textarea";
    const INPUT: &str = "Hello World";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_list_fields(&h.server, &[(FIELD_ID, FIELD_NAME)]).await;
    mount_issue_types(&h.server, "TEST").await;
    mount_createmeta_textarea(&h.server, "TEST", "10001", FIELD_ID, FIELD_NAME).await;
    mount_post_issue(&h.server).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "TEST",
            "--type",
            "Task",
            "--summary",
            "Test issue",
            "--field",
            &format!("{FIELD_NAME}={INPUT}"),
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-012: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Live table-mode ADF marker is on stderr (Symmetric convention: human echoes
    // → stderr; stdout reserved for --output json). Do NOT check stdout.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("(adf)"),
        "AC-012: create table must show '(adf)' for ADF-backed field; stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains(INPUT),
        "AC-012: create table must NOT show raw input '{INPUT}' on stdout; stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-013a / BC-3.3.013/014 createmeta adaptation fidelity
// ---------------------------------------------------------------------------

/// AC-013a: a createmeta response with BOTH a `:textarea` custom field AND a
/// `environment` system field — both must be detected as ADF-backed.
///
/// A lossy deserialization that maps absent `system` to `""` (not `None`)
/// would cause `is_adf_field` to miss the system-field sub-case.
///
/// RED: currently no ADF branch in dispatch_field_value; both fields are
/// stored as `Value::String` in the POST body → assertions that the POST body
/// contains ADF doc objects for both fields fail.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_3_013_014_createmeta_adaptation_preserves_system_custom_for_adf_detection() {
    let h = Harness::new().await;
    const TEXTAREA_ID: &str = "customfield_12345";
    const TEXTAREA_NAME: &str = "My Textarea";
    const ENV_ID: &str = "environment";
    const ENV_DISPLAY_NAME: &str = "Environment";
    const INPUT: &str = "Some content";

    write_fields_cache(
        h.cache_dir.path(),
        "default",
        &[(TEXTAREA_ID, TEXTAREA_NAME), (ENV_ID, ENV_DISPLAY_NAME)],
    );
    mount_list_fields(
        &h.server,
        &[(TEXTAREA_ID, TEXTAREA_NAME), (ENV_ID, ENV_DISPLAY_NAME)],
    )
    .await;
    mount_issue_types(&h.server, "TEST").await;
    mount_createmeta_mixed_adf_fields(
        &h.server,
        "TEST",
        "10001",
        AdfFieldDesc {
            id: TEXTAREA_ID,
            name: TEXTAREA_NAME,
            schema_system: None,
            schema_custom: Some("com.atlassian.jira.plugin.system.customfieldtypes:textarea"),
        },
        AdfFieldDesc {
            id: ENV_ID,
            name: ENV_DISPLAY_NAME,
            schema_system: Some("environment"),
            schema_custom: None,
        },
    )
    .await;
    mount_post_issue(&h.server).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "TEST",
            "--type",
            "Task",
            "--summary",
            "Test issue",
            "--field",
            &format!("{TEXTAREA_NAME}={INPUT}"),
            "--field",
            &format!("{ENV_DISPLAY_NAME}={INPUT}"),
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-013a: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let reqs = h.server.received_requests().await.unwrap();
    let post_req = reqs
        .iter()
        .find(|r| r.method == wiremock::http::Method::POST)
        .expect("AC-013a: POST request must have been made");
    let post_body: Value =
        serde_json::from_slice(&post_req.body).expect("AC-013a: POST body must be valid JSON");

    // Both fields must be ADF doc objects in the POST body.
    assert_eq!(
        post_body["fields"][TEXTAREA_ID]["type"].as_str(),
        Some("doc"),
        "AC-013a: textarea field must be an ADF doc in POST body; got: {:?}",
        post_body["fields"][TEXTAREA_ID]
    );
    assert_eq!(
        post_body["fields"][ENV_ID]["type"].as_str(),
        Some("doc"),
        "AC-013a: environment system field must be an ADF doc in POST body; got: {:?}",
        post_body["fields"][ENV_ID]
    );
}

// ---------------------------------------------------------------------------
// AC-004 Axis B / VP-FIELD-ADF-003 Axis B
// ---------------------------------------------------------------------------

/// AC-004 Axis B: platform create with empty bare ADF-backed `--field NAME=`
/// must OMIT the field from the POST `fields` map entirely.
///
/// This is an alternative verification of the same invariant as AC-005.
/// Both tests pin the BC-3.3.015 postcondition: the field is absent (not null,
/// not empty string, not empty object) from the POST body.
///
/// RED: current code includes `Value::String("")` in the POST body.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_3_015_create_empty_adf_field_omitted() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_99999";
    const FIELD_NAME: &str = "Notes Field";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_list_fields(&h.server, &[(FIELD_ID, FIELD_NAME)]).await;
    mount_issue_types(&h.server, "TEST").await;
    mount_createmeta_textarea(&h.server, "TEST", "10001", FIELD_ID, FIELD_NAME).await;
    mount_post_issue(&h.server).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "TEST",
            "--type",
            "Task",
            "--summary",
            "Test issue for empty ADF omit",
            "--field",
            &format!("{FIELD_NAME}="),
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-004B: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let reqs = h.server.received_requests().await.unwrap();
    let post_req = reqs
        .iter()
        .find(|r| r.method == wiremock::http::Method::POST)
        .expect("AC-004B: POST request must have been made");
    let post_body: Value =
        serde_json::from_slice(&post_req.body).expect("AC-004B: POST body must be valid JSON");

    assert!(
        post_body["fields"].get(FIELD_ID).is_none(),
        "AC-004B: POST body must NOT contain field '{FIELD_ID}' when ADF field is empty; \
         create path must omit (not null, not \"\", not clear-doc); got: {:?}",
        post_body["fields"]
    );
}

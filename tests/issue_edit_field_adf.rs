//! Tests for ADF auto-conversion on the platform EDIT path.
//!
//! Covers VP-FIELD-ADF-002/003/004 Axes E/F/H/h2 plus AC-004 Axis A, AC-007,
//! AC-008, AC-009, AC-010, AC-011 from S-cycle12-platform-adf-autoconvert.
//!
//! Pre-fix (RED) behavioral gaps that drove these tests:
//! - AC-007: edit guard exited 64 with wrong message substring.
//! - AC-008/009: dry-run paths stored Value::String in plannedChanges/table
//!   instead of ADF doc object / "(adf)" sentinel.
//! - AC-010/011: live-edit wire PUT body carried plain string; table showed raw
//!   value / "(updated)" rather than "(adf)".
//! - AC-004 Axis A: [pre-fix BUG] edit empty ADF field sent plain empty string
//!   on wire instead of the required clear-doc ({type:doc,version:1,content:[]});
//!   post-fix behavior (the contract) is that the clear-doc is sent.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

fn jr_cmd_with_xdg(
    server_url: &str,
    cache_dir: &std::path::Path,
    config_dir: &std::path::Path,
) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("JR_CACHE_DIR", cache_dir.join("jr"))
        .env("XDG_CONFIG_HOME", config_dir)
        .env("JR_CONFIG_DIR", config_dir.join("jr"));
    cmd
}

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
    let tuples: Vec<serde_json::Value> =
        fields.iter().map(|(id, name)| json!([id, name])).collect();
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
        jr_cmd_with_xdg(
            &self.server.uri(),
            self.cache_dir.path(),
            self.config_dir.path(),
        )
    }
}

/// Mount `GET /rest/api/3/field` with a single textarea field.
async fn mount_list_fields_textarea(server: &MockServer, field_id: &str, field_name: &str) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": field_id,
                "name": field_name,
                "custom": true,
                "schema": {
                    "type": "string",
                    "custom": "com.atlassian.jira.plugin.system.customfieldtypes:textarea"
                }
            }
        ])))
        .mount(server)
        .await;
}

/// Mount `GET .../editmeta` returning a textarea custom field.
async fn mount_editmeta_textarea(server: &MockServer, key: &str, field_id: &str, field_name: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}/editmeta")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": {
                field_id: {
                    "name": field_name,
                    "schema": {
                        "type": "string",
                        "system": null,
                        "custom": "com.atlassian.jira.plugin.system.customfieldtypes:textarea"
                    },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(server)
        .await;
}

/// Mount `GET .../editmeta` returning description as a system field.
async fn mount_editmeta_description_system(server: &MockServer, key: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}/editmeta")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": {
                "description": {
                    "name": "Description",
                    "schema": {
                        "type": "string",
                        "system": "description",
                        "custom": null
                    },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(server)
        .await;
}

/// Mount `PUT /rest/api/3/issue/{key}` returning 204.
async fn mount_put_204(server: &MockServer, key: &str) {
    Mock::given(method("PUT"))
        .and(path(format!("/rest/api/3/issue/{key}")))
        .respond_with(ResponseTemplate::new(204))
        .mount(server)
        .await;
}

// ---------------------------------------------------------------------------
// AC-007 / VP-FIELD-ADF-004 Axis h2
// ---------------------------------------------------------------------------

/// AC-007: `--markdown + --field description=VALUE` on edit path must exit 64
/// with a message containing `"cannot be combined with \`--markdown\`"`.
///
/// RED: current `edit.rs` exits 64 with the pre-existing guard message
/// `"--markdown requires --description or --description-stdin"` which does
/// NOT contain the required substring.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_035_3_markdown_field_description_conflict_exits_64_edit() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    let out = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "edit",
            "TEST-1",
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
        "AC-007: must exit 64; stderr: {stderr}"
    );
    assert!(
        stderr.contains("cannot be combined with `--markdown`"),
        "AC-007: stderr must contain pinned substring 'cannot be combined with `--markdown`'; got: {stderr}"
    );
    assert!(
        stderr.contains("Pass `--description` with `--markdown`, or omit `--markdown`."),
        "AC-007: stderr must contain ADR-0024 remediation phrase \
         'Pass `--description` with `--markdown`, or omit `--markdown`.'; got: {stderr}"
    );
    assert!(
        out.stdout.is_empty(),
        "AC-007: stdout must be empty; got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

// ---------------------------------------------------------------------------
// F-1 negative anchor (ADR-0024 §uniform-exit-64): guard is case-SENSITIVE
// ---------------------------------------------------------------------------

/// F-1 negative anchor (edit path): `--field Description=VALUE` (capital D)
/// with `--markdown` must NOT fire the guard — only the exact raw token
/// `"description"` (lowercase, no `:kind` suffix) matches.
///
/// RED: the current guard uses `eq_ignore_ascii_case("description")` on the
/// PARSED map key, so capital-D fires the guard spuriously.
/// GREEN after fix: guard iterates raw CLI tokens, matching only
/// substring-before-first-`=` == `"description"` (case-sensitive).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_3_014_5_markdown_field_description_guard_is_case_sensitive_edit() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    let out = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Description=Some description",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    // The guard MUST NOT fire for capital-D. The command may fail for other
    // reasons (e.g. "--markdown requires --description"), but not with the
    // "cannot be combined" message.
    assert!(
        !stderr.contains("cannot be combined with `--markdown`"),
        "F-1 edit: capital-D 'Description' must NOT trigger the guard; stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-008 / VP-FIELD-ADF-002 dry-run JSON axis + VP-FIELD-ADF-003 Axis E
// ---------------------------------------------------------------------------

/// AC-008a: `--dry-run --output json` for a non-empty ADF-backed field must
/// include the full ADF doc object in `plannedChanges[human_name]`, NOT a
/// plain string.
///
/// RED: current code stores `Value::String("Hello")` in planned_preview →
/// `plannedChanges["My Textarea"] == "Hello"`, assertion on type="doc" fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_033_dry_run_planned_preview_contains_adf_object_keyed_by_human_name() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_12345";
    const FIELD_NAME: &str = "My Textarea";
    const KEY: &str = "TEST-1";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_editmeta_textarea(&h.server, KEY, FIELD_ID, FIELD_NAME).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("{FIELD_NAME}=Hello World"),
            "--dry-run",
            "--output",
            "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-008a: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout_json: Value =
        serde_json::from_slice(&out.stdout).expect("AC-008a: stdout must be valid JSON");

    let preview = &stdout_json["plannedChanges"][FIELD_NAME];
    assert_eq!(
        preview.get("type").and_then(Value::as_str),
        Some("doc"),
        "AC-008a: plannedChanges[\"{FIELD_NAME}\"] must be ADF doc object (type=doc), got: {preview}"
    );
    assert_eq!(
        preview.get("version").and_then(Value::as_i64),
        Some(1),
        "AC-008a: ADF object must have version=1"
    );
}

/// AC-008b: `--dry-run --output json` for an empty ADF-backed field must
/// include the clear-doc `{type:doc,version:1,content:[]}` in
/// `plannedChanges[human_name]`.
///
/// RED: current code stores `Value::String("")` → assertion on type="doc" fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_036_dry_run_planned_preview_contains_clear_doc_keyed_by_human_name() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_12345";
    const FIELD_NAME: &str = "My Textarea";
    const KEY: &str = "TEST-2";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_editmeta_textarea(&h.server, KEY, FIELD_ID, FIELD_NAME).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("{FIELD_NAME}="),
            "--dry-run",
            "--output",
            "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-008b: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout_json: Value =
        serde_json::from_slice(&out.stdout).expect("AC-008b: stdout must be valid JSON");

    let preview = &stdout_json["plannedChanges"][FIELD_NAME];
    assert_eq!(
        preview.get("type").and_then(Value::as_str),
        Some("doc"),
        "AC-008b: clear-doc must have type=doc; got: {preview}"
    );
    assert_eq!(
        preview.get("version").and_then(Value::as_i64),
        Some(1),
        "AC-008b: clear-doc must have version=1"
    );
    let content = preview.get("content").and_then(Value::as_array);
    assert!(
        matches!(content, Some(v) if v.is_empty()),
        "AC-008b: clear-doc must have empty content array; got: {preview}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 / VP-FIELD-ADF-002 table-mode axis + VP-FIELD-ADF-003 Axis H
// ---------------------------------------------------------------------------

/// AC-009a: `--dry-run` (table mode) for a non-empty ADF-backed field must
/// show `(adf)` in the field row.
///
/// RED: current code stores `Value::String` in planned_preview; the
/// `field_markers` map is never populated → table shows raw value, not "(adf)".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_033_dry_run_table_shows_adf_marker() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_12345";
    const FIELD_NAME: &str = "My Textarea";
    const KEY: &str = "TEST-3";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_editmeta_textarea(&h.server, KEY, FIELD_ID, FIELD_NAME).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("{FIELD_NAME}=Hello World"),
            "--dry-run",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-009a: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("(adf)"),
        "AC-009a: dry-run table must show '(adf)' for non-empty ADF-backed field; stdout: {stdout}"
    );
}

/// AC-009b: `--dry-run` (table mode) for an empty ADF-backed field must
/// show `(adf-clear)` from `field_markers`.
///
/// RED: `field_markers` is never populated; table shows empty string or
/// raw whitespace, not "(adf-clear)".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_036_dry_run_table_shows_adf_clear_sentinel_from_field_markers() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_12345";
    const FIELD_NAME: &str = "My Textarea";
    const KEY: &str = "TEST-4";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_editmeta_textarea(&h.server, KEY, FIELD_ID, FIELD_NAME).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("{FIELD_NAME}="),
            "--dry-run",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-009b: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("(adf-clear)"),
        "AC-009b: dry-run table must show '(adf-clear)' for empty ADF-backed field; stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-010 / VP-FIELD-ADF-002 live-echo axes + VP-FIELD-ADF-003 Axis F
// ---------------------------------------------------------------------------

/// AC-010a: live edit `--output json` channel: `changed_fields[human_name]`
/// carries the raw user-supplied input string — NOT the ADF object.
///
/// RED: currently no ADF branch, so the PUT body carries plain string
/// (not ADF doc). The assertion on the PUT body wire shape fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_033_live_edit_json_changed_fields_raw_input_not_adf_object() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_12345";
    const FIELD_NAME: &str = "My Textarea";
    const KEY: &str = "TEST-5";
    const INPUT: &str = "Hello World";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_list_fields_textarea(&h.server, FIELD_ID, FIELD_NAME).await;
    mount_editmeta_textarea(&h.server, KEY, FIELD_ID, FIELD_NAME).await;
    mount_put_204(&h.server, KEY).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("{FIELD_NAME}={INPUT}"),
            "--output",
            "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-010a: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout_json: Value =
        serde_json::from_slice(&out.stdout).expect("AC-010a: stdout must be valid JSON");

    // #398 invariant: JSON channel carries raw input string.
    assert_eq!(
        stdout_json["changed_fields"][FIELD_NAME].as_str(),
        Some(INPUT),
        "AC-010a: changed_fields must carry raw input string"
    );

    // Additionally verify the PUT wire body carried an ADF doc (VP-FIELD-ADF-002).
    // RED: currently PUT body carries plain string "Hello World", not ADF doc.
    let reqs = h.server.received_requests().await.unwrap();
    let put_req = reqs
        .iter()
        .find(|r| r.method == wiremock::http::Method::PUT)
        .expect("AC-010a: PUT request must have been made");
    let put_body: Value =
        serde_json::from_slice(&put_req.body).expect("AC-010a: PUT body must be valid JSON");
    assert_eq!(
        put_body["fields"][FIELD_ID]["type"].as_str(),
        Some("doc"),
        "AC-010a: PUT wire body must contain ADF doc object; got: {:?}",
        put_body["fields"][FIELD_ID]
    );
}

/// AC-010b: live edit table channel: field row shows `(adf)`, NOT raw value.
///
/// RED: current code stores raw string in `fields`; `field_markers` is never
/// populated → table shows raw value "Hello World", not "(adf)".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_033_live_edit_table_shows_adf_marker_not_raw_value() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_12345";
    const FIELD_NAME: &str = "My Textarea";
    const KEY: &str = "TEST-6";
    const INPUT: &str = "Hello World";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_list_fields_textarea(&h.server, FIELD_ID, FIELD_NAME).await;
    mount_editmeta_textarea(&h.server, KEY, FIELD_ID, FIELD_NAME).await;
    mount_put_204(&h.server, KEY).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("{FIELD_NAME}={INPUT}"),
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-010b: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Live table-mode ADF marker is on stderr (Symmetric convention: human echoes
    // → stderr; stdout reserved for --output json). Do NOT check stdout.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("(adf)"),
        "AC-010b: table must show '(adf)' for ADF-backed field; stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains(INPUT),
        "AC-010b: table must NOT show raw input value '{INPUT}' on stdout; stdout: {stdout}"
    );
}

/// AC-010c: live edit empty/whitespace ADF-backed field, JSON channel:
/// `changed_fields[human_name]` carries raw empty string — NOT the clear-doc.
///
/// RED: currently no ADF branch and no clear-doc; the PUT body carries
/// plain string "" rather than clear-doc. PUT wire body assertion fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_036_live_edit_json_changed_fields_raw_empty_input_not_clear_doc() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_12345";
    const FIELD_NAME: &str = "My Textarea";
    const KEY: &str = "TEST-7";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_list_fields_textarea(&h.server, FIELD_ID, FIELD_NAME).await;
    mount_editmeta_textarea(&h.server, KEY, FIELD_ID, FIELD_NAME).await;
    mount_put_204(&h.server, KEY).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("{FIELD_NAME}="),
            "--output",
            "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-010c: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout_json: Value =
        serde_json::from_slice(&out.stdout).expect("AC-010c: stdout must be valid JSON");

    // #398 invariant: JSON channel carries raw empty string, not the clear-doc.
    assert_eq!(
        stdout_json["changed_fields"][FIELD_NAME].as_str(),
        Some(""),
        "AC-010c: changed_fields must carry raw empty string"
    );

    // Additionally verify the PUT wire body carried the clear-doc (VP-FIELD-ADF-003).
    // RED: currently PUT body carries plain string "", not clear-doc.
    let reqs = h.server.received_requests().await.unwrap();
    let put_req = reqs
        .iter()
        .find(|r| r.method == wiremock::http::Method::PUT)
        .expect("AC-010c: PUT request must have been made");
    let put_body: Value =
        serde_json::from_slice(&put_req.body).expect("AC-010c: PUT body must be valid JSON");
    let clear_doc = json!({"type": "doc", "version": 1, "content": []});
    assert_eq!(
        put_body["fields"][FIELD_ID], clear_doc,
        "AC-010c: PUT wire body must contain clear-doc for empty ADF field; got: {:?}",
        put_body["fields"][FIELD_ID]
    );
}

/// AC-010c-ws: live edit whitespace-only ADF-backed field, JSON channel:
/// `changed_fields[human_name]` carries the RAW whitespace-only input string
/// (e.g. `"   "`) — NOT `""` and NOT the clear-doc (#398 lossless-machine-channel
/// invariant; BC-3.4.036 Postcondition / AC-010).
///
/// The empty-clear pre-check correctly fires for whitespace-only input
/// (`value.trim().is_empty()` is true), so the wire body must still carry the
/// clear-doc. Only the JSON echo channel is affected: it must preserve the raw
/// untrimmed value, not silently replace it with `String::new()`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_036_live_edit_json_changed_fields_raw_whitespace_input_not_empty_string() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_12345";
    const FIELD_NAME: &str = "My Textarea";
    const KEY: &str = "TEST-7";
    const WHITESPACE: &str = "   ";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_list_fields_textarea(&h.server, FIELD_ID, FIELD_NAME).await;
    mount_editmeta_textarea(&h.server, KEY, FIELD_ID, FIELD_NAME).await;
    mount_put_204(&h.server, KEY).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("{FIELD_NAME}={WHITESPACE}"),
            "--output",
            "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-010c-ws: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout_json: Value =
        serde_json::from_slice(&out.stdout).expect("AC-010c-ws: stdout must be valid JSON");

    // #398 invariant: JSON channel carries the raw whitespace string, not "".
    assert_eq!(
        stdout_json["changed_fields"][FIELD_NAME].as_str(),
        Some(WHITESPACE),
        "AC-010c-ws: changed_fields must carry raw whitespace string '   ', not empty string"
    );

    // Wire body must still carry the clear-doc (whitespace triggers the
    // empty-clear path, same as truly-empty input).
    let reqs = h.server.received_requests().await.unwrap();
    let put_req = reqs
        .iter()
        .find(|r| r.method == wiremock::http::Method::PUT)
        .expect("AC-010c-ws: PUT request must have been made");
    let put_body: Value =
        serde_json::from_slice(&put_req.body).expect("AC-010c-ws: PUT body must be valid JSON");
    let clear_doc = json!({"type": "doc", "version": 1, "content": []});
    assert_eq!(
        put_body["fields"][FIELD_ID], clear_doc,
        "AC-010c-ws: PUT wire body must contain clear-doc for whitespace-only ADF field; got: {:?}",
        put_body["fields"][FIELD_ID]
    );
}

// ---------------------------------------------------------------------------
// AC-011 / VP-FIELD-ADF-002 live-echo axis
// ---------------------------------------------------------------------------

/// AC-011: `--field description=VALUE` (bare form, M3 path) on live edit:
/// table cell must show `(adf)`, NOT `(updated)`.
///
/// The emit loop must consult `field_markers` BEFORE the legacy
/// `if field == "description" { "(updated)" }` hard-code.
///
/// RED: `field_markers` is never populated; the legacy hard-code fires first
/// → table shows "(updated)", not "(adf)".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_035_live_edit_field_description_shows_adf_not_updated() {
    let h = Harness::new().await;
    const KEY: &str = "TEST-8";
    const INPUT: &str = "Some description text";

    // Pre-populate the fields cache so the `list_fields()` HTTP call is not
    // needed; "description"'s field_id IS "description" (system field).
    write_fields_cache(
        h.cache_dir.path(),
        "default",
        &[("description", "Description")],
    );
    mount_editmeta_description_system(&h.server, KEY).await;
    mount_put_204(&h.server, KEY).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("description={INPUT}"),
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-011: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Live table-mode ADF marker is on stderr (Symmetric convention: human echoes
    // → stderr; stdout reserved for --output json). Do NOT check stdout.
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Must show "(adf)", NOT "(updated)".
    assert!(
        stderr.contains("(adf)"),
        "AC-011: table must show '(adf)' for --field description=VALUE; stderr: {stderr}"
    );
    assert!(
        !stderr.contains("(updated)"),
        "AC-011: table must NOT show '(updated)' when --field description= is used; stderr: {stderr}"
    );

    // JSON channel: changed_fields must carry raw input string.
    // Run again with --output json to check changed_fields.
    mount_editmeta_description_system(&h.server, KEY).await;
    mount_put_204(&h.server, KEY).await;

    let out_json = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("description={INPUT}"),
            "--output",
            "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stdout_json: Value = serde_json::from_slice(&out_json.stdout)
        .expect("AC-011: --output json stdout must be valid JSON");
    assert_eq!(
        stdout_json["changed_fields"]["description"].as_str(),
        Some(INPUT),
        "AC-011: changed_fields[description] must carry raw input string (lossless)"
    );
}

// ---------------------------------------------------------------------------
// AC-004 Axis A / VP-FIELD-ADF-003 Axis A
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// BC-3.4.037 / AC-002(d) — customfield-id bypass regression pin (M4 pin)
// ---------------------------------------------------------------------------

/// BC-3.4.037 / AC-002(d) regression pin (M4): `--field customfield_NNNNN=VALUE`
/// (the LITERAL customfield-id bypass form, not the display-name form) against
/// a `:textarea` ADF-backed field must place an ADF doc object in the PUT wire
/// body — not a plain `Value::String`.
///
/// ADF detection gates on `is_adf_field(&meta_field.schema)`, which inspects
/// the `system`/`custom` allowlist, NOT the `--field` key form (display-name
/// vs. customfield-id). This test pins that the bypass form routes through the
/// same ADF branch as the display-name form — a pure regression guard for
/// already-correct behavior, not a RED/GREEN TDD cycle.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_037_customfield_id_bypass_fires_adf_conversion() {
    let h = Harness::new().await;
    // Use the customfield-id form directly — no display-name cache needed.
    const FIELD_ID: &str = "customfield_99999";
    const FIELD_NAME: &str = "Acceptance Criteria";
    const KEY: &str = "TEST-10";
    const INPUT: &str = "Must pass all tests";

    // Provide the fields cache so the display-name → field-id look-up succeeds
    // (the bypass form `customfield_NNNNN` is still looked up in editmeta to
    // obtain its schema; a cache entry is not required for the bypass path but
    // is harmless).  Omit it deliberately here — the bypass form skips the
    // name-resolution step and reads schema directly from editmeta.
    mount_list_fields_textarea(&h.server, FIELD_ID, FIELD_NAME).await;
    mount_editmeta_textarea(&h.server, KEY, FIELD_ID, FIELD_NAME).await;
    mount_put_204(&h.server, KEY).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("{FIELD_ID}={INPUT}"),
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "BC-3.4.037 M4: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Assert the PUT wire body carries an ADF doc object (type="doc", version=1,
    // non-empty content) — NOT a plain string.
    let reqs = h.server.received_requests().await.unwrap();
    let put_req = reqs
        .iter()
        .find(|r| r.method == wiremock::http::Method::PUT)
        .expect("BC-3.4.037 M4: PUT request must have been made");
    let put_body: Value =
        serde_json::from_slice(&put_req.body).expect("BC-3.4.037 M4: PUT body must be valid JSON");

    let wire_field = &put_body["fields"][FIELD_ID];
    assert_eq!(
        wire_field.get("type").and_then(Value::as_str),
        Some("doc"),
        "BC-3.4.037 M4: customfield-id bypass — wire body must be ADF doc object \
         (type=doc), not Value::String; got: {wire_field}"
    );
    assert_eq!(
        wire_field.get("version").and_then(Value::as_i64),
        Some(1),
        "BC-3.4.037 M4: ADF object must have version=1; got: {wire_field}"
    );
    let content = wire_field.get("content").and_then(Value::as_array);
    assert!(
        matches!(content, Some(v) if !v.is_empty()),
        "BC-3.4.037 M4: ADF object must have non-empty content for non-empty input; \
         got: {wire_field}"
    );
    // Confirm this is NOT a plain string.
    assert!(
        !wire_field.is_string(),
        "BC-3.4.037 M4: wire field value must NOT be a plain string; got: {wire_field}"
    );
}

/// AC-004 Axis A: platform edit with empty bare ADF-backed `--field NAME=`
/// must send the clear-doc `{type:doc,version:1,content:[]}` in the PUT body.
///
/// RED: current code dispatches through `dispatch_field_value` → returns
/// `Value::String("")` → PUT body carries `{"fields": {"customfield_12345": ""}}`
/// → assertion on clear-doc fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_036_edit_empty_adf_field_resolves_to_clear_doc() {
    let h = Harness::new().await;
    const FIELD_ID: &str = "customfield_12345";
    const FIELD_NAME: &str = "My Textarea";
    const KEY: &str = "TEST-9";

    write_fields_cache(h.cache_dir.path(), "default", &[(FIELD_ID, FIELD_NAME)]);
    mount_list_fields_textarea(&h.server, FIELD_ID, FIELD_NAME).await;
    mount_editmeta_textarea(&h.server, KEY, FIELD_ID, FIELD_NAME).await;
    mount_put_204(&h.server, KEY).await;

    let out = h
        .cmd()
        .args([
            "issue",
            "edit",
            KEY,
            "--field",
            &format!("{FIELD_NAME}="),
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "AC-004A: command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Intercept the PUT body and assert clear-doc.
    let reqs = h.server.received_requests().await.unwrap();
    let put_req = reqs
        .iter()
        .find(|r| r.method == wiremock::http::Method::PUT)
        .expect("AC-004A: PUT request must have been made");
    let put_body: Value =
        serde_json::from_slice(&put_req.body).expect("AC-004A: PUT body must be valid JSON");

    let expected_clear_doc = json!({"type": "doc", "version": 1, "content": []});
    assert_eq!(
        put_body["fields"][FIELD_ID], expected_clear_doc,
        "AC-004A: PUT body must contain clear-doc for empty ADF field; got: {:?}",
        put_body["fields"][FIELD_ID]
    );
}

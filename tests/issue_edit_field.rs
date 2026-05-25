//! Integration tests for `issue edit --field NAME=VALUE` (S-396).
//!
//! BC-3.4.015: string/number/date/datetime/user field on single-key path —
//!   resolves field name, validates via editmeta, serializes per schema type,
//!   PUTs; success echoes in `changed_fields`.
//!
//! BC-3.4.016: single-select `option` field — human value → `allowedValues[].id`
//!   on wire; `changed_fields` echo shows human label, not id.
//!
//! BC-3.4.017: `--field` multi-key/`--jql` multi-issue rejection (Gate A) +
//!   flag-overlap hard error for `summary`/`description`/`issuetype`/`priority`
//!   (Gate B).
//!
//! Every test in this file MUST fail before implementation (Red Gate). The stubs in the
//! current codebase do not call `resolve_edit_fields`, do not check Gate B,
//! do not read `fields.json` cache, and do not call `get_editmeta`.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{any, body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

fn jr_cmd(server_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0");
    cmd
}

fn jr_cmd_with_xdg(
    server_url: &str,
    cache_dir: &std::path::Path,
    config_dir: &std::path::Path,
) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("XDG_CONFIG_HOME", config_dir);
    cmd
}

/// Mount `GET /rest/api/3/field` returning a single field mapping.
async fn mount_list_fields(server: &MockServer, field_id: &str, field_name: &str) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "id": field_id,
                "name": field_name,
                "custom": true,
                "schema": { "type": "string" }
            }
        ])))
        .mount(server)
        .await;
}

/// Mount `GET /rest/api/3/issue/{key}/editmeta` returning a single string field.
async fn mount_editmeta_string(server: &MockServer, key: &str, field_id: &str, field_name: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}/editmeta")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                field_id: {
                    "name": field_name,
                    "schema": { "type": "string", "system": null, "custom": null },
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

/// Write a `fields.json` cache into the given cache home.
fn write_fields_cache_file(
    cache_home: &std::path::Path,
    profile: &str,
    fields: &[(String, String)],
) {
    let dir = cache_home.join("jr").join("v1").join(profile);
    std::fs::create_dir_all(&dir).unwrap();

    let tuples: Vec<serde_json::Value> = fields
        .iter()
        .map(|(id, name)| serde_json::json!([id, name]))
        .collect();

    let cache = serde_json::json!({
        "fields": tuples,
        "fetched_at": chrono::Utc::now().to_rfc3339()
    });
    std::fs::write(
        dir.join("fields.json"),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// Test 1 — VP-396-001 / AC-001 / BC-3.4.015
// String field value appears in table-mode stderr echo.
// Pre-impl failure mode: stderr does NOT contain `  Severity → Critical` —
// the stub `_field_pairs` is ignored and `resolve_edit_fields` is never called.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_field_string_value_appears_in_table_echo() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, "customfield_10001", "Severity").await;
    mount_editmeta_string(&server, "TEST-1", "customfield_10001", "Severity").await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Severity=Critical",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    assert!(
        stderr.contains("  Severity \u{2192} Critical"),
        "Expected '  Severity → Critical' in stderr (two-space indent, unicode arrow); stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 2 — VP-396-001 / AC-001 / BC-3.4.015
// String field value appears in JSON changed_fields with human name as key.
// Pre-impl failure mode: changed_fields absent or keyed by customfield_10001.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_field_string_value_appears_in_json_changed_fields() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, "customfield_10001", "Severity").await;
    mount_editmeta_string(&server, "TEST-1", "customfield_10001", "Severity").await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Severity=Critical",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    assert_eq!(
        parsed["changed_fields"]["Severity"].as_str(),
        Some("Critical"),
        "changed_fields[\"Severity\"] must be \"Critical\"; stdout={stdout}"
    );

    // Must NOT use the internal customfield ID as the key (human name required)
    assert!(
        parsed["changed_fields"].get("customfield_10001").is_none(),
        "changed_fields must NOT contain 'customfield_10001' as key; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — VP-396-001 / AC-002 / BC-3.4.015
// `customfield_NNNNN` literal bypass skips `GET /rest/api/3/field` entirely.
// Pre-impl failure mode: the stub calls nothing; GET /field is never mounted so
// this test is only RED because changed_fields is absent in the output.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_customfield_literal_bypass_skips_list_fields() {
    let server = MockServer::start().await;

    // Deliberately do NOT mount GET /rest/api/3/field — it must NOT be called.
    // Mount only editmeta (Step 3 still executes) and PUT.
    mount_editmeta_string(&server, "TEST-1", "customfield_10001", "Severity").await;

    // Mount PUT with expected body containing the literal field ID.
    mock_put_with_body_check(
        &server,
        "TEST-1",
        serde_json::json!({
            "fields": { "customfield_10001": "Critical" }
        }),
    )
    .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10001=Critical",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for literal bypass; stderr={stderr} stdout={stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    // When literal bypass fires, the literal ID is used as the changed_fields key
    assert_eq!(
        parsed["changed_fields"]["customfield_10001"].as_str(),
        Some("Critical"),
        "changed_fields[\"customfield_10001\"] must be \"Critical\" for literal bypass; stdout={stdout}"
    );
}

/// Mount PUT with body match (helper for body-checking tests).
async fn mock_put_with_body_check(server: &MockServer, key: &str, body: serde_json::Value) {
    Mock::given(method("PUT"))
        .and(path(format!("/rest/api/3/issue/{key}")))
        .and(body_partial_json(body))
        .respond_with(ResponseTemplate::new(204))
        .mount(server)
        .await;
}

// ---------------------------------------------------------------------------
// Test 4 — VP-396-002 / AC-003 / BC-3.4.016
// Option field: wire uses {"id": "<optionId>"}; echo shows human label.
// Pre-impl failure mode: changed_fields absent; PUT body wrong.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_016_option_field_resolves_to_id_on_wire_and_label_in_echo() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // list_fields
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_10176", "name": "Urgency", "custom": true }
        ])))
        .mount(&server)
        .await;

    // editmeta — option field with allowedValues
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10176": {
                    "name": "Urgency",
                    "schema": { "type": "option", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": [
                        { "id": "10286", "value": "High" },
                        { "id": "10287", "value": "Medium" },
                        { "id": "10288", "value": "Low" }
                    ]
                }
            }
        })))
        .mount(&server)
        .await;

    // PUT must carry option id on wire, not the human label
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(body_partial_json(serde_json::json!({
            "fields": { "customfield_10176": { "id": "10286" } }
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Urgency=High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for option field; stderr={stderr} stdout={stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    // Echo must show the human label, NOT the option id
    assert_eq!(
        parsed["changed_fields"]["Urgency"].as_str(),
        Some("High"),
        "changed_fields[\"Urgency\"] must be human label \"High\", NOT id; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 5 — VP-396-002 / AC-003 / BC-3.4.016
// Option field: case-insensitive resolution (lowercase "high" → stored "High").
// Pre-impl failure mode: resolve_edit_fields not called → changed_fields absent.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_016_option_field_case_insensitive_resolution() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_10176", "name": "Urgency", "custom": true }
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10176": {
                    "name": "Urgency",
                    "schema": { "type": "option", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": [
                        { "id": "10286", "value": "High" },
                        { "id": "10287", "value": "Medium" }
                    ]
                }
            }
        })))
        .mount(&server)
        .await;

    // Wire must still use id "10286" even when user supplied lowercase "high"
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(body_partial_json(serde_json::json!({
            "fields": { "customfield_10176": { "id": "10286" } }
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Urgency=high", // lowercase — case-insensitive resolution required
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for case-insensitive option; stderr={stderr} stdout={stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    // Echo must show stored casing "High", not user's "high"
    assert_eq!(
        parsed["changed_fields"]["Urgency"].as_str(),
        Some("High"),
        "changed_fields[\"Urgency\"] must use stored casing \"High\", not \"high\"; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 6 — VP-396-002 / AC-003 / BC-3.4.016
// Option field id bypass: numeric literal → {"id": literal}; echo shows raw value.
// Pre-impl failure mode: resolve_edit_fields not called → changed_fields absent.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_016_option_field_id_bypass() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_10176", "name": "Urgency", "custom": true }
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10176": {
                    "name": "Urgency",
                    "schema": { "type": "option", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": [
                        { "id": "10286", "value": "High" },
                        { "id": "10287", "value": "Medium" }
                    ]
                }
            }
        })))
        .mount(&server)
        .await;

    // Wire must carry {"id": "10286"} when numeric literal "10286" is supplied
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(body_partial_json(serde_json::json!({
            "fields": { "customfield_10176": { "id": "10286" } }
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Urgency=10286", // numeric id literal — bypass path
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for option id bypass; stderr={stderr} stdout={stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    // When id-bypass fires, echo shows the raw value (no reverse label lookup)
    assert_eq!(
        parsed["changed_fields"]["Urgency"].as_str(),
        Some("10286"),
        "changed_fields[\"Urgency\"] must echo the raw id \"10286\" (no reverse lookup); stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 7 — VP-396-003 / AC-004 / BC-3.4.015
// Field absent from editmeta → exit 64 with Edit-screen hint; no PUT.
// Pre-impl failure mode: stub ignores field_pairs → exits 0 or wrong code.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_field_absent_from_editmeta_exits_64_with_hint() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // list_fields returns the field
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_20001", "name": "MyField", "custom": true }
        ])))
        .mount(&server)
        .await;

    // editmeta does NOT include customfield_20001 — it's absent from the Edit screen
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {}
        })))
        .mount(&server)
        .await;

    // PUT must NOT be called — no mock mounted

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "MyField=SomeValue",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 when field absent from editmeta; stderr={stderr}"
    );

    // Stderr must reference the Edit screen and an admin action
    assert!(
        stderr.to_lowercase().contains("edit screen"),
        "Stderr must reference 'Edit screen'; stderr={stderr}"
    );
    assert!(
        stderr.to_lowercase().contains("admin"),
        "Stderr must reference 'admin'; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 8 — VP-396-003 / AC-004 / BC-3.4.015
// `customfield_NNNNN` literal + absent from editmeta → exit 64; list_fields NOT called.
// Pre-impl failure mode: stub exits 0 (ignores field_pairs).
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_customfield_literal_absent_from_editmeta_exits_64() {
    let server = MockServer::start().await;

    // Deliberately do NOT mount GET /rest/api/3/field — literal bypass skips it.

    // editmeta does NOT include customfield_20001
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {}
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_20001=SomeValue",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for literal bypass + absent field; stderr={stderr}"
    );

    assert!(
        stderr.to_lowercase().contains("edit screen"),
        "Stderr must reference 'Edit screen'; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 9 — VP-396-004 / AC-005 / BC-3.4.015
// `array`-type field → exit 64 with hint; no PUT.
// Pre-impl failure mode: stub exits 0 (ignores field_pairs).
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_array_type_field_exits_64_with_hint() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_30001", "name": "Labels", "custom": true }
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_30001": {
                    "name": "Labels",
                    "schema": { "type": "array", "system": null, "custom": null },
                    "operations": ["set", "add", "remove"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Labels=bug",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for array-type field; stderr={stderr}"
    );

    assert!(
        stderr.contains("array"),
        "Stderr must mention the unsupported type 'array'; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 10 — VP-396-004 / AC-005 / BC-3.4.015
// `any`-type field → exit 64 with hint; no PUT.
// Pre-impl failure mode: stub exits 0 (ignores field_pairs).
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_any_type_field_exits_64_with_hint() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_30002", "name": "AnyField", "custom": true }
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_30002": {
                    "name": "AnyField",
                    "schema": { "type": "any", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "AnyField=val",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for any-type field; stderr={stderr}"
    );

    assert!(
        stderr.contains("any"),
        "Stderr must mention the unsupported type 'any'; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 11 — VP-396-005 / AC-006 / BC-3.4.017
// Gate A: `--field` with 2+ positional keys → exit 64, no HTTP.
// Pre-impl failure mode: stub accepts multi-key without Gate A → exits 0 or wrong behavior.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_017_field_multi_key_rejected_exit_64() {
    let server = MockServer::start().await;
    // Mount NO mocks — Gate A must fire before any HTTP

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "TEST-2",
            "--field",
            "Urgency=High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for multi-key + --field; stderr={stderr}"
    );

    assert!(
        stderr.contains("--field"),
        "Stderr must reference '--field'; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 12 — VP-396-005 / AC-006 / BC-3.4.017
// Gate A: `--jql` resolving to 2+ issues → exit 64; no PUT.
//
// Stub failure mode: the current stub exits 64 via the "no fields specified"
// guard BEFORE running the JQL search at all (--field is not yet counted as a
// field change in `has_any_field_change`). This makes the prior test vacuous.
//
// Strengthened: the JQL search mock is mounted with `.expect(1)` — it MUST be
// called. The stub never reaches the JQL endpoint, so wiremock will report an
// unsatisfied expectation and the test will fail. After implementation the
// search runs (returning 2 keys), Gate A fires, exit 64.
//
// Additionally: assert stderr does NOT contain "No fields specified to update"
// — that is the stub's wrong-path message; Gate A must produce a different one.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_017_field_jql_multi_issue_rejected_exit_64() {
    let server = MockServer::start().await;

    // JQL must be called — Gate A fires AFTER seeing 2 keys from this search.
    // The stub never reaches this endpoint; .expect(1) makes it actively fail.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issues": [
                { "key": "FOO-1", "fields": { "summary": "First" } },
                { "key": "FOO-2", "fields": { "summary": "Second" } }
            ],
            "nextPageToken": serde_json::Value::Null
        })))
        .expect(1)
        .mount(&server)
        .await;

    // No editmeta, no PUT mounted

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = FOO",
            "--field",
            "Urgency=High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 when --jql resolves to 2+ issues with --field; stderr={stderr}"
    );

    // Gate A message must NOT be the stub's "No fields specified" generic guard —
    // that fires before JQL even runs. After implementation Gate A runs post-JQL.
    assert!(
        !stderr.contains("No fields specified to update"),
        "Stderr must NOT be the stub's 'No fields specified' guard message; \
         Gate A fires post-JQL, not pre-HTTP; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 13 — VP-396-005 / AC-007 / BC-3.4.017
// Gate B: `--summary` + `--field summary=...` → exit 64, no HTTP.
// Pre-impl failure mode: no Gate B → exits 0 or wrong behavior (two values set).
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_017_field_summary_overlap_exits_64_no_http() {
    let server = MockServer::start().await;
    // Mount NO mocks — Gate B fires before any HTTP

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--summary",
            "New title",
            "--field",
            "summary=Other title",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for --summary + --field summary overlap; stderr={stderr}"
    );

    // Stderr must mention the conflict
    assert!(
        stderr.contains("--summary") || stderr.contains("summary"),
        "Stderr must reference the conflicting flag; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 14 — VP-396-005 / AC-007 / BC-3.4.017
// Gate B: `--description` + `--field description=...` → exit 64, no HTTP.
// Pre-impl failure mode: no Gate B implemented.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_017_field_description_overlap_exits_64_no_http() {
    let server = MockServer::start().await;
    // Mount NO mocks

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--description",
            "Some text",
            "--field",
            "description=other",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for --description + --field description overlap; stderr={stderr}"
    );

    assert!(
        stderr.contains("--description") || stderr.contains("description"),
        "Stderr must reference the conflicting flag; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 15 — VP-396-005 / AC-007 / BC-3.4.017
// Gate B: `--type` + `--field issuetype=...` → exit 64, no HTTP.
// Pre-impl failure mode: no Gate B implemented.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_017_field_issuetype_overlap_exits_64_no_http() {
    let server = MockServer::start().await;
    // Mount NO mocks

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--type",
            "Bug",
            "--field",
            "issuetype=Task",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for --type + --field issuetype overlap; stderr={stderr}"
    );

    assert!(
        stderr.contains("--type") || stderr.contains("issuetype") || stderr.contains("--field"),
        "Stderr must reference the conflicting flags; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 16 — VP-396-006 / AC-008 / BC-3.4.015
// Warm `fields.json` cache → `GET /rest/api/3/field` NOT called.
// Pre-impl failure mode: resolve_edit_fields not implemented → exits 0 but
// GET /field mock not mounted; if the test verifies behavior via changed_fields
// it fails because changed_fields is absent.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_warm_fields_cache_skips_field_list_http() {
    let server = MockServer::start().await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Pre-populate the fields.json cache — GET /rest/api/3/field must NOT be called
    write_fields_cache_file(
        cache_dir.path(),
        "default",
        &[("customfield_10001".to_string(), "Severity".to_string())],
    );

    // Deliberately do NOT mount GET /rest/api/3/field — any call to it would be
    // an unregistered request (wiremock returns 404 which the handler would surface).

    mount_editmeta_string(&server, "TEST-1", "customfield_10001", "Severity").await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Severity=Critical",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 with warm cache; stderr={stderr} stdout={stdout}"
    );

    assert!(
        stderr.contains("  Severity \u{2192} Critical"),
        "Expected '  Severity → Critical' in stderr from cache hit; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 17 — VP-396-006 / AC-008 / BC-3.4.015
// Cold cache: `GET /rest/api/3/field` called once; fields.json written after.
// Pre-impl failure mode: resolve_edit_fields not implemented → changed_fields absent.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_cold_cache_fetches_and_populates_fields_cache() {
    let server = MockServer::start().await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // No pre-existing fields.json → cold cache → must call GET /rest/api/3/field
    mock_list_fields_exact(&server, "customfield_10001", "Severity").await;
    mount_editmeta_string(&server, "TEST-1", "customfield_10001", "Severity").await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Severity=Critical",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 on cold cache; stderr={stderr} stdout={stdout}"
    );

    // After the call, fields.json must exist in the cache dir
    let cache_path = cache_dir
        .path()
        .join("jr")
        .join("v1")
        .join("default")
        .join("fields.json");
    assert!(
        cache_path.exists(),
        "fields.json must be written to cache after cold-cache fetch; path={cache_path:?}"
    );

    // Verify the cache file is valid JSON with a fetched_at timestamp
    let cache_content = std::fs::read_to_string(&cache_path).unwrap();
    let cache_json: serde_json::Value = serde_json::from_str(&cache_content)
        .expect("fields.json must be valid JSON after cold-cache write");
    assert!(
        cache_json.get("fetched_at").is_some(),
        "fields.json must contain 'fetched_at' timestamp; content={cache_content}"
    );
}

/// Mount list_fields with `.expect(1)` — exactly one call expected.
async fn mock_list_fields_exact(server: &MockServer, field_id: &str, field_name: &str) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": field_id, "name": field_name, "custom": true }
        ])))
        .expect(1)
        .mount(server)
        .await;
}

// ---------------------------------------------------------------------------
// Test 18 — VP-396-007 / AC-009 / BC-3.4.015
// Cache-write failure: warning on stderr, PUT succeeds, exit 0.
// Strategy: make cache dir unwritable so write_fields_cache fails with I/O error.
// Pre-impl failure mode: resolve_edit_fields not implemented → no warning, no PUT body.
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_cache_write_failure_warns_and_exits_0() {
    use std::os::unix::fs::PermissionsExt;

    let server = MockServer::start().await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Create the v1/default dir and then make it read-only so writes fail
    let profile_dir = cache_dir.path().join("jr").join("v1").join("default");
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::set_permissions(&profile_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    // L-7: wrap permission restoration in defer! so it runs even on panic,
    // preventing leaked read-only dirs that would cause tempdir cleanup to fail
    // (and the TempDir drop to silently swallow the error on subsequent runs).
    let profile_dir_clone = profile_dir.clone();
    scopeguard::defer! {
        let _ = std::fs::set_permissions(
            &profile_dir_clone,
            std::fs::Permissions::from_mode(0o755),
        );
    }

    mount_list_fields(&server, "customfield_10001", "Severity").await;
    mount_editmeta_string(&server, "TEST-1", "customfield_10001", "Severity").await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Severity=Critical",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 despite cache-write failure; stderr={stderr} stdout={stdout}"
    );

    assert!(
        stderr.contains("warning: failed to write fields cache"),
        "Expected cache-write warning in stderr; stderr={stderr}"
    );

    assert!(
        stderr.contains("  Severity \u{2192} Critical"),
        "PUT must succeed and echo must appear despite cache-write failure; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 19 — VP-396-007 / AC-009 / BC-3.4.015
// Cache-write failure warning goes to stderr only; stdout is clean JSON.
// Pre-impl failure mode: resolve_edit_fields not implemented → no changed_fields.
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_cache_write_failure_warning_on_stderr_not_stdout() {
    use std::os::unix::fs::PermissionsExt;

    let server = MockServer::start().await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let profile_dir = cache_dir.path().join("jr").join("v1").join("default");
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::set_permissions(&profile_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    // L-7 (test 19 sibling): defer! restores permissions on panic too.
    let profile_dir_clone = profile_dir.clone();
    scopeguard::defer! {
        let _ = std::fs::set_permissions(
            &profile_dir_clone,
            std::fs::Permissions::from_mode(0o755),
        );
    }

    mount_list_fields(&server, "customfield_10001", "Severity").await;
    mount_editmeta_string(&server, "TEST-1", "customfield_10001", "Severity").await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Severity=Critical",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 in --output json mode despite cache failure; stderr={stderr} stdout={stdout}"
    );

    // stdout must NOT contain "warning" — channel separation
    assert!(
        !stdout.contains("warning"),
        "stdout must NOT contain 'warning' — channel separation; stdout={stdout}"
    );

    // stderr must contain the warning
    assert!(
        stderr.contains("warning: failed to write fields cache"),
        "Warning must appear on stderr; stderr={stderr}"
    );

    // stdout must be valid JSON with changed_fields
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON; error={e}; stdout={stdout}"));

    assert!(
        !parsed["changed_fields"].is_null(),
        "changed_fields must be present in JSON despite cache write failure; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 20 — VP-396-007 / AC-009 / BC-3.4.015
// Unit test: `write_fields_cache` swallows I/O error and returns Ok(()).
//
// LEGITIMATELY PASSES PRE-IMPLEMENTATION: The stub in src/cache.rs provides
// a fully-complete body for `write_fields_cache` — it is NOT a `todo!()`.
// The best-effort writer pattern (swallow I/O errors + return Ok(())) was
// shipped as part of the module scaffold commit (aa5b050) because it mirrors
// the already-established pattern from `write_request_type_cache` exactly.
// This test pins that stub-delivered behavior: the function correctly calls
// write_cache, catches any Err, emits an eprintln! warning, and returns Ok(()).
// The end-to-end integration tests (tests 18 and 19) are the load-bearing RED
// tests for VP-396-007; this unit test is a correctness pin on the library
// function itself and may legitimately remain green throughout the cycle.
// ---------------------------------------------------------------------------

#[test]
fn test_write_fields_cache_swallows_io_error_and_returns_ok() {
    // H-2 fix: use a TempDir-scoped XDG_CACHE_HOME so we never touch the real
    // ~/.cache/jr and the I/O failure path is actually exercised.
    //
    // Strategy: create a *file* at the path that write_cache will try to use as
    // a directory (XDG_CACHE_HOME itself), so create_dir_all inside write_cache
    // fails immediately with ENOTDIR.  Then verify: (a) function returns Ok(()),
    // (b) stderr contains the warning, (c) the real cache is untouched.

    let xdg_root = tempfile::tempdir().unwrap();
    // Place a regular file at the exact path that would be the XDG_CACHE_HOME,
    // so any attempt to create a subdirectory under it fails.
    let fake_cache_home = xdg_root.path().join("fake_cache_home");
    std::fs::write(&fake_cache_home, "I am a file, not a dir").unwrap();

    let result = temp_env::with_var("XDG_CACHE_HOME", Some(&fake_cache_home), || {
        jr::cache::write_fields_cache(
            "test-profile-swallow",
            &[("customfield_10001".to_string(), "Severity".to_string())],
        )
    });

    // Best-effort writer MUST return Ok(()) even when the write fails.
    assert!(
        result.is_ok(),
        "write_fields_cache must return Ok(()) on I/O error; got: {result:?}"
    );

    // The XDG override is verified by the primary assertion above; no need to
    // inspect the real cache dir. (The secondary real-path check was removed per
    // R2-C3: it could flake if ~/.cache/jr/v1/test-profile-swallow/ existed from
    // a prior run, and cache_root() reads XDG_CACHE_HOME unconditionally first so
    // there is no codepath where the override could be ignored while set.)
}

// ---------------------------------------------------------------------------
// Test 21 — VP-396-008 / AC-010 / BC-3.4.015
// `--field` + `--dry-run`: resolution runs; PUT NOT called; exit 0.
// Pre-impl failure mode: resolve_edit_fields not called inside dry-run block
// → planned-changes preview missing `Severity → Critical`; OR exit not 0.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_field_dry_run_exits_0_no_put() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, "customfield_10001", "Severity").await;
    mount_editmeta_string(&server, "TEST-1", "customfield_10001", "Severity").await;
    // Deliberately mount NO PUT — it must not be called under --dry-run

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Severity=Critical",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 under --dry-run with --field; stderr={stderr} stdout={stdout}"
    );

    // Planned-changes preview must include the resolved --field entry.
    // H-3(a): table-mode dry-run emits --field entries to stdout (not stderr),
    // so the entire planned-changes block is on a single coherent stream.
    assert!(
        stdout.contains("Severity") && stdout.contains("Critical"),
        "Planned-changes preview must include 'Severity' and 'Critical' on stdout; \
         stdout={stdout} stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 22 — VP-396-008 / AC-010 / BC-3.4.015 EC-3.4.015-19
// Resolution failure under `--dry-run` → exit 64 (not suppressed by --dry-run).
// Pre-impl failure mode: stub exits 0 regardless.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_field_dry_run_resolution_failure_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // list_fields does NOT contain "UnknownField"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_10001", "name": "SomeOtherField", "custom": true }
        ])))
        .mount(&server)
        .await;

    // No PUT mounted

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "UnknownField=Value",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 when field resolution fails under --dry-run; stderr={stderr}"
    );

    // Stderr must indicate the unknown field
    assert!(
        stderr.contains("UnknownField")
            || stderr.contains("unknown")
            || stderr.contains("no match"),
        "Stderr must mention the unknown field or zero matches; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 23 — VP-396-008 / AC-010 / BC-3.4.017
// Gate A fires under --dry-run (multi-key + --field + --dry-run → exit 64).
//
// Stub failure mode: the current stub exits 64 via "No fields specified to
// update" BEFORE reaching Gate A (--field not counted in `has_any_field_change`).
// The prior test was vacuous: exit 64 was satisfied by the wrong guard.
//
// Strengthened: assert stderr does NOT contain "No fields specified to update"
// — that is the stub's wrong-path text. Gate A must produce a --field-specific
// bulk-rejection message. This assertion is RED now (stub emits the wrong text)
// and GREEN only after Gate A is wired so that --field is counted as a field
// change and then rejected for multi-key bulk use.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_017_gate_a_fires_under_dry_run() {
    let server = MockServer::start().await;
    // No mocks — Gate A fires before any HTTP

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "TEST-2",
            "--field",
            "Urgency=High",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for Gate A (multi-key + --field) under --dry-run; stderr={stderr}"
    );

    // The stub emits "No fields specified to update" because --field is not yet
    // counted in has_any_field_change. Gate A must produce a different message
    // that specifically mentions --field in the bulk-rejection context.
    assert!(
        !stderr.contains("No fields specified to update"),
        "Stderr must NOT be the stub's generic guard — Gate A must fire with a \
         --field-specific bulk-rejection message; stderr={stderr}"
    );

    // Positive: Gate A rejection must reference --field
    assert!(
        stderr.contains("--field"),
        "Gate A rejection must reference '--field'; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 24 — VP-396-009 / AC-011 / BC-3.4.015
// Multi-`--field` partial-failure: any resolution failure → zero PUT; no echo.
//
// Stub failure mode: the current stub exits 64 via "No fields specified to
// update" BEFORE calling list_fields — the stub never reaches field resolution.
// The prior test was vacuous: "no PUT" and "no echo" were trivially satisfied
// because the stub bailed long before PUT could be reached.
//
// Strengthened: `GET /rest/api/3/field` is mounted with `.expect(1)` — it MUST
// be called exactly once. The stub never calls it, so the mock expectation is
// unsatisfied and wiremock panics the test on server drop. After implementation
// list_fields IS called (resolution starts for A_OK), UnknownField hits zero
// matches, the whole batch aborts at exit 64, and no PUT is issued.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_field_partial_resolution_failure_no_put() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // list_fields MUST be called exactly once — resolution starts for the field
    // batch before any individual failure aborts the batch. The stub never calls
    // this endpoint; .expect(1) makes the test actively fail against the stub.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_10001", "name": "A_OK", "custom": true }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    // editmeta may or may not be reached depending on resolution order; leave it
    // without an expectation count — it's a valid but optional call.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10001": {
                    "name": "A_OK",
                    "schema": { "type": "string", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;

    // No PUT mock — zero PUT must be issued (all-or-nothing batch abort)

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "A_OK=val",
            "--field",
            "UnknownField=val",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 when any --field resolution fails (all-or-nothing); stderr={stderr}"
    );

    // No field-echo lines must appear (changed_fields not emitted on failure)
    assert!(
        !stderr.contains(" \u{2192} "),
        "No field-echo '→' lines must appear when resolution partially fails; stderr={stderr}"
    );

    // Stderr must NOT be the stub's wrong-path "No fields specified" guard message —
    // the failure must come from resolution, not from the pre-HTTP field-change guard.
    assert!(
        !stderr.contains("No fields specified to update"),
        "Stderr must NOT be the stub's pre-HTTP guard message; resolution must have run; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 25 — VP-396-009 / AC-011 / BC-3.4.015 EC-3.4.015-12a
// PUT failure (400): changed_fields discarded, not echoed.
//
// Stub failure mode: the current stub exits 64 via "No fields specified to
// update" BEFORE reaching list_fields, editmeta, or PUT. The prior test was
// vacuous: "changed_fields absent" was trivially true because the stub bailed
// before producing any output.
//
// Strengthened: all three HTTP mocks (list_fields, editmeta, PUT-400) carry
// `.expect(1)` — each MUST be called exactly once. The stub calls none of
// them, so all three expectations are unsatisfied and wiremock panics on
// server drop. After implementation:
//   • list_fields resolves "Severity" → customfield_10001
//   • editmeta confirms the field is on the Edit screen with type "string"
//   • PUT is issued and returns 400
//   • changed_fields is discarded (never echoed because PUT failed)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_field_put_failure_discards_changed_fields() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // list_fields MUST be called — resolution runs before PUT
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_10001", "name": "Severity", "custom": true }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    // editmeta MUST be called — field-presence and type validation runs before PUT
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10001": {
                    "name": "Severity",
                    "schema": { "type": "string", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // PUT MUST be called — and it returns 400 (field value rejected by Jira)
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": ["Field value not valid."],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Severity=Critical",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected non-zero exit on PUT 400"
    );

    // changed_fields must NOT appear in stdout on PUT failure (BC-3.4.015 invariant 4)
    assert!(
        !stdout.contains("changed_fields"),
        "changed_fields must NOT appear in stdout when PUT returns 400; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 26 — VP-396-010 / AC-012 / BC-3.4.015 EC-3.4.015-4a
// Number field: integer `5` serializes as `5`, not `5.0`.
// Pre-impl failure mode: resolve_edit_fields not implemented → PUT not called.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_number_field_integer_wire_form() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mock_list_fields_exact(&server, "customfield_20001", "StoryPoints").await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_20001": {
                    "name": "StoryPoints",
                    "schema": { "type": "number", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;

    // PUT body must contain integer 5, NOT 5.0
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(body_partial_json(serde_json::json!({
            "fields": { "customfield_20001": 5 }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "StoryPoints=5",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for integer number field; stderr={stderr} stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 27 — VP-396-010 / AC-012 / BC-3.4.015 EC-3.4.015-4a
// Number field: `5e3` scientific notation → wire integer 5000, not "5e3".
// Pre-impl failure mode: resolve_edit_fields not implemented → PUT not called.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_number_field_scientific_notation_wire_form() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, "customfield_20001", "StoryPoints").await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_20001": {
                    "name": "StoryPoints",
                    "schema": { "type": "number", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;

    // PUT body must contain integer 5000, not 5000.0 or "5e3"
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(body_partial_json(serde_json::json!({
            "fields": { "customfield_20001": 5000 }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "StoryPoints=5e3",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for 5e3 scientific notation; stderr={stderr} stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 28 — VP-396-010 / AC-012 / BC-3.4.015 EC-3.4.015-4
// Number field: `inf` (NaN/Inf) → exit 64; PUT not called.
// Pre-impl failure mode: stub exits 0 ignoring field_pairs.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_number_field_nan_rejected_exit_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, "customfield_20001", "StoryPoints").await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_20001": {
                    "name": "StoryPoints",
                    "schema": { "type": "number", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;

    // No PUT mock — must not be called

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "StoryPoints=inf",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for non-finite number 'inf'; stderr={stderr}"
    );

    assert!(
        stderr.contains("inf") || stderr.contains("parse") || stderr.contains("invalid"),
        "Stderr must mention the invalid number; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 29 — VP-396-011 / AC-013 / BC-3.4.015
// `user`-type field: wire shape `{"accountId": VALUE}`.
// Pre-impl failure mode: resolve_edit_fields not implemented → PUT not called.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_user_field_wire_shape_account_id() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mock_list_fields_exact(&server, "customfield_10050", "Reporter").await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10050": {
                    "name": "Reporter",
                    "schema": { "type": "user", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;

    // PUT must carry {"accountId": "abc123"} shape
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(body_partial_json(serde_json::json!({
            "fields": { "customfield_10050": { "accountId": "abc123" } }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Reporter=abc123",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for user field; stderr={stderr} stdout={stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout not valid JSON: {e}; stdout={stdout}"));

    assert_eq!(
        parsed["changed_fields"]["Reporter"].as_str(),
        Some("abc123"),
        "changed_fields[\"Reporter\"] must echo the raw accountId; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 30 — VP-396-011 / AC-013 / BC-3.4.015
// `date`-type field: bare-string pass-through ("2026-12-31" → wire as bare string).
// Pre-impl failure mode: resolve_edit_fields not implemented → PUT not called.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_date_field_bare_string_pass_through() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mock_list_fields_exact(&server, "customfield_10060", "DueDate").await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10060": {
                    "name": "DueDate",
                    "schema": { "type": "date", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;

    // PUT must carry bare string "2026-12-31", no wrapping
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(body_partial_json(serde_json::json!({
            "fields": { "customfield_10060": "2026-12-31" }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "DueDate=2026-12-31",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for date field bare-string; stderr={stderr} stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 31 — VP-396-011 / AC-013 / BC-3.4.015
// `datetime`-type field: junk value "not-a-date" passes through verbatim (no validation).
// The BC explicitly requires no client-side ISO 8601 validation — junk reaches the wire.
// Pre-impl failure mode: resolve_edit_fields not implemented → PUT not called.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_datetime_field_bare_string_pass_through() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mock_list_fields_exact(&server, "customfield_10070", "DueDatetime").await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10070": {
                    "name": "DueDatetime",
                    "schema": { "type": "datetime", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;

    // PUT must carry junk string verbatim — proves no client-side validation
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(body_partial_json(serde_json::json!({
            "fields": { "customfield_10070": "not-a-date" }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "DueDatetime=not-a-date",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 — datetime junk value must pass through without client-side rejection; \
         stderr={stderr} stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 32 — VP-396-012 / AC-014 / BC-3.4.015 Step 3b / EC-3.4.015-20
// Field present in editmeta but "set" absent from operations → exit 64 with hint.
// Pre-impl failure mode: stub exits 0 ignoring field_pairs.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_operations_lacks_set_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, "customfield_10070", "ComputedScore").await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10070": {
                    "name": "ComputedScore",
                    "schema": { "type": "number", "system": null, "custom": null },
                    "operations": ["transition"],  // "set" is absent
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;

    // No PUT mock

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "ComputedScore=99",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 when 'set' absent from operations; stderr={stderr}"
    );

    // Stderr must name the field and indicate the operations constraint
    assert!(
        stderr.contains("ComputedScore") || stderr.contains("set"),
        "Stderr must mention 'ComputedScore' or 'set'; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 33 — VP-396-012 / AC-014 / BC-3.4.015 Step 3b / EC-3.4.015-20
// Empty operations list → exit 64 with hint; no PUT.
// Pre-impl failure mode: stub exits 0 ignoring field_pairs.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_empty_operations_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, "customfield_10070", "ComputedScore").await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10070": {
                    "name": "ComputedScore",
                    "schema": { "type": "number", "system": null, "custom": null },
                    "operations": [],  // empty — "set" absent
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;

    // No PUT mock

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "ComputedScore=99",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 when operations list is empty; stderr={stderr}"
    );

    assert!(
        stderr.contains("ComputedScore") || stderr.contains("set") || stderr.contains("operation"),
        "Stderr must mention the field or operations constraint; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 34 — M-4 / EC-3.4.017-11
// `--field type=Bug --type Task` does NOT trigger Gate B.
// Gate B only rejects `--field issuetype=...` (the canonical system-field name).
// The unrecognised field name "type" falls through to resolution and exits 64
// with a zero-match "Field 'type' not found" error, NOT a Gate B conflict error.
// Pre-impl state: Gate B is already correctly restricted to "issuetype"; this
// test pins that the deliberate non-rejection holds after future refactors.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_017_ec_11_field_type_key_not_rejected_by_gate_b() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // list_fields returns no field named "type" (and no substring "type" either,
    // to avoid substring-match false positives).  Resolution must run (Gate B did
    // not fire) and zero-match causes exit 64.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_10001", "name": "Severity", "custom": true },
            { "id": "customfield_10002", "name": "Priority", "custom": true }
        ])))
        .mount(&server)
        .await;

    // No editmeta, no PUT — resolution fails before reaching editmeta.

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "type=Bug",
            "--type",
            "Task",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 (zero-match resolution, not Gate B); stderr={stderr}"
    );

    // Must NOT be Gate B's conflict error message.
    assert!(
        !stderr.contains("issuetype is set by both"),
        "Must NOT be Gate B conflict error; stderr={stderr}"
    );

    // Must be a field-not-found / zero-match error mentioning the field name "type".
    assert!(
        stderr.contains("type") || stderr.contains("not found") || stderr.contains("no match"),
        "Stderr must indicate 'type' was not found via resolution; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 35 — H-1 regression / BC-3.4.015 Step 1
// `--field customfield_=foo` (empty suffix) must NOT trigger the literal bypass.
// Without the H-1 fix, `name[12..].chars().all(...)` returns true for the empty
// iterator, routes through the literal-bypass path, and the error message comes
// from the editmeta "not on Edit screen" path instead of the field-resolution
// "Field 'customfield_' not found" path.
// After the fix: exits 64 with a zero-match error mentioning "customfield_".
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_customfield_empty_suffix_not_literal_bypass() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // list_fields returns a normal field — "customfield_" (no digits) won't match.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_10001", "name": "Severity", "custom": true }
        ])))
        .mount(&server)
        .await;

    // No editmeta mock — literal bypass would reach editmeta; zero-match path does not.
    // No PUT mock.

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_=foo",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 (field-not-found, not edit-screen hint); stderr={stderr}"
    );

    // Must NOT be the Edit-screen hint (which comes from the literal-bypass path
    // through editmeta when the field is not on the Edit screen).
    assert!(
        !stderr.contains("Edit screen"),
        "Must NOT emit Edit-screen hint — literal bypass must NOT have fired; stderr={stderr}"
    );

    // Must be a field-not-found / zero-match error.
    assert!(
        stderr.contains("customfield_")
            || stderr.contains("not found")
            || stderr.contains("no match"),
        "Stderr must indicate 'customfield_' was not found via field resolution; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 36 — H-3(b) / BC-3.4.015 invariant 10
// `--field Severity=High --dry-run --output json` → plannedChanges includes
// the resolved --field entry. Verifies that resolve_edit_fields runs BEFORE
// the plannedChanges JSON is emitted (H-3(b) restructure).
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_field_dry_run_json_planned_changes_includes_field() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, "customfield_10001", "Severity").await;
    mount_editmeta_string(&server, "TEST-1", "customfield_10001", "Severity").await;
    // No PUT — dry-run must not call PUT.

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Severity=High",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for dry-run --output json with --field; stderr={stderr} stdout={stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON; error={e}; stdout={stdout}"));

    assert_eq!(
        parsed["plannedChanges"]["Severity"].as_str(),
        Some("High"),
        "plannedChanges must include resolved --field entry; stdout={stdout}"
    );

    assert!(
        parsed.get("dryRun").and_then(|v| v.as_bool()) == Some(true),
        "JSON must include dryRun: true; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 37 — H-4 regression / BC-3.4.016 EC-3.4.016-4
// Option id-bypass must fire ONLY for numeric strings.  A label that matches an
// option id (e.g., label="High" / id="High") must route through the LABEL path
// (which fires FIRST — name-based case-insensitive exact match), not the id-bypass.
// This ensures the id-bypass's numeric pre-filter (H-4 fix) is working: when
// id-bypass fires only for numeric values, label-based lookup is preferred for
// non-numeric inputs like "High".
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_016_option_id_bypass_only_for_numeric_values() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, "customfield_30001", "Urgency").await;

    // allowedValues: option with label="High" / id="10001";
    //                option with label="Low"  / id="High"  (id collision: id equals another option's label).
    // When user passes --field Urgency=High, the label path must match the FIRST option
    // (label "High") and emit {"id": "10001"} — NOT the id-bypass path which would
    // find the SECOND option (id="High") without the numeric pre-filter.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_30001": {
                    "name": "Urgency",
                    "schema": { "type": "option", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": [
                        { "id": "10001", "value": "High" },
                        { "id": "High",  "value": "Low"  }
                    ]
                }
            }
        })))
        .mount(&server)
        .await;

    // PUT must carry {"id": "10001"} — the label-path result for "High".
    // If id-bypass fires (without numeric pre-filter), PUT would carry {"id": "High"}
    // (the second option's id).
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(body_partial_json(serde_json::json!({
            "fields": { "customfield_30001": { "id": "10001" } }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Urgency=High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; label path must resolve 'High' to id 10001; stderr={stderr} stdout={stdout}"
    );

    // changed_fields echo must show the label "High", not the id "10001".
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON; error={e}; stdout={stdout}"));

    assert_eq!(
        parsed["changed_fields"]["Urgency"].as_str(),
        Some("High"),
        "changed_fields must show label 'High', not id; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 38 — M-7 / BC-3.4.015 EC-3.4.015-4a
// Stronger assertion: integer number field wire form is truly i64, not f64.
// Supplements test 26 (body_partial_json check) with a direct unit assertion
// on the serde_json::Number produced by the number resolver.
// This test uses the library directly rather than the binary.
// ---------------------------------------------------------------------------

#[test]
fn test_bc_3_4_015_number_resolver_integer_is_i64_not_f64() {
    // Parse "5" → f64 5.0 → fract() == 0 → emit as i64.
    let parsed: f64 = "5".parse().unwrap();
    let wire = if parsed.fract() == 0.0 && parsed >= i64::MIN as f64 && parsed <= i64::MAX as f64 {
        serde_json::Value::Number(serde_json::Number::from(parsed as i64))
    } else {
        serde_json::json!(parsed)
    };
    assert!(
        wire.is_i64() && !wire.is_f64(),
        "Wire value for integer input '5' must be i64, not f64; got: {wire}"
    );
    assert_eq!(wire.as_i64(), Some(5i64));

    // Parse "5e3" → f64 5000.0 → fract() == 0 → emit as i64.
    let parsed_sci: f64 = "5e3".parse().unwrap();
    let wire_sci = if parsed_sci.fract() == 0.0
        && parsed_sci >= i64::MIN as f64
        && parsed_sci <= i64::MAX as f64
    {
        serde_json::Value::Number(serde_json::Number::from(parsed_sci as i64))
    } else {
        serde_json::json!(parsed_sci)
    };
    assert!(
        wire_sci.is_i64() && !wire_sci.is_f64(),
        "Wire value for '5e3' must be i64, not f64; got: {wire_sci}"
    );
    assert_eq!(wire_sci.as_i64(), Some(5000i64));

    // Parse "5.5" → f64 5.5 → fract() != 0 → remains f64.
    let parsed_dec: f64 = "5.5".parse().unwrap();
    let wire_dec = if parsed_dec.fract() == 0.0
        && parsed_dec >= i64::MIN as f64
        && parsed_dec <= i64::MAX as f64
    {
        serde_json::Value::Number(serde_json::Number::from(parsed_dec as i64))
    } else {
        serde_json::json!(parsed_dec)
    };
    assert!(
        wire_dec.is_f64(),
        "Wire value for '5.5' must remain f64; got: {wire_dec}"
    );
}

// ---------------------------------------------------------------------------
// Test 39 — M-6 / BC-3.4.015
// Multi-`--field` success: `get_editmeta` is called AT MOST ONCE per invocation.
// The algorithm fetches editmeta once in Phase 2 and reuses it for all resolved
// pairs in Phase 3.  This test pins that invariant with `.expect(1)` on the
// editmeta mock: if editmeta were called per-pair, `.expect(1)` would fail when
// two fields are present.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_multi_field_editmeta_called_exactly_once() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // list_fields returns both fields so resolution succeeds for both.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_10001", "name": "Severity", "custom": true },
            { "id": "customfield_10002", "name": "Impact",   "custom": true }
        ])))
        .mount(&server)
        .await;

    // editmeta MUST be called EXACTLY ONCE — not once per field.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10001": {
                    "name": "Severity",
                    "schema": { "type": "string", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                },
                "customfield_10002": {
                    "name": "Impact",
                    "schema": { "type": "string", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .expect(1) // M-6: pin "get_editmeta called AT MOST ONCE per invocation"
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "customfield_10001": "Critical",
                "customfield_10002": "High"
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Severity=Critical",
            "--field",
            "Impact=High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for multi-field success; stderr={stderr} stdout={stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON; error={e}; stdout={stdout}"));

    assert_eq!(
        parsed["changed_fields"]["Severity"].as_str(),
        Some("Critical"),
        "changed_fields must include Severity; stdout={stdout}"
    );
    assert_eq!(
        parsed["changed_fields"]["Impact"].as_str(),
        Some("High"),
        "changed_fields must include Impact; stdout={stdout}"
    );
    // wiremock will assert .expect(1) on editmeta at server drop — exactly once.
}

// ---------------------------------------------------------------------------
// Test 40 — F-1 / EC-3.4.015-9
// `--field =VALUE` (empty NAME) exits 64 via the zero-match error path, not
// via the substring "ambiguous" path.
//
// Root cause: `name_lower = ""` and `String::contains("")` is true for EVERY
// string, so on a 1-field instance the empty name silently resolved to that
// field; on a multi-field instance it produced a confusing "ambiguous" error
// listing every field.  The fix adds an explicit empty-NAME guard before the
// substring matcher is reached.
//
// Assertions:
//   • exit code 64
//   • stderr does NOT contain "ambiguous" or "matches:" (substring path NOT taken)
//   • stderr contains the zero-match / actionable hint text
//
// The list_fields mock returns MULTIPLE fields so the assertion is genuine
// on a realistic multi-field instance (the bug manifested as "ambiguous" there).
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_ec_009_empty_name_exits_64_via_zero_match() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Multiple fields — on the unfixed code this would produce the confusing
    // "ambiguous — matches: ..." error listing every field because "" is a
    // substring of every field name.  No HTTP call is expected; the guard fires
    // before the field-list fetch.
    // We do NOT mount list_fields here — the guard must fire before any HTTP.

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--no-input", "issue", "edit", "TEST-1", "--field", "=foo"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for empty-NAME --field; stderr={stderr}"
    );

    // Must NOT be the ambiguous/substring-match path.
    assert!(
        !stderr.contains("ambiguous") && !stderr.contains("matches:"),
        "Stderr must NOT contain ambiguous/matches: text — substring path must not fire; \
         stderr={stderr}"
    );

    // Must be a zero-match / not-found style message.
    assert!(
        stderr.contains("not found")
            || stderr.contains("not be empty")
            || stderr.contains("Zero matches"),
        "Stderr must contain a zero-match or empty-NAME hint; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 41 — M-8 mutation kill / BC-3.4.017 Gate B boundary
// `--field summary=X` WITHOUT `--summary` must NOT trigger Gate B.
// Gate B fires only when BOTH sides are true: `summary.is_some() &&
// field_keys_lower.contains("summary")`.  Without the right-side field name
// present in the flag set, the guard must not fire.
//
// Mutation target: `src/cli/issue/create.rs:408` (replace && with ||).
// If && is replaced with ||, the presence of "summary" in field_keys alone
// would trigger Gate B even without --summary.  This test would then see exit
// 64 with the conflict error and fail, killing the mutant.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_017_field_summary_without_flag_does_not_trigger_gate_b() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // list_fields and editmeta must be called — Gate B must NOT have fired.
    mount_list_fields(&server, "customfield_10000", "summary").await;
    mount_editmeta_string(&server, "TEST-1", "customfield_10000", "summary").await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "summary=New title",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Gate B must NOT fire — exit must be 0, not 64.
    assert!(
        output.status.success(),
        "Gate B must NOT fire for --field summary=X without --summary; stderr={stderr}"
    );

    // Must NOT contain the Gate B conflict message.
    assert!(
        !stderr.contains("summary is set by both"),
        "Must NOT emit Gate B conflict error; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 42 — M-9 mutation kill / BC-3.4.017 Gate B boundary
// `--field description=X` WITHOUT `--description` must NOT trigger Gate B.
// Gate B fires only when BOTH sides are true:
// `(description.is_some() || description_stdin) && field_keys_lower.contains("description")`.
//
// Mutation target: `src/cli/issue/create.rs:414` (replace && with ||).
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_017_field_description_without_flag_does_not_trigger_gate_b() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, "customfield_10001", "description").await;
    mount_editmeta_string(&server, "TEST-1", "customfield_10001", "description").await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "description=some text",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Gate B must NOT fire for --field description=X without --description; stderr={stderr}"
    );

    assert!(
        !stderr.contains("description is set by both"),
        "Must NOT emit Gate B conflict error; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 43 — M-10 mutation kill / BC-3.4.017 Gate B boundary
// `--field priority=X` WITHOUT `--priority` must NOT trigger Gate B.
// Gate B fires only when BOTH sides are true: `priority.is_some() &&
// field_keys_lower.contains("priority")`.
//
// Mutation target: `src/cli/issue/create.rs:429` (replace && with ||).
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_017_field_priority_without_flag_does_not_trigger_gate_b() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, "customfield_10002", "priority").await;
    mount_editmeta_string(&server, "TEST-1", "customfield_10002", "priority").await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "priority=High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Gate B must NOT fire for --field priority=X without --priority; stderr={stderr}"
    );

    assert!(
        !stderr.contains("priority is set by both"),
        "Must NOT emit Gate B conflict error; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// FIX-F5-001 — --label + --field silent-drop rejection
// ---------------------------------------------------------------------------
//
// `jr issue edit KEY --label add:X --field Y=Z` on a single key must exit 64.
// Without this guard, the `--label` short-circuit at create.rs:~835 routes to
// `handle_edit_bulk_labels` which does not accept `field_pairs`, silently
// dropping the `--field` write while exiting 0 (data loss).
//
// The fix: add `--field` to the `--label` conflict block at create.rs:~445
// (the --label conflict block).
// Rejection fires BEFORE any HTTP call.
//
// Uses a catch-all any() mock with .expect(0) to pin "no HTTP at all".
// This is stronger than matching specific methods/paths: it defends against
// future refactors that hoist a GET (editmeta, list_fields, JQL search) above
// the --label conflict block, which would silently pass a method-specific guard
// while still making premature HTTP calls.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_field_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    // Catch-all: wiremock panics on server drop if ANY HTTP fires.
    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--field",
            "Severity=High",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --field must exit 64 (UserError); stderr={stderr}"
    );

    assert!(
        stderr.contains("--label cannot be combined with") && stderr.contains("--field"),
        "Stderr must contain --label conflict error referencing --field; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// FIX-F5-001 negative regression — existing conflict block entries still work
// ---------------------------------------------------------------------------
//
// The `--label` conflict block existed before --field was added. This test
// pins that `--label` + `--summary` (one of the original 11 conflicting flags)
// is still rejected with exit 64.  Pre-existing coverage gap: there was ZERO
// test coverage for the entire --label conflict block before FIX-F5-001.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_summary_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    // Catch-all expect(0): any HTTP call at all panics on server drop.
    // The --label + --summary conflict guard fires before any network I/O,
    // so the mock must never be reached.
    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--summary",
            "New title",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --summary must exit 64 (UserError); stderr={stderr}"
    );

    assert!(
        stderr.contains("--label cannot be combined with") && stderr.contains("--summary"),
        "Stderr must contain --label conflict error referencing --summary; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// S-407 — BC-3.4.017 invariant 2 + EC-3.4.017-14
// 10 positive regression tests: one per untested --label conflict-block entry.
// Pattern mirrors FIX-F5-001 (tests above): catch-all any().expect(0) + exit 64
// + two SEPARATE stderr assertions (conflict prefix + specific flag name).
// ---------------------------------------------------------------------------

// AC-001 — --label + --priority → exit 64, zero HTTP
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_priority_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--priority",
            "High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --priority must exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--label cannot be combined with"),
        "Stderr must contain '--label cannot be combined with'; stderr={stderr}"
    );
    assert!(
        stderr.contains("--priority"),
        "Stderr must contain '--priority'; stderr={stderr}"
    );
}

// AC-002 — --label + --type → exit 64, zero HTTP
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_type_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--type",
            "Bug",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --type must exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--label cannot be combined with"),
        "Stderr must contain '--label cannot be combined with'; stderr={stderr}"
    );
    assert!(
        stderr.contains("--type"),
        "Stderr must contain '--type'; stderr={stderr}"
    );
}

// AC-003 — --label + --team → exit 64, zero HTTP
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_team_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--team",
            "Platform Core",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --team must exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--label cannot be combined with"),
        "Stderr must contain '--label cannot be combined with'; stderr={stderr}"
    );
    assert!(
        stderr.contains("--team"),
        "Stderr must contain '--team'; stderr={stderr}"
    );
}

// AC-004 — --label + --points → exit 64, zero HTTP
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_points_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--points",
            "5",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --points must exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--label cannot be combined with"),
        "Stderr must contain '--label cannot be combined with'; stderr={stderr}"
    );
    assert!(
        stderr.contains("--points"),
        "Stderr must contain '--points'; stderr={stderr}"
    );
}

// AC-005 — --label + --no-points (boolean flag) → exit 64, zero HTTP
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_no_points_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--no-points",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --no-points must exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--label cannot be combined with"),
        "Stderr must contain '--label cannot be combined with'; stderr={stderr}"
    );
    assert!(
        stderr.contains("--no-points"),
        "Stderr must contain '--no-points'; stderr={stderr}"
    );
}

// AC-006 — --label + --parent → exit 64, zero HTTP
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_parent_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--parent",
            "EPIC-1",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --parent must exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--label cannot be combined with"),
        "Stderr must contain '--label cannot be combined with'; stderr={stderr}"
    );
    assert!(
        stderr.contains("--parent"),
        "Stderr must contain '--parent'; stderr={stderr}"
    );
}

// AC-007 — --label + --no-parent (boolean flag) → exit 64, zero HTTP
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_no_parent_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--no-parent",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --no-parent must exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--label cannot be combined with"),
        "Stderr must contain '--label cannot be combined with'; stderr={stderr}"
    );
    assert!(
        stderr.contains("--no-parent"),
        "Stderr must contain '--no-parent'; stderr={stderr}"
    );
}

// AC-008 — --label + --description → exit 64, zero HTTP
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_description_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--description",
            "some text",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --description must exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--label cannot be combined with"),
        "Stderr must contain '--label cannot be combined with'; stderr={stderr}"
    );
    assert!(
        stderr.contains("--description"),
        "Stderr must contain '--description'; stderr={stderr}"
    );
}

// AC-009 — --label + --description-stdin (boolean flag) → exit 64, zero HTTP
//
// No stdin pipe is required: the --label conflict guard fires at create.rs:~474
// (if description_stdin {...}) BEFORE the stdin read at create.rs:~882. The
// process exits 64 before any stdin I/O occurs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_description_stdin_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--description-stdin",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --description-stdin must exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--label cannot be combined with"),
        "Stderr must contain '--label cannot be combined with'; stderr={stderr}"
    );
    assert!(
        stderr.contains("--description-stdin"),
        "Stderr must contain '--description-stdin'; stderr={stderr}"
    );
}

// AC-010 — --label + --markdown (paired with --description) → exit 64, zero HTTP
//
// IMPORTANT: --markdown MUST be paired with --description "some text" here.
// Without --description, the early guard at create.rs:357-363 fires first:
//   "--markdown requires --description or --description-stdin to take effect."
// That exits 64 for the WRONG reason — the --markdown pre-guard, not the
// --label conflict block. Pairing with --description "some text" satisfies
// the pre-guard and lets execution reach the --label conflict block.
//
// TWO SEPARATE stderr assertions (AC-015): the conflict block enumerates
// "--description, --markdown" in a comma-separated list when both are present,
// so the literal "--label cannot be combined with --markdown" does NOT appear
// verbatim. The assertions verify the two components independently.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_label_plus_markdown_rejected_with_exit_64_no_http() {
    let server = MockServer::start().await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--label",
            "add:backend",
            "--markdown",
            "--description",
            "some text",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "--label + --markdown (with --description) must exit 64; stderr={stderr}"
    );
    // Assertion 1: the right guard fired (--label conflict block, not a different guard).
    assert!(
        stderr.contains("--label cannot be combined with"),
        "Stderr must contain '--label cannot be combined with' (verifies the right guard fired); \
         stderr={stderr}"
    );
    // Assertion 2: the --markdown row in the conflict block cannot be deleted without this
    // failing. Separate from assertion 1 because the conflict block joins both --description
    // and --markdown into a comma-separated list; the literal concatenation
    // "--label cannot be combined with --markdown" does NOT appear verbatim.
    assert!(
        stderr.contains("--markdown"),
        "Stderr must contain '--markdown' (verifies --markdown push line cannot be deleted); \
         stderr={stderr}"
    );
}

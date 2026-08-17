//! CLI-level integration tests for `jr component` commands (S-604-1 + S-604-2).
//!
//! S-604-1 tests (handle_list): all PASS — fully implemented.
//! S-604-2 tests (handle_create, handle_edit): all PASS — fully implemented.
//!
//! BC anchors: BC-8.1.001–BC-8.1.008, BC-8.4.002–BC-8.4.004
//! Stories: S-604-1 (list), S-604-2 (create/edit)

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::time::Duration;
use tempfile::TempDir;
use wiremock::matchers::{
    body_json, body_partial_json, method, path, query_param, query_param_is_missing,
};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::fixtures::{
    component_create_response, component_delete_snapshot_page, component_edit_response,
    component_list_response, component_list_two_same_name, component_response,
    component_response_no_project_field, component_response_with_flags,
    multi_project_user_search_response, multi_project_user_search_response_with_email,
    related_issue_counts_response, write_profile_config,
};

// ── Harness ──────────────────────────────────────────────────────────────────

fn jr_cmd(server_uri: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_CACHE_DIR", cache_dir)
        .env("JR_CONFIG_DIR", config_dir);
    cmd
}

// ── AC-001 (BC-8.1.001 — table columns and dash for absent fields) ───────────

/// AC-001 / BC-8.1.001: table has columns ID, Name, Description, Lead,
/// Assignee Type; null description and lead render as "-".
#[tokio::test]
async fn test_bc_8_1_001_component_list_table_columns_and_dash_for_absent() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "FOO"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Table headers
    assert!(
        stdout.contains("ID"),
        "Expected 'ID' column header in output: {stdout}"
    );
    assert!(
        stdout.contains("Name"),
        "Expected 'Name' column header in output: {stdout}"
    );
    assert!(
        stdout.contains("Description"),
        "Expected 'Description' column header in output: {stdout}"
    );
    assert!(
        stdout.contains("Lead"),
        "Expected 'Lead' column header in output: {stdout}"
    );
    assert!(
        stdout.contains("Assignee Type"),
        "Expected 'Assignee Type' column header in output: {stdout}"
    );

    // Component data
    assert!(
        stdout.contains("10001"),
        "Expected component id '10001' in output: {stdout}"
    );
    assert!(
        stdout.contains("Backend"),
        "Expected component name 'Backend' in output: {stdout}"
    );
    // null description and lead render as "-"
    assert!(
        stdout.contains('-'),
        "Expected '-' placeholder for null fields in output: {stdout}"
    );
}

// ── AC-002 (BC-8.1.001 — empty project exits zero) ───────────────────────────

/// AC-002 / BC-8.1.001 EC-8.1.001-1: zero components → exit 0, empty table.
#[tokio::test]
async fn test_bc_8_1_001_component_list_empty_project_exits_zero() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(component_list_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "FOO"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0 for empty component list; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-003 (BC-8.1.001 — config fallback from .jr.toml) ─────────────────────

/// AC-003 / BC-8.1.001: no --project flag; project resolved from .jr.toml in CWD.
#[tokio::test]
async fn test_bc_8_1_001_component_list_falls_back_to_configured_project() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Write .jr.toml in the temp working directory
    std::fs::write(cwd.path().join(".jr.toml"), "project = \"FOO\"\n").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(component_list_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        // no --project flag — project must come from .jr.toml
        .args(["component", "list"])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0 when project resolved from .jr.toml; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-004 (BC-8.1.004 — no project, no config → exit 64 before HTTP) ────────

/// AC-004 / BC-8.1.004: no --project and no configured project → exit 64 before
/// any HTTP call; stderr names --project.
#[tokio::test]
async fn test_bc_8_1_004_component_list_no_project_no_config_exits_64() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // No component endpoint mock — this must never be called (BC-8.1.004 exits before HTTP)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(0)
        .mount(&server)
        .await;

    let cwd = TempDir::new().unwrap();

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list"])
        .current_dir(cwd.path()) // no .jr.toml here
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 when no project and no config; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--project"),
        "Expected '--project' mentioned in stderr; got: {stderr}"
    );
}

// ── AC-005 (BC-8.1.002 — JSON output full object array) ─────────────────────

/// AC-005 / BC-8.1.002: --output json returns full component array on stdout.
#[tokio::test]
async fn test_bc_8_1_002_component_list_json_full_object_array() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response(
                    "10001",
                    "Backend",
                    Some("Backend services"),
                    Some("Alice"),
                    Some("PROJECT_LEAD"),
                ),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "FOO", "--output", "json"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    let arr = parsed.as_array().expect("JSON output must be an array");
    assert_eq!(arr.len(), 1, "Expected 1 component in JSON output");
    let c = &arr[0];
    assert_eq!(c["id"], "10001");
    assert_eq!(c["name"], "Backend");
}

// ── AC-006 (BC-8.1.002 — empty JSON array) ───────────────────────────────────

/// AC-006 / BC-8.1.002: --output json on zero-component project → [] on stdout.
#[tokio::test]
async fn test_bc_8_1_002_component_list_json_empty_array() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(component_list_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "FOO", "--output", "json"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    let arr = parsed.as_array().expect("JSON output must be an array");
    assert!(arr.is_empty(), "Expected empty JSON array; got {arr:?}");
}

// ── AC-007 (BC-8.1.003 — counts enrichment, N+1 pattern) ────────────────────

/// AC-007 / BC-8.1.003: --counts issues exactly one relatedIssueCounts GET per
/// component returned by the list call.
#[tokio::test]
async fn test_bc_8_1_003_component_list_counts_issues_one_get_per_component() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
                component_response("10002", "Frontend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001/relatedIssueCounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(related_issue_counts_response(7)))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10002/relatedIssueCounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(related_issue_counts_response(3)))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "FOO", "--counts"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Issues"),
        "Expected 'Issues' column in --counts output: {stdout}"
    );
}

// ── AC-008 (BC-8.1.003 — zero extra calls on empty project) ─────────────────

/// AC-008 / BC-8.1.003 EC-8.1.003-1: --counts on zero-component project issues
/// zero relatedIssueCounts calls beyond the initial list GET.
#[tokio::test]
async fn test_bc_8_1_003_component_list_counts_noop_on_empty_project() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(component_list_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    // relatedIssueCounts must never be called when component list is empty
    Mock::given(method("GET"))
        .and(wiremock::matchers::path_regex(
            r"/rest/api/3/component/.*/relatedIssueCounts",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "FOO", "--counts"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0 for empty project with --counts; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-009 (BC-8.1.003 — fail-soft on one 5xx) ──────────────────────────────

/// AC-009 / BC-8.1.003 EC-8.1.003-2: one component's relatedIssueCounts returns
/// 5xx → that row shows '?' (table) or issueCount: null (JSON), stderr warning
/// names the component, exit 0, other component counts still render.
#[tokio::test]
async fn test_bc_8_1_003_component_list_counts_fail_soft_on_one_5xx() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
                component_response("10002", "Frontend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Backend succeeds
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001/relatedIssueCounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(related_issue_counts_response(5)))
        .expect(1)
        .mount(&server)
        .await;

    // Frontend fails with 5xx
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10002/relatedIssueCounts"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({"error": "internal"})))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "FOO", "--counts"])
        .output()
        .unwrap();

    server.verify().await;
    // exit 0 despite one 5xx (fail-soft)
    assert!(
        output.status.success(),
        "Expected exit 0 even with one 5xx; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    // '?' in table for failed row
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains('?'),
        "Expected '?' for failed count in table output: {stdout}"
    );
    // Warning on stderr naming the failed component
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Frontend") || stderr.contains("10002"),
        "Expected stderr warning naming the failed component; got: {stderr}"
    );
}

// ── AC-007-JSON (BC-8.1.003 — --counts --output json has issueCount field) ───

/// AC-007-JSON / BC-8.1.003: `--counts --output json` adds an integer
/// `issueCount` field (named exactly `issueCount`, per BC-8.1.003) to each
/// component object in the JSON array.  The value must match the count
/// returned by the `relatedIssueCounts` endpoint for that component.
#[tokio::test]
async fn test_bc_8_1_003_component_list_counts_json_has_issue_count_field() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
                component_response("10002", "Frontend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001/relatedIssueCounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(related_issue_counts_response(7)))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10002/relatedIssueCounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(related_issue_counts_response(3)))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "list",
            "--project",
            "FOO",
            "--counts",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    let arr = parsed.as_array().expect("JSON output must be an array");
    assert_eq!(arr.len(), 2, "Expected 2 components in JSON output");

    // Both components must have an integer `issueCount` field (BC-8.1.003).
    // The field must be named exactly `issueCount` — NOT `relatedIssueCounts`.
    for comp in arr {
        let obj = comp
            .as_object()
            .expect("each component must be a JSON object");
        assert!(
            obj.contains_key("issueCount"),
            "Each component JSON object must contain `issueCount` key (BC-8.1.003); \
             got keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            obj["issueCount"].is_u64() || obj["issueCount"].is_i64(),
            "issueCount must be an integer; got: {:?}",
            obj["issueCount"]
        );
    }

    // Spot-check the actual values against the fixture counts.
    let backend = arr
        .iter()
        .find(|c| c["id"] == "10001")
        .expect("component 10001 must be in output");
    assert_eq!(
        backend["issueCount"], 7,
        "Backend issueCount must be 7 per fixture"
    );

    let frontend = arr
        .iter()
        .find(|c| c["id"] == "10002")
        .expect("component 10002 must be in output");
    assert_eq!(
        frontend["issueCount"], 3,
        "Frontend issueCount must be 3 per fixture"
    );
}

// ── AC-009-JSON (BC-8.1.003 — fail-soft: failing component gets null issueCount) ──

/// AC-009-JSON / BC-8.1.003 EC-8.1.003-2 (JSON path): when one component's
/// `relatedIssueCounts` call fails with 5xx, its JSON object has `issueCount`
/// present as JSON `null` (key MUST be present — not omitted) while the
/// succeeding component has an integer `issueCount`.  Exit 0 (fail-soft).
/// Stderr warning names the failing component.
#[tokio::test]
async fn test_bc_8_1_003_component_list_counts_fail_soft_json_null_for_failed() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
                component_response("10002", "Frontend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Backend succeeds
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001/relatedIssueCounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(related_issue_counts_response(5)))
        .expect(1)
        .mount(&server)
        .await;

    // Frontend fails with 5xx
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10002/relatedIssueCounts"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({"error": "internal"})))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "list",
            "--project",
            "FOO",
            "--counts",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    server.verify().await;

    // Exit 0 despite one 5xx (fail-soft per BC-8.1.003 EC-8.1.003-2).
    assert!(
        output.status.success(),
        "Expected exit 0 (fail-soft) even with one 5xx; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    // Stderr must name the failing component (BC-8.1.003).
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Frontend") || stderr.contains("10002"),
        "Expected stderr warning naming the failed component; got: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    let arr = parsed.as_array().expect("JSON output must be an array");
    assert_eq!(arr.len(), 2, "Expected 2 components in JSON output");

    // Succeeding component (Backend/10001): integer issueCount.
    let backend = arr
        .iter()
        .find(|c| c["id"] == "10001")
        .expect("component 10001 must be present");
    let backend_obj = backend.as_object().expect("component must be an object");
    assert!(
        backend_obj.contains_key("issueCount"),
        "Backend must have issueCount key; keys: {:?}",
        backend_obj.keys().collect::<Vec<_>>()
    );
    assert!(
        backend["issueCount"].is_u64() || backend["issueCount"].is_i64(),
        "Backend issueCount must be an integer; got: {:?}",
        backend["issueCount"]
    );

    // Failing component (Frontend/10002): issueCount key PRESENT but JSON null
    // (BC-8.1.003: "issueCount: null" in JSON mode, NOT key omission).
    let frontend = arr
        .iter()
        .find(|c| c["id"] == "10002")
        .expect("component 10002 must be present");
    let frontend_obj = frontend.as_object().expect("component must be an object");
    assert!(
        frontend_obj.contains_key("issueCount"),
        "Frontend must have issueCount key present (even on failure — BC-8.1.003 \
         requires null, not omission); keys: {:?}",
        frontend_obj.keys().collect::<Vec<_>>()
    );
    assert!(
        frontend["issueCount"].is_null(),
        "Frontend issueCount must be JSON null on 5xx failure (BC-8.1.003); got: {:?}",
        frontend["issueCount"]
    );
}

// ── AC-005-NULL (BC-8.1.002 — null fields PRESENT in JSON, not dropped) ──────

/// AC-005-NULL / BC-8.1.002: `--output json` must include ALL fields the API
/// returned, even when their value is null — "no field is dropped for JSON
/// mode".  A component with null description, lead, assigneeType, and project
/// must still appear in the JSON object with those keys explicitly present as
/// JSON null (not omitted).
#[tokio::test]
async fn test_bc_8_1_002_component_list_json_null_fields_present_not_dropped() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Component with null description, lead, assigneeType (fixture also sets project: null).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                // All optional fields null — exercises BC-8.1.002 null-preservation contract.
                component_response("10099", "Infra", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "FOO", "--output", "json"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    let arr = parsed.as_array().expect("JSON output must be an array");
    assert_eq!(arr.len(), 1, "Expected 1 component");

    let comp = &arr[0];
    let obj = comp.as_object().expect("component must be a JSON object");

    // id and name must always be present.
    assert_eq!(comp["id"], "10099");
    assert_eq!(comp["name"], "Infra");

    // Null-valued fields MUST be present as JSON null, NOT omitted
    // (BC-8.1.002: "no field is dropped for JSON mode").
    let null_fields = ["description", "lead", "assigneeType", "project"];
    for field in &null_fields {
        assert!(
            obj.contains_key(*field),
            "Field `{field}` must be present in JSON output even when null \
             (BC-8.1.002 — no field dropped); keys present: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            comp[field].is_null(),
            "Field `{field}` must be JSON null (not some other value); got: {:?}",
            comp[field]
        );
    }
}

// ── F-B2 (adversarial pass-3 / LOW — counts JSON must be superset of plain JSON) ──

/// F-B2 (adversarial pass-3 / LOW) — BC-8.1.003 is ADDITIVE over BC-8.1.002:
/// `--counts --output json` must be a STRICT SUPERSET of plain `--output json`
/// (same fields + `issueCount`).  A component with `isAssigneeTypeValid: true`
/// must have that field present in BOTH plain and counts JSON output.
///
/// Part (a): plain `--output json` MUST contain `isAssigneeTypeValid` — passes
/// because `Component` serializes it when `Some(...)`.
///
/// Part (b): `--counts --output json` MUST ALSO contain `isAssigneeTypeValid` —
/// passes because the F-B2 fix (pass 3) removed `ComponentCountJson` entirely;
/// counts JSON is now produced by serializing the full `Component` to
/// `serde_json::Value`, then swapping `relatedIssueCount`→`issueCount`, so
/// `isAssigneeTypeValid` (and all BC-8.1.002 fields) are preserved.
#[tokio::test]
async fn test_bc_8_1_003_counts_json_is_superset_of_plain_json_fields() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Component with isAssigneeTypeValid: true — exercises the superset invariant.
    let fixture = component_response_with_flags(
        "10001",
        "Backend",
        None,
        None,
        Some("PROJECT_LEAD"),
        None,
        Some(true),
    );

    // Single list mock responds to both requests (plain + counts).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![fixture])),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001/relatedIssueCounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(related_issue_counts_response(4)))
        .mount(&server)
        .await;

    // ── Part (a): plain --output json must include isAssigneeTypeValid ──────
    let plain_output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "FOO", "--output", "json"])
        .output()
        .unwrap();

    assert!(
        plain_output.status.success(),
        "Part (a): Expected exit 0 for plain json; got {:?}\nstderr: {}",
        plain_output.status.code(),
        String::from_utf8_lossy(&plain_output.stderr)
    );

    let plain_stdout = String::from_utf8_lossy(&plain_output.stdout);
    let plain_parsed: serde_json::Value =
        serde_json::from_str(&plain_stdout).expect("plain stdout must be valid JSON");
    let plain_arr = plain_parsed
        .as_array()
        .expect("plain JSON must be an array");
    let plain_comp = &plain_arr[0];
    let plain_obj = plain_comp
        .as_object()
        .expect("component must be a JSON object");

    assert!(
        plain_obj.contains_key("isAssigneeTypeValid"),
        "Part (a): plain --output json must include isAssigneeTypeValid when it is \
         Some(true) (Component serializes it via skip_serializing_if = Option::is_none); \
         keys present: {:?}",
        plain_obj.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        plain_comp["isAssigneeTypeValid"],
        serde_json::Value::Bool(true),
        "Part (a): isAssigneeTypeValid must be true"
    );

    // ── Part (b): --counts --output json must ALSO include isAssigneeTypeValid ──
    let counts_output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "list",
            "--project",
            "FOO",
            "--counts",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        counts_output.status.success(),
        "Part (b): Expected exit 0 for counts json; got {:?}\nstderr: {}",
        counts_output.status.code(),
        String::from_utf8_lossy(&counts_output.stderr)
    );

    let counts_stdout = String::from_utf8_lossy(&counts_output.stdout);
    let counts_parsed: serde_json::Value =
        serde_json::from_str(&counts_stdout).expect("counts stdout must be valid JSON");
    let counts_arr = counts_parsed
        .as_array()
        .expect("counts JSON must be an array");
    let counts_comp = &counts_arr[0];
    let counts_obj = counts_comp
        .as_object()
        .expect("component must be a JSON object");

    // issueCount must be present (BC-8.1.003 additive field).
    assert!(
        counts_obj.contains_key("issueCount"),
        "Part (b): --counts --output json must include issueCount (BC-8.1.003); \
         keys: {:?}",
        counts_obj.keys().collect::<Vec<_>>()
    );

    // isAssigneeTypeValid MUST be present — BC-8.1.003 is additive over
    // BC-8.1.002; counts JSON must be a superset, not a subset.
    // Passes: counts JSON is built by serializing the full `Component` to
    // `serde_json::Value` then swapping `relatedIssueCount`→`issueCount`
    // (F-B2, pass 3), so `isAssigneeTypeValid` is preserved.
    assert!(
        counts_obj.contains_key("isAssigneeTypeValid"),
        "Part (b): --counts --output json must include isAssigneeTypeValid \
         (BC-8.1.003 is a strict superset of BC-8.1.002 — same fields + issueCount); \
         keys present: {:?}",
        counts_obj.keys().collect::<Vec<_>>()
    );
}

// ── F-B3 (coverage gap / keep green — populated project round-trips) ─────────

/// F-B3 (adversarial pass-3 / coverage gap, keep green): the existing
/// `component_response` fixture hard-codes `"project": null`, so the
/// populated-project JSON/table path is never exercised.  This test uses a
/// fixture with `"project": "FOO"` (a string) and asserts that the value
/// round-trips through deserialization and re-serialization intact.
///
/// `Component.project` is `Option<String>` without `skip_serializing_if`, so
/// `None` → `null` and `Some("FOO")` → `"FOO"` in JSON output — both are
/// valid and this test must stay GREEN.
#[tokio::test]
async fn test_bc_8_1_002_component_list_json_populated_project_round_trips() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Component with populated project field (not null).
    let fixture = component_response_with_flags(
        "10050",
        "Infra",
        Some("Infrastructure services"),
        None,
        Some("UNASSIGNED"),
        Some("FOO"), // populated project — the gap being closed
        None,
    );

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![fixture])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "FOO", "--output", "json"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    let arr = parsed.as_array().expect("JSON output must be an array");
    assert_eq!(arr.len(), 1, "Expected 1 component");

    let comp = &arr[0];
    let obj = comp.as_object().expect("component must be a JSON object");

    // project field must be present and equal to the string "FOO" (not null).
    assert!(
        obj.contains_key("project"),
        "project key must be present in JSON output; keys: {:?}",
        obj.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        comp["project"],
        serde_json::Value::String("FOO".to_string()),
        "Populated project value must round-trip as the string \"FOO\" \
         (Component.project: Option<String> without skip_serializing_if)"
    );

    // Sanity-check that other fields also round-trip correctly.
    assert_eq!(comp["id"], "10050");
    assert_eq!(comp["name"], "Infra");
    assert_eq!(comp["description"], "Infrastructure services");
}

// ── VP-COMPONENT-001 negative half (plain list issues ZERO relatedIssueCounts GETs) ──

/// VP-COMPONENT-001 negative half: a plain `jr component list` (without
/// `--counts`) must issue ZERO calls to the relatedIssueCounts endpoint.
///
/// Without this pin, a mutation that removes the `if counts` guard in
/// `src/cli/component.rs` would survive all existing tests: the enrichment
/// loop would run on the plain-list path, the unmounted endpoint would 404,
/// the fail-soft arm would swallow the error, and the command would still
/// exit 0 with a table rendered.  This `.expect(0)` closes that gap —
/// wiremock enforces zero calls at `server.verify()` time.
///
/// Paired with AC-007 (the `.expect(N)` positive half already pinned).
#[tokio::test]
async fn test_vp_component_001_plain_list_issues_zero_related_issue_counts_gets() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Return ≥1 component so the enrichment loop would have something to
    // iterate over if the guard were absent.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
                component_response("10002", "Frontend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // The relatedIssueCounts endpoint MUST NEVER be called on a plain list
    // (no --counts flag).  .expect(0) is the enforcement pin for VP-COMPONENT-001.
    Mock::given(method("GET"))
        .and(wiremock::matchers::path_regex(
            r"/rest/api/3/component/.*/relatedIssueCounts",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "FOO"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0 for plain list; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-012 (BC-8.4.004 — resolver never spans projects) ─────────────────────

/// AC-012 / BC-8.4.004: listing PRJA components never triggers PRJB's
/// component-list endpoint (project-scoped isolation invariant).
#[tokio::test]
async fn test_bc_8_4_004_resolve_component_never_spans_projects() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PRJA/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PRJB's component list MUST NEVER be called
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PRJB/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("20001", "Backend", None, None, None),
            ])),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "list", "--project", "PRJA"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0 for PRJA list; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// S-604-2: component create / edit tests
// Handlers are fully implemented; all tests below exercise the live handler.
// ══════════════════════════════════════════════════════════════════════════════

// ── AC-001 (BC-8.1.005 — minimal create body via body_json matcher) ───────────

/// AC-001 / BC-8.1.005: `jr component create --project FOO Backend` (no
/// optional flags) POSTs exactly `{"name":"Backend","project":"FOO"}`.
/// Verified via wiremock `body_json` matcher — absent optional keys must NOT
/// appear in the body (VP-COMPONENT-022, omit-if-absent invariant).
#[tokio::test]
async fn test_bc_8_1_005_component_create_minimal_body() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // The exact POST body must equal {"name":"Backend","project":"FOO"} — no
    // extra keys like "description":null.  body_json enforces equality.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .and(body_json(json!({"name": "Backend", "project": "FOO"})))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(component_create_response("10001", "Backend", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // No assignable-users call must fire when --lead is absent.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "create", "--project", "FOO", "Backend"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-002 (BC-8.1.005 — all optional fields present in POST body) ───────────

/// AC-002 / BC-8.1.005: when all optional flags are supplied, the POST body
/// contains all of them: description, leadAccountId (resolved accountId),
/// assigneeType (API string from clap ValueEnum mapping).
/// `--assignee-type` takes SCREAMING_SNAKE values (AC-002 literal: PROJECT_LEAD /
/// COMPONENT_LEAD / UNASSIGNED / PROJECT_DEFAULT) per story S-604-2 Behavior
/// Summary and BC-8.1.005.  Kebab-case variants (project-lead, etc.) are INVALID.
#[tokio::test]
async fn test_bc_8_1_005_component_create_all_optional_fields_present() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Lead resolver: "Alice" resolves to accountId "acc-alice".
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(multi_project_user_search_response(vec![(
                "acc-alice",
                "Alice",
            )])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Full POST body including all optional fields.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .and(body_json(json!({
            "name": "Backend",
            "project": "FOO",
            "description": "Backend services",
            "leadAccountId": "acc-alice",
            "assigneeType": "COMPONENT_LEAD"
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(component_create_response("10001", "Backend", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "create",
            "--project",
            "FOO",
            "--description",
            "Backend services",
            "--lead",
            "Alice",
            "--assignee-type",
            "COMPONENT_LEAD",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-003 (BC-8.1.005 — absent optional keys OMITTED from POST body) ────────

/// AC-003 / BC-8.1.005 VP-COMPONENT-022: POST body must omit absent optional
/// keys (never send `"description":null`).  Verified by `body_json` with exact
/// JSON — if the implementation sends extra null-valued keys, the matcher will
/// reject the request, `.expect(1)` will fail at `server.verify()`.
#[tokio::test]
async fn test_bc_8_1_005_component_create_omits_absent_optional_keys() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Only name and description supplied — leadAccountId and assigneeType absent.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .and(body_json(json!({
            "name": "API",
            "project": "FOO",
            "description": "API gateway"
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(component_create_response("10002", "API", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "create",
            "--project",
            "FOO",
            "--description",
            "API gateway",
            "API",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-004 (BC-8.1.005 — success output: JSON stdout + stderr table line) ────

/// AC-004 / BC-8.1.005: on 201 success, JSON mode (`--output json`) returns
/// `{"id","name","project"}` on stdout; human mode writes a confirmation line
/// to stderr (symmetric output channel profile 4).
#[tokio::test]
async fn test_bc_8_1_005_component_create_success_output_both_modes() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(component_create_response("10001", "Backend", "FOO")),
        )
        .mount(&server)
        .await;

    // ── Part A: --output json ──────────────────────────────────────────────
    let json_output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "create",
            "--project",
            "FOO",
            "--output",
            "json",
            "Backend",
        ])
        .output()
        .unwrap();

    assert!(
        json_output.status.success(),
        "Part A: expected exit 0 with --output json; got {:?}\nstderr: {}",
        json_output.status.code(),
        String::from_utf8_lossy(&json_output.stderr)
    );
    let stdout = String::from_utf8_lossy(&json_output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("--output json stdout must be valid JSON");
    let obj = parsed.as_object().expect("JSON output must be an object");
    // F-04: key-set equality — exactly {"id","name","project"}, no extras
    {
        let actual_keys: std::collections::BTreeSet<&str> =
            obj.keys().map(|s| s.as_str()).collect();
        let expected_keys: std::collections::BTreeSet<&str> =
            ["id", "name", "project"].iter().copied().collect();
        assert_eq!(
            actual_keys,
            expected_keys,
            "AC-004 F-04: --output json must return EXACTLY {{\"id\",\"name\",\"project\"}} \
             (BC-8.1.005 §JSON); no extra keys like description/lead/assigneeType; \
             got keys: {:?}",
            obj.keys().collect::<Vec<_>>(),
        );
    }
    assert_eq!(parsed["id"], "10001");
    assert_eq!(parsed["name"], "Backend");
    assert_eq!(parsed["project"], "FOO");

    // ── Part B: human output → confirmation to stderr ─────────────────────
    let human_output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "create", "--project", "FOO", "Backend"])
        .output()
        .unwrap();

    assert!(
        human_output.status.success(),
        "Part B: expected exit 0 for human mode; got {:?}\nstderr: {}",
        human_output.status.code(),
        String::from_utf8_lossy(&human_output.stderr)
    );
    let stderr = String::from_utf8_lossy(&human_output.stderr);
    // F-05: exact BC-8.1.005 confirmation line
    assert!(
        stderr.contains("Created component \"Backend\" (id 10001) in project FOO."),
        "AC-004 F-05: stderr must contain exact BC-8.1.005 confirmation line \
         'Created component \"Backend\" (id 10001) in project FOO.'; got: {stderr}"
    );
}

// ── AC-005 (BC-8.1.005 EC-8.1.005-2 — bad assignee-type → clap exit 2) ──────

/// AC-005 / BC-8.1.005 EC-8.1.005-2: `--assignee-type BOGUS` triggers a clap
/// ValueEnum parse failure (exit 2) BEFORE the handler runs.
///
/// This test passes before and after handler implementation because clap
/// validates the enum at parse time — the handler is never reached for an
/// invalid value.  Included as a compile + regression guard.
#[tokio::test]
async fn test_bc_8_1_005_component_create_bad_assignee_type_exits_2() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // No HTTP calls expected — clap fails before any network access.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "create",
            "--project",
            "FOO",
            "--assignee-type",
            "BOGUS",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(2),
        "Expected clap exit 2 for bad assignee-type; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-005a (BC-8.1.005 — PROJECT_LEAD accepted, maps to "PROJECT_LEAD" on wire) ──

/// AC-005a / BC-8.1.005 INFO-3: `--assignee-type PROJECT_LEAD` is accepted (not
/// exit 2) and the POST body contains `"assigneeType":"PROJECT_LEAD"`.
/// FAILS against a kebab-case impl because clap rejects PROJECT_LEAD → exit 2 → mock
/// `.expect(1)` never fires and the `output.status.success()` assertion fails.
#[tokio::test]
async fn test_bc_8_1_005_assignee_type_project_lead_accepted_and_wired() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .and(body_json(json!({
            "name": "Backend",
            "project": "FOO",
            "assigneeType": "PROJECT_LEAD"
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(component_create_response("10001", "Backend", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "create",
            "--project",
            "FOO",
            "--assignee-type",
            "PROJECT_LEAD",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "AC-005a: --assignee-type PROJECT_LEAD must be accepted (exit 0); \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-005b (BC-8.1.005 — COMPONENT_LEAD accepted, maps to "COMPONENT_LEAD" on wire) ──

/// AC-005b / BC-8.1.005 INFO-3: `--assignee-type COMPONENT_LEAD` is accepted
/// and the POST body contains `"assigneeType":"COMPONENT_LEAD"`.
#[tokio::test]
async fn test_bc_8_1_005_assignee_type_component_lead_accepted_and_wired() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .and(body_json(json!({
            "name": "Backend",
            "project": "FOO",
            "assigneeType": "COMPONENT_LEAD"
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(component_create_response("10001", "Backend", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "create",
            "--project",
            "FOO",
            "--assignee-type",
            "COMPONENT_LEAD",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "AC-005b: --assignee-type COMPONENT_LEAD must be accepted (exit 0); \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-005c (BC-8.1.005 — UNASSIGNED accepted, maps to "UNASSIGNED" on wire) ────

/// AC-005c / BC-8.1.005 INFO-3: `--assignee-type UNASSIGNED` is accepted
/// and the POST body contains `"assigneeType":"UNASSIGNED"`.
#[tokio::test]
async fn test_bc_8_1_005_assignee_type_unassigned_accepted_and_wired() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .and(body_json(json!({
            "name": "Backend",
            "project": "FOO",
            "assigneeType": "UNASSIGNED"
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(component_create_response("10001", "Backend", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "create",
            "--project",
            "FOO",
            "--assignee-type",
            "UNASSIGNED",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "AC-005c: --assignee-type UNASSIGNED must be accepted (exit 0); \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-005d (BC-8.1.005 — PROJECT_DEFAULT accepted, maps to "PROJECT_DEFAULT" on wire) ──

/// AC-005d / BC-8.1.005 INFO-3: `--assignee-type PROJECT_DEFAULT` is accepted
/// and the POST body contains `"assigneeType":"PROJECT_DEFAULT"`.
#[tokio::test]
async fn test_bc_8_1_005_assignee_type_project_default_accepted_and_wired() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .and(body_json(json!({
            "name": "Backend",
            "project": "FOO",
            "assigneeType": "PROJECT_DEFAULT"
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(component_create_response("10001", "Backend", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "create",
            "--project",
            "FOO",
            "--assignee-type",
            "PROJECT_DEFAULT",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "AC-005d: --assignee-type PROJECT_DEFAULT must be accepted (exit 0); \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-006 (BC-8.1.006 — empty --lead on create → exit 64, zero POST) ────────

/// AC-006 / BC-8.1.006: `--lead ""` on `component create` exits 64 with a
/// descriptive error message (app-level guard, not clap).  Zero POST calls.
/// Message must contain the exact substring
/// `"--lead \"\" has no effect on create"`.
#[tokio::test]
async fn test_bc_8_1_006_component_create_empty_lead_exits_64_zero_post() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Zero POST calls — guard must fire before any HTTP.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
        .expect(0)
        .mount(&server)
        .await;

    // Zero assignable-users calls — lead resolver must not run.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "create",
            "--project",
            "FOO",
            "--lead",
            "",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for empty --lead on create; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    // BC-8.1.006 verbatim wording pin (pass-3 LOW-1): assert the distinctive clause
    // including the em-dash and the trailing "Omit --lead, or supply a name." sentence.
    assert!(
        stderr.contains("has no effect on create \u{2014} there is no existing lead to clear. Omit --lead, or supply a name."),
        "Expected BC-8.1.006 verbatim empty-lead guard message; got: {stderr}"
    );
}

// ── AC-007 (BC-8.1.006 — ambiguous/no-match lead → exit 64, zero POST) ───────

/// AC-007 / BC-8.1.006 VP-COMPONENT-002: when `--lead` resolution returns no
/// matches or multiple matches, the command exits 64 and issues zero POST calls.
#[tokio::test]
async fn test_bc_8_1_006_component_create_lead_ambiguous_and_no_match_zero_post() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // ── Case A: no match ──────────────────────────────────────────────────
    let server_a = MockServer::start().await;
    write_profile_config(config.path(), &server_a.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(multi_project_user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server_a)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
        .expect(0)
        .mount(&server_a)
        .await;

    let no_match = jr_cmd(&server_a.uri(), cache.path(), config.path())
        .args([
            "component",
            "create",
            "--project",
            "FOO",
            "--lead",
            "nonexistent-person",
            "Backend",
        ])
        .output()
        .unwrap();

    server_a.verify().await;
    assert_eq!(
        no_match.status.code(),
        Some(64),
        "Case A: expected exit 64 for no-match lead; got {:?}\nstderr: {}",
        no_match.status.code(),
        String::from_utf8_lossy(&no_match.stderr)
    );
    // F-02: BC-8.1.006 EC-8.1.006-2 exact no-match message
    let stderr_a = String::from_utf8_lossy(&no_match.stderr);
    assert!(
        stderr_a.contains("No user matching 'nonexistent-person'"),
        "AC-007 F-02: Case A stderr must contain BC-8.1.006 EC-8.1.006-2 exact message \
         \"No user matching 'nonexistent-person'\"; got: {stderr_a}"
    );

    // ── Case B: ambiguous match ───────────────────────────────────────────
    let server_b = MockServer::start().await;
    write_profile_config(config.path(), &server_b.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            multi_project_user_search_response_with_email(vec![
                ("acc-001", "Alice Smith", "alice.smith@example.com"),
                ("acc-002", "Alice Jones", "alice.jones@example.com"),
            ]),
        ))
        .expect(1)
        .mount(&server_b)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
        .expect(0)
        .mount(&server_b)
        .await;

    let ambiguous = jr_cmd(&server_b.uri(), cache.path(), config.path())
        .args([
            "component",
            "create",
            "--project",
            "FOO",
            "--lead",
            "alice",
            "Backend",
        ])
        .output()
        .unwrap();

    server_b.verify().await;
    assert_eq!(
        ambiguous.status.code(),
        Some(64),
        "Case B: expected exit 64 for ambiguous lead; got {:?}\nstderr: {}",
        ambiguous.status.code(),
        String::from_utf8_lossy(&ambiguous.stderr)
    );
    // F-02: BC-8.1.006 EC-8.1.006-1 — both candidate emails/accountIds must appear
    let stderr_b = String::from_utf8_lossy(&ambiguous.stderr);
    assert!(
        stderr_b.contains("alice.smith@example.com") || stderr_b.contains("acc-001"),
        "AC-007 F-02: Case B BC-8.1.006 EC-8.1.006-1 ambiguous message must name first \
         candidate (email alice.smith@example.com or accountId acc-001); got: {stderr_b}"
    );
    assert!(
        stderr_b.contains("alice.jones@example.com") || stderr_b.contains("acc-002"),
        "AC-007 F-02: Case B BC-8.1.006 EC-8.1.006-1 ambiguous message must name second \
         candidate (email alice.jones@example.com or accountId acc-002); got: {stderr_b}"
    );
}

// ── AC-008 (BC-8.1.007 — edit PUT contains ONLY supplied fields) ─────────────

/// AC-008 / BC-8.1.007 VP-COMPONENT-023: a partial edit supplying only `--name`
/// sends `{"name":"New Name"}` in the PUT body.  No other keys (description,
/// leadAccountId) may appear.  Enforced via `body_json` exact matching.
#[tokio::test]
async fn test_bc_8_1_007_component_edit_put_contains_only_supplied_fields() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Resolution: name-based via project component list.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Exact PUT body: only name, no description or leadAccountId.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .and(body_json(json!({"name": "New Name"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_edit_response("10001", "New Name", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "FOO",
            "--name",
            "New Name",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── F-01 / F-05-edit (BC-8.1.007 — edit --output json exact shape + field-echo) ─

/// F-01+F-05 / BC-8.1.007: `component edit --output json` returns EXACTLY
/// `{"id","name","project"}` — same 3-key shape as create (BC-8.1.005).
/// The API response contains more fields (description, lead, assigneeType) that
/// the handler MUST project away.
///
/// Part B: table mode emits a header line `Updated component "<name>" (id <id>) in project <key>.`
/// followed by `  name \u{2192} New Name` on stderr (BC-3.4.012 field-echo).
#[tokio::test]
async fn test_bc_8_1_007_component_edit_success_output_json_shape() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Resolution via project component list.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server)
        .await;

    // PUT returns the full 6-key API response; output must project to 3 keys.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_edit_response("10001", "New Name", "FOO")),
        )
        .mount(&server)
        .await;

    // ── Part A: --output json → exactly {"id","name","project"} ──────────────
    let json_output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "FOO",
            "--name",
            "New Name",
            "--output",
            "json",
            "Backend",
        ])
        .output()
        .unwrap();

    assert!(
        json_output.status.success(),
        "F-01 Part A: expected exit 0 with --output json; got {:?}\nstderr: {}",
        json_output.status.code(),
        String::from_utf8_lossy(&json_output.stderr)
    );
    let stdout = String::from_utf8_lossy(&json_output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("F-01: --output json stdout must be valid JSON");
    let obj = parsed
        .as_object()
        .expect("F-01: JSON output must be an object");
    // F-01: key-set equality — exactly {"id","name","project"}, no extras
    {
        let actual_keys: std::collections::BTreeSet<&str> =
            obj.keys().map(|s| s.as_str()).collect();
        let expected_keys: std::collections::BTreeSet<&str> =
            ["id", "name", "project"].iter().copied().collect();
        assert_eq!(
            actual_keys,
            expected_keys,
            "F-01: edit --output json must return EXACTLY {{\"id\",\"name\",\"project\"}} \
             (BC-8.1.007 same shape as create); no extra keys like description/lead/assigneeType; \
             got keys: {:?}",
            obj.keys().collect::<Vec<_>>(),
        );
    }
    assert_eq!(parsed["id"], "10001");
    assert_eq!(parsed["name"], "New Name");
    assert_eq!(parsed["project"], "FOO");

    // ── Part B: table mode → field-echo on stderr (BC-3.4.012) ───────────────
    let server_b = MockServer::start().await;
    write_profile_config(config.path(), &server_b.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_b)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_edit_response("10001", "New Name", "FOO")),
        )
        .mount(&server_b)
        .await;

    let table_output = jr_cmd(&server_b.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "FOO",
            "--name",
            "New Name",
            "Backend",
        ])
        .output()
        .unwrap();

    assert!(
        table_output.status.success(),
        "F-05 edit Part B: expected exit 0 for table mode; got {:?}\nstderr: {}",
        table_output.status.code(),
        String::from_utf8_lossy(&table_output.stderr)
    );
    let stderr_b = String::from_utf8_lossy(&table_output.stderr);
    // F-A2 (PR#704 finding 2): BC-8.1.007 header line must appear before the field echoes.
    // FAILS against impl that emits only field echoes with no header.
    assert!(
        stderr_b.contains("Updated component \"New Name\" (id 10001) in project FOO."),
        "F-A2: BC-8.1.007 edit table mode must emit header line \
         'Updated component \"New Name\" (id 10001) in project FOO.' on stderr; got: {stderr_b}"
    );
    // F-05: BC-3.4.012 field-echo format "  field \u{2192} value"
    assert!(
        stderr_b.contains("  name \u{2192} New Name"),
        "F-05 edit Part B: stderr must contain BC-3.4.012 field-echo \
         '  name \u{2192} New Name'; got: {stderr_b}"
    );
}

// ── AC-009 (BC-8.1.007 — --lead "" clears vs omitting --lead keeps unchanged) ─

/// AC-009 / BC-8.1.007: `--lead ""` sends `{"leadAccountId":null}` (explicit
/// clear); omitting `--lead` means `leadAccountId` is absent from the PUT body
/// (no-op — existing lead unchanged).
#[tokio::test]
async fn test_bc_8_1_007_component_edit_lead_empty_string_clears_vs_omitted() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // ── Case A: --lead "" sends leadAccountId:null ─────────────────────
    let server_a = MockServer::start().await;
    write_profile_config(config.path(), &server_a.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_a)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .and(body_json(json!({"name": "Backend", "leadAccountId": null})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_edit_response("10001", "Backend", "FOO")),
        )
        .expect(1)
        .mount(&server_a)
        .await;

    let clear_output = jr_cmd(&server_a.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "FOO",
            "--name",
            "Backend",
            "--lead",
            "",
            "Backend",
        ])
        .output()
        .unwrap();

    server_a.verify().await;
    assert!(
        clear_output.status.success(),
        "Case A: expected exit 0 for --lead \"\"; got {:?}\nstderr: {}",
        clear_output.status.code(),
        String::from_utf8_lossy(&clear_output.stderr)
    );

    // ── Case B: no --lead flag → leadAccountId absent ─────────────────
    let server_b = MockServer::start().await;
    write_profile_config(config.path(), &server_b.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_b)
        .await;

    // Exact body: only name, no leadAccountId key at all.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .and(body_json(json!({"name": "Backend"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_edit_response("10001", "Backend", "FOO")),
        )
        .expect(1)
        .mount(&server_b)
        .await;

    let omit_output = jr_cmd(&server_b.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "FOO",
            "--name",
            "Backend",
            "Backend",
        ])
        .output()
        .unwrap();

    server_b.verify().await;
    assert!(
        omit_output.status.success(),
        "Case B: expected exit 0 when --lead omitted; got {:?}\nstderr: {}",
        omit_output.status.code(),
        String::from_utf8_lossy(&omit_output.stderr)
    );
}

// ── AC-010 (BC-8.1.007 P16 — name input, no fields → exit 64, zero HTTP) ─────

/// AC-010 / BC-8.1.007 Precondition 1 (P16 fix-burst): when the input is a
/// component NAME and no edit fields (--name, --description, --lead) are
/// supplied, the handler exits 64 BEFORE making any HTTP call — including zero
/// component-list GETs.  Precondition 1 fires before Precondition 2 (resolution).
#[tokio::test]
async fn test_bc_8_1_007_component_edit_name_input_no_fields_zero_http() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Component list MUST NOT be called (Precondition 1 fires first).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(component_list_response(vec![])))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "edit", "--project", "FOO", "Backend"])
        // No --name, --description, or --lead
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 when no fields supplied; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    // F-A6a (PR#704 finding 6): BC-8.1.007 no-fields guard exact phrase.
    // FAILS against impl that uses a differently-worded message.
    let stderr_10 = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr_10.contains("no fields specified to update"),
        "AC-010 F-A6a: BC-8.1.007 no-fields guard message must contain \
         'no fields specified to update' (BC-8.1.007 exact phrasing); got: {stderr_10}"
    );
    // F-R2-001: the UserError message must NOT include a leading "Error: " prefix
    // (main.rs renders errors as "Error: {e}" — a message with its own "Error: "
    // prefix produces "Error: Error: …" double-prefix).
    assert!(
        !stderr_10.contains("Error: Error:"),
        "AC-010 F-R2-001: stderr must NOT contain doubled 'Error: Error:' prefix; got: {stderr_10}"
    );
}

// ── AC-011 (BC-8.1.007 P16 — numeric input, no fields → exit 64, zero HTTP) ──

/// AC-011 / BC-8.1.007 Precondition 1 (P16 fix-burst ordering): when the input
/// is a NUMERIC component ID and no edit fields are supplied, exit 64 fires
/// BEFORE the confirming GET — not after.  This is the critical ordering
/// invariant that P16 enforces: no-fields guard > confirming GET.
#[tokio::test]
async fn test_bc_8_1_007_component_edit_numeric_input_no_fields_zero_http() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Confirming GET MUST NOT be called when no fields supplied.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_response_with_flags(
                "10001",
                "Backend",
                None,
                None,
                None,
                Some("FOO"),
                None,
            )),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "edit", "10001"])
        // No --name, --description, or --lead
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 when numeric input with no fields; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    // F-A6b (PR#704 finding 6): BC-8.1.007 no-fields guard exact phrase (numeric path).
    // FAILS against impl that uses a differently-worded message.
    let stderr_11 = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr_11.contains("no fields specified to update"),
        "AC-011 F-A6b: BC-8.1.007 no-fields guard message must contain \
         'no fields specified to update' (BC-8.1.007 exact phrasing); got: {stderr_11}"
    );
    // F-R2-001: the UserError message must NOT include a leading "Error: " prefix.
    assert!(
        !stderr_11.contains("Error: Error:"),
        "AC-011 F-R2-001: stderr must NOT contain doubled 'Error: Error:' prefix; got: {stderr_11}"
    );
}

// ── AC-012 (BC-8.1.007 / EC-8.1.007-3 — numeric edit derives project for --lead) ─

/// AC-012 / BC-8.1.007 / EC-8.1.007-3: for numeric component ID, ONE confirming
/// GET (`/rest/api/3/component/{id}`) derives the project key.  `--lead` is then
/// resolved via `multiProjectSearch` scoped to THAT derived project.  The PUT body
/// carries `{"leadAccountId":"acc-eng-lead"}` — verified via `body_json` exact
/// matching.  `.expect(1)` on both GET and PUT enforces no extra calls.
///
/// This covers VP-COMPONENT-002 (edit half) and EC-8.1.007-3: derived-project
/// scoping + correct `leadAccountId` wire key.
#[tokio::test]
async fn test_bc_8_1_007_component_edit_numeric_derives_project_for_lead_resolution() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Exactly ONE confirming GET — derives project key "ENG" for lead lookup.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_response_with_flags(
                "10001",
                "Backend",
                None,
                None,
                None,
                Some("ENG"),
                None,
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Lead resolver: "Alice" resolves to accountId "acc-eng-lead" scoped to ENG.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(multi_project_user_search_response(vec![(
                "acc-eng-lead",
                "Alice",
            )])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PUT must carry exactly {"leadAccountId":"acc-eng-lead"} — body_json enforces equality.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .and(body_json(json!({"leadAccountId": "acc-eng-lead"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_edit_response("10001", "Backend", "ENG")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--lead",
            "Alice",
            "10001", // numeric — no --project required; project derived from GET
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "Expected exit 0 for numeric edit with --lead; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-013 (BC-8.1.007 — --project mismatch → exit 64, zero PUT) ─────────────

/// AC-013 / BC-8.1.007: when the user supplies `--project WRONG` but the
/// confirming GET reveals the component belongs to project "FOO", the handler
/// exits 64 BEFORE the PUT.  Message: "Component 10001 belongs to project FOO,
/// not WRONG."  Zero PUT calls.
#[tokio::test]
async fn test_bc_8_1_007_component_edit_numeric_project_mismatch_zero_put() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Confirming GET: component belongs to FOO.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_response_with_flags(
                "10001",
                "Backend",
                None,
                None,
                None,
                Some("FOO"),
                None,
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PUT MUST NOT be called — mismatch guard fires first.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_edit_response("10001", "X", "FOO")),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "WRONG",
            "--name",
            "Renamed",
            "10001",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for project mismatch; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Pin the BC-8.1.007 M1 verbatim message produced by
    // component.rs::handle_edit § "project mismatch guard".
    // A mutation deleting the project-check branch, or changing the format string,
    // would fail here (loose contains("FOO") && contains("WRONG") would survive
    // such mutations).
    assert!(
        stderr.contains("Component 10001 belongs to project FOO, not WRONG."),
        "Expected BC-8.1.007 M1 verbatim message \
         \"Component 10001 belongs to project FOO, not WRONG.\"; got: {stderr}"
    );
}

// ── AC-013a (BC-8.1.007 — --project case-insensitive leniency, coverage pin) ────

/// AC-013a / BC-8.1.007: when the user supplies `--project eng` (lowercase) but
/// the confirming GET reveals the component belongs to project `"ENG"` (uppercase),
/// the mismatch check uses `eq_ignore_ascii_case`, so the lowercase form is treated
/// as a MATCH — the PUT proceeds and exits 0.
///
/// This test PASSES against the current impl (which uses `eq_ignore_ascii_case`) and
/// pins that behavior so a mutation to strict `==` is caught.
/// (PR#704 finding 5 — coverage pin.)
#[tokio::test]
async fn test_bc_8_1_007_component_edit_numeric_project_case_insensitive_match_proceeds() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Confirming GET: component belongs to "ENG" (uppercase).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_response_with_flags(
                "10001",
                "Backend",
                None,
                None,
                None,
                Some("ENG"),
                None,
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PUT must fire (case-insensitive match → not a mismatch).
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_edit_response("10001", "Renamed", "ENG")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // User passes --project eng (all lowercase) — should be accepted as ENG.
    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "eng",
            "--name",
            "Renamed",
            "10001",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "AC-013a: --project case-insensitive leniency: 'eng' must match 'ENG' → exit 0; \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-014 (BC-8.1.008 — numeric not-found message variants) ─────────────────

/// AC-014 / BC-8.1.008: 404 on the confirming GET produces two message variants:
/// (A) with `--project`: "Component '99999' not found in project FOO."
/// (B) without `--project`: "Component '99999' not found." (project-less)
#[tokio::test]
async fn test_bc_8_1_008_component_edit_numeric_notfound_message_variants() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // ── Case A: --project FOO supplied, confirming GET → 404 ─────────────
    let server_a = MockServer::start().await;
    write_profile_config(config.path(), &server_a.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/99999"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(json!({"errorMessages": ["Not found"]})),
        )
        .expect(1)
        .mount(&server_a)
        .await;

    let with_project = jr_cmd(&server_a.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "FOO",
            "--name",
            "X",
            "99999",
        ])
        .output()
        .unwrap();

    server_a.verify().await;
    assert_eq!(
        with_project.status.code(),
        Some(64),
        "Case A: expected exit 64 for not-found with --project; got {:?}\nstderr: {}",
        with_project.status.code(),
        String::from_utf8_lossy(&with_project.stderr)
    );
    let stderr_a = String::from_utf8_lossy(&with_project.stderr);
    assert!(
        stderr_a.contains("not found in project FOO"),
        "Case A: expected 'not found in project FOO' in message; got: {stderr_a}"
    );
    // F-03: BC-8.1.008 exact message with Run: suffix
    assert!(
        stderr_a.contains("Component '99999' not found in project FOO. Run: jr component list"),
        "AC-014 F-03: Case A stderr must contain BC-8.1.008 exact message with Run: suffix \
         \"Component '99999' not found in project FOO. Run: jr component list\"; got: {stderr_a}"
    );

    // ── Case B: no --project, confirming GET → 404 (project-less message) ─
    let server_b = MockServer::start().await;
    write_profile_config(config.path(), &server_b.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/99999"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(json!({"errorMessages": ["Not found"]})),
        )
        .expect(1)
        .mount(&server_b)
        .await;

    let without_project = jr_cmd(&server_b.uri(), cache.path(), config.path())
        .args(["component", "edit", "--name", "X", "99999"])
        .output()
        .unwrap();

    server_b.verify().await;
    assert_eq!(
        without_project.status.code(),
        Some(64),
        "Case B: expected exit 64 for not-found without --project; got {:?}\nstderr: {}",
        without_project.status.code(),
        String::from_utf8_lossy(&without_project.stderr)
    );
    let stderr_b = String::from_utf8_lossy(&without_project.stderr);
    // F-03: BC-8.1.008 project-less variant — exact message + Run: suffix
    assert!(
        stderr_b.contains("Component '99999' not found."),
        "AC-014 F-03: Case B stderr must match BC-8.1.008 project-less message \
         \"Component '99999' not found.\"; got: {stderr_b}"
    );
    assert!(
        !stderr_b.contains("not found in project"),
        "AC-014 F-03: Case B message must NOT say 'not found in project' (no project known); got: {stderr_b}"
    );
    assert!(
        stderr_b.contains("Run: jr component list --project"),
        "AC-014 F-03: Case B stderr must contain BC-8.1.008 Run: hint \
         'Run: jr component list --project'; got: {stderr_b}"
    );
    assert!(
        stderr_b.contains("to see valid components."),
        "AC-014 F-03: Case B stderr must end with BC-8.1.008 suffix \
         'to see valid components.'; got: {stderr_b}"
    );
}

// ── F-06 (BC-8.1.008 — numeric 404 with .jr.toml project → project-qualified) ─

/// F-06 / BC-8.1.008: numeric edit with NO --project but `.jr.toml` in CWD
/// supplies `project = "FOO"`.  GET 404 → project-qualified not-found message,
/// NOT the project-less variant (which tells the user to supply --project).
#[tokio::test]
async fn test_bc_8_1_008_component_edit_numeric_notfound_config_project_qualified() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // .jr.toml in CWD supplies the project without --project flag.
    std::fs::write(cwd.path().join(".jr.toml"), "project = \"FOO\"\n").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/99999"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(json!({"errorMessages": ["Not found"]})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "edit", "--name", "X", "99999"])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "F-06: expected exit 64 for numeric not-found with .jr.toml project; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Must use the project-qualified variant since project is known from .jr.toml
    assert!(
        stderr.contains("Component '99999' not found in project FOO. Run: jr component list"),
        "F-06: stderr must contain BC-8.1.008 project-qualified message \
         \"Component '99999' not found in project FOO. Run: jr component list\"; got: {stderr}"
    );
    assert!(
        !stderr.contains("to see valid components."),
        "F-06: project-qualified variant must NOT contain the project-less suffix \
         'to see valid components.'; got: {stderr}"
    );
}

// ── AC-015 (BC-8.4.002/003 — name not-found and ambiguous messages) ───────────

/// AC-015 / BC-8.4.002+BC-8.4.003: name-based resolution (via project component
/// list) produces:
/// (A) Not found → "Component 'xyz' not found in project FOO. Available: Backend, Frontend."
/// (B) Ambiguous → "Ambiguous component 'back'. Matches: Backend, Backoffice."
#[tokio::test]
async fn test_bc_8_1_008_component_edit_name_notfound_and_ambiguous_messages() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // ── Case A: not found ─────────────────────────────────────────────────
    let server_a = MockServer::start().await;
    write_profile_config(config.path(), &server_a.uri());

    // B-01: fixture supplies components in NON-alphabetical order so the impl
    // must sort them before rendering.  The expected sorted list is:
    // Api, Backend, Zebra (case-insensitive alphabetical).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Zebra", None, None, None),
                component_response("10002", "Api", None, None, None),
                component_response("10003", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server_a)
        .await;

    let not_found = jr_cmd(&server_a.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "FOO",
            "--name",
            "X",
            "xyz",
        ])
        .output()
        .unwrap();

    server_a.verify().await;
    assert_eq!(
        not_found.status.code(),
        Some(64),
        "Case A: expected exit 64 for not-found; got {:?}\nstderr: {}",
        not_found.status.code(),
        String::from_utf8_lossy(&not_found.stderr)
    );
    let stderr_a = String::from_utf8_lossy(&not_found.stderr);
    // F-08: BC-8.4.002 exact prefix (name not-found message)
    assert!(
        stderr_a.contains("Component 'xyz' not found in project FOO. Available:"),
        "AC-015 F-08: Case A stderr must contain BC-8.4.002 exact prefix \
         \"Component 'xyz' not found in project FOO. Available:\"; got: {stderr_a}"
    );
    // B-01 / BC-8.4.002: Available list must be ALPHABETICALLY SORTED (case-insensitive).
    // B-02 / BC-8.4.002: Available list must end with a trailing period.
    // Fixture returns ["Zebra","Api","Backend"] (non-alphabetical); impl must sort and terminate with ".".
    assert!(
        stderr_a.contains("Available: Api, Backend, Zebra."),
        "AC-015 F-08 B-01/B-02: Case A stderr must contain alphabetically-sorted Available list \
         with trailing period \"Available: Api, Backend, Zebra.\"; got: {stderr_a}"
    );

    // ── Case B: ambiguous match ───────────────────────────────────────────
    let server_b = MockServer::start().await;
    write_profile_config(config.path(), &server_b.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
                component_response("10003", "Backoffice", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server_b)
        .await;

    let ambiguous = jr_cmd(&server_b.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "FOO",
            "--name",
            "X",
            "back",
        ])
        .output()
        .unwrap();

    server_b.verify().await;
    assert_eq!(
        ambiguous.status.code(),
        Some(64),
        "Case B: expected exit 64 for ambiguous; got {:?}\nstderr: {}",
        ambiguous.status.code(),
        String::from_utf8_lossy(&ambiguous.stderr)
    );
    let stderr_b = String::from_utf8_lossy(&ambiguous.stderr);
    // F-08: BC-8.4.003 exact prefix — case-sensitive "Ambiguous component"
    assert!(
        stderr_b.contains("Ambiguous component 'back'. Matches:"),
        "AC-015 F-08: Case B stderr must contain BC-8.4.003 exact prefix \
         \"Ambiguous component 'back'. Matches:\"; got: {stderr_b}"
    );
    assert!(
        stderr_b.contains("Backend") && stderr_b.contains("Backoffice"),
        "AC-015 F-08: Case B BC-8.4.003 Matches list must include Backend and Backoffice; got: {stderr_b}"
    );
    // B-02 / BC-8.4.003: Matches list must end with a trailing period.
    assert!(
        stderr_b.contains("Matches: Backend, Backoffice."),
        "AC-015 F-08 B-02: Case B stderr must contain Matches list with trailing period \
         \"Matches: Backend, Backoffice.\"; got: {stderr_b}"
    );
}

// ── AC-016 (BC-8.1.007/BC-8.1.008 — PUT-race 404 exits 1, not 64) ────────────

/// AC-016 / VP-COMPONENT-024: a 404 on the mutating PUT (after successful
/// resolution) is a racing delete — the component existed at resolve time but
/// is gone by mutation time.  This is ApiError (exit 1), NOT UserError (exit 64).
/// Distinct from resolver 404 which is exit 64 (BC-8.1.008).
#[tokio::test]
async fn test_bc_8_1_007_component_edit_put_race_404_exits_1_distinct_from_resolver_404() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Resolution succeeds — component found.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PUT races and returns 404 — component deleted between resolve and mutate.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(json!({"errorMessages": ["Not found"]})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "FOO",
            "--name",
            "Renamed",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    // Must be exit 1 (ApiError), NOT exit 64 (UserError).
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected exit 1 for PUT-race 404 (ApiError, not UserError); got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-017 (BC-8.1.004 — numeric exemption vs name requires project) ─────────

/// AC-017 / BC-8.1.004: all-ASCII-digit component IDs bypass the no-project
/// guard (numeric-id exemption).  A name-based input without `--project` exits
/// 64 before any HTTP.
/// Numeric case: handler proceeds and exits 0.  Name case: exits 64 (project guard fires before HTTP).
#[tokio::test]
async fn test_bc_8_1_004_component_edit_numeric_id_exemption_vs_name_requires_project() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // ── Case A: numeric input, no --project → should proceed (needs confirming GET)
    let server_a = MockServer::start().await;
    write_profile_config(config.path(), &server_a.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_response_with_flags(
                "10001",
                "Backend",
                None,
                None,
                None,
                Some("FOO"),
                None,
            )),
        )
        .mount(&server_a)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_edit_response("10001", "New Name", "FOO")),
        )
        .mount(&server_a)
        .await;

    let numeric_output = jr_cmd(&server_a.uri(), cache.path(), config.path())
        .args(["component", "edit", "--name", "New Name", "10001"])
        .output()
        .unwrap();

    // Handler proceeds for numeric input; expected exit 0.
    assert!(
        numeric_output.status.success(),
        "Case A: numeric exempt from --project guard; expected exit 0; got {:?}\nstderr: {}",
        numeric_output.status.code(),
        String::from_utf8_lossy(&numeric_output.stderr)
    );

    // ── Case B: name input, no --project → exit 64, zero HTTP ────────────
    let server_b = MockServer::start().await;
    write_profile_config(config.path(), &server_b.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(component_list_response(vec![])))
        .expect(0)
        .mount(&server_b)
        .await;

    let cwd = TempDir::new().unwrap(); // no .jr.toml → no configured project

    let name_output = jr_cmd(&server_b.uri(), cache.path(), config.path())
        .args(["component", "edit", "--name", "New Name", "backend"])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    server_b.verify().await;
    assert_eq!(
        name_output.status.code(),
        Some(64),
        "Case B: name input without --project must exit 64; got {:?}\nstderr: {}",
        name_output.status.code(),
        String::from_utf8_lossy(&name_output.stderr)
    );
    let stderr_b = String::from_utf8_lossy(&name_output.stderr);
    assert!(
        stderr_b.contains("--project"),
        "Case B: expected '--project' mentioned in error; got: {stderr_b}"
    );
}

// ── AC-018 (ADR-0018 §2 — create/edit invalidate components cache) ───────────

/// AC-018 / ADR-0018 §2: after a successful `component create` or
/// `component edit`, `invalidate_components_cache(profile, project_key)` is
/// called, removing the cached entry for that project.
///
/// The test pre-writes a cache file, runs the command, then asserts the cache
/// entry for the project is absent.
#[tokio::test]
async fn test_adr_0018_component_create_and_edit_invalidate_cache() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Pre-write a components cache entry for project FOO.
    // Path: {JR_CACHE_DIR}/v1/default/components_default.json
    let cache_dir_path = cache.path().join("v1").join("default");
    std::fs::create_dir_all(&cache_dir_path).unwrap();
    let cache_file = cache_dir_path.join("components_default.json");
    std::fs::write(
        &cache_file,
        r#"{"FOO":{"components":[{"id":"10001","name":"Backend"}],"fetched_at":"2026-01-01T00:00:00Z"}}"#,
    )
    .unwrap();

    // Assert the cache file has the FOO entry before the command.
    let before: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cache_file).unwrap()).unwrap();
    assert!(
        before.get("FOO").is_some(),
        "Pre-condition: FOO entry must be present in cache before command"
    );

    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(component_create_response("10002", "API", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "create", "--project", "FOO", "API"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected exit 0 after create (cache invalidation only happens on success); \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    // After successful create, the FOO cache entry must be gone.
    let after_content = std::fs::read_to_string(&cache_file).unwrap_or_default();
    let after: serde_json::Value = serde_json::from_str(&after_content).unwrap_or(json!({}));
    assert!(
        after.get("FOO").is_none(),
        "After create, FOO entry must be removed from components cache (ADR-0018 §2); \
         cache after: {after}"
    );

    // ── Edit path (ADR-0018 §2 — edit also invalidates cache) ────────────────
    // Use a fresh isolated cache and server so the create arm above cannot
    // bleed state into this arm.
    let cache2 = TempDir::new().unwrap();
    let config2 = TempDir::new().unwrap();
    let server2 = MockServer::start().await;
    write_profile_config(config2.path(), &server2.uri());

    // Pre-write a components cache entry for project FOO in the new cache dir.
    let cache2_dir = cache2.path().join("v1").join("default");
    std::fs::create_dir_all(&cache2_dir).unwrap();
    let cache2_file = cache2_dir.join("components_default.json");
    std::fs::write(
        &cache2_file,
        r#"{"FOO":{"components":[{"id":"10001","name":"Backend"}],"fetched_at":"2026-01-01T00:00:00Z"}}"#,
    )
    .unwrap();

    // Assert the cache file has the FOO entry before the command.
    let before_edit: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cache2_file).unwrap()).unwrap();
    assert!(
        before_edit.get("FOO").is_some(),
        "Pre-condition (edit arm): FOO entry must be present in cache before edit"
    );

    // Confirming GET: component 10001 belongs to project FOO.
    // (Numeric ID path — ADR-0018 §1: ONE confirming GET derives project.)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_response_with_flags(
                "10001",
                "Backend",
                None,
                None,
                None,
                Some("FOO"),
                None,
            )),
        )
        .expect(1)
        .mount(&server2)
        .await;

    // PUT: successful edit — cache invalidation fires only on success.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_edit_response("10001", "Renamed", "FOO")),
        )
        .expect(1)
        .mount(&server2)
        .await;

    let edit_output = jr_cmd(&server2.uri(), cache2.path(), config2.path())
        .args(["component", "edit", "--name", "Renamed", "10001"])
        .output()
        .unwrap();

    server2.verify().await;
    assert!(
        edit_output.status.success(),
        "Expected exit 0 after edit (cache invalidation only happens on success); \
         got {:?}\nstderr: {}",
        edit_output.status.code(),
        String::from_utf8_lossy(&edit_output.stderr)
    );

    // After successful edit, the FOO cache entry must be gone (ADR-0018 §2).
    let after_edit_content = std::fs::read_to_string(&cache2_file).unwrap_or_default();
    let after_edit: serde_json::Value =
        serde_json::from_str(&after_edit_content).unwrap_or(json!({}));
    assert!(
        after_edit.get("FOO").is_none(),
        "After edit, FOO entry must be removed from components cache (ADR-0018 §2); \
         cache after: {after_edit}"
    );
}

/// AC-018b / ADR-0018 §2: a FAILED `component edit` (PUT 500) must NOT
/// invalidate the components cache.  The coverage pin here would catch a
/// mutation that calls `invalidate_components_cache` unconditionally (before
/// the PUT or in the error branch).
///
/// A mutation calling `invalidate_components_cache` unconditionally would wipe
/// the cache even on failure, which this test catches.
#[tokio::test]
async fn test_adr_0018_component_edit_failed_does_not_invalidate_cache() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Pre-write a components cache entry for project FOO.
    let cache_dir_path = cache.path().join("v1").join("default");
    std::fs::create_dir_all(&cache_dir_path).unwrap();
    let cache_file = cache_dir_path.join("components_default.json");
    std::fs::write(
        &cache_file,
        r#"{"FOO":{"components":[{"id":"10001","name":"Backend"}],"fetched_at":"2026-01-01T00:00:00Z"}}"#,
    )
    .unwrap();

    // Confirming GET: component 10001 belongs to FOO.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_response_with_flags(
                "10001",
                "Backend",
                None,
                None,
                None,
                Some("FOO"),
                None,
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PUT: server returns 500 — the edit fails.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "errorMessages": ["Internal server error"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "edit", "--name", "Renamed", "10001"])
        .output()
        .unwrap();

    server.verify().await;

    // The command must NOT succeed.
    assert!(
        !output.status.success(),
        "Expected non-zero exit on PUT 500; got success\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // The FOO cache entry must still be present — invalidation must NOT fire on
    // failure.
    let cache_content = std::fs::read_to_string(&cache_file).unwrap_or_default();
    let cache_after: serde_json::Value = serde_json::from_str(&cache_content).unwrap_or(json!({}));
    assert!(
        cache_after.get("FOO").is_some(),
        "After failed edit, FOO entry must REMAIN in components cache; \
         cache after: {cache_after}"
    );
}

// ── Pass-7/8 coverage: edit --lead resolution paths ──────────────────────────

/// VP-COMPONENT-002 (edit half) / EC-8.1.007-3: `--lead` returns 0 matches on
/// the name-based edit path → exit 64, zero PUT calls.
///
/// Exercises `src/cli/component.rs::handle_edit` § "0-match arm → UserError".
#[tokio::test]
async fn test_bc_8_1_006_component_edit_lead_no_match_zero_put() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Name-based resolution: list components for project ENG.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/ENG/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Lead resolver returns no users.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(multi_project_user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PUT MUST NOT be called — guard fires before the PUT.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "ENG",
            "--lead",
            "Alice",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for no-match --lead on edit; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No user matching 'Alice'"),
        "Expected BC-8.1.006 no-match message; got: {stderr}"
    );
}

/// VP-COMPONENT-002 (edit half) / BC-8.1.006: `--lead` returns 2+ matches on
/// the name-based edit path → exit 64, zero PUT calls.  Stderr lists each
/// candidate's email + accountId (BC-X.7.004).
///
/// Exercises `src/cli/component.rs::handle_edit` § "2+-match arm → UserError with candidate list".
#[tokio::test]
async fn test_bc_8_1_006_component_edit_lead_ambiguous_zero_put() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Name-based resolution: list components for project ENG.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/ENG/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Lead resolver returns 2 matches — ambiguous.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            multi_project_user_search_response_with_email(vec![
                ("acc-001", "Alice Smith", "alice.smith@example.com"),
                ("acc-002", "Alice Jones", "alice.jones@example.com"),
            ]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    // PUT MUST NOT be called — ambiguity guard fires before the PUT.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "ENG",
            "--lead",
            "alice",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for ambiguous --lead on edit; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    // BC-8.1.006 / BC-X.7.004: both candidate emails or accountIds must appear.
    assert!(
        stderr.contains("alice.smith@example.com") || stderr.contains("acc-001"),
        "Expected first candidate (alice.smith@example.com or acc-001) in stderr; got: {stderr}"
    );
    assert!(
        stderr.contains("alice.jones@example.com") || stderr.contains("acc-002"),
        "Expected second candidate (alice.jones@example.com or acc-002) in stderr; got: {stderr}"
    );
}

// ── PR#704 Finding A (BC-X.10.003 — ExactMultiple fail-closed) ───────────────

/// BC-X.10.003 / PR#704 Finding A (HIGH):
/// When `partial_match` returns `ExactMultiple` — two components share the same
/// name case-insensitively — `handle_edit` MUST exit 64 and emit a
/// "Multiple components named … (IDs: …). Pass the numeric ID directly."
/// message.  It MUST NOT silently pick the first component and call PUT.
///
/// `handle_edit` in `src/cli/component.rs::handle_edit` § "ExactMultiple guard"
/// handles this path, mirroring `src/cli/requesttype.rs::handle_list` §
/// "ExactMultiple fail-closed" and `src/cli/queue.rs::handle_view` §
/// "ExactMultiple fail-closed".
#[tokio::test]
async fn test_bc_x_10_003_component_edit_exact_multiple_fails_closed() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Two components share the same name case-insensitively:
    // "Backend" (10001) and "backend" (10002).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_two_same_name(
                "10001", "Backend", "10002", "backend",
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PUT MUST NOT be called — ExactMultiple fail-closed guard fires before any
    // mutation.
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "FOO",
            "--name",
            "Renamed",
            "backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-X.10.003: expected exit 64 for ExactMultiple; \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Exact message shape mirrors requesttype.rs::handle_list § "ExactMultiple":
    //   Multiple components named "<first-casing>" found (IDs: 10001, 10002). Pass the numeric ID directly.
    assert!(
        stderr.contains("Multiple components named"),
        "BC-X.10.003: expected 'Multiple components named' in stderr; got: {stderr}"
    );
    assert!(
        stderr.contains("10001") && stderr.contains("10002"),
        "BC-X.10.003: expected both IDs (10001, 10002) in stderr; got: {stderr}"
    );
    // F-02: IDs must appear in LIST order (10001 before 10002).
    {
        let idx_10001 = stderr.find("10001").expect("10001 must appear in stderr");
        let idx_10002 = stderr.find("10002").expect("10002 must appear in stderr");
        assert!(
            idx_10001 < idx_10002,
            "BC-X.10.003 F-02: ID 10001 must appear before 10002 in stderr (list order); \
             got: {stderr}"
        );
    }
    assert!(
        stderr.contains("Pass the numeric ID directly"),
        "BC-X.10.003: expected 'Pass the numeric ID directly' in stderr; got: {stderr}"
    );
}

// ── PR#704 Finding B (allow_hyphen_values asymmetry — hyphen-leading names) ───

/// BC-8.1.005 / PR#704 Finding B (MINOR):
/// The `name` positional of `component create` MUST accept leading-dash values
/// (e.g. `-legacy`) so components with hyphen-prefixed names can be created.
///
/// `src/cli/mod.rs::ComponentSubcommand::Create` § "name positional" has
/// `allow_hyphen_values = true` so clap passes `-legacy` through instead of
/// treating it as an unknown flag.
#[tokio::test]
async fn test_bc_8_1_005_component_create_hyphen_leading_name() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .and(body_json(json!({"name": "-legacy", "project": "FOO"})))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(component_create_response("10001", "-legacy", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "create", "--project", "FOO", "-legacy"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "BC-8.1.005: expected exit 0 for hyphen-leading name '-legacy'; \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// BC-8.1.007 / PR#704 Finding B (MINOR):
/// `component edit --name <value>` MUST accept a leading-dash new name
/// (e.g. `--name -legacy`) so components can be renamed to hyphen-prefixed names.
///
/// `src/cli/mod.rs::ComponentSubcommand::Edit` § "`--name` flag" has
/// `allow_hyphen_values = true` so clap passes `-legacy` through instead of
/// treating it as an unknown flag.
#[tokio::test]
async fn test_bc_8_1_007_component_edit_hyphen_leading_new_name() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Name-based lookup: resolve "Backend" to id 10001.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PUT body must carry exactly {"name": "-legacy"}.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .and(body_json(json!({"name": "-legacy"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_edit_response("10001", "-legacy", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "Backend",
            "--project",
            "FOO",
            "--name",
            "-legacy",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "BC-8.1.007: expected exit 0 for hyphen-leading --name '-legacy'; \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── PR#704 Finding C (numeric-edit fail-open when GET returns no project field) ─

/// BC-8.1.007 / PR#704 Finding C (MINOR):
/// When a numeric `component edit` GET returns a component with NO `project`
/// field AND the user supplies `--project`, the handler MUST exit 64
/// (cannot verify the component's project) and issue ZERO PUTs.
///
/// `src/cli/component.rs::handle_edit` § "missing-project fail-closed guard"
/// treats an absent project field + a user-supplied `--project` as an error,
/// rather than silently adopting the supplied key as unverified scope.
#[tokio::test]
async fn test_bc_8_1_007_component_edit_numeric_missing_project_field_fails_closed() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Confirming GET: component exists but Jira returned no `"project"` key.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_response_no_project_field("10001", "Backend")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PUT MUST NOT fire — missing-project guard fires first.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "10001",
            "--project",
            "WRONG",
            "--name",
            "X",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-8.1.007: expected exit 64 when project field is absent but --project \
         is supplied; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── F-R3-001 (BC-8.1.007 — global --project honored by edit) ─────────────────

/// BC-8.1.007 / F-R3-001 (MEDIUM — coverage pin):
/// `component edit` MUST honor the global `--project` flag (placed BEFORE the
/// subcommand) the same way `component list` does.
///
/// clap's `global = true` on `Cli::project` propagates `--project FOO` from
/// the root into `ComponentSubcommand::Edit.project` directly, so the edit
/// subcommand receives `project = Some("FOO")` without an explicit merge in
/// `src/cli/component.rs::handle` § "Edit dispatch".  This test pins that
/// behavior: a mutation removing the `project` arg from `ComponentSubcommand::Edit`
/// or blocking global propagation would cause the GET to receive no project key
/// and fail.
///
/// **Regression pin against a recurring false-positive:** two independent
/// adversary passes wrongly claimed `component edit` drops the global
/// `--project` flag because `handle_edit` has no explicit `.or(project_flag)`
/// call.  It does NOT drop it — clap's `global = true` definition on the
/// top-level `Cli::project` field makes the parsed value available directly
/// inside `ComponentSubcommand::Edit.project` before `handle_edit` is even
/// called.  `jr --project FOO component edit …` therefore resolves against
/// FOO even though `--project` appears BEFORE the subcommand.  Do NOT add a
/// redundant `.or(project_flag)` merge to silence that concern — it would be
/// a no-op at best and could mask a future clap API change at worst.
#[tokio::test]
async fn test_bc_8_1_007_component_edit_honors_global_project_flag() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    // Write a profile config with NO default project — the global --project flag
    // is the ONLY source of the project key.
    write_profile_config(config.path(), &server.uri());

    // Name-based resolution: one component in FOO.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PUT MUST fire — edit proceeds with the global-flag project.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .and(body_json(json!({"name": "New"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_edit_response("10001", "New", "FOO")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Global flag BEFORE the subcommand — no per-subcommand --project.
    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "--project",
            "FOO",
            "component",
            "edit",
            "Backend",
            "--name",
            "New",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-8.1.007 F-R3-001: expected exit 0 when project is supplied via the \
         global --project flag; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── F-R3-002 (BC-8.1.005/007 — name-collision 400 surfaced) ──────────────────

/// BC-8.1.005 / F-R3-002 (LOW — coverage pin):
/// When `POST /rest/api/3/component` returns 400 (e.g. "A component with the
/// name already exists"), the error body MUST be surfaced to the user.
///
/// Regression pin: verifies `handle_create` propagates the API error without
/// swallowing or replacing it.
#[tokio::test]
async fn test_bc_8_1_005_component_create_name_collision_400_surfaced() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("POST"))
        .and(path("/rest/api/3/component"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "errorMessages": ["A component with the name already exists."]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "create", "--project", "FOO", "Backend"])
        .output()
        .unwrap();

    server.verify().await;
    assert_ne!(
        output.status.code(),
        Some(0),
        "BC-8.1.005 F-R3-002: expected non-zero exit for 400 collision; \
         got exit 0\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("A component with the name already exists"),
        "BC-8.1.005 F-R3-002: Jira 400 body must be surfaced in stderr; got: {stderr}"
    );
}

/// BC-8.1.007 / F-R3-002 (LOW — coverage pin):
/// When `PUT /rest/api/3/component/{id}` returns 400 (e.g. "A component with
/// the name already exists"), the error body MUST be surfaced to the user.
///
/// Regression pin: verifies `handle_edit` propagates the API error without
/// swallowing or replacing it.
#[tokio::test]
async fn test_bc_8_1_007_component_edit_name_collision_400_surfaced() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Name-based resolution: one component in FOO.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // PUT returns 400.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "errorMessages": ["A component with the name already exists."]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "edit",
            "--project",
            "FOO",
            "--name",
            "Backend",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_ne!(
        output.status.code(),
        Some(0),
        "BC-8.1.007 F-R3-002: expected non-zero exit for 400 collision; \
         got exit 0\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("A component with the name already exists"),
        "BC-8.1.007 F-R3-002: Jira 400 body must be surfaced in stderr; got: {stderr}"
    );
}

// =============================================================================
// S-604-3: `jr component delete` — disposition-required, snapshot-before-delete
// safety (DEC-279). SAFETY-CRITICAL — tdd_mode: strict.
//
// BC anchors: BC-8.2.001–BC-8.2.008. All tests below are RED at Red Gate:
// `handle_delete` is `todo!()`.
//
// Ordering assertions (BC-8.2.007 "a mutant that reorders snapshot/DELETE
// must fail") use `server.received_requests()` position comparison — the
// same idiom already established in `tests/attachment_upload.rs` for
// VP-576-003's DELETE-before-POST pin.
//
// Verbatim-pin discipline (drift LOOSE-CONTAINS-MASKS-BC-VERBATIM-MESSAGE-
// DRIFT): every BC-specified exact string below is asserted as a full
// substring match on the EXACT sentence, not a subset of loosely related
// words. Where BC gives no exact string (e.g. BC-8.2.002's numeric-target-
// mismatch message, BC-8.2.008's table-mode echo), only structural/content
// assertions are made — never invented and asserted as if verbatim.
// =============================================================================

/// Build a `jr component delete` command with a fixed harness (mirrors
/// `jr_cmd` but scoped to this section's doc so a reader doesn't have to
/// scroll up to find it).
fn delete_cmd(
    server_uri: &str,
    cache_dir: &std::path::Path,
    config_dir: &std::path::Path,
) -> Command {
    jr_cmd(server_uri, cache_dir, config_dir)
}

// ── AC-001 (BC-8.2.001 postcondition 1 / VP-COMPONENT-003) ───────────────────

/// AC-001 / EC-8.2.001-1: neither `--move-to` nor `--orphan` supplied → exit 64,
/// stderr names BOTH flags, ZERO `DELETE`/snapshot-search calls.
///
/// Invariant 1 (BC-8.2.001): the NAME|ID resolution (§8.4) still fires BEFORE
/// the disposition guard — so the project component-list GET IS expected here
/// (`.expect(1)`), even though the command ultimately fails.
///
/// This test also covers EC-8.2.001-1 verbatim (the story's own worked
/// example is this exact command shape).
#[tokio::test]
async fn test_bc_8_2_001_component_delete_neither_flag_exits_64_zero_http() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Invariant 1: resolution happens even without a disposition.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "delete", "Backend", "--project", "FOO"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-001: expected exit 64 for neither-flag; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--move-to <NAME|ID>"),
        "AC-001: stderr must name --move-to <NAME|ID>; got: {stderr}"
    );
    assert!(
        stderr.contains("--orphan"),
        "AC-001: stderr must name --orphan; got: {stderr}"
    );
    // BC-8.2.001 Postcondition 1 (corrected M3 fix-burst): the message does
    // NOT include an affected-issue count — the snapshot never fired.
    assert!(
        !stderr.contains("issue(s)"),
        "AC-001: no-disposition message must NOT include an affected-issue \
         count (the snapshot never fires in this path); got: {stderr}"
    );
}

// ── AC-002 (BC-8.2.001 postcondition 2/3, DEC-188 mechanism) ─────────────────

/// AC-002 / EC-8.2.001-2: `--move-to X --orphan` together → clap exit 2
/// (mutual exclusion), before any resolution or HTTP call whatsoever.
///
/// Also asserts the DEC-188 mechanism split: this exit-2 case is a clap
/// `conflicts_with` violation, structurally distinct from AC-001's
/// application-level exit-64 guard.
///
/// NOTE (Red Gate): unlike every other S-604-3 test, this one PASSES both
/// before and after `handle_delete` is implemented — `conflicts_with` on
/// `ComponentSubcommand::Delete` (`src/cli/mod.rs`) is clap-derive parse-time
/// validation, already wired independently of the still-`todo!()` handler
/// body; the handler is never reached for a conflicting pair. Same
/// pre-existing pattern as `test_bc_8_1_005_component_create_bad_assignee_type_exits_2`
/// above. This is the one test in this Red Gate report that is expected to
/// be green at Red Gate, not a false/tautological pass.
#[tokio::test]
async fn test_bc_8_2_001_component_delete_both_flags_clap_exit_2() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(component_list_response(vec![])))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "Frontend",
            "--orphan",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(2),
        "AC-002: both flags together must be a clap mutual-exclusion exit 2, \
         NOT the app-level exit 64; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-003 (BC-8.2.001 Invariant 1 / EC-8.2.001-3) ────────────────────────────

/// AC-003: `jr component delete Nonexistent --orphan` (NAME, unresolvable)
/// → exit 64 "not found" (Invariant 1 ordering), NOT the disposition-guard
/// message — even though `--orphan` alone would otherwise satisfy the guard.
#[tokio::test]
async fn test_bc_8_2_001_component_delete_name_notfound_before_disposition_guard() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Nonexistent",
            "--project",
            "FOO",
            "--orphan",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-003: expected exit 64 not-found; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    // BC-8.4.002 verbatim message (Invariant 1: the not-found path, not the
    // disposition guard, must fire).
    assert!(
        stderr.contains("Component 'Nonexistent' not found in project FOO. Available: Backend."),
        "AC-003: expected BC-8.4.002 verbatim not-found message; got: {stderr}"
    );
    assert!(
        !stderr.contains("--move-to <NAME|ID>"),
        "AC-003: must NOT show the disposition-guard message (Invariant 1 \
         ordering — not-found reports first); got: {stderr}"
    );
}

// ── AC-004 (BC-8.2.001 Invariant 1 documented exception / EC-8.2.001-4) ──────

/// AC-004: `jr component delete 999999999` (numeric, nonexistent, NEITHER
/// flag) → exit 64 disposition-guard message, NOT "not found" — the inverse
/// of AC-003. Per the documented numeric/no-disposition asymmetry, there is
/// NO HTTP call available in this path to discover the id's non-existence
/// (the numeric-source confirming GET only fires once a disposition is
/// chosen) — so ZERO HTTP calls of any kind occur.
#[tokio::test]
async fn test_bc_8_2_001_component_delete_numeric_no_disposition_asymmetry() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Zero HTTP whatsoever — no confirming GET is reachable in this path.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/999999999"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(json!({"errorMessages": ["Not found"]})),
        )
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/999999999"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "delete", "999999999"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-004: expected exit 64 disposition-guard (not not-found); \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--move-to <NAME|ID>") && stderr.contains("--orphan"),
        "AC-004: stderr must be the disposition-guard message naming both \
         flags (the inverse of AC-003); got: {stderr}"
    );
    assert!(
        !stderr.to_lowercase().contains("not found"),
        "AC-004: must NOT report 'not found' — no HTTP call in this path \
         could have discovered non-existence; got: {stderr}"
    );
}

// ── AC-005 (BC-8.2.002 postcondition 2 — move-to success) ────────────────────

/// AC-005: `--move-to Frontend` → target resolves BEFORE `DELETE`; `DELETE
/// /rest/api/3/component/{sourceId}?moveIssuesTo=<targetId>` fires exactly
/// once on success. Ordering pinned via `received_requests()` position:
/// the snapshot search must precede the DELETE.
#[tokio::test]
async fn test_bc_8_2_002_component_delete_move_to_success_delete_after_resolution() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
                component_response("10002", "Frontend", None, None, None),
            ])),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(
            json!({"jql": "component = 10001 ORDER BY key ASC"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1", "FOO-2"], None)),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .and(query_param("moveIssuesTo", "10002"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "Frontend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-005: expected exit 0 on move-to success; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let received = server.received_requests().await.unwrap();
    let search_pos = received
        .iter()
        .position(|r| r.url.path() == "/rest/api/3/search/jql")
        .expect("AC-005: snapshot search must have fired");
    let delete_pos = received
        .iter()
        .position(|r| r.method == wiremock::http::Method::DELETE)
        .expect("AC-005: DELETE must have fired");
    assert!(
        search_pos < delete_pos,
        "AC-005: snapshot search (pos {search_pos}) must fire BEFORE DELETE \
         (pos {delete_pos})"
    );
}

// ── S-604-3 coverage pin (BC-8.2.002 — global --project flag propagation) ────

/// S-604-3 coverage pin / refutes ADV pass-4 LOW-1
/// (`ADVERSARY-READONLY-CLAP-INFERENCE-FALSE-POSITIVE`): this is the
/// delete-side analog of `test_bc_8_1_007_component_edit_honors_global_project_flag`.
///
/// Adversarial pass-4 claimed `handle_delete`'s `ComponentSubcommand::Delete`
/// dispatch arm drops the GLOBAL `--project` flag because it doesn't
/// explicitly forward a `project_flag` parameter. This was empirically
/// REFUTED: the top-level `--project` is `#[arg(long, global = true)]`
/// (`src/cli/mod.rs:29`), so clap propagates it directly into
/// `ComponentSubcommand::Delete`'s OWN `project` field, which `handle_delete`
/// reads via `config.project_key(args.project.as_deref())` — no separate
/// forwarding is needed or possible. A live binary run of
/// `jr --project FOO component delete Backend --orphan --yes` reached the
/// HTTP layer (network error), NOT the "No project configured" exit-64
/// guard, proving propagation works end to end.
///
/// This test is the durable coverage pin for that empirical finding: config
/// carries NO default project — the global `--project` flag is the ONLY
/// source of the project key — and `--project` is placed BEFORE the
/// subcommand (no per-subcommand `--project`). If a future refactor ever
/// broke global-flag propagation into the delete dispatch arm,
/// `handle_delete` would see `project: None`, exit 64 ("No project
/// configured") before any HTTP call, and BOTH the components-GET
/// `.expect(1)` and the DELETE `.expect(1)` below would go unmet — failing
/// this test (and `server.verify()` would panic).
#[tokio::test]
async fn test_bc_8_2_002_component_delete_honors_global_project_flag() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    // Write a profile config with NO default project — the global --project
    // flag is the ONLY source of the project key.
    write_profile_config(config.path(), &server.uri());

    // Name-based resolution: one component in FOO.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Zero affected issues — orphan --yes proceeds without a prompt.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(
            json!({"jql": "component = 10001 ORDER BY key ASC"}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_delete_snapshot_page(&[], None)),
        )
        .expect(1)
        .mount(&server)
        .await;

    // DELETE MUST fire (no moveIssuesTo) — proceeds with the global-flag project.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .and(query_param_is_missing("moveIssuesTo"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // Global flag BEFORE the subcommand — no per-subcommand --project.
    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "--project",
            "FOO",
            "component",
            "delete",
            "Backend",
            "--orphan",
            "--yes",
            "--no-input",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-8.2.002 S-604-3: expected exit 0 when project is supplied via the \
         global --project flag; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-006 (BC-8.2.003 Behavior / EC-8.2.003-1) ───────────────────────────────

/// AC-006: `--move-to Backend` where the SAME-named component exists ONLY in
/// a different project (BAR) → target resolution is scoped EXCLUSIVELY to the
/// source's project (FOO); BAR's component-list endpoint is never called
/// (`.expect(0)`) — a cross-project name collision must never be silently
/// considered a match.
#[tokio::test]
async fn test_bc_8_2_003_component_delete_move_to_never_spans_projects() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // FOO's own component list does NOT contain "Backend" — only "Widget"
    // (the component being deleted).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10005", "Widget", None, None, None),
            ])),
        )
        .mount(&server)
        .await;

    // BAR has a component named "Backend" — MUST NEVER be considered.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/BAR/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("20007", "Backend", None, None, None),
            ])),
        )
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10005"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Widget",
            "--project",
            "FOO",
            "--move-to",
            "Backend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-006: expected exit 64 (target not found in scope); got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Component 'Backend' not found in project FOO. Available: Widget."),
        "AC-006: expected BC-8.4.002 verbatim not-found message scoped to \
         FOO only; got: {stderr}"
    );
}

// ── AC-007 (BC-8.2.002 numeric-target confirmation / EC-8.2.003-2) ───────────

/// AC-007: `--move-to 20007` (numeric, belonging to a DIFFERENT project than
/// the source) → confirming `GET /rest/api/3/component/20007` returns the
/// mismatching project → exit 64, ZERO `DELETE`.
#[tokio::test]
async fn test_bc_8_2_002_component_delete_move_to_numeric_target_project_mismatch() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server)
        .await;

    // Numeric target confirming GET: belongs to project BAR, not FOO.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/20007"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_response_with_flags(
                "20007",
                "Other",
                None,
                None,
                None,
                Some("BAR"),
                None,
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "20007",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-007: expected exit 64 for numeric target project mismatch; \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("20007"),
        "AC-007: stderr should reference the mismatching target id 20007; \
         got: {stderr}"
    );
}

// ── AC-008 (BC-8.2.004 postcondition — unknown/ambiguous target) ─────────────

/// AC-008: `--move-to BadName` (zero matches) and `--move-to Amb` (2+
/// matches) both → exit 64 via §8.4's BC-8.4.002/003 messages, ZERO `DELETE`
/// calls (VP-COMPONENT-004).
#[tokio::test]
async fn test_bc_8_2_004_component_delete_move_to_unknown_ambiguous_zero_delete() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // ── Case A: unknown target (zero matches) ─────────────────────────────
    let server_a = MockServer::start().await;
    write_profile_config(config.path(), &server_a.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_a)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server_a)
        .await;

    let unknown = delete_cmd(&server_a.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "BadName",
        ])
        .output()
        .unwrap();

    server_a.verify().await;
    assert_eq!(
        unknown.status.code(),
        Some(64),
        "AC-008 Case A: expected exit 64 for unknown target; got {:?}\nstderr: {}",
        unknown.status.code(),
        String::from_utf8_lossy(&unknown.stderr)
    );
    let stderr_a = String::from_utf8_lossy(&unknown.stderr);
    assert!(
        stderr_a.contains("Component 'BadName' not found in project FOO. Available: Backend."),
        "AC-008 Case A: expected BC-8.4.002 verbatim message; got: {stderr_a}"
    );

    // ── Case B: ambiguous target (2+ matches) ─────────────────────────────
    let server_b = MockServer::start().await;
    write_profile_config(config.path(), &server_b.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
                component_response("10010", "AmbOne", None, None, None),
                component_response("10011", "AmbTwo", None, None, None),
            ])),
        )
        .mount(&server_b)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server_b)
        .await;

    let ambiguous = delete_cmd(&server_b.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "Amb",
        ])
        .output()
        .unwrap();

    server_b.verify().await;
    assert_eq!(
        ambiguous.status.code(),
        Some(64),
        "AC-008 Case B: expected exit 64 for ambiguous target; got {:?}\nstderr: {}",
        ambiguous.status.code(),
        String::from_utf8_lossy(&ambiguous.stderr)
    );
    let stderr_b = String::from_utf8_lossy(&ambiguous.stderr);
    assert!(
        stderr_b.contains("Ambiguous component 'Amb'. Matches: AmbOne, AmbTwo."),
        "AC-008 Case B: expected BC-8.4.003 verbatim message; got: {stderr_b}"
    );
}

// ── AC-009 (BC-8.2.005 postcondition / VP-COMPONENT-005) ─────────────────────

/// AC-009: `--move-to Backend` (same name given twice) and `--move-to 10001`
/// (mixed name/numeric self-reference, `Backend` IS id `10001`) both → exit
/// 64, zero `DELETE` calls. ID-equality catches both forms identically.
#[tokio::test]
async fn test_bc_8_2_005_component_delete_self_move_guard_name_and_numeric() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    const SELF_MOVE_MSG: &str = "--move-to target is the same component being deleted. \
        Choose a different component, or use --orphan.";

    // ── Case A: same name given twice ──────────────────────────────────────
    let server_a = MockServer::start().await;
    write_profile_config(config.path(), &server_a.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_a)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server_a)
        .await;

    let case_a = delete_cmd(&server_a.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "Backend",
        ])
        .output()
        .unwrap();

    server_a.verify().await;
    assert_eq!(
        case_a.status.code(),
        Some(64),
        "AC-009 Case A: expected exit 64 for same-name self-move; got {:?}\nstderr: {}",
        case_a.status.code(),
        String::from_utf8_lossy(&case_a.stderr)
    );
    let stderr_a = String::from_utf8_lossy(&case_a.stderr);
    assert!(
        stderr_a.contains(SELF_MOVE_MSG),
        "AC-009 Case A: expected verbatim self-move message; got: {stderr_a}"
    );

    // ── Case B: mixed name/numeric self-reference ──────────────────────────
    let server_b = MockServer::start().await;
    write_profile_config(config.path(), &server_b.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_b)
        .await;
    // Numeric --move-to target confirming GET: same id, same project.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_response_with_flags(
                "10001",
                "Backend",
                None,
                None,
                None,
                Some("FOO"),
                None,
            )),
        )
        .expect(1)
        .mount(&server_b)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server_b)
        .await;

    let case_b = delete_cmd(&server_b.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "10001",
        ])
        .output()
        .unwrap();

    server_b.verify().await;
    assert_eq!(
        case_b.status.code(),
        Some(64),
        "AC-009 Case B: expected exit 64 for mixed name/numeric self-move; \
         got {:?}\nstderr: {}",
        case_b.status.code(),
        String::from_utf8_lossy(&case_b.stderr)
    );
    let stderr_b = String::from_utf8_lossy(&case_b.stderr);
    assert!(
        stderr_b.contains(SELF_MOVE_MSG),
        "AC-009 Case B: expected verbatim self-move message (ID-equality \
         catches the mixed form too); got: {stderr_b}"
    );
}

// ── AC-010 (BC-8.2.002 M1 numeric-SOURCE confirmation — --move-to) ───────────

/// AC-010: `jr component delete 20007 --project A --move-to Frontend` where
/// `20007` actually belongs to project B → source-confirmation GET returns
/// `"project":"B"`, mismatching `--project A` → exit 64 pre-flight, ZERO HTTP
/// beyond the one confirming GET (no `--move-to` resolution GET, no `DELETE`).
#[tokio::test]
async fn test_bc_8_2_002_component_delete_numeric_source_project_mismatch_move_to() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/20007"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_response_with_flags(
                "20007",
                "Backend",
                None,
                None,
                None,
                Some("B"),
                None,
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    // No --move-to resolution GET for either project.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/A/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(component_list_response(vec![])))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/B/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(component_list_response(vec![])))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/20007"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "20007",
            "--project",
            "A",
            "--move-to",
            "Frontend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-010: expected exit 64 for numeric-source project mismatch; \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Component 20007 belongs to project B, not A."),
        "AC-010: expected BC-8.2.002 M1 verbatim mismatch message; got: {stderr}"
    );
}

// ── AC-011 (BC-8.2.002 M1, P4-broadened to --orphan) ──────────────────────────

/// AC-011: `jr component delete 20007 --project A --orphan --yes` where
/// `20007` belongs to project B → identical mismatch check fires under
/// `--orphan` too → exit 64 pre-flight, ZERO snapshot search, ZERO
/// confirmation prompt, ZERO `DELETE`.
#[tokio::test]
async fn test_bc_8_2_002_component_delete_numeric_source_project_mismatch_orphan() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/20007"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_response_with_flags(
                "20007",
                "Backend",
                None,
                None,
                None,
                Some("B"),
                None,
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/20007"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "20007",
            "--project",
            "A",
            "--orphan",
            "--yes",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-011: expected exit 64 for numeric-source project mismatch under \
         --orphan; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Component 20007 belongs to project B, not A."),
        "AC-011: expected BC-8.2.002 M1 verbatim mismatch message (shared \
         mechanism with --move-to, P4-broadened to --orphan); got: {stderr}"
    );
}

// ── ADV pass-1 LOW-1 (BC-8.2.002 M1 — numeric-source missing-project field) ──

/// ADV pass-1 LOW-1: a NUMERIC source whose confirming GET
/// (`GET /rest/api/3/component/{sourceId}`) returns a component body that
/// OMITS the `project` field entirely, under `--orphan --yes` with NO
/// `--project` supplied. `src/cli/component.rs::handle_delete`'s numeric-
/// source branch (~line 853) must fail closed with exit 64 BEFORE any
/// snapshot search or `DELETE` — previously uncovered, so a mutant dropping
/// the `return Err(...)` (falling through with an empty derived project key)
/// would compile and pass all existing tests. BC-8.2.002 M1 / ADR-0018 §1.
#[tokio::test]
async fn test_bc_8_2_002_component_delete_numeric_source_missing_project_field_fail_closed_orphan()
{
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Confirming GET: component exists but Jira returned no `"project"` key.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/20007"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_response_no_project_field("20007", "Backend")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/20007"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args(["component", "delete", "20007", "--orphan", "--yes"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "ADV pass-1 LOW-1 (orphan, no --project): expected exit 64 when the \
         numeric-source confirming GET omits the project field; got {:?}\n\
         stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "Component 20007 exists but Jira returned no project field. \
             Pass --project KEY to disambiguate."
        ),
        "ADV pass-1 LOW-1 (orphan, no --project): expected the BC-8.2.002 M1 \
         verbatim no-project-field message; got: {stderr}"
    );
}

/// ADV pass-1 LOW-1 (companion, `--project`-supplied sub-case): same NUMERIC
/// source + confirming GET omitting `project`, this time invoked with
/// `--project A --move-to Frontend`. `handle_delete`'s numeric-source branch
/// must fail closed with exit 64 BEFORE resolving `--move-to` (zero
/// target-resolution component-list GET), the snapshot search, or `DELETE`.
/// Covers the sibling `derived_project.is_empty() && project.is_some()`
/// arm the orphan/no-`--project` test above does not exercise.
#[tokio::test]
async fn test_bc_8_2_002_component_delete_numeric_source_missing_project_field_fail_closed_move_to()
{
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // Confirming GET: component exists but Jira returned no `"project"` key.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/20007"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_response_no_project_field("20007", "Backend")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // --move-to target resolution MUST NOT fire — the guard fires first.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/A/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(component_list_response(vec![])))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/20007"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "20007",
            "--project",
            "A",
            "--move-to",
            "Frontend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "ADV pass-1 LOW-1 (move-to, --project supplied): expected exit 64 \
         when the numeric-source confirming GET omits the project field; \
         got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "Component 20007 returned no project field; cannot verify --project \
             or scope the delete. The component's project could not be determined."
        ),
        "ADV pass-1 LOW-1 (move-to, --project supplied): expected the \
         BC-8.2.002 M1 verbatim no-project-field message; got: {stderr}"
    );
}

// ── AC-012 (BC-8.2.006 Postconditions — interactive) ──────────────────────────

/// AC-012: `--orphan` on a TTY (no `--yes`) → `dialoguer`-style confirm
/// prompt names the component and the snapshot-derived affected-issue count;
/// decline/Enter default → exit 0, ZERO `DELETE`; confirm → proceeds to
/// `DELETE` (VP-COMPONENT-007).
#[tokio::test]
async fn test_bc_8_2_006_component_delete_orphan_interactive_prompt_decline_and_confirm() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    const PROMPT: &str =
        "Delete component 'Backend' and remove it from 2 issue(s)? This cannot be undone.";

    // ── Decline ─────────────────────────────────────────────────────────────
    let server_decline = MockServer::start().await;
    write_profile_config(config.path(), &server_decline.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_decline)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1", "FOO-2"], None)),
        )
        .expect(1)
        .mount(&server_decline)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server_decline)
        .await;

    let decline = delete_cmd(&server_decline.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
        ])
        .write_stdin("N\n")
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();

    server_decline.verify().await;
    assert_eq!(
        decline.status.code(),
        Some(0),
        "AC-012 decline: expected exit 0; got {:?}\nstderr: {}",
        decline.status.code(),
        String::from_utf8_lossy(&decline.stderr)
    );
    let stderr_decline = String::from_utf8_lossy(&decline.stderr);
    assert!(
        stderr_decline.contains(PROMPT),
        "AC-012 decline: expected verbatim BC-8.2.006 prompt naming the \
         component and the real count; got: {stderr_decline}"
    );

    // ── Confirm ─────────────────────────────────────────────────────────────
    let server_confirm = MockServer::start().await;
    write_profile_config(config.path(), &server_confirm.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_confirm)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1", "FOO-2"], None)),
        )
        .expect(1)
        .mount(&server_confirm)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .and(query_param_is_missing("moveIssuesTo"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server_confirm)
        .await;

    let confirm = delete_cmd(&server_confirm.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
        ])
        .write_stdin("y\n")
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();

    server_confirm.verify().await;
    assert_eq!(
        confirm.status.code(),
        Some(0),
        "AC-012 confirm: expected exit 0 after DELETE succeeds; got {:?}\nstderr: {}",
        confirm.status.code(),
        String::from_utf8_lossy(&confirm.stderr)
    );
}

// ── AC-013 (BC-8.2.006 Postconditions — non-interactive / VP-COMPONENT-006) ──

/// AC-013 / EC-8.2.006-4: non-interactive `--orphan` without `--yes` → exit
/// 64, message contains the REAL, snapshot-derived affected-issue count `<N>`
/// (7, not a placeholder), ZERO `DELETE`. `--yes` present → proceeds without
/// a prompt.
#[tokio::test]
async fn test_bc_8_2_006_component_delete_orphan_noninteractive_requires_yes_real_count() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // ── --yes absent: exit 64 with real count ──────────────────────────────
    let server_no_yes = MockServer::start().await;
    write_profile_config(config.path(), &server_no_yes.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_no_yes)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_delete_snapshot_page(
                &[
                    "FOO-1", "FOO-2", "FOO-3", "FOO-4", "FOO-5", "FOO-6", "FOO-7",
                ],
                None,
            )),
        )
        .expect(1)
        .mount(&server_no_yes)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server_no_yes)
        .await;

    let no_yes = delete_cmd(&server_no_yes.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
        ])
        .output()
        .unwrap();

    server_no_yes.verify().await;
    assert_eq!(
        no_yes.status.code(),
        Some(64),
        "AC-013: expected exit 64 for non-interactive --orphan without --yes; \
         got {:?}\nstderr: {}",
        no_yes.status.code(),
        String::from_utf8_lossy(&no_yes.stderr)
    );
    let stderr = String::from_utf8_lossy(&no_yes.stderr);
    assert!(
        stderr.contains(
            "--orphan requires --yes when running non-interactively. This permanently \
             removes the component from 7 issue(s) with no replacement."
        ),
        "AC-013: expected BC-8.2.006 verbatim non-interactive message with \
         the REAL count 7 (not a placeholder); got: {stderr}"
    );

    // ── --yes present: proceeds without a prompt ────────────────────────────
    let server_yes = MockServer::start().await;
    write_profile_config(config.path(), &server_yes.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_yes)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1"], None)),
        )
        .expect(1)
        .mount(&server_yes)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .and(query_param_is_missing("moveIssuesTo"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server_yes)
        .await;

    let yes = delete_cmd(&server_yes.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
            "--yes",
        ])
        .output()
        .unwrap();

    server_yes.verify().await;
    assert_eq!(
        yes.status.code(),
        Some(0),
        "AC-013: --yes present must proceed directly (exit 0); got {:?}\nstderr: {}",
        yes.status.code(),
        String::from_utf8_lossy(&yes.stderr)
    );
}

// ── AC-014 (BC-8.2.006 Invariant 1) ───────────────────────────────────────────

/// AC-014: `--move-to` NEVER shows a confirmation prompt or requires `--yes`,
/// regardless of TTY state. Empty stdin on a TTY is fed; if a prompt DID
/// attempt to read stdin it would hit EOF → `JrError::Interrupted` (exit
/// 130) — exit 0 here proves no prompt was ever shown.
#[tokio::test]
async fn test_bc_8_2_006_component_delete_move_to_never_prompts() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
                component_response("10002", "Frontend", None, None, None),
            ])),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1"], None)),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .and(query_param("moveIssuesTo", "10002"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "Frontend",
        ])
        .write_stdin("")
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-014: --move-to must never prompt — exit 0 (not 130/EOF-Interrupted) \
         proves no stdin read was attempted; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-015 (BC-8.2.006 Edge Case EC-8.2.006-2) ────────────────────────────────

/// AC-015: `--orphan` on a component with ZERO affected issues → the
/// non-interactive `--yes`-absent message STILL fires, showing `0 issue(s)`
/// — deleting the component itself is still permanent regardless of current
/// usage.
#[tokio::test]
async fn test_bc_8_2_006_component_delete_orphan_zero_affected_issues_still_prompts() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_delete_snapshot_page(&[], None)),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-015: expected exit 64 even with zero affected issues; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "--orphan requires --yes when running non-interactively. This permanently \
             removes the component from 0 issue(s) with no replacement."
        ),
        "AC-015: expected BC-8.2.006 verbatim message showing '0 issue(s)'; \
         got: {stderr}"
    );
}

// ── AC-016 (BC-8.2.007 Postcondition 1 — firing boundary) ────────────────────

/// AC-016: the snapshot search fires exactly once for a chosen, guard-cleared
/// disposition and does NOT fire in (a) the no-disposition exit-64 path, (b)
/// an unknown `--move-to` target, or (c) a self-reference — any pre-flight
/// exit-64 path before a disposition is confirmed.
#[tokio::test]
async fn test_bc_8_2_007_component_delete_snapshot_fires_only_after_disposition_cleared() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // ── (a) no-disposition path ─────────────────────────────────────────────
    let server_a = MockServer::start().await;
    write_profile_config(config.path(), &server_a.uri());
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_a)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server_a)
        .await;

    let no_disposition = delete_cmd(&server_a.uri(), cache.path(), config.path())
        .args(["component", "delete", "Backend", "--project", "FOO"])
        .output()
        .unwrap();
    server_a.verify().await;
    assert_eq!(no_disposition.status.code(), Some(64));

    // ── (b) unknown --move-to target ────────────────────────────────────────
    let server_b = MockServer::start().await;
    write_profile_config(config.path(), &server_b.uri());
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_b)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server_b)
        .await;

    let unknown_target = delete_cmd(&server_b.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "BadName",
        ])
        .output()
        .unwrap();
    server_b.verify().await;
    assert_eq!(unknown_target.status.code(), Some(64));

    // ── (c) self-reference ───────────────────────────────────────────────────
    let server_c = MockServer::start().await;
    write_profile_config(config.path(), &server_c.uri());
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_c)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server_c)
        .await;

    let self_ref = delete_cmd(&server_c.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "Backend",
        ])
        .output()
        .unwrap();
    server_c.verify().await;
    assert_eq!(
        self_ref.status.code(),
        Some(64),
        "AC-016 (c): expected exit 64 for self-reference before snapshot; \
         got {:?}\nstderr: {}",
        self_ref.status.code(),
        String::from_utf8_lossy(&self_ref.stderr)
    );
}

// ── AC-017 (BC-8.2.007 Postcondition 4 — JQL clause shape) ───────────────────

/// AC-017: the composed snapshot JQL is ALWAYS `component = <resolvedId>
/// ORDER BY key ASC` — a fixture with two projects sharing a same-named
/// component asserts the snapshot body contains the resolved NUMERIC id,
/// never the shared name string. Asserted via EXACT `serde_json::Value`
/// equality on the parsed request body (not a wiremock partial match alone),
/// so a mutant swapping the id for the name string is caught even if the
/// mock's own matcher were loosened.
#[tokio::test]
async fn test_bc_8_2_007_component_delete_snapshot_jql_uses_resolved_id_not_name() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    // FOO's "Backend" is id 10001; BAR ALSO has a "Backend" (id 20007) —
    // proves the snapshot JQL cannot be using the bare name string, which
    // would be ambiguous across the two same-named components.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1"], None)),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
            "--yes",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-017: expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let received = server.received_requests().await.unwrap();
    let search_req = received
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/search/jql")
        .expect("AC-017: snapshot search must have fired");
    let body: Value = serde_json::from_slice(&search_req.body).expect("body must be valid JSON");
    assert_eq!(
        body["jql"],
        json!("component = 10001 ORDER BY key ASC"),
        "AC-017: snapshot JQL must be EXACTLY 'component = 10001 ORDER BY \
         key ASC' — the resolved numeric id, never the shared name \
         'Backend'; got jql: {:?}",
        body["jql"]
    );
}

// ── AC-018 (BC-8.2.007 Postcondition 5 — full pagination) ────────────────────

/// AC-018: a wiremock fixture returning ≥2 pages via `nextPageToken` → every
/// page is fetched; `affectedIssueCount`/`affectedIssues` reflect the FULL
/// multi-page result (3 keys), not just page one (which alone would be 2).
#[tokio::test]
async fn test_bc_8_2_007_component_delete_snapshot_paginates_to_completion() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server)
        .await;

    // Page 1: no nextPageToken in the REQUEST body (initial fetch).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(
            json!({"jql": "component = 10001 ORDER BY key ASC"}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_delete_snapshot_page(
                &["FOO-1", "FOO-2"],
                Some("cursor-2"),
            )),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    // Page 2: request body carries nextPageToken "cursor-2" — terminal.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(json!({"nextPageToken": "cursor-2"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-3"], None)),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .and(query_param_is_missing("moveIssuesTo"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
            "--yes",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-018: expected exit 0; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("AC-018: stdout must be valid JSON: {e}\nstdout: {stdout}"));
    assert_eq!(
        parsed["affectedIssueCount"],
        json!(3),
        "AC-018: affectedIssueCount must reflect the FULL multi-page result \
         (3), not just page one (2); got: {parsed}"
    );
    assert_eq!(
        parsed["affectedIssues"],
        json!(["FOO-1", "FOO-2", "FOO-3"]),
        "AC-018: affectedIssues must contain all keys across both pages in \
         order; got: {parsed}"
    );
}

// ── AC-019 (BC-8.2.007 Postcondition 5 — fail-closed on drift/error) ─────────

/// AC-019: a fixture simulating the JRACLOUD-95368 anti-loop drift condition
/// (`has_more=true` partial return) → `.expect(0)` on `DELETE`, process
/// exits 1, stderr contains "could not reliably enumerate affected issues —
/// aborting delete" (VP-COMPONENT-017). A genuine snapshot-search 5xx
/// failure produces the same fail-closed outcome (zero DELETE).
#[tokio::test]
async fn test_bc_8_2_007_component_delete_snapshot_drift_and_fetch_error_fail_closed() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // ── (a) JRACLOUD-95368 drift: repeated nextPageToken triggers the
    // anti-loop guard's has_more=true partial return. ──────────────────────
    let server_drift = MockServer::start().await;
    write_profile_config(config.path(), &server_drift.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_drift)
        .await;
    // Both pages return the SAME nextPageToken "loop" — the anti-loop guard
    // fires on the second hit before a third request is ever made.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1"], Some("loop"))),
        )
        .expect(2)
        .mount(&server_drift)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server_drift)
        .await;

    let drift = delete_cmd(&server_drift.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
            "--yes",
        ])
        .output()
        .unwrap();

    server_drift.verify().await;
    assert_eq!(
        drift.status.code(),
        Some(1),
        "AC-019 (a): drift-abort must exit 1 (JrError::SnapshotIncomplete), \
         NOT 64; got {:?}\nstderr: {}",
        drift.status.code(),
        String::from_utf8_lossy(&drift.stderr)
    );
    let stderr_drift = String::from_utf8_lossy(&drift.stderr);
    assert!(
        stderr_drift.contains("could not reliably enumerate affected issues — aborting delete"),
        "AC-019 (a): stderr must contain the verbatim fail-closed message; \
         got: {stderr_drift}"
    );

    // ── (b) genuine snapshot-search 5xx failure ─────────────────────────────
    let server_5xx = MockServer::start().await;
    write_profile_config(config.path(), &server_5xx.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_5xx)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server_5xx)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server_5xx)
        .await;

    let fetch_err = delete_cmd(&server_5xx.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
            "--yes",
        ])
        .output()
        .unwrap();

    server_5xx.verify().await;
    assert_eq!(
        fetch_err.status.code(),
        Some(1),
        "AC-019 (b): a genuine snapshot-search 5xx must abort before DELETE \
         with exit 1 (JrError::ApiError), for parity with sub-case (a)'s \
         drift-abort exit code; got {:?}\nstderr: {}",
        fetch_err.status.code(),
        String::from_utf8_lossy(&fetch_err.stderr)
    );
}

// ── AC-020 (BC-8.2.008 Behavior — success shape) ──────────────────────────────

/// AC-020: on success, `--output json` returns EXACTLY `{"deleted",
/// "movedIssuesTo", "affectedIssueCount", "affectedIssues"}` (verbatim key
/// set, BTreeSet comparison) matching the snapshot, for both the `--move-to`
/// (`movedIssuesTo` = target id string) and `--orphan` (`movedIssuesTo` =
/// JSON null) shapes. Table mode echoes a one-line confirmation naming the
/// disposition and count (no BC-specified exact string for this line, so
/// only content — not verbatim wording — is asserted).
#[tokio::test]
async fn test_bc_8_2_008_component_delete_success_json_shape_matches_snapshot() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // ── --move-to shape: movedIssuesTo is the target id string ─────────────
    let server_move = MockServer::start().await;
    write_profile_config(config.path(), &server_move.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
                component_response("10002", "Frontend", None, None, None),
            ])),
        )
        .mount(&server_move)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1", "FOO-2"], None)),
        )
        .expect(1)
        .mount(&server_move)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .and(query_param("moveIssuesTo", "10002"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server_move)
        .await;

    let move_out = delete_cmd(&server_move.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "Frontend",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    server_move.verify().await;
    assert_eq!(
        move_out.status.code(),
        Some(0),
        "AC-020 move-to: expected exit 0; got {:?}\nstderr: {}",
        move_out.status.code(),
        String::from_utf8_lossy(&move_out.stderr)
    );
    let stdout_move = String::from_utf8_lossy(&move_out.stdout);
    let parsed_move: Value = serde_json::from_str(&stdout_move).unwrap_or_else(|e| {
        panic!("AC-020 move-to: stdout must be valid JSON: {e}\nstdout: {stdout_move}")
    });
    let keys_move: BTreeSet<&str> = parsed_move
        .as_object()
        .expect("AC-020 move-to: stdout must be a JSON object")
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        keys_move,
        BTreeSet::from([
            "deleted",
            "movedIssuesTo",
            "affectedIssueCount",
            "affectedIssues"
        ]),
        "AC-020 move-to: top-level key set must be EXACTLY \
         {{deleted, movedIssuesTo, affectedIssueCount, affectedIssues}}; got: {keys_move:?}"
    );
    assert_eq!(parsed_move["deleted"], json!("10001"));
    assert_eq!(parsed_move["movedIssuesTo"], json!("10002"));
    assert_eq!(parsed_move["affectedIssueCount"], json!(2));
    assert_eq!(parsed_move["affectedIssues"], json!(["FOO-1", "FOO-2"]));

    // ── --orphan shape: movedIssuesTo is JSON null ──────────────────────────
    let server_orphan = MockServer::start().await;
    write_profile_config(config.path(), &server_orphan.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_orphan)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1"], None)),
        )
        .expect(1)
        .mount(&server_orphan)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .and(query_param_is_missing("moveIssuesTo"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server_orphan)
        .await;

    let orphan_out = delete_cmd(&server_orphan.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
            "--yes",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    server_orphan.verify().await;
    let stdout_orphan = String::from_utf8_lossy(&orphan_out.stdout);
    let parsed_orphan: Value = serde_json::from_str(&stdout_orphan).unwrap_or_else(|e| {
        panic!("AC-020 orphan: stdout must be valid JSON: {e}\nstdout: {stdout_orphan}")
    });
    assert_eq!(
        parsed_orphan["movedIssuesTo"],
        Value::Null,
        "AC-020 orphan: movedIssuesTo must be JSON null (no --move-to \
         target); got: {parsed_orphan}"
    );
    assert_eq!(parsed_orphan["deleted"], json!("10001"));
    assert_eq!(parsed_orphan["affectedIssueCount"], json!(1));

    // ── Table mode: one-line confirmation naming disposition and count ─────
    let server_table = MockServer::start().await;
    write_profile_config(config.path(), &server_table.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_table)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1"], None)),
        )
        .expect(1)
        .mount(&server_table)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server_table)
        .await;

    let table_out = delete_cmd(&server_table.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
            "--yes",
        ])
        .output()
        .unwrap();

    server_table.verify().await;
    assert_eq!(table_out.status.code(), Some(0));
    let stderr_table = String::from_utf8_lossy(&table_out.stderr);
    assert!(
        stderr_table.contains("Backend") && stderr_table.contains('1'),
        "AC-020 table mode: confirmation echo must name the component and \
         the affected-issue count; got: {stderr_table}"
    );
}

// ── AC-021 (BC-8.2.008 Idempotency — not-found vs race, VP-COMPONENT-024) ────

/// AC-021: SOURCE resolution returning not-found (BC-8.1.008) → ordinary
/// exit-64 not-found path, NEVER exit-0/idempotent-skip, ZERO `DELETE`
/// calls. A `DELETE` that itself races to 404 AFTER a successful resolution
/// → `ApiError(404)`, exit 1 — DISTINGUISHABLE by exit code from the
/// resolver-layer not-found. Both paths pinned in ONE test per
/// VP-COMPONENT-024's own "asserts the exit-code divergence" requirement.
#[tokio::test]
async fn test_bc_8_2_008_component_delete_resolver_notfound_vs_delete_race_exit_code_divergence() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // ── Resolver-layer not-found: exit 64 ───────────────────────────────────
    let server_notfound = MockServer::start().await;
    write_profile_config(config.path(), &server_notfound.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_notfound)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server_notfound)
        .await;

    let notfound = delete_cmd(&server_notfound.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Ghost",
            "--project",
            "FOO",
            "--orphan",
            "--yes",
        ])
        .output()
        .unwrap();

    server_notfound.verify().await;
    assert_eq!(
        notfound.status.code(),
        Some(64),
        "AC-021 resolver-not-found: expected exit 64 (ordinary not-found), \
         NEVER exit 0/idempotent-skip; got {:?}\nstderr: {}",
        notfound.status.code(),
        String::from_utf8_lossy(&notfound.stderr)
    );

    // ── DELETE-layer race: 404 AFTER successful resolution → exit 1 ────────
    let server_race = MockServer::start().await;
    write_profile_config(config.path(), &server_race.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server_race)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1"], None)),
        )
        .expect(1)
        .mount(&server_race)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(json!({"errorMessages": ["Not found"]})),
        )
        .expect(1)
        .mount(&server_race)
        .await;

    let race = delete_cmd(&server_race.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
            "--yes",
        ])
        .output()
        .unwrap();

    server_race.verify().await;
    assert_eq!(
        race.status.code(),
        Some(1),
        "AC-021 delete-race: a DELETE 404 AFTER successful resolution must \
         exit 1 (ApiError), DISTINGUISHABLE from the resolver's exit 64 \
         above; got {:?}\nstderr: {}",
        race.status.code(),
        String::from_utf8_lossy(&race.stderr)
    );
    assert_ne!(
        notfound.status.code(),
        race.status.code(),
        "AC-021: the two 404 sources (resolver-layer vs DELETE-call-layer) \
         MUST NOT be collapsed into a single exit code"
    );
}

// ── AC-022 (BC-8.2.008 Edge Case EC-8.2.008-1) ────────────────────────────────

/// AC-022: `--move-to` target is deleted by a concurrent actor between
/// BC-8.2.002's resolution and the `DELETE` call → the `DELETE` itself 404s
/// on the `moveIssuesTo` id → `ApiError(404)`, exit 1 (a genuine race, not a
/// resolver-layer not-found).
#[tokio::test]
async fn test_bc_8_2_008_component_delete_move_to_target_race_404_exits_1() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
                component_response("10002", "Frontend", None, None, None),
            ])),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1"], None)),
        )
        .expect(1)
        .mount(&server)
        .await;
    // moveIssuesTo target was deleted concurrently — the DELETE itself 404s.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .and(query_param("moveIssuesTo", "10002"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(json!({"errorMessages": ["Not found"]})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--move-to",
            "Frontend",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(1),
        "AC-022: a moveIssuesTo-target race 404 on DELETE must exit 1 \
         (ApiError), NOT 64 (resolver-layer not-found); got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// =============================================================================
// Expanded Edge Case coverage (story S-604-3: "Additional ECs to cover in
// test-writer's expanded suite... trace to the parent BC directly").
//
// EC-8.2.001-1/2 are already covered verbatim by AC-001/AC-002 above (the
// story's own worked examples for those ECs ARE those ACs' command shapes —
// see the doc comments on those two tests). EC-8.2.006-4 (real-count message
// text) is covered by AC-013 (uses the story's own "7 issues" example
// verbatim). EC-8.2.006-5 (numeric-source --orphan mismatch) is covered by
// AC-011. The two facets below are NOT otherwise covered by any AC test.
// =============================================================================

/// EC-8.2.006-1: `--orphan --yes` on a TTY → no prompt shown even though
/// stdin IS a terminal; proceeds directly. Empty stdin proves no prompt read
/// was attempted (would otherwise EOF → exit 130).
#[tokio::test]
async fn test_ec_8_2_006_1_component_delete_orphan_yes_bypasses_prompt_on_tty() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1"], None)),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .and(query_param_is_missing("moveIssuesTo"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
            "--yes",
        ])
        .write_stdin("")
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(0),
        "EC-8.2.006-1: --yes on a TTY must bypass the prompt entirely \
         (exit 0, not 130/EOF-Interrupted); got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// EC-8.2.006-3: `--no-input` + `--orphan` (no `--yes`) → exit 64, the SAME
/// message as plain non-interactive without `--yes` — `--no-input` and
/// "stdin is not a TTY" are treated identically. Set on a TTY seam to prove
/// `--no-input` alone (not merely the auto-no-input piped-stdin flip)
/// enforces the gate.
#[tokio::test]
async fn test_ec_8_2_006_3_component_delete_orphan_no_input_flag_parity() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    write_profile_config(config.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(component_list_response(vec![
                component_response("10001", "Backend", None, None, None),
            ])),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(component_delete_snapshot_page(&["FOO-1", "FOO-2"], None)),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/component/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = delete_cmd(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "--no-input",
            "component",
            "delete",
            "Backend",
            "--project",
            "FOO",
            "--orphan",
        ])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "EC-8.2.006-3: --no-input must force the same exit-64 gate as a \
         non-TTY, even though JR_STDIN_IS_TTY=1 is set; got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "--orphan requires --yes when running non-interactively. This permanently \
             removes the component from 2 issue(s) with no replacement."
        ),
        "EC-8.2.006-3: expected the SAME verbatim non-interactive message \
         as the plain (non-TTY) case; got: {stderr}"
    );
}

// ── Security LOW-1 (CWE-116) — delete_component percent-encoding ────────────

/// Security LOW-1 (CWE-116, S-604-3 hardening): `delete_component` MUST
/// percent-encode both `component_id` (path segment) and `target_id` (query
/// value) via `urlencoding::encode`, matching the established convention in
/// `src/api/client.rs`, `src/api/jira/links.rs`, `src/api/jira/users.rs`, and
/// `src/api/jira/teams.rs`. Production ids are API-sourced numeric strings,
/// so encoding is a no-op there — this is defense-in-depth, not a live bug.
///
/// This is an API-layer test (`JiraClient::new_for_test` against a wiremock
/// `MockServer`, the pattern used in `tests/comment_crud_api.rs` and
/// `tests/issue_commands.rs`), not a CLI-level one, because it must inspect
/// the exact bytes on the wire.
///
/// Mutation-resistance note: a bare space does NOT distinguish
/// `urlencoding::encode` from no encoding at all — the `url` crate
/// percent-encodes a raw space to `%20` at parse time regardless (see
/// `tests/comment_crud_api.rs::test_delete_comment_encodes_key_with_space_in_url`'s
/// doc comment for the same finding). This test instead uses characters that
/// are STRUCTURALLY significant to URL parsing when left raw:
/// - `component_id = "10/25"` — an unencoded `/` is read as a path-segment
///   separator, so the request path splits into an extra segment
///   (`/rest/api/3/component/10/25`) instead of staying one segment
///   (`/rest/api/3/component/10%2F25`).
/// - `target_id = "20&x=1"` — an unencoded `&` is read as a query-parameter
///   delimiter, so `moveIssuesTo` decodes to `"20"` with a smuggled `x=1`
///   pair instead of the single pair `moveIssuesTo=20&x=1`.
///
/// If `urlencoding::encode` were removed from `delete_component`, the mock
/// below (which asserts on the exact encoded path and the exact decoded
/// query pair) would not match the resulting request, and this test would
/// fail as a genuine assertion failure — not merely miss a request count.
#[tokio::test]
async fn test_delete_component_percent_encodes_ids_in_url() {
    let server = MockServer::start().await;

    // Loose method-only matcher — assertions below inspect the received
    // request's raw URL directly, so we don't pre-encode the expected path
    // in the mock matcher itself (that would make the mock's own encoding
    // logic part of what's under test).
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client
        .delete_component("10/25", Some("20&x=1"))
        .await
        .unwrap();

    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1, "expected exactly 1 DELETE request");

    let url = &reqs[0].url;
    assert_eq!(
        url.path(),
        "/rest/api/3/component/10%2F25",
        "component_id's '/' must be percent-encoded to %2F so it stays one \
         path segment; got path: {}",
        url.path()
    );
    assert!(
        !url.path().contains("/10/25"),
        "component_id must NOT appear as raw, unencoded '/' splitting the \
         path into extra segments; got path: {}",
        url.path()
    );

    let target_pair = url
        .query_pairs()
        .find(|(k, _)| k == "moveIssuesTo")
        .map(|(_, v)| v.into_owned());
    assert_eq!(
        target_pair.as_deref(),
        Some("20&x=1"),
        "target_id's '&'/'=' must be percent-encoded so moveIssuesTo decodes \
         back to the full raw value \"20&x=1\", not truncated at the first \
         '&'; got query: {:?}",
        url.query()
    );
    assert!(
        !url.query_pairs().any(|(k, _)| k == "x"),
        "an unencoded '&' in target_id must not smuggle in an extra 'x' \
         query parameter; got query: {:?}",
        url.query()
    );
}

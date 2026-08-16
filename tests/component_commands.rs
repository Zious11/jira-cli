//! CLI-level integration tests for `jr component` commands (S-604-1).
//!
//! All tests in this file PASS — `handle_list` is fully implemented.
//!
//! BC anchors: BC-8.1.001, BC-8.1.002, BC-8.1.003, BC-8.1.004, BC-8.4.004
//! Story: S-604-1, GitHub issue #604

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::fixtures::{
    component_list_response, component_response, component_response_with_flags,
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

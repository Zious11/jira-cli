//! CLI-level integration tests for `jr component` commands (S-604-1 + S-604-2).
//!
//! S-604-1 tests (handle_list): all PASS — fully implemented.
//! S-604-2 tests (handle_create, handle_edit): all FAIL — todo!() stubs.
//!
//! BC anchors: BC-8.1.001–BC-8.1.008, BC-8.4.002–BC-8.4.004
//! Stories: S-604-1 (list), S-604-2 (create/edit)

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::fixtures::{
    component_create_response, component_edit_response, component_list_response,
    component_response, component_response_with_flags, multi_project_user_search_response,
    multi_project_user_search_response_with_email, related_issue_counts_response,
    write_profile_config,
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
// S-604-2: component create / edit tests (all FAIL — Red Gate)
// Handlers are todo!() stubs; all tests below MUST fail until implemented.
// Exception: AC-005 (clap ValueEnum rejection) legitimately PASSES against
// stubs because clap rejects --assignee-type BOGUS before the handler runs.
// ══════════════════════════════════════════════════════════════════════════════

// ── AC-001 (BC-8.1.005 — minimal create body via body_json matcher) ───────────

/// AC-001 / BC-8.1.005: `jr component create --project FOO Backend` (no
/// optional flags) POSTs exactly `{"name":"Backend","project":"FOO"}`.
/// Verified via wiremock `body_json` matcher — absent optional keys must NOT
/// appear in the body (VP-COMPONENT-022, omit-if-absent invariant).
/// Red Gate: todo!() handler panics → exit ≠ 0 → assertion fails.
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
/// Red Gate: todo!() handler panics before any HTTP.
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
            "component-lead",
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
/// Red Gate: todo!() panics before HTTP.
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
/// Red Gate: todo!() panics → exit ≠ 0.
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
/// NOTE: This test LEGITIMATELY PASSES against todo!() stubs.  clap validates
/// the enum before dispatching to the handler, so the todo!() code is never
/// reached.  This is intentional and correct behavior; the test is included
/// as a compile + regression guard.
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

// ── AC-006 (BC-8.1.006 — empty --lead on create → exit 64, zero POST) ────────

/// AC-006 / BC-8.1.006: `--lead ""` on `component create` exits 64 with a
/// descriptive error message (app-level guard, not clap).  Zero POST calls.
/// Message must contain the exact substring
/// `"--lead \"\" has no effect on create"`.
/// Red Gate: todo!() panics before the guard fires → exit ≠ 64.
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
/// Red Gate: todo!() panics before lead resolution fires.
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
/// Red Gate: todo!() panics before HTTP.
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
/// Part B: table mode emits `  name \u{2192} New Name` on stderr (BC-3.4.012 field-echo).
///
/// Red Gate: todo!() panics before HTTP.
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
/// Red Gate: todo!() panics before HTTP.
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
/// Red Gate: todo!() panics → exit ≠ 64.
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
}

// ── AC-011 (BC-8.1.007 P16 — numeric input, no fields → exit 64, zero HTTP) ──

/// AC-011 / BC-8.1.007 Precondition 1 (P16 fix-burst ordering): when the input
/// is a NUMERIC component ID and no edit fields are supplied, exit 64 fires
/// BEFORE the confirming GET — not after.  This is the critical ordering
/// invariant that P16 enforces: no-fields guard > confirming GET.
/// Red Gate: todo!() panics → exit ≠ 64.
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
/// Red Gate: todo!() panics before HTTP.
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
    // Pin the BC-8.1.007 M1 verbatim message produced by component.rs:445-449.
    // A mutation deleting the project-check branch, or changing the format string,
    // would fail here (loose contains("FOO") && contains("WRONG") would survive
    // such mutations).
    assert!(
        stderr.contains("Component 10001 belongs to project FOO, not WRONG."),
        "Expected BC-8.1.007 M1 verbatim message \
         \"Component 10001 belongs to project FOO, not WRONG.\"; got: {stderr}"
    );
}

// ── AC-014 (BC-8.1.008 — numeric not-found message variants) ─────────────────

/// AC-014 / BC-8.1.008: 404 on the confirming GET produces two message variants:
/// (A) with `--project`: "Component '99999' not found in project FOO."
/// (B) without `--project`: "Component '99999' not found." (project-less)
/// Red Gate: todo!() panics before HTTP.
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
///
/// Red Gate: todo!() panics before HTTP.
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
/// Red Gate: todo!() panics before HTTP.
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
/// Red Gate: todo!() panics before HTTP.
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
/// Red Gate for numeric case: todo!() panics.  Name case exits 64 (passes).
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

    // Red Gate: todo!() panics → exit ≠ 0.  With implementation: should be exit 0.
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
/// entry for the project is absent.  Red Gate: todo!() panics → exit ≠ 0.
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

    // Red Gate: todo!() panics → exit ≠ 0.
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
/// Red Gate: todo!() panics or unconditional-invalidation mutation would wipe
/// the cache even on failure.
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
/// Exercises `src/cli/component.rs` lines 532-535 (0-match arm → UserError).
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
/// Exercises `src/cli/component.rs` lines 543-553 (2+-match arm → UserError with candidate list).
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

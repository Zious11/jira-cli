//! CLI-level integration tests for `jr component` commands (S-604-1).
//!
//! RED GATE: all tests in this file FAIL because `handle_list` contains
//! `todo!()` — the spawned subprocess exits 101 (Rust panic) instead of the
//! expected exit codes and output.
//!
//! BC anchors: BC-8.1.001, BC-8.1.002, BC-8.1.003, BC-8.1.004, BC-8.4.004
//! Story: S-604-1, GitHub issue #604

mod common;

use assert_cmd::Command;
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::fixtures::{
    component_list_response, component_response, related_issue_counts_response,
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
        .respond_with(
            ResponseTemplate::new(200).set_body_json(related_issue_counts_response("10001", 7)),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/component/10002/relatedIssueCounts"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(related_issue_counts_response("10002", 3)),
        )
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
        .respond_with(
            ResponseTemplate::new(200).set_body_json(related_issue_counts_response("10001", 5)),
        )
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

/// Integration and unit tests for BC-4.3.001: multi-workspace asset HashMap
/// composite key correctness (H-036 holdout).
///
/// AC-001 (MUST-FAIL pre-fix): two issues in different workspaces share the
///   same oid string ("88").  The buggy `resolved: HashMap<String, _>` lets
///   the second workspace overwrite the first.  The test asserts BOTH labels
///   appear in the JSON output; pre-fix it fails because only one survives.
///
/// AC-002 (regression guard, MUST-PASS always): single-workspace tenant
///   still resolves the asset label correctly after the fix.
///
/// AC-003 (implicit): the `to_enrich` HashMap (line 398, already composite)
///   is exercised via AC-001.  The fix scope is line 446 only; AC-001 catches
///   any regression to `to_enrich`.
#[allow(dead_code)]
mod common;

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Return a fields response that advertises one CMDB custom field.
fn fields_with_cmdb() -> serde_json::Value {
    json!([
        {
            "id": "summary",
            "name": "Summary",
            "custom": false,
            "schema": { "type": "string" }
        },
        {
            "id": "customfield_10191",
            "name": "Client Asset",
            "custom": true,
            "schema": {
                "type": "any",
                "custom": "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
                "customId": 10191
            }
        }
    ])
}

/// Build a minimal issue JSON whose CMDB field carries only
/// `workspaceId` + `objectId` (no pre-resolved label/key).
/// This triggers the enrichment path in `handle_list`.
fn issue_with_raw_cmdb(key: &str, workspace_id: &str, object_id: &str) -> serde_json::Value {
    json!({
        "key": key,
        "fields": {
            "summary": format!("Issue {key}"),
            "status": { "name": "To Do" },
            "issuetype": { "name": "Task" },
            "priority": { "name": "Medium" },
            "assignee": null,
            "project": { "key": key.split('-').next().unwrap_or("PROJ") },
            "customfield_10191": [
                {
                    "workspaceId": workspace_id,
                    "objectId": object_id
                }
            ]
        }
    })
}

/// Build a minimal asset GET response for `GET /jsm/assets/workspace/<wid>/v1/object/<oid>`.
fn asset_response(object_key: &str, label: &str) -> serde_json::Value {
    json!({
        "id": "88",
        "label": label,
        "objectKey": object_key,
        "objectType": { "id": "13", "name": "Client" }
    })
}

// ── AC-001: multi-workspace collision (FAILS pre-fix, PASSES post-fix) ──────

/// test_bc_4_3_001_multi_workspace_no_collision
///
/// Two issues from different workspaces (ws-A and ws-B) both reference an
/// asset with objectId "88", but each workspace maps "88" to a different
/// name: "Acme Corp" and "Widgets Inc".
///
/// Pre-fix (buggy): `resolved: HashMap<String, _>` keys on bare `oid`.
/// The second insert for "88" overwrites the first, so the JSON output
/// contains only ONE of the two labels.  The assertion that BOTH are present
/// fails → Red Gate confirmed.
///
/// Post-fix: `resolved: HashMap<(String, String), _>` preserves both entries
/// → both labels appear in the output → test passes.
#[tokio::test]
async fn test_bc_4_3_001_multi_workspace_no_collision() {
    let server = MockServer::start().await;

    // Project existence check (`project_exists` calls GET /rest/api/3/project/PROJ).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "key": "PROJ",
            "name": "Test Project"
        })))
        .mount(&server)
        .await;

    // Fields discovery: advertise one CMDB field.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fields_with_cmdb()))
        .mount(&server)
        .await;

    // Issue search: return PROJ-1 (ws-A/88) and PROJ-2 (ws-B/88).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [
                issue_with_raw_cmdb("PROJ-1", "ws-A", "88"),
                issue_with_raw_cmdb("PROJ-2", "ws-B", "88")
            ],
            "nextPageToken": null
        })))
        .mount(&server)
        .await;

    // Asset resolution for ws-A / object 88 → "Acme Corp"
    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-A/v1/object/88"))
        .and(query_param("includeAttributes", "false"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(asset_response("WS-A-88", "Acme Corp")),
        )
        .mount(&server)
        .await;

    // Asset resolution for ws-B / object 88 → "Widgets Inc"
    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-B/v1/object/88"))
        .and(query_param("includeAttributes", "false"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(asset_response("WS-B-88", "Widgets Inc")),
        )
        .mount(&server)
        .await;

    // Workspace discovery endpoint (fallback; the assets already carry wids,
    // so this may not be called — mount a 500 to surface unintended calls).
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "message": "unexpected workspace discovery call in multi-wid test"
        })))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .current_dir(project_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "list",
            "--assets",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr: {stderr}\nstdout: {stdout}"
    );

    // Both workspace-specific labels MUST appear.
    // Pre-fix: last-write-wins means only one label survives in `resolved`
    // (the map key is bare "88", so the second insert clobbers the first).
    // This assertion fails pre-fix, confirming the Red Gate.
    assert!(
        stdout.contains("Acme Corp"),
        "PROJ-1 (ws-A/88) label 'Acme Corp' missing from output.\n\
         Pre-fix last-write-wins: only the second workspace's label survives.\n\
         stdout: {stdout}"
    );
    assert!(
        stdout.contains("Widgets Inc"),
        "PROJ-2 (ws-B/88) label 'Widgets Inc' missing from output.\n\
         Pre-fix last-write-wins: only the last-inserted label survives.\n\
         stdout: {stdout}"
    );
}

// ── AC-002: single-workspace regression guard (PASSES pre-fix and post-fix) ──

/// test_bc_4_3_001_single_workspace_regression_guard
///
/// One workspace, one issue, one asset.  The fix must not break this
/// common-case path.  This test is expected to PASS on the current (buggy)
/// branch and must continue to PASS after the fix is applied.
#[tokio::test]
async fn test_bc_4_3_001_single_workspace_regression_guard() {
    let server = MockServer::start().await;

    // Project existence check.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "key": "PROJ",
            "name": "Test Project"
        })))
        .mount(&server)
        .await;

    // Fields discovery
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fields_with_cmdb()))
        .mount(&server)
        .await;

    // Issue search: one issue in the single workspace
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [
                issue_with_raw_cmdb("PROJ-1", "ws-only", "1")
            ],
            "nextPageToken": null
        })))
        .mount(&server)
        .await;

    // Asset resolution for ws-only / object 1 → "Only Asset"
    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-only/v1/object/1"))
        .and(query_param("includeAttributes", "false"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(asset_response("WS-ONLY-1", "Only Asset")),
        )
        .mount(&server)
        .await;

    // Workspace discovery (may or may not be called; respond gracefully)
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 50, "isLastPage": true,
            "values": [{ "workspaceId": "ws-only" }]
        })))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .current_dir(project_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "list",
            "--assets",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr: {stderr}\nstdout: {stdout}"
    );

    assert!(
        stdout.contains("Only Asset"),
        "Single-workspace asset label 'Only Asset' missing.\nstdout: {stdout}"
    );
}

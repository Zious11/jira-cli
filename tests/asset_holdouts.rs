//! BC-4 assets/CMDB regression holdout suite — S-2.03
//!
//! Pins existing assets/CMDB behavior across three holdout scenarios. All tests
//! pass on current develop — no implementation changes required. Future
//! regressions in any of these paths will break this suite.
//!
//! Holdout coverage:
//! - AC-001 / H-037 / BC-4.2.001: workspace ID is cached after first discovery
//!   call — a second `jr assets search` invocation hits NO workspace HTTP call.
//! - AC-002 / H-038 / BC-4.3.002: `enrich_assets` skips assets that already have
//!   both `key` and `name` populated — only id-only assets are fetched via GET.
//! - AC-003 / H-039 / BC-4.2.006: `jr assets tickets OBJ-1 --status PROG` with
//!   two matching statuses ("In Progress", "Progressing") → exit 64,
//!   stderr contains "Ambiguous status" + both status names.
//!
//! H-038 placement: `tests/asset_holdouts.rs` (library-level test).
//! `enrich_assets` is declared `pub` in `src/api/assets/linked.rs`, so it is
//! accessible from integration tests without requiring an inline `#[cfg(test)]`
//! block in the production file.
//!
//! Infrastructure:
//! - H-037: Process-spawn via `assert_cmd::Command` with two consecutive
//!   `jr assets search` invocations sharing a temp `XDG_CACHE_HOME`.
//!   `expect(1)` on the workspace endpoint confirms exactly one HTTP call.
//! - H-038: Library-level test using `JiraClient::new_for_test` and wiremock.
//! - H-039: Process-spawn via `assert_cmd::Command` with `JR_BASE_URL` +
//!   `JR_AUTH_HEADER`.
//! - XDG isolation via `tempfile::TempDir` for every test.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use jr::api::assets::linked::enrich_assets;
use jr::api::client::JiraClient;
use jr::types::assets::LinkedAsset;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build a `jr` command with XDG isolation and mock-server wiring.
fn jr_cmd(server_uri: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("XDG_CONFIG_HOME", config_dir)
        .arg("--no-input");
    cmd
}

// ---------------------------------------------------------------------------
// AC-001 / H-037 / BC-4.2.001
// ---------------------------------------------------------------------------

/// BC-4.2.001 postcondition: the workspace ID is cached after the first
/// discovery call. A second `jr assets search` invocation within the 7-day TTL
/// window must not hit the workspace discovery endpoint.
///
/// Strategy A (preferred): mount the workspace endpoint with `expect(1)`. Spawn
/// `jr assets search "Key = X"` twice, both sharing the same temp `XDG_CACHE_HOME`.
/// Call `server.verify().await` after both invocations — confirms exactly one HTTP
/// call to the workspace discovery endpoint across two process-spawn invocations.
///
/// Both AQL search calls may or may not succeed (empty result is fine); the
/// assertion that matters is the wiremock `expect(1)` guard.
///
/// Pins `src/api/assets/workspace.rs::get_or_fetch_workspace_id` cache-hit branch.
/// Without this guard a future refactor that removes or bypasses the cache check
/// would fire two workspace discovery requests instead of one, burning an extra
/// API round-trip on every command.
#[tokio::test]
async fn test_s_2_03_h_037_bc_4_2_001_workspace_id_cached_after_first_call() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    // Workspace discovery endpoint — must be called exactly ONCE across both
    // `jr assets search` invocations. The second invocation must serve the
    // workspace ID from the cache written by the first.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "workspaceId": "ws-hold-037" }
            ]
        })))
        .expect(1) // exactly 1 call across BOTH invocations
        .mount(&server)
        .await;

    // AQL search endpoint — both invocations may call this; no constraint needed.
    // Return an empty result so `jr assets search` exits cleanly (exit 0 or a
    // "no results" message — either is acceptable; the workspace call count is
    // what the test asserts on).
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-hold-037/v1/object/aql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 0,
            "isLast": true,
            "values": []
        })))
        .mount(&server)
        .await;

    // First invocation: workspace endpoint is called once and result is cached
    // to `<XDG_CACHE_HOME>/jr/v1/default/workspace.json`.
    let output1 = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .current_dir(cwd.path())
        .args(["assets", "search", "Key = X"])
        .output()
        .unwrap();

    let stderr1 = String::from_utf8_lossy(&output1.stderr);
    // Both exit codes are acceptable: 0 (empty result table) or any non-zero
    // from a network-level error. The only thing we assert is that the workspace
    // endpoint was called exactly once total (checked after the second invocation).
    // However, if the first call failed before caching the workspace ID (e.g.,
    // due to a network error before the workspace write), the second call will
    // also hit the endpoint and the expect(1) will fail — which is correct.
    assert!(
        !stderr1.contains("workspace.json unreadable"),
        "first invocation must not see a corrupt workspace cache; stderr: {stderr1}"
    );

    // Second invocation: workspace endpoint must NOT be called — the cache
    // written by the first invocation is read from `workspace.json`.
    let output2 = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .current_dir(cwd.path())
        .args(["assets", "search", "Key = X"])
        .output()
        .unwrap();

    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    assert!(
        !stderr2.contains("workspace.json unreadable"),
        "second invocation must not see a corrupt workspace cache; stderr: {stderr2}"
    );

    // Verify exactly 1 workspace HTTP call occurred across both invocations.
    // wiremock raises an error if the `expect(1)` constraint is violated.
    server.verify().await;
}

// ---------------------------------------------------------------------------
// AC-002 / H-038 / BC-4.3.002
// ---------------------------------------------------------------------------

/// BC-4.3.002 postcondition: `enrich_assets` skips assets that already have both
/// `key` and `name` populated. Only assets with an `id` but no `key` or `name`
/// are enriched via GET.
///
/// Test setup:
/// - asset A: `id = "id-A"`, `key = None`, `name = None`  → must be fetched
/// - asset B: `id = "id-B"`, `key = Some("B-001")`, `name = Some("Asset B")` → must NOT be fetched
///
/// Wiremock constraint:
/// - GET asset object endpoint for asset A: `expect(1)` (fetched once)
/// - GET asset object endpoint for asset B: `expect(0)` (never fetched)
///
/// After enrichment:
/// - asset A has `key` and `name` populated from the API response
/// - asset B's `key` and `name` are unchanged (still "B-001" / "Asset B")
///
/// H-038 placement: this test is in `tests/asset_holdouts.rs` (not inline in
/// `src/api/assets/linked.rs`) because `enrich_assets` is declared `pub`.
///
/// Pins `src/api/assets/linked.rs::enrich_assets` skip-already-resolved branch.
/// Without this guard a future refactor that removes the `a.key.is_none() &&
/// a.name.is_none()` filter would make unnecessary GET requests for all assets,
/// burning extra API round-trips and potentially overwriting enriched data.
#[tokio::test]
async fn test_s_2_03_h_038_bc_4_3_002_enrich_assets_skips_already_resolved() {
    let server = MockServer::start().await;

    let workspace_id = "ws-hold-038";

    // GET asset A (id-only) — must be fetched exactly once.
    // The URL is: {base}/jsm/assets/workspace/{wid}/v1/object/{id}?includeAttributes=false
    Mock::given(method("GET"))
        .and(path(format!(
            "/jsm/assets/workspace/{workspace_id}/v1/object/id-A"
        )))
        .and(query_param("includeAttributes", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "id-A",
            "label": "Resolved Asset A",
            "objectKey": "A-001",
            "objectType": { "id": "13", "name": "Client" }
        })))
        .expect(1) // must be fetched exactly once
        .mount(&server)
        .await;

    // GET asset B (already resolved: key + name set) — must NOT be fetched.
    Mock::given(method("GET"))
        .and(path(format!(
            "/jsm/assets/workspace/{workspace_id}/v1/object/id-B"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "id-B",
            "label": "Asset B",
            "objectKey": "B-001",
            "objectType": { "id": "13", "name": "Client" }
        })))
        .expect(0) // must NEVER be fetched (key + name already present)
        .mount(&server)
        .await;

    // asset A: id only, must be enriched via HTTP GET
    // asset B: id + key + name already populated, must be skipped
    let mut assets = vec![
        LinkedAsset {
            id: Some("id-A".to_string()),
            workspace_id: Some(workspace_id.to_string()),
            key: None,
            name: None,
            asset_type: None,
        },
        LinkedAsset {
            id: Some("id-B".to_string()),
            workspace_id: Some(workspace_id.to_string()),
            key: Some("B-001".to_string()),
            name: Some("Asset B".to_string()),
            asset_type: None,
        },
    ];

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    enrich_assets(&client, &mut assets).await;

    // verify the wiremock expect constraints (1 call for A, 0 for B)
    server.verify().await;

    // asset A must now have key and name populated from the API response
    assert_eq!(
        assets[0].key.as_deref(),
        Some("A-001"),
        "BC-4.3.002: asset A must have key 'A-001' after enrichment; got: {:?}",
        assets[0].key
    );
    assert_eq!(
        assets[0].name.as_deref(),
        Some("Resolved Asset A"),
        "BC-4.3.002: asset A must have name 'Resolved Asset A' after enrichment; got: {:?}",
        assets[0].name
    );

    // asset B's key and name must be unchanged (skip-already-resolved invariant)
    assert_eq!(
        assets[1].key.as_deref(),
        Some("B-001"),
        "BC-4.3.002: asset B key must be unchanged; got: {:?}",
        assets[1].key
    );
    assert_eq!(
        assets[1].name.as_deref(),
        Some("Asset B"),
        "BC-4.3.002: asset B name must be unchanged; got: {:?}",
        assets[1].name
    );
}

// ---------------------------------------------------------------------------
// AC-003 / H-039 / BC-4.2.006
// ---------------------------------------------------------------------------

/// BC-4.2.006 postcondition: `jr assets tickets OBJ-1 --status PROG` with two
/// connected tickets whose statuses are "In Progress" and "Progressing" must:
///   - exit 64 (UserError)
///   - include "Ambiguous status" in stderr
///   - include "In Progress" in stderr
///   - include "Progressing" in stderr
///
/// The disambiguation logic in `filter_tickets` (src/cli/assets.rs) uses
/// `partial_match::partial_match` which routes single or multi-substring hits
/// through `MatchResult::Ambiguous`. Both "In Progress" and "Progressing"
/// contain the substring "PROG" (case-insensitive), so "PROG" is ambiguous.
///
/// Wiremock wiring for this process-spawn test:
/// 1. GET /rest/servicedeskapi/assets/workspace → workspace discovery
/// 2. POST /jsm/assets/workspace/{wid}/v1/object/aql → resolves "OBJ-1" to
///    its numeric ID (resolve_object_key: non-numeric key → AQL search)
/// 3. GET /jsm/assets/workspace/{wid}/v1/objectconnectedtickets/{id}/tickets
///    → returns two tickets with statuses "In Progress" and "Progressing"
///
/// Pins `src/cli/assets.rs::filter_tickets` Ambiguous branch.
/// Without this guard a future change that relaxes partial_match resolution
/// (e.g., auto-selecting the first match) would silently return incorrect
/// filtered results instead of rejecting ambiguous input.
#[tokio::test]
async fn test_s_2_03_h_039_bc_4_2_006_assets_tickets_ambiguous_status_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let workspace_id = "ws-hold-039";
    // Numeric object ID returned by AQL resolve for "OBJ-1"
    let object_id = "88001";

    // 1. Workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "workspaceId": workspace_id }
            ]
        })))
        .mount(&server)
        .await;

    // 2. resolve_object_key("OBJ-1"): AQL search to get numeric ID
    //    Path: POST /jsm/assets/workspace/{wid}/v1/object/aql?startAt=0&maxResults=1&includeAttributes=false
    Mock::given(method("POST"))
        .and(path(format!(
            "/jsm/assets/workspace/{workspace_id}/v1/object/aql"
        )))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "1"))
        .and(query_param("includeAttributes", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "startAt": 0,
            "maxResults": 1,
            "total": 1,
            "isLast": true,
            "values": [
                {
                    "id": object_id,
                    "label": "OBJ-1 Asset",
                    "objectKey": "OBJ-1",
                    "objectType": { "id": "13", "name": "Hardware" }
                }
            ]
        })))
        .mount(&server)
        .await;

    // 3. Connected tickets: two tickets with overlapping status substrings.
    //    Both "In Progress" and "Progressing" match "--status PROG"
    //    → partial_match returns Ambiguous → filter_tickets returns Err(UserError)
    Mock::given(method("GET"))
        .and(path(format!(
            "/jsm/assets/workspace/{workspace_id}/v1/objectconnectedtickets/{object_id}/tickets"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tickets": [
                {
                    "key": "HELP-1",
                    "id": "10001",
                    "title": "VPN access issue",
                    "reporter": null,
                    "created": null,
                    "updated": null,
                    "status": { "name": "In Progress", "colorName": "yellow" },
                    "type": { "name": "Service Request" },
                    "priority": { "name": "Medium" }
                },
                {
                    "key": "HELP-2",
                    "id": "10002",
                    "title": "Progressing ticket",
                    "reporter": null,
                    "created": null,
                    "updated": null,
                    "status": { "name": "Progressing", "colorName": "yellow" },
                    "type": { "name": "Service Request" },
                    "priority": { "name": "Low" }
                }
            ],
            "allTicketsQuery": null
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .current_dir(cwd.path())
        .args(["assets", "tickets", "OBJ-1", "--status", "PROG"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-4.2.006: ambiguous --status must exit 64 (UserError); got: {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Ambiguous status"),
        "BC-4.2.006: stderr must contain 'Ambiguous status'; got: {stderr}"
    );
    assert!(
        stderr.contains("In Progress"),
        "BC-4.2.006: stderr must contain 'In Progress' (first candidate); got: {stderr}"
    );
    assert!(
        stderr.contains("Progressing"),
        "BC-4.2.006: stderr must contain 'Progressing' (second candidate); got: {stderr}"
    );
}

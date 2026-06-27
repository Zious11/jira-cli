//! D2 warm-hit / no-HTTP integration tests for five of the nine cache families.
//!
//! BC anchor: BC-6.2.018 (warm cache hit returns cached value and issues ZERO HTTP
//! calls to the backing API endpoint — invariant holds for all nine cache families).
//! This file covers five families (1, 2, 4, 5, 7); the remaining four families
//! (3 workspace, 6 request_types, 8 request_type_fields, 9 fields) are tested
//! elsewhere or are candidates for future warm-hit coverage.
//!
//! Technique: wiremock `expect(1)` call-count pin — mount the backing endpoint with
//! `.expect(1)`, run the command twice sharing the same JR_CACHE_DIR temp dir, then
//! let the MockServer drop so wiremock enforces the call count. If the warm path
//! re-fetched, the endpoint would be hit twice and wiremock would panic on drop.
//!
//! Coverage:
//!   test_team_list_warm_cache_skips_http             — Family 1 (team list)
//!   test_resolutions_warm_cache_skips_http           — Family 4 (resolutions)
//!   test_project_meta_warm_cache_skips_http          — Family 2 (project_meta; bespoke inline reader)
//!   test_cmdb_fields_warm_cache_skips_http           — Family 5 (cmdb_fields; GET /rest/api/3/field)
//!   test_object_type_attrs_warm_cache_skips_http     — Family 7 (object_type_attrs; GET objecttype/.../attributes)

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::json;
use wiremock::matchers::{method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── Shared helper ─────────────────────────────────────────────────────────────

/// Write a minimal jr config to a temp config dir so the subprocess finds a URL.
/// Supports optional org_id (for the team-list test — avoids the GraphQL discovery call).
fn write_minimal_config_with_org(config_home: &std::path::Path, url: &str, org_id: Option<&str>) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    let mut content = format!("[profiles.default]\nurl = \"{url}\"\n");
    if let Some(oid) = org_id {
        content.push_str(&format!("org_id = \"{oid}\"\n"));
    }
    std::fs::write(dir.join("config.toml"), content).unwrap();
}

/// Build a `jr` Command with isolated XDG / JR_CACHE_DIR / JR_CONFIG_DIR dirs.
fn jr_cmd_isolated(
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

// ── Family 1 (team list) ──────────────────────────────────────────────────────

/// BC-6.2.018 (D2) — Family 1: warm team-list cache skips HTTP.
///
/// `jr team list` fetches org teams via `GET .../teams` on the first call (cold
/// miss) and writes a `teams.json` cache. On the second call (warm hit, same
/// JR_CACHE_DIR), the cache is fresh so the teams endpoint MUST NOT be hit again.
///
/// Non-tautology: would fail if warm path re-fetched — the teams endpoint would be
/// hit twice → expect(1) panics on drop.
///
/// org_id is pre-populated in the config so the GraphQL metadata call is skipped
/// (resolve_org_id early-returns from config, never calling get_org_metadata).
#[tokio::test]
async fn test_team_list_warm_cache_skips_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config_with_org(config_dir.path(), &server.uri(), Some("test-org-id-456"));

    // CRITICAL: expect(1) — teams endpoint must fire EXACTLY ONCE across BOTH invocations.
    Mock::given(method("GET"))
        .and(path_regex("/gateway/api/public/teams/v1/org/.*/teams"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "entities": [
                { "teamId": "team-uuid-alpha", "displayName": "Alpha Team" },
                { "teamId": "team-uuid-beta",  "displayName": "Beta Team" }
            ],
            "cursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Invocation 1: cold miss — populates teams.json cache.
    let out1 = jr_cmd_isolated(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["team", "list", "--no-input"])
        .output()
        .unwrap();

    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    assert!(
        out1.status.success(),
        "Invocation 1 expected exit 0; stderr: {stderr1}"
    );

    let stdout1 = String::from_utf8_lossy(&out1.stdout);
    assert!(
        stdout1.contains("Alpha Team"),
        "Invocation 1 must list teams; stdout: {stdout1}"
    );

    // Invocation 2: warm hit — must NOT call the teams endpoint again.
    let out2 = jr_cmd_isolated(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["team", "list", "--no-input"])
        .output()
        .unwrap();

    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(
        out2.status.success(),
        "Invocation 2 expected exit 0; stderr: {stderr2}"
    );

    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    assert!(
        stdout2.contains("Alpha Team"),
        "Invocation 2 must return same team data from cache; stdout: {stdout2}"
    );

    // MockServer drop automatically enforces expect(1):
    // if teams endpoint fired twice, wiremock panics here.
}

// ── Family 4 (resolutions) ────────────────────────────────────────────────────

/// BC-6.2.018 (D2) — Family 4: warm resolutions cache skips HTTP.
///
/// `jr issue resolutions` loads the resolution list via `GET /rest/api/3/resolution`
/// on the first call (cold miss) and writes `resolutions.json`. On the second call
/// (warm hit, same JR_CACHE_DIR), the cache is fresh and the resolutions endpoint
/// MUST NOT be hit again.
///
/// Non-tautology: would fail if warm path re-fetched — the resolutions endpoint
/// would be hit twice → expect(1) panics on drop.
#[tokio::test]
async fn test_resolutions_warm_cache_skips_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config_with_org(config_dir.path(), &server.uri(), None);

    // CRITICAL: expect(1) — resolutions endpoint must fire EXACTLY ONCE.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/resolution"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": "10000", "name": "Done",     "description": "Work is complete." },
            { "id": "10001", "name": "Won't Do", "description": "Will not fix." }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    // Invocation 1: cold miss — populates resolutions.json cache.
    let out1 = jr_cmd_isolated(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "resolutions", "--no-input", "--output", "json"])
        .output()
        .unwrap();

    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    assert!(
        out1.status.success(),
        "Invocation 1 expected exit 0; stderr: {stderr1}"
    );

    let parsed1: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&out1.stdout))
        .unwrap_or_else(|e| panic!("Invocation 1 expected valid JSON; error: {e}"));
    let arr1 = parsed1.as_array().expect("expected JSON array from inv 1");
    assert_eq!(arr1.len(), 2, "Invocation 1 must return 2 resolutions");

    // Invocation 2: warm hit — must NOT call the resolutions endpoint again.
    let out2 = jr_cmd_isolated(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "resolutions", "--no-input", "--output", "json"])
        .output()
        .unwrap();

    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(
        out2.status.success(),
        "Invocation 2 expected exit 0; stderr: {stderr2}"
    );

    let parsed2: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&out2.stdout))
        .unwrap_or_else(|e| panic!("Invocation 2 expected valid JSON; error: {e}"));
    let arr2 = parsed2.as_array().expect("expected JSON array from inv 2");
    // Count check kept separately: gives a faster, more specific count-mismatch
    // message than the full-value assert_eq! below (e.g., "got 0, expected 2").
    assert_eq!(
        arr1.len(),
        arr2.len(),
        "Both invocations must return same number of resolutions"
    );
    assert_eq!(
        parsed1, parsed2,
        "cache hit must return byte-identical JSON output across invocations"
    );

    // MockServer drop automatically enforces expect(1).
}

// ── Family 2 (project_meta — bespoke inline reader) ───────────────────────────

/// BC-6.2.018 (D2) — Family 2: warm project_meta cache skips HTTP.
///
/// `jr requesttype list --project HELP` calls `get_or_fetch_project_meta` which
/// checks `read_project_meta` (bespoke per-entry inline reader, NOT `read_cache<T>`).
/// On the first call (cold miss) it fetches `GET /rest/api/3/project/HELP` and the
/// service desk list, then writes `project_meta.json`. On the second call (warm hit),
/// `read_project_meta` returns the cached entry and the project endpoint is NEVER
/// called again.
///
/// Non-tautology: would fail if warm path re-fetched — the project endpoint would be
/// hit twice → expect(1) panics on drop.
///
/// Note: the service desk list and request-types endpoints are mounted WITHOUT
/// expect() — only the project endpoint carries expect(1) because that is the
/// canonical signal for project_meta cache activity.
#[tokio::test]
async fn test_project_meta_warm_cache_skips_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config_with_org(config_dir.path(), &server.uri(), None);

    // CRITICAL: expect(1) — project endpoint must fire EXACTLY ONCE across BOTH invocations.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELP"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "99",
            "key": "HELP",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    // CRITICAL: expect(1) — service desk list must fire EXACTLY ONCE across BOTH
    // invocations. On the warm-hit path, get_or_fetch_project_meta returns early from
    // read_project_meta (which already contains serviceDeskId) without calling
    // list_service_desks. Verified against src/api/jsm/servicedesks.rs: the SD list
    // is only called inside the cold-miss branch (line ~78), never on the warm path.
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
        .expect(1)
        .mount(&server)
        .await;

    // Request types endpoint — needed to return results on both invocations.
    // Mounted WITHOUT expect() to keep the test focused on project_meta caching.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
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
                    "id": "11001",
                    "name": "Get IT Help",
                    "description": "Get IT help",
                    "helpText": null,
                    "issueTypeId": "12345",
                    "serviceDeskId": "10",
                    "portalId": "2",
                    "groupIds": []
                }
            ]
        })))
        .mount(&server)
        .await;

    // Invocation 1: cold miss on project_meta — fetches project + service desk list.
    let out1 = jr_cmd_isolated(&server.uri(), cache_dir.path(), config_dir.path())
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
        "Invocation 1 expected exit 0; stderr: {stderr1}"
    );

    // Non-empty content check: an empty [] from inv-1 would make parsed1==parsed2
    // pass vacuously and hide a broken cold-miss path.
    let parsed1: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&out1.stdout))
        .unwrap_or_else(|e| {
            panic!(
                "Invocation 1 expected valid JSON; error: {e}; stdout: {}",
                String::from_utf8_lossy(&out1.stdout)
            )
        });
    let arr1 = parsed1.as_array().expect("expected JSON array from inv 1");
    assert!(
        !arr1.is_empty(),
        "Invocation 1 must return at least one request type (e.g. 'Get IT Help'); \
         got empty array; stdout: {}",
        String::from_utf8_lossy(&out1.stdout)
    );

    // Invocation 2: warm hit on project_meta — must NOT call project endpoint again.
    let out2 = jr_cmd_isolated(&server.uri(), cache_dir.path(), config_dir.path())
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
        "Invocation 2 expected exit 0; stderr: {stderr2}"
    );

    // Both outputs must be valid JSON arrays with identical content.
    let parsed2: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&out2.stdout))
        .unwrap_or_else(|e| panic!("Invocation 2 expected valid JSON; error: {e}"));
    assert_eq!(
        parsed1, parsed2,
        "cache hit must return equivalent output across invocations"
    );

    // MockServer drop automatically enforces expect(1) on project endpoint.
}

// ── Family 5 (cmdb_fields) ────────────────────────────────────────────────────

/// BC-6.2.018 (D2) — Family 5: warm cmdb_fields cache skips HTTP.
///
/// `jr issue list --project PROJ --assets` calls `get_or_fetch_cmdb_fields` which
/// hits `GET /rest/api/3/field` on the first call (cold miss) and writes
/// `cmdb_fields.json`. On the second call (warm hit, same JR_CACHE_DIR), the
/// cache is fresh so the field-discovery endpoint MUST NOT be hit again.
///
/// Non-tautology: would fail if warm path re-fetched — the /field endpoint would
/// be hit twice → expect(1) panics on drop.
///
/// The mock issue response carries pre-resolved `label`/`objectKey` (no bare
/// `objectId`-only), so the asset-enrichment path is skipped and no workspace
/// discovery call is needed on either invocation.
#[tokio::test]
async fn test_cmdb_fields_warm_cache_skips_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    // INTENTIONAL: the config omits `story_points_field_id` and `team_field_id`.
    // Both of those config keys would trigger an additional `list_fields()` call
    // (story-points and team-field discovery each call GET /rest/api/3/field).
    // Keeping them absent guarantees that the single expect(1) on /rest/api/3/field
    // below is reached ONLY by `get_or_fetch_cmdb_fields` — not by an unrelated
    // discovery path — making the call-count pin unambiguous. If you ever add those
    // keys here, you must raise the expect() to match the extra call(s).
    write_minimal_config_with_org(config_dir.path(), &server.uri(), None);

    // CRITICAL: expect(1) — field-discovery endpoint must fire EXACTLY ONCE across
    // BOTH invocations. Returns one CMDB-typed custom field so cmdb_fields.json is
    // written on the cold-miss invocation and the warm-hit invocation reads from it.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
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
        ])))
        .expect(1)
        .mount(&server)
        .await;

    // Project existence check — fires on both invocations (not cached). Mount
    // without expect() so it does not constrain the test.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "key": "PROJ",
            "name": "Test Project"
        })))
        .mount(&server)
        .await;

    // JQL search — fires on both invocations (not cached). Returns one issue
    // with a pre-resolved CMDB asset (label + objectKey already set) so the
    // asset-enrichment HTTP path is skipped entirely on both invocations.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [
                {
                    "key": "PROJ-1",
                    "fields": {
                        "summary": "Test Issue",
                        "status": { "name": "To Do" },
                        "issuetype": { "name": "Task" },
                        "priority": { "name": "Medium" },
                        "assignee": null,
                        "project": { "key": "PROJ" },
                        "customfield_10191": [
                            {
                                "label": "Acme Corp",
                                "objectKey": "ASSET-1"
                            }
                        ]
                    }
                }
            ],
            "nextPageToken": null
        })))
        .mount(&server)
        .await;

    // Invocation 1: cold miss — fetches GET /rest/api/3/field, writes cmdb_fields.json.
    let out1 = jr_cmd_isolated(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "list",
            "--project",
            "PROJ",
            "--assets",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    assert!(
        out1.status.success(),
        "Invocation 1 expected exit 0; stderr: {stderr1}"
    );

    // Non-empty content check: an empty [] from inv-1 would let parsed1==parsed2
    // pass vacuously and hide a broken cold-miss path.
    let parsed1: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&out1.stdout))
        .unwrap_or_else(|e| {
            panic!(
                "Invocation 1 expected valid JSON; error: {e}; stdout: {}",
                String::from_utf8_lossy(&out1.stdout)
            )
        });
    let arr1 = parsed1.as_array().expect("expected JSON array from inv 1");
    assert!(
        !arr1.is_empty(),
        "Invocation 1 must return at least one issue; got empty array; stdout: {}",
        String::from_utf8_lossy(&out1.stdout)
    );

    // Invocation 2: warm hit — must NOT call GET /rest/api/3/field again.
    let out2 = jr_cmd_isolated(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "list",
            "--project",
            "PROJ",
            "--assets",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(
        out2.status.success(),
        "Invocation 2 expected exit 0; stderr: {stderr2}"
    );

    let parsed2: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&out2.stdout))
        .unwrap_or_else(|e| panic!("Invocation 2 expected valid JSON; error: {e}"));
    let arr2 = parsed2.as_array().expect("expected JSON array from inv 2");
    assert_eq!(
        arr1.len(),
        arr2.len(),
        "Both invocations must return same number of issues"
    );

    // MockServer drop automatically enforces expect(1) on the field endpoint.
}

// ── Family 7 (object_type_attrs) ──────────────────────────────────────────────

/// BC-6.2.018 (D2) — Family 7: warm object_type_attrs cache skips HTTP.
///
/// `jr assets search ... --attributes` calls `enrich_search_attributes` which hits
/// `GET /jsm/assets/workspace/<wid>/v1/objecttype/<type_id>/attributes` on the
/// first call (cold miss) and writes `object_type_attrs.json`. On the second call
/// (warm hit, same JR_CACHE_DIR), the cache is fresh so the objecttype-attributes
/// endpoint MUST NOT be hit again.
///
/// Non-tautology: would fail if warm path re-fetched — the attributes endpoint
/// would be hit twice → expect(1) panics on drop.
///
/// The workspace cache is pre-seeded in the TempDir before any invocation so
/// workspace discovery (`GET /rest/servicedeskapi/assets/workspace`) is never
/// needed. The AQL search fires on both invocations (not cached); it is mounted
/// without expect().
#[tokio::test]
async fn test_object_type_attrs_warm_cache_skips_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config_with_org(config_dir.path(), &server.uri(), None);

    // Pre-seed the workspace cache so workspace discovery is never needed.
    // Cache path: JR_CACHE_DIR/v1/default/workspace.json
    let workspace_id = "test-workspace-abc123";
    let cache_profile_dir = cache_dir.path().join("jr").join("v1").join("default");
    std::fs::create_dir_all(&cache_profile_dir).unwrap();
    let workspace_cache = serde_json::json!({
        "workspace_id": workspace_id,
        "fetched_at": "2099-01-01T00:00:00Z"
    });
    std::fs::write(
        cache_profile_dir.join("workspace.json"),
        serde_json::to_string_pretty(&workspace_cache).unwrap(),
    )
    .unwrap();

    // AQL search — fires on both invocations (not cached). Returns one object
    // with objectType.id "42" so enrich_search_attributes is called on cold miss.
    Mock::given(method("POST"))
        .and(path(format!(
            "/jsm/assets/workspace/{workspace_id}/v1/object/aql"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": true,
            "values": [
                {
                    "id": "88",
                    "label": "Web Server 01",
                    "objectKey": "SRV-1",
                    "objectType": { "id": "42", "name": "Server" },
                    "attributes": [
                        {
                            "id": "501",
                            "objectTypeAttributeId": "201",
                            "objectAttributeValues": [
                                { "value": "prod", "displayValue": "prod" }
                            ]
                        }
                    ]
                }
            ]
        })))
        .mount(&server)
        .await;

    // CRITICAL: expect(1) — objecttype attributes endpoint must fire EXACTLY ONCE
    // across BOTH invocations. Returns one non-system, non-hidden attribute so the
    // result is meaningful and the cache write actually stores something.
    Mock::given(method("GET"))
        .and(path(format!(
            "/jsm/assets/workspace/{workspace_id}/v1/objecttype/42/attributes"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "201",
                "name": "Environment",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 1
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    // Invocation 1: cold miss on object_type_attrs — fetches objecttype/42/attributes,
    // writes object_type_attrs.json.
    let out1 = jr_cmd_isolated(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "assets",
            "search",
            "objectType = Server",
            "--attributes",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    assert!(
        out1.status.success(),
        "Invocation 1 expected exit 0; stderr: {stderr1}"
    );

    let stdout1 = String::from_utf8_lossy(&out1.stdout);
    assert!(
        stdout1.contains("Web Server 01") || stdout1.contains("SRV-1"),
        "Invocation 1 must list the asset; stdout: {stdout1}"
    );

    // Invocation 2: warm hit — must NOT call objecttype/42/attributes again.
    let out2 = jr_cmd_isolated(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "assets",
            "search",
            "objectType = Server",
            "--attributes",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(
        out2.status.success(),
        "Invocation 2 expected exit 0; stderr: {stderr2}"
    );

    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    assert!(
        stdout2.contains("Web Server 01") || stdout2.contains("SRV-1"),
        "Invocation 2 must return same asset data from cache; stdout: {stdout2}"
    );

    // MockServer drop automatically enforces expect(1) on the objecttype attributes endpoint.
}

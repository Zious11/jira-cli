//! Red-gate integration tests for `issue create` changed-fields echo (S-398).
//!
//! BC-3.4.014: table-mode success echoes ALL set fields to stderr (mirroring
//! BC-3.4.012): one `  <field> → <value>` line per field, alphabetical order,
//! between the "Created issue" confirmation and the browse URL.
//!
//! Every test in this file is expected to FAIL before the implementation
//! because the `create_echo` BTreeMap is not yet built or emitted in
//! `handle_create`. The binary currently only emits "Created issue KEY" and
//! the browse URL — no field echo lines.
//!
//! Failure mode per test:
//! - Echo-presence tests: `stderr.contains("  <field> → <value>")` fails because
//!   the field echo lines are not emitted yet.
//! - Order tests: the positional assertions fail because no lines exist to order.
//! - Negative guard tests (JSM, unresolvable team): currently pass vacuously
//!   because no echo fires at all; after implementation they pin the exclusion.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

const TEST_TEAM_FIELD_ID: &str = "customfield_10100";

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

/// Write a team cache with "Platform Core" (team-uuid-platform) and
/// "Security Engineering" (team-uuid-security).
fn write_team_cache_with_platform_core(cache_home: &std::path::Path) {
    let teams_dir = cache_home.join("jr").join("v1").join("default");
    std::fs::create_dir_all(&teams_dir).unwrap();
    let cache = jr::cache::TeamCache {
        fetched_at: chrono::Utc::now(),
        teams: vec![
            jr::cache::CachedTeam {
                id: "team-uuid-platform".into(),
                name: "Platform Core".into(),
            },
            jr::cache::CachedTeam {
                id: "team-uuid-security".into(),
                name: "Security Engineering".into(),
            },
        ],
    };
    std::fs::write(
        teams_dir.join("teams.json"),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();
}

/// Write a config.toml with team_field_id set.
fn write_config_with_team_field(config_home: &std::path::Path) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        format!("[fields]\nteam_field_id = \"{TEST_TEAM_FIELD_ID}\"\n"),
    )
    .unwrap();
}

/// Write a config.toml with team_field_id AND an instance URL.
fn write_config_with_team_field_and_instance(config_home: &std::path::Path, url: &str) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        format!(
            "[instance]\nurl = \"{url}\"\n[fields]\nteam_field_id = \"{TEST_TEAM_FIELD_ID}\"\n"
        ),
    )
    .unwrap();
}

/// Write a minimal config with no extra fields (for plain create tests).
fn write_minimal_config(config_home: &std::path::Path) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(conf_dir.join("config.toml"), "").unwrap();
}

/// Write a config.toml with an instance URL (used where the JSM path needs the URL
/// while JR_BASE_URL / JR_AUTH_HEADER override the actual credentials).
fn write_config_with_instance(config_home: &std::path::Path, url: &str) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        format!("[instance]\nurl = \"{url}\"\n"),
    )
    .unwrap();
}

/// Write a config.toml with story_points_field_id set.
fn write_config_with_story_points(config_home: &std::path::Path) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        "[fields]\nstory_points_field_id = \"customfield_10031\"\n",
    )
    .unwrap();
}

/// Mount POST /rest/api/3/issue returning 201 with the given issue key.
async fn mount_post_201(server: &MockServer, key: &str) {
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10001",
            "key": key,
            "self": format!("{}/rest/api/3/issue/10001", server.uri())
        })))
        .expect(1)
        .mount(server)
        .await;
}

// ---------------------------------------------------------------------------
// Test 12 — AC-006: create table-mode echoes all fields in alphabetical order
// BC-3.4.014 postconditions; VP-398-005 part B.
//
// Fields: issue_type (i), priority (p), summary (s), team (t) — alphabetical.
//
// Pre-impl failure mode: stderr does NOT contain any `  field → value` lines.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_014_create_all_fields_echo_alphabetical_order() {
    let server = MockServer::start().await;
    mount_post_201(&server, "PROJ-1").await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_team_cache_with_platform_core(cache_dir.path());
    write_config_with_team_field(config_dir.path());

    // Use exact team name — partial_match returns Exact on case-insensitive
    // exact match; substrings return Ambiguous which causes --no-input exit 64.
    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Fix login bug",
            "--priority",
            "High",
            "--team",
            "Platform Core",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    // stdout must be empty in table mode (all output on stderr)
    assert!(
        stdout.is_empty(),
        "stdout must be empty in table mode; stdout={stdout}"
    );

    // Confirmation line must be present
    assert!(
        stderr.contains("Created issue PROJ-1"),
        "Expected 'Created issue PROJ-1' in stderr; stderr={stderr}"
    );

    // Each field echo must be present — BC-3.4.014 postcondition
    assert!(
        stderr.contains("  issue_type \u{2192} Task"),
        "Expected '  issue_type → Task' in stderr; stderr={stderr}"
    );
    assert!(
        stderr.contains("  priority \u{2192} High"),
        "Expected '  priority → High' in stderr; stderr={stderr}"
    );
    assert!(
        stderr.contains("  summary \u{2192} Fix login bug"),
        "Expected '  summary → Fix login bug' in stderr; stderr={stderr}"
    );
    assert!(
        stderr.contains("  team \u{2192} Platform Core"),
        "Expected '  team → Platform Core' in stderr; stderr={stderr}"
    );

    // Alphabetical ordering assertions
    let issue_type_pos = stderr
        .find("  issue_type \u{2192} Task")
        .expect("issue_type echo not found");
    let priority_pos = stderr
        .find("  priority \u{2192} High")
        .expect("priority echo not found");
    let summary_pos = stderr
        .find("  summary \u{2192} Fix login bug")
        .expect("summary echo not found");
    let team_pos = stderr
        .find("  team \u{2192} Platform Core")
        .expect("team echo not found");

    assert!(
        issue_type_pos < priority_pos,
        "issue_type must appear before priority (i < p); stderr={stderr}"
    );
    assert!(
        priority_pos < summary_pos,
        "priority must appear before summary (p < s); stderr={stderr}"
    );
    assert!(
        summary_pos < team_pos,
        "summary must appear before team (s < t); stderr={stderr}"
    );

    // Field echo lines must appear BEFORE the browse URL
    let team_line_pos = team_pos;
    let browse_pos = stderr
        .find("https://")
        .or_else(|| stderr.find("http://"))
        .expect("browse URL not found in stderr");
    assert!(
        team_line_pos < browse_pos,
        "field echo lines must appear before browse URL; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 13 — AC-002 (create): team echo is resolved display name, not UUID
// BC-3.4.014 invariant 1; VP-398-001 positive case on create path.
//
// Pre-impl failure mode: stderr does NOT contain `  team → Platform Core`.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_014_create_team_echo_is_resolved_name_not_uuid() {
    let server = MockServer::start().await;
    mount_post_201(&server, "PROJ-1").await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_team_cache_with_platform_core(cache_dir.path());
    write_config_with_team_field(config_dir.path());

    // Use exact team name — partial_match returns Exact on case-insensitive
    // exact match; substrings return Ambiguous which causes --no-input exit 64.
    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Team name test",
            "--team",
            "Platform Core",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    // Must echo resolved display name — VP-398-001 positive case on create path
    assert!(
        stderr.contains("  team \u{2192} Platform Core"),
        "Expected '  team → Platform Core' in stderr; stderr={stderr}"
    );

    // Must NOT echo the UUID
    assert!(
        !stderr.contains("team-uuid-platform"),
        "UUID must NOT appear in team echo; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 14 — AC-009: create echoes `(updated)` for description — never content
// BC-3.4.014 invariant 2; VP-398-006.
//
// Pre-impl failure mode: `  description → (updated)` absent in stderr.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_014_create_description_echo_is_updated_marker() {
    let server = MockServer::start().await;
    mount_post_201(&server, "PROJ-1").await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path());

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "X",
            "--description",
            "Some longer description text that must never appear in table output",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    // Must echo the (updated) marker — BC-3.4.014 invariant 2
    assert!(
        stderr.contains("  description \u{2192} (updated)"),
        "Expected 'description → (updated)' in stderr; stderr={stderr}"
    );

    // Must NOT echo the content
    assert!(
        !stderr.contains("Some longer description text"),
        "Description content must NOT appear in stderr; stderr={stderr}"
    );

    // stdout must be empty
    assert!(
        stdout.is_empty(),
        "stdout must be empty in table mode; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 15 — AC-011: label echo is comma-space joined, command-line order
// BC-3.4.014 invariant 5; EC-3.4.014-9.
//
// Pre-impl failure mode: `  label → bug, urgent` absent in stderr.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_014_create_label_echo_comma_space_joined() {
    let server = MockServer::start().await;
    mount_post_201(&server, "PROJ-1").await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path());

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "S",
            "--label",
            "bug",
            "--label",
            "urgent",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    // Labels joined with ", " in command-line order — BC-3.4.014 invariant 5
    assert!(
        stderr.contains("  label \u{2192} bug, urgent"),
        "Expected '  label → bug, urgent' in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 16 — AC-012: assignee echo shows display name for `--to` path
// BC-3.4.014 postcondition: `assignee → <display_name>`.
//
// Pre-impl failure mode: `  assignee → Jane Doe` absent in stderr.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_014_create_assignee_echo_display_name() {
    let server = MockServer::start().await;
    mount_post_201(&server, "PROJ-1").await;

    // `--to` path uses multiProjectSearch
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("query", "jane"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "accountId": "acc-jane-001",
                "displayName": "Jane Doe",
                "active": true
            }
        ])))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path());

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Assignee test",
            "--to",
            "jane",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    // Echo must show the display name, not the account ID
    assert!(
        stderr.contains("  assignee \u{2192} Jane Doe"),
        "Expected '  assignee → Jane Doe' in stderr; stderr={stderr}"
    );

    // Must NOT echo the raw account ID
    assert!(
        !stderr.contains("acc-jane-001"),
        "Account ID must NOT appear in assignee echo; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 17 — AC-019: unresolvable team name with `--no-input` exits 64, no POST
// BC-3.4.014 EC-5; VP-398-005 part A.
//
// Pre-impl failure mode: currently this test behavior is driven by the existing
// `resolve_team_field` MatchResult::None path which already exits 64. This test
// is CORRECT both pre- and post-impl but is included here to pin the AC-019
// guarantee: no POST is issued (`.expect(0)`) and stderr contains `No team matching`.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_014_create_unresolvable_team_no_input_exits_64() {
    let server = MockServer::start().await;

    // POST must NOT be called — unresolvable team should abort before POST
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "99999",
            "key": "PROJ-99"
        })))
        .expect(0) // must NOT be called
        .mount(&server)
        .await;

    // Mount GraphQL + teams endpoints to verify auto-refresh is also tried
    // (and also returns no matching team).
    Mock::given(method("POST"))
        .and(path("/gateway/api/graphql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::graphql_org_metadata_json()),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex("/gateway/api/public/teams/v1/org/.*/teams"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "entities": [
                { "teamId": "team-uuid-other", "displayName": "Other Team" }
            ],
            "cursor": null
        })))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_config_with_team_field_and_instance(config_dir.path(), &server.uri());

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Test",
            "--team",
            "definitely-does-not-exist-xyzzy",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Must exit 64 — JrError::UserError
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for unresolvable team; stderr={stderr} stdout={stdout}"
    );

    // stdout must be empty (no issue key, no JSON)
    assert!(
        stdout.is_empty(),
        "stdout must be empty when team resolution fails; stdout={stdout}"
    );

    // Error message must contain the stable substring
    assert!(
        stderr.contains("No team matching"),
        "Expected 'No team matching' in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 18 — AC-015: create JSON output path is UNCHANGED (no `changed_fields` key)
// BC-3.4.014 postcondition: create JSON path unchanged; no `changed_fields` added.
//
// Pre-impl failure mode: none (the JSON path has no changed_fields currently).
// This is a regression guard: after implementation, the create JSON path must
// NOT gain a `changed_fields` key (unlike edit JSON path which DOES gain it).
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_014_create_json_output_unchanged_no_changed_fields_key() {
    let server = MockServer::start().await;

    // POST 201
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-1",
            "self": format!("{}/rest/api/3/issue/10001", server.uri())
        })))
        .expect(1)
        .mount(&server)
        .await;

    // GET for follow-up (JSON path does a GET after successful POST)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-1",
            "fields": {
                "summary": "No changed_fields test",
                "status": {"name": "To Do", "statusCategory": {"name": "To Do", "key": "new"}},
                "issuetype": {"name": "Task"},
                "project": {"key": "PROJ"}
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // GET /rest/api/3/field — for CMDB field discovery on JSON path
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path());

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "No changed_fields test",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    // The JSON output must NOT contain a `changed_fields` key — AC-015
    assert!(
        parsed.get("changed_fields").is_none(),
        "create JSON output must NOT contain 'changed_fields' key; stdout={stdout}"
    );

    // The key must be present (sanity)
    assert_eq!(
        parsed["key"].as_str(),
        Some("PROJ-1"),
        "key must be present; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 19 — EC-3.4.014-11: `--points 5` echoes `  points → 5` on create
// BC-3.4.014 EC-3.4.014-11; concrete (non-snapshot) assertion on f64::to_string().
//
// Behavior is already implemented (`pts.to_string()`); this test pins the format.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_014_create_points_integer_value_echo() {
    let server = MockServer::start().await;
    mount_post_201(&server, "PROJ-1").await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_config_with_story_points(config_dir.path());

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Points echo test",
            "--points",
            "5",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    // BC-3.4.014 EC-3.4.014-11: integer points must echo as "5", not "5.0" or "5.00"
    assert!(
        stderr.contains("  points \u{2192} 5"),
        "Expected '  points → 5' in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 20 (OBS-4) — AC-012 clause 2: `--account-id` assignee echo shows raw ID
// BC-3.4.014 postcondition: `assignee → <account_id_string>` (no name resolution).
//
// The `--account-id` arm in create.rs (~line 219-222) inserts the raw account ID
// string directly into the echo map without any display-name lookup. This test
// pins that contract — the implementation already handles this path, so the test
// passes on first run. A failure here indicates a real implementation regression.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_014_create_assignee_account_id_echo() {
    let server = MockServer::start().await;
    mount_post_201(&server, "PROJ-2").await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path());

    // Placeholder account-id — realistic format but NOT a real Jira account ID.
    let account_id = "5b10a2844c20165700ede21g";

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Account-id assignee echo test",
            "--account-id",
            account_id,
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    // stdout must be empty in table mode
    assert!(
        stdout.is_empty(),
        "stdout must be empty in table mode; stdout={stdout}"
    );

    // Confirmation line must be present
    assert!(
        stderr.contains("Created issue PROJ-2"),
        "Expected 'Created issue PROJ-2' in stderr; stderr={stderr}"
    );

    // AC-012 clause 2: --account-id path echoes the raw account-id string, not a
    // display name (no user lookup is performed on this code path).
    assert!(
        stderr.contains(&format!("  assignee \u{2192} {account_id}")),
        "Expected '  assignee → {account_id}' in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 21 — AC-014: JSM `--request-type` path never emits field echo lines
// BC-3.4.014 EC-014; AC-014.
//
// `handle_jsm_create` is entered when `--request-type` is set. The `create_echo`
// BTreeMap is declared structurally AFTER the dispatch fork, so `handle_jsm_create`
// can never build or emit it. This test pins that guarantee: a JSM create with
// `--summary`, `--description`, and `--priority` set must exit 0 and emit NO
// `  field → value` echo lines on stderr.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_014_create_jsm_request_type_path_no_field_echo() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_config_with_instance(config_dir.path(), &server.uri());

    // GET /rest/api/3/project/HELP — service_desk type, project id "99"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELP"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "99",
            "key": "HELP",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .mount(&server)
        .await;

    // GET /rest/servicedeskapi/servicedesk — maps project id "99" to service desk "10"
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
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
        .mount(&server)
        .await;

    // GET /rest/servicedeskapi/servicedesk/10/requesttype — single "Password Reset" type
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "_links": {},
            "values": [
                {
                    "id": "11002",
                    "name": "Password Reset",
                    "description": "Reset your password",
                    "helpText": "Provide your username",
                    "issueTypeId": "12346",
                    "serviceDeskId": "10",
                    "portalId": "2",
                    "groupIds": ["12"]
                }
            ]
        })))
        .mount(&server)
        .await;

    // POST /rest/servicedeskapi/request — must succeed (201)
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "issueId": "107001",
            "issueKey": "HELP-42",
            "requestTypeId": "11002",
            "serviceDeskId": "10",
            "_links": {
                "self": "https://example.atlassian.net/rest/servicedeskapi/request/107001",
                "web": "https://example.atlassian.net/servicedesk/customer/portal/10/HELP-42"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "Password Reset",
            "--summary",
            "Reset my account password",
            "--description",
            "I cannot log in to my account.",
            "--priority",
            "High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // AC-014 precondition: command must succeed (exit 0)
    assert!(
        output.status.success(),
        "AC-014: expected exit 0 on JSM --request-type path; stderr={stderr} stdout={stdout}"
    );

    // AC-014 primary assertion: NO `  field → value` echo lines on stderr.
    // The echo map is never built on the JSM dispatch path.
    assert!(
        !stderr.contains(" \u{2192} "),
        "AC-014: JSM --request-type path must emit NO field echo lines; stderr={stderr}"
    );

    // Negative guards: specific fields that WOULD appear on the platform echo path
    // must be absent here.
    assert!(
        !stderr.contains("  summary \u{2192}"),
        "AC-014: 'summary →' must not appear on JSM path; stderr={stderr}"
    );
    assert!(
        !stderr.contains("  description \u{2192}"),
        "AC-014: 'description →' must not appear on JSM path; stderr={stderr}"
    );
    assert!(
        !stderr.contains("  priority \u{2192}"),
        "AC-014: 'priority →' must not appear on JSM path; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 22 — TH-398-4: create --description-stdin echoes `(updated)` marker
// BC-3.4.014 invariant 2; parity with edit-side
// test_bc_3_4_013_description_stdin_trailing_newline_preserved_in_changed_fields.
//
// The create side must echo `  description → (updated)` (table mode) when
// description is supplied via stdin. The stdin content must NOT appear in stderr.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_014_create_description_stdin_echo_is_updated_marker() {
    let server = MockServer::start().await;
    mount_post_201(&server, "PROJ-1").await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "X",
            "--description-stdin",
        ])
        .write_stdin("Some stdin description\n")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    // Must echo the (updated) marker — BC-3.4.014 invariant 2
    assert!(
        stderr.contains("  description \u{2192} (updated)"),
        "Expected 'description → (updated)' in stderr; stderr={stderr}"
    );

    // Must NOT echo the stdin content — BC-3.4.014 invariant 2
    assert!(
        !stderr.contains("Some stdin description"),
        "Stdin description content must NOT appear in stderr; stderr={stderr}"
    );

    // stdout must be empty in table mode
    assert!(
        stdout.is_empty(),
        "stdout must be empty in table mode; stdout={stdout}"
    );
}

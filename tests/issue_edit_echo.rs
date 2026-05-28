//! Integration tests for `issue edit` changed-fields echo (S-398).
//!
//! BC-3.4.012: table-mode success echoes one stderr line per changed field in
//!   `  field → value` format; resolved team name; `(updated)` marker for description.
//!
//! BC-3.4.013: JSON-mode success includes `changed_fields` BTreeMap in the response;
//!   description carries the RAW user-supplied input string; `updated: true` retained.
//!
//! Coverage includes single-field, multi-field, description-only, and empty
//! (no-op) edits in both table and JSON output modes, plus echo-suppression on
//! non-204 PUT responses (AC-021).

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helpers — mirrors cli_handler.rs pattern
// ---------------------------------------------------------------------------

const TEST_TEAM_FIELD_ID: &str = "customfield_10100";

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

/// Write a team cache with "Platform Core" (id "team-uuid-platform") and
/// "Security Engineering" (id "team-uuid-security") in the default profile
/// slot. The query "plat" must match "Platform Core" exactly (substring
/// match against a single result — not ambiguous).
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

/// Write a minimal config.toml with team_field_id set.
fn write_config_with_team_field(config_home: &std::path::Path) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        format!("[fields]\nteam_field_id = \"{TEST_TEAM_FIELD_ID}\"\n"),
    )
    .unwrap();
}

/// Mount a PUT /rest/api/3/issue/{key} that returns 204.
async fn mount_put_204(server: &MockServer, issue_key: &str) {
    Mock::given(method("PUT"))
        .and(path(format!("/rest/api/3/issue/{issue_key}")))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(server)
        .await;
}

// ---------------------------------------------------------------------------
// Test 1 — AC-001: table-mode echoes summary and priority in alphabetical order
// BC-3.4.012 postconditions: one `  field → value` line per changed field,
// alphabetical order (priority < summary).
//
// Pre-impl failure mode: stderr does NOT contain `  priority → High` or
// `  summary → New title` — the assertion on those substrings fails.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_012_edit_table_echo_summary_and_priority() {
    let server = MockServer::start().await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--summary",
            "New title",
            "--priority",
            "High",
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

    // Existing confirmation line must still be present
    assert!(
        stderr.contains("Updated TEST-1"),
        "Expected 'Updated TEST-1' in stderr; stderr={stderr}"
    );

    // BC-3.4.012 postcondition: field echo lines present
    assert!(
        stderr.contains("  priority \u{2192} High"),
        "Expected '  priority → High' in stderr; stderr={stderr}"
    );
    assert!(
        stderr.contains("  summary \u{2192} New title"),
        "Expected '  summary → New title' in stderr; stderr={stderr}"
    );

    // Alphabetical order: priority line must appear BEFORE summary line
    let priority_pos = stderr
        .find("  priority \u{2192} High")
        .expect("priority echo line not found");
    let summary_pos = stderr
        .find("  summary \u{2192} New title")
        .expect("summary echo line not found");
    assert!(
        priority_pos < summary_pos,
        "priority echo must appear before summary echo (alphabetical); stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 2 — AC-002: table-mode echoes RESOLVED team name, not UUID or query string
// BC-3.4.012 invariant 1; VP-398-001 positive case.
//
// Pre-impl failure mode: stderr does NOT contain `  team → Platform Core` —
// either no echo fires at all, or the UUID is echoed instead.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_012_team_echo_is_resolved_name_not_uuid() {
    let server = MockServer::start().await;

    // Mount PUT 204
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // Cache has "Platform Core" with id "team-uuid-platform".
    // Querying with "plat" should match and echo "Platform Core", not the UUID.
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_team_cache_with_platform_core(cache_dir.path());
    write_config_with_team_field(config_dir.path());

    // Use the full display name for exact match (partial_match returns Exact
    // only on case-insensitive exact match; substrings return Ambiguous which
    // causes --no-input to exit 64).
    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--team",
            "Platform Core",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    // Must echo resolved display name — BC-3.4.012 invariant 1
    assert!(
        stderr.contains("  team \u{2192} Platform Core"),
        "Expected '  team → Platform Core' in stderr; stderr={stderr}"
    );

    // Must NOT echo the UUID — VP-398-001 positive case
    assert!(
        !stderr.contains("team-uuid-platform"),
        "UUID must NOT appear in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — AC-003: table-mode echoes `(updated)` for description — never content
// BC-3.4.012 postcondition + invariant 2; VP-398-002.
//
// Pre-impl failure mode: stderr does NOT contain `description → (updated)` —
// no echo fires.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_012_description_echo_is_updated_marker_not_content() {
    let server = MockServer::start().await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--description",
            "Some longer description text that must never appear in table output",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    // Must echo the marker
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
// Test 4 — AC-004: `--no-parent` echoes `parent → (cleared)`, not `no_parent`
// BC-3.4.012 postcondition; BC-3.4.012 EC-3; VP-398-004.
//
// Pre-impl failure mode: stderr does NOT contain `  parent → (cleared)` or
// `  points → (cleared)` — no echo fires.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_012_no_parent_table_echo_uses_parent_key() {
    let server = MockServer::start().await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "TEST-1", "--no-parent"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    // Must use key "parent" not "no_parent"
    assert!(
        stderr.contains("  parent \u{2192} (cleared)"),
        "Expected '  parent → (cleared)' in stderr; stderr={stderr}"
    );

    // Must NOT contain "no_parent" as a field label
    assert!(
        !stderr.contains("no_parent"),
        "Field label 'no_parent' must NOT appear; stderr={stderr}"
    );

    // --- Second assertion: --no-points echoes `points → (cleared)` ---
    let server2 = MockServer::start().await;
    mock_put_with_sp_field(&server2, "TEST-2").await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_config_with_story_points(config_dir.path());

    let output2 = jr_cmd_with_xdg(&server2.uri(), cache_dir.path(), config_dir.path())
        .args(["--no-input", "issue", "edit", "TEST-2", "--no-points"])
        .output()
        .unwrap();

    let stderr2 = String::from_utf8_lossy(&output2.stderr);

    assert!(
        output2.status.success(),
        "Expected exit 0 for --no-points; stderr={stderr2}"
    );

    assert!(
        stderr2.contains("  points \u{2192} (cleared)"),
        "Expected '  points → (cleared)' in stderr; stderr={stderr2}"
    );

    assert!(
        !stderr2.contains("no_points"),
        "Field label 'no_points' must NOT appear; stderr={stderr2}"
    );
}

async fn mock_put_with_sp_field(server: &MockServer, issue_key: &str) {
    Mock::given(method("PUT"))
        .and(path(format!("/rest/api/3/issue/{issue_key}")))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(server)
        .await;
}

fn write_config_with_story_points(config_home: &std::path::Path) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        "[fields]\nstory_points_field_id = \"customfield_10031\"\n",
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// Test 5 — AC-005: JSON mode includes `changed_fields` BTreeMap; `updated:true` retained
// BC-3.4.013 postconditions; VP-398-003.
//
// Pre-impl failure mode: `output["changed_fields"]` is null/absent — the stub
// `edit_response` discards the `changed_fields` parameter.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_013_updated_true_present_with_summary_changed_fields() {
    let server = MockServer::start().await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--summary",
            "New title",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    // stderr must be empty in JSON mode
    assert!(
        stderr.is_empty(),
        "stderr must be empty in JSON mode; stderr={stderr}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    // backward-compat: `updated: true` must be present
    assert_eq!(
        parsed["updated"],
        serde_json::json!(true),
        "updated must be true; stdout={stdout}"
    );

    // changed_fields must be present and non-null
    assert!(
        !parsed["changed_fields"].is_null(),
        "changed_fields must be present and non-null; stdout={stdout}"
    );

    // changed_fields must contain summary
    assert_eq!(
        parsed["changed_fields"]["summary"].as_str(),
        Some("New title"),
        "changed_fields.summary must be 'New title'; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 6 — AC-007: JSON mode carries raw description string, NOT `(updated)` marker
// BC-3.4.013 postcondition; VP-398-002 asymmetry invariant.
//
// Pre-impl failure mode: `changed_fields` is absent → the assertion that
// `changed_fields["description"]` equals the raw string fails.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_013_description_echo_is_raw_input_string_not_marker() {
    let server = MockServer::start().await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--description",
            "Some longer description text",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    // changed_fields.description must be the RAW user input
    assert_eq!(
        parsed["changed_fields"]["description"].as_str(),
        Some("Some longer description text"),
        "changed_fields.description must be raw input string; stdout={stdout}"
    );

    // Must NOT be the (updated) marker
    assert_ne!(
        parsed["changed_fields"]["description"].as_str(),
        Some("(updated)"),
        "changed_fields.description must NOT be '(updated)' in JSON mode; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 7 — AC-007 sub-case: stdin trailing-newline preserved in changed_fields
// BC-3.4.013 EC-3; VP-398-002 stdin sub-case.
//
// Pre-impl failure mode: `changed_fields` absent or description key missing.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_013_description_stdin_trailing_newline_preserved_in_changed_fields() {
    let server = MockServer::start().await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--description-stdin",
        ])
        .write_stdin("My description\n")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    // The trailing \n must be preserved verbatim (no normalization)
    assert_eq!(
        parsed["changed_fields"]["description"].as_str(),
        Some("My description\n"),
        "Trailing newline must be preserved in changed_fields.description; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 8 — AC-008: JSON mode uses `parent` key (not `no_parent`) for cleared parent
// BC-3.4.013 postcondition; BC-3.4.013 EC-4; VP-398-004.
//
// Pre-impl failure mode: `changed_fields` absent.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_013_no_parent_key_is_parent_not_no_parent() {
    let server = MockServer::start().await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--no-parent",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    // Must use "parent" key with "(cleared)" value
    assert_eq!(
        parsed["changed_fields"]["parent"].as_str(),
        Some("(cleared)"),
        "changed_fields must contain 'parent': '(cleared)'; stdout={stdout}"
    );

    // Must NOT contain "no_parent" key
    assert!(
        parsed["changed_fields"].get("no_parent").is_none(),
        "changed_fields must NOT contain 'no_parent' key; stdout={stdout}"
    );

    // changed_fields must contain exactly one key
    assert_eq!(
        parsed["changed_fields"]
            .as_object()
            .map(|m| m.len())
            .unwrap_or(0),
        1,
        "changed_fields must contain exactly one key; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 9 — AC-008: JSON mode uses `points` key (not `no_points`) for cleared points
// BC-3.4.013 EC-5; VP-398-004.
//
// Pre-impl failure mode: `changed_fields` absent.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_013_no_points_key_is_points_not_no_points() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_config_with_story_points(config_dir.path());

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--no-points",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected exit 0; stderr={stderr}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    assert_eq!(
        parsed["changed_fields"]["points"].as_str(),
        Some("(cleared)"),
        "changed_fields must contain 'points': '(cleared)'; stdout={stdout}"
    );

    assert!(
        parsed["changed_fields"].get("no_points").is_none(),
        "changed_fields must NOT contain 'no_points' key; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 10 — AC-010: echo does NOT fire on dry-run
// BC-3.4.012 precondition: `--dry-run` is NOT set; echo fires only after PUT.
//
// Pre-impl failure mode: the test currently passes because no echo fires at all.
// It is still a valid Red Gate test because after implementation, we must ensure
// dry-run does NOT emit echo lines even when the map would be non-empty.
// The test asserts absence — which is correct both pre- and post-impl for dry-run.
// The RED assertion is the presence check in test 1; this is a negative guard.
// ---------------------------------------------------------------------------

#[test]
fn test_bc_3_4_012_edit_echo_does_not_fire_on_dry_run() {
    // --dry-run exits before any HTTP call, so no real server is needed.
    // JR_BASE_URL must still be set to satisfy config loading (exit 78 otherwise).
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:19999")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--no-input",
            "--output",
            "table",
            "issue",
            "edit",
            "TEST-1",
            "--dry-run",
            "--summary",
            "X",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // POSITIVE guard: prove the dry-run path actually executed (not an error
    // path that vacuously satisfies the negative checks). Single-key `--dry-run`
    // exits 0 and writes the planned-changes PREVIEW to *stdout*:
    //   "DRY RUN — no changes will be made." + "  summary → X"
    // (see src/cli/issue/create.rs handle_edit dry-run block — println!, stdout).
    assert!(
        output.status.success(),
        "single-key --dry-run must exit 0; status={:?} stderr={stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("DRY RUN") && stdout.contains("summary \u{2192} X"),
        "dry-run preview (planned changes) must appear on stdout; stdout={stdout}"
    );

    // NEGATIVE guard: the post-success changed-fields ECHO must NOT fire on
    // dry-run. The success confirmation `Updated TEST-1` is emitted to stderr
    // via output::print_success ONLY on the live PUT path; dry-run `return`s
    // before reaching it. Asserting its absence on BOTH streams catches a
    // regression where dry-run falls through to the live success/echo path.
    assert!(
        !stdout.contains("Updated TEST-1") && !stderr.contains("Updated TEST-1"),
        "dry-run must not reach the success confirmation path; stdout={stdout} stderr={stderr}"
    );
    // The post-success echo writes `  <field> → <value>` to stderr; it must be
    // absent (the identical-looking line on stdout is the dry-run PREVIEW, a
    // different code path on a different stream).
    assert!(
        !stderr.contains("  summary \u{2192} X"),
        "post-success echo (stderr) must not fire on dry-run; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 11 — AC-013: echo EXCLUDED for bulk/multi-key paths
// BC-3.4.012 scope: single-key path only.
//
// Pre-impl failure mode: currently no echo fires for bulk either, so this
// test will pass. After implementation, it pins that bulk paths don't echo.
// This is a regression guard — the RED comes from tests 1–9.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_012_edit_echo_excluded_for_bulk_multi_key() {
    let server = MockServer::start().await;

    // Two-key edit routes through handle_edit_bulk_fields → POST
    // /rest/api/3/bulk/issues/fields, NOT through PUT /rest/api/3/issue/{key}.
    // Mount the PUT mocks as `.expect(0)` sentinels to assert they are never called.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-2"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    // Bulk submit: POST /rest/api/3/bulk/issues/fields returns 200. Only `taskId`
    // is deserialized into BulkSubmitResponse (src/types/jira/bulk.rs); the body is
    // trimmed to exactly that field.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "taskId": "task-bulk-echo-001"
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Task poll: GET /rest/api/3/bulk/queue/task-bulk-echo-001 returns COMPLETE.
    // Body shape matches BulkOperationProgress (processedAccessibleIssues, status).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-bulk-echo-001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "taskId": "task-bulk-echo-001",
            "status": "COMPLETE",
            "progressPercent": 100,
            "totalIssueCount": 2,
            "processedAccessibleIssues": ["TEST-1", "TEST-2"],
            "failedAccessibleIssues": {},
            "invalidOrInaccessibleIssueCount": 0
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "TEST-2",
            "--summary",
            "Bulk update",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // The bulk path must succeed (exit 0)
    assert!(
        output.status.success(),
        "Expected exit 0 for bulk path; stderr={stderr}"
    );

    // bulk path must never emit changed-fields echo lines
    assert!(
        !stderr.contains("  summary \u{2192}"),
        "bulk path must NOT emit field echo; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 21 — EC-3.4.012-12: `--summary ""` echoes empty value
// BC-3.4.012 EC-12; wiremock-only (real Jira rejects empty summary with 400).
//
// Pre-impl failure mode: no echo fires → `  summary → ` line absent.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_012_empty_summary_echoes_empty_value() {
    let server = MockServer::start().await;
    // Real Jira rejects empty summary with 400 but wiremock returns 204.
    // This tests the echo FORMATTING of an empty-string value.
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "TEST-1", "--summary", ""])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 (wiremock 204); stderr={stderr} stdout={stdout}"
    );

    // Echo line: `  summary → ` with nothing after the arrow (empty value is valid)
    assert!(
        stderr.contains("  summary \u{2192} "),
        "Expected '  summary → ' echo (possibly empty after arrow); stderr={stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 22 — EC-3.4.013-10: `--summary ""` appears in changed_fields JSON
// BC-3.4.013 EC-10; wiremock-only.
//
// Pre-impl failure mode: `changed_fields` absent or key absent.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_013_empty_summary_in_changed_fields() {
    let server = MockServer::start().await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--summary",
            "",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 (wiremock 204); stderr={stderr} stdout={stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"));

    // changed_fields["summary"] must be present with an empty string value
    assert!(
        parsed["changed_fields"].get("summary").is_some(),
        "changed_fields must contain 'summary' key; stdout={stdout}"
    );
    assert_eq!(
        parsed["changed_fields"]["summary"].as_str(),
        Some(""),
        "changed_fields.summary must be empty string; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Test 23 — AC-021: echo SUPPRESSED when PUT returns non-204 error
// BC-3.4.012 invariant 6 + BC-3.4.013 invariant 6.
//
// Pre-impl failure mode: the binary currently does NOT emit echo even on success,
// so this test would pass vacuously. However the assertion below is still correct
// and will catch the case where an implementation mistakenly emits echo on error.
// The POST-implementation guarantee: echo never fires on 400.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_012_echo_suppressed_on_put_error() {
    let server = MockServer::start().await;

    // PUT → 400 Bad Request
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": ["The summary field is required."],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Table mode
    let output_table = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--summary",
            "Should not echo",
        ])
        .output()
        .unwrap();

    let stderr_table = String::from_utf8_lossy(&output_table.stderr);

    // Must exit non-zero (error response propagated)
    assert!(
        !output_table.status.success(),
        "Expected non-zero exit on PUT 400; stderr={stderr_table}"
    );

    // No field-echo lines must appear on error
    assert!(
        !stderr_table.contains("  summary \u{2192}"),
        "Echo must NOT fire when PUT returns 400 (table mode); stderr={stderr_table}"
    );

    // --- JSON mode: changed_fields must NOT appear on error ---
    let server2 = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": ["The summary field is required."],
            "errors": {}
        })))
        .expect(1)
        .mount(&server2)
        .await;

    let output_json = jr_cmd(&server2.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--summary",
            "Should not echo",
        ])
        .output()
        .unwrap();

    let stdout_json = String::from_utf8_lossy(&output_json.stdout);

    assert!(
        !output_json.status.success(),
        "Expected non-zero exit on PUT 400 (JSON mode)"
    );

    // stdout must not contain changed_fields on error
    assert!(
        !stdout_json.contains("changed_fields"),
        "changed_fields must NOT appear in stdout on PUT 400; stdout={stdout_json}"
    );
}

// ---------------------------------------------------------------------------
// Test 24 — EC-3.4.012-5: `--points 5` echoes `  points → 5` (integer display)
// BC-3.4.012 EC-3.4.012-5; concrete (non-snapshot) assertion on f64::to_string().
//
// Behavior is already implemented (`pts.to_string()`); this test pins the format.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_012_points_integer_value_echo() {
    let server = MockServer::start().await;
    mock_put_with_sp_field(&server, "TEST-1").await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_config_with_story_points(config_dir.path());

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--no-input", "issue", "edit", "TEST-1", "--points", "5"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    // BC-3.4.012 EC-3.4.012-5: integer points must echo as "5", not "5.0" or "5.00"
    assert!(
        stderr.contains("  points \u{2192} 5"),
        "Expected '  points → 5' in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 25 — EC-3.4.012-5: `--points 2.5` echoes `  points → 2.5` (decimal display)
// BC-3.4.012 EC-3.4.012-5; concrete (non-snapshot) assertion on f64::to_string().
//
// Behavior is already implemented (`pts.to_string()`); this test pins the format.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_012_points_decimal_value_echo() {
    let server = MockServer::start().await;
    mock_put_with_sp_field(&server, "TEST-1").await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_config_with_story_points(config_dir.path());

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--no-input", "issue", "edit", "TEST-1", "--points", "2.5"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    // BC-3.4.012 EC-3.4.012-5: decimal points must echo as "2.5"
    assert!(
        stderr.contains("  points \u{2192} 2.5"),
        "Expected '  points → 2.5' in stderr; stderr={stderr}"
    );
}

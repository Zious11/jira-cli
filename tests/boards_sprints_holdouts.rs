//! BC-5/7 boards, sprints, and ADF rendering regression holdout suite — S-2.04
//!
//! Pins existing boards/sprints/ADF behavior across five holdout scenarios. All
//! tests pass on activation HEAD e9c2ba8 — no implementation changes required.
//! Future regressions in any of these paths will break this suite.
//!
//! Holdout coverage:
//! - AC-001 / H-040 / BC-5.2.005: `sprint current` truncation — three sub-cases
//!   (a) 35 issues → 30 shown + hint in stderr
//!   (b) 35 issues + `--all` → 35 shown, no hint
//!   (c) 10 issues → 10 shown, no hint
//! - AC-002 / H-041 / BC-5.2.007: `sprint add --output json` response has `sprint_id` key
//! - AC-003 / H-041 / BC-5.2.008: `sprint remove --output json` response has NO `sprint_id` key
//! - AC-004 / H-042 / BC-5.2.001: `sprint list --board 1` on kanban board → non-zero exit,
//!   stderr contains exact prefix `Sprint commands are only available for scrum boards`
//! - AC-005 / H-043 / BC-5.3.001: Team column appears when `team_field_id` configured
//!   AND at least one issue has a non-null team UUID (conjunctive gate — present case)
//! - AC-006 / H-043 / BC-5.3.002: Team column absent when all issues have null team UUID
//!   even when `team_field_id` is configured
//! - AC-007 / H-044 / BC-7.2.001: `issue view` on ADF doc with heading, paragraph,
//!   codeBlock, and mention nodes — heading + paragraph rendered, mention silently
//!   dropped, exit 0, no panic
//!
//! Test placement: all in `tests/boards_sprints_holdouts.rs`.
//! All functions under test are exercised through process-spawn (`assert_cmd`). No
//! inline `#[cfg(test)]` blocks are required because all relevant code is exercised
//! via the binary's public CLI surface.
//!
//! Truncation threshold: `DEFAULT_LIMIT = 30` (src/cli/mod.rs:744). If this
//! constant is bumped, the three H-040 cases below must be updated to match.
//!
//! Kanban error literal: the actual error message is
//! `"Sprint commands are only available for scrum boards. Board {id} is a {type} board."`
//! (src/cli/sprint.rs:80-85). The holdout asserts on the shared prefix so that a
//! change to only the suffix does not accidentally silence the guard.
//!
//! Teams cache: `$XDG_CACHE_HOME/jr/v1/default/teams.json`. `CachedTeam { id, name }`.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

/// Write a minimal `config.toml` under `<config_home>/jr/config.toml` with the
/// given `team_field_id` in `[profiles.default]` shape. The `url` field is a
/// placeholder; `JR_BASE_URL` overrides it in tests.
fn write_config_with_team_field(config_home: &std::path::Path, team_field_id: &str) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        format!(
            r#"
default_profile = "default"

[profiles.default]
url = "https://example.atlassian.net"
team_field_id = "{team_field_id}"
"#
        ),
    )
    .unwrap();
}

/// Write a `teams.json` cache entry to the canonical per-profile path
/// `$XDG_CACHE_HOME/jr/v1/default/teams.json`.
fn write_team_cache_entry(cache_home: &std::path::Path, team_id: &str, team_name: &str) {
    let teams_dir = cache_home.join("jr").join("v1").join("default");
    std::fs::create_dir_all(&teams_dir).unwrap();
    let cache = jr::cache::TeamCache {
        fetched_at: chrono::Utc::now(),
        teams: vec![jr::cache::CachedTeam {
            id: team_id.to_string(),
            name: team_name.to_string(),
        }],
    };
    std::fs::write(
        teams_dir.join("teams.json"),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();
}

/// Mount the three mocks required before `sprint current` fetches issues:
/// board auto-resolve (via board list), board config (scrum), active sprint list.
///
/// Uses board ID 42, sprint ID 100. Pass `--board 42` or `--project PROJ` to
/// bypass auto-discovery — tests that need explicit `--board` skip this helper.
async fn mount_scrum_sprint_prereqs(server: &MockServer) {
    // Board list for project PROJ — returns a single scrum board
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .and(query_param("type", "scrum"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::board_list_response(vec![common::fixtures::board_response(
                42,
                "PROJ Scrum Board",
                "scrum",
                "PROJ",
            )]),
        ))
        .mount(server)
        .await;

    // Board config → scrum
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(server)
        .await;

    // Active sprint list — one sprint with ID 100
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/sprint"))
        .and(query_param("state", "active"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::sprint_list_response(vec![common::fixtures::sprint(
                100, "Sprint 1", "active",
            )]),
        ))
        .mount(server)
        .await;
}

/// Build a list of `count` minimal issue JSON objects for sprint issue mocks.
fn make_sprint_issues(count: usize) -> Vec<Value> {
    (1..=count)
        .map(|i| {
            common::fixtures::issue_response(
                &format!("PROJ-{i}"),
                &format!("Issue {i}"),
                "In Progress",
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// AC-001 / H-040 / BC-5.2.005 — sprint current truncation (three sub-cases)
// ---------------------------------------------------------------------------

/// BC-5.2.005 postcondition (a): when a sprint has 35 issues and `jr sprint
/// current` is invoked with no explicit limit, exactly 30 issues are shown
/// (DEFAULT_LIMIT = 30) and stderr contains the truncation hint
/// `"Showing 30 results"`.
///
/// Pins `src/cli/sprint.rs::handle_current` has-more → hint branch.
/// If DEFAULT_LIMIT in `src/cli/mod.rs` is bumped, this test will fail,
/// making the threshold change explicit and visible to reviewers.
#[tokio::test]
async fn test_s_2_04_h_040_bc_5_2_005_truncates_to_30_with_hint_when_35_issues() {
    let server = MockServer::start().await;
    mount_scrum_sprint_prereqs(&server).await;

    // 35 issues in one page (total=35, maxResults=50 → page_has_more=false).
    // The early-stop path fires because 35 >= DEFAULT_LIMIT (30):
    //   result_has_more = 35 > 30 || false = true; truncate to 30.
    let issues = make_sprint_issues(35);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 35)),
        )
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "sprint", "current"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-5.2.005(a): command must exit 0; stderr: {stderr}"
    );

    // Count issue rows by looking for "PROJ-" keys in stdout
    let issue_count = stdout.lines().filter(|l| l.contains("PROJ-")).count();
    assert_eq!(
        issue_count, 30,
        "BC-5.2.005(a): expected exactly 30 issue rows in stdout (DEFAULT_LIMIT); \
         got {issue_count}; stdout: {stdout}"
    );

    assert!(
        stderr.contains("Showing 30 results"),
        "BC-5.2.005(a): stderr must contain the truncation hint 'Showing 30 results'; \
         got: {stderr}"
    );
}

/// BC-5.2.005 postcondition (b): when a sprint has 35 issues and `jr sprint
/// current --all` is invoked, all 35 issues are shown and no truncation hint
/// appears in stderr.
///
/// Pins that `--all` disables the DEFAULT_LIMIT early-stop in
/// `src/cli/mod.rs::resolve_effective_limit` (returns `None`).
#[tokio::test]
async fn test_s_2_04_h_040_bc_5_2_005_all_flag_shows_all_35_no_hint() {
    let server = MockServer::start().await;
    mount_scrum_sprint_prereqs(&server).await;

    let issues = make_sprint_issues(35);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 35)),
        )
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "sprint", "current", "--all"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-5.2.005(b): command must exit 0; stderr: {stderr}"
    );

    let issue_count = stdout.lines().filter(|l| l.contains("PROJ-")).count();
    assert_eq!(
        issue_count, 35,
        "BC-5.2.005(b): --all must show all 35 issues; got {issue_count}; stdout: {stdout}"
    );

    assert!(
        !stderr.contains("Showing 30 results"),
        "BC-5.2.005(b): --all must produce no truncation hint; stderr: {stderr}"
    );
    assert!(
        !stderr.contains("Use --limit or --all to see more"),
        "BC-5.2.005(b): --all must produce no truncation hint; stderr: {stderr}"
    );
}

/// BC-5.2.005 postcondition (c): when a sprint has 10 issues (under DEFAULT_LIMIT)
/// and `jr sprint current` is invoked, all 10 issues are shown and no truncation
/// hint appears in stderr.
///
/// Pins the has_more=false branch: no early-stop fires, result.has_more is false,
/// and `handle_current` skips the hint block.
#[tokio::test]
async fn test_s_2_04_h_040_bc_5_2_005_under_limit_shows_all_no_hint() {
    let server = MockServer::start().await;
    mount_scrum_sprint_prereqs(&server).await;

    let issues = make_sprint_issues(10);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 10)),
        )
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "sprint", "current"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-5.2.005(c): command must exit 0; stderr: {stderr}"
    );

    let issue_count = stdout.lines().filter(|l| l.contains("PROJ-")).count();
    assert_eq!(
        issue_count, 10,
        "BC-5.2.005(c): all 10 issues must be shown; got {issue_count}; stdout: {stdout}"
    );

    assert!(
        !stderr.contains("Showing 30 results"),
        "BC-5.2.005(c): under-limit sprint must produce no truncation hint; stderr: {stderr}"
    );
    assert!(
        !stderr.contains("Use --limit or --all to see more"),
        "BC-5.2.005(c): under-limit sprint must produce no truncation hint; stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 / H-041 / BC-5.2.007 — sprint add JSON response has sprint_id
// ---------------------------------------------------------------------------

/// BC-5.2.007 postcondition: `jr sprint add --sprint 100 TEST-1 TEST-2 --output json`
/// returns JSON with `sprint_id` key present and value equal to the sprint ID.
///
/// Pins `src/cli/sprint.rs::sprint_add_response` shape against accidental
/// "harmonization" with the remove response (which intentionally omits sprint_id).
#[tokio::test]
async fn test_s_2_04_h_041_bc_5_2_007_sprint_add_json_has_sprint_id() {
    let server = MockServer::start().await;

    // POST to add issues to sprint → 204 No Content
    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args([
            "--output", "json", "sprint", "add", "--sprint", "100", "TEST-1", "TEST-2",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-5.2.007: sprint add must exit 0; stderr: {stderr}"
    );

    let json: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("BC-5.2.007: stdout must be valid JSON; got: {stdout}; err: {e}")
    });

    assert!(
        json.get("sprint_id").is_some(),
        "BC-5.2.007: sprint add JSON response must contain 'sprint_id' key; got: {json}"
    );
    assert_eq!(
        json["sprint_id"],
        serde_json::json!(100),
        "BC-5.2.007: sprint_id must equal 100; got: {json}"
    );
    assert_eq!(
        json["added"],
        serde_json::json!(true),
        "BC-5.2.007: 'added' key must be true; got: {json}"
    );

    // Verify the issue keys are present in the response
    let issues = json["issues"]
        .as_array()
        .unwrap_or_else(|| panic!("BC-5.2.007: 'issues' key must be an array; got: {json}"));
    assert!(
        issues.contains(&serde_json::json!("TEST-1")),
        "BC-5.2.007: 'issues' must contain 'TEST-1'; got: {json}"
    );
    assert!(
        issues.contains(&serde_json::json!("TEST-2")),
        "BC-5.2.007: 'issues' must contain 'TEST-2'; got: {json}"
    );
}

// ---------------------------------------------------------------------------
// AC-003 / H-041 / BC-5.2.008 — sprint remove JSON response has NO sprint_id
// ---------------------------------------------------------------------------

/// BC-5.2.008 postcondition: `jr sprint remove TEST-1 TEST-2 --output json`
/// returns JSON with `sprint_id` key ABSENT. The remove response shape is
/// intentionally asymmetric to the add response.
///
/// Pins `src/cli/sprint.rs::sprint_remove_response` shape. If sprint_id were
/// ever added to the remove response to "harmonize" with add, this test fails
/// and forces an explicit review of the BC change.
#[tokio::test]
async fn test_s_2_04_h_041_bc_5_2_008_sprint_remove_json_has_no_sprint_id() {
    let server = MockServer::start().await;

    // POST to move issues to backlog → 204 No Content
    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/backlog/issue"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args(["--output", "json", "sprint", "remove", "TEST-1", "TEST-2"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "BC-5.2.008: sprint remove must exit 0; stderr: {stderr}"
    );

    let json: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("BC-5.2.008: stdout must be valid JSON; got: {stdout}; err: {e}")
    });

    assert!(
        json.get("sprint_id").is_none(),
        "BC-5.2.008: sprint remove JSON response must NOT contain 'sprint_id'; \
         H-041 pins this asymmetry with the add response; got: {json}"
    );
    assert_eq!(
        json["removed"],
        serde_json::json!(true),
        "BC-5.2.008: 'removed' key must be true; got: {json}"
    );

    let issues = json["issues"]
        .as_array()
        .unwrap_or_else(|| panic!("BC-5.2.008: 'issues' key must be an array; got: {json}"));
    assert!(
        issues.contains(&serde_json::json!("TEST-1")),
        "BC-5.2.008: 'issues' must contain 'TEST-1'; got: {json}"
    );
    assert!(
        issues.contains(&serde_json::json!("TEST-2")),
        "BC-5.2.008: 'issues' must contain 'TEST-2'; got: {json}"
    );
}

// ---------------------------------------------------------------------------
// AC-004 / H-042 / BC-5.2.001 — kanban board is a hard error for sprint commands
// ---------------------------------------------------------------------------

/// BC-5.2.001 postcondition: `jr sprint list --board 1` against a kanban board
/// exits non-zero and stderr contains the prefix
/// `Sprint commands are only available for scrum boards`.
///
/// The full error message is:
/// `Sprint commands are only available for scrum boards. Board {id} is a {type} board.`
/// (src/cli/sprint.rs:80-85). The holdout asserts on the shared prefix so that
/// a change to only the suffix does not silently drop the mandatory guard text.
///
/// Pins `src/cli/sprint.rs::resolve_scrum_board` kanban rejection branch.
/// Without this guard, a future refactor that removes the board_type check
/// would allow sprint commands to run against kanban boards, producing
/// either empty results or incorrect output.
#[tokio::test]
async fn test_s_2_04_h_042_bc_5_2_001_kanban_board_errors_on_sprint_list() {
    let server = MockServer::start().await;

    // Board config for board 1 → type: kanban
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/1/configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1,
            "name": "Kanban Board",
            "type": "kanban"
        })))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args(["sprint", "list", "--board", "1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "BC-5.2.001: sprint list on kanban board must exit non-zero; stderr: {stderr}"
    );
    assert!(
        stderr.contains("Sprint commands are only available for scrum boards"),
        "BC-5.2.001: stderr must contain the exact literal prefix \
         'Sprint commands are only available for scrum boards'; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-005 / H-043 / BC-5.3.001 — Team column present when field configured + UUID set
// ---------------------------------------------------------------------------

/// BC-5.3.001 postcondition: the Team column appears in `sprint current` output
/// when BOTH `team_field_id` is configured in the active profile AND at least one
/// issue in the sprint has a non-null team UUID. The UUID must resolve to the team
/// name via the teams.json cache.
///
/// Conjunctive gate: if either condition is missing, the column must NOT appear
/// (covered by BC-5.3.002 in AC-006 below).
///
/// Pins `src/cli/sprint.rs::handle_current` team_displays gate (line ~291-318).
/// Without this guard, removing the `any(|u| u.is_some())` check would
/// cause the Team column to appear for every sprint with `team_field_id`
/// configured, even when no issue has a team UUID.
#[tokio::test]
async fn test_s_2_04_h_043_bc_5_3_001_team_column_present_when_field_and_uuid_set() {
    let server = MockServer::start().await;
    mount_scrum_sprint_prereqs(&server).await;

    // Use a team field ID and UUID that don't appear in any summary text,
    // so the "Team" column assertion isn't accidentally satisfied by a
    // summary match (unlike some tests in team_column_parity.rs).
    let team_field_id = "customfield_10200";
    let team_uuid = "uuid-engr-hold-043";
    let team_name = "Engineering";

    // One issue has the team UUID set
    let mut issue_with_team = common::fixtures::issue_response("PROJ-1", "Work item A", "To Do");
    issue_with_team["fields"][team_field_id] = serde_json::json!(team_uuid);

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::sprint_issues_response(vec![issue_with_team], 1),
        ))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Write team cache with the matching UUID → name entry
    write_team_cache_entry(cache_dir.path(), team_uuid, team_name);

    // Write config with team_field_id configured
    write_config_with_team_field(config_dir.path(), team_field_id);

    jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "sprint", "current"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Team").and(predicate::str::contains("Engineering")));
}

// ---------------------------------------------------------------------------
// AC-006 / H-043 / BC-5.3.002 — Team column absent when all issues have null UUID
// ---------------------------------------------------------------------------

/// BC-5.3.002 postcondition: the Team column is absent from `sprint current` output
/// when `team_field_id` IS configured but NO issue in the sprint has a non-null
/// team UUID. The absence check uses a negative predicate on `"Team"` in stdout.
///
/// Positive anchors (`"Summary"`, `"Assignee"`) ensure the table rendered
/// correctly and the negative result is not vacuously true from an empty output.
///
/// Pins the conjunctive gate: `team_field_id` configured is NECESSARY but NOT
/// SUFFICIENT for the Team column to appear — at least one issue must have a
/// populated UUID as well.
#[tokio::test]
async fn test_s_2_04_h_043_bc_5_3_002_team_column_absent_when_no_uuid_set() {
    let server = MockServer::start().await;
    mount_scrum_sprint_prereqs(&server).await;

    let team_field_id = "customfield_10200";

    // Issues have NO team UUID — field is absent from each issue's extra fields
    let issues = vec![
        common::fixtures::issue_response("PROJ-1", "No team alpha", "To Do"),
        common::fixtures::issue_response("PROJ-2", "No team beta", "In Progress"),
    ];

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 2)),
        )
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // team_field_id IS configured — but no issue carries a UUID
    write_config_with_team_field(config_dir.path(), team_field_id);

    jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "sprint", "current"])
        .assert()
        .success()
        // Positive anchors: table rendered, not vacuously absent
        .stdout(predicate::str::contains("No team alpha"))
        .stdout(predicate::str::contains("Summary"))
        .stdout(predicate::str::contains("Assignee"))
        // Negative gate: Team column must be absent
        .stdout(predicate::str::contains("Team").not());
}

// ---------------------------------------------------------------------------
// AC-007 / H-044 / BC-7.2.001 — ADF rendering: heading/paragraph shown, mention dropped
// ---------------------------------------------------------------------------

/// BC-7.2.001 postcondition: `jr issue view PROJ-1` on an issue with an ADF
/// description containing heading, paragraph, codeBlock, and mention nodes:
/// - exits 0 (no panic)
/// - stdout contains the heading text
/// - stdout contains the paragraph text
/// - stdout does NOT contain the mention text (silently dropped per current behavior)
///
/// Current behavior: mention nodes have no `content` child array, so the
/// `_ =>` catch-all in `src/adf.rs::AdfRenderer::render_node` (line ~531-540)
/// finds no content to recurse into and silently drops the node. The ADF `attrs`
/// field (including `text`) is not rendered. This is the correct behavior per
/// issue #202 (NFR-O-I deferred to Wave 3).
///
/// KNOWN-GAP: H-044. When NFR-O-I is implemented (mention → @displayName),
/// change this assertion to: assert!(stdout.contains("@John Smith"));
///
/// Pins `src/adf.rs::AdfRenderer::render_node` mention drop (catch-all branch).
/// Without this guard, a future change that adds a `"mention"` match arm that
/// panics or outputs debug text would not be caught until user-facing regression.
#[tokio::test]
async fn test_s_2_04_h_044_bc_7_2_001_adf_renders_heading_paragraph_drops_mention() {
    let server = MockServer::start().await;

    // ADF document with:
    //   1. heading (level 1): "My Heading"
    //   2. paragraph: "Some paragraph text"
    //   3. codeBlock: "let x = 42;"
    //   4. mention: attrs.id = "account:abc", attrs.text = "John Smith"
    //      (mention has no content array — will be silently dropped)
    let adf_description = serde_json::json!({
        "version": 1,
        "type": "doc",
        "content": [
            {
                "type": "heading",
                "attrs": { "level": 1 },
                "content": [
                    { "type": "text", "text": "My Heading" }
                ]
            },
            {
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "Some paragraph text" }
                ]
            },
            {
                "type": "codeBlock",
                "attrs": { "language": "rust" },
                "content": [
                    { "type": "text", "text": "let x = 42;" }
                ]
            },
            {
                "type": "paragraph",
                "content": [
                    {
                        "type": "mention",
                        "attrs": {
                            "id": "account:abc-123",
                            "text": "John Smith",
                            "accessLevel": "APPLICATION"
                        }
                    }
                ]
            }
        ]
    });

    // Build a complete issue response with the ADF description
    let issue_body = serde_json::json!({
        "key": "PROJ-1",
        "fields": {
            "summary": "Issue with ADF description",
            "description": adf_description,
            "status": { "name": "To Do" },
            "issuetype": { "name": "Task" },
            "priority": { "name": "Medium" },
            "assignee": null,
            "reporter": null,
            "project": { "key": "PROJ", "name": "PROJ Project" },
            "labels": [],
            "issuelinks": []
        }
    });

    // issue view calls GET /rest/api/3/issue/{key}
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_body))
        .expect(1)
        .mount(&server)
        .await;

    // CMDB fields discovery — return empty so no asset enrichment is attempted
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args(["issue", "view", "PROJ-1"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Exit 0 — no panic
    assert!(
        output.status.success(),
        "BC-7.2.001: issue view must exit 0 on ADF with mixed node types; stderr: {stderr}"
    );

    // Heading text must appear
    assert!(
        stdout.contains("My Heading"),
        "BC-7.2.001: heading text 'My Heading' must appear in stdout; got: {stdout}"
    );

    // Paragraph text must appear
    assert!(
        stdout.contains("Some paragraph text"),
        "BC-7.2.001: paragraph text 'Some paragraph text' must appear in stdout; got: {stdout}"
    );

    // Mention text must NOT appear in stdout — silently dropped per current behavior.
    // KNOWN-GAP: H-044. When NFR-O-I is implemented (mention → @displayName),
    // change this assertion to: assert!(stdout.contains("@John Smith"));
    assert!(
        !stdout.contains("John Smith"),
        "BC-7.2.001: mention text 'John Smith' must NOT appear in stdout \
         (silently dropped per current behavior — NFR-O-I deferred); got: {stdout}"
    );
}

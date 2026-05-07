//! Integration tests for BC-6.3.001 — multi-profile field routing (S-0.04).
//!
//! Holdout H-NEW-MP-001: these tests MUST FAIL at the pre-fix HEAD and MUST
//! PASS after the 14 read-site migrations in the implementation story.
//!
//! Each test is named `test_bc_6_3_001_<descriptive>` per the project TDD
//! naming convention (lowercase snake_case to avoid non_snake_case warnings).

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── Config helpers ─────────────────────────────────────────────────────────

/// Write a two-profile config: `prod` uses prod field IDs, `sandbox` uses
/// sandbox field IDs.  The `[fields]` block is intentionally ABSENT — this
/// mirrors the post-`save_global` state where the legacy block has been
/// dropped.  Pre-fix code reads from `config.global.fields` (which is
/// empty/default here) and therefore returns `None` for every field ID.
/// Post-fix code reads from `config.active_profile()` and returns the correct
/// per-profile value.
fn write_two_profile_config(config_home: &std::path::Path) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        r#"
default_profile = "prod"

[profiles.prod]
url = "https://prod.atlassian.net"
story_points_field_id = "customfield_10005"
team_field_id = "customfield_10010"

[profiles.sandbox]
url = "https://sandbox.atlassian.net"
story_points_field_id = "customfield_10099"
team_field_id = "customfield_10199"
"#,
    )
    .unwrap();
}

/// Write a single-profile config (default profile) that has field IDs only in
/// `[profiles.default]`, NOT in the legacy `[fields]` block.  This simulates
/// the post-save-round-trip state for a single-profile user.
fn write_single_profile_config_no_legacy_fields(
    config_home: &std::path::Path,
    sp_field_id: &str,
    team_field_id: &str,
) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        format!(
            r#"
default_profile = "default"

[profiles.default]
url = "https://acme.atlassian.net"
story_points_field_id = "{sp_field_id}"
team_field_id = "{team_field_id}"
"#
        ),
    )
    .unwrap();
}

/// Build a `jr` command targeting a wiremock server, pointing XDG dirs at the
/// temp dirs, and appending `--no-input`.
fn jr_cmd(server_uri: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("XDG_CONFIG_HOME", config_dir)
        .arg("--no-input");
    cmd
}

// ── Shared mock helpers ────────────────────────────────────────────────────

/// Mount the three prereq GET mocks for `sprint current` (board → config →
/// active sprint).
async fn mount_sprint_prereqs(server: &MockServer) {
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

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(server)
        .await;

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

// ── AC-001: --profile sandbox uses sandbox field IDs ──────────────────────

/// BC-6.3.001 postcondition: given two profiles where `prod` has
/// `story_points_field_id = "customfield_10005"` and `sandbox` has
/// `story_points_field_id = "customfield_10099"`, running
/// `jr --profile sandbox issue create --story-points 5 ...` MUST produce a
/// POST body containing `"customfield_10099": 5` and NOT `"customfield_10005"`.
///
/// Pre-fix FAILS: code reads from `config.global.fields.story_points_field_id`
/// which falls back to `None` (legacy block absent), so the field is never
/// sent — or, if the prod [fields] block is present, sends the prod ID.
/// Post-fix PASSES: code reads from `config.active_profile()`, which for
/// profile=sandbox returns `customfield_10099`.
///
/// This is holdout H-NEW-MP-001.
#[tokio::test]
async fn test_bc_6_3_001_sandbox_profile_uses_sandbox_story_points_field_id() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    write_two_profile_config(config_dir.path());

    // POST /rest/api/3/issue — capture payload to assert field IDs.
    // We use body_partial_json to assert the sandbox ID is present.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(json!({
            "fields": {
                "customfield_10099": 5.0
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "10001",
            "key": "PROJ-1",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    // GET /rest/api/3/field — needed by compose_extra_fields on JSON output
    // path; use empty array so it doesn't interfere.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--profile",
            "sandbox",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Story",
            "--summary",
            "Holdout H-NEW-MP-001",
            "--points",
            "5",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // The mock with .expect(1) verifies the sandbox field ID was used.
    // Asserting success also rules out a "field not configured" error that
    // would occur pre-fix when global.fields is empty and active_profile()
    // is not yet consulted.
    assert!(
        output.status.success(),
        "expected success with sandbox profile; stderr: {stderr}, stdout: {stdout}"
    );

    // Additional negative assertion: the body must NOT contain the prod field ID.
    // We inspect received_requests to verify the prod ID is absent.
    let requests = server
        .received_requests()
        .await
        .expect("received_requests recording");
    let post_req = requests
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/issue" && r.method == wiremock::http::Method::POST)
        .expect("POST /rest/api/3/issue must have been called");
    let body = String::from_utf8_lossy(&post_req.body);
    assert!(
        body.contains("customfield_10099"),
        "POST body must contain sandbox story-points field ID (customfield_10099); body: {body}"
    );
    assert!(
        !body.contains("customfield_10005"),
        "POST body must NOT contain prod story-points field ID (customfield_10005); body: {body}"
    );
}

// ── AC-002: --points column populated after save round-trip ───────────────

/// BC-6.3.001 postcondition: after a save round-trip (legacy `[fields]` block
/// absent, field IDs only in `[profiles.default]`), `jr issue list --points`
/// must still show the Points column.
///
/// Pre-fix FAILS: code reads `config.global.fields.story_points_field_id`
/// which is `None` when the `[fields]` block has been dropped; the field is
/// not requested in the search API call; the Points column is blank / absent.
/// Post-fix PASSES: code reads `config.active_profile().story_points_field_id`
/// which returns the correct value from `[profiles.default]`.
#[tokio::test]
async fn test_bc_6_3_001_points_column_present_after_save_round_trip() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Config with field IDs only in [profiles.default], no [fields] block.
    write_single_profile_config_no_legacy_fields(
        config_dir.path(),
        "customfield_10031",
        "customfield_10100",
    );

    // GET /rest/api/3/project/PROJ — project_exists() check in list.rs:191.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"key": "PROJ"})))
        .mount(&server)
        .await;

    // One issue with story points populated under customfield_10031.
    let issue = {
        let mut base = common::fixtures::issue_response("PROJ-1", "Has points", "In Progress");
        base["fields"]["customfield_10031"] = json!(8.0);
        base
    };

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(vec![issue])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "issue", "list", "--points"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "expected success; stderr: {stderr}, stdout: {stdout}"
    );

    // The Points column must appear in the table output.
    assert!(
        stdout.contains("Points"),
        "Points column header must be present; stdout: {stdout}, stderr: {stderr}"
    );

    // Verify the field ID was actually requested so the column is populated
    // from real data, not a degenerate empty column.
    let requests = server
        .received_requests()
        .await
        .expect("received_requests recording");
    let search_req = requests
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/search/jql")
        .expect("POST /rest/api/3/search/jql must have been made");
    let body = String::from_utf8_lossy(&search_req.body);
    assert!(
        body.contains("customfield_10031"),
        "search call must request the SP field ID (customfield_10031); body: {body}"
    );
}

// ── AC-003: sprint current shows team/points after save round-trip ─────────

/// BC-6.3.001 postcondition: `jr sprint current` must include team and points
/// columns when field IDs live only in `[profiles.default]` (no `[fields]`).
///
/// Pre-fix FAILS: both `config.global.fields.story_points_field_id` and
/// `config.global.fields.team_field_id` are `None`; the sprint issues API call
/// does not request those fields; columns are absent.
/// Post-fix PASSES: code reads from `config.active_profile()`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_6_3_001_sprint_current_shows_team_and_points_after_save_round_trip() {
    let server = MockServer::start().await;
    mount_sprint_prereqs(&server).await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_single_profile_config_no_legacy_fields(
        config_dir.path(),
        "customfield_10031", // SP
        "customfield_10100", // team
    );

    // Write a team cache so the UUID can be resolved to a display name.
    // Write to the canonical v1/<profile>/ path to match production cache layout.
    {
        let teams_dir = cache_dir.path().join("jr").join("v1").join("default");
        std::fs::create_dir_all(&teams_dir).unwrap();
        let cache = jr::cache::TeamCache {
            fetched_at: chrono::Utc::now(),
            teams: vec![jr::cache::CachedTeam {
                id: "team-uuid-alpha".into(),
                name: "Alpha".into(),
            }],
        };
        std::fs::write(
            teams_dir.join("teams.json"),
            serde_json::to_string(&cache).unwrap(),
        )
        .unwrap();
    }

    // Issue with both story points and team UUID.
    let issue = {
        let mut base = common::fixtures::issue_response("PROJ-1", "Sprint work", "In Progress");
        base["fields"]["customfield_10031"] = json!(3.0);
        base["fields"]["customfield_10100"] = json!("team-uuid-alpha");
        base
    };

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(vec![issue], 1)),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "sprint", "current"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "expected success; stderr: {stderr}, stdout: {stdout}"
    );

    assert!(
        stdout.contains("Points"),
        "Points column must be present in sprint current output; stdout: {stdout}"
    );
    assert!(
        stdout.contains("Team"),
        "Team column must be present in sprint current output; stdout: {stdout}"
    );

    // Verify both field IDs were requested in the API call.
    let requests = server
        .received_requests()
        .await
        .expect("received_requests recording");
    let sprint_req = requests
        .iter()
        .find(|r| r.url.path() == "/rest/agile/1.0/sprint/100/issue")
        .expect("sprint/100/issue GET must have been made");
    let query = sprint_req.url.query().unwrap_or("");
    assert!(
        query.contains("customfield_10031"),
        "sprint call must request SP field; got query: {query}"
    );
    assert!(
        query.contains("customfield_10100"),
        "sprint call must request team field; got query: {query}"
    );
}

// ── AC-004: board view shows team after save round-trip ───────────────────

/// BC-6.3.001 postcondition: `jr board view` must include a Team column when
/// `team_field_id` lives only in `[profiles.default]` (no `[fields]` block).
///
/// Pre-fix FAILS: `config.global.fields.team_field_id` is `None`; the board
/// view search call does not request the team field; Team column absent.
/// Post-fix PASSES: code reads from `config.active_profile()`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_6_3_001_board_view_shows_team_after_save_round_trip() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_single_profile_config_no_legacy_fields(
        config_dir.path(),
        "customfield_10031",
        "customfield_10100",
    );

    // Write a team cache to the canonical v1/<profile>/ path.
    {
        let teams_dir = cache_dir.path().join("jr").join("v1").join("default");
        std::fs::create_dir_all(&teams_dir).unwrap();
        let cache = jr::cache::TeamCache {
            fetched_at: chrono::Utc::now(),
            teams: vec![jr::cache::CachedTeam {
                id: "team-uuid-beta".into(),
                name: "Beta".into(),
            }],
        };
        std::fs::write(
            teams_dir.join("teams.json"),
            serde_json::to_string(&cache).unwrap(),
        )
        .unwrap();
    }

    // Kanban board prereqs: board list → board config.
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::board_list_response(vec![common::fixtures::board_response(
                42,
                "PROJ Kanban Board",
                "kanban",
                "PROJ",
            )]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("kanban")),
        )
        .mount(&server)
        .await;

    // Issue with team UUID in the team field.
    let issue = {
        let mut base = common::fixtures::issue_response("PROJ-10", "Board ticket", "In Progress");
        base["fields"]["customfield_10100"] = json!("team-uuid-beta");
        base
    };

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(vec![issue])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "board", "view"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "expected success; stderr: {stderr}, stdout: {stdout}"
    );

    assert!(
        stdout.contains("Team"),
        "Team column must be present in board view output; stdout: {stdout}"
    );
    assert!(
        stdout.contains("Beta"),
        "Resolved team name 'Beta' must appear; stdout: {stdout}"
    );

    // Verify team field was requested in the API call.
    let requests = server
        .received_requests()
        .await
        .expect("received_requests recording");
    let search_req = requests
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/search/jql")
        .expect("POST /rest/api/3/search/jql must have been made");
    let body = String::from_utf8_lossy(&search_req.body);
    assert!(
        body.contains("customfield_10100"),
        "board view must request the team custom field (customfield_10100); body: {body}"
    );
}

// ── AC-005: error message references [profiles.<name>] not [fields] ────────

/// BC-6.3.001 postcondition (error message): when `story_points_field_id` is
/// missing for the active profile, the error message must reference
/// `[profiles.<name>]` not the deprecated `[fields]` section.
///
/// Pre-fix FAILS: `resolve_story_points_field_id()` emits
///   "set story_points_field_id under [fields] in ~/.config/jr/config.toml"
/// Post-fix PASSES: error says
///   "set story_points_field_id under [profiles.<name>] in ..."
#[tokio::test]
async fn test_bc_6_3_001_error_message_references_profiles_section_not_fields() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Config with NO story_points_field_id for the default profile.
    let conf_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        r#"
default_profile = "default"

[profiles.default]
url = "https://acme.atlassian.net"
"#,
    )
    .unwrap();

    // Mount a minimal GET /rest/api/3/field so any CMDB discovery doesn't fail
    // first.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .mount(&server)
        .await;

    // Run `jr issue create --points 5` which calls
    // `resolve_story_points_field_id(config)` — the path that emits the error.
    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Story",
            "--summary",
            "Error message test",
            "--points",
            "5",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // The command must fail (no field ID configured).
    assert!(
        !output.status.success(),
        "expected failure when story_points_field_id is not configured; stderr: {stderr}"
    );

    // Post-fix error message: must say [profiles.<name>]
    assert!(
        stderr.contains("[profiles."),
        "error must reference [profiles.<name>]; got stderr: {stderr}"
    );

    // Pre-fix error message: must NOT say [fields]
    assert!(
        !stderr.contains("[fields]"),
        "error must NOT reference deprecated [fields] section; got stderr: {stderr}"
    );
}

// ── AC-006: list.rs resolve_show_points warning updated ───────────────────

/// BC-6.3.001 postcondition (AC-006 — list.rs:593-604 warning text): when
/// `--points` is passed but `story_points_field_id` is missing from the
/// active profile, the warning on stderr must say
/// "set story_points_field_id under [profiles.<name>]" not the old wording
/// "set [fields].story_points_field_id in ...".
///
/// Pre-fix FAILS: `resolve_show_points` at list.rs:599-600 emits old wording.
/// Post-fix PASSES: wording updated to reference [profiles.<name>].
#[tokio::test]
async fn test_bc_6_3_001_list_points_warning_references_profiles_section() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Config with NO story_points_field_id.
    let conf_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        r#"
default_profile = "default"

[profiles.default]
url = "https://acme.atlassian.net"
"#,
    )
    .unwrap();

    // GET /rest/api/3/project/PROJ — project_exists() check in list.rs:191.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"key": "PROJ"})))
        .mount(&server)
        .await;

    // Minimal search response so the list command does not error on the API
    // call (we want it to succeed overall — points simply silently ignored
    // with a warning because the field isn't configured).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                "PROJ-1",
                "No points issue",
                "To Do",
            )]),
        ))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "issue", "list", "--points"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // The command itself succeeds (--points is silently ignored with a warning).
    assert!(
        output.status.success(),
        "expected success (--points degraded gracefully); stderr: {stderr}, stdout: {stdout}"
    );

    // Post-fix: warning must mention [profiles.
    assert!(
        stderr.contains("[profiles."),
        "warning must reference [profiles.<name>]; got stderr: {stderr}"
    );

    // Pre-fix: warning says [fields].story_points_field_id — must NOT appear.
    assert!(
        !stderr.contains("[fields]"),
        "warning must NOT reference deprecated [fields] section; got stderr: {stderr}"
    );
}

// ── Unit-level: active_profile() returns per-profile field IDs ─────────────

/// BC-6.3.001 unit postcondition: `Config::active_profile()` for profile
/// "sandbox" returns the sandbox-specific field IDs, not the prod ones.
/// This is a pure library test that does not spawn a process.
///
/// Pre-fix note: this test is likely to PASS even pre-fix because
/// `active_profile()` in config.rs is already correct; the bug is at the 14
/// call sites that read from `config.global.fields` instead of calling
/// `config.active_profile()`. This test documents the expected contract and
/// will alert if `active_profile()` regresses.
#[test]
fn test_bc_6_3_001_active_profile_returns_per_profile_field_ids() {
    use jr::config::{Config, GlobalConfig, ProfileConfig, ProjectConfig};
    use std::collections::BTreeMap;

    let mut profiles = BTreeMap::new();
    profiles.insert(
        "prod".to_string(),
        ProfileConfig {
            story_points_field_id: Some("customfield_10005".into()),
            team_field_id: Some("customfield_10010".into()),
            url: Some("https://prod.atlassian.net".into()),
            ..ProfileConfig::default()
        },
    );
    profiles.insert(
        "sandbox".to_string(),
        ProfileConfig {
            story_points_field_id: Some("customfield_10099".into()),
            team_field_id: Some("customfield_10199".into()),
            url: Some("https://sandbox.atlassian.net".into()),
            ..ProfileConfig::default()
        },
    );

    let config = Config {
        global: GlobalConfig {
            default_profile: Some("prod".into()),
            profiles,
            ..GlobalConfig::default()
        },
        project: ProjectConfig::default(),
        active_profile_name: "sandbox".into(),
    };

    let profile = config.active_profile();
    assert_eq!(
        profile.story_points_field_id.as_deref(),
        Some("customfield_10099"),
        "active_profile() must return sandbox SP field ID"
    );
    assert_eq!(
        profile.team_field_id.as_deref(),
        Some("customfield_10199"),
        "active_profile() must return sandbox team field ID"
    );
}

/// BC-6.3.001 unit postcondition: after serializing a `GlobalConfig` that has
/// per-profile field IDs (the post-save state), reloading with
/// `active_profile_name = "sandbox"` still returns the correct field IDs.
/// Simulates the save round-trip at the `Config` level without process
/// invocation.
#[test]
fn test_bc_6_3_001_field_ids_survive_toml_save_round_trip() {
    use figment::{
        Figment,
        providers::{Format, Toml},
    };
    use jr::config::{GlobalConfig, ProfileConfig};
    use std::collections::BTreeMap;

    let mut profiles = BTreeMap::new();
    profiles.insert(
        "sandbox".to_string(),
        ProfileConfig {
            story_points_field_id: Some("customfield_10099".into()),
            team_field_id: Some("customfield_10199".into()),
            url: Some("https://sandbox.atlassian.net".into()),
            ..ProfileConfig::default()
        },
    );
    let global = GlobalConfig {
        default_profile: Some("sandbox".into()),
        profiles,
        ..GlobalConfig::default()
    };

    // Serialize (save_global would do this via toml::to_string_pretty).
    let serialized =
        toml::to_string_pretty(&global).expect("GlobalConfig must serialize to valid TOML");

    // Verify the legacy [fields] block is absent (it is skip_serializing).
    assert!(
        !serialized.contains("[fields]"),
        "serialized config must not contain [fields] block; got:\n{serialized}"
    );

    // Reload from the serialized TOML.
    let reloaded: GlobalConfig = Figment::new()
        .merge(Toml::string(&serialized))
        .extract()
        .expect("GlobalConfig must deserialize from serialized TOML");

    let p = reloaded
        .profiles
        .get("sandbox")
        .expect("sandbox profile must survive round-trip");
    assert_eq!(
        p.story_points_field_id.as_deref(),
        Some("customfield_10099"),
        "SP field ID must survive TOML round-trip"
    );
    assert_eq!(
        p.team_field_id.as_deref(),
        Some("customfield_10199"),
        "team field ID must survive TOML round-trip"
    );
}

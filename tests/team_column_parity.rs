//! End-to-end coverage for the Team column parity in `jr sprint current` and
//! `jr board view` (#246). Mirrors the gating rules from `issue list` (#191):
//! the column appears only when `team_field_id` is configured AND at least
//! one returned issue carries a populated team UUID.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_TEAM_FIELD_ID: &str = "customfield_10100";

/// Build a `jr` command with XDG cache/config dir overrides so tests can
/// pre-populate the team cache and a config.toml with `team_field_id` set.
/// Matches the `jr_cmd_with_xdg` pattern in tests/cli_handler.rs but kept
/// local to avoid coupling this test file to cli_handler's internals.
fn jr_cmd_with_xdg(
    server_uri: &str,
    cache_dir: &std::path::Path,
    config_dir: &std::path::Path,
) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("XDG_CONFIG_HOME", config_dir)
        .arg("--no-input")
        .arg("--output")
        .arg("table");
    cmd
}

fn write_team_cache(cache_home: &std::path::Path) {
    let teams_dir = cache_home.join("jr");
    std::fs::create_dir_all(&teams_dir).unwrap();
    let cache = jr::cache::TeamCache {
        fetched_at: chrono::Utc::now(),
        teams: vec![
            jr::cache::CachedTeam {
                id: "team-uuid-platform".into(),
                name: "Platform".into(),
            },
            jr::cache::CachedTeam {
                id: "team-uuid-growth".into(),
                name: "Growth".into(),
            },
        ],
    };
    std::fs::write(
        teams_dir.join("teams.json"),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();
}

fn write_config_with_team_field(config_home: &std::path::Path) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        format!("[fields]\nteam_field_id = \"{TEST_TEAM_FIELD_ID}\"\n"),
    )
    .unwrap();
}

fn write_config_without_team_field(config_home: &std::path::Path) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(conf_dir.join("config.toml"), "[fields]\n").unwrap();
}

/// Build an issue with a team UUID set under `fields.<team_field_id>`.
fn issue_with_team(key: &str, summary: &str, status: &str, team_uuid: &str) -> Value {
    let mut issue = common::fixtures::issue_response(key, summary, status);
    issue["fields"][TEST_TEAM_FIELD_ID] = json!(team_uuid);
    issue
}

/// Mount the three prereq GET mocks needed before `sprint current` fetches
/// issues: board auto-resolve → board config (scrum) → active sprint list.
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

/// `jr sprint current` shows a Team column + resolved team name when
/// `team_field_id` is configured and at least one issue carries a team UUID.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sprint_current_shows_team_column_when_populated() {
    let server = MockServer::start().await;
    mount_sprint_prereqs(&server).await;

    let issues = vec![
        issue_with_team(
            "PROJ-1",
            "Platform work",
            "In Progress",
            "team-uuid-platform",
        ),
        issue_with_team("PROJ-2", "Growth work", "In Progress", "team-uuid-growth"),
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
    write_team_cache(cache_dir.path());
    write_config_with_team_field(config_dir.path());

    jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "sprint", "current"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Team"))
        .stdout(predicate::str::contains("Platform"))
        .stdout(predicate::str::contains("Growth"));
}

/// `jr sprint current` omits the Team column when `team_field_id` is not
/// configured, regardless of whether issues carry team UUIDs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sprint_current_omits_team_column_when_field_unconfigured() {
    let server = MockServer::start().await;
    mount_sprint_prereqs(&server).await;

    let issues = vec![issue_with_team(
        "PROJ-1",
        "Platform work",
        "In Progress",
        "team-uuid-platform",
    )];
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 1)),
        )
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_config_without_team_field(config_dir.path());

    jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "sprint", "current"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Platform work"))
        .stdout(predicate::str::contains("│ Team").not());
}

/// `jr sprint current` omits the Team column when `team_field_id` IS
/// configured but no issue in the sprint has a populated team.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sprint_current_omits_team_column_when_no_issue_has_team() {
    let server = MockServer::start().await;
    mount_sprint_prereqs(&server).await;

    // Plain issues — no team field set.
    let issues = vec![common::fixtures::issue_response(
        "PROJ-1",
        "Untagged work",
        "In Progress",
    )];
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 1)),
        )
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_team_cache(cache_dir.path());
    write_config_with_team_field(config_dir.path());

    jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "sprint", "current"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Untagged work"))
        .stdout(predicate::str::contains("│ Team").not());
}

/// Mount the two prereq mocks for `board view` against a kanban board:
/// board auto-resolve → board config (kanban).
async fn mount_kanban_board_prereqs(server: &MockServer) {
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
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("kanban")),
        )
        .mount(server)
        .await;
}

/// `jr board view` (kanban path) shows a Team column when the config has
/// `team_field_id` and at least one returned issue has a populated team.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn board_view_kanban_shows_team_column_when_populated() {
    let server = MockServer::start().await;
    mount_kanban_board_prereqs(&server).await;

    let issues = vec![
        issue_with_team("PROJ-10", "Platform ticket", "To Do", "team-uuid-platform"),
        issue_with_team(
            "PROJ-11",
            "Growth ticket",
            "In Progress",
            "team-uuid-growth",
        ),
    ];
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(issues)),
        )
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_team_cache(cache_dir.path());
    write_config_with_team_field(config_dir.path());

    jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "board", "view"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Team"))
        .stdout(predicate::str::contains("Platform"))
        .stdout(predicate::str::contains("Growth"));
}

/// `jr board view` (kanban) omits the Team column when configured but no
/// issue has a populated team UUID.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn board_view_kanban_omits_team_column_when_no_issue_has_team() {
    let server = MockServer::start().await;
    mount_kanban_board_prereqs(&server).await;

    let issues = vec![common::fixtures::issue_response(
        "PROJ-10",
        "Untagged ticket",
        "To Do",
    )];
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(issues)),
        )
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_team_cache(cache_dir.path());
    write_config_with_team_field(config_dir.path());

    jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--project", "PROJ", "board", "view"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Untagged ticket"))
        .stdout(predicate::str::contains("│ Team").not());
}

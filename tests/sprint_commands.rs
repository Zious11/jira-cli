#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper: build N issues for testing.
fn make_issues(count: usize) -> Vec<serde_json::Value> {
    (1..=count)
        .map(|i| {
            common::fixtures::issue_response(
                &format!("TEST-{}", i),
                &format!("Issue {}", i),
                "In Progress",
            )
        })
        .collect()
}

/// Mount prereq mocks (board list, board config, active sprint) on the server.
async fn mount_prereqs(server: &MockServer) {
    // Board auto-resolve: list boards for project PROJ, type=scrum → 1 board
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

    // Active sprint list → one sprint
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

#[tokio::test]
async fn sprint_current_default_limit_caps_at_30() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    let issues = make_issues(35);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 35)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show exactly 30 issues (default limit)
    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 30, "Expected 30 issues, got {issue_count}");

    // Should show "more results" hint
    assert!(
        stderr.contains("Showing 30 results"),
        "Expected 'Showing 30 results' in stderr, got: {stderr}"
    );
}

#[tokio::test]
async fn sprint_current_limit_flag() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    let issues = make_issues(20);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 20)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .arg("--limit")
        .arg("5")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 5, "Expected 5 issues, got {issue_count}");

    assert!(
        stderr.contains("Showing 5 results"),
        "Expected 'Showing 5 results' in stderr, got: {stderr}"
    );
}

#[tokio::test]
async fn sprint_current_all_flag_returns_everything() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    let issues = make_issues(35);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 35)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .arg("--all")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 35, "Expected 35 issues, got {issue_count}");

    assert!(
        !stderr.contains("Showing"),
        "Should NOT show 'Showing' hint with --all, got: {stderr}"
    );
}

#[tokio::test]
async fn sprint_current_under_limit_no_hint() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    let issues = make_issues(10);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 10)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 10, "Expected 10 issues, got {issue_count}");

    assert!(
        !stderr.contains("Showing"),
        "Should NOT show hint when under limit, got: {stderr}"
    );
}

#[test]
fn sprint_current_limit_and_all_conflict() {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.arg("sprint")
        .arg("current")
        .arg("--limit")
        .arg("3")
        .arg("--all");

    cmd.assert().failure().code(2);
}

#[tokio::test]
async fn sprint_add_with_sprint_id() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["sprint", "add", "--sprint", "100", "FOO-1", "FOO-2"])
        .output()
        .unwrap();

    assert!(output.status.success(), "Expected success, got: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Added 2 issue(s) to sprint 100"),
        "Expected success message, got: {stdout}"
    );
}

#[tokio::test]
async fn sprint_add_json_output() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/sprint/200/issue"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--output", "json",
            "sprint", "add", "--sprint", "200", "BAR-1",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "Expected success, got: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["sprint_id"], 200);
    assert_eq!(parsed["issues"], serde_json::json!(["BAR-1"]));
    assert_eq!(parsed["added"], true);
}

#[tokio::test]
async fn sprint_remove_moves_to_backlog() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/backlog/issue"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["sprint", "remove", "FOO-1", "FOO-3"])
        .output()
        .unwrap();

    assert!(output.status.success(), "Expected success, got: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Moved 2 issue(s) to backlog"),
        "Expected success message, got: {stdout}"
    );
}

#[tokio::test]
async fn sprint_remove_json_output() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/backlog/issue"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--output", "json",
            "sprint", "remove", "QUX-5",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "Expected success, got: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["issues"], serde_json::json!(["QUX-5"]));
    assert_eq!(parsed["removed"], true);
}

#[tokio::test]
async fn sprint_add_with_current_flag() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .args(["sprint", "add", "--current", "TEST-1", "TEST-2"])
        .output()
        .unwrap();

    assert!(output.status.success(), "Expected success, got: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Added 2 issue(s) to sprint 100"),
        "Expected success message, got: {stdout}"
    );
}

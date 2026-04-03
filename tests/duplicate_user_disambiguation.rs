#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper: build a user JSON object for wiremock responses.
fn user_json(account_id: &str, display_name: &str, email: Option<&str>) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "accountId": account_id,
        "displayName": display_name,
        "active": true,
    });
    if let Some(e) = email {
        obj["emailAddress"] = serde_json::json!(e);
    }
    obj
}

#[tokio::test]
async fn issue_list_assignee_duplicate_names_no_input_errors() {
    let server = MockServer::start().await;

    // User search returns two users with same display name
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            user_json("acc-john-1", "John Smith", Some("john1@acme.com")),
            user_json("acc-john-2", "John Smith", Some("john2@other.org")),
        ])))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list", "--assignee", "John Smith", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on duplicate user names, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("john1@acme.com"),
        "Should list first user's email, got: {stderr}"
    );
    assert!(
        stderr.contains("john2@other.org"),
        "Should list second user's email, got: {stderr}"
    );
    assert!(
        stderr.contains("John Smith"),
        "Should mention the duplicate name, got: {stderr}"
    );
}

#[tokio::test]
async fn issue_assign_duplicate_names_no_input_errors() {
    let server = MockServer::start().await;

    // Assignable user search returns two users with same display name
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            user_json("acc-john-1", "John Smith", Some("john1@acme.com")),
            user_json("acc-john-2", "John Smith", Some("john2@other.org")),
        ])))
        .mount(&server)
        .await;

    // Mock get issue (needed for assign flow idempotency check, though error
    // happens before this is reached)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee("FOO-1", "Test issue", None),
        ))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args([
            "issue",
            "assign",
            "FOO-1",
            "--to",
            "John Smith",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on duplicate user names, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("john1@acme.com"),
        "Should list first user's email, got: {stderr}"
    );
    assert!(
        stderr.contains("john2@other.org"),
        "Should list second user's email, got: {stderr}"
    );
}

#[tokio::test]
async fn issue_list_assignee_exact_match_among_multiple_results_no_input_errors() {
    let server = MockServer::start().await;

    // Three users: two share "John Smith", one is "John Smithson"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            user_json("acc-john-1", "John Smith", Some("john1@acme.com")),
            user_json("acc-smithson", "John Smithson", None),
            user_json("acc-john-2", "John Smith", Some("john2@other.org")),
        ])))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list", "--assignee", "John Smith", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on duplicate user names even with extra results, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("john1@acme.com") && stderr.contains("john2@other.org"),
        "Should list both duplicate users' emails, got: {stderr}"
    );
    assert!(
        !stderr.contains("acc-smithson") && !stderr.contains("Smithson"),
        "Should not mention non-duplicate user, got: {stderr}"
    );
}

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::fixtures;

fn jr_cmd(base_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", base_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input");
    cmd
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_search_returns_matching_users() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "jane"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixtures::user_search_response(vec![
                ("acc-1", "Jane Smith", true),
                ("acc-2", "Jane Doe", true),
            ])),
        )
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["user", "search", "jane"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Jane Smith"))
        .stdout(predicate::str::contains("Jane Doe"))
        .stdout(predicate::str::contains("acc-1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_search_empty_result_prints_no_results() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "nobody"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["user", "search", "nobody"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No results found."));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_search_json_output_is_array() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "jane"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixtures::user_search_response(vec![(
                "acc-1",
                "Jane Smith",
                true,
            )])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--output", "json", "user", "search", "jane"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON array");
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert_eq!(parsed[0]["accountId"], "acc-1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_search_limit_truncates_results() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "alice"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixtures::user_search_response(vec![
                ("acc-1", "Alice One", true),
                ("acc-2", "Alice Two", true),
                ("acc-3", "Alice Three", true),
            ])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--output", "json", "user", "search", "alice", "--limit", "2",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_list_requires_project_flag() {
    // No server needed — clap should fail before any HTTP call.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "user", "list"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "missing --project should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--project") || stderr.contains("required"),
        "expected error mentions missing --project, got: {stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_list_by_project_returns_users() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("query", ""))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            fixtures::multi_project_user_search_response(vec![
                ("acc-1", "Alice"),
                ("acc-2", "Bob"),
            ]),
        ))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["user", "list", "--project", "FOO"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("Bob"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_view_returns_detail_rows() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", "acc-xyz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "acc-xyz",
            "displayName": "Jane Smith",
            "emailAddress": "jane@acme.io",
            "active": true
        })))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["user", "view", "acc-xyz"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Jane Smith"))
        .stdout(predicate::str::contains("jane@acme.io"))
        .stdout(predicate::str::contains("acc-xyz"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_view_json_emits_user_object() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", "acc-xyz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "acc-xyz",
            "displayName": "Jane Smith",
            "emailAddress": "jane@acme.io",
            "active": true
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--output", "json", "user", "view", "acc-xyz"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["accountId"], "acc-xyz");
    assert_eq!(parsed["displayName"], "Jane Smith");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_view_404_shows_friendly_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", "does-not-exist"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["User not found"]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["user", "view", "does-not-exist"])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(64),
        "view on unknown accountId should exit 64 (JrError::UserError convention), got: {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("User with accountId 'does-not-exist' not found"),
        "expected friendly not-found message, got: {stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_view_hidden_email_renders_dash() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", "private-user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "private-user",
            "displayName": "Private Person",
            "active": true
        })))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["user", "view", "private-user"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Private Person"))
        .stdout(predicate::str::contains("—"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_view_json_hidden_email_is_null() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", "private-user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "private-user",
            "displayName": "Private Person",
            "active": true
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--output", "json", "user", "view", "private-user"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["accountId"], "private-user");
    assert_eq!(parsed["displayName"], "Private Person");
    let email = parsed
        .get("emailAddress")
        .expect("emailAddress key should be present (serialized as null), not omitted");
    assert!(
        email.is_null(),
        "emailAddress should serialize to JSON null when privacy hides it (not the em-dash placeholder), got: {email}"
    );
}

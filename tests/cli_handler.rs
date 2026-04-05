#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a `jr` command pre-configured for handler-level testing.
///
/// Sets `JR_BASE_URL` and `JR_AUTH_HEADER` env vars so the binary
/// routes API calls to the mock server and bypasses keychain auth.
fn jr_cmd(server_uri: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("--output")
        .arg("json");
    cmd
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_assign_with_account_id() {
    let server = MockServer::start().await;

    // Mock GET issue — currently unassigned
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee("HDL-1", "Handler test", None),
        ))
        .mount(&server)
        .await;

    // Mock PUT assignee
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-1/assignee"))
        .and(body_partial_json(serde_json::json!({
            "accountId": "direct-id-001"
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "assign", "HDL-1", "--account-id", "direct-id-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed\": true"))
        .stdout(predicate::str::contains("\"key\": \"HDL-1\""))
        .stdout(predicate::str::contains("\"assignee\": \"direct-id-001\""))
        .stdout(predicate::str::contains(
            "\"assignee_account_id\": \"direct-id-001\"",
        ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_assign_with_to_name_search() {
    let server = MockServer::start().await;

    // Mock assignable user search for issue HDL-2
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .and(query_param("query", "Jane"))
        .and(query_param("issueKey", "HDL-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![("acc-jane-456", "Jane Doe", true)]),
        ))
        .mount(&server)
        .await;

    // Mock GET issue — currently unassigned
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee("HDL-2", "Name search test", None),
        ))
        .mount(&server)
        .await;

    // Mock PUT assignee
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-2/assignee"))
        .and(body_partial_json(serde_json::json!({
            "accountId": "acc-jane-456"
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "assign", "HDL-2", "--to", "Jane"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"assignee\": \"Jane Doe\""))
        .stdout(predicate::str::contains("\"changed\": true"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_assign_self() {
    let server = MockServer::start().await;

    // Mock GET myself
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_response()))
        .mount(&server)
        .await;

    // Mock GET issue — currently unassigned
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee("HDL-3", "Self-assign test", None),
        ))
        .mount(&server)
        .await;

    // Mock PUT assignee
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-3/assignee"))
        .and(body_partial_json(serde_json::json!({
            "accountId": "abc123"
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "assign", "HDL-3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"assignee\": \"Test User\""))
        .stdout(predicate::str::contains("\"changed\": true"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_assign_unassign() {
    let server = MockServer::start().await;

    // Mock PUT assignee with null (unassign)
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-4/assignee"))
        .and(body_partial_json(serde_json::json!({
            "accountId": null
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "assign", "HDL-4", "--unassign"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"assignee\": null"))
        .stdout(predicate::str::contains("\"changed\": true"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_assign_idempotent() {
    let server = MockServer::start().await;

    // Mock GET issue — already assigned to the target account
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee(
                "HDL-5",
                "Already assigned",
                Some(("direct-id-001", "direct-id-001")),
            ),
        ))
        .mount(&server)
        .await;

    // PUT assignee should NOT be called — explicitly expect 0 requests
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-5/assignee"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "assign", "HDL-5", "--account-id", "direct-id-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed\": false"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_create_with_account_id() {
    let server = MockServer::start().await;

    // Mock POST create issue
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "project": {"key": "HDL"},
                "issuetype": {"name": "Task"},
                "summary": "Created via handler",
                "assignee": {"accountId": "direct-create-789"}
            }
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("HDL-100")),
        )
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args([
            "issue",
            "create",
            "-p",
            "HDL",
            "-t",
            "Task",
            "-s",
            "Created via handler",
            "--account-id",
            "direct-create-789",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"key\": \"HDL-100\""))
        .stdout(predicate::str::contains("/browse/HDL-100"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_create_with_to_name_search() {
    let server = MockServer::start().await;

    // Mock multi-project assignable user search
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("query", "Bob"))
        .and(query_param("projectKeys", "HDL"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::multi_project_user_search_response(vec![(
                "acc-bob-555",
                "Bob Smith",
            )]),
        ))
        .mount(&server)
        .await;

    // Mock POST create issue — verify assignee uses accountId
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "project": {"key": "HDL"},
                "issuetype": {"name": "Bug"},
                "summary": "Created with --to",
                "assignee": {"accountId": "acc-bob-555"}
            }
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("HDL-101")),
        )
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args([
            "issue",
            "create",
            "-p",
            "HDL",
            "-t",
            "Bug",
            "-s",
            "Created with --to",
            "--to",
            "Bob",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"key\": \"HDL-101\""));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_create_basic() {
    let server = MockServer::start().await;

    // Mock POST create issue — no assignee field
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "project": {"key": "HDL"},
                "issuetype": {"name": "Task"},
                "summary": "Basic create"
            }
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("HDL-102")),
        )
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args([
            "issue",
            "create",
            "-p",
            "HDL",
            "-t",
            "Task",
            "-s",
            "Basic create",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"key\": \"HDL-102\""))
        .stdout(predicate::str::contains("\"url\":"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_assign_to_me() {
    let server = MockServer::start().await;

    // This test covers the explicit `--to me` keyword path (resolve_assignee → is_me_keyword).
    // test_handler_assign_self covers the no-flag default path (handler calls get_myself directly).

    // Mock GET myself — resolve_assignee() detects "me" keyword via is_me_keyword() and calls get_myself()
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_response()))
        .mount(&server)
        .await;

    // Mock GET issue — currently unassigned
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-6"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee("HDL-6", "Assign to me test", None),
        ))
        .mount(&server)
        .await;

    // Mock PUT assignee
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-6/assignee"))
        .and(body_partial_json(serde_json::json!({
            "accountId": "abc123"
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "assign", "HDL-6", "--to", "me"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed\": true"))
        .stdout(predicate::str::contains("\"key\": \"HDL-6\""))
        .stdout(predicate::str::contains("\"assignee\": \"Test User\""))
        .stdout(predicate::str::contains(
            "\"assignee_account_id\": \"abc123\"",
        ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_create_to_me() {
    let server = MockServer::start().await;

    // Mock GET myself — resolve_assignee_by_project() detects "me" keyword via is_me_keyword() and calls get_myself()
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_response()))
        .mount(&server)
        .await;

    // Mock POST create issue — verify "me" keyword resolves to accountId via get_myself()
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "project": {"key": "HDL"},
                "issuetype": {"name": "Task"},
                "summary": "Created with --to me",
                "assignee": {"accountId": "abc123"}
            }
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("HDL-200")),
        )
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args([
            "issue",
            "create",
            "-p",
            "HDL",
            "-t",
            "Task",
            "-s",
            "Created with --to me",
            "--to",
            "me",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"key\": \"HDL-200\""))
        .stdout(predicate::str::contains("\"url\":"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_assign_idempotent_with_name_search() {
    let server = MockServer::start().await;

    // Mock assignable user search — returns Jane Doe
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .and(query_param("query", "Jane"))
        .and(query_param("issueKey", "HDL-7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![("acc-jane-456", "Jane Doe", true)]),
        ))
        .mount(&server)
        .await;

    // Mock GET issue — already assigned to Jane Doe (same account ID)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee(
                "HDL-7",
                "Already assigned to Jane",
                Some(("acc-jane-456", "Jane Doe")),
            ),
        ))
        .mount(&server)
        .await;

    // PUT assignee should NOT be called — already assigned to target
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-7/assignee"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "assign", "HDL-7", "--to", "Jane"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed\": false"))
        .stdout(predicate::str::contains("\"key\": \"HDL-7\""))
        .stdout(predicate::str::contains(
            "\"assignee_account_id\": \"acc-jane-456\"",
        ));
}

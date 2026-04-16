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

/// Build a `jr` command pre-configured for handler-level testing of `jr api`.
/// Unlike `jr_cmd`, does not set `--output json` since `jr api` ignores it.
fn jr_api_cmd(server_uri: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input");
    cmd
}

/// Build a `jr` command with explicit XDG overrides for cache and config dirs.
///
/// Required for tests that need to pre-populate the team cache or set a custom
/// config (e.g. `team_field_id`). Use `.env()` on the spawned `Command`
/// instead of `std::env::set_var` so these overrides stay isolated to this
/// child process and do not mutate the test process's global environment,
/// which can cause interference when tests run in parallel.
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

const TEST_TEAM_FIELD_ID: &str = "customfield_10100";

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

    // Mock GET issue — currently assigned (so unassign proceeds)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-4"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee(
                "HDL-4",
                "Unassign test",
                Some(("someone-123", "Someone")),
            ),
        ))
        .mount(&server)
        .await;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_unassign_idempotent() {
    let server = MockServer::start().await;

    // Mock GET issue — already unassigned (assignee is null)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-8"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee("HDL-8", "Already unassigned", None),
        ))
        .mount(&server)
        .await;

    // PUT assignee should NOT be called — already unassigned
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-8/assignee"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "assign", "HDL-8", "--unassign"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed\": false"))
        .stdout(predicate::str::contains("\"key\": \"HDL-8\""))
        .stdout(predicate::str::contains("\"assignee\": null"));
}

#[tokio::test]
async fn test_handler_list_created_after() {
    let server = MockServer::start().await;

    // Project existence check
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ",
            "id": "10000",
            "name": "Test Project"
        })))
        .mount(&server)
        .await;

    // The search endpoint should receive JQL with the date clause
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": "project = \"PROJ\" AND created >= \"2026-03-18\" ORDER BY updated DESC"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                "PROJ-1",
                "Test issue",
                "To Do",
            )]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "list",
            "--project",
            "PROJ",
            "--created-after",
            "2026-03-18",
            "--no-input",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn test_handler_list_created_before() {
    let server = MockServer::start().await;

    // Project existence check
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ",
            "id": "10000",
            "name": "Test Project"
        })))
        .mount(&server)
        .await;

    // --created-before 2026-03-18 should produce created < "2026-03-19" (next day)
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": "project = \"PROJ\" AND created < \"2026-03-19\" ORDER BY updated DESC"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                "PROJ-1",
                "Test issue",
                "To Do",
            )]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "list",
            "--project",
            "PROJ",
            "--created-before",
            "2026-03-18",
            "--no-input",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_list_asset_name_resolves_to_key() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();

    // 1. Workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    // 2. AQL search — returns single match
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("includeAttributes", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": true,
            "values": [{
                "id": "70",
                "label": "Acme Corp",
                "objectKey": "OBJ-70",
                "objectType": { "id": "13", "name": "Client" }
            }]
        })))
        .mount(&server)
        .await;

    // 3. CMDB fields discovery
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "id": "customfield_10191",
                "name": "Client",
                "custom": true,
                "schema": {
                    "type": "any",
                    "custom": "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
                    "customId": 10191
                }
            }
        ])))
        .mount(&server)
        .await;

    // 4. Project check
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ",
            "id": "10000",
            "name": "Test Project"
        })))
        .mount(&server)
        .await;

    // 5. Issue search — verify JQL uses resolved key OBJ-70
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": "project = \"PROJ\" AND \"Client\" IN aqlFunction(\"Key = \\\"OBJ-70\\\"\") ORDER BY updated DESC"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                "PROJ-1",
                "Test issue",
                "To Do",
            )]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args([
            "issue",
            "list",
            "--project",
            "PROJ",
            "--asset",
            "Acme",
            "--no-input",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_list_asset_name_no_match_errors() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();

    // 1. Workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    // 2. AQL search — returns zero matches
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 0,
            "isLast": true,
            "values": []
        })))
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args([
            "issue",
            "list",
            "--project",
            "PROJ",
            "--asset",
            "Nonexistent",
            "--no-input",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No assets matching \"Nonexistent\" found",
        ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_list_asset_key_passthrough_skips_assets_api() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();

    // Direct asset keys should NOT trigger workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1, "start": 0, "limit": 50, "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .expect(0)
        .mount(&server)
        .await;

    // Direct asset keys should NOT trigger AQL search
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "startAt": 0, "maxResults": 25, "total": 0, "isLast": true, "values": []
        })))
        .expect(0)
        .mount(&server)
        .await;

    // CMDB fields discovery (still needed for build_asset_clause)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "id": "customfield_10191",
                "name": "Client",
                "custom": true,
                "schema": {
                    "type": "any",
                    "custom": "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
                    "customId": 10191
                }
            }
        ])))
        .mount(&server)
        .await;

    // Project check
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ", "id": "10000", "name": "Test Project"
        })))
        .mount(&server)
        .await;

    // Issue search — verify JQL uses provided key OBJ-18 directly
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": "project = \"PROJ\" AND \"Client\" IN aqlFunction(\"Key = \\\"OBJ-18\\\"\") ORDER BY updated DESC"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                "PROJ-1", "Test issue", "To Do",
            )]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args([
            "issue",
            "list",
            "--project",
            "PROJ",
            "--asset",
            "OBJ-18",
            "--no-input",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_comment_internal_flag_adds_property() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/HELP-42/comment"))
        .and(body_partial_json(serde_json::json!({
            "properties": [{
                "key": "sd.public.comment",
                "value": { "internal": true }
            }]
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10001",
            "created": "2026-04-05T12:00:00.000+0000"
        })))
        .expect(1)
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "comment",
            "HELP-42",
            "Internal note",
            "--internal",
            "--no-input",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_comment_without_internal_omits_property() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/HELP-42/comment"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10002",
            "created": "2026-04-05T12:00:00.000+0000"
        })))
        .expect(1)
        .named("comment without internal")
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comment", "HELP-42", "External note", "--no-input"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_comments_shows_visibility_column_for_jsm() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HELP-42/comment"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [
                {
                    "id": "10001",
                    "author": { "accountId": "abc", "displayName": "Agent", "active": true },
                    "body": { "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Agent investigation notes" }] }] },
                    "created": "2026-04-05T10:00:00.000+0000",
                    "properties": [{"key": "sd.public.comment", "value": {"internal": true}}]
                },
                {
                    "id": "10002",
                    "author": { "accountId": "def", "displayName": "Agent", "active": true },
                    "body": { "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Customer reply" }] }] },
                    "created": "2026-04-05T11:00:00.000+0000",
                    "properties": [{"key": "sd.public.comment", "value": {"internal": false}}]
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 2
        })))
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "HELP-42", "--no-input"])
        .assert()
        .success()
        .stdout(predicates::prelude::predicate::str::contains("Visibility"))
        .stdout(predicates::prelude::predicate::str::contains("Internal"))
        .stdout(predicates::prelude::predicate::str::contains("External"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_comments_hides_visibility_column_for_non_jsm() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/DEV-99/comment"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [
                {
                    "id": "10001",
                    "author": { "accountId": "abc", "displayName": "Dev", "active": true },
                    "body": { "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Fixed in commit abc123" }] }] },
                    "created": "2026-04-05T10:00:00.000+0000",
                    "properties": []
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 1
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "DEV-99", "--no-input"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Visibility"),
        "Non-JSM comments should not show Visibility column, got: {stdout}"
    );
    assert!(
        !stdout.contains("Internal"),
        "Non-JSM comments should not show Internal, got: {stdout}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_get_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"accountId":"abc-123","displayName":"Test User"}"#),
        )
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args(["api", "/rest/api/3/myself"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"accountId\":\"abc-123\""))
        .stdout(predicate::str::contains("\"displayName\":\"Test User\""));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_post_with_inline_data() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(
            serde_json::json!({"fields": {"summary": "Test"}}),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_string(r#"{"key":"PROJ-1"}"#))
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args([
            "api",
            "/rest/api/3/issue",
            "--method",
            "post",
            "--data",
            r#"{"fields":{"summary":"Test"}}"#,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"key\":\"PROJ-1\""));

    // Verify exactly one Content-Type header on the received request
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let content_type_count = requests[0]
        .headers
        .iter()
        .filter(|(name, _)| name.as_str().eq_ignore_ascii_case("content-type"))
        .count();
    assert_eq!(
        content_type_count, 1,
        "expected exactly one Content-Type header, got {content_type_count}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_put_with_method_flag() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-1/assignee"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args([
            "api",
            "/rest/api/3/issue/PROJ-1/assignee",
            "-X",
            "put",
            "-d",
            r#"{"accountId":"abc-123"}"#,
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_custom_header_passes_through() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/1/organization"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"values":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args([
            "api",
            "/rest/servicedeskapi/servicedesk/1/organization",
            "-H",
            "X-ExperimentalApi: opt-in",
        ])
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let has_experimental_header = requests[0].headers.iter().any(|(name, value)| {
        name.as_str().eq_ignore_ascii_case("x-experimentalapi") && value.as_bytes() == b"opt-in"
    });
    assert!(has_experimental_header, "X-ExperimentalApi header missing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_custom_content_type_overrides_default() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/thing"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;

    // Note: body must still be valid JSON (we validate at resolve_body stage).
    // The Content-Type override is tested separately from the JSON validation.
    jr_api_cmd(&server.uri())
        .args([
            "api",
            "/rest/api/3/thing",
            "-X",
            "post",
            "-d",
            r#"{"ok":true}"#,
            "-H",
            "Content-Type: application/vnd.atlassian.custom+json",
        ])
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let content_type_values: Vec<String> = requests[0]
        .headers
        .iter()
        .filter(|(name, _)| name.as_str().eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| String::from_utf8_lossy(value.as_bytes()).to_string())
        .collect();
    assert_eq!(
        content_type_values.len(),
        1,
        "expected exactly one Content-Type, got {content_type_values:?}"
    );
    assert_eq!(
        content_type_values[0], "application/vnd.atlassian.custom+json",
        "user-supplied Content-Type must override the default"
    );
    // Defensive: verify application/json is NOT present alongside the custom value
    assert!(
        !content_type_values.iter().any(|v| v == "application/json"),
        "default application/json should have been replaced, got {content_type_values:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_error_response_body_to_stdout() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/MISSING-1"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_string(r#"{"errorMessages":["Issue does not exist"],"errors":{}}"#),
        )
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args(["api", "/rest/api/3/issue/MISSING-1"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("Issue does not exist"))
        // main.rs prints "Error: {e}" where e is JrError::ApiError with Display
        // "API error ({status}): {message}" — stderr contains "(404)" and the extracted message
        .stderr(predicate::str::contains("(404)"))
        .stderr(predicate::str::contains("Issue does not exist"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_path_normalization_missing_slash() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .expect(1)
        .mount(&server)
        .await;

    // No leading slash — should still work
    jr_api_cmd(&server.uri())
        .args(["api", "rest/api/3/myself"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ok\":true"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_rejects_absolute_url() {
    let server = MockServer::start().await;
    // No mock defined — if the handler tries to hit the network, it will fail

    jr_api_cmd(&server.uri())
        .args(["api", "https://example.atlassian.net/rest/api/3/myself"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("do not include the instance URL"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_rejects_authorization_header() {
    let server = MockServer::start().await;

    jr_api_cmd(&server.uri())
        .args([
            "api",
            "/rest/api/3/myself",
            "-H",
            "Authorization: Bearer pwned",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cannot override the Authorization header",
        ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_stdin_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/thing"))
        .and(body_partial_json(serde_json::json!({"from":"stdin"})))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args(["api", "/rest/api/3/thing", "-X", "post", "-d", "@-"])
        .write_stdin(r#"{"from":"stdin"}"#)
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_401_returns_not_authenticated() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(r#"{"errorMessages":["Client must be authenticated"]}"#),
        )
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args(["api", "/rest/api/3/myself"])
        .assert()
        .failure()
        // 401 → JrError::NotAuthenticated, which has display "Not authenticated..."
        .stderr(predicate::str::contains("Not authenticated"))
        // Body is still printed to stdout before the status check
        .stdout(predicate::str::contains("Client must be authenticated"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_stdout_byte_exact() {
    let server = MockServer::start().await;

    // Deliberately non-pretty-printed JSON to verify raw byte passthrough
    let exact_body = r#"{"key":"PROJ-1","custom":"no reformatting"}"#;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_string(exact_body))
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args(["api", "/rest/api/3/myself"])
        .assert()
        .success()
        // Byte-exact: no trailing newline, no pretty-printing
        .stdout(predicate::eq(exact_body));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_output_json_flag_ignored() {
    let server = MockServer::start().await;

    let raw_body = r#"{"accountId":"abc-123"}"#;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_string(raw_body))
        .expect(1)
        .mount(&server)
        .await;

    // Pass --output json globally — jr api should still return raw body, not wrapped
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("--output")
        .arg("json")
        .args(["api", "/rest/api/3/myself"])
        .assert()
        .success()
        .stdout(predicate::eq(raw_body));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_api_warns_on_429_retry_exhaustion() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "0")
                .set_body_string(r#"{"errorMessages":["Rate limit exceeded"]}"#),
        )
        .expect(4) // initial + 3 retries (MAX_RETRIES)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args(["api", "/rest/api/3/myself"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("warning: rate limited by Jira"))
        .stderr(predicate::str::contains("3 retries"))
        .stderr(predicate::str::contains("Wait a moment and try again"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_send_warns_on_429_retry_exhaustion() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FAKE-1"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "0")
                .set_body_string(r#"{"errorMessages":["Rate limit exceeded"]}"#),
        )
        .expect(4) // initial + 3 retries (MAX_RETRIES)
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "view", "FAKE-1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("warning: rate limited by Jira"))
        .stderr(predicate::str::contains("3 retries"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_create_table_mode_outputs_to_stderr() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("HDL-300")),
        )
        .mount(&server)
        .await;

    // Use jr_api_cmd (no --output json) to test Table mode
    jr_api_cmd(&server.uri())
        .args([
            "issue",
            "create",
            "-p",
            "HDL",
            "-t",
            "Task",
            "-s",
            "Table mode test",
        ])
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("Created issue HDL-300"))
        .stderr(predicate::str::contains("/browse/HDL-300"));
}

/// Helper: pre-populate team cache at the given XDG cache dir root.
fn write_test_team_cache(cache_home: &std::path::Path) {
    let teams_dir = cache_home.join("jr");
    std::fs::create_dir_all(&teams_dir).unwrap();
    let cache = jr::cache::TeamCache {
        fetched_at: chrono::Utc::now(),
        teams: vec![
            jr::cache::CachedTeam {
                id: "team-uuid-abc".into(),
                name: "Platform".into(),
            },
            jr::cache::CachedTeam {
                id: "team-uuid-platform-ops".into(),
                name: "Platform Ops".into(),
            },
        ],
    };
    std::fs::write(
        teams_dir.join("teams.json"),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();
}

/// Helper: write a config.toml with team_field_id set to TEST_TEAM_FIELD_ID.
fn write_test_config_with_team_field(config_home: &std::path::Path) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        format!("[fields]\nteam_field_id = \"{TEST_TEAM_FIELD_ID}\"\n"),
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_view_renders_team_name_when_cached() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-500"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_team(
                "HDL-500",
                "Team cached",
                TEST_TEAM_FIELD_ID,
                "team-uuid-abc",
            ),
        ))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_test_team_cache(cache_dir.path());
    write_test_config_with_team_field(config_dir.path());

    jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "view", "HDL-500"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Team"))
        .stdout(predicate::str::contains("Platform"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_view_renders_team_uuid_fallback_when_not_cached() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-501"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_team(
                "HDL-501",
                "Team uncached",
                TEST_TEAM_FIELD_ID,
                "team-uuid-unknown",
            ),
        ))
        .mount(&server)
        .await;

    // Empty cache dir (no teams.json) — UUID should appear with fallback text.
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_test_config_with_team_field(config_dir.path());

    jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "view", "HDL-501"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Team"))
        .stdout(predicate::str::contains("team-uuid-unknown"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_view_omits_team_row_when_field_unconfigured() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-502"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "HDL-502",
                "No team field",
                "To Do",
            )),
        )
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    // Write a config without team_field_id (empty [fields] section)
    let conf_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(conf_dir.join("config.toml"), "[fields]\n").unwrap();

    jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "view", "HDL-502"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No team field")) // summary present
        .stdout(predicate::str::contains("│ Team").not());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_view_omits_team_row_when_field_absent_from_response() {
    // Table mode: team_field_id IS configured, but the Jira response does not
    // include that custom field on the issue. The Team row must be omitted —
    // team_id() returns None when the key is missing, and the outer
    // `if let Some(team_uuid)` guard skips rendering entirely.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ABC-123"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "ABC-123",
                "No team set",
                "To Do",
            )),
        )
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_test_team_cache(cache_dir.path());
    write_test_config_with_team_field(config_dir.path());

    jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "view", "ABC-123"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No team set")) // summary present
        .stdout(predicate::str::contains("│ Team").not()) // no Team field row in table
        .stdout(predicate::str::contains(TEST_TEAM_FIELD_ID).not()); // no field ID leaking
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_edit_team_substring_rejects_under_no_input() {
    // Single-hit substring must NOT silently resolve under --no-input.
    //
    // Cache contains "Platform Ops" (id: team-uuid-platform-ops). Passing --team Ops
    // matches only "Platform Ops" as a substring → partial_match returns Ambiguous →
    // resolve_team_field bails with an error before any HTTP call is made.
    let server = MockServer::start().await;
    // Intentionally no PUT or GET mocks — the command must fail before hitting the wire.

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_test_team_cache(cache_dir.path());
    write_test_config_with_team_field(config_dir.path());

    jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "edit", "HDL-600", "--team", "Ops"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Multiple teams match"))
        .stderr(predicate::str::contains("Platform Ops"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_verbose_logs_request_body_for_put() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-1"))
        .and(body_partial_json(serde_json::json!({
            "fields": {"summary": "new summary"}
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .arg("--verbose")
        .args(["issue", "edit", "HDL-1", "--summary", "new summary"])
        .assert()
        .success()
        .stderr(predicate::str::contains("[verbose] PUT"))
        .stderr(predicate::str::contains("[verbose] body:"))
        .stderr(predicate::str::contains("\"summary\":\"new summary\""));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_verbose_logs_request_body_for_send_raw() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/HDL-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .arg("--verbose")
        .args([
            "api",
            "/rest/api/3/issue/HDL-1/transitions",
            "-X",
            "post",
            "-d",
            r#"{"transition":{"id":"31"}}"#,
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("[verbose] POST"))
        .stderr(predicate::str::contains(
            "[verbose] body: {\"transition\":{\"id\":\"31\"}}",
        ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_verbose_omits_body_line_for_get() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "HDL-1",
                "old summary",
                "To Do",
            )),
        )
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .arg("--verbose")
        .args(["issue", "view", "HDL-1"])
        .assert()
        .success()
        .stderr(predicate::str::contains("[verbose] GET"))
        .stderr(predicate::str::contains("[verbose] body:").not());
}

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

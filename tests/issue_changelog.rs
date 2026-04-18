#[allow(dead_code)]
mod common;

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_changelog_single_page_returns_entries() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 100,
            "total": 1,
            "isLast": true,
            "values": [
                {
                    "id": "10000",
                    "author": { "accountId": "abc", "displayName": "Alice", "active": true },
                    "created": "2026-04-16T14:02:11.000+0000",
                    "items": [{
                        "field": "status", "fieldtype": "jira",
                        "from": "1", "fromString": "To Do",
                        "to": "3", "toString": "In Progress"
                    }]
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let entries = client.get_changelog("FOO-1").await.unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "10000");
    assert_eq!(entries[0].items[0].field, "status");
}

#[tokio::test]
async fn get_changelog_auto_paginates_across_pages() {
    let server = MockServer::start().await;

    // Page 1 (startAt=0, total=2, has_more because startAt+maxResults < total)
    // Use maxResults=1 to force a second page; client asks maxResults=100 but
    // the server can cap it — simulate that by returning total=2 with a
    // single entry in values[].
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-2/changelog"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 1,
            "total": 2,
            "isLast": false,
            "values": [{
                "id": "1", "author": null,
                "created": "2026-04-10T00:00:00.000+0000",
                "items": [{"field": "status", "fieldtype": "jira",
                           "from": "1", "fromString": "To Do",
                           "to": "2", "toString": "In Progress"}]
            }]
        })))
        .mount(&server)
        .await;

    // Page 2
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-2/changelog"))
        .and(query_param("startAt", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 1,
            "maxResults": 1,
            "total": 2,
            "isLast": true,
            "values": [{
                "id": "2", "author": null,
                "created": "2026-04-11T00:00:00.000+0000",
                "items": [{"field": "status", "fieldtype": "jira",
                           "from": "2", "fromString": "In Progress",
                           "to": "3", "toString": "Done"}]
            }]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let entries = client.get_changelog("FOO-2").await.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, "1");
    assert_eq!(entries[1].id, "2");
}

use assert_cmd::Command;

#[test]
fn changelog_help_lists_subcommand() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "changelog", "--help"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "--help should exit 0, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--limit"), "help missing --limit: {stdout}");
    assert!(stdout.contains("--all"), "help missing --all: {stdout}");
    assert!(stdout.contains("--field"), "help missing --field: {stdout}");
    assert!(
        stdout.contains("--author"),
        "help missing --author: {stdout}"
    );
    assert!(
        stdout.contains("--reverse"),
        "help missing --reverse: {stdout}"
    );
}

#[tokio::test]
async fn changelog_table_renders_flat_rows_newest_first() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "alice", "displayName": "Alice", "active": true },
                    "created": "2026-04-14T16:02:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "backend"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "alice", "displayName": "Alice", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [
                        {"field": "status", "fieldtype": "jira",
                         "from": "1", "fromString": "To Do",
                         "to": "3", "toString": "In Progress"},
                        {"field": "resolution", "fieldtype": "jira",
                         "from": null, "fromString": null,
                         "to": "10000", "toString": "Done"}
                    ]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Newest-first: status row (from entry id=2) appears before labels row.
    let status_idx = stdout.find("status").expect("status row missing");
    let labels_idx = stdout.find("labels").expect("labels row missing");
    assert!(
        status_idx < labels_idx,
        "expected status (newer) before labels (older), got:\n{stdout}"
    );

    // Flat rows: entry id=2 produces TWO rows (status + resolution).
    assert!(
        stdout.contains("resolution"),
        "resolution row missing: {stdout}"
    );

    // From/to rendering: "To Do" → "In Progress"; null rendered as em dash.
    assert!(stdout.contains("To Do"), "fromString missing: {stdout}");
    assert!(stdout.contains("In Progress"), "toString missing: {stdout}");
    assert!(
        stdout.contains("—"),
        "em-dash null marker missing: {stdout}"
    );
}

#[tokio::test]
async fn changelog_json_preserves_nested_structure() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 1, "isLast": true,
            "values": [{
                "id": "10000",
                "author": { "accountId": "alice", "displayName": "Alice", "active": true },
                "created": "2026-04-16T14:02:11.000+0000",
                "items": [
                    {"field": "status", "fieldtype": "jira",
                     "from": "1", "fromString": "To Do",
                     "to": "3", "toString": "In Progress"},
                    {"field": "resolution", "fieldtype": "jira",
                     "from": null, "fromString": null,
                     "to": "10000", "toString": "Done"}
                ]
            }]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--output", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("expected valid JSON");

    assert_eq!(parsed["key"], "FOO-1");
    let entries = parsed["entries"].as_array().expect("entries must be array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["id"], "10000");
    assert_eq!(entries[0]["items"].as_array().unwrap().len(), 2);
    // Nested structure preserved — item[0].field is accessible directly.
    assert_eq!(entries[0]["items"][0]["field"], "status");
}

#[tokio::test]
async fn changelog_reverse_renders_oldest_first() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "newer",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "In Progress"}]
                },
                {
                    "id": "older",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-14T16:02:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "backend"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--reverse"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let status_idx = stdout.find("status").expect("missing status row");
    let labels_idx = stdout.find("labels").expect("missing labels row");
    // With --reverse, oldest (labels) comes first.
    assert!(
        labels_idx < status_idx,
        "expected labels (older) before status (newer), got:\n{stdout}"
    );
}

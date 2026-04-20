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

// Exercises the pagination-advancement guard: a page that advertises
// `has_more` (total > startAt + maxResults) but reports `maxResults: 0`,
// which would cause `next_start()` to equal `start_at` and infinite-loop
// without the guard. Expect an explicit `anyhow` error mentioning the
// anomaly, not a hang.
#[tokio::test]
async fn get_changelog_errors_when_page_fails_to_advance() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-STALE/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 0,
            "total": 5,
            "isLast": false,
            "values": []
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let err = client
        .get_changelog("FOO-STALE")
        .await
        .expect_err("guard should reject non-advancing page");
    let message = err.to_string();
    assert!(
        message.contains("did not advance") || message.contains("malformed"),
        "expected guard error message, got: {message}"
    );
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

#[tokio::test]
async fn changelog_field_filter_keeps_only_matching_items() {
    let server = MockServer::start().await;

    // Single entry with TWO items; --field status should keep only one row.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 1, "isLast": true,
            "values": [{
                "id": "1",
                "author": { "accountId": "a", "displayName": "Alice", "active": true },
                "created": "2026-04-16T14:02:00.000+0000",
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
        .args(["issue", "changelog", "FOO-1", "--field", "status"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("status"), "status row missing: {stdout}");
    assert!(
        !stdout.contains("resolution"),
        "resolution row should be filtered out: {stdout}"
    );
}

#[tokio::test]
async fn changelog_field_filter_is_case_insensitive_and_substring() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 1, "isLast": true,
            "values": [{
                "id": "1",
                "author": { "accountId": "a", "displayName": "Alice", "active": true },
                "created": "2026-04-16T14:02:00.000+0000",
                "items": [
                    {"field": "Story Points", "fieldtype": "custom",
                     "from": null, "fromString": "3", "to": null, "toString": "5"}
                ]
            }]
        })))
        .mount(&server)
        .await;

    // "points" matches "Story Points" via case-insensitive substring.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--field", "points"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Story Points"));
}

#[tokio::test]
async fn changelog_author_me_resolves_via_myself() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": "me-acc",
            "displayName": "Me User",
            "emailAddress": "me@test.com",
            "active": true
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "me-acc", "displayName": "Me User", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "other", "displayName": "Someone Else", "active": true },
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "x"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "me"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Me User"));
    assert!(!stdout.contains("Someone Else"));
}

// Empty or whitespace-only `--author` must be rejected before any API
// call. Without the guard, the needle lowercases to `""` and
// `haystack.contains("")` is always `true` per Rust's `str::contains`,
// so every author silently matches — a filter bypass that surfaces when
// an agent passes an unset shell variable as `--author "$UNSET_VAR"`.
//
// `MockServer::start()` is created but registers no mocks: it exists
// so the handler can construct a `JiraClient` against a valid
// `JR_BASE_URL`. If the guard regresses, the handler would reach the
// unmocked changelog path and exit with a non-64 code, which the
// `Some(64)` assertion catches.
#[tokio::test]
async fn changelog_rejects_empty_author() {
    let server = MockServer::start().await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", ""])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 (UserError), got: {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--author cannot be empty"),
        "expected '--author cannot be empty' in stderr: {stderr}"
    );
}

#[tokio::test]
async fn changelog_rejects_whitespace_only_author() {
    let server = MockServer::start().await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "   "])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--author cannot be empty"));
}

// Tabs and newlines are Unicode `White_Space` per the stdlib, so
// `str::trim()` strips them. Pins that the guard correctly rejects
// these forms too — an agent that renders `$UNSET_VAR` via a template
// could end up with `\t` or `\n` as the entire argument.
#[tokio::test]
async fn changelog_rejects_tab_or_newline_only_author() {
    let server = MockServer::start().await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "\t\n"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--author cannot be empty"));
}

// --field suffers the same class of silent-filter-bypass as --author:
// the filter is `needles.iter().any(|n| h.contains(n))` with lowercased
// user input, and `contains("")` is always `true` per `str::contains`,
// so an empty value would make every item match. Reject before the API
// call for the same reasons (exit 64 / JrError::UserError). MockServer
// mounts no expectations — a regression would reach the unmocked
// changelog path and exit with a non-64 code.
#[tokio::test]
async fn changelog_rejects_empty_field() {
    let server = MockServer::start().await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--field", ""])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 (UserError), got: {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--field cannot be empty"),
        "expected '--field cannot be empty' in stderr: {stderr}"
    );
}

#[tokio::test]
async fn changelog_rejects_whitespace_only_field() {
    let server = MockServer::start().await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--field", "   "])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--field cannot be empty"));
}

// Parity with the --author guard: tabs/newlines are Unicode
// `White_Space` and `str::trim()` strips them, so the rejection
// fires for these forms too.
#[tokio::test]
async fn changelog_rejects_tab_or_newline_only_field() {
    let server = MockServer::start().await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--field", "\t\n"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--field cannot be empty"));
}

// --field is repeatable (`Vec<String>`). A mixed invocation with one
// valid + one empty value must still reject — otherwise the empty
// needle would OR with the valid one and match every item anyway.
#[tokio::test]
async fn changelog_rejects_empty_field_among_valid() {
    let server = MockServer::start().await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "changelog",
            "FOO-1",
            "--field",
            "status",
            "--field",
            "",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--field cannot be empty"));
}

#[tokio::test]
async fn changelog_author_name_substring_case_insensitive() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "a", "displayName": "Alice Smith", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "b", "displayName": "Bob Jones", "active": true },
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "x"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "alice"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Alice Smith"));
    assert!(!stdout.contains("Bob Jones"));
}

// End-to-end regression pin for the #213 bug: a long single-word
// display name (≥12 chars, no digits) must classify as NameSubstring,
// not AccountId, or the CLI silently returns zero matches. The
// existing `from_raw_long_alpha_only_name_is_substring` unit test
// pins the classifier; this pins the full pipeline from argv through
// the wiremock-stubbed changelog response to rendered stdout.
#[tokio::test]
async fn changelog_author_long_alpha_name_matches_display_name() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "a", "displayName": "AlexanderGreene", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "b", "displayName": "Bob Jones", "active": true },
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "x"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "AlexanderGreene"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("AlexanderGreene"),
        "long single-word display name was missed — possible regression of #213: {stdout}"
    );
    assert!(!stdout.contains("Bob Jones"));
}

// Short (< 12 chars, no colon) `--author` values go through
// `AuthorNeedle::NameSubstring`, which matches against both `displayName`
// and `accountId` — so a short accountId prefix still works. This test
// locks that behavior in. For the literal-AccountId branch (colon or
// ≥12 alphanumeric/-/_ chars), see
// `changelog_author_long_accountid_literal_match` below.
#[tokio::test]
async fn changelog_author_short_value_matches_accountid_substring() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "abc123", "displayName": "Alice", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "def456", "displayName": "Bob", "active": true },
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "x"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "abc123"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Alice"));
    assert!(!stdout.contains("Bob"));
}

// Exercises the `AuthorNeedle::AccountId` (literal/exact) branch. The input
// `557058:abc-def-0123` contains a colon, which `classify_author` routes to
// AccountId — partial-match against displayName/accountId is NOT attempted.
#[tokio::test]
async fn changelog_author_long_accountid_literal_match() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": {
                        "accountId": "557058:abc-def-0123",
                        "displayName": "Alice Smith",
                        "active": true
                    },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2",
                    "author": {
                        "accountId": "557058:abc-def-0999",
                        "displayName": "Bob Jones",
                        "active": true
                    },
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "x"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "changelog",
            "FOO-1",
            "--author",
            "557058:abc-def-0123",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Alice Smith"),
        "Alice row missing: {stdout}"
    );
    assert!(
        !stdout.contains("Bob Jones"),
        "Bob row should not match (different accountId): {stdout}"
    );
}

#[tokio::test]
async fn changelog_author_null_filtered_out_when_flag_set() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1", "author": null,
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "x"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "alice"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // System entry is dropped when --author is set and doesn't match.
    assert!(
        !stdout.contains("(system)"),
        "(system) row should be filtered when --author set: {stdout}"
    );
    assert!(stdout.contains("Alice"));
}

#[tokio::test]
async fn changelog_limit_truncates_after_sort() {
    let server = MockServer::start().await;

    // Three entries; we expect --limit 2 to keep the two newest.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 3, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-10T00:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "oldest"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-15T00:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "middle"}]
                },
                {
                    "id": "3",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-17T00:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "newest"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--limit", "2"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("newest"), "missing newest row: {stdout}");
    assert!(stdout.contains("middle"), "missing middle row: {stdout}");
    assert!(
        !stdout.contains("oldest"),
        "oldest row should be truncated: {stdout}"
    );
}

#[tokio::test]
async fn changelog_limit_zero_renders_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 1, "isLast": true,
            "values": [{
                "id": "1",
                "author": { "accountId": "a", "displayName": "Alice", "active": true },
                "created": "2026-04-16T14:02:00.000+0000",
                "items": [{"field": "status", "fieldtype": "jira",
                           "from": "1", "fromString": "To Do",
                           "to": "3", "toString": "Done"}]
            }]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--limit", "0"])
        .output()
        .unwrap();

    assert!(output.status.success(), "--limit 0 should exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The project's `print_output` prints "No results found." for empty tables.
    assert!(stdout.contains("No results found"), "got: {stdout}");
}

#[tokio::test]
async fn changelog_all_disables_truncation() {
    // Generate 40 entries so the default 30 would truncate — verify --all keeps all.
    let server = MockServer::start().await;

    let values: Vec<serde_json::Value> = (0..40)
        .map(|i| {
            json!({
                "id": format!("{}", i),
                "author": { "accountId": "a", "displayName": "Alice", "active": true },
                "created": format!("2026-04-{:02}T00:00:00.000+0000", (i % 28) + 1),
                "items": [{"field": "labels", "fieldtype": "jira",
                           "from": "", "to": format!("v{}", i)}]
            })
        })
        .collect();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 40, "isLast": true,
            "values": values
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Row count includes header + separator lines; just look for v0 and v39.
    assert!(
        stdout.contains("v0"),
        "missing v0: first 120 bytes (or full output if mid-codepoint):\n{}",
        stdout.get(..120).unwrap_or(&stdout)
    );
    assert!(
        stdout.contains("v39"),
        "missing v39 — --all did not disable limit"
    );
}

#[tokio::test]
async fn changelog_default_limit_is_thirty() {
    let server = MockServer::start().await;

    let values: Vec<serde_json::Value> = (0..40)
        .map(|i| {
            json!({
                "id": format!("{}", i),
                "author": { "accountId": "a", "displayName": "Alice", "active": true },
                "created": format!("2026-04-{:02}T00:00:00.000+0000", (i % 28) + 1),
                "items": [{"field": "labels", "fieldtype": "jira",
                           "from": "", "to": format!("v{}", i)}]
            })
        })
        .collect();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 40, "isLast": true,
            "values": values
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

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Default cap = 30 rows → we should NOT see all 40 values.
    // Count occurrences of "v" followed by digit to estimate.
    let v_count = stdout.matches("v").filter(|m| !m.is_empty()).count();
    // Not an exact count (comfy-table decorations use dashes/bars), but a rough
    // upper bound: should be ≤ ~30 + a handful of decorations.
    assert!(
        v_count <= 35,
        "default limit not applied, saw {v_count} 'v' occurrences"
    );
}

#[tokio::test]
async fn changelog_404_surfaces_not_found() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-999/changelog"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "errorMessages": ["Issue does not exist or you do not have permission to see it."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-999"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("404"),
        "expected status in stderr: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn changelog_401_suggests_reauth() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
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

    assert!(!output.status.success());
    assert_eq!(
        output.status.code(),
        Some(2),
        "401 should exit 2, got: {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Not authenticated"),
        "expected 'Not authenticated' in stderr: {stderr}"
    );
    assert!(stderr.contains("jr auth login"));
}

#[tokio::test]
async fn changelog_network_drop_surfaces_reach_error() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "PROJ-1"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Could not reach"),
        "expected 'Could not reach' in stderr: {stderr}"
    );
}

#[tokio::test]
async fn changelog_empty_response_exit_zero_with_empty_table() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 0, "isLast": true,
            "values": []
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

    assert!(output.status.success(), "empty response should exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No results found"),
        "expected empty-state message: {stdout}"
    );
}

#[tokio::test]
async fn changelog_empty_response_json_has_empty_entries() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 0, "isLast": true,
            "values": []
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

    assert!(output.status.success());
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(parsed["key"], "FOO-1");
    assert_eq!(parsed["entries"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn changelog_json_output_snapshot() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "2",
                    "author": { "accountId": "alice", "displayName": "Alice Smith", "active": true },
                    "created": "2026-04-16T14:02:11.000+0000",
                    "items": [
                        {"field": "status", "fieldtype": "jira",
                         "from": "1", "fromString": "To Do",
                         "to": "3", "toString": "In Progress"},
                        {"field": "resolution", "fieldtype": "jira",
                         "from": null, "fromString": null,
                         "to": "10000", "toString": "Done"}
                    ]
                },
                {
                    "id": "1", "author": null,
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
        .args(["issue", "changelog", "FOO-1", "--output", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    insta::assert_json_snapshot!(parsed);
}

#[tokio::test]
async fn changelog_limit_partial_trims_inside_straddling_entry() {
    // One entry with 3 items. --limit 2 should keep the first two items
    // (in sorted order) of that entry and drop the third.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 1, "isLast": true,
            "values": [{
                "id": "1",
                "author": { "accountId": "a", "displayName": "Alice", "active": true },
                "created": "2026-04-16T14:02:00.000+0000",
                "items": [
                    {"field": "status", "fieldtype": "jira",
                     "from": "1", "fromString": "To Do",
                     "to": "3", "toString": "In Progress"},
                    {"field": "resolution", "fieldtype": "jira",
                     "from": null, "fromString": null,
                     "to": "10000", "toString": "Done"},
                    {"field": "labels", "fieldtype": "jira",
                     "from": "", "to": "backend"}
                ]
            }]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--limit", "2"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // First two items of the entry survive; the third does not.
    assert!(stdout.contains("status"), "first item missing: {stdout}");
    assert!(
        stdout.contains("resolution"),
        "second item missing: {stdout}"
    );
    assert!(
        !stdout.contains("labels"),
        "third item should be trimmed by partial-trim: {stdout}"
    );
}

#[tokio::test]
async fn changelog_author_me_drops_null_author_entries() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": "me-acc",
            "displayName": "Me User",
            "active": true
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "me-acc", "displayName": "Me User", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2", "author": null,
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "automated"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "me"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Me User"));
    // Null-author (system) entry must be dropped when --author is set, even
    // for the AccountId branch (resolved from "me").
    assert!(
        !stdout.contains("(system)"),
        "(system) entry should be filtered: {stdout}"
    );
    assert!(
        !stdout.contains("automated"),
        "null-author entry should be dropped, saw 'automated': {stdout}"
    );
}

#[tokio::test]
async fn changelog_field_filter_repeatable_uses_or_semantics() {
    // Entry has 3 items (status, resolution, labels). --field status --field labels
    // should keep status and labels, drop resolution.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 1, "isLast": true,
            "values": [{
                "id": "1",
                "author": { "accountId": "a", "displayName": "Alice", "active": true },
                "created": "2026-04-16T14:02:00.000+0000",
                "items": [
                    {"field": "status", "fieldtype": "jira",
                     "from": "1", "fromString": "To Do",
                     "to": "3", "toString": "Done"},
                    {"field": "resolution", "fieldtype": "jira",
                     "from": null, "fromString": null,
                     "to": "10000", "toString": "Fixed"},
                    {"field": "labels", "fieldtype": "jira",
                     "from": "", "to": "backend"}
                ]
            }]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "changelog",
            "FOO-1",
            "--field",
            "status",
            "--field",
            "labels",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("status"), "status row missing: {stdout}");
    assert!(stdout.contains("labels"), "labels row missing: {stdout}");
    assert!(
        !stdout.contains("resolution"),
        "resolution should be filtered out: {stdout}"
    );
}

#[tokio::test]
async fn changelog_author_me_is_case_insensitive() {
    // --author ME (uppercase) must resolve via /myself just like --author me,
    // matching the shared `helpers::is_me_keyword` behavior used by other
    // commands (see src/cli/issue/helpers.rs).
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": "me-acc",
            "displayName": "Me User",
            "active": true
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "me-acc", "displayName": "Me User", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "other", "displayName": "Someone Else", "active": true },
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "x"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "ME"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Me User"), "Me User row missing: {stdout}");
    assert!(
        !stdout.contains("Someone Else"),
        "Someone Else should be filtered: {stdout}"
    );
}

#[tokio::test]
async fn changelog_verbose_logs_parse_failure_once() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/BAD-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "values": [
                {
                    "id": "1",
                    "author": {
                        "accountId": "u1",
                        "displayName": "Alice",
                        "emailAddress": null,
                        "active": true
                    },
                    "created": "not-a-date",
                    "items": [{
                        "field": "status",
                        "fieldtype": "jira",
                        "from": null, "fromString": "To Do",
                        "to": null, "toString": "In Progress"
                    }]
                },
                {
                    "id": "2",
                    "author": {
                        "accountId": "u1",
                        "displayName": "Alice",
                        "emailAddress": null,
                        "active": true
                    },
                    "created": "still-not-a-date",
                    "items": [{
                        "field": "status",
                        "fieldtype": "jira",
                        "from": null, "fromString": "In Progress",
                        "to": null, "toString": "Done"
                    }]
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 2,
            "isLast": true
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "BAD-1", "--verbose"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "jr exited non-zero ({:?}). stdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );
    let count = stderr.matches("timestamp failed to parse").count();
    assert_eq!(
        count, 1,
        "expected exactly one parse-failure log across 2 bad entries, got {count}. stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("[verbose] changelog"),
        "expected [verbose] changelog prefix in stderr, got:\n{stderr}"
    );
}

#[tokio::test]
async fn changelog_parse_failure_silent_without_verbose() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/BAD-2/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "values": [{
                "id": "1",
                "author": {
                    "accountId": "u1",
                    "displayName": "Alice",
                    "emailAddress": null,
                    "active": true
                },
                "created": "not-a-date",
                "items": [{
                    "field": "status",
                    "fieldtype": "jira",
                    "from": null, "fromString": "A",
                    "to": null, "toString": "B"
                }]
            }],
            "startAt": 0,
            "maxResults": 100,
            "total": 1,
            "isLast": true
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "BAD-2"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "jr exited non-zero ({:?}). stdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("failed to parse"),
        "expected no verbose parse-failure output without --verbose, got:\n{stderr}"
    );
}

#[tokio::test]
async fn changelog_verbose_mixed_good_bad_entries() {
    // Scopes the verbose log: good entries render normally, only bad
    // entries trigger `[verbose]`, and the dedup flag still caps at
    // exactly one line across multiple bad entries.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/MIX-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "values": [
                {
                    "id": "1",
                    "author": {
                        "accountId": "u1",
                        "displayName": "Alice",
                        "emailAddress": null,
                        "active": true
                    },
                    "created": "2026-03-20T10:00:00.000+0000",
                    "items": [{
                        "field": "status",
                        "fieldtype": "jira",
                        "from": null, "fromString": "To Do",
                        "to": null, "toString": "In Progress"
                    }]
                },
                {
                    "id": "2",
                    "author": {
                        "accountId": "u1",
                        "displayName": "Alice",
                        "emailAddress": null,
                        "active": true
                    },
                    "created": "not-a-date",
                    "items": [{
                        "field": "status",
                        "fieldtype": "jira",
                        "from": null, "fromString": "In Progress",
                        "to": null, "toString": "Done"
                    }]
                },
                {
                    "id": "3",
                    "author": {
                        "accountId": "u1",
                        "displayName": "Alice",
                        "emailAddress": null,
                        "active": true
                    },
                    "created": "2026-03-21T11:00:00.000+0000",
                    "items": [{
                        "field": "resolution",
                        "fieldtype": "jira",
                        "from": null, "fromString": "Unresolved",
                        "to": null, "toString": "Done"
                    }]
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 3,
            "isLast": true
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "MIX-1", "--verbose"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "jr exited non-zero ({:?}). stdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );

    let fail_count = stderr.matches("timestamp failed to parse").count();
    assert_eq!(
        fail_count, 1,
        "expected exactly one parse-failure log across 1 bad entry among 3, got {fail_count}. stderr:\n{stderr}"
    );

    // Parseable rows must still render. Use `Unresolved` from the good
    // id=3 row — it appears in no other row's fromString/toString, so
    // its presence uniquely proves id=3 rendered. Avoiding date
    // substrings keeps this timezone-independent.
    assert!(
        stdout.contains("Unresolved"),
        "expected good-row field content ('Unresolved' from id=3) in stdout, got:\n{stdout}"
    );
    // The raw bad timestamp string surfaces in the date column of the bad row.
    assert!(
        stdout.contains("not-a-date"),
        "expected raw-timestamp fallback to appear in stdout for the bad row, got:\n{stdout}"
    );
}

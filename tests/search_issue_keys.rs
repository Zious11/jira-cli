// tests/search_issue_keys.rs
//
// Integration tests for `JiraClient::search_issue_keys` (issue #350).
//
// Pins BC-2.6.050 — keys-only JQL search posts body `fields: ["key"]`,
// deserializes only the top-level `key`, and signals caller-side
// truncation via `KeySearchResult { keys, has_more }`.
//
// Library-level tests use `jr::api::client::JiraClient::new_for_test`
// (no subprocess wiring). Pattern mirrors `tests/issue_read_holdouts.rs`.

use jr::api::client::JiraClient;
use jr::api::jira::issues::KeySearchResult;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a `JiraClient` pointing at the mock server.
fn test_client(server: &MockServer) -> JiraClient {
    JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string())
}

/// Build a `/rest/api/3/search/jql` response with the given keys and an
/// optional next-page cursor. Mirrors the minimal shape `search_issue_keys`
/// expects (top-level `key`, possibly-empty `fields {}`, cursor metadata).
fn jql_keys_response(keys: &[&str], next_page_token: Option<&str>, is_last: bool) -> serde_json::Value {
    let issues: Vec<serde_json::Value> = keys
        .iter()
        .map(|k| {
            serde_json::json!({
                "id": "10000",
                "key": k,
                "self": format!("https://example.atlassian.net/rest/api/3/issue/{}", k),
                "fields": {}
            })
        })
        .collect();
    let mut body = serde_json::json!({
        "issues": issues,
        "isLast": is_last,
    });
    if let Some(t) = next_page_token {
        body["nextPageToken"] = serde_json::json!(t);
    }
    body
}

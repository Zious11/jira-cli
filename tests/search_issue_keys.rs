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

use assert_cmd::Command;
use jr::api::client::JiraClient;
use jr::api::jira::issues::KeySearchResult;
use tempfile::TempDir;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a `JiraClient` pointing at the mock server.
fn test_client(server: &MockServer) -> JiraClient {
    JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string())
}

/// Build a `/rest/api/3/search/jql` response with the given keys and an
/// optional next-page cursor. Mirrors the minimal shape `search_issue_keys`
/// expects (top-level `key`, possibly-empty `fields {}`, cursor metadata).
fn jql_keys_response(
    keys: &[&str],
    next_page_token: Option<&str>,
    is_last: bool,
) -> serde_json::Value {
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

// ---------------------------------------------------------------------------
// AC-001 (BC-2.6.050 §1) — request body asks for ONLY the `key` field.
//
// IMPORTANT — wiremock's `body_partial_json` uses
// `assert_json_diff::assert_json_include` which has SUBSET semantics for
// arrays: a matcher built from `["key"]` would ALSO match a request whose
// `fields` array was `["key", "summary", "description"]`. That would silently
// pass while BASE_ISSUE_FIELDS leaked back in. To get true length-strict
// matching on the array, we inspect the captured request post-hoc via
// `MockServer::received_requests()` and compare with `assert_eq!` on the
// serde_json::Value, which IS length-strict for arrays.
//
// Verified via Perplexity 2026-05-13 against wiremock 0.6.5 source.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_sends_fields_key_only() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(
            &["FOO-1"],
            None,
            true,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client
        .search_issue_keys("project = FOO", Some(50))
        .await
        .expect("happy path must succeed");

    assert_eq!(result.keys, vec!["FOO-1".to_string()]);
    assert!(!result.has_more);

    // Length-strict assertion on `fields`: prove BASE_ISSUE_FIELDS is NOT sent.
    let requests = server
        .received_requests()
        .await
        .expect("wiremock must record requests");
    assert_eq!(requests.len(), 1, "exactly one request expected");
    let body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("body must be valid JSON");
    let fields = body.get("fields").expect("body must include `fields` key");
    assert_eq!(
        fields,
        &serde_json::json!(["key"]),
        "request body `fields` must be EXACTLY [\"key\"] (length-strict), got: {fields}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 (BC-2.6.050 §2) — deserialization reads only top-level `key`.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_happy_path() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(
            &["FOO-1", "FOO-2", "FOO-3"],
            None,
            true,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result: KeySearchResult = client
        .search_issue_keys("project = FOO", None)
        .await
        .expect("happy path must succeed");

    assert_eq!(
        result.keys,
        vec![
            "FOO-1".to_string(),
            "FOO-2".to_string(),
            "FOO-3".to_string()
        ],
    );
    assert!(
        !result.has_more,
        "pure exhaustion must report has_more=false"
    );
}

// ---------------------------------------------------------------------------
// AC-003 (BC-2.6.050 §3) — paginates via nextPageToken across two pages.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_paginates_with_next_page_token() {
    let server = MockServer::start().await;

    // Page 1 — has cursor.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({"jql": "q"})))
        // No nextPageToken in body → matches the first request only because
        // we mount page 2 with a higher-specificity matcher below.
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(
            &["P1-A", "P1-B"],
            Some("cursor-2"),
            false,
        )))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Page 2 — terminal.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(
            serde_json::json!({"nextPageToken": "cursor-2"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(
            &["P2-A"],
            None,
            true,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client.search_issue_keys("q", None).await.expect("ok");

    assert_eq!(result.keys, vec!["P1-A", "P1-B", "P2-A"]);
    assert!(!result.has_more);
}

// ---------------------------------------------------------------------------
// AC-004 (BC-2.6.050 §4) — has_more=true when limit is hit before exhaustion.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_truncates_at_limit_and_sets_has_more() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(
            &["FOO-1", "FOO-2", "FOO-3"],
            None,
            true,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client
        .search_issue_keys("project = FOO", Some(2))
        .await
        .expect("ok");

    assert_eq!(result.keys.len(), 2);
    assert!(result.has_more, "limit was hit → has_more must be true");
}

// ---------------------------------------------------------------------------
// AC-004 (BC-2.6.050 §4) — empty result is empty + has_more=false.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_returns_empty_for_no_matches() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(&[], None, true)))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client
        .search_issue_keys("project = NOWHERE", None)
        .await
        .expect("ok");

    assert!(result.keys.is_empty());
    assert!(!result.has_more);
}

// ---------------------------------------------------------------------------
// AC-003 (BC-2.6.050 §3) — repeated-cursor abort (JRACLOUD-95368 live-data drift).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_repeated_cursor_aborts_with_warning() {
    let server = MockServer::start().await;

    // Two pages, both return the SAME nextPageToken `"loop"`.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(
            &["X-1"],
            Some("loop"),
            false,
        )))
        // Exactly 2 hits: page 1 establishes prev_cursor = Some("loop"),
        // page 2 returns the same token and triggers the guard before a 3rd request.
        // An upper bound of 3 would mask a regression where the guard fails to fire.
        .expect(2)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client
        .search_issue_keys("q", None)
        .await
        .expect("guard must abort gracefully");

    // Keys from page 1 are kept; loop is broken before runaway.
    assert!(!result.keys.is_empty());
    // Guard-aborted pagination signals has_more=true — caller can distinguish
    // "repeated-cursor abort" from "clean cursor exhaustion".
    assert!(
        result.has_more,
        "guard abort must set has_more=true to signal incomplete results"
    );
    // `eprintln!` writes directly to the process's stderr fd; it cannot be
    // captured inside a library-level test. The literal "JRACLOUD-95368"
    // assertion for the `search_issue_keys` codepath is in the subprocess
    // test `test_search_issue_keys_stderr_emits_jracloud_95368_literal`
    // below, which exercises the same path via `jr issue edit --jql ... --dry-run`.
}

// ---------------------------------------------------------------------------
// No-dedupe contract pin (issue #361 follow-up).
//
// When the repeated-cursor guard fires under JRACLOUD-95368 live-data drift,
// earlier pages may already have collected keys that the server emits again
// on a later page before the cursor repeats. `search_issue_keys` does NOT
// dedupe — callers see the duplicates. This test pins that contract: if a
// future "helpful" dedupe is added inside `search_issue_keys`, this assertion
// fails and forces an explicit decision (with a corresponding caller-level
// migration plan).
//
// The deferred follow-up to add in-function dedupe should also update this
// test: see "New follow-up (deferred)" in
// `docs/specs/2026-05-13-search-issue-keys.md::Out of Scope / Follow-ups`.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_repeated_cursor_abort_does_not_dedupe() {
    let server = MockServer::start().await;

    // Page 1 — request body has NO `nextPageToken` (initial fetch); response
    // contains `nextPageToken: "loop"` and `["X-1"]`. Note the page-1 matcher
    // `{"jql": "q"}` is a SUBSET match and ALSO matches the page-2 request body
    // (which adds `nextPageToken: "loop"` but keeps `jql: "q"`); disambiguation
    // between pages relies on (i) `up_to_n_times(1)` exhausting this mock after
    // its single hit, and (ii) the page-2 mock below having a more specific
    // matcher on `nextPageToken: "loop"` that wiremock prefers when both pages
    // would otherwise match. Same pattern as
    // `test_search_issue_keys_paginates_with_next_page_token`.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({"jql": "q"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(
            &["X-1"],
            Some("loop"),
            false,
        )))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Page 2 — request body has `nextPageToken: "loop"`; response repeats the
    // same `nextPageToken: "loop"` and returns ["X-1", "X-2"]. This simulates
    // live-data drift mid-pagination: the server has emitted "X-1" twice (drift
    // caused the position to slide back), and is now repeating the cursor — the
    // guard must fire on the NEXT iteration check, but before it does, page 2's
    // payload is already extended into all_keys.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(
            serde_json::json!({"nextPageToken": "loop"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(
            &["X-1", "X-2"],
            Some("loop"),
            false,
        )))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client
        .search_issue_keys("q", None)
        .await
        .expect("guard must abort gracefully");

    // Contract pin: duplicates are preserved. `result.keys` MUST equal exactly
    // ["X-1", "X-1", "X-2"] — no dedupe inside search_issue_keys.
    assert_eq!(
        result.keys,
        vec!["X-1".to_string(), "X-1".to_string(), "X-2".to_string()],
        "search_issue_keys MUST NOT dedupe on repeated-cursor abort — duplicates \
         from live-data drift are passed through to callers. Callers needing \
         uniqueness must dedupe themselves or re-issue with `ORDER BY key ASC`."
    );
    assert!(
        result.has_more,
        "repeated-cursor abort must set has_more=true (incomplete results)"
    );
}

// ---------------------------------------------------------------------------
// Defense-in-depth: Apr 2025 Atlassian maxResults regression
// (community.developer.atlassian.com thread 88287). Server returns more
// rows than asked for; our `limit` truncate must still hold.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_apr2025_regression_bound() {
    let server = MockServer::start().await;

    // Server returns 500 rows in a single page despite maxResults=10.
    let many: Vec<String> = (0..500).map(|i| format!("REG-{}", i)).collect();
    let many_refs: Vec<&str> = many.iter().map(String::as_str).collect();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jql_keys_response(&many_refs, None, true)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client.search_issue_keys("q", Some(10)).await.expect("ok");

    assert_eq!(result.keys.len(), 10, "caller-side truncate must hold");
    assert!(result.has_more, "got more than limit → has_more=true");
}

// ---------------------------------------------------------------------------
// AC-002 (BC-2.6.050 §2) — unknown top-level fields are silently ignored.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_ignores_unknown_fields() {
    let server = MockServer::start().await;

    let resp = serde_json::json!({
        "issues": [
            {
                "id": "10001",
                "key": "FOO-1",
                "self": "https://example/issue/10001",
                "fields": {},
                "expand": "names",                      // unknown top-level
                "securityLevel": {"name": "Public"}     // future-hypothetical
            }
        ],
        "isLast": true,
        "expand": "names,schema"                        // unknown response-level
    });

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(resp))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client.search_issue_keys("q", None).await.expect("ok");

    assert_eq!(result.keys, vec!["FOO-1"]);
}

// ---------------------------------------------------------------------------
// AC-003 (BC-2.6.050 §3) — 401 mid-pagination propagates as Err.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_401_mid_pagination_propagates() {
    let server = MockServer::start().await;

    // Page 1: 200 with cursor.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({"jql": "q"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(
            &["P1-A"],
            Some("c2"),
            false,
        )))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Page 2: 401.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(
            serde_json::json!({"nextPageToken": "c2"}),
        ))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Authentication required"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client.search_issue_keys("q", None).await;

    assert!(result.is_err(), "401 on page 2 must propagate as Err");
}

// ---------------------------------------------------------------------------
// Malformed JSON body on page 1 → serde error propagates.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_malformed_json_returns_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"issues": [{"key": "#), // truncated mid-string
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client.search_issue_keys("q", None).await;

    assert!(result.is_err(), "malformed JSON must propagate as Err");
}

// ---------------------------------------------------------------------------
// BC-2.6.050 §5 — `.min(100)` per-page clamp is honored.
//
// Caller passes `limit = Some(200)` (> 100). The clamp must reduce
// `maxResults` to 100 in the request body. Verified by inspecting the
// captured request via `MockServer::received_requests()`.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_clamps_max_results_to_100_per_page() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(
            &["FOO-1"],
            None,
            true,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let _ = client
        .search_issue_keys("project = FOO", Some(200))
        .await
        .expect("clamp must not error");

    // Inspect the captured request body — `maxResults` must be 100, not 200.
    let requests = server
        .received_requests()
        .await
        .expect("wiremock must record requests");
    assert_eq!(requests.len(), 1, "exactly one request expected");
    let body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("body must be valid JSON");
    let max_results = body
        .get("maxResults")
        .expect("body must include `maxResults`")
        .as_u64()
        .expect("`maxResults` must be a u64");
    assert_eq!(
        max_results, 100,
        "BC-2.6.050 §5: per-page clamp must reduce caller's limit=200 to maxResults=100"
    );
}

// ---------------------------------------------------------------------------
// AC-003 (BC-2.6.050 §3) — JRACLOUD-95368 stderr-literal coverage.
//
// Subprocess test: exercises `search_issue_keys` through the real `jr` binary
// via `jr issue edit --jql ... --dry-run --no-input`.
//
// A stuck-cursor mock causes `search_issue_keys` to fire the anti-loop guard,
// which emits the JRACLOUD-95368 warning via `eprintln!` to the process's
// stderr fd. `--dry-run` short-circuits all HTTP mutations so no additional
// mocks are needed beyond the search endpoint.
//
// AC-003 requires: `String::from_utf8_lossy(&output.stderr).contains("JRACLOUD-95368")`.
//
// Citation history: the stderr literal was rebound from JRACLOUD-94632 to
// JRACLOUD-95368 by issue #361 after research showed 94632 covers an unrelated
// /search/jql defect (resolved Jun 2025). 95368 is the correctly-attributed
// root-cause ticket: "nextPageToken pagination is not snapshot-stable under
// live mutation".
// ---------------------------------------------------------------------------

/// `jr issue edit --jql 'project = X' --label add:foo --dry-run --no-input`
/// with a stuck cursor on /rest/api/3/search/jql:
///   - `search_issue_keys` fires the repeated-cursor anti-loop guard on page 2.
///   - stderr contains the literal string "JRACLOUD-95368".
///   - `--dry-run` exits 0 and produces no bulk HTTP mutations.
#[tokio::test]
async fn test_search_issue_keys_stderr_emits_jracloud_95368_literal() {
    let server = MockServer::start().await;

    // Mount a mock that always returns the same nextPageToken — simulates
    // the JRACLOUD-95368 repeated-cursor symptom: live-data drift between
    // page fetches causes the server to land on a previously-emitted offset.
    // `search_issue_keys` detects the repeated token on the second iteration
    // and breaks with a JRACLOUD-95368 warning emitted to stderr.
    //
    // `.expect(2)` pins the loop-termination invariant at the subprocess level:
    // page 1 establishes prev_cursor, page 2 triggers the guard before a 3rd
    // request. Without a bound, a regression that disables the guard would loop
    // until the 15s timeout fires, flooding CI and masking the root cause.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_keys_response(
            &["X-1"],
            Some("stuck-loop"),
            false,
        )))
        .expect(2)
        .mount(&server)
        .await;

    let config_dir = TempDir::new().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("XDG_CACHE_HOME", config_dir.path())
        .env_remove("JR_PROFILE")
        // Insulate the stderr substring assertion from locale/ANSI variation
        // in case a future change adds color to warning output.
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .env("CLICOLOR", "0")
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = X",
            "--label",
            "add:foo",
            "--dry-run",
        ])
        .timeout(std::time::Duration::from_secs(15))
        .output()
        .expect("jr subprocess must not time out");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // AC-003 literal assertion: the JRACLOUD-95368 warning must appear in stderr
    // when search_issue_keys detects a repeated cursor in the new codepath.
    assert!(
        stderr.contains("JRACLOUD-95368"),
        "AC-003: stderr must contain 'JRACLOUD-95368' when search_issue_keys \
         fires the anti-loop guard. Got stderr: {stderr}"
    );
}

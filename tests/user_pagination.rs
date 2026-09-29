//! End-to-end coverage for `--all` true pagination on `user search` and
//! `user list` (#189). Library-level tests assert that `_all` variants loop
//! the endpoint until an empty page is returned and that pages are
//! concatenated in order. CLI-level tests verify the flag wiring in handlers.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

use jr::api::client::JiraClient;

/// Build a `jr` command pre-configured for non-interactive JSON output
/// against a mock server. Matches the pattern used in tests/all_flag_behavior.rs.
#[allow(dead_code)]
fn jr_cmd_json(server_uri: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "--output", "json"]);
    cmd
}

/// Build a user-search fixture of `count` users with names/ids derived from `prefix`.
/// Constructs the JSON response directly from owned `String`s to avoid leaking
/// memory just to satisfy the borrowed-string fixture signature.
fn users_page(count: usize, prefix: &str) -> Value {
    let users: Vec<Value> = (0..count)
        .map(|i| {
            serde_json::json!({
                "accountId": format!("{prefix}-acc-{i:03}"),
                "displayName": format!("{prefix} User {i:03}"),
                "emailAddress": format!("{prefix}.user.{i:03}@test.com"),
                "active": true,
            })
        })
        .collect();
    Value::Array(users)
}

/// `search_users_all` paginates three sequential pages (100 + 100 + 27)
/// and returns 227 users concatenated in order. `startAt` advances by the
/// requested page size (100) each iteration regardless of returned count —
/// Jira uses fixed-window pagination, so advancing by the short page's
/// length would re-scan already-seen raw users.
#[tokio::test]
async fn search_users_all_paginates_and_concatenates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "100"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "200"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(27, "p3")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "300"))
        .and(query_param("maxResults", "100"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client
        .search_users_all("u")
        .await
        .expect("pagination must succeed");
    assert_eq!(users.len(), 227, "expected 227 users across 3 pages");
    assert_eq!(users[0].display_name, "p1 User 000");
    assert_eq!(users[100].display_name, "p2 User 000");
    assert_eq!(users[200].display_name, "p3 User 000");
    assert_eq!(users[226].display_name, "p3 User 026");
}

/// Loop stops as soon as a page comes back empty; subsequent startAt
/// windows are not requested. The strict `.expect(1)` on each mock,
/// combined with wiremock rejecting unmatched requests, asserts that
/// any fourth request would fail.
#[tokio::test]
async fn search_users_all_stops_on_empty_page() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "100"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users_all("u").await.expect("must succeed");
    assert_eq!(users.len(), 100);
}

/// If the API never returns an empty page (pathological behavior), the loop
/// stops at USER_PAGINATION_SAFETY_CAP iterations = 15 requests.
#[tokio::test]
async fn search_users_all_respects_safety_cap() {
    let server = MockServer::start().await;

    // Unbounded responder for any startAt; .expect(15) pins the iteration cap.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "cap")))
        .expect(15)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users_all("u").await.expect("must succeed");
    assert_eq!(users.len(), 1500, "15 iterations * 100 per page = 1500");
}

/// If a page request fails mid-pagination, the error is propagated and the
/// loop does not silently return partial results.
#[tokio::test]
async fn search_users_all_propagates_error_mid_pagination() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.search_users_all("u").await;
    let err = result.expect_err("500 on page 2 must propagate");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("500"),
        "error must surface the 500 status, got: {msg}"
    );
}

/// Atlassian docs warn that the user-search endpoint "usually returns fewer
/// users than specified in maxResults" due to post-page filtering. A short
/// non-empty page is NOT end-of-data; the loop must keep paginating until it
/// sees a truly empty page. Pins two contracts at once: (a) short response
/// doesn't trigger early termination, and (b) `startAt` advances by the
/// requested window size (100) regardless of returned count — advancing by
/// 35 would re-scan users[35..100] and produce duplicates per JRACLOUD-71293.
#[tokio::test]
async fn search_users_all_continues_past_short_non_empty_page() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(35, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "200"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p3")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "300"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users_all("u").await.expect("must succeed");
    assert_eq!(
        users.len(),
        235,
        "must keep paginating past a short non-empty page (100 + 35 + 100)"
    );
}

/// `search_assignable_users_by_project_all` paginates the assignable-users
/// endpoint and concatenates pages in order.
#[tokio::test]
async fn search_assignable_users_by_project_all_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("query", ""))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "100"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(40, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "200"))
        .and(query_param("maxResults", "100"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client
        .search_assignable_users_by_project_all("", "FOO")
        .await
        .expect("pagination must succeed");
    assert_eq!(users.len(), 140);
    assert_eq!(users[0].display_name, "p1 User 000");
    assert_eq!(users[100].display_name, "p2 User 000");
}

/// End-to-end: `jr user search --all` paginates and emits all users as JSON.
#[tokio::test]
async fn user_search_all_cli_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(50, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "200"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "search", "u", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user search --all JSON is an array");
    assert_eq!(arr.len(), 150, "--all should paginate to 150 users");
}

/// End-to-end: `jr user list --all --project FOO` paginates.
#[tokio::test]
async fn user_list_all_cli_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(35, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "200"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "list", "--project", "FOO", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user list --all JSON is an array");
    assert_eq!(arr.len(), 135);
}

/// Without `--all`, `jr user search` must still make exactly one API request
/// (the existing single-call path) — no accidental pagination.
/// Asserts via `received_requests()` that the request query string contains
/// no `startAt` or `maxResults` — a loose matcher like `query_param("query", "u")`
/// would still match a paginated request, so the post-hoc inspection is
/// required to actually guard against accidental pagination regression.
#[tokio::test]
async fn user_search_no_all_issues_single_request() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(50, "u")))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "search", "u"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user search JSON is an array");
    assert_eq!(
        arr.len(),
        30,
        "default cap should truncate to 30, got {}",
        arr.len()
    );

    // Primary guard: verify the actual request the binary sent contains no
    // pagination parameters. Without `--all` the single-call path must not
    // send `startAt` or `maxResults`.
    let requests = server
        .received_requests()
        .await
        .expect("received_requests must be recording");
    assert_eq!(requests.len(), 1, "expected exactly one API request");
    let query = requests[0].url.query().unwrap_or("");
    assert!(
        !query.contains("startAt"),
        "single-call path must not send startAt; got query: {query}"
    );
    assert!(
        !query.contains("maxResults"),
        "single-call path must not send maxResults; got query: {query}"
    );
}

/// End-to-end: when the server never returns an empty page, the safety cap
/// bites and the command emits a stderr warning so the truncation is
/// observable instead of silent. Pins the user-visible warning contract.
#[tokio::test]
async fn user_search_all_cli_emits_safety_cap_warning() {
    let server = MockServer::start().await;

    // Every request (any startAt) returns a full 100-user page — the loop
    // never sees an empty response and must exit via the safety cap.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "cap")))
        .expect(15)
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "search", "u", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "command must still succeed when cap hits; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hit pagination safety cap"),
        "stderr must contain the safety-cap warning so truncation is observable; got: {stderr}"
    );
}

/// Same safety-cap warning contract for `user list --all` (assignable users
/// endpoint). The two paginated methods have parallel warning behavior; both
/// need explicit coverage so a refactor of either one doesn't silently drop
/// the stderr notice.
#[tokio::test]
async fn user_list_all_cli_emits_safety_cap_warning() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "cap")))
        .expect(15)
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "list", "--project", "FOO", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "command must still succeed when cap hits; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hit pagination safety cap"),
        "stderr must contain the safety-cap warning so truncation is observable; got: {stderr}"
    );
}

// ── AC-007 (BC-X.7.002 Postcondition 5, cycle-014 STORY-A) ─────────────────
//
// `--all` pagination must carry the RESOLVED project key on every page, for
// both the global-flag and configured-default resolution paths. Both tests
// build their own `Command`, hermetic per
// `.factory/cycles/cycle-014/phase-f2-spec-evolution/verification-delta.md`
// §2 (fresh JR_CONFIG_DIR/JR_CACHE_DIR, every other ambient JR_* var
// scrubbed) — deliberately NOT `jr_cmd_json` above, which sets only
// JR_BASE_URL/JR_AUTH_HEADER with no config/cache isolation.

/// Hermetic `jr` command builder for the AC-007 pagination cells below.
/// `JR_CONFIG_DIR`/`JR_CACHE_DIR` use the `<home>/jr` convention
/// `global_config_dir()`'s `JR_CONFIG_DIR` debug seam expects directly
/// (matches `tests/multi_profile_fields.rs`/`tests/user_list_project_resolution.rs`).
/// Every other ambient `JR_*` variable is scrubbed via
/// `common::hermetic::scrub_ambient_jr_env` (F-002) so a stray
/// developer/CI-set value can't leak a configured default into these
/// tests; the scrub runs before the seams below are set, so it never
/// removes them (see that helper's doc comment).
fn jr_cmd_hermetic(
    server_uri: &str,
    cache_home: &std::path::Path,
    config_home: &std::path::Path,
) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    common::hermetic::scrub_ambient_jr_env(
        &mut cmd,
        &[
            "JR_BASE_URL",
            "JR_AUTH_HEADER",
            "JR_CACHE_DIR",
            "JR_CONFIG_DIR",
        ],
    );
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_CACHE_DIR", cache_home.join("jr"))
        .env("JR_CONFIG_DIR", config_home.join("jr"))
        .args(["--no-input", "--output", "json"]);
    cmd
}

/// Writes a single-profile `config.toml` (`default_profile = "default"`)
/// with an optional profile-level `project` default. Duplicated (not
/// imported) from `tests/user_list_project_resolution.rs` because each
/// integration-test file compiles as its own separate binary crate.
fn write_default_profile_config(
    config_home: &std::path::Path,
    base_url: &str,
    project: Option<&str>,
) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    let project_line = project
        .map(|p| format!("project = \"{p}\"\n"))
        .unwrap_or_default();
    std::fs::write(
        dir.join("config.toml"),
        format!(
            "default_profile = \"default\"\n[profiles.default]\nurl = \"{base_url}\"\nauth_method = \"api_token\"\n{project_line}"
        ),
    )
    .unwrap();
}

/// AC-007, global-flag variant: `jr --project FOO user list --all`
/// paginates through 3 pages (100 + 35 + 0), every page carrying exactly
/// one `projectKeys=FOO` pair. Resolves via clap's own global-value
/// propagation straight through to `search_assignable_users_by_project_all`.
/// F-001 (Step 4.5 adversarial pass 1): a `query_param_is_missing`
/// catch-all mock pins that `projectKeys` is never omitted, and
/// `server.received_requests()` is inspected directly to prove every
/// request carries exactly one `projectKeys` pair whose value is `FOO` —
/// the full VP(c) Postcondition-5 assertion, not just the total user count.
#[tokio::test]
async fn test_user_list_all_sends_single_project_keys_param_per_page_via_global_flag() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    common::hermetic::assert_no_ancestor_jr_toml(cwd.path());
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(35, "p2")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "200"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;
    // F-001: catch-all tripwire — any request missing `projectKeys` entirely
    // must never occur.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param_is_missing("projectKeys"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_hermetic(&server.uri(), cache.path(), config.path())
        .args(["--project", "FOO", "user", "list", "--all"])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user list --all JSON is an array");
    assert_eq!(arr.len(), 135);

    // F-001: every request the binary actually sent carries exactly one
    // `projectKeys` pair, with value `FOO` — proves the resolved key is
    // applied consistently to every page, not merely present somewhere.
    let requests = server
        .received_requests()
        .await
        .expect("received_requests must be recording");
    assert_eq!(requests.len(), 3, "expected exactly 3 paginated requests");
    for req in &requests {
        let project_keys: Vec<_> = req
            .url
            .query_pairs()
            .filter(|(k, _)| k == "projectKeys")
            .collect();
        assert_eq!(
            project_keys.len(),
            1,
            "expected exactly one projectKeys pair, got {project_keys:?} for {}",
            req.url
        );
        assert_eq!(
            project_keys[0].1, "FOO",
            "expected projectKeys=FOO, got {:?} for {}",
            project_keys[0].1, req.url
        );
    }
}

/// AC-007, configured-default variant: `jr user list --all` (no
/// local/global flag) with a profile-configured project default (`FOO`)
/// paginates the same way, proving the resolved key is applied to every
/// page, not just the first. `project` is `None` at `handle_list`'s entry
/// and is resolved via `resolve_user_list_project`'s configured-default
/// fallback chain before pagination begins.
/// F-001 (Step 4.5 adversarial pass 1): a `query_param_is_missing`
/// catch-all mock pins that `projectKeys` is never omitted, and
/// `server.received_requests()` is inspected directly to prove every
/// request carries exactly one `projectKeys` pair whose value is `FOO` —
/// the full VP(c) Postcondition-5 assertion, not just the total user count.
#[tokio::test]
async fn test_user_list_all_sends_single_project_keys_param_per_page_via_configured_default() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    common::hermetic::assert_no_ancestor_jr_toml(cwd.path());
    let server = MockServer::start().await;
    write_default_profile_config(config.path(), &server.uri(), Some("FOO"));

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(35, "p2")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "200"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;
    // F-001: catch-all tripwire — any request missing `projectKeys` entirely
    // must never occur.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param_is_missing("projectKeys"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_hermetic(&server.uri(), cache.path(), config.path())
        .args(["user", "list", "--all"])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user list --all JSON is an array");
    assert_eq!(arr.len(), 135);

    // F-001: every request the binary actually sent carries exactly one
    // `projectKeys` pair, with value `FOO` — proves the resolved key is
    // applied consistently to every page, not merely present somewhere.
    let requests = server
        .received_requests()
        .await
        .expect("received_requests must be recording");
    assert_eq!(requests.len(), 3, "expected exactly 3 paginated requests");
    for req in &requests {
        let project_keys: Vec<_> = req
            .url
            .query_pairs()
            .filter(|(k, _)| k == "projectKeys")
            .collect();
        assert_eq!(
            project_keys.len(),
            1,
            "expected exactly one projectKeys pair, got {project_keys:?} for {}",
            req.url
        );
        assert_eq!(
            project_keys[0].1, "FOO",
            "expected projectKeys=FOO, got {:?} for {}",
            project_keys[0].1, req.url
        );
    }
}

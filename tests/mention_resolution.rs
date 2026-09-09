//! Red Gate integration tests for effectful mention resolution
//! (S-cycle5-mention-resolution-wiring, issue #674).
//!
//! Covers `src/cli/issue/mentions.rs::resolve_mentions`/`filter_by_name_match`
//! (both `todo!()` stubs as of this commit) and their wiring into the four
//! write-command call sites: `issue create` (platform), `issue edit` (dry-run
//! + live), `issue comment add`/`issue comment edit`, and JSM
//! `issue create --request-type`.
//!
//! `resolve_mentions`/`filter_by_name_match` are `pub(super)` — inaccessible
//! from this file — so every assertion here goes through the `jr` binary via
//! `assert_cmd` + `wiremock`, mirroring `tests/duplicate_user_disambiguation.rs`'s
//! style (per `artifact-mapping.md` §3 and `verification-delta-674.md` §6).
//!
//! ALL tests in this file are expected to FAIL before implementation (Red
//! Gate): none of the four call sites currently invoke `resolve_mentions` at
//! all (they still call bare `adf::markdown_to_adf`/`JsmRequestBuilder::build`
//! unconditionally), so every resolution-triggering assertion here (call
//! counts, exit codes, `attrs.text` presence, zero-mutation guarantees) fails
//! for real, observable reasons — never a build error, and never a `todo!()`
//! panic (since the stub is never reached from these call sites yet).
//!
//! BC refs: BC-X.7.007/008/009/010 (cross-cutting.md §X.7), BC-3.3.012,
//! BC-3.4.032, BC-3.5.013, BC-3.8.018 (bc-3-issue-write.md).
//! VP refs: VP-674-002/003/009/010/011/013/014/015/016/017/019/020/021.
//! Holdout refs: H-NEW-MENTION-001/002/003/004/006/008/010/011/012.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

/// Build a `jr` command wired to a mock server, with isolated XDG dirs so no
/// test touches a developer's real config/cache.
fn jr_cmd(server_url: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("JR_CACHE_DIR", cache_dir.join("jr"))
        .env("XDG_CONFIG_HOME", config_dir)
        .env("JR_CONFIG_DIR", config_dir.join("jr"));
    cmd
}

/// Convenience: a fresh `jr` command with fresh throwaway temp dirs. Returns
/// the command plus the `TempDir` guards, which MUST be kept alive for the
/// duration of the subprocess call.
fn jr_cmd_fresh(server_url: &str) -> (Command, tempfile::TempDir, tempfile::TempDir) {
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cmd = jr_cmd(server_url, cache_dir.path(), config_dir.path());
    (cmd, cache_dir, config_dir)
}

/// Mount `GET /rest/api/3/user/search?query=<query>` returning a 200 with the
/// given raw JSON array body. Optionally pass an exact expected call count.
async fn mount_search(server: &MockServer, query: &str, users: Value, expect: Option<u64>) {
    let mut mock = Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", query))
        .respond_with(ResponseTemplate::new(200).set_body_json(users));
    if let Some(n) = expect {
        mock = mock.expect(n);
    }
    mock.mount(server).await;
}

/// Mount `GET /rest/api/3/user?accountId=<id>` returning 200 with a user
/// object (bracket-form preflight success).
async fn mount_get_user(
    server: &MockServer,
    account_id: &str,
    display_name: &str,
    active: bool,
    expect: Option<u64>,
) {
    let mut mock = Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", account_id))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": account_id,
            "displayName": display_name,
            "active": active,
        })));
    if let Some(n) = expect {
        mock = mock.expect(n);
    }
    mock.mount(server).await;
}

/// Mount `GET /rest/api/3/user?accountId=<id>` returning a failure status
/// (404/400/401/403/5xx) with the given body.
async fn mount_get_user_error(server: &MockServer, account_id: &str, status: u16, body: Value) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", account_id))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(server)
        .await;
}

/// Mount `POST /rest/api/3/issue/<key>/comment` returning 201 with a minimal
/// comment response. `expect` bounds the exact call count (0 for a
/// must-not-be-called guard).
async fn mount_post_comment(server: &MockServer, key: &str, expect: u64) {
    Mock::given(method("POST"))
        .and(path(format!("/rest/api/3/issue/{key}/comment")))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id": "10050"})))
        .expect(expect)
        .mount(server)
        .await;
}

/// Mount `PUT /rest/api/3/issue/<key>/comment/<id>` returning 200.
async fn mount_put_comment(server: &MockServer, key: &str, id: &str, expect: u64) {
    Mock::given(method("PUT"))
        .and(path(format!("/rest/api/3/issue/{key}/comment/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": id})))
        .expect(expect)
        .mount(server)
        .await;
}

/// Mount `POST /rest/api/3/issue` returning 201 with the given key.
async fn mount_post_issue(server: &MockServer, key: &str, expect: u64) {
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "10001",
            "key": key,
            "self": format!("{}/rest/api/3/issue/{key}", server.uri())
        })))
        .expect(expect)
        .mount(server)
        .await;
}

/// Mount `PUT /rest/api/3/issue/<key>` returning 204.
async fn mount_put_issue(server: &MockServer, key: &str, expect: u64) {
    Mock::given(method("PUT"))
        .and(path(format!("/rest/api/3/issue/{key}")))
        .respond_with(ResponseTemplate::new(204))
        .expect(expect)
        .mount(server)
        .await;
}

/// Mount the wrong (scoped) user-search endpoint with `.expect(0)` — a
/// regression guard for the Architecture Compliance Rule that the resolver
/// MUST use the unscoped `/user/search`, never `multiProjectSearch`.
async fn mount_wrong_search_endpoint_must_not_be_called(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .expect(0)
        .mount(server)
        .await;
}

// ---------------------------------------------------------------------------
// ADF body-inspection helpers
// ---------------------------------------------------------------------------

/// Recursively collect every `{"type":"mention", ...}` node in an ADF value.
fn collect_mentions(v: &Value, out: &mut Vec<Value>) {
    if v.get("type").and_then(Value::as_str) == Some("mention") {
        out.push(v.clone());
    }
    if let Some(children) = v.get("content").and_then(Value::as_array) {
        for c in children {
            collect_mentions(c, out);
        }
    }
}

fn mentions_in(body: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    collect_mentions(body, &mut out);
    out
}

fn has_mention_with_id_and_text(body: &Value, id: &str, text: &str) -> bool {
    mentions_in(body)
        .iter()
        .any(|m| m["attrs"]["id"].as_str() == Some(id) && m["attrs"]["text"].as_str() == Some(text))
}

fn has_mention_with_id(body: &Value, id: &str) -> bool {
    mentions_in(body)
        .iter()
        .any(|m| m["attrs"]["id"].as_str() == Some(id))
}

/// Recursively concatenate every plain `text` node's string content.
fn collect_text(v: &Value, out: &mut String) {
    if v.get("type").and_then(Value::as_str) == Some("text") {
        if let Some(s) = v.get("text").and_then(Value::as_str) {
            out.push_str(s);
        }
    }
    if let Some(children) = v.get("content").and_then(Value::as_array) {
        for c in children {
            collect_text(c, out);
        }
    }
}

fn all_text_in(body: &Value) -> String {
    let mut out = String::new();
    collect_text(body, &mut out);
    out
}

/// Extract the JSON body of the first request matching `method`+`path_suffix`.
async fn captured_body(server: &MockServer, http_method: &str, path_suffix: &str) -> Value {
    let reqs = server.received_requests().await.expect("requests recorded");
    let req = reqs
        .iter()
        .find(|r| r.method.as_str() == http_method && r.url.path().ends_with(path_suffix))
        .unwrap_or_else(|| panic!("no {http_method} request to *{path_suffix} was recorded"));
    serde_json::from_slice(&req.body).expect("request body must be valid JSON")
}

// ===========================================================================
// AC-001 (BC-X.7.007 EC-1 / BC-X.7.010 point 1; VP-674-009/013): dedup
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_x_7_007_at_name_repeated_three_times_dedupes_to_one_search_call() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        "smith",
        json!([{"accountId": "acc-1", "displayName": "John Smith", "active": true}]),
        Some(1),
    )
    .await;
    mount_wrong_search_endpoint_must_not_be_called(&server).await;
    mount_post_comment(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc @smith — @smith — @smith please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");

    let body = captured_body(&server, "POST", "/comment").await;
    assert!(
        has_mention_with_id_and_text(&body["body"], "acc-1", "@John Smith"),
        "expected resolved mention node; body={body}"
    );
    // The `.expect(1)` mounts above are verified at MockServer drop.
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_x_7_010_bracket_form_repeated_three_times_dedupes_to_one_get_call() {
    let server = MockServer::start().await;
    mount_get_user(
        &server,
        "5b10ac8d82e05b22cc7d4349",
        "Jane Doe",
        true,
        Some(1),
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc [~accountid:5b10ac8d82e05b22cc7d4349] [~accountid:5b10ac8d82e05b22cc7d4349] [~accountid:5b10ac8d82e05b22cc7d4349]",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");

    let body = captured_body(&server, "POST", "/comment").await;
    assert!(
        has_mention_with_id_and_text(&body["body"], "5b10ac8d82e05b22cc7d4349", "@Jane Doe"),
        "expected resolved mention node with attrs.text; body={body}"
    );
}

// ===========================================================================
// H-NEW-MENTION-001 (BC-X.7.010, BC-7.2.017, BC-3.5.013): bracket-form
// preflight success, ordering, attrs.text
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_h_new_mention_001_bracket_form_preflight_before_post_populates_attrs_text() {
    let server = MockServer::start().await;
    mount_get_user(
        &server,
        "5b10ac8d82e05b22cc7d4349",
        "Jane Doe",
        true,
        Some(1),
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc [~accountid:5b10ac8d82e05b22cc7d4349] please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");
    assert!(
        !stderr.contains("not found"),
        "success path must not carry a hard-error string; stderr={stderr}"
    );

    // Ordering: the preflight GET must have happened before the POST.
    let reqs = server.received_requests().await.unwrap();
    let get_idx = reqs
        .iter()
        .position(|r| r.method.as_str() == "GET" && r.url.path() == "/rest/api/3/user")
        .expect("GET /user?accountId= must have been recorded");
    let post_idx = reqs
        .iter()
        .position(|r| r.method.as_str() == "POST" && r.url.path().ends_with("/comment"))
        .expect("POST comment must have been recorded");
    assert!(
        get_idx < post_idx,
        "preflight GET must precede the mutation POST; order={reqs:?}"
    );

    let body = captured_body(&server, "POST", "/comment").await;
    assert!(
        has_mention_with_id_and_text(&body["body"], "5b10ac8d82e05b22cc7d4349", "@Jane Doe"),
        "expected mention node with attrs.text=='@Jane Doe'; body={body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_x_7_010_bracket_form_404_hard_error_exit64_zero_post() {
    let server = MockServer::start().await;
    mount_get_user_error(
        &server,
        "deadbeef",
        404,
        json!({"errorMessages": ["The user does not exist or you don't have permission"]}),
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc [~accountid:deadbeef] please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("not found"),
        "expected 'not found' substring; stderr={stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ec_x_7_010_2_two_distinct_bracket_ids_both_must_succeed() {
    let server = MockServer::start().await;
    mount_get_user(&server, "acc-a", "Alice A", true, Some(1)).await;
    mount_get_user(&server, "acc-b", "Bob B", true, Some(1)).await;
    mount_post_comment(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc [~accountid:acc-a] and [~accountid:acc-b]",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");
    let body = captured_body(&server, "POST", "/comment").await;
    assert!(has_mention_with_id(&body["body"], "acc-a"));
    assert!(has_mention_with_id(&body["body"], "acc-b"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ec_x_7_010_3_bracket_form_401_propagates_standard_error_not_wrapped_as_not_found() {
    let server = MockServer::start().await;
    mount_get_user_error(
        &server,
        "acc-1",
        401,
        json!({"errorMessages": ["Client must be authenticated to access this resource."]}),
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc [~accountid:acc-1] please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "a 401 during preflight must propagate as the standard NotAuthenticated \
         mapping (exit 2), not a mention-specific 'not found' error; stderr={stderr}"
    );
    assert!(
        !stderr.contains("not found"),
        "401 must NOT be re-wrapped as a 'not found' error; stderr={stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ec_x_7_010_3_bracket_form_500_propagates_standard_error_not_wrapped_as_not_found() {
    let server = MockServer::start().await;
    mount_get_user_error(
        &server,
        "acc-1",
        500,
        json!({"errorMessages": ["Internal server error"]}),
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc [~accountid:acc-1] please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a 5xx during preflight must propagate as the standard ApiError mapping \
         (exit 1), not exit 64; stderr={stderr}"
    );
    assert!(
        !stderr.contains("not found"),
        "5xx must NOT be re-wrapped as a 'not found' error; stderr={stderr}"
    );
}

// ===========================================================================
// H-NEW-MENTION-002 / AC-002/003 (BC-X.7.007): unique-match happy path
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_h_new_mention_002_at_name_unique_name_matching_result_resolves() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        "smith",
        json!([{"accountId": "acc-1", "displayName": "John Smith", "active": true}]),
        Some(1),
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc @smith — @smith please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");
    let body = captured_body(&server, "POST", "/comment").await;
    assert!(has_mention_with_id_and_text(
        &body["body"],
        "acc-1",
        "@John Smith"
    ));
}

// ===========================================================================
// H-NEW-MENTION-012 / VP-674-021 (BC-X.7.007 point 2): single-result
// name-match tightening
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_h_new_mention_012_single_non_name_matching_result_hard_errors_no_deactivated_hint() {
    let server = MockServer::start().await;
    // "jsmith" is NOT a case-insensitive substring of "John Smith".
    mount_search(
        &server,
        "jsmith",
        json!([{"accountId": "acc-1", "displayName": "John Smith", "active": true}]),
        Some(1),
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc @jsmith please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 (regression to disambiguate_user's len==1 short-circuit \
         would silently resolve and exit 0); stderr={stderr}"
    );
    assert!(
        stderr.contains("No user found matching"),
        "expected the pinned empty-list-branch substring; stderr={stderr}"
    );
    assert!(
        !stderr.contains("deactivated"),
        "the sole hit is ACTIVE — the neutral wording must be used, not the \
         deactivated-hint wording; stderr={stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_vp_674_021_single_name_matching_result_resolves() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        "smith",
        json!([{"accountId": "acc-1", "displayName": "John Smith", "active": true}]),
        None,
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc @smith please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected exit 0; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body = captured_body(&server, "POST", "/comment").await;
    assert!(has_mention_with_id(&body["body"], "acc-1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_vp_674_021_case3_multi_result_reduces_to_lone_name_match_resolves_no_ambiguity() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        "smith",
        json!([
            {"accountId": "acc-1", "displayName": "John Smith", "active": true},
            {"accountId": "acc-2", "displayName": "Jane Doe", "active": true}
        ]),
        None,
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc @smith please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "reducing 2 raw results to a lone name-match must resolve silently, \
         with NO ambiguity prompt/error; stderr={stderr}"
    );
    assert!(
        !stderr.contains("Multiple users"),
        "must not trigger the ambiguous-disambiguation path; stderr={stderr}"
    );
    let body = captured_body(&server, "POST", "/comment").await;
    assert!(has_mention_with_id_and_text(
        &body["body"],
        "acc-1",
        "@John Smith"
    ));
}

// ===========================================================================
// H-NEW-MENTION-003 / AC-004 (BC-X.7.008): ambiguous disambiguation
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_h_new_mention_003_exact_multiple_non_interactive_exit64_zero_post() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        "jane",
        json!([
            {"accountId": "acc-1", "displayName": "Jane Doe", "active": true, "emailAddress": "jane.doe@example.com"},
            {"accountId": "acc-2", "displayName": "Jane Doe", "active": true, "emailAddress": "jane.doe2@example.com"}
        ]),
        None,
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc @jane please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("Multiple users named"),
        "must reuse disambiguate_user's ExactMultiple wording verbatim; stderr={stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_x_7_008_ambiguous_non_exact_non_interactive_exit64_zero_post() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        "jan",
        json!([
            {"accountId": "acc-1", "displayName": "Janet Roe", "active": true},
            {"accountId": "acc-2", "displayName": "Janice Doe", "active": true}
        ]),
        None,
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc @jan please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("Multiple users match"),
        "must reuse disambiguate_user's Ambiguous wording verbatim; stderr={stderr}"
    );
}

// ===========================================================================
// H-NEW-MENTION-004 / AC-005 (BC-X.7.009): zero-match three-way messaging
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_h_new_mention_004_zero_raw_results_hard_error_exit64_not_exit0() {
    let server = MockServer::start().await;
    mount_search(&server, "nobody", json!([]), None).await;
    mount_post_comment(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc @nobody please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(64),
        "this is the human-approved HARD-ERROR override — NOT exit 0/pass-through; \
         stderr={stderr}"
    );
    assert!(
        stderr.contains("No user found matching"),
        "expected the pinned substring; stderr={stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "stdout must be empty on this error path; stdout={stdout}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_x_7_009_all_matches_deactivated_hard_error_with_deactivated_hint() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        "retired",
        json!([{"accountId": "acc-1", "displayName": "Retired Person", "active": false}]),
        None,
    )
    .await;
    mount_post_comment(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc @retired please review",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("No user found matching"),
        "expected the pinned substring; stderr={stderr}"
    );
    assert!(
        stderr.contains("deactivated"),
        "an all-inactive raw result set MUST carry the deactivated hint \
         (wording (ii)); stderr={stderr}"
    );
}

// ===========================================================================
// AC-007 (BC-X.7.008/009/010 zero-mutation guarantees): all-or-nothing
// across a mixed body
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ac_007_mixed_resolvable_and_unresolvable_fails_whole_write_zero_post() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        "smith",
        json!([{"accountId": "acc-1", "displayName": "John Smith", "active": true}]),
        Some(1),
    )
    .await;
    mount_search(&server, "nobody", json!([]), Some(1)).await;
    mount_post_comment(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc @smith and @nobody",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "the whole write must fail even though @smith resolved; stderr={stderr}"
    );
    assert!(
        stderr.contains("No user found matching"),
        "expected the @nobody failure substring; stderr={stderr}"
    );
    // Both `.expect(1)` mounts above additionally assert that resolution is
    // attempted for EVERY candidate, not short-circuited at the first failure.
}

// ===========================================================================
// AC-008 (BC-3.3.012): `issue create` (platform) wiring
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_3_012_create_platform_resolves_bracket_mention_before_post() {
    let server = MockServer::start().await;
    mount_get_user(
        &server,
        "5b10ac8d82e05b22cc7d4349",
        "Jane Doe",
        true,
        Some(1),
    )
    .await;
    mount_post_issue(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Test",
            "--description",
            "cc [~accountid:5b10ac8d82e05b22cc7d4349]",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");

    let reqs = server.received_requests().await.unwrap();
    let get_idx = reqs
        .iter()
        .position(|r| r.method.as_str() == "GET" && r.url.path() == "/rest/api/3/user")
        .expect("preflight GET must have been recorded");
    let post_idx = reqs
        .iter()
        .position(|r| r.method.as_str() == "POST" && r.url.path() == "/rest/api/3/issue")
        .expect("create POST must have been recorded");
    assert!(get_idx < post_idx, "GET must precede POST; order={reqs:?}");

    let body = captured_body(&server, "POST", "/rest/api/3/issue").await;
    let desc = &body["fields"]["description"];
    assert!(
        has_mention_with_id_and_text(desc, "5b10ac8d82e05b22cc7d4349", "@Jane Doe"),
        "expected resolved mention node in description ADF; desc={desc}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_h_new_mention_010_create_mixed_resolvable_unresolvable_fails_whole_create_zero_post()
{
    let server = MockServer::start().await;
    mount_search(
        &server,
        "jsmith",
        json!([{"accountId": "acc-1", "displayName": "John Smith", "active": true}]),
        Some(1),
    )
    .await;
    mount_search(&server, "nobody", json!([]), Some(1)).await;
    mount_post_issue(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Test",
            "--description",
            "cc @jsmith and @nobody",
            "--markdown",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("No user found matching"),
        "expected the @nobody failure substring; stderr={stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ec_3_3_012_2_no_mentions_without_markdown_is_silent_noop() {
    let server = MockServer::start().await;
    mount_post_issue(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Test",
            "--description",
            "cc @jsmith",
            "--no-mentions",
            "--no-input",
        ])
        .output()
        .unwrap();

    // No --markdown at all: `--no-mentions` alone must not error or change
    // routing — text_to_adf's path was never going to detect mentions anyway.
    assert!(
        output.status.success(),
        "expected exit 0 (silent no-op); stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ===========================================================================
// AC-009/AC-010 (BC-3.4.032): `issue edit` dry-run + live wiring
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_032_edit_live_resolves_before_put() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        "smith",
        json!([{"accountId": "acc-1", "displayName": "John Smith", "active": true}]),
        Some(1),
    )
    .await;
    mount_put_issue(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "--no-input",
            "issue",
            "edit",
            "PROJ-1",
            "--description",
            "cc @smith",
            "--markdown",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected exit 0; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = captured_body(&server, "PUT", "/rest/api/3/issue/PROJ-1").await;
    let desc = &body["fields"]["description"];
    assert!(
        has_mention_with_id_and_text(desc, "acc-1", "@John Smith"),
        "expected resolved mention node in PUT description ADF; desc={desc}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_032_edit_live_resolution_failure_exit64_zero_put() {
    let server = MockServer::start().await;
    mount_search(&server, "nobody", json!([]), None).await;
    mount_put_issue(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "--no-input",
            "issue",
            "edit",
            "PROJ-1",
            "--description",
            "cc @nobody",
            "--markdown",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64; stderr={stderr}"
    );
    assert!(stderr.contains("No user found matching"), "stderr={stderr}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_h_new_mention_011_edit_dry_run_resolution_failure_empty_stdout_json() {
    let server = MockServer::start().await;
    mount_search(&server, "nobody", json!([]), None).await;
    mount_put_issue(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "--no-input",
            "issue",
            "edit",
            "PROJ-1",
            "--description",
            "cc @nobody",
            "--markdown",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64; stderr={stderr} stdout={stdout}"
    );
    assert!(
        stdout.trim().is_empty(),
        "stdout must be EMPTY on a mention-resolution dry-run error (mirrors \
         the MAX_ADF_DEPTH no-leak invariant, VP-692-002/-004); stdout={stdout}"
    );
    assert!(
        stderr.contains("No user found matching"),
        "expected the pinned substring; stderr={stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_h_new_mention_011_edit_dry_run_resolution_failure_empty_stdout_table() {
    let server = MockServer::start().await;
    mount_search(&server, "nobody", json!([]), None).await;
    mount_put_issue(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "--no-input",
            "issue",
            "edit",
            "PROJ-1",
            "--description",
            "cc @nobody",
            "--markdown",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64; stderr={stderr} stdout={stdout}"
    );
    assert!(
        stdout.trim().is_empty(),
        "no partial table-mode preview may leak to stdout before the exit-64 \
         return; stdout={stdout}"
    );
    assert!(
        stderr.starts_with("Error: "),
        "stderr must carry the canonical 'Error: ...' prefix in table mode; \
         stderr={stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_vp_674_020_dry_run_forces_non_interactive_resolution_no_prompt() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        "jane",
        json!([
            {"accountId": "acc-1", "displayName": "Jane Doe", "active": true, "emailAddress": "jane.doe@example.com"},
            {"accountId": "acc-2", "displayName": "Jane Doe", "active": true, "emailAddress": "jane.doe2@example.com"}
        ]),
        None,
    )
    .await;
    mount_put_issue(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    // JR_STDIN_IS_TTY=1 forces TTY mode WITHOUT --no-input — if the dry-run
    // path incorrectly threaded the ambient no_input instead of forcing
    // `true`, this would pop a dialoguer::Select prompt and hang (no stdin
    // is provided here, so a hang would manifest as this call blocking
    // until the test harness's own timeout kills it).
    let output = cmd
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "edit",
            "PROJ-1",
            "--description",
            "cc @jane please review",
            "--markdown",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "dry-run must ALWAYS force the non-interactive branch, even at a TTY \
         without --no-input; stderr={stderr}"
    );
    assert!(
        stderr.contains("Multiple users named"),
        "expected the non-interactive candidate-list wording, not a prompt; \
         stderr={stderr}"
    );
}

// ===========================================================================
// AC-011/AC-012 (BC-3.5.013): comment add/edit wiring + visibility
// orthogonality
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_5_013_comment_add_resolution_failure_exit64_zero_post_new_guarantee() {
    let server = MockServer::start().await;
    mount_search(&server, "nobody", json!([]), None).await;
    mount_post_comment(&server, "PROJ-1", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc @nobody",
            "--markdown",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "this is a NEW zero-POST guarantee for comment add; stderr={stderr}"
    );
    assert!(stderr.contains("No user found matching"), "stderr={stderr}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_5_013_comment_edit_resolves_before_put() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        "smith",
        json!([{"accountId": "acc-1", "displayName": "John Smith", "active": true}]),
        Some(1),
    )
    .await;
    mount_put_comment(&server, "FOO-1", "10001", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "cc @smith",
            "--markdown",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");
    let body = captured_body(&server, "PUT", "/comment/10001").await;
    assert!(has_mention_with_id_and_text(
        &body["body"],
        "acc-1",
        "@John Smith"
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_5_013_comment_edit_resolution_failure_exit64_zero_put() {
    let server = MockServer::start().await;
    mount_search(&server, "nobody", json!([]), None).await;
    mount_put_comment(&server, "FOO-1", "10001", 0).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "cc @nobody",
            "--markdown",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64; stderr={stderr}"
    );
    assert!(stderr.contains("No user found matching"), "stderr={stderr}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_vp_674_011_internal_visibility_orthogonal_to_mention_resolution() {
    let server = MockServer::start().await;
    mount_get_user(&server, "acc-1", "Jane Doe", true, Some(1)).await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .and(body_partial_json(json!({
            "properties": [{"key": "sd.public.comment", "value": {"internal": true}}]
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id": "10050"})))
        .expect(1)
        .mount(&server)
        .await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            "cc [~accountid:acc-1]",
            "--markdown",
            "--internal",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");
    let body = captured_body(&server, "POST", "/comment").await;
    assert!(
        has_mention_with_id(&body["body"], "acc-1"),
        "a mention-bearing internal comment must still resolve the mention; body={body}"
    );
    // The `.expect(1)` body_partial_json mount above verifies the
    // sd.public.comment property is present, unmutated by mention presence.
}

// ===========================================================================
// AC-013 / H-NEW-MENTION-008 (BC-3.8.018): JSM create wiring
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_h_new_mention_008_jsm_create_resolves_mention_before_synchronous_build() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELP"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "99",
            "key": "HELP",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 50, "isLastPage": true, "_links": {},
            "values": [{"id": "10", "projectId": "99", "projectKey": "HELP", "projectName": "Help Center"}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 50, "isLastPage": true, "_links": {},
            "values": [{"id": "10", "name": "General Request", "description": "General support request"}]
        })))
        .mount(&server)
        .await;

    mount_search(
        &server,
        "jsmith",
        json!([{"accountId": "acc-1", "displayName": "John Smith", "active": true}]),
        Some(1),
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"issueKey": "HELP-1"})))
        .expect(1)
        .mount(&server)
        .await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "General Request",
            "--summary",
            "Need help",
            "--description",
            "cc @jsmith for context",
            "--markdown",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");

    let reqs = server.received_requests().await.unwrap();
    let get_idx = reqs
        .iter()
        .position(|r| r.method.as_str() == "GET" && r.url.path() == "/rest/api/3/user/search")
        .expect("resolver GET must have been recorded");
    let post_idx = reqs
        .iter()
        .position(|r| r.method.as_str() == "POST" && r.url.path() == "/rest/servicedeskapi/request")
        .expect("JSM POST must have been recorded");
    assert!(
        get_idx < post_idx,
        "mention resolution must complete BEFORE JsmRequestBuilder::build()'s \
         synchronous POST; order={reqs:?}"
    );

    let body = captured_body(&server, "POST", "/rest/servicedeskapi/request").await;
    let desc = &body["requestFieldValues"]["description"];
    assert!(
        has_mention_with_id_and_text(desc, "acc-1", "@John Smith"),
        "expected resolved mention node in requestFieldValues.description; desc={desc}"
    );
}

// ===========================================================================
// AC-014/AC-015 / H-NEW-MENTION-006: `--no-mentions` zero-resolver-HTTP at
// all four call sites
// ===========================================================================

const NO_MENTIONS_BODY: &str = "cc @jsmith and [~accountid:5b10ac8d82e05b22cc7d4349]";

async fn mount_zero_expect_resolver_endpoints(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .expect(0)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": "5b10ac8d82e05b22cc7d4349",
            "displayName": "Jane Doe",
            "active": true
        })))
        .expect(0)
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_h_new_mention_006_no_mentions_comment_add_zero_resolver_http() {
    let server = MockServer::start().await;
    mount_zero_expect_resolver_endpoints(&server).await;
    mount_post_comment(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "add",
            "PROJ-1",
            NO_MENTIONS_BODY,
            "--markdown",
            "--no-mentions",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");
    let body = captured_body(&server, "POST", "/comment").await;
    assert!(
        mentions_in(&body["body"]).is_empty(),
        "no mention node may be emitted under --no-mentions; body={body}"
    );
    let text = all_text_in(&body["body"]);
    assert!(
        text.contains("@jsmith") && text.contains("[~accountid:5b10ac8d82e05b22cc7d4349]"),
        "both forms must survive as literal, unconverted text; text={text}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ac_015_no_mentions_comment_edit_zero_resolver_http() {
    let server = MockServer::start().await;
    mount_zero_expect_resolver_endpoints(&server).await;
    mount_put_comment(&server, "FOO-1", "10001", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            NO_MENTIONS_BODY,
            "--markdown",
            "--no-mentions",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");
    let body = captured_body(&server, "PUT", "/comment/10001").await;
    assert!(
        mentions_in(&body["body"]).is_empty(),
        "no mention node may be emitted under --no-mentions; body={body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ac_015_no_mentions_create_zero_resolver_http() {
    let server = MockServer::start().await;
    mount_zero_expect_resolver_endpoints(&server).await;
    mount_post_issue(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "Test",
            "--description",
            NO_MENTIONS_BODY,
            "--markdown",
            "--no-mentions",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");
    let body = captured_body(&server, "POST", "/rest/api/3/issue").await;
    assert!(
        mentions_in(&body["fields"]["description"]).is_empty(),
        "no mention node may be emitted under --no-mentions; desc={}",
        body["fields"]["description"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ac_015_no_mentions_edit_zero_resolver_http() {
    let server = MockServer::start().await;
    mount_zero_expect_resolver_endpoints(&server).await;
    mount_put_issue(&server, "PROJ-1", 1).await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "--no-input",
            "issue",
            "edit",
            "PROJ-1",
            "--description",
            NO_MENTIONS_BODY,
            "--markdown",
            "--no-mentions",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");
    let body = captured_body(&server, "PUT", "/rest/api/3/issue/PROJ-1").await;
    assert!(
        mentions_in(&body["fields"]["description"]).is_empty(),
        "no mention node may be emitted under --no-mentions; desc={}",
        body["fields"]["description"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ac_015_no_mentions_jsm_create_zero_resolver_http() {
    let server = MockServer::start().await;
    mount_zero_expect_resolver_endpoints(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELP"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "99", "key": "HELP", "projectTypeKey": "service_desk", "simplified": false
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 50, "isLastPage": true, "_links": {},
            "values": [{"id": "10", "projectId": "99", "projectKey": "HELP", "projectName": "Help Center"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 50, "isLastPage": true, "_links": {},
            "values": [{"id": "10", "name": "General Request", "description": "General support request"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"issueKey": "HELP-1"})))
        .expect(1)
        .mount(&server)
        .await;

    let (mut cmd, _cache, _config) = jr_cmd_fresh(&server.uri());
    let output = cmd
        .args([
            "issue",
            "create",
            "--project",
            "HELP",
            "--request-type",
            "General Request",
            "--summary",
            "Need help",
            "--description",
            NO_MENTIONS_BODY,
            "--markdown",
            "--no-mentions",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "expected exit 0; stderr={stderr}");
    let body = captured_body(&server, "POST", "/rest/servicedeskapi/request").await;
    assert!(
        mentions_in(&body["requestFieldValues"]["description"]).is_empty(),
        "no mention node may be emitted under --no-mentions on the JSM path; desc={}",
        body["requestFieldValues"]["description"]
    );
}

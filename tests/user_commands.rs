#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
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
    // Asserts: `jr user list` with no resolvable project exits 64 (jr's own
    // JrError::UserError, not clap's exit 2), stderr mentions `--project`,
    // and stderr carries no connection-error text. The no-HTTP check is a
    // PROXY: with the unreachable JR_BASE_URL, an attempted request would
    // surface connection-error text, but its absence does not strictly prove
    // zero requests (the sibling wiremock `.expect(0)` test does).
    //
    // Hermetic per BC-X.7.002 Preconditions / verification-delta.md §2
    // (cycle-014 STORY-A, issue #862): once BC-X.7.002 landed, this test
    // became CONFIG-SENSITIVE — a real developer/CI environment with a
    // configured default project (.jr.toml or profile default) would
    // silently resolve step 3 and this test's failure assertion would
    // spuriously fail. JR_CONFIG_DIR/JR_CACHE_DIR point at fresh TempDirs,
    // cwd has no ancestor .jr.toml, and every other ambient JR_* var is
    // scrubbed. The unreachable JR_BASE_URL=http://127.0.0.1:1 is
    // intentionally kept, with no mock server: a stray request fails with a
    // connection error rather than a mock response, which this test's
    // no-connection-text assertion rejects.
    //
    // The failure this test pins is never clap's own "required argument"
    // error (exit 2) — it is jr's own JrError::UserError exit-64 message
    // (BC-X.7.002 Invariants: the name is retained, no rename).
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    common::hermetic::assert_no_ancestor_jr_toml(cwd.path());

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
    cmd.env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_CACHE_DIR", cache.path().join("jr"))
        .env("JR_CONFIG_DIR", config.path().join("jr"))
        .args(["--no-input", "user", "list"])
        .current_dir(cwd.path());

    let output = cmd.output().unwrap();

    // FIX-P5-006 (P5-005): strengthened from "any failure mentioning
    // --project or 'required'" (which a connection error to the unreachable
    // JR_BASE_URL could not satisfy, but a clap exit-2 usage error could).
    // Pin jr's own exit-64 UserError, a `--project` mention, and the absence
    // of any connection-error text (a proxy for the guard firing before HTTP).
    assert_eq!(
        output.status.code(),
        Some(64),
        "missing --project must exit 64 (UserError), not clap's exit 2"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--project"),
        "expected error mentions missing --project, got: {stderr}"
    );
    assert!(
        !stderr.contains("127.0.0.1:1") && !stderr.to_lowercase().contains("connection"),
        "guard must fire before any HTTP attempt, got: {stderr}"
    );
}

/// AC-004 / EC-X.7.002-4 (BC-X.7.002 Postcondition 4, VP-USER-LIST-PROJECT-001(c)
/// EC-4 cell, cycle-014 STORY-A): none of {local --project, global
/// --project, configured default} present → exit 64 JrError::UserError with
/// the byte-identical pinned message, before any HTTP call. Hermetic per
/// verification-delta.md §2. Unlike `user_list_requires_project_flag`
/// above, this test needs a REAL wiremock server so it can assert
/// `.expect(0)` on the assignable-users endpoint (proving zero HTTP calls
/// rather than merely an unreachable URL / connection error).
#[tokio::test]
async fn test_user_list_without_resolvable_project_exits_64_with_zero_http() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    common::hermetic::assert_no_ancestor_jr_toml(cwd.path());
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;

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
    cmd.env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_CACHE_DIR", cache.path().join("jr"))
        .env("JR_CONFIG_DIR", config.path().join("jr"))
        .args(["--no-input", "user", "list"])
        .current_dir(cwd.path());

    let output = cmd.output().unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "No project configured. Run \"jr init\" or pass --project. Run \"jr project list\" to see available projects."
        ),
        "expected the byte-identical pinned exit-64 message, got: {stderr}"
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

/// F-006 (Step 4.5 adversarial pass 1): `handle_view`'s downcast branch
/// treats a 400 identically to a 404 (`*status == 404 || *status == 400`,
/// `src/cli/user.rs::handle_view`) — both rewrite into the same friendly
/// "not found" `JrError::UserError`, exit 64. Companion to
/// `user_view_404_shows_friendly_error` above, now that `handle_view` is in
/// mutation scope.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_user_view_400_response_rewrites_to_not_found_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", "bad-request-user"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": ["Invalid accountId"]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["user", "view", "bad-request-user"])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(64),
        "view on a 400 response should exit 64 via the same not-found branch as 404, got: {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("User with accountId 'bad-request-user' not found"),
        "expected the 404-equivalent friendly not-found message for a 400, got: {stderr}"
    );
}

/// F-006 (Step 4.5 adversarial pass 1): a 500 does NOT match `handle_view`'s
/// `*status == 404 || *status == 400` guard, so it falls through to
/// `return Err(e)` unrewritten — the raw `JrError::ApiError` surfaces
/// (exit 1, "API error (500): ..."), never the "not found" message.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_user_view_500_response_surfaces_raw_api_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", "server-error-user"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["user", "view", "server-error-user"])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(1),
        "view on a 500 response should exit 1 (unrewritten JrError::ApiError), got: {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("API error (500)"),
        "expected the raw API error surfaced for a 500, got: {stderr}"
    );
    assert!(
        !stderr.contains("not found"),
        "a 500 must not be rewritten into the not-found message, got: {stderr}"
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

// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn user_search_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["user", "search", "test"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "5xx should exit 1, got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("API error (500)"),
        "Expected 'API error (500)' in stderr, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn user_search_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["user", "search", "test"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "401 should exit 2, got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Not authenticated"),
        "Expected 'Not authenticated' in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "Expected 'jr auth login' suggestion in stderr, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn user_search_network_drop_surfaces_reach_error() {
    // Privileged port 1 — connect-refused from any unprivileged process.
    let output = jr_cmd("http://127.0.0.1:1")
        .args(["user", "search", "test"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "Net-drop should exit 1, got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Could not reach"),
        "Expected 'Could not reach' in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("check your connection"),
        "Expected 'check your connection' in stderr, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

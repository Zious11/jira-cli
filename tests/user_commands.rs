#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::fixtures;

/// Every ambient `JR_`-prefixed variable the hermetic tests below do NOT
/// themselves set, pinned per `verification-delta.md` §2 step 5 (mirrors
/// `tests/auth_profiles.rs`'s pinned scrub list) so a stray `JR_*` variable
/// in the ambient shell (e.g. a direnv-set `JR_PROFILE`) cannot leak a
/// configured default into these BC-X.7.002 tests.
fn scrub_ambient_jr_env(cmd: &mut Command) -> &mut Command {
    cmd.env_remove("JR_PROFILE")
        .env_remove("JR_DEFAULT_PROFILE")
        .env_remove("JR_INSTANCE_URL")
        .env_remove("JR_INSTANCE_AUTH_METHOD")
        .env_remove("JR_INSTANCE_CLOUD_ID")
        .env_remove("JR_INSTANCE_ORG_ID")
        .env_remove("JR_INSTANCE_OAUTH_SCOPES")
        .env_remove("JR_FIELDS_TEAM_FIELD_ID")
        .env_remove("JR_FIELDS_STORY_POINTS_FIELD_ID")
        .env_remove("JR_DEFAULTS_OUTPUT")
        .env_remove("JR_EMAIL")
        .env_remove("JR_API_TOKEN")
        .env_remove("JR_OAUTH_CLIENT_ID")
        .env_remove("JR_OAUTH_CLIENT_SECRET")
}

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
    // --project/required stderr assertion rejects — so it still proves zero
    // successful HTTP calls without needing a live mock server.
    //
    // Once this fix lands, the failure this test pins is never clap's own
    // "required argument" error — it is jr's own JrError::UserError exit-64
    // message, which also contains the literal substring "--project",
    // satisfying the same loose assertion (BC-X.7.002 Invariants: this
    // test's assertion continues to accurately describe what it checks
    // after the fix lands — no rename).
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_CACHE_DIR", cache.path().join("jr"))
        .env("JR_CONFIG_DIR", config.path().join("jr"))
        .args(["--no-input", "user", "list"])
        .current_dir(cwd.path());
    scrub_ambient_jr_env(&mut cmd);

    let output = cmd.output().unwrap();

    assert!(!output.status.success(), "missing --project should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--project") || stderr.contains("required"),
        "expected error mentions missing --project, got: {stderr}"
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
/// Classification: RED-at-stub — `project` is `None` at `handle_list`'s
/// entry, reaching the stub's `todo!()` via the child process (exit 101),
/// whose stderr satisfies neither half of the pinned-message assertion.
#[tokio::test]
async fn user_list_no_project_resolvable_exits_64_zero_http() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_CACHE_DIR", cache.path().join("jr"))
        .env("JR_CONFIG_DIR", config.path().join("jr"))
        .args(["--no-input", "user", "list"])
        .current_dir(cwd.path());
    scrub_ambient_jr_env(&mut cmd);

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

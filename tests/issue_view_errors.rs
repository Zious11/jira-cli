//! Error-path coverage (#187) for `jr issue view`.
//!
//! Note: `handle_view` in `src/cli/issue/list.rs` calls
//! `get_or_fetch_cmdb_fields(client).await.unwrap_or_default()` BEFORE fetching
//! the issue. On a cache miss that call hits `/rest/api/3/field`, but
//! `unwrap_or_default()` swallows any error, so only the `/rest/api/3/issue/...`
//! endpoint needs mocking. If that swallow is ever removed (e.g. CMDB errors
//! propagate), these tests must also mock `/rest/api/3/field` or set
//! `XDG_CACHE_HOME` to a prewarmed tempdir.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn issue_view_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "view", "PROJ-1"])
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
async fn issue_view_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "view", "PROJ-1"])
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
async fn issue_view_network_drop_surfaces_reach_error() {
    // Privileged port 1 — connect-refused from any unprivileged process.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "view", "PROJ-1"])
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

/// Corrupt `teams.json` must be non-fatal: `jr issue view` keeps running and
/// the Team row shows the UUID with an actionable hint pointing the user at
/// `jr team list --refresh` (same path as a cold cache).
///
/// Behavior follows `src/cache.rs:23-26` — `serde_json::from_str` failures
/// return `Ok(None)` rather than propagating the parse error. The `Ok(None)`
/// branch at `src/cli/issue/list.rs:947` surfaces the `(name not cached —
/// run 'jr team list --refresh')` hint inline in the table. See issue #194
/// for the divergence from the original "stderr warning" proposal.
#[tokio::test]
async fn issue_view_corrupt_team_cache_falls_back_gracefully() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let jr_cache_dir = cache_dir.path().join("jr");
    std::fs::create_dir_all(&jr_cache_dir).unwrap();
    // Truncated JSON — serde_json::from_str returns Err, which read_cache
    // maps to Ok(None) per src/cache.rs:23-26.
    std::fs::write(jr_cache_dir.join("teams.json"), "{ not json").unwrap();

    let jr_config_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&jr_config_dir).unwrap();
    std::fs::write(
        jr_config_dir.join("config.toml"),
        "[fields]\nteam_field_id = \"customfield_10001\"\n",
    )
    .unwrap();

    let team_uuid = "36885b3c-1bf0-4f85-a357-c5b858c31de4";
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_team(
                "PROJ-1",
                "Issue with team",
                "customfield_10001",
                team_uuid,
            ),
        ))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "view", "PROJ-1"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Corrupt team cache should be non-fatal, stderr: {stderr}, stdout: {stdout}"
    );
    assert!(
        stdout.contains(team_uuid),
        "Output should show the UUID so the user can identify the team, stdout: {stdout}"
    );
    assert!(
        stdout.contains("name not cached") && stdout.contains("jr team list --refresh"),
        "Output should guide the user to refresh the cache, stdout: {stdout}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

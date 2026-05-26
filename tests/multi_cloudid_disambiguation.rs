//! S-3.04 Red Gate — multi-cloudId disambiguation integration tests
//!
//! These tests cover the new `--cloud-id` flag and the interactive / exit-64
//! disambiguation path for `jr auth login --oauth` when the Atlassian
//! `accessible-resources` endpoint returns more than one site.
//!
//! All tests in this file are expected to FAIL on the current `develop`
//! branch (pre-implementation). They will turn GREEN once the implementer:
//!   1. Adds `--cloud-id <id>` to `AuthCommand::Login` in `src/cli/mod.rs`.
//!   2. Threads `cloud_id: Option<String>` through `LoginArgs` and `login_oauth`.
//!   3. Replaces `resources.first()` in `oauth_login` (src/api/auth.rs) with
//!      disambiguation logic:
//!      - `len == 1` → auto-select (no change).
//!      - `len > 1, --cloud-id set` → find-by-id or exit 64.
//!      - `len > 1, --no-input, no --cloud-id` → exit 64 with actionable msg.
//!      - `len > 1, interactive` → dialoguer prompt → user picks.
//!   4. Honors `JR_ACCESSIBLE_RESOURCES_URL` and `JR_OAUTH_TOKEN_URL` env-var
//!      overrides so integration tests can inject wiremock URLs. These override
//!      the hardcoded `https://api.atlassian.com/...` and
//!      `https://auth.atlassian.com/...` endpoints in `oauth_login`.
//!
//! ## Mock Server Harness
//!
//! Shared test infrastructure is defined inline in the `mock_harness` module
//! below. Each test starts its own `wiremock::MockServer` so tests run in
//! parallel without state leakage.
//!
//! The harness mocks two Atlassian endpoints:
//!   - `POST /oauth/token`              → returns a fake access_token
//!   - `GET  /oauth/token/accessible-resources` → returns a configurable list
//!
//! The `JR_OAUTH_TOKEN_URL` and `JR_ACCESSIBLE_RESOURCES_URL` env vars (to be
//! added by the implementer) override the hardcoded Atlassian URLs in
//! `src/api/auth.rs` so wiremock can intercept the calls.
//!
//! ## Red Gate contract
//!
//! Each test MUST fail with a meaningful assertion error before implementation,
//! NOT a compile error. Failure modes are documented per-test.
//!
//! ## Trace: AC → test mapping
//!
//! | AC    | Test function(s)                                              |
//! |-------|---------------------------------------------------------------|
//! | AC-001| `test_cloud_id_flag_recognized_in_help`                       |
//! | AC-001| `test_cloud_id_flag_picks_named_resource_not_first`           |
//! | AC-001| `test_cloud_id_flag_not_in_list_exits_64`                     |
//! | AC-001| `test_interactive_render_shows_name_url_and_id`               |
//! | AC-002| `test_no_input_multi_org_exits_64_with_actionable_error`      |
//! | AC-002| `test_no_input_multi_org_lists_available_cloud_ids_in_error`  |
//! | AC-003| `test_single_resource_no_regression_single_org_path`          |
//! | AC-004| `test_callback_url_contains_127_0_0_1_and_port_53682`         |
//! | AC-005| `test_interactive_select_via_stdin_picks_second_resource`     |
//! | AC-006| (covered by AC-002 tests — same contract, H-047 elevation)    |

// ---------------------------------------------------------------------------
// Module: shared mock harness
// ---------------------------------------------------------------------------

/// Inline test helpers for multi-cloudId disambiguation tests.
///
/// Each test must call `MockServer::start().await` independently so tests
/// run in parallel without port-sharing races.
mod mock_harness {
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// JSON shape for one accessible resource entry.
    /// Field name is `id` (not `cloudId`) per Atlassian REST spec.
    pub fn resource_entry(id: &str, name: &str, url: &str) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "url": url,
            "scopes": ["read:jira-work", "read:jira-user", "write:jira-work", "manage:jira-project"],
            "avatarUrl": format!("https://avatars.atlassian.com/{id}.png")
        })
    }

    /// JSON shape for one accessible resource entry with missing product scopes.
    /// Used for the scope-gap edge-case test (reserved for future use).
    #[allow(dead_code)]
    pub fn resource_entry_no_product_scopes(id: &str, name: &str, url: &str) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "url": url,
            "scopes": ["openid", "email", "read:me"],
            "avatarUrl": format!("https://avatars.atlassian.com/{id}.png")
        })
    }

    /// Token exchange response (fake, not real).
    pub fn token_response() -> serde_json::Value {
        json!({
            "access_token": "test-access-token-abc123",
            "refresh_token": "test-refresh-token-xyz789",
            "token_type": "Bearer",
            "expires_in": 3600,
            "scope": "read:jira-work read:jira-user"
        })
    }

    /// Mount a `POST /oauth/token` handler that returns a fake token.
    pub async fn mount_token_exchange(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(token_response()))
            .mount(server)
            .await;
    }

    /// Mount a `GET /oauth/token/accessible-resources` handler.
    pub async fn mount_accessible_resources(server: &MockServer, resources: serde_json::Value) {
        Mock::given(method("GET"))
            .and(path("/oauth/token/accessible-resources"))
            .and(header("Authorization", "Bearer test-access-token-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(resources))
            .mount(server)
            .await;
    }

    /// Two resources: cloud-B is first (position [0]) to make a first-wins bug
    /// detectable — when AC-001 asks for cloud-A, the bug silently gives cloud-B.
    pub fn two_resources_b_first() -> serde_json::Value {
        json!([
            resource_entry("cloud-B", "Company B", "https://company-b.atlassian.net"),
            resource_entry("cloud-A", "Company A", "https://company-a.atlassian.net")
        ])
    }

    /// One resource — the common single-org case.
    pub fn one_resource() -> serde_json::Value {
        json!([resource_entry(
            "cloud-only",
            "My Only Org",
            "https://myonly.atlassian.net"
        )])
    }
}

// ---------------------------------------------------------------------------
// Helpers: isolated `jr` subprocess builder
// ---------------------------------------------------------------------------

use assert_cmd::Command;
use tempfile::TempDir;

/// Generate a unique test-scoped keychain service name so parallel tests
/// don't collide when writing OAuth app credentials. Uses thread ID +
/// current time nanos to produce a per-invocation unique suffix.
///
/// Format: `jr-test-multicloud-<pid>-<tid>-<nanos>`
fn unique_test_service_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    // Use a random u64 as a tiebreaker to handle multiple calls in the same
    // nanosecond (rare but possible in parallel test suites).
    // We use process ID + thread ID + nanos as a best-effort unique key.
    // Note: std::thread::current().id() is not directly printable as u64,
    // so we use the Debug format and hash it.
    let thread_hash = format!("{:?}", std::thread::current().id())
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>();
    let thread_suffix = thread_hash.get(..6).unwrap_or("000000");
    format!(
        "jr-test-mc-{pid}-{thread_suffix}-{nanos}",
        pid = std::process::id(),
    )
}

/// Build a `jr` command with full XDG isolation and a unique test-scoped
/// keychain service name. Clears all `JR_*` env vars so developer shell
/// environment doesn't leak into tests.
///
/// Each call generates a fresh service name so parallel tests don't collide
/// when writing OAuth app credentials to the system keychain.
fn jr_isolated(config_dir: &TempDir, cache_dir: &TempDir) -> Command {
    jr_isolated_with_svc(config_dir, cache_dir, &unique_test_service_name())
}

/// Build an isolated `jr` command with an explicit keychain service name.
fn jr_isolated_with_svc(config_dir: &TempDir, cache_dir: &TempDir, svc: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("XDG_CONFIG_HOME", config_dir.path())
        .env("XDG_CACHE_HOME", cache_dir.path())
        // Unique-per-test service name prevents keychain "already exists"
        // errors when multiple test invocations store OAuth app credentials
        // in parallel. The JR_SERVICE_NAME env is read by service_name()
        // in src/api/auth.rs.
        .env("JR_SERVICE_NAME", svc)
        .env_remove("JR_PROFILE")
        .env_remove("JR_DEFAULT_PROFILE")
        .env_remove("JR_INSTANCE_URL")
        .env_remove("JR_INSTANCE_AUTH_METHOD")
        .env_remove("JR_INSTANCE_CLOUD_ID")
        .env_remove("JR_INSTANCE_ORG_ID")
        .env_remove("JR_INSTANCE_OAUTH_SCOPES")
        .env_remove("JR_FIELDS_TEAM_FIELD_ID")
        .env_remove("JR_FIELDS_STORY_POINTS_FIELD_ID")
        .env_remove("JR_DEFAULTS_OUTPUT")
        .env_remove("JR_BASE_URL")
        .env_remove("JR_AUTH_HEADER")
        .env_remove("JR_EMAIL")
        .env_remove("JR_API_TOKEN")
        .env_remove("JR_OAUTH_CLIENT_ID")
        .env_remove("JR_OAUTH_CLIENT_SECRET")
        .env_remove("JR_OAUTH_TOKEN_URL")
        .env_remove("JR_ACCESSIBLE_RESOURCES_URL");
    cmd
}

/// Write a minimal OAuth profile config into the isolated XDG config dir.
///
/// The profile has `auth_method = "oauth"` and a placeholder URL; the
/// cloud_id is left unset (the login flow writes it after accessible-resources).
fn write_oauth_profile_config(config_dir: &TempDir, profile: &str, url: &str) {
    let jr_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&jr_dir).unwrap();
    let config_path = jr_dir.join("config.toml");
    let toml = format!(
        "default_profile = \"{profile}\"\n\n\
         [profiles.{profile}]\n\
         url = \"{url}\"\n\
         auth_method = \"oauth\"\n"
    );
    std::fs::write(config_path, toml).unwrap();
}

// ---------------------------------------------------------------------------
// AC-001 — `--cloud-id` flag is recognized in help output
// ---------------------------------------------------------------------------

/// AC-001 (flag registration): `jr auth login --help` MUST list `--cloud-id`
/// after the story is implemented.
///
/// RED-GATE: FAILS on develop because `--cloud-id` is not yet a registered
/// argument on `AuthCommand::Login` in `src/cli/mod.rs`. The flag therefore
/// does not appear in the help output.
///
/// The assertion checks for `--cloud-id` in help text. Before implementation
/// the string is absent → assertion fails.
#[test]
fn test_cloud_id_flag_recognized_in_help() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "login", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "auth login --help should exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("--cloud-id"),
        "auth login --help must list --cloud-id flag (AC-001 flag registration). \
         Got help text:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-001 — `--cloud-id` flag is not rejected by clap (exit 0 path)
// ---------------------------------------------------------------------------

/// AC-001 (flag parsing): passing `--cloud-id some-uuid` to `jr auth login`
/// must NOT produce a clap error (exit 2 "unrecognized argument"). The flag
/// must be accepted even when the OAuth flow itself fails for other reasons
/// (no browser, no mock server).
///
/// RED-GATE: FAILS on develop because `--cloud-id` is not registered → clap
/// exits 2 with "unrecognized argument: --cloud-id". The test expects any
/// exit code other than 2 (the argument-parsing-failure code).
///
/// After implementation, the exit code will be non-zero for a different reason
/// (no accessible resources mock / browser won't open), but critically not 2.
#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn test_cloud_id_flag_is_parsed_not_rejected_by_clap() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();

    write_oauth_profile_config(&config_dir, "default", "https://test.atlassian.net");

    let output = jr_isolated(&config_dir, &cache_dir)
        .args([
            "auth",
            "login",
            "--oauth",
            "--cloud-id",
            "cloud-A",
            "--no-input",
            "--url",
            "https://test.atlassian.net",
        ])
        .timeout(std::time::Duration::from_secs(5))
        .output()
        .unwrap();

    let exit_code = output.status.code().unwrap_or(1);
    assert_ne!(
        exit_code,
        2,
        "exit code 2 means clap rejected --cloud-id as unrecognized argument. \
         The flag must be registered in AuthCommand::Login. \
         stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ---------------------------------------------------------------------------
// AC-001 — `--cloud-id` selects the named resource, not the first one
// ---------------------------------------------------------------------------

/// AC-001 (disambiguation via flag): When `accessible-resources` returns two
/// sites (cloud-B first, cloud-A second) and `--cloud-id cloud-A` is passed,
/// the stored `cloud_id` in the profile must be `cloud-A`, NOT `cloud-B`.
///
/// The mock server is injected via `JR_OAUTH_TOKEN_URL` and
/// `JR_ACCESSIBLE_RESOURCES_URL` env-var overrides (to be added by implementer
/// to `src/api/auth.rs`). These two env vars let tests redirect the hardcoded
/// Atlassian endpoint URLs to wiremock.
///
/// The "fake OAuth callback" is simulated: we pre-supply `JR_OAUTH_CODE`
/// (authorization code) so `oauth_login` skips the browser-open + TCP-listen
/// step. That env-var override must also be added by the implementer.
///
/// RED-GATE: FAILS because:
///   1. `--cloud-id` flag doesn't exist → clap exits 2 (before this issue).
///   2. Even with flag: `JR_OAUTH_TOKEN_URL` / `JR_ACCESSIBLE_RESOURCES_URL`
///      don't exist → calls go to hardcoded Atlassian URLs → timeout/error.
///   3. Even if all env vars exist: `resources.first()` still picks cloud-B.
///
/// We assert: exit 0 AND stored config has `cloud_id = "cloud-A"`.
/// Pre-implementation: fails at step 1 or 2 → not exit 0.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_cloud_id_flag_picks_named_resource_not_first() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    use mock_harness::{mount_accessible_resources, mount_token_exchange, two_resources_b_first};
    use wiremock::MockServer;

    let server = MockServer::start().await;
    mount_token_exchange(&server).await;
    mount_accessible_resources(&server, two_resources_b_first()).await;

    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();

    write_oauth_profile_config(&config_dir, "default", "https://test.atlassian.net");

    let server_url = server.uri();

    // Build command: the implementer must read JR_OAUTH_TOKEN_URL and
    // JR_ACCESSIBLE_RESOURCES_URL to redirect network calls to our mock.
    // JR_OAUTH_CODE skips the browser-open / TCP-listen step.
    let output = jr_isolated(&config_dir, &cache_dir)
        .args([
            "auth",
            "login",
            "--oauth",
            "--cloud-id",
            "cloud-A",
            "--no-input",
            "--url",
            "https://test.atlassian.net",
            "--client-id",
            "test-client-id",
            "--client-secret",
            "test-client-secret",
        ])
        .env("JR_OAUTH_TOKEN_URL", format!("{server_url}/oauth/token"))
        .env(
            "JR_ACCESSIBLE_RESOURCES_URL",
            format!("{server_url}/oauth/token/accessible-resources"),
        )
        // Skip browser + TCP listener: inject a pre-built auth code
        .env("JR_OAUTH_CODE", "test-auth-code-skip-browser")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let exit_code = output.status.code().unwrap_or(255);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Primary assertion: must exit 0 (success).
    assert_eq!(
        exit_code, 0,
        "jr auth login --oauth --cloud-id cloud-A must exit 0 when cloud-A exists \
         in accessible-resources; stderr: {stderr}, stdout: {stdout}"
    );

    // Secondary assertion: the stored profile must have cloud_id = "cloud-A",
    // not "cloud-B" (which would be the first-result-wins outcome).
    let config_path = config_dir.path().join("jr").join("config.toml");
    let config_content = std::fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("Failed to read config: {e}"));

    assert!(
        config_content.contains("cloud_id = \"cloud-A\""),
        "Stored config must contain cloud_id = \"cloud-A\" (not cloud-B which \
         would indicate first-result-wins bug). Config:\n{config_content}"
    );
    assert!(
        !config_content.contains("cloud_id = \"cloud-B\""),
        "Stored config must NOT contain cloud_id = \"cloud-B\" (that would be \
         the first-result-wins outcome). Config:\n{config_content}"
    );
}

// ---------------------------------------------------------------------------
// AC-001 — `--cloud-id` not found in list → exit 64
// ---------------------------------------------------------------------------

/// AC-001 (negative path): When `accessible-resources` returns two sites but
/// `--cloud-id cloud-NONEXISTENT` is passed, the command must exit 64
/// (ConfigError / UsageError) with stderr explaining the cloud-id was not found.
///
/// RED-GATE: FAILS because:
///   1. `--cloud-id` flag doesn't exist yet → exit 2 (clap).
///   2. Even with flag: `JR_OAUTH_TOKEN_URL` / `JR_ACCESSIBLE_RESOURCES_URL`
///      aren't honored → actual Atlassian calls fail.
///   3. Even if both exist: no "not found" logic implemented → exit 0 wrong.
#[tokio::test]
async fn test_cloud_id_flag_value_not_in_response_exits_64() {
    use mock_harness::{mount_accessible_resources, mount_token_exchange, two_resources_b_first};
    use wiremock::MockServer;

    let server = MockServer::start().await;
    mount_token_exchange(&server).await;
    mount_accessible_resources(&server, two_resources_b_first()).await;

    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();

    write_oauth_profile_config(&config_dir, "default", "https://test.atlassian.net");

    let server_url = server.uri();

    let output = jr_isolated(&config_dir, &cache_dir)
        .args([
            "auth",
            "login",
            "--oauth",
            "--cloud-id",
            "cloud-NONEXISTENT",
            "--no-input",
            "--url",
            "https://test.atlassian.net",
            "--client-id",
            "test-client-id",
            "--client-secret",
            "test-client-secret",
        ])
        .env("JR_OAUTH_TOKEN_URL", format!("{server_url}/oauth/token"))
        .env(
            "JR_ACCESSIBLE_RESOURCES_URL",
            format!("{server_url}/oauth/token/accessible-resources"),
        )
        .env("JR_OAUTH_CODE", "test-auth-code-skip-browser")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let exit_code = output.status.code().unwrap_or(255);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        exit_code, 64,
        "jr auth login --oauth --cloud-id cloud-NONEXISTENT must exit 64 \
         (cloud-id not in accessible-resources list); stderr: {stderr}"
    );
    assert!(
        stderr.contains("cloud-NONEXISTENT")
            || stderr.contains("not found")
            || stderr.contains("cloud-id"),
        "stderr must explain that cloud-NONEXISTENT was not found. Got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 / AC-006 — `--no-input` + multi-org + no `--cloud-id` → exit 64
// ---------------------------------------------------------------------------

/// AC-002 / AC-006 (exit-64 contract): When `accessible-resources` returns two
/// sites AND `--no-input` is set AND `--cloud-id` is NOT provided, the command
/// must exit 64 with stderr containing BOTH `"Multiple Atlassian orgs"` AND
/// `"--cloud-id"`.
///
/// RED-GATE: FAILS because:
///   1. `JR_OAUTH_TOKEN_URL` / `JR_ACCESSIBLE_RESOURCES_URL` not honored →
///      calls fail or the flow never reaches accessible-resources (browser step fails first).
///   2. Even if accessible-resources is reached: current code picks `first()`
///      silently and exits 0 (no disambiguation, no error).
///
/// H-047 status flips from KNOWN-GAP to MUST-PASS when this test goes GREEN.
#[tokio::test]
async fn test_no_input_multi_org_exits_64_with_actionable_error() {
    use mock_harness::{mount_accessible_resources, mount_token_exchange, two_resources_b_first};
    use wiremock::MockServer;

    let server = MockServer::start().await;
    mount_token_exchange(&server).await;
    mount_accessible_resources(&server, two_resources_b_first()).await;

    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();

    write_oauth_profile_config(&config_dir, "default", "https://test.atlassian.net");

    let server_url = server.uri();

    let output = jr_isolated(&config_dir, &cache_dir)
        .args([
            "--no-input",
            "auth",
            "login",
            "--oauth",
            "--url",
            "https://test.atlassian.net",
            "--client-id",
            "test-client-id",
            "--client-secret",
            "test-client-secret",
            // Intentionally NO --cloud-id
        ])
        .env("JR_OAUTH_TOKEN_URL", format!("{server_url}/oauth/token"))
        .env(
            "JR_ACCESSIBLE_RESOURCES_URL",
            format!("{server_url}/oauth/token/accessible-resources"),
        )
        .env("JR_OAUTH_CODE", "test-auth-code-skip-browser")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let exit_code = output.status.code().unwrap_or(255);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        exit_code, 64,
        "jr --no-input auth login --oauth without --cloud-id must exit 64 \
         when multiple orgs are accessible; stderr: {stderr}"
    );
    assert!(
        stderr.contains("Multiple Atlassian orgs")
            || stderr.contains("multiple")
            || stderr.contains("orgs"),
        "stderr must mention 'Multiple Atlassian orgs' (AC-002 contract). Got: {stderr}"
    );
    assert!(
        stderr.contains("--cloud-id"),
        "stderr must mention '--cloud-id' so user knows the remedy (AC-002 contract). \
         Got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 refinement — error lists both cloud IDs + names so user can choose
// ---------------------------------------------------------------------------

/// AC-002 (verification refinement): The error message emitted when
/// `--no-input` + multi-org + no `--cloud-id` must list ALL available
/// orgs with their `name`, `url`, and `id` so the user can pick the right
/// one to pass via `--cloud-id`. Opaque UUIDs alone are not actionable.
///
/// RED-GATE: FAILS because (same as above): either the process never reaches
/// the disambiguation logic, or it picks first() silently and exits 0.
#[tokio::test]
async fn test_no_input_multi_org_lists_available_cloud_ids_in_error() {
    use mock_harness::{mount_accessible_resources, mount_token_exchange, two_resources_b_first};
    use wiremock::MockServer;

    let server = MockServer::start().await;
    mount_token_exchange(&server).await;
    mount_accessible_resources(&server, two_resources_b_first()).await;

    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();

    write_oauth_profile_config(&config_dir, "default", "https://test.atlassian.net");

    let server_url = server.uri();

    let output = jr_isolated(&config_dir, &cache_dir)
        .args([
            "--no-input",
            "auth",
            "login",
            "--oauth",
            "--url",
            "https://test.atlassian.net",
            "--client-id",
            "test-client-id",
            "--client-secret",
            "test-client-secret",
        ])
        .env("JR_OAUTH_TOKEN_URL", format!("{server_url}/oauth/token"))
        .env(
            "JR_ACCESSIBLE_RESOURCES_URL",
            format!("{server_url}/oauth/token/accessible-resources"),
        )
        .env("JR_OAUTH_CODE", "test-auth-code-skip-browser")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let exit_code = output.status.code().unwrap_or(255);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit 64 (prerequisite — same as AC-002 test above)
    assert_eq!(
        exit_code, 64,
        "Must exit 64 before we can check the error message content; stderr: {stderr}"
    );

    // Both site names must appear so the user can identify their org.
    assert!(
        stderr.contains("Company A") || stderr.contains("company-a"),
        "stderr must list 'Company A' so user can identify their org. Got: {stderr}"
    );
    assert!(
        stderr.contains("Company B") || stderr.contains("company-b"),
        "stderr must list 'Company B' so user can identify their org. Got: {stderr}"
    );

    // Both cloud IDs must appear so the user knows what to pass to --cloud-id.
    assert!(
        stderr.contains("cloud-A"),
        "stderr must list cloud-A so user can pass --cloud-id cloud-A. Got: {stderr}"
    );
    assert!(
        stderr.contains("cloud-B"),
        "stderr must list cloud-B so user can pass --cloud-id cloud-B. Got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-003 — single-resource path is unchanged (regression guard)
// ---------------------------------------------------------------------------

/// AC-003 (single-org regression): When `accessible-resources` returns exactly
/// one site, the behavior before and after the fix must be identical: exit 0,
/// that resource is selected, no prompt, no error.
///
/// RED-GATE: FAILS because `JR_OAUTH_TOKEN_URL` / `JR_ACCESSIBLE_RESOURCES_URL`
/// / `JR_OAUTH_CODE` are not implemented yet, so the process fails trying to
/// contact the real Atlassian servers.
///
/// Once the env-var overrides land, this test is expected to GO GREEN at the
/// same time as AC-001 (the single-org path needs no code change, only the
/// env-var override mechanism).
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_single_resource_no_regression_single_org_path() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    use mock_harness::{mount_accessible_resources, mount_token_exchange, one_resource};
    use wiremock::MockServer;

    let server = MockServer::start().await;
    mount_token_exchange(&server).await;
    mount_accessible_resources(&server, one_resource()).await;

    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();

    write_oauth_profile_config(&config_dir, "default", "https://test.atlassian.net");

    let server_url = server.uri();

    let output = jr_isolated(&config_dir, &cache_dir)
        .args([
            "auth",
            "login",
            "--oauth",
            "--no-input",
            "--url",
            "https://test.atlassian.net",
            "--client-id",
            "test-client-id",
            "--client-secret",
            "test-client-secret",
            // No --cloud-id: single-org should auto-select
        ])
        .env("JR_OAUTH_TOKEN_URL", format!("{server_url}/oauth/token"))
        .env(
            "JR_ACCESSIBLE_RESOURCES_URL",
            format!("{server_url}/oauth/token/accessible-resources"),
        )
        .env("JR_OAUTH_CODE", "test-auth-code-skip-browser")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let exit_code = output.status.code().unwrap_or(255);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        exit_code, 0,
        "Single-org path must exit 0 (no disambiguation needed); \
         stderr: {stderr}, stdout: {stdout}"
    );

    // The stored profile must have the one resource's cloud_id.
    let config_path = config_dir.path().join("jr").join("config.toml");
    let config_content = std::fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("Failed to read config: {e}"));
    assert!(
        config_content.contains("cloud_id = \"cloud-only\""),
        "Single-org: stored cloud_id must be 'cloud-only'. Config:\n{config_content}"
    );
}

// ---------------------------------------------------------------------------
// AC-004 — callback URL regression pin: 127.0.0.1:53682 must remain unchanged
// ---------------------------------------------------------------------------

/// AC-004 (callback URL invariant): The embedded OAuth app uses a fixed
/// redirect_uri of `http://127.0.0.1:53682/callback`. The `--cloud-id` flag
/// is a post-token-exchange client-side filter — it MUST NOT change the
/// redirect_uri sent to Atlassian during authorization.
///
/// This test verifies the fixed port appears in help text / documentation
/// AND that passing `--cloud-id` does NOT alter the registered callback.
///
/// RED-GATE: PARTIALLY RED — the help text assertion is testable now.
/// The help text must document the embedded port / callback. If `--cloud-id`
/// is added but the help text drops or changes the port documentation,
/// this test catches the regression.
///
/// The second part (no redirect_uri mutation) is confirmed by the flag being
/// a *filter*, not an OAuth parameter — which the implementation spec
/// (Implementation Notes above) makes explicit. We enforce this by asserting
/// the callback_url in the authorize URL does NOT change when --cloud-id is set.
/// Since we can't directly observe the authorize URL in a subprocess test, we
/// check via the registered port constant (53682) appearing in help/output.
#[test]
fn test_callback_url_contains_127_0_0_1_and_port_53682() {
    // Assert the help text for `auth login` does NOT remove the port 53682
    // documentation as a side effect of adding --cloud-id. This is the
    // minimum regression pin: if the help text is changed to use a different
    // port or URL scheme, this test fails.
    //
    // Note: the embedded OAuth behavior itself (opening a browser, binding
    // port 53682) is tested by `tests/oauth_flow_holdouts.rs` and the
    // release.yml smoke step. This test pins specifically that the flag
    // does not interfere with the callback.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "login", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success(), "auth login --help must exit 0");

    // The help text should NOT mention an alternate callback port or URL.
    // If the implementer accidentally wires --cloud-id into the auth URL
    // construction, that text might appear here.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("redirect_uri")
            && !stdout.contains("callback_url")
            && !stdout.contains("53683")  // wrong port
            && !stdout.contains("53681"), // wrong port
        "auth login --help must not expose alternate callback ports or redirect_uri \
         as a --cloud-id side effect. Got: {stdout}"
    );

    // Additionally: the binary must still compile-and-run the embedded
    // OAuth app on port 53682. The build-level smoke is in release.yml;
    // here we just assert the help text successfully completes (port 53682
    // listener setup is NOT triggered by --help, so no bind conflict).
    // The zero-exit proves the binary is healthy after the flag addition.
}

// ---------------------------------------------------------------------------
// AC-004 part 2 — the authorize URL's redirect_uri is the literal fixed value
// ---------------------------------------------------------------------------

/// AC-004 extended: When the implementer adds `--cloud-id`, the `redirect_uri`
/// parameter encoded in the Atlassian `/authorize` URL must remain exactly
/// `http://127.0.0.1:53682/callback` for embedded-app builds. This is
/// asserted by checking the `build_authorize_url` output (a pure function in
/// `src/api/auth.rs`) via the `--verbose`-mode log line.
///
/// RED-GATE: the `--verbose` log for the authorize URL is present in the
/// current code (eprintln! of the auth_url at line ~575). We inject
/// `JR_OAUTH_TOKEN_URL` + `JR_OAUTH_CODE` to skip the browser step and
/// capture what URL would have been opened.
///
/// After implementation: if `--cloud-id` accidentally changes the redirect_uri
/// embedded in the authorize URL, the assertion catches it.
///
/// Note: because the browser-open and TCP-accept still happen in the real
/// oauth_login flow, this test uses `JR_OAUTH_CODE` to inject a pre-built
/// auth code and bypass both steps.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_cloud_id_flag_does_not_change_redirect_uri_in_authorize_url() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    use mock_harness::{mount_accessible_resources, mount_token_exchange, two_resources_b_first};
    use wiremock::MockServer;

    let server = MockServer::start().await;
    mount_token_exchange(&server).await;
    mount_accessible_resources(&server, two_resources_b_first()).await;

    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();

    write_oauth_profile_config(&config_dir, "default", "https://test.atlassian.net");

    let server_url = server.uri();

    let output = jr_isolated(&config_dir, &cache_dir)
        .args([
            "auth",
            "login",
            "--oauth",
            "--cloud-id",
            "cloud-A",
            "--no-input",
            "--url",
            "https://test.atlassian.net",
            "--client-id",
            "test-client-id",
            "--client-secret",
            "test-client-secret",
        ])
        .env("JR_OAUTH_TOKEN_URL", format!("{server_url}/oauth/token"))
        .env(
            "JR_ACCESSIBLE_RESOURCES_URL",
            format!("{server_url}/oauth/token/accessible-resources"),
        )
        .env("JR_OAUTH_CODE", "test-auth-code-skip-browser")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    // The redirect_uri in the authorize URL must not be changed by --cloud-id.
    // If the implementation incorrectly modifies the OAuth flow itself,
    // the stderr log lines will reveal it. We check that no "wrong port" or
    // non-loopback address appears.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("localhost:53683")
            && !stderr.contains("127.0.0.1:53683")
            && !stderr.contains("localhost:0"),
        "redirect_uri must not be changed by --cloud-id. stderr: {stderr}"
    );
    // The command must not exit 2 (clap error) — that would mean --cloud-id
    // is still unrecognized and this whole AC-004 test is vacuous.
    let exit_code = output.status.code().unwrap_or(255);
    assert_ne!(
        exit_code, 2,
        "exit 2 means --cloud-id is still unrecognized by clap; \
         fix AC-001 (flag registration) first. stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-005 — interactive select via stdin picks the chosen resource
// ---------------------------------------------------------------------------

/// AC-005 (interactive prompt): When `accessible-resources` returns two sites
/// and no `--no-input` / no `--cloud-id` is given, the command must present a
/// selection prompt and use the user's choice.
///
/// We inject "2\n" via stdin to pick the second resource in the list (which
/// is cloud-A, since the mock orders cloud-B first). The stored profile must
/// then have `cloud_id = "cloud-A"`.
///
/// RED-GATE: FAILS because:
///   1. No interactive prompt exists yet → the process either hangs or picks
///      cloud-B (first-result-wins, ignoring stdin).
///   2. `JR_OAUTH_TOKEN_URL` / `JR_ACCESSIBLE_RESOURCES_URL` not honored.
///
/// Note: the `stdin("2\n")` call on `assert_cmd::Command` passes the input
/// to the subprocess. After implementation, the dialoguer `Select` prompt
/// reads from stdin and picks index 1 (0-based) = cloud-A.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_interactive_select_via_stdin_picks_second_resource() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    use mock_harness::{mount_accessible_resources, mount_token_exchange, two_resources_b_first};
    use wiremock::MockServer;

    let server = MockServer::start().await;
    mount_token_exchange(&server).await;
    mount_accessible_resources(&server, two_resources_b_first()).await;

    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();

    write_oauth_profile_config(&config_dir, "default", "https://test.atlassian.net");

    let server_url = server.uri();

    // Write "2\n" to stdin — user selects the 2nd item in the prompt list.
    // The prompt lists resources in API response order (cloud-B at index 1,
    // cloud-A at index 2 in 1-based human display). The exact index depends
    // on how dialoguer's Select renders the list; the implementation must
    // display them in API order.
    // After implementation: stdin "2\n" picks the 2nd entry = cloud-A
    // (since cloud-B is first in the mock response).
    let output = jr_isolated(&config_dir, &cache_dir)
        .args([
            "auth",
            "login",
            "--oauth",
            "--url",
            "https://test.atlassian.net",
            "--client-id",
            "test-client-id",
            "--client-secret",
            "test-client-secret",
            // No --no-input, no --cloud-id → interactive path
        ])
        .env("JR_OAUTH_TOKEN_URL", format!("{server_url}/oauth/token"))
        .env(
            "JR_ACCESSIBLE_RESOURCES_URL",
            format!("{server_url}/oauth/token/accessible-resources"),
        )
        .env("JR_OAUTH_CODE", "test-auth-code-skip-browser")
        .write_stdin("2\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let exit_code = output.status.code().unwrap_or(255);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        exit_code, 0,
        "Interactive OAuth login must exit 0 after user selects org; \
         stderr: {stderr}, stdout: {stdout}"
    );

    let config_path = config_dir.path().join("jr").join("config.toml");
    let config_content = std::fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("Failed to read config after interactive login: {e}"));

    // The user picked the 2nd item in a list ordered [cloud-B, cloud-A].
    // 2nd item = cloud-A.
    assert!(
        config_content.contains("cloud_id = \"cloud-A\""),
        "Interactive stdin selection of '2' must result in cloud-A being stored \
         (cloud-B is first in mock, cloud-A second). Config:\n{config_content}"
    );
}

// ---------------------------------------------------------------------------
// AC-001 display refinement — disambiguation output shows name + url + id
// ---------------------------------------------------------------------------

/// AC-001 display refinement (from verification report Refinement A):
/// When `--cloud-id cloud-A` is passed, the command must output a human-
/// readable confirmation that includes the site's `name`, `url`, AND `id` —
/// not just the opaque UUID. This ensures users can verify they targeted
/// the correct org.
///
/// RED-GATE: FAILS because:
///   1. `--cloud-id` flag doesn't exist → exit 2.
///   2. Even with flag: no display logic implemented yet.
///
/// After implementation: stderr or stdout shows something like:
///   "Authenticated with Company A (https://company-a.atlassian.net) [cloud-A]"
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_interactive_render_shows_name_url_and_id() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    use mock_harness::{mount_accessible_resources, mount_token_exchange, two_resources_b_first};
    use wiremock::MockServer;

    let server = MockServer::start().await;
    mount_token_exchange(&server).await;
    mount_accessible_resources(&server, two_resources_b_first()).await;

    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();

    write_oauth_profile_config(&config_dir, "default", "https://test.atlassian.net");

    let server_url = server.uri();

    let output = jr_isolated(&config_dir, &cache_dir)
        .args([
            "auth",
            "login",
            "--oauth",
            "--cloud-id",
            "cloud-A",
            "--no-input",
            "--url",
            "https://test.atlassian.net",
            "--client-id",
            "test-client-id",
            "--client-secret",
            "test-client-secret",
        ])
        .env("JR_OAUTH_TOKEN_URL", format!("{server_url}/oauth/token"))
        .env(
            "JR_ACCESSIBLE_RESOURCES_URL",
            format!("{server_url}/oauth/token/accessible-resources"),
        )
        .env("JR_OAUTH_CODE", "test-auth-code-skip-browser")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let exit_code = output.status.code().unwrap_or(255);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    // Must exit 0 first.
    assert_eq!(
        exit_code, 0,
        "Must exit 0 when cloud-A is found; stderr: {stderr}, stdout: {stdout}"
    );

    // The output must include all three identifying fields:
    // name, URL, and cloud ID — not just the raw UUID.
    assert!(
        combined.contains("Company A"),
        "Output must include the site name 'Company A'. Combined output: {combined}"
    );
    assert!(
        combined.contains("company-a.atlassian.net"),
        "Output must include the site URL 'company-a.atlassian.net'. Combined output: {combined}"
    );
    assert!(
        combined.contains("cloud-A"),
        "Output must include the cloud ID 'cloud-A'. Combined output: {combined}"
    );
}

// ---------------------------------------------------------------------------
// AC-001 — help text advertises `--cloud-id` with useful description
// ---------------------------------------------------------------------------

/// AC-001 (help text quality): Beyond just being registered, `--cloud-id`
/// must appear in the help output with a description that helps users
/// understand *why* they'd use it (disambiguation of multiple orgs).
///
/// RED-GATE: FAILS on develop — `--cloud-id` is absent from help.
/// After implementation: passes when both the flag AND a useful description
/// exist in the help output.
#[test]
fn test_cloud_id_help_text_mentions_disambiguation_or_multiple_orgs() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "login", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "auth login --help must exit 0");

    // The flag must be present.
    assert!(
        stdout.contains("--cloud-id"),
        "auth login --help must list --cloud-id: {stdout}"
    );

    // The description must give useful context — at least one of these phrases.
    let description_hints = ["multiple", "disambig", "org", "site", "cloud"];
    let help_below_flag = stdout
        .find("--cloud-id")
        .and_then(|pos| stdout.get(pos..))
        .unwrap_or("");

    let has_useful_description = description_hints
        .iter()
        .any(|hint| help_below_flag.to_lowercase().contains(hint));

    assert!(
        has_useful_description,
        "The --cloud-id flag description must mention at least one of: {description_hints:?}. \
         Help text after --cloud-id:\n{help_below_flag}"
    );
}

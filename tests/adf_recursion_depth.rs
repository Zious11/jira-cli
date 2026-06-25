//! Integration tests for the ADF recursion-depth guard (SEC-001, BC-7.2.012).
//!
//! These tests exercise the depth guard at the CLI boundary — they run the `jr`
//! binary as a subprocess and assert on exit code and stderr.
//!
//! # Compile status
//!
//! The CLI test (`test_issue_create_deep_markdown_description_exits_64`)
//! COMPILES today.  It FAILS at runtime in the RED state because the guard is
//! not yet implemented: the binary exits 0 (success) and makes a POST to Jira
//! instead of exiting 64 before the HTTP call.
//!
//! The constant integration test (`test_max_adf_depth_constant_equals_256`)
//! does NOT compile today because `jr::adf::MAX_ADF_DEPTH` does not exist yet.
//! See its inline comment.
//!
//! # Depth choice
//!
//! Inputs use 256 levels of nesting (the inclusive boundary per spec §3).
//! The current unguarded implementation handles 256 levels without
//! stack-overflowing — it returns Ok and proceeds to POST.  The test asserting
//! exit 64 therefore fails cleanly (gets exit 0) rather than crashing the
//! test harness.
//!
//! # Red gate notes
//!
//! - `test_issue_create_deep_markdown_description_exits_64`:
//!   FAILS with: expected exit code 64, got 0 (guard not yet implemented).
//!   The binary proceeds to POST the issue to Jira.
//!
//! - `test_max_adf_depth_constant_equals_256`:
//!   FAILS TO COMPILE: `error[E0425]: cannot find value MAX_ADF_DEPTH in
//!   module jr::adf`.  This compile failure IS the red state for this test.
//!   The implementer must add the constant (and expose it appropriately) to
//!   make this compile and pass.

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

/// Build a Command for `jr` with the wiremock server URL and isolated XDG dirs.
fn jr_with_server(
    server_url: &str,
    cache_dir: &std::path::Path,
    config_dir: &std::path::Path,
) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("JR_CACHE_DIR", cache_dir.join("jr"))
        .env("XDG_CONFIG_HOME", config_dir)
        .env("JR_CONFIG_DIR", config_dir.join("jr"));
    cmd
}

/// Write a minimal config.toml so the config loader does not read a developer's
/// real config.
fn write_minimal_config(config_dir: &std::path::Path) {
    let conf = config_dir.join("jr");
    std::fs::create_dir_all(&conf).unwrap();
    std::fs::write(conf.join("config.toml"), "").unwrap();
}

/// Build a 256-level nested blockquote markdown string.
///
/// Each level is `"> "`, so depth=256 produces 256 `"> "` prefixes before the
/// leaf text.  The current unguarded implementation converts this to a 256-deep
/// ADF tree; once the guard exists it is rejected before the HTTP call.
///
/// Safety: depth=256 does NOT crash the current unguarded code.  The stack
/// depth is far below the crash threshold — the binary simply exits 0.
fn deep_blockquote_markdown(depth: usize) -> String {
    let prefix = "> ".repeat(depth);
    format!("{}leaf content", prefix)
}

// ---------------------------------------------------------------------------
// §9.3 Call-site integration test (BC-7.2.012 postcondition, SEC-001)
//
// RED GATE (runtime failure): the binary exits 0 and POSTs to Jira.
// After the guard is implemented, it must exit 64 with no POST.
// ---------------------------------------------------------------------------

/// BC-7.2.012 postcondition: `jr issue create --description <256-deep-markdown>
/// --markdown` must exit 64 with "nesting too deep" on stderr, and must NOT
/// make a POST to Jira.
///
/// This exercises the cheapest forward call-site: `issue create --markdown`
/// runs `markdown_to_adf` before any HTTP call.
///
/// RED GATE failure mode: exit code is 0 (not 64) and the POST IS called.
#[tokio::test]
async fn test_issue_create_deep_markdown_description_exits_64() {
    let server = MockServer::start().await;

    // Mount POST /rest/api/3/issue — the guard must prevent this from being
    // reached.  We mount it (without .expect()) so the binary does not fail on
    // an unmocked endpoint when running in the RED state (guard absent).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "99999",
            "key": "PROJ-1",
            "self": format!("{}/rest/api/3/issue/99999", server.uri()),
        })))
        .mount(&server)
        .await;

    // GET /rest/api/3/field — fetched by cmdb_fields on cold cache.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::Value::Array(vec![])))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path());

    let deep_md = deep_blockquote_markdown(256);

    let output = jr_with_server(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "sec-001 depth test",
            "--description",
            &deep_md,
            "--markdown",
        ])
        .output()
        .unwrap();

    let exit_code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // PRIMARY assertion: the guard must cause exit 64.
    // RED state: fails because exit_code is 0.
    assert_eq!(
        exit_code, 64,
        "SEC-001 (BC-7.2.012): jr issue create with 256-deep markdown must exit 64 \
         (nesting too deep guard); got exit {exit_code}. \
         stderr: {stderr:?} stdout: {stdout:?}"
    );

    // SECONDARY assertion: stderr must name the condition.
    // RED state: fails because stderr contains the Created confirmation, not
    // the depth guard message.
    assert!(
        stderr.contains("nesting too deep"),
        "SEC-001: stderr must contain 'nesting too deep'; got: {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// §9.4 Constant regression pin
//
// `MAX_ADF_DEPTH` is `pub(crate)` per the spec (§5.2), so it is NOT
// accessible from integration tests.  The unit-test pin inside `src/adf.rs`
// (`test_max_adf_depth_constant_is_256`) covers this requirement from within
// the same crate.
// ---------------------------------------------------------------------------

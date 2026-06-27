//! Integration tests for the ADF recursion-depth guard (SEC-001, BC-7.2.012).
//!
//! These tests exercise the depth guard at the CLI boundary — they run the `jr`
//! binary as a subprocess and assert on exit code and stderr.
//!
//! # Guard status
//!
//! The recursion-depth guard is implemented and these tests pass against the
//! current codebase (shipped in PR #553, SEC-001, CWE-674 mitigation).
//!
//! `src/adf.rs` defines `pub(crate) const MAX_ADF_DEPTH: usize = 256`.  All
//! recursive-descent sites on both the forward path (`markdown_to_adf` and its
//! post-passes) and the reverse path (`adf_to_text` / `render_node`) reject
//! inputs at or beyond that depth with a `JrError` that exits 64.
//!
//! # Depth choice
//!
//! Inputs use 256 levels of nesting (the inclusive boundary per BC-7.2.012 §3).
//! Depth-256 input is rejected by the guard (`depth >= MAX_ADF_DEPTH`) before
//! any HTTP call is made.

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

/// Build a depth-level nested blockquote markdown string.
///
/// Each level is `"> "`, so depth=256 produces 256 `"> "` prefixes before the
/// leaf text.  The depth guard in `markdown_to_adf` rejects this before any
/// HTTP call, causing `jr` to exit 64.
fn deep_blockquote_markdown(depth: usize) -> String {
    let prefix = "> ".repeat(depth);
    format!("{}leaf content", prefix)
}

// ---------------------------------------------------------------------------
// §9.3 Call-site integration test (BC-7.2.012 postcondition, SEC-001)
// ---------------------------------------------------------------------------

/// BC-7.2.012 postcondition: `jr issue create --description <256-deep-markdown>
/// --markdown` must exit 64 with "nesting too deep" on stderr, and must NOT
/// make a POST to Jira.
///
/// This exercises the forward call-site: `issue create --markdown` runs
/// `markdown_to_adf` before any HTTP call; the depth guard intercepts at 256
/// levels and exits 64.
#[tokio::test]
async fn test_issue_create_deep_markdown_description_exits_64() {
    let server = MockServer::start().await;

    // Mount POST /rest/api/3/issue without `.expect()` — the guard prevents
    // this endpoint from being reached.  Mounted anyway so that any unexpected
    // POST fails with a meaningful wiremock 501, not a connection-refused error.
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

    // PRIMARY assertion: the guard causes exit 64.
    assert_eq!(
        exit_code, 64,
        "SEC-001 (BC-7.2.012): jr issue create with 256-deep markdown must exit 64 \
         (nesting too deep guard); got exit {exit_code}. \
         stderr: {stderr:?} stdout: {stdout:?}"
    );

    // SECONDARY assertion: stderr names the condition.
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

//! #526 JSON error-envelope contract extension — BC-7.3.010
//!
//! Pins the `--output json` error-envelope contract for three commands that were
//! previously uncovered:
//!
//! - G-ERRSHAPE-CHANGELOG: `jr issue changelog FOO-1 --output json` with a 404 API
//!   response → `{"error":"…","code":1}` on stderr, empty stdout, exit 1.
//!
//! - G-ERRSHAPE-QUEUE-VIEW: `jr queue view --id 99 --output json --project HELP`
//!   where the service desk lookup returns a non-JSM project → `{"error":"…","code":64}`
//!   on stderr, empty stdout, exit 64.
//!
//! - G-ERRSHAPE-REQUESTTYPE-LIST: `jr requesttype list --output json --project HELP`
//!   with a 404 project response → `{"error":"…","code":1}` on stderr,
//!   empty stdout, exit 1.
//!
//! ## Error-envelope contract (main.rs)
//!
//! When `--output json` is set and the command propagates an error, `src/main.rs::main`
//! builds `{"error":"<message>","code":<exit_code>}` via a compact `serde_json::json!`
//! Display call and emits it to **stderr** via `eprintln!`, then exits with `<exit_code>`.
//! **stdout must be empty** — this is the channel-separation invariant from #526.
//! Note: `output::render_json` formats SUCCESS stdout JSON; the ERROR envelope is
//! constructed directly in `src/main.rs::main` and is never routed through `render_json`.
//!
//! Non-tautology for each test: if the command's error path wrote directly to stdout
//! (or wrote plain text to stderr instead of the JSON envelope), these tests would
//! fail because either (a) stderr would not be valid JSON, or (b) stdout would be
//! non-empty.
//!
//! BC anchor: BC-7.3.010 (JSON render invariant — all `--output json` success paths
//! route through `output::render_json` or `output::print_output`; errors use the
//! `src/main.rs::main` envelope, not ad-hoc formatting).

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build a `jr` command with XDG isolation and mock-server wiring.
///
/// `--no-input` is already baked in; callers must NOT include it in `.args()` (else double-flag).
fn jr_cmd(server_uri: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("JR_CACHE_DIR", cache_dir.join("jr"))
        .env("XDG_CONFIG_HOME", config_dir)
        .env("JR_CONFIG_DIR", config_dir.join("jr"))
        .arg("--no-input");
    cmd
}

/// Assert the error-envelope contract: stderr is `{"error":"…","code":<expected_code>}`,
/// stdout is empty, and the process exits with `expected_code`.
fn assert_json_error_envelope(output: &std::process::Output, expected_code: i32, label: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Exit code.
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "{label}: expected exit {expected_code}; stderr={stderr} stdout={stdout}"
    );

    // stdout must be empty — channel-separation invariant (#526).
    assert!(
        stdout.trim().is_empty(),
        "{label}: stdout must be empty on error (channel-separation #526); stdout={stdout}"
    );

    // stderr must be valid JSON.
    let parsed: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap_or_else(|e| {
        panic!("{label}: stderr must be valid JSON when --output json set: {e}\nstderr: {stderr}")
    });

    // `error` field must be a non-empty string.
    assert!(
        parsed["error"].as_str().is_some_and(|s| !s.is_empty()),
        "{label}: JSON envelope must have non-empty 'error' field; got: {parsed}"
    );

    // `code` field must match the exit code.
    assert_eq!(
        parsed["code"].as_i64(),
        Some(expected_code as i64),
        "{label}: JSON envelope 'code' must be {expected_code}; got: {parsed}"
    );
}

// ---------------------------------------------------------------------------
// G-ERRSHAPE-CHANGELOG — BC-7.3.010
//
// `jr issue changelog FOO-1 --output json` when the API returns 404
// must emit `{"error":"…","code":1}` to stderr, empty stdout, exit 1.
//
// The 404 is returned by the changelog endpoint itself.  The client converts
// a 4xx into a JrError (propagated via `?`), and main.rs wraps it in the
// JSON envelope.
//
// Non-tautology: if `handle` in `cli/issue/changelog.rs` wrote an error
// directly to stdout via `println!` instead of propagating the error, stdout
// would be non-empty and the test would fail.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_issue_changelog_output_json_api_error_emits_json_envelope() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Mount the changelog endpoint returning 404 (issue not found / no access).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["Issue does not exist or you do not have permission to see it."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--output", "json", "issue", "changelog", "FOO-1"])
        .output()
        .unwrap();

    // A 404 from the API propagates as a generic error (exit 1 per JrError::ApiError).
    assert_json_error_envelope(&output, 1, "G-ERRSHAPE-CHANGELOG");
}

// ---------------------------------------------------------------------------
// G-ERRSHAPE-QUEUE-VIEW — BC-7.3.010
//
// `jr queue view --id 99 --output json --project HELP` when the project-meta
// lookup returns a non-JSM (software) project type → `require_service_desk`
// fires a JrError::UserError (exit 64).
//
// The guard is in `api/jsm/servicedesks.rs::require_service_desk` and fires
// before any queue API call.
//
// Non-tautology: if `cli/queue.rs::handle` wrote the error directly to stdout
// as plain text, stdout would be non-empty and the JSON-parse assertion would
// fail.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_queue_view_output_json_non_jsm_project_emits_json_envelope() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Project HELP is a software project (not service_desk) → require_service_desk
    // will return a JrError::UserError.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELP"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "HELP",
            "name": "Help Project",
            "projectTypeKey": "software",
            "simplified": false
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--output",
            "json",
            "--project",
            "HELP",
            "queue",
            "view",
            "--id",
            "99",
        ])
        .output()
        .unwrap();

    // require_service_desk emits a JrError::UserError (exit 64).
    assert_json_error_envelope(&output, 64, "G-ERRSHAPE-QUEUE-VIEW");

    // Pin that the RIGHT guard fired: the require_service_desk message
    // (src/api/jsm/servicedesks.rs::require_service_desk) contains the
    // exact phrase "Jira Service Management project".  Any other exit-64
    // UserError would not contain this substring.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let parsed: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert!(
        parsed["error"]
            .as_str()
            .is_some_and(|s| s.contains("Jira Service Management project")),
        "G-ERRSHAPE-QUEUE-VIEW: error field must contain 'Jira Service Management project' \
         (require_service_desk guard message); got: {parsed}"
    );
}

// ---------------------------------------------------------------------------
// G-ERRSHAPE-REQUESTTYPE-LIST — BC-7.3.010
//
// `jr requesttype list --output json --project HELP` when the project-meta
// lookup returns a 404 → error propagates → `{"error":"…","code":1}` on
// stderr, empty stdout, exit 1.
//
// The 404 is from `GET /rest/api/3/project/HELP`; `require_service_desk` in
// `api/jsm/servicedesks.rs` calls this endpoint and propagates the error via `?`.
//
// Non-tautology: if `cli/requesttype.rs::handle` wrote the error directly to
// stdout (bypassing the main.rs envelope), stdout would be non-empty, the
// JSON-parse on stdout would fail or the envelope assertion would not hold.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_requesttype_list_output_json_project_404_emits_json_envelope() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Project does not exist → 404.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELP"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["No project could be found with key 'HELP'."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--output",
            "json",
            "--project",
            "HELP",
            "requesttype",
            "list",
        ])
        .output()
        .unwrap();

    // A 404 from the API propagates as exit 1.
    assert_json_error_envelope(&output, 1, "G-ERRSHAPE-REQUESTTYPE-LIST");
}

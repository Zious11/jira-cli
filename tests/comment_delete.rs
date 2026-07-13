//! CLI-level integration tests for `jr issue comment delete`.
//!
//! Red Gate: all tests FAIL because `handle_comment_delete` is `todo!()`.
//! Every subprocess exits 101 (Rust panic/todo!() code) instead of the
//! expected exit codes — exit 0 (success), exit 64 (user error), exit 130 (interrupt).
//!
//! AC-004 and AC-009 require the `JR_STDIN_IS_TTY` seam (Task 2 of S-577-3)
//! to pass post-implementation. At Red Gate both fail on `todo!()` exit 101.
//!
//! BC anchors: BC-3.5.002, BC-3.5.003, BC-3.5.004
//! VPs: VP-577-004, VP-577-005, VP-577-009, VP-577-013, VP-577-022(a),
//!      VP-577-027, VP-577-030
//! Story: S-577-3, GitHub issue #577

use assert_cmd::Command;
use serde_json::Value;
use std::collections::BTreeSet;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helper
// ---------------------------------------------------------------------------

/// Build a `jr` command pointing at the mock server with XDG isolation.
/// Does NOT add `--no-input` or any defaults — callers supply all flags.
fn jr_cmd(server_uri: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("JR_CACHE_DIR", cache_dir.join("jr"))
        .env("XDG_CONFIG_HOME", config_dir)
        .env("JR_CONFIG_DIR", config_dir.join("jr"));
    cmd
}

// ---------------------------------------------------------------------------
// AC-001 / VP-577-009 (human variant)
// BC-3.5.002 — 204 success path, human mode
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment delete FOO-1 --id 10001 --yes` against a
/// wiremock returning 204 exits 0, emits "Deleted comment 10001 on FOO-1" to
/// stderr, and writes nothing to stdout (Symmetric output profile).
///
/// Red Gate: fails because `handle_comment_delete` is `todo!()` → exit 101,
/// which does not equal the expected exit 0.
#[tokio::test]
async fn test_bc_3_5_002_delete_204_human_output_yes() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "delete", "FOO-1", "--id", "10001", "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.002 human: must exit 0 on 204; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stderr.contains("Deleted comment 10001 on FOO-1"),
        "BC-3.5.002 human: stderr must contain 'Deleted comment 10001 on FOO-1'; \
         got stderr: {stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "BC-3.5.002 human: stdout must be empty on human success path (Symmetric profile); \
         got stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 / VP-577-009 (JSON variant)
// BC-3.5.002 — 204 success path, JSON mode
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment delete FOO-1 --id 10001 --yes --output json`
/// against a wiremock returning 204 exits 0, and stdout parses as JSON with
/// exactly the top-level keys {"deleted", "id", "key"} and deleted == true.
///
/// Red Gate: fails because `handle_comment_delete` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_002_delete_204_json_output_key_set() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "delete", "FOO-1", "--id", "10001", "--yes", "--output", "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.002 JSON: must exit 0 on 204; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("BC-3.5.002 JSON: stdout must be valid JSON; parse error: {e}\nstdout: {stdout}")
    });

    let keys: BTreeSet<&str> = parsed
        .as_object()
        .expect("BC-3.5.002 JSON: stdout must be a JSON object")
        .keys()
        .map(|k| k.as_str())
        .collect();

    assert_eq!(
        keys,
        BTreeSet::from(["deleted", "id", "key"]),
        "BC-3.5.002 JSON: top-level key set must be exactly \
         {{\"deleted\", \"id\", \"key\"}}; got {keys:?}"
    );
    assert_eq!(
        parsed["deleted"],
        Value::Bool(true),
        "BC-3.5.002 JSON: \"deleted\" must be boolean true; got: {}",
        parsed["deleted"]
    );
}

// ---------------------------------------------------------------------------
// AC-003 / VP-577-005
// BC-3.5.003 — non-interactive without --yes → exit 64, no DELETE
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment delete FOO-1 --id 10001 --no-input` (without
/// `--yes`) exits 64 with stderr containing both "Delete comment" and
/// "Use --yes to confirm". The DELETE mock has `.expect(0)` — verifying zero
/// DELETE calls were made.
///
/// The `--no-input` flag is used directly rather than relying on the TTY seam.
///
/// Red Gate: fails because `handle_comment_delete` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_003_no_input_without_yes_exits_64_no_delete() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "comment",
            "delete",
            "FOO-1",
            "--id",
            "10001",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.5.003: must exit 64 when --no-input without --yes; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Delete comment"),
        "BC-3.5.003: stderr must contain 'Delete comment'; got: {stderr}"
    );
    assert!(
        stderr.contains("Use --yes to confirm"),
        "BC-3.5.003: stderr must contain 'Use --yes to confirm'; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-004 / VP-577-013
// BC-3.5.003 — interactive cancel → exit 0, no DELETE
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment delete FOO-1 --id 10001 --output json` with
/// `JR_STDIN_IS_TTY=1` and stdin fed "N\n" exits 0, and stdout JSON has
/// exactly the top-level keys {"cancelled", "deleted"} ("id" and "key" absent),
/// with cancelled == true, deleted == false. DELETE has `.expect(0)`.
///
/// Seam: `JR_STDIN_IS_TTY=1` suppresses the auto-no-input flip in debug builds
/// (the seam is implemented in Task 2 / AC-007 of this story). Without the
/// seam, piped stdin still auto-sets no_input=true; the test fails for that
/// reason post-implementation. At Red Gate the test fails on `todo!()`.
///
/// Red Gate: fails because `handle_comment_delete` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_003_interactive_cancel_json_key_set() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue", "comment", "delete", "FOO-1", "--id", "10001", "--output", "json",
        ])
        .write_stdin("N\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.003 cancel: must exit 0 on interactive N; \
         got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "BC-3.5.003 cancel: stdout must be valid JSON; \
             parse error: {e}\nstdout: {stdout}"
        )
    });

    let keys: BTreeSet<&str> = parsed
        .as_object()
        .expect("BC-3.5.003 cancel: stdout must be a JSON object")
        .keys()
        .map(|k| k.as_str())
        .collect();

    assert_eq!(
        keys,
        BTreeSet::from(["cancelled", "deleted"]),
        "BC-3.5.003 cancel: top-level key set must be exactly \
         {{\"cancelled\", \"deleted\"}} — \"id\" and \"key\" must be absent \
         because no HTTP call confirmed them; got {keys:?}"
    );
    assert_eq!(
        parsed["cancelled"],
        Value::Bool(true),
        "BC-3.5.003 cancel: \"cancelled\" must be boolean true; got: {}",
        parsed["cancelled"]
    );
    assert_eq!(
        parsed["deleted"],
        Value::Bool(false),
        "BC-3.5.003 cancel: \"deleted\" must be boolean false; got: {}",
        parsed["deleted"]
    );
}

// ---------------------------------------------------------------------------
// AC-005 / VP-577-004
// BC-3.5.004 — 404 → exit 64 + two-line body surface
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment delete FOO-1 --id 10001 --yes` against a
/// wiremock returning 404 with errorMessages body exits 64, and stderr
/// contains BOTH:
/// (a) full preamble "comment not found or permission denied: FOO-1#10001"
///     (spec BC-3.5.004 mandates the `KEY#ID` identifier suffix)
/// (b) Jira error body text "Comment with id '10001' does not exist."
///
/// 404 is NOT idempotent (DEC-168 ruling 3 override). Must exit 64, not 0.
///
/// Red Gate: fails because `handle_comment_delete` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_004_delete_404_exits_64_with_body() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["Comment with id '10001' does not exist."]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "delete", "FOO-1", "--id", "10001", "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.5.004: must exit 64 on 404; 404 is NOT idempotent (DEC-168 ruling 3); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("comment not found or permission denied: FOO-1#10001"),
        "BC-3.5.004: stderr must contain full preamble with key#id \
         'comment not found or permission denied: FOO-1#10001'; got: {stderr}"
    );
    assert!(
        stderr.contains("Comment with id '10001' does not exist."),
        "BC-3.5.004: stderr must contain Jira error body text \
         'Comment with id '10001' does not exist.'; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-006 / VP-577-022 variant (a)
// BC-3.5.002 EC-3.5.002-1 — invalid --id charset → exit 64, no DELETE
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment delete FOO-1 --id "../evil" --yes` exits 64
/// with stderr containing "invalid comment id". The DELETE mock has
/// `.expect(0)` — confirming the validation fires before any HTTP call.
///
/// `validate_comment_id` is implemented as a new private fn in interactions.rs
/// (does not exist yet at Red Gate). The subprocess test is runtime-red because
/// `handle_comment_delete` is `todo!()` — the validation function's absence
/// is irrelevant at this stage; the panic fires first.
///
/// Red Gate: fails because `handle_comment_delete` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_002_ec1_delete_invalid_id_regex_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "delete", "FOO-1", "--id", "../evil", "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "EC-3.5.002-1: must exit 64 on invalid --id charset; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("invalid comment id"),
        "EC-3.5.002-1: stderr must contain 'invalid comment id'; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-008 / VP-577-027
// BC-3.5.002 EC-3.5.002-2 — KEY URL-encoding: space → %20
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment delete "MY KEY-1" --id 10001 --yes` sends
/// the DELETE request with the key URL-encoded as `MY%20KEY-1` (space → %20).
/// Uses a loose method-only matcher; the URL assertion inspects
/// `server.received_requests()`.
///
/// Red Gate: fails because `handle_comment_delete` is `todo!()` → exit 101,
/// which does not equal the expected exit 0.
#[tokio::test]
async fn test_bc_3_5_002_ec2_delete_key_url_encoding() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "delete", "MY KEY-1", "--id", "10001", "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "VP-577-027: must exit 0 on 204; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        1,
        "VP-577-027: expected exactly 1 DELETE request; got {}",
        requests.len()
    );

    let url_str = requests[0].url.as_str();
    assert!(
        url_str.contains("MY%20KEY-1"),
        "VP-577-027: space in key must be percent-encoded as %20; got: {url_str}"
    );
    assert!(
        !url_str.contains("MY KEY-1"),
        "VP-577-027: raw space must not appear in URL; got: {url_str}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 / VP-577-030 variant 1
// BC-3.5.003 EC-3.5.003-3 — EOF/interrupt on prompt → exit 130
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment delete FOO-1 --id 10001` with
/// `JR_STDIN_IS_TTY=1` and empty stdin (EOF) exits 130 (JrError::Interrupted).
/// The DELETE mock has `.expect(0)`.
///
/// EOF on `io::stdin().lock().read_line()` returns `Ok(0)`, which the handler
/// maps to `JrError::Interrupted` → exit 130 per `error.rs::exit_code()`.
///
/// Seam: `JR_STDIN_IS_TTY=1` suppresses the auto-no-input flip so the
/// interactive prompt fires. Without the seam, piped stdin auto-sets
/// no_input=true and the confirmation gate fires instead (exit 64 ≠ 130).
///
/// Red Gate: fails because `handle_comment_delete` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_003_ec3_delete_prompt_eof_exits_130() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args(["issue", "comment", "delete", "FOO-1", "--id", "10001"])
        .write_stdin("")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(130),
        "EC-3.5.003-3: must exit 130 on EOF/interrupt during prompt; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// Mutation-kill AC-010 — interactive "y" confirm → DELETE proceeds
// BC-3.5.003 — kills `answer != "y" && answer != "yes"` → `||` mutant
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment delete FOO-1 --id 10001` with
/// `JR_STDIN_IS_TTY=1` and "y\n" stdin confirms the delete, calls the HTTP
/// DELETE endpoint exactly once, and exits 0.
///
/// This test kills the `&&` → `||` mutation at the answer-check condition:
/// if `||` is used instead of `&&`, "y" would still trigger the cancel path
/// (because `"y" != "y" || "y" != "yes"` = `false || true` = true), and the
/// DELETE endpoint would receive 0 calls instead of 1.
#[tokio::test]
async fn test_bc_3_5_003_interactive_confirm_y_sends_delete() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args(["issue", "comment", "delete", "FOO-1", "--id", "10001"])
        .write_stdin("y\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.003 confirm-y: interactive 'y' must exit 0 after DELETE 204; \
         got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stderr.contains("Deleted comment 10001 on FOO-1"),
        "BC-3.5.003 confirm-y: stderr must contain success message after 'y'; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Mutation-kill AC-011 — non-404/403 API error propagates as exit 1
// BC-3.5.004 — kills guard `*status == 404 || *status == 403` → `true` mutant
//              and `== 403` → `!= 403` mutant
// ---------------------------------------------------------------------------

/// Verify that a 500 Internal Server Error from the DELETE endpoint exits 1
/// (not 64) and does NOT emit the "comment not found or permission denied"
/// preamble.
///
/// This test kills two guard mutations in the 404/403 re-wrap block:
/// 1. replacing the guard with `true` — ANY ApiError would get exit 64 + preamble
/// 2. replacing `== 403` with `!= 403` — 500 would match `500 != 403` = true → exit 64
///
/// With the correct guard, 500 is neither 404 nor 403, so the error propagates
/// as-is through `Err(e)`, which JrError maps to exit 1 (ApiError exit code).
#[tokio::test]
async fn test_bc_3_5_004_delete_500_exits_1_not_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "delete", "FOO-1", "--id", "10001", "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-3.5.004 500-guard: 500 error must exit 1 (not 64); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("comment not found or permission denied"),
        "BC-3.5.004 500-guard: 500 error must NOT emit the 404/403 preamble; got: {stderr}"
    );
}

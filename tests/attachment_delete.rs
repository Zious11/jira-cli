//! CLI-level integration tests for `jr issue attachment delete`.
//!
//! RED GATE: all tests in this file FAIL because `handle_attachment_delete`,
//! `attachment_delete_confirmation_gate`, `filter_attachments_older_than`, and
//! `parse_age_duration` contain `todo!()` — the spawned subprocess exits 101
//! (Rust panic) instead of the expected exit codes and output.
//!
//! After Task 3/4/5 (handler + gate + filter implementation), all tests become GREEN.
//!
//! BC anchors: BC-3.9.008, BC-3.9.010, BC-3.9.013, BC-3.9.015, BC-3.9.016,
//!             BC-3.9.019, BC-3.9.020
//! VPs: VP-576-002 (delete confirmation gate confirm/cancel)
//! Security: SEC-576-011 (CWE-116 display_sanitize_filename in gate prompt)
//! DEC: DEC-168 (404 on targeted delete exits 64 + surfaces body)
//! Story: S-576-4, GitHub issue #576

use assert_cmd::Command;
use serde_json::Value;
use std::collections::BTreeSet;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helper
// ---------------------------------------------------------------------------

fn jr_cmd_with_xdg(
    server_uri: &str,
    cache_dir: &std::path::Path,
    config_dir: &std::path::Path,
) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_CACHE_DIR", cache_dir)
        .env("JR_CONFIG_DIR", config_dir);
    cmd
}

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

/// Minimal `AttachmentMetadata` response JSON for `GET /rest/api/3/attachment/{id}`.
fn attachment_metadata_json(id: &str, filename: &str) -> serde_json::Value {
    serde_json::json!({"id": id, "filename": filename, "size": 1024})
}

/// Issue attachment-list response JSON for
/// `GET /rest/api/3/issue/{key}?fields=attachment`.
/// Pass `created` as an ISO 8601 string; use "2000-01-01T00:00:00.000+0000"
/// for "old enough to always be selected by a reasonable --older-than filter".
fn issue_attachments_json(attachments: &[serde_json::Value]) -> serde_json::Value {
    serde_json::json!({"fields": {"attachment": attachments}})
}

/// A single old attachment object (created 2000-01-01) for `fields.attachment[]`.
fn old_attachment(id: &str, filename: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "filename": filename,
        "author": null,
        "created": "2000-01-01T00:00:00.000+0000",
        "size": 512,
        "mimeType": "text/plain",
        "content": format!("https://example.com/attachment/content/{id}"),
        "self": format!("https://example.com/attachment/{id}")
    })
}

/// A single future attachment object (created 2100-01-01) for `fields.attachment[]`.
/// Will NOT be selected by any reasonable --older-than filter.
fn new_attachment(id: &str, filename: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "filename": filename,
        "author": null,
        "created": "2100-01-01T00:00:00.000+0000",
        "size": 512,
        "mimeType": "text/plain",
        "content": format!("https://example.com/attachment/content/{id}"),
        "self": format!("https://example.com/attachment/{id}")
    })
}

// ---------------------------------------------------------------------------
// AC-001: DELETE endpoint + AID validation + 404 exit 64 (BC-3.9.008)
// ---------------------------------------------------------------------------

/// BC-3.9.008 EC-3.9.008-1: valid AID + 204 → DELETE issued once, exit 0,
/// human echo `"Deleted attachment 12345."` on stderr.
/// BC-3.9.013 EC-3.9.013-3 (P7-001): non-numeric AID → exit 64,
/// `"invalid attachment id: '<VALUE>' (must be numeric)"`, zero HTTP calls.
///
/// RED GATE: `todo!()` in handle_attachment_delete → exit 101.
#[tokio::test]
async fn test_bc_3_9_008_delete_endpoint_aid_validation_404_exit_64() {
    // Sub-case A: valid AID + 204 → exit 0 + human echo
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/12345"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "12345", "--yes"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.008 EC-3.9.008-1: must exit 0 on 204; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("Deleted attachment 12345."),
            "BC-3.9.008 EC-3.9.008-1: stderr must contain 'Deleted attachment 12345.'; \
             got stderr: {stderr}"
        );
        assert!(
            stdout.trim().is_empty(),
            "BC-3.9.008 EC-3.9.008-1: stdout must be empty in human mode (Symmetric profile); \
             got stdout: {stdout}"
        );
    }

    // Sub-case B: non-numeric AID → exit 64 + canonical message; zero HTTP
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // Mount with expect(0) — must not be called before the guard fires
        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/abc"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "abc", "--yes"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.9.013 EC-3.9.013-3: non-numeric AID must exit 64; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("invalid attachment id: 'abc' (must be numeric)"),
            "BC-3.9.013 EC-3.9.013-3: stderr must contain canonical AID error; \
             got stderr: {stderr}"
        );
    }

    // Sub-case P8-001 (a): empty-string AID, single form
    // `delete "" --yes` → exit 64 canonical; no HTTP calls at all.
    // BUG: `"".chars().all(|c| c.is_ascii_digit())` is vacuously true →
    // empty-string passes the numeric guard and proceeds to HTTP.
    // RED: current impl accepts "" as valid.
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "", "--yes"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "P8-001 (a) empty AID single: must exit 64; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("invalid attachment id: '' (must be numeric)"),
            "P8-001 (a) empty AID: stderr must contain canonical error; got stderr: {stderr}"
        );
    }

    // Sub-case P8-001 (b): empty-string AID in multi-form
    // `delete "" 12345 --yes` → exit 64 canonical; 12345 NOT deleted (no partial delete).
    // RED: current impl accepts "" and proceeds to attempt deletes.
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        // 12345 must NOT be deleted even though it is a valid AID in the list
        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/12345"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "", "12345", "--yes"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "P8-001 (b) empty AID multi: must exit 64 before any delete; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("invalid attachment id: '' (must be numeric)"),
            "P8-001 (b) empty AID multi: stderr must contain canonical error; got stderr: {stderr}"
        );
    }

    // Sub-case P8-001 (c): empty-string AID + --dry-run
    // Must exit 64 (AID guard fires); must NOT emit a `{"id":""}` preview.
    // RED: current impl emits the single-AID dry-run hint/JSON preview for empty string.
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "", "--dry-run"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert_eq!(
            output.status.code(),
            Some(64),
            "P8-001 (c) empty AID dry-run: must exit 64 (guard fires before dry-run); \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("invalid attachment id: '' (must be numeric)"),
            "P8-001 (c) empty AID dry-run: stderr must contain canonical error; \
             got stderr: {stderr}"
        );
        // Must NOT emit a preview JSON with an empty-string id
        assert!(
            !stdout.contains("\"id\""),
            "P8-001 (c) empty AID dry-run: must NOT emit JSON preview; got stdout: {stdout}"
        );
        // Must NOT emit the single-AID dry-run hint
        assert!(
            !stderr.contains("--dry-run has no effect"),
            "P8-001 (c) empty AID dry-run: hint must NOT appear when guard fires; \
             got stderr: {stderr}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-002: single-AID confirmation gate (BC-3.9.015; VP-576-002)
// ---------------------------------------------------------------------------

/// BC-3.9.015 (P7-001): AID validation fires BEFORE the gate.
/// Invalid AID → exit 64 with canonical message; no prompt text; zero HTTP calls.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_015_aid_validation_before_gate() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    // Both metadata GET and DELETE must NOT be called
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10001/../../evil"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .args(["issue", "attachment", "delete", "10001/../../evil"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.9.015 AID validation: must exit 64 before gate; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("invalid attachment id: '10001/../../evil' (must be numeric)"),
        "BC-3.9.015 AID validation: stderr must contain canonical error; got stderr: {stderr}"
    );
    // Gate prompt text must NOT appear
    assert!(
        !stderr.contains("[y/N]"),
        "BC-3.9.015 AID validation: gate prompt must NOT be shown on invalid AID; \
         got stderr: {stderr}"
    );
}

/// VP-576-002 confirm variant: gate presented → "y" → DELETE issued → exit 0.
/// Also tests SEC-576-011 (CWE-116): U+007F in filename is sanitized to '?' in prompt.
///
/// Sub-case A: normal filename, confirm with "y"
/// Sub-case B: filename with U+007F (DEL), confirm with "y", verify sanitized prompt
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_vp_576_002_delete_gate_confirm_proceeds() {
    // Sub-case A: normal filename
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // Pre-prompt metadata GET — supplies filename for the prompt
        Mock::given(method("GET"))
            .and(path("/rest/api/3/attachment/12345"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(attachment_metadata_json("12345", "report.pdf")),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/12345"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .env("JR_STDIN_IS_TTY", "1")
            .args(["issue", "attachment", "delete", "12345"])
            .write_stdin("y\n")
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "VP-576-002 confirm: must exit 0 after 'y'; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("Delete attachment report.pdf (12345)? [y/N]"),
            "VP-576-002 confirm: stderr must contain gate prompt with filename; \
             got stderr: {stderr}"
        );
        assert!(
            stderr.contains("Deleted attachment 12345."),
            "VP-576-002 confirm: stderr must contain success echo after deletion; \
             got stderr: {stderr}"
        );
    }

    // Sub-case B: U+007F (DEL) in filename — SEC-576-011 CWE-116 display sanitization
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // Filename contains U+007F (0x7F = DEL control char); must appear as '?' in prompt
        Mock::given(method("GET"))
            .and(path("/rest/api/3/attachment/99999"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(attachment_metadata_json("99999", "evil\x7fname.pdf")),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/99999"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .env("JR_STDIN_IS_TTY", "1")
            .args(["issue", "attachment", "delete", "99999"])
            .write_stdin("y\n")
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "SEC-576-011: must exit 0 after confirm with sanitized filename; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("evil?name.pdf"),
            "SEC-576-011 CWE-116: U+007F in filename must be replaced with '?' in gate prompt; \
             got stderr: {stderr}"
        );
        assert!(
            !stderr.contains("evil\x7fname.pdf"),
            "SEC-576-011 CWE-116: raw DEL char must NOT appear in gate prompt; \
             got stderr: {stderr}"
        );
    }
}

/// VP-576-002 cancel variant: gate presented → "n" → zero DELETEs → exit 0.
/// JSON mode: stdout `{"cancelled":true,"deleted":false}`.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_vp_576_002_delete_gate_cancel_stays() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    // Pre-prompt metadata GET
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/12345"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(attachment_metadata_json("12345", "report.pdf")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // DELETE must NOT be issued on cancel
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/12345"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args(["issue", "attachment", "delete", "12345", "--output", "json"])
        .write_stdin("n\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "VP-576-002 cancel: must exit 0 on 'n'; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("VP-576-002 cancel: stdout must be valid JSON; parse error: {e}\nstdout: {stdout}")
    });

    let keys: BTreeSet<&str> = parsed
        .as_object()
        .expect("VP-576-002 cancel: stdout must be a JSON object")
        .keys()
        .map(|k| k.as_str())
        .collect();

    assert_eq!(
        keys,
        BTreeSet::from(["cancelled", "deleted"]),
        "VP-576-002 cancel: key set must be exactly {{\"cancelled\",\"deleted\"}} \
         (\"id\" absent — no HTTP confirmed it); got {keys:?}"
    );
    assert_eq!(
        parsed["cancelled"],
        Value::Bool(true),
        "VP-576-002 cancel: \"cancelled\" must be true; got: {}",
        parsed["cancelled"]
    );
    assert_eq!(
        parsed["deleted"],
        Value::Bool(false),
        "VP-576-002 cancel: \"deleted\" must be false; got: {}",
        parsed["deleted"]
    );
    // Human/table cancel message — appears on stderr even in JSON mode (BC-3.9.015 cancel-note)
    // Note: BC-3.9.015 says human mode emits "Deletion cancelled." to stderr;
    // JSON mode emits the cancel envelope to stdout. The stderr message is human-mode only.
    // In JSON mode, test that JSON cancel envelope is correct (above assertions cover this).
    // Absence of "Deletion cancelled." in stderr for JSON mode is implementation-defined;
    // the load-bearing assertion is the JSON shape above.
}

/// BC-3.9.015 EC-3.9.015-5: EOF on stdin → `JrError::Interrupted` → exit 130.
/// `read_line` returns `Ok(0)` (zero bytes, Ctrl+D) → exit 130.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_015_gate_eof_exits_130() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    // Pre-prompt metadata GET — must be issued (before EOF is read)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/12345"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(attachment_metadata_json("12345", "report.pdf")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // DELETE must NOT be issued on EOF path
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/12345"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args(["issue", "attachment", "delete", "12345"])
        .write_stdin("") // EOF — empty stdin, read_line returns Ok(0)
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(130),
        "BC-3.9.015 EC-3.9.015-5: EOF on gate stdin must exit 130 (JrError::Interrupted); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// AC-003: single-AID JSON response shape (BC-3.9.010)
// ---------------------------------------------------------------------------

/// BC-3.9.010 EC-3.9.010-1: single AID + 204 + `--output json` →
/// `{"deleted":true,"id":"12345"}` (2 keys, alphabetical).
/// Also covers human-mode echo `"Deleted attachment 12345."` to stderr.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_010_single_aid_json_shape() {
    // JSON mode
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/12345"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "12345",
                "--yes",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.010 single JSON: must exit 0 on 204; got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!(
                "BC-3.9.010 single JSON: stdout must be valid JSON; error: {e}\nstdout: {stdout}"
            )
        });

        let keys: BTreeSet<&str> = parsed
            .as_object()
            .expect("BC-3.9.010 single JSON: stdout must be a JSON object")
            .keys()
            .map(|k| k.as_str())
            .collect();

        assert_eq!(
            keys,
            BTreeSet::from(["deleted", "id"]),
            "BC-3.9.010 single JSON: key set must be exactly {{\"deleted\",\"id\"}}; \
             got {keys:?}"
        );
        assert_eq!(
            parsed["deleted"],
            Value::Bool(true),
            "BC-3.9.010 single JSON: \"deleted\" must be true; got: {}",
            parsed["deleted"]
        );
        assert_eq!(
            parsed["id"],
            Value::String("12345".to_string()),
            "BC-3.9.010 single JSON: \"id\" must be \"12345\"; got: {}",
            parsed["id"]
        );
    }

    // Human mode — success echo to stderr
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/12345"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "12345", "--yes"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.008 EC-3.9.008-1 human: must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("Deleted attachment 12345."),
            "BC-3.9.008 EC-3.9.008-1 human: stderr must contain echo; got stderr: {stderr}"
        );
        assert!(
            stdout.trim().is_empty(),
            "BC-3.9.008 EC-3.9.008-1 human: stdout must be empty (Symmetric profile); \
             got stdout: {stdout}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-004: bulk --yes required; fail-soft on 404; non-404 aborts (BC-3.9.016/010/013)
// ---------------------------------------------------------------------------

/// BC-3.9.013 multi-delete 404 exception + EC-3.9.010-4: all-404 bulk delete
/// → exit 0, JSON `{"count":0,"deleted":false,"ids":[]}`, `deleted:false`.
/// EC-3.9.010-5 (P3-011): human-mode HINT `"No attachments deleted..."` to stderr
/// (JSON-suppressed).
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_013_bulk_delete_fail_soft_all_404() {
    // JSON mode: all-404 → zero-count JSON shape
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/10001"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "errorMessages": ["Attachment does not exist."]
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/10002"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "errorMessages": ["Attachment does not exist."]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "10001",
                "10002",
                "--yes",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.010 EC-3.9.010-4 all-404: must exit 0 (benign race); \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!(
                "BC-3.9.010 all-404 JSON: stdout must be valid JSON; error: {e}\nstdout: {stdout}"
            )
        });

        assert_eq!(
            parsed["count"],
            Value::Number(0.into()),
            "BC-3.9.010 all-404: count must be 0; got: {}",
            parsed["count"]
        );
        assert_eq!(
            parsed["deleted"],
            Value::Bool(false),
            "BC-3.9.010 all-404: deleted must be false when count=0; got: {}",
            parsed["deleted"]
        );
        assert_eq!(
            parsed["ids"],
            serde_json::json!([]),
            "BC-3.9.010 all-404: ids must be empty array; got: {}",
            parsed["ids"]
        );
        // EC-3.9.010-5 HINT must NOT appear in JSON-mode stderr
        assert!(
            !stderr.contains("No attachments deleted"),
            "BC-3.9.010 EC-3.9.010-5: HINT must be suppressed in JSON mode; \
             got stderr: {stderr}"
        );
    }

    // Human mode: all-404 → EC-3.9.010-5 HINT on stderr
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/10001"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/10002"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "10001", "10002", "--yes"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.010 EC-3.9.010-4 human all-404: must exit 0; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("No attachments deleted (all were already removed or not found)."),
            "BC-3.9.010 EC-3.9.010-5: human mode must emit HINT on all-404; \
             got stderr: {stderr}"
        );
    }
}

/// BC-3.9.016 EC-3.9.016-1/-8: missing `--yes` on bulk paths → exit 64.
/// Sub-case (a): `--issue FOO-1 --older-than 30d` without `--yes` → EC-3.9.016-1 message.
/// Sub-case (b): `10001 10002` without `--yes` → EC-3.9.016-8 message.
/// Both: exit 64 + canonical string + zero HTTP calls.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_016_bulk_requires_yes_exits_64() {
    // Sub-case (a): --older-than without --yes
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // No HTTP calls should be issued before the guard fires
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "30d",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.9.016 EC-3.9.016-1: --older-than without --yes must exit 64; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("--older-than requires --yes to confirm bulk deletion."),
            "BC-3.9.016 EC-3.9.016-1: stderr must contain canonical EC-1 message; \
             got stderr: {stderr}"
        );
    }

    // Sub-case (b): multi-AID without --yes
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "40001", "40002"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.9.016 EC-3.9.016-8: multi-AID without --yes must exit 64; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains(
                "--yes is required to delete multiple attachments without a confirmation prompt."
            ),
            "BC-3.9.016 EC-3.9.016-8: stderr must contain canonical EC-8 message; \
             got stderr: {stderr}"
        );
    }
}

/// BC-3.9.010 EC-3.9.010-4: non-404 error on any AID ABORTS the sequence.
/// AID1→204 (success), AID2→403 (abort), AID3→204 (NOT reached).
/// First deletion stands (not reversed); 403 surfaced; exit 1.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_010_bulk_delete_non_404_aborts_sequence() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    // AID1 succeeds
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // AID2 returns 403 — aborts sequence
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/10002"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorMessages": ["Permission denied."]
        })))
        .expect(1)
        .mount(&server)
        .await;

    // AID3 must NOT be called — sequence aborted after AID2 403
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/10003"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .args([
            "issue",
            "attachment",
            "delete",
            "10001",
            "10002",
            "10003",
            "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-3.9.010 EC-3.9.010-4 non-404 abort: must exit 1 on 403; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // 403 error must be surfaced
    assert!(
        !stderr.is_empty() || {
            // In JSON mode errors go to stdout; in human mode to stderr. Accept either.
            let stdout = String::from_utf8_lossy(&output.stdout);
            !stdout.is_empty()
        },
        "BC-3.9.010 non-404 abort: 403 error must be surfaced; stderr: {stderr}"
    );
}

/// BC-3.9.010 EC-3.9.010-4: partial-404 bulk — 404 is benign; other AIDs succeed.
/// AID1→204, AID2→404 (benign skip), AID3→204.
/// All 3 DELETEs issued; AID2 excluded from ids;
/// JSON `{"count":2,"deleted":true,"ids":["10001","10003"]}`.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_010_bulk_partial_404_skip_continues() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // AID2: 404 — benign skip, iteration continues
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/10002"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/10003"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .args([
            "issue",
            "attachment",
            "delete",
            "10001",
            "10002",
            "10003",
            "--yes",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.010 partial-404: must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("BC-3.9.010 partial-404: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
    });

    assert_eq!(
        parsed["count"],
        Value::Number(2.into()),
        "BC-3.9.010 partial-404: count must be 2; got: {}",
        parsed["count"]
    );
    assert_eq!(
        parsed["deleted"],
        Value::Bool(true),
        "BC-3.9.010 partial-404: deleted must be true; got: {}",
        parsed["deleted"]
    );
    // ids must contain AID1 and AID3, not AID2 (the 404'd one)
    let ids = parsed["ids"]
        .as_array()
        .expect("BC-3.9.010 partial-404: ids must be an array");
    let id_set: BTreeSet<&str> = ids.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(
        id_set,
        BTreeSet::from(["10001", "10003"]),
        "BC-3.9.010 partial-404: ids must contain only the successful AIDs; got {id_set:?}"
    );
    assert!(
        !ids.iter().any(|v| v.as_str() == Some("10002")),
        "BC-3.9.010 partial-404: 404'd AID2 must NOT appear in ids"
    );
}

// ---------------------------------------------------------------------------
// AC-005: bulk JSON response shape (BC-3.9.010)
// ---------------------------------------------------------------------------

/// BC-3.9.010 EC-3.9.010-2: two AIDs, both 204.
/// JSON `{"count":2,"deleted":true,"ids":["10001","10002"]}` (3 keys, alphabetical).
/// `ids` in command-line-supplied order.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_010_bulk_json_shape() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/10002"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .args([
            "issue",
            "attachment",
            "delete",
            "10001",
            "10002",
            "--yes",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.010 bulk JSON: must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("BC-3.9.010 bulk JSON: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
    });

    let keys: BTreeSet<&str> = parsed
        .as_object()
        .expect("BC-3.9.010 bulk JSON: stdout must be a JSON object")
        .keys()
        .map(|k| k.as_str())
        .collect();

    assert_eq!(
        keys,
        BTreeSet::from(["count", "deleted", "ids"]),
        "BC-3.9.010 bulk JSON: key set must be exactly {{\"count\",\"deleted\",\"ids\"}}; \
         got {keys:?}"
    );
    assert_eq!(
        parsed["count"],
        Value::Number(2.into()),
        "BC-3.9.010 bulk JSON: count must be 2; got: {}",
        parsed["count"]
    );
    assert_eq!(
        parsed["deleted"],
        Value::Bool(true),
        "BC-3.9.010 bulk JSON: deleted must be true; got: {}",
        parsed["deleted"]
    );
    assert_eq!(
        parsed["ids"],
        serde_json::json!(["10001", "10002"]),
        "BC-3.9.010 bulk JSON: ids must be [\"10001\",\"10002\"] in supplied order; \
         got: {}",
        parsed["ids"]
    );
}

// ---------------------------------------------------------------------------
// AC-006: --issue KEY + --older-than + --yes combined (BC-3.9.016 + BC-3.9.019)
// ---------------------------------------------------------------------------

/// BC-3.9.016 + BC-3.9.019: `--issue FOO-1 --older-than 1d --yes`.
/// Wire: GET attachment list → age filter (2 old, 1 new) → 2 DELETEs.
/// Sub-assertion (a) N>0 human: stderr has pre-deletion HINT + success summary (JSON-suppressed).
/// Sub-assertion (b) N>0 JSON: NEITHER hint on stderr.
/// Sub-assertion (c) zero-match human: stderr has zero-match echo.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_019_issue_key_older_than_resolution() {
    // Sub-assertion (a): N>0 human — pre-deletion HINT + success summary
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // Issue attachment list: 2 old (will be selected), 1 new (will be excluded)
        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(issue_attachments_json(&[
                    old_attachment("10001", "old1.txt"),
                    old_attachment("10002", "old2.txt"),
                    new_attachment("10003", "new1.txt"),
                ])),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/10001"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/10002"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        // new1.txt must NOT be deleted
        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/10003"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "1d",
                "--yes",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.019 N>0 human: must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("Deleting 2 attachment(s) older than 1d from FOO-1."),
            "BC-3.9.019: stderr must contain pre-deletion HINT (human mode); got stderr: {stderr}"
        );
        assert!(
            stderr.contains("Deleted 2 attachment(s) older than 1d from FOO-1."),
            "BC-3.9.019: stderr must contain success summary (human mode); got stderr: {stderr}"
        );
    }

    // Sub-assertion (b): N>0 JSON — hints suppressed
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(issue_attachments_json(&[
                    old_attachment("10001", "old1.txt"),
                    old_attachment("10002", "old2.txt"),
                ])),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/10001"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/10002"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "1d",
                "--yes",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.019 N>0 JSON: must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        // HINT strings must NOT appear in JSON mode (JSON-suppressed per P30-002)
        assert!(
            !stderr.contains("Deleting"),
            "BC-3.9.019: pre-deletion HINT must be suppressed in JSON mode; \
             got stderr: {stderr}"
        );
        assert!(
            !stderr.contains("Deleted 2 attachment(s) older than"),
            "BC-3.9.019: success summary must be suppressed in JSON mode; \
             got stderr: {stderr}"
        );
        // JSON result must carry count
        let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("BC-3.9.019 N>0 JSON: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
        });
        assert_eq!(
            parsed["count"],
            Value::Number(2.into()),
            "BC-3.9.019 N>0 JSON: count must be 2; got: {}",
            parsed["count"]
        );
        assert_eq!(
            parsed["deleted"],
            Value::Bool(true),
            "BC-3.9.019 N>0 JSON: deleted must be true; got: {}",
            parsed["deleted"]
        );
    }

    // Sub-assertion (c): zero-match human — zero-match echo
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // All attachments are new — none selected by filter
        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(issue_attachments_json(&[
                    new_attachment("10003", "new1.txt"),
                ])),
            )
            .expect(1)
            .mount(&server)
            .await;

        // No DELETEs should be issued
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "1d",
                "--yes",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.019 zero-match: must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("No attachments older than 1d found on FOO-1."),
            "BC-3.9.019 EC-3.9.019-2: stderr must contain zero-match echo; \
             got stderr: {stderr}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-007: --older-than duration parsing via parse_age_duration (BC-3.9.019)
// ---------------------------------------------------------------------------

/// BC-3.9.019 EC-3.9.019-3: invalid duration → exit 64 + canonical error string.
/// BC-3.9.019 EC-3.9.019-2: valid duration, zero matches → exit 0 +
/// `{"count":0,"deleted":false,"ids":[]}`.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_019_older_than_parse_age_duration_filter() {
    // Sub-case: invalid duration string
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // No HTTP calls should be issued before the parse guard fires
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "badval",
                "--yes",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.9.019 EC-3.9.019-3: invalid duration must exit 64; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("invalid duration: 'badval'. Use formats like 30m, 2h, 1d, 7d, 2w."),
            "BC-3.9.019 EC-3.9.019-3: stderr must contain canonical error; \
             got stderr: {stderr}"
        );
    }

    // Sub-case: valid duration, no attachments on issue → empty JSON
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // Empty attachment list
        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_attachments_json(&[])))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "7d",
                "--yes",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.019 EC-3.9.019-2 zero-match: must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!(
                "BC-3.9.019 zero-match JSON: stdout must be valid JSON; error: {e}\nstdout: {stdout}"
            )
        });

        assert_eq!(
            parsed,
            serde_json::json!({"count": 0, "deleted": false, "ids": []}),
            "BC-3.9.019 EC-3.9.019-2: zero-match JSON must be \
             {{\"count\":0,\"deleted\":false,\"ids\":[]}}; got: {parsed}"
        );
    }

    // Sub-case P1-001a: multibyte trailing char — must exit 64, NOT panic
    // `s.split_at(s.len()-1)` on "5€" is off a char boundary → panic without guard.
    // Assertion also verifies "panicked" is absent from stderr.
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // Parse guard must fire BEFORE any HTTP call
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "5\u{20AC}", // "5€" — multibyte (3 UTF-8 bytes) trailing char
                "--yes",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "P1-001a multibyte trailing char '5€': must exit 64 (not panic/101); \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("invalid duration: '5\u{20AC}'. Use formats like 30m, 2h, 1d, 7d, 2w."),
            "P1-001a multibyte: stderr must contain canonical error; got stderr: {stderr}"
        );
        assert!(
            !stderr.contains("panicked"),
            "P1-001a multibyte: must NOT emit panic output; got stderr: {stderr}"
        );
    }

    // Sub-case P1-001b: overflow-large week value — must exit 64, NOT overflow-panic
    // In debug builds, `n * 7 * 24` overflows i64 for n=99999999999999999.
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "99999999999999999w",
                "--yes",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "P1-001b overflow weeks '99999999999999999w': must exit 64 (not overflow-panic); \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("invalid duration"),
            "P1-001b overflow: stderr must contain 'invalid duration'; got stderr: {stderr}"
        );
        assert!(
            !stderr.contains("panicked"),
            "P1-001b overflow: must NOT emit panic output; got stderr: {stderr}"
        );
    }

    // Sub-case P2-001: chrono::Duration::seconds panic band
    // n=1e12 days → 8.64e16 seconds — inside the range (i64::MAX/1000, i64::MAX] where
    // Duration::hours calls Duration::seconds which multiplies by MILLIS_PER_SEC (1000),
    // overflowing i64 and panicking via checked_mul(...).expect("out of bounds").
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // Parse guard must fire BEFORE any HTTP call
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "1000000000000d", // n=1e12 days → 8.64e16 s → inside chrono panic band
                "--yes",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "P2-001 chrono band '1000000000000d': must exit 64 (not SIGABRT/panic/101); \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains(
                "invalid duration: '1000000000000d'. Use formats like 30m, 2h, 1d, 7d, 2w."
            ),
            "P2-001 chrono band: stderr must contain canonical error; got stderr: {stderr}"
        );
        assert!(
            !stderr.contains("panicked"),
            "P2-001 chrono band: must NOT emit panic output; got stderr: {stderr}"
        );
    }

    // Sub-case P6-001: DateTime-subtraction overflow band
    // n=1e11 days → 8.64e15 seconds. try_seconds(8.64e15) succeeds (8.64e15 * 1000 = 8.64e18
    // < i64::MAX 9.22e18), BUT `Utc::now() - duration` produces a date ~274 million years
    // before epoch — before chrono::NaiveDate::MIN (~year −262144) — and panics at the
    // subtraction site (attachments.rs::handle_attachment_delete_older_than ~1607).
    // Implementation must guard with `Utc::now().checked_sub_signed(duration)` or clamp
    // inside parse_age_duration with an additional bound check.
    // RED: current impl panics instead of returning exit 64.
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // Parse/subtraction guard must fire BEFORE any HTTP call
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "100000000000d", // n=1e11 days → 8.64e15 s — passes try_seconds but
                // Utc::now()-duration panics (DateTime subtraction overflow band)
                "--yes",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "P6-001 DateTime-subtraction band '100000000000d': must exit 64 (not SIGABRT/\
             panic/101); got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains(
                "invalid duration: '100000000000d'. Use formats like 30m, 2h, 1d, 7d, 2w."
            ),
            "P6-001 DateTime band: stderr must contain canonical error; got stderr: {stderr}"
        );
        assert!(
            !stderr.contains("panicked"),
            "P6-001 DateTime band: must NOT emit panic output; got stderr: {stderr}"
        );
    }
}

// Unit test test_bc_3_9_019_ec_8_parse_age_duration_1d_is_24h lives in
// src/cli/issue/attachments.rs #[cfg(test)] (EC-3.9.019-8; private helper).

// ---------------------------------------------------------------------------
// AC-008: --dry-run single-AID (BC-3.9.020 EC-3.9.020-3)
// ---------------------------------------------------------------------------

/// BC-3.9.020 EC-3.9.020-3: `jr issue attachment delete <AID> --dry-run`.
/// AID validation fires (guard NOT suppressed).
/// Human: stderr hint `"--dry-run has no effect on single-ID delete; omit the flag."` + exit 0.
/// JSON: `{"attachments":[{"id":"<AID>"}],"dryRun":true,"ids":["<AID>"]}`.
/// Invalid AID + --dry-run: exit 64 (guard fires; dry-run hint NOT emitted).
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_020_dry_run_single_aid() {
    // Human mode: valid AID + --dry-run → stderr hint, no DELETE, exit 0
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/12345"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0) // no DELETE on dry-run
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "12345", "--dry-run"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.020 EC-3.9.020-3 human: must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("--dry-run has no effect on single-ID delete; omit the flag."),
            "BC-3.9.020 EC-3.9.020-3 human: stderr must contain canonical hint; \
             got stderr: {stderr}"
        );
    }

    // JSON mode: valid AID + --dry-run → JSON shape, no stderr hint, exit 0
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/12345"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "12345",
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
            Some(0),
            "BC-3.9.020 EC-3.9.020-3 JSON: must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        // In JSON mode, the dry-run hint is NOT emitted to stderr
        assert!(
            !stderr.contains("--dry-run has no effect"),
            "BC-3.9.020 EC-3.9.020-3 JSON: hint must NOT appear on stderr in JSON mode; \
             got stderr: {stderr}"
        );

        let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!(
                "BC-3.9.020 single dry-run JSON: stdout must be valid JSON; \
                 error: {e}\nstdout: {stdout}"
            )
        });

        let keys: BTreeSet<&str> = parsed
            .as_object()
            .expect("BC-3.9.020 single dry-run: stdout must be a JSON object")
            .keys()
            .map(|k| k.as_str())
            .collect();

        assert_eq!(
            keys,
            BTreeSet::from(["attachments", "dryRun", "ids"]),
            "BC-3.9.020 EC-3.9.020-3 JSON: key set must be \
             {{\"attachments\",\"dryRun\",\"ids\"}}; got {keys:?}"
        );
        assert_eq!(
            parsed["dryRun"],
            Value::Bool(true),
            "BC-3.9.020 EC-3.9.020-3 JSON: dryRun must be true; got: {}",
            parsed["dryRun"]
        );
        assert_eq!(
            parsed["ids"],
            serde_json::json!(["12345"]),
            "BC-3.9.020 EC-3.9.020-3 JSON: ids must be [\"12345\"]; got: {}",
            parsed["ids"]
        );
        let attachments = parsed["attachments"]
            .as_array()
            .expect("BC-3.9.020 single dry-run JSON: attachments must be an array");
        assert_eq!(
            attachments.len(),
            1,
            "BC-3.9.020 single dry-run JSON: attachments must have 1 element"
        );
        assert_eq!(
            attachments[0]["id"],
            Value::String("12345".to_string()),
            "BC-3.9.020 single dry-run JSON: attachments[0].id must be \"12345\"; \
             got: {}",
            attachments[0]["id"]
        );
        // No "filename" key — no metadata fetch on single-ID dry-run (P8-004)
        assert!(
            attachments[0].get("filename").is_none(),
            "BC-3.9.020 single dry-run JSON: attachments[0] must NOT have \"filename\" \
             (no metadata fetch on single-ID dry-run)"
        );
    }

    // Invalid AID + --dry-run: exit 64 (guard fires; dry-run hint NOT emitted)
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "bad_id", "--dry-run"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.9.020 EC-3.9.020-3 invalid AID: must exit 64 even with --dry-run; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("invalid attachment id: 'bad_id' (must be numeric)"),
            "BC-3.9.020 invalid AID: stderr must contain canonical AID error; \
             got stderr: {stderr}"
        );
        // Dry-run hint must NOT appear (the AID guard fires first)
        assert!(
            !stderr.contains("--dry-run has no effect"),
            "BC-3.9.020 EC-3.9.020-3: dry-run hint must NOT appear when AID guard fires; \
             got stderr: {stderr}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-009: --dry-run bulk (BC-3.9.020 EC-3.9.020-1/2)
// ---------------------------------------------------------------------------

/// BC-3.9.020 EC-3.9.020-1/2: bulk `--dry-run` (--issue/--older-than path).
/// No DELETE issued; --yes NOT required.
/// Human: table + `"<N> attachment(s) would be deleted. Run without --dry-run to confirm."`.
/// JSON: `{"attachments":[{"filename":"<n>","id":"<AID>"}],"dryRun":true,"ids":[...]}`.
/// Zero matches: `{"attachments":[],"dryRun":true,"ids":[]}`.
///
/// Also tests multi-AID --dry-run path (b): per-AID metadata fan-out GET.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_020_dry_run_bulk() {
    // --older-than --dry-run (path a): N>0 JSON shape
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(issue_attachments_json(&[
                    old_attachment("10001", "report.pdf"),
                    old_attachment("10002", "notes.txt"),
                ])),
            )
            .expect(1)
            .mount(&server)
            .await;

        // No DELETEs on dry-run
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "1d",
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
            Some(0),
            "BC-3.9.020 EC-3.9.020-1 --older-than dry-run: must exit 0; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!(
                "BC-3.9.020 --older-than dry-run JSON: stdout must be valid JSON; \
                 error: {e}\nstdout: {stdout}"
            )
        });

        let outer_keys: BTreeSet<&str> = parsed
            .as_object()
            .expect("BC-3.9.020 dry-run: stdout must be a JSON object")
            .keys()
            .map(|k| k.as_str())
            .collect();

        assert_eq!(
            outer_keys,
            BTreeSet::from(["attachments", "dryRun", "ids"]),
            "BC-3.9.020 EC-3.9.020-1 JSON: key set must be \
             {{\"attachments\",\"dryRun\",\"ids\"}}; got {outer_keys:?}"
        );
        assert_eq!(
            parsed["dryRun"],
            Value::Bool(true),
            "BC-3.9.020 dry-run JSON: dryRun must be true"
        );
        let ids = parsed["ids"]
            .as_array()
            .expect("BC-3.9.020 dry-run JSON: ids must be an array");
        assert_eq!(
            ids.len(),
            2,
            "BC-3.9.020 dry-run JSON: ids must have 2 elements"
        );
        let atts = parsed["attachments"]
            .as_array()
            .expect("BC-3.9.020 dry-run JSON: attachments must be an array");
        assert_eq!(
            atts.len(),
            2,
            "BC-3.9.020 dry-run JSON: attachments must have 2 elements"
        );
        // Each attachment element must have filename and id keys
        for att in atts {
            let att_keys: BTreeSet<&str> = att
                .as_object()
                .expect("BC-3.9.020 dry-run: attachment element must be an object")
                .keys()
                .map(|k| k.as_str())
                .collect();
            assert_eq!(
                att_keys,
                BTreeSet::from(["filename", "id"]),
                "BC-3.9.020 dry-run: attachment element key set must be \
                 {{\"filename\",\"id\"}}; got {att_keys:?}"
            );
        }
    }

    // --older-than --dry-run zero-match JSON shape
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(issue_attachments_json(&[
                    new_attachment("10003", "new1.txt"),
                ])),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "1d",
                "--dry-run",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.020 EC-3.9.020-2 dry-run zero-match: must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("BC-3.9.020 dry-run zero-match: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
        });
        assert_eq!(
            parsed,
            serde_json::json!({"attachments": [], "dryRun": true, "ids": []}),
            "BC-3.9.020 EC-3.9.020-2: zero-match dry-run JSON must match canonical shape; \
             got: {parsed}"
        );
    }

    // P3-001: --dry-run human: full [ID, Filename, Size, Created] table + final count line.
    // Two attachments; second has U+007F in filename for CWE-116 display-sanitization pin.
    // RED: current impl prints count line only — no table rows with ID/Filename/Size/Created.
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(issue_attachments_json(&[
                    // size=512 → "512 B"; created="2000-01-01T..."
                    old_attachment("10001", "report.pdf"),
                    // U+007F prefix → display_sanitize_filename → "?poisoned.pdf" (CWE-116)
                    old_attachment("10002", "\x7fpoisoned.pdf"),
                ])),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "--issue",
                "FOO-1",
                "--older-than",
                "1d",
                "--dry-run",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{stderr}{stdout}");

        assert_eq!(
            output.status.code(),
            Some(0),
            "P3-001 issue/older-than dry-run human: must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        // Per-attachment table row: ID column
        assert!(
            combined.contains("10001"),
            "P3-001 issue/older-than dry-run: table must contain ID '10001'; got combined: {combined}"
        );
        assert!(
            combined.contains("10002"),
            "P3-001 issue/older-than dry-run: table must contain ID '10002'; got combined: {combined}"
        );

        // Filename column
        assert!(
            combined.contains("report.pdf"),
            "P3-001 issue/older-than dry-run: table must contain filename 'report.pdf'; \
             got combined: {combined}"
        );
        // CWE-116: U+007F must be sanitized to '?'
        assert!(
            combined.contains("?poisoned.pdf"),
            "P3-001 CWE-116: U+007F in filename must be sanitized to '?' in dry-run table; \
             got combined: {combined}"
        );
        assert!(
            !combined.contains("\x7fpoisoned.pdf"),
            "P3-001 CWE-116: raw U+007F (0x7F) must NOT appear in any output channel; \
             got combined: {combined}"
        );

        // Size column: format_size(512) = "512 B"
        assert!(
            combined.contains("512 B"),
            "P3-001 issue/older-than dry-run: table must contain human size '512 B'; \
             got combined: {combined}"
        );

        // Created column: both fixtures have created="2000-01-01T..."
        assert!(
            combined.contains("2000"),
            "P3-001 issue/older-than dry-run: table must contain created date fragment '2000'; \
             got combined: {combined}"
        );

        // Final count line (AC-009 canonical)
        assert!(
            combined.contains("2 attachment(s) would be deleted"),
            "P3-001 issue/older-than dry-run: output must contain \
             '2 attachment(s) would be deleted'; got combined: {combined}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-009 P2-002: multi-AID dry-run metadata fan-out
// ---------------------------------------------------------------------------

/// AC-009 P2-002: `jr issue attachment delete <AID1> <AID2> --dry-run`
/// performs per-AID GET /rest/api/3/attachment/{id} to populate filename.
///
/// Current impl (lines ~1480-1483) emits `{"id": id}` with zero metadata calls.
/// This test pins the required behavior:
/// - Sub-case A: both GETs succeed → JSON `{filename, id}` per attachment
/// - Sub-case B: one GET fails (403) → {id}-only fallback; still exit 0
/// - Sub-case C: human mode → "Filename" column populated (not blank / id-only)
///
/// RED: current impl emits id-only with zero metadata calls; `.expect(1)` mock
/// verification will abort OR assertion on `filename` key will fail.
#[tokio::test]
async fn test_bc_3_9_020_dry_run_multi_aid_metadata_fan_out() {
    // Sub-case A: both metadata GETs succeed → JSON has {filename,id} for each
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("GET"))
            .and(path("/rest/api/3/attachment/20001"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "20001",
                "filename": "archive.zip",
                "size": 4096
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/attachment/20002"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "20002",
                "filename": "notes.txt",
                "size": 512
            })))
            .expect(1)
            .mount(&server)
            .await;

        // No DELETEs on dry-run
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "20001",
                "20002",
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
            Some(0),
            "AC-009 P2-002 sub-A: multi-AID dry-run must exit 0; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("AC-009 P2-002 sub-A: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
        });

        let atts = parsed["attachments"]
            .as_array()
            .expect("AC-009 P2-002 sub-A: 'attachments' must be an array");
        assert_eq!(
            atts.len(),
            2,
            "AC-009 P2-002 sub-A: attachments array must have 2 elements"
        );

        // Every attachment element must carry {filename, id}
        for att in atts {
            let att_keys: BTreeSet<&str> = att
                .as_object()
                .expect("AC-009 P2-002 sub-A: attachment element must be a JSON object")
                .keys()
                .map(|k| k.as_str())
                .collect();
            assert_eq!(
                att_keys,
                BTreeSet::from(["filename", "id"]),
                "AC-009 P2-002 sub-A: each attachment must have exactly \
                 {{\"filename\",\"id\"}} keys (from metadata GET); got {att_keys:?}\n\
                 full att: {att}"
            );
        }

        // Filenames must be populated from the metadata responses
        let filenames: Vec<&str> = atts
            .iter()
            .map(|a| {
                a["filename"]
                    .as_str()
                    .expect("AC-009 P2-002 sub-A: filename must be a string")
            })
            .collect();
        assert!(
            filenames.contains(&"archive.zip"),
            "AC-009 P2-002 sub-A: must contain filename 'archive.zip'; got {filenames:?}"
        );
        assert!(
            filenames.contains(&"notes.txt"),
            "AC-009 P2-002 sub-A: must contain filename 'notes.txt'; got {filenames:?}"
        );

        // ids array must contain both AIDs
        let ids = parsed["ids"]
            .as_array()
            .expect("AC-009 P2-002 sub-A: 'ids' must be an array");
        let id_strs: Vec<&str> = ids.iter().map(|v| v.as_str().unwrap_or("")).collect();
        assert!(
            id_strs.contains(&"20001"),
            "AC-009 P2-002 sub-A: ids must contain '20001'; got {id_strs:?}"
        );
        assert!(
            id_strs.contains(&"20002"),
            "AC-009 P2-002 sub-A: ids must contain '20002'; got {id_strs:?}"
        );
    }

    // Sub-case B: one metadata GET fails (403) → that row is {id}-only; still exit 0
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        Mock::given(method("GET"))
            .and(path("/rest/api/3/attachment/20003"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "20003",
                "filename": "report.pdf"
            })))
            .expect(1)
            .mount(&server)
            .await;

        // 20004: metadata fetch fails
        Mock::given(method("GET"))
            .and(path("/rest/api/3/attachment/20004"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_json(serde_json::json!({"errorMessages": ["Permission denied."]})),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "20003",
                "20004",
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
            Some(0),
            "AC-009 P2-002 sub-B: metadata-fail dry-run must exit 0 (fallback, not abort); \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("AC-009 P2-002 sub-B: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
        });

        let atts = parsed["attachments"]
            .as_array()
            .expect("AC-009 P2-002 sub-B: 'attachments' must be an array");
        assert_eq!(
            atts.len(),
            2,
            "AC-009 P2-002 sub-B: 2 elements in attachments"
        );

        // Row for 20003 must have filename
        let att_20003 = atts
            .iter()
            .find(|a| a["id"] == "20003")
            .expect("AC-009 P2-002 sub-B: must have entry for 20003");
        assert_eq!(
            att_20003["filename"], "report.pdf",
            "AC-009 P2-002 sub-B: 20003 filename must be 'report.pdf'"
        );

        // Row for 20004 (failed GET) must have ONLY "id" key (no "filename")
        let att_20004 = atts
            .iter()
            .find(|a| a["id"] == "20004")
            .expect("AC-009 P2-002 sub-B: must have entry for 20004");
        let keys_20004: BTreeSet<&str> = att_20004
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        assert_eq!(
            keys_20004,
            BTreeSet::from(["id"]),
            "AC-009 P2-002 sub-B: metadata-fail row must have only 'id' key \
             (no 'filename'); got {keys_20004:?}"
        );
    }

    // Sub-case C (P3-001): human mode — full [ID, Filename, Size, Created] table
    // + fallback "(metadata unavailable)" for failed metadata GET (AID 20007)
    // + U+007F poison in 20006 filename for CWE-116 pin
    // RED: impl emits count line only — no per-row table, no size/created columns.
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // 20005: normal metadata — size=2048 → "2.0 KB", created fragment "2023"
        Mock::given(method("GET"))
            .and(path("/rest/api/3/attachment/20005"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "20005",
                "filename": "diagram.png",
                "size": 2048,
                "created": "2023-06-15T10:00:00.000+0000"
            })))
            .expect(1)
            .mount(&server)
            .await;

        // 20006: U+007F in filename (CWE-116) — must appear as "s?pec.docx" in table
        Mock::given(method("GET"))
            .and(path("/rest/api/3/attachment/20006"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "20006",
                "filename": "s\u{007F}pec.docx",
                "size": 512,
                "created": "2023-06-15T11:00:00.000+0000"
            })))
            .expect(1)
            .mount(&server)
            .await;

        // 20007: metadata GET fails (403) → "(metadata unavailable)" fallback row
        Mock::given(method("GET"))
            .and(path("/rest/api/3/attachment/20007"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_json(serde_json::json!({"errorMessages": ["Permission denied."]})),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "20005",
                "20006",
                "20007",
                "--dry-run",
                // human mode (no --output json)
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{stdout}{stderr}");

        assert_eq!(
            output.status.code(),
            Some(0),
            "AC-009 P3-001 sub-C: human dry-run must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        // Table row 20005: ID, filename, size "2.0 KB", created "2023"
        assert!(
            combined.contains("20005"),
            "P3-001 sub-C: table must contain ID '20005'; got combined: {combined}"
        );
        assert!(
            combined.contains("diagram.png"),
            "P3-001 sub-C: table must contain filename 'diagram.png'; got combined: {combined}"
        );
        assert!(
            combined.contains("2.0 KB"),
            "P3-001 sub-C: table must contain human size '2.0 KB' (2048 B); got combined: {combined}"
        );
        assert!(
            combined.contains("2023"),
            "P3-001 sub-C: table must contain created date fragment '2023'; got combined: {combined}"
        );

        // Table row 20006: U+007F sanitized to '?' (CWE-116)
        assert!(
            combined.contains("20006"),
            "P3-001 sub-C: table must contain ID '20006'; got combined: {combined}"
        );
        assert!(
            combined.contains("s?pec.docx"),
            "P3-001 sub-C CWE-116: U+007F in filename must be sanitized to '?' in dry-run table; \
             got combined: {combined}"
        );
        assert!(
            !combined.contains("s\x7fpec.docx"),
            "P3-001 sub-C CWE-116: raw U+007F must NOT appear in any output channel; \
             got combined: {combined}"
        );

        // Table row 20007: failed metadata → "(metadata unavailable)" fallback
        assert!(
            combined.contains("20007"),
            "P3-001 sub-C: table must contain ID '20007' even for failed metadata; \
             got combined: {combined}"
        );
        assert!(
            combined.contains("(metadata unavailable)"),
            "P3-001 sub-C: failed metadata row must show '(metadata unavailable)'; \
             got combined: {combined}"
        );

        // Final count line
        assert!(
            combined.contains("3 attachment(s) would be deleted"),
            "P3-001 sub-C: must emit '3 attachment(s) would be deleted'; got combined: {combined}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-010: non-interactive without --yes exits 64 (BC-3.9.015; DEC-174)
// ---------------------------------------------------------------------------

/// BC-3.9.015 EC-3.9.015-3: `--no-input` or non-TTY stdin without `--yes` → exit 64.
/// Canonical message: `"Use --yes to confirm deletion without a prompt."`.
/// No HTTP calls issued.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_015_non_interactive_without_yes_exits_64() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    // No HTTP calls allowed before the non-interactive guard fires
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/12345"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .args(["issue", "attachment", "delete", "12345", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.9.015 EC-3.9.015-3: --no-input without --yes must exit 64; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Use --yes to confirm deletion without a prompt."),
        "BC-3.9.015 EC-3.9.015-3: stderr must contain canonical --yes hint; \
         got stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-011: --issue + --older-than + --yes combined (BC-3.9.016 + BC-3.9.019)
// ---------------------------------------------------------------------------

/// BC-3.9.016 + BC-3.9.019: full combined flow with `--yes`.
/// Bulk forms ALWAYS require `--yes` (no interactive gate offered).
/// Wire: list GET → age filter → serial DELETEs → exit 0.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_016_issue_older_than_yes_combined() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    // Issue has 3 old attachments — all should be selected and deleted
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/BAR-2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_attachments_json(&[
                old_attachment("20001", "a.txt"),
                old_attachment("20002", "b.txt"),
                old_attachment("20003", "c.txt"),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/20001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/20002"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/20003"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .args([
            "issue",
            "attachment",
            "delete",
            "--issue",
            "BAR-2",
            "--older-than",
            "7d",
            "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.016 + BC-3.9.019 combined: must exit 0 on full success; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // No interactive gate prompt should appear (bulk always-yes, no gate)
    assert!(
        !stderr.contains("[y/N]"),
        "BC-3.9.016 bulk: no interactive gate prompt should appear on bulk --yes path; \
         got stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-013: VP-576-002 wiremock anchor + DEC-168 body surfacing (BC-3.9.008)
// ---------------------------------------------------------------------------

/// DEC-168 body surfacing (EC-3.9.008-2): stderr MUST BEGIN with canonical prefix
/// `"Attachment <AID> not found or not accessible."` THEN the Jira error body.
/// NOT body-only; NOT silent exit 0. Exit 64.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_008_404_body_surfaced_to_stderr() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/12345"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["Attachment does not exist."],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .args(["issue", "attachment", "delete", "12345", "--yes"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.9.008 EC-3.9.008-2 DEC-168: must exit 64 on 404; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // Canonical prefix must appear in stderr (DEC-168: prepend canonical string, append body)
    assert!(
        stderr.contains("Attachment 12345 not found or not accessible."),
        "BC-3.9.008 EC-3.9.008-2 DEC-168: stderr must contain canonical prefix; \
         got stderr: {stderr}"
    );
    // Jira error body must also appear (NOT silent — DEC-168 requires body surface)
    assert!(
        stderr.contains("Attachment does not exist."),
        "BC-3.9.008 EC-3.9.008-2 DEC-168: stderr must also contain Jira error body; \
         got stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-014: bare --issue without --older-than → exit 2 (BC-3.9.016 EC-3.9.016-9)
// ---------------------------------------------------------------------------

/// BC-3.9.016 EC-3.9.016-9: `--issue <KEY>` without `--older-than` → exit 2 (clap `requires`).
/// No application code reached.
///
/// This test exercises a clap-level constraint and may pass at Red Gate
/// if the clap `requires` constraint is correctly declared in the stub.
#[tokio::test]
async fn test_bc_3_9_016_issue_without_older_than_exit_2() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .args(["issue", "attachment", "delete", "--issue", "FOO-1"])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(2),
        "BC-3.9.016 EC-3.9.016-9: --issue without --older-than must exit 2 (clap requires); \
         got {:?}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// AC-015: clap mutual-exclusion + required-group constraints (BC-3.9.016)
// ---------------------------------------------------------------------------

/// BC-3.9.016 EC-3.9.016-4/5/9/10: all five clap-level constraint cases → exit 2.
/// (a) `<AID> --issue FOO-1` → exit 2 (conflicts_with).
/// (b) `<AID> --older-than 7d` → exit 2 (conflicts_with).
/// (c) `--older-than 7d` (no --issue) → exit 2 (requires).
/// (d) `--issue FOO-1` (no --older-than) → exit 2 (requires).
/// (e) `delete` (no args) → exit 2 (required group).
///
/// These tests exercise clap constraints and may pass at Red Gate
/// if the clap definition in the stub is correct.
#[tokio::test]
async fn test_bc_3_9_016_clap_mutual_exclusion_constraints() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    // (a) AID + --issue → exit 2
    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "12345", "--issue", "FOO-1"])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "(a) AID + --issue must exit 2 (conflicts_with); got {:?}",
            out.status.code()
        );
    }

    // (b) AID + --older-than → exit 2
    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args([
                "issue",
                "attachment",
                "delete",
                "12345",
                "--older-than",
                "7d",
            ])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "(b) AID + --older-than must exit 2 (conflicts_with); got {:?}",
            out.status.code()
        );
    }

    // (c) --older-than without --issue → exit 2
    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "--older-than", "7d"])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "(c) --older-than without --issue must exit 2 (requires); got {:?}",
            out.status.code()
        );
    }

    // (d) --issue without --older-than → exit 2
    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "--issue", "FOO-1"])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "(d) --issue without --older-than must exit 2 (requires); got {:?}",
            out.status.code()
        );
    }

    // (e) bare delete (no args) → exit 2
    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete"])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "(e) bare 'delete' with no args must exit 2 (required-group); got {:?}",
            out.status.code()
        );
    }
}

// ---------------------------------------------------------------------------
// AC-016: delete error taxonomy (BC-3.9.013)
// ---------------------------------------------------------------------------

/// BC-3.9.013: 401 → exit 2; stderr contains "Not authenticated" AND "jr auth login".
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_013_delete_401_exit_2() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/12345"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["You are not authenticated."]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .args(["issue", "attachment", "delete", "12345", "--yes"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(2),
        "BC-3.9.013: 401 must exit 2 (JrError::NotAuthenticated); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Not authenticated"),
        "BC-3.9.013: stderr must contain 'Not authenticated'; got stderr: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "BC-3.9.013: stderr must contain 'jr auth login'; got stderr: {stderr}"
    );
}

/// BC-3.9.013 EC-3.9.013-2: 403 → exit 1; Jira error body surfaced to stderr.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_013_delete_403_exit_1() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/12345"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorMessages": ["You do not have permission to delete this attachment."]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .args(["issue", "attachment", "delete", "12345", "--yes"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-3.9.013 EC-3.9.013-2: 403 must exit 1; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // Jira error body must be surfaced (either channel depending on --output mode)
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("You do not have permission"),
        "BC-3.9.013 EC-3.9.013-2: Jira 403 error body must be surfaced; \
         got stderr: {stderr}\nstdout: {stdout}"
    );
}

/// BC-3.9.013: 5xx → exit 1; stderr contains `"API error ("` (loose-substring).
/// Full literal: `"API error (500): <message>"` from `src/error.rs::JrError`.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_013_delete_5xx_exit_1() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/12345"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
        .args(["issue", "attachment", "delete", "12345", "--yes"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-3.9.013: 500 must exit 1; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // API error prefix must appear (either channel)
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("API error ("),
        "BC-3.9.013: 5xx error must surface 'API error (' prefix; \
         got stderr: {stderr}\nstdout: {stdout}"
    );
}

/// BC-3.9.013: network failure → exit 1; stderr contains `"Could not reach"`.
/// Full literal: `"Could not reach <host> — check your connection"` from
/// `src/error.rs::JrError::NetworkError`.
///
/// RED GATE: `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_9_013_delete_network_error_exit_1() {
    let cache = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();

    // Point at a port that will refuse connections immediately
    let output = jr_cmd_with_xdg("http://127.0.0.1:1", cache.path(), cfg.path())
        .args(["issue", "attachment", "delete", "12345", "--yes"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-3.9.013: network failure must exit 1; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Could not reach"),
        "BC-3.9.013: network error must contain 'Could not reach'; got stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// F5-R1-004: single-AID 404 message parity (metadata-GET path vs. DELETE path)
// ---------------------------------------------------------------------------

/// F5-R1-004: when a single-AID delete encounters a 404, BOTH the interactive path
/// (metadata GET 404, no `--yes`) and the `--yes` path (targeted DELETE 404) must
/// emit the SAME canonical prefix AND the Jira error body. Currently the two paths
/// are asymmetric:
///
/// - `--yes` path:  `delete_attachment_targeted` on 404 returns
///   `"Attachment {id} not found or not accessible.\n{Jira error body}"` ✓
/// - Interactive path:  `get_attachment_metadata` on 404 returns
///   `"Attachment {id} not found or not accessible."` — WITHOUT the error body ✗
///
/// Target behavior: both paths emit the canonical prefix PLUS the Jira error body.
/// The interactive path fix is in `get_attachment_metadata` (~`src/api/jira/attachments.rs`
/// line 211-212): the `..` wildcard discarding `message` must be replaced with
/// `message` to include the error body in the formatted string.
///
/// IMPORTANT: `delete_attachment` (bulk benign-skip, DEC-168) must NOT be changed —
/// its 404→`"not found or already deleted"` benign semantics are load-bearing.
///
/// Sub-case A (interactive, metadata GET 404): RED — body absent from stderr now.
/// Sub-case B (--yes, DELETE 404): GREEN — body already present; regression guard.
#[tokio::test]
async fn test_f5_r1_004_single_aid_404_message_includes_jira_error_body() {
    // A Jira-style 404 error body — the real API returns something like this.
    let jira_404_body = serde_json::json!({
        "errorMessages": ["The attachment with id '77777' does not exist."],
        "errors": {}
    });
    let jira_404_text = serde_json::to_string(&jira_404_body).unwrap();

    // -----------------------------------------------------------------------
    // Sub-case A: interactive path (no --yes).
    //   metadata GET → 404 → error propagated via `get_attachment_metadata`.
    //   Current: stderr contains prefix, NOT the Jira error body.
    //   Target:  stderr contains prefix AND the Jira error body.
    // -----------------------------------------------------------------------
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // Metadata GET returns 404 with a Jira error body.
        Mock::given(method("GET"))
            .and(path("/rest/api/3/attachment/77777"))
            .respond_with(ResponseTemplate::new(404).set_body_json(jira_404_body.clone()))
            .expect(1)
            .mount(&server)
            .await;

        // DELETE must NOT be issued (exits on metadata-GET failure).
        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/77777"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        // Run without --yes in interactive (TTY) mode.
        // JR_STDIN_IS_TTY=1 bypasses the auto-no-input flip so the code reaches
        // the metadata GET (line ~1833 in attachments.rs). When that GET returns
        // 404, the command exits 64 before showing any prompt.
        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .env("JR_STDIN_IS_TTY", "1")
            .args(["issue", "attachment", "delete", "77777"])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "F5-R1-004(A) interactive 404: must exit 64; got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        // Canonical prefix must be present.
        assert!(
            stderr.contains("Attachment 77777 not found or not accessible."),
            "F5-R1-004(A) interactive 404: stderr must contain canonical prefix; \
             got stderr: {stderr}"
        );

        // Jira error body must also be present.
        // RED: current `get_attachment_metadata` discards `message` (the `..` wildcard)
        //      so the error body does NOT appear in the formatted error.
        assert!(
            stderr.contains("does not exist"),
            "F5-R1-004(A) interactive 404: Jira error body must appear in stderr; \
             got stderr: {stderr}"
        );
    }

    // -----------------------------------------------------------------------
    // Sub-case B: --yes path (targeted DELETE 404).
    //   `delete_attachment_targeted` on 404 already includes the error body.
    //   This sub-case is a GREEN regression guard (DEC-168).
    // -----------------------------------------------------------------------
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();

        // DELETE returns 404 with a Jira error body.
        Mock::given(method("DELETE"))
            .and(path("/rest/api/3/attachment/77777"))
            .respond_with(ResponseTemplate::new(404).set_body_json(jira_404_body.clone()))
            .expect(1)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), cfg.path())
            .args(["issue", "attachment", "delete", "77777", "--yes"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(64),
            "F5-R1-004(B) --yes 404: must exit 64; got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        // Canonical prefix.
        assert!(
            stderr.contains("Attachment 77777 not found or not accessible."),
            "F5-R1-004(B) --yes 404: canonical prefix must be present; got stderr: {stderr}"
        );

        // Jira error body (DEC-168: already implemented in delete_attachment_targeted).
        // This assertion is GREEN now — regression guard for the --yes path.
        assert!(
            stderr.contains(&jira_404_text) || stderr.contains("does not exist"),
            "F5-R1-004(B) --yes 404: Jira error body must appear in stderr (DEC-168); \
             got stderr: {stderr}"
        );
    }
}

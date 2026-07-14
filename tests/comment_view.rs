//! CLI-level integration tests for `jr issue comment view`.
//!
//! Red Gate: all 14 tests FAIL because `handle_comment_view` is `todo!()`.
//! Every subprocess exits 101 (Rust panic / todo!() code) instead of the
//! expected exit codes — exit 0 (success), exit 64 (user error), exit 1
//! (serde parse-error path).
//!
//! AC-007 tier (i) lib-unit lives in src/cli/issue/interactions.rs #[cfg(test)],
//! not here. It calls `adf_to_text` directly and is GREEN-throughout
//! (not a Red Gate participant).
//!
//! BC anchors: BC-3.5.010
//! VPs: VP-577-007, VP-577-016, VP-577-021 (7 variants), VP-577-022(c)
//! Story: S-577-6, GitHub issue #577

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helper
// ---------------------------------------------------------------------------

/// Build a `jr` command pointing at the mock server with XDG isolation.
/// Does NOT add `--no-input` or any other defaults — callers supply all flags.
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

/// Minimal ADF body paragraph containing "hello world".
fn adf_hello_world() -> Value {
    serde_json::json!({
        "version": 1,
        "type": "doc",
        "content": [
            {
                "type": "paragraph",
                "content": [
                    {
                        "type": "text",
                        "text": "hello world"
                    }
                ]
            }
        ]
    })
}

// ---------------------------------------------------------------------------
// AC-001 / VP-577-021 variant 1
// BC-3.5.010 — 6 labeled fields + unlabeled body block render completeness
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment view FOO-1 --id 10001` against a full comment
/// fixture exits 0 and stdout contains all 6 labeled field headers in byte order:
/// "ID:", "Author:", "Created:", "Updated:", "JSM internal: Yes", "Restricted: None".
/// No "Body:" label is asserted — the body block is unlabeled per BC-3.5.010.
///
/// Fixture: no `visibility` field → rung (d) → "Restricted: None".
/// `properties[0].value.internal == true` → "JSM internal: Yes".
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_view_human_render_all_seven_fields() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let body = adf_hello_world();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "self": "https://example.atlassian.net/rest/api/3/issue/FOO-1/comment/10001",
            "author": {
                "displayName": "Jane Smith",
                "emailAddress": "jane@example.com",
                "accountId": "abc123"
            },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": body,
            "properties": [
                {
                    "key": "sd.public.comment",
                    "value": { "internal": true }
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 all-fields: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    for label in [
        "ID:",
        "Author:",
        "Created:",
        "Updated:",
        "JSM internal: Yes",
        "Restricted: None",
    ] {
        assert!(
            stdout.contains(label),
            "BC-3.5.010 all-fields: stdout must contain '{label}'; got stdout: {stdout}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-002 / VP-577-021 variant 1
// BC-3.5.010 — Field-5 JSM internal: Yes when sd.public.comment.internal=true
// ---------------------------------------------------------------------------

/// Verify that a comment with `properties[0].key == "sd.public.comment"` and
/// `value.internal == true` → stdout contains "JSM internal: Yes".
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_jsm_internal_yes_when_internal_true() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let body = adf_hello_world();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "author": { "displayName": "Jane Smith" },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": body,
            "properties": [
                {
                    "key": "sd.public.comment",
                    "value": { "internal": true }
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 jsm-yes: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("JSM internal: Yes"),
        "BC-3.5.010 jsm-yes: stdout must contain 'JSM internal: Yes'; got stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-003 / VP-577-021 variant 7
// BC-3.5.010 — Field-5 JSM internal: No when sd.public.comment.internal=false
// ---------------------------------------------------------------------------

/// Verify that a comment with `properties[0].value.internal == false` →
/// stdout contains "JSM internal: No".
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_jsm_internal_no_when_internal_false() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let body = adf_hello_world();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "author": { "displayName": "Jane Smith" },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": body,
            "properties": [
                {
                    "key": "sd.public.comment",
                    "value": { "internal": false }
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 jsm-no: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("JSM internal: No"),
        "BC-3.5.010 jsm-no: stdout must contain 'JSM internal: No'; got stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-004 / VP-577-021 variant 3
// BC-3.5.010 — Field-5 JSM internal: N/A when no properties field
// ---------------------------------------------------------------------------

/// Verify that a comment with no `properties` field (or empty array) →
/// stdout contains "JSM internal: N/A".
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_jsm_internal_na_when_no_properties() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let body = adf_hello_world();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "author": { "displayName": "Jane Smith" },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": body
            // no "properties" field
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 jsm-na: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("JSM internal: N/A"),
        "BC-3.5.010 jsm-na: stdout must contain 'JSM internal: N/A'; got stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-005 / VP-577-021 variants 4, 5, 6
// BC-3.5.010 — Field-6 Restricted 4-rung ladder
// ---------------------------------------------------------------------------

/// Rung (a): visibility.type == "role" or "group" with non-empty value →
/// "Restricted: <value>" (not "id=" prefix, not "<type>:" prefix).
///
/// Fixture: type="role", value="Software Engineers" → "Restricted: Software Engineers".
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_restricted_ladder_rung_a() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let body = adf_hello_world();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "author": { "displayName": "Jane Smith" },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": body,
            "visibility": {
                "type": "role",
                "value": "Software Engineers",
                "identifier": "sr-engineers"
            }
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 rung-a: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("Restricted: Software Engineers"),
        "BC-3.5.010 rung-a: stdout must contain 'Restricted: Software Engineers'; \
         got stdout: {stdout}"
    );
}

/// Rung (b): visibility.type == "role" or "group" with empty/null value, non-empty
/// identifier → "Restricted: id=<identifier>".
///
/// Fixture: type="group", value="", identifier="sr-engineers" →
/// "Restricted: id=sr-engineers".
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_restricted_ladder_rung_b() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let body = adf_hello_world();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "author": { "displayName": "Jane Smith" },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": body,
            "visibility": {
                "type": "group",
                "value": "",
                "identifier": "sr-engineers"
            }
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 rung-b: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("Restricted: id=sr-engineers"),
        "BC-3.5.010 rung-b: stdout must contain 'Restricted: id=sr-engineers'; \
         got stdout: {stdout}"
    );
}

/// Rung (c): visibility.type is non-role/non-group with non-empty value →
/// "Restricted: <type>:<value>".
///
/// Fixture: type="Team", value="AlphaTeam" → "Restricted: Team:AlphaTeam".
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_restricted_ladder_rung_c() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let body = adf_hello_world();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "author": { "displayName": "Jane Smith" },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": body,
            "visibility": {
                "type": "Team",
                "value": "AlphaTeam",
                "identifier": "some-uuid"
            }
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 rung-c: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("Restricted: Team:AlphaTeam"),
        "BC-3.5.010 rung-c: stdout must contain 'Restricted: Team:AlphaTeam'; \
         got stdout: {stdout}"
    );
}

/// Rung (d): no `visibility` field → "Restricted: None".
///
/// Fixture: visibility field absent from response JSON.
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_restricted_ladder_rung_d() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let body = adf_hello_world();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "author": { "displayName": "Jane Smith" },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": body
            // no "visibility" field → rung d → "Restricted: None"
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 rung-d: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("Restricted: None"),
        "BC-3.5.010 rung-d: stdout must contain 'Restricted: None'; got stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-006 / VP-577-021 variant 1
// BC-3.5.010 — Field-7 body rendered via adf_to_text with blank-line separator
// ---------------------------------------------------------------------------

/// Verify that a comment with an ADF body containing "hello world" renders body
/// text after a blank-line separator following the header block, with no
/// "Body:" label emitted.
///
/// Assertion: stdout contains a blank line ("\n\n") followed by "hello world".
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_body_rendered_with_blank_line_separator() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let body = adf_hello_world();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "author": { "displayName": "Jane Smith" },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": body
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 body-sep: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    // The blank-line separator ("\n\n") separates headers from body block.
    assert!(
        stdout.contains("\n\nhello world"),
        "BC-3.5.010 body-sep: stdout must contain blank-line separator before body text; \
         expected '\\n\\nhello world'; got stdout: {stdout}"
    );
    assert!(
        !stdout.contains("Body:"),
        "BC-3.5.010 body-sep: stdout must NOT contain a 'Body:' label; got stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-007 tier (ii) — subprocess
// EC-3.5.010-2(a) — JSON-parse-error boundary behavior
// ---------------------------------------------------------------------------

/// Verify that a wiremock response body with ≥129 levels of JSON nesting causes
/// `serde_json::from_slice` to fail (serde default recursion limit = 128), which
/// propagates as `JrError::Json` → exit 1 at the CLI boundary.
///
/// This test pins the BOUNDARY BEHAVIOR: the `adf_to_text` depth-guard exit-64
/// path is unreachable via the HTTP boundary because serde_json rejects the body
/// before `adf_to_text` is called. See story AC-007 for the full analysis.
/// The unit-level propagation test (tier-i) lives in
/// `src/cli/issue/interactions.rs #[cfg(test)]`.
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101 ≠ 1.
#[tokio::test]
async fn test_bc_3_5_010_ec2a_deep_json_parse_error_exits_1() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Build a 129-deep nested JSON string that serde_json cannot parse
    // (serde_json recursion limit = 128). Constructed as a String to bypass the
    // serde_json::Value recursion limit (construction via json! macro is also
    // bounded, so we build the string directly).
    let mut json_str = "42".to_string(); // leaf value
    for _ in 0..129 {
        json_str = format!("{{\"a\":{json_str}}}");
    }

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(json_str)
                .insert_header("Content-Type", "application/json"),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // The serde_json recursion limit fires first → JrError::Json → exit 1.
    // (NOT exit 64, which would require adf_to_text to be called.)
    assert_eq!(
        output.status.code(),
        Some(1),
        "EC-3.5.010-2 boundary: 129-deep JSON body must cause JrError::Json → exit 1 \
         (NOT exit 64); got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// AC-008
// BC-3.5.010 Response-404 — 404 → exit 64 + two-part body surface
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment view FOO-1 --id 10001` against a wiremock
/// returning 404 exits 64, and stderr contains BOTH:
/// (a) preamble "comment not found or permission denied"
/// (b) Jira body text "Comment not found."
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_404_exits_64_with_body_surface() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["Comment not found."]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.5.010 404: must exit 64; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("comment not found or permission denied"),
        "BC-3.5.010 404: stderr must contain preamble \
         'comment not found or permission denied'; got: {stderr}"
    );
    assert!(
        stderr.contains("Comment not found."),
        "BC-3.5.010 404: stderr must contain Jira body text 'Comment not found.'; \
         got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 / VP-577-016, VP-577-007
// BC-3.5.010 EC-3.5.010-1 — JSON output: serde_json::Value passthrough
// ---------------------------------------------------------------------------

/// Two assertions in one test (VP-577-016 + VP-577-007):
///
/// (a) VP-577-016 — "self" field survives passthrough: `--output json` returns
///     a JSON response that includes the `"self"` URL field (a standard Jira API
///     field absent from any typed `Comment` struct). The `"self"` key must survive
///     in stdout, confirming lossless serde_json::Value passthrough.
///
/// (b) VP-577-007 — `?expand=properties` is in the request URL AND
///     `properties[0].value.internal` == true in the JSON output.
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_ec1_json_output_passthrough() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let body = adf_hello_world();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "self": "https://example.atlassian.net/rest/api/3/issue/FOO-1/comment/10001",
            "author": { "displayName": "Jane Smith" },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": body,
            "properties": [
                {
                    "key": "sd.public.comment",
                    "value": { "internal": true }
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "view", "FOO-1", "--id", "10001", "--output", "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 json-passthrough: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "BC-3.5.010 json-passthrough: stdout must be valid JSON; \
             parse error: {e}\nstdout: {stdout}"
        )
    });

    // VP-577-016: "self" key must survive (lossless Value passthrough)
    assert!(
        parsed.get("self").is_some(),
        "VP-577-016: 'self' field must survive passthrough in JSON output; \
         got JSON: {parsed}"
    );

    // VP-577-007: properties[0].value.internal must be true
    let internal = &parsed["properties"][0]["value"]["internal"];
    assert_eq!(
        *internal,
        Value::Bool(true),
        "VP-577-007: properties[0].value.internal must be true in JSON output; \
         got: {internal}"
    );

    // VP-577-007: URL must contain expand=properties (checked via received requests)
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        1,
        "VP-577-007: expected exactly 1 GET request; got {}",
        requests.len()
    );
    let url_str = requests[0].url.as_str();
    assert!(
        url_str.contains("expand=properties"),
        "VP-577-007: request URL must contain 'expand=properties'; got: {url_str}"
    );
}

// ---------------------------------------------------------------------------
// AC-010 / VP-577-022 variant (c)
// EC-3.5.002-1 — invalid --id charset → exit 64, no GET
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment view FOO-1 --id "../x"` exits 64 with stderr
/// containing "invalid comment id". The GET mock has `.expect(0)` — the
/// validation must fire before any HTTP call.
///
/// Shared validation rule: EC-3.5.002-1 (`validate_comment_id`), shared with
/// delete (VP-577-022a) and edit (VP-577-022b).
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_002_ec1_view_invalid_id_regex_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "../x"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "EC-3.5.002-1 view: must exit 64 on invalid --id charset; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("invalid comment id"),
        "EC-3.5.002-1 view: stderr must contain 'invalid comment id'; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-011 / VP-577-021 variant 2
// BC-3.5.010 — body-absent fallback: empty body block + byte-level stdout pin
// ---------------------------------------------------------------------------

/// Verify that a comment fixture without a `body` field → exit 0; header fields
/// 1–6 render with graceful-degradation fallbacks; body block is empty (blank
/// line after Restricted field, no additional content).
///
/// Byte-level pin (VP-577-021 variant 2): stdout ends with "Restricted: None\n\n"
/// — the structural blank-line separator always renders, leaving nothing after it
/// when body is absent.
///
/// Red Gate: fails because `handle_comment_view` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_010_body_absent_empty_block_stdout_ends_restricted_none() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "author": { "displayName": "Jane Smith" },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000"
            // no "body" field, no "visibility", no "properties"
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 body-absent: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    // Byte-level pin: stdout ends with "Restricted: None\n\n"
    // The blank-line separator always renders, leaving nothing after it when body
    // is absent.
    assert!(
        stdout.ends_with("Restricted: None\n\n"),
        "VP-577-021 variant 2: stdout must end with 'Restricted: None\\n\\n'; \
         got stdout: {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// BC-3.5.010 — fallback tokens for absent/null header fields
// ---------------------------------------------------------------------------

/// Verify graceful-degradation tokens for all four required-but-possibly-absent
/// header fields (BC-3.5.010 spec pins:
///   field 1 ID  → "N/A" when `id` key absent
///   field 2 Author → "Unknown" when `author` is null or `displayName` absent
///   field 3 Created → "N/A" when `created` key absent
///   field 4 Updated → "N/A" when `updated` key absent
///
/// Fixture: `author: null` (anonymized/deleted user — real Jira case);
/// `id`, `created`, `updated` keys omitted.
///
/// Must be RED against code using `unwrap_or("(unknown)")` for all four fields.
#[tokio::test]
async fn test_bc_3_5_010_degraded_fixture_fallback_tokens() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            // id, created, updated keys deliberately absent
            "author": null   // null author — GDPR-anonymized or deleted user
            // no body, no visibility, no properties
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.5.010 degraded: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    // BC-3.5.010 pin: field 1 absent id → "N/A"
    assert!(
        stdout.contains("ID: N/A"),
        "BC-3.5.010 degraded: stdout must contain 'ID: N/A' when id key absent; \
         got stdout: {stdout}"
    );
    // BC-3.5.010 pin: field 2 null author → "Unknown"
    assert!(
        stdout.contains("Author: Unknown"),
        "BC-3.5.010 degraded: stdout must contain 'Author: Unknown' when author is null; \
         got stdout: {stdout}"
    );
    // BC-3.5.010 pin: field 3 absent created → "N/A"
    assert!(
        stdout.contains("Created: N/A"),
        "BC-3.5.010 degraded: stdout must contain 'Created: N/A' when created key absent; \
         got stdout: {stdout}"
    );
    // BC-3.5.010 pin: field 4 absent updated → "N/A"
    assert!(
        stdout.contains("Updated: N/A"),
        "BC-3.5.010 degraded: stdout must contain 'Updated: N/A' when updated key absent; \
         got stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Mutation-kill — non-404/403 API error propagates as exit 1
// BC-3.5.010 — kills two guard mutations in handle_comment_view:
//   mutant #1: replace `*status == 404 || *status == 403` with true
//              → ANY ApiError would get exit 64 + preamble; 500 would match
//   mutant #2: replace `== 403` with `!= 403`
//              → 500 matches `500 != 403` = true → exit 64
// With the correct guard, 500 is neither 404 nor 403, so the error propagates
// as-is through `Err(e)`, which JrError maps to exit 1 (ApiError exit code).
// ---------------------------------------------------------------------------

/// Verify that a 500 Internal Server Error from the GET endpoint exits 1
/// (not 64) and does NOT emit the "comment not found or permission denied"
/// preamble.
#[tokio::test]
async fn test_bc_3_5_010_view_500_exits_1_not_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "view", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-3.5.010 500-guard: 500 error must exit 1 (not 64); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("comment not found or permission denied"),
        "BC-3.5.010 500-guard: 500 error must NOT emit the 404/403 preamble; got: {stderr}"
    );
}

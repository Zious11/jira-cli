//! CLI-level integration tests for `jr issue attachment upload --public/--internal`
//! JSM visibility path (S-576-5).
//!
//! RED GATE: All 16 tests fail because the S-576-3 AC-017 interim rejection guard is
//! still active in `handle_attachment_upload`.  When `--public` or `--internal` is
//! passed, the handler exits 64 with `"--public and --internal are not yet
//! supported. …"` instead of the expected behavior.  This is valid RED evidence.
//!
//! Several tests also reference `resolve_service_desk_id` and
//! `invalidate_project_meta_cache` stubs whose bodies are `todo!()`, causing
//! exit 101 (Rust panic) when reached — also RED.
//!
//! After Task 5 (remove interim guard + wire JSM flow), all tests become GREEN.
//!
//! BC anchors: BC-3.9.003, BC-3.9.004, BC-3.9.005, BC-3.9.006, BC-3.9.007,
//!             BC-3.9.011, BC-3.9.020 (EC-3.9.020-7/8), BC-X.8.010
//! VPs: VP-576-005 (combined-gate single-prompt pin)
//! Security: SEC-576-006 (stale-ID self-heal)
//! Story: S-576-5, GitHub issue #576

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helper (mirrors attachment_upload.rs)
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

/// Software project response (projectTypeKey != "service_desk").
fn non_jsm_project_response(key: &str) -> Value {
    serde_json::json!({
        "id": "10001",
        "key": key,
        "name": "Development",
        "projectTypeKey": "software",
        "simplified": false
    })
}

/// JSM project response (projectTypeKey == "service_desk").
fn jsm_project_response(key: &str, project_id: &str) -> Value {
    serde_json::json!({
        "id": project_id,
        "key": key,
        "name": "Help Desk",
        "projectTypeKey": "service_desk",
        "simplified": false
    })
}

/// Service desk list page with one entry whose projectId matches `project_id`.
fn service_desk_list_response(sd_id: &str, project_id: &str) -> Value {
    serde_json::json!({
        "size": 1,
        "start": 0,
        "limit": 50,
        "isLastPage": true,
        "values": [
            {
                "id": sd_id,
                "projectId": project_id,
                "projectName": "Help Desk"
            }
        ]
    })
}

/// Service desk list page with NO entries (zero project_id matches — EC-9).
fn service_desk_list_empty() -> Value {
    serde_json::json!({
        "size": 0,
        "start": 0,
        "limit": 50,
        "isLastPage": true,
        "values": []
    })
}

/// Wire-format issue GET response (Step 0).
fn issue_get_response(key: &str, project_key: &str) -> Value {
    serde_json::json!({
        "id": "10100",
        "key": key,
        "fields": {
            "summary": "Test issue",
            "project": {
                "key": project_key,
                "id": "10001"
            },
            "status": {
                "name": "Open",
                "statusCategory": {"key": "new"}
            }
        }
    })
}

/// Raw attachment object as returned by the servicedeskapi AttachmentDTO (JSM-specific shape).
///
/// Confirmed by P2-3c schema probe run 29940792930 (S-576-5):
/// - `created` is an OBJECT with an `iso8601` sub-key (NOT a bare string).
/// - NO top-level `id` — the attachment ID is derived from `_links.jiraRest` URL tail.
/// - Content URL lives at `_links.content` (NOT a top-level `content` field).
/// - `author` is a full `UserDTO` object (more fields than the curated `{accountId, displayName}`).
fn attachment_object(id: &str, filename: &str) -> Value {
    serde_json::json!({
        "filename": filename,
        "author": {
            "accountId": "user123",
            "displayName": "Test User",
            "timeZone": "UTC",
            "accountType": "atlassian"
        },
        "created": {
            "iso8601": "2026-07-20T00:00:00.000+0000",
            "jira": "20/Jul/26 12:00 AM",
            "friendly": "Jul 20, 2026",
            "epochMillis": 1753056000000_u64
        },
        "size": 1024_u64,
        "mimeType": "text/plain",
        "_links": {
            "jiraRest": format!("https://example.atlassian.net/rest/api/3/attachment/{id}"),
            "content": format!("https://example.atlassian.net/rest/api/3/attachment/content/{id}"),
            "self": "https://example.atlassian.net/rest/servicedeskapi/request/EJ-1/attachment"
        }
    })
}

/// Raw attachment object in the PLATFORM shape (`POST /rest/api/3/issue/{key}/attachments`).
///
/// The platform upload endpoint returns a bare array of platform AttachmentObjects — NOT the
/// servicedeskapi AttachmentDTO shape.  Use this helper for mocking `/rest/api/3/issue/…/attachments`
/// responses (non-JSM and OQ-9 no-op paths).  For servicedeskapi step-2 mocks use `attachment_object`.
fn platform_attachment_object(id: &str, filename: &str) -> Value {
    serde_json::json!({
        "id": id,
        "filename": filename,
        "self": format!("https://example.atlassian.net/rest/api/3/attachment/{id}"),
        "content": format!("https://example.atlassian.net/rest/api/3/attachment/content/{id}"),
        "created": "2026-07-20T00:00:00.000+0000",
        "size": 1024_u64,
        "mimeType": "text/plain",
        "author": {
            "accountId": "user123",
            "displayName": "Test User",
            "self": "https://example.atlassian.net/rest/api/3/user?accountId=user123",
            "avatarUrls": {},
            "accountType": "atlassian"
        }
    })
}

/// Curated JSON form of `attachment_object` for assertion.
fn attachment_curated(id: &str, filename: &str) -> Value {
    serde_json::json!({
        "author": {"accountId": "user123", "displayName": "Test User"},
        "contentUrl": format!("https://example.atlassian.net/rest/api/3/attachment/content/{id}"),
        "created": "2026-07-20T00:00:00.000+0000",
        "filename": filename,
        "id": id,
        "mimeType": "text/plain",
        "size": 1024_u64
    })
}

/// Platform attachment list response used by --replace-existing.
fn attachments_list_response(key: &str, attachments: Vec<Value>) -> Value {
    serde_json::json!({
        "key": key,
        "fields": {"attachment": attachments}
    })
}

/// Step-2 `POST .../request/{key}/attachment` response (AttachmentCreateResultDTO).
///
/// Confirmed by P2-3c schema probe run 29936980027 (S-576-5): the real Jira
/// API returns an OBJECT, not a bare array:
///   `{"comment": null, "attachments": {"size": N, "start": 0, "limit": 50,
///     "isLastPage": true, "values": [...]}}`.
fn attachment_create_result_dto(attachments: Vec<Value>) -> Value {
    let size = attachments.len();
    serde_json::json!({
        "comment": null,
        "attachments": {
            "size": size,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": attachments
        }
    })
}

// ---------------------------------------------------------------------------
// AC-001: test_bc_3_9_003_public_on_non_jsm_exits_64_before_gate
//
// EC-3.9.003-7: non-JSM guard fires BEFORE non-interactive gate.
// Expected: exit 64 + stderr contains BC-3.9.005 canonical message.
//
// RED evidence: interim guard fires first with wrong message
// ("--public and --internal are not yet supported. …"), and the project meta
// fetch never happens. The test asserts the BC-3.9.005 canonical message,
// so stderr won't match → assertion fail → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_003_public_on_non_jsm_exits_64_before_gate() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"pdf content").unwrap();

    // P1-004: issue key lookup (GET …?fields=project) must resolve before project meta check.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/DEV-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("DEV-1", "DEV")))
        .mount(&server)
        .await;
    // Mount: GET project meta → non-JSM project.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(non_jsm_project_response("DEV")))
        .mount(&server)
        .await;

    // No attachment upload endpoint should ever be called.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "DEV-1",
            &file.to_string_lossy(),
            "--public",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.9.003 AC-001: --public on non-JSM must exit 64; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("--public is only supported on Jira Service Management (JSM) issues."),
        "BC-3.9.005 AC-001: stderr must contain canonical BC-3.9.005 message; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 test 1: test_bc_3_9_003_public_gate_confirm_proceeds
//
// `jr issue attachment upload <JSM-KEY> <FILE> --public` (interactive, confirm "y"):
// After gate confirm, should complete the two-step servicedeskapi flow.
//
// RED evidence: interim guard exits 64 before reaching the gate → exit 64 ≠ 0 → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_003_public_gate_confirm_proceeds() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("attach.txt");
    std::fs::write(&file, b"hello").unwrap();

    // Step 0: issue GET.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-1", "EJ")))
        .mount(&server)
        .await;

    // Project meta fetch for JSM detection + sdId resolution.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Step 1: attachTemporaryFile.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/servicedesk/42/attachTemporaryFile"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-abc-001", "fileName": "attach.txt"}]
        })))
        .mount(&server)
        .await;

    // Step 2: post_request_attachment.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-1/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20001", "attach.txt"),
            ])),
        )
        .mount(&server)
        .await;

    // Interactive confirm: provide "y\n" on stdin, set JR_STDIN_IS_TTY=1 so no-input is suppressed.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-1",
            &file.to_string_lossy(),
            "--public",
        ])
        .write_stdin("y\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.003 AC-002: confirm 'y' → exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// AC-002 test 2: test_bc_3_9_003_public_gate_cancel_exits_0
//
// `jr issue attachment upload <JSM-KEY> <FILE> --public` (interactive, cancel Enter):
// Gate cancel → exit 0 + stderr "Upload cancelled." + JSON {"cancelled":true,"uploaded":false}.
//
// RED evidence: interim guard exits 64 → exit 64 ≠ 0 → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_003_public_gate_cancel_exits_0() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("attach.txt");
    std::fs::write(&file, b"hello").unwrap();

    // JSM project meta setup.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-1", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Interactive cancel: empty Enter.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-1",
            &file.to_string_lossy(),
            "--public",
            "--output",
            "json",
        ])
        .write_stdin("\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.003 AC-002 cancel: empty-Enter → exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Upload cancelled."),
        "BC-3.9.003 AC-002 cancel: stderr must contain 'Upload cancelled.'; got: {stderr}"
    );
    let json: Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON on cancel");
    assert_eq!(
        json.get("cancelled"),
        Some(&Value::Bool(true)),
        "BC-3.9.003 AC-002 cancel: JSON must have cancelled:true; got: {json}"
    );
    assert_eq!(
        json.get("uploaded"),
        Some(&Value::Bool(false)),
        "BC-3.9.003 AC-002 cancel: JSON must have uploaded:false; got: {json}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 test 3 / AC-009: test_vp_576_005_combined_gate_single_prompt_fires_once
//
// `jr issue attachment upload <JSM-KEY> <FILE> --public --replace-existing`:
// VP-576-005: ONE combined prompt (not two). The test asserts that exactly one
// read_line prompt fires — verified by asserting the prompt text appears exactly
// once in stderr output.
//
// RED evidence: interim guard fires with a different error → exit 64 → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_vp_576_005_combined_gate_single_prompt_fires_once() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("attach.txt");
    std::fs::write(&file, b"hello").unwrap();

    // JSM project meta for JSM detection.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-1", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Existing attachment with same name for --replace-existing.
    // .with_priority(1) is REQUIRED: the plain issue-GET mock (priority 5, registered first,
    // same path) would otherwise win under FIFO ordering, returning issue_get_response with
    // no `attachment` field → empty list → consumer-1 fires instead of consumer-3 (false-GREEN).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ-1",
                vec![platform_attachment_object("99001", "attach.txt")],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // DELETE mock for the replace-existing path (VP-576-003 ordering: DELETE before POST).
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/99001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // Step-1: attachTemporaryFile.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/servicedesk/42/attachTemporaryFile"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-vp005", "fileName": "attach.txt"}]
        })))
        .mount(&server)
        .await;

    // Step-2: post_request_attachment with public:true.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-1/attachment"))
        .and(body_partial_json(serde_json::json!({"public": true})))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20099", "attach.txt"),
            ])),
        )
        .mount(&server)
        .await;

    // Interactive: supply "y\n" — should proceed once the combined gate fires.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-1",
            &file.to_string_lossy(),
            "--public",
            "--replace-existing",
        ])
        .write_stdin("y\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "VP-576-005: combined gate confirm 'y' must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // VP-576-005: exactly ONE confirmation prompt fires for the combined gate.
    // The consumer-1 gate prompt ends with "? [y/N] "; consumer-3 also ends with
    // "Continue? [y/N] ".  Both contain "[y/N]" exactly once — ensuring neither gate
    // fires twice, and the two gates don't both fire.
    let yn_count = stderr.matches("[y/N]").count();
    assert!(
        yn_count == 1,
        "VP-576-005: combined gate must fire exactly once (not zero or two prompts); \
         '[y/N]' appeared {yn_count} times in stderr:\n{stderr}"
    );

    // Consumer-3 combined-gate text must appear.
    assert!(
        stderr.contains("and replace existing attachment(s):"),
        "VP-576-005: consumer-3 combined gate text must appear in stderr; got: {stderr}"
    );

    // Consumer-1 text must NOT appear (consumer-3 fires instead).
    assert!(
        !stderr.contains("as customer-visible (public)?"),
        "VP-576-005: consumer-1 text must NOT appear when consumer-3 fires; got: {stderr}"
    );

    // P5-001 VP-576-003 ordering pin: all DELETEs must precede first attachTemporaryFile POST.
    let reqs = server.received_requests().await.unwrap();
    let last_delete_idx = reqs
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.method == wiremock::http::Method::DELETE
                && r.url.path().starts_with("/rest/api/3/attachment/")
        })
        .map(|(i, _)| i)
        .next_back()
        .expect("VP-576-003: at least one DELETE must have been received");
    let first_step1_idx = reqs
        .iter()
        .enumerate()
        .find(|(_, r)| {
            r.method == wiremock::http::Method::POST && r.url.path().contains("attachTemporaryFile")
        })
        .map(|(i, _)| i)
        .expect("VP-576-003: at least one step-1 POST must have been received");
    assert!(
        last_delete_idx < first_step1_idx,
        "VP-576-003 ordering: all DELETEs must precede first attachTemporaryFile POST; \
         last DELETE at {last_delete_idx}, first step-1 at {first_step1_idx}"
    );
}

// ---------------------------------------------------------------------------
// AC-003: test_bc_3_9_003_two_step_attach_temporary_then_request_attachment
//
// BC-3.9.003 wire sequence: Step 0 GET issue → Step 1 POST attachTemporaryFile
// per file → Step 2 POST request/{key}/attachment with all temp IDs.
//
// RED evidence: interim guard exits 64 → no servicedeskapi calls ever made → RED
// (wiremock expects (0) calls to attachTemporaryFile, but assertion on exit code fails).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_003_two_step_attach_temporary_then_request_attachment() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("upload.txt");
    std::fs::write(&file, b"data").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-2", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Step 1: attachTemporaryFile must be called with X-Atlassian-Token: no-check.
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-001", "fileName": "upload.txt"}]
        })))
        .expect(1) // Exactly one step-1 call per file (EC-3.9.003-3)
        .mount(&server)
        .await;

    // Step 2: post_request_attachment with public:true.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-2/attachment"))
        .and(body_partial_json(serde_json::json!({"public": true})))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20002", "upload.txt"),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-2",
            &file.to_string_lossy(),
            "--public",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.003 AC-003: two-step flow must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// AC-004: test_bc_x_8_010_service_desk_id_from_project_meta_projectid_match
//
// BC-X.8.010: sdId resolved via ServiceDesk.project_id (String) equality.
// Tests that a service desk whose projectId matches the issue's numeric project.id
// is found and used (NOT projectKey matching).
//
// RED evidence: todo!() stub in resolve_service_desk_id → test panics (exit 101).
// Actually for the subprocess test, interim guard exits 64 first → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_x_8_010_service_desk_id_from_project_meta_projectid_match() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("test.txt");
    std::fs::write(&file, b"x").unwrap();

    // The JSM project has numeric id "10099" but key "EJ" — these are different.
    // sdId resolution MUST match on project_id (numeric string "10099"), NOT on key.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-3", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    // Service desk list: only the entry with projectId "10099" matches.
    // There is also a decoy with projectId "EJ" (the key string — must NOT match).
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "99", "projectId": "EJ", "projectName": "Decoy (key, not id)" },
                { "id": "42", "projectId": "10099", "projectName": "Help Desk (correct)" }
            ]
        })))
        .mount(&server)
        .await;

    // Step 1 MUST use sdId "42" (not "99").
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-x", "fileName": "test.txt"}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-3/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20003", "test.txt"),
            ])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-3",
            &file.to_string_lossy(),
            "--public",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-X.8.010 AC-004: sdId from project_id match → exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// AC-005: test_sec_576_006_stale_id_self_heal_invalidate_retry_once
//
// SEC-576-006: step-1 404/403 with cached sdId → invalidate + retry ONCE.
// Exactly 2 step-1 attempts must fire (NOT 3 — single retry only).
//
// RED evidence: interim guard exits 64 → never reaches step-1 → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_sec_576_006_stale_id_self_heal_invalidate_retry_once() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("stale.txt");
    std::fs::write(&file, b"stale").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-4"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-4", "EJ")))
        .mount(&server)
        .await;

    // SEC-576-006: pre-populate the project_meta cache with stale sdId "OLD-SD" so that
    // `get_or_fetch_project_meta` returns "OLD-SD" on first call (cache hit, no API GET),
    // triggering the stale-heal path. The fresh fetch (after invalidation) uses the mocks
    // mounted below. fetched_at is set to a future date so the entry is within the 7-day TTL.
    {
        let profile_dir = cache.path().join("v1").join("default");
        std::fs::create_dir_all(&profile_dir).unwrap();
        let stale_meta = serde_json::json!({
            "EJ": {
                "project_type": "service_desk",
                "simplified": false,
                "project_id": "10099",
                "service_desk_id": "OLD-SD",
                "fetched_at": "2099-01-01T00:00:00Z"
            }
        });
        std::fs::write(
            profile_dir.join("project_meta.json"),
            serde_json::to_string_pretty(&stale_meta).unwrap(),
        )
        .unwrap();
    }

    // First project meta fetch returns stale sdId "OLD-SD" (served from cache above).
    // After invalidation, a fresh fetch returns correct sdId "42".
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Step-1 attempt 1: with stale sdId → 404 (triggers self-heal).
    // Step-1 attempt 2: with fresh sdId "42" → 200 (succeeds).
    // Wiremock: first POST to stale sdId returns 404; POST to real sdId returns 200.
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/OLD-SD/attachTemporaryFile",
        ))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessage": "Service desk not found"
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/servicedesk/42/attachTemporaryFile"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-stale-healed", "fileName": "stale.txt"}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-4/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20004", "stale.txt"),
            ])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-4",
            &file.to_string_lossy(),
            "--public",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "SEC-576-006 AC-005: self-heal retry → exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // AC-005 constraint: exactly 2 step-1 attempts (NOT 3).
    let received = server.received_requests().await.unwrap();
    let step1_calls: Vec<_> = received
        .iter()
        .filter(|r| r.url.path().contains("attachTemporaryFile"))
        .collect();
    assert_eq!(
        step1_calls.len(),
        2,
        "SEC-576-006 AC-005: exactly 2 step-1 attempts required (first with stale sdId, second with fresh sdId); got {}",
        step1_calls.len()
    );
}

// ---------------------------------------------------------------------------
// AC-006 test 1: test_bc_3_9_004_internal_on_jsm_two_step
//
// `jr issue attachment upload <JSM-KEY> <FILE> --internal`:
// JSM issue → two-step flow with public:false (no gate needed).
//
// RED evidence: interim guard exits 64 → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_004_internal_on_jsm_two_step() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("internal.txt");
    std::fs::write(&file, b"internal").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-5", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/servicedesk/42/attachTemporaryFile"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-int-001", "fileName": "internal.txt"}]
        })))
        .mount(&server)
        .await;

    // Step 2 must send public:false.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-5/attachment"))
        .and(body_partial_json(serde_json::json!({"public": false})))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20005", "internal.txt"),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-5",
            &file.to_string_lossy(),
            "--internal",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.004 AC-006: --internal on JSM → exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// AC-006 test 2: test_bc_3_9_004_internal_on_non_jsm_silent_noop_oq9
//
// OQ-9 ruling: `--internal` on non-JSM = silent no-op.
// Platform POST only; zero servicedeskapi calls; no warning emitted.
//
// RED evidence: interim guard exits 64 → exit 64 ≠ 0 → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_004_internal_on_non_jsm_silent_noop_oq9() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("nonjsm.txt");
    std::fs::write(&file, b"nonjsm").unwrap();

    // P1-004: issue key lookup (GET …?fields=project) must resolve before project meta check.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/DEV-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("DEV-1", "DEV")))
        .mount(&server)
        .await;
    // Non-JSM project (software).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(non_jsm_project_response("DEV")))
        .mount(&server)
        .await;

    // Platform POST — returns a successful upload (platform DTO shape, not JSM DTO).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            platform_attachment_object("20006", "nonjsm.txt")
        ])))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "DEV-1",
            &file.to_string_lossy(),
            "--internal",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // OQ-9: must exit 0 with NO servicedeskapi calls and NO warning about --internal.
    assert_eq!(
        output.status.code(),
        Some(0),
        "OQ-9 AC-006: --internal on non-JSM → silent no-op, exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("--internal"),
        "OQ-9 AC-006: stderr must NOT mention --internal on non-JSM; got: {stderr}"
    );
    let received = server.received_requests().await.unwrap();
    let sda_calls: Vec<_> = received
        .iter()
        .filter(|r| r.url.path().contains("servicedeskapi"))
        .filter(|r| {
            r.url.path().contains("attachTemporaryFile") || r.url.path().contains("attachment")
        })
        .collect();
    assert!(
        sda_calls.is_empty(),
        "OQ-9 AC-006: zero servicedeskapi attachment calls on non-JSM; got {} calls",
        sda_calls.len()
    );
}

// ---------------------------------------------------------------------------
// AC-007: test_bc_3_9_007_servicedeskapi_response_shape
//
// BC-3.9.007: `--public --output json` returns curated attachment array.
// Minimum confirmed shape (P2-3c deferred): bare curated array (same as S3).
//
// RED evidence: interim guard exits 64 → no JSON output → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_007_servicedeskapi_response_shape() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("shape.txt");
    std::fs::write(&file, b"shape").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-6"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-6", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/servicedesk/42/attachTemporaryFile"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-shape-001", "fileName": "shape.txt"}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-6/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20007", "shape.txt"),
            ])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-6",
            &file.to_string_lossy(),
            "--public",
            "--yes",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.007 AC-007: --public --output json → exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );

    let arr: Vec<Value> = serde_json::from_str(&stdout)
        .expect("BC-3.9.007: --public --output json stdout must be a JSON array");
    assert!(
        !arr.is_empty(),
        "BC-3.9.007: response array must be non-empty"
    );
    let item = &arr[0];
    // Curated shape: must have these keys; must NOT have "self".
    for key in &[
        "author",
        "contentUrl",
        "created",
        "filename",
        "id",
        "mimeType",
        "size",
    ] {
        assert!(
            item.get(key).is_some(),
            "BC-3.9.007: curated shape missing key '{key}'; got: {item}"
        );
    }
    assert!(
        item.get("self").is_none(),
        "BC-3.9.007: curated shape must NOT contain 'self' key; got: {item}"
    );
}

// ---------------------------------------------------------------------------
// AC-008: test_bc_3_9_006_jsm_upload_error_taxonomy
//
// BC-3.9.003 + BC-3.9.005 + BC-3.9.006: full error taxonomy.
// Each row is an explicit sub-assertion.
//
// RED evidence: most sub-assertions fail because interim guard exits 64 with
// the wrong error message. Only the clap mutual-exclusion case (--public +
// --internal) fires at the clap layer and exits 2 correctly.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_006_jsm_upload_error_taxonomy() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("tax.txt");
    std::fs::write(&file, b"tax").unwrap();

    // --- Sub-assertion 0: issue 404 → exit 64 + "not found or not accessible" (P1-004) ---
    // get_issue_project_key fires FIRST; a 404 from Jira surfaces as exit 64 with the
    // canonical message before any project-meta or gate logic runs.
    {
        // No mock for GHOST-1 → wiremock 404 for unmatched requests.
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "GHOST-1",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(64),
            "taxonomy sub-0: issue 404 → exit 64; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("GHOST-1 not found or not accessible"),
            "taxonomy sub-0: issue 404 → 'not found or not accessible'; got: {stderr}"
        );
    }

    // --- Sub-assertion 1: --public on non-JSM → exit 64 + BC-3.9.005 message ---
    // P1-004: issue key lookup fires first; DEV-1 exists with project "DEV".
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/DEV-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("DEV-1", "DEV")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(non_jsm_project_response("DEV")))
        .mount(&server)
        .await;

    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "DEV-1",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(64),
            "taxonomy: --public on non-JSM → exit 64; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("--public is only supported on Jira Service Management (JSM) issues."),
            "taxonomy: --public on non-JSM → BC-3.9.005 message; got: {stderr}"
        );
    }

    // --- Sub-assertion 2: non-interactive + --public without --yes → exit 64 + "Use --yes" ---
    // JSM project mocks for sub-assertions 2–10 (permanent, shared).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-10", "EJ")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;
    // Returns EJ, EJSA, and the dedicated SD IDs for sub-assertions 13/14 (P3-005).
    // Sub-13 uses EJSC (pid "70002") → SD7-403 (permanent 403 response).
    // Sub-14 uses EJSD (pid "70003") → SD7-401 (permanent 401 response).
    // Each uses its own SD path to avoid scoped-mock ordering races.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 4,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                {"id": "42",      "projectId": "10099", "projectName": "EJ JSM"},
                {"id": "SD7-NEW", "projectId": "70001", "projectName": "JSM SA7"},
                {"id": "SD7-403", "projectId": "70002", "projectName": "JSM SC (403)"},
                {"id": "SD7-401", "projectId": "70003", "projectName": "JSM SD (401)"}
            ]
        })))
        .mount(&server)
        .await;

    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            // Non-interactive: no JR_STDIN_IS_TTY, no --yes.
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-10",
                &file.to_string_lossy(),
                "--public",
                "--no-input",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(64),
            "taxonomy: non-interactive + --public without --yes → exit 64; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("Use --yes to confirm uploading"),
            "taxonomy: non-interactive + --public without --yes → 'Use --yes to confirm uploading'; got: {stderr}"
        );
    }

    // --- Sub-assertion 3: non-interactive + --public + --replace-existing (≥1 match) without --yes ---
    {
        // Attachment list: existing "tax.txt" — matches upload file "tax.txt" → ≥1 match.
        // Uses platform shape (top-level `id`/`self`/`content`) for correct AttachmentObject deserialization.
        // `.with_priority(1)` is REQUIRED: the plain EJ-10 issue GET mock (priority 5, registered
        // before this mock) would otherwise win under same-priority FIFO ordering, returning
        // `issue_get_response` with no `attachment` field → empty list → consumer 1 hint (wrong).
        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/EJ-10"))
            .and(wiremock::matchers::query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(attachments_list_response(
                    "EJ-10",
                    vec![platform_attachment_object("99010", "tax.txt")],
                )),
            )
            .with_priority(1)
            .mount(&server)
            .await;

        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-10",
                &file.to_string_lossy(),
                "--public",
                "--replace-existing",
                "--no-input",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(64),
            "taxonomy: non-interactive + --public + --replace-existing without --yes → exit 64; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(stderr.contains("Use --yes to confirm uploading as customer-visible (public) and deleting existing same-filename attachments."),
            "taxonomy: non-interactive combined path → exact message; got: {stderr}");
    }

    // --- Sub-assertion 4: gate cancel (interactive, non-EOF) → exit 0 + "Upload cancelled." ---
    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .env("JR_STDIN_IS_TTY", "1")
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-10",
                &file.to_string_lossy(),
                "--public",
                "--output",
                "json",
            ])
            .write_stdin("\n")
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(
            out.status.code(),
            Some(0),
            "taxonomy: gate cancel → exit 0; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("Upload cancelled."),
            "taxonomy: gate cancel → 'Upload cancelled.' in stderr; got: {stderr}"
        );
        let json: Value =
            serde_json::from_str(&stdout).expect("taxonomy: cancel JSON must be valid");
        assert_eq!(
            json.get("cancelled"),
            Some(&Value::Bool(true)),
            "taxonomy: gate cancel JSON must have cancelled:true; got: {json}"
        );
        assert_eq!(
            json.get("uploaded"),
            Some(&Value::Bool(false)),
            "taxonomy: gate cancel JSON must have uploaded:false; got: {json}"
        );
    }

    // --- Sub-assertion 5: gate EOF / IO error → exit 130 ---
    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .env("JR_STDIN_IS_TTY", "1")
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-10",
                &file.to_string_lossy(),
                "--public",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap(); // stdin closed = EOF
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(130),
            "taxonomy: gate EOF → exit 130; got {:?}; stderr: {stderr}",
            out.status.code()
        );
    }

    // --- Sub-assertions 6–10: step-2 error taxonomy ---
    // wiremock 0.6 uses FIFO ordering (first-registered mock wins for equal-priority
    // mocks on the same path).  To prevent sub-assertion 6's 400 mock from shadowing
    // sub-assertions 8/9/10's 401/403/500 mocks, each step-2 mock is mounted with
    // `mount_as_scoped` so it is removed before the next sub-assertion adds its mock.
    // The step-1 (attachTemporaryFile) mock is permanent and reused across all four
    // sub-assertions.

    // Step-1 permanent mock — shared by sub-assertions 6, 8, 9, 10.
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-err", "fileName": "tax.txt"}]
        })))
        .mount(&server)
        .await;

    // --- Sub-assertion 6: step-2 failure 4xx (excl 401/403) → exit 64 + retry hint ---
    {
        let _step2_guard = Mock::given(method("POST"))
            .and(path("/rest/servicedeskapi/request/EJ-10/attachment"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "errorMessage": "Bad request"
            })))
            .mount_as_scoped(&server)
            .await;
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-10",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        // BC-3.9.006 EC-3.9.006-1: step-2 4xx → exit 64 + retry hint
        assert_eq!(
            out.status.code(),
            Some(64),
            "taxonomy: step-2 400 → exit 64; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("Temporary attachment IDs may have expired. Try the upload again."),
            "taxonomy: step-2 400 → retry hint; got: {stderr}"
        );
    } // _step2_guard drops → 400 mock removed before sub-assertion 7

    // --- Permanent mocks for sub-assertion 7 (EJSA stale-heal scenario) ---
    // These use unique paths not shared with EJ sub-assertions; no scoped overrides needed.

    // P1-004: issue key lookup for EJSA-1 → project "EJSA".
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJSA-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_get_response("EJSA-1", "EJSA")),
        )
        .mount(&server)
        .await;

    // Project GET for EJSA re-fetch after stale-heal invalidation.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJSA"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "70001",
            "key": "EJSA",
            "name": "JSM SA7",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .mount(&server)
        .await;

    // Step-1 on stale sd "SD7-OLD" → 404 (triggers stale-heal).
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/SD7-OLD/attachTemporaryFile",
        ))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    // Step-1 retry on fresh sd "SD7-NEW" → 404 (EC-4 mapping: exit 64 + "not found after refresh").
    // This is what P1-001 must catch: before fix bare .await? gives exit 1, after fix gives exit 64.
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/SD7-NEW/attachTemporaryFile",
        ))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    // --- Permanent mocks for sub-assertions 13/14 (P3-005) ---
    // Each uses a distinct project + SD chain to avoid scoped-mock ordering races.

    // Sub-13: EJSC (pid "70002") chain — stale SD8-OLD → 404, fresh SD7-403 → 403.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJSC-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_get_response("EJSC-1", "EJSC")),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJSC"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "70002",
            "key": "EJSC",
            "name": "JSM SC",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .mount(&server)
        .await;
    // Step-1 on stale sd "SD8-OLD" → 404 (triggers stale-heal for sub-13).
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/SD8-OLD/attachTemporaryFile",
        ))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    // Step-1 retry on fresh sd "SD7-403" → 403 (permanent; post-retry 403 path).
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/SD7-403/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    // Sub-14: EJSD (pid "70003") chain — stale SD9-OLD → 404, fresh SD7-401 → 401.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJSD-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_get_response("EJSD-1", "EJSD")),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJSD"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "70003",
            "key": "EJSD",
            "name": "JSM SD",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .mount(&server)
        .await;
    // Step-1 on stale sd "SD9-OLD" → 404 (triggers stale-heal for sub-14).
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/SD9-OLD/attachTemporaryFile",
        ))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    // Step-1 retry on fresh sd "SD7-401" → 401 (permanent; post-retry 401 path).
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/SD7-401/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    // --- Sub-assertion 7: post-retry 404 after stale-heal → exit 64 + "not found after refresh" ---
    // P1-001/P1-002: the stale-heal retry branch ends with bare `.await?` which propagates
    // ApiError{status:404} → exit 1. After P1-001 fix, it maps to exit 64 + "not found after refresh".
    // RED evidence: bare .await? gives exit 1 (not 64) → assertion fails → RED.
    // Design: EJSA (pid "70001") uses permanent mocks only — no scoped priority overrides needed.
    // The permanent servicedesk list returns SD7-NEW for pid "70001" so service_desk_id = Some("SD7-NEW")
    // after re-fetch, ensuring the P1-001 retry code path is exercised (not the None branch).
    {
        // Pre-populate cache with stale sd "SD7-OLD" for project "EJSA" (fetched_at far-future → fresh).
        let profile_dir = cache.path().join("v1").join("default");
        std::fs::create_dir_all(&profile_dir).unwrap();
        let cache_path = profile_dir.join("project_meta.json");
        let mut existing: serde_json::Value = if cache_path.exists() {
            std::fs::read_to_string(&cache_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        existing["EJSA"] = serde_json::json!({
            "project_type": "service_desk",
            "simplified": false,
            "project_id": "70001",
            "service_desk_id": "SD7-OLD",
            "fetched_at": "2099-01-01T00:00:00Z"
        });
        std::fs::write(
            &cache_path,
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJSA-1",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(64),
            "taxonomy sub-7: post-retry 404 after stale-heal → exit 64; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("not found after refresh"),
            "taxonomy sub-7: post-retry 404 after stale-heal → 'not found after refresh'; got: {stderr}"
        );
    }

    // --- Sub-assertion 8: step-2 failure 401 → exit 2 ---
    {
        let _step2_guard = Mock::given(method("POST"))
            .and(path("/rest/servicedeskapi/request/EJ-10/attachment"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "errorMessage": "Unauthorized"
            })))
            .mount_as_scoped(&server)
            .await;
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-10",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(2),
            "taxonomy: step-2 401 → exit 2; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("Temporary attachment IDs may have expired. Try the upload again."),
            "taxonomy: step-2 401 → retry hint present; got: {stderr}"
        );
    } // _step2_guard drops → 401 mock removed before sub-assertion 9

    // --- Sub-assertion 9: step-2 failure 403 → exit 1 ---
    {
        let _step2_guard = Mock::given(method("POST"))
            .and(path("/rest/servicedeskapi/request/EJ-10/attachment"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "errorMessage": "Forbidden"
            })))
            .mount_as_scoped(&server)
            .await;
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-10",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(1),
            "taxonomy: step-2 403 → exit 1; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("Temporary attachment IDs may have expired. Try the upload again."),
            "taxonomy: step-2 403 → retry hint present; got: {stderr}"
        );
        assert!(
            stderr.contains("Forbidden"),
            "taxonomy sub-9: step-2 403 Jira error body text must reach stderr; \
             AC-008 'Jira error body + retry hint'; got: {stderr}"
        );
    } // _step2_guard drops → 403 mock removed before sub-assertion 10

    // --- Sub-assertion 10: step-2 failure 5xx → exit 1 + retry hint ---
    {
        let _step2_guard = Mock::given(method("POST"))
            .and(path("/rest/servicedeskapi/request/EJ-10/attachment"))
            .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
                "errorMessage": "Internal server error"
            })))
            .mount_as_scoped(&server)
            .await;
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-10",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(1),
            "taxonomy: step-2 500 → exit 1; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("Temporary attachment IDs may have expired. Try the upload again."),
            "taxonomy: step-2 500 → retry hint; got: {stderr}"
        );
    } // _step2_guard drops

    // --- Sub-assertion 11: --public + --internal together → exit 2 (clap mutual-exclusion) ---
    // This fires at the clap layer, before any handler code, so it works even with the interim guard.
    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-10",
                &file.to_string_lossy(),
                "--public",
                "--internal",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(2),
            "taxonomy: --public + --internal → clap exit 2; got {:?}; stderr: {stderr}",
            out.status.code()
        );
    }

    // --- Sub-assertion 12: step-1 first-occurrence 401 → exit 2 + auth hint (P2-003) ---
    //
    // BC-3.9.012: all codes other than 403/404 map on FIRST occurrence (no stale-heal).
    // 401 → `JrError::NotAuthenticated` → exit 2 + "Not authenticated" + "jr auth login".
    // This is symmetric with the step-2 401 path (sub-assertion 8) and the platform
    // upload 401 path.
    //
    // RED evidence: before P2-003 fix, `attach_temporary_file` falls into the generic
    // `!is_success()` arm → `JrError::ApiError { status: 401 }` → exit 1 (not 2).
    {
        // Scoped step-1 mock returning 401 — priority 1 overrides the permanent 200 mock.
        let _step1_401_guard = Mock::given(method("POST"))
            .and(path(
                "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
            ))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "errorMessages": ["User not authenticated."]
            })))
            .with_priority(1) // overrides permanent 200 mock (same path + headers)
            .mount_as_scoped(&server)
            .await;

        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-10",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(2),
            "taxonomy sub-12: step-1 first-occurrence 401 → exit 2; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("Not authenticated") || stderr.contains("jr auth login"),
            "taxonomy sub-12: step-1 401 → NotAuthenticated hint in stderr; got: {stderr}"
        );
    } // _step1_401_guard drops → permanent 200 mock restored

    // --- Sub-assertion 13: post-retry 403 after stale-heal → exit 1 (P3-005) ---
    // After P3-004 (dead arm removed), a 403 from attach_temporary_file is
    // JrError::ApiError { status: 403 } → falls through Ok(other) → exit 1.
    //
    // Uses EJSC (pid "70002") with its own stale SD8-OLD → 404 → stale-heal → SD7-403 → 403.
    // Permanent mocks avoid scoped-mock ordering races (DeadMock race on SD7-NEW).
    {
        // Pre-populate cache with stale sd "SD8-OLD" for project "EJSC".
        let profile_dir = cache.path().join("v1").join("default");
        std::fs::create_dir_all(&profile_dir).unwrap();
        let cache_path = profile_dir.join("project_meta.json");
        let mut existing: serde_json::Value = if cache_path.exists() {
            std::fs::read_to_string(&cache_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        existing["EJSC"] = serde_json::json!({
            "project_type": "service_desk",
            "simplified": false,
            "project_id": "70002",
            "service_desk_id": "SD8-OLD",
            "fetched_at": "2099-01-01T00:00:00Z"
        });
        std::fs::write(
            &cache_path,
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJSC-1",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(1),
            "taxonomy sub-13: post-retry 403 after stale-heal → exit 1; got {:?}; stderr: {stderr}",
            out.status.code()
        );
    }

    // --- Sub-assertion 14: post-retry 401 after stale-heal → exit 2 (P3-005) ---
    // attach_temporary_file maps 401 → JrError::NotAuthenticated (P2-003 / BC-3.9.012).
    // After P3-004 (dead ApiError{status:401} arm removed), NotAuthenticated falls through
    // to Ok(other) → propagates as exit 2.
    //
    // Uses EJSD (pid "70003") with its own stale SD9-OLD → 404 → stale-heal → SD7-401 → 401.
    // Permanent mocks avoid scoped-mock ordering races (DeadMock race on SD7-NEW).
    {
        // Pre-populate cache with stale sd "SD9-OLD" for project "EJSD".
        let profile_dir = cache.path().join("v1").join("default");
        std::fs::create_dir_all(&profile_dir).unwrap();
        let cache_path = profile_dir.join("project_meta.json");
        let mut existing: serde_json::Value = if cache_path.exists() {
            std::fs::read_to_string(&cache_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        existing["EJSD"] = serde_json::json!({
            "project_type": "service_desk",
            "simplified": false,
            "project_id": "70003",
            "service_desk_id": "SD9-OLD",
            "fetched_at": "2099-01-01T00:00:00Z"
        });
        std::fs::write(
            &cache_path,
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJSD-1",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(2),
            "taxonomy sub-14: post-retry 401 after stale-heal → exit 2; got {:?}; stderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("Not authenticated") || stderr.contains("jr auth login"),
            "taxonomy sub-14: post-retry 401 → auth hint in stderr; got: {stderr}"
        );
    }

    // --- Sub-assertion 15: post-retry 5xx after stale-heal → exit 1 (P4-005) ---
    //
    // Stale-heal fires (step-1 returns 404), cache invalidated, fresh sdId resolved,
    // retried step-1 returns 500 → propagates as JrError::ApiError → exit 1.
    //
    // Uses EJSE (pid "70004") with its own stale SD10-OLD → 404 → stale-heal →
    // SD10-500 → 500. Scoped priority-1 service desk list override adds EJSE's entry
    // for the duration of this sub-assertion.
    {
        // EJSE issue meta mock (permanent)
        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/EJSE-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(issue_get_response("EJSE-1", "EJSE")),
            )
            .mount(&server)
            .await;
        // EJSE project meta mock (permanent; serves initial + post-invalidation re-fetch)
        Mock::given(method("GET"))
            .and(path("/rest/api/3/project/EJSE"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "70004",
                "key": "EJSE",
                "name": "JSM SE",
                "projectTypeKey": "service_desk",
                "simplified": false
            })))
            .mount(&server)
            .await;
        // Step-1 on stale sd "SD10-OLD" → 404 (triggers stale-heal for sub-15)
        Mock::given(method("POST"))
            .and(path(
                "/rest/servicedeskapi/servicedesk/SD10-OLD/attachTemporaryFile",
            ))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        // Step-1 retry on fresh sd "SD10-500" → 500 (post-retry 5xx path)
        Mock::given(method("POST"))
            .and(path(
                "/rest/servicedeskapi/servicedesk/SD10-500/attachTemporaryFile",
            ))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        // Scoped service desk list override: adds EJSE entry (pid "70004" → "SD10-500").
        // Priority 1 overrides the permanent mock (default priority 5).
        let _sd_list_guard = Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/servicedesk"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "size": 5,
                "start": 0,
                "limit": 50,
                "isLastPage": true,
                "values": [
                    {"id": "42",       "projectId": "10099", "projectName": "EJ JSM"},
                    {"id": "SD7-NEW",  "projectId": "70001", "projectName": "JSM SA7"},
                    {"id": "SD7-403",  "projectId": "70002", "projectName": "JSM SC (403)"},
                    {"id": "SD7-401",  "projectId": "70003", "projectName": "JSM SD (401)"},
                    {"id": "SD10-500", "projectId": "70004", "projectName": "JSM SE (500)"}
                ]
            })))
            .with_priority(1)
            .mount_as_scoped(&server)
            .await;

        // Pre-populate cache with stale sd "SD10-OLD" for project "EJSE".
        let profile_dir = cache.path().join("v1").join("default");
        std::fs::create_dir_all(&profile_dir).unwrap();
        let cache_path = profile_dir.join("project_meta.json");
        let mut existing: serde_json::Value = if cache_path.exists() {
            std::fs::read_to_string(&cache_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        existing["EJSE"] = serde_json::json!({
            "project_type": "service_desk",
            "simplified": false,
            "project_id": "70004",
            "service_desk_id": "SD10-OLD",
            "fetched_at": "2099-01-01T00:00:00Z"
        });
        std::fs::write(
            &cache_path,
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJSE-1",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(1),
            "taxonomy sub-15: post-retry 5xx after stale-heal → exit 1; got {:?}; stderr: {stderr}",
            out.status.code()
        );
    }

    // --- Sub-assertion 16: first-attempt step-1 500 → exit 1, exactly ONE step-1 POST (P5-005) ---
    //
    // 500 is NOT a stale-heal trigger (only 404/403 are). A first-attempt 500 must
    // propagate immediately as exit 1 without any retry or stale-heal. Exactly ONE
    // step-1 POST must be observed.
    //
    // Kill-test mutation: add `|| 500` to the stale-heal guard at the 404/403 check →
    // stale-heal fires → cache invalidated → re-fetch project meta → SD list miss →
    // exit 64 (not exit 1) → exit-code assertion fires.
    //
    // Uses pre-populated cache (service_desk_id = "SD11-500") so no SD list fetch
    // is needed on the fresh-fetch path. Avoids scoped-mock priority races.
    {
        // EJSF issue GET mock (step 0 — issue key resolution).
        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/EJSF-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(issue_get_response("EJSF-1", "EJSF")),
            )
            .mount(&server)
            .await;
        // EJSF project GET mock (needed only if stale-heal fires — defensive).
        Mock::given(method("GET"))
            .and(path("/rest/api/3/project/EJSF"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "80001",
                "key": "EJSF",
                "name": "JSM SF",
                "projectTypeKey": "service_desk",
                "simplified": false
            })))
            .mount(&server)
            .await;
        // Step-1 on sd "SD11-500" → 500 (no stale-heal retry expected).
        Mock::given(method("POST"))
            .and(path(
                "/rest/servicedeskapi/servicedesk/SD11-500/attachTemporaryFile",
            ))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        // Pre-populate EJSF project meta with service_desk_id = "SD11-500" (future TTL).
        // This avoids an SD list fetch and establishes the SD ID needed by step-1.
        let profile_dir = cache.path().join("v1").join("default");
        std::fs::create_dir_all(&profile_dir).unwrap();
        let cache_path_p16 = profile_dir.join("project_meta.json");
        let mut existing_p16: serde_json::Value = if cache_path_p16.exists() {
            std::fs::read_to_string(&cache_path_p16)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        existing_p16["EJSF"] = serde_json::json!({
            "project_type": "service_desk",
            "simplified": false,
            "project_id": "80001",
            "service_desk_id": "SD11-500",
            "fetched_at": "2099-01-01T00:00:00Z"
        });
        std::fs::write(
            &cache_path_p16,
            serde_json::to_string_pretty(&existing_p16).unwrap(),
        )
        .unwrap();

        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJSF-1",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(1),
            "taxonomy sub-16: first-attempt step-1 500 → exit 1; got {:?}; stderr: {stderr}",
            out.status.code()
        );

        // Assert exactly ONE step-1 POST (no stale-heal retry on 500).
        let reqs = server.received_requests().await.unwrap();
        let step1_count = reqs
            .iter()
            .filter(|r| {
                r.method == wiremock::http::Method::POST
                    && r.url.path().contains("SD11-500/attachTemporaryFile")
            })
            .count();
        assert_eq!(
            step1_count, 1,
            "taxonomy sub-16: first-attempt 500 must make exactly ONE step-1 POST (no stale-heal); \
             got {step1_count}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-010: test_bc_x_8_010_jsm_determination_triggers_project_meta_fetch
//
// JSM determination triggers get_or_fetch_project_meta (exactly one fetch if cache miss).
// Non-JSM --public guard fires AFTER the project meta fetch (not before).
//
// RED evidence: interim guard exits 64 before project meta is fetched → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_x_8_010_jsm_determination_triggers_project_meta_fetch() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("det.txt");
    std::fs::write(&file, b"det").unwrap();

    // Mount project GET with .expect() to verify it is called.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/SOFT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10002",
            "key": "SOFT",
            "name": "Software",
            "projectTypeKey": "software",
            "simplified": false
        })))
        .expect(1) // Must be called exactly once for JSM determination.
        .mount(&server)
        .await;

    // Mount issue GET.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/SOFT-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_get_response("SOFT-1", "SOFT")),
        )
        .mount(&server)
        .await;

    // --public on non-JSM: must still fetch project meta to determine JSM status.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "SOFT-1",
            &file.to_string_lossy(),
            "--public",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-X.8.010 AC-010: --public on non-JSM → exit 64; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // The wiremock .expect(1) guard will fail the test if the project meta fetch did not happen.
    // (Verified by MockServer::verify_and_reset — implicit on drop with expect()).
}

// ---------------------------------------------------------------------------
// AC-012: test_bc_3_9_011_public_json_output_shape
//
// BC-3.9.011: `--public --yes --output json` returns bare curated array.
// Keys must be BTreeMap-alphabetical; "self" absent; "contentUrl" present.
//
// RED evidence: interim guard exits 64 → no JSON on stdout → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_011_public_json_output_shape() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("shape.txt");
    std::fs::write(&file, b"shape").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-7", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-012", "fileName": "shape.txt"}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-7/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20012", "shape.txt"),
            ])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-7",
            &file.to_string_lossy(),
            "--public",
            "--yes",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.011 AC-012: --public --output json → exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );

    let arr: Vec<Value> =
        serde_json::from_str(&stdout).expect("BC-3.9.011 AC-012: stdout must be a JSON array");
    assert!(
        !arr.is_empty(),
        "BC-3.9.011 AC-012: array must be non-empty"
    );

    let item = &arr[0];
    // Curated shape: {author, contentUrl, created, filename, id, mimeType, size}
    for key in &[
        "author",
        "contentUrl",
        "created",
        "filename",
        "id",
        "mimeType",
        "size",
    ] {
        assert!(
            item.get(key).is_some(),
            "BC-3.9.011 AC-012: missing key '{key}'; got: {item}"
        );
    }
    assert!(
        item.get("self").is_none(),
        "BC-3.9.011 AC-012: 'self' must be absent; got: {item}"
    );
    // BC-2.7.002 v1.3.95 curated author form: exactly {accountId, displayName}.
    let author = item.get("author").expect("author present");
    if !author.is_null() {
        assert!(
            author.get("accountId").is_some(),
            "BC-3.9.011 AC-012: author.accountId required; got: {author}"
        );
        assert!(
            author.get("displayName").is_some(),
            "BC-3.9.011 AC-012: author.displayName required; got: {author}"
        );
        assert!(
            author.get("self").is_none(),
            "BC-3.9.011 AC-012: author.self must be stripped; got: {author}"
        );
        assert!(
            author.get("avatarUrls").is_none(),
            "BC-3.9.011 AC-012: author.avatarUrls must be stripped; got: {author}"
        );
    }

    // Minimum confirmed shape matches attachment_curated fixture.
    let expected = attachment_curated("20012", "shape.txt");
    assert_eq!(
        item, &expected,
        "BC-3.9.011 AC-012: curated shape mismatch; expected: {expected}\ngot: {item}"
    );
}

// ---------------------------------------------------------------------------
// AC-013: test_bc_3_9_011_internal_json_shape_jsm_vs_non_jsm_asymmetry
//
// BC-3.9.011 OQ-9 asymmetry:
// - JSM: bare curated array (from servicedeskapi two-step)
// - non-JSM: bare curated array (platform POST, NO "public" key, NO envelope)
// The JSON output shape is identical in both cases; no "public" field anywhere.
//
// RED evidence: interim guard exits 64 → no JSON output in either path → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_011_internal_json_shape_jsm_vs_non_jsm_asymmetry() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("asym.txt");
    std::fs::write(&file, b"asym").unwrap();

    // --- JSM path ---
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-8"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-8", "EJ")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/servicedesk/42/attachTemporaryFile"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-013-jsm", "fileName": "asym.txt"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-8/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20013", "asym.txt"),
            ])),
        )
        .mount(&server)
        .await;

    let jsm_out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-8",
            &file.to_string_lossy(),
            "--internal",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let jsm_stderr = String::from_utf8_lossy(&jsm_out.stderr);
    let jsm_stdout = String::from_utf8_lossy(&jsm_out.stdout);
    assert_eq!(
        jsm_out.status.code(),
        Some(0),
        "BC-3.9.011 AC-013: --internal JSM → exit 0; got {:?}\nstdout: {jsm_stdout}\nstderr: {jsm_stderr}",
        jsm_out.status.code()
    );

    let jsm_arr: Vec<Value> = serde_json::from_str(&jsm_stdout)
        .expect("BC-3.9.011 AC-013: JSM --internal json must be array");
    assert!(
        !jsm_arr.is_empty(),
        "BC-3.9.011 AC-013: JSM --internal array non-empty"
    );
    // No "public" key anywhere in the output (bare curated array, no envelope).
    assert!(
        serde_json::to_string(&jsm_arr)
            .unwrap()
            .find("\"public\"")
            .is_none(),
        "BC-3.9.011 AC-013: JSM --internal output must NOT contain 'public' key; got: {jsm_stdout}"
    );

    // --- non-JSM path ---
    // P1-004: issue key lookup fires before project meta check.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/DEV-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("DEV-2", "DEV")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(non_jsm_project_response("DEV")))
        .mount(&server)
        .await;
    // Platform POST for non-JSM path — uses platform DTO shape (NOT JSM DTO).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-2/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            platform_attachment_object("20013b", "asym.txt")
        ])))
        .mount(&server)
        .await;

    let nonjsm_out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "DEV-2",
            &file.to_string_lossy(),
            "--internal",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let nonjsm_stderr = String::from_utf8_lossy(&nonjsm_out.stderr);
    let nonjsm_stdout = String::from_utf8_lossy(&nonjsm_out.stdout);
    assert_eq!(
        nonjsm_out.status.code(),
        Some(0),
        "BC-3.9.011 AC-013: --internal non-JSM (OQ-9) → exit 0; got {:?}\nstdout: {nonjsm_stdout}\nstderr: {nonjsm_stderr}",
        nonjsm_out.status.code()
    );

    let nonjsm_arr: Vec<Value> = serde_json::from_str(&nonjsm_stdout)
        .expect("BC-3.9.011 AC-013: non-JSM --internal json must be array");
    // No "public" key (no envelope).
    assert!(
        serde_json::to_string(&nonjsm_arr)
            .unwrap()
            .find("\"public\"")
            .is_none(),
        "BC-3.9.011 AC-013: non-JSM --internal output must NOT contain 'public' key; got: {nonjsm_stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-015: test_bc_3_9_020_dry_run_public_visibility_annotation
//
// BC-3.9.020 EC-3.9.020-7: `--dry-run --public` annotates wouldUpload entries.
// JSON mode: {"dryRun":true,"wouldDelete":[...],"wouldUpload":[{"filename":"…","visibility":"public"}]}
// Human mode: [public] annotation present on each file line.
// Sub-assertion: non-JSM key --replace-existing --dry-run --public → exit 64 (EC-3.9.020-8).
//
// RED evidence:
// - interim guard fires before dry_run → exit 64 with wrong message for JSM path → RED.
// - EC-3.9.020-8 sub-assertion: exit 64 is correct code, but message is wrong → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_020_dry_run_public_visibility_annotation() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("dry.txt");
    std::fs::write(&file, b"dry").unwrap();

    // Primary path: JSM key with --replace-existing --dry-run --public.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-15"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-15", "EJ")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;
    // --replace-existing requires list attachments.
    // .with_priority(1) ensures this mock wins over the plain EJ-15 issue GET mock
    // registered above (same path, same default priority → FIFO, first-registered wins).
    // platform_attachment_object provides the top-level `id` field required for wouldDelete.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-15"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ-15",
                vec![platform_attachment_object("AID-01", "dry.txt")],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // --- Primary assertion: JSON mode ---
    let out_json = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-15",
            &file.to_string_lossy(),
            "--replace-existing",
            "--dry-run",
            "--public",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();
    let json_stderr = String::from_utf8_lossy(&out_json.stderr);
    let json_stdout = String::from_utf8_lossy(&out_json.stdout);
    assert_eq!(
        out_json.status.code(),
        Some(0),
        "BC-3.9.020 AC-015: --dry-run --public JSON → exit 0; got {:?}\nstdout: {json_stdout}\nstderr: {json_stderr}",
        out_json.status.code()
    );

    let preview: Value = serde_json::from_str(&json_stdout)
        .expect("BC-3.9.020 AC-015: dry-run JSON must be valid object");
    // Pinned shape: {"dryRun":true,"wouldDelete":[{…}],"wouldUpload":[{"filename":"dry.txt","visibility":"public"}]}
    assert_eq!(
        preview.get("dryRun"),
        Some(&Value::Bool(true)),
        "BC-3.9.020 AC-015: dryRun must be true; got: {preview}"
    );
    let would_upload = preview
        .get("wouldUpload")
        .and_then(Value::as_array)
        .expect("BC-3.9.020 AC-015: wouldUpload must be array");
    assert!(
        !would_upload.is_empty(),
        "BC-3.9.020 AC-015: wouldUpload must be non-empty"
    );
    for entry in would_upload {
        assert_eq!(
            entry.get("visibility").and_then(Value::as_str),
            Some("public"),
            "BC-3.9.020 EC-3.9.020-7: each wouldUpload entry must have visibility:public; got: {entry}"
        );
    }
    let would_delete = preview
        .get("wouldDelete")
        .and_then(Value::as_array)
        .expect("BC-3.9.020 AC-015: wouldDelete must be array");
    assert!(
        !would_delete.is_empty(),
        "BC-3.9.020 AC-015: wouldDelete must be non-empty (attachment-list mock was shadowed if empty)"
    );
    assert_eq!(
        would_delete[0].get("filename").and_then(Value::as_str),
        Some("dry.txt"),
        "BC-3.9.020 AC-015: wouldDelete[0].filename must be 'dry.txt'; got: {:?}",
        would_delete[0]
    );
    assert_eq!(
        would_delete[0].get("id").and_then(Value::as_str),
        Some("AID-01"),
        "BC-3.9.020 AC-015: wouldDelete[0].id must be 'AID-01'; got: {:?}",
        would_delete[0]
    );

    // --- Primary assertion: human mode ---
    let out_human = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-15",
            &file.to_string_lossy(),
            "--replace-existing",
            "--dry-run",
            "--public",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();
    let human_stdout = String::from_utf8_lossy(&out_human.stdout);
    let human_stderr = String::from_utf8_lossy(&out_human.stderr);
    assert_eq!(
        out_human.status.code(),
        Some(0),
        "BC-3.9.020 AC-015: --dry-run --public human → exit 0; got {:?}\nstdout: {human_stdout}\nstderr: {human_stderr}",
        out_human.status.code()
    );
    // BC-3.9.020 EC-3.9.020-7: [public] annotation must appear in output.
    let human_combined = format!("{human_stdout}{human_stderr}");
    assert!(
        human_combined.contains("[public]"),
        "BC-3.9.020 EC-3.9.020-7: [public] annotation must appear in human output; stdout: {human_stdout}\nstderr: {human_stderr}"
    );

    // --- Sub-assertion: EC-3.9.020-8 — non-JSM key + --dry-run --public → exit 64 + BC-3.9.005 message ---
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(non_jsm_project_response("DEV")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/DEV-3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("DEV-3", "DEV")))
        .mount(&server)
        .await;

    // List endpoint for --replace-existing (must not be called before guard fires).
    // (Not mounted — zero calls expected.)

    let out_nonjsm = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "DEV-3",
            &file.to_string_lossy(),
            "--replace-existing",
            "--dry-run",
            "--public",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();
    let nonjsm_stderr = String::from_utf8_lossy(&out_nonjsm.stderr);
    assert_eq!(
        out_nonjsm.status.code(),
        Some(64),
        "EC-3.9.020-8: non-JSM --dry-run --public → exit 64; got {:?}\nstderr: {nonjsm_stderr}",
        out_nonjsm.status.code()
    );
    assert!(
        nonjsm_stderr
            .contains("--public is only supported on Jira Service Management (JSM) issues."),
        "EC-3.9.020-8: non-JSM --dry-run --public → BC-3.9.005 message; got: {nonjsm_stderr}"
    );
    // EC-3.9.020-8: no list GET must have been issued for the non-JSM path.
    let received = server.received_requests().await.unwrap();
    let list_gets: Vec<_> = received
        .iter()
        .filter(|r| {
            r.url.path().contains("DEV-3")
                && r.method == wiremock::http::Method::GET
                && r.url.query().is_some_and(|q| q.contains("attachment"))
        })
        .collect();
    assert!(
        list_gets.is_empty(),
        "EC-3.9.020-8: zero list GETs for non-JSM dry-run --public (guard fires before preview); got {}",
        list_gets.len()
    );

    // --- JSON-mode sub-assertion: non-JSM --dry-run --public --output json → exit 64 + stderr message + stdout EMPTY ---
    let out_nonjsm_json = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "DEV-3",
            &file.to_string_lossy(),
            "--replace-existing",
            "--dry-run",
            "--public",
            "--yes",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();
    let nonjsm_json_stderr = String::from_utf8_lossy(&out_nonjsm_json.stderr);
    let nonjsm_json_stdout = String::from_utf8_lossy(&out_nonjsm_json.stdout);
    assert_eq!(
        out_nonjsm_json.status.code(),
        Some(64),
        "EC-3.9.020-8 JSON mode: exit 64; got {:?}",
        out_nonjsm_json.status.code()
    );
    assert!(
        nonjsm_json_stderr
            .contains("--public is only supported on Jira Service Management (JSM) issues."),
        "EC-3.9.020-8 JSON mode: BC-3.9.005 message on stderr; got: {nonjsm_json_stderr}"
    );
    assert!(
        nonjsm_json_stdout.trim().is_empty(),
        "EC-3.9.020-8 JSON mode: stdout must be EMPTY (render_json NOT called); got: {nonjsm_json_stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-016: test_ec_x_8_010_1_no_matching_service_desk_exits_64
//
// EC-X.8.010-1: zero projectId matches → exit 64 pre-step-1.
// Canonical error string (exact): "No JSM service desk found for project <KEY>.
//   The project may still be provisioning; verify with `jr queue list --project <KEY>`."
// No stale-heal; both modes; stdout EMPTY in JSON mode.
//
// RED evidence: interim guard exits 64 with wrong message → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_ec_x_8_010_1_no_matching_service_desk_exits_64() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("nomatch.txt");
    std::fs::write(&file, b"nomatch").unwrap();

    // JSM project but service desk list returns HTTP 200 with NO matching projectId.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-16"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-16", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    // Service desk list: returns HTTP 200 with NO entry whose projectId == "10099".
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(service_desk_list_empty()))
        .mount(&server)
        .await;

    // Human mode.
    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-16",
                &file.to_string_lossy(),
                "--public",
                "--yes",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(64),
            "EC-X.8.010-1: zero matches → exit 64; got {:?}\nstderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("No JSM service desk found for project EJ."),
            "EC-X.8.010-1: canonical error message; got: {stderr}"
        );
        assert!(
            stderr.contains("The project may still be provisioning"),
            "EC-X.8.010-1: provisioning hint; got: {stderr}"
        );
        assert!(
            stderr.contains("jr queue list --project EJ"),
            "EC-X.8.010-1: jr queue list hint; got: {stderr}"
        );
    }

    // JSON mode: same exit 64 + same message on stderr + stdout EMPTY.
    {
        let out = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "EJ-16",
                &file.to_string_lossy(),
                "--public",
                "--yes",
                "--output",
                "json",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(
            out.status.code(),
            Some(64),
            "EC-X.8.010-1 JSON mode: exit 64; got {:?}\nstderr: {stderr}",
            out.status.code()
        );
        assert!(
            stderr.contains("No JSM service desk found for project EJ."),
            "EC-X.8.010-1 JSON mode: canonical message on stderr; got: {stderr}"
        );
        assert!(
            stdout.trim().is_empty(),
            "EC-X.8.010-1 JSON mode: stdout must be EMPTY (render_json NOT called); got: {stdout}"
        );
    }
}

// ---------------------------------------------------------------------------
// P1-006-a: test_bc_3_9_014_consumer1_n_le_3_lists_filenames
//
// BC-3.9.014 EC-3.9.014-5 (N ≤ 3): prompt must list individual filenames —
// `"Upload <filename1>, ..., <filenameN> to <KEY> as customer-visible (public)? [y/N] "`
//
// RED evidence: current prompt is `"Continue? Upload 1 file(s) to EJ-17 as customer-visible? [y/N] "`.
// Assertion: stderr contains "Upload upload_one.txt to EJ-17 as customer-visible (public)?"
// → fails because "(public)" and leading "Upload" (not "Continue?") are missing.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_014_consumer1_n_le_3_lists_filenames() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("upload_one.txt");
    std::fs::write(&file, b"hello").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-17"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-17", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Cancel: send "n\n" to avoid needing upload mocks.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-17",
            &file.to_string_lossy(),
            "--public",
        ])
        .write_stdin("n\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // BC-3.9.014 EC-3.9.014-5: N=1 prompt must list the filename, not just the count.
    // Must match: "Upload upload_one.txt to EJ-17 as customer-visible (public)? [y/N] "
    assert!(
        stderr.contains("Upload upload_one.txt to EJ-17 as customer-visible (public)?"),
        "BC-3.9.014 EC-3.9.014-5: N=1 prompt must list filename and include '(public)'; got:\n{stderr}"
    );

    // Cancel → exit 0.
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.014 EC-3.9.014-5: cancel 'n' → exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// P1-006-b: test_bc_3_9_014_consumer1_n_gt_3_shows_count
//
// BC-3.9.014 EC-3.9.014-5 (N > 3): prompt must use count summary —
// `"Upload <N> files to <KEY> as customer-visible (public)? [y/N] "`
//
// RED evidence: current prompt is `"Continue? Upload 4 file(s) to EJ-18 as customer-visible? [y/N] "`.
// Assertion: stderr contains "Upload 4 files to EJ-18 as customer-visible (public)?"
// → fails because "(public)" and "4 files" (not "4 file(s)") and leading "Upload" are missing.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_014_consumer1_n_gt_3_shows_count() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();

    // Create 4 files with distinct names.
    let f1 = tmp.path().join("alpha.txt");
    let f2 = tmp.path().join("beta.txt");
    let f3 = tmp.path().join("gamma.txt");
    let f4 = tmp.path().join("delta.txt");
    for f in [&f1, &f2, &f3, &f4] {
        std::fs::write(f, b"data").unwrap();
    }

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-18"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-18", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Cancel: send "n\n" to avoid needing upload mocks.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-18",
            &f1.to_string_lossy(),
            &f2.to_string_lossy(),
            &f3.to_string_lossy(),
            &f4.to_string_lossy(),
            "--public",
        ])
        .write_stdin("n\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // BC-3.9.014 EC-3.9.014-5: N=4 prompt must use count summary with "(public)".
    // Must match: "Upload 4 files to EJ-18 as customer-visible (public)? [y/N] "
    assert!(
        stderr.contains("Upload 4 files to EJ-18 as customer-visible (public)?"),
        "BC-3.9.014 EC-3.9.014-5: N=4 prompt must show count and include '(public)'; got:\n{stderr}"
    );

    // Cancel → exit 0.
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.014 EC-3.9.014-5: cancel 'n' → exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// P1-006-c: test_bc_3_9_014_consumer3_combined_prompt_text
//
// BC-3.9.014 consumer 3 (--public + ≥1 match): prompt must start with
// `"Upload to <KEY> as customer-visible (public) and replace existing attachment(s):"`
// followed by each `"  <filename> (id: <AID>)"` entry, then `"Continue? [y/N] "`.
//
// RED evidence: current prompt starts with "Replace existing attachment(s) and upload as
// customer-visible on EJ-19:" → fails the assertion below.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_014_consumer3_combined_prompt_text() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("combo.txt");
    std::fs::write(&file, b"hello").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-19"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-19", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Platform attachment list: one existing "combo.txt" with id "77001".
    // Uses the platform shape (top-level `id` + `self` + `content`) for correct deserialization.
    // `.with_priority(1)` is REQUIRED: the plain issue GET mock (priority 5) would otherwise
    // win because it was registered first (same-priority = FIFO in wiremock 0.6).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-19"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ-19",
                vec![platform_attachment_object("77001", "combo.txt")],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // Cancel: send "n\n" to avoid needing servicedeskapi upload mocks.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-19",
            &file.to_string_lossy(),
            "--public",
            "--replace-existing",
        ])
        .write_stdin("n\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // BC-3.9.014 consumer 3: prompt must begin with "Upload to KEY as customer-visible (public)
    // and replace existing attachment(s):".
    assert!(
        stderr.contains(
            "Upload to EJ-19 as customer-visible (public) and replace existing attachment(s):"
        ),
        "BC-3.9.014 consumer3: combined prompt must start with correct text; got:\n{stderr}"
    );

    // The attachment entry must appear in the prompt.
    assert!(
        stderr.contains("combo.txt (id: 77001)"),
        "BC-3.9.014 consumer3: attachment entry 'combo.txt (id: 77001)' must appear in prompt; got:\n{stderr}"
    );

    // Must end with "Continue? [y/N]".
    assert!(
        stderr.contains("Continue? [y/N]"),
        "BC-3.9.014 consumer3: prompt must end with 'Continue? [y/N]'; got:\n{stderr}"
    );

    // Cancel → exit 0.
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.014 consumer3: cancel 'n' → exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// P1-007: test_bc_3_9_014_noinput_replace_no_match_uses_consumer1_hint
//
// BC-3.9.014 P17-004: in non-interactive mode, consumer choice is symmetric
// with the interactive path.  When --replace-existing has ZERO filename matches,
// the hint MUST be consumer 1 ("Use --yes to confirm uploading N file(s) …"),
// NOT consumer 3 ("Use --yes to confirm uploading as customer-visible (public)
// and deleting …").
//
// RED evidence: current code uses consumer 3 hint whenever `replace_existing` is
// set, regardless of actual matches.  The assertion below checks for the consumer 1
// hint string → fails because the current message is the consumer 3 string.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_014_noinput_replace_no_match_uses_consumer1_hint() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    // File named "nomatch.txt"; the attachment list will contain no "nomatch.txt".
    let file = tmp.path().join("nomatch.txt");
    std::fs::write(&file, b"content").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-20", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Attachment list: existing "other.txt" — no filename match with "nomatch.txt".
    // .with_priority(1) so this mock wins over the plain EJ-20 GET mock (same priority = FIFO,
    // plain GET registered first; priority 1 overrides that ordering).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-20"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ-20",
                vec![platform_attachment_object("88001", "other.txt")],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // Non-interactive: --no-input flag + --replace-existing (no match).
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-20",
            &file.to_string_lossy(),
            "--public",
            "--replace-existing",
            "--no-input",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // BC-3.9.014 P17-004: 0 filename matches → consumer 1 hint, NOT consumer 3 hint.
    assert_eq!(
        output.status.code(),
        Some(64),
        "P1-007: 0 matches non-interactive → exit 64; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Use --yes to confirm uploading 1 file(s) to EJ-20 as customer-visible, or run interactively."),
        "P1-007: 0 matches → consumer 1 hint; got:\n{stderr}"
    );
    assert!(
        !stderr.contains("deleting existing same-filename attachments"),
        "P1-007: consumer 3 hint must NOT appear when there are 0 matches; got:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// P3-001 (CWE-116): test_cwe116_jsm_public_gate_sanitizes_hostile_filename
//
// jsm_public_gate (N≤3 path) must pass filenames through display_sanitize_filename
// before printing to stderr. A file with U+202E (RIGHT-TO-LEFT OVERRIDE) in its
// name must be rendered as '?' in the confirmation prompt.
//
// RED evidence: before fix, jsm_public_gate calls
//   `p.file_name().and_then(|n| n.to_str()).unwrap_or("<unknown>")`
// directly — the raw bidi char appears in stderr and the sanitized assertion fails.
//
// GREEN: after fix, display_sanitize_filename maps U+202E → '?' so stderr contains
// "evil?name.txt" and does NOT contain the raw U+202E byte sequence.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cwe116_jsm_public_gate_sanitizes_hostile_filename() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();

    // Create a file whose name contains U+202E (RIGHT-TO-LEFT OVERRIDE, 0x202E).
    // This is a valid UTF-8 codepoint and a valid filename on macOS/Linux.
    let hostile_name = "evil\u{202E}name.txt";
    let file = tmp.path().join(hostile_name);
    std::fs::write(&file, b"hostile").unwrap();

    // JSM project mocks for EJ-21.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-21"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-21", "EJ")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Step 1: attachTemporaryFile → success.
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-bidi-001", "fileName": hostile_name}]
        })))
        .mount(&server)
        .await;

    // Step 2: post_request_attachment → success.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-21/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("bidi-001", hostile_name),
            ])),
        )
        .mount(&server)
        .await;

    // Interactive path: JR_STDIN_IS_TTY=1 + write_stdin("y\n") so the gate fires and is confirmed.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-21",
            &file.to_string_lossy(),
            "--public",
        ])
        .write_stdin("y\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Must exit 0 (upload succeeded).
    assert_eq!(
        output.status.code(),
        Some(0),
        "P3-001 CWE-116: interactive --public confirm 'y' with bidi filename → exit 0;\nstdout: {stdout}\nstderr: {stderr}"
    );

    // The prompt must contain the sanitized filename (U+202E replaced by '?').
    assert!(
        stderr.contains("evil?name.txt"),
        "P3-001 CWE-116: jsm_public_gate must sanitize U+202E → '?' in stderr; got: {stderr}"
    );

    // The raw bidi char (U+202E, UTF-8: E2 80 AE) must NOT appear in stderr.
    assert!(
        !stderr.contains('\u{202E}'),
        "P3-001 CWE-116: raw U+202E must NOT appear in stderr; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// P4-001 Test A: test_jsm_internal_replace_existing_issues_delete_then_two_step
//
// BC-3.9.004 + BC-3.9.017 consumer-2: `--internal --replace-existing --yes` on JSM
// must DELETE matching attachments BEFORE the two-step upload (VP-576-003 ordering).
//
// RED evidence: replace_existing fetch+DELETE loop lives inside `if public { }` in
// `handle_attachment_upload_jsm`; `--internal` never enters that branch → DELETE
// mock .expect(1) is not satisfied → test fails on drop → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_jsm_internal_replace_existing_issues_delete_then_two_step() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("upload.txt");
    std::fs::write(&file, b"data").unwrap();

    // Issue GET: EJ16-1 → project key EJ16 (dedicated key to avoid FIFO shadowing)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ16-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_get_response("EJ16-1", "EJ16")),
        )
        .mount(&server)
        .await;

    // Project GET: EJ16 → JSM, project_id "90001"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ16"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ16", "90001")),
        )
        .mount(&server)
        .await;

    // Service desk list: entry with projectId "90001" → sdId "60003"
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{"id": "60003", "projectId": "90001", "projectName": "EJ16 Help Desk"}]
        })))
        .mount(&server)
        .await;

    // Attachment list: existing "upload.txt" → matches upload filename
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ16-1"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ16-1",
                vec![platform_attachment_object("AID-EJ16-01", "upload.txt")],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // DELETE mock: VP-576-003 ordering — DELETE before POST
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-EJ16-01"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // Step 1: attachTemporaryFile → sdId 60003
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/60003/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-ej16-01", "fileName": "upload.txt"}]
        })))
        .mount(&server)
        .await;

    // Step 2: post_request_attachment with public:false (--internal)
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ16-1/attachment"))
        .and(body_partial_json(serde_json::json!({"public": false})))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("AID-EJ16-02", "upload.txt"),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ16-1",
            &file.to_string_lossy(),
            "--internal",
            "--replace-existing",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "P4-001 Test A: --internal --replace-existing --yes → exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    // DELETE mock .expect(1) verified on MockServer drop

    // P5-001 VP-576-003 ordering pin: DELETE must precede first step-1 POST.
    let reqs = server.received_requests().await.unwrap();
    let last_delete_idx = reqs
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.method == wiremock::http::Method::DELETE
                && r.url.path().starts_with("/rest/api/3/attachment/")
        })
        .map(|(i, _)| i)
        .next_back()
        .expect("P5-001 VP-576-003: at least one DELETE must have been received");
    let first_step1_idx = reqs
        .iter()
        .enumerate()
        .find(|(_, r)| {
            r.method == wiremock::http::Method::POST && r.url.path().contains("attachTemporaryFile")
        })
        .map(|(i, _)| i)
        .expect("P5-001 VP-576-003: at least one step-1 POST must have been received");
    assert!(
        last_delete_idx < first_step1_idx,
        "P5-001 VP-576-003 ordering: DELETE must precede step-1 POST; \
         last DELETE at {last_delete_idx}, first step-1 at {first_step1_idx}"
    );
}

// ---------------------------------------------------------------------------
// P4-001 Test B: test_jsm_internal_replace_existing_noinput_exits_64_with_hint
//
// BC-3.9.017 consumer-2 non-interactive path: `--internal --replace-existing --no-input`
// (without --yes) exits 64 with canonical hint.
//
// RED evidence: `--internal` path never enters `if public { }` → no non-interactive
// exit fires → falls through to step-1 → gate not checked → wrong exit code → RED.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_jsm_internal_replace_existing_noinput_exits_64_with_hint() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("upload.txt");
    std::fs::write(&file, b"data").unwrap();

    // Same project/meta/list mocks as Test A (EJ16, permanent)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ16-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_get_response("EJ16-1", "EJ16")),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ16"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ16", "90001")),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{"id": "60003", "projectId": "90001", "projectName": "EJ16 Help Desk"}]
        })))
        .mount(&server)
        .await;

    // Attachment list: "upload.txt" exists → ≥1 match triggers consumer-2 gate
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ16-1"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ16-1",
                vec![platform_attachment_object("AID-EJ16-01", "upload.txt")],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // No DELETE or step-1/step-2 mocks: non-interactive exit fires before those

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ16-1",
            &file.to_string_lossy(),
            "--internal",
            "--replace-existing",
            "--no-input",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "P4-001 Test B: --internal --replace-existing --no-input → exit 64; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Use --yes to confirm deletion of existing same-filename attachments."),
        "P4-001 Test B: consumer-2 hint must appear in stderr; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// P4-001 Test C: test_jsm_internal_replace_existing_dry_run_shows_woulddelete
//
// `--internal --replace-existing --dry-run --output json` on JSM must show
// wouldDelete entries (dry_run_upload already handles --internal; verifying).
//
// Expected GREEN immediately (dry_run_upload handles replace_existing independently
// of --public/--internal). Added for explicit coverage of BC-3.9.017 dry-run path.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_jsm_internal_replace_existing_dry_run_shows_woulddelete() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("upload.txt");
    std::fs::write(&file, b"data").unwrap();

    // Same EJ16 mocks as Test A
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ16-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_get_response("EJ16-1", "EJ16")),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ16"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ16", "90001")),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{"id": "60003", "projectId": "90001", "projectName": "EJ16 Help Desk"}]
        })))
        .mount(&server)
        .await;

    // Attachment list: "upload.txt" → populates wouldDelete
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ16-1"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ16-1",
                vec![platform_attachment_object("AID-EJ16-01", "upload.txt")],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // No DELETE or step-1/step-2 mocks: dry-run suppresses those

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ16-1",
            &file.to_string_lossy(),
            "--internal",
            "--replace-existing",
            "--dry-run",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "P4-001 Test C: --internal --replace-existing --dry-run → exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );

    let preview: Value =
        serde_json::from_str(&stdout).expect("P4-001 Test C: dry-run JSON must be valid object");
    let would_delete = preview
        .get("wouldDelete")
        .and_then(Value::as_array)
        .expect("P4-001 Test C: wouldDelete must be array");
    assert!(
        !would_delete.is_empty(),
        "P4-001 Test C: wouldDelete must be non-empty"
    );
    assert_eq!(
        would_delete[0].get("filename").and_then(Value::as_str),
        Some("upload.txt"),
        "P4-001 Test C: wouldDelete[0].filename must be 'upload.txt'; got: {:?}",
        would_delete[0]
    );
    assert_eq!(
        would_delete[0].get("id").and_then(Value::as_str),
        Some("AID-EJ16-01"),
        "P4-001 Test C: wouldDelete[0].id must be 'AID-EJ16-01'; got: {:?}",
        would_delete[0]
    );
}

// ---------------------------------------------------------------------------
// P4-004: test_ec_3_9_003_3_multi_file_jsm_upload_two_step_1_posts
//
// EC-3.9.003-3: for N files, step-1 is called N times (once per file), each
// returning a distinct tmpId. Step-2 is called ONCE with all N tmpIds.
//
// Uses dedicated project key EJ17 (sdId 60004) to avoid FIFO shadowing.
// Wiremock FIFO ordering: step-1 mock A serves the first POST, mock B serves the second.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_ec_3_9_003_3_multi_file_jsm_upload_two_step_1_posts() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file1 = tmp.path().join("file1.txt");
    let file2 = tmp.path().join("file2.txt");
    std::fs::write(&file1, b"content1").unwrap();
    std::fs::write(&file2, b"content2").unwrap();

    // Issue GET: EJ17-1 → project EJ17
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ17-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_get_response("EJ17-1", "EJ17")),
        )
        .mount(&server)
        .await;

    // Project GET: EJ17 → JSM, project_id "90002"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ17"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ17", "90002")),
        )
        .mount(&server)
        .await;

    // Service desk list: entry with projectId "90002" → sdId "60004"
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{"id": "60004", "projectId": "90002", "projectName": "EJ17 Help Desk"}]
        })))
        .mount(&server)
        .await;

    // Step-1: called ONCE PER FILE (EC-3.9.003-3 pin).
    // wiremock FIFO: a single mock with .expect(2) verifies exactly 2 step-1 calls.
    // Using a fixed tmpId ("tmp-multi") — the important assertion is call count,
    // not distinct IDs per call.
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/60004/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-multi", "fileName": "file.txt"}]
        })))
        .expect(2)
        .mount(&server)
        .await;

    // Step-2: called EXACTLY ONCE with public:true (EC-3.9.003-3 pin).
    // The body contains temporaryAttachmentIds with 2 entries (one per file).
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ17-1/attachment"))
        .and(body_partial_json(serde_json::json!({"public": true})))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("AID-EJ17-01", "file1.txt"),
                attachment_object("AID-EJ17-02", "file2.txt"),
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ17-1",
            &file1.to_string_lossy(),
            &file2.to_string_lossy(),
            "--public",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "P4-004 EC-3.9.003-3: multi-file two-step → exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );

    // Verify step-2 body contains 2 entries in temporaryAttachmentIds (one per file).
    let received = server.received_requests().await.unwrap();
    let step2_req = received
        .iter()
        .find(|r| {
            r.url.path().ends_with("/attachment")
                && r.method == wiremock::http::Method::POST
                && r.url.path().contains("EJ17-1")
        })
        .expect("P4-004: step-2 request must have been received");
    let body: Value =
        serde_json::from_slice(&step2_req.body).expect("step-2 body must be valid JSON");
    let tmp_ids = body
        .get("temporaryAttachmentIds")
        .and_then(Value::as_array)
        .expect("P4-004: step-2 body must have temporaryAttachmentIds array");
    assert_eq!(
        tmp_ids.len(),
        2,
        "P4-004 EC-3.9.003-3: step-2 must contain 2 tmpIds (one per file); got {:?}",
        tmp_ids
    );
    // step-1 .expect(2) and step-2 .expect(1) verified on MockServer drop
}

// ---------------------------------------------------------------------------
// P5-002: test_sec_576_004_jsm_step1_content_disposition_crlf_guard
//
// SEC-576-004 CWE-93: `attach_temporary_file` guard (L66-76 in api/jsm/attachments.rs)
// replaces '\r', '\n', '\0' with '_' in the multipart Content-Disposition filename
// before passing to Part::file_name(). Verify on the JSM step-1 path.
//
// RED evidence (kill-test): removing L66-76 sets safe_name = raw_name; step-1 body
// then contains the raw '\n' between "evil" and "name.txt" → last assertion fires.
// ---------------------------------------------------------------------------

#[tokio::test]
#[cfg_attr(windows, ignore)] // Windows rejects filenames with \n (WinError 123); attack vector absent on Windows
async fn test_sec_576_004_jsm_step1_content_disposition_crlf_guard() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();

    // Create a file with a literal LF in the name (legal on macOS/Linux).
    let hostile_name = "evil\nname.txt";
    let file = tmp.path().join(hostile_name);
    std::fs::write(&file, b"crlf guard test").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJCWE-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_get_response("EJCWE-1", "EJCWE")),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJCWE"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsm_project_response("EJCWE", "92001")),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("60099", "92001")),
        )
        .mount(&server)
        .await;

    // Step-1: responds with sanitized fileName (server echoes what it received).
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/60099/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-cwe93", "fileName": "evil_name.txt"}]
        })))
        .mount(&server)
        .await;

    // Step-2: complete the upload so exit 0.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJCWE-1/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("cwe93-01", "evil_name.txt"),
            ])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJCWE-1",
            &file.to_string_lossy(),
            "--public",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "SEC-576-004 JSM step-1: upload with LF in filename → exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // Inspect step-1 POST body for sanitized Content-Disposition filename.
    let reqs = server.received_requests().await.unwrap();
    let step1_req = reqs
        .iter()
        .find(|r| {
            r.method == wiremock::http::Method::POST && r.url.path().contains("attachTemporaryFile")
        })
        .expect("SEC-576-004 JSM: step-1 POST must have been received");

    let body = String::from_utf8_lossy(&step1_req.body);
    // Guard present: '\n' → '_' → "evil_name.txt" in Content-Disposition.
    assert!(
        body.contains("evil_name.txt"),
        "SEC-576-004 JSM: sanitized filename 'evil_name.txt' must appear in step-1 body; \
         body excerpt: {}",
        &body[..body.len().min(400)]
    );
    // Guard absent (kill-test mutation): raw LF would appear between "evil" and "name.txt".
    assert!(
        !body.contains("evil\nname.txt"),
        "SEC-576-004 JSM: raw LF must NOT appear in Content-Disposition filename in step-1 body; \
         body excerpt: {}",
        &body[..body.len().min(400)]
    );
}

// ---------------------------------------------------------------------------
// P5-003 Test A: test_p5_003_consumer3_combined_prompt_hostile_server_filename_sanitized
//
// CWE-116: server-supplied hostile filename (U+202E bidi) in the attachment list
// must be sanitized by display_sanitize_filename before appearing in the consumer-3
// combined gate prompt. Higher-risk vector: the attacker controls server-returned names.
//
// RED evidence (kill-test): remove display_sanitize_filename() call in consumer-3
// prompt display → raw U+202E appears in stderr → last assertion fires.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_p5_003_consumer3_combined_prompt_hostile_server_filename_sanitized() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();

    // Hostile name shared by local file and server-returned attachment.
    let hostile_name = "evil\u{202E}name.txt";
    let file = tmp.path().join(hostile_name);
    std::fs::write(&file, b"hostile c3").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-P5C3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-P5C3", "EJ")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Server-supplied hostile filename in attachment list → replace-existing matches →
    // consumer-3 fires. Use .with_priority(1) to shadow the plain issue GET mock.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-P5C3"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ-P5C3",
                vec![platform_attachment_object("AID-P5C3-01", hostile_name)],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // Cancel via "n\n" so no upload mocks needed.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-P5C3",
            &file.to_string_lossy(),
            "--public",
            "--replace-existing",
        ])
        .write_stdin("n\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "P5-003 consumer-3: cancel 'n' → exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // CWE-116: U+202E must be replaced by '?' in the consumer-3 prompt.
    assert!(
        stderr.contains("evil?name.txt"),
        "P5-003 consumer-3: sanitized 'evil?name.txt' must appear in prompt; got: {stderr}"
    );
    // Raw bidi char must NOT appear in stderr.
    assert!(
        !stderr.contains('\u{202E}'),
        "P5-003 consumer-3: raw U+202E must NOT appear in stderr; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// P5-003 Test B: test_p5_003_consumer2_internal_replace_hostile_server_filename_sanitized
//
// CWE-116: same as P5-003 Test A but for the consumer-2 (--internal --replace-existing)
// interactive gate prompt.
//
// RED evidence (kill-test): remove display_sanitize_filename() in consumer-2 prompt
// display → raw U+202E appears in stderr → last assertion fires.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_p5_003_consumer2_internal_replace_hostile_server_filename_sanitized() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();

    let hostile_name = "evil\u{202E}name.txt";
    let file = tmp.path().join(hostile_name);
    std::fs::write(&file, b"hostile c2").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-P5C2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-P5C2", "EJ")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Server-supplied hostile filename in attachment list → consumer-2 fires.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-P5C2"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ-P5C2",
                vec![platform_attachment_object("AID-P5C2-01", hostile_name)],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // Cancel via "n\n" so no upload mocks needed.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-P5C2",
            &file.to_string_lossy(),
            "--internal",
            "--replace-existing",
        ])
        .write_stdin("n\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "P5-003 consumer-2: cancel 'n' → exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // CWE-116: U+202E must be replaced by '?' in the consumer-2 prompt.
    assert!(
        stderr.contains("evil?name.txt"),
        "P5-003 consumer-2: sanitized 'evil?name.txt' must appear in prompt; got: {stderr}"
    );
    assert!(
        !stderr.contains('\u{202E}'),
        "P5-003 consumer-2: raw U+202E must NOT appear in stderr; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// P6-001: BC-3.9.007 — step-2 201 echo with non-conformant body (bare array)
// MUST NOT fail the upload. Schema drift on the 2xx echo → exit 0 + empty
// JSON array on stdout + warning on stderr (best-effort semantics).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bc_3_9_007_step2_bare_array_echo_exits_0_best_effort() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("p6.txt");
    std::fs::write(&file, b"p6test").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-P6A"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-P6A", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-p6-001", "fileName": "p6.txt"}]
        })))
        .mount(&server)
        .await;

    // Step-2 returns 201 with a bare JSON array — NOT the expected
    // {"attachments":{"values":[...]}} envelope.  BC-3.9.007 mandates that
    // schema drift on the 2xx echo must NEVER fail the upload command.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-P6A/attachment"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-P6A",
            &file.to_string_lossy(),
            "--public",
            "--yes",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // BC-3.9.007: upload succeeded server-side → MUST exit 0 even when echo is malformed.
    assert_eq!(
        output.status.code(),
        Some(0),
        "P6-001 BC-3.9.007: step-2 bare-array echo → exit 0 (upload succeeded); got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    // Best-effort: unrecognized echo shape → empty JSON array on stdout.
    let arr: Vec<Value> = serde_json::from_str(&stdout)
        .expect("P6-001 BC-3.9.007: stdout must be a JSON array even on malformed echo");
    assert!(
        arr.is_empty(),
        "P6-001 BC-3.9.007: unrecognized echo → empty output array; got: {stdout}"
    );
    // Warning on stderr so the operator knows the echo was unexpected.
    assert!(
        stderr.contains("warning:"),
        "P6-001 BC-3.9.007: warning must appear on stderr when echo shape unrecognized; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Mutation-gate tests — S-576-5 PR #640 mutant-kill supplement
//
// Four tests covering surviving mutants in src/cli/issue/attachments.rs:
//   C  — line 1425: `jsm_public_gate` "yes" long-form alias (eq_ignore_ascii_case)
//   D1 — line 1619: `replace_existing && has_replace_matches` (public path)
//   D2 — line 1638: `replace_existing && has_replace_matches` (internal/else-if path)
//   E  — line 1676: `!is_benign` delete-error propagation
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// C: test_jsm_public_gate_long_form_yes_accepted
//
// `jsm_public_gate` (line 1425): the "yes" long-form alias must be accepted.
// `trimmed.eq_ignore_ascii_case("yes")` — deleting or mutating this arm allows
// "yes\n" to fall through to a false return, cancelling the upload.
//
// RED evidence (mutant): `|| trimmed.eq_ignore_ascii_case("yes")` deleted →
// stdin "yes\n" → gate returns false → "Upload cancelled." → exit 0 but with
// cancelled:true output, NOT the uploaded attachments (test expects exit 0 with
// uploaded attachment, so assertions on step-1 POST being made would catch it).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_jsm_public_gate_long_form_yes_accepted() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("attach.txt");
    std::fs::write(&file, b"hello").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-22"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-22", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Step-1: attachTemporaryFile — must be called if "yes" gate accepted
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/servicedesk/42/attachTemporaryFile"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-yes-001", "fileName": "attach.txt"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-22/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20100", "attach.txt"),
            ])),
        )
        .mount(&server)
        .await;

    // Long-form "yes\n" — covers the eq_ignore_ascii_case("yes") branch at line 1425.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-22",
            &file.to_string_lossy(),
            "--public",
        ])
        .write_stdin("yes\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "C / line 1425: 'yes' long-form must be accepted by jsm_public_gate → exit 0; \
         got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    // Upload cancelled implies the gate returned false — must not happen.
    assert!(
        !stderr.contains("Upload cancelled"),
        "C / line 1425: 'yes' must not cancel; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// D1: test_jsm_public_replace_zero_matches_consumer1_not_consumer3
//
// line 1619: `replace_existing && has_replace_matches` (interactive public path).
//
// With the `||` mutant (`replace_existing || has_replace_matches`), the combined
// consumer-3 gate fires even when zero filenames match.  The test confirms that
// when --replace-existing is set but there are ZERO same-name matches, the
// consumer-1 gate fires (contains "as customer-visible (public)?") and consumer-3
// does NOT fire (no "and replace existing attachment(s):").
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_jsm_public_replace_zero_matches_consumer1_not_consumer3() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    // Upload "nomatch.txt"; attachment list has only "other.txt" → zero filename matches.
    let file = tmp.path().join("nomatch.txt");
    std::fs::write(&file, b"content").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-23"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-23", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Attachment list: "other.txt" exists — NOT "nomatch.txt" → has_replace_matches = false.
    // .with_priority(1) wins over the EJ-23 issue-GET mock (same path, registered first).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-23"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ-23",
                vec![platform_attachment_object("AID-99001", "other.txt")],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // Step-1 and step-2 mocks: upload proceeds after consumer-1 gate accepts "y".
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/servicedesk/42/attachTemporaryFile"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-d1-001", "fileName": "nomatch.txt"}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-23/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20101", "nomatch.txt"),
            ])),
        )
        .mount(&server)
        .await;

    // Interactive "y\n": consumer-1 fires (not consumer-3); upload completes.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-23",
            &file.to_string_lossy(),
            "--public",
            "--replace-existing",
        ])
        .write_stdin("y\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "D1 / line 1619: --public --replace-existing zero matches + 'y' → exit 0; \
         got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    // Consumer-1 gate text must appear (zero matches → not combined gate).
    assert!(
        stderr.contains("as customer-visible (public)?"),
        "D1 / line 1619: consumer-1 prompt must fire when matches=0; got: {stderr}"
    );
    // Consumer-3 combined gate text must NOT appear.
    assert!(
        !stderr.contains("and replace existing attachment(s):"),
        "D1 / line 1619: consumer-3 combined gate must NOT fire when matches=0; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// D2: test_jsm_internal_replace_zero_matches_no_gate_upload_proceeds
//
// line 1638: `else if replace_existing && has_replace_matches` (internal path).
//
// With the `||` mutant, consumer-2 gate fires even with zero matches.
// Combined with `--no-input`, the consumer-2 `if no_input && !yes` branch would
// exit 64 — preventing the upload entirely.
// Original: zero matches → condition false → no gate → upload proceeds.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_jsm_internal_replace_zero_matches_no_gate_upload_proceeds() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    // Upload "newfile.txt"; attachment list has only "existing.txt" → zero matches.
    let file = tmp.path().join("newfile.txt");
    std::fs::write(&file, b"data").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-24"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-24", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Attachment list: "existing.txt" but NOT "newfile.txt" → has_replace_matches = false.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-24"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ-24",
                vec![platform_attachment_object("AID-99002", "existing.txt")],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // Step-1 must be called: if no gate fires the two-step upload runs.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/servicedesk/42/attachTemporaryFile"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-d2-001", "fileName": "newfile.txt"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJ-24/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("20102", "newfile.txt"),
            ])),
        )
        .mount(&server)
        .await;

    // --internal --replace-existing --no-input without --yes:
    // zero matches → else-if condition false → no gate fires → upload runs.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-24",
            &file.to_string_lossy(),
            "--internal",
            "--replace-existing",
            "--no-input",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "D2 / line 1638: --internal --replace-existing --no-input zero matches \
         → no gate → exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    // Consumer-2 hint must NOT appear (gate did not fire).
    assert!(
        !stderr.contains("Use --yes to confirm deletion"),
        "D2 / line 1638: consumer-2 hint must NOT appear when matches=0; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// E: test_replace_existing_nonbenign_delete_error_propagates
//
// line 1676: `if !is_benign { return Err(e); }` — non-benign delete error propagation.
//
// With the `!` deletion mutant (`if is_benign { return Err(e); }`):
// - A non-benign error (e.g. HTTP 500) has is_benign=false → condition false →
//   error silently swallowed → upload continues → exits 0.
// - The test asserts exit non-zero AND step-1 was never called.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_replace_existing_nonbenign_delete_error_propagates() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("upload.txt");
    std::fs::write(&file, b"data").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_get_response("EJ-25", "EJ")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsm_project_response("EJ", "10099")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
        .mount(&server)
        .await;

    // Attachment list: "upload.txt" exists → 1 match → delete loop fires.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-25"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ-25",
                vec![platform_attachment_object("AID-DEL-001", "upload.txt")],
            )),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    // DELETE returns 500 — non-benign (is_benign = false).
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-DEL-001"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .expect(1)
        .mount(&server)
        .await;

    // Step-1 must NOT be called: error propagates before it is reached.
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
        ))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-25",
            &file.to_string_lossy(),
            "--public",
            "--replace-existing",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_ne!(
        output.status.code(),
        Some(0),
        "E / line 1676: non-benign DELETE 500 must exit non-zero; \
         got exit 0\nstdout: {stdout}\nstderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// F5-R1-006 (JSM step-1): SEC-576-004 double-quote guard in attach_temporary_file
// ---------------------------------------------------------------------------

/// F5-R1-006 (JSM step-1): double-quote (`"`) in the upload filename must be mapped
/// to underscore (`_`) by the SEC-576-004 guard in `attach_temporary_file` BEFORE
/// the filename is placed into the `Content-Disposition` header of the multipart POST.
///
/// This is the JSM-path counterpart of
/// `test_f5_r1_006_upload_content_disposition_double_quote_mapped_to_underscore`
/// (in tests/attachment_upload.rs).
///
/// Note: `test_sec_576_004_jsm_step1_content_disposition_crlf_guard` (above) covers
/// `\n` injection. This test extends coverage to `"` injection in the JSM step-1 path.
///
/// RED: current guard (`src/api/jsm/attachments.rs` ~L67-76) only maps `\r`, `\n`,
/// `\0` — NOT `"`. The step-1 body will contain reqwest-escaped `file\"name.txt`,
/// NOT the sanitized form `file_name.txt` → the first body assertion fails.
///
/// Unix only: `"` is a valid filename char on Unix.
#[cfg(unix)]
#[tokio::test]
async fn test_f5_r1_006_jsm_step1_content_disposition_double_quote_mapped_to_underscore() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();

    // `"` is valid on Unix; guard must replace it with `_`.
    let hostile_name = "file\"name.txt";
    let file = tmp.path().join(hostile_name);
    std::fs::write(&file, b"f5-r1-006 jsm guard test").unwrap();

    // Step 0: issue GET.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJFR6-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_get_response("EJFR6-1", "EJFR6")),
        )
        .mount(&server)
        .await;

    // Step 0b: project GET.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/EJFR6"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsm_project_response("EJFR6", "93006")),
        )
        .mount(&server)
        .await;

    // Service desk list.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("61006", "93006")),
        )
        .mount(&server)
        .await;

    // Step 1: attachTemporaryFile — responds with sanitized fileName.
    Mock::given(method("POST"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/61006/attachTemporaryFile",
        ))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "temporaryAttachments": [{"temporaryAttachmentId": "tmp-f5r1006", "fileName": "file_name.txt"}]
        })))
        .mount(&server)
        .await;

    // Step 2: request attachment — completes the upload.
    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request/EJFR6-1/attachment"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(attachment_create_result_dto(vec![
                attachment_object("f5r1006-01", "file_name.txt"),
            ])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJFR6-1",
            &file.to_string_lossy(),
            "--public",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "F5-R1-006 JSM step-1: upload with '\"' in filename must exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // Inspect the step-1 POST body for the sanitized Content-Disposition filename.
    let reqs = server.received_requests().await.unwrap();
    let step1_req = reqs
        .iter()
        .find(|r| {
            r.method == wiremock::http::Method::POST && r.url.path().contains("attachTemporaryFile")
        })
        .expect("F5-R1-006 JSM: step-1 POST to attachTemporaryFile must have been received");

    let body = String::from_utf8_lossy(&step1_req.body);

    // Target: guard maps '"' → '_' → Content-Disposition contains "file_name.txt".
    // RED: current guard does NOT map '"'; body contains "file\"name.txt" (reqwest form).
    assert!(
        body.contains("file_name.txt"),
        "F5-R1-006 JSM step-1: '\"' must be mapped to '_' by SEC-576-004 guard; \
         Content-Disposition filename must be 'file_name.txt'; \
         body excerpt: {}",
        &body[..body.len().min(500)]
    );

    // The reqwest-escaped form must NOT appear when the guard fires.
    assert!(
        !body.contains("file\\\"name.txt"),
        "F5-R1-006 JSM step-1: reqwest-escaped '\"' (backslash form) must NOT appear \
         when guard maps it to '_'; body excerpt: {}",
        &body[..body.len().min(500)]
    );
}

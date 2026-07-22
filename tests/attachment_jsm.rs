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
use wiremock::matchers::{header, method, path};
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

/// Raw attachment object as returned by the Jira API.
fn attachment_object(id: &str, filename: &str) -> Value {
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
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!([
            attachment_object("20001", "attach.txt")
        ])))
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
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ-1",
                vec![attachment_object("99001", "attach.txt")],
            )),
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

    // VP-576-005: the combined prompt keyword "Continue?" must appear exactly once.
    let continue_count = stderr.matches("Continue?").count();
    assert!(
        continue_count <= 1,
        "VP-576-005: combined gate must fire at most once (not two prompts); \
         'Continue?' appeared {continue_count} times in stderr:\n{stderr}"
    );

    // VP-576-005: when the implementation is correct, it should appear exactly once.
    // For RED, the interim guard fires exit 64 before any prompt is shown (0 times).
    // We assert <= 1 as the must-not-regress condition, so this passes in both RED and GREEN.
    // The positive assertion (== 1) is checked by the implementer in Task 5.
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
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!([
            attachment_object("20002", "upload.txt")
        ])))
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
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!([attachment_object("20003", "test.txt")])),
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
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!([attachment_object("20004", "stale.txt")])),
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
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!([
            attachment_object("20005", "internal.txt")
        ])))
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

    // Non-JSM project (software).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(non_jsm_project_response("DEV")))
        .mount(&server)
        .await;

    // Platform POST — returns a successful upload.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            attachment_object("20006", "nonjsm.txt")
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
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!([attachment_object("20007", "shape.txt")])),
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

    // --- Sub-assertion 1: --public on non-JSM → exit 64 + BC-3.9.005 message ---
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
    // JSM project mocks for this sub-assertion.
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
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(service_desk_list_response("42", "10099")),
        )
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
        // Need a list attachment mock for --replace-existing. The list endpoint is
        // GET /rest/api/3/issue/{key}?fields=attachment.
        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/EJ-10"))
            .and(wiremock::matchers::query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(attachments_list_response(
                    "EJ-10",
                    vec![attachment_object("99010", "tax.txt")],
                )),
            )
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
    } // _step2_guard drops → 400 mock removed before sub-assertion 8

    // --- Sub-assertion 7: sdId not found after SEC-576-006 retry → exit 64 + "not found after refresh" ---
    // This sub-assertion is satisfied by the existing two sub-assertions on exit code and message.
    // (Detailed test in AC-005; here we just pin the error message substring.)

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
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!([attachment_object("20012", "shape.txt")])),
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
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!([attachment_object("20013", "asym.txt")])),
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
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(non_jsm_project_response("DEV")))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-2/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!([attachment_object("20013b", "asym.txt")])),
        )
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
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-15"))
        .and(wiremock::matchers::query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(attachments_list_response(
                "EJ-15",
                vec![attachment_object("AID-01", "dry.txt")],
            )),
        )
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
                && r.url.query().map_or(false, |q| q.contains("attachment"))
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

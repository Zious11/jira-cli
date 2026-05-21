//! Integration tests for `jr issue edit KEY --type X` HTTP 400 enrichment paths.
//!
//! BC anchors:
//!   BC-3.4.010 — cross-hierarchy mismatch (`subtask` flag differs) →
//!                exit 1 + `CROSS_HIERARCHY_HINT` containing `JRACLOUD-27893` on stderr;
//!                stderr does NOT contain `jr api /rest/api/3/issue` (regression pin on
//!                the removed fake-endpoint hint).
//!   BC-3.4.011 — same-hierarchy OR indeterminate → exit 1 + typo hint or raw error;
//!                `JRACLOUD-27893` MUST NOT appear on stderr.
//!
//! All ten tests exercise the fully-implemented `is_cross_hierarchy_type_error` and
//! `handle_edit` dispatch on `Classification`. They are GREEN after implementation.
//!
//! Wiremock topology summary:
//!   PUT /rest/api/3/issue/{key}         — edit call (400 or 403 depending on test)
//!   GET /rest/api/3/issue/{key}         — enrichment fetch for source issuetype
//!   GET /rest/api/3/project/{proj_key}  — enrichment fetch for project issue types

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Start a wiremock server.
async fn start_server() -> MockServer {
    MockServer::start().await
}

/// Build a `jr` command pointing at the mock server.
/// Auth is injected via `JR_AUTH_HEADER` (Basic test:test in base64) — a
/// debug-only seam documented in CLAUDE.md "AI Agent Notes".
fn jr_cmd(server_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0"); // test:test
    cmd
}

/// JSON body returned by PUT /rest/api/3/issue/{key} → HTTP 400.
/// Uses a plain-ASCII extracted message so test substrings survive
/// `extract_error_message` / `sanitize_for_stderr` (BC-3.4.011 note).
fn edit_400_body() -> serde_json::Value {
    serde_json::json!({
        "errorMessages": [],
        "errors": {
            "issuetype": "The issue type selected is invalid."
        }
    })
}

/// JSON body returned by PUT /rest/api/3/issue/{key} → HTTP 403.
fn edit_403_body() -> serde_json::Value {
    serde_json::json!({
        "errorMessages": ["You do not have permission to edit this issue."],
        "errors": {}
    })
}

/// GET /rest/api/3/issue/{key} — source issue with `subtask: false` (standard type).
fn get_issue_standard(key: &str, project_key: &str) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "fields": {
            "summary": "A standard issue",
            "status": {"name": "To Do"},
            "issuetype": {
                "name": "Task",
                "subtask": false
            },
            "priority": {"name": "Medium"},
            "assignee": null,
            "project": {
                "key": project_key,
                "name": "Test Project"
            }
        }
    })
}

/// GET /rest/api/3/issue/{key} — source issue with `subtask: true` (sub-task type).
fn get_issue_subtask(key: &str, project_key: &str) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "fields": {
            "summary": "A subtask issue",
            "status": {"name": "To Do"},
            "issuetype": {
                "name": "Sub-task",
                "subtask": true
            },
            "priority": {"name": "Medium"},
            "assignee": null,
            "project": {
                "key": project_key,
                "name": "Test Project"
            }
        }
    })
}

/// GET /rest/api/3/issue/{key} — source issue with `subtask` key OMITTED from issuetype.
/// Produces `IssueType { name: "Task", subtask: None }` via `#[serde(default)]`.
fn get_issue_no_subtask_field(key: &str, project_key: &str) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "fields": {
            "summary": "An issue without subtask flag",
            "status": {"name": "To Do"},
            "issuetype": {
                "name": "Task"
                // "subtask" key intentionally absent
            },
            "priority": {"name": "Medium"},
            "assignee": null,
            "project": {
                "key": project_key,
                "name": "Test Project"
            }
        }
    })
}

/// GET /rest/api/3/project/{key} response — list includes a Sub-task type (`subtask: true`)
/// and a standard Task type (`subtask: false`).
fn get_project_types_with_subtask_target() -> serde_json::Value {
    serde_json::json!({
        "key": "TEST",
        "name": "Test Project",
        "issueTypes": [
            {
                "name": "Task",
                "subtask": false
            },
            {
                "name": "Bug",
                "subtask": false
            },
            {
                "name": "Sub-task",
                "subtask": true
            }
        ]
    })
}

/// GET /rest/api/3/project/{key} response — list includes only standard types (no subtask).
/// The target type name `"Task"` is in the list with `subtask: false`.
fn get_project_types_standard_only() -> serde_json::Value {
    serde_json::json!({
        "key": "TEST",
        "name": "Test Project",
        "issueTypes": [
            {
                "name": "Task",
                "subtask": false
            },
            {
                "name": "Bug",
                "subtask": false
            },
            {
                "name": "Story",
                "subtask": false
            }
        ]
    })
}

/// GET /rest/api/3/project/{key} response — target type `subtask` key OMITTED.
/// Produces `IssueTypeMetadata { name: "Task", subtask: None }`.
fn get_project_types_target_no_subtask_field() -> serde_json::Value {
    serde_json::json!({
        "key": "TEST",
        "name": "Test Project",
        "issueTypes": [
            {
                "name": "Task"
                // "subtask" key intentionally absent
            },
            {
                "name": "Bug",
                "subtask": false
            }
        ]
    })
}

/// GET /rest/api/3/project/{key} response — list does NOT contain `"Taks"`.
/// Used for unresolvable-name sub-path tests.
fn get_project_types_no_target_match() -> serde_json::Value {
    serde_json::json!({
        "key": "TEST",
        "name": "Test Project",
        "issueTypes": [
            {
                "name": "Story",
                "subtask": false
            },
            {
                "name": "Bug",
                "subtask": false
            },
            {
                "name": "Task",
                "subtask": false
            }
        ]
    })
}

// ---------------------------------------------------------------------------
// Test #1 — BC-3.4.010 CrossHierarchy std→subtask
// ---------------------------------------------------------------------------
//
// Source issue: `subtask: Some(false)` (standard Task).
// Target type:  `subtask: Some(true)` (Sub-task).
// Expected: exit 1, stderr contains `JRACLOUD-27893`, does NOT contain
//           `jr api /rest/api/3/issue`.
//
// Red failure mode: `is_cross_hierarchy_type_error` panics with `todo!()` or
// the dispatch block does not exist → binary crashes/exits non-1 without the
// expected hint → assertions fail.

#[tokio::test]
async fn test_edit_type_cross_hierarchy_std_to_subtask_surfaces_conversion_hint() {
    let server = start_server().await;

    // PUT /rest/api/3/issue/TEST-1 → 400
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(400).set_body_json(edit_400_body()))
        .mount(&server)
        .await;

    // GET /rest/api/3/issue/TEST-1 → 200 with subtask: false (standard)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(get_issue_standard("TEST-1", "TEST")),
        )
        .mount(&server)
        .await;

    // GET /rest/api/3/project/TEST → 200 with subtask target type
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/TEST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(get_project_types_with_subtask_target()),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--type",
            "Sub-task",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit 1 (API error path), not 2 (clap) or 0 (success).
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected exit 1 for cross-hierarchy 400; stderr={stderr}"
    );

    // BC-3.4.010 postcondition: JRACLOUD-27893 MUST appear on stderr.
    assert!(
        stderr.contains("JRACLOUD-27893"),
        "Expected JRACLOUD-27893 in stderr (cross-hierarchy hint); stderr={stderr}"
    );

    // BC-3.4.010 postcondition: regression pin on removed fake-endpoint hint.
    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint must not appear in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test #2 — BC-3.4.010 CrossHierarchy subtask→std (reverse direction)
// ---------------------------------------------------------------------------
//
// Source issue: `subtask: Some(true)` (sub-task).
// Target type:  `subtask: Some(false)` (standard Task).
// Same assertions as test #1.

#[tokio::test]
async fn test_edit_type_cross_hierarchy_subtask_to_std_surfaces_conversion_hint() {
    let server = start_server().await;

    // PUT /rest/api/3/issue/SUB-1 → 400
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/SUB-1"))
        .respond_with(ResponseTemplate::new(400).set_body_json(edit_400_body()))
        .mount(&server)
        .await;

    // GET /rest/api/3/issue/SUB-1 → 200 with subtask: true (sub-task)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/SUB-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(get_issue_subtask("SUB-1", "TEST")))
        .mount(&server)
        .await;

    // GET /rest/api/3/project/TEST → 200 with standard types (target "Task" has subtask: false)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/TEST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(get_project_types_standard_only()))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "SUB-1", "--type", "Task"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected exit 1 for cross-hierarchy 400 (subtask→std); stderr={stderr}"
    );

    assert!(
        stderr.contains("JRACLOUD-27893"),
        "Expected JRACLOUD-27893 in stderr (subtask→std cross-hierarchy hint); stderr={stderr}"
    );

    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint must not appear in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test #3 — BC-3.4.011 SameCategory: both flags equal, typo hint emitted
// ---------------------------------------------------------------------------
//
// Source issue: `subtask: Some(false)`.
// Target type:  `subtask: Some(false)` (same hierarchy level).
// Expected: exit 1; stderr contains `jr project types` (typo hint);
//           stderr contains a substring from the extracted 400 message;
//           `JRACLOUD-27893` MUST NOT appear; `jr api /rest/api/3/issue` MUST NOT appear.

#[tokio::test]
async fn test_edit_type_same_hierarchy_400_surfaces_typo_hint() {
    let server = start_server().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-2"))
        .respond_with(ResponseTemplate::new(400).set_body_json(edit_400_body()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(get_issue_standard("TEST-2", "TEST")),
        )
        .mount(&server)
        .await;

    // Target type "Task" has subtask: false → same hierarchy as source → SameCategory.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/TEST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(get_project_types_standard_only()))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "TEST-2", "--type", "Task"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected exit 1 for same-hierarchy 400; stderr={stderr}"
    );

    // BC-3.4.011 SameCategory postcondition: typo hint with `jr project types`.
    assert!(
        stderr.contains("jr project types"),
        "Expected typo hint containing 'jr project types' in stderr; stderr={stderr}"
    );

    // The extracted 400 message text must appear (plain-ASCII substring safe through sanitize_for_stderr).
    assert!(
        stderr.contains("issue type selected is invalid")
            || stderr.contains("The issue type selected is invalid"),
        "Expected extracted 400 message substring in stderr; stderr={stderr}"
    );

    // BC-3.4.011 negative constraint: JRACLOUD-27893 MUST NOT appear.
    assert!(
        !stderr.contains("JRACLOUD-27893"),
        "JRACLOUD-27893 must NOT appear on SameCategory path; stderr={stderr}"
    );

    // Regression pin on removed fake-endpoint hint.
    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint must not appear in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test #4 — BC-3.4.011 Indeterminate Cause-1 R2: project-types 5xx
// ---------------------------------------------------------------------------
//
// Source issue fetch: succeeds, returns `subtask: Some(false)`.
// Project types fetch: HTTP 5xx → `Result::is_err()` → Indeterminate.
// Expected: exit 1; stderr contains extracted 400 message substring;
//           no enrichment hint; `JRACLOUD-27893` MUST NOT appear;
//           `jr api /rest/api/3/issue` MUST NOT appear.

#[tokio::test]
async fn test_edit_type_indeterminate_project_types_5xx_surfaces_raw_error() {
    let server = start_server().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-3"))
        .respond_with(ResponseTemplate::new(400).set_body_json(edit_400_body()))
        .mount(&server)
        .await;

    // GET issue succeeds with subtask: Some(false).
    // `.expect(1)` — pre-impl this is never called (handle_edit does not
    // dispatch to the enrichment path) → wiremock verify fails on server drop.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-3"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(get_issue_standard("TEST-3", "TEST")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // GET project types → 5xx → Indeterminate (Cause-1 R2).
    // `.expect(1)` — pre-impl this is never called → verify fails on drop.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/TEST"))
        .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({
            "errorMessages": ["Service temporarily unavailable."],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "TEST-3", "--type", "Task"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for indeterminate 400; stderr={stderr}"
    );

    // Extracted 400 message from `edit_issue` must appear on stderr.
    assert!(
        stderr.contains("issue type selected is invalid")
            || stderr.contains("The issue type selected is invalid"),
        "Expected extracted 400 message substring in stderr (Indeterminate R2); stderr={stderr}"
    );

    // BC-3.4.011 negative constraint.
    assert!(
        !stderr.contains("JRACLOUD-27893"),
        "JRACLOUD-27893 must NOT appear on Indeterminate R2 path; stderr={stderr}"
    );

    // Regression pin.
    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint must not appear in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test #5 — BC-3.4.010 regression pin: fake-endpoint literal absent
// ---------------------------------------------------------------------------
//
// Mirrors the JRACLOUD-95368 literal-pin pattern in CLAUDE.md.
// CrossHierarchy 400 path → stderr does NOT contain `jr api /rest/api/3/issue`.
// This is a dedicated regression pin for the removed fake-endpoint hint.

#[tokio::test]
async fn test_edit_type_cross_hierarchy_hint_no_fake_endpoint_literal() {
    let server = start_server().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-5"))
        .respond_with(ResponseTemplate::new(400).set_body_json(edit_400_body()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-5"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(get_issue_standard("TEST-5", "TEST")),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/TEST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(get_project_types_with_subtask_target()),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-5",
            "--type",
            "Sub-task",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Primary regression pin: the removed fake-endpoint hint MUST NOT appear.
    // The prior hint text was: `jr api /rest/api/3/issue/{key}/convert -X put -d ...`
    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint ('jr api /rest/api/3/issue') must not appear in stderr; stderr={stderr}"
    );

    // Confirm the cross-hierarchy hint DID appear (so the test is not vacuously passing).
    assert!(
        stderr.contains("JRACLOUD-27893"),
        "Expected JRACLOUD-27893 in stderr to confirm CrossHierarchy path was taken; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test #6 — BC-3.4.011 Indeterminate Cause-2: source `subtask` field absent
// ---------------------------------------------------------------------------
//
// GET issue returns 200 with `issuetype: {"name":"Task"}` — NO `subtask` key.
// Deserializes to `IssueType { name: "Task", subtask: None }` via `#[serde(default)]`.
// `is_cross_hierarchy_type_error(None, _, _)` → Indeterminate.
// Expected: exit 1; extracted 400 message on stderr; no enrichment hint;
//           `JRACLOUD-27893` MUST NOT appear; `jr api /rest/api/3/issue` MUST NOT appear.
// (EC-3.4.011-5)

#[tokio::test]
async fn test_edit_type_indeterminate_absent_subtask_flag_surfaces_raw_error() {
    let server = start_server().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-6"))
        .respond_with(ResponseTemplate::new(400).set_body_json(edit_400_body()))
        .mount(&server)
        .await;

    // GET issue: issuetype present but `subtask` key omitted → subtask: None.
    // `.expect(1)` — pre-impl this is never called (no enrichment dispatch)
    // → wiremock verify fails on server drop, making the test genuinely red.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-6"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(get_issue_no_subtask_field("TEST-6", "TEST")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // GET project types is mounted with `.expect(1)`.
    // The classifier MUST call this even when src_subtask is None (the spec
    // requires fetching project types first, then determining Indeterminate
    // from the combination of src/tgt flags). Pre-impl: never called →
    // verify fails on drop.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/TEST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(get_project_types_standard_only()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "TEST-6", "--type", "Task"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for Indeterminate (absent source subtask flag); stderr={stderr}"
    );

    // The extracted 400 message text must be present — no enrichment hint.
    assert!(
        stderr.contains("issue type selected is invalid")
            || stderr.contains("The issue type selected is invalid"),
        "Expected extracted 400 message substring in stderr (Indeterminate Cause-2 source-side); stderr={stderr}"
    );

    assert!(
        !stderr.contains("JRACLOUD-27893"),
        "JRACLOUD-27893 must NOT appear on Indeterminate path; stderr={stderr}"
    );

    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint must not appear in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test #7 — BC-3.4.011 Indeterminate Cause-2: target `subtask` field absent
// ---------------------------------------------------------------------------
//
// GET issue returns 200 with source `subtask: Some(false)`.
// GET project types returns 200 with matched target type's `subtask` key OMITTED.
// Deserializes to `IssueTypeMetadata { name: "Task", subtask: None }`.
// `is_cross_hierarchy_type_error(Some(false), None, _)` → Indeterminate.
// Expected: same as test #6. (EC-3.4.011-6)

#[tokio::test]
async fn test_edit_type_indeterminate_absent_target_subtask_flag_surfaces_raw_error() {
    let server = start_server().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-7"))
        .respond_with(ResponseTemplate::new(400).set_body_json(edit_400_body()))
        .mount(&server)
        .await;

    // GET issue: source subtask: Some(false).
    // `.expect(1)` — pre-impl this is never called (no enrichment dispatch)
    // → wiremock verify fails on server drop, making the test genuinely red.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-7"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(get_issue_standard("TEST-7", "TEST")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // GET project types: target type's `subtask` key OMITTED → tgt_subtask: None.
    // `.expect(1)` — pre-impl this is never called → verify fails on drop.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/TEST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(get_project_types_target_no_subtask_field()),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "TEST-7", "--type", "Task"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for Indeterminate (absent target subtask flag); stderr={stderr}"
    );

    assert!(
        stderr.contains("issue type selected is invalid")
            || stderr.contains("The issue type selected is invalid"),
        "Expected extracted 400 message substring in stderr (Indeterminate Cause-2 target-side); stderr={stderr}"
    );

    assert!(
        !stderr.contains("JRACLOUD-27893"),
        "JRACLOUD-27893 must NOT appear on Indeterminate path; stderr={stderr}"
    );

    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint must not appear in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test #8 — BC-3.4.011 unresolvable-name sub-path: typo hint emitted
// ---------------------------------------------------------------------------
//
// GET issue returns 200 with source `subtask: Some(false)`.
// GET project types returns 200 with a list that does NOT contain `--type "Taks"`.
// Expected: unresolvable-name route → typo hint (caller-side, classifier NOT invoked);
//           exit 1; stderr contains `jr project types`;
//           `JRACLOUD-27893` MUST NOT appear; `jr api /rest/api/3/issue` MUST NOT appear.
// (EC-3.4.011-3, EC-3.4.011-7)

#[tokio::test]
async fn test_edit_type_unresolved_type_name_surfaces_typo_hint() {
    let server = start_server().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-8"))
        .respond_with(ResponseTemplate::new(400).set_body_json(edit_400_body()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-8"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(get_issue_standard("TEST-8", "TEST")),
        )
        .mount(&server)
        .await;

    // Project types list contains "Story", "Bug", "Task" — does NOT contain "Taks".
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/TEST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(get_project_types_no_target_match()))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "TEST-8", "--type", "Taks"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected exit 1 for unresolvable type name; stderr={stderr}"
    );

    // Unresolvable-name sub-path emits the typo hint.
    assert!(
        stderr.contains("jr project types"),
        "Expected typo hint containing 'jr project types' in stderr; stderr={stderr}"
    );

    assert!(
        !stderr.contains("JRACLOUD-27893"),
        "JRACLOUD-27893 must NOT appear on unresolvable-name path; stderr={stderr}"
    );

    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint must not appear in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test #9 — BC-3.4.011 Indeterminate Cause-1 R1: GET issue fails
// ---------------------------------------------------------------------------
//
// PUT → 400; GET issue → 5xx (any error) → `Result::is_err()` → Indeterminate immediately.
// `get_project_issue_types` is NOT called (no mock mounted for it).
// Expected: exit nonzero; raw error on stderr; no enrichment hint;
//           `JRACLOUD-27893` MUST NOT appear; `jr api /rest/api/3/issue` MUST NOT appear.
// Distinct from test #4 (R2): test #4 has GET issue succeed then GET project types fail.
// (EC-3.4.011-4)

#[tokio::test]
async fn test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error() {
    let server = start_server().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-9"))
        .respond_with(ResponseTemplate::new(400).set_body_json(edit_400_body()))
        .mount(&server)
        .await;

    // GET issue → 5xx → `is_err()` → Indeterminate; get_project_issue_types NOT called.
    // `.expect(1)` — pre-impl this is never called (no enrichment dispatch)
    // → wiremock verify fails on server drop, making the test genuinely red.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-9"))
        .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({
            "errorMessages": ["Service unavailable."],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    // GET project types MUST NOT be called once GET issue fails — this asserts
    // the get_issue-first ordering invariant. `.expect(0)` verifies it is never
    // reached both pre-impl (correct) and post-impl (must stay correct).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/TEST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "TEST",
            "name": "Test Project",
            "issueTypes": []
        })))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "TEST-9", "--type", "Task"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for Indeterminate R1 (GET issue fails); stderr={stderr}"
    );

    // No cross-hierarchy hint (JRACLOUD-27893).
    assert!(
        !stderr.contains("JRACLOUD-27893"),
        "JRACLOUD-27893 must NOT appear when GET issue fails (Indeterminate R1); stderr={stderr}"
    );

    // No typo hint.
    assert!(
        !stderr.contains("jr project types"),
        "Typo hint must NOT appear when GET issue fails (Indeterminate R1); stderr={stderr}"
    );

    // Regression pin.
    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint must not appear in stderr; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test #10 — BC-3.4.010/011 R0b: non-400 edit error, no enrichment
// ---------------------------------------------------------------------------
//
// PUT → 403 (non-400). HTTP-400 gate not entered; NO enrichment fetches.
// `get_issue` and `get_project_issue_types` are NOT called.
// Expected: exit nonzero; raw error on stderr; no enrichment hint of any kind;
//           `JRACLOUD-27893` MUST NOT appear; `jr api /rest/api/3/issue` MUST NOT appear.

#[tokio::test]
async fn test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment() {
    let server = start_server().await;

    // REGRESSION-GUARD TEST — deliberate Red Gate exception.
    //
    // This test verifies that the new `if is_400` gate added by the implementer
    // does NOT accidentally enrich non-400 errors. The baseline already satisfies
    // the behavioural assertion (no enrichment for 403), so this test is green
    // pre- and post-implementation. The `.expect(0)` mocks below provide the
    // guard: if an implementer accidentally broadens the gate to cover 403, the
    // enrichment calls fire and the `.expect(0)` verification fails at server drop.

    // PUT → 403 (non-400 error → R0b routing row).
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-10"))
        .respond_with(ResponseTemplate::new(403).set_body_json(edit_403_body()))
        .expect(1) // exactly one PUT
        .mount(&server)
        .await;

    // GET issue enrichment MUST NOT be called for a non-400 response.
    // `.expect(0)` guards that the implementer's `if is_400` gate does not
    // over-fire and call the enrichment path on 403.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-10"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(get_issue_standard("TEST-10", "TEST")),
        )
        .expect(0)
        .mount(&server)
        .await;

    // GET project types enrichment MUST NOT be called for a non-400 response.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/TEST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(get_project_types_standard_only()))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "TEST-10", "--type", "Task"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for non-400 edit error (R0b); stderr={stderr}"
    );

    // No cross-hierarchy hint.
    assert!(
        !stderr.contains("JRACLOUD-27893"),
        "JRACLOUD-27893 must NOT appear for non-400 edit error (R0b); stderr={stderr}"
    );

    // No typo hint.
    assert!(
        !stderr.contains("jr project types"),
        "Typo hint must NOT appear for non-400 edit error (R0b); stderr={stderr}"
    );

    // Regression pin.
    assert!(
        !stderr.contains("jr api /rest/api/3/issue"),
        "Fake-endpoint hint must not appear in stderr; stderr={stderr}"
    );
}

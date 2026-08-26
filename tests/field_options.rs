//! Integration tests for `jr field options <field>` (issue #580, S-580-1).
//!
//! Red Gate suite (Step 3, strict TDD). `src/cli/field.rs::handle` is a
//! `todo!()` stub as of this pass — EVERY test below must currently FAIL by
//! panicking on that `todo!()` (surfaced by the subprocess as a non-zero,
//! non-64 exit code with a panic message on stderr), except
//! `test_bc_x_14_001_get_createmeta_fields_paginates_all_pages`'s underlying
//! HTTP-pagination mechanism (`JiraClient::get_createmeta_fields`), which is
//! ALREADY fully implemented in the stub — that single test is still Red
//! today (because `handle()` itself is `todo!()` and panics before ever
//! reaching the pagination logic), but is flagged as a REGRESSION PIN: once
//! `handle()` is implemented, this test's real job is to confirm the
//! pre-existing pagination code, not to validate new implementer work.
//!
//! Pattern mirrors `tests/requesttype_commands.rs`: subprocess (`assert_cmd`)
//! + wiremock + `JR_BASE_URL`/`JR_AUTH_HEADER` env overrides + isolated
//! `XDG_CONFIG_HOME`/`XDG_CACHE_HOME`/`JR_CONFIG_DIR`/`JR_CACHE_DIR` per test.
//!
//! Traces: BC-X.14.001..004, ADR-0019, VP-580-001..012,
//! `.factory/stories/S-580-1-field-options-command.md` AC-001..014.

use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ─── Shared test scaffolding ────────────────────────────────────────────────

/// Write a minimal jr config to a temp `XDG_CONFIG_HOME` so the subprocess
/// finds a URL while `JR_BASE_URL`/`JR_AUTH_HEADER` override the real values.
fn write_minimal_config(config_home: &std::path::Path, url: &str) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        format!("[instance]\nurl = \"{url}\"\n"),
    )
    .unwrap();
}

/// Write a config with a profile-level default `project` set — used by
/// AC-004's profile-default resolution tests.
fn write_config_with_profile_project(config_home: &std::path::Path, url: &str, project: &str) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        format!(
            "[instance]\nurl = \"{url}\"\n\n[profiles.default]\nproject = \"{project}\"\n"
        ),
    )
    .unwrap();
}

/// Test harness bundle: wiremock server + isolated cache/config temp dirs.
struct Harness {
    server: MockServer,
    cache_dir: tempfile::TempDir,
    config_dir: tempfile::TempDir,
}

impl Harness {
    async fn new() -> Self {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());
        Self {
            server,
            cache_dir,
            config_dir,
        }
    }

    async fn with_profile_project(project: &str) -> Self {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_config_with_profile_project(config_dir.path(), &server.uri(), project);
        Self {
            server,
            cache_dir,
            config_dir,
        }
    }

    fn cmd(&self, args: &[&str]) -> assert_cmd::assert::Assert {
        Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", self.server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("XDG_CACHE_HOME", self.cache_dir.path())
            .env("JR_CACHE_DIR", self.cache_dir.path().join("jr"))
            .env("XDG_CONFIG_HOME", self.config_dir.path())
            .env("JR_CONFIG_DIR", self.config_dir.path().join("jr"))
            .args(args)
            .assert()
    }
}

/// Strongest form of the zero-HTTP guarantee (mirrors
/// `tests/issue_commands.rs::s606_1_expect_zero_http`): registers catch-all
/// `.expect(0)` mocks for ANY GET and ANY POST. Used for the mode-selector
/// arity pre-flight guards, which BC-X.14.001 Invariant 1 documents as firing
/// BEFORE any HTTP call at all.
async fn expect_zero_http(server: &MockServer) {
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(server)
        .await;
}

async fn mount_project_meta(server: &MockServer, key: &str, id: &str, project_type: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/project/{key}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": id,
            "key": key,
            "projectTypeKey": project_type,
            "simplified": false
        })))
        .mount(server)
        .await;
}

async fn mount_project_not_found(server: &MockServer, key: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/project/{key}")))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "errorMessages": ["No project could be found with key 'NOPE'."],
            "errors": {}
        })))
        .mount(server)
        .await;
}

async fn mount_service_desk_list(server: &MockServer, project_id: &str, sd_id: &str) {
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "_links": {},
            "values": [
                {
                    "id": sd_id,
                    "projectId": project_id,
                    "projectKey": "IGNORED",
                    "projectName": "Ignored"
                }
            ]
        })))
        .mount(server)
        .await;
}

async fn mount_issue_types(server: &MockServer, project_key: &str, types: &[(&str, &str)]) {
    let issue_types: Vec<Value> = types
        .iter()
        .map(|(id, name)| json!({"id": id, "name": name}))
        .collect();
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/api/3/issue/createmeta/{project_key}/issuetypes"
        )))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issueTypes": issue_types,
            "startAt": 0,
            "maxResults": 200,
            "total": types.len()
        })))
        .mount(server)
        .await;
}

async fn mount_createmeta_fields_single_page(
    server: &MockServer,
    project_key: &str,
    issue_type_id: &str,
    fields: Vec<Value>,
) {
    let total = fields.len();
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/api/3/issue/createmeta/{project_key}/issuetypes/{issue_type_id}"
        )))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": fields,
            "startAt": 0,
            "maxResults": 200,
            "total": total
        })))
        .mount(server)
        .await;
}

async fn mount_editmeta(server: &MockServer, key: &str, fields: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}/editmeta")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "fields": fields })))
        .mount(server)
        .await;
}

async fn mount_editmeta_not_found(server: &MockServer, key: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}/editmeta")))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "errorMessages": ["Issue does not exist or you do not have permission to see it."],
            "errors": {}
        })))
        .mount(server)
        .await;
}

async fn mount_request_type_fields(
    server: &MockServer,
    sd_id: &str,
    rt_id: &str,
    fields: Vec<Value>,
) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/servicedeskapi/servicedesk/{sd_id}/requesttype/{rt_id}/field"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "canRaiseOnBehalfOf": false,
            "canAddRequestParticipants": false,
            "requestTypeFields": fields
        })))
        .mount(server)
        .await;
}

async fn mount_list_fields(server: &MockServer, fields: Vec<Value>) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(fields)))
        .mount(server)
        .await;
}

/// A well-formed `allowedValues`-shaped M2/M1 createmeta field descriptor for
/// `customfield_10084` with two options.
fn createmeta_field_10084() -> Value {
    json!({
        "fieldId": "customfield_10084",
        "name": "SOC Client",
        "schema": {"type": "option", "custom": "com.atlassian.jira.plugin.system.customfieldtypes:select", "system": null},
        "allowedValues": [
            {"id": "10001", "value": "Client A", "name": null},
            {"id": "10002", "value": "Client B", "name": null}
        ]
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-001 / AC-002 / AC-003 — mode-selector arity (BC-X.14.001 Invariant 1)
// ═══════════════════════════════════════════════════════════════════════════

/// AC-001 / AC-002: zero mode selectors -> exit 64, canonical message, ZERO
/// HTTP calls whatsoever.
#[tokio::test]
async fn test_bc_x_14_001_zero_mode_selectors_exits_64_zero_http() {
    let h = Harness::new().await;
    expect_zero_http(&h.server).await;

    let assert = h.cmd(&["field", "options", "customfield_10084", "--no-input"]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for zero mode selectors, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("specify exactly one of --type, --request-type, --issue"),
        "BC-X.14.001 Invariant 1: exact message pin; got: {stderr}"
    );
}

/// AC-002: a BARE `--project` alone (no mode selector at all) hits the SAME
/// zero-mode-selector row, NOT the incomplete-M2 row — `--project` is never
/// itself a mode selector.
#[tokio::test]
async fn test_bc_x_14_004_bare_project_no_mode_selector_is_zero_mode_error() {
    let h = Harness::new().await;
    expect_zero_http(&h.server).await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--project",
        "FOO",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(64));
    assert!(
        stderr.contains("specify exactly one of --type, --request-type, --issue"),
        "bare --project must land on the zero-mode-selector row, not incomplete-M2; got: {stderr}"
    );
    assert!(
        !stderr.contains("needs a resolvable project"),
        "must NOT be the incomplete-M2 message; got: {stderr}"
    );
}

/// AC-003: two-or-more mode selectors -> exit 64, same message, zero HTTP.
#[tokio::test]
async fn test_bc_x_14_001_two_mode_selectors_exits_64() {
    let h = Harness::new().await;
    expect_zero_http(&h.server).await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-1",
        "--request-type",
        "IT Help",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(64));
    assert!(
        stderr.contains("specify exactly one of --type, --request-type, --issue"),
        "two-or-more selectors must share the same message as zero-selectors; got: {stderr}"
    );
}

/// BC-X.14.004 precedence: all THREE flags present at once must still be
/// reported as "two or more" (not e.g. a --project companion error).
#[tokio::test]
async fn test_bc_x_14_004_all_three_mode_selectors_exits_64_precedence() {
    let h = Harness::new().await;
    expect_zero_http(&h.server).await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--project",
        "FOO",
        "--request-type",
        "IT Help",
        "--type",
        "Bug",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(64));
    assert!(stderr.contains("specify exactly one of --type, --request-type, --issue"));
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-004 / AC-005 — M2 project resolution + --type name→id resolution
// ═══════════════════════════════════════════════════════════════════════════

/// AC-004: `--type` present with NO resolvable project (no flag, no profile
/// default) -> exit 64, the WIDENED incomplete-M2 message, zero HTTP.
#[tokio::test]
async fn test_bc_x_14_004_m2_no_resolvable_project_exits_64_widened_message() {
    let h = Harness::new().await;
    expect_zero_http(&h.server).await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--type",
        "Bug",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(64));
    assert!(
        stderr.contains(
            "--type needs a resolvable project — pass --project <P> or configure a default"
        ),
        "BC-X.14.004 widened incomplete-M2 message pin; got: {stderr}"
    );
}

/// AC-004: `--type` with no `--project` flag but a profile/config default
/// project configured resolves without requiring the flag (VP-580-010).
#[tokio::test]
async fn test_bc_x_14_001_m2_resolves_via_profile_default_project() {
    let h = Harness::with_profile_project("HELP").await;
    mount_issue_types(&h.server, "HELP", &[("10000", "Bug")]).await;
    mount_createmeta_fields_single_page(&h.server, "HELP", "10000", vec![createmeta_field_10084()])
        .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--type",
        "Bug",
        "--no-input",
        "--output",
        "json",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0 using profile-default project, got {:?}. stderr: {stderr}",
        output.status.code()
    );
}

/// AC-005: `--type Bug` resolves to an `issueTypeId` via
/// `get_issue_types_for_project` (S-331) BEFORE calling
/// `get_createmeta_fields`, and the enumerated options are returned.
#[tokio::test]
async fn test_bc_x_14_001_m2_type_resolution_reused_from_s331() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "HELP", &[("10000", "Bug"), ("10001", "Task")]).await;
    mount_createmeta_fields_single_page(&h.server, "HELP", "10000", vec![createmeta_field_10084()])
        .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--type",
        "Bug",
        "--project",
        "HELP",
        "--no-input",
        "--output",
        "json",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("expected valid JSON, got {stdout}\nerror: {e}"));
    let arr = parsed.as_array().expect("expected a JSON array");
    assert_eq!(arr.len(), 2, "expected 2 normalized options, got: {parsed}");
    assert_eq!(arr[0]["id"], json!("10001"));
    assert_eq!(arr[1]["id"], json!("10002"));
}

/// AC-005 / EC-X.14.004-4: unknown `--type` name -> exit 64 listing valid
/// issue types, and `get_createmeta_fields`'s endpoint is NEVER called
/// (`.expect(0)`).
#[tokio::test]
async fn test_bc_x_14_004_ec_x_14_004_4_unknown_type_exits_64_before_createmeta() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "HELP", &[("10000", "Bug"), ("10001", "Task")]).await;

    Mock::given(method("GET"))
        .and(path(
            "/rest/api/3/issue/createmeta/HELP/issuetypes/10000",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&h.server)
        .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--type",
        "NonExistentType",
        "--project",
        "HELP",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for unknown --type, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Bug") && stderr.contains("Task"),
        "stderr must list the project's valid issue type names; got: {stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-006 — M3 --request-type [--project] resolution
// ═══════════════════════════════════════════════════════════════════════════

/// AC-006: `--project --request-type` together is a VALID invocation, NOT a
/// pairing error (regression pin, adversary pass-20 M1).
#[tokio::test]
async fn test_bc_x_14_001_m3_project_request_type_together_is_valid() {
    let h = Harness::new().await;
    mount_project_meta(&h.server, "HELP", "99", "service_desk").await;
    mount_service_desk_list(&h.server, "99", "10").await;
    mount_request_type_fields(
        &h.server,
        "10",
        "11001",
        vec![json!({
            "fieldId": "customfield_10084",
            "name": "SOC Client",
            "description": null,
            "required": false,
            "jiraSchema": {"type": "option"},
            "validValues": [
                {"value": "10001", "label": "Client A"},
                {"value": "10002", "label": "Client B"}
            ],
            "visible": true
        })],
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--request-type",
        "11001",
        "--project",
        "HELP",
        "--no-input",
        "--output",
        "json",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0 for M3 --project + --request-type, got {:?}. stderr: {stderr}",
        output.status.code()
    );
}

/// AC-006: a resolved project that is non-JSM (software) supplied on the M3
/// path -> exit 64 via `require_service_desk`'s call-site-specific message.
#[tokio::test]
async fn test_bc_x_14_001_m3_non_jsm_project_exits_64_require_service_desk() {
    let h = Harness::new().await;
    mount_project_meta(&h.server, "SW", "100", "software").await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--request-type",
        "11001",
        "--project",
        "SW",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(64));
    assert!(
        stderr.contains("Jira Service Management project"),
        "expected require_service_desk's non-JSM message; got: {stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-007 — M1 --issue resolution (reuses get_editmeta verbatim)
// ═══════════════════════════════════════════════════════════════════════════

/// AC-007: `--issue <KEY>` reuses `get_editmeta` verbatim.
#[tokio::test]
async fn test_bc_x_14_001_m1_issue_reuses_get_editmeta() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10084": {
                "name": "SOC Client",
                "schema": {"type": "option"},
                "allowedValues": [
                    {"id": "10001", "value": "Client A"},
                    {"id": "10002", "value": "Client B"}
                ],
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-1",
        "--no-input",
        "--output",
        "json",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 2);
}

/// AC-007: a stray `--project` alongside `--issue` is harmlessly ignored —
/// the command still succeeds via M1, `--project` is never consulted.
#[tokio::test]
async fn test_bc_x_14_001_m1_stray_project_harmlessly_ignored() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10084": {
                "name": "SOC Client",
                "schema": {"type": "option"},
                "allowedValues": [{"id": "10001", "value": "Client A"}],
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;
    // No project-meta mock mounted at all — if `--project` were consulted on
    // the M1 path, the command would fail trying to resolve it.

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-1",
        "--project",
        "SOMEPROJECT",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "a stray --project alongside --issue must be harmlessly ignored; got {:?}. stderr: {stderr}",
        output.status.code()
    );
}

/// AC-007 / EC-3.4.015-7 parallel: `--issue <KEY>` not found (404) -> exit
/// 64, "issue not found or not accessible".
#[tokio::test]
async fn test_bc_x_14_004_m1_issue_not_found_exits_64() {
    let h = Harness::new().await;
    mount_editmeta_not_found(&h.server, "FOO-999").await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-999",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(64));
    assert!(
        stderr.contains("issue not found or not accessible") || stderr.contains("not found"),
        "expected an issue-not-found message; got: {stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-008 — get_createmeta_fields offset-pagination (regression pin — the
// pagination logic itself is ALREADY implemented in the S-580-1 stub).
// ═══════════════════════════════════════════════════════════════════════════

/// AC-008: a target field on createmeta fields-page >=2 is collected and
/// resolves, NOT dropped. Page 1 returns ONE unrelated field (total=2);
/// page 2 (`startAt=1`) returns the target `customfield_10084` field.
#[tokio::test]
async fn test_bc_x_14_001_get_createmeta_fields_paginates_all_pages() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "HELP", &[("10000", "Bug")]).await;

    // Page 1: one unrelated field, total = 2.
    Mock::given(method("GET"))
        .and(path(
            "/rest/api/3/issue/createmeta/HELP/issuetypes/10000",
        ))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": [{
                "fieldId": "summary",
                "name": "Summary",
                "schema": {"type": "string"},
                "allowedValues": null
            }],
            "startAt": 0,
            "maxResults": 200,
            "total": 2
        })))
        .mount(&h.server)
        .await;

    // Page 2 (startAt derived from actual page-1 length, 1): target field.
    Mock::given(method("GET"))
        .and(path(
            "/rest/api/3/issue/createmeta/HELP/issuetypes/10000",
        ))
        .and(query_param("startAt", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": [createmeta_field_10084()],
            "startAt": 1,
            "maxResults": 200,
            "total": 2
        })))
        .mount(&h.server)
        .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--type",
        "Bug",
        "--project",
        "HELP",
        "--no-input",
        "--output",
        "json",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "expected exit 0 (target field on page 2 must still resolve), got {:?}. stderr: {stderr}",
        output.status.code()
    );
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 2, "expected the page-2 field's 2 options, got: {parsed}");
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-011 — <field> resolution (customfield_NNNNN bypass / list_fields + partial_match)
// ═══════════════════════════════════════════════════════════════════════════

/// AC-011: `customfield_10084` literal bypasses `list_fields()` entirely
/// (zero HTTP to `/rest/api/3/field`).
#[tokio::test]
async fn test_bc_x_14_001_customfield_bypass_skips_list_fields() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10084": {
                "name": "SOC Client",
                "schema": {"type": "option"},
                "allowedValues": [{"id": "10001", "value": "Client A"}],
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(0)
        .mount(&h.server)
        .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
}

/// AC-011 / EC-X.14.001-3: a human field name resolving to MULTIPLE
/// candidates -> exit 64 naming the candidates and their `customfield_NNNNN`
/// ids, before any enumeration HTTP call.
#[tokio::test]
async fn test_bc_x_14_001_field_name_ambiguous_exits_64() {
    let h = Harness::new().await;
    mount_list_fields(
        &h.server,
        vec![
            json!({"id": "customfield_10084", "name": "SOC Client A", "custom": true, "schema": null}),
            json!({"id": "customfield_10085", "name": "SOC Client B", "custom": true, "schema": null}),
        ],
    )
    .await;
    // No editmeta mock — must never be reached.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"fields": {}})))
        .expect(0)
        .mount(&h.server)
        .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "SOC Client",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for ambiguous field name, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("customfield_10084") && stderr.contains("customfield_10085"),
        "ambiguous-field error must list candidate customfield_NNNNN ids; got: {stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-013 — output shape (table / --output json)
// ═══════════════════════════════════════════════════════════════════════════

/// AC-013: default table output has exactly the ID / Label columns;
/// cascading children render as additional indented rows.
#[tokio::test]
async fn test_bc_x_14_003_table_two_columns_cascading_indent() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10084": {
                "name": "SOC Client",
                "schema": {"type": "option-with-child"},
                "allowedValues": [
                    {"id": "p1", "value": "Parent", "children": [
                        {"id": "c1", "value": "Child"}
                    ]}
                ],
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(stdout.contains("ID"), "table must have an ID column header; got: {stdout}");
    assert!(
        stdout.contains("Label"),
        "table must have a Label column header; got: {stdout}"
    );
    assert!(stdout.contains("Parent"));
    assert!(stdout.contains("Child"));
}

/// AC-013: `--output json` returns the normalized `{id, label, children}`
/// array, pretty-printed via `render_json` (JSON render invariant #526).
#[tokio::test]
async fn test_bc_x_14_003_json_array_shape_render_json_invariant() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10084": {
                "name": "SOC Client",
                "schema": {"type": "option"},
                "allowedValues": [{"id": "10001", "value": "Client A"}],
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-1",
        "--no-input",
        "--output",
        "json",
    ]);
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    // JSON render invariant #526: output must be pretty-printed (multi-line,
    // 2-space indent), never compact `json!` Display output on one line.
    assert!(
        stdout.contains("\n  "),
        "expected pretty-printed JSON (render_json invariant #526); got: {stdout}"
    );
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr[0]["id"], json!("10001"));
    assert_eq!(arr[0]["label"], json!("Client A"));
    assert_eq!(arr[0]["children"], json!([]));
}

/// AC-013 / ADR-0019 Amendment F-B: table-mode degenerate-entry glyphs —
/// missing id -> NULL_GLYPH ("—"), missing label -> "(unnamed)".
#[tokio::test]
async fn test_bc_x_14_003_degenerate_entry_table_glyphs() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10084": {
                "name": "SOC Client",
                "schema": {"type": "option"},
                "allowedValues": [
                    {"id": null, "value": "Has Label"},
                    {"id": "10002", "value": null}
                ],
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains('—'),
        "missing id must render NULL_GLYPH (\"—\") in table mode; got: {stdout}"
    );
    assert!(
        stdout.contains("(unnamed)"),
        "missing label must render the literal \"(unnamed)\" in table mode; got: {stdout}"
    );
}

/// AC-013 / ADR-0019 Amendment F-B: `--output json` performs NO substitution
/// for a degenerate entry — raw `null`, never the table-mode glyphs.
#[tokio::test]
async fn test_bc_x_14_003_degenerate_entry_json_emits_null_not_glyph() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10084": {
                "name": "SOC Client",
                "schema": {"type": "option"},
                "allowedValues": [{"id": null, "value": null}],
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-1",
        "--no-input",
        "--output",
        "json",
    ]);
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1, "degenerate entry must never be dropped");
    assert_eq!(arr[0]["id"], Value::Null);
    assert_eq!(arr[0]["label"], Value::Null);
    assert!(
        !stdout.contains('—') && !stdout.contains("(unnamed)"),
        "JSON mode must never emit the table-mode glyphs; got: {stdout}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-014 — error taxonomy + graceful degradation (BC-X.14.004)
// ═══════════════════════════════════════════════════════════════════════════

/// VP-580-012: `--project NONEXISTENT` (404, not 401) on the M3 path -> exit
/// 64, "project not found or not accessible".
#[tokio::test]
async fn test_bc_x_14_004_project_404_exits_64_message() {
    let h = Harness::new().await;
    mount_project_not_found(&h.server, "NOPE").await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--request-type",
        "11001",
        "--project",
        "NOPE",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for a 404 project, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("project not found or not accessible"),
        "VP-580-012: exact message pin; got: {stderr}"
    );
}

/// Graceful degradation: an Assets/CMDB object field with no `allowedValues`
/// -> exit 0, "no enumerable options — this field uses Assets" hint.
#[tokio::test]
async fn test_bc_x_14_004_graceful_degrade_assets_field() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10084": {
                "name": "Affected Assets",
                "schema": {
                    "type": "object",
                    "custom": "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype"
                },
                "allowedValues": null,
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "graceful degrade must exit 0, not error; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("no enumerable options") && stderr.contains("Assets"),
        "expected the Assets-specific degrade hint on stderr; got: {stderr}"
    );
    assert!(
        stdout.trim().is_empty() || stdout.contains("No results"),
        "table-mode graceful degrade stdout should be empty/no-results; got: {stdout}"
    );
}

/// Graceful degradation: a user-picker/suggestion-backed field -> exit 0,
/// "no enumerable options (dynamic/lookup field)" hint.
#[tokio::test]
async fn test_bc_x_14_004_graceful_degrade_userpicker_field() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10000": {
                "name": "Nominee",
                "schema": {
                    "type": "user",
                    "custom": "com.atlassian.jira.plugin.system.customfieldtypes:userpicker"
                },
                "allowedValues": null,
                "autoCompleteUrl": "https://example.atlassian.net/rest/api/1.0/users/picker",
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10000",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "expected exit 0; stderr: {stderr}");
    assert!(
        stderr.contains("no enumerable options") && stderr.contains("dynamic"),
        "expected the dynamic/lookup-field degrade hint; got: {stderr}"
    );
}

/// Graceful degradation: a free-text field -> exit 0, "no enumerable options
/// (this field type has no fixed value set)" hint, no `autoCompleteUrl`.
#[tokio::test]
async fn test_bc_x_14_004_graceful_degrade_freetext_field() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10099": {
                "name": "Free Text",
                "schema": {"type": "string"},
                "allowedValues": null,
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10099",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "expected exit 0; stderr: {stderr}");
    assert!(
        stderr.contains("no enumerable options") && stderr.contains("no fixed value set"),
        "expected the free-text degrade hint; got: {stderr}"
    );
}

/// EC-X.14.004-2: `--output json` graceful-degrade -> `[]` on stdout, hint
/// on stderr (never folded into the JSON payload).
#[tokio::test]
async fn test_bc_x_14_004_graceful_degrade_json_mode_empty_array_stderr_hint() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10099": {
                "name": "Free Text",
                "schema": {"type": "string"},
                "allowedValues": null,
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10099",
        "--issue",
        "FOO-1",
        "--no-input",
        "--output",
        "json",
    ]);
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "expected exit 0; stderr: {stderr}");
    let parsed: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("expected valid JSON `[]`, got {stdout}\nerror: {e}"));
    assert_eq!(parsed, json!([]), "graceful-degrade stdout must be an empty JSON array");
    assert!(
        stderr.contains("no enumerable options"),
        "hint text must still be on stderr in JSON mode; got: {stderr}"
    );
}

/// EC-X.14.004-1 / M3-specific note: JSM Assets fields return
/// `validValues: []` unconditionally (JSDCLOUD-15551) — treated identically
/// to the Assets degrade case, never as a misconfiguration error.
#[tokio::test]
async fn test_bc_x_14_004_m3_assets_empty_validvalues_is_degrade_not_misconfig() {
    let h = Harness::new().await;
    mount_project_meta(&h.server, "HELP", "99", "service_desk").await;
    mount_service_desk_list(&h.server, "99", "10").await;
    mount_request_type_fields(
        &h.server,
        "10",
        "11001",
        vec![json!({
            "fieldId": "customfield_10077",
            "name": "Affected Services",
            "description": null,
            "required": false,
            "jiraSchema": {
                "type": "object",
                "custom": "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype"
            },
            "validValues": [],
            "visible": true
        })],
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10077",
        "--request-type",
        "11001",
        "--project",
        "HELP",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "M3 empty validValues on an Assets field must degrade gracefully (exit 0), not error; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Assets"),
        "expected the Assets-specific degrade hint, not a generic misconfiguration message; got: {stderr}"
    );
}

/// VP-580-011: `--value` supplied alongside a field with no enumerable
/// options still exits 0 with the graceful-degrade hint — identical to the
/// no-`--value` case.
#[tokio::test]
async fn test_bc_x_14_002_value_with_graceful_degrade_still_hints() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10099": {
                "name": "Free Text",
                "schema": {"type": "string"},
                "allowedValues": null,
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10099",
        "--issue",
        "FOO-1",
        "--value",
        "anything",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "expected exit 0; stderr: {stderr}");
    assert!(
        stderr.contains("no enumerable options"),
        "--value must not suppress the graceful-degrade hint; got: {stderr}"
    );
}

/// EC-X.14.001-5 / EC-X.14.004-3: `<field>` resolves globally but is ABSENT
/// from the selected M1 context's field set -> exit 64 with a per-context
/// message, BEFORE `allowedValues` is even inspected.
#[tokio::test]
async fn test_bc_x_14_004_field_absent_from_editmeta_context_exits_64() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "summary": {
                "name": "Summary",
                "schema": {"type": "string"},
                "allowedValues": null,
                "operations": ["set"],
                "required": true
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for a field absent from the editmeta field set, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Edit screen") || stderr.contains("not available"),
        "expected a per-context 'not on the Edit screen' style message; got: {stderr}"
    );
}

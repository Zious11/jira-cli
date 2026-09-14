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
//!   `XDG_CONFIG_HOME`/`XDG_CACHE_HOME`/`JR_CONFIG_DIR`/`JR_CACHE_DIR` per test.
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

/// Write a config already in the post-migration multi-profile shape
/// (`default_profile` plus `[profiles.default]`, no legacy `[instance]`
/// block) — avoids the one-time `"Migrated config to multi-profile layout"`
/// `eprintln!` that `write_minimal_config`'s legacy `[instance]` shape
/// triggers on first load. Used by stderr-emptiness assertions where that
/// migration notice would be a false-positive stderr leak unrelated to the
/// command under test.
fn write_premigrated_config(config_home: &std::path::Path, url: &str) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        format!("default_profile = \"default\"\n\n[profiles.default]\nurl = \"{url}\"\n"),
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
        format!("[instance]\nurl = \"{url}\"\n\n[profiles.default]\nproject = \"{project}\"\n"),
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

/// M2 project-404: `GET .../createmeta/{project}/issuetypes` 404s (distinct
/// from the M3 project-404 path, which goes through `GET
/// /rest/api/3/project/{key}` via `mount_project_not_found` — VP-580-012
/// covers BOTH enumeration paths, not just one).
async fn mount_issue_types_not_found(server: &MockServer, project_key: &str) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/api/3/issue/createmeta/{project_key}/issuetypes"
        )))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "errorMessages": [format!("No project could be found with key '{project_key}'.")],
            "errors": {}
        })))
        .mount(server)
        .await;
}

/// `GET /rest/servicedeskapi/servicedesk/{sd}/requesttype` — the M3
/// `resolve_request_type_id` non-numeric name-resolution list call.
async fn mount_list_request_types(server: &MockServer, sd_id: &str, types: &[(&str, &str)]) {
    let values: Vec<Value> = types
        .iter()
        .map(|(id, name)| {
            json!({
                "id": id,
                "name": name,
                "description": null,
                "helpText": null,
                "issueTypeId": null,
                "groupIds": []
            })
        })
        .collect();
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/servicedeskapi/servicedesk/{sd_id}/requesttype"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": types.len(),
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "_links": {},
            "values": values
        })))
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
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "expected exit 0 using profile-default project, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    // Content assertion (S-580-1 convergence pass): the JSON body must be
    // the actual enumerated options resolved via the profile-default
    // project, not merely a successful exit code.
    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("expected valid JSON, got {stdout}\nerror: {e}"));
    let arr = parsed.as_array().expect("expected a JSON array");
    assert_eq!(arr.len(), 2, "expected 2 normalized options, got: {parsed}");
    assert_eq!(arr[0]["id"], json!("10001"));
    assert_eq!(arr[0]["label"], json!("Client A"));
    assert_eq!(arr[1]["id"], json!("10002"));
    assert_eq!(arr[1]["label"], json!("Client B"));
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
        .and(path("/rest/api/3/issue/createmeta/HELP/issuetypes/10000"))
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

/// VP-580-012 (M2 half): `--project NOPE --type <T>` where the FIRST
/// createmeta-family call (`get_issue_types_for_project`'s own
/// `GET .../createmeta/{project}/issuetypes`) 404s -> exit 64, "project not
/// found or not accessible", and `get_createmeta_fields`'s own endpoint is
/// never reached (`.expect(0)`). Distinct from the pre-existing
/// `test_bc_x_14_004_project_404_exits_64_message`, which only covers the
/// M3 half of this VP via `get_or_fetch_project_meta`.
#[tokio::test]
async fn test_bc_x_14_004_m2_project_404_exits_64_message() {
    let h = Harness::new().await;
    mount_issue_types_not_found(&h.server, "NOPE").await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/NOPE/issuetypes/10000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&h.server)
        .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--type",
        "Bug",
        "--project",
        "NOPE",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for a 404 project on the M2 path, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("project not found or not accessible"),
        "VP-580-012 (M2 half): exact message pin; got: {stderr}"
    );
}

/// B-M2 (round-4 mutation-coverage sweep): `--type` resolves the
/// issueTypeId successfully (`get_issue_types_for_project` 200s), but the
/// SUBSEQUENT `get_createmeta_fields` call 400s — a TOCTOU (the issue type
/// existed a moment ago; the createmeta-fields-by-issue-type endpoint
/// itself now rejects the request). This propagates as a raw
/// `JrError::ApiError` (exit 1), NOT `JrError::UserError` (exit 64) —
/// pinning that `get_createmeta_fields`'s call site in `handle()`
/// (`Mode::Createmeta` arm, `src/cli/field.rs`) is deliberately NOT wrapped
/// in `map_project_not_found`, unlike the sibling `get_issue_types_for_project`
/// call one statement above it.
#[tokio::test]
async fn test_bc_x_14_004_createmeta_fields_400_exits_1_not_64() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "HELP", &[("10000", "Bug")]).await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/HELP/issuetypes/10000"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "errorMessages": ["The issue type selected is invalid."],
            "errors": {}
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
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "get_createmeta_fields 400 must propagate as a raw ApiError (exit 1), \
         NOT exit 64 (it is deliberately not wrapped in map_project_not_found); \
         got {:?}. stderr: {stderr}",
        output.status.code()
    );
}

/// B-M2 5xx variant (round-4 mutation-coverage sweep): a transient
/// `get_createmeta_fields` 500 must ALSO propagate as exit 1 (`ApiError`),
/// not exit 64 — same taxonomy pin as the 400 case above, over the other
/// half of the `map_err`-vs-propagate distinction (client vs server error).
#[tokio::test]
async fn test_bc_x_14_004_createmeta_fields_500_exits_1_not_64() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "HELP", &[("10000", "Bug")]).await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/HELP/issuetypes/10000"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "errorMessages": ["Internal server error."],
            "errors": {}
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
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "get_createmeta_fields 500 must propagate as a raw ApiError (exit 1), \
         got {:?}. stderr: {stderr}",
        output.status.code()
    );
}

/// EC-X.14.001-5 / EC-X.14.004-3 (M2 half): `<field>` resolves globally but
/// is ABSENT from the createmeta Create-screen field set for the resolved
/// project + issue type -> exit 64, per-context "not available for issue
/// type" message. The M1 (editmeta) sibling of this row is already covered
/// by `test_bc_x_14_004_field_absent_from_editmeta_context_exits_64`.
#[tokio::test]
async fn test_bc_x_14_004_field_absent_from_createmeta_context_exits_64() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "HELP", &[("10000", "Bug")]).await;
    // The Create screen exists but does not carry customfield_10084 —
    // only an unrelated "summary" field.
    mount_createmeta_fields_single_page(
        &h.server,
        "HELP",
        "10000",
        vec![json!({
            "fieldId": "summary",
            "name": "Summary",
            "schema": {"type": "string"},
            "allowedValues": null
        })],
    )
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
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for a field absent from the createmeta field set, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("not available") || stderr.contains("Create screen"),
        "expected a per-context 'not available on the Create screen' style message; got: {stderr}"
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
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "expected exit 0 for M3 --project + --request-type, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    // Content assertion (S-580-1 convergence pass): the JSON body must be
    // the actual enumerated M3 options, not merely a successful exit code.
    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("expected valid JSON, got {stdout}\nerror: {e}"));
    let arr = parsed.as_array().expect("expected a JSON array");
    assert_eq!(arr.len(), 2, "expected 2 normalized options, got: {parsed}");
    assert_eq!(arr[0]["id"], json!("10001"));
    assert_eq!(arr[0]["label"], json!("Client A"));
    assert_eq!(arr[1]["id"], json!("10002"));
    assert_eq!(arr[1]["label"], json!("Client B"));
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

/// AC-006 / EC-X.14.004-5: `--request-type` with NO `--project` companion
/// and no profile/config default -> exit 64 before `require_service_desk`
/// (or any request-type-fields HTTP) is ever reached — the field.rs-local
/// "needs a resolvable project" guard fires first (mirrors the M2 M1
/// incomplete-project taxonomy row, but on the M3 dispatch arm).
#[tokio::test]
async fn test_bc_x_14_004_m3_no_resolvable_project_exits_64() {
    let h = Harness::new().await;
    expect_zero_http(&h.server).await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--request-type",
        "11001",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for --request-type with no resolvable project, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("--request-type") && stderr.contains("resolvable project"),
        "expected a 'needs a resolvable project' style message; got: {stderr}"
    );
}

/// AC-006 / M3 `resolve_request_type_id` non-numeric name resolution — the
/// `partial_match::Exact` branch: a single unambiguous name resolves to its
/// request-type id, mirroring `jr requesttype fields`'s own name resolution.
/// All pre-existing M3 tests use the all-ASCII-digit numeric bypass; this is
/// the first to exercise the actual `partial_match` name-resolution path.
#[tokio::test]
async fn test_bc_x_14_001_m3_request_type_name_exact_match_resolves() {
    let h = Harness::new().await;
    mount_project_meta(&h.server, "HELP", "99", "service_desk").await;
    mount_service_desk_list(&h.server, "99", "10").await;
    mount_list_request_types(
        &h.server,
        "10",
        &[("11001", "Get IT Help"), ("11002", "Password Reset")],
    )
    .await;
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
            "validValues": [{"value": "10001", "label": "Client A"}],
            "visible": true
        })],
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--request-type",
        "Get IT Help",
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
        "expected exit 0 for an unambiguous request-type name, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], json!("10001"));
}

/// AC-006 / M3 `resolve_request_type_id` — the `partial_match::Ambiguous`
/// branch: a substring matching MULTIPLE request-type names -> exit 64
/// naming the request-type-list fallback, and `get_request_type_fields` is
/// never called (`.expect(0)`).
#[tokio::test]
async fn test_bc_x_14_001_m3_request_type_name_ambiguous_exits_64() {
    let h = Harness::new().await;
    mount_project_meta(&h.server, "HELP", "99", "service_desk").await;
    mount_service_desk_list(&h.server, "99", "10").await;
    mount_list_request_types(
        &h.server,
        "10",
        &[("11001", "Get IT Help"), ("11002", "Get HR Help")],
    )
    .await;
    Mock::given(method("GET"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/10/requesttype/11001/field",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "canRaiseOnBehalfOf": false,
            "canAddRequestParticipants": false,
            "requestTypeFields": []
        })))
        .expect(0)
        .mount(&h.server)
        .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--request-type",
        "Get",
        "--project",
        "HELP",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for an ambiguous request-type name, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Ambiguous") || stderr.contains("ambiguous"),
        "expected an ambiguous-request-type message; got: {stderr}"
    );
}

/// AC-006 / M3 `resolve_request_type_id` — the `partial_match::None` branch:
/// a name matching zero request types -> exit 64 naming the
/// `jr requesttype list` fallback.
#[tokio::test]
async fn test_bc_x_14_001_m3_request_type_name_zero_match_exits_64() {
    let h = Harness::new().await;
    mount_project_meta(&h.server, "HELP", "99", "service_desk").await;
    mount_service_desk_list(&h.server, "99", "10").await;
    mount_list_request_types(&h.server, "10", &[("11001", "Get IT Help")]).await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10084",
        "--request-type",
        "NonExistentRequestType",
        "--project",
        "HELP",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for a zero-match request-type name, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("not found") || stderr.contains("requesttype list"),
        "expected a request-type-not-found message; got: {stderr}"
    );
}

/// EC-X.14.001-5 / EC-X.14.004-3 (M3 half): `<field>` resolves globally but
/// is ABSENT from the resolved request type's `validValues`/field set ->
/// exit 64, per-context "not a field on this request type" message.
#[tokio::test]
async fn test_bc_x_14_004_field_absent_from_request_type_context_exits_64() {
    let h = Harness::new().await;
    mount_project_meta(&h.server, "HELP", "99", "service_desk").await;
    mount_service_desk_list(&h.server, "99", "10").await;
    mount_request_type_fields(
        &h.server,
        "10",
        "11001",
        vec![json!({
            "fieldId": "summary",
            "name": "Summary",
            "description": null,
            "required": true,
            "jiraSchema": {"type": "string"},
            "validValues": [],
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
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for a field absent from the request-type field set, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("not available") || stderr.contains("request type"),
        "expected a per-context 'not a field on this request type' style message; got: {stderr}"
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
        .and(path("/rest/api/3/issue/createmeta/HELP/issuetypes/10000"))
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
        .and(path("/rest/api/3/issue/createmeta/HELP/issuetypes/10000"))
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
    assert_eq!(
        arr.len(),
        2,
        "expected the page-2 field's 2 options, got: {parsed}"
    );
}

/// C-LOW-2 (round-2 convergence, TDD RED): pagination must continue past a
/// page whose response OMITS `total` entirely — `CreateMetaFieldsResponse
/// .total` is `#[serde(default)]` (`u32`), so a missing `total` silently
/// defaults to `0`. `get_createmeta_fields`'s termination check is
/// `page_len == 0 || start_at + page_len >= total`; with `total` defaulted
/// to `0`, `start_at + page_len >= 0` is true unconditionally on page 1 (any
/// non-empty page), so pagination incorrectly stops after page 1 and NEVER
/// issues the `startAt=200` request for page 2 — a target field placed on
/// page 2 is silently lost. Page 1 here returns a FULL page (200 fields, the
/// real `page_size`) to specifically exercise the "a full page must still
/// continue" invariant (not merely "a short page correctly stops"), and
/// neither page carries a `total` key. Expected to FAIL until the
/// implementer fixes the termination check to not treat a missing `total`
/// as "no more pages" (e.g. `Option<u32>` + explicit no-`total` handling, or
/// continuing whenever the page was full).
#[tokio::test]
async fn test_bc_x_14_001_get_createmeta_fields_continues_pagination_when_total_absent() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "HELP", &[("10000", "Bug")]).await;

    // Page 1: a FULL page (200 dummy fields), `total` intentionally absent.
    let page_1_fields: Vec<Value> = (0..200)
        .map(|i| {
            json!({
                "fieldId": format!("customfield_9{i:03}"),
                "name": format!("Dummy {i}"),
                "schema": {"type": "string"},
                "allowedValues": null
            })
        })
        .collect();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/HELP/issuetypes/10000"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": page_1_fields,
            "startAt": 0,
            "maxResults": 200
            // "total" intentionally absent — defaults to 0 via #[serde(default)].
        })))
        .mount(&h.server)
        .await;

    // Page 2 (startAt=200): the target field. `total` also absent.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/HELP/issuetypes/10000"))
        .and(query_param("startAt", "200"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": [createmeta_field_10084()],
            "startAt": 200,
            "maxResults": 200
            // "total" intentionally absent.
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
        "TDD RED (C-LOW-2): pagination must continue past a full page 1 onto \
         page 2 even when `total` is absent from the response, so the target \
         field on page 2 still resolves; got exit {:?}. stderr: {stderr}",
        output.status.code()
    );
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(
        arr.len(),
        2,
        "expected the page-2 field's 2 options, got: {parsed}"
    );
}

/// SEC-001 (S-580-1, CWE-400/770): a server that ALWAYS returns a full page
/// (`page_len == page_size == 200`) with `total` absent never satisfies
/// EITHER termination condition in `get_createmeta_fields`'s `done`
/// computation — this is exactly the shape the two 240s-timeout mutants
/// (`||`→`&&` on the fallback branch, `+=`→`*=` on `start_at`) exploited to
/// hang forever. `MAX_CREATEMETA_PAGES` (500) must fire independently of
/// that computation and return a loud error instead of looping. Kept fast
/// (well under a few seconds locally) since 500 in-process loopback
/// round-trips of a small fixed payload each cost microseconds — no real
/// per-mutant 240s timeout risk here, the pagination loop is exercised
/// directly, no timing dependency.
#[tokio::test]
async fn test_bc_x_14_001_get_createmeta_fields_hard_cap_prevents_infinite_loop() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "HELP", &[("10000", "Bug")]).await;

    // A single full page's worth of minimal, distinct-enough field
    // descriptors — reused verbatim on every request since the mock does
    // not vary on `startAt`, which is exactly the point: nothing in the
    // response ever signals "this is the last page".
    let full_page: Vec<Value> = (0..200)
        .map(|i| {
            json!({
                "fieldId": format!("customfield_9{i:03}"),
                "name": format!("Dummy {i}"),
                "schema": {"type": "string"}
            })
        })
        .collect();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/HELP/issuetypes/10000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": full_page,
            "maxResults": 200
            // "total" intentionally absent on every page, and every page is
            // a full 200-length page — both fallback-branch legs
            // (`page_len == 0` / `page_len < page_size`) are false forever.
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
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit 1 (JrError::Internal) once the hard page cap is hit \
         instead of an infinite loop; got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("exceeded") && stderr.contains("pages"),
        "expected a loud 'exceeded N pages' error naming the cap; got: {stderr}"
    );
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

/// AC-011: a human field name resolving to a SINGLE unambiguous match
/// succeeds end-to-end — the field-name-resolution happy path via
/// `list_fields()` + `partial_match`. All pre-existing tests use either the
/// `customfield_NNNNN` bypass or the ambiguous-name error path; this is the
/// first to exercise a successful human-name resolution.
#[tokio::test]
async fn test_bc_x_14_001_field_name_human_name_resolves_via_partial_match() {
    let h = Harness::new().await;
    mount_list_fields(
        &h.server,
        vec![
            json!({"id": "customfield_10001", "name": "Story Points", "custom": true, "schema": null}),
            json!({"id": "customfield_10002", "name": "Sprint", "custom": true, "schema": null}),
        ],
    )
    .await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10001": {
                "name": "Story Points",
                "schema": {"type": "option"},
                "allowedValues": [{"id": "1", "value": "One"}, {"id": "2", "value": "Two"}],
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "Story Points",
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
        "expected exit 0 for an unambiguous human field name, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(
        arr.len(),
        2,
        "expected the 2 options for the resolved field"
    );
}

/// AC-011 / EC-3.4.015-1 parallel: a human field name resolving to ZERO
/// candidates -> exit 64, hinting `jr project fields`, before any
/// enumeration HTTP call.
#[tokio::test]
async fn test_bc_x_14_001_field_name_zero_match_exits_64() {
    let h = Harness::new().await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10002", "name": "Sprint", "custom": true, "schema": null})],
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"fields": {}})))
        .expect(0)
        .mount(&h.server)
        .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "NonExistentFieldName",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for a zero-match field name, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("not found") || stderr.contains("project fields"),
        "expected a field-not-found message; got: {stderr}"
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
    assert!(
        stdout.contains("ID"),
        "table must have an ID column header; got: {stdout}"
    );
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
    // B-M1 (round-2 convergence, MEDIUM): pin the EXACT Assets-branch phrase
    // (matching the sibling `test_bc_x_14_004_graceful_degrade_array_typed_cmdb_field`
    // assertion), not a bare `contains("Assets")` substring check — the
    // fixture's own field name ("Affected Assets") would satisfy a bare
    // "Assets" substring match via the GENERIC fall-through hint even if
    // the CMDB classification itself were broken, making the original
    // assertion non-discriminating.
    assert!(
        stderr.contains("no enumerable options") && stderr.contains("uses Assets (CMDB)"),
        "expected the Assets-specific degrade hint (exact CMDB-branch phrase) on \
         stderr, not merely a field-name coincidence; got: {stderr}"
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
    // BC-X.14.004 AC-014: "+ autoCompleteUrl if present in the response" —
    // TDD RED (S-580-1 convergence pass, M3): `degrade_hint_for_schema`
    // does not currently accept or emit `autoCompleteUrl` at all, and
    // `EditMetaField`/`CreateMetaField` do not currently deserialize it —
    // this assertion is expected to FAIL until the implementer threads
    // `autoCompleteUrl` through both the wire types and the hint text.
    assert!(
        stderr.contains("https://example.atlassian.net/rest/api/1.0/users/picker"),
        "AC-014: the dynamic/lookup-field hint must include autoCompleteUrl when \
         the field schema carries one; got: {stderr}"
    );
}

/// Graceful degradation — BROADER classification (BC-X.14.004, S-580-1
/// convergence pass): an ARRAY-typed CMDB/Assets field (schema.type
/// "array", not "object" — e.g. a multi-select Assets field) must STILL get
/// the Assets-specific hint, not the generic "no fixed value set" hint.
/// TDD RED: `degrade_hint_for_schema` currently gates the Assets branch on
/// `field_type == "object"` exactly, so an "array"-typed CMDB field falls
/// through to the generic hint — expected to FAIL until the implementer
/// broadens the Assets classification to cover both shapes.
#[tokio::test]
async fn test_bc_x_14_004_graceful_degrade_array_typed_cmdb_field() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10055": {
                "name": "Connected Objects",
                "schema": {
                    "type": "array",
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
        "customfield_10055",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "expected exit 0; stderr: {stderr}");
    // Assert the SPECIFIC Assets-hint phrase (not a bare "Assets" substring
    // match, which the field's own human-readable name could coincidentally
    // satisfy) — pinned to the exact wording `degrade_hint_for_schema` emits
    // on the object-typed CMDB branch.
    assert!(
        stderr.contains("no enumerable options") && stderr.contains("uses Assets (CMDB)"),
        "an array-typed CMDB field must still get the Assets-specific degrade \
         hint, not the generic hint; got: {stderr}"
    );
}

/// Graceful degradation — BROADER classification (BC-X.14.004, S-580-1
/// convergence pass): a `labels` field (a suggestion-backed, non-fixed-value
/// field per BC-X.14.004's degrade table: "user-picker/multi-user-picker/
/// Approvers/labels/other suggestion-backed fields") must get the
/// dynamic/lookup-field hint, not the generic "no fixed value set" hint.
/// TDD RED: `degrade_hint_for_schema` currently gates the dynamic branch on
/// `field_type == "user"` exactly, so a `labels` field (schema.type
/// "array", a Jira system field with no `custom` key) falls through to the
/// generic hint — expected to FAIL until the implementer broadens the
/// dynamic/suggestion-backed classification.
#[tokio::test]
async fn test_bc_x_14_004_graceful_degrade_labels_field() {
    let h = Harness::new().await;
    // "labels" is a human/system field name, not a `customfield_NNNNN`
    // literal, so it resolves via `list_fields()` + `partial_match` first.
    mount_list_fields(
        &h.server,
        vec![json!({"id": "labels", "name": "Labels", "custom": false, "schema": null})],
    )
    .await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "labels": {
                "name": "Labels",
                "schema": {"type": "array", "system": "labels"},
                "allowedValues": null,
                "autoCompleteUrl": "https://example.atlassian.net/rest/api/1.0/labels/suggest",
                "operations": ["add", "set", "remove"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "labels",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "expected exit 0; stderr: {stderr}");
    assert!(
        stderr.contains("no enumerable options") && stderr.contains("dynamic"),
        "a labels/suggestion-backed field must get the dynamic/lookup-field \
         degrade hint, not the generic hint; got: {stderr}"
    );
}

/// Graceful degradation — A-MEDIUM (round-2 convergence, TDD RED):
/// `autoCompleteUrl` presence should ITSELF classify a field as
/// dynamic/suggestion-backed, independent of the keyword allowlist
/// (`is_dynamic` in `degrade_hint_for_schema`). A group-picker field
/// (`schema.type` "group", `schema.custom` naming `...:grouppicker`) sits
/// OUTSIDE the current keyword set (`user`/`userpicker`/`multiuserpicker`/
/// `approve`/`labels`) entirely, so it currently falls through to the
/// GENERIC "no fixed value set" hint even though it carries a real
/// `autoCompleteUrl` and is resolved live via a suggestion endpoint —
/// contradicting BC-X.14.004's degrade table ("...Approvers/labels/OTHER
/// suggestion-backed fields" — "OTHER" covers any field with an
/// `autoCompleteUrl`, not just the named keyword families). Expected to
/// FAIL until the implementer adds `|| auto_complete_url.is_some()` to
/// `is_dynamic`'s classification.
#[tokio::test]
async fn test_bc_x_14_004_graceful_degrade_group_picker_classified_by_autocompleteurl() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10200": {
                "name": "Reviewer Group",
                "schema": {
                    "type": "group",
                    "custom": "com.atlassian.jira.plugin.system.customfieldtypes:grouppicker"
                },
                "allowedValues": null,
                "autoCompleteUrl": "https://example.atlassian.net/rest/api/1.0/groups/picker",
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10200",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "expected exit 0; stderr: {stderr}");
    assert!(
        stderr.contains("no enumerable options") && stderr.contains("dynamic"),
        "a group-picker field carrying an autoCompleteUrl must be classified \
         as dynamic/lookup — not the generic 'no fixed value set' hint — even \
         though 'group'/'grouppicker' matches none of the keyword allowlist; \
         got: {stderr}"
    );
    assert!(
        stderr.contains("https://example.atlassian.net/rest/api/1.0/groups/picker"),
        "the autoCompleteUrl must be surfaced in the dynamic-field hint; got: {stderr}"
    );
    assert!(
        !stderr.contains("no fixed value set"),
        "must NOT fall through to the generic hint when autoCompleteUrl is \
         present; got: {stderr}"
    );
}

/// Graceful degradation — C-LOW-1 (round-2 convergence, TDD RED): a JSM
/// Approvers field (`schema.custom` =
/// `com.atlassian.servicedesk.approvals-plugin:sd-approvals`) must get the
/// dynamic/lookup hint per BC-X.14.004's degrade table ("...Approvers/
/// labels/other suggestion-backed fields"). Currently `is_dynamic`'s
/// substring check is `c_lower.contains("approve")`, which does NOT match
/// `"...approvals-plugin:sd-approvals"` (no literal "approve" substring —
/// "approvals" lacks the trailing "e" after "approv"). Expected to FAIL
/// until the implementer either widens the substring to `"approv"` or adds
/// the `autoCompleteUrl` classifier from the sibling group-picker test
/// above (this fixture carries one too, so either fix closes this gap).
#[tokio::test]
async fn test_bc_x_14_004_graceful_degrade_approvers_field() {
    let h = Harness::new().await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10300": {
                "name": "Approvers",
                "schema": {
                    "type": "array",
                    "custom": "com.atlassian.servicedesk.approvals-plugin:sd-approvals"
                },
                "allowedValues": null,
                "autoCompleteUrl": "https://example.atlassian.net/rest/servicedeskapi/request/FOO-1/approver",
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "customfield_10300",
        "--issue",
        "FOO-1",
        "--no-input",
    ]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "expected exit 0; stderr: {stderr}");
    assert!(
        stderr.contains("no enumerable options") && stderr.contains("dynamic"),
        "a JSM Approvers field must get the dynamic/lookup-field degrade \
         hint, not the generic 'no fixed value set' hint; got: {stderr}"
    );
    assert!(
        !stderr.contains("no fixed value set"),
        "must NOT fall through to the generic hint for an Approvers field; \
         got: {stderr}"
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
    assert_eq!(
        parsed,
        json!([]),
        "graceful-degrade stdout must be an empty JSON array"
    );
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
    // B-LOW (round-3 convergence): pin the exact hardened Assets-branch hint
    // phrase, not the bare "Assets" substring -- a field literally named
    // "Assets" could satisfy the looser assertion by coincidence. Matches
    // the sibling CMDB tests' discriminating-phrase convention.
    assert!(
        stderr.contains("uses Assets (CMDB)"),
        "expected the exact Assets-branch hint phrase (defeats a field-name coincidence via the generic hint); got: {stderr}"
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

// ═══════════════════════════════════════════════════════════════════════════
// Round-3 convergence pins (S-580-1): HIGH regression pin for the CWE-835
// empty-page infinite-loop fix (commit 7ea5e9d9), plus MEDIUM/LOW coverage
// gaps identified in round-2 adversarial review.
// ═══════════════════════════════════════════════════════════════════════════

/// HIGH regression pin (CWE-835, commit 7ea5e9d9): `get_createmeta_fields`'s
/// `total > 0` termination branch must treat an EMPTY page as `done`
/// regardless of whether `start_at + page_len >= total` — otherwise
/// `start_at += page_len` adds 0 forever and the identical `startAt=200` GET
/// repeats without bound. Page 1 (`startAt=0`) returns a FULL 200-field page
/// (with the target field embedded in it) against `total: 250`; page 2
/// (`startAt=200`) returns an EMPTY `fields` array despite `total: 250`
/// still exceeding `start_at + page_len` (200 + 0 = 200 < 250) — a
/// permission-filtered short/empty page in the JRACLOUD-71293/95368 class.
/// Without the `page_len == 0` guard in the `total > 0` branch, `done` stays
/// `false` forever and the subprocess would hang until the test harness's
/// timeout; with the guard, the command terminates immediately after the
/// empty page and reports the page-1 field's options.
#[tokio::test]
async fn test_bc_x_14_001_get_createmeta_fields_empty_page_terminates_not_infinite_loop() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "HELP", &[("10000", "Bug")]).await;

    // Page 1: a FULL page (200 fields) with the target field embedded, total = 250.
    let mut page_1_fields: Vec<Value> = (0..199)
        .map(|i| {
            json!({
                "fieldId": format!("customfield_9{i:03}"),
                "name": format!("Dummy {i}"),
                "schema": {"type": "string"},
                "allowedValues": null
            })
        })
        .collect();
    page_1_fields.push(createmeta_field_10084());
    assert_eq!(page_1_fields.len(), 200, "page 1 must be a full page");

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/HELP/issuetypes/10000"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": page_1_fields,
            "startAt": 0,
            "maxResults": 200,
            "total": 250
        })))
        .mount(&h.server)
        .await;

    // Page 2 (startAt=200): EMPTY fields despite total=250 still exceeding
    // start_at + page_len (200 + 0 = 200 < 250) -- must still terminate.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/HELP/issuetypes/10000"))
        .and(query_param("startAt", "200"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": [],
            "startAt": 200,
            "maxResults": 200,
            "total": 250
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
        "expected exit 0 (empty page must terminate pagination, not loop forever), got {:?}. stderr: {stderr}",
        output.status.code()
    );
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(
        arr.len(),
        2,
        "expected the page-1 field's 2 options, got: {parsed}"
    );
}

/// B-M1 (round-2 MEDIUM): `--value` at the `handle()`/subprocess level
/// against a NON-empty, multi-option enumeration must narrow the JSON array
/// to exactly the matching subset. Every prior discriminating `--value` test
/// exercises the pure `filter_options` fn directly; the only prior
/// integration-level `--value` test runs against a zero-options degrade
/// field (`test_bc_x_14_002_value_with_graceful_degrade_still_hints`), so a
/// `handle()`-level defect that drops or mis-threads the `--value` arg
/// before it ever reaches `filter_options` would be invisible.
#[tokio::test]
async fn test_bc_x_14_002_value_narrows_json_output_against_nonempty_enumeration() {
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
                    {"id": "10002", "value": "Client B"},
                    {"id": "10003", "value": "Other Value"}
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
        "--value",
        "Client",
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
    assert_eq!(
        arr.len(),
        2,
        "--value \"Client\" must narrow to exactly the 2 matching entries (Client A, Client B), not all 3 or 0; got: {parsed}"
    );
    let labels: Vec<&str> = arr.iter().map(|e| e["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"Client A"));
    assert!(labels.contains(&"Client B"));
    assert!(
        !labels.contains(&"Other Value"),
        "narrowed output must not include the non-matching entry; got: {parsed}"
    );
}

/// B-M1 companion: `--value ""` (explicit empty string) and no `--value` at
/// all are BOTH identity filters over the same enumeration and must produce
/// byte-identical JSON output.
#[tokio::test]
async fn test_bc_x_14_002_value_empty_string_identical_to_value_absent() {
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

    let base_args = [
        "field",
        "options",
        "customfield_10084",
        "--issue",
        "FOO-1",
        "--no-input",
        "--output",
        "json",
    ];

    let without_value = h.cmd(&base_args);
    let stdout_without = String::from_utf8_lossy(&without_value.get_output().stdout).to_string();

    let mut with_empty_args: Vec<&str> = base_args.to_vec();
    with_empty_args.push("--value");
    with_empty_args.push("");
    let with_empty_value = h.cmd(&with_empty_args);
    let stdout_with_empty =
        String::from_utf8_lossy(&with_empty_value.get_output().stdout).to_string();

    assert_eq!(
        stdout_without, stdout_with_empty,
        "--value \"\" must be identical to omitting --value entirely"
    );
}

/// B-M2 (round-2 MEDIUM, VP-580-008 property 3): on an ordinary (non-degrade)
/// success path, stderr must be EMPTY -- Profile-2 (Read-only) output-channel
/// contract forbids any stray warning/hint/cache-write `eprintln!` on a plain
/// enumeration success. Captures stdout/stderr SEPARATELY (not merely
/// checking stdout has content) so a stray stderr leak cannot hide behind an
/// otherwise-passing assertion.
#[tokio::test]
async fn test_bc_x_14_003_zero_stderr_on_ordinary_enumeration_success() {
    let h = Harness::new().await;
    // Overwrite the default legacy-shape config with an already-migrated one
    // so the one-time migration `eprintln!` doesn't false-positive this
    // stderr-emptiness assertion (unrelated to `field options` behavior).
    write_premigrated_config(h.config_dir.path(), &h.server.uri());
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
        !stdout.is_empty(),
        "sanity check: stdout must carry the table output"
    );
    assert!(
        stderr.is_empty(),
        "ordinary enumeration success must emit ZERO stderr (Profile-2 output-channel contract); got: {stderr:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// S-580-1 convergence pass — final LOW coverage pins
// ═══════════════════════════════════════════════════════════════════════════

/// `resolve_field_id`'s empty-field-name guard (`src/cli/field.rs` §
/// `if query.is_empty()`): an empty `<field>` positional must exit 64 with
/// the canonical "must not be empty" message, BEFORE any cache read or HTTP
/// call — a mutant deleting this guard would fall through to
/// `search_field_list(&fields, "", "")`, which matches every field's name
/// (an empty substring is contained in every string) and would silently
/// resolve to whichever field a real Jira instance happens to return first.
#[tokio::test]
async fn test_bc_x_14_001_empty_field_name_exits_64_zero_http() {
    let h = Harness::new().await;
    expect_zero_http(&h.server).await;

    let assert = h.cmd(&["field", "options", "", "--issue", "FOO-1", "--no-input"]);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for an empty field name, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Field '' not found") && stderr.contains("must not be empty"),
        "expected the canonical empty-field-name message; got: {stderr}"
    );
}

/// AC-011 warm-cache contract: when the per-profile fields cache
/// (`~/.cache/jr/v1/<profile>/fields.json`) is already warm and contains
/// the queried field, `resolve_field_id` must resolve directly from the
/// cache WITHOUT issuing `GET /rest/api/3/field` at all. Every other
/// human-name-resolution test in this file starts cold (empty cache dir),
/// so the warm-cache early-return branch was previously untested — a
/// mutant deleting the `if let Some(fc) = cache::read_fields_cache(...)`
/// block (or its early `return Ok(found)`) would be invisible to those
/// tests since they'd still pass via the cold-fetch fallback.
#[tokio::test]
async fn test_bc_x_14_001_warm_cache_resolves_without_list_fields_call() {
    let h = Harness::new().await;

    // Pre-populate the profile's fields cache (profile = "default", the
    // legacy `[instance]`-shape config's implicit profile) at the exact
    // path `cache::write_fields_cache` would have written, mirroring the
    // `FieldsCache { fields, fetched_at }` shape in `src/cache.rs`.
    let cache_file = h
        .cache_dir
        .path()
        .join("jr")
        .join("v1")
        .join("default")
        .join("fields.json");
    std::fs::create_dir_all(cache_file.parent().unwrap()).unwrap();
    let fetched_at = chrono::Utc::now().to_rfc3339();
    std::fs::write(
        &cache_file,
        json!({
            "fields": [["customfield_10001", "Story Points"], ["customfield_10002", "Sprint"]],
            "fetched_at": fetched_at
        })
        .to_string(),
    )
    .unwrap();

    // `GET /rest/api/3/field` must NEVER be called — the warm cache alone
    // must satisfy resolution.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(0)
        .mount(&h.server)
        .await;
    mount_editmeta(
        &h.server,
        "FOO-1",
        json!({
            "customfield_10001": {
                "name": "Story Points",
                "schema": {"type": "option"},
                "allowedValues": [{"id": "1", "value": "One"}, {"id": "2", "value": "Two"}],
                "operations": ["set"],
                "required": false
            }
        }),
    )
    .await;

    let assert = h.cmd(&[
        "field",
        "options",
        "Story Points",
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
        "expected exit 0 resolving a human field name via the warm cache, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(
        arr.len(),
        2,
        "expected the 2 options for the cache-resolved field"
    );
}

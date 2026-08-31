//! Integration tests for `jr issue create --field` on the PLATFORM
//! (non-JSM) path — createmeta-driven resolution (BC-3.3.010/011), the
//! DEC-188 guard reversal (BC-3.8.012/013, DEC-310), and the create-path
//! ten-member D2 collision guard (ADR-0019 §"D2 correction").
//!
//! Story: S-578-4. Red Gate suite (strict TDD): `handle_create`'s step 2b
//! (`field_resolve::detect_flag_field_overlap`) and step 4b
//! (`resolve_edit_fields`'s `FieldMetaSource::Create` arm) are `todo!()`
//! stubs as of commit c479220a — every test below that supplies at least one
//! `--field` on the platform path is expected to FAIL today, either via the
//! `todo!()` panic surfacing as a non-64/non-0 subprocess exit code, or via
//! an explicit assertion mismatch (e.g. a guard-ordering test whose
//! underlying mechanism — BC-3.8.013's pre-existing, UNCHANGED guard — is
//! already fully implemented and therefore already green; see the PR/report
//! notes for which specific tests fall into that category).
//!
//! Traces: BC-3.3.010, BC-3.3.011, BC-3.4.014 (amended), BC-3.8.012/013
//! (amended/reversed, DEC-310), ADR-0019, VP-578-001..004/017..022.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ─── Shared test scaffolding ────────────────────────────────────────────────

/// Legacy `[instance]`-shaped config (human-mode tests; the one-time
/// migration notice this shape emits is harmless for `stderr.contains(...)`
/// substring assertions — only strict-JSON-parse assertions need the
/// pre-migrated `[profiles.default]` shape from `common::fixtures::write_profile_config`).
fn write_minimal_config(config_home: &std::path::Path, url: &str) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        format!("[instance]\nurl = \"{url}\"\n"),
    )
    .unwrap();
}

/// Legacy `[instance]`-shaped config with a `[fields]` block configuring
/// `story_points_field_id` and/or `team_field_id` (AC-011 resolved-id
/// collision tests).
fn write_config_with_fields(
    config_home: &std::path::Path,
    url: &str,
    story_points_field_id: Option<&str>,
    team_field_id: Option<&str>,
) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    let mut body = format!("[instance]\nurl = \"{url}\"\n\n[fields]\n");
    if let Some(id) = story_points_field_id {
        body.push_str(&format!("story_points_field_id = \"{id}\"\n"));
    }
    if let Some(id) = team_field_id {
        body.push_str(&format!("team_field_id = \"{id}\"\n"));
    }
    std::fs::write(dir.join("config.toml"), body).unwrap();
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

    async fn with_fields_config(
        story_points_field_id: Option<&str>,
        team_field_id: Option<&str>,
    ) -> Self {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_config_with_fields(
            config_dir.path(),
            &server.uri(),
            story_points_field_id,
            team_field_id,
        );
        Self {
            server,
            cache_dir,
            config_dir,
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("jr").unwrap();
        cmd.env("JR_BASE_URL", self.server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("XDG_CACHE_HOME", self.cache_dir.path())
            .env("JR_CACHE_DIR", self.cache_dir.path().join("jr"))
            .env("XDG_CONFIG_HOME", self.config_dir.path())
            .env("JR_CONFIG_DIR", self.config_dir.path().join("jr"));
        cmd
    }
}

/// Registers catch-all `.expect(0)` mocks for ANY GET and ANY POST on an
/// isolated `MockServer` — the strongest zero-HTTP guarantee, used for the
/// D2 collision-guard-tripping tests (AC-011) per the HARD CONSTRAINT:
/// wiremock 0.6 is FIFO-ordered, so a shared `mount_platform_create_stubs`-
/// style free-fire mock registered first would defeat these `expect(0)`
/// mocks — isolated `MockServer` instances only.
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

async fn mount_list_fields(server: &MockServer, fields: Vec<Value>) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(fields)))
        .mount(server)
        .await;
}

async fn mount_create_post(server: &MockServer, key: &str) {
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "10001",
            "key": key,
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
        })))
        .mount(server)
        .await;
}

/// A well-formed `option`-typed createmeta field descriptor.
fn createmeta_option_field(field_id: &str, name: &str, allowed: Vec<Value>) -> Value {
    json!({
        "fieldId": field_id,
        "name": name,
        "schema": {"type": "option", "custom": "com.atlassian.jira.plugin.system.customfieldtypes:select", "system": null},
        "allowedValues": allowed
    })
}

/// A well-formed `option-with-child` (cascading select) createmeta field
/// descriptor — mirrors `tests/issue_field_hint_kinds.rs`'s cascading
/// fixture shape (parent `allowedValues` entries carrying a `children`
/// array) so `field_resolve::compose_option_hint`'s cascading `>`-split path
/// (BC-3.4.027) is exercised on the create path (AC-013).
fn createmeta_cascading_option_field(field_id: &str, name: &str, allowed: Vec<Value>) -> Value {
    json!({
        "fieldId": field_id,
        "name": name,
        "schema": {"type": "option-with-child", "custom": "com.atlassian.jira.plugin.system.customfieldtypes:cascadingselect", "system": null},
        "allowedValues": allowed
    })
}

fn createmeta_string_field(field_id: &str, name: &str) -> Value {
    json!({
        "fieldId": field_id,
        "name": name,
        "schema": {"type": "string", "custom": null, "system": null},
        "allowedValues": null
    })
}

fn createmeta_number_field(field_id: &str, name: &str) -> Value {
    json!({
        "fieldId": field_id,
        "name": name,
        "schema": {"type": "number", "custom": null, "system": null},
        "allowedValues": null
    })
}

fn createmeta_any_field(field_id: &str, name: &str) -> Value {
    json!({
        "fieldId": field_id,
        "name": name,
        "schema": {"type": "any", "custom": null, "system": null},
        "allowedValues": null
    })
}

/// A `doc`-typed createmeta field descriptor — the REAL Jira Cloud schema
/// type for the built-in `description` field (ADF document, not a plain
/// string). `dispatch_field_value`'s bare-form type dispatch has no `"doc"`
/// arm, so this fixture correctly routes to the `unsupported_field_type_error`
/// branch (AC-018 realism fix, adversary Pass 5 LOW).
fn createmeta_doc_field(field_id: &str, name: &str) -> Value {
    json!({
        "fieldId": field_id,
        "name": name,
        "schema": {"type": "doc", "custom": null, "system": null},
        "allowedValues": null
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-001: Guard ordering — steps 2, 2a, 2b in the pinned deterministic order
// ═══════════════════════════════════════════════════════════════════════════

/// AC-001 (SSOT "Platform-Path Guard Ordering", BC-3.3.010 Preconditions):
/// step 2 (BC-3.8.013 `--on-behalf-of`) wins over step 2a (`parse_field_kv`)
/// and step 2b (D2 collision guard) — an invocation tripping all three
/// guards' trigger conditions surfaces ONLY the step-2 error.
///
/// NOTE: this invocation short-circuits at step 2 (a PRE-EXISTING,
/// unmodified guard) BEFORE `parse_field_kv`/`detect_flag_field_overlap`
/// are ever reached — so this specific test does not exercise either of
/// this story's `todo!()` stubs and may already be green pre-implementation
/// (see PR notes). It is still required as a regression pin per the story's
/// explicit AC-001 test-name mandate.
#[tokio::test]
async fn test_ssot_guard_ordering_step2_wins_over_step2a_and_2b() {
    let h = Harness::new().await;
    // Would-otherwise-succeed precondition — proves the guard fires
    // deterministically, not merely that something downstream is unreachable.
    mount_create_post(&h.server, "PROJ-123").await;
    mount_list_fields(&h.server, vec![]).await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--priority",
            "X",
            "--field",
            "priority=Y",
            "--on-behalf-of",
            "Z",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-001: step 2 (BC-3.8.013) must fire; stderr={stderr}"
    );
    assert!(
        stderr.contains("--on-behalf-of is only valid with"),
        "AC-001: step-2 guard message must appear; stderr={stderr}"
    );
    assert!(
        !stderr.contains("unknown field-value kind"),
        "AC-001: step-2a's parse error must NOT appear (never reached); stderr={stderr}"
    );
    assert!(
        !stderr.contains("Created issue"),
        "AC-001: no success path must have executed; stderr={stderr}"
    );
}

/// AC-001: when step 2 (`--on-behalf-of`) is ABSENT, step 2a
/// (`parse_field_kv`'s malformed-hint exit-64, BC-3.4.031) wins over step 2b
/// (the D2 collision guard) — a malformed hint alongside a would-be
/// collision surfaces ONLY the step-2a error; the D2 collision guard's
/// `todo!()` is never reached because `parse_field_kv` short-circuits first.
#[tokio::test]
async fn test_ssot_guard_ordering_step2a_wins_over_step2b_when_step2_absent() {
    let h = Harness::new().await;
    mount_create_post(&h.server, "PROJ-123").await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--priority",
            "X",
            "--field",
            "cf:bogus=Y",
            "--field",
            "priority=Z",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-001: step 2a (parse_field_kv) must fire; stderr={stderr}"
    );
    assert!(
        stderr.contains("unknown field-value kind"),
        "AC-001: step-2a's malformed-hint error must appear; stderr={stderr}"
    );
    assert!(
        !stderr.contains("Created issue"),
        "AC-001: no success path must have executed; stderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-002: `--field` no longer exits 64 pre-flight — DEC-188 guard removed
// (VP-578-017). Supersedes S-639-1 AC-1's exit-64 assertion.
// ═══════════════════════════════════════════════════════════════════════════

/// AC-002 (BC-3.8.012 `[CURRENT BEHAVIOR]`, VP-578-017): `jr issue create
/// --field a=b` (no `--request-type`, well-formed field) resolves via
/// createmeta and the platform POST fires — exit 0. Stderr does NOT contain
/// the old DEC-188 verbatim string `"--field is only valid with"`.
///
/// RED today: `field_resolve::detect_flag_field_overlap` (step 2b) is a
/// blanket `todo!()` that panics for ANY non-empty `--field`, regardless of
/// whether a collision would actually occur — so this well-formed,
/// non-colliding invocation currently panics before ever reaching
/// resolution or the POST.
#[tokio::test]
async fn test_bc_3_8_012_field_alone_no_longer_exits_64() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_string_field("customfield_10077", "A Field")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10077", "name": "A Field"})],
    )
    .await;
    mount_create_post(&h.server, "PROJ-123").await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "a=b",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-002 / VP-578-017: expected exit 0; got {:?}. stderr={stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Created issue"),
        "AC-002: platform POST success path must have executed; stderr={stderr}"
    );
    assert!(
        !stderr.contains("--field is only valid with"),
        "AC-002: the old DEC-188 verbatim string is DEAD; stderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-003: combined `--field` + `--on-behalf-of` — ONLY BC-3.8.013's
// standalone guard fires (VP-578-018). Supersedes S-639-1 AC-3.
// ═══════════════════════════════════════════════════════════════════════════

/// AC-003 (BC-3.8.013 "Combined pre-flight error [REWRITTEN]", VP-578-018):
/// `--field a=b --on-behalf-of X` (no `--request-type`) exits 64 via
/// BC-3.8.013's STANDALONE guard — NOT the now-removed combined guard.
///
/// NOTE: like AC-001's first test, this short-circuits at step 2 (the
/// PRE-EXISTING, unmodified `on_behalf_of.is_some()` check) before
/// `parse_field_kv`/`detect_flag_field_overlap` are reached, so it may
/// already be green pre-implementation (see PR notes) — required regardless
/// per the story's explicit test-name mandate and as the VP-578-018
/// regression pin.
#[tokio::test]
async fn test_bc_3_8_013_combined_invocation_fires_standalone_guard_only() {
    let h = Harness::new().await;
    mount_create_post(&h.server, "PROJ-123").await;
    mount_list_fields(&h.server, vec![]).await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "a=b",
            "--on-behalf-of",
            "X",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-003 / VP-578-018: expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--on-behalf-of is only valid with"),
        "AC-003: standalone BC-3.8.013 error string must appear; stderr={stderr}"
    );
    assert!(
        !stderr.contains("--field and --on-behalf-of are only valid with"),
        "AC-003: the now-removed combined-error string must NOT appear; stderr={stderr}"
    );
    assert!(
        !stderr.contains("--field is only valid with"),
        "AC-003: --field must not itself contribute any pre-flight error; stderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-004: `--on-behalf-of` alone — unchanged, wire-for-wire regression pin
// (VP-578-019).
// ═══════════════════════════════════════════════════════════════════════════

/// AC-004 (VP-578-019 regression pin): `jr issue create --on-behalf-of X`
/// alone (no `--field`, no `--request-type`) exits 64 via BC-3.8.013,
/// UNCHANGED wire-for-wire from DEC-188-era behavior — proves this
/// reversal did not accidentally weaken BC-3.8.013's own guard.
///
/// This tests PRE-EXISTING, unmodified behavior and is expected to already
/// be green — a regression pin, not new-behavior coverage.
#[tokio::test]
async fn test_vp_578_019_on_behalf_of_alone_unchanged_regression_pin() {
    let h = Harness::new().await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--on-behalf-of",
            "X",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-004 / VP-578-019: expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains(
            "--on-behalf-of is only valid with --request-type (JSM service-desk requests). Add --request-type <NAME> to raise a request on behalf of another user, or drop --on-behalf-of to create a standard platform issue."
        ),
        "AC-004: FULL-STRING verbatim single-flag error must appear unchanged; stderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-005: resolution algorithm — customfield_NNNNN bypass + cache-first
// field-name resolution.
// ═══════════════════════════════════════════════════════════════════════════

/// AC-005 (BC-3.3.010 Resolution algorithm Step 1): `customfield_NNNNN`
/// literal bypasses field-NAME resolution entirely — `GET /rest/api/3/field`
/// must never be called for a pure-bypass invocation.
#[tokio::test]
async fn test_bc_3_3_010_customfield_bypass_on_create() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_string_field("customfield_10099", "Bypassed")],
    )
    .await;
    // Bypass form must never call list_fields() — zero GET /rest/api/3/field.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(0)
        .mount(&h.server)
        .await;
    mount_create_post(&h.server, "PROJ-123").await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "customfield_10099=bypassed-value",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-005: customfield_NNNNN bypass must resolve successfully (exit 0); \
         got {:?}. stderr={stderr}",
        output.status.code()
    );
}

/// AC-005 (BC-3.3.010 Resolution algorithm Step 2, same cache-first
/// `fields.json`/`list_fields()` lookup as BC-3.4.015 Step 2/2b — shared
/// function, shared cache): a NAME-form `--field` resolves via
/// `GET /rest/api/3/field`, and that field list is shared across both
/// `issue edit --field` and `issue create --field` (VP-578-002) — no
/// second, create-path-specific cache family.
#[tokio::test]
async fn test_bc_3_3_010_cache_first_field_name_resolution() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_string_field("customfield_10077", "A Field")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10077", "name": "A Field"})],
    )
    .await;
    mount_create_post(&h.server, "PROJ-123").await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "A Field=some-value",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-005: NAME-form field resolution via list_fields() must succeed; \
         got {:?}. stderr={stderr}",
        output.status.code()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-006: source substitution — `get_createmeta_fields` instead of editmeta
// (VP-578-020a).
// ═══════════════════════════════════════════════════════════════════════════

/// AC-006 (BC-3.3.010 Resolution algorithm Step 3, VP-578-001): the
/// create-path `--field` resolution calls `get_createmeta_fields` — NEVER
/// `GET /issue/{key}/editmeta` (there is no issue key yet at create time).
#[tokio::test]
async fn test_bc_3_3_010_source_substitution_createmeta_not_editmeta() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_string_field("customfield_10077", "A Field")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10077", "name": "A Field"})],
    )
    .await;
    mount_create_post(&h.server, "PROJ-123").await;
    // VP-578-001: editmeta must NEVER be called on the create path.
    Mock::given(method("GET"))
        .and(wiremock::matchers::path_regex(
            r"^/rest/api/3/issue/.+/editmeta$",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"fields": {}})))
        .expect(0)
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "A Field=some-value",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-006 / VP-578-001: createmeta resolution must succeed; got {:?}. stderr={stderr}",
        output.status.code()
    );
}

/// AC-006 / VP-578-020(a): a `--field` target field landing on createmeta
/// fields-page >=2 is collected and resolves (exit 0) — NOT dropped with a
/// spurious "not on the Create screen" error. Page 1 returns one unrelated
/// field (total=2); page 2 (`startAt=1`) returns the target field.
#[tokio::test]
async fn test_vp_578_020a_field_on_createmeta_page_2_resolves() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/PROJ/issuetypes/10000"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": [createmeta_string_field("summary", "Summary")],
            "startAt": 0,
            "maxResults": 200,
            "total": 2
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/PROJ/issuetypes/10000"))
        .and(query_param("startAt", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": [createmeta_string_field("customfield_10077", "A Field")],
            "startAt": 1,
            "maxResults": 200,
            "total": 2
        })))
        .mount(&h.server)
        .await;

    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10077", "name": "A Field"})],
    )
    .await;
    mount_create_post(&h.server, "PROJ-123").await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "A Field=some-value",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "VP-578-020(a): a field on createmeta page 2 must still resolve; \
         got {:?}. stderr={stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("is not on the Create screen"),
        "VP-578-020(a): must NOT spuriously report the field absent; stderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-007: `get_issue_types_for_project` — offset-paginated, page >=2
// resolves (VP-578-020b).
// ═══════════════════════════════════════════════════════════════════════════

/// AC-007 / VP-578-020(b): a `--type` name whose entry lands on
/// issuetypes-list page >=2 still resolves to its `issueTypeId`. Page 1
/// returns one unrelated type (total=2); page 2 (`startAt=1`) returns the
/// target "Bug" type.
#[tokio::test]
async fn test_vp_578_020b_type_on_issuetypes_page_2_resolves() {
    let h = Harness::new().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/PROJ/issuetypes"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issueTypes": [{"id": "10000", "name": "Task"}],
            "startAt": 0,
            "maxResults": 200,
            "total": 2
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/createmeta/PROJ/issuetypes"))
        .and(query_param("startAt", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issueTypes": [{"id": "10001", "name": "Bug"}],
            "startAt": 1,
            "maxResults": 200,
            "total": 2
        })))
        .mount(&h.server)
        .await;

    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10001",
        vec![createmeta_string_field("customfield_10077", "A Field")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10077", "name": "A Field"})],
    )
    .await;
    mount_create_post(&h.server, "PROJ-123").await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Bug",
            "--summary",
            "test",
            "--field",
            "A Field=some-value",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "VP-578-020(b): an issue type on page 2 must still resolve; \
         got {:?}. stderr={stderr}",
        output.status.code()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-008: type dispatch / option-value resolution shared with BC-3.4.015
// Step 4 / BC-3.4.016 Step 4a; hint kinds available on platform create.
// ═══════════════════════════════════════════════════════════════════════════

/// AC-008 (BC-3.3.010 Resolution algorithm Step 4): option-type dispatch on
/// create reads `allowedValues[].id` from the createmeta field entry via the
/// SAME shared dispatch `resolve_edit_fields` already uses for `issue edit`
/// — no second, independently-implemented dispatch.
#[tokio::test]
async fn test_bc_3_3_010_type_dispatch_shares_resolve_edit_fields_createmeta_source() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_option_field(
            "customfield_10084",
            "Client",
            vec![json!({"id": "1", "value": "Client A"})],
        )],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10084", "name": "Client"})],
    )
    .await;
    mount_create_post(&h.server, "PROJ-123").await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "Client=Client A",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-008: option-type dispatch on create must succeed via the shared \
         resolve_edit_fields dispatch; got {:?}. stderr={stderr}",
        output.status.code()
    );
}

/// AC-008: hint-kind syntax (`:option`/`:id`/`:name`/`:asset`, BC-3.4.026-030)
/// is available on the platform create path — SAME parser (S-578-1), SAME
/// wire-shape rules. Exercises `:id` (the simplest hint — a raw id literal,
/// no lookup).
#[tokio::test]
async fn test_bc_3_3_010_hint_kinds_available_on_platform_create() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_option_field(
            "customfield_10084",
            "Client",
            vec![json!({"id": "1", "value": "Client A"})],
        )],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10084", "name": "Client"})],
    )
    .await;
    mount_create_post(&h.server, "PROJ-123").await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "Client:id=1",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-008: :id hint must be available (and resolve) on the platform \
         create path; got {:?}. stderr={stderr}",
        output.status.code()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-009: `:asset` cold-cache workspace-discovery failure taxonomy —
// create-path call site (VP-578-022, 3rd of 3 shared call sites).
// ═══════════════════════════════════════════════════════════════════════════

/// AC-009 (BC-3.4.030 taxonomy, VP-578-022): 403/404 from
/// `GET /rest/servicedeskapi/assets/workspace` -> exit 64, "Assets is not
/// available on this Jira site...", exercised independently at the
/// CREATE-path call site.
#[tokio::test]
async fn test_bc_3_4_030_create_path_asset_cold_cache_403_404_assets_unavailable() {
    for status in [403u16, 404u16] {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_createmeta_fields_single_page(
            &h.server,
            "PROJ",
            "10000",
            vec![createmeta_any_field("customfield_60000", "Asset Field")],
        )
        .await;
        mount_list_fields(
            &h.server,
            vec![json!({"id": "customfield_60000", "name": "Asset Field"})],
        )
        .await;

        let _guard = Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/assets/workspace"))
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({
                "errorMessages": ["nope"], "errors": {}
            })))
            .mount_as_scoped(&h.server)
            .await;

        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({"key": "PROJ-1"})))
            .expect(0)
            .mount(&h.server)
            .await;

        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "customfield_60000:asset=456",
                "--no-input",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "AC-009 status={status}: expected exit 64; stderr={stderr}"
        );
        assert!(
            stderr.contains(
                "Assets is not available on this Jira site. Assets requires \
                 Jira Service Management Premium or Enterprise."
            ),
            "AC-009 status={status}: message must match the taxonomy row's \
             specific wording; stderr={stderr}"
        );
    }
}

/// AC-009: `GET /rest/servicedeskapi/assets/workspace` returning 200 with
/// zero entries -> exit 64, "No Assets workspace found on this Jira site...".
#[tokio::test]
async fn test_bc_3_4_030_create_path_asset_cold_cache_empty_workspace() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_any_field("customfield_60001", "Asset Field")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_60001", "name": "Asset Field"})],
    )
    .await;

    let _guard = Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 0, "start": 0, "limit": 25, "isLastPage": true, "values": []
        })))
        .mount_as_scoped(&h.server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"key": "PROJ-1"})))
        .expect(0)
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "customfield_60001:asset=456",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-009 empty-workspace: expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains(
            "No Assets workspace found on this Jira site. Assets requires \
             Jira Service Management Premium or Enterprise."
        ),
        "AC-009 empty-workspace: stderr={stderr}"
    );
}

/// AC-009: `GET /rest/servicedeskapi/assets/workspace` returning 401 must
/// use the STANDARD `JrError::NotAuthenticated` mapping (exit 2).
#[tokio::test]
async fn test_bc_3_4_030_create_path_asset_cold_cache_401_standard_auth_mapping() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_any_field("customfield_60002", "Asset Field")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_60002", "name": "Asset Field"})],
    )
    .await;

    let _guard = Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount_as_scoped(&h.server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"key": "PROJ-1"})))
        .expect(0)
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "customfield_60002:asset=456",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "AC-009 401: must use standard NotAuthenticated mapping (exit 2); stderr={stderr}"
    );
    assert!(stderr.contains("Not authenticated"), "stderr={stderr}");
}

/// AC-009: `GET /rest/servicedeskapi/assets/workspace` returning 5xx uses
/// the STANDARD `ApiError` mapping (exit 1).
#[tokio::test]
async fn test_bc_3_4_030_create_path_asset_cold_cache_5xx_network_standard_mapping() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_any_field("customfield_60003", "Asset Field")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_60003", "name": "Asset Field"})],
    )
    .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "errorMessages": ["Internal server error"], "errors": {}
        })))
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "customfield_60003:asset=456",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "AC-009 5xx: must use standard ApiError mapping (exit 1); stderr={stderr}"
    );
    assert!(stderr.contains("API error (500)"), "stderr={stderr}");
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-010: all-or-nothing multi-`--field` failure — zero POST on any
// resolution failure (VP-578-003).
// ═══════════════════════════════════════════════════════════════════════════

/// AC-010 / VP-578-003: two `--field` pairs, one resolvable and one that
/// fails (zero matches in `list_fields()`) -> exit 64, ZERO HTTP POST.
#[tokio::test]
async fn test_vp_578_003_all_or_nothing_multi_field_failure() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_string_field("customfield_10077", "A Field")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10077", "name": "A Field"})],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"key": "PROJ-1"})))
        .expect(0)
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "A Field=ok",
            "--field",
            "Totally Unknown Field=x",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "VP-578-003: expected exit 64 on partial resolution failure; stderr={stderr}"
    );
    assert!(
        !stderr.contains("Created issue"),
        "VP-578-003: no success path must have executed; stderr={stderr}"
    );
    // The .expect(0) POST mock is enforced on server drop — the NORMATIVE
    // zero-POST proof for this all-or-nothing invariant.
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-011: create-path collision guard — ten-member governed set
// (D2/D2-correction, VP-578-021).
// ═══════════════════════════════════════════════════════════════════════════

/// AC-011 / VP-578-021: the 5 original Gate-B-shaped static keys (summary,
/// description, issuetype, priority, components) each trip the create-path
/// D2 guard when a dedicated flag AND a `--field` target the same wire key.
/// Isolated `MockServer` per case, ONLY `.expect(0)` mocks (hard constraint).
#[tokio::test]
async fn test_vp_578_021_create_path_collision_5_original_static_keys() {
    let cases: &[(&str, &str, &str)] = &[
        ("--summary", "test", "summary=Other"),
        ("--description", "test-desc", "description=Other"),
        ("--type", "Task", "issuetype:id=10001"),
        ("--priority", "Medium", "priority=Low"),
        ("--component", "X", "components:name=Y"),
    ];
    for (flag, flag_value, field_pair) in cases {
        let server = MockServer::start().await;
        expect_zero_http(&server).await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        let mut args = vec![
            "issue".to_string(),
            "create".to_string(),
            "--project".to_string(),
            "PROJ".to_string(),
        ];
        // Base --summary/--type are required for a well-formed invocation
        // (would-otherwise-succeed precondition) — but clap rejects a
        // scalar Option<String> flag supplied twice, so when the flag under
        // test IS --summary or --type, substitute its own value in place of
        // the base value rather than appending both.
        args.push("--summary".to_string());
        args.push(if *flag == "--summary" {
            (*flag_value).to_string()
        } else {
            "base summary".to_string()
        });
        args.push("--type".to_string());
        args.push(if *flag == "--type" {
            (*flag_value).to_string()
        } else {
            "Task".to_string()
        });
        if *flag != "--summary" && *flag != "--type" {
            args.push((*flag).to_string());
            args.push((*flag_value).to_string());
        }
        args.push("--field".to_string());
        args.push((*field_pair).to_string());
        args.push("--no-input".to_string());

        let output = Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("XDG_CACHE_HOME", cache_dir.path())
            .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
            .env("XDG_CONFIG_HOME", config_dir.path())
            .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
            .args(&args)
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "AC-011 flag={flag}: D2 collision guard must fire, zero HTTP; stderr={stderr}"
        );
        assert!(
            !stderr.contains("Created issue"),
            "AC-011 flag={flag}: no success path must have executed; stderr={stderr}"
        );
    }
}

/// AC-011 / VP-578-021: the 3 NEW static keys (labels, parent, assignee) —
/// deliberately governed on create (unlike edit-path Gate B, which excludes
/// `labels` for BUG-LABEL-400 reasons that have no analog on create).
#[tokio::test]
async fn test_vp_578_021_create_path_collision_labels_parent_assignee() {
    let cases: &[(&[&str], &str)] = &[
        (&["--label", "foo"], "labels=bar"),
        (&["--parent", "FOO-1"], "parent=BAR-2"),
        (&["--to", "jane"], "assignee=other-account-id"),
    ];
    for (flag_args, field_pair) in cases {
        let server = MockServer::start().await;
        expect_zero_http(&server).await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_minimal_config(config_dir.path(), &server.uri());

        let mut args = vec![
            "issue",
            "create",
            "--project",
            "PROJ",
            "--summary",
            "test",
            "--type",
            "Task",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
        args.extend(flag_args.iter().map(|s| s.to_string()));
        args.push("--field".to_string());
        args.push((*field_pair).to_string());
        args.push("--no-input".to_string());

        let output = Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("XDG_CACHE_HOME", cache_dir.path())
            .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
            .env("XDG_CONFIG_HOME", config_dir.path())
            .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
            .args(&args)
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "AC-011 flag_args={flag_args:?}: D2 collision guard must fire, zero HTTP; \
             stderr={stderr}"
        );
        assert!(
            !stderr.contains("Created issue"),
            "AC-011 flag_args={flag_args:?}: no success path must have executed; stderr={stderr}"
        );
    }
}

/// AC-011 / VP-578-021: `--points` collides with `--field customfield_NNNNN=…`
/// via RESOLVED-ID equality (bypass-form-only) when `story_points_field_id`
/// is configured. `resolve_story_points_field_id` is unconditionally
/// config-only — no HTTP fallback is ever needed to service this guard.
#[tokio::test]
async fn test_vp_578_021_create_path_collision_points_resolved_id() {
    let server = MockServer::start().await;
    expect_zero_http(&server).await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_config_with_fields(
        config_dir.path(),
        &server.uri(),
        Some("customfield_10050"),
        None,
    );

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--summary",
            "test",
            "--type",
            "Task",
            "--points",
            "5",
            "--field",
            "customfield_10050=8",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-011 points: D2 collision guard must fire via resolved-id equality, \
         zero HTTP; stderr={stderr}"
    );
    assert!(
        !stderr.contains("Created issue"),
        "AC-011 points: no success path must have executed; stderr={stderr}"
    );
}

/// AC-011 / VP-578-021: `--team` collides with `--field customfield_NNNNN=…`
/// via the SAME resolved-id mechanism, ONLY when `team_field_id` is
/// configured — `client.find_team_field_id()` (HTTP) must NEVER be invoked
/// to service this guard.
#[tokio::test]
async fn test_vp_578_021_create_path_collision_team_resolved_id_configured() {
    let server = MockServer::start().await;
    expect_zero_http(&server).await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_config_with_fields(
        config_dir.path(),
        &server.uri(),
        None,
        Some("customfield_10060"),
    );

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--summary",
            "test",
            "--type",
            "Task",
            "--team",
            "Platform Core",
            "--field",
            "customfield_10060=other-team",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-011 team: D2 collision guard must fire via resolved-id equality \
         (team_field_id configured), zero HTTP; stderr={stderr}"
    );
    assert!(
        !stderr.contains("Created issue"),
        "AC-011 team: no success path must have executed; stderr={stderr}"
    );
    // expect_zero_http's catch-all .expect(0) mocks are the NORMATIVE proof
    // that client.find_team_field_id()'s GET /rest/api/3/field was never
    // called to service this guard.
}

/// AC-011 / VP-578-021 NEGATIVE PIN: `--points 5 --field "Story Points"=8`
/// — a human DISPLAY NAME spelling on the `--field` side, NOT the
/// `customfield_NNNNN` bypass form — does NOT trip the guard (documented
/// non-firing residual, BC-3.3.010 EC-3.3.010-6a). Both values reach
/// resolution unordered (proven here by the invocation making at least one
/// HTTP call, i.e. NOT being rejected pre-flight with zero HTTP like the
/// positive-collision cases above).
#[tokio::test]
async fn test_vp_578_021_negative_pin_display_name_spelling_does_not_trip_guard() {
    let h = Harness::with_fields_config(Some("customfield_10050"), None).await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_number_field("customfield_10050", "Story Points")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10050", "name": "Story Points"})],
    )
    .await;
    mount_create_post(&h.server, "PROJ-123").await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--summary",
            "test",
            "--type",
            "Task",
            "--points",
            "5",
            "--field",
            "Story Points=8",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let requests = h.server.received_requests().await.unwrap();
    assert!(
        !requests.is_empty(),
        "AC-011 negative pin: a display-name --field spelling must NOT be \
         rejected pre-flight with zero HTTP — the guard's documented \
         non-firing residual means resolution proceeds; stderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-012: error taxonomy — 10-row table, collision row evaluated FIRST
// (VP-578-004).
// ═══════════════════════════════════════════════════════════════════════════

/// AC-012 / VP-578-004 (BC-3.3.011 Error taxonomy table): each of the 10
/// rows is independently exercised, asserting exit code, zero POST (where
/// applicable), and the row's load-bearing substring. Table-driven, one
/// isolated `MockServer` per row.
#[tokio::test]
async fn test_bc_3_3_011_error_taxonomy_all_10_rows() {
    // Row 1: dedicated-flag x --field collision, evaluated FIRST.
    {
        let h = Harness::new().await;
        mount_create_post(&h.server, "PROJ-1").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--priority",
                "X",
                "--field",
                "priority=Y",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "row1: stderr={stderr}");
        // AC-012/BC-3.3.011 mandates the row's load-bearing substring, not
        // just exit code + absence-of-success (adversary Pass 4 NITPICK).
        // Exact wording from `field_resolve.rs::collision_error`:
        // "{key} is set by both {flag_hint} and --field; use only one."
        assert!(
            stderr.contains("is set by both --priority and --field")
                && stderr.contains("use only one"),
            "row1: collision error's load-bearing substrings must appear; stderr={stderr}"
        );
        assert!(!stderr.contains("Created issue"), "row1: stderr={stderr}");
    }

    // Row 2: zero matches in list_fields().
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_list_fields(&h.server, vec![]).await;
        mount_create_post(&h.server, "PROJ-1").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "Nonexistent Field=x",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "row2: stderr={stderr}");
        assert!(
            stderr.contains("not found") && stderr.contains("Zero matches"),
            "row2: stderr={stderr}"
        );
        assert!(!stderr.contains("Created issue"), "row2: stderr={stderr}");
    }

    // Row 3: multiple substring matches.
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_list_fields(
            &h.server,
            vec![
                json!({"id": "customfield_1", "name": "Custom Alpha"}),
                json!({"id": "customfield_2", "name": "Custom Beta"}),
            ],
        )
        .await;
        mount_create_post(&h.server, "PROJ-1").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "Custom=x",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "row3: stderr={stderr}");
        // AC-012/BC-3.3.011 mandates the row's load-bearing substring
        // (adversary Pass 4 NITPICK). Exact wording from
        // `field_resolve.rs::resolve_edit_fields`'s `search_field` closure:
        // "Field name '{name}' is ambiguous — matches: {candidates}. …"
        assert!(
            stderr.contains("is ambiguous")
                && stderr.contains("Custom Alpha")
                && stderr.contains("Custom Beta"),
            "row3: ambiguous-candidates listing substring must appear; stderr={stderr}"
        );
        assert!(!stderr.contains("Created issue"), "row3: stderr={stderr}");
    }

    // Row 4: field absent from resolved issue type's createmeta.
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_createmeta_fields_single_page(&h.server, "PROJ", "10000", vec![]).await;
        mount_list_fields(
            &h.server,
            vec![json!({"id": "customfield_10077", "name": "A Field"})],
        )
        .await;
        mount_create_post(&h.server, "PROJ-1").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "A Field=x",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "row4: stderr={stderr}");
        assert!(
            stderr.contains("is not on the Create screen")
                && stderr.contains("A project admin must add it to the Create screen"),
            "row4: stderr={stderr}"
        );
        assert!(!stderr.contains("Created issue"), "row4: stderr={stderr}");
    }

    // Row 5: number field, non-numeric value.
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_createmeta_fields_single_page(
            &h.server,
            "PROJ",
            "10000",
            vec![createmeta_number_field("customfield_10050", "Points")],
        )
        .await;
        mount_list_fields(
            &h.server,
            vec![json!({"id": "customfield_10050", "name": "Points"})],
        )
        .await;
        mount_create_post(&h.server, "PROJ-1").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "Points=abc",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "row5: stderr={stderr}");
        assert!(
            stderr.contains("Cannot parse") && stderr.contains("as a number"),
            "row5: stderr={stderr}"
        );
        assert!(!stderr.contains("Created issue"), "row5: stderr={stderr}");
    }

    // Row 6: array/any unsupported type.
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_createmeta_fields_single_page(
            &h.server,
            "PROJ",
            "10000",
            vec![createmeta_any_field("customfield_10090", "Unsupported")],
        )
        .await;
        mount_list_fields(
            &h.server,
            vec![json!({"id": "customfield_10090", "name": "Unsupported"})],
        )
        .await;
        mount_create_post(&h.server, "PROJ-1").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "Unsupported=x",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "row6: stderr={stderr}");
        assert!(
            stderr.contains("which is not supported by `--field`"),
            "row6: stderr={stderr}"
        );
        assert!(!stderr.contains("Created issue"), "row6: stderr={stderr}");
    }

    // Row 7: option field, value matches no allowedValues.
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_createmeta_fields_single_page(
            &h.server,
            "PROJ",
            "10000",
            vec![createmeta_option_field(
                "customfield_10084",
                "Client",
                vec![json!({"id": "1", "value": "Client A"})],
            )],
        )
        .await;
        mount_list_fields(
            &h.server,
            vec![json!({"id": "customfield_10084", "name": "Client"})],
        )
        .await;
        mount_create_post(&h.server, "PROJ-1").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "Client=Nonexistent",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "row7: stderr={stderr}");
        assert!(
            stderr.contains("not found") && stderr.contains("Allowed values"),
            "row7: stderr={stderr}"
        );
        assert!(!stderr.contains("Created issue"), "row7: stderr={stderr}");
    }

    // Row 8: option field, value ambiguous substring match.
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_createmeta_fields_single_page(
            &h.server,
            "PROJ",
            "10000",
            vec![createmeta_option_field(
                "customfield_10084",
                "Client",
                vec![
                    json!({"id": "1", "value": "Client A"}),
                    json!({"id": "2", "value": "Client Alpha"}),
                ],
            )],
        )
        .await;
        mount_list_fields(
            &h.server,
            vec![json!({"id": "customfield_10084", "name": "Client"})],
        )
        .await;
        mount_create_post(&h.server, "PROJ-1").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "Client=Client",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "row8: stderr={stderr}");
        assert!(stderr.contains("ambiguous"), "row8: stderr={stderr}");
        assert!(!stderr.contains("Created issue"), "row8: stderr={stderr}");
    }

    // Row 9: option field, matched entry has id: None.
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_createmeta_fields_single_page(
            &h.server,
            "PROJ",
            "10000",
            vec![createmeta_option_field(
                "customfield_10084",
                "Client",
                vec![json!({"value": "No Id Option"})],
            )],
        )
        .await;
        mount_list_fields(
            &h.server,
            vec![json!({"id": "customfield_10084", "name": "Client"})],
        )
        .await;
        mount_create_post(&h.server, "PROJ-1").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "Client=No Id Option",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "row9: stderr={stderr}");
        assert!(
            stderr.contains("no machine-readable id"),
            "row9: stderr={stderr}"
        );
        assert!(!stderr.contains("Created issue"), "row9: stderr={stderr}");
    }

    // Row 10: list_fields()/createmeta HTTP failure (5xx) propagated.
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        Mock::given(method("GET"))
            .and(path("/rest/api/3/field"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({
                "errorMessages": ["Internal server error"], "errors": {}
            })))
            .mount(&h.server)
            .await;
        mount_create_post(&h.server, "PROJ-1").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "Some Field=x",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "row10: stderr={stderr}");
        assert!(
            stderr.contains("API error (500)"),
            "row10: load-bearing substring for the propagated list_fields()/createmeta \
             5xx must appear (AC-012); stderr={stderr}"
        );
        assert!(!stderr.contains("Created issue"), "row10: stderr={stderr}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-013: create-path echo (BC-3.4.014 amendment) — `--field` fields in the
// success echo, per hint kind.
// ═══════════════════════════════════════════════════════════════════════════

/// AC-013 (BC-3.4.014 amended `--field` bullet): bare/`:option` echo the
/// resolved human-readable label; `:id` echoes the raw id literal; `:name`
/// echoes VALUE verbatim. Each is exercised as its own successful create,
/// checking for the `  <field> -> <value>` echo line shape.
#[tokio::test]
async fn test_bc_3_4_014_field_echo_bare_and_hinted_per_kind() {
    // Bare/:option form -> resolved human-readable label.
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_createmeta_fields_single_page(
            &h.server,
            "PROJ",
            "10000",
            vec![createmeta_option_field(
                "customfield_10084",
                "Client",
                vec![json!({"id": "1", "value": "Client A"})],
            )],
        )
        .await;
        mount_list_fields(
            &h.server,
            vec![json!({"id": "customfield_10084", "name": "Client"})],
        )
        .await;
        mount_create_post(&h.server, "PROJ-123").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "Client=Client A",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "bare/:option echo case: expected exit 0; stderr={stderr}"
        );
        assert!(
            stderr.contains("Client \u{2192} Client A"),
            "bare form must echo the resolved human-readable label; stderr={stderr}"
        );
    }

    // :id form -> raw id literal, no reverse lookup.
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_createmeta_fields_single_page(
            &h.server,
            "PROJ",
            "10000",
            vec![createmeta_option_field(
                "customfield_10084",
                "Client",
                vec![json!({"id": "1", "value": "Client A"})],
            )],
        )
        .await;
        mount_list_fields(
            &h.server,
            vec![json!({"id": "customfield_10084", "name": "Client"})],
        )
        .await;
        mount_create_post(&h.server, "PROJ-123").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "Client:id=1",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            ":id echo case: expected exit 0; stderr={stderr}"
        );
        assert!(
            stderr.contains("Client \u{2192} 1"),
            ":id hint must echo the raw id literal verbatim; stderr={stderr}"
        );
    }

    // :name form -> VALUE verbatim, no lookup.
    {
        let h = Harness::new().await;
        mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
        mount_createmeta_fields_single_page(
            &h.server,
            "PROJ",
            "10000",
            vec![createmeta_string_field("customfield_10099", "Free Text")],
        )
        .await;
        mount_list_fields(
            &h.server,
            vec![json!({"id": "customfield_10099", "name": "Free Text"})],
        )
        .await;
        mount_create_post(&h.server, "PROJ-123").await;
        let output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                "PROJ",
                "--type",
                "Task",
                "--summary",
                "test",
                "--field",
                "Free Text:name=Verbatim Value",
                "--no-input",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            ":name echo case: expected exit 0; stderr={stderr}"
        );
        assert!(
            stderr.contains("Free Text \u{2192} Verbatim Value"),
            ":name hint must echo VALUE verbatim; stderr={stderr}"
        );
    }
}

/// AC-013 (BC-3.4.014 amended `--field` bullet, adversarial review Pass 1
/// LOW finding): `:asset` echoes the composite `"<workspaceId>:<objectId>"`
/// string in the platform create path's success echo. Uses the BARE
/// `:asset=OBJECTID` form with a COLD workspace-id cache — the returned
/// workspace id ("ws-999") is deliberately distinct from the input objectId
/// ("789") so a passing assertion proves genuine `<workspaceId>:<objectId>`
/// composition rather than an echo of the raw `--field` input.
#[tokio::test]
async fn test_bc_3_4_014_field_echo_asset_composite() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_any_field("customfield_60010", "Asset Field")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_60010", "name": "Asset Field"})],
    )
    .await;
    // Cold-cache workspace discovery (mirrors mount_workspace_ok in
    // tests/issue_field_hint_kinds.rs) — no pre-populated workspace.json.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{ "workspaceId": "ws-999" }]
        })))
        .mount(&h.server)
        .await;
    mount_create_post(&h.server, "PROJ-123").await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "Asset Field:asset=789",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-013: :asset echo case must succeed (exit 0); stderr={stderr}"
    );
    assert!(
        stderr.contains("Asset Field \u{2192} ws-999:789"),
        "AC-013: :asset hint must echo the composite '<workspaceId>:<objectId>' \
         string in the success echo; stderr={stderr}"
    );
}

/// AC-013 (BC-3.4.014 amended `--field` bullet, adversarial review Pass 1
/// LOW finding): a cascading `:option` value (`Parent>Child`) echoes
/// `"<parent> > <child>"` in the platform create path's success echo,
/// mirroring `tests/issue_field_hint_kinds.rs`'s
/// `test_changed_fields_echo_per_hint_kind` cascading case for the edit
/// path.
#[tokio::test]
async fn test_bc_3_4_014_field_echo_cascading_option() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_cascading_option_field(
            "customfield_20002",
            "Cascade Field",
            vec![json!({
                "id": "1", "value": "Parent",
                "children": [{"id": "2", "value": "Child"}]
            })],
        )],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_20002", "name": "Cascade Field"})],
    )
    .await;
    mount_create_post(&h.server, "PROJ-123").await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "Cascade Field:option=Parent>Child",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-013: cascading :option echo case must succeed (exit 0); stderr={stderr}"
    );
    assert!(
        stderr.contains("Cascade Field \u{2192} Parent > Child"),
        "AC-013: cascading :option must echo '<parent> > <child>' in the \
         success echo; stderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Adversary Pass 4 MEDIUM: create-path POST-body wire-shape assertions.
// `mount_create_post` (used throughout this file) is a free-fire 201 mock —
// it accepts ANY body, so a wrong wire shape on the create-path merge would
// go uncaught by every test above. These tests use a dedicated
// `body_partial_json`-gated mock with `.expect(1)`, mirroring the exact
// idiom `tests/issue_field_hint_kinds.rs` uses for its editmeta wire-shape
// pins (e.g. its `:id`/`:name` EC-8/EC-9 mocks): the mock's 201 response
// only fires when the POST body genuinely contains the expected
// `{"fields": {...}}` shape. A mismatched body leaves the mock unmatched
// (wiremock 404s the request) and `MockServer`'s Drop-time expectation
// check panics.
// ═══════════════════════════════════════════════════════════════════════════

/// `:id` hint composes `{"fields":{"customfield_XXXXX":{"id":"<value>"}}}` —
/// `compose_id_hint` sends VALUE verbatim, no `allowedValues` lookup.
#[tokio::test]
async fn test_bc_3_3_010_create_post_body_wire_shape_id() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_string_field("customfield_10084", "Client")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10084", "name": "Client"})],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(
            json!({"fields": {"customfield_10084": {"id": "1"}}}),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", h.server.uri()),
        })))
        .expect(1)
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "Client:id=1",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "wire-shape :id: expected exit 0 (POST body must match); stderr={stderr}"
    );
}

/// Bare/`:option` form composes `{"fields":{"customfield_XXXXX":{"id":"<resolved-option-id>"}}}`
/// — `resolve_option_value` matches VALUE against `allowedValues` and sends
/// the matched entry's `id`, never the label itself.
#[tokio::test]
async fn test_bc_3_3_010_create_post_body_wire_shape_option() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_option_field(
            "customfield_10084",
            "Client",
            vec![json!({"id": "1", "value": "Client A"})],
        )],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10084", "name": "Client"})],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(
            json!({"fields": {"customfield_10084": {"id": "1"}}}),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", h.server.uri()),
        })))
        .expect(1)
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "Client=Client A",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "wire-shape bare/:option: expected exit 0 (POST body must match); stderr={stderr}"
    );
}

/// `:name` hint composes `{"fields":{"customfield_XXXXX":{"name":"<value>"}}}`
/// — `compose_name_hint` sends VALUE verbatim, no lookup.
#[tokio::test]
async fn test_bc_3_3_010_create_post_body_wire_shape_name() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_string_field("customfield_10099", "Free Text")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10099", "name": "Free Text"})],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(
            json!({"fields": {"customfield_10099": {"name": "Verbatim Value"}}}),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", h.server.uri()),
        })))
        .expect(1)
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "Free Text:name=Verbatim Value",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "wire-shape :name: expected exit 0 (POST body must match); stderr={stderr}"
    );
}

/// `:asset` hint composes the CMDB object-reference array shape
/// `{"fields":{"customfield_XXXXX":[{"workspaceId":"<ws>","id":"<ws>:<obj>","objectId":"<obj>"}]}}`
/// — matches `compose_asset_hint`'s exact output shape (`src/cli/issue/field_resolve.rs`).
/// Bare `:asset=OBJECTID` form with a cold workspace-id cache (mirrors
/// `test_bc_3_4_014_field_echo_asset_composite` above): the returned
/// workspace id ("ws-999") is deliberately distinct from the input objectId
/// ("789") so a passing assertion proves genuine composition, not an echo.
#[tokio::test]
async fn test_bc_3_3_010_create_post_body_wire_shape_asset() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_any_field("customfield_60010", "Asset Field")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_60010", "name": "Asset Field"})],
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{ "workspaceId": "ws-999" }]
        })))
        .mount(&h.server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(json!({
            "fields": {
                "customfield_60010": [
                    {"workspaceId": "ws-999", "id": "ws-999:789", "objectId": "789"}
                ]
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", h.server.uri()),
        })))
        .expect(1)
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "Asset Field:asset=789",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "wire-shape :asset: expected exit 0 (POST body must match); stderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-014: JSON mode is UNCHANGED — no `changed_fields` key added.
// ═══════════════════════════════════════════════════════════════════════════

/// AC-014 (BC-3.4.014 "JSON mode is UNCHANGED"): `issue create --output json`
/// with a resolved `--field` performs the same follow-up GET as always — no
/// `changed_fields` key is added to the JSON output.
#[tokio::test]
async fn test_bc_3_4_014_json_mode_unchanged_no_changed_fields_key() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_string_field("customfield_10077", "A Field")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10077", "name": "A Field"})],
    )
    .await;
    mount_create_post(&h.server, "PROJ-123").await;
    // Follow-up GET (issue view shape) for --output json.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "10001",
            "key": "PROJ-123",
            "fields": {"summary": "test", "status": {"name": "To Do"}}
        })))
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "A Field=x",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-014: expected exit 0; stderr={stderr}"
    );
    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("AC-014: stdout must be valid JSON: {e}; stdout={stdout}"));
    assert!(
        parsed.get("changed_fields").is_none(),
        "AC-014: JSON output must NOT gain a changed_fields key; got: {parsed}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-015: field resolution runs AFTER project/type resolution, BEFORE POST
// (Invariant 1).
// ═══════════════════════════════════════════════════════════════════════════

/// AC-015 (BC-3.3.010 Invariant 1): when project/type cannot be resolved
/// (no `--project`, no profile default, `--no-input`), the PRE-EXISTING
/// project-resolution error fires BEFORE `--field` createmeta resolution is
/// ever attempted — no createmeta HTTP call occurs.
#[tokio::test]
async fn test_bc_3_3_010_field_resolution_ordering_after_project_type_before_post() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // createmeta / list_fields must NEVER be called — project resolution
    // fails first.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args([
            "issue",
            "create",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "A Field=x",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-015: expected exit 64 (project-key resolution failure); stderr={stderr}"
    );
    assert!(
        stderr.contains("Project key is required"),
        "AC-015: the PRE-EXISTING project-resolution error must fire; stderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-018: `--markdown --field description=x` intersection with the
// (now-removed) DEC-188 guard — regression check only.
// ═══════════════════════════════════════════════════════════════════════════

/// AC-018 / EC-3.8.012-5 (now stale post-reversal): `--markdown --field
/// description=x` WITHOUT `--request-type` no longer fires BC-3.8.012's
/// removed guard.
///
/// Adversarial review Pass 5 LOW fixture-realism fix: the original version of
/// this test mounted a `string`-typed `description` createmeta fixture, which
/// is unrealistic — a real Jira Cloud `description` field's schema type is
/// `"doc"` (an ADF document), not `"string"`. `dispatch_field_value`'s
/// bare-form type dispatch (`src/cli/issue/field_resolve.rs`) has no `"doc"`
/// arm, so a genuinely realistic invocation correctly falls through to the
/// `unsupported_field_type_error` branch (exit 64, "which is not supported by
/// `--field`") rather than succeeding. This test's PRIMARY, load-bearing
/// assertion is unchanged from the original intent: stderr does NOT contain
/// the removed DEC-188 guard string (`"--field is only valid with"`) — proof
/// that resolution genuinely ran (createmeta was fetched, the field was
/// located, dispatch was attempted) rather than the invocation being rejected
/// pre-flight by the old, now-reversed guard. The exit code and error message
/// simply reflect what an honest `"doc"`-typed fixture actually produces.
///
/// IMPORTANT — do not read this test as proving `--markdown` applies to a
/// `--field` value: it does not. `--field` VALUEs are never passed through
/// `markdown_to_adf` at any point in `resolve_against_createmeta` /
/// `dispatch_field_value` — only the dedicated `--description` /
/// `--description-stdin` flag's text goes through `adf::markdown_to_adf`
/// (`create.rs`, gated on `desc_text`, entirely independent of `--field`).
/// `--field description=**bold**` here sends the literal string `**bold**`
/// unmodified into whichever type arm `description`'s schema type dispatches
/// to; in this realistic fixture that arm is the unsupported-type error, so
/// the literal value is never even sent to Jira.
#[tokio::test]
async fn test_ec_3_8_012_5_markdown_field_description_no_longer_guarded() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_doc_field("description", "Description")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "description", "name": "Description"})],
    )
    .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--markdown",
            "--field",
            "description=**bold**",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-018 (realistic fixture): a 'doc'-typed description field has no \
         supported bare-form type arm — expect exit 64 via \
         unsupported_field_type_error, proving resolution ran past the \
         removed guard rather than being rejected pre-flight by it; \
         stderr={stderr}"
    );
    assert!(
        stderr.contains("which is not supported by `--field`"),
        "AC-018: must fail via the unsupported-field-type arm specifically \
         (proof that createmeta resolution genuinely engaged), not some \
         unrelated error; stderr={stderr}"
    );
    assert!(
        !stderr.contains("--field is only valid with"),
        "AC-018: the removed DEC-188 guard must not fire; stderr={stderr}"
    );
    assert!(
        !stderr.contains("cannot be combined with `--markdown`"),
        "AC-018: handle_create has no --markdown-requires-description guard \
         of its own (that guard is JSM/edit-only); stderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// AC-016: `--help` text — DEC-310 reversal is complete. `--field`'s help
// line no longer carries "requires --request-type"; the clause survives
// exactly once, scoped to `--on-behalf-of`'s line. (Adversarial review
// Pass 1 LOW finding — these two exact-named tests were mandated by AC-016
// but absent from this file; `tests/issue_create_jsm.rs`'s
// `test_platform_create_help_flags_requires_request_type_in_help` already
// covers the bare count and is unchanged/untouched by this addition.)
// ═══════════════════════════════════════════════════════════════════════════

/// AC-016: the `--field` flag's own `--help` line/block no longer contains
/// the (now-reversed) "requires --request-type" clause. Isolates the
/// `--field` help block specifically — from its own flag marker up to the
/// next flag's marker (`--on-behalf-of`, its immediate successor in
/// declaration order, `src/cli/mod.rs`) — rather than checking the whole
/// `--help` output, so this positively pins WHICH line is clean, distinct
/// from the whole-output count check in the sibling test below.
#[tokio::test]
async fn test_bc_3_8_012_field_help_text_no_longer_requires_request_type() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "create", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Normalization is MANDATORY — clap 4 next-line layout may wrap long doc
    // strings, causing a flag's help text to straddle a newline (same
    // technique as tests/issue_create_jsm.rs's
    // test_platform_create_help_flags_requires_request_type_in_help).
    let normalized = stdout.split_whitespace().collect::<Vec<_>>().join(" ");

    let field_start = normalized
        .find("--field <FIELD>")
        .expect("--field <FIELD> must appear in `jr issue create --help` output");
    let on_behalf_start = normalized
        .find("--on-behalf-of <ON_BEHALF_OF>")
        .expect("--on-behalf-of <ON_BEHALF_OF> must appear in `jr issue create --help` output");
    assert!(
        field_start < on_behalf_start,
        "expected --field's help block to precede --on-behalf-of's in \
         declaration order; normalized={normalized}"
    );
    let field_block = &normalized[field_start..on_behalf_start];

    assert!(
        !field_block.contains("requires --request-type"),
        "AC-016 / BC-3.8.012 (DEC-310 reversal): the --field help line must \
         no longer carry the 'requires --request-type' clause; \
         field_block={field_block}"
    );
}

/// AC-016 (mirrors `tests/issue_create_jsm.rs`'s
/// `test_platform_create_help_flags_requires_request_type_in_help`, added
/// here IN THIS FILE per the story's explicit test-name mandate): the full
/// `jr issue create --help` output contains "requires --request-type"
/// EXACTLY ONCE post-DEC-310-reversal, and that single occurrence lies
/// within the `--on-behalf-of` help block (i.e. AFTER `--on-behalf-of`'s own
/// flag marker) — not on `--field`'s line.
#[tokio::test]
async fn test_ac12_help_text_substring_count_is_1_on_behalf_of_only() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "create", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let normalized = stdout.split_whitespace().collect::<Vec<_>>().join(" ");

    assert_eq!(
        normalized.matches("requires --request-type").count(),
        1,
        "AC-016: 'requires --request-type' must appear EXACTLY ONCE in the \
         full --help output post-DEC-310-reversal; normalized={normalized}"
    );

    let on_behalf_start = normalized
        .find("--on-behalf-of <ON_BEHALF_OF>")
        .expect("--on-behalf-of <ON_BEHALF_OF> must appear in `jr issue create --help` output");
    let requires_idx = normalized
        .find("requires --request-type")
        .expect("presence already asserted above");

    assert!(
        requires_idx > on_behalf_start,
        "AC-016: the sole 'requires --request-type' occurrence must fall \
         within the --on-behalf-of help block (after its flag marker), not \
         on the --field line; normalized={normalized}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Adversary Pass 2 LOW: create-vs-edit behavioral asymmetry — two distinct
// `--field` NAME keys resolving to the SAME field_id must not falsely report
// "not on the Create screen" for the second occurrence.
// ═══════════════════════════════════════════════════════════════════════════

/// AC-008 (BC-3.3.010/BC-3.3.011): `resolve_against_createmeta` must mirror
/// `resolve_against_editmeta`'s NON-consuming `editmeta.fields.get(&field_id)`
/// lookup rather than `meta_by_id.remove(&field_id)`. `parse_field_kv`'s
/// last-wins collapse only dedupes identical NAME keys — `customfield_10050=a`
/// and `"Display Name"=b` (where "Display Name" resolves via `list_fields` to
/// the SAME `customfield_10050`) survive Phase 1 as two DISTINCT map keys that
/// both resolve to `customfield_10050` in Phase 2. With a consuming
/// `.remove()`, the first pair's lookup succeeds and deletes the createmeta
/// entry; the second pair's lookup then spuriously fails with "is not on the
/// Create screen" even though the field IS present. The edit path's `.get()`
/// arm does not have this defect (both lookups succeed; last write wins on
/// the wire) — this test drives the same reachable input through the create
/// path and asserts it behaves identically: exit 0, the POST fires, and no
/// false "not on the Create screen" error.
#[tokio::test]
async fn test_bc_3_3_010_two_names_same_field_id_resolve_like_edit_no_false_screen_error() {
    let h = Harness::new().await;
    mount_issue_types(&h.server, "PROJ", &[("10000", "Task")]).await;
    mount_createmeta_fields_single_page(
        &h.server,
        "PROJ",
        "10000",
        vec![createmeta_string_field("customfield_10050", "Display Name")],
    )
    .await;
    mount_list_fields(
        &h.server,
        vec![json!({"id": "customfield_10050", "name": "Display Name"})],
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", h.server.uri()),
        })))
        .expect(1)
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test",
            "--field",
            "customfield_10050=a",
            "--field",
            "Display Name=b",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "adversary Pass 2 LOW: two --field pairs resolving to the same \
         field_id must succeed like the edit path does, not falsely report \
         the field missing from the Create screen; got {:?}. stderr={stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("is not on the Create screen"),
        "adversary Pass 2 LOW: false 'not on the Create screen' error must \
         not appear when the field IS present in createmeta; stderr={stderr}"
    );
}

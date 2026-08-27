//! Integration tests for S-578-2: `issue edit --field NAME:kind=VALUE` hint-kind
//! dispatch (`:option`/`:id`/`:name`/`:asset`), cascading-select composition, and
//! the dry-run `plannedChanges` per-hint-kind preview.
//!
//! BC-3.4.015 (amended): bare-form dispatch unchanged/permanent; `>` is a literal
//!   character in the bare form.
//! BC-3.4.016 (amended): `:option`'s label→id auto-detect dispatch is the SAME
//!   path the bare form already uses.
//! BC-3.4.021 (amended): `--dry-run` `plannedChanges` per-hint-kind wire-shape
//!   preview (documented exception to the general display-value-string rule).
//! BC-3.4.027: `--field NAME:option=VALUE` — cascading `Parent>Child` composition,
//!   non-cascading-field `>` collision (D4).
//! BC-3.4.028: `--field NAME:id=VALUE` — verbatim `{"id":"<VALUE>"}`.
//! BC-3.4.029: `--field NAME:name=VALUE` — verbatim `{"name":"<VALUE>"}`.
//! BC-3.4.030: `--field NAME:asset=WORKSPACE:OBJECTID` — Assets object-reference
//!   array composition + cold-cache workspace-discovery error taxonomy.
//! BC-3.4.031: malformed-hint edge cases (regression pass at this call site).
//!
//! MERGED / GREEN STATE (S-578-2): all `--field NAME:kind=VALUE` dispatch is
//! now fully implemented. `field_resolve.rs`'s four composers (`:option`,
//! `:id`, `:name`, `:asset`) build real ADF/JSON payloads instead of
//! panicking, and `edit.rs` dispatches hinted `--field` pairs directly
//! through `resolve_edit_fields` — the S-578-1 interim guard
//! (`reject_unsupported_hint_kinds`) has been removed from the call site, so
//! every test below exercises the real, per-kind composer behavior end to
//! end rather than any placeholder rejection.
//!
//! Historical (RED gate, now closed): earlier revisions of this file, staged
//! ahead of the S-578-2 implementation landing, described every hinted test
//! below as deterministically red — blocked by the (now-removed) interim
//! guard exiting 64 with its generic "not yet supported" message before the
//! composers were ever reached. That framing is retained here only as
//! historical context; it no longer describes current behavior.
//!
//! The proptests in this file that assert on the ABSENCE of the interim
//! guard's generic message remain meaningful post-merge: they confirm dispatch
//! reached the real per-kind composer path rather than short-circuiting on
//! the old guard, which is still a valid regression signal even though the
//! guard itself is gone.
//!
//! THREE test functions in this file are expected to PASS immediately (not
//! forced red — see the comment on each):
//!   - `test_bc_3_4_015_bare_form_greater_than_is_literal_falls_through_to_ec_3_4_016_2`
//!     (AC-005): exercises the BARE form only (`spec.kind == None`), which is
//!     pre-existing, unmodified BC-3.4.015/016 code untouched by this story
//!     and never reaches the interim guard (which only fires on
//!     `spec.kind.is_some()`).
//!   - one sub-assertion inside `test_ec6_ec7_ec8_ec9_regression_at_edit_call_site`
//!     (AC-014, EC-7): the unknown-kind rejection happens inside
//!     `parse_field_kv` itself (S-578-1, already implemented), before the
//!     interim guard is ever reached.
//!   - AC-011's serde round-trip test lives in
//!     `src/types/jira/editmeta.rs`'s inline `#[cfg(test)]` module, not this
//!     file, and already passes (the `children` field was added by S-580-1).

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use proptest::prelude::*;
use wiremock::matchers::{any, body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helpers (mirrors tests/issue_edit_field.rs's conventions)
// ---------------------------------------------------------------------------

fn jr_cmd_with_xdg(
    server_url: &str,
    cache_dir: &std::path::Path,
    config_dir: &std::path::Path,
) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("JR_CACHE_DIR", cache_dir.join("jr"))
        .env("XDG_CONFIG_HOME", config_dir)
        .env("JR_CONFIG_DIR", config_dir.join("jr"));
    cmd
}

/// Mount `GET /rest/api/3/field` returning the given `(id, name)` list.
async fn mount_list_fields(server: &MockServer, fields: &[(&str, &str)]) {
    let body: Vec<serde_json::Value> = fields
        .iter()
        .map(|(id, name)| serde_json::json!({"id": id, "name": name, "custom": true}))
        .collect();
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

/// Build a single editmeta field descriptor.
fn field_descriptor(
    name: &str,
    field_type: &str,
    allowed_values: Option<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "schema": { "type": field_type, "system": null, "custom": null },
        "operations": ["set"],
        "required": false,
        "allowedValues": allowed_values
    })
}

/// Mount `GET /rest/api/3/issue/{key}/editmeta` with a single field entry.
async fn mount_editmeta_one(
    server: &MockServer,
    key: &str,
    field_id: &str,
    field_json: serde_json::Value,
) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}/editmeta")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": { field_id: field_json }
        })))
        .mount(server)
        .await;
}

/// Mount `PUT /rest/api/3/issue/{key}` returning 204, no body assertion.
async fn mount_put_204(server: &MockServer, key: &str) {
    Mock::given(method("PUT"))
        .and(path(format!("/rest/api/3/issue/{key}")))
        .respond_with(ResponseTemplate::new(204))
        .mount(server)
        .await;
}

/// Mount `PUT /rest/api/3/issue/{key}` asserting a partial-JSON body match.
async fn mount_put_204_with_body(
    server: &MockServer,
    key: &str,
    body: serde_json::Value,
    expect: u64,
) {
    Mock::given(method("PUT"))
        .and(path(format!("/rest/api/3/issue/{key}")))
        .and(body_partial_json(body))
        .respond_with(ResponseTemplate::new(204))
        .expect(expect)
        .mount(server)
        .await;
}

/// Mount `GET /rest/servicedeskapi/assets/workspace` returning a single
/// workspace id.
async fn mount_workspace_ok(server: &MockServer, workspace_id: &str) {
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{ "workspaceId": workspace_id }]
        })))
        .mount(server)
        .await;
}

/// Write a `workspace.json` cache file (BC-4.2.001 `WorkspaceCache` shape) so
/// `get_or_fetch_workspace_id` hits a WARM cache and performs zero HTTP.
fn write_workspace_cache_file(cache_home: &std::path::Path, profile: &str, workspace_id: &str) {
    let dir = cache_home.join("jr").join("v1").join(profile);
    std::fs::create_dir_all(&dir).unwrap();
    let cache = serde_json::json!({
        "workspace_id": workspace_id,
        "fetched_at": chrono::Utc::now().to_rfc3339()
    });
    std::fs::write(
        dir.join("workspace.json"),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();
}

/// Literal substring emitted by the S-578-1 interim guard
/// (`reject_unsupported_hint_kinds`, `src/cli/issue/create.rs`). Every test
/// below that supplies a hinted `--field` currently observes THIS message
/// instead of the real per-kind behavior — that is the Red Gate failure mode
/// for this story.
const INTERIM_GUARD_MSG: &str = "not yet supported on this command";

// ---------------------------------------------------------------------------
// AC-001 (BC-3.4.015 "Hint-syntax interaction" amendment, traces to nothing
// prior) — hinted-bypass dispatch runs BEFORE the existing schema.type match.
// ---------------------------------------------------------------------------

/// A hinted pair must bypass Step 4's schema.type dispatch ENTIRELY — even a
/// field whose schema.type the bare-form dispatch outright REJECTS ("array",
/// EC-3.4.015-5 shape) must succeed under `:id`, because the hinted branch
/// never reaches the type-dispatch match at all.
///
/// Red today: the interim guard intercepts before `resolve_edit_fields` is
/// even called, so this exits 64 with the guard's generic message instead of
/// succeeding with the composed `{"id": "999"}` wire body.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_hinted_bypass_runs_before_bare_dispatch() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // schema.type == "array" is REJECTED by the bare-form dispatch
    // (EC-3.4.015-5 "type not supported" shape) — proving the hinted branch
    // never reaches that match arm at all.
    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10001",
        field_descriptor("Weird Field", "array", None),
    )
    .await;
    mount_put_204_with_body(
        &server,
        "TEST-1",
        serde_json::json!({"fields": {"customfield_10001": {"id": "999"}}}),
        1,
    )
    .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10001:id=999",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "AC-001: :id hint on an 'array'-typed field must bypass the type \
         dispatch and succeed; stderr={stderr} stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON: {e}; stdout={stdout}"));
    assert_eq!(
        parsed["changed_fields"]["customfield_10001"].as_str(),
        Some("999"),
        "AC-001: changed_fields echo must be the raw id literal; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 (BC-3.4.027 Description, VP-578-007) — `:option` non-cascading is
// byte-identical to the bare-form auto-detect.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_027_option_hint_non_cascading_byte_identical_to_bare() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, &[("customfield_10176", "Urgency")]).await;
    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10176",
        field_descriptor(
            "Urgency",
            "option",
            Some(serde_json::json!([
                {"id": "10286", "value": "High"},
                {"id": "10287", "value": "Medium"}
            ])),
        ),
    )
    .await;
    let expected_body = serde_json::json!({"fields": {"customfield_10176": {"id": "10286"}}});
    mount_put_204_with_body(&server, "TEST-1", expected_body, 2).await;

    // (a) bare form — pre-existing, unaffected by this story.
    let bare = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Urgency=High",
        ])
        .output()
        .unwrap();
    assert!(
        bare.status.success(),
        "bare form must succeed (pre-existing behavior); stderr={}",
        String::from_utf8_lossy(&bare.stderr)
    );

    // (b) explicit `:option` hint — MUST produce the identical wire body.
    let hinted = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "Urgency:option=High",
        ])
        .output()
        .unwrap();
    let hinted_stderr = String::from_utf8_lossy(&hinted.stderr);
    assert!(
        hinted.status.success(),
        "AC-002: :option hint must succeed exactly like the bare form; \
         stderr={hinted_stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-003 (BC-3.4.027 "Cascading-select composition", VP-578-008) —
// `str::split_once('>')`, `{"value":..,"child":{"value":..}}` wire shape.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_027_cascading_split_once_wire_shape() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_20002",
        field_descriptor(
            "Cascade Field",
            "option-with-child",
            Some(serde_json::json!([
                {
                    "id": "1", "value": "Parent",
                    "children": [{"id": "2", "value": "Child"}]
                }
            ])),
        ),
    )
    .await;
    mount_put_204_with_body(
        &server,
        "TEST-1",
        serde_json::json!({
            "fields": {
                "customfield_20002": {"value": "Parent", "child": {"value": "Child"}}
            }
        }),
        1,
    )
    .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_20002:option=Parent>Child",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-003: cascading :option must compose the {{value,child}} wire shape; \
         stderr={stderr} stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    // NOTE: keyed by the literal `customfield_20002` bypass name, not the
    // editmeta's own field name ("Cascade Field") — the `customfield_NNNNN`
    // literal bypass (BC-3.4.015 Step 1) sets `human_name = name.clone()`
    // (the literal itself) unconditionally, a resolution shared identically
    // by the bare-form and hinted dispatch paths (Architecture Compliance
    // Rule 1). Corrected to match the identical-scenario sibling assertions
    // in `test_bc_3_4_021_dry_run_option_hint_cascading_preview_shape` and
    // `test_changed_fields_echo_per_hint_kind`, both of which key on
    // `"customfield_20002"` for this exact same `--field
    // customfield_20002:option=Parent>Child` input.
    assert_eq!(
        parsed["changed_fields"]["customfield_20002"].as_str(),
        Some("Parent > Child"),
        "AC-003: changed_fields echo must be '<parent> > <child>'; stdout={stdout}"
    );
}

/// AC-003 (D3 multibyte MUST): no panic over arbitrary UTF-8 input at the
/// `>` split site, including a multibyte scalar adjacent to the delimiter
/// (e.g. `Pré>Bñ`, EC-3.4.027-5). Uses `proptest!` with a shared, lazily
/// initialized wiremock fixture so each of the (small) case count spawns one
/// real `jr` subprocess.
///
/// RED today: every generated case hits the interim guard, which never
/// panics but ALSO never performs the real dispatch — the
/// `!stderr.contains(INTERIM_GUARD_MSG)` assertion fails on every case,
/// deterministically red. The panic-freedom assertions are forward-looking:
/// they remain meaningful (and must keep holding) once S-578-2 implements
/// the real composer.
fn hint_dispatch_fixture() -> &'static (
    tokio::runtime::Runtime,
    MockServer,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    static CELL: std::sync::OnceLock<(
        tokio::runtime::Runtime,
        MockServer,
        tempfile::TempDir,
        tempfile::TempDir,
    )> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        let server = rt.block_on(async {
            let server = MockServer::start().await;
            mount_list_fields(&server, &[("customfield_30001", "cf")]).await;
            mount_editmeta_one(
                &server,
                "TEST-1",
                "customfield_30001",
                field_descriptor(
                    "cf",
                    "option-with-child",
                    Some(serde_json::json!([
                        {
                            "id": "1", "value": "Parent",
                            "children": [{"id": "2", "value": "Child"}]
                        }
                    ])),
                ),
            )
            .await;
            mount_workspace_ok(&server, "ws-prop").await;
            // Catch-all: any PUT (or otherwise-unmatched request) succeeds —
            // registered LAST so the specific mounts above win on their
            // matching paths (wiremock 0.6 FIFO ordering).
            Mock::given(any())
                .respond_with(ResponseTemplate::new(204))
                .mount(&server)
                .await;
            server
        });
        (rt, server, cache_dir, config_dir)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn prop_cascading_split_no_panic(val in "[^\\x00]{0,24}") {
        let (_rt, server, cache_dir, config_dir) = hint_dispatch_fixture();
        let field_arg = format!("cf:option={val}");
        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args(["--no-input", "issue", "edit", "TEST-1", "--field", &field_arg])
            .output()
            .expect("subprocess must spawn");

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        prop_assert_ne!(
            output.status.code(),
            Some(101),
            "process appears to have panicked (exit 101) for input {:?}; stderr={}",
            val,
            stderr
        );
        prop_assert!(
            !stderr.contains("panicked at"),
            "stderr shows a real panic for input {:?}: {}",
            val,
            stderr
        );
        prop_assert!(
            !stderr.contains(INTERIM_GUARD_MSG),
            "RED: hinted :option dispatch is still intercepted by the interim \
             guard for input {:?}; stderr={}",
            val,
            stderr
        );
    }
}

// ---------------------------------------------------------------------------
// AC-004 (BC-3.4.027 "Non-cascading-field collision" D4, EC-3.4.027-7,
// VP-578-023) — structural children-empty detection, distinct exit-64.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_027_ec7_non_cascading_collision_distinct_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_20003",
        field_descriptor(
            "Plain Option Field",
            "option",
            Some(serde_json::json!([
                {"id": "1", "value": "A", "children": []}
            ])),
        ),
    )
    .await;
    // PUT must never be called — this is a client-side exit-64.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_20003:option=A>B",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-004: A>B against a plain (non-cascading) option field must exit \
         64; stderr={stderr}"
    );
    assert!(
        stderr.contains("is not a cascading select"),
        "AC-004: load-bearing substring 'is not a cascading select' missing; \
         stderr={stderr}"
    );
    assert!(
        stderr.contains("remove the"),
        "AC-004: load-bearing substring 'remove the' missing; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-005 (BC-3.4.015 "`>` is a LITERAL character in the bare form" amendment,
// VP-578-023 sibling assertion).
//
// NOTE — this test is expected to PASS TODAY, not forced red. It exercises
// the BARE form only (`spec.kind == None`), which is pre-existing,
// UNCHANGED BC-3.4.015/016 code this story does not touch. Because the
// interim guard only intercepts `spec.kind.is_some()` pairs, a bare-form
// invocation never reaches it at all — this is a regression pin for
// already-correct behavior, the third "may legitimately pass immediately"
// test alongside AC-011 and AC-015 (see the module doc comment above).
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_bare_form_greater_than_is_literal_falls_through_to_ec_3_4_016_2() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_20004",
        field_descriptor(
            "Cascade-ish Field",
            "option",
            Some(serde_json::json!([{"id": "1", "value": "ParentOnly"}])),
        ),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_20004=Parent>Child",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-005: bare 'Parent>Child' must fail as one opaque unresolved \
         candidate; stderr={stderr}"
    );
    assert!(
        !stderr.contains("is not a cascading select"),
        "AC-005: bare form must NOT surface EC-3.4.027-7's distinct message; \
         stderr={stderr}"
    );
    assert!(
        stderr.contains("Allowed values"),
        "AC-005: must fall through to the EC-3.4.016-2 'unresolvable value' \
         shape; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-006 (BC-3.4.028 Description/Postconditions, VP-578-009) — `:id`
// bypasses `allowedValues` entirely, even when the array is EMPTY.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_028_id_hint_bypasses_allowed_values_lookup() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10286",
        field_descriptor("Blocker", "option", Some(serde_json::json!([]))),
    )
    .await;
    mount_put_204_with_body(
        &server,
        "TEST-1",
        serde_json::json!({"fields": {"customfield_10286": {"id": "10286"}}}),
        1,
    )
    .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10286:id=10286",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-006: :id must bypass an EMPTY allowedValues array and succeed; \
         stderr={stderr} stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        parsed["changed_fields"]["customfield_10286"].as_str(),
        Some("10286"),
        "AC-006: changed_fields echo must be the raw id literal, no reverse \
         lookup; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-007 (BC-3.4.029 Description/Postconditions/Invariant 2, VP-578-010) —
// `:name` verbatim; `priority:name=Medium` byte-identical to `--priority`.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_029_name_hint_priority_byte_identical_to_dedicated_flag() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_list_fields(&server, &[("priority", "Priority")]).await;
    mount_editmeta_one(
        &server,
        "TEST-1",
        "priority",
        field_descriptor("Priority", "priority", None),
    )
    .await;
    let expected_body = serde_json::json!({"fields": {"priority": {"name": "Medium"}}});
    mount_put_204_with_body(&server, "TEST-1", expected_body, 2).await;

    // (a) dedicated --priority flag — pre-existing, unaffected by this story.
    let dedicated = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--priority",
            "Medium",
        ])
        .output()
        .unwrap();
    assert!(
        dedicated.status.success(),
        "dedicated --priority flag must succeed (pre-existing behavior); \
         stderr={}",
        String::from_utf8_lossy(&dedicated.stderr)
    );

    // (b) `:name`-hinted --field — MUST produce the identical wire body.
    let hinted = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "priority:name=Medium",
        ])
        .output()
        .unwrap();
    let hinted_stderr = String::from_utf8_lossy(&hinted.stderr);
    assert!(
        hinted.status.success(),
        "AC-007: --field priority:name=Medium must produce byte-identical \
         wire output to --priority Medium; stderr={hinted_stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-008 (BC-3.4.030 Parsing rules/Postconditions, VP-578-011) — `:asset`
// composes `[{workspaceId,id,objectId}]` from `WORKSPACE:OBJECTID`.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_asset_bare_form_warm_cache_zero_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    write_workspace_cache_file(cache_dir.path(), "default", "ws-777");
    // WARM cache: the workspace-discovery GET must NEVER be called.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10001",
        field_descriptor("Asset Field", "any", None),
    )
    .await;
    mount_put_204_with_body(
        &server,
        "TEST-1",
        serde_json::json!({
            "fields": {
                "customfield_10001": [
                    {"workspaceId": "ws-777", "id": "ws-777:456", "objectId": "456"}
                ]
            }
        }),
        1,
    )
    .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10001:asset=456",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-008: bare :asset= form must resolve workspace id from the warm \
         cache with zero HTTP; stderr={stderr} stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        parsed["changed_fields"]["customfield_10001"].as_str(),
        Some("ws-777:456"),
        "AC-008/AC-017: changed_fields echo must be '<workspaceId>:<objectId>'; \
         stdout={stdout}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_asset_explicit_workspace_form() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Explicit workspaceId form must NEVER call workspace discovery either.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10002",
        field_descriptor("Asset Field 2", "any", None),
    )
    .await;
    mount_put_204_with_body(
        &server,
        "TEST-1",
        serde_json::json!({
            "fields": {
                "customfield_10002": [
                    {"workspaceId": "wsXYZ", "id": "wsXYZ:456", "objectId": "456"}
                ]
            }
        }),
        1,
    )
    .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10002:asset=wsXYZ:456",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-008: explicit workspaceId:objectId form must succeed without any \
         workspace-discovery HTTP; stderr={stderr} stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        parsed["changed_fields"]["customfield_10002"].as_str(),
        Some("wsXYZ:456"),
        "stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 (BC-3.4.030 Error taxonomy / BC-3.4.031 EC-2/EC-3, VP-578-012) —
// `:asset` composer safety: malformed shapes exit 64 before any HTTP.
// ---------------------------------------------------------------------------

/// EC-2a: `--field cf:asset=` (empty value) → exit 64, "asset reference
/// cannot be empty".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_031_ec2a_empty_asset_value_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10001",
        field_descriptor("Asset Field", "any", None),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10001:asset=",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(64), "stderr={stderr}");
    assert!(
        stderr.contains("asset reference cannot be empty"),
        "EC-2a: stderr={stderr}"
    );
}

/// EC-2b: `--field cf:asset=ws:` (objectId segment empty) → exit 64, same
/// message as EC-3.4.030-3 ("objectId must be numeric").
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_031_ec2b_empty_objectid_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10001",
        field_descriptor("Asset Field", "any", None),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10001:asset=ws:",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(64), "stderr={stderr}");
    assert!(
        stderr.contains("objectId must be numeric"),
        "EC-2b: stderr={stderr}"
    );
}

/// EC-2c: `--field cf:asset=:12345` (workspace segment empty, colon present)
/// → exit 64, distinct "workspace segment cannot be empty" message. This
/// check MUST run BEFORE the objectId-segment checks — `:asset=:` (both
/// segments empty) must ALSO surface this same EC-2c message, never EC-2b's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_031_ec2c_empty_workspace_segment_precedence() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10001",
        field_descriptor("Asset Field", "any", None),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    for value in [":12345", ":"] {
        let field_arg = format!("customfield_10001:asset={value}");
        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                &field_arg,
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "value={value:?}; stderr={stderr}"
        );
        assert!(
            stderr.contains("workspace segment cannot be empty"),
            "EC-2c value={value:?}: stderr={stderr}"
        );
        assert!(
            stderr.contains("omit the workspace prefix"),
            "EC-2c value={value:?}: stderr={stderr}"
        );
    }
}

/// EC-2d: `--field cf:asset=W:Y:Z` (a second colon inside the value) → exit
/// 64 with a message naming the extra-colon mistake specifically, NOT the
/// generic "objectId must be numeric" text alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_031_ec2d_extra_colon_distinct_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10001",
        field_descriptor("Asset Field", "any", None),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10001:asset=W:Y:Z",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(64), "stderr={stderr}");
    assert!(
        stderr.contains("unexpected extra ':'"),
        "EC-2d: message must name the extra-colon mistake specifically; \
         stderr={stderr}"
    );
}

/// EC-3: `:asset` non-numeric `objectId` — bare form, explicit form, and
/// non-ASCII "numeric" digit scripts (Arabic-Indic/fullwidth) all exit 64
/// with "objectId must be numeric".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_031_ec3_non_ascii_numeric_objectid_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10001",
        field_descriptor("Asset Field", "any", None),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    for value in [
        "abc",
        "ws:abc",
        "\u{0661}\u{0662}\u{0663}",
        "\u{ff11}\u{ff12}\u{ff13}",
    ] {
        let field_arg = format!("customfield_10001:asset={value}");
        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                &field_arg,
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "value={value:?}; stderr={stderr}"
        );
        assert!(
            stderr.contains("objectId must be numeric"),
            "EC-3 value={value:?}: stderr={stderr}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// AC-009 (VP-578-012): no panic over arbitrary UTF-8 input across ALL
    /// malformed `:asset` shapes, including the `WORKSPACE:OBJECTID`
    /// first-colon split's own no-panic corpus, PLUS the PUT-body-is-valid-
    /// JSON assertion below actually gets exercised.
    ///
    /// The `val` strategy is a `prop_oneof!` mix: mostly arbitrary UTF-8 (the
    /// malformed-shape corpus, which never reaches HTTP), but ~29% of cases
    /// are guaranteed all-ASCII-digit strings (`[0-9]{1,10}`) — a valid bare
    /// objectId per `compose_asset_hint`'s parsing rules. Without this
    /// guaranteed-numeric lane, an unbounded `[^\x00]{0,24}` strategy almost
    /// never independently samples a purely-digit string, so the PUT-body
    /// JSON-validity check below would be vacuously true on effectively
    /// every run (the `if let Some(reqs) = ...` / `.filter(PUT)` loop body
    /// never executes) — a dead assertion that always "passes" without ever
    /// checking anything (PR #741 review, S-578-2 fix-burst).
    #[test]
    fn prop_asset_composer_no_malformed_json_ever(
        val in prop_oneof![
            5 => "[^\\x00]{0,24}",
            2 => "[0-9]{1,10}",
        ]
    ) {
        let (rt, server, cache_dir, config_dir) = hint_dispatch_fixture();
        let field_arg = format!("customfield_30001:asset={val}");
        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args(["--no-input", "issue", "edit", "TEST-1", "--field", &field_arg])
            .output()
            .expect("subprocess must spawn");

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        prop_assert_ne!(
            output.status.code(),
            Some(101),
            "process appears to have panicked (exit 101) for input {:?}; stderr={}",
            val,
            stderr
        );
        prop_assert!(
            !stderr.contains("panicked at"),
            "stderr shows a real panic for input {:?}: {}",
            val,
            stderr
        );
        prop_assert!(
            !stderr.contains(INTERIM_GUARD_MSG),
            "regression guard: hinted :asset dispatch must never be \
             intercepted by the retired interim guard for input {:?}; \
             stderr={}",
            val,
            stderr
        );

        // Whenever a PUT reaches the mock server, its body must be valid
        // JSON — never malformed. The `[0-9]{1,10}` lane of the `val`
        // strategy above guarantees this loop body actually executes on a
        // meaningful fraction of cases (a bare numeric objectId resolves
        // and PUTs); the arbitrary-UTF-8 lane mostly exits before any HTTP
        // call and leaves this loop legitimately empty for those cases.
        let requests = rt.block_on(async { server.received_requests().await });
        if let Some(reqs) = requests {
            for req in reqs.iter().filter(|r| r.method.as_str() == "PUT") {
                prop_assert!(
                    serde_json::from_slice::<serde_json::Value>(&req.body).is_ok(),
                    "a PUT request body was not valid JSON for input {:?}",
                    val
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AC-010 (BC-3.4.030 Error taxonomy table, VP-578-022 — 1 of 3 shared call
// sites) — `:asset` cold-cache workspace-discovery failure taxonomy at the
// EDIT-path call site.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_edit_path_asset_cold_cache_403_404_assets_unavailable() {
    for status in [403u16, 404u16] {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        mount_editmeta_one(
            &server,
            "TEST-1",
            "customfield_10001",
            field_descriptor("Asset Field", "any", None),
        )
        .await;
        {
            let _guard = Mock::given(method("GET"))
                .and(path("/rest/servicedeskapi/assets/workspace"))
                .respond_with(
                    ResponseTemplate::new(status).set_body_json(serde_json::json!({
                        "errorMessages": ["nope"], "errors": {}
                    })),
                )
                .mount_as_scoped(&server)
                .await;

            let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
                .args([
                    "--no-input",
                    "issue",
                    "edit",
                    "TEST-1",
                    "--field",
                    "customfield_10001:asset=456",
                ])
                .output()
                .unwrap();

            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                output.status.code(),
                Some(64),
                "status={status}; stderr={stderr}"
            );
            assert!(
                stderr.contains(
                    "Assets is not available on this Jira site. Assets requires \
                     Jira Service Management Premium or Enterprise."
                ),
                "status={status}: stderr={stderr}"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_edit_path_asset_cold_cache_empty_workspace() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10001",
        field_descriptor("Asset Field", "any", None),
    )
    .await;
    {
        let _guard = Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/assets/workspace"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "size": 0, "start": 0, "limit": 25, "isLastPage": true, "values": []
            })))
            .mount_as_scoped(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10001:asset=456",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "stderr={stderr}");
        assert!(
            stderr.contains(
                "No Assets workspace found on this Jira site. Assets requires \
                 Jira Service Management Premium or Enterprise."
            ),
            "stderr={stderr}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_edit_path_asset_cold_cache_401_standard_auth_mapping() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10001",
        field_descriptor("Asset Field", "any", None),
    )
    .await;
    {
        let _guard = Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/assets/workspace"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "errorMessages": ["Client must be authenticated to access this resource."],
                "errors": {}
            })))
            .mount_as_scoped(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10001:asset=456",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(2),
            "401 must use the standard NotAuthenticated mapping (exit 2); \
             stderr={stderr}"
        );
        assert!(stderr.contains("Not authenticated"), "stderr={stderr}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_edit_path_asset_cold_cache_5xx_network_standard_mapping() {
    // (a) 5xx.
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        mount_editmeta_one(
            &server,
            "TEST-1",
            "customfield_10001",
            field_descriptor("Asset Field", "any", None),
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/assets/workspace"))
            .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
                "errorMessages": ["Internal server error"], "errors": {}
            })))
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10001:asset=456",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "5xx must use the standard ApiError mapping (exit 1); stderr={stderr}"
        );
        assert!(stderr.contains("API error (500)"), "stderr={stderr}");
    }

    // (b) network error — connect-refused (privileged port 1, matches the
    // established convention in tests/assets_errors.rs). This necessarily
    // exercises the FIRST HTTP call the edit-path resolution chain makes
    // (editmeta), not exclusively the workspace-discovery GET, since jr's
    // single JR_BASE_URL applies to every call — but it demonstrates the
    // identical standard NetworkError/exit-1 mapping this taxonomy row
    // requires for the same underlying client machinery
    // `get_or_fetch_workspace_id` shares.
    {
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        let output = jr_cmd_with_xdg("http://127.0.0.1:1", cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10001:asset=456",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "network error must use the standard NetworkError mapping (exit \
             1); stderr={stderr}"
        );
        assert!(stderr.contains("Could not reach"), "stderr={stderr}");
    }
}

// ---------------------------------------------------------------------------
// AC-012 (BC-3.4.021 amended Postconditions — json item 3 scope note,
// VP-578-024) — dry-run `plannedChanges` per-hint-kind composed wire shape.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_id_hint_preview_shape() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10286",
        field_descriptor("Blocker", "option", Some(serde_json::json!([]))),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10286:id=10286",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-012 :id dry-run preview; stderr={stderr} stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["dryRun"].as_bool(), Some(true), "stdout={stdout}");
    assert_eq!(
        parsed["plannedChanges"]["customfield_10286"],
        serde_json::json!({"id": "10286"}),
        "AC-012: :id preview must be the composed wire OBJECT, not a display \
         string; stdout={stdout}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_name_hint_preview_shape() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10999",
        field_descriptor("Named Field", "string", None),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10999:name=Medium",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-012 :name dry-run preview; stderr={stderr} stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        parsed["plannedChanges"]["customfield_10999"],
        serde_json::json!({"name": "Medium"}),
        "stdout={stdout}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_option_hint_non_cascading_preview_shape() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10176",
        field_descriptor(
            "Urgency",
            "option",
            Some(serde_json::json!([{"id": "10286", "value": "High"}])),
        ),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10176:option=High",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-012 :option non-cascading dry-run preview; stderr={stderr} \
         stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        parsed["plannedChanges"]["customfield_10176"],
        serde_json::json!({"id": "10286"}),
        "stdout={stdout}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_option_hint_cascading_preview_shape() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_20002",
        field_descriptor(
            "Cascade Field",
            "option-with-child",
            Some(serde_json::json!([
                {
                    "id": "1", "value": "Parent",
                    "children": [{"id": "2", "value": "Child"}]
                }
            ])),
        ),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_20002:option=Parent>Child",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-012 :option cascading dry-run preview; stderr={stderr} \
         stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        parsed["plannedChanges"]["customfield_20002"],
        serde_json::json!({"value": "Parent", "child": {"value": "Child"}}),
        "stdout={stdout}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_asset_hint_preview_shape() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    write_workspace_cache_file(cache_dir.path(), "default", "ws-1");
    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10001",
        field_descriptor("Asset Field", "any", None),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10001:asset=456",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-012 :asset dry-run preview; stderr={stderr} stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        parsed["plannedChanges"]["customfield_10001"],
        serde_json::json!([{"workspaceId": "ws-1", "id": "ws-1:456", "objectId": "456"}]),
        "AC-012: :asset preview must be the composed wire ARRAY, not the \
         simplified '<ws>:<oid>' composite string; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-013 (BC-3.4.030 "Dry-run preview shape and side effect" Postconditions,
// VP-578-024) — `:asset` cold-cache side effect reachable under `--dry-run`.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_dry_run_asset_cold_cache_exits_64_before_preview() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_10001",
        field_descriptor("Asset Field", "any", None),
    )
    .await;
    {
        let _guard = Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/assets/workspace"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "errorMessages": ["nope"], "errors": {}
            })))
            .mount_as_scoped(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/rest/api/3/issue/TEST-1"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "--output",
                "json",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10001:asset=456",
                "--dry-run",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            output.status.code(),
            Some(64),
            "cold-cache workspace-discovery failure must exit 64 even under \
             --dry-run; stderr={stderr}"
        );
        assert!(
            stderr.contains("Assets is not available on this Jira site"),
            "stderr={stderr}"
        );
        assert!(
            !stdout.contains("plannedChanges"),
            "no plannedChanges output may be emitted before this exit-64; \
             stdout={stdout}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-014 (BC-3.4.031, regression pass at the edit call site) — EC-6/7/8/9.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ec6_ec7_ec8_ec9_regression_at_edit_call_site() {
    // EC-6: colon INSIDE VALUE (after '='), not misparsed as a nested hint.
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        mount_editmeta_one(
            &server,
            "TEST-1",
            "customfield_10176",
            field_descriptor(
                "Urgency",
                "option",
                Some(serde_json::json!([{"id": "77", "value": "High:Priority"}])),
            ),
        )
        .await;
        mount_put_204_with_body(
            &server,
            "TEST-1",
            serde_json::json!({"fields": {"customfield_10176": {"id": "77"}}}),
            1,
        )
        .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10176:option=High:Priority",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "EC-6: colon in VALUE must resolve normally, not be reinterpreted \
             as a nested hint; stderr={stderr}"
        );
    }

    // EC-7: multiple ':' in NAME before '=', no valid kind at the end — this
    // fires inside parse_field_kv itself (S-578-1, already implemented) and
    // never reaches this call site's own dispatch logic or the interim
    // guard. Expected to PASS already (see module doc comment).
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "Region: EMEA:bogus=X",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "stderr={stderr}");
        assert!(
            stderr.contains("unknown field-value kind"),
            "EC-7: stderr={stderr}"
        );
    }

    // EC-8: `:id=` empty VALUE passes through as {"id": ""}, server-validated,
    // NOT a client-side exit-64. Proven by observing the PUT actually fires
    // with that exact body (robust to whatever exit code jr maps a 400 to).
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        mount_editmeta_one(
            &server,
            "TEST-1",
            "customfield_10001",
            field_descriptor("Str Field", "string", None),
        )
        .await;
        // `.expect(1)` makes this a genuine wire-body assertion, not just a
        // "some PUT happened" check: `body_partial_json` is a wiremock
        // REQUEST MATCHER — this mock (and hence its 400 response) only
        // fires when the PUT body actually contains
        // `{"fields":{"customfield_10001":{"id":""}}}`. If the composer
        // sent a different shape, wiremock would 404 the request as
        // unmatched, this mock's count would stay at 0, and `MockServer`'s
        // Drop-time expectation check would panic — the SAME idiom
        // `mount_put_204_with_body` already uses elsewhere in this file.
        Mock::given(method("PUT"))
            .and(path("/rest/api/3/issue/TEST-1"))
            .and(body_partial_json(
                serde_json::json!({"fields": {"customfield_10001": {"id": ""}}}),
            ))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "errorMessages": ["id is required"], "errors": {}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let _output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10001:id=",
            ])
            .output()
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        assert!(
            requests.iter().any(|r| r.method.as_str() == "PUT"),
            "EC-8: :id= must pass VALUE through verbatim to the server (a PUT \
             must be attempted), not be pre-validated client-side"
        );
    }

    // EC-9: `:name=` empty VALUE, same posture as EC-8.
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        mount_editmeta_one(
            &server,
            "TEST-1",
            "customfield_10001",
            field_descriptor("Str Field", "string", None),
        )
        .await;
        // See EC-8's comment above: `.expect(1)` on this `body_partial_json`
        // mock turns "a PUT happened" into "the PUT body was exactly
        // `{"fields":{"customfield_10001":{"name":""}}}`" — a mismatched
        // body would leave this mock unmatched (0 calls) and fail at
        // `MockServer` drop.
        Mock::given(method("PUT"))
            .and(path("/rest/api/3/issue/TEST-1"))
            .and(body_partial_json(
                serde_json::json!({"fields": {"customfield_10001": {"name": ""}}}),
            ))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "errorMessages": ["name is required"], "errors": {}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let _output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10001:name=",
            ])
            .output()
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        assert!(
            requests.iter().any(|r| r.method.as_str() == "PUT"),
            "EC-9: :name= must pass VALUE through verbatim to the server (a \
             PUT must be attempted), not be pre-validated client-side"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-016 (BC-3.4.015 Invariant 10, EC-3.4.015-18, regression pin at this call
// site) — `--field` + `--dry-run` combined: resolution runs INSIDE the
// dry-run block, for a HINTED pair too.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_015_dry_run_hinted_field_resolution_runs_inside_dry_run_block() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // `.expect(1)`: editmeta MUST be fetched even though this is a dry run —
    // proving resolution runs inside the dry-run block, not skipped.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_10286": field_descriptor("Blocker", "option", Some(serde_json::json!([])))
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_10286:id=10286",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-016: hinted --field --dry-run must succeed and resolve inside \
         the dry-run block; stderr={stderr} stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["dryRun"].as_bool(), Some(true), "stdout={stdout}");
    assert_eq!(
        parsed["plannedChanges"]["customfield_10286"],
        serde_json::json!({"id": "10286"}),
        "stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-017 (BC-3.4.027/028/029/030 Postconditions "changed_fields echo") —
// echo conventions preserved for every hint kind.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_changed_fields_echo_per_hint_kind() {
    // :option non-cascading -> matched label.
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        mount_editmeta_one(
            &server,
            "TEST-1",
            "customfield_10176",
            field_descriptor(
                "Urgency",
                "option",
                Some(serde_json::json!([{"id": "10286", "value": "High"}])),
            ),
        )
        .await;
        mount_put_204(&server, "TEST-1").await;

        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "--output",
                "json",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10176:option=High",
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "stderr={stderr}");
        let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(
            parsed["changed_fields"]["customfield_10176"].as_str(),
            Some("High"),
            "stdout={stdout}"
        );
    }

    // :option cascading -> "<parent> > <child>".
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        mount_editmeta_one(
            &server,
            "TEST-1",
            "customfield_20002",
            field_descriptor(
                "Cascade Field",
                "option-with-child",
                Some(serde_json::json!([
                    {
                        "id": "1", "value": "Parent",
                        "children": [{"id": "2", "value": "Child"}]
                    }
                ])),
            ),
        )
        .await;
        mount_put_204(&server, "TEST-1").await;

        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "--output",
                "json",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_20002:option=Parent>Child",
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "stderr={stderr}");
        let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(
            parsed["changed_fields"]["customfield_20002"].as_str(),
            Some("Parent > Child"),
            "stdout={stdout}"
        );
    }

    // :id -> raw literal.
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        mount_editmeta_one(
            &server,
            "TEST-1",
            "customfield_10286",
            field_descriptor("Blocker", "option", Some(serde_json::json!([]))),
        )
        .await;
        mount_put_204(&server, "TEST-1").await;

        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "--output",
                "json",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10286:id=10286",
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "stderr={stderr}");
        let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(
            parsed["changed_fields"]["customfield_10286"].as_str(),
            Some("10286"),
            "stdout={stdout}"
        );
    }

    // :name -> VALUE verbatim.
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        mount_editmeta_one(
            &server,
            "TEST-1",
            "customfield_10999",
            field_descriptor("Named Field", "string", None),
        )
        .await;
        mount_put_204(&server, "TEST-1").await;

        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "--output",
                "json",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10999:name=Medium",
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "stderr={stderr}");
        let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(
            parsed["changed_fields"]["customfield_10999"].as_str(),
            Some("Medium"),
            "stdout={stdout}"
        );
    }

    // :asset -> "<workspaceId>:<objectId>" composite string.
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        write_workspace_cache_file(cache_dir.path(), "default", "ws-1");
        mount_editmeta_one(
            &server,
            "TEST-1",
            "customfield_10001",
            field_descriptor("Asset Field", "any", None),
        )
        .await;
        mount_put_204(&server, "TEST-1").await;

        let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "--output",
                "json",
                "issue",
                "edit",
                "TEST-1",
                "--field",
                "customfield_10001:asset=456",
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "stderr={stderr}");
        let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(
            parsed["changed_fields"]["customfield_10001"].as_str(),
            Some("ws-1:456"),
            "stdout={stdout}"
        );
    }
}

// ===========================================================================
// Adversarial fix-burst (adv-p1) — S-578-2
//
// GROUP A: AC-019 / EC-3.4.027-1 entry-point `:option` type gate. This gate
// is NOT implemented today — `compose_option_hint` (field_resolve.rs) never
// inspects `meta_field.schema.field_type` at all; it goes straight to
// `allowedValues`. These three tests are the Red Gate for that gap and MUST
// fail against the current implementation (on ASSERTIONS, not a build error
// or panic).
//
// GROUP B: coverage backfill for already-implemented behavior (P1-002
// cascading error branches, P1-004 multi-:asset single-GET, P1-007
// second-`>` cascading). These should PASS immediately — a red result here
// is a real defect in `compose_option_hint`/`compose_asset_hint`, not a
// missing gate, and must be reported rather than papered over.
// ===========================================================================

// ---------------------------------------------------------------------------
// GROUP A — AC-019 (BC-3.4.027 EC-3.4.027-1, Invariant 7)
// ---------------------------------------------------------------------------

/// EC-3.4.027-1 sub-case (a): `schema.type == "array"` (a real Jira
/// multi-select shape carrying `allowedValues`) under `:option` MUST reuse
/// EC-3.4.015-5's exact "unsupported type" message/exit behavior — the SAME
/// code path BC-3.4.015 Step 4 already uses for this schema.type, not a
/// re-derived one. Load-bearing: the literal `schema.type` string ("array").
///
/// RED today: `compose_option_hint` never inspects `schema.type` — it
/// proceeds straight to `allowedValues` matching, so `High` resolves
/// successfully against the mounted `allowedValues` and the edit SUCCEEDS
/// (wrongly) instead of exiting 64 with the reused EC-3.4.015-5 message.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_027_ec1_array_type_reuses_ec_3_4_015_5_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // A real Jira "array" (multi-select) shape: schema.type == "array" but
    // still carrying allowedValues — exactly the shape EC-3.4.027-1
    // sub-case (a) targets (distinguishing it from EC-3.4.016's "no
    // configured option values" case, which has an EMPTY/absent
    // allowedValues).
    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_40001",
        field_descriptor(
            "Multi Select",
            "array",
            Some(serde_json::json!([{"id": "1", "value": "High"}])),
        ),
    )
    .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_40001:option=High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-019 (a): :option on an array-typed field must exit 64 via the \
         entry-point gate, reusing EC-3.4.015-5's behavior; stderr={stderr}"
    );
    assert!(
        stderr.contains("array"),
        "AC-019 (a): load-bearing literal schema.type string 'array' \
         missing (must reuse EC-3.4.015-5's exact message); stderr={stderr}"
    );
    assert!(
        stderr.contains("not supported by"),
        "AC-019 (a): must reuse EC-3.4.015-5's exact 'not supported by' \
         wording, not a re-derived message; stderr={stderr}"
    );

    let requests = server.received_requests().await.unwrap();
    assert!(
        !requests
            .iter()
            .any(|r| r.method == wiremock::http::Method::PUT),
        "AC-019 (a): no PUT may be issued once the entry-point gate fires; \
         requests={requests:?}"
    );
}

/// EC-3.4.027-1 sub-case (b): `schema.type` is a bare-form-SUPPORTED scalar
/// type (`string`/`number`/`date`/`datetime`/`user`) under `:option` must
/// exit 64 with a DISTINCT message from EC-3.4.015-5's "unsupported type"
/// wording — the field itself IS settable, just not via `:option`.
/// Load-bearing substrings: "is not an option field" + the resolved
/// `schema.type` string.
///
/// RED today: the entry-point gate does not exist, so `compose_option_hint`
/// falls through to its `allowedValues.is_empty()` check (a `string`-typed
/// field has no `allowedValues`) and emits BC-3.4.016's PRE-EXISTING "no
/// configured option values" message instead — same exit code (64) but the
/// WRONG message, which is exactly the conflation AC-019's "Non-goal"
/// paragraph forbids.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_027_ec1_scalar_type_distinct_is_not_an_option_field_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_40002",
        field_descriptor("Plain Text Field", "string", None),
    )
    .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_40002:option=SomeValue",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-019 (b): :option on a string-typed field must exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("is not an option field"),
        "AC-019 (b): load-bearing substring 'is not an option field' \
         missing; stderr={stderr}"
    );
    assert!(
        stderr.contains("string"),
        "AC-019 (b): the resolved schema.type string ('string') must be \
         named in the message; stderr={stderr}"
    );
    assert!(
        !stderr.contains("no configured option values"),
        "AC-019 (b): must NOT reuse BC-3.4.016's 'no configured option \
         values' message (that is a DIFFERENT case — EC-3.4.016's own, \
         unaffected by this gate); stderr={stderr}"
    );
}

/// The entry-point gate MUST run BEFORE any `allowedValues`/`children`
/// inspection — i.e. a non-option field with EMPTY/absent `allowedValues`
/// still gets THIS gate's "is not an option field" message, never
/// BC-3.4.016's "no configured option values" message (which presupposes
/// the field already passed the type gate and is scoped to
/// option/option-with-child fields only, per AC-019's "Non-goal"
/// paragraph).
///
/// RED today: identical failure mode to sub-case (b) above — proves the
/// ORDERING claim, not just the message-content claim, using a distinct
/// scalar type ("number") so this test is not a verbatim duplicate.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_027_ec1_gate_runs_before_allowed_values_children_inspection() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // `allowedValues: null` — a non-option field's editmeta never carries
    // allowedValues at all. If the gate did NOT run first, the composer
    // would fall through straight to the `allowed.is_empty()` check and
    // emit the WRONG ("no configured option values") message.
    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_40003",
        field_descriptor("Story Points", "number", None),
    )
    .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_40003:option=5",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(64), "stderr={stderr}");
    assert!(
        stderr.contains("is not an option field"),
        "AC-019 ordering: the entry-point gate must fire BEFORE the \
         allowedValues-empty check; stderr={stderr}"
    );
    assert!(
        stderr.contains("number"),
        "AC-019 ordering: resolved schema.type ('number') must be named; \
         stderr={stderr}"
    );
    assert!(
        !stderr.contains("no configured option values"),
        "AC-019 ordering: must not fall through to the allowedValues-empty \
         message; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// GROUP B — P1-002 coverage backfill: cascading error branches
// (EC-3.4.027-2/3/6). Already-implemented behavior; should PASS.
// ---------------------------------------------------------------------------

/// Two parents, each with their own children — used to prove that an error
/// message enumerates the CORRECT candidate set (e.g. one parent's children,
/// never the sibling parent's) rather than merely being non-empty.
fn cascading_field_with_two_parents() -> serde_json::Value {
    serde_json::json!([
        {
            "id": "1", "value": "Parent",
            "children": [
                {"id": "2", "value": "ChildA"},
                {"id": "3", "value": "ChildB"}
            ]
        },
        {
            "id": "4", "value": "OtherParent",
            "children": [{"id": "5", "value": "OtherChild"}]
        }
    ])
}

/// EC-3.4.027-6 (empty parent, `>Child`): same exit-64 shape as
/// EC-3.4.027-2 (unresolvable parent) — an empty parent segment can never
/// match a real `allowedValues[].value` entry. This check MUST run before
/// the D4 `children.is_empty()` collision path (AC-004) — it fires on the
/// parent segment alone, before any parent-entry lookup happens.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_027_ec6_empty_parent_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_50001",
        field_descriptor(
            "Cascade",
            "option-with-child",
            Some(cascading_field_with_two_parents()),
        ),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_50001:option=>Child",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(64), "stderr={stderr}");
    assert!(
        stderr.contains("Option value ''"),
        "EC-3.4.027-6 (empty parent): must report the empty value; \
         stderr={stderr}"
    );
    assert!(
        stderr.contains("Parent") && stderr.contains("OtherParent"),
        "EC-3.4.027-6 (empty parent): must list the top-level allowed \
         PARENT values; stderr={stderr}"
    );
    assert!(
        !stderr.contains("under parent"),
        "EC-3.4.027-6 (empty parent) must use the plain 'not found ... \
         Allowed values' shape — same as EC-3.4.027-2/3's own \
         `find_option_match`-produced messages, neither of which ever \
         names the parent inline via an 'under parent NAME' clause (PR #741 \
         review: the empty-CHILD case previously had a bespoke 'under \
         parent' variant that has since been corrected to match this same \
         shape — see `test_bc_3_4_027_ec6_empty_child_exits_64`); \
         stderr={stderr}"
    );
}

/// EC-3.4.027-6 (empty child, `Parent>`): same exit-64 shape as
/// EC-3.4.027-3 (resolvable parent, unresolvable child) — checked AFTER
/// parent resolution succeeds but BEFORE the D4 `children.is_empty()`
/// collision check (AC-004's own precondition is a NON-empty child
/// segment), per BC-3.4.027's documented ordering. Per BC-3.4.027 (bc-3-
/// issue-write.md EC-3.4.027-6): "falls through to the SAME ... shape as
/// EC-3.4.027-2/3 ... rather than introducing a distinct empty-segment
/// error message" — this test therefore asserts the message is
/// byte-shape-identical to `find_option_match`'s own "not found" error
/// (`"Option value '{value}' not found for field '{human_name}'. Allowed
/// values: {…}."`), NOT a bespoke "under parent NAME" / "Allowed child
/// values" variant (PR #741 review, S-578-2 fix-burst — corrected from an
/// earlier revision that introduced exactly that distinct shape).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_027_ec6_empty_child_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_50002",
        field_descriptor(
            "Cascade",
            "option-with-child",
            Some(cascading_field_with_two_parents()),
        ),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_50002:option=Parent>",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(64), "stderr={stderr}");
    assert!(
        stderr.contains("Option value ''"),
        "EC-3.4.027-6 (empty child): must report the empty value; \
         stderr={stderr}"
    );
    assert!(
        stderr.contains("not found for field") && stderr.contains("Allowed values:"),
        "EC-3.4.027-6 (empty child): must reuse EC-3.4.027-3's exact \
         `find_option_match`-shaped 'not found ... Allowed values' wording \
         — not a distinct empty-segment message; stderr={stderr}"
    );
    assert!(
        !stderr.contains("under parent") && !stderr.contains("Allowed child values"),
        "EC-3.4.027-6 (empty child): must NOT introduce a distinct \
         empty-segment shape (an 'under parent NAME' clause or an 'Allowed \
         child values' relabel) per BC-3.4.027's explicit anti-drift \
         instruction; stderr={stderr}"
    );
    assert!(
        stderr.contains("ChildA") && stderr.contains("ChildB"),
        "EC-3.4.027-6 (empty child): must list THAT PARENT's ('Parent', the \
         one resolved from the 'Parent>' segment) allowed CHILD values — \
         the correct-scoping proof stands in for literally naming the \
         parent, since EC-3.4.027-3's own shape never does either; \
         stderr={stderr}"
    );
    assert!(
        !stderr.contains("OtherChild"),
        "EC-3.4.027-6 (empty child): must not leak the OTHER parent's \
         children; stderr={stderr}"
    );
}

/// EC-3.4.027-2: unresolvable parent segment → exit 64 listing allowed
/// parent values.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_027_ec2_unresolvable_parent_lists_allowed_values() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_50003",
        field_descriptor(
            "Cascade",
            "option-with-child",
            Some(cascading_field_with_two_parents()),
        ),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_50003:option=NoSuchParent>X",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(64), "stderr={stderr}");
    assert!(
        stderr.contains("NoSuchParent"),
        "EC-3.4.027-2: must echo the unresolvable parent value; \
         stderr={stderr}"
    );
    assert!(
        stderr.contains("Parent") && stderr.contains("OtherParent"),
        "EC-3.4.027-2: must list the allowed top-level PARENT values; \
         stderr={stderr}"
    );
}

/// EC-3.4.027-3: resolvable parent, unresolvable child segment → exit 64
/// listing THAT PARENT's allowed child values (never the sibling parent's).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_027_ec3_resolvable_parent_unresolvable_child_lists_child_values() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_50004",
        field_descriptor(
            "Cascade",
            "option-with-child",
            Some(cascading_field_with_two_parents()),
        ),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_50004:option=Parent>NoSuchChild",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(64), "stderr={stderr}");
    assert!(
        stderr.contains("NoSuchChild"),
        "EC-3.4.027-3: must echo the unresolvable child value; \
         stderr={stderr}"
    );
    assert!(
        stderr.contains("ChildA") && stderr.contains("ChildB"),
        "EC-3.4.027-3: must list the RESOLVED parent's allowed child \
         values; stderr={stderr}"
    );
    assert!(
        !stderr.contains("OtherChild"),
        "EC-3.4.027-3: must not leak the OTHER parent's children; \
         stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// GROUP B — P1-004 coverage backfill: `:asset` workspace-id resolution is
// called AT MOST ONCE per invocation regardless of how many `:asset` hints
// are present (AC-008), on a COLD cache. Already-implemented behavior (via
// the disk-cache write-then-read round trip inside
// `get_or_fetch_workspace_id`); should PASS. If it fails, that is a real
// defect (per the adversary's cache-write-failure caveat) — do NOT loosen
// this assertion to force green; report the finding instead.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_030_two_bare_asset_hints_single_workspace_get() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // COLD cache: no workspace.json written ahead of time. Two DISTINCT
    // customfield_NNNNN literals so both bypass list_fields (Step 1) and
    // both reach the `:asset` composer within the SAME `resolve_edit_fields`
    // invocation.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1/editmeta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": {
                "customfield_60001": {
                    "name": "Asset A",
                    "schema": { "type": "any", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                },
                "customfield_60002": {
                    "name": "Asset B",
                    "schema": { "type": "any", "system": null, "custom": null },
                    "operations": ["set"],
                    "required": false,
                    "allowedValues": null
                }
            }
        })))
        .mount(&server)
        .await;
    mount_workspace_ok(&server, "ws-cold").await;
    mount_put_204(&server, "TEST-1").await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_60001:asset=111",
            "--field",
            "customfield_60002:asset=222",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "P1-004: two bare :asset hints on a cold cache must both resolve; \
         stderr={stderr} stdout={stdout}"
    );

    let requests = server.received_requests().await.unwrap();
    let workspace_gets: Vec<_> = requests
        .iter()
        .filter(|r| {
            r.method == wiremock::http::Method::GET
                && r.url.path() == "/rest/servicedeskapi/assets/workspace"
        })
        .collect();
    assert_eq!(
        workspace_gets.len(),
        1,
        "P1-004 (AC-008): get_or_fetch_workspace_id must be called AT MOST \
         ONCE per invocation regardless of how many :asset hints are \
         present — got {} workspace-discovery GET(s); requests={requests:?}",
        workspace_gets.len()
    );
}

// ---------------------------------------------------------------------------
// GROUP B — P1-007 coverage backfill: a second `>` in a cascading VALUE —
// everything after the FIRST `>` (via `str::split_once('>')`) is the
// verbatim child, including any further `>` characters. Already-implemented
// behavior; should PASS.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_027_cascading_second_greater_than_is_verbatim_child() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    mount_editmeta_one(
        &server,
        "TEST-1",
        "customfield_70001",
        field_descriptor(
            "Cascade",
            "option-with-child",
            Some(serde_json::json!([
                {
                    "id": "1", "value": "Parent",
                    "children": [{"id": "2", "value": "Child>Trailing"}]
                }
            ])),
        ),
    )
    .await;
    mount_put_204_with_body(
        &server,
        "TEST-1",
        serde_json::json!({
            "fields": {
                "customfield_70001": {"value": "Parent", "child": {"value": "Child>Trailing"}}
            }
        }),
        1,
    )
    .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "TEST-1",
            "--field",
            "customfield_70001:option=Parent>Child>Trailing",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "P1-007: split_once('>') must produce parent='Parent', \
         child='Child>Trailing' (everything after the FIRST '>'); \
         stderr={stderr} stdout={stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        parsed["changed_fields"]["customfield_70001"].as_str(),
        Some("Parent > Child>Trailing"),
        "P1-007: changed_fields echo must be '<parent> > <child>' with the \
         verbatim second '>' preserved in the child; stdout={stdout}"
    );
}

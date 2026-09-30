//! End-to-end table/JSON sanitization asymmetry (BC-7.1.006, FIX-P5-001,
//! SEC-001-RENDER-TABLE-ANSI-SANITIZE, CWE-150/CWE-116).
//!
//! VP-SEC-001-001(c): a real command, driven through a hostile wiremock
//! fixture, must never leak a raw ESC byte or a raw C1 code point
//! (`U+0080`-`U+009F`) into `--output table` (default) stdout, while
//! `--output json` must round-trip the identical hostile payload lossless
//! (`sanitize_table_cell` is never invoked on the JSON path — mirrors
//! `sanitize_env_display`'s own documented rule and the issue #398
//! description-echo asymmetry). Note: JSON's own grammar mandates escaping
//! ESC (`0x1B`) as `\u001b`, so the JSON-mode tests below check round-trip
//! value equality (decode back to the exact same `char`s) plus the raw,
//! unescaped survival of the C1 byte `U+009B` (which JSON does NOT mandate
//! escaping) — not literal raw-ESC-byte containment in the serialized
//! text, which no valid JSON string could ever satisfy.
//!
//! `output::sanitize_table_cell` is implemented and wired into
//! `render_table` (and its styled sibling `render_table_with_styles`) —
//! every table-mode assertion below exercises and pins that production
//! behavior. The `--output json` assertions pin a pre-existing, unrelated
//! guarantee (see each test's doc comment): the JSON path was already
//! raw/lossless before this fix and must stay that way.
//!
//! Hermeticity per `tests/common/hermetic.rs` (S-cycle14 STORY-A
//! convention): every command scrubs ambient `JR_*` env vars before
//! re-injecting only the ones this test itself sets, and uses per-test
//! isolated `JR_CACHE_DIR`/`JR_CONFIG_DIR`/cwd temp directories.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use common::hermetic;
use serde_json::{Value, json};
use tempfile::TempDir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The hostile payload used across both commands below: an ANSI CSI color
/// sequence, its reset, and a C1 CSI introducer (`U+009B`) immediately
/// followed by survivor text — exercising both the CSI-consumption state
/// machine and the C1-single-code-point-removal path in one string (mirrors
/// `src/output.rs`'s own EC-1/EC-5-flavored unit-test pin).
const HOSTILE_PAYLOAD: &str = "\u{1b}[31mFAKE\u{1b}[0m\u{9b}pwned";

/// Asserts `stdout` contains neither a raw ESC byte (`U+001B`) nor any
/// character in the C1 control range `U+0080`-`U+009F` — the exact
/// table-mode guarantee VP-SEC-001-001(c) requires. Decoding via
/// `String::from_utf8_lossy` (not a raw byte scan) so a multi-byte UTF-8
/// encoding of a C1 code point is caught via its decoded `char`, not missed
/// by a byte-level substring search.
fn assert_no_esc_or_c1(stdout: &str, context: &str) {
    assert!(
        !stdout.contains('\u{1b}'),
        "{context}: raw ESC byte must not survive in --output table stdout: {stdout:?}"
    );
    assert!(
        !stdout.chars().any(|c| (0x80..=0x9F).contains(&(c as u32))),
        "{context}: raw C1 code point must not survive in --output table stdout: {stdout:?}"
    );
}

/// Isolated wiremock + cache/config/cwd harness, following the
/// `tests/user_commands.rs` / `tests/field_options.rs` precedent.
struct Harness {
    server: MockServer,
    cache: TempDir,
    config: TempDir,
    cwd: TempDir,
}

impl Harness {
    async fn new() -> Self {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        hermetic::assert_no_ancestor_jr_toml(cwd.path());
        Self {
            server,
            cache,
            config,
            cwd,
        }
    }

    /// Runs `jr <args>`, scrubbing every ambient `JR_*` env var first
    /// (`hermetic::scrub_ambient_jr_env`) so a developer/CI environment's
    /// own `JR_*` settings can't leak into the subprocess, then re-injecting
    /// only this harness's own isolated overrides.
    fn run(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::cargo_bin("jr").unwrap();
        hermetic::scrub_ambient_jr_env(
            &mut cmd,
            &[
                "JR_BASE_URL",
                "JR_AUTH_HEADER",
                "JR_CACHE_DIR",
                "JR_CONFIG_DIR",
            ],
        );
        cmd.env("JR_BASE_URL", self.server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("JR_CACHE_DIR", self.cache.path().join("jr"))
            .env("JR_CONFIG_DIR", self.config.path().join("jr"))
            .args(args)
            .current_dir(self.cwd.path());
        cmd.output().unwrap()
    }
}

/// Mounts the two createmeta calls `jr field options --type` needs (S-331
/// issue-type resolution + S-580-1 createmeta-fields enumeration), with a
/// single `customfield_10084` option whose label is `label`.
async fn mount_field_options_fixture(server: &MockServer, project: &str, label: &str) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/api/3/issue/createmeta/{project}/issuetypes"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issueTypes": [{"id": "10000", "name": "Bug"}],
            "startAt": 0,
            "maxResults": 200,
            "total": 1
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/api/3/issue/createmeta/{project}/issuetypes/10000"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": [{
                "fieldId": "customfield_10084",
                "name": "SOC Client",
                "schema": {
                    "type": "option",
                    "custom": "com.atlassian.jira.plugin.system.customfieldtypes:select",
                    "system": null
                },
                "allowedValues": [
                    {"id": "10001", "value": label, "name": null}
                ]
            }],
            "startAt": 0,
            "maxResults": 200,
            "total": 1
        })))
        .mount(server)
        .await;
}

/// Mounts `POST /rest/api/3/search/jql` returning one issue whose summary
/// is `summary`.
async fn mount_issue_list_fixture(server: &MockServer, key: &str, summary: &str) {
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                key, summary, "To Do",
            )]),
        ))
        .mount(server)
        .await;
}

/// Mounts `GET /rest/api/3/user/assignable/multiProjectSearch` returning
/// one user whose display name is `display_name` — the fixture `jr user
/// list` (`src/cli/user.rs::handle_list`) hits without `--all`.
async fn mount_user_list_fixture(server: &MockServer, project: &str, display_name: &str) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", project))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "accountId": "acc-hostile-1",
            "displayName": display_name,
            "emailAddress": "user@example.invalid",
            "active": true,
        }])))
        .mount(server)
        .await;
}

// ═══════════════════════════════════════════════════════════════════════
// `jr field options` — VP-SEC-001-001(c)
// ═══════════════════════════════════════════════════════════════════════

/// Table mode (default output): a hostile option label must not leak a raw
/// ESC byte or C1 code point into stdout.
///
/// Pins production behavior: `sanitize_table_cell` is wired into
/// `render_table`, so the hostile payload must not survive in the "Label"
/// column.
#[tokio::test]
async fn test_bc_7_1_006_field_options_table_mode_strips_hostile_option_label() {
    let h = Harness::new().await;
    mount_field_options_fixture(&h.server, "HELP", HOSTILE_PAYLOAD).await;

    let output = h.run(&[
        "field",
        "options",
        "customfield_10084",
        "--type",
        "Bug",
        "--project",
        "HELP",
        "--no-input",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert_no_esc_or_c1(&stdout, "jr field options (table mode)");
}

/// JSON mode: the identical hostile fixture must round-trip lossless —
/// `sanitize_table_cell` is never invoked on the `--output json` path.
///
/// Note on "byte-for-byte": JSON's own grammar mandates escaping `0x1B`
/// (ESC, within the `0x00`-`0x1F` mandatory-escape range) as the 6-char
/// sequence `\u001b` — no valid JSON string can contain a literal raw ESC
/// byte, sanitized or not, so a literal-byte containment check on the ESC
/// portion of the payload would fail even against a CORRECT
/// implementation. The C1 code point `U+009B`, by contrast, is NOT in
/// JSON's mandatory-escape set, so it DOES survive as a literal raw byte
/// in the serialized text when (and only when) nothing has sanitized it —
/// asserted directly below as the strongest available "untouched" signal,
/// alongside full round-trip equality of the decoded value (which does
/// cover the ESC portion, since `serde_json::from_str` decodes `\u001b`
/// back to the same `char` that was serialized).
///
/// Expected GREEN today: the JSON path was already raw/lossless before this
/// fix (`output::render_json` never called `sanitize_table_cell`, which
/// doesn't exist in any call graph yet), so this pins a pre-existing
/// invariant rather than new behavior — included for completeness of the
/// table/JSON asymmetry proof (VP-SEC-001-001(c) requires both sides be
/// checked against the same fixture).
#[tokio::test]
async fn test_bc_7_1_006_field_options_json_mode_preserves_hostile_option_label_raw() {
    let h = Harness::new().await;
    mount_field_options_fixture(&h.server, "HELP", HOSTILE_PAYLOAD).await;

    let output = h.run(&[
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains('\u{9b}'),
        "--output json must carry the hostile label's raw C1 byte literally \
         (JSON does not mandate escaping it, and sanitize_table_cell must \
         never run on the JSON path): {stdout:?}"
    );

    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("expected valid JSON, got {stdout}\nerror: {e}"));
    let arr = parsed.as_array().expect("expected a JSON array");
    assert_eq!(
        arr[0]["label"],
        json!(HOSTILE_PAYLOAD),
        "decoded JSON value must round-trip the hostile label exactly \
         unchanged, including its ESC portion"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// `jr issue list` — VP-SEC-001-001(c)
// ═══════════════════════════════════════════════════════════════════════

/// Table mode (default output): a hostile issue summary must not leak a raw
/// ESC byte or C1 code point into stdout.
///
/// Pins production behavior: `sanitize_table_cell` is wired into
/// `render_table`, so the hostile payload must not survive in the "Summary"
/// column.
#[tokio::test]
async fn test_bc_7_1_006_issue_list_table_mode_strips_hostile_summary() {
    let h = Harness::new().await;
    mount_issue_list_fixture(&h.server, "FOO-1", HOSTILE_PAYLOAD).await;

    let output = h.run(&["issue", "list", "--jql", "project = FOO", "--no-input"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert_no_esc_or_c1(&stdout, "jr issue list (table mode)");
}

/// JSON mode: the identical hostile fixture must round-trip lossless.
///
/// See the field-options JSON test above for why this checks the C1 byte's
/// literal survival plus full round-trip equality, rather than raw-byte
/// containment of the whole payload (JSON's grammar mandates escaping ESC
/// `0x1B` as `\u001b`, so no valid JSON text can ever contain it as a
/// literal byte — sanitized or not).
///
/// Expected GREEN today — same rationale as the field-options JSON test
/// above: the JSON path is already lossless and untouched by this fix.
#[tokio::test]
async fn test_bc_7_1_006_issue_list_json_mode_preserves_hostile_summary_raw() {
    let h = Harness::new().await;
    mount_issue_list_fixture(&h.server, "FOO-1", HOSTILE_PAYLOAD).await;

    let output = h.run(&[
        "issue",
        "list",
        "--jql",
        "project = FOO",
        "--no-input",
        "--output",
        "json",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains('\u{9b}'),
        "--output json must carry the hostile summary's raw C1 byte \
         literally (JSON does not mandate escaping it, and \
         sanitize_table_cell must never run on the JSON path): {stdout:?}"
    );

    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("expected valid JSON, got {stdout}\nerror: {e}"));
    let arr = parsed.as_array().expect("expected a JSON array");
    assert_eq!(
        arr[0]["fields"]["summary"],
        json!(HOSTILE_PAYLOAD),
        "decoded JSON value must round-trip the hostile summary exactly \
         unchanged, including its ESC portion"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// `jr user list` — VP-SEC-001-001(c) (pr-review cycle-1 finding B-1)
// ═══════════════════════════════════════════════════════════════════════
//
// Unlike `field options`/`issue list` above, `jr user list` renders
// through the STYLED chokepoint (`output::render_table_with_styles` /
// `output::print_output_with_styles`, via `src/cli/user.rs::print_user_list`
// -> `format_user_row_styled`), not plain `render_table`/`print_output`.
// These two tests are the end-to-end proof that the styled path is wired
// to the same `sanitize_table_cell` chokepoint as every other table-mode
// command — B-1 flagged that nothing exercised this path at all.

/// Table mode (default output): a hostile server-supplied display name
/// must not leak a raw ESC byte or C1 code point into stdout.
///
/// Pins production behavior: `render_table_with_styles` sanitizes cell
/// TEXT exactly like plain `render_table` (the Active column's structural
/// color is applied separately, via `active_cell`, and never bypasses
/// this).
#[tokio::test]
async fn test_bc_7_1_006_user_list_table_mode_strips_hostile_display_name() {
    let h = Harness::new().await;
    mount_user_list_fixture(&h.server, "HELP", HOSTILE_PAYLOAD).await;

    let output = h.run(&["user", "list", "--project", "HELP", "--no-input"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert_no_esc_or_c1(&stdout, "jr user list (table mode)");
}

/// JSON mode: the identical hostile fixture must round-trip lossless —
/// `sanitize_table_cell` is never invoked on the `--output json` path,
/// including through the styled `print_output_with_styles` dispatch.
///
/// See the field-options JSON test above for why this checks the C1 byte's
/// literal survival plus full round-trip equality, rather than raw-byte
/// containment of the whole payload.
#[tokio::test]
async fn test_bc_7_1_006_user_list_json_mode_preserves_hostile_display_name_raw() {
    let h = Harness::new().await;
    mount_user_list_fixture(&h.server, "HELP", HOSTILE_PAYLOAD).await;

    let output = h.run(&[
        "user",
        "list",
        "--project",
        "HELP",
        "--no-input",
        "--output",
        "json",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains('\u{9b}'),
        "--output json must carry the hostile display name's raw C1 byte \
         literally (JSON does not mandate escaping it, and \
         sanitize_table_cell must never run on the JSON path): {stdout:?}"
    );

    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("expected valid JSON, got {stdout}\nerror: {e}"));
    let arr = parsed.as_array().expect("expected a JSON array");
    assert_eq!(
        arr[0]["displayName"],
        json!(HOSTILE_PAYLOAD),
        "decoded JSON value must round-trip the hostile display name \
         exactly unchanged, including its ESC portion"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// `jr issue comment view` — SEC-003 (FIX-P5-001 extension, D-393)
// ═══════════════════════════════════════════════════════════════════════
//
// `handle_comment_view`'s table-mode arm (`src/cli/issue/interactions.rs`
// ~L659-676) prints the comment `author`, `visibility`-derived `restricted`,
// and ADF-rendered `body_text` via bare `print!`/`println!` — NOT through
// `render_table`/`print_output`, so none of them are currently routed
// through `sanitize_table_cell`. SEC-003 extends BC-7.1.006's chokepoint
// guarantee to this handler's plain-text fields.

/// Hostile `author.displayName`: an ANSI CSI color sequence around
/// survivor text "Mallory", matching `HOSTILE_PAYLOAD`'s CSI-consumption
/// shape above but scoped to this section's own fixtures.
const COMMENT_HOSTILE_AUTHOR: &str = "\u{1b}[31mMallory\u{1b}[0m";

/// Hostile `visibility.value` — exercises `format_restricted_field`'s rung
/// (a) (`role`/`group` with non-empty `value`), which echoes `value`
/// verbatim into the printed "Restricted: " field. This is the "some other
/// printed field" server-controlled site beyond author/body.
const COMMENT_HOSTILE_VISIBILITY_VALUE: &str = "\u{1b}[35mEvilRole\u{1b}[0m";

/// ADF body: a single paragraph containing `"pwned"`, an OSC sequence, a
/// bare CR, a `hardBreak`, then a C1-byte-prefixed `"line2"`.
///
/// A raw `\n` inside one ADF `text` node is not how real ADF encodes a line
/// break (BC-7.2.011's file-wide invariant: no `text` node may contain a
/// raw `\n`) — multi-line content here comes from the `hardBreak` node
/// between the two `text` nodes instead, mirroring `adf_to_text`'s own
/// `"hardBreak" => self.output.push('\n')` rendering.
fn comment_hostile_body() -> Value {
    json!({
        "version": 1,
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [
                { "type": "text", "text": "pwned\u{1b}]0;evil\u{7}\r" },
                { "type": "hardBreak" },
                { "type": "text", "text": "\u{9b}line2" }
            ]
        }]
    })
}

/// Entirely clean, ASCII-only ADF body — the regression-guard pin's body
/// source.
fn comment_clean_body() -> Value {
    json!({
        "version": 1,
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{ "type": "text", "text": "hello world" }]
        }]
    })
}

/// Mounts `GET /rest/api/3/issue/{key}/comment/{id}` returning a comment
/// with a hostile `author.displayName`, `visibility.value`, and ADF `body`.
/// No `properties` key → "JSM internal: N/A" — a static fallback token
/// (`format_jsm_internal_field` returns `"N/A"` when `properties` is
/// absent), not server-controlled text, so it needs no hostile-payload
/// coverage here.
async fn mount_comment_view_hostile_fixture(server: &MockServer, key: &str, id: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}/comment/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": id,
            "author": { "displayName": COMMENT_HOSTILE_AUTHOR },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": comment_hostile_body(),
            "visibility": { "type": "role", "value": COMMENT_HOSTILE_VISIBILITY_VALUE }
        })))
        .mount(server)
        .await;
}

/// Mounts the same endpoint with an entirely clean, ASCII-only fixture — no
/// `properties` (→ "JSM internal: N/A") and no `visibility` (→
/// "Restricted: None") — the regression-guard pin's data source.
async fn mount_comment_view_clean_fixture(server: &MockServer, key: &str, id: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}/comment/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": id,
            "author": { "displayName": "Jane Smith" },
            "created": "2026-07-01T09:00:00.000+0000",
            "updated": "2026-07-01T10:30:00.000+0000",
            "body": comment_clean_body()
        })))
        .mount(server)
        .await;
}

/// Table mode (default output): hostile `author`/`visibility.value`/ADF
/// `body` fields must not leak a raw ESC byte, a raw C1 code point, or a
/// bare `\r` into stdout — while the CSI/OSC-stripped survivor text
/// (`Mallory`, and `pwned`/`line2` on two separate lines via the preserved
/// `hardBreak`-turned-`\n`) must still render, and the six field labels
/// must still all be present (layout unchanged).
///
/// RED against current code: `handle_comment_view`'s table-mode arm prints
/// `author`, `restricted`, and `body_text` via bare `print!`/`println!`,
/// never through `sanitize_table_cell` — the hostile ESC/C1/`\r` bytes
/// survive verbatim, so `assert_no_esc_or_c1` and the `\r`-absence
/// assertion both fail today.
#[tokio::test]
async fn test_bc_7_1_006_comment_view_human_output_strips_hostile_body_and_author() {
    let h = Harness::new().await;
    mount_comment_view_hostile_fixture(&h.server, "FOO-1", "10001").await;

    let output = h.run(&[
        "issue",
        "comment",
        "view",
        "FOO-1",
        "--id",
        "10001",
        "--no-input",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert_no_esc_or_c1(&stdout, "jr issue comment view (table mode)");
    assert!(
        !stdout.contains('\r'),
        "jr issue comment view (table mode): raw CR must not survive in \
         stdout: {stdout:?}"
    );

    assert!(
        stdout.contains("Author: Mallory"),
        "sanitized author must still render its CSI-stripped survivor \
         text: {stdout:?}"
    );

    let pwned_line = stdout
        .lines()
        .find(|l| l.contains("pwned"))
        .unwrap_or_else(|| panic!("expected a line containing 'pwned': {stdout:?}"));
    let line2_line = stdout
        .lines()
        .find(|l| l.contains("line2"))
        .unwrap_or_else(|| panic!("expected a line containing 'line2': {stdout:?}"));
    assert_ne!(
        pwned_line, line2_line,
        "the hardBreak-separated body text must still render on two \
         separate lines after sanitization: {stdout:?}"
    );

    for label in [
        "ID:",
        "Author:",
        "Created:",
        "Updated:",
        "JSM internal:",
        "Restricted:",
    ] {
        assert!(
            stdout.contains(label),
            "label layout must be unchanged — missing '{label}': {stdout:?}"
        );
    }
}

/// JSON mode: the identical hostile fixture must round-trip lossless —
/// `sanitize_table_cell` must never run on the `--output json` path for
/// `jr issue comment view` either.
///
/// Expected GREEN today and after the fix: `handle_comment_view`'s JSON arm
/// (`println!("{}", output::render_json(&response)?)`) already passes the
/// raw `serde_json::Value` straight through (EC-3.5.010-1, #526 invariant)
/// — this pins a pre-existing guarantee, not new behavior, completing the
/// table/JSON asymmetry proof for this handler.
#[tokio::test]
async fn test_bc_7_1_006_comment_view_json_output_preserves_hostile_body_and_author_raw() {
    let h = Harness::new().await;
    mount_comment_view_hostile_fixture(&h.server, "FOO-1", "10001").await;

    let output = h.run(&[
        "issue",
        "comment",
        "view",
        "FOO-1",
        "--id",
        "10001",
        "--no-input",
        "--output",
        "json",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains('\u{9b}'),
        "--output json must carry the hostile body's raw C1 byte literally \
         (JSON does not mandate escaping it, and sanitize_table_cell must \
         never run on the JSON path): {stdout:?}"
    );

    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("expected valid JSON, got {stdout}\nerror: {e}"));
    assert_eq!(
        parsed["author"]["displayName"],
        json!(COMMENT_HOSTILE_AUTHOR),
        "author.displayName must round-trip exactly unchanged, including \
         its ESC portion"
    );
    assert_eq!(
        parsed["visibility"]["value"],
        json!(COMMENT_HOSTILE_VISIBILITY_VALUE),
        "visibility.value must round-trip exactly unchanged, including its \
         ESC portion"
    );
    assert_eq!(
        parsed["body"],
        comment_hostile_body(),
        "the ADF body must round-trip exactly unchanged, including its OSC \
         sequence, bare CR, and C1 byte"
    );
}

/// Regression guard: a CLEAN comment's human (table-mode) output must be
/// byte-identical before and after SEC-003's sanitization fix — pinning the
/// exact expected stdout so any accidental format change (label wording,
/// spacing, blank-line placement, trailing newline) is caught the same way
/// a hostile-payload leak would be.
///
/// Expected GREEN today and after the fix: `sanitize_table_cell` is a
/// no-op on ASCII text containing no control characters, ANSI escapes, or
/// C1 code points — this fixture contains none, so wiring sanitization into
/// `handle_comment_view` cannot change this specific output.
#[tokio::test]
async fn test_bc_7_1_006_comment_view_human_output_clean_fixture_byte_identical() {
    let h = Harness::new().await;
    mount_comment_view_clean_fixture(&h.server, "FOO-1", "10001").await;

    let output = h.run(&[
        "issue",
        "comment",
        "view",
        "FOO-1",
        "--id",
        "10001",
        "--no-input",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stderr: {stderr}",
        output.status.code()
    );

    let expected = "ID: 10001\n\
                     Author: Jane Smith\n\
                     Created: 2026-07-01T09:00:00.000+0000\n\
                     Updated: 2026-07-01T10:30:00.000+0000\n\
                     JSM internal: N/A\n\
                     Restricted: None\n\
                     \n\
                     hello world\n";
    assert_eq!(
        stdout, expected,
        "a clean comment's human output must be byte-identical before and \
         after SEC-003's sanitization fix — any diff here is a format \
         change, not a sanitization change: {stdout:?}"
    );
}

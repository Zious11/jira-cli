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
//!
//! The `jr issue comment view` tests below (SEC-003, FIX-P5-001 extension,
//! D-393) are the one exception to the "through `render_table`" framing
//! above: `handle_comment_view` bypasses both table chokepoints entirely
//! and prints its labeled fields and ADF-derived body directly via
//! `print!`/`println!`, sanitizing each at its own print site through
//! `output::sanitize_terminal_text` instead. Those tests exercise that
//! direct-print-site sanitization, not `render_table`.

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
// ~L657-689) prints the comment `id`, `author`, `created`, `updated`,
// `visibility`-derived `restricted`, and ADF-rendered `body_text` via bare
// `print!`/`println!` — NOT through `render_table`/`print_output`. SEC-003
// extends BC-7.1.006's chokepoint guarantee to this handler's plain-text
// fields: each one is sanitized at its own print site via
// `output::sanitize_terminal_text` (pins production behavior, same as the
// `render_table`-backed tests above).

/// Hostile `author.displayName`: an ANSI CSI color sequence around
/// survivor text "Mallory", matching `HOSTILE_PAYLOAD`'s CSI-consumption
/// shape above but scoped to this section's own fixtures.
const COMMENT_HOSTILE_AUTHOR: &str = "\u{1b}[31mMallory\u{1b}[0m";

/// Hostile `visibility.value` — exercises `format_restricted_field`'s rung
/// (a) (`role`/`group` with non-empty `value`), which echoes `value`
/// verbatim into the printed "Restricted: " field. This is the "some other
/// printed field" server-controlled site beyond author/body.
const COMMENT_HOSTILE_VISIBILITY_VALUE: &str = "\u{1b}[35mEvilRole\u{1b}[0m";

/// Hostile response-body `id` field value (W-2: a prior version of this
/// fixture gave `id`/`created`/`updated` clean values, so a regression
/// deleting any of their `sanitize_terminal_text` call sites would go
/// undetected). Decoupled from the mock's URL-path `id` / the CLI's `--id`
/// argument, which must satisfy `validate_comment_id`'s alnum/underscore/
/// hyphen charset and so cannot itself carry ESC/C1 bytes — the server is
/// free to return any `id` value in the response body regardless of what
/// `--id` the client requested with. Same CSI-wrap-plus-trailing-C1-byte
/// shape as `COMMENT_HOSTILE_AUTHOR` above, sanitizing (per
/// `output::sanitize_table_cell`'s documented policy) to `"10001X"`: the
/// `ESC [ 33 m` and `ESC [ 0 m` CSI sequences are each consumed wholesale
/// (a CSI sequence ends at the first byte in `0x40..=0x7E`, here `m`), and
/// the lone `U+009B` C1 code point is dropped as a single code point —
/// `"10001"` and the trailing `"X"` are both ordinary printable ASCII and
/// pass through `Keep` unchanged.
const COMMENT_HOSTILE_ID: &str = "\u{1b}[33m10001\u{1b}[0m\u{9b}X";

/// Hostile `created` field value — same shape as `COMMENT_HOSTILE_ID`,
/// sanitizing to `"2026-07-01T09:00:00.000+0000Y"`.
const COMMENT_HOSTILE_CREATED: &str = "\u{1b}[36m2026-07-01T09:00:00.000+0000\u{1b}[0m\u{9b}Y";

/// Hostile `updated` field value — same shape as `COMMENT_HOSTILE_ID`,
/// sanitizing to `"2026-07-01T10:30:00.000+0000Z"`.
const COMMENT_HOSTILE_UPDATED: &str = "\u{1b}[36m2026-07-01T10:30:00.000+0000\u{1b}[0m\u{9b}Z";

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
/// with a hostile response-body `id`, `author.displayName`, `created`,
/// `updated`, `visibility.value`, and ADF `body` (W-2: every server-derived
/// field `handle_comment_view` prints is hostile here, so a regression
/// dropping any one of its `sanitize_terminal_text` call sites is caught).
/// The `id` function parameter still identifies the mock's URL path (and so
/// must match the CLI's `--id` argument, which is charset-restricted) — the
/// response body's own `"id"` field is independently hostile via
/// `COMMENT_HOSTILE_ID`, decoupled from that parameter. No `properties` key
/// → "JSM internal: N/A" — a static fallback token (`format_jsm_internal_field`
/// returns `"N/A"` when `properties` is absent), not server-controlled text,
/// so it needs no hostile-payload coverage here.
async fn mount_comment_view_hostile_fixture(server: &MockServer, key: &str, id: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}/comment/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": COMMENT_HOSTILE_ID,
            "author": { "displayName": COMMENT_HOSTILE_AUTHOR },
            "created": COMMENT_HOSTILE_CREATED,
            "updated": COMMENT_HOSTILE_UPDATED,
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

/// Table mode (default output): hostile `id`/`author`/`created`/`updated`/
/// `visibility.value`/ADF `body` fields must not leak a raw ESC byte, a raw
/// C1 code point, or a bare `\r` into stdout — while the CSI/C1-stripped
/// survivor text (`10001X`, `Mallory`, the two timestamps each suffixed
/// with `Y`/`Z`, `EvilRole`, and `pwned`/`line2` on two separate lines via
/// the preserved `hardBreak`-turned-`\n`) must still render, and the six
/// field labels must still all be present (layout unchanged).
///
/// Pins production behavior: `handle_comment_view`'s table-mode arm
/// sanitizes `id`, `author`, `created`, `updated`, `restricted`, and
/// `body_text` via `output::sanitize_terminal_text` at each of its own
/// print sites before printing — the hostile ESC/C1/`\r` bytes must not
/// survive, while the CSI/C1-stripped survivor text must still render.
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

    // Positive survivor assertions (W-2): the exact sanitized value for
    // every hostile field, not just "no raw ESC/C1 survived" — this is what
    // actually detects a deleted `sanitize_terminal_text` call site on
    // `id`/`created`/`updated` (a prior version of the fixture gave those
    // three fields clean values, so `assert_no_esc_or_c1` alone could not
    // have caught a regression there). Each expected literal is traced
    // against `output::sanitize_table_cell`'s documented per-character
    // policy: the `ESC [ … <final byte 0x40-0x7E>` CSI sequences wrapping
    // each hostile constant are consumed wholesale, and the lone C1 code
    // point (`U+009B`) before the trailing survivor letter is dropped as a
    // single code point — see each `COMMENT_HOSTILE_*` constant's doc
    // comment above for the full derivation.
    assert!(
        stdout.contains("ID: 10001X"),
        "sanitized id must still render its CSI/C1-stripped survivor \
         text: {stdout:?}"
    );
    assert!(
        stdout.contains("Author: Mallory"),
        "sanitized author must still render its CSI-stripped survivor \
         text: {stdout:?}"
    );
    assert!(
        stdout.contains("Created: 2026-07-01T09:00:00.000+0000Y"),
        "sanitized created must still render its CSI/C1-stripped survivor \
         text: {stdout:?}"
    );
    assert!(
        stdout.contains("Updated: 2026-07-01T10:30:00.000+0000Z"),
        "sanitized updated must still render its CSI/C1-stripped survivor \
         text: {stdout:?}"
    );
    assert!(
        stdout.contains("Restricted: EvilRole"),
        "sanitized visibility.value must still render its CSI-stripped \
         survivor text via format_restricted_field's rung (a): {stdout:?}"
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

    // Pins the exact trailing body block: the blank-line separator after
    // field 6, then the sanitized two-line body (OSC sequence, bare `\r`,
    // and C1 byte all stripped; the `hardBreak` survives as the `\n`
    // between "pwned" and "line2") followed by the `println!`-added final
    // newline. Traced against `adf_to_text` + `sanitize_terminal_text`:
    // "pwned" + (OSC dropped) + (`\r` dropped) + "\n" (hardBreak) +
    // (C1 dropped) + "line2" + "\n" (println!) = "pwned\nline2\n", preceded
    // by the print! block's own trailing "\n\n" separator.
    assert!(
        stdout.ends_with("\n\npwned\nline2\n"),
        "exact trailing body block must match after sanitization: {stdout:?}"
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

// ═══════════════════════════════════════════════════════════════════════
// `jr issue assign` — D-394 (FIX-P5-001 extension)
// ═══════════════════════════════════════════════════════════════════════
//
// `handle_assign` (`src/cli/issue/workflow.rs` ~L1006-1109) echoes the
// assignee's server-side Jira `displayName` into its human-output success
// messages via `output::print_success`, which writes to STDERR (see
// `output::print_success`'s `eprintln!` body), at two print sites, reached
// via three resolution paths that all sanitize the `display_name` before
// printing:
//   - `--to`/`--account-id` newly-assigned success:
//     `"Assigned {key} to {display_name}"` (~L1104).
//   - The idempotent already-assigned exit-0 path:
//     `"{key} is already assigned to {display_name}"` (~L1084).
//   - Self-assign (bare `jr issue assign <key>`, no `--to`/`--account-id`,
//     which resolves `display_name` via `client.get_myself()`): shares the
//     same "Assigned {key} to {display_name}" success format as the `--to`
//     path above — same L1104 call site, not a fourth site.
// `--unassign`'s two human-output messages (`"Unassigned {key}"` /
// `"{key} is already unassigned"`) echo only the CLI-supplied issue key,
// never a server-derived display name, so they need no hostile-payload
// coverage here.
//
// `--output json` emits `display_name` raw via the `assignee` key
// (`json_output::assign_changed_response`/`assign_unchanged_response`,
// `src/cli/issue/json_output.rs`) — lossless, per the issue #398
// description-echo asymmetry convention documented in CLAUDE.md.

/// Hostile assignee `displayName`: an OSC window-title sequence (terminated
/// by BEL) immediately followed by survivor text "Mallory", then a CSI
/// "clear screen" sequence (`ESC [ 2 J` — `J` is `0x4A`, within the
/// `0x40..=0x7E` CSI final-byte range). Traced against
/// `output::sanitize_table_cell`'s documented per-character policy: the OSC
/// sequence (`ESC ] 0 ; pwned <BEL>`) is consumed wholesale through its BEL
/// terminator, "Mallory" is ordinary printable ASCII and passes through
/// `Keep` unchanged, and the CSI sequence (`ESC [ 2 J`) is consumed wholesale
/// through its final byte `J` — sanitizing to exactly `"Mallory"`.
const ASSIGN_HOSTILE_DISPLAY_NAME: &str = "\u{1b}]0;pwned\u{7}Mallory\u{1b}[2J";

/// Mounts `GET /rest/api/3/user/assignable/search` (the `--to` resolution
/// endpoint, scoped by `issueKey`) returning a single user — `handle_assign`
/// -> `helpers::resolve_assignee` -> `disambiguate_user` returns a
/// single-element result set immediately without matching `name` against
/// `display_name` at all, so the mocked `display_name` need not relate to
/// the CLI's `--to` argument value.
async fn mount_assign_search_fixture(
    server: &MockServer,
    issue_key: &str,
    account_id: &str,
    display_name: &str,
) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .and(query_param("issueKey", issue_key))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![(account_id, display_name, true)]),
        ))
        .mount(server)
        .await;
}

/// Mounts `GET /rest/api/3/myself` (the self-assign / `me` resolution
/// endpoint) with a hostile `displayName`.
async fn mount_assign_myself_fixture(server: &MockServer, account_id: &str, display_name: &str) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": account_id,
            "displayName": display_name,
            "emailAddress": "mallory@example.invalid"
        })))
        .mount(server)
        .await;
}

/// Mounts `GET /rest/api/3/issue/{key}` returning a currently-unassigned
/// issue (so `handle_assign`'s idempotent check falls through to the PUT).
async fn mount_assign_get_issue_unassigned(server: &MockServer, key: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee(key, "Assign sanitization test", None),
        ))
        .mount(server)
        .await;
}

/// Mounts `GET /rest/api/3/issue/{key}` returning an issue already assigned
/// to `account_id` (so `handle_assign`'s idempotent check short-circuits
/// before any PUT).
async fn mount_assign_get_issue_assigned(server: &MockServer, key: &str, account_id: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee(
                key,
                "Assign sanitization test",
                Some((account_id, "Whatever The Server Has On File")),
            ),
        ))
        .mount(server)
        .await;
}

/// Mounts `PUT /rest/api/3/issue/{key}/assignee` -> 204.
async fn mount_assign_put_assignee(server: &MockServer, key: &str) {
    Mock::given(method("PUT"))
        .and(path(format!("/rest/api/3/issue/{key}/assignee")))
        .respond_with(ResponseTemplate::new(204))
        .mount(server)
        .await;
}

/// Table mode (default output), `--to` path: a hostile server-supplied
/// assignee `displayName` must not leak a raw ESC byte or C1 code point into
/// STDERR (`print_success` writes to stderr, not stdout), while the
/// CSI/OSC-stripped survivor text "Mallory" must still render in its normal
/// position within the unchanged "Assigned {key} to {name}" format.
///
/// Pins production behavior: `handle_assign`'s `Table` success arm
/// (`src/cli/issue/workflow.rs` ~L1104) sanitizes `display_name` via
/// `output::sanitize_terminal_text` before formatting it into
/// `output::print_success(&format!("Assigned {} to {}", key, display_name))`
/// — no raw ESC/BEL/CSI byte survives in stderr.
#[tokio::test]
async fn test_bc_7_1_006_issue_assign_human_output_strips_hostile_display_name() {
    let h = Harness::new().await;
    mount_assign_search_fixture(
        &h.server,
        "FOO-1",
        "acc-mallory",
        ASSIGN_HOSTILE_DISPLAY_NAME,
    )
    .await;
    mount_assign_get_issue_unassigned(&h.server, "FOO-1").await;
    mount_assign_put_assignee(&h.server, "FOO-1").await;

    let output = h.run(&["issue", "assign", "FOO-1", "--to", "Mallory", "--no-input"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stdout: {stdout} stderr: {stderr}",
        output.status.code()
    );
    assert_no_esc_or_c1(&stderr, "jr issue assign --to (human output, stderr)");
    assert_eq!(
        stderr, "Assigned FOO-1 to Mallory\n",
        "sanitized message must show the CSI/OSC-stripped survivor text \
         'Mallory' in its normal position within the unchanged 'Assigned \
         {{key}} to {{name}}' format: {stderr:?}"
    );
}

/// Table mode, idempotent already-assigned path: same hostile-payload
/// guarantee as the newly-assigned case above, for the
/// `"{key} is already assigned to {display_name}"` message
/// (`src/cli/issue/workflow.rs` ~L1084). No PUT is mocked — the idempotent
/// short-circuit must fire before any HTTP write, exactly as
/// `test_handler_assign_idempotent` (`tests/cli_handler.rs`) already proves
/// for the non-hostile case; a stray PUT call here would 404 against
/// wiremock's unmocked-request default and fail the test via the exit-code
/// assertion.
///
/// Pins production behavior: same sanitized `print_success` call site,
/// just the idempotent branch (~L1084) instead of the newly-assigned one.
#[tokio::test]
async fn test_bc_7_1_006_issue_assign_human_output_strips_hostile_display_name_idempotent() {
    let h = Harness::new().await;
    mount_assign_search_fixture(
        &h.server,
        "FOO-2",
        "acc-mallory",
        ASSIGN_HOSTILE_DISPLAY_NAME,
    )
    .await;
    mount_assign_get_issue_assigned(&h.server, "FOO-2", "acc-mallory").await;

    let output = h.run(&["issue", "assign", "FOO-2", "--to", "Mallory", "--no-input"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stdout: {stdout} stderr: {stderr}",
        output.status.code()
    );
    assert_no_esc_or_c1(&stderr, "jr issue assign --to (idempotent, stderr)");
    assert_eq!(
        stderr, "FOO-2 is already assigned to Mallory\n",
        "sanitized idempotent message must show the CSI/OSC-stripped \
         survivor text 'Mallory' in its normal position: {stderr:?}"
    );
}

/// Table mode, self-assign path (bare `jr issue assign <key>`, no `--to`/
/// `--account-id`): same hostile-payload guarantee, resolving
/// `display_name` via `client.get_myself()` instead of the assignable-user
/// search, but sharing the SAME "Assigned {key} to {name}" print-site
/// (~L1104) as the `--to` test above.
///
/// Pins production behavior: same sanitized `print_success` call site,
/// reached via the self-assign resolution branch instead of `--to`.
#[tokio::test]
async fn test_bc_7_1_006_issue_assign_human_output_strips_hostile_display_name_self_assign() {
    let h = Harness::new().await;
    mount_assign_myself_fixture(&h.server, "acc-mallory", ASSIGN_HOSTILE_DISPLAY_NAME).await;
    mount_assign_get_issue_unassigned(&h.server, "FOO-3").await;
    mount_assign_put_assignee(&h.server, "FOO-3").await;

    let output = h.run(&["issue", "assign", "FOO-3", "--no-input"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stdout: {stdout} stderr: {stderr}",
        output.status.code()
    );
    assert_no_esc_or_c1(
        &stderr,
        "jr issue assign self-assign (human output, stderr)",
    );
    assert_eq!(
        stderr, "Assigned FOO-3 to Mallory\n",
        "sanitized self-assign message must show the CSI/OSC-stripped \
         survivor text 'Mallory' in its normal position: {stderr:?}"
    );
}

/// JSON mode: the identical hostile fixture must round-trip lossless via
/// the `assignee` key — `sanitize_table_cell`/`sanitize_terminal_text` must
/// never run on the `--output json` path, mirroring every other JSON-mode
/// test in this file.
///
/// Expected GREEN today and after the fix: `handle_assign`'s `Json` success
/// arm (`src/cli/issue/workflow.rs` ~L1093-1101) already passes
/// `json_output::assign_changed_response`'s raw `display_name` straight
/// through `output::render_json` (#526 invariant) — this pins a
/// pre-existing guarantee, not new behavior, completing the table/JSON
/// asymmetry proof for this handler.
#[tokio::test]
async fn test_bc_7_1_006_issue_assign_json_output_preserves_hostile_display_name_raw() {
    let h = Harness::new().await;
    mount_assign_search_fixture(
        &h.server,
        "FOO-4",
        "acc-mallory",
        ASSIGN_HOSTILE_DISPLAY_NAME,
    )
    .await;
    mount_assign_get_issue_unassigned(&h.server, "FOO-4").await;
    mount_assign_put_assignee(&h.server, "FOO-4").await;

    let output = h.run(&[
        "issue",
        "assign",
        "FOO-4",
        "--to",
        "Mallory",
        "--no-input",
        "--output",
        "json",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stdout: {stdout} stderr: {stderr}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("expected valid JSON, got {stdout}\nerror: {e}"));
    assert_eq!(
        parsed["assignee"],
        json!(ASSIGN_HOSTILE_DISPLAY_NAME),
        "--output json's 'assignee' key must round-trip the hostile display \
         name exactly unchanged, including its ESC/BEL/CSI bytes: {parsed:?}"
    );
    assert_eq!(parsed["changed"], json!(true));
}

/// Regression guard: a CLEAN assignee display name's human (table-mode)
/// output must be byte-identical before and after this fix — pinning the
/// exact expected stderr so any accidental format change (wording, spacing,
/// trailing newline) is caught the same way a hostile-payload leak would be.
///
/// Expected GREEN today and after the fix: `sanitize_table_cell`/
/// `sanitize_terminal_text` is a no-op on ASCII text containing no control
/// characters, ANSI escapes, or C1 code points — "Jane Doe" contains none,
/// so wiring sanitization into `handle_assign` cannot change this specific
/// output.
#[tokio::test]
async fn test_bc_7_1_006_issue_assign_human_output_clean_fixture_byte_identical() {
    let h = Harness::new().await;
    mount_assign_search_fixture(&h.server, "FOO-5", "acc-jane", "Jane Doe").await;
    mount_assign_get_issue_unassigned(&h.server, "FOO-5").await;
    mount_assign_put_assignee(&h.server, "FOO-5").await;

    let output = h.run(&["issue", "assign", "FOO-5", "--to", "Jane", "--no-input"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}. stdout: {stdout} stderr: {stderr}",
        output.status.code()
    );
    assert_eq!(
        stderr, "Assigned FOO-5 to Jane Doe\n",
        "a clean assignee's human output must be byte-identical before and \
         after this fix — any diff here is a format change, not a \
         sanitization change: {stderr:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Shared user disambiguation — D-395 (FIX-P5-001 extension)
// ═══════════════════════════════════════════════════════════════════════
//
// `src/cli/issue/helpers.rs::disambiguate_user` (~L276-400) is the SHARED
// disambiguation helper behind `resolve_user`, `resolve_assignee` (`jr
// issue assign --to`), `resolve_assignee_by_project` (`jr issue create/edit
// --assignee`), and `mentions::resolve_mentions`. Its THREE non-interactive
// (`--no-input`/non-TTY) `JrError::UserError` branches echo server-supplied,
// user-editable `display_name`/`email_address`/`account_id` — each routed
// through `output::sanitize_terminal_text` (D-395) before it reaches the
// message:
//   - `MatchResult::ExactMultiple`: `"Multiple users named \"{name}\"
//     found:\n  {display_name} ({email}, account: {account_id})\n...\n
//     Specify the accountId directly or use a more specific name."`
//     (`email` absent → `"  {display_name} (account: {account_id})"`).
//   - `MatchResult::Ambiguous`: `"Multiple users match \"{name}\": {csv of
//     raw display_name}. Use a more specific name."`.
//   - `MatchResult::None`: `resolve_assignee`'s `none_msg_fn` closure joins
//     `all_names` — every assignable user on the issue, not only ones that
//     matched the query — into its own `"… Found: {csv of raw
//     display_name}"` message.
// `name` (the CLI-supplied search string) is NOT itself server-derived, so
// these tests deliberately keep the `--to`/`--assignee` argument ASCII-clean
// — ExactMultiple's trigger mechanics require the argument to equal the
// candidate display names EXACTLY (case-insensitively), so hostile
// `display_name` values are exercised only via the Ambiguous (substring)
// branch below, where equality is not required.
//
// Driven through TWO different callers (`jr issue assign` and `jr issue
// create --assignee`) to prove the fix, once applied to the shared
// `disambiguate_user` function, covers every caller — not just one
// command's call site.

/// ExactMultiple hostile `account_id`/`email_address` fixtures. `display_name`
/// is deliberately the CLEAN, ASCII string `"Mallory"` for both users (see
/// section header above for why) — these four consts exercise the CSI, `\r`,
/// and C1 sanitization mechanisms across the two echoed server fields.
const DISAMBIG_HOSTILE_ACC_1: &str = "\u{1b}[32mACC-1\u{1b}[0m";
const DISAMBIG_HOSTILE_EMAIL_1: &str = "\u{1b}[35mmallory1\u{1b}[0m@example.invalid";
const DISAMBIG_HOSTILE_ACC_2: &str = "ACC-2\rZ";
const DISAMBIG_HOSTILE_EMAIL_2: &str = "mallory2\u{9b}Q@example.invalid";

/// Ambiguous hostile `display_name` fixtures — the exact two example
/// payloads from the D-395 task brief. Both sanitize to the identical
/// survivor text `"Alice"` (traced in each const's doc comment) despite
/// being raw-distinct strings, which is why the Ambiguous branch's
/// `matches.join(", ")` is expected to render `"Alice, Alice"` once fixed.
///
/// `"\u{1b}[31mAlice\u{1b}[0m"`: CSI `ESC [ 31 m` (consumed through final
/// byte `m`) + `"Alice"` (kept) + CSI `ESC [ 0 m` (consumed) → `"Alice"`.
const DISAMBIG_HOSTILE_NAME_1: &str = "\u{1b}[31mAlice\u{1b}[0m";
/// `"Al\u{9b}ice\u{1b}]0;x\u{7}"`: `"Al"` (kept) + C1 `U+009B` (dropped, a
/// single code point — no separator inserted) + `"ice"` (kept) + OSC
/// `ESC ] 0 ; x <BEL>` (consumed through its BEL terminator) → `"Alice"`.
const DISAMBIG_HOSTILE_NAME_2: &str = "Al\u{9b}ice\u{1b}]0;x\u{7}";

/// Builds a `User` JSON object (`accountId`/`displayName`/`active`, plus
/// `emailAddress` when `email` is `Some`) for the two disambiguation search
/// fixtures below.
fn disambig_user_obj(account_id: &str, display_name: &str, email: Option<&str>) -> Value {
    let mut obj = json!({
        "accountId": account_id,
        "displayName": display_name,
        "active": true,
    });
    if let Some(e) = email {
        obj["emailAddress"] = json!(e);
    }
    obj
}

/// Mounts `GET /rest/api/3/user/assignable/search` (the `jr issue assign
/// --to` resolution endpoint, scoped by `issueKey`) returning `users`
/// verbatim.
async fn mount_disambig_search_by_issue(server: &MockServer, issue_key: &str, users: Vec<Value>) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .and(query_param("issueKey", issue_key))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(users)))
        .mount(server)
        .await;
}

/// Mounts `GET /rest/api/3/user/assignable/multiProjectSearch` (the `jr
/// issue create --assignee` resolution endpoint, scoped by `projectKeys`)
/// returning `users` verbatim.
async fn mount_disambig_search_by_project(server: &MockServer, project: &str, users: Vec<Value>) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", project))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(users)))
        .mount(server)
        .await;
}

/// The two ExactMultiple hostile duplicate users shared by the
/// human-output, JSON-envelope-pin, and clean-fixture-guard tests below.
fn disambig_exact_multiple_hostile_users() -> Vec<Value> {
    vec![
        disambig_user_obj(
            DISAMBIG_HOSTILE_ACC_1,
            "Mallory",
            Some(DISAMBIG_HOSTILE_EMAIL_1),
        ),
        disambig_user_obj(
            DISAMBIG_HOSTILE_ACC_2,
            "Mallory",
            Some(DISAMBIG_HOSTILE_EMAIL_2),
        ),
    ]
}

/// `jr issue assign --to Mallory`, ExactMultiple branch: a hostile
/// `account_id`/`email_address` pair on each of two same-named duplicate
/// users must not leak a raw ESC byte, C1 code point, or bare `\r` into
/// STDERR, while the CSI/`\r`/C1-stripped survivor text must still render
/// in the unchanged message format.
///
/// Pins production behavior: `disambiguate_user`'s `ExactMultiple`
/// non-interactive branch (`src/cli/issue/helpers.rs`) builds `lines` from
/// `u.display_name`/`email`/`u.account_id`, each routed through
/// `output::sanitize_terminal_text` before formatting — the hostile bytes
/// never reach stderr.
#[tokio::test]
async fn test_bc_7_1_006_issue_assign_exact_multiple_human_output_strips_hostile_fields() {
    let h = Harness::new().await;
    mount_disambig_search_by_issue(&h.server, "FOO-10", disambig_exact_multiple_hostile_users())
        .await;

    let output = h.run(&["issue", "assign", "FOO-10", "--to", "Mallory", "--no-input"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 (UserError), got {:?}. stdout: {stdout} stderr: {stderr}",
        output.status.code()
    );
    assert_no_esc_or_c1(&stderr, "jr issue assign --to (ExactMultiple, stderr)");
    assert!(
        !stderr.contains('\r'),
        "jr issue assign --to (ExactMultiple, stderr): raw CR must not \
         survive: {stderr:?}"
    );
    assert_eq!(
        stderr,
        "Error: Multiple users named \"Mallory\" found:\n  \
         Mallory (mallory1@example.invalid, account: ACC-1)\n  \
         Mallory (mallory2Q@example.invalid, account: ACC-2Z)\n\
         Specify the accountId directly or use a more specific name.\n",
        "sanitized ExactMultiple message must show the CSI/\\r/C1-stripped \
         survivor text for both duplicates' email/account_id, in their \
         normal position within the unchanged message format: {stderr:?}"
    );
}

/// `jr issue assign --to ic`, Ambiguous branch: two hostile `display_name`
/// values (the exact D-395 task-brief example payloads — see
/// `DISAMBIG_HOSTILE_NAME_1`/`_2`'s doc comments) must not leak a raw ESC
/// byte, C1 code point, or OSC sequence into STDERR, while the
/// CSI/C1/OSC-stripped survivor text must still render.
///
/// Pins production behavior: `disambiguate_user`'s `Ambiguous`
/// non-interactive branch (`src/cli/issue/helpers.rs`) maps `matches`
/// through `output::sanitize_terminal_text` into `sanitized_matches`
/// before joining it into the message — the hostile bytes never reach
/// stderr.
#[tokio::test]
async fn test_bc_7_1_006_issue_assign_ambiguous_human_output_strips_hostile_display_names() {
    let h = Harness::new().await;
    mount_disambig_search_by_issue(
        &h.server,
        "FOO-11",
        vec![
            disambig_user_obj("ACC-A", DISAMBIG_HOSTILE_NAME_1, None),
            disambig_user_obj("ACC-B", DISAMBIG_HOSTILE_NAME_2, None),
        ],
    )
    .await;

    let output = h.run(&["issue", "assign", "FOO-11", "--to", "ic", "--no-input"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 (UserError), got {:?}. stdout: {stdout} stderr: {stderr}",
        output.status.code()
    );
    assert_no_esc_or_c1(&stderr, "jr issue assign --to (Ambiguous, stderr)");
    assert_eq!(
        stderr, "Error: Multiple users match \"ic\": Alice, Alice. Use a more specific name.\n",
        "sanitized Ambiguous message must show the CSI/C1/OSC-stripped \
         survivor text 'Alice' for both distinct hostile display names: \
         {stderr:?}"
    );
}

/// `jr issue create --assignee` (via `--to`), Ambiguous branch, proving the
/// fix (once applied to the SHARED `disambiguate_user` function) covers a
/// second, different caller — `resolve_assignee_by_project`, not
/// `resolve_assignee`. Same hostile `display_name` fixtures and expected
/// sanitized message shape as the `jr issue assign` Ambiguous test above.
///
/// Pins production behavior: same sanitized `matches.join(", ")` shared
/// code path as the test above, reached via
/// `helpers::resolve_assignee_by_project` instead of
/// `helpers::resolve_assignee`.
#[tokio::test]
async fn test_bc_7_1_006_issue_create_assignee_ambiguous_human_output_strips_hostile_display_names()
{
    let h = Harness::new().await;
    mount_disambig_search_by_project(
        &h.server,
        "FOO",
        vec![
            disambig_user_obj("ACC-A", DISAMBIG_HOSTILE_NAME_1, None),
            disambig_user_obj("ACC-B", DISAMBIG_HOSTILE_NAME_2, None),
        ],
    )
    .await;

    let output = h.run(&[
        "issue",
        "create",
        "--project",
        "FOO",
        "--type",
        "Task",
        "--summary",
        "D-395 create --assignee coverage",
        "--to",
        "ic",
        "--no-input",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 (UserError), got {:?}. stdout: {stdout} stderr: {stderr}",
        output.status.code()
    );
    assert_no_esc_or_c1(&stderr, "jr issue create --to (Ambiguous, stderr)");
    assert_eq!(
        stderr, "Error: Multiple users match \"ic\": Alice, Alice. Use a more specific name.\n",
        "sanitized Ambiguous message must show the CSI/C1/OSC-stripped \
         survivor text 'Alice' for both distinct hostile display names, \
         proving the SHARED disambiguate_user fix covers this different \
         caller too: {stderr:?}"
    );
}

/// `--output json`, ExactMultiple branch: per BC-7.1.006's `disambiguate_user`
/// Behavior subsection and its dedicated `--output json` decision paragraph
/// (D-395), this sink has only ONE underlying message string —
/// `src/main.rs`'s top-level error handler builds BOTH the human-mode
/// `eprintln!("Error: {e}")` text AND the `--output json` error envelope's
/// `{"error": e.to_string(), "code": <exit>}` from the SAME
/// `JrError::UserError` `Display` string. Because `disambiguate_user`
/// sanitizes at message-construction time (inside the function itself,
/// not at a table-mode print site), that single shared string is already
/// sanitized before either channel renders it — there is no separate,
/// lossless machine channel for this sink the way `render_table`'s own
/// table/JSON success-data asymmetry works. This test therefore asserts
/// the JSON `"error"` field carries the SAME sanitized, CSI/`\r`/C1-stripped
/// text as the table-mode stderr message (EC-16/VP-SEC-001-001(c)),
/// covering a downstream script or CI log viewer that surfaces
/// `--output json`'s `"error"` text to a terminal (e.g.
/// `jr … --output json | jq -r .error`), which is exactly as exposed to
/// CWE-150/CWE-116 as the human-mode path.
#[tokio::test]
async fn test_bc_7_1_006_issue_assign_exact_multiple_json_error_envelope_carries_sanitized_text() {
    let h = Harness::new().await;
    mount_disambig_search_by_issue(&h.server, "FOO-12", disambig_exact_multiple_hostile_users())
        .await;

    let output = h.run(&[
        "issue",
        "assign",
        "FOO-12",
        "--to",
        "Mallory",
        "--no-input",
        "--output",
        "json",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 (UserError), got {:?}. stdout: {stdout} stderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout, "", "no data should reach stdout on an error path");

    let parsed: Value = serde_json::from_str(stderr.trim_end())
        .unwrap_or_else(|e| panic!("expected valid JSON on stderr, got {stderr}\nerror: {e}"));
    assert_eq!(parsed["code"], json!(64));

    let expected_sanitized_error = "Multiple users named \"Mallory\" found:\n  \
         Mallory (mallory1@example.invalid, account: ACC-1)\n  \
         Mallory (mallory2Q@example.invalid, account: ACC-2Z)\n\
         Specify the accountId directly or use a more specific name.";
    assert_eq!(
        parsed["error"],
        json!(expected_sanitized_error),
        "the JSON error envelope's \"error\" field must carry the identical \
         sanitized, CSI/\\r/C1-stripped text as the table-mode stderr \
         message — disambiguate_user sanitizes once at message-construction \
         time, so both channels share the same already-sanitized string \
         (BC-7.1.006's `disambiguate_user` --output json decision \
         paragraph, D-395): {parsed:?}"
    );
    let raw_serialized = serde_json::to_string(&parsed).unwrap();
    assert_no_esc_or_c1(
        &raw_serialized,
        "jr issue assign --to (ExactMultiple, --output json \"error\" field)",
    );
}

/// Regression guard: a CLEAN ExactMultiple duplicate pair's human
/// (table-mode) output must be byte-identical before and after this fix —
/// pinning the exact expected stderr so any accidental format change is
/// caught the same way a hostile-payload leak would be.
///
/// Expected GREEN today and after the fix: `sanitize_table_cell`/
/// `sanitize_terminal_text` is a no-op on ASCII text containing no control
/// characters, ANSI escapes, or C1 code points — none of this fixture's
/// fields contain any, so wiring sanitization into `disambiguate_user`
/// cannot change this specific output.
#[tokio::test]
async fn test_bc_7_1_006_issue_assign_exact_multiple_human_output_clean_fixture_byte_identical() {
    let h = Harness::new().await;
    mount_disambig_search_by_issue(
        &h.server,
        "FOO-13",
        vec![
            disambig_user_obj("acc-jane-1", "Jane Doe", Some("jane1@example.com")),
            disambig_user_obj("acc-jane-2", "Jane Doe", Some("jane2@example.com")),
        ],
    )
    .await;

    let output = h.run(&[
        "issue",
        "assign",
        "FOO-13",
        "--to",
        "Jane Doe",
        "--no-input",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 (UserError), got {:?}. stdout: {stdout} stderr: {stderr}",
        output.status.code()
    );
    assert_eq!(
        stderr,
        "Error: Multiple users named \"Jane Doe\" found:\n  \
         Jane Doe (jane1@example.com, account: acc-jane-1)\n  \
         Jane Doe (jane2@example.com, account: acc-jane-2)\n\
         Specify the accountId directly or use a more specific name.\n",
        "a clean ExactMultiple duplicate pair's human output must be \
         byte-identical before and after this fix — any diff here is a \
         format change, not a sanitization change: {stderr:?}"
    );
}

/// `jr issue assign --to <no-match>`, `MatchResult::None` branch: the
/// `all_names` candidate list — every assignable user on the issue, not
/// only ones that matched the query — is joined by `resolve_assignee`'s
/// `none_msg_fn` closure into its own "… Found: …" message. This is a
/// WIDER exposure surface than the `ExactMultiple`/`Ambiguous` branches
/// above, since a hostile display name can leak here merely by being
/// assignable on the same issue, without ever matching the query string
/// (D-395, §10 spec-delta, PR #891 finding found during the amendment's
/// own code trace — not explicitly named in the originating SEC-891-2
/// finding).
///
/// Pins production behavior: `disambiguate_user`'s `MatchResult::None`
/// branch (`src/cli/issue/helpers.rs`) maps `all_names` through
/// `output::sanitize_terminal_text` into `sanitized_names` before handing
/// that vec to `none_msg_fn` — the hostile bytes never reach stderr.
#[tokio::test]
async fn test_bc_7_1_006_issue_assign_none_human_output_strips_hostile_candidate_names() {
    let h = Harness::new().await;
    mount_disambig_search_by_issue(
        &h.server,
        "FOO-14",
        vec![
            disambig_user_obj("ACC-A", DISAMBIG_HOSTILE_NAME_1, None),
            disambig_user_obj("ACC-B", "Bob", None),
        ],
    )
    .await;

    let output = h.run(&[
        "issue",
        "assign",
        "FOO-14",
        "--to",
        "zzz-no-match",
        "--no-input",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 (UserError), got {:?}. stdout: {stdout} stderr: {stderr}",
        output.status.code()
    );
    assert_no_esc_or_c1(&stderr, "jr issue assign --to (None, stderr)");
    assert_eq!(
        stderr,
        "Error: No assignable user with a name matching \"zzz-no-match\" \
         on issue FOO-14. Found: Alice, Bob\n",
        "sanitized None-branch message must show the CSI-stripped survivor \
         text 'Alice' for the hostile candidate, in the unchanged 'Found: …' \
         message format: {stderr:?}"
    );
}

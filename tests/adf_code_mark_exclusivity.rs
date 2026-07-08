//! Integration tests for BC-7.2.015: code-mark exclusivity at the CLI boundary.
//! H-NEW-ADF-010 Calls A–D (platform path, `POST /rest/api/3/issue`).
//!
//! These tests drive `jr issue create --markdown` through a wiremock server,
//! capture the `POST /rest/api/3/issue` body, and assert that in every
//! `fields.description` ADF text node that carries a `code` mark, NO typographic
//! mark (`strong`, `em`, `strike`, `subsup`, `underline`, `textColor`,
//! `backgroundColor`) is present.
//!
//! The four calls cover the main cases from BC-7.2.015:
//!
//!   Call A (EC-1): `**\`hello\`**` — `strong` wrapping code — `strong` stripped.
//!   Call B (EC-4): `^\`code\`^` — `subsup` wrapping code — `subsup` stripped
//!                  (primary issue #571 regression target).
//!   Call C (EC-5): `[\`code\`](https://example.com)` — `link` alongside code —
//!                  `link` MUST be preserved (schema-valid co-mark).
//!   Call D (EC-6): `**a \`b\` c**` — mixed range — surrounding `strong` text
//!                  retains its mark; the inner code node strips `strong`.
//!
//! Call E (JSM path parity) lives in `tests/issue_create_jsm.rs`.
//!
//! BC anchor: BC-7.2.015 (`push_code` allowlist filter, sole emit site for
//! `{"type":"code"}` marks in `markdown_to_adf`).
//! Story: S-ADF-CODE-MARK-1 (issue #571).
//! Holdout: H-NEW-ADF-010.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

fn jr_cmd(server_url: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("JR_CACHE_DIR", cache_dir.join("jr"))
        .env("XDG_CONFIG_HOME", config_dir)
        .env("JR_CONFIG_DIR", config_dir.join("jr"));
    cmd
}

/// Write a minimal empty `config.toml` so the config loader does not read a
/// developer's real config. `JR_BASE_URL` (debug seam) overrides the instance
/// URL at runtime; no `[instance].url` key is required here.
fn write_minimal_config(config_home: &std::path::Path) {
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(conf_dir.join("config.toml"), "").unwrap();
}

// ---------------------------------------------------------------------------
// ADF-walking helpers (local copies — integration tests cannot reach into the
// `#[cfg(test)]` helpers in `src/adf.rs`).
// ---------------------------------------------------------------------------

/// Recursively collect all `{"type":"text"}` nodes from an ADF tree.
fn collect_text_nodes<'a>(node: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
    if node.get("type").and_then(|t| t.as_str()) == Some("text") {
        out.push(node);
    }
    if let Some(children) = node.get("content").and_then(|c| c.as_array()) {
        for child in children {
            collect_text_nodes(child, out);
        }
    }
}

/// Return `true` if a `marks` JSON array contains an entry with the given type.
fn has_mark(marks: &serde_json::Value, mark_type: &str) -> bool {
    marks
        .as_array()
        .is_some_and(|arr| arr.iter().any(|m| m["type"].as_str() == Some(mark_type)))
}

/// Return the first mark entry whose `type` field equals `mark_type`.
fn find_mark<'a>(marks: &'a serde_json::Value, mark_type: &str) -> Option<&'a serde_json::Value> {
    marks
        .as_array()?
        .iter()
        .find(|m| m["type"].as_str() == Some(mark_type))
}

/// Assert BC-7.2.015 code-mark exclusivity across every text node in `adf`:
/// no text node that carries a `code` mark may also carry any typographic mark.
///
/// Local equivalent of `assert_code_mark_exclusivity` in `src/adf.rs::tests`.
fn assert_code_mark_exclusivity(adf: &serde_json::Value) {
    const FORBIDDEN: &[&str] = &[
        "strong",
        "em",
        "strike",
        "subsup",
        "underline",
        "textColor",
        "backgroundColor",
    ];
    let mut text_nodes = Vec::new();
    collect_text_nodes(adf, &mut text_nodes);
    for tn in &text_nodes {
        let marks = &tn["marks"];
        if has_mark(marks, "code") {
            for ftype in FORBIDDEN {
                assert!(
                    !has_mark(marks, ftype),
                    "BC-7.2.015: text node {:?} carries both `code` mark and forbidden \
                     typographic mark {ftype:?}. marks={marks}",
                    tn["text"]
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared mock setup and body capture helpers
// ---------------------------------------------------------------------------

/// Mount `POST /rest/api/3/issue` returning HTTP 201 with `.expect(1)`.
///
/// `.expect(1)` fires automatically on server drop, verifying exactly one POST
/// was made (guards against silent retry or double-submission regressions).
async fn mount_platform_post(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10010",
            "key": "PROJ-10",
            "self": format!("{}/rest/api/3/issue/10010", server.uri()),
        })))
        .expect(1)
        .mount(server)
        .await;
}

/// Capture the POST body and return the parsed `fields.description` ADF value.
///
/// Asserts exactly 1 request was received (the human-text `issue create` path
/// makes only the single POST; the JSON path would add a follow-on GET for the
/// created issue, but these tests use the default human output path) and that
/// `fields.description` is non-null.
async fn capture_description_adf(server: &MockServer) -> serde_json::Value {
    let captured = server
        .received_requests()
        .await
        .expect("wiremock must record received requests");
    assert_eq!(
        captured.len(),
        1,
        "Expected exactly 1 POST to /rest/api/3/issue (human-text output path makes \
         no follow-on GETs); got {}",
        captured.len()
    );
    let body_str = std::str::from_utf8(&captured[0].body).expect("POST body must be valid UTF-8");
    let body_json: serde_json::Value = serde_json::from_str(body_str)
        .unwrap_or_else(|e| panic!("POST body must be valid JSON: {e}; body={body_str}"));
    let desc = body_json["fields"]["description"].clone();
    assert!(
        !desc.is_null(),
        "fields.description must be present in POST body; body={body_str}"
    );
    desc
}

// ---------------------------------------------------------------------------
// Call A — strong wrapping code (EC-1, BC-7.2.015)
// ---------------------------------------------------------------------------

/// H-NEW-ADF-010 Call A (BC-7.2.015 EC-1): `**\`hello\`**` — `strong` wrapping
/// inline code — `strong` must be stripped from the code text node.
///
/// The `code`-marked text node `"hello"` must carry marks `[{"type":"code"}]` ONLY.
/// NOT `[{"type":"strong"},{"type":"code"}]`.
///
/// GREEN pre-fix AND post-fix (retention anchor): `push_code` must not accidentally
/// drop the `code` mark itself, nor change the text content.
#[tokio::test]
async fn test_bc_7_2_015_call_a_strong_code_mark_stripped_platform_path() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path());
    mount_platform_post(&server).await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "strong-code",
            "--markdown",
            "--description",
            "**`hello`**",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Call A: expected exit 0; stderr={stderr:?} stdout={stdout:?}"
    );

    let adf = capture_description_adf(&server).await;

    // Universal invariant: no code-marked text node carries a typographic mark.
    assert_code_mark_exclusivity(&adf);

    // Specific assertion: text "hello" carries marks [code] ONLY.
    let mut text_nodes = Vec::new();
    collect_text_nodes(&adf, &mut text_nodes);
    let hello_node = text_nodes
        .iter()
        .find(|n| n["text"].as_str() == Some("hello"))
        .unwrap_or_else(|| panic!("Call A: expected text node 'hello' in ADF; adf={adf}"));
    let marks = &hello_node["marks"];
    assert!(
        has_mark(marks, "code"),
        "Call A: 'hello' node must carry `code` mark; marks={marks}"
    );
    assert!(
        !has_mark(marks, "strong"),
        "Call A: 'hello' node must NOT carry `strong` mark (stripped by push_code \
         allowlist filter); marks={marks}"
    );
    assert_eq!(
        marks.as_array().map(|a| a.len()).unwrap_or(0),
        1,
        "Call A: 'hello' node must carry exactly 1 mark ([{{\"type\":\"code\"}}]); marks={marks}"
    );
}

// ---------------------------------------------------------------------------
// Call B — subsup wrapping code (EC-4, BC-7.2.015 primary regression target)
// ---------------------------------------------------------------------------

/// H-NEW-ADF-010 Call B (BC-7.2.015 EC-4): `^\`code\`^` — `subsup` wrapping
/// inline code — `subsup` must be stripped. Primary issue #571 regression target.
///
/// The `code`-marked text node `"code"` must carry marks `[{"type":"code"}]` ONLY.
/// NOT `[{"type":"subsup","attrs":{"type":"sup"}},{"type":"code"}]`.
///
/// `subsup` is applied to the `active_marks` stack during the superscript span;
/// `push_code`'s allowlist filter clones and strips it at the `Code` event boundary.
#[tokio::test]
async fn test_bc_7_2_015_call_b_subsup_code_mark_stripped_platform_path() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path());
    mount_platform_post(&server).await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "subsup-code",
            "--markdown",
            "--description",
            "^`code`^",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Call B: expected exit 0; stderr={stderr:?} stdout={stdout:?}"
    );

    let adf = capture_description_adf(&server).await;

    // Universal invariant: no code-marked text node carries a typographic mark.
    assert_code_mark_exclusivity(&adf);

    // Specific assertion: text "code" carries marks [code] ONLY.
    let mut text_nodes = Vec::new();
    collect_text_nodes(&adf, &mut text_nodes);
    let code_node = text_nodes
        .iter()
        .find(|n| n["text"].as_str() == Some("code"))
        .unwrap_or_else(|| panic!("Call B: expected text node 'code' in ADF; adf={adf}"));
    let marks = &code_node["marks"];
    assert!(
        has_mark(marks, "code"),
        "Call B: 'code' node must carry `code` mark; marks={marks}"
    );
    assert!(
        !has_mark(marks, "subsup"),
        "Call B (issue #571 regression guard): 'code' node must NOT carry `subsup` mark \
         (stripped by push_code allowlist filter); marks={marks}"
    );
    assert_eq!(
        marks.as_array().map(|a| a.len()).unwrap_or(0),
        1,
        "Call B: 'code' node must carry exactly 1 mark ([{{\"type\":\"code\"}}]); marks={marks}"
    );
}

// ---------------------------------------------------------------------------
// Call C — link preserved alongside code (EC-5, BC-7.2.015)
// ---------------------------------------------------------------------------

/// H-NEW-ADF-010 Call C (BC-7.2.015 EC-5): `[\`code\`](https://example.com)` —
/// `link` mark co-existing with inline code — `link` MUST be preserved.
///
/// The `code`-marked text node `"code"` must carry BOTH `{"type":"code"}` AND a
/// `link` mark with `attrs.href == "https://example.com"`.
///
/// The `link` mark is schema-valid alongside `code` (`code_inline_node` permits
/// `code`, `link`, `annotation`). GREEN pre-fix AND post-fix (retention anchor)
/// — catches a regression where `link` is accidentally dropped from the
/// `push_code` allowlist.
#[tokio::test]
async fn test_bc_7_2_015_call_c_link_preserved_with_code_mark_platform_path() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path());
    mount_platform_post(&server).await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "link-code",
            "--markdown",
            "--description",
            "[`code`](https://example.com)",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Call C: expected exit 0; stderr={stderr:?} stdout={stdout:?}"
    );

    let adf = capture_description_adf(&server).await;

    // Universal invariant: no code-marked text node carries a typographic mark.
    assert_code_mark_exclusivity(&adf);

    // Specific assertion: text "code" carries both `code` AND `link` marks.
    let mut text_nodes = Vec::new();
    collect_text_nodes(&adf, &mut text_nodes);
    let code_node = text_nodes
        .iter()
        .find(|n| n["text"].as_str() == Some("code"))
        .unwrap_or_else(|| panic!("Call C: expected text node 'code' in ADF; adf={adf}"));
    let marks = &code_node["marks"];
    assert!(
        has_mark(marks, "code"),
        "Call C: 'code' node must carry `code` mark; marks={marks}"
    );
    assert!(
        has_mark(marks, "link"),
        "Call C: 'code' node must carry `link` mark (schema-valid co-mark; must NOT \
         be dropped by push_code allowlist); marks={marks}"
    );
    // The `link` mark must carry attrs.href == "https://example.com".
    let link_mark = find_mark(marks, "link")
        .unwrap_or_else(|| panic!("Call C: link mark must be present; marks={marks}"));
    let href = link_mark["attrs"]["href"]
        .as_str()
        .unwrap_or_else(|| panic!("Call C: link mark must have attrs.href; mark={link_mark}"));
    assert_eq!(
        href, "https://example.com",
        "Call C: link mark attrs.href must be 'https://example.com'; got {href:?}"
    );
    // No typographic marks (already covered by assert_code_mark_exclusivity; explicit for clarity).
    assert!(
        !has_mark(marks, "strong"),
        "Call C: 'code' node must NOT carry `strong` mark; marks={marks}"
    );
}

// ---------------------------------------------------------------------------
// Call D — mixed range: surrounding strong retained; inner code stripped (EC-6)
// ---------------------------------------------------------------------------

/// H-NEW-ADF-010 Call D (BC-7.2.015 EC-6): `**a \`b\` c**` — mixed range.
///
/// Three text nodes from the `**a \`b\` c**` span:
///   - `"a "` (trailing space): marks include `{"type":"strong"}`.
///   - `"b"`: marks are `[{"type":"code"}]` — `strong` stripped by `push_code`.
///   - `" c"` (leading space): marks include `{"type":"strong"}`.
///
/// Pins VP-571-003: the allowlist filter clones `active_marks` per text node
/// rather than mutating a shared mark stack. A shared-stack mutation regression
/// would also strip `strong` from the surrounding `"a "` and `" c"` nodes.
#[tokio::test]
async fn test_bc_7_2_015_call_d_surrounding_strong_retained_inner_code_stripped_platform_path() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path());
    mount_platform_post(&server).await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "mixed-range",
            "--markdown",
            "--description",
            "**a `b` c**",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Call D: expected exit 0; stderr={stderr:?} stdout={stdout:?}"
    );

    let adf = capture_description_adf(&server).await;

    // Universal invariant: no code-marked text node carries a typographic mark.
    assert_code_mark_exclusivity(&adf);

    let mut text_nodes = Vec::new();
    collect_text_nodes(&adf, &mut text_nodes);

    // "a " — must carry `strong` (surrounding text retains marks; VP-571-003).
    let a_node = text_nodes
        .iter()
        .find(|n| n["text"].as_str() == Some("a "))
        .unwrap_or_else(|| panic!("Call D: expected text node 'a ' in ADF; adf={adf}"));
    let a_marks = &a_node["marks"];
    assert!(
        has_mark(a_marks, "strong"),
        "Call D: 'a ' node must carry `strong` mark (surrounding text retains marks, \
         VP-571-003); marks={a_marks}"
    );

    // "b" — code node: `strong` must be stripped by push_code.
    let b_node = text_nodes
        .iter()
        .find(|n| n["text"].as_str() == Some("b"))
        .unwrap_or_else(|| panic!("Call D: expected text node 'b' in ADF; adf={adf}"));
    let b_marks = &b_node["marks"];
    assert!(
        has_mark(b_marks, "code"),
        "Call D: 'b' node must carry `code` mark; marks={b_marks}"
    );
    assert!(
        !has_mark(b_marks, "strong"),
        "Call D: 'b' node must NOT carry `strong` mark (stripped by push_code allowlist \
         filter); marks={b_marks}"
    );

    // " c" — must carry `strong` (surrounding text retains marks; VP-571-003).
    let c_node = text_nodes
        .iter()
        .find(|n| n["text"].as_str() == Some(" c"))
        .unwrap_or_else(|| panic!("Call D: expected text node ' c' in ADF; adf={adf}"));
    let c_marks = &c_node["marks"];
    assert!(
        has_mark(c_marks, "strong"),
        "Call D: ' c' node must carry `strong` mark (surrounding text retains marks, \
         VP-571-003); marks={c_marks}"
    );
}

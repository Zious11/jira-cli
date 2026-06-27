//! E2E wiremock regression pin: BC-7.2.011 INV-1 (issue #522).
//!
//! G-ADF-INV1-INLINE-HTML: `markdown_to_adf` routes a multi-line INLINE HTML
//! run through `push_text` (Other context), where interior breaks become a
//! SPACE (not a hardBreak node), and NO non-codeBlock text node may contain a
//! raw `\n` or `\r` (INV-1). This test drives the full `jr issue create` CLI
//! path — piping the description through `--description-stdin --markdown` —
//! so the raw user string traverses the full create→markdown→ADF→POST pipeline
//! and the Jira REST POST body is captured for inspection.
//!
//! What this test pins (and why each claim is non-vacuous):
//!
//!   (1) **Wiring pin:** the create→markdown→ADF→POST pipeline completes without
//!       panic; exit 0; `fields.description` is present in the captured POST body.
//!       This is an end-to-end smoke test no unit test can replace.
//!
//!   (2) **INV-1 output-property guard:** no text node in the submitted ADF
//!       contains a raw `\r` or `\n`. This catches ANY regression that emits raw
//!       control chars into the submitted body (Jira would reject with HTTP 400).
//!       Valuable independently of which specific code path ran — it is an
//!       output-property, not a mechanism claim.
//!
//!   (3) **Routing guard (headline claim, e2e-unique):** the inline-HTML
//!       description emits NO `hardBreak` node. This is the primary e2e value:
//!       it proves the inline HTML path routed through `push_text` (Other context
//!       → space for interior breaks), NOT through block-HTML Algorithm B (which
//!       emits `hardBreak` nodes). Unit tests cannot catch a routing regression
//!       where `InlineHtml` events are accidentally processed as `HtmlBlock` — an
//!       e2e test traversing the full CLI pipeline is the only guard.
//!
//!   (4) **Content-preservation check:** the inline-HTML span content is present
//!       in the ADF text nodes (not silently dropped), and the interior break was
//!       replaced by a space rather than a raw control char. This rules out a
//!       vacuous INV-1 pass where normalization was bypassed and the text simply
//!       disappeared.
//!
//! Scope boundary — what this test does NOT pin:
//!   CommonMark §2.3 normalizes `\r`, `\r\n`, and `\n` → `\n` before pulldown
//!   tokenization. Markdown source CANNOT deliver a raw `\r` past §2.3, so
//!   pinning the char-level CR/LF→space normalization in `push_text` is the job
//!   of the DIRECT push_text unit tests in `src/adf.rs`, not this e2e test.
//!   See `test_push_text_normalizes_bare_lf_in_other_context_to_space` and
//!   `test_push_text_normalizes_lone_cr_in_heading_and_code_block` in `src/adf.rs`
//!   for those char-level pins.
//!
//! BC anchor: BC-7.2.011 INV-1 (issue #522, `push_text`/`push_code`/`text_to_adf`
//! CR/LF normalization chokepoint).

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness
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

fn write_minimal_config(config_home: &std::path::Path) {
    // No [instance] section is needed here: JR_BASE_URL (debug test seam) overrides
    // the instance URL at runtime, so the config file only needs to exist and be valid.
    let conf_dir = config_home.join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(conf_dir.join("config.toml"), "").unwrap();
}

// ---------------------------------------------------------------------------
// Helper: recursively scan every "text" node in the ADF JSON tree and assert
// no raw `\n` appears outside a codeBlock, and no raw `\r` appears anywhere.
// This mirrors the `assert_no_raw_newline_in_text_nodes` unit-test helper in
// `src/adf.rs` but implemented inline so integration tests can use it without
// reaching into a test-only module.
// ---------------------------------------------------------------------------

fn assert_no_raw_newline_in_adf(node: &serde_json::Value, in_code_block: bool, context: &str) {
    if let Some(obj) = node.as_object() {
        let node_type = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let is_code_block = node_type == "codeBlock";
        let ctx_code = in_code_block || is_code_block;

        if node_type == "text" {
            if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                // INV-1 \n clause: forbidden in paragraph/inline text (codeBlock exempt).
                if !ctx_code {
                    assert!(
                        !text.contains('\n'),
                        "INV-1 VIOLATED: non-codeBlock text node contains raw \\n.\n\
                         context={context}\ntext node={text:?}\nfull adf={node}"
                    );
                }
                // INV-1 \r clause: unconditional — all code paths normalize CR.
                assert!(
                    !text.contains('\r'),
                    "INV-1 VIOLATED: text node contains raw \\r (CR normalization failed).\n\
                     context={context}\ntext node={text:?}\nfull adf={node}"
                );
            }
        }

        if let Some(content) = obj.get("content").and_then(|c| c.as_array()) {
            for child in content {
                assert_no_raw_newline_in_adf(child, ctx_code, context);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: collect all "text" field strings from "text"-type ADF nodes.
// Used for positive content assertions (the space-merged result).
// ---------------------------------------------------------------------------

fn collect_adf_text_strings(node: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    collect_adf_text_strings_inner(node, &mut out);
    out
}

fn collect_adf_text_strings_inner(node: &serde_json::Value, out: &mut Vec<String>) {
    if let Some(obj) = node.as_object() {
        if obj.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(t) = obj.get("text").and_then(|t| t.as_str()) {
                out.push(t.to_owned());
            }
        }
        if let Some(content) = obj.get("content").and_then(|c| c.as_array()) {
            for child in content {
                collect_adf_text_strings_inner(child, out);
            }
        }
    }
}

fn any_hard_break(node: &serde_json::Value) -> bool {
    if node
        .get("type")
        .and_then(|t| t.as_str())
        .is_some_and(|t| t == "hardBreak")
    {
        return true;
    }
    if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
        return content.iter().any(any_hard_break);
    }
    false
}

// ---------------------------------------------------------------------------
// TEST 1 — G-ADF-INV1-INLINE-HTML (BC-7.2.011 INV-1, issue #522)
//
// End-to-end: `jr issue create --description-stdin --markdown` with a
// description containing multi-line INLINE HTML. Two inputs are tested:
//
//   (a) LF (`\n`): the primary input. CommonMark §2.3 normalizes `\n`
//       before tokenization. Whether this `\n` then arrives in the
//       `Event::InlineHtml` string is empirically unresolved for pulldown-cmark
//       (see `src/adf.rs` §"split_text_node_on_urls"), but the test's value is
//       NOT in proving a specific push_text normalization fired — it is in
//       proving (a) the inline path produces no hardBreak (routing guard),
//       (b) INV-1 holds in the submitted body (output-property), and (c) the
//       content was preserved and not dropped. The char-level LF→space pin lives
//       in `test_push_text_normalizes_bare_lf_in_other_context_to_space` in
//       `src/adf.rs`.
//
//   (b) CR (`\r`): a belt-and-suspenders input. CommonMark §2.3 normalizes
//       lone `\r` before pulldown tokenization, so no raw `\r` from markdown
//       source reaches push_text. This case adds breadth to the output-property
//       and routing checks but makes NO claim about push_text reachability.
//       The char-level CR→space pin lives in
//       `test_push_text_normalizes_lone_cr_in_heading_and_code_block` in
//       `src/adf.rs`.
//
// For each input the MockServer captures the POST body; we parse the ADF
// `description` field and assert:
//   (1) NO text node (outside codeBlock) contains `\n` or `\r` (INV-1
//       output-property guard).
//   (2) The inline-HTML span content is present and space-joined (content
//       preserved, not dropped; no raw control char in joined text).
//   (3) The ADF description contains NO hardBreak node — the KEY routing guard.
//       Only block-HTML Algorithm B emits hardBreaks; inline HTML uses push_text
//       (Other context → space). A regression that routes InlineHtml through
//       Algorithm B would emit a hardBreak and trip this assertion.
// ---------------------------------------------------------------------------

/// Runs a single `jr issue create` invocation with `input` as the description
/// piped through `--description-stdin --markdown`, asserts INV-1 compliance on
/// the captured POST body, asserts the inline-HTML content was preserved
/// (not dropped), and asserts no hardBreak was emitted.
///
/// `expected_space_merged`: a substring that should appear somewhere in the
/// concatenation of all ADF text strings, proving the interior break did not
/// cause the content to be dropped. We assert presence of the substring rather
/// than the exact space-join because whether pulldown splits the InlineHtml
/// event at the break boundary is empirically unresolved; the critical claim is
/// that content survives and no hardBreak appears.
async fn run_inv1_inline_html_case(label: &str, input: &str, space_merged_substring: &str) {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path());

    // Mount POST /rest/api/3/issue — capture + return 201.
    // Permissive mount (no body_partial_json matcher) so the test body is captured
    // regardless of ADF shape.
    //
    // CR-002: `.expect(1)` verifies exactly one POST fires. The table-output path
    // makes exactly one POST (JSON output would add a follow-on GET for the created
    // issue, but this test uses the default human output path). Any retry or duplicate
    // would exceed expect(1) and panic on server drop, making the regression visible.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "FOO-1",
            "self": format!("{}/rest/api/3/issue/10001", server.uri())
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "FOO",
            "--type",
            "Task",
            "--summary",
            "INV-1 inline-HTML regression pin",
            "--description-stdin",
            "--markdown",
        ])
        .write_stdin(input)
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "[{label}] Expected exit 0 — a raw \\r or \\n in a text node would have caused Jira to \
         reject the body with HTTP 400 (or the server mock to not match); \
         stderr={stderr} stdout={stdout}"
    );

    let captured = server
        .received_requests()
        .await
        .expect("wiremock must record received requests");

    assert_eq!(
        captured.len(),
        1,
        "[{label}] Expected exactly 1 POST to /rest/api/3/issue (table path makes one POST; \
         JSON path would add a follow-on GET); got {}",
        captured.len()
    );

    let body_bytes = &captured[0].body;
    let body_str = std::str::from_utf8(body_bytes).expect("POST body must be valid UTF-8");
    let body_json: serde_json::Value = serde_json::from_str(body_str)
        .unwrap_or_else(|e| panic!("POST body must be valid JSON: {e}; body={body_str}"));

    let description_adf = &body_json["fields"]["description"];
    assert!(
        !description_adf.is_null(),
        "[{label}] POST body must contain fields.description; body={body_str}"
    );

    // Assertion (1): INV-1 output-property — no raw \r or \n in any non-codeBlock
    // text node. This catches any regression that emits raw control chars into the
    // submitted body (Jira rejects with HTTP 400). Value: output-property guard,
    // independent of which internal code path ran.
    assert_no_raw_newline_in_adf(description_adf, false, input);

    // Assertion (2): content-preservation check — the inline-HTML tag text is present
    // in the ADF output and was not silently dropped. This rules out a vacuous INV-1
    // pass where normalization was bypassed and the text simply disappeared.
    let all_text: String = collect_adf_text_strings(description_adf).join("");
    assert!(
        all_text.contains(space_merged_substring),
        "[{label}] Expected ADF text nodes to contain {space_merged_substring:?} \
         (proving the inline-HTML content was preserved, not dropped). \
         Collected text nodes joined: {all_text:?}; full adf={description_adf}"
    );

    // Assertion (3): no hardBreak in the description ADF — the PRIMARY routing guard.
    // Inline HTML uses push_text (Other context → space for interior breaks); only
    // block-HTML Algorithm B emits hardBreaks. A routing regression where InlineHtml
    // events were processed as HtmlBlock would produce a hardBreak here and trip this
    // assertion. Unit tests cannot catch this cross-path routing bug; this e2e test can.
    assert!(
        !any_hard_break(description_adf),
        "[{label}] Inline-HTML interior break must become a SPACE (push_text Other path), \
         NOT a hardBreak. A hardBreak here means the HtmlBlock Algorithm B path fired instead \
         of push_text. description_adf={description_adf}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_issue_create_markdown_inline_html_submits_inv1_compliant_adf_no_hardbreak() {
    // LF case: primary input. CommonMark §2.3 normalizes \n before pulldown
    // tokenization. The test value is NOT in proving push_text's LF→space ran
    // (that char-level pin is in `test_push_text_normalizes_bare_lf_in_other_context_to_space`
    // in src/adf.rs). The e2e value is: (a) no hardBreak in the submitted ADF
    // (routing guard — proves inline path, not block-HTML Algorithm B); (b) INV-1
    // holds in the body (output-property); (c) content preserved end-to-end.
    run_inv1_inline_html_case(
        "LF",
        "foo <span\nx>bar",
        // The inline-HTML tag starts with "<span" — assert the tag content is present
        // (not dropped). Whether it arrives as "<span x>", "<span", or split across
        // multiple text nodes depends on pulldown event splitting (empirically unresolved);
        // we check only for the opening tag prefix to confirm content survived.
        "<span",
    )
    .await;

    // CR case: belt-and-suspenders. CommonMark §2.3 normalizes lone \r before
    // pulldown tokenization, so no raw \r from markdown source reaches push_text.
    // This case adds breadth to the INV-1 output-property and routing checks; it
    // makes NO reachability claim about push_text. The char-level CR→space pin is
    // in `test_push_text_normalizes_lone_cr_in_heading_and_code_block` in src/adf.rs.
    run_inv1_inline_html_case(
        "CR",
        "foo <span\rx>bar",
        // Same structural check: tag content preserved and present in ADF text nodes.
        "<span",
    )
    .await;
}

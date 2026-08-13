//! Integration tests for `issue edit --dry-run` stdin-read + ADF-preview
//! rendering (S-692-1, issue #692, DEC-274).
//!
//! BC-3.4.021 (STATUS: UPDATED, DEC-274): `--dry-run` now reads stdin for
//! `--description-stdin` and renders an ADF preview for ANY supplied
//! description input (`--description` or `--description-stdin`), using the
//! identical `markdown_to_adf`/`text_to_adf` selection the live path uses.
//! The rendered ADF is emitted as an additive `plannedChanges.descriptionAdf`
//! field (json mode) / a `"  description (ADF): rendered OK"` line (table
//! mode). A `markdown_to_adf` depth-guard `Err` (BC-7.2.012, `MAX_ADF_DEPTH`)
//! now exits 64 from the dry-run path too — this closes a false-OK regression
//! where a pathologically nested description under `--dry-run` returned exit
//! 0 while the corresponding live edit would exit 64.
//!
//! Every test in this file MUST fail before implementation (Red Gate): the
//! current `handle_edit` dry-run block never reads stdin for
//! `--description-stdin` (it emits a literal placeholder string) and never
//! calls `markdown_to_adf`/`text_to_adf` for either description-input flag,
//! so `plannedChanges.descriptionAdf` / the `"  description (ADF): rendered
//! OK"` line do not exist yet, and the depth guard is unreachable from
//! `--dry-run`.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;

use common::assertions::assert_json_error_envelope;

// ---------------------------------------------------------------------------
// Harness helpers — mirrors tests/issue_edit_echo.rs / tests/adf_recursion_depth.rs
// ---------------------------------------------------------------------------

/// Build a `jr` command wired to a mock server. No PUT is ever mounted in this
/// file — the dry-run path under test must never issue a mutation HTTP call,
/// so an accidental PUT would 404 and surface as a non-zero exit / different
/// error shape, failing the relevant assertion.
fn jr_cmd(server_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0");
    cmd
}

/// Build a depth-256 nested blockquote Markdown string that trips
/// `adf::markdown_to_adf`'s `MAX_ADF_DEPTH = 256` recursion-depth guard
/// (BC-7.2.012, CWE-674). Mirrors `tests/adf_recursion_depth.rs::deep_blockquote_markdown`.
fn deep_blockquote_markdown(depth: usize) -> String {
    let prefix = "> ".repeat(depth);
    format!("{prefix}leaf content")
}

/// Parse `stdout` as JSON, panicking with full context on failure.
fn parse_json(stdout: &str) -> serde_json::Value {
    serde_json::from_str(stdout)
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}; stdout={stdout}"))
}

/// Assert the parsed JSON object's top-level keys are exactly
/// `{dryRun, issues, plannedChanges}` — no more, no fewer (BC-3.4.021
/// Postconditions-json item 1, AC-10).
fn assert_exactly_three_top_level_keys(parsed: &serde_json::Value, label: &str) {
    let mut keys: Vec<&str> = parsed
        .as_object()
        .unwrap_or_else(|| panic!("{label}: top-level value is not a JSON object: {parsed}"))
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["dryRun", "issues", "plannedChanges"],
        "{label}: top-level JSON keys must be exactly {{dryRun, issues, plannedChanges}}; got: {parsed}"
    );
}

// ---------------------------------------------------------------------------
// AC-1 (BC-3.4.021 EC-3.4.021-6, VP-692-001): `--description-stdin` happy
// path, JSON mode.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_description_stdin_renders_adf_preview_json() {
    let server = wiremock::MockServer::start().await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description-stdin",
            "--dry-run",
            "--output",
            "json",
        ])
        .write_stdin("Fixed it")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    let parsed = parse_json(&stdout);
    assert_exactly_three_top_level_keys(&parsed, "AC-1");

    assert_eq!(
        parsed["plannedChanges"]["description"].as_str(),
        Some("Fixed it"),
        "plannedChanges.description must be the raw stdin string; stdout={stdout}"
    );

    let expected_adf = jr::adf::text_to_adf("Fixed it");
    assert_eq!(
        parsed["plannedChanges"]["descriptionAdf"], expected_adf,
        "plannedChanges.descriptionAdf must be byte-identical to adf::text_to_adf(\"Fixed it\"); stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-2 (BC-3.4.021 EC-3.4.021-15, VP-692-002): `--description-stdin`
// depth-guard error, JSON mode.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_description_stdin_depth_guard_exits_64_json_stdout_empty() {
    let server = wiremock::MockServer::start().await;

    let deep_md = deep_blockquote_markdown(256);

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description-stdin",
            "--markdown",
            "--dry-run",
            "--output",
            "json",
        ])
        .write_stdin(deep_md)
        .output()
        .unwrap();

    assert_json_error_envelope(&output, 64, "AC-2 (stdin depth-guard, json)");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let parsed: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert!(
        parsed.get("plannedChanges").is_none()
            && parsed.get("dryRun").is_none()
            && parsed.get("issues").is_none(),
        "No plannedChanges/dryRun/issues keys may appear anywhere on a depth-guard \
         error; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-3 (BC-3.4.021 EC-3.4.021-15, VP-692-002; AC-11 MANDATED ORDERING pin):
// `--description-stdin` depth-guard error, table mode.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_description_stdin_depth_guard_exits_64_table_stdout_empty() {
    let server = wiremock::MockServer::start().await;

    let deep_md = deep_blockquote_markdown(256);

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description-stdin",
            "--markdown",
            "--dry-run",
        ])
        .write_stdin(deep_md)
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64; stderr={stderr} stdout={stdout}"
    );

    // AC-11 / MANDATED ORDERING: no incremental table preview lines
    // ("DRY RUN — ...", "Issues affected", etc.) may leak to stdout before
    // the exit-64 return — this is the discriminating proof that the
    // read+conversion pre-step runs strictly before `match output_format`
    // begins printing.
    assert!(
        stdout.trim().is_empty(),
        "stdout must be EMPTY on a table-mode depth-guard error (no partial \
         preview leak); stdout={stdout}"
    );

    assert!(
        stderr.starts_with("Error: "),
        "stderr must carry 'Error: ...' in table (human) mode; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-4 (BC-3.4.021 EC-3.4.021-18, VP-692-003): bare `--description` happy
// path, JSON mode.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_bare_description_renders_adf_preview_json() {
    let server = wiremock::MockServer::start().await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description",
            "Fixed it",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    let parsed = parse_json(&stdout);
    assert_exactly_three_top_level_keys(&parsed, "AC-4");

    assert_eq!(
        parsed["plannedChanges"]["description"].as_str(),
        Some("Fixed it"),
        "plannedChanges.description must be the raw --description value; stdout={stdout}"
    );

    let expected_adf = jr::adf::text_to_adf("Fixed it");
    assert_eq!(
        parsed["plannedChanges"]["descriptionAdf"], expected_adf,
        "plannedChanges.descriptionAdf must be byte-identical to adf::text_to_adf(\"Fixed it\"); stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-5 (BC-3.4.021 EC-3.4.021-19, VP-692-004 — the exact false-OK regression
// this story closes): bare `--description` depth-guard error, JSON mode.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_bare_description_depth_guard_exits_64_json_stdout_empty() {
    let server = wiremock::MockServer::start().await;

    let deep_md = deep_blockquote_markdown(256);

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description",
            &deep_md,
            "--markdown",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert_json_error_envelope(&output, 64, "AC-5 (bare-description depth-guard, json)");
}

// ---------------------------------------------------------------------------
// AC-6 (BC-3.4.021 EC-3.4.021-19, VP-692-004): bare `--description`
// depth-guard error, table mode.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_bare_description_depth_guard_exits_64_table_stdout_empty() {
    let server = wiremock::MockServer::start().await;

    let deep_md = deep_blockquote_markdown(256);

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description",
            &deep_md,
            "--markdown",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64; stderr={stderr} stdout={stdout}"
    );

    assert!(
        stdout.trim().is_empty(),
        "stdout must be EMPTY on a table-mode depth-guard error; stdout={stdout}"
    );

    assert!(
        stderr.starts_with("Error: "),
        "stderr must carry 'Error: ...' in table (human) mode; stderr={stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-7 (BC-3.4.021 EC-3.4.021-17): empty stdin edge case.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_empty_stdin_produces_empty_description_and_valid_adf() {
    let server = wiremock::MockServer::start().await;

    // JSON mode.
    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description-stdin",
            "--dry-run",
            "--output",
            "json",
        ])
        .write_stdin("")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for empty stdin; stderr={stderr} stdout={stdout}"
    );

    let parsed = parse_json(&stdout);
    assert_eq!(
        parsed["plannedChanges"]["description"].as_str(),
        Some(""),
        "plannedChanges.description must be present and empty (not absent/null); stdout={stdout}"
    );

    let expected_adf = jr::adf::text_to_adf("");
    assert_eq!(
        parsed["plannedChanges"]["descriptionAdf"], expected_adf,
        "plannedChanges.descriptionAdf must equal adf::text_to_adf(\"\"); stdout={stdout}"
    );

    // Table mode.
    let table_output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description-stdin",
            "--dry-run",
        ])
        .write_stdin("")
        .output()
        .unwrap();

    let table_stderr = String::from_utf8_lossy(&table_output.stderr);
    let table_stdout = String::from_utf8_lossy(&table_output.stdout);

    assert!(
        table_output.status.success(),
        "Expected exit 0 for empty stdin (table mode); stderr={table_stderr} stdout={table_stdout}"
    );

    assert!(
        table_stdout.contains("  description → "),
        "Table stdout must contain the empty description preview line; stdout={table_stdout}"
    );
    assert!(
        table_stdout.contains("  description (ADF): rendered OK"),
        "Table stdout must contain the render-OK line even for empty stdin; stdout={table_stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-8 (BC-3.4.021 EC-3.4.021-16): multi-line Markdown stdin round-trip.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_multiline_markdown_stdin_produces_real_adf_document() {
    let server = wiremock::MockServer::start().await;

    let multiline_md = "- item one\n- item two\n\n```\nfn main() {}\n```\n";

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description-stdin",
            "--markdown",
            "--dry-run",
            "--output",
            "json",
        ])
        .write_stdin(multiline_md)
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    let parsed = parse_json(&stdout);

    // The raw multi-line stdin string must appear verbatim, including
    // embedded newlines — this is a bare JSON string, not an ADF text node,
    // so the newline-in-text-node prohibition does not apply here.
    assert_eq!(
        parsed["plannedChanges"]["description"].as_str(),
        Some(multiline_md),
        "plannedChanges.description must be the raw multi-line stdin string verbatim; stdout={stdout}"
    );

    let expected_adf = jr::adf::markdown_to_adf(multiline_md).unwrap();
    assert_eq!(
        parsed["plannedChanges"]["descriptionAdf"], expected_adf,
        "plannedChanges.descriptionAdf must equal the full markdown_to_adf output; stdout={stdout}"
    );

    // Sanity: the real ADF document contains bulletList/codeBlock nodes, not
    // a placeholder or a flattened string.
    let adf_str = serde_json::to_string(&parsed["plannedChanges"]["descriptionAdf"]).unwrap();
    assert!(
        adf_str.contains("bulletList"),
        "descriptionAdf must contain a bulletList node; adf={adf_str}"
    );
    assert!(
        adf_str.contains("codeBlock"),
        "descriptionAdf must contain a codeBlock node; adf={adf_str}"
    );
}

// ---------------------------------------------------------------------------
// AC-9 (BC-3.4.021 Postconditions-table items 1-2, EC-3.4.021-7/-13):
// table-mode render-OK line unconditional on truncation, plus pinned
// ordering vs. "markdown rendering: enabled".
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_table_render_ok_line_unconditional_on_truncation_and_ordering() {
    let server = wiremock::MockServer::start().await;

    // Case 1: 61 codepoints — truncated with "..." suffix, PLUS the
    // render-OK line unconditionally.
    let long_desc = "a".repeat(61);
    let output_long = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description",
            &long_desc,
            "--dry-run",
        ])
        .output()
        .unwrap();
    let stdout_long = String::from_utf8_lossy(&output_long.stdout);
    assert!(
        output_long.status.success(),
        "Expected exit 0; stdout={stdout_long}"
    );
    let expected_truncated = format!("  description → {}...", "a".repeat(60));
    assert!(
        stdout_long.contains(&expected_truncated),
        "61-codepoint description must be truncated to 60 chars + '...'; stdout={stdout_long}"
    );
    assert!(
        stdout_long.contains("  description (ADF): rendered OK"),
        "Render-OK line must appear even when truncation fired; stdout={stdout_long}"
    );

    // Case 2: exactly 60 codepoints — NOT truncated, but render-OK line
    // still appears.
    let exact_desc = "a".repeat(60);
    let output_exact = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description",
            &exact_desc,
            "--dry-run",
        ])
        .output()
        .unwrap();
    let stdout_exact = String::from_utf8_lossy(&output_exact.stdout);
    assert!(
        output_exact.status.success(),
        "Expected exit 0; stdout={stdout_exact}"
    );
    let expected_exact_line = format!("  description → {exact_desc}");
    assert!(
        stdout_exact.contains(&expected_exact_line)
            && !stdout_exact.contains(&format!("{exact_desc}...")),
        "Exactly-60-codepoint description must NOT be truncated; stdout={stdout_exact}"
    );
    assert!(
        stdout_exact.contains("  description (ADF): rendered OK"),
        "Render-OK line must appear for an untruncated description too; stdout={stdout_exact}"
    );

    // Case 3: pinned ordering — with BOTH --markdown and a description
    // input, "markdown rendering: enabled" precedes "description (ADF):
    // rendered OK".
    let output_order = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description",
            "Fixed it",
            "--markdown",
            "--dry-run",
        ])
        .output()
        .unwrap();
    let stdout_order = String::from_utf8_lossy(&output_order.stdout);
    assert!(
        output_order.status.success(),
        "Expected exit 0; stdout={stdout_order}"
    );
    let markdown_idx = stdout_order
        .find("markdown rendering: enabled")
        .unwrap_or_else(|| {
            panic!("stdout must contain the markdown-enabled line; stdout={stdout_order}")
        });
    let render_ok_idx = stdout_order
        .find("description (ADF): rendered OK")
        .unwrap_or_else(|| panic!("stdout must contain the render-OK line; stdout={stdout_order}"));
    assert!(
        markdown_idx < render_ok_idx,
        "'markdown rendering: enabled' must precede 'description (ADF): rendered OK'; stdout={stdout_order}"
    );
}

// ---------------------------------------------------------------------------
// AC-10 (BC-3.4.021 Postconditions-json item 1): three-top-level-key
// invariant preserved for any description-input-flag combination.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_json_top_level_key_count_preserved_with_description_adf() {
    let server = wiremock::MockServer::start().await;

    // Combo 1: bare --description, no --markdown.
    let out1 = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description",
            "Fixed it",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    assert!(out1.status.success());
    assert_exactly_three_top_level_keys(
        &parse_json(&String::from_utf8_lossy(&out1.stdout)),
        "AC-10 combo 1 (bare --description)",
    );

    // Combo 2: bare --description + --markdown.
    let out2 = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description",
            "Fixed it",
            "--markdown",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    assert!(out2.status.success());
    assert_exactly_three_top_level_keys(
        &parse_json(&String::from_utf8_lossy(&out2.stdout)),
        "AC-10 combo 2 (--description + --markdown)",
    );

    // Combo 3: --description-stdin, no --markdown.
    let out3 = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description-stdin",
            "--dry-run",
            "--output",
            "json",
        ])
        .write_stdin("Fixed it")
        .output()
        .unwrap();
    assert!(out3.status.success());
    assert_exactly_three_top_level_keys(
        &parse_json(&String::from_utf8_lossy(&out3.stdout)),
        "AC-10 combo 3 (--description-stdin)",
    );

    // Combo 4: --description-stdin + --markdown.
    let out4 = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--description-stdin",
            "--markdown",
            "--dry-run",
            "--output",
            "json",
        ])
        .write_stdin("Fixed it")
        .output()
        .unwrap();
    assert!(out4.status.success());
    assert_exactly_three_top_level_keys(
        &parse_json(&String::from_utf8_lossy(&out4.stdout)),
        "AC-10 combo 4 (--description-stdin + --markdown)",
    );
}

// ---------------------------------------------------------------------------
// AC-12 (BC-3.4.021 Invariant 1, description exception): live-wire
// byte-identity is the ONE exception; other dry-run preview fields remain
// intentionally simplified, unaffected by this story's fix.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bc_3_4_021_dry_run_other_fields_remain_simplified_previews_unaffected_by_description_fix()
 {
    let server = wiremock::MockServer::start().await;

    // --label is mutually exclusive with --summary/--priority/--type/
    // --description on one key (Gate B, BC-3.4.017) — exercise the
    // summary/priority/type/description combo and the label preview
    // separately.
    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--summary",
            "New title",
            "--priority",
            "High",
            "--type",
            "Bug",
            "--description",
            "Fixed it",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0; stderr={stderr} stdout={stdout}"
    );

    let parsed = parse_json(&stdout);
    let planned = &parsed["plannedChanges"];

    // Simplified previews, unchanged by this story: bare strings, NOT the
    // live wire shapes.
    assert_eq!(planned["summary"].as_str(), Some("New title"));
    assert_eq!(
        planned["priority"].as_str(),
        Some("High"),
        "priority must remain a bare string, NOT {{\"priorityId\":...}}; planned={planned}"
    );
    assert_eq!(
        planned["issueType"].as_str(),
        Some("Bug"),
        "issueType must remain a bare string, NOT {{\"issueTypeId\":...}}; planned={planned}"
    );

    // The ONE exception: descriptionAdf IS byte-identical to the live payload.
    let expected_adf = jr::adf::text_to_adf("Fixed it");
    assert_eq!(planned["descriptionAdf"], expected_adf);

    // Labels preview (checked separately — --label cannot share a call with
    // --description under Gate B): still the flat-array preview, NOT
    // labelsFields.
    let label_output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--label",
            "add:foo",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    let label_stderr = String::from_utf8_lossy(&label_output.stderr);
    let label_stdout = String::from_utf8_lossy(&label_output.stdout);
    assert!(
        label_output.status.success(),
        "Expected exit 0; stderr={label_stderr} stdout={label_stdout}"
    );
    let label_parsed = parse_json(&label_stdout);
    let label_planned = &label_parsed["plannedChanges"];
    assert_eq!(
        label_planned["labels"],
        serde_json::json!([{"action": "ADD", "name": "foo"}]),
        "labels must remain the flat-array preview, NOT labelsFields; stdout={label_stdout}"
    );

    // Negative pin: no description input flag was supplied on this call, so
    // NEITHER `description` NOR `descriptionAdf` may appear in plannedChanges
    // (the derived-key absence direction of BC-3.4.021 Postconditions-json
    // item 2 — descriptionAdf is present IFF a description input is supplied).
    assert!(
        label_planned.get("descriptionAdf").is_none() && label_planned.get("description").is_none(),
        "neither description nor descriptionAdf may appear when no description flag was supplied; planned={label_planned:?}"
    );
}

// ---------------------------------------------------------------------------
// AC-13 (BC-3.4.021 Invariant 6): no `--file` flag regression pin.
// ---------------------------------------------------------------------------

#[test]
fn test_bc_3_4_021_issue_edit_help_has_no_file_flag() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "edit", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "jr issue edit --help must exit 0; stdout={stdout}"
    );
    assert!(
        !stdout.contains("--file"),
        "jr issue edit --help must NOT advertise a --file flag; stdout={stdout}"
    );
}

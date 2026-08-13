//! Red Gate tests for S-694-1 — attachment help-text/doc-comment sync (#694).
//!
//! DOCS-ONLY story: these tests assert on `--help` output substrings only.
//! They pin help-text content, not runtime behavior — the underlying
//! behavior (BC-2.7.008/009/010) is already correct and unchanged by this
//! story; only the clap doc-comment strings in `src/cli/mod.rs` are stale.
//!
//! Each assertion below is scoped to the *specific* help-text region the
//! story's doc-comment edit targets (the parent `about` text, or a single
//! `--flag`'s own help block) rather than the whole `--help` stdout. This
//! matters because e.g. "download"/"upload"/"delete" already appear in the
//! `Commands:` subcommand listing regardless of the parent `about` text, and
//! "created" already appears in the pre-fix `--newest` help — a whole-stdout
//! substring search would find those and produce a false-green before the
//! doc fix landed. See `about_text`/`option_block` below.

use assert_cmd::Command;

/// Runs `jr issue attachment --help` and returns captured stdout as a String.
fn attachment_help_stdout() -> String {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "attachment", "--help"])
        .output()
        .expect("failed to run `jr issue attachment --help`");
    assert!(
        output.status.success(),
        "`jr issue attachment --help` did not exit successfully"
    );
    String::from_utf8(output.stdout).expect("stdout is not valid UTF-8")
}

/// Runs `jr issue attachment download --help` and returns captured stdout as a String.
fn attachment_download_help_stdout() -> String {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "attachment", "download", "--help"])
        .output()
        .expect("failed to run `jr issue attachment download --help`");
    assert!(
        output.status.success(),
        "`jr issue attachment download --help` did not exit successfully"
    );
    String::from_utf8(output.stdout).expect("stdout is not valid UTF-8")
}

/// Extracts the parent `about` text — everything before the `Usage:` line —
/// from a `--help` stdout capture. Isolates AC-1's assertion to the
/// clap-derive doc-comment on `IssueCommand::Attachment` itself (the string
/// this story edits), not the `Commands:` subcommand listing further down
/// (which already names each subcommand today and would make an
/// unscoped assertion pass before the doc fix).
fn about_text(stdout: &str) -> &str {
    let idx = stdout
        .find("Usage:")
        .expect("no 'Usage:' marker found in --help output");
    &stdout[..idx]
}

/// Extracts one flag's help-text block from a `--help` stdout capture,
/// scoped between its own `      --<flag> ...` header line and the next
/// flag's header line. Isolates AC-2/AC-3 assertions to exactly the
/// `--out-dir`/`--newest` doc-comment text this story edits, not the whole
/// `download --help` output (which contains unrelated matches, e.g.
/// "created" already appears in the pre-fix `--newest` block itself, and a
/// wider search could also match text belonging to a neighboring flag).
///
/// `header`/`next_header` must be the bare `--flag <VALUE>` text; this
/// function anchors the search to the six-space `Options:`-list indent
/// (`"      --flag ..."`) so it does NOT match the same flag name inside the
/// `Usage: ... <--id <ID>|--all|--newest <NEWEST>>` synopsis line, which
/// appears earlier in the same stdout and would otherwise be matched first.
fn option_block<'a>(stdout: &'a str, header: &str, next_header: &str) -> &'a str {
    let anchored_header = format!("      {header}");
    let start = stdout.find(&anchored_header).unwrap_or_else(|| {
        panic!("no '{anchored_header}' marker found in --help output:\n{stdout}")
    });
    let rest = &stdout[start..];
    let anchored_next = format!("      {next_header}");
    let end = rest.find(&anchored_next).unwrap_or_else(|| {
        panic!("no '{anchored_next}' marker found after '{anchored_header}' in --help output:\n{stdout}")
    });
    &rest[..end]
}

/// AC-1 (BC-2.7.008): the parent `about` string on `jr issue attachment --help`
/// must enumerate all four subcommands (list, download, upload, delete) —
/// not merely list them in the `Commands:` usage table further down.
///
/// RED today: the current about string is
/// `"Attachment operations: list. (S-576-1)"` — it names only `list`.
#[test]
fn test_bc_2_7_008_attachment_help_about_enumerates_all_four_subcommands() {
    let stdout = attachment_help_stdout();
    let about = about_text(&stdout);

    assert!(
        about.contains("list"),
        "parent `about` text should mention 'list':\n{about}"
    );
    assert!(
        about.contains("download"),
        "parent `about` text should mention 'download':\n{about}"
    );
    assert!(
        about.contains("upload"),
        "parent `about` text should mention 'upload':\n{about}"
    );
    assert!(
        about.contains("delete"),
        "parent `about` text should mention 'delete':\n{about}"
    );
}

/// AC-2 (BC-2.7.010): the `--out-dir` help text on
/// `jr issue attachment download --help` must document the batch on-disk
/// naming scheme — `<40-char-SHA-1-of-attachment-id>_<sanitized-filename>` —
/// and that the JSON manifest's `path` field is how callers recover the
/// actual on-disk name (the name is not otherwise predictable from `list`).
///
/// RED today: the current `--out-dir` help text says only "Output directory
/// for batch downloads. Requires the `batch` group... Conflicts with
/// `--id`" — no mention of SHA-1 or the manifest `path` field.
#[test]
fn test_bc_2_7_010_attachment_download_help_out_dir_documents_sha1_naming_scheme() {
    let stdout = attachment_download_help_stdout();
    let block = option_block(&stdout, "--out-dir <OUT_DIR>", "--filter <FILTER>");
    let block_lower = block.to_ascii_lowercase();

    assert!(
        block_lower.contains("sha-1") || block_lower.contains("sha1"),
        "--out-dir help should document the SHA-1 batch naming scheme:\n{block}"
    );
    assert!(
        block.contains("path"),
        "--out-dir help should mention the JSON manifest's `path` field for recovering the on-disk name:\n{block}"
    );
}

/// AC-3 (BC-2.7.009): the `--newest` help text on
/// `jr issue attachment download --help` must document that `--filter`
/// predicates are applied BEFORE `--newest` truncation, and that the
/// surviving set is sorted by `created` (most recent first) before
/// truncation to N.
///
/// RED today: the current `--newest` help text describes only the N-most-
/// recent-by-`created` selection itself and its mutual-exclusion group — it
/// never mentions `--filter` or that filtering happens first.
#[test]
fn test_bc_2_7_009_attachment_download_help_newest_documents_filter_then_sort_order() {
    let stdout = attachment_download_help_stdout();
    let block = option_block(&stdout, "--newest <NEWEST>", "--out <OUT>");
    let block_lower = block.to_ascii_lowercase();

    assert!(
        block.contains("--filter"),
        "--newest help should mention that `--filter` is applied first:\n{block}"
    );
    assert!(
        block_lower.contains("before"),
        "--newest help should state `--filter` is applied BEFORE `--newest` truncation:\n{block}"
    );
    assert!(
        block.contains("created"),
        "--newest help should mention sorting by `created`:\n{block}"
    );
}

/// AC-5 regression pin (documentation only — not a substitute for `cargo
/// test`'s full run, which is the actual AC-5 gate per the story). This
/// story's `files_modified` is scoped to `src/cli/mod.rs` doc-comment
/// strings only; `src/cli/issue/attachments.rs` (batch naming, filter/sort/
/// truncate logic, single-file path) is read-only reference material and
/// MUST NOT be touched. There is no runtime behavior to assert here — this
/// test exists as a standing reminder of that scope boundary and always
/// passes.
#[test]
fn test_attachment_help_text_story_is_docs_only_and_touches_no_attachment_logic() {
    // Intentionally no assertions against src/cli/issue/attachments.rs.
    // AC-4 (BC bodies byte-identical) and AC-5 (full attachment suite green)
    // are enforced at PR review / `cargo test` time, per the story's own
    // "Test: manual/PR-review gate" and "Test: cargo test full suite green"
    // notes — not by a runtime assertion in this file.
}

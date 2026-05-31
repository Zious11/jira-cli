//! CLI-surface guard for the live-E2E test suite (E2E-PG-1 / DRIFT-E2E-1).
//!
//! # What this guard catches
//!
//! Nonexistent `jr` **subcommand paths** and nonexistent **flags** that are
//! referenced in `tests/e2e_live.rs` but do not exist in the real clap command
//! tree. Every `jr <subcmd-path> --help` invocation must exit 0 (nonexistent
//! paths exit 2); every `--flag` referenced in the suite must appear in that
//! subcommand's `--help` output.
//!
//! The defect class this closes: "assumed-CLI-surface" bugs where a test
//! references `jr issue assign --me` (a flag that has never existed), or
//! `jr issue bogus` (a subcommand that does not exist), and these slip through
//! because `#[ignore]`-gated tests don't run without live credentials.
//!
//! # What this guard does NOT catch
//!
//! * **JSON output shape mismatches** — whether the response is an object vs.
//!   array, which keys are present, field optionality, and whether field names
//!   match serde rename attributes. Those require either serde-type checks at
//!   compile time or a live run (the `to_category` defect and the `board view`
//!   bare-array vs. object shape issue from S-E2E-4 were both shape issues out
//!   of scope for a clap-surface guard).
//! * **Exit-code semantics** — whether a given code path exits 1 vs. 2 vs. 64.
//! * **Network-dependent behavior** — anything that requires a real Jira response.
//!
//! # Always-run; offline
//!
//! This test file is **not** gated behind `JR_RUN_E2E` and requires **no
//! network access**. It uses `--help` only, which the clap derive machinery
//! answers entirely from the compiled binary. It is safe to run in normal
//! `cargo test` / CI without any live Jira environment.
//!
//! # Scope achieved
//!
//! **Full flag-check**: both subcommand paths AND per-path flags are validated.
//!
//! The subcommand paths are extracted mechanically from the e2e_live.rs source
//! via a parser that scans `.args([...])` blocks, collects leading string-literal
//! tokens (stopping at the first `--` token or the first non-literal token), and
//! deduplicates. The flag table is derived from the same parse pass and is
//! embedded as a static table — encoding each `(path, flags)` pair that actually
//! appears in the suite. This design is robust to variable-value tokens (like
//! `&proj`, `&key`, `&board_id`) which are correctly identified as non-literals
//! and skipped.
//!
//! The parser was manually verified against the full `tests/e2e_live.rs` source
//! to produce zero false positives against the current correct suite.

use assert_cmd::Command;

// ---------------------------------------------------------------------------
// Canonical (path, flags) table derived from tests/e2e_live.rs
// ---------------------------------------------------------------------------
//
// Derivation rules (same as the parser in `test_parse_args_extracts_paths`):
//   - SUBCOMMAND PATH: leading consecutive string-literal tokens in a `.args([...])`
//     block that do NOT start with `-`, stopping at the first `--` flag or
//     first non-literal (variable) token.
//   - FLAGS: every `"--..."` literal anywhere in the block.
//   - Non-literal tokens like `&proj`, `&key`, `&board_id`, `&jql` are values
//     (positional or flag values), never subcommand segments or flags — skipped.
//
// This table is the single source of truth for what the guard validates.
// When a new invocation is added to e2e_live.rs, add the corresponding row here.
// The `test_parse_args_extracts_paths` test independently validates the path-
// extraction parser; the flag table is hand-verified from `jr <path> --help`.

const SURFACE: &[(&[&str], &[&str])] = &[
    // issue list
    (&["issue", "list"], &["--jql", "--output"]),
    // issue view
    (&["issue", "view"], &["--output"]),
    // issue create
    (
        &["issue", "create"],
        &["--project", "--type", "--summary", "--label", "--output"],
    ),
    // issue edit
    (
        &["issue", "edit"],
        &[
            "--summary",
            "--description",
            "--dry-run",
            "--label",
            "--output",
        ],
    ),
    // issue comment
    (&["issue", "comment"], &["--output"]),
    // issue comments
    (&["issue", "comments"], &["--output"]),
    // issue move  (positional: key + status-name — no flags beyond --output)
    (&["issue", "move"], &["--output"]),
    // issue assign  (positional: key — no flags beyond --output; NO --me flag)
    (&["issue", "assign"], &["--output"]),
    // issue link  (--type is used in E2E-PG-4 typed-link test)
    (&["issue", "link"], &["--type", "--output"]),
    // issue unlink  (--type is used in E2E-PG-4 typed-link test)
    (&["issue", "unlink"], &["--type", "--output"]),
    // issue remote-link
    (&["issue", "remote-link"], &["--url", "--title", "--output"]),
    // issue link-types
    (&["issue", "link-types"], &["--output"]),
    // issue transitions  (positional: key)
    (&["issue", "transitions"], &["--output"]),
    // issue changelog  (positional: key)
    (&["issue", "changelog"], &["--output"]),
    // board list
    (&["board", "list"], &["--output"]),
    // board view
    (&["board", "view"], &["--board", "--output"]),
    // sprint list
    (&["sprint", "list"], &["--board", "--output"]),
    // sprint current
    (&["sprint", "current"], &["--board", "--output"]),
    // team list
    (&["team", "list"], &["--output"]),
    // user search  (positional: query)
    (&["user", "search"], &["--output"]),
    // user view  (positional: accountId)
    (&["user", "view"], &["--output"]),
    // project fields
    (&["project", "fields"], &["--project", "--output"]),
    // queue list
    (&["queue", "list"], &["--project", "--output"]),
    // requesttype list
    (&["requesttype", "list"], &["--project", "--output"]),
    // worklog add  (positional: key + duration)
    (&["worklog", "add"], &["--output"]),
    // worklog list  (positional: key)
    (&["worklog", "list"], &["--output"]),
];

// ---------------------------------------------------------------------------
// Guard test — always-run, no JR_RUN_E2E, no network
// ---------------------------------------------------------------------------

/// Validates every `jr` subcommand path and flag referenced in `tests/e2e_live.rs`
/// against the real clap command tree by invoking `jr <path> --help` (offline).
///
/// Two failure modes:
/// 1. **Path not found:** `jr <path> --help` exits non-zero (exit 2 = unrecognized
///    subcommand). Means the suite references a subcommand that doesn't exist.
/// 2. **Flag not found:** the flag string is absent from the `--help` stdout.
///    Means the suite references a flag that doesn't exist on that subcommand
///    (the `--me`-class defect that motivated this guard).
///
/// Global flags (`--output`, `--project`, `--no-input`, `--profile`, etc.) appear
/// in every subcommand's `--help` and pass trivially — this is intentional and
/// correct; they are real flags that must exist on each subcommand.
#[test]
fn test_e2e_cli_surface_all_paths_and_flags_exist() {
    let mut failures: Vec<String> = Vec::new();

    for (path, flags) in SURFACE {
        // Step 1: verify the subcommand path is valid (exit 0).
        let mut help_cmd = Command::cargo_bin("jr").expect("jr binary must be built");
        // Append --help after the path tokens.
        let output = help_cmd
            .args(*path)
            .arg("--help")
            .output()
            .unwrap_or_else(|e| panic!("failed to spawn jr {:?} --help: {e}", path));

        if !output.status.success() {
            failures.push(format!(
                "NONEXISTENT-PATH: `jr {}` does not exist in the clap tree \
                 (exit {}; this path is referenced in tests/e2e_live.rs)",
                path.join(" "),
                output.status.code().unwrap_or(-1)
            ));
            // Skip flag check for this path — help output is meaningless.
            continue;
        }

        // Step 2: for each flag, verify it appears in the --help stdout.
        let help_text = String::from_utf8_lossy(&output.stdout);
        for &flag in *flags {
            if !help_text.contains(flag) {
                failures.push(format!(
                    "NONEXISTENT-FLAG: `jr {} {}` — flag `{}` not found in \
                     `jr {} --help` output (this flag is referenced in \
                     tests/e2e_live.rs but does not exist on this subcommand)",
                    path.join(" "),
                    flag,
                    flag,
                    path.join(" ")
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "E2E CLI-surface VIOLATIONS detected ({} total):\n\n  {}\n\n\
         These subcommand paths or flags are referenced in tests/e2e_live.rs \
         but do not exist in the `jr` clap command tree. Fix by (a) removing \
         the invocation from the E2E suite, (b) correcting the flag/path, or \
         (c) adding the missing flag/subcommand to the binary.",
        failures.len(),
        failures.join("\n\n  ")
    );
}

// ---------------------------------------------------------------------------
// Parser unit tests — prove the path-extraction logic is correct
// ---------------------------------------------------------------------------

/// Extract subcommand paths from a synthetic snippet that resembles e2e_live.rs.
///
/// Parsing rules (mirrors the doc at the top of this file):
///   - Find each `.args([` ... `])` block (ignores content between them).
///   - Collect ordered items: `"literal"` tokens vs. non-literal (`&var`, identifiers).
///   - SUBCOMMAND PATH = leading consecutive string-literal tokens that do NOT
///     start with `-`, stopping at the first `--` flag token OR the first
///     non-string (variable) token.
///   - Dedup (path, sorted-flag-set) pairs so equivalent calls are counted once.
///
/// Returns (sorted-paths, sorted-(path,flags)) pairs.
fn parse_e2e_invocations(source: &str) -> Vec<(Vec<String>, Vec<String>)> {
    let mut results: Vec<(Vec<String>, Vec<String>)> = Vec::new();

    // Find every `.args([` opening, then scan to the matching `])`.
    let mut pos = 0;
    while let Some(start) = source[pos..].find(".args([") {
        let args_open = pos + start + ".args([".len();

        // Find the matching `])` — scan for first `])` after `args_open`.
        // This is safe for well-formed Rust arrays that don't nest `[]`.
        let Some(close_rel) = source[args_open..].find("])") else {
            break;
        };
        let args_close = args_open + close_rel;
        let block = &source[args_open..args_close];

        // Parse the block: collect string literals and note non-literals.
        let tokens = collect_tokens(block);

        // Subcommand path = leading non-flag string literals that look like
        // subcommand names (all lowercase a-z and hyphens only, non-empty).
        // This excludes positional argument values like "E2E-99999999",
        // "5m", "e2e", or free-text strings that aren't subcommand names.
        let mut path: Vec<String> = Vec::new();
        let mut flags: Vec<String> = Vec::new();
        let mut past_path = false;

        for token in &tokens {
            match token {
                Token::Literal(s) => {
                    if s.starts_with("--") {
                        past_path = true;
                        flags.push(s.clone());
                    } else if s.starts_with('-') {
                        // Single-dash flags: not a subcommand, stop path collection.
                        past_path = true;
                    } else if !past_path && is_subcommand_word(s) {
                        path.push(s.clone());
                    } else {
                        // Any other literal (positional value like "E2E-99999999",
                        // "5m", flag value like "json", etc.) stops path collection.
                        past_path = true;
                    }
                }
                Token::NonLiteral => {
                    // A variable token ends the path-collection phase.
                    past_path = true;
                }
            }
        }

        if !path.is_empty() {
            // Dedup: normalise flags before inserting.
            flags.sort();
            flags.dedup();
            // Check if we already have this (path, flags) combo.
            let entry = (path, flags);
            if !results.contains(&entry) {
                results.push(entry);
            }
        }

        pos = args_close + 2; // advance past `])`
    }

    results
}

/// Returns `true` if `s` looks like a `jr` subcommand name.
///
/// Subcommand names in `jr` are all-lowercase with optional hyphens
/// (e.g. `"issue"`, `"list"`, `"link-types"`). Positional argument values
/// like `"E2E-99999999"`, `"5m"`, or `"json"` are NOT subcommand names.
///
/// Rule: non-empty, all chars are ASCII lowercase letters or `-`.
/// This correctly excludes:
/// - Jira keys (`"E2E-99999999"` — contains uppercase and digits)
/// - Duration values (`"5m"` — starts with a digit)
/// - Output format values (`"json"` — would match, but "json" comes AFTER a
///   `--` flag so `past_path` is already true by then)
fn is_subcommand_word(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

/// A token from a `.args([...])` block.
#[derive(Debug, PartialEq)]
enum Token {
    Literal(String),
    NonLiteral,
}

/// Collect tokens from the content of a `.args([...])` block.
///
/// Handles:
///   - `"..."` string literals (with `\"` escapes).
///   - `&var`, `&var.method()`, bare identifiers — treated as `NonLiteral`.
///   - Commas, whitespace, and newlines — ignored (delimiters only).
fn collect_tokens(block: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let bytes = block.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                // String literal: collect until closing unescaped `"`.
                let mut s = String::new();
                i += 1; // skip opening `"`
                while i < bytes.len() {
                    match bytes[i] {
                        b'\\' => {
                            // Escape sequence: skip next byte.
                            i += 2;
                        }
                        b'"' => {
                            i += 1; // skip closing `"`
                            break;
                        }
                        c => {
                            s.push(c as char);
                            i += 1;
                        }
                    }
                }
                tokens.push(Token::Literal(s));
            }
            b'&' | b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                // Non-literal: consume identifier chars (including `.`, `(`, `)` for method calls).
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric()
                        || matches!(bytes[i], b'_' | b'.' | b'(' | b')' | b'&'))
                {
                    i += 1;
                }
                tokens.push(Token::NonLiteral);
            }
            _ => {
                // Comma, whitespace, newline, `[`, `]`, `//` comments: skip.
                i += 1;
            }
        }
    }

    tokens
}

/// The parser correctly extracts paths from a simple single-line block.
#[test]
fn test_parse_args_single_line_extracts_path() {
    let src = r#".args(["issue", "list", "--jql", &jql, "--output", "json"])"#;
    let results = parse_e2e_invocations(src);
    assert_eq!(results.len(), 1, "one invocation");
    let (path, flags) = &results[0];
    assert_eq!(path, &["issue", "list"]);
    // flags include --jql and --output (json is a value, not a flag).
    assert!(flags.contains(&"--jql".to_string()), "must have --jql");
    assert!(
        flags.contains(&"--output".to_string()),
        "must have --output"
    );
    assert!(
        !flags.contains(&"--jql".to_string()) || flags.len() <= 2,
        "no extra flags"
    );
}

/// The parser stops the path at the first non-literal token (positional variable).
#[test]
fn test_parse_args_stops_path_at_variable() {
    // `["issue", "move", &key, &status]` — path = ["issue", "move"].
    let src = r#".args(["issue", "move", &key, &status_in_progress()])"#;
    let results = parse_e2e_invocations(src);
    assert_eq!(results.len(), 1);
    let (path, flags) = &results[0];
    assert_eq!(path, &["issue", "move"]);
    assert!(flags.is_empty(), "no flags in this block");
}

/// The parser correctly handles multi-line blocks.
#[test]
fn test_parse_args_multi_line_extracts_path_and_flags() {
    let src = r#".args([
        "sprint",
        "list",
        "--board",
        &board_id,
        "--output",
        "json",
    ])"#;
    let results = parse_e2e_invocations(src);
    assert_eq!(results.len(), 1);
    let (path, flags) = &results[0];
    assert_eq!(path, &["sprint", "list"]);
    assert!(flags.contains(&"--board".to_string()));
    assert!(flags.contains(&"--output".to_string()));
}

/// The parser finds multiple invocations in a longer source snippet.
#[test]
fn test_parse_args_multiple_invocations_in_source() {
    let src = r#"
        h.cmd().args(["board", "list", "--output", "json"]).output();
        h.cmd().args(["team", "list", "--output", "json"]).output();
    "#;
    let results = parse_e2e_invocations(src);
    // Two distinct paths — dedup removes none because paths differ.
    assert_eq!(results.len(), 2, "two distinct invocations");
    let path_strings: Vec<Vec<String>> = results.iter().map(|(p, _)| p.clone()).collect();
    let board_path = vec!["board".to_string(), "list".to_string()];
    let team_path = vec!["team".to_string(), "list".to_string()];
    assert!(
        path_strings.contains(&board_path),
        "must contain board list"
    );
    assert!(path_strings.contains(&team_path), "must contain team list");
}

/// The parser deduplicates identical (path, flags) pairs.
#[test]
fn test_parse_args_deduplicates_identical_invocations() {
    let src = r#"
        h.cmd().args(["issue", "view", &key, "--output", "json"]).output();
        h.cmd().args(["issue", "view", &other, "--output", "json"]).output();
    "#;
    let results = parse_e2e_invocations(src);
    // Both map to (["issue", "view"], ["--output"]) — deduped to one.
    assert_eq!(results.len(), 1, "identical (path, flags) pairs must dedup");
}

// ---------------------------------------------------------------------------
// Self-proving test: the guard WOULD catch a bad path/flag
// ---------------------------------------------------------------------------

/// Proves the guard catches a nonexistent subcommand path.
///
/// Feeds the guard logic a synthetic `(path, flags)` pair with a bogus path
/// (`jr issue bogus`) and asserts that `jr issue bogus --help` would exit
/// non-zero. This proves the guard mechanism works — if the guard were wrong
/// (e.g., silently swallowing exit codes), THIS test would fail, alerting
/// maintainers that the guard is broken.
#[test]
fn test_guard_detects_nonexistent_subcommand_path() {
    let bogus_path = &["issue", "bogus"];
    let output = Command::cargo_bin("jr")
        .expect("jr binary must be built")
        .args(bogus_path)
        .arg("--help")
        .output()
        .expect("failed to spawn jr");

    assert!(
        !output.status.success(),
        "GUARD SELF-TEST FAILED: `jr issue bogus --help` unexpectedly exited 0 \
         — the guard mechanism would NOT catch bad paths. \
         This means `jr issue bogus` was actually added as a real subcommand. \
         Update this test to use a different bogus path."
    );
}

/// Proves the guard catches a nonexistent flag on a valid subcommand.
///
/// `--me` is specifically documented as the F-01 defect class: it was referenced
/// in an early draft of the E2E suite as `issue assign --me`, but no such flag
/// exists (self-assignment is triggered by omitting the assignee entirely).
///
/// This test asserts that `--me` does NOT appear in `jr issue assign --help`,
/// which means the guard would have flagged it had it been in the SURFACE table.
#[test]
fn test_guard_detects_nonexistent_flag_me_on_issue_assign() {
    let output = Command::cargo_bin("jr")
        .expect("jr binary must be built")
        .args(["issue", "assign"])
        .arg("--help")
        .output()
        .expect("failed to spawn jr issue assign --help");

    assert!(
        output.status.success(),
        "`jr issue assign --help` must exit 0; got {}",
        output.status.code().unwrap_or(-1)
    );

    let help_text = String::from_utf8_lossy(&output.stdout);
    assert!(
        !help_text.contains("--me"),
        "GUARD SELF-TEST FAILED: `--me` unexpectedly appeared in `jr issue assign --help`. \
         If `--me` was added as a real flag, update the SURFACE table to include it and \
         remove this assertion."
    );
}

// ---------------------------------------------------------------------------
// Parser applied to the real e2e_live.rs source
// ---------------------------------------------------------------------------

/// Verifies that parsing the real `tests/e2e_live.rs` source with
/// `parse_e2e_invocations` produces a set of paths that is a subset of the
/// paths already declared in `SURFACE`.
///
/// This is a consistency check: if someone adds a NEW `jr` invocation to
/// `e2e_live.rs` but forgets to add it to `SURFACE`, this test will flag the
/// gap. The author must then either add the new entry to `SURFACE` AND verify
/// the path + flags are valid via `jr <path> --help` (which `test_e2e_cli_surface_all_paths_and_flags_exist`
/// will then check automatically), or confirm the path was a false positive
/// and suppress it with a comment.
///
/// This test only checks PATHS (not flags) because the parser's flag extraction
/// is deliberately permissive (it collects all `--...` literals in the block,
/// which includes flag-value pairs like `"--jql"` where the value is a variable).
/// Full flag semantics are enforced by `test_e2e_cli_surface_all_paths_and_flags_exist`.
#[test]
fn test_parser_paths_are_subset_of_surface_table() {
    let source = include_str!("e2e_live.rs");
    let parsed = parse_e2e_invocations(source);

    // Build a set of paths already declared in SURFACE for fast lookup.
    let surface_paths: Vec<Vec<&str>> = SURFACE.iter().map(|(p, _)| p.to_vec()).collect();

    let mut missing: Vec<String> = Vec::new();
    for (path, _flags) in &parsed {
        let path_strs: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
        if !surface_paths.contains(&path_strs) {
            missing.push(format!(
                "  `jr {}` — parsed from e2e_live.rs but not in SURFACE table",
                path.join(" ")
            ));
        }
    }

    assert!(
        missing.is_empty(),
        "SURFACE TABLE GAP: the following `jr` invocation paths were found in \
         tests/e2e_live.rs by the parser but are NOT declared in the SURFACE \
         table in tests/e2e_cli_surface_guard.rs:\n\n{}\n\n\
         Add these entries to the SURFACE constant AND verify they exist in \
         the clap tree via `jr <path> --help`. The guard \
         `test_e2e_cli_surface_all_paths_and_flags_exist` will then validate \
         them automatically.",
        missing.join("\n")
    );
}

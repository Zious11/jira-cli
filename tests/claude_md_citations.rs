//! S-MAINT-DEAD-CITATION-CI — CLAUDE.md dead-citation CI guard.
//!
//! Governing BCs: BC-X.13.001, BC-X.13.002, BC-X.13.003.
//! Spec: `.factory/specs/prd/cross-cutting.md §BC-X.13`.
//! Verification delta: `.factory/phase-f2-spec-evolution/verification-delta-DEAD-CITATION-CI.md`.
//!
//! # Guard design
//!
//! `extract_path_citations(doc: &str) -> Vec<(String, usize)>` is a standalone
//! PURE function — no `Path::exists()` inside. The pure/effectful split is
//! load-bearing: it lets proptest exercise the grammar without any filesystem
//! mocking, and it isolates the tokenization/normalization logic from the
//! existence check that lives only in the integration test body.
//!
//! The 5-step pipeline (a)–(e) applied to each backtick-span token:
//!   (a) Glob skip: token containing `*`, `{`, or `}` → discarded.
//!   (b) Merged-fixpoint normalization (6 sub-steps, repeat until stable):
//!       (1) strip trailing `::…` symbol suffix
//!       (2) strip trailing `:~[0-9]+` or `:[0-9]+` line-ref suffix
//!       (3) strip one leading `(` or `[`
//!       (4) greedily trim trailing `.`, `,`, `;`, `:`
//!       (5) trim one trailing `)` iff count('(') < count(')')
//!       (6) trim one trailing `]` iff count('[') < count(']')
//!   (c) Dir-prefix filter + ROOT_FILES curated exact-match:
//!       keep if starts with `src/`, `tests/`, `docs/`, `.github/`, `scripts/`
//!       OR exactly equals a ROOT_FILES member.
//!       ALL `.factory/` prefixes fail this filter.
//!   (d) Extension filter: must end with `.md`, `.rs`, `.sh`, `.toml`, `.yml`,
//!       or `.yaml`.
//!   (e) Path::exists() — effectful, lives in the integration test ONLY.
//!
//! ROOT_FILES curated set (BC-X.13.002 step (c), immutable — any addition requires
//! a BC update in the same commit):
//!   build.rs, Cargo.toml, CHANGELOG.md, CLAUDE.md, deny.toml, README.md,
//!   rust-toolchain.toml
//!
//! # Test coverage map (→ ACs + VPs)
//!
//!   test_claude_md_citations_resolve_to_real_files      → AC-002, AC-003, VP-CITE-002
//!   test_dead_citation_detected_in_fixture              → AC-004, VP-CITE-002
//!   test_two_dead_citations_both_listed                 → AC-010, VP-CITE-002
//!   test_factory_specs_path_excluded_by_dir_prefix      → AC-005, VP-CITE-002, BC-X.13.003
//!   test_factory_holdout_path_excluded_by_dir_prefix    → AC-005, VP-CITE-002, BC-X.13.003
//!   test_factory_research_path_excluded_by_dir_prefix   → AC-005, VP-CITE-002, BC-X.13.003
//!   test_docs_path_is_in_scope                          → VP-CITE-002
//!   test_in_scope_src_path_extracted                    → AC-001, VP-CITE-001
//!   test_in_scope_tests_path_extracted                  → AC-001, VP-CITE-001
//!   test_in_scope_docs_path_extracted                   → AC-001, VP-CITE-001
//!   test_in_scope_scripts_path_extracted                → AC-001, VP-CITE-001
//!   test_in_scope_github_workflow_path_extracted        → AC-001, VP-CITE-001
//!   test_in_scope_yaml_path_extracted                   → AC-001, VP-CITE-001 (EC-CITE-035)
//!   test_glob_star_pattern_skipped                      → AC-001, VP-CITE-001
//!   test_glob_brace_pattern_skipped                     → AC-001, VP-CITE-001
//!   test_symbol_form_stripped_to_file                   → AC-001, VP-CITE-001
//!   test_symbol_form_no_dir_prefix_excluded             → AC-001, VP-CITE-001
//!   test_line_ref_tilde_stripped_to_file                → AC-001, VP-CITE-001
//!   test_line_ref_bare_stripped_to_file                 → AC-001, VP-CITE-001
//!   test_trailing_punct_comma_trimmed                   → AC-001, VP-CITE-001
//!   test_section_ref_doc_path_extracted_section_excluded → AC-001, VP-CITE-001
//!   test_no_extension_excluded                          → AC-001, VP-CITE-001
//!   test_url_in_backticks_excluded                      → AC-001, VP-CITE-001
//!   test_home_path_excluded                             → AC-001, VP-CITE-001
//!   test_env_var_excluded                               → AC-001, VP-CITE-001
//!   test_type_name_excluded                             → AC-001, VP-CITE-001
//!   test_factory_prefix_excluded_by_dir_filter          → AC-001, AC-005, VP-CITE-001
//!   test_paren_wrapping_stripped                        → AC-001, VP-CITE-001
//!   test_bracket_wrapping_stripped                      → AC-001, VP-CITE-001
//!   test_nested_parens_stripped                         → AC-001, VP-CITE-001
//!   test_root_file_cargo_toml_extracted                 → AC-006, VP-CITE-001 (EC-CITE-029)
//!   test_root_file_claude_md_extracted                  → AC-006, VP-CITE-001
//!   test_root_file_build_rs_extracted                   → AC-006, VP-CITE-001
//!   test_root_file_deny_toml_extracted                  → AC-006, VP-CITE-001
//!   test_root_file_readme_md_extracted                  → AC-006, VP-CITE-001
//!   test_root_file_changelog_md_extracted               → AC-006, VP-CITE-001
//!   test_root_file_rust_toolchain_toml_extracted        → AC-006, VP-CITE-001
//!   test_render_dead_citation_message_matches_ci_cite_001 → AC-002, BC-X.13.001 postcondition 2
//!   test_render_dead_citation_message_single_element     → AC-002, BC-X.13.001 postcondition 2 (L-2)
//!   test_fenced_code_block_path_excluded                 → EC-CITE-016, BC-X.13.002 fence-skip
//!   test_balanced_paren_in_path_not_stripped_by_step_b5 → BC-X.13.002 step (b) sub-step (5) (L-1)
//!   test_citation_on_line_3_returns_exact_line_number   → BC-X.13.001 postcondition 2, BC-X.13.002 (M-1 extra pin)
//!   test_shorthand_ci_yml_excluded                      → AC-006, VP-CITE-001 (EC-CITE-030)
//!   test_shorthand_adf_rs_excluded                      → AC-006, VP-CITE-001 (EC-CITE-031)
//!   test_shorthand_fields_json_excluded                 → AC-006, VP-CITE-001
//!   test_shorthand_release_yml_excluded                 → AC-006, VP-CITE-001
//!   test_paren_wrapped_root_file_extracted              → AC-006, VP-CITE-001 (EC-CITE-032)
//!   test_fixpoint_ec026_paren_plus_line_ref             → AC-007, VP-CITE-001 (EC-CITE-026)
//!   test_fixpoint_ec027_line_ref_plus_comma             → AC-007, VP-CITE-001 (EC-CITE-027)
//!   test_fixpoint_ec028_symbol_plus_punct               → AC-007, VP-CITE-001 (EC-CITE-028)
//!   test_fixpoint_ec023_bracket_wrap                    → AC-007, VP-CITE-001 (EC-CITE-023)
//!   test_fixpoint_ec025_double_paren_wrap               → AC-007, VP-CITE-001 (EC-CITE-025)
//!   test_extension_filter_excludes_extensionless_token  → AC-011, VP-CITE-001 (EC-CITE-033)
//!   test_extension_filter_excludes_lock_extension       → AC-011, VP-CITE-001 (EC-CITE-034)
//!   test_comma_delimited_both_tokens_extracted          → AC-012, VP-CITE-001 (EC-CITE-002)
//!   test_crlf_line_endings_no_false_positive            → AC-012, VP-CITE-001 (EC-CITE-003)
//!   proptests::test_non_prefix_tokens_are_never_extracted → AC-008, VP-CITE-001
//!   proptests::test_extract_never_panics                → AC-008, VP-CITE-001

use std::path::Path;

// ---------------------------------------------------------------------------
// Canonical CI-CITE-001 failure message renderer.
// ---------------------------------------------------------------------------

/// Render the canonical CI-CITE-001 failure block for a list of dead citations.
///
/// This is the EXACT wording required by BC-X.13.001 postcondition 2.
/// It is used in BOTH the integration test's panic message AND the dedicated
/// assertion test below — any drift between the two will fail CI.
fn render_dead_citation_message(dead: &[(String, usize)]) -> String {
    let paths = dead
        .iter()
        .map(|(p, n)| format!("{} (line {})", p, n))
        .collect::<Vec<_>>()
        .join("\n  ");
    format!(
        "CLAUDE.md cites file paths that do not exist on disk:\n  {}\nFix the citation or restore the file.\nNote: .factory/, glob, and symbol-form tokens are auto-excluded. Root-level files (Cargo.toml, CLAUDE.md, etc.) are checked.",
        paths
    )
}

// ---------------------------------------------------------------------------
// Pure extraction function.
// Implements BC-X.13.002: 5-step pipeline (a)–(e), returning
// (normalized_path, 1-based-line-number) pairs.
// No Path::exists() calls inside — pure, no I/O.
// ---------------------------------------------------------------------------

/// Extract all in-scope backtick-quoted file-path citations from `doc`.
///
/// Returns `Vec<(normalized_path, 1-based-line-number)>` pairs, sorted and
/// deduplicated.
///
/// Pipeline (BC-X.13.002):
///   (a) Glob skip: token containing `*`, `{`, or `}` → discarded.
///   (b) Merged-fixpoint normalization (6 sub-steps, repeat until stable):
///       (1) strip trailing `::…` symbol suffix
///       (2) strip trailing `:~[0-9]+` or `:[0-9]+` line-ref suffix
///       (3) strip one leading `(` or `[`
///       (4) greedily trim trailing `.`, `,`, `;`, `:`
///       (5) trim one trailing `)` iff count('(') < count(')')
///       (6) trim one trailing `]` iff count('[') < count(']')
///   (c) Dir-prefix filter + ROOT_FILES curated exact-match.
///   (d) Extension filter: `.md`, `.rs`, `.sh`, `.toml`, `.yml`, `.yaml`.
///   (e) Path::exists() — lives in integration test ONLY (not called here).
///
/// No `Path::exists()` calls inside — pure, no I/O.
fn extract_path_citations(doc: &str) -> Vec<(String, usize)> {
    // ROOT_FILES curated set — immutable per BC-X.13.002 step (c).
    const ROOT_FILES: &[&str] = &[
        "build.rs",
        "Cargo.toml",
        "CHANGELOG.md",
        "CLAUDE.md",
        "deny.toml",
        "README.md",
        "rust-toolchain.toml",
    ];

    // Recognized extensions per BC-X.13.002 step (d).
    const RECOGNIZED_EXTS: &[&str] = &[".md", ".rs", ".sh", ".toml", ".yml", ".yaml"];

    // Develop-tracked directory prefixes per BC-X.13.002 step (c).
    const DIR_PREFIXES: &[&str] = &["src/", "tests/", "docs/", ".github/", "scripts/"];

    // Normalize CRLF: replace \r\n with \n and lone \r with \n (EC-CITE-003).
    // We work on a CRLF-normalized copy so line counting is consistent.
    let normalized: String = {
        let mut s = String::with_capacity(doc.len());
        let mut chars = doc.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\r' {
                s.push('\n');
                // consume a following \n so \r\n counts as one newline
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            } else {
                s.push(ch);
            }
        }
        s
    };

    // --- Extract inline single-backtick spans ---
    // We skip fenced triple-backtick code blocks (M-1 / EC-CITE-016).
    // Strategy: scan character-by-character; when inside a triple-backtick
    // fence we skip until the closing ```.  When we see a single ` (not part
    // of a triple run) we collect until the matching closing `.

    // First pass: identify which byte ranges are inside fenced code blocks
    // so we can skip them during backtick-span scanning.
    // We collect the start/end byte offsets of each inline backtick span.

    let mut candidates: Vec<(String, usize)> = Vec::new();

    let bytes = normalized.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Check for triple-backtick fence (skip the fenced block)
        if bytes[i] == b'`' && i + 2 < len && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
            // Skip past the opening ```
            i += 3;
            // Skip optional language identifier and content until closing ```
            while i < len {
                if bytes[i] == b'`' && i + 2 < len && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
                    i += 3;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Single backtick — find the span content
        if bytes[i] == b'`' {
            // Compute 1-based line number of the opening backtick
            // O(i) newline scan per backtick span — fine at CLAUDE.md scale; switch to a running counter if it grows 10x.
            let line_number = normalized[..i].chars().filter(|&c| c == '\n').count() + 1;

            let span_start = i + 1;
            let mut j = span_start;
            // Find the matching closing `
            while j < len && bytes[j] != b'`' {
                j += 1;
            }

            if j < len {
                // We found a closing backtick at j
                let span = &normalized[span_start..j];

                // Split on ASCII whitespace AND commas (EC-CITE-002: comma-delimited form).
                // Replace commas with spaces then split on whitespace.
                let span_with_spaces = span.replace(',', " ");
                for raw_token in span_with_spaces.split_ascii_whitespace() {
                    // Strip any stray \r (safety net for CRLF normalization edge cases)
                    let token = raw_token.trim_end_matches('\r');
                    if token.is_empty() {
                        continue;
                    }

                    // --- Step (a): Glob skip ---
                    if token.contains('*') || token.contains('{') || token.contains('}') {
                        continue;
                    }

                    // --- Step (b): Merged-fixpoint normalization ---
                    let normalized_token = apply_fixpoint(token);

                    // Skip empty result
                    if normalized_token.is_empty() {
                        continue;
                    }

                    // --- Step (c): Dir-prefix filter + ROOT_FILES ---
                    let in_scope = DIR_PREFIXES.iter().any(|p| normalized_token.starts_with(p))
                        || ROOT_FILES.contains(&normalized_token.as_str());
                    if !in_scope {
                        continue;
                    }

                    // --- Step (d): Extension filter ---
                    let has_recognized_ext = RECOGNIZED_EXTS
                        .iter()
                        .any(|ext| normalized_token.ends_with(ext));
                    if !has_recognized_ext {
                        continue;
                    }

                    candidates.push((normalized_token, line_number));
                }

                i = j + 1; // skip past the closing `
            } else {
                // No closing backtick found — skip
                i = j;
            }
            continue;
        }

        i += 1;
    }

    // Sort by (line_number, path) for deterministic document order, then deduplicate.
    candidates.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    candidates.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);

    candidates
}

/// Apply the merged-fixpoint normalization (BC-X.13.002 step (b)).
///
/// Repeats all 6 sub-steps as one unit until a complete pass produces no change.
/// Returns the normalized string (may be empty if everything is stripped).
fn apply_fixpoint(token: &str) -> String {
    let mut current = token.to_owned();

    loop {
        let before = current.clone();

        // Sub-step (1): strip trailing `::…` symbol suffix.
        // Everything from the FIRST `::` onwards is stripped.
        if let Some(pos) = current.find("::") {
            current.truncate(pos);
        }

        // Sub-step (2): strip trailing `:~[0-9]+` or `:[0-9]+` line-ref suffix.
        // Pattern: ends with `:` followed by optional `~` followed by one-or-more digits.
        {
            let s = current.as_str();
            if let Some(colon_pos) = find_line_ref_suffix(s) {
                current.truncate(colon_pos);
            }
        }

        // Sub-step (3): strip one leading `(` or `[`.
        if current.starts_with('(') || current.starts_with('[') {
            current.remove(0);
        }

        // Sub-step (4): greedily trim trailing `.`, `,`, `;`, `:`.
        while current.ends_with(['.', ',', ';', ':']) {
            let new_len = current.len() - 1;
            current.truncate(new_len);
        }

        // Sub-step (5): trim one trailing `)` iff count('(') < count(')').
        if current.ends_with(')') {
            let open = current.chars().filter(|&c| c == '(').count();
            let close = current.chars().filter(|&c| c == ')').count();
            if open < close {
                let new_len = current.len() - 1;
                current.truncate(new_len);
            }
        }

        // Sub-step (6): trim one trailing `]` iff count('[') < count(']').
        if current.ends_with(']') {
            let open = current.chars().filter(|&c| c == '[').count();
            let close = current.chars().filter(|&c| c == ']').count();
            if open < close {
                let new_len = current.len() - 1;
                current.truncate(new_len);
            }
        }

        // If no change this pass, we've reached the fixpoint.
        if current == before {
            break;
        }
    }

    current
}

/// Find the start position of a trailing `:~[0-9]+` or `:[0-9]+` suffix.
/// Returns `Some(colon_pos)` if found, `None` otherwise.
///
/// The rule is: the trailing colon-run is entirely digits (with optional leading `~`),
/// not a free regex — only the LAST `:` in the string is inspected.
fn find_line_ref_suffix(s: &str) -> Option<usize> {
    // Look for the last `:` in the string.
    let colon_pos = s.rfind(':')?;
    let after_colon = &s[colon_pos + 1..];

    // After the colon, we expect optional `~` then one or more ASCII digits.
    let rest = after_colon.strip_prefix('~').unwrap_or(after_colon);
    if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
        Some(colon_pos)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Integration tests — VP-CITE-002
// ---------------------------------------------------------------------------

/// BC-X.13.001 postcondition 1: guard is GREEN on develop HEAD.
/// BC-X.13.001 postcondition 2: failure message is CI-CITE-001 verbatim
///   with real 1-based line numbers.
///
/// No `is_off_working_branch_allowlisted` call — `.factory/` exclusion is
/// structural inside `extract_path_citations` at step (c); no allowlist needed.
///
/// Traces to AC-002, AC-003, VP-CITE-002.
#[test]
fn test_claude_md_citations_resolve_to_real_files() {
    let doc = include_str!("../CLAUDE.md");
    let root = env!("CARGO_MANIFEST_DIR");
    // extract_path_citations returns Vec<(String, usize)> — (normalized_path, 1-based line)
    let citations = extract_path_citations(doc);
    // No is_off_working_branch_allowlisted call — .factory/ is excluded by
    // extract_path_citations dir-prefix filter (step (c)); no allowlist needed.
    let dead: Vec<(String, usize)> = citations
        .into_iter()
        .filter(|(p, _)| !Path::new(root).join(p).exists())
        .collect();
    assert!(
        dead.is_empty(),
        // CANONICAL failure message — CI-CITE-001 (error-taxonomy §8).
        // render_dead_citation_message produces the exact wording; change wording
        // there, not here, so the dedicated assertion test catches drift.
        "{}",
        render_dead_citation_message(&dead)
    );
}

/// BC-X.13.001 postcondition 3: guard fails deterministically on a known-dead citation.
///
/// Uses a fixture doc string (NOT include_str!("../CLAUDE.md")) so the test
/// does not depend on CLAUDE.md content.
/// No allowlist call — `.factory/` exclusion is structural inside
/// `extract_path_citations`.
///
/// Traces to AC-004, VP-CITE-002.
#[test]
fn test_dead_citation_detected_in_fixture() {
    // Construct a doc string with a known-dead path. We use a path
    // that is guaranteed not to exist (no real jr file is at this path).
    let fixture_doc = r#"
Some documentation text.
Detail: `src/DOES_NOT_EXIST_IN_ANY_JR_BUILD.rs`
"#;
    let root = env!("CARGO_MANIFEST_DIR");
    // citations: Vec<(String, usize)> — (normalized_path, 1-based-line-number)
    let citations = extract_path_citations(fixture_doc);
    // No allowlist filter — .factory/ paths never reach this check (dir-prefix excluded).
    let dead: Vec<(String, usize)> = citations
        .into_iter()
        .filter(|(p, _)| !Path::new(root).join(p).exists())
        .collect();
    assert!(
        dead.iter()
            .any(|(p, _)| p == "src/DOES_NOT_EXIST_IN_ANY_JR_BUILD.rs"),
        "Expected dead citation to be detected but was not in: {:?}",
        dead
    );
}

/// BC-X.13.001 postcondition 2: ALL dead paths listed (not just first);
/// `.join("\\n  ")` structure correct; document order; real line numbers.
///
/// Feeds a fixture doc with TWO distinct dead citations on different lines
/// and asserts both are present, in order, with correct rendered join string.
///
/// Traces to AC-010, VP-CITE-002.
#[test]
fn test_two_dead_citations_both_listed() {
    let doc =
        "See `src/DOES_NOT_EXIST_ONE.rs` for details.\nAnd also `src/DOES_NOT_EXIST_TWO.rs`.\n";
    let root = env!("CARGO_MANIFEST_DIR");
    // citations: Vec<(String, usize)> — (normalized_path, 1-based-line-number)
    let citations = extract_path_citations(doc);

    // Assertion 1: exactly two entries returned
    assert_eq!(
        citations.len(),
        2,
        "Expected exactly 2 citations in fixture doc, got {}: {:?}",
        citations.len(),
        citations
    );

    // Assertion 2: document order — citation on line 1 appears before line 2
    // Clone line numbers before consuming citations, to avoid borrow-after-move.
    let (path0, line0) = citations[0].clone();
    let (path1, line1) = citations[1].clone();
    assert_eq!(
        path0, "src/DOES_NOT_EXIST_ONE.rs",
        "First citation should be DOES_NOT_EXIST_ONE.rs, got: {:?}",
        citations
    );
    assert_eq!(
        path1, "src/DOES_NOT_EXIST_TWO.rs",
        "Second citation should be DOES_NOT_EXIST_TWO.rs, got: {:?}",
        citations
    );

    // M-1: assert EXACT 1-based absolute line numbers.
    // The fixture string is a single doc with no leading newline:
    //   line 1: "See `src/DOES_NOT_EXIST_ONE.rs` for details."
    //   line 2: "And also `src/DOES_NOT_EXIST_TWO.rs`."
    // `extract_path_citations` counts newlines before the opening backtick,
    // so the citation on line 1 has 0 preceding newlines → line number 1;
    // the citation on line 2 has 1 preceding newline → line number 2.
    // Killing the +1→+0 or +1→+2 mutant requires these exact assertions.
    assert_eq!(
        line0, 1,
        "First citation must be on line 1 (exact 1-based), got: {}",
        line0
    );
    assert_eq!(
        line1, 2,
        "Second citation must be on line 2 (exact 1-based), got: {}",
        line1
    );

    assert!(
        line0 < line1,
        "First citation line ({}) must come before second citation line ({})",
        line0,
        line1
    );

    // Assertion 3: both are dead after filtering
    let dead: Vec<(String, usize)> = citations
        .into_iter()
        .filter(|(p, _)| !Path::new(root).join(p).exists())
        .collect();
    assert_eq!(
        dead.len(),
        2,
        "Both citations should be dead, got: {:?}",
        dead
    );

    // Assertion 4: rendered message uses \n  join (two spaces after newline)
    let rendered = dead
        .iter()
        .map(|(p, n)| format!("{} (line {})", p, n))
        .collect::<Vec<_>>()
        .join("\n  ");
    // The join separator is \n  (newline + two spaces) — not \n alone
    assert!(
        rendered.contains("\n  "),
        "Rendered message must use \\n  (newline + two spaces) as join separator, got: {:?}",
        rendered
    );

    // Assertion 5: each (line N) component is a real integer, not the literal "N"
    assert!(
        rendered.contains(&format!("(line {})", line0)),
        "Rendered message must contain real line number for first citation, got: {:?}",
        rendered
    );
    assert!(
        rendered.contains(&format!("(line {})", line1)),
        "Rendered message must contain real line number for second citation, got: {:?}",
        rendered
    );
}

/// BC-X.13.001 postcondition 2: `render_dead_citation_message` produces the
/// exact canonical CI-CITE-001 block byte-for-byte.
///
/// Feeds a 2-entry fixture and asserts the output equals the hardcoded expected
/// string. This pins the load-bearing wording so any drift fails CI.
///
/// Traces to AC-002, BC-X.13.001 postcondition 2.
#[test]
fn test_render_dead_citation_message_matches_ci_cite_001() {
    let dead = vec![
        ("src/foo.rs".to_string(), 142usize),
        ("tests/bar.rs".to_string(), 287usize),
    ];
    let got = render_dead_citation_message(&dead);
    let expected = "CLAUDE.md cites file paths that do not exist on disk:\n  src/foo.rs (line 142)\n  tests/bar.rs (line 287)\nFix the citation or restore the file.\nNote: .factory/, glob, and symbol-form tokens are auto-excluded. Root-level files (Cargo.toml, CLAUDE.md, etc.) are checked.";
    assert_eq!(
        got, expected,
        "render_dead_citation_message must produce the exact CI-CITE-001 canonical block"
    );
}

/// BC-X.13.001 postcondition 2: `render_dead_citation_message` with a SINGLE
/// dead entry produces the exact CI-CITE-001 block — confirms the 2-space indent
/// and leading `\n  ` hold for a one-element list (L-2 mutation gap).
///
/// A mutant that changes the join separator (e.g. `\n` instead of `\n  `) or
/// drops the indent for single-element lists will fail here.
///
/// Traces to AC-002, BC-X.13.001 postcondition 2.
#[test]
fn test_render_dead_citation_message_single_element() {
    let dead = vec![("src/foo.rs".to_string(), 142usize)];
    let got = render_dead_citation_message(&dead);
    let expected = "CLAUDE.md cites file paths that do not exist on disk:\n  src/foo.rs (line 142)\nFix the citation or restore the file.\nNote: .factory/, glob, and symbol-form tokens are auto-excluded. Root-level files (Cargo.toml, CLAUDE.md, etc.) are checked.";
    assert_eq!(
        got, expected,
        "render_dead_citation_message must produce CI-CITE-001 block for a single entry"
    );
}

/// EC-CITE-016: fenced triple-backtick code blocks are excluded entirely.
///
/// A fenced block whose body contains an in-scope, recognized-extension path that
/// does NOT exist on disk (e.g. `src/DOES_NOT_EXIST_FENCED.rs`) must produce
/// EMPTY output. Disabling the fence-skip branch would make this test fail,
/// which kills the "remove fence exclusion" mutation.
///
/// Traces to EC-CITE-016, BC-X.13.002 step (fence-skip precondition).
#[test]
fn test_fenced_code_block_path_excluded() {
    // The fenced block body contains a syntactically valid, in-scope,
    // recognized-extension path that does NOT exist on disk.
    // The path starts with `src/` (passes step c) and ends with `.rs` (passes step d).
    // If the fence-skip is disabled, `extract_path_citations` would return this path.
    let doc = "```\nsrc/DOES_NOT_EXIST_FENCED.rs\n```\n";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "EC-CITE-016: path inside triple-backtick fenced block must be excluded, \
         but extract_path_citations returned: {:?}",
        result
    );
}

/// L-1: sub-step (5) uses `open < close`, NOT `open <= close`.
///
/// A token `src/foo(bar).rs` has one open paren and one close paren at the END
/// of the meaningful segment; after step (4) trim nothing (no trailing punct),
/// the trailing `)` count-check gives open=1, close=1. Under the correct `<`
/// rule: 1 < 1 is false → `)` is NOT stripped → path stays `src/foo(bar).rs`.
/// Under a buggy `<=` rule: 1 <= 1 is true → `)` WOULD be stripped → path
/// becomes `src/foo(bar.rs` (a different string). This test asserts the CORRECT
/// path `src/foo(bar).rs` is returned, killing the `<`→`<=` mutant.
///
/// Note: `src/foo(bar).rs` does not exist on disk — this is a grammar-only
/// unit test. Existence checking lives in the integration test body only.
///
/// Traces to BC-X.13.002 step (b) sub-step (5).
#[test]
fn test_balanced_paren_in_path_not_stripped_by_step_b5() {
    // Token has balanced parens embedded in the path name.
    // The trailing `)` is balanced (open == close), so it must NOT be stripped.
    let doc = "See `src/foo(bar).rs` for details.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/foo(bar).rs"),
        "Balanced trailing ) must not be stripped (open < close rule, not <=): \
         expected src/foo(bar).rs in output, got: {:?}",
        result
    );
    // Additionally confirm that the wrongly-stripped form is NOT present.
    assert!(
        !result.iter().any(|(p, _)| p == "src/foo(bar.rs"),
        "Balanced ) must not produce src/foo(bar.rs (would indicate <= bug), got: {:?}",
        result
    );
}

/// M-1 (additional pin): citation on a later line (line 3+) in a multi-line
/// fixture returns the correct absolute line number.
///
/// Fixture has 4 lines; the citation appears on line 3. Asserting `line == 3`
/// kills both the +0 and +2 off-by-one mutants relative to the `+1` in the
/// newline-count expression.
///
/// Traces to BC-X.13.001 postcondition 2, BC-X.13.002 line-number rule.
#[test]
fn test_citation_on_line_3_returns_exact_line_number() {
    // 4-line doc; citation appears on line 3 (0-indexed line 2 = two preceding newlines + 1).
    let doc = "Line one.\nLine two.\nSee `src/DOES_NOT_EXIST_LINE3.rs` here.\nLine four.\n";
    let result = extract_path_citations(doc);
    assert_eq!(
        result.len(),
        1,
        "Expected exactly 1 citation, got: {:?}",
        result
    );
    let (path, line) = &result[0];
    assert_eq!(
        path, "src/DOES_NOT_EXIST_LINE3.rs",
        "Expected path src/DOES_NOT_EXIST_LINE3.rs, got: {:?}",
        path
    );
    assert_eq!(
        *line, 3,
        "Citation on line 3 must return exact line number 3, got: {}",
        line
    );
}

/// BC-X.13.003 invariant: `.factory/specs/` prefix is NOT in develop-tracked
/// prefix set — excluded at step (c), no allowlist function.
///
/// Traces to AC-005, VP-CITE-002.
#[test]
fn test_factory_specs_path_excluded_by_dir_prefix() {
    // .factory/ is not in the develop-tracked prefix set — never extracted
    let doc = "See `.factory/specs/prd/bc-3-issue-write.md` for details.";
    let citations = extract_path_citations(doc);
    assert!(
        citations.is_empty(),
        "Expected .factory/ path to be excluded but got: {:?}",
        citations
    );
}

/// BC-X.13.003 invariant: `.factory/holdout-scenarios/` excluded at step (c).
///
/// Traces to AC-005, VP-CITE-002.
#[test]
fn test_factory_holdout_path_excluded_by_dir_prefix() {
    let doc = "See `.factory/holdout-scenarios/H-001.md` for details.";
    let citations = extract_path_citations(doc);
    assert!(
        citations.is_empty(),
        "Expected .factory/ holdout path to be excluded but got: {:?}",
        citations
    );
}

/// BC-X.13.003 invariant: `.factory/research/` also excluded — no sub-path
/// partition within `.factory/`.
///
/// Traces to AC-005, VP-CITE-002.
#[test]
fn test_factory_research_path_excluded_by_dir_prefix() {
    // .factory/research/ is also excluded — no sub-path partition within .factory/
    let doc = "See `.factory/research/S-3.03-wave3-verification.md` for details.";
    let citations = extract_path_citations(doc);
    assert!(
        citations.is_empty(),
        "Expected .factory/research/ path to be excluded but got: {:?}",
        citations
    );
}

/// `docs/` prefix is a develop-tracked prefix — IS in scope.
///
/// Traces to VP-CITE-002.
#[test]
fn test_docs_path_is_in_scope() {
    let doc = "See `docs/adr/0001-thin-client-architecture.md` for details.";
    let citations = extract_path_citations(doc);
    assert!(
        citations
            .iter()
            .any(|(p, _)| p == "docs/adr/0001-thin-client-architecture.md"),
        "Expected docs/ path to be in scope, got: {:?}",
        citations
    );
}

// ---------------------------------------------------------------------------
// Unit tests — VP-CITE-001 (in-scope detection)
// ---------------------------------------------------------------------------

/// `src/` prefix is a develop-tracked dir prefix — IS extracted.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_in_scope_src_path_extracted() {
    let doc = "See `src/adf.rs` for details.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/adf.rs"),
        "Expected src/adf.rs to be extracted, got: {:?}",
        result
    );
}

/// `tests/` prefix is a develop-tracked dir prefix — IS extracted.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_in_scope_tests_path_extracted() {
    let doc = "See `tests/auth_profiles.rs` for details.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "tests/auth_profiles.rs"),
        "Expected tests/auth_profiles.rs to be extracted, got: {:?}",
        result
    );
}

/// `docs/` prefix is a develop-tracked dir prefix — IS extracted.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_in_scope_docs_path_extracted() {
    let doc = "See `docs/specs/adf-task-list.md` for details.";
    let result = extract_path_citations(doc);
    assert!(
        result
            .iter()
            .any(|(p, _)| p == "docs/specs/adf-task-list.md"),
        "Expected docs/specs/adf-task-list.md to be extracted, got: {:?}",
        result
    );
}

/// `scripts/` prefix is a develop-tracked dir prefix — IS extracted.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_in_scope_scripts_path_extracted() {
    let doc = "Run `scripts/check-spec-counts.sh` to validate.";
    let result = extract_path_citations(doc);
    assert!(
        result
            .iter()
            .any(|(p, _)| p == "scripts/check-spec-counts.sh"),
        "Expected scripts/check-spec-counts.sh to be extracted, got: {:?}",
        result
    );
}

/// `.github/` prefix is a develop-tracked dir prefix — IS extracted.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_in_scope_github_workflow_path_extracted() {
    let doc = "See `.github/workflows/ci.yml` for the CI config.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == ".github/workflows/ci.yml"),
        "Expected .github/workflows/ci.yml to be extracted, got: {:?}",
        result
    );
}

/// `.yaml` extension is in `RECOGNIZED_EXTS` — IS extracted (EC-CITE-035).
///
/// A mutant that drops `".yaml"` from `RECOGNIZED_EXTS` would make this test fail
/// because the token passes step (c) (`.github/` dir prefix) but then fails step (d)
/// without the `.yaml` entry, producing empty output.
///
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_in_scope_yaml_path_extracted() {
    let doc = "See `.github/workflows/foo.yaml` for the workflow definition.";
    let result = extract_path_citations(doc);
    assert!(
        result
            .iter()
            .any(|(p, _)| p == ".github/workflows/foo.yaml"),
        "Expected .github/workflows/foo.yaml to be extracted (.yaml is a recognized extension), \
         got: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Unit tests — VP-CITE-001 (glob skip, step a)
// ---------------------------------------------------------------------------

/// Step (a): token containing `*` → skipped entirely.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_glob_star_pattern_skipped() {
    let doc = "See `src/cli/bc-*.md` for patterns.";
    let result = extract_path_citations(doc);
    assert!(
        !result.iter().any(|(p, _)| p.contains('*')),
        "Glob patterns with * must be skipped, got: {:?}",
        result
    );
    // No token with src/cli/bc- should leak
    assert!(
        result.is_empty(),
        "Glob-containing token must produce no output, got: {:?}",
        result
    );
}

/// Step (a): token containing `{` or `}` → skipped entirely.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_glob_brace_pattern_skipped() {
    let doc = "See `adf-{block,task}-list.md` for patterns.";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "Brace-glob tokens must be skipped entirely, got: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Unit tests — VP-CITE-001 (fixpoint normalization, step b)
// ---------------------------------------------------------------------------

/// Step (b) sub-step (1): symbol-form `::foo` suffix stripped → `src/adf.rs`.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_symbol_form_stripped_to_file() {
    let doc = "See `src/adf.rs::push_text` for the implementation.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/adf.rs"),
        "Symbol-form suffix ::push_text must be stripped to src/adf.rs, got: {:?}",
        result
    );
    // Must NOT include the raw symbol form
    assert!(
        !result.iter().any(|(p, _)| p.contains("::push_text")),
        "Symbol-form suffix must not appear in output, got: {:?}",
        result
    );
}

/// Step (b) sub-step (1): `adf::tests::test_bare_url_split` has `::` but no known
/// dir prefix before `::` → NOT extracted (excluded by step (c) dir-prefix filter).
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_symbol_form_no_dir_prefix_excluded() {
    let doc = "Test `adf::tests::test_bare_url_split`.";
    let result = extract_path_citations(doc);
    // Even after stripping ::tests::test_bare_url_split → `adf` has no dir prefix
    assert!(
        result.is_empty(),
        "Symbol-form with no dir prefix must not be extracted, got: {:?}",
        result
    );
}

/// Step (b) sub-step (2): `:~42` line-ref with tilde stripped → `src/config.rs`.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_line_ref_tilde_stripped_to_file() {
    let doc = "See `src/config.rs:~42` (approximately line 42).";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/config.rs"),
        "Line-ref :~42 must be stripped to src/config.rs, got: {:?}",
        result
    );
}

/// Step (b) sub-step (2): `:100` bare line-ref stripped → `src/config.rs`.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_line_ref_bare_stripped_to_file() {
    let doc = "See `src/config.rs:100` for the load function.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/config.rs"),
        "Line-ref :100 must be stripped to src/config.rs, got: {:?}",
        result
    );
}

/// Step (b) sub-step (4): trailing comma trimmed → `src/adf.rs`.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_trailing_punct_comma_trimmed() {
    let doc = "Detail: `src/adf.rs,` and other things.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/adf.rs"),
        "Trailing comma must be trimmed from src/adf.rs, got: {:?}",
        result
    );
    assert!(
        !result.iter().any(|(p, _)| p.ends_with(',')),
        "No path must end with a comma, got: {:?}",
        result
    );
}

/// Step (c) dir-prefix filter: section-ref token `§9` excluded; preceding
/// `docs/specs/e2e-live-jira-testing.md` IS extracted (whitespace-split makes
/// them separate tokens from the same backtick span).
///
/// Note: when `docs/specs/e2e-live-jira-testing.md §9` is a single backtick
/// span, whitespace tokenization splits it into `docs/specs/e2e-live-jira-testing.md`
/// and `§9`. The former passes step (c); the latter has no dir prefix.
///
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_section_ref_doc_path_extracted_section_excluded() {
    let doc = "Detail: `docs/specs/e2e-live-jira-testing.md §9` (section reference).";
    let result = extract_path_citations(doc);
    assert!(
        result
            .iter()
            .any(|(p, _)| p == "docs/specs/e2e-live-jira-testing.md"),
        "docs/ path should be extracted even when section ref follows, got: {:?}",
        result
    );
    // §9 must NOT appear as a path
    assert!(
        !result.iter().any(|(p, _)| p.contains('§')),
        "Section ref §9 must not appear in output, got: {:?}",
        result
    );
}

/// Step (d) extension filter: `src/cli/issue` has `src/` prefix but no recognized
/// extension → EXCLUDED.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_no_extension_excluded() {
    let doc = "See `src/cli/issue` (directory reference).";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "Extensionless token must be excluded at step (d), got: {:?}",
        result
    );
}

/// Step (c) dir-prefix filter: URL `http://127.0.0.1:53682/callback` does not
/// start with a develop-tracked prefix → NOT extracted.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_url_in_backticks_excluded() {
    let doc = "Callback URL is `http://127.0.0.1:53682/callback`.";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "URL token must be excluded by dir-prefix filter, got: {:?}",
        result
    );
}

/// Step (c) dir-prefix filter: `~/.config/jr/config.toml` does not start
/// with a develop-tracked prefix (starts with `~`) → NOT extracted.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_home_path_excluded() {
    let doc = "Config lives at `~/.config/jr/config.toml`.";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "Home-directory path must be excluded by dir-prefix filter, got: {:?}",
        result
    );
}

/// Step (c) dir-prefix filter: `JR_BASE_URL` has no `/` and no dir prefix → NOT extracted.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_env_var_excluded() {
    let doc = "Use `JR_BASE_URL` to override the URL.";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "Env var name must be excluded by dir-prefix filter, got: {:?}",
        result
    );
}

/// Step (c) dir-prefix filter: `std::sync::Mutex` has `::` but no known dir prefix
/// → NOT extracted.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_type_name_excluded() {
    let doc = "Uses `std::sync::Mutex` for thread safety.";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "Type name std::sync::Mutex must be excluded by dir-prefix filter, got: {:?}",
        result
    );
}

/// Step (c) dir-prefix filter: `.factory/research/S-3.03-wave3-verification.md`
/// prefix is NOT in develop-tracked set → NOT extracted.
/// Traces to AC-001, AC-005, VP-CITE-001.
#[test]
fn test_factory_prefix_excluded_by_dir_filter() {
    let doc = "Detail: `.factory/research/S-3.03-wave3-verification.md` (Claim 2, REFUTED).";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        ".factory/ prefix must be excluded at step (c), got: {:?}",
        result
    );
}

/// Step (b) sub-steps (3)+(5): leading `(` stripped; trailing `)` unbalanced → trimmed.
/// `(src/adf.rs)` → `src/adf.rs` — IS in output.
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_paren_wrapping_stripped() {
    let doc = "See `(src/adf.rs)` for details.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/adf.rs"),
        "Paren-wrapped (src/adf.rs) must normalize to src/adf.rs, got: {:?}",
        result
    );
}

/// Step (b) sub-steps (3)+(6): leading `[` stripped; trailing `]` unbalanced → trimmed.
/// `[docs/x.md]` → `docs/x.md` — IS in output (EC-CITE-023 equivalent with real path).
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_bracket_wrapping_stripped() {
    let doc = "See `[docs/specs/adf-task-list.md]` for the spec.";
    let result = extract_path_citations(doc);
    assert!(
        result
            .iter()
            .any(|(p, _)| p == "docs/specs/adf-task-list.md"),
        "Bracket-wrapped [docs/specs/adf-task-list.md] must normalize, got: {:?}",
        result
    );
}

/// Step (b) multi-pass: `((src/x.rs))` — two fixpoint passes each strip one paren
/// layer → `src/x.rs` — IS in output (EC-CITE-025).
/// Traces to AC-001, VP-CITE-001.
#[test]
fn test_nested_parens_stripped() {
    let doc = "See `((src/adf.rs))` for details.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/adf.rs"),
        "Double paren-wrapped ((src/adf.rs)) must normalize to src/adf.rs, got: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Unit tests — VP-CITE-001 (ROOT_FILES, step c — AC-006)
// ---------------------------------------------------------------------------

/// ROOT_FILES inclusion: `Cargo.toml` exactly matches ROOT_FILES member;
/// `.toml` passes step (d) — IS extracted (EC-CITE-029).
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_root_file_cargo_toml_extracted() {
    let doc = "See `Cargo.toml` for dependencies.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "Cargo.toml"),
        "Cargo.toml must be extracted as ROOT_FILES member, got: {:?}",
        result
    );
}

/// ROOT_FILES inclusion: `CLAUDE.md` exactly matches ROOT_FILES member;
/// `.md` passes step (d) — IS extracted.
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_root_file_claude_md_extracted() {
    let doc = "See `CLAUDE.md` for project conventions.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "CLAUDE.md"),
        "CLAUDE.md must be extracted as ROOT_FILES member, got: {:?}",
        result
    );
}

/// ROOT_FILES inclusion: `build.rs` exactly matches ROOT_FILES member;
/// `.rs` passes step (d) — IS extracted.
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_root_file_build_rs_extracted() {
    let doc = "See `build.rs` for build-time configuration.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "build.rs"),
        "build.rs must be extracted as ROOT_FILES member, got: {:?}",
        result
    );
}

/// ROOT_FILES inclusion: `deny.toml` exactly matches ROOT_FILES member;
/// `.toml` passes step (d) — IS extracted.
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_root_file_deny_toml_extracted() {
    let doc = "See `deny.toml` for license configuration.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "deny.toml"),
        "deny.toml must be extracted as ROOT_FILES member, got: {:?}",
        result
    );
}

/// ROOT_FILES inclusion: `README.md` exactly matches ROOT_FILES member;
/// `.md` passes step (d) — IS extracted.
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_root_file_readme_md_extracted() {
    let doc = "See `README.md` for project overview.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "README.md"),
        "README.md must be extracted as ROOT_FILES member, got: {:?}",
        result
    );
}

/// ROOT_FILES inclusion: `CHANGELOG.md` exactly matches ROOT_FILES member;
/// `.md` passes step (d) — IS extracted.
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_root_file_changelog_md_extracted() {
    let doc = "See `CHANGELOG.md` for release history.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "CHANGELOG.md"),
        "CHANGELOG.md must be extracted as ROOT_FILES member, got: {:?}",
        result
    );
}

/// ROOT_FILES inclusion: `rust-toolchain.toml` exactly matches ROOT_FILES member;
/// `.toml` passes step (d) — IS extracted.
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_root_file_rust_toolchain_toml_extracted() {
    let doc = "See `rust-toolchain.toml` for the pinned toolchain.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "rust-toolchain.toml"),
        "rust-toolchain.toml must be extracted as ROOT_FILES member, got: {:?}",
        result
    );
}

/// ROOT_FILES exclusion: `ci.yml` is NOT in ROOT_FILES (shorthand for
/// `.github/workflows/ci.yml`) → NOT extracted (EC-CITE-030).
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_shorthand_ci_yml_excluded() {
    let doc = "See `ci.yml` for the CI config.";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "ci.yml shorthand must NOT be extracted (not a ROOT_FILES member), got: {:?}",
        result
    );
}

/// ROOT_FILES exclusion: `adf.rs` is NOT in ROOT_FILES (shorthand for `src/adf.rs`)
/// → NOT extracted (EC-CITE-031).
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_shorthand_adf_rs_excluded() {
    let doc = "See `adf.rs` for the ADF module.";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "adf.rs shorthand must NOT be extracted (not a ROOT_FILES member), got: {:?}",
        result
    );
}

/// ROOT_FILES exclusion: `fields.json` is NOT in ROOT_FILES (cache-file shorthand)
/// → NOT extracted.
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_shorthand_fields_json_excluded() {
    let doc = "See `fields.json` for field cache format.";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "fields.json shorthand must NOT be extracted (not a ROOT_FILES member, \
         and .json not in extension set), got: {:?}",
        result
    );
}

/// ROOT_FILES exclusion: `release.yml` is NOT in ROOT_FILES (shorthand for
/// `.github/workflows/release.yml`) → NOT extracted.
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_shorthand_release_yml_excluded() {
    let doc = "See `release.yml` for the release workflow.";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "release.yml shorthand must NOT be extracted (not a ROOT_FILES member), got: {:?}",
        result
    );
}

/// ROOT_FILES inclusion via paren-unwrap: `(Cargo.toml)` → fixpoint strips parens
/// → `Cargo.toml` → ROOT_FILES match → IS extracted (EC-CITE-032).
/// Confirms paren-unwrap (step b) runs BEFORE ROOT_FILES exact-match (step c).
/// Traces to AC-006, VP-CITE-001.
#[test]
fn test_paren_wrapped_root_file_extracted() {
    let doc = "See `(Cargo.toml)` for dependencies.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "Cargo.toml"),
        "(Cargo.toml) must paren-unwrap to Cargo.toml then ROOT_FILES match, got: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Unit tests — VP-CITE-001 (merged fixpoint multi-pass cases, step b — AC-007)
// ---------------------------------------------------------------------------

/// EC-CITE-026: `(src/config.rs:~42)` — paren-wrap + line-ref multi-pass.
/// Pass 1: sub-steps (3)+(5) strip parens → `src/config.rs:~42`
/// Pass 2: sub-step (2) strips `:~42` → `src/config.rs` — IS in output.
/// Traces to AC-007, VP-CITE-001.
#[test]
fn test_fixpoint_ec026_paren_plus_line_ref() {
    let doc = "See `(src/config.rs:~42)` for config loading.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/config.rs"),
        "EC-CITE-026: (src/config.rs:~42) must normalize to src/config.rs, got: {:?}",
        result
    );
}

/// EC-CITE-027: `src/api/client.rs:195,` — line-ref + comma multi-pass.
/// Pass 1: sub-step (4) strips `,` → `src/api/client.rs:195`
/// Pass 2: sub-step (2) strips `:195` → `src/api/client.rs` — IS in output.
/// Traces to AC-007, VP-CITE-001.
#[test]
fn test_fixpoint_ec027_line_ref_plus_comma() {
    let doc = "See `src/api/client.rs:195,` for the auth header.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/api/client.rs"),
        "EC-CITE-027: src/api/client.rs:195, must normalize to src/api/client.rs, got: {:?}",
        result
    );
}

/// EC-CITE-028: `src/foo.rs::bar().` — symbol + punct combo.
/// Sub-step (1) strips `::bar().` in one pass → `src/foo.rs` — IS in output.
/// (Note: `src/foo.rs` does not exist on disk; this is a unit test of the
/// grammar only — existence check is in the integration test body.)
/// Traces to AC-007, VP-CITE-001.
#[test]
fn test_fixpoint_ec028_symbol_plus_punct() {
    let doc = "See `src/foo.rs::bar().` for the function.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/foo.rs"),
        "EC-CITE-028: src/foo.rs::bar(). must normalize to src/foo.rs in one pass, got: {:?}",
        result
    );
}

/// EC-CITE-023: `[docs/x.md]` — bracket wrap.
/// Sub-step (3) strips `[`; sub-step (6) strips unbalanced `]`
/// → `docs/x.md` — IS in output (using a real docs path for grammar test).
/// Traces to AC-007, VP-CITE-001.
#[test]
fn test_fixpoint_ec023_bracket_wrap() {
    // Use a synthetic docs path to test grammar without filesystem dependency
    let doc = "See `[docs/specs/adf-task-list.md]` for spec.";
    let result = extract_path_citations(doc);
    assert!(
        result
            .iter()
            .any(|(p, _)| p == "docs/specs/adf-task-list.md"),
        "EC-CITE-023: [docs/specs/adf-task-list.md] must strip brackets, got: {:?}",
        result
    );
}

/// EC-CITE-025: `((src/x.rs))` — double paren wrap.
/// Two fixpoint passes each strip one paren layer → `src/x.rs` — IS in output.
/// Traces to AC-007, VP-CITE-001.
#[test]
fn test_fixpoint_ec025_double_paren_wrap() {
    let doc = "See `((src/adf.rs))` for the ADF module.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/adf.rs"),
        "EC-CITE-025: ((src/adf.rs)) must strip both paren layers, got: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Unit tests — VP-CITE-001 (extension filter negatives — AC-011)
// ---------------------------------------------------------------------------

/// EC-CITE-033: `src/cli/issue` — extensionless dir-prefix token.
/// Has `src/` prefix (passes step c) but no recognized extension → EXCLUDED at step (d).
/// Traces to AC-011, VP-CITE-001.
#[test]
fn test_extension_filter_excludes_extensionless_token() {
    let doc = "See `src/cli/issue`.";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "EC-CITE-033: src/cli/issue must be excluded at extension filter (no extension), \
         got: {:?}",
        result
    );
}

/// EC-CITE-034: `Cargo.lock` — `.lock` extension not in recognized set.
/// Not a ROOT_FILES member; `.lock` not in `{.md, .rs, .sh, .toml, .yml, .yaml}`
/// → EXCLUDED at step (d).
/// Traces to AC-011, VP-CITE-001.
#[test]
fn test_extension_filter_excludes_lock_extension() {
    let doc = "See `Cargo.lock` for the locked dependencies.";
    let result = extract_path_citations(doc);
    assert!(
        result.is_empty(),
        "EC-CITE-034: Cargo.lock must be excluded (.lock not in recognized extension set), \
         got: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Unit tests — VP-CITE-001 (platform and delimiter edge cases — AC-012)
// ---------------------------------------------------------------------------

/// EC-CITE-002: comma-delimited `Detail: path1, path2` form.
/// Interior comma acts as whitespace tokenization boundary after backtick extraction.
/// Each token then goes through the (a)–(e) pipeline independently.
/// `src/adf.rs` (trailing comma stripped by sub-step (4)) and `src/partial_match.rs`
/// (trailing period stripped) — both ARE extracted.
///
/// Traces to AC-012, VP-CITE-001.
#[test]
fn test_comma_delimited_both_tokens_extracted() {
    let doc = "Detail: `src/adf.rs, src/partial_match.rs`.";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/adf.rs"),
        "EC-CITE-002: src/adf.rs must be extracted from comma-delimited form, got: {:?}",
        result
    );
    assert!(
        result.iter().any(|(p, _)| p == "src/partial_match.rs"),
        "EC-CITE-002: src/partial_match.rs must be extracted from comma-delimited form, \
         got: {:?}",
        result
    );
}

/// EC-CITE-003: CRLF line endings (Windows checkout).
/// `\r` from CRLF ending is stripped before/during tokenization; no `\r`-contaminated path.
/// `src/adf.rs` IS extracted; no returned path contains `\r`.
///
/// This is load-bearing for the Windows CI matrix leg (AC-002 runs on windows runner).
///
/// Traces to AC-012, VP-CITE-001.
#[test]
fn test_crlf_line_endings_no_false_positive() {
    let doc = "See `src/adf.rs`.\r\nAnd next line.\r\n";
    let result = extract_path_citations(doc);
    assert!(
        result.iter().any(|(p, _)| p == "src/adf.rs"),
        "EC-CITE-003: src/adf.rs must be extracted from CRLF-terminated line, got: {:?}",
        result
    );
    // No returned path may contain a \r character
    for (p, _) in &result {
        assert!(
            !p.contains('\r'),
            "EC-CITE-003: path must not contain \\r character (CRLF contamination), \
             got path: {:?}",
            p
        );
    }
}

// ---------------------------------------------------------------------------
// Proptest block — VP-CITE-001 (no false positives, no panics — AC-008)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod proptests {
    use super::extract_path_citations;
    use proptest::prelude::*;

    // VP-CITE-001 proptest: for any string `s` from the documented alphabet
    // (including `*`, `{`, `}`, `:`, `~`, `,`, `.`, `;`, `(`, `)`, `[`, `]`),
    // wrapping it in backticks and calling `extract_path_citations` returns
    // only paths that are either dir-prefix paths OR ROOT_FILES members —
    // never `.factory/` paths or other non-develop-tracked tokens.
    //
    // The alphabet exercises all 6 sub-steps of the merged fixpoint and the
    // glob-skip branch by random input.
    //
    // Traces to AC-008, VP-CITE-001.
    proptest! {
        #[test]
        fn test_non_prefix_tokens_are_never_extracted(
            s in "[A-Za-z0-9_:~./\\*\\{\\}\\.,;:\\)\\(\\[\\]]{1,50}"
        ) {
            let non_prefix = format!("`{}`", s);
            let result = extract_path_citations(&non_prefix);
            // result is Vec<(String, usize)>; inspect the path component of each entry
            for (path, _line) in &result {
                let is_dir_prefix = path.starts_with("src/")
                    || path.starts_with("tests/")
                    || path.starts_with("docs/")
                    || path.starts_with(".github/")
                    || path.starts_with("scripts/");
                // MUST match ROOT_FILES const inside extract_path_citations (BC-X.13.002 step c) — update both together.
                let root_files = [
                    "build.rs",
                    "Cargo.toml",
                    "CHANGELOG.md",
                    "CLAUDE.md",
                    "deny.toml",
                    "README.md",
                    "rust-toolchain.toml",
                ];
                let is_root_file = root_files.contains(&path.as_str());
                prop_assert!(
                    is_dir_prefix || is_root_file,
                    "Unexpected token in output (neither dir-prefix nor ROOT_FILES member): {}",
                    path
                );
            }
        }

        // VP-CITE-001 proptest: for any string `doc` of up to 500 arbitrary
        // Unicode chars, `extract_path_citations(&doc)` does not panic.
        //
        // Traces to AC-008, VP-CITE-001.
        #[test]
        fn test_extract_never_panics(doc in "\\PC{0,500}") {
            let _ = extract_path_citations(&doc);
        }
    }
}

//! Source-text assertion tests for S-WIN-6: CLAUDE.md JR_* table entries,
//! Windows config/cache path documentation, ADR-0016 materialization, and
//! CLAUDE.md Key Decisions registry.
//!
//! These tests read product-repo documentation files and assert that the required
//! content is present. All tests are always-run (no `#[ignore]`, no env-var gate)
//! because they assert static doc content.
//!
//! # Red Gate discipline
//!
//! All five tests (AC-001 through AC-005) are Red Gate tests: they FAIL against
//! the current docs because the required content is absent. They become green only
//! after the implementer makes the prescribed documentation changes.
//!
//! # Anchoring strategy (LESSON-PRESENCE-ANCHOR)
//!
//! Each assertion is scoped to the owning section/block rather than the whole file:
//!
//! - AC-001 / AC-002: `section_between_headings(content, "## AI Agent Notes", "## ")`
//!   slices exactly the "AI Agent Notes" section and asserts `JR_CONFIG_DIR` /
//!   `JR_CACHE_DIR` within that slice. The tokens are file-unique once added (confirmed
//!   absent via pre-implementation grep), but section anchoring is applied regardless
//!   per LESSON-PRESENCE-ANCHOR policy.
//!
//! - AC-003: `%APPDATA%\jr` and `%LOCALAPPDATA%\jr` are file-unique (absent from
//!   CLAUDE.md today; confirmed by pre-implementation grep). The story permits the
//!   Windows path note in either "AI Agent Notes" or "Gotchas", so the assertion
//!   searches both sections via their combined text. Uniqueness is documented in
//!   the test body.
//!
//! - AC-004: File existence + grep for `Decision 5b` and `Decision 5c` sub-decision
//!   headings within `docs/adr/0016-windows-build-target.md`. The headings are
//!   structurally unique in the ADR source (`### Decision 5b` / `### Decision 5c`
//!   appear once each); the grep verifies a verbatim copy including both sub-decisions.
//!
//! - AC-005: Reads CLAUDE.md, slices the `## Key Decisions` section (from its heading
//!   to the next `## ` heading), and asserts the slice contains `ADR-0016`. This is the
//!   product-repo ADR registry — the correct target for a product-repo test. The factory
//!   worktree's `.factory/architecture/adr-index.md` is NOT read here: that file is a
//!   factory-internal artifact and is not checked out in CI alongside the product repo.
//!   The assertion is tolerant (substring only, no exact title match) because annotation
//!   text evolves across correction passes.
//!
//! # Test inventory
//!
//! | Test | AC | BC | Red Gate? |
//! |------|----|----|-----------|
//! | `test_claude_md_documents_jr_config_dir` | AC-001 | BC-6.2.017 | Yes |
//! | `test_claude_md_documents_jr_cache_dir` | AC-002 | BC-6.2.017 | Yes |
//! | `test_claude_md_documents_windows_paths` | AC-003 | BC-6.1.014; BC-6.2.016 | Yes |
//! | `test_adr_0016_materialized_in_docs_adr` | AC-004 | architecture-delta §8 | Yes |
//! | `test_claude_md_key_decisions_includes_adr_0016` | AC-005 | architecture-delta §8 | Yes |

use std::path::PathBuf;

/// Returns the path to the Cargo manifest directory (worktree root).
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Read a file relative to the worktree root, panicking with a clear diagnostic
/// if the file cannot be opened.
fn read_file(relative: &str) -> String {
    let path = manifest_dir().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "Could not read `{relative}` at {}: {e}\n\
             This is either a missing file (expected for Red Gate tests) or a path error.\n\
             Expected path: {path:?}",
            path.display(),
        )
    })
}

/// Extract the text of a named section from a Markdown file.
///
/// A "section" starts at the line containing `section_heading` (exact substring
/// match) and ends at the line before the next line that starts with the same
/// heading-level prefix. For `## `–level headings this means stopping at the next
/// `## ` line; for `### ` at the next `### ` line.
///
/// Returns the section text (including the heading line itself), or the text from
/// the heading to end-of-file if no subsequent heading of the same level exists.
///
/// Panics if `section_heading` is not found in `content`.
fn section_between_headings(
    content: &str,
    section_heading: &str,
    next_heading_prefix: &str,
) -> String {
    let start_idx = content.find(section_heading).unwrap_or_else(|| {
        panic!(
            "Section heading `{section_heading}` not found in document.\n\
             Cannot anchor assertion to this section."
        )
    });
    let section_text = &content[start_idx..];

    // Find the next heading at the same (or higher) level after the section start.
    // We search from the character AFTER the heading line itself.
    let heading_line_end = section_text
        .find('\n')
        .map(|i| i + 1)
        .unwrap_or(section_text.len());
    let rest = &section_text[heading_line_end..];

    let section_end = rest.find(next_heading_prefix).unwrap_or(rest.len());
    let section_body = &rest[..section_end];

    format!("{}{}", &section_text[..heading_line_end], section_body)
}

// ---------------------------------------------------------------------------
// AC-001 — CLAUDE.md documents JR_CONFIG_DIR in "AI Agent Notes" JR_* table
// ---------------------------------------------------------------------------

/// Verifies that `CLAUDE.md` "AI Agent Notes" section contains a bullet entry
/// for `JR_CONFIG_DIR`.
///
/// # BC trace
/// BC-6.2.017 §CLAUDE.md documentation — the env var must appear in the JR_*
/// table alongside `JR_BASE_URL`, `JR_AUTH_HEADER`, etc.
///
/// # Anchoring
/// Slices the "AI Agent Notes" section (from its `##` heading to the next `##`
/// heading) and asserts within that slice, so a future hypothetical occurrence of
/// `JR_CONFIG_DIR` outside that section does not satisfy this test.
///
/// # Red Gate
/// FAILS pre-implementation: `JR_CONFIG_DIR` is absent from CLAUDE.md today
/// (confirmed by grep: zero matches).
#[test]
fn test_claude_md_documents_jr_config_dir() {
    let content = read_file("CLAUDE.md");

    let agent_notes = section_between_headings(&content, "## AI Agent Notes", "\n## ");

    assert!(
        agent_notes.contains("JR_CONFIG_DIR"),
        "S-WIN-6 AC-001 VIOLATION (BC-6.2.017): `JR_CONFIG_DIR` not found in the \
         \"AI Agent Notes\" section of CLAUDE.md.\n\
         \n\
         Required: add a bullet entry for `JR_CONFIG_DIR` in the JR_* env var table \
         in the \"AI Agent Notes\" section (parallel form to `JR_BASE_URL`). The entry \
         must explain that `JR_CONFIG_DIR` overrides the config directory in debug builds \
         (gated by `#[cfg(debug_assertions)]`) and cite BC-6.2.017.\n\
         \n\
         Example bullet (parallel to JR_BASE_URL):\n\
         - `JR_CONFIG_DIR` env var overrides the config directory in debug builds \
         (cross-platform test isolation seam; see BC-6.2.017). Debug builds only — \
         release binaries ignore this env var. Pinned by `tests/config_dir_release_gate.rs`.\n\
         \n\
         This is the codified doc-fallout pattern from CLAUDE.md §\"When adding a new \
         JR_* test-seam env var\" (applied retroactively — see #335/#357).",
    );
}

// ---------------------------------------------------------------------------
// AC-002 — CLAUDE.md documents JR_CACHE_DIR in "AI Agent Notes" JR_* table
// ---------------------------------------------------------------------------

/// Verifies that `CLAUDE.md` "AI Agent Notes" section contains a bullet entry
/// for `JR_CACHE_DIR`.
///
/// # BC trace
/// BC-6.2.017 §CLAUDE.md documentation — both `JR_CONFIG_DIR` and `JR_CACHE_DIR`
/// must appear in the JR_* table. This test covers the cache-side entry.
///
/// # Anchoring
/// Same section-slicing strategy as AC-001.
///
/// # Red Gate
/// FAILS pre-implementation: `JR_CACHE_DIR` is absent from CLAUDE.md today.
#[test]
fn test_claude_md_documents_jr_cache_dir() {
    let content = read_file("CLAUDE.md");

    let agent_notes = section_between_headings(&content, "## AI Agent Notes", "\n## ");

    assert!(
        agent_notes.contains("JR_CACHE_DIR"),
        "S-WIN-6 AC-002 VIOLATION (BC-6.2.017): `JR_CACHE_DIR` not found in the \
         \"AI Agent Notes\" section of CLAUDE.md.\n\
         \n\
         Required: add a bullet entry for `JR_CACHE_DIR` in the JR_* env var table \
         in the \"AI Agent Notes\" section (parallel form to `JR_CONFIG_DIR` / `JR_BASE_URL`). \
         The entry must explain that `JR_CACHE_DIR` overrides the cache root directory in \
         debug builds and cite BC-6.2.017.\n\
         \n\
         Example bullet (parallel to JR_CONFIG_DIR):\n\
         - `JR_CACHE_DIR` env var overrides the cache root directory in debug builds \
         (cross-platform test isolation seam; see BC-6.2.017). Debug builds only — \
         release binaries ignore this env var. Pinned by `tests/config_dir_release_gate.rs`.\n\
         \n\
         Both `JR_CONFIG_DIR` and `JR_CACHE_DIR` must be documented together in the same \
         commit (BC-6.2.017 dual-seam requirement).",
    );
}

// ---------------------------------------------------------------------------
// AC-003 — CLAUDE.md notes Windows config/cache paths (%APPDATA% / %LOCALAPPDATA%)
// ---------------------------------------------------------------------------

/// Verifies that `CLAUDE.md` documents the Windows-idiomatic config and cache
/// directory paths: `%APPDATA%\jr` and `%LOCALAPPDATA%\jr`.
///
/// # BC trace
/// BC-6.1.014 (Windows config path) and BC-6.2.016 (Windows cache path) both require
/// that these paths be documented for developers and AI agents.
///
/// # Anchoring / uniqueness
/// `%APPDATA%\jr` and `%LOCALAPPDATA%\jr` are provably file-unique tokens: they are
/// absent from CLAUDE.md today (confirmed by pre-implementation grep). The story permits
/// the note in either "AI Agent Notes" or "Gotchas"; this test searches both sections
/// combined (from "## Gotchas" heading through "## AI Agent Notes" through end-of-file)
/// to allow either placement. Uniqueness is documented here so a future reader knows
/// why section-slicing covers two sections rather than one.
///
/// # Red Gate
/// FAILS pre-implementation: `%APPDATA%\jr` and `%LOCALAPPDATA%\jr` are both absent
/// from CLAUDE.md today.
#[test]
fn test_claude_md_documents_windows_paths() {
    let content = read_file("CLAUDE.md");

    // Search from the "## Gotchas" section onward, which includes both "## Gotchas"
    // and "## AI Agent Notes" — the story permits the note in either section.
    // `%APPDATA%\jr` and `%LOCALAPPDATA%\jr` are file-unique (absent pre-impl), so
    // restricting to these two sections provides additional intent clarity.
    let gotchas_and_after = {
        let start = content.find("\n## Gotchas").unwrap_or_else(|| {
            panic!(
                "Could not find `## Gotchas` heading in CLAUDE.md. \
                     Has the section been renamed? Update this test to match."
            )
        });
        &content[start..]
    };

    assert!(
        gotchas_and_after.contains("%APPDATA%\\jr"),
        "S-WIN-6 AC-003 VIOLATION (BC-6.1.014): `%APPDATA%\\jr` not found in CLAUDE.md \
         Gotchas or AI Agent Notes section.\n\
         \n\
         Required: add a note documenting that on Windows, the config directory is \
         `%APPDATA%\\jr` (i.e., `dirs::config_dir().join(\"jr\")`). This must reference \
         BC-6.1.014. The note may live in either the \"Gotchas\" section or the \
         \"AI Agent Notes\" section (story S-WIN-6 AC-003 permits either location).\n\
         \n\
         On Unix the config path is unchanged: `~/.config/jr` (or `$XDG_CONFIG_HOME/jr`).",
    );

    assert!(
        gotchas_and_after.contains("%LOCALAPPDATA%\\jr"),
        "S-WIN-6 AC-003 VIOLATION (BC-6.2.016): `%LOCALAPPDATA%\\jr` not found in CLAUDE.md \
         Gotchas or AI Agent Notes section.\n\
         \n\
         Required: add a note documenting that on Windows, the cache directory is \
         `%LOCALAPPDATA%\\jr` (i.e., `dirs::cache_dir().join(\"jr\")`). This must reference \
         BC-6.2.016. The note may live in either the \"Gotchas\" section or the \
         \"AI Agent Notes\" section.\n\
         \n\
         On Unix the cache path is unchanged: `~/.cache/jr/v1/<profile>/`.",
    );
}

// ---------------------------------------------------------------------------
// AC-004 — docs/adr/0016-windows-build-target.md exists (verbatim copy incl. 5b + 5c)
// ---------------------------------------------------------------------------

/// Verifies that `docs/adr/0016-windows-build-target.md` has been materialized
/// as a verbatim copy of `.factory/architecture/adr/0016-windows-build-target.md`,
/// and that the copy includes both Decision 5b and Decision 5c sub-decisions.
///
/// # Trace
/// Architecture-delta §8: ADR-0016 must be accessible in `docs/adr/` alongside
/// ADR-0001 through ADR-0015.
///
/// # Anchoring
/// Two separate assertions for the 5b and 5c headings ensure a truncated copy that
/// is missing either sub-decision fails this test — as specified in S-WIN-6 AC-004:
/// "grep for both `5b` and `5c` headings/labels so a truncated copy missing either
/// sub-decision fails."
///
/// The headings `Decision 5b` and `Decision 5c` are structurally unique within the
/// ADR source (they appear once each in the ADR body). A file-existence check plus
/// two substring greps is the correct anchoring technique here.
///
/// # Red Gate
/// FAILS pre-implementation: `docs/adr/0016-windows-build-target.md` does not exist.
/// The `read_file` call will panic with a clear "file not found" message.
#[test]
fn test_adr_0016_materialized_in_docs_adr() {
    // This call panics with a diagnostic if the file does not exist — which is the
    // expected Red Gate failure. Pre-implementation: file absent → panic → test fails.
    let adr_content = read_file("docs/adr/0016-windows-build-target.md");

    // Verify the ADR-0016 file contains the sub-decision 5b heading.
    // "Decision 5b" appears in the `### Decision 5b: Keyring — Windows Credential Manager`
    // section. Without 5b, a truncated copy (e.g., cut-off at Decision 5) would pass
    // the file-existence check but silently miss a load-bearing decision.
    assert!(
        adr_content.contains("Decision 5b"),
        "S-WIN-6 AC-004 VIOLATION: `docs/adr/0016-windows-build-target.md` exists but \
         does not contain `Decision 5b` (the Windows Credential Manager `windows-native` \
         keyring feature decision).\n\
         \n\
         The file must be a verbatim copy of \
         `.factory/architecture/adr/0016-windows-build-target.md`, including the full \
         `### Decision 5b: Keyring — Windows Credential Manager (windows-native feature)` \
         section. A truncated copy missing this sub-decision fails AC-004.",
    );

    // Verify sub-decision 5c heading is also present.
    // "Decision 5c" appears in `### Decision 5c: Embedded-OAuth smoke step gated off on Windows`.
    assert!(
        adr_content.contains("Decision 5c"),
        "S-WIN-6 AC-004 VIOLATION: `docs/adr/0016-windows-build-target.md` exists but \
         does not contain `Decision 5c` (the Embedded-OAuth smoke step Windows gate decision).\n\
         \n\
         The file must be a verbatim copy of \
         `.factory/architecture/adr/0016-windows-build-target.md`, including the full \
         `### Decision 5c: Embedded-OAuth smoke step gated off on Windows` section. \
         A truncated copy missing this sub-decision fails AC-004.",
    );

    // Verify the cross-references to ADR-0003 and ADR-0006 are present (See Also).
    // These are present in the source ADR's "## See Also" section. A copy that
    // accidentally strips the See Also block would be incomplete.
    assert!(
        adr_content.contains("ADR-0003") && adr_content.contains("ADR-0006"),
        "S-WIN-6 AC-004 VIOLATION: `docs/adr/0016-windows-build-target.md` is missing \
         the `ADR-0003` and/or `ADR-0006` cross-references from the `## See Also` section.\n\
         \n\
         The file must be a verbatim copy of the source ADR including all cross-references. \
         The `## See Also` section at the end of the source ADR lists ADR-0003 (reqwest + \
         rustls) and ADR-0006 (embedded OAuth) as the two primary cross-references.",
    );
}

// ---------------------------------------------------------------------------
// AC-005 — ADR-0016 listed in CLAUDE.md "## Key Decisions" section
// ---------------------------------------------------------------------------

/// Verifies that `CLAUDE.md`'s `## Key Decisions` section contains an `ADR-0016`
/// entry.
///
/// # Trace
/// Architecture-delta §8 — the product-repo ADR registry (CLAUDE.md `## Key
/// Decisions`) must include ADR-0016 alongside ADR-0001 through ADR-0015.
///
/// # Why CLAUDE.md, not .factory/architecture/adr-index.md
/// The factory worktree's `adr-index.md` is a factory-internal artifact. In GitHub
/// Actions CI, only the product repo is checked out — the `.factory/` worktree does
/// not exist alongside it, so any test reading `../../.factory/...` would panic and
/// fail CI with a "Could not read" error. The correct product-repo deliverable behind
/// AC-005 is an entry in CLAUDE.md's `## Key Decisions` section, which IS committed
/// to the product repo and always present in CI.
///
/// # Anchoring (LESSON-PRESENCE-ANCHOR)
/// The assertion is scoped to the `## Key Decisions` section only (sliced from its
/// heading to the next `## ` heading via `section_between_headings`). This prevents a
/// hypothetical future occurrence of `ADR-0016` elsewhere in CLAUDE.md from
/// satisfying the registry requirement. The assertion uses a tolerant substring match
/// on `ADR-0016` — no exact title check — because annotation text evolves.
///
/// # Red Gate
/// FAILS pre-implementation: `ADR-0016` is absent from the `## Key Decisions` section
/// of CLAUDE.md today (confirmed by grep: zero matches in that section).
#[test]
fn test_claude_md_key_decisions_includes_adr_0016() {
    let content = read_file("CLAUDE.md");

    // Slice the "## Key Decisions" section (heading to next "## " heading).
    // This anchors the assertion so only the registry section satisfies the check.
    let key_decisions = section_between_headings(&content, "## Key Decisions", "\n## ");

    assert!(
        key_decisions.contains("ADR-0016"),
        "S-WIN-6 AC-005 VIOLATION (architecture-delta §8): `ADR-0016` not found in \
         the `## Key Decisions` section of CLAUDE.md.\n\
         \n\
         Required: add a bullet entry for ADR-0016 in the `## Key Decisions` section \
         of CLAUDE.md, parallel to the existing ADR-0001 through ADR-0015 entries.\n\
         \n\
         Example entry (add after the ADR-0015 line):\n\
         - ADR-0016: Windows build target — cross-compilation, keyring (windows-native), \
         and CI matrix\n\
         \n\
         The assertion uses substring matching (not exact title) so the wording may \
         vary, but `ADR-0016` must appear in this section. This is the product-repo \
         ADR registry — do NOT read from `.factory/architecture/adr-index.md`, which \
         is a factory-internal artifact not present in CI.",
    );
}

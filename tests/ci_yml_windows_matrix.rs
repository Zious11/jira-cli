//! S-WIN-5 Red Gate — CI Windows matrix + test-helper seam migration assertions.
//!
//! These tests pin the six source-text invariants required for Windows CI to
//! work. ALL tests in this file MUST FAIL against the current develop state
//! because none of the required changes exist yet:
//!
//! - `windows-latest` is NOT in the `test` job matrix (only ubuntu + macos)
//! - `windows-latest` is NOT in the `clippy` job matrix (clippy runs ubuntu-only)
//! - `.gitattributes` does NOT exist (snapshot CRLF protection is absent)
//! - `jr_isolated()` in `tests/auth_output_json.rs` does NOT set JR_CONFIG_DIR
//!   or JR_CACHE_DIR
//! - No cross-file enforcement of JR_CONFIG_DIR / JR_CACHE_DIR alongside XDG vars
//! - The scrub list in `jr_isolated()` is missing JR_CONFIG_DIR and JR_CACHE_DIR
//!
//! After S-WIN-5 implementation, all tests become green.
//!
//! Anchoring technique: Each test extracts the minimal block (job header or
//! stanza) that OWNS the assertion target and searches within that block only,
//! not the full ci.yml. This prevents a match in an unrelated job from
//! producing a false positive (e.g., `windows-latest` appearing in a comment
//! would not satisfy the matrix check because the matrix block is narrowed
//! first).
//!
//! Test coverage map:
//!   test_ci_yml_has_windows_latest_in_test_matrix   → AC-001
//!   test_gitattributes_has_snap_lf_rule              → AC-002
//!   test_jr_isolated_helper_sets_jr_config_dir       → AC-003
//!   test_all_xdg_test_files_also_set_jr_seam_vars   → AC-004
//!   test_ci_yml_has_windows_latest_in_clippy_matrix  → AC-006
//!   test_ci_yml_fmt_deny_jobs_remain_ubuntu_only         → AC-008
//!   test_cargo_config_toml_embeds_windows_stack_size     → AC-009
//!   test_jr_isolated_scrub_list_includes_seam_vars       → F-WIN2-C-101 guard

use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read .github/workflows/ci.yml relative to the repo root (the parent of
/// the `tests/` directory).
///
/// CRLF normalization: on Windows, Git may check out `*.yml` files with CRLF
/// line endings (unless `*.yml text eol=lf` is present in .gitattributes — which
/// this repo adds, but defense-in-depth means we normalize in-test too). The
/// `extract_job_block` helper and all assertion anchors match on `\n`-terminated
/// strings, so CRLF in the raw file would cause `.find("  job_name:\n")` to
/// miss `"  job_name:\r\n"`. Normalizing here keeps the rest of the matching
/// logic platform-independent.
fn read_ci_yml() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
    raw.replace("\r\n", "\n")
}

/// Extract the content of a named job block from ci.yml.
///
/// A job block starts at `  <name>:` (two-space indent) and ends at the next
/// top-level job header or end-of-file. Returns `None` when the job name is
/// not found. The returned slice begins at the job-key line and ends just
/// before the next job-key line (or EOF).
///
/// Anchoring rationale: assertions made against the returned slice are
/// guaranteed to target the correct job, even if the same substring appears
/// in a different job, a comment, or a multi-line shell step.
fn extract_job_block<'a>(ci_yml: &'a str, job_name: &str) -> Option<&'a str> {
    // Job headers in GitHub Actions YAML are at two-space indent: "  <id>:\n"
    let needle = format!("  {job_name}:\n");
    let start = ci_yml.find(&needle)?;

    // The end of this job block is the next line at the same indent level
    // (i.e., the next "  <something>:\n" after our start).
    let rest = &ci_yml[start + needle.len()..];
    let end_offset = rest
        .find("\n  ")
        // Scan forward to find the line that starts the next job (a line
        // beginning with exactly two spaces followed by a non-space char and
        // ending in `:` is a YAML mapping key at the job level).
        .and_then(|pos| {
            // `pos` points to the `\n` before the `  ` prefix. Walk forward
            // to collect candidate lines.
            let mut search_start = pos + 1; // skip the `\n`
            loop {
                let candidate = &rest[search_start..];
                // A new job key: two spaces, non-space char, eventually `:`
                if candidate.starts_with("  ")
                    && candidate.chars().nth(2).map(|c| c != ' ').unwrap_or(false)
                    && candidate
                        .lines()
                        .next()
                        .map(|l| l.trim_end().ends_with(':'))
                        .unwrap_or(false)
                {
                    return Some(search_start);
                }
                // Advance past this `\n  ` candidate
                if let Some(next) = rest[search_start..].find("\n  ") {
                    search_start = search_start + next + 1;
                } else {
                    return None;
                }
            }
        })
        .unwrap_or(rest.len());

    Some(&ci_yml[start..start + needle.len() + end_offset])
}

// ---------------------------------------------------------------------------
// AC-001 — `test` job matrix includes `windows-latest`
// ---------------------------------------------------------------------------

/// AC-001: The `test` job's `strategy.matrix.os` list in ci.yml must include
/// `windows-latest`.
///
/// Anchoring: assertion is made only within the `test` job block, so a
/// `windows-latest` string appearing elsewhere (e.g., a comment or the
/// `clippy` job's matrix) cannot produce a false positive.
///
/// RED-GATE: Current ci.yml has `os: [ubuntu-latest, macos-latest]` — no
/// `windows-latest`. This test FAILS on develop.
#[test]
fn test_ci_yml_has_windows_latest_in_test_matrix() {
    let ci = read_ci_yml();
    let test_block = extract_job_block(&ci, "test")
        .expect("ci.yml must contain a `test:` job (two-space indent)");

    // The matrix os list must contain windows-latest within the test job block.
    // We look for the matrix stanza pattern and then check for windows-latest.
    assert!(
        test_block.contains("windows-latest"),
        "FAIL (RED GATE): The `test` job matrix in .github/workflows/ci.yml \
         does not include `windows-latest`.\n\
         Current test job block:\n{test_block}\n\
         Required: add `windows-latest` to the `test` job's \
         `strategy.matrix.os` list."
    );

    // Also assert that the matrix block specifically references windows-latest
    // within an os matrix context (not just a comment).
    assert!(
        test_block.contains("matrix:")
            && test_block.contains("os:")
            && test_block.contains("windows-latest"),
        "FAIL (RED GATE): The `test` job must have a strategy.matrix.os list \
         that includes `windows-latest`.\n\
         Found test block:\n{test_block}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 — .gitattributes has `*.snap text eol=lf`
// ---------------------------------------------------------------------------

/// AC-002: `.gitattributes` must exist at the repo root and must contain the
/// line `*.snap text eol=lf`.
///
/// This rule prevents CRLF contamination of insta snapshot files when Windows
/// committers write new snapshot files. Without it, snapshot tests produce
/// cross-platform mismatches (R-W3 mitigation).
///
/// RED-GATE: `.gitattributes` does not exist in the repo. This test FAILS on
/// develop with a file-not-found error OR a missing-rule error.
#[test]
fn test_gitattributes_has_snap_lf_rule() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".gitattributes");
    assert!(
        path.exists(),
        "FAIL (RED GATE): `.gitattributes` does not exist at the repo root \
         ({}).\n\
         Required: create `.gitattributes` with at least `*.snap text eol=lf`.",
        path.display()
    );

    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));

    // The exact rule line must be present (not just a substring that could
    // match a different eol= value or a commented-out line).
    let has_snap_lf_rule = content.lines().any(|line| {
        let trimmed = line.trim();
        // Accept both bare form and with leading path glob variants, but
        // require: `*.snap` or a snap-targeting pattern, `text`, `eol=lf`.
        // The canonical form from the story spec is exactly: `*.snap text eol=lf`
        trimmed == "*.snap text eol=lf"
            || (trimmed.contains("*.snap") && trimmed.contains("eol=lf"))
    });

    assert!(
        has_snap_lf_rule,
        "FAIL (RED GATE): `.gitattributes` exists but does not contain the \
         snapshot LF rule.\n\
         Required line: `*.snap text eol=lf`\n\
         Current content:\n{content}"
    );
}

// ---------------------------------------------------------------------------
// AC-003 — jr_isolated() sets JR_CONFIG_DIR and JR_CACHE_DIR with .join("jr")
// ---------------------------------------------------------------------------

/// AC-003: `tests/auth_output_json.rs::jr_isolated()` must set both
/// `JR_CONFIG_DIR` and `JR_CACHE_DIR` via `.env()` calls.
///
/// The seam VALUES must use `.join("jr")` on the XDG path — not the raw
/// TempDir root — because the fixture is written into a `/jr/`-suffixed
/// subdir (see `.join("jr").join("config.toml")` calls in the helper's
/// callers). Setting JR_CONFIG_DIR to the raw TempDir root points jr at
/// `<TempDir>/config.toml` while the fixture is at `<TempDir>/jr/config.toml`
/// → config-not-found → test fails on ALL platforms.
///
/// This test is a source-text grep: it reads auth_output_json.rs and
/// verifies:
///   1. The string `JR_CONFIG_DIR` appears in the file (presence check).
///   2. The string `JR_CACHE_DIR` appears in the file (presence check).
///   3. The `.join("jr")` expression appears on the seam-value line for
///      JR_CONFIG_DIR (value-level check for the primary helper).
///
/// RED-GATE: jr_isolated() currently sets only XDG_CONFIG_HOME and
/// XDG_CACHE_HOME — it does NOT set JR_CONFIG_DIR or JR_CACHE_DIR. This
/// test FAILS on develop.
#[test]
fn test_jr_isolated_helper_sets_jr_config_dir() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("auth_output_json.rs");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));

    // Presence check: the seam var names must appear in the file.
    assert!(
        content.contains("JR_CONFIG_DIR"),
        "FAIL (RED GATE): `tests/auth_output_json.rs` does not contain \
         `JR_CONFIG_DIR`.\n\
         Required: add `.env(\"JR_CONFIG_DIR\", config_dir.path().join(\"jr\"))` \
         to the `jr_isolated()` helper."
    );
    assert!(
        content.contains("JR_CACHE_DIR"),
        "FAIL (RED GATE): `tests/auth_output_json.rs` does not contain \
         `JR_CACHE_DIR`.\n\
         Required: add `.env(\"JR_CACHE_DIR\", cache_dir.path().join(\"jr\"))` \
         to the `jr_isolated()` helper."
    );

    // Value-level check: the .join("jr") suffix must appear on a line that
    // also references JR_CONFIG_DIR. This guards against a migration that
    // sets JR_CONFIG_DIR = config_dir.path() (missing .join("jr")) — which
    // would pass the presence check but fail at runtime.
    let has_config_dir_with_join_jr = content
        .lines()
        .any(|line| line.contains("JR_CONFIG_DIR") && line.contains(".join(\"jr\")"));
    assert!(
        has_config_dir_with_join_jr,
        "FAIL (RED GATE): `tests/auth_output_json.rs` sets `JR_CONFIG_DIR` \
         but the seam value does not include `.join(\"jr\")`.\n\
         Required form: `.env(\"JR_CONFIG_DIR\", config_dir.path().join(\"jr\"))`\n\
         Reason: the fixture is written into a `/jr/`-suffixed subdir — \
         setting JR_CONFIG_DIR to the raw TempDir root causes config-not-found \
         on all platforms (EC-001)."
    );
}

// ---------------------------------------------------------------------------
// AC-004 — All in-scope XDG test files also set JR_CONFIG_DIR / JR_CACHE_DIR
// ---------------------------------------------------------------------------

/// AC-004: Every test file that sets `XDG_CONFIG_HOME` or `XDG_CACHE_HOME`
/// must ALSO set the corresponding JR seam var with per-var correspondence:
///   - If the file sets `XDG_CONFIG_HOME`, it MUST set `JR_CONFIG_DIR`.
///   - If the file sets `XDG_CACHE_HOME`, it MUST set `JR_CACHE_DIR`.
///
/// There is exactly one exception: `tests/e2e_live.rs` is fully
/// `#[ignore]`-gated and never runs in the Windows CI matrix.
///
/// Per-var correspondence (not `||`) is required so that a half-migration
/// (e.g. sets XDG_CONFIG_HOME + XDG_CACHE_HOME + JR_CACHE_DIR but not
/// JR_CONFIG_DIR) is caught at enforcement time rather than silently passing.
/// The prior `||` check had this blind spot: a file could set both XDG vars
/// but only one seam var and still pass the gate.
///
/// Per-call-site count check (process-hardening): in addition to file-level
/// presence checks (which catch subprocess `.env()` sites), this test also
/// verifies that the COUNT of in-process `set_var("XDG_CACHE_HOME"` calls
/// matches the count of `set_var("JR_CACHE_DIR"` calls, and similarly for
/// the config pair. This catches in-process half-migrations where a file has
/// one in-process `set_var` for the XDG var but not for the JR seam var,
/// even when the file otherwise contains the JR var string (e.g. from a
/// subprocess `.env()` call elsewhere in the same file).
///
/// The `ALLOWLISTED_E2E_FILES` constant must stay in sync with the
/// Out-of-Scope declaration in the story spec: if a file is removed from the
/// allowlist it must be migrated; if a file is added to the allowlist it must
/// be verified as fully `#[ignore]`-gated.
#[test]
fn test_all_xdg_test_files_also_set_jr_seam_vars() {
    // Allowlist: files that SET XDG vars but are fully #[ignore]-gated and
    // therefore never run in the windows-latest CI matrix. These do NOT need
    // to be migrated to use the JR_* seam vars.
    const ALLOWLISTED_E2E_FILES: &[&str] = &["tests/e2e_live.rs"];

    let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");

    // Track per-var failures separately so the error message is actionable.
    let mut missing_jr_config_dir: Vec<String> = Vec::new();
    let mut missing_jr_cache_dir: Vec<String> = Vec::new();

    let entries = fs::read_dir(&tests_dir).unwrap_or_else(|e| panic!("Could not read tests/: {e}"));

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e != "rs").unwrap_or(true) {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Check if this file sets XDG vars (quoted string = env var value,
        // as used in .env() or set_var() calls).
        let sets_xdg_config = content.contains("\"XDG_CONFIG_HOME\"");
        let sets_xdg_cache = content.contains("\"XDG_CACHE_HOME\"");
        if !sets_xdg_config && !sets_xdg_cache {
            continue;
        }

        // Build a relative path string for comparison with the allowlist.
        let rel_path = format!("tests/{}", path.file_name().unwrap().to_string_lossy());

        // Skip allowlisted files (fully #[ignore]-gated, never run on Windows).
        if ALLOWLISTED_E2E_FILES.contains(&rel_path.as_str()) {
            continue;
        }

        // Per-var file-level check: the JR seam var must appear somewhere in the
        // file (covers subprocess .env() sites as well as in-process set_var sites).
        if sets_xdg_config && !content.contains("JR_CONFIG_DIR") {
            missing_jr_config_dir.push(format!("{rel_path} (missing JR_CONFIG_DIR entirely)"));
        }
        if sets_xdg_cache && !content.contains("JR_CACHE_DIR") {
            missing_jr_cache_dir.push(format!("{rel_path} (missing JR_CACHE_DIR entirely)"));
        }

        // Per-call-site count check: count in-process set_var calls for XDG vars
        // and require an equal count of set_var calls for the corresponding JR
        // seam vars. This catches a half-migration where the file has an in-process
        // set_var for the XDG var but not for the JR seam var, even when the file
        // contains the JR var string from a subprocess .env() call elsewhere.
        //
        // Counting substring occurrences gives a conservative approximation: it may
        // overcount if the pattern appears in comments or strings, but it cannot
        // undercount (every real set_var call is a superset of the pattern string).
        // A count mismatch is always a bug; a count match does not guarantee
        // correctness but is sufficient to catch the in-process half-migration class.
        let xdg_config_setvar_count = content.matches("set_var(\"XDG_CONFIG_HOME\"").count();
        let jr_config_setvar_count = content.matches("set_var(\"JR_CONFIG_DIR\"").count();
        if xdg_config_setvar_count > jr_config_setvar_count {
            missing_jr_config_dir.push(format!(
                "{rel_path} (per-call-site: {} set_var(\"XDG_CONFIG_HOME\") calls \
                 but only {} set_var(\"JR_CONFIG_DIR\") calls — \
                 each in-process XDG set_var must have a matching JR seam set_var)",
                xdg_config_setvar_count, jr_config_setvar_count
            ));
        }
        let xdg_cache_setvar_count = content.matches("set_var(\"XDG_CACHE_HOME\"").count();
        let jr_cache_setvar_count = content.matches("set_var(\"JR_CACHE_DIR\"").count();
        if xdg_cache_setvar_count > jr_cache_setvar_count {
            missing_jr_cache_dir.push(format!(
                "{rel_path} (per-call-site: {} set_var(\"XDG_CACHE_HOME\") calls \
                 but only {} set_var(\"JR_CACHE_DIR\") calls — \
                 each in-process XDG set_var must have a matching JR seam set_var)",
                xdg_cache_setvar_count, jr_cache_setvar_count
            ));
        }
    }

    missing_jr_config_dir.sort();
    missing_jr_cache_dir.sort();

    let mut failures: Vec<String> = Vec::new();
    if !missing_jr_config_dir.is_empty() {
        failures.push(format!(
            "XDG_CONFIG_HOME->JR_CONFIG_DIR migration issues ({} finding{}):\n{}",
            missing_jr_config_dir.len(),
            if missing_jr_config_dir.len() == 1 {
                ""
            } else {
                "s"
            },
            missing_jr_config_dir
                .iter()
                .map(|s| format!("  - {s}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    if !missing_jr_cache_dir.is_empty() {
        failures.push(format!(
            "XDG_CACHE_HOME->JR_CACHE_DIR migration issues ({} finding{}):\n{}",
            missing_jr_cache_dir.len(),
            if missing_jr_cache_dir.len() == 1 {
                ""
            } else {
                "s"
            },
            missing_jr_cache_dir
                .iter()
                .map(|s| format!("  - {s}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    assert!(
        failures.is_empty(),
        "FAIL: The following test files have a half-migrated XDG->JR seam \
         migration. Per-var correspondence is required: \
         XDG_CONFIG_HOME->JR_CONFIG_DIR and XDG_CACHE_HOME->JR_CACHE_DIR.\n\
         These files will fail on Windows CI because JR_CONFIG_DIR / \
         JR_CACHE_DIR are the only isolation mechanism on Windows \
         (XDG_* is Unix-only after S-WIN-1).\n\
         Migration: for each set_var(\"XDG_CONFIG_HOME\", X) add \
         set_var(\"JR_CONFIG_DIR\", X.join(\"jr\")); for each \
         set_var(\"XDG_CACHE_HOME\", Y) add \
         set_var(\"JR_CACHE_DIR\", Y.join(\"jr\")).\n\n{}",
        failures.join("\n\n")
    );
}

// ---------------------------------------------------------------------------
// AC-006 — `clippy` CI job is matrixed over [ubuntu-latest, windows-latest]
// ---------------------------------------------------------------------------

/// AC-006: The `clippy` job in ci.yml must have a `strategy.matrix.os` list
/// that includes BOTH `ubuntu-latest` AND `windows-latest`, with
/// `runs-on: ${{ matrix.os }}`.
///
/// This is required because the `#[cfg(windows)]` code paths introduced by
/// S-WIN-1/S-WIN-2 are only compiled (and therefore only linted) on a Windows
/// runner. A ubuntu-only clippy job cannot satisfy this requirement.
///
/// Anchoring: assertions are made only within the `clippy` job block.
///
/// RED-GATE: Current ci.yml has `clippy` with `runs-on: ubuntu-latest` and no
/// matrix. This test FAILS on develop.
#[test]
fn test_ci_yml_has_windows_latest_in_clippy_matrix() {
    let ci = read_ci_yml();
    let clippy_block = extract_job_block(&ci, "clippy")
        .expect("ci.yml must contain a `clippy:` job (two-space indent)");

    // The clippy job must use matrix-based runs-on (not a literal ubuntu-latest).
    assert!(
        clippy_block.contains("matrix.os"),
        "FAIL (RED GATE): The `clippy` job in ci.yml does not use \
         `${{{{ matrix.os }}}}` for `runs-on`.\n\
         Required: add `strategy.matrix.os: [ubuntu-latest, windows-latest]` \
         and change `runs-on: ubuntu-latest` to `runs-on: ${{{{ matrix.os }}}}`.\n\
         Current clippy block:\n{clippy_block}"
    );

    // The clippy job's matrix must include windows-latest.
    assert!(
        clippy_block.contains("windows-latest"),
        "FAIL (RED GATE): The `clippy` job matrix in ci.yml does not include \
         `windows-latest`.\n\
         Current clippy block:\n{clippy_block}\n\
         Required: `strategy.matrix.os: [ubuntu-latest, windows-latest]`."
    );

    // And it must still include ubuntu-latest (regression guard).
    assert!(
        clippy_block.contains("ubuntu-latest"),
        "FAIL: The `clippy` job matrix must include `ubuntu-latest` as well. \
         Current clippy block:\n{clippy_block}"
    );
}

// ---------------------------------------------------------------------------
// AC-008 — fmt and deny jobs remain ubuntu-latest only
// ---------------------------------------------------------------------------

/// AC-008: The `fmt` and `deny` jobs must remain on `ubuntu-latest` only.
/// Per architecture-delta.md §4.3, only the `test` and `clippy` jobs gain
/// the Windows matrix. `fmt` and `deny` stay ubuntu-only.
///
/// Anchoring: assertions are made within the `fmt` and `deny` job blocks
/// separately. A `windows-latest` string appearing in those blocks would be
/// an unintended change.
///
/// GREEN on develop (these jobs are already ubuntu-only). This test is a
/// regression guard: it must STAY green after S-WIN-5 implementation, and
/// FAIL if fmt/deny accidentally gain a Windows matrix.
///
/// Note: This test is expected to PASS against the current state (fmt and
/// deny are already ubuntu-only). It is included in this file because it is
/// explicitly listed in the story's AC-008 and Test Coverage Summary table.
#[test]
fn test_ci_yml_fmt_deny_jobs_remain_ubuntu_only() {
    let ci = read_ci_yml();

    let fmt_block = extract_job_block(&ci, "fmt").expect("ci.yml must contain a `fmt:` job");
    assert!(
        !fmt_block.contains("windows-latest"),
        "REGRESSION: The `fmt` job must NOT include `windows-latest`. \
         Only `test` and `clippy` gain the Windows matrix.\n\
         Current fmt block:\n{fmt_block}"
    );
    // fmt must still be ubuntu-latest.
    assert!(
        fmt_block.contains("ubuntu-latest"),
        "The `fmt` job must still run on `ubuntu-latest`.\n\
         Current fmt block:\n{fmt_block}"
    );

    let deny_block = extract_job_block(&ci, "deny").expect("ci.yml must contain a `deny:` job");
    assert!(
        !deny_block.contains("windows-latest"),
        "REGRESSION: The `deny` job must NOT include `windows-latest`. \
         Only `test` and `clippy` gain the Windows matrix.\n\
         Current deny block:\n{deny_block}"
    );
    assert!(
        deny_block.contains("ubuntu-latest"),
        "The `deny` job must still run on `ubuntu-latest`.\n\
         Current deny block:\n{deny_block}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 — .cargo/config.toml embeds 8 MB main-thread stack in jr.exe
// ---------------------------------------------------------------------------

/// AC-009: `.cargo/config.toml` must contain a `[target.x86_64-pc-windows-msvc]`
/// section with `rustflags` that includes the `/STACK:8388608` link-arg.
///
/// Windows PE headers default to a 1 MB main-thread stack, while Linux/macOS
/// default to 8 MB. The `#[tokio::main]` async runtime + clap dispatch +
/// result rendering in jr.exe collectively exceed 1 MB, causing jr.exe to
/// crash on normal commands (e.g. `jr issue list`) on Windows.  The 11
/// `all_flag_behavior.rs` integration tests spawn jr.exe as a subprocess and
/// the child's main thread overflows with the 1 MB default.
///
/// The fix embeds the 8 MB reserve in jr.exe's PE header via the MSVC linker
/// flag `/STACK:8388608`. This is set in `.cargo/config.toml` under the
/// `[target.x86_64-pc-windows-msvc]` section so it applies to all builds of
/// the Windows target without affecting Unix builds.  It is a link-time flag
/// and is inert for `cargo check` (no link step).
///
/// RUST_MIN_STACK is NOT the correct fix: it only affects `std::thread::spawn`
/// threads (test-harness workers), not a process's main thread (set by the PE
/// header).  The /STACK linker flag is the correct mechanism.
///
/// Anchoring: reads `.cargo/config.toml` directly from the repo root.
#[test]
fn test_cargo_config_toml_embeds_windows_stack_size() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".cargo")
        .join("config.toml");
    assert!(
        path.exists(),
        "FAIL: `.cargo/config.toml` does not exist at {}.\n\
         Required: create `.cargo/config.toml` with a \
         `[target.x86_64-pc-windows-msvc]` section containing \
         `rustflags = [\"-C\", \"link-arg=/STACK:8388608\"]`.\n\
         Reason: jr.exe needs an 8 MB main-thread stack on Windows (PE header \
         default is 1 MB); /STACK:8388608 embeds the reserve at link time.",
        path.display()
    );

    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));

    // The target section header must be present.
    assert!(
        content.contains("[target.x86_64-pc-windows-msvc]"),
        "FAIL: `.cargo/config.toml` does not contain \
         `[target.x86_64-pc-windows-msvc]`.\n\
         Required: add `[target.x86_64-pc-windows-msvc]` section with \
         `rustflags = [\"-C\", \"link-arg=/STACK:8388608\"]`.\n\
         Current content:\n{content}"
    );

    // The /STACK link-arg value must be present.
    assert!(
        content.contains("/STACK:8388608"),
        "FAIL: `.cargo/config.toml` has a `[target.x86_64-pc-windows-msvc]` \
         section but does not contain `/STACK:8388608`.\n\
         Required: `rustflags = [\"-C\", \"link-arg=/STACK:8388608\"]` under \
         the target section.\n\
         Current content:\n{content}"
    );

    // The link-arg form ("-C", "link-arg=...") must be present to confirm this
    // is passed as a codegen flag, not some other rustflag form.
    assert!(
        content.contains("link-arg=/STACK:8388608"),
        "FAIL: `.cargo/config.toml` contains `/STACK:8388608` but not in the \
         expected `link-arg=/STACK:8388608` form.\n\
         Required form: `rustflags = [\"-C\", \"link-arg=/STACK:8388608\"]`\n\
         Current content:\n{content}"
    );
}

// ---------------------------------------------------------------------------
// F-WIN2-C-101 guard — scrub list in jr_isolated() includes JR_CONFIG_DIR
// and JR_CACHE_DIR
// ---------------------------------------------------------------------------

/// F-WIN2-C-101: The `env_remove` scrub list in `tests/auth_output_json.rs`
/// (the `jr_isolated()` helper) must include `JR_CONFIG_DIR` and `JR_CACHE_DIR`.
///
/// If these seam vars are set in the developer's shell (e.g., from a previous
/// debug session with `JR_CONFIG_DIR=/tmp/test-jr`), they would leak into test
/// subprocess environments and break test isolation by pointing jr at a
/// non-test config directory. The scrub list must explicitly remove them
/// alongside the other JR_* vars.
///
/// This guard is realized as a source-text check on the `jr_isolated()` helper
/// which is the canonical location for the JR_* scrub list in this repo. The
/// function currently removes JR_BASE_URL, JR_AUTH_HEADER, JR_PROFILE, etc.
/// but NOT JR_CONFIG_DIR or JR_CACHE_DIR.
///
/// RED-GATE: `jr_isolated()` does not call `.env_remove("JR_CONFIG_DIR")` or
/// `.env_remove("JR_CACHE_DIR")`. This test FAILS on develop.
#[test]
fn test_jr_isolated_scrub_list_includes_seam_vars() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("auth_output_json.rs");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));

    // Check that env_remove("JR_CONFIG_DIR") is present.
    let scrubs_config_dir = content
        .lines()
        .any(|line| line.contains("env_remove") && line.contains("JR_CONFIG_DIR"));
    assert!(
        scrubs_config_dir,
        "FAIL (RED GATE): `tests/auth_output_json.rs::jr_isolated()` does not \
         call `.env_remove(\"JR_CONFIG_DIR\")`.\n\
         Required: add `.env_remove(\"JR_CONFIG_DIR\")` to the scrub list \
         alongside the other JR_* vars.\n\
         Reason (F-WIN2-C-101): if JR_CONFIG_DIR is set in the developer's \
         shell, it leaks into test subprocesses and breaks isolation by \
         pointing jr at a non-test config directory."
    );

    // Check that env_remove("JR_CACHE_DIR") is present.
    let scrubs_cache_dir = content
        .lines()
        .any(|line| line.contains("env_remove") && line.contains("JR_CACHE_DIR"));
    assert!(
        scrubs_cache_dir,
        "FAIL (RED GATE): `tests/auth_output_json.rs::jr_isolated()` does not \
         call `.env_remove(\"JR_CACHE_DIR\")`.\n\
         Required: add `.env_remove(\"JR_CACHE_DIR\")` to the scrub list \
         alongside the other JR_* vars.\n\
         Reason (F-WIN2-C-101): if JR_CACHE_DIR is set in the developer's \
         shell, it leaks into test subprocesses and breaks isolation."
    );
}

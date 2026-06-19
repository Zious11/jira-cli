//! S-FORK-OPS-BACKFILL-1 Red Gate — backfill-release.yml matrix parity and
//! check-then-upsert assertions.
//!
//! These tests pin the structural invariants required by
//! S-FORK-OPS-BACKFILL-1. ALL failing tests in this file MUST FAIL against the
//! current branch state because none of the required changes exist yet:
//!
//! - `x86_64-pc-windows-msvc` is NOT in `backfill-release.yml`'s build matrix
//!   (only four Unix/macOS targets exist)
//! - `gh release delete ... || true` IS present (the destructive delete+create
//!   pattern has not been replaced by check-then-upsert)
//! - `gh release view` (the upsert check) is NOT present
//! - `gh release upload` (the upsert upload branch) is NOT present
//! - `jr-*.zip` is NOT in the `Upload artifact` path block
//! - `isDraft` (the draft-release detection) is NOT present
//!
//! After S-FORK-OPS-BACKFILL-1 implementation, all tests become green.
//!
//! Anchoring technique: each test extracts the minimal block that OWNS the
//! assertion target and searches within that block only, not the full file.
//! This prevents a match in an unrelated job (e.g. a comment in the sign job)
//! from producing a false positive. Mirrors the technique established by
//! `tests/ci_yml_windows_matrix.rs` and `tests/release_yml_windows_matrix.rs`.
//!
//! # Test inventory
//!
//! | Test | AC | Drift Item |
//! |------|----|------------|
//! | `test_backfill_matrix_parity_matches_release_yml`        | AC-004 | WIN-TARGET |
//! | `test_backfill_build_matrix_contains_windows_target`     | AC-001 | WIN-TARGET |
//! | `test_backfill_upload_artifact_includes_zip`             | AC-001 | WIN-TARGET |
//! | `test_backfill_release_job_has_no_delete_command`        | AC-002 | DESTRUCTIVE |
//! | `test_backfill_release_job_has_no_or_true_silencer`      | AC-002 | DESTRUCTIVE |
//! | `test_backfill_release_job_has_upsert_view_check`        | AC-002 | DESTRUCTIVE |
//! | `test_backfill_release_job_has_upsert_upload_branch`     | AC-002 | DESTRUCTIVE |
//! | `test_backfill_release_job_zip_in_both_upsert_branches`  | AC-002 | DESTRUCTIVE |
//! | `test_backfill_release_job_has_draft_detection`          | AC-002 (EC-001) | DESTRUCTIVE |
//! | `test_backfill_build_step_declares_shell_bash`           | CR-001 | WIN-TARGET |
//! | `test_backfill_unix_package_step_declares_shell_bash`    | CR-002 | WIN-TARGET |

use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read `.github/workflows/backfill-release.yml` relative to the repo root.
///
/// CRLF normalization: on Windows, Git may check out `*.yml` files with CRLF
/// line endings. Normalizing to LF here keeps all substring searches
/// platform-independent — identical to the precedent in
/// `tests/ci_yml_windows_matrix.rs` and `tests/release_yml_windows_matrix.rs`.
fn read_backfill_yml() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/backfill-release.yml");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
    raw.replace("\r\n", "\n")
}

/// Read `.github/workflows/release.yml` relative to the repo root.
///
/// Same CRLF normalization as `read_backfill_yml`.
fn read_release_yml() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/release.yml");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
    raw.replace("\r\n", "\n")
}

/// Extract `target:` values from a workflow file's `jobs.build.strategy.matrix.include`
/// block.
///
/// Strategy: locate the `matrix:` header inside the `build:` job block, then
/// extract all `target:` lines that follow — stopping when we exit the `include:`
/// list (a line that returns to the same or lower indentation as `include:`).
///
/// This is a text-pattern extraction (not a full YAML parse) that follows the
/// implementation shape from `tests/ci_yml_windows_matrix.rs`: anchor to a named
/// YAML section, then match within it.
///
/// Returns a sorted `Vec<String>` of target triple strings for order-independent
/// comparison.
fn extract_build_matrix_targets(yaml_content: &str) -> Vec<String> {
    let mut targets = Vec::new();

    // Locate the build job block: find `  build:` at two-space indent.
    let build_needle = "  build:\n";
    let build_start = yaml_content
        .find(build_needle)
        .expect("Could not find `  build:` job in workflow YAML");

    // Within the build block, find the `matrix:` section.
    let build_section = &yaml_content[build_start..];
    let matrix_needle = "      matrix:\n";
    let matrix_rel = build_section
        .find(matrix_needle)
        .expect("Could not find `      matrix:` section in the build job");

    let matrix_section = &build_section[matrix_rel..];

    // Within the matrix section, find the `include:` list.
    let include_needle = "        include:\n";
    let include_rel = matrix_section
        .find(include_needle)
        .expect("Could not find `        include:` list in the matrix section");

    let include_section = &matrix_section[include_rel + include_needle.len()..];

    // Extract all `target:` values from the include list items.
    // Each item is a YAML mapping at 10-space indent (under `include:`).
    // We stop when we encounter a line that is not deeper than `include:` indentation
    // (i.e., a line starting with 8 or fewer non-space characters at the root level,
    // which signals the end of the include list and the start of the next key).
    for line in include_section.lines() {
        // A line at 8-space indent (or less non-blank) means we have left the include
        // list. The include items are at 10 spaces; their keys are `- target:` or
        // just `  os:` etc. at 10 spaces.
        // Stop when we see a line that has non-whitespace at column 8 or less
        // (indicating we've returned to matrix: level or above).
        if !line.is_empty() {
            let leading_spaces = line.len() - line.trim_start().len();
            if leading_spaces <= 8 {
                break;
            }
        }

        // Extract `target:` value from list item lines.
        // The canonical form is `          - target: x86_64-apple-darwin`
        // (10-space indent, dash, space, `target:`, space, value).
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- target:") {
            let target = rest.trim().to_string();
            if !target.is_empty() {
                targets.push(target);
            }
        }
        // Also handle the form where target is on a separate line from the dash:
        // ```
        //           - target: x86_64-apple-darwin
        // ```
        // The above strip_prefix already covers this. No additional case needed.
    }

    targets.sort();
    targets
}

/// Extract the content of the `jobs.release` block from a workflow YAML string.
///
/// A job block starts at `  <name>:` (two-space indent) and ends at the next
/// two-space-indented job key or end-of-file. Returns `None` when the job name
/// is not found.
///
/// Anchoring rationale: assertions made against the returned slice are
/// guaranteed to target the correct job, even if the same substring appears
/// in a different job, a comment, or a multi-line shell step.
fn extract_job_block<'a>(yaml: &'a str, job_name: &str) -> Option<&'a str> {
    let needle = format!("  {job_name}:\n");
    let start = yaml.find(&needle)?;

    let rest = &yaml[start + needle.len()..];
    let end_offset = rest
        .find("\n  ")
        .and_then(|pos| {
            let mut search_start = pos + 1; // skip the `\n`
            loop {
                let candidate = &rest[search_start..];
                // A new job key at two-space indent: two spaces, non-space char, `:`
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
                if let Some(next) = rest[search_start..].find("\n  ") {
                    search_start = search_start + next + 1;
                } else {
                    return None;
                }
            }
        })
        .unwrap_or(rest.len());

    Some(&yaml[start..start + needle.len() + end_offset])
}

// ---------------------------------------------------------------------------
// AC-004 — Matrix parity: backfill-release.yml must have the same five targets
// as release.yml (the canonical parity guard)
// ---------------------------------------------------------------------------

/// AC-004 (REQUIRED): `backfill-release.yml` and `release.yml` must have
/// exactly the same five build targets in their `jobs.build.strategy.matrix.include`
/// list:
///   - x86_64-apple-darwin
///   - aarch64-apple-darwin
///   - x86_64-unknown-linux-gnu
///   - aarch64-unknown-linux-gnu
///   - x86_64-pc-windows-msvc
///
/// Anchoring: targets are extracted from the `build:` job's `matrix.include`
/// block only, so a `target:` key in a comment or another job cannot satisfy
/// this assertion.
///
/// RED GATE: `backfill-release.yml` currently has only four targets — the
/// Windows entry is missing. This test FAILS on the current branch.
///
/// After S-FORK-OPS-BACKFILL-1 implementation, this test becomes green.
///
/// Traces to: verification-delta § "Required New Test: backfill-release.yml
/// Matrix Parity Guard"; AC-004; drift item FORK-OPS-BACKFILL-WIN-TARGET.
#[test]
fn test_backfill_matrix_parity_matches_release_yml() {
    let backfill_yml = read_backfill_yml();
    let release_yml = read_release_yml();

    let backfill_targets = extract_build_matrix_targets(&backfill_yml);
    let release_targets = extract_build_matrix_targets(&release_yml);

    // The five canonical targets that must be present in both files.
    let expected_targets: Vec<String> = {
        let mut v = vec![
            "x86_64-apple-darwin".to_string(),
            "aarch64-apple-darwin".to_string(),
            "x86_64-unknown-linux-gnu".to_string(),
            "aarch64-unknown-linux-gnu".to_string(),
            "x86_64-pc-windows-msvc".to_string(),
        ];
        v.sort();
        v
    };

    // Assert release.yml has exactly the five expected targets (regression guard).
    assert_eq!(
        release_targets, expected_targets,
        "FAIL: `.github/workflows/release.yml` does not have exactly the five \
         expected targets.\n\
         Expected (sorted): {expected_targets:?}\n\
         Found in release.yml (sorted): {release_targets:?}\n\
         This should never happen — release.yml is the canonical reference. \
         If release.yml changed, update this test's expected_targets to match."
    );

    // Assert backfill-release.yml has the SAME five targets as release.yml
    // (set equality, order-independent — both vectors are already sorted).
    assert_eq!(
        backfill_targets,
        release_targets,
        "FAIL (RED GATE): `.github/workflows/backfill-release.yml` does not have \
         the same build targets as `.github/workflows/release.yml`.\n\
         \n\
         Expected (from release.yml, sorted): {release_targets:?}\n\
         Found in backfill-release.yml (sorted): {backfill_targets:?}\n\
         \n\
         Missing from backfill-release.yml: {:?}\n\
         Unexpected in backfill-release.yml: {:?}\n\
         \n\
         Required fix: add the following entry to `jobs.build.strategy.matrix.include` \
         in `.github/workflows/backfill-release.yml`:\n\
           - target: x86_64-pc-windows-msvc\n\
             os: windows-latest\n\
         \n\
         (S-FORK-OPS-BACKFILL-1 AC-004 / drift item FORK-OPS-BACKFILL-WIN-TARGET)",
        {
            let mut missing: Vec<_> = release_targets
                .iter()
                .filter(|t| !backfill_targets.contains(t))
                .collect();
            missing.sort();
            missing
        },
        {
            let mut extra: Vec<_> = backfill_targets
                .iter()
                .filter(|t| !release_targets.contains(t))
                .collect();
            extra.sort();
            extra
        }
    );
}

// ---------------------------------------------------------------------------
// AC-001 (WIN-TARGET) — Windows matrix entry is present
// ---------------------------------------------------------------------------

/// AC-001: `backfill-release.yml` `jobs.build.strategy.matrix.include` must
/// contain an entry with `target: x86_64-pc-windows-msvc` and `os: windows-latest`.
///
/// Anchoring: the assertion searches within the `build:` job block only,
/// preventing a `windows-latest` string in a comment or another job (e.g.
/// the homebrew job) from satisfying this check.
///
/// RED GATE: the Windows matrix entry is absent. This test FAILS on the
/// current branch.
///
/// Traces to: AC-001; drift item FORK-OPS-BACKFILL-WIN-TARGET.
#[test]
fn test_backfill_build_matrix_contains_windows_target() {
    let yml = read_backfill_yml();

    let build_block = extract_job_block(&yml, "build").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/backfill-release.yml` does not contain a \
             `build:` job (two-space indent).\n\
             Expected: a `build` job with a `strategy.matrix.include` section."
        )
    });

    // The Windows target triple must appear in the build job.
    assert!(
        build_block.contains("x86_64-pc-windows-msvc"),
        "FAIL (RED GATE): The `build` job matrix in `.github/workflows/backfill-release.yml` \
         does not contain `x86_64-pc-windows-msvc`.\n\
         \n\
         Required: add the following entry to `jobs.build.strategy.matrix.include`:\n\
           - target: x86_64-pc-windows-msvc\n\
             os: windows-latest\n\
         \n\
         Current build job block (first 500 chars):\n{}\n\
         \n\
         (S-FORK-OPS-BACKFILL-1 AC-001 / drift item FORK-OPS-BACKFILL-WIN-TARGET)",
        &build_block[..build_block.len().min(500)]
    );

    // The Windows runner must also be present.
    assert!(
        build_block.contains("windows-latest"),
        "FAIL (RED GATE): The `build` job in `backfill-release.yml` does not contain \
         `windows-latest`.\n\
         The Windows matrix row must specify `os: windows-latest`.\n\
         (S-FORK-OPS-BACKFILL-1 AC-001 / drift item FORK-OPS-BACKFILL-WIN-TARGET)"
    );
}

/// AC-001 (artifact coverage): The `Upload artifact` step in the `build` job
/// must include `jr-*.zip` in its `path:` block, so the Windows .zip artifact
/// is uploaded to GitHub Actions artifacts alongside .tar.gz and .sha256.
///
/// Anchoring: the assertion is scoped to the `build` job block.
///
/// RED GATE: the Upload artifact step currently only lists `jr-*.tar.gz` and
/// `jr-*.sha256`. This test FAILS on the current branch.
///
/// Traces to: AC-001; drift item FORK-OPS-BACKFILL-WIN-TARGET (Upload Artifact
/// path update in story tasks §Item 4).
#[test]
fn test_backfill_upload_artifact_includes_zip() {
    let yml = read_backfill_yml();

    let build_block = extract_job_block(&yml, "build").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/backfill-release.yml` does not contain a \
             `build:` job (two-space indent)."
        )
    });

    assert!(
        build_block.contains("jr-*.zip"),
        "FAIL (RED GATE): The `build` job's `Upload artifact` step in \
         `.github/workflows/backfill-release.yml` does not contain `jr-*.zip` \
         in its `path:` block.\n\
         \n\
         Required: add `jr-*.zip` to the `path:` block of the `Upload artifact` step:\n\
           path: |\n\
             jr-*.tar.gz\n\
             jr-*.zip\n\
             jr-*.sha256\n\
         \n\
         Without this, the Windows .zip artifact is built but never uploaded to \
         GitHub Actions artifacts and is therefore unavailable to the `release` job.\n\
         \n\
         (S-FORK-OPS-BACKFILL-1 AC-001 / story tasks §Item 4)"
    );
}

// ---------------------------------------------------------------------------
// AC-002 (DESTRUCTIVE) — check-then-upsert replaces delete+create
// ---------------------------------------------------------------------------

/// AC-002 (delete absent): The `release` job in `backfill-release.yml` must
/// NOT contain a `gh release delete` command.
///
/// The old `gh release delete "$TAG" --yes --repo ... 2>/dev/null || true`
/// pattern unconditionally destroys curator-edited release notes and is replaced
/// by a check-then-upsert block.
///
/// Anchoring: assertion is scoped to the `release` job block.
///
/// RED GATE: `gh release delete` IS present in the current `release` job.
/// This test FAILS on the current branch.
///
/// Traces to: AC-002; drift item FORK-OPS-BACKFILL-DESTRUCTIVE; spec-delta
/// §DESTRUCTIVE "Current delete+create block (REMOVE)".
#[test]
fn test_backfill_release_job_has_no_delete_command() {
    let yml = read_backfill_yml();

    let release_block = extract_job_block(&yml, "release").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/backfill-release.yml` does not contain a \
             `release:` job (two-space indent).\n\
             Expected: a `release` job with a 'Create or update GitHub Release' step."
        )
    });

    assert!(
        !release_block.contains("gh release delete"),
        "FAIL (RED GATE): The `release` job in `.github/workflows/backfill-release.yml` \
         contains `gh release delete`.\n\
         \n\
         The destructive delete+create pattern MUST be removed entirely and replaced \
         by the check-then-upsert block. `gh release delete` unconditionally destroys \
         curator-edited release notes and is a data-loss risk on re-runs.\n\
         \n\
         Required: remove the `gh release delete \"$TAG\" --yes ...` line and replace \
         the entire pattern with the check-then-upsert block from the spec delta.\n\
         \n\
         (S-FORK-OPS-BACKFILL-1 AC-002 / drift item FORK-OPS-BACKFILL-DESTRUCTIVE)"
    );
}

/// AC-002 (silencer absent): The `release` job must NOT contain the `|| true`
/// silencer on a `gh` command line.
///
/// The old `... 2>/dev/null || true` silencer on `gh release delete` hid errors
/// and prevented operators from seeing concurrent-dispatch failures. With the
/// `|| true` silencer removed, errors surface and fail the step cleanly.
///
/// Anchoring: assertion is scoped to the `release` job block.
///
/// RED GATE: `|| true` IS present on the `gh release delete` line. This test
/// FAILS on the current branch.
///
/// Traces to: AC-002; spec-delta DESTRUCTIVE Invariant 2 ("Silencer removed:
/// The `|| true` on the old `gh release delete` line is removed entirely").
#[test]
fn test_backfill_release_job_has_no_or_true_silencer() {
    let yml = read_backfill_yml();

    let release_block = extract_job_block(&yml, "release").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/backfill-release.yml` does not contain a \
             `release:` job (two-space indent)."
        )
    });

    // The `|| true` silencer must not appear on any `gh release` command line.
    // We scope the check to lines that also contain `gh release` (not any `gh`
    // command) to avoid false positives from `|| true` in unrelated contexts.
    let has_silenced_gh_command = release_block
        .lines()
        .any(|line| line.contains("gh release") && line.contains("|| true"));

    assert!(
        !has_silenced_gh_command,
        "FAIL (RED GATE): The `release` job in `.github/workflows/backfill-release.yml` \
         contains a `gh release ... || true` silenced command.\n\
         \n\
         The `|| true` error silencer MUST be removed. It hides failures (including \
         concurrent-dispatch conflicts) and prevents the step from surfacing errors \
         to the operator.\n\
         \n\
         Required: remove `|| true` from all `gh release` command lines in the \
         release job. Errors in the upsert path must propagate and fail the step.\n\
         \n\
         (S-FORK-OPS-BACKFILL-1 AC-002 / spec-delta DESTRUCTIVE Invariant 2)"
    );
}

/// AC-002 (upsert view check present): The `release` job must contain a
/// `gh release view` command — the existence check in the check-then-upsert
/// pattern.
///
/// The check-then-upsert block opens with:
/// ```bash
/// if gh release view "$TAG" --repo ... >/dev/null 2>&1; then
/// ```
/// This guards against unconditional destruction of existing releases.
///
/// Anchoring: assertion is scoped to the `release` job block.
///
/// RED GATE: `gh release view` is NOT present in the current `release` job
/// (it uses the delete+create pattern instead). This test FAILS on the current
/// branch.
///
/// Traces to: AC-002; spec-delta DESTRUCTIVE §"Replacement" step 1.
#[test]
fn test_backfill_release_job_has_upsert_view_check() {
    let yml = read_backfill_yml();

    let release_block = extract_job_block(&yml, "release").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/backfill-release.yml` does not contain a \
             `release:` job (two-space indent)."
        )
    });

    assert!(
        release_block.contains("gh release view"),
        "FAIL (RED GATE): The `release` job in `.github/workflows/backfill-release.yml` \
         does not contain `gh release view`.\n\
         \n\
         The check-then-upsert pattern requires an existence check:\n\
           if gh release view \"$TAG\" --repo \"${{{{ github.repository }}}}\" \
         >/dev/null 2>&1; then\n\
         \n\
         Without this check, every run unconditionally destroys the existing release \
         (the old delete+create pattern).\n\
         \n\
         Required: replace the `gh release delete ... || true` + `gh release create` \
         block with the check-then-upsert block from the spec delta.\n\
         \n\
         (S-FORK-OPS-BACKFILL-1 AC-002 / spec-delta DESTRUCTIVE §Replacement step 1)"
    );
}

/// AC-002 (upsert upload branch present): The `release` job must contain a
/// `gh release upload` command — the upload branch in the check-then-upsert
/// pattern (used when the release already exists).
///
/// The upload branch uses `--clobber` to replace existing assets without
/// touching release notes or publish flags.
///
/// Anchoring: assertion is scoped to the `release` job block.
///
/// RED GATE: `gh release upload` is NOT present in the current `release` job.
/// This test FAILS on the current branch.
///
/// Traces to: AC-002; spec-delta DESTRUCTIVE §"Replacement" exists-branch.
#[test]
fn test_backfill_release_job_has_upsert_upload_branch() {
    let yml = read_backfill_yml();

    let release_block = extract_job_block(&yml, "release").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/backfill-release.yml` does not contain a \
             `release:` job (two-space indent)."
        )
    });

    assert!(
        release_block.contains("gh release upload"),
        "FAIL (RED GATE): The `release` job in `.github/workflows/backfill-release.yml` \
         does not contain `gh release upload`.\n\
         \n\
         The check-then-upsert pattern requires an upload branch for existing releases:\n\
           gh release upload \"$TAG\" --repo \"${{{{ github.repository }}}}\" \
         --clobber jr-*.tar.gz jr-*.zip jr-*.sha256\n\
         \n\
         The `gh release upload --clobber` command replaces assets without touching \
         curator-edited release notes or draft/prerelease flags (Invariants 1, 6, 7).\n\
         \n\
         (S-FORK-OPS-BACKFILL-1 AC-002 / spec-delta DESTRUCTIVE §Replacement exists-branch)"
    );
}

/// AC-002 (asset completeness — zip in both branches): `jr-*.zip` must appear
/// in BOTH the upload (exists) branch AND the create (new release) branch of the
/// check-then-upsert block in the `release` job.
///
/// Adding `jr-*.zip` to only one branch creates an asset-completeness bug on the
/// other path — backfilled releases created for the first time would be missing
/// the Windows binary, or re-run releases would be missing it on upsert.
///
/// The assertion counts occurrences of `jr-*.zip` in the release job block and
/// requires at least two — one per branch. (The Upload artifact step in the build
/// job also uses `jr-*.zip`, but that is in the `build` job block, not `release`.)
///
/// Anchoring: assertion is scoped to the `release` job block.
///
/// RED GATE: `jr-*.zip` is NOT present in the current `release` job (the old
/// delete+create pattern does not include it). This test FAILS on the current
/// branch.
///
/// Traces to: AC-002; spec-delta DESTRUCTIVE Invariant 3 ("Asset completeness:
/// `jr-*.zip` appears in BOTH the upload branch and the create branch").
#[test]
fn test_backfill_release_job_zip_in_both_upsert_branches() {
    let yml = read_backfill_yml();

    let release_block = extract_job_block(&yml, "release").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/backfill-release.yml` does not contain a \
             `release:` job (two-space indent)."
        )
    });

    let zip_count = release_block.matches("jr-*.zip").count();

    assert!(
        zip_count >= 2,
        "FAIL (RED GATE): `jr-*.zip` appears fewer than 2 times in the `release` \
         job block of `.github/workflows/backfill-release.yml` \
         (found {} occurrence{}).\n\
         \n\
         `jr-*.zip` MUST appear in BOTH branches of the check-then-upsert block:\n\
           1. The upload branch (`gh release upload ... jr-*.tar.gz jr-*.zip jr-*.sha256`)\n\
           2. The create branch (`gh release create ... jr-*.tar.gz jr-*.zip jr-*.sha256`)\n\
         \n\
         Adding it to only one branch creates an asset-completeness bug: \
         a first-run release would be missing the Windows binary, or a re-run \
         release would be missing it on upsert (Invariant 3).\n\
         \n\
         (S-FORK-OPS-BACKFILL-1 AC-002 / spec-delta DESTRUCTIVE Invariant 3)",
        zip_count,
        if zip_count == 1 { "" } else { "s" }
    );
}

/// AC-002 / EC-001 (draft detection present): The `release` job must contain
/// `isDraft` — the draft-release detection check in the check-then-upsert
/// exists-branch.
///
/// When an existing release is a draft, the upload branch uploads assets via
/// `--clobber` but emits `::warning::` and does NOT set `--draft false`. This
/// guards against silently publishing a draft release that a curator intentionally
/// held back. The `isDraft` check via `gh release view --json isDraft --jq '.isDraft'`
/// is the mechanism.
///
/// Anchoring: assertion is scoped to the `release` job block.
///
/// RED GATE: `isDraft` is NOT present in the current `release` job. This test
/// FAILS on the current branch.
///
/// Traces to: AC-002; spec-delta DESTRUCTIVE Invariant 6 (draft-release
/// handling); edge case EC-001 (release exists and is a draft when backfill runs).
#[test]
fn test_backfill_release_job_has_draft_detection() {
    let yml = read_backfill_yml();

    let release_block = extract_job_block(&yml, "release").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/backfill-release.yml` does not contain a \
             `release:` job (two-space indent)."
        )
    });

    assert!(
        release_block.contains("isDraft"),
        "FAIL (RED GATE): The `release` job in `.github/workflows/backfill-release.yml` \
         does not contain `isDraft`.\n\
         \n\
         The check-then-upsert exists-branch must detect draft releases:\n\
           DRAFT_STATUS=$(gh release view \"$TAG\" \\\n\
             --repo \"${{{{ github.repository }}}}\" \\\n\
             --json isDraft --jq '.isDraft')\n\
           if [ \"$DRAFT_STATUS\" = \"true\" ]; then\n\
             echo \"::warning::Release $TAG is a draft. Uploading assets but NOT \
         publishing — curator must manually publish.\"\n\
           fi\n\
         \n\
         Without this check, the upload branch silently proceeds on a draft release. \
         The `::warning::` annotation ensures the operator is notified that the \
         release remains unpublished after the run (EC-001 / Invariant 6).\n\
         \n\
         (S-FORK-OPS-BACKFILL-1 AC-002 / spec-delta DESTRUCTIVE Invariant 6 / EC-001)"
    );
}

// ---------------------------------------------------------------------------
// CR-001 / CR-002 (shell: bash) — Build and Unix Package steps must declare
// shell: bash so they work correctly on the new Windows runner
// ---------------------------------------------------------------------------

/// CR-001 (CRITICAL): The `Build` step in `backfill-release.yml` must declare
/// `shell: bash`.
///
/// Without `shell: bash`, GitHub Actions defaults to `pwsh` on `windows-latest`
/// runners. The Build step body contains a POSIX `if [ ... ]; then ... fi`
/// construct that is invalid PowerShell — the Windows build job would fail
/// immediately on the new `x86_64-pc-windows-msvc` matrix row.
///
/// `release.yml`'s Build step carries `shell: bash` for exactly this reason.
///
/// Anchoring: the assertion is scoped to the `build` job block to prevent
/// a `shell: bash` in the `sign` job or a comment from satisfying the check.
///
/// RED GATE (CR-001): `shell: bash` is currently absent from the Build step.
/// This test FAILS against the workflow as-committed in ac5cdf4.
///
/// Traces to: CR-001; release.yml `jobs.build.steps[name=Build].shell`.
#[test]
fn test_backfill_build_step_declares_shell_bash() {
    let yml = read_backfill_yml();

    let build_block = extract_job_block(&yml, "build").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/backfill-release.yml` does not contain a \
             `build:` job (two-space indent)."
        )
    });

    // Locate the `Build` step by finding `      - name: Build\n` within the
    // build block, then search the following lines for `shell: bash` before
    // the next step boundary (`      - name:`).
    let build_step_needle = "      - name: Build\n";
    let step_start = build_block.find(build_step_needle).unwrap_or_else(|| {
        panic!(
            "FAIL: Could not find the `Build` step (      - name: Build) inside the \
                 `build` job of `.github/workflows/backfill-release.yml`."
        )
    });

    // Slice from the start of the Build step to the next step boundary.
    let after_step = &build_block[step_start + build_step_needle.len()..];
    let next_step_offset = after_step.find("      - name:").unwrap_or(after_step.len());
    let build_step_body = &after_step[..next_step_offset];

    assert!(
        build_step_body.contains("shell: bash"),
        "FAIL (CR-001): The `Build` step in `.github/workflows/backfill-release.yml` \
         does not declare `shell: bash`.\n\
         \n\
         Without `shell: bash`, GitHub Actions defaults to `pwsh` on `windows-latest`. \
         The Build step body contains POSIX shell syntax (`if [ ... ]; then ... fi`) \
         that is invalid PowerShell — the Windows build job (x86_64-pc-windows-msvc) \
         will fail immediately.\n\
         \n\
         Required fix: add `shell: bash` to the Build step, matching `release.yml`:\n\
           - name: Build\n\
             shell: bash\n\
             env:\n\
               ...\n\
         \n\
         (CR-001 / release.yml `jobs.build.steps[name=Build].shell`)"
    );
}

/// CR-002 (HIGH): The Unix `Package` step in `backfill-release.yml` must declare
/// `shell: bash`.
///
/// The step is gated `if: runner.os != 'Windows'`, so it never runs on Windows.
/// However, adding `shell: bash` makes the shell contract explicit — consistent
/// with `release.yml`'s Package (Unix) step which also carries `shell: bash` —
/// and ensures the step behaves correctly if the `if:` condition ever changes.
///
/// Anchoring: the assertion is scoped to the `build` job block and targets the
/// `Package` step (the Unix one, without `(Windows)` in the name).
///
/// RED GATE (CR-002): `shell: bash` is currently absent from the Unix Package step.
/// This test FAILS against the workflow as-committed in ac5cdf4.
///
/// Traces to: CR-002; release.yml `jobs.build.steps[name="Package (Unix)"].shell`.
#[test]
fn test_backfill_unix_package_step_declares_shell_bash() {
    let yml = read_backfill_yml();

    let build_block = extract_job_block(&yml, "build").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/backfill-release.yml` does not contain a \
             `build:` job (two-space indent)."
        )
    });

    // Locate the Unix `Package` step: `      - name: Package\n` (NOT
    // `Package (Windows)`) within the build block, then search the following
    // lines for `shell: bash` before the next step boundary.
    // We find the step by looking for the exact name `Package\n` to avoid
    // matching `Package (Windows)`.
    let package_step_needle = "      - name: Package\n";
    let step_start = build_block.find(package_step_needle).unwrap_or_else(|| {
        panic!(
            "FAIL: Could not find the Unix `Package` step (      - name: Package) \
                 inside the `build` job of `.github/workflows/backfill-release.yml`.\n\
                 The step must be named exactly `Package` (not `Package (Windows)`) \
                 and gated with `if: runner.os != 'Windows'`."
        )
    });

    // Slice from the start of the Package step to the next step boundary.
    let after_step = &build_block[step_start + package_step_needle.len()..];
    let next_step_offset = after_step.find("      - name:").unwrap_or(after_step.len());
    let package_step_body = &after_step[..next_step_offset];

    assert!(
        package_step_body.contains("shell: bash"),
        "FAIL (CR-002): The Unix `Package` step in `.github/workflows/backfill-release.yml` \
         does not declare `shell: bash`.\n\
         \n\
         The step is gated `if: runner.os != 'Windows'` so it does not run on Windows, \
         but explicitly declaring `shell: bash` matches `release.yml`'s Package (Unix) \
         step convention and makes the shell contract explicit.\n\
         \n\
         Required fix: add `shell: bash` to the Unix Package step:\n\
           - name: Package\n\
             if: runner.os != 'Windows'\n\
             shell: bash\n\
             env:\n\
               ...\n\
         \n\
         (CR-002 / release.yml `jobs.build.steps[name=\"Package (Unix)\"].shell`)"
    );
}

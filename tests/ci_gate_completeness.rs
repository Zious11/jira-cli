//! S-CIGATE-1 — permanent regression guard for the `ci-gate` aggregator job.
//!
//! These tests pin the structural invariants required for the `ci-gate`
//! aggregator job to work correctly as the single required branch-protection
//! status check.  All tests must remain GREEN — they guard against future
//! regressions that would silently reopen the skipped-job trap or weaken the
//! pass/fail semantics.
//!
//! Anchoring technique: each test calls `extract_job_block("ci-gate")` first
//! and then asserts within that slice only — identical to the precedent in
//! `tests/ci_yml_windows_matrix.rs`.  This prevents a match in an unrelated
//! job from producing a false positive.
//!
//! Architecture compliance rules traced here:
//!   - `if: ${{ always() }}` is LOAD-BEARING: without it a failed upstream
//!     causes `ci-gate` to be SKIPPED (not failed), which GitHub branch
//!     protection evaluates as SUCCESS — the worst failure mode (broken
//!     upstream silently permits merge).  Detail: F1 delta analysis §4,
//!     Architecture Compliance Rules in S-CIGATE-1 story spec.
//!   - PR-only jobs (`security`, `mutants`) must NOT be in `needs` because
//!     they emit `skipped` on push events, which would poison push-triggered
//!     `ci-gate` runs.  Same for `coverage` (advisory by design).
//!   - `spec-guard` has no `if:` guard and must be promoted to a blocking
//!     check (DEC-101).
//!
//! Test coverage map (→ S-CIGATE-1 AC):
//!   test_ci_gate_job_exists_with_correct_shell               → AC-001
//!   test_ci_gate_needs_exactly_the_required_jobs             → AC-003
//!   test_ci_gate_excludes_pr_only_jobs                       → AC-003
//!   test_ci_gate_fails_on_failed_or_cancelled_need           → AC-002
//!   test_ci_gate_needs_jobs_have_no_event_conditional_if     → EC-002 (M1)
//!   test_ci_gate_pass_fail_semantics_are_structurally_placed → AC-001/AC-002 (M2)

use std::collections::HashSet;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Helpers (mirror of ci_yml_windows_matrix.rs)
// ---------------------------------------------------------------------------

/// Read `.github/workflows/ci.yml` relative to the repo root (the parent of
/// the `tests/` directory).
///
/// CRLF normalization: on Windows, Git may check out `*.yml` files with CRLF
/// line endings.  Normalizing here keeps the rest of the matching logic
/// platform-independent — identical to the precedent in
/// `tests/ci_yml_windows_matrix.rs`.
fn read_ci_yml() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
    raw.replace("\r\n", "\n")
}

#[allow(dead_code)]
mod common;
use common::yaml::extract_job_block;

/// Parse the `needs:` value from a job block.
///
/// Handles both the inline-array form `needs: [a, b, c]` and the block-list
/// form:
/// ```yaml
/// needs:
///   - a
///   - b
/// ```
/// Returns `None` if no `needs:` line is found in the block.
fn parse_needs_set(job_block: &str) -> Option<HashSet<String>> {
    // Try inline-array form first: `needs: [fmt, clippy, ...]`
    for line in job_block.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("needs:") {
            let rest = rest.trim();
            if rest.starts_with('[') && rest.ends_with(']') {
                // Inline array: strip brackets, split on `,`, trim each item.
                let inner = &rest[1..rest.len() - 1];
                let set: HashSet<String> = inner
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                return Some(set);
            }
        }
    }

    // Try block-list form: `needs:` on its own line, followed by `  - item` lines.
    let mut in_needs = false;
    let mut set = HashSet::new();
    for line in job_block.lines() {
        let trimmed = line.trim();
        if trimmed == "needs:" {
            in_needs = true;
            continue;
        }
        if in_needs {
            if let Some(item) = trimmed.strip_prefix("- ") {
                set.insert(item.trim().to_string());
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                // Reached a non-list-item non-comment line — end of needs block.
                break;
            }
        }
    }

    if in_needs && !set.is_empty() {
        Some(set)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// AC-001 — `ci-gate` job exists with correct structural properties
// ---------------------------------------------------------------------------

/// AC-001: A job keyed `ci-gate` must exist in `.github/workflows/ci.yml`.
/// Within that job block:
///   - `name: CI Gate` must be present.
///   - `runs-on: ubuntu-latest` must be present.
///   - An `if:` line containing `always()` must be present (load-bearing —
///     see file-level doc comment and Architecture Compliance Rules in the
///     story spec).
///
/// Anchoring: assertion is made only within the `ci-gate` job block, so
/// a matching substring in an unrelated job cannot produce a false positive.
///
/// RED GATE: `ci-gate` does not exist in ci.yml.  This test FAILS on develop.
#[test]
fn test_ci_gate_job_exists_with_correct_shell() {
    let ci = read_ci_yml();
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): `.github/workflows/ci.yml` does not contain a \
             `ci-gate:` job (two-space indent).\n\
             Required: append the `ci-gate` aggregator job to ci.yml per \
             S-CIGATE-1 AC-001."
        )
    });

    // name: CI Gate — produces the human-readable branch-protection context
    // string "CI Gate".  If omitted, the context would be the key "ci-gate".
    assert!(
        gate_block.contains("name: CI Gate"),
        "FAIL (RED GATE): The `ci-gate` job block does not contain \
         `name: CI Gate`.\n\
         Required: set `name: CI Gate` so the branch-protection context string \
         is human-readable (EC-003).\n\
         Current ci-gate block:\n{gate_block}"
    );

    // runs-on: ubuntu-latest — the aggregator is a lightweight step that only
    // inspects upstream results; ubuntu-latest is the correct runner.
    assert!(
        gate_block.contains("runs-on: ubuntu-latest"),
        "FAIL (RED GATE): The `ci-gate` job block does not contain \
         `runs-on: ubuntu-latest`.\n\
         Current ci-gate block:\n{gate_block}"
    );

    // if: ${{ always() }} — LOAD-BEARING.  Without this, a failed upstream
    // skips `ci-gate` entirely; GitHub evaluates skip as SUCCESS, so a broken
    // upstream would silently permit merge (EC-001).
    assert!(
        gate_block.lines().any(|l| {
            let t = l.trim();
            t.starts_with("if:") && t.contains("always()")
        }),
        "FAIL (RED GATE): The `ci-gate` job block does not have an \
         `if:` line containing `always()`.\n\
         Required: `if: ${{{{ always() }}}}` at the job level (load-bearing — \
         without it a failed upstream SKIPS ci-gate, which GitHub branch \
         protection evaluates as SUCCESS).\n\
         Current ci-gate block:\n{gate_block}"
    );
}

// ---------------------------------------------------------------------------
// AC-003 — `ci-gate.needs` is exactly the required six-job set
// ---------------------------------------------------------------------------

/// AC-003 (exact-set check): `ci-gate.needs` must contain exactly the seven
/// jobs `{fmt, clippy, test, msrv, deny, spec-guard, check-signing-workflow-injection}`
/// — order-insensitive, no extras, none missing.
///
/// Rationale for exact-set (not subset):
///   - Adding a job to `needs` without updating this test intentionally fails
///     the test, prompting the author to (a) confirm the new job has no
///     PR-only `if:` guard and (b) update the expected set.
///   - Dropping a required job from `needs` (CI drift) also fails this test.
///
/// Anchoring: assertion is made only within the `ci-gate` job block.
///
/// RED GATE: `ci-gate` does not exist in ci.yml.  This test FAILS on develop.
#[test]
fn test_ci_gate_needs_exactly_the_required_jobs() {
    let ci = read_ci_yml();
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): `.github/workflows/ci.yml` does not contain a \
             `ci-gate:` job.\n\
             Required: append the `ci-gate` aggregator job to ci.yml per \
             S-CIGATE-1 AC-001 / AC-003."
        )
    });

    let actual = parse_needs_set(gate_block).unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): The `ci-gate` job block does not contain a \
             `needs:` key.\n\
             Required: `needs: [fmt, clippy, test, msrv, deny, spec-guard]`\n\
             Current ci-gate block:\n{gate_block}"
        )
    });

    let expected: HashSet<String> = [
        "fmt",
        "clippy",
        "test",
        "msrv",
        "deny",
        "spec-guard",
        "check-signing-workflow-injection",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let missing: Vec<&String> = expected.difference(&actual).collect();
    let extra: Vec<&String> = actual.difference(&expected).collect();

    let mut failures: Vec<String> = Vec::new();
    if !missing.is_empty() {
        let mut sorted = missing.iter().map(|s| s.as_str()).collect::<Vec<_>>();
        sorted.sort();
        failures.push(format!(
            "Missing from ci-gate.needs ({}): {}",
            sorted.len(),
            sorted.join(", ")
        ));
    }
    if !extra.is_empty() {
        let mut sorted = extra.iter().map(|s| s.as_str()).collect::<Vec<_>>();
        sorted.sort();
        failures.push(format!(
            "Unexpected in ci-gate.needs ({}): {}\n\
             If a new mandatory CI job was added here, verify it has no \
             `if: github.event_name == 'pull_request'` guard (PR-only jobs \
             emit `skipped` on push and would poison the gate), then update \
             the expected set in this test.",
            sorted.len(),
            sorted.join(", ")
        ));
    }

    assert!(
        failures.is_empty(),
        "FAIL: `ci-gate.needs` does not match the required exact set \
         {{fmt, clippy, test, msrv, deny, spec-guard, check-signing-workflow-injection}}.\n\
         Actual needs: {:?}\n\
         {}",
        {
            let mut v: Vec<_> = actual.iter().collect();
            v.sort();
            v
        },
        failures.join("\n")
    );
}

// ---------------------------------------------------------------------------
// AC-003 — PR-only jobs must NOT be in `ci-gate.needs`
// ---------------------------------------------------------------------------

/// AC-003 (exclusion check): `security` and `mutants` must NOT appear in
/// `ci-gate.needs`.
///
/// Both jobs carry `if: github.event_name == 'pull_request'` — they emit
/// `skipped` on push events.  Including them in `ci-gate.needs` would make
/// every push-triggered `ci-gate` run fail because GitHub treats `skipped`
/// as a non-success result for aggregator purposes.
///
/// `coverage` is also excluded: it uses `fail_ci_if_error: false` on the
/// codecov upload and is advisory by design.
///
/// Anchoring: assertion is made only within the `ci-gate` job block.
///
/// RED GATE: `ci-gate` does not exist in ci.yml.  This test FAILS on develop.
#[test]
fn test_ci_gate_excludes_pr_only_jobs() {
    let ci = read_ci_yml();
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): `.github/workflows/ci.yml` does not contain a \
             `ci-gate:` job.\n\
             Required: append the `ci-gate` aggregator job to ci.yml per \
             S-CIGATE-1 AC-001 / AC-003."
        )
    });

    let needs = parse_needs_set(gate_block).unwrap_or_default();

    // PR-only jobs: skipped on push → must not be in needs.
    assert!(
        !needs.contains("security"),
        "FAIL: `security` must NOT be in `ci-gate.needs`.\n\
         The `security` job carries `if: github.event_name == 'pull_request'` \
         and emits `skipped` on push events.  Including it poisons every \
         push-triggered `ci-gate` run.\n\
         Current needs: {:?}",
        {
            let mut v: Vec<_> = needs.iter().collect();
            v.sort();
            v
        }
    );

    assert!(
        !needs.contains("mutants"),
        "FAIL: `mutants` must NOT be in `ci-gate.needs`.\n\
         The `mutants` job carries `if: github.event_name == 'pull_request'` \
         and emits `skipped` on push events.  Including it poisons every \
         push-triggered `ci-gate` run.\n\
         Current needs: {:?}",
        {
            let mut v: Vec<_> = needs.iter().collect();
            v.sort();
            v
        }
    );

    // Advisory job: fail_ci_if_error: false — must not be in needs.
    assert!(
        !needs.contains("coverage"),
        "FAIL: `coverage` must NOT be in `ci-gate.needs`.\n\
         The `coverage` job uses `fail_ci_if_error: false` on the codecov \
         upload and is advisory by design.  Including it would let a flaky \
         coverage upload block legitimate merges.\n\
         Current needs: {:?}",
        {
            let mut v: Vec<_> = needs.iter().collect();
            v.sort();
            v
        }
    );
}

// ---------------------------------------------------------------------------
// AC-002 — gate step exits 1 on failure or cancelled upstream
// ---------------------------------------------------------------------------

/// AC-002: The `ci-gate` job must contain a step that exits 1 when any
/// upstream `needs` result is `'failure'` or `'cancelled'`.
///
/// Required substrings in the job block (F1 delta analysis §4):
///   - `needs.*.result` — accesses the result map across all deps.
///   - `'failure'`      — catches failed upstreams.
///   - `'cancelled'`    — catches cancelled upstreams (not `skipped` —
///     `skipped` is not possible for the six unconditionally-run jobs).
///
/// The step's `if:` condition gates the `run: exit 1` so the step is skipped
/// (and the job passes) when all `needs` results are `success`.
///
/// Anchoring: assertion is made only within the `ci-gate` job block.
///
/// RED GATE: `ci-gate` does not exist in ci.yml.  This test FAILS on develop.
#[test]
fn test_ci_gate_fails_on_failed_or_cancelled_need() {
    let ci = read_ci_yml();
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): `.github/workflows/ci.yml` does not contain a \
             `ci-gate:` job.\n\
             Required: append the `ci-gate` aggregator job to ci.yml per \
             S-CIGATE-1 AC-001 / AC-002."
        )
    });

    // The step condition (or the run: body) must reference needs.*.result
    // with both 'failure' and 'cancelled' checks.
    assert!(
        gate_block.contains("needs.*.result"),
        "FAIL (RED GATE): The `ci-gate` job block does not contain \
         `needs.*.result`.\n\
         Required: a step `if:` condition like \
         `${{{{ contains(needs.*.result, 'failure') || \
         contains(needs.*.result, 'cancelled') }}}}`\n\
         Current ci-gate block:\n{gate_block}"
    );

    assert!(
        gate_block.contains("'failure'"),
        "FAIL (RED GATE): The `ci-gate` job block does not contain `'failure'`.\n\
         Required: the gate step must check \
         `contains(needs.*.result, 'failure')` so that a failed upstream causes \
         `ci-gate` to fail (not be skipped).\n\
         Current ci-gate block:\n{gate_block}"
    );

    assert!(
        gate_block.contains("'cancelled'"),
        "FAIL (RED GATE): The `ci-gate` job block does not contain `'cancelled'`.\n\
         Required: the gate step must check \
         `contains(needs.*.result, 'cancelled')` so that a cancelled upstream \
         causes `ci-gate` to fail.\n\
         Current ci-gate block:\n{gate_block}"
    );
}

// ---------------------------------------------------------------------------
// M1 — needs jobs must run unconditionally (no event-conditional job-level if:)
// ---------------------------------------------------------------------------

/// M1: For each job listed in `ci-gate.needs`, assert that the job's block
/// contains NO job-level `if:` line that references `github.event_name`.
///
/// Rationale: the existing exact-set test (`test_ci_gate_needs_exactly_the_required_jobs`)
/// pins WHICH jobs are in `needs`, but not that those jobs run unconditionally.
/// If a future maintainer adds `if: github.event_name == 'pull_request'` to
/// e.g. `deny`, the gate would silently pass on push events (deny → skipped →
/// not 'failure').  This is the exact drift vector EC-002 claims to prevent.
///
/// Job-level `if:` lines are at 4-space indent immediately under the job key
/// (e.g. `    if: github.event_name == 'pull_request'`).  Step-level `if:`
/// lines are at 8-space or greater indent (inside `steps:`); those are not
/// hazardous and are deliberately not checked here.
///
/// Anchoring: each job's block is extracted via `extract_job_block` before
/// the assertion is made, so a match in an unrelated job cannot produce a
/// false positive.
#[test]
fn test_ci_gate_needs_jobs_have_no_event_conditional_if() {
    let ci = read_ci_yml();

    // The seven jobs that must run unconditionally on every push and PR.
    let required_jobs = [
        "fmt",
        "clippy",
        "test",
        "msrv",
        "deny",
        "spec-guard",
        "check-signing-workflow-injection",
    ];

    for job_name in &required_jobs {
        let job_block = extract_job_block(&ci, job_name).unwrap_or_else(|| {
            panic!(
                "FAIL: job `{job_name}` (listed in ci-gate.needs) was not found \
                 in ci.yml.  Either the job was renamed or removed — update \
                 ci-gate.needs and this test together."
            )
        });

        // A job-level `if:` is at exactly 4-space indent directly under the
        // job key (GitHub Actions YAML convention).  Step-level `if:` blocks
        // are indented 8+ spaces; those are irrelevant to this check.
        //
        // We detect a job-level if: by looking for lines that start with
        // exactly four spaces followed by "if:" (with optional trailing space).
        for line in job_block.lines() {
            // Match lines at job-property indent (4 spaces, not 8+).
            if line.starts_with("    if:")
                && !line.starts_with("        ")
                && line.contains("github.event_name")
            {
                panic!(
                    "FAIL (M1): Job `{job_name}` has a job-level `if:` that \
                     references `github.event_name`:\n\
                     \n  {line}\n\
                     \n\
                     This makes `{job_name}` skip on push events (it emits \
                     `skipped`, not `failure`), which silently satisfies \
                     `ci-gate.needs` and allows broken code to merge.\n\
                     \n\
                     Fix: either remove the job-level `if:` guard from \
                     `{job_name}` and use a step-level `if:` instead, or \
                     remove `{job_name}` from `ci-gate.needs` and update this \
                     test accordingly."
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// M2 — pass/fail semantics are structurally placed correctly
// ---------------------------------------------------------------------------

/// M2: Pin the structural placement of `always()` (job-level) and
/// `contains(needs.*.result, …)` (step-level) so they cannot be accidentally
/// transposed by a future maintainer.
///
/// The correct shape is:
///
/// ```yaml
///   ci-gate:
///     if: ${{ always() }}          ← job-level: must have always(), must NOT
///     steps:                           have contains(needs
///       - name: …
///         if: >-                   ← step-level: must have contains(needs.*.result,
///           ${{ contains(…) }}         'failure') and contains(…, 'cancelled')
///         run: exit 1              ← a run: step must be present
/// ```
///
/// If `always()` were moved to the step-level `if:` and `contains(needs…)` to
/// the job-level `if:`, the job would run only when needs fail (breaking the
/// intended pass path) or the job would be skipped silently when needs fail
/// (reopening the skipped-job trap).
///
/// Distinction technique: job-level `if:` lines start with exactly 4 spaces
/// (not 8+) and are not inside a `steps:` block.  Step-level `if:` lines are
/// inside `steps:` and start with 8 spaces (within a list item).
#[test]
fn test_ci_gate_pass_fail_semantics_are_structurally_placed() {
    let ci = read_ci_yml();
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/ci.yml` does not contain a `ci-gate:` job.\n\
             Required: append the `ci-gate` aggregator job to ci.yml per \
             S-CIGATE-1 AC-001."
        )
    });

    // -----------------------------------------------------------------------
    // Assertion 1: The job-level `if:` contains `always()` and does NOT
    // contain `contains(needs`.
    //
    // "Job-level" means the `if:` key is a direct child of the job block,
    // indented 4 spaces from the left margin (GitHub Actions YAML convention).
    // We stop scanning once we enter the `steps:` section to avoid picking up
    // step-level `if:` lines.
    // -----------------------------------------------------------------------
    let mut found_job_level_if = false;
    let mut in_steps = false;
    let mut job_if_line = String::new();

    for line in gate_block.lines() {
        // Detect entry into the steps: section (4-space indent).
        if line.starts_with("    steps:") && !line.starts_with("        ") {
            in_steps = true;
        }
        // A job-level `if:` is at 4-space indent, NOT inside steps.
        if !in_steps && line.starts_with("    if:") && !line.starts_with("        ") {
            found_job_level_if = true;
            job_if_line = line.to_string();
            break;
        }
    }

    assert!(
        found_job_level_if,
        "FAIL (M2-a): The `ci-gate` job block has no job-level `if:` line \
         (expected at 4-space indent, before `steps:`).\n\
         Required: `    if: ${{{{ always() }}}}` so that ci-gate runs even when \
         upstream jobs fail.\n\
         Current ci-gate block:\n{gate_block}"
    );

    assert!(
        job_if_line.contains("always()"),
        "FAIL (M2-b): The job-level `if:` in `ci-gate` does not contain \
         `always()`.\n\
         Found:    {job_if_line}\n\
         Required: the job-level `if:` must be `always()` so the aggregator \
         runs regardless of upstream results (without this, a failed upstream \
         skips ci-gate and GitHub branch protection evaluates the skip as \
         SUCCESS).\n\
         Current ci-gate block:\n{gate_block}"
    );

    assert!(
        !job_if_line.contains("contains(needs"),
        "FAIL (M2-c): The job-level `if:` in `ci-gate` contains \
         `contains(needs` — the failure condition is at the WRONG level.\n\
         Found:    {job_if_line}\n\
         The `contains(needs.*.result, …)` expression must be on a STEP-level \
         `if:` (inside `steps:`), not the job-level `if:`.  At job level, \
         only `always()` should appear — placing `contains(needs…)` there \
         would prevent the job from running when all needs succeed.\n\
         Current ci-gate block:\n{gate_block}"
    );

    // -----------------------------------------------------------------------
    // Assertion 2: A step-level `if:` contains the failure/cancelled checks.
    //
    // "Step-level" means the `if:` key is inside the `steps:` block,
    // indented 8 spaces (within a list item under steps:).  We accept any
    // line starting with 8 spaces that begins with "if:" (after trimming).
    // -----------------------------------------------------------------------
    let mut step_if_lines: Vec<&str> = Vec::new();
    let mut past_steps = false;
    // Collect all step-level if: lines (8-space indent, or inside the
    // block-scalar continuation of such a line).
    // We gather the entire gate block after the `steps:` marker.
    for line in gate_block.lines() {
        if line.trim_start() == "steps:" {
            past_steps = true;
        }
        if past_steps && line.starts_with("        if:") {
            step_if_lines.push(line);
        }
    }

    // Also capture block-scalar continuation lines (they follow the `if: >-`
    // line and contain the actual condition expression).
    let gate_lines: Vec<&str> = gate_block.lines().collect();
    let mut all_step_if_content = String::new();
    let mut in_step_if_block = false;
    let mut step_if_indent = 0usize;
    for line in &gate_lines {
        let stripped = line.trim_end();
        if stripped.starts_with("        if:") {
            in_step_if_block = true;
            step_if_indent = 8;
            all_step_if_content.push_str(stripped);
            all_step_if_content.push('\n');
            continue;
        }
        if in_step_if_block {
            // Continuation lines are indented MORE than the `if:` line.
            let leading = stripped.len() - stripped.trim_start().len();
            if leading > step_if_indent && !stripped.trim().is_empty() {
                all_step_if_content.push_str(stripped);
                all_step_if_content.push('\n');
            } else {
                in_step_if_block = false;
            }
        }
    }

    assert!(
        all_step_if_content.contains("contains(needs.*.result")
            || step_if_lines
                .iter()
                .any(|l| l.contains("contains(needs.*.result")),
        "FAIL (M2-d): No step-level `if:` in `ci-gate` contains \
         `contains(needs.*.result`.\n\
         The failure/cancelled check must live on a STEP-level `if:` \
         (inside `steps:`), not the job-level `if:`.\n\
         Step-level if: lines found: {step_if_lines:?}\n\
         Current ci-gate block:\n{gate_block}"
    );

    assert!(
        all_step_if_content.contains("'failure'"),
        "FAIL (M2-e): The step-level `if:` in `ci-gate` does not check for \
         `'failure'`.\n\
         Step-level if content collected:\n{all_step_if_content}\n\
         Current ci-gate block:\n{gate_block}"
    );

    assert!(
        all_step_if_content.contains("'cancelled'"),
        "FAIL (M2-f): The step-level `if:` in `ci-gate` does not check for \
         `'cancelled'`.\n\
         Step-level if content collected:\n{all_step_if_content}\n\
         Current ci-gate block:\n{gate_block}"
    );

    // -----------------------------------------------------------------------
    // Assertion 3: A `run:` step exists in the ci-gate block.
    //
    // The gate must execute something that can fail.  Without a `run:` step,
    // the entire job would trivially succeed on any upstream result.
    // -----------------------------------------------------------------------
    let has_run_step = gate_block
        .lines()
        .any(|l| l.trim_start().starts_with("run:"));

    assert!(
        has_run_step,
        "FAIL (M2-g): The `ci-gate` job block contains no `run:` step.\n\
         The gate must have a `run: exit 1` (or equivalent) step that \
         actually fails the job when the step-level `if:` condition is met.\n\
         Without a `run:` step the job trivially succeeds for every upstream \
         result.\n\
         Current ci-gate block:\n{gate_block}"
    );
}

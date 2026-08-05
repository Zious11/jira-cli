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
//!   - `security` and `coverage` must NOT be in `needs`: `security` is PR-only
//!     (emits `skipped` on push and is gated by GITLEAKS_DISABLED); `coverage`
//!     is advisory by design (`fail_ci_if_error: false`).
//!   - `mutants` IS in `needs` (MUTATION-CI-TIMEOUT, 2026-06-28).  It carries
//!     `if: github.event_name == 'pull_request'` and emits `skipped` on push
//!     events.  The ci-gate pass condition checks for `failure` or `cancelled`
//!     only — `skipped` is neither, so ci-gate passes on push events.  This is
//!     the correct behavior per DEC-096/097 and delta-analysis §5.
//!   - `spec-guard` has no `if:` guard and must be promoted to a blocking
//!     check (DEC-101).
//!
//! Test coverage map (→ S-CIGATE-1 AC):
//!   test_ci_gate_job_exists_with_correct_shell               → AC-001
//!   test_ci_gate_needs_exactly_the_required_jobs             → AC-003
//!   test_ci_gate_excludes_advisory_and_secret_scan_jobs      → AC-003
//!   test_mutants_is_in_ci_gate_needs                         → MUTATION-CI-TIMEOUT / AC-003
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

/// AC-003 (exact-set check): `ci-gate.needs` must contain exactly the eight
/// jobs `{fmt, clippy, test, msrv, deny, spec-guard, check-signing-workflow-injection, mutants}`
/// — order-insensitive, no extras, none missing.
///
/// `mutants` was promoted to hard-required in MUTATION-CI-TIMEOUT (2026-06-28).
/// It carries `if: github.event_name == 'pull_request'` and emits `skipped` on
/// push events.  The ci-gate condition checks for `failure` or `cancelled` only
/// — `skipped` is neither, so ci-gate passes on push events (correct behavior).
///
/// Rationale for exact-set (not subset):
///   - Adding a job to `needs` without updating this test intentionally fails
///     the test, prompting the author to confirm push-event safety and update
///     the expected set.
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
             Required: `needs: [fmt, clippy, test, msrv, deny, spec-guard, \
             check-signing-workflow-injection, mutants]`\n\
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
        // MUTATION-CI-TIMEOUT (2026-06-28): promoted to hard-required.
        // Carries `if: github.event_name == 'pull_request'`; emits `skipped`
        // on push events — safe because ci-gate checks `failure`/`cancelled`
        // only.  See delta-analysis §5 and cargo-mutants-policy.md §CI Gate.
        "mutants",
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
         {{fmt, clippy, test, msrv, deny, spec-guard, check-signing-workflow-injection, mutants}}.\n\
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
// AC-003 — advisory/secret-scan jobs must NOT be in `ci-gate.needs`
// ---------------------------------------------------------------------------

/// AC-003 (exclusion check): `security` and `coverage` must NOT appear in
/// `ci-gate.needs`.
///
/// `security` carries `if: github.event_name == 'pull_request'` AND is
/// further gated by `vars.GITLEAKS_DISABLED`.  Including it would poison
/// push-triggered `ci-gate` runs (the job emits `skipped` on push).
///
/// `coverage` uses `fail_ci_if_error: false` on the codecov upload and is
/// advisory by design.  Including it would let a flaky coverage upload block
/// legitimate merges.
///
/// NOTE: `mutants` IS in `ci-gate.needs` since MUTATION-CI-TIMEOUT
/// (2026-06-28).  It carries `if: github.event_name == 'pull_request'` but
/// the ci-gate pass condition checks `failure` or `cancelled` only — `skipped`
/// is neither, so ci-gate passes on push events.  See delta-analysis §5.
///
/// Anchoring: assertion is made only within the `ci-gate` job block.
///
/// RED GATE: `ci-gate` does not exist in ci.yml.  This test FAILS on develop.
#[test]
fn test_ci_gate_excludes_advisory_and_secret_scan_jobs() {
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

    // `security` is gated by `github.event_name == 'pull_request'` AND by
    // `vars.GITLEAKS_DISABLED` — emits `skipped` on push; must not be in needs.
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
// MUTATION-CI-TIMEOUT — `mutants` is in `ci-gate.needs`
// ---------------------------------------------------------------------------

/// MUTATION-CI-TIMEOUT (2026-06-28): `mutants` must be in `ci-gate.needs`.
///
/// The `mutants` job was promoted to hard-required to enforce the 90% kill-rate
/// gate on every PR.  It carries `if: github.event_name == 'pull_request'` and
/// emits `skipped` on push events.  The ci-gate condition checks for `failure`
/// or `cancelled` only — `skipped` is neither, so ci-gate passes on push events.
///
/// This test pins the promotion: a future edit that accidentally removes
/// `mutants` from `ci-gate.needs` must explicitly update this test.
///
/// Anchoring: assertion is made only within the `ci-gate` job block.
#[test]
fn test_mutants_is_in_ci_gate_needs() {
    let ci = read_ci_yml();
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/ci.yml` does not contain a `ci-gate:` job.\n\
             Required: append the `ci-gate` aggregator job to ci.yml per \
             S-CIGATE-1 AC-001."
        )
    });

    let needs = parse_needs_set(gate_block).unwrap_or_else(|| {
        panic!(
            "FAIL: The `ci-gate` job block does not contain a `needs:` key.\n\
             Current ci-gate block:\n{gate_block}"
        )
    });

    assert!(
        needs.contains("mutants"),
        "FAIL (MUTATION-CI-TIMEOUT): `mutants` is missing from `ci-gate.needs`.\n\
         The `mutants` job was promoted to hard-required in MUTATION-CI-TIMEOUT \
         (2026-06-28) to enforce the 90% kill-rate gate on every PR.\n\
         Push-event safety: `mutants` emits `skipped` on push events; the \
         ci-gate condition checks `failure`/`cancelled` only — `skipped` is \
         neither, so ci-gate passes on push events.\n\
         To restore: add `mutants` to `ci-gate.needs` in ci.yml.\n\
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

/// M1: For each unconditionally-running job listed in `ci-gate.needs`, assert
/// that the job's block contains NO job-level `if:` line that references
/// `github.event_name`.
///
/// Rationale: the existing exact-set test (`test_ci_gate_needs_exactly_the_required_jobs`)
/// pins WHICH jobs are in `needs`, but not that those jobs run unconditionally.
/// If a future maintainer adds `if: github.event_name == 'pull_request'` to
/// e.g. `deny`, the gate would silently pass on push events (deny → skipped →
/// not 'failure').  This is the exact drift vector EC-002 claims to prevent.
///
/// `mutants` is intentionally excluded from this list: it carries
/// `if: github.event_name == 'pull_request'` by design (PR-only scope),
/// emits `skipped` on push events, and the ci-gate condition checks `failure`
/// or `cancelled` only — so `skipped` is safe.  The `test_mutants_is_in_ci_gate_needs`
/// test pins that `mutants` remains in `ci-gate.needs`.
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
    // `mutants` is intentionally excluded — it is PR-only by design and
    // emits `skipped` on push events (ci-gate-safe; see test docstring above).
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

// ---------------------------------------------------------------------------
// POL-11 — `test` job must have a zero-test floor
// ---------------------------------------------------------------------------

/// POL-11: The `test` job step must contain a zero-test floor guard so that
/// `cargo test` exiting 0 with 0 tests executed does not satisfy the required
/// branch-protection check.
///
/// `cargo test` exits 0 when zero test targets are found or all targets are
/// filtered out.  Without an explicit floor, a `Cargo.toml` change
/// (`autotests = false`, a `[[test]]` rename, or a harness misconfiguration)
/// can orphan all integration-test targets and still green the `test` job —
/// which is in `ci-gate.needs`.  This is the POL-11 false-green class covered
/// by story S-626-1 AC-10, anchored to BC-X.13.007 ("The `test` job enforces
/// a runtime-computed test-execution floor") in
/// `.factory/specs/prd/cross-cutting.md`.
///
/// A passed-count `> 0` floor is insufficient: `cargo test --all-features`
/// also runs the inline `#[cfg(test)]` modules in `src/` (~1,100 tests at
/// head), so orphaning every file in `tests/` via `autotests = false` or a
/// `[[test]]` rename still produces a non-zero total — the `> 0` predicate is
/// inert against the defect class it claims to catch.  Three gates are
/// therefore required:
///   (1) Binary-count floor (`< 90`): catches mass orphaning of tests/ files.
///       At head ~103 binaries run; orphaning all integration targets drops
///       this below 90.  The threshold tolerates ~13 legitimate reductions.
///   (2) Named canary: asserts `tests/ci_gate_completeness` ran, catching the
///       self-orphaning case where the guard binary itself stops running —
///       even when the binary count stays above 90.
///   (3) Zero-test floor (`"${total}" -eq 0`): catches the case where ≥90
///       binaries (including the canary) report results but zero tests
///       passed within them — e.g. a global test filter matching nothing.
///       Neither (1) nor (2) detects this scenario: the binary count and the
///       named binary's presence are both satisfied; only the passed-count
///       check catches it.
///
/// A fourth mechanism, orthogonal to all three gates above, has to hold for
/// any of them to run against a genuine result at all: `set -euo pipefail`
/// at the top of the step is the SOLE mechanism propagating a real `cargo
/// test` failure through `cargo test --all-features 2>&1 | tee ...` into a
/// failed step (without `-o pipefail`, `set -e` sees only `tee`'s
/// always-0 exit status).
///
/// Both the `CARGO_TERM_COLOR: never` override (Defect 3 fix) and the
/// `set +o pipefail` / `set -o pipefail` scoped-disable bracket around the
/// count computations (Defect 2 fix) are structural parts of the guard too.
///
/// **What is pinned below, and what is not.**  An earlier version of this
/// docstring claimed the assertions below "pin all operative parts" — that
/// was false.  Gate (3) above (`"${total}" -eq 0`) and the `set -euo
/// pipefail` line had NO assertion prior to round 17: three independent
/// reviewers each demonstrated a defeat that left every other assertion in
/// this test green — deleting the `if [ "${total}" -eq 0 ]; ... fi` block
/// verbatim, and separately replacing `set -euo pipefail` with `set -eu`
/// while feeding a mock `cargo` that exits 101 after printing passing "test
/// result:" lines (which made the step exit 0 and print `Check passed:
/// ...` despite the simulated failure) — both reproduced in a scratchpad
/// copy before the fix. The assertions below are graded by how hard they
/// are to defeat without also breaking the enforcement logic itself:
///   - **Variable/command-bound** (hardest to defeat: a rename or rewrite
///     that neuters the check also breaks the literal text this assertion
///     requires): `"${binaries}" -lt 90`, `"${total}" -eq 0`,
///     `grep -q "ci_gate_completeness"`.  None of these three forms
///     currently appears anywhere else in `ci.yml` (verified) — a bare
///     `-lt 90` / `-eq 0` / `ci_gate_completeness` substring, by contrast,
///     also appears in this guard's own comments and echo diagnostics and
///     would be satisfied even after the check was neutered by a variable
///     rename.
///   - **Exact standalone line** (comment-satisfiable only by a future
///     comment reproducing the identical trailing form — no such comment
///     exists today): `set -euo pipefail\n`, `set +o pipefail\n`,
///     `set -o pipefail\n`.  `"set -o pipefail\n"` is NOT a substring of
///     `"set -euo pipefail\n"` (the characters after `set -` are `euo`, not
///     `o`), so these three assertions are independent of one another —
///     dropping any one of the three lines fails exactly one assertion, not
///     all three.  This is incidental to the current comment wording, not a
///     structural guarantee: a future comment line ending exactly with one
///     of these forms (no trailing text) immediately before the newline
///     would also satisfy the corresponding assertion.
///   - **Literal substring, weaker still** (a comment or unrelated step
///     could in principle reproduce it): `CARGO_TERM_COLOR: never`.
///   - **Weakest** (also appears inside this guard's own `echo`
///     diagnostics, or — for `exit 1` — is a generic command with no
///     natural variable-binding; a rewrite that preserves the diagnostic
///     strings while gutting the enforcement logic underneath would not be
///     caught by these alone): `FAIL (POL-11)`, `Check passed:`, `exit 1`.
///   - **NOT PINNED — no assertion covers these today:** the
///     `cargo test --all-features 2>&1 | tee "$RUNNER_TEMP/cargo_test_out.txt"`
///     invocation itself (a rewrite dropping `--all-features`, or changing
///     what gets captured, is undetected); and the `total=`/`binaries=`
///     computation pipelines (the `grep`/`grep -Eo`/`awk` and
///     `grep`/`wc -l`/`tr` chains) that produce the values the gates above
///     test — only their *usages* (`"${total}" -eq 0`,
///     `"${binaries}" -lt 90`) are pinned, so a rewrite of the computation
///     logic that leaves those two variables holding a wrong-but-passing
///     value is undetected as long as the variable names survive.
///
/// Anchoring: assertion is made only within the `test` job block, so a
/// matching substring in an unrelated job cannot produce a false positive.
#[test]
fn test_verify_test_job_has_zero_test_floor() {
    let ci = read_ci_yml();
    let test_block = extract_job_block(&ci, "test").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/ci.yml` does not contain a `test:` job.\n\
             Required: the `test` job must exist with a zero-test floor guard \
             (POL-11 / F-07)."
        )
    });

    // --- Instrument 0: error sentinel ---
    // The floor guard emits "FAIL (POL-11)" in all failure branches.
    // Asserting on it means that removing the guard entirely fails this test.
    assert!(
        test_block.contains("FAIL (POL-11)"),
        "FAIL (POL-11): The `test` job step does not contain the zero-test \
         floor guard.\n\
         Required: the `cargo test` step must count tests executed at runtime \
         and fail loudly (emitting `FAIL (POL-11): ...`).\n\
         Removing the floor reopens the false-green class documented in \
         S-626-1 / F-07: cargo test exits 0 on 0 tests, so the `test` job \
         passes even when all integration-test targets are orphaned, causing \
         ci-gate to silently green with ~2000+ regression pins unenforced.\n\
         Current test job block:\n{test_block}"
    );

    // --- Instrument 1: binary-count floor ---
    // Guards against mass orphaning of tests/ files.  The prior "> 0" check
    // on the passed count was inert because src/ inline tests still run when
    // tests/ is fully orphaned.  The floor must use a non-trivial threshold
    // on the *binary count*, not the passed count.
    //
    // The assertion targets `"${binaries}" -lt 90` (variable-bound), not the
    // bare `-lt 90`.  A future edit renaming `binaries` to `total` would keep
    // bare `-lt 90` present while neutering the floor (`total` is ~2345 tests,
    // never < 90); this variable-bound form catches that regression.
    assert!(
        test_block.contains("\"${binaries}\" -lt 90"),
        "FAIL (POL-11): The `test` job step does not contain the binary-count \
         floor using the `binaries` variable (`\"${{binaries}}\" -lt 90`).\n\
         A '> 0' passed-count predicate is inert: src/ inline tests (~1,100) \
         still run when tests/ is orphaned, keeping total > 0.  The floor must \
         gate on the number of test *binaries* (\"${{binaries}}\" -lt 90).\n\
         Current test job block:\n{test_block}"
    );

    // --- Instrument 2: named canary ---
    // Guards the self-orphaning case: the binary carrying this guard and all
    // CI-gate Rust regression pins stops running.  The binary floor cannot
    // detect this case when the count stays above 90.
    //
    // The assertion targets the command form `grep -q "ci_gate_completeness"`
    // rather than the bare substring `ci_gate_completeness`.  The bare form
    // also appears in a YAML comment (ci.yml :: test / "Run tests (zero-test
    // floor, POL-11)" § "(2) Named canary") and an echo diagnostic
    // (ci.yml :: test / "Run tests (zero-test floor, POL-11)" § "did not run"
    // echo); either line satisfies a bare-substring check while leaving the
    // operative grep command absent.  Only the command line contains
    // `grep -q "ci_gate_completeness"`.
    assert!(
        test_block.contains("grep -q \"ci_gate_completeness\""),
        "FAIL (POL-11): The `test` job step does not contain the named-canary \
         command `grep -q \"ci_gate_completeness\"`.\n\
         Required: grep the captured output for the ci_gate_completeness binary \
         to detect the self-orphaning case (guard binary renamed, autotests=false,\
         or [[test]] override).\n\
         Current test job block:\n{test_block}"
    );

    // --- Instrument 3: zero-test floor (`total -eq 0`) ---
    // Distinct from both instruments above: the binary floor and the named
    // canary both pass when ≥90 binaries (including ci_gate_completeness)
    // report results but zero tests passed within them (e.g. a global
    // `--skip` filter, or a harness that reports results without running
    // any test body).  This is BC-X.13.007 Behavior item 3, and this
    // assertion is the ONLY gate covering that scenario — proven by
    // deleting the `if [ "${total}" -eq 0 ]; then ... fi` block wholesale
    // in a scratchpad copy: every other assertion in this test (including
    // the binary floor and named canary above) stayed green.
    //
    // The assertion targets the variable-bound form `"${total}" -eq 0`, not
    // a bare `-eq 0`, for the same reason Instrument 1 targets
    // `"${binaries}" -lt 90` rather than bare `-lt 90`: a bare form would
    // survive a variable rename that neuters the check.
    assert!(
        test_block.contains("\"${total}\" -eq 0"),
        "FAIL (POL-11): The `test` job step does not contain the zero-test \
         floor gate using the `total` variable (`\"${{total}}\" -eq 0`).\n\
         This is the only gate that catches ≥90 test binaries reporting \
         results while zero tests actually passed (e.g. a global test \
         filter matching nothing) — the binary-count floor and named canary \
         both pass in that scenario.\n\
         Current test job block:\n{test_block}"
    );

    // --- exit 1 is present ---
    // The floor branches must actually fail the step, not merely warn.
    assert!(
        test_block.contains("exit 1"),
        "FAIL (POL-11): The `test` job step does not contain `exit 1`.\n\
         The floor/canary guards must fail the step on violation.\n\
         Current test job block:\n{test_block}"
    );

    // --- Positive-coverage line is present ---
    // POL-11 requires emitting a runtime-computed count to prove tests ran
    // (exit code alone is insufficient — the count proves the assertions
    // above were actually evaluated at non-trivial scale).
    assert!(
        test_block.contains("Check passed:"),
        "FAIL (POL-11): The `test` job step does not contain the \
         `Check passed:` positive-coverage assertion.\n\
         Required: emit a runtime-computed count so that a reviewer can see \
         both the guard passed and how many tests ran.\n\
         Current test job block:\n{test_block}"
    );

    // --- CARGO_TERM_COLOR: never is present ---
    // The file-level `env: CARGO_TERM_COLOR: always` is overridden for this
    // step.  Empirically, cargo does not forward its color preference to
    // libtest today, so the "test result:" line the grep anchors on is plain
    // ASCII even without this override — the claimed failure is not
    // presently reachable.  The override is defensive: it removes the
    // dependency on that behavior continuing to hold across future
    // cargo/libtest versions or a differently-configured CI runner, where a
    // color-wrapped "test result:" line would silently zero the anchored
    // grep and produce a false-red with no diagnostic.
    assert!(
        test_block.contains("CARGO_TERM_COLOR: never"),
        "FAIL (POL-11): The `test` job step does not override \
         `CARGO_TERM_COLOR` to `never`.\n\
         Required: add `env: CARGO_TERM_COLOR: never` to the step so that ANSI \
         escape codes in libtest output do not break the anchored grep.\n\
         Current test job block:\n{test_block}"
    );

    // --- top-level abort mechanism (`set -euo pipefail`) is present ---
    // This is the SOLE mechanism that propagates a genuine `cargo test`
    // failure through `cargo test --all-features 2>&1 | tee ...` into a
    // failed step.  Without pipefail here, the tee pipeline's exit status is
    // `tee`'s (always 0), so `set -e` never fires on a failed `cargo test` —
    // the step falls through to the count computations, which sum whatever
    // partial "test result:" lines cargo did print, satisfy the binary
    // floor / canary / zero-test-floor gates, and print `Check passed:` —
    // an unambiguous false-green.  Reproduced directly: replacing this line
    // with `set -eu` (dropping `-o pipefail`) and feeding a mock `cargo`
    // that exits 101 after printing 103 "test result:" lines summing to a
    // non-zero passed count causes the step to exit 0 and print
    // `Check passed: ... test binaries` — while the unmutated line correctly
    // propagates the mock's exit 101.
    //
    // The assertion targets the exact whole-line form `set -euo pipefail\n`
    // rather than a bare `pipefail` substring, and is distinct from (does
    // not overlap) the `set -o pipefail\n` assertion below: `"set -o
    // pipefail\n"` is not a substring of `"set -euo pipefail\n"` (the
    // characters immediately after `set -` are `euo`, not `o`), so a
    // regression that drops the `-o pipefail` combination into `set -eu`
    // would fail THIS assertion while the scoping-bracket assertions below
    // are unaffected (they check a different, later line in the script).
    assert!(
        test_block.contains("set -euo pipefail\n"),
        "FAIL (POL-11): The `test` job step does not open with the command \
         `set -euo pipefail` as a standalone line.\n\
         Required: `-o pipefail` is what causes a failed `cargo test` in \
         `cargo test --all-features 2>&1 | tee ...` to actually fail the \
         step — without it, `tee`'s (always-0) exit status is what `set -e` \
         sees, and a genuine test failure silently falls through to the \
         count computations and prints `Check passed:` (verified false-green \
         reproduction, S-626-1 round 17).\n\
         Current test job block:\n{test_block}"
    );

    // --- pipefail scoping bracket is present ---
    // The count computations must run under `set +o pipefail` (disabled).
    // Under set -o pipefail, grep exits 1 on no-match; that exit propagates
    // through the pipeline and, combined with set -e, aborts the step before
    // any FAIL (POL-11) diagnostic can print.  Removing the OPENING bracket
    // (`set +o pipefail`) reintroduces that false-abort class.
    //
    // Removing the CLOSING bracket (`set -o pipefail`, the restore) does
    // NOT reintroduce the same class — it is the opposite failure mode:
    // pipefail stays disabled, which is strictly more permissive and can
    // never cause an abort.  The two brackets guard opposite failure modes,
    // not a shared one.  The restore's stated purpose — so that "real I/O
    // errors in the gate checks are not silently swallowed" — has no
    // operative effect today: nothing after the restore is a pipeline (the
    // gate checks are `[ ... ]` tests, a bare `grep -q`, and `echo`s), so
    // there is no pipe exit status for pipefail to change the handling of.
    // It remains defensive hygiene — restoring the step's ambient default
    // in case a future gate check introduces a pipe — but is not, in the
    // current file, load-bearing the way the opening bracket is.
    //
    // The trailing `\n` in each check is load-bearing: it distinguishes the
    // standalone command line from the comments that also mention the same
    // flags (e.g. `# Under set -o pipefail, ...` or
    // `# set +o pipefail the pipeline exit ...`).  Those comment lines
    // currently carry additional text after the flag name, so they do not
    // match the `...\n` form today — but this is an incidental property of
    // the current comment wording, not a guarantee: a future comment line
    // ending exactly with `set -o pipefail` (no trailing text) immediately
    // before the newline would also satisfy this assertion.
    assert!(
        test_block.contains("set +o pipefail\n"),
        "FAIL (POL-11): The `test` job step does not contain the command \
         `set +o pipefail` as a standalone line.\n\
         Required: the count computations must run with pipefail disabled so \
         that grep's no-match exit-1 does not abort the step via `set -e` \
         before the FAIL (POL-11) diagnostic can print.\n\
         Current test job block:\n{test_block}"
    );
    assert!(
        test_block.contains("set -o pipefail\n"),
        "FAIL (POL-11): The `test` job step does not contain the command \
         `set -o pipefail` as a standalone line (the restoring bracket).\n\
         Required: pipefail must be re-enabled after the count computations \
         as defensive hygiene (restoring the step's ambient default) — even \
         though nothing after it is currently a pipeline, so this has no \
         operative effect on the gate checks that follow today.\n\
         Current test job block:\n{test_block}"
    );
}

// ---------------------------------------------------------------------------
// S-626-1 AC-3 — `msrv` job genuinely validates 1.85.0
// ---------------------------------------------------------------------------

/// S-626-1 AC-3: the `msrv` job must genuinely compile at Rust 1.85.0,
/// not silently fall through to
/// `rust-toolchain.toml`'s `channel = "stable"`.
///
/// `rust-toolchain.toml` outranks `rustup default` in rustup's precedence
/// chain. The pre-fix `msrv` job pointed at the tip of dtolnay's `1.85.0`
/// version-branch action, which has no `toolchain` input and only sets
/// `rustup default` — so `cargo check` silently ran under `stable` in the
/// repo root, a false-green (documented in this project's CLAUDE.md under
/// "`rust-toolchain.toml` outranks `rustup default`"). The fix requires
/// TWO cooperating pieces: (1) `with: {toolchain: "1.85.0"}` on the
/// `dtolnay/rust-toolchain` step, to install the correct toolchain, and
/// (2) `env: {RUSTUP_TOOLCHAIN: "1.85.0"}` on the `cargo check` step,
/// which outranks `rust-toolchain.toml` at process level and is the part
/// that actually forces the check to run at 1.85.0. Deleting only the
/// `env:` block is a two-line, silent regression: before this test
/// existed, nothing else in CI or the test suite *asserted* on
/// `RUSTUP_TOOLCHAIN` — CLAUDE.md and CHANGELOG.md both mention it in
/// prose, but documentation references are not guards — so `msrv` would
/// have kept passing while validating `stable` again.
///
/// This test makes THREE assertions, not two. The first two asserted
/// strings are exact, quote-included forms (`toolchain: "1.85.0"` and
/// `RUSTUP_TOOLCHAIN: "1.85.0"`) that appear exactly once each in the whole
/// of `ci.yml`, both in operative `with:`/`env:` key-value position — never
/// inside a comment. (The bare substring `1.85.0` also appears in the job's
/// `name:` line, the `dtolnay/rust-toolchain` version-pin comment, and two
/// scope-rationale comments — which is why the assertions below match the
/// longer, key-qualified forms rather than the bare version string.)
///
/// The third assertion, `cargo check --all-features --locked` (added when
/// `d848d9a5` pinned `--locked` to close a dependency-drift gap — see the
/// AC-3 rationale above), is a different shape: it is the literal `run:`
/// step command itself, not a `with:`/`env:` key-value pair. It also
/// appears exactly once in the whole of `ci.yml`, in operative position.
/// The `msrv` job carries a 10-line scope-rationale comment discussing
/// `--all-targets` and `--all-features` (why the job omits the former and
/// why the latter is a no-op for this crate) — that comment does NOT
/// currently reproduce the full concatenated substring
/// `cargo check --all-features --locked`, so today this assertion is not
/// comment-satisfiable. That is, as with the `toolchain`/`RUSTUP_TOOLCHAIN`
/// pair above, an incidental property of the current comment wording, not a
/// structural guarantee: a future edit to that scope comment that happened
/// to quote the full command verbatim would satisfy this assertion without
/// the operative `run:` line needing to match it — this is a live,
/// unresolved question this docstring does not close, not a claim that it
/// cannot happen.
///
/// Anchoring: assertion is made only within the `msrv` job block, so a
/// matching substring in an unrelated job (e.g. `coverage`, which pins a
/// different dtolnay toolchain) cannot produce a false positive.
#[test]
fn test_verify_msrv_job_pins_toolchain_and_rustup_toolchain_env() {
    let ci = read_ci_yml();
    let msrv_block = extract_job_block(&ci, "msrv").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/ci.yml` does not contain an `msrv:` job.\n\
             Required: the `msrv` job must exist and genuinely validate Rust \
             1.85.0 (S-626-1 AC-3)."
        )
    });

    assert!(
        msrv_block.contains("toolchain: \"1.85.0\""),
        "FAIL (S-626-1): The `msrv` job does not pin `toolchain: \"1.85.0\"` \
         on its `dtolnay/rust-toolchain` step.\n\
         Required: at the pinned SHA (`fa04a1451ff1842e2626ccb99004d0195b455a88`), \
         `toolchain` is a required input with no default — omitting the `with:` \
         block does not fall back to `rust-toolchain.toml` or a default; the \
         action exits 1 with `'toolchain' is a required input`, failing the job \
         loudly. The genuinely silent revert vector is removal of the \
         `RUSTUP_TOOLCHAIN` env override on the `cargo check` step (see the next \
         assertion) — that's what this input's presence guards against staying \
         meaningful.\n\
         Current msrv job block:\n{msrv_block}"
    );

    assert!(
        msrv_block.contains("RUSTUP_TOOLCHAIN: \"1.85.0\""),
        "FAIL (S-626-1): The `msrv` job does not set \
         `RUSTUP_TOOLCHAIN: \"1.85.0\"` as an env override on its \
         `cargo check` step.\n\
         Required: `RUSTUP_TOOLCHAIN` outranks `rust-toolchain.toml` \
         (`channel = \"stable\"`) in rustup's precedence chain. Without this \
         override, `cargo check` silently validates `stable` instead of \
         1.85.0 — the exact false-green this job exists to close.\n\
         Current msrv job block:\n{msrv_block}"
    );

    assert!(
        msrv_block.contains("cargo check --all-features --locked"),
        "FAIL (S-626-1 AC-3): The `msrv` job's `cargo check` step is not \
         invoked with `--locked`.\n\
         Required: without `--locked`, `cargo check` is free to re-resolve \
         other and transitive dependencies against `Cargo.toml` at check \
         time, decoupling the MSRV check from the exact dependency graph \
         the rest of CI (and users) actually build against — a silent \
         drift vector with no other test or CI signal to catch it.\n\
         Current msrv job block:\n{msrv_block}"
    );
}

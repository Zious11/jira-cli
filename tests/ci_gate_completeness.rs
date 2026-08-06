//! S-CIGATE-1 / S-CIGATE-2 — permanent regression guard for the `ci-gate`
//! aggregator job.
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
//!     events.  Since S-CIGATE-2, the gate's pass/fail decision lives in
//!     `scripts/check-ci-gate.sh` (invoked with `toJSON(needs)`), which
//!     fails closed by default: `mutants` reporting `skipped` on push is
//!     tolerated ONLY because it is named in that script's restrictive
//!     `ALLOWED_SKIPS` allowlist (which still fails the gate on
//!     `failure`/`cancelled` for that same job) — not because the gate's
//!     decision happens not to catch it. Any other job's `skipped` result,
//!     or any result value the script has never seen before, fails the gate
//!     by default (see the S-CIGATE-2 section below for the full history).
//!   - `spec-guard` has no `if:` guard and must be promoted to a blocking
//!     check (DEC-101).
//!
//! Test coverage map (→ S-CIGATE-1 AC):
//!   test_ci_gate_job_exists_with_correct_shell               → AC-001
//!   test_ci_gate_needs_exactly_the_required_jobs             → AC-003
//!   test_ci_gate_excludes_advisory_and_secret_scan_jobs      → AC-003
//!   test_mutants_is_in_ci_gate_needs                         → MUTATION-CI-TIMEOUT / AC-003
//!   test_ci_gate_fails_on_failed_or_cancelled_need           → AC-002 (retargeted, S-CIGATE-2)
//!   test_ci_gate_needs_jobs_have_no_event_conditional_if     → EC-002 (M1)
//!   test_ci_gate_pass_fail_semantics_are_structurally_placed → AC-001/AC-002 (M2, retargeted, S-CIGATE-2)
//!
//! ## S-CIGATE-2 (2026-08-06) — skipped-status false-green fix
//!
//! Before S-CIGATE-2, `ci-gate`'s pass condition checked only
//! `contains(needs.*.result, 'failure') || contains(needs.*.result,
//! 'cancelled')`. `skipped` satisfies neither `contains()` call, so a job
//! that never ran (e.g. `mutants` on every push, by design) silently made
//! the sole required branch-protection check report green without that job
//! having run at all — confirmed reachable on every push via live CI run
//! 30465686049 (`gh run view 30465686049 --json jobs`: `Mutation testing`
//! concluded `skipped`, `CI Gate` concluded `success`). An earlier revision
//! of this file's doc comment described that behavior as "the correct
//! behavior per DEC-096/097 and delta-analysis §5" — it was not; that
//! framing described the defect this story fixes, and has been corrected
//! above rather than left standing beside this note.
//!
//! Under Option C (human-approved fix; Options A and B were both rejected —
//! see `.factory/stories/S-CIGATE-2-skipped-status-false-green.md`), the
//! gate's decision logic moved from this file's inline
//! `contains(needs.*.result, 'failure') || contains(needs.*.result,
//! 'cancelled')` condition into `scripts/check-ci-gate.sh`, invoked with
//! `toJSON(needs)`. `mutants` reporting `skipped` on push is tolerated ONLY
//! because it is named in that script's restrictive `ALLOWED_SKIPS`
//! allowlist (which still fails the gate on `failure`/`cancelled` for that
//! same job) — not because the gate's condition happens not to catch it.
//! Any other job's `skipped` result, or any result value the script has
//! never seen before, fails the gate by default.
//!
//! New test coverage (→ S-CIGATE-2 AC):
//!   test_ci_gate_step_invokes_check_ci_gate_script_with_needs_json → AC-001
//!   test_spec_guard_contains_check_ci_gate_self_test_step          → AC-008
//!   test_mutants_job_structure_unchanged_by_cigate2_option_c       → AC-006
//!
//! `test_ci_gate_fails_on_failed_or_cancelled_need` and
//! `test_ci_gate_pass_fail_semantics_are_structurally_placed` (both
//! pre-existing, from S-CIGATE-1) were retargeted rather than left pinning
//! the retired inline condition's literal text: per this story's own
//! `files_modified` plan, they now assert the retired condition is GONE and
//! that `scripts/check-ci-gate.sh` performs the decision instead.

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
/// push events. Since S-CIGATE-2, `scripts/check-ci-gate.sh` tolerates this
/// ONLY because `mutants` is named in that script's restrictive
/// `ALLOWED_SKIPS` allowlist — any other job's `skipped` result fails the
/// gate by default.
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
/// (2026-06-28).  It carries `if: github.event_name == 'pull_request'`.
/// Since S-CIGATE-2, `scripts/check-ci-gate.sh` tolerates its `skipped`
/// push-event result ONLY because `mutants` is named in that script's
/// restrictive `ALLOWED_SKIPS` allowlist — not because the gate's decision
/// happens not to catch it.
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
/// emits `skipped` on push events.  Since S-CIGATE-2, `scripts/check-ci-gate.sh`
/// tolerates this ONLY because `mutants` is named in that script's restrictive
/// `ALLOWED_SKIPS` allowlist — any other job's `skipped` result, or a `failure`
/// / `cancelled` result for `mutants` itself, fails the gate.
///
/// This test pins the promotion: a future edit that accidentally removes
/// `mutants` from `ci-gate.needs` must explicitly update this test (and, per
/// CLAUDE.md's `ci-gate` convention, `scripts/check-ci-gate.sh`'s
/// `ALLOWED_SKIPS`).
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
         Push-event safety: `mutants` emits `skipped` on push events; \
         `scripts/check-ci-gate.sh` tolerates this ONLY because `mutants` is \
         named in that script's restrictive `ALLOWED_SKIPS` allowlist \
         (S-CIGATE-2).\n\
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
// AC-002 — gate fails on failure or cancelled upstream (via check-ci-gate.sh)
// ---------------------------------------------------------------------------

/// AC-002 (retargeted, S-CIGATE-2): The `ci-gate` job must delegate its
/// failure/cancelled/skipped decision to `scripts/check-ci-gate.sh`'s
/// `evaluate_needs()`, not to an inline YAML condition.
///
/// Before S-CIGATE-2, this test pinned the retired inline condition
/// (`contains(needs.*.result, 'failure') || contains(needs.*.result,
/// 'cancelled')`) directly in the `ci-gate` step body. That condition is the
/// root cause this story fixes: `skipped` satisfies neither `contains()`
/// call, so a job that never ran (e.g. `mutants` on every push) silently
/// made the gate report green (live CI run 30465686049). Under Option C
/// (human-approved; see
/// `.factory/stories/S-CIGATE-2-skipped-status-false-green.md`), the
/// failure/cancelled/skipped decision moves into
/// `scripts/check-ci-gate.sh`'s `evaluate_needs()`, which is exercised
/// directly by that script's own `--self-test` fixture suite (fixtures
/// `one-job-failure`, `job-cancelled`, `unlisted-job-skipped`,
/// `mutants-failure-allowlist-is-restrictive`).
///
/// This test now pins the migration itself:
///   (a) the retired inline `contains(needs.*.result, ...)` condition must
///       be GONE from `ci-gate`'s step body — its presence alongside the
///       new script invocation would indicate a half-migrated state, and
///   (b) the step invokes `scripts/check-ci-gate.sh`, which performs the
///       failure/cancelled/skipped decision instead (see
///       `test_ci_gate_step_invokes_check_ci_gate_script_with_needs_json`
///       for the full AC-001 invocation-shape assertion).
///
/// Anchoring: assertion is made only within the `ci-gate` job block.
#[test]
fn test_ci_gate_fails_on_failed_or_cancelled_need() {
    let ci = read_ci_yml();
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/ci.yml` does not contain a \
             `ci-gate:` job.\n\
             Required: append the `ci-gate` aggregator job to ci.yml per \
             S-CIGATE-1 AC-001 / AC-002."
        )
    });

    assert!(
        !gate_block.contains("contains(needs.*.result"),
        "FAIL (S-CIGATE-2 AC-002): The `ci-gate` job block still contains \
         the retired inline `contains(needs.*.result, ...)` condition.\n\
         This condition is the root cause S-CIGATE-2 fixes: `skipped` \
         satisfies neither `contains(needs.*.result, 'failure')` nor \
         `contains(needs.*.result, 'cancelled')`, so a job that never ran \
         (e.g. `mutants` on every push) silently made the gate report \
         green. It must be fully replaced by an invocation of \
         `scripts/check-ci-gate.sh`, not kept alongside it.\n\
         Current ci-gate block:\n{gate_block}"
    );

    assert!(
        gate_block.contains("check-ci-gate.sh"),
        "FAIL (S-CIGATE-2 AC-002): The `ci-gate` job block does not invoke \
         `scripts/check-ci-gate.sh`, which now performs the \
         failure/cancelled/skipped decision that the retired inline \
         condition used to perform directly in YAML.\n\
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
/// e.g. `deny` WITHOUT also adding it to `scripts/check-ci-gate.sh`'s
/// `ALLOWED_SKIPS`, the gate correctly starts FAILING deny's push-event runs
/// (S-CIGATE-2 fail-closed default) rather than silently passing — but this
/// test still exists to catch the drift at review time, before a maintainer
/// is surprised by a newly-red gate.
///
/// `mutants` is intentionally excluded from this list: it carries
/// `if: github.event_name == 'pull_request'` by design (PR-only scope),
/// emits `skipped` on push events, and is named in
/// `scripts/check-ci-gate.sh`'s restrictive `ALLOWED_SKIPS` allowlist — so
/// its `skipped` result is tolerated deliberately, not by accident.  The
/// `test_mutants_is_in_ci_gate_needs` test pins that `mutants` remains in
/// `ci-gate.needs`.
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

/// M2 (retargeted, S-CIGATE-2): Pin the structural placement of `always()`
/// (job-level, unchanged) and the failure decision (now delegated to
/// `scripts/check-ci-gate.sh` via an unconditional `run:` step, rather than
/// a step-level `if:` gating a bare `exit 1`).
///
/// The correct shape (Option C) is:
///
/// ```yaml
///   ci-gate:
///     if: ${{ always() }}          ← job-level: must have always(), must NOT
///     steps:                           have contains(needs
///       - name: …
///         env:
///           NEEDS_JSON: ${{ toJSON(needs) }}
///         run: echo "${NEEDS_JSON}" | bash scripts/check-ci-gate.sh
///                                  ← the script's own exit code is the
///                                     pass/fail signal; no step-level `if:`
///                                     gates it
/// ```
///
/// Before S-CIGATE-2, the failure/cancelled decision lived in a step-level
/// `if: contains(needs.*.result, 'failure') || contains(needs.*.result,
/// 'cancelled')` gating a bare `run: exit 1` — the retired condition that
/// let `skipped` (satisfying neither `contains()` call) silently pass. Under
/// Option C, `scripts/check-ci-gate.sh`'s `evaluate_needs()` makes that
/// decision internally (see the script's own `--self-test` fixture suite)
/// and communicates it via its own exit code from an unconditional `run:`
/// step — there is no longer a step-level `if:` to transpose with the
/// job-level `always()`.
///
/// If `always()` were removed from the job level (or a step-level `if:` were
/// reintroduced to gate the `check-ci-gate.sh` invocation), the job could be
/// skipped silently when needs fail (reopening the skipped-job trap) or the
/// gate script could stop running on some upstream results.
///
/// Distinction technique: job-level `if:` lines start with exactly 4 spaces
/// (not 8+) and are not inside a `steps:` block.
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
    // Assertion 2 (retargeted, S-CIGATE-2): NO step-level `if:` gates the
    // gate-check invocation. Before this story, a step-level
    // `if: contains(needs.*.result, 'failure') || contains(needs.*.result,
    // 'cancelled')` decided whether to run a bare `exit 1` — that condition
    // is exactly what let `skipped` (satisfying neither `contains()` call)
    // pass through silently. Under Option C, `scripts/check-ci-gate.sh`
    // makes the decision internally and signals pass/fail via its own exit
    // code from an UNCONDITIONAL `run:` step — reintroducing a step-level
    // `if:` here would mean some upstream results no longer even reach the
    // script.
    // -----------------------------------------------------------------------
    let has_step_level_if = gate_block.lines().any(|l| l.starts_with("        if:"));

    assert!(
        !has_step_level_if,
        "FAIL (M2-d, S-CIGATE-2): The `ci-gate` job block contains a \
         step-level `if:`. Under Option C, `scripts/check-ci-gate.sh` is \
         invoked unconditionally and its own exit code IS the pass/fail \
         signal — no step-level `if:` should gate that invocation (a \
         reintroduced `if:` here would mean some upstream results never \
         even reach the script).\n\
         Current ci-gate block:\n{gate_block}"
    );

    // -----------------------------------------------------------------------
    // Assertion 3: A `run:` step invoking `scripts/check-ci-gate.sh` exists
    // in the ci-gate block, fed `toJSON(needs)`. The gate must execute
    // something that can fail; without this the job would trivially
    // succeed for every upstream result. (Overlaps in part with
    // `test_ci_gate_step_invokes_check_ci_gate_script_with_needs_json`
    // (AC-001) — kept here too so this test's own M2 structural-placement
    // story remains self-contained.)
    // -----------------------------------------------------------------------
    let has_run_step = gate_block
        .lines()
        .any(|l| l.trim_start().starts_with("run:"));

    assert!(
        has_run_step,
        "FAIL (M2-g): The `ci-gate` job block contains no `run:` step.\n\
         The gate must have a `run:` step that actually invokes \
         `scripts/check-ci-gate.sh` and fails the job via that script's \
         exit code.\n\
         Without a `run:` step the job trivially succeeds for every upstream \
         result.\n\
         Current ci-gate block:\n{gate_block}"
    );

    assert!(
        gate_block.contains("check-ci-gate.sh") && gate_block.contains("toJSON(needs)"),
        "FAIL (M2-h, S-CIGATE-2): The `ci-gate` job block's `run:` step \
         does not invoke `scripts/check-ci-gate.sh` fed `toJSON(needs)`.\n\
         Current ci-gate block:\n{gate_block}"
    );
}

// ---------------------------------------------------------------------------
// S-CIGATE-2 AC-001 — ci-gate's step invokes scripts/check-ci-gate.sh
// ---------------------------------------------------------------------------

/// S-CIGATE-2 AC-001: `ci-gate`'s step must invoke `scripts/check-ci-gate.sh`,
/// fed the serialized `needs` context via `toJSON(needs)`, replacing the
/// retired inline `contains(needs.*.result, ...)` condition. Option C
/// (human-approved; Options A and B rejected — see
/// `.factory/stories/S-CIGATE-2-skipped-status-false-green.md`) keeps
/// `if: ${{ always() }}` at the job level, unchanged — only the step body
/// changes.
///
/// This closes the exact gap the pre-fix `ci-gate` condition has: `skipped`
/// satisfies neither `contains(needs.*.result, 'failure')` nor
/// `contains(needs.*.result, 'cancelled')`, so a job that never ran (e.g.
/// `mutants` on every push) silently makes the gate report green. Under
/// Option C, the decision moves into `scripts/check-ci-gate.sh`, which
/// fails closed by default and tolerates `skipped` only for jobs named in
/// its restrictive `ALLOWED_SKIPS` allowlist.
///
/// RED GATE (S-CIGATE-2, 2026-08-06): as of this commit, `ci-gate`'s step
/// still uses the retired inline `contains(needs.*.result, ...)` condition
/// and does not invoke `check-ci-gate.sh` anywhere — this test FAILS until
/// the Green phase implements AC-001. Proven RED locally: `cargo test
/// --test ci_gate_completeness
/// test_ci_gate_step_invokes_check_ci_gate_script_with_needs_json` fails
/// with the diagnostic below, naming exactly what's missing.
///
/// Anchoring: assertion is made only within the `ci-gate` job block.
#[test]
fn test_ci_gate_step_invokes_check_ci_gate_script_with_needs_json() {
    let ci = read_ci_yml();
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_else(|| {
        panic!(
            "FAIL: `.github/workflows/ci.yml` does not contain a \
             `ci-gate:` job."
        )
    });

    assert!(
        gate_block.contains("check-ci-gate.sh"),
        "FAIL (RED GATE, S-CIGATE-2 AC-001): The `ci-gate` job block does \
         not invoke `scripts/check-ci-gate.sh`.\n\
         Required: the gate step's `run:` body must call \
         `scripts/check-ci-gate.sh`, fed `toJSON(needs)` (Option C — see \
         `.factory/stories/S-CIGATE-2-skipped-status-false-green.md`).\n\
         Current ci-gate block:\n{gate_block}"
    );

    assert!(
        gate_block.contains("toJSON(needs)"),
        "FAIL (RED GATE, S-CIGATE-2 AC-001): The `ci-gate` job block does \
         not serialize the `needs` context via `toJSON(needs)`.\n\
         Required: pass `toJSON(needs)` to `scripts/check-ci-gate.sh` (as \
         an environment variable, a piped stdin payload, or a temp-file \
         argument).\n\
         Current ci-gate block:\n{gate_block}"
    );

    // The job-level `if: always()` must remain — Option C does not change
    // this pre-existing S-CIGATE-1 invariant, only the step body.
    assert!(
        gate_block.lines().any(|l| {
            let t = l.trim();
            t.starts_with("if:") && t.contains("always()")
        }),
        "FAIL (S-CIGATE-2 AC-001): The `ci-gate` job-level `if: always()` \
         is missing. Option C retains `if: ${{{{ always() }}}}` at job \
         level — only the step body is replaced with an invocation of \
         `scripts/check-ci-gate.sh`.\n\
         Current ci-gate block:\n{gate_block}"
    );
}

// ---------------------------------------------------------------------------
// S-CIGATE-2 AC-008 — spec-guard runs check-ci-gate.sh --self-test
// ---------------------------------------------------------------------------

/// S-CIGATE-2 AC-008: `spec-guard` must gain a step running
/// `bash scripts/check-ci-gate.sh --self-test`, positioned consistently
/// with the existing self-test-then-real-check pairing already used in that
/// job for `check-cargo-mutants-policy-citations.sh` and
/// `check-bc-citation-symbols.sh`. The self-test must NOT be wired into
/// `ci-gate` itself: a gate cannot depend on a job that depends on it, and
/// `ci-gate` has no other steps to host a self-test alongside the real gate
/// check it performs against the actual `needs` payload for that run.
///
/// RED GATE (S-CIGATE-2, 2026-08-06): `spec-guard` does not yet contain a
/// `check-ci-gate.sh --self-test` step — this test FAILS until the Green
/// phase implements AC-008. Proven RED locally: `cargo test --test
/// ci_gate_completeness
/// test_spec_guard_contains_check_ci_gate_self_test_step` fails with the
/// diagnostic below.
///
/// Anchoring: assertions are made only within the `spec-guard` and
/// `ci-gate` job blocks respectively.
#[test]
fn test_spec_guard_contains_check_ci_gate_self_test_step() {
    let ci = read_ci_yml();
    let spec_guard_block = extract_job_block(&ci, "spec-guard").unwrap_or_else(|| {
        panic!("FAIL: `.github/workflows/ci.yml` does not contain a `spec-guard:` job.")
    });

    assert!(
        spec_guard_block.contains("check-ci-gate.sh") && spec_guard_block.contains("--self-test"),
        "FAIL (RED GATE, S-CIGATE-2 AC-008): `spec-guard` does not contain \
         a step running `scripts/check-ci-gate.sh --self-test`.\n\
         Required: add a step mirroring the existing self-test pairing \
         pattern already used in this job for \
         `check-cargo-mutants-policy-citations.sh --self-test` and \
         `check-bc-citation-symbols.sh --self-test`.\n\
         Current spec-guard block:\n{spec_guard_block}"
    );

    // The self-test must NOT be wired into ci-gate itself — structurally
    // impossible/circular (a gate cannot depend on a job that depends on
    // it). This distinguishes the real gate check (against the actual
    // `needs` payload) from the fixture suite.
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_default();
    assert!(
        !gate_block.contains("--self-test"),
        "FAIL (S-CIGATE-2 AC-008): `ci-gate` itself contains a \
         `--self-test` invocation. The self-test must live in \
         `spec-guard`, not `ci-gate` (a gate cannot depend on a job that \
         depends on it, and `ci-gate` has no other steps to host it \
         alongside the real gate check).\n\
         Current ci-gate block:\n{gate_block}"
    );
}

// ---------------------------------------------------------------------------
// S-CIGATE-2 AC-006 — `mutants` job structure pinned (Option C drift guard)
// ---------------------------------------------------------------------------

/// S-CIGATE-2 AC-006: Option C's defining constraint is that the `mutants`
/// job is left entirely UNCHANGED by this story — no NEW step-level `if:`
/// guards are added to any of its six steps. (The rejected Option B would
/// have required exactly that: step-level `if:` guards on five of the six
/// steps, plus a new branch in `Check kill rate`.) This test pins the six
/// step identifiers, the job-level `if: github.event_name ==
/// 'pull_request'` guard, and the single pre-existing step-level `if:
/// always()` on `Check kill rate` — so that a mid-implementation drift
/// toward Option B (adding a step-level `if:` to any of the OTHER five
/// steps) is caught by CI, not just by a manual PR-description diff check.
///
/// This is a content-checking pin (not a bare "unchanged" assertion) per
/// this repo's naming-convention carve-out precedent (S-626-1): it asserts
/// the specific structural facts Option C's "mutants is unchanged" claim
/// depends on, not merely that some arbitrary text is stable.
///
/// Not expected to be RED at Red Gate time: `mutants` has not yet been
/// touched by S-CIGATE-2's Green phase. This is a preventive pin exercised
/// throughout the implementation, not a Red Gate assertion for this
/// story's own fix (contrast with the two tests above, which target AC-001
/// and AC-008 directly and ARE proven RED now).
///
/// Anchoring: assertion is made only within the `mutants` job block.
#[test]
fn test_mutants_job_structure_unchanged_by_cigate2_option_c() {
    let ci = read_ci_yml();
    let mutants_block = extract_job_block(&ci, "mutants").unwrap_or_else(|| {
        panic!("FAIL: `.github/workflows/ci.yml` does not contain a `mutants:` job.")
    });

    // Job-level if: unchanged — PR-only scope, the exact fact
    // `scripts/check-ci-gate.sh`'s `ALLOWED_SKIPS` allowlist is built to
    // tolerate (mutants reports `skipped` on push BECAUSE of this guard).
    assert!(
        mutants_block.lines().any(|l| {
            let t = l.trim_start();
            t.starts_with("if:") && t.contains("github.event_name == 'pull_request'")
        }),
        "FAIL (S-CIGATE-2 AC-006): `mutants`'s job-level `if: \
         github.event_name == 'pull_request'` guard is missing or was \
         changed. Option C leaves the `mutants` job entirely UNCHANGED — \
         this guard (and the fact that `mutants` therefore reports \
         `skipped` on push) is exactly what `scripts/check-ci-gate.sh`'s \
         `ALLOWED_SKIPS` allowlist is built to tolerate.\n\
         Current mutants block:\n{mutants_block}"
    );

    // All six steps present, unchanged (Option C's principal advantage
    // over the rejected Option B).
    let required_step_names = [
        "Harden the runner (Audit all outbound calls)",
        "Run mutation tests on PR diff",
        "Check kill rate",
    ];
    for step_name in &required_step_names {
        assert!(
            mutants_block.contains(step_name),
            "FAIL (S-CIGATE-2 AC-006): `mutants` is missing expected step \
             `{step_name}`. Option C requires the `mutants` job to remain \
             unchanged.\n\
             Current mutants block:\n{mutants_block}"
        );
    }
    let required_uses_prefixes = [
        "uses: actions/checkout@",
        "uses: taiki-e/install-action@",
        "uses: Swatinem/rust-cache@",
    ];
    for prefix in &required_uses_prefixes {
        assert!(
            mutants_block.contains(prefix),
            "FAIL (S-CIGATE-2 AC-006): `mutants` is missing an expected \
             step using `{prefix}`. Option C requires the `mutants` job to \
             remain unchanged (no steps added, removed, or reordered).\n\
             Current mutants block:\n{mutants_block}"
        );
    }

    // Exactly one step-level `if:` (8-space indent) must exist in the
    // block — the pre-existing `if: always()` on `Check kill rate`. A NEW
    // step-level `if:` on any of the other five steps would be the
    // rejected Option B's signature edit (moving PR-only gating from job
    // level down to individual steps).
    let step_if_lines: Vec<&str> = mutants_block
        .lines()
        .filter(|l| l.starts_with("        if:"))
        .collect();

    assert_eq!(
        step_if_lines.len(),
        1,
        "FAIL (S-CIGATE-2 AC-006): expected exactly one step-level `if:` \
         in `mutants` (the pre-existing `if: always()` on `Check kill \
         rate`). Found {}: {:?}\n\
         A NEW step-level `if:` on any of the other five steps (Harden the \
         runner, checkout, install-action, rust-cache, Run mutation tests \
         on PR diff) would be the rejected Option B's signature edit — \
         Option C requires `mutants` to remain entirely unchanged.\n\
         Current mutants block:\n{mutants_block}",
        step_if_lines.len(),
        step_if_lines
    );

    assert!(
        step_if_lines[0].contains("always()"),
        "FAIL (S-CIGATE-2 AC-006): the sole step-level `if:` in `mutants` \
         no longer reads `if: always()` (found: {:?}).\n\
         Current mutants block:\n{mutants_block}",
        step_if_lines[0]
    );
}

// ---------------------------------------------------------------------------
// CRITICAL — behavioral closure (PR #671 review, round 3).
//
// Rounds 1 and 2 (see the CRITICAL-1/round-2 section below) each guarded a
// REPRESENTATION of `ALLOWED_SKIPS`: round 1 parsed the array literal's
// source text; round 2 replaced that with a regex-narrowed occurrence count
// plus a shell-out that asked bash for the array's PRINTED value. Both were
// independently bypassed by ordinary bash constructions that never touch
// the three guarded shapes:
//   - Mutation A: `ALLOWED_SKIPS[9]=test` as the first statement inside
//     `evaluate_needs` — a subscripted assignment. It matches none of the
//     three counted shapes (`ALLOWED_SKIPS=`, `ALLOWED_SKIPS+=`,
//     `${ALLOWED_SKIPS`), and `--print-allowed-skips` never runs
//     `evaluate_needs`, so the print-mode test's own shell-out never
//     observes it either — the mutation desyncs the two round-2 guards via
//     CONTROL FLOW, not declaration form.
//   - Mutation B: inside `is_allowed_skip`, `EXTRA_SKIPS=("test" "deny"
//     "clippy")` followed by `for allowed in "${ALLOWED_SKIPS[@]}"
//     "${EXTRA_SKIPS[@]}"; do` — a parallel array read alongside the real
//     one. `--print-allowed-skips` still prints only `mutants` (it reads
//     `ALLOWED_SKIPS` alone, correctly, but that is no longer the full
//     set `is_allowed_skip` actually honors).
// Both reproduced end-to-end: gate rc=0 on payloads with the
// illegitimately-tolerated jobs `skipped`, while `--self-test` stayed
// 12/12 and both round-1/round-2 tests stayed green.
//
// The fix is to stop testing any representation of the trust boundary and
// instead test the gate's ACTUAL DECISION, derived entirely from
// `ci.yml`'s real job graph — see
// `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`
// below. THIS is the closure. The round-1/round-2 tests below it are kept
// as diagnostics (they fire with a more specific message pointing at
// `ALLOWED_SKIPS` when the behavioral test's failure would otherwise only
// say "the gate accepted/rejected a payload it shouldn't have") — they are
// not, and must not be described as, the enforcement mechanism.
// ---------------------------------------------------------------------------

/// Determine whether a `ci.yml` job block's job-level `if:` expression can
/// actually evaluate to `false` — not merely whether an `if:` line is
/// present.
///
/// CRITICAL (PR #671 review, round 4/5, "M1"): the prior version of this
/// function (`job_has_job_level_if`, presence-only: any line starting with
/// exactly 4-space `if:`, not 8+ spaces for a step-level `if:` inside
/// `steps:`) accepted a semantically no-op condition. Reproduced
/// end-to-end: adding `if: ${{ always() }}` to `deny` (a genuine no-op —
/// `deny` has no `needs:`, so it runs unconditionally either way; also the
/// EXACT idiom `ci-gate` itself uses in this same file, so it reads as
/// unremarkable in a diff) plus `ALLOWED_SKIPS=("mutants" "deny")` in
/// `scripts/check-ci-gate.sh` passed `--self-test` (12/12) and `cargo test
/// --test ci_gate_completeness` (13/13), and the real gate accepted `deny:
/// skipped` with rc=0.
///
/// Worse: with ONLY the ci.yml half applied (script untouched), the OLD
/// oracle made THIS TEST'S OWN FAILURE MESSAGE steer a maintainer toward
/// completing the bypass — it said `deny` "carries a job-level `if:` ...
/// (the legitimate case, like `mutants` today)" and that rejecting the
/// skip "would be a false-red: the gate must still tolerate the one
/// legitimate skip", directly inviting the second edit
/// (`ALLOWED_SKIPS+=("deny")`) that turns the whole suite green. A test
/// that asks for its own bypass on an intermediate step is worse than one
/// that merely misses it.
///
/// FIX (this function): a job-level `if:` only counts as "the legitimate
/// case" when its expression:
///   1. Is NOT exactly one of the known always-true no-ops (`always()`,
///      `true`, `success()`, `!cancelled()`, `!failure()`, with or without
///      a `${{ }}` wrapper), AND
///   2. References at least one event- or config-dependent context value:
///      `github.event_name`, `github.ref`, `vars.`, `inputs.`, or
///      `github.event.` — something whose value can plausibly differ
///      between CI runs.
///
/// STATED RESIDUAL LIMITATION (per PR #671 review round 5 — this is a
/// proxy, not a full boolean evaluator, and is not claimed as closure): a
/// sufficiently adversarial boolean combination that satisfies rule 2
/// while remaining vacuously true regardless of the referenced value's
/// actual value — e.g. `always() || github.event_name == 'push'`, which
/// technically contains `github.event_name` but is always true because of
/// the leading `always() ||` — would still defeat this proxy. Closing that
/// gap would require actually evaluating the expression's possible truth
/// values (a real boolean-expression parser/evaluator for GitHub Actions
/// expression syntax), not pattern-matching its text. Out of scope here;
/// recorded as a known, accepted limit rather than silently left uncaught.
/// IMPORTANT 3 (CLAUDE.md correction, same review round): because ci.yml's
/// half of the trust boundary has no code guard at all (only this
/// pattern-based proxy plus ordinary code review), a sufficiently
/// determined bypass targeting exactly this residual gap is possible; the
/// pairing of (a real job-level `if:`) AND (`ALLOWED_SKIPS` membership) is
/// the trust boundary, not either alone, and ci.yml changes are visible in
/// PR diffs where a reviewer is expected to catch a `||`-with-`always()`
/// construction on inspection.
fn job_if_can_evaluate_false(job_block: &str) -> bool {
    let if_line = job_block
        .lines()
        .find(|l| l.starts_with("    if:") && !l.starts_with("        "));

    let Some(if_line) = if_line else {
        return false;
    };

    let expr = if_line
        .trim_start()
        .strip_prefix("if:")
        .unwrap_or("")
        .trim()
        .trim_start_matches("${{")
        .trim_end_matches("}}")
        .trim();

    const ALWAYS_TRUE_NOOPS: &[&str] = &[
        "always()",
        "true",
        "success()",
        "!cancelled()",
        "!failure()",
    ];
    if ALWAYS_TRUE_NOOPS.contains(&expr) {
        return false;
    }

    const EVENT_OR_CONFIG_MARKERS: &[&str] = &[
        "github.event_name",
        "github.ref",
        "vars.",
        "inputs.",
        "github.event.",
    ];
    EVENT_OR_CONFIG_MARKERS.iter().any(|m| expr.contains(m))
}

/// Build a minimal `toJSON(needs)`-shaped JSON payload: every job in
/// `all_jobs` reports `success`, except `target_job`, which reports
/// `skipped`.
fn build_single_skip_payload(all_jobs: &[String], target_job: &str) -> String {
    let mut obj = serde_json::Map::new();
    for job in all_jobs {
        let result = if job == target_job {
            "skipped"
        } else {
            "success"
        };
        obj.insert(job.clone(), serde_json::json!({ "result": result }));
    }
    serde_json::Value::Object(obj).to_string()
}

/// Run `scripts/check-ci-gate.sh` with `json_payload` on stdin (via a real
/// bash subprocess, not an in-process call — this is a black-box test of
/// the gate's actual decision), returning its full `Output` (exit status
/// AND captured stdout/stderr — IMPORTANT 1, PR #671 review round 5:
/// inspecting only the exit status let rc=2/rc=127/an-unrelated-rc=1 all
/// satisfy a bare `!status.success()` negative assertion; the caller now
/// also checks stdout for the specific `OK`/`FAIL` line naming the job).
/// The payloads used by this test are a few hundred bytes and the
/// script's own output is a handful of short lines, both far under a pipe
/// buffer, so a plain write-then-wait is sufficient (no risk of the
/// classic write/read deadlock this pattern can have with larger
/// payloads).
#[cfg(unix)]
fn run_check_ci_gate_sh(json_payload: &str) -> std::process::Output {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-ci-gate.sh");
    let mut child = Command::new("bash")
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("Could not spawn {}: {e}", script_path.display()));

    child
        .stdin
        .take()
        .expect("child stdin was requested via Stdio::piped()")
        .write_all(json_payload.as_bytes())
        .unwrap_or_else(|e| panic!("Could not write to child stdin: {e}"));

    child
        .wait_with_output()
        .unwrap_or_else(|e| panic!("Could not wait on child: {e}"))
}

/// S-CIGATE-2 CRITICAL (PR #671 review, round 3, tightened round 5) — the
/// behavioral closure. See the module comment above this section for the
/// full history of why rounds 1 and 2 (representation-based guards) were
/// each bypassed, and `job_if_can_evaluate_false`'s doc comment for why
/// round 3's presence-only `if:` check (M1) was itself insufficient and
/// this function's stated residual limitation.
///
/// For every job in `ci-gate.needs` (parsed from `ci.yml`, so this test
/// tracks the real job graph automatically as it changes):
///   - if the job's `ci.yml` block has NO `if:` that can evaluate `false`
///     (an always-run job — `fmt`, `clippy`, `test`, `msrv`, `deny`,
///     `spec-guard`, `check-signing-workflow-injection` today), synthesize
///     a payload where THAT job reports `skipped` and every other
///     `ci-gate.needs` job reports `success`, run
///     `scripts/check-ci-gate.sh` against it in a real subprocess, and
///     assert: exit code is EXACTLY 1 (not merely non-zero — IMPORTANT 1,
///     PR #671 review round 5: a bare non-zero check is satisfied by
///     rc=2/rc=127/an-unrelated-rc=1) AND stdout contains a `FAIL  <job> =
///     skipped` line naming this specific job.
///   - if the job's `ci.yml` block HAS an `if:` that can evaluate `false`
///     (`mutants` today), synthesize the same payload shape and assert:
///     exit code is EXACTLY 0 AND stdout contains an `OK  <job> = skipped`
///     line naming this specific job. This half matters exactly as much as
///     the first: a fix that made the gate reject everything would trade
///     the false-green this story fixes for a false-red, silently breaking
///     every normal push.
///
/// Non-vacuity (IMPORTANT 1): also asserts at least one job took the
/// positive branch and at least one took the negative branch, so a
/// pathological mutation that makes EVERY payload fail the same way (e.g.
/// reading the payload from an env var instead of stdin, so every
/// `run_check_ci_gate_sh` call sees empty input and every negative
/// assertion passes "by accident") cannot pass silently — it would trip
/// this assertion instead, and additionally now fails on message content,
/// not just exit code, for exactly that reason.
///
/// This test does not parse, count, or shell out to ask about
/// `ALLOWED_SKIPS` at all — it has no opinion on how the script represents
/// its allowlist internally. A subscripted assignment, a nameref, `read
/// -a`, `mapfile`, a parallel array, or a fully rewritten
/// `is_allowed_skip` can only pass this test by making the SCRIPT ITSELF
/// produce the correct exit code AND the correct diagnostic line on every
/// one of these synthesized payloads — which is the actual property this
/// story exists to guarantee.
///
/// `#[cfg(unix)]`: `scripts/check-ci-gate.sh` only ever runs on
/// `ubuntu-latest` in this repo's actual CI; this test still runs on
/// `ubuntu-latest` and `macos-latest` in the `test` job's 3-OS matrix (the
/// `test` matrix requires all three legs green, so this is not a coverage
/// gap on the platform that matters).
///
/// Proven RED (this round, with sha256-verified byte-identical restores)
/// against: M1 both halves together, M1's ci.yml-only half in isolation
/// (now failing with a NON-STEERING message — see this round's fix-report
/// for the exact before/after text), the env-var-instead-of-stdin vacuity
/// mutation, and a trailing-YAML-comment `extract_job_block` over-run.
/// Also re-confirmed RED (unchanged from round 3) against mutations A and
/// B and all four round-2 `+=`-form bypasses.
#[cfg(unix)]
#[test]
fn test_ci_gate_decision_matches_job_level_if_for_every_needs_member() {
    let ci = read_ci_yml();
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_else(|| {
        panic!("FAIL: `.github/workflows/ci.yml` does not contain a `ci-gate:` job.")
    });
    let needs = parse_needs_set(gate_block).unwrap_or_else(|| {
        panic!("FAIL: the `ci-gate` job block does not contain a `needs:` key.")
    });

    let mut all_jobs: Vec<String> = needs.into_iter().collect();
    all_jobs.sort(); // deterministic order; not behaviorally required, just stable failure output

    assert!(
        !all_jobs.is_empty(),
        "FAIL: `ci-gate.needs` is empty — cannot synthesize any payload."
    );

    let mut saw_positive_branch = false;
    let mut saw_negative_branch = false;

    for job in &all_jobs {
        let job_block = extract_job_block(&ci, job).unwrap_or_else(|| {
            panic!("FAIL: `ci-gate.needs` names `{job}`, but no `{job}:` job exists in ci.yml.")
        });
        let can_evaluate_false = job_if_can_evaluate_false(job_block);

        let payload = build_single_skip_payload(&all_jobs, job);
        let output = run_check_ci_gate_sh(&payload);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let exit_code = output.status.code();

        if can_evaluate_false {
            saw_positive_branch = true;
            let expected_line = format!("OK  {job} = skipped");
            assert!(
                exit_code == Some(0) && stdout.contains(&expected_line),
                "FAIL (S-CIGATE-2 behavioral closure, PR #671 review round \
                 5): `{job}` carries a job-level `if:` that CAN evaluate \
                 `false` in ci.yml (the legitimate case, like `mutants` \
                 today), but scripts/check-ci-gate.sh did not accept a \
                 payload where `{job}` is `skipped` and every other \
                 ci-gate.needs job is `success` — exit code: {exit_code:?} \
                 (expected Some(0)), stdout did{}contain \"{expected_line}\" \
                 (expected: contains it).\n\
                 \n\
                 This would be a false-red: the gate must still tolerate \
                 the one legitimate PR-only/repo-variable-gated skip, not \
                 just reject everything (a fix that rejects every skip \
                 would trade this story's false-green for a false-red on \
                 every normal push).\n\
                 \n\
                 Payload used:\n{payload}\n\
                 \n\
                 --- stdout ---\n{stdout}\n\
                 --- stderr ---\n{}",
                if stdout.contains(&expected_line) {
                    " "
                } else {
                    " NOT "
                },
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            saw_negative_branch = true;
            let expected_line = format!("FAIL  {job} = skipped");
            assert!(
                exit_code == Some(1) && stdout.contains(&expected_line),
                "FAIL (S-CIGATE-2 behavioral closure, PR #671 review round \
                 5): `{job}` has NO job-level `if:` that can evaluate \
                 `false` in ci.yml (an always-run job, or one whose `if:` \
                 is a semantically no-op condition like `always()` — M1, \
                 PR #671 review round 4/5), but scripts/check-ci-gate.sh \
                 did not reject a payload where `{job}` is `skipped` and \
                 every other ci-gate.needs job is `success` — exit code: \
                 {exit_code:?} (expected Some(1)), stdout did{}contain \
                 \"{expected_line}\" (expected: contains it).\n\
                 \n\
                 An always-run job's `skipped` result silently passed the \
                 sole required branch-protection check — the exact \
                 false-green class this story exists to fix, regardless of \
                 how ALLOWED_SKIPS is represented internally (this test \
                 does not inspect that representation at all; it only ran \
                 the real script against a real payload).\n\
                 \n\
                 Payload used:\n{payload}\n\
                 \n\
                 --- stdout ---\n{stdout}\n\
                 --- stderr ---\n{}",
                if stdout.contains(&expected_line) {
                    " "
                } else {
                    " NOT "
                },
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    assert!(
        saw_positive_branch,
        "FAIL: no job in ci-gate.needs has an `if:` that can evaluate \
         `false` — the positive (legitimate-skip) branch above never ran, \
         so this test cannot prove the gate still tolerates a real skip. \
         This should not happen while `mutants` remains in ci-gate.needs \
         with its current `if:`; if it does, something upstream (job list, \
         `if:` detection) changed unexpectedly."
    );
    assert!(
        saw_negative_branch,
        "FAIL: no job in ci-gate.needs lacks an `if:` that can evaluate \
         `false` — the negative (reject-illegitimate-skip) branch above \
         never ran. Without at least one negative case, a pathological \
         mutation that makes every payload succeed (or every payload fail \
         identically, e.g. by reading input from the wrong source) could \
         pass this test vacuously."
    );
}

// ---------------------------------------------------------------------------
// CRITICAL-1 (PR #671 review, round 1) / round 2 — DIAGNOSTICS, not the
// closure. These two tests guard REPRESENTATIONS of `ALLOWED_SKIPS` (its
// printed value via `--print-allowed-skips`, and a narrow occurrence count
// over the script's source text). Round 3 proved both bypassable
// (mutations A and B above) without tripping either. They are kept because
// they fire with a message pointing directly at `ALLOWED_SKIPS` when it IS
// the cause — a faster diagnosis than the behavioral test's generic
// accept/reject message — but
// `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`
// above is the actual enforcement mechanism. Do not describe these two as
// sufficient on their own.
// ---------------------------------------------------------------------------

/// Read `scripts/check-ci-gate.sh` relative to the repo root.
fn read_check_ci_gate_sh() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-ci-gate.sh");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
    raw.replace("\r\n", "\n")
}

/// Count non-comment-line occurrences of an actual bash reference to the
/// `ALLOWED_SKIPS` array (an assignment/append, or a `${ALLOWED_SKIPS...}`
/// expansion) in `scripts/check-ci-gate.sh`'s source text.
///
/// Deliberately narrower than "any line containing the substring
/// `ALLOWED_SKIPS`": a `#`-prefixed comment line is excluded (the file's
/// doc comments mention "ALLOWED_SKIPS" in prose many times — counting
/// those would dwarf the real count and defeat the purpose), AND a
/// non-comment `echo "... ALLOWED_SKIPS ..."` human-readable message line
/// is also excluded — it mentions the array's name but neither assigns to
/// nor reads it. Only two shapes count as a real reference: an
/// assignment/append (`ALLOWED_SKIPS=` or `ALLOWED_SKIPS+=`, which is how
/// bash would ever WIDEN the array) and an expansion (`${ALLOWED_SKIPS`,
/// which is how bash READS it, e.g. `"${ALLOWED_SKIPS[@]}"`).
fn count_allowed_skips_code_occurrences(script: &str) -> usize {
    script
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .map(|l| {
            let assign_or_append =
                l.matches("ALLOWED_SKIPS=").count() + l.matches("ALLOWED_SKIPS+=").count();
            let expansion = l.matches("${ALLOWED_SKIPS").count();
            assign_or_append + expansion
        })
        .sum()
}

/// CRITICAL (PR #671 review, round 2): the round-1 fix
/// (`parse_allowed_skips_from_script`, retired below) was ITSELF a
/// form-specific text parser — it recognized exactly one bash
/// array-declaration shape (`ALLOWED_SKIPS=("a" "b")` on a single line,
/// double-quoted tokens) and silently returned only what that one shape
/// matched. Bash honors far more. Reproduced independently: appending
/// `ALLOWED_SKIPS+=("test" "deny" "clippy")` on the line immediately after
/// the declaration desynced completely from that parser — `--self-test`
/// stayed 11/11, the OLD version of this test stayed green (it only ever
/// saw the first line), and a `needs` payload with `test`/`deny`/`clippy`
/// all `skipped` yielded gate rc=0. The same class of defect this story
/// fixes (a decision maker's authority silently exceeding what a static
/// check verifies), one layer down, inside the fix for the first layer.
///
/// FIX: eliminate the second parser rather than patch it for one more
/// form. `scripts/check-ci-gate.sh` gained a `--print-allowed-skips` mode
/// (`print_allowed_skips()`: `printf '%s\n' "${ALLOWED_SKIPS[@]}"`) that
/// asks BASH ITSELF what it considers `ALLOWED_SKIPS` to be — the same
/// in-memory array `evaluate_needs()` consults, AS BASH EVALUATES IT AT
/// THE POINT `print_allowed_skips` RUNS (i.e. after every
/// declaration/append that executed before that call). No
/// array-DECLARATION form can desync from this.
///
/// Proven RED against all four independently-reproduced bypass forms
/// before this fix, and confirmed still-passing after a byte-identical
/// restore (sha256-verified) between each: `+=` append, single-quoted
/// entries, bare words, and `"${OTHER[@]}"` expansion — see this round's
/// fix-report for per-form output of `--print-allowed-skips` proving each
/// is correctly reflected.
///
/// CORRECTION (PR #671 review, round 3): the claim above is narrower than
/// it originally read. This test asks bash for `ALLOWED_SKIPS`'s printed
/// value, which is faithful to any DECLARATION-form widening (any way of
/// constructing the array literal). It is NOT faithful to a
/// CONTROL-FLOW-based desync: a subscripted assignment
/// (`ALLOWED_SKIPS[9]=test`) placed as the first statement inside
/// `evaluate_needs` — a code path `--print-allowed-skips` never
/// executes — silently widens what the real gate honors while this test's
/// own shell-out still observes only `mutants`. Reproduced end-to-end;
/// see `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`
/// above, which is the actual behavioral closure and does not depend on
/// this test (or this function's `--print-allowed-skips` call) at all.
///
/// `#[cfg(unix)]`: `scripts/check-ci-gate.sh` is a bash script that only
/// ever runs on `ubuntu-latest` in this repo's actual CI (`ci-gate` and
/// `spec-guard` are both `runs-on: ubuntu-latest`) — gating this
/// bash-shell-out test to unix platforms avoids depending on Windows CI
/// runners' bash/PATH availability for a script that never executes there
/// in production, without reducing coverage of the thing actually being
/// protected (this test still runs on both `ubuntu-latest` and
/// `macos-latest` in the `test` job's matrix).
#[cfg(unix)]
#[test]
fn test_allowed_skips_members_require_job_level_conditional_in_ci_yml() {
    use std::process::Command;

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-ci-gate.sh");
    let output = Command::new("bash")
        .arg(&script_path)
        .arg("--print-allowed-skips")
        .output()
        .unwrap_or_else(|e| panic!("Could not run {}: {e}", script_path.display()));

    assert!(
        output.status.success(),
        "FAIL: `scripts/check-ci-gate.sh --print-allowed-skips` exited \
         non-zero (status: {:?}).\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let allowed_skips: Vec<String> = stdout
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    assert!(
        !allowed_skips.is_empty(),
        "FAIL: `scripts/check-ci-gate.sh --print-allowed-skips` printed no \
         job names — either ALLOWED_SKIPS is unexpectedly empty (it should \
         contain at least `mutants`) or the `--print-allowed-skips` mode is \
         broken.\nstdout: {stdout}"
    );

    let ci = read_ci_yml();

    for job in &allowed_skips {
        let job_block = extract_job_block(&ci, job).unwrap_or_else(|| {
            panic!(
                "FAIL: scripts/check-ci-gate.sh's ALLOWED_SKIPS (per \
                 --print-allowed-skips) names `{job}`, but no `{job}:` job \
                 exists in `.github/workflows/ci.yml`. A job named in \
                 ALLOWED_SKIPS must be a real job."
            )
        });

        let has_job_level_if = job_block
            .lines()
            .any(|l| l.starts_with("    if:") && !l.starts_with("        "));

        assert!(
            has_job_level_if,
            "FAIL (CRITICAL, PR #671 review): `{job}` is listed in \
             scripts/check-ci-gate.sh's ALLOWED_SKIPS (per \
             --print-allowed-skips), but its ci.yml job block has no \
             job-level `if:` condition — meaning `{job}` always runs and \
             can never legitimately report `skipped`.\n\
             \n\
             Allowlisting an always-run job makes the gate STRICTLY WEAKER \
             than the retired inline condition it replaced: a future \
             accidental (or malicious) skip of `{job}` would silently pass \
             the sole required branch-protection check.\n\
             \n\
             Fix: remove `{job}` from ALLOWED_SKIPS, or — if `{job}` is \
             being given a genuine conditional (e.g. `if: \
             github.event_name == 'pull_request'`) — add that conditional \
             to its ci.yml job block FIRST, in the same change, so this \
             test can verify the reason before the carve-out is granted.\n\
             \n\
             Current `{job}` block:\n{job_block}"
        );
    }
}

/// Belt-and-braces occurrence check (PR #671 review, round 2): independent
/// of `--print-allowed-skips` entirely — this test does not shell out to
/// anything and does not try to interpret bash syntax. It counts
/// non-comment-line occurrences of an actual bash reference to
/// `ALLOWED_SKIPS` (an assignment/append via `ALLOWED_SKIPS=` /
/// `ALLOWED_SKIPS+=`, or a `${ALLOWED_SKIPS...}` expansion — see
/// `count_allowed_skips_code_occurrences`'s doc comment for why plain
/// substring-of-any-non-comment-line is too broad: several `echo` messages
/// mention "ALLOWED_SKIPS" in human-readable text without referencing the
/// array at all) in `scripts/check-ci-gate.sh`'s source text, and asserts
/// the count equals exactly the known set of legitimate code-level sites:
/// the declaration (`ALLOWED_SKIPS=("mutants")`), the read in
/// `is_allowed_skip` (`for allowed in "${ALLOWED_SKIPS[@]}"`), and the read
/// in `print_allowed_skips` (`printf '%s\n' "${ALLOWED_SKIPS[@]}"`) — 3
/// total.
///
/// Why this matters even with `--print-allowed-skips` in place: that mode
/// asks bash for whatever `ALLOWED_SKIPS` evaluates to AT THE POINT
/// `print_allowed_skips` runs. A pathological future edit could place a
/// widening line (e.g. an `ALLOWED_SKIPS+=(...)` append) AFTER
/// `print_allowed_skips` is defined but reachable only via some other code
/// path, or otherwise structure the file so the widening is real (honored
/// by `evaluate_needs()`) without necessarily being visible to
/// `--print-allowed-skips`'s own read. This test does not depend on
/// execution order or reachability at all — it is a pure count over the
/// file's text.
///
/// CORRECTION (PR #671 review, round 3): the doc comment previously
/// claimed this fails on "a NEW code-level reference to ALLOWED_SKIPS
/// anywhere in the file, unconditionally" — that overstates it. This test
/// counts exactly THREE textual shapes (`ALLOWED_SKIPS=`,
/// `ALLOWED_SKIPS+=`, `${ALLOWED_SKIPS`), not every possible code-level
/// reference. A subscripted assignment (`ALLOWED_SKIPS[9]=test`) matches
/// none of the three and is invisible to this count, as is a
/// `declare -n`/nameref alias, `read -a ALLOWED_SKIPS`, or `mapfile -t
/// ALLOWED_SKIPS`. Reproduced end-to-end (mutation A: a subscripted
/// assignment as the first statement of `evaluate_needs`) — this test
/// stayed green while the real gate accepted an illegitimate skip. This
/// test is a fast diagnostic for the three shapes it does cover, not a
/// general "any new reference" guarantee — see
/// `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`
/// above for the guarantee that does not depend on enumerating shapes at
/// all.
///
/// Runs on all platforms (no bash shell-out) — no `#[cfg(unix)]` needed.
#[test]
fn test_allowed_skips_has_exactly_three_code_level_references() {
    let script = read_check_ci_gate_sh();
    let count = count_allowed_skips_code_occurrences(&script);

    assert_eq!(
        count, 3,
        "FAIL (CRITICAL, PR #671 review round 2): expected exactly 3 \
         non-comment-line occurrences of `ALLOWED_SKIPS=` / \
         `ALLOWED_SKIPS+=` / `${{ALLOWED_SKIPS` in scripts/check-ci-gate.sh \
         (the declaration, the read in is_allowed_skip, and the read in \
         print_allowed_skips); found {count}.\n\
         \n\
         A NEW occurrence of one of those THREE SPECIFIC shapes (e.g. an \
         `ALLOWED_SKIPS+=(...)` append line added anywhere in the file) \
         would silently widen the allowlist and is caught here regardless \
         of where it sits relative to `--print-allowed-skips`'s own read \
         or whether it is reachable. This is narrower than \"any code-level \
         reference\" (PR #671 review round 5 correction): a subscripted \
         assignment (`ALLOWED_SKIPS[9]=test`), a nameref, `read -a`, or \
         `mapfile -t` targeting ALLOWED_SKIPS matches none of the three \
         shapes and is invisible to this count — see \
         `test_ci_gate_decision_matches_job_level_if_for_every_needs_member` \
         for the guarantee that does not depend on enumerating shapes.\n\
         \n\
         If this count legitimately needs to change (e.g. a future \
         refactor adds another consumer of ALLOWED_SKIPS), update the \
         expected constant here ONLY after confirming every occurrence is a \
         read, never a widening write."
    );
}

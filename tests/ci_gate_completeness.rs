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
///
/// BOM stripping (PR #671 review round 13, IMPORTANT 4): `fs::
/// read_to_string` does NOT strip a leading UTF-8 byte-order mark —
/// unlike PyYAML (and, by extension, most spec-compliant YAML parsers,
/// which strip a BOM at document start per the YAML spec) this file's own
/// line-based extractors would otherwise see a literal `\u{FEFF}` glued
/// to the first character of whatever line the BOM precedes, silently
/// corrupting that line's key-name match (e.g. `\u{FEFF}defaults` !=
/// `defaults`). A BOM is only ever meaningful at the very start of a
/// text stream — stripping it once here, rather than per-line in
/// `extract_key_name_at_indent`, matches where a real BOM can actually
/// occur. NOTE: whether `actions/runner`'s own YAML parser accepts a
/// BOM-prefixed workflow file at all is UNVERIFIED (a targeted search
/// found no documentation either way) — this fix closes a gap in THIS
/// CHECKER's fidelity to what PyYAML (a real, spec-compliant parser)
/// does with a BOM; it does not, by itself, establish that a BOM-fronted
/// `defaults:` is an exploitable end-to-end bypass of the real gate.
fn read_ci_yml() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
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

/// List every job name defined anywhere in `.github/workflows/ci.yml`'s
/// `jobs:` map — not just `ci-gate.needs` members.
///
/// Used by the pin-coverage check (PR #671 review round 9, IMPORTANT 2) as
/// the candidate universe for a direct behavioral probe of
/// `scripts/check-ci-gate.sh`'s actual `ALLOWED_SKIPS` enforcement: only a
/// name that is a real job in this file could ever appear as a key in a
/// real `toJSON(needs)` payload, so this is the correct (and only
/// practical) bound on "every job that could possibly need a pin".
///
/// Same 2-space-indent, comment-stripped convention as
/// `common::yaml::extract_job_block` (kept consistent deliberately — this
/// file already anchors on that exact shape everywhere else). `jobs:` is
/// this file's last top-level key (verified: `grep -n '^[a-zA-Z]'
/// .github/workflows/ci.yml` returns only `name:`, `on:`, `env:`, `jobs:`,
/// in that order), so scanning to EOF is correct today; the 0-indent
/// early-return guards against silently mis-scanning if that ever changes.
///
/// `#[cfg(unix)]` (PR #671 review round 15, CI-caught): this helper's only
/// caller is `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`,
/// which is itself `#[cfg(unix)]`-gated (the bash shell-outs it performs
/// don't run on Windows). Gating a TEST to unix does not remove the
/// HELPERS it alone uses from a Windows build — they still compile there,
/// now genuinely unused, and `-D warnings` promotes that to a hard clippy
/// error. `cargo clippy` run 31128902318 caught exactly this on
/// `windows-latest` (six items, this one included) after thirteen rounds
/// of macOS-only local review missed it entirely — local verification,
/// however rigorous, does not cover the platform matrix. Gate the helper
/// the same way as its only caller, every time.
#[cfg(unix)]
fn list_all_ci_yml_job_names(ci: &str) -> Vec<String> {
    let Some(jobs_start) = ci.find("\njobs:\n") else {
        panic!("FAIL: `.github/workflows/ci.yml` has no top-level `jobs:` key.");
    };
    let jobs_section = &ci[jobs_start + 1..];
    let mut names = Vec::new();
    for line in jobs_section.lines().skip(1) {
        if line.is_empty() || line.starts_with(' ') {
            if line.starts_with("  ") && line.chars().nth(2).map(|c| c != ' ').unwrap_or(false) {
                let without_comment = line.split('#').next().unwrap_or(line).trim_end();
                if let Some(name) = without_comment
                    .strip_prefix("  ")
                    .and_then(|s| s.strip_suffix(':'))
                {
                    if !name.is_empty() {
                        names.push(name.to_string());
                    }
                }
            }
            continue;
        }
        // A non-empty, 0-indent line: end of the `jobs:` map.
        break;
    }
    names
}

/// Does `line` declare the YAML key `key` at a job's direct-child level
/// (exactly 4-space indent, not 8+)?
///
/// PR #671 review round 10, IMPORTANT 2: recognizes `key`, `"key"`, and
/// `'key'` before the `:` (with any amount of whitespace between the key
/// token and the colon) — PyYAML-confirmed all four spellings
/// (`outputs:`, `"outputs":`, `'outputs':`, `outputs :`) parse to the
/// identical key. The original round-9 outputs-content guard matched only
/// the first, bare spelling — a real job-level `outputs:` block written
/// in any of the other three left that guard silently blind (the `{}`
/// premise it protects no longer held, but nothing said so). This
/// function's job is narrow enough (one literal key name at a time, one
/// fixed indent depth) that enumerating the recognized forms is tractable
/// and auditable, unlike `extract_and_normalize_if_expr`'s arbitrary
/// expression text, which is why that function rejects instead of trying
/// to enumerate.
///
/// The indent check is written as "starts with 4 spaces AND the 5th
/// character is not a space" rather than the round-9 original's `&&
/// !l.starts_with("        ")` (8 spaces) — that second clause could never
/// be false once the first clause (4-space prefix) matched a job-level
/// key, since no line can simultaneously start with exactly 4 spaces AND
/// 8 spaces. It read as protection against a deeper indent that it did
/// not actually provide; this version's clause does.
///
/// ACKNOWLEDGED LIMITATION (PR #671 review round 11, small correction 3,
/// verified independently via PyYAML — a job's own child keys need only
/// be indented CONSISTENTLY WITH EACH OTHER, not with any other job's
/// indent choice, so a job written with 6-space (or any other) indent for
/// ALL its own direct children is valid, `actionlint`-clean YAML): this
/// function's "exactly 4 spaces" assumption is the SAME file-wide
/// indentation-depth assumption every other 4/8/10-space-hardcoded check
/// in this file already makes (`extract_and_normalize_if_expr`'s
/// job-level `if:` detection, the M2 job/step-level checks, etc.) — this
/// function does not introduce a new weakness, it inherits an existing,
/// broader one. Solving it in general (indent-independent job-block
/// parsing) would touch every such function in this file under a single
/// unifying re-derivation of "what indent does THIS job actually use,"
/// which is a larger, dedicated change, not a one-line fix to this
/// function alone — scoped out of this round for that reason, matching
/// how review round 11 itself scoped it ("low stakes given the file-wide
/// 4-space assumption").
///
/// `#[cfg(unix)]` (PR #671 review round 15, CI-caught): only caller is
/// `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`,
/// itself `#[cfg(unix)]`-gated — see `list_all_ci_yml_job_names`'s doc
/// comment above for the full "gating a test orphans its helpers"
/// explanation.
#[cfg(unix)]
fn line_declares_job_level_key(line: &str, key: &str) -> bool {
    let Some(after_indent) = line.strip_prefix("    ") else {
        return false;
    };
    if after_indent.starts_with(' ') {
        return false; // 5+ leading spaces: a step-level or deeper key.
    }
    for quoted in [format!("\"{key}\""), format!("'{key}'"), key.to_string()] {
        if let Some(after_key) = after_indent.strip_prefix(quoted.as_str()) {
            if after_key.trim_start().starts_with(':') {
                return true;
            }
        }
    }
    false
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
        "FAIL (M2-g): The `ci-gate` job block contains no `run:` step \
         matching this check's pattern (`run:` as the first non-whitespace \
         text on its own line).\n\
         The gate must have a `run:` step that actually invokes \
         `scripts/check-ci-gate.sh` and fails the job via that script's \
         exit code.\n\
         Without a `run:` step the job trivially succeeds for every upstream \
         result. NOTE (PR #671 review round 12, benign-false-red message \
         fix): if a `run:` step genuinely exists but its KEYS were \
         reordered so `run:` is now the step's FIRST key (the one on the \
         same line as the YAML sequence marker, e.g. `      - run: ...` \
         instead of `      - name: ...` followed by `run:` later), this \
         check's `trim_start()`-based pattern does not strip the leading \
         `- ` marker and so does not recognize it — `run:` is not \
         missing, it is in an unrecognized POSITION. See M2-l below (step \
         key SETS, order-independent) for the check that still passes in \
         that case, confirming the step itself is intact.\n\
         Current ci-gate block:\n{gate_block}"
    );

    assert!(
        gate_block.contains("check-ci-gate.sh") && gate_block.contains("toJSON(needs)"),
        "FAIL (M2-h, S-CIGATE-2): The `ci-gate` job block's `run:` step \
         does not invoke `scripts/check-ci-gate.sh` fed `toJSON(needs)`.\n\
         Current ci-gate block:\n{gate_block}"
    );

    // -----------------------------------------------------------------------
    // Assertion 4 (M2-i, PR #671 review round 10, CRITICAL): M2-g/h above
    // only check that a `run:` step exists and mentions `check-ci-gate.sh`
    // + `toJSON(needs)` as SUBSTRINGS — an allowlist of known-good
    // substrings, with nothing verifying the script's exit code actually
    // reaches the job's conclusion. `echo "${NEEDS_JSON}" | bash
    // scripts/check-ci-gate.sh || true`, or the same line piped through
    // `| cat`, still contains both substrings and leaves the full suite
    // green while making the gate tolerate EVERY upstream failure, not
    // just an illegitimate skip. Pin the run line byte-for-byte instead
    // (after the same narrow, reject-don't-parse normalization used for
    // `if:` — see `extract_and_normalize_sole_run_line`'s doc comment for
    // why it is a separate function): any suffix or pipe stage changes the
    // normalized string and fails here.
    // -----------------------------------------------------------------------
    let actual_run_line =
        extract_and_normalize_sole_run_line(gate_block).unwrap_or_else(|reason| {
            panic!(
                "FAIL (M2-i, PR #671 review round 10, CRITICAL): the `ci-gate` \
             job's gate-decision `run:` line {reason}\n\
             This `run:` form is UNSUPPORTED for pinning and MUST be \
             rewritten as a single-line plain scalar before it can be \
             evaluated against PINNED_GATE_RUN_LINE at all.\n\
             Current ci-gate block:\n{gate_block}"
            )
        });

    assert_eq!(
        actual_run_line, PINNED_GATE_RUN_LINE,
        "FAIL (M2-i, PR #671 review round 10, CRITICAL): the `ci-gate` \
         job's `run:` line (\"{actual_run_line}\") does not byte-match the \
         pinned, human-reviewed literal (\"{PINNED_GATE_RUN_LINE}\"). \
         `scripts/check-ci-gate.sh`'s own exit code is the ONLY pass/fail \
         signal (see this test's M2-g rationale above) — ANY suffix or \
         pipe stage appended after it (`|| true`, `| cat`, `; exit 0`, \
         redirection, etc.) can swallow that signal while leaving every \
         substring-based check (M2-g, M2-h) satisfied. NOTE (PR #671 \
         review round 12, benign-false-red message fix): this checker \
         treats the `run:` value as an OPAQUE plain-scalar string and \
         does NOT interpret YAML quoting — a cosmetic rewrap of the exact \
         same command in single or double quotes (e.g. `run: 'echo \
         \"${{NEEDS_JSON}}\" | bash scripts/check-ci-gate.sh'`) changes \
         this byte-comparison even though the command GitHub Actions \
         actually runs is unchanged; that is not a bug in this check, it \
         is the same \"more literal is strictly safer than guessing which \
         rewrites are equivalent\" tradeoff `extract_and_normalize_if_expr` \
         makes for its `${{ }}` wrapper. If this is a deliberate, \
         reviewed change to the run line (quoting or otherwise), update \
         PINNED_GATE_RUN_LINE in the SAME change.\n\
         Current ci-gate block:\n{gate_block}"
    );

    // -----------------------------------------------------------------------
    // Assertion 5 (M2-j, PR #671 review round 10, CRITICAL): `continue-on-
    // error: true`, at either step level or job level, defeats the pinned
    // run line just as effectively as a shell-level `|| true` — and unlike
    // a shell suffix, it is invisible to a byte-for-byte comparison of the
    // run line's own text (the run line is untouched; the tolerance is
    // declared on a SIBLING key). Aggravating: `continue-on-error: true`
    // is already an established idiom in this same file (`ci.yml ::
    // mutants`, the `Run mutation tests on PR diff` step) — on `ci-gate`
    // specifically it reads as unremarkable in a diff. Reject the literal
    // substring anywhere in the job block (step-level OR job-level) rather
    // than trying to parse which key it attaches to — no legitimate use
    // of `continue-on-error` exists anywhere in `ci-gate`, so there is no
    // narrower rule to get subtly wrong.
    // -----------------------------------------------------------------------
    assert!(
        !gate_block.contains("continue-on-error"),
        "FAIL (M2-j, PR #671 review round 10, CRITICAL): the `ci-gate` job \
         block contains `continue-on-error`. This tolerates every upstream \
         failure regardless of the pinned run line's own text (M2-i) — the \
         tolerance is declared on a sibling key, not the run line itself. \
         `continue-on-error` has no legitimate use anywhere in `ci-gate`; \
         remove it.\n\
         Current ci-gate block:\n{gate_block}"
    );

    // -----------------------------------------------------------------------
    // Assertion 6 (M2-k, PR #671 review round 11, CRITICAL 3 / structural):
    // M2-i/j pinned the run line's VALUE and rejected one specific key
    // (`continue-on-error`) — still an enumeration of known-bad members,
    // one layer up from the retired inline condition. Pin the job's
    // COMPLETE key set instead: an added job-level key of ANY name (a
    // second `if:`, a future `outputs:`, anything not yet imagined)
    // changes this set and fails here, closing the class rather than
    // adding another named exception.
    // -----------------------------------------------------------------------
    let actual_job_keys = extract_job_level_key_set(gate_block);
    assert_eq!(
        actual_job_keys, PINNED_GATE_JOB_KEYS,
        "FAIL (M2-k, PR #671 review round 11, CRITICAL): the `ci-gate` \
         job's complete key set ({actual_job_keys:?}) does not match the \
         pinned, human-reviewed set ({PINNED_GATE_JOB_KEYS:?}). Any added, \
         removed, or renamed job-level key changes this set — that is the \
         point: enumerating individually-forbidden keys (as M2-j does for \
         `continue-on-error`) only ever closes the ONE member enumerated. \
         If this is a deliberate, reviewed change, update \
         PINNED_GATE_JOB_KEYS in the SAME change.\n\
         Current ci-gate block:\n{gate_block}"
    );

    // -----------------------------------------------------------------------
    // Assertion 7 (M2-l, PR #671 review round 11, CRITICAL 3): same
    // principle, one level deeper — pin every STEP's complete key set, in
    // order. Reproduced: adding `shell: cat {{0}}` to the gate step (GitHub
    // accepts a custom shell TEMPLATE `command [options] {{0}}`, so the
    // runner runs `cat <tempfile>` instead of executing the run line's
    // script — the run body never executes, and the step still exits 0)
    // left M2-i/M2-j/M2-k all satisfied (the run line's text, the absence
    // of `continue-on-error`, and the job's own key set are all
    // untouched) while producing a 14/14-green suite.
    // -----------------------------------------------------------------------
    let actual_step_keys = extract_gate_step_key_sets(gate_block);
    assert_eq!(
        actual_step_keys, PINNED_GATE_STEP_KEY_SETS,
        "FAIL (M2-l, PR #671 review round 11, CRITICAL): the `ci-gate` \
         job's per-step key sets ({actual_step_keys:?}) do not match the \
         pinned, human-reviewed sets ({PINNED_GATE_STEP_KEY_SETS:?}). Any \
         added, removed, or renamed key on ANY step (most importantly \
         `shell:`, which can silently replace the run line's script body \
         with `cat`'s output of it — see this assertion's doc comment),\
         or any added/removed/reordered step, changes this. If this is a \
         deliberate, reviewed change, update PINNED_GATE_STEP_KEY_SETS in \
         the SAME change.\n\
         Current ci-gate block:\n{gate_block}"
    );

    // -----------------------------------------------------------------------
    // Assertion 8 (M2-m, PR #671 review round 11, CRITICAL 2): M2-a/b/c
    // above require only that a job-level `if:` exists, contains
    // `always()`, and lacks the literal substring `contains(needs` — none
    // of which excludes `if: ${{ always() && false }}` or `if: ${{
    // always() && github.ref == 'refs/heads/nonexistent' }}` (the latter
    // is VERBATIM the always-FALSE bypass construction
    // `scripts/check-ci-gate.sh`'s own `ALLOWED_SKIPS` comment cites from
    // round 5 — applied there to a `ci-gate.needs` member's `if:`, never
    // to `ci-gate`'s OWN `if:`). Either form means the job never runs at
    // all; GitHub Actions reports the required check as `skipped`, and
    // branch protection treats a skipped required check as passing — this
    // is S-CIGATE-2's entire founding premise, reopened at the one job
    // whose `if:` this suite had never pinned. Reuses the EXISTING
    // `extract_and_normalize_if_expr` (previously applied only to
    // `ci-gate.needs` members for the `ALLOWED_SKIPS` pin) against a new,
    // separate pin — `ci-gate`'s own `if:` has nothing to do with
    // `ALLOWED_SKIPS` membership, so it gets its own constant rather than
    // an entry in `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS`.
    // -----------------------------------------------------------------------
    let actual_gate_if_expr = extract_and_normalize_if_expr(gate_block).unwrap_or_else(|reason| {
        panic!(
            "FAIL (M2-m, PR #671 review round 11, CRITICAL): `ci-gate`'s \
             own job-level `if:` {reason}\n\
             Current ci-gate block:\n{gate_block}"
        )
    });

    assert_eq!(
        actual_gate_if_expr.as_deref(),
        Some(PINNED_GATE_IF_EXPR),
        "FAIL (M2-m, PR #671 review round 11, CRITICAL): `ci-gate`'s own \
         `if:` ({actual_gate_if_expr:?}) does not byte-match the pinned, \
         human-reviewed literal (\"{PINNED_GATE_IF_EXPR}\"). \
         `if: ${{{{ always() && false }}}}` and `if: ${{{{ always() && \
         github.ref == 'refs/heads/nonexistent' }}}}` both contain \
         `always()` and lack `contains(needs`, satisfying M2-a/b/c while \
         permanently skipping the job — a skipped required check passes \
         branch protection. If this is a deliberate, reviewed change to \
         `ci-gate`'s own `if:`, update PINNED_GATE_IF_EXPR in the SAME \
         change.\n\
         Current ci-gate block:\n{gate_block}"
    );

    // -----------------------------------------------------------------------
    // Assertion 9 (M2-n, PR #671 review round 11, CRITICAL 1): M2-i pins
    // the run line's VALUE; M2-l (above) confirms `env:` exists as a KEY
    // on the gate step — neither says anything about what value `env:`'s
    // `NEEDS_JSON` child carries. Reproduced: replacing `${{
    // toJSON(needs) }}` with a hand-written, permanently-`success`
    // literal (e.g. `'{{"fmt":{{"result":"success","outputs":{{}}}}, ...}}'`)
    // left the run line, every step's key set, the job's key set, and
    // `ci-gate`'s own `if:` ALL untouched — 14/14 green — and piping that
    // fabricated literal into the real script yields exit 0
    // unconditionally, worse than round 10's `|| true`: the script's own
    // log then reads as a legitimate all-green run instead of a
    // suppressed real failure.
    // -----------------------------------------------------------------------
    let actual_needs_json_line = extract_and_normalize_sole_needs_json_line(gate_block)
        .unwrap_or_else(|reason| {
            panic!(
                "FAIL (M2-n, PR #671 review round 11, CRITICAL): the \
                 `ci-gate` job's `NEEDS_JSON:` env-child line {reason}\n\
                 Current ci-gate block:\n{gate_block}"
            )
        });

    assert_eq!(
        actual_needs_json_line, PINNED_GATE_NEEDS_JSON_LINE,
        "FAIL (M2-n, PR #671 review round 11, CRITICAL): the `ci-gate` \
         job's `NEEDS_JSON:` value (\"{actual_needs_json_line}\") does not \
         byte-match the pinned, human-reviewed literal \
         (\"{PINNED_GATE_NEEDS_JSON_LINE}\"). `NEEDS_JSON` is the ENTIRE \
         input `scripts/check-ci-gate.sh` ever sees — a fabricated literal \
         here defeats every other pin in this test (the run line, the key \
         sets, and `ci-gate`'s own `if:` all stay byte-identical). If this \
         is a deliberate, reviewed change, update \
         PINNED_GATE_NEEDS_JSON_LINE in the SAME change.\n\
         Current ci-gate block:\n{gate_block}"
    );

    // -----------------------------------------------------------------------
    // Assertion 10 (M2-o, PR #671 review round 12, CRITICAL 2): M2-n pins
    // the ONE `NEEDS_JSON:` line's VALUE but never asserted it is the
    // ONLY child of the gate step's `env:` block. A sibling
    // `BASH_ENV: <path>` alongside `NEEDS_JSON:` leaves every prior pin
    // satisfied while giving the runner's non-interactive bash shell a
    // script to source BEFORE the pinned run line's script body executes
    // — a `BASH_ENV` shim ending in `exit 0` ends the shell first,
    // regardless of `--norc`/`--noprofile` (verified locally under the
    // runner's actual invocation flags, not assumed). Pin the env:
    // block's complete key set the same way M2-l pins step key sets.
    // -----------------------------------------------------------------------
    let actual_gate_env_keys = extract_gate_env_key_set(gate_block);
    // PR #671 review round 13, CRITICAL fix: without this, a missing
    // `env:` block (e.g. the anchor logic failing to find it at all)
    // returns `Vec::new()`, which would only be caught by the assert_eq!
    // below as an ACCIDENT of `PINNED_GATE_ENV_KEYS` being non-empty —
    // change the pin to `&[]` in the same breath as a real bug and this
    // guard silently stops meaning anything.
    assert!(
        !actual_gate_env_keys.is_empty(),
        "FAIL (M2-o, PR #671 review round 13): `extract_gate_env_key_set` \
         returned an EMPTY key set for the gate step's `env:` block — \
         either the block is genuinely missing (in which case M2-n above \
         should already have failed on a missing `NEEDS_JSON:` line), this \
         function's anchoring logic failed to find it, or (PR #671 review \
         round 14 SUGGESTION) `env:` was legally reordered to AFTER \
         `run:` on this step — this function only scans BACKWARD from \
         `run:`, so a legal-but-reordered `env:` looks identical to a \
         missing one from here. An empty result here must never be \
         silently treated as \"no env vars to worry about\".\n\
         Current ci-gate block:\n{gate_block}"
    );
    assert_eq!(
        actual_gate_env_keys, PINNED_GATE_ENV_KEYS,
        "FAIL (M2-o, PR #671 review round 12, CRITICAL): the `ci-gate` \
         gate step's `env:` key set ({actual_gate_env_keys:?}) does not \
         match the pinned, human-reviewed set ({PINNED_GATE_ENV_KEYS:?}). \
         `NEEDS_JSON`'s VALUE is pinned by M2-n above, but a SIBLING env \
         var (most importantly `BASH_ENV`, which non-interactive bash \
         sources before running any script — including the pinned run \
         line's body — regardless of `--norc`/`--noprofile`) is a separate \
         attack this key-set pin exists to close. If this is a \
         deliberate, reviewed change, update PINNED_GATE_ENV_KEYS in the \
         SAME change.\n\
         Current ci-gate block:\n{gate_block}"
    );
}

/// PR #671 review round 11, CRITICAL 3 (workflow-level half):
/// `defaults: run: shell: cat {{0}}` at the WORKFLOW level (a top-level
/// key, a sibling of `name:`/`on:`/`env:`/`jobs:` — NOT inside any job
/// block) overrides the default shell for EVERY step in EVERY job in the
/// file that doesn't set its own `shell:`, including `ci-gate`'s gate
/// step. GitHub accepts a custom shell TEMPLATE `command [options] {{0}}`
/// for `shell:`; `cat {{0}}` makes the runner `cat` the run line's script
/// body (a temp file) instead of executing it — the pinned run line's
/// text is completely irrelevant once this fires, and it still exits 0.
/// Reproduced: 14/14 green.
///
/// This construct lives OUTSIDE every job block by construction, so NO
/// `extract_job_block`-anchored assertion — every other check in this
/// file, including all of M2-k/l/m/n above — can ever see it; `ci.yml`'s
/// top-level `defaults:` key is invisible to a function that only ever
/// receives the text between one job's name and the next. This is
/// therefore the ONLY test in this file that reads `ci.yml` at the
/// WORKFLOW level rather than scoping to a specific job's block — see
/// `common::yaml::extract_job_block`'s doc comment for why that function
/// itself cannot be asked to cover this case.
///
/// CORRECTED (PR #671 review round 12, CRITICAL 1): the original form of
/// this check was `ci.lines().any(|l| l == "defaults:")` — exact LINE
/// equality, not key detection. Round 10's `outputs:` finding, one level
/// up: reproduced end-to-end, flow style (`defaults: {{run: {{shell: "cat
/// {{0}}"}}}}`, a single added line, `actionlint`-clean), quoted
/// (`"defaults":`), a trailing comment, and a trailing space all left
/// this exact-equality check blind (15/15 green) while adding the exact
/// same workflow-level override. Fixed by reusing
/// `extract_key_name_at_indent(l, 0)` — the same quote/whitespace-aware
/// key matcher round 11 already built for job/step-level key sets,
/// applied here at indent 0 for the workflow's own top-level keys. This
/// was available in the SAME commit that introduced the exact-equality
/// form; the new check was written with `==` instead of calling it.
#[test]
fn test_ci_yml_has_no_workflow_level_shell_override() {
    let ci = read_ci_yml();
    let has_top_level_defaults = ci
        .lines()
        .any(|l| extract_key_name_at_indent(l, 0).as_deref() == Some("defaults"));

    assert!(
        !has_top_level_defaults,
        "FAIL (PR #671 review round 11, CRITICAL 3): \
         `.github/workflows/ci.yml` declares a top-level `defaults:` key. \
         A `defaults:\\n  run:\\n    shell: cat {{0}}` override at the \
         WORKFLOW level defeats every job's pinned run line in this file, \
         including `ci-gate`'s (see \
         `tests/ci_gate_completeness.rs::extract_and_normalize_sole_run_line`'s \
         pin) — GitHub accepts a custom shell TEMPLATE `command [options] \
         {{0}}`, so the runner would `cat` the run line's script body \
         instead of executing it, exiting 0 regardless of what the pinned \
         run line says. This construct lives OUTSIDE every job block, so \
         no job-block-anchored assertion in this suite can ever see it — \
         this test exists specifically because of that gap. `defaults:` \
         has no legitimate use in this file today; if one is ever needed, \
         this test must be updated deliberately in the SAME change, not \
         silently bypassed."
    );
}

/// PR #671 review round 12, CRITICAL 2 (workflow-level half): the
/// WORKFLOW's own top-level `env:` block is the second member of the
/// unpinned-env-siblings set M2-o (above) closes at the gate-step level.
/// `BASH_ENV` appended there is invisible to every job-block-anchored
/// check for the same structural reason `defaults:` is (see
/// `test_ci_yml_has_no_workflow_level_shell_override`'s doc comment) —
/// this is therefore the second (and, alongside that test, only) test in
/// this file reading `ci.yml` at the workflow level.
#[test]
fn test_ci_yml_workflow_level_env_key_set_is_pinned() {
    let ci = read_ci_yml();
    let actual_workflow_env_keys = extract_workflow_env_key_set(&ci);

    // PR #671 review round 13, CRITICAL fix: see the parallel assertion
    // in `test_ci_gate_pass_fail_semantics_are_structurally_placed` (M2-o)
    // for why an empty extraction result must never be silently trusted
    // as "nothing to worry about".
    assert!(
        !actual_workflow_env_keys.is_empty(),
        "FAIL (PR #671 review round 13): `extract_workflow_env_key_set` \
         returned an EMPTY key set for the workflow's own top-level \
         `env:` block — either the block is genuinely missing or this \
         function's anchoring logic failed to find it. An empty result \
         here must never be silently treated as \"no env vars to worry \
         about\"."
    );
    assert_eq!(
        actual_workflow_env_keys, PINNED_WORKFLOW_ENV_KEYS,
        "FAIL (PR #671 review round 12, CRITICAL 2): the WORKFLOW's own \
         top-level `env:` key set ({actual_workflow_env_keys:?}) does not \
         match the pinned, human-reviewed set \
         ({PINNED_WORKFLOW_ENV_KEYS:?}). This construct lives OUTSIDE \
         every job block (a sibling of `name:`/`on:`/`jobs:`/`defaults:`), \
         so no job-block-anchored assertion in this suite can ever see \
         it — most importantly `BASH_ENV`, which non-interactive bash \
         sources before running ANY job's script body, regardless of \
         `--norc`/`--noprofile`. If this is a deliberate, reviewed \
         change, update PINNED_WORKFLOW_ENV_KEYS in the SAME change."
    );
}

/// PR #671 review round 14, CRITICAL: `read_ci_yml()` normalizes `\r\n` to
/// `\n`, but every extractor in this file (`extract_key_name_at_indent`,
/// `extract_job_level_key_set`, `extract_gate_step_key_sets`,
/// `collect_mapping_key_set`, and both workflow-level env-key-set tests)
/// then operates on `str::lines()`, which splits ONLY on `\n`. YAML's own
/// line-break character set (YAML 1.1 `b-char`, which PyYAML implements)
/// is larger: CR (U+000D) alone, NEL (U+0085), LINE SEPARATOR (U+2028),
/// and PARAGRAPH SEPARATOR (U+2029) are all valid YAML line breaks that
/// end a LOGICAL line for a real parser but do NOT split a `str::lines()`
/// iteration on their own. A key placed after one of these, on the same
/// PHYSICAL text line as a preceding key, is therefore invisible to EVERY
/// line-based check in this file simultaneously — not one extractor's
/// bug, a property of the whole file-reading strategy.
///
/// Reproduced, each a genuine one-line diff, PyYAML- AND Ruby-Psych/
/// libyaml-confirmed legal (independent implementations agreeing rules
/// out a single-parser quirk), actionlint-clean, and — since
/// `.gitattributes`' `text eol=lf` normalizes CRLF pairs but does NOT
/// strip a lone CR or the three multi-byte Unicode line breaks —
/// confirmed to survive a real `git add`/commit:
///   1. `CARGO_TERM_COLOR: always<CR>  BASH_ENV: /tmp/shim.sh` reopens
///      round 12's workflow-env CRITICAL, for every job in the file.
///   2. `name: CI<CR>defaults: {{run: {{shell: "cat {{0}}"}}}}` reopens
///      round 11's workflow-level `defaults:` CRITICAL.
///   3. Appending `<CR>        shell: cat {{0}}` to the gate step's
///      `name:` line defeats `PINNED_GATE_STEP_KEY_SETS` — the step-key-
///      set pin confirmed default-deny against fourteen constructions in
///      round 12, but only over the lines `str::lines()` actually
///      produced; PyYAML sees this step's keys as `["env", "name", "run",
///      "shell"]` with `shell: "cat {{0}}"`, the real GitHub runner would
///      accept that override, and the gate would `cat` its own decision
///      script instead of running it — while this suite stayed 16/16
///      green throughout, because every extractor's line splitting hid
///      the smuggled key from view.
///
/// THIS CLOSES THE CLASS BY CONSTRUCTION, NOT BY ENUMERATION (PR #671
/// review round 14, reviewer-confirmed framing): this test asserts the
/// raw bytes of `ci.yml` contain none of the four characters above.
/// Unlike most checks in this file, it needs no position assumption (it
/// scans the WHOLE file, not a line at an expected indent), no presence
/// assumption (it does not first have to LOCATE a target key before
/// checking it), and its extractor is trivial (`char_indices()` over the
/// raw bytes has nothing to silently under-report) — the same shape as
/// M2-j (`!contains("continue-on-error")`), the one check the round-13
/// corrected rule certifies as safe by construction rather than as an
/// accident of what happened to get tested. It is NOT, however, a
/// general fix for line-based extraction, and it does not make
/// `str::lines()`-based checks correct for cases beyond these four
/// specific characters — that remains a real limitation of this file's
/// design. The durable fix is parsing `ci.yml` ONCE with a real,
/// off-the-shelf YAML parser (the same category of tool `actionlint` and
/// GitHub's own `actions/runner` already use) and asserting over the
/// PARSED TREE for every structural check in this suite, keeping today's
/// byte-for-byte scalar pins (the run line, `if:` expressions,
/// `NEEDS_JSON:`) as a second assertion layered on parsed VALUES rather
/// than raw text. NAMING THE OPTION PRECISELY (round 14 correction of a
/// round-11 mischaracterization): round 11's design note rejected
/// "re-deriving YAML block-mapping semantics" as impractical, which is
/// correct — hand-rolling a general block-mapping parser was, and
/// remains, the wrong call — but the conclusion it drew from that was
/// "hand-roll a narrower slice" (the job/step key-set pins, the `if:`/
/// run-line reject-don't-parse normalizers, and now this byte scan).
/// USING A REAL PARSER was always the third option, distinct from both
/// "hand-roll a full parser" (rejected, correctly) and "hand-roll a
/// narrower slice" (what this file actually does). That rewrite is out
/// of scope for this round and is tracked as a follow-up story in that
/// specific direction — it is not implied closed by this test.
#[test]
fn test_ci_yml_contains_no_non_lf_yaml_line_breaks() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));

    const FORBIDDEN: &[(char, &str)] = &[
        ('\r', "CR (U+000D)"),
        ('\u{0085}', "NEL (U+0085)"),
        ('\u{2028}', "LINE SEPARATOR (U+2028)"),
        ('\u{2029}', "PARAGRAPH SEPARATOR (U+2029)"),
    ];

    for (byte_offset, ch) in raw.char_indices() {
        if let Some((_, label)) = FORBIDDEN.iter().find(|(forbidden, _)| *forbidden == ch) {
            panic!(
                "FAIL (PR #671 review round 14, CRITICAL): \
                 `.github/workflows/ci.yml` contains a {label} character \
                 at byte offset {byte_offset}. Every extractor in this \
                 suite splits on `str::lines()`, which recognizes ONLY \
                 `\\n` as a line break — but this character is a valid \
                 YAML line break in its own right (YAML 1.1 `b-char`; \
                 PyYAML and Ruby Psych/libyaml both honor it) that a real \
                 YAML parser treats as ending the logical line. A key \
                 placed after this character, on the same physical text \
                 line as a preceding key, is INVISIBLE to every line- \
                 based check in this file simultaneously while a real \
                 parser sees a normal, separate key — see this test's \
                 doc comment for three reproduced one-line exploits \
                 (workflow-env, workflow `defaults:`, and gate-step \
                 `shell:` smuggling). This is a byte-level tripwire, not \
                 a general fix for line-based extraction — see the doc \
                 comment for why a real YAML-parse rewrite is the \
                 durable fix, tracked as a follow-up story, not this \
                 test."
            );
        }
    }
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
// CRITICAL — behavioral closure via PINNED LITERALS (PR #671 review, round
// 6 — replaces every predicate tried in rounds 3-5).
//
// Rounds 1 and 2 (see the CRITICAL-1/round-2 section below) each guarded a
// REPRESENTATION of `ALLOWED_SKIPS`: round 1 parsed the array literal's
// source text; round 2 replaced that with a regex-narrowed occurrence count
// plus a shell-out that asked bash for the array's PRINTED value. Both were
// bypassed by ordinary bash constructions (mutations A and B — a
// subscripted assignment and a parallel array read, neither of which
// touches the guarded representations at all).
//
// Round 3 moved from representation to BEHAVIOR: run the real script
// against a synthesized payload and check its actual decision. That part
// survived every subsequent round intact. But round 3 (and its round 5
// tightening, "M1") still needed an oracle to decide, from `ci.yml`'s text
// alone, whether a job's `if:` was "the legitimate case" — and every
// predicate tried for that oracle was bypassed:
//   - Presence-only (round 3): accepted ANY `if:` line, including
//     `if: ${{ always() }}` — a genuine no-op.
//   - No-op-blacklist + event-marker-required (round 5, "M1" fix): a
//     trailing YAML comment defeated BOTH halves of the predicate at once
//     — the string no longer ends in `}}` (so the `${{ }}`-wrapped
//     `always()` literal never matched the blacklist) AND the comment
//     text itself could satisfy the "references an event/config marker"
//     rule (round 6 CRITICAL-1: `if: ${{ always() }} # not gated on
//     vars.SOMETHING` plus allowlisting `deny` passed the full suite,
//     13/13, gate rc=0).
//   - Structural gap in the SAME predicate, independent of comments (round
//     6 CRITICAL-2): `ALWAYS_TRUE_NOOPS` enumerated always-TRUE no-ops.
//     `if: ${{ github.ref == 'refs/heads/does-not-exist' }}` is an
//     always-FALSE condition — permanently `skipped`, on every event, on
//     every branch — and no blacklist of always-true literals can ever
//     reach the always-false direction. Also confirmed 13/13 green, gate
//     rc=0.
//
// CONCLUSION (round 6): "is this `if:` expression legitimate?" is
// undecidable from source text by pattern-matching — six rounds, six
// bypasses. `job_if_can_evaluate_false` (the predicate) is DELETED, not
// patched again.
//
// FIX: `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS` below replaces the question
// "does this expression MEAN something legitimate?" (undecidable) with
// "is this expression BYTE-IDENTICAL, after narrow normalization, to one a
// human has actually reviewed and approved?" (fully decidable — string
// equality). No predicate can ever answer the first question soundly for
// arbitrary GitHub Actions expression text; the second question doesn't
// need to be answered by code at all, only checked. See
// `extract_and_normalize_if_expr` and `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS`
// below, and `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`
// for how they combine with the (unchanged, round-3) real-subprocess
// behavioral check. THIS combination is the closure. The round-1/round-2
// tests further below are kept as diagnostics (faster, more specific
// messages when they DO fire) but are not, and must not be described as,
// sufficient alone.
//
// ROUND 7 ADDENDUM: the round-6 design's IMPLEMENTATION (not its core
// idea) still had gaps — a hand-rolled parser can misrepresent input even
// when the underlying "compare to a pin" design is sound. `extract_and_
// normalize_if_expr` mis-normalized a YAML block-scalar `if:` (`>-`) to
// its marker text instead of rejecting it (CRITICAL-1), and its
// first-`#`-wins comment stripper truncated a glued, non-whitespace-
// preceded `#` as if it were a real comment (CRITICAL-2) — both now hard
// `Err` rejections instead of silent mis-parses. Separately, every
// synthesized payload in this test used to carry only a `result` field;
// a mutation keying tolerance on an unrelated sibling field or on
// skip-count passed unnoticed because no payload had that field or that
// shape (CRITICAL-3) — payloads are now production-shaped and each job is
// checked in both a single-skip and a `job`+`mutants`-both-skipped
// variant. See `extract_and_normalize_if_expr`'s and
// `build_multi_skip_payload`'s doc comments for the full detail.
//
// ROUND 8 ADDENDUM: round 7's production shape was itself wrong in one
// respect — it modeled a phantom `outcome` field that the real `needs`
// context does not have (`outcome` is a `steps`-context-only property;
// verified against GitHub's contexts reference). `build_multi_skip_payload`
// now models exactly `result`+`outputs`, the two documented `needs.
// <job_id>` properties, nothing more. See that function's doc comment for
// the full correction and the reproduced inverted-mutation blind spot this
// phantom field created.
// ---------------------------------------------------------------------------

/// PINNED, human-reviewed `if:` expressions for every job legitimately
/// permitted to be a member of `scripts/check-ci-gate.sh`'s
/// `ALLOWED_SKIPS` allowlist (PR #671 review, round 6).
///
/// One entry per job. The value is the EXACT `if:` expression text (after
/// `extract_and_normalize_if_expr`'s narrow normalization — see its doc
/// comment) that a human has reviewed and approved as a condition that can
/// genuinely evaluate to both `true` and `false` depending on real,
/// externally-meaningful state (which event triggered the run, which
/// branch, a repo variable, a workflow input — not a constant). Adding a
/// job here, or changing an existing entry's text, is ITSELF the human
/// review step this design relies on — do not add or edit an entry without
/// actually reasoning about whether the expression can evaluate `false` in
/// practice, on both the always-true and always-false axes (the axis that
/// bypassed round 5's predicate).
///
/// Verified by direct read of `.github/workflows/ci.yml :: mutants` for
/// this revision (do not trust a transcription — re-verify at the time of
/// any change): the job-level `if:` is exactly `github.event_name ==
/// 'pull_request'`, with NO `${{ }}` wrapper.
///
/// `#[cfg(unix)]` (PR #671 review round 15, CI-caught): every reader of
/// this pin lives inside one of the three `#[cfg(unix)]`-gated tests —
/// see `list_all_ci_yml_job_names`'s doc comment for the full "gating a
/// test orphans its helpers" explanation.
#[cfg(unix)]
const PINNED_ALLOWED_SKIP_IF_EXPRESSIONS: &[(&str, &str)] =
    &[("mutants", "github.event_name == 'pull_request'")];

/// Find the byte index of a legitimate YAML comment start in `s`: a `#`
/// immediately preceded by whitespace (space or tab). Returns `None` if no
/// such `#` exists — including when `s` contains a `#` that is NOT
/// preceded by whitespace (a "glued" `#`, e.g. `}}#{{`), which is not a
/// comment start under YAML's own rules and must not be treated as one.
///
/// Deliberately does not special-case index 0 (the byte immediately after
/// `if:`): every caller here passes the raw text following `if:`, which
/// always starts with the separating space from `if: ...`; a `#`
/// literally glued to `if:` itself (`if:#...`, no space at all) is
/// degenerate input that this function correctly does NOT treat as a
/// comment, leaving it in the value for `extract_and_normalize_if_expr`'s
/// later "value still contains `#`" check to reject.
fn find_comment_start(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    (1..bytes.len()).find(|&i| bytes[i] == b'#' && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t'))
}

/// Extract a `ci.yml` job block's job-level `if:` expression and apply the
/// narrow normalization used for pinned-literal comparison.
///
/// Returns:
///   - `Ok(None)` if the job has no job-level `if:` at all.
///   - `Ok(Some(expr))` if it has a single-line, unambiguously-delimited
///     `if:` expression.
///   - `Err(reason)` if the `if:` value is in a YAML form this function
///     cannot FAITHFULLY represent as a single string (see REJECT,
///     DON'T PARSE below) — the caller must treat this as an immediate,
///     unconditional test failure naming the job, never as "no pin" or
///     "pin mismatch".
///
/// Job-level `if:` detection is unchanged from every prior round: a line
/// starting with exactly 4-space indent (`    if:`), not 8+ spaces (a
/// step-level `if:` inside `steps:`).
///
/// REJECT, DON'T PARSE (PR #671 review round 7, CRITICAL-1/CRITICAL-2):
/// this repo pins MSRV 1.85 and runs `cargo deny check`; adding a real
/// YAML parser dependency (e.g. `serde_yaml`, which is unmaintained) to
/// correctly handle every YAML scalar form is its own risk surface
/// requiring its own review — out of proportion for a test helper. This
/// function does not need to SUPPORT every YAML `if:` form; it needs to
/// never MISREPRESENT one. Two round-6 gaps were reproduced end-to-end
/// and are now hard rejections instead of silent mis-parses:
///   - CRITICAL-1: `if: >-` (or any block-scalar header: `>`, `|`, `>-`,
///     `|-`, `>+`, `|+`) puts the real expression on CONTINUATION lines.
///     The round-6 implementation read only the `if:` line itself and
///     normalized to the block-scalar marker text (e.g. `">-"`) — pinning
///     `("deny", ">-")` then let the continuation lines be swapped to ANY
///     expression, including a permanently-false one, with ZERO further
///     change to the pin. Now: any block-scalar header, OR a value that
///     continues onto a following line more deeply indented than the
///     `if:` line itself (plain-scalar line folding — no block marker
///     needed for YAML to fold a value across lines), is a hard `Err`.
///   - CRITICAL-2: the round-6 comment stripper split on the FIRST `#`
///     unconditionally, so `if: ${{ vars.RUN_DENY == 'true' }}#${{
///     always() }}` (no whitespace before the `#`) normalized to the
///     textbook-legitimate-looking `${{ vars.RUN_DENY == 'true' }}` —
///     exactly what a human would approve — while the REAL YAML value
///     (per YAML's own comment-start rule: a `#` starts a comment only
///     when preceded by whitespace or at line start) includes the glued
///     `#${{ always() }}` suffix. Now: `find_comment_start` only
///     recognizes a `#` preceded by whitespace as a comment start; if a
///     `#` remains in the value after removing a legitimate trailing
///     comment (i.e. an ambiguous embedded `#` this function cannot
///     safely interpret), that is also a hard `Err`.
///
/// NORMALIZATION APPLIED, for the single-line plain-scalar case that
/// remains after the rejections above (documented exhaustively — per PR
/// #671 review rounds 6-7, normalization is exactly where every bypass in
/// this design lived, so scope creep here reopens the same class of
/// hole):
///   1. Take the raw text after the `if:` key on that one line.
///   2. Strip a trailing YAML comment via `find_comment_start` (whitespace-
///      preceded `#` only — see CRITICAL-2 above).
///   3. Collapse all whitespace: split on any run of whitespace and
///      rejoin with single spaces (this also trims leading/trailing
///      whitespace, since `split_whitespace` yields only non-empty
///      tokens). Defends against a purely cosmetic reformat (e.g.
///      `if:   github.event_name  ==  'pull_request'`) failing to
///      byte-match a pin written with single spaces.
///
/// Deliberately NOT normalized: a `${{ ... }}` wrapper is left AS-IS.
/// `if: github.event_name == 'pull_request'` and `if: ${{
/// github.event_name == 'pull_request' }}` are treated as DIFFERENT
/// strings and would NOT match the same pin. Adding or removing the
/// wrapper is a real textual change to what's actually in `ci.yml`; under
/// the "byte-identical to a human-reviewed pin" design, that change
/// requires the same explicit re-review as any other textual change to
/// the expression, not silent tolerance for an equivalent-looking
/// rewrite. (This is a deliberate design choice, not an oversight: being
/// MORE literal here is strictly safer than trying to guess which textual
/// variations are "equivalent" — that guessing is exactly the kind of
/// judgment call that produced six rounds of predicate bypasses.)
fn extract_and_normalize_if_expr(job_block: &str) -> Result<Option<String>, String> {
    let lines: Vec<&str> = job_block.lines().collect();
    let is_job_level_if_line = |l: &&str| l.starts_with("    if:") && !l.starts_with("        ");

    // SUGGESTION (PR #671 review round 9): a job block with TWO job-level
    // `if:` keys is a hard `Err`, never "use the first match". Reviewed and
    // confirmed NOT independently exploitable today (GitHub Actions' YAML
    // parser, and `actionlint`, both reject a duplicate mapping key as a
    // parse error — a workflow with two `if:` keys under one job never
    // runs at all), so this is defense-in-depth, not a closed bypass: it
    // costs one comparison and removes "which `if:` wins" as a question
    // this checker would otherwise have to answer via first-match order,
    // which is exactly the kind of implicit, unreviewed tie-break rule
    // that produced earlier rounds' bypasses.
    if lines.iter().filter(|l| is_job_level_if_line(l)).count() > 1 {
        return Err(
            "has more than one job-level `if:` key in its ci.yml job block \
             — this is invalid YAML (a duplicate mapping key) that GitHub \
             Actions and actionlint both reject at parse time, but this \
             checker refuses to silently pick a winner rather than rely on \
             that external validation. Remove the duplicate `if:` key."
                .to_string(),
        );
    }

    let if_line_idx = lines.iter().position(is_job_level_if_line);

    let Some(if_line_idx) = if_line_idx else {
        return Ok(None);
    };

    let if_line = lines[if_line_idx];
    let raw = if_line.trim_start().strip_prefix("if:").unwrap_or("");
    let raw_value_leading_trimmed = raw.trim_start();

    // CRITICAL-1a: reject any YAML block-scalar header.
    if raw_value_leading_trimmed.starts_with('>') || raw_value_leading_trimmed.starts_with('|') {
        return Err(format!(
            "uses a YAML block-scalar form (\"{}\") for its `if:` value — \
             the real expression lives on continuation lines this checker \
             does not read, so it cannot be safely represented as a single \
             pinned literal. Rewrite as a single-line plain scalar to make \
             it pinnable.",
            raw_value_leading_trimmed.trim()
        ));
    }

    // CRITICAL-1b: reject a value that continues onto a following,
    // more-deeply-indented line (plain-scalar line folding — YAML permits
    // this without any block-scalar marker at all).
    //
    // IMPORTANT-4 (PR #671 review round 9): skip lines whose first
    // non-whitespace character is `#` when looking for "the next line" —
    // a full-line YAML comment does not contribute to a plain scalar's
    // folded value (PyYAML-confirmed: `if: always()` followed by an
    // indented `# comment` line on its own parses to exactly `"always()"`,
    // comment fully discarded, REGARDLESS of the comment line's indent).
    // Before this fix, a genuinely single-line, pin-matching `if:` such as
    // `mutants`' followed by an ordinary trailing comment line (e.g.
    // `      # NOTE: PR-only by design; see the comment above.`) hard-failed
    // this function with "appears to continue onto a following line" —
    // advice this specific case cannot act on, since the value already IS
    // a single-line plain scalar. That false-red is exactly the kind of
    // loosening pressure that produced rounds 3-6: a maintainer's cheapest
    // fix for a false alarm is to weaken the check, not to narrow it
    // correctly. A line that is NOT a full-line comment (i.e. any text
    // before its own `#`, or no `#` at all) is still treated as a
    // candidate continuation exactly as before — this only excludes lines
    // that are comments in their entirety.
    if let Some(next_line) = lines[if_line_idx + 1..].iter().find(|l| {
        let trimmed = l.trim_start();
        !trimmed.is_empty() && !trimmed.starts_with('#')
    }) {
        let indent = next_line.len() - next_line.trim_start().len();
        if indent > 4 {
            return Err(format!(
                "has an `if:` value that appears to continue onto a \
                 following line (\"{}\", indented {indent} spaces) — this \
                 cannot be safely represented as a single pinned literal. \
                 Rewrite as a single-line plain scalar to make it pinnable.",
                next_line.trim()
            ));
        }
    }

    // CRITICAL-2: strip a trailing comment via the whitespace-preceded-`#`
    // rule only; reject if a `#` remains anywhere in the value afterward
    // (an embedded `#` this function cannot safely interpret).
    let value = match find_comment_start(raw) {
        Some(idx) => &raw[..idx],
        None => raw,
    };
    if value.contains('#') {
        return Err(format!(
            "has an `if:` value containing a `#` that is not a clearly \
             whitespace-delimited trailing comment (\"{}\") — this cannot \
             be safely normalized. Rewrite without an embedded `#` to make \
             it pinnable.",
            raw.trim()
        ));
    }

    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");

    if collapsed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(collapsed))
    }
}

/// PINNED, human-reviewed exact text of the `ci-gate` job's gate-decision
/// `run:` line (PR #671 review round 10, CRITICAL).
///
/// Every prior round's CRITICAL made the gate tolerate an illegitimate
/// SKIP. This one is a different shape entirely: nothing in this suite (or
/// in `test_ci_gate_pass_fail_semantics_are_structurally_placed`'s M2-g/h
/// assertions, which check only that a `run:` step exists and mentions
/// `check-ci-gate.sh` + `toJSON(needs)` as SUBSTRINGS) verifies that the
/// script's exit code actually reaches the JOB's conclusion. Reproduced,
/// each independently leaving the full suite at 14/14 green: appending
/// `|| true` to the run line, `continue-on-error: true` on the step,
/// `continue-on-error: true` on the job, and piping through `| cat`. Every
/// one of these makes the gate tolerate EVERY upstream failure, not just
/// an illegitimate skip — the retired inline condition (an allowlist of
/// known-bad `contains()` values) and this gap are the same shape one
/// layer up: an allowlist of known-good SUBSTRINGS with no default-deny on
/// the run line itself.
///
/// Verified by direct read of `.github/workflows/ci.yml :: ci-gate` for
/// this revision (do not trust a transcription — re-verify at the time of
/// any change): the step-level `run:` is exactly `echo "${NEEDS_JSON}" |
/// bash scripts/check-ci-gate.sh`, with NO trailing `|| true`, `| cat`,
/// `; exit 0`, or similar, and NO `continue-on-error` anywhere in the job
/// block.
const PINNED_GATE_RUN_LINE: &str = "echo \"${NEEDS_JSON}\" | bash scripts/check-ci-gate.sh";

/// Extract and normalize the SOLE step-level `run:` line in a job block,
/// for pinned-literal comparison against `PINNED_GATE_RUN_LINE`.
///
/// Deliberately a SEPARATE function from `extract_and_normalize_if_expr`
/// rather than a generalization of it: that function has been the site of
/// every bypass in rounds 1-9 of this story's review, and refactoring it
/// under this round's time pressure to serve a second caller is exactly
/// the kind of change that reopens a closed class of bug in the one place
/// most likely to hide it. The normalization RULES are intentionally the
/// same (reject-don't-parse: block-scalar headers, line continuations, and
/// ambiguous embedded `#` are all hard errors; a legitimate trailing
/// comment is stripped; internal whitespace runs collapse to one space) —
/// only the indent depth (8, for a step-level key, vs. 4 for a job-level
/// `if:`) and the key name differ.
///
/// Returns `Ok(String)` only for a single, unambiguous, single-line
/// `run:` value. Every other case — no `run:` line at all, more than one
/// step-level `run:` line (this checker refuses to guess which one is the
/// gate decision), a block-scalar form, a folded continuation, or an
/// unresolvable embedded `#` — is `Err(reason)`, which the caller must
/// treat as an immediate, unconditional test failure, never as "no pin"
/// (there is only one pinned line here, not a per-job map).
fn extract_and_normalize_sole_run_line(job_block: &str) -> Result<String, String> {
    let lines: Vec<&str> = job_block.lines().collect();
    // PR #671 review round 11, small correction 1: the previous form here
    // (`l.starts_with("        run:") && !l.starts_with("          ")`) had
    // the same dead conjunct just removed from `line_declares_job_level_key`
    // — a line starting with 8 spaces then "run:" can never ALSO start with
    // 10 spaces (character 9 is 'r', not a space), so the second clause was
    // unreachable-false protection that provided nothing. This version's
    // "is the 9th character a space" check actually rejects a 9+-space
    // indent.
    let is_step_level_run_line = |l: &&str| {
        let Some(after_indent) = l.strip_prefix("        ") else {
            return false;
        };
        !after_indent.starts_with(' ') && after_indent.starts_with("run:")
    };

    let run_line_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| is_step_level_run_line(l))
        .map(|(i, _)| i)
        .collect();

    if run_line_indices.is_empty() {
        return Err(
            "has no step-level `run:` line matching this check's expected \
             8-space indent — the gate must execute something that can \
             fail; without one, the job trivially succeeds for every \
             upstream result. NOTE (PR #671 review round 13, benign-false- \
             red message fix, same class as M2-g's above; CORRECTED round \
             14 — the round-13 wording pointed to a check that does not \
             actually help, see below): if a `run:` line genuinely exists \
             but at an unexpected POSITION (e.g. a legal re-indent of the \
             whole step's child block), this check cannot tell \"missing\" \
             apart from \"moved\" — `run:` is not necessarily missing, it \
             may simply be in an unrecognized POSITION. This step's \
             structure CANNOT be independently confirmed by another check \
             in this suite in that case: every assertion here, including \
             M2-l's step key SETS, shares this same exact-indent \
             assumption (M2-l hardcodes 6-space step markers and 6/8-space \
             child keys) — verified empirically: under the exact \
             re-indent this note describes, `extract_gate_step_key_sets` \
             returns an empty Vec (not the pinned three-step shape), and \
             M2-i's own panic ends the test function before M2-l's \
             assertion is ever reached, so M2-l reports nothing either \
             way. The correct maintainer action under this diagnosis is \
             to RESTORE the file's 6/8/10-space indent convention, not to \
             loosen this or any other pin."
                .to_string(),
        );
    }
    if run_line_indices.len() > 1 {
        return Err(format!(
            "has {} step-level `run:` lines — this checker requires exactly \
             one so a single pinned literal unambiguously covers the gate \
             decision. Disambiguate (or, if a second `run:` step is a \
             deliberate, reviewed addition, update this checker to identify \
             the gate-decision step specifically).",
            run_line_indices.len()
        ));
    }
    let run_line_idx = run_line_indices[0];

    let run_line = lines[run_line_idx];
    let raw = run_line.trim_start().strip_prefix("run:").unwrap_or("");
    let raw_value_leading_trimmed = raw.trim_start();

    if raw_value_leading_trimmed.starts_with('>') || raw_value_leading_trimmed.starts_with('|') {
        return Err(format!(
            "uses a YAML block-scalar form (\"{}\") for its `run:` value — \
             the real command(s) live on continuation lines this checker \
             does not read, so it cannot be safely represented as a single \
             pinned literal. Rewrite as a single-line plain scalar to make \
             it pinnable.",
            raw_value_leading_trimmed.trim()
        ));
    }

    if let Some(next_line) = lines[run_line_idx + 1..].iter().find(|l| {
        let trimmed = l.trim_start();
        !trimmed.is_empty() && !trimmed.starts_with('#')
    }) {
        let indent = next_line.len() - next_line.trim_start().len();
        if indent > 8 {
            return Err(format!(
                "has a `run:` value that appears to continue onto a \
                 following line (\"{}\", indented {indent} spaces) — this \
                 cannot be safely represented as a single pinned literal. \
                 Rewrite as a single-line plain scalar to make it pinnable.",
                next_line.trim()
            ));
        }
    }

    let value = match find_comment_start(raw) {
        Some(idx) => &raw[..idx],
        None => raw,
    };
    if value.contains('#') {
        return Err(format!(
            "has a `run:` value containing a `#` that is not a clearly \
             whitespace-delimited trailing comment (\"{}\") — this cannot \
             be safely normalized. Rewrite without an embedded `#` to make \
             it pinnable.",
            raw.trim()
        ));
    }

    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");

    if collapsed.is_empty() {
        return Err("has an empty `run:` value.".to_string());
    }

    Ok(collapsed)
}

/// PR #671 review round 11 — CRITICALs 1-3. Round 10's M2-i pinned the
/// gate's `run:` line's own VALUE, but nothing pinned the surrounding
/// KEYS: the payload SOURCE (`env: NEEDS_JSON: ...`), the job's own
/// `if:`, or the complete key set of the job and its steps (which is
/// where an unguarded `shell:` or a future `continue-on-error` spelling
/// would land). Review 9's diagnosis: this was the same failure mode as
/// the retired inline `contains()` condition, one layer up — enumerating
/// members of an open set one round at a time. This section closes the
/// class via the "minimum equivalent" to a full-block pin (a full
/// byte-for-byte multi-line block pin was assessed and rejected as
/// impractical: the existing reject-don't-parse machinery is built for
/// ONE scalar value per call, and generalizing it to an entire nested
/// block would mean re-deriving YAML block-mapping semantics, which is
/// exactly the parser this design has twice now avoided writing):
///   1. the job's COMPLETE key set (`PINNED_GATE_JOB_KEYS`) — an added
///      job-level key (a second `if:`, `continue-on-error`, a future
///      `outputs:`, anything not yet imagined) changes this set.
///   2. EVERY step's COMPLETE key set, in order (`PINNED_GATE_STEP_KEY_SETS`)
///      — an added step-level key (`shell:`, `continue-on-error:`,
///      `working-directory:`) on ANY step changes its entry; an added,
///      removed, or reordered step changes the list's length or content.
///   3. the job's `if:` expression, via the EXISTING
///      `extract_and_normalize_if_expr` (previously applied only to
///      `ci-gate.needs` members' `if:` lines for the `ALLOWED_SKIPS` pin —
///      never to `ci-gate`'s OWN `if:`) against a new `PINNED_GATE_IF_EXPR`.
///   4. the gate step's `env:` child line that supplies
///      `check-ci-gate.sh`'s entire input, byte-for-byte
///      (`PINNED_GATE_NEEDS_JSON_LINE`) — reproduced end-to-end (round 11):
///      replacing `${{ toJSON(needs) }}` with a fabricated
///      all-`success` literal left the run line, the step's key set, and
///      the job's key set ALL untouched, and piping that literal into the
///      real script yields exit 0 unconditionally — worse than round 10's
///      `|| true`, because the script's own log then reads as a
///      legitimate all-green run instead of a suppressed real failure.
///
/// PR #671 review round 12 added:
///   5. the gate step's `env:` block's own COMPLETE key set
///      (`PINNED_GATE_ENV_KEYS`, M2-o) — item 4 above pins `NEEDS_JSON`'s
///      VALUE but never asserted it was the ONLY env child; a sibling
///      `BASH_ENV` is independently exploitable (non-interactive bash
///      sources it before running anything, `--norc`/`--noprofile`
///      notwithstanding — verified under the runner's actual invocation
///      flags).
///   6. the WORKFLOW's own top-level `env:` block's key set
///      (`PINNED_WORKFLOW_ENV_KEYS`,
///      `test_ci_yml_workflow_level_env_key_set_is_pinned`) — the same
///      `BASH_ENV` vector, one level further out, outside every job block
///      entirely (same structural reason as `defaults:` below).
///   7. fixed CRITICAL 1: `test_ci_yml_has_no_workflow_level_shell_override`
///      originally checked `ci.lines().any(|l| l == "defaults:")` — exact
///      LINE equality, not key detection, defeated by flow style, a
///      quoted key, a trailing comment, or a trailing space (round 10's
///      `outputs:` finding, one level up). Fixed to reuse
///      `extract_key_name_at_indent(l, 0)`.
///
/// PRECISE SCOPE (PR #671 review round 12, IMPORTANT — the "minimum
/// equivalent to a full-block pin" framing above, and this suite's
/// panic text, previously read as though the whole block were closed;
/// it is not, and was never claimed to be at the per-assertion level —
/// only the summary framing overclaimed):
///   PINNED (an unreviewed change WILL be caught): job-level key NAMES
///   (item 1); step-level key NAMES on every step, in order (item 2);
///   `ci-gate`'s own `if:` VALUE (item 3); the `NEEDS_JSON:` line's VALUE
///   (item 4); the gate step's `env:` key NAMES (item 5); the workflow's
///   top-level `env:` key NAMES (item 6); the workflow's top-level
///   `defaults:` key PRESENCE (item 7); `continue-on-error` PRESENCE
///   anywhere in the job block (M2-j).
///   NOT PINNED (an unreviewed change will NOT be caught by this file):
///   `uses:` VALUES (e.g. swapping the checkout action for a malicious
///   fork at the same key), `with:` block CONTENTS (the `harden-runner`
///   step's `egress-policy` value), and `name:` VALUES on any step or the
///   job itself.
///
///   CORRECTED (PR #671 review round 13, IMPORTANT 3): the round-12
///   version of this note justified leaving `uses:` unpinned by calling
///   it out-of-scope as "supply-chain pinning of actions used ELSEWHERE
///   in the file" — then gave, as its own example, swapping the
///   `checkout` action, which is a step INSIDE `ci-gate`, not elsewhere.
///   That was self-contradictory: both `uses:` steps in `ci-gate` run
///   BEFORE the gate step, IN THE SAME JOB, and are squarely ON the
///   pass/fail decision path this story is about — they are NOT out of
///   scope by being elsewhere, they are IN scope and KNOWINGLY unpinned.
///   The real reason to leave them unpinned is narrower: this story is
///   about the pass/fail DECISION mechanism (the run line, the payload,
///   the job's own `if:`), not about supply-chain-pinning every action
///   reference in the file — a general concern applicable to the OTHER
///   ~10 jobs in `ci.yml` too, not specific to `ci-gate`, and out of
///   THIS story's scope for that reason, not because these two `uses:`
///   sit somewhere safe.
///
///   DOCUMENTED, NOT EXECUTED (PR #671 review round 13): a step CAN write
///   `key=value` to the file at `$GITHUB_ENV` (an officially documented
///   GitHub Actions mechanism — not this story's own finding) to set an
///   env var for every SUBSEQUENT step in the same job, including the
///   gate step. A step added via `uses:` that does this would reproduce
///   the `BASH_ENV` CRITICAL (M2-o above) WITHOUT ever touching the gate
///   step's own `env:` block at all — modifying an EXISTING step's
///   `uses:` value is exactly the "NOT PINNED" gap this note names; ADDING
///   a new `run:` step to do the same thing is already caught by the
///   step-key-set arity check (M2-l), so `uses:` is the one surviving
///   route for this specific mechanism. This paragraph documents that the
///   mechanism exists and is reachable through the one channel this file
///   does not pin — it has not been executed against a real runner to
///   confirm the round-trip end-to-end, so it is recorded as a known,
///   reasoned risk, not a proven exploit.
///
/// RESIDUAL RISK — LEXER CLASS, DOCUMENTED NOT FIXED (PR #671 review round
/// 16; human decision: merge with this risk documented rather than fixed
/// here — a follow-up story to replace line-based extraction with a real
/// YAML parser is tracked separately and is NOT opened by this round):
///
///   SCOPE SUMMARY (read this before the detail below):
///   Enforced: decision semantics (behavioral subprocess tests) · assertion
///   shapes (set-equality, default-deny given faithful extraction) · value
///   substitution (byte pins that fail loud via `Err`) · non-LF line
///   breaks (complete for that one class). NOT enforced: the lexer layer
///   generally — YAML node properties (`&anchor`, `!tag`/`!!tag`) defeat
///   every set-equality pin with one line of plain ASCII; `uses:` values
///   on the decision path; human judgment on pin updates. Control for the
///   unenforced: code review of the `ci.yml` diff — no automated check
///   here catches it, acceptable because the residual requires
///   hand-crafted YAML visible in a diff, unlike the original defect,
///   which was silent. Durable fix: a real YAML parser over the parsed
///   tree (follow-up story, not opened here).
///
///   ENFORCED (verified by attack against this file's own tests, not by
///   reading an assertion and trusting its shape):
///   - `scripts/check-ci-gate.sh`'s DECISION semantics are tested
///     BEHAVIORALLY, as a real subprocess consuming a real JSON payload —
///     there is no line-based extraction layer in that path to under-
///     report anything, which is why the entire class of findings from
///     rounds 1-9 (illegitimate skips, `if:` expression bypasses) stopped
///     recurring once that redesign landed.
///   - This file's SET-EQUALITY / default-deny assertion SHAPES (job/step
///     key sets, `if:` value pins, env key sets) are sound GIVEN faithful
///     extraction — every fabricated bypass found so far (rounds 10-15)
///     was a bug in what fed the assertion, not in the assertion's own
///     logic.
///   - VALUE-substitution pins (the run line, `NEEDS_JSON:`, `if:`
///     expressions) use a reject-don't-parse design that returns an `Err`
///     — a LOUD, visible test failure — on anything it cannot confidently
///     normalize, rather than silently passing an under-reported value.
///   - The LINE-BREAK class specifically (round 14) is CLOSED, exhaustively:
///     the four-character guard
///     (`test_ci_yml_contains_no_non_lf_yaml_line_breaks`) covers every
///     character YAML 1.1 §4.1.4's `b-char` production recognizes as a
///     line break (`\n`, CR, NEL, LINE SEPARATOR, PARAGRAPH SEPARATOR),
///     cross-checked against both PyYAML's and Ruby Psych's independent
///     implementations. Spot-checked directly (round 16, not merely taken
///     on report): VT (U+000B), FF (U+000C), NBSP (U+00A0), and ZWSP
///     (U+200B) are each confirmed NOT to act as a YAML line break in
///     PyYAML (each raises `ReaderError`/`ScannerError` rather than being
///     treated as a break). This claim is about LINE BREAKS specifically,
///     not about every way YAML can disagree with `str::lines()` — see
///     the next item.
///
///   NOT ENFORCED — say this plainly, not "mitigated":
///   - This file's line-based LEXER layer (`extract_key_name_at_indent`
///     and everything built on it) under-reports on YAML that a real
///     parser reads differently than a naive line-by-line scan. The
///     line-break class (round 14) was ONE member; a SECOND, DISTINCT
///     member exists and is NOT fixed by the line-break guard: YAML NODE
///     PROPERTIES (anchors `&name`, tags `!tag`/`!!tag`) prefixing a
///     mapping key on the SAME physical line, with ZERO line breaks
///     involved (orthogonal to round 14's fix). Both `&x shell: cat {0}`
///     and `!!str shell: cat {0}`, inserted as a new line in the gate
///     step, are read by BOTH PyYAML and Ruby Psych as adding a real
///     `shell:` key to that step (`['env','name','run','shell']`,
///     `shell: 'cat {0}'` — a real GitHub Actions custom-shell-template
///     override, the exact mechanism rounds 11/14 already showed defeats
///     the pinned run line) while `extract_key_name_at_indent`'s bare-key
///     branch — which takes `key_end` at the first colon-or-whitespace —
///     stops at the space after `&x`/`!!str`, sees text not starting with
///     `:`, and returns `None`: the key is invisible to EVERY pin built on
///     that one function, `PINNED_GATE_STEP_KEY_SETS` included. Reproduced
///     and verified independently (round 16, not merely reported): a
///     one-line insertion, full 17/17 suite green with the malicious line
///     present. `!!str shell: cat {0}` is `actionlint`-clean (exit 0); the
///     `&x shell: cat {0}` form makes `actionlint` warn (unused-anchor,
///     exit 1) but is accepted identically by both YAML loaders, so
///     `actionlint` is not a backstop for that spelling either.
///   - The underlying MECHANISM (YAML anchors/aliases in GitHub Actions
///     workflows) is not speculative or merely "requested" — independently
///     verified (round 16) that GitHub shipped it in production on
///     2025-09-18 ("Actions: YAML anchors and non-public workflow
///     templates", the official github.blog changelog), after four years
///     tracked as `actions/runner#1182` (1,650+ reactions), switching the
///     runner to a YAML-1.2.2-conformant parser specifically for "better
///     conformance with the YAML specification"; `github/community`
///     discussion #185877 is the follow-on request for merge-key (`<<:`)
///     support, which GitHub has NOT shipped — confirming anchors
///     specifically (not merge keys) are the shipped, live mechanism. This
///     is CONFIRMED SHIPPED, not a documented-but-hypothetical request.
///     What remains UNVERIFIED is the exact end-to-end behavior of this
///     repo's specific `&x shell:` / `!!str shell:` payloads against a
///     live GitHub Actions runner — that has not been executed against
///     real GitHub Actions infrastructure by anyone on this story, only
///     against PyYAML/Ruby Psych locally.
///   - This is the THIRD consecutive round to find a new member of the
///     "lexer disagrees with a real YAML parser" class after the previous
///     member was patched: round 13 found comment-indentation truncation,
///     a UTF-8 BOM, and explicit-key (`? key`) syntax; round 14 found lone
///     CR and the three multi-byte Unicode line breaks; this round found
///     node properties. The finding RATE across these three rounds is
///     FLAT, not decreasing — each fix closed exactly the member it
///     targeted and no more. The line-break guard's "closed by
///     construction" property (no position/presence assumption, and an
///     exhaustively-verified character set) is true, and true ONLY, of
///     that one member — it does not extend to, and must not be read as
///     covering, the lexer layer generally. Nothing in this file currently
///     closes the lexer layer generally.
///   - `uses:` VALUES on the decision path, and human judgment on
///     `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS`/other pin updates, remain
///     unenforced for the reasons already recorded above (rounds 12-13) —
///     unchanged by this round.
///
///   NO AUTOMATED CHECK IN THIS REPOSITORY CATCHES A NODE-PROPERTY BYPASS.
///   CODE REVIEW IS THE CONTROL — not "mitigated," UNGUARDED, with review
///   as the control. A hostile or careless one-line YAML edit using node
///   properties (or a future, still-undiscovered member of this same
///   lexer-disagreement class) requires a human to notice hand-crafted
///   YAML syntax (an anchor or a tag on a key that has no ordinary reason
///   to carry one) in a diff. That is acceptable for a specific reason,
///   not merely tolerated: this residual requires HAND-CRAFTED YAML
///   visible in a diff (`&x shell: cat {0}` / `!!str shell: cat {0}` is
///   not a construction a careless edit, `sed`, or auto-formatter
///   produces) — unlike the pre-S-CIGATE-2 defect this whole story exists
///   to fix, which was silent and already live in production on every
///   push before anyone noticed. This is the ambient control this
///   repository already relies on for everything this file does not pin
///   (see "NOT PINNED" above), stated plainly rather than implied.
///
///   THE DURABLE FIX (tracked as a follow-up story, NOT opened by this
///   round): parse `ci.yml` ONCE with a real YAML parser — an off-the-
///   shelf Rust crate (`saphyr`/`yaml-rust2`) or a PyYAML shell-out
///   matching `scripts/check-ci-gate.sh`'s own precedent — and assert over
///   the PARSED TREE for every structural check in this suite, keeping
///   today's byte-for-byte scalar pins as a SECOND assertion layered on
///   parsed VALUES rather than raw text. Round 11's design note correctly
///   rejected hand-rolling a general block-mapping parser as impractical,
///   but its own conclusion ("hand-roll a narrower slice" — the job/step
///   key-set pins, the `if:`/run-line reject-don't-parse normalizers, and
///   the round-14 byte scan) conflated that correct rejection with a
///   DIFFERENT, un-rejected option: using an OFF-THE-SHELF parser, the
///   same category of tool `actionlint` and GitHub's own `actions/runner`
///   already use (and, as of 2025-09-18, GitHub's own parser now also
///   understands the anchors that defeat this file's lexer). Record this
///   as the follow-up's specific direction so a future reader does not
///   re-litigate the same two choices rounds 11-16 already worked through.
///
/// CORRECTED (PR #671 review round 12; RULE ITSELF CORRECTED round 13 —
/// see below): an earlier limitation note here speculated that every
/// hardcoded-indent check (including all of the above) is vulnerable to
/// re-indenting `ci-gate`'s own block, since a job's children need only
/// be indented consistently with each other, not with the file's
/// convention (PyYAML-confirmed, round 11). Round 12 attacked this
/// directly: re-indenting the whole child block +2, alone and combined
/// with an always-false `if:` or a `shell:` override, failed CLOSED every
/// time — on M2-a, which asserts PRESENCE of `    if:` at the exact
/// indent this suite expects; moving the indent makes that presence
/// check itself fail, before any deeper pin is even reached.
///
/// RULE, CORRECTED (PR #671 review round 13, IMPORTANT 1): round 12
/// recorded the general rule as "presence-shaped checks fail closed
/// under position drift; absence-shaped checks with a position
/// assumption fail open." That rule is WRONG, and round 13's own
/// CRITICAL is the counterexample: M2-o and the workflow-env test are
/// SET-EQUALITY checks — neither presence- nor absence-shaped — which
/// the recorded rule implied were safe. Both failed OPEN (a YAML comment
/// caused `extract_gate_env_key_set`/`extract_workflow_env_key_set` to
/// silently truncate their own extracted set, so the truncated set still
/// matched the pin). As written, the rule would have cleared the exact
/// bug it was recorded alongside. The property that actually predicts
/// failure is the EXTRACTOR's, not the assertion's: **any check whose
/// extractor can silently under-report its input fails open, regardless
/// of whether the assertion built on top of it is presence, absence, or
/// set-equality.** The assertion is only as strong as the extraction
/// feeding it. M2-a (round 12's negative result) survives under this
/// corrected rule for a different reason than originally stated: its
/// "extractor" is trivial (a single `starts_with` check with nothing to
/// under-report), so there is no silent-truncation failure mode for it
/// to have. M2-j (`!contains("continue-on-error")`) similarly has a
/// trivial, whole-block extractor (nothing to anchor or terminate on)
/// hence nothing to under-report. The pre-round-12 `defaults:` check DID
/// have a narrow, exact-line extractor that could silently miss a
/// legitimate spelling — consistent with the corrected rule, not the
/// original one, which happened to describe the same failure by
/// coincidence of that check ALSO being position-shaped.
///
/// VERIFIED CLOSED (round-12 follow-up; previously recorded here as
/// untested): attacked directly, specifically targeting the PINNED items
/// (1-6) rather than M2-a's `if:` presence check. First construction
/// tried (`if:` left at 4-space, `steps:` moved to 6-space) turned out to
/// be INVALID YAML (PyYAML: "mapping values are not allowed here") — a
/// job's sibling keys must share one indent, so `steps:` cannot move
/// without moving `if:`/`name:`/`runs-on:` with it; that construction was
/// never a real bypass to begin with. The VALID form of "only the deeper
/// structure moves": job-level keys (`if:`, `steps:`, etc.) stay at their
/// original 4-space indent, but `steps:`'s LIST ITEMS and everything
/// under them shift deeper (6→8 for step markers, 8/10→10/12 for their
/// children) — PyYAML-confirmed this parses to the semantically identical
/// structure. Reproduced: this fails closed too, but via a DIFFERENT
/// mechanism than M2-a — `extract_and_normalize_sole_run_line`'s
/// step-level-run-line search itself has a position assumption (exactly
/// 8-space indent) baked into its "find the candidate line" step, before
/// it ever gets to comparing a VALUE; shifting the run line to 10-space
/// makes the search find zero candidates, which is itself a presence
/// check ("has no step-level `run:` line at all") that fires (M2-i) ahead
/// of any of the key-set/if:/env: pins below. Under the CORRECTED rule
/// above: this search's "find the candidate" step is itself an extractor
/// that can under-report (find zero candidates) — and unlike the
/// round-13 CRITICAL's comment-truncation bug, an under-report here
/// produces an explicit `Err` (a hard, loud failure) rather than a
/// silently-shrunken `Vec`, which is exactly the difference between an
/// extractor design that fails safe and one that doesn't. No open
/// question remains on re-indentation; both the whole-block and
/// deeper-structure-only constructions are now verified closed.
const PINNED_GATE_IF_EXPR: &str = "${{ always() }}";
const PINNED_GATE_NEEDS_JSON_LINE: &str = "${{ toJSON(needs) }}";
const PINNED_GATE_JOB_KEYS: &[&str] = &["if", "name", "needs", "runs-on", "steps"];
const PINNED_GATE_STEP_KEY_SETS: &[&[&str]] = &[
    &["name", "uses", "with"],
    &["uses"],
    &["env", "name", "run"],
];

/// PR #671 review round 12, CRITICAL 2: `PINNED_GATE_STEP_KEY_SETS` above
/// confirms `env:` exists as a KEY on the gate step — it says nothing
/// about which children `env:` itself has. `env:`'s children (10-space
/// indent, one level deeper than the step's own 8-space keys) are an
/// unpinned open set under a pinned key: M2-n pins the ONE `NEEDS_JSON:`
/// line's value but never asserted it is the ONLY env child. Reproduced:
/// adding a sibling `BASH_ENV: .github/ci-shim.sh` alongside the pinned
/// `NEEDS_JSON:` line left every existing pin satisfied (the run line,
/// the job/step key sets, `ci-gate`'s own `if:`, and `NEEDS_JSON`'s value
/// are all untouched) — 15/15 green — while `BASH_ENV` is genuinely
/// exploitable: verified locally under BOTH shells this gate step could
/// actually run under — CORRECTED (PR #671 review round 13): the gate
/// step sets no `shell:` of its own, so GitHub's real default here is
/// `bash -e {0}` (not the longer `bash --noprofile --norc -e -o
/// pipefail`, which is what you get only with an EXPLICIT `shell: bash`
/// — the round-12 original cited the wrong one) — `BASH_ENV` is sourced
/// by non-interactive bash regardless of which of these two forms is in
/// play, so a shim script ending in `exit 0` ends the shell before the
/// pinned run line's script body ever executes either way.
const PINNED_GATE_ENV_KEYS: &[&str] = &["NEEDS_JSON"];

/// PR #671 review round 12, CRITICAL 2 (workflow-level half): the
/// WORKFLOW's own top-level `env:` key is at 0-space indent (a sibling
/// of `name:`/`on:`/`jobs:`/`defaults:`); its VALUE (2-space indent —
/// corrected, PR #671 review round 13: the round-12 original said "2-space
/// indent, a sibling of ..." — the KEY is at indent 0, its CHILDREN at
/// indent 2; the code was always right, only this doc comment mislabeled
/// which indent belongs to which) is currently just
/// `CARGO_TERM_COLOR: always`, and is the second member of the same open
/// set. `BASH_ENV` appended there is invisible to every job-block-
/// anchored check for the same structural reason `defaults:` was
/// (`test_ci_yml_has_no_workflow_level_shell_override`'s doc comment) —
/// it lives outside every job block by construction — and is
/// independently exploitable via the same non-interactive-bash
/// `BASH_ENV`-sourcing mechanism, this time affecting every job in the
/// file rather than one gate step.
const PINNED_WORKFLOW_ENV_KEYS: &[&str] = &["CARGO_TERM_COLOR"];

/// Collect the sorted key set of a mapping block whose children begin at
/// `child_indent` spaces, starting the scan at `lines[start..]`.
///
/// PR #671 review round 13, CRITICAL fix: the round-12 originals here
/// used `take_while(|l| trimmed.is_empty() || l.starts_with(indent))` —
/// which TERMINATES (not skips) on the first line that is neither blank
/// nor at the exact child indent. A YAML COMMENT is such a line, at ANY
/// indentation (YAML ignores comment indentation entirely — confirmed
/// via PyYAML), so a comment inserted between `NEEDS_JSON:` and a
/// smuggled `BASH_ENV:` sibling stopped the scan at the comment, leaving
/// `BASH_ENV:` (and anything after it) invisible: the extracted set
/// still equalled the pin, 16/16 green, while PyYAML confirmed the var
/// was genuinely set. Fixed by treating a comment line as something to
/// SKIP OVER, never something that ends the block — real YAML semantics,
/// which don't recognize "comment indentation" as meaningful at all. The
/// block ends only at a real (non-blank, non-comment) line that is NOT
/// indented at least `child_indent` spaces — i.e. back at or above the
/// mapping key's OWN level.
fn collect_mapping_key_set(lines: &[&str], start: usize, child_indent: usize) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for l in &lines[start..] {
        let trimmed = l.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let actual_indent = l.len() - trimmed.len();
        if actual_indent < child_indent {
            break;
        }
        if let Some(k) = extract_key_name_at_indent(l, child_indent) {
            keys.push(k);
        }
    }
    keys.sort();
    keys
}

/// Extract the sorted, complete key set of the gate step's `env:` block
/// (10-space indent — one level deeper than the step's own 8-space keys)
/// for comparison against `PINNED_GATE_ENV_KEYS`.
///
/// PR #671 review round 13, IMPORTANT 2: anchors to the `env:` line
/// belonging to the STEP THAT DECLARES THE PINNED `run:` line, not simply
/// the first indent-8 `env:` anywhere in `job_block` (the round-12
/// original). Those are the same thing TODAY only because
/// `PINNED_GATE_STEP_KEY_SETS` forbids an earlier step from having its
/// own `env:` — but a legitimate, reviewed future change (adding `env:`
/// to the harden-runner or checkout step, updating that pin in the same
/// commit, exactly as every panic message here instructs) would silently
/// repoint this function at the WRONG step's `env:`, reopening the gate
/// step's `env:` as an unpinned set again with the whole suite green
/// throughout. Anchoring to the step carrying `run:` ties this
/// extraction to the step whose behavior actually matters, not to
/// positional luck.
fn extract_gate_env_key_set(job_block: &str) -> Vec<String> {
    let lines: Vec<&str> = job_block.lines().collect();
    let Some(run_line_idx) = lines
        .iter()
        .position(|l| extract_key_name_at_indent(l, 8).as_deref() == Some("run"))
    else {
        return Vec::new();
    };

    // PR #671 review round 14, SUGGESTION (doc/code mismatch fixed): this
    // scans BACKWARD from `run:` only, on the assumption (ci.yml's current
    // convention, not a YAML requirement) that the step's `env:` line
    // precedes its `run:` line. A prior version of this comment claimed
    // "YAML mapping keys have no required order, so scan the whole step
    // rather than assume this" — the code never did that; it only ever
    // scanned backward. Reordering `env:` to AFTER `run:` on the gate step
    // is a legal, semantically identical edit (YAML mapping keys are
    // genuinely unordered) that this function cannot see — verified
    // fail-CLOSED, not open: it fires M2-o's non-empty assertion below,
    // since the backward scan then finds no indent-8 `env:` before hitting
    // the step's own `- ` marker.
    let Some(env_line_idx) = lines[..run_line_idx]
        .iter()
        .enumerate()
        .rev()
        .take_while(|(_, l)| !l.starts_with("      -"))
        .find(|(_, l)| extract_key_name_at_indent(l, 8).as_deref() == Some("env"))
        .map(|(i, _)| i)
    else {
        return Vec::new();
    };

    collect_mapping_key_set(&lines, env_line_idx + 1, 10)
}

/// Extract the sorted, complete key set of the WORKFLOW's own top-level
/// `env:` block (2-space indent) for comparison against
/// `PINNED_WORKFLOW_ENV_KEYS`. Reads `ci` (the whole file), not a
/// `job_block` — see `PINNED_WORKFLOW_ENV_KEYS`'s doc comment for why
/// this construct is outside every job block by construction.
fn extract_workflow_env_key_set(ci: &str) -> Vec<String> {
    let lines: Vec<&str> = ci.lines().collect();
    let Some(env_line_idx) = lines
        .iter()
        .position(|l| extract_key_name_at_indent(l, 0).as_deref() == Some("env"))
    else {
        return Vec::new();
    };

    collect_mapping_key_set(&lines, env_line_idx + 1, 2)
}

/// Extract and normalize the SOLE `NEEDS_JSON:` env-child line (10-space
/// indent, inside the gate step's `env:` block) for pinned-literal
/// comparison against `PINNED_GATE_NEEDS_JSON_LINE`.
///
/// This is CRITICAL 1's fix (PR #671 review round 11): `NEEDS_JSON` is
/// the ENTIRE input `scripts/check-ci-gate.sh` ever sees. A step-key-set
/// pin (below) confirms `env:` exists as a key but says nothing about
/// what value its child carries — replacing `${{ toJSON(needs) }}` with
/// a hand-written, permanently-successful JSON literal is invisible to
/// every check that existed before this one. Same reject-don't-parse
/// normalization rules as `extract_and_normalize_sole_run_line` (a
/// deliberately separate function for the same reason that one is
/// separate from `extract_and_normalize_if_expr` — see that function's
/// doc comment), parameterized for the 10-space indent depth and the
/// `NEEDS_JSON:` key instead of `run:`.
fn extract_and_normalize_sole_needs_json_line(job_block: &str) -> Result<String, String> {
    let lines: Vec<&str> = job_block.lines().collect();
    let is_needs_json_line = |l: &&str| {
        let Some(after_indent) = l.strip_prefix("          ") else {
            return false;
        };
        !after_indent.starts_with(' ') && after_indent.starts_with("NEEDS_JSON:")
    };

    let indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| is_needs_json_line(l))
        .map(|(i, _)| i)
        .collect();

    if indices.is_empty() {
        return Err(
            "has no `NEEDS_JSON:` env-child line at all — the gate script \
             would run with no `NEEDS_JSON` env var set, which fails \
             closed (empty input) rather than silently, but is not the \
             pinned, reviewed input path."
                .to_string(),
        );
    }
    if indices.len() > 1 {
        return Err(format!(
            "has {} `NEEDS_JSON:` env-child lines — this checker requires \
             exactly one so a single pinned literal unambiguously covers \
             the gate's input source.",
            indices.len()
        ));
    }
    let idx = indices[0];

    let line = lines[idx];
    let raw = line.trim_start().strip_prefix("NEEDS_JSON:").unwrap_or("");
    let raw_value_leading_trimmed = raw.trim_start();

    if raw_value_leading_trimmed.starts_with('>') || raw_value_leading_trimmed.starts_with('|') {
        return Err(format!(
            "uses a YAML block-scalar form (\"{}\") for its `NEEDS_JSON:` \
             value — the real value lives on continuation lines this \
             checker does not read, so it cannot be safely represented as \
             a single pinned literal.",
            raw_value_leading_trimmed.trim()
        ));
    }

    if let Some(next_line) = lines[idx + 1..].iter().find(|l| {
        let trimmed = l.trim_start();
        !trimmed.is_empty() && !trimmed.starts_with('#')
    }) {
        let indent = next_line.len() - next_line.trim_start().len();
        if indent > 10 {
            return Err(format!(
                "has a `NEEDS_JSON:` value that appears to continue onto a \
                 following line (\"{}\", indented {indent} spaces) — this \
                 cannot be safely represented as a single pinned literal.",
                next_line.trim()
            ));
        }
    }

    let value = match find_comment_start(raw) {
        Some(i) => &raw[..i],
        None => raw,
    };
    if value.contains('#') {
        return Err(format!(
            "has a `NEEDS_JSON:` value containing a `#` that is not a \
             clearly whitespace-delimited trailing comment (\"{}\") — this \
             cannot be safely normalized.",
            raw.trim()
        ));
    }

    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return Err("has an empty `NEEDS_JSON:` value.".to_string());
    }

    Ok(collapsed)
}

/// Extract the YAML key name `line` declares, if `line` declares a key at
/// EXACTLY `indent` spaces of indentation.
///
/// PR #671 review round 11: a generalization of
/// `line_declares_job_level_key` (round 10, IMPORTANT 2) from "does this
/// line declare THIS SPECIFIC key" to "what key, if any, does this line
/// declare" — needed here to build a COMPLETE key set rather than test
/// membership of one known name. Carries over the same quoting awareness
/// (bare, double-quoted, single-quoted, arbitrary whitespace before the
/// colon) for the same reason: a key-set pin that only recognized bare
/// spellings would be exactly as blind to a quoted key as round 9's
/// original outputs guard was.
///
/// Additionally strips a leading YAML sequence marker (`- `) if present,
/// so this same function extracts BOTH a step's own first key (e.g. the
/// `name` in `      - name: Harden the runner`, at 6-space indent) and an
/// ordinary mapping key (everything else, at whatever indent the caller
/// requests) — a list item's first key is textually preceded by the `- `
/// marker but is otherwise a normal key at that nesting level.
fn extract_key_name_at_indent(line: &str, indent: usize) -> Option<String> {
    let padding = " ".repeat(indent);
    let after_indent = line.strip_prefix(padding.as_str())?;
    if after_indent.starts_with(' ') {
        return None; // Deeper than `indent`: not a key at this level.
    }
    let after_marker = after_indent.strip_prefix("- ").unwrap_or(after_indent);

    for quote in ['"', '\''] {
        if let Some(rest) = after_marker.strip_prefix(quote) {
            if let Some(end) = rest.find(quote) {
                if rest[end + 1..].trim_start().starts_with(':') {
                    return Some(rest[..end].to_string());
                }
            }
        }
    }

    // YAML explicit-key syntax: "? <key>" declares a key with the value
    // appearing on a SEPARATE following line (": <value>"), so there is
    // no colon to require on this line at all. Without this branch,
    // `key_end` below lands on the space after "?" and the bare-key
    // fallback bails (no colon found on the same line), letting a
    // `? defaults` / `: {run: {shell: "cat {0}"}}` override slip past
    // undetected (round-13 IMPORTANT 4).
    if let Some(explicit_key) = after_marker.strip_prefix("? ") {
        let key = explicit_key.trim();
        if !key.is_empty() {
            return Some(key.to_string());
        }
    }

    let key_end = after_marker.find(|c: char| c == ':' || c.is_whitespace())?;
    if after_marker[key_end..].trim_start().starts_with(':') {
        Some(after_marker[..key_end].to_string())
    } else {
        None
    }
}

/// Extract the sorted, complete list of job-level key NAMES in
/// `job_block` (4-space indent — job-level, not step-level or deeper).
/// Not deduplicated: a duplicate key is preserved so it shows up as an
/// extra entry against `PINNED_GATE_JOB_KEYS`, which is itself sorted
/// and duplicate-free — a real duplicate key correctly fails the
/// comparison rather than silently collapsing.
fn extract_job_level_key_set(job_block: &str) -> Vec<String> {
    let mut keys: Vec<String> = job_block
        .lines()
        .filter_map(|l| extract_key_name_at_indent(l, 4))
        .collect();
    keys.sort();
    keys
}

/// Extract the sorted, complete key set of EVERY step in the `ci-gate`
/// job's `steps:` list, in step order, as a `Vec` of per-step sorted key
/// lists.
///
/// Scoped to AFTER the job-level `steps:` line specifically (rather than
/// scanning the whole `job_block` for 6-space-indent-plus-dash lines) so
/// this does not misidentify a block-list-style `needs:` item as a step
/// if `needs:` is ever converted from its current inline-array form
/// (`needs: [a, b, c]`) to block-list form — inline-array `needs:` has no
/// items at any indent today, so this distinction is not exercised by
/// the current file, but scoping to `steps:` is correct regardless of
/// that fact, not because of it.
fn extract_gate_step_key_sets(job_block: &str) -> Vec<Vec<String>> {
    let lines: Vec<&str> = job_block.lines().collect();
    let Some(steps_line_idx) = lines
        .iter()
        .position(|l| extract_key_name_at_indent(l, 4).as_deref() == Some("steps"))
    else {
        return Vec::new();
    };

    let step_start_indices: Vec<usize> = lines[steps_line_idx + 1..]
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("      -"))
        .map(|(i, _)| i + steps_line_idx + 1)
        .collect();

    step_start_indices
        .iter()
        .enumerate()
        .map(|(idx, &start)| {
            let end = step_start_indices
                .get(idx + 1)
                .copied()
                .unwrap_or(lines.len());
            let mut keys: Vec<String> = lines[start..end]
                .iter()
                .filter_map(|l| {
                    extract_key_name_at_indent(l, 6).or_else(|| extract_key_name_at_indent(l, 8))
                })
                .collect();
            keys.sort();
            keys
        })
        .collect()
}

/// Build a `toJSON(needs)`-shaped JSON payload matching PRODUCTION shape:
/// every job in `all_jobs` carries `result` and `outputs` (an empty
/// object) — not `result` alone. Every job named in `skipped_jobs` reports
/// `skipped` for `result`; every other job reports `success`.
///
/// SOURCE (PR #671 review round 8 — corrects a round-7 error; round 9
/// added two more independent confirmations plus a correction to the
/// record below): per GitHub's contexts reference
/// (<https://docs.github.com/en/actions/learn-github-actions/contexts>,
/// "needs context" — also independently confirmed against the raw
/// `github/docs` source, `content/actions/reference/workflows-and-actions/
/// contexts.md`, whose `needs` table has exactly five rows and no
/// `outcome` row; that file path is the more durable of the two citations,
/// since the rendered URL can be restructured across a docs-site redesign
/// while the source repo path is versioned), `needs.<job_id>` has EXACTLY
/// two properties: `result` and `outputs`. There is no `outcome` on
/// `needs` — `outcome` belongs solely to the STEPS context
/// (`steps.<step_id>.outcome`, "result of a completed step before
/// `continue-on-error` is applied"), a per-step concept with no job-level
/// analog. Round 7 modeled this payload as `result`+`outputs`+`outcome`,
/// sourced from an earlier fixture's unverified "realistic shape" comment
/// that nobody checked against the docs before three more rounds cited and
/// built on it.
///
/// CORRECTED RECORD (round 9 — round 8's own report of its RED proof was
/// itself imprecise about which guard caught what): that phantom `outcome`
/// field was worse than merely redundant — because every fixture and
/// payload had it while production payloads never do, a mutation keyed on
/// its ABSENCE could diverge between test and production. But the SPECIFIC
/// single-conjunct mutation round 8 reproduced,
/// `is_allowed_skip || !has("outcome")`, was only HALF-lethal under the
/// round-7 shape: reproduced fresh (round 9, against the actual round-7
/// commit's fixtures and payload builder in isolation) it defeated
/// `cargo test` completely (stayed 13/13 green) but was still CAUGHT by
/// `--self-test` (11/13 — both the minimal `result`-only
/// `unlisted-job-skipped` fixture and the then-`outcome`-carrying
/// `unlisted-job-skipped-full-production-shape` fixture independently
/// flagged it), because bash's own fixtures were not uniformly
/// `outcome`-carrying the way every Rust-side payload was. Round 8's
/// original report described this as caught by "neither" guard fully
/// fooled — that was wrong in the self-test direction; it should have
/// read "cargo test only." The mutation that actually matters — the one
/// GitHub Actions review round 7 identified as truly production-shape-
/// exact — is the TWO-CONJUNCT form: `has("outputs") and (has("outcome")
/// | not)`. Reproduced (round 9, against the true round-7 commit): this
/// form defeats BOTH `--self-test` (13/13) AND `cargo test` (13/13)
/// completely, because it evaluates to `true` for every job in every
/// round-7 fixture/payload (all of which carry both `outputs` and
/// `outcome`) exactly as it does for a real, unlisted skipped job in
/// production (which carries `outputs` but never `outcome`) — a live gate
/// run on `{"fmt":{"result":"skipped","outputs":{}}}` returns rc=0 under
/// this mutation. Reproduced again against the CURRENT (round 8+)
/// `outputs`-only shape: this same two-conjunct mutation is caught by both
/// guards (`--self-test` 12/13, `cargo test` 12 passed/2 failed) — the
/// CRITICAL this shape correction closes is real and confirmed closed.
///
/// CRITICAL-3 (PR #671 review round 7 — corrected attribution, round 9):
/// the ORIGINAL prior-round single-field `{"job":{"result":"..."}}` shape,
/// used everywhere in this test before round 7, never exercised any
/// codepath keying on a sibling field at all — a mutation tolerating any
/// skipped job carrying an `outputs` key (i.e. every real job, since
/// `outputs` is always present) passed every payload this test built
/// before round 7 (reproduced: exploitable under commit `eb6e4551`'s
/// `result`-only payloads). Round 7's STATED rationale for its fix chased
/// the phantom `outcome` field described above — chasing the wrong
/// diagnosis — but its ACTUAL code change (fixture 13 plus modeling
/// `outputs` in every payload) closed this real, live hole regardless:
/// right fix, wrong stated reason. `outputs` is genuinely part of the real
/// shape (see SOURCE above) and stays modeled here; `outcome` does not
/// exist on `needs` and has been removed (round 8).
///
/// Also used to cover the realistic PRODUCTION case that every actual push
/// already has: `mutants` is essentially always `skipped` alongside
/// whatever job is under test (see `all_skip_variants_for` below) — a
/// mutation keying on skip-COUNT (e.g. "≥2 skips means tolerate") rather
/// than per-job identity would pass a single-skip-only test suite but fail
/// against the shape every push actually produces.
///
/// STRUCTURAL (PR #671 review round 9): the root cause shared by CRITICAL-3
/// and the round-7/8 `outcome` defect is the same — the payload shape was a
/// HAND-CHOSEN sample, so round 7 could "fix" one wrong sample by
/// substituting another wrong one without anything forcing a check against
/// the actual contract. `NEEDS_CONTEXT_JOB_KEYS` below is that contract,
/// pinned once, with its own citation; every payload this function builds
/// is asserted against it before being returned, so a future round can no
/// longer invent (or drop) a field here without this assertion failing and
/// forcing them to look at — and update, in the same change as any real
/// contract change — the pinned source of truth, not just this call site.
///
/// `#[cfg(unix)]` (PR #671 review round 15, CI-caught): used only by
/// `build_multi_skip_payload` below, itself called only from the two
/// `#[cfg(unix)]`-gated tests that build multi-skip payloads — see
/// `list_all_ci_yml_job_names`'s doc comment for the full "gating a test
/// orphans its helpers" explanation.
#[cfg(unix)]
const NEEDS_CONTEXT_JOB_KEYS: &[&str] = &["outputs", "result"];

/// `#[cfg(unix)]`: this function's only callers are
/// `test_ci_gate_decision_matches_job_level_if_for_every_needs_member` and
/// `test_ci_gate_decision_is_arity_independent_for_unlisted_skips`, both
/// themselves `#[cfg(unix)]`-gated.
#[cfg(unix)]
fn build_multi_skip_payload(all_jobs: &[String], skipped_jobs: &[&str]) -> String {
    let mut obj = serde_json::Map::new();
    for job in all_jobs {
        let result = if skipped_jobs.contains(&job.as_str()) {
            "skipped"
        } else {
            "success"
        };
        let job_value = serde_json::json!({
            "result": result,
            "outputs": {},
        });
        let mut actual_keys: Vec<&str> = job_value
            .as_object()
            .expect("job_value is always constructed as a JSON object literal above")
            .keys()
            .map(String::as_str)
            .collect();
        actual_keys.sort_unstable();
        assert_eq!(
            actual_keys, NEEDS_CONTEXT_JOB_KEYS,
            "FAIL (PR #671 review round 9, structural): the payload this \
             test synthesizes for `{job}` has key set {actual_keys:?}, not \
             the pinned `{NEEDS_CONTEXT_JOB_KEYS:?}` — per GitHub's \
             contexts reference \
             (https://docs.github.com/en/actions/learn-github-actions/contexts, \
             \"needs context\"), `needs.<job_id>` has EXACTLY these two \
             properties. If you are adding a field here because you believe \
             the real `needs` context has grown a new property, verify that \
             against the docs FIRST and update NEEDS_CONTEXT_JOB_KEYS in the \
             same change — do not just add a field to this literal, or the \
             next reader inherits an unverified assumption exactly as \
             happened with the phantom `outcome` field this round removed \
             (see this function's doc comment)."
        );
        obj.insert(job.clone(), job_value);
    }
    serde_json::Value::Object(obj).to_string()
}

/// The set of payload "skip variants" to check for job `job`: always the
/// single-skip case (only `job` skipped), PLUS the production-realistic
/// multi-skip case where `job` and `mutants` are both skipped (the shape
/// every real push already has, since `mutants` reports `skipped` on
/// every push by design). When `job` IS `mutants`, the two variants are
/// identical — still run both for code-path uniformity; the redundancy is
/// cheap.
///
/// `#[cfg(unix)]` (PR #671 review round 15, CI-caught): only caller is
/// `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`,
/// itself `#[cfg(unix)]`-gated — see `list_all_ci_yml_job_names`'s doc
/// comment for the full "gating a test orphans its helpers" explanation.
#[cfg(unix)]
fn all_skip_variants_for(job: &str) -> Vec<Vec<&str>> {
    vec![vec![job], vec![job, "mutants"]]
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

/// S-CIGATE-2 CRITICAL (PR #671 review, round 3, pinned-literal design as
/// of round 6, hardened round 7) — the behavioral closure. See the module
/// comment above this section for the full history of why rounds 1-5 (a
/// representation guard, then four generations of predicate-based
/// `if:`-legitimacy oracles) were each bypassed, and
/// `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS`/`extract_and_normalize_if_expr`'s
/// doc comments for the pinned-literal design and its round-7
/// reject-don't-parse hardening.
///
/// For every job in `ci-gate.needs` (parsed from `ci.yml`, so this test
/// tracks the real job graph automatically as it changes):
///   - Extract and normalize the job's `ci.yml` job-level `if:` expression
///     (`extract_and_normalize_if_expr`). An `Err` (an `if:` form this
///     checker cannot safely represent — see CRITICAL-1/2 in that
///     function's doc comment) is an IMMEDIATE, unconditional test
///     failure for that job — it never falls through to either branch
///     below.
///   - Look up a pinned literal for this job by name in
///     `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS`.
///   - If BOTH exist AND are byte-identical: this is the POSITIVE
///     (legitimate-skip) case. For EACH of `all_skip_variants_for(job)`
///     (single-skip, and the production-realistic job+`mutants` both
///     skipped — CRITICAL-3, PR #671 review round 7), synthesize a
///     PRODUCTION-SHAPED payload (`result`+`outputs` per job, not
///     `result` alone — `build_multi_skip_payload`; round 8 corrected
///     this to drop a phantom `outcome` field the real `needs` context
///     does not have — see that function's doc comment), run
///     `scripts/check-ci-gate.sh` against it in a real subprocess, and
///     assert: exit code is EXACTLY 0 AND stdout contains an `OK  <job> =
///     skipped` line naming this job.
///   - Otherwise (no `if:` at all, no pin exists for this job, OR the
///     `if:` text doesn't match the pinned literal): this is the NEGATIVE
///     case. Same payload variants, asserting: exit code is EXACTLY 1 AND
///     stdout contains a `FAIL  <job> = skipped` line. If a job WAS
///     actually added to `ALLOWED_SKIPS` without a matching pin, this
///     branch's assertion fails LOUDLY, naming the job.
///
/// Non-vacuity (IMPORTANT 1, round 5): also asserts at least one job took
/// the positive branch and at least one took the negative branch.
///
/// Pin-coverage (IMPORTANT 5, PR #671 review round 7): independent of the
/// per-job loop above (which only ever sees `ci-gate.needs` members),
/// asserts every job `--print-allowed-skips` reports as an `ALLOWED_SKIPS`
/// member has a pinned entry — closing the gap where a job outside
/// `needs` today could be pre-emptively (or accidentally) added to
/// `ALLOWED_SKIPS` with no pin, then silently inherit tolerance the
/// moment a later, unrelated change adds it to `ci-gate.needs`.
///
/// This test does not parse, count, or shell out to ask about
/// `ALLOWED_SKIPS`'s array representation — it has no opinion on how the
/// script represents its allowlist internally, and no opinion on whether
/// an `if:` expression is "legitimate" either — only on whether it
/// matches a human-reviewed pin, PRODUCTION-SHAPED payloads matching what
/// real `toJSON(needs)` actually looks like. A subscripted assignment, a
/// nameref, `read -a`, `mapfile`, a parallel array, a fully rewritten
/// `is_allowed_skip`, a trailing comment, a permanently-false condition,
/// a block-scalar `if:`, a glued `#`, or a mutation keying tolerance on
/// an unrelated sibling field can only pass this test by making the
/// SCRIPT ITSELF produce the correct exit code AND diagnostic line on
/// every one of these synthesized, production-shaped payloads — which is
/// the actual property this story exists to guarantee.
///
/// `#[cfg(unix)]`: `scripts/check-ci-gate.sh` only ever runs on
/// `ubuntu-latest` in this repo's actual CI; this test still runs on
/// `ubuntu-latest` and `macos-latest` in the `test` job's 3-OS matrix (the
/// `test` matrix requires all three legs green, so this is not a coverage
/// gap on the platform that matters).
///
/// Proven RED (round 7, with sha256-verified byte-identical restores)
/// against all three CRITICALs from that round's review: CRITICAL-1
/// (`if: >-` block-scalar laundering, including the continuation-swap
/// variant — pinning `("deny", ">-")` then swapping the continuation line
/// with zero pin change), CRITICAL-2 (glued `#`:
/// `${{ vars.RUN_DENY == 'true' }}#${{ always() }}` normalizing to the
/// legitimate-looking prefix before the round-7 fix), and the CRITICAL-3
/// skip-count-≥2 mutation (still valid under the round-8 `outputs`-only
/// shape). Round 7's OTHER claimed CRITICAL-3 instance
/// (`has("outcome")`) was itself built on a false premise — see
/// `build_multi_skip_payload`'s doc comment — and is superseded by round
/// 8's finding below.
///
/// Proven RED (round 8, sha256-verified byte-identical restores; CORRECTED
/// round 9 — see below): the single-conjunct INVERTED mutation,
/// `is_allowed_skip "${job}" || ! echo "${json}" | jq -e --arg j "${job}"
/// '.[$j] | has("outcome")'`, tolerates every skip in a live gate run,
/// since real `needs` entries never carry `outcome`. Under the round-8
/// corrected shape (`outputs` only, no `outcome`), this same mutated line
/// is inert — `has("outcome")` is always `false` for both test payloads
/// and production, so the mutation can no longer distinguish anything and
/// every fixture/test still catches it via the unchanged `is_allowed_skip`
/// half. Also re-verified RED: `has("outputs")` (tolerates any skipped
/// job, since `outputs` IS always present — CRITICAL-3 original, still
/// exploitable and still caught).
///
/// CORRECTED (round 9): round 8's report of this proof claimed the
/// single-conjunct inverted mutation "passed `--self-test` (13/13) and
/// `cargo test` (all green)" under the round-7 shape. Re-reproduced
/// (round 9) against the actual round-7 commit in isolation: `cargo test`
/// DID stay 13/13 green, but `--self-test` did NOT — it dropped to 11/13,
/// with BOTH the minimal `unlisted-job-skipped` fixture (`result`-only,
/// no `outcome` key, so `!has("outcome")` was already `true` there
/// independent of any shape question) and
/// `unlisted-job-skipped-full-production-shape` correctly flagging the
/// mismatch. The claim should have read "cargo test only" — self-test was
/// only ever half-fooled, not fully. The mutation that IS fully lethal
/// under the round-7 shape (defeats BOTH guards completely, and matches
/// production exactly) is the TWO-CONJUNCT form review round 7 actually
/// identified: `has("outputs") and (has("outcome") | not)`. Reproduced
/// (round 9) against the true round-7 commit: `--self-test` 13/13,
/// `cargo test` 13/13, live gate rc=0 on
/// `{"fmt":{"result":"skipped","outputs":{}}}` — full false-green.
/// Reproduced again against the CURRENT (`outputs`-only, post-round-8)
/// shape: this same two-conjunct mutation is now caught by both guards
/// (`--self-test` 12/13, `cargo test` 12 passed / 2 failed) — the CRITICAL
/// this round's shape correction was meant to close is confirmed closed
/// against the mutation that actually mattered, not just the half-lethal
/// one originally reported.
///
/// Also re-confirmed RED (unchanged from rounds 3/5/6): mutations A and B,
/// all four round-2 `+=`-form bypasses, and the four round-5/6 bypasses
/// (trailing-comment no-op, bare `always()`, permanently-false
/// `github.ref`, `always() || github.ref`). The legitimate `mutants` case
/// still passes (positive branch, exit 0, both payload variants).
#[cfg(unix)]
#[test]
fn test_ci_gate_decision_matches_job_level_if_for_every_needs_member() {
    use std::process::Command;

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

    // ADDENDUM (PR #671 review round 9): the structural key-set pin above
    // (`NEEDS_CONTEXT_JOB_KEYS`) guarantees every payload has the right
    // KEYS, but `build_multi_skip_payload` still hard-codes `"outputs":
    // {}` — faithful ONLY because none of today's `ci-gate.needs` jobs
    // declare a job-level `outputs:` block in ci.yml. The moment one does,
    // production `needs.<job>.outputs` stops being reliably `{}` and this
    // model silently diverges — the same train/test-divergence class as
    // the phantom `outcome` field and the original CRITICAL-3, a third
    // variant. Rather than try to synthesize realistic output VALUES
    // (which would need per-job knowledge this test has no way to derive
    // generically), fail closed: assert the premise that makes `{}`
    // faithful still holds, so a job gaining real outputs turns this into
    // a loud, named failure instead of a silent model/reality gap.
    //
    // IMPORTANT 2 (PR #671 review round 10): the original round-9 check —
    // `l.starts_with("    outputs:")` — matches exactly ONE of four
    // equivalent YAML spellings for a job-level `outputs:` key.
    // PyYAML-confirmed `outputs:`, `"outputs":`, `'outputs':`, and
    // `outputs :` all parse to the identical key; a real job-level
    // `outputs:` block spelled any of the latter three left this guard
    // silently blind (14/14 green) while the `{}` premise it exists to
    // protect no longer held. `line_declares_job_level_key` below
    // recognizes all four. This guard's job is narrow enough (one literal
    // key name, one fixed indent depth) that enumerating the recognized
    // forms is tractable and auditable — unlike `if:`'s arbitrary
    // expression text, which is why THAT function rejects rather than
    // tries to enumerate.
    for job in &all_jobs {
        let job_block = extract_job_block(&ci, job).unwrap_or_else(|| {
            panic!("FAIL: `ci-gate.needs` names `{job}`, but no `{job}:` job exists in ci.yml.")
        });
        let declares_outputs = job_block
            .lines()
            .any(|l| line_declares_job_level_key(l, "outputs"));
        assert!(
            !declares_outputs,
            "FAIL (PR #671 review round 9, outputs-content addendum): \
             `{job}` declares a job-level `outputs:` block in ci.yml. This \
             test's synthesized payloads hard-code `\"outputs\": {{}}` \
             (empty), which was faithful only because no `ci-gate.needs` \
             job had real outputs — that assumption no longer holds for \
             `{job}`. Update `build_multi_skip_payload` (and this guard) \
             to model `{job}`'s real output shape before trusting this \
             test's coverage of `{job}` again; do not just delete this \
             assertion."
        );
    }

    let mut saw_positive_branch = false;
    let mut saw_negative_branch = false;

    for job in &all_jobs {
        let job_block = extract_job_block(&ci, job).unwrap_or_else(|| {
            panic!("FAIL: `ci-gate.needs` names `{job}`, but no `{job}:` job exists in ci.yml.")
        });

        let actual_if_expr = match extract_and_normalize_if_expr(job_block) {
            Err(reason) => {
                panic!(
                    "FAIL (S-CIGATE-2, PR #671 review round 7): `{job}`'s \
                     ci.yml job-level `if:` {reason}\n\
                     \n\
                     This `if:` form is UNSUPPORTED for pinning and MUST \
                     be rewritten as a single-line plain scalar (no YAML \
                     block-scalar marker, no line continuation, no \
                     ambiguous embedded `#`) before `{job}` can be \
                     evaluated for ALLOWED_SKIPS membership at all."
                );
            }
            Ok(expr) => expr,
        };

        let pinned_expr = PINNED_ALLOWED_SKIP_IF_EXPRESSIONS
            .iter()
            .find(|(name, _)| *name == job.as_str())
            .map(|(_, expr)| *expr);

        let is_pinned_and_matches = match (&actual_if_expr, pinned_expr) {
            (Some(actual), Some(pinned)) => actual == pinned,
            _ => false,
        };

        if is_pinned_and_matches {
            saw_positive_branch = true;
        } else {
            saw_negative_branch = true;
        }

        for skipped_jobs in all_skip_variants_for(job) {
            let variant_desc = if skipped_jobs.len() == 1 {
                "single-skip".to_string()
            } else {
                format!("multi-skip ({})", skipped_jobs.join(" + "))
            };
            let payload = build_multi_skip_payload(&all_jobs, &skipped_jobs);
            let output = run_check_ci_gate_sh(&payload);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let exit_code = output.status.code();

            if is_pinned_and_matches {
                let expected_line = format!("OK  {job} = skipped");
                let exit_ok = exit_code == Some(0);
                let stdout_ok = stdout.contains(&expected_line);
                assert!(
                    exit_ok && stdout_ok,
                    "FAIL (S-CIGATE-2 behavioral closure, PR #671 review \
                     round 7, {variant_desc} payload): `{job}`'s job-level \
                     `if:` matches its pinned, human-reviewed literal \
                     (\"{}\"), but scripts/check-ci-gate.sh did not accept \
                     a payload where `{job}` (and, for the multi-skip \
                     variant, `mutants`) is `skipped`.\n\
                     {}\n\
                     {}\n\
                     \n\
                     This would be a false-red: the gate must still \
                     tolerate the one legitimate PR-only/repo-\
                     variable-gated skip, not just reject everything.\n\
                     \n\
                     Payload used:\n{payload}\n\
                     \n\
                     --- stdout ---\n{stdout}\n\
                     --- stderr ---\n{}",
                    pinned_expr.unwrap_or(""),
                    if exit_ok {
                        format!("Exit code: {exit_code:?} (OK).")
                    } else {
                        format!("Exit code: {exit_code:?} (expected Some(0)).")
                    },
                    if stdout_ok {
                        format!("stdout contained \"{expected_line}\" (OK).")
                    } else {
                        format!("stdout did NOT contain \"{expected_line}\".")
                    },
                    String::from_utf8_lossy(&output.stderr)
                );
            } else {
                let expected_line = format!("FAIL  {job} = skipped");
                let exit_ok = exit_code == Some(1);
                let stdout_ok = stdout.contains(&expected_line);

                let diagnosis = match (&actual_if_expr, pinned_expr) {
                    (None, _) => format!(
                        "`{job}` has no job-level `if:` in ci.yml at all (an always-run job)."
                    ),
                    (Some(actual), None) => format!(
                        "`{job}` has a job-level `if:` (\"{actual}\") but no pinned \
                         literal exists for `{job}` in PINNED_ALLOWED_SKIP_IF_EXPRESSIONS."
                    ),
                    (Some(actual), Some(pinned)) => format!(
                        "`{job}`'s job-level `if:` (\"{actual}\") does not match its \
                         pinned literal (\"{pinned}\")."
                    ),
                };

                assert!(
                    exit_ok && stdout_ok,
                    "FAIL (S-CIGATE-2 behavioral closure, PR #671 review \
                     round 7, {variant_desc} payload): {diagnosis}\n\
                     \n\
                     If `{job}` was intentionally added to \
                     scripts/check-ci-gate.sh's ALLOWED_SKIPS, this is \
                     UNAUTHORIZED WIDENING from this test's perspective: \
                     widening the trust boundary requires (1) a human \
                     reviewing the exact `if:` expression text and \
                     confirming it can genuinely evaluate both `true` and \
                     `false` in practice, (2) adding a matching entry to \
                     PINNED_ALLOWED_SKIP_IF_EXPRESSIONS naming `{job}` and \
                     its exact (normalized) `if:` text, in the SAME change \
                     that adds it to ALLOWED_SKIPS. Without that pin, the \
                     gate must reject `{job}`'s skip.\n\
                     {}\n\
                     {}\n\
                     \n\
                     Payload used:\n{payload}\n\
                     \n\
                     --- stdout ---\n{stdout}\n\
                     --- stderr ---\n{}",
                    if exit_ok {
                        format!("Exit code: {exit_code:?} (OK).")
                    } else {
                        format!("Exit code: {exit_code:?} (expected Some(1)).")
                    },
                    if stdout_ok {
                        format!("stdout contained \"{expected_line}\" (OK).")
                    } else {
                        format!("stdout did NOT contain \"{expected_line}\".")
                    },
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }

    assert!(
        saw_positive_branch,
        "FAIL: no job in ci-gate.needs matched a pinned literal — the \
         positive (legitimate-skip) branch above never ran, so this test \
         cannot prove the gate still tolerates a real skip. This should \
         not happen while `mutants` remains in ci-gate.needs with its \
         current `if:` matching PINNED_ALLOWED_SKIP_IF_EXPRESSIONS; if it \
         does, something upstream (job list, `if:` text, or the pinned \
         literal) changed unexpectedly and needs re-review."
    );
    assert!(
        saw_negative_branch,
        "FAIL: no job in ci-gate.needs lacks a matching pinned literal — \
         the negative (reject-illegitimate-skip) branch above never ran. \
         Without at least one negative case, a pathological mutation that \
         makes every payload succeed (or every payload fail identically, \
         e.g. by reading input from the wrong source) could pass this \
         test vacuously."
    );

    // IMPORTANT 5 / IMPORTANT 2 / IMPORTANT 3 (PR #671 review rounds 7 and
    // 9): pin-coverage check, independent of ci-gate.needs.
    //
    // WHAT THIS BLOCK ACTUALLY ADDS (corrected, round 9 — the round-7
    // rationale below was run and found false): the main per-job loop
    // above iterates `ci-gate.needs` members and, for each, already
    // requires a pin whenever `is_pinned_and_matches` is false — so for a
    // job that IS a `ci-gate.needs` member, the main loop ALREADY fails
    // loudly if it's skip-tolerant with no pin. Reproduced: adding `deny`
    // (a real `needs` member) to `ALLOWED_SKIPS` with no pin fails at the
    // main loop's negative-branch assertion, naming `deny` — NOT here.
    // This block's actual, unique value is for an `ALLOWED_SKIPS` member
    // that is NOT (yet) a `ci-gate.needs` member (e.g. `security`, a real
    // job in this file that ci-gate does not depend on) — the main loop
    // never iterates such a job at all, so THIS is the only code path that
    // would catch a missing pin for it, closing exactly the gap the
    // round-7 text described but attributed to the wrong scenario.
    // Reproduced: adding `security` (not a needs member) to
    // `ALLOWED_SKIPS` with no pin is caught here, and only here.
    //
    // IMPORTANT 2 (round 9): the round-7 version of this block asked ONLY
    // `--print-allowed-skips` what `ALLOWED_SKIPS` contains, and trusted
    // that answer completely — but `print_allowed_skips()` is a SEPARATE
    // function from `is_allowed_skip()` (the one `evaluate_needs()` — the
    // function that actually decides pass/fail — calls). A mutation to
    // `print_allowed_skips()` alone (e.g. filtering its output to hide a
    // member) desyncs the two without touching enforcement at all:
    // `--print-allowed-skips` would under-report, this block would
    // silently require pins for fewer jobs than are actually tolerated,
    // and the real gate would still accept the hidden member's skip.
    // Reproduced: this exact case (`ALLOWED_SKIPS=("mutants" "security")`
    // with `print_allowed_skips` filtering `security` out of its own
    // output) left this round-7 block silently satisfied while the real
    // gate returned rc=0 on a skipped `security`.
    //
    // FIX: do not trust `print_allowed_skips()`'s account of what
    // `evaluate_needs()` tolerates — ask `evaluate_needs()` directly, by
    // actually running it (via the real script entry point) on a
    // single-job payload for every job name that exists ANYWHERE in
    // ci.yml (`list_all_ci_yml_job_names` — not just `ci-gate.needs`
    // members, since an `ALLOWED_SKIPS` member need not be one). Whichever
    // jobs `evaluate_needs()` ITSELF actually tolerates a `skipped` result
    // for (`behaviorally_allowed` below) is behaviorally-verified ground
    // truth: it cannot diverge from real enforcement, because it comes
    // from the same code path enforcement uses, not a separate reporting
    // function that could drift from it. Require a pin for every
    // behaviorally-allowed job, AND separately assert that this
    // independently-derived set matches what `--print-allowed-skips`
    // reports — a real regression in EITHER function surfaces as a named
    // mismatch, not a silent pass.
    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-ci-gate.sh");

    let mut behaviorally_allowed: Vec<String> = Vec::new();
    for candidate in list_all_ci_yml_job_names(&ci) {
        let probe_payload = format!(r#"{{"{candidate}":{{"result":"skipped","outputs":{{}}}}}}"#);
        let probe_output = run_check_ci_gate_sh(&probe_payload);
        let probe_stdout = String::from_utf8_lossy(&probe_output.stdout);
        if probe_stdout.contains(&format!("OK  {candidate} = skipped")) {
            behaviorally_allowed.push(candidate);
        }
    }
    behaviorally_allowed.sort();

    for member in &behaviorally_allowed {
        let has_pin = PINNED_ALLOWED_SKIP_IF_EXPRESSIONS
            .iter()
            .any(|(name, _)| name == member);
        assert!(
            has_pin,
            "FAIL (PR #671 review round 9, IMPORTANT 2/5): `{member}` is \
             BEHAVIORALLY tolerated as a `skipped` result by \
             scripts/check-ci-gate.sh's real `evaluate_needs()` (verified \
             by directly running the script — not by trusting \
             `--print-allowed-skips`) but has no entry in \
             PINNED_ALLOWED_SKIP_IF_EXPRESSIONS. This check is independent \
             of whether `{member}` is currently in `ci-gate.needs`. Add a \
             pinned entry for `{member}` naming its exact (normalized) \
             `if:` text."
        );
    }

    let print_output = Command::new("bash")
        .arg(&script_path)
        .arg("--print-allowed-skips")
        .output()
        .unwrap_or_else(|e| panic!("Could not run {}: {e}", script_path.display()));
    assert!(
        print_output.status.success(),
        "FAIL: `scripts/check-ci-gate.sh --print-allowed-skips` exited \
         non-zero (status: {:?}).",
        print_output.status.code()
    );
    let mut allowed_skips_members: Vec<String> = String::from_utf8_lossy(&print_output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    allowed_skips_members.sort();

    assert_eq!(
        behaviorally_allowed, allowed_skips_members,
        "FAIL (PR #671 review round 9, IMPORTANT 2): \
         `--print-allowed-skips`'s reported ALLOWED_SKIPS membership \
         disagrees with what scripts/check-ci-gate.sh's real \
         `evaluate_needs()` actually tolerates (verified by direct probe, \
         one job at a time). This means `print_allowed_skips()` has \
         drifted from `is_allowed_skip()` — the two are separate \
         functions in the script, and a mutation to either one alone (not \
         both) produces exactly this kind of disagreement. Whichever side \
         is wrong, `--print-allowed-skips` is no longer a trustworthy \
         account of the gate's real behavior."
    );
}

/// IMPORTANT 1 (PR #671 review round 9): arity-independence — the main
/// behavioral closure test above, via `all_skip_variants_for`, only ever
/// checks a payload with 1 skip or 2 skips (`job` alone, or `job` +
/// `mutants`). A mutation keyed on skip-COUNT rather than per-job identity
/// (e.g. `is_allowed_skip "${job}" || [ "$(… | jq '[.[] | select(.result==
/// "skipped")] | length')" -ge 3 ]`) is invisible to both arities tested
/// there and produces gate rc=0 on a real payload with 3+ skips, 2 of them
/// unlisted.
///
/// CHOICE (documented per review's request): this is closed by testing the
/// EXTREMAL case — every job in `ci-gate.needs` skipped simultaneously —
/// rather than the full 2^N cross product of skip subsets. A count-keyed
/// mutation of the form "skip count >= K tolerates everything" is exposed
/// by ANY payload with skip count >= K; testing the maximum possible
/// arity (`all_jobs.len()`) therefore refutes every such mutation for
/// every K up to the current job count in ONE additional subprocess call,
/// where the full cross product would need 2^`all_jobs.len()` payloads
/// (256 for today's 8 jobs) for a property already fully proven by the
/// boundary case. This scales automatically as `ci-gate.needs` grows,
/// since `all_jobs` (and therefore the arity tested) is derived from the
/// real job graph at runtime, not hardcoded — a mutation requiring K
/// skips to fire is exposed for exactly as long as K stays <=
/// `all_jobs.len()`. What this does NOT prove: a mutation keyed on some
/// property other than a skip-count threshold (e.g. skip-count PARITY) is
/// out of scope for this specific closure, same as it would be out of
/// scope for a bounded cross product of any size short of the full 2^N.
///
/// Reproduced: without a 3+-skip payload anywhere in this test file, a
/// `[ … -ge 3 ]` mutation passed `--self-test` (13/13, no fixture combines
/// 3+ skips) and this file's main behavioral test (13/13) while returning
/// gate rc=0 on a payload skipping `mutants` plus two other unlisted jobs.
///
/// Expected result computed independently of `all_skip_variants_for`'s
/// per-job classification, from the same `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS`
/// source of truth: with every job skipped, the gate must reject (exit 1)
/// unless literally every job's `if:` matches its pin (not the case today
/// — only `mutants` has one), and must name every non-matching job with a
/// `FAIL  <job> = skipped` line and every matching job with an
/// `OK  <job> = skipped` line.
#[cfg(unix)]
#[test]
fn test_ci_gate_decision_is_arity_independent_for_unlisted_skips() {
    let ci = read_ci_yml();
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_else(|| {
        panic!("FAIL: `.github/workflows/ci.yml` does not contain a `ci-gate:` job.")
    });
    let needs = parse_needs_set(gate_block).unwrap_or_else(|| {
        panic!("FAIL: the `ci-gate` job block does not contain a `needs:` key.")
    });

    let mut all_jobs: Vec<String> = needs.into_iter().collect();
    all_jobs.sort();

    assert!(
        all_jobs.len() >= 3,
        "FAIL: `ci-gate.needs` has fewer than 3 jobs ({}) — this test needs \
         at least 3 to distinguish a >=3-skip-count mutation from the \
         2-skip coverage the main behavioral test already provides. If \
         `ci-gate.needs` has legitimately shrunk below 3, this test's \
         premise needs revisiting, not silent weakening.",
        all_jobs.len()
    );

    let all_skipped: Vec<&str> = all_jobs.iter().map(String::as_str).collect();
    let payload = build_multi_skip_payload(&all_jobs, &all_skipped);
    let output = run_check_ci_gate_sh(&payload);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let exit_code = output.status.code();

    let mut expected_ok: Vec<&str> = Vec::new();
    let mut expected_fail: Vec<&str> = Vec::new();
    for job in &all_jobs {
        let job_block = extract_job_block(&ci, job).unwrap_or_else(|| {
            panic!("FAIL: `ci-gate.needs` names `{job}`, but no `{job}:` job exists in ci.yml.")
        });
        let actual_if_expr = extract_and_normalize_if_expr(job_block).unwrap_or_else(|reason| {
            panic!(
                "FAIL (S-CIGATE-2, PR #671 review round 9, arity-independence \
                 payload): `{job}`'s ci.yml job-level `if:` {reason}"
            )
        });
        let pinned_expr = PINNED_ALLOWED_SKIP_IF_EXPRESSIONS
            .iter()
            .find(|(name, _)| *name == job.as_str())
            .map(|(_, expr)| *expr);
        let matches = matches!((&actual_if_expr, pinned_expr), (Some(a), Some(p)) if a == p);
        if matches {
            expected_ok.push(job);
        } else {
            expected_fail.push(job);
        }
    }

    assert!(
        !expected_fail.is_empty(),
        "FAIL: every job in ci-gate.needs matches its pinned literal — this \
         test's premise (at least one job must be a legitimate rejection \
         target when ALL jobs are skipped) no longer holds; re-derive the \
         expected exit code before trusting this test's result."
    );

    assert_eq!(
        exit_code,
        Some(1),
        "FAIL (PR #671 review round 9, IMPORTANT 1): skipping every job in \
         ci-gate.needs simultaneously (arity {}) did not produce exit code \
         1, even though {} of them ({:?}) have no matching pin. A mutation \
         keyed on skip-COUNT rather than per-job identity (e.g. \"skip \
         count >= K tolerates everything\") would produce exactly this \
         symptom without being caught by the main behavioral test's 1- and \
         2-skip payloads.\n\nPayload used:\n{payload}\n\n--- stdout \
         ---\n{stdout}\n--- stderr ---\n{}",
        all_jobs.len(),
        expected_fail.len(),
        expected_fail,
        String::from_utf8_lossy(&output.stderr)
    );

    for job in &expected_fail {
        let expected_line = format!("FAIL  {job} = skipped");
        assert!(
            stdout.contains(&expected_line),
            "FAIL (PR #671 review round 9, IMPORTANT 1): expected stdout to \
             contain \"{expected_line}\" (arity-{} payload) but it did not.\n\
             \n--- stdout ---\n{stdout}",
            all_jobs.len()
        );
    }
    for job in &expected_ok {
        let expected_line = format!("OK  {job} = skipped");
        assert!(
            stdout.contains(&expected_line),
            "FAIL (PR #671 review round 9, IMPORTANT 1): expected stdout to \
             contain \"{expected_line}\" (arity-{} payload) but it did not \
             — a legitimately pinned job must still be tolerated even when \
             every OTHER job is also (illegitimately) skipped in the same \
             payload.\n\n--- stdout ---\n{stdout}",
            all_jobs.len()
        );
    }
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

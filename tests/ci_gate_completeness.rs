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
//! Test coverage map (→ S-CIGATE-1 AC — this story's ACs are zero-padded,
//! e.g. `AC-001`/`AC-002`/`AC-003`; see the SEPARATE S-626-1 table below,
//! which uses a different story's different, unpadded AC numbering):
//!   test_ci_gate_job_exists_with_required_metadata           → AC-001
//!   test_ci_gate_needs_exactly_the_required_jobs             → AC-003
//!   test_ci_gate_excludes_advisory_and_secret_scan_jobs      → AC-003
//!   test_mutants_is_in_ci_gate_needs                         → MUTATION-CI-TIMEOUT / AC-003
//!   test_ci_gate_fails_on_failed_or_cancelled_need           → AC-002 (retargeted, S-CIGATE-2)
//!   test_ci_gate_needs_jobs_have_no_job_level_if             → EC-002 (M1)
//!   test_ci_gate_pass_fail_semantics_are_structurally_placed → AC-001/AC-002 (M2, retargeted, S-CIGATE-2)
//!
//! Test coverage map (→ S-626-1 AC — ADV-P48-LOW-002, round 20: split out
//! into its own section. S-626-1 is a SEPARATE story from S-CIGATE-1/2
//! with its OWN AC numbering; round 19 added the two rows below directly
//! under the "S-CIGATE-1 AC" header above, which read as though `AC-10`
//! and `AC-3` belonged to S-CIGATE-1's AC list — they do not, and the two
//! schemes use different padding conventions by their own source stories
//! (`.factory/stories/S-CIGATE-1*.md` zero-pads to 3 digits; `AC-1`
//! .. `AC-10` in `.factory/stories/S-626-1.md` do not pad at all) — so
//! rows below are NOT renumbered/padded to match the table above; doing so
//! would misrepresent which story defines the AC):
//!   test_verify_test_job_has_zero_test_floor                 → AC-10 / BC-X.13.007
//!   test_verify_msrv_job_pins_toolchain_and_rustup_toolchain_env → AC-3
//!
//! ## ADV-P51 (adversarial pass 51) — guard-strength gaps in the `test`
//! ## job's POL-11 guard step, and one class-wide sweep
//!
//! Every anti-neutering control built for `ci-gate` over 20 review rounds
//! (a step-level `if:` ban, a `continue-on-error` ban, byte-for-byte
//! value pins, `env:` key-set pins, ordered per-step key sets) was never
//! propagated to the OTHER jobs in this file — above all `test`, the
//! `ci-gate.needs` member that carries the entire regression suite. New
//! test coverage (→ ADV-P51 finding ID):
//!   test_test_job_guard_step_key_set_and_env_are_pinned → HIGH-001/HIGH-002/MED-002
//!   test_test_job_pipefail_bracket_ordering_is_position_constrained → HIGH-003
//!   test_always_run_jobs_have_no_continue_on_error (class sweep, 7 jobs) → HIGH-002
//!   test_verify_test_job_has_zero_test_floor (per-branch `exit 1`, in place) → MED-001
//! LOW-001 (a dead-branch rationale in `ci.yml`'s POL-11 guard step,
//! fail-closed so not a false-green) was fixed as a `ci.yml` comment
//! correction — no new test, since there is nothing behavioral to pin (see
//! the corrected comment in `ci.yml :: test` immediately above the
//! `_canary_running_line=$(...)` assignment).
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
/// Returns `None` if no job-level `needs:` line is found in the block.
///
/// S-626-1 pass-55 (ADV-P55-HIGH-001): the original version of this
/// function applied `line.trim()` to EVERY line in `job_block` and matched
/// on `trimmed.strip_prefix("needs:")` / `trimmed == "needs:"` — with no
/// indentation check at all, a `needs:` key nested arbitrarily deep (e.g.
/// inside a step's `with:` block, or a decoy `env:` value) was
/// indistinguishable from the job's own job-level `needs:` key. Verified:
/// planting a decoy `needs: [<all ci-gate.needs members>]` inside the gate
/// step's `with:` block left the REAL `ci-gate.needs` (a genuinely
/// different, narrower set) invisible to this function — it returned the
/// decoy's full membership instead, defeating every one of this function's
/// six-plus callers from a single read, including
/// `test_ci_gate_needs_partitions_all_ci_yml_jobs` (the U1 fix). Fixed by
/// anchoring key detection to `extract_key_name_at_indent(line, 4)` — the
/// SAME quote/whitespace-aware, depth-exact matcher every other job-level
/// key check in this file already uses — so a nested decoy is invisible to
/// this function by construction, not by luck. A SECOND job-level `needs:`
/// key anywhere in the block is a hard `panic!`, mirroring
/// `extract_and_normalize_if_expr`'s refusal to silently pick a winner
/// between two job-level `if:` keys (also invalid YAML — a duplicate
/// mapping key GitHub Actions/actionlint reject at parse time — but this
/// checker does not rely on that external validation alone).
fn parse_needs_set(job_block: &str) -> Option<HashSet<String>> {
    let lines: Vec<&str> = job_block.lines().collect();
    let needs_line_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| extract_key_name_at_indent(l, 4).as_deref() == Some("needs"))
        .map(|(i, _)| i)
        .collect();

    if needs_line_indices.len() > 1 {
        panic!(
            "FAIL (S-626-1 pass-55, ADV-P55-HIGH-001): job block contains \
             {} job-level `needs:` keys at 4-space indent — this checker \
             refuses to silently pick one. This is ALSO invalid YAML (a \
             duplicate mapping key) that GitHub Actions and actionlint \
             both reject at parse time, but this checker will not rely on \
             that external validation alone.\n\
             Job block:\n{job_block}",
            needs_line_indices.len()
        );
    }

    let needs_line_idx = needs_line_indices.first().copied()?;

    // Try inline-array form first: `needs: [fmt, clippy, ...]`
    let trimmed = lines[needs_line_idx].trim();
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

    // Try block-list form: `needs:` on its own line, followed by `  - item`
    // lines, scanning ONLY from the single job-level `needs:` line found
    // above (never re-scanning the whole block for a bare `needs:` literal
    // at any depth).
    let mut set = HashSet::new();
    for line in &lines[needs_line_idx + 1..] {
        let trimmed = line.trim();
        if let Some(item) = trimmed.strip_prefix("- ") {
            set.insert(item.trim().to_string());
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            // Reached a non-list-item non-comment line — end of needs block.
            break;
        }
    }

    if !set.is_empty() { Some(set) } else { None }
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
/// Portable (NOT `#[cfg(unix)]`-gated): pure string parsing, no subprocess.
/// Originally gated to unix (PR #671 review round 15, CI-caught) because
/// its only caller at the time,
/// `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`, was
/// itself `#[cfg(unix)]`-gated (the bash shell-outs IT performs don't run
/// on Windows) — gating a TEST to unix does not remove the HELPERS it
/// alone uses from a Windows build, so this function stayed compiled-but-
/// unused there and `-D warnings` promoted that to a hard clippy error
/// (`cargo clippy` run 31128902318 caught exactly this on
/// `windows-latest`, six items including this one, after thirteen rounds
/// of macOS-only local review missed it). S-626-1 (U1) added a second,
/// portable caller — `test_ci_gate_needs_partitions_all_ci_yml_jobs` —
/// which does no subprocess work and runs on every platform, so the
/// `#[cfg(unix)]` gate no longer applies to this function itself (it is no
/// longer "only used by a unix-gated test"); the unix-only callers below
/// keep their own gate unchanged. NOTE for future readers: gating a test
/// still orphans whatever helpers ONLY it uses — always check for other
/// callers (like this one gained) before assuming a helper's gate should
/// simply mirror its original caller's.
fn list_all_ci_yml_job_names(ci: &str) -> Vec<String> {
    let Some(jobs_start) = ci.find("\njobs:\n") else {
        panic!("FAIL: `.github/workflows/ci.yml` has no top-level `jobs:` key.");
    };
    let jobs_section = &ci[jobs_start + 1..];
    // S-626-1 pass-55, ADV-P55-MED-002: per-job-id detection previously
    // required the line to END with `:` (`strip_prefix("  ").and_then(|s|
    // s.strip_suffix(':'))`) — a flow-style job entry (e.g. `gate: {name:
    // CI Gate, runs-on: ubuntu-latest}`) does not end with `:` at all and
    // was invisible to this scan, reopening the exact U1 partition gap
    // this function's own caller (`test_ci_gate_needs_partitions_all_
    // ci_yml_jobs`) exists to close, by formatting alone. Routed through
    // `collect_mapping_key_set` — the same quote/whitespace-aware,
    // comment-and-blank-line-tolerant primitive `extract_gate_env_key_set`
    // / `extract_workflow_env_key_set` already use for an identical
    // "collect this mapping's child key set" shape — rather than
    // reimplementing key detection a third time.
    let lines: Vec<&str> = jobs_section.lines().skip(1).collect();
    collect_mapping_key_set(&lines, 0, 2)
}

/// Cross-platform-safe (NOT `#[cfg(unix)]`-gated) list of `ci-gate.needs`
/// member job names that are legitimately permitted to report `skipped`.
///
/// This is a narrower, portable duplicate of the job-name half of
/// `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS` (which additionally pins each
/// job's exact `if:` expression text, byte-for-byte, and is
/// `#[cfg(unix)]`-gated because every one of its readers shells out to
/// bash). This constant exists so `always_run_needs_members` below — used
/// by tests that do NOT need bash and must run on every platform — has a
/// skip-tolerance list to filter against without depending on a
/// unix-only pin. Kept in sync manually; today both contain exactly
/// `["mutants"]`. Drift is caught by
/// `test_skip_tolerant_needs_members_matches_pinned_if_expressions`
/// (unix-only, since it reads the unix-gated constant) — see that test's
/// doc comment for what "drift" means here and why it can't be closed
/// portably.
const SKIP_TOLERANT_NEEDS_MEMBERS: &[&str] = &["mutants"];

/// Every `ci-gate.needs` member EXCEPT those in `SKIP_TOLERANT_NEEDS_MEMBERS`
/// — i.e. every job required to run unconditionally (no job-level `if:`
/// that could produce anything other than `success`/`failure`/`cancelled`)
/// on every push and PR.
///
/// S-626-1 sweep-to-class fix: `test_ci_gate_needs_jobs_have_no_job_level_if`
/// and `test_always_run_jobs_have_no_continue_on_error` each used to
/// hand-maintain their own literal 7-name `required_jobs` array — a sibling
/// instance of the same "allowlist not tied to the real universe" shape as
/// the S-626-1 U1 finding, one level down (within `ci-gate.needs`, rather
/// than across all of `ci.yml`): a job added to `ci-gate.needs` without
/// also being added to both hardcoded literals would silently escape both
/// checks. Deriving `required_jobs` from the LIVE `needs:` set here instead
/// means a newly-added always-run job is automatically covered by both
/// checks the moment it lands in `ci-gate.needs` — no second edit to
/// remember. Functionally identical to the two prior hardcoded lists for
/// today's real `ci.yml` (both were `{fmt, clippy, test, msrv, deny,
/// spec-guard, check-signing-workflow-injection}`, i.e. `ci-gate.needs`
/// minus `mutants`) — this is a strict widening of future coverage, never a
/// narrowing of today's.
fn always_run_needs_members(ci: &str) -> Vec<String> {
    let gate_block = extract_job_block(ci, "ci-gate").unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): `.github/workflows/ci.yml` does not contain a \
             `ci-gate:` job."
        )
    });
    let needs = parse_needs_set(gate_block).unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): The `ci-gate` job block does not contain a \
             `needs:` key.\n\
             Current ci-gate block:\n{gate_block}"
        )
    });
    let mut always_run: Vec<String> = needs
        .into_iter()
        .filter(|j| !SKIP_TOLERANT_NEEDS_MEMBERS.contains(&j.as_str()))
        .collect();
    always_run.sort();
    always_run
}

/// S-626-1 pass-59 (ADV-P57-HIGH-001 + ADV-P57-MED-001, shared fix — one
/// root cause, two silent-fail-open call sites): every hardcoded-indent
/// check in this file (`extract_key_name_at_indent(line, 4)` for job-level
/// keys, `extract_job_display_name`, `matrix_needs_members`'s `strategy:`
/// scan, ...) ASSUMES a job's direct children are indented exactly 4
/// spaces. That assumption is legal, `actionlint`-clean, PyYAML-valid YAML
/// only by CONVENTION — a job's own children need only be indented
/// CONSISTENTLY WITH EACH OTHER, not with any other job's indent choice
/// (the same acknowledged limitation `extract_key_name_at_indent`'s own
/// doc comment already names). Verified end-to-end (S-626-1 pass-59): a
/// sibling workflow job body written at 3-, 6-, or 8-space indent left
/// `extract_key_name_at_indent(l, 4)` returning `None` for EVERY line in
/// that job's block — not "this job declares no name"/"no strategy", but
/// "this checker looked at the wrong indent level entirely" — and the
/// 910b8ab0 class sweep's "key-detect vs. value-reparse swallow" fix does
/// not help here, because there is no KEY DETECTED to re-read a value
/// for: the miss happens one step earlier, at indent selection.
///
/// This function derives each job block's ACTUAL direct-child indent —
/// the indentation of the first non-blank, non-comment line after the
/// job-key line itself — and panics if it is not 4, rather than silently
/// certifying an unverifiable absence. `assert!`, not a `Result`: every
/// caller of this function is itself in a "reject, don't parse" context
/// (a `panic!`/`unwrap_or_else(|| panic!(...))` site), so a bool-returning
/// helper would just be unwrapped at the call site anyway.
fn assert_job_block_uses_4_space_child_indent(job_block: &str, caller: &str) {
    let Some(child_line) = job_block.lines().skip(1).find(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#')
    }) else {
        // No children at all (an empty job block, or a job block that is
        // ONLY the job-key line) — nothing to verify an indent assumption
        // against.
        return;
    };
    let indent = child_line.len() - child_line.trim_start().len();
    assert!(
        indent == 4,
        "FAIL ({caller}, S-626-1 pass-59, ADV-P57-HIGH-001/MED-001): this \
         job block's direct children are indented {indent} spaces, not \
         the 4-space indent every hardcoded-indent check in this file \
         assumes. This is legal, `actionlint`-clean, PyYAML-valid YAML (a \
         job's own children need only be indented CONSISTENTLY WITH EACH \
         OTHER, not with any other job's indent choice) — but every \
         `extract_key_name_at_indent(line, 4)` call against this job \
         block would silently see NO job-level keys at all: not \"this \
         job declares no name\"/\"no strategy\", but \"this checker looked \
         at the wrong indent level\". This checker refuses to silently \
         certify an unverifiable absence — investigate this job's indent \
         before relying on any indent-4 extraction against it.\n\
         First child line found: {child_line:?}\n\
         Job block:\n{job_block}"
    );
}

/// Every `ci-gate.needs` member whose job block declares a job-level
/// `strategy:` key (4-space indent) — i.e. every build-matrix job, derived
/// from the LIVE `needs:` set rather than a hand-maintained literal.
///
/// S-626-1 pass-56 (ADV-P56-LOW-002): Guard B's iteration list used to be
/// the closed-enumeration literal `["clippy", "test"]` — the exact
/// "allowlist not tied to the real universe" shape `always_run_needs_
/// members`'s own doc comment above condemns, one level down (within
/// `ci-gate.needs`'s matrix-job subset, rather than across all of
/// `ci-gate.needs`). A future matrix job added to `ci-gate.needs` without
/// also being added to that literal would silently escape Guard B
/// entirely — the same class the U1 finding closed one level up. Deriving
/// this list from the LIVE `needs:` set plus a `strategy:` presence check
/// means a newly-added matrix job is automatically covered the moment it
/// lands in `ci-gate.needs`, with no second edit to remember.
///
/// S-626-1 pass-59 (ADV-P57-LOW-002): the `extract_job_block(ci, job_id)`
/// miss branch used to silently `return false` — the exact silent-skip
/// shape this same commit replaces with a hard panic ~40 lines away in
/// Guard A's sibling-workflow check
/// (`test_no_sibling_workflow_declares_a_job_named_ci_gate`). A
/// `ci-gate.needs` member naming a job that does not actually exist under
/// `jobs:` is a broken configuration, not "not a matrix job" — panic
/// loudly instead, matching the Guard A precedent.
fn matrix_needs_members(ci: &str) -> Vec<String> {
    let gate_block = extract_job_block(ci, "ci-gate").unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): `.github/workflows/ci.yml` does not contain a \
             `ci-gate:` job."
        )
    });
    let needs = parse_needs_set(gate_block).unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): The `ci-gate` job block does not contain a \
             `needs:` key.\n\
             Current ci-gate block:\n{gate_block}"
        )
    });

    let mut matrix_jobs: Vec<String> = needs
        .into_iter()
        .filter(|job_id| {
            let job_block = extract_job_block(ci, job_id).unwrap_or_else(|| {
                panic!(
                    "FAIL (S-626-1 Guard B, ADV-P57-LOW-002): `ci-gate.needs` \
                     names job `{job_id}`, but no `{job_id}:` job block \
                     exists under `jobs:` in ci.yml. This checker refuses \
                     to silently treat a missing job as \"not a matrix \
                     job\" — the same silent-skip shape Guard A's \
                     sibling-workflow check \
                     (`test_no_sibling_workflow_declares_a_job_named_ci_gate`) \
                     already refuses via a hard panic. Investigate why \
                     `ci-gate.needs` names a job that does not exist."
                )
            });
            assert_job_block_uses_4_space_child_indent(
                job_block,
                "S-626-1 Guard B, ADV-P57-HIGH-001 (matrix_needs_members)",
            );
            job_block
                .lines()
                .any(|l| extract_key_name_at_indent(l, 4).as_deref() == Some("strategy"))
        })
        .collect();
    matrix_jobs.sort();
    matrix_jobs
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
/// Naming note (F-05): this test asserts `name:`, `runs-on:`, and the
/// job-level `if: always()` — it makes no assertion about a shell. It was
/// previously misnamed `..._with_correct_shell`; renamed to describe what it
/// actually verifies rather than adding an unrelated shell assertion.
///
/// RED GATE: `ci-gate` does not exist in ci.yml.  This test FAILS on develop.
#[test]
fn test_ci_gate_job_exists_with_required_metadata() {
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
// AC-003 — `ci-gate.needs` is exactly the required eight-job set
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
        // on push events — safe ONLY because `mutants` is named in
        // `scripts/check-ci-gate.sh`'s restrictive `ALLOWED_SKIPS` allowlist
        // (with a matching `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS` entry in
        // this file).  Since S-CIGATE-2 an unlisted job's `skipped` result
        // fails the gate by default (fail-closed) — `ALLOWED_SKIPS`
        // membership is the mechanism, not "ci-gate checks failure/cancelled
        // only" (that inline condition was retired).  See delta-analysis §5
        // and cargo-mutants-policy.md §CI Gate.
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
             If a new mandatory CI job was added here, verify it either runs \
             unconditionally (no job-level `if:` that can produce `skipped`) \
             or, if it legitimately can be skipped (e.g. PR-only), is added \
             to `scripts/check-ci-gate.sh`'s `ALLOWED_SKIPS` allowlist with \
             a matching `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS` entry in this \
             file.  Since S-CIGATE-2, an unlisted job's `skipped` result \
             fails the gate by default (fail-closed) rather than silently \
             passing it — so an unreviewed PR-only job here now surfaces as \
             a newly-red gate on every push, not a false-green.  Then update \
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
/// further gated by `vars.GITLEAKS_DISABLED`.  Including it would FAIL every
/// push-triggered `ci-gate` run: `security`'s designed-in `skipped` result on
/// push is not in `scripts/check-ci-gate.sh`'s `ALLOWED_SKIPS` allowlist, and
/// since S-CIGATE-2 an unlisted `skipped` fails the gate by default
/// (fail-closed) — not the pre-S-CIGATE-2 silent pass this docstring
/// previously implied.
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

    let needs = parse_needs_set(gate_block).unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): The `ci-gate` job block does not contain a \
             `needs:` key.\n\
             An empty/absent needs set would otherwise vacuously satisfy \
             both `!needs.contains(...)` assertions below (F-04) — panic \
             instead, matching every sibling test in this file.\n\
             Current ci-gate block:\n{gate_block}"
        )
    });

    // `security` is gated by `github.event_name == 'pull_request'` AND by
    // `vars.GITLEAKS_DISABLED` — emits `skipped` on push; must not be in needs.
    assert!(
        !needs.contains("security"),
        "FAIL: `security` must NOT be in `ci-gate.needs`.\n\
         The `security` job carries `if: github.event_name == 'pull_request'` \
         and emits `skipped` on push events.  Including it would FAIL every \
         push-triggered `ci-gate` run: `security`'s `skipped` result is not \
         in `scripts/check-ci-gate.sh`'s `ALLOWED_SKIPS` allowlist, and an \
         unlisted `skipped` fails the gate by default (fail-closed, since \
         S-CIGATE-2).  If `security` legitimately needed to gate merges, the \
         remedy would be adding it to `ALLOWED_SKIPS` with a matching \
         `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS` entry — but it is advisory by \
         design and should stay out of `needs` entirely.\n\
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
// S-626-1 U1 — `ci-gate.needs` must PARTITION every job in ci.yml
// ---------------------------------------------------------------------------

/// PINNED, human-reviewed set of `ci.yml` jobs that must NEVER gate merges —
/// the exclusion half of the S-626-1 U1 partition (see
/// `test_ci_gate_needs_partitions_all_ci_yml_jobs` immediately below).
///
/// Adding a job here is a deliberate, reviewed act (mirroring
/// `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS`'s convention for the opposite
/// carve-out): it is a claim that this job must NEVER be required to pass,
/// under any circumstances, for any future reason — not merely "it isn't
/// required today." Today's two members and their rationale (carried over
/// verbatim from `test_ci_gate_excludes_advisory_and_secret_scan_jobs`,
/// which predates this partition check and remains as a narrower,
/// faster-to-read diagnostic):
///   - `security`: PR-only (`if: github.event_name == 'pull_request'`) AND
///     further gated by `vars.GITLEAKS_DISABLED` — a secret scan that is
///     advisory by policy, not a merge blocker.
///   - `coverage`: uses `fail_ci_if_error: false` on the codecov upload —
///     advisory by design; a flaky coverage upload must not block merges.
const PINNED_GATE_EXCLUDED_JOBS: &[&str] = &["security", "coverage"];

/// S-626-1 U1 (external research finding): closes the "allowlist with no
/// default-deny over its universe" gap one level up from the fixes already
/// applied to `scripts/check-ci-gate.sh`'s per-job RESULT decision
/// (`ALLOWED_SKIPS` + a default-fail `case` arm — see that script's own
/// doc comment) and to this file's `if:`/step/env-key pins (`PINNED_GATE_*`,
/// all set-equality, default-deny). Until this test, NOTHING asserted the
/// partition at the NEEDS-SET level: `test_ci_gate_needs_exactly_the_required_jobs`
/// pins `ci-gate.needs` against a hardcoded 8-name literal, and
/// `test_ci_gate_excludes_advisory_and_secret_scan_jobs` denies exactly two
/// names (`security`, `coverage`) — between them, a NINTH job added to
/// `ci.yml` and left out of both `ci-gate.needs` and the exclusion check
/// was invisible to every existing assertion: both tests stayed green, the
/// new job was entirely unenforced by the sole required branch-protection
/// check, and nothing noticed. Same shape as the pre-S-CIGATE-2 defect this
/// story's sibling tests already fixed at the result-value layer, one
/// layer up: an allowlist of known-good members with no default-deny over
/// the universe it's drawn from.
///
/// THE FIX: every job actually defined in `ci.yml` (via
/// `list_all_ci_yml_job_names` — the same candidate-universe source
/// `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`
/// already trusts for its own pin-coverage check) must be EITHER a member
/// of `ci-gate.needs` OR named in the pinned, human-reviewed
/// `PINNED_GATE_EXCLUDED_JOBS` literal above — a job satisfying neither is
/// a maintainer's un-reviewed silent gap and fails this test by name,
/// forcing an explicit choice (add to `needs`, or add to
/// `PINNED_GATE_EXCLUDED_JOBS` with a rationale) rather than an implicit
/// one made by omission.
///
/// `ci-gate` itself is excluded from the partition explicitly (a job
/// cannot depend on itself; GitHub Actions would reject a self-referencing
/// `needs:` at workflow-parse time regardless) — handled by name, not by
/// falling through the "neither in needs nor excluded" branch by accident.
///
/// A job present in BOTH `ci-gate.needs` and `PINNED_GATE_EXCLUDED_JOBS`
/// simultaneously is also a failure: those two are meant to partition the
/// universe (every job is in exactly one), and a job claimed by both is a
/// contradiction a human must resolve, not something this test should
/// silently prefer one interpretation of.
///
/// `list_all_ci_yml_job_names` was previously `#[cfg(unix)]`-gated because
/// its only caller shelled out to bash on a unix-only test. This test adds
/// a second, portable caller (no subprocess, pure string parsing), so the
/// `#[cfg(unix)]` gate was removed from the helper — see its doc comment.
///
/// RED PROOF (S-626-1): a dummy job added to `ci.yml` in neither
/// `ci-gate.needs` nor `PINNED_GATE_EXCLUDED_JOBS` makes this test FAIL,
/// naming the offending job (verified manually during implementation and
/// reverted via `git checkout HEAD -- .github/workflows/ci.yml` — not
/// left as a fixture, since a real edit to the tracked file is the only
/// way to prove this without a second, out-of-sync copy of ci.yml).
#[test]
fn test_ci_gate_needs_partitions_all_ci_yml_jobs() {
    let ci = read_ci_yml();
    let gate_block = extract_job_block(&ci, "ci-gate").unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): `.github/workflows/ci.yml` does not contain a \
             `ci-gate:` job.\n\
             Required: append the `ci-gate` aggregator job to ci.yml per \
             S-CIGATE-1 AC-001 / AC-003."
        )
    });
    let needs = parse_needs_set(gate_block).unwrap_or_else(|| {
        panic!(
            "FAIL (RED GATE): The `ci-gate` job block does not contain a \
             `needs:` key.\n\
             Current ci-gate block:\n{gate_block}"
        )
    });

    let all_jobs = list_all_ci_yml_job_names(&ci);
    assert!(
        !all_jobs.is_empty(),
        "FAIL: `list_all_ci_yml_job_names` returned no jobs at all — the \
         `jobs:` key scan is broken (or ci.yml itself is malformed), which \
         would make this test vacuously pass every job it never saw. \
         Current ci.yml jobs section could not be parsed."
    );

    let mut both: Vec<&str> = Vec::new();
    let mut unaccounted: Vec<&str> = Vec::new();
    for job in &all_jobs {
        if job == "ci-gate" {
            // `ci-gate` cannot depend on itself — explicitly excluded from
            // the partition rather than falling through either branch below
            // by accident (S-626-1 mandate: "handle that explicitly rather
            // than by accident").
            continue;
        }
        let in_needs = needs.contains(job);
        let in_excluded = PINNED_GATE_EXCLUDED_JOBS.contains(&job.as_str());
        match (in_needs, in_excluded) {
            (true, true) => both.push(job.as_str()),
            (false, false) => unaccounted.push(job.as_str()),
            _ => {}
        }
    }
    both.sort_unstable();
    unaccounted.sort_unstable();

    assert!(
        both.is_empty(),
        "FAIL: the following ci.yml job(s) are claimed by BOTH \
         `ci-gate.needs` AND `PINNED_GATE_EXCLUDED_JOBS` at once — these \
         two are meant to partition every job in ci.yml (each job in \
         exactly one), so a job in both is a contradiction: {both:?}\n\
         Fix: remove it from whichever side is wrong."
    );

    assert!(
        unaccounted.is_empty(),
        "FAIL (S-626-1 U1): the following ci.yml job(s) are neither in \
         `ci-gate.needs` NOR in the pinned exclusion list \
         `PINNED_GATE_EXCLUDED_JOBS` ({PINNED_GATE_EXCLUDED_JOBS:?}): \
         {unaccounted:?}\n\
         Every job defined in ci.yml must be a deliberate, reviewed \
         choice: either\n\
         \n\
         1. add it to `ci-gate.needs` (and, if it can legitimately report \
            `skipped`, to `scripts/check-ci-gate.sh`'s `ALLOWED_SKIPS` \
            allowlist with a matching `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS` \
            entry in this file — see `test_ci_gate_needs_exactly_the_required_jobs` \
            for the exact-set pin to update in the same change), or\n\
         2. add it to `PINNED_GATE_EXCLUDED_JOBS` above with an in-code \
            rationale for why it must never gate merges.\n\
         \n\
         Leaving a new job out of both silently drops it from enforcement \
         by the sole required branch-protection check — this test exists \
         so that omission fails loudly instead."
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
// M1 — needs jobs must run unconditionally (no job-level if: key at all)
// ---------------------------------------------------------------------------

/// M1: For each unconditionally-running job listed in `ci-gate.needs`, assert
/// that the job's block contains NO job-level `if:` key at all.
///
/// Rationale: the existing exact-set test (`test_ci_gate_needs_exactly_the_required_jobs`)
/// pins WHICH jobs are in `needs`, but not that those jobs run unconditionally.
/// If a future maintainer adds a job-level `if:` guard to e.g. `deny` — for
/// ANY condition, not just `github.event_name` — a false condition makes the
/// job report `skipped`. Since S-CIGATE-2, `scripts/check-ci-gate.sh`'s
/// fail-closed `evaluate_needs()` means an unlisted job's `skipped` result
/// correctly FAILS the gate rather than silently passing it — so the
/// worst-case consequence of this drift is no longer a false-green merge,
/// but a newly-red gate that surprises a maintainer at review/CI time rather
/// than at design time. This test still exists to catch the drift at review
/// time, before that surprise happens: the required property is "no
/// job-level `if:` key" on these unconditionally-run jobs — not merely "no
/// `github.event_name`-referencing job-level `if:` key" — because a job
/// wrongly reporting `skipped` here also isn't automatically covered by
/// `scripts/check-ci-gate.sh`'s `ALLOWED_SKIPS` allowlist (which requires an
/// explicit, human-reviewed opt-in per job).
///
/// F-03 (round 19): the prior version of this test matched only single-line
/// `    if:` lines containing the literal substring `github.event_name`. Two
/// escapes survived that: (a) a folded/block scalar (`if: >-` with the
/// expression on continuation lines — a shape YAML permits on any job-level
/// or step-level `if:`; `ci-gate` itself has no step-level `if:` at all —
/// see M2-d below, which asserts exactly that) moves the `github.event_name`
/// substring (or any other condition) off the `if:` line itself; (b) any non-event
/// conditional (`if: false`, `github.ref == 'refs/heads/main'`,
/// `vars.X != 'true'`) never contains `github.event_name` in the first place.
/// Both produce a `skipped` result exactly as hazardous as the
/// `github.event_name` case. Matching on presence of the `if:` KEY at
/// job-property indent — regardless of the condition's shape or content —
/// closes both escapes, because a folded scalar's opening line still starts
/// with `    if:` even though its condition text lives on continuation lines.
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
///
/// ADV-P48-LOW-001 (round 20): renamed from
/// `test_ci_gate_needs_jobs_have_no_event_conditional_if`. Round 19's F-03
/// fix broadened the predicate from "no job-level `if:` referencing
/// `github.event_name`" to "no job-level `if:` key at all" (see the F-03
/// docstring above), but the function name was left unchanged — a
/// maintainer reading only the name could reasonably judge the check
/// over-reaching for its documented purpose and re-narrow it back to an
/// event-conditional-only match, silently reopening the two escapes F-03
/// closed. The name now states the actual predicate.
///
/// S-626-1 pass-55 (ADV-P55-LOW-001): "no job-level `if:` key at all"
/// above describes VALUE-content independence (this check does not care
/// what the condition says, only that the key exists) — that claim was
/// always accurate. What was NOT fully accurate, here and in the
/// corresponding `CLAUDE.md` sentence, was KEY-SPELLING coverage: the
/// detection itself was a bare `line.starts_with("    if:")`, matching
/// only that one literal spelling and missing `"if":`, `'if':`, and
/// `if :` (space before colon) — all PyYAML-identical to the bare
/// spelling. Fixed by routing detection through
/// `extract_key_name_at_indent`, the same quote/whitespace-aware matcher
/// used everywhere else in this file.
#[test]
fn test_ci_gate_needs_jobs_have_no_job_level_if() {
    let ci = read_ci_yml();

    // Every `ci-gate.needs` member that must run unconditionally on every
    // push and PR — derived from the live `needs:` set (S-626-1 sweep-to-
    // class fix; see `always_run_needs_members`'s doc comment), not a
    // hand-maintained literal. `mutants` is excluded — it is PR-only by
    // design and emits `skipped` on push events (ci-gate-safe; see test
    // docstring above).
    let required_jobs = always_run_needs_members(&ci);

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
        // We detect a job-level if: KEY via `extract_key_name_at_indent`
        // (S-626-1 pass-55, ADV-P55-LOW-001) — deliberately NOT filtering
        // on the condition's content or shape (see F-03 docstring above):
        // any job-level `if:` key on these seven jobs is hazardous, whether
        // it's a single-line condition, a folded/block scalar, or references
        // something other than `github.event_name`. A bare `starts_with("
        // if:")` used to match ONLY that one spelling, missing `"if":`,
        // `'if':`, and `if :` (space before colon) — all PyYAML-identical
        // to the bare spelling, and all recognized by
        // `extract_key_name_at_indent` already.
        for line in job_block.lines() {
            // Match lines at job-property indent (4 spaces, not 8+).
            if extract_key_name_at_indent(line, 4).as_deref() == Some("if") {
                panic!(
                    "FAIL (M1/F-03): Job `{job_name}` has a job-level `if:` \
                     key:\n\
                     \n  {line}\n\
                     \n\
                     Any job-level `if:` on this job is hazardous regardless \
                     of the condition's shape or content: a false condition \
                     makes `{job_name}` report `skipped` (not `failure`). \
                     Since `{job_name}` is not in `scripts/check-ci-gate.sh`'s \
                     `ALLOWED_SKIPS` allowlist, its fail-closed `evaluate_needs()` \
                     will correctly FAIL the gate on that skip — this is no \
                     longer a false-green — but only at CI time, surprising a \
                     maintainer at review time who did not expect this job to \
                     ever report anything but `success`/`failure`.\n\
                     \n\
                     Fix: either remove the job-level `if:` guard from \
                     `{job_name}` and use a step-level `if:` instead, or, if \
                     the skip is legitimate, add `{job_name}` to \
                     `ALLOWED_SKIPS` in `scripts/check-ci-gate.sh` with a \
                     matching entry in this file's \
                     `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS`, or remove \
                     `{job_name}` from `ci-gate.needs` and update this test \
                     accordingly."
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
    // indented 4 spaces from the left margin (GitHub Actions YAML
    // convention) — detected via `extract_key_name_at_indent`, the same
    // quote/whitespace-aware matcher used throughout this file, rather
    // than a raw `line.starts_with("    if:")`.
    //
    // S-626-1 pass-59 (ADV-P58-LOW-003): the prior manual scan (a
    // hand-rolled `in_steps` flag plus `line.starts_with("    if:")`) was
    // absence-shaped and fail-open two ways: (1) a quoted key spelling
    // (`"if":`/`'if':`) or `if :` (space before colon) — all forms
    // `extract_key_name_at_indent` already recognizes elsewhere in this
    // file — made this scan report "no job-level `if:` line" even though
    // one plainly exists: a MISDIAGNOSIS, since M2-m below (which already
    // routes through `extract_and_normalize_if_expr`'s quote-aware
    // detection) would have found and evaluated it correctly, but M2-a
    // fires FIRST and its wrong diagnosis masked M2-m's accurate one
    // before this test could ever reach it; (2) the `in_steps` bookkeeping
    // was needed only because a raw `starts_with` has no notion of indent
    // depth on its own — an exact-indent matcher removes the need for it
    // entirely, since a step-level `if:` (8-space indent) never satisfies
    // `extract_key_name_at_indent(line, 4)` regardless of whether the scan
    // has "entered steps:" yet.
    // -----------------------------------------------------------------------
    let job_if_line = gate_block
        .lines()
        .find(|l| extract_key_name_at_indent(l, 4).as_deref() == Some("if"));

    assert!(
        job_if_line.is_some(),
        "FAIL (M2-a): The `ci-gate` job block has no job-level `if:` key \
         at 4-space indent (checked via the same quote/whitespace-aware \
         matcher used throughout this file — `if:`, `\"if\":`, `'if':`, \
         and `if :` are all recognized, so this is not merely a bare-\
         spelling presence check).\n\
         Required: `    if: ${{{{ always() }}}}` so that ci-gate runs even when \
         upstream jobs fail.\n\
         Current ci-gate block:\n{gate_block}"
    );
    let job_if_line = job_if_line.unwrap_or_default();

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
         `contains(needs` — this is the retired inline condition \
         S-CIGATE-2 replaced, not merely a misplaced one.\n\
         Found:    {job_if_line}\n\
         Under the shipped fail-closed design, the pass/fail decision does \
         NOT live in any `if:` expression at all — not job-level, and \
         (per M2-d below) not step-level either. It lives entirely inside \
         `scripts/check-ci-gate.sh`'s `evaluate_needs()`, invoked from an \
         UNCONDITIONAL `run:` step whose own exit code is the sole \
         pass/fail signal (see M2-i/M2-h below). The job-level `if:` must \
         be exactly `always()` and nothing else — its only job is to make \
         `ci-gate` run even when upstream jobs fail, not to evaluate \
         `needs.*.result` itself. Do NOT move `contains(needs…)` to a \
         step-level `if:` — M2-d fails the suite if any step-level `if:` \
         exists at all, precisely to prevent reopening the `skipped` \
         false-green this whole story closed.\n\
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
    //
    // S-626-1 pass-59 (ADV-P58-LOW-003): routed through
    // `extract_key_name_at_indent(l, 8)` rather than a raw
    // `l.starts_with("        if:")`, for the same quote-awareness reason
    // as M2-a above (`"if":`/`'if':`/`if :` are all recognized).
    // -----------------------------------------------------------------------
    let has_step_level_if = gate_block
        .lines()
        .any(|l| extract_key_name_at_indent(l, 8).as_deref() == Some("if"));

    assert!(
        !has_step_level_if,
        "FAIL (M2-d, S-CIGATE-2): The `ci-gate` job block contains a \
         step-level `if:` key at 8-space indent (checked via the same \
         quote/whitespace-aware matcher used throughout this file). Under \
         Option C, `scripts/check-ci-gate.sh` is invoked unconditionally \
         and its own exit code IS the pass/fail signal — no step-level \
         `if:` should gate that invocation (a reintroduced `if:` here \
         would mean some upstream results never even reach the script). \
         NOTE: M2-l below (the per-step COMPLETE key-set pin,\
         `PINNED_GATE_STEP_KEY_SETS`) is the OPERATIVE default-deny for a \
         step-level `if:` in ANY form — even a spelling this presence \
         check does not recognize would still surface there as an \
         unexpected `if` key on that step; this assertion is a faster, \
         more specific diagnostic, not the sole backstop.\n\
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

    // -----------------------------------------------------------------------
    // Assertion 11 (M2-p, S-626-1 pass-55, ADV-P55-HIGH-001 part (c)): every
    // other decision-path text line in this job (the run line, the
    // NEEDS_JSON payload source, the job's own `if:`) is byte-pinned — the
    // job's `needs:` line was the one remaining exception, covered only by
    // `parse_needs_set`'s SET-membership extraction (now depth-anchored
    // and duplicate-key-safe, see that function's own doc comment) with no
    // pin on the line's own TEXT. Pin it the same way, closing the last
    // gap in this run of byte-for-byte decision-path pins.
    // -----------------------------------------------------------------------
    let actual_needs_line =
        extract_and_normalize_sole_needs_line(gate_block).unwrap_or_else(|reason| {
            panic!(
                "FAIL (M2-p, S-626-1 pass-55, ADV-P55-HIGH-001): the \
                 `ci-gate` job's `needs:` line {reason}\n\
                 Current ci-gate block:\n{gate_block}"
            )
        });

    assert_eq!(
        actual_needs_line, PINNED_GATE_NEEDS_LINE,
        "FAIL (M2-p, S-626-1 pass-55, ADV-P55-HIGH-001): the `ci-gate` \
         job's `needs:` value (\"{actual_needs_line}\") does not \
         byte-match the pinned, human-reviewed literal \
         (\"{PINNED_GATE_NEEDS_LINE}\"). `needs:` is the entire membership \
         list `scripts/check-ci-gate.sh` evaluates via `toJSON(needs)` — \
         every set-based pin in this file (M2-k's job key set included) \
         still ultimately depends on this one text line. If this is a \
         deliberate, reviewed change to `ci-gate.needs`, update BOTH \
         PINNED_GATE_NEEDS_LINE here AND `test_ci_gate_needs_exactly_the_ \
         required_jobs`'s exact-set pin in the SAME change.\n\
         Current ci-gate block:\n{gate_block}"
    );

    // -----------------------------------------------------------------------
    // ADV-P48-LOW-003 (round 20): re-reviewed this note on its merits per
    // the round-20 fix-round instructions. Content verdict: ACCURATE — it
    // correctly states M2-i's `PINNED_GATE_RUN_LINE` byte-for-byte pin
    // strictly subsumes F-01's narrower `run: exit 1` literal pin (both
    // are exact-match checks; M2-i additionally catches `|| true`/`| cat`
    // suffix-tolerance mutants F-01 could not), and it does not leave a
    // reader thinking coverage was dropped. One framing gap WAS found and
    // is fixed by this heading: the note previously called itself
    // "Assertion 4 (F-01, round 19)", the exact same ordinal already used
    // by the REAL, code-bearing Assertion 4 (M2-i) above at this
    // function's start — a documentation-only block with no `assert!` of
    // its own reusing a live assertion's number reads as though a second,
    // separate numbered check exists here. It doesn't; this block adds no
    // assertion. Retitled below to make that unambiguous.
    //
    // Historical note — F-01 (round 19), SUPERSEDED by S-CIGATE-2, not
    // reinstated (reconciliation note, PR #667 x #671 merge). NOT a
    // numbered assertion — no `assert!` follows this comment block; it
    // exists purely so a future reader doesn't mistake F-01's absence for
    // a dropped regression guard.
    //
    // F-01 originally pinned the `ci-gate` job's `run:` step body to the
    // exact literal `run: exit 1`, guarding against the same class of
    // regression M2-g's substring check missed: a body that can never fail
    // (e.g. `run: echo "gate disabled"`) would make the single required
    // branch-protection check permanently green regardless of upstream
    // results, even with every other structural assertion above still
    // passing.
    //
    // Under Option C (S-CIGATE-2), the gate's `run:` step is no longer a
    // bare `exit 1` — it is the `scripts/check-ci-gate.sh` invocation
    // (`echo "${{NEEDS_JSON}}" | bash scripts/check-ci-gate.sh`, see the M2
    // module doc comment's "correct shape" block above), so F-01's literal
    // `run: exit 1` pin is no longer true of the shipped, authoritative
    // ci.yml and reinstating it verbatim would make this test permanently
    // RED against correct code. F-01's actual goal — prevent an
    // unenforceable/no-op gate body — is already met, and more strongly:
    // Assertion 4 above (M2-i) pins the run line BYTE-FOR-BYTE against
    // `PINNED_GATE_RUN_LINE`, which rejects `run: echo "gate disabled"` (or
    // any other body that isn't the exact pinned invocation) exactly as
    // F-01 intended, while also catching the narrower `|| true` / `| cat`
    // suffix-tolerance mutants F-01 itself could not have caught. No
    // separate `exit 1`-literal assertion is reinstated here.
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
/// inert against the defect class it claims to catch.  Four gates are
/// therefore required (promoted from an earlier three-gate framing that kept
/// the canary's passed-count check nested as "2b" — `ci.yml`'s own POL-11
/// comment header and this docstring now agree at four top-level gates):
///   (1) Binary-count floor (`< 90`): catches mass orphaning of tests/ files.
///       At head ~103 binaries run; orphaning all integration targets drops
///       this below 90.  The threshold tolerates ~13 legitimate reductions.
///   (2) Named canary: asserts `tests/ci_gate_completeness` ran, catching the
///       self-orphaning case where the guard binary itself stops running —
///       even when the binary count stays above 90.
///   (3) Named-canary passed-count gate (round 20, ADV-P50-LOW-002): gate
///       (2) alone proves only that the binary was INVOKED — cargo prints
///       its "Running ..." line before running anything inside it, so the
///       substring is present even if every test is `#[ignore]`d or an
///       env-gate skips the whole suite before any assertion runs.  Round 20
///       locates the canary's own "test result:" line (scoped forward from
///       its "Running" line) and requires a non-zero passed count
///       specifically from it — closing the same fail-open shape as gate
///       (4) below, but scoped to the one binary carrying every CI-gate
///       regression pin.  The "Running" lookup is path-separator-agnostic
///       (`[/\\]` character class) because cargo prints a forward slash on
///       Unix runners but a backslash on `windows-latest`; a forward-slash-
///       only pattern silently fails to match on Windows, hardcoding this
///       gate's passed count to 0 and failing every Windows run
///       unconditionally regardless of whether the canary ran (CI run
///       31182605820 on `424d64de`, fixed in `177b3727`).
///   (4) Zero-test floor (`"${total}" -eq 0`): catches the case where ≥90
///       binaries (including the canary) report results but zero tests
///       passed within them — e.g. a global test filter matching nothing.
///       Neither (1) nor (2) nor (3) detects this scenario: the binary
///       count and the named binary's presence and passed-count are all
///       satisfied; only the whole-suite passed-count check catches it.
///
/// A fifth mechanism, orthogonal to all four gates above, has to hold for
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
/// are to defeat without also breaking the enforcement logic itself.
/// Eighteen assertions total (twelve added through round 17: `shell: bash`
/// and the full `cargo test` capture invocation were added in a later pass;
/// three more added in round 20 — see gate (2b) above — for the
/// named-canary passed-count gate and its path-separator-agnostic lookup;
/// net three more added by ADV-P51-MED-001, which replaced the single
/// generic `exit 1` presence check with four PER-BRANCH pins — see the
/// updated "exit 1" bullet below):
///   - **Variable/command-bound** (hardest to defeat: a rename or rewrite
///     that neuters the check also breaks the literal text this assertion
///     requires): `"${binaries}" -lt 90`, `"${total}" -eq 0`,
///     `grep -q "ci_gate_completeness"`, `tail -n +"${_canary_running_line}"`,
///     `"${_canary_passed}" -eq 0`.  None of these five forms currently
///     appears anywhere else in `ci.yml` (verified) — a bare
///     `-lt 90` / `-eq 0` / `ci_gate_completeness` substring, by contrast,
///     also appears in this guard's own comments and echo diagnostics and
///     would be satisfied even after the check was neutered by a variable
///     rename.
///   - **Exact quoted-literal, appears once** (comment-satisfiable only by a
///     future comment reproducing the identical quoted form — no such
///     comment exists today): the path-separator-agnostic regex
///     `Running tests[/\\]ci_gate_completeness\.rs` (gate (2b)'s Windows
///     fix; pinned via a Rust raw string literal so the source reproduces
///     the exact backslash bytes from `ci.yml` without additional escaping).
///   - **Exact standalone line** (comment-satisfiable only by a future
///     comment reproducing the identical trailing form — no such comment
///     exists today): `set -euo pipefail\n`, `set +o pipefail\n`,
///     `set -o pipefail\n`, and the full capture invocation
///     `cargo test --all-features 2>&1 | tee "$RUNNER_TEMP/cargo_test_out.txt"\n`.
///     `"set -o pipefail\n"` is NOT a substring of
///     `"set -euo pipefail\n"` (the characters after `set -` are `euo`, not
///     `o`), so these three assertions are independent of one another —
///     dropping any one of the three lines fails exactly one assertion, not
///     all three.  This is incidental to the current comment wording, not a
///     structural guarantee: a future comment line ending exactly with one
///     of these forms (no trailing text) immediately before the newline
///     would also satisfy the corresponding assertion.  The capture
///     invocation is the literal `run:` command itself (not a `with:`/`env:`
///     key-value pair, and not variable-bound like the tier above) — it
///     occurs exactly once in the whole of `ci.yml` (verified) and is not
///     reproduced by any comment today, so it sits in this tier rather than
///     the weaker one below, even though nothing prevents a future comment
///     from quoting it verbatim.
///   - **Literal substring, weaker still** (a comment or unrelated step
///     could in principle reproduce it): `CARGO_TERM_COLOR: never`,
///     `shell: bash`.  Both are step-level YAML key-value pairs; `shell:
///     bash` occurs exactly once in the whole of `ci.yml` (verified) today,
///     but nothing prevents a future comment from reproducing the string.
///   - **Weakest** (also appears inside this guard's own `echo`
///     diagnostics; a rewrite that preserves the diagnostic strings while
///     gutting the enforcement logic underneath would not be caught by
///     these alone): `FAIL (POL-11)`, `Check passed:`.
///   - **Per-branch, scoped** (ADV-P51-MED-001): `exit 1` moved OUT of the
///     "weakest" tier above — a bare `exit 1` presence check is satisfied
///     by ANY ONE of the four gate branches retaining it, so mutating a
///     single branch's `exit 1` to `exit 0` (that branch prints its FAIL
///     diagnostic, then exits the STEP successfully, skipping the
///     remaining three gates) went undetected by the old generic check.
///     `extract_if_block` locates each branch's own `if ... then ... fi`
///     block by its unique condition line and asserts `exit 1` appears
///     INSIDE that block specifically, so each of the four gates is now
///     independently pinned.
///   - **NOT PINNED — no assertion covers these today:** the `total=`/
///     `binaries=` computation pipelines (the `grep`/`grep -Eo`/`awk` and
///     `grep`/`wc -l`/`tr` chains) that produce the values the gates above
///     test — only their *usages* (`"${total}" -eq 0`,
///     `"${binaries}" -lt 90`) are pinned, so a rewrite of the computation
///     logic that leaves those two variables holding a wrong-but-passing
///     value is undetected as long as the variable names survive.  (The
///     capture invocation itself — `--all-features` and the `tee` target —
///     moved out of this bucket into the "Exact standalone line" tier above;
///     these computation pipelines remain structurally unpinnable by
///     substring matching, since a wrong-but-passing rewrite can preserve
///     every substring this test could reasonably assert on.)
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
    // Asserting on its PRESENCE means DELETING the guard's diagnostic text
    // entirely fails this test.
    //
    // ADV-P51-HIGH-001 (correction): a prior version of this comment
    // claimed "removing the guard entirely fails this test" without
    // qualification — that reads as though this one instrument covers
    // every way the guard can stop being enforced. It does not: `if:
    // false` (HIGH-1) or `continue-on-error: true` (HIGH-2) on the step
    // SKIPS it (or neutralizes its exit code) without touching a single
    // byte of this diagnostic text, so Instrument 0 alone stays green
    // under either attack — verified directly (RED proof, S-626-1
    // ADV-P51 fix commit). Those two attacks are closed by a SEPARATE,
    // dedicated test, `test_test_job_guard_step_key_set_and_env_are_pinned`
    // below, which pins this step's COMPLETE key set (no `if:`, no
    // `continue-on-error:` beyond the reviewed `env`/`name`/`run`/`shell`
    // set) the same way `PINNED_GATE_STEP_KEY_SETS` already does for
    // `ci-gate`'s own steps. Instrument 0 here narrowly proves what its
    // assertion actually checks: the diagnostic TEXT survives.
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

    // --- Instrument 2b: named-canary passed-count gate (round 20 strengthening) ---
    // ADV-P50-LOW-002 (round 20): `grep -q "ci_gate_completeness"` above
    // proves only that the binary was INVOKED — cargo prints its
    // "Running tests/ci_gate_completeness.rs (...)" line before running
    // anything inside it, so the substring is present even if every test in
    // the file is `#[ignore]`d or an env-gate skips the whole suite before
    // any assertion runs.  That leaves Instrument 2 "satisfied" at 0 passed —
    // the exact fail-open shape POL-11 exists to close for the whole-suite
    // floor, just scoped to this one binary.  Round 20 strengthens this by
    // locating the canary binary's own "test result:" summary line (scoping
    // forward from its "Running" line, not the first "test result:" line in
    // the whole capture — a global grep here would find some *other*
    // binary's result line and could pass even if the canary itself reported
    // 0) and requiring a non-zero passed count specifically from it.
    //
    // Two sub-assertions, mirroring why Instrument 1/3 pin the
    // variable-bound form rather than a bare `-eq 0`/`-lt 90`: a rename or a
    // rewrite that neuters the check also breaks the literal text these
    // assertions require.
    //
    // (a) `tail -n +"${_canary_running_line}"` proves the result line is
    //     scoped to the located binary's own output, not merely "the first
    //     test result: line anywhere in the file" (which is what the
    //     pre-round-20 `grep -q` presence check was blind to).  This exact
    //     form appears once in `ci.yml` today (verified).
    assert!(
        test_block.contains("tail -n +\"${_canary_running_line}\""),
        "FAIL (POL-11): The `test` job step does not scope the canary's \
         `test result:` line lookup to `tail -n +\"${{_canary_running_line}}\"`.\n\
         Required: round 20 strengthened the named-canary check (ADV-P50-LOW-002) \
         to read the RESULT LINE THAT BELONGS TO THE CANARY BINARY ITSELF, not \
         just the first `test result:` line anywhere in the capture — without \
         this scoping a global grep could pass on some other binary's result \
         while the canary itself reported 0 passed.\n\
         Current test job block:\n{test_block}"
    );
    // (b) `"${_canary_passed}" -eq 0` is the gate itself: it fails the step
    //     when the canary's own scoped result line shows 0 passed — the
    //     property that closes ADV-P50-LOW-002 (a canary that was invoked
    //     but never actually ran an assertion, e.g. every test `#[ignore]`d
    //     or an env-gate early-return).  Reverting this whole Instrument 2b
    //     block to the pre-round-20 form (a bare `grep -q
    //     "ci_gate_completeness"` presence check with no passed-count gate)
    //     removes this string entirely — proven below (RED proof).  This
    //     exact form appears once in `ci.yml` today (verified).
    assert!(
        test_block.contains("\"${_canary_passed}\" -eq 0"),
        "FAIL (POL-11): The `test` job step does not gate on \
         `\"${{_canary_passed}}\" -eq 0`.\n\
         Required: round 20 (ADV-P50-LOW-002) strengthened the named-canary \
         check from proving the binary was merely INVOKED to proving it \
         actually EXECUTED at least one passing assertion — reverting to a \
         bare `grep -q \"ci_gate_completeness\"` presence check reopens the \
         exact fail-open shape POL-11 exists to close, scoped to the one \
         binary carrying every CI-gate regression pin.\n\
         Current test job block:\n{test_block}"
    );

    // --- Instrument 2c: path-separator-agnostic canary lookup (round 20 Windows fix) ---
    // `cargo test` prints "Running tests/ci_gate_completeness.rs" on Unix
    // runners but "Running tests\ci_gate_completeness.rs" (backslash) on
    // `windows-latest` — the `test` job is a 3-OS matrix.  A forward-slash-only
    // regex here silently finds no match on Windows, so `_canary_running_line`
    // is always empty there and `_canary_passed` is hardcoded to `0` by the
    // `if [ -z "${_canary_running_line}" ]` branch — every Windows run of the
    // `test` job would fail this gate unconditionally, regardless of whether
    // the canary actually ran.  This is exactly what broke in CI run
    // 31182605820 on commit `424d64de` and was fixed in `177b3727` by
    // widening the regex to a `[/\\\\]` character class matching either
    // separator.  Reproduced directly (RED proof below): hardcoding the
    // regex back to a bare forward slash reintroduces the Windows false-red
    // while leaving Instrument 2b's assertions untouched (they pin the
    // `_canary_passed`/`tail` machinery around the regex, not the regex
    // pattern itself) — demonstrating this assertion is independent of, not
    // a duplicate of, Instrument 2b above.
    //
    // Pinned via a raw string literal so the Rust source reproduces the
    // exact byte sequence from `ci.yml` (four literal backslashes inside the
    // character class, one literal backslash before the escaped dot)
    // without the extra escaping a normal string literal would require —
    // this exact quoted regex appears once in `ci.yml` today (verified).
    assert!(
        test_block.contains(r#"Running tests[/\\\\]ci_gate_completeness\.rs"#),
        "FAIL (POL-11): The `test` job step does not use the path-separator- \
         agnostic regex `Running tests[/\\\\]ci_gate_completeness\\.rs` to \
         locate the canary binary's \"Running\" line.\n\
         Required: cargo prints `Running tests/ci_gate_completeness.rs` on \
         Unix runners but `Running tests\\ci_gate_completeness.rs` (backslash) \
         on windows-latest. A forward-slash-only pattern here silently finds \
         no match on Windows, hardcoding `_canary_passed` to 0 and failing \
         every Windows run of the `test` job unconditionally (CI run \
         31182605820 on 424d64de) — this is the \
         LOCAL-VERIFICATION-MISSES-PLATFORM-MATRIX class documented in \
         CLAUDE.md.\n\
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

    // --- exit 1 is present, PER BRANCH (ADV-P51-MED-001) ---
    // A single unqualified `contains("exit 1")` check (the pre-ADV-P51 form
    // of this instrument) is satisfied by ANY ONE of the four gate
    // branches below having its own `exit 1` — changing exactly one
    // branch's `exit 1` to `exit 0` (e.g. the binary-count floor's) makes
    // that branch print its `FAIL (POL-11)` diagnostic and then exit the
    // STEP successfully, skipping the three remaining gates entirely,
    // while every other assertion in this test (including the old generic
    // `exit 1` check) stayed green — verified directly (RED proof,
    // S-626-1 ADV-P51 fix commit). Each branch is now pinned
    // independently via `extract_if_block` (defined below, near the
    // other line-based extraction helpers), which locates that branch's
    // own `if ... then ... fi` block and asserts `exit 1` appears INSIDE
    // it specifically — a mutation to any one branch's exit code fails
    // exactly that branch's assertion, not a generic shared one.
    let binary_floor_block = extract_if_block(test_block, "if [ \"${binaries}\" -lt 90 ]; then\n");
    assert!(
        binary_floor_block.contains("\n            exit 1\n"),
        "FAIL (POL-11 / ADV-P51-MED-001): the binary-count floor branch \
         does not contain its own `exit 1` inside its `if` block.\n\
         Current branch block:\n{binary_floor_block}"
    );

    let canary_presence_block = extract_if_block(
        test_block,
        "if ! grep -q \"ci_gate_completeness\" \"$RUNNER_TEMP/cargo_test_out.txt\"; then\n",
    );
    assert!(
        canary_presence_block.contains("\n            exit 1\n"),
        "FAIL (POL-11 / ADV-P51-MED-001): the named-canary presence-gate \
         branch does not contain its own `exit 1` inside its `if` block.\n\
         Current branch block:\n{canary_presence_block}"
    );

    let canary_passed_block =
        extract_if_block(test_block, "if [ \"${_canary_passed}\" -eq 0 ]; then\n");
    assert!(
        canary_passed_block.contains("\n            exit 1\n"),
        "FAIL (POL-11 / ADV-P51-MED-001): the named-canary passed-count- \
         gate branch does not contain its own `exit 1` inside its `if` \
         block.\n\
         Current branch block:\n{canary_passed_block}"
    );

    let zero_test_floor_block = extract_if_block(test_block, "if [ \"${total}\" -eq 0 ]; then\n");
    assert!(
        zero_test_floor_block.contains("\n            exit 1\n"),
        "FAIL (POL-11 / ADV-P51-MED-001): the zero-test floor branch does \
         not contain its own `exit 1` inside its `if` block.\n\
         Current branch block:\n{zero_test_floor_block}"
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

    // --- `shell: bash` override is present ---
    // The `test` job is a 3-OS matrix (ubuntu/macos/windows). GitHub Actions
    // defaults `run:` steps to `pwsh` on `windows-latest`; the floor guard's
    // script uses `set -euo pipefail`, `[ ... ]` tests, and `$(...)` command
    // substitution, none of which are valid pwsh syntax. Without this
    // override the step would fail outright on the Windows leg with a pwsh
    // parse error, rather than silently skip the floor — but that failure
    // would be indistinguishable at a glance from a genuine POL-11 violation
    // and would block the Windows leg on every run, so the override is
    // treated as load-bearing rather than something to discover by accident.
    // `"shell: bash"` appears exactly once in the whole of `ci.yml` (verified),
    // so this is not comment-satisfiable by any text elsewhere in the file
    // today.
    assert!(
        test_block.contains("shell: bash"),
        "FAIL (POL-11): The `test` job step does not override `shell: bash`.\n\
         Required: GitHub Actions defaults `run:` steps to `pwsh` on \
         `windows-latest`; the floor guard's script (`set -euo pipefail`, \
         `[ ... ]` tests, `$(...)` substitution) is bash syntax and is \
         invalid under pwsh. Without this override the step fails outright \
         on the Windows leg of the 3-OS matrix.\n\
         Current test job block:\n{test_block}"
    );

    // --- the exact `cargo test` capture invocation is present ---
    // Pins the full `run:` command line the rest of this test's gates
    // depend on: `--all-features` (so `#[cfg(test)]` gated code is actually
    // exercised) and the `tee` target (`$RUNNER_TEMP/cargo_test_out.txt`,
    // the same path the `total=`/`binaries=` computations below read back
    // from). A rewrite that drops `--all-features`, redirects to a
    // different file, or otherwise changes what gets captured is
    // undetected by any other assertion in this test — the `total`/
    // `binaries` gates only check the *values* of those variables, not how
    // they were populated. This does NOT pin the `total=`/`binaries=`
    // computation pipelines themselves (the `grep`/`grep -Eo`/`awk` and
    // `grep`/`wc -l`/`tr` chains) — those remain structurally unpinnable by
    // substring matching, per the "NOT PINNED" note above.
    //
    // The assertion targets the exact standalone line (trailing `\n`), not
    // a bare `2>&1` substring: `2>&1` occurs exactly once in the whole of
    // `ci.yml` today, so a bare substring would already be unambiguous, but
    // the longer command-bound form is chosen anyway because it also pins
    // `--all-features` and the `tee` target, closing that portion of the
    // "NOT PINNED" gap in one assertion. `RUNNER_TEMP/cargo_test_out.txt`
    // alone would be ambiguous — it also appears in the `grep` lines that
    // read the file back — so the full command line (unique in the file)
    // is required.
    assert!(
        test_block
            .contains("cargo test --all-features 2>&1 | tee \"$RUNNER_TEMP/cargo_test_out.txt\"\n"),
        "FAIL (POL-11): The `test` job step does not contain the exact \
         capture invocation \
         `cargo test --all-features 2>&1 | tee \"$RUNNER_TEMP/cargo_test_out.txt\"`.\n\
         Required: `--all-features` ensures feature-gated tests run; the \
         `tee` target must match the file the `total=`/`binaries=` \
         computations read back from.\n\
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
// ADV-P51 — guard-strength gaps in the `test` job's POL-11 guard step
// ---------------------------------------------------------------------------
//
// Adversarial pass 51 identified that every anti-neutering control built for
// `ci-gate` over 20 review rounds (a step-level `if:` ban, a
// `continue-on-error` ban, a byte-for-byte `run:` line pin, an `env:`
// key-set pin, ordered per-step key sets) was never propagated to the OTHER
// jobs in `ci.yml` — above all `test`, the `ci-gate.needs` member that
// carries the entire regression suite. This section closes that gap for the
// `test` job's own multi-instrument guard step, reusing the SAME idiom
// (`PINNED_GATE_STEP_KEY_SETS`/`extract_gate_step_key_sets`,
// `PINNED_GATE_ENV_KEYS`/`extract_gate_env_key_set`) already proven against
// `ci-gate` in rounds 11-13, scoped to this one step rather than the whole
// `test` job.
//
// SCOPE (deliberate, documented per the governing mandate — "where you
// deliberately exclude a job or step from a control, say so in an in-code
// comment"):
//   - The step-KEY-SET pin below (`PINNED_TEST_GUARD_STEP_KEYS`) is scoped
//     to the ONE step carrying the POL-11 guard (`name: Run tests
//     (zero-test floor, POL-11)`), NOT the whole `test` job. `test` runs a
//     3-OS matrix and may legitimately need an OS-conditional step in the
//     future (e.g. a Windows-only setup step) — a blanket step-level `if:`
//     ban across every step in this job would foreclose that. The reported
//     attack (ADV-P51-HIGH-001/HIGH-002) targets this ONE step
//     specifically; closing it there does not require closing the whole
//     job, and closing the whole job would risk a real false-positive
//     against a future, legitimate matrix-conditional step.
//   - `continue-on-error` is separately banned across ALL SEVEN always-run
//     jobs (`fmt`, `clippy`, `test`, `msrv`, `deny`, `spec-guard`,
//     `check-signing-workflow-injection` — the same list
//     `test_ci_gate_needs_jobs_have_no_job_level_if` already uses) by
//     `test_always_run_jobs_have_no_continue_on_error` below, since
//     ADV-P51-HIGH-002 named this gap on `msrv`, `spec-guard`, `deny`,
//     `fmt`, and `clippy` explicitly, in addition to `test`. `mutants` is
//     deliberately EXCLUDED — it legitimately uses `continue-on-error:
//     true` (see that job's own in-YAML comment: the "Check kill rate"
//     step is the sole pass/fail arbiter and must run regardless of
//     whether "Run mutation tests" exits non-zero) — and `ci-gate` is
//     deliberately EXCLUDED — already covered by its own dedicated,
//     narrower M2-j presence-ban in
//     `test_ci_gate_pass_fail_semantics_are_structurally_placed`.
//     `security` and `coverage` are not `ci-gate.needs` members (advisory
//     by design) and are out of scope for the same reason
//     `test_ci_gate_needs_jobs_have_no_job_level_if` already excludes
//     them.
//   - The pipefail-bracket ORDERING constraint (ADV-P51-HIGH-003) and the
//     per-branch `exit 1` pins (ADV-P51-MED-001, added in place above) are
//     specific to the `test` job's own POL-11 script shape — no other job
//     in `ci.yml` has this count-computation/floor-gate pattern — so they
//     are not generalized to other jobs.

/// ADV-P51-HIGH-001/HIGH-002: the `test` job's POL-11 guard step's
/// COMPLETE key set, mirroring `PINNED_GATE_STEP_KEY_SETS`'s idiom for
/// `ci-gate`. Adding `if:` (HIGH-1: makes the step conditionally
/// skippable — e.g. `if: false` — the job concludes `success`, `ci-gate`
/// goes green with zero tests run) or `continue-on-error:` (HIGH-2:
/// neutralizes all four `exit 1` gates AND a genuine `cargo test` failure
/// — step outcome `failure`, conclusion `success`, job `success`) changes
/// this set and fails `test_test_job_guard_step_key_set_and_env_are_pinned`
/// below.
const PINNED_TEST_GUARD_STEP_KEYS: &[&str] = &["env", "name", "run", "shell"];

/// ADV-P51-MED-002: the `env:` block's COMPLETE key set on the `test`
/// job's POL-11 guard step, mirroring `PINNED_GATE_ENV_KEYS`'s idiom for
/// `ci-gate`. A sibling key alongside `CARGO_TERM_COLOR` (e.g. `BASH_ENV`,
/// sourced by non-interactive bash before the pinned script body runs —
/// the same mechanism PR #671 review round 12 CRITICAL 2 demonstrated
/// against `ci-gate`) changes this set and fails the same test.
const PINNED_TEST_GUARD_ENV_KEYS: &[&str] = &["CARGO_TERM_COLOR"];

/// Locate the `test` job's POL-11 guard step (`name: Run tests (zero-test
/// floor, POL-11)`, 6-space `- ` marker) within `job_block` and return
/// `(all lines, start index, end index)` bounding just that step — from
/// its own `- ` marker up to (but not including) the next 6-space `- `
/// step marker, or the end of the job block if it is the last step.
///
/// Scoped narrowly (unlike `extract_gate_step_key_sets`, which collects
/// EVERY step in `ci-gate`) because the `test` job's other three steps
/// (harden-runner, checkout, rust-cache) are not part of ADV-P51's
/// reported attack surface and are deliberately out of scope for this pin
/// — see the module-level ADV-P51 scope note above.
fn extract_test_guard_step_lines(job_block: &str) -> Option<(Vec<&str>, usize, usize)> {
    let lines: Vec<&str> = job_block.lines().collect();
    let start = lines
        .iter()
        .position(|l| *l == "      - name: Run tests (zero-test floor, POL-11)")?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.starts_with("      -"))
        .map(|i| i + start + 1)
        .unwrap_or(lines.len());
    Some((lines, start, end))
}

/// Extract the sorted, complete key set of the `test` job's POL-11 guard
/// step (see `extract_test_guard_step_lines`), for comparison against
/// `PINNED_TEST_GUARD_STEP_KEYS`.
fn extract_test_guard_step_keys(job_block: &str) -> Vec<String> {
    let Some((lines, start, end)) = extract_test_guard_step_lines(job_block) else {
        return Vec::new();
    };
    let mut keys: Vec<String> = lines[start..end]
        .iter()
        .filter_map(|l| {
            extract_key_name_at_indent(l, 6).or_else(|| extract_key_name_at_indent(l, 8))
        })
        .collect();
    keys.sort();
    keys
}

/// Extract the sorted, complete key set of the `env:` block belonging to
/// the `test` job's POL-11 guard step (10-space indent — one level deeper
/// than the step's own 8-space keys), for comparison against
/// `PINNED_TEST_GUARD_ENV_KEYS`.
///
/// Anchored to the `env:` line found WITHIN this one step's own line
/// range (`start..run_idx`, not the whole job block) — narrower, and
/// therefore safer against the round-13 IMPORTANT-2 mis-anchoring class,
/// than `extract_gate_env_key_set`'s whole-job-block backward scan, since
/// this function cannot see any OTHER step's `env:` block even
/// transiently.
fn extract_test_guard_env_key_set(job_block: &str) -> Vec<String> {
    let Some((lines, start, end)) = extract_test_guard_step_lines(job_block) else {
        return Vec::new();
    };
    let Some(run_rel_idx) = lines[start..end]
        .iter()
        .position(|l| extract_key_name_at_indent(l, 8).as_deref() == Some("run"))
    else {
        return Vec::new();
    };
    let run_idx = start + run_rel_idx;
    let Some(env_rel_idx) = lines[start..run_idx]
        .iter()
        .position(|l| extract_key_name_at_indent(l, 8).as_deref() == Some("env"))
    else {
        return Vec::new();
    };
    let env_idx = start + env_rel_idx;
    collect_mapping_key_set(&lines, env_idx + 1, 10)
}

/// ADV-P51-HIGH-001/HIGH-002/MED-002. RED proof (S-626-1 ADV-P51 fix
/// commit): adding `if: false` to the guard step, separately adding
/// `continue-on-error: true` to the guard step, and separately adding a
/// `BASH_ENV: /tmp/shim.sh` sibling under the guard step's `env:`, were
/// each verified to fail this test before the pin existed to catch them —
/// then verified to pass again once `git checkout HEAD --
/// .github/workflows/ci.yml` restored the unmodified file.
#[test]
fn test_test_job_guard_step_key_set_and_env_are_pinned() {
    let ci = read_ci_yml();
    let test_block = extract_job_block(&ci, "test").unwrap_or_else(|| {
        panic!("FAIL: `.github/workflows/ci.yml` does not contain a `test:` job.")
    });

    let mut expected_step_keys: Vec<String> = PINNED_TEST_GUARD_STEP_KEYS
        .iter()
        .map(|s| s.to_string())
        .collect();
    expected_step_keys.sort();
    let actual_step_keys = extract_test_guard_step_keys(test_block);
    assert!(
        !actual_step_keys.is_empty(),
        "FAIL (ADV-P51-HIGH-001/HIGH-002): could not locate the `test` \
         job's POL-11 guard step at all (expected to find its `- name: \
         Run tests (zero-test floor, POL-11)` marker at 6-space indent). \
         Either the step's `name:` value changed, or its structure was \
         otherwise rewritten — update `extract_test_guard_step_lines` if \
         this is a deliberate rename.\n\
         Current test job block:\n{test_block}"
    );
    assert_eq!(
        actual_step_keys, expected_step_keys,
        "FAIL (ADV-P51-HIGH-001/HIGH-002): the `test` job's POL-11 guard \
         step's key set does not match the pinned, human-reviewed set \
         ({PINNED_TEST_GUARD_STEP_KEYS:?}).\n\
         Any added key (`if:`, `continue-on-error:`, `working-directory:`, \
         anything not yet imagined) — or a removed one — changes this \
         set. `if: false` makes the step SKIP silently (job concludes \
         `success`, `ci-gate` goes green with zero tests run — HIGH-1); \
         `continue-on-error: true` neutralizes all four `exit 1` gates AND \
         a genuine `cargo test` failure (HIGH-2). If this is a \
         deliberate, reviewed change, update `PINNED_TEST_GUARD_STEP_KEYS` \
         in the same commit.\n\
         Actual: {actual_step_keys:?}\n\
         Current test job block:\n{test_block}"
    );

    let mut expected_env_keys: Vec<String> = PINNED_TEST_GUARD_ENV_KEYS
        .iter()
        .map(|s| s.to_string())
        .collect();
    expected_env_keys.sort();
    let actual_env_keys = extract_test_guard_env_key_set(test_block);
    assert!(
        !actual_env_keys.is_empty(),
        "FAIL (ADV-P51-MED-002): could not locate the `env:` block on the \
         `test` job's POL-11 guard step at all (expected at least \
         `CARGO_TERM_COLOR`). Either the step was renamed/restructured, or \
         `env:` was reordered to after `run:` (legal YAML, but this \
         extractor scans only `env:` lines preceding `run:` within this \
         step) — update `extract_test_guard_env_key_set` if so.\n\
         Current test job block:\n{test_block}"
    );
    assert_eq!(
        actual_env_keys, expected_env_keys,
        "FAIL (ADV-P51-MED-002): the `test` job's POL-11 guard step's \
         `env:` key set does not match the pinned set \
         ({PINNED_TEST_GUARD_ENV_KEYS:?}).\n\
         A sibling key alongside `CARGO_TERM_COLOR` (e.g. `BASH_ENV`, \
         sourced by non-interactive bash before the pinned script body \
         runs) is independently exploitable even though the step's own \
         key set (`env`/`name`/`run`/`shell`) stays unchanged. If this is \
         a deliberate, reviewed change, update `PINNED_TEST_GUARD_ENV_KEYS` \
         in the same commit.\n\
         Actual: {actual_env_keys:?}\n\
         Current test job block:\n{test_block}"
    );
}

/// ADV-P51-HIGH-003 — the highest-impact finding in this pass (fires on
/// ORDINARY test breakage, not a deliberate bypass). The three `set
/// [-+]o pipefail` lines pinned by `test_verify_test_job_has_zero_test_floor`
/// are presence checks only — none of them constrain RELATIVE POSITION.
/// Moving the existing `set +o pipefail` line up ~13 lines, so it
/// precedes `cargo test --all-features 2>&1 | tee ...`, makes the `tee`
/// pipeline's exit status always 0 (`tee`'s own exit code, since pipefail
/// is already disabled by the time the pipe runs) — `set -e` therefore
/// never fires on a genuinely failing `cargo test`, and the step falls
/// through to the count computations, satisfies every floor/canary gate
/// on whatever partial output cargo printed before failing, and prints
/// `Check passed:` even though tests failed. Net line count is unchanged
/// (reads as "consolidated the pipefail bracket" in a diff) and all three
/// presence checks (`set -euo pipefail\n`, `set +o pipefail\n`, `set -o
/// pipefail\n`) stay satisfied regardless of WHERE each line sits.
///
/// This test closes that gap with a POSITION constraint: the byte offset
/// of each marker (within the `test` job block) must be strictly
/// increasing in the required order — `set -euo pipefail` (open) → the
/// `cargo test` capture line → `set +o pipefail` (disable) → `set -o
/// pipefail` (restore). `.find()` locates the FIRST occurrence of each
/// marker, so inserting an EARLIER duplicate of `set +o pipefail` (rather
/// than moving the original) is caught identically — the earliest
/// occurrence is what gets compared against the capture line's position.
///
/// RED proof (S-626-1 ADV-P51 fix commit): moving `set +o pipefail\n` to
/// immediately after `set -euo pipefail\n` (before the `cargo test`
/// capture line) was verified to fail this test before the pin existed,
/// then verified to pass again once `git checkout HEAD --
/// .github/workflows/ci.yml` restored the unmodified file.
#[test]
fn test_test_job_pipefail_bracket_ordering_is_position_constrained() {
    let ci = read_ci_yml();
    let test_block = extract_job_block(&ci, "test").unwrap_or_else(|| {
        panic!("FAIL: `.github/workflows/ci.yml` does not contain a `test:` job.")
    });

    let markers: &[(&str, &str)] = &[
        ("set -euo pipefail (open)", "set -euo pipefail\n"),
        (
            "cargo test capture line",
            "cargo test --all-features 2>&1 | tee \"$RUNNER_TEMP/cargo_test_out.txt\"\n",
        ),
        (
            "set +o pipefail (disable, before count computations)",
            "set +o pipefail\n",
        ),
        ("set -o pipefail (restore)", "set -o pipefail\n"),
    ];

    let mut offsets: Vec<(&str, usize)> = Vec::with_capacity(markers.len());
    for (label, marker) in markers {
        let idx = test_block.find(marker).unwrap_or_else(|| {
            panic!(
                "FAIL (ADV-P51-HIGH-003): required marker for '{label}' \
                 ({marker:?}) was not found in the `test` job's guard \
                 step.\n\
                 Current test job block:\n{test_block}"
            )
        });
        offsets.push((label, idx));
    }

    for pair in offsets.windows(2) {
        let (prev_label, prev_idx) = pair[0];
        let (label, idx) = pair[1];
        assert!(
            prev_idx < idx,
            "FAIL (ADV-P51-HIGH-003): pipefail bracket ordering violated \
             — '{prev_label}' (byte offset {prev_idx}) must appear BEFORE \
             '{label}' (byte offset {idx}) in the `test` job's guard step, \
             but it does not.\n\
             Required order: `set -euo pipefail` → the `cargo test` \
             capture line → `set +o pipefail` → `set -o pipefail` (the \
             restore). Moving `set +o pipefail` to precede the `cargo \
             test` capture line makes the `tee` pipeline's exit status \
             always 0 (`tee`'s own exit code), so `set -e` never fires on \
             a genuinely failing test suite — the step falls through to \
             the count computations, satisfies every floor/canary gate on \
             whatever partial output cargo printed, and prints `Check \
             passed:` even though tests failed. This is ADV-P51's \
             highest-impact finding: it fires on ORDINARY test breakage, \
             not a deliberate bypass.\n\
             Current test job block:\n{test_block}"
        );
    }
}

/// ADV-P51-HIGH-002 (class sweep, per the governing mandate — "sweep to
/// class, do not fix only the reported instances"): `continue-on-error`
/// has no legitimate use on any of the seven always-run jobs required to
/// pass unconditionally on every push and PR (the same list
/// `test_ci_gate_needs_jobs_have_no_job_level_if` uses) — unlike
/// `mutants`, which legitimately relies on it, and unlike `ci-gate`,
/// which already has its own dedicated presence-ban
/// (`test_ci_gate_pass_fail_semantics_are_structurally_placed`'s M2-j).
/// See the module-level ADV-P51 scope note above for why `mutants` and
/// `ci-gate` are excluded from this loop.
///
/// `continue-on-error: true` on any step in these seven jobs would make a
/// genuinely failing step (step outcome `failure`) report job conclusion
/// `success` — silently satisfying `ci-gate.needs` for that job while the
/// underlying check (format, lint, test, MSRV compile, spec-guard, or the
/// signing-injection guard) never actually gated anything.
///
/// RED proof (S-626-1 ADV-P51 fix commit): adding `continue-on-error:
/// true` to the `fmt` job's sole step was verified to fail this test
/// before it existed, then verified to pass again once `git checkout
/// HEAD -- .github/workflows/ci.yml` restored the unmodified file.
#[test]
fn test_always_run_jobs_have_no_continue_on_error() {
    let ci = read_ci_yml();

    // Derived from the live `needs:` set (S-626-1 sweep-to-class fix; see
    // `always_run_needs_members`'s doc comment) rather than a hand-
    // maintained literal — see the module-level ADV-P51 scope note above
    // for why `mutants` and `ci-gate` are excluded from this loop.
    let required_jobs = always_run_needs_members(&ci);

    for job_name in &required_jobs {
        let job_block = extract_job_block(&ci, job_name).unwrap_or_else(|| {
            panic!(
                "FAIL: job `{job_name}` (listed in ci-gate.needs) was not \
                 found in ci.yml. Either the job was renamed or removed — \
                 update ci-gate.needs and this test together."
            )
        });
        assert!(
            !job_block.contains("continue-on-error"),
            "FAIL (ADV-P51-HIGH-002): job `{job_name}` contains \
             `continue-on-error`, which has no legitimate use on any of \
             the seven always-run jobs required to pass unconditionally \
             on every push and PR. `continue-on-error: true` on a failing \
             step reports job conclusion `success` regardless of that \
             step's own outcome, silently satisfying `ci-gate.needs` for \
             this job while the underlying check never actually gated \
             anything.\n\
             Current `{job_name}` block:\n{job_block}"
        );
    }
}

/// ADV-P51-MED-001: extract the `if ... then ... fi` block belonging to
/// `condition_line` (searched as an exact substring, expected to include
/// its own trailing `\n`) within `job_block`, up to and including its
/// closing `fi` (10-space indent, matching this guard step's
/// convention). Panics loudly (not a silent empty return) if either the
/// condition line or its closing `fi` cannot be found — an extractor
/// that can silently under-report here would reproduce the exact class
/// of bug PR #671 review round 13 fixed in `collect_mapping_key_set`
/// (see that function's doc comment).
fn extract_if_block<'a>(job_block: &'a str, condition_line: &str) -> &'a str {
    let start = job_block.find(condition_line).unwrap_or_else(|| {
        panic!(
            "FAIL (POL-11 / ADV-P51-MED-001): required condition line \
             {condition_line:?} was not found in the `test` job's guard \
             step.\n\
             Current test job block:\n{job_block}"
        )
    });
    let after = &job_block[start..];
    let fi_marker = "\n          fi\n";
    let end = after.find(fi_marker).unwrap_or_else(|| {
        panic!(
            "FAIL (POL-11 / ADV-P51-MED-001): no closing `fi` (10-space \
             indent) found for condition line {condition_line:?} in the \
             `test` job's guard step.\n\
             Current test job block:\n{job_block}"
        )
    });
    &after[..end + fi_marker.len()]
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

    // -----------------------------------------------------------------------
    // F-02 (round 19): pin PLACEMENT, not just whole-block presence.
    //
    // The `RUSTUP_TOOLCHAIN: "1.85.0"` assertion above is a whole-block
    // substring check — it passes as long as that string appears ANYWHERE
    // in the `msrv` job, including on the WRONG step. Moving the `env:`
    // block from the `cargo check --all-features --locked` step onto the
    // `dtolnay/rust-toolchain` step (or any other step) keeps that
    // assertion green while `cargo check` runs with no `RUSTUP_TOOLCHAIN`
    // override; `rust-toolchain.toml` (`channel = "stable"`) then wins at
    // process level, and the job silently validates stable — exactly the
    // false-green AC-3 exists to close (see this test's own docstring and
    // CLAUDE.md § "rust-toolchain.toml outranks rustup default").
    //
    // Technique: isolate the step slice that starts at the `cargo check
    // --all-features --locked` anchor and runs to the next step boundary
    // (a line at the same `      - ` list-item indent used throughout
    // `steps:` in this file) or end of block — the same indent-based
    // level-distinction technique `test_ci_gate_pass_fail_semantics_are_structurally_placed`
    // uses to separate job-level from step-level `if:` keys. Then assert
    // the env override lives INSIDE that slice, not merely inside the
    // whole `msrv` block.
    //
    // ADV-P48-MED-001 (round 20): the anchor previously used here was the
    // BARE command substring `cargo check --all-features --locked`, found
    // via `str::find` (first occurrence in file order). The `msrv` job
    // carries a ~10-line scope-rationale comment ABOVE the real `run:`
    // step discussing `--all-targets`/`--all-features` (see this
    // function's docstring); that comment does not currently reproduce
    // the full concatenated command string, but nothing structurally
    // prevents a future edit from quoting it verbatim there for
    // explanatory purposes. If it did, `find` would anchor on the
    // COMMENT occurrence (earlier in the file than the real step) instead
    // of the real `run:` line, and the step-slice sliced from that wrong
    // anchor would not contain the real step's `env:` block — this test
    // would then fail RED on a genuinely correct config. That is a
    // robustness bug, not a false-green: the failure mode is fail-loud on
    // a false-positive substring match, never fail-silent.
    //
    // Fixed by anchoring on the actual YAML STEP SYNTAX rather than just
    // the command text: `\n      - run: <cmd>` — the exact newline +
    // 6-space list-item indent + `- run: ` prefix that appears only once
    // in this file, on the real step line. Every scope comment in this
    // job is `      #`-prefixed prose (see the comment immediately above
    // the real step); defeating this anchor would require reproducing the
    // literal step-declaration syntax `- run: ` character for character —
    // no longer an accidental substring collision but hand-authored YAML
    // forgery, the same "code review is the control for hand-crafted
    // YAML" boundary already documented for the node-property residual in
    // CLAUDE.md's CI Gate history (round 16). `rfind` (last occurrence)
    // was considered and rejected: it is still a bare substring match
    // with no syntactic anchoring, so a comment added AFTER the real step
    // (nothing prevents that ordering) would defeat it identically —
    // `rfind` narrows the reachable window without closing the
    // underlying gap the way anchoring on real step syntax does.
    // -----------------------------------------------------------------------
    let cargo_check_anchor = "\n      - run: cargo check --all-features --locked";
    let anchor_pos = msrv_block.find(cargo_check_anchor).unwrap_or_else(|| {
        panic!(
            "FAIL (S-626-1 AC-3 / F-02): could not re-locate the step-line \
             anchor `{cargo_check_anchor:?}` in the `msrv` job block to \
             check `RUSTUP_TOOLCHAIN` placement — the assertion immediately \
             above this one should already have failed.\n\
             Current msrv job block:\n{msrv_block}"
        )
    });
    // Skip the leading `\n` captured by the anchor so the slice starts on
    // the `      - run: …` step line itself.
    let after_anchor = &msrv_block[anchor_pos + 1..];
    // The next step begins at a line with the `      - ` (6-space + dash)
    // list-item indent used for every step in this file. Skip past the
    // anchor's own leading byte before searching so the anchor line itself
    // is never mistaken for the boundary.
    let step_end = after_anchor[1..]
        .find("\n      - ")
        .map(|p| p + 1)
        .unwrap_or(after_anchor.len());
    let cargo_check_step = &after_anchor[..step_end];

    assert!(
        cargo_check_step.contains("RUSTUP_TOOLCHAIN: \"1.85.0\""),
        "FAIL (S-626-1 AC-3 / F-02): `RUSTUP_TOOLCHAIN: \"1.85.0\"` is not \
         on the SAME step as `cargo check --all-features --locked`.\n\
         `RUSTUP_TOOLCHAIN` outranks `rust-toolchain.toml` at PROCESS level \
         — it must be an `env:` override on the `cargo check` step itself. \
         Setting it on any other step (e.g. the `dtolnay/rust-toolchain` \
         step) only affects that step's own process; `cargo check` would \
         then run with no override, `rust-toolchain.toml`'s \
         `channel = \"stable\"` would win, and the job would silently \
         validate stable again.\n\
         Step slice inspected (from the `cargo check` anchor to the next \
         step boundary):\n{cargo_check_step}\n\
         Full msrv job block:\n{msrv_block}"
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
/// S-626-1 pass-55 (ADV-P55-LOW-002): originally scoped to `ci.yml` alone.
/// Guard A (`test_no_sibling_workflow_declares_a_job_named_ci_gate`) and
/// its helpers (`list_job_ids_in_workflow`, `extract_job_display_name`)
/// now also line-scan every sibling `.github/workflows/*.yml`/`*.yaml`
/// file — each of those extractors shares the exact same `str::lines()`-
/// only line-splitting this test exists to guard, but until this fix, no
/// byte-level scan covered them: a lone CR (or NEL / LINE SEPARATOR /
/// PARAGRAPH SEPARATOR) smuggled into a sibling workflow file could hide a
/// `name: CI Gate` job-key from `extract_job_display_name`'s line-based
/// scan the same way round 14 showed it could hide a key from `ci.yml`'s
/// own extractors — with no test anywhere in this file able to catch it.
/// Extended (cheaper than a second, separate test, and `list_workflow_
/// files` is already a directory walk this file performs elsewhere) to
/// scan every file `list_workflow_files` enumerates, `ci.yml` included,
/// rather than leave sibling files as a documented-but-unguarded gap.
#[test]
fn test_ci_yml_contains_no_non_lf_yaml_line_breaks() {
    const FORBIDDEN: &[(char, &str)] = &[
        ('\r', "CR (U+000D)"),
        ('\u{0085}', "NEL (U+0085)"),
        ('\u{2028}', "LINE SEPARATOR (U+2028)"),
        ('\u{2029}', "PARAGRAPH SEPARATOR (U+2029)"),
    ];

    for path in list_workflow_files() {
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));

        for (byte_offset, ch) in raw.char_indices() {
            if let Some((_, label)) = FORBIDDEN.iter().find(|(forbidden, _)| *forbidden == ch) {
                panic!(
                    "FAIL (PR #671 review round 14, CRITICAL; extended to \
                     sibling workflows S-626-1 pass-55, ADV-P55-LOW-002): \
                     {} contains a {label} character at byte offset \
                     {byte_offset}. Every line-based extractor in this \
                     suite splits on `str::lines()`, which recognizes ONLY \
                     `\\n` as a line break — but this character is a valid \
                     YAML line break in its own right (YAML 1.1 `b-char`; \
                     PyYAML and Ruby Psych/libyaml both honor it) that a \
                     real YAML parser treats as ending the logical line. A \
                     key placed after this character, on the same \
                     physical text line as a preceding key, is INVISIBLE \
                     to every line-based check in this file simultaneously \
                     while a real parser sees a normal, separate key — see \
                     this test's doc comment for three reproduced \
                     one-line exploits against `ci.yml` (workflow-env, \
                     workflow `defaults:`, and gate-step `shell:` \
                     smuggling); the same mechanism applies to Guard A's \
                     sibling-workflow scan. This is a byte-level tripwire, \
                     not a general fix for line-based extraction — see the \
                     doc comment for why a real YAML-parse rewrite is the \
                     durable fix, tracked as a follow-up story, not this \
                     test.",
                    path.display()
                );
            }
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
/// HISTORICAL RED GATE NOTE (S-CIGATE-2, 2026-08-06): at the commit that
/// introduced this test, `ci-gate`'s step still used the retired inline
/// `contains(needs.*.result, ...)` condition and did not invoke
/// `check-ci-gate.sh` anywhere, so this test FAILED until the Green phase
/// implemented AC-001 in the same story. As of head, the Green phase has
/// long since landed: `ci-gate`'s step invokes `scripts/check-ci-gate.sh`
/// fed `toJSON(needs)`, and this test is expected to PASS on every run —
/// the RED description above documents the TDD history of this test, not
/// its current expected result.
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

    // S-626-1 pass-54/56 (ADV-P54-MED-001 / ADV-P56-MED-001, dedupe): the
    // two substring checks above are satisfied by EITHER substring
    // appearing anywhere in `spec-guard` — including inside one of the
    // job's TWO OTHER `--self-test` steps, unrelated to `check-ci-gate.sh`
    // — so they can never independently confirm the check-ci-gate.sh
    // self-test step specifically executes anything. Byte-pin that step's
    // OWN `run:` line, anchored to the step named "check-ci-gate self-test
    // (fixture suite, S-CIGATE-2)".
    let actual_self_test_run_line = extract_and_normalize_step_run_line_by_name(
        spec_guard_block,
        "check-ci-gate self-test (fixture suite, S-CIGATE-2)",
    )
    .unwrap_or_else(|reason| {
        panic!(
            "FAIL (S-626-1 pass-54, ADV-P54-MED-001): `spec-guard` {reason}\n\
             Current spec-guard block:\n{spec_guard_block}"
        )
    });
    assert_eq!(
        actual_self_test_run_line, PINNED_CI_GATE_SELF_TEST_RUN_LINE,
        "FAIL (S-626-1 pass-54, ADV-P54-MED-001): `spec-guard`'s \
         check-ci-gate self-test step's `run:` line \
         (\"{actual_self_test_run_line}\") does not byte-match the pinned, \
         human-reviewed literal (\"{PINNED_CI_GATE_SELF_TEST_RUN_LINE}\"). \
         A suffix like `|| true` would silently disable the entire \
         13-fixture suite's pass/fail signal while leaving both substring \
         checks above (and `test_always_run_jobs_have_no_continue_on_error`, \
         which only bans the `continue-on-error` KEY, not a shell-level \
         suffix) fully satisfied. If this is a deliberate, reviewed \
         change, update PINNED_CI_GATE_SELF_TEST_RUN_LINE in the SAME \
         change.\n\
         Current spec-guard block:\n{spec_guard_block}"
    );

    // S-626-1 pass-59 (ADV-P58-MED-001): the run-line pin above covers the
    // step's `run:` VALUE, but nothing previously asserted the step's
    // KEYS were exactly `{name, run}` — the same "value pinned, keys left
    // an open enumeration" gap round 11 closed for `ci-gate` itself via
    // `PINNED_GATE_STEP_KEY_SETS`. Reproduced: `if: false` on this step
    // (the step silently never runs; job concludes `success`) and `shell:
    // cat {0}` (the runner `cat`s the run line's script body instead of
    // executing it) each leave the byte-pinned run-line assertion above,
    // both substring checks, and `test_always_run_jobs_have_no_continue_
    // on_error` all satisfied.
    let actual_self_test_step_keys = extract_step_key_set_by_name(
        spec_guard_block,
        "check-ci-gate self-test (fixture suite, S-CIGATE-2)",
    )
    .unwrap_or_else(|reason| {
        panic!(
            "FAIL (S-626-1 pass-59, ADV-P58-MED-001): `spec-guard` {reason}\n\
             Current spec-guard block:\n{spec_guard_block}"
        )
    });
    let mut expected_self_test_step_keys: Vec<String> = PINNED_CI_GATE_SELF_TEST_STEP_KEYS
        .iter()
        .map(|s| s.to_string())
        .collect();
    expected_self_test_step_keys.sort();
    assert_eq!(
        actual_self_test_step_keys, expected_self_test_step_keys,
        "FAIL (S-626-1 pass-59, ADV-P58-MED-001): `spec-guard`'s \
         check-ci-gate self-test step's key set \
         ({actual_self_test_step_keys:?}) does not match the pinned set \
         ({PINNED_CI_GATE_SELF_TEST_STEP_KEYS:?}). An added `if:` \
         (silently skips the step; job still concludes `success`) or \
         `shell:` (replaces the run line's interpreter — the same \
         `shell: cat {{0}}` vector rounds 11/14 already showed defeats a \
         pinned `run:` line elsewhere in this file) would leave the \
         byte-pinned run-line value above untouched while neutralizing \
         the entire 13-fixture self-test suite. If this is a deliberate, \
         reviewed change, update PINNED_CI_GATE_SELF_TEST_STEP_KEYS in \
         the SAME change.\n\
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

/// S-626-1 sweep-to-class fix: `SKIP_TOLERANT_NEEDS_MEMBERS` (portable) is a
/// hand-maintained duplicate of `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS`'s
/// (`#[cfg(unix)]`-only) job-name set — kept separate rather than derived
/// from one another because `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS` is
/// `#[cfg(unix)]`-gated and `SKIP_TOLERANT_NEEDS_MEMBERS` must not be (its
/// callers run on every platform). Two independent pins for the same
/// underlying fact can drift silently unless something checks them against
/// each other — this test is that check. It is itself `#[cfg(unix)]`-gated
/// (it reads the unix-gated constant), so it cannot close the drift window
/// on Windows; a divergence would only be caught there indirectly, the
/// next time someone runs the full suite on a unix runner (which CI does,
/// via `ubuntu-latest`).
#[cfg(unix)]
#[test]
fn test_skip_tolerant_needs_members_matches_pinned_if_expressions() {
    let mut from_expressions: Vec<&str> = PINNED_ALLOWED_SKIP_IF_EXPRESSIONS
        .iter()
        .map(|(job, _)| *job)
        .collect();
    from_expressions.sort_unstable();

    let mut from_portable: Vec<&str> = SKIP_TOLERANT_NEEDS_MEMBERS.to_vec();
    from_portable.sort_unstable();

    assert_eq!(
        from_portable, from_expressions,
        "FAIL: `SKIP_TOLERANT_NEEDS_MEMBERS` ({SKIP_TOLERANT_NEEDS_MEMBERS:?}) \
         has drifted from the job-name set in \
         `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS` ({from_expressions:?}). These \
         two pins name the same set of jobs (those legitimately permitted \
         to report `skipped` in `ci-gate.needs`) for two different \
         audiences — a portable one used by cross-platform tests, and a \
         unix-only one that additionally pins each job's exact `if:` \
         expression text. Update whichever one is stale in the same change \
         as whatever added or removed a skip-tolerant job."
    );
}

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
    // S-626-1 pass-55, ADV-P55-LOW-001: routed through
    // `extract_key_name_at_indent` rather than a bare `starts_with("    if:")`,
    // which matched only that one spelling and missed `"if":`, `'if':`, and
    // `if :` (space before colon) — all PyYAML-identical to the bare
    // spelling.
    let is_job_level_if_line = |l: &&str| extract_key_name_at_indent(l, 4).as_deref() == Some("if");

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
    // S-626-1 pass-59 (ADV-P58-LOW-002): the key was detected via
    // `is_job_level_if_line` (`extract_key_name_at_indent`, quote/
    // whitespace-aware), but this VALUE re-read used to be a bare
    // `strip_prefix("if:").unwrap_or("")` — the identical key-detect vs.
    // value-reparse swallow shape the `910b8ab0` class sweep fixed at the
    // two OTHER sites in this file that share this exact pattern
    // (`extract_job_display_name`'s `name:` re-read,
    // `test_matrix_os_lists_remain_static_literals`'s `os:` re-read). A
    // quoted key spelling (`"if":`/`'if':`) or `if :` (space before
    // colon) made `raw` silently collapse to `""`, which then normalizes
    // to `collapsed.is_empty()` -> `Ok(None)` — INDISTINGUISHABLE from
    // "this job declares no job-level `if:` key at all" for a job that
    // plainly has one. Every M2-m-style pin built on this function
    // compares against `Some(pin)`, so `Ok(None)` still fails LOUDLY
    // today only as an accident of that comparison shape, not because
    // this function itself refuses to guess — the same brittleness this
    // sweep's own rationale (see `extract_job_display_name`'s doc
    // comment) rejects. Fixed: `Err`, not a silent `Ok(None)`, so the
    // failure is diagnosable on its own terms rather than merely
    // happening to also fail a downstream equality check.
    let Some(raw) = if_line.trim_start().strip_prefix("if:") else {
        return Err(format!(
            "has a job-level `if:` key detected via the quote/whitespace-\
             aware matcher at 4-space indent, but this function's own \
             value-extraction re-read (a bare `strip_prefix(\"if:\")`) \
             could not parse the same line — most likely a quoted key \
             spelling (`\"if\":` / `'if':`) or `if :` (space before \
             colon), which `extract_key_name_at_indent` recognizes but \
             this bare re-read does not. Silently collapsing to an empty \
             string here would make this function return `Ok(None)` — \
             indistinguishable from \"this job declares no `if:` key at \
             all\" for a job that plainly has one, defeating every pin \
             built on this function's result.\n\
             Offending line: {if_line:?}"
        ));
    };
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

/// PINNED, human-reviewed exact text of the `spec-guard` job's
/// `check-ci-gate self-test (fixture suite, S-CIGATE-2)` step's `run:`
/// line (S-626-1 pass-54/56, ADV-P54-MED-001 / ADV-P56-MED-001 — same
/// finding reported twice, deduped here).
///
/// `test_spec_guard_contains_check_ci_gate_self_test_step` (AC-008) used
/// to check only `spec_guard_block.contains("check-ci-gate.sh") &&
/// spec_guard_block.contains("--self-test")` — two independent whole-BLOCK
/// substring checks, satisfied by either substring appearing ANYWHERE in
/// `spec-guard`, not necessarily on the same line or step. `spec-guard`
/// contains TWO OTHER `--self-test` invocations (`check-cargo-mutants-
/// policy-citations.sh --self-test`, `check-bc-citation-symbols.sh
/// --self-test`), so the `--self-test` conjunct can never fail on its own
/// — it is trivially satisfied by either of those two, independent of
/// whether the `check-ci-gate.sh --self-test` step exists at all.
/// Reproduced: replacing the step's `run:` line with `run: bash
/// scripts/check-ci-gate.sh --self-test || true` (silently disabling the
/// entire 13-fixture suite's pass/fail signal) leaves both substring
/// checks satisfied and `test_always_run_jobs_have_no_continue_on_error`
/// green too (that test bans the literal `continue-on-error` key, not a
/// shell-level `|| true` suffix) — the whole regression suite stays green
/// while the self-test step becomes a permanent no-op.
const PINNED_CI_GATE_SELF_TEST_RUN_LINE: &str = "bash scripts/check-ci-gate.sh --self-test";

/// PINNED, human-reviewed COMPLETE key set of the `spec-guard` job's
/// `check-ci-gate self-test (fixture suite, S-CIGATE-2)` step (S-626-1
/// pass-59, ADV-P58-MED-001). Mirrors `PINNED_GATE_STEP_KEY_SETS`/
/// `PINNED_TEST_GUARD_STEP_KEYS`'s idiom: `PINNED_CI_GATE_SELF_TEST_RUN_
/// LINE` (above) pins the `run:` line's VALUE, but nothing previously
/// asserted the step's KEYS were exactly `{name, run}` — an added
/// `if: false` or `shell: cat {0}` on this step leaves the run-line pin
/// (and every substring check in this test) fully satisfied while making
/// the entire 13-fixture self-test suite silently not run at all (`if:
/// false`) or run the wrong interpreter entirely (`shell: cat {0}`, the
/// same custom-shell-template override rounds 11/14 already showed
/// defeats a pinned `run:` line elsewhere in this file).
const PINNED_CI_GATE_SELF_TEST_STEP_KEYS: &[&str] = &["name", "run"];

/// Locate the SOLE step in `lines` whose `name:` value is EXACTLY
/// `step_name` (matched as the trimmed line `- name: {step_name}` —
/// INDENT-AGNOSTIC: the match is against `l.trim_start()`, so this finds
/// the step regardless of what indentation its `- name: ...` marker
/// actually sits at, not only the conventional 6-space step-marker
/// indent), returning its line index. `Err` if zero or more than one
/// step matches.
///
/// Shared by `extract_and_normalize_step_run_line_by_name` and
/// `extract_step_key_set_by_name` — factored out (S-626-1 pass-59,
/// ADV-P58-MED-001) so the duplicate-step-name rejection lives in exactly
/// one place rather than being reimplemented (and potentially
/// re-forgotten) at each new by-name step accessor.
///
/// **DOC CORRECTION (S-626-1 pass-60, ADV-P60-LOW-003):** this doc
/// comment and both `Err` messages below previously claimed the match
/// was against "the literal line `      - name: {step_name}`, 6-space
/// step-marker indent" — that overstated it. The code has always been
/// indent-agnostic (`l.trim_start() == name_needle`); the 6-space
/// figure described this file's one conventional step indent, not an
/// enforced requirement. This was a misleading-message defect, not a
/// false green: the function's actual behavior is STRICTER than the
/// old wording implied (it matches at ANY indent, so an unconventionally-
/// indented decoy or real step is still found, not silently missed),
/// but the old wording would have sent a debugger looking for an
/// indent-related cause that was never the issue.
fn find_sole_step_by_name(lines: &[&str], step_name: &str) -> Result<usize, String> {
    let name_needle = format!("- name: {step_name}");
    let matches: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_start() == name_needle)
        .map(|(i, _)| i)
        .collect();
    match matches.as_slice() {
        [] => Err(format!(
            "has no step with `name: {step_name}` at all (checked, at \
             any indent, for a line whose trimmed text is exactly \
             `- name: {step_name}`)."
        )),
        [only] => Ok(*only),
        multiple => Err(format!(
            "has {} steps named `name: {step_name}` (checked, at any \
             indent, for a line whose trimmed text is exactly \
             `- name: {step_name}`) — this checker requires exactly one \
             so a single pinned literal unambiguously covers it. A \
             decoy step sharing the real step's name, inserted anywhere \
             in the job block regardless of its own indentation, would \
             otherwise silently win a first-match lookup instead of \
             being flagged as ambiguous.",
            multiple.len()
        )),
    }
}

/// Extract the sorted, complete key set (6/8-space indent — a step's own
/// first key, on the `- ` marker line, plus every subsequent step-level
/// key) of the SOLE step in `job_block` whose `name:` value is EXACTLY
/// `step_name`, for comparison against `PINNED_CI_GATE_SELF_TEST_STEP_
/// KEYS`. Mirrors `extract_test_guard_step_keys`'s `indent(6).or_else(
/// indent(8))` idiom for a step's key set.
fn extract_step_key_set_by_name(job_block: &str, step_name: &str) -> Result<Vec<String>, String> {
    let lines: Vec<&str> = job_block.lines().collect();
    let name_line_idx = find_sole_step_by_name(&lines, step_name)?;

    let next_step_offset = lines[name_line_idx + 1..]
        .iter()
        .position(|l| l.starts_with("      -"));
    let step_end = next_step_offset
        .map(|off| name_line_idx + 1 + off)
        .unwrap_or(lines.len());

    let mut keys: Vec<String> = lines[name_line_idx..step_end]
        .iter()
        .filter_map(|l| {
            extract_key_name_at_indent(l, 6).or_else(|| extract_key_name_at_indent(l, 8))
        })
        .collect();
    keys.sort();
    Ok(keys)
}

/// Extract and normalize the SOLE `run:` line belonging to the step whose
/// `name:` value is EXACTLY `step_name` (matched as the literal line
/// `      - name: {step_name}`, 6-space step-marker indent — the shape
/// every step in `spec-guard` currently uses) inside `job_block`.
///
/// A deliberately separate function from `extract_and_normalize_sole_run_
/// line` rather than a generalization of it — same precedent that
/// function's own doc comment cites for staying separate from
/// `extract_and_normalize_if_expr`: reject-don't-parse normalization is
/// duplicated, not shared. This function additionally scopes its search
/// to ONE step (bounded by the next `      -` step marker or EOF) rather
/// than the whole job block, since `spec-guard` — unlike `ci-gate` — has
/// many steps and many `run:` lines; `extract_and_normalize_sole_run_
/// line`'s "exactly one `run:` in the whole block" invariant does not
/// hold there.
///
/// S-626-1 pass-59 (ADV-P58-MED-001): despite the `_sole_` in this
/// function's name describing the "exactly one `run:` line WITHIN the
/// matched step" invariant, step-NAME matching itself used to be
/// `.position(...)` — first match, with NO duplicate-name rejection. A
/// second, decoy step also named `name: {step_name}` inserted BEFORE the
/// real one (e.g. a no-op `run: true` step with the identical name) would
/// silently make this function pin the DECOY's `run:` line instead of the
/// real step's — the whole 27/27 suite stays green while the check-
/// ci-gate self-test step (or any future caller) is invisibly bypassed.
/// Fixed to collect ALL matching `name:` line indices and `Err` on more
/// than one, mirroring the `needs_line_indices`/`run_line_indices`
/// multiple-match rejection idiom already used throughout this file
/// (`parse_needs_set`, `extract_and_normalize_if_expr`, and this same
/// function's own `run:`-line duplicate check just below).
fn extract_and_normalize_step_run_line_by_name(
    job_block: &str,
    step_name: &str,
) -> Result<String, String> {
    let lines: Vec<&str> = job_block.lines().collect();
    let name_line_idx = find_sole_step_by_name(&lines, step_name)?;

    let next_step_offset = lines[name_line_idx + 1..]
        .iter()
        .position(|l| l.starts_with("      -"));
    let step_end = next_step_offset
        .map(|off| name_line_idx + 1 + off)
        .unwrap_or(lines.len());
    let step_lines = &lines[name_line_idx..step_end];

    let run_line_indices: Vec<usize> = step_lines
        .iter()
        .enumerate()
        .filter(|(_, l)| extract_key_name_at_indent(l, 8).as_deref() == Some("run"))
        .map(|(i, _)| i)
        .collect();

    if run_line_indices.is_empty() {
        return Err(format!(
            "'s `{step_name}` step has no step-level `run:` line at the \
             expected 8-space indent."
        ));
    }
    if run_line_indices.len() > 1 {
        return Err(format!(
            "'s `{step_name}` step has {} step-level `run:` lines — this \
             checker requires exactly one so a single pinned literal \
             unambiguously covers it.",
            run_line_indices.len()
        ));
    }
    let run_line = step_lines[run_line_indices[0]];

    let raw = run_line.trim_start().strip_prefix("run:").unwrap_or("");
    let raw_value_leading_trimmed = raw.trim_start();

    if raw_value_leading_trimmed.starts_with('>') || raw_value_leading_trimmed.starts_with('|') {
        return Err(format!(
            "'s `{step_name}` step's `run:` value uses a YAML block-scalar \
             form (\"{}\") — the real command lives on continuation lines \
             this checker does not read, so it cannot be safely \
             represented as a single pinned literal.",
            raw_value_leading_trimmed.trim()
        ));
    }

    if let Some(next_line) = step_lines[run_line_indices[0] + 1..].iter().find(|l| {
        let trimmed = l.trim_start();
        !trimmed.is_empty() && !trimmed.starts_with('#')
    }) {
        let indent = next_line.len() - next_line.trim_start().len();
        if indent > 8 {
            return Err(format!(
                "'s `{step_name}` step's `run:` value appears to continue \
                 onto a following line (\"{}\", indented {indent} spaces) \
                 — this cannot be safely represented as a single pinned \
                 literal.",
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
            "'s `{step_name}` step's `run:` value contains a `#` that is \
             not a clearly whitespace-delimited trailing comment \
             (\"{}\") — this cannot be safely normalized.",
            raw.trim()
        ));
    }

    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return Err(format!("'s `{step_name}` step has an empty `run:` value."));
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
// ADV-P50-LOW-003 (round 20): `timeout-minutes` added to `ci-gate` (see
// ci.yml comment on that job) to match every sibling job's explicit
// timeout instead of silently inheriting GitHub Actions' 360-minute
// default. `PINNED_GATE_JOB_KEYS` is a COMPLETE key-set pin (M2-k) —
// adding a real job-level key without updating this constant in the same
// change would make the pin fail on genuinely correct config.
const PINNED_GATE_JOB_KEYS: &[&str] =
    &["if", "name", "needs", "runs-on", "steps", "timeout-minutes"];
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

/// PINNED, human-reviewed exact text of the `ci-gate` job's own job-level
/// `needs:` value (S-626-1 pass-55, ADV-P55-HIGH-001 part (c)).
///
/// `parse_needs_set` (above) is the SET-membership extractor every other
/// test in this file relies on; it is now depth-anchored and panics on a
/// duplicate job-level `needs:` key (parts (a)/(b) of this same finding),
/// but nothing byte-pins the line's own TEXT the way `PINNED_GATE_RUN_LINE`
/// and `PINNED_GATE_NEEDS_JSON_LINE` pin theirs. `needs:` is exactly as
/// load-bearing as those two: it is the entire membership list
/// `check-ci-gate.sh` evaluates via `toJSON(needs)`, and every set-based
/// pin in this file is still, ultimately, downstream of one text line. A
/// byte-for-byte pin here closes the class the same way, rather than
/// leaving `needs:` as the one job-level key in `PINNED_GATE_JOB_KEYS`
/// with a presence pin but no value pin.
const PINNED_GATE_NEEDS_LINE: &str =
    "[fmt, clippy, test, msrv, deny, spec-guard, check-signing-workflow-injection, mutants]";

/// Extract and normalize the SOLE job-level `needs:` line (4-space indent)
/// for pinned-literal comparison against `PINNED_GATE_NEEDS_LINE`.
///
/// A deliberately separate function from `extract_and_normalize_sole_run_line`
/// / `_needs_json_line` rather than a generalization of either — same
/// precedent those two functions' own doc comments cite: reject-don't-parse
/// normalization is duplicated, not shared, so a bug in one byte-pin
/// extractor cannot silently widen into another. Only the indent depth (4,
/// for a job-level key, vs. 8/10 for a step-level or env-child key) and the
/// key name differ from `extract_and_normalize_sole_needs_json_line`.
///
/// Only the inline-array form (`needs: [a, b, c]`) is supported for
/// pinning — `ci.yml`'s current convention — mirroring `parse_needs_set`'s
/// own inline-array-first handling. A same-line-empty value (block-list
/// form) is `Err`, not silently treated as "no pin to check": this checker
/// refuses to guess at a form it was not built to normalize.
fn extract_and_normalize_sole_needs_line(job_block: &str) -> Result<String, String> {
    let lines: Vec<&str> = job_block.lines().collect();
    let needs_line_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| extract_key_name_at_indent(l, 4).as_deref() == Some("needs"))
        .map(|(i, _)| i)
        .collect();

    if needs_line_indices.is_empty() {
        return Err(
            "has no job-level `needs:` line at 4-space indent — `ci-gate` \
             must declare which upstream jobs it aggregates."
                .to_string(),
        );
    }
    if needs_line_indices.len() > 1 {
        return Err(format!(
            "has {} job-level `needs:` lines — this checker requires \
             exactly one so a single pinned literal unambiguously covers \
             the aggregated job set. This is ALSO invalid YAML (a \
             duplicate mapping key) that GitHub Actions and actionlint \
             both reject at parse time; this checker refuses to silently \
             pick a winner rather than rely on that external validation.",
            needs_line_indices.len()
        ));
    }
    let idx = needs_line_indices[0];

    let line = lines[idx];
    // S-626-1 pass-59 (ADV-P57-INFO-003): the key was detected via
    // `extract_key_name_at_indent` (quote/whitespace-aware), but this
    // VALUE re-read used to be a bare `strip_prefix("needs:").unwrap_or(
    // "")` — a quoted key spelling (`"needs":`/`'needs':`) or `needs :`
    // (space before colon) silently collapsed `raw` to `""`, which then
    // fell into the `is_empty()` branch below and reported the MISLEADING
    // message "has an empty same-line `needs:` value ... a block-list \
    // form ... cannot be safely represented" — actively wrong for a
    // quoted-key job block, which has neither an empty value nor a
    // block-list form. Same key-detect vs. value-reparse swallow shape
    // the `910b8ab0` sweep closed elsewhere; fixed the same way (a loud,
    // specific `Err` on the re-read itself) rather than leaving a
    // downstream branch to misdiagnose the symptom.
    let Some(raw) = line.trim_start().strip_prefix("needs:") else {
        return Err(format!(
            "has a job-level `needs:` key detected via the quote/\
             whitespace-aware matcher at 4-space indent, but this \
             function's own value-extraction re-read (a bare \
             `strip_prefix(\"needs:\")`) could not parse the same line — \
             most likely a quoted key spelling (`\"needs\":` / \
             `'needs':`) or `needs :` (space before colon), which \
             `extract_key_name_at_indent` recognizes but this bare \
             re-read does not.\n\
             Offending line: {line:?}"
        ));
    };
    let raw_value_leading_trimmed = raw.trim_start();

    if raw_value_leading_trimmed.starts_with('>') || raw_value_leading_trimmed.starts_with('|') {
        return Err(format!(
            "uses a YAML block-scalar form (\"{}\") for its `needs:` value \
             — the real list lives on continuation lines this checker does \
             not read, so it cannot be safely represented as a single \
             pinned literal.",
            raw_value_leading_trimmed.trim()
        ));
    }

    if raw_value_leading_trimmed.is_empty() {
        return Err("has an empty same-line `needs:` value — this checker only \
             supports the inline-array form (`needs: [a, b, c]`), \
             `ci.yml`'s current convention; a block-list form (items on \
             following `- item` lines) cannot be safely represented as a \
             single pinned literal by this function."
            .to_string());
    }

    if let Some(next_line) = lines[idx + 1..].iter().find(|l| {
        let trimmed = l.trim_start();
        !trimmed.is_empty() && !trimmed.starts_with('#')
    }) {
        let indent = next_line.len() - next_line.trim_start().len();
        if indent > 4 {
            return Err(format!(
                "has a `needs:` value that appears to continue onto a \
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
            "has a `needs:` value containing a `#` that is not a clearly \
             whitespace-delimited trailing comment (\"{}\") — this cannot \
             be safely normalized.",
            raw.trim()
        ));
    }

    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return Err("has an empty `needs:` value.".to_string());
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

        // S-626-1 pass-59 (ADV-P57-INFO-001): routed through
        // `extract_key_name_at_indent`, the same quote/whitespace-aware
        // matcher used throughout this file, rather than a raw
        // `l.starts_with("    if:")` — this was the THIRD such site the
        // `910b8ab0` class sweep missed (M2-a/M2-d were the other two;
        // see `test_ci_gate_pass_fail_semantics_are_structurally_placed`).
        // Also drops the dead conjunct `&& !l.starts_with("        ")`,
        // which — as round 10 already noted when removing the same dead
        // conjunct from `line_declares_job_level_key` — could never be
        // `false` once the 4-space-prefix check had already matched (no
        // line can simultaneously start with exactly 4 spaces AND 8
        // spaces). This test is a faster diagnostic layered on top of the
        // behavioral closure in
        // `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`
        // (see this function's own doc comment) — fail-closed either way,
        // consistency fix only.
        let has_job_level_if = job_block
            .lines()
            .any(|l| extract_key_name_at_indent(l, 4).as_deref() == Some("if"));

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

// ---------------------------------------------------------------------------
// S-626-1 — DEC-246 follow-on hardening: sibling-workflow exposure (Guard A)
// and matrix staticity (Guard B)
// ---------------------------------------------------------------------------
//
// Basis: `.factory/research/dec-246-github-actions-gating-semantics.md`
// (2026-08-09), specifically §"Sibling-workflow frontier" (Guard A) and
// the "New material this reconstruction contributes" item 1 (Guard B).
//
// KNOWN LIMITATION SHARED BY BOTH GUARDS BELOW (round-16 residual, restated
// here rather than assumed known): every extractor in this file, including
// the two new ones added for these guards, is LINE-BASED. A YAML NODE
// PROPERTY (an anchor `&name` or a tag `!tag`/`!!tag`) prefixing a mapping
// key on the same physical line defeats line-based key detection with zero
// non-LF bytes involved and zero line breaks — `extract_key_name_at_indent`
// stops at the space after `&x`/`!!str`, sees no colon, and returns `None`.
// Neither guard below closes that residual; both inherit it exactly as
// every other set-equality pin in this file does. It is tracked as
// follow-up story S-CIGATE-3 (durable YAML-parser rewrite) — see
// `CLAUDE.md`'s CI Gate section, "Round 16", for the full record. Do not
// read either guard below as covering the line-based-lexer-vs-real-parser
// gap generally; neither does.

/// Read an arbitrary workflow YAML file, applying the same normalization as
/// `read_ci_yml` (CRLF -> LF, strip a leading BOM) so downstream line-based
/// scanning behaves identically regardless of which workflow file is read.
/// A deliberately separate function from `read_ci_yml` (which is hardcoded
/// to `.github/workflows/ci.yml`) rather than a generalization of it — this
/// file's established precedent (see `extract_and_normalize_sole_run_line`
/// vs. `extract_and_normalize_if_expr`) is to duplicate small, load-bearing
/// normalization logic rather than risk widening a more heavily-relied-on
/// function's contract.
fn read_workflow_file(path: &Path) -> String {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    raw.replace("\r\n", "\n")
}

/// Enumerate every `.github/workflows/*.yml` / `*.yaml` file, sorted for
/// deterministic iteration order.
///
/// Glob-based over the directory — deliberately NOT a hardcoded file list.
/// A hardcoded list would reproduce, one level up, the exact
/// closed-enumeration defect S-626-1's U1 finding closed for
/// `ci-gate.needs` (`test_ci_gate_needs_partitions_all_ci_yml_jobs`): a new
/// sibling workflow file added later would silently sit outside a
/// hand-maintained list, and this guard exists specifically to prevent
/// that class of gap for the sibling-workflow-name vector.
fn list_workflow_files() -> Vec<std::path::PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    let mut files: Vec<std::path::PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", dir.display()))
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("yml") | Some("yaml")
            )
        })
        .collect();
    files.sort();
    files
}

/// List every job id defined under a workflow file's top-level `jobs:` map.
///
/// Deliberately a SEPARATE function from `list_all_ci_yml_job_names` rather
/// than a generalization of it — same precedent cited on
/// `read_workflow_file` above. A file with genuinely EMPTY content (or only
/// whitespace) returns an empty `Vec` rather than panicking — a sibling
/// workflow file is not on the gate's decision path, so an incidentally
/// content-free file is a legitimate no-op for Guard A, not a hard
/// failure.
///
/// S-626-1 pass-54 (ADV-P54-MED-002): a file with REAL, non-empty content
/// but no detectable `jobs:` key is different — that shape is exactly what
/// a malformed or unusually-formatted (but not empty) sibling workflow
/// file looks like, and silently returning an empty `Vec` for it would
/// make Guard A a silent no-op for a file that may genuinely define a job
/// this checker never got a chance to inspect — the same closed-
/// enumeration blind spot the U1 finding closed one level up for
/// `ci-gate.needs`. Panics loudly instead, naming the file. Both the
/// `jobs:` key itself (0-indent) and each job id under it (2-indent) are
/// detected via `collect_mapping_key_set` — the same quote/whitespace-
/// aware, comment-and-blank-line-tolerant primitive
/// `list_all_ci_yml_job_names` above now also uses — rather than the
/// original bespoke `strip_prefix("  ").and_then(|s| s.strip_suffix(':'))`
/// scan, which required the job-id line to END with `:` and was blind to
/// a flow-style job entry (e.g. `gate: {name: CI Gate, ...}`) for the same
/// reason ADV-P55-MED-002 fixed in `list_all_ci_yml_job_names`.
fn list_job_ids_in_workflow(content: &str, path: &Path) -> Vec<String> {
    if content.trim().is_empty() {
        return Vec::new();
    }

    let lines: Vec<&str> = content.lines().collect();
    let Some(jobs_line_idx) = lines
        .iter()
        .position(|l| extract_key_name_at_indent(l, 0).as_deref() == Some("jobs"))
    else {
        panic!(
            "FAIL (S-626-1 pass-54, ADV-P54-MED-002): {} has non-empty \
             content but no detectable top-level `jobs:` key (checked via \
             the same quote/whitespace-aware matcher used everywhere else \
             in this file). A malformed or unusually-formatted sibling \
             workflow file that genuinely defines jobs would otherwise \
             silently sit outside Guard A's coverage entirely — the exact \
             closed-enumeration shape the U1 finding closed one level up \
             for `ci-gate.needs`. If this file legitimately has no \
             `jobs:` key at all (e.g. a reusable-workflow fragment with \
             only top-level metadata), narrow this panic; do not silently \
             return an empty Vec for a file with real content.",
            path.display()
        );
    };

    collect_mapping_key_set(&lines, jobs_line_idx + 1, 2)
}

/// Extract a job block's job-level `name:` value (4-space indent), trimmed
/// and with one layer of matching surrounding quotes stripped. A trailing
/// YAML comment on the same line is stripped before the quote check, same
/// convention `extract_job_block` already uses for job-key lines. Returns
/// `None` if the job block has no job-level `name:` key at all (GitHub then
/// displays the job id itself as the check name).
///
/// Anchored on `extract_key_name_at_indent(line, 4)` so a step-level
/// `      - name: ...` line (6-space indent, one level deeper) is never
/// mistaken for the job's own `name:` — the same indent discipline
/// `extract_job_level_key_set` already relies on for job-level keys
/// generally.
///
/// S-626-1 pass-56 (ADV-P56-HIGH-002 + ADV-P55-MED-003): two distinct
/// fail-open gaps closed together, both diagnosed as the same root cause
/// as ADV-P56-HIGH-001 below — detect the key with the quote/whitespace-
/// aware matcher, then re-read the VALUE with a bare, non-quote-aware
/// re-parse, silently swallowing the mismatch:
///   1. (HIGH-002) `line.trim_start().strip_prefix("name:")?` used `?` to
///      propagate a `None` up through the WHOLE function on ANY line whose
///      bare re-read didn't match — including a quoted key spelling
///      (`"name":` / `'name':`) or `name :` (space before colon), both of
///      which `extract_key_name_at_indent` above already recognizes as
///      declaring `name`. That `None` is indistinguishable from "this job
///      declares no `name:` key at all", silently letting a job spelled
///      that way escape Guard A's sibling-workflow check entirely. Fixed:
///      `unwrap_or_else` now panics loudly instead, naming the offending
///      line — this function refuses to guess at a value it detected the
///      KEY for but cannot safely re-parse.
///   2. (MED-003) even once the value is reached, comparing its raw SOURCE
///      bytes against a plain-scalar constant like `"CI Gate"` silently
///      misses two YAML forms that render to the identical string: a
///      block-scalar (`name: >-` with the text on continuation lines this
///      function does not read) and a double-quoted scalar containing a
///      backslash escape (`"\x43I Gate"` — this function does not
///      interpret YAML escape sequences). Reject-don't-parse, the same
///      discipline `extract_and_normalize_if_expr` uses for its `${{ }}`
///      wrapper: panic rather than guess at folding/escape rules this
///      checker was never built to interpret.
///
/// S-626-1 pass-59 (ADV-P57-HIGH-001): a third gap, one step EARLIER than
/// either of the two above — this function's `extract_key_name_at_indent
/// (line, 4)` scan silently finds nothing at all (not a wrong VALUE, no
/// KEY detected in the first place) when the job body is written at any
/// indent other than 4 spaces, which is legal, `actionlint`-clean YAML.
/// Verified: a sibling-workflow job with `name: CI Gate` at 6-space
/// indent returned `None` here — indistinguishable from "declares no
/// name" — leaving Guard A's `Some("CI Gate")` comparison silently
/// unable to catch the exact duplicate-check-name collision it exists to
/// detect. See `assert_job_block_uses_4_space_child_indent`'s doc comment
/// for the shared root-cause analysis (also applied to
/// `matrix_needs_members`).
fn extract_job_display_name(job_block: &str) -> Option<String> {
    assert_job_block_uses_4_space_child_indent(
        job_block,
        "S-626-1 Guard A, ADV-P57-HIGH-001 (extract_job_display_name)",
    );
    for line in job_block.lines() {
        if extract_key_name_at_indent(line, 4).as_deref() != Some("name") {
            continue;
        }
        // S-626-1 pass-60 (ADV-P60-HIGH-002): a job-level key can never be
        // a YAML sequence entry — `jobs.<id>` is a mapping, and its own
        // keys (`name:`, `runs-on:`, `steps:`, ...) are plain mapping
        // keys, never `- `-prefixed list items. A `- ` marker at this
        // indent belongs to a SEQUENCE living under an earlier job-level
        // key — in practice, `steps:` written at the SAME 4-space indent
        // as the job's own keys, which is ordinary, `actionlint`-clean
        // YAML (a block sequence may sit at its parent mapping key's
        // indent, not strictly deeper). `extract_key_name_at_indent`
        // above deliberately strips a leading `- ` marker so it can ALSO
        // extract a STEP's first key (see that function's own doc
        // comment) — which means a 4-space `    - name: Checkout` step
        // line is indistinguishable from a genuine job-level `name:`
        // line to that one call alone. The prior revision of this
        // function leaned into that ambiguity instead of resolving it:
        // it stripped the SAME marker before its own value re-read
        // below, so it silently substituted the step's name
        // (`"Checkout"`) for the job's and returned early — the job's
        // real, later `name: CI Gate` line was never reached. Verified
        // directly: a sibling-workflow job with exactly this shape
        // (4-space `steps:` children, no job-level `name:` before them,
        // a real `name: CI Gate` after) made Guard A's sibling-duplicate
        // check return `Some("Checkout")` instead of `Some("CI Gate")`
        // — 27 passed, 0 failed, silently missing the exact collision
        // this guard exists to detect.
        //
        // The prior doc comment on this branch claimed the reachable
        // shape was latent because "`jobs.<id>` must be a YAML MAPPING
        // … a job whose value is a sequence is rejected by GitHub's own
        // parser" — that is true but answers a different question. The
        // actual reachable shape is NOT a sequence-valued `jobs.<id>`;
        // it is this ordinary `steps:`-at-4-space-indent style, which
        // GitHub accepts every day. Fixed: skip any 4-space line that is
        // itself a sequence entry, so this loop only ever considers
        // genuine job-level mapping keys — and the marker strip is
        // dropped from the value re-read below, so a genuinely
        // unparseable spelling (a quoted key, `name :` with a space
        // before the colon, ...) still panics loudly rather than being
        // silently misread.
        if line.trim_start().starts_with("- ") {
            continue;
        }
        let after_key = line.trim_start().strip_prefix("name:").unwrap_or_else(|| {
            panic!(
                "FAIL (S-626-1 Guard A, ADV-P56-HIGH-002): a job-level \
                 `name:` key was detected via the quote/whitespace-aware \
                 matcher at 4-space indent, but this function's own \
                 value-extraction re-read (a bare `strip_prefix(\"name:\")`) \
                 could not parse the same line — most likely a quoted key \
                 spelling (`\"name\":` / `'name':`) or `name :` (space \
                 before colon), which `extract_key_name_at_indent` \
                 recognizes but this bare re-read does not. Silently \
                 returning `None` here would be indistinguishable from \
                 \"this job declares no name\" and would let a job spelled \
                 this way escape Guard A's sibling-workflow check \
                 entirely.\n\
                 Offending line: {line:?}"
            )
        });
        let value = after_key.trim();
        let value = value.split('#').next().unwrap_or(value).trim();

        if value.starts_with('>') || value.starts_with('|') {
            panic!(
                "FAIL (S-626-1 Guard A, ADV-P55-MED-003): a job-level \
                 `name:` value uses a YAML block-scalar form (\"{value}\") \
                 — the real rendered name lives on continuation lines this \
                 checker does not read, so it cannot be safely compared \
                 against a plain-scalar constant like `\"CI Gate\"`. This \
                 checker refuses to guess at block-scalar folding rules \
                 rather than risk silently missing a spelling of the exact \
                 same rendered name Guard A exists to catch.\n\
                 Offending line: {line:?}"
            );
        }

        // S-626-1 pass-59 (ADV-P57-LOW-001): a value beginning with `*`
        // (an alias reference), `&` (an anchor declaration — usually
        // paired with a `*`-referenced value elsewhere), or `!` (an
        // explicit YAML tag, e.g. `!!str`) is a node-property form this
        // checker cannot resolve from this one line alone: an alias's
        // real text lives at its `&anchor` definition elsewhere in the
        // file, and a tag does not change the scalar's rendered text but
        // this checker does not interpret tag semantics to know that.
        // Verified (PyYAML AND Ruby Psych, independent implementations):
        // `name: *nm` (with `x-tpl: &nm CI Gate` declared elsewhere) and
        // `name: !!str CI Gate` both render to the plain string `CI
        // Gate` — exactly the duplicate-check-name collision Guard A
        // exists to catch — while comparing the literal text `"*nm"` or
        // `"!!str CI Gate"` against `"CI Gate"` would silently miss it.
        // Same class this file's CLAUDE.md documents as "round 16 —
        // UNGUARDED, code review is the control": this checker refuses to
        // guess at anchor/alias/tag resolution rather than risk silently
        // missing a spelling of the identical rendered name.
        if value.starts_with('*') || value.starts_with('&') || value.starts_with('!') {
            panic!(
                "FAIL (S-626-1 Guard A, ADV-P57-LOW-001): a job-level \
                 `name:` value uses a YAML alias/anchor/tag form \
                 (\"{value}\") — an alias (`*name`), anchor (`&name`), or \
                 explicit tag (`!tag`/`!!str`) renders to a string this \
                 checker cannot resolve from this line alone (an alias's \
                 real text lives at its `&anchor` definition elsewhere in \
                 the file; a tag does not change the scalar's rendered \
                 text, but this checker does not interpret tag semantics \
                 to know that). This checker refuses to guess at \
                 anchor/alias/tag resolution rather than risk silently \
                 missing a spelling of the exact same rendered name Guard \
                 A exists to catch.\n\
                 Offending line: {line:?}"
            );
        }

        for quote in ['"', '\''] {
            if let Some(stripped) = value.strip_prefix(quote) {
                if let Some(stripped) = stripped.strip_suffix(quote) {
                    if quote == '"' && stripped.contains('\\') {
                        panic!(
                            "FAIL (S-626-1 Guard A, ADV-P55-MED-003): a \
                             job-level `name:` value is a double-quoted \
                             YAML scalar containing a backslash escape \
                             (\"{stripped}\") — this checker treats \
                             double-quoted values as opaque, unescaped \
                             text and does not interpret YAML escape \
                             sequences (`\\\"`, `\\x43`, `\\n`, ...), so it \
                             cannot safely compare this against a \
                             plain-scalar constant like `\"CI Gate\"` \
                             without risking a silent miss on an escaped \
                             spelling of the identical rendered name.\n\
                             Offending line: {line:?}"
                        );
                    }
                    return Some(stripped.to_string());
                }
            }
        }
        return Some(value.to_string());
    }
    None
}

/// S-626-1 Guard A (DEC-246 §"Sibling-workflow frontier"): branch
/// protection matches a required status check by the job's `name:` STRING
/// ALONE — the workflow FILE that declares the job is not part of the
/// check's identity. GitHub's own docs state plainly that "[u]sing the
/// same job name in multiple workflows can cause **ambiguous** status
/// check results" and instruct keeping job names unique across all
/// workflows
/// (<https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches>,
/// verified 2026-08-09 — see DEC-246 Q5).
///
/// **PREMISE LABEL CORRECTED (S-626-1 research pass, 2026-08-10, Q-D):** the
/// specific claim that a duplicate `CI Gate` check NAME yields a false
/// green has been carried since DEC-246 as though established; it must be
/// labelled **INFERRED — neither verified nor refuted**, not established
/// fact. What IS confirmed (CONFIRM, primary, per the docs quoted above):
/// the check name alone is the identity, and the declaring workflow file is
/// not part of it. What resolution TWO check runs sharing that name
/// actually get (all-must-pass / last-writer-wins / something else) is
/// INCONCLUSIVE on primary sources. The leading hypothesis is
/// **last-writer-wins** (most-recently-updated check run governs the
/// required-check state), supported only by: a GitHub staff member's
/// personal blog (Ken Muse, "Creating GitHub Checks",
/// SEMI-AUTHORITATIVE SECONDARY, not documentation — "If multiple Checks
/// exist with the same name, only the most recently updated one will be
/// used for the Status"), and the REST Checks API's `filter=latest`
/// affordance on "List check runs for a Git reference" (an inference from
/// an API design choice, not a documented statement about branch
/// protection resolution). **Precision that must not be lost:** the docs
/// sentence "[i]f a check and a commit status have the same name, both must
/// pass when that name is required" concerns a check run vs. a commit
/// status — two different API objects — and is NOT evidence about two check
/// runs sharing a name; do not cite it as such. See
/// `.factory/research/gh-actions-open-semantics-2026-08-10.md` §Q-D.
///
/// **This guard is kept regardless of that label.** It is cheap, fully
/// decidable from the repository alone, needs no live experiment, and
/// prevents a state GitHub's own documentation instructs maintainers not to
/// create in the first place ("ambiguous status check results"). Every
/// existing pin in this file reads `ci.yml` alone (via
/// `read_ci_yml`/`extract_job_block`) — a job named `CI Gate` declared in
/// ANY OTHER workflow file is outside every one of those pins BY
/// CONSTRUCTION, structurally the same blind spot as the workflow-level
/// `defaults:` vector found in round 11 (see
/// `common::yaml::extract_job_block`'s doc comment for that precedent).
///
/// This test enumerates every `.github/workflows/*.yml`/`*.yaml` file via
/// `list_workflow_files` (glob-based, not a hardcoded list — see that
/// function's doc comment) and asserts that no workflow file OTHER than
/// `ci.yml` declares a job whose `name:` value, after trimming and
/// unquoting, equals `CI Gate` case-sensitively (GitHub check names are
/// case-sensitive).
///
/// See the module-level "KNOWN LIMITATION SHARED BY BOTH GUARDS" note above
/// this section — this test is line-based and shares the round-16
/// node-property residual like every other pin in this file.
#[test]
fn test_no_sibling_workflow_declares_a_job_named_ci_gate() {
    for path in list_workflow_files() {
        if path.file_name().and_then(|f| f.to_str()) == Some("ci.yml") {
            continue;
        }

        let content = read_workflow_file(&path);
        for job_id in list_job_ids_in_workflow(&content, &path) {
            // S-626-1 pass-55, ADV-P55-MED-001: `list_job_ids_in_workflow`
            // detected `job_id` under `jobs:`, but `extract_job_block`
            // could not anchor to it — its needle requires an EXACT
            // `  {job_id}:\n` line with no trailing comment (e.g. a
            // legitimate `  gate:  # comment` job-key spelling, the exact
            // shape `extract_job_block`'s own doc comment cites as its
            // motivating example). Two extractors in this file
            // disagreeing about whether a job exists is a precondition
            // violation this check refuses to silently paper over by
            // skipping the job — that silent skip is exactly how a job
            // spelled with a `name: CI Gate` and a trailing-comment job
            // key would escape Guard A entirely.
            let Some(job_block) = extract_job_block(&content, &job_id) else {
                panic!(
                    "FAIL (S-626-1 Guard A, ADV-P55-MED-001): {} — \
                     `list_job_ids_in_workflow` detected job id `{job_id}` \
                     under `jobs:`, but `extract_job_block` could not \
                     anchor to it. This checker refuses to silently skip \
                     a job two of its own extractors disagree about the \
                     existence of — investigate the job-key line's exact \
                     spelling (a trailing comment, quoting, or unusual \
                     whitespace are the likely causes) rather than treat \
                     this as \"nothing to check\".",
                    path.display()
                );
            };
            if extract_job_display_name(job_block).as_deref() == Some("CI Gate") {
                panic!(
                    "FAIL (S-626-1 Guard A, DEC-246 Sibling-workflow frontier): \
                     {} declares job `{job_id}` with `name: CI Gate` — the SAME \
                     required-check name as `.github/workflows/ci.yml`'s \
                     `ci-gate` job.\n\
                     \n\
                     Branch protection matches a required status check by job \
                     NAME ALONE; the declaring workflow file is not part of the \
                     check's identity. GitHub's own docs state that using the \
                     same job name in multiple workflows \"can cause ambiguous \
                     status check results\" and instruct keeping job names \
                     unique across all workflows. Every pin in \
                     tests/ci_gate_completeness.rs reads ci.yml alone — a \
                     second `CI Gate` check produced by this job sits outside \
                     every one of those pins by construction.\n\
                     \n\
                     Fix: rename this job's `name:` to something other than \
                     `CI Gate`.",
                    path.display()
                );
            }
        }
    }
}

/// PINNED count of `ci-gate.needs` members that carry a build matrix
/// (today: `clippy`, `test`). S-626-1 pass-59 (ADV-P57-HIGH-001): an
/// exact-arity pin, not a mere non-empty check — see
/// `test_matrix_os_lists_remain_static_literals`'s assertion for why a
/// non-empty check alone cannot detect losing ONE of several matrix
/// jobs. Update in the SAME change as any deliberate addition/removal
/// of a build-matrix job.
///
/// S-626-1 pass-60 (ADV-P60-LOW-002): this constant declaration used to
/// sit BETWEEN Guard B's ~130-line rationale docstring and the
/// `#[test] fn` it explains — which meant that entire docstring was
/// actually the rustdoc of THIS `usize` constant, not of the test, and
/// the test itself had no doc comment of its own (a later pass-60
/// change to the block above this one, still calling it "the docstring
/// of Guard B", repeated the same misattribution rather than
/// correcting it). Moved above the docstring so rustdoc attaches
/// correctly: this short paragraph documents the constant, and Guard
/// B's full rationale below documents the test.
const PINNED_MATRIX_NEEDS_MEMBER_COUNT: usize = 2;

/// S-626-1 Guard B (DEC-246 §Q4 + "New material this reconstruction
/// contributes" item 1): `ci-gate.needs` includes two matrix jobs, `clippy`
/// and `test`. What `needs.<job>.result` reports when a matrix job expands
/// to ZERO legs is UNDOCUMENTED by GitHub — tracked as the open drift item
/// `ZERO-LEG-MATRIX-RESULT-UNDOCUMENTED`. GitHub's own docs are silent on
/// matrix-parent -> `needs.result` aggregation entirely.
///
/// **CORRECTED CLAIM (S-626-1 research pass, 2026-08-10, Q-A) — REFUTE of
/// the prior "community reports split" characterization:** an earlier
/// revision of this docstring stated community reports were "split between
/// `skipped` (safe) and `success` (a silent false green)". That
/// characterization does not hold up: no report claiming either outcome for
/// an actual zero-leg job was found. The two mechanisms that DO exist are
/// different and neither produces a zero-leg job: (1) a dynamic
/// `fromJSON()` matrix evaluating to an empty list HARD-ERRORS at strategy
/// evaluation, before any step runs (`orgs/community#27096`, 2021-06-10,
/// SECONDARY, no staff reply: `"Matrix vector 'cfg' does not contain any
/// values"`) — that is fail-CLOSED, not a false green, though whether a
/// strategy-evaluation-stage error maps to `failure` in `needs` (as opposed
/// to a step-stage failure) is itself UNVERIFIED, a different lifecycle
/// stage GitHub does not document; (2) the apparent "success" half traced
/// to `orgs/community#9141` is, read verbatim, about a *per-step `if:`
/// workaround* (copying the job-level condition onto every step) that
/// makes every leg still expand and every step skip, legitimately
/// concluding `success` — ordinary documented semantics for a job whose
/// steps all skip, not evidence about a zero-leg matrix at all. So it is
/// not established that a zero-leg job state is reachable by ANY
/// construction; property (1) below (no `${{ }}`/`fromJSON` in `os:`) is
/// kept as defense-in-depth against that unmapped lifecycle edge — a real
/// but modest justification — not because a "might report `success`"
/// false-green claim the evidence does not support. Retiring the guard on
/// the strength of one five-year-old, staff-unconfirmed forum thread would
/// repeat this cluster's founding mistake in the opposite direction.
/// See `.factory/research/gh-actions-open-semantics-2026-08-10.md` §Q-A.
///
/// DEC-246 established that the reachability question is currently
/// UNREACHABLE in this file: both matrix jobs use STATIC LITERAL `os:` lists
/// (`[ubuntu-latest, windows-latest]` and `[ubuntu-latest, macos-latest,
/// windows-latest]`). The zero-leg case becomes reachable if a future edit
/// converts one of these to a DYNAMIC matrix (e.g. `fromJSON(...)`) — that
/// vector is what the assertion below (the `${{ }}` / `fromJSON` check)
/// exists to catch.
///
/// CORRECTED CLAIM (S-626-1 pass-57, `ADV-P56-INFO-001`): an earlier
/// revision of this docstring claimed "a static literal list cannot ever
/// evaluate to zero legs" without qualification. That overstates it — a
/// static literal `os:` list cannot ever EXPAND to zero legs on its own,
/// but GitHub Actions' `strategy.matrix.exclude:` key can remove
/// combinations from an already-expanded static list, and a fully-excluded
/// matrix (every generated combination removed) is a second, independent
/// path to the same undocumented zero-leg question — orthogonal to
/// `fromJSON`/`${{ }}` dynamism.
///
/// **STRENGTHENED (S-626-1 research pass, 2026-08-10, Q-B) — supersedes the
/// "UNVERIFIED" note this replaces:** an all-excluding `exclude:` is NOT
/// rejected at parse time. `orgs/community#179993` (2025-11-19, SECONDARY,
/// single report, bot-only reply, no staff confirmation), "Matrix exclude
/// produces an empty matrix.entry instead of skipping the job", reports the
/// job runs ONCE with matrix variables empty rather than expanding to zero
/// legs — a degenerate run that CAN conclude `success` having done nothing.
/// That is precisely the false-green shape this property exists to prevent,
/// and it is better-evidenced now than "UNVERIFIED" reflected. (One
/// unasserted secondary mitigation: `clippy`/`test` both use
/// `runs-on: ${{ matrix.os }}`, and with matrix variables empty `runs-on`
/// evaluates to the empty string — what that actually does on a real
/// runner is not established here and is NOT relied on as a safety
/// property.) See
/// `.factory/research/gh-actions-open-semantics-2026-08-10.md` §Q-B. Today
/// neither matrix job declares an `exclude:` key at all (verified, and
/// pinned by the assertion below) — the `exclude:` vector is CLOSED BY
/// SOURCE PROPERTY, not by an argument about what `exclude:` can or cannot
/// produce at runtime.
///
/// This test converts the undecidable RUNTIME question ("what does a
/// zero-leg matrix report to `needs`?") into two decidable SOURCE
/// properties — far cheaper than a live empirical probe, and correct for
/// exactly as long as they hold: (1) both `clippy` and `test`'s
/// `strategy.matrix.os:` value contains neither a `${{ }}` expression nor
/// a `fromJSON` call, and (2) neither job's `strategy.matrix:` mapping
/// declares an `exclude:` key at all. Adding an `exclude:` key to either
/// matrix reopens the zero-leg question this test currently keeps closed —
/// it must be resolved (see `ZERO-LEG-MATRIX-RESULT-UNDOCUMENTED` above)
/// before that change lands, not inferred.
///
/// See the module-level "KNOWN LIMITATION SHARED BY BOTH GUARDS" note above
/// this section — this test is line-based and shares the round-16
/// node-property residual like every other pin in this file.
#[test]
fn test_matrix_os_lists_remain_static_literals() {
    let ci = read_ci_yml();

    // S-626-1 pass-56, ADV-P56-LOW-002: derived from the live `needs:` set
    // (see `matrix_needs_members`'s doc comment) rather than the prior
    // hardcoded `["clippy", "test"]` literal.
    let matrix_job_ids = matrix_needs_members(&ci);
    // S-626-1 pass-59 (ADV-P57-HIGH-001 arity check): `!is_empty()` only
    // proves Guard B has SOMETHING to check — losing one of TWO matrix
    // jobs (today: `clippy`, `test`) is invisible to a non-empty check,
    // since the set still has ≥1 member either way. Pin the exact count
    // instead, mirroring every other exact-arity pin in this file (e.g.
    // `PINNED_GATE_JOB_KEYS`'s set-equality). Update
    // `PINNED_MATRIX_NEEDS_MEMBER_COUNT` in the SAME change as any
    // deliberate addition/removal of a build-matrix job.
    assert_eq!(
        matrix_job_ids.len(),
        PINNED_MATRIX_NEEDS_MEMBER_COUNT,
        "FAIL (S-626-1 Guard B, ADV-P57-HIGH-001): `matrix_needs_members` \
         returned {} member(s) ({matrix_job_ids:?}), not the pinned count \
         of {PINNED_MATRIX_NEEDS_MEMBER_COUNT}. A non-empty check alone \
         cannot detect losing ONE of several matrix jobs (e.g. `clippy` \
         silently dropping out while `test` remains) — the set would \
         still be non-empty either way. If `clippy`/`test` (or a future \
         matrix job) genuinely gained or lost a build matrix, this is \
         expected; update PINNED_MATRIX_NEEDS_MEMBER_COUNT in the SAME \
         change. If it fires unexpectedly, `strategy:`'s indent, \
         `matrix_needs_members`'s `needs:` derivation, or a job's \
         extracted block may be broken.",
        matrix_job_ids.len()
    );

    for job_id in &matrix_job_ids {
        let job_block = extract_job_block(&ci, job_id)
            .unwrap_or_else(|| panic!("FAIL: no `{job_id}:` job in ci.yml."));

        let lines: Vec<&str> = job_block.lines().collect();
        let os_line_idx = lines
            .iter()
            .position(|l| extract_key_name_at_indent(l, 8).as_deref() == Some("os"))
            .unwrap_or_else(|| {
                panic!(
                    "FAIL (S-626-1 Guard B): `{job_id}` has no \
                     `strategy.matrix.os:` line at the expected 8-space \
                     indent — has this job's matrix shape changed? If `os:` \
                     moved to a different indent or key name, update this \
                     test's anchor alongside the ci.yml change."
                )
            });
        let os_line = lines[os_line_idx];

        // ADV-P56-HIGH-001: the key was detected via the quote/whitespace-
        // aware `extract_key_name_at_indent` above, but this bare
        // `strip_prefix("os:")` re-read of the SAME line does not
        // recognize a quoted key spelling (`"os":` / `'os':`) or `os :`
        // (space before colon). The prior `.unwrap_or("")` silently
        // collapsed that mismatch to an empty string, and an empty string
        // trivially satisfies `!"".contains("${{") &&
        // !"".contains("fromJSON")` — CERTIFYING a dynamic matrix as
        // static rather than merely missing it. Panic loudly instead: this
        // checker refuses to guess at a value it detected the KEY for but
        // cannot safely re-parse.
        let raw_value = os_line
            .trim_start()
            .strip_prefix("os:")
            .unwrap_or_else(|| {
                panic!(
                    "FAIL (S-626-1 Guard B, ADV-P56-HIGH-001): `{job_id}`'s \
                     `strategy.matrix.os:` key was detected via the \
                     quote/whitespace-aware matcher at 8-space indent, but \
                     this checker's own value-extraction re-read (a bare \
                     `strip_prefix(\"os:\")`) could not parse the same \
                     line — most likely a quoted key spelling (`\"os\":` / \
                     `'os':`) or `os :` (space before colon). Silently \
                     collapsing to an empty string here would make \
                     `!\"\".contains(\"${{{{\") && \
                     !\"\".contains(\"fromJSON\")` evaluate TRUE, \
                     CERTIFYING a dynamic matrix as static rather than \
                     merely missing it.\n\
                     Offending line: {os_line:?}"
                )
            })
            .trim();

        // ADV-P55-MED-004: `os:` alone on its own line, with the actual
        // list on FOLLOWING `- item` block-sequence lines, is legal YAML
        // and leaves `raw_value` empty here — which, exactly as above,
        // would trivially (and wrongly) certify the matrix as static
        // without this checker ever reading the real list. Read the
        // block-sequence form explicitly rather than let an empty same-
        // line value fall through unnoticed.
        let value: std::borrow::Cow<'_, str> = if raw_value.is_empty() {
            let mut collected: Vec<String> = Vec::new();
            for line in &lines[os_line_idx + 1..] {
                let trimmed = line.trim_start();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let indent = line.len() - trimmed.len();
                if indent <= 8 {
                    break;
                }
                let Some(item) = trimmed.strip_prefix("- ") else {
                    panic!(
                        "FAIL (S-626-1 Guard B, ADV-P55-MED-004): \
                         `{job_id}`'s `strategy.matrix.os:` has an empty \
                         same-line value, and the following indented line \
                         (\"{line}\") is not a plain `- item` \
                         block-sequence entry. This checker only supports \
                         the inline-array (`os: [a, b]`) and plain \
                         block-sequence (`- a` / `- b`) forms and refuses \
                         to guess at anything else (e.g. a flow mapping \
                         per item) rather than risk certifying a dynamic \
                         matrix as static."
                    );
                };
                collected.push(item.trim().to_string());
            }
            if collected.is_empty() {
                panic!(
                    "FAIL (S-626-1 Guard B, ADV-P55-MED-004): `{job_id}`'s \
                     `strategy.matrix.os:` has an empty same-line value and \
                     no following block-sequence items at all — this \
                     checker cannot certify a matrix with no discoverable \
                     `os` list as static."
                );
            }
            std::borrow::Cow::Owned(collected.join(", "))
        } else {
            std::borrow::Cow::Borrowed(raw_value)
        };

        assert!(
            !value.contains("${{") && !value.contains("fromJSON"),
            "FAIL (S-626-1 Guard B, DEC-246 §Q4): `{job_id}.strategy.matrix.os` \
             is no longer a static literal list (found: `{value}`).\n\
             \n\
             GitHub does not document what `needs.{job_id}.result` reports \
             when a DYNAMIC matrix (e.g. `fromJSON(...)`) expands to ZERO \
             legs — community reports on the general zero-leg-matrix \
             question are split between `skipped` (safe) and `success` (a \
             silent false green), and this repository has never verified \
             which applies here. A static literal `os:` list can never \
             expand to zero legs, so this question has been provably moot \
             until now.\n\
             \n\
             Before converting `{job_id}`'s matrix to a dynamic form, first \
             resolve ZERO-LEG-MATRIX-RESULT-UNDOCUMENTED empirically (a \
             throwaway PR with a matrix that can expand to zero legs, \
             observed against a real GitHub Actions run) — do not resolve \
             it by inference, and do not land the conversion in the same \
             change that would make this newly-reachable question live \
             without that verification.",
        );

        // S-626-1 pass-57, ADV-P56-INFO-001 (b): converts the second,
        // independent zero-leg vector — `strategy.matrix.exclude:` removing
        // combinations from an otherwise-static list — into the same kind
        // of decidable source property as the `${{ }}`/`fromJSON` check
        // above: does `{job_id}.strategy.matrix` declare an `exclude:` key
        // at all? Scoped to the `matrix:` mapping specifically (not a bare
        // `job_block.contains("exclude:")`) via `collect_mapping_key_set`
        // — the same quote/whitespace-aware, comment-and-blank-line-
        // tolerant primitive used for every other key-set pin in this
        // file — anchored on the `matrix:` key at 6-space indent (one
        // level above `os:`'s 8-space indent) so a coincidental
        // `exclude:` living under a step's `with:` block elsewhere in the
        // job would not be mistaken for this one.
        let matrix_line_idx = lines
            .iter()
            .position(|l| extract_key_name_at_indent(l, 6).as_deref() == Some("matrix"))
            .unwrap_or_else(|| {
                panic!(
                    "FAIL (S-626-1 Guard B, ADV-P56-INFO-001): `{job_id}` \
                     has no `strategy.matrix:` line at the expected \
                     6-space indent, even though it has an `os:` line at \
                     8-space indent one level deeper — has this job's \
                     matrix nesting changed? Update this test's anchor \
                     alongside the ci.yml change."
                )
            });
        let matrix_keys = collect_mapping_key_set(&lines, matrix_line_idx + 1, 8);
        assert!(
            !matrix_keys.iter().any(|k| k == "exclude"),
            "FAIL (S-626-1 Guard B, ADV-P56-INFO-001): \
             `{job_id}.strategy.matrix` now declares an `exclude:` key.\n\
             \n\
             `exclude:` can remove combinations from an otherwise-static \
             matrix, reopening the same undocumented \
             ZERO-LEG-MATRIX-RESULT-UNDOCUMENTED question the `${{{{ }}}}`/\
             `fromJSON` check above exists to guard against — whether \
             GitHub permits a fully-excluded matrix at all, and if so what \
             `needs.{job_id}.result` reports for it, is UNVERIFIED. Do not \
             resolve this by inference: first verify empirically (a \
             throwaway PR with a matrix that can fully exclude itself, \
             observed against a real GitHub Actions run) before landing \
             an `exclude:` on this matrix.",
        );
    }
}

// ---------------------------------------------------------------------------
// S-626-1 pass-54 — fixed-denominator self-check (ADV-P54-MED-003)
// ---------------------------------------------------------------------------

/// PINNED count of `#[test]` functions in THIS file.
///
/// POL-11's `test` job canary requires only a NON-ZERO passed count from
/// the `ci_gate_completeness` binary — it says nothing about HOW MANY
/// tests this binary is supposed to contain. Deleting some number of
/// `#[test]` functions from this file trips nothing under that guard: the
/// binary still runs, still reports a non-zero passed count, and the
/// canary stays green. Mirrors the fixed-denominator pattern already used
/// by `scripts/check-ci-gate.sh --self-test`'s own `EXPECTED_FIXTURES`,
/// `scripts/check-bc-citation-symbols.sh`'s, and
/// `scripts/check-cargo-mutants-policy-citations.sh`'s self-tests — and,
/// within this file, `test_allowed_skips_has_exactly_three_code_level_
/// references`'s narrower precedent of counting textual occurrences of a
/// known-good shape.
///
/// UPDATE THIS CONSTANT in the SAME change whenever a `#[test]` fn is
/// added to or removed from this file. This is a tripwire, not a
/// mechanism meant to stay in sync automatically — a mismatch is a signal
/// to look at what changed (a legitimate addition/removal vs. an
/// accidental deletion), not something to silence by "fixing" the number
/// without checking why it moved.
const EXPECTED_GUARD_TEST_COUNT: usize = 27;

/// Coverage note (S-626-1 pass-57, `DENOMINATOR-GUARD-USES-EXACT-LINE-MATCH`):
/// counts lines whose TRIMMED text STARTS WITH the literal `#[test]`, not
/// lines EQUAL to it. The prior exact-`==` match was the same shape the
/// `910b8ab0` class sweep fixed everywhere else in this file (key-detect
/// vs. value-reparse swallow) — it missed `#[test] fn foo() {}` written on
/// one line, silently lowering the denominator; that evasion was mitigated
/// only by `cargo fmt --check` forcing the attribute onto its own line in a
/// DIFFERENT CI job, not by this guard itself. `starts_with` also counts a
/// `#[test]` line followed by trailing same-line content (a comment, or —
/// as just described — the function signature itself).
///
/// This is still a literal textual match, not a Rust parser: it does NOT
/// catch semantically-equivalent-but-differently-spelled forms such as
/// `#[ test ]` (internal whitespace before the trimmed prefix breaks),
/// `#[core::prelude::v1::test]` (fully-qualified attribute path), or a
/// locally aliased/renamed `test` import — none of which occur in this file
/// today. Verified this change does not move `EXPECTED_GUARD_TEST_COUNT`:
/// the file's other seven textual occurrences of `#[test]` all live inside
/// `///` doc comments or a panic-message string literal, and none of those
/// lines' trimmed text starts with the literal `#[test]`.
///
/// S-626-1 pass-59 (ADV-P57-MED-002 ≡ ADV-P58-MED-002, dedupe — same
/// finding reported twice): `EXPECTED_GUARD_TEST_COUNT` above pins the
/// `#[test]` ATTRIBUTE count, not ENFORCEMENT. Two distinct ways to make
/// a test attribute stop running while the count above stays unmoved:
///   - `#[ignore]`: `cargo test` still reports the binary's overall
///     result as `ok` (e.g. "26 passed; 1 ignored"), so `ci.yml`'s
///     POL-11 gate (3) — which only requires a non-zero passed count
///     from THIS binary (`_canary_passed -eq 0` → FAIL) — is satisfied
///     by the 26 REMAINING tests.
///   - `#[cfg(target_os = "haiku")]` (or any predicate false on every
///     platform this repo actually builds for — `ubuntu-latest` /
///     `windows-latest` / `macos-latest`, per the `test` job's matrix):
///     the gated `#[test]` fn does not exist in the compiled binary AT
///     ALL, on ANY of those platforms, yet the textual attribute is
///     still present in `include_str!`'s source, so `actual` above is
///     unaffected.
///
/// Either construction, applied to
/// `test_ci_gate_pass_fail_semantics_are_structurally_placed` — the
/// single largest concentration of decision-path pins in this file
/// (M2-a..p) — silently removes that entire test from every CI run while
/// `EXPECTED_GUARD_TEST_COUNT` and POL-11's canary both stay green.
///
/// FIX: assert zero `#[ignore]` attributes anywhere in this file, and
/// that every `#[cfg(...)]` line immediately preceding a `#[test]` line
/// is the ONE legitimate form in this file today, `#[cfg(unix)]` (used
/// by the tests that shell out to bash and therefore cannot exist on
/// Windows — see `list_all_ci_yml_job_names`'s doc comment for the
/// "gating a test also orphans its helpers" lesson from PR #671 review
/// round 15, which this same allowlist must be kept consistent with).
/// `ci.yml`'s POL-11 gate (3) is NOT changed from `> 0` to an equality
/// against `EXPECTED_GUARD_TEST_COUNT`: this file's `test` job runs on a
/// TWO-OS matrix (`ubuntu-latest`, `windows-latest`), and the four
/// `#[cfg(unix)]`-gated `#[test]` fns in this file genuinely do not
/// compile on the Windows leg — a literal `27` there would fail every
/// Windows `test` run even on a fully green tree. That entanglement is
/// resolved on the Rust side only; see this test's assertions below.
#[test]
fn test_this_file_test_count_matches_expected_denominator() {
    let source = include_str!("ci_gate_completeness.rs");
    let lines: Vec<&str> = source.lines().collect();
    let actual = lines
        .iter()
        .filter(|l| l.trim().starts_with("#[test]"))
        .count();
    assert_eq!(
        actual, EXPECTED_GUARD_TEST_COUNT,
        "FAIL (S-626-1 pass-54, ADV-P54-MED-003): this file contains \
         {actual} `#[test]` functions, but EXPECTED_GUARD_TEST_COUNT pins \
         {EXPECTED_GUARD_TEST_COUNT}. POL-11's zero-test-floor canary in \
         `ci.yml :: test` only requires a NON-ZERO passed count from this \
         binary — it does not know how many tests this file is SUPPOSED \
         to contain, so silently deleting tests from this file trips \
         nothing there. If this mismatch is from a deliberate, reviewed \
         addition or removal of a `#[test]` fn, update \
         EXPECTED_GUARD_TEST_COUNT in the SAME change. If it is not \
         deliberate, some `#[test]` fn was lost — find out which one \
         before changing this constant."
    );

    // S-626-1 pass-60 (ADV-P60-HIGH-001): the assertion above pins the
    // TEXTUAL count of `#[test]` attributes — it says nothing about
    // whether every one of those attributes still actually RUNS. This
    // doc comment previously claimed (since pass-59) that the two
    // documented evasions below were resolved "on the Rust side only;
    // see this test's assertions below" — but until this pass, no such
    // assertion existed; the function body ended at the `assert_eq!`
    // above. The two assertions that follow are that missing
    // enforcement, added in the SAME change as this correction so the
    // doc comment's claim is finally backed by code, not just prose.
    //
    //   - `#[ignore]`: the attribute count above stays unmoved (the
    //     line still starts with `#[test]`), but `cargo test` reports
    //     the binary's overall result as `ok` regardless (e.g. "26
    //     passed; 1 ignored"), and `ci.yml`'s POL-11 canary only
    //     requires a NON-ZERO passed count from this binary — satisfied
    //     by the remaining tests. Guarded by the zero-`#[ignore]`
    //     assertion immediately below.
    //   - `#[cfg(...)]` with a predicate false on every platform this
    //     repo actually builds for: the gated `#[test]` fn does not
    //     exist in the compiled binary AT ALL on any of those
    //     platforms, yet its textual attribute is still present in
    //     `include_str!`'s source, so `actual` above is unaffected.
    //     Guarded by the allowlist assertion below, which accepts only
    //     the one legitimate form used in this file today,
    //     `#[cfg(unix)]` (the tests that shell out to bash and
    //     therefore cannot exist on Windows — see
    //     `list_all_ci_yml_job_names`'s doc comment for the "gating a
    //     test also orphans its helpers" lesson from PR #671 review
    //     round 15, which this allowlist must be kept consistent
    //     with).
    let ignore_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim().starts_with("#[ignore"))
        .map(|(i, _)| i + 1) // 1-based line numbers for the diagnostic.
        .collect();
    assert!(
        ignore_lines.is_empty(),
        "FAIL (S-626-1 pass-60, ADV-P60-HIGH-001): found `#[ignore]` \
         attribute(s) at line(s) {ignore_lines:?} in this file. The \
         `actual == EXPECTED_GUARD_TEST_COUNT` assertion above counts \
         `#[test]` ATTRIBUTES textually — an `#[ignore]`d test still \
         carries that attribute, so the count does not move, but the \
         test never runs. `cargo test` still reports an overall `ok` \
         result for this binary (e.g. \"26 passed; 1 ignored\"), and \
         `ci.yml`'s POL-11 zero-test-floor canary only requires a \
         NON-ZERO passed count — satisfied by the remaining tests. \
         Remove the `#[ignore]` attribute, or if a test genuinely must \
         be disabled, that decision needs its own explicit review, not \
         a silent addition that leaves this guard green."
    );

    // ADV-P60-HIGH-001, second gap: a `#[cfg(...)]` predicate false on
    // every platform this repo builds for removes the gated `#[test]`
    // fn from the compiled binary entirely — invisible to both the
    // count above and the `#[ignore]` scan above (no `#[ignore]`
    // attribute is present; the fn is simply absent from that
    // platform's build). This scan requires that ANY `#[cfg(...)]`
    // line directly preceding a `#[test]` line be byte-for-byte one of
    // the allowlisted forms below — reject-don't-parse, the same
    // discipline this file uses elsewhere (e.g.
    // `extract_and_normalize_if_expr`) rather than attempt to evaluate
    // arbitrary `cfg` predicate syntax.
    const ALLOWED_TEST_CFG_GATES: &[&str] = &["#[cfg(unix)]"];
    let bad_cfg_gates: Vec<(usize, String)> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim().starts_with("#[test]"))
        .filter_map(|(i, _)| {
            if i == 0 {
                return None; // No line above index 0 to inspect.
            }
            let prev = lines[i - 1].trim();
            if prev.starts_with("#[cfg(") && !ALLOWED_TEST_CFG_GATES.contains(&prev) {
                Some((i + 1, prev.to_string())) // 1-based `#[test]` line number.
            } else {
                None
            }
        })
        .collect();
    assert!(
        bad_cfg_gates.is_empty(),
        "FAIL (S-626-1 pass-60, ADV-P60-HIGH-001): found `#[test]` fn(s) \
         whose immediately preceding line is a `#[cfg(...)]` attribute \
         NOT in the allowlist {ALLOWED_TEST_CFG_GATES:?}: {bad_cfg_gates:?} \
         (line number, offending attribute). A `#[cfg(...)]` predicate \
         false on every platform this repo builds for \
         (`ubuntu-latest`/`windows-latest`/`macos-latest`, per the \
         `test` job's matrix) removes the gated `#[test]` fn from the \
         compiled binary ENTIRELY on every one of those platforms — the \
         textual `#[test]` count above does not move (the attribute is \
         still present in source), and there is no `#[ignore]` \
         attribute for the scan above to catch either. If this is a \
         deliberate, reviewed new platform-gated test, add its exact \
         `#[cfg(...)]` spelling to ALLOWED_TEST_CFG_GATES in the SAME \
         change — and confirm any helper that test alone uses is \
         gated the same way (PR #671 review round 15's \"gating a test \
         also orphans its helpers\" lesson)."
    );
}

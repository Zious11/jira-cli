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
use common::wf::{Job, ScalarStyle, Step, Value, WfDoc};
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
/// Returns `None` if no job-level `needs:` key is found in the block.
///
/// # S-CIGATE-3 pass C: rewritten on `WfDoc::parse_single_job` +
/// `job_level_nested_sequence_items`, event-stream backed
///
/// This function used to be the motivating case for S-626-1 pass-55
/// (ADV-P55-HIGH-001): a line-based `line.trim()` scan with no indentation
/// check let a `needs:` key nested arbitrarily deep (e.g. inside a step's
/// `with:` block) masquerade as the job's own job-level `needs:` key, and
/// the fix — anchoring on `extract_key_name_at_indent(line, 4)` — was
/// still bound to that ONE hard-coded indent column
/// (`POSITIONAL-ASSUMPTION-AXIS`, closed by this rewrite per AC-008: a job
/// whose own direct children are legally indented 3, 6, or 8 spaces
/// instead of 4 was invisible to that scan). Duplicate-`needs:` detection
/// is now `job.keys.iter().filter(|k| ...).count()` over
/// [`WfDoc::parse_single_job`]'s tree-derived, non-deduplicated key list —
/// TREE MEMBERSHIP, not a line scan, so it is immune by construction to
/// both the spelling axis (`needs:`/`"needs":`/`'needs':`/`needs :` all
/// resolve to the identical key text) and the indent axis (a job's direct
/// children are found by tree structure, not indent arithmetic) at once. A
/// SECOND job-level `needs:` key anywhere in the block is still a hard
/// `panic!`, mirroring `extract_and_normalize_if_expr`'s refusal to
/// silently pick a winner between two job-level `if:` keys (also invalid
/// YAML — a duplicate mapping key GitHub Actions/actionlint reject at
/// parse time — but this checker does not rely on that external validation
/// alone).
///
/// The actual VALUE (the set of job names) is resolved via
/// [`job_level_nested_sequence_items`] with the single-segment path
/// `&["needs"]` — a SEQUENCE accessor, since `needs: [a, b, c]` is a
/// sequence node, not a scalar. Unlike the pre-parser checker, block-list
/// (`- item`) and flow (`[a, b, c]`) forms need no separate handling here:
/// `read_sequence` (`tests/common/wf.rs`) walks either shape identically by
/// tree membership. A bare scalar `needs: single_job` (no brackets) still
/// resolves to `None` — deliberately preserved from the pre-parser
/// checker's behavior (see `job_level_nested_sequence_items`'s own doc
/// comment) rather than newly special-cased as a one-item set.
fn parse_needs_set(job_block: &str) -> Option<HashSet<String>> {
    let job = WfDoc::parse_single_job(job_block);
    let needs_count = job.keys.iter().filter(|k| k.as_str() == "needs").count();

    if needs_count > 1 {
        panic!(
            "FAIL (S-626-1 pass-55, ADV-P55-HIGH-001; S-CIGATE-3 pass C): \
             job block contains {needs_count} job-level `needs:` keys — \
             this checker refuses to silently pick one. This is ALSO \
             invalid YAML (a duplicate mapping key) that GitHub Actions \
             and actionlint both reject at parse time, but this checker \
             will not rely on that external validation alone.\n\
             Job block:\n{job_block}"
        );
    }
    if needs_count == 0 {
        return None;
    }

    let items = common::wf::job_level_nested_sequence_items(job_block, &["needs"])?;
    Some(items.into_iter().collect())
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
/// # S-CIGATE-3 pass C: rewritten on `WfDoc::parse`, event-stream backed
///
/// `ci` is parsed once via [`WfDoc::parse`]; `doc.jobs` already IS the
/// tree-derived, source-order, non-deduplicated list of every entry under
/// the document's top-level `jobs:` mapping — this function reduces to
/// mapping each [`Job`]'s `id` field. This closes S-626-1 pass-55's
/// ADV-P55-MED-002 finding (a flow-style job entry, e.g. `gate: {name: CI
/// Gate, runs-on: ubuntu-latest}`, does not end with `:` and was invisible
/// to the old line-based `strip_suffix(':')` scan) BY CONSTRUCTION: a
/// `MappingStart` event marks a job entry regardless of whether its value
/// is written in flow or block style — there is no line-ending shape left
/// to get wrong. The former "jobs: is this file's last top-level key, so
/// scanning to EOF is safe today" caveat no longer applies either: tree
/// traversal bounds itself at the `jobs:` mapping's own `MappingEnd` event,
/// not at end-of-file, so a hypothetical future top-level key AFTER
/// `jobs:` would not silently get swept in.
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
    let doc = WfDoc::parse(ci);
    if !doc.root_keys.iter().any(|k| k == "jobs") {
        panic!("FAIL: `.github/workflows/ci.yml` has no top-level `jobs:` key.");
    }
    doc.jobs.iter().map(|j| j.id.clone()).collect()
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

/// **DELETED (S-CIGATE-3 pass C):** `assert_job_block_uses_4_space_child_indent`
/// used to live here. It existed solely to guard the "a job's direct
/// children are indented exactly 4 spaces" assumption every hardcoded-indent
/// line-based check in this file baked in (S-626-1 pass-59,
/// ADV-P57-HIGH-001/MED-001) — `extract_key_name_at_indent(line, 4)` for
/// job-level keys, `matrix_needs_members`'s old `strategy:` scan (below),
/// and others. Under this pass's tree-based rewrite, `matrix_needs_members`
/// no longer makes ANY indent assumption at all (`Job::keys`, from
/// [`WfDoc::parse_single_job`], is found by tree membership, not indent
/// arithmetic) — so the function guarding that assumption became vacuous
/// for its one remaining call site and was deleted rather than kept as
/// dead weight.
///
/// **This is a genuine behavioral RELAXATION, being called out explicitly
/// per this story's mandate, not silently dropped:** the deleted function
/// used to HARD-FAIL (`assert!`) on a job body indented anything other than
/// 4 spaces. Nothing in this file asserts that anymore. That is the
/// CORRECT outcome — indent is now semantically irrelevant to every
/// rewritten guard, which is the entire point of closing
/// `POSITIONAL-ASSUMPTION-AXIS` structurally rather than patching it — but
/// it does mean `.github/workflows/ci.yml` (or a sibling workflow file)
/// could adopt a 2- or 6-space job body tomorrow and nothing in this test
/// suite would object, where before this rewrite it would have hard-failed
/// with a named diagnostic. Confirmed before deleting: the function had
/// exactly one call site in this file (`matrix_needs_members`, immediately
/// below) — no other caller depended on it.
///
/// Every `ci-gate.needs` member whose job block declares a job-level
/// `strategy:` key — i.e. every build-matrix job, derived from the LIVE
/// `needs:` set rather than a hand-maintained literal.
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
///
/// # S-CIGATE-3 pass C: `strategy:` presence rewritten on
/// `WfDoc::parse_single_job`
///
/// The old `job_block.lines().any(|l| extract_key_name_at_indent(l,
/// 4).as_deref() == Some("strategy"))` scan — and the
/// `assert_job_block_uses_4_space_child_indent` guard that used to run
/// immediately before it, to at least fail LOUDLY rather than silently
/// under-report when that indent assumption didn't hold — are both gone.
/// `job.keys.iter().any(|k| k == "strategy")` over
/// [`WfDoc::parse_single_job`]'s tree-derived key list finds `strategy:`
/// by TREE MEMBERSHIP: immune to every spelling variant AND every indent
/// by construction, so there is no assumption left for a guard to protect.
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
            let job = WfDoc::parse_single_job(job_block);
            job.keys.iter().any(|k| k == "strategy")
        })
        .collect();
    matrix_jobs.sort();
    matrix_jobs
}

/// Does `job_block`'s JOB itself declare the YAML key `key` at the job's
/// own direct-child level?
///
/// # S-CIGATE-3 pass C: rewritten on `WfDoc::parse_single_job`; renamed
/// from `line_declares_job_level_key`
///
/// The pre-rewrite version of this function took a single `line: &str` and
/// pattern-matched a hard-coded 4-space indent plus an enumerated list of
/// quote spellings (`key`, `"key"`, `'key'`) directly against that one
/// line's text — the same `POSITIONAL-ASSUMPTION-AXIS` shape this story's
/// AC-008 exists to close. Renamed (from `line_declares_job_level_key`) to
/// reflect that its signature is no longer line-based: it now takes the
/// whole `job_block` and asks `job.keys.iter().any(|k| k == key)` over
/// [`WfDoc::parse_single_job`]'s tree-derived key list, which is immune BY
/// CONSTRUCTION to both axes at once — every spelling variant resolves to
/// the identical key text, and there is no indent literal left to hard-code
/// (a job's direct children are found by tree membership, not by scanning
/// for a specific leading-whitespace column). PR #671 review round 10,
/// IMPORTANT 2's original motivation (a real job-level `outputs:` block
/// spelled `"outputs":`/`'outputs':`/`outputs :` left the old round-9 guard
/// silently blind) is preserved, not merely reproduced: those three
/// spellings, PLUS every job-body indent, are now covered.
///
/// `#[cfg(unix)]` (PR #671 review round 15, CI-caught): only caller is
/// `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`,
/// itself `#[cfg(unix)]`-gated — see `list_all_ci_yml_job_names`'s doc
/// comment above for the full "gating a test orphans its helpers"
/// explanation.
#[cfg(unix)]
fn job_declares_job_level_key(job_block: &str, key: &str) -> bool {
    let job = WfDoc::parse_single_job(job_block);
    job.keys.iter().any(|k| k == key)
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
/// # S-CIGATE-3 pass C: rewritten on `WfDoc::parse_single_job`
///
/// All three checks are now `Job::value_of` lookups over
/// [`WfDoc::parse_single_job`]'s tree-derived job model, rather than raw
/// `str::contains`/line-scan substring checks against the whole job block
/// text. This is a genuine tightening, not merely a rewrite: the OLD
/// `if:` check (`t.starts_with("if:") && t.contains("always()")`) matched
/// ANY line in the block starting with `if:` after trimming — including a
/// hypothetical STEP-level `if:` deep inside `steps:` — even though the
/// docstring (and EC-001's rationale) is specifically about the JOB-level
/// `if:`; `job.value_of("if")` inherently looks at the job's own direct
/// keys only. All three lookups are immune, by construction, to key
/// spelling (`name:`/`"name":`/`'name':`/`name :`, etc.) and to
/// `ci-gate`'s own job-body indent — neither axis has a literal to get
/// wrong under tree membership.
///
/// RED GATE: `ci-gate` does not exist in ci.yml.  This test FAILS on develop.
///
/// # S-CIGATE-3 finding fix (ADV-SC3-P1-LOW-002, quoting-fidelity
/// completeness, 2026-08-11)
///
/// All three `matches!` checks below now additionally require
/// `style == ScalarStyle::Plain`, closing a gap the fresh-context
/// adversarial review flagged: pre-migration, these were whole-block
/// substring checks (`gate_block.contains("runs-on: ubuntu-latest")`), so
/// `runs-on: "ubuntu-latest"` (double-quoted), `name: 'CI Gate'`
/// (single-quoted), or a block-scalar `runs-on: >-\n  ubuntu-latest` all
/// FAILED the old check — GitHub Actions resolves all these forms
/// identically, but the pre-migration checker was byte-literal and did not
/// know that. The tree-based rewrite's `text == "..."` comparison alone
/// would have silently ACCEPTED all three re-quoted/re-folded forms (the
/// real parser resolves them to the same `text`), which is a real, if
/// narrow, loosening versus the pre-migration behavior even though it is
/// immaterial to `ci-gate`'s actual runtime semantics (chosen here to keep
/// every migrated pin in this file consistently strict — see AC-004 in
/// `extract_and_normalize_if_expr`'s doc comment for the same rationale
/// applied everywhere else) — rather than record `name:`/`runs-on:` here
/// as a deliberate exemption alongside the documented NOT-PINNED set
/// (`uses:` VALUES / `with:` block CONTENTS / `name:` VALUES generally,
/// per the CLAUDE.md SCOPE SUMMARY), the style conjunct is added to all
/// three so the mandate is not silently partial for this specific test.
/// This job's own `if:` value is SEPARATELY, more precisely style-pinned
/// by `PINNED_GATE_IF_EXPR`/`extract_and_normalize_if_expr` (M2-m) below —
/// the style check added here is defense-in-depth on this test's own
/// independent lookup, not the operative enforcement point for `if:`.
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
    let job = WfDoc::parse_single_job(gate_block);

    // name: CI Gate — produces the human-readable branch-protection context
    // string "CI Gate".  If omitted, the context would be the key "ci-gate".
    let name_is_ci_gate = matches!(
        job.value_of("name"),
        Some(Value::Scalar { text, style, .. }) if text == "CI Gate" && *style == ScalarStyle::Plain
    );
    assert!(
        name_is_ci_gate,
        "FAIL (RED GATE): The `ci-gate` job block does not have a plain \
         (unquoted) `name: CI Gate` (found job-level `name:` value {:?}).\n\
         Required: set `name: CI Gate` so the branch-protection context string \
         is human-readable (EC-003).\n\
         Current ci-gate block:\n{gate_block}",
        job.value_of("name")
    );

    // runs-on: ubuntu-latest — the aggregator is a lightweight step that only
    // inspects upstream results; ubuntu-latest is the correct runner.
    let runs_on_ubuntu = matches!(
        job.value_of("runs-on"),
        Some(Value::Scalar { text, style, .. }) if text == "ubuntu-latest" && *style == ScalarStyle::Plain
    );
    assert!(
        runs_on_ubuntu,
        "FAIL (RED GATE): The `ci-gate` job block does not have a plain \
         (unquoted) `runs-on: ubuntu-latest` (found job-level `runs-on:` \
         value {:?}).\n\
         Current ci-gate block:\n{gate_block}",
        job.value_of("runs-on")
    );

    // if: ${{ always() }} — LOAD-BEARING.  Without this, a failed upstream
    // skips `ci-gate` entirely; GitHub evaluates skip as SUCCESS, so a broken
    // upstream would silently permit merge (EC-001).
    let has_always_if = matches!(
        job.value_of("if"),
        Some(Value::Scalar { text, style, .. }) if text.contains("always()") && *style == ScalarStyle::Plain
    );
    assert!(
        has_always_if,
        "FAIL (RED GATE): The `ci-gate` job block does not have a plain \
         (unquoted) job-level `if:` value containing `always()` (found \
         {:?}).\n\
         Required: `if: ${{{{ always() }}}}` at the job level (load-bearing — \
         without it a failed upstream SKIPS ci-gate, which GitHub branch \
         protection evaluates as SUCCESS).\n\
         Current ci-gate block:\n{gate_block}",
        job.value_of("if")
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
/// Job-level `if:` is the job's own DIRECT mapping key (a job-level `if:`
/// example: `    if: github.event_name == 'pull_request'`, at 4-space
/// indent in `ci.yml`'s current formatting). Step-level `if:` lives inside
/// one of the job's `steps:` sequence entries instead; those are not
/// hazardous and are deliberately not checked here. **Correction
/// (S-CIGATE-3 pass C):** the distinction below used to be made by INDENT
/// POSITION (4 spaces vs. 8+) on the theory that job-level and step-level
/// `if:` lines are reliably distinguishable by column alone in `ci.yml`'s
/// current formatting; the implementation below no longer does this at
/// all — see that section's own "S-CIGATE-3 pass C" doc comment for why
/// tree membership (`Job::keys` vs. `Job::steps[i].keys`) makes indent
/// position irrelevant, closing the `POSITIONAL-ASSUMPTION-AXIS` drift
/// item this indent literal used to carry.
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
/// spelling. Fixed (at the time, S-626-1 pass-55) by routing detection
/// through `extract_key_name_at_indent`, the same quote/whitespace-aware
/// matcher used everywhere else in this file at that point. **That
/// function itself no longer exists.** S-CIGATE-3 pass C superseded this
/// entire mechanism, not merely patched it further: the implementation
/// below now resolves a job-level `if:` by TREE MEMBERSHIP
/// (`WfDoc::parse_single_job` + `Job::keys`), which is immune to key
/// spelling AND indent position simultaneously, with no line-based matcher
/// of any kind left in the call path. See that section's own doc comment
/// (below) for the current mechanism; this paragraph is kept only as a
/// historical record of the ADV-P55-LOW-001 finding and fix, not as a
/// description of current behavior.
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
    assert!(
        !required_jobs.is_empty(),
        "FAIL: `always_run_needs_members` returned no jobs at all — the \
         always-run job derivation is broken (or ci.yml itself is \
         malformed), which would make this test vacuously pass every \
         job-level `if:` it never saw. Current ci.yml jobs section could \
         not be parsed."
    );

    for job_name in &required_jobs {
        let job_block = extract_job_block(&ci, job_name).unwrap_or_else(|| {
            panic!(
                "FAIL: job `{job_name}` (listed in ci-gate.needs) was not found \
                 in ci.yml.  Either the job was renamed or removed — update \
                 ci-gate.needs and this test together."
            )
        });

        // S-CIGATE-3 pass C: rewritten on `WfDoc::parse_single_job`. A
        // job-level `if:` key is found by TREE MEMBERSHIP
        // (`Job::keys`/`WfDoc::parse_single_job`) — deliberately NOT
        // filtering on the condition's content or shape (see F-03 docstring
        // above): any job-level `if:` key on these seven jobs is hazardous,
        // whether it's a single-line condition, a folded/block scalar, or
        // references something other than `github.event_name`. Tree
        // membership is immune, by construction, to every spelling variant
        // (`if:`/`"if":`/`'if':`/`if :`, previously enumerated by hand via
        // `extract_key_name_at_indent`) AND every job-body indent (the old
        // scan hard-coded 4 spaces — `POSITIONAL-ASSUMPTION-AXIS`, closed
        // here per AC-008).
        let job = WfDoc::parse_single_job(job_block);
        if job.keys.iter().any(|k| k == "if") {
            panic!(
                "FAIL (M1/F-03): Job `{job_name}` has a job-level `if:` key.\n\
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
                 accordingly.\n\
                 \n\
                 Current `{job_name}` block:\n{job_block}"
            );
        }
    }
}

/// PINNED, human-reviewed COMPLETE job-level key set for every
/// `ci-gate.needs` member that must run unconditionally (every member
/// except `mutants`, per [`always_run_needs_members`]) — S-CIGATE-3
/// fix-burst-5 (ADV-SC3-P3-HIGH-001).
///
/// `PINNED_GATE_JOB_KEYS`/M2-k already pins this shape for `ci-gate`
/// itself; this table extends the SAME idiom to every OTHER always-run
/// job, closing a gap that previously existed for every one of them
/// (including `msrv`, which — per ADV-SC3-P3-HIGH-001's own report — had
/// NO key-set pin of any kind before this table, not even a narrower one).
///
/// Concretely, this closes `jobs.<job_id>.defaults.run.shell`: a
/// documented GitHub Actions key applying to every `run:` step in that
/// job unless a step overrides it with its own `shell:`. Inserting
/// ```yaml
///     defaults:
///       run:
///         shell: cat {0}
/// ```
/// as a job-level sibling of `spec-guard`'s `timeout-minutes: 5` (verified
/// as a concrete RED-proof reproduction against a temporary, untracked
/// copy of `ci.yml` this session — the tracked file was never modified)
/// makes every one of `spec-guard`'s nine steps — none of which declares
/// its own step-level `shell:` — `cat` its `run:` body instead of
/// executing it, INCLUDING `bash scripts/check-ci-gate.sh --self-test`,
/// the entire 13-fixture self-test suite. `spec-guard` still reports
/// `success` (`cat` exits 0), and `ci-gate` goes green. Every check that
/// existed before this table passed against that reproduction: the
/// self-test's byte-pinned run line and step key set
/// (`PINNED_CI_GATE_SELF_TEST_RUN_LINE`/`_STEP_KEYS`) are both scoped to
/// the STEP, not the job, so a job-level `defaults:` is invisible to
/// them; `test_always_run_jobs_have_no_continue_on_error` bans only that
/// one literal; `test_ci_gate_needs_jobs_have_no_job_level_if` bans only
/// `if`; `test_ci_yml_has_no_workflow_level_shell_override` and
/// `test_ci_yml_workflow_root_key_set_is_pinned` are both scoped to the
/// WORKFLOW root (`WfDoc::root_keys`), not any individual job.
///
/// **Verification note (fails OPEN toward accuracy, not overclaiming):**
/// whether `defaults.run.shell: cat {0}` is actually ACCEPTED and applied
/// by a live GitHub Actions runner in exactly this way is UNVERIFIED by
/// this session — the same status rounds 11/14 recorded for their own
/// `shell: cat {0}` payloads (confirmed against PyYAML/Ruby Psych parsing
/// and GitHub's documented custom-shell-template syntax, never executed
/// against a real runner). This table closes the STRUCTURAL gap (the
/// absence of any pin on these jobs' key sets) regardless of that
/// open question.
///
/// **Scope — why ALL seven always-run jobs, not just the five with an
/// unguarded `run:` step:** of the seven, only `fmt`, `clippy`, `msrv`,
/// `spec-guard`, and `check-signing-workflow-injection` are concretely
/// exposed to the `defaults.run.shell` vector specifically (none of their
/// steps sets its own `shell:`). `test` is NOT exposed to that specific
/// vector — its POL-11 guard step already carries an explicit
/// step-level `shell: bash` (pinned separately by
/// `PINNED_TEST_GUARD_STEP_KEYS`), which GitHub Actions' documented
/// precedence has override any job-level `defaults.run.shell`. `deny` is
/// not exposed EITHER, for an even simpler reason: it has no `run:` step
/// at all (`Deny (licenses + vulnerabilities)` is two `uses:` steps only)
/// — there is nothing for a job-level shell override to redirect. Both
/// are included in this table anyway: a COMPLETE key-set pin is strictly
/// more general than a `defaults`-specific check (it also catches an
/// unrelated smuggled key on either job — an `env:` block, a second
/// `if:`, anything not yet imagined), and per this fix's own mandate
/// ("closes the whole class, not just `defaults`"), narrowing the table
/// to only the five concretely-exposed jobs would leave that broader
/// protection un-added for `test`/`deny` for no real savings — the pins
/// are simple 4-5-entry sorted lists, not appreciably more churn-prone
/// than `PINNED_GATE_JOB_KEYS` already is for `ci-gate`. This is a
/// DELIBERATE scope choice, not an accidental one: `test`/`deny`'s
/// inclusion in this table is defense-in-depth against a DIFFERENT,
/// unrelated smuggled key — not a claim that they are exposed to the
/// `defaults.run.shell` vector this fix's RED proof concretely
/// reproduced.
const PINNED_ALWAYS_RUN_JOB_KEY_SETS: &[(&str, &[&str])] = &[
    (
        "check-signing-workflow-injection",
        &["name", "runs-on", "steps", "timeout-minutes"],
    ),
    (
        "clippy",
        &["name", "runs-on", "steps", "strategy", "timeout-minutes"],
    ),
    ("deny", &["name", "runs-on", "steps", "timeout-minutes"]),
    ("fmt", &["name", "runs-on", "steps", "timeout-minutes"]),
    ("msrv", &["name", "runs-on", "steps", "timeout-minutes"]),
    (
        "spec-guard",
        &["name", "runs-on", "steps", "timeout-minutes"],
    ),
    (
        "test",
        &["name", "runs-on", "steps", "strategy", "timeout-minutes"],
    ),
];

/// S-CIGATE-3 fix-burst-5 (ADV-SC3-P3-HIGH-001). RED proof (this
/// session): spliced `defaults:\n      run:\n        shell: cat {0}` in as
/// a job-level sibling of `spec-guard`'s `timeout-minutes: 5` in a
/// temporary, untracked copy of `ci.yml` — before this test existed, the
/// full 48-test suite (including `test_spec_guard_contains_check_ci_gate_
/// self_test_step`, whose byte-pinned run-line/step-key checks are scoped
/// to the self-test STEP, not the job) stayed green against that
/// reproduction. GREEN proof: with this test added, the same
/// reproduction fails here specifically, naming `spec-guard` and its
/// added `defaults` key. Both verified this session; the tracked
/// `.github/workflows/ci.yml` was never modified — see this test's own
/// completion report for exact commands run.
#[test]
fn test_always_run_jobs_have_pinned_complete_job_key_sets() {
    let ci = read_ci_yml();
    let required_jobs = always_run_needs_members(&ci);
    assert!(
        !required_jobs.is_empty(),
        "FAIL: `always_run_needs_members` returned no jobs at all — the \
         always-run job derivation is broken (or ci.yml itself is \
         malformed), which would make this test vacuously pass every \
         job-level key-set pin it never checked. Current ci.yml jobs \
         section could not be parsed."
    );

    for job_name in &required_jobs {
        let job_block = extract_job_block(&ci, job_name).unwrap_or_else(|| {
            panic!(
                "FAIL: job `{job_name}` (listed in ci-gate.needs) was not found \
                 in ci.yml.  Either the job was renamed or removed — update \
                 ci-gate.needs and this test together."
            )
        });

        let Some((_, pinned_keys)) = PINNED_ALWAYS_RUN_JOB_KEY_SETS
            .iter()
            .find(|(name, _)| *name == job_name.as_str())
        else {
            panic!(
                "FAIL (ADV-SC3-P3-HIGH-001): job `{job_name}` is a member of \
                 `ci-gate.needs` that must run unconditionally (per \
                 `always_run_needs_members`), but has NO entry in \
                 `PINNED_ALWAYS_RUN_JOB_KEY_SETS`. A newly-added always-run \
                 job's job-level key set (including a job-level `defaults:` \
                 that could redirect every one of its `run:` steps' \
                 interpreter — see this constant's own doc comment) is \
                 unpinned until a human reviews and adds it here. Add a \
                 `(\"{job_name}\", &[...])` entry with its current, \
                 reviewed key set in the SAME change.\n\
                 Current `{job_name}` block:\n{job_block}"
            );
        };

        let mut expected_keys: Vec<String> = pinned_keys.iter().map(|s| s.to_string()).collect();
        expected_keys.sort();
        let actual_keys = extract_job_level_key_set(job_block);

        assert_eq!(
            actual_keys, expected_keys,
            "FAIL (ADV-SC3-P3-HIGH-001): job `{job_name}`'s complete \
             job-level key set ({actual_keys:?}) does not match the \
             pinned, human-reviewed set ({expected_keys:?}). Any added, \
             removed, or renamed job-level key changes this set — most \
             importantly a job-level `defaults:` key, which can redirect \
             EVERY `run:` step in this job through a custom shell template \
             (e.g. `shell: cat {{0}}`, which `cat`s the run line's script \
             body instead of executing it) unless that step declares its \
             own step-level `shell:` override. If this is a deliberate, \
             reviewed change, update PINNED_ALWAYS_RUN_JOB_KEY_SETS in the \
             SAME change.\n\
             Current `{job_name}` block:\n{job_block}"
        );
    }
}

// ---------------------------------------------------------------------------
// S-CIGATE-3 pass 6 (ADV-SC3-P6-MED-001) — class sweep, fourth and final
// layer: "the correctly-selected step is never asserted to RUN AT ALL"
// ---------------------------------------------------------------------------
//
// Three prior passes each closed the SELECTION half of this shape at one
// layer and left the RUNS-AT-ALL half open: pass 3 required exactly one
// step to match by NAME (`find_sole_step_by_name`); pass 4 required exactly
// one step to have a `run:` KEY; pass 5 required exactly one step to match
// the `run:` VALUE (`find_sole_step_by` — the class sweep of the SELECTION
// layer, closing three separate raw `.find()` call sites in the SAME
// change). None of the four asked whether the correctly, unambiguously
// selected step is itself DISABLED. Concrete bypass, reproduced this pass
// against a temporary, untracked copy of `ci.yml` (the tracked file was
// never modified):
//
// ```yaml
//       - run: cargo check --all-features --locked
//         if: false
//         env:
//           RUSTUP_TOOLCHAIN: "1.85.0"
// ```
//
// `find_sole_step_by` still matches exactly one step (`if:` does not change
// the `run:` value it predicates on); `step_mapping_child_value_for_step`
// still span-resolves that same, now-permanently-skipped step correctly;
// both assertions in `test_verify_msrv_job_pins_toolchain_and_rustup_
// toolchain_env` pass. `test_always_run_jobs_have_no_continue_on_error`
// bans only that one literal key, not `if:`. `test_ci_gate_needs_jobs_
// have_no_job_level_if` checks `Job::keys` (the JOB's own direct keys),
// not any `Step::keys` — a step-level `if:` is invisible to it by
// construction. `PINNED_ALWAYS_RUN_JOB_KEY_SETS` (immediately above) pins
// `{name, runs-on, steps, timeout-minutes}` at the JOB level and never
// inspects `steps`'s own CONTENTS. RED-proven: the full suite stayed
// green with the `if: false` line present; `msrv` would report `success`;
// MSRV would never be validated — the exact S-626-1 false-green this
// story exists to prevent, reopened one level below where every prior
// pass looked.
//
// FIX (this pass): the SAME idiom already proven against `ci-gate`
// (`PINNED_GATE_STEP_KEY_SETS`/`extract_gate_step_key_sets` — a COMPLETE,
// ordered, per-step key-set pin) is extended to every OTHER always-run job
// whose steps were not yet covered by an equivalent pin:
//   - The five NON-matrix always-run jobs (`msrv`, `fmt`, `deny`,
//     `check-signing-workflow-injection`, `spec-guard`) get a COMPLETE
//     per-step pin covering EVERY step, via `PINNED_ALWAYS_RUN_STEP_KEY_
//     SETS` below and `test_always_run_jobs_have_pinned_complete_step_key_
//     sets`. A step-level `if:`, `continue-on-error:`, or `shell:` added
//     to ANY step in ANY of these five jobs now changes that step's key
//     set and fails this test — not merely the one step a prior pass
//     happened to already have a narrower pin for.
//   - `test` and `clippy` are MATRIX jobs (`ubuntu-latest`/`windows-
//     latest`/`macos-latest`) and are DELIBERATELY EXCLUDED from the
//     complete-per-step pin above — same rationale as the pre-existing
//     ADV-P51 module-level scope note (above `PINNED_TEST_GUARD_STEP_
//     KEYS`): a matrix job may legitimately need an OS-conditional step
//     in the future (e.g. a Windows-only setup step), and a blanket
//     step-level pin across EVERY step in a matrix job would foreclose
//     that. `test`'s own guard-bearing step (the POL-11 zero-test-floor
//     step) already has a narrower, by-name pin
//     (`PINNED_TEST_GUARD_STEP_KEYS`/`test_test_job_guard_step_key_set_
//     and_env_are_pinned`) that fully closes this pass's finding for
//     `test` specifically. `clippy`'s equivalent guard-bearing step (the
//     one `run:` step carrying the actual `-D warnings` lint gate) had NO
//     such pin before this pass — closed below by
//     `PINNED_CLIPPY_GUARD_STEP_KEYS`/
//     `test_clippy_job_guard_step_key_set_is_pinned`, selecting the step
//     by its `run:` VALUE (via `find_sole_step_by`, the same ambiguity-
//     checked accessor `msrv`'s two step lookups use) rather than by
//     `name:`, since this step currently has no `name:` key of its own.
//   - `mutants` is DELIBERATELY EXCLUDED from this whole section — it is
//     not an always-run job (`if: github.event_name == 'pull_request'`,
//     tracked separately by `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS`), and its
//     own step-level `if:` COUNT is separately pinned elsewhere in this
//     file (see the module comment on `mutants`'s own step-count guard).
//     `ci-gate` itself already has the ORIGINAL, most complete form of
//     this pin (`PINNED_GATE_STEP_KEY_SETS`, M2-l) and is not repeated
//     here. `coverage`/`security` are not `ci-gate.needs` members
//     (advisory by design) and are out of scope for the same reason
//     `test_ci_gate_needs_jobs_have_no_job_level_if` already excludes
//     them.

/// PINNED, human-reviewed COMPLETE per-step key sets, in step order, for
/// every step of each of the five NON-matrix always-run `ci-gate.needs`
/// jobs. Sibling of `PINNED_GATE_STEP_KEY_SETS` (which pins `ci-gate`'s own
/// steps this same way) and `PINNED_ALWAYS_RUN_JOB_KEY_SETS` (which pins
/// these same jobs' KEYS one level up — JOB-level, not per-step). See the
/// module comment immediately above this constant for the concrete `if:
/// false` bypass this closes and why `test`/`clippy` (matrix jobs) and
/// `mutants`/`ci-gate` (out of scope for other, already-documented reasons)
/// are not listed here.
const PINNED_ALWAYS_RUN_STEP_KEY_SETS: &[(&str, &[&[&str]])] = &[
    (
        "check-signing-workflow-injection",
        &[
            &["name", "uses", "with"], // Harden the runner (Audit all outbound calls)
            &["uses"],                 // actions/checkout
            &["name", "run"],          // Run injection guard (YAML-aware)
            &["name", "run"],          // Run injection guard negative fixture self-test
        ],
    ),
    (
        "deny",
        &[
            &["name", "uses", "with"], // Harden the runner (Audit all outbound calls)
            &["uses"],                 // actions/checkout
            &["uses"],                 // EmbarkStudios/cargo-deny-action
        ],
    ),
    (
        "fmt",
        &[
            &["name", "uses", "with"], // Harden the runner (Audit all outbound calls)
            &["uses"],                 // actions/checkout
            &["run"],                  // cargo fmt --all -- --check
        ],
    ),
    (
        "msrv",
        &[
            &["name", "uses", "with"], // Harden the runner (Audit all outbound calls)
            &["uses"],                 // actions/checkout
            &["uses", "with"],         // dtolnay/rust-toolchain
            &["uses"],                 // Swatinem/rust-cache
            &["env", "run"],           // cargo check --all-features --locked
        ],
    ),
    (
        "spec-guard",
        &[
            &["name", "uses", "with"], // Harden the runner (Audit all outbound calls)
            &["uses"],                 // actions/checkout
            &["name", "run"],          // Fetch factory-artifacts branch
            &["name", "run"],          // check-spec-counts (DRIFT-001)
            &["name", "run"],          // check-bc-no-numeric-test-counts (PG-365-1)
            &["name", "run"],          // check-bc-cumulative-counts self-test (fixture suite)
            &["name", "run"],          // check-bc-cumulative-counts (DRIFT-002)
            &["name", "run"],          // check-cargo-mutants-policy-citations self-test (Guard 2)
            &["name", "run"],          // check-cargo-mutants-policy-citations (Guard 2, DEC-150)
            &["name", "run"],          // check-bc-citation-symbols self-test (BC-CITE-001)
            &["name", "run"],          // check-bc-citation-symbols (BC-CITE-001)
            &["name", "run"],          // check-ci-gate self-test (fixture suite, S-CIGATE-2)
        ],
    ),
];

/// S-CIGATE-3, ADV-SC3-P6-MED-001. RED proof (this session): appended
/// `if: false` directly below the `msrv` job's `cargo check` step's `run:`
/// line in a temporary, untracked copy of `ci.yml`. Confirmed the CURRENT
/// suite (pre-fix) passed 29/29 against that reproduction — including
/// `test_verify_msrv_job_pins_toolchain_and_rustup_toolchain_env`, which
/// still resolves and validates `RUSTUP_TOOLCHAIN` on the (now-disabled)
/// step correctly, and `test_always_run_jobs_have_pinned_complete_job_key_
/// sets`, which is JOB-level and does not see a step-level key at all.
/// GREEN proof: with this test added, the same reproduction fails here
/// specifically, naming `msrv` and reporting the mismatched step key set
/// (`["env", "if", "run"]` vs. the pinned `["env", "run"]`). Separately
/// confirmed a `shell: cat {{0}}` addition to a `spec-guard` step (the same
/// custom-shell-template-override vector rounds 11/14 proved against
/// `ci-gate`) is caught the same way. Both verified this session; the
/// tracked `.github/workflows/ci.yml` was never modified.
#[test]
fn test_always_run_jobs_have_pinned_complete_step_key_sets() {
    let ci = read_ci_yml();
    let matrix_jobs = matrix_needs_members(&ci);
    let non_matrix_always_run: Vec<String> = always_run_needs_members(&ci)
        .into_iter()
        .filter(|j| !matrix_jobs.contains(j))
        .collect();

    for job_name in &non_matrix_always_run {
        let job_block = extract_job_block(&ci, job_name).unwrap_or_else(|| {
            panic!(
                "FAIL: job `{job_name}` (listed in ci-gate.needs) was not found \
                 in ci.yml. Either the job was renamed or removed — update \
                 ci-gate.needs and this test together."
            )
        });

        let Some((_, pinned_step_sets)) = PINNED_ALWAYS_RUN_STEP_KEY_SETS
            .iter()
            .find(|(name, _)| *name == job_name.as_str())
        else {
            panic!(
                "FAIL (S-CIGATE-3, ADV-SC3-P6-MED-001): job `{job_name}` is a \
                 non-matrix member of `ci-gate.needs` that must run \
                 unconditionally, but has NO entry in \
                 `PINNED_ALWAYS_RUN_STEP_KEY_SETS`. A newly-added always-run, \
                 non-matrix job's per-step key sets (including a step-level \
                 `if:`, which can silently skip that step while the job still \
                 reports `success` — see this section's own module comment) \
                 are unpinned until a human reviews and adds them here. Add a \
                 `(\"{job_name}\", &[...])` entry with its current, reviewed \
                 per-step key sets, in step order, in the SAME change.\n\
                 Current `{job_name}` block:\n{job_block}"
            );
        };

        let expected: Vec<Vec<String>> = pinned_step_sets
            .iter()
            .map(|set| {
                let mut v: Vec<String> = set.iter().map(|s| s.to_string()).collect();
                v.sort();
                v
            })
            .collect();
        let actual = extract_gate_step_key_sets(job_block);

        assert_eq!(
            actual, expected,
            "FAIL (S-CIGATE-3, ADV-SC3-P6-MED-001): job `{job_name}`'s \
             per-step key sets ({actual:?}) do not match the pinned, \
             human-reviewed sets ({expected:?}). Any added, removed, or \
             renamed key on ANY step — most importantly a step-level `if:` \
             (silently skips that one step; the job still concludes \
             `success` and `ci-gate` goes green with that step's real work \
             never having run), `continue-on-error:`, or `shell:` (can \
             redirect the step's `run:` body through a custom shell \
             template, e.g. `shell: cat {{0}}`) — or any added/removed/\
             reordered step, changes this. If this is a deliberate, \
             reviewed change, update PINNED_ALWAYS_RUN_STEP_KEY_SETS in the \
             SAME change.\n\
             Current `{job_name}` block:\n{job_block}"
        );
    }
}

/// S-CIGATE-3, ADV-SC3-P6-MED-001. The `clippy` job's lint-gate step's
/// COMPLETE key set — mirrors `PINNED_TEST_GUARD_STEP_KEYS`'s idiom for the
/// `test` job's POL-11 guard step (see the module-level scope note above
/// this whole section for why `clippy`, a matrix job, gets a per-step pin
/// on only this ONE step rather than the complete-job pin the five
/// non-matrix jobs get). This step currently carries no `name:` key of its
/// own — `test_clippy_job_guard_step_key_set_is_pinned` below selects it by
/// its `run:` VALUE instead, via `find_sole_step_by` (the same
/// ambiguity-checked accessor `msrv`'s two step lookups already use). An
/// added `if:` on this step silently skips `cargo clippy` entirely while
/// `clippy` still reports `success`.
const PINNED_CLIPPY_GUARD_STEP_KEYS: &[&str] = &["run"];

/// S-CIGATE-3, ADV-SC3-P6-MED-001. RED proof (this session): added `if:
/// false` to the `clippy` job's lint-gate step in a temporary, untracked
/// copy of `ci.yml`. Confirmed the pre-fix suite passed 29/29 against that
/// reproduction (`find_sole_step_by` still selects the step correctly by
/// its `run:` value; nothing previously asserted it actually runs). GREEN
/// proof: with this test added, the same reproduction fails here, naming
/// `clippy` and reporting the mismatched key set (`["if", "run"]` vs. the
/// pinned `["run"]`). Both verified this session; the tracked
/// `.github/workflows/ci.yml` was never modified.
#[test]
fn test_clippy_job_guard_step_key_set_is_pinned() {
    let ci = read_ci_yml();
    let clippy_block = extract_job_block(&ci, "clippy").unwrap_or_else(|| {
        panic!("FAIL: `.github/workflows/ci.yml` does not contain a `clippy:` job.")
    });
    let job = WfDoc::parse_single_job(clippy_block);

    let guard_step = find_sole_step_by(
        &job.steps,
        "with a `run:` value of exactly \"cargo clippy --all --all-features \
         --tests -- -D warnings\"",
        |s| {
            matches!(
                s.value_of("run"),
                Some(Value::Scalar { text, .. })
                    if text == "cargo clippy --all --all-features --tests -- -D warnings"
            )
        },
    )
    .unwrap_or_else(|reason| {
        panic!(
            "FAIL (S-CIGATE-3, ADV-SC3-P6-MED-001): the `clippy` job \
             {reason}\n\
             Current clippy job block:\n{clippy_block}"
        );
    });

    let mut expected: Vec<String> = PINNED_CLIPPY_GUARD_STEP_KEYS
        .iter()
        .map(|s| s.to_string())
        .collect();
    expected.sort();
    let mut actual = guard_step.keys.clone();
    actual.sort();

    assert_eq!(
        actual, expected,
        "FAIL (S-CIGATE-3, ADV-SC3-P6-MED-001): the `clippy` job's lint-gate \
         step's key set ({actual:?}) does not match the pinned, \
         human-reviewed set ({PINNED_CLIPPY_GUARD_STEP_KEYS:?}). An added \
         `if:` makes the step SKIP silently (job concludes `success`, \
         `ci-gate` goes green with `cargo clippy` never having run); \
         `continue-on-error:` neutralizes a genuine lint failure; `shell: \
         cat {{0}}` replaces the run line's script body entirely (the same \
         custom-shell-template override this file's other step-key-set \
         pins already guard against elsewhere). If this is a deliberate, \
         reviewed change, update `PINNED_CLIPPY_GUARD_STEP_KEYS` in the \
         same commit.\n\
         Current clippy job block:\n{clippy_block}"
    );
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
/// Distinction technique (S-CIGATE-3 pass F correction): job-level vs.
/// step-level `if:` is resolved by TREE MEMBERSHIP, not indent position —
/// `Job::keys` (via `WfDoc::parse_single_job`, below) contains only the
/// job's own DIRECT mapping keys, and a step-level `if:` lives inside
/// `Job::steps[i].keys` instead, a structurally separate field the parser
/// never conflates with `Job::keys` regardless of how the YAML happens to
/// be indented in `ci.yml` today. There is no "starts with exactly 4
/// spaces" indent literal left anywhere in the assertion below.
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
    // "Job-level" means the `if:` key is a direct child of the job's own
    // mapping — resolved by TREE MEMBERSHIP via `WfDoc::parse_single_job`
    // (event-stream backed), not a re-derived indent-column literal.
    //
    // # S-CIGATE-3 pass F (FINAL): rewritten on `WfDoc::parse_single_job`
    //
    // The pre-parser version anchored on `extract_key_name_at_indent(line,
    // 4)` — a hard-coded job-child indent (`POSITIONAL-ASSUMPTION-AXIS`):
    // a job whose own direct children are legally indented 3, 6, or 8
    // spaces instead of 4 was invisible to that scan. `Job::keys` closes
    // this by construction: a job's direct children are found by tree
    // membership under its own mapping, not indent arithmetic, so no
    // indent literal is left to hard-code at all. Also carries forward
    // every guarantee the pre-parser fix (S-626-1 pass-59, ADV-P58-LOW-003)
    // already established for the spelling axis — `if:`/`"if":`/`'if':`/
    // `if :` all resolve to the identical key text under a real parser —
    // and adds immunity to a node property directly on the key (`&x if:` /
    // `!!str if:`, the round-16 residual) that `extract_key_name_at_indent`
    // never had: `read_mapping` resolves a key's TEXT from its
    // `Event::Scalar`, independent of any anchor/tag riding along on it.
    // The old `in_steps` bookkeeping problem (a raw `starts_with` has no
    // notion of indent depth on its own, so it needed separate tracking of
    // "have we entered `steps:` yet" to avoid confusing a step-level `if:`
    // for a job-level one) is moot here too: a step-level `if:` lives in
    // `Job::steps[i].keys`, never in `Job::keys`, so the two can never be
    // conflated regardless of scan order.
    // -----------------------------------------------------------------------
    let job = WfDoc::parse_single_job(gate_block);
    let has_job_level_if = job.keys.iter().any(|k| k == "if");

    assert!(
        has_job_level_if,
        "FAIL (M2-a): The `ci-gate` job has no job-level `if:` key \
         (resolved by tree membership under the job's own mapping — immune \
         to both key-spelling and indent variance, not merely a bare-\
         spelling presence check).\n\
         Required: `    if: ${{{{ always() }}}}` so that ci-gate runs even when \
         upstream jobs fail.\n\
         Current ci-gate block:\n{gate_block}"
    );

    let job_if_text = match job.value_of("if") {
        Some(Value::Scalar { text, .. }) => text.clone(),
        _ => String::new(),
    };

    assert!(
        job_if_text.contains("always()"),
        "FAIL (M2-b): The job-level `if:` in `ci-gate` does not contain \
         `always()`.\n\
         Found:    if: {job_if_text}\n\
         Required: the job-level `if:` must be `always()` so the aggregator \
         runs regardless of upstream results (without this, a failed upstream \
         skips ci-gate and GitHub branch protection evaluates the skip as \
         SUCCESS).\n\
         Current ci-gate block:\n{gate_block}"
    );

    assert!(
        !job_if_text.contains("contains(needs"),
        "FAIL (M2-c): The job-level `if:` in `ci-gate` contains \
         `contains(needs` — this is the retired inline condition \
         S-CIGATE-2 replaced, not merely a misplaced one.\n\
         Found:    if: {job_if_text}\n\
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
    // # S-CIGATE-3 pass F (FINAL): rewritten on `WfDoc::parse_single_job`/
    // `Step::keys`
    //
    // Same closure as M2-a above, one level deeper: `Job::steps` is built
    // by walking `steps:`'s sequence directly (tree membership), so this
    // check is immune to both the spelling and indent axes, and to a node
    // property on a step's own `if:` key, for the same reasons M2-a is.
    // -----------------------------------------------------------------------
    let has_step_level_if = job.steps.iter().any(|s| s.keys.iter().any(|k| k == "if"));

    assert!(
        !has_step_level_if,
        "FAIL (M2-d, S-CIGATE-2): The `ci-gate` job contains a step-level \
         `if:` key (resolved by tree membership under `Job::steps` — \
         immune to both key-spelling and indent variance). Under Option C, \
         `scripts/check-ci-gate.sh` is invoked unconditionally and its own \
         exit code IS the pass/fail signal — no step-level `if:` should \
         gate that invocation (a reintroduced `if:` here would mean some \
         upstream results never even reach the script). \
         NOTE: M2-l below (the per-step COMPLETE key-set pin,\
         `PINNED_GATE_STEP_KEY_SETS`) is the OPERATIVE default-deny for a \
         step-level `if:` in ANY form; this assertion is a faster, more \
         specific diagnostic, not the sole backstop.\n\
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
    // # S-CIGATE-3 fix-burst-5 (ADV-SC3-P3-LOW-001): rewritten on
    // `Job::steps`/`Step::keys`
    //
    // Same closure as M2-d immediately above, applied to `run:` instead of
    // `if:`: whether ANY step carries a `run:` key is now resolved by tree
    // membership under `Job::steps` (`WfDoc::parse_single_job`), not a
    // `str::lines()` scan for a line whose trimmed text starts with
    // `"run:"`. This closes the round-12 benign-false-red the old
    // comment below documented (a step whose `run:` key is reordered to be
    // the sequence-marker-adjacent FIRST key, e.g. `      - run: ...`,
    // defeated the line-based `trim_start().starts_with("run:")` pattern
    // because it never strips the leading `- ` marker) — key ORDER within
    // a step's own mapping is irrelevant to `Step::keys` membership, so
    // there is no reordering left that could produce that false-red. This
    // also corrects the module-level claim (this file's header comment)
    // that "every structural assertion in this file queries the parsed
    // tree — not `str::lines()`/indent arithmetic" — this was the second
    // of two live counterexamples to that claim (the other being AC-001's
    // job-level `if: always()` check below), both closed in this pass.
    // -----------------------------------------------------------------------
    let has_run_step = job.steps.iter().any(|s| s.keys.iter().any(|k| k == "run"));

    assert!(
        has_run_step,
        "FAIL (M2-g): The `ci-gate` job block contains no step carrying a \
         `run:` key (resolved by tree membership under `Job::steps` — \
         immune to both key-spelling and key-ORDER variance; a step whose \
         `run:` key is not its first key is found identically to one \
         where it is).\n\
         The gate must have a `run:` step that actually invokes \
         `scripts/check-ci-gate.sh` and fails the job via that script's \
         exit code.\n\
         Without a `run:` step the job trivially succeeds for every upstream \
         result. NOTE: M2-l below (step key SETS, order-independent) is a \
         second, independent confirmation that the step itself is intact.\n\
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
         should already have failed on a missing `NEEDS_JSON:` line) or \
         no step in this job block has BOTH a `run:` key and an `env:` key \
         (see `step_mapping_child_keys`'s own doc comment for its exact \
         `None`-vs-empty-`Vec` contract). Unlike the pre-S-CIGATE-3 \
         version of this function (PR #671 review round 14 SUGGESTION), \
         this is NOT an order-sensitivity bug: `extract_gate_env_key_set` \
         is rewritten on `step_mapping_child_keys`, which finds `env:` by \
         TREE MEMBERSHIP within the run-bearing step's own mapping — a \
         legal `env:`-after-`run:` reorder on that same step resolves \
         identically to `env:`-before-`run:`, so reordering is ruled out \
         as a cause of an empty result here. An empty result here must \
         never be silently treated as \"no env vars to worry about\".\n\
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
    // Assertion 12 (M2-q, S-CIGATE-3 pass B, AC-007/round-16): NO key
    // anywhere in `ci-gate`'s parse tree — job-level, step-level, or
    // `env:`-level — carries a YAML node property (`&anchor`/`!tag`).
    //
    // M2-k/M2-l/M2-o above are all tree-based key-SET comparisons — they
    // already catch a NODE-PROPERTIED KEY THAT IS NEW (e.g. `&x shell: cat
    // {0}` inserted into the gate step: `saphyr-parser` resolves this to a
    // real `"shell"` key regardless of the anchor, so the step's key set
    // becomes `{"env","name","run","shell"}`, which fails to match
    // `PINNED_GATE_STEP_KEY_SETS`'s `{"env","name","run"}` by its OWN TEXT
    // alone — see AC-007's reproduction below). What a plain `Vec<String>`
    // key-set comparison CANNOT catch is a node property attached to a key
    // that is ALREADY a legitimate, expected member of a pinned set — e.g.
    // `&x run: some-other-command`: the key set stays exactly
    // `{"env","name","run"}`, textually identical to the pin, while the
    // KEY scalar `run` has silently gained an anchor (node properties bind
    // to the node immediately following them — here, the key, not the
    // value) a later `*alias` elsewhere in the same document (GitHub
    // shipped anchor/alias support to production Actions 2025-09-18 — a
    // live mechanism, not hypothetical) could reference.
    // `find_key_node_properties` closes that residual by scanning for the
    // node property itself, independent of whether SET membership also
    // happens to be correct — this is the "Additionally reject any anchor
    // or tag on a pinned key" requirement, applied once here for the whole
    // `ci-gate` tree rather than duplicated into every individual pin
    // function above.
    //
    // Scope (S-CIGATE-3 fix-burst-4, ADV-SC3-P2-LOW-004): this covers
    // node properties on KEYS only, matching `find_key_node_properties`'s
    // own name and doc comment. A node property on a pinned scalar's VALUE
    // instead (`run: &x cargo check --all-features --locked`) is a
    // DIFFERENT construct this assertion does not scan for — see that
    // function's doc comment for the full analysis of why this residual
    // exists and why no live exploit was constructible from it alone.
    // -----------------------------------------------------------------------
    let node_properties = common::wf::find_key_node_properties(gate_block);
    let node_property_count = node_properties.len();
    assert!(
        node_properties.is_empty(),
        "FAIL (M2-q, S-CIGATE-3 AC-007/round-16): the `ci-gate` job block \
         contains {node_property_count} key(s) carrying a YAML node \
         property (anchor and/or tag) directly on the key itself: \
         {node_properties:?}. This is \
         the exact round-16 residual (`&x shell: cat {{0}}` / `!!str \
         shell: cat {{0}}`) that defeated every set-equality pin in the \
         pre-S-CIGATE-3 line-based checker — a real YAML parser resolves \
         the key correctly regardless of the node property, so this \
         checker rejects the node property outright rather than silently \
         trusting a value it did not choose to resolve. No key in \
         `ci-gate` has a legitimate reason to carry an anchor or tag; \
         remove it.\n\
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

/// Extract the sorted, complete key set of the `test` job's POL-11 guard
/// step (`name: Run tests (zero-test floor, POL-11)`), for comparison
/// against `PINNED_TEST_GUARD_STEP_KEYS`.
///
/// Scoped narrowly (unlike `extract_gate_step_key_sets`, which collects
/// EVERY step in `ci-gate`) because the `test` job's other three steps
/// (harden-runner, checkout, rust-cache) are not part of ADV-P51's
/// reported attack surface and are deliberately out of scope for this pin
/// — see the module-level ADV-P51 scope note above.
///
/// # S-CIGATE-3 pass E: rewritten on `WfDoc::parse_single_job`/`Step::name`
///
/// The guard step is located by TREE MEMBERSHIP via `Step::name`
/// (`wf.rs`'s `build_step` resolves it from the step's own `name:` key,
/// found anywhere in that step's mapping) rather than a line-position
/// `- name: ...` scan, and its key set is `Step::keys` directly — no
/// indent literal (the old code's `extract_key_name_at_indent(l,
/// 6).or_else(|| extract_key_name_at_indent(l, 8))`) survives to get
/// wrong. This closes `POSITIONAL-ASSUMPTION-AXIS` for this guard the
/// same way pass B/C closed it for the gate-block/job-graph clusters, and
/// every `name:` spelling (`name:`/`"name":`/`'name':`/`name :`) resolves
/// identically because a real YAML parser treats them as the same key.
///
/// STRICTER than the old line-based lookup, not merely equivalent: the
/// old matcher required the literal line `- name: <value>` (`name:` as
/// the step's SEQUENCE-MARKER key, first in the mapping) — a decoy step
/// with the same `name:` VALUE but `name:` reordered to a LATER key in
/// its own mapping (e.g. `- run: ...\n  name: <value>`) was invisible to
/// it, since that decoy's `name:` line never matches the `- name: ...`
/// needle, so the old code would have silently returned only the REAL
/// step's keys while a same-named decoy sat unexamined a few lines away.
/// `Step::name` finds a step's `name:` key anywhere in its own mapping,
/// regardless of position — closing that specific gap.
///
/// # Correction (S-CIGATE-3 fix-burst-5, ADV-SC3-P3-MED-001)
///
/// The paragraph above, and this function's ORIGINAL implementation,
/// still used a raw `job.steps.iter().find(...)` — first-match, not
/// ambiguity-checked — despite the doc comment above claiming a decoy
/// step is made "visible to a caller that checks for ambiguity": this
/// call site was not such a caller. Concrete bypass: insert a DECOY step
/// carrying the identical `name:` value (and its own `run:`/`env:` keys,
/// matching `PINNED_TEST_GUARD_ENV_KEYS`) immediately BEFORE the real
/// step, and add `if: ${{ false }}` to the real step — `.find()` returns
/// the decoy, whose key set matches the pin, while the real (disabled)
/// step goes unexamined and `cargo test` never runs. This function now
/// routes through [`find_sole_step_by_name`] — the SAME ambiguity-
/// checked lookup [`extract_step_key_set_by_name`] already used —
/// returning `Err` on zero or more than one match instead of silently
/// picking the first.
fn extract_test_guard_step_keys(job_block: &str) -> Result<Vec<String>, String> {
    let job = WfDoc::parse_single_job(job_block);
    let step = find_sole_step_by_name(&job.steps, "Run tests (zero-test floor, POL-11)")?;
    let mut keys = step.keys.clone();
    keys.sort();
    Ok(keys)
}

/// Extract the sorted, complete key set of the `env:` block belonging to
/// the `test` job's POL-11 guard step, for comparison against
/// `PINNED_TEST_GUARD_ENV_KEYS`.
///
/// # S-CIGATE-3 pass E: rewritten on `step_mapping_child_keys`
///
/// Reused the SAME idiom pass B built for `extract_gate_env_key_set`
/// (`step_mapping_child_keys(job_block, "run", "env")`) rather than a
/// parallel implementation: anchoring on "the step that carries a `run:`
/// key" rather than a line-position scan closed the
/// `POSITIONAL-ASSUMPTION-AXIS` gap the old `extract_key_name_at_indent(l,
/// 10)`-based scanner had, and is order-independent BY CONSTRUCTION
/// (`env:` may legally appear before OR after `run:` on the step).
///
/// # Correction (S-CIGATE-3 fix-burst-5, ADV-SC3-P3-MED-001)
///
/// `step_mapping_child_keys(job_block, "run", "env")` anchors on the
/// FIRST step in the job's `steps:` sequence that carries a `run:` key —
/// within TODAY's `test` job that happens to be the same step
/// `extract_test_guard_step_keys` locates by name (the other three steps
/// are `uses:`-only), but nothing tied the two lookups together. A DECOY
/// step inserted BEFORE the real POL-11 step, sharing its `name:` value
/// AND carrying its own `run:` and `env: {CARGO_TERM_COLOR: never}` keys
/// (satisfying `PINNED_TEST_GUARD_ENV_KEYS` too), silently wins this
/// anchor-based lookup — the SAME class of bypass
/// `extract_test_guard_step_keys` had, one function over. This now routes
/// through [`common::wf::step_mapping_child_keys_by_step_name`], which
/// resolves the step by its own `name:` value with the same ambiguity
/// contract as [`find_sole_step_by_name`] (`Err` on zero or more than one
/// match), so it is anchored on the exact SAME step
/// `extract_test_guard_step_keys` resolves — not merely one that happens
/// to coincide with it today.
fn extract_test_guard_env_key_set(job_block: &str) -> Result<Vec<String>, String> {
    common::wf::step_mapping_child_keys_by_step_name(
        job_block,
        "Run tests (zero-test floor, POL-11)",
        "env",
    )
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
    // S-CIGATE-3 fix-burst-5 (ADV-SC3-P3-MED-001): `extract_test_guard_
    // step_keys` now returns `Result` — `Err` means either zero or MORE
    // THAN ONE step named `Run tests (zero-test floor, POL-11)` (the
    // latter is the decoy-step bypass this fix closes; a first-match
    // `.find()` used to silently prefer whichever of the two came first).
    let actual_step_keys = extract_test_guard_step_keys(test_block).unwrap_or_else(|reason| {
        panic!(
            "FAIL (ADV-P51-HIGH-001/HIGH-002, S-CIGATE-3 fix-burst-5): \
                 the `test` job {reason}\n\
                 Either the step's `name:` value changed, its structure \
                 was otherwise rewritten, or a same-named DECOY step is \
                 present — update `extract_test_guard_step_keys` if this \
                 is a deliberate rename, or remove the decoy if it is \
                 not.\n\
                 Current test job block:\n{test_block}"
        )
    });
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
    // S-CIGATE-3 fix-burst-5 (ADV-SC3-P3-MED-001): `extract_test_guard_
    // env_key_set` now returns `Result` for the same reason as above —
    // it is anchored on the SAME name-resolved step, not "the first step
    // that happens to carry a `run:` key".
    let actual_env_keys = extract_test_guard_env_key_set(test_block).unwrap_or_else(|reason| {
        panic!(
            "FAIL (ADV-P51-MED-002, S-CIGATE-3 fix-burst-5): the \
                 `test` job {reason}\n\
                 Either the step's `name:` value changed, its structure \
                 was otherwise rewritten, or a same-named DECOY step is \
                 present — update `extract_test_guard_env_key_set` if \
                 this is a deliberate rename, or remove the decoy if it \
                 is not.\n\
                 Current test job block:\n{test_block}"
        )
    });
    assert!(
        !actual_env_keys.is_empty(),
        "FAIL (ADV-P51-MED-002): could not locate the `env:` block on the \
         `test` job's POL-11 guard step at all (expected at least \
         `CARGO_TERM_COLOR`). The step itself was found unambiguously by \
         name, but it has no `env:` key (or `env:`'s value is not a \
         mapping) — update `extract_test_guard_env_key_set` if this is \
         deliberate.\n\
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
    assert!(
        !required_jobs.is_empty(),
        "FAIL: `always_run_needs_members` returned no jobs at all — the \
         always-run job derivation is broken (or ci.yml itself is \
         malformed), which would make this test vacuously pass every \
         `continue-on-error` check it never ran. Current ci.yml jobs \
         section could not be parsed."
    );

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
///
/// # S-CIGATE-3 pass E: intentionally UNCHANGED, not a YAML function
///
/// This function parses BASH inside an already-extracted `run:` script
/// body, not YAML — there is no mapping key, no scalar style, no node
/// property to resolve here, so `saphyr-parser`'s event stream has
/// nothing to offer it. What DOES matter is where `job_block`'s text
/// comes from: every caller passes `test_block`, itself
/// `extract_job_block`'s return value — a byte-for-byte SPAN-SLICE of
/// the ORIGINAL source text (see `tests/common/wf.rs`'s `Job::span` doc
/// comment), not a resolved `Event::Scalar` value. The `run: |` block
/// scalar's WORKFLOW indentation (the 10-space `fi` this function
/// searches for) therefore survives intact in `job_block` — it is never
/// dedented, because dedenting only happens when the PARSED scalar VALUE
/// is read (`Value::Scalar::text`), which this function never touches.
/// Had this function instead been fed a parsed scalar value, the
/// `"\n          fi\n"` search below would silently find nothing (the
/// workflow's 10-space indent stripped to whatever the block scalar's
/// OWN relative indent was) — verified by inspection of `wf.rs`'s
/// `resolve_value`, which calls `saphyr-parser`'s already-dedented
/// `Event::Scalar` text directly with no re-indent step. This function
/// is therefore correct as-is, unmodified, PROVIDED its `job_block`
/// input keeps coming from a span-sliced source (which
/// `extract_job_block`/`WfDoc` guarantee) rather than from a resolved
/// scalar value — a constraint worth stating explicitly since it is the
/// one hazard that would silently break this function if violated by a
/// future edit.
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
///
/// # S-CIGATE-3 pass C, fix-burst-6, fix-burst-7: two-step TREE-MEMBERSHIP
/// resolution, both steps ambiguity-checked
///
/// Every check below used to be either a whole-job-block `str::contains`
/// substring test (loose — matches anywhere, including the WRONG step) or,
/// for the one placement-sensitive check (F-02, round 19), a hand-rolled
/// byte-offset anchor + step-boundary slice keyed on the LITERAL run-line
/// text (fragile — a comment reproducing that text earlier in the file, or
/// a re-quoted run line, could mis-anchor it; see the F-02/ADV-P48-MED-001
/// history above, now superseded by this rewrite rather than deleted, so
/// the reasoning that motivated it remains legible).
///
/// The current body resolves the "dtolnay/rust-toolchain" step and the
/// "cargo check" step in TWO stages, both stages ambiguity-checked end to
/// end (fix-burst-7, ADV-SC3-P5-MED-001 — the class-sweep fix; see
/// `find_sole_step_by`'s own doc comment for the full history, including
/// two earlier point-fixes of the same underlying "exactly one" shape that
/// each left a sibling instance reachable):
///   1. **Selection** — `find_sole_step_by` picks the ONE step matching a
///      predicate (`Err`, naming the count, on zero or more than one
///      match): the dtolnay step by its `uses:` value starting with
///      `"dtolnay/rust-toolchain@"` (`@`-anchored at the version-pin
///      boundary, not a bare-prefix `starts_with` that would also match a
///      same-org decoy action name), and the cargo-check step by its
///      `run:` value being exactly `"cargo check --all-features
///      --locked"`. This replaces a raw, unchecked
///      `job.steps.iter().find(...)` for BOTH steps — the msrv job has
///      FOUR steps that all carry a `uses:` key (harden-runner, checkout,
///      dtolnay/rust-toolchain, Swatinem/rust-cache), so an anchor as weak
///      as "has a `uses:` key at all" was never viable in the first place,
///      and a decoy step sharing either target step's own selector text
///      is now flagged as ambiguous rather than silently resolved to
///      whichever comes first in `steps:`.
///   2. **Child-value resolution** — once a step is uniquely resolved,
///      `step_mapping_child_value_for_step` reads `with.toolchain` (on the
///      dtolnay step) or `env.RUSTUP_TOOLCHAIN` (on the cargo-check step)
///      by that EXACT step's own byte [`Step::span`] — a unique
///      source-text position, immune to a decoy step sharing some OTHER
///      key (the flaw `step_mapping_child_value`'s `step_anchor_key`
///      design had, and the reason this dedicated function exists — see
///      its own doc comment).
///
/// AC-004 quoting fidelity: `toolchain: "1.85.0"` and
/// `RUSTUP_TOOLCHAIN: "1.85.0"` are asserted as `ScalarStyle::DoubleQuoted`
/// (matching `ci.yml`'s current spelling — an unquoted `toolchain: 1.85.0`
/// would parse to a YAML FLOAT, not the string the `dtolnay/rust-toolchain`
/// action expects, so this is a real behavioral distinction, not
/// cosmetic); the `run:` line is asserted as `ScalarStyle::Plain` (its
/// current, unquoted spelling). All three axes this story's AC-008 cares
/// about — spelling, indent, and (here specifically) quoting style — are
/// closed for every check in this rewrite.
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
    let job = WfDoc::parse_single_job(msrv_block);

    // The `dtolnay/rust-toolchain` step, found by its OWN `uses:` value
    // prefix via tree membership — S-CIGATE-3 fix-burst-6 (ADV-SC3-P4-MED-001
    // "related instance"). `first_step_mapping_child_value(msrv_block,
    // "with", "toolchain")` (the pre-fix-burst-6 form) returns the FIRST
    // step whose `with:` mapping happens to contain a `toolchain` child,
    // without verifying that step is genuinely the `dtolnay/rust-toolchain`
    // action — a decoy step (any `uses:`) placed earlier in `steps:` with
    // its own `with: {toolchain: "1.85.0"}` would satisfy the old lookup
    // even if the REAL `dtolnay/rust-toolchain` step's `with.toolchain` were
    // removed entirely (which would silently fail the toolchain install and
    // let CI fall back to whatever `rustup` happens to select). Anchoring on
    // the step's own `uses:` value prefix, then resolving `with.toolchain`
    // via `step_mapping_child_value_for_step` (byte-span identity, not a
    // re-scan), closes this the same way as the `RUSTUP_TOOLCHAIN` fix
    // below.
    //
    // # S-CIGATE-3 fix-burst-7 (ADV-SC3-P5-MED-001): OUTER selection is now
    // ambiguity-checked too, and the prefix is `@`-anchored
    //
    // This selection step — which step even IS "the dtolnay/rust-toolchain
    // step" in the first place — was itself still a raw, unchecked
    // `job.steps.iter().find(...)` until this pass: fix-burst-6 relocated
    // the "exactly one" precondition from "one step has `run:`" to "one
    // step has `uses:` starting with `dtolnay/rust-toolchain`", but never
    // asserted it, so `step_mapping_child_value_for_step` (byte-span
    // identity) below faithfully anchored on whichever step `.find()`
    // picked — including a decoy `uses: dtolnay/rust-toolchain-anything@…`
    // step inserted earlier in `steps:`. Now routed through
    // `find_sole_step_by`, which errors (naming the count) on zero or more
    // than one match instead of silently preferring the first. The prefix
    // itself is also now anchored at the `@` version-pin boundary
    // (`"dtolnay/rust-toolchain@"`, not the bare `"dtolnay/rust-toolchain"`
    // substring) — the un-anchored form matched any `uses:` value merely
    // starting with that text, including a same-org decoy action name like
    // `dtolnay/rust-toolchain-anything@<sha>`.
    let dtolnay_step = find_sole_step_by(
        &job.steps,
        "with a `uses:` value starting with \"dtolnay/rust-toolchain@\"",
        |s| {
            matches!(
                s.value_of("uses"),
                Some(Value::Scalar { text, .. }) if text.starts_with("dtolnay/rust-toolchain@")
            )
        },
    )
    .unwrap_or_else(|reason| {
        panic!(
            "FAIL (S-626-1 AC-3 / S-CIGATE-3 fix-burst-6/7): the `msrv` job \
             {reason}\n\
             Current msrv job block:\n{msrv_block}"
        );
    });

    // toolchain: "1.85.0" on the dtolnay/rust-toolchain step's `with:` block.
    match common::wf::step_mapping_child_value_for_step(
        msrv_block,
        dtolnay_step,
        "with",
        "toolchain",
    ) {
        Some(Value::Scalar { text, style, .. }) => {
            assert_eq!(
                text, "1.85.0",
                "FAIL (S-626-1): the `msrv` job's `with.toolchain` value is \
                 {text:?}, not \"1.85.0\".\n\
                 Current msrv job block:\n{msrv_block}"
            );
            assert_eq!(
                style,
                ScalarStyle::DoubleQuoted,
                "FAIL (S-626-1, AC-004 quoting fidelity): the `msrv` job's \
                 `with.toolchain: \"1.85.0\"` value must be a DOUBLE-QUOTED \
                 scalar (matching ci.yml's current spelling) — found \
                 {style:?} instead. A bare `toolchain: 1.85.0` (unquoted) \
                 would parse to a YAML FLOAT, not the string \"1.85.0\" the \
                 `dtolnay/rust-toolchain` action expects — re-quoting it any \
                 other way is a real behavioral change this pin must catch, \
                 not a cosmetic one.\n\
                 Current msrv job block:\n{msrv_block}"
            );
        }
        other => panic!(
            "FAIL (S-626-1): The `msrv` job does not pin `toolchain: \"1.85.0\"` \
             on its `dtolnay/rust-toolchain` step (found {other:?} instead of \
             a scalar `with.toolchain`).\n\
             Required: at the pinned SHA (`fa04a1451ff1842e2626ccb99004d0195b455a88`), \
             `toolchain` is a required input with no default — omitting the `with:` \
             block does not fall back to `rust-toolchain.toml` or a default; the \
             action exits 1 with `'toolchain' is a required input`, failing the job \
             loudly. The genuinely silent revert vector is removal of the \
             `RUSTUP_TOOLCHAIN` env override on the `cargo check` step (see the next \
             assertion) — that's what this input's presence guards against staying \
             meaningful.\n\
             Current msrv job block:\n{msrv_block}"
        ),
    }

    // The `cargo check --all-features --locked` step, found by its OWN
    // `run:` value via tree membership — not a byte-offset anchor + slice.
    //
    // # S-CIGATE-3 fix-burst-7 (ADV-SC3-P5-MED-001, the finding's PRIMARY
    // concrete bypass): OUTER selection is now ambiguity-checked
    //
    // This was the exact site the fix-burst-5/6 doc comments (below, on the
    // `RUSTUP_TOOLCHAIN` check) claimed had "no 'exactly one' precondition
    // left to silently violate" — true of `step_mapping_child_value_for_
    // step`'s byte-span anchoring, but NOT of the selection immediately
    // above it: `job.steps.iter().find(...)` picked the SOURCE-ORDER-FIRST
    // step with `run: cargo check --all-features --locked`, unchecked for
    // a second match. Concrete bypass (RED-proven this pass — see this
    // pass's completion report): insert a decoy step with the identical
    // `run:` text immediately before the real one, give the decoy its own
    // `env: {RUSTUP_TOOLCHAIN: "1.85.0"}`, and delete the real step's
    // `env:` block entirely. `.find()` returned the decoy; `step_mapping_
    // child_value_for_step` then faithfully resolved the DECOY's
    // `RUSTUP_TOOLCHAIN` (byte-span identity works perfectly — it was
    // never the problem), so both assertions passed while the real `cargo
    // check` step ran with no override at all, silently falling back to
    // `rust-toolchain.toml`'s `channel = "stable"` — verbatim the S-626-1
    // false-green this whole test exists to prevent. Now routed through
    // `find_sole_step_by`, which errors (naming the count) on zero or more
    // than one match instead of silently preferring the first.
    let cargo_check_step = find_sole_step_by(
        &job.steps,
        "with a `run:` value of exactly \"cargo check --all-features --locked\"",
        |s| {
            matches!(
                s.value_of("run"),
                Some(Value::Scalar { text, .. }) if text == "cargo check --all-features --locked"
            )
        },
    )
    .unwrap_or_else(|reason| {
        panic!(
            "FAIL (S-626-1 AC-3 / S-CIGATE-3 fix-burst-7): the `msrv` job \
             {reason}\n\
             Required: without `--locked`, `cargo check` is free to \
             re-resolve other and transitive dependencies against \
             `Cargo.toml` at check time, decoupling the MSRV check from \
             the exact dependency graph the rest of CI (and users) \
             actually build against — a silent drift vector with no other \
             test or CI signal to catch it. A second step sharing the \
             identical `run:` text (a decoy) is flagged as ambiguous \
             rather than silently resolved to whichever came first.\n\
             Current msrv job block:\n{msrv_block}"
        );
    });
    if let Some(Value::Scalar { style, .. }) = cargo_check_step.value_of("run") {
        assert_eq!(
            *style,
            ScalarStyle::Plain,
            "FAIL (S-626-1 AC-3, AC-004 quoting fidelity): the `msrv` job's \
             `run: cargo check --all-features --locked` step must be a \
             PLAIN (unquoted) scalar — found {style:?} instead.\n\
             Current msrv job block:\n{msrv_block}"
        );
    }

    // F-02 (round 19, re-derived S-CIGATE-3 pass C; STRENGTHENED
    // fix-burst-6, ADV-SC3-P4-MED-001): `RUSTUP_TOOLCHAIN` must be an
    // `env:` override on THIS SAME step (`cargo_check_step`, already
    // resolved above by its own `run:` VALUE), not merely present on SOME
    // step that happens to have a `run:` key at all.
    //
    // The prior form of this check —
    // `step_mapping_child_value(msrv_block, "run", "env",
    // "RUSTUP_TOOLCHAIN")` — anchored on `step_anchor_key = "run"`, i.e.
    // "the first step in `steps:` that has a `run:` key present", and its
    // own doc comment claimed this was safe because "exactly ONE step in
    // this job has a `run:` key at all". That precondition was never
    // actually asserted anywhere, so it silently stopped holding the
    // moment a second `run:`-bearing step existed: a decoy step (e.g.
    // `- name: Show MSRV toolchain` with its own `run:` + `env:
    // {RUSTUP_TOOLCHAIN: "1.85.0"}`) inserted BEFORE `cargo_check_step`,
    // combined with deleting `cargo_check_step`'s own `env:` block
    // entirely, satisfied the old lookup while `cargo check` silently ran
    // under `rust-toolchain.toml`'s `channel = "stable"` — verbatim the
    // S-626-1 false-green this whole test exists to prevent. Confirmed by
    // a RED proof against a decoy copy of `ci.yml` (never the tracked
    // file): the pre-fix-burst-6 lookup returned
    // `RUSTUP_TOOLCHAIN="1.85.0"` even though `cargo_check_step` itself
    // carried no `env:` key at all.
    //
    // `step_mapping_child_value_for_step` closes this by resolving
    // `env.RUSTUP_TOOLCHAIN` against `cargo_check_step`'s own byte SPAN —
    // a unique source-text position — rather than re-scanning `steps:` for
    // "the first step with some other key present". GIVEN an already-
    // resolved `cargo_check_step`, that byte-span anchoring has no
    // "exactly one" precondition left to silently violate — span identity
    // is provably unique by construction.
    //
    // # Correction (S-CIGATE-3 fix-burst-7, ADV-SC3-P5-MED-001)
    //
    // The paragraph above was true only of `step_mapping_child_value_for_
    // step`'s OWN anchoring, not of the check as a whole: it did not
    // account for HOW `cargo_check_step` itself gets resolved in the first
    // place. Until this pass, that resolution — the `job.steps.iter()
    // .find(...)` immediately above this comment block — was itself an
    // unchecked first-match, i.e. exactly the same "exactly one"
    // precondition, one step earlier, silently unenforced. See that
    // selection's own comment for the concrete bypass this closed. Only
    // now, with `find_sole_step_by` guarding that outer selection too, is
    // the "no 'exactly one' precondition left to silently violate" claim
    // accurate end-to-end rather than for `step_mapping_child_value_for_
    // step` alone.
    match common::wf::step_mapping_child_value_for_step(
        msrv_block,
        cargo_check_step,
        "env",
        "RUSTUP_TOOLCHAIN",
    ) {
        Some(Value::Scalar { text, style, .. }) => {
            assert_eq!(
                text, "1.85.0",
                "FAIL (S-626-1): the `msrv` job's `cargo check` step's \
                 `env.RUSTUP_TOOLCHAIN` value is {text:?}, not \"1.85.0\".\n\
                 Current msrv job block:\n{msrv_block}"
            );
            assert_eq!(
                style,
                ScalarStyle::DoubleQuoted,
                "FAIL (S-626-1, AC-004 quoting fidelity): \
                 `RUSTUP_TOOLCHAIN: \"1.85.0\"` must be a double-quoted \
                 scalar (matching ci.yml's current spelling) — found \
                 {style:?} instead.\n\
                 Current msrv job block:\n{msrv_block}"
            );
        }
        other => panic!(
            "FAIL (S-626-1 AC-3 / F-02): `RUSTUP_TOOLCHAIN: \"1.85.0\"` is \
             not an `env:` child of THE `cargo check --all-features \
             --locked` step itself (found {other:?} instead) — this check \
             is anchored to that exact step's own byte span, not to \
             \"some step with a `run:` key\", so a decoy step elsewhere in \
             `steps:` cannot satisfy it.\n\
             `RUSTUP_TOOLCHAIN` outranks `rust-toolchain.toml` at PROCESS \
             level — it must be an `env:` override on the `cargo check` \
             step itself. Setting it on any other step (e.g. the \
             `dtolnay/rust-toolchain` step) only affects that step's own \
             process; `cargo check` would then run with no override, \
             `rust-toolchain.toml`'s `channel = \"stable\"` would win, and \
             the job would silently validate stable again.\n\
             Current msrv job block:\n{msrv_block}"
        ),
    }
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
/// same workflow-level override. Fixed (round 12) by reusing
/// `extract_key_name_at_indent(l, 0)` — the same quote/whitespace-aware
/// key matcher round 11 already built for job/step-level key sets.
///
/// # S-CIGATE-3 pass F (FINAL): rewritten on `WfDoc::parse`'s `root_keys`
///
/// `WfDoc::root_keys` (event-stream backed — the document root mapping's
/// own direct key names, resolved by tree membership) closes the
/// `POSITIONAL-ASSUMPTION-AXIS` drift item for this guard the same way
/// `Job::keys` closed it for every job-scoped guard: there is no indent
/// literal left to hard-code at all (the pre-parser fix above was already
/// indent-0-correct for THIS file's own convention, but "indent 0" is
/// itself still a position assumption a re-derived line scan could get
/// wrong for a hypothetical BOM-prefixed or oddly-formatted sibling file —
/// tree membership has no such assumption to make). Every key spelling
/// (`defaults:`/`"defaults":`/`'defaults':`/`defaults :`) resolves
/// identically for the same reason it does everywhere else in this file: a
/// real YAML parser treats them as the same key regardless of quoting.
/// `root_keys` is ALSO immune to a node property directly on the key
/// (`&x defaults:` / `!!str defaults:`, the round-16 residual) for the
/// same reason `Job::keys`/`Step::keys` are: `read_mapping` resolves a
/// key's TEXT from its `Event::Scalar`, independent of any anchor/tag
/// riding along on the same key — `read_ci_yml()`'s line-based reader
/// (this test's ancestor at round 11/12) had none of these guarantees.
#[test]
fn test_ci_yml_has_no_workflow_level_shell_override() {
    let ci = read_ci_yml();
    let doc = WfDoc::parse(&ci);
    let has_top_level_defaults = doc.root_keys.iter().any(|k| k == "defaults");

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

/// S-CIGATE-3 fix-burst-3 (ADV-SC3-P1-MED-004): the document root's own
/// COMPLETE key set, pinned as one sorted list — see
/// `PINNED_WORKFLOW_ROOT_KEYS`'s own doc comment for why this closes an
/// adjacent gap next to the two single-key checks above (`defaults:`
/// absence, `env:` presence) rather than duplicating either of them.
#[test]
fn test_ci_yml_workflow_root_key_set_is_pinned() {
    let ci = read_ci_yml();
    let doc = WfDoc::parse(&ci);
    let mut actual_root_keys = doc.root_keys.clone();
    actual_root_keys.sort();

    let mut expected: Vec<String> = PINNED_WORKFLOW_ROOT_KEYS
        .iter()
        .map(|s| s.to_string())
        .collect();
    expected.sort();

    assert_eq!(
        actual_root_keys, expected,
        "FAIL (S-CIGATE-3 fix-burst-3, ADV-SC3-P1-MED-004): the document \
         ROOT's key set ({actual_root_keys:?}) does not match the pinned, \
         human-reviewed set ({expected:?}). This checks the workflow's \
         top-level keys as a COMPLETE set, not one name at a time — an \
         added, removed, or renamed root key (not merely `defaults:` or \
         `env:` specifically, which the two tests above already name) now \
         fails this test. If this is a deliberate, reviewed change to \
         `.github/workflows/ci.yml`'s top-level shape, update \
         PINNED_WORKFLOW_ROOT_KEYS in the SAME change. Note: `root_keys` \
         is deliberately NOT deduplicated (see `WfDoc::root_keys`'s own \
         doc comment) — a genuinely DUPLICATED root key (e.g. two \
         `env:` blocks) is in fact ALSO caught by THIS equality check \
         itself: `actual_root_keys` is a `Vec<String>`, so a duplicate \
         key lengthens it by one entry relative to `expected` (the \
         pinned, unique-by-construction list), and `assert_eq!` on two \
         `Vec`s of different length fails on that length mismatch alone \
         — there is no length-blind path here that would let a duplicate \
         silently collapse into \"the same key name twice with no \
         signal\". Separately, and independently of this test, a \
         specific duplicated root key that some OTHER guard descends \
         into by name (e.g. `env:`, via `root_level_nested_keys` in \
         `test_ci_yml_workflow_level_env_key_set_is_pinned`) is also \
         caught there, via `descend_as_mappings`'s duplicate-key panic in \
         `tests/common/wf.rs` — that is an ADDITIONAL, more specific \
         catcher for that one key, not the exclusive mechanism, and it \
         does not cover a duplicated root key nothing else descends into \
         (e.g. two `on:` blocks), which only this test's own length-\
         mismatch failure would catch."
    );
}

/// # Why this test must survive the S-CIGATE-3 real-parser rewrite
/// (retention reason, stated up front — S-CIGATE-3, ADV-SC3-P5-LOW-003)
///
/// **This test is RETAINED, UNCONDITIONALLY, as an independent layer — it
/// is NOT subsumed by `tests/common/wf.rs`'s `saphyr-parser`-backed
/// rewrite, which replaced the line-based lexer every OTHER structural
/// check in this file used to depend on.** The reason is a genuine
/// YAML-1.2-vs-YAML-1.1 divergence, not leftover caution: `saphyr-parser`
/// implements YAML 1.2, under which only `\n` and a lone `\r` are
/// recognized line breaks (its own line-break classification,
/// `char_traits.rs`'s `is_break`, matches exactly those two characters).
/// NEL (U+0085), LINE SEPARATOR (U+2028), and PARAGRAPH SEPARATOR
/// (U+2029) are YAML-1.1-only line breaks — PyYAML (the parser this
/// test's exploits below were originally verified against, and the
/// reference this project treats as authoritative for "what a real YAML
/// parser does") recognizes all four; `saphyr-parser` correctly does
/// NOT treat the latter three as ending a logical line at all. Of the
/// four characters this test scans for, the real parser therefore
/// natively subsumes exactly ONE (lone CR) — the other THREE remain
/// invisible to `saphyr-parser` the same way they were invisible to the
/// line-based lexer this story replaced. Deleting this test on the
/// assumption "we have a real parser now, so this byte scan is
/// redundant" would silently reopen two-thirds of the gap it exists to
/// close — exactly the deletion this test, and this doc comment, exist to
/// prevent.
///
/// # Original motivation (PR #671 review round 14, CRITICAL) — historical
///
/// Before the S-CIGATE-3 rewrite, `read_ci_yml()` normalized `\r\n` to
/// `\n`, but every structural check in this file then operated on
/// `str::lines()`, which splits ONLY on `\n`. YAML's own line-break
/// character set (YAML 1.1 `b-char`, which PyYAML implements) is larger:
/// CR (U+000D) alone, NEL (U+0085), LINE SEPARATOR (U+2028), and
/// PARAGRAPH SEPARATOR (U+2029) all end a LOGICAL line for a real
/// YAML-1.1 parser but do NOT split a `str::lines()` iteration on their
/// own. A key placed after one of these, on the same PHYSICAL text line
/// as a preceding key, was therefore invisible to every line-based check
/// in the file simultaneously — not one extractor's bug, a property of
/// the whole pre-S-CIGATE-3 file-reading strategy.
///
/// Reproduced, each a genuine one-line diff, PyYAML- AND Ruby-Psych/
/// libyaml-confirmed legal (independent implementations agreeing rules
/// out a single-parser quirk), actionlint-clean, and — since
/// `.gitattributes`' `text eol=lf` normalizes CRLF pairs but does NOT
/// strip a lone CR or the three multi-byte Unicode line breaks —
/// confirmed to survive a real `git add`/commit:
///   1. `CARGO_TERM_COLOR: always<CR>  BASH_ENV: /tmp/shim.sh` reopened
///      the workflow-env CRITICAL, for every job in the file.
///   2. `name: CI<CR>defaults: {{run: {{shell: "cat {{0}}"}}}}` reopened
///      the workflow-level `defaults:` CRITICAL.
///   3. Appending `<CR>        shell: cat {{0}}` to the gate step's
///      `name:` line defeated the gate step's key-set pin — over the
///      lines `str::lines()` actually produced, PyYAML sees this step's
///      keys as `["env", "name", "run", "shell"]` with `shell: "cat
///      {{0}}"`, the real GitHub runner would accept that override, and
///      the gate would `cat` its own decision script instead of running
///      it — while the pre-S-CIGATE-3 suite stayed fully green
///      throughout, because line splitting hid the smuggled key from
///      every check.
///
/// # This test closes the class by construction, not by enumeration (PR
/// #671 review round 14, reviewer-confirmed framing) — for its one
/// member (line breaks) specifically
///
/// This test asserts the raw bytes of `ci.yml` (and every sibling
/// workflow file, see below) contain none of the four characters above.
/// It needs no position assumption (it scans the WHOLE file, not a line
/// at an expected indent), no presence assumption (it does not first have
/// to LOCATE a target key before checking it), and its extractor is
/// trivial (`char_indices()` over the raw bytes has nothing to silently
/// under-report). It must NOT be read as covering the lexer layer
/// generally, then or now — it is complete for exactly the "non-LF YAML
/// line break" defect class, and the S-CIGATE-3 real-parser rewrite
/// separately closes most (not all — see the retention reason above)
/// structural gaps a line-based lexer could have, as its own gap.
/// S-626-1 pass-55 (ADV-P55-LOW-002): originally scoped to `ci.yml` alone,
/// extended to scan every sibling `.github/workflows/*.yml`/`*.yaml` file
/// `list_workflow_files` enumerates (cheaper than a second, separate test,
/// and `list_workflow_files` is already a directory walk this file
/// performs elsewhere). At the time, Guard A's helpers
/// (`list_job_ids_in_workflow`, `extract_job_display_name`) were
/// themselves still line-based, sharing the exact `str::lines()`-only
/// splitting this test exists to guard — a lone CR (or NEL / LINE
/// SEPARATOR / PARAGRAPH SEPARATOR) smuggled into a sibling workflow file
/// could hide a `name: CI Gate` job-key from Guard A's own scan the same
/// way round 14 showed it could hide a key from `ci.yml`'s extractors.
/// **Correction (S-CIGATE-3):** both helpers are now `WfDoc::parse`-backed
/// (a real parser), same as every other structural check in this file —
/// but the retention reason at the top of this doc comment applies to
/// them identically: `saphyr-parser` natively subsumes lone CR but not
/// NEL/LINE SEPARATOR/PARAGRAPH SEPARATOR, so this byte-level scan of
/// sibling files remains load-bearing for those three characters even
/// though its ORIGINAL motivation (Guard A's helpers being line-based) no
/// longer holds.
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
    //
    // # S-CIGATE-3 fix-burst-5 (ADV-SC3-P3-LOW-001): rewritten on
    // `Job::keys`/`Job::value_of`
    //
    // The old form (`gate_block.lines().any(|l| { let t = l.trim();
    // t.starts_with("if:") && t.contains("always()") })`) was the one
    // remaining line-based key-detection primitive this story's rewrite
    // deleted everywhere else — it duplicated the presence half of what
    // `extract_and_normalize_if_expr` already establishes correctly via
    // tree membership, with none of that function's spelling/indent/
    // node-property immunity. `Job::value_of("if")` resolves the key by
    // tree membership (same guarantee as M2-a's `has_job_level_if` above),
    // and `Value::Scalar { text, .. }.contains("always()")` inspects the
    // PARSED, already-resolved scalar text (not a raw source line), so a
    // re-quoted `if: "${{ always() }}"` or a node-property-prefixed key
    // are both still found. This was the first of two live counterexamples
    // to this file's header claim that "every structural assertion in
    // this file queries the parsed tree — not `str::lines()`/indent
    // arithmetic" (the second was M2-g's `has_run_step`, fixed the same
    // way above) — both closed in this pass, making that claim true
    // rather than merely aspirational.
    let job = WfDoc::parse_single_job(gate_block);
    let job_level_if_contains_always = job
        .value_of("if")
        .is_some_and(|v| matches!(v, Value::Scalar { text, .. } if text.contains("always()")));
    assert!(
        job_level_if_contains_always,
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
///
/// # S-CIGATE-3 finding fix (ADV-SC3-P1-MED-003, fresh-context adversarial
/// review, 2026-08-11)
///
/// This test was NOT touched by the S-CIGATE-3 migration and was left
/// behind on the two line-based primitives that migration exists to
/// eliminate: the job-level `if:` check matched only the bare `if:`
/// spelling via `l.trim_start().starts_with("if:")`, and the step-level
/// check hard-coded an 8-space indent AND the bare spelling via
/// `l.starts_with("        if:")` — both the `POSITIONAL-ASSUMPTION-AXIS`
/// and the spelling-variant axis this whole story exists to close.
///
/// **Verified working bypass (RED-proven this session against an
/// in-memory copy of the real `mutants` block — the tracked `ci.yml` was
/// never modified):** adding `        "if": false` (quoted spelling, 8
/// spaces) as a sibling of `continue-on-error: true` on the "Run mutation
/// tests on PR diff" step left `step_if_lines.len() == 1` and
/// `step_if_lines[0].contains("always()")` both still true — the smuggled
/// key was invisible to the old checker, and mutation testing would
/// silently never run (the step's own `if: false` short-circuits it)
/// while this test, and the rest of the suite, stayed green. The same
/// bypass generalizes: any of the three alternate `if:` spellings
/// (`"if":`, `'if':`, `if :`) at the natural 8-space indent, OR a bare
/// `if:` smuggled onto an entirely NEW step at a self-chosen indent (a
/// list entry may pick its own content indent independently of its
/// siblings — tested at both 6 and 10 spaces), each defeated the old
/// checker identically.
///
/// **Fix:** both checks are rewritten on [`WfDoc::parse_single_job`] +
/// tree membership, mirroring `job_declares_job_level_key` (job-level) and
/// the `has_step_level_if` pattern already used by
/// `test_ci_gate_pass_fail_semantics_are_structurally_placed`'s M2-d
/// (step-level): the job-level guard now reuses
/// `extract_and_normalize_if_expr` (already AC-004 quoting-fidelity
/// compliant — rejects any non-`ScalarStyle::Plain` value outright); the
/// step-level guard counts `job.steps[i].keys` membership (immune to both
/// spelling and indent by construction — there is no indent literal or
/// enumerated spelling list left to be incomplete), then reads the sole
/// matching step's `if:` value via `Step::value_of` and requires
/// `ScalarStyle::Plain` (AC-004 mandate applied to this newly-introduced
/// value pin) plus no YAML tag before trusting its text.
#[test]
fn test_mutants_job_structure_unchanged_by_cigate2_option_c() {
    let ci = read_ci_yml();
    let mutants_block = extract_job_block(&ci, "mutants").unwrap_or_else(|| {
        panic!("FAIL: `.github/workflows/ci.yml` does not contain a `mutants:` job.")
    });

    // Job-level if: unchanged — PR-only scope, the exact fact
    // `scripts/check-ci-gate.sh`'s `ALLOWED_SKIPS` allowlist is built to
    // tolerate (mutants reports `skipped` on push BECAUSE of this guard).
    //
    // Tree-based via `extract_and_normalize_if_expr` (S-CIGATE-3 finding
    // fix, ADV-SC3-P1-MED-003): immune to spelling/indent variance and
    // enforces the AC-004 `ScalarStyle::Plain` mandate on the value itself.
    let actual_job_if_expr =
        extract_and_normalize_if_expr(mutants_block).unwrap_or_else(|reason| {
            panic!(
                "FAIL (S-CIGATE-2 AC-006): `mutants`'s job-level `if:` {reason}\n\
             Current mutants block:\n{mutants_block}"
            )
        });
    assert_eq!(
        actual_job_if_expr.as_deref(),
        Some("github.event_name == 'pull_request'"),
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

    // Exactly one step-level `if:` must exist in the block — the
    // pre-existing `if: always()` on `Check kill rate`. A NEW step-level
    // `if:` on any of the other five steps would be the rejected Option
    // B's signature edit (moving PR-only gating from job level down to
    // individual steps).
    //
    // S-CIGATE-3 finding fix (ADV-SC3-P1-MED-003): rewritten on
    // `WfDoc::parse_single_job` + `Job::steps[i].keys` tree membership,
    // mirroring `test_ci_gate_pass_fail_semantics_are_structurally_placed`'s
    // M2-d `has_step_level_if` pattern. Unlike the deleted
    // `l.starts_with("        if:")` scan, this has no indent literal to
    // hard-code and no enumerated spelling list to be incomplete — a step's
    // `if:` key (however spelled, however indented, on any step including
    // one smuggled in as an entirely new list entry) is found by walking
    // the parsed tree, not by re-deriving a column/text-prefix guess.
    let job = WfDoc::parse_single_job(mutants_block);
    let steps_with_if: Vec<&Step> = job
        .steps
        .iter()
        .filter(|s| s.keys.iter().any(|k| k == "if"))
        .collect();

    assert_eq!(
        steps_with_if.len(),
        1,
        "FAIL (S-CIGATE-2 AC-006): expected exactly one step-level `if:` \
         in `mutants` (the pre-existing `if: always()` on `Check kill \
         rate`), found {} (resolved by tree membership under \
         `Job::steps` — immune to `if:` key-spelling and indent variance, \
         and to a smuggled `if:` on a brand-new step).\n\
         A NEW step-level `if:` on any of the other five steps (Harden the \
         runner, checkout, install-action, rust-cache, Run mutation tests \
         on PR diff), or on an entirely new step, would be the rejected \
         Option B's signature edit — Option C requires `mutants` to remain \
         entirely unchanged.\n\
         Current mutants block:\n{mutants_block}",
        steps_with_if.len(),
    );

    // AC-004 quoting-fidelity mandate applied to this newly-introduced
    // value pin: the sole step-level `if:` value must be a plain
    // (unquoted, single-physical-line, untagged) scalar reading
    // `always()` — a re-quoted or re-tagged form is rejected outright
    // rather than resolved and trusted, matching every other rewritten
    // pin in this file (see `extract_and_normalize_if_expr`'s doc comment
    // for the full AC-004 rationale).
    match steps_with_if[0].value_of("if") {
        Some(Value::Scalar {
            text, style, tag, ..
        }) => {
            assert!(
                tag.is_none(),
                "FAIL (S-CIGATE-2 AC-006): the sole step-level `if:` in \
                 `mutants` carries a YAML tag ({tag:?}) — a node property \
                 on the value is rejected outright rather than resolved \
                 and trusted (S-CIGATE-3 AC-007).\n\
                 Current mutants block:\n{mutants_block}"
            );
            assert_eq!(
                *style,
                ScalarStyle::Plain,
                "FAIL (S-CIGATE-2 AC-006): the sole step-level `if:` in \
                 `mutants` is written in a non-plain YAML scalar style \
                 ({style:?}) — S-CIGATE-3 AC-004 treats a quoted or \
                 block-scalar `if:` value as a DIFFERENT, unpinned form \
                 even when its resolved text is identical to the plain \
                 pin.\n\
                 Current mutants block:\n{mutants_block}"
            );
            assert!(
                text.contains("always()"),
                "FAIL (S-CIGATE-2 AC-006): the sole step-level `if:` in \
                 `mutants` no longer reads `if: always()` (found: \
                 {text:?}).\n\
                 Current mutants block:\n{mutants_block}"
            );
        }
        other => panic!(
            "FAIL (S-CIGATE-2 AC-006): the sole step-level `if:` in \
             `mutants` is not a plain scalar value (found: {other:?}).\n\
             Current mutants block:\n{mutants_block}"
        ),
    }
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

// `find_comment_start` (pre-S-CIGATE-3: a hand-rolled YAML comment-boundary
// scanner over raw line text) was REMOVED in pass E. Pass B's own doc
// comment on this function explained why it survived pass B unchanged: its
// sole extraction target at the time, `extract_and_normalize_if_expr`, no
// longer called it (a real parser's resolved scalar text has comments
// already stripped per spec), but `extract_and_normalize_step_run_line_by_
// name` — owned by this pass, not yet migrated as of pass B's commit — still
// did, against raw line text. Pass E's rewrite of
// `extract_and_normalize_step_run_line_by_name` onto `Job::steps`/
// `Step::values` removed that last caller for the same reason: the real
// parser has already resolved the `run:` scalar's text with any trailing
// YAML comment stripped, so there is no comment boundary left for a
// hand-rolled scanner to find. With zero remaining callers, the function was
// deleted rather than left as dead code (`-D warnings` would reject an
// `#[allow(dead_code)]` workaround, and this repo's convention is to remove
// unused code, not suppress the lint).

/// Extract a `ci.yml` job block's job-level `if:` expression and apply the
/// narrow normalization used for pinned-literal comparison.
///
/// Returns:
///   - `Ok(None)` if the job has no job-level `if:` at all.
///   - `Ok(Some(expr))` if it has a single-line, unambiguously-delimited
///     `if:` expression.
///   - `Err(reason)` if the `if:` value is in a YAML form this function
///     cannot FAITHFULLY represent as a single string — the caller must
///     treat this as an immediate, unconditional test failure naming the
///     job, never as "no pin" or "pin mismatch".
///
/// **SIGNATURE FROZEN (S-CIGATE-3 pass B):** `job_block: &str ->
/// Result<Option<String>, String>` is unchanged. Two `#[cfg(unix)]` tests
/// (`test_ci_gate_decision_matches_job_level_if_for_every_needs_member`,
/// `test_ci_gate_decision_is_arity_independent_for_unlisted_skips`) call
/// this for EVERY `ci-gate.needs` member's `job_block` (not only
/// `ci-gate`'s own) and needed zero edits for this rewrite.
///
/// # S-CIGATE-3 pass B: rewritten on `WfDoc::parse_single_job`, event-
/// stream backed
///
/// Job-level `if:` detection is no longer a line-indent scan — `job_block`
/// is parsed once via [`WfDoc::parse_single_job`], and the `if:` key is
/// found by TREE MEMBERSHIP (`Job::keys`/`Job::value_of`), which is
/// immune, by construction, to every spelling variant (`if:`/`"if":`/
/// `'if':`/`if :`) and every job-body indent (this checker's old
/// `extract_key_name_at_indent(l, 4)` hard-coded 4 spaces; a real parser
/// has no such literal to get wrong — see `POSITIONAL-ASSUMPTION-AXIS`,
/// closed by this rewrite per AC-008).
///
/// A duplicate job-level `if:` key is still a hard `Err` (defense in
/// depth — GitHub Actions/`actionlint` already reject a duplicate mapping
/// key as a parse error, but this checker does not rely on that alone):
/// `saphyr-parser` does NOT collapse duplicate keys (see `tests/common/
/// wf.rs` module docs), so `Job::keys` genuinely contains two `"if"`
/// entries in that case, detected by counting occurrences.
///
/// # AC-004 quoting-fidelity mandate (orchestrator ruling, human-approved
/// this session)
///
/// The real parser resolves `if: ${{ always() }}`, `if: "${{ always() }}"`,
/// and `if: '${{ always() }}'` to the IDENTICAL scalar text, differing only
/// in `ScalarStyle`. The pre-parser byte-comparison pin implicitly rejected
/// the two re-quoted forms (the raw quote characters survived into the
/// compared string and never matched an unquoted pin). To preserve that
/// exact strictness — not weaken it — this function requires
/// `style == ScalarStyle::Plain`; anything else (`SingleQuoted`,
/// `DoubleQuoted`, or a block-scalar `Literal`/`Folded`) is a hard `Err`.
/// This one check subsumes the old CRITICAL-1a block-scalar-header
/// rejection: `Literal`/`Folded` are their own `ScalarStyle` variants,
/// distinct from `Plain`.
///
/// A YAML tag (e.g. `!!str`) on the VALUE is likewise rejected outright —
/// same rationale, a node property this checker refuses to resolve and
/// trust rather than risk silently accepting a re-tagged form.
///
/// The old CRITICAL-1b "value folds onto a following line" rejection is
/// preserved too, even though the real parser CAN correctly resolve a
/// folded multi-line plain scalar: accepting a source form the old checker
/// hard-rejected — even if its resolved text still matches the pin — would
/// be a behavioral loosening this pass's mandate forbids. Detected via
/// `Value::Scalar`'s `start_line`/`end_line` (S-CIGATE-3 pass B addition to
/// `tests/common/wf.rs`): `start_line != end_line` means the scalar's
/// source spanned more than one physical line.
///
/// CRITICAL-2 (embedded `#` ambiguity) no longer needs its own check: the
/// real parser has already stripped a legitimate trailing YAML comment
/// (whitespace-preceded `#`, per spec) when resolving the scalar's text —
/// there is no comment-boundary decision left for this function to get
/// wrong, and no ambiguous embedded `#` can survive into `text` at all
/// (an unquoted `#` not preceded by whitespace is either consumed as part
/// of the plain scalar's content by the real parser, exactly as YAML
/// requires, or the document fails to parse — never silently misread).
///
/// NORMALIZATION APPLIED, after the rejections above: internal whitespace
/// runs are collapsed to one space (`split_whitespace().join(" ")`) —
/// preserved from the pre-parser version as a defensive measure against a
/// purely cosmetic multi-space reformat, even though it is not currently
/// known to be reachable now that folded continuations are hard-rejected.
///
/// Deliberately NOT normalized: a `${{ ... }}` wrapper is left AS-IS —
/// same rationale as before this rewrite (see git history for the
/// pre-S-CIGATE-3 doc comment's fuller rationale, if this exact accounting
/// is ever needed again: being more literal is strictly safer than
/// guessing which textual variations are "equivalent").
fn extract_and_normalize_if_expr(job_block: &str) -> Result<Option<String>, String> {
    let job = WfDoc::parse_single_job(job_block);

    if job.keys.iter().filter(|k| k.as_str() == "if").count() > 1 {
        return Err(
            "has more than one job-level `if:` key in its ci.yml job block \
             — this is invalid YAML (a duplicate mapping key) that GitHub \
             Actions and actionlint both reject at parse time, but this \
             checker refuses to silently pick a winner rather than rely on \
             that external validation. Remove the duplicate `if:` key."
                .to_string(),
        );
    }

    match job.value_of("if") {
        None => Ok(None),
        Some(Value::Scalar {
            text,
            style,
            tag,
            has_anchor,
            start_line,
            end_line,
        }) => {
            if *has_anchor {
                return Err("has a job-level `if:` value carrying a YAML anchor \
                     (`&...`) — S-CIGATE-3 B-1 fix \
                     (VALUE-SIDE-ANCHOR-GAP-UNCLOSED): a node property on \
                     a pinned key's VALUE is rejected outright rather than \
                     resolved and trusted, the same as a value-side tag."
                    .to_string());
            }
            if tag.is_some() {
                return Err(format!(
                    "has a job-level `if:` value carrying a YAML tag \
                     ({tag:?}) — S-CIGATE-3 AC-007/round-16: a node \
                     property on a pinned key's VALUE is rejected outright \
                     rather than resolved and trusted."
                ));
            }
            if *style != ScalarStyle::Plain {
                return Err(format!(
                    "has a job-level `if:` value written in a non-plain \
                     YAML scalar style ({style:?}) — S-CIGATE-3 AC-004: \
                     this checker treats a quoted or block-scalar `if:` \
                     value as a DIFFERENT, unpinned form even when its \
                     resolved text is identical to a plain-scalar pin, \
                     preserving the strictness the pre-parser byte \
                     comparison already had. Rewrite as a plain (unquoted) \
                     scalar to make it pinnable, or update the pin's \
                     comparison design in the SAME reviewed change if a \
                     quoted form is now deliberately intended."
                ));
            }
            if start_line != end_line {
                return Err(format!(
                    "has an `if:` value whose YAML source spans physical \
                     lines {start_line}..={end_line} (a folded plain \
                     scalar) — this cannot be safely represented as a \
                     single pinned literal, even though this checker's \
                     real YAML parser CAN correctly resolve the folded \
                     text; accepting that source form would be a \
                     behavioral loosening versus the pre-parser checker's \
                     unconditional rejection of it. Rewrite as a \
                     single-physical-line plain scalar to make it \
                     pinnable."
                ));
            }

            let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if collapsed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(collapsed))
            }
        }
        Some(Value::Alias) => Err(
            "has a job-level `if:` value that is a YAML alias (`*anchor`) \
             reference — this checker does not resolve an alias to its \
             `&anchor` definition's value (see tests/common/wf.rs's module \
             docs, \"Aliases are not resolved\"), so it cannot safely \
             compare it against a plain-scalar pin without risking a \
             silent miss."
                .to_string(),
        ),
        Some(Value::Other) => Err("has a job-level `if:` value that is a nested mapping or \
             sequence — not a valid GitHub Actions `if:` expression form. \
             Investigate this workflow file directly; this checker \
             refuses to guess at what GitHub Actions would evaluate it as."
            .to_string()),
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
/// to serve a second caller risks reopening a closed class of bug in the
/// one place most likely to hide it. The normalization RULES stay
/// intentionally duplicated (reject-don't-parse: non-plain style, a
/// multi-physical-line span, or a value tag are all hard errors) — only
/// the target key (`run` vs. `if`) and the SEARCH SCOPE (every step, vs.
/// the job's own direct keys) differ.
///
/// # S-CIGATE-3 pass B: rewritten on `WfDoc::parse_single_job`
///
/// `run:` is a STEP-level key, so — unlike `extract_and_normalize_if_expr`,
/// which looks at `Job::keys` directly — this walks `Job::steps`, counting
/// every occurrence of a `"run"` key across ALL steps (this deliberately
/// mirrors the pre-parser version's semantics exactly: it too counted
/// every step-level `run:`-shaped line in the WHOLE job block as one flat
/// pool, not per-step, so "two steps each with one `run:`" and "one step
/// with `run:` declared twice" were both just "2 matches" — indistinguishable
/// then and now). Tree-based `Job::steps`/`Step::keys` finds a `run:` key
/// regardless of spelling (`run:`/`"run":`/`'run':`/`run :`) or which
/// physical indent the step's child block uses — the old scanner's
/// hard-coded 8-space assumption, and the position-vs-missing ambiguity
/// its own error message used to warn about, are both gone: tree
/// membership does not have an "unrecognized position" failure mode.
///
/// Returns `Ok(String)` only for a single, unambiguous `run:` value that is
/// a `ScalarStyle::Plain` scalar (AC-004 quoting-fidelity — see
/// `extract_and_normalize_if_expr`'s doc comment for the full rationale),
/// carries no YAML tag, and whose source occupies exactly one physical
/// line (`start_line == end_line` — preserved even though the real parser
/// COULD correctly resolve a folded multi-line value, to avoid loosening
/// versus the pre-parser checker's unconditional rejection of that source
/// form). Every other case is `Err(reason)`, which the caller must treat
/// as an immediate, unconditional test failure, never as "no pin" (there
/// is only one pinned line here, not a per-job map).
fn extract_and_normalize_sole_run_line(job_block: &str) -> Result<String, String> {
    let job = WfDoc::parse_single_job(job_block);

    let run_occurrences: Vec<(&Step, usize)> = job
        .steps
        .iter()
        .flat_map(|s| {
            s.keys
                .iter()
                .enumerate()
                .filter(|(_, k)| k.as_str() == "run")
                .map(move |(i, _)| (s, i))
        })
        .collect();

    if run_occurrences.is_empty() {
        return Err("has no step declaring a `run:` key at all — the gate must \
             execute something that can fail; without one, the job \
             trivially succeeds for every upstream result."
            .to_string());
    }
    if run_occurrences.len() > 1 {
        return Err(format!(
            "has {} step-level `run:` keys (counted across all steps, and \
             within a single step if a step somehow declares `run:` more \
             than once) — this checker requires exactly one so a single \
             pinned literal unambiguously covers the gate decision. \
             Disambiguate (or, if a second `run:` step is a deliberate, \
             reviewed addition, update this checker to identify the \
             gate-decision step specifically).",
            run_occurrences.len()
        ));
    }
    let (step, key_idx) = run_occurrences[0];

    match &step.values[key_idx] {
        Value::Scalar {
            text,
            style,
            tag,
            has_anchor,
            start_line,
            end_line,
        } => {
            if *has_anchor {
                return Err("has a `run:` value carrying a YAML anchor (`&...`) — \
                     S-CIGATE-3 B-1 fix (VALUE-SIDE-ANCHOR-GAP-UNCLOSED): \
                     a node property on a pinned key's VALUE is rejected \
                     outright rather than resolved and trusted, the same \
                     as a value-side tag."
                    .to_string());
            }
            if tag.is_some() {
                return Err(format!(
                    "has a `run:` value carrying a YAML tag ({tag:?}) — \
                     S-CIGATE-3 AC-007/round-16: a node property on a \
                     pinned key's VALUE is rejected outright rather than \
                     resolved and trusted."
                ));
            }
            if *style != ScalarStyle::Plain {
                return Err(format!(
                    "has a `run:` value written in a non-plain YAML scalar \
                     style ({style:?}) — S-CIGATE-3 AC-004: this checker \
                     treats a quoted or block-scalar `run:` value as a \
                     DIFFERENT, unpinned form even when its resolved text \
                     is identical to a plain-scalar pin, preserving the \
                     strictness the pre-parser byte comparison already \
                     had (its own doc comment noted this exact tradeoff \
                     for a cosmetic single/double-quote rewrap). Rewrite \
                     as a plain (unquoted) scalar to make it pinnable."
                ));
            }
            if start_line != end_line {
                return Err(format!(
                    "has a `run:` value whose YAML source spans physical \
                     lines {start_line}..={end_line} (a folded plain \
                     scalar, or a block-scalar form) — this cannot be \
                     safely represented as a single pinned literal, even \
                     though this checker's real YAML parser CAN correctly \
                     resolve the folded text; accepting that source form \
                     would be a behavioral loosening versus the \
                     pre-parser checker's unconditional rejection of it. \
                     Rewrite as a single-physical-line plain scalar to \
                     make it pinnable."
                ));
            }

            let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if collapsed.is_empty() {
                return Err("has an empty `run:` value.".to_string());
            }
            Ok(collapsed)
        }
        Value::Alias => Err(
            "has a `run:` value that is a YAML alias (`*anchor`) reference \
             — this checker does not resolve an alias to its `&anchor` \
             definition's value, so it cannot safely compare it against a \
             plain-scalar pin without risking a silent miss."
                .to_string(),
        ),
        Value::Other => Err(
            "has a `run:` value that is a nested mapping or sequence — not \
             a valid `run:` step body. Investigate this workflow file \
             directly."
                .to_string(),
        ),
    }
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

/// Locate the SOLE step in `steps` whose `name:` value is EXACTLY
/// `step_name`. `Err` if zero or more than one step matches.
///
/// Shared by `extract_and_normalize_step_run_line_by_name` and
/// `extract_step_key_set_by_name` — factored out (S-626-1 pass-59,
/// ADV-P58-MED-001) so the duplicate-step-name rejection lives in exactly
/// one place rather than being reimplemented (and potentially
/// re-forgotten) at each new by-name step accessor.
///
/// # S-CIGATE-3 pass E: rewritten on `Job::steps`/`Step::name`
///
/// The pre-S-CIGATE-3 version of this function (S-626-1 pass-60,
/// ADV-P60-LOW-003) matched a line whose TRIMMED text was exactly
/// `- name: {step_name}` — indent-agnostic (any indent worked, per that
/// pass's doc correction), but still required `name:` to be the step's
/// SEQUENCE-MARKER key specifically (the one paired with `- `, i.e. the
/// step's FIRST mapping key). `Step::name` (`wf.rs`'s `build_step`)
/// instead finds a step's `name:` key by TREE MEMBERSHIP — anywhere in
/// that step's own mapping, not only as its first key — which closes
/// `POSITIONAL-ASSUMPTION-AXIS` for this guard (no indent literal
/// survives at all, not even an agnostic one) and every `name:` spelling
/// (`name:`/`"name":`/`'name':`/`name :`) resolves identically.
///
/// STRICTER than the old lookup, not merely equivalent, for the same
/// reason documented on `extract_test_guard_step_keys`: a decoy step
/// whose `name:` key is reordered to a LATER position in its own mapping
/// (e.g. `- run: ...\n  name: {step_name}`) was invisible to the old
/// line-exact matcher (that decoy's `name:` line never equals the
/// `- name: ...` needle) but is now a distinct `Job::steps` entry whose
/// `.name` also resolves to `step_name` — correctly flagged as ambiguous
/// here rather than silently ignored.
fn find_sole_step_by_name<'a>(steps: &'a [Step], step_name: &str) -> Result<&'a Step, String> {
    let matches: Vec<&Step> = steps
        .iter()
        .filter(|s| s.name.as_deref() == Some(step_name))
        .collect();
    match matches.as_slice() {
        [] => Err(format!(
            "has no step with `name: {step_name}` at all (checked every \
             step's own `name:` value, wherever it appears in that \
             step's mapping, via the parsed event-stream model — tree \
             membership, not a line-position scan)."
        )),
        [only] => Ok(only),
        multiple => Err(format!(
            "has {} steps named `name: {step_name}` — this checker \
             requires exactly one so a single pinned literal \
             unambiguously covers it. A decoy step sharing the real \
             step's `name:` VALUE — anywhere in that step's own mapping, \
             not only as its first (sequence-marker) key — would \
             otherwise silently win a first-match lookup instead of \
             being flagged as ambiguous.",
            multiple.len()
        )),
    }
}

/// Select the SOLE step in `steps` matching `predicate` — the generic form
/// of [`find_sole_step_by_name`], for a caller that anchors a step by
/// something other than its own `name:` value. `Err` on zero or more than
/// one match, naming the count exactly like `find_sole_step_by_name`.
///
/// # Why this exists (S-CIGATE-3 fix-burst-7, ADV-SC3-P5-MED-001 — third
/// appearance of the same defect class)
///
/// This is the THIRD time this exact shape has needed fixing in this
/// story: pass 3 found a decoy `test`-job step matched by NAME
/// (`find_sole_step_by_name` was the fix); pass 4 found the `msrv` job's
/// `with.toolchain` lookup anchored to the first step with a `run:` KEY
/// present at all (byte-span anchoring via `step_mapping_child_value_for_
/// step` was the fix); pass 5 (this one) found that fix's OWN outer
/// selection step — `job.steps.iter().find(|s| ...)`, locating the
/// `dtolnay/rust-toolchain` step and the `cargo check` step in the first
/// place, BEFORE `step_mapping_child_value_for_step` ever anchors on their
/// span — was still a raw, unchecked first-match: fix-burst-6 relocated
/// the "exactly one" precondition from "one step has `run:`" to "one step
/// has `run:` == `cargo check --all-features --locked`", but never
/// asserted it, so `step_mapping_child_value_for_step` faithfully resolved
/// whichever step `.find()` happened to pick — including a decoy inserted
/// earlier in `steps:` with the same `run:` text and its own `env:
/// {RUSTUP_TOOLCHAIN: "1.85.0"}`, while the REAL step's `env:` block is
/// deleted entirely. Point-fixing a fourth time would leave the same shape
/// reachable at the next construction (this project's precedent for a
/// recurring defect class — DEC-243, DEC-244, DEC-255 — is a class sweep,
/// not a point fix): every step-selection predicate in this file now
/// routes through this one ambiguity-checked accessor instead of a raw
/// `.find()`.
fn find_sole_step_by<'a, F>(
    steps: &'a [Step],
    description: &str,
    predicate: F,
) -> Result<&'a Step, String>
where
    F: Fn(&Step) -> bool,
{
    let matches: Vec<&Step> = steps.iter().filter(|s| predicate(s)).collect();
    match matches.as_slice() {
        [] => Err(format!("has no step {description}.")),
        [only] => Ok(only),
        multiple => Err(format!(
            "has {} steps {description} — this checker requires exactly \
             one so a single pinned literal unambiguously covers it. A \
             decoy step matching the same predicate would otherwise \
             silently win a first-match lookup instead of being flagged \
             as ambiguous.",
            multiple.len()
        )),
    }
}

/// Extract the sorted, complete key set of the SOLE step in `job_block`
/// whose `name:` value is EXACTLY `step_name`, for comparison against
/// `PINNED_CI_GATE_SELF_TEST_STEP_KEYS`.
///
/// # S-CIGATE-3 pass E: rewritten on `WfDoc::parse_single_job`/`Step::keys`
///
/// Mirrors `extract_test_guard_step_keys`'s rewrite: the step is located
/// by `find_sole_step_by_name` (tree membership via `Step::name`), and
/// its key set is `Step::keys` directly — no indent literal (the old
/// code's `extract_key_name_at_indent(l, 6).or_else(|| extract_key_name_
/// at_indent(l, 8))`) survives to get wrong.
fn extract_step_key_set_by_name(job_block: &str, step_name: &str) -> Result<Vec<String>, String> {
    let job = WfDoc::parse_single_job(job_block);
    let step = find_sole_step_by_name(&job.steps, step_name)?;
    let mut keys = step.keys.clone();
    keys.sort();
    Ok(keys)
}

/// Extract and normalize the SOLE `run:` value belonging to the step
/// whose `name:` value is EXACTLY `step_name` inside `job_block`.
///
/// A deliberately separate function from `extract_and_normalize_sole_run_
/// line` rather than a generalization of it — same precedent that
/// function's own doc comment cites for staying separate from
/// `extract_and_normalize_if_expr`: reject-don't-parse normalization is
/// duplicated, not shared. This function additionally scopes its search
/// to ONE step (the one `find_sole_step_by_name` returns) rather than
/// the whole job block, since `spec-guard` — unlike `ci-gate` — has many
/// steps and many `run:` keys; `extract_and_normalize_sole_run_line`'s
/// "exactly one `run:` in the whole block" invariant does not hold
/// there.
///
/// # S-CIGATE-3 pass E: rewritten on `Job::steps`/`Step::keys`/`Step::values`
///
/// Step lookup is `find_sole_step_by_name` (see that function's own doc
/// comment for the tree-membership rewrite and the STRICTER-not-just-
/// equivalent duplicate-name detection it now provides). Once the step is
/// found, its `run:` value is read via `Step::keys`/`Step::values` (a
/// `run:` key found anywhere in the step's mapping, any spelling, no
/// indent literal) rather than an 8-space-indent line scan — closing
/// `POSITIONAL-ASSUMPTION-AXIS` for this guard the same way pass B closed
/// it for `extract_and_normalize_sole_run_line`.
///
/// Reject-don't-parse rules mirror `extract_and_normalize_sole_run_line`
/// exactly (AC-004 quoting-fidelity mandate — see that function's doc
/// comment for the full rationale): the resolved `run:` value must be a
/// `ScalarStyle::Plain` scalar (a quoted or block-scalar `|`/`>` form is a
/// hard `Err`, even though the real parser CAN correctly resolve either —
/// accepting a source form the pre-parser checker unconditionally
/// rejected would be a behavioral loosening this pass's mandate
/// forbids), must carry no YAML tag (S-CIGATE-3 AC-007/round-16 — a node
/// property on a pinned key's VALUE is rejected outright rather than
/// resolved and trusted), and must occupy exactly one physical source
/// line (`start_line == end_line` — a folded plain scalar is rejected
/// even though its resolved text would still be safely representable).
/// The old comment-boundary/continuation-line detection this replaces
/// (`find_comment_start`, the "does the next line continue this value"
/// scan) is now handled by the real parser during resolution — there is
/// no comment-boundary decision left for this function to get wrong, and
/// a genuinely continued value is caught by the `start_line != end_line`
/// check instead.
fn extract_and_normalize_step_run_line_by_name(
    job_block: &str,
    step_name: &str,
) -> Result<String, String> {
    let job = WfDoc::parse_single_job(job_block);
    let step = find_sole_step_by_name(&job.steps, step_name)?;

    let run_indices: Vec<usize> = step
        .keys
        .iter()
        .enumerate()
        .filter(|(_, k)| k.as_str() == "run")
        .map(|(i, _)| i)
        .collect();

    if run_indices.is_empty() {
        return Err(format!(
            "'s `{step_name}` step has no step-level `run:` key at all."
        ));
    }
    if run_indices.len() > 1 {
        return Err(format!(
            "'s `{step_name}` step has {} step-level `run:` keys — this \
             checker requires exactly one so a single pinned literal \
             unambiguously covers it.",
            run_indices.len()
        ));
    }
    let key_idx = run_indices[0];

    match &step.values[key_idx] {
        Value::Scalar {
            text,
            style,
            tag,
            has_anchor,
            start_line,
            end_line,
        } => {
            if *has_anchor {
                return Err(format!(
                    "'s `{step_name}` step has a `run:` value carrying a \
                     YAML anchor (`&...`) — S-CIGATE-3 B-1 fix \
                     (VALUE-SIDE-ANCHOR-GAP-UNCLOSED): a node property on \
                     a pinned key's VALUE is rejected outright rather than \
                     resolved and trusted, the same as a value-side tag."
                ));
            }
            if tag.is_some() {
                return Err(format!(
                    "'s `{step_name}` step has a `run:` value carrying a \
                     YAML tag ({tag:?}) — S-CIGATE-3 AC-007/round-16: a \
                     node property on a pinned key's VALUE is rejected \
                     outright rather than resolved and trusted."
                ));
            }
            if *style != ScalarStyle::Plain {
                return Err(format!(
                    "'s `{step_name}` step has a `run:` value written in \
                     a non-plain YAML scalar style ({style:?}) — \
                     S-CIGATE-3 AC-004: this checker treats a quoted or \
                     block-scalar `run:` value as a DIFFERENT, unpinned \
                     form even when its resolved text is identical to a \
                     plain-scalar pin, preserving the strictness the \
                     pre-parser byte comparison already had. Rewrite as \
                     a plain (unquoted) scalar to make it pinnable."
                ));
            }
            if start_line != end_line {
                return Err(format!(
                    "'s `{step_name}` step has a `run:` value whose YAML \
                     source spans physical lines {start_line}..={end_line} \
                     (a folded plain scalar) — this cannot be safely \
                     represented as a single pinned literal, even though \
                     this checker's real YAML parser CAN correctly \
                     resolve the folded text; accepting that source form \
                     would be a behavioral loosening versus the \
                     pre-parser checker's unconditional rejection of it. \
                     Rewrite as a single-physical-line plain scalar to \
                     make it pinnable."
                ));
            }

            let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if collapsed.is_empty() {
                return Err(format!("'s `{step_name}` step has an empty `run:` value."));
            }
            Ok(collapsed)
        }
        Value::Alias => Err(format!(
            "'s `{step_name}` step has a `run:` value that is a YAML \
             alias (`*anchor`) reference — this checker does not resolve \
             an alias to its `&anchor` definition's value, so it cannot \
             safely compare it against a plain-scalar pin without \
             risking a silent miss."
        )),
        Value::Other => Err(format!(
            "'s `{step_name}` step has a `run:` value that is a nested \
             mapping or sequence — not a valid `run:` step body. \
             Investigate this workflow file directly."
        )),
    }
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
/// # S-CIGATE-3 SUPERSEDES MUCH OF THE FOLLOWING (2026-08-11, ADV-SC3-P1-MED-002)
///
/// Everything below this note is the round-16 historical record, kept
/// verbatim per this repo's "do not delete historical record" convention —
/// it is NOT current fact. A fresh-context adversarial review of the
/// S-CIGATE-3 migration found this section had gone stale in three
/// specific, harmful ways (a reviewer reading it in isolation would
/// conclude work is still open that has in fact landed, or would
/// re-litigate an already-closed question): read this note FIRST.
///
///   1. **The "follow-up story… tracked separately" this section keeps
///      promising has LANDED, not merely been re-promised.** `ci.yml` is
///      now parsed once with a real YAML 1.2 parser
///      (`saphyr-parser` 0.0.11's low-level event stream, via
///      `tests/common/wf.rs`'s `WfDoc`/`Job`/`Step`/`Value` model) and
///      every structural assertion in this file queries the parsed tree —
///      not `str::lines()`/indent arithmetic. `extract_key_name_at_indent`
///      and `collect_mapping_key_set`, the two primitives every finding
///      below is actually about, are DELETED from this file (grep
///      confirms zero remaining call sites; every surviving occurrence of
///      either name left in this file, including several above, is inside
///      a doc comment narrating this pre-migration history, not a live
///      function call). Byte-for-byte scalar pins are RETAINED as a
///      second layer, now sourced from parsed `Event::Scalar` values with
///      an added `ScalarStyle::Plain` assertion (AC-004) so a re-quoted
///      form is not silently accepted — see `extract_and_normalize_if_expr`'s
///      doc comment for the full design.
///
///      **Correction (S-CIGATE-3 fix-burst-5, ADV-SC3-P3-LOW-001):** the
///      "every structural assertion… queries the parsed tree" sentence
///      above was FALSE as stated when first written — two live
///      `assert!`s still built their condition on `gate_block.lines()`:
///      M2-g's `has_run_step` (`gate_block.lines().any(|l|
///      l.trim_start().starts_with("run:"))`) and AC-001's job-level
///      `if: always()` check inside
///      `test_ci_gate_step_invokes_check_ci_gate_script_with_needs_json`
///      (`gate_block.lines().any(|l| { let t = l.trim(); t.starts_with("if:")
///      && t.contains("always()") })`). Neither was a live BYPASS — both
///      are presence assertions independently backstopped by tree-based
///      pins (M2-l's `PINNED_GATE_STEP_KEY_SETS` for the run-step case;
///      M2-a/M2-m for the job-level `if:` case) and both fail CLOSED under
///      a quoted spelling (`"if":`/`'if':` would not match `starts_with("if:")`,
///      producing a false RED, never a false GREEN) — so this was a stale
///      claim, not a reopened finding. Both are now rewritten on
///      `Job::keys`/`Job::value_of`/`Step::keys` (`has_run_step` iterates
///      `job.steps.iter().any(|s| s.keys.iter().any(|k| k == "run"))`; the
///      AC-001 check reads `job.value_of("if")`'s resolved `Value::Scalar`
///      text), making the sentence above true in fact, not only in intent.
///   2. **The round-16 node-property residual this section's "NOT
///      ENFORCED" bullet and its "NO AUTOMATED CHECK… CODE REVIEW IS THE
///      CONTROL" sentence describe is now CLOSED FOR `ci-gate`
///      specifically — verified by opening the enforcing test, not taken
///      on report.** `tests/common/wf.rs::find_key_node_properties` walks
///      the parsed event stream for any mapping key carrying an anchor or
///      tag; it is called exactly once in this file, at Assertion 12
///      (`M2-q`, AC-007) inside `test_ci_gate_pass_fail_semantics_are_
///      structurally_placed`, asserted empty against the `ci-gate` job
///      block. Both round-16 reproduction forms (`&x shell: cat {0}` and
///      `!!str shell: cat {0}`) were re-proven RED this session against a
///      temporary, untracked copy of the gate step (the tracked `ci.yml`
///      was never modified) and confirmed caught post-fix.
///      **Scope correction, so this claim is not overstated either:**
///      `find_key_node_properties` is `ci-gate`-scoped only — it is not
///      called against `mutants`, `msrv`, `spec-guard`, or any other job's
///      block. Outside `ci-gate`, a node-propertied key that is ALREADY a
///      legitimate member of a pinned set (e.g. `&x run: …` on an
///      existing step) is not independently detected by those other
///      jobs' checks, though a *new* anchored/tagged key at any job is
///      still caught by the ordinary tree-based key-SET comparisons those
///      checks already make (per item 2's own explanation two paragraphs
///      up in the pre-existing text below) — the real parser resolves
///      `&x shell: …` to a genuine `shell` key regardless of the anchor,
///      so a key-set pin still sees it and still fails. `ci-gate` is the
///      one job this scoping matters for: it is the sole required
///      branch-protection check, so the narrower, harder-to-detect case
///      (anchor on an already-pinned key, silent under a pure key-set
///      diff) is closed exactly where the pass/fail decision actually
///      lives.
///   3. **The Guard B (matrix-staticity) cross-references elsewhere in
///      this file that call it "line-based" / "shares the round-16
///      node-property residual" are themselves stale** — see the
///      correction at that guard's own test function, which is fully
///      migrated onto `job_level_nested_value`/`job_level_nested_sequence_
///      items`/`job_level_nested_keys`.
///
///   Everything else below — `uses:` VALUES on the decision path, `with:`
///   block CONTENTS, `name:` VALUES, and human judgment on
///   `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS`/other pin-literal updates — is
///   still accurately unenforced; S-CIGATE-3 did not touch any of those,
///   and nothing above should be read as claiming otherwise.
///
///   SCOPE SUMMARY (round-16 original; **[SUPERSEDED, S-CIGATE-3 — see
///   the note at the top of this "RESIDUAL RISK" section]**, kept
///   verbatim below):
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
///   CURRENT SCOPE SUMMARY (S-CIGATE-3, 2026-08-11): Enforced — everything
///   the round-16 line above claims, PLUS the node-property class for
///   `ci-gate` specifically (M2-q/`find_key_node_properties`, verified
///   above). NOT enforced — `uses:` VALUES on the decision path; `with:`
///   block CONTENTS; `name:` VALUES; human judgment on pin-literal
///   updates; the node-property class for jobs OTHER than `ci-gate`
///   (`mutants`/`msrv`/`spec-guard`/etc. — narrowly, only for an anchor/tag
///   on a key that is ALREADY a legitimate pinned member; a NEW anchored
///   key is still caught by those jobs' ordinary key-set pins). Durable
///   fix — LANDED, not a follow-up: `WfDoc` (`tests/common/wf.rs`), built
///   on `saphyr-parser` 0.0.11's event stream.
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
///   - **[SUPERSEDED, S-CIGATE-3 — see item 1 in the note at the top of
///     this "RESIDUAL RISK" section.]** `extract_key_name_at_indent` no
///     longer exists in this file; it and everything built on it were
///     deleted and replaced by `WfDoc`'s event-stream tree walk. This
///     bullet is kept verbatim as the historical description of the
///     defect that migration fixed, not as a description of this file's
///     current extraction mechanism. This file's line-based LEXER layer
///     (`extract_key_name_at_indent`
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
///   **[SUPERSEDED for `ci-gate` — see the S-CIGATE-3 note at the top of
///   this "RESIDUAL RISK" section, item 2.]** This sentence is now false
///   as a blanket claim: `test_ci_gate_pass_fail_semantics_are_structurally_
///   placed`'s M2-q assertion (`find_key_node_properties`) IS an automated
///   check that catches the exact `&x shell: cat {0}` / `!!str shell: cat
///   {0}` bypass this paragraph describes, scoped to `ci-gate`. Kept
///   verbatim below rather than deleted (historical record); it remains
///   literally true only for jobs OTHER than `ci-gate` (see the scope
///   correction above) and for the unrelated items this section's closing
///   paragraph lists (`uses:` values, pin-literal judgment).
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
///   **[LANDED, not merely "tracked" — S-CIGATE-3, 2026-08-11. See the
///   note at the top of this "RESIDUAL RISK" section, item 1.]** This is
///   no longer an open follow-up: `tests/common/wf.rs`'s `WfDoc` does
///   exactly what this paragraph specifies, off-the-shelf `saphyr-parser`
///   0.0.11 (not a PyYAML shell-out), asserted over the parsed tree with
///   byte-for-byte scalar pins retained as the second layer this paragraph
///   describes. Kept verbatim below as the design rationale that was
///   actually followed, not as an open item.
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

/// S-CIGATE-3 fix-burst-3 (ADV-SC3-P1-MED-004): the document ROOT's own
/// complete key set — `name`, `on`, `env`, `jobs` — pinned as a complete
/// set for the same reason `PINNED_WORKFLOW_ENV_KEYS` pins `env:`'s
/// children one level down: `WfDoc::root_keys` is deliberately NOT
/// deduplicated (see that field's own doc comment) and this file's other
/// document-root checks (`test_ci_yml_has_no_workflow_level_shell_
/// override`'s `defaults:` absence check,
/// `test_ci_yml_workflow_level_env_key_set_is_pinned`'s `env:` presence
/// check) each name ONE root key at a time — an open enumeration that a
/// smuggled sibling root key nobody thought to name explicitly (e.g. a
/// second `permissions:` block, or any future root-level construct this
/// suite has not yet been taught to check for by name) could slip past
/// entirely. Asserting the FULL root key set as one complete, sorted,
/// human-reviewed list closes that adjacent gap in one assertion: any
/// ADDED, REMOVED, or RENAMED root key — not merely `defaults:`/`env:`
/// specifically — now fails this test. `descend_as_mappings`'s own
/// duplicate-key panic (`tests/common/wf.rs`) is the complementary half:
/// this const catches an unexpected DISTINCT root key; that panic catches
/// a genuinely DUPLICATED one (e.g. a second `env:` block), which
/// `root_keys`'s deliberate non-deduplication would otherwise let sit in
/// this list twice with no signal that anything is wrong.
const PINNED_WORKFLOW_ROOT_KEYS: &[&str] = &["env", "jobs", "name", "on"];

/// **DELETED (S-CIGATE-3 pass F, FINAL migration pass):** `collect_mapping_
/// key_set` used to live here — a line-based scanner collecting a mapping
/// block's children at a hard-coded `child_indent`, with the round-13
/// comment-skipping fix described in its own now-deleted doc comment. Its
/// sole real caller, `extract_workflow_env_key_set`, was rewritten on
/// `common::wf::root_level_nested_keys` (tree-membership-based, no indent
/// literal, immune to the exact comment-truncation class round 13 fixed
/// here by hand). Confirmed before deleting: zero remaining callers in this
/// file. Deleted together with `extract_key_name_at_indent` immediately
/// below — see that deletion note for the fuller rationale, since the two
/// primitives were always a matched pair (this function existed only to
/// apply that one, repeatedly, over a block's children).
///
/// Extract the sorted, complete key set of the gate step's `env:` block for
/// comparison against `PINNED_GATE_ENV_KEYS`.
///
/// # S-CIGATE-3 pass B: rewritten on `step_mapping_child_keys`
/// (event-stream backed, `tests/common/wf.rs`)
///
/// Anchors to the STEP whose OWN keys include `run` (mirroring
/// `extract_and_normalize_sole_run_line`'s "the run-bearing step" anchor,
/// via `step_mapping_child_keys(job_block, "run", "env")`) — by TREE
/// MEMBERSHIP, not textual proximity. This is a real, not merely
/// cosmetic, improvement over the pre-parser version (PR #671 review round
/// 14 SUGGESTION, previously documented as a known limitation, not fixed):
/// the old scanner only searched BACKWARD from the `run:` line for `env:`,
/// so a legal-but-reordered `env:`-after-`run:` on the same step was
/// indistinguishable from a genuinely missing `env:` block (both returned
/// an empty `Vec`, caught only as an accident of M2-o's separate
/// non-empty assertion). `step_mapping_child_keys` finds `env:` regardless
/// of its position relative to `run:` within the step's own mapping —
/// YAML mapping keys are genuinely unordered, and this function no longer
/// assumes otherwise.
///
/// Returns an empty `Vec` if no step has both a `run:` key and an `env:`
/// key, or if `env:`'s value is not itself a mapping — `M2-o`'s own
/// non-empty assertion in the caller (`test_ci_gate_pass_fail_semantics_
/// are_structurally_placed`) is still the operative "this must not be
/// silently treated as nothing to worry about" backstop.
///
/// # Accepted residual: first-match anchoring, not ambiguity-checked
/// (adversarial pass 5, S-CIGATE-3, LOW-3)
///
/// See `tests/common/wf.rs`'s `find_key_node_properties` doc comment,
/// "Additional accepted residuals" § LOW-3, for the full analysis of why
/// this first-match-by-`step_anchor_key` anchoring is backstopped (not
/// exploitable) by `PINNED_GATE_STEP_KEY_SETS`/`PINNED_GATE_JOB_KEYS` and
/// therefore left as-is rather than fixed here.
fn extract_gate_env_key_set(job_block: &str) -> Vec<String> {
    common::wf::step_mapping_child_keys(job_block, "run", "env").unwrap_or_default()
}

/// Extract the sorted, complete key set of the WORKFLOW's own top-level
/// `env:` block for comparison against `PINNED_WORKFLOW_ENV_KEYS`. Reads
/// `ci` (the whole file), not a `job_block` — see
/// `PINNED_WORKFLOW_ENV_KEYS`'s doc comment for why this construct is
/// outside every job block by construction.
///
/// # S-CIGATE-3 pass F (FINAL): rewritten on `root_level_nested_keys`
/// (event-stream backed, `tests/common/wf.rs`)
///
/// Mirrors `extract_gate_env_key_set`'s own pass-B rewrite one level up —
/// document root instead of a step. The pre-parser version anchored on
/// `extract_key_name_at_indent(l, 0)` to find the workflow's own `env:`
/// KEY, then `collect_mapping_key_set(&lines, env_line_idx + 1, 2)` to scan
/// its CHILDREN at a hard-coded 2-space indent — both are line-position
/// arithmetic, and round 13's CRITICAL (a YAML comment silently truncating
/// `collect_mapping_key_set`'s scan before reaching a smuggled sibling key)
/// applied to this exact call site. `root_level_nested_keys(ci, &["env"])`
/// finds `env:`'s children by tree membership under the document root's
/// mapping instead: no indent literal to hard-code (closes
/// `POSITIONAL-ASSUMPTION-AXIS`), no comment-termination bug to reintroduce
/// (a YAML parser does not treat a comment as a mapping entry in the first
/// place — there is no "scan" to truncate), and every key spelling
/// resolves identically regardless of quoting or a node property on the
/// key.
///
/// Returns an empty `Vec` if the workflow has no top-level `env:` key, or
/// its value is not itself a mapping — `test_ci_yml_workflow_level_env_
/// key_set_is_pinned`'s own non-empty assertion is still the operative
/// "this must not be silently treated as nothing to worry about" backstop,
/// mirroring `extract_gate_env_key_set`'s contract.
fn extract_workflow_env_key_set(ci: &str) -> Vec<String> {
    let mut keys = common::wf::root_level_nested_keys(ci, &["env"]).unwrap_or_default();
    keys.sort();
    keys
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
/// every check that existed before this one.
///
/// # S-CIGATE-3 pass B: rewritten on `step_mapping_child_value`
///
/// Same reject-don't-parse normalization rules as
/// `extract_and_normalize_sole_run_line` (a deliberately separate function
/// for the same reason that one is separate from
/// `extract_and_normalize_if_expr` — see that function's doc comment):
/// non-plain `ScalarStyle`, a value tag, or a multi-physical-line span are
/// all hard errors (AC-004 quoting fidelity + no source-form loosening).
/// Duplicate-key detection uses `step_mapping_child_keys` (this function's
/// sibling, already used by `extract_gate_env_key_set`) to count
/// `"NEEDS_JSON"` occurrences among the `env:` block's own keys before
/// resolving its value — tree-based, so it is immune to the
/// `POSITIONAL-ASSUMPTION-AXIS`/spelling gaps the old 10-space line scan
/// had.
///
/// # Accepted residual: first-match anchoring, not ambiguity-checked
/// (adversarial pass 5, S-CIGATE-3, LOW-3)
///
/// See `tests/common/wf.rs`'s `find_key_node_properties` doc comment,
/// "Additional accepted residuals" § LOW-3, for the full analysis of why
/// this first-match-by-`step_anchor_key` anchoring (shared with
/// `extract_gate_env_key_set` above, both via `step_mapping_child_keys`)
/// is backstopped (not exploitable) by `PINNED_GATE_STEP_KEY_SETS`/
/// `PINNED_GATE_JOB_KEYS` and therefore left as-is rather than fixed here.
fn extract_and_normalize_sole_needs_json_line(job_block: &str) -> Result<String, String> {
    let env_keys = common::wf::step_mapping_child_keys(job_block, "run", "env");
    let Some(env_keys) = env_keys else {
        return Err(
            "has no step with both a `run:` key and an `env:` mapping — \
             the gate script would run with no `NEEDS_JSON` env var set, \
             which fails closed (empty input) rather than silently, but \
             is not the pinned, reviewed input path."
                .to_string(),
        );
    };
    let count = env_keys
        .iter()
        .filter(|k| k.as_str() == "NEEDS_JSON")
        .count();
    if count == 0 {
        return Err(
            "has no `NEEDS_JSON:` env-child key at all — the gate script \
             would run with no `NEEDS_JSON` env var set, which fails \
             closed (empty input) rather than silently, but is not the \
             pinned, reviewed input path."
                .to_string(),
        );
    }
    if count > 1 {
        return Err(format!(
            "has {count} `NEEDS_JSON:` env-child entries — this checker \
             requires exactly one so a single pinned literal unambiguously \
             covers the gate's input source."
        ));
    }

    let value = common::wf::step_mapping_child_value(job_block, "run", "env", "NEEDS_JSON")
        .unwrap_or_else(|| {
            panic!(
                "wf.rs: step_mapping_child_value returned None for \
                 `NEEDS_JSON` immediately after step_mapping_child_keys \
                 confirmed it present exactly once — this indicates a bug \
                 in this module's event-index bookkeeping (both functions \
                 parse the same job_block text independently and must \
                 agree deterministically), not malformed input."
            )
        });

    match value {
        Value::Scalar {
            text,
            style,
            tag,
            has_anchor,
            start_line,
            end_line,
        } => {
            if has_anchor {
                return Err("has a `NEEDS_JSON:` value carrying a YAML anchor \
                     (`&...`) — S-CIGATE-3 B-1 fix \
                     (VALUE-SIDE-ANCHOR-GAP-UNCLOSED): a node property on \
                     a pinned key's VALUE is rejected outright rather than \
                     resolved and trusted, the same as a value-side tag."
                    .to_string());
            }
            if tag.is_some() {
                return Err(format!(
                    "has a `NEEDS_JSON:` value carrying a YAML tag \
                     ({tag:?}) — S-CIGATE-3 AC-007/round-16: a node \
                     property on a pinned key's VALUE is rejected outright \
                     rather than resolved and trusted."
                ));
            }
            if style != ScalarStyle::Plain {
                return Err(format!(
                    "has a `NEEDS_JSON:` value written in a non-plain YAML \
                     scalar style ({style:?}) — S-CIGATE-3 AC-004: this \
                     checker treats a quoted or block-scalar \
                     `NEEDS_JSON:` value as a DIFFERENT, unpinned form \
                     even when its resolved text is identical to a \
                     plain-scalar pin. Rewrite as a plain (unquoted) \
                     scalar to make it pinnable."
                ));
            }
            if start_line != end_line {
                return Err(format!(
                    "has a `NEEDS_JSON:` value whose YAML source spans \
                     physical lines {start_line}..={end_line} — this \
                     cannot be safely represented as a single pinned \
                     literal, even though this checker's real YAML parser \
                     CAN correctly resolve the folded text; accepting \
                     that source form would be a behavioral loosening \
                     versus the pre-parser checker's unconditional \
                     rejection of it."
                ));
            }

            let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if collapsed.is_empty() {
                return Err("has an empty `NEEDS_JSON:` value.".to_string());
            }
            Ok(collapsed)
        }
        Value::Alias => Err(
            "has a `NEEDS_JSON:` value that is a YAML alias (`*anchor`) \
             reference — this checker does not resolve an alias to its \
             `&anchor` definition's value, so it cannot safely compare it \
             against a plain-scalar pin without risking a silent miss."
                .to_string(),
        ),
        Value::Other => Err("has a `NEEDS_JSON:` value that is a nested mapping or \
             sequence — not a valid single-scalar env value. Investigate \
             this workflow file directly."
            .to_string()),
    }
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

/// Extract and normalize the SOLE job-level `needs:` value for
/// pinned-literal comparison against `PINNED_GATE_NEEDS_LINE`.
///
/// A deliberately separate function from `extract_and_normalize_sole_run_line`
/// / `_needs_json_line` rather than a generalization of either — same
/// precedent those two functions' own doc comments cite: reject-don't-parse
/// normalization is duplicated, not shared, so a bug in one byte-pin
/// extractor cannot silently widen into another.
///
/// # S-CIGATE-3 pass B: rewritten on `job_level_value_span`
///
/// `needs: [a, b, c]` is a SEQUENCE, not a single `Event::Scalar` — unlike
/// `if:`/`run:`/`NEEDS_JSON:`, there is no single resolved-text-plus-style
/// value to source this pin from the way AC-004 describes for a scalar.
/// Instead, this function slices the ORIGINAL SOURCE TEXT between the
/// value node's span-start and span-end (verified empirically: for a flow
/// sequence, `SequenceStart`'s span starts at the literal `[` and
/// `SequenceEnd`'s span ends immediately after the `]`) — this is still
/// tree-membership-derived (the span bounds come from the parsed node's
/// event positions, not a raw substring search over the whole file), just
/// applied to a composite value instead of a single scalar's resolved
/// text. Because it is a raw slice rather than a re-resolved value, a
/// re-quoted list ITEM (e.g. `needs: ["fmt", clippy, ...]`) is naturally
/// preserved verbatim in the sliced text and so naturally fails to match
/// an unquoted pin — no separate quoting-style check is needed here the
/// way `if:`/`run:`/`NEEDS_JSON:` need one.
///
/// Only the single-physical-line inline-array form (`needs: [a, b, c]`) is
/// supported for pinning — `ci.yml`'s current convention, mirroring
/// `parse_needs_set`'s own inline-array-first handling and preserving the
/// pre-parser checker's rejection of a block-list form (items on following
/// `- item` lines), detected here via a raw newline in the sliced span
/// text.
fn extract_and_normalize_sole_needs_line(job_block: &str) -> Result<String, String> {
    let job = WfDoc::parse_single_job(job_block);

    let needs_count = job.keys.iter().filter(|k| k.as_str() == "needs").count();
    if needs_count > 1 {
        return Err(format!(
            "has {needs_count} job-level `needs:` keys — this checker \
             requires exactly one so a single pinned literal unambiguously \
             covers the aggregated job set. This is ALSO invalid YAML (a \
             duplicate mapping key) that GitHub Actions and actionlint \
             both reject at parse time; this checker refuses to silently \
             pick a winner rather than rely on that external validation."
        ));
    }

    match job.value_of("needs") {
        None => Err(
            "has no job-level `needs:` key at all — `ci-gate` must declare \
             which upstream jobs it aggregates."
                .to_string(),
        ),
        Some(Value::Scalar { .. }) => Err(
            "has a job-level `needs:` value that is a single bare scalar \
             (e.g. `needs: fmt`), not the inline-array form \
             (`needs: [a, b, c]`) this checker supports for pinning."
                .to_string(),
        ),
        Some(Value::Alias) => Err("has a job-level `needs:` value that is a YAML alias \
             (`*anchor`) reference — this checker does not resolve an \
             alias to its `&anchor` definition's value, so it cannot \
             safely compare it against a pinned literal without risking a \
             silent miss."
            .to_string()),
        Some(Value::Other) => {
            let outcome =
                common::wf::job_level_value_span(job_block, "needs").unwrap_or_else(|| {
                    panic!(
                        "wf.rs: job_level_value_span returned None for `needs` \
                     immediately after Job::value_of confirmed a mapping/\
                     sequence value present — this indicates a bug in this \
                     module's event-index bookkeeping, not malformed \
                     input."
                    )
                });
            let span = match outcome {
                common::wf::ValueSpanOutcome::NodeProperty { has_anchor, tag } => {
                    return Err(format!(
                        "has a job-level `needs:` value carrying a YAML \
                         node property directly on the value node \
                         (anchor={has_anchor}, tag={tag:?}) — S-CIGATE-3 \
                         B-1 fix (VALUE-SIDE-ANCHOR-GAP-UNCLOSED): a node \
                         property on a pinned key's VALUE is rejected \
                         outright rather than resolved and trusted, \
                         matching every other rewritten pin in this file."
                    ));
                }
                common::wf::ValueSpanOutcome::Span(span) => span,
            };
            let raw = &job_block[span];

            if raw.contains('\n') {
                return Err("has a job-level `needs:` value that spans multiple \
                     physical lines (a block-list form) — this checker \
                     only supports the single-physical-line inline-array \
                     form (`needs: [a, b, c]`), `ci.yml`'s current \
                     convention; a multi-line form cannot be safely \
                     represented as a single pinned literal by this \
                     function."
                    .to_string());
            }
            if !raw.starts_with('[') {
                return Err(format!(
                    "has a job-level `needs:` value ({raw:?}) that is not \
                     a flow sequence (`[a, b, c]`) — this checker only \
                     supports the inline-array form for pinning."
                ));
            }

            let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
            if collapsed.is_empty() {
                return Err("has an empty `needs:` value.".to_string());
            }
            Ok(collapsed)
        }
    }
}

/// **DELETED (S-CIGATE-3 pass F, FINAL migration pass):** `extract_key_
/// name_at_indent` used to live here — the line-based, hand-rolled
/// quote/whitespace-aware key matcher every hardcoded-indent guard in this
/// file was built on (round 11's generalization of round 10's
/// `line_declares_job_level_key`, hardened across rounds 13/14/16 for BOM,
/// explicit-key syntax, non-LF line breaks, and — never fully closed by
/// this function itself — YAML node properties on a key). It, and its
/// sibling `collect_mapping_key_set` (deleted immediately above), were the
/// two primitives at the ROOT of the entire round-13/14/16 "lexer disagrees
/// with a real YAML parser" defect class this whole story exists to close
/// structurally — every prior S-CIGATE-3 pass (B, C, E) migrated one
/// cluster of callers off this function onto `common::wf`'s event-stream
/// model; this pass migrated the two remaining clusters (M2-a/M2-d's
/// job-level/step-level `if:` presence checks, now `Job::keys`/`Step::keys`
/// membership; the workflow-root `defaults:`/`env:` guards, now
/// `WfDoc::root_keys`/`root_level_nested_keys`).
///
/// Confirmed before deleting (grepped for every non-comment call site in
/// this file): zero remaining callers. Every guard this function used to
/// back is now immune, by tree-membership construction rather than by
/// enumerated case-handling, to both axes `POSITIONAL-ASSUMPTION-AXIS` and
/// `RED-PROOF-NEEDS-SPELLING-VARIANTS` named — an indented-differently job
/// body and a quoted/tagged/anchored key spelling both resolve to the
/// identical key text under `saphyr-parser`'s real YAML 1.2 parse, with no
/// indent literal or quote-form enumeration left in this file to get wrong.
/// `test_this_file_test_count_matches_expected_denominator`'s guard-test
/// count is unaffected — deleting a helper function is not deleting a
/// `#[test]`.
///
/// Extract the sorted, complete list of job-level key NAMES in
/// `job_block`.
///
/// # S-CIGATE-3 pass B: rewritten on `WfDoc::parse_single_job`
///
/// `Job::keys` (event-stream backed — tree membership under the job's own
/// mapping, not a re-derived 4-space indent literal) closes the
/// `POSITIONAL-ASSUMPTION-AXIS` drift item for this guard by construction:
/// there is no indent-column assumption left to hard-code and get wrong,
/// and every key spelling (`key:`/`"key":`/`'key':`/`key :`) resolves
/// identically, since a real YAML parser treats them as the same key.
///
/// Not deduplicated: a duplicate key is preserved (`Job::keys` never
/// collapses duplicates — see `tests/common/wf.rs` module docs) so it
/// shows up as an extra entry against `PINNED_GATE_JOB_KEYS`, which is
/// itself sorted and duplicate-free — a real duplicate key correctly
/// fails the comparison rather than silently collapsing.
fn extract_job_level_key_set(job_block: &str) -> Vec<String> {
    let job = WfDoc::parse_single_job(job_block);
    let mut keys = job.keys.clone();
    keys.sort();
    keys
}

/// Extract the sorted, complete key set of EVERY step in the job's
/// `steps:` list, in step order, as a `Vec` of per-step sorted key lists.
///
/// # S-CIGATE-3 pass B: rewritten on `WfDoc::parse_single_job`
///
/// `Job::steps` is built by walking `steps:`'s sequence directly (tree
/// membership), so a step's key set can no longer be confused with a
/// block-list-style `needs:` item (the old line scanner's own doc comment
/// noted this was "not exercised by the current file, but... correct
/// regardless" — a real parser makes that guarantee structural rather
/// than incidental: `Job::steps` is scoped to `steps:`'s sequence
/// specifically by construction, there is no shared `      -` line-prefix
/// pattern for `needs:` block-list items and step-sequence items to
/// collide on in the first place). Each step's key set is likewise immune
/// to spelling/indent variance for the same reason `extract_job_level_
/// key_set` is.
fn extract_gate_step_key_sets(job_block: &str) -> Vec<Vec<String>> {
    let job = WfDoc::parse_single_job(job_block);
    job.steps
        .iter()
        .map(|s| {
            let mut keys = s.keys.clone();
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
    // protect no longer held. `job_declares_job_level_key` below
    // recognizes all four (S-CIGATE-3 pass C: tree-based rewrite, renamed
    // from `line_declares_job_level_key` to reflect its new
    // `job_block`-based, not per-line, signature — recognizes every indent
    // too, not just 4 spaces). This guard's job is narrow enough (one
    // literal key name) that enumerating the recognized forms was
    // tractable and auditable even before the tree-based rewrite closed
    // the indent axis for free.
    for job in &all_jobs {
        let job_block = extract_job_block(&ci, job).unwrap_or_else(|| {
            panic!("FAIL: `ci-gate.needs` names `{job}`, but no `{job}:` job exists in ci.yml.")
        });
        let declares_outputs = job_declares_job_level_key(job_block, "outputs");
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

        // S-CIGATE-3 pass C: rewritten on `WfDoc::parse_single_job` — a
        // job-level `if:` key is found by TREE MEMBERSHIP, immune by
        // construction to both the spelling axis (the prior
        // `extract_key_name_at_indent` quote/whitespace-aware matcher this
        // replaces) and the indent axis (that matcher's hard-coded 4-space
        // assumption — `POSITIONAL-ASSUMPTION-AXIS`, closed here per
        // AC-008). This test is a faster diagnostic layered on top of the
        // behavioral closure in
        // `test_ci_gate_decision_matches_job_level_if_for_every_needs_member`
        // (see this function's own doc comment) — fail-closed either way,
        // consistency fix only.
        let parsed_job = WfDoc::parse_single_job(job_block);
        let has_job_level_if = parsed_job.keys.iter().any(|k| k == "if");

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
// KNOWN LIMITATION, NOW SPLIT BETWEEN THE TWO GUARDS BELOW (round-16
// residual, restated here rather than assumed known — UPDATED by
// S-CIGATE-3 pass D): originally, every extractor backing both guards was
// LINE-BASED, and a YAML NODE PROPERTY (an anchor `&name` or a tag
// `!tag`/`!!tag`) prefixing a mapping key on the same physical line
// defeated line-based key detection with zero non-LF bytes involved and
// zero line breaks — `extract_key_name_at_indent` stops at the space after
// `&x`/`!!str`, sees no colon, and returns `None`.
//
// Guard A (`list_job_ids_in_workflow`, `extract_job_display_name`,
// `test_no_sibling_workflow_declares_a_job_named_ci_gate`) was migrated
// onto `tests/common/wf.rs`'s `saphyr-parser` event-stream model in
// S-CIGATE-3 pass D and no longer inherits this residual: a real parser
// identifies a mapping key by its resolved text regardless of any
// anchor/tag prefixing it on the same line (see `tests/common/wf.rs`'s
// `read_mapping`, which matches on `Event::Scalar` text only) — the class
// of gap closes structurally for Guard A's own extractors, not just for
// the specific `&x`/`!!str shell:` reproduction case.
//
// CORRECTED (S-CIGATE-3, ADV-SC3-P1-MED-002, 2026-08-11 — verified by
// reading the test body, not taken on report): the paragraph immediately
// below this one (kept for historical record, struck through in effect
// by this correction) claimed Guard B was "UNCHANGED by pass D and still
// inherits the line-based residual… tracked as a follow-up S-CIGATE-3
// pass, not closed by this comment." That is FALSE as of this revision —
// `test_matrix_os_lists_remain_static_literals` (Guard B) IS fully
// migrated, in pass C (which preceded, and was independent of, Guard A's
// pass-D migration referenced above), onto `job_level_nested_value` /
// `job_level_nested_sequence_items` / `job_level_nested_keys` — see that
// test's own "S-CIGATE-3 pass C" doc comment a few dozen lines below,
// which is accurate and was simply never cross-referenced back up here
// when it landed. Guard B inherits the SAME closure Guard A does: a real
// parser resolves `strategy.matrix.os`/`strategy.matrix` by tree
// membership, immune to a node property on any key along that path, for
// the same reason `read_mapping` closes it for Guard A. There is no
// remaining follow-up pass for Guard B's own extractors; do not
// re-schedule migrating them.
//
// The paragraph below is retained verbatim as the historical record of
// what was believed true at the time it was written, per this repo's
// "do not delete historical record" convention — it is superseded by the
// correction immediately above, not current fact:
//
// Guard B (matrix staticity, below) is UNCHANGED by pass D and still
// inherits the line-based residual exactly as documented originally — it
// is tracked as a follow-up S-CIGATE-3 pass (durable YAML-parser rewrite),
// not closed by this comment. See `CLAUDE.md`'s CI Gate section, "Round
// 16", for the full record. Do not read Guard A's migration as having
// closed the line-based-lexer-vs-real-parser gap generally across this
// whole file; it has not — only Guard A's own two helper functions and its
// test are affected.

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
/// `ci-gate.needs`. Panics loudly instead, naming the file.
///
/// S-CIGATE-3 pass D: reimplemented on top of [`WfDoc::parse`] instead of
/// `collect_mapping_key_set`/`extract_key_name_at_indent`. The old
/// implementation's own doc comment cited a real, fixed gap (job ids
/// spelled as a flow-style entry) — that class of gap, and its sibling
/// `POSITIONAL-ASSUMPTION-AXIS` drift item (a job body indented at
/// anything other than 4 spaces), are both closed BY CONSTRUCTION here:
/// `WfDoc::parse` identifies `jobs:`'s children by tree membership, not by
/// a re-derived indent-column literal or a `:`-suffix line scan, so there
/// is no indent assumption or key-spelling lexer gap left to reproduce.
/// Verified across both axes (4 key spellings × 3/4/6/8-space job-body
/// indents) against local fixtures per this story's AC-008 — see this
/// pass's completion report, not a persisted test (`EXPECTED_GUARD_TEST_COUNT`
/// is frozen; this file's test count does not change for an internal
/// helper rewrite).
fn list_job_ids_in_workflow(content: &str, path: &Path) -> Vec<String> {
    if content.trim().is_empty() {
        return Vec::new();
    }

    let doc = WfDoc::parse(content);
    if !doc.root_keys.iter().any(|k| k == "jobs") {
        panic!(
            "FAIL (S-626-1 pass-54, ADV-P54-MED-002): {} has non-empty \
             content but no detectable top-level `jobs:` key (checked \
             against a real saphyr-parser event-stream parse of the whole \
             document — see WfDoc::root_keys). A malformed or unusually- \
             formatted sibling workflow file that genuinely defines jobs \
             would otherwise silently sit outside Guard A's coverage \
             entirely — the exact closed-enumeration shape the U1 finding \
             closed one level up for `ci-gate.needs`. If this file \
             legitimately has no `jobs:` key at all (e.g. a \
             reusable-workflow fragment with only top-level metadata), \
             narrow this panic; do not silently return an empty Vec for a \
             file with real content.",
            path.display()
        );
    }

    doc.jobs.iter().map(|j| j.id.clone()).collect()
}

/// Extract a job's job-level `name:` display value from its parsed
/// [`Job`]. Returns `None` if the job has no job-level `name:` key at all
/// (GitHub then displays the job id itself as the check name).
///
/// S-CIGATE-3 pass D: reimplemented on top of [`Job::value_of`] instead of
/// a line-by-line re-scan of a `job_block: &str` slice. The two prior
/// entire classes of fail-open gap this function used to guard against by
/// hand no longer exist to guard against:
///
/// - **Key spelling / indent position** (S-626-1 pass-56/59, ADV-P56-HIGH-002,
///   ADV-P57-HIGH-001): `Job::keys`/`Job::values` come from [`WfDoc::parse`]
///   walking the real mapping tree — a quoted key (`"name":`/`'name':`),
///   `name :` (space before colon), or a job body indented at anything
///   other than 4 spaces are all just "the mapping has a key literally
///   named `name`" to a real parser. There is no separate "detect the key"
///   vs. "re-read the value" step left to silently disagree with each
///   other, and no indent literal to hard-code and get wrong.
/// - **Value form** (S-626-1 pass-55/59, ADV-P55-MED-003, ADV-P57-LOW-001):
///   `saphyr-parser` already resolves a scalar's rendered text regardless
///   of its YAML style — block-scalar folding (`>`/`|`), quote-escape
///   sequences (`"\x43I Gate"`), and a tag prefix (`!!str CI Gate`) are all
///   just `Value::Scalar { text: "CI Gate", .. }` at this layer (see
///   `tests/common/wf.rs::Value`'s doc comment) — there is no folding or
///   escape-decoding left for this function to refuse to guess at.
///
/// The one case this function still explicitly rejects rather than guess
/// at is a `name:` value that is a YAML **alias** (`*anchor`) reference:
/// resolving it to its `&anchor` definition's text requires a document-wide
/// anchor table `tests/common/wf.rs` deliberately does not build (see that
/// module's doc comment, "Aliases are not resolved") — this is now the ONLY
/// remaining "reject, don't guess" case, down from four.
///
/// Verified across both AC-008 axes against local fixtures (not persisted
/// as new tests — `EXPECTED_GUARD_TEST_COUNT` is frozen for this pass):
/// (a) 4 key spellings (`name:`/`"name":`/`'name':`/`name :`) for both the
/// job id under `jobs:` and the job's own `name:` key, and (b) job bodies
/// at 3/6/8-space indent (in addition to the file's native 4-space
/// convention) — all resolve identically via [`WfDoc::parse`], closing the
/// `POSITIONAL-ASSUMPTION-AXIS` drift item for this guard by construction.
///
/// **Duplicate `name:` backstop (S-CIGATE-3, ADV-SC3-P6-LOW-002):** a job
/// declaring TWO `name:` keys — invalid YAML, but a real bypass vector
/// against this specific guard, which exists to catch a duplicate CHECK
/// NAME across sibling workflow files — is no longer this function's own
/// concern: `Job::value_of` itself now panics on a duplicate key rather
/// than silently resolving to the first occurrence (see its doc comment).
/// Before that fix, `job.value_of("name")` below would have silently
/// returned whichever `name:` came first, which could hide a smuggled
/// second `name: CI Gate` behind a benign-looking first `name: something
/// else`.
fn extract_job_display_name(job: &Job) -> Option<String> {
    match job.value_of("name") {
        None => None,
        Some(Value::Scalar { text, .. }) => Some(text.clone()),
        Some(Value::Alias) => panic!(
            "FAIL (S-626-1 Guard A, ADV-P57-LOW-001): job `{}`'s `name:` \
             value is a YAML alias (`*anchor`) reference. This checker \
             does not resolve an alias to its `&anchor` definition's value \
             (see tests/common/wf.rs's module docs, \"Aliases are not \
             resolved\" — that requires a document-wide anchor table this \
             codebase deliberately does not build), so it cannot safely \
             compare the alias against a plain-scalar constant like \
             \"CI Gate\" without risking a silent miss on the exact \
             duplicate-check-name collision Guard A exists to catch. \
             Resolve manually: find this alias's `&anchor` definition \
             elsewhere in the workflow file and compare its text by hand.",
            job.id
        ),
        Some(Value::Other) => panic!(
            "FAIL: job `{}`'s `name:` value is a nested mapping or \
             sequence — not valid GitHub Actions YAML for a job's display \
             name. Investigate this workflow file directly; this checker \
             refuses to guess at what GitHub would render for it.",
            job.id
        ),
    }
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
/// `ci.yml` declares a job whose resolved `name:` value equals `CI Gate`
/// case-sensitively (GitHub check names are case-sensitive).
///
/// S-CIGATE-3 pass D: `list_job_ids_in_workflow` and
/// `extract_job_display_name` are both now backed by a single
/// `WfDoc::parse` of `content` per file (see their own doc comments) —
/// this test's own "two extractors disagree about job existence" defensive
/// check below is accordingly reframed from "two structurally different
/// extractors" to "two independent parses of identical content", which is
/// a much stronger invariant (a real parser is deterministic on fixed
/// input) but kept as cheap insurance against a bug in this file's own
/// bookkeeping rather than removed outright.
///
/// See the module-level "KNOWN LIMITATION" note above this section — that
/// note documents Guard A (this test, migrated in pass D, closed by
/// construction). **Correction (S-CIGATE-3, ADV-SC3-P1-MED-002,
/// 2026-08-11):** Guard B (matrix staticity,
/// `test_matrix_os_lists_remain_static_literals`) is NOT "still
/// line-based, unaffected by this pass" — it was independently migrated
/// in pass C (which predates this pass-D test) onto
/// `job_level_nested_value`/`job_level_nested_sequence_items`/
/// `job_level_nested_keys` and is equally closed by construction. See the
/// module-level note's own correction, immediately above its historical
/// paragraph, for the full account.
#[test]
fn test_no_sibling_workflow_declares_a_job_named_ci_gate() {
    for path in list_workflow_files() {
        if path.file_name().and_then(|f| f.to_str()) == Some("ci.yml") {
            continue;
        }

        let content = read_workflow_file(&path);
        let job_ids = list_job_ids_in_workflow(&content, &path);
        let doc = WfDoc::parse(&content);

        for job_id in &job_ids {
            // S-626-1 pass-55, ADV-P55-MED-001 (reframed, S-CIGATE-3 pass
            // D): `list_job_ids_in_workflow` detected `job_id` under
            // `jobs:` via its own `WfDoc::parse(content)` call; this
            // second, independent `WfDoc::parse(&content)` over the exact
            // same string must therefore also contain a `Job` with this
            // id — a real parser is deterministic, so this is a
            // same-process, same-input invariant, not a coincidence
            // between two different algorithms the way it was pre-rewrite.
            // Kept as a defensive check (cheap, and it would be the first
            // signal of a genuine bug in this file's own bookkeeping —
            // e.g. `content` mutating between the two calls) rather than
            // removed as "now unreachable in practice".
            let Some(job) = doc.jobs.iter().find(|j| &j.id == job_id) else {
                panic!(
                    "FAIL (S-626-1 Guard A, ADV-P55-MED-001, reframed \
                     S-CIGATE-3 pass D): {} — `list_job_ids_in_workflow` \
                     detected job id `{job_id}` under `jobs:`, but a fresh \
                     `WfDoc::parse` of the SAME `content` string has no \
                     matching job. Two parses of identical content \
                     disagreeing indicates a bug in this test file's own \
                     bookkeeping (e.g. `content` was mutated between the \
                     two calls) — investigate this checker, not the \
                     workflow file, before treating this as \"nothing to \
                     check\".",
                    path.display()
                );
            };
            if extract_job_display_name(job).as_deref() == Some("CI Gate") {
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
/// See the module-level "KNOWN LIMITATION" note above this section.
/// **Correction (S-CIGATE-3, ADV-SC3-P1-MED-002, 2026-08-11):** this test
/// is NOT line-based and does NOT share the round-16 node-property
/// residual — see the "S-CIGATE-3 pass C" doc comment a few lines below,
/// inside this function's body, for the actual (tree-based) extraction
/// this test now uses. The sentence this replaces was accurate when
/// written and went stale when the pass-C migration below landed without
/// this cross-reference being updated in the same change.
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

    // S-CIGATE-3 pass C: rewritten on `job_level_nested_value` /
    // `job_level_nested_sequence_items` / `job_level_nested_keys`
    // (`tests/common/wf.rs`), event-stream backed. Every hand-rolled
    // indent-anchored line scan below (`os:` at 8-space indent, `matrix:`
    // at 6-space indent, a manual inline-array-vs-block-sequence fork) is
    // gone: `strategy.matrix.os`/`strategy.matrix` are now reached by
    // JOB-LEVEL NESTED PATH — tree membership, not indent arithmetic — and
    // `read_sequence` (`tests/common/wf.rs`) walks a flow (`[a, b]`) and a
    // block (`- a` / `- b`) sequence identically, so the old manual
    // block-sequence fallback (ADV-P55-MED-004) is no longer needed at
    // all: there is no "empty same-line value" case to special-case when
    // the sequence's items are read by tree structure regardless of which
    // physical-line shape they were written in.
    let os_path = ["strategy", "matrix", "os"];
    for job_id in &matrix_job_ids {
        let job_block = extract_job_block(&ci, job_id)
            .unwrap_or_else(|| panic!("FAIL: no `{job_id}:` job in ci.yml."));

        let value: String = match common::wf::job_level_nested_value(job_block, &os_path) {
            Some(Value::Scalar { text, .. }) => text,
            Some(Value::Other) => common::wf::job_level_nested_sequence_items(job_block, &os_path)
                .unwrap_or_else(|| {
                    panic!(
                        "FAIL (S-626-1 Guard B): `{job_id}`'s \
                             `strategy.matrix.os` value is a nested mapping, \
                             not a scalar or sequence — unsupported shape. \
                             Investigate this workflow file directly."
                    )
                })
                .join(", "),
            Some(Value::Alias) => panic!(
                "FAIL (S-626-1 Guard B): `{job_id}`'s `strategy.matrix.os` \
                 value is a YAML alias (`*anchor`) reference — this checker \
                 does not resolve aliases (see tests/common/wf.rs module \
                 docs, \"Aliases are not resolved\") and refuses to guess \
                 whether the aliased value is static."
            ),
            None => panic!(
                "FAIL (S-626-1 Guard B): `{job_id}` has no \
                 `strategy.matrix.os:` key — has this job's matrix shape \
                 changed? If `os:` moved to a different key path, update \
                 this test's `os_path` alongside the ci.yml change."
            ),
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
        // at all? Scoped to the `matrix:` mapping specifically via
        // `job_level_nested_keys(job_block, &["strategy", "matrix"])` — a
        // TREE-MEMBERSHIP lookup, so a coincidental `exclude:` living under
        // a step's `with:` block elsewhere in the job is structurally
        // unreachable from this path, not merely unlikely to collide.
        let matrix_keys = common::wf::job_level_nested_keys(job_block, &["strategy", "matrix"])
            .unwrap_or_else(|| {
                panic!(
                    "FAIL (S-626-1 Guard B, ADV-P56-INFO-001): `{job_id}` \
                         has no `strategy.matrix:` mapping, even though it \
                         has an `os:` value one level deeper — has this \
                         job's matrix nesting changed? Update this test's \
                         path alongside the ci.yml change."
                )
            });
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
/// S-CIGATE-3 fix-burst-3 (ADV-SC3-P1-MED-004): bumped 27 -> 28 for the one
/// new `#[test] fn test_ci_yml_workflow_root_key_set_is_pinned` added in
/// this pass (`PINNED_WORKFLOW_ROOT_KEYS`'s own doc comment explains why).
/// No other `#[test]` fn was added to or removed from THIS file in this
/// pass — the other new tests from this pass live in `tests/common/wf.rs`'s
/// own `#[cfg(test)] mod tests`, which `include_str!("ci_gate_completeness.rs")`
/// below does not see (by design — see this pass's completion report for
/// why those tests could not live here instead).
///
/// S-CIGATE-3 fix-burst-5 (ADV-SC3-P3-HIGH-001): bumped 28 -> 29 for the
/// one new `#[test] fn test_always_run_jobs_have_pinned_complete_job_key_
/// sets` added in this pass (`PINNED_ALWAYS_RUN_JOB_KEY_SETS`'s own doc
/// comment explains why — it closes the job-level `defaults:` shell-
/// override gap on every always-run job besides `ci-gate`). No other
/// `#[test]` fn was added to or removed from THIS file in this pass; the
/// two other findings fixed in the same pass (ADV-SC3-P3-MED-001,
/// ADV-SC3-P3-LOW-001) changed existing functions' bodies/signatures and
/// one existing test's assertions, not the `#[test]` count.
///
/// S-CIGATE-3 pass 6 (ADV-SC3-P6-MED-001): bumped 29 -> 31 for the two new
/// `#[test]` fns `test_always_run_jobs_have_pinned_complete_step_key_sets`
/// and `test_clippy_job_guard_step_key_set_is_pinned` added in this pass
/// (see the module comment above `PINNED_ALWAYS_RUN_STEP_KEY_SETS` for the
/// class-sweep rationale — closing the "a correctly-selected step is never
/// asserted to RUN AT ALL" gap on every non-matrix always-run job plus
/// `clippy`'s lint-gate step). No other `#[test]` fn was added to or
/// removed from THIS file in this pass; the three other findings fixed in
/// the same pass (ADV-SC3-P6-LOW-001, -LOW-002, -LOW-003) changed existing
/// functions' bodies/doc comments, not the `#[test]` count.
///
/// S-CIGATE-3 adversarial pass 8 (finding 2): bumped 31 -> 32 for the one
/// new `#[test] fn test_ac_008_guards_are_key_spelling_and_indent_agnostic`
/// added in this pass — the persisted, standing two-axis (key-spelling ×
/// job-body-indent) RED proof AC-008 requires, closing a HIGH drift item
/// (no such proof survived past the temporary/untracked `ci.yml` copies
/// used to produce it during earlier passes) down to LOW. No other
/// `#[test]` fn was added to or removed from THIS file in this pass.
///
/// S-CIGATE-3 (PR #680 review finding B-1 / VALUE-SIDE-ANCHOR-GAP-
/// UNCLOSED): bumped 32 -> 38 for six new `#[test]` fns —
/// `test_b1_if_expr_rejects_value_side_anchor`,
/// `test_b1_sole_run_line_rejects_value_side_anchor`,
/// `test_b1_step_run_line_by_name_rejects_value_side_anchor`,
/// `test_b1_needs_json_line_rejects_value_side_anchor`,
/// `test_b1_needs_line_rejects_value_side_anchor`, and
/// `test_b1_needs_line_rejects_value_side_tag` — added to RED-prove, then
/// pin, the fix closing the coverage regression left open by round 16's
/// `find_key_node_properties`: a YAML node property on the VALUE side of
/// an already-correctly-pinned scalar (or, for `needs:`'s flow-sequence
/// form, the composite value node itself) previously slipped past all
/// five byte-for-byte pins (`if:`, `run:` ×2 forms, `NEEDS_JSON:`,
/// `needs:`). See `tests/common/wf.rs`'s `Value::Scalar::has_anchor` field
/// and `ValueSpanOutcome` enum for the fix itself. No other `#[test]` fn
/// was added to or removed from THIS file in this pass.
const EXPECTED_GUARD_TEST_COUNT: usize = 38;

/// Collect the line indices (0-based, into `lines`) of every `#[cfg(...)]`
/// attribute in the CONTIGUOUS attribute/doc block surrounding a `#[test]`
/// line at `lines[test_idx]` — both the block immediately BEFORE it and the
/// block immediately AFTER it, up to (not including) the `fn` line.
///
/// # Why "surrounding", not just "the single line immediately above"
/// (S-CIGATE-3, ADV-SC3-P6-LOW-002)
///
/// Rust's outer attributes are order-independent — `#[test]` and
/// `#[cfg(...)]` may be written in either order relative to each other, and
/// a doc comment may sit between them. The scan this function replaces
/// looked ONLY at `lines[test_idx - 1]` (the single line immediately above
/// `#[test]`), which both of these reproductions evade, each leaving the
/// fully-green 29/29 suite silently blind to the gated test:
///
/// ```text
/// #[test]
/// #[cfg(target_os = "haiku")]
/// fn test_ci_gate_pass_fail_semantics_are_structurally_placed() { … }
/// ```
///
/// ```text
/// #[cfg(target_os = "haiku")]
/// /// any doc line
/// #[test]
/// fn test_ci_gate_pass_fail_semantics_are_structurally_placed() { … }
/// ```
///
/// This function instead walks BOTH directions from `test_idx`: BACKWARD
/// while a line is an outer attribute (`#[...]`) or doc comment
/// (`///`/`//`), collecting every line visited and stopping at the first
/// line that is neither; and FORWARD while a line is an outer attribute,
/// collecting every line visited and stopping at (not including) the `fn`
/// line. Either direction can legally hold `#[cfg(...)]`; every line
/// visited in either direction is returned to the caller for allowlist
/// checking, regardless of which side of `#[test]` the governing
/// `#[cfg(...)]` sits on.
fn attribute_window_around_test_line(lines: &[&str], test_idx: usize) -> Vec<usize> {
    let mut window = Vec::new();

    let mut i = test_idx;
    while i > 0 {
        let prev = lines[i - 1].trim();
        if prev.starts_with("#[") || prev.starts_with("///") || prev.starts_with("//") {
            window.push(i - 1);
            i -= 1;
        } else {
            break;
        }
    }

    let mut j = test_idx + 1;
    while j < lines.len() {
        let cur = lines[j].trim();
        if cur.starts_with("#[") {
            window.push(j);
            j += 1;
        } else {
            break;
        }
    }

    window
}

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
    //     with). That allowlist assertion scans the CONTIGUOUS
    //     attribute/doc block surrounding `#[test]` — both directions,
    //     not merely the single line immediately above it — see
    //     `attribute_window_around_test_line`'s own doc comment
    //     (S-CIGATE-3, ADV-SC3-P6-LOW-002) for why: Rust's outer
    //     attributes are order-independent, so `#[cfg(...)]` may
    //     legally appear either before OR after `#[test]`, with or
    //     without an intervening doc comment.
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
    // platform's build). This scan requires that every `#[cfg(...)]`
    // line found anywhere in the CONTIGUOUS attribute/doc block
    // surrounding a `#[test]` line — both immediately before it
    // (walking upward through attributes and doc comments) and
    // immediately after it up to the `fn` line — be byte-for-byte one
    // of the allowlisted forms below — reject-don't-parse, the same
    // discipline this file uses elsewhere (e.g.
    // `extract_and_normalize_if_expr`) rather than attempt to evaluate
    // arbitrary `cfg` predicate syntax.
    //
    // CORRECTED (S-CIGATE-3, ADV-SC3-P6-LOW-002): this scan previously
    // inspected ONLY `lines[i - 1]`, the single line immediately above
    // `#[test]` — a position assumption in the one guard that protects
    // every other guard in this file (M2-a..q live in the test this
    // very denominator check exists to keep from being silently
    // removed). Rust's outer attributes are order-independent, so both
    // `#[test]\n#[cfg(target_os = "haiku")]\nfn foo()` (the `#[cfg]`
    // AFTER `#[test]`) and `#[cfg(target_os = "haiku")]\n/// doc\n#[test]\nfn foo()`
    // (a doc comment between the `#[cfg]` and `#[test]`) evaded the old
    // single-line-lookback check while leaving the full suite green.
    // See `attribute_window_around_test_line`'s own doc comment for the
    // fix and both reproductions verbatim.
    const ALLOWED_TEST_CFG_GATES: &[&str] = &["#[cfg(unix)]"];
    let bad_cfg_gates: Vec<(usize, String)> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim().starts_with("#[test]"))
        .flat_map(|(i, _)| {
            attribute_window_around_test_line(&lines, i)
                .into_iter()
                .filter_map(|idx| {
                    let candidate = lines[idx].trim();
                    if candidate.starts_with("#[cfg(")
                        && !ALLOWED_TEST_CFG_GATES.contains(&candidate)
                    {
                        Some((idx + 1, candidate.to_string())) // 1-based line number.
                    } else {
                        None
                    }
                })
        })
        .collect();
    assert!(
        bad_cfg_gates.is_empty(),
        "FAIL (S-CIGATE-3, ADV-SC3-P6-LOW-002; supersedes S-626-1 \
         pass-60, ADV-P60-HIGH-001): found `#[test]` fn(s) with a \
         `#[cfg(...)]` attribute somewhere in their surrounding \
         attribute/doc block (before OR after `#[test]`, not merely the \
         single line immediately above it) that is NOT in the allowlist \
         {ALLOWED_TEST_CFG_GATES:?}: {bad_cfg_gates:?} (line number, \
         offending attribute). A `#[cfg(...)]` predicate false on every \
         platform this repo builds for \
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

/// AC-008 persisted two-axis RED proof (S-CIGATE-3, adversarial pass 8,
/// finding 2 — closes a HIGH drift item down to LOW).
///
/// Story AC-008 requires a RED proof, for every guard rewritten onto
/// `WfDoc`, crossing BOTH the key-spelling axis (`key:`/`"key":`/
/// `'key':`/`key :`, `RED-PROOF-NEEDS-SPELLING-VARIANTS`) and the
/// job-body-indent axis (a job's own direct children indented 3, 6, or 8
/// spaces instead of `ci.yml`'s native 4,
/// `POSITIONAL-ASSUMPTION-AXIS`). That proof WAS done, repeatedly,
/// during S-CIGATE-3's implementation and its seven adversarial-review
/// fix-bursts — but every one of those proofs ran against a temporary or
/// untracked `ci.yml` copy that was discarded afterward (see, e.g., the
/// round-16/S-CIGATE-3 history in this file's own module-level "RESIDUAL
/// RISK" doc comment, which narrates several such proofs in prose without
/// a single line of them surviving as a runnable test). Nothing in this
/// repository PERSISTED that proof until now — this test is that
/// persisted proof, run on every `cargo test` invocation rather than
/// once, by hand, during review.
///
/// # Why this test would catch a regression to line-based matching
///
/// `WfDoc::parse_single_job` is the single choke point every job/step
/// key-set guard in this file (`extract_job_level_key_set`,
/// `extract_gate_step_key_sets`, and every `Job::value_of`/`Step::
/// value_of`-based byte pin) routes through — proving IT is
/// spelling/indent-agnostic proves the same for all of them
/// simultaneously, which is why this test exercises it directly (plus
/// the two named wrapper functions above, to prove the wrapping itself
/// introduces no new position/spelling assumption) rather than picking
/// one arbitrary named guard.
///
/// If this file (or `tests/common/wf.rs` underneath it) were ever
/// reverted to a `str::lines()` scanner hard-coded to 4-space job-child
/// indent and the bare `key:` spelling — the exact shape every
/// S-626-1-era checker this story replaced actually had, per
/// `extract_key_name_at_indent`'s own documented history in the SCOPE
/// SUMMARY / round-16 history above this test — it would:
///   - see NOTHING at all for the 3- and 6-space indent fixtures below
///     (a hard-coded `"    key:"`-style prefix check never matches a
///     line indented 3 or 6 spaces), producing an EMPTY key set where
///     this test asserts a non-empty one containing exactly `"if"` — an
///     immediate, loud assertion failure, not a silent pass; and
///   - even at the native 4-space indent, see NOTHING for the `"if":`,
///     `'if':`, and `if :` spellings (only the bare `if:` spelling has a
///     literal `"    if:"` prefix) — three of the four spelling
///     fixtures per indent would fail the same way.
///
/// Both failure shapes are exactly what this file's own documented
/// history says the pre-S-CIGATE-3 line-based checkers actually produced
/// before being replaced — this test would have caught every one of
/// those historical regressions, and catches a reintroduction of the
/// same shape today.
#[test]
fn test_ac_008_guards_are_key_spelling_and_indent_agnostic() {
    // Spelling axis: `key:`, `"key":`, `'key':`, `key :` (space before
    // colon) — the four spellings named by `RED-PROOF-NEEDS-SPELLING-
    // VARIANTS`.
    const SPELLINGS: [&str; 4] = ["if:", "\"if\":", "'if':", "if :"];
    // Indent axis: job-body direct-child indent at 3, 6, and 8 spaces —
    // the three widths named by `POSITIONAL-ASSUMPTION-AXIS`, deliberately
    // NOT including the file's native 4-space convention.
    const INDENTS: [usize; 3] = [3, 6, 8];

    // Job-level: the full 4×3 = 12 combination cross-product.
    for &indent in &INDENTS {
        for &spelling in &SPELLINGS {
            let pad = " ".repeat(indent);
            let job_block =
                format!("gate-job:\n{pad}{spelling} always-true\n{pad}runs-on: ubuntu-latest\n");

            let job = WfDoc::parse_single_job(&job_block);
            assert!(
                job.keys.iter().any(|k| k == "if"),
                "FAIL (AC-008 RED proof): spelling {spelling:?} at indent \
                 {indent} was not detected as job-level key `if` via \
                 `WfDoc::parse_single_job` — job.keys = {:?}. This means a \
                 guard has regressed toward a line-based / hard-coded-\
                 indent matcher.\njob_block:\n{job_block}",
                job.keys
            );

            let keyset = extract_job_level_key_set(&job_block);
            assert!(
                keyset.iter().any(|k| k == "if"),
                "FAIL (AC-008 RED proof, extract_job_level_key_set): \
                 spelling {spelling:?} at indent {indent} not found in \
                 extracted key set {keyset:?}.\njob_block:\n{job_block}"
            );

            let value = job.value_of("if").unwrap_or_else(|| {
                panic!(
                    "FAIL (AC-008 RED proof): job.value_of(\"if\") \
                     returned None for spelling {spelling:?} at indent \
                     {indent}.\njob_block:\n{job_block}"
                )
            });
            match value {
                Value::Scalar { text, .. } => assert_eq!(
                    text, "always-true",
                    "FAIL (AC-008 RED proof): resolved value text \
                     differs for spelling {spelling:?} at indent \
                     {indent}: {text:?}."
                ),
                other => panic!(
                    "FAIL (AC-008 RED proof): expected a Scalar value \
                     for spelling {spelling:?} at indent {indent}, got \
                     {other:?}."
                ),
            }
        }
    }

    // Step-level: the SAME two axes, on a step's own `if:` key rather
    // than the job's — a representative subset (the bare and single-
    // quoted spellings, crossed with all three indents) rather than the
    // full 12, since step-key detection routes through the same
    // `Job::steps` tree walk the job-level loop above already exercised
    // for all four spellings; this closes the remaining "is the
    // step-scoped accessor ALSO immune" question without re-deriving the
    // whole matrix.
    for &indent in &INDENTS {
        for &spelling in &[SPELLINGS[0], SPELLINGS[2]] {
            let pad = " ".repeat(indent);
            let job_block = format!(
                "gate-job:\n{pad}runs-on: ubuntu-latest\n{pad}steps:\n{pad}- name: x\n{pad}  {spelling} always-true\n"
            );
            let job = WfDoc::parse_single_job(&job_block);
            assert_eq!(
                job.steps.len(),
                1,
                "FAIL (AC-008 RED proof, step-level setup): expected \
                 exactly one step for spelling {spelling:?} at indent \
                 {indent}.\njob_block:\n{job_block}"
            );
            assert!(
                job.steps[0].keys.iter().any(|k| k == "if"),
                "FAIL (AC-008 RED proof, step-level): spelling \
                 {spelling:?} at indent {indent} not detected as step \
                 key `if` — steps[0].keys = {:?}.\njob_block:\n{job_block}",
                job.steps[0].keys
            );

            let step_key_sets = extract_gate_step_key_sets(&job_block);
            assert!(
                step_key_sets
                    .first()
                    .is_some_and(|keys| keys.iter().any(|k| k == "if")),
                "FAIL (AC-008 RED proof, extract_gate_step_key_sets): \
                 spelling {spelling:?} at indent {indent} not found in \
                 extracted step key sets {step_key_sets:?}.\n\
                 job_block:\n{job_block}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// S-CIGATE-3 B-1 fix: value-side node-property (anchor/tag) rejection on
// the 5 byte-for-byte scalar/composite pins (`if:`, `run:` ×2 forms,
// `NEEDS_JSON:`, `needs:`).
//
// PR #680 review finding B-1 / VALUE-SIDE-ANCHOR-GAP-UNCLOSED: a YAML node
// property (`&anchor` and/or `!tag`) attached directly to the VALUE side of
// an already-correctly-pinned scalar or sequence — e.g. `run: &x echo
// "${NEEDS_JSON}" | bash scripts/check-ci-gate.sh` — resolved to
// byte-identical `text`/`style` (or, for `needs:`'s flow-sequence form, a
// byte-identical sliced span) as the un-anchored pinned form, so it slipped
// past all five pins even though `find_key_node_properties`'s KEY-side
// rejection (round-16, S-CIGATE-3) already closed the equivalent gap for a
// node property on the KEY. Root cause: `resolve_value` (`tests/common/
// wf.rs`) discarded `anchor_id` entirely, and `job_level_value_span`'s span
// started at the composite value's CONTENT, not at a preceding node-
// property token.
//
// Each test below is a RED proof: before the fix, `resolve_value` ignored
// `anchor_id` and `job_level_value_span` never inspected the value node's
// own anchor/tag, so every pin function here returned `Ok(_)` for a
// value-side-anchored (or, for `needs:`, value-side-tagged) input whose
// resolved text was otherwise byte-correct. After the fix, each returns
// `Err(_)`, mirroring the existing value-side `tag` rejection these
// functions (except the pre-fix `needs:` pin, which had no tag check
// either) already had. Each test also asserts the POSITIVE control (the
// identical fixture with the node property removed) still succeeds, so
// this is not just a "reject everything" regression.
// ---------------------------------------------------------------------------

#[test]
fn test_b1_if_expr_rejects_value_side_anchor() {
    let anchored = "gate-job:\n  if: &x ${{ always() }}\n  runs-on: ubuntu-latest\n";
    let result = extract_and_normalize_if_expr(anchored);
    assert!(
        result.is_err(),
        "FAIL (B-1 RED proof): extract_and_normalize_if_expr accepted a \
         job-level `if:` value carrying a YAML anchor instead of \
         rejecting it — got {result:?}.\njob_block:\n{anchored}"
    );

    // Positive control: the identical fixture minus the anchor must still
    // resolve normally — this pin must not have become unconditionally
    // rejecting.
    let plain = "gate-job:\n  if: ${{ always() }}\n  runs-on: ubuntu-latest\n";
    assert_eq!(
        extract_and_normalize_if_expr(plain),
        Ok(Some("${{ always() }}".to_string())),
        "FAIL (B-1 positive control): the un-anchored form of the same \
         `if:` value must still resolve successfully.\njob_block:\n{plain}"
    );
}

#[test]
fn test_b1_sole_run_line_rejects_value_side_anchor() {
    let anchored =
        "ci-gate:\n  runs-on: ubuntu-latest\n  steps:\n    - name: Gate\n      run: &x echo hi\n";
    let result = extract_and_normalize_sole_run_line(anchored);
    assert!(
        result.is_err(),
        "FAIL (B-1 RED proof): extract_and_normalize_sole_run_line \
         accepted a `run:` value carrying a YAML anchor instead of \
         rejecting it — got {result:?}.\njob_block:\n{anchored}"
    );

    let plain =
        "ci-gate:\n  runs-on: ubuntu-latest\n  steps:\n    - name: Gate\n      run: echo hi\n";
    assert_eq!(
        extract_and_normalize_sole_run_line(plain),
        Ok("echo hi".to_string()),
        "FAIL (B-1 positive control): the un-anchored form of the same \
         `run:` value must still resolve successfully.\njob_block:\n{plain}"
    );
}

#[test]
fn test_b1_step_run_line_by_name_rejects_value_side_anchor() {
    const STEP_NAME: &str = "check-ci-gate self-test (fixture suite, S-CIGATE-2)";
    let anchored = format!(
        "spec-guard:\n  runs-on: ubuntu-latest\n  steps:\n    - name: {STEP_NAME}\n      run: &x bash scripts/check-ci-gate.sh --self-test\n"
    );
    let result = extract_and_normalize_step_run_line_by_name(&anchored, STEP_NAME);
    assert!(
        result.is_err(),
        "FAIL (B-1 RED proof): extract_and_normalize_step_run_line_by_name \
         accepted a `run:` value carrying a YAML anchor instead of \
         rejecting it — got {result:?}.\njob_block:\n{anchored}"
    );

    let plain = format!(
        "spec-guard:\n  runs-on: ubuntu-latest\n  steps:\n    - name: {STEP_NAME}\n      run: bash scripts/check-ci-gate.sh --self-test\n"
    );
    assert_eq!(
        extract_and_normalize_step_run_line_by_name(&plain, STEP_NAME),
        Ok("bash scripts/check-ci-gate.sh --self-test".to_string()),
        "FAIL (B-1 positive control): the un-anchored form of the same \
         `run:` value must still resolve successfully.\njob_block:\n{plain}"
    );
}

#[test]
fn test_b1_needs_json_line_rejects_value_side_anchor() {
    let anchored = "ci-gate:\n  runs-on: ubuntu-latest\n  steps:\n    - name: Gate\n      run: echo hi\n      env:\n        NEEDS_JSON: &x ${{ toJSON(needs) }}\n";
    let result = extract_and_normalize_sole_needs_json_line(anchored);
    assert!(
        result.is_err(),
        "FAIL (B-1 RED proof): extract_and_normalize_sole_needs_json_line \
         accepted a `NEEDS_JSON:` value carrying a YAML anchor instead of \
         rejecting it — got {result:?}.\njob_block:\n{anchored}"
    );

    let plain = "ci-gate:\n  runs-on: ubuntu-latest\n  steps:\n    - name: Gate\n      run: echo hi\n      env:\n        NEEDS_JSON: ${{ toJSON(needs) }}\n";
    assert_eq!(
        extract_and_normalize_sole_needs_json_line(plain),
        Ok("${{ toJSON(needs) }}".to_string()),
        "FAIL (B-1 positive control): the un-anchored form of the same \
         `NEEDS_JSON:` value must still resolve successfully.\n\
         job_block:\n{plain}"
    );
}

#[test]
fn test_b1_needs_line_rejects_value_side_anchor() {
    let anchored = "ci-gate:\n  runs-on: ubuntu-latest\n  needs: &x [fmt, clippy]\n";
    let result = extract_and_normalize_sole_needs_line(anchored);
    assert!(
        result.is_err(),
        "FAIL (B-1 RED proof): extract_and_normalize_sole_needs_line \
         accepted a job-level `needs:` value carrying a YAML anchor \
         instead of rejecting it — got {result:?}.\njob_block:\n{anchored}"
    );

    let plain = "ci-gate:\n  runs-on: ubuntu-latest\n  needs: [fmt, clippy]\n";
    assert_eq!(
        extract_and_normalize_sole_needs_line(plain),
        Ok("[fmt, clippy]".to_string()),
        "FAIL (B-1 positive control): the un-anchored form of the same \
         `needs:` value must still resolve successfully.\njob_block:\n{plain}"
    );
}

#[test]
fn test_b1_needs_line_rejects_value_side_tag() {
    // Unlike the other four pins, `extract_and_normalize_sole_needs_line`
    // had NO tag check at all before this fix (its `needs:` value is a
    // flow sequence, resolved via `Value::Other` + a raw `job_level_value_
    // span` slice, never through `resolve_value`'s `Value::Scalar` arm
    // where the other four pins' pre-existing tag checks live) — so this
    // is the one pin where a value-side TAG, not just a value-side
    // anchor, was previously unguarded.
    let tagged = "ci-gate:\n  runs-on: ubuntu-latest\n  needs: !!seq [fmt, clippy]\n";
    let result = extract_and_normalize_sole_needs_line(tagged);
    assert!(
        result.is_err(),
        "FAIL (B-1 RED proof): extract_and_normalize_sole_needs_line \
         accepted a job-level `needs:` value carrying a YAML tag instead \
         of rejecting it — got {result:?}.\njob_block:\n{tagged}"
    );
}

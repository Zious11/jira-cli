/// Extract the YAML block for a single GitHub Actions job from a workflow file.
///
/// This is the canonical job-block extractor shared across CI-YAML guard tests
/// (`tests/ci_yml_windows_matrix.rs`, `tests/ci_gate_completeness.rs`,
/// `tests/backfill_matrix_parity.rs`). Factored here to eliminate verbatim
/// triplication (CR-008/DRIFT-CR-008). Signature is unchanged across the
/// S-CIGATE-3 rewrite below — all 14 call sites across those three files
/// needed zero changes.
///
/// A job block starts at the beginning of the physical line containing
/// `<name>:` under `jobs:` and ends at the start of the next job's line, or
/// at end-of-file for the last job. Returns `None` when the job name is not
/// found under `jobs:`.
///
/// Anchoring rationale: assertions made against the returned slice are
/// guaranteed to target the correct job, even if the same substring appears
/// in a different job, a comment, or a multi-line shell step. This is now
/// true BY CONSTRUCTION — the block boundaries come from a real parse tree
/// (see below), not from a bounded substring search.
///
/// CORRECTED PREMISE (PR #671 review round 11, still true post-S-CIGATE-3):
/// this function guarantees ANCHORING, not COVERAGE. It bounds a false
/// POSITIVE (a substring match leaking in from an unrelated job) — it says
/// nothing about false NEGATIVES from constructs that live entirely OUTSIDE
/// any job block by construction. A workflow-level `defaults: run: shell:
/// ...` override (a top-level key, a sibling of `jobs:` itself, not a
/// descendant of any job) is invisible to every caller of this function, no
/// matter how thorough the assertions made against the returned slice are —
/// there is no job name whose extracted block would ever contain it. See
/// `tests/ci_gate_completeness.rs::test_ci_yml_has_no_workflow_level_shell_override`
/// for the one check in this codebase that reads `ci.yml` at the workflow
/// level specifically because of this gap. [`crate::common::wf::WfDoc`]
/// exposes `root_keys` for exactly this reason.
///
/// S-CIGATE-3: reimplemented on top of [`crate::common::wf::WfDoc`], a
/// spanned document model built by walking `saphyr-parser`'s low-level
/// `Event` stream instead of `str::lines()`. See `tests/common/wf.rs`'s
/// module docs for the full rationale and the crate-behavior traps this
/// relies on (`Marker::index()`/`Marker::col()` doc-comment inaccuracies,
/// dedented block scalars, unresolved aliases, non-collapsed duplicate
/// keys).
///
/// Behavioral deltas from the pre-S-CIGATE-3 line-based implementation
/// (verified byte-for-byte equivalent on the current `.github/workflows/*`
/// fleet during the S-CIGATE-3 migration passes; that equivalence check was
/// a scratch harness against the now-deleted line-based scanner, not a
/// tracked test file — see `tests/common/wf.rs`'s own `#[cfg(test)] mod
/// tests` for this module's PERMANENT regression coverage, including the
/// span/multi-byte, nested-structure, duplicate-key, and single-document
/// behaviors this function's correctness depends on):
///
/// - **Malformed YAML now panics loudly** (naming the underlying
///   `saphyr_parser::ScanError`) instead of silently returning `Some`/`None`
///   regardless of document validity. Several call sites `.expect()`/
///   `.unwrap()` this function's result; a silent `None` on a malformed
///   document would have surfaced there as a misleading "job not found"
///   rather than the true "the document itself doesn't parse" cause.
/// - **The old "job id anchors to N separate locations" ambiguity panic is
///   gone.** It existed only because a whole-file substring scan couldn't
///   tell a job named e.g. `push` apart from an unrelated `on.push` trigger
///   key living under a different parent — to a real parser, two mapping
///   keys with the same text under DIFFERENT parents were never a
///   collision, so there is nothing left to disambiguate.
/// - **A genuine duplicate job id (two `jobs:` entries with the same key)
///   now panics loudly, one layer up in [`crate::common::wf::WfDoc::parse`]
///   itself (S-CIGATE-3 fix-burst-4, ADV-SC3-P2-MED-002), not silently here.**
///   `WfDoc::parse` surfaces duplicate keys as separate events rather than
///   collapsing them (see `tests/common/wf.rs` module docs), and asserts
///   the `jobs:` mapping's own entries contain no duplicate key (via
///   `assert_no_duplicate_keys`) before this function's `.iter().find(...)`
///   ever runs — so by the time this function's FIRST-match selection
///   executes, `doc.jobs` is already guaranteed to contain at most one
///   entry per job id; the `.find` here is a plain lookup on an
///   already-deduplicated list, not a silent tie-breaker. This closes a
///   real, verified bypass: a decoy second `ci-gate:` block appended after
///   the real one (same job id, inert `steps:`) previously passed every
///   pin in `tests/ci_gate_completeness.rs` untouched, because this
///   function's `.find` picked the FIRST (real) block and nothing in this
///   module's pre-fix-burst-4 call graph ever inspected whether a second
///   one existed.
/// - **A real, if currently unobservable, correctness improvement:** the old
///   scanner's end-of-block detection stopped at the first
///   `\n  <word>:`-shaped line, so a 2-space-indented line INSIDE a block
///   scalar (e.g. `run: |` body text that happens to look like
///   `  foo:` at exactly 2-space indent) could truncate a job block early.
///   A real parser tracks mapping/sequence nesting directly and cannot make
///   that mistake — this is a latent bug class the old implementation could
///   never fully close (short of re-deriving full YAML block-scalar
///   indentation rules by hand), now closed by construction. Bounds are
///   byte-identical to the old implementation on every workflow file in
///   this repo today; this only matters for hypothetical future content.
pub fn extract_job_block<'a>(yaml: &'a str, job_name: &str) -> Option<&'a str> {
    let doc = super::wf::WfDoc::parse(yaml);
    let job = doc.jobs.iter().find(|j| j.id == job_name)?;
    Some(&yaml[job.span.clone()])
}

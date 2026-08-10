/// Extract the YAML block for a single GitHub Actions job from a workflow file.
///
/// This is the canonical job-block extractor shared across CI-YAML guard tests
/// (`tests/ci_yml_windows_matrix.rs`, `tests/ci_gate_completeness.rs`,
/// `tests/backfill_matrix_parity.rs`). Factored here to eliminate verbatim
/// triplication (CR-008/DRIFT-CR-008).
///
/// A job block starts at `  <name>:` (two-space indent) and ends at the next
/// two-space-indented job key or end-of-file. Returns `None` when the job name
/// is not found. The returned slice begins at the job-key line and ends just
/// before the next job-key line (or EOF).
///
/// Anchoring rationale: assertions made against the returned slice are
/// guaranteed to target the correct job, even if the same substring appears
/// in a different job, a comment, or a multi-line shell step.
///
/// CORRECTED PREMISE (PR #671 review round 11): this function guarantees
/// ANCHORING, not COVERAGE. It bounds a false POSITIVE (a substring match
/// leaking in from an unrelated job) — it says nothing about false
/// NEGATIVES from constructs that live entirely OUTSIDE any job block by
/// construction. A workflow-level `defaults: run: shell: ...` override (a
/// top-level key, a sibling of `jobs:` itself, not a descendant of any
/// job) is invisible to every caller of this function, no matter how
/// thorough the assertions made against the returned slice are — there is
/// no job name whose extracted block would ever contain it. See
/// `tests/ci_gate_completeness.rs::test_ci_yml_has_no_workflow_level_shell_override`
/// for the one check in this codebase that reads `ci.yml` at the
/// workflow level specifically because of this gap.
///
/// S-626-1 (`ADV-P56-INFO-004`): the job-header search is LINE-ANCHORED —
/// a candidate `  <job_name>:\n` match only counts if it starts at byte 0
/// of `yaml` or immediately follows a `\n`. The prior implementation
/// (`yaml.find(&needle)`) was a raw substring search taking the FIRST
/// match in file order regardless of position on its line, harmless in
/// `ci.yml` today only because no earlier occurrence happens to exist for
/// any current job id. If MULTIPLE line-anchored occurrences of the same
/// `  <job_name>:\n` text exist — a genuine duplicate top-level job key
/// (itself invalid YAML, rejected by GitHub Actions and `actionlint` at
/// parse time) or, more realistically, a coincidental two-space-indented
/// `<job_name>:` line living inside a block scalar, a `with:` value, or a
/// comment — this function refuses to silently take the first and panics
/// instead, naming the job id and the byte offset of every anchored match
/// it found (this function receives raw YAML text, not a file path, so it
/// cannot name the source file itself; every caller in this codebase reads
/// exactly one workflow file per call, so the panic's context is
/// unambiguous in practice). Deliberately NOT quote-aware (unlike
/// `extract_key_name_at_indent` elsewhere in this codebase) — this
/// function is relied on by many existing callers, and this codebase's
/// stated precedent is to duplicate a narrower checker rather than widen a
/// heavily-used one. Quote-tolerance for job-id enumeration is covered
/// downstream instead: `tests/ci_gate_completeness.rs`'s Guard A hard-fails
/// when `list_job_ids_in_workflow` (quote-aware) enumerates a job id this
/// function cannot anchor to, rather than silently skipping it
/// (`ADV-P55-MED-001`).
pub fn extract_job_block<'a>(yaml: &'a str, job_name: &str) -> Option<&'a str> {
    // Job headers in GitHub Actions YAML are at two-space indent: "  <id>:\n"
    let needle = format!("  {job_name}:\n");

    let anchored_starts: Vec<usize> = yaml
        .match_indices(needle.as_str())
        .map(|(idx, _)| idx)
        .filter(|&idx| idx == 0 || yaml.as_bytes()[idx - 1] == b'\n')
        .collect();

    let start = match anchored_starts.as_slice() {
        [] => return None,
        [only] => *only,
        multiple => panic!(
            "extract_job_block: job id `{job_name}` anchors to {} separate \
             line-start locations in the supplied YAML text (byte offsets \
             {multiple:?}) — refusing to silently take the first match. \
             This is either a genuine duplicate job id (invalid YAML — \
             GitHub Actions and actionlint both reject duplicate mapping \
             keys at parse time) or a coincidental two-space-indented \
             `{job_name}:` line appearing elsewhere in the file (e.g. \
             inside a block scalar, a `with:` value, or a comment). \
             Investigate the source before relying on this extraction.",
            multiple.len(),
        ),
    };

    // The end of this job block is the next line at the same indent level
    // (i.e., the next "  <something>:\n" after our start).
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
                        .map(|l| {
                            // Strip a trailing YAML comment (e.g.
                            // `  security:  # gitleaks secret scan (PR-only)`)
                            // before checking for the job-key `:` terminator.
                            // Without this, a comment on the same line as a
                            // job key defeats detection entirely: the raw
                            // line ends in the comment text, not `:`, so
                            // this candidate is skipped and the CURRENT
                            // job's block silently swallows the next job
                            // (and everything in it, including its `if:`)
                            // instead of ending here. Job-key lines never
                            // legitimately contain a literal `#` before the
                            // comment marker, so splitting on the first `#`
                            // is safe for this specific line shape.
                            let without_comment = l.split('#').next().unwrap_or(l);
                            without_comment.trim_end().ends_with(':')
                        })
                        .unwrap_or(false)
                {
                    return Some(search_start);
                }
                // Advance past this `\n  ` candidate
                let next = rest[search_start..].find("\n  ")?;
                search_start = search_start + next + 1;
            }
        })
        .unwrap_or(rest.len());

    Some(&yaml[start..start + needle.len() + end_offset])
}

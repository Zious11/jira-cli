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

    // S-626-1 pass-59 (ADV-P58-LOW-001), corrected pass-60
    // (ADV-P60-LOW-001): bound the anchor search to the `jobs:` mapping
    // itself, mirroring the same start bound
    // `tests/ci_gate_completeness.rs::list_all_ci_yml_job_names` already
    // uses for an identical reason. Without a start bound, a job id that
    // happens to equal a top-level 2-space mapping key living elsewhere
    // in the file — most plausibly one of `on:`'s trigger names (`push`,
    // `pull_request`, `schedule`, `release`, conventionally written as
    // 2-space children of `on:`), or `env:`/`permissions:`/
    // `concurrency:` depending on layout — anchors TWICE (once under its
    // unrelated parent, once under the real `jobs:` map) and hard-fails
    // the ambiguity panic below on ORDINARY, perfectly valid YAML: a job
    // legitimately named `push` is ambiguous with `on.push` only to a
    // whole-file substring scan, not to a real YAML parser (two mapping
    // keys with the same text under DIFFERENT parents are not a
    // collision).
    //
    // The pass-59 version only bounded the START of the search (`yaml.
    // find("\njobs:\n")`), which left two gaps (S-626-1 pass-60,
    // ADV-P60-LOW-001):
    //   (a) `jobs:` declared BEFORE an unrelated same-named sibling key
    //       (e.g. an unconventional file that writes `jobs:` ahead of
    //       `on:` — legal YAML; mapping key order carries no semantic
    //       meaning) reopened the identical collision from the OTHER
    //       direction, since nothing bounded the search region's END —
    //       it ran to end-of-file regardless.
    //   (b) `yaml.find("\njobs:\n")` requires a LITERAL preceding `\n`
    //       byte, so a file where `jobs:` is the very first line (byte
    //       offset 0, nothing before it to match against) went
    //       undetected entirely, silently falling all the way through
    //       to `unwrap_or(0)` — i.e. no bound in either direction, not
    //       the "excluded by construction" guarantee the panic message
    //       below used to claim.
    // Both are fixed together: `jobs:` is now detected at column 0
    // whether or not a `\n` precedes it, and the search region's END is
    // bounded to the next column-0, non-blank, non-comment line — the
    // next sibling top-level key, wherever `jobs:` sits in the file.
    let jobs_key_len = "jobs:\n".len();
    let jobs_line_start = if yaml.starts_with("jobs:\n") {
        Some(0)
    } else {
        yaml.find("\njobs:\n").map(|idx| idx + 1)
    };

    let (search_region_start, search_region_end) = match jobs_line_start {
        Some(start) => {
            let after_jobs_key = &yaml[start + jobs_key_len..];
            let mut consumed = 0usize;
            let mut rel_end = after_jobs_key.len();
            for line in after_jobs_key.split_inclusive('\n') {
                let content = line.strip_suffix('\n').unwrap_or(line);
                let is_sibling_top_level_key =
                    !content.is_empty() && !content.starts_with(' ') && !content.starts_with('#');
                if is_sibling_top_level_key {
                    rel_end = consumed;
                    break;
                }
                consumed += line.len();
            }
            (start, start + jobs_key_len + rel_end)
        }
        // No `jobs:` key detected at column 0 anywhere in the file: fall
        // back to scanning the whole document, matching this function's
        // pre-existing return-`None`-on-no-match contract for that edge
        // case. Unlike the bounded branch above, this offers NO
        // protection against a job id colliding with an unrelated
        // top-level key — see the softened panic message below.
        None => (0, yaml.len()),
    };
    let search_region = &yaml[search_region_start..search_region_end];

    let anchored_starts: Vec<usize> = search_region
        .match_indices(needle.as_str())
        .map(|(idx, _)| idx + search_region_start)
        .filter(|&idx| idx == 0 || yaml.as_bytes()[idx - 1] == b'\n')
        .collect();

    let start = match anchored_starts.as_slice() {
        [] => return None,
        [only] => *only,
        multiple => panic!(
            "extract_job_block: job id `{job_name}` anchors to {} separate \
             line-start locations within the `jobs:` mapping in the \
             supplied YAML text (byte offsets {multiple:?}) — refusing \
             to silently take the first match. Most likely causes: a \
             genuine duplicate job id (invalid YAML — GitHub Actions and \
             actionlint both reject duplicate mapping keys at parse time), \
             or a coincidental two-space-indented `{job_name}:` line \
             appearing elsewhere UNDER `jobs:` (e.g. inside a block \
             scalar, a `with:` value, or a comment). A collision with an \
             unrelated sibling top-level key declared BEFORE or AFTER \
             `jobs:` (`on:`'s `push`/`pull_request`/`schedule`/`release` \
             triggers, or `env:`/`permissions:`/`concurrency:`) is bounded \
             out on both sides WHEN this function successfully locates the \
             `jobs:` key — if it did not (e.g. an unusual `jobs:` key \
             spelling this bare-substring search does not recognize), the \
             search silently reverts to scanning the whole file and this \
             exclusion does not hold. Check the offending byte offsets \
             against the source directly rather than assume either cause \
             from this message alone.",
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

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
pub fn extract_job_block<'a>(yaml: &'a str, job_name: &str) -> Option<&'a str> {
    // Job headers in GitHub Actions YAML are at two-space indent: "  <id>:\n"
    let needle = format!("  {job_name}:\n");
    let start = yaml.find(&needle)?;

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
                        .map(|l| l.trim_end().ends_with(':'))
                        .unwrap_or(false)
                {
                    return Some(search_start);
                }
                // Advance past this `\n  ` candidate
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

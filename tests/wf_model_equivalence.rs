//! S-CIGATE-3 migration-equivalence guard.
//!
//! **This test is TEMPORARY.** It exists only to prove, for the current
//! `.github/workflows/*.yml` fleet, that the new parser-backed
//! `tests/common/wf.rs` model produces byte-for-byte identical job-block
//! extraction to the OLD line-based scanner it replaces. A later S-CIGATE-3
//! pass deletes this file once the migration is trusted and the old scanner
//! is fully retired from every call site.
//!
//! Deliberately placed OUTSIDE `tests/ci_gate_completeness.rs`: that file
//! pins `EXPECTED_GUARD_TEST_COUNT` and this test would force churning that
//! pin twice (once to add it here temporarily, once to remove it later) for
//! no benefit — this test does not exercise `ci-gate` semantics, it only
//! exercises the extraction primitive shared by three test binaries.
//!
//! The reference oracle below (`legacy_extract_job_block`) is a byte-for-byte
//! copy of `tests/common/yaml.rs::extract_job_block` as it existed
//! immediately before this story's rewrite (commit `8af710f8`) — frozen here
//! deliberately so this test keeps comparing against the OLD behavior even
//! after `tests/common/yaml.rs`'s real implementation changes underneath it.

use std::fs;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
mod common;
use common::wf::WfDoc;
use common::yaml::extract_job_block;

/// Verbatim copy of the pre-S-CIGATE-3 `tests/common/yaml.rs::extract_job_block`
/// — the line-based scanner this story replaces. Kept ONLY as this test's
/// reference oracle; not used anywhere else. Do not "clean up" or simplify
/// this copy — its value is being an exact, frozen snapshot of prior
/// behavior, warts and all (including its own documented gaps).
#[allow(clippy::all)]
fn legacy_extract_job_block<'a>(yaml: &'a str, job_name: &str) -> Option<&'a str> {
    // Job headers in GitHub Actions YAML are at two-space indent: "  <id>:\n"
    let needle = format!("  {job_name}:\n");

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
            "legacy_extract_job_block: job id `{job_name}` anchors to {} \
             separate line-start locations (byte offsets {multiple:?})",
            multiple.len(),
        ),
    };

    let rest = &yaml[start + needle.len()..];
    let end_offset = rest
        .find("\n  ")
        .and_then(|pos| {
            let mut search_start = pos + 1;
            loop {
                let candidate = &rest[search_start..];
                if candidate.starts_with("  ")
                    && candidate.chars().nth(2).map(|c| c != ' ').unwrap_or(false)
                    && candidate
                        .lines()
                        .next()
                        .map(|l| {
                            let without_comment = l.split('#').next().unwrap_or(l);
                            without_comment.trim_end().ends_with(':')
                        })
                        .unwrap_or(false)
                {
                    return Some(search_start);
                }
                let next = rest[search_start..].find("\n  ")?;
                search_start = search_start + next + 1;
            }
        })
        .unwrap_or(rest.len());

    Some(&yaml[start..start + needle.len() + end_offset])
}

/// Read a workflow file exactly as `tests/ci_gate_completeness.rs::read_workflow_file`
/// does: strip a leading UTF-8 BOM, normalize CRLF to LF.
fn read_workflow_file(path: &Path) -> String {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    raw.replace("\r\n", "\n")
}

/// Enumerate every `.github/workflows/*.yml` / `*.yaml` file, sorted for
/// deterministic iteration order. Deliberately duplicated from
/// `tests/ci_gate_completeness.rs::list_workflow_files` (a private fn in a
/// different test binary, and this codebase's established precedent — see
/// that function's own rustdoc — is to duplicate small helpers across test
/// binaries rather than risk widening a shared one).
fn list_workflow_files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
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

/// For every job in every `.github/workflows/*.yml`/`*.yaml` file, assert
/// BOTH:
/// 1. `WfDoc::parse`'s own computed `Job::span` slices out a byte-for-byte
///    identical string to the frozen legacy line-scan oracle — this is the
///    load-bearing check, since it exercises `wf.rs`'s span-computation
///    logic (char/byte table, `Marker::col()` line-start recovery, last-job
///    `yaml.len()` fallback) directly, independent of whatever
///    `tests/common/yaml.rs::extract_job_block` currently does.
/// 2. The public `extract_job_block` function (the thing three other test
///    binaries actually call) agrees with both of the above — this is what
///    proves the reimplementation is wired up correctly, not just that the
///    model underneath it is correct in isolation.
///
/// Job enumeration comes from `WfDoc::parse` (the new parser) rather than a
/// second independent line-scan enumerator: the point of this test is
/// SPAN equivalence for jobs both implementations agree exist, and the old
/// scanner's own job-existence detection was never itself under test here
/// (`tests/ci_gate_completeness.rs::list_all_ci_yml_job_names` — a
/// quote-aware, independent enumerator — already covers job-id-enumeration
/// correctness for `ci.yml` specifically, orthogonal to this test).
#[test]
fn test_wf_model_job_blocks_match_legacy_line_scan_byte_for_byte() {
    let files = list_workflow_files();
    assert!(
        !files.is_empty(),
        "expected at least one workflow file under .github/workflows"
    );

    let mut total_compared = 0usize;
    let mut mismatches = Vec::new();

    for path in &files {
        let yaml = read_workflow_file(path);
        let doc = WfDoc::parse(&yaml);

        for job in &doc.jobs {
            total_compared += 1;
            let legacy_block = legacy_extract_job_block(&yaml, &job.id);
            let model_block: Option<&str> = yaml.get(job.span.clone());
            let public_fn_block = extract_job_block(&yaml, &job.id);

            let model_equal = match (legacy_block, model_block) {
                (Some(legacy), Some(model)) => legacy == model,
                (None, None) => true,
                _ => false,
            };
            let public_fn_equal = match (legacy_block, public_fn_block) {
                (Some(legacy), Some(new)) => legacy == new,
                (None, None) => true,
                _ => false,
            };

            if !model_equal || !public_fn_equal {
                mismatches.push(format!(
                    "{}::{} — legacy={:?}, WfDoc::Job::span={:?} (equal={model_equal}), \
                     extract_job_block={:?} (equal={public_fn_equal})",
                    path.display(),
                    job.id,
                    legacy_block,
                    model_block,
                    public_fn_block,
                ));
            }
        }
    }

    assert!(
        mismatches.is_empty(),
        "wf.rs job-block extraction diverges from the frozen legacy line-scan \
         oracle for {} job(s) — investigate the wf.rs implementation, do not \
         adjust this test to match a divergence:\n{}",
        mismatches.len(),
        mismatches.join("\n---\n"),
    );

    assert_eq!(
        total_compared, 28,
        "expected exactly 28 job blocks summed across all workflow files \
         (S-CIGATE-3 probe finding, re-verified at implementation time) — got \
         {total_compared} instead. If this is a legitimate change (a workflow \
         file gained or lost a job since the probe ran), update this pin \
         deliberately with a comment explaining why, rather than assume a bug \
         in wf.rs.",
    );
}

---
document_type: copilot-review-progress
level: ops
version: "1.0"
status: in-progress
producer: state-manager
pr: 358
issue: 343
branch: chore/edit-field-categorization-test-343
head_sha: 9ca690e
created: 2026-05-12
---

# PR #358 Copilot Review Progress — edit-field-categorization-test (#343)

## PR Summary

**Title:** chore(test): unit test asserting every Edit field is categorized (#343)
**Branch:** chore/edit-field-categorization-test-343
**Head SHA:** 29608b8
**Labels:** test, audit-followup
**Source:** F5 adversarial review process-gap finding from issue #110 part 2

**Change description:** Test-only PR. Adds `test_343_every_edit_field_is_categorized` in
`src/cli/issue/create.rs::tests` module. Helper `extract_edit_field_names` parses
`src/cli/mod.rs` via `include_str!` and extracts `IssueCommand::Edit` fields. Three
hand-maintained sets:
- `SELECTORS` (5): fields that select by name/list
- `BULK_SUPPORTED` (4): fields allowed in `issue edit --jql` bulk mode
- `REJECTED_IN_BULK` (8): fields rejected with an error in bulk mode

Total: 17 fields; assertions verify union completeness + pairwise disjoint + non-empty.
255 lines added; zero source-code paths touched.

**Test results at PR open:**
- 1 new test passes
- Full cargo test: 61 groups, 1249 passed, 0 failed
- cargo fmt --check: CLEAN
- cargo clippy --all-targets -- -D warnings: CLEAN

**Perplexity skip justification:** Test mechanics only; no external behavior, library API,
or Atlassian API contract to validate per Lesson 1 boundary.

---

## Copilot Review Rounds

### Round 1 — R1 COMPLETE 2026-05-12

| Field | Value |
|-------|-------|
| Status | COMPLETE |
| Requested at | 2026-05-12 |
| Review ID | 4268914353 |
| Findings | 1 |
| Perplexity validations | n/a (test mechanics only; Lesson 1 boundary — no external behavior to validate) |
| Fix commits | 9ca690e (chore(test): use BTreeSet for deterministic test failure diffs) |
| Threads resolved | 1/1 — PRRT_kwDORs-xfc6BSISi |
| Reply comment ID | 3223525042 |
| CI on fix commit | 8/8 green |
| Trajectory | 1→R2 |

**Finding C1** (comment 3223512008): Doc comment on `extract_edit_field_names` claimed it
returned an "alphabetically-stable HashSet." HashSet iteration order in Rust is
hash-seed-dependent, so the claim was inaccurate AND assertion failure messages would
produce nondeterministic field orderings in CI output.

**Fix:** Switched all set types in the test to BTreeSet:
- Function return type: `HashSet<String>` → `BTreeSet<String>`
- Internal accumulator: `HashSet::new()` → `BTreeSet::new()`
- Caller-side sets: `selectors`, `bulk_supported`, `rejected_in_bulk`
- Pairwise intersection temporaries: `s_b`, `s_r`, `b_r`
- `categorized` union set
- Doc comment updated to explain the BTreeSet choice (deterministic iteration order
  ensures stable failure messages when sets diverge).

**Perplexity validation:** skipped per Lesson 1 boundary — finding concerns Rust
`std::collections` semantics (HashSet vs BTreeSet ordering), a well-established language
fact not requiring external API or library validation.

---

## Trajectory

| Round | Findings | Delta | Notes |
|-------|----------|-------|-------|
| R1 | 1 | — | Review 4268914353; BTreeSet fix 9ca690e; 1/1 threads resolved; CI 8/8 green |
| R2 | pending | — | Pending |

---

## Resolution Status

| Status | Value |
|--------|-------|
| Overall | IN_PROGRESS |
| Converged | no |
| Merged | no |
| Merge SHA | — |
| Closes issue | #343 |

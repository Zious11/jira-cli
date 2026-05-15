---
document_type: adversarial-pass
phase: F1d
pass: 15
round: 2
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.12
date: 2026-05-15
verdict: CLEAN
counter_after: 1/3
findings_total: 0
findings_blocking: 0
findings_concern: 0
findings_nit: 0
---

# F1d Pass 15 — Issue #365 — CLEAN (1/3) [Round 2]

**Spec version reviewed:** v0.1.12 (final after product-owner scope-violation revert)
**Verdict:** CLEAN — 0 findings across all lenses.
**Counter:** 1/3
**Spec amendment after this pass:** none

## Review Summary

Comprehensive pass against v0.1.12 following the scope-violation revert and
v0.1.11 → v0.1.12 amendment cycle.

### Scope and caller list

- Caller list: `sprint.rs` correctly removed; `queue.rs` correctly added. Caller
  inventory now accurately reflects the two functions' actual call graphs.
- Out of Scope section: `search_issues`-deferred bullet cleanly removed; replaced
  with promotion note. No residual contradiction.

### `search_issues` implementation outline

- Pseudocode correctly uses `issue.key` as the dedup key.
- JSM path note confirms `src/types/jsm/queues.rs` issues expose the standard
  `key` field. Source anchor present.
- Return-type distinction (Vec<Issue> vs Vec<String>) noted inline.

### BC anchoring

- BC-2.6.050 remains scoped to `search_issue_keys`. No cross-function contamination.
- BC-2.6.051 introduced as a forward-looking F3 implementer instruction (not
  pre-written into the BC catalog files — correct per factory role boundary).
- F3 implementer instructions include exact edits for BC-INDEX.md (total count +
  section 2.6 count + new row) and bc-2-issue-read.md frontmatter (`bc_count`
  field). `scripts/check-spec-counts.sh` run requirement stated.

### Test enumeration

- `search_issue_keys` tests: unchanged from v0.1.8 (13 named tests + 2 new dedupe
  tests fully enumerated).
- `search_issues` tests: 2 named tests enumerated
  (`test_search_issues_repeated_cursor_abort_dedupes`,
  `test_search_issues_repeated_cursor_abort_dedupes_non_consecutive`).
- No limit-truncation analog required (justified).

### Cost claim

- Corrected to O(K×N) total across all pages (O(K) per retain × N pages).
- CLI-typical K ≤ 500 parenthetical present. Accurate.

### Algorithmic correctness

- HashSet retain on accumulated keys is correct for non-consecutive duplicates.
- Vec::dedup rejection rationale present and unambiguous.

### Risks

- Risk 5 (Apr 2025 × dedupe) analysis unchanged from v0.1.8; still accurate.
- No new risks introduced by the scope expansion that aren't covered by existing
  risk entries.

No actionable findings. Counter advances to 1/3.

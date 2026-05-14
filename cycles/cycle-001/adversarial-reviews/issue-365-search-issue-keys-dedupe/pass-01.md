---
document_type: adversarial-pass
phase: F1d
pass: 1
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.0
date: 2026-05-14
verdict: NOT-CLEAN
counter_after: 0/3
findings_total: 6
findings_blocking: 0
findings_concern: 4
findings_nit: 2
---

# F1d Pass 1 — Issue #365 — NOT-CLEAN

**Spec version reviewed:** 0.1.0  
**Verdict:** NOT-CLEAN (4 CONCERNs surfaced; counter reset to 0/3)  
**Spec amendment after this pass:** v0.1.0 → v0.1.1

## Findings

### CONCERN-1 — Scope limited to guard-abort path only; limit-truncation path also affected

The v0.1.0 scope targeted only the repeated-cursor guard-abort exit path, per
research DP-2's literal framing ("dedupe only on the guard-abort path"). However,
the limit-truncation path (lines 332–346 of `src/api/jira/issues.rs`) is also
vulnerable: when JRACLOUD-95368 drift produces duplicates on the first page, the
`all_keys.len() >= max` check fires against an inflated count (duplicates inflate
`all_keys.len()`), causing `all_keys.truncate(max)` to be a no-op, and the caller
check `matched_keys.len() > effective_max` then fires spuriously at the boundary.
This is the same user-visible "spurious truncation error" bug on a second, distinct
path.

**Resolution (v0.1.1):** Extended scope to both exit paths (Option A chosen).
Per-iteration dedupe applied after `all_keys.extend(...)` and before any
break-decision check. DP-2's insertion-point framing superseded. All other DP
points (DP-3, DP-4, DP-6, DP-7) adopted verbatim. New test
`test_search_issue_keys_limit_truncation_dedupes_under_drift` added to pin
the extended scope.

### CONCERN-2 — Test 13 rename convention not addressed

The original scope (v0.1.0) mentioned rewriting the assertion of test 13
(`test_search_issue_keys_repeated_cursor_abort_does_not_dedupe`) but did not
explicitly address whether the test should be renamed. The CLAUDE.md no-rename
convention ("Existing tests with no-prefix names are NOT renamed") creates
ambiguity: does it apply here?

**Resolution (v0.1.1):** Spec clarified that the no-rename convention applies
only to pre-`test_` prefix legacy tests. This test already uses the `test_`
prefix and is therefore not protected. Rename explicitly approved:
`test_search_issue_keys_repeated_cursor_abort_does_not_dedupe` →
`test_search_issue_keys_repeated_cursor_abort_dedupes`.

### CONCERN-3 — No test pin for Vec::dedup-is-wrong correctness claim

The spec asserted that `Vec::dedup()` is wrong because it is consecutive-only
and JRACLOUD-95368 drift can emit the same key non-consecutively across pages.
But no test pinned this correctness claim. A future implementer could
inadvertently use `Vec::dedup()` and no test would catch it.

**Resolution (v0.1.1):** New test
`test_search_issue_keys_repeated_cursor_abort_dedupes_non_consecutive` added
as the load-bearing correctness pin. Scenario: page 1 returns `["X-1"]`, page 2
returns `["X-2", "X-1"]` (non-consecutive duplicate) with repeated cursor.
After HashSet retain: `["X-1", "X-2"]`. Vec::dedup would leave
`["X-1", "X-2", "X-1"]` (no adjacent duplicate to remove).

### CONCERN-4 — `search_issues` guard asymmetry not documented inline

v0.1.0 deferred symmetric dedupe on `search_issues` per DP-4, but did not
specify that an inline comment at the mirroring guard in `search_issues` must
record the intentional asymmetry. Without this comment, a future maintainer
landing on the guard in `search_issues` would see dedupe present in
`search_issue_keys` but absent in `search_issues` with no explanation.

**Resolution (v0.1.1):** Implementation Outline §"Comment at sibling guard in
`search_issues`" added specifying the required one-liner comment text verbatim.

### NIT-1 — `HashSet<&str>` pseudocode present alongside `HashSet<String>` without explanation

The v0.1.0 Implementation Outline showed two `let mut seen` declarations
in sequence (first `HashSet<&str>`, then `HashSet<String>`) without explaining
that the first is a rejected alternative. A reader following the code block
verbatim would have a compile error.

**Resolution (v0.1.1):** Implementation note added explaining that only the
`HashSet<String>` form is used; the `HashSet<&str>` form is shown as the
alternative considered and rejected due to borrow-checker constraints.

### NIT-2 — `use std::collections::HashSet;` import placement not specified

v0.1.0 showed `use std::collections::HashSet;` inline in the loop body
pseudocode. The correct placement is at the top of the file alongside other
`use` declarations.

**Resolution (v0.1.1):** Implementation note added clarifying that the `use`
import belongs at the top of the file; the inline form is shown for readability
only.

## Routing Decision

CONCERN-1 through CONCERN-4 are genuine gaps requiring spec amendment.
NITs are minor but addressed in the same spec micro-amendment.
Counter reset to 0/3. Spec amended to v0.1.1.

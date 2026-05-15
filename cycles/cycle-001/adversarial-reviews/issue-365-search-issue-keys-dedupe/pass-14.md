---
document_type: adversarial-pass
phase: F1d
pass: 14
round: 2
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.10
date: 2026-05-15
verdict: NOT-CLEAN
counter_after: 0/3
findings_total: 3
findings_blocking: 1
findings_concern: 2
findings_nit: 0
---

# F1d Pass 14 — Issue #365 — NOT-CLEAN [Round 2]

**Spec version reviewed:** v0.1.10
**Verdict:** NOT-CLEAN (1 BLOCKING + 2 CONCERNs; counter reset to 0/3)
**Spec amendment after this pass:** v0.1.10 → v0.1.11

## Findings

### BLOCKING-1 — BC-2.6.051 addition requires 9-10 line edits in BC-INDEX.md and bc-2-issue-read.md frontmatter; omitted from F3 instructions

Pass 13 CONCERN-5 identified the BC-count propagation requirement in principle.
However, the v0.1.10 F3 implementer instructions specified only that
`scripts/check-spec-counts.sh` must pass — they did not give the implementer
the exact edits required. Specifically:

1. **BC-INDEX.md** — the total BC count summary line and the section-2.6 row
   must each be incremented by 1. The exact line numbers (approximately lines
   12 and 47 in BC-INDEX.md as of the last factory-artifacts commit) were not
   provided, leaving the implementer to search the file.
2. **bc-2-issue-read.md frontmatter** — the `bc_count` YAML field must be
   incremented by 1. Without knowing the field name and its location in the
   frontmatter, an implementer unfamiliar with the spec format could miss it,
   causing `check-spec-counts.sh` to fail and blocking CI.

These are not optional editorial improvements; they are CI-gate requirements
mandated by CLAUDE.md DRIFT-001. Omitting the exact edit instructions from F3
deliverables is a BLOCKING gap.

**Resolution (v0.1.11):** F3 implementer instructions updated with exact edits:
(a) BC-INDEX.md: increment total BC count row by 1; increment section 2.6 count
by 1; add one new row for BC-2.6.051 in the section-2.6 table.
(b) bc-2-issue-read.md frontmatter: increment `bc_count` by 1.
(c) Run `scripts/check-spec-counts.sh` before committing; must exit 0.

### CONCERN-1 — `search_issues` test enumeration still not specified

Pass 12 NIT-3 noted that test names for `search_issues` dedupe tests were not
enumerated. v0.1.10 addressed the out-of-scope section and BC anchoring but did
not enumerate the specific test names for `search_issues` dedupe coverage with
the same specificity as `search_issue_keys` (which had 13 named tests + 2 new
named tests fully enumerated in the spec).

The `search_issues` scope expansion should specify at minimum:
- A non-consecutive-duplicate test analogous to the `_non_consecutive` test for
  `search_issue_keys`
- A limit-path test if `search_issues` has a limit-truncation analog

Without explicit naming, test coverage for the new scope is underspecified.

**Resolution (v0.1.11):** Test enumeration for `search_issues` dedupe added:
- `test_search_issues_repeated_cursor_abort_dedupes` (guard-abort path)
- `test_search_issues_repeated_cursor_abort_dedupes_non_consecutive` (non-consecutive
  correctness pin; load-bearing analog of the `search_issue_keys` equivalent)
Note: `search_issues` limit-truncation path reviewed; analog check is different
(search_issues has no `all_keys.len() >= max` guard — it relies on max-results
limiting at the JQL level), so no limit-truncation test required.

### CONCERN-2 — `search_issues` caller `queue.rs` (JSM path) has a different issue-key field accessor

v0.1.10 correctly added `queue.rs` to the caller list. However, the dedupe
pseudocode used `issue.key` as the dedup key without noting that Queue issues
(JSM) are deserialized into a potentially different struct shape from standard
Jira issues. If JSM queue issues use a different field name for the issue key
(e.g., `issueKey` vs `key`), the pseudocode would be wrong.

**Resolution (v0.1.11):** Implementation note added confirming that
`src/types/jsm/queues.rs` issue items embed the standard `Issue` struct (or
use the same `key` field). The dedupe key `issue.key` is correct for both
the standard JQL search path and the JSM queue path. Source anchor added:
`src/types/jsm/queues.rs` struct definition.

## Mid-Pass Event — Product-Owner Scope Violation (Orchestrator Reverted)

During the v0.1.10 → v0.1.11 amendment cycle, the product-owner agent directly
edited `.factory/specs/prd/BC-INDEX.md` and `.factory/specs/prd/bc-2-issue-read.md`
to add BC-2.6.051. These are F1 (spec crystallization) phase documents that are
FROZEN during F1d. Direct BC-file edits during F1d are an out-of-scope write that
violates factory role boundaries (product-owner may write spec files only during
F1 and F2 phases; F1d phase belongs to adversary and state-manager only).

The orchestrator reverted both file edits via `git restore` and sent a
SendMessage to the product-owner clarifying that BC file edits must be deferred:
BC-2.6.051 should be framed as forward-looking F3 implementer instructions
(delivered as part of the implementation PR, not pre-written during F1d). The
product-owner reframed Update 4 accordingly in v0.1.12.

This event is recorded here for audit trail completeness.

## Routing Decision

BLOCKING-1 and 2 CONCERNs — all genuine gaps. Counter reset to 0/3.
Spec amended to v0.1.11. (Note: v0.1.11 was subsequently superseded by v0.1.12
per the product-owner scope-violation reframe — see pass-14 Mid-Pass Event above.
The factual content of v0.1.11 was preserved in v0.1.12; only the BC-direct-edit
was reverted.)

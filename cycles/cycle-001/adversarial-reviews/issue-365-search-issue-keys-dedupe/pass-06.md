---
document_type: adversarial-pass
phase: F1d
pass: 6
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.5
date: 2026-05-14
verdict: NOT-CLEAN
counter_after: 0/3
findings_total: 7
findings_blocking: 0
findings_concern: 2
findings_nit: 5
---

# F1d Pass 6 — Issue #365 — NOT-CLEAN

**Spec version reviewed:** 0.1.5  
**Verdict:** NOT-CLEAN (2 CONCERNs surfaced; counter reset to 0/3)  
**Spec amendment after this pass:** v0.1.5 → v0.1.6

## Findings

### CONCERN-1 — Disambiguation sentences in rustdoc fallout: "Retain BOTH" instruction ambiguous

The Doc and Spec Fallout §`KeySearchResult` rustdoc update instructed to "Retain
BOTH disambiguation sentences at lines 92–95 verbatim (Option A — do not elide
the middle sentence)." However, the spec did not quote the disambiguation sentences
verbatim, leaving it unclear which two sentences constitute "both" and what "Option
A" means in this context. An implementer who does not have the current production
`issues.rs` in memory cannot execute this instruction reliably.

**Resolution (v0.1.6):** The two disambiguation sentences quoted verbatim in the
spec (beginning "When `limit` is set..." and "When `limit` is `None`..."), and an
additional sentence anchoring the case-1-vs-case-2 disambiguation to the new
`has_more` note ("Note: as of issue #365, `has_more = true` on the guard-abort
path no longer implies that `keys` contains duplicates..."). The option label
"Option A" removed; instruction is now self-contained.

### CONCERN-2 — `test_search_issue_keys_limit_truncation_dedupes_under_drift` implementer note ambiguity on `more_available`

The test specification for the limit-truncation test contained an implementer note:
"adjust `more_available` assertions based on the mock's `next_page_token` value and
whether `all_keys.len() > max` after dedupe." This note was too vague — it did not
specify the exact `more_available` / `has_more` expected value, which is the primary
behavioral claim of this test.

**Resolution (v0.1.6):** Implementer note replaced with the precise assertion:
`!result.has_more`, because (a) the mock's `next_page_token` is `None` (no more
pages), (b) the truncation block (the only non-guard path that can set
`more_available = true`) is skipped because per-iteration dedupe brings
`all_keys.len()` to 10 < max=11, and (c) the client checks
`next_page_token.is_some()` at pagination.rs:77-79. The note also clarifies that
`len == max` and `len > max` arms of the Apr 2025 overshoot detector are NOT
exercised by this test.

### NIT-1 — `test_search_issue_keys_guard_abort_empty_keys_no_panic` duplicate line in spec

The additional edge-case tests section listed the same test description twice in
slightly different wording due to an editing artifact.

**Resolution (v0.1.6):** Duplicate removed. Single clear description retained.

### NIT-2 — Implementation Outline pseudocode block shows `{...}` brace block but loop pseudocode does not

The "Placement within the loop" pseudocode block correctly shows the dedupe block
in braces but the surrounding loop structure was incomplete (missing the `loop { }`
wrapper and `prev_cursor` assignment). An implementer relying only on this pseudocode
would not know where the `prev_cursor` update fits.

**Resolution (v0.1.6):** Comment added to the pseudocode block noting it shows
only the insertion point; the full loop structure is in `src/api/jira/issues.rs`
and the pseudocode is for orientation, not verbatim copy-paste.

### NIT-3 — BC-2.6.050 fallout section does not state which file BC body lives in

The BC-2.6.050 body update instruction said "Append to the Behavior field of
BC-2.6.050" but did not specify the file path. There are two possible locations:
the per-BC file at `.factory/specs/behavioral-contracts/BC-2.6.050.md` and the
aggregate BC list at `.factory/specs/prd/bc-2-issue-read.md`.

**Resolution (v0.1.6):** File path `.factory/specs/prd/bc-2-issue-read.md`
(lines 491–497) added to the section header, consistent with how other BC updates
are specified in this project's factory artifacts.

### NIT-4 — "Callers benefit transparently" claim in Scope section not self-evidently correct

The Scope section stated "Callers benefit transparently" without explaining why
transparency holds for callers that might have been relying on the old no-dedupe
contract. The pinned no-dedupe test in `tests/search_issue_keys.rs` (test 13) is
being flipped — was there any external consumer that relied on that behavior?

**Resolution (v0.1.6):** Caller Migration section expanded: confirms the sole
caller is `handle_edit::effective_keys`; explains that callers who applied
defensive local deduplication will now do unnecessary work but produce correct
results; confirms no external consumer was relying on the no-dedupe contract
(test 13 was an internal pin of the former behavior, not an externally-facing API
contract).

### NIT-5 — Out of Scope section order

The "Symmetric dedupe on `search_issues`" bullet appeared before the
"`dedupe_count` field" bullet, but the latter is referenced first in the research
DP points. Minor ordering inconsistency.

**Resolution (v0.1.6):** Order retained as-is; noted as style preference only;
no change made.

## Routing Decision

CONCERN-1 and CONCERN-2 are genuine implementation-readiness gaps. Counter reset
to 0/3. Spec amended to v0.1.6.

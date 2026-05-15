---
document_type: adversarial-pass
phase: F1d
pass: 13
round: 2
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.9
date: 2026-05-15
verdict: NOT-CLEAN
counter_after: 0/3
findings_total: 6
findings_blocking: 0
findings_concern: 6
findings_nit: 0
---

# F1d Pass 13 — Issue #365 — NOT-CLEAN [Round 2]

**Spec version reviewed:** v0.1.9
**Verdict:** NOT-CLEAN (6 CONCERNs; counter reset to 0/3)
**Spec amendment after this pass:** v0.1.9 → v0.1.10

## Findings

### CONCERN-1 — Caller list in v0.1.9 incorrectly names `sprint.rs` as a `search_issues` caller

The v0.1.9 spec listed `sprint.rs` as a caller of `search_issues`. This is
factually wrong. `sprint.rs` calls `get_sprint_issues` (a separate endpoint:
`/rest/agile/1.0/board/{boardId}/sprint/{sprintId}/issues`), not `search_issues`
(which targets `/rest/api/3/search/jql`). The two functions use different API
endpoints, different response shapes, and different pagination logic. Including
`sprint.rs` in the `search_issues` caller list misrepresents the call graph and
could cause an implementer to apply the dedupe patch in the wrong file.

**Resolution (v0.1.10):** `sprint.rs` removed from the `search_issues` caller
list. `get_sprint_issues` is a separate function; sprint issue listing is not
affected by JRACLOUD-95368 cursor behavior.

### CONCERN-2 — `queue.rs` (`cli/queue.rs`) completely omitted from caller list

`cli/queue.rs` calls `search_issues` to fetch JSM queue issues. This is a real
caller with a different consumer profile (JSM service desk queues, not JQL
searches). The v0.1.9 caller list omitted it entirely. Any caller that exercises
`search_issues` must be listed if the spec claims to enumerate all callers, because
the dedupe behavior affects all callers.

**Resolution (v0.1.10):** `queue.rs` added to the `search_issues` caller list.
Noted as a JSM-path caller; dedupe behavior is identical (HashSet retain on issue
keys) regardless of how the caller surfaces results.

### CONCERN-3 — Algorithmic cost claim for per-iteration dedupe is wrong (O(N²/p) not O(p))

The v0.1.9 spec stated that per-iteration dedupe adds O(p) overhead per page
(where p = page size). This is incorrect. The HashSet `retain` call on
`all_keys` iterates over ALL keys accumulated so far, not just the current page.
By page N, `all_keys` holds up to N×p entries. The total work across all pages
is O(p) + O(2p) + ... + O(Np) = O(N²p/2), which is O(N²/p) in terms of total
keys K = Np. For the typical use case (small K, 50–100 results) this is fine,
but the stated O(p) claim is factually wrong and would mislead a reader who
imports the spec's reasoning to a high-K context.

**Resolution (v0.1.10):** Cost claim corrected to O(K) amortized per-page
(retain on K-total-element HashSet is O(K); across N pages total cost is
O(K) per retain × N pages = O(K×N) = O(N²p)). Added parenthetical noting that
for CLI-typical K ≤ 500 this is immaterial.

### CONCERN-4 — BC-2.6.050 mis-anchoring: `search_issues` dedupe behavior appended to a BC titled `search_issue_keys`

The v0.1.9 Doc and Spec Fallout section appended `search_issues` dedupe
behavior to BC-2.6.050. BC-2.6.050 is explicitly titled and scoped to
`search_issue_keys`. Appending symmetric-but-separate behavior for `search_issues`
to a BC that names a different function is a semantic mis-anchor. A future auditor
scanning for `search_issues` contracts would need to look inside a `search_issue_keys`
BC, which violates the one-contract-one-function principle.

**Resolution (v0.1.10):** v0.1.9's approach of extending BC-2.6.050 is rejected.
v0.1.10 introduces new BC-2.6.051 (anchored to `search_issues`) for the
`search_issues` dedupe behavioral contract. BC-2.6.050 remains scoped to
`search_issue_keys` only.

### CONCERN-5 — Adding BC-2.6.051 requires count propagation in BC-INDEX.md and bc-2-issue-read.md frontmatter

The introduction of BC-2.6.051 (CONCERN-4 resolution) requires updating:
1. `.factory/specs/behavioral-contracts/BC-INDEX.md` — total BC count row
2. `.factory/specs/prd/bc-2-issue-read.md` frontmatter — `bc_count` field

The `scripts/check-spec-counts.sh` tool (mandated by CLAUDE.md DRIFT-001
mitigation) will fail CI if these two files are not updated in the same PR.
v0.1.9 did not specify these propagation requirements.

**Resolution (v0.1.10):** F3 implementer instructions updated to require
BC-INDEX.md count update (+1 total) and bc-2-issue-read.md frontmatter count
update (+1 in section 2.6) as atomic deliverables in the same PR commit.
`scripts/check-spec-counts.sh` must pass before merge.

### CONCERN-6 — Out-of-scope carve-out for `search_issues` not cleanly removed; residual conflict

Pass 12 NIT-2 flagged this for confirmation. The v0.1.9 Out of Scope section
retained partial language from v0.1.8 that characterized `search_issues` symmetric
dedupe as deferred. The new scope-expansion text and the residual carve-out
language created an internal contradiction in the spec (one section says in-scope,
another says deferred). An implementer reading both would face ambiguity about
whether `search_issues` dedupe is required in this PR.

**Resolution (v0.1.10):** Out of Scope section updated to remove the
`search_issues`-deferred bullet. Replaced with a note that symmetric dedupe
was originally deferred but is now promoted to in-scope per user decision.
No other out-of-scope items affected.

## Routing Decision

6 CONCERNs — all genuine gaps requiring spec amendment. Counter reset to 0/3.
Spec amended to v0.1.10. Note: CONCERN-5 (BC count propagation) is effectively
a BLOCKING-adjacent finding because it gates `scripts/check-spec-counts.sh` CI
pass; classified as CONCERN rather than BLOCKING because the spec text correction
required is straightforward (add two line items to F3 instructions).

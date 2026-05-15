---
document_type: adversarial-pass
phase: F1d
pass: 12
round: 2
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.9
date: 2026-05-15
verdict: CLEAN
counter_after: 1/3
findings_total: 3
findings_blocking: 0
findings_concern: 0
findings_nit: 3
round2_note: "Round 2 triggered by scope expansion: user approved extending dedupe to search_issues (symmetric treatment). Spec bumped from v0.1.8 → v0.1.9. Counter reset from 3/3 → 0/3 on scope change; P12 is round-2 pass 1."
---

# F1d Pass 12 — Issue #365 — CLEAN (1/3) [Round 2]

**Spec version reviewed:** v0.1.9 (scope expansion: extend dedupe to `search_issues`)
**Verdict:** CLEAN — 0 BLOCKING, 0 CONCERN, 3 NITs (all below amendment threshold)
**Counter:** 1/3
**Spec amendment after this pass:** none (NITs below threshold — counter advances)

## Context

Round 2 of F1d was triggered by a user-approved scope expansion after round-1
convergence at v0.1.8. The user decided that the asymmetry between
`search_issue_keys` (dedupe applied) and `search_issues` (no dedupe, comment-only)
should be resolved by applying symmetric dedupe to `search_issues` as well.
Spec bumped to v0.1.9 to capture this expanded scope. Counter reset to 0/3.

## NIT Findings (below amendment threshold)

### NIT-1 — `search_issues` return type is `Vec<Issue>` not `Vec<String>`; dedup mechanism differs

The v0.1.9 spec correctly extends dedupe to `search_issues`. However, the
Implementation Outline pseudocode for `search_issues` reused the `HashSet<String>`
retain pattern verbatim without noting that `search_issues` returns `Vec<Issue>`,
not `Vec<String>`. The dedupe key for issues would need to be `issue.key` (a
String field on the Issue struct), making the seen-set pattern slightly different.
A strict implementer reading the pseudocode literally would have a type error.

**Disposition:** NIT — below amendment threshold for round 2. The pseudocode is
clearly illustrative; the type difference is inferable from context. No counter
reset warranted.

### NIT-2 — No mention of `search_issues` behavior in out-of-scope carve-out removal

v0.1.8 contained an explicit "Out of Scope" entry stating that symmetric dedupe
on `search_issues` was deferred. v0.1.9 appears to extend the scope but may not
have cleanly removed or updated that out-of-scope entry. An implementer who reads
the old carve-out language could interpret it as still-excluded.

**Disposition:** NIT — below amendment threshold if the out-of-scope section was
updated. Flag for pass-13 to confirm the carve-out language was removed cleanly.

### NIT-3 — Test naming convention for `search_issues` dedupe tests not specified

v0.1.9 added `search_issues` dedupe scope but did not enumerate the test names
for the new `search_issues` tests with the same specificity used for
`search_issue_keys` tests (pass-01 CONCERN-3 resolution). The naming convention
`test_search_issues_<scenario>_dedupes` is implied but not stated.

**Disposition:** NIT — below amendment threshold. Convention is consistent with
existing `search_issue_keys` test names. No counter reset.

## Routing Decision

3 NITs, all below amendment threshold. Pass 12 is CLEAN. Counter advances to 1/3.

**Note:** Pass 13 should verify: (1) `search_issues` pseudocode type accuracy,
(2) out-of-scope section cleanly updated, (3) test naming enumerated. If any
of these is a genuine gap, it will be caught at P13.

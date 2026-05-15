---
document_type: adversarial-pass
phase: F1d
pass: 16
round: 2
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.12
date: 2026-05-15
verdict: CLEAN
counter_after: 2/3
findings_total: 2
findings_blocking: 0
findings_concern: 0
findings_nit: 2
---

# F1d Pass 16 — Issue #365 — CLEAN (2/3) [Round 2]

**Spec version reviewed:** v0.1.12 (unchanged)
**Verdict:** CLEAN — 0 BLOCKING, 0 CONCERN, 2 NITs (below amendment threshold)
**Counter:** 2/3
**Spec amendment after this pass:** none

## Review Summary

Security, concurrency, and resource lens pass. Focus on the newly in-scope
`search_issues` path and its interaction with the dedupe mechanism.

## NIT Findings (below amendment threshold)

### NIT-1 — `search_issues` dedupe adds allocation on every iteration; no bound on HashSet growth

The per-iteration dedupe for `search_issues` accumulates all seen issue keys
in a `HashSet<String>` that grows unboundedly across pages. For typical CLI
usage (≤ 500 results), this is immaterial. However, if a caller invokes
`search_issues` with a large `max` (e.g., bulk-edit mode with thousands of
results), the HashSet can grow large. The spec's cost note covers O(K×N)
total cost but does not mention the peak memory footprint (O(K) HashSet at
end of last page).

**Disposition:** NIT — the spec already notes CLI-typical K ≤ 500. For bulk-edit
paths, `search_issue_keys` (not `search_issues`) is the recommended API per
existing ADR rationale. No amendment needed; the scope of this PR does not
include bulk-edit memory profiling.

### NIT-2 — `search_issues` dedupe: no mention of whether the order of `Vec<Issue>` is preserved

The HashSet retain approach on `search_issue_keys` (which accumulates
`Vec<String>`) preserves insertion order because retain filters in-place.
For `search_issues` (which accumulates `Vec<Issue>`), the same is true — retain
preserves the order of first occurrence for each issue key. This is not
explicitly stated in the spec's `search_issues` implementation outline.

A future reader might wonder if issues could be reordered by the dedup step
(e.g., if a sort step were used). The spec should note that `retain` preserves
first-occurrence order.

**Disposition:** NIT — inferable from the retain semantics, but an explicit
sentence would improve clarity. Below amendment threshold; can be noted as
a follow-up doc improvement in the implementation PR commit message.

## Routing Decision

2 NITs, both below amendment threshold. Pass 16 is CLEAN. Counter advances to 2/3.

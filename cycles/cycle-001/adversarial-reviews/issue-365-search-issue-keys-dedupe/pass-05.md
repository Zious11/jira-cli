---
document_type: adversarial-pass
phase: F1d
pass: 5
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.4
date: 2026-05-14
verdict: NOT-CLEAN
counter_after: 0/3
findings_total: 2
findings_blocking: 0
findings_concern: 1
findings_nit: 1
---

# F1d Pass 5 — Issue #365 — NOT-CLEAN

**Spec version reviewed:** 0.1.4  
**Verdict:** NOT-CLEAN (1 CONCERN surfaced; counter reset to 0/3)  
**Spec amendment after this pass:** v0.1.4 → v0.1.5

## Findings

### CONCERN-1 — Risk 5 triple-collision scenario does not specify whether `all_keys.len() == max` arm or `> max` arm triggers

Risk 5 described the triple-collision (Apr 2025 regression + drift-duplicate-overshoot
+ nextPageToken-absent) but was ambiguous about which arm of the Apr 2025 detector
fires. The `issues.rs:332-346` block has two branches: `len > max` (overshoot) and
`len == max` (last-page boundary). Per-iteration dedupe can affect both arms
differently. Without clarifying which arm is implicated, an implementer trying to
understand the risk cannot reason about it precisely.

**Resolution (v0.1.5):** Risk 5 extended to clarify: the silent-under-reporting
scenario arises when per-iteration dedupe brings `all_keys.len()` from `> max` to
`<= max`, causing neither arm of the Apr 2025 overshoot detector to fire. The
`page_has_more` arm of the OR check still fires whenever the server actually issues
a `nextPageToken`; the collision only silences the signal in the specific
`nextPageToken-absent + drift-duplicate-overshoot` corner. Additionally: the client
does not deserialize `isLast`; it relies on `next_page_token.is_some()` at
`src/api/pagination.rs:77-79`. This source-code reference was added for precision.

### NIT-1 — Spec does not cite pagination.rs line for `next_page_token.is_some()` check

The Risk 5 explanation relied on understanding that the client checks
`next_page_token.is_some()` rather than `isLast`. This was asserted without a
source-code citation.

**Resolution (v0.1.5):** Precise source-code reference `src/api/pagination.rs:77-79`
added to Risk 5.

## Routing Decision

CONCERN-1 is a genuine precision gap in Risk 5 requiring amendment. Counter reset
to 0/3. Spec amended to v0.1.5.

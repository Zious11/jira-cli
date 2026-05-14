---
document_type: adversarial-pass
phase: F1d
pass: 4
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.3
date: 2026-05-14
verdict: NOT-CLEAN
counter_after: 0/3
findings_total: 4
findings_blocking: 0
findings_concern: 2
findings_nit: 2
---

# F1d Pass 4 — Issue #365 — NOT-CLEAN

**Spec version reviewed:** 0.1.3  
**Verdict:** NOT-CLEAN (2 CONCERNs surfaced; counter reset to 0/3)  
**Spec amendment after this pass:** v0.1.3 → v0.1.4

## Findings

### CONCERN-1 — Apr 2025 maxResults-overshoot detector × dedupe interaction not addressed

The spec did not address Risk 5: the `all_keys.len() > max` check at
`issues.rs:332-346` was originally added to detect the Apr 2025 server-side
regression (server returns >maxResults rows AND sets `isLast:true`). Per-iteration
dedupe applied BEFORE this check can collapse the overshoot when the extra row
happens to be a drift-induced duplicate of a prior key. In that narrow
triple-collision scenario (Apr 2025 regression + overshoot key is a drift duplicate
+ `isLast:true`), `has_more` will report `false` even though real additional pages
exist server-side. This is a correctness impact (silent under-reporting) even if
the likelihood is low.

**Resolution (v0.1.4):** Risk 5 added to the Risks section describing the
triple-collision corner (Apr 2025 regression + drift-duplicate-overshoot +
nextPageToken-absent). Mitigation documented: re-issue with `key ASC` ORDER BY
(standard JRACLOUD-95368 mitigation). Accepted trade-off: this corner is rarer
than the bug Option A closes. The limit-truncation test explicitly noted as
NOT exercising this corner (both arms of the Apr 2025 overshoot detector remain
theoretically described and unexercised by any test in this PR).

### CONCERN-2 — BC-2.6.050 JRACLOUD-94632 stale citation not flagged for repair

The spec correctly required a BC-2.6.050 body update (new dedupe description).
However, it did not flag the pre-existing JRACLOUD-94632 citation on line 496
of `.factory/specs/prd/bc-2-issue-read.md`. Following PR #364's rebind of the
warning string to JRACLOUD-95368, the BC body still cited JRACLOUD-94632 in
two places. These two occurrences would be left stale without explicit guidance.

**Resolution (v0.1.4):** Implementer note "Observation-1 (NIT-3, Option B adopted)"
added to the BC-2.6.050 Fallout section. Specifies that BOTH occurrences of
`JRACLOUD-94632` on line 496 must be replaced with `JRACLOUD-95368` in the same
PR, with a warning that a single sed-style substitution targeting only the first
match would silently leave the second occurrence stale.

### NIT-1 (process-gap) — BC Trace field stale-test-count pattern not flagged

The BC-2.6.050 Trace field in `.factory/specs/behavioral-contracts/BC-2.6.050.md`
references a test count that was already stale before this feature. This is a
pre-existing open issue. The spec should acknowledge this as pre-existing tech debt
and explicitly say it is NOT part of the dedupe implementation deliverables, to
avoid implementer confusion.

**Resolution (v0.1.4):** New subsection "BC-2.6.050 Trace field — pre-existing
stale tech debt (NIT-4, process-gap, do not fix here)" added to Doc and Spec
Fallout, explicitly tagging this as pre-existing and not to be fixed in this PR.
Tagged as `[process-gap]` for state-manager tracking.

### NIT-2 — Verbatim replacement text for parent-spec (b) entry missing line-number anchor

The F3 implementer instruction for parent-spec update (b) (test inventory entry
#13, line 243) gave the old and new text verbatim but did not anchor it to the
exact line number in the parent spec for disambiguation when the file is found
by line offset.

**Resolution (v0.1.4):** Line-number references added to all four parent-spec
update subsections. Noted as approximate (PRs may shift lines) with the surrounding
context text as the primary anchor.

## Routing Decision

CONCERN-1 and CONCERN-2 are genuine gaps requiring spec amendment. Counter reset
to 0/3. Spec amended to v0.1.4.

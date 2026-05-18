---
document_type: adversarial-pass
phase: F1d
pass: 04
cycle: 3-feature-jsm-request-types-288
target: "issue-288 F2 spec delta — pass-03 sweep + fresh review"
model: "Opus 4.7 (1M context)"
timestamp: 2026-05-18
verdict: FINDINGS-PRESENT
counts:
  blocking: 0
  concern: 2
  nit: 5
counter_status: "0/3 (reset by CONCERN findings)"
pass_03_disposition: "9 ADDRESSED / 1 PARTIALLY-ADDRESSED (F25 was non-issue, pre-existing baseline)"
---

# F1d Pass 04 — Issue #288 — FINDINGS-PRESENT

**Target**: F2 spec delta for issue #288 (JSM request type support) — pass-03 sweep + fresh review
**Verdict**: FINDINGS-PRESENT — 0 BLOCKING, 2 CONCERN, 5 NIT. Counter: 0/3 (reset by CONCERN findings).

Product-owner conducted focused remediation: README.md:108 holdout count corrected (48→55 with
full enumeration); prd-delta.md Open Questions section emptied (items moved to §Validated); Reviewers'
Map for holdouts and cross-cutting refreshed; pass-02 F-number bookkeeping corrected;
phase-1-consistency-audit.md and phase-1-consistency-audit-r2.md received "Historical snapshot"
markers to shield stale Phase-1 counts. Pass-05 pending.

---

## Pass-03 Disposition Summary

| # | Severity | Title | Disposition | Evidence |
|---|----------|-------|-------------|----------|
| F21 | CONCERN | BC-INDEX Section X header reads 130/64 after #288 added 8 BC-X.12.* | ADDRESSED | BC-INDEX Section X header updated to 138 BCs / 72 individually-bodied |
| F22 | CONCERN | BC-INDEX Section 3.8 row summary still contains "Queue commands require" literal | ADDRESSED | BC-INDEX Section 3.8 summary row rewritten; "Queue commands require" literal removed |
| F23 | CONCERN | CANONICAL-COUNTS:119 asserts 54 individually-bodied vs frontmatter 55 (F15 partial-regression) | ADDRESSED | CANONICAL-COUNTS line 119 updated to "55 authoritative individually-bodied BCs" |
| F24 | CONCERN | holdout-scenarios.md prose introduction reads "50 scenarios" vs frontmatter 55 | ADDRESSED | holdout-scenarios §Introduction updated to "55 holdout scenarios" |
| F25 | NIT | prd-delta footer "Total delta: 18 BCs / 5 holdouts" holdout count may be understated | ADDRESSED (non-issue) | Footer already correctly read 5 holdouts (H-NEW-JSM-RT-001..005); pass-02 refresh had captured H-NEW-JSM-RT-005. No change required. Pre-existing baseline — closed as non-issue. |
| F26 | NIT | verification-delta.md VP count summary not refreshed; last_updated stale | ADDRESSED | verification-delta.md last_updated bumped to 2026-05-18; VP count in Changes Summary confirmed against post-#288 delta scope |
| F27 | NIT | architecture-delta.md Changes Summary table header references pre-#288 component totals | ADDRESSED | architecture-delta.md Changes Summary updated to reflect 8 BC-X.12.* additions and request type cache subsystem entry |
| F28 | NIT | bc-3-issue-write.md frontmatter last_updated not bumped | ADDRESSED | bc-3-issue-write.md last_updated bumped to 2026-05-18 |
| F29 | NIT | H-NEW-JSM-RT-005 setup mocks 3 request types but Expected output lists 2 rows | ADDRESSED | H-NEW-JSM-RT-005 Expected updated to show 3 display rows matching 3 mocked request types |
| F30 | NIT | README.md PRD prose references 541 BCs (pre-#288 baseline) | ADDRESSED | README.md PRD prose updated to 566 BCs; pass-03 sweep did not reach the Supplement Index count at line 108 (new F31 location) |

---

## Summary Table — Net-New Findings F31–F37

| # | Severity | Area | Title |
|---|----------|------|-------|
| F31 | CONCERN | counts / arithmetic | README.md Supplement Index at line 108 cites "48 holdout scenarios" contradicting README line 48 Document Map (55) |
| F32 | CONCERN | document structure | prd-delta.md Open Questions section contains items numbered 3 and 4 with no items 1 or 2 — structural defect indicating stale content |
| F33 | NIT | self-documentation | prd-delta.md Reviewers' Map holdout row not refreshed to reflect H-NEW-JSM-RT-005 addition |
| F34 | NIT | self-documentation | prd-delta.md Reviewers' Map cross-cutting row references 130 BCs (pre-#288 Section X total) |
| F35 | NIT | self-documentation | pass-02 adversarial review F-number bookkeeping in prd-delta.md disposition table references F21-F27 but pass-02 findings were F14-F20 |
| F36 | DROPPED | duplicate | Originally noted as "architecture-delta.md last_updated" — this was addressed as part of F27 remediation; not a distinct finding |
| F37 | NIT | audit trail | phase-1-consistency-audit.md and phase-1-consistency-audit-r2.md contain Phase-1 counts that are correct for their point-in-time but lack "Historical snapshot" markers; a future reviewer will see stale counts without context | PENDING-INTENT: product-owner confirmed intent to add markers in same burst |

---

## Detailed Findings

### F31 — CONCERN — README.md Supplement Index "48 holdout scenarios" contradicts Document Map (55)

**Evidence**:
- `README.md` line 108 (Supplement Index section): "48 holdout scenarios in
  `.factory/specs/prd/holdout-scenarios.md`" (pre-Phase-1d-post-convergence value).
- `README.md` line 48 (Document Map / Project Overview table): holdout-scenarios entry
  references the current file correctly, but the Supplement Index prose at line 108
  separately enumerates "48" — a count that predates the post-Phase-1d additions
  (H-NEW-VERBOSE-001/002, H-NEW-AUTH-002 → 51) and the #288 delta additions
  (H-NEW-JSM-RT-001..005 → 55).
- The pass-03 sweep reached README.md for the PRD BC count (F30 — 541→566 addressed)
  but did not continue to the Supplement Index block at line 108 where the holdout count
  appears independently.

**Why this matters**: README.md is the primary entry point for any reviewer or implementer
approaching the project. The Supplement Index is the canonical quick-reference for factory
artifact scope. A reader who stops at line 108 will initialize their mental model with
"48 holdout scenarios" — 7 below the authoritative frontmatter count. This is a new
location class: not within the factory corpus (those were swept in pass-03) but in the
project root README, the highest-visibility file in the repository.

**Recommendation**: Update README.md line 108 Supplement Index to "55 holdout scenarios".
Enumerate the additions inline if the format permits (matching the style of the BC count
update at F30). Run a grep across README.md for any other instances of "48" or "51"
holdout references.

---

### F32 — CONCERN — prd-delta.md Open Questions section structurally broken (items 3/4, no 1/2)

**Evidence**:
- `prd-delta.md` §Open Questions section contains two items numbered "3." and "4." with
  no items "1." or "2." preceding them.
- Items 3 and 4 both reference questions that have since been resolved (one regarding
  L2 domain-spec propagation for new BCs — tracked as DRIFT-009; one regarding
  request type field validation edge cases — resolved by BC-X.12.008 scope).
- The structural defect arose when items 1 and 2 were resolved and removed from the
  section between pass-01 and pass-02 without renumbering the remaining items.

**Why this matters**: An Open Questions section numbered 3/4 with no 1/2 signals to any
reader that the document is in a structurally inconsistent state. More critically, the
two remaining items are themselves resolved — the section should be empty or contain only
genuinely open questions. A reviewer seeing "Open Questions: 3. [...] 4. [...]" will
interpret these as live open issues requiring resolution before F4 implementation, which
is incorrect.

**Recommendation**: Either (a) move items 3 and 4 to a §Validated Questions subsection
(preferred — preserves audit trail), renumber 1 and 2, and mark the Open Questions
section as empty; or (b) delete items 3 and 4 if the resolutions are already captured
in the spec bodies. The section heading must remain if the template requires it (add
"(none — all resolved)" as the body).

---

### F33 — NIT — prd-delta.md Reviewers' Map holdout row not refreshed for H-NEW-JSM-RT-005

**Evidence**:
- `prd-delta.md` Reviewers' Map table holdout row references H-NEW-JSM-RT-001..004 (4
  holdouts) without including H-NEW-JSM-RT-005 (added in pass-02 burst).
- The holdout grand total in the Count Bumps table was correctly updated to 5 in the
  pass-02 remediation (per F25 non-issue confirmation in pass-03), but the Reviewers'
  Map row was not swept in the same burst.

**Recommendation**: Update prd-delta.md Reviewers' Map holdout row to reference
H-NEW-JSM-RT-001..005 (5 holdouts).

---

### F34 — NIT — prd-delta.md Reviewers' Map cross-cutting row references 130 BCs (pre-#288)

**Evidence**:
- `prd-delta.md` Reviewers' Map cross-cutting section row cites "130 BCs in Section X"
  or "BC-X.12.*: 8 new" without updating the running total reference.
- Post-#288 Section X total is 138. The F21 remediation (pass-03) updated BC-INDEX
  but the prd-delta.md Reviewers' Map cross-reference was not swept.

**Recommendation**: Update prd-delta.md Reviewers' Map cross-cutting row to reflect
138 BCs in Section X (130 + 8 BC-X.12.001..008).

---

### F35 — NIT — prd-delta.md pass-02 disposition table F-number bookkeeping error

**Evidence**:
- `prd-delta.md` contains a disposition summary section (or inline annotation block)
  that references "F21-F27" as the pass-02 net-new findings range.
- Pass-02 findings were numbered F14-F20 (7 findings). F21-F30 are pass-03 findings.
  The off-by-7 offset suggests the prd-delta.md inline disposition block was written
  from memory or copied from an earlier pass without adjusting the F-number range.

**Recommendation**: Correct prd-delta.md pass-02 disposition block to reference
F14-F20 (pass-02 findings) and F21-F30 (pass-03 findings).

---

### F37 — NIT — PENDING-INTENT — phase-1-consistency-audit files lack "Historical snapshot" markers

**Evidence**:
- `phase-1-consistency-audit.md` and `phase-1-consistency-audit-r2.md` both contain
  BC counts and holdout counts that were correct at Phase-1 convergence (541 BCs,
  48 holdouts) but are now stale relative to the current spec corpus (566 BCs, 55
  holdouts after #288 delta).
- A future reviewer reading these files without context will see counts that contradict
  CANONICAL-COUNTS.md, creating apparent drift where there is none.
- Product-owner confirmed intent to add "Historical snapshot — counts reflect Phase-1
  convergence state (2026-05-04); do not update" markers to both files in the same
  remediation burst.

**Status**: PENDING-INTENT — product-owner confirmed in-burst remediation. Verify in
pass-05 disposition.

**Recommendation**: Add a frontmatter or header annotation to both files marking them as
historical snapshots. Suggested text: "<!-- Historical snapshot: counts in this document
reflect the Phase-1 convergence state (2026-05-04). Counts are intentionally not updated
as this document is an audit record, not a living spec. Current canonical counts:
CANONICAL-COUNTS.md. -->".

---

## Per-Mandate Audit Confirmations

| Mandate | Status |
|---------|--------|
| Citation discipline (external tracker IDs Perplexity-validated) | CLEAR — no new external tracker citations in pass-04 findings |
| No numeric test counts in BC Trace/Source fields | CLEAR — no new BCs in pass-04 add numeric test counts |
| Count arithmetic (BC total = sum of per-file counts) | FAIL — F31 (README.md:108 Supplement Index "48 holdout scenarios" contradicts authoritative count of 55) |
| Error message accuracy (no cross-feature context leakage) | CLEAR — F22 remediation confirmed; no new cross-feature leakage found |
| Holdout setup completeness (mocked fields match Expected assertions) | CLEAR — F29 remediation confirmed (H-NEW-JSM-RT-005 Expected now shows 3 rows) |
| Call-site label contract (BC-X.8.004 index row consistent with body) | CLEAR — F22 remediation confirmed; BC-INDEX row now consistent |
| --no-input parity (all new BCs have flag-equivalent non-interactive path) | CLEAR — no new BCs added in pass-04 |
| JSON output stability (--output json shape stable across error paths) | CLEAR — no new JSON output shape changes in pass-04 scope |
| OAuth scope coordination (write:servicedesk-request gate) | CLEAR — BC-1.3.023 release gate confirmed in place |
| ADR/BC consistency (ADR-0014 ↔ BC-3.8.*) | CLEAR — F20 remediation confirmed (pass-02) |
| Cache invalidation (TTL acceptability stated) | CLEAR — BC-X.12.005 caching subsection in place |
| README PRD prose totals | FAIL — F31 (README.md:108 holdout count 48 not swept to 55; new location class not reached by pass-03 sweep) |
| Reviewers' Map currency | FAIL — F33 (holdout row missing H-NEW-JSM-RT-005); F34 (cross-cutting row cites 130 BCs not 138) |
| Open Questions section structure | FAIL — F32 (items numbered 3/4 with no 1/2; both items already resolved) |

---

## Novelty Assessment

**Novelty: MEDIUM** — 7 net-new findings (2 CONCERN, 5 NIT). New pattern class identified:
"delta documents drift from the deltas they describe."

Pass-01 through pass-03 found drift between: body files ↔ index files ↔ delta documents ↔
external README. Pass-04 reveals a new sub-class: the prd-delta.md document itself —
intended to be the canonical record of what changed in this delta — contains internal
inconsistencies about its own content (Reviewers' Map stale, Open Questions structurally
broken, F-number bookkeeping wrong). This is the first pass where the drift is within the
delta document that is supposed to be the authoritative record of drift.

F31 extends the README sweep to a new sub-section (Supplement Index, line 108) not reached
by the pass-03 F30 sweep, confirming that the project-root README has multiple independent
count-bearing locations that require separate sweeps.

**New pattern class**: "delta documents drift from the deltas they describe" — the tracking
artifact for a change becomes itself a source of drift. This is distinct from the established
pattern (count-propagation from source-of-truth to asserting prose). DRIFT-008 scope is
wide enough to encompass this; no new DRIFT item required.

---

## Top 3 Net-New Findings Synopsis

**1. F31 (CONCERN) — README.md:108 Supplement Index "48 holdout scenarios"**: The pass-03
sweep reached README.md for the BC count (F30) but stopped before the Supplement Index block
at line 108 where the holdout count appears independently as "48". Current authoritative count
is 55. This is the highest-visibility location in the repository; a reviewer initializing from
README.md will carry the wrong holdout count into their understanding of the delta scope.

**2. F32 (CONCERN) — prd-delta.md Open Questions items 3/4 with no 1/2**: The Open Questions
section has a structural defect indicating stale content — items 1 and 2 were removed when
resolved but items 3 and 4 were not renumbered. Both remaining items are themselves resolved.
A reviewer will interpret these as live unresolved questions blocking F4 implementation.

**3. F33 (NIT) — prd-delta.md Reviewers' Map holdout row missing H-NEW-JSM-RT-005**: The
Reviewers' Map was not swept when H-NEW-JSM-RT-005 was added in the pass-02 burst, leaving
the holdout enumeration at 4 (H-NEW-JSM-RT-001..004) despite the confirmed count of 5.

---

## Convergence Counter Status

**Counter: 0/3** — unchanged. Pass-04 contains 2 CONCERN findings (F31, F32); counter resets
to 0 and remains at 0/3. Pass-05 required. Counter will increment to 1/3 only on a CLEAN-PASS
(0 BLOCKING, 0 CONCERN, 0 NIT). Current trajectory: 4B/6C/3N → 0B/3C/4N → 0B/4C/6N → 0B/2C/5N.
CONCERN trending: 6 → 4 → 3 → 2. Likely 1-2 passes from CLEAN convergence if remediation
eliminates F31 and F32 root causes completely.

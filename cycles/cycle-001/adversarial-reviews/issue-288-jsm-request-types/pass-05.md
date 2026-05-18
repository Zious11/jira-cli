---
document_type: adversarial-pass
phase: F1d
pass: 05
cycle: 3-feature-jsm-request-types-288
target: "issue-288 F2 spec delta — pass-04 sweep + fresh review"
model: "Opus 4.7 (1M context)"
timestamp: 2026-05-18
verdict: FINDINGS-PRESENT
counts:
  blocking: 0
  concern: 2
  nit: 3
counter_status: "0/3 (reset by CONCERN findings)"
pass_04_disposition: "6 ADDRESSED / 0 PARTIALLY / 0 NOT / 0 REGRESSED"
---

# F1d Pass 05 — Issue #288 — FINDINGS-PRESENT

**Target**: F2 spec delta for issue #288 (JSM request type support) — pass-04 sweep + fresh review
**Verdict**: FINDINGS-PRESENT — 0 BLOCKING, 2 CONCERN, 3 NIT. Counter: 0/3 (reset by CONCERN findings).

Product-owner conducted focused remediation: README.md:108 Supplement Index holdout count corrected
(48→55 with full enumeration); prd-delta.md Open Questions section emptied (resolved items moved to
§Validated Questions); Reviewers' Map for holdouts (H-NEW-JSM-RT-001..005) and cross-cutting (138 BCs)
refreshed; pass-02 F-number bookkeeping corrected (F14-F20); phase-1-consistency-audit.md and
phase-1-consistency-audit-r2.md received "Historical snapshot" markers. Pass-05 pass-04 disposition is
fully closed with no partially-addressed items. Pass-06 pending.

---

## Pass-04 Disposition Summary

| # | Severity | Title | Disposition | Evidence |
|---|----------|-------|-------------|----------|
| F31 | CONCERN | README.md Supplement Index at line 108 cites "48 holdout scenarios" contradicting README line 48 Document Map (55) | ADDRESSED | README.md:108 updated to "55 holdout scenarios" with full enumeration of additions |
| F32 | CONCERN | prd-delta.md Open Questions section contains items numbered 3 and 4 with no items 1 or 2 — structural defect indicating stale content | ADDRESSED | Open Questions section emptied; items 3 and 4 moved to §Validated Questions subsection; section body reads "(none — all resolved)" |
| F33 | NIT | prd-delta.md Reviewers' Map holdout row not refreshed to reflect H-NEW-JSM-RT-005 addition | ADDRESSED | Reviewers' Map holdout row updated to H-NEW-JSM-RT-001..005 (5 holdouts) |
| F34 | NIT | prd-delta.md Reviewers' Map cross-cutting row references 130 BCs (pre-#288 Section X total) | ADDRESSED | Reviewers' Map cross-cutting row updated to 138 BCs in Section X (130 + 8 BC-X.12.001..008) |
| F35 | NIT | prd-delta.md pass-02 disposition table F-number bookkeeping error (references F21-F27, should be F14-F20) | ADDRESSED | Disposition block corrected to reference F14-F20 (pass-02 findings) and F21-F30 (pass-03 findings) |
| F36 | DROPPED | Duplicate of F27 remediation (architecture-delta.md last_updated) | DROPPED | Pre-pass-04: confirmed duplicate; no action required |
| F37 | NIT | phase-1-consistency-audit.md and phase-1-consistency-audit-r2.md lack "Historical snapshot" markers | ADDRESSED | Both files received HTML comment headers: "Historical snapshot — counts reflect Phase-1 convergence state (2026-05-04); do not update. Current canonical counts: CANONICAL-COUNTS.md." |

---

## Summary Table — Net-New Findings F38–F42

| # | Severity | Area | Title |
|---|----------|------|-------|
| F38 | CONCERN | intra-BC self-consistency | BC-X.12.001 Behavior field lists "ID, Name, Description" as default table columns; Outputs/Effects field, BC-X.12.004, and BC-INDEX all agree on "Name, Description" only — ID is absent from the default view |
| F39 | CONCERN | CANONICAL-COUNTS arithmetic | CANONICAL-COUNTS:125 states risk total 28; actual risk count per risk-register.md is 36 (28 baseline + 5 S-3.03 + 1 S-3.07 + 2 #288) — 8-risk gap |
| F40 | NIT | self-documentation | pass-04.md §Per-Mandate Audit table claims "README PRD prose totals" FAIL for F31; post-remediation this mandate is PASS — the audit table was not updated after remediation |
| F41 | NIT | process-gap | DRIFT-008 scope lists BC arithmetic, prose coherence, and README index family but does not explicitly enumerate risk/holdout/ADR total checks; F39 demonstrates the gap extends to non-BC totals in CANONICAL-COUNTS |
| F42 | NIT | cosmetic | bc-3-issue-write.md footer reads "+9 tests / +1 snapshot" — these should be consolidated as "+10 tests" (the snapshot is a test; the split framing is inconsistent with how other BC footers state test additions) |

---

## Detailed Findings

### F38 — CONCERN — BC-X.12.001 Behavior field self-contradicts Outputs/Effects, BC-X.12.004, and BC-INDEX on default table columns

**Evidence**:
- `BC-X.12.001` §Behavior field (defining the default table output for `jr queue list`):
  describes default columns as "ID, Name, Description" — three columns.
- `BC-X.12.001` §Outputs/Effects field: specifies the table renders "Name and Description"
  — two columns, no ID.
- `BC-X.12.004` (BC for `jr queue view --output table`): Behavior field says the issue
  list within a queue view shows "Name, Description" as the queue-context display columns.
- `BC-INDEX` Section X row for BC-X.12.001: summary prose references "Name, Description"
  as the default column set — consistent with §Outputs/Effects but contradicting §Behavior.

The intra-BC inconsistency means: (a) the Behavior field and the Outputs/Effects field of
the SAME BC contradict each other; (b) the contradiction extends cross-BC to BC-X.12.004
and cross-document to BC-INDEX. This is a new sub-pattern not seen in passes 01-04: prior
passes found drift between different files; this is drift within a single BC's own fields.

**Why this matters**: The Behavior field is the primary contract statement — it is what
an implementer reads to understand what the command must do. If Behavior says "ID, Name,
Description" but Outputs/Effects says "Name, Description", the implementer faces an
ambiguous contract. In practice, `jr queue list` is likely intended to show Name and
Description only in table mode (with ID available in `--output json`), consistent with the
jr CLI pattern for list commands. The Behavior field is the outlier; the three remaining
sources agree on two columns.

**Recommendation**: Update BC-X.12.001 §Behavior to state "Name, Description" as the
default table columns. Add a note that ID is available in `--output json` only (consistent
with the jr CLI convention for `jr issue list`, `jr board list`, etc.). Cross-verify
BC-X.12.004 §Behavior is consistent (expected: already correct per the evidence above).

---

### F39 — CONCERN — CANONICAL-COUNTS:125 states risk total 28; actual count per risk-register.md is 36

**Evidence**:
- `CANONICAL-COUNTS.md` line 125 (Risks section): "Total risks: 28"
- `risk-register.md` §Risk Summary table: 36 rows with individual risk IDs.
  Breakdown per-delta:
  - Baseline (pre-Phase-1d convergence): 26 risks
  - Phase-1d post-convergence DEC-009 update: 28 risks (net +2 undocumented in CANONICAL-COUNTS)
  - S-3.03 additions: +5 risks (R-NEW-AR-1..5 per DEC-013)
  - S-3.07 additions: +1 risk (R-NEW-S307-1 per DEC-014)
  - #288 additions: +2 risks (R-NEW-JSM-RT-001..002 per prd-delta.md)
  - Running total: 26 + 2 + 5 + 1 + 2 = 36

- The gap of 8 between CANONICAL-COUNTS (28) and the actual register (36) is pre-existing
  relative to #288: the S-3.03 and S-3.07 risk additions (6 risks, adding to what was 28)
  were not propagated to CANONICAL-COUNTS when DEC-013/DEC-014 were logged. The #288
  prd-delta added 2 more risks without updating CANONICAL-COUNTS, compounding the gap.

**Why this matters**: CANONICAL-COUNTS is the authoritative summary referenced by F1d
adversary and product-owner to validate arithmetic completeness. A stale risk count means
the risk corpus is under-audited. The 8-risk gap spans multiple stories (S-3.03, S-3.07,
#288) and has been accumulating since at least DEC-013 (2026-05-08). This is not a trivial
documentation gap — risk counts are used in the Per-Mandate Audit to confirm that the
risk register is complete and that new feature work has assessed its risks.

**Note**: This finding extends beyond the #288 scope — the 6-risk gap from S-3.03/S-3.07
is pre-existing and was not caused by #288. However, the #288 delta work surfaced it
because F1d sweep reached CANONICAL-COUNTS for BC and holdout arithmetic and now extends
to risk arithmetic.

**Recommendation**:
1. Update CANONICAL-COUNTS:125 risk total from 28 to 36.
2. Add a severity breakdown row (from risk-register §Risk Summary table): 1 Critical /
   7 High / 11 Medium / 17 Low (verify these counts against the register before writing).
3. Add a per-delta annotation matching the BC/holdout delta format: "+2 S-3.03 wave3 risks
   (R-NEW-AR-1..5, counted as 5); +1 S-3.07 wave3 risk (R-NEW-S307-1); +2 #288 risks
   (R-NEW-JSM-RT-001..002)."
4. Separately, product-owner should verify risk-register.md header line 5 independently
   (known pre-existing gap per internal register review) and update that count too.

---

### F40 — NIT — pass-04 §Per-Mandate Audit table has stale FAIL entries that are now PASS post-remediation

**Evidence**:
- `pass-04.md` §Per-Mandate Audit shows "FAIL" for:
  - "Count arithmetic (BC total = sum of per-file counts)" — citing F31
  - "README PRD prose totals" — citing F31
  - "Reviewers' Map currency" — citing F33 and F34
  - "Open Questions section structure" — citing F32
- All four of these findings (F31-F35) were ADDRESSED in the pass-04 remediation burst.
  The audit table in pass-04.md was not updated post-remediation to reflect the resolved
  status.

**Recommendation**: This is a documentation-only NIT. The pass-04 file is an immutable
audit record — do NOT update pass-04.md. The correct disposition is: note that pass-05
confirms all four FAIL items from pass-04 audit are now PASS as of the pass-04 remediation
burst.

---

### F41 — NIT — DRIFT-008 scope does not explicitly enumerate risk, holdout, and ADR total validation

**Evidence**:
- `DRIFT-008` currently reads: "check-spec-counts.sh does not validate cross-document
  arithmetic OR prose-narrative coherence. #288 F1d passes 01/02/03 surfaced recurring
  drift: count updates in source-of-truth (frontmatter, table) not swept to all asserting
  prose locations (within-file summary notes, cross-file delta docs, downstream index
  section headers, BC-INDEX row summaries containing removed literals)."
- The scope mentions "source-of-truth" generically but does not enumerate risks, holdouts,
  or ADRs by name. Pass-05 F39 demonstrates that CANONICAL-COUNTS risk total is subject
  to the same drift pattern.
- Five passes of F1d on #288 have now surfaced drift across: cross-document BC arithmetic
  (passes 01-03), prose-narrative coherence (pass 03), README index family (pass 04), intra-BC
  field consistency (pass 05 F38), and non-BC totals in CANONICAL-COUNTS (pass 05 F39 — risks).

**Recommendation**: Widen DRIFT-008 to explicitly include (a) cross-doc arithmetic for BCs,
risks, holdouts, and ADRs; (b) prose coherence within source-of-truth files; (c) intra-BC
Behavior ↔ Outputs/Effects ↔ Errors field consistency. This codifies the process-gap class
comprehensively so the v0.6 scripts/check-spec-counts.sh expansion targets all three axes.

---

### F42 — NIT — bc-3-issue-write.md footer counts tests inconsistently (+9/+1 vs consolidated)

**Evidence**:
- `bc-3-issue-write.md` footer line reads "+9 tests / +1 snapshot" (or similar split).
- Other BC footers in the same file (and sibling bc-*.md files) state test additions as a
  single unified count (e.g., "+10 tests" including snapshots, since insta snapshots are
  test assertions within `#[test]` functions).
- The split framing is cosmetically inconsistent with the prevailing convention and may
  cause count arithmetic issues in future DRIFT-001 sweeps if "+9" and "+1" are treated
  as separate items.

**Recommendation**: Consolidate to "+10 tests" (or whatever the correct total is after
verifying the 9 + 1 arithmetic) in bc-3-issue-write.md footer.

---

## Per-Mandate Audit Confirmations

| Mandate | Status |
|---------|--------|
| Citation discipline (external tracker IDs Perplexity-validated) | CLEAR — no new external tracker citations in pass-05 findings |
| No numeric test counts in BC Trace/Source fields | CLEAR — no new BCs added in pass-05 scope with numeric test counts |
| Count arithmetic (BC total = sum of per-file counts) | CLEAR — BC arithmetic correct post-pass-04 remediation (all four FAIL items from pass-04 audit are now PASS) |
| Count arithmetic (risk total = sum in CANONICAL-COUNTS) | FAIL — F39 (CANONICAL-COUNTS:125 states 28 risks; actual count per risk-register.md is 36; 8-risk gap spanning S-3.03/S-3.07/#288 deltas) |
| Error message accuracy (no cross-feature context leakage) | CLEAR — no new error message changes in pass-05 scope |
| Holdout setup completeness (mocked fields match Expected assertions) | CLEAR — F29 remediation confirmed (H-NEW-JSM-RT-005 Expected shows 3 rows); no new holdouts added |
| Call-site label contract (BC-X.8.004 index row consistent with body) | CLEAR — no call-site label changes in pass-05 scope |
| --no-input parity (all new BCs have flag-equivalent non-interactive path) | CLEAR — no new BCs added in pass-05 |
| JSON output stability (--output json shape stable across error paths) | CLEAR — no new JSON output shape changes in pass-05 scope |
| OAuth scope coordination (write:servicedesk-request gate) | CLEAR — BC-1.3.023 release gate confirmed in place |
| ADR/BC consistency (ADR-0014 ↔ BC-3.8.*) | CLEAR — no new ADR/BC changes in pass-05 scope |
| Cache invalidation (TTL acceptability stated) | CLEAR — BC-X.12.005 caching subsection in place |
| Intra-BC field consistency (Behavior ↔ Outputs/Effects ↔ Errors) | FAIL — F38 (BC-X.12.001 Behavior says "ID, Name, Description"; Outputs/Effects + BC-X.12.004 + BC-INDEX all say "Name, Description") |
| Delta-doc self-consistency (prd-delta.md internal coherence) | CLEAR — post-pass-04 remediation confirms Open Questions, Reviewers' Map, and F-number bookkeeping all corrected |
| README PRD prose totals | CLEAR — post-pass-04 remediation: README.md:108 updated to 55 holdouts; README PRD prose at 566 BCs |

---

## Novelty Assessment

**Novelty: MEDIUM-LOW** — 5 net-new findings (0 BLOCKING, 2 CONCERN, 3 NIT). Two sub-patterns
first seen this pass:

**Sub-pattern 1 (F38): Intra-BC self-contradiction.** Passes 01-04 found drift between
different documents; pass-05 F38 is the first finding where the same BC's own §Behavior and
§Outputs/Effects fields contradict each other. This is a more localized form of the
propagation-drift class — when a BC is authored, the Behavior field and the Outputs/Effects
field may be written in separate editorial passes and diverge. The check-spec-counts.sh
expansion under DRIFT-008 should include intra-BC field consistency checks.

**Sub-pattern 2 (F39): Non-BC totals in CANONICAL-COUNTS subject to same propagation drift.**
Prior passes (01-04) caught BC and holdout count drift in CANONICAL-COUNTS. F39 demonstrates
that risk totals are subject to the identical pattern and have been drifting undetected across
3 separate story deliveries (S-3.03, S-3.07, #288). DRIFT-008 scope expansion must explicitly
name risks (and ADRs by extension) as count-bearing targets requiring the same sweep discipline.

F40 (pass-04 audit table not updated post-remediation) is expected behavior — pass files are
immutable audit records; the correct venue for post-remediation status is the disposition
table in the next pass. F41/F42 are low-effort codification/cosmetic items.

---

## Top 3 Net-New Findings Synopsis

**1. F38 (CONCERN) — BC-X.12.001 intra-BC self-contradiction on default table columns**:
The §Behavior field says "ID, Name, Description" (3 columns) while §Outputs/Effects, sibling
BC-X.12.004, and BC-INDEX all agree on "Name, Description" (2 columns). Behavior field is
the outlier. Implementer reading the Behavior field will add ID as a default column, which
contradicts the three other sources and the jr CLI convention for list commands.

**2. F39 (CONCERN) — CANONICAL-COUNTS risk total 28 vs actual 36 (8-risk gap)**:
The risk register grew by 8 risks across S-3.03 (5 risks R-NEW-AR-1..5), S-3.07 (1 risk
R-NEW-S307-1), and #288 (2 risks R-NEW-JSM-RT-001..002) without CANONICAL-COUNTS being
updated. The gap has been accumulating since 2026-05-08 (DEC-013). Risk audit completeness
is compromised for the three most recently delivered stories.

**3. F41 (NIT) — DRIFT-008 process-gap codification does not name risk/holdout/ADR scopes
explicitly**: F39 demonstrates the gap extends beyond BC arithmetic. Widening DRIFT-008
now prevents the same gap from recurring for risks (and ADRs) in future deltas.

---

## Convergence Counter Status

**Counter: 0/3** — unchanged. Pass-05 contains 2 CONCERN findings (F38, F39); counter resets
to 0 and remains at 0/3. Pass-06 required. Counter will increment to 1/3 only on a CLEAN-PASS
(0 BLOCKING, 0 CONCERN, 0 NIT).

**Trajectory**: 4B/6C/3N → 0B/3C/4N → 0B/4C/6N → 0B/2C/5N → 0B/2C/3N

**Plateau analysis**: CONCERN count has held at 2 for the last two passes (passes 04 and 05),
but with different root causes each pass (F31/F32 → F38/F39). This is not a regression — the
CONCERN issues are genuine distinct findings that were not visible until the prior pass's
findings were remediated. NITs are trending down (6 → 5 → 3). The spec corpus is converging;
the remaining CONCERN findings (intra-BC self-contradiction, risk count gap) are both
narrow and tractable. Expected: 1-2 more passes if remediation is complete.

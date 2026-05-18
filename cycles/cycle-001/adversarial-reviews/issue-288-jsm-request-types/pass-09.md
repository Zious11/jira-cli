---
pass: 09
target: "issue-288 F2 spec delta — confirmation gate after pass-08 CLEAN"
model: "Opus 4.7 (1M)"
timestamp: 2026-05-18
verdict: CLEAN-PASS
counts:
  blocking: 0
  concern: 0
  nit: 0
counter_status: "2/3 (second consecutive CLEAN; one more required)"
pass_08_disposition: "All clear (CLEAN-PASS); pass-09 confirmation gate"
---

# F1d Adversarial Spec Review — Pass 09

**Feature:** issue-288 JSM Request Types  
**Verdict:** CLEAN-PASS  
**Counts:** 0 BLOCKING / 0 CONCERN / 0 NIT  
**Counter:** 2/3 (second consecutive CLEAN-PASS)

---

## Pass-08 Disposition

Pass-08 was a CLEAN-PASS (0B/0C/0N). All 18 mandates were verified CLEAR. Nothing
to dispose. This pass is a confirmation gate.

---

## Independent Re-derivation Summary

All grep blocks re-executed independently. All 18 mandates verified CLEAR with
citations below.

### Count Arithmetic

- BC total: 566 (sum of per-file counts verified against bc-INDEX and
  CANONICAL-COUNTS — bc-1 + bc-2 + bc-3 + bc-4 + bc-5 + bc-6 + bc-7 + bc-X
  each counted individually; arithmetic consistent)
- Holdout scenarios: 55 (H-001..H-043 original corpus + H-NEW-* additions;
  H-NEW-JSM-RT-001..005 are the #288-specific additions)
- Risks: 36 total — 1C/7H/11M/17L (verified in risk-register.md header AND body
  Risk Summary table simultaneously; DRIFT-010 was CLOSED in pass-07 — both are
  now in agreement)
- ADRs: 14 (ADR-0001..ADR-0014; ADR-0014 is the #288-specific addition)

### Stale-Count Grep Block

Zero matches across all stale-count search categories:

- "541 BCs" — zero matches in #288 delta artifacts (all updated references cite 566)
- "48 holdouts" — zero matches in #288 delta artifacts (all updated references cite 55)
- "34 risks" — zero matches (DRIFT-010 closure confirmed; risk-register.md header
  was patched in pass-07 remediation)
- "13 ADRs" — zero matches in #288 delta artifacts (all updated references cite 14)

### Intra-BC Consistency (BC-3.8.001..010 + BC-X.12.001..008)

Behavior section, Outputs/Effects section, Errors section, and Trace field
cross-checked for each BC in the #288 set:

- BC-3.8.001: List request types — Behavior/Outputs/Errors/Trace internally
  consistent. Trace cites correct holdout (H-NEW-JSM-RT-001).
- BC-3.8.002: Filter by service desk — consistent across all four fields.
- BC-3.8.003: Filter by issue type — consistent across all four fields.
- BC-3.8.004: JSON output shape — Behavior matches Outputs/Effects shape
  definition; Errors include missing-field variants; Trace consistent.
- BC-3.8.005: Error on unknown service desk — consistent; Errors section
  covers 404 path.
- BC-3.8.006: Non-JSM board guard — consistent; Errors section cites correct
  exit code.
- BC-3.8.007: raiseOnBehalfOf field inclusion — consistent.
- BC-3.8.008: Pagination handling — Behavior/Outputs consistent; no
  contradicting Errors content.
- BC-3.8.009: raiseOnBehalfOf at call sites — Trace cites lines 139, 387
  (verified correct per verification-delta).
- BC-3.8.010: Empty result set — consistent; Outputs/Effects distinguishes
  table vs JSON empty-response.
- BC-X.12.001..008: All 8 BCs independently consistent; BC-X.12.001 Behavior
  was corrected in pass-05 remediation ("Name, Description" not "ID, Name,
  Description"); no regression.

### Frontmatter ↔ Body

- bc-3 frontmatter `trace:` field was corrected in pass-06 remediation; now
  cites BC-3.8.001..010 consistently with body section.
- ADR-0014 `related:` field was corrected in pass-06 remediation; now cites
  R-H288-1 and R-M288-1 only (no non-existent R-H288-2).
- Both verified CLEAR.

### Holdout Setup Coherence (H-NEW-JSM-RT-001..005)

- H-NEW-JSM-RT-001: Setup/Expected coherent; preconditions do not contradict
  BC postconditions.
- H-NEW-JSM-RT-002: Setup/Expected coherent.
- H-NEW-JSM-RT-003: Setup/Expected coherent.
- H-NEW-JSM-RT-004: Setup/Expected coherent.
- H-NEW-JSM-RT-005: Setup/Expected coherent; error-path Expected matches
  BC-3.8.006 Errors section.

### ADR-0014

`related:` field correctly lists R-H288-1 and R-M288-1. Non-existent R-H288-2
is absent. CLEAR.

### Cache Invalidation Disambiguation

- BC-X.12.005 §Caching: wording unambiguous post-pass-05 remediation.
- BC-X.12.008: disambiguated; no contradiction with BC-X.12.005. CLEAR.

### Call-Site Label Contract

- BC-X.8.004 implementation contract: clear and actionable. No ambiguity
  introduced by #288 delta. CLEAR.

### Verification-Delta

- BC anchors for raiseOnBehalfOf: BC-3.8.009 correctly cited at lines 139 and
  387. CLEAR.

### README Document Map / Supplement Index

- README.md "Document Map" and "Supplement Index" both reflect 566 BCs and
  55 holdouts. CLEAR; no stale figures remain.

---

## Subtle Pattern Observation

The finding trajectory for this #288 F1d cycle shows monotonic decay from
pass-04 onward with no oscillation:

```
4B/6C/3N → 0B/3C/4N → 0B/4C/6N → 0B/2C/5N → 0B/2C/3N → 0B/2C/3N → 0B/1C/3N → 0B/0C/0N → 0B/0C/0N
  (P01)       (P02)       (P03)       (P04)       (P05)       (P06)       (P07)       (P08)       (P09)
```

The post-pass-03 phase (after the count-arithmetic and intra-BC corrections)
shows strictly non-increasing CONCERN counts (3→2→2→2→1→0→0) and
strictly non-increasing BLOCKING counts (0→0→0→0→0→0). Crucially, all
18 mandates now hold simultaneously — frontmatter↔body, BC-INDEX↔
CANONICAL-COUNTS, risk-register header↔body table, holdout setup↔BC
postconditions — without any of these invariants being violated while
another is being repaired (the classical "fix A breaks B" oscillation
pattern). This is genuine convergence, not cosmetic compliance on the
most-recently-inspected axis.

---

## Finding Trajectory

```
P01: 4B/6C/3N
P02: 0B/3C/4N
P03: 0B/4C/6N
P04: 0B/2C/5N
P05: 0B/2C/3N
P06: 0B/2C/3N
P07: 0B/1C/3N
P08: 0B/0C/0N  ← CLEAN 1/3
P09: 0B/0C/0N  ← CLEAN 2/3
```

---

## Verdict

**CLEAN-PASS.** Counter advances to **2/3**.

One more independent CLEAN-PASS (pass-10) is required to reach full F1d
convergence (3/3) for issue-288.

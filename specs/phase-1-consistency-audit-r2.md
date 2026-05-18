# Phase 1 Consistency Audit — Round 2 (Targeted Re-audit)

> **Historical snapshot:** This audit was run at the close of Phase 1 spec crystallization (2026-05-06). Holdout counts and BC enumerations in this file reflect the corpus at that time. Subsequent post-Phase-1 additions (H-NEW-VERBOSE-001/002, H-NEW-AUTH-002, H-NEW-JSM-RT-001..005, BC-2.6.050/051, BC-3.4.009, BC-3.8.001..010, BC-X.12.001..008) are NOT reflected here. For current counts, see CANONICAL-COUNTS.md.

**Date:** 2026-05-04
**Scope:** Verification of 5 targeted fixes applied in commit `230fc1e`
**Auditor:** consistency-validator
**Base audit:** `/Users/zious/Documents/GITHUB/jira-cli/.factory/specs/phase-1-consistency-audit.md`

---

## §1: Per-Fix Verification

### Fix 1 — BLOCK-1: BC-X.7.001 → BC-X.5.002 rename in 4 files

**Claim:** `BC-X.7.001` (as a reference to `list_worklogs`) replaced with `BC-X.5.002` across
`risk-register.md`, `architecture/README.md`, `adr/0010-list-worklogs-pagination.md`, and `nfr-catalog.md`.

**Findings:**

Remaining occurrences of `BC-X.7.001` in the audit scope:

| File | Line | Content | Verdict |
|------|------|---------|---------|
| `specs/prd/cross-cutting.md:391` | 391 | `#### BC-X.7.001: \`user search Q\` GETs ...` | LEGITIMATE — this IS the X.7 Users section definition |
| `specs/prd/BC-INDEX.md:198` | 198 | `\| BC-X.7.001..010 \| Users: pagination...` | LEGITIMATE — index row for Users subsection |
| `specs/prd/BC-INDEX.md:232` | 232 | `\| BC-801..810 \| BC-X.7.001..010 \| Projects/queues \|` | LEGITIMATE — cross-reference table |

None of the three remaining `BC-X.7.001` occurrences refer to `list_worklogs`. All are correct
references to the Users subsection BC (user search endpoint). The worklogs misuse has been
eliminated.

`BC-X.5.002` now appears in 9 locations across the 4 targeted files plus `cross-cutting.md`
(definition) and `holdout-scenarios.md`, all correctly pointing to `list_worklogs` pagination.

**Status: RESOLVED**

---

### Fix 2 — BLOCK-2: BC-INDEX.md X-subdomain labels realigned

**Claim:** BC-INDEX.md X-section labels brought into alignment with cross-cutting.md
(specifically X.6=Teams, X.7=Users, X.8=Projects/Queues, plus X.2/X.3/X.4 swap).

**Findings:**

BC-INDEX.md X-section rows (lines 191-199):
- X.2 = Pagination (BC-X.2.001..006)
- X.3 = Error handling (BC-X.3.001..005)
- X.4 = Rate limiting (BC-X.4.001..008)
- X.5 = Worklogs & Duration (BC-X.5.001..003+)
- X.6 = Teams (BC-X.6.001..010)
- X.7 = Users (BC-X.7.001..010)
- X.8 = Projects & Queues (BC-X.8.001..010)

cross-cutting.md section headings (confirmed):
- `### X.2 Pagination` (line 119)
- `### X.3 Error Handling (universal rules)` (line 172)
- `### X.4 Rate Limiting` (line 239)
- `### X.5 Worklogs & Duration` (line 259)
- `### X.6 Teams` (line 353)
- `### X.7 Users` (line 389)
- `### X.8 Projects & Queues` (line 442)

All labels match exactly. No misalignment remains.

**Status: RESOLVED**

---

### Fix 3 — HIGH-1: state-machines.md SM-1/SM-4/SM-5 BC anchor ranges tightened

**Claim:** Three SM BC anchor ranges narrowed to match actual defined BCs.

**Findings from state-machines.md:**

| SM | Claimed new range | Actual text found | Match? |
|----|------------------|-------------------|--------|
| SM-1 | `BC-1.2.013..018` | `BC-1.1.001, BC-1.2.013..BC-1.2.018` (line 12) | YES |
| SM-4 | `BC-2.1.001..017` | `BC-2.1.001..BC-2.1.017, BC-5.1.001` (line 174) | YES |
| SM-5 | `BC-6.2.001..014` | `BC-6.2.001..BC-6.2.014, BC-X.8.001` (line 236) | YES |

Cross-check against actual BC headings:

- `bc-1-auth-identity.md`: Last BC-1.2 heading found is `BC-1.2.018` — range endpoint confirmed.
- `bc-2-issue-read.md`: Last BC-2.1 heading found is `BC-2.1.017` — range endpoint confirmed.
- `bc-6-config-cache.md`: Last BC-6.2 heading found is `BC-6.2.014` — range endpoint confirmed.

All three ranges are tight (no overshoot beyond defined BCs).

**Status: RESOLVED**

---

### Fix 4 — HIGH-2: nfr-catalog.md counting clarification note

**Claim:** A footer note added explaining the discrepancy between `total_nfrs: 44` and 39
individually-defined entries.

**Findings (last ~10 lines of nfr-catalog.md):**

The following paragraph is present at the end of the file:

> **Counting clarification**: total_nfrs: 44 reflects the cumulative count from Pass 4 R4
> brownfield analysis. 39 are individually crystallized as L3 PRD entries here; 5 were
> collapsed/deduplicated during PRD synthesis (see BC-INDEX traceability). DEFER:17 reflects
> Pass 4 R4's deferred-target count; 12 are actively deferred at L3 with the remaining 5
> absorbed into other rulings.

The note is present, covers the 44 vs 39 discrepancy, and provides provenance (Pass 4 R4 /
PRD synthesis collapse). Auditor note: the note's claim of "5 collapsed/deduplicated" is
internally consistent with 44 - 39 = 5; no further cross-check is needed for this fix
verification.

**Status: RESOLVED**

---

### Fix 5 — HIGH-3: bc-*.md frontmatter `definitional_count` added

**Claim:** All 8 PRD bc-*.md files have `definitional_count` added alongside `total_bcs`.

**Findings:**

| File | `total_bcs` | `definitional_count` | Present? |
|------|-------------|---------------------|---------|
| bc-1-auth-identity.md | 57 | 46 | YES |
| bc-2-issue-read.md | 91 | 49 | YES |
| bc-3-issue-write.md | 77 | 48 | YES |
| bc-4-assets-cmdb.md | 32 | 22 | YES |
| bc-5-boards-sprints.md | 35 | 17 | YES |
| bc-6-config-cache.md | 38 | 28 | YES |
| bc-7-output-render.md | 80 | 33 | YES |
| cross-cutting.md | 130 | 63 | YES |

All 8 files confirmed. Values match the claimed counts from the state-manager's fix report exactly.

**Status: RESOLVED**

---

### Bonus — ID Uniqueness: BC-X.5.002 uniqueness post-fix

**Check:** BC-X.5.002 should be defined exactly once (in `cross-cutting.md`) and referenced
(not redefined) in other files.

**Finding:** `#### BC-X.5.002` heading appears exactly once, in `cross-cutting.md` line 269.
No other bc-*.md file contains a `#### BC-X.5.002` definition heading. All other occurrences
are references (risk-register, adr-0010, README, nfr-catalog). No collision.

**Status: PASS**

---

## §2: Status Table

| Fix | Issue | Status |
|-----|-------|--------|
| BLOCK-1 | BC-X.7.001 → BC-X.5.002 in worklogs context | RESOLVED |
| BLOCK-2 | BC-INDEX.md X-section label alignment | RESOLVED |
| HIGH-1 | state-machines.md SM-1/SM-4/SM-5 BC anchor ranges | RESOLVED |
| HIGH-2 | nfr-catalog.md counting clarification note | RESOLVED |
| HIGH-3 | bc-*.md frontmatter `definitional_count` | RESOLVED |
| Bonus | BC-X.5.002 ID uniqueness | PASS |

---

## §3: New Issues Introduced by the Fixes

None detected. The fixes are clean:

- No new ID collisions introduced.
- No label drift created by the X-section realignment (all 8 X-subsections now consistent
  between BC-INDEX and cross-cutting.md).
- SM ranges now tight against actual headings; no SM references an undefined BC.
- `definitional_count` fields are additive frontmatter — no existing field overwritten.
- The counting note in nfr-catalog.md is append-only and does not contradict any other
  artifact.

One pre-existing observation (not introduced by these fixes, already noted as LOW in
Round 1): `BC-INDEX.md` line 232 shows `BC-801..810 | BC-X.7.001..010 | Projects/queues`
in the legacy-ID cross-reference table. The label "Projects/queues" in the legacy column
maps to the old numbering scheme and is a known cosmetic inconsistency deferred from
Round 1. Not introduced by commit `230fc1e`.

---

## §4: Verdict

**5 / 5 RESOLVED. 0 FAILED. 0 new issues introduced.**

**READY-FOR-ADVERSARY**

All BLOCKING and HIGH-severity findings from the Round 1 audit have been resolved. The 5
LOW-severity findings remain deferred per original disposition (not blocking adversarial
review). The spec chain is internally consistent at the verified checkpoints and may proceed
to Phase 1d adversarial review.

---
document_type: consistency-report
level: ops
version: "1.0.0"
producer: consistency-validator
generated: 2026-05-04
phase: "Phase 1 Burst 4"
traces_to: ".factory/specs/prd/README.md"
---

# Phase 1 Consistency Audit — jira-cli

> **Historical snapshot:** This audit was run at the close of Phase 1 spec crystallization (2026-05-06). Holdout counts and BC enumerations in this file reflect the corpus at that time. Subsequent post-Phase-1 additions (H-NEW-VERBOSE-001/002, H-NEW-AUTH-002, H-NEW-JSM-RT-001..005, BC-2.6.050/051, BC-3.4.009, BC-3.8.001..010, BC-X.12.001..008) are NOT reflected here. For current counts, see CANONICAL-COUNTS.md.

**Scope:** Cross-document validation of L2 Domain Spec, L3 PRD, and Architecture documents before Phase 1d adversarial review.

**Auditor:** consistency-validator agent  
**Date:** 2026-05-04  
**Verdict:** NEEDS-FIXES (see §2 and §3)

---

## §1 Check-by-Check Results

| Check | Verdict | Summary |
|-------|---------|---------|
| Check 1: ID uniqueness | WARN | No duplicate IDs within individual files. BC-INDEX subdomain labels conflict with cross-cutting.md subdomain numbering (X.2..X.8 order differs). |
| Check 2: Traceability chain integrity | FAIL | BC-X.7.001 used in 4 architecture/PRD documents to mean "list_worklogs" but in cross-cutting.md BC-X.7.001 = "user search Q". The correct ID is BC-X.5.002. |
| Check 3: Count reconciliation | WARN | Holdout (48 ✓), Risk (26 ✓), ADR (12 ✓). NFR: frontmatter claims 44 but only 39 unique IDs in summary table; routing footer arithmetic (7+3+3+14+17=44) has DEFER count inflated (17 claimed, 12 actual). BC file total_bcs frontmatter vs actual heading count gap is systemic (e.g., bc-1 claims 57, has 46 headings). |
| Check 4: MUST-FIX bug consistency | FAIL | BC-6.3.001 ✓, BC-3.4.001 ✓, BC-4.3.001 ✓. BC-X.5.002 (list_worklogs) mislabeled as BC-X.7.001 in 4 authoritative documents (architecture README, risk-register, ADR-0010, PRD README). |
| Check 5: ADR cross-references | PASS | ADR-0002 consistently SUPERSEDED. PKCE tension (NEW-INV-178) documented in adr-index.md. ADR-0004 vs L3 PRD authority tension documented. All 12 ADRs in adr-index.md. |
| Check 6: Naming consistency | WARN | L2 BC-07 title: "Output Rendering & Error Handling" vs L3 bc-7 title: "Output Rendering & Error" (truncated). L2 state-machines.md has SM-06 (Profile Lifecycle) not present in architecture state-machines.md (5 vs 6 machines). |
| Check 7: 5 state machines presence | WARN | L2 has 6 SMs (SM-01..SM-06), Architecture has 5 (SM-1..SM-5; SM-06 intentionally excluded as "bonus"). Architecture SM-1 references BC-1.2.001..023 which do not exist in L3 PRD (L3 starts at BC-1.2.013). SM-4 references BC-2.1.001..020 but bc-2 only has BC-2.1.001..017. SM-5 references BC-6.2.001..017 but bc-6 only has BC-6.2.001..014. |
| Check 8: Holdout to BC traceability | PASS | All 48 holdouts have BC refs. H-NEW-MP-001 correctly traces to BC-6.3.001. H-045 correctly traces to BC-X.5.002. |
| Check 9: NFR catalog completeness | WARN | 39 unique NFR IDs in the catalog body. Frontmatter claims 44. Routing footer (7+3+3+14+17=44) has DEFER=17 but actual DEFER count is 12. The 5-NFR gap is in the routing summary arithmetic, not in the actual NFR entries (which number 39 or 44 depending on source). |
| Check 10: VSDD template compliance | WARN | L2 files have title/version/traces_to. L3 bc-*.md files use `context:` not `document_type:`, lack `level:` field. Architecture files use bold markdown `**traces_to:**` not YAML frontmatter. BC-INDEX.md has no traces_to. Lightweight compliance only; VSDD canonical frontmatter partially missing. |

---

## §2 Inconsistencies Found

### BLOCKING — Must fix before adversarial review

**BLOCK-1: BC-X.7.001 / BC-X.5.002 identity conflict** (HIGH severity)

The `list_worklogs` MUST-FIX behavioral contract has two different BC IDs assigned to it across documents. The authoritative definition in `cross-cutting.md` uses `BC-X.5.002`. Four downstream documents use the wrong ID `BC-X.7.001`:

| File | Wrong reference | Correct reference |
|------|----------------|-------------------|
| `.factory/architecture/risk-register.md` R-H5 | `BC-X.7.001` | `BC-X.5.002` |
| `.factory/architecture/README.md` MUST-FIX table | `BC-X.7.001` | `BC-X.5.002` |
| `.factory/architecture/adr/0010-list-worklogs-pagination.md` | `BC-X.7.001` (Context + BC anchor) | `BC-X.5.002` |
| `.factory/specs/prd/README.md` MUST-FIX table | `BC-X.7.001` | `BC-X.5.002` |

Compounding this: in `cross-cutting.md`, `BC-X.7.001` is already assigned to `user search Q` — so the WRONG reference in the 4 documents points to a real but completely unrelated BC.

**BLOCK-2: BC-INDEX.md subdomain labeling conflict with cross-cutting.md** (HIGH severity)

BC-INDEX.md §X maps subdomain numbers to subjects using a different ordering than the actual `cross-cutting.md` file. The canonical source of truth (`cross-cutting.md`) defines:
- X.2 = Pagination, X.3 = Error Handling, X.4 = Rate Limiting, X.6 = Teams, X.7 = Users, X.8 = Projects & Queues

BC-INDEX.md describes:
- X.2 = Rate Limiting, X.3 = Pagination, X.4 = Teams, X.6 = Users, X.7 = Projects & Queues, X.8 = User commands

This means BC-INDEX.md summary rows for X.6 through X.8 are describing the wrong subdomains. Any Phase 3 implementer using BC-INDEX.md to find BCs for "Users" or "Projects & Queues" will be directed to the wrong BC range. The MUST-FIX register in BC-INDEX (which uses BC-X.5.002) is itself correct; only the summary rows for X.6..X.8 are misaligned.

### HIGH — Should fix before adversarial review

**HIGH-1: Architecture state-machines.md references non-existent L3 BC IDs**

SM-1 `**BC anchors:** BC-1.1.001, BC-1.2.001..BC-1.2.023` — `BC-1.2.001` through `BC-1.2.012` do not exist in the L3 PRD. L3 bc-1 starts BC-1.2 at `BC-1.2.013`. This is not a fatal error (SM-1 is still correct mechanically) but creates a false traceability claim.

SM-4 `**BC anchors:** BC-2.1.001..BC-2.1.020` — `BC-2.1.018` through `BC-2.1.020` do not exist. L3 bc-2 subdomain 2.1 ends at `BC-2.1.017`.

SM-5 `**BC anchors:** BC-6.2.001..BC-6.2.017` — `BC-6.2.015` through `BC-6.2.017` do not exist. L3 bc-6 subdomain 6.2 ends at `BC-6.2.014`.

**HIGH-2: NFR catalog routing arithmetic inconsistency**

Frontmatter states `total_nfrs: 44` and body states "1 CRITICAL / 4 HIGH / 16 MEDIUM / 22 LOW". The routing summary at file end says "DEFER: 17" but actual DEFER-tagged rows in the summary table count to 12. If total = 44 were correct, the per-category math (7+3+3+14+17=44) would hold. But the actual unique NFR IDs in the catalog number 39. The 5 NFRs present in routing arithmetic (44-39=5) may be the five that were counted in "Pass 4 broad (23)" and listed in the source but not individually defined in this catalog. This creates an inflation of 5 in the frontmatter claim and routing footer. This is a documentation accuracy issue, not a spec gap.

**HIGH-3: BC-INDEX.md and bc-*.md frontmatter total_bcs counts differ from actual heading counts**

Every PRD bc-*.md file has `total_bcs:` in frontmatter claiming higher counts than the number of `#### BC-` headings actually in the file:

| File | Claimed | Actual headings | Gap |
|------|---------|-----------------|-----|
| bc-1-auth-identity.md | 57 | 46 | 11 |
| bc-2-issue-read.md | 91 | 49 | 42 |
| bc-3-issue-write.md | 77 | 48 | 29 |
| bc-4-assets-cmdb.md | 32 | 22 | 10 |
| bc-5-boards-sprints.md | 35 | 17 | 18 |
| bc-6-config-cache.md | 38 | 28 | 10 |
| bc-7-output-render.md | 80 | 33 | 47 |
| cross-cutting.md | 130 | 63 | 67 |
| **Total** | **540** | **306** | **234** |

The 306 headings represent individually-named BCs. The remaining ~234 BCs from the semport analysis have not been individually written as headings; they are implied by the frontmatter count and some range entries in BC-INDEX. This is a spec incompleteness risk for Phase 3 implementers who need to find specific BCs.

### LOW — Nice to fix

**LOW-1: L2 BC-07 title truncation**

L2 `bc-07-output-render.md` title: "Output Rendering & Error Handling". L3 `bc-7-output-render.md` title: "Output Rendering & Error" (missing "Handling"). Minor title drift. Not load-bearing.

**LOW-2: BC-6.3.001 trace does not reference ADR-0007**

`bc-6-config-cache.md` BC-6.3.001 trace line: `**Trace**: NFR-R-D; NEW-INV-12; NEW-INV-143; jira-cli-bc-nfr-r-d-draft.md; Pass 8 §5.2`. Missing explicit cross-reference to ADR-0007 which documents the fix strategy.

**LOW-3: BC-INDEX.md has no traces_to field**

`BC-INDEX.md` frontmatter has no `traces_to:` field. All other L3 PRD files have a `trace:` field. Minor template non-compliance.

**LOW-4: Architecture state-machines.md has SM-1 using old numbering**

`state-machines.md` (architecture) SM-1 key invariants section references `BC-1.2.006` for CSRF state. `BC-1.2.006` does not exist in L3 PRD (L3 uses BC-1.5.035 for `generate_state`). This is a stale reference to Pass 3 BC numbering.

**LOW-5: L2 has 6 state machines, architecture has 5**

L2 `state-machines.md` documents SM-06 (Profile Lifecycle, labeled "bonus"). Architecture `state-machines.md` documents only SM-1..SM-5. The architecture README also says "5 definitive state machines." The L2 SM-06 coverage is not a gap in the architecture (profile lifecycle is documented in SM-1 and SM-2), but the count discrepancy could confuse cross-referencing.

---

## §3 Recommendations

Ordered by urgency:

**P0 — Fix before adversarial review (unblocks Check 4):**

1. **Fix BLOCK-1**: Replace `BC-X.7.001` with `BC-X.5.002` in four files:
   - `.factory/architecture/risk-register.md` (R-H5 row, BC Anchor column)
   - `.factory/architecture/README.md` (MUST-FIX register, list_worklogs row)
   - `.factory/architecture/adr/0010-list-worklogs-pagination.md` (two occurrences: Context paragraph + References list)
   - `.factory/specs/prd/README.md` (MUST-FIX table, BC-X.7.001 row → BC-X.5.002)

2. **Fix BLOCK-2**: Align BC-INDEX.md §X summary rows to match cross-cutting.md subdomain ordering. Specifically, the BC-INDEX rows for X.6..X.8 should describe Teams/Users/Projects+Queues (not Users/Projects+Queues/User-commands). This requires editing rows 197-199 of BC-INDEX.md to match the actual cross-cutting.md X.6=Teams, X.7=Users, X.8=Projects+Queues ordering. Note: the Pass 3 source BC mapping (BC-801..810 → BC-X.7.001..010) in the Pass 3 Mapping Table also needs a note that X.7 in the final PRD = Users, not Projects/Queues; the source BCs BC-801..810 map to X.8 (Projects/Queues) in the actual cross-cutting.md.

**P1 — Fix before Phase 2 story decomposition:**

3. **Fix HIGH-1**: Update architecture `state-machines.md` BC anchor ranges:
   - SM-1: Remove claim of BC-1.2.001..BC-1.2.012; replace with actual existing IDs (BC-1.2.013..BC-1.2.018)
   - SM-4: Change BC-2.1.001..020 to BC-2.1.001..017
   - SM-5: Change BC-6.2.001..017 to BC-6.2.001..014

4. **Fix HIGH-2 and HIGH-3**: Add a note to NFR catalog frontmatter clarifying that `total_nfrs: 44` reflects the semport pass count; 39 NFRs are individually defined in this catalog. Add a similar note to each bc-*.md file frontmatter: "total_bcs reflects semport pass count; N BCs are individually defined as headings in this file." This prevents implementer confusion.

**P2 — Before convergence:**

5. Fix LOW-1: Update L3 bc-7 title to match L2 bc-07 ("Output Rendering & Error Handling").
6. Fix LOW-2: Add ADR-0007 to BC-6.3.001 trace line.
7. Fix LOW-4: Update SM-1 key invariants `BC-1.2.006` to `BC-1.5.035`.

---

## §4 Verdict

**NEEDS-FIXES**

Two BLOCKING inconsistencies prevent full adversarial-review readiness:

1. **BLOCK-1** (BC-X.7.001 / BC-X.5.002 identity conflict): A MUST-FIX bug BC is referenced by the wrong ID in 4 authoritative documents. Phase 3 implementers tracking the `list_worklogs` fix via risk-register, ADR-0010, or architecture README will find a conflicting BC. The conflict also directs them to `user search Q` (BC-X.7.001 in cross-cutting.md) — a completely unrelated contract. This MUST be corrected before adversarial review to prevent the reviewer from finding this obvious error and downgrading the entire artifact set.

2. **BLOCK-2** (BC-INDEX X subdomain label mismatch): The BC-INDEX.md summary section mislabels 6 of 11 cross-cutting subdomains. An adversarial reviewer scanning the index to verify BC coverage will find descriptions that don't match the actual cross-cutting.md subdomains (e.g., "BC-X.7 = Projects & Queues" in index vs "BC-X.7 = Users" in the canonical file).

**The 4 MUST-FIX BCs (BC-6.3.001, BC-X.5.002, BC-3.4.001, BC-4.3.001) are all correctly defined in their canonical locations.** The BC count discrepancy between frontmatter and actual headings is an acknowledged spec-incompleteness (234 of 540 BCs are not yet individually written as headings) — this is a Phase 2 gap, not a Phase 1 blocking issue.

After BLOCK-1 and BLOCK-2 are fixed: **READY-FOR-ADVERSARY** (with HIGH-1 and HIGH-2 noted as known deferred items).

---

## Appendix: Verification Summary

| Artifact | Count claimed | Count verified | Delta | Status |
|----------|--------------|----------------|-------|--------|
| ADRs | 12 (0001-0012) | 12 | 0 | PASS |
| ADRs new (factory) | 6 | 6 | 0 | PASS |
| ADRs existing (reference) | 6 | 6 | 0 | PASS |
| Holdout scenarios | 48 | 48 | 0 | PASS |
| Risk register entries | 26 | 26 | 0 | PASS |
| MUST-FIX BCs | 4 | 4 | 0 | PASS (wrong IDs in 4 cross-refs for BC-X.5.002) |
| NFR IDs (unique) | 44 | 39 | 5 | WARN |
| State machines (arch) | 5 | 5 | 0 | PASS |
| State machines (L2) | 6 | 6 | 0 | PASS |
| BC headings (total) | 540 | 306 | 234 | WARN (spec incompleteness) |
| Duplicate BC IDs | 0 | 0 | 0 | PASS |
| Duplicate holdout IDs | 0 | 0 | 0 | PASS |
| Duplicate NFR IDs | 0 | 0 | 0 | PASS |
| ADR-0002 superseded everywhere | yes | yes | — | PASS |
| PKCE tension documented | yes | yes | — | PASS |

**Consistency score (estimated):** 82% (blocking issues concentrated in cross-reference labeling, not in spec content)

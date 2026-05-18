---
document_type: adversarial-pass
phase: F1d
pass: 06
cycle: 3-feature-jsm-request-types-288
target: "issue-288 F2 spec delta — pass-05 sweep + fresh review"
model: "Opus 4.7 (1M)"
timestamp: 2026-05-18
verdict: FINDINGS-PRESENT
counts:
  blocking: 0
  concern: 2
  nit: 3
counter_status: "0/3 (reset by CONCERN findings)"
pass_05_disposition: "5 ADDRESSED / 0 PARTIAL / 0 NOT / 0 REGRESSED"
---

# F1d Pass 06 — Issue #288 — FINDINGS-PRESENT

**Target**: F2 spec delta for issue #288 (JSM request type support) — pass-05 sweep + fresh review
**Verdict**: FINDINGS-PRESENT — 0 BLOCKING, 2 CONCERN, 3 NIT. Counter: 0/3 (reset by CONCERN findings).

Product-owner conducted frontmatter sweep after adversary identified sibling-field drift: bc-3
trace field updated (BC-3.8.001..010), ADR-0014 related[] corrected (R-H288-2 → R-M288-1),
verification-delta BC mis-anchor repaired (BC-3.8.006 → BC-3.8.009 at lines 139 and 387).
DRIFT-008 widened to include frontmatter sibling-field enumeration targets. DRIFT-010 severity
upgraded LOW → MEDIUM. Pass-07 pending.

---

## Pass-05 Disposition Summary

| Finding | ID | Verdict | Notes |
|---------|----|---------|-------|
| BC-X.12.001 intra-BC self-contradiction (Behavior "ID, Name, Description") | F38 | ADDRESSED | Behavior updated to "Name, Description"; ID relegated to json-only note |
| CANONICAL-COUNTS risk total 28 vs actual 36 | F39 | ADDRESSED | Risks updated to 36; severity breakdown 1C/7H/11M/17L |
| bc-3-issue-write.md footer consolidation | F40 | ADDRESSED | Footer updated to +10 BCs with F2/F1d provenance inline |
| DRIFT-008 scope extension (intra-BC + non-BC totals) | F41 | ADDRESSED | DRIFT-008 entry widened in STATE.md |
| DRIFT-010 added (risk-register header 34 vs table 36) | F42 | ADDRESSED | DRIFT-010 entry created in STATE.md |

5 ADDRESSED / 0 PARTIAL / 0 NOT / 0 REGRESSED.

---

## Summary Table — Net-New Findings (Pass 06)

| ID | Severity | Area | Title |
|----|----------|------|-------|
| F43 | CONCERN | bc-3-issue-write.md frontmatter | `trace:` field cites BC-3.8.001..009 — stale after F1d added BC-3.8.010 |
| F44 | CONCERN | ADR-0014 frontmatter | `related[]` cites non-existent R-H288-2; intended R-M288-1 |
| F45 | NIT | Process gap | Frontmatter sibling-field sweep not covered by any current scripts/check |
| F46 | NIT | DRIFT-010 | Severity LOW understates impact — per BC-title/subsystem-label-sync rubric should be MEDIUM |
| F47 | NIT | verification-delta | BC mis-anchor: cites BC-3.8.006 (description/ADF) for raiseOnBehalfOf semantics — should be BC-3.8.009 |

---

## Detailed Findings

### F43 — CONCERN — bc-3 frontmatter trace stale

**Location**: `.factory/specs/prd/bc-3-issue-write.md` frontmatter, `trace:` field

**Observation**: The `trace:` field enumerates BC-3.8.001 through BC-3.8.009. F1d pass-01
added BC-3.8.010 (raiseOnBehalfOf write-only guard) as the tenth BC in this section. The
frontmatter was not updated at that time. The body correctly documents all ten BCs; only the
frontmatter enumeration is stale.

**Impact**: Tooling that reads `trace:` to build coverage maps will under-count BC-3.8 by one
(9 of 10). If scripts/check-spec-counts.sh or future sweep tooling is extended to validate
frontmatter `trace:` fields against body section contents (cf. DRIFT-008 scope), this will
surface as a validation failure.

**Remediation**: Update `trace:` to `BC-3.8.001..010`.

---

### F44 — CONCERN — ADR-0014 related[] cites non-existent R-H288-2

**Location**: `.factory/specs/architecture/ADR-0014.md` frontmatter, `related:` array

**Observation**: `related:` contains `R-H288-2`. Per `.factory/specs/risk-register.md` §ISSUE
#288 RISKS, the two risks added for issue #288 are:
- R-H288-1 — HIGH — (scope/implementation risk)
- R-M288-1 — MEDIUM — (API contract / Atlassian compatibility risk)

There is no `R-H288-2` in the register. The intended citation was R-M288-1 (the MEDIUM-severity
#288 risk, which covers the Atlassian API surface that ADR-0014 directly addresses).

**Impact**: A `related:` field citing a non-existent risk ID is a dangling reference. If any
tooling resolves `related:` links to validate cross-document consistency (the widened DRIFT-008
scope includes frontmatter fields enumerating other artifacts), this will fail. More critically,
the audit trail connecting ADR-0014 to its associated risk is broken.

**Remediation**: Change `R-H288-2` → `R-M288-1` in ADR-0014 frontmatter.

---

### F45 — NIT — Process gap: frontmatter sibling-field sweep not in scripts/check

**Location**: `scripts/` directory; DRIFT-008 entry

**Observation**: Six passes of F1d on issue #288 have now surfaced drift in frontmatter sibling
fields: `trace:` (F43, this pass), `related:` (F44, this pass). Previous passes surfaced cross-doc
BC arithmetic, prose coherence, README index family, and intra-BC field consistency. None of the
existing `scripts/check-*.sh` tools validate that frontmatter fields enumerating other artifacts
(e.g., `trace:`, `related:`, `sections:`, `traces_to:`, `bc_anchors:`, `holdout_anchors:`) cite
symbols that actually exist.

**Impact**: Process gap. Drift of this class will recur on every future pass until a sweep tool
is added. Low urgency (not blocking #288); medium recurrence risk.

**Remediation**: Widen DRIFT-008 to include frontmatter sibling-field sweep as a target
category. Add to v0.6 scope.

---

### F46 — NIT — DRIFT-010 severity LOW understates impact

**Location**: STATE.md DRIFT-010 entry

**Observation**: DRIFT-010 records that `risk-register.md` header (line 5) says "Total risks:
34" while the document's own Risk Summary table (line 119) says 36. The current severity is LOW.

Per the established BC-Title/Subsystem-Label Sync rubric applied in prior passes: a
document-header count contradicting its own body table in a security-relevant document used for
release gating is a MEDIUM-minimum finding. The risk-register is consulted at Phase 4 holdout
evaluation and Phase 6 formal hardening. A self-contradicting header undermines gate verdicts.

**Impact**: LOW severity designation understates the release-gating risk of a self-contradicting
security-relevant document. Should be reclassified MEDIUM.

**Remediation**: Upgrade DRIFT-010 severity LOW → MEDIUM.

---

### F47 — NIT — verification-delta BC mis-anchor for raiseOnBehalfOf

**Location**: `.factory/cycles/cycle-001/adversarial-reviews/issue-288-jsm-request-types/verification-delta.md`, lines 139 and 387

**Observation**: The verification-delta document cites BC-3.8.006 as the behavioral contract
anchor for `raiseOnBehalfOf` semantics. BC-3.8.006 governs description/ADF body handling for
issue create/edit — unrelated to `raiseOnBehalfOf`. The correct anchor is BC-3.8.009, which
was added by F1d pass-01 specifically for the `raiseOnBehalfOf` write-only guard.

**Impact**: Traceability audit following the verification-delta to BC-3.8.006 will land on the
wrong BC. The off-by-three error (006 vs 009) suggests a copy-paste from an earlier raiseOnBehalf
citation in a different section context.

**Remediation**: At lines 139 and 387, replace `BC-3.8.006` with `BC-3.8.009`.

---

## Per-Mandate Audit

| Mandate | Verdict | Notes |
|---------|---------|-------|
| All BCs traceable to PRD requirements | PASS | BC-3.8.001..010 traceability intact |
| ADR/BC consistency | FAIL | ADR-0014 `related:` cites non-existent R-H288-2 (F44) |
| Frontmatter ↔ body coherence | FAIL | bc-3 `trace:` field stale at BC-3.8.009 (F43) |
| Verification-delta BC cross-references | FAIL | raiseOnBehalfOf mis-anchored to BC-3.8.006 instead of BC-3.8.009 (F47) |
| No orphan holdout references | PASS | H-NEW-JSM-RT-001..005 all correctly registered |
| Risk register cross-references | CONDITIONAL PASS | R-H288-2 non-existent in ADR-0014 (F44); R-H288-1 and R-M288-1 correctly registered |

---

## Novelty Assessment

**MEDIUM novelty.** Three sub-patterns surfaced for the first time this pass:

1. **Frontmatter `trace:` drift** (F43): frontmatter enumeration of BCs not updated when a new
   BC was added mid-review. This is structurally distinct from the body-count drift caught in
   passes 01-03 — it is a machine-readable enumeration field, making it more amenable to
   automated validation.

2. **Frontmatter `related:` mis-anchor to non-existent symbol** (F44): cross-document reference
   citing a risk ID that does not exist. Novel because it is an existential gap (the cited symbol
   does not exist at all), not a stale-count drift (the symbol exists but with wrong value).

3. **Verification-delta BC off-by-three** (F47): traceability document anchors an AC to a BC
   that is three IDs away from the correct one, sharing a BC section prefix. Consistent with the
   copy-paste-from-adjacent-BC error pattern; new location class (verification-delta, not PRD or
   story spec).

---

## Top 3 Findings

1. **F44** (CONCERN) — ADR-0014 `related[]` cites non-existent R-H288-2; this breaks the
   release-gating risk traceability from the ADR to its associated MEDIUM risk.

2. **F43** (CONCERN) — bc-3 frontmatter `trace:` field lists 9 BCs (001..009) after F1d added
   a tenth (BC-3.8.010). Downstream coverage tooling will under-count.

3. **F47** (NIT) — verification-delta cites BC-3.8.006 (description/ADF) for raiseOnBehalfOf;
   correct anchor is BC-3.8.009. Off-by-three in the same section; traceability audit lands on
   wrong BC.

---

## Counter Status

Counter: **0/3** — unchanged from pass-05.

CONCERN plateau: 2 CONCERN findings for 3 consecutive passes (passes 04, 05, 06), but the root
causes shift each pass:
- Pass 04: README Supplement Index self-contradiction + Open Questions ordering
- Pass 05: BC-X.12.001 intra-BC self-contradiction + CANONICAL-COUNTS risk total gap
- Pass 06: bc-3 frontmatter `trace:` stale + ADR-0014 `related:` non-existent symbol

The plateau reflects a pattern of fresh-context audit compounding value — each pass surfaces a
different class of spec maintenance drift. The frontmatter sibling-field class (F43/F44) is new
and amenable to automated sweeping. Expect break to CLEAN after frontmatter-sweep stabilization.

Pass-07 pending.

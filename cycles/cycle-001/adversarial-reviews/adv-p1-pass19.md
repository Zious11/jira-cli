---
document_type: adversarial-review
phase: phase-1-spec-adversarial
pass: 19
producer: adversary
timestamp: 2026-05-04T00:00:00Z
fresh_context: true
inputs_reviewed:
  - .factory/specs/domain-spec/*.md
  - .factory/specs/prd/*.md
  - .factory/architecture/**/*.md
finding_count: 5
severity_distribution: "1C/1H/3M/0L"
final_assessment: "SUBSTANTIVE"
---

# Phase 1 Spec Adversarial Review — Pass 19

## Final Assessment
SUBSTANTIVE

Counter regress: 0/3 → 0/3. Pass 18 fixes verified. Five new findings on rotated lenses 1, 3, 5.

## Findings

### ADV-P19-001: SM-5 Cache Lifecycle anchors BC-X.8.001 (HTTP) instead of BC-X.8.003 (cache)
- Severity: CRITICAL
- Confidence: HIGH
- Lens: 5 (state-machine ↔ BC alignment)
- Locations:
  - `.factory/architecture/state-machines.md:236` — `**BC anchors:** BC-6.2.001..BC-6.2.014, BC-X.8.001`
  - `.factory/specs/prd/cross-cutting.md:458` — BC-X.8.001 = `project_exists(key)` HTTP check (no cache)
  - `.factory/specs/prd/cross-cutting.md:474` — BC-X.8.003 = `get_or_fetch_project_meta(client, key)` 7d TTL cache
- Evidence: SM-5 ("Cache Lifecycle") declares BC-X.8.001 as a cross-cutting anchor. BC-X.8.001 is the `project_exists` HTTP existence check with no cache semantics. The intended anchor is BC-X.8.003 (the cache BC).
- Expected: SM-5's cross-cutting BC anchor should point at BC-X.8.003.
- Suggested fix: Change `architecture/state-machines.md:236` from `BC-6.2.001..BC-6.2.014, BC-X.8.001` to `BC-6.2.001..BC-6.2.015, BC-X.8.003`.
- Tag: [content-defect]
- Routing: architect

### ADV-P19-002: INV-CACHE-003 cache-count drift — "7 cache categories" contradicts canonical "6"
- Severity: HIGH
- Confidence: HIGH
- Lens: 3 (cache 6-instance audit) + 1 (cross-doc severity reconciliation)
- Locations:
  - `.factory/specs/domain-spec/bc-06-config-cache.md:108` — INV-CACHE-003: `"all 7 cache categories"`
  - `.factory/architecture/state-machines.md:269` — `### Cache types (6 distinct)` with explicit table
  - `.factory/specs/domain-spec/state-machines.md:300` — "6 distinct cache categories"
  - `CLAUDE.md` cache.rs gotcha — enumerates 6 cache types
- Evidence: L2 INV-CACHE-003 says "7"; every other authoritative location says 6.
- Expected: All cache-count refs should agree on "6 distinct cache categories".
- Suggested fix: Update `.factory/specs/domain-spec/bc-06-config-cache.md:108` from "all 7 cache categories" to "all 6 cache categories".
- Tag: [content-defect]
- Routing: architect

### ADV-P19-003: SM-5 BC-anchor range omits BC-6.2.015 (cache profile-fence convention)
- Severity: MEDIUM
- Confidence: HIGH
- Lens: 5 (state-machine ↔ BC alignment) + 3 (cache audit)
- Locations:
  - `.factory/architecture/state-machines.md:236` — anchor range stops at BC-6.2.014
  - `.factory/specs/prd/BC-INDEX.md:425` — BC-6.2.015 (added at ADV-P1-019, profile-fence soft convention)
  - `.factory/specs/prd/bc-6-config-cache.md:281-289` — BC-6.2.015 fully bodied
- Evidence: SM-5 prose at architecture/state-machines.md:284-286 narrates BC-6.2.015 but the anchor range was not updated.
- Expected: SM-5 BC-anchor range should be `BC-6.2.001..BC-6.2.015`.
- Suggested fix: Combined with ADV-P19-001: update `architecture/state-machines.md:236` to `BC-6.2.001..BC-6.2.015, BC-X.8.003`.
- Tag: [content-defect]
- Routing: architect

### ADV-P19-004: H-027 holdout missing BC-X.4.009 forward-state cross-reference
- Severity: MEDIUM
- Confidence: HIGH
- Lens: 1 (bidirectional holdout ↔ BC traceability)
- Locations:
  - `.factory/specs/prd/holdout-scenarios.md:274` — H-027 `BC refs: BC-X.4.002` only
  - `.factory/specs/prd/cross-cutting.md:267` — BC-X.4.009 explicitly names H-027 (forward direction)
- Evidence: BC-X.4.009 ↔ H-027 round-trip broken — BC names holdout but holdout doesn't name BC.
- Expected: H-027 should reference both BC-X.4.002 (current) and BC-X.4.009 (future MUST-FAIL when fix lands).
- Suggested fix: Update `holdout-scenarios.md:274` to `BC refs: BC-X.4.002 (current behavior pinned); BC-X.4.009 (future MUST-FAIL target when fix lands)`.
- Tag: [content-defect]
- Routing: product-owner

### ADV-P19-005: MUST-FIX BC ↔ Holdout reverse-trace asymmetry (3 of 4 MUST-FIX BCs lack holdout cross-ref)
- Severity: MEDIUM
- Confidence: HIGH
- Lens: 1 (bidirectional holdout ↔ BC traceability)
- Locations:
  - `.factory/specs/prd/bc-6-config-cache.md:353` — BC-6.3.001 has `**Holdout:** H-NEW-MP-001` (precedent)
  - `.factory/specs/prd/bc-3-issue-write.md:301-314` — BC-3.4.001 missing H-046 cross-ref
  - `.factory/specs/prd/cross-cutting.md:283-295` — BC-X.5.002 missing H-045 cross-ref
  - `.factory/specs/prd/bc-4-assets-cmdb.md:179-196` — BC-4.3.001 missing H-036 cross-ref
- Evidence: 4 MUST-FIX BCs are anchored by holdouts; only BC-6.3.001 cites its holdout in body. Pattern: precedent set late, not retrofitted to legacy 3.
- Expected: Each MUST-FIX BC should cross-reference its anchoring holdout under a `**Holdout:**` line.
- Suggested fix:
  - bc-3-issue-write.md BC-3.4.001: add `**Holdout:** H-046 — handle_open uses instance_url() for OAuth profiles.`
  - cross-cutting.md BC-X.5.002: add `**Holdout:** H-045 — list_worklogs pagination — all pages returned.`
  - bc-4-assets-cmdb.md BC-4.3.001: add `**Holdout:** H-036 — Multi-workspace asset HashMap composite key.`
- Tag: [content-defect]
- Routing: product-owner

## Lens Coverage Summary
- Lens 1 (bidirectional holdout ↔ BC traceability): 2 findings (ADV-P19-004, ADV-P19-005)
- Lens 2 (risk ↔ error ↔ NFR reconciliation): 0 net findings
- Lens 3 (cache 6-instance audit): 2 findings (ADV-P19-002, ADV-P19-003)
- Lens 4 (frontmatter compliance): 0 findings — heterogeneous styles are established convention
- Lens 5 (state-machine ↔ BC alignment): 2 findings (ADV-P19-001, ADV-P19-003)
- Lens 6 (component-graph completeness): 0 findings
- Lens 7 (ADR rationale completeness): 0 findings

## Verdict — SUBSTANTIVE (5 findings)

Five findings cluster around **partial-fix propagation pattern**: when one artifact is updated (BC-6.2.015 added, BC-6.3.001 introduced holdout cross-ref pattern, cache-type count fixed in 3 of 4 places), the change does not propagate to all sibling locations. CRITICAL ADV-P19-001 blocks convergence on its own.

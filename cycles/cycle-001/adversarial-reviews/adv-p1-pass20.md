---
document_type: adversarial-review
phase: phase-1-spec-adversarial
pass: 20
producer: adversary
timestamp: 2026-05-04T00:00:00Z
fresh_context: true
inputs_reviewed:
  - .factory/specs/domain-spec/*.md
  - .factory/specs/prd/*.md
  - .factory/architecture/**/*.md
  - src/observability.rs
  - src/cli/issue/workflow.rs
  - src/cli/issue/list.rs
  - src/api/jira/worklogs.rs
  - Cargo.toml
finding_count: 3
severity_distribution: "0C/1H/1M/1L"
final_assessment: "SUBSTANTIVE"
---

# Phase 1 Spec Adversarial Review — Pass 20

## Final Assessment
SUBSTANTIVE

Counter regress: 0/3 → 0/3 (Pass 19 fixes verified; new findings in previously-under-examined edge-case-catalog.md). All Pass 19 fixes verified clean.

## Findings

### ADV-P20-001: edge-case-catalog G-EO1 contradicts Cargo.toml and NFR-O-A on tracing dependency presence
- Severity: HIGH
- Confidence: HIGH
- Lens: 6 (telemetry/observability NFR completeness) + 1 (cross-doc reconciliation)
- Locations:
  - `.factory/specs/prd/edge-case-catalog.md:305` — G-EO1: "no tracing crate integration **despite dep present**"
  - `.factory/specs/prd/nfr-catalog.md:90` — NFR-O-A: "tracing is NOT currently a dependency"
  - `.factory/architecture/cross-cutting.md:186` — "tracing is NOT currently a dependency (verified Cargo.toml:14-37)"
  - `Cargo.toml:14-37` — no `tracing` and no `tracing-subscriber` in dependencies
- Evidence: Cargo.toml verified — neither `tracing` nor `tracing-subscriber` is present. Both NFR-O-A and architecture/cross-cutting.md correctly state the dep is NOT present. G-EO1 is the only doc claiming the dep is present.
- Expected: G-EO1 must agree: "tracing crate not present in Cargo.toml".
- Suggested fix: Update edge-case-catalog.md:305 from "despite dep present" to "; tracing crate not present in Cargo.toml".
- Tag: [content-defect]
- Routing: product-owner

### ADV-P20-002: edge-case-catalog G-EO1 says "3 sites"; architecture/cross-cutting.md §9 says "exactly 2 call sites"
- Severity: MEDIUM
- Confidence: HIGH
- Lens: 6 (telemetry/observability NFR completeness) + 1 (cross-doc reconciliation)
- Locations:
  - `.factory/specs/prd/edge-case-catalog.md:305` — "observability.rs is 39 LOC with one function at **3 sites**"
  - `.factory/architecture/cross-cutting.md:178-180` — "Used at exactly 2 call sites: cli/issue/format.rs:127, cli/issue/changelog.rs:276"
  - `src/observability.rs:1-40` — single `pub(crate)` function `log_parse_failure_once`
- Evidence: Architecture spec enumerates 2 call sites with file:line precision. G-EO1 claims 3.
- Expected: G-EO1 should say "2 sites".
- Suggested fix: Update edge-case-catalog.md:305 "at 3 sites" → "at 2 sites".
- Tag: [content-defect]
- Routing: product-owner

### ADV-P20-003: MUST-FIX edge-case entries lack holdout cross-references (partial-fix propagation gap)
- Severity: LOW
- Confidence: HIGH
- Lens: 5 (EC → BC anchor → test-fixture chain) + 1 (cross-doc consistency)
- Locations:
  - `.factory/specs/prd/edge-case-catalog.md:105-108` — EC-CFG-005: `Status: MUST-FIX (NFR-R-D) → BC-6.3.001` (no holdout ref; H-NEW-MP-001 anchors)
  - `.factory/specs/prd/edge-case-catalog.md:202-205` — EC-ASSET-006: `Status: MUST-FIX (NFR-R-E) → BC-4.3.001` (no holdout ref; H-036 anchors)
  - `.factory/specs/prd/edge-case-catalog.md:39-41` — EC-AUTH-002 canonical pattern: `Status: Covered by BC-1.1.006; holdout H-016`
- Evidence: Non-MUST-FIX ECs follow `Covered by BC-X.Y.Z; holdout H-NNN` pattern. MUST-FIX ECs (EC-CFG-005, EC-ASSET-006) drop the holdout cross-reference. Pass 19's BC↔Holdout fix didn't propagate to EC catalog.
- Expected: MUST-FIX ECs should include `; holdout H-NNN`.
- Suggested fix:
  - EC-CFG-005: append `; holdout H-NEW-MP-001` to Status line
  - EC-ASSET-006: append `; holdout H-036` to Status line
- Tag: [content-defect]
- Routing: product-owner

## Lens Coverage Summary
- Lens 1 (source-truth MUST-FIX): 0 findings — all 4 MUST-FIX bug citations verified against source HEAD
- Lens 2 (NFR coverage of risks): 0 findings
- Lens 3 (holdout MUST-PASS/MUST-FAIL classification): 0 findings
- Lens 4 (ADR Status field consistency): 0 findings
- Lens 5 (EC → BC → fixture chain): 1 finding (ADV-P20-003)
- Lens 6 (telemetry NFR completeness): 2 findings (ADV-P20-001, ADV-P20-002)
- Lens 7 (Pass 19 fix verification): 0 findings — all 5 Pass 19 fixes landed clean

## Pass 19 Fix Verification
- ADV-P19-001+003 (SM-5 anchor `BC-6.2.001..BC-6.2.015, BC-X.8.003`): **verified**
- ADV-P19-002 (INV-CACHE-003 "all 6 cache categories"): **verified**
- ADV-P19-004 (H-027 BC refs include BC-X.4.009): **verified**
- ADV-P19-005a (BC-3.4.001 Holdout: H-046 line): **verified**
- ADV-P19-005b (BC-X.5.002 Holdout: H-045 line): **verified**
- ADV-P19-005c (BC-4.3.001 Holdout: H-036 line): **verified**

## Verdict — SUBSTANTIVE (3 findings)

All Pass 19 fixes verified. Today's findings are localized to `.factory/specs/prd/edge-case-catalog.md` — a previously-under-examined artifact. Source-truth verification (Lens 1) confirmed all 4 MUST-FIX bug citations match HEAD. NFR ↔ risk coverage (Lens 2) is complete. Holdout classification (Lens 3) is correct. ADR Status fields (Lens 4) are consistent.

Trajectory: 30→15→9→5→10→5→4→3→4→0→2→0→3→0→2→0→3→0→3→5→3. Steady asymptotic regime around 0-5 findings/pass.

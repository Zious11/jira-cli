---
document_type: adversarial-review
phase: phase-1-spec-adversarial
pass: 23
producer: adversary
timestamp: 2026-05-04T00:00:00Z
fresh_context: true
inputs_reviewed:
  - .factory/specs/prd/*.md
  - .factory/specs/domain-spec/*.md
  - .factory/architecture/*.md
  - src/api/jira/*.rs, src/api/jsm/*.rs, src/api/assets/*.rs (Lens 1+6)
  - tests/*.rs (Lens 2 fixture existence)
finding_count: 5
severity_distribution: "0C/1H/3M/1L"
final_assessment: "SUBSTANTIVE"
---

# Phase 1 Spec Adversarial Review — Pass 23

## Final Assessment
SUBSTANTIVE

Counter regress: 0/3 → 0/3. Pass 22 fixes 5/5 verified at primary targets. 5 new findings: 1 Pass 22 propagation gap (process pattern recurrence), 4 pre-existing drifts.

## Pass 22 Fix Verification (5/5)
- ADV-P22-001 (H-044 BC-7.2.001..051): VERIFIED holdout-scenarios.md:436
- ADV-P22-002 (L2 51 BCs): VERIFIED bc-07-output-render.md:15,127
- ADV-P22-003 (component-graph 7-level): VERIFIED component-graph.md:133
- ADV-P22-004 (H-027 reframe parsing test): VERIFIED holdout-scenarios.md:269-274
- ADV-P22-005 (CANONICAL-COUNTS MEDIUM 15): VERIFIED CANONICAL-COUNTS.md:103

## Findings

### ADV-P23-001: bc-07-output-render.md L2 has 3 stale "6-level" references — P22 propagation gap (HIGH)
- Severity: HIGH
- Lens: 1 (P22 propagation downstream sweep) + L2↔L3 invariant convergence
- Locations:
  - bc-07-output-render.md L2 line 35 — "6-level precedence chain" enumerating REMOVED fields (errors.field.messages[], errorDescription)
  - line 72 — JrError ApiError "6-level extract_error_message chain"
  - line 115 — INV-OUT-014 with same removed-fields enumeration
- Evidence: ADV-P2-001 corrected the chain to 7 steps and removed `errors.field.messages[]` and `errorDescription` (per error-taxonomy.md:74-76). P22 fixed component-graph.md:133 but missed L2 at three sites. Drift not just numeric — enumerates removed fields.
- Suggested fix:
  - Line 35: replace chain with canonical 7-step list per error-taxonomy.md §2: (1) empty body literal, (2) non-UTF-8, (3) errorMessages[], (4) errors{}, (5) message, (6) errorMessage (singular, JSM), (7) raw body
  - Line 115: same 7-step replacement for INV-OUT-014
  - Line 72: "6-level" → "7-level"
- Tag: [content-defect]
- Routing: product-owner

### ADV-P23-002: API resource file count drift — claims 17, actual 18 (MEDIUM)
- Severity: MEDIUM
- Lens: 6 (CANONICAL-COUNTS internal)
- Locations:
  - CANONICAL-COUNTS.md:184 — "API resource files | 17"
  - adr-index.md:33 — "All 17 impl JiraClient resource files"
  - system-overview.md:40 — "L4 api resource impls (17 files)"
- Evidence: src/api/jira/mod.rs declares 11 (boards, fields, issues, links, projects, resolutions, sprints, statuses, teams, users, worklogs); jsm/mod.rs declares 2 (queues, servicedesks); assets/mod.rs declares 5 (linked, objects, schemas, tickets, workspace). Total = 18. resolutions.rs has real `impl JiraClient` block.
- Suggested fix: Update all three citations from 17 → 18.
- Tag: [content-defect]
- Routing: architect

### ADV-P23-003: H-017 cites non-existent test fixture `tests/h017_aql_clause` (MEDIUM)
- Severity: MEDIUM
- Lens: 2 (test fixture file existence)
- Locations: holdout-scenarios.md:184 — "Source: tests/h017_aql_clause"
- Evidence: No such file exists. Actual tests are inline at src/jql.rs:278-308 (build_asset_clause_*).
- Suggested fix: Replace with `src/jql.rs:278-308 (build_asset_clause_* unit tests)`.
- Tag: [content-defect]
- Routing: product-owner

### ADV-P23-004: Group 1 holdout header omits middle holdouts from range list (MEDIUM)
- Severity: MEDIUM
- Lens: 6 (bullet arithmetic)
- Locations: holdout-scenarios.md:27 — "Group 1: Auth & Profile Edge Cases (H-001..H-008, H-016, H-019, H-021..H-029)"
- Evidence: Group 1 spans H-001..H-029 sequentially. Header omits H-009..H-015, H-017, H-018, H-020. Also misnamed "Auth & Profile" since it contains cache, pagination, config-migration, JQL, duration holdouts.
- Suggested fix: Either (a) expand range to "H-001..H-029" and rename group to "Foundational / Mixed Edge Cases"; OR (b) split into multiple topical groups. Recommend (a) for minimal churn.
- Tag: [content-defect]
- Routing: product-owner

### ADV-P23-005: H-030 mis-categorized in "Group 2: Issue Read, JQL, and Filtering" (LOW)
- Severity: LOW
- Lens: 5 (holdout categorization)
- Locations: holdout-scenarios.md:297-304 — Group 2 header; H-030 at line 299
- Evidence: H-030 anchors BC-7.3.001 (Output Rendering & Error Display). Behavior is extract_error_message chain. Group label misanchors domain.
- Suggested fix: Refactor Group 2 header to "Issue Read, JQL, Filtering, and Error Extraction" — minimal churn. (Alternative: move H-030 to Group 5/Output but that's bigger refactor.)
- Tag: [content-defect]
- Routing: product-owner

## Observations
- OBS-001: [process-gap] P22 OBS-003 sweep recommendation was NOT enacted as a gate — same propagation pattern recurs in P23-001. Codify "count/chain-length change → grep sweep across L2/architecture/edge-case-catalog using literal old value" as a hard rule.
- OBS-002: holdout-scenarios.md total_holdouts:48 matches actual (H-001..H-047 + H-NEW-MP-001 = 48) ✓
- OBS-003: api/jira/resolutions.rs is real, not stub. 17→18 drift is just stale counts.
- OBS-004: NFR catalog 41-total reconciles. No new arithmetic drift.
- OBS-005: BC-7 file 80 reconciles: 5+51+9+12+3=80 ✓

## Lens Coverage Summary
- Lens 1 (P22 verification + propagation sweep): 1 finding (ADV-P23-001) — propagation pattern recurs
- Lens 2 (Test fixture existence): 1 finding (ADV-P23-003)
- Lens 3 (Code snippet validity): 0 findings
- Lens 4 (Terminology drift): 0 findings
- Lens 5 (Holdout categorization): 1 finding (ADV-P23-005)
- Lens 6 (List arithmetic): 2 findings (ADV-P23-002, ADV-P23-004)
- Lens 7 (L2↔L3 invariant): 1 finding subsumed in ADV-P23-001

## Verdict
SUBSTANTIVE (5 findings + 5 obs). Trajectory ...→3→4→5→5. Asymptotic regime. Same-pattern propagation gap (P23-001) confirms need for codified downstream-sweep rule per OBS-001.

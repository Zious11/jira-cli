---
document_type: adversarial-review
phase: phase-1-spec-adversarial
pass: 24
producer: adversary
timestamp: 2026-05-04T00:00:00Z
fresh_context: true
inputs_reviewed:
  - .factory/specs/prd/*.md
  - .factory/specs/domain-spec/*.md
  - .factory/architecture/*.md
  - src/cli/issue/list.rs, src/jql.rs, src/error.rs (Lens 2/3/4)
finding_count: 5
severity_distribution: "0C/0H/1M/4L"
final_assessment: "SUBSTANTIVE"
---

# Phase 1 Spec Adversarial Review — Pass 24

## Final Assessment
SUBSTANTIVE — but severity distribution dropped dramatically (1M/4L vs prior 4M/1L). Adversary self-notes spec approaching natural floor. Predict CLEAN-PASS in Pass 25-26.

Counter regress: 0/3 → 0/3. Pass 23 fixes 5/5 verified + downstream sweep clean.

## Pass 23 Fix Verification (5/5 + Sweep Clean)
- ADV-P23-001 (L2 7-level chain): VERIFIED at bc-07-output-render.md:35,72,115
- ADV-P23-002 (API files 18): VERIFIED at CANONICAL-COUNTS.md:184, adr-index.md:33, system-overview.md:40
- ADV-P23-003 (H-017 source): VERIFIED at holdout-scenarios.md:184
- ADV-P23-004 (Group 1 expanded): VERIFIED at holdout-scenarios.md:27
- ADV-P23-005 (Group 2 refactored): VERIFIED at holdout-scenarios.md:297

Downstream sweep: NO residual "6-level", "17 (`api/", or "tests/h017" tokens.

## Findings

### ADV-P24-001: BC-2.1.006 says "12 filter sources" but body lists 13 (MEDIUM)
- Severity: MEDIUM
- Lens: 5 (CLI flag parity)
- Locations:
  - bc-2-issue-read.md:75 — heading "12 filter sources"
  - bc-2-issue-read.md:80 — body lists 13 flags
  - src/cli/issue/list.rs:347 — same 13-flag literal
- Evidence: --project, --assignee, --reporter, --status, --open, --team, --recent, --created-after, --created-before, --updated-after, --updated-before, --asset, --jql = 13.
- Suggested fix: bc-2-issue-read.md:75 "12" → "13"
- Tag: [content-defect]
- Routing: product-owner

### ADV-P24-002: nfr-catalog.md line 15 narrative ends "= 42 total" — should be 41 (LOW)
- Severity: LOW
- Lens: 6 (count internal consistency)
- Locations: nfr-catalog.md:15 (says 42 total), :17 (says 41), :195 (says 41), frontmatter total_nfrs:41
- Evidence: NFR-O-K merged into NFR-S-D at ADV-P7-002 reduces count from 42 to 41. Line 15 omits the merge step.
- Suggested fix: Append " then NFR-O-K merged into NFR-S-D at ADV-P7-002 = 41 total" or replace trailing "= 42 total" with "= 41 total".
- Tag: [content-defect]
- Routing: product-owner

### ADV-P24-003: state-machines.md L2 preamble "Five state machines" but file documents six (LOW)
- Severity: LOW
- Lens: 6 (count internal consistency)
- Locations:
  - domain-spec/state-machines.md:11 — "Five state machines"
  - domain-spec/state-machines.md:305 — SM-06 Profile Lifecycle (bonus)
  - domain-spec/README.md:35 — "5 state machines"
- Evidence: L2 file has SM-01..SM-06; SM-06 labeled bonus. Architecture has only SM-1..SM-5 (consistent).
- Suggested fix: domain-spec/state-machines.md:11 → "Five canonical state machines (plus SM-06 Profile Lifecycle as bonus context)"
- Tag: [content-defect]
- Routing: business-analyst

### ADV-P24-004: SM-3 source pin disagrees L2 vs architecture (LOW)
- Severity: LOW
- Lens: 2 (state machine ↔ source alignment)
- Locations:
  - domain-spec/state-machines.md:156 — SM-03 source: cli/issue/list.rs:390-487
  - architecture/README.md:64 — SM-3 source: cli/issue/list.rs:395-463
- Evidence: Source 390-487 covers both 3-pass enrichment AND JSON re-injection coda. 395-463 covers only 3-pass enrichment.
- Suggested fix: Align both to 395-463 (canonical 3-pass region) for clean SM scope.
- Tag: [content-defect] (pending intent verification)
- Routing: architect

### ADV-P24-005: holdout-scenarios.md typo "JiaClient" → "JiraClient" (LOW)
- Severity: LOW
- Lens: General typo
- Locations: holdout-scenarios.md:23 — "JiaClient::new_for_test"
- Evidence: Should be JiraClient. Pure typo.
- Suggested fix: JiaClient → JiraClient
- Tag: [content-defect]
- Routing: product-owner

## Observations
- OBS-001: Architecture canonicalizes 5 SMs (excludes SM-06). Only L2 preamble out of sync.
- OBS-002 [process-gap]: Third recurrence of "count-claim heading drift". S-7.01 codification recommended.
- OBS-003: P23 downstream sweep clean.
- OBS-004: All 11 JrError variants properly mapped to exit codes.
- OBS-005: JQL escaping invariants verified against src/jql.rs.
- OBS-006: SM-3 NFR-R-E bug accurately described in state machine.

## Lens Coverage Summary
- Lens 1 (P23 verification + sweep): 5/5 + clean ✓
- Lens 2 (SM ↔ source): 1 finding (ADV-P24-004)
- Lens 3 (JQL escaping): 0 findings
- Lens 4 (error/exit code): 0 findings
- Lens 5 (CLI flag parity): 1 finding (ADV-P24-001)
- Lens 6 (frontmatter): 0 findings
- Lens 7 (README cross-refs): 0 findings
- Bonus (typo/SM count): 2 findings (ADV-P24-003, ADV-P24-005)

## Novelty Assessment
MEDIUM. Severity distribution shift (4M/1L → 1M/4L) signals approach to floor. Inflection toward CLEAN-PASS predicted Pass 25-26.

## Verdict
SUBSTANTIVE (5 findings + 6 obs). Trajectory ...→4→5→5→5. Severity-down trend.

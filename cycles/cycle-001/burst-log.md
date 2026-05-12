---
document_type: burst-log
level: ops
version: "1.0"
status: in-progress
producer: state-manager
timestamp: 2026-05-04T00:00:00
cycle: "cycle-001"
inputs: [STATE.md]
input-hash: "[live-state]"
traces_to: STATE.md
---

# Burst Log — cycle-001

## Burst 1 (2026-05-04)

**Agents dispatched:** devops-engineer, state-manager
**Files touched:** .factory/STATE.md, .factory/cycles/cycle-001/cycle-manifest.md, .factory/cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Factory infrastructure bootstrapped by devops-engineer: factory-artifacts branch created, .factory/ worktree mounted, placeholder STATE.md written. State-manager seeded STATE.md with full brownfield activation state at v0.5.0-dev.7 (activation HEAD dea166471e22eff55974d7675593469b37048c5f, factory-artifacts seed SHA b8f66501d12a37f7669e01cc95cdb24029a1b4b2). Cycle-001 directory initialized. Env preflight running in parallel via dx-engineer.

### Details

| Agent | Task | Output |
|-------|------|--------|
| devops-engineer | factory-artifacts branch + .factory/ worktree bootstrap | .factory/ mounted on factory-artifacts |
| state-manager | Seed STATE.md + initialize cycle-001 | .factory/STATE.md, .factory/cycles/cycle-001/ |

---

## Burst 2 (2026-05-04)

**Agents dispatched:** codebase-analyzer ×7, state-manager
**Files touched:** semport/jira-cli/ (7 pass artifacts), .factory/.gitignore, .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Phase A brownfield ingest of jira-cli complete. codebase-analyzer ran 7 broad passes (inventory → architecture → domain model → behavioral contracts → NFR catalog → conventions → synthesis). All 7 pass files committed to factory-artifacts (SHA 0380885). logs/ untracked via .gitignore. DEC-002 added: default pre-VSDD docs treatment is HARMONIZE per Pass 6 §7.5 — pending human approval at Phase 0 → Phase 1 gate.

### Details

| Agent | Task | Output |
|-------|------|--------|
| codebase-analyzer | Pass 0 — Inventory | semport/jira-cli/jira-cli-pass-0-inventory.md |
| codebase-analyzer | Pass 1 — Architecture | semport/jira-cli/jira-cli-pass-1-architecture.md |
| codebase-analyzer | Pass 2 — Domain Model | semport/jira-cli/jira-cli-pass-2-domain-model.md |
| codebase-analyzer | Pass 3 — Behavioral Contracts | semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md |
| codebase-analyzer | Pass 4 — NFR Catalog | semport/jira-cli/jira-cli-pass-4-nfr-catalog.md |
| codebase-analyzer | Pass 5 — Conventions | semport/jira-cli/jira-cli-pass-5-conventions.md |
| codebase-analyzer | Pass 6 — Synthesis | semport/jira-cli/jira-cli-pass-6-synthesis.md |
| state-manager | Commit Phase A artifacts + .gitignore + STATE.md update | factory-artifacts 0380885 |

---

## Burst 3 (2026-05-04)

**Agents dispatched:** codebase-analyzer ×20 rounds across 6 passes, state-manager
**Files touched:** semport/jira-cli/ (21 deep-round files), .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Phase B convergence deepening complete. All 6 passes converged to NITPICK via iterative deepening. codebase-analyzer ran 20 total rounds (Pass 0: R1-R2, Pass 1: R1-R2, Pass 2: R1-R7, Pass 3: R1-R4, Pass 4: R1-R4, Pass 5: R1-R2). 21 deep-round artifacts committed to factory-artifacts (SHA 257bdd7). 5 cross-pollination bugs verified at source. 12+ hallucinations caught and retracted (CONV-ABS markers). DEC-003 added: address 4 MUST-FIX bugs at Phase 0→1 gate. Phase B.5 coverage audit is next.

Key findings cataloged:
- 540 BCs total (475 HIGH / 59 MEDIUM / 6 LOW), 47 holdout scenarios
- 411 domain invariants, 265 domain entities
- 44 NFR gaps (1 CRITICAL / 4 HIGH / 16 MEDIUM / 22 LOW)
- 7 architectural patterns + 7 anti-patterns identified
- 4 MUST-FIX correctness bugs: handle_open OAuth, list_worklogs truncation, hardcoded 8h/5d, multi-workspace HashMap
- CRITICAL multi-profile fields silent regression (12 read sites)
- 2 security gaps: JR_AUTH_HEADER no production gating, --verbose header dump

### Details

| Agent | Task | Output |
|-------|------|--------|
| codebase-analyzer | Pass 0 — deepening R1-R2 (metric corrections, orphan modules) | jira-cli-pass-0-deep-r1.md, jira-cli-pass-0-deep-r2.md |
| codebase-analyzer | Pass 1 — deepening R1-R2 (5 new state machines, 26 risks) | jira-cli-pass-1-deep-r1.md, jira-cli-pass-1-deep-r2.md |
| codebase-analyzer | Pass 2 — deepening R1-R7 (265 entities, 411 invariants) | jira-cli-pass-2-deep-r1.md through jira-cli-pass-2-deep-r7.md |
| codebase-analyzer | Pass 3 — deepening R1-R4 (540 BCs, 47 holdouts) | jira-cli-pass-3-deep-r1.md through jira-cli-pass-3-deep-r4.md |
| codebase-analyzer | Pass 4 — deepening R1-R4 (44 NFR gaps, 4 MUST-FIX bugs) | jira-cli-pass-4-deep-r1.md through jira-cli-pass-4-deep-r4.md |
| codebase-analyzer | Pass 5 — deepening R1-R2 (7 patterns, 7 anti-patterns) | jira-cli-pass-5-deep-r1.md, jira-cli-pass-5-deep-r2.md |
| state-manager | Commit Phase B artifacts + STATE.md update | factory-artifacts 257bdd7 |

---

## Burst 4 (2026-05-04)

**Agents dispatched:** codebase-analyzer (B.5, B.6, C), state-manager
**Files touched:** semport/jira-cli/jira-cli-coverage-audit.md, semport/jira-cli/jira-cli-extraction-validation.md, semport/jira-cli/jira-cli-pass-8-deep-synthesis.md, .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Phase B.5 coverage audit: PASS — no implementation-surface blind spots. 2 MEDIUM optional doc-surface items flagged (README + install.sh), non-blocking.

Phase B.6 extraction validation: PASS — 96.7% behavioral accuracy (29/30 confirmed, 1 inaccurate, 0 hallucinated). 0 phantom modules / dependencies / BCs. 2 minor metric annotation deltas (off-by-one NFR count summary; mermaid count annotation).

Phase C final synthesis: complete — 750 lines. Lessons section: P0=4, P1=8, P2=6, P3=5. Downstream skill recommendations: /create-brief, /create-domain-spec (READY), /create-prd, /decompose-stories (~22 stories / 3 waves), /create-architecture. Pre-VSDD docs treatment: HARMONIZE (per Pass 6 §7.5, updated).

Brownfield ingest (Phase 0) is COMPLETE. Phase 0 → Phase 1 human approval gate is next.

### Details

| Agent | Task | Output |
|-------|------|--------|
| codebase-analyzer | Phase B.5 — Coverage audit | semport/jira-cli/jira-cli-coverage-audit.md |
| codebase-analyzer | Phase B.6 — Extraction validation | semport/jira-cli/jira-cli-extraction-validation.md |
| codebase-analyzer | Phase C — Final synthesis | semport/jira-cli/jira-cli-pass-8-deep-synthesis.md (750 lines) |
| state-manager | Commit Phase B.5/B.6/C artifacts; update STATE.md to Phase 0 complete | factory-artifacts (this commit) |

---

## Burst 5 (2026-05-04)

**Agents dispatched:** state-manager ×2, codebase-analyzer ×3
**Files touched:** semport/jira-cli/jira-cli-pre-vsdd-plans-spot-check.md, semport/jira-cli/jira-cli-bc-nfr-r-d-draft.md, semport/jira-cli/jira-cli-pre-vsdd-harmonization-plan.md, semport/jira-cli/jira-cli-pre-gate-consistency-audit.md, .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Phase 0 gate closeout. Human approved Phase 0 → Phase 1 transition. Gate-resolution artifacts produced and committed (d1a30f1):

- Q1: 5/5 sampled pre-VSDD plans confirmed DELIVERED. Directory-wide SUPERSEDE confirmed for `docs/superpowers/plans/`.
- Q2: NFR-R-D BC draft produced — 11 production read sites in 5 files. Smoking-gun at config.rs:142-149. Holdout H-NEW-MP-001 proposed. Draft ready for Phase 1 PRD formalization.
- Q4: 78-doc harmonization plan complete — 74 DELIVERED-AS-DESIGNED, 2 DELIVERED-DIVERGENT, 1 ARCHAEOLOGICAL, 0 PARTIAL/UNDELIVERED. 74 specs become BC validation inputs. v1 design imports as historical with annotated supersessions (OAuth → ADR-0006; Global config → multi-profile-auth; Project Structure → Pass 0 inventory).
- Q5: synthesis fixes committed earlier as d8ca198 (5 consistency repairs to Phase C synthesis).

DEC-001/DEC-002/DEC-003 resolved. DEC-004 added (streamlined vs full Phase 1 scope). Phase 0 COMPLETE. Phase 1 entry pending DEC-004 human decision.

### Details

| Agent | Task | Output |
|-------|------|--------|
| codebase-analyzer | Q1 — spot-check 5 pre-VSDD plans | semport/jira-cli/jira-cli-pre-vsdd-plans-spot-check.md |
| codebase-analyzer | Q2 — BC draft for NFR-R-D (multi-profile fields regression) | semport/jira-cli/jira-cli-bc-nfr-r-d-draft.md |
| codebase-analyzer | Q4 — harmonization plan for 78 docs | semport/jira-cli/jira-cli-pre-vsdd-harmonization-plan.md |
| codebase-analyzer | Pre-gate consistency audit (produced Q5 fixes) | semport/jira-cli/jira-cli-pre-gate-consistency-audit.md |
| state-manager | Commit closeout artifacts | factory-artifacts d1a30f1 |
| state-manager | Update STATE.md — Phase 0 COMPLETE, Phase 1 entry, DEC-001-004 | factory-artifacts (commit 2, pending) |

---

## Burst 6 (2026-05-04)

**Agents dispatched:** state-manager, product-owner, architect (parallel)
**Files touched:** specs/prd/BC-INDEX.md, specs/prd/bc-1-auth-identity.md, specs/prd/bc-2-issue-read.md, specs/prd/bc-3-issue-write.md, specs/prd/bc-4-assets-cmdb.md, specs/prd/bc-5-boards-sprints.md, specs/prd/bc-6-config-cache.md, specs/prd/bc-7-output-render.md, specs/prd/cross-cutting.md, specs/prd/edge-case-catalog.md, specs/prd/holdout-scenarios.md, specs/prd/nfr-catalog.md, architecture/cross-cutting.md, architecture/dtu-assessment.md, architecture/state-machines.md, architecture/risk-register.md, architecture/adr-index.md, architecture/adr/0007-multi-profile-fields-fix.md, architecture/adr/0009-handle-open-instance-url.md, architecture/security-decisions/SD-001-pkce.md, architecture/security-decisions/SD-002-jr-auth-header-prod-gating.md, architecture/security-decisions/SD-003-verbose-pii-redaction.md, cicd-setup.md
**Versions bumped:** (none)

### Summary

Phase 1d adversary Pass 1 + fixes. Adversarial review produced 30 findings (4C/11H/12M/3L). 29 addressed, 1 deferred (ADV-P1-030 — orchestrator process-gap, .factory/policies.yaml — codification task post Phase 1). BC-INDEX rebuilt from canonical body files (CRITICAL). 3 SD-NNN security decision artifacts created. Adversary Pass 2 next.

### Details

| Agent | Task | Output |
|-------|------|--------|
| product-owner | BC-INDEX rebuild; 9 holdout anchors; BC-2.2.021, BC-3.7.004, BC-6.3.001, BC-6.2.015, BC-7.3.005, BC-X.4.009; EC-CFG-001/002 swap; NFR-S-E; NFR count reconciliation; BC-6.1.011 | 12 specs/prd files |
| architect | extract_error_message chain 7-level; DTU PKCE struck; ADR-0007 Option A; SM-1/SM-2 anchors; risk register numbering; cicd-setup §7; ADR-0009; 3 SD-NNN artifacts; adr-index harmonization | 8 architecture files + 3 new SD-NNN |
| state-manager | Stage + commit 23 files; update STATE.md + burst-log | factory-artifacts e00d01e (fixes), + state commit (this) |

---

## Burst 7 (2026-05-04)

**Agents dispatched:** adversary (fresh-context), state-manager
**Files touched:** .factory/cycles/cycle-001/adversarial-reviews/adv-p1-pass2.md, .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Adversary Pass 2 complete. 15 findings (0 CRITICAL / 6 HIGH / 6 MEDIUM / 3 LOW). Pass 1=30 → Pass 2=15. Trend favorable. Convergence counter 0/3 (need 3 consecutive clean passes; Pass 2 still has 6 HIGH requiring fixes before Pass 3).

Key HIGH findings:
- ADV-P2-001: extract_error_message 3-way contradiction across 4 docs (error-taxonomy 6 vs 7 level header/body; BC-7.3.001 vs BC-7.3.005 empty-body; BC-INDEX wrong quote)
- ADV-P2-002: ≥11 of 48 holdout BC anchors incorrect after rebuild
- ADV-P2-003: NFR-R-NEW-1 referenced in 4 places but missing from NFR catalog
- ADV-P2-004: NFR-S-E severity — LOW (nfr-catalog) vs CRITICAL (cicd-setup) vs absent (risk-register)
- ADV-P2-005: NFR catalog count disagrees 4 ways (45 / 44 / 43 / 40)
- ADV-P2-006: DTU assessment cites 47 holdouts vs canonical 48

### Details

| Agent | Task | Output |
|-------|------|--------|
| adversary | Phase 1d adversarial spec review Pass 2 (fresh-context) | adv-p1-pass2.md (15 findings; 0C/6H/6M/3L) |
| state-manager | Persist Pass 2 findings; update STATE.md convergence + checkpoint; commit | factory-artifacts (this commit) |

---

## Burst 8 (2026-05-04)

**Agents dispatched:** product-owner, architect (parallel)
**Files touched:** specs/prd/bc-7-output-render.md, specs/prd/error-taxonomy.md, specs/prd/BC-INDEX.md, specs/prd/holdout-scenarios.md, specs/prd/nfr-catalog.md, specs/prd/cross-cutting.md, specs/prd/bc-6-config-cache.md, architecture/dtu-assessment.md, architecture/cicd-setup.md, architecture/risk-register.md, architecture/security-decisions/SD-001-pkce.md, architecture/security-decisions/SD-002-jr-auth-header-prod-gating.md, architecture/security-decisions/SD-003-verbose-pii-redaction.md
**Versions bumped:** (none)

### Summary

Pass 2 fixes (product-owner + architect parallel). 12 of 15 findings addressed; 3 deferred/no-action.

Product-owner fixes (10 findings): extract_error_message chain canonicalized to 7-step from source (src/api/client.rs:448-490) — empty-body → literal "<empty response body>", errorMessage as level 6 (not errorDescription); BC-7.3.001/005, error-taxonomy, BC-INDEX all aligned. 12 holdout BC anchors corrected (H-002/008/009/010/011/015/016/020/023/025/029/030/047). NFR-R-NEW-1 (Retry-After unbounded LOW) added to nfr-catalog.md. NFR catalog reconciled to 41 entries (1C/5H/15M/20L); all 4 totals unified. cross-cutting.md range-collapsed marker for BC-X.4.003..008. BC-6.3.001 cross-references ADR-0007 Config::field_id() accessor.

Architect fixes (3 findings, 1 shared): DTU holdout count corrected 47 → 48. NFR-S-E severity reconciled to HIGH (was LOW in catalog, CRITICAL in cicd-setup); R-H7 added to risk register; risk total 26 → 27. SD-001/002/003 deadlines scheduled for Phase 1 → 2 gate.

Deferred: ADV-P2-013 (LOW) — BC-X.4.003..008 numbering aesthetic; ADV-P2-014 (LOW) — H-014 intentional 3-pass-3-BC collapse; ADV-P2-015 — resolved by ADV-P2-001 fix.

Convergence counter: 0/3 clean passes needed. Pass 3 dispatching next.

### Details

| Agent | Task | Output |
|-------|------|--------|
| product-owner | ADV-P2-001/002/003/005/007/011 fixes: error chain, holdout anchors, NFR-R-NEW-1, NFR catalog totals, cross-cutting range-collapse, BC-6.3.001 ADR ref | 7 specs/prd files |
| architect | ADV-P2-004/006/009 fixes: NFR-S-E HIGH, DTU count 48, SD deadlines, risk R-H7 | 6 architecture files |
| state-manager | Stage + commit 13 files; update STATE.md + burst-log | factory-artifacts (this commit) |

## Burst 9 (2026-05-04)

**Agents dispatched:** state-manager
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass3.md, specs/prd/bc-6-config-cache.md, specs/prd/README.md, specs/prd/BC-INDEX.md, specs/prd/nfr-catalog.md, specs/prd/edge-case-catalog.md, specs/prd/holdout-scenarios.md, architecture/adr/0007-multi-profile-fields-fix.md, architecture/risk-register.md, architecture/cross-cutting.md, STATE.md
**Versions bumped:** (none)

### Summary

Pass 3 adversarial review (9 findings: 1C/3H/3M/2L) persisted and all 9 addressed (8 fixed, 1 documented with rationale). Trajectory: 30→15→9 (linear convergence). Convergence counter still 0/3 — Pass 4 dispatches next.

CRITICAL: ADV-P3-001 — site count canonicalized to 14 across 4 docs (bc-6, ADR-0007, risk-register R-C1, nfr-catalog NFR-R-D). The BC table has 14 rows; "11 hot-path" and "12+" stale references removed.

HIGH: ADV-P3-002 — ADR-0007 §Context fallback clause struck; no-fallback policy now unified with §Decision/§Consequences; rejected sub-option note added. ADV-P3-003 — cross-cutting.md error chain replaced with PRD-canonical 7-level table (Priority 4 = non-empty errors object; Priority 6 = errorMessage); old divergent chain removed; single-source note added. ADV-P3-004 — NFR catalog total reconciled to 42 (1C/6H/15M/20L) after NFR-S-F addition; README doc-map and supplement index updated.

MEDIUM: ADV-P3-005 — EC-AUTH-002/003/004 BC mis-anchors fixed; spot-check of EC-CFG/HTTP/JQL/ASSET/SPRINT shows no additional errors. ADV-P3-006 — PRD README total BCs 541→542. ADV-P3-007 — NFR-S-F (cargo-deny multiple-versions) added as HIGH; R-H6 cross-linked; NFR totals propagated to 4 docs.

LOW: ADV-P3-008 — H-022 BC refs appended with BC-1.6.045. ADV-P3-009 — NFR-R-NEW-1 severity LOW retained with inline rationale documented.

### Details

| Agent | Task | Output |
|-------|------|--------|
| state-manager | Write adv-p1-pass3.md; apply 9 fixes across 10 spec files; commit 69741c3 | factory-artifacts 69741c3 |
| state-manager | Update STATE.md Phase Progress, Current Steps, Convergence Tracker, Session Checkpoint, burst-log | factory-artifacts (this commit) |

## Burst 10 (2026-05-04)

**Agents dispatched:** state-manager
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass4.md, specs/prd/holdout-scenarios.md, specs/prd/nfr-catalog.md, architecture/README.md, STATE.md
**Versions bumped:** (none)

### Summary

Pass 4 adversarial review (5 findings: 0C/0H/4M/1L) persisted and all 5 fixed. Trajectory: 30→15→9→5 (linear decay continuing). Convergence counter still 0/3 — Pass 5 dispatches next.

MEDIUM: ADV-P4-001 — H-004 BC anchor corrected from BC-1.6.046 to BC-1.1.011 (auth refresh unconfigured profile). ADV-P4-002 — H-005 BC anchor corrected from BC-6.1.002 to BC-1.1.012 (malformed TOML); consistent with EC-AUTH-004. ADV-P4-003 — H-012 BC anchors corrected from BC-1.6.044/BC-X.1.007 to BC-1.6.042/BC-X.3.005 (scope-mismatch). ADV-P4-004 — architecture README risk count refreshed 26→27; site count updated 12+→14.

LOW: ADV-P4-005 — nfr-catalog routing arithmetic corrected from 0M/3L to 2M/1L for FIX-IN-PHASE-3 bucket.

### Details

| Agent | Task | Output |
|-------|------|--------|
| state-manager | Write adv-p1-pass4.md; apply 5 fixes across 3 spec files; commit | factory-artifacts |
| state-manager | Update STATE.md Phase Progress, Current Steps, Session Checkpoint, burst-log | factory-artifacts (this commit) |

## Burst 11 (2026-05-04)

**Agents dispatched:** product-owner, architect, state-manager
**Files touched:** 8 spec/arch files + adv-p1-pass5.md + STATE.md + burst-log.md
**Versions bumped:** (none)

### Summary

Pass 5 + comprehensive sweep (product-owner + architect): 10 cited findings FIXED + 4 sweep additionals. REGRESSION from Pass 4 (5→10). Root cause: anchor tables in supplement files (Competitive Differentiators table in PRD README, edge-case-catalog EC-OUT-005) not subjected to same audit as BC bodies in prior passes. Counter remains 0/3. Pass 6 dispatches next.

Final count manifest: 542 BCs / 42 NFRs / 48 holdouts / 27 risks.

### 10 Cited Findings Fixed

| Finding | Fix |
|---------|-----|
| ADV-P5-001 | PRD README "6-level" → "7-level" extract_error_message |
| ADV-P5-002 | EC-OUT-005 empty-body propagation completed |
| ADV-P5-003 | BC-6.3.001 "11 read sites" → "14" |
| ADV-P5-004 | bc-6 body "38" → "39" (matches frontmatter) |
| ADV-P5-005 | 4 PRD Competitive Differentiators anchor fixes |
| ADV-P5-006 | EC-OUT-007 → BC-7.3.005 |
| ADV-P5-007 | 542 BC count formula reconciled across PRD + BC-INDEX |
| ADV-P5-008 | bc-7 definitional_count 33 → 34 |
| ADV-P5-009 | NFR-R-NEW-1 routing harmonized to FIX-IN-PHASE-3 |
| ADV-P5-010 | DTU assessment "14" → "7" bounded contexts |

### 4 Comprehensive Sweep Additionals Fixed

| Item | Fix |
|------|-----|
| A. Holdout BC anchors (all 48 verified) | H-033 fixed |
| B. EC-* anchors sweep | EC-HTTP-001, EC-AUTH-008, EC-SPRINT-002 fixed |
| C. PRD README + BC-INDEX MUST-FIX registers | verified clean |
| D. Cross-reference recount | complete |

### Details

| Agent | Task | Output |
|-------|------|--------|
| product-owner | Fix 9 cited findings (P5-001..009) across 7 spec files | specs/prd/*.md, architecture/dtu-assessment.md |
| architect | Fix ADV-P5-010 (DTU bounded context count) | architecture/dtu-assessment.md |
| state-manager | Write adv-p1-pass5.md; commit fixes (826bd67) | factory-artifacts |
| state-manager | Update STATE.md Phase Progress, Convergence Tracker, Session Checkpoint, burst-log | factory-artifacts (this commit) |

## Burst 12 (2026-05-04)

**Agents dispatched:** state-manager
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass6.md, architecture/cross-cutting.md, specs/prd/nfr-catalog.md, architecture/risk-register.md, architecture/README.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Pass 6 adversarial review (5 findings: 0C/1H/3M/1L) persisted and all 5 fixed. Trajectory: 30→15→9→5→10→5 (recovery from Pass 5 regression). Convergence counter still 0/3 — Pass 7 dispatches next.

HIGH: ADV-P6-001 — MatchResult enum corrected in arch cross-cutting.md (Exact/ExactMultiple/Ambiguous/None; removed fabricated `Unique` variant; added `ExactMultiple` per source partial_match.rs).

MEDIUM: ADV-P6-002 — 7-step extract_error_message table removed from arch cross-cutting.md (single-source now PRD error-taxonomy.md §2). ADV-P6-003 — NFR-R-NEW-1/2 moved from ### MEDIUM section to ### LOW in nfr-catalog.md (severity already LOW; section was incorrect). ADV-P6-004 — R-H3 demoted from HIGH to MEDIUM (matches NFR-S-C severity; `--verbose` is opt-in, user-controlled); HIGH 7→6, MEDIUM 8→9, total 27 unchanged; ID renumbered R-M0 (traceability note added), former R-H4..H7 renumbered R-H3..H6.

LOW: ADV-P6-005 — arch README risk arithmetic corrected to match risk-register.md preamble (11 R1-NEW + 14 broad-pass + 1 R1-NEW reclassified to CRITICAL + 1 Pass-2 ADV-P2-004 addition).

### Details

| Agent | Task | Output |
|-------|------|--------|
| state-manager | Write adv-p1-pass6.md; apply 5 fixes across 4 spec/arch files | factory-artifacts |
| state-manager | Update STATE.md Phase Progress, Convergence Tracker, Session Checkpoint, burst-log | factory-artifacts (this commit) |

## Burst 13 (2026-05-04)

**Agents dispatched:** state-manager
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass7.md, specs/prd/nfr-catalog.md, specs/prd/cross-cutting.md, specs/prd/README.md, specs/prd/BC-INDEX.md, architecture/cross-cutting.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Pass 7 adversarial review (4 findings: 0C/0H/3M/1L) persisted and all 4 fixed. Trajectory: 30→15→9→5→10→5→4. Convergence counter still 0/3 — Pass 8 dispatches next.

ADV-P7-001 CLOSED (no change): BC count 542 is correct — BC-INDEX table sums 541 from sections + 1 new BC-X.4.009 = 542. Finding was a false alarm.

ADV-P7-002 FIXED: NFR-O-K (duplicate of NFR-S-D; same site src/config.rs:113-140, same routing DOCUMENT-AS-IS) merged into NFR-S-D with cross-reference note. NFR total 42→41; severity 1C/6H/15M/19L=41. Count propagated to nfr-catalog.md frontmatter, header totals, routing summary, README.md (×2), BC-INDEX.md.

ADV-P7-003 FIXED: cross-cutting.md definitional_count corrected 63→64 (actual `#### BC-` heading count = 64; BC-INDEX already showed 64 individually-bodied — now in sync).

ADV-P7-004 FIXED: arch cross-cutting.md MatchResult::ExactMultiple description rewritten — "first wins, no disambiguation" replaces misleading "used for disambiguation".

### Details

| Agent | Task | Output |
|-------|------|--------|
| state-manager | Write adv-p1-pass7.md; apply 3 real fixes + 1 sweep | factory-artifacts |
| state-manager | Update STATE.md Position, Convergence counter, burst-log | factory-artifacts (this commit) |

## Burst 14 (2026-05-04)

**Agents dispatched:** state-manager
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass8.md, specs/prd/nfr-catalog.md, architecture/adr-index.md, architecture/risk-register.md, architecture/README.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Pass 8 adversarial review (3 findings: 0C/1H/2M/0L) persisted and all 3 FIXED. Trajectory: 30→15→9→5→10→5→4→3. Convergence counter still 0/3 — Pass 9 dispatches next.

ADV-P8-001 FIXED (HIGH): nfr-catalog.md routing summary DEFER count corrected 17→12. Sum now 10+3+3+13+12=41 (reconciles to NFR total).

ADV-P8-002 FIXED (MEDIUM): adr-index.md ADR-0009 architecture section anchor corrected §R-H4→§R-H3. R-H3 is handle_open (ADR-0009); R-H4 is list_worklogs (ADR-0010).

ADV-P8-003 FIXED (MEDIUM): R-M3 (Retry-After MEDIUM) merged into R-L11 (Retry-After LOW) — duplicate concern. NFR-SCA-1 authoritative severity is LOW. Risk totals: MEDIUM 9→8, total 27→26. Architecture README updated 27→26. R-L11 annotated with merger note.

### Details

| Agent | Task | Output |
|-------|------|--------|
| state-manager | Write adv-p1-pass8.md; apply 3 fixes across 4 arch/spec files | factory-artifacts |
| state-manager | Update STATE.md Phase Progress, Convergence Tracker, Session Checkpoint, burst-log | factory-artifacts (this commit) |

## Burst 15 (2026-05-04)

**Agents dispatched:** state-manager
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass9.md, architecture/risk-register.md, specs/prd/nfr-catalog.md, architecture/cross-cutting.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Pass 9 adversarial review (4 findings: 0C/0H/4M/0L) persisted and all 4 FIXED. Trajectory: 30→15→9→5→10→5→4→3→4 (plateau in 3-5 range). Convergence counter still 0/3 — small-blast-radius drift in summary arithmetic and cross-doc anchors.

ADV-P9-001 FIXED (MEDIUM): risk-register.md Risk Summary action breakdown recounted from body. HIGH: 5×FIX/1×SEC-DECIDE (was 4/2); MEDIUM: 4×DEFER/1×DOC/1×FIX/2×SEC (was 3/2/1/2); LOW: 8×DOC/2×DEFER/1×POLICY (was 7/3/1).

ADV-P9-002 FIXED (MEDIUM): NFR-S-F site path corrected from `.cargo/deny.toml` to `deny.toml` (file lives at project root, not in `.cargo/`). Cross-ref `.github/workflows/ci.yml` retained.

ADV-P9-003 FIXED (MEDIUM): NFR-S-F cross-ref corrected R-H6 → R-H5 in nfr-catalog.md. R-H5 is supply-chain (NFR-S-F); R-H6 is SHA-pinning (NFR-S-E).

ADV-P9-004 FIXED (MEDIUM): arch cross-cutting.md MatchResult::Ambiguous description corrected — "one or more items contain the needle substring (single substring hit is also `Ambiguous` — fail-closed design)". Prior text "multiple items" was factually wrong per partial_match.rs:39-42.

### Details

| Agent | Task | Output |
|-------|------|--------|
| state-manager | Write adv-p1-pass9.md; apply 4 fixes across 3 spec/arch files | factory-artifacts |
| state-manager | Update STATE.md Phase Progress, Convergence Tracker, Session Checkpoint, burst-log | factory-artifacts (this commit) |

## Burst 16 (2026-05-04)

**Agents dispatched:** state-manager, adversary
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass10.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Burst 16 — Pass 10 (state-manager + adversary): CLEAN-PASS achieved! Trajectory 30→15→9→5→10→5→4→3→4→0. Counter 0/3 → 1/3. First clean pass after 9 fix-bursts. Pass 11 next (target 2/3).

No findings. All Pass 9 fixes verified propagated cleanly. NFR catalog 41, risk register 26, BC count 542, holdouts 48 — all reconcile. MUST-FIX register consistent across 5+ docs. ADR-0009 anchor correct. 5 BC source-line spot-checks exact.

### Details

| Agent | Task | Output |
|-------|------|--------|
| adversary | Phase 1d adversarial spec review Pass 10 (CLEAN-PASS) | adv-p1-pass10.md (0 findings) |
| state-manager | Persist Pass 10 CLEAN-PASS; update STATE.md convergence counter 0/3 → 1/3; commit | factory-artifacts (this commit) |

## Burst 17 (2026-05-04)

**Agents dispatched:** state-manager
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass11.md, specs/prd/nfr-catalog.md, architecture/cross-cutting.md, specs/domain-spec/state-machines.md, architecture/state-machines.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Burst 17 — Pass 11 + fixes: 2 findings (1H/1M), all FIXED. New lenses surfaced dep-fact contradiction + cache-count semantic. Counter REGRESSED 1/3 → 0/3.

HIGH: ADV-P11-001 — nfr-catalog.md NFR-O-A + arch cross-cutting.md corrected: `tracing` is NOT a current dep (Cargo.toml:14-37 verified). L2 was correct; PRD and arch claimed it was "already a dep". Phase 3 task clarified to dep-add + subscriber wire-up.

MEDIUM: ADV-P11-002 — L2 state-machines.md + arch state-machines.md cache count corrected "7 distinct" → "6 distinct". Hybrid breakdown: 4 pure-Expiring + 1 keyed-map + 1 hybrid (object_type_attrs is BOTH, not a 7th category). Table already had 6 rows — only header and body text were wrong.

### Details

| Agent | Task | Output |
|-------|------|--------|
| state-manager | Write adv-p1-pass11.md; apply 4 edits across 4 spec/arch files | factory-artifacts |
| state-manager | Update STATE.md Phase Progress, Convergence Tracker, Session Checkpoint, burst-log | factory-artifacts (this commit) |

## Burst 18 (2026-05-04)

**Agents dispatched:** state-manager, adversary
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass12.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Burst 18 — Pass 12 (state-manager + adversary): CLEAN-PASS. Counter 0/3 → 1/3. Pass 11 regression healed; 2 more consecutive clean required.

Pass 11 fixes verified propagated cleanly: tracing dep claim consistent across Cargo.toml/L2/PRD/arch (all 4 docs); cache count = 6 distinct (hybrid breakdown) consistent across L2 + arch state-machines.md. No new findings. BC totals 542, holdouts 48, NFR 41 all reconcile. Pass 13 dispatches next.

### Details

| Agent | Task | Output |
|-------|------|--------|
| adversary | Phase 1d adversarial spec review Pass 12 (CLEAN-PASS) | adv-p1-pass12.md (0 findings) |
| state-manager | Persist Pass 12 CLEAN-PASS; update STATE.md convergence counter 0/3 → 1/3; burst-log; commit | factory-artifacts (this commit) |

---

## Burst 19 (2026-05-04)

**Agents dispatched:** product-owner, architect, state-manager
**Files touched:** specs/prd/BC-INDEX.md, specs/prd/README.md, specs/prd/nfr-catalog.md, architecture/risk-register.md, architecture/README.md, specs/prd/CANONICAL-COUNTS.md (new), cycles/cycle-001/adversarial-reviews/adv-p1-pass13.md (new), STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Burst 19 — Pass 13 + comprehensive pre-Pass-14 sweep (product-owner + architect): 3 MEDIUM findings all fixed; 4-sweep audit completed; BC count canonicalized to 541; CANONICAL-COUNTS.md created as single source of truth. Counter 1/3 → 0/3 (regression). Pass 14 next.

Pass 13 fixes:
- ADV-P13-001: BC grand total 542 → 541 — BC-X.4.009 was double-counted in BC-INDEX:648 footnote; corrected across PRD README + BC-INDEX.
- ADV-P13-002: NFR-O-G stale LOC updated 970 → 1,083 in nfr-catalog.md.
- ADV-P13-003: cicd-setup.md dangling path ref in risk-register.md corrected to ../cicd-setup.md; entry added to arch README Document Map.

Comprehensive 4-sweep audit:
- Sweep 1 (counts): definitional_count grep confirms sum=541; NFR=41; holdouts=48; risks=26.
- Sweep 2 (paths): no other broken refs found.
- Sweep 3 (source-line, 10 samples): zero drift.
- Sweep 4 (severity/routing): all 7 HIGH/CRIT NFRs match risk register rows.

CANONICAL-COUNTS.md created with shell-verifiable counts for future passes.

### Details

| Agent | Task | Output |
|-------|------|--------|
| adversary | Phase 1d adversarial spec review Pass 13 | adv-p1-pass13.md (3 MEDIUM findings) |
| product-owner | Fix ADV-P13-001 (BC count 542→541), ADV-P13-002 (NFR-O-G LOC 970→1,083); 4-sweep audit; create CANONICAL-COUNTS.md | specs/prd/BC-INDEX.md, specs/prd/README.md, specs/prd/nfr-catalog.md, specs/prd/CANONICAL-COUNTS.md |
| architect | Fix ADV-P13-003 (cicd-setup.md path refs in risk-register + arch README) | architecture/risk-register.md, architecture/README.md |
| state-manager | Persist Pass 13 findings; update STATE.md (counter 0/3, trajectory, checkpoint, steps); burst-log; commit | factory-artifacts |

---

## Burst 20 (2026-05-04)

**Agents dispatched:** state-manager, adversary
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass14.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Burst 20 — Pass 14 (state-manager + adversary): CLEAN-PASS. Counter 0/3 → 1/3. CANONICAL-COUNTS.md prevents count drift. 2 more clean passes needed.

No substantive findings. 2 nitpicks honestly demoted to LOW (holdout Group 1 label inaccuracy; L2 README "12+" vs canonical "14" — non-contradictory). 4/4 source-truth spot checks exact. CANONICAL-COUNTS = 541 BCs / 41 NFRs / 48 holdouts / 26 risks stable across all docs.

### Details

| Agent | Task | Output |
|-------|------|--------|
| adversary | Phase 1d adversarial spec review Pass 14 (CLEAN-PASS) | adv-p1-pass14.md (0 findings) |
| state-manager | Persist Pass 14 CLEAN-PASS; update STATE.md convergence counter 0/3 → 1/3; burst-log; commit | factory-artifacts (this commit) |

---

## Burst 21 (2026-05-04)

**Agents dispatched:** state-manager
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass15.md, specs/prd/bc-3-issue-write.md, specs/prd/bc-1-auth-identity.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Burst 21 — Pass 15 + bc-*.md body sweep: 2 findings + per-file body audit. Counter 1/3 → 0/3 reset.

Pass 15 trajectory: 30→15→9→5→10→5→4→3→4→0→2→0→3→0→2. Counter regress 1/3 → 0/3.

ADV-P15-001 (HIGH): bc-3-issue-write.md end-of-file "Total BCs in this file: 40" corrected to "48 individually-bodied (cumulative 77 incl. range-collapsed)".

ADV-P15-002 (MEDIUM): bc-3-issue-write.md body intro enumeration corrected — "7 subdomains" kept (matches 7 `### N.N` headings); 8-item list collapsed to 7 by merging Edit+Open under 3.4 (reflecting combined section header "### 3.4 Edit and Open").

Pre-Pass-16 body sweep across all 8 bc-*.md files:
- bc-1-auth-identity.md: DRIFT — body claimed "5 subdomains" but 6 `### N.N` headings present (1.1–1.6); corrected to "6 subdomains" with 1.6 Auth error handling listed.
- bc-2-issue-read.md: CLEAN — "6 subdomains" matches 6 headings; end-of-file "Total: 49" matches definitional_count: 49.
- bc-3-issue-write.md: FIXED (ADV-P15-001 + ADV-P15-002 above).
- bc-4-assets-cmdb.md: CLEAN — "4 subdomains" matches 4 headings; no end-of-file total line.
- bc-5-boards-sprints.md: CLEAN — "4 subdomains" matches 4 headings; no end-of-file total line.
- bc-6-config-cache.md: CLEAN — "3 subdomains" matches 3 headings; no end-of-file total line.
- bc-7-output-render.md: CLEAN — "5 subdomains" matches 5 headings; no end-of-file total line.
- cross-cutting.md: CLEAN — no `### N.N` subdomains (uses `### X.N` style with 0 matches); no end-of-file total line.

### Details

| Agent | Task | Output |
|-------|------|--------|
| state-manager | Persist adv-p1-pass15.md (2 findings: 1H/1M) | cycles/cycle-001/adversarial-reviews/adv-p1-pass15.md |
| state-manager | ADV-P15-001 fix: bc-3 end-of-file "40" → "48 individually-bodied" | specs/prd/bc-3-issue-write.md |
| state-manager | ADV-P15-002 fix: bc-3 intro 8-item list → 7 items (Edit+Open merged under 3.4) | specs/prd/bc-3-issue-write.md |
| state-manager | Body sweep drift: bc-1 "5 subdomains" → "6 subdomains" (1.6 added) | specs/prd/bc-1-auth-identity.md |
| state-manager | Update STATE.md (counter 1/3 → 0/3, trajectory, checkpoint, steps); burst-log; commit | factory-artifacts |

---

## Burst 22 (2026-05-04)

**Agents dispatched:** state-manager, adversary
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass16.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Burst 22 — Pass 16 (state-manager + adversary): CLEAN-PASS. Counter 0/3 → 1/3. bc-*.md body sweep effective. 2 more consecutive clean passes needed.

No findings. CANONICAL-COUNTS.md arithmetic verified (541 grand total, 309 bodied + 232 range-collapsed); risk register 1+6+8+11=26 match header; cross-cutting 7-module / 6-invariant-table anchors correct; 4 MUST-FIX P0 BCs traceable across risk-register and ADR-0007..0010; 6 holdout BC anchors spot-checked — all resolve.

### Details

| Agent | Task | Output |
|-------|------|--------|
| adversary | Phase 1d adversarial spec review Pass 16 (CLEAN-PASS) | adv-p1-pass16.md (0 findings) |
| state-manager | Persist Pass 16 CLEAN-PASS; update STATE.md convergence counter 0/3 → 1/3; burst-log; commit | factory-artifacts (this commit) |

---

## Burst 23 (2026-05-04)

**Agents dispatched:** state-manager
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p1-pass17.md, architecture/security-decisions/SD-003-verbose-pii-redaction.md, specs/domain-spec/state-machines.md, specs/domain-spec/bc-04-assets-cmdb.md, specs/domain-spec/bc-06-config-cache.md, specs/domain-spec/bc-07-output-render.md, specs/prd/CANONICAL-COUNTS.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Burst 23 — Pass 17 + fixes: 3 findings (1H/2M), all FIXED. 4th counter reset (1/3 → 0/3 across 17 passes). Convergence asymptotic. Awaiting orchestrator decision on continuation strategy.

ADV-P17-001 FIXED (HIGH): SD-003-verbose-pii-redaction.md reference corrected R-H3 → R-M0 (risk-register.md). R-H3 is handle_open URL bug (post Pass 6 reclassification); R-M0 is canonical for verbose body PII.

ADV-P17-002 FIXED (MEDIUM): domain-spec/state-machines.md phantom NFR-R-NEW-3 replaced with NFR-O-B (refresh_oauth_token zero-callers — correct canonical NFR).

ADV-P17-003 FIXED (MEDIUM): L2 bc_count frontmatter synced to L3 total_bcs for 3 files: bc-04 (44→32), bc-06 (38→39), bc-07 (126→80). bc-01/02/03/05 were already aligned. CANONICAL-COUNTS.md updated with L2↔L3 alignment table.

### Details

| Agent | Task | Output |
|-------|------|--------|
| state-manager | Persist adv-p1-pass17.md (3 findings: 1H/2M) | cycles/cycle-001/adversarial-reviews/adv-p1-pass17.md |
| state-manager | ADV-P17-001: SD-003 R-H3 → R-M0 | architecture/security-decisions/SD-003-verbose-pii-redaction.md |
| state-manager | ADV-P17-002: state-machines NFR-R-NEW-3 → NFR-O-B | specs/domain-spec/state-machines.md |
| state-manager | ADV-P17-003: L2 bc_count bc-04/06/07 synced to L3 total_bcs | specs/domain-spec/bc-04-assets-cmdb.md, bc-06-config-cache.md, bc-07-output-render.md |
| state-manager | CANONICAL-COUNTS.md L2↔L3 alignment table added | specs/prd/CANONICAL-COUNTS.md |
| state-manager | Update STATE.md (counter 1/3 → 0/3, trajectory, checkpoint, steps); burst-log; commit | factory-artifacts |

## Burst 24 (2026-05-06) — Phase 2 Burst 1 archived (STATE.md 5-row overflow)

**Agents dispatched:** (archive operation only)
**Archived step:** "Phase 2 Burst 1 — STORY-INDEX + WAVE-PLAN + Wave 0 (7 stories) | story-writer | complete | 7 stories: 4 MUST-FIX bug fixes + S-0.05 #[cfg(test)] gate + S-0.06 --verbose-bodies + S-0.07 H-NEW-AUTH-002 holdout"

### Summary

Oldest row dropped from STATE.md Current Phase Steps to maintain 5-row limit per content routing rules.

## Burst 25 (2026-05-06) — Phase 2-adv Pass 2 + fixes

**Agents dispatched:** adversary, state-manager
**Files touched:** cycles/cycle-001/adversarial-reviews/adv-p2-pass2.md, stories/wave-2/S-2.02-bc-3-issue-write-holdout-suite.md, stories/wave-2/S-2.06-worklog-duration-and-cmdb-cache-tuple.md, stories/STORY-INDEX.md, STATE.md, cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Pass 2: 5 findings (0C/0H/3M/1L), all FIXED. Trajectory 14→5. Severity dropping. Counter 0/3.

ADV-P2-S2-001 FIXED (MED): S-2.02 AC-003 parenthetical still cited "H-021 is covered in S-1.06" — P1 sibling-text propagation gap. Updated to "H-021 is covered in S-2.01 AC-007" with correct BC differentiation (BC-2.1.013 vs BC-2.1.007).

ADV-P2-S2-002 FIXED (MED): STORY-INDEX H-018 row mis-anchored to BC-X.9.001 (JQL escape proptest). Corrected to BC-X.5.005 / BC-X.9.002 with test_complex at line 90.

ADV-P2-S2-003 FIXED (MED): STORY-INDEX S-2.06 row and S-2.06 frontmatter/body mis-anchored to BC-X.9.001 (JQL escape). Corrected to BC-X.5.009 across frontmatter bc_anchors, body Behavioral Contracts label, and all 4 AC trace-to annotations.

ADV-P2-S2-004 FIXED (LOW): STORY-INDEX H-017 row mis-anchored to BC-X.8.003 (project-meta cache). Corrected to BC-4.1.002 (AQL clause uses field NAME + capital Key).

### Details

| Agent | Task | Output |
|-------|------|--------|
| state-manager | Persist adv-p2-pass2.md (5 findings: 0C/0H/3M/1L) | cycles/cycle-001/adversarial-reviews/adv-p2-pass2.md |
| state-manager | ADV-P2-S2-001: S-2.02 AC-003 H-021 ref S-1.06 → S-2.01 AC-007 | stories/wave-2/S-2.02-bc-3-issue-write-holdout-suite.md |
| state-manager | ADV-P2-S2-002: STORY-INDEX H-018 BC-X.9.001 → BC-X.5.005/X.9.002, line 90 | stories/STORY-INDEX.md |
| state-manager | ADV-P2-S2-003: STORY-INDEX S-2.06 + S-2.06 story BC-X.9.001 → BC-X.5.009 (frontmatter + body label + 4 AC traces) | stories/STORY-INDEX.md, stories/wave-2/S-2.06-worklog-duration-and-cmdb-cache-tuple.md |
| state-manager | ADV-P2-S2-004: STORY-INDEX H-017 BC-X.8.003 → BC-4.1.002 | stories/STORY-INDEX.md |
| state-manager | Update STATE.md (pass 2 convergence, trajectory 14→5, checkpoint, steps); burst-log; commit | factory-artifacts |

---

## Burst: Phase 3 Wave 0 COMPLETE (2026-05-07)

**Agents dispatched:** devops-engineer (PRs #289-#294), state-manager (S-0.07 spec-only)
**Files touched:** (source code via PRs on develop; factory artifacts via state-manager direct) .factory/specs/prd/holdout-scenarios.md, .factory/sprint-state.yaml, .factory/stories/WAVE-PLAN.md, .factory/STATE.md, .factory/cycles/cycle-001/lessons.md
**PRs merged to develop:** #289 (S-0.01), #290 (S-0.02), #291 (S-0.03), #292 (S-0.04), #293 (S-0.05), #294 (S-0.06)

### Wave 0 Delivery Summary

All 7 Wave 0 stories complete. 6 via PRs to develop; 1 (S-0.07) spec-only on factory-artifacts:

| Story | Type | PR | develop SHA | Holdouts |
|-------|------|----|-------------|---------|
| S-0.01 | MUST-FIX | #289 | b7b9c9c | H-046 MUST-PASS |
| S-0.02 | MUST-FIX | #290 | a84e063 | H-045 MUST-PASS |
| S-0.03 | MUST-FIX | #291 | cb2c612 | H-036 MUST-PASS |
| S-0.04 | MUST-FIX | #292 | dbbea12 | H-NEW-MP-001 MUST-PASS |
| S-0.05 | SD-implementation | #293 | d907504 | H-NEW-AUTH-002 (gated) |
| S-0.06 | SD-implementation | #294 | 06ecd6a | H-NEW-VERBOSE-001/002 MUST-PASS |
| S-0.07 | holdout | factory-artifacts direct | (no develop PR) | H-NEW-AUTH-002 formalized |

### S-0.07 Delivery Details

- H-NEW-AUTH-002 appended to holdout-scenarios.md (v1.1.1, total_holdouts=51)
- Group 8 added: "SD-002 Release Binary Auth Gate"
- sprint-state.yaml: S-0.07 → completed; wave_0_progress: 7/7 COMPLETE; wave_1: active
- WAVE-PLAN.md: Wave 0 COMPLETE, Wave 1 ACTIVE (v1.2.0)
- STATE.md: compacted Phase 3 row; session checkpoint updated; wave_0 archived

### Wave 0 Metrics

- 6 PRs merged to develop (#289-#294)
- 1 spec-only delivery on factory-artifacts (S-0.07)
- Total deferred findings: 5 (R1-001, R1-002, S-0.03-S1, S-0.05-F1/F2/F3, S-0.05-DEV resolved)
- Tests added: ~40 new (issue_open + worklog_commands + issue_list_assets + multi_profile_fields + auth_header_release_gate + verbose_bodies + 2 cli_handler rewrites)
- 0 production regressions; subprocess test compat preserved through Option B canonization
- 3 new lessons learned (PR deviations tracking; dispatcher block pattern; clippy version skew)

---

## Burst: S-2.03 DELIVERED — BC-4 assets/CMDB holdout suite (2026-05-08)

**Agents dispatched:** test-writer → demo-recorder → devops-engineer (push + pr-manager) → devops-engineer (worktree cleanup) → state-manager
**Files touched (develop):** `tests/asset_holdouts.rs` (417 lines, new), `docs/demo-evidence/S-2.03/` (evidence-report.md, combined-transcript.txt, AC-003-ambiguous-status.tape/.gif/.webm)
**Commits:** dd5c41f (tests/asset_holdouts.rs — 3 regression-pin tests), 212a237 (demo evidence)
**Squash-merge SHA:** e9c2ba8 (PR #305 squash-merged to develop, 2026-05-08)
**Files touched (factory):** STATE.md, sprint-state.yaml, stories/STORY-INDEX.md, cycles/cycle-001/burst-log.md, cycles/cycle-001/implementation/red-gate-log.md

### Summary

S-2.03 (BC-4 assets/CMDB regression holdout suite) delivered and merged via PR #305 to develop at squash SHA e9c2ba8. This is a regression-pin holdout story: 3 tests were written in `tests/asset_holdouts.rs` against existing correct production behavior at activation HEAD dea1664. Tests PASS on first run by design — they pin the existing behavior rather than driving new code (no production code changes). 8/8 CI green. Review: APPROVE, 1 cycle, 0 blocking findings. Worktree and local/remote branch `test/S-2.03-bc-4-asset-enrichment-holdout-suite` fully cleaned up. 1 LOW deferred: S-2.03-DOC-01 (story spec line ~123 names cache file `workspace_id.json` but actual filename in `src/cache.rs` and tests is `workspace.json`; tests are correct; spec text needs follow-up doc PR).

Wave 2: 3/7 merged (S-2.01, S-2.02, S-2.03). Phase 3 progress: 19/31 (61%). Active story: S-2.04.

### Delivery Details

| Agent | Task | Output |
|-------|------|--------|
| test-writer | Write 3 regression-pin tests for H-037/H-038/H-039 in `tests/asset_holdouts.rs` | dd5c41f (417 lines); passes green against unmodified production code |
| demo-recorder | Record demo evidence for S-2.03 ACs | 212a237; `docs/demo-evidence/S-2.03/` (evidence-report.md, combined-transcript.txt, AC-003-ambiguous-status.tape/.gif/.webm) |
| devops-engineer | Push branch, create PR #305, merge --squash --delete-branch | e9c2ba8 squash-merge SHA on develop; remote branch deleted |
| devops-engineer | Worktree cleanup | Worktree removed; local branch `test/S-2.03-bc-4-asset-enrichment-holdout-suite` deleted |
| state-manager | Update STATE.md, sprint-state.yaml, STORY-INDEX.md, burst-log.md, red-gate-log.md; commit factory-artifacts | This commit |

### H-038 Placement Note

H-038 pins `enrich_assets` (BC-4.3.002 — asset enrichment join_all concurrency). `enrich_assets` is declared `pub` in `src/cli/assets.rs` and re-exported via the `pub mod api` chain in `src/lib.rs`. A library-level integration test in `tests/asset_holdouts.rs` is the correct placement for this function — no workaround or special access was required.

### Deferred

| ID | Description | Severity |
|----|-------------|----------|
| S-2.03-DOC-01 | Story spec line ~123 names workspace cache file `workspace_id.json` but actual filename per `src/cache.rs` and tests is `workspace.json`. Tests use correct filename. Update story spec in a follow-up doc PR. | LOW |

---

## Burst: S-2.04 DELIVERED — BC-5/7 boards, sprints, and ADF rendering holdout suite (2026-05-08)

**Agents dispatched:** devops-engineer (worktree → test-writer) → demo-recorder → devops-engineer (push + pr-manager) → devops-engineer (worktree cleanup) → state-manager
**Files touched (develop):** `tests/boards_sprints_holdouts.rs` (770 lines, new), `docs/demo-evidence/S-2.04/` (evidence-report.md, combined-transcript.txt, AC-004-kanban-error.tape/.gif/.webm, AC-007-adf-rendering.tape/.gif/.webm)
**Commits:** e71a61e (tests/boards_sprints_holdouts.rs — 9 regression-pin tests), 893d45a (demo evidence)
**Squash-merge SHA:** ada9126 (PR #306 squash-merged to develop, 2026-05-08)
**Files touched (factory):** STATE.md, sprint-state.yaml, stories/STORY-INDEX.md, cycles/cycle-001/burst-log.md, cycles/cycle-001/implementation/red-gate-log.md
**Code-delivery artifacts:** `.factory/code-delivery/S-2.04/pr-description.md`, `.factory/code-delivery/S-2.04/review-findings.md`

### Summary

S-2.04 (BC-5/7 boards/sprints/ADF rendering regression holdout suite) delivered and merged via PR #306 to develop at squash SHA ada9126. This is a regression-pin holdout story: 9 tests were written in `tests/boards_sprints_holdouts.rs` against existing correct production behavior at activation HEAD dea1664/ada9126. Tests PASS on first run by design — they pin the existing behavior rather than driving new code (no production code changes, no dev-deps added). 8/8 CI green. Review: APPROVE, 1 cycle, 0 blocking findings. Worktree and local/remote branch fully cleaned up. 3 LOW deferred items recorded.

Holdouts covered: H-040 (board list pagination — split into 3 cases), H-041 (board view sprint state), H-042 (sprint list scrum board), H-043 (sprint current team+points — split into 2 cases), H-044 (ADF→text rendering). Total: 5 holdouts → 9 tests.

BCs pinned: BC-5.2.001, BC-5.2.005, BC-5.2.007, BC-5.2.008, BC-5.3.001, BC-5.3.002, BC-7.2.001 (7 BCs).

Wave 2: 4/7 merged (S-2.01, S-2.02, S-2.03, S-2.04). Phase 3 progress: 20/31 (65%). Active story: S-2.05.

### Delivery Details

| Agent | Task | Output |
|-------|------|--------|
| test-writer | Write 9 regression-pin tests for H-040..H-044 in `tests/boards_sprints_holdouts.rs` | e71a61e (770 lines); passes green against unmodified production code |
| demo-recorder | Record demo evidence for S-2.04 ACs | 893d45a; `docs/demo-evidence/S-2.04/` (evidence-report.md, combined-transcript.txt, AC-004-kanban-error.tape/.gif/.webm, AC-007-adf-rendering.tape/.gif/.webm) |
| devops-engineer | Push branch, create PR #306, merge --squash --delete-branch | ada9126 squash-merge SHA on develop; remote branch deleted |
| devops-engineer | Worktree cleanup | Worktree removed; local branch deleted |
| state-manager | Update STATE.md, sprint-state.yaml, STORY-INDEX.md, burst-log.md, red-gate-log.md; commit factory-artifacts | This commit |

### Deferred

| ID | Description | Severity |
|----|-------------|----------|
| S-2.04-DEFER-01 | Story spec AC-004 quotes kanban literal as prefix only ('Sprint commands are only available for scrum boards'); production code at src/cli/sprint.rs:80-85 emits prefix + suffix '. Board {id} is a {type} board.'. Test uses contains(prefix) — robust against suffix changes. Update story spec text in follow-up doc PR. | LOW |
| S-2.04-DEFER-02 | Story spec H-043 implementation notes use 'displayName' for team-cache JSON shape; actual jr::cache::CachedTeam struct uses 'name'. Test uses production struct directly — cannot drift. Update story spec text in follow-up doc PR. | LOW |
| S-2.04-DOC-01 | Pre-existing: tests/team_column_parity.rs::write_team_cache writes to $XDG_CACHE_HOME/jr/teams.json (missing v1/default/ segment). Canonical path per src/cache.rs:90-92 is $XDG_CACHE_HOME/jr/v1/default/teams.json. Existing tests pass coincidentally. Not introduced by S-2.04. Target: separate fix story. | LOW |

---

## Burst: S-2.05 DELIVERED — CLAUDE.md documentation update for 6 NFRs + bonus NFR-O-H (2026-05-08)

**Agents dispatched:** devops-engineer (worktree → implementer) → devops-engineer (push + pr-manager) → devops-engineer (worktree cleanup) → state-manager
**Files touched (develop):** `CLAUDE.md` (+35 lines), `src/api/jira/users.rs` (+9 lines), `src/api/jira/issues.rs` (+7 lines) — 51 insertions / 0 deletions total
**Commit (feature branch):** 594f00c
**Squash-merge SHA:** 7f004ca (PR #307 squash-merged to develop, 2026-05-08)
**Files touched (factory):** STATE.md, sprint-state.yaml, stories/STORY-INDEX.md, cycles/cycle-001/burst-log.md, cycles/cycle-001/implementation/red-gate-log.md

### Summary

S-2.05 (CLAUDE.md documentation update for NFR-O-L/M/O/V/R + NFR-R-F gap + bonus NFR-O-H) delivered and merged via PR #307 to develop at squash SHA 7f004ca. This is a documentation-only story: no production behavior was changed, no tests were added, no dev-deps were added. Cargo.toml and Cargo.lock are unchanged.

NFRs resolved as DOCUMENT-AS-IS: NFR-O-L (orphan module entries in CLAUDE.md), NFR-O-M (module-to-file mapping accuracy), NFR-O-O (source-comment coverage), NFR-O-V (source comment function references), NFR-O-R (source comment references use function names not line numbers), NFR-R-F (retry-after cap gap documented). Bonus NFR-O-H (source comment style convention) also included.

Source comments added to `search_users_all` and `search_assignable_users_by_project_all` in `src/api/jira/users.rs`, and to `get_changelog`, `search_issues`, and `filter_tickets` in `src/api/jira/issues.rs`. All comments reference function names (not line numbers) per the Architecture Compliance Rules in the story spec. CLAUDE.md updated with descriptions for orphan modules.

**Explicit deviation — no test-writer phase, no demo-recorder phase:** This story is documentation-only. The Red Gate concept does not apply (there are no tests to fail or behavior to verify). The PR body itself is the evidence, with embedded grep checks confirming every AC. No `docs/demo-evidence/S-2.05/` directory was created; this is a deliberate and correct deviation, not a missing artifact.

8/8 CI green. Review: APPROVE, 1 cycle, 0 blocking findings. 1 LOW suggestion deferred (S-2.05-DEFER-01). Worktree and local/remote branch fully cleaned up.

Wave 2: 5/7 merged (S-2.01, S-2.02, S-2.03, S-2.04, S-2.05). Phase 3 progress: 21/31 (68%). Active story: S-2.06.

### Delivery Details

| Agent | Task | Output |
|-------|------|--------|
| devops-engineer (worktree) | Create worktree + implementer dispatch | Worktree for `docs/S-2.05-claude-md-documentation-update` branch |
| implementer | Add orphan module entries to CLAUDE.md; add source comments to users.rs + issues.rs using function names | 594f00c (51 insertions / 0 deletions across 3 files) |
| devops-engineer (push + pr-manager) | Push branch, create PR #307, request review, merge --squash --delete-branch | 7f004ca squash-merge SHA on develop; remote branch deleted |
| devops-engineer (worktree cleanup) | Remove worktree, delete local branch | Worktree removed; local branch deleted |
| state-manager | Update STATE.md, sprint-state.yaml, STORY-INDEX.md, burst-log.md, red-gate-log.md; commit factory-artifacts | This commit |

### NFRs Resolved

| NFR ID | Resolution | Mechanism |
|--------|-----------|-----------|
| NFR-O-L | DOCUMENT-AS-IS | Orphan module entries added to CLAUDE.md architecture tree |
| NFR-O-M | DOCUMENT-AS-IS | Module-to-file mapping in CLAUDE.md updated/verified |
| NFR-O-O | DOCUMENT-AS-IS | Source comment coverage added to users.rs + issues.rs |
| NFR-O-V | DOCUMENT-AS-IS | Comments reference function names (not line numbers) |
| NFR-O-R | DOCUMENT-AS-IS | Architecture Compliance Rules enforced: function-name references |
| NFR-R-F | DOCUMENT-AS-IS (gap documented) | Retry-After cap gap noted in source comment |
| NFR-O-H (bonus) | DOCUMENT-AS-IS | Source comment style convention confirmed |

### Deferred

| ID | Description | Severity |
|----|-------------|----------|
| S-2.05-DEFER-01 | CLAUDE.md `list.rs` description still reads 'list + view + comments (read operations, unified JQL composition)'. After S-2.05, `view.rs` and `comments.rs` are now separately documented sibling modules. Pre-existing text; out of scope for S-2.05. Target: bundle into a future small CLAUDE.md cleanup PR. | LOW |

---

## Burst: S-2.06 DELIVERED — Worklog timeSpent server-side parsing + CMDB cache tuple pin (2026-05-08)

**Story:** S-2.06 (v2.0.0 — pivoted from v1.0.0 after Perplexity verification 2026-05-08)
**Agents dispatched:** research-agent (Perplexity) → story-writer (v2.0.0 pivot) → devops-engineer (worktree) → test-writer (Red Gate) → implementer (Green Gate) → demo-recorder → devops-engineer (push + pr-manager) → devops-engineer (worktree cleanup) → state-manager
**Files touched (develop):** `tests/worklog_duration_holdouts.rs` (+589, NEW), `src/duration.rs` (+76), `src/api/jira/worklogs.rs` (+8, -4), `src/cli/worklog.rs` (+3, -3), `tests/worklog_commands.rs` (+1, -1), `tests/common/fixtures.rs` (extended), `docs/demo-evidence/S-2.06/` (evidence-report.md, combined-transcript.txt, AC-003-invalid-duration-rejected.tape/.gif/.webm)
**Commits (feature branch):** b3d2500 (Red Gate tests), 3d5a6ca (impl: parse_duration_validate + timeSpent passthrough), 15f509c (un-gate AC-004), a5b64a2 (fixup worklog_commands test + fmt), 1d88d07 (demo evidence)
**Squash-merge SHA:** c8f15d8 (PR #308 squash-merged to develop, 2026-05-08)
**Files touched (factory):** STATE.md, sprint-state.yaml, stories/STORY-INDEX.md, cycles/cycle-001/burst-log.md, cycles/cycle-001/implementation/red-gate-log.md

### Pivot Narrative (v1.0.0 → v2.0.0)

v1.0.0 of this story proposed fetching Jira's timetracking configuration via `/configuration/timetracking` to normalize `timeSpentSeconds` into a per-instance-correct integer. Research-agent (Perplexity) verified the approach on 2026-05-08 and found four blocking problems: the endpoint returns provider configuration, not hours-per-day or days-per-week settings; the field names in the Jira REST API docs are `workingHoursPerDay`/`workingDaysPerWeek`, not the names in the v1 spec; the field types are float64, not u32; and the endpoint is admin-only (v1 spec assumed non-admin). User chose **Option 1: timeSpent string passthrough** — pass the raw duration string (e.g., `"2h"`, `"1d"`, `"2d 3h 30m"`) directly as the `timeSpent` field, letting Jira's server parse it against its own instance config. This matches the `ankitpokhrel/jira-cli` pattern and eliminates the admin endpoint and cache dependencies entirely. v2.0.0 spec written by story-writer and committed to factory-artifacts at 37a4be6. Verification report at `.factory/research/S-2.06-jira-timetracking-verification.md`.

### Summary

S-2.06 (v2.0.0) delivered and merged via PR #308 to develop at squash SHA c8f15d8. This is the FIRST Wave 2 story with a production code change (all prior Wave 2 stories were regression-pin holdout suites or documentation-only). The story resolves NFR-R-C — the pre-existing hardcoded 8h/5d assumption in `add_worklog` — by switching the POST body from `{"timeSpentSeconds": <computed number>}` to `{"timeSpent": "<raw string>"}`. Jira's server applies its own `workingHoursPerDay`/`workingDaysPerWeek` instance configuration at parse time, making the behaviour correct on all Jira instances without any admin-level API call.

Wire-protocol change: `timeSpentSeconds` (number) → `timeSpent` (string). Invisible to end users; visible to anyone proxying requests. End-user impact: inputs previously computed incorrectly on customized Jira instances (e.g., 7.5h/day, 4-day week) now resolve correctly. AC-002 makes `"2d 3h 30m"` (space-separated compound) valid input — strict superset of prior accepted formats, no regression.

True Red Gate at b3d2500: AC-001/002/003 FAIL for behavioral reasons; AC-004 COMPILE-ERROR (gated with `#[cfg(any())]` because `parse_duration_validate` not yet defined); AC-005/006 PASS (inverted pin — CMDB graceful-degradation already correct). Green Gate at a5b64a2: 6/6 pass, 0 regressions across 614 unit + integration suites.

8/8 CI green. Review: APPROVE, 1 cycle, 0 blocking findings, 2 nits non-blocking → 3 LOW deferred items. Worktree and local/remote branch fully cleaned up.

Wave 2: 6/7 merged (S-2.01, S-2.02, S-2.03, S-2.04, S-2.05, S-2.06). Phase 3 progress: 22/31 (71%). Active story: S-2.07 (LAST Wave 2 story).

### Delivery Details

| Agent | Task | Output |
|-------|------|--------|
| research-agent (Perplexity) | Verify Jira timetracking API — endpoint correctness, field names, types, auth | Verification report: .factory/research/S-2.06-jira-timetracking-verification.md; v1.0.0 BLOCKED |
| story-writer | Write v2.0.0 spec (Option 1 pivot: timeSpent string passthrough) | .factory/stories/wave-2/S-2.06-... updated; committed to factory-artifacts 37a4be6 |
| devops-engineer | Create worktree + test-writer dispatch | Worktree for S-2.06 feature branch |
| test-writer | Write 6 Red Gate tests (AC-001..AC-006) | b3d2500 (tests/worklog_duration_holdouts.rs, +589); AC-001/002/003 FAIL; AC-004 compile-error gated; AC-005/006 PASS (inverted pin) |
| implementer | add parse_duration_validate + timeSpent passthrough; un-gate AC-004; fixup | 3d5a6ca (impl), 15f509c (un-gate AC-004), a5b64a2 (worklog_commands test update + fmt fix); all 6 ACs green |
| demo-recorder | Record demo evidence for AC-003 (invalid duration rejection) | 1d88d07; docs/demo-evidence/S-2.06/ (evidence-report.md, combined-transcript.txt, AC-003-invalid-duration-rejected.tape/.gif/.webm) |
| devops-engineer | Push branch, create PR #308, request review, merge --squash --delete-branch | c8f15d8 squash-merge SHA on develop; remote branch deleted |
| devops-engineer | Worktree cleanup | Worktree removed; local branch deleted |
| state-manager | Update STATE.md, sprint-state.yaml, STORY-INDEX.md, burst-log.md, red-gate-log.md; commit factory-artifacts | This commit |

### BCs and NFR Resolved

| Anchor | Resolution |
|--------|-----------|
| BC-X.5.009 | RESOLVED — add_worklog POST body uses timeSpent string; server parses per instance config |
| BC-6.2.013 | RESOLVED — CMDB cache tuple format pinned (AC-005/AC-006 regression pin; no change needed) |
| NFR-R-C | RESOLVED — timeSpent string passthrough eliminates hardcoded 8h/5d assumption |

### Deferred

| ID | Description | Severity |
|----|-------------|----------|
| S-2.06-DEFER-01 | src/duration.rs::parse_duration calculator preserved with SUPERSEDED-BY comment because format_duration round-trip proptest still uses it. If format_duration is later removed/refactored, the calculator can be deleted. Target: future cleanup story. | LOW |
| S-2.06-DEFER-02 | tests/worklog_duration_holdouts.rs AC-003 stderr OR-chain assertion is lenient (passes if any one of Nw/Nd/Nh/Nm appears). Could be tightened to require all four substrings. Target: future test cleanup. | LOW |
| S-2.06-DEFER-03 | src/duration.rs:65 !found_any guard reachability is constrained by prior guards — logically sound but slightly defensive. No action needed. | LOW |

---

## Burst: S-2.07 DELIVERED — Auth --output json (4 subcommands) + verb-aligned JSON policy + test naming (2026-05-08)

**Story:** S-2.07 (v2.0.0 — pivoted from v1.0.0 after Perplexity verification 2026-05-08)
**Agents dispatched:** research-agent (Perplexity + WebSearch + WebFetch) → story-writer (v2.0.0 pivot) → technical-writer (retroactive S-2.06 sweep: H-018 fix in holdout-scenarios.md, closes S-2.02-DEFER) → story-writer (H-018 replacement + S-3.10 queue) → devops-engineer (worktree) → test-writer (Red Gate tests) → implementer (Green Gate) → demo-recorder → devops-engineer (push + pr-manager) → devops-engineer (worktree cleanup) → state-manager
**Files touched (develop):** `src/cli/auth.rs` (+205, -9), `src/main.rs` (+12, -4), 4 snapshot files (auth_login_json.snap, auth_switch_json.snap, auth_logout_json.snap, auth_remove_json.snap — all new), `tests/auth_output_json.rs` (new, 363 lines), `docs/specs/json-output-shapes.md` (new, 41 lines), `docs/specs/test-naming-convention.md` (new, 41 lines), `CLAUDE.md` (+1 bullet), `docs/demo-evidence/S-2.07/` (8 artifacts)
**Commits (feature branch → squash):** 6348037 (Red Gate tests — auth_output_json.rs + refresh regression-pin), 082169a (impl — auth.rs + main.rs), 9f456d9 (snapshots — cargo insta accept), cd69fd6 (json-output-shapes spec), ae38093 (test-naming-convention spec), d445b7c (CLAUDE.md bullet), 23227a9 (demo evidence)
**Squash-merge SHA:** ca22be0 (PR #309 squash-merged to develop, 2026-05-08)
**Files touched (factory):** STATE.md, sprint-state.yaml, stories/STORY-INDEX.md, cycles/cycle-001/burst-log.md, cycles/cycle-001/implementation/red-gate-log.md

### Pivot Narrative (v1.0.0 → v2.0.0)

v1.0.0 of this story contained three concrete errors discovered by research-agent (Perplexity + WebSearch + WebFetch) on 2026-05-08:

1. **AC-002 wiremock premise structurally untestable** — `jr auth refresh` re-runs the full OAuth 3LO flow via `login_oauth`, never calling a refresh-token API endpoint. The v1 spec's wiremock fixture for a `/oauth/token` refresh response was architecturally impossible to trigger from the current implementation.

2. **NFR-O-F shape conflict** — v1 prescribed a uniform `{profile, action, ok}` shape for all auth subcommands including `refresh`. But `auth refresh` had already shipped a distinct `{status, auth_method, next_step}` shape in a pre-existing `refresh_success_payload` helper. Forcing refresh to emit the uniform shape would be a silent behavior regression on already-shipped output.

3. **AC-005 `transitioned` vs `changed` ambiguity** — Verified at `src/cli/issue/json_output.rs:4-10` that the canonical field name is `changed`, not `transitioned`. This also resolved S-2.02-DEFER, which had been open since the issue-write holdout suite story.

User chose **Option A: apply all 3 corrections**. v2.0.0 spec written and committed to factory-artifacts. Verification report at `.factory/research/S-2.07-json-policy-and-conventions-research.md`.

### Summary

S-2.07 (v2.0.0) delivered and merged via PR #309 to develop at squash SHA ca22be0. This is the second Wave 2 story with a production code change (the first being S-2.06) and the LAST Wave 2 story.

Behavioral delta: `jr auth login/switch/logout/remove --output json` now each emit `{"profile": "<name>", "action": "<verb>", "ok": true}` to stdout. `jr auth refresh --output json` retains its existing asymmetric shape `{"status": "refreshed", "auth_method": "<method>", "next_step": "<desc>"}` — this asymmetry is intentional (refresh triggers re-auth, not a state mutation) and is documented in the new `docs/specs/json-output-shapes.md` shapes registry.

AC-003 (auth JSON error path) was already satisfied by `main.rs`'s existing `--output json` error wrapper — all propagated `JrError` values get `{"error": "<msg>", "code": <N>}` to stderr. This was confirmed as already-working and documented as S-2.07-DEFER-01.

New spec docs shipped: `docs/specs/json-output-shapes.md` (canonical JSON output shapes registry, 41 lines) and `docs/specs/test-naming-convention.md` (naming convention for all test functions, 41 lines). Both referenced from CLAUDE.md (1-line addition).

True Red Gate at 6348037 (before implementation):
- 4 process-spawn tests (auth_output_json.rs: login, switch, logout, remove) — FAILED with assertion errors (handlers not yet emitting JSON)
- 4 snapshot tests (cli::auth::tests in auth.rs) — FAILED (snapshot files did not exist; insta wrote `.snap.new`)
- 2 refresh regression-pin unit tests — PASSED (helper already shipped)
- 1 unexpected pass: `test_auth_switch_unknown_profile_returns_json_error` — already PASSED on develop (S-2.07-DEFER-01 confirmed: main.rs error wrapper was already active)

Green Gate at 23227a9 (after implementation): 5/5 process-spawn pass; 4/4 snapshot tests pass (after `cargo insta accept` at 9f456d9); 2/2 refresh regression-pin tests still pass. Full lib suite: 620 passed, 0 failed, 10 ignored. Clippy clean, fmt clean.

8/8 CI green. Review: APPROVE, 1 cycle, 0 blocking findings, 2 non-blocking nits → 2 LOW deferred items. Worktree and local/remote branch fully cleaned up.

Wave 2: 7/7 merged (S-2.01 through S-2.07). **Wave 2 COMPLETE.** Phase 3 progress: 23/31 (74%). Next: Wave 2 Integration Gate.

### Delivery Details

| Agent | Task | Output |
|-------|------|--------|
| research-agent | Verify S-2.07 v1 spec — AC-002 wiremock architecture, NFR-O-F shape conflict, AC-005 field name, AC-006 snapshot reuse | Verification report: .factory/research/S-2.07-json-policy-and-conventions-research.md; v1.0.0 3 errors found |
| story-writer | Write v2.0.0 spec (Option A: 3 corrections) | .factory/stories/wave-2/S-2.07-... updated; committed to factory-artifacts |
| technical-writer | Retroactive S-2.06 sweep: replace H-018 inline in holdout-scenarios.md (Option 2); queue S-3.10 in STORY-INDEX + sprint-state.yaml | H-018 holdout replaced in place; S-2.02-DEFER resolved (changed confirmed); S-3.10 queued |
| devops-engineer | Create worktree + test-writer dispatch | Worktree for S-2.07 feature branch |
| test-writer | Write Red Gate tests (6348037) | tests/auth_output_json.rs (363 lines): 4 process-spawn + 2 refresh pin; 4 snapshot tests in auth.rs |
| implementer | Add --output json handlers to 4 auth subcommands + snapshot accepted | 082169a (auth.rs +205/-9, main.rs +12/-4); 9f456d9 (cargo insta accept: 4 .snap files) |
| technical-writer | Write new spec docs | cd69fd6 (json-output-shapes.md); ae38093 (test-naming-convention.md); d445b7c (CLAUDE.md +1) |
| demo-recorder | Record demo evidence for S-2.07 ACs | 23227a9; docs/demo-evidence/S-2.07/ (8 artifacts) |
| devops-engineer | Push branch, create PR #309, request review, merge --squash --delete-branch | ca22be0 squash-merge SHA on develop; remote branch deleted |
| devops-engineer | Worktree cleanup | Worktree removed; local branch deleted |
| state-manager | Update STATE.md, sprint-state.yaml, STORY-INDEX.md, burst-log.md, red-gate-log.md; commit factory-artifacts | This commit |

### BCs and NFRs Resolved

| Anchor | Resolution |
|--------|-----------|
| BC-7.3.004 | RESOLVED — auth login/switch/logout/remove emit {profile, action, ok: true} under --output json |
| BC-7.3.005 | RESOLVED — auth refresh retains asymmetric {status, auth_method, next_step} shape; documented |
| NFR-O-F | RESOLVED — all 5 auth subcommands have documented JSON output shapes |
| NFR-O-J | RESOLVED — json-output-shapes.md registry created as canonical reference |
| NFR-O-W | RESOLVED — test-naming-convention.md captures naming convention; CLAUDE.md updated |

### Deferred

| ID | Description | Severity |
|----|-------------|----------|
| S-2.07-DEFER-01 | src/main.rs: AC-003 (auth subcommand JSON error path) was already satisfied by main.rs's existing --output json error wrapper. Propagated JrError values get {"error","code"} to stderr. Documented in docs/specs/json-output-shapes.md as already-working. No action needed. | LOW |
| S-2.07-DEFER-02 | src/cli/auth.rs::mod tests: Pre-existing refresh_payload_pins_token_shape and refresh_payload_pins_oauth_shape tests already cover much of AC-002's ground. New tests are intentionally additive (more specific assertions). No action; intentional overlap. | LOW |

### Reviewer Nits (non-blocking, documented in review-findings)

1. Multi-line JSON output style — reviewer suggested collapsing multi-line assert_eq blocks to single-line. Non-blocking; style preference.
2. Serialization expect message — reviewer suggested more descriptive `.expect("should serialize")` messages. Non-blocking; style preference.

---

## Burst: WAVE 2 CLOSURE (2026-05-08)

**Date:** 2026-05-08
**Wave:** Wave 2 (S-2.01 through S-2.07)
**PRs:** #303 (S-2.01) → #304 (S-2.02) → #305 (S-2.03) → #306 (S-2.04) → #307 (S-2.05) → #308 (S-2.06) → #309 (S-2.07)
**Stories:** 7 stories, 7 merges, all on develop
**Integration Gate:** PENDING — orchestrator dispatches next

### Wave 2 Summary

Wave 2 ran 2026-05-08. All 7 stories delivered to develop in a single session. Story types:

| Story | Type | PR | SHA | Notable |
|-------|------|----|-----|---------|
| S-2.01 | Regression-pin holdout (BC-2 issue-read) | #303 | f6516f8 | 7 tests, 9 BCs, 7 holdouts |
| S-2.02 | Regression-pin holdout (BC-3 issue-write) | #304 | 7528960 | 4 tests, 4 BCs, 4 holdouts |
| S-2.03 | Regression-pin holdout (BC-4 assets/CMDB) | #305 | e9c2ba8 | 3 tests, 3 BCs, 3 holdouts |
| S-2.04 | Regression-pin holdout (BC-5/7 boards/sprints/ADF) | #306 | ada9126 | 9 tests, 7 BCs, 5 holdouts |
| S-2.05 | Documentation-only (6 NFRs + bonus NFR-O-H) | #307 | 7f004ca | No tests; grep verification |
| S-2.06 | Production code change (worklog timeSpent passthrough) | #308 | c8f15d8 | TRUE Red Gate; v2.0.0 pivot (DEC-010) |
| S-2.07 | Production code change (auth --output json + specs) | #309 | ca22be0 | TRUE Red Gate; v2.0.0 pivot (DEC-011) |

Total product commits squashed into the 7 PRs: approximately 24 commits (S-2.01: 2, S-2.02: 2, S-2.03: 2, S-2.04: 2, S-2.05: 1, S-2.06: 5, S-2.07: 7 = 21 tracked; plus a small number of doc/fix micro-commits).

### Design Pivots (Wave 2 Firsts)

Wave 2 is the first wave with **two mid-stream design pivots** driven by Perplexity verification:

- **DEC-010 (S-2.06 pivot):** v1 timetracking spec was wrong on endpoint, field names, types, and auth requirements. User chose Option 1 (timeSpent string passthrough — eliminates admin endpoint and cache). See `.factory/research/S-2.06-jira-timetracking-verification.md`.

- **DEC-011 (S-2.07 pivot):** v1 auth JSON spec had AC-002 wiremock premise structurally untestable, NFR-O-F shape conflicted with pre-shipped refresh shape, and AC-005 field name wrong. User chose Option A (3 corrections). See `.factory/research/S-2.07-json-policy-and-conventions-research.md`.

Both pivots were discovered through Perplexity-backed research rather than during implementation — this is the intended pattern (verify early, correct the spec, deliver to v2.0.0).

### Drift Items Active During Wave 2

Items **resolved** during Wave 2:
- S-2.02-DEFER: JSON field name reconciliation (`transitioned` vs `changed`) — RESOLVED 2026-05-08 by DEC-011 (verified `changed` at src/cli/issue/json_output.rs:4-10; holdout-scenarios.md corrected)
- S-1.05-AC-001: GitHub Secret Scanning PENDING_MANUAL — RESOLVED 2026-05-08 (user enabled via gh CLI)
- S-2.06-DEFER-01 (initial open): parse_duration calculator preserved → RESOLVED as Option 4 follow-up (S-3.10 queued, H-018 replaced in holdout-scenarios.md by technical-writer retroactive sweep)

Items **added** during Wave 2 (from S-2.01 through S-2.07):
- S-2.03-DOC-01 (LOW): workspace_id.json vs workspace.json spec text
- S-2.04-DEFER-01 (LOW): kanban literal prefix-only in spec
- S-2.04-DEFER-02 (LOW): displayName vs name in H-043 spec
- S-2.04-DOC-01 (LOW): pre-existing non-canonical test cache path
- S-2.05-DEFER-01 (LOW): list.rs description stale after S-2.05 module split
- S-2.06-DEFER-02 (LOW): AC-003 OR-chain assertion leniency
- S-2.06-DEFER-03 (LOW): duration.rs:65 !found_any guard
- S-2.07-DEFER-01 (LOW): AC-003 already-passed by main.rs wrapper
- S-2.07-DEFER-02 (LOW): refresh_payload_pins tests intentional overlap

All added items are LOW severity. None are blocking for Wave 3 dispatch.

### Wave 2 Integration Gate — PENDING

Per `per-story-delivery.md` Wave Integration Gate protocol (max 10 cycles):
1. Full `cargo test --all-targets` on merged develop (Wave 2 regression check)
2. Adversarial review of combined Wave 2 diff (fresh context, different model) — Phase 3-adv first pass
3. Holdout re-evaluation (H-020 + H-021 + all Wave 2 holdouts still green on merged develop)
4. Code-reviewer constructive review (architecture, patterns, completeness)
5. Security review (auth.rs surface change in S-2.07; duration.rs surface in S-2.06)
6. Consistency-validator (BC anchors, NFR anchors, holdout registration)

Gate status: PENDING. Orchestrator dispatches Phase 3-adv next.

---

## Burst: WAVE 2 INTEGRATION GATE — CLOSED (2026-05-08)

**Date:** 2026-05-08
**Wave:** Wave 2 (S-2.01 through S-2.07)
**Gate:** Wave 2 Integration Gate — CLOSED with verdict GATE-PASSES
**develop SHA at gate open:** ca22be0 (S-2.07 merge, pre-gate)
**develop SHA at gate close:** 6cb9994 (post-WV2-SEC-01 PR #310)
**factory-artifacts SHA at gate open:** 7fd17bf (Fix-PR B)
**factory-artifacts SHA at gate close:** b92ee5d (this commit)

### Gate Sequence Summary

| Step | Agent | Output | Commit |
|------|-------|--------|--------|
| (a) Test suite | orchestrator (Bash) | 1108 pass / 0 fail / 13 ignored on develop @ ca22be0 | n/a |
| (b) Adversary pass-01 | adversary | 12 findings (3 BLOCKING + 5 CONCERN + 4 NIT) | factory-artifacts `ded2210` |
| (c) Code-reviewer | code-reviewer | 11 findings (0 critical/high) | factory-artifacts `c6e798c` |
| (d) Security-reviewer | security-reviewer | LOW-RISK; 1 MEDIUM (WV2-SEC-01) + 2 LOW + 2 INFO | factory-artifacts `1c5201f` |
| (e) Consistency pass-01 | consistency-validator | 12 findings (1 BLOCKING + 7 DRIFT + 4 NIT) | factory-artifacts `4918e6e` |
| Decision research | research-agent | D1=A, D2=separate, D3=defer, D4=C | factory-artifacts (research doc only) |
| Fix-PR A (anchor sweep) | spec-steward | 8 files; new BC-7.4.013-016; DEC-012 | factory-artifacts `28b0f35` |
| Fix-PR B (NFR sweep) | spec-steward | nfr-catalog.md; 11 NFRs RESOLVED | factory-artifacts `7fd17bf` |
| WV2-SEC-01 fix | implementer + pr-manager | PR #310 squash-merged at `6cb9994` | develop `6cb9994` |
| Consistency pass-02 | consistency-validator | DRIFT-FOUND, GATE-PASSES; 3 new minor drift items | factory-artifacts `8ae5511` |
| Gate-close state update | state-manager | BC-INDEX/CANONICAL-COUNTS count fixup; WV2-SEC-01 RESOLVED notation; STATE.md/sprint-state finalized | factory-artifacts b92ee5d (this commit) |

### Fix-PR Summary

| Fix-PR | Agent | Files Changed | SHA | Key Changes |
|--------|-------|---------------|-----|-------------|
| Fix-PR A (anchor sweep) | spec-steward | 8 files | `28b0f35` | BC-7.3.004→BC-7.1.001 re-anchor in S-2.07 spec; BC-6.2.013→BC-6.2.006 in S-2.06; 4 new BCs (BC-7.4.013-016) created in bc-7-output-render.md; DEC-012 logged; WV2-CV-01/02/07 resolved |
| Fix-PR B (NFR sweep) | spec-steward | nfr-catalog.md | `7fd17bf` | 11 NFRs marked RESOLVED in routing table + Summary Table; WV2-CV-08 resolved |
| WV2-SEC-01 | implementer + pr-manager | src/duration.rs | `6cb9994` | MAX_DURATION_INPUT_LEN=64 guard + 2 regression-pin tests (PR #310) |
| Pass-02 consistency review | consistency-validator | (review doc only) | `8ae5511` | Verified all 4 BLOCKING resolved; found P2-CV-01/02/03 (minor count propagation) |

### Inline Drift Fixes (this commit — no additional PR)

| Finding | Fix Applied | Files |
|---------|------------|-------|
| P2-CV-01 | BC-INDEX.md body Section 7 header (80→84, 34→38) + summary table row (80→84, 34→38) + totals (541→545, 309→313) | .factory/specs/prd/BC-INDEX.md |
| P2-CV-02 | CANONICAL-COUNTS.md bc-7 rows (34→38, 80→84) + grand total (541→545, 309→313) + last_verified updated | .factory/specs/prd/CANONICAL-COUNTS.md |
| P2-CV-03 | WV2-SEC-01 RESOLVED postscript added to security review doc; WV2-SEC-01 row added to STATE.md Drift Items | .factory/cycles/cycle-001/security-reviews/wave-2-gate-security-review-pass-01.md; .factory/STATE.md |
| WV2-CV-05 | Phase 3 progress count corrected 23/31 (74%) → 22/31 (71%). Arithmetic: Wave 0(7)+Wave 1(8)+Wave 2(7)=22. Prior 23 was off-by-one. | .factory/STATE.md (Session Resume Checkpoint, Phase 3 row, Phase Progress) |

### Deferred Items (wave 2 gate close — not blocking)

| ID | Description | Target |
|----|-------------|--------|
| WV2-FIX-A-FOLLOWUP-01 | 11 auth test docstrings cite BC-7.3.004 (need develop-side PR to re-anchor to BC-7.4.013-016) | Next develop touch or Wave 3 doc-cleanup PR |
| WV2-FIX-A-FOLLOWUP-02 | 2 worklog test names embed bc_6_2_013 (need develop-side rename to bc_6_2_006) | Next develop touch or Wave 3 doc-cleanup PR |
| WV2-CV-03 | STORY-INDEX Wave 0/1 rows (15 stories) still show `draft` | Wave 3 doc-cleanup or S-3.06 sweep |
| WV2-CV-11 | H-018 BC field has `(post-S-2.06 v2.0.0)` non-standard annotation | S-3.10 delivery or Wave 3 cleanup |
| WV2-CV-12 | STATE.md S-0.05-F2 drift item shows `TO_VERIFY` without resolution target | Wave 3 dev touch |

### Gate Close State

- develop: `6cb9994` — post-WV2-SEC-01 (PR #310); 1109 pass / 0 fail / 13 ignored (adding 1 new test from PR #310)
- factory-artifacts: b92ee5d (this commit)
- Phase 3 progress (corrected): **22/31 (71%)** (Wave 0:7 + Wave 1:8 + Wave 2:7 = 22 of original 31 stories)
- Wave 3 scope: 10 stories (S-3.01..S-3.10), status `blocked` → unblocked by Wave 2 gate closure
- Next: Wave 3 first-story scoping and story-writer dispatch

---

## Archived Step: S-2.04 MERGED (archived from STATE.md Current Phase Steps on 2026-05-08)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-2.04 MERGED — Boards/sprints/ADF holdout suite | devops-engineer | complete | PR #306 squash-merged to develop at ada9126 (2026-05-08); 9 regression-pin tests for H-040..H-044 across 7 BCs; 8/8 CI green; APPROVE 1 cycle; 0 blocking; 3 LOW deferred (S-2.04-DEFER-01/-02 spec text + S-2.04-DOC-01 pre-existing path bug). Wave 2: 4/7. Phase 3: 20/31 (65%). |

## Archived Step: S-2.05 MERGED (archived from STATE.md Current Phase Steps on 2026-05-08)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-2.05 MERGED — CLAUDE.md doc update | devops-engineer | complete | PR #307 squash-merged to develop at 7f004ca (2026-05-08); doc-only — 6 NFRs DOCUMENT-AS-IS + bonus NFR-O-H; 51/0 insertions; 8/8 CI green; APPROVE 1 cycle; 0 blocking; 1 LOW deferred (S-2.05-DEFER-01). Wave 2: 5/7. Phase 3: 21/31 (68%). |

## Archived Step: S-2.06 MERGED (archived from STATE.md Current Phase Steps on 2026-05-09)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-2.06 MERGED — Worklog timeSpent passthrough + CMDB cache pin | devops-engineer | complete | PR #308 squash-merged to develop at c8f15d8 (2026-05-08); v2.0.0 pivot after Perplexity verification BLOCKED v1; production code change (NOT holdout-only); `parse_duration_validate` validator + `timeSpent` string passthrough resolves NFR-R-C without admin endpoint or cache; 6/6 ACs; 8/8 CI; APPROVE 1 cycle; 0 blocking; 3 LOW deferred (calculator preservation + 2 reviewer nits). Wave 2: 6/7. Phase 3: 22/31 (71%). |



## Archived Step: S-2.07 MERGED (archived from STATE.md Current Phase Steps on 2026-05-09)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-2.07 MERGED — Auth JSON + verb-aligned policy + test naming | devops-engineer | complete | PR #309 squash-merged to develop at ca22be0 (2026-05-08); v2.0.0 pivot after Perplexity verification (DEC-011); 4 auth subcommands now emit JSON; auth refresh asymmetric shape preserved; AC-003 already-passed by main.rs wrapper; 7 commits → squash; +6 tests (4 snapshots + 2 refresh regression-pin); 8/8 CI; APPROVE 1 cycle; 0 blocking; 2 LOW deferred (S-2.07-DEFER-01/02). **Wave 2 COMPLETE 7/7.** Phase 3: 22/31 (71%). |

## Archived Step: S-3.10 MERGED (archived from STATE.md Current Phase Steps on 2026-05-09)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-3.10 MERGED — format_roundtrip rewrite + parse_duration calculator deletion | deliver-story (full chain) | complete | PR #313 squash-merged to develop at f492e59 (2026-05-09). 117 LOC removed; 9 ACs delivered; 8/8 CI green; APPROVE 1 cycle; 0 blocking; demo evidence at docs/demo-evidence/S-3.10/. Spec changes at factory-artifacts@4250e2c. Wave 3: 1/10. **Unblocks S-3.07** (AC-NEW-B sequencing gate satisfied on develop). |

## Archived Step: S-3.06 MERGED (archived from STATE.md Current Phase Steps on 2026-05-09)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-3.06 MERGED — DRIFT-001 spec count checker | deliver-story (full chain) | complete | PR #314 squash-merged to develop at 01ba293 (2026-05-09). Facade-mode story: shell script (61 LOC) + CLAUDE.md addition + lessons-codification.md (factory-artifacts@4194611). 5/5 ACs delivered; 8/8 CI green; APPROVE 1 cycle; 0 blocking; 0 security findings. Demo evidence at docs/demo-evidence/S-3.06/. Wave 3: 2/10. |

## Archived Step: S-3.07 MERGED (archived from STATE.md Current Phase Steps on 2026-05-09)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-3.07 MERGED — Retry-After cap + profile name precision + JRACLOUD-94632 anti-loop | deliver-story (full chain) | complete | PR #315 squash-merged to develop at 6bce18c (2026-05-09). v2.0.0 (3 parts A/C/D; Part B conditionally dropped). 5 commits + companion factory-artifacts@d8dcf7a (H-027 + NFR routing flips). 8/8 CI green; APPROVE 1 cycle; 0 security findings. 6/7 ACs new behavior + AC-NEW-B sequencing gate satisfied (S-3.10 dependency confirmed on develop). Demo evidence at docs/demo-evidence/S-3.07/. Wave 3: 3/10. Phase 3 progress: 25/31 (81%). |

## Archived Step: S-3.05 MERGED (archived from STATE.md Current Phase Steps on 2026-05-09)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-3.05 MERGED — asset enrichment concurrency cap | deliver-story (full chain) | complete | PR #316 squash-merged to develop at 10e1db4 (2026-05-09). buffer_unordered(8) replaces join_all at 2 sites; new MAX_CONCURRENT_ASSET_FETCHES const. 4/4 ACs delivered (AC-002 timing-based per wiremock 0.6.5 constraint). 8/8 CI green; APPROVE 1 cycle; 0 security findings; 0 new deps. Demo evidence at docs/demo-evidence/S-3.05/. Wave 3: 4/10. Phase 3 progress: 26/31 (84%). |

## Archived Step: S-3.09 CLOSED (archived from STATE.md Current Phase Steps on 2026-05-09)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-3.09 CLOSED — PKCE deferral formally recorded | state-manager | complete | factory-artifacts direct commit (doc-only facade). NFR-S-A routing flip SECURITY-DECIDE → DEFER (per ADR-0013) at 3 occurrences in nfr-catalog.md + DEFER count increment. ADR-0013 + SD-001 verified pre-satisfied (no edits). No develop-branch impact. STORY-INDEX + sprint-state + STATE.md synced atomically. Wave 3: 5/10. Phase 3: 27/31 (87%). |

## Archived Step: S-3.08 MERGED (archived from STATE.md Current Phase Steps on 2026-05-09)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-3.08 MERGED — DOCUMENT-AS-IS LOW NFR closures (5 source + 6 CLAUDE entries) | deliver-story (full chain) | complete | PR #317 squash-merged to develop at fba47ad (2026-05-09). 6 docs commits + 1 demo commit (40c205c → c48bbc8). +36 LOC across 6 files (5 .rs + CLAUDE.md). 5/5 ACs delivered; 8/8 CI green; APPROVE 1 cycle; 0 security findings; 0 new deps. Verified canonical wording for NFR-O-T + NFR-O-I (Atlassian docs retrieved 2026-05-08). Companion factory-artifacts commit @ 79afb49 (catalog routing flips: 7 → DOCUMENT-AS-IS-COMPLETE, 4 → DEFER-DOCUMENTED). Demo evidence at docs/demo-evidence/S-3.08/. Wave 3: 6/10. Phase 3 progress: 28/31 (90%). |

## Archived Step: S-3.01 MERGED (archived from STATE.md Current Phase Steps on 2026-05-11)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-3.01 MERGED — cli/auth.rs shard-split (9 production files + tests/) | deliver-story (full chain) | complete | PR #319 squash-merged to develop at b20cfee (2026-05-09). 10 refactor micro-commits + 1 demo commit (857e7e6..f029ba3). Pure refactor: 2,245 LOC single-file split into 9 production modules (mod 121 / login 366 / keychain 256 / refresh 144 / status 140 / remove 129 / list 70 / switch 51 / logout 50) + consolidated tests/mod.rs (997 — excluded from AC-004 production-cap). Max prod shard 366 LOC < 800 cap. 6/6 ACs delivered (4 spec + 2 bonus); 8/8 CI green; APPROVE 1 cycle; 0 security findings; Cargo.lock unchanged. AC-002 over-satisfied: ZERO direct keyring::Entry in cli/auth/* (all keychain access delegates to api/auth.rs). AuthFlow → pub(crate) for cross-shard dispatch. Demo evidence at docs/demo-evidence/S-3.01/. Wave 3: 8/10. Phase 3 progress: 30/31 (97%). |

## Archived Step: S-3.04 MERGED (archived from STATE.md Current Phase Steps on 2026-05-11)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-3.04 MERGED — multi-cloudId disambiguation + H-047 elevation | deliver-story (full chain) | complete | PR #320 squash-merged to develop at b6ab77c (2026-05-09). 5 commits (7c83907 test-writer + bfbda6a feat + b84c940 demos + 1075dd9 fmt-fix + post-merge state sync). Real feature (medium): closes H-047 KNOWN-GAP via --cloud-id flag + dialoguer::Select prompt + --no-input exit-64 with actionable listing. All disambiguation output renders name + URL + cloudId. 8/8 ACs delivered (6 spec + 2 bonus); 12 new integration tests; 12/12 + 612/612 = no regression; 8/8 CI green; APPROVE 1 cycle; 0 security findings; Cargo.lock unchanged. BC-1.5.031 invariant preserved (callback URL fixed at 127.0.0.1:53682/callback; regression-pin test asserts). Test seams JR_OAUTH_TOKEN_URL/ACCESSIBLE_RESOURCES_URL/OAUTH_CODE added (test-only). Demo evidence at docs/demo-evidence/S-3.04/. Wave 3: 9/10. Phase 3 progress: 31/31 (100% original scope). |

## Archived Step: S-3.02 MERGED (archived from STATE.md Current Phase Steps on 2026-05-09)

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-3.02 MERGED — cli/assets.rs shard-split (5 module files) | deliver-story (full chain) | complete | PR #318 squash-merged to develop at 68092af (2026-05-09). 6 refactor commits + 1 demo commit (2f20052..c057ffd). Pure refactor: 1,055 LOC single-file split into 5 modules (mod 65 / search 158 / view 91 / tickets 285 / schemas 490 — all <600 LOC cap). 5/5 ACs delivered; 8/8 CI green; APPROVE 1 cycle; 0 security findings; Cargo.lock unchanged. 612/612 unit tests + H-037/H-038/H-039 holdouts intact. --open filter (color_name != "green") survived in tickets.rs. Demo evidence at docs/demo-evidence/S-3.02/. Wave 3: 7/10. Phase 3 progress: 29/31 (94%). |

---

## Burst: PR #351 MERGED + PR #352 Round 1 Triage/Apply/Push/Reply/Resolve/Re-request (2026-05-11)

**Date:** 2026-05-11T15:15:10Z–15:23:30Z
**Agents:** orchestrator + direct edits (no sub-agent dispatch)
**Input files touched:** CLAUDE.md (L211 gotcha), src/cli/mod.rs (L401 comment), tests/issue_bulk_pr2.rs (L554 inline comment)
**Output commits:** develop @ 3216ec2 (PR #351 merge), develop @ f42bfa5 (PR #352 Round 1 fix micro-commit)
**factory-artifacts commit:** this commit

### Summary

PR #351 (chore/test-hygiene-339-344-347) was merged by GitHub at 3216ec2
(2026-05-11T15:15:10Z), closing issues #339 and #344. Develop fast-forwarded
e480ff2→3216ec2. Local worktree `.worktrees/test-hygiene` removed; local branch
`chore/test-hygiene-round2-rebase` deleted; remote branch auto-deleted on merge.
Issue #347 deferred to PR #352.

PR #352 (chore/docs-cleanup-337-341-347) received Copilot Round 1 at
2026-05-11T15:17:14Z (3 inline comments, pre-round head 05c12cd). All 3 were valid
local-consistency findings; all fixed in one micro-commit
`docs(bulk): address Copilot review on PR #352` → head f42bfa5. Validation
strategy: local file verification (no Perplexity needed — all 3 claims were
internal-consistency questions about the repo's own files, not external API behavior).
Pre-push CI-equivalent: `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` +
`cargo test` (612 unit + 38 bulk + all suites) all green. Remote CI settled 8/8 green at
2026-05-11T15:23:08Z. Three review threads resolved via GraphQL `resolveReviewThread`
mutation (PRRT_kwDORs-xfc6BIW9e, PRRT_kwDORs-xfc6BIW-y, PRRT_kwDORs-xfc6BIW_R);
post-resolve verification: {total:3, resolved:3, unresolved:0}. Copilot re-requested
~2026-05-11T15:23:30Z. Round 1 convergence: 3→0 (one round, all valid, all fixed).

A Copilot reply for comment 3220034266 was initially posted with a missing `jr issue move`
token (shell expanded backticks inside `-f body="..."` before gh saw the argument).
Corrected via PATCH using `printf '%s' '...' | jq -Rs '{body: .}' | gh api --input -`.
Lesson codified in lessons.md.

| Step | Agent | Output |
|------|-------|--------|
| PR #351 merged by GitHub | GitHub | develop @ 3216ec2; closes #339+#344 |
| Worktree + branch cleanup | orchestrator | `.worktrees/test-hygiene` removed; `chore/test-hygiene-round2-rebase` branch deleted |
| PR #352 Round 1 triage | orchestrator + direct edits | 3 findings triaged; all valid; validation strategy: local file verification |
| PR #352 Round 1 fixes | direct edits | CLAUDE.md L211 + src/cli/mod.rs L401 + tests/issue_bulk_pr2.rs L554 |
| PR #352 Round 1 push | orchestrator | f42bfa5; 8/8 CI green |
| Review threads resolved | orchestrator (GraphQL) | 3/3 resolved; {total:3, resolved:3, unresolved:0} |
| Copilot reply corrected | orchestrator | Comment 3220057819 PATCH'd via `jq -Rs + --input -` |
| Re-request Copilot review for Round 2 | orchestrator | Re-requested ~2026-05-11T15:23:30Z on head f42bfa5 |
| Wait for Round 2 review | orchestrator | Polled for review id > 3220034401 |
| Verify Round 2 review body | orchestrator | review id 4265005419 (2026-05-11T15:25:48Z): "Copilot reviewed 3 out of 3 changed files in this pull request and generated no new comments." |
| Verify 0 inline comments in Round 2 | orchestrator | `gh api .../pulls/352/comments --jq '.[] | select(.user.login == "Copilot" and .id > 3220034401)'` returned empty |
| Confirm Phase 8 stop condition met | orchestrator | Overview comment only (no file-level findings) — stop condition explicitly satisfied |

**Outcome:** PR #352 CONVERGED. Final trajectory: 3→0 (R1: 3 valid local-consistency fixes; R2: clean). OPEN/MERGEABLE/CLEAN; awaiting human merge. Closes #337+#341+#347 on merge.

---

## Burst: PR #352 Round 2 Convergence (2026-05-11)

**Date:** 2026-05-11T15:23:30Z–15:25:48Z
**Agents:** orchestrator (no sub-agent dispatch)
**Input files touched:** none (read-only verification)
**Output commits:** none on develop; factory-artifacts state update only
**factory-artifacts commit:** this commit

### Summary

Round 2 Copilot review on PR #352 (chore/docs-cleanup-337-341-347) returned 0 new
inline comments. Review id 4265005419 submitted at 2026-05-11T15:25:48Z with body:
"Copilot reviewed 3 out of 3 changed files in this pull request and generated no new
comments." Verified via `gh api` that no Copilot inline comments exist with id >
3220034401 (the last Round 1 comment id).

Phase 8 stop condition confirmed: overview comment alone (no file-level findings) is
not a reason to continue. PR #352 is CONVERGED at 3→0 over 2 rounds.

| Step | Agent | Output |
|------|-------|--------|
| Await Round 2 review | orchestrator | review id 4265005419 received 2026-05-11T15:25:48Z |
| Confirm 0 inline findings | orchestrator | Empty result from inline comment filter — no new R2 comments |
| Confirm OPEN/MERGEABLE/CLEAN | orchestrator | PR state verified; 8/8 CI green unchanged since f42bfa5 |
| Factory state update | state-manager | STATE.md + burst-log.md + pr-352-docs-cleanup convergence record |
| Copilot re-requested | orchestrator | ~2026-05-11T15:23:30Z; awaiting Round 2 |

---

## Burst: PR #352 Merged (2026-05-11)

**Date:** 2026-05-11T15:36:10Z
**Agents:** orchestrator (human merge)
**Input files touched:** none (human action)
**Output commits:** develop @ 57cc0ae (squash merge of chore/docs-cleanup-337-341-347)
**factory-artifacts commit:** included in PR #353 open burst below

### Summary

PR #352 (chore/docs-cleanup-337-341-347 @ f42bfa5) was squash-merged to develop at
57cc0ae by human. Closes GitHub issues #337, #341, and #347. Develop fast-forwarded
3216ec2→57cc0ae. This completes the docs-cleanup audit theme.

| Step | Agent | Output |
|------|-------|--------|
| Human merges PR #352 | GitHub (human) | develop @ 57cc0ae; closes #337+#341+#347 |

**Outcome:** PR #352 MERGED. Develop at 57cc0ae. 12 audit-followups remain after #338 closes.

---

## Burst: PR #353 (#338 consolidate BULK_MAX_KEYS) Open + Implementation (2026-05-11)

**Date:** 2026-05-11
**Agents:** orchestrator + state-manager
**Branch:** refactor/bulk-max-keys-338
**Head commit:** 3b98a3d
**Input files touched (read):** src/cli/issue/create.rs, src/cli/issue/workflow.rs (verify premise)
**Output files changed:** src/api/jira/bulk.rs (+9), src/cli/issue/create.rs (-3 net), src/cli/issue/workflow.rs (-2 net)
**factory-artifacts commit:** this commit

### Summary

Premise verified via `grep -rE "BULK_(MOVE_)?MAX_KEYS"`: two duplicate `usize = 1000`
constants existed — `BULK_MAX_KEYS` in src/cli/issue/create.rs and `BULK_MOVE_MAX_KEYS`
in src/cli/issue/workflow.rs — both representing the same Atlassian per-call cap.

Trivial-changes path selected per validated-feature-lifecycle skill: no design decisions,
no external API claims, no new user-visible behavior. Skipped brainstorm/spec/plan phases;
kept implementation + review + PR + Copilot validation.

Worktree created off develop @ 57cc0ae (post-#352 merge tip). Canonical constant
`pub const BULK_MAX_KEYS: usize = 1000` added to src/api/jira/bulk.rs. Both CLI handlers
updated to remove local constant definitions and import the canonical one. Net change:
+14/-9 lines across 3 files. No behavioral change — same numeric limit at same call sites.

Local CI-equivalent passed: cargo fmt --check, cargo clippy --all-targets -- -D warnings,
cargo test (613 unit + 38 bulk integration + all other suites). Commit 3b98a3d pushed.
PR #353 created. Copilot review requested.

| Step | Agent | Output |
|------|-------|--------|
| Read existing constants (verify premise) | orchestrator | `grep -rE "BULK_(MOVE_)?MAX_KEYS"` confirmed 2 duplicate usize=1000 constants |
| Create worktree off develop @ 57cc0ae | orchestrator | `.worktrees/issue-338-consolidate-bulk-max` |
| Add pub const BULK_MAX_KEYS to src/api/jira/bulk.rs | orchestrator | +9 lines |
| Remove local const + add import in create.rs | orchestrator | -3 lines net |
| Remove local const + rename refs + add import in workflow.rs | orchestrator | -2 lines net |
| Local cargo fmt + clippy + test | orchestrator | All green; 613 unit + 38 bulk integration |
| Commit 3b98a3d + push refactor/bulk-max-keys-338 | orchestrator | 3b98a3d on refactor/bulk-max-keys-338 |
| Create PR #353 (closes #338) | orchestrator | https://github.com/Zious11/jira-cli/pull/353 |
| Request Copilot review | orchestrator | Review requested on 3b98a3d |
| Factory state update | state-manager | STATE.md session checkpoint + phase progress + convergence tracker; burst-log.md new entries |

**Outcome:** PR #353 OPEN. Awaiting CI green + Copilot Round 1. Trivial-changes path — no adversarial review needed.

---

## Burst N+1 (2026-05-11) — PR #353 Round 1 Convergence + Post-hoc Perplexity Validation

**Agents dispatched:** orchestrator, state-manager
**Files touched:** .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md, .factory/cycles/cycle-001/lessons.md, .factory/cycles/cycle-001/adversarial-reviews/pr-353-bulk-max-keys/pr-353-copilot-convergence.md
**Versions bumped:** (none)

### Summary

CI on 3b98a3d settled 8/8 green (2026-05-11T15:43:21Z). Copilot Round 1 submitted
2026-05-11T15:43:07Z (review id 4265141297, state COMMENTED) with 0 inline comments —
only an overview praising the consolidation. Phase 8 stop condition met immediately:
overview alone with no file-level findings. No Round 2 needed.

User raised post-hoc question: "did we validate with perplexity?" The trivial-changes
path explicitly lists Perplexity in the skip column for refactors with no design
decisions. However, the distinct constant names (`BULK_MAX_KEYS` vs `BULK_MOVE_MAX_KEYS`)
represented an implicit external-knowledge claim: that the two Atlassian endpoints share
the same per-call cap. Perplexity query run to validate.

Perplexity result CONFIRMED: both POST /rest/api/3/bulk/issues/fields (bulk edit) and
POST /rest/api/3/bulk/issues/transition (bulk transition) share a 1000-issue per-call
cap. Citations: developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/
and bulk-operation-additional-examples-and-faqs/. Consolidation is correct; no regression.

A process-gap lesson was codified in lessons.md: when two same-typed constants exist with
distinct names suggesting they might differ, Perplexity should be run to confirm the
underlying constraint is actually shared — even on the trivial-changes path.

| Step | Agent | Output |
|------|-------|--------|
| Await CI on 3b98a3d | orchestrator | 8/8 SUCCESS (settled 2026-05-11T15:43:21Z) |
| Await Copilot Round 1 | orchestrator | Review id 4265141297 — 0 inline comments; overview only |
| Evaluate Phase 8 stop condition | orchestrator | Met — 0 inline findings; no Round 2 needed |
| User prompt: "did we validate with perplexity?" | orchestrator | Ran post-hoc Perplexity validation |
| Perplexity query — Atlassian bulk cap-equivalence | orchestrator | CONFIRMED: both endpoints cap at 1000 (2 citations) |
| Create pr-353-copilot-convergence.md | state-manager | cycles/cycle-001/adversarial-reviews/pr-353-bulk-max-keys/pr-353-copilot-convergence.md |
| Append process-gap lesson to lessons.md | state-manager | cycles/cycle-001/lessons.md — lesson [candidate] added |
| Update STATE.md | state-manager | Phase progress CONVERGED; session checkpoint replaced; convergence tracker updated |

---

## Burst N+2 (2026-05-11) — PR #353 Merged + Post-merge Cleanup

**Agents dispatched:** pr-manager, state-manager
**Files touched:** .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

PR #353 (refactor/bulk-max-keys-338; closes #338) was merged by the human at
2026-05-11T15:50:22Z (merge commit 7fbf14d7d748c37d6948104da4109591fbe5ac0c).
GitHub auto-deleted the remote branch refactor/bulk-max-keys-338 on merge.
Issue #338 was automatically closed at 2026-05-11T15:50:23Z.

Post-merge cleanup performed: worktree `.worktrees/issue-338-consolidate-bulk-max`
removed, local branch `refactor/bulk-max-keys-338` deleted, develop locally
fast-forwarded from 57cc0ae to 7fbf14d.

| Step | Agent | Output |
|------|-------|--------|
| Human merges PR #353 | human | Merge commit 7fbf14d; issue #338 auto-closed |
| Remove worktree `.worktrees/issue-338-consolidate-bulk-max` | pr-manager | Removed |
| Delete local branch refactor/bulk-max-keys-338 | pr-manager | Branch deleted (remote already gone) |
| Fast-forward develop to 7fbf14d | pr-manager | develop: 57cc0ae..7fbf14d |
| Update STATE.md | state-manager | Phase progress row MERGED; convergence tracker + session checkpoint updated |

**Outcome:** #338 CLOSED. Develop at 7fbf14d (post-#353 tip). 11 audit-followups remain.

---

## Burst N+3 (2026-05-11) — PR #354 (#342 plannedChanges.labels shape doc) Open + Implementation

**Agents dispatched:** orchestrator, state-manager
**Files touched:** src/cli/issue/create.rs (+24 lines), .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Implemented issue #342: document the plannedChanges.labels shape divergence between
dry-run preview JSON and the live POST body. Documentation-only change (+24 lines) at
two sites in src/cli/issue/create.rs:

1. Dry-run JSON builder (~line 485): cross-referenced comment explaining why
   `plannedChanges.labels` uses simplified `[{action, name}]` shape instead of the
   nested Atlassian shape, with forward reference to #331 + #345 for eventual convergence.
2. `handle_edit_bulk_labels` docstring: note that the dry-run preview path uses a
   different shape, documenting the divergence:
   - Dry-run: `"labels": [{"action": "ADD", "name": "foo"}]`
   - POST body: `"labels": {"labelsAction": "ADD", "labels": [{"name": "foo"}]}`

Rationale for documenting vs normalizing: the POST shape is itself a best-guess pending
#331 empirical sandbox verification. Locking dry-run consumers to that shape now would
force a second breaking change once #331 confirms the canonical shape. Documented in
PR description. Once #331 + #345 land, the two paths can converge to byte-identical JSON.

Validation path: trivial-changes / docs-only per validated-feature-lifecycle skill.
Divergence claim empirically verifiable by reading both builders — local verification
authoritative. No adversarial review needed.

Local CI-equivalent: cargo fmt --check green, cargo clippy --all-targets -- -D warnings
green, cargo test green (613 unit + 38 bulk integration + all other suites).
Commit 0eb77f3 pushed to docs/labels-shape-divergence-342 (base develop @ 7fbf14d).
PR #354 created: https://github.com/Zious11/jira-cli/pull/354
Copilot review requested (poller bb3qub9yc). Remote CI in-flight (poller beij5gw3i).

| Step | Agent | Output |
|------|-------|--------|
| Create worktree off develop @ 7fbf14d | orchestrator | `.worktrees/issue-342-labels-doc` |
| Read both builders in src/cli/issue/create.rs | orchestrator | Confirmed 2 divergence sites |
| Add doc comment at dry-run JSON builder (~line 485) | orchestrator | +12 lines explaining simplified shape + forward refs |
| Add docstring to handle_edit_bulk_labels | orchestrator | +12 lines noting dry-run vs POST shape divergence |
| Local cargo fmt + clippy + test | orchestrator | All green; 613 unit + 38 bulk integration |
| Commit 0eb77f3 + push docs/labels-shape-divergence-342 | orchestrator | 0eb77f3 on docs/labels-shape-divergence-342 |
| Create PR #354 (closes #342) | orchestrator | https://github.com/Zious11/jira-cli/pull/354 |
| Request Copilot review | orchestrator | Review requested; poller bb3qub9yc watching |
| Factory state update | state-manager | STATE.md phase progress + convergence tracker + session checkpoint; burst-log.md two new entries |

**Outcome:** PR #354 OPEN. Awaiting CI green + Copilot Round 1. Docs-only — no adversarial review needed.

**Outcome:** PR #353 CONVERGED Round 1 (0 inline comments). Perplexity-validated. Awaiting human merge (closes #338).

---

## Burst N+4 (2026-05-11) — PR #354 Copilot R1+R2+R3 Convergence

**Agents dispatched:** orchestrator (Copilot rounds), state-manager
**Files touched:** src/cli/issue/create.rs (b835438: +reword; 0644b1d: +30/-17), .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md, .factory/cycles/cycle-001/adversarial-reviews/pr-354-labels-shape-doc/pr-354-copilot-convergence.md (new), .factory/cycles/cycle-001/lessons.md
**Versions bumped:** (none)

### Summary

PR #354 converged through 3 Copilot review rounds (trajectory 1→1→0). The change documents
the dry-run vs POST shape divergence for `labels`, `priority`, and `issueType` in
src/cli/issue/create.rs.

**Round 1** (review id 4265225515, ~15:58:50Z): 1 inline comment. Finding: the docstring
at `handle_edit_bulk_labels` used "canonical" in the NOTE heading while admitting the shape
is "still-unverified, pending #331" — a self-contradiction. Fix (b835438): reworded so
"canonical" appears only in future-state phrasing ("Once #331 confirms the canonical wire
shape…"). Thread resolved.

**Round 2** (review id 4265308298, 16:05:45Z): 1 inline comment. Finding: the R1 NOTE
covered only the `labels` divergence, but the same dry-run-vs-POST pattern applies to
`priority` (bare string vs `{"name": ...}`) and `issueType` (bare string vs
`{"issuetype": {"name": ...}}`). Documenting only labels implies false completeness.
Validation: local file verification confirmed all three fields; SCHEMA NOTES in
`bulk.rs::BulkEditRequest` confirms priority and issueType are also best-guesses pending
#331. Triage: Fix now (doc accuracy in changed code). Fix (0644b1d): expanded NOTE to
cover all three fields uniformly; added parallel cross-reference on `handle_edit_bulk_fields`.
+30 -17 lines. Thread resolved. Copilot value-add — genuine scope gap caught.

**Round 3** (review id 4265361087, 16:12:31Z): 0 inline comments. Phase 8 stop condition
met. Convergence declared.

CI on 0644b1d: 8/8 green (settled 16:10:18Z). All 2 threads resolved (2/2).

| Step | Agent | Output |
|------|-------|--------|
| Copilot Round 1 (review 4265225515) — 1 inline finding | Copilot | R1 self-contradiction: "canonical" vs "unverified" |
| Local file verification of R1 finding | orchestrator | Confirmed — fix warranted |
| Fix b835438: reword docstring, remove contradictory "canonical" | orchestrator | b835438 on docs/labels-shape-divergence-342 |
| Resolve R1 thread via GraphQL | orchestrator | R1 thread resolved |
| Request Copilot Round 2 | orchestrator | Round 2 dispatched |
| Copilot Round 2 (review 4265308298) — 1 inline finding | Copilot | R2 scope-narrowness: labels-only NOTE implies false completeness |
| Local file verification of R2 finding (all 3 fields) | orchestrator | Confirmed: priority + issueType have same divergence pattern |
| Fix 0644b1d: expand NOTE to cover labels + priority + issueType; add parallel note on handle_edit_bulk_fields | orchestrator | 0644b1d (+30/-17) on docs/labels-shape-divergence-342 |
| Resolve R2 thread via GraphQL | orchestrator | R2 thread resolved |
| Request Copilot Round 3 | orchestrator | Round 3 dispatched |
| Copilot Round 3 (review 4265361087) — 0 inline findings | Copilot | Phase 8 stop condition met — CONVERGED |
| Factory state update | state-manager | STATE.md + burst-log + convergence record + lessons |

**Outcome:** PR #354 CONVERGED Round 3 (1→1→0). CI 8/8 green (0644b1d). 2/2 threads resolved. Awaiting human merge (closes #342). 11 audit-followups remain after #342 merges: #331, #332, #333, #334, #335, #336, #340, #343, #345, #346, #350.

---

## Burst N+1 (2026-05-11) — PR #354 Merged + Cleanup

**Agents dispatched:** orchestrator, state-manager
**Files touched:** .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

PR #354 merged by human at 2026-05-11T16:13:59Z (merge commit 4e148490e98b0f516258908f75de4ec8d0367ea4). Issue #342 automatically closed at 2026-05-11T16:14:00Z (verified). Post-merge cleanup completed: develop fast-forwarded locally from 7fbf14d to 4e14849, worktree `.worktrees/issue-342-labels-doc` removed, local branch `docs/labels-shape-divergence-342` deleted. Final convergence trajectory for PR #354: 1→1→0 over 3 Copilot rounds.

| Step | Agent | Output |
|------|-------|--------|
| Human merges PR #354 @ 4e14849 | human | Merge commit 4e148490e98b0f516258908f75de4ec8d0367ea4; #342 closed |
| Verify #342 closed | orchestrator | #342 CLOSED at 2026-05-11T16:14:00Z — confirmed |
| Develop fast-forward 7fbf14d..4e14849 | orchestrator | Local develop HEAD at 4e14849 |
| Remove worktree `.worktrees/issue-342-labels-doc` | orchestrator | Worktree removed |
| Delete local branch `docs/labels-shape-divergence-342` | orchestrator | Branch deleted |
| Factory state update | state-manager | STATE.md + burst-log updated |

**Outcome:** PR #354 MERGED (closes #342). Develop at 4e14849. Cleanup complete. 11 audit-followups now active: #331, #332, #333, #334, #335, #336, #340, #343, #345, #346, #350.

---

## Burst N+2 (2026-05-11) — PR #355 (#332 task_id validation) Open + Implementation

**Agents dispatched:** orchestrator (implementer), research-agent (Perplexity), state-manager
**Files touched:** src/api/jira/bulk.rs (+168 lines), .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md
**Versions bumped:** (none)

### Summary

Implemented defense-in-depth security validation for `BulkSubmitResponse.task_id` (issue #332). The `task_id` field is subsequently used in URL paths and terminal output; CWE-117-adjacent CR/LF injection from a hostile/spoofed Atlassian response (or `JR_BASE_URL`-controlled MitM proxy) is the primary threat vector.

**Perplexity pre-design validation (per DEC-018):** Two queries run before designing the allowlist:

- Query 1 ("Atlassian Jira Cloud REST API v3 bulk operations taskId format"): Perplexity returned no specific docs on taskId format; recommended consulting official Atlassian docs and empirical testing. Inconclusive.
- Query 2 ("OpenAPI specification BulkOperationProgress schema property taskId definition"): No OpenAPI schema pinned for taskId in v3 bulk-operations group. Inferred pattern from Atlassian cloud-identifier conventions: `{numericPrefix}:{uuid}` (e.g., `"123456:4ac97bc8-ab12-ab12-8d38-eda562abc123"`), ~40-50 chars typical. Citations: community.atlassian.com/forums/Confluence-questions/API-accountId-..., jira.atlassian.com/browse/JIRAALIGN-7538. Inconclusive but constrains design.

**Allowlist design** (conservative, given format uncertainty):
- Charset: `[A-Za-z0-9._:-]+` (covers UUIDs, `domainId:uuid` pattern, numeric tokens, opaque ASCII)
- Length: 1..=256 bytes (generous ceiling; observed pattern ~40-50 chars)
- Rejects: empty string, oversized (>256 bytes), `/`, `\`, NUL, CR, LF, space, non-ASCII, control bytes

**Implementation:** `validate_task_id` function + `MAX_TASK_ID_LEN: 256` constant added in `src/api/jira/bulk.rs`. Wired into 3 call sites: `bulk_edit_fields`, `bulk_transition`, `poll_bulk_task`. 15 new unit tests covering valid/invalid classes (valid UUIDs, domainId:uuid, alphanumeric tokens; invalid: empty, oversized, path-traversal chars `/`/`\`, CR/LF injection, NUL byte, non-ASCII, leading/trailing space, control bytes). Clippy octal-escape warning caught and fixed during local CI (`"task\0123"` → `"task\x00123"`).

**Local CI-equivalent results:**
- cargo fmt --check: PASS
- cargo clippy --all-targets -- -D warnings: PASS (after octal-escape fix)
- cargo test: PASS — 628 unit + 38 bulk integration + all other suites green

**PR and remote status:** Commit 64e9c97 pushed to `chore/task-id-validation-332`. PR #355 opened against develop @ 4e14849. Remote CI in-flight (poller bc312fqxe). Copilot review requested (poller becpc7kbf).

| Step | Agent | Output |
|------|-------|--------|
| Perplexity Query 1: Atlassian taskId format in bulk ops | research-agent | Inconclusive — no specific docs; recommended empirical testing |
| Perplexity Query 2: OpenAPI BulkOperationProgress taskId schema | research-agent | Inconclusive — inferred `{numericPrefix}:{uuid}` ~40-50 chars from community sources |
| Design allowlist: `[A-Za-z0-9._:-]+` 1..=256 | orchestrator | Conservative charset + generous ceiling; rejects CR/LF/NUL/non-ASCII |
| Implement `validate_task_id` + `MAX_TASK_ID_LEN: 256` in src/api/jira/bulk.rs | orchestrator | +168 lines |
| Wire into 3 call sites (bulk_edit_fields, bulk_transition, poll_bulk_task) | orchestrator | 3 call sites updated |
| Write 15 unit tests (valid + invalid classes) | orchestrator | 15 tests covering threat-model classes |
| Fix clippy octal-escape warning (`"task\0123"` → `"task\x00123"`) | orchestrator | Clippy clean |
| cargo fmt + clippy + test (local CI-equivalent) | orchestrator | All green — 628 unit + 38 bulk integration |
| Commit 64e9c97, push chore/task-id-validation-332 | orchestrator | 64e9c97 on remote |
| Open PR #355 (closes #332) | orchestrator | https://github.com/Zious11/jira-cli/pull/355 |
| Request Copilot review | orchestrator | Poller becpc7kbf in-flight |
| Factory state update | state-manager | STATE.md + burst-log updated |

**Outcome:** PR #355 OPEN (chore/task-id-validation-332 @ 64e9c97; closes #332). Defense-in-depth task_id validation shipped. Behavioral change: none for well-formed Atlassian responses; rejects malformed/hostile input. Remote CI in-flight; awaiting Copilot Round 1. 10 audit-followups remain after #332 closes: #331, #333, #334, #335, #336, #340, #343, #345, #346, #350.

---

## Burst N+3 (2026-05-11) — PR #355 R1+R2+R3 Convergence

**Agents dispatched:** orchestrator (fixer), research-agent (Perplexity), state-manager
**Files touched:** src/api/jira/bulk.rs (b120032: +64 -17; 62766f4: +10), .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md, .factory/cycles/cycle-001/lessons.md, .factory/cycles/cycle-001/adversarial-reviews/pr-355-task-id-validation/pr-355-copilot-convergence.md
**Versions bumped:** (none)

### Summary

PR #355 (chore/task-id-validation-332) converged at Round 3 (3→1→0). Two real security findings
were caught and fixed across Rounds 1 and 2. Round 3 returned 0 inline comments — Phase 8 stop
condition met. A notable Perplexity calibration event occurred in R2.

### Round 1 Steps

| Step | Agent | Output |
|------|-------|--------|
| Receive Copilot R1 (review id 4265474208) — 3 inline comments | orchestrator | 3 findings: dot-segment path-confusion, UX actionability, test comment accuracy |
| Perplexity query: RFC 3986 §5.2.4 dot-segment removal in reqwest/curl/hyper | research-agent | **CONFIRMED** — reqwest/hyper/curl apply §5.2.4 before send; urlencoding does NOT escape `.`; path-confusion confirmed |
| Fix: add `if task_id == "." || task_id == ".."` rejection before length/charset checks | orchestrator | RFC 3986 §5.2.4 rejection added with dedicated error message |
| Fix: add 2 dot-segment tests + 1 accepts-dot-within-longer-token boundary test | orchestrator | 3 new tests |
| Fix: reword oversized-taskId and empty-taskId error messages to actionable pattern | orchestrator | Error messages now match "re-run the bulk command" convention |
| Fix: correct misleading test comment about `..` + urlencoding::encode | orchestrator | Test comment corrected |
| Commit b120032 (+64 -17), push chore/task-id-validation-332 | orchestrator | b120032 on remote; 3 R1 threads resolved |
| Request Copilot R2 | orchestrator | R2 dispatched |

### Round 2 Steps

| Step | Agent | Output |
|------|-------|--------|
| Receive Copilot R2 (review id 4265541072) — 1 inline comment | orchestrator | CWE-117: `await_bulk_task` interpolates unvalidated task_id before poll_bulk_task (timeout=0 path) |
| Perplexity query: does Rust `{:?}` Debug formatter for `&str` escape CR/LF/NUL/ANSI? | research-agent | **INCORRECT** — Perplexity claimed `{:?}` does NOT escape control chars; hallucination detected |
| Local empirical verification: 5-line Rust program + cat -v | orchestrator | **CONTRADICTS Perplexity** — `{:?}` DOES escape \r/\n/\0/\t/\x1b via str::escape_debug |
| Fix decision: add `validate_task_id(task_id)?` at very start of `await_bulk_task` | orchestrator | Entry-validation at function boundary; formatter semantics moot |
| Update docstring: credit CWE-117 defense-in-depth rationale | orchestrator | Docstring updated |
| Commit 62766f4 (+10 lines), push chore/task-id-validation-332 | orchestrator | 62766f4 on remote; R2 thread resolved |
| Request Copilot R3 | orchestrator | R3 dispatched |

### Round 3 Steps

| Step | Agent | Output |
|------|-------|--------|
| Receive Copilot R3 (review id 4265717871) — 0 inline comments | orchestrator | "generated no new comments" |
| Phase 8 stop condition met | orchestrator | Convergence declared — no R4 dispatched |

### Perplexity Calibration Note

R2 produced the third documented instance of Perplexity hallucinating about observable Rust
stdlib behavior while citing correct documentation URLs. The tiered-validation backstop (local
empirical verification for Rust behavior) caught the hallucination before the wrong diagnosis
was acted on. DEC-018 standing rule unchanged; tiered-validation rule reinforced. Codified in
lessons.md.

### Final State

**PR #355:** OPEN, MERGEABLE, mergeStateStatus CLEAN, CI 8/8 green on 62766f4, 4/4 threads resolved.
Convergence trajectory: 3→1→0 (3 rounds). Awaiting human merge. Closes #332 on merge.
10 audit-followups remain after merge: #331, #333, #334, #335, #336, #340, #343, #345, #346, #350.

---

## Burst N+1 (2026-05-11): PR #355 Merged + Cleanup

**Agents dispatched:** pr-manager, devops-engineer
**Files touched:** (source repo) develop branch fast-forwarded to 448c568; worktree + branch deleted
**Versions bumped:** (none)

### Summary

PR #355 (chore/task-id-validation-332) was merged by the human at 2026-05-11T17:32:05Z via merge commit 448c568. GitHub automatically closed issue #332 at 2026-05-11T17:32:06Z. Develop was fast-forwarded from 4e14849 to 448c568 (4 new commits since PR #354). Post-merge cleanup: worktree `.worktrees/issue-332-task-id-validation` removed, local branch `chore/task-id-validation-332` deleted. Final convergence trajectory for PR #355 was 3→1→0 over 3 Copilot rounds.

### Details

| Agent | Task | Output |
|-------|------|--------|
| pr-manager | Observe PR #355 merge | Merge commit 448c568; issue #332 closed |
| devops-engineer | Post-merge cleanup | Worktree `.worktrees/issue-332-task-id-validation` removed; branch `chore/task-id-validation-332` deleted (local + remote); develop fast-forwarded 4e14849→448c568 |
| state-manager | STATE.md update | Phase Progress row updated to MERGED; Current Phase Steps updated; Session Resume Checkpoint replaced |

---

## Burst N+2 (2026-05-11): PR #356 Opened — #334 Sanitize errorMessages (CWE-117)

**Agents dispatched:** orchestrator, implementer, pr-manager
**Files touched:** src/api/client.rs (+139 lines: sanitize_for_stderr fn + extract_error_message_raw refactor + 11 unit tests), tests/api_client.rs (+43 lines: 4 new integration tests)
**Versions bumped:** (none)

### Summary

PR #356 opened implementing issue #334: CWE-117 defense at the `extract_error_message` public boundary in `src/api/client.rs`. The fix adds `sanitize_for_stderr(s: &str) -> String` which strips ASCII control characters (bytes 0x00–0x1F, 0x7F) from Atlassian error message strings before they are emitted to stderr, preventing terminal injection (log forging, ANSI escape injection) via hostile or proxy-injected error payloads.

**Design decision — custom sanitizer over `str::escape_debug`:** The Rust standard library's `escape_debug` would escape all control characters correctly, but it also escapes non-ASCII bytes to `\u{XXXX}` sequences. This would garble localized error messages from non-English Jira tenants (e.g., Japanese, Arabic, Chinese). The custom sanitizer replaces only control characters with U+FFFD (replacement character) while passing through all non-ASCII Unicode unchanged.

**Test fixture quirk encountered and fixed:** Initial test fixtures used Rust raw byte strings with embedded NUL (`\x00`) and ESC (`\x1b`) bytes directly. The `serde_json` parser failed to parse these because the JSON spec requires control characters to be escaped as `\uXXXX` in string values; raw control bytes are invalid JSON. Fixed by writing fixture strings with literal ` ` and `` JSON Unicode escapes (e.g., `"some error injected[31mRED"`), which parse correctly and deliver the real control bytes to the sanitizer.

**Test coverage:** 11 unit tests in `src/api/client.rs` (NUL, CR, LF, ESC ANSI, tab-preserved, non-ASCII Unicode preserved, all-clean passthrough, multi-source error concatenation, empty input, all-control stripped, boundary bytes). 4 integration tests in `tests/api_client.rs` covering the public `extract_error_message` API across the 4 precedence paths (x-reason header, statusCode body field, errorMessages array, empty body fallback).

**Validation strategy:** No Perplexity research needed. CWE-117 pattern and Rust `escape_debug` behavior were already empirically established during PR #355 Round 2 analysis. The design choice (custom vs escape_debug) was made based on that prior empirical work plus the non-ASCII preservation requirement.

**Local CI state at commit d1b9fe7:** cargo fmt clean, cargo clippy --all-targets -- -D warnings clean, cargo test passing (641 unit + 26 api_client integration + all other suites). Remote CI in-flight. Copilot review requested.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Scope and design CWE-117 fix | sanitize_for_stderr design; custom sanitizer rationale documented |
| implementer | Implement sanitize_for_stderr + refactor extract_error_message_raw | src/api/client.rs +139 lines; 11 unit tests |
| implementer | Integration tests for public extract_error_message API | tests/api_client.rs +43 lines; 4 integration tests; fixture JSON escape quirk identified and fixed |
| orchestrator | Commit d1b9fe7, push chore/sanitize-errors-334, open PR #356 | PR #356 at https://github.com/Zious11/jira-cli/pull/356; base develop @ 448c568 |
| pr-manager | Request Copilot review | Copilot R1 poller b9vv6n65e; CI poller bkulwe03a |

---

## Burst N+3 (2026-05-11): PR #356 Copilot Round 1 — 4 findings, fix commit 51e2807

**Agents dispatched:** orchestrator, implementer
**Files touched:** src/api/client.rs (sanitize_for_stderr rewrite: std::fmt::Write::write!, fast-path signature change, MAX_ERROR_ENTRY_LEN=1024, cap_entry helper, 5 new tests)
**Versions bumped:** (none)

### Summary

PR #356 Copilot Round 1 (2026-05-11T17:49:49Z) returned 4 inline findings. All 4 were valid.
Perplexity-validation was run for R1 per DEC-018 — confirmed CWE-117 + OWASP length-capping
guidance (https://cwe.mitre.org/data/definitions/117.html). Finding 4 was a requirements gap:
issue #334 explicitly required a per-entry length cap (1 KiB), which was absent from the
initial implementation.

**Findings:**
1. Doc comment "single allocation" claim mismatched `format!()` per escaped char implementation.
2. `format!()` inside the escape loop allocated per char — replaced with `std::fmt::Write::write!`.
3. Clean-input fast path unnecessarily allocated a new String — changed signature to `fn(String) -> String` with zero-copy passthrough (pointer-equality test added).
4. **REQUIREMENTS GAP:** Missing per-entry length cap (issue #334 explicitly requires 1 KiB truncation).

**Fix:** Added `MAX_ERROR_ENTRY_LEN = 1024`, `cap_entry` helper, `std::fmt::Write::write!` rewrite,
5 new tests including pointer-equality fast-path assertion. All 4 threads resolved.

**Perplexity validation:** R1 — CONFIRMED CWE-117 + OWASP length-cap as defense-in-depth.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 4 Copilot R1 findings; run Perplexity DEC-018 | All 4 confirmed valid; OWASP length-cap confirmed |
| implementer | Fix doc accuracy + loop allocation + fast-path + cap requirement | src/api/client.rs rewrite; 5 new tests |
| orchestrator | Commit 51e2807; push; request R2 | 4/4 threads resolved; R2 requested |

---

## Burst N+4 (2026-05-11): PR #356 Copilot Round 2 — 1 finding, fix commit d061b14

**Agents dispatched:** orchestrator, implementer
**Files touched:** src/api/client.rs (cap_entry marker budget reservation; test_cap_entry_size_invariant_at_boundary_oversize added)
**Versions bumped:** (none)

### Summary

PR #356 Copilot Round 2 (2026-05-11T18:10:07Z) returned 1 inline finding. Valid — invariant
violation in cap_entry for slightly-oversized inputs (1025-byte input → 1054-byte output via
1024-byte prefix + ~30-byte marker, defeating the flood-prevention cap).

**Perplexity-validation: SKIPPED [process-gap]** — the claim was judged "empirically verifiable
from arithmetic" and DEC-018 was not applied. This is the failure mode DEC-018 was designed to
prevent. (Codified as Lesson: see lessons.md — "Inconsistent Perplexity-validation undermines DEC-018".)

**Fix:** Reserve marker budget upfront: compute marker length first, set
`target_prefix_len = MAX_ERROR_ENTRY_LEN - marker.len()`. Added defensive branch for oversized
markers. Added `test_cap_entry_size_invariant_at_boundary_oversize` iterating [MAX+1..MAX+10000]
asserting output_len <= MAX_ERROR_ENTRY_LEN. 5/5 threads resolved (cumulative).

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage Copilot R2 finding; Perplexity SKIPPED [process-gap] | Finding confirmed valid via code analysis only |
| implementer | Fix cap_entry marker budget; add boundary invariant test | src/api/client.rs; test_cap_entry_size_invariant_at_boundary_oversize |
| orchestrator | Commit d061b14; push; request R3 | 5/5 threads resolved; R3 requested |

---

## Burst N+5 (2026-05-11): PR #356 Copilot Round 3 — 2 findings (1 critical), fix commit 274961c

**Agents dispatched:** orchestrator, implementer
**Files touched:** src/api/client.rs (MAX_SANITIZED_OUTPUT_LEN=4096, byte-budget-aware char loop, cap_entry marker fallback fix, 3 new tests)
**Versions bumped:** (none)

### Summary

PR #356 Copilot Round 3 (2026-05-11T18:18:03Z) returned 2 inline findings. Both valid.
Finding 1 was critical: the per-entry pre-sanitization cap allowed 4x byte expansion (1 control
char → 4-byte `\xNN` escape), making the 1024-byte pre-cap meaningless as an output bound.

**Perplexity-validation: SKIPPED [process-gap]** — again judged "verifiable from code analysis
(1→4 byte expansion is arithmetic)." Per DEC-018, should have validated. Second skipped round.
(Same codified lesson applies.)

**Fix:**
1. Added `MAX_SANITIZED_OUTPUT_LEN = 4096`. Restructured `sanitize_for_stderr` with a
   byte-budget-aware char loop that accounts for escape expansion. Output is guaranteed
   `<= MAX_SANITIZED_OUTPUT_LEN` regardless of input composition.
2. Fixed `cap_entry` marker fallback: defensive branch previously returned marker un-truncated,
   violating own size invariant. Now truncates marker at UTF-8 boundary.
3. Added 3 new tests: post-sanitization expansion, oversized clean input, under-cap no marker.
All 7/7 threads resolved (cumulative).

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 2 Copilot R3 findings; Perplexity SKIPPED [process-gap] | Both confirmed valid via code analysis only |
| implementer | Byte-budget-aware sanitize loop; cap_entry marker fallback fix; 3 tests | src/api/client.rs; MAX_SANITIZED_OUTPUT_LEN=4096 |
| orchestrator | Commit 274961c; push; request R4 | 7/7 threads resolved; R4 requested |

---

## Burst N+6 (2026-05-11): PR #356 Copilot Round 4 — 2 findings (efficiency), fix commit fe25e22

**Agents dispatched:** orchestrator, implementer
**Files touched:** src/api/client.rs (Cow<str> cap_entry, single-allocation errorMessages join, retroactive-trim sanitize restructure)
**Versions bumped:** (none)

### Summary

PR #356 Copilot Round 4 (2026-05-11T18:29:07Z) returned 2 inline findings. Both valid (efficiency).

**Perplexity-validation: CONFIRMED** — Validated `Cow<str>` idiomatic Rust pattern per Rust API
Guidelines C-COST. `Cow::Borrowed` is zero-cost (zero allocation for unchanged entries);
`Cow::Owned` matches a single String allocation for over-cap entries. Citation:
https://doc.rust-lang.org/std/borrow/enum.Cow.html

**Findings:**
1. Premature truncation: sanitize_for_stderr reserved 64-byte marker space unconditionally,
   truncating messages that fit cleanly within the full cap.
2. cap_entry allocated String per entry unconditionally — zero-alloc path missing for under-cap
   inputs (the common case).

**Fix:**
1. Restructured sanitize_for_stderr to allow full cap, then retroactively trim at UTF-8 boundary
   only when cap is breached. Marker appended only on actual truncation.
2. Changed cap_entry signature to `fn cap_entry(s: &str) -> Cow<'_, str>` — unchanged entries
   return `Cow::Borrowed` (zero alloc), over-cap entries return `Cow::Owned`.
3. Rewrote errorMessages join with single `String::with_capacity` allocation instead of N+1.
All 9/9 threads resolved (cumulative). CI in-flight on fe25e22 (poller b08xrozoq). R5 pending.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 2 Copilot R4 findings; Perplexity CONFIRMED Cow<str> C-COST pattern | Both confirmed valid; Cow<str> idiom validated |
| implementer | Cow<str> cap_entry; single-alloc join; retroactive-trim sanitize | src/api/client.rs fe25e22 |
| orchestrator | Commit fe25e22; push; request R5 | 9/9 threads resolved; CI in-flight; R5 requested |
| state-manager | [REMEDIATION] Backfill audit trail — R1-R4 burst entries, PR #356 progress file, lessons | burst-log.md, pr-356-copilot-progress.md, lessons.md, STATE.md all updated |

---

## Burst N+7 (2026-05-11): PR #356 Copilot Round 5 — Memory-Amplification Defense, fix commit c9be4de

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs (pre-cap body slice before from_utf8_lossy; streaming errorMessages join with running budget check); PR #356 description updated via gh pr edit --body-file
**Versions bumped:** (none)

### Summary

PR #356 Copilot Round 5 (2026-05-11T18:45:11Z, review id 4266436155) returned 3 inline findings. All 3 valid. 2 findings were memory-amplification security issues (OWASP A06/AP11), 1 was PR description drift.

**Perplexity-validation: CONFIRMED for R5 #1 and #2** (per codified Lesson 1 / DEC-018 standing rule). Both findings confirmed as OWASP A06:2021 Resource Exhaustion / AP11 Resource Exhaustion. Production codebases (kubernetes/client-go, docker/cli, tokio/hyper) all use `take(MAX_SIZE)` or pre-cap before parsing. `String::from_utf8_lossy` confirmed to allocate the FULL byte slice regardless of downstream truncation. R5 #3 (PR description drift) did not require Perplexity — purely doc-internal claim with no external library/API behavior.

**Process improvement:** This is the first state-manager dispatch made AFTER a Copilot round fix commit IN REAL TIME (not retroactively in batch). Per codified Lesson 2 ("Skipping state-manager between Copilot rounds creates audit-trail debt"), the audit trail is now being maintained continuously starting this round.

**Findings:**
1. Non-UTF8 fallback memory amplification: `String::from_utf8_lossy(body)` allocates owned String for ENTIRE byte slice before cap_entry truncation to 1 KiB. Hostile server returning 1 GB non-UTF8 body forces ~1 GB allocation. Fix: pre-cap byte slice to `MAX_ERROR_ENTRY_LEN * 4 = 4096 bytes` BEFORE `from_utf8_lossy`. 4x multiplier accommodates worst-case U+FFFD replacement expansion (3 bytes each). Total memory: O(MAX_ERROR_ENTRY_LEN) regardless of body size.
2. errorMessages join entry-count amplification: NUMBER of entries is server-controlled. Hostile response with 1M entries × 1024 bytes forces ~1 GB allocation in join before sanitize_for_stderr truncates. Fix: streaming build with running budget check — pre-sized to MAX_SANITIZED_OUTPUT_LEN (4 KiB), iterate lazily, check budget before each push, set truncated flag and break when exceeded, append " [...truncated]". Total memory: O(MAX_SANITIZED_OUTPUT_LEN) regardless of entry count.
3. PR description drift: PR body still described old `&str -> String` signature; implementation now takes `String` by value. Fix: updated PR description via `gh pr edit --body-file` to reflect final 5-round design.
All 3/3 threads resolved. Cumulative 12/12 resolved. Fix commit c9be4de (+48 -20 lines). CI in-flight on c9be4de. R6 pending.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 3 Copilot R5 findings; Perplexity CONFIRMED OWASP A06/AP11 for #1 + #2 | R5 #1 + #2: memory-amplification confirmed valid; R5 #3: PR description drift confirmed valid (no Perplexity needed) |
| implementer | Pre-cap body slice (4096 bytes) before from_utf8_lossy; streaming errorMessages join with budget; PR description sync | src/api/client.rs c9be4de; PR #356 description updated |
| orchestrator | Commit c9be4de; push; request R6 | 12/12 threads resolved; CI in-flight; R6 requested |
| state-manager | In-cycle state update (first real-time dispatch per codified Lesson 2) | STATE.md, burst-log.md, pr-356-copilot-progress.md updated |

---

## Burst N+8 (2026-05-11): PR #356 Copilot Round 6 — Marker Correctness, fix commit 59a0a12

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs (upfront JOIN_MARKER budget reservation in streaming join; truncation marker text excludes out.len() — references only original_len; 1 new regression test)
**Versions bumped:** (none)

### Summary

PR #356 Copilot Round 6 (2026-05-11T19:00:25Z, review id 4266560193) returned 2 inline findings. Both valid. Both are correctness/invariant issues in the streaming join marker and the sanitize truncation marker text.

**Perplexity-validation: CONFIRMED for both R6 findings** (per codified Lesson 1 / DEC-018 standing rule). Single Perplexity query covered both findings: upfront marker reservation is the standard pattern (cited Rust `std::fmt` buffer sizing + log-crate truncation conventions; retroactive trim "fails correctness"). Byte-count reporting must reflect FINAL emitted content length, not pre-trim value.

**Process note: SECOND consecutive in-cycle state-manager dispatch per codified Lesson 2.** Audit-trail discipline is now consistent. First dispatch was R5 (Burst N+7); this is R6 (Burst N+8).

**Findings:**

1. Streaming join marker overflow: `" [...truncated]"` (15 bytes) was appended unconditionally after the build loop broke. If `joined.len()` was close to `MAX_SANITIZED_OUTPUT_LEN` when break fired, final output exceeded the cap.
   - Fix: Reserve `JOIN_MARKER.len()` budget upfront. `content_budget_join = MAX_SANITIZED_OUTPUT_LEN - JOIN_MARKER.len()`. Budget check uses reduced budget; final output guaranteed `<= MAX_SANITIZED_OUTPUT_LEN`. Added `debug_assert!`. 15-byte reservation preserves R4 no-premature-truncation property.

2. Sanitize over-reporting retained byte count: Marker text `[...truncated at N sanitized bytes; original M bytes]` referenced `out.len()` BEFORE retroactive trim, over-reporting actual retained bytes.
   - Fix: Marker now references only `original_len` (immutable input byte count), NOT `out.len()`. New format: `[...truncated; original M bytes]`. Eliminates over-reporting; marker length is constant under retroactive trim (depends only on original_len digit count); R4 no-premature-truncation property preserved; operator still gets accurate "original M bytes" info.

**New regression test:** `test_sanitize_for_stderr_truncation_marker_excludes_out_len` — positive ("original N bytes" present), negative ("sanitized bytes" / "at N" absent), size invariant (output_len <= cap).

**Test results at 59a0a12:** 22 sanitize unit tests pass (1 new); 26 api_client integration tests pass; 60 test suites, 0 failures; cargo fmt --check + cargo clippy --all-targets -- -D warnings clean. CI in-flight on 59a0a12.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 2 Copilot R6 findings; Perplexity CONFIRMED upfront marker reservation + byte-count accuracy | Both confirmed valid via single Perplexity query covering both findings |
| implementer | Upfront JOIN_MARKER budget reservation; marker text references only original_len; debug_assert!; regression test | src/api/client.rs 59a0a12 |
| orchestrator | Commit 59a0a12; push; request R7 | 14/14 threads resolved; CI in-flight; R7 requested |
| state-manager | Second consecutive in-cycle dispatch (Lesson 2 compliance now consistent) | STATE.md, burst-log.md, pr-356-copilot-progress.md updated |

---

## Burst N+9 (2026-05-11): PR #356 Copilot Round 7 — Terminology + Annotation Cleanup, fix commit cdc4c64

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs (docstring terminology fix: "strip" → "escape"; 6 inline comment sites cleaned of stale round references; test annotations reworded to describe pinned behavior)
**Versions bumped:** (none)

### Summary

PR #356 Copilot Round 7 (2026-05-11T19:23:31Z, review id 4266726028) returned 3 inline findings. All valid. All are documentation/annotation quality issues — no behavior change. Fix commit cdc4c64 (+33 -31 lines).

**Perplexity-validation per Lesson 1 / DEC-018:**
- Finding 1 (terminology "strip" vs "escape"): CONFIRMED — OWASP/security-sanitization terminology distinguishes "strip" (irreversible deletion) from "escape" (reversible representation transformation). The code performs `\xNN` substitution, which is "escape" not "strip." Citations: https://blog.presidentbeef.com/blog/2020/01/14/injection-prevention-sanitizing-vs-escaping/ + https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html
- Findings 2 + 3 (stale round annotations): NO EXTERNAL CLAIM — purely project-internal annotation cleanup. Lesson 1 wording addresses "at least one external-claim aspect"; findings with no external claim do not require Perplexity. Skip is per-spec, not a rationalization.

**Process note: THIRD consecutive in-cycle state-manager dispatch per codified Lesson 2.** R5 → R6 → R7 all dispatched state-manager in real time. The discipline is now habit; future PRs in this cycle should retain this pattern.

**Findings:**

1. Terminology "strip" vs "escape": `extract_error_message` docstring said "strips ASCII control chars" but the implementation escapes them as visible `\xNN` literals (non-destructive, reversible). "Strip" implies deletion; "escape" is the correct term.
   - Perplexity CONFIRMED OWASP terminology distinction.
   - Fix: Reworded docstring to "escapes ASCII control chars from server-supplied content as visible `\xNN` literals before they reach stderr ... while keeping the byte information visible to the operator."

2. Stale round annotations in inline comments: Several comments referenced "PR #356 R6 fix", "(R6 fix)", or "R[N] finding on PR #356" — useful during iteration but stale post-merge.
   - No external claim; Perplexity skipped per Lesson 1 wording.
   - Fix: Cleaned 6 comment sites — replaced round-specific annotations with stable descriptions. Stable references retained: CWE-117, constant names, "issue #334."

3. Stale PR/round references in test annotations: Test comments like "Regression pin for the Copilot R2 finding on PR #356" don't decode for a future reader without cycle history.
   - No external claim; Perplexity skipped per Lesson 1 wording.
   - Fix: Addressed by Finding 2 fix (overlapping cleanup). Test annotations now describe pinned behavior: "Regression pin: inputs slightly larger than MAX_ERROR_ENTRY_LEN..." instead of cycle references.

**Test results at cdc4c64:** 22 sanitize unit tests pass (no behavior change — all changes are doc/comment); 60 test suites, 0 failures; cargo fmt --check + cargo clippy --all-targets -- -D warnings clean. CI in-flight on cdc4c64.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 3 Copilot R7 findings; Perplexity CONFIRMED OWASP terminology for Finding 1; Findings 2+3 no external claim (Perplexity skipped per Lesson 1) | All 3 findings confirmed valid; fix plan approved |
| implementer | Reword docstring ("strip" → "escape" + "keeping byte information visible to operator"); clean 6 inline comment sites; reword test annotations to describe pinned behavior | src/api/client.rs cdc4c64 |
| orchestrator | Commit cdc4c64; push; request R8 | 17/17 threads resolved; CI in-flight; R8 requested |
| state-manager | Third consecutive in-cycle dispatch per Lesson 2 — discipline is now habit | STATE.md, burst-log.md, pr-356-copilot-progress.md updated |

---

## Burst N+10 (2026-05-11): PR #356 Copilot Round 8 — Errors-Map Memory Bound + Doc Accuracy, fix commit e6262dd

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs (errors-map extraction bounded to MAX_ERROR_PAIRS=256 via `.iter().take(...)`; streaming join with upfront marker reservation; MAX_SANITIZED_OUTPUT_LEN doc reworded to describe retroactive-trim approach accurately)
**Versions bumped:** (none)

### Summary

PR #356 Copilot Round 8 (2026-05-11T19:41:09Z, review id 4266853645) returned 2 inline findings. Both valid. Fix commit e6262dd (+46 -7 lines).

**Process note: FOURTH consecutive in-cycle state-manager dispatch per codified Lesson 2.** R5 → R6 → R7 → R8 all dispatched state-manager in real time. The discipline is consistent habit.

**Perplexity-validation per Lesson 1 / DEC-018:**
- Finding 1 (errors-map memory amplification): RE-CITED OWASP A06/AP11 per Lesson 1 allowance for same-class findings already validated this cycle. R5 confirmed the same threat class (unbounded entry-count allocation pattern) for errorMessages; errors-map uses an identical `.iter().map(...).collect()` pattern with no entry-count bound. Same threat, same mitigation category, same prior validation still applies.
- Finding 2 (doc inaccuracy on MAX_SANITIZED_OUTPUT_LEN): NO EXTERNAL CLAIM — purely doc accuracy. Lesson 1 wording requires "at least one external-claim aspect" to warrant Perplexity. A comment describing a code mechanism has no such aspect. Skip is per-spec, not a rationalization.

**Findings:**

1. Errors-map memory amplification: The errors-map extraction path used `.iter().map(...).collect()` then sorted then joined — same unbounded entry-count pattern that R5 fixed for errorMessages. A hostile response with 1M keys would force ~100 MB allocation.
   - Threat class: OWASP A06:2021 Resource Exhaustion / AP11 (same as R5). Memory bounded to O(256 KiB) intermediate, O(4 KiB) output after fix.
   - Fix: Bounded entry count to `MAX_ERROR_PAIRS = 256` via `errors.iter().take(MAX_ERROR_PAIRS)`. Added streaming join with upfront marker reservation mirroring the errorMessages path. Tracks both `join_truncated` AND `pairs_truncated` states; marker reflects the active truncation condition.

2. MAX_SANITIZED_OUTPUT_LEN doc inaccuracy: Doc comment said "still leaving room for the marker via reserved headroom inside sanitize_for_stderr" — but the implementation uses retroactive trim, not reserved headroom. R4 restructured the implementation to retroactive trim, but the doc comment wasn't updated to match.
   - No external claim; Perplexity skipped per Lesson 1 wording.
   - Fix: Reworded doc to accurately describe the retroactive-trim approach: "after writing, the buffer is trimmed at a UTF-8 boundary if it exceeds the cap, then the truncation marker is appended."

**Test results at e6262dd:** 22 sanitize unit tests pass; 26 api_client integration tests pass; full cargo test 60 suites 0 failures (parallel-execution flake in unrelated multi_cloudid_disambiguation test passed on single-threaded retry); cargo fmt --check + cargo clippy --all-targets -- -D warnings clean. CI in-flight on e6262dd.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 2 Copilot R8 findings; Perplexity re-cited OWASP A06/AP11 for #1 (same class as R5); Finding 2 no external claim (Perplexity skipped per Lesson 1) | Both confirmed valid; fix plan approved |
| implementer | Bound errors-map to MAX_ERROR_PAIRS=256 with streaming join + upfront marker reservation; reword MAX_SANITIZED_OUTPUT_LEN doc | src/api/client.rs e6262dd (+46 -7) |
| orchestrator | Commit e6262dd; push; request R9 | 19/19 threads resolved; CI in-flight; R9 requested |

---

## Burst N+11 (2026-05-11): PR #356 Copilot Round 9 — Key-Amplification Cap + Bounded Value Serialization, fix commit 85f0dd4

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs (key wrapped in cap_entry before format!; new serialize_value_bounded helper using serde_json::to_writer with byte-limited Write impl)
**Versions bumped:** (none)

### Summary

PR #356 Copilot Round 9 (review 4266950826 @ 2026-05-11T19:55:57Z) returned 2 new inline findings from R9b, both Perplexity-validated as legitimate memory-amplification gaps. Fix commit 85f0dd4 pushed at 2026-05-11T15:13:09-0500. CI 8/8 green on 85f0dd4.

R9a (review 4266853645 @ 19:41:09Z) and R9c (comments @ 20:08:56-57Z) re-raised already-addressed concerns from prior rounds; 4 replies posted explaining prior resolutions. 4 R9b/R9c threads resolved; all 23 threads now resolved (0 unresolved).

**Process note: FIFTH consecutive in-cycle state-manager dispatch per codified Lesson 2.** R5 → R6 → R7 → R8 → R9 all dispatched state-manager in real time. The discipline is fully embedded.

**Perplexity-validation per DEC-018:**
- Finding 1 (key-amplification in format!("{k}: {v}")): CONFIRMED — large server-controlled keys (e.g., 1 MB) could amplify intermediate allocation even with the R8 entry-count cap. Keys are now wrapped in cap_entry(k) before the format! call. Perplexity validated this as a legitimate memory-amplification gap.
- Finding 2 (non-string errors values via v.to_string()): CONFIRMED — v.to_string() called full JSON serialization (materializing the entire value) before cap_entry truncated the result; deeply nested or huge values could force GB-scale allocations. New serialize_value_bounded(v, MAX_ERROR_ENTRY_LEN) helper uses serde_json::to_writer with a byte-limited Write impl returning WriteZero on overflow. Perplexity validated as a legitimate gap.

**Findings (R9b — review 4266950826):**

1. Key-amplification gap: `format!("{k}: {v}")` used the raw key `k` without any cap. With the R8 entry-count cap of MAX_ERROR_PAIRS=256, a server could still send 256 entries each with a 1 MB key — the intermediate format! allocation reaches 256 MB before the final join truncates. Fix: wrap key in `cap_entry(k)` before format!.

2. Non-string value serialization before cap: `v.to_string()` on a serde_json Value materializes the entire JSON subtree as a String before cap_entry truncates. A single deeply nested or large value forces a full allocation. Fix: new `serialize_value_bounded(v, MAX_ERROR_ENTRY_LEN)` helper writes to a WriteZeroOnOverflow adapter that returns WriteZero once the limit is hit, limiting output to MAX_ERROR_ENTRY_LEN bytes.

**R9a / R9c re-raised concerns:**
- R9a and R9c surfaced comments re-raising concerns that were already fully addressed in prior rounds (R5-R8). Four reply comments posted: 3221850022, 3221850177, 3221850294, 3221850424 (R9a/R9b) and 3222673033, 3222673079 (R9c). These explained the timing (R9a pre-dated 85f0dd4; R9c was mid-round) and prior resolutions.

**Test results at 85f0dd4:** 5 new unit tests pinning serialize_value_bounded contract; 27 sanitize unit tests total; 658 cargo test total green. Parallel-execution flake (test_interactive_render_shows_name_url_and_id in multi_cloudid_disambiguation) passes single-threaded — unrelated to this change.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage R9b 2 findings; Perplexity CONFIRMED both as legitimate memory-amplification gaps | Both confirmed valid; fix plan approved |
| implementer | Wrap key in cap_entry(k) before format!; implement serialize_value_bounded with WriteZeroOnOverflow adapter; 5 new unit tests | src/api/client.rs 85f0dd4 |
| orchestrator | Post 6 reply comments on R9a/R9b/R9c threads explaining prior resolutions; commit 85f0dd4; push; verify CI 8/8 green | 23/23 threads resolved; CI green; R10 pending |

---

## Burst N+12 (2026-05-11): PR #356 Copilot Round 10 — Truncation Marker Visibility in serialize_value_bounded, fix commit f328a2f

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs (Bounded writer tracks overflowed flag; serialize_value_bounded reserves marker bytes upfront; appends " [...truncated]" on overflow; degenerate fallback for limit < marker.len())
**Versions bumped:** (none)

### Summary

PR #356 Copilot Round 10 (review 4268026428 @ 2026-05-11T23:07:46Z, comment id 3222691664) returned 1 new inline finding, Perplexity-validated as a legitimate UX correctness gap. Fix commit f328a2f pushed at 2026-05-11T18:13:08-ish UTC. CI in-flight on f328a2f (Format + Secret Scan green; remaining checks pending — expected to settle 8/8 green per prior pattern).

This is the FIRST round where the finding count declined two consecutive times (R9: 2 → R10: 1). Trajectory now 4→1→2→2→3→2→3→2→2→1 — converging signal toward the Phase 8 stop condition (0-new-comment round).

1 R10 thread resolved (id 3222691664 → PRRT_kwDORs-xfc6BP1Oa); reply 3222725048 posted. All 24/24 threads now resolved (0 unresolved).

**Process note: SIXTH consecutive in-cycle state-manager dispatch per codified Lesson 2.** R5 → R6 → R7 → R8 → R9 → R10 all dispatched state-manager in real time. The discipline is fully embedded.

**Perplexity-validation per DEC-018:**
- Finding 1 (silent truncation in serialize_value_bounded): CONFIRMED — `serialize_value_bounded` produced a truncated JSON prefix WITHOUT any visible marker when overflow occurred. Since the returned String was `<= limit`, the downstream `cap_entry` call did NOT add its own marker either. Result: operators saw malformed-but-silently-incomplete JSON with no indication it was cut off. Perplexity confirmed this is a "looks valid but is actually malformed prefix" anti-pattern recognized in tracing/slog/OpenTelemetry conventions. Standard fix: track overflow flag; reserve marker bytes upfront so prefix-plus-marker fits within limit.

**Finding (R10 — review 4268026428, comment 3222691664):**

`serialize_value_bounded` used a `Bounded` writer that stopped writing once the byte limit was hit, but returned the partial (prefix-only) bytes silently with no truncation marker. The returned String was always `<= limit` so the downstream `cap_entry` call's marker logic was never triggered. The result: a silently malformed JSON prefix that looked like a valid JSON value to the operator.

**Fix:** `Bounded` writer now tracks an `overflowed: bool` flag. `serialize_value_bounded` reserves marker bytes upfront (`limit - " [...truncated]".len()`) so the prefix-plus-marker total fits within `limit`. Appends `" [...truncated]"` when `overflowed` is true. Degenerate-case fallback: when `limit < marker.len()`, returns the marker prefix truncated at `limit` (pinned via test).

**New tests (3 new + 1 updated):**
1. `test_serialize_value_bounded_no_marker_no_overflow` — small value: no marker, no overflow.
2. `test_serialize_value_bounded_marker_fits_within_limit` — oversized value: marker present, output within limit.
3. `test_serialize_value_bounded_degenerate_tiny_limit` — degenerate case: limit < marker.len(); output truncated at limit.
4. Updated existing oversized test to also assert marker present (previously only checked size invariant).

**Test results at f328a2f:** 30 sanitize unit tests total; 661 cargo test green; 0 failed.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 1 Copilot R10 finding (comment 3222691664 @ 23:07:46Z); Perplexity CONFIRMED as legitimate UX-correctness gap (silent truncation anti-pattern per tracing/slog/OpenTelemetry conventions) | Confirmed valid; fix plan approved |
| implementer | Add overflowed flag to Bounded writer; reserve marker bytes upfront in serialize_value_bounded; append " [...truncated]" on overflow; degenerate fallback pinned via test; 3 new tests + 1 updated | src/api/client.rs f328a2f |
| orchestrator | Resolve thread PRRT_kwDORs-xfc6BP1Oa; post reply 3222725048; commit f328a2f; push; request CI; verify Format+Secret Scan green | 24/24 threads resolved; CI in-flight; R11 pending |
| state-manager | Fourth consecutive in-cycle dispatch per Lesson 2 — consistent habit | STATE.md, burst-log.md, pr-356-copilot-progress.md updated |

---

## Burst: PR #356 Copilot R11 (2026-05-11T23:31Z)

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs
**Versions bumped:** (none — chore/sanitize-errors-334 branch)
**Commit:** 2ecc18c ("chore(security): byte-level size gate before JSON DOM parse (PR #356 R11)")
**CI result:** 8/8 green on 2ecc18c

### Summary

One new Copilot finding from R11 (review 4268102135 @ 23:27:03Z, comment id 3222756019): `extract_error_message_raw` deserialized the entire response body into `serde_json::Value` via `serde_json::from_str`, materializing a full DOM costing roughly 2-3x body size in memory. All prior R5-R10 caps bounded OUTPUT only; none prevented the INPUT DOM from being allocated. A hostile valid 100 MB JSON body would force 200-300 MB DOM allocation before any truncation occurred.

Fix: byte-level size gate via new constant `MAX_PARSE_BODY_LEN = 16 * 1024`. Bodies exceeding 16 KiB skip JSON parse and fall back to the existing byte-bounded raw-body path. Zero allocation attack surface — no serde_json::Value DOM is created for over-threshold bodies. Perplexity-validated as superior to streaming/partial parse approaches.

1 R11 thread resolved (id 3222756019 → PRRT_kwDORs-xfc6BQA9s); reply 3222775607 posted. All 25/25 threads now resolved (0 unresolved).

3 new unit tests: `test_extract_skips_parse_for_huge_body`, `test_extract_allows_normal_body`, `test_parse_body_threshold_pinned`. Total sanitize tests now 33; full cargo test: 664 passed, 0 failed, 10 ignored.

**Convergence signal:** Trajectory now 4→1→2→2→3→2→3→2→2→1→1. Finding count plateaued at 1 for two consecutive rounds (R10, R11). Healthy converging signal — R12=0 would trigger Phase 8 stop condition.

**Perplexity-validation per DEC-018:**
- Finding 1 (INPUT DOM allocation attack surface): CONFIRMED — `serde_json::from_str` allocates a full `serde_json::Value` DOM regardless of downstream truncation. Byte-level gate before parse is superior to streaming/partial parse (zero allocation attack surface vs. partial materialization). Prior R5-R10 caps bounded output only; this closes the input-side amplification vector.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 1 Copilot R11 finding (comment 3222756019 @ 23:27:03Z); Perplexity CONFIRMED as legitimate INPUT DOM allocation attack surface (serde_json::from_str materializes full Value regardless of downstream truncation) | Confirmed valid; fix plan approved |
| implementer | Add MAX_PARSE_BODY_LEN = 16 * 1024 constant; gate `serde_json::from_str` call behind byte-length check in `extract_error_message_raw`; bodies >16 KiB fall back to byte-bounded raw-body path; 3 new unit tests | src/api/client.rs 2ecc18c |
| orchestrator | Resolve thread PRRT_kwDORs-xfc6BQA9s; post reply 3222775607; commit 2ecc18c; push; verify CI 8/8 green | 25/25 threads resolved; CI green; R12 pending |
| state-manager | Seventh consecutive in-cycle dispatch per Lesson 2 — discipline is consistent habit | STATE.md, burst-log.md, pr-356-copilot-progress.md updated |

---

## Burst: PR #356 Copilot R12 (2026-05-11T23:43Z)

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs
**Versions bumped:** (none — chore/sanitize-errors-334 branch)
**Commit:** 6832967 ("chore(security): Write contract compliance + accurate non-UTF8 marker (PR #356 R12)")
**CI result:** 8/8 green on 6832967

### Summary

Two new Copilot findings from R12 (review 4268158285 @ 2026-05-11T23:39:52Z). Both Perplexity-validated.

**Finding 1 — `Bounded::write` violated `std::io::Write` contract (comment 3222800383):**
The `Bounded` writer's `write` method returned `Err(WriteZero)` when the byte limit was hit but `buf` still had unwritten bytes. The `std::io::Write` contract mandates: "If an error is returned then no bytes in the buffer were written." The prior implementation was writing a prefix into the buffer AND returning `Err(WriteZero)`, contradicting the contract. This could confuse serde_json's streaming serializer — if serde_json interpreted the error as a hard I/O failure it might produce inconsistent state.

Fix: return `Err(WriteZero)` ONLY when `remaining == 0` at the start of the call (nothing to write). For partial writes: append only the prefix that fits, set `overflowed = true`, and return `Ok(buf.len())` (the full input length, per the contract's "partial write is OK" allowance). On the subsequent call, `remaining == 0` fires immediately and returns `Err(WriteZero)`, stopping serde_json. This closes the contract violation while preserving the truncation semantics.

**Finding 2 — non-UTF8 fallback marker under-reported true body size (comment 3222800411):**
The non-UTF8 fallback path used `cap_entry` on a `from_utf8_lossy`-produced string. The `cap_entry` marker reported the post-pre-cap lossy string length (max ~4096 bytes), NOT the actual body length. For hostile or flood inputs (e.g., 1 MB non-UTF8 body), the marker `[...truncated; original 4096 bytes]` silently under-reported the true size — operators saw no signal that the body was large.

Fix: bypass `cap_entry` and build a custom marker: `[...truncated, {original_len} bytes total, non-UTF8 body]` where `original_len` is `body.len()` (the true byte count before any pre-capping). This provides accurate operator visibility into body size and explicitly flags the non-UTF8 source for disambiguation from normal JSON truncation.

2 R12 threads resolved (PRRT_kwDORs-xfc6BQI52, PRRT_kwDORs-xfc6BQI6M). All 27/27 threads now resolved (0 unresolved). Replies 3222826557 and 3222826602 posted.

3 new unit tests: partial-write produces marker, 5 MB body marker reports true size, small non-UTF8 body skips marker. Total sanitize tests now 36; full cargo test: 667 passed, 0 failed, 10 ignored.

**Trajectory note:** R12 ticked back up to 2 findings (trajectory 4→1→2→2→3→2→3→2→2→1→1→2). Both findings are distinct from R11's INPUT-DOM class (contract-level + UX-level vs DOM-allocation). Not a regression — Copilot is exploring different correctness categories. Expect 2-4 more rounds. R13 will be the telltale: if R13 returns 0-1, convergence is on track. R13 pending.

**Perplexity-validation per DEC-018:**
- Finding 1 (std::io::Write contract violation): CONFIRMED — `std::io::Write` contract mandates "no bytes written if error returned." The prior partial-write + error combination violated this. Perplexity-validated as a legitimate contract violation with real risk of confusing downstream callers.
- Finding 2 (non-UTF8 marker under-reporting): CONFIRMED — accurate body-size reporting is required for operator diagnostics; using the post-cap length silently hides the true input size for hostile/flood inputs. Custom marker with `body.len()` is the correct approach.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 2 Copilot R12 findings (comments 3222800383, 3222800411 @ 23:39:52Z); Perplexity CONFIRMED both as legitimate (Write contract violation + non-UTF8 marker under-reporting) | Both confirmed valid; fix plan approved |
| implementer | Fix Bounded::write to return Err(WriteZero) only on remaining==0; on partial write: append prefix, set overflowed, return Ok(buf.len()); build custom non-UTF8 marker using body.len(); 3 new unit tests | src/api/client.rs 6832967 |
| orchestrator | Resolve threads PRRT_kwDORs-xfc6BQI52 + PRRT_kwDORs-xfc6BQI6M; post replies 3222826557 + 3222826602; commit 6832967; push; verify CI 8/8 green | 27/27 threads resolved; CI green; R13 pending |
| state-manager | Eighth consecutive in-cycle dispatch per Lesson 2 — discipline is consistent habit | STATE.md, burst-log.md, pr-356-copilot-progress.md updated |

---

## Burst: PR #356 Copilot R13 (2026-05-11T23:55Z)

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs
**Versions bumped:** (none — chore/sanitize-errors-334 branch)
**Commit:** bcc2db4 ("chore(security): correct OWASP/CWE labels for memory-amplification defenses (PR #356 R13)")
**CI result:** 8/8 green on bcc2db4

### Summary

One new Copilot finding from R13 (review 4268206656 @ 2026-05-11T23:52:40Z, comment 3222841940). Perplexity-validated as a real labeling error.

**Finding — OWASP/CWE label inaccuracy in doc comments (comment 3222841940):**
Doc comments throughout `src/api/client.rs` labeled the memory-amplification mitigation as "OWASP A06 / AP11" — both incorrect. OWASP A06:2021 is "Vulnerable and Outdated Components" (dependency vulnerabilities, not resource exhaustion). "AP11" does not correspond to any recognized standard categorization scheme (not OWASP API Security Top 10, not OWASP Top 10, not CWE, not CVE).

The correct labels for this threat class (unbounded resource allocation from server-controlled input): **OWASP API4:2023 (Unrestricted Resource Consumption) / CWE-770 (Allocation of Resources Without Limits or Throttling)**.

**Validation (Perplexity per DEC-018):** CONFIRMED — OWASP API4:2023 is unambiguously the correct category for unrestricted resource consumption. CWE-770 maps to allocation-without-limits. Both are authoritative and widely cited for this threat class. Perplexity confirmed the original labels (A06/AP11) were incorrect.

Fix: mechanical search-and-replace across 6 comment locations in `src/api/client.rs`. No behavior change. Historical commit messages and prior reply comments retain old labels (immutable history); correction lives in current source code comments where future maintainers will read.

1 R13 thread resolved (PRRT_kwDORs-xfc6BQQan). All 28/28 threads now resolved (0 unresolved). Reply 3222883003 posted.

No new tests (comment-only change); 36 sanitize tests still pass; full cargo test: 667 passed, 0 failed.

**Convergence signal:** R13 returned 1 finding — down from R12's 2 (trajectory segment ...→1→1→2→1). Crucially, the finding is documentation-quality (OWASP label correctness) rather than a security-defense gap. This shift in finding category is a strong convergence indicator: the security defenses themselves are converged; Copilot is now exploring incidental quality issues. Phase 8 stop condition (0-new-comment round) is likely 1-2 rounds away.

**Perplexity-validation per DEC-018:**
- Finding (OWASP A06 / AP11 mislabeling): CONFIRMED — OWASP A06:2021 is "Vulnerable and Outdated Components"; correct label for resource exhaustion defense is OWASP API4:2023 / CWE-770. Authoritative references cited in commit message.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 1 Copilot R13 finding (comment 3222841940 @ 23:52:40Z); Perplexity CONFIRMED OWASP A06 / AP11 are incorrect labels for resource exhaustion defense; correct labels are OWASP API4:2023 / CWE-770 | Confirmed valid; fix plan approved |
| implementer | Search-and-replace across 6 comment locations in src/api/client.rs; no behavior change | src/api/client.rs bcc2db4 |
| orchestrator | Resolve thread PRRT_kwDORs-xfc6BQQan; post reply 3222883003; commit bcc2db4; push; verify CI 8/8 green | 28/28 threads resolved; CI green; R14 pending |
| state-manager | Ninth consecutive in-cycle dispatch per Lesson 2 — discipline is consistent habit | STATE.md, burst-log.md, pr-356-copilot-progress.md updated |

---

## PR #356 Copilot R14 Fix Burst (2026-05-12T00:14 UTC)

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs (sanitize_for_stderr + tests), .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md, .factory/cycles/cycle-001/adversarial-reviews/pr-356-sanitize-errors/pr-356-copilot-progress.md
**Versions bumped:** (none — chore/security hardening only)
**Commit:** d4a07c8 ("chore(security): escape Unicode C1 controls in sanitize_for_stderr (PR #356 R14)")
**CI:** 8/8 green on d4a07c8

### Summary

1 finding from Copilot R14 (review 4268270089 @ 2026-05-12T00:10:42Z, comment id 3222898738). Perplexity-validated as legitimate defense-in-depth hardening:

**Finding (Unicode C1 control escape gap):** `sanitize_for_stderr` used `is_ascii_control()` to identify control characters, which covers only C0 controls (U+0000..U+001F) and DEL (U+007F), but misses Unicode C1 controls U+0080..U+009F. The C1 range includes CSI (U+009B, Control Sequence Introducer) and NEL (U+0085, Next Line) — characters that legacy/embedded/non-UTF8 terminals can interpret as control sequences, enabling the same terminal injection threat class as C0.

Modern UTF-8 terminals silently drop C1 bytes as invalid continuation bytes (not a current exploitation vector in mainstream environments), but the defense-in-depth rationale holds for legacy/embedded terminal contexts. The finding is correctly categorized as defense-in-depth hardening, consistent with the overall PR #356 security posture.

**Fix:** Switch `is_ascii_control()` to `char::is_control()` in `sanitize_for_stderr`, which covers both C0 (U+0000..U+001F + DEL U+007F) and C1 (U+0080..U+009F). Branch on `c.is_ascii()` for escape format: ASCII controls keep `\xNN` (4 bytes); C1 controls use `\u{NNNN}` (8 bytes). Fast-path scan changed from byte-level `bytes().any(|b| b.is_ascii_control())` to char-level `chars().any(|c| c.is_control())` — required because byte-level scanning cannot distinguish C1 control code-point bytes from valid 2-byte UTF-8 continuation bytes. The 4x expansion budget (4 KiB cap) comfortably absorbs the 8-byte `\u{NNNN}` escapes for C1 characters.

3 new unit tests added: CSI escape (U+009B → `\u{009b}`), NEL escape (U+0085 → `\u{0085}`), anti-regression for non-control Unicode above ASCII (U+00C0 LATIN CAPITAL LETTER A WITH GRAVE — must pass through unescaped). Total sanitize tests now 39; full cargo test: 670 passed, 0 failed, 10 ignored.

1 R14 thread resolved (PRRT_kwDORs-xfc6BQamK). All 29 threads now resolved (0 unresolved). Reply 3222921647 posted.

**Trajectory:** 4→1→2→2→3→2→3→2→2→1→1→2→1→1 — two consecutive 1-finding rounds (R13, R14). Finding category remains defense-in-depth / documentation-quality rather than security-defense gaps. R15 may be the convergence round (0 new findings = Phase 8 stop condition).

**Perplexity-validation per DEC-018:**
- Finding (C1 control escape gap): CONFIRMED — `char::is_control()` covers C0 + C1; `is_ascii_control()` misses C1. Defense-in-depth rationale validated for legacy/embedded terminal contexts. C1 `\u{NNNN}` format is the standard Rust Unicode escape format.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 1 Copilot R14 finding (comment 3222898738 @ 00:10:42Z); Perplexity CONFIRMED C1 gap is legitimate defense-in-depth hardening; fix plan approved | Confirmed valid |
| implementer | Switch `is_ascii_control()` to `char::is_control()`; branch on `c.is_ascii()` for `\xNN` vs `\u{NNNN}` format; fix fast-path scan to char-level; add 3 new unit tests | src/api/client.rs d4a07c8 |
| orchestrator | Resolve thread PRRT_kwDORs-xfc6BQamK; post reply 3222921647; commit d4a07c8; push; verify CI 8/8 green | 29/29 threads resolved; CI green; R15 pending |
| state-manager | Tenth consecutive in-cycle dispatch per Lesson 2 — discipline is consistent habit | STATE.md, burst-log.md, pr-356-copilot-progress.md updated |

---

## Burst (2026-05-12): PR #356 Copilot Round 15 — 2 doc-quality findings, fix commit 7f0177d

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs (comment-only: fast-path comment rewritten; all R-number annotations stripped)
**Versions bumped:** (none)

### Summary

PR #356 Copilot Round 15 (review 4268312988 @ 2026-05-12T00:23:00Z) returned 2 inline findings.
Both were documentation/annotation quality issues. No security or behavioral gaps identified;
substantive defenses are unchanged since R14.

**Finding C1 (comment 3222937344):** The fast-path comment in `sanitize_for_stderr` still
described byte-level scanning (`bytes().any(...)`) even though R14 had switched the implementation
to char-level `chars().any(|c| c.is_control())`. Rewritten to accurately describe the current
char-level fast path and explain why byte-level scanning cannot be used: C1 control code points
(U+0080..U+009F) are encoded as 2-byte UTF-8 sequences (0xC2 0x80..0x9F) that are
indistinguishable from valid 2-byte UTF-8 continuation bytes at the byte level.

**Finding C2 (comment 3222937368):** Stale internal "(R10 finding)" annotation on the
`serialize_value_bounded` marker comment. This is the same annotation-hygiene class as R7
(where R2/R3/R6 round annotations were cleaned from production comments and test files).
Fix was broader than the single flagged instance: systematic strip of ALL R-number annotations
across the file — "(R10 finding)", "(R11 finding)", "(R12 finding)", "(R9 finding)",
"(R9 defense — see comment block above)", "R10 pin: ", "R14 anti-regression: ",
"R10 degenerate case: ", "R12 pins — ", etc.

**No new tests.** Both changes are comment-only; the 39 sanitize tests and 670 cargo test suite
remain unchanged and green.

**Threads resolved:** PRRT_kwDORs-xfc6BQhi- and PRRT_kwDORs-xfc6BQhjV (2 R15 threads).
All 31 threads resolved (0 unresolved).
**Replies posted:** 3222972524 and 3222972567.

**Trajectory:** 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2 — R15 was 2 findings but both documentation
cleanup. Substantive defenses have been converged since R14. Recent 5-round window: 1, 2, 1, 1, 2
(averaging 1.4 findings/round), all in the defense-in-depth / documentation category.
R16 is likely the Phase 8 stop condition (0-new-comment round).

**Perplexity-validation per DEC-018:** Both findings are purely internal-consistency /
annotation-accuracy questions with no external library or API behavior claims. No external
Perplexity validation required per Lesson 1 wording ("at least one external-claim aspect").
Skip is per-spec, not a rationalization.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 2 Copilot R15 findings (comments 3222937344 + 3222937368 @ 00:23:00Z); both documentation-only; Perplexity not required (no external claims per Lesson 1) | Confirmed valid documentation gaps |
| implementer | Rewrite fast-path comment in `sanitize_for_stderr` to describe char-level scan and explain C1 2-byte UTF-8 encoding constraint; systematically strip all R-number annotations from src/api/client.rs | src/api/client.rs 7f0177d |
| orchestrator | Resolve threads PRRT_kwDORs-xfc6BQhi- + PRRT_kwDORs-xfc6BQhjV; post replies 3222972524 + 3222972567; commit 7f0177d; push; verify CI 8/8 green | 31/31 threads resolved; CI green; R16 pending |

---

## Burst: PR #356 Copilot R16 (2026-05-12T00:38Z)

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs, tests/api_client.rs
**Versions bumped:** (none)
**Commit:** dc09501 ("chore(security): correct doc strategy bullets + accurate C1 terminal behavior (PR #356 R16)")

### Summary

Copilot R16 returned 3 findings (review 4268365143 @ 00:38Z), all doc-accuracy consequences of
the R14 C1-control expansion. No behavior change; no new tests; 39 sanitize tests + 670 cargo
test unchanged and green. CI 8/8 green on dc09501. 34/34 threads resolved (0 unresolved).

**Trajectory:** 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2→3 — R16 ticked up to 3 but all doc-fallout
from R14. Substantive defenses unchanged since R14. 12 consecutive in-cycle state-manager
dispatches (Lesson 2). R17 pending; predicted 0-1 findings (Phase 8 stop condition within reach).

**Perplexity-validation per DEC-018:** All 3 findings are purely internal-consistency /
doc-accuracy questions with no external library or API behavior claims. Perplexity not required
per Lesson 1 wording ("at least one external-claim aspect"). Skips are per-spec.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 3 Copilot R16 findings (comments 3222985472 + 3222985491 + 3222985507 @ 00:38Z); all doc-accuracy consequences of R14 C1 expansion; Perplexity not required (no external claims per Lesson 1) | Confirmed valid doc-accuracy gaps |
| implementer | (C1) Rewrite strategy bullets in `sanitize_for_stderr` to list both escape branches (`\xNN` C0/DEL, `\u{NNNN}` C1); (C2) Fix C1 control description — not "invalid UTF-8 continuation bytes" but "valid 2-byte UTF-8 encoding whose semantics modern terminals ignore in UTF-8 mode"; (C3) Update integration test comment "only ASCII control bytes" → "only control characters (ASCII C0/DEL and Unicode C1)" | src/api/client.rs + tests/api_client.rs dc09501 |
| orchestrator | Resolve threads PRRT_kwDORs-xfc6BQqRd + PRRT_kwDORs-xfc6BQqRt + PRRT_kwDORs-xfc6BQqR6; post replies 3223009560 + 3223009636 + 3223009710; commit dc09501; push; verify CI 8/8 green | 34/34 threads resolved; CI green; R17 pending |
| state-manager | Update STATE.md + burst-log.md + pr-356-copilot-progress.md for R16; commit factory-artifacts | Factory state current through R16 |
| state-manager | Eleventh consecutive in-cycle dispatch per Lesson 2 | STATE.md, burst-log.md, pr-356-copilot-progress.md updated |

---

## Burst: PR #356 Copilot R17 (2026-05-12T00:55Z)

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** tests/api_client.rs (comment-only: header comment updated to mention both ASCII \xNN and C1 \u{NNNN} escapes)
**Versions bumped:** (none)
**Commit:** fb91f32 ("chore(security): correct integration-test header comment for C1 escapes (PR #356 R17)")
**CI:** 8/8 green on fb91f32

### Summary

Copilot R17 returned 1 finding (review 4268400605 @ 00:54Z, comment id 3223021119). Comment-only
change; no behavior change; no new tests; 39 sanitize tests + 26 api_client tests pass;
670 cargo test green. CI 8/8 green on fb91f32. 35/35 threads resolved (0 unresolved).

**Finding (CWE-117 integration-test header comment stale, comment 3223021119):**
The header comment block in `tests/api_client.rs` described the sanitization as rendering hostile
control chars "as \xNN literals". This was accurate before R14 but became incomplete after R14
expanded the escape set to include Unicode C1 controls (U+0080..U+009F), which use `\u{NNNN}`
format rather than `\xNN`. The comment now reads to cover both: ASCII C0/DEL chars escaped as
`\xNN` and C1 chars escaped as `\u{NNNN}`.

**Perplexity-validation per DEC-018:** No external library or API behavior claims — purely
internal doc accuracy. Perplexity skipped per Lesson 1 ("at least one external-claim aspect"
required). Skip is per-spec, not a rationalization.

**Thread resolved:** PRRT_kwDORs-xfc6BQwwb (1 new R17 thread). All 35/35 threads resolved
(0 unresolved). Reply 3223040033 posted.

**Trajectory:** 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2→3→1 — R17 down to 1, continuing the tapering
of the R14 doc-fallout cluster (R15:2 → R16:3 → R17:1). Substantive defenses unchanged since
R14. Phase 8 prediction: R18 likely 0-finding stop condition.

**Perplexity-validation per DEC-018:** No external claims; skip per Lesson 1.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 1 Copilot R17 finding (comment 3223021119 @ 00:54Z, review 4268400605); doc-accuracy only; Perplexity not required (no external claims per Lesson 1) | Confirmed valid doc-accuracy gap |
| implementer | Extend tests/api_client.rs header comment to mention both `\xNN` (ASCII C0/DEL) and `\u{NNNN}` (C1) escapes | tests/api_client.rs fb91f32 |
| orchestrator | Resolve thread PRRT_kwDORs-xfc6BQwwb; post reply 3223040033; commit fb91f32; push; verify CI 8/8 green | 35/35 threads resolved; CI green; R18 pending |
| state-manager | Thirteenth consecutive in-cycle dispatch per Lesson 2 | STATE.md, burst-log.md, pr-356-copilot-progress.md updated |

---

## Burst: PR #356 Copilot R18 (2026-05-12T01:07Z)

**Agents dispatched:** orchestrator, implementer, state-manager
**Files touched:** src/api/client.rs (comment-only: public-API doc extended to describe both ASCII \xNN and C1 \u{NNNN} escape branches; threat-model phrase extended from "CR/LF/ANSI" to "CR/LF/ANSI/CSI")
**Versions bumped:** (none)
**Commit:** 9acf01d ("chore(security): correct extract_error_message public-API doc for C1 escapes (PR #356 R18)")
**CI:** 8/8 green on 9acf01d

### Summary

Copilot R18 returned 1 finding (review 4268435007 @ 01:05Z, comment id 3223053065). Comment-only
change; no behavior change; no new tests; 39 sanitize tests + 26 api_client tests pass;
670 cargo test green. CI 8/8 green on 9acf01d. 36/36 threads resolved (0 unresolved).

**Finding (CWE-117 public-API doc comment stale, comment 3223053065):**
The `extract_error_message` public-API doc comment (visible to all callers of the public API)
described only the ASCII control character escape branch — "escapes ASCII control chars ... as
\xNN". This was accurate before R14 but became incomplete after R14 expanded the escape set to
also cover Unicode C1 controls (U+0080..U+009F), which are escaped as `\u{NNNN}` rather than
`\xNN`. In addition, the threat-model phrase "protects against CR/LF/ANSI injection" omitted
CSI (U+009B, the C1 control sequence introducer). Fixed by: extending the doc to accurately
describe both branches (C0/DEL → `\xNN`, C1 → `\u{NNNN}`) and expanding the threat-model
phrase to "CR/LF/ANSI/CSI injection".

**Perplexity-validation per DEC-018:** No external library or API behavior claims — purely
internal doc accuracy. Perplexity skipped per Lesson 1 ("at least one external-claim aspect"
required). Skip is per-spec, not a rationalization.

**Thread resolved:** PRRT_kwDORs-xfc6BQ2o4 (1 new R18 thread). All 36/36 threads resolved
(0 unresolved). Reply 3223074074 posted.

**Trajectory:** 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2→3→1→1 — R18 held at 1, completing the
R14 doc-fallout cluster tapering (R15:2 → R16:3 → R17:1 → R18:1). This is the final known
doc-fallout item from R14's C1 expansion. Substantive defenses unchanged since R14. All known
doc sites now updated: public API doc (R18), strategy bullets (R16 C1), C1 description (R16 C2),
integration test comment (R17), R-number cleanup in progress records (prior rounds). Phase 8
prediction: R19 very likely 0-finding stop condition.

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage 1 Copilot R18 finding (comment 3223053065 @ 01:05Z, review 4268435007); doc-accuracy only; Perplexity not required (no external claims per Lesson 1) | Confirmed valid doc-accuracy gap |
| implementer | Extend extract_error_message public-API doc to describe both `\xNN` (ASCII C0/DEL) and `\u{NNNN}` (C1) escapes; expand threat-model phrase from "CR/LF/ANSI" to "CR/LF/ANSI/CSI" | src/api/client.rs 9acf01d |
| orchestrator | Resolve thread PRRT_kwDORs-xfc6BQ2o4; post reply 3223074074; commit 9acf01d; push; verify CI 8/8 green | 36/36 threads resolved; CI green; R19 pending |
| state-manager | Fourteenth consecutive in-cycle dispatch per Lesson 2 | STATE.md, burst-log.md, pr-356-copilot-progress.md updated |

---

## Burst: PR #356 Copilot R19 — Phase 8 Stop Condition (2026-05-12T01:18Z)

**Agents dispatched:** orchestrator, state-manager
**Files touched:** none (no code or doc changes — stop condition round)
**Versions bumped:** (none)
**Commit:** n/a (no fix commit for stop-condition round)
**CI:** 8/8 green on 9acf01d (unchanged head)

### Summary

Copilot R19 (review id 4268474794 @ 2026-05-12T01:18:43Z) returned zero inline comments.
Review body: "Copilot reviewed 2 out of 2 changed files in this pull request and generated
no new comments." Phase 8 stop condition met per validated-feature-lifecycle skill:
"a freshly-requested Copilot review posts zero new inline comments. The overview comment
alone (no file-level findings) is not a reason to continue."

PR #356 is CONVERGED. No further Copilot rounds are needed. PR is ready for human merge
approval.

**Final cycle stats:**
- 19 rounds total (R0 initial PR + 18 fix rounds + R19 stop)
- 18 fix commits: 51e2807 (R1) → d061b14 (R2) → 274961c (R3) → fe25e22 (R4) → c9be4de (R5)
  → 59a0a12 (R6) → cdc4c64 (R7) → e6262dd (R8) → 85f0dd4 (R9) → f328a2f (R10)
  → 2ecc18c (R11) → 6832967 (R12) → bcc2db4 (R13) → d4a07c8 (R14) → 7f0177d (R15)
  → dc09501 (R16) → fb91f32 (R17) → 9acf01d (R18)
- Head at stop: 9acf01d
- Tests: 670 passed, 0 failed, 10 ignored (39 sanitize unit + 26 api_client integration)
- CI: 8/8 green
- Review threads: 36/36 resolved (0 unresolved)
- Mergeable: CLEAN
- Final trajectory: 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2→3→1→1→0

**Defense profile post-convergence:**
- CWE-117: ASCII C0/DEL escaping (\xNN) + Unicode C1 escaping (\u{NNNN}) via char::is_control()
- CWE-770 / OWASP API4:2023 memory amplification: bounded at every stage (UTF-8 conversion ≤4 KiB,
  JSON parse input ≤16 KiB, DOM worst case ≤~48 KiB, per-entry caps ≤1 KiB, streaming joins ≤4 KiB,
  final output ≤4 KiB)
- std::io::Write contract compliance for bounded writer
- Accurate truncation markers with original byte counts
- All doc comments accurate post-R14 C1 expansion (R15-R18 doc-fallout cluster fully resolved)

**Process milestone:**
- 15 consecutive in-cycle state-manager dispatches (Lesson 2 compliance — RECORD for this project)
- 12 Perplexity validations per Lesson 1 / DEC-018
- R14 doc-fallout cluster fully resolved (R15:2 → R16:3 → R17:1 → R18:1 → R19:0)

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage R19 review (id 4268474794 @ 01:18:43Z); confirm stop condition; no findings to dispatch | Phase 8 stop condition confirmed; PR declared CONVERGED |
| state-manager | Final state update: mark PR #356 CONVERGED; update trajectory to →0; archive R19 record; append lessons | STATE.md, burst-log.md, pr-356-copilot-progress.md, lessons.md updated |
| state-manager | Fifteenth consecutive in-cycle dispatch per Lesson 2 | STATE.md, burst-log.md, pr-356-copilot-progress.md, lessons.md updated |

---

## Burst: PR #357 OPENED — #335 release-gate JR_BASE_URL (RETROACTIVE, 2026-05-12)

**Date:** 2026-05-12 (retroactive — state-manager dispatch skipped at PR creation)
**Agents:** orchestrator (implementer), state-manager (retroactive)
**Branch:** chore/release-gate-jr-base-url-335
**Head commit:** cb3e8a3
**PR:** #357 — https://github.com/Zious11/jira-cli/pull/357
**Input files touched:** src/api/client.rs (+4 lines), CLAUDE.md (+4 lines net)
**factory-artifacts commit:** this commit

### Summary

PR #357 opened implementing issue #335: release-gate the `JR_BASE_URL` environment variable
behind `#[cfg(debug_assertions)]` to prevent token leakage via env override in release builds.

**Security context:** `JR_BASE_URL` overrides the configured Jira instance URL and is used by
tests to inject a wiremock server. In a release binary, a hostile environment variable
`JR_BASE_URL=http://attacker.example/` would redirect all authenticated HTTP requests (including
those carrying OAuth access tokens) to a non-Atlassian host — a token-exfiltration vector.

**Fix:** Wrapped the `std::env::var("JR_BASE_URL")` read in `src/api/client.rs` with
`#[cfg(debug_assertions)]`, returning `None` in release builds. The change mirrors the
existing `JR_AUTH_HEADER` gate (SD-002 resolution, same file ~line 72). 8 lines total
(+4 in client.rs, +4 in CLAUDE.md "AI Agent Notes" section clarifying debug-only scope).

**Perplexity pre-validation (RETROACTIVE — run after user course-corrected skipped dispatch):**
- `#[cfg(debug_assertions)]` confirmed as idiomatic compile-time gate (prior art: gh CLI,
  aws-cli, kubectl all use compile-time gating for test endpoints).
- `cargo build --release` reliably disables debug_assertions; not overridable without explicit
  `debug-assertions = true` in `[profile.release]` override.
- Cargo.toml verified: no `debug-assertions = true` in release profile (clean).
- Better than alternatives: runtime env flag (deploy-time vuln if env accidentally set),
  feature flag (release-process risk), URL allow-list (overkill).

**Process gap — same rationalization pattern as DEC-018/Lessons 1+2:**
State-manager dispatch was skipped at PR creation with rationalization "pattern already
established in same file." This is exactly the failure mode captured in Lesson 1 (Perplexity
validation) and Lesson 2 (per-round state-manager). The equivalent rule for a single-burst PR:
state-manager dispatch is required at PR creation, not only per-Copilot-round. Lesson 2
addendum captured in lessons.md.

**Test results at cb3e8a3:**
- cargo test: 60 groups, 1244 passed, 0 failed, 10 ignored
- cargo fmt --check: PASS
- cargo clippy --all-targets -- -D warnings: PASS (debug)
- cargo clippy --all-targets --release -- -D warnings: PASS (NEW — added release-mode clippy)
- All 182 existing JR_BASE_URL test usages work in debug builds (CI default)
- Tests using `JiraClient::new_for_test(base_url, auth_header)` bypass env-var resolution entirely

**Documentation sweep:**
- CLAUDE.md "AI Agent Notes" updated: clarified `JR_BASE_URL` is debug-only with rationale
- docs/specs/issue-create-json-full-shape.md:87 references JR_BASE_URL as "existing pattern
  in tests/" — accurate, no change needed
- No README.md mentions

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Implement #[cfg(debug_assertions)] gate on JR_BASE_URL in src/api/client.rs | +4 lines mirroring SD-002 / JR_AUTH_HEADER gate |
| orchestrator | Update CLAUDE.md AI Agent Notes for debug-only scope | +4 lines (clarification + rationale) |
| orchestrator | Run cargo test / fmt / clippy (debug + release modes) | All green; 1244 passed |
| orchestrator | Open PR #357 (closes #335) | https://github.com/Zious11/jira-cli/pull/357 |
| orchestrator | Request Copilot review | Copilot R1 requested 2026-05-12 |
| research-agent | Perplexity pre-validation (retroactive) | CONFIRMED #[cfg(debug_assertions)] idiomatic; Cargo.toml clean; prior art validated |
| state-manager | Retroactive state update — PR #357 opened, PR #356 MERGED noted, Lessons 1+2 recurrence captured | STATE.md, burst-log.md, lessons.md, pr-357-copilot-progress.md |

**Outcome:** PR #357 OPEN @ cb3e8a3 (closes #335). Copilot R1 requested. CI in-flight. 8 audit-followups remain after #335 closes: #331, #333, #336, #340, #343, #345, #346, #350.

---

## Burst: PR #357 R1 COMPLETE — 3 findings resolved (2026-05-12)

**Date:** 2026-05-12 (~02:26–02:35 UTC)
**Agents:** orchestrator (implementer), state-manager
**Branch:** chore/release-gate-jr-base-url-335
**Head at R1 open:** cb3e8a3
**Fix commit:** 144aaff ("chore(security): gate Config::base_url JR_BASE_URL read + add regression tests (PR #357 R1)")
**PR:** #357 — https://github.com/Zious11/jira-cli/pull/357
**Copilot review:** 4268736728 @ 2026-05-12T02:26:30Z
**Files touched (develop):** src/config.rs (+#[cfg(debug_assertions)] gate on Config::base_url JR_BASE_URL read), tests/base_url_release_gate.rs (new, 4 tests), CLAUDE.md (two-site gating doc correction)
**factory-artifacts commit:** this commit

### Summary

Copilot R1 review (3 findings, all Perplexity-validated as legitimate):

**Finding 1 — CRITICAL (comment 3223330261):**
`Config::base_url()` at `src/config.rs:357` also read `JR_BASE_URL` unconditionally. The
initial fix (cb3e8a3) gated only the secondary read site in `src/api/client.rs` (the
`JiraClient::new` base-URL override). The primary read site in `Config::base_url()` was
missed — an attacker environment with `JR_BASE_URL=http://attacker.example/` would still
route all requests through the config layer.

Root cause: grep of `JR_BASE_URL` across `src/` was not performed before pushing. The
mental model conflated "the env-var read I edited" with "all places the env var is read."

**Fix:** Applied `#[cfg(debug_assertions)]` gate to `Config::base_url()`, returning
`None` in release builds. Now both read sites are gated.

**Finding 2 — MEDIUM (comment 3223330280):**
Missing regression test mirroring `tests/auth_header_release_gate.rs`. Created
`tests/base_url_release_gate.rs` with 4 tests (all named `test_335_*`):
- `test_335_base_url_gate_source_present_in_config_rs` — source-level grep pin
- `test_335_base_url_gate_source_present_in_client_rs` — source-level grep pin (both sites)
- `test_335_base_url_gate_compile_time_evidence` — compile-time gate evidence
- `test_335_new_for_test_bypasses_env_var_resolution` — regression guard for test helper

**Finding 3 — LOW (comment 3223330291):**
CLAUDE.md "AI Agent Notes" section claimed release ignores `JR_BASE_URL` but only one site
was gated at cb3e8a3. Updated to reflect two-site gating and reference the new regression
test file.

### Perplexity Validation

All 3 findings validated before acting per DEC-018:
- Finding 1: confirmed that `Config::base_url()` reading JR_BASE_URL creates a token-leak
  vector identical to the client.rs path; two-site gating required.
- Finding 2: confirmed regression test pattern (source-level grep pins) is idiomatic for
  compile-time gate verification; `auth_header_release_gate.rs` is the established prior art.
- Finding 3: confirmed CLAUDE.md accuracy is load-bearing for AI agent sessions that read
  it as context (false claim would cause agents to skip the gate in future work).

### Process Note — Surface Area vs Approach

New sub-lesson codified from this round:

**"Perplexity validates the APPROACH; grep validates the SURFACE AREA. Both are required
for security-sensitive env-var gating. Always grep before claiming closure."**

In this case: Perplexity confirmed `#[cfg(debug_assertions)]` is the correct approach. But
`grep -rn JR_BASE_URL src/` would have revealed the Config::base_url() read site BEFORE
pushing cb3e8a3. That grep was not run. Copilot caught it in one round.

The sub-lesson is appended under Lesson 1 in `cycles/cycle-001/lessons.md`.

### Results

| Metric | cb3e8a3 (before R1) | 144aaff (after R1) |
|--------|---------------------|--------------------|
| cargo test | 1244 passed | 1248 passed (+4 test_335_*) |
| cargo fmt --check | PASS | PASS |
| cargo clippy debug | PASS | PASS |
| cargo clippy --release | PASS | PASS |
| JR_BASE_URL read sites gated | 1 of 2 | 2 of 2 |
| Copilot threads resolved | 0 | 3/3 |
| CI (8 checks) | green | green |

### Thread Dispositions

| Thread ID | Comment | Finding | Status |
|-----------|---------|---------|--------|
| PRRT_kwDORs-xfc6BRm7j | 3223330261 | Config::base_url() CRITICAL | Resolved — reply 3223391764 |
| PRRT_kwDORs-xfc6BRm7q | 3223330280 | Missing regression test | Resolved — reply 3223391824 |
| PRRT_kwDORs-xfc6BRm7w | 3223330291 | CLAUDE.md doc inaccuracy | Resolved — reply 3223391863 |

### Details

| Agent | Task | Output |
|-------|------|--------|
| orchestrator | Triage R1 review (id 4268736728 @ 02:26:30Z); Perplexity-validate all 3 findings | All 3 confirmed legitimate |
| orchestrator | Fix Finding 1: gate Config::base_url() JR_BASE_URL read with #[cfg(debug_assertions)] | src/config.rs patched |
| orchestrator | Fix Finding 2: create tests/base_url_release_gate.rs with 4 test_335_* tests | New test file; 1248 passed |
| orchestrator | Fix Finding 3: update CLAUDE.md to reflect two-site gating | CLAUDE.md updated |
| orchestrator | Push fix commit 144aaff; confirm CI 8/8 green | develop @ 144aaff |
| orchestrator | Resolve 3 threads; post replies 3223391764, 3223391824, 3223391863 | 3/3 threads resolved |
| orchestrator | Request Copilot R2 | R2 pending |
| state-manager | Update STATE.md, burst-log.md, lessons.md, pr-357-copilot-progress.md | This commit |

**Outcome:** PR #357 R1 COMPLETE @ 144aaff. 3/3 R1 threads resolved. CI 8/8 green. Two-site JR_BASE_URL gating confirmed. R2 requested.

---

## Burst N+1 — PR #357 R2 Convergence (2026-05-12)

**Agents dispatched:** state-manager
**Files touched:** .factory/STATE.md, .factory/cycles/cycle-001/burst-log.md, .factory/cycles/cycle-001/adversarial-reviews/pr-357-release-gate-jr-base-url/pr-357-copilot-progress.md
**Versions bumped:** (none)

### Summary

PR #357 Copilot R2 hit the Phase 8 stop condition. Review id 4268805775 posted
2026-05-12T02:52:59Z returned zero inline comments: "Copilot reviewed 4 out of 4 changed
files in this pull request and generated no new comments." Trajectory 3→0. PR #357
CONVERGED. Awaiting human merge approval.

Cycle stats: 2 rounds total, 3 findings in R1 (all resolved), 0 in R2. 2 commits
(cb3e8a3 initial, 144aaff R1 fix). cargo test: 1248 passed (+4 regression tests vs
baseline 1244). 3/3 threads resolved. CI 8/8 green. Mergeable: CLEAN.

This is the fastest convergence in cycle-001 to date (2 rounds vs PR #356's 19). Speed
is attributed to the CRITICAL nature of R1's primary finding: once the two-site gating
gap was fixed with a tightly scoped commit, no residual issues remained.

### Details

| Agent | Task | Output |
|-------|------|--------|
| state-manager | Record R2 stop condition in pr-357-copilot-progress.md + cycle summary | pr-357-copilot-progress.md updated (status: converged) |
| state-manager | Append R2 burst entry to burst-log.md | This entry |
| state-manager | Update STATE.md Phase Progress row + Convergence Tracker + Session Checkpoint | STATE.md updated |

**Outcome:** PR #357 CONVERGED @ 144aaff. Phase 8 stop condition hit (R2: 0 inline comments). Next action: awaiting human merge approval to close #335.

---
document_type: pipeline-state
level: ops
version: "2.0"
status: active
producer: state-manager
timestamp: 2026-05-08T00:00:00
phase: phase-3-tdd-implementation
inputs: []
input-hash: "[live-state]"
traces_to: ""
project: jira-cli
mode: BROWNFIELD
current_step: "phase-3-wave-2-S-2.04-start"
current_cycle: "cycle-001"
dtu_required: false
phase_2_status: APPROVED
phase_2_approved_at: 2026-05-07
phase_2_approved_by: "human (user)"
phase_3_status: IN_PROGRESS
activation_head: "dea166471e22eff55974d7675593469b37048c5f"
activation_version: "v0.5.0-dev.7"
---

<!-- SIZE BUDGET: <200 lines. Historical content → cycle files. Run /vsdd-factory:compact-state if over 200. -->

# Pipeline State: jira-cli

## Project Metadata

| Field | Value |
|-------|-------|
| **Product** | jr (Jira CLI) |
| **Repository** | /Users/zious/Documents/GITHUB/jira-cli |
| **Mode** | BROWNFIELD |
| **Language** | Rust |
| **Target Workspace** | develop → main |
| **Started** | 2026-05-04 |
| **Last Updated** | 2026-05-08 (S-2.03 MERGED via PR #305 at e9c2ba8; Wave 2: 3/7 merged; S-2.04 active) |
| **Current Phase** | Phase 3 — TDD Implementation **IN PROGRESS** (Wave 0 COMPLETE 7/7; Wave 1 COMPLETE 8/8 via PRs #295-#302; Wave 2 ACTIVE — 3/7 merged; active: S-2.04) |
| **Next Phase** | Phase 3 Wave 2 delivery |
| **Activation HEAD** | dea166471e22eff55974d7675593469b37048c5f (v0.5.0-dev.7) |
| **factory-artifacts SHA** | 0b01262 (Phase 1 gate APPROVE; phase-1-converged tag) |

## Pipeline Goal

Goal 1c: **Harden v0.5 + feature delivery** — formalize existing codebase with VSDD specs, holdouts, and verification; AND use VSDD pipeline for all post-v0.5.0 feature work.

## Phase Progress

| Phase | Status | Started | Completed | Gate | Finding Progression |
|-------|--------|---------|-----------|------|---------------------|
| pre-pipeline: Setup | complete | 2026-05-04 | 2026-05-04 | env-preflight | |
| 0: Codebase Ingestion | **COMPLETE** | 2026-05-04 | 2026-05-04 | Phase A + B + B.5 + B.6 + C + gate APPROVED | |
| 1: Spec Crystallization | **COMPLETE** | 2026-05-04 | 2026-05-04 | PASSED — DEC-006 (SD-001=C), DEC-007 (SD-002=A), DEC-008 (SD-003=B), gate APPROVE | |
| 1d: Adversarial Spec Review | **COMPLETE** — **3/3 CONVERGED** at Pass 28 after 28 passes (5 counter resets, 3 consecutive clean P26-P27-P28) | 2026-05-04 | 2026-05-04 | 3/3 FULL CONVERGENCE | 30→15→9→5→10→5→4→3→4→0→2→0→3→0→2→0→3→0→3→5→3→4→5→5→5→2→0→0→0 |
| 1-gate-prep: Consistency Validation + Drift Items | **COMPLETE** | 2026-05-06 | 2026-05-04 | DEC-006/007/008 resolved; ADR-0013 created | CV: 4H/1M; CV-001/003/005 FIXED; CV-002 resolved (SD-001=C/SD-002=A/SD-003=B); CV-004 DRIFT-002 resolved post-SD-002 |
| 2: Story Decomposition | **complete** (story creation phase) | 2026-05-04 | 2026-05-06 | 31 stories created (W0:7 + W1:8 + W2:7 + W3:9); Phase 2-adv pending | |
| 2-adv: Adversarial Story Review | **CONVERGED** — Pass 13 CLEAN-PASS; Counter 3/3 | 2026-05-06 | 2026-05-07 | 3/3 FULL CONVERGENCE | 14→5→5→5→4→5→4→4→4→1→0→1→0 |
| Phase 2 gate | **APPROVED** (2026-05-07) — 31 stories locked, ready for TDD | — | 2026-05-07 | APPROVED by human | — |
| 3: TDD Implementation | **IN_PROGRESS** — Wave 0 COMPLETE (7/7); **WAVE 1 COMPLETE (8/8)** via PRs #295-#302; Wave 2 ACTIVE — 3/7 merged; S-2.04 active | 2026-05-07 | | | Wave 0: COMPLETE — 7/7 via PRs #289-#294 + S-0.07 spec-only. Wave 1: COMPLETE — 8/8 via PRs #295-#302; 0 regressions; ~50 new tests; 5 deferred + 1 PENDING_MANUAL (S-1.05-AC-001 now RESOLVED). Wave 2: 3/7 — S-2.01 MERGED at f6516f8 (PR #303; 7 issue-read regression tests for H-021/H-030-035 across 9 BCs); S-2.02 MERGED at 75289600 (PR #304; 4 issue-write regression tests for H-006/007/008/014 across 4 BCs; 8/8 CI; 1 deferred S-2.02-DEFER: transitioned vs changed field name); S-2.03 MERGED at e9c2ba8 (PR #305; 3 regression-pin tests for H-037/H-038/H-039 across 3 BCs; 8/8 CI; APPROVE 1 cycle; 0 blocking; 0 deferred non-doc; H-038 library-level test placement; 1 LOW deferred S-2.03-DOC-01 cache filename in spec). S-2.04 active. Phase 3 progress: 19/31 (61%). |
| 3-adv: Wave Adversarial Reviews | not-started | | | | |
| 4: Holdout Evaluation | not-started | | | | |
| 5: Adversarial Refinement | not-started | | | | |
| 6: Formal Hardening | not-started | | | | |
| 7: Convergence | not-started | | | | |

## Current Phase Steps

<!-- Keep last 5 rows only. Archive older rows to cycles/cycle-001/burst-log.md. -->

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| S-2.01 MERGED — Issue-read holdout suite | devops-engineer | complete | PR #303 squash-merged to develop at f6516f8 (2026-05-08); 7 regression-pin tests for H-021/H-030-035 across 9 BCs; AC-006 deviation: --open/--status clap conflicts_with; 8/8 CI green; APPROVE 1 cycle; 3 non-blocking; S-002 doc fix 4b0b55d; 0 deferred. Wave 2: 1/7. |
| S-2.02 MERGED — Issue-write holdout suite | devops-engineer | complete | PR #304 merged to develop at 75289600 (2026-05-08); 4 regression-pin tests for H-006/007/008/014 across 4 BCs; 8/8 CI green; review APPROVE 1 cycle (direct gh CLI flow — API hiccup; Lesson 6); 1 deferred (S-2.02-DEFER: transitioned vs changed JSON field). Wave 2: 2/7. S-1.05-AC-001 RESOLVED. |
| S-2.03 MERGED — BC-4 assets/CMDB holdout suite | devops-engineer | complete | PR #305 squash-merged to develop at e9c2ba8 (2026-05-08); 3 regression-pin tests for H-037/H-038/H-039 across 3 BCs; H-038 library-level placement (`enrich_assets` is `pub`); 8/8 CI green; APPROVE 1 cycle; 0 blocking findings; 1 LOW deferred (S-2.03-DOC-01 cache filename mismatch in story spec). Wave 2: 3/7. Phase 3: 19/31 (61%). |
| S-2.04 START — Boards/sprints/ADF holdout suite | orchestrator | active | Wave 2 S-2.04 active (BC-5/7; H-040..H-044). Phase 3 progress: 19/31 (61%). |

## Decisions Log

| ID | Decision | Rationale | Phase | Date | Made By |
|----|----------|-----------|-------|------|---------|
| DEC-001 | Pre-VSDD docs treatment: RESOLVED — HARMONIZE per Q4 (74 specs become BC validation inputs; 1 archaeological excluded; 2 divergent need reconciliation; v1 design imported as historical with annotated supersessions on 3 sections; 75 plans SUPERSEDE) | Q4 harmonization plan confirmed 74 DELIVERED-AS-DESIGNED, 0 PARTIAL/UNDELIVERED. Plans dir cleanly SUPERSEDE. | Phase 0 | 2026-05-04 | human |
| DEC-002 | Pre-VSDD docs at Phase 0→1 gate: RESOLVED — see DEC-001 | Consolidated into DEC-001 outcome | Phase 0 | 2026-05-04 | human |
| DEC-003 | 5 MUST-FIX bugs treatment: PARTIALLY RESOLVED — NFR-R-D has draft BC (14 read sites in 6 files; holdout H-NEW-MP-001 proposed). 4 P0 bugs route to Phase 3 (decompose-stories) for fix-in-phase-3 treatment. | Draft BC ready for Phase 1 PRD formalization. | Phase 0 | 2026-05-04 | orchestrator + human |
| DEC-005 | Phase 1d Adversarial Spec Review converged 3/3 at Pass 28 | 28 total passes (25 SUBSTANTIVE + 3 consecutive CLEAN-PASS). 80+ findings addressed across rotating lens axes. Trajectory shows healthy descent. Spec corpus locked at convergence: 541 BCs, 41 NFRs, 48 holdouts, 26 risks, 12 ADRs, 3 SD. Post-convergence additions: +3 holdouts (H-NEW-VERBOSE-001/002, H-NEW-AUTH-002) → 51 total. | Phase 1d | 2026-05-04 | orchestrator + adversary |
| DEC-006 | SD-001 = Option C — PKCE deferred with ADR-0013 | Atlassian Cloud doesn't publicly support PKCE; Options A/B technically infeasible. Threat model documented with mitigations. Reactivation trigger set. | Phase 1→2 gate | 2026-05-04 | human + perplexity research |
| DEC-007 | SD-002 = Option A (`#[cfg(test)]`) at gate; canonized to Option B-revised (`#[cfg(debug_assertions)]`) during S-0.05 implementation (2026-05-07) | ~151 subprocess tests use `cargo_bin("jr").env("JR_AUTH_HEADER", ...)` — subprocess binary has no cfg(test); `#[cfg(debug_assertions)]` achieves identical release-binary security. See SD-002 canonization. | Phase 1→2 gate / S-0.05 | 2026-05-04 / 2026-05-07 | human + perplexity research / implementer |
| DEC-008 | SD-003 = Option B — header-only `--verbose` default + opt-in `--verbose-bodies` with PII warning | Strongest default security; mitigates AI-agent context capture (EDPB Apr 2025). Breaking change for v0.6. | Phase 1→2 gate | 2026-05-04 | human + perplexity research |
| DEC-009 | Phase 1 → Phase 2 gate APPROVED | All pending decisions resolved (DEC-006/007/008). Spec corpus locked at gate: 541 BCs / 41 NFRs / 48 holdouts / 28 risks / 13 ADRs / 3 SDs. Wave 0 additions brought holdouts to 51 (H-NEW-VERBOSE-001/002 + H-NEW-AUTH-002). | Phase 1→2 gate | 2026-05-04 | human |

## Skip Log

| Step | Skipped? | Justification |
|------|----------|---------------|
| | | |

## Blocking Issues

<!-- Open issues only. Move resolved issues to cycles/cycle-001/blocking-issues-resolved.md. -->

| ID | Issue | Severity | Blocking Phase | Owner | Resolution |
|----|-------|----------|----------------|-------|------------|

## Drift Items

<!-- Populated during Phase 0 codebase ingestion. -->

| ID | Area | Description | Severity | Status |
|----|------|-------------|----------|--------|
| DRIFT-001 | Pass 21+ propagation (recurring) | Count/chain-length fixes require downstream grep sweep. P21 missed H-044+L2; P23-001 reaffirms; ADV-P24-001 is third recurrence. Codify as S-7.01. Every count/chain-length change must trigger grep sweep. | MEDIUM | process-gap recurring (S-3.06 codification story in Wave 3) |
| DRIFT-002 | NFR-S-B holdout gap | **RESOLVED** — SD-002 = Option A; NFR-S-B holdout now definable (S-1.05). | MEDIUM | **RESOLVED** |
| DRIFT-003 | STORY-INDEX → WAVE-PLAN sibling propagation gap | Recurred P1/P2/P3/P4/P7/P8/P9/P12 of Phase 2-adv. Structural pattern. S-3.06 scope should include WAVE-PLAN↔STORY-INDEX↔frontmatter triple-sync verification. | MEDIUM | process-gap (S-3.06 scope expansion needed) |
| DRIFT-004 | STORY-INDEX BC IDs not validated against canonical bc-N-*.md | P6 surfaced BC-6.4.* dangling (since corpus inception). Fix authors must open canonical BC file. | HIGH | process-gap (verify every BC ID against canonical bc-N-*.md) |
| ADV-P2-S12-001 | S-1.08 body line 274 stale dep | **RESOLVED** — 2026-05-07 — body line 274 updated to "No Wave 0 dependencies…" | MEDIUM | **RESOLVED** |
| OBS-13-1 | JiaClient cosmetic typo | **RESOLVED** — 2026-05-07 — global sweep; 0 remaining matches | LOW | **RESOLVED** |
| OBS-13-2 | Story manifest tooling gap | **RESOLVED** — 2026-05-07 — Story Manifest table (31 rows) added to STORY-INDEX v1.4.1 | LOW | **RESOLVED** |
| CV2-001 | STATE.md stale story count | **RESOLVED** — 2026-05-07 — STATE.md line 54 fixed (30→31, W3:8→W3:9) | MEDIUM | **RESOLVED** |
| CV2-002 | STORY-INDEX S-2.04 BC column incomplete | **RESOLVED** — 2026-05-07 — S-2.04 BC column completed (3→7 BCs); v1.4.2 | MEDIUM | **RESOLVED** |
| CV2-003 | SD-003 holdout gap | **RESOLVED** — 2026-05-07 — H-NEW-VERBOSE-001/002 registered; WAVE-PLAN updated (v1.1.1); S-0.06 cross-link added | MEDIUM | **RESOLVED** |
| R1-001 | JiraClient::new_for_test_with_instance_url ergonomics | DEFERRED — 2026-05-07 — takes (base_url, instance_url) where one concept might suffice. Test-infra only; no correctness impact. Target: bundle into next workflow.rs/client.rs touch (likely Wave 3 cleanup or absorbed by S-0.05/0.06). | LOW | DEFERRED |
| R1-002 | Stale doc comment in workflow.rs handle_open | DEFERRED — 2026-05-07 — referenced "base URL" pre-fix; one-line text fix. Target: assign to next implementer touching workflow.rs. | LOW | DEFERRED |
| S-0.03-S1 | Missing integration test for effective_wid fallback path at list.rs:464-470 | DEFERRED — 2026-05-07 — raw_wid empty → fallback_wid lookup branch has no integration test. Logic correct; coverage gap. Target: bundle into next list.rs/CMDB-related touch (likely Wave 2 issue-list test suite expansion S-2.01/S-2.02) OR add as a follow-up small story in the CMDB epic. | LOW | DEFERRED |
| S-0.05-DEV | SD-002 doc-vs-code drift (gate canonization) | **RESOLVED** — 2026-05-07 — SD-002 canonized to Option B-revised (`#[cfg(debug_assertions)]`) during S-0.05 implementation. 151-subprocess-test compatibility preserved. Threat model mitigation equivalent to Option A original. Doc updates: SD-002.md (Resolution, Options, Decision Log, version 1.0.1) + S-0.05 (Context, BC, ACs, Implementation Notes, Compliance Rules) + S-0.07 (Context, AC-004, holdout spec SD field) + STATE.md (DEC-007, Current Phase Steps). | MEDIUM | **RESOLVED** |
| S-0.05-F1 | Cosmetic typo "JiaClient" → "JiraClient" in test doc comment line 153 of tests/auth_header_release_gate.rs | DEFERRED — 2026-05-07 — Target: bundle into next test-doc cleanup or absorbed by S-0.07 since that story authors related holdouts. | LOW | DEFERRED |
| S-0.05-F2 | Stale doc comment in renamed test (likely fixed in clippy-fix commit c82832c) | TO_VERIFY — 2026-05-07 — Verify in next read; close if resolved. | LOW | TO_VERIFY |
| S-0.05-F3 | load_auth_from_keychain could comment that _refresh token is intentionally discarded (matches pre-existing behavior) | DEFERRED — 2026-05-07 — Cosmetic doc improvement. Target: next src/api/client.rs touch. | LOW | DEFERRED |
| S-1.02-DEFER | 12 of 22 [[bans.skip]] entries are upstream-blocked: figment toml 1.x adoption (8 skips for serde_spanned/toml/toml_datetime/winnow), jni thiserror 2.x + windows-sys 0.6x adoption (4 skips). Target: create follow-up dedupe-tracking story (S-2.NN or Wave 3 cleanup) with AC "prune skip entries when upstream blockers ship; cargo deny check still exits 0". Recheck quarterly via Dependabot PRs. | LOW | DEFERRED — 2026-05-07 |
| S-1.03-DEFER | Body logging events in src/api/client.rs retain [verbose] prefix eprintln! (not tracing::trace!) to preserve SD-003 contract verified by 6 verbose_bodies regression tests. Target: Wave 2 cleanup story to renegotiate the body-logging contract holistically (update SD-003 test expectations + migrate to tracing::trace! at TRACE level + add explicit body redaction layer if needed). | LOW | DEFERRED — 2026-05-07 |
| S-1.04-DEFER-01 | No fail-fast: false on test matrix; if a platform fails, other legs cancel before timeout fires. Pre-existing behavior. Target: separate matrix-strategy story if cross-platform flakiness becomes an issue. | LOW | DEFERRED — 2026-05-07 |
| S-1.04-DEFER-02 | coverage timeout (30m) matches test (30m); coverage runs instrumented build (~2-3x slower). Today fine (~2m observed); revisit if codebase grows significantly. | LOW | DEFERRED — 2026-05-07 |
| S-1.04-DEFER-03 | release.yml uses job-level timeout-minutes only; no step-level timeouts on long-running steps (LTO link, cross-compile install). Revisit if a single step hangs inside the job window. | LOW | DEFERRED — 2026-05-07 |
| S-1.05-DEFER-01 | gitleaks-action ff98106e (v2.3.9) runs on Node.js 20; GitHub forces Node.js 24 starting June 2, 2026. Action upgrade required before that date. Target: Wave 2 maintenance sweep, OR Dependabot weekly PR pickup. | MEDIUM | DEFERRED — 2026-05-08 |
| S-1.05-AC-001 | Repo-level GitHub Secret Scanning: user enabled secret_scanning + push_protection on Zious11/jira-cli via `gh api PATCH security_and_analysis` (2026-05-08). Verified via `gh api repos/Zious11/jira-cli --jq '.security_and_analysis'` showing both enabled. CI gitleaks job + GitHub native scanner now both active for layered defense. | HIGH | **RESOLVED** — 2026-05-08 |
| S-2.02-DEFER | JSON field-name reconciliation: BC-3.2.001 spec language uses `"transitioned": false` for move_response output, but actual code emits `"changed"`. Test pinned to actual implementation. Target: update L3 PRD spec text OR rename field via separate PR (consider deprecation cycle if field is documented in user-facing CLAUDE.md or release notes). | LOW | DEFERRED — 2026-05-08 |
| S-2.03-DOC-01 | Story spec text | Story spec line ~123 names workspace cache file `workspace_id.json` but actual filename per `src/cache.rs` and tests is `workspace.json`. Tests use correct filename. | LOW | DEFERRED — 2026-05-08 |

## Convergence Trackers

### Phase 1d — Adversarial Spec Review
_**3/3 FULLY CONVERGED** at Pass 28 (2026-05-04). 28 passes total: 25 SUBSTANTIVE + 3 consecutive CLEAN-PASS (P26-P27-P28). 5 counter resets. ~80+ findings addressed. Final trajectory: 30→15→9→5→10→5→4→3→4→0→2→0→3→0→2→0→3→0→3→5→3→4→5→5→5→2→0→0→0. Spec corpus at convergence: 541 BCs, 41 NFRs, 48 holdouts, 26 risks, 13 ADRs, 3 SDs. Phase 1 → Phase 2 gate APPROVED (DEC-009, 2026-05-04). Full per-pass details: `cycles/cycle-001/convergence-trajectory.md`._

### Phase 2-adv — Adversarial Story Review
_**3/3 FULLY CONVERGED** at Pass 13 (2026-05-07). 13 passes: 10 SUBSTANTIVE + 3 consecutive CLEAN-PASS (P11-P12-P13). Trajectory: 14→5→5→5→4→5→4→4→4→1→0→1→0. Full per-pass details: `cycles/cycle-001/convergence-trajectory.md`._

### Phase 3-adv — Wave Adversarial Reviews (per-story + wave)
_Not started._

### Phase 5-adv — Adversarial Refinement
_Not started._

## Session Resume Checkpoint

<!-- Keep ONLY the latest checkpoint. Archive prior checkpoints to cycles/cycle-001/session-checkpoints.md. -->

| Field | Value |
|-------|-------|
| **Date** | 2026-05-08 |
| **Position** | S-2.03 merged (PR #305 at e9c2ba8). Wave 2 progress: 3/7. Active story: S-2.04 (BC-5/7 boards/sprints/ADF holdout suite). Total Phase 3 progress: 19/31 (61%). Open deferred: R1-001, R1-002, S-0.03-S1, S-0.05-F1, S-0.05-F2 (TO_VERIFY), S-0.05-F3, S-1.02-DEFER, S-1.03-DEFER, S-1.04-DEFER-01/02/03, S-1.05-DEFER-01 (Node.js 24 Jun 2026), S-2.02-DEFER (transitioned vs changed field), S-2.03-DOC-01 (cache filename mismatch in story spec). |
| **Convergence counter** | 3/3 CONVERGED (Phase 2-adv; Pass 13 CLEAN-PASS — final trajectory: 14→5→5→5→4→5→4→4→4→1→0→1→0) |

## Historical Content

| Content | Location |
|---------|----------|
| Burst history | `cycles/cycle-001/burst-log.md` |
| Convergence trajectory (full per-pass) | `cycles/cycle-001/convergence-trajectory.md` |
| Session checkpoints | `cycles/cycle-001/session-checkpoints.md` |
| Lessons learned | `cycles/cycle-001/lessons.md` |
| Resolved blockers | `cycles/cycle-001/blocking-issues-resolved.md` |
| Phase 2→3 gate document | `cycles/cycle-001/gates/phase-2-to-3-gate.md` |

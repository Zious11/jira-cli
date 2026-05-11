---
document_type: pipeline-state
level: ops
version: "2.0"
status: active
producer: state-manager
timestamp: 2026-05-11T13:00:00
phase: phase-3-tdd-implementation
inputs: []
input-hash: "[live-state]"
traces_to: ""
project: jira-cli
mode: BROWNFIELD
current_step: "phase-3-wave-2-gate-CLOSED"
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
| **Last Updated** | 2026-05-11 — PR #353 CONVERGED Round 1 (0 inline comments; Perplexity post-hoc confirms Atlassian bulk cap-equivalence at 1000). Awaiting human merge. 12 audit-followups after #338 closes: #331, #332, #333, #334, #335, #336, #340, #342, #343, #345, #346, #350. |
| **Current Phase** | Phase 3 — TDD Implementation **IN PROGRESS** — Wave 3 CLOSED (10/10). Feature Mode #110-pr2 COMPLETE. PR #353 CONVERGED (refactor/bulk-max-keys-338 @ 3b98a3d; closes #338; awaiting human merge). |
| **Next Phase** | Wave 3 — 10 stories (S-3.01..S-3.10) |
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
| 3: TDD Implementation | **IN_PROGRESS** — Wave 0 COMPLETE (7/7); **WAVE 1 COMPLETE (8/8)** via PRs #295-#302; **Wave 2 COMPLETE (7/7)** via PRs #303-#309 + Wave 2 integration gate CLOSED 2026-05-08; **Wave 3: 10/10 COMPLETE** (S-3.10 + S-3.06 + S-3.07 + S-3.05 MERGED; S-3.09 + S-3.08 closed; S-3.02 MERGED; S-3.01 MERGED; S-3.04 MERGED; S-3.03 v2 MERGED PR #321 / 597dd23) | 2026-05-07 | | | Wave 0: COMPLETE — 7/7 via PRs #289-#294 + S-0.07 spec-only. Wave 1: COMPLETE — 8/8 via PRs #295-#302; 0 regressions; ~50 new tests. Wave 2: COMPLETE 7/7 — S-2.01..S-2.07 MERGED (PRs #303-#309). Wave 2 integration gate CLOSED 2026-05-08. Wave 3: 10/10 COMPLETE — S-3.10 (PR #313 / f492e59), S-3.06 (PR #314 / 01ba293), S-3.07 (PR #315 / 6bce18c), S-3.05 (PR #316 / 10e1db4), S-3.09 (factory-artifacts direct commit — doc-only facade), S-3.08 (PR #317 / fba47ad), S-3.02 (PR #318 / 68092af), S-3.01 (PR #319 / b20cfee), S-3.04 (PR #320 / b6ab77c), S-3.03 v2 (PR #321 / 597dd23). Phase 3 progress: **32/32 (100% v2 scope)** — Wave 3 CLOSED. All 10 Wave 3 stories delivered. S-3.03 v2 added as larger sibling after DEC-013 pivot; +1 WV2-SEC-01 fix (PR #310); total 33-story backlog. |
| 3-adv: Wave Adversarial Reviews | **WAVE 2 GATE CLOSED 2026-05-08** | 2026-05-08 | 2026-05-08 | GATE-PASSES (consistency pass-02 `8ae5511`) | adversary pass-01 (12 findings: 3 BLOCKING + 5 CONCERN + 4 NIT); code-reviewer (0 critical/high); security-reviewer (LOW-RISK; 1 MEDIUM resolved); consistency pass-01 (12 findings: 1 BLOCKING + 7 DRIFT + 4 NIT) → 4 fix-PRs (Fix-PR A spec-anchor `28b0f35`; Fix-PR B nfr-catalog `7fd17bf`; WV2-SEC-01 PR #310 `6cb9994`; pass-02 consistency `8ae5511`) → consistency pass-02 verdict GATE-PASSES. 3 minor drift items addressed inline in this commit (P2-CV-01/02/03 count-rollups). 2 deferred follow-ups for develop-side test docstring re-anchoring (WV2-FIX-A-FOLLOWUP-01/02). Wave 3 cleared for start. |
| 3-feature-#110-pr2 | **COMPLETE** — PR #348 MERGED 2026-05-11 @ e480ff2; closes #110 | 2026-05-10 | 2026-05-11 | F5 CONVERGED + F6 PASS + F7 PASS-WITH-FOLLOWUPS | 12→5→0→0→0 |
| 3-feature-test-hygiene | **MERGED** — PR #351 squash-merged @ 3216ec2 (2026-05-11T15:15:10Z); closes #339+#344; issue #347 deferred to PR #352 | 2026-05-11 | 2026-05-11 | MERGED | 2→1→0 / rebase / 0 |
| 3-feature-docs-cleanup | **MERGED** — PR #352 squash-merged @ 57cc0ae (2026-05-11); closes #337+#341+#347 | 2026-05-11 | 2026-05-11 | MERGED | 3→0 |
| 3-feature-bulk-max-keys-338 | **CONVERGED, awaiting human merge** — PR #353 OPEN (refactor/bulk-max-keys-338 @ 3b98a3d; closes #338); Round 1: 0 inline comments (overview only); Perplexity post-hoc confirms both Atlassian bulk endpoints share 1000 per-call cap | 2026-05-11 | | CONVERGED Round 1 | 0 (1-round clean) |
| 4: Holdout Evaluation | not-started | | | | |
| 5: Adversarial Refinement | not-started | | | | |
| 6: Formal Hardening | not-started | | | | |
| 7: Convergence | not-started | | | | |

## Current Phase Steps

<!-- Keep last 5 rows only. Archive older rows to cycles/cycle-001/burst-log.md. -->

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| v0.5.0-dev.9 bump — Wave 3 closure dev-release marker | state-manager | complete | PR #322 squash-merged to develop at 811fbc7 (2026-05-09). 2-line bump only. 8/8 CI green. |
| PR #351 MERGED — test hygiene (gate test + BulkOperationProgress unit table) | pr-manager | complete | PR #351 squash-merged to develop at 3216ec2 (2026-05-11T15:15:10Z). Closes #339 + #344. Issue #347 deferred to PR #352. Develop: e480ff2→3216ec2. Worktree `.worktrees/test-hygiene` removed; branch deleted. |
| PR #352 MERGED — docs cleanup (CLAUDE.md + mod.rs + bulk test comment) | pr-manager | complete | PR #352 squash-merged to develop at 57cc0ae (2026-05-11T15:36:10Z). Closes #337+#341+#347. Convergence: 3→0 (R1 fix f42bfa5; R2 clean). |
| PR #353 OPENED — consolidate BULK_MAX_KEYS (#338) | orchestrator | complete | PR #353 opened (refactor/bulk-max-keys-338 @ 3b98a3d). Trivial-changes path. pub const BULK_MAX_KEYS canonical in src/api/jira/bulk.rs; create.rs + workflow.rs import it. +14/-9 lines. CI 8/8 green (3b98a3d). Copilot Round 1: 0 inline comments. Perplexity post-hoc: both Atlassian bulk endpoints share 1000 per-call cap (confirmed). PR CONVERGED, awaiting human merge. |

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
| DEC-010 | S-2.06 spec pivot to timeSpent string passthrough (Option 1) | Perplexity verification on 2026-05-08 found v1.0.0 spec had wrong endpoint (/configuration/timetracking returns provider, not hours/days), wrong field names (workingHoursPerDay/workingDaysPerWeek not hoursPerDay/daysPerWeek), wrong types (f64 not u32), and wrong auth assumption (admin-only endpoint). User chose Option 1 (string passthrough — matches ankitpokhrel/jira-cli pattern; eliminates admin endpoint and cache entirely). v2.0.0 spec at .factory/stories/wave-2/S-2.06-... committed at factory-artifacts 37a4be6. Verification report at .factory/research/S-2.06-jira-timetracking-verification.md. | Phase 3 / Wave 2 | 2026-05-08 | human + research-agent (Perplexity) |
| DEC-011 | S-2.07 spec pivot to v2.0.0 (Option A: apply 3 corrections) | Perplexity-driven verification on 2026-05-08 found 3 concrete errors in v1: (a) AC-002 wiremock premise structurally untestable — `jr auth refresh` re-runs full OAuth 3LO flow via login_oauth, never hits a refresh-token API; (b) NFR-O-F's prescribed `{profile, action, ok}` shape conflicted with already-shipped `refresh_success_payload` shape `{status, auth_method, next_step}` — v2 keeps both with documented asymmetry (refresh triggers re-auth, not state mutation); (c) AC-005 `transitioned` vs `changed` ambiguity resolved as `changed` (verified at src/cli/issue/json_output.rs:4-10) — also closes S-2.02-DEFER. AC-006 reframed to extend the existing 11-test insta snapshot suite at src/cli/issue/json_output.rs:84-149. Verification report at .factory/research/S-2.07-json-policy-and-conventions-research.md. | Phase 3 / Wave 2 | 2026-05-08 | human + research-agent (Perplexity + WebSearch + WebFetch) |
| DEC-012 | BC-7.3.004 mis-anchor repair: Option A (4 new sub-BCs) | Per .factory/research/wave-2-gate-decisions-research.md, Option A re-anchors S-2.07 ACs to BC-7.1.001 + creates 4 new sub-BCs (BC-7.4.013-016) for the auth JSON shapes. Justification: BC-7.4 already houses 12 per-shape JSON pins; Google AIP-162 prefers extending topical sections over inventing new top-level IDs; per-shape pins have lower future-churn risk than one shared abstract contract. Develop-side test docstring re-anchoring deferred to a future touch. | Phase 3 Wave 2 gate | 2026-05-08 | human (final say) + research-agent |
| DEC-013 | S-3.03 spec pivot to v2.0.0 (Option A-fixed: actually wire auto-refresh on 401 with per-profile single-flight coordination, default ON, no config flag) | Perplexity-driven verification on 2026-05-08 found 2 defects in v1's Option A: (a) `{"code": "EXPIRED"}` trigger condition does not exist in Jira REST v3 (real shape is `{"errorMessages": [...]}` with no machine-readable code field and no RFC-6750-compliant WWW-Authenticate header) — v2 uses blanket-401 trigger matching `gh` CLI pattern; (b) v1 omits per-profile single-flight refresh, which would cause concurrent refresh attempts to race against Atlassian's enforced rotation and fail with `invalid_grant` — v2 adds `OnceLock<std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<RefreshState>>>>>` per-profile coordination via new `src/api/refresh_coordinator.rs` (no DashMap dep). v2-design pre-flight verification added 6 refinements: drop 10-minute reuse window claim (Atlassian does not document it; treat refresh as strictly single-use), mandate tokio::sync::Mutex for inner mutex (no poisoning), add R-NEW-AR-4 inter-process race + post-hoc reconcile mitigation (re-read keychain on invalid_grant; if refresh_token changed, retry with new access_token), add R-NEW-AR-5 persist-failure wedge + persist-before-publish ordering, drop atlassian-mcp-server citation (source-level patterns not verified), cite gh CLI alone for blanket-401 precedent (aws-cli + gcloud use proactive expiry-time refresh, different pattern). No `--auto-refresh` config flag — phased-rollout strategy (default-ON; reactive flag only on concrete user demand). Effort grows MEDIUM → LARGE; ACs grow 5 → 11; risks grow 0 → 5 (R-NEW-AR-1..5 added to register). Verification reports: `.factory/research/S-3.03-wave3-verification.md` (v1 defects) + `.factory/research/S-3.03-v2-design-verification.md` (v2 design refinements). Story rewrite: `.factory/stories/wave-3/S-3.03-auto-refresh-oauth-on-401-with-single-flight.md` (renamed from `-investigation.md`). Develop-side CLAUDE.md gotchas (verified 401 shape + single-use refresh rule + mutex layering rule) on PR #312 (`docs/s-3-03-claude-md-gotchas` worktree branch). | Phase 3 / Wave 3 | 2026-05-08 | human + research-agent (Perplexity + Context7 + WebFetch) |
| DEC-015 | Use Feature Mode F1-F7 for post-Wave-3 issue work (issue #110 PR2 pioneer) | Provides TDD discipline + adversarial convergence + full audit trail for feature work outside the main wave cadence. PR2 (#348) is the first Feature Mode delivery in this project. | Phase 3 post-Wave-3 | 2026-05-10 | orchestrator + human |
| DEC-017 | Bundle audit-followups via per-theme PRs post-#348 merge | PR #351 (test hygiene) covers #339 + #344 + likely #347. Future bundles per theme: docs, refactors, security. Avoids 18 micro-PRs for low-severity follow-ups. | Phase 3 post-#348 | 2026-05-11 | orchestrator + human |
| DEC-016 | Defer empirical Atlassian Bulk API schema verification to sandbox-required follow-up #331; ship best-guess shapes with loose test matchers + SCHEMA NOTES + PR-description disclaimer | No sandbox access during development; loose matchers + deferred-pending-sandbox pattern is acceptable and documented. Codified in lessons.md. | Phase 3 / #110-pr2 | 2026-05-10 | human + orchestrator |
| DEC-018 | Always validate Copilot review findings with Perplexity (or Context7 for library-specific claims) BEFORE acting on them. Established 2026-05-11 per user standing rule; codified in lessons.md. Post-hoc validated PR #351 rounds 1-2 (all 3 Copilot claims confirmed correct via Perplexity). Post-rebase re-request on b38c018 returned 0 new comments, reinforcing that Perplexity-validation kept fix quality high through 3 rounds AND a force-push. Prior counterexample: PR #348 round-2 C1 (compile-error claim was wrong; CI was green). | Universal Copilot review discipline. Avoids wasted or wrong fixes when Copilot's training data is stale on external library/API behavior. Pattern holds across rebase cycles. | Phase 3 ongoing | 2026-05-11 | human (standing rule) |
| DEC-014 | S-3.07 spec pivot to v2.0.0 (3 corrections: Part A reframe + Part B conditional drop + Part D elevation as confirmed JRACLOUD-94632 bug response) | Perplexity-driven verification on 2026-05-08 found 3 errors in v1: (a) Atlassian Retry-After typical values are 1425-3089 seconds with documented 3600s ceiling, NOT the 86400s extreme used as v1's threat framing — `MAX_RETRY_AFTER_SECS=60` aborts essentially every real-world 429 (still defensible for interactive CLI per RFC 9110 §10.2.3, but story rationale + user error message must reflect reality); (b) Part B's `checked_mul` overflow guard targets the 3-arg `parse_duration` calculator that S-3.10 deletes — the orchestrator's earlier "WV2-SEC-01's 64-byte cap eliminates overflow" reasoning was mathematically false (14-20 digit inputs still overflow u64 within 64 bytes) — correct reason to drop is that S-3.10 deletes the function; drop is conditional via `depends_on: [S-3.10]` + AC-NEW-B sequencing gate, with reinstatement plan if S-3.10 slips; (c) Part D's `/rest/api/3/search/jql` cursor-loop is NOT a defensive nice-to-have — it is a confirmed Jira Cloud bug per JRACLOUD-94632 + JRACLOUD-92049 + JRACLOUD-85546 (also reported in atlassian/atlassian-mcp-server#118 and ankitpokhrel/jira-cli#898) → v2 elevates from KNOWN-GAP source comment to real defensive guard + stderr warning containing literal "JRACLOUD-94632" so users have a copy-pasteable upstream search term. ACs change: drop AC-004/005 (Part B specific); add AC-NEW-B sequencing guard; add AC-NEW-D JRACLOUD content assertion. New risk: R-NEW-S307-1 (silent partial results — failure mode now visible). NFR catalog: NFR-R-NEW-2 row removed (Part B dropped → no longer in scope); NFR-R-F routing flipped from DOCUMENT-AS-IS to DOCUMENT-AS-IS-FIXED (real guard delivered, not just documented). Verification report: `.factory/research/S-3.07-wave3-verification.md`. Story rewrite: `.factory/stories/wave-3/S-3.07-low-nfr-code-fixes-and-search-jql-anti-loop.md` (renamed from `-low-nfr-code-cleanup.md`) at factory-artifacts@898937e. No develop-branch impact. | Phase 3 / Wave 3 | 2026-05-08 | human + research-agent (Perplexity + WebFetch) |

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
| S-2.02-DEFER | JSON field-name reconciliation: BC-3.2.001 spec language uses `"transitioned": false` for move_response output, but actual code emits `"changed"`. Test pinned to actual implementation. Target: update L3 PRD spec text OR rename field via separate PR (consider deprecation cycle if field is documented in user-facing CLAUDE.md or release notes). | LOW | **RESOLVED** — 2026-05-08 — verified canonical field name is `changed` per src/cli/issue/json_output.rs:4-10; documented in S-2.07 v2.0.0 AC-005 and DEC-011; holdout-scenarios.md:84 corrected to `"changed": false` in same factory-artifacts commit |
| S-2.03-DOC-01 | Story spec text | Story spec line ~123 names workspace cache file `workspace_id.json` but actual filename per `src/cache.rs` and tests is `workspace.json`. Tests use correct filename. | LOW | DEFERRED — 2026-05-08 |
| S-2.04-DEFER-01 | Story spec text | Story spec AC-004 quotes kanban literal as prefix only ('Sprint commands are only available for scrum boards'); production code at src/cli/sprint.rs:80-85 emits prefix + suffix '. Board {id} is a {type} board.'. Test uses contains(prefix). Update story spec text in follow-up doc PR. | LOW | DEFERRED — 2026-05-08 |
| S-2.04-DEFER-02 | Story spec text | Story spec H-043 implementation notes use 'displayName' for team-cache JSON shape; actual jr::cache::CachedTeam struct uses 'name'. Test uses production struct directly. Update story spec text in follow-up doc PR. | LOW | DEFERRED — 2026-05-08 |
| S-2.04-DOC-01 | tests/team_column_parity.rs (pre-existing) | write_team_cache writes to $XDG_CACHE_HOME/jr/teams.json (missing v1/default/). Canonical path per src/cache.rs:90-92 is $XDG_CACHE_HOME/jr/v1/default/teams.json. Existing tests pass coincidentally. Pre-existing; not introduced by S-2.04. Target: separate fix story. | LOW | DEFERRED — 2026-05-08 |
| S-2.05-DEFER-01 | CLAUDE.md text (pre-existing) | list.rs description reads 'list + view + comments' but view.rs and comments.rs are now separately documented sibling modules after S-2.05. Pre-existing text; out of scope for S-2.05. Target: bundle into next small CLAUDE.md cleanup PR. | LOW | DEFERRED — 2026-05-08 |
| S-2.06-DEFER-01 | src/duration.rs | ~~parse_duration calculator preserved with SUPERSEDED-BY comment because format_duration round-trip proptest still uses it. If format_duration is later removed/refactored, the calculator can be deleted.~~ | LOW | **RESOLVED — 2026-05-08 — H-018 replaced in place (Option 2) per research-agent recommendation; follow-up Option 4 story queued in Wave 3 as S-3.10 to delete the deprecated calculator. See `.factory/research/H-018-holdout-strategy-research.md`.** |
| S-2.06-DEFER-02 | tests/worklog_duration_holdouts.rs | AC-003 stderr OR-chain assertion is lenient (passes on any one of Nw/Nd/Nh/Nm). Could be tightened to require all four substrings. | LOW | DEFERRED — 2026-05-08 |
| S-2.06-DEFER-03 | src/duration.rs:65 | !found_any guard reachability is constrained by prior guards — logically sound but slightly defensive. No action needed. | LOW | DEFERRED — 2026-05-08 |
| S-2.07-DEFER-01 | src/main.rs (existing global error wrapper) | AC-003 (auth subcommand JSON error path) was already satisfied by main.rs's existing --output json error wrapper — propagated JrError values get `{"error","code"}` to stderr. Documented in docs/specs/json-output-shapes.md as already-working. No action needed. | LOW | DEFERRED — 2026-05-08 |
| S-2.07-DEFER-02 | src/cli/auth.rs::mod tests | Pre-existing refresh_payload_pins_token_shape and refresh_payload_pins_oauth_shape tests cover much of AC-002's ground. New tests test_refresh_success_payload_emits_status_refreshed_for_token_flow and _for_oauth_flow are intentionally additive (more specific assertions). No action; intentional overlap. | LOW | DEFERRED — 2026-05-08 |
| WV2-FIX-A-FOLLOWUP-01 | tests/auth_output_json.rs + src/cli/auth.rs::mod tests | 11 test docstrings cite BC-7.3.004 — needs develop-side fix-PR to re-anchor to BC-7.4.013-016 / BC-7.3.005. SHA range src/cli/auth.rs:2126,2156,2198,2210,2222,2234 + tests/auth_output_json.rs:99,141,184,235,298. | LOW | DEFERRED — bundle into next develop touch (e.g., S-3.11 if created, or fold into a Wave 3 doc-cleanup PR). |
| WV2-FIX-A-FOLLOWUP-02 | tests/worklog_duration_holdouts.rs:467,524 | 2 test names embed bc_6_2_013 in their function name — needs develop-side rename to bc_6_2_006. Test names are not load-bearing for runtime; load-bearing for traceability searches. | LOW | DEFERRED — bundle into next develop touch. |
| WV2-ADV-01 | S-2.07 spec + 11 test docstrings | BC-7.3.004 semantic mis-anchor in S-2.07 spec. Story spec re-anchored to BC-7.1.001 + BC-7.4.013-016 (Fix-PR A). Develop-side test docstring re-anchoring deferred as WV2-FIX-A-FOLLOWUP-01. | BLOCKING | **RESOLVED — 2026-05-08 — Fix-PR A** (spec portion resolved; test docstrings deferred as FOLLOWUP-01) |
| WV2-ADV-03 | S-2.06 spec + 2 holdout test names | BC-6.2.013 mis-anchor in S-2.06 Part B. Story spec re-anchored to BC-6.2.006 (Fix-PR A). Develop-side test name rename deferred as WV2-FIX-A-FOLLOWUP-02. | BLOCKING | **RESOLVED — 2026-05-08 — Fix-PR A** (spec portion resolved; test function names deferred as FOLLOWUP-02) |
| WV2-CV-01 | .factory/specs/prd/cross-cutting.md:316 | BC-X.5.005 H1 heading named deprecated 3-arg calculator. Updated to reflect post-S-2.06 dual-function situation (validator is production path). | BLOCKING | **RESOLVED — 2026-05-08 — Fix-PR A** |
| WV2-CV-02 | .factory/stories/WAVE-PLAN.md | Wave 2 status was ACTIVE/draft; Wave 3 showed 9 stories without S-3.10; S-2.06→S-3.10 dependency missing. | DRIFT | **RESOLVED — 2026-05-08 — Fix-PR A** |
| WV2-CV-03 | .factory/stories/STORY-INDEX.md | Wave 0/1 rows (15 stories, S-0.01..S-1.08) still show `draft` status. These waves are fully merged; status is a display-only cosmetic gap. | DRIFT | DEFERRED — Wave 3 doc-cleanup PR or S-3.06 sweep. |
| WV2-CV-05 | .factory/STATE.md | Phase 3 progress count audit: Wave 0 (7) + Wave 1 (8) + Wave 2 (7) = 22. STATE.md previously claimed 23/31 (74%). Off-by-one confirmed; corrected to **22/31 (71%)** in this commit. | DRIFT | **RESOLVED — 2026-05-08 — Wave 2 gate-close commit** (count corrected throughout STATE.md) |
| WV2-CV-07 | .factory/stories/STORY-INDEX.md + STATE.md | S-2.02 SHA typo 75289600 → 7528960 in STATE.md. | DRIFT | **RESOLVED — 2026-05-08 — Fix-PR A** |
| WV2-CV-11 | .factory/specs/prd/holdout-scenarios.md:195 | H-018 BC field has `(post-S-2.06 v2.0.0)` non-standard annotation. | NIT | DEFERRED — bundle into S-3.10 delivery or Wave 3 doc-cleanup. |
| WV2-CV-12 | .factory/STATE.md | S-0.05-F2 drift item shows `TO_VERIFY` without resolution target. | NIT | DEFERRED — verify and close in Wave 3 dev touch or doc-cleanup PR. |
| WV2-SEC-01 | src/duration.rs::parse_duration_validate | Wave 2 integration-gate security finding (CWE-400 uncontrolled resource consumption). parse_duration_validate reflected unbounded user input into error messages. Added MAX_DURATION_INPUT_LEN = 64 byte cap + 2 regression-pin tests. Not exploitable; defense-in-depth. | MEDIUM | **RESOLVED — 2026-05-08 — develop @ 6cb9994 (PR #310)** |

## Convergence Trackers

### Phase 1d — Adversarial Spec Review
_**3/3 FULLY CONVERGED** at Pass 28 (2026-05-04). 28 passes total: 25 SUBSTANTIVE + 3 consecutive CLEAN-PASS (P26-P27-P28). 5 counter resets. ~80+ findings addressed. Final trajectory: 30→15→9→5→10→5→4→3→4→0→2→0→3→0→2→0→3→0→3→5→3→4→5→5→5→2→0→0→0. Spec corpus at convergence: 541 BCs, 41 NFRs, 48 holdouts, 26 risks, 13 ADRs, 3 SDs. Phase 1 → Phase 2 gate APPROVED (DEC-009, 2026-05-04). Full per-pass details: `cycles/cycle-001/convergence-trajectory.md`._

### Phase 2-adv — Adversarial Story Review
_**3/3 FULLY CONVERGED** at Pass 13 (2026-05-07). 13 passes: 10 SUBSTANTIVE + 3 consecutive CLEAN-PASS (P11-P12-P13). Trajectory: 14→5→5→5→4→5→4→4→4→1→0→1→0. Full per-pass details: `cycles/cycle-001/convergence-trajectory.md`._

### Phase 3-adv — Wave Adversarial Reviews (per-story + wave)
_Wave gate: not started. Feature Mode #110-pr2: **F5 CONVERGED** 12→5→0→0→0 (Pass 5, 2026-05-10). F6: SECURITY PASS (→#334). F7: PASS-WITH-FOLLOWUPS (5/5; →#347). 10 Copilot rounds: 27/27 resolved. PR #348 MERGED 2026-05-11 @ e480ff2 (closes #110). **PR #351 MERGED 2026-05-11 @ 3216ec2** (closes #339+#344). **PR #352 MERGED 2026-05-11 @ 57cc0ae** (closes #337+#341+#347; R2 clean 3→0). **PR #353 CONVERGED Round 1** (refactor/bulk-max-keys-338 @ 3b98a3d; closes #338; 0 inline comments; Perplexity post-hoc confirms cap-equivalence; awaiting human merge). 12 audit-followups remain after #338 closes: #331, #332, #333, #334, #335, #336, #340, #342, #343, #345, #346, #350. Full records: `cycles/cycle-001/adversarial-reviews/issue-110-pr2/` + `cycles/cycle-001/adversarial-reviews/pr-352-docs-cleanup/` + `cycles/cycle-001/adversarial-reviews/pr-353-bulk-max-keys/`._

### Phase 5-adv — Adversarial Refinement
_Not started._

## Session Resume Checkpoint

<!-- Keep ONLY the latest checkpoint. Archive prior checkpoints to cycles/cycle-001/session-checkpoints.md. -->
| Field | Value |
|-------|-------|
| **Date** | 2026-05-11 |
| **Position** | **PR #353 CONVERGED Round 1** (refactor/bulk-max-keys-338 @ 3b98a3d; closes #338; mechanical DRY refactor). CI 8/8 green on 3b98a3d. Copilot Round 1: 0 inline comments (overview only; stop condition met). Premise post-hoc Perplexity-validated: Atlassian docs confirm both bulk endpoints share 1000 per-call cap (cited: developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/ + bulk-operation-additional-examples-and-faqs/). Consolidation is correct; no regression. **Next action: await human merge of PR #353 (closes #338). Then: 12 audit-followups — #331, #332, #333, #334, #335, #336, #340, #342, #343, #345, #346, #350.** |
| **Convergence counter** | 3/3 CONVERGED Phase 2-adv; Phase 3-adv: Wave 2 gate CLOSED; Feature Mode #110-pr2 F5 CONVERGED (12→5→0→0→0); PR #351 MERGED (2→1→0 / rebase / 0); PR #352 MERGED (3→0); PR #353 CONVERGED Round 1 (0 inline comments; Perplexity-validated) |

## Historical Content

| Content | Location |
|---------|----------|
| Burst history | `cycles/cycle-001/burst-log.md` |
| Convergence trajectory (full per-pass) | `cycles/cycle-001/convergence-trajectory.md` |
| Session checkpoints | `cycles/cycle-001/session-checkpoints.md` |
| Lessons learned | `cycles/cycle-001/lessons.md` |
| Resolved blockers | `cycles/cycle-001/blocking-issues-resolved.md` |
| Phase 2→3 gate document | `cycles/cycle-001/gates/phase-2-to-3-gate.md` |

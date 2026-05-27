---
document_type: pipeline-state
level: ops
version: "2.0"
status: active
producer: state-manager
timestamp: 2026-05-27T00:00:00
phase: phase-3-tdd-implementation
inputs: []
input-hash: "[live-state]"
traces_to: ""
project: jira-cli
mode: BROWNFIELD
current_step: "S-410-MERGED-PR416"
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
| **Last Updated** | 2026-05-27 — S-410 MERGED via PR #416 (develop @ 04e019a). Issue #410 closed. Cycle closed. |
| **Current Phase** | Phase 3 — TDD Implementation IN PROGRESS — Wave 3 CLOSED (10/10). Feature Mode ongoing. Open backlog: #210, #331, #368, #372, #387, #400, #408, #409. Held Dependabot PRs #403/#404 due 2026-05-31, #368 stale. |
| **Next Phase** | Phase 4: Holdout Evaluation (not started) |
| **Activation HEAD** | dea166471e22eff55974d7675593469b37048c5f (v0.5.0-dev.7) |

## Pipeline Goal

Goal 1c: **Harden v0.5 + feature delivery** — formalize existing codebase with VSDD specs, holdouts, and verification; AND use VSDD pipeline for all post-v0.5.0 feature work.

## Phase Progress

| Phase | Status | Completed | Gate | Finding Progression |
|-------|--------|-----------|------|---------------------|
| pre-pipeline: Setup | COMPLETE | 2026-05-04 | env-preflight | |
| 0: Codebase Ingestion | COMPLETE | 2026-05-04 | Phase A+B+B.5+B.6+C+gate APPROVED | |
| 1: Spec Crystallization | COMPLETE | 2026-05-04 | PASSED — DEC-006/007/008, gate APPROVE | |
| 1d: Adversarial Spec Review | COMPLETE — 3/3 CONVERGED at Pass 28 | 2026-05-04 | 3/3 FULL CONVERGENCE | 30→15→9→5→10→5→4→3→4→0→2→0→3→0→2→0→3→0→3→5→3→4→5→5→5→2→0→0→0 |
| 1-gate-prep: Consistency Validation | COMPLETE | 2026-05-06 | DEC-006/007/008 resolved; ADR-0013 created | CV: 4H/1M; all fixed |
| 2: Story Decomposition | COMPLETE | 2026-05-06 | 31 stories; Phase 2-adv pending | |
| 2-adv: Adversarial Story Review | CONVERGED — Pass 13 CLEAN | 2026-05-07 | 3/3 FULL CONVERGENCE | 14→5→5→5→4→5→4→4→4→1→0→1→0 |
| Phase 2 gate | APPROVED | 2026-05-07 | APPROVED by human | |
| 3: TDD Implementation | IN_PROGRESS — Wave 0/1/2/3 ALL COMPLETE (32/32). Feature Mode active. | — | — | Wave cadence complete; Feature Mode ongoing |
| 3-adv: Wave Adversarial Reviews | WAVE 2 GATE CLOSED 2026-05-08 | 2026-05-08 | GATE-PASSES (consistency pass-02 `8ae5511`) | adv pass-01: 12 findings; fix-PRs A+B+WV2-SEC-01+pass-02 |
| Feature Mode issues #110/#332..#346/#350..#367/#369..#373/#382..#388/#392/#396..#399/#407 | ALL COMPLETE | 2026-05-26 | F1–F7 ALL COMPLETE — CYCLE CONVERGED | F5: 3/3 CLEAN at P4. F6: 100% mutation kill. Regression 1483/0. |
| issue-327 (Dependabot rand 0.9→0.10) | CYCLE CONVERGED — PR #413 @ 375c0f91 | 2026-05-26 | F1–F7 ALL COMPLETE — MAXIMUM_VIABLE_REFINEMENT_REACHED | F5: HIGH-FP→0→0. F6: 100% (2/2). F7: 6/6 PASS. |
| 4: Holdout Evaluation | not-started | | | |
| 5: Adversarial Refinement | not-started | | | |
| 6: Formal Hardening | not-started | | | |
| 7: Convergence | not-started | | | |

## Current Phase Steps

<!-- Keep last 5 rows only. Archive older rows to cycles/cycle-001/burst-log.md. -->

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| #407 F2 PASSED (human-approved 2026-05-25) + F3 PASSED (human-approved 2026-05-25) | state-manager | complete | F2: EC-3.4.017-14 added to BC-3.4.017; adversarial 4 passes, 3/3 CLEAN. F3: S-407 created (16 ACs, 12 test deliverables, 1 SP, LOW criticality, tdd, depends_on S-396). STORY-INDEX total_stories 46→47. |
| #407 F5 CONVERGED — 3 passes, all CLEAN, no fix-PRs; O-1/O-2 routed to #408 | state-manager | complete | F5 trajectory: 4 LOW → 0 → 0. 12/12 conflict-block entries covered. Meta-test (EC-3.4.017-14) mechanically enforces invariant. AWAITING F6. |
| #407 F6 PASS + F7 PASS + CYCLE CLOSED (human-authorized 2026-05-25) | state-manager | complete | F6: Mutation 100% (1/1 in-diff caught). cargo-audit 0 vulns, cargo-deny clean. Regression 1483/0. CI @ 6eb2535 green (2m40s). F7: all 5 dimensions PASS. MAXIMUM_VIABLE_REFINEMENT_REACHED (12 refinement iterations, monotonic decay to zero). PR #411 @ 6eb2535. Issue #407 CLOSED. DI-396-F5-1/DI-396-F5-2 RESOLVED. O-1/O-2 pre-existing → #408. |
| #327 F7 CONVERGED + CYCLE CLOSED (2026-05-26) | state-manager | complete | F7: all 6 dimensions PASS. Mutation 100% (2/2). Regression 1483/0. PR #413 @ 375c0f91. Dependabot PR #327 auto-closed. 4 PG items justified deferrals. 3 lessons codified (L-327-1/2/3). factory-artifacts committed. |
| #410 MERGED 2026-05-27 via PR #416 (develop @ 04e019a) | state-manager | complete | S-410: 13 keychain-transitive tests gated behind JR_RUN_KEYRING_TESTS=1 (6 in multi_cloudid_disambiguation.rs + 7 in oauth_refresh_integration.rs). 1 review cycle: pr-reviewer caught F1 audit undercount (11→12 in multi_cloudid), Copilot pass 1 found description count mismatch (5→6/12→13), Copilot pass 2 clean. Issue #410 auto-closed. F1-AUDIT-MISCOUNT-410 codified as drift deferral (see Drift Items). L-410-1 in lessons.md. |

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
| DEC-019 | S-333 JrError variant taxonomy: introduce `JrError::DeadlineExceeded { remaining_ms, message }` (exit code 124, POSIX `timeout(1)` convention) for client-side caller-deadline-expired errors at all 3 sites (`[deadline:send-entry]` + `[deadline:429-retry]` + `[deadline:bulk-outer]`). REVERSES the earlier F3-pre-approval research-validation pass-02 Q6 decision that said "do NOT introduce a new variant; reuse `ApiError(429)` to avoid taxonomy churn". The reversal was triggered by F5 adversary pass-02 C-2 finding + research-validation pass-03 Q2 verification — empirical re-check found only 6 `JrError` match arms in `src/`, all on `404`, none on `429`. External CLI precedent (kubectl, gh, aws-cli, doctl, fly) unanimously uses a dedicated variant for client-side deadlines, NOT 4xx-code overloading. Cap-vs-deadline precedence also REORDERED so deadline-clamp fires BEFORE BC-X.4.009 cap-abort (research-validation pass-04 Q2 — aws-smithy-rs / tokio::time::timeout / kubectl client-go / RFC 9110 §10.2.3 all treat client deadline as a hard contract superseding server Retry-After). Story `.factory/code-delivery/issue-333/story.md` AC-005 annotated-as-superseded with an AC-005-v2 block (per research-validation pass-04 Q3 — supersession-via-iteration preserves audit trail). | Phase 3 / Feature Mode #333 | 2026-05-12 | orchestrator + research-agent + adversary (4 research-validation docs + 6 adversarial passes) |
| DEC-020 | S-410 keychain test isolation — gate 13 tests behind JR_RUN_KEYRING_TESTS=1 per Option A convention from delta analysis | Per-test audit: multi_cloudid_disambiguation.rs has 12 tests (6 gate, 6 remain always-run for red-gate signal); oauth_refresh_integration.rs has 12 tests (7 gate, 5 remain always-run). Issue body count (10+7) corrected to actual 6+7 after audit. Initial F1 audit missed `test_interactive_render_shows_name_url_and_id`; corrected post-pr-review. F2/F3 skipped per bug-fix routing (no BC changes, no story spec needed beyond tracking). | Feature Mode / issue #410 | 2026-05-26 | orchestrator |
| DEC-021 | 2026-05-27: S-410 MERGED via PR #416 (develop @ 04e019a). 1 review cycle (Copilot found undercounted F1 audit; followup commit 211265a gated the missed test; description count reconciliation 5→6 / 12→13). F1-AUDIT-MISCOUNT-410 stays in drift register pending codification follow-up. Codified as drift deferral per established PG-NNN precedent (single instance, low recurrence risk). | cycle close | 2026-05-27 | state-manager |
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

<!-- Populated during Phase 0 codebase ingestion. RESOLVED/CLOSED rows → cycles/cycle-001/blocking-issues-resolved.md -->

| ID | Area | Description | Severity | Status |
|----|------|-------------|----------|--------|
| DRIFT-001 | Pass 21+ propagation (recurring) | Count/chain-length fixes require downstream grep sweep. P21 missed H-044+L2; P23-001 reaffirms; ADV-P24-001 is third recurrence. Codify as S-7.01. Every count/chain-length change must trigger grep sweep. | MEDIUM | process-gap recurring (S-3.06 codification story in Wave 3) |
| DRIFT-003 | STORY-INDEX → WAVE-PLAN sibling propagation gap | Recurred P1/P2/P3/P4/P7/P8/P9/P12 of Phase 2-adv. Structural pattern. S-3.06 scope should include WAVE-PLAN↔STORY-INDEX↔frontmatter triple-sync verification. | MEDIUM | process-gap (S-3.06 scope expansion needed) |
| DRIFT-004 | STORY-INDEX BC IDs not validated against canonical bc-N-*.md | P6 surfaced BC-6.4.* dangling (since corpus inception). Fix authors must open canonical BC file. | HIGH | process-gap (verify every BC ID against canonical bc-N-*.md) |
| R1-001 | JiraClient::new_for_test_with_instance_url ergonomics | DEFERRED — 2026-05-07 — takes (base_url, instance_url) where one concept might suffice. Test-infra only; no correctness impact. Target: bundle into next workflow.rs/client.rs touch. | LOW | DEFERRED |
| R1-002 | Stale doc comment in workflow.rs handle_open | DEFERRED — 2026-05-07 — referenced "base URL" pre-fix; one-line text fix. Target: assign to next implementer touching workflow.rs. | LOW | DEFERRED |
| S-0.03-S1 | Missing integration test for effective_wid fallback path at list.rs:464-470 | DEFERRED — 2026-05-07 — raw_wid empty → fallback_wid lookup branch has no integration test. Logic correct; coverage gap. Target: bundle into next list.rs/CMDB-related touch. | LOW | DEFERRED |
| S-0.05-F1 | Cosmetic typo "JiaClient" → "JiraClient" in test doc comment | DEFERRED — 2026-05-07 | LOW | DEFERRED |
| S-0.05-F2 | Stale doc comment in renamed test | LOW | TO_VERIFY — 2026-05-07 |
| S-0.05-F3..S-1.04-DEFER-03, S-2.03-DOC-01..S-2.07-DEFER-02 | 14 LOW cosmetic/doc DEFERRED items (Wave 1+2 era) | LOW | DEFERRED — bundle into next relevant touch. Full details: `cycles/cycle-001/blocking-issues-resolved.md` section "Drift Items". |
| S-1.05-DEFER-01 | gitleaks-action Node.js 20 → Node.js 24 required by June 2, 2026 | MEDIUM | DEFERRED — 2026-05-08; Dependabot may auto-pick |
| WV2-FIX-A-FOLLOWUP-01 | 11 test docstrings in auth_output_json.rs cite BC-7.3.004 (should be BC-7.4.013-016) | LOW | DEFERRED — bundle into next develop touch |
| WV2-FIX-A-FOLLOWUP-02 | 2 test names embed bc_6_2_013 (should be bc_6_2_006) | LOW | DEFERRED — bundle into next develop touch |
| WV2-CV-03 | STORY-INDEX Wave 0/1 rows show `draft` status | DRIFT | DEFERRED — S-3.06 sweep |
| WV2-CV-11, WV2-CV-12 | NITs: H-018 annotation + S-0.05-F2 TO_VERIFY target | NIT | DEFERRED |
| DRIFT-005..DRIFT-009, PG-365-2 | Process-gap/drift items (doc-fallout, chore-mode, check-spec-counts scope, L2 propagation, engine-level citation scope) | LOW | process-gap codified; Owner: orchestrator. Target: v0.6 / engine. |
| M-03, O-01, O-08-01..07 | #288 retrospective observations — all filed as GitHub issues | MEDIUM/LOW | FILED → all CLOSED (#382/#383/#384/#385). |
| PG-01..04 | Process gaps from pr4-dispatch adversary passes | LOW | DEFERRED — engine-scope. |
| S-288-pr2-PG group | 13 DEFERRED process-gap items from S-288-pr2 cycle (PG-1a..1g, PG-2a/2b, PG-3a/3b, PG-F1, PG-1b/1c/1e/1f). PG-2c RESOLVED (see blocking-issues-resolved.md). | LOW | DEFER → post-S-288 self-improvement epic. Full details: `cycles/cycle-001/drift-items-deferred-S-288.md`. |
| F1-AUDIT-MISCOUNT-410 | F1 architect undercounted tests in `multi_cloudid_disambiguation.rs` by 1 (claimed 11, actual 12). Reviewer caught during PR. Future: F1 audits of test files should cross-check by counting `^async fn test_\|^fn test_` matches against the explicit per-test table row count before sign-off. | LOW | DEFERRED — single instance, low recurrence risk, codified informally in L-410-1 (lessons.md). Target: next maintenance sweep. No follow-up story created (per PG-NNN precedent for single-occurrence process gaps). |
| S-382-FLAKE-01 | tests/multi_cloudid_disambiguation.rs keychain contention (macOS) | LOW | PRE-EXISTING — future test-infra cleanup. |
| PG-388-4, PG-384-1/2, PG-385-1..7, PG-398-1..5 | Process gaps from issues #388/#384/#385/#398 cycles (checklists, template gaps, spec-guard gaps, worktree-path class) | LOW | CODIFIED in lessons.md / TRACKED IN #400. |
| DI-396-F5 + R2-C4 | #396 cycle drift items: DI-396-F5-1/2/3/4, R2-C4 | LOW | FILED → #407 (F5-1/2), #408 (F5-4), #409 (R2-C4), #410 (F5-3). |

## Convergence Trackers

See `cycles/cycle-001/convergence-trajectory.md` for all per-issue convergence narratives (Phase 1d, Phase 2-adv, Phase 3-adv, issues #288/#382/#383/#384/#385/#396/#398/#407).

Current trajectory summary: All active Feature Mode cycles CONVERGED. BC corpus: 583 BCs. Story corpus: 49 stories (S-410 MERGED 2026-05-27).

## Session Resume Checkpoint

<!-- Keep ONLY the latest checkpoint. Archive prior checkpoints to cycles/cycle-001/session-checkpoints.md. -->
| Field | Value |
|-------|-------|
| **Date** | 2026-05-27 |
| **Position** | **S-410 CYCLE CLOSED — MERGED via PR #416 (develop @ 04e019a).** 13 keychain-transitive integration tests now gated behind JR_RUN_KEYRING_TESTS=1 (6 in tests/multi_cloudid_disambiguation.rs, 7 in tests/oauth_refresh_integration.rs). Issue #410 auto-closed. F1-AUDIT-MISCOUNT-410 codified as drift deferral. L-410-1 in lessons.md. STORY-INDEX v1.4.22 (S-410 → MERGED). Next work surface: held Dependabot PRs #403/#404 (due 2026-05-31) and/or #368 (stale); OR process-gap codification for F1-AUDIT-MISCOUNT-410 follow-up. Open backlog: #210, #331, #368, #372, #387, #400, #408, #409. |
| **Convergence counter** | S-410 MERGED. BC corpus: 583 BCs (unchanged). Story corpus: 49 stories (all feature-mode cycles CONVERGED). |

## Open Issues Tracker (post-#288)

| Issue | Title | Status | Priority | Notes |
|-------|-------|--------|----------|-------|
| #210 | (backlog) | OPEN | — | |
| #331 | Sandbox-blocked defer | OPEN | DEFERRED | Requires sandbox access |
| #372 | cargo-mutants partial baseline | OPEN | LOW | Follow-up from #346 |
| #400 | Test-hardening + process-gap follow-ups from #398 | OPEN | LOW | Filed 2026-05-22 — non-blocking, future maintenance sweep. Tracks TH-398-1..4 + PG-398-1..5. |
| #408 | spec/CLAUDE.md line-anchor citation drift class (follow-up from #396 F5) | OPEN | LOW | Filed 2026-05-25. Need systematic guard or sweep process. |
| #409 | `parsed_number_to_wire_value` helper extraction — tautological test 38 | OPEN | LOW | Filed 2026-05-25. R2-C4: extract helper so test exercises production path. |
| #387 | git history rewrite for demo-evidence blobs | OPEN | LOW | Deferred; force-push needed |
| #368 | (open PR — see backlog) | OPEN | — | |

## Historical Content

| Content | Location |
|---------|----------|
| Burst history (all bursts + extracted Phase Progress narratives + Post-Cycle Housekeeping 2026-05-19) | `cycles/cycle-001/burst-log.md` |
| Convergence trajectory (full per-pass + extracted convergence narratives) | `cycles/cycle-001/convergence-trajectory.md` |
| Session checkpoints (archived) | `cycles/cycle-001/session-checkpoints.md` |
| Lessons learned | `cycles/cycle-001/lessons.md` |
| Resolved blockers + resolved drift items | `cycles/cycle-001/blocking-issues-resolved.md` |
| Closed issues (CLOSED/MERGED/DELIVERED) | `cycles/cycle-001/closed-issues-archive.md` |
| Phase 2→3 gate document | `cycles/cycle-001/gates/phase-2-to-3-gate.md` |

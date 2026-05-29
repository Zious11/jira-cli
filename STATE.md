---
document_type: pipeline-state
level: ops
version: "2.0"
status: active
producer: state-manager
timestamp: 2026-05-29T12:00:00
phase: phase-3-tdd-implementation
inputs: []
input-hash: "[live-state]"
traces_to: ""
project: jira-cli
mode: BROWNFIELD
current_step: "v0.5.0-dev.11-RELEASED-develop-15bf305"
current_cycle: "cycle-001"
dtu_required: false
phase_2_status: APPROVED
phase_2_approved_at: 2026-05-07
phase_2_approved_by: "human (user)"
phase_3_status: IN_PROGRESS
activation_head: "15bf305"
activation_version: "v0.5.0-dev.11"
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
| **Last Updated** | 2026-05-29 — E2E feature (Feature Mode) F1+F2+F3 COMPLETE. Story S-E2E-1 created (12 ACs, MEDIUM/8SP). STORY-INDEX 53→54. Design spec on feat/e2e-live-jira-testing @ c3e967a. Next: F4. |
| **Current Phase** | Phase 3 — TDD Implementation IN PROGRESS — Wave 3 CLOSED (10/10). Feature Mode ongoing. Open backlog: #210, #331, #368, #372, #387, #400, #429. Held Dependabot PRs #404/#422/#423/#424/#425/#426 due 2026-05-31. |
| **Next Phase** | Phase 4: Holdout Evaluation (not started) |
| **Activation HEAD** | 15bf305 (v0.5.0-dev.11) |

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
| #428 MERGED 2026-05-28 via PR #430 (develop @ e1706d4) | state-manager | complete | S-428: wiremock-only refactor extracting `#[doc(hidden)] pub fn resolve_cloud_id` + lifting `AccessibleResource` to module scope in `src/api/auth.rs`. Tests #4/#5/#6 in `tests/multi_cloudid_disambiguation.rs` rewritten in-process. 5 adversarial passes, 3 CLEAN. CI 11/11 green. L-428-1/L-428-2 codified. DEC-029 recorded. |
| S-400-A MERGED 2026-05-28 via PR #431 (develop @ 9d4a65b) | state-manager | complete | S-400-A: TEST-ONLY hardening of 4 echo tests (TH-398-1..4). 4 Copilot rounds → 0 findings. 3/3 CLEAN adversarial passes. CI 11/11 green. #400 stays OPEN (Story B + PG-398-4/5 remain). DEC-030 recorded. |
| v0.5.0-dev.11 RELEASED 2026-05-28 (UTC 2026-05-29) via PR #432 (develop @ 15bf305) | state-manager | complete | Dev release v0.5.0-dev.11 tagged on develop @ 15bf305 (PR #432 squash-merged). 7 commits since dev.10. CI 11/11 green. Release workflow triggered. DEC-031 recorded. |
| E2E feature (F1 APPROVED + F2 COMPLETE) 2026-05-29 | state-manager | complete | Feature Mode: "Live-Jira E2E testing in CI" (DEC-032). F1: zero src/ changes, BC delta EMPTY, LOW risk. F2: NFR-T-E2E-1 added (nfr-catalog.md, MEDIUM, Dimension 6). NFR 40→41. OQ-2 resolved: status names configurable via JR_E2E_STATUS_DONE/JR_E2E_STATUS_IN_PROGRESS. Both guards green. Next: F3 story S-E2E-1 (11 ACs). |
| E2E feature F3 COMPLETE 2026-05-29 (factory-artifacts 187e477) | state-manager | complete | F3 (Incremental Stories): story S-E2E-1 created (12 ACs, MEDIUM/8SP, draft). Traceability: NFR-T-E2E-1 + design-spec §3–§8; BC delta EMPTY. STORY-INDEX v1.4.30→v1.4.31, total_stories 53→54, feature-followup group 21→22. |

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
| DEC-023 | 2026-05-27: S-409 extract `parsed_number_to_wire_value` helper — issue body option 2 (extract + unit-test helper). 6 new inline unit tests in field_resolve.rs cover whole/scientific/fractional/zero/negative/out-of-range cases. Tautological test 38 deleted from tests/issue_edit_field.rs (was reimplementing the production conversion inline 3x). Integration tests 26/27 unchanged. Production behavior byte-identical via helper. Closes #409 on merge. | Tautological tests mask regressions — extracting a named helper and testing it directly gives the assertion real discriminating power. | Feature Mode / #409 | 2026-05-27 | orchestrator |
| DEC-022 | 2026-05-27: S-408 MERGED via PR #417 (develop @ d53278a). 5 stale line-anchor citations re-anchored to symbol-form (2 in CLAUDE.md AI Agent Notes, 3 in bc-3-issue-write.md). Going-forward convention adopted in CLAUDE.md: prefer symbol-form (`<file>::<function>` or `<file>::<function> § "<comment>"`); ~LINE as fallback; existing bare/range citations cleaned up opportunistically (no mass-sweep). 1 Copilot review cycle (caught path-prefix inconsistency on line 336; fixed in bfa333d; re-review clean). Symbol-form citation convention now active in CLAUDE.md AI Agent Notes section. | Stale line-number anchors silently break on refactors; symbol-form is rename-proof. | Feature Mode / #408 | 2026-05-27 | state-manager |
| DEC-024 | 2026-05-27: S-409 MERGED via PR #418 (develop @ 88cf863). 1 Copilot review cycle caught 2 pre-existing f64→i64 precision findings at `parsed_number_to_wire_value` bounds check; Perplexity-validated as real-but-tiny; deferred as follow-up issue #421 (out of scope for byte-identical refactor). Copilot re-review (round 2) clean. 6 new inline unit tests for the helper; tautological test 38 deleted. | Byte-identical refactors surface inherited bugs at low cost; opportunistically flag → file follow-up rather than fix in-scope. | Feature Mode / #409 | 2026-05-27 | state-manager |
| DEC-025 | 2026-05-27: S-421 i64-boundary precision fix — Option C from F1 (two-stage parser: i64-first, then f64 fallback with strict inequalities). F2 spec evolution added EC-3.4.015-4b + updated BC-3.4.015 invariant 5 wording (factory commit 6680de7). 8 new unit tests in field_resolve.rs::tests cover both boundary classes + bonus precision win for 2^53+1..2^63 range. Implementer reported a possibly-pre-existing parallel-run flake on `test_cloud_id_flag_value_not_in_response_exits_64` (NO-KEYCHAIN, left always-run in S-410); deferred — will track via CI behavior on PR. Closes #421 on merge. | Two-stage parser avoids f64 round-trip loss for i64-representable integers; strict inequalities prevent saturation at ±2^63. | Feature Mode / #421 | 2026-05-27 | orchestrator |
| DEC-026 | 2026-05-28: S-421 MERGED via PR #427 (develop @ c7ffb55). 9-round Copilot review cycle (deepest of the project to date): R1 deferred to follow-up; R2 caught BLOCKING precision regression in initial fix; R3-R8 caught doc-prose imprecision (including 4 rounds of stale-cross-reference propagation introduced during rewrites); R5 caught contract-vs-impl mismatch (`trim_start_matches` multi-sign); R6 caught empirically-false serde_json serialization claim; R9 accepted as documented F1 Option C design trade-off (3-way boundary asymmetry at ±2^63 between integer/decimal/scientific notation; trade-off documented in rustdoc). Final fix: 2-stage→3-stage parser (Stage 1 i64 parse, Stage 1.5 strip_integer_decimal_suffix retry, Stage 2 f64 with strict bounds). 20 unit tests in field_resolve.rs::tests (was 14). F2 spec evolution (BC-3.4.015 invariant 5 update + EC-3.4.015-4b) at factory commit 6680de7. Follow-up #428 tracks S-410 architect-miscount extension (3 more NO-KEYCHAIN exit-64 tests need gating). | Deep Copilot review cycles have diminishing returns after R5-ish; once findings transition from 'bugs in fix' to 'imprecision in my own doc cleanup', that is the inflection point. L-421-1..5 codified. | Feature Mode / #421 | 2026-05-28 | state-manager |
| DEC-027 | 2026-05-28: S-428 F1 scope expansion — user chose to close the always-run coverage gap (wiremock-only refactor) rather than accept it (gate-only). F1 v1 proposed gating tests #4/#5/#6 behind JR_RUN_KEYRING_TESTS=1 (simpler, closes CI flakes, but loses always-run exit-64 coverage). User closed the OQ by selecting option (b): extract resolve_cloud_id + rewrite tests in-process. Scope expanded from 3-line gating patch to ~60 LOC production refactor + test rewrites. | F1 human gate for #428 | 2026-05-28 | human |
| DEC-028 | 2026-05-28: S-428 — 4 design decisions locked at F1 human gate: (1) AccessibleResource lifted to module scope with pub(crate) visibility + Debug + PartialEq derives (not function-local); (2) resolve_cloud_id is pub(crate) fn (not async) with return type Result<String, JrError> so tests match variants without downcasting; (3) Vec<AccessibleResource> struct literals in tests (no serde JSON round-trip — cleaner and faster); (4) pub(crate) visibility is unconditional — not cfg(test)-gated — because function may have future callers (e.g., jr auth check). | F1 human gate for #428 | 2026-05-28 | human |
| DEC-029 | 2026-05-28: #429 WONTFIX decision deferred to F7 (open). Issue #429 proposed a crypto-random `JR_SERVICE_NAME` suffix to prevent keychain contention across parallel subprocess tests. Now that #428 has merged (in-process rewrite removes the subprocess keychain-race root cause for tests #4/#5/#6), #429's mechanism is superseded for those 3 tests. WONTFIX decision is pending — #429 may still have value for other subprocess-based tests beyond the 3 rewritten in #428. Defer to next F7 cycle-close or maintenance sweep. Do NOT close #429 autonomously; requires human decision. | Open decision deferred to F7 | Feature Mode / #428 close | 2026-05-28 | deferred-to-human |
| DEC-031 | 2026-05-28 (UTC 2026-05-29): Dev release v0.5.0-dev.11 shipped via branch chore/release-v0.5.0-dev.11 → PR #432 → squash-merge to develop @ 15bf305. Annotated tag v0.5.0-dev.11. 7 commits bundled since dev.10 (S-400-A/#431, #430, #427, #418, #417, #368, #416). Pre-PR local checks all green: cargo check, fmt, clippy -D warnings, full test suite. PR CI 11/11 green. Release workflow triggered (pre-release binaries). Protected-branch + standing branch-PR-tag rule followed throughout. | Dev releases follow the branch+PR+tag flow — no direct commits to develop. | Phase 3 / dev release cadence | 2026-05-28 | state-manager |
| DEC-032 | 2026-05-29: "Live-Jira E2E testing in CI" opened in Feature Mode (BROWNFIELD, per DEC-015). F1 APPROVED: zero src/ changes, BC delta EMPTY, LOW regression risk, one story S-E2E-1 recommended (11 ACs, MEDIUM effort). F2 COMPLETE: NFR-T-E2E-1 added to nfr-catalog.md (Dimension 6: Testing/CI Infrastructure, MEDIUM); NFR count 40→41; CANONICAL-COUNTS.md updated (MEDIUM 15→16, LOW 19→18); both guard scripts green; BC corpus unchanged at 583. OQ-2 resolved: status names configurable via JR_E2E_STATUS_DONE / JR_E2E_STATUS_IN_PROGRESS. Design spec: docs/specs/e2e-live-jira-testing.md on feat/e2e-live-jira-testing @ c3e967a. Provisioning tracking issue R-NEW-1 pending (file GitHub issue before F4). | Test-infra features (zero src/ changes) still warrant a Feature Mode cycle for spec discipline and audit trail. | Phase 3 / Feature Mode | 2026-05-29 | orchestrator + human |
| DEC-030 | 2026-05-28: S-400-A MERGED via PR #431 (develop @ 9d4a65b). TEST-ONLY hardening of TH-398-1..4 (issue #400 Story A). 4 Copilot review rounds converged to 0 findings. Round-3 Copilot finding (config.defaults.output flips dry-run output branch) REFUTED by code trace — `config.defaults.output` is not wired into the runtime output decision in main.rs (which reads `cli.output`, clap-defaulted to "table"); `--output table` flag added as defensive hardening / explicit intent rather than a bug fix. This validates the DEC-018 pattern: validate Copilot causal mechanism by code trace before acting. #400 NOT closed — Story B (PG-398-1 count-guard extension) and engine-scoped items (PG-398-4/5) remain open. | Receiving-code-review discipline: validate stated causal mechanism by code trace; hardening that documents intent is still a valid outcome even when the stated mechanism is wrong. | Feature Mode / S-400-A | 2026-05-28 | orchestrator + code-trace |
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
| L-428-2-PG | Story-writer AC verification greps drift from as-built code. Greps should anchor on stable code-arm patterns, validated against actual code, not speculative implementations. Consider whether story-writer agent prompt should require this. | LOW | DEFERRED — target: next maintenance sweep; reason: low-severity doc-mechanics gap, no runtime impact. No follow-up story created. |
| S-382-FLAKE-01 | tests/multi_cloudid_disambiguation.rs keychain contention (macOS) | LOW | PRE-EXISTING — future test-infra cleanup. |
| PG-388-4, PG-384-1/2, PG-385-1..7, PG-398-1..5 | Process gaps from issues #388/#384/#385/#398 cycles (checklists, template gaps, spec-guard gaps, worktree-path class) | LOW | CODIFIED in lessons.md / TRACKED IN #400. |
| DI-396-F5 + R2-C4 | #396 cycle drift items: DI-396-F5-1/2/3/4, R2-C4 | LOW | FILED → #407 (F5-1/2), #408 (F5-4), #409 (R2-C4), #410 (F5-3). |

## Convergence Trackers

See `cycles/cycle-001/convergence-trajectory.md` for all per-issue convergence narratives (Phase 1d, Phase 2-adv, Phase 3-adv, issues #288/#382/#383/#384/#385/#396/#398/#407).

Current trajectory summary: E2E feature F1+F2+F3 COMPLETE (2026-05-29). NFR-T-E2E-1 added (41 NFRs). BC corpus: 583 BCs (unchanged). Story corpus: 54 stories (S-E2E-1 created, 12 ACs, MEDIUM/8SP, draft). Active: e2e-live-jira-testing F4 (TDD implementation). Develop @ 15bf305 (v0.5.0-dev.11).

## Session Resume Checkpoint

<!-- Keep ONLY the latest checkpoint. Archive prior checkpoints to cycles/cycle-001/session-checkpoints.md. -->
| Field | Value |
|-------|-------|
| **Date** | 2026-05-29 |
| **Position** | **E2E Feature Mode: F1 APPROVED + F2 COMPLETE + F3 COMPLETE. Story S-E2E-1 created (12 ACs, MEDIUM/8SP, draft). Design spec on feat/e2e-live-jira-testing @ c3e967a. Next: F4 delta implementation (TDD).** 54 stories / 41 NFRs. Develop @ 15bf305 (v0.5.0-dev.11). Open backlog: #210, #331, #368, #372, #387, #400 (Story B), #429. Held Dependabot PRs #404/#422/#423/#424/#425/#426 due 2026-05-31. #429 WONTFIX-pending (DEC-029) — do NOT close #429 autonomously. File provisioning GitHub issue (R-NEW-1) before F4. |
| **Convergence counter** | E2E feature F1+F2+F3 complete. BC corpus: 583 BCs (unchanged). NFR corpus: 41 NFRs. Story corpus: 54 stories (+1 S-E2E-1). |
| **Resume prompt** | `Read .factory/STATE.md. E2E feature (Feature Mode, DEC-032): F1✓ F2✓ F3✓ (story S-E2E-1, 12 ACs, draft). Design spec: docs/specs/e2e-live-jira-testing.md on feat/e2e-live-jira-testing @ c3e967a. Next: F4 delta implementation (TDD). File provisioning GitHub issue (R-NEW-1) before F4. 54 stories / 41 NFRs. Develop @ 15bf305 (v0.5.0-dev.11). Dependabot PRs held until 2026-05-31. DEC-029 deferred to human (do NOT close #429).` |

## Open Issues Tracker (post-#288)

| Issue | Title | Status | Priority | Notes |
|-------|-------|--------|----------|-------|
| e2e-live-jira-testing | Live-Jira E2E testing in CI (Feature Mode) | IN_PROGRESS (F4 next) | MEDIUM | DEC-032. F1+F2+F3 COMPLETE. S-E2E-1 (12 ACs, draft). Design spec feat/e2e-live-jira-testing @ c3e967a. File GitHub issue R-NEW-1 before F4. |
| #210 | (backlog) | OPEN | — | |
| #331 | Sandbox-blocked defer | OPEN | DEFERRED | Requires sandbox access |
| #372 | cargo-mutants partial baseline | OPEN | LOW | Follow-up from #346 |
| #400 | Test-hardening + process-gap follow-ups from #398 | OPEN | LOW | Filed 2026-05-22. Story A (TH-398-1..4) MERGED PR #431 @ 9d4a65b (2026-05-28). Story B (PG-398-1 count-guard extension) + engine items (PG-398-4/5) remain open. |
| #429 | jr_isolated() crypto-random JR_SERVICE_NAME suffix to prevent keychain contention across parallel subprocess tests | OPEN | LOW | Filed 2026-05-28. Alternative root-cause fix to #428's approach. Now that #428 merged, #429's mechanism is superseded for tests #4/#5/#6. WONTFIX decision deferred to human (DEC-029). Do NOT close autonomously. |
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

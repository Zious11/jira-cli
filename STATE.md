---
document_type: pipeline-state
version: "2.0"
status: active
timestamp: 2026-06-02T00:00:00
phase: phase-3-tdd-implementation
project: jira-cli
mode: BROWNFIELD
current_step: "jsm-e2e-expansion-F1-approved-F2-spec-complete-F3-pending"
current_cycle: "cycle-001"
dtu_required: false
phase_2_status: APPROVED
phase_2_approved_at: 2026-05-07
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
| **Last Updated** | 2026-06-02 — JSM E2E expansion feature opened (DEC-064): F1 APPROVED, F2 spec complete (docs/specs/jsm-e2e-coverage.md). develop HEAD: afa12570 (no code merged). No active worktrees. |
| **Current Phase** | Phase 3 — TDD Implementation IN PROGRESS — Feature Mode active. JSM E2E expansion (EJ / E2E-JSM): F1+F2 COMPLETE, F3–F7 pending. develop HEAD: afa12570. BC corpus 585. NFR corpus 41. Stories 59. No active worktrees. |
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
| issue-331 (issueType bulk-edit wire schema fix — PR #453 + live-fix #454+#455) | CYCLE CLOSED + LIVE-GREEN — PR #455 → develop @ f418bf5; run 26779732719 66/0; #331 CLOSED | 2026-06-01 | F1–F7 ALL COMPLETE — CYCLE CONVERGED + LIVE-VALIDATED | F5: 3/3 CLEAN (P5/P6/P7; 7 findings fixed). F6: 91.7% mutation. Regression 1568/0. Live run 26777755130 (65/1) caught createmeta schema defect → fixed #454+#455 → live run 26779732719 (66/0) GREEN. |
| E2E fork-safe CI enablement (`JR_E2E_ENABLED` repo-var gate + README badge) — S-E2E-FORK-1 | **CYCLE CLOSED + LIVE-GREEN** — PR #459 → develop @ afa12570; run 26793560680 = 67/0 (2026-06-02). JR_E2E_ENABLED=true repo variable set. 7 files: e2e.yml, e2e-sweeper.yml, README.md, CLAUDE.md, CHANGELOG.md, docs/specs/e2e-fork-safe-ci-enablement.md, e2e-live-jira-testing.md. | 2026-06-02 | F1–F7 ALL COMPLETE — CYCLE CONVERGED + LIVE-GREEN (DEC-063) | F5: sibling-omission→fix→off-branch-spec→fix→polish-idiom-drift→sweep→CLEAN×3. VER-E2E-FORK-1..4 all confirmed. BC: 585 unchanged. NFR: 41 unchanged. No formal VP-NNN (zero Rust). |
| JSM E2E coverage expansion (project EJ / E2E-JSM) — S-JSM-E2E-1 | **F1 APPROVED + F2 COMPLETE — F3 pending** (2026-06-02). 7 scenarios: queue list/view (name+id), requesttype list/fields (numeric-bypass pin), comments internal/external round-trip, issue create --request-type write round-trip, non-JSM guard (BC-X.8.004 exit 64). Zero-src. 1 story ~3 SP. develop HEAD: afa12570 (no code merged). | — | F1 gate APPROVED (DEC-064); F2 spec: docs/specs/jsm-e2e-coverage.md; VER-JSM-E2E-1..7 defined | BC: 585 unchanged. NFR: 41 unchanged. Deferred: --on-behalf-of, 401 scope hint. Rollout: set JR_E2E_JSM_PROJECT=EJ in jira-e2e env. |
| E2E feature (S-E2E-1..5) — Live-Jira E2E testing in CI + E2E enhancements | F7 CONVERGED — SHIPPED + LIVE-GREEN (CYCLE CLOSED 2026-05-31) | 2026-05-31 | F1–F7 ALL COMPLETE all 5 stories; live workflow GREEN (run 26719160283, 57/0; develop @ fef44bd via #440+#441+#442) | S-E2E-1: (4C/4H)→(1C/2H)→(1C/2H/1M)→(2M)→CLEAN×3; S-E2E-2: 1M→CLEAN×3; E2E-enh F2: P1 13→P4 2C/2H→CLEAN×3; F5: 2H→CLEAN×3; live: 54/3→56/1→57/0 |
| issue-327 (Dependabot rand 0.9→0.10) | CYCLE CONVERGED — PR #413 @ 375c0f91 | 2026-05-26 | F1–F7 ALL COMPLETE — MAXIMUM_VIABLE_REFINEMENT_REACHED | F5: HIGH-FP→0→0. F6: 100% (2/2). F7: 6/6 PASS. |
| 4: Holdout Evaluation | not-started | | | |
| 5: Adversarial Refinement | not-started | | | |
| 6: Formal Hardening | not-started | | | |
| 7: Convergence | not-started | | | |

## Current Phase Steps

<!-- Keep last 5 rows only. Archive older rows to cycles/cycle-001/burst-log.md. -->

| Step | Agent | Status | Output |
|------|-------|--------|--------|
| Dev release v0.5.0-dev.13 SHIPPED 2026-06-01 — PR #457 → develop @ ec8f6be; tag v0.5.0-dev.13; run 26785757910 SUCCESS | state-manager | complete | 11 commits bundled since dev.12 (#452-#456 + Dependabot). CI 6/6. Prerelease 2026-06-01T22:29:16Z: 8 assets. DEC-060. |
| assign-by-query E2E LIVE-GREEN 2026-06-02 — PR #458 → develop @ d45ec88; live run 26790203429 = 67/0 (was 66/0) | test-writer + adversary + state-manager | complete | E2E-PG-4 assign-specific-user sub-gap RESOLVED. Adversarial: 5 passes (P1-P2 FINDINGS_REMAIN, P3-P5 CLEAN×3). DEC-061. L-458-1 codified. |
| E2E fork-safe CI enablement feature opened 2026-06-02 — F1 APPROVED → F2 COMPLETE (spec written) | orchestrator + human (F1 gate) + state-manager | complete (F1+F2) | DEC-062: `JR_E2E_ENABLED` repo-variable gate + preflight + README badge. Spec: docs/specs/e2e-fork-safe-ci-enablement.md. VER-E2E-FORK-1..4. develop HEAD: d45ec88 (no code merged). |
| S-E2E-FORK-1 CYCLE CLOSED + LIVE-GREEN 2026-06-02 — F3→F7 COMPLETE; PR #459 → develop @ afa12570; live run 26793560680 = 67/0 | orchestrator + implementer + adversary + state-manager | complete | F3 story S-E2E-FORK-1, F4 7-file delivery (zero src/), F5 CONVERGED (sibling-omission + off-branch-spec + idiom-drift caught → fixed → CLEAN×3), F6 VER-E2E-FORK-4 empirically verified, F7 5-dim MET. JR_E2E_ENABLED=true repo var set. Preflight "OK" printed. L-459-1 + L-459-2 codified. PG-459-1/2 deferred. DEC-063. |
| JSM E2E expansion opened 2026-06-02 — brainstorm (brainstorming-report-jsm-e2e.md) → F1 APPROVED (DEC-064) → F2 COMPLETE (docs/specs/jsm-e2e-coverage.md + VER-JSM-E2E-1..7) | orchestrator + human (F1 gate) + state-manager | complete (F1+F2) | 7 scenarios; zero-src; 1 story ~3 SP; BC 585 unchanged; NFR 41 unchanged. Teardown: SELF-CLOSE in test body. Dynamic RT id + queue discovery. CLEAN-SKIP on JR_E2E_JSM_PROJECT unset/403. Deferred: --on-behalf-of, 401 scope hint. Rollout: JR_E2E_JSM_PROJECT=EJ in jira-e2e env. develop HEAD: afa12570 (no code merged). |

## Decisions Log

| ID | Decision | Rationale | Phase | Date | Made By |
|----|----------|-----------|-------|------|---------|
| DEC-001..DEC-016 | Phase 0/1/2 + Wave-2/3 spec-pivot decisions (HARMONIZE, SD-001/002/003, Phase gates, S-2.06/S-2.07/S-3.03/S-3.07 pivots, Feature Mode pioneer, bundle follow-ups) | Archived to `cycles/cycle-001/burst-log.md` (Burst N+10, 2026-06-01) for SIZE budget. Full text preserved there. | Phase 0→3 | 2026-05-04..2026-05-10 | human + orchestrator |
| DEC-018..DEC-026 | Closed-cycle Feature Mode decisions: Copilot-validation standing rule (#018); S-333 JrError::DeadlineExceeded (#019); S-410 keychain gate (#020/021); S-408 symbol-form citations (#022); S-409 number helper (#023/024); S-421 3-stage precision parser (#025/026) | All MERGED + closed. Full text in `cycles/cycle-001/burst-log.md`. | Phase 3 / 2026-05-11..28 | see cycle archive |
| DEC-027..DEC-036 + DEC-037..DEC-049 + DEC-042 | Closed-cycle Feature Mode decisions (S-428/S-400-A/S-E2E-1/S-E2E-2/E2E-enh) + dev releases dev.11/dev.12/dev.13. All CYCLE CLOSED. Full text archived to `cycles/cycle-001/burst-log.md` (2026-06-02 archival). Key open item: DEC-029 = #429 WONTFIX deferred to human. | All CYCLE CLOSED (except DEC-029 open per human) | Phase 3 / 2026-05-28..31 | see cycle archive |
| DEC-050 | 2026-06-01: E2E-PG-4 label/link coverage gap-fill MERGED via PR #445 (squash 259d450; CI 11/11 green). 3 new portable gated tests (label add/remove roundtrip, link --type/unlink --type, remote-link smoke), test-only zero-src. Research-gated (Perplexity-primary). PROCESS INCIDENT + RECOVERY: orchestrator initially fabricated non-existent commit SHAs (4a8e36b/4be9c33/5f1c9e2) from garbled tool output and built a replacement PR (#444) on a STALE base (2ca9fc1, pre-#443) that regressed the surface guard 9→7; caught by re-verifying against actual git output, recovered the correct commit (dc7c34b, parent c395e27) from the object store, forward-ported the adversary fixes (85198c5), shipped as #445, closed #444 + deleted its branch. Live proof run 26730687481 pending. | Recurrence of the DEC-047 fabrication failure mode — counts/SHAs/run-ids must be command-read, never recalled. PROCESS INCIDENT: stale-base PR (#444) would have regressed a guard if not caught. | Phase 3 / E2E-PG-4 | 2026-06-01 | orchestrator (gh/git-verified) |
| DEC-051 | 2026-06-01: Live e2e run 26730687481 on develop @ 259d450 returned 59/1. The 1 failure (test_e2e_issue_edit_label_add_remove_roundtrip) is the new test working as designed — it caught a genuine jr product bug: single-key `issue edit --label` posts a fabricated, never-live-validated bulk payload that real Jira rejects with HTTP 400. This vindicates the e2e-enhancement thesis (live tests catch what wiremock cannot). Link --type and remote-link smoke passed live. Fix (route single-key to PUT issue update) Perplexity-verified and scoped; pending human go-ahead. e2e.yml is non-blocking so develop is not gated, but the nightly will stay red on this one test until fixed. | Phase 3 / E2E-PG-4 live run | 2026-06-01 | orchestrator (gh/verified) |
| DEC-052 | 2026-06-01: `jr issue edit --label` fix chain COMPLETE + LIVE-GREEN. 4-layer-deep latent bug family caught by live E2E (PR #445). Fix-chain: PR #447 (single-key PUT /issue/{key} update.labels), #448 (labelsFields schema), #449 (integer taskId), #450 (numeric issue-id). Live sequence: 26730687481 (59/1)→26733056812 (60/0)→26733998365 (60/1)→26735034015 (60/1)→26735722804 (61/0 GREEN). Each fix added a REAL-wire-shape offline regression test. Codified L-E2E-12. | Live E2E is the backstop; mock fidelity from client assumptions provides false confidence. | Phase 3 / E2E-PG-4 fix chain | 2026-06-01 | orchestrator (gh/git-verified) |
| DEC-053 | 2026-06-01: Dev release v0.5.0-dev.12 shipped via branch chore/release-v0.5.0-dev.12 → PR #451 → squash-merge develop @ 432f381; annotated tag v0.5.0-dev.12. 11 commits bundled since dev.11 (E2E suite + enhancements #433/#434/#440/#441/#442, CLI-surface guard #443, label/link/remote-link e2e coverage #445, and the issue-edit-label fix chain #447-#450 closing #446/BUG-LABEL-400). Pre-PR checks + CI green; release workflow run 26757668724 building pre-release binaries. Branch→PR→tag flow followed (DEC-031 precedent). Release run 26757668724 COMPLETED SUCCESS (4 platform builds + Create Release); GitHub prerelease v0.5.0-dev.12 published 2026-06-01T13:27:57Z with 8 assets (aarch64/x86_64 × apple-darwin/linux-gnu, tar.gz + sha256). Release CLOSED. | Dev releases follow the branch+PR+tag flow — no direct commits to develop. | Phase 3 / dev release cadence | 2026-06-01 | state-manager |
| DEC-054 | 2026-06-01: Priority/worklog/unassign E2E + bulk-priority fix shipped via PR #452 → develop @ 4fd91f1; live run 26767211620 = 65/0. Bulk `issue edit --priority` → {priorityId} schema via GET /rest/api/3/priority — validated live FIRST-TRY (research-led, no fix-forward chain). VSDD cycle: Perplexity→TDD→adversary 2 MEDIUM fixed→clean. | Upfront schema research avoids live-fix chains. | Phase 3 / E2E-PG-4 + #331 partial | 2026-06-01 | orchestrator (gh/git-verified) |
| DEC-055 | 2026-06-01: #331 F1 gate: (1) cross-project `--type` → exit 64 before any API call; (2) name→issueTypeId = one-shot HTTP via createmeta, NO cache; (3) gated live E2E test (JR_E2E_ISSUE_TYPE_ALT env var); (4) project-key = last-hyphen split. F2: BC-3.4.018/019 (585 BCs). F3: S-331 12 ACs. F4: branch fix/issue-331-issuetype-bulk. | Feature Mode / #331 F1 | 2026-06-01 | human + orchestrator |
| DEC-056 | 2026-06-01: #331 F5 CONVERGED (P5/P6/P7 3-clean) + F6 PASS (91.7% mutation; Mutant B killed @ 723ccd7). LESSON: adversary dispatch MUST include captured diff + HEAD self-check (two passes reviewed wrong-tree). PG-331-1 (surface-guard used⊆listed gap) + PG-331-2 (wrong-tree dispatch) deferred. | Feature Mode / #331 F5+F6 | 2026-06-01 | orchestrator + adversary + formal-verifier |
| DEC-057 | 2026-06-01: #331 issueType bulk CYCLE CLOSED. PR #453 squash-merged to develop @ 6494e27d739619488f509146e5c8011055291ce9. Issue #331 CLOSED (all 3 documented field-shape verifications delivered: priority #452, labels #448, issueType #453; closure comment https://github.com/Zious11/jira-cli/issues/331#issuecomment-4595694697). Worktree .worktrees/issue-331 + branch fix/issue-331-issuetype-bulk removed. issueType bulk now uses verified camelCase issueType + issueTypeId schema with project-scoped resolution + cross-project exit-64 guard. BC corpus: 585 BCs. Process-gaps PG-331-1 (surface-guard used⊆listed direction) and PG-331-2 (adversary dispatch should attach HEAD+diff-stat) confirmed deferred with justification per DEC-056. DRIFT-E2E-ALT tracking live-E2E gated test follow-up (requires JR_E2E_ISSUE_TYPE_ALT in jira-e2e env). | #331 was the final planned field-shape verification; all 3 bulk schema bugs (labels, priority, issueType) from the original empirical-verification scope are now live-green and merged. | Phase 3 / #331 cycle close | 2026-06-01 | state-manager |
| DEC-059 | 2026-06-01: Dependabot 6-PR merge batch — soak policy applied: 7 days from dependency VERSION PUBLISH DATE (not PR-open age); all 6 PRs cleared (tightest: serde_json 1.0.149→1.0.150 at 11 days). 4 Actions major-bump vetting conclusion: sole breaking surface is Node.js 24 runtime (Actions Runner >= v2.327.1), satisfied by GitHub-hosted runners; checkout v6 + upload-artifact v7 already proven green in ci.yml/release.yml/e2e.yml; only laggard workflow files (dependency-review.yml, scorecards.yml) touched. PR #425 (checkout) macOS Test failure = S-382-FLAKE-01 keychain flake (all `test result:` lines 0 failed); cleared by `@dependabot rebase` + re-run. All 6 merged via code-owner approval, no admin bypass. Merge order: #404 (9dfea264) → #424 (9ba3e484) → #422 (e5592edf) → #423 (2ba19c68) → #426 (c4404c890) → #425 (403582e7). develop HEAD = 403582e7. | Phase 3 / Dependabot maintenance | 2026-06-01 | orchestrator (gh/git-verified) |
| DEC-060 | 2026-06-01: Dev release v0.5.0-dev.13 shipped. Branch chore/release-v0.5.0-dev.13 (off develop @ 403582e) → PR #457 → squash-merge @ ec8f6be; tag v0.5.0-dev.13. 11 commits since dev.12 (432f381): #452–#456 + Dependabot #404/#422–#426. Pre-PR checks all green; CI 6/6. PR #457 self-authored: could not self-approve but merged CLEAN (count=0; code-owner auto-satisfied for own PR). Run 26785757910 SUCCESS; prerelease published 2026-06-01T22:29:16Z, 8 assets. | Dev releases follow branch+PR+tag flow (DEC-031/053). | Phase 3 / dev release | 2026-06-01 | state-manager |
| DEC-058 | 2026-06-01: #331 issueType live-validation CLOSED. First live e2e run post-merge (run 26777755130, develop @ 6494e27) produced 65/1: sole failure `test_e2e_issue_edit_issuetype_multikey_bulk_roundtrip`. Root cause re-researched via Perplexity + Atlassian OpenAPI: `GET /rest/api/3/issue/createmeta/{proj}/issuetypes` returns `{"issueTypes":[...]}` — field is `issueTypes`, NOT `values`; pagination is offset-based (startAt/maxResults/total), NOT cursor-based (no isLast). Research report: `.factory/research/issue-331-createmeta-response-schema.md`. Fix: PR #454 (e2e.yml wire JR_E2E_ISSUE_TYPE_ALT, @ 1ee7040) + PR #455 (fix issueTypes/offset, @ f418bf5). Live re-run 26779732719 = 66/0. DRIFT-E2E-ALT RESOLVED. Codified in L-331-LIVE-1 (lessons.md). | Live E2E is the backstop; mock fidelity from client assumptions provides false confidence — second documented instance (first: L-E2E-12 label chain). | Phase 3 / #331 live-validation | 2026-06-01 | orchestrator (gh/git-verified) |
| DEC-061 | 2026-06-02: assign-by-query E2E (E2E-PG-4 sub-gap) delivered test-only via PR #458 → develop @ d45ec88. LIVE-GREEN: run 26790203429 (67/0; was 66/0). Test `test_e2e_issue_assign_by_query` exercises `jr issue assign <KEY> --to <query>` (assignable-user-search path), distinct from no-arg /myself self-assign. Email-primary with display-name fallback; both paths assert changed:true + bounded RYW retry + 3-arm terminal. Clean-skips when JR_E2E_EMAIL unset or displayName hidden. Single-user instance validation: own-account assignment (JR_E2E_EMAIL) — no second user required. KEY LESSON: multiple fresh-context adversarial passes are load-bearing — passes 1-3 rubber-stamped C-1 (bare positional that would hard-fail every live run); passes 4/5 caught it. PG-458-1: surface guard does not validate positional arity. Email-by-query resolves to own account in single-user instance (no 2nd user needed). | Multiple fresh-context passes are load-bearing even for test-only features; surface guard validates flag existence but not positional arity (PG-458-1). | Phase 3 / E2E-PG-4 assign-by-query | 2026-06-02 | orchestrator (gh/git-verified) |
| DEC-063 | 2026-06-02: S-E2E-FORK-1 CYCLE CLOSED + LIVE-GREEN. PR #459 squash-merged to develop @ afa12570. JR_E2E_ENABLED=true repository variable created (repo scope via gh). Post-merge e2e.yml run 26793560680 = completed/success: "E2E preflight OK — all required config present." printed; live suite 67/0. VER-E2E-FORK-2 (canonical repo with var set runs) + VER-E2E-FORK-4 (preflight loud) confirmed live. VER-E2E-FORK-1 (fork skip) verified by gate semantics + research. VER-E2E-FORK-3 (badge green) confirmed from passing run. F5 lessons: (a) adversary caught MISSED SIBLING workflow (e2e-sweeper.yml unguarded) — scope-completeness class; (b) F2 spec authored in main checkout was ABSENT from feature branch (×4 dangling refs) — F-cycle artifacts must be committed onto feature branch before adversary pass (L-459-1); (c) polish-introduced `${VAR:?}`→collect-all idiom drift across spec/comment/sibling-pseudocode — sweep all citations when changing an idiom (L-459-2); (d) gate MUST be repository variable — environment-scoped vars are not available in job-level `if:` (research-confirmed, DEC-062). Process gaps: PG-459-1 (no CI actionlint for workflow YAML), PG-459-2 (no spec-vs-workflow drift check), PG-459-3 (no check that cited paths exist on feature branch). All 3 deferred to maintenance sweep. | Phase 3 / Feature Mode F3–F7 | 2026-06-02 | orchestrator + human + adversary |
| DEC-064 | 2026-06-02: JSM E2E expansion (E2E-JSM / project EJ) — F1 gate decisions locked: (1) 7 scenarios: queue list (id+name shape) + view (name AND --id, dynamic fixture); requesttype list (id+name) + fields <NAME|ID> (numeric-bypass pin); comments internal/external round-trip (--internal → sd.public.comment, comments --output json → assert visibility); issue create --request-type write round-trip (dynamic RT id → create EJ → verify → self-close); non-JSM guard (BC-X.8.004, exit 64). (2) Teardown = SELF-CLOSE in test body; ACCEPT residual mid-panic orphan risk (LOW; sweeper does NOT cover EJ). (3) Dynamic RT id + queue discovery — NO new env var; CLEAN-SKIP on JR_E2E_JSM_PROJECT unset / 403 / empty. (4) NO new BC, NO new NFR. 1 story ~3 SP. Zero-src confirmed. (5) Deferred: --on-behalf-of (needs 2nd customer account), 401 scope hint (scope-stripped token). (6) Rollout: set JR_E2E_JSM_PROJECT=EJ in jira-e2e GitHub Environment (already wired in e2e.yml; no workflow change). | F1 human gate; zero-src confirmed; 7 scenarios | Phase 3 / Feature Mode JSM E2E | 2026-06-02 | human + orchestrator |
| DEC-062 | 2026-06-02: E2E fork-safe CI enablement — F1 gate decisions locked: (1) Gate = REPOSITORY-level variable `JR_E2E_ENABLED` (`if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'`). RESEARCH-CONFIRMED via docs.github.com: repo/org `vars` available in job-level `if:`; environment-scoped vars are NOT ("only available on the runner after the job starts executing") — MUST be a repo variable or gate would always-skip on canonical. `environment: jira-e2e` retained for secrets (load after gate passes); secrets can't be used in `if:`. Fork with var unset → '' → clean skip; scheduled workflows don't run on forks. (2) Preflight step INCLUDED (human chose include over defer): asserts JR_E2E_BASE_URL, JR_E2E_PROJECT, JR_E2E_API_TOKEN, JR_E2E_EMAIL present before suite builds, fail-loud. (3) Badge = GitHub Actions status badge for e2e.yml pinned to develop, 2nd position in README (after CI). (4) 1 story ~3 SP (bumped from 2 for preflight). No new BC (585 unchanged). No formal NFR (D-7 omit; zero Rust). (5) F2: spec at docs/specs/e2e-fork-safe-ci-enablement.md; VER-E2E-FORK-1..4 (fork-skip/canonical-runs/badge/preflight-loud); spec-changelog.md [1.3.1]. | repo-var `vars` is available in job-level `if:`; environment-scoped vars load only after job starts (post-gate). | Phase 3 / Feature Mode F1 | 2026-06-02 | human + orchestrator |

## Skip Log

| Step | Skipped? | Justification |
|------|----------|---------------|

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
| DI-E2E-F5-1 | S-E2E-1 F5 LOW: AC-006 grep text imprecise (matches doc comments; executable code correct). | LOW | DEFERRED — doc/runbook-level. |
| OQ-5 | CLAUDE.md NFR-O-N line stale: "`auth status --output json` covers single-profile JSON" but `src/cli/auth/status.rs` has no JSON arm and makes no API call. Recommend filing separate GitHub follow-up issue: either implement a JSON arm calling /myself, or remove the inaccurate NFR-O-N claim. Out-of-scope for S-E2E-1 (zero src/ change feature). | LOW | DEFERRED — doc drift. File GitHub issue before next auth touch. |
| S-382-FLAKE-01 | tests/multi_cloudid_disambiguation.rs keychain contention (macOS) | LOW | PRE-EXISTING — future test-infra cleanup. |
| PG-388-4, PG-384-1/2, PG-385-1..7, PG-398-1..5 | Process gaps from issues #388/#384/#385/#398 cycles (checklists, template gaps, spec-guard gaps, worktree-path class) | LOW | CODIFIED in lessons.md / TRACKED IN #400. |
| DI-396-F5 + R2-C4 | #396 cycle drift items: DI-396-F5-1/2/3/4, R2-C4 | LOW | FILED → #407 (F5-1/2), #408 (F5-4), #409 (R2-C4), #410 (F5-3). |
| E2E-PG-4 | E2E label/link/priority/worklog/unassign/issueType/assign coverage. Label: DONE #447-#450. Link/unlink/remote-link smoke: DONE PR #445. Priority single+multi-key bulk: DONE PR #452. Worklog+unassign: DONE PR #452. issueType bulk: DONE PR #453+#454+#455 (run 26779732719 66/0). assign-by-query (--to): DONE PR #458 (run 26790203429 67/0; DEC-061). REMAINING OPEN sub-gap: remote-link round-back (blocked: no `jr remote-link read`). | remote-link round-back future | test-infra / e2e-coverage | **partially-addressed — OPEN (1 sub-gap remains; assign-specific-user RESOLVED via #458)** |
| PG-331-1 | [process-gap] CLI surface guard direction gap: the `tests/e2e_cli_surface_guard.rs` guard only validates used-flags ⊆ listed-flags (test invocations reference only flags that exist in `--help`). The reverse direction — listed-flags ⊆ used-flags (every `--help` flag gets a test invocation) — is not enforced. Tagged as I-3 from F5 P1. | LOW | DEFERRED — engine/test-infra scope; target: maintenance sweep. No follow-up story created (engine-scope process gap, low recurrence risk given the existing guard handles the primary failure class). |
| PG-331-2 | [process-gap] Adversary dispatch wrong-tree misread: adversary reviewed main-repo develop instead of the worktree twice (original P1 + original P5 dispatch). Root cause: dispatch prompt lacked a diff attachment and HEAD self-check requirement. Mitigation (per DEC-056): feed captured diff as explicit context + require adversary self-check line. | LOW | DEFERRED / CODIFIED-AS-LESSON — cycles/cycle-001/lessons.md; target: engine-level adversary dispatch prompt template. No follow-up story (codified as lesson; low recurrence risk now mitigation is applied). |
| PG-458-1/2 | [process-gap] Surface guard gaps: (1) does not validate POSITIONAL ARITY per subcommand — C-1 bare-positional survived offline guard and 3 adversarial passes (L-458-1); (2) no reverse flag-completeness check and no `conflicts_with` assertion. Both pre-existing; target: maintenance sweep. No follow-up story. | LOW | DEFERRED — engine/test-infra scope. |
| PG-459-1 | [process-gap] No CI lint (actionlint or similar) for GitHub Actions workflow YAML + embedded shell. The gate/preflight in e2e.yml and e2e-sweeper.yml are validated only by human adversarial review. Target: maintenance sweep. No follow-up story (engine/test-infra scope, same justification class as PG-331-1). | LOW | DEFERRED — engine/test-infra scope. |
| PG-459-2 | [process-gap] No spec-vs-workflow drift check (fenced bash/yaml in `docs/specs/*.md` vs `.github/workflows/*.yml`). The `${VAR:?}`→collect-all idiom drift in S-E2E-FORK-1 survived into the same-PR new spec until caught by adversary. Target: maintenance sweep. No follow-up story (same justification class as PG-331-1). | LOW | DEFERRED — engine/test-infra scope. |
| DRIFT-331-PAGINATION | `get_issue_types_for_project` in `src/api/jira/issues.rs` reimplements offset pagination inline (advances by returned page_len) rather than reusing `OffsetPage<T>` from `src/api/pagination.rs`. Advancing by returned-count vs page-size is theoretically vulnerable to the JRACLOUD-71293 fixed-window-overlap class (advancing by returned-count would overlap windows). Practically moot for issue types (single page of ≤200 entries). DEFERRED per human decision 2026-06-01 — log only, no GitHub issue. Owner: maintainer. Target: next pagination/createmeta touch — reuse `OffsetPage<T>` + advance by maxResults. | LOW | OPEN — tracking only (deferred 2026-06-01). |

## Convergence Trackers

Full per-issue narratives: `cycles/cycle-001/convergence-trajectory.md`. Current: **[2026-06-02] JSM E2E expansion feature opened — F1 APPROVED (DEC-064) + F2 COMPLETE (docs/specs/jsm-e2e-coverage.md). develop HEAD: afa12570 (no code merged; S-E2E-FORK-1 CYCLE CLOSED prior). BC: 585. NFR: 41. Stories: 59. F3–F7 pending for S-JSM-E2E-1.**

## Session Resume Checkpoint

<!-- Keep ONLY the latest checkpoint. Archive prior checkpoints to cycles/cycle-001/session-checkpoints.md. -->
| Field | Value |
|-------|-------|
| **Date** | 2026-06-02 |
| **Position** | **JSM E2E expansion at F2-complete / F3-pending.** Feature cycle "JSM E2E coverage expansion (project EJ / E2E-JSM)" opened. F1 APPROVED (DEC-064) + F2 spec complete (docs/specs/jsm-e2e-coverage.md; VER-JSM-E2E-1..7 defined; spec-changelog.md [1.3.2]). Brainstorming report: .factory/planning/brainstorming-report-jsm-e2e.md. F1 delta-analysis: .factory/planning/jsm-e2e-expansion/delta-analysis.md. develop @ afa12570 (no code merged). No active worktrees. Deferred sub-gaps: --on-behalf-of (needs 2nd customer account), write:servicedesk-request 401 scope hint (scope-stripped token needed). |
| **Convergence counter** | BC corpus: 585 BCs. NFR corpus: 41 NFRs. Story corpus: 59 stories. develop HEAD: afa12570. jira-e2e env: JR_E2E_ISSUE_TYPE_ALT=Bug. DRIFT-331-PAGINATION tracked (deferred). |
| **Next step options** | Continue JSM-E2E Feature cycle: F3 story S-JSM-E2E-1 → F4 (zero-src test+env+guard delivery) → F5/F6/F7. OR: E2E-PG-4 remote-link round-back sub-gap (blocked: needs `jr remote-link read`); open backlog #210/#368/#372/#387/#400 Story B; #429 (human-only DEC-029); OQ-5 (NFR-O-N doc drift). |
| **Key lessons** | (a) Zero-src confirmed for JSM E2E: --internal writes sd.public.comment, comments --output json reads it back — no jr capability gap. (b) Dynamic RT id + queue discovery removes need for new env var. (c) SELF-CLOSE teardown (create → move Done) acceptable; sweeper does not cover EJ. |
| **Resume prompt** | `Read .factory/STATE.md. JSM E2E expansion feature at F2-complete/F3-pending (DEC-064). develop HEAD = afa12570 (PR #459, S-E2E-FORK-1 CYCLE CLOSED prior). No active worktrees. factory-artifacts HEAD = git -C .factory log -1 --format='%h'. jira-e2e env: JR_E2E_ISSUE_TYPE_ALT=Bug; set JR_E2E_JSM_PROJECT=EJ to activate JSM tests. 59 stories / 41 NFRs / 585 BCs. Do NOT close #429 (human decision, DEC-029). OQ-5 open. DRIFT-331-PAGINATION: log-only (deferred). F4 touch-points: tests/e2e_live.rs (7 gated tests), tests/e2e_cli_surface_guard.rs (4 new SURFACE rows), docs/specs/e2e-live-jira-testing.md §4/§8, CLAUDE.md E2E note.` |

## Open Issues Tracker (post-#288)

| Issue | Title | Status | Priority | Notes |
|-------|-------|--------|----------|-------|
| #210 | (backlog) | OPEN | — | |
| #331 | issueType bulk-edit wire schema fix | **CLOSED + LIVE-GREEN** — PR #453 + PR #454+#455 → develop @ f418bf5 (2026-06-01). Issue #331 CLOSED. | HIGH | Full VSDD F1–F7 COMPLETE (DEC-057). Live-fix chain: first live run 26777755130 (65/1, createmeta schema defect) → Perplexity+OpenAPI re-research → PR #454 (e2e wiring) + PR #455 (issueTypes/offset fix) → live run 26779732719 (66/0 ALL GREEN) (DEC-058). BC-3.4.018/019 (585 BCs). DRIFT-E2E-ALT RESOLVED. DRIFT-331-PAGINATION tracked (deferred). |
| #372 | cargo-mutants partial baseline | OPEN | LOW | Follow-up from #346 |
| #400 | Test-hardening + process-gap follow-ups from #398 | OPEN | LOW | Filed 2026-05-22. Story A (TH-398-1..4) MERGED PR #431 @ 9d4a65b (2026-05-28). Story B (PG-398-1 count-guard extension) + engine items (PG-398-4/5) remain open. |
| #429 | jr_isolated() crypto-random JR_SERVICE_NAME suffix to prevent keychain contention across parallel subprocess tests | OPEN | LOW | Filed 2026-05-28. Alternative root-cause fix to #428's approach. Now that #428 merged, #429's mechanism is superseded for tests #4/#5/#6. WONTFIX decision deferred to human (DEC-029). Do NOT close autonomously. |
| #387/#368 | git history rewrite demo-evidence blobs / open PR | OPEN | LOW | #387: deferred; force-push needed. #368: see backlog. |

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

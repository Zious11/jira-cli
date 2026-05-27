---
document_type: closed-issues-archive
level: ops
version: "1.0"
status: archive
producer: state-manager
timestamp: 2026-05-26T00:00:00
cycle: "cycle-001"
inputs: [STATE.md]
traces_to: STATE.md
---

# Closed Issues Archive — cycle-001

<!-- Issues with Status CLOSED / MERGED / DELIVERED extracted from STATE.md Open Issues Tracker on 2026-05-26 (compact-state run).
     Open / DEFERRED / PENDING issues remain in STATE.md. -->

| Issue | Title | Status | Priority | Notes |
|-------|-------|--------|----------|-------|
| #382 | M-03: JrError::InsufficientScope stale text | **CLOSED** (auto-closed via PR #389 b1c863e) | MEDIUM | Delivered 2026-05-19 |
| #383 | O-01: platform-path flag symmetry | **CLOSED** (auto-closed via PR #390 25f7211) | LOW | Delivered 2026-05-19 |
| #384 | O-08-01+O-08-05 JSM 401 auth-aware error hints | **CLOSED** (auto-closed via PR #394 b36b291; 2026-05-20; F7 CONVERGED + CYCLE CLOSED 2026-05-20) | LOW | F1–F7 COMPLETE. 4 new BCs (BC-3.8.014/015, BC-X.8.006/007); spec v1.1.0; CRITICAL OAuth control-flow defect caught + corrected; F4 adversary 3/3 CLEAN; Copilot 3 cycles →0; all 3 spec guards PASS; PG-384-1/2 recorded |
| #385 | O-08-02/04/06/07 UX polish | **CLOSED** (auto-closed via PR #395 f7fc8c3; 2026-05-20; F7 CONVERGED + CYCLE CLOSED) | LOW | F1–F7 COMPLETE. 2 new BCs (BC-3.8.016/017), 3 modified (BC-3.8.002/010/011), 2 holdouts (H-NEW-JSM-RT-006/007), spec v1.2.0 (575 BCs). PG-385-1..7 recorded. |
| #388 | Accurate cross-hierarchy type-change error + fix fake-endpoint hint (Option A) | **CLOSED — DELIVERED** (PR #397 @ e0ea24b; issue #388 closed; cycle CLOSED 2026-05-21) | MEDIUM | F1–F7 COMPLETE. 2 new BCs (BC-3.4.010/011); spec v1.3.0 (577 BCs). F5: 2 CLEAN. F6: 100% mutation kill, 1398/0. F7: all 5 dimensions PASS. PG-388-1/2/3/4 justified deferrals. Ships with next batched develop→main release. |
| #398 | issue edit/create confirmation echoes changed fields | **CLOSED — DELIVERED** (PR #399 @ b49f2fd) | MEDIUM | F1–F7 ALL COMPLETE — CYCLE CLOSED (human-authorized 2026-05-22). BC-3.4.012/013/014 + VP-398-001..006; 580 BCs. F5 CONVERGED (3 clean), F6 100% mutation, F7 5/5 PASS. Ships with next batched develop→main release. |
| #396 | `jr issue edit --field NAME=VALUE` (custom field support) | **CLOSED — DELIVERED** (PR #401 @ `2f61566` + FIX-F5-001 PR #406 @ `699a5fd`, 2026-05-25) | MEDIUM | F1–F7 ALL COMPLETE. 3 new BCs (BC-3.4.015/016/017), 12 VPs; 583 BCs total. F5 CONVERGED; F6 PASS; F7 5/5 PASS. |
| #407 | `--label` conflict-block structural meta-test + coverage (follow-up from #396 FIX-F5-001) | **CLOSED — DELIVERED** (PR #411 @ `6eb2535`, 2026-05-25) | LOW | F1–F7 ALL COMPLETE — CYCLE CLOSED. F5: 3 passes, all CLEAN, no fix-PRs; 4→0→0; 12/12 coverage. F6: 100% mutation kill (1/1). F7: 5/5 PASS. MAXIMUM_VIABLE_REFINEMENT_REACHED. |
| #327 | Dependabot: rand 0.9.4→0.10.1 | **CLOSED — DELIVERED** (PR #413 @ `375c0f91`, 2026-05-26; Dependabot PR #327 auto-closed) | LOW | F1–F7 ALL COMPLETE — CYCLE CONVERGED. F6: mutation 100% (2/2), cargo-deny exit 0, regression 1483/0. F7: all 6 dimensions PASS. 4 PG items (PG-327-1..4) as justified deferrals. 3 lessons (L-327-1/2/3) codified. |
| #386 | docs/demo-evidence removal | **MERGED** @ acdf212 | — | 505 files, ~85 MB freed at HEAD |
| #389 | S-382: JrError::InsufficientScope required_scope refactor | **MERGED** @ b1c863e (2026-05-19T18:40:25Z) | — | PR merged; issue #382 auto-closed |
| #390 | S-383: platform-path inverse warnings (--field/--on-behalf-of) | **MERGED** @ 25f7211 (2026-05-19) | — | PR merged; issue #383 auto-closed |
| #391 | docs: harmonize bc-3 subdomain 3.8 heading to ### N.M format | **CLOSED** (factory-artifacts commit 2026-05-20; DEFER-383-1 resolved) | LOW | bc-3 subdomain 3.8 heading harmonized to `### 3.8` format; stale deferral note removed |
| #392 | ci: extend spec-count guard to validate cumulative total_bcs + BC-INDEX section headers | **CLOSED** (auto-closed via PR #393 0be2e3a) | LOW | DRIFT-002 guard live; DEFER-383-3 + DRIFT-BC2-PROSE resolved |
| #374 | Dependabot: cargo-deny-action 2.0.17→2.0.18 | **MERGED** @ aac5ff4 (squash, 2026-05-20) | — | 9-day soak (pub 2026-05-11); CI green |
| #377 | Dependabot: open 5.3.4→5.3.5 | **MERGED** @ cb3436a (squash, 2026-05-20) | — | 9-day soak; CI green |
| #376 | Dependabot: assert_cmd 2.2.1→2.2.2 | **MERGED** @ b2d066b (squash, 2026-05-20) | — | 9-day soak; CI green |
| #375 | Dependabot: clap_complete 4.6.2→4.6.5 | **MERGED** @ a66d664 (squash, 2026-05-20) | — | 9-day soak; CI green |
| #410 | keychain-touching test isolation infra — developer macOS (S-410) | **CLOSED — DELIVERED** (PR #416 @ 04e019a; 2026-05-27; issue #410 auto-closed) | LOW | F1–F7 (F2/F3 skipped per bug-fix routing). 13 keychain-transitive tests gated behind JR_RUN_KEYRING_TESTS=1 (6 in multi_cloudid_disambiguation.rs + 7 in oauth_refresh_integration.rs). 1 review cycle: pr-reviewer found F1 audit undercount (11→12), Copilot pass 1 found description count mismatch (5→6/12→13), Copilot pass 2 clean. F1-AUDIT-MISCOUNT-410 codified as drift deferral. |

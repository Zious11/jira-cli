---
document_type: story-index
phase: phase-2-story-decomposition
producer: story-writer
version: "1.4.16"
total_stories: 44
total_waves: 4
status: complete-pending-adv-review
last_updated: 2026-05-20 (S-388 added; feature mode F3; 43→44)
activation_head: dea1664
---

# Story Index — jira-cli (jr)

Phase 2 Story Decomposition. Activation HEAD: dea1664 (v0.5.0-dev.7).
Phase 1 converged at adversary Pass 28. Gate approved 2026-05-04.

---

## Wave Plan

| Wave | Theme | Story count | Estimated effort | Gate |
|------|-------|-------------|------------------|------|
| 0 | MUST-FIX bugs + SD-002/SD-003 security + H-NEW-AUTH-002 holdout | 7 | ~5-6 dev-days | All H-MUST-FAIL holdouts become MUST-PASS; no regression on H-001..H-047 |
| 1 | High-priority security posture, supply-chain hardening, structured logging, regression holdouts | 8 | ~6-7 dev-days | NFR-S-E/F gate; wave-0 holdouts green; H-001..H-006 MUST-PASS |
| 2 | Medium-priority NFRs, BC-2/3/4/5 holdout suites, JSON output policy, documentation | 7 | ~5-6 dev-days | NFR-P-* gate; H-030..H-044 MUST-PASS |
| 3 | Low priority + deferred (DEFER NFRs, shard splits, process codification, DOCUMENT-AS-IS) | 10 | ~5-7 dev-days | Per-story gates; no v0.5 blocking |
| feature-followup | Audit-followup test pins for shipped features (S-333, #340) | 1 | ~0.5 dev-days | cargo test green; per-story BC gates |

**Final totals: 44 stories (authoritative count — see `total_stories` frontmatter and Story Manifest section).** Wave 0: 7, Wave 1: 8, Wave 2: 7, Wave 3: 10 (+S-3.10 added during Wave 2 as S-2.06 DEFER-01 follow-up). Wave 2: **7/7 COMPLETE** (PRs #303-#309; 2026-05-08). Feature-followup group: 12 stories — S-340 (task_id pin; 2026-05-15), S-345 (label-coalesce refactor + proptest; 2026-05-16), S-346 (cargo-mutants CI job + whitelist policy; 2026-05-16), issue-288-pr1-api, issue-288-pr2-cli, issue-288-pr4-dispatch (JSM request type cycle 3; 2026-05-18/19), S-382 (JrError::InsufficientScope refactor; 2026-05-19), S-383 (platform-inverse warnings; 2026-05-19), S-392 (cumulative BC-count guard DRIFT-002; 2026-05-19), S-384 (JSM 401 auth-aware hints; 2026-05-20), S-385 (JSM input validation UX polish; 2026-05-20), S-388 (cross-hierarchy edit --type 400 enrichment; 2026-05-20). Note: the earlier "36 stories" figure was the wave-only baseline before feature-followup stories were added; the authoritative count is always `total_stories` / the Story Manifest row count. Updated 2026-05-20 (S-388 added; feature mode F3; 43→44).

Story file naming: `stories/wave-W/S-W.NN-short-slug.md`
Story ID convention: `S-W.NN` (e.g., `S-0.01`, `S-1.03`)

---

## Wave 0 — MUST-FIX + Security (Active)

All Wave 0 stories are CRITICAL or HIGH priority. No v0.5 release without green on all Wave 0 holdouts.

| Story ID | Title | BC Anchors | Holdout Anchors | Status | Est. Effort |
|----------|-------|------------|-----------------|--------|-------------|
| S-0.01 | Fix `handle_open` OAuth instance URL | BC-3.4.001 | H-046 | merged (PR #289 / b7b9c9c) | small |
| S-0.02 | Paginate `list_worklogs` | BC-X.5.002 | H-045 | merged (PR #290 / a84e063) | small |
| S-0.03 | Multi-workspace asset HashMap composite key | BC-4.3.001 | H-036 | merged (PR #291 / cb2c612) | small |
| S-0.04 | Multi-profile fields active-profile migration | BC-6.3.001 | H-NEW-MP-001 | merged (PR #292 / dbbea12) | medium |
| S-0.05 | Gate `JR_AUTH_HEADER` behind `#[cfg(debug_assertions)]` (canonized from `#[cfg(test)]`) | SD-002 / NFR-S-B | H-NEW-AUTH-002 | merged (PR #293 / d907504) | small |
| S-0.06 | Add `--verbose-bodies` flag + PII warning | SD-003 / NFR-S-C | (new holdout per SD-003) | merged (PR #294 / 06ecd6a) | small |
| S-0.07 | Formalize holdout H-NEW-AUTH-002 in spec | SD-002 (docs) | H-NEW-AUTH-002 | merged (factory-artifacts direct / spec-only) | xsmall |

Wave 0 story files: `stories/wave-0/S-0.NN-*.md`

---

## Wave 1 — High Priority Infrastructure (Added 2026-05-06)

Wave 1 covers HIGH-priority security posture, supply-chain hardening, structured logging,
and critical regression-pinning holdouts. All stories are independent of each other
except S-1.03 (depends on S-0.06) and S-1.06 (depends on S-0.05 — OAuth holdout suite
benefits from S-0.05's `#[cfg(debug_assertions)]` gate landing first). Can otherwise be implemented in parallel groups.

Parallel group A: S-1.01, S-1.02, S-1.04, S-1.05 (CI/CD hardening, no code deps)
Parallel group B: S-1.07, S-1.08 (independent holdout suites); S-1.06 starts after S-0.05 merges
Sequential: S-1.03 after S-0.06 merges (tracing depends on --verbose-bodies flag)

| Story ID | Title | NFR/BC Anchors | Holdout Anchors | Status | Est. Effort |
|----------|-------|----------------|-----------------|--------|-------------|
| S-1.01 | Pin GitHub Actions to full commit SHAs | NFR-S-E, R-H6 | — | merged (PR #295 / adae3c5) | small |
| S-1.02 | cargo-deny supply chain hardening | NFR-S-F | — | merged (PR #296 / 88a2e02) | small |
| S-1.03 | Add tracing + wire structured logging | NFR-O-A | — | merged (PR #297 / 2d64112) | medium |
| S-1.04 | Add timeout-minutes to all CI/CD jobs | R-L12 | — | merged (PR #298 / e0ea180) | xsmall |
| S-1.05 | GitHub secret scanning + gitleaks CI | NFR-S-E, R-L13 | — | merged (PR #299 / da4c527) | small |
| S-1.06 | OAuth flow holdout suite | BC-1.1.001, BC-1.1.002 | H-001..H-006, H-022, H-029 | merged (PR #300 / f49af67) | medium |
| S-1.07 | Rate-limit holdout suite | BC-X.1.005, BC-X.4.002 | H-013, H-027 | merged (PR #301 / 5813059) | small |
| S-1.08 | Keychain per-profile layout holdout | BC-1.4.027, BC-1.4.025 | H-016 | merged (PR #302 / ab19783) | small |

Wave 1 story files: `stories/wave-1/S-1.NN-*.md`

### Wave 1 exit gate

All of the following must be true before Wave 2 dispatch:
- H-001, H-002, H-003, H-004, H-005, H-006, H-022, H-029 MUST-PASS (S-1.06 test suite green)
  (Note: H-007, H-008 are in S-2.02 scope; H-006 is in BOTH S-1.06 and S-2.02 for dual coverage)
- H-013, H-027 MUST-PASS (S-1.07 test suite green)
- H-016 MUST-PASS (S-1.08 test suite green)
- All Wave 0 holdouts remain green (no regression)
- NFR-S-E: no floating action tags in `.github/workflows/` (S-1.01)
- NFR-S-F: `cargo deny check bans` exits 0 (S-1.02)
- NFR-S-E: gitleaks CI job passes (S-1.05)
- S-1.03 (tracing): `cargo test --all-features` green; verbose behavior unchanged

---

## Wave 2 — Medium Priority (Added 2026-05-06)

Wave 2 covers MEDIUM-priority NFRs requiring code changes, regression-pinning holdout
suites for bounded contexts BC-2 through BC-7, and policy decisions for JSON output
shapes and test naming conventions.

Parallel group A: S-2.01, S-2.02, S-2.03, S-2.04 (holdout suites, no code deps between them)
Parallel group B: S-2.05 (documentation only, no code changes)
Parallel group C: S-2.06, S-2.07 (code changes, independent of each other)
Note: S-2.03's S-0.03 dependency was demoted to a recommended merge order (no longer blocks); S-2.07 and S-2.05 both modify CLAUDE.md
(coordinate merge order to avoid conflicts).

| Story ID | Title | NFR/BC Anchors | Holdout Anchors | Status | Est. Effort |
|----------|-------|----------------|-----------------|--------|-------------|
| S-2.01 | BC-2 issue-read holdout suite | BC-2.1.001, BC-2.1.007, BC-2.1.012, BC-X.7.006, BC-X.2.005, BC-X.2.006, BC-3.7.001, BC-3.7.004, BC-7.3.001 | H-021, H-030..H-035 | merged (PR #303 / f6516f8) | medium |
| S-2.02 | BC-3 issue-write holdout suite | BC-3.2.001, BC-3.2.009, BC-2.1.013, BC-X.7.004 | H-006, H-007, H-008, H-014 | merged (PR #304 / 7528960) | medium |
| S-2.03 | BC-4 assets/CMDB holdout suite | BC-4.2.001, BC-4.3.002, BC-4.2.006 | H-037, H-038, H-039 | merged (PR #305 / e9c2ba8) | small |
| S-2.04 | BC-5/7 boards, sprints, ADF holdout suite | BC-5.2.001, BC-5.2.005, BC-5.2.007, BC-5.2.008, BC-5.3.001, BC-5.3.002, BC-7.2.001 | H-040..H-044 | merged (PR #306 / ada9126) | medium |
| S-2.05 | CLAUDE.md documentation update | NFR-O-L, NFR-O-M, NFR-O-O, NFR-O-V, NFR-O-R, NFR-R-F | — | merged (PR #307 / 7f004ca) | small |
| S-2.06 | Worklog timeSpent server-side parsing + CMDB cache tuple pin | NFR-R-C, BC-X.5.009, BC-6.2.006 | — | merged (PR #308 / c8f15d8) | small | <!-- v2.0.0 2026-05-08: pivot from admin-only timetracking config fetch to timeSpent string passthrough; see .factory/research/S-2.06-jira-timetracking-verification.md --> |
| S-2.07 | Auth --output json (4 subcommands) + verb-aligned JSON policy + test naming | BC-7.1.001, BC-7.4.013, BC-7.4.014, BC-7.4.015, BC-7.4.016, BC-7.3.005, NFR-O-F, NFR-O-J, NFR-O-W | H-020 | merged (PR #309 / ca22be0) | small | <!-- v2.1.0 2026-05-08: BC anchors re-anchored from BC-7.3.004 to BC-7.1.001+BC-7.4.013-016; see WV2-ADV-01 fix-PR A --> |

Wave 2 story files: `stories/wave-2/S-2.NN-*.md`

### Wave 2 exit gate

All of the following must be true before Wave 3 dispatch:
- H-021, H-030..H-035 MUST-PASS (S-2.01 test suite green; H-021 pre-existing at tests/issue_list_errors.rs:369)
- H-006, H-007, H-008, H-014 MUST-PASS (S-2.02 test suite green)
- H-037, H-038, H-039 MUST-PASS (S-2.03 test suite green)
- H-040..H-044 MUST-PASS (S-2.04 test suite green)
- All Wave 0 and Wave 1 holdouts remain green (no regression)
- NFR-O-L: CLAUDE.md contains the 4 orphan module entries (S-2.05)
- NFR-R-C: worklog POST uses `timeSpent` string (server-parsed), not `timeSpentSeconds` with hardcoded 8/5 (S-2.06 v2.0.0)
- NFR-O-F: `jr auth login/switch/logout/remove/refresh --output json` emit structured JSON (S-2.07)
- Snapshot tests green (S-2.07 insta snapshots)

---

## Wave 3 — Low Priority / Deferred (Added 2026-05-06)

Wave 3 covers LOW-severity NFRs requiring code (refactors and small fixes), DEFER NFRs carried
forward from Wave 2, process-gap codification (DRIFT-001), and DOCUMENT-AS-IS closures for
all remaining LOW NFRs. All stories are independent and can be implemented in parallel.

Parallel group A: S-3.01, S-3.02 (shard splits — independent of each other, no deps)
Parallel group B: S-3.03, S-3.04, S-3.05 (OAuth + asset concurrency — independent)
Parallel group C: S-3.06, S-3.07, S-3.08, S-3.09 (process + documentation — independent)
Note: S-3.08 depends on S-2.05 merging first (CLAUDE.md conflict risk).

| Story ID | Title | NFR/BC Anchors | Holdout Anchors | Status | Est. Effort |
|----------|-------|----------------|-----------------|--------|-------------|
| S-3.01 | Shard-split src/cli/auth.rs (1,998 LOC) | NFR-O-D, BC-1.1.001, BC-1.4.027, BC-7.4.013, BC-7.4.014, BC-7.4.015, BC-7.4.016 | — | completed (PR #319 / b20cfee) | medium |
| S-3.02 | Shard-split src/cli/assets.rs (1,055 LOC) | NFR-O-D, BC-4.2.001, BC-4.2.006 | H-037, H-038 | completed (PR #318 / 68092af) | small |
| S-3.03 | Investigate + wire refresh_oauth_token | NFR-O-B, BC-1.1.002, BC-1.4.027 | — | completed (PR #321 / 597dd23) | large |
| S-3.04 | Multi-cloudId --cloud-id flag + prompt | NFR-O-S, BC-1.5.038, BC-1.1.007, BC-1.5.031 | H-047 | completed (PR #320 / b6ab77c) | medium |
| S-3.05 | Asset enrichment join_all → buffer_unordered(8) | NFR-P-NEW-1, BC-4.3.002, BC-X.1.005 | H-038 | completed (PR #316 / 10e1db4) | small |
| S-3.06 | Pre-merge spec numeric claim checker (DRIFT-001) | — | — | completed (PR #314 / 01ba293) | small |
| S-3.07 | LOW NFR code fixes (Retry-After cap, profile name precision) + /rest/api/3/search/jql anti-loop guard for confirmed JRACLOUD-94632 bug | BC-X.4.009, BC-6.1.004, NFR-R-NEW-1, NFR-S-D, NFR-R-F | H-027 | completed (PR #315 / 6bce18c) | small |
| S-3.08 | DOCUMENT-AS-IS LOW NFR closures: source comments + CLAUDE.md additions | NFR-R-G, NFR-O-C/E/G/H/I/N/P/R/T/U/X, NFR-SCA-1/2/3 | — | completed (PR #317 / fba47ad) | small |
| S-3.09 | Formally record PKCE deferral (SD-001 → DEFER; ADR-0013) | NFR-S-A, BC-1.5.036 | — | completed (factory-artifacts@c2231f5) | xsmall |
| S-3.10 | Rewrite format_roundtrip proptest + delete deprecated 3-arg parse_duration calculator + retire H-018 | BC-X.5.005 | H-018 (to delete) | completed (PR #313 / f492e59) | small | <!-- follows S-2.06 (depends_on); Option 4 follow-up per .factory/research/H-018-holdout-strategy-research.md --> |

Wave 3 story files: `stories/wave-3/S-3.NN-*.md`

### Wave 3 exit gate

All of the following must be true before Phase 2 is considered fully complete:
- S-3.01: `cargo test --lib` green after cli/auth.rs shard split; no shard file >800 LOC
- S-3.02: `cargo test --lib` green after cli/assets.rs shard split; H-037, H-038 still green
- S-3.03: `refresh_oauth_token` either wired (Option A test green) or removed (no dead_code lint)
- S-3.04: H-047 updated from KNOWN-GAP to MUST-PASS; AC-002 and AC-006 green (AC-001 is the --cloud-id flag-override regression guard, not the H-047 fixture)
- S-3.05: asset enrichment concurrency cap ≤8; H-038 still green
- S-3.06: `scripts/check-spec-counts.sh` exits 0 on current spec corpus; exits 1 on corrupted frontmatter
- S-3.07: H-027 updated from KNOWN-GAP to MUST-PASS; `parse_duration("99999999999999w")` returns Err
- S-3.08: `cargo clippy -- -D warnings` exits 0; all 15 DOCUMENT-AS-IS LOW NFRs have a paper trail
- S-3.09: NFR-S-A routing column in nfr-catalog.md reads `DEFER (per ADR-0013)`; ADR-0013 has Reactivation section
- S-3.10: `rg -n "parse_duration\b" --type rust src/` returns only `parse_duration_validate` hits; H-018 physically deleted from holdout-scenarios.md; total_holdouts updated 51→50; `cargo test --all-targets` green

---

## Feature Followup — Audit-followup Test Pins (Added 2026-05-15)

Feature-followup stories pin behavioral contracts for production code already shipped.
They have `wave: feature-followup` in frontmatter and live under `.factory/code-delivery/<issue>/story.md`.

| Story ID | Title | BC Anchors | Holdout Anchors | Status | Est. Effort |
|----------|-------|------------|-----------------|--------|-------------|
| S-340 | Pin task_id-in-bulk-poll-timeout-message contract with regression test | BC-3.4.009 | — | MERGED — PR #370 @ 394dc25 (2026-05-16) | small |
| S-345 | Extract label-coalesce JSON builder into pure function with proptest coverage | BC-3.4.006 | — | MERGED — PR #371 @ bb352ea (2026-05-16) | small |
| S-346 | Add cargo-mutants CI job + whitelist policy for bulk + create modules | — | — | MERGED — PR #373 @ d909e65 (2026-05-16) | small |
| S-382 | Refactor JrError::InsufficientScope Display to use structured required_scope field (closes #382) | BC-1.6.042 | — | completed (PR #389 / b1c863e; merged 2026-05-19) | small (2 SP) |
| S-383 | Emit stderr warnings when --field/--on-behalf-of used without --request-type on platform path (closes #383) | BC-3.8.012, BC-3.8.013 | — | completed (PR #390 / 25f7211; merged 2026-05-19) | small (2 SP) |
| S-392 | Add cumulative BC-count CI guard: check-bc-cumulative-counts.sh + DRIFT-002 (closes #392) | N/A — CI tooling | — | completed (PR #393 / 0be2e3a; merged 2026-05-20) | medium (4 SP) |
| S-384 | JSM 401 auth-aware hints: gate is_oauth_auth() in handle_jsm_create + require_service_desk (closes #384) | BC-3.8.014, BC-3.8.015, BC-X.8.006, BC-X.8.007 | H-NEW-JSM-RT-003 | **completed** — PR #394 / b36b291 (2026-05-20) | medium (5 SP) |
| S-385 | JSM input validation UX polish: harmonize project-required error, guard empty --request-type, reject --markdown+--field description= conflict, move platform-flag warnings post-require_service_desk (closes #385) | BC-3.8.016, BC-3.8.017, BC-3.8.002, BC-3.8.010, BC-3.8.011, BC-3.8.003 (regression-pin) | H-NEW-JSM-RT-006, H-NEW-JSM-RT-007 | **completed** — PR #395 / f7fc8c3 (2026-05-20) | medium (5 SP) |
| S-388 | Cross-hierarchy `edit --type` 400 enrichment + fix `--no-parent` fake-endpoint hint (closes #388) | BC-3.4.010, BC-3.4.011, BC-3.4.003 (annotation-only) | — | **completed** — PR #397 / e0ea24b (2026-05-21; squash-merged; issue #388 closed) | medium (5 SP) |

Feature-followup story files: `.factory/code-delivery/issue-NNN/story.md`

---

## Cycle 3 — Feature: JSM Request Types (Issue #288)

Feature cycle 3 implements JSM request type support: `jr requesttype list/fields` discovery
commands and `jr issue create --request-type` dispatch to `POST /rest/servicedeskapi/request`.

Decomposed into 3 stories across 3 waves (pr3-scope dropped 2026-05-18; scope absorbed into pr4).
Dependency graph and BC traceability matrix live at
`.factory/code-delivery/issue-288-pr1-api/dependency-graph.md`.

| Story ID | Title | BC Anchors | Holdout Anchors | Status | Est. Effort | Wave |
|----------|-------|------------|-----------------|--------|-------------|------|
| issue-288-pr1-api | JSM request submission API client + types (no CLI surface) | BC-3.8.001 (partial), BC-X.12.001, BC-X.12.005, BC-X.12.008 | — | completed (PR #379 / 0f219eb; merged 2026-05-18) | medium (3 SP) | 1 |
| issue-288-pr2-cli | jr requesttype list/fields discovery commands + cache | BC-X.12.001..008, BC-X.8.004 | H-NEW-JSM-RT-002 (partial), H-NEW-JSM-RT-005 | completed (PR #380 / 9d0b72c; merged 2026-05-19) | medium (3 SP) | 2 |
| issue-288-pr4-dispatch | jr issue create --request-type dispatch fork + OAuth scope addition | BC-3.8.001..010, BC-3.3.001, BC-1.3.023, BC-X.3.005 | H-NEW-JSM-RT-001..004 | completed (PR #381 / 95232555; merged 2026-05-19) | large (5 SP) | 3 |

Cycle 3 story files: `.factory/code-delivery/issue-288-pr{1,2,4}-*/story.md`

Wave 1: issue-288-pr1-api (sole Wave 1 story; pr3-scope dropped)
Wave 2: issue-288-pr2-cli (depends on pr1)
Wave 3: issue-288-pr4-dispatch (depends on pr1, pr2; absorbs OAuth scope addition from former pr3-scope)

Total cycle SP: 11 (was 11 with 4 stories at 3+3+1+4; now 11 with 3 stories at 3+3+5).

---

## Cross-Reference Convention

Each story frontmatter uses:
- `bc_anchors:` — list of BC-S.SS.NNN IDs this story implements
- `holdout_anchors:` — list of H-NNN IDs (MUST-FAIL pre-fix, MUST-PASS post-fix)
- `nfr_anchors:` — NFR IDs this story satisfies
- `risk_anchors:` — Risk register IDs (R-NNN) mitigated or pinned by this story
- `adr_refs:` — ADR IDs that constrain this story
- `sd_refs:` — Security Decision IDs (if applicable)
- `files_modified:` — source files touched (with line ranges)
- `test_files:` — test files to create or modify

---

## Pre-existing Test Coverage

Holdouts confirmed covered by tests present in the codebase at activation HEAD dea1664.
These are formally anchored in story `holdout_anchors:` but do not require net-new test
code — the implementer should verify the existing test still passes and consolidate it
into the story's test file if the story creates a new file for that BC area.

| Holdout | Behavioral Contract | Pre-existing Test File | Line(s) | Anchored In |
|---------|---------------------|------------------------|---------|-------------|
| H-009 | BC-2.3.035 (corrupt teams.json graceful degrade) | `tests/issue_view_errors.rs` | 146 | (no story anchor — gap, see below) |
| H-010 | BC-2.2.018 / BC-2.2.019 (issue list default truncates at 30; --all bypasses cap) | `tests/all_flag_behavior.rs` | 90 | (no story anchor — gap, see below) |
| H-011 | BC-6.1.001 / BC-6.1.002 (legacy config migration + idempotency) | `tests/migration_legacy.rs` | 94, 146 | (no story anchor — gap, see below) |
| H-012 | BC-1.6.042 / BC-X.3.005 (401 scope-mismatch InsufficientScope + workaround docs) | `tests/api_client.rs` | 100, 184, 219 | (no story anchor — gap, see below) |
| H-015 | BC-2.2.020 (--all + --limit clap mutual exclusion) | `tests/cli_smoke.rs` | 263 | (no story anchor — gap, see below) |
| H-017 | BC-4.1.002 (AQL clause uses field NAME + capital Key) | `src/jql.rs` | 278–308 | (no story anchor — gap, see below) |
| H-018 | BC-X.9.002 / BC-X.9.003 (parse_duration combined units vs validate_duration) | `src/duration.rs::tests::test_complex` | 90 | S-2.06 |
| H-019 | BC-6.1.004 (validate_profile_name rejects reserved/invalid names) | `src/config.rs` | 759, 769 | (no story anchor — gap, see below) |
| H-021 | BC-2.1.007 (--status ambiguous short-circuit, no JQL fired) | `tests/issue_list_errors.rs` | 369 | S-2.01 (AC-007) |
| H-023 | BC-2.1.012 (--asset substring ambiguous rejection) | `tests/assets.rs` | 1485, 1553 | S-2.01 (via BC-2.1.012 in bc_anchors) |
| H-024 | BC-4.2.007 (assets schema --type substring ambiguous) | `tests/assets.rs` | 1696 | S-2.03 |
| H-026 | BC-7.3.002 (extract_error_message errors object mixed values) | `tests/api_client.rs` | 310 | (no story anchor — gap, see below) |

### Gap Register — Unanchored Holdouts

Holdouts with no story anchor and no pre-existing formal test. These represent coverage
gaps that are not blocking for v0.5 but should be tracked.

| Gap ID | Holdout | BC | Pre-existing Test | Justification | Resolution Target |
|--------|---------|-----|------------------|---------------|-------------------|
| GAP-H-001 | H-009 | BC-2.3.035 | `tests/issue_view_errors.rs:146` (partial) | Existing test covers the corrupt teams.json path; no story formally anchors it. Add to S-2.01 Out of Scope note — covered by pre-existing test. | v0.5 (no action needed; existing test is sufficient) |
| GAP-H-002 | H-010 | BC-2.2.018 / BC-2.2.019 | `tests/all_flag_behavior.rs:90` | 30-item truncation behavior is tested; no dedicated holdout story. Acceptable — test is stable. | v0.5 (no action needed) |
| GAP-H-003 | H-011 | BC-6.1.001 / BC-6.1.002 | `tests/migration_legacy.rs:94,146` | Migration + idempotency well covered by existing tests. No story anchor needed. | v0.5 (no action needed) |
| GAP-H-004 | H-012 | BC-1.6.042 / BC-X.3.005 | `tests/api_client.rs:100,184,219` | Auth dispatch on scope mismatch tested at unit level. Coverage adequate. | v0.5 (no action needed) |
| GAP-H-005 | H-015 | BC-2.2.020 | `tests/cli_smoke.rs:263` | Clap mutual exclusion tested. No story anchor needed. | v0.5 (no action needed) |
| GAP-H-006 | H-017 | BC-4.1.002 | `src/jql.rs:278-308` (unit tests) | JQL asset clause tested at unit level in the source file. No integration test gap. | v0.5 (no action needed) |
| GAP-H-007 | H-019 | BC-6.1.004 | `src/config.rs:759,769` (unit tests) | Profile name validation tested at unit level. Adequate coverage. | v0.5 (no action needed) |
| GAP-H-008 | H-025 | BC-6.2.014 | None found | Cache write atomicity (temp file + rename) has no test pin at activation HEAD. The behavior exists in `src/cache.rs` but is untested. Adding a test is safe but non-critical — atomic rename is well-established OS behavior. | v0.6 (low priority — document in S-2.06 or create S-4.NN if needed) |
| GAP-H-009 | H-026 | BC-7.3.002 | `tests/api_client.rs:310` | `extract_error_message` mixed-values path tested. Pre-existing coverage adequate; no story anchor needed. | v0.5 (no action needed) |
| GAP-H-010 | H-028 | BC-6.1.005 | None found | Hand-edited config with `foo:bar` profile key (config-file-load boundary) has no specific test. `JR_PROFILE` env var and `--profile` flag validation are tested (H-019), but the config-file load path rejecting invalid profile keys is a distinct code path not yet covered. | v0.6 (add to S-3.06 scope or create S-4.NN for config boundary tests) |

---

## Story Manifest

Complete mapping of every `story_id` to its absolute file path. Generated 2026-05-07; updated 2026-05-08 (S-3.10 added).
Total rows: 44 (matches `total_stories: 44` in frontmatter). Updated 2026-05-15 (S-340 added). Updated 2026-05-16 (S-345 added). Updated 2026-05-16 (S-346 added). Updated 2026-05-18 (issue-288-pr1..pr4 added). Updated 2026-05-18 (issue-288-pr3-scope dropped; 40→39). Updated 2026-05-19 (S-382 added; quick-dev F4; 39→40). Updated 2026-05-19 (S-382 completed PR #389 / b1c863e). Updated 2026-05-19 (S-383 added; F3; 40→41). Updated 2026-05-19 (S-383 completed PR #390 / 25f7211). Updated 2026-05-19 (S-392 added; infrastructure; 41→42). Updated 2026-05-20 (S-392 completed PR #393 / 0be2e3a). Updated 2026-05-20 (S-384 added; feature mode F3; 42→43). Updated 2026-05-20 (S-384 completed PR #394 / b36b291). Updated 2026-05-20 (S-385 added; feature mode F3; 43→43; count held at 43). Updated 2026-05-20 (corrected pre-existing off-by-one overcount: total_stories 44→43 to match actual 43 manifest rows; overcount accumulated across S-382..S-384 additions — PG-385-6). Updated 2026-05-20 (S-385 completed PR #395 / f7fc8c3; issue #385 F1–F7 CLOSED; cycle CLOSED). Updated 2026-05-20 (S-388 added; feature mode F3; 43→44).

### Wave 0

| story_id | wave | file_path |
|----------|------|-----------|
| S-0.01 | 0 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-0/S-0.01-fix-handle-open-oauth-instance-url.md |
| S-0.02 | 0 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-0/S-0.02-paginate-list-worklogs.md |
| S-0.03 | 0 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-0/S-0.03-multi-workspace-asset-hashmap-key.md |
| S-0.04 | 0 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-0/S-0.04-multi-profile-fields-active.md |
| S-0.05 | 0 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-0/S-0.05-jr-auth-header-cfg-test-gate.md |
| S-0.06 | 0 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-0/S-0.06-verbose-bodies-flag-and-pii-warning.md |
| S-0.07 | 0 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-0/S-0.07-formalize-h-new-auth-002-holdout.md |

### Wave 1

| story_id | wave | file_path |
|----------|------|-----------|
| S-1.01 | 1 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-1/S-1.01-pin-github-actions-shas.md |
| S-1.02 | 1 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-1/S-1.02-cargo-deny-supply-chain-audit.md |
| S-1.03 | 1 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-1/S-1.03-tracing-observability-wire-up.md |
| S-1.04 | 1 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-1/S-1.04-ci-job-timeouts.md |
| S-1.05 | 1 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-1/S-1.05-github-secret-scanning.md |
| S-1.06 | 1 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-1/S-1.06-oauth-flow-holdout-suite.md |
| S-1.07 | 1 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-1/S-1.07-rate-limit-holdout-suite.md |
| S-1.08 | 1 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-1/S-1.08-keychain-roundtrip-holdout.md |

### Wave 2

| story_id | wave | file_path |
|----------|------|-----------|
| S-2.01 | 2 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-2/S-2.01-bc-2-issue-read-holdout-suite.md |
| S-2.02 | 2 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-2/S-2.02-bc-3-issue-write-holdout-suite.md |
| S-2.03 | 2 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-2/S-2.03-bc-4-asset-enrichment-holdout-suite.md |
| S-2.04 | 2 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-2/S-2.04-bc-5-boards-sprints-holdout-suite.md |
| S-2.05 | 2 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-2/S-2.05-claude-md-documentation-update.md |
| S-2.06 | 2 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-2/S-2.06-worklog-duration-and-cmdb-cache-tuple.md |
| S-2.07 | 2 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-2/S-2.07-json-output-policy-and-test-naming.md |

### Wave 3

| story_id | wave | file_path |
|----------|------|-----------|
| S-3.01 | 3 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-3/S-3.01-refactor-auth-rs-shard-split.md |
| S-3.02 | 3 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-3/S-3.02-refactor-cli-assets-shard-split.md |
| S-3.03 | 3 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-3/S-3.03-refresh-oauth-token-investigation.md |
| S-3.04 | 3 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-3/S-3.04-multi-cloudid-disambiguation.md |
| S-3.05 | 3 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-3/S-3.05-asset-enrichment-concurrency-cap.md |
| S-3.06 | 3 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-3/S-3.06-spec-numeric-claim-checker.md |
| S-3.07 | 3 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-3/S-3.07-low-nfr-code-cleanup.md |
| S-3.08 | 3 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-3/S-3.08-low-nfr-document-as-is.md |
| S-3.09 | 3 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-3/S-3.09-pkce-decision-deferred.md |
| S-3.10 | 3 | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/wave-3/S-3.10-rewrite-format-roundtrip-proptest-delete-deprecated-parse-duration.md |

### Feature Followup

| story_id | wave | file_path |
|----------|------|-----------|
| S-340 | feature-followup | /Users/zious/Documents/GITHUB/jira-cli/.factory/code-delivery/issue-340/story.md |
| S-345 | feature-followup | /Users/zious/Documents/GITHUB/jira-cli/.factory/code-delivery/issue-345/story.md |
| S-346 | feature-followup | /Users/zious/Documents/GITHUB/jira-cli/.factory/code-delivery/issue-346/story.md |
| issue-288-pr1-api | cycle-3-wave-1 | /Users/zious/Documents/GITHUB/jira-cli/.factory/code-delivery/issue-288-pr1-api/story.md |
| issue-288-pr2-cli | cycle-3-wave-2 | /Users/zious/Documents/GITHUB/jira-cli/.factory/code-delivery/issue-288-pr2-cli/story.md |
| issue-288-pr4-dispatch | cycle-3-wave-3 | /Users/zious/Documents/GITHUB/jira-cli/.factory/code-delivery/issue-288-pr4-dispatch/story.md |
| S-382 | feature-followup (quick-dev F4) | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/S-382-error-insufficient-scope-refactor.md |
| S-383 | feature-followup (F3) | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/S-383-platform-inverse-warnings.md |
| S-392 | feature-followup (infrastructure) | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/S-392-cumulative-spec-count-guard.md |
| S-384 | feature-followup (feature mode F3) | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/S-384-jsm-401-auth-aware-hints.md |
| S-385 | feature-followup (feature mode F4 → F7) | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/S-385-jsm-input-validation-ux-polish.md |
| S-388 | feature-followup (feature mode F3) | /Users/zious/Documents/GITHUB/jira-cli/.factory/stories/S-388-cross-hierarchy-type-change-error-and-fake-endpoint-fix.md |

# Demo Evidence Report — S-604-3

**Story:** S-604-3 — `jr component delete` — disposition-required, snapshot-before-delete safety (DEC-279)
**Branch:** feature/S-604-3-component-delete-safety
**Date:** 2026-08-17
**Binary:** `jr` (debug build, `cargo build` on this branch)
**Recording tool:** VHS 0.11.0
**Mock server:** `mock-server.py` (this directory), a stdlib-only Python HTTP server on `127.0.0.1:19880` — serves `GET /rest/api/3/project/{key}/components`, `GET /rest/api/3/component/{id}`, `POST /rest/api/3/search/jql` (the BC-8.2.007 pre-delete snapshot), and `DELETE /rest/api/3/component/{id}`

---

## Safety note (read first)

`jr component delete` is **irreversible** — no trash/archive/undelete endpoint exists (research
§Q1.2). Per the recording brief's explicit safety constraint, **no command in this evidence set
was ever run against a live or real Jira instance.** Every recording below runs the real debug
`jr` binary against the local mock server exclusively, via the `JR_BASE_URL` debug-only seam
(CLAUDE.md "AI Agent Notes" — release binaries ignore this env var entirely). The "DELETE" calls
shown in these recordings only ever reach the throwaway in-memory Python fixtures in
`mock-server.py`; nothing in this evidence directory can mutate a real Jira project. All four
`.tape` sources and `mock-server.py` are included so every recording is independently
reproducible via `run-recordings.sh`.

---

## Coverage Summary

All 22 acceptance criteria are covered by the full `cargo test --test component_commands` suite
(**104/104 passing**, including every `test_bc_8_2_*`/`test_ec_8_2_*` function below — see
`tests/component_commands.rs`). The six ACs the recording brief called out as safety-critical
(AC-001, AC-005, AC-012, AC-013, AC-019, AC-020) additionally have **VHS recordings** driving the
real binary against the local mock, captured below.

| AC | Description | Evidence Type | Artifact |
|----|-------------|---------------|---------|
| AC-001 | Neither `--move-to` nor `--orphan` → exit 64, names BOTH flags, ZERO DELETE/snapshot calls | **VHS recording** + integration test | `AC-001-004-disposition-guard.gif/.webm` + `test_bc_8_2_001_component_delete_neither_flag_exits_64_zero_http` |
| AC-002 | `--move-to X --orphan` together → clap exit 2 (mutual exclusion) | Integration test (passing) | `test_bc_8_2_001_component_delete_both_flags_clap_exit_2` |
| AC-003 | NAME source, unresolvable, `--orphan` → exit 64 "not found" (Invariant 1 ordering), NOT the disposition guard | Integration test (passing) | `test_bc_8_2_001_component_delete_name_notfound_before_disposition_guard` |
| AC-004 | NUMERIC source, nonexistent, neither flag → disposition-guard message (not "not found"), zero HTTP | **VHS recording** + integration test | `AC-001-004-disposition-guard.gif/.webm` + `test_bc_8_2_001_component_delete_numeric_no_disposition_asymmetry` |
| AC-005 | `--move-to Frontend` → target resolves BEFORE DELETE; `DELETE .../component/{id}?moveIssuesTo=<targetId>` fires exactly once | **VHS recording** + integration test | `AC-005-020-move-to-success.gif/.webm` + `test_bc_8_2_002_component_delete_move_to_success_delete_after_resolution` |
| AC-006 | `--move-to Backend` where a same-named component exists in a different project → resolves ONLY within source's project | Integration test (passing) | `test_bc_8_2_003_component_delete_move_to_never_spans_projects` |
| AC-007 | Numeric `--move-to` target belonging to a different project → confirming GET catches mismatch → exit 64, zero DELETE | Integration test (passing) | `test_bc_8_2_002_component_delete_move_to_numeric_target_project_mismatch` |
| AC-008 | `--move-to` unknown/ambiguous target → exit 64 via §8.4 messages, zero DELETE | Integration test (passing) | `test_bc_8_2_004_component_delete_move_to_unknown_ambiguous_zero_delete` |
| AC-009 | Self-move guard (name AND numeric self-reference, ID equality) → exit 64, zero DELETE | Integration test (passing) | `test_bc_8_2_005_component_delete_self_move_guard_name_and_numeric` |
| AC-010 | Numeric SOURCE project mismatch under `--move-to` → exit 64 pre-flight, zero HTTP beyond the one confirming GET | Integration test (passing) | `test_bc_8_2_002_component_delete_numeric_source_project_mismatch_move_to` |
| AC-011 | Numeric SOURCE project mismatch under `--orphan` (P4-broadened) → identical pre-flight exit 64 | Integration test (passing) | `test_bc_8_2_002_component_delete_numeric_source_project_mismatch_orphan` |
| AC-012 | `--orphan` interactive (TTY): prompt names component + real count; decline → exit 0 zero DELETE; confirm → proceeds | **VHS recording** + integration test | `AC-012-013-orphan-gate.gif/.webm` + `test_bc_8_2_006_component_delete_orphan_interactive_prompt_decline_and_confirm` |
| AC-013 | Non-interactive `--orphan` without `--yes` → exit 64 with the REAL affected-issue count; `--yes` proceeds without a prompt | **VHS recording** + integration test | `AC-012-013-orphan-gate.gif/.webm` + `test_bc_8_2_006_component_delete_orphan_noninteractive_requires_yes_real_count` |
| AC-014 | `--move-to` NEVER shows a confirmation prompt or requires `--yes`, regardless of TTY/`--no-input` state | Integration test (passing) | `test_bc_8_2_006_component_delete_move_to_never_prompts` |
| AC-015 | `--orphan` on a component with ZERO affected issues → prompt still fires, shows `0 issue(s)` | Integration test (passing) | `test_bc_8_2_006_component_delete_orphan_zero_affected_issues_still_prompts` |
| AC-016 | Snapshot search fires exactly once, only after a disposition is guard-cleared | Integration test (passing) | `test_bc_8_2_007_component_delete_snapshot_fires_only_after_disposition_cleared` |
| AC-017 | Composed snapshot JQL is ALWAYS `component = <resolvedId> ORDER BY key ASC` — the resolved numeric id, never a bare name | Integration test (passing) | `test_bc_8_2_007_component_delete_snapshot_jql_uses_resolved_id_not_name` |
| AC-018 | Multi-page (`nextPageToken`) snapshot → ALL pages fetched; count/keys reflect the full result | Integration test (passing) | `test_bc_8_2_007_component_delete_snapshot_paginates_to_completion` |
| AC-019 | JRACLOUD-95368 pagination-drift (`has_more=true`) → exit 1, zero DELETE, fail-closed message; genuine 5xx fetch error → identical fail-closed outcome | **VHS recording** + integration test | `AC-019-snapshot-drift-fail-closed.gif/.webm` + `test_bc_8_2_007_component_delete_snapshot_drift_and_fetch_error_fail_closed` |
| AC-020 | `--output json` success shape EXACTLY `{deleted, movedIssuesTo, affectedIssueCount, affectedIssues}`, matching the snapshot, for both `--move-to` and `--orphan` | **VHS recording** + integration test | `AC-005-020-move-to-success.gif/.webm` + `test_bc_8_2_008_component_delete_success_json_shape_matches_snapshot` |
| AC-021 | Idempotency taxonomy: resolver-layer not-found → exit 64 (never idempotent-skip); DELETE races to 404 after successful resolution → exit 1, distinguishable by exit code (VP-COMPONENT-024) | Integration test (passing) | `test_bc_8_2_008_component_delete_resolver_notfound_vs_delete_race_exit_code_divergence` |
| AC-022 | `--move-to` target deleted by a concurrent actor between resolution and DELETE → the DELETE itself 404s → exit 1 | Integration test (passing) | `test_bc_8_2_008_component_delete_move_to_target_race_404_exits_1` |

**Total:** 22/22 ACs covered — 4 VHS recordings (covering the 6 safety-critical ACs called out
in the recording brief) + 104/104 passing integration tests covering all 22 ACs (including
edge cases EC-8.2.001-1/2, EC-8.2.006-1/3/4/5, and the global-`--project`-flag propagation
coverage pin).

---

## VHS Recordings

All four recordings share one mock-server session (`mock-server.py`, port 19880) with these
fixture components in project `FOO`:

| Name | ID | Purpose |
|------|----|---------|
| Backend | 10001 | `--move-to` source; snapshot returns 2 issues (FOO-101, FOO-102) |
| Frontend | 10002 | `--move-to` target |
| DriftComp | 10003 | Snapshot always returns a repeating `nextPageToken` — simulates JRACLOUD-95368 drift. DELETE on this id returns a loud 500 in the mock, proving in the recording that it is never reached. |
| Orphaned | 10004 | `--orphan` source; snapshot returns 5 issues (FOO-301..FOO-305) |

### AC-001-004-disposition-guard — the disposition-required guard (BC-8.2.001)

**Files:** `AC-001-004-disposition-guard.gif`, `.webm`, `.tape`

1. `jr component delete Backend --project FOO --no-input` (neither `--move-to` nor `--orphan`,
   NAME source) → exit 64, stderr: *"Refusing to delete: no disposition supplied for this
   component's issues. Supply --move-to <NAME|ID> to move them to another component, or
   --orphan to remove the component with no replacement."* — names both flags, zero
   DELETE/snapshot calls (AC-001).
2. `jr component delete 999999999 --no-input` (numeric, nonexistent, neither flag) → the
   IDENTICAL disposition-guard message, exit 64 — the numeric/no-disposition asymmetry: no HTTP
   call is reachable in this path to discover non-existence, so the guard fires first rather
   than a "not found" message (AC-004).

### AC-005-020-move-to-success — `--move-to` success + `--output json` shape (BC-8.2.002, BC-8.2.008)

**Files:** `AC-005-020-move-to-success.gif`, `.webm`, `.tape`

1. `jr component delete Backend --project FOO --move-to Frontend --no-input` → resolves
   Frontend (10002) as the target, snapshots the 2 affected issues, then
   `DELETE /rest/api/3/component/10001?moveIssuesTo=10002` → *"Deleted component "Backend" (id
   10001) — 2 affected issue(s), moved to component 10002."*, exit 0 (AC-005).
2. The same command with `--output json` → stdout is EXACTLY
   `{"affectedIssueCount": 2, "affectedIssues": ["FOO-101", "FOO-102"], "deleted": "10001",
   "movedIssuesTo": "10002"}`, matching the snapshot precisely, exit 0 (AC-020).

### AC-012-013-orphan-gate — the `--orphan` confirmation gate (BC-8.2.006)

**Files:** `AC-012-013-orphan-gate.gif`, `.webm`, `.tape`

1. Interactive (TTY), decline: `jr component delete Orphaned --project FOO --orphan` prompts
   *"Delete component 'Orphaned' and remove it from 5 issue(s)? This cannot be undone. [y/N]"*
   — the real, snapshot-derived count (5), not a placeholder. Typing `N` → exit 0, zero DELETE
   (AC-012).
2. Interactive, confirm: same command, typing `y` → *"Deleted component "Orphaned" (id 10004) —
   5 affected issue(s), orphaned (no replacement)."*, exit 0 (AC-012).
3. Non-interactive without `--yes`: `jr component delete Orphaned --project FOO --orphan
   --no-input` → exit 64, *"--orphan requires --yes when running non-interactively. This
   permanently removes the component from 5 issue(s) with no replacement."* — again the real
   count, zero DELETE (AC-013).
4. Non-interactive with `--yes`: `jr component delete Orphaned --project FOO --orphan --yes
   --no-input` → proceeds directly without any prompt, exit 0 (AC-013).

### AC-019-snapshot-drift-fail-closed — fail-closed on pagination drift (BC-8.2.007)

**Files:** `AC-019-snapshot-drift-fail-closed.gif`, `.webm`, `.tape`

`jr component delete DriftComp --project FOO --orphan --yes --no-input` — the mock server always
returns the same `nextPageToken` for this component's snapshot query, reproducing the
JRACLOUD-95368 live-data-drift condition. The existing `search_issue_keys` anti-loop guard
detects the repeated cursor and returns `has_more=true`; `component delete` treats this as a
fresh failure rather than a partial success: *"[jr] WARNING: Atlassian /rest/api/3/search/jql
returned the same nextPageToken twice — aborting pagination to prevent an infinite loop. …"*
followed by *"Error: could not reliably enumerate affected issues — aborting delete"*, exit 1.
The mock's DELETE handler for this component id is wired to return a loud 500 if ever reached —
proving in this recording that it never fires (zero DELETE, fail-closed).

---

## Reproducing these recordings

```bash
cd .worktrees/S-604-3
bash docs/demo-evidence/S-604-3/run-recordings.sh
```

This builds the debug binary, starts `mock-server.py` on `127.0.0.1:19880`, records all four
tapes with VHS, and tears the mock server down. No network access or real Jira credentials are
required or used.

---

## Full Test Suite

```
cargo test --test component_commands
```

```
test result: ok. 104 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.80s
```

Every `test_bc_8_2_*` and `test_ec_8_2_*` function cited in the Coverage Summary table above is
part of this run.

---

## Deviation from the requested output location

The recording brief for this story requested placing artifacts under
`.factory/demos/S-604-3/`. Per the Demo Recorder operating contract, evidence for a story
ALWAYS goes to `docs/demo-evidence/<STORY-ID>/` (committed to the feature branch, visible in the
PR diff) — the same location used by the two prior stories in this bundle,
`docs/demo-evidence/S-604-1/` and `docs/demo-evidence/S-604-2/`. This report and all recordings
were placed there instead, for consistency with that convention and with sibling-story evidence.

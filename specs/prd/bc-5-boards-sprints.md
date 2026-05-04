---
context: bc-5
title: "Boards & Sprints"
total_bcs: 35   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 17   # count of `#### BC-` headings in this file
last_updated: 2026-05-04
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/bc-05-boards-sprints.md
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.5
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.7 (BC-1138)
---

# BC-5 — Boards & Sprints

35 behavioral contracts across 4 subdomains: Board commands (5.1), Sprint commands (5.2),
Team column parity (5.3), API layer (5.4).

---

## Subdomains

### 5.1 Board Commands

#### BC-5.1.001: `client.list_boards(project, type)` GETs `/rest/agile/1.0/board` with query params

**Confidence**: HIGH
**Source**: `tests/board_commands.rs:111-`; `tests/sprint_commands.rs:23-39`
**Subject**: Boards & Sprints
**Behavior**: Boards filtered by `projectKeyOrId=PROJ` + `type=scrum|kanban`.
**Trace**: Pass 3 BC-401

---

#### BC-5.1.002: `board view --limit --all` clap conflict

**Confidence**: HIGH
**Source**: `tests/board_commands.rs:96-106`; `tests/cli_smoke.rs:300-307`
**Trace**: Pass 3 BC-408

---

#### BC-5.1.003: Auto-resolve board: list scrum boards for project, pick first

**Confidence**: HIGH
**Source**: `tests/sprint_commands.rs:23-61`
**Subject**: Boards & Sprints
**Behavior**: When no board_id configured, auto-resolves by listing boards and picking the first matching.
**Trace**: Pass 3 BC-410

---

#### BC-5.1.004: `client.get_sprint_issues(sprintId, jql, limit, fields)` with `limit=Some(3)` returns 3 issues, `has_more=true`

**Confidence**: HIGH
**Source**: `tests/board_commands.rs:23-71`
**Trace**: Pass 3 BC-409

---

### 5.2 Sprint Commands

#### BC-5.2.001: `sprint list/current` errors on kanban boards with `"Sprint commands are only available for scrum boards"`

**Confidence**: HIGH
**Source**: `src/cli/sprint.rs:79-86`; inline tests
**Subject**: Boards & Sprints
**Behavior**: `if board_type != "scrum"` → bail with the literal message. Hard error (not silent degrade).
**Trace**: Pass 3 BC-402

---

#### BC-5.2.002: `sprint add --sprint ID` and `sprint add --current` are mutually exclusive (clap)

**Confidence**: HIGH
**Source**: `tests/cli_smoke.rs:116-123`
**Trace**: Pass 3 BC-403

---

#### BC-5.2.003: `sprint add` requires `--sprint` or `--current`

**Confidence**: HIGH
**Source**: `tests/cli_smoke.rs:126-133`
**Trace**: Pass 3 BC-404

---

#### BC-5.2.004: `MAX_SPRINT_ISSUES = 50` caps `sprint add` and `sprint remove`

**Confidence**: MEDIUM
**Source**: `src/cli/sprint.rs:35-41, 55-61, 107`; 6 inline tests
**Subject**: Boards & Sprints
**Behavior**: At most 50 issues processed per sprint operation.
**Trace**: Pass 3 BC-405

---

#### BC-5.2.005: `sprint current` truncates to 30 by default; with `--all` returns full set; under-limit no hint

**Confidence**: HIGH
**Source**: `tests/sprint_commands.rs:63-180`
**Subject**: Boards & Sprints
**Behavior**: 35 issues + default → 30 in stdout + stderr `"Showing 30 results"`. With `--all` → 35 + no hint. With 10 issues → no hint.
**Trace**: Pass 3 BC-406

---

#### BC-5.2.006: `sprint current --all --limit N` clap conflict

**Confidence**: HIGH
**Source**: `tests/cli_smoke.rs:310-317`
**Trace**: Pass 3 BC-407

---

#### BC-5.2.007: Sprint JSON output snapshot: `sprint_add_response(100, &["TEST-1", "TEST-2"])` → `{"added": true, "issues": ["TEST-1", "TEST-2"], "sprint_id": 100}`

**Confidence**: HIGH
**Source**: `src/cli/snapshots/jr__cli__sprint__tests__sprint_add_response.snap`
**Behavior**: 3 keys: `sprint_id` (snake_case), `issues` (array), `added` (bool). Sprint ID included on add.
**Trace**: Pass 3 BC-1113 (R4)

---

#### BC-5.2.008: Sprint JSON output: `sprint_remove_response(&["TEST-1", "TEST-2"])` → `{"issues": [...], "removed": true}` (NO sprint_id)

**Confidence**: HIGH
**Source**: `src/cli/snapshots/jr__cli__sprint__tests__sprint_remove_response.snap`
**Behavior**: 2 keys only. Asymmetric with add — remove is sprint-agnostic.
**Trace**: Pass 3 BC-1114 (R4)

---

### 5.3 Team Column Parity (7 contracts)

#### BC-5.3.001: Team column appears IFF `team_field_id` configured AND at least one issue has populated team UUID

**Confidence**: HIGH
**Source**: `tests/team_column_parity.rs:124, 181` (BC-1138a/c)
**Subject**: Boards & Sprints
**Behavior**: Column gating is conjunctive — both conditions required. Affects `jr sprint current` and `jr board view`.
**Trace**: Pass 3 BC-1138a (R4)

---

#### BC-5.3.002: Team column omitted when `team_field_id` not configured OR no issue has team UUID

**Confidence**: HIGH
**Source**: `tests/team_column_parity.rs:220, 284` (BC-1138b/d)
**Trace**: Pass 3 BC-1138b (R4)

---

#### BC-5.3.003: Team column shows `"UUID (name not cached — run 'jr team list --refresh')"` when cache is stale

**Confidence**: HIGH
**Source**: `tests/team_column_parity.rs:341` (BC-1138e)
**Trace**: Pass 3 BC-1138e (R4)

---

#### BC-5.3.004: `--output json` preserves team UUID without resolution (no cache lookup)

**Confidence**: HIGH
**Source**: `tests/team_column_parity.rs:380` (BC-1138f)
**Trace**: Pass 3 BC-1138f (R4)

---

### 5.4 API Layer

#### BC-5.4.001: `IssueFields::team_id` accepts string-UUID; rejects non-string id (object form without `id` key)

**Confidence**: HIGH
**Source**: `src/types/jira/issue.rs:101-131`; 9 tests in `issue.rs::tests`; `tests/team_object_shape.rs`
**Subject**: Boards & Sprints
**Behavior**: String UUID → deserialized. Object `{id: "<uuid>"}` → deserialized (object form). Non-string id without proper structure → `None` or Err.
**Trace**: Pass 3 BC-606

---

## Key Invariants

- `MAX_SPRINT_ISSUES = 50`: hard cap, not configurable
- Scrum-only check: `sprint` commands hard-error on kanban; `issue list` silently degrades (asymmetry documented in Pass 8 §2.5)
- Default limit = 30 (`DEFAULT_LIMIT`); with `--all` → no cap
- Truncation hint emitted to stderr (NOT stdout)
- `--all` suppresses truncation hint

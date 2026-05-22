---
document_type: story
story_id: "S-398"
title: "issue edit / issue create — changed-fields echo on success (closes #398)"
wave: feature-followup
status: ready
intent: enhancement
feature_type: backend
scope: standard
issue: 398
points: 5
priority: medium
tdd_mode: strict
estimated_effort: medium
depends_on: []  # builds on develop HEAD; no dependency on open stories
bc_anchors:
  - BC-3.4.012
  - BC-3.4.013
  - BC-3.4.014
verification_properties:
  - VP-398-001
  - VP-398-002
  - VP-398-003
  - VP-398-004
  - VP-398-005
  - VP-398-006
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: ".factory/phase-f2-spec-evolution/prd-delta-398.md"
implementation_strategy: tdd
module_criticality: HIGH  # src/cli/issue/create.rs — core edit/create command success path; shared with all issue-write BCs
files_modified:
  - src/cli/issue/create.rs       # MODIFIED — handle_edit: add BTreeMap changed_fields, populate per flag, emit echo lines and extend edit_response call; handle_create: add BTreeMap create_echo, populate per flag, emit echo lines after POST 201; IMP-2: replace HashMap import with BTreeMap (remove HashMap if no longer used)
  - src/cli/issue/helpers.rs      # MODIFIED — resolve_team_field signature change: (String, String) -> (String, String, String); add team_name as 3rd element across all 5 return paths; UUID-bypass: 3rd element = raw UUID
  - src/cli/issue/json_output.rs  # MODIFIED — edit_response signature change: fn edit_response(key: &str, changed_fields: &BTreeMap<String, String>) -> Value; update test_edit to pass non-empty BTreeMap; add test_edit_response_empty_changed_fields; regenerate insta snapshot
  - src/cli/issue/list.rs         # MODIFIED — handle_list resolve_team_field call: 2-tuple destructure -> 3-tuple (field_id, team_uuid, _resolved_team_name); update any explicit Option<(String,String)> type annotation to Option<(String,String,String)>
  - CLAUDE.md                     # MODIFIED — Gotchas section: add description-echo-asymmetry entry (pinned verbatim from prd-delta-398.md §6)
test_files:
  - tests/issue_edit_echo.rs      # NEW — integration tests for edit table-mode echo (AC-001..AC-005, AC-007, AC-008, AC-010, AC-013, AC-021; EC-3.4.012-12, EC-3.4.013-10)
  - tests/issue_create_echo.rs    # NEW — integration tests for create table-mode echo (AC-006, AC-009, AC-011, AC-012, AC-014)
breaking_change: false
# BC status: BC-3.4.012/013/014 produced in F2 (2026-05-21, prd-delta-398.md CONVERGED).
# All BCs sealed; do NOT re-edit unless adversary finds implementation-BC discrepancy.
# F3 story produced after F2 convergence confirmed.
---

# S-398 — Issue Edit / Issue Create: Changed-Fields Echo on Success

## Source of Truth

F2 PRD delta: `.factory/phase-f2-spec-evolution/prd-delta-398.md` (CONVERGED, 2026-05-21).
F2 verification delta: `.factory/phase-f2-spec-evolution/verification-delta-398.md` (CONVERGED, 2026-05-21).
BC bodies: `.factory/specs/prd/bc-3-issue-write.md` §BC-3.4.012, §BC-3.4.013, §BC-3.4.014.

**Authoritative BC count: THREE new BCs (BC-3.4.012, BC-3.4.013, BC-3.4.014).** Spec version bump:
`bc-3 total_bcs 97→100; definitional_count 68→71; BC-INDEX total_bcs 577→580`.

## Problem Statement

`jr issue edit KEY` currently prints only `Updated FOO-123` on success. When `--team` is used,
the partial-match resolution is silent — the user has no way to confirm which team was actually
set. Similarly, `jr issue create` only prints `Created issue FOO-123` with no field summary.

This story adds a "changed fields" echo to:
- `issue edit` (single-key path): table mode echoes `  <field> → <value>` lines to stderr;
  JSON mode gains a `changed_fields` BTreeMap in the response object.
- `issue create` (platform path, not JSM `--request-type`): table mode echoes all set fields
  between the "Created issue" confirmation and the browse URL.

The `resolve_team_field` return type is also extended from a 2-tuple to a 3-tuple
`(field_id, team_id, team_name)` — required by all three call sites.

## Behavioral Contracts

| BC ID | File | Title | Clause(s) |
|-------|------|-------|-----------|
| BC-3.4.012 | `bc-3-issue-write.md` | `issue edit` table-mode success echo — per-field `field → value` lines to stderr (alphabetical, resolved team name, `(updated)` description marker) | preconditions, postconditions, invariants |
| BC-3.4.013 | `bc-3-issue-write.md` | `issue edit` JSON-mode success echo — `changed_fields` BTreeMap added to `edit_response`; raw description string; `updated: true` retained | preconditions, postconditions, invariants |
| BC-3.4.014 | `bc-3-issue-write.md` | `issue create` table-mode all-fields echo — mirrors BC-3.4.012; fields alphabetical between "Created issue" and browse URL; JSON path unchanged | preconditions, postconditions, invariants |

## Acceptance Criteria

### AC-001 — `issue edit` single-key table mode echoes all changed fields to stderr
(traces to BC-3.4.012 postconditions: one `  <field> → <value>` line per changed field)

When `jr issue edit KEY --summary "New title" --priority High` succeeds (PUT 204, table mode),
stderr MUST contain:
- `Updated KEY` (existing confirmation line, unchanged)
- `  priority → High`
- `  summary → New title`

Lines appear in alphabetical field-key order (`priority` before `summary`). Stdout MUST be empty.
Exit code MUST be 0.

Field key names are the literal lowercase identifiers from the BC: `summary`, `issue_type`,
`priority`, `parent`, `points`, `team`, `description`. The issue-type key is `issue_type`
(NOT `type`, NOT `issuetype`).

### AC-002 — `issue edit` table mode echoes resolved team display name (not UUID or query string)
(traces to BC-3.4.012 postcondition: team echo is RESOLVED display name; VP-398-001)

When `jr issue edit KEY --team plat` resolves to team `"Platform Core"` (id `"some-uuid-string"`),
stderr MUST contain `  team → Platform Core`. Stderr MUST NOT contain `"some-uuid-string"`.
Stderr MUST NOT contain the raw query string `"plat"`.

UUID-bypass exception: when the caller passes `--team <raw-36-char-uuid>`, the echo value IS
the UUID (no lookup occurred). Assert UUID is echoed as-is — this is correct behavior, not a bug.

Unit test `is_team_uuid_rejects_wrong_length` at `src/cli/issue/helpers.rs` (~line 617) already
covers the 35-char boundary negative case — cite it, no new test required for that assertion.

### AC-003 — `issue edit` table mode echoes `(updated)` for description — never the content
(traces to BC-3.4.012 postcondition: description echo is marker only; VP-398-002)

When `jr issue edit KEY --description "Some longer description text"` succeeds in table mode:
- Stderr MUST contain the substring `description → (updated)`.
- Stderr MUST NOT contain `"Some longer description text"`.
- Stderr MUST NOT contain any truncated preview of the content.
- Stdout MUST be empty. Exit code MUST be 0.

### AC-004 — `issue edit` table mode echoes `(cleared)` for `--no-parent` and `--no-points`
(traces to BC-3.4.012 postcondition: cleared-field marker; VP-398-004)

When `jr issue edit KEY --no-parent` succeeds in table mode:
- Stderr MUST contain `  parent → (cleared)`.
- Stderr MUST NOT contain any field key named `no_parent`.

When `jr issue edit KEY --no-points` succeeds in table mode:
- Stderr MUST contain `  points → (cleared)`.
- Stderr MUST NOT contain any field key named `no_points`.

### AC-005 — `issue edit` single-key JSON mode includes `changed_fields` BTreeMap in response
(traces to BC-3.4.013 postconditions: `changed_fields` object present; VP-398-003)

When `jr issue edit KEY --summary "New title" --output json` succeeds:
- Stdout JSON MUST parse cleanly.
- `output["updated"]` MUST be `true` (backward-compat retention).
- `output["changed_fields"]` MUST be present and MUST contain `{"summary": "New title"}`.
- Stderr MUST be empty. Exit code MUST be 0.

`changed_fields` is a `BTreeMap<String, String>` — all values are JSON strings including numeric
fields. Key order within `changed_fields` is alphabetical (guaranteed by `BTreeMap`).

Pinned snapshot body for the updated `test_edit` insta snapshot
(`jr__cli__issue__json_output__tests__edit.snap`):
```json
{
  "changed_fields": {
    "summary": "New title"
  },
  "key": "TEST-1",
  "updated": true
}
```
Top-level key order (`changed_fields`, `key`, `updated`) is alphabetical because `serde_json::Map`
serializes keys in alphabetical order by default (the `preserve_order` feature is NOT enabled —
confirmed in `Cargo.toml`).

### AC-006 — `issue create` table mode echoes all set fields between confirmation and browse URL
(traces to BC-3.4.014 postconditions: per-field echo lines; alphabetical order; VP-398-005 part B)

When `jr issue create --summary "Fix login bug" --type Task --priority High --team "plat"`
(where `"plat"` resolves to `"Platform Core"`) succeeds in table mode, stderr MUST contain:
1. `Created issue KEY` (confirmation line)
2. `  issue_type → Task`
3. `  priority → High`
4. `  summary → Fix login bug`
5. `  team → Platform Core`
6. Browse URL

Lines appear in this order: confirmation, then field echo lines (alphabetical), then browse URL.
No `  description` line (description not supplied). Stdout MUST be empty. Exit code MUST be 0.

Field ordering assertions: `  issue_type → Task` appears before `  priority → High` (i < p);
`  priority → High` appears before `  summary → Fix login bug` (p < s);
`  summary → Fix login bug` appears before `  team → Platform Core` (s < t).

### AC-007 — `issue edit` JSON mode carries raw description string (not `(updated)` marker)
(traces to BC-3.4.013 postcondition: `changed_fields.description` is raw user input; VP-398-002)

When `jr issue edit KEY --description "Some longer description text" --output json` succeeds:
- `output["changed_fields"]["description"]` MUST be exactly `"Some longer description text"`.
- `output["changed_fields"]["description"]` MUST NOT be `"(updated)"`.
- `output["changed_fields"]["description"]` MUST NOT contain raw ADF JSON structure.
- Stderr MUST be empty.

**Deliberate asymmetry with AC-003:** table mode shows `(updated)`, JSON mode shows raw input.
This is a LOCKED DECISION — do NOT "fix" them to match. See CLAUDE.md Gotchas entry (AC-016).

Sub-case — stdin trailing-newline preserved (VP-398-002 sub-case):
When `printf 'My description\n' | jr issue edit KEY --description-stdin --output json`,
`output["changed_fields"]["description"]` MUST be exactly `"My description\n"` (trailing `\n`
preserved — no silent strip before passing to `markdown_to_adf`).

### AC-008 — `issue edit` JSON `changed_fields` uses single `parent`/`points` keys for cleared fields
(traces to BC-3.4.013 postcondition: cleared-field single-key model; VP-398-004)

When `jr issue edit KEY --no-parent --output json` succeeds:
- `output["changed_fields"]` MUST contain exactly `{"parent": "(cleared)"}`.
- `output["changed_fields"]` MUST NOT contain a key named `"no_parent"`.
- `output["changed_fields"]` MUST contain exactly one key.

When `jr issue edit KEY --no-points --output json` succeeds:
- `output["changed_fields"]` MUST contain `{"points": "(cleared)"}`.
- `output["changed_fields"]` MUST NOT contain a key named `"no_points"`.

### AC-009 — `issue create` table mode echoes `(updated)` for description — never the content
(traces to BC-3.4.014 postcondition: description echo is marker on create path; VP-398-006)

When `jr issue create --summary "X" --type Task --description "Some longer description text"`
succeeds in table mode:
- Stderr MUST contain the substring `description → (updated)`.
- Stderr MUST NOT contain `"Some longer description text"`.
- Stderr MUST NOT contain any truncated preview.
- Stdout MUST be empty. Exit code MUST be 0.

### AC-010 — `issue edit` echo does NOT fire on dry-run or when no field flags are supplied
(traces to BC-3.4.012 postcondition: echo fires only after PUT; no-op guard)

`jr issue edit KEY --dry-run --summary "X"` MUST NOT emit any `  field → value` line to stderr.
`jr issue edit KEY` (no field flags) MUST exit 1 before PUT; no echo fires.
Both: stdout and stderr contain no `→` field-echo lines.

### AC-011 — `issue create` label field echoed as comma-space-separated list in command-line order
(traces to BC-3.4.014 postcondition: label echo format)

When `jr issue create --summary "S" --type Task --label bug --label urgent` succeeds in table mode,
stderr MUST contain `  label → bug, urgent` (comma-space separated, command-line order).

### AC-012 — `issue create` assignee echoed as display name (`--to`) or account ID (`--account-id`)
(traces to BC-3.4.014 postcondition: assignee echo; `resolve_assignee_by_project` second return element)

`--to` path: `resolve_assignee_by_project` returns `(acct_id, display_name)`. The second element
MUST be bound (no underscore prefix) and inserted as `"assignee"` in the echo map. Stderr MUST
contain `  assignee → <display_name>`.

`--account-id` path: the raw account ID string is inserted. Stderr MUST contain
`  assignee → <account_id_value>`.

### AC-013 — `issue edit` echo is EXCLUDED for bulk/multi-key paths and `--label` (bulk label route)
(traces to BC-3.4.012 scope: single-key path only; `handle_edit_bulk_labels` exclusion)

`jr issue edit KEY1 KEY2 --summary "X"` (multi-key, bulk path) MUST NOT emit any `  field → value`
echo lines.

`jr issue edit KEY --label foo` routes through `handle_edit_bulk_labels` — MUST NOT emit a `label`
key echo line (even on single-key input). The `label` key exclusion applies to edit only; create
path DOES echo labels (see AC-011).

### AC-014 — `issue create` with `--request-type` (JSM path) is EXCLUDED from echo
(traces to BC-3.4.014 scope: platform path only; JSM `--request-type` dispatch fork)

When `jr issue create --request-type "IT Help" --summary "S"` is run, `handle_jsm_create` handles
the request. The `create_echo` BTreeMap MUST NOT be built or emitted on the JSM path. Stderr
MUST NOT contain any `  field → value` echo lines from the JSM dispatch.

### AC-015 — `issue create` JSON output path is UNCHANGED
(traces to BC-3.4.014 postcondition: create JSON path unchanged; no `changed_fields` added)

When `jr issue create --summary "X" --type Task --output json` succeeds, stdout JSON MUST be the
full issue object from the follow-up GET (existing behavior, unchanged). The JSON MUST NOT contain
a `"changed_fields"` key. No behavioral change to the JSON output path.

### AC-016 — CLAUDE.md Gotchas section gains the pinned description-echo-asymmetry entry verbatim
(traces to BC-3.4.013 invariant: asymmetry is LOCKED and documented; prd-delta-398.md §6)

The following entry MUST be added to the CLAUDE.md Gotchas section, byte-for-byte:

```
- **`issue edit` description echo asymmetry (issue #398):** Table/human output echoes
  `description → (updated)` — a marker, never the content. JSON `changed_fields.description`
  carries the **raw user-supplied input string** from `--description` / `--description-stdin`,
  not `"(updated)"` and not an ADF→text round-trip. The two channels intentionally differ:
  the human channel optimizes for scannability; the machine channel must be lossless. Do NOT
  "fix" them to match — this asymmetry is load-bearing. Tested by VP-398-002
  (`test_BC_3_4_012_description_echo_is_updated_marker_not_content` and
  `test_BC_3_4_013_description_echo_is_raw_input_string_not_marker`).
```

This AC MUST NOT be closed without the Gotcha entry present in CLAUDE.md.

### AC-017 — `resolve_team_field` returns 3-tuple; all call sites updated
(traces to BC-3.4.012 and BC-3.4.014: team name used in echo; prd-delta-398.md §5)

`resolve_team_field` in `src/cli/issue/helpers.rs` MUST return
`Result<(String, String, String)>` where the third element is the team display name per the
5-path enumeration in prd-delta-398.md §5:

| Branch | team_id (2nd) | team_name (3rd) |
|--------|--------------|-----------------|
| UUID-bypass | raw UUID | raw UUID (same value) |
| Exact | teams[idx].id | matched_name (MatchResult::Exact) |
| ExactMultiple | duplicates[selection].id | duplicates[selection].name (stored casing) |
| Ambiguous | teams[idx].id | teams[idx].name (stored casing) |
| None | — (Err returned) | — (Err returned) |

Three call sites MUST be updated:
1. `handle_edit` (create.rs ~line 794): `(field_id, team_id, resolved_team_name)` — use `resolved_team_name` in `changed_fields`.
2. `handle_create` (create.rs ~line 187): `(field_id, team_id, resolved_team_name)` — use `resolved_team_name` in `create_echo`.
3. `handle_list` (list.rs ~line 167 closure): `(field_id, team_uuid, _resolved_team_name)` — underscore-prefixed; the JQL filter path does NOT echo team name.

`MatchResult::None(_)` error message contains the stable substring `No team matching`.
Tests MUST assert only this stable substring — never the full message literal.

### AC-018 — `edit_response` signature extended; snapshot regenerated; empty-map test added
(traces to BC-3.4.013 postcondition: `changed_fields` in edit JSON; VP-398-003)

`edit_response` in `src/cli/issue/json_output.rs` MUST have new signature:
`pub(crate) fn edit_response(key: &str, changed_fields: &BTreeMap<String, String>) -> Value`

- The existing `test_edit` unit test MUST be updated to pass a non-empty `BTreeMap`
  (e.g., `{"summary": "New title"}`). Insta snapshot file
  `jr__cli__issue__json_output__tests__edit.snap` MUST be regenerated. Filename is NOT renamed.
- A NEW test `test_edit_response_empty_changed_fields` MUST be added (new-test naming convention).
  It calls `edit_response` with an empty `BTreeMap<String, String>` and asserts:
  `output["updated"] == true` and `output["changed_fields"] == {}`. Does NOT use insta snapshot.

The sole production call site `create.rs:910` MUST be updated:
`json_output::edit_response(key)` → `json_output::edit_response(key, &changed_fields)`.

### AC-019 — `MatchResult::None` from `resolve_team_field` exits with code 64 on create path
(traces to BC-3.4.014 edge case: unresolvable team name; VP-398-005 part A)

When `jr issue create --summary "Test" --type Task --team <unresolvable_name> --no-input`
is run:
- Process exit code MUST be 64 (matching `JrError::UserError.exit_code()`).
- Stdout MUST be empty (no issue key, no JSON).
- Stderr MUST contain the stable substring `No team matching`.
- No POST to `/rest/api/3/issue` MUST be issued (assert via wiremock `expect(0)` pattern).

### AC-020 — `import BTreeMap` added to `create.rs`; `HashMap` import removed if unused
(traces to prd-delta-398.md §6 IMP-2: import hygiene)

`src/cli/issue/create.rs` MUST have `use std::collections::BTreeMap;`. If `HashMap` is no
longer used in `create.rs` after the change, its import MUST be removed. `#[allow(unused_imports)]`
is NOT acceptable — remove the import instead (CLAUDE.md zero-warning policy).

### AC-021 — Changed-fields echo is suppressed when PUT returns a non-204 error
(traces to BC-3.4.012 invariant 6 + BC-3.4.013 invariant 6)

When `jr issue edit KEY --summary "New title"` issues a PUT that returns a non-204 error response
(e.g. HTTP 400 Bad Request), the changed-fields echo MUST NOT fire:
- Stderr MUST NOT contain any `  field → value` echo lines (table mode).
- Stdout MUST NOT contain a `"changed_fields"` key in any JSON output (JSON mode).
- The constructed `changed_fields` BTreeMap is discarded; no partial echo is emitted.
- The error response (existing error-handling path) is surfaced to the user unchanged.

This invariant ensures the echo is strictly a post-success side-effect, never a pre-success
or on-error side-effect. Both table-mode and JSON-mode paths are covered.

## `changed_fields` Map Construction Reference

### In `handle_edit` (prd-delta-398.md §6)

New declaration at the top of `handle_edit` (alongside `fields` JSON):
```rust
let mut changed_fields: BTreeMap<String, String> = BTreeMap::new();
```

Canonical key → value mapping (populated in parallel with `fields` as each flag is resolved):

| Flag | Key | Value |
|------|-----|-------|
| `--summary` | `"summary"` | literal `--summary` value |
| `--type` | `"issue_type"` | literal `--type` value |
| `--priority` | `"priority"` | literal `--priority` value |
| `--parent KEY` | `"parent"` | issue key string |
| `--no-parent` | `"parent"` | `"(cleared)"` |
| `--points N` | `"points"` | `pts.to_string()` |
| `--no-points` | `"points"` | `"(cleared)"` |
| `--team` | `"team"` | `resolved_team_name` (3rd tuple element) |
| `--description`/`--description-stdin` | `"description"` | raw `desc_text` (table shows `"(updated)"`) |

`--label` is NOT inserted — routes through `handle_edit_bulk_labels`, excluded per BC-3.4.012.

### In `handle_create` (prd-delta-398.md §6)

New declaration after `--request-type` dispatch fork, before field resolution:
```rust
let mut create_echo: BTreeMap<String, String> = BTreeMap::new();
```

| Flag | Key | Value |
|------|-----|-------|
| `--summary` | `"summary"` | always inserted (required) |
| `--type` | `"issue_type"` | always inserted (required) |
| `--description`/`--description-stdin` | `"description"` | `"(updated)"` (when `desc_text.is_some()`) |
| `--priority` | `"priority"` | literal value (when `priority.is_some()`) |
| `--label` | `"label"` | `labels.join(", ")` (when `!labels.is_empty()`; comma-space join) |
| `--team` | `"team"` | `resolved_team_name` (3rd tuple element) |
| `--points` | `"points"` | `pts.to_string()` (when `points.is_some()`) |
| `--parent` | `"parent"` | issue key (when `parent.is_some()`) |
| `--to` | `"assignee"` | `display_name` (bind second element, not `_display_name`) |
| `--account-id` | `"assignee"` | raw account ID string |

`project` is NOT echoed (required but analogous to not echoing the issue key).
No `(cleared)` variants exist on create (`--no-points`/`--no-parent` are edit-only).

Emitted in `OutputFormat::Table` arm AFTER POST 201:
```rust
OutputFormat::Table => {
    output::print_success(&format!("Created issue {}", response.key));
    for (field, value) in &create_echo {
        eprintln!("  {} \u{2192} {}", field, value);
    }
    eprintln!("{}", browse_url);
}
```

## Required Test Deliverables Summary

| # | Test function name | File | Type | AC |
|---|-------------------|----|------|----|
| 1 | `test_BC_3_4_012_edit_table_echo_summary_and_priority` | `tests/issue_edit_echo.rs` | NEW integration | AC-001 |
| 2 | `test_BC_3_4_012_team_echo_is_resolved_name_not_uuid` | `tests/issue_edit_echo.rs` | NEW integration | AC-002 |
| 3 | `test_BC_3_4_012_description_echo_is_updated_marker_not_content` | `tests/issue_edit_echo.rs` | NEW integration | AC-003 |
| 4 | `test_BC_3_4_012_no_parent_table_echo_uses_parent_key` | `tests/issue_edit_echo.rs` | NEW integration | AC-004 |
| 5 | `test_BC_3_4_013_updated_true_present_with_summary_changed_fields` | `tests/issue_edit_echo.rs` | NEW integration | AC-005 |
| 6 | `test_BC_3_4_013_description_echo_is_raw_input_string_not_marker` | `tests/issue_edit_echo.rs` | NEW integration | AC-007 |
| 7 | `test_BC_3_4_013_description_stdin_trailing_newline_preserved_in_changed_fields` | `tests/issue_edit_echo.rs` | NEW integration | AC-007 |
| 8 | `test_BC_3_4_013_no_parent_key_is_parent_not_no_parent` | `tests/issue_edit_echo.rs` | NEW integration | AC-008 |
| 9 | `test_BC_3_4_013_no_points_key_is_points_not_no_points` | `tests/issue_edit_echo.rs` | NEW integration | AC-008 |
| 10 | `test_BC_3_4_012_edit_echo_does_not_fire_on_dry_run` | `tests/issue_edit_echo.rs` | NEW integration | AC-010 |
| 11 | `test_BC_3_4_012_edit_echo_excluded_for_bulk_multi_key` | `tests/issue_edit_echo.rs` | NEW integration | AC-013 |
| 12 | `test_BC_3_4_014_create_all_fields_echo_alphabetical_order` | `tests/issue_create_echo.rs` | NEW integration | AC-006 |
| 13 | `test_BC_3_4_014_create_team_echo_is_resolved_name_not_uuid` | `tests/issue_create_echo.rs` | NEW integration | AC-002 |
| 14 | `test_BC_3_4_014_create_description_echo_is_updated_marker` | `tests/issue_create_echo.rs` | NEW integration | AC-009 |
| 15 | `test_BC_3_4_014_create_label_echo_comma_space_joined` | `tests/issue_create_echo.rs` | NEW integration | AC-011 |
| 16 | `test_BC_3_4_014_create_assignee_echo_display_name` | `tests/issue_create_echo.rs` | NEW integration | AC-012 |
| 17 | `test_BC_3_4_014_create_unresolvable_team_no_input_exits_64` | `tests/issue_create_echo.rs` | NEW integration | AC-019 |
| 18 | `test_BC_3_4_014_create_json_output_unchanged_no_changed_fields_key` | `tests/issue_create_echo.rs` | NEW integration | AC-015 |
| 19 | `test_edit_response_empty_changed_fields` | `src/cli/issue/json_output.rs` | NEW unit | AC-018 |
| 20 | `test_edit` (existing, MODIFIED) | `src/cli/issue/json_output.rs` | MODIFIED unit + snapshot regen | AC-018 |
| 21 | `test_BC_3_4_012_empty_summary_echoes_empty_value` | `tests/issue_edit_echo.rs` | NEW integration | BC-3.4.012 EC-3.4.012-12 |
| 22 | `test_BC_3_4_013_empty_summary_in_changed_fields` | `tests/issue_edit_echo.rs` | NEW integration | BC-3.4.013 EC-3.4.013-10 |
| 23 | `test_BC_3_4_012_echo_suppressed_on_put_error` | `tests/issue_edit_echo.rs` | NEW integration (wiremock PUT 400) | AC-021 (BC-3.4.012 inv 6 + BC-3.4.013 inv 6) |

## Files NOT to Touch (Regression Baseline)

| File | Why Unchanged |
|------|--------------|
| `src/api/jira/issues.rs` | `edit_issue`/`get_issue` signatures unchanged |
| `src/api/jira/users.rs` | `resolve_assignee_by_project` signature unchanged |
| `src/api/client.rs` | No new HTTP layer change |
| `src/error.rs` | No new error variant |
| `src/cli/mod.rs` | No new CLI flags |
| `src/cli/issue/workflow.rs` | Unrelated command path |
| `src/cli/issue/create.rs` `handle_jsm_create` | JSM dispatch path; no echo (AC-014) |
| `src/cli/issue/create.rs` lines for `handle_edit_bulk_fields`/`handle_edit_bulk_labels` | Bulk paths unaffected; echo applies single-key only |
| `tests/issue_write_holdouts.rs` | Success paths unchanged; no regression |
| `tests/issue_commands.rs` | BC-3.3.x, BC-3.4.x success paths; pre-echo regressions must remain green |
| `tests/issue_create_jsm.rs` | JSM path; no echo on that path |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~8 k |
| F2 PRD delta (`prd-delta-398.md`) | ~10 k |
| F2 verification delta (`verification-delta-398.md`) | ~7 k |
| BC files (BC-3.4.012/013/014 sections in `bc-3-issue-write.md`) | ~8 k |
| `src/cli/issue/create.rs` (read `handle_edit` + `handle_create` vicinity + field-building blocks) | ~18 k |
| `src/cli/issue/helpers.rs` (read `resolve_team_field` function + surrounding context) | ~5 k |
| `src/cli/issue/json_output.rs` (read `edit_response` + `test_edit` unit test) | ~3 k |
| `src/cli/issue/list.rs` (read `handle_list` ~line 161 closure only) | ~3 k |
| `tests/common/` (fixture context for new integration tests) | ~4 k |
| Tool outputs + `cargo test` + `cargo clippy` output | ~8 k |
| **Total** | **~74 k** |

Well within a single-agent context window (~200 k). No split required.
LOC delta estimate: ~80 lines in `helpers.rs` (3-tuple return + 5-path enumeration); ~150 lines
in `create.rs` (two BTreeMap blocks + echo loops + `edit_response` call update + import swap);
~15 lines in `json_output.rs` (signature + new test); ~5 lines in `list.rs` (destructure update);
~1 CLAUDE.md entry (~8 lines); ~420 lines in `tests/issue_edit_echo.rs` (14 tests: 11 original + 2 empty-summary EC tests + 1 PUT-error suppression test); ~200 lines in
`tests/issue_create_echo.rs`.

## Tasks

- [ ] Read `prd-delta-398.md` — capture all locked decisions: team echo rules, description asymmetry,
      BTreeMap construction details for handle_edit and handle_create, `resolve_team_field` 5-path
      enumeration, IMP-2 import note, snapshot pinned body, and CLAUDE.md Gotcha verbatim text
- [ ] Read `verification-delta-398.md` — capture VP-398-001..006 test strategies and suggested test names
- [ ] Read `bc-3-issue-write.md` §BC-3.4.012, §BC-3.4.013, §BC-3.4.014 — extract preconditions,
      postconditions, invariants, and edge case IDs for AC compliance verification
- [ ] Read `src/cli/issue/helpers.rs` — locate `resolve_team_field` function; understand all 5 return
      paths and where to insert `team_name` as 3rd tuple element; confirm `is_team_uuid` predicate
      location (UUID-bypass: 36-char, 8-4-4-4-12 ASCII hex, case-insensitive)
- [ ] Read `src/cli/issue/create.rs` — locate `handle_edit` (full function: `changed_fields` BTreeMap
      insertion site, `edit_response` call at ~line 910, team/description/parent/points flag blocks);
      locate `handle_create` (~line 187, `--request-type` dispatch fork, `resolve_assignee_by_project`
      `_display_name` binding, team/description/label/points/parent/priority flag blocks);
      confirm `HashMap` usage elsewhere in `create.rs` (IMP-2)
- [ ] Read `src/cli/issue/json_output.rs` — locate `edit_response` + existing `test_edit` unit test;
      confirm current signature; confirm snapshot file name
- [ ] Read `src/cli/issue/list.rs` — locate `handle_list` line ~167 2-tuple closure for `resolved_team`;
      check for explicit `Option<(String, String)>` type annotation on `resolved_team` binding
- [ ] Read `CLAUDE.md` Gotchas section — confirm entry does not already exist before inserting
- [ ] Update `src/cli/issue/helpers.rs`: extend `resolve_team_field` to return
      `Result<(String, String, String)>` with `team_name` as 3rd element across all 5 paths
- [ ] Update `src/cli/issue/list.rs`: change `|(field_id, team_uuid)|` to
      `|(field_id, team_uuid, _resolved_team_name)|` in the `handle_list` map closure;
      update any explicit type annotation from `Option<(String,String)>` to `Option<(String,String,String)>`
- [ ] Update `src/cli/issue/json_output.rs`: change `edit_response` signature to accept
      `changed_fields: &BTreeMap<String, String>`; add `BTreeMap` to body; update `test_edit` to
      pass non-empty map; add `test_edit_response_empty_changed_fields` unit test (new naming convention);
      trigger snapshot regeneration
- [ ] Update `src/cli/issue/create.rs` `handle_edit`:
      - Add `use std::collections::BTreeMap;` (remove `HashMap` if no longer used)
      - Add `let mut changed_fields: BTreeMap<String, String> = BTreeMap::new();` at function top
      - Populate `changed_fields` for each flag in parallel with `fields` JSON construction
      - For description table echo: insert `"(updated)"` marker into `changed_fields` for table;
        store raw `desc_text` for JSON (same key, different display path — see AC-016 comment)
      - Update `edit_response` call: `json_output::edit_response(key, &changed_fields)`
      - Emit table-mode echo loop after `output::print_success`: `for (field, value) in &changed_fields { eprintln!("  {} → {}", field, value); }`
      - Confirm echo fires ONLY on single-key success path (`effective_keys.len() == 1`)
- [ ] Update `src/cli/issue/create.rs` `handle_create` (platform path only, after `--request-type` fork):
      - Add `let mut create_echo: BTreeMap<String, String> = BTreeMap::new();`
      - Populate `create_echo` for each resolved flag per the canonical mapping in §`changed_fields` Map Construction
      - Rebind `resolve_assignee_by_project` second element from `_display_name` to `display_name`
      - Emit echo in `OutputFormat::Table` arm between confirmation line and browse URL
      - `OutputFormat::Json` arm: NO change (create JSON path unchanged, AC-015)
- [ ] Add CLAUDE.md Gotcha entry (verbatim from prd-delta-398.md §6) — byte-for-byte copy
- [ ] Add `tests/issue_edit_echo.rs` with 14 new integration test functions per Required Test
      Deliverables table (tests #1–#11, #21–#23); mount wiremock for PUT 204 or PUT 400 as
      appropriate per test; tests #21–#22 cover empty-string `--summary ""` echo (EC-3.4.012-12
      and EC-3.4.013-10); test #23 mocks PUT 400 and asserts no echo fires (AC-021)
- [ ] Add `tests/issue_create_echo.rs` with 7 new integration test functions per Required Test
      Deliverables table (tests #12–#18); mount wiremock for POST 201 / project meta / team list
- [ ] Run `cargo test --test issue_edit_echo` — all 14 tests pass
- [ ] Run `cargo test --test issue_create_echo` — all 7 tests pass
- [ ] Run `cargo test --lib` — `test_edit` and `test_edit_response_empty_changed_fields` pass;
      `resolve_team_field` callers in helpers.rs and list.rs compile and test green
- [ ] Run `cargo test` — full suite green; `tests/issue_write_holdouts.rs` unchanged;
      `tests/issue_commands.rs` unchanged; `tests/issue_create_jsm.rs` unchanged
- [ ] Run `cargo clippy -- -D warnings` — zero warnings; no `#[allow]` suppressions; refactor if needed
- [ ] Run `cargo build --release` — succeeds
- [ ] Run `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh && bash scripts/check-bc-no-numeric-test-counts.sh` — all three exit 0 (BC count surfaces updated in F2; no additional spec file edits needed)
- [ ] Verify CLAUDE.md Gotcha entry present and matches pinned text byte-for-byte (AC-016 gate)
- [ ] Per-story adversary 3/3 CLEAN before push

## Previous Story Intelligence

This story immediately follows S-388 (cross-hierarchy edit --type 400 enrichment, PR #397) and
is within the same `src/cli/issue/create.rs` file. Key lessons carried forward:

- **From S-388 (cross-hierarchy errors):** Both S-388 and S-398 modify `handle_edit` in `create.rs`.
  Read the full `handle_edit` function before making changes — context around the field-building
  blocks and the error block matters. Do NOT assume line numbers are stable; search by function
  name and identifier patterns.

- **From S-385 (JSM UX polish):** The `handle_create` function has a `--request-type` dispatch
  fork early in the function body. The `create_echo` map MUST be declared after the fork on the
  platform path — do NOT insert it before the fork or it will be in scope on the JSM path.

- **From S-382 (InsufficientScope refactor):** Signature changes that thread through multiple files
  (`resolve_team_field` affects helpers.rs, create.rs x2, list.rs) must be done atomically —
  update all call sites before running `cargo build` to avoid cascading compiler errors.

- **From S-345 (label-coalesce proptest):** Pure function helpers near their call sites is the
  established pattern. No new public API is needed — all changes are internal to existing modules.

- **Verbatim string discipline (from multiple prior stories):** Copy the CLAUDE.md Gotcha entry
  byte-for-byte from `prd-delta-398.md §6`. Any character deviation will fail the AC-016 gate.
  The Gotcha text is PINNED (DECISION LOCKED — round 5 F-4).

- **Import hygiene (IMP-2):** After switching `changed_fields` to `BTreeMap`, check whether
  `HashMap` is still used elsewhere in `create.rs`. If not, remove the `use std::collections::HashMap`
  import — leaving an unused import fails `cargo clippy -- -D warnings`.

- **`_display_name` → `display_name` rebind:** The `resolve_assignee_by_project` call in
  `handle_create` currently discards the second return element with an underscore prefix. The
  rebind must be done to enable the assignee echo. `resolve_assignee_by_project` encapsulates
  `--to me` internally — the same rebind covers both the named-user and `me` cases.

## Architecture Compliance Rules

Extracted from BC bodies and CLAUDE.md conventions:

1. **Single-key path only for `issue edit` echo.** The echo (both table and JSON) applies
   EXCLUSIVELY when `effective_keys.len() == 1`, including `--jql` that resolves to exactly one
   issue. Bulk paths (`effective_keys.len() > 1`) are unaffected — no echo, no `changed_fields`.

2. **JSM path (`--request-type`) excluded from create echo.** The `create_echo` BTreeMap MUST
   NOT be constructed or emitted on the `handle_jsm_create` path. The dispatch fork gates this.

3. **BTreeMap guarantees alphabetical key order.** Use `BTreeMap<String, String>` (not `HashMap`)
   for both `changed_fields` (edit) and `create_echo` (create). This provides deterministic
   alphabetical key order in both table-mode iteration and JSON serialization without additional
   sorting.

4. **Description asymmetry is LOCKED.** Table mode echoes `"(updated)"`. JSON mode stores the
   raw `desc_text` string. Do NOT add a display-layer transformation to unify them. The CLAUDE.md
   Gotcha entry (AC-016) is the institutional guard. A code comment at each insertion site MUST
   reference the asymmetry and AC-016.

5. **`label` key excluded from `handle_edit` echo.** `--label` on `issue edit` routes through
   `handle_edit_bulk_labels`, which is separate from `handle_edit`. No `"label"` key is ever
   inserted into `changed_fields` on the edit path. (Create path IS different — `--label` on
   create is the single-POST path and IS echoed per AC-011.)

6. **`project` is NOT echoed on create.** Analogous to not echoing the issue key itself on edit.

7. **No `#[allow]` suppressions.** Zero-warning policy per CLAUDE.md. Refactor if clippy warns.

8. **No new modules, no new public API.** All changes are within existing files.

9. **`edit_response` is `pub(crate)`.** It is NOT callable from `tests/` directory. The only
   callers are the production site `create.rs:910` and inline unit tests in `json_output.rs`.
   Do NOT attempt to call it from `tests/issue_edit_echo.rs` — test via binary invocation.

## Library & Framework Requirements

No new dependencies. All changes use stdlib + existing project types.

- `BTreeMap` is in `std::collections` — no external crate needed.
- `wiremock` is already a dev-dependency — use the same version and import pattern as
  `tests/issue_edit_no_parent.rs` for the new integration tests.
- `assert_cmd` + `predicates` are already present for binary-level assertions.
- `insta` is already a dev-dependency — use the same `assert_snapshot!` pattern as existing
  `json_output.rs` tests for snapshot regeneration.
- No version pins change; no `Cargo.toml` edits.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `src/cli/issue/helpers.rs` | Modify | `resolve_team_field` return type `(String,String)` → `(String,String,String)`; add `team_name` per 5-path enumeration; ~80 LOC net |
| `src/cli/issue/create.rs` | Modify | Two BTreeMap blocks (`changed_fields` in handle_edit, `create_echo` in handle_create); echo loops; `edit_response` call update; `_display_name` rebind; `BTreeMap` import; `HashMap` import removal if unused; CLAUDE.md Gotcha reference comments; ~150 LOC net |
| `src/cli/issue/json_output.rs` | Modify | `edit_response` signature; `test_edit` non-empty BTreeMap; `test_edit_response_empty_changed_fields`; snapshot regeneration; ~20 LOC net |
| `src/cli/issue/list.rs` | Modify | `handle_list` closure 2-tuple → 3-tuple; type annotation update if present; ~5 LOC net |
| `CLAUDE.md` | Modify | Add description-echo-asymmetry Gotcha entry (verbatim pinned text); ~8 LOC net |
| `tests/issue_edit_echo.rs` | Create new | 11 integration tests for edit table/JSON echo; ~350 LOC net |
| `tests/issue_create_echo.rs` | Create new | 7 integration tests for create table echo; ~200 LOC net |
| `.factory/stories/STORY-INDEX.md` | Modify | Append S-398 row to Story Manifest + Feature Followup table; update total_stories (44→45) and last_updated |
| `.factory/sprint-state.yaml` | Modify | Append S-398 entry under `feature_followup_standalone` block |

**Files NOT to create:** No new source modules. No separate VP document files (VPs are inlined
in BC bodies per project convention per verification-delta-398.md §Project Convention Note).
No new spec files — BC files are sealed.

## Branch / PR Plan

- Branch: `feat/issue-398-changed-fields-echo`
- Target: `develop`
- Commit style: `feat(issue): echo changed/set fields on edit + create success (closes #398)`
- PR closes #398
- CHANGELOG entry required: user-visible output change on `issue edit` and `issue create` success paths

**Why `breaking_change: false`:** The `issue edit` table-mode output gains additional stderr lines —
additive (existing `Updated KEY` line unchanged). The JSON output gains a new `changed_fields` key —
additive (`updated: true` retained, backward-compat preserved). The `issue create` table mode gains
additional stderr lines between confirmation and browse URL — additive. No previously-succeeding
invocation changes its exit code or stdout shape. `resolve_team_field` change is internal only.

## Per-Story Delivery Notes

- F2 is CONVERGED (2026-05-21) — BC files are sealed. Do NOT re-edit BC files unless the
  adversary finds a discrepancy between the BC body and the implementation.
- BC count surfaces were updated in F2. Both guard scripts (`check-spec-counts.sh` and
  `check-bc-cumulative-counts.sh`) MUST exit 0 post-edit. If they do not, the F2 count-surface
  updates from `prd-delta-398.md §7` may not have been applied — verify BC-INDEX and
  CANONICAL-COUNTS.md before implementing.
- The `check-bc-no-numeric-test-counts.sh` guard also MUST exit 0 — do not add numeric test
  counts to BC Trace/Source fields.
- Per-story adversary 3/3 CLEAN required before push.
- Demos are LOCAL-ONLY per `docs/demo-evidence/` gitignore convention.

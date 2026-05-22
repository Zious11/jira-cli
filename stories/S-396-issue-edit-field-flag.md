---
document_type: story
story_id: "S-396"
title: "issue edit --field NAME=VALUE — arbitrary custom field editing via editmeta (closes #396)"
wave: feature-followup
status: ready
intent: enhancement
feature_type: backend
scope: standard
issue: 396
points: 8
priority: medium
tdd_mode: strict
estimated_effort: medium
depends_on: [S-398]  # S-398 supplies the changed_fields BTreeMap in handle_edit; S-396 extends it
bc_anchors:
  - BC-3.4.015
  - BC-3.4.016
  - BC-3.4.017
verification_properties:
  - VP-396-001
  - VP-396-002
  - VP-396-003
  - VP-396-004
  - VP-396-005
  - VP-396-006
  - VP-396-007
  - VP-396-008
  - VP-396-009
  - VP-396-010
  - VP-396-011
  - VP-396-012
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: ".factory/phase-f2-spec-evolution/prd-delta-396.md"
implementation_strategy: tdd
module_criticality: HIGH  # src/cli/issue/create.rs — core edit command single-key path; new api/jira/issues.rs endpoint + cache.rs additions; helpers.rs orchestration
files_modified:
  - src/cli/mod.rs                   # MODIFIED — Add `field: Vec<String>` to IssueCommand::Edit variant (matching IssueCommand::Create field definition); accept ArgAction::Append
  - src/cli/issue/create.rs          # MODIFIED — handle_edit: (1) destructure `field` from Edit, (2) parse via parse_field_kv → HashMap<String,String>, (3) add !field_pairs.is_empty() to has_any_field_change guard, (4) add --field to C-1 REJECTED_IN_BULK set, (5) Gate B flag-overlap detection (pre-HTTP), (6) call resolve_edit_fields in BOTH dry-run block AND live block (BC-3.4.015 invariant 10)
  - src/cli/issue/helpers.rs         # MODIFIED — new pub(crate) async fn resolve_edit_fields(client, profile, key, field_pairs, fields, changed_fields) -> Result<()>; orchestrates: customfield_NNNNN bypass, fields.json cache read, list_fields() fallback, write_fields_cache (best-effort), case-insensitive exact+substring match, get_editmeta(), operations/"set" check, type dispatch (string/number/option/date/datetime/user/array→exit64), option resolution (id-bypass + case-insensitive value match + ambiguous), changed_fields insertion
  - src/api/jira/issues.rs           # MODIFIED — Add pub async fn get_editmeta(key: &str) -> Result<EditMeta>; GET /rest/api/3/issue/{key}/editmeta
  - src/api/jira/fields.rs           # MODIFIED — (implementation detail) internal helpers for exact/substring field name matching; reused by resolve_edit_fields via list_fields() result
  - src/cache.rs                     # MODIFIED — Add FieldsCache struct + read_fields_cache + write_fields_cache (best-effort writer, matches CmdbFieldsCache pattern); 7-day TTL; path ~/.cache/jr/v1/<profile>/fields.json
  - CLAUDE.md                        # MODIFIED — New Gotcha entry: --field on issue edit (single-key-only; Request-Type non-goal JSDCLOUD-4609; JSM Urgency/Impact via Edit screen; editmeta HTTP round-trip) — F4 implementation deliverable
  - CHANGELOG.md                     # MODIFIED — [Unreleased] ### Added entry for --field on issue edit (prd-delta-396.md §12)
files_created:
  - src/types/jira/editmeta.rs       # NEW — EditMeta, EditMetaField, EditMetaFieldSchema, AllowedValue Serde structs; #[serde(rename="allowedValues")] on allowed_values field (CRITICAL — see prd-delta OBS-1)
  - tests/issue_edit_field.rs        # NEW — Integration tests for --field on issue edit; 26 test functions covering all 12 VPs
breaking_change: false
assumption_validations: []
risk_mitigations: []
# BC status: BC-3.4.015/016/017 produced in F2 (2026-05-22, prd-delta-396.md CONVERGED at pass 9).
# All BCs sealed; do NOT re-edit unless adversary finds implementation-BC discrepancy.
# F3 story produced after F2 convergence confirmed (3 consecutive clean passes: 7, 8, 9).
---

# S-396 — Issue Edit: `--field NAME=VALUE` Arbitrary Custom Field Editing

## Source of Truth

F2 PRD delta: `.factory/phase-f2-spec-evolution/prd-delta-396.md` (CONVERGED, 2026-05-22, pass 9).
F2 verification delta: `.factory/phase-f2-spec-evolution/verification-delta-396.md` (CONVERGED, 2026-05-22).
BC bodies: `.factory/specs/prd/bc-3-issue-write.md` §BC-3.4.015, §BC-3.4.016, §BC-3.4.017.

**Authoritative BC count: THREE new BCs (BC-3.4.015, BC-3.4.016, BC-3.4.017).** Spec version bump:
`bc-3 total_bcs 100→103; definitional_count 71→74; BC-INDEX total_bcs 580→583`.

## Problem Statement

`jr issue edit KEY` has no mechanism to set arbitrary custom fields — including JSM
request-type-scoped select fields like Urgency and Impact. The dedicated flags
(`--summary`, `--type`, `--priority`, `--team`, `--points`, `--parent`, `--description`)
cover first-party Jira fields only.

This story adds a `--field NAME=VALUE` flag (repeatable) to `issue edit`, enabling
callers to set any custom field that appears on the issue's agent Edit screen. The
mechanism extends the existing platform `PUT /rest/api/3/issue/{key}` path with a new
`GET /rest/api/3/issue/{key}/editmeta` call for field-presence validation and
single-select option resolution. A `fields.json` per-profile cache eliminates the
`GET /rest/api/3/field` round-trip on warm (repeated) invocations.

The primary motivating use case is JSM Urgency/Impact and comparable custom select
fields — but the feature works for any field type supported in v1: string, number,
option (single-select), date, datetime, user.

## Behavioral Contracts

| BC ID | File | Title | Clause(s) |
|-------|------|-------|-----------|
| BC-3.4.015 | `bc-3-issue-write.md` | `issue edit KEY --field NAME=VALUE` string/number/date/datetime/user field on single-key path — resolves field name, validates via editmeta, serializes per schema type, PUTs; success echoes in `changed_fields` | preconditions, postconditions, invariants 1–10, EC-3.4.015-1..20 |
| BC-3.4.016 | `bc-3-issue-write.md` | `issue edit KEY --field NAME=VALUE` single-select `option` field — human value → `allowedValues[].id` on wire; `changed_fields` echo shows human label, not id | preconditions, postconditions, invariants, EC-3.4.016-1..7 |
| BC-3.4.017 | `bc-3-issue-write.md` | `--field` multi-key/`--jql` multi-issue rejection (C-1 Gate A) + flag-overlap hard error for `summary`/`description`/`issuetype`/`priority` (Gate B) | preconditions, postconditions, invariants, EC-3.4.017-1..12 |

## Acceptance Criteria

### AC-001 — `--field NAME=VALUE` with string field: value in `changed_fields` echo (table and JSON)
(traces to BC-3.4.015 postconditions: resolved pair in `changed_fields`; VP-396-001)

When `jr issue edit KEY --field Severity=Critical` succeeds on a string-type field,
the resolved human name is used as the key (NOT the `customfield_NNNNN` internal ID)
in both output channels:

- Table mode: stderr MUST contain `  Severity → Critical` (two-space indent, unicode arrow).
- JSON mode (`--output json`): `changed_fields["Severity"]` MUST be `"Critical"`.
- JSON mode: `changed_fields` MUST NOT contain `"customfield_10001"` as a key.
- Exit code MUST be 0. PUT mock MUST be called with `{"fields": {"customfield_10001": "Critical"}}`.

### AC-002 — `customfield_NNNNN` literal bypass: skips `list_fields()` HTTP call entirely
(traces to BC-3.4.015 precondition: `customfield_\d+` literal bypass; VP-396-001 sub-case)

When `jr issue edit KEY --field customfield_10001=Critical` is run:

- `GET /rest/api/3/field` (list_fields) MUST NOT be called — the literal ID bypasses
  name resolution AND the `fields.json` cache entirely.
- `GET /rest/api/3/issue/KEY/editmeta` MUST still be called (literal bypass only skips
  Step 2, not Step 3).
- `changed_fields["customfield_10001"]` MUST be `"Critical"` (literal ID as key when
  bypass fires — no reverse lookup to a human name occurs).
- Exit code MUST be 0.

### AC-003 — Single-select `option` field: wire uses `{"id": "<optionId>"}`, echo uses human label
(traces to BC-3.4.016 postconditions: id on wire, label in echo; VP-396-002)

When `jr issue edit KEY --field Urgency=High --output json` succeeds:

- PUT body MUST contain `{"fields": {"customfield_10176": {"id": "10286"}}}` (option id on wire).
- `changed_fields["Urgency"]` MUST be `"High"` (human label, NOT `"10286"`).
- Exit code MUST be 0.

Case-insensitive option resolution sub-case: `--field Urgency=high` (lowercase) MUST resolve
to `allowedValues` stored-casing `"High"` in `changed_fields`, and `{"id": "10286"}` on wire.

Option id bypass sub-case: `--field Urgency=10286` (numeric id literal) MUST place `{"id": "10286"}`
on wire; `changed_fields["Urgency"]` MUST be `"10286"` (raw value echoed — no reverse label lookup).

### AC-004 — Field absent from `editmeta` → exit 64 with Edit-screen actionable hint; no PUT
(traces to BC-3.4.015 EC-3.4.015-3 and EC-3.4.015-8; VP-396-003)

When the requested field is not present in `editmeta.fields`:

- Exit code MUST be 64.
- Stderr MUST contain a substring referencing the Edit screen (e.g., "Edit screen") and
  an admin action (e.g., "admin" or "project admin").
- `PUT /rest/api/3/issue/KEY` MUST NOT be called.

`customfield_NNNNN` literal sub-case: same expectations — the literal-bypass still passes
through `get_editmeta()` (Step 3); `GET /rest/api/3/field` MUST NOT be called.

### AC-005 — Unsupported field types (`array`, `any`) → exit 64 with hint; no PUT
(traces to BC-3.4.015 EC-3.4.015-5; VP-396-004)

When `editmeta` reports `schema.type` as `"array"` or `"any"` (or any other unknown type):

- Exit code MUST be 64.
- Stderr MUST mention the unsupported type and provide an actionable hint.
- PUT MUST NOT be called.

### AC-006 — Gate A: `--field` rejected with multi-key positional or `--jql` resolving to 2+ issues
(traces to BC-3.4.017 Gate A; VP-396-005)

Multi-key positional sub-case (`jr issue edit KEY1 KEY2 --field Urgency=High`):

- Exit code MUST be 64.
- Stderr MUST reference `--field` and the bulk-rejection pattern.
- No HTTP calls MUST be made (no `list_fields()`, no `editmeta`, no PUT).

`--jql` resolving to 2+ issues sub-case:

- Exit code MUST be 64. PUT MUST NOT be called.

`--jql` resolving to exactly ONE issue is NOT rejected — it proceeds normally on the
single-key path (consistent with existing `--team` single-match `--jql` behavior).

### AC-007 — Gate B: flag-overlap for `--summary`, `--description`, `--type`, `--priority` → exit 64 no HTTP
(traces to BC-3.4.017 Gate B; VP-396-005)

For each overlap:
- `jr issue edit KEY --summary "New" --field summary=Other` → exit 64, conflict message mentioning both `--summary` and `--field`; no HTTP calls.
- `jr issue edit KEY --description "text" --field description=other` → exit 64; no HTTP.
- `jr issue edit KEY --type Bug --field issuetype=Task` → exit 64; no HTTP.

Gate B does NOT apply to `--team` or `--points` (those use dynamically-resolved custom
field IDs; overlap detection for them requires an API call, deferred to v2).

`--field type=X` (the string `"type"`) does NOT trigger Gate B — Gate B only triggers on
`"issuetype"` (the actual system field key). See EC-3.4.017-11.

### AC-008 — Warm `fields.json` cache prevents `GET /rest/api/3/field` HTTP call
(traces to BC-3.4.015 EC-3.4.015-14; VP-396-006)

When a non-stale `fields.json` cache exists for the active profile:

- `GET /rest/api/3/field` MUST NOT be called (zero HTTP calls to that endpoint).
- `editmeta` and PUT calls MUST still execute normally.
- Exit code MUST be 0.

Cold-cache sub-case (VP-396-006 EC-3.4.015-15):

- `GET /rest/api/3/field` MUST be called EXACTLY ONCE.
- `fields.json` MUST exist in the cache directory with a `fetched_at` timestamp within
  the last few seconds.
- A second invocation with the same warm cache MUST NOT call `GET /rest/api/3/field`.

### AC-009 — Cache-write failure is swallowed; resolution and PUT succeed; warning on stderr only
(traces to BC-3.4.015 EC-3.4.015-16, invariant 7; VP-396-007)

When `write_fields_cache` encounters a disk I/O error:

- Exit code MUST be 0.
- Stderr MUST contain `"warning: failed to write fields cache"`.
- PUT MUST be called with the correct field payload.
- `changed_fields` MUST appear in JSON output.

`--output json` channel-separation sub-case (VP-396-007 P2-007): stdout MUST NOT contain
the substring `"warning"` — the warning goes to stderr only, never stdout.

### AC-010 — `--field` + `--dry-run`: resolution runs inside dry-run block; PUT never called; exit 0
(traces to BC-3.4.015 EC-3.4.015-18, invariant 10; VP-396-008)

Success path (`jr issue edit KEY --field Severity=Critical --dry-run`):

- `GET /rest/api/3/field` (or cache) and `GET .../editmeta` MUST be called (resolution runs).
- PUT MUST NOT be called.
- Planned-changes preview MUST include `Severity → Critical`.
- Exit code MUST be 0 (pinned: `return Ok(())` at the end of the dry-run block).

Gate A under `--dry-run` sub-case: `jr issue edit KEY1 KEY2 --field Urgency=High --dry-run`
MUST still exit 64 (Gate A fires before any HTTP).

Resolution failure under `--dry-run` sub-case (EC-3.4.015-19):

- When field name is unknown, exit code MUST be 64 (resolution errors are NOT suppressed by `--dry-run`).
- No planned-changes preview is emitted.

CRITICAL implementation note: `resolve_edit_fields` MUST be called INSIDE the
`if dry_run { ... }` block, before the `return Ok(())` short-circuit. Placing it
only on the live path (after the dry-run block) is a correctness bug that violates
BC-3.4.015 invariant 10, EC-3.4.015-18, EC-3.4.015-19, and VP-396-008.

### AC-011 — Multi-`--field` partial-failure: any resolution failure → zero PUT; `changed_fields` not emitted
(traces to BC-3.4.015 EC-3.4.015-12; VP-396-009)

When `jr issue edit KEY --field A_OK=val --field UnknownField=val`:

- Exit code MUST be 64 (UnknownField: zero matches).
- PUT MUST NOT be called.
- Stderr MUST contain NO `→` echo lines.

PUT-failure sub-case (EC-3.4.015-12a): when PUT returns 400:

- `changed_fields` MUST NOT be echoed in table mode.
- JSON stdout MUST NOT contain `changed_fields.Severity`.

### AC-012 — Number field: integer wire form; `5e3` → `5000`; NaN/Inf rejected exit 64
(traces to BC-3.4.015 EC-3.4.015-4a, invariant 5; VP-396-010)

When `--field StoryPoints=5` targets a `number`-type field:

- PUT body MUST contain integer `5` (NOT `5.0`).
- `5e3` → PUT body MUST contain `5000` (NOT `"5e3"` string, NOT `5000.0`).
- `inf` (or NaN) → exit 64, stderr mentions parse error, PUT NOT called.

### AC-013 — `user` wire shape `{"accountId": VALUE}`; `date`/`datetime` bare-string pass-through
(traces to BC-3.4.015 Step 4; VP-396-011)

`user` type: `--field Reporter=abc123` → PUT body MUST contain
`{"customfield_10050": {"accountId": "abc123"}}`. `changed_fields["Reporter"]` MUST be
`"abc123"` (raw accountId echoed).

`date` type: `--field DueDate=2026-12-31` → PUT body MUST contain bare string
`"2026-12-31"` (no wrapping). Exit code MUST be 0.

`datetime` type: bare-string pass-through; junk value `"not-a-date"` MUST be transmitted
verbatim to Jira without client-side rejection (no-validation contract — VP-396-011 step 4).

### AC-014 — Field present in `editmeta` but `"set"` absent from `operations` → exit 64 with hint
(traces to BC-3.4.015 Step 3b, EC-3.4.015-20; VP-396-012)

When `editmeta` returns a field with `operations: ["transition"]` (missing `"set"`):

- Exit code MUST be 64.
- Stderr MUST mention the field name and the `"set"` operations constraint.
- PUT MUST NOT be called.

Empty `operations: []` sub-case: same exit 64, same hint.

### AC-015 — CLAUDE.md Gotcha entry added during F4 (NOT during F3/spec)
(traces to BC-3.4.015 prd-delta §10 timing constraint; prd-delta-396.md §10)

The following Gotcha entry MUST be added to CLAUDE.md during implementation (F4 deliverable),
in the same PR as the `--field` feature code:

1. `--field` on `issue edit` is single-key only (C-1 guard rejects bulk).
2. Changing the Request Type of an existing JSM issue is NOT supported via any Jira Cloud API.
   `jr issue edit --field` does NOT support `sd-customerrequesttype` (JSDCLOUD-4609, open since
   2016; PUT returns HTTP 500).
3. JSM Urgency/Impact CAN be set via `jr issue edit --field NAME=VALUE` if the field is on
   the issue's agent Edit screen (admin must add it — not on Edit screen by default).
4. `--field` on `issue edit` uses `editmeta` to validate and resolve `allowedValues`. The
   `GET .../editmeta` call adds one HTTP round-trip when `--field` is set; it is skipped
   when `--field` is absent.

This AC MUST NOT be closed without the Gotcha entry present in CLAUDE.md.

### AC-016 — CHANGELOG.md `[Unreleased] ### Added` entry included in PR
(traces to prd-delta-396.md §12)

A CHANGELOG entry under `[Unreleased] / ### Added` MUST be present in the implementation PR,
covering: `--field NAME=VALUE` (repeatable) on `issue edit`; supported types; field-name
resolution; editmeta validation; flag-overlap guard; single-key-only constraint; Request-Type
non-goal (JSDCLOUD-4609). See prd-delta-396.md §12 for the authoritative entry text.

### AC-017 — `ci/issue-396-bc-cumulative-counts-surface-h` guard cherry-picked into implementation branch
(traces to prd-delta-396.md P2-002; DRIFT-002 mitigation)

The Surface-H guard extension (`check-bc-cumulative-counts.sh` extended to parse the
`## Total BCs in this file: N individually-bodied (cumulative M ...)` footer) committed
on branch `ci/issue-396-bc-cumulative-counts-surface-h` (commit `3dd6fdb`) MUST be
cherry-picked or folded into the `#396` feature branch at F4. It ships in the same PR as
the `--field` feature code — NOT as a separate PR — so the script validating the
BC footer and the BCs themselves land atomically.

Implementation note: after cherry-pick, run `bash scripts/check-bc-cumulative-counts.sh`
and verify exit 0 (583 total across 8 files; Surface H verified).

### AC-018 — `editmeta` Serde structs have correct field-rename annotations
(traces to BC-3.4.015, prd-delta-396.md §5 OBS-1)

`EditMetaField.allowed_values` MUST carry `#[serde(rename = "allowedValues")]`. Without
this rename, the field deserializes to `None` for every field, causing BC-3.4.016 to fail
with EC-3.4.016-3 on every valid option field.

`EditMetaFieldSchema.field_type` MUST carry `#[serde(rename = "type")]`.

All other fields (`name`, `schema`, `operations`, `required`, `id`, `value`) match their
JSON keys exactly and require no rename. Verify with a unit-level deserialization test
before integration testing option fields.

## Serde Struct Reference (copy-paste-correct per prd-delta-396.md §5 OBS-1)

```rust
// src/types/jira/editmeta.rs

pub struct EditMeta {
    pub fields: HashMap<String, EditMetaField>,
}

pub struct EditMetaField {
    pub name: String,
    pub schema: EditMetaFieldSchema,
    #[serde(rename = "allowedValues")]
    pub allowed_values: Option<Vec<AllowedValue>>,
    pub operations: Vec<String>,
    pub required: bool,   // future use: required-field validation; retain, do not remove
}

pub struct EditMetaFieldSchema {
    #[serde(rename = "type")]
    pub field_type: String,   // "string", "number", "option", "array", "date", "datetime", "user"
    pub system: Option<String>,
    pub custom: Option<String>,
}

pub struct AllowedValue {
    pub id: String,
    pub value: Option<String>,
    pub name: Option<String>,   // future use: cascade-select matching; retain, suppress lint only if compiler warns
}
```

## `resolve_edit_fields` Canonical Signature

```rust
// in src/cli/issue/helpers.rs
pub(crate) async fn resolve_edit_fields(
    client: &JiraClient,
    profile: &str,                              // CLAUDE.md cache-boundary rule
    key: &str,
    field_pairs: &HashMap<String, String>,      // parse_field_kv output; last-wins duplicates
    fields: &mut serde_json::Value,             // shared fields JSON object
    changed_fields: &mut BTreeMap<String, String>,
) -> Result<()>
```

The `profile` parameter (second after `client`) is MANDATORY per CLAUDE.md cache-boundary
rule: every cache reader/writer takes `profile: &str`. Cross-profile field-ID leakage is a
correctness bug (sandbox vs prod custom-field IDs differ).

## `--dry-run` × `--field` Execution Placement (BC-3.4.015 invariant 10)

```
// CORRECT placement — resolve_edit_fields INSIDE the dry-run block:
if dry_run {
    // ... existing flag resolutions (description, summary, type, priority, team, points) ...
    resolve_edit_fields(client, &config.active_profile_name, key, &field_pairs, &mut fields, &mut changed_fields).await?;
    // ... render planned-changes preview ...
    return Ok(());   // exit 0; PUT NOT called
}

// live path:
// ... existing flag resolutions ...
resolve_edit_fields(client, &config.active_profile_name, key, &field_pairs, &mut fields, &mut changed_fields).await?;
client.edit_issue(key, fields).await?;
// ... success echo ...
```

Placing `resolve_edit_fields` ONLY after the `return Ok(())` short-circuit means the
dry-run path never validates the field, never previews `--field` entries, and never
exits 64 on a bad `--field` value. This violates EC-3.4.015-18, EC-3.4.015-19, and VP-396-008.

## `fields.json` Cache Pattern

Mirrors `CmdbFieldsCache` / `cmdb_fields.json` in `src/cache.rs` exactly:

```rust
pub struct FieldsCache {
    pub fields: Vec<(String, String)>,   // (id, name) tuples
    pub fetched_at: DateTime<Utc>,
}

pub fn read_fields_cache(profile: &str) -> Result<Option<FieldsCache>> {
    read_cache(profile, "fields.json")
}

/// Best-effort writer: swallows disk-write errors with eprintln! and returns Ok(()).
/// A missed write costs at most one extra HTTP call on the next invocation.
/// See "best-effort writer" pattern in CLAUDE.md Gotchas (request-type cache writers).
pub fn write_fields_cache(profile: &str, fields: &[(String, String)]) -> Result<()> { ... }
```

TTL: 7 days. Path: `~/.cache/jr/v1/<profile>/fields.json`.

## Required Test Deliverables Summary

All tests in `tests/issue_edit_field.rs` (new file). Test naming convention: `test_BC_<id>_<behavior>`.

| # | Test function name | Type | VP/AC | BC |
|---|-------------------|----|-------|----|
| 1 | `test_BC_3_4_015_field_string_value_appears_in_table_echo` | integration | VP-396-001, AC-001 | BC-3.4.015 |
| 2 | `test_BC_3_4_015_field_string_value_appears_in_json_changed_fields` | integration | VP-396-001, AC-001 | BC-3.4.015 |
| 3 | `test_BC_3_4_015_customfield_literal_bypass_skips_list_fields` | integration | VP-396-001, AC-002 | BC-3.4.015 |
| 4 | `test_BC_3_4_016_option_field_resolves_to_id_on_wire_and_label_in_echo` | integration | VP-396-002, AC-003 | BC-3.4.016 |
| 5 | `test_BC_3_4_016_option_field_case_insensitive_resolution` | integration | VP-396-002, AC-003 | BC-3.4.016 |
| 6 | `test_BC_3_4_016_option_field_id_bypass` | integration | VP-396-002, AC-003 | BC-3.4.016 |
| 7 | `test_BC_3_4_015_field_absent_from_editmeta_exits_64_with_hint` | integration | VP-396-003, AC-004 | BC-3.4.015 |
| 8 | `test_BC_3_4_015_customfield_literal_absent_from_editmeta_exits_64` | integration | VP-396-003, AC-004 | BC-3.4.015 |
| 9 | `test_BC_3_4_015_array_type_field_exits_64_with_hint` | integration | VP-396-004, AC-005 | BC-3.4.015 |
| 10 | `test_BC_3_4_015_any_type_field_exits_64_with_hint` | integration | VP-396-004, AC-005 | BC-3.4.015 |
| 11 | `test_BC_3_4_017_field_multi_key_rejected_exit_64` | integration | VP-396-005, AC-006 | BC-3.4.017 |
| 12 | `test_BC_3_4_017_field_jql_multi_issue_rejected_exit_64` | integration | VP-396-005, AC-006 | BC-3.4.017 |
| 13 | `test_BC_3_4_017_field_summary_overlap_exits_64_no_http` | integration | VP-396-005, AC-007 | BC-3.4.017 |
| 14 | `test_BC_3_4_017_field_description_overlap_exits_64_no_http` | integration | VP-396-005, AC-007 | BC-3.4.017 |
| 15 | `test_BC_3_4_017_field_issuetype_overlap_exits_64_no_http` | integration | VP-396-005, AC-007 | BC-3.4.017 |
| 16 | `test_BC_3_4_015_warm_fields_cache_skips_field_list_http` | integration | VP-396-006, AC-008 | BC-3.4.015 |
| 17 | `test_BC_3_4_015_cold_cache_fetches_and_populates_fields_cache` | integration | VP-396-006, AC-008 | BC-3.4.015 |
| 18 | `test_BC_3_4_015_cache_write_failure_warns_and_exits_0` | integration | VP-396-007, AC-009 | BC-3.4.015 |
| 19 | `test_BC_3_4_015_cache_write_failure_warning_on_stderr_not_stdout` | integration | VP-396-007, AC-009 | BC-3.4.015 |
| 20 | `test_write_fields_cache_swallows_io_error_and_returns_ok` | unit (src/cache.rs) | VP-396-007, AC-009 | BC-3.4.015 |
| 21 | `test_BC_3_4_015_field_dry_run_exits_0_no_put` | integration | VP-396-008, AC-010 | BC-3.4.015 |
| 22 | `test_BC_3_4_015_field_dry_run_resolution_failure_exits_64` | integration | VP-396-008, AC-010 | BC-3.4.015 |
| 23 | `test_BC_3_4_017_gate_a_fires_under_dry_run` | integration | VP-396-008, AC-010 | BC-3.4.017 |
| 24 | `test_BC_3_4_015_field_partial_resolution_failure_no_put` | integration | VP-396-009, AC-011 | BC-3.4.015 |
| 25 | `test_BC_3_4_015_field_put_failure_discards_changed_fields` | integration | VP-396-009, AC-011 | BC-3.4.015 |
| 26 | `test_BC_3_4_015_number_field_integer_wire_form` | integration | VP-396-010, AC-012 | BC-3.4.015 |
| 27 | `test_BC_3_4_015_number_field_scientific_notation_wire_form` | integration | VP-396-010, AC-012 | BC-3.4.015 |
| 28 | `test_BC_3_4_015_number_field_nan_rejected_exit_64` | integration | VP-396-010, AC-012 | BC-3.4.015 |
| 29 | `test_BC_3_4_015_user_field_wire_shape_account_id` | integration | VP-396-011, AC-013 | BC-3.4.015 |
| 30 | `test_BC_3_4_015_date_field_bare_string_pass_through` | integration | VP-396-011, AC-013 | BC-3.4.015 |
| 31 | `test_BC_3_4_015_datetime_field_bare_string_pass_through` | integration | VP-396-011, AC-013 | BC-3.4.015 |
| 32 | `test_BC_3_4_015_operations_lacks_set_exits_64` | integration | VP-396-012, AC-014 | BC-3.4.015 |
| 33 | `test_BC_3_4_015_empty_operations_exits_64` | integration | VP-396-012, AC-014 | BC-3.4.015 |

**VP Coverage**: All 12 VPs covered. VP-396-001 → tests #1–3; VP-396-002 → tests #4–6;
VP-396-003 → tests #7–8; VP-396-004 → tests #9–10; VP-396-005 → tests #11–15;
VP-396-006 → tests #16–17; VP-396-007 → tests #18–20; VP-396-008 → tests #21–23;
VP-396-009 → tests #24–25; VP-396-010 → tests #26–28; VP-396-011 → tests #29–31;
VP-396-012 → tests #32–33.

## Files NOT to Touch (Regression Baseline)

| File | Why Unchanged |
|------|--------------|
| `src/api/jsm/` | No JSM path changes |
| `src/cli/issue/create.rs` `handle_jsm_create` | JSM dispatch path; `--field` already implemented (S-288-pr4, BC-3.8.008) |
| `src/cli/issue/create.rs` `handle_edit_bulk_labels` / `handle_edit_bulk_fields` | Bulk paths unaffected |
| `src/cli/issue/json_output.rs` | `edit_response` already accepts `&BTreeMap<String, String>` from S-398 |
| `src/api/auth.rs` | No auth changes |
| `tests/issue_edit_echo.rs` | S-398 tests; regression baseline must stay green |
| `tests/issue_create_echo.rs` | S-398 tests; regression baseline must stay green |
| `tests/issue_commands.rs` | Pre-echo regressions; must remain green |
| `tests/issue_write_holdouts.rs` | Success paths unchanged |
| `tests/issue_create_jsm.rs` | JSM path; no change |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~10 k |
| F2 PRD delta (`prd-delta-396.md`) | ~16 k |
| F2 verification delta (`verification-delta-396.md`) | ~12 k |
| BC files — BC-3.4.015/016/017 sections in `bc-3-issue-write.md` | ~14 k |
| `src/cli/issue/create.rs` (handle_edit full function + dry-run block + C-1 guard + parse_field_kv) | ~20 k |
| `src/cli/issue/helpers.rs` (resolve_team_field vicinity + new resolve_edit_fields site) | ~8 k |
| `src/api/jira/issues.rs` (edit_issue + surrounding context for new get_editmeta) | ~6 k |
| `src/api/jira/fields.rs` (list_fields + find_field_by_name) | ~4 k |
| `src/cache.rs` (CmdbFieldsCache / cmdb_fields pattern to mirror) | ~5 k |
| `src/cli/mod.rs` (Edit variant for field: Vec<String> addition) | ~3 k |
| `tests/common/` (fixture context for new integration tests) | ~4 k |
| Tool outputs + `cargo test` + `cargo clippy` output | ~10 k |
| **Total** | **~112 k** |

Well within a single-agent context window (~200 k). No split required.
LOC delta estimate: ~25 LOC in `src/cli/mod.rs`; ~80 LOC in `create.rs` (Gate B + dry-run/live
calls + field_pairs destructure); ~200 LOC in `helpers.rs` (resolve_edit_fields + type dispatch
+ option resolution); ~30 LOC in `issues.rs` (get_editmeta); ~50 LOC in `cache.rs` (FieldsCache
+ read/write); ~40 LOC in `editmeta.rs` (new type file); ~1 CLAUDE.md Gotcha (~12 lines);
~700 LOC in `tests/issue_edit_field.rs` (33 tests).

## Tasks

- [ ] Read `prd-delta-396.md` — capture: locked design decisions table, Field-Name Resolution Algorithm
      (Steps 1–6), execution sequence in handle_edit (§9 dry-run/live split), canonical
      `resolve_edit_fields` signature (with `profile: &str`), FieldsCache pattern (§5), Serde
      rename audit (§5 OBS-1), `parse_field_kv` → HashMap semantics (last-wins, §9 F-1 amendment),
      CLAUDE.md Gotcha verbatim (§10), CHANGELOG entry (§12)
- [ ] Read `verification-delta-396.md` — capture VP-396-001..012 test strategies and suggested names;
      note VP permanence decision (§Project Convention Note): VP detail blocks are transient F2/F3
      working artifacts consumed by test-writer in F4
- [ ] Read `bc-3-issue-write.md` §BC-3.4.015, §BC-3.4.016, §BC-3.4.017 — extract all preconditions,
      postconditions, invariants, and EC IDs (EC-3.4.015-1..20, EC-3.4.016-1..7, EC-3.4.017-1..12)
- [ ] Read `src/cli/mod.rs` — locate IssueCommand::Edit variant; confirm `field: Vec<String>` does
      not already exist; check ArgAction for existing `Vec<String>` fields (e.g., IssueCommand::Create)
- [ ] Read `src/cli/issue/create.rs` — locate: `handle_edit` function boundary; C-1 REJECTED_IN_BULK
      set; `has_any_field_change` guard; `parse_field_kv` call site (on JSM path); `if dry_run { ... }`
      block and its `return Ok(())` exit; `changed_fields` BTreeMap (from S-398); `edit_response` call site
- [ ] Read `src/cli/issue/helpers.rs` — confirm `parse_field_kv` is `pub(crate)` and can be reused;
      locate appropriate insertion point for `resolve_edit_fields`; check current LOC (should be ~833+80)
- [ ] Read `src/api/jira/issues.rs` — confirm `edit_issue` and `get_issue` signatures; locate insertion
      point for `get_editmeta`; check existing request-building pattern for new GET method
- [ ] Read `src/api/jira/fields.rs` — confirm `list_fields()` return type; understand field struct
      fields (id, name) for cache tuple construction
- [ ] Read `src/cache.rs` — locate `CmdbFieldsCache` and `cmdb_fields.json` pattern; mirror it exactly
      for `FieldsCache` + `read_fields_cache` + `write_fields_cache` (best-effort writer)
- [ ] Read `CLAUDE.md` Gotchas section — confirm `--field` entry does not already exist; note exact
      location to insert new entry
- [ ] Cherry-pick or fold `ci/issue-396-bc-cumulative-counts-surface-h` commit `3dd6fdb` into the
      `#396` feature branch (AC-017); run `bash scripts/check-bc-cumulative-counts.sh` → exit 0
- [ ] Create `src/types/jira/editmeta.rs` with EXACTLY the Serde structs from the §Serde Struct
      Reference section — pay special attention to `#[serde(rename = "allowedValues")]` on
      `allowed_values` and `#[serde(rename = "type")]` on `field_type`; register in
      `src/types/jira/mod.rs`
- [ ] Add `get_editmeta(key: &str) -> Result<EditMeta>` to `src/api/jira/issues.rs`
- [ ] Add `FieldsCache` + `read_fields_cache` + `write_fields_cache` (best-effort) to `src/cache.rs`
- [ ] Update `src/cli/mod.rs`: add `field: Vec<String>` with `#[arg(long = "field", action = ArgAction::Append)]`
      to `IssueCommand::Edit` variant
- [ ] Update `src/cli/issue/helpers.rs`: add `resolve_edit_fields` implementing Steps 1–6 per
      prd-delta-396.md §6 algorithm:
      - Step 1: `customfield_\d+` regex bypass (skips cache+API; goes directly to Step 3)
      - Step 2: `read_fields_cache(profile)` → hit → use cached list; miss/stale → `list_fields()` → `write_fields_cache` (best-effort)
      - Step 2b: case-insensitive exact match first, then substring; 0 matches → exit 64; multiple → exit 64
      - Step 3: `get_editmeta(key)` → absent → exit 64 with Edit-screen hint
      - Step 3b: `"set"` ∉ `operations` → exit 64 with operations hint
      - Step 4: type dispatch per prd-delta §6 table; `option` arm dispatches to Step 4a BEFORE unknown arm
      - Step 4a: option id bypass (numeric string matches `allowedValues[].id`) → exact → substring; empty → exit 64; ambiguous → exit 64
      - Step 5: merge `(field_id, wire_value)` into `fields` JSON object
      - Step 6: insert `(human_name, display_value)` into `changed_fields` (option fields: human label not id)
- [ ] Update `src/cli/issue/create.rs` `handle_edit`:
      - Destructure `field` from `IssueCommand::Edit`; call `parse_field_kv` to get `HashMap<String, String>`
      - Extend `has_any_field_change` to include `!field_pairs.is_empty()`
      - Add `"field"` (or the flag identifier) to C-1 `REJECTED_IN_BULK` set
      - Add Gate B overlap detection (before any HTTP): check `summary`, `description`, `issuetype`, `priority`
      - INSIDE `if dry_run { ... }` block: call `resolve_edit_fields(client, &config.active_profile_name, key, &field_pairs, &mut fields, &mut changed_fields).await?`
      - ALSO on live path (after dry-run block): call `resolve_edit_fields` with same args
- [ ] Add CLAUDE.md Gotcha entry (§10 verbatim text) — F4 implementation deliverable, do NOT add
      it during F3/spec-only work
- [ ] Add CHANGELOG.md entry per prd-delta-396.md §12
- [ ] Create `tests/issue_edit_field.rs` with 33 test functions per Required Test Deliverables table;
      for each test: mount wiremock stubs, invoke binary, assert exit code + output channel content
- [ ] Run `cargo test --test issue_edit_field` — all 33 tests pass
- [ ] Run `cargo test --test issue_edit_echo && cargo test --test issue_create_echo` — S-398 regression baseline green
- [ ] Run `cargo test --lib` — `test_write_fields_cache_swallows_io_error_and_returns_ok` passes; all other unit tests green
- [ ] Run `cargo test` — full suite green; `tests/issue_write_holdouts.rs`, `tests/issue_commands.rs`, `tests/issue_create_jsm.rs` unchanged
- [ ] Run `cargo clippy -- -D warnings` — zero warnings; no `#[allow]` suppressions; refactor if needed
- [ ] Run `cargo build --release` — succeeds
- [ ] Run `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh && bash scripts/check-bc-no-numeric-test-counts.sh` — all three exit 0
- [ ] Verify CLAUDE.md Gotcha entry present and matches §10 description point-for-point (AC-015 gate)
- [ ] Per-story adversary 3/3 CLEAN before push

## Previous Story Intelligence

This story immediately follows S-398 (issue edit/create changed-fields echo) and extends
the same `handle_edit` function in `src/cli/issue/create.rs`. Key lessons carried forward:

- **From S-398 (changed-fields echo):** S-398 added the `changed_fields: BTreeMap<String, String>`
  to `handle_edit` and the `edit_response` signature change. S-396 depends on S-398 being merged
  first — `resolve_edit_fields` inserts into the `changed_fields` map that S-398 created. Read
  `handle_edit` in full before modifying — the dry-run block structure and BTreeMap placement are
  now load-bearing.

- **From S-388 (cross-hierarchy type errors):** Both S-388 and S-396 modify `handle_edit`. Read
  the FULL function before changes — context around the C-1 guard, the existing `REJECTED_IN_BULK`
  set, and the dry-run block matters. Do NOT assume line numbers are stable; search by identifier.

- **From S-288-pr4 (JSM create --field flag):** `parse_field_kv` is already `pub(crate)` and
  handles `last-wins` duplicate key semantics via `HashMap::insert`. Call it from `handle_edit`
  exactly as it is called in `handle_jsm_create`. No signature change needed.

- **Placement discipline for `resolve_edit_fields` (CRITICAL from prd-delta §9):** The function
  MUST appear in BOTH the `if dry_run { ... }` sub-path AND the live sub-path. Implementers who
  put it only after the dry-run `return Ok(())` break three ACs silently. See AC-010 and the
  §`--dry-run × --field` section above.

- **Cache pattern (from S-288-pr2 request-type caches):** Use the best-effort writer model for
  `write_fields_cache` — swallow I/O errors with `eprintln!("warning: ...")` and return `Ok(())`.
  Cache write failures must NEVER break a successful API call.

- **Serde rename discipline (from `cmdb_fields.json` pattern):** `allowed_values` in
  `EditMetaField` MUST carry `#[serde(rename = "allowedValues")]` or option field tests will
  all fail silently with EC-3.4.016-3 ("field has no configured option values"). Verify with
  a deserialization unit test before running integration tests.

- **Verbatim string discipline:** Copy the CLAUDE.md Gotcha verbatim from prd-delta-396.md §10
  (4 bullet points). Any deviation will fail the AC-015 gate.

## Architecture Compliance Rules

Extracted from BC bodies and CLAUDE.md conventions:

1. **Single-key path only for `--field`.** Gate A (`REJECTED_IN_BULK`) rejects `--field` when
   `effective_keys.len() > 1` or `--jql` resolves to 2+ issues. The exact-one `--jql` fast
   path is NOT rejected.

2. **Gate B fires before any HTTP call.** Overlap detection (summary/description/issuetype/priority)
   must be evaluated after clap parsing but before `resolve_edit_fields`, `list_fields()`, and
   `get_editmeta()`. See prd-delta §9 Steps 1–3 (common) vs Steps 4–7 (diverge at dry-run).

3. **`resolve_edit_fields` is in BOTH dry-run and live sub-paths** (BC-3.4.015 invariant 10).
   This is non-negotiable. Dry-run resolution failures must exit 64.

4. **`customfield_NNNNN` bypass skips Steps 2/2b only; Step 3 (`get_editmeta`) still executes.**
   The bypass is case-sensitive: `customfield_10001` (lowercase) is bypassed; `CUSTOMFIELD_10001`
   falls through to name resolution (deliberate — Jira uses lowercase only; uppercase would mask typos).

5. **`fields.json` is per-profile** (`read_cache(profile, "fields.json")`). Pass
   `&config.active_profile_name` from the handler. Cross-profile cache leakage is a correctness bug.

6. **Best-effort writer model** for `write_fields_cache`. Disk errors must never break a successful
   API call. Warning goes to stderr; stdout is clean in `--output json` mode.

7. **`option` type dispatch arm BEFORE unknown→exit-64 arm** in `resolve_edit_fields` type dispatch
   table. If `option` is placed after the unknown arm, option fields exit 64 silently.

8. **`changed_fields` for option fields stores human label, not option id.** Consistent with
   BC-3.4.012/013 model: machine-readable id is on the wire; human-readable label is in the echo.

9. **No `#[allow]` suppressions.** Zero-warning policy. Refactor if clippy warns.

10. **No new public API surface.** All new functions are `pub(crate)`. `edit_response` caller
    signature is unchanged (already accepts `&BTreeMap<String, String>` from S-398).

11. **`editmeta` is NOT cached.** The response is issue-specific and mutable (admin can change
    the Edit screen). Caching risks stale `allowedValues` producing wrong option IDs on wire.

## Library & Framework Requirements

No new dependencies beyond what S-398 introduced. All changes use stdlib + existing project types.

- `BTreeMap` is in `std::collections` — no external crate needed.
- `HashMap` is `std::collections::HashMap` — used for `parse_field_kv` output (last-wins semantics).
- `regex` crate is NOT required for `customfield_\d+` detection — a simple string prefix check
  (`name.starts_with("customfield_") && name[12..].chars().all(|c| c.is_ascii_digit())`) is sufficient.
- `wiremock` is already a dev-dependency — use the same version and import pattern as
  `tests/issue_edit_echo.rs` for new integration tests.
- `assert_cmd` + `predicates` are already present for binary-level assertions.
- `chrono::Utc` is already used in cache.rs for `fetched_at` timestamps.
- No version pins change; no `Cargo.toml` edits required.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `src/types/jira/editmeta.rs` | Create new | EditMeta, EditMetaField, EditMetaFieldSchema, AllowedValue; register in src/types/jira/mod.rs |
| `src/api/jira/issues.rs` | Modify | Add get_editmeta; ~30 LOC net |
| `src/api/jira/fields.rs` | Modify | Internal field-name matching helpers; ~20 LOC net |
| `src/cache.rs` | Modify | FieldsCache + read/write (best-effort writer); ~50 LOC net |
| `src/cli/mod.rs` | Modify | `field: Vec<String>` in IssueCommand::Edit; ~5 LOC net |
| `src/cli/issue/create.rs` | Modify | Gate B + C-1 extension + resolve_edit_fields call (2x: dry-run + live); ~80 LOC net |
| `src/cli/issue/helpers.rs` | Modify | resolve_edit_fields (~200 LOC); total file should stay below 1,100 LOC |
| `CLAUDE.md` | Modify | New Gotcha entry (F4 deliverable, in same PR as feature code); ~12 LOC net |
| `CHANGELOG.md` | Modify | [Unreleased] ### Added entry; ~10 LOC net |
| `tests/issue_edit_field.rs` | Create new | 33 integration tests (+ 1 unit); ~750 LOC net |
| `.factory/stories/STORY-INDEX.md` | Modify | Append S-396 row to Feature Followup table; update total_stories 45→46 and last_updated |

**Files NOT to create:** No `field_resolve.rs` (Option A: keep in helpers.rs per locked §2 Q5 decision;
extract only if helpers.rs exceeds 1,100 LOC after addition). No separate VP document files (VPs
are inlined in BC bodies per project convention). No new spec files — BC files are sealed.

## Branch / PR Plan

- Branch: `feat/issue-396-edit-field-flag`
- Target: `develop`
- Commit style: `feat(issue): add --field NAME=VALUE to issue edit for arbitrary custom fields (closes #396)`
- PR closes #396
- Cherry-pick `3dd6fdb` from `ci/issue-396-bc-cumulative-counts-surface-h` into this branch (AC-017)
- CHANGELOG entry required: user-visible new flag on `issue edit`

**Why `breaking_change: false`:** Additive flag on `issue edit`; existing invocations without
`--field` are byte-for-byte unchanged in behavior and latency (no `editmeta` call when `--field`
absent). The `list_fields()` call is also skipped when `--field` is absent. No previously-succeeding
invocation changes its exit code, stdout shape, or stderr content.

## Per-Story Delivery Notes

- F2 is CONVERGED (2026-05-22, pass 9) — BC files are sealed. Do NOT re-edit BC files unless
  the adversary finds a discrepancy between the BC body and the implementation.
- BC count surfaces were updated in F2. Both guard scripts (`check-spec-counts.sh` and
  `check-bc-cumulative-counts.sh`) MUST exit 0 post-edit. Surface H (end-of-file footer in
  bc-3-issue-write.md) is validated by the Surface-H extension in commit `3dd6fdb`.
- This story depends on S-398 (`changed_fields` BTreeMap in handle_edit). Confirm S-398 is
  merged to develop before starting F4 implementation.
- The `parse_field_kv` function from `handle_jsm_create` is `pub(crate)` — reuse it without
  modification. Do not duplicate the parsing logic.
- Per-story adversary 3/3 CLEAN required before push.
- Demos are LOCAL-ONLY per `docs/demo-evidence/` gitignore convention.

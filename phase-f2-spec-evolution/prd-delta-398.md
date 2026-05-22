---
document_type: prd-delta
issue: "#398"
title: "issue edit / issue create — changed-fields echo on success"
date: "2026-05-21"
phase: F2
spec_version_bump: "bc-3 total_bcs 97→100; definitional_count 68→71; BC-INDEX total_bcs 577→580"
new_bcs:
  - BC-3.4.012
  - BC-3.4.013
  - BC-3.4.014
modified_bcs:
  - BC-3.4.003
inputs:
  - .factory/phase-f1-delta-analysis/issue-398/delta-analysis.md
  - .factory/research/issue-398-field-echo-conventions.md
---

# PRD Delta — Issue #398: `issue edit` and `issue create` field-echo on success

## 1. Summary of Change

`jr issue edit KEY` currently prints only `Updated FOO-123` on success. When `--team` is used,
the partial-match resolution is silent — the user has no way to confirm which team was actually
set. This delta adds a "changed fields" echo to both table and JSON output for `issue edit`, and
adds a team-name echo to `issue create` table output.

## 2. Locked Design Decisions (human-gated)

The following decisions were locked before spec evolution. They are NOT re-litigated here.

| Decision | Value |
|----------|-------|
| Scope: `issue edit` echo | ALL changed fields echoed on single-key success path |
| Scope: `issue create` echo | ONLY `--team` resolved name echoed in table output |
| Table output channel | stderr (Symmetric profile 4, consistent with existing confirmation) |
| Description in table/human | `(updated)` marker only — content never echoed |
| Description in JSON | Raw `--description`/`--description-stdin` input string (no ADF→text round-trip; `src/adf.rs` converter NOT used) |
| `"updated": true` in JSON | RETAINED for backward compatibility |
| Team echo value | RESOLVED display name — never the UUID or partial-match query |
| Points value format | Rust `f64::to_string()` result (pinned by snapshot test) |

## 3. New Behavioral Contracts

### BC-3.4.012 — `issue edit` table-mode success echo

Single-key `jr issue edit KEY [flags...]` (table mode, no `--output json`):
- Existing: prints `Updated <key>` to stderr via `output::print_success`.
- New: prints one additional `  <field> → <value>` line per changed field to stderr.
- `--label` edits (single OR multi key) route through `handle_edit_bulk_labels` and are NOT covered by this contract; no `label` key appears in `changed_fields`.
- No field flags supplied → `handle_edit` bails with exit 1 before PUT; echo does not fire.
- `--dry-run` → short-circuits before PUT; echo does not fire.
- Fields echoed and their table-mode values:
  - `summary` → literal `--summary` value
  - `issue_type` → literal `--type` value
  - `priority` → literal `--priority` value
  - `parent` → issue key (from `--parent`); or `(cleared)` (from `--no-parent`). Key is always `parent`.
  - `points` → `f64::to_string()` result (from `--points`); or `(cleared)` (from `--no-points`). Key is always `points`.
  - `team` → RESOLVED display name (not UUID, not partial query). UUID-bypass predicate: 36-char, 8-4-4-4-12 ASCII hex groups, case-insensitive.
  - `description` → literal `(updated)` marker. Content never echoed in table mode.
- Map keys are always the literal lowercase identifiers (`summary`, `issue_type`, `priority`, `parent`, `points`, `team`, `description`) — never `customfield_*` IDs. The issue-type key is the literal `issue_type` (matching the Rust field identifier), NOT `type` and NOT `issuetype`.
- `--team` echo: RESOLVED team display name (not UUID, not partial-match query).
- `--description` / `--description-stdin` echo: literal `(updated)` marker.
- Stdout: empty. Exit code: 0.
- Scope: single-key path only (`effective_keys.len() == 1`, including `--jql` matching exactly one issue); bulk paths unaffected.

### BC-3.4.013 — `issue edit` JSON-mode success echo

Single-key `jr issue edit KEY [flags...] --output json`:
- Extends `edit_response` with `changed_fields` object: `{field_name: string_value}`.
- `"updated": true` RETAINED in payload.
- `changed_fields.team` is the RESOLVED display name.
- `changed_fields.description` is the **raw user-supplied input string** from `--description` / `--description-stdin` — NOT the `(updated)` marker and NOT an ADF→text round-trip. The raw string is lossless (exactly what the caller sent, before any `markdown_to_adf` conversion). This deliberate asymmetry with BC-3.4.012 (table shows `(updated)`) MUST NOT be "fixed."
- `--label` edits route through `handle_edit_bulk_labels` and are NOT covered; no `"label"` key in `changed_fields`.
- No field flags supplied → `handle_edit` bails with exit 1 before PUT; no JSON emitted.
- `--dry-run` → short-circuits before PUT; no JSON emitted from this contract.
- Cleared-field key model: key `"parent"` carries both `--parent` value and `--no-parent` `"(cleared)"`; key `"points"` carries both `--points` value and `--no-points` `"(cleared)"`. No separate `no_parent` / `no_points` keys.
- `edit_response` signature change: `pub(crate) fn edit_response(key: &str, changed_fields: &BTreeMap<String, String>) -> Value`. Uses `BTreeMap` (not `HashMap`) so JSON key order within `changed_fields` is deterministic (alphabetical). All values are JSON strings including numeric fields. **Top-level object key order note**: `serde_json::Map` serializes object keys in **alphabetical order** by default (the `preserve_order` feature is NOT enabled in this crate — confirmed in Cargo.toml). The top-level keys `changed_fields`, `key`, `updated` are already in alphabetical order, so the pinned snapshot body is `{"changed_fields": {...}, "key": "TEST-1", "updated": true}` regardless of the order they are written in the `json!{}` literal. The top-level key order is NOT contractually pinned beyond whatever the regenerated snapshot records. Only the INNER `changed_fields` key order is contractually alphabetical (guaranteed by `BTreeMap`).
- Insta snapshot `jr__cli__issue__json_output__tests__edit.snap` MUST be updated.
- Scope: single-key path only (`effective_keys.len() == 1`, including `--jql` matching exactly one issue); bulk paths unaffected.
- Stderr: empty. Exit code: 0.

### BC-3.4.014 — `issue create` table-mode team echo

`jr issue create [flags...] --team <name_or_uuid>` (table mode):
- Existing output: `Created issue FOO-123\nhttps://...`.
- New output: `Created issue FOO-123\n  team → Platform Core\nhttps://...`.
- Echo line appears between the "Created issue" confirmation and the browse URL.
- Echo value is the RESOLVED team display name from `resolve_team_field` 3-tuple return.
- When `--team` absent: output byte-for-byte identical to pre-#398 behavior.
- JSON output path (`--output json`) is unchanged.

## 4. Modified Behavioral Contracts

### BC-3.4.003 (annotation only)

Added success output cross-reference:
> "Success output: On the single-key success path (PUT 204), see BC-3.4.012 (table-mode success)
> and BC-3.4.013 (JSON-mode success). This contract specifies only the PUT wire contract."

No behavioral change to the PUT wire contract itself.

## 5. Signature Change: `resolve_team_field`

`src/cli/issue/helpers.rs::resolve_team_field`:
- Current: `-> Result<(String, String)>` returning `(field_id, team_id)`.
- Required: `-> Result<(String, String, String)>` returning `(field_id, team_id, team_name)`.

Call sites that must be updated:
1. `handle_edit` (create.rs ~line 794): destructure to `(field_id, team_id, resolved_team_name)` — use `resolved_team_name` in `changed_fields`.
2. `handle_create` (create.rs ~line 187): destructure to `(field_id, team_id, resolved_team_name)` — use `resolved_team_name` for the table-mode team echo line. **Note (O-4, round 7):** The superseded F1 delta-analysis §7 says bind the team name to `_resolved_team_name` (underscore-prefixed unused) — that guidance is SUPERSEDED; the create table path now USES the resolved name, so bind it as `resolved_team_name` (no underscore). **Note (OBS-1, round 11):** The resolved team name must outlive the `if let Some(ref team_name) = team` block to reach the post-POST table-output match arm — hoist it (e.g., a `let mut team_echo: Option<String> = None;` declared before the `if let`, assigned inside it). `handle_edit` has no equivalent issue because its echo BTreeMap is constructed across the whole function body.
3. `handle_list` (list.rs): The 3-tuple change does NOT affect line 161 (the `.await?` line — `Some(helpers::resolve_team_field(...).await?)` — which is unchanged). The actual destructure that must be updated is the closure at list.rs:167: `resolved_team.as_ref().map(|(field_id, team_uuid)| { ... })` becomes `.map(|(field_id, team_uuid, _resolved_team_name)| { ... })`. The underscore-prefixed third element is intentional because this path uses the `--team` flag only to build a JQL filter (it inserts `field_id`/`team_uuid` into the query) and does NOT echo the team name anywhere. **Do NOT apply the "use the resolved name for echo" guidance from sites 1 and 2 to this site.** The unused third element must be prefixed with an underscore (`_resolved_team_name`) to satisfy the `clippy -D warnings` zero-warning policy. Additionally, if there is an explicit type annotation on the `resolved_team` binding (e.g., `Option<(String, String)>`), it must be updated to `Option<(String, String, String)>`; if no explicit annotation exists, no annotation change is needed.

**Affected files** (all three call sites):
- `src/cli/issue/helpers.rs` — signature change
- `src/cli/issue/create.rs` — two sites: `handle_edit` (~line 794) and `handle_create` (~line 187)
- `src/cli/issue/list.rs` — one site: `handle_list` (line 167 closure `map(|(field_id, team_uuid)| ...)` → `map(|(field_id, team_uuid, _resolved_team_name)| ...)`); line 161 (`.await?`) is unchanged

**`edit_response` call sites (BC-3.4.013 signature change):**

The delta changes `edit_response`'s signature from `fn edit_response(key: &str) -> Value` to `fn edit_response(key: &str, changed_fields: &BTreeMap<String, String>) -> Value`. Every call site must be updated:

- **Production call site — `src/cli/issue/create.rs:910`** (inside `handle_edit`'s `OutputFormat::Json` arm): change `json_output::edit_response(key)` → `json_output::edit_response(key, &changed_fields)`. This is the SOLE production call site.
- **Test call site — `src/cli/issue/json_output.rs` (the `test_edit` / `test_edit_response_empty_changed_fields` unit tests)**: handled by the §6 snapshot-split note — the test must pass a non-empty `BTreeMap` and a separate test covers the empty case. `edit_response` is declared `pub(crate)`, so it is NOT callable from the `tests/` integration-test directory; there are NO call sites in `tests/`. The only callers are the production site `create.rs:910` and the inline unit tests in `json_output.rs`.

UUID-bypass predicate (`is_team_uuid` in `src/cli/issue/helpers.rs`): exactly 36 characters, in 8-4-4-4-12 hyphen-separated groups of ASCII hex digits (case-insensitive). Any string that does not satisfy this predicate exactly — including strings that resemble a UUID but differ in length or contain non-hex characters — routes through the normal partial-match path, not the UUID bypass.

**Third-element (`team_name`) per return path — all five return paths enumerated:**

| Branch | Condition | `team_id` (element 2) | `team_name` (element 3) |
|--------|-----------|----------------------|------------------------|
| UUID-bypass | `is_team_uuid(team_name)` returns `true` | the raw UUID string | the raw UUID string (same value; no lookup occurred) |
| `Exact` | partial_match returns `MatchResult::Exact(matched_name)` | `teams[idx].id` (looked up by position) | `matched_name` (the `MatchResult::Exact` matched display name) |
| `ExactMultiple` | partial_match returns `MatchResult::ExactMultiple(_)` AND user selects from prompt | `duplicates[selection].id` | `duplicates[selection].name` verbatim — the cached team's STORED casing, not the user's query casing |
| `Ambiguous` | partial_match returns `MatchResult::Ambiguous(matches)` AND user selects from prompt | `teams[idx].id` (found via position lookup on `selected_name`) | `teams[idx].name` verbatim — the cached team's STORED casing, not the user's query casing |
| `None` | partial_match returns `MatchResult::None(_)` | — (no 3-tuple produced; `Err` returned) | — (no 3-tuple produced; `Err` returned) |

Note on the `None` branch: `MatchResult::None(_)` returns `Err(JrError::UserError)` — no 3-tuple is produced, so the echo does not fire. The `None` variant carries a `Vec<String>` of candidate names (unused by this contract; the error message uses the stable substring `No team matching` regardless). The error TEXT emitted by `resolve_team_field` varies by `fetched_fresh` (cold-fetch variant vs disk-cache variant): both contain the stable substring `No team matching`, but the full message differs. Tests MUST assert only the stable substring `No team matching` — never the full message literal. `ExactMultiple` / `Ambiguous` with `--no-input` also return `Err` (exit 64) before the echo fires.

## 6. Implementation Notes

### `changed_fields` map construction in `handle_edit`

**Add a NEW declaration** at the top of `handle_edit`, alongside the existing `fields` JSON object (the wire payload):

```rust
let mut changed_fields: BTreeMap<String, String> = BTreeMap::new();
```

No equivalent map exists today — only `fields`, the Jira REST wire JSON. The `changed_fields` map is entirely new and must be populated in parallel with `fields` as each user-supplied option is resolved and translated into a wire field. Alphabetical key order in JSON output is guaranteed by `BTreeMap`. All values are stored as strings (including numeric fields such as `points`). The canonical key→value mapping:

```
description      → raw user-supplied input string from --description / --description-stdin
                   [table-mode echo suppresses this in favor of the "(updated)" marker]
issue_type       → literal --type value
parent           → two distinct insertion sites (see MAJOR-1 enumeration below)
points           → two distinct insertion sites (see MAJOR-1 enumeration below)
priority         → literal --priority value
summary          → literal --summary value
team             → resolved_team_name (3rd element of resolve_team_field return)
```

**MAJOR-1 — Two-site insertion enumeration for `parent` and `points`:**

The `parent` key is inserted at TWO disjoint code branches — both produce the same map
key `"parent"` but via separate logic:

- **`--parent <key>` branch** (`if let Some(parent_key) = parent`): at this site, the
  handler has a `String` value in scope; inserts `"parent" → parent_key_string`.
- **`--no-parent` branch** (`if no_parent`): at this site, there is no issue-key value
  in scope; inserts `"parent" → "(cleared)"`.

The `points` key is likewise inserted at TWO disjoint code branches:

- **`--points <n>` branch** (`if let Some(pts) = points`): at this site, the handler has
  an `f64` in scope; inserts `"points" → pts.to_string()`. The `.to_string()` on `f64`
  applies ONLY at this site (e.g., `"5"` for `5.0`, `"2.5"` for `2.5`).
- **`--no-points` branch** (`if no_points`): at this site, there is no numeric value in
  scope; inserts `"points" → "(cleared)"`. The `.to_string()` invariant does NOT apply
  here — the value is the literal string `"(cleared)"`.

No separate `no_parent` / `no_points` keys are ever inserted; the cleared-field model
uses the same key (`parent`, `points`) with the marker value `"(cleared)"`.

DECISION LOCKED (M-2): `changed_fields["description"]` carries the **raw `desc_text`** —
the user's literal `--description` / `--description-stdin` input string — NOT an
ADF→text round-trip. Rationale: lossless (no `adf_to_text` call); the table-mode echo
suppresses it in favor of the `(updated)` marker at the display layer.

DECISION LOCKED (MED-1): Cleared-field key model uses single map keys. `parent` covers
both `--parent` (value = issue key) and `--no-parent` (value = `"(cleared)"`). `points`
covers both `--points` (value = number string) and `--no-points` (value = `"(cleared)"`).
No separate `no_parent` / `no_points` keys exist.

### Import note: HashMap → BTreeMap (IMP-2)

`src/cli/issue/create.rs` currently has `use std::collections::HashMap;` at line 1.
Switching `changed_fields` to `BTreeMap` requires adding `use std::collections::BTreeMap;`.
The implementer MUST check whether `HashMap` remains used elsewhere in `create.rs` after
the change — if `HashMap` is no longer used, its import must be removed to satisfy the
`clippy -D warnings` zero-warning policy (unused import warning). Do not suppress the
warning with `#[allow(unused_imports)]`; remove the unused import instead.

### Description asymmetry documentation

A one-line code comment at the description-echo sites AND a CLAUDE.md Gotchas entry
MUST be added explaining the deliberate asymmetry. This prevents a future maintainer
from "fixing" table and JSON to match each other.

**DECISION LOCKED (round 5 F-4) — pinned Gotcha text (copy-paste exactly into CLAUDE.md Gotchas section):**

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

**Required acceptance criterion for the F3 implementation story:** "CLAUDE.md Gotchas section gains the pinned description-echo-asymmetry entry above verbatim." This is a tracked deliverable — the story MUST NOT close without the Gotcha entry present in CLAUDE.md.

### Snapshot test impact

`src/cli/issue/snapshots/jr__cli__issue__json_output__tests__edit.snap` pins
`{"key": "TEST-1", "updated": true}`. This MUST be regenerated.

**Pinned regenerated snapshot body (DECISION LOCKED — round 10 MAJOR-1)**: The updated
`test_edit` test calls `edit_response("TEST-1", &<BTreeMap with "summary" → "New title">)`.
Because the map is a `BTreeMap`, keys are sorted alphabetically. The exact expected
regenerated snapshot content is:

```json
{
  "changed_fields": {
    "summary": "New title"
  },
  "key": "TEST-1",
  "updated": true
}
```

Note: `changed_fields` sorts before `key` before `updated` in the top-level object because `serde_json::Map` serializes object keys in **alphabetical order** by default (the `preserve_order` feature is NOT enabled in this crate — confirmed in Cargo.toml). The top-level keys `changed_fields`, `key`, `updated` are already in alphabetical order, so the pinned snapshot body is `{"changed_fields": {...}, "key": "TEST-1", "updated": true}` regardless of the order they are written in the `json!{}` literal. The implementer
MUST regenerate the snapshot using `cargo test -- --update` (or `INSTA_UPDATE=always
cargo test`) and verify the pinned body above matches what insta records. If the
serialization order differs, the pinned body above MUST be updated to match the actual
snapshot before merging — a wrong-but-stable snapshot is a HIGH severity finding.

**Snapshot test split (DECISION LOCKED — round 7 F-3)**:
- The existing `test_edit` unit test in `src/cli/issue/json_output.rs` covers the **NON-EMPTY** `changed_fields` case. It MUST be updated to pass a non-empty `BTreeMap` (e.g., `{"summary": "New title"}`). The insta snapshot file `jr__cli__issue__json_output__tests__edit.snap` stays at its current filename — it MUST NOT be renamed (the "existing no-prefix tests are not renamed" convention from CLAUDE.md applies: `test_edit` has no prefix and is grandfathered).
- A NEW test `test_edit_response_empty_changed_fields` MUST be added (applying the new-test `test_<verb>_<subject>_<expected_outcome>` naming convention). This test covers the **empty** `BTreeMap` case — it calls `edit_response` with an empty `BTreeMap<String, String>` and asserts the resulting JSON has `"updated": true` and `"changed_fields": {}`. This new test does NOT use an insta snapshot.

## 7. Count Surfaces Updated

| Surface | Before | After |
|---------|--------|-------|
| bc-3-issue-write.md `total_bcs` | 97 | 100 |
| bc-3-issue-write.md `definitional_count` | 68 | 71 |
| bc-3-issue-write.md body preamble | "97 behavioral contracts" | "100 behavioral contracts" |
| BC-INDEX.md `total_bcs` frontmatter | 577 | 580 |
| BC-INDEX.md Section 3 header | "97 BCs cumulative; 68 individually-bodied" | "100 BCs cumulative; 71 individually-bodied" |
| BC-INDEX.md Section 3.4 header | "11 BCs: BC-3.4.001..011" | "14 BCs: BC-3.4.001..014" |
| BC-INDEX.md Coverage Statistics table (Section 3 row) | `97 \| 68` | `100 \| 71` |
| BC-INDEX.md Coverage Statistics table (Total row) | `577 \| 345` | `580 \| 348` |
| BC-INDEX.md Coverage Statistics prose ("Canonical total is ...") | "577" | "580" |
| BC-INDEX.md Coverage Statistics prose ("+N enumeration") | ends "...+2 BC-3.4.010..011 added 2026-05-20 via issue #388 F2" | appended "+3 BC-3.4.012..014 added 2026-05-21 via issue #398 F2" |
| BC-INDEX.md Coverage Statistics prose ("Cumulative total (NNN) ≠ ... (NNN)") | "577 / 345" | "580 / 348" |
| CANONICAL-COUNTS.md per-file table bc-3 definitional | 68 | 71 |
| CANONICAL-COUNTS.md per-file table bc-3 total_bcs | 97 | 100 |
| CANONICAL-COUNTS.md Sum row | 577 | 580 |
| CANONICAL-COUNTS.md individually-bodied total | 345 | 348 |
| CANONICAL-COUNTS.md grand total | 577 | 580 |
| CANONICAL-COUNTS.md Breakdown bullet (BC-X.4.009 "in the NNN sum") | "577 sum" | "580 sum" |
| CANONICAL-COUNTS.md frontmatter `last_verified` | `"2026-05-20 ..."` | `"2026-05-21 (F2 delta issue #398; +3 BC-3.4.012..014; bc-3 definitional 68→71 total_bcs 97→100)"` |

**[process-gap]**: `scripts/check-bc-cumulative-counts.sh` currently guards 8 count surfaces. The BC-INDEX.md Coverage Statistics body table (Section 3 row + Total row + prose) is a **9th surface** with no automated guard. Any BC count change requires a manual update to this table. This gap should be addressed by extending the guard script in a future maintenance pass. Tracked here for the orchestrator's awareness.

**[process-gap]**: The count-surface enumeration checklist for PRD deltas should include: every numeric BC-total literal in CANONICAL-COUNTS.md Breakdown bullets, the CANONICAL-COUNTS.md frontmatter `last_verified` field, and the BC-INDEX.md Coverage Statistics table (Section file row + Total row + prose). Omitting these surfaces allows stale literals to survive adversarial review. The orchestrator should add these items to the delta-authoring checklist for future F2 cycles.

**[process-gap]**: The verification-delta `new_vps:` frontmatter array length, the count of `### VP-` headings in the verification-delta body, and the row count of the VP-to-BC mapping table at the end of the verification-delta document must agree. There is no automated guard for this invariant. The delta-authoring checklist should require: `new_vps:` frontmatter length == `### VP-` heading count == VP-to-BC mapping-table row count. Any mismatch indicates a VP was added or removed without updating all three surfaces. Tracked here for the orchestrator's awareness.

## 8. Changelog Entry

**v0.7 / issue #398 (2026-05-21):**
- `issue edit`: single-key success now echoes changed fields to stderr (table mode) and
  includes `changed_fields` in JSON output. `--team` shows resolved display name.
  Description shown as `(updated)` marker in table mode; raw user-supplied input string in JSON.
- `issue create`: `--team` resolved name now echoed in table mode between "Created issue"
  and browse URL.
- `resolve_team_field` return type changed to 3-tuple `(field_id, team_id, team_name)`.
  All THREE call sites updated: `handle_edit` (create.rs ~794), `handle_create` (create.rs ~187),
  and `handle_list` (list.rs ~161). The `list.rs` site destructures to
  `(field_id, team_uuid, _resolved_team_name)` with an underscore-prefixed unused third element
  because the `--team` JQL-filter path does not echo the team name.

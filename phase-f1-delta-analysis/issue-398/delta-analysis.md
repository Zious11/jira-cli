# Phase F1 Delta Analysis — Issue #398
# `issue edit` confirmation echo of changed fields

**Date**: 2026-05-21
**Status**: Draft — awaiting human gate

---

## 1. Problem Statement

`jr issue edit` succeeds silently. The confirmation tells the user WHICH issue was
updated but not WHAT was changed. For `--team`, this is especially harmful: team names
resolve via partial match (`resolve_team_field` in `helpers.rs`) so `--team plat` could
silently resolve to "Platform Core", "Platform Infra", or any other partial-match winner.
The user has no way to confirm which team was actually set without running `jr issue view`
afterward.

Current success output (single-key path, table mode):

```
Updated FOO-123        ← stderr via output::print_success; green-coloured
```

Current success output (single-key path, JSON mode, via `json_output::edit_response`):

```json
{"key": "FOO-123", "updated": true}
```

Neither surface tells the user which fields were changed or what values they resolved to.

---

## 2. Scope

### In scope (approved)

- **Single-key `issue edit` success path** — echo all changed fields with their resolved
  values in both table and JSON output.
- **`--team` specifically** — surface the RESOLVED team name (not the user's partial-match
  query, not the UUID). `resolve_team_field` must return the matched name alongside the
  existing `(field_id, team_id)` tuple.
- **`issue create` table output** — assess the same gap (see section 8).

### Out of scope

- Multi-key bulk path (`handle_edit_bulk_fields`, `handle_edit_bulk_labels`) — these go
  through a different code path and already emit per-key `"Updated <key>"` lines. The bulk
  results payload (`render_bulk_edit_results`) is a separate concern.
- Dry-run path — already echoes planned changes explicitly; no gap here.
- `issue create` JSON output — already returns the full issue object via follow-up GET.

---

## 3. Current Behavior (Detailed)

### Single-key success path (`handle_edit`, lines 906-916, `create.rs`)

```rust
match output_format {
    OutputFormat::Json => {
        println!(
            "{}",
            serde_json::to_string_pretty(&json_output::edit_response(key))?
        );
    }
    OutputFormat::Table => {
        output::print_success(&format!("Updated {}", key));
    }
}
```

`json_output::edit_response` (line 42, `json_output.rs`):

```rust
pub(crate) fn edit_response(key: &str) -> Value {
    json!({
        "key": key,
        "updated": true
    })
}
```

`output::print_success` (line 45, `output.rs`) writes to **stderr** (green text via
`eprintln!`). The JSON path writes to **stdout** via `println!`.

### `resolve_team_field` return signature (line 36, `helpers.rs`)

```rust
pub(super) async fn resolve_team_field(
    config: &Config,
    client: &JiraClient,
    team_name: &str,
    no_input: bool,
) -> Result<(String, String)>   // (field_id, team_id)
```

The matched team name is computed internally (line 108-112: `teams[idx].id.clone()` is
returned but `matched_name` is dropped). The caller in `handle_edit` (lines 793-795)
discards the name:

```rust
let (field_id, team_id) =
    helpers::resolve_team_field(config, client, team_name, no_input).await?;
fields[&field_id] = json!(team_id);
```

---

## 4. Desired Behavior

### Table mode

After a successful single-key edit, emit the key confirmation PLUS a field-change summary
to **stderr** (consistent with the Symmetric output-channel profile). Format:

```
Updated FOO-123
  summary → "New title"
  priority → High
  team → Platform Core   ← resolved team NAME, not UUID
  description → (updated)
```

The description field is special: descriptions are long ADF blobs. A brief marker
`(updated)` is appropriate; full content truncation (like the dry-run path at 60 chars)
is also acceptable.

### JSON mode

Extend the `edit_response` payload to include a `changed_fields` map of human-readable
field name → resolved value. Send to **stdout** (as now). Example:

```json
{
  "key": "FOO-123",
  "updated": true,
  "changed_fields": {
    "summary": "New title",
    "priority": "High",
    "team": "Platform Core",
    "description": "(updated)"
  }
}
```

`changed_fields` uses human-readable keys (`"summary"`, `"team"`, `"description"`) — NOT
internal Jira field IDs (`"customfield_10001"`). The team entry specifically shows the
resolved team NAME, not the UUID.

---

## 5. Output-Channel Decision

### Which profile applies?

CLAUDE.md defines five profiles. The current `issue edit` success path writes confirmation
to **stderr** (table mode) and success data to **stdout** (JSON mode). This is the
**Symmetric** profile: stdout for `--output json`, stderr for human-readable in either mode.

The confirmed field echo follows the same profile:

- **Table mode**: all output (key + changed fields) goes to **stderr**. This is correct
  for a state-changing command — diagnostics/confirmation never pollute stdout.
- **JSON mode**: extended `edit_response` payload goes to **stdout** as valid JSON.
  No additional stderr output.

This is NOT the "No-log facade" profile (profile 5), because `issue edit` already shows
a confirmation message in table mode.

The changed-field echo belongs in the **same channel as the current confirmation**:
stderr in table mode, stdout payload extension in JSON mode.

---

## 6. Affected Behavioral Contracts

### Existing BCs that govern `issue edit` output

**BC-3.4.003** (`issue edit` PUTs and accepts 204): The success-path output is described
only in terms of "Updated \<key\>" in the source reference. This BC will require a note
that the success output now includes changed fields. The BC body needs annotation.

**BC-3.4.004, BC-3.4.005** (ADF description, multiple fields): Both reference the wire
format. No output assertion changes. These BCs are unaffected by the output change.

**BC-3.4.010, BC-3.4.011** (error paths for `--type` 400): Completely unaffected; these
are error paths and the changed-fields echo only applies on the success path.

### New BCs required

Two new BCs must be added to `bc-3-issue-write.md` under subdomain 3.4:

**BC-3.4.012** (proposed): `issue edit KEY` success, table mode — stderr contains
"Updated \<key\>" followed by one line per changed field in `field → value` format. For
`--team`, the value is the RESOLVED team name. For `--description` / `--description-stdin`,
the value is the marker `(updated)`.

**BC-3.4.013** (proposed): `issue edit KEY` success, JSON mode — stdout JSON contains
`{"key": ..., "updated": true, "changed_fields": {...}}`. The `changed_fields` object
maps human-readable field names to resolved string values. For `--team`, the value is the
RESOLVED team name. For `--description`, the value is `"(updated)"`. The `"updated"` key
remains for backward compatibility.

Adding these BCs increases `bc-3` `total_bcs` and `definitional_count` by 3 (BC-3.4.012, BC-3.4.013, BC-3.4.014 — the third was added after the human gate approved `issue create` table echo as in-scope; see §9 annotation).
`BC-INDEX.md` and `CANONICAL-COUNTS.md` must be updated after BC addition.

---

## 7. Signature Change: `resolve_team_field`

Current: `-> Result<(String, String)>` returns `(field_id, team_id)`.

Required: `-> Result<(String, String, String)>` returning `(field_id, team_id, team_name)`.

All call sites must be updated:

1. `src/cli/issue/create.rs` — `handle_edit` (line 793): destructure to
   `(field_id, team_id, resolved_team_name)`.
2. `src/cli/issue/create.rs` — `handle_create` (line 186): destructure to
   `(field_id, team_id, _resolved_team_name)` — the name is not used in create output
   (JSON output does a follow-up GET; table output already says "Created issue KEY" only).
   The name can be captured for a future `issue create` echo enhancement.

> [SUPERSEDED by prd-delta-398.md §5 Note O-4: handle_create now USES resolved_team_name for the team echo; bind as `resolved_team_name` (no underscore).]

3. `src/cli/issue/list.rs` — `handle_list` (~line 161): destructure to `(field_id, team_uuid, _resolved_team_name)` — the `--team` JQL-filter path does NOT echo the team name; the unused third element must be underscore-prefixed. [CORRECTED — adversary round 10 CRITICAL-1]: the original F1 search falsely claimed "these are the only two call sites." A third call site exists at `src/cli/issue/list.rs` (~line 161, consumed ~line 167) for the `--team` JQL filter in `jr issue list`. The 2-tuple destructure at that site would fail to compile when `resolve_team_field` returns a 3-tuple. Confirmed by `grep -rn "resolve_team_field" src/`: three call sites in total (create.rs:187, create.rs:794, list.rs:161).

The UUID pass-through path in `resolve_team_field` (line 64) currently returns the UUID
as both the ID and the display value. After the change, it must return `(field_id, uuid,
uuid)` — using the raw UUID as the "name" when the caller passed a UUID directly. This
is acceptable since the caller already knew the UUID.

---

## 8. JSON Shape Decision for `edit_response`

> **[SUPERSEDED]** The `HashMap<String, String>` type shown in Option A below is superseded by
> `BTreeMap<String, String>` (per prd-delta-398.md §6 / MED-1). The separate `no_parent` and
> `no_points` keys listed in the canonical key→value mapping below are superseded by the
> single `parent` and `points` keys with `"(cleared)"` value (prd-delta-398.md §6 MED-1). The
> implementation MUST use `BTreeMap`, not `HashMap`. The `no_parent`/`no_points` key model is
> never implemented. See prd-delta-398.md §6 MAJOR-1 for the authoritative two-site insertion
> enumeration. Additionally, the `description` key in `changed_fields` for JSON mode carries
> the raw user-supplied input string (NOT `"(updated)"`). The `(updated)` marker applies only
> to table/human output (BC-3.4.012). See prd-delta-398.md §6 DECISION LOCKED M-2 and
> BC-3.4.013.

Option A (chosen): Extend `edit_response` to accept a `changed_fields` parameter.

```rust
pub(crate) fn edit_response(key: &str, changed_fields: &HashMap<String, String>) -> Value {
    json!({
        "key": key,
        "updated": true,
        "changed_fields": changed_fields,
    })
}
```

This keeps backward-compatible fields (`"key"`, `"updated"`) while adding the new map.
Downstream consumers using `jq '.key'` or `.updated` are unaffected.

Option B (rejected): A separate `edit_response_with_fields` function. This creates
unnecessary duplication since the original `edit_response` would be dead code.

The `changed_fields` map uses `String` values for all field types:
- `summary` → verbatim string value
- `priority` → verbatim string value (e.g., `"High"`)
- `issue_type` → verbatim string value
- `parent` → issue key string (e.g., `"FOO-99"`)
- `no_parent` → `"(cleared)"`
- `points` → numeric value converted to string (e.g., `"5"`)
- `no_points` → `"(cleared)"`
- `team` → RESOLVED team name (not UUID, not user's partial query)
- `description` / `description_stdin` → `"(updated)"` (content is an ADF blob, not
  suitable for echo)

### Snapshot test impact

The snapshot `jr__cli__issue__json_output__tests__edit.snap` pins the current shape:
```json
{"key": "TEST-1", "updated": true}
```
This snapshot MUST be updated. `insta`'s `--update` flag regenerates it. The test
itself (`test_edit` in `json_output.rs`) must be updated to pass a non-empty
`changed_fields` and `HashMap::new()` (empty map) to test the two cases.

---

## 9. `issue create` Table Output Assessment

> **[SUPERSEDED by locked human-gate decision 2026-05-21: `issue create` table-mode team echo is IN SCOPE — see BC-3.4.014 / prd-delta-398.md §2.]**
> The recommendation below was the original F1 analysis. After the human gate approved `issue create` table echo as in-scope, BC-3.4.014 was added as a new behavioral contract. The "OUT OF SCOPE" recommendation below no longer applies.

**Current create table output** (line 259, `create.rs`):

```rust
OutputFormat::Table => {
    output::print_success(&format!("Created issue {}", response.key));
    eprintln!("{}", browse_url);
}
```

The create table output already shows the browse URL — a useful second line. Adding a
`changed_fields` echo here is lower priority than for `edit`:

- Create always requires summary + issue type (mandatory), so there is less ambiguity.
- The JSON path already does a follow-up GET and returns the full Issue object.
- The only ambiguous resolution is `--team` (same partial-match concern as edit).

**Original recommendation (superseded)**: `issue create` table output was recommended as OUT OF SCOPE for this issue. This recommendation was overridden at the human gate. BC-3.4.014 implements the `--team` echo for `issue create` table mode.

---

## 10. Regression Risks

### Existing tests asserting the current output shape

The following locations assert on the current minimal output and must be updated:

1. **`src/cli/issue/snapshots/jr__cli__issue__json_output__tests__edit.snap`** — pins
   `{"key": "TEST-1", "updated": true}`. Must be regenerated after signature change.

2. **`src/cli/issue/json_output.rs`** — `test_edit()` snapshot test calls
   `edit_response("TEST-1")`. Must pass a `changed_fields` argument.

3. **`tests/issue_edit_no_parent.rs`** (lines 384-397) — the JSON path test asserts
   `parsed["key"] == "FOO-100"` and `parsed` is valid JSON. The `"key"` assertion is
   safe after the extension; `"updated"` is not asserted there, so adding `changed_fields`
   does not break this test. No changes needed.

4. **`tests/issue_edit_no_parent.rs`** (lines 130-145) — the table success path asserts
   `exit 0`. No stdout/stderr content assertion on the success line. No changes needed.

5. **`tests/issue_edit_type_errors.rs`** — all tests are on error paths. No success
   output is asserted. No changes needed.

6. **`tests/issue_commands.rs`** (lines 609-727) — `test_edit_issue_with_description`,
   `test_edit_issue_with_markdown_description`, `test_edit_issue_description_with_other_fields`
   — these are client-level unit tests that call `client.edit_issue(...)` directly, NOT
   the CLI handler. They assert on the wire body shape only, not on stdout/stderr. No
   changes needed.

### `resolve_team_field` call sites

Both call sites in `create.rs` must destructure the new 3-tuple. Any test that exercises
`handle_create` or `handle_edit` with `--team` must be checked. Currently the team-flag
test coverage appears to be in integration tests using the binary; these will pass as long
as the team resolution still succeeds.

---

## 11. Implementation Approach

> **[SUPERSEDED]** The `HashMap<String, String>` type in step 3 below is superseded by
> `BTreeMap<String, String>` (per prd-delta-398.md §6 / MED-1 / MAJOR-1). The separate
> `no_parent → "(cleared)"` and `no_points → "(cleared)"` keys in the map construction
> below are superseded: use single keys `parent` and `points` with value `"(cleared)"` at
> the respective `if no_parent` / `if no_points` branches. See prd-delta-398.md §6
> MAJOR-1 for the authoritative two-site insertion enumeration for `parent` and `points`.
> Additionally, the `description` key in `changed_fields` for JSON mode carries the raw
> user-supplied input string (NOT `"(updated)"`). The `(updated)` marker applies only to
> table/human output (BC-3.4.012). See prd-delta-398.md §6 DECISION LOCKED M-2 and
> BC-3.4.013.

### Phase ordering

1. Change `resolve_team_field` signature to return `(field_id, team_id, team_name)`.
2. Update both call sites in `create.rs` to destructure the 3-tuple.
3. Build the `changed_fields: BTreeMap<String, String>` in `handle_edit` (single-key path
   only) as fields are resolved, using human-readable keys. (NOTE: use `BTreeMap`, not
   `HashMap` — see superseded note above.)
4. Change `json_output::edit_response` to accept `&BTreeMap<String, String>` for the
   changed fields.
5. Update the table-mode success output to echo the changed fields to stderr.
6. Update the snapshot test and the `test_edit` unit test.
7. Add new integration tests for BC-3.4.012 and BC-3.4.013.

### Where the `changed_fields` map is built

In `handle_edit`, after each field is resolved, insert into the map:

```
summary          → summary value
issue_type       → issue_type value
priority         → priority value
parent           → parent key string          (if let Some(parent_key) = parent branch)
parent           → "(cleared)"               (if no_parent branch — SUPERSEDES no_parent key)
points           → pts.to_string()            (if let Some(pts) = points branch)
points           → "(cleared)"               (if no_points branch — SUPERSEDES no_points key)
team             → resolved_team_name   ← from new 3rd return value
description      → "(updated)"          ← when either description or description_stdin is used
```

The map is passed to `edit_response` in JSON mode and iterated for table-mode stderr output.

### Pure-function design (verifiability)

The `changed_fields` map construction is pure (no I/O after resolve), making it
straightforwardly testable with proptest or unit tests. The `json_output::edit_response`
function remains pure.

---

## 12. Open Questions for Human Gate

1. **Description value**: Should `(updated)` be the marker string for description
   changes, or is a short truncated preview (first 60 chars, as on the dry-run path)
   preferable in table mode? The dry-run path truncates to 60 chars; a consistent UX
   would do the same here.

2. **`issue create` scope**: Confirm that `issue create` table output is OUT OF SCOPE
   for #398 and will be tracked separately.

3. **Backward compatibility of `"updated": true`**: Should `"updated": true` be RETAINED
   in the JSON payload (backward-compat shim for existing scripts) or removed in favour of
   just `"changed_fields"`? Recommendation is retain for compatibility.

4. **Points value formatting**: For `--points`, should the `changed_fields` value be the
   raw float string (e.g., `"5"`, `"2.5"`) or an integer where applicable? Using
   `.to_string()` on an `f64` may produce `"5"` or `"5.0"` depending on Rust defaults.
   Recommend using the exact string Rust produces from `.to_string()` and pinning it in
   the snapshot.

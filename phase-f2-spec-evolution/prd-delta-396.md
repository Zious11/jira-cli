---
document_type: prd-delta
issue: "#396"
title: "issue edit --field NAME=VALUE — arbitrary custom field editing via editmeta"
date: "2026-05-22"
phase: F2
spec_version_bump: "bc-3 total_bcs 100→103; definitional_count 71→74; BC-INDEX total_bcs 580→583"
new_bcs:
  - BC-3.4.015
  - BC-3.4.016
  - BC-3.4.017
modified_bcs: []
inputs:
  - .factory/phase-f1-delta-analysis/issue-396/delta-analysis.md
  - .factory/research/issue-396-jsm-fields-validation.md
---

# PRD Delta — Issue #396: `issue edit --field NAME=VALUE`

## 1. Summary of Change

`jr issue edit` currently has no mechanism to set arbitrary custom fields on an
existing issue. Dedicated flags (`--summary`, `--type`, `--priority`, `--team`,
`--points`, `--parent`, `--description`) cover first-party Jira fields only. Any
custom field — including JSM request-type-scoped select fields like Urgency and
Impact — is unreachable via `jr issue edit` today.

This delta adds a `--field NAME=VALUE` flag (repeatable) to `issue edit`, enabling
callers to set any custom field that appears on an issue's agent Edit screen. The
mechanism routes through the existing platform `PUT /rest/api/3/issue/{key}` endpoint
(same path `issue edit` already uses), extended with a new `GET /rest/api/3/issue/{key}/editmeta`
call for field-presence validation and single-select option resolution.

Research source: `.factory/research/issue-396-jsm-fields-validation.md` (all five
research questions answered at HIGH confidence). The JSM create side (`--request-type`
dispatching to `POST /rest/servicedeskapi/request`) is already shipped and is NOT
changed by this delta.

## 2. Locked Design Decisions (human-gated)

The following decisions were locked at the F1 gate. They are NOT re-litigated here.

| Decision | Value |
|----------|-------|
| Scope: `--field` on `issue edit` | Single-key path only — multi-key positional or `--jql` matching 2+ issues → exit 64 |
| Flag overlap detection | `--summary X --field Summary=Y` (or any other dedicated-flag + `--field` targeting the same system field) → hard error exit 64, NO HTTP call |
| Flag overlap scope | Covers exactly four first-party system-field overlaps: `summary`, `description`, `issuetype` (for `--type`), `priority`. Team and points use dynamically-resolved IDs; overlap detection for those is deferred to v2. |
| Field-name resolution | Case-insensitive exact match first, then substring match against `list_fields()`. `customfield_NNNNN` literals bypass name lookup entirely. |
| v1 supported field types | `string`, `number`, `option` (single-select → `{"id": "<optionId>"}`), `date` (pass-through), `datetime` (pass-through), `user` (pass-through as `{"accountId": VALUE}`) |
| Unsupported types | `array` (multi-select) and `any`/CMDB types → exit 64 with actionable hint |
| Date/datetime validation | No client-side ISO 8601 validation; server-side validation only |
| User fields | Caller supplies raw `accountId`; "me" resolution deferred to v2 |
| `editmeta` caching | Not cached in v1 — called once per `issue edit` invocation when `--field` is set, skipped when absent |
| Changing Request Type | NOT supported. Declared non-goal; no reliable Jira Cloud API exists (JSDCLOUD-4609, open since 2016). Attempting PUT of `sd-customerrequesttype` is known to return HTTP 500. |
| `changed_fields` echo for `--field` | Each resolved `--field` pair is inserted into the existing `changed_fields` BTreeMap using the human field name as the key and the resolved display value as the value (for option fields: the matched `allowedValues[].value`, not the id). Consistent with BC-3.4.012/013. |
| `resolve_edit_fields` placement | Lives in `src/cli/issue/helpers.rs` — the existing helper module for `handle_edit`-related resolutions (team/points lookup, user resolution, prompts). This silently resolves F1 open-question Q5 (which file owns the orchestration function). [OBS-1] |

## 3. New Behavioral Contracts

### BC-3.4.015 — `issue edit KEY --field NAME=VALUE` (string/number field, single-key path)

When `NAME` refers to a `string` or `number` field (or `date`, `datetime`, `user`
pass-through types), the handler:

1. If `NAME` matches `customfield_\d+`: bypass `list_fields()` lookup; use `NAME`
   as the field ID directly.
2. Otherwise: call `list_fields()` (`GET /rest/api/3/field`). Perform case-insensitive
   exact match, then substring match. Zero matches → exit 64, UserError with actionable
   hint. Multiple matches → exit 64, UserError naming the ambiguous candidates.
3. Call `get_editmeta(key)` (`GET /rest/api/3/issue/{key}/editmeta`). If the resolved
   field ID is absent from `editmeta.fields` → exit 64, UserError with Edit-screen hint.
4. Serialize `VALUE` per `schema.type`:
   - `string`/`text`: bare JSON string
   - `number`: parse as `f64`, error if not parseable
   - `date`/`datetime`: bare string (server validates)
   - `user`: `{"accountId": VALUE}`
   - `array`/`any`/unknown: exit 64 with hint naming the unsupported type
5. Merge into the shared `fields` JSON object and call `client.edit_issue(key, fields)`.
6. On PUT 204: insert `(human_name_or_id, VALUE)` into `changed_fields` and emit the
   standard success echo (BC-3.4.012 table mode, BC-3.4.013 JSON mode).

`customfield_NNNNN` literals still pass through `get_editmeta()` (Step 3) — the bypass
only skips the `list_fields()` name-resolution step.

Exit 0 on success. All error paths → exit 64.

### BC-3.4.016 — `issue edit KEY --field NAME=VALUE` (single-select `option` field)

When `editmeta` reports `schema.type == "option"`:

1. Resolve the option value: if `VALUE` matches an `allowedValues[].id` exactly
   (numeric string) → use that id as-is. Otherwise perform case-insensitive exact
   match on `allowedValues[].value`, then substring match.
   - Zero matches → exit 64, UserError listing allowed values.
   - Multiple substring matches → exit 64, UserError listing ambiguous matches with ids.
   - Empty `allowedValues` → exit 64, UserError ("field has no configured option values").
2. Wire payload: `{"customfield_NNNNN": {"id": "<optionId>"}}`.
3. `changed_fields` echo value: the matched `allowedValues[].value` (human label), NOT
   the option `id`. This keeps both the table-mode echo (`  Urgency → High`) and the
   JSON `changed_fields` object human-readable.

Preconditions and postconditions from BC-3.4.015 apply for the non-option steps
(field-name resolution, `editmeta` fetch, PUT 204, success echo).

### BC-3.4.017 — `--field` multi-key/`--jql` rejection + flag-overlap hard error

Two enforcement gates that fire before any HTTP call:

**Gate A — multi-key/`--jql` rejection (C-1 guard):**
When `--field` is provided alongside 2+ positional keys, or when `--jql` resolves
to 2+ issues, the C-1 rejection block fires. Error message follows the existing
pattern for other bulk-rejected flags (e.g., `--parent`, `--team`,
`--description`): "Multi-key bulk edit doesn't yet support: `--field`. Use a single
key, or open an issue if this matters for your workflow." Exit 64.

`--jql` resolving to exactly ONE issue is NOT rejected (consistent with existing
single-match `--jql` fast path for other flags).

**Gate B — flag-overlap hard error:**
If a dedicated flag and `--field` target the same system field in the same
invocation:
- `--summary X --field summary=Y` (or `--field Summary=Y`, case-insensitive)
- `--description X --field description=Y`
- `--type X --field issuetype=Y` (note: `--type` maps to the `issuetype` system field key)
- `--priority X --field priority=Y`

→ exit 64 with: `"<Field> is set by both --<flag> and --field; use only one."` NO
HTTP call. Gate B is evaluated after clap parsing (so the values are in scope) but
before any field resolution or HTTP calls.

Scope of Gate B: exactly the four first-party flags that map 1:1 to a known system
field key (`summary`, `description`, `issuetype`, `priority`). Team and points have
dynamically-resolved custom field IDs; overlap detection for those is deferred to v2
because resolution would require an API call (breaking the "no HTTP before the guard"
invariant).

## 4. Non-Goals (explicitly out of scope)

- Changing the **Request Type** of an existing JSM issue. No supported Jira Cloud API
  exists (JSDCLOUD-4609, open since Dec 2016). `PUT` of the `sd-customerrequesttype`
  system field returns HTTP 500. Declared non-goal; documented in CLAUDE.md Gotchas.
- `--field` on multi-key or `--jql`-resolved multi-issue sets.
- `--field` on `issue create` (platform path) — BC-3.8.012 (warning + silent-drop on
  platform path) is UNCHANGED.
- `--field` on `handle_jsm_create` — already fully implemented (S-288-pr4, BC-3.8.008).
- Multi-value field types (`array`, labels, version arrays).
- Interactive `allowedValues` picker.
- `editmeta` caching (the `editmeta` response is issue-specific and mutable; caching it would risk stale `allowedValues` producing wrong option IDs on the wire).
- `--field Assignee=me` resolution to current user's accountId (deferred to v2).

## 5. New API Surface

### `GET /rest/api/3/issue/{key}/editmeta`

**New method in `src/api/jira/issues.rs`:**

```rust
pub async fn get_editmeta(&self, key: &str) -> Result<EditMeta>
```

Response shape (relevant excerpt):
```json
{
  "fields": {
    "customfield_10176": {
      "required": false,
      "schema": { "type": "option", "custom": "...", "customId": 10176 },
      "name": "Urgency",
      "key": "customfield_10176",
      "operations": ["set"],
      "allowedValues": [
        { "self": "...", "value": "High",   "id": "10286" },
        { "self": "...", "value": "Medium", "id": "10287" },
        { "self": "...", "value": "Low",    "id": "10288" }
      ]
    }
  }
}
```

**New Serde structs** (new file `src/types/jira/editmeta.rs` or inline in `issues.rs`):

```rust
pub struct EditMeta { pub fields: HashMap<String, EditMetaField> }
pub struct EditMetaField {
    pub name: String,
    pub schema: EditMetaFieldSchema,
    #[serde(rename = "allowedValues")]
    pub allowed_values: Option<Vec<AllowedValue>>,
    pub operations: Vec<String>,
    pub required: bool,
}
pub struct EditMetaFieldSchema {
    #[serde(rename = "type")] pub field_type: String,
    pub system: Option<String>,
    pub custom: Option<String>,
}
pub struct AllowedValue {
    pub id: String,
    pub value: Option<String>,
    pub name: Option<String>,
}
```

> **Serde rename audit** (OBS-1, pass 8): The `allowed_values` field MUST carry
> `#[serde(rename = "allowedValues")]` — the Jira API key is camelCase. Without it,
> `allowed_values` deserializes to `None` for every field, causing BC-3.4.016 to
> fail with EC-3.4.016-3 ("field has no configured option values") on every valid
> option field. The remaining `EditMetaField` fields (`name`, `schema`, `operations`,
> `required`) match their JSON keys exactly (no rename needed). `EditMetaFieldSchema`
> already carries `#[serde(rename = "type")]` on `field_type`; `system` and `custom`
> match. `AllowedValue` fields (`id`, `value`, `name`) all match. This struct block
> is now copy-paste-correct for the implementer.

The `get_editmeta` call is made ONLY when `--field` is set; existing `issue edit`
invocations without `--field` are byte-for-byte unchanged in behavior and latency.

**Struct field usage clarification** (P3-LOW-002): all fields in `EditMetaField` are
actively used or structurally necessary — there is no dead-code risk:
- `name`: used in error messages (e.g., edit-screen hint, operations-check hint).
- `schema` (`field_type`): used in Step 4 type dispatch.
- `allowed_values`: used in BC-3.4.016 option-value resolution.
- `operations`: used in Step 3b — if `"set"` is absent, exit 64 with hint.
- `required`: deserialized from the API response; not used in v1 resolution logic but
  retained for future validation (e.g., blocking `--field NAME=` empty-value clears on
  required fields). Mark with `#[allow(dead_code)]` ONLY if the Rust compiler emits a
  warning and no v1 use can be found — otherwise leave it as a parsed-but-future-use
  field. (CLAUDE.md: no lint suppression without refactoring; if the compiler warns,
  add a `// Future use: required-field validation` comment and open a follow-up issue
  rather than silently removing the field.)
- `EditMetaFieldSchema.system`: same as `required` — parsed, not used in v1; retain.
- `AllowedValue.name`: parsed but NOT used in v1 resolution logic. The option-value
  resolution path (BC-3.4.016 Step 4a) matches against `AllowedValue.value` only; `name`
  is a secondary label present on some Jira option types (e.g., cascade-select children)
  but not on the standard single-select options used by JSM Urgency/Impact fields. Retained
  for two reasons: (1) the Jira API returns it and omitting it from the struct would cause
  `#[serde(deny_unknown_fields)]` to panic (if used) or would silently drop data on
  round-trips; (2) future v2 option resolution may prefer `name` over `value` for some
  field families. Mark with `#[allow(dead_code)]` ONLY if the Rust compiler warns —
  otherwise leave as parsed-but-future-use. If suppressed, add comment:
  `// Parsed from editmeta allowedValues; unused in v1. Future: cascade-select name matching.`
  [O-2 amendment]

**Non-goal**: the `editmeta` response is NOT cached. It is issue-specific and mutable
(an admin can change the Edit screen at any time). Caching it would risk stale
`allowedValues` producing wrong option IDs on the wire.

### Field-list cache (`fields.json`) — F2 amendment

Field-name resolution (Step 2 of BC-3.4.015) reads a per-profile cache before falling
back to `GET /rest/api/3/field`. This eliminates the global-field-list HTTP call on
warm (repeated) invocations.

**Cache file**: `~/.cache/jr/v1/<profile>/fields.json`  
**TTL**: 7 days (matches all other jr caches)  
**Content**: `Vec<(String, String)>` — `(id, name)` tuples  
**Pattern**: mirrors `CmdbFieldsCache` / `cmdb_fields.json` in `src/cache.rs` exactly

**New cache functions in `src/cache.rs`:**

```rust
pub struct FieldsCache {
    pub fields: Vec<(String, String)>,
    pub fetched_at: DateTime<Utc>,
}
// implements Expiring { fn fetched_at() { self.fetched_at } }

pub fn read_fields_cache(profile: &str) -> Result<Option<FieldsCache>> {
    read_cache(profile, "fields.json")
}

/// Best-effort writer: swallows disk-write errors with eprintln! and returns Ok(()).
/// A missed write costs at most one extra HTTP call on the next invocation.
/// See "best-effort writer" pattern in CLAUDE.md Gotchas (request-type cache writers).
pub fn write_fields_cache(profile: &str, fields: &[(String, String)]) -> Result<()> {
    let result = write_cache(profile, "fields.json", &FieldsCache {
        fields: fields.to_vec(),
        fetched_at: Utc::now(),
    });
    if let Err(e) = result {
        eprintln!("warning: failed to write fields cache: {e}");
    }
    Ok(())
}
```

**Behavior**:
- Cache hit (≤7 days old): use cached field list, no `GET /rest/api/3/field` call.
- Cache miss or stale: fetch via `list_fields()`, write to cache (best-effort), proceed.
- Cache-write failure: warn to stderr, return `Ok(())`, proceed with fetched list.
- `customfield_NNNNN` literal bypass: skips cache AND API entirely.

VP-396-006 verifies the cache-hit path (warm cache → no field-list HTTP call).

## 6. Field-Name Resolution Algorithm

For each `--field NAME=VALUE` pair, resolution proceeds in this order:

| Step | Action |
|------|--------|
| 1 | If `NAME` matches `customfield_\d+` → bypass Steps 2–2b entirely; use `NAME` as field ID; no cache or API read |
| 2 | Read `fields.json` cache (7-day TTL). Cache hit → use cached field list, no HTTP. Cache miss/stale → call `list_fields()` (`GET /rest/api/3/field`), populate cache (best-effort writer), proceed. |
| 2b | Exact (case-insensitive) match first, then substring. 0 matches → exit 64. Multiple → exit 64. Single → field ID. |
| 3 | Call `get_editmeta(key)`. Field ID absent from result → exit 64 with Edit-screen hint. |
| 4 | Read `schema.type`. Dispatch to type-aware serializer. Unsupported type → exit 64 with hint. |
| 4a | For `option` type: resolve `VALUE` → `allowedValues[].id`. See BC-3.4.016. |
| 5 | Merge `(field_id, wire_value)` into `fields` JSON object. |
| 6 | After successful resolution: insert `(human_name, display_value)` into `changed_fields`. |

`list_fields()` is fetched ONCE per invocation, shared across all `--field` pairs.

## 7. Error and Edge Cases (authoritative pointer)

> **The enumerated EC table that was originally here is now stale and has been
> removed to prevent recurring drift** (P3-MED-001, adversary pass 3, 2026-05-22).
>
> **Authoritative EC catalog**: see the `**Edge Cases**` sections of:
> - `bc-3-issue-write.md §BC-3.4.015` — EC-3.4.015-1 through EC-3.4.015-20
>   (incl. -4a; cache ECs -14/-15/-16; dry-run ECs -18/-19; operations check -20)
> - `bc-3-issue-write.md §BC-3.4.016` — EC-3.4.016-1 through EC-3.4.016-7
> - `bc-3-issue-write.md §BC-3.4.017` — EC-3.4.017-1 through EC-3.4.017-14 (EC-3.4.017-13 added FIX-F5-001; EC-3.4.017-14 added issue #407 F2)
>
> The BC body is the single source of truth for EC details. This §7 now serves
> only as a navigation pointer to avoid maintaining two copies of the same catalog.

## 8. `changed_fields` Integration

Each successfully resolved `--field` pair is appended to the existing `changed_fields`
BTreeMap after all field resolution (Step 6 above). The map key is the human field name
(e.g., `"Urgency"`, `"customfield_10176"` for literal bypasses). The map value is:
- For string/number/date/datetime: the raw `VALUE` string.
- For option (single-select): the matched `allowedValues[].value` (human label, not the
  option id). This matches the description echo model: machine-readable in JSON, human-
  readable in table mode.
- For user: the raw `accountId` value as supplied.

`changed_fields` keys from `--field` appear in BTreeMap alphabetical order alongside
existing keys (`summary`, `description`, etc.), because all keys share the same BTreeMap.

## 9. Execution Sequence in `handle_edit` (updated)

Steps 1–3 are common to BOTH the live path and the `--dry-run` path. Steps 4–7 diverge
at the dry-run branch. See BC-3.4.015 invariant 10 for the mandatory control-flow
placement rule.

**Steps common to both live and dry-run paths:**

1. Guard: `has_any_field_change` (updated to include `!field_pairs.is_empty()`).
2. Gate B: flag-overlap detection (no HTTP).
3. C-1 multi-key rejection (updated to include `--field` in the `REJECTED_IN_BULK` set).

**Inside the `if dry_run { ... }` block (before the `return Ok(())` short-circuit):**

4-dry. Existing flag resolutions (description, summary, type, priority, team, points,
   no_points, parent, no_parent) — same as the live path.
5-dry. NEW: `resolve_edit_fields(client, &config.active_profile_name, key, &field_pairs, &mut fields, &mut changed_fields).await?`
   — MUST be called here, inside the dry-run block. If resolution fails, `Err` propagates
   and the caller exits 64 (resolution errors are NOT suppressed by `--dry-run`). If
   resolution succeeds, the resolved entries are in `changed_fields` for the preview.
6-dry. Render the planned-changes preview table/JSON (reads from the populated `fields`
   and `changed_fields`). The `--field` entries appear in the preview alongside other
   flag changes.
7-dry. `return Ok(());` — exit 0. PUT is NOT called.

**On the live path (outside the `if dry_run` block):**

4. Existing flag resolutions (description, summary, type, priority, team, points,
   no_points, parent, no_parent).
5. NEW: `resolve_edit_fields(client, &config.active_profile_name, key, &field_pairs, &mut fields, &mut changed_fields).await?`
   `profile` is the second argument (after `client`) per the CLAUDE.md cache-boundary rule
   (every cache reader/writer takes `profile: &str`; cross-profile field-ID leakage is a
   correctness bug). [P2-006 amendment]
   `field_pairs` is a `&HashMap<String, String>` produced by `parse_field_kv` — NOT a
   `&[(String, String)]` slice. `parse_field_kv` applies `map.insert(key, value)` with
   last-wins semantics: duplicate `--field` keys are collapsed AT PARSE TIME before
   `resolve_edit_fields` is called (see EC-3.4.017-10; BC-3.8.008). [F-1 amendment]
6. `client.edit_issue(key, fields).await` (unchanged).
7. Success echo (unchanged shape; now includes `--field` entries in `changed_fields`).

**Critical implementation constraint**: `resolve_edit_fields` appears in BOTH the dry-run
and live sub-paths. An implementer who places it only on the live path (after the dry-run
`return Ok(())`) will silently produce a dry-run path that (a) never runs editmeta,
(b) never previews `--field` entries, and (c) never exits 64 on a bad `--field` value.
This violates EC-3.4.015-18, EC-3.4.015-19, and VP-396-008. See BC-3.4.015 invariant 10.
[F-1 amendment, pass 6]

## 10. CLAUDE.md Update

> **Timing**: This CLAUDE.md edit is an **F4 implementation deliverable**. It MUST NOT
> be written into CLAUDE.md during F2 (spec evolution) or F3 (story decomposition) —
> it documents behavior that does not yet exist in the codebase. The F3 story for this
> feature MUST carry the CLAUDE.md Gotcha update as an explicit acceptance criterion.
> The edit ships in the same implementation PR as the `--field` feature code.

A new Gotcha entry must be added to CLAUDE.md covering:

1. `--field` on `issue edit` is single-key only (C-1 guard rejects bulk).
2. Changing the Request Type of an existing JSM issue is NOT supported via any Jira
   Cloud API. `jr issue edit --field` does NOT support the `sd-customerrequesttype`
   system field (JSDCLOUD-4609, open since 2016; PUT returns HTTP 500).
3. JSM Urgency/Impact and other request-type select fields CAN be set via
   `jr issue edit --field NAME=VALUE` provided the field is on the issue's agent Edit
   screen. By default these fields are on the portal request form only; an admin must
   add them to the Edit screen first.
4. `--field` on `issue edit` uses `editmeta` to validate and resolve `allowedValues`.
   The `GET .../editmeta` call adds one HTTP round-trip when `--field` is set; it is
   skipped when `--field` is absent.

## 11. Count Surfaces Updated

| Surface | Before | After |
|---------|--------|-------|
| bc-3-issue-write.md `total_bcs` | 100 | 103 |
| bc-3-issue-write.md `definitional_count` | 71 | 74 |
| bc-3-issue-write.md body preamble | "100 behavioral contracts" | "103 behavioral contracts" |
| BC-INDEX.md `total_bcs` frontmatter | 580 | 583 |
| BC-INDEX.md Section 3 header | "100 BCs cumulative; 71 individually-bodied" | "103 BCs cumulative; 74 individually-bodied" |
| BC-INDEX.md Section 3.4 header | "14 BCs: BC-3.4.001..014" | "17 BCs: BC-3.4.001..017" |
| BC-INDEX.md sections: frontmatter line (bc-3) | "100 BCs cumulative; 71 individually-bodied" | "103 BCs cumulative; 74 individually-bodied" |
| BC-INDEX.md Coverage Statistics table (Section 3 row) | `100 \| 71` | `103 \| 74` |
| BC-INDEX.md Coverage Statistics table (Total row) | `580 \| 348` | `583 \| 351` |
| BC-INDEX.md Coverage Statistics prose ("Canonical total is ...") | "580" | "583" |
| BC-INDEX.md Coverage Statistics prose ("+N enumeration") | ends "...+3 BC-3.4.012..014 added 2026-05-21 via issue #398 F2" | appended "+3 BC-3.4.015..017 added 2026-05-22 via issue #396 F2" |
| BC-INDEX.md frontmatter comment | ends "...+3 added 2026-05-21 (BC-3.4.012..014, issue #398 F2)" | appended "; +3 added 2026-05-22 (BC-3.4.015..017, issue #396 F2)" |
| CANONICAL-COUNTS.md per-file table bc-3 definitional | 71 | 74 |
| CANONICAL-COUNTS.md per-file table bc-3 total_bcs | 100 | 103 |
| CANONICAL-COUNTS.md Sum row | 580 | 583 |
| CANONICAL-COUNTS.md individually-bodied total | 348 | 351 |
| CANONICAL-COUNTS.md grand total | 580 | 583 |
| CANONICAL-COUNTS.md grand-total prose ("+N enumeration") | ends "...+3 BC-3.4.012..014 added 2026-05-21 via issue #398 F2" | appended "+3 BC-3.4.015..017 added 2026-05-22 via issue #396 F2" |
| CANONICAL-COUNTS.md frontmatter `last_verified` | `"2026-05-21 ..."` | `"2026-05-22 (F2 delta issue #396; +3 BC-3.4.015..017; bc-3 definitional 71→74 total_bcs 100→103)"` |
| bc-3-issue-write.md end-of-file footer | "71 individually-bodied (cumulative 100 ...)" | "74 individually-bodied (cumulative 103 ...)" [P2-001] |
| bc-3-issue-write.md `_Last updated_` prose (line below footer) | "2026-05-21: +3 BCs (BC-3.4.012..014) ... Section 3.4 header updated to 14 contracts." | "2026-05-22 (issue #396 F2): +3 BCs (BC-3.4.015..017) ... Section 3.4 header updated to 17 contracts." [F-2, pass 6] |
| BC-INDEX.md `last_updated` frontmatter | `2026-05-21` | `2026-05-22` [LOW-001, pass 7] |

> **Note on script coverage** (P3-MED-002, extended passes 6–7): The rows in this
> table fall into two categories. Rows covered by `check-bc-cumulative-counts.sh`
> (Surfaces A–H) are script-guarded and fail CI on drift. The **BC-INDEX Coverage
> Statistics** table rows (Section 3 row, Total row, prose, and "+N enumeration") and
> the **`BC-INDEX.md last_updated` and `_Last updated_` prose** rows are **NOT**
> validated by the guard scripts — they are manually-updated surfaces. When updating
> counts, these rows require a manual visual check. The `_Last updated_` prose line is
> a human-maintained narrative surface NOT validated by either guard script (Surface H
> parses only the numeric footer). Process-gap O-5 (adversary pass 6) tracks the prose
> surface as a known unguarded surface.

## 12. Changelog Entry

**v0.7 / issue #396 (2026-05-22):**
- `issue edit`: new `--field NAME=VALUE` flag (repeatable) for setting arbitrary custom
  fields on an existing issue. Supports string, number, single-select (option),
  date, datetime, and user field types. Single-select options are resolved from
  `editmeta` `allowedValues` by human label (case-insensitive). Unsupported types
  (array, CMDB/any) exit 64 with an actionable hint.
- Field-name resolution: case-insensitive substring match against `list_fields()`; or
  supply `customfield_NNNNN` directly to bypass name resolution.
- Field-presence validation: `GET .../editmeta` confirms the field is on the issue's
  Edit screen before the PUT is attempted, with an actionable admin-hint on failure.
- Flag-overlap guard: combining `--field summary=X` with `--summary Y` (and similarly
  for `--description`, `--type`, `--priority`) exits 64 before any HTTP call.
- `--field` is single-key only; rejected in bulk-edit context (multi-key positional or
  `--jql` matching 2+ issues).
- Request-Type change on existing JSM issues is NOT supported (JSDCLOUD-4609).
- Successful `--field` edits appear in the `changed_fields` echo (table mode stderr,
  JSON `changed_fields` object), consistent with BC-3.4.012/013.

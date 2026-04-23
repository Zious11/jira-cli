# Team field object-shape tolerance

**Issue:** [#254](https://github.com/Zious11/jira-cli/issues/254)

## Problem

On tenants using the Atlas Teams custom field (the modern, cross-product "Team" field provided by the Atlassian Teams platform), `GET /rest/api/3/issue/{key}` returns `customfield_10001` as a **JSON object**:

```json
{"id": "<uuid>", "name": "Team Name"}
```

`src/types/jira/issue.rs:94` currently extracts team UUIDs via `value.as_str()` — it handles only the scalar string shape. For the object shape, `team_id()` returns `None` and emits a once-per-process verbose warning misleadingly claiming the config is broken:

```
[verbose] team field "customfield_10001" has unexpected shape (expected string UUID, got object). Check team_field_id in config.
```

User-visible impact:
- Team row is silently dropped from `issue view` output.
- Team column is silently dropped from `issue list`, `sprint view`, `board view`.
- The verbose warning gaslights the user into re-checking their config when the config is correct.

## Root cause validation

Validated against Atlassian developer documentation (`developer.atlassian.com/platform/teams/components/team-field-in-jira-rest-api`):

- The Team custom field (schema type `com.atlassian.teams:rm-teams-custom-field-team`) accepts a scalar UUID on write (POST/PUT).
- On read (GET), some tenants return a scalar UUID string, others return an object with `id` and `name` properties. The object form is the Atlas Teams platform format, introduced alongside Atlassian Teams.
- Both shapes are valid responses for the same custom-field type; the shape is tenant- and config-dependent.

The existing code comment calling the object shape a "misconfigured `team_field_id`" is wrong. Reconciling the comment is part of this fix.

## Design

Extend `IssueFields::team_id` to accept both shapes. Extract `id` from the object form; keep the existing string form; emit the verbose warning only for genuinely unexpected shapes (bool, number, array, null-with-non-null-value).

### Shape handling

| Value shape | New behaviour |
|---|---|
| `null` or missing | `None`, no warning |
| `"<uuid>"` (scalar string) | `Some("<uuid>".into())` |
| `{"id": "<uuid>", ...}` (object with string `id`) | `Some("<uuid>".into())` |
| `{"id": null, ...}` or object without string `id` | `None`, warn (genuinely unexpected) |
| bool / number / array | `None`, warn (unchanged behaviour) |

The `name` field in the object form is **not** consumed by this change. Display code already resolves UUIDs via the team cache; introducing a parallel "use embedded name when present" code path is scope creep (tracked separately if wanted).

### Callers are unaffected

`team_id()` returns `Option<String>` (a UUID). All four call sites (`list.rs:500`, `list.rs:983`, `sprint.rs:293`, `board.rs:235`) pipe the UUID into team-name resolution via the existing team cache. They do not need to change.

### Warning message reconciliation

The current warning text blames the config. With the fix, the object shape is a happy path and should produce no warning. For the remaining genuinely-unexpected shapes (bool, number, array, object-missing-id), keep a warning but drop the misleading "Check team_field_id in config" suffix — those shapes are genuinely surprising and not typically a config issue.

New warning text (once per process when `verbose`):
```
warning: team field "customfield_10001" has unexpected shape: got <kind>
```

(Lowercase `warning:` per codebase convention established in PR #253. No more `[verbose]` tag — `[verbose]` is a separate pattern for `--verbose`-gated diagnostics, and this case is already verbose-gated via the existing `verbose: bool` parameter. Keep the parameter; drop the tag.)

### What about the `[verbose]` vs `warning:` convention?

Grepping the codebase:
- `[verbose]` tag: used in `src/types/jira/issue.rs:102` (the one we're changing) and likely elsewhere for `--verbose`-gated logs.
- `warning:` tag: used at ~7 sites for user-actionable warnings regardless of verbose mode.

This particular message is `--verbose`-gated (only emits when `client.verbose()`), so `[verbose]` is arguably the right tag for consistency. The spec for this fix: keep `[verbose]` tag to match the existing gated-diagnostic pattern, just drop the misleading "Check team_field_id" suffix.

### Decision

Revised warning:
```
[verbose] team field "customfield_10001" has unexpected shape (got <kind>). Expected string UUID or object with string "id".
```

Same `[verbose]` tag, dropped the config-blame clause, replaced the "expected string UUID" suffix with a positive statement of the two accepted shapes — users hitting this (genuinely broken data: bool, number, array, or object missing a string `id`) get both diagnosis and expectation in one line.

## Testing

Unit tests inline in `src/types/jira/issue.rs`:

1. **Object with string `id`** — `customfield_10001 = {"id": "team-uuid-abc", "name": "Platform Team"}` → `Some("team-uuid-abc")`, no warning.
2. **Object with string `id` AND no `name`** — `{"id": "team-uuid-xyz"}` → `Some("team-uuid-xyz")`, no warning.
3. **Object with null `id`** — `{"id": null, "name": "..."}` → `None`, warning emitted.
4. **Object without `id` key** — `{"name": "..."}` → `None`, warning emitted.
5. **String UUID (regression)** — existing behaviour preserved.
6. **Missing / null / bool / number / array** — existing behaviour preserved.

Existing tests at `src/types/jira/issue.rs:225-265` already cover most non-object cases; extend rather than replace.

Integration test via wiremock: add to `tests/team_column_parity.rs` or a new `tests/team_object_shape.rs` — mount a wiremock issue response with the object-shape team field, run `jr issue view KEY --output json` with `team_field_id` configured, assert the team UUID is extracted (visible in the output, OR via the team-cache-resolved name if the cache is primed).

## Files touched

| Path | Change |
|---|---|
| `src/types/jira/issue.rs` | Rewrite `team_id` match to accept object shape. Update doc comment + warning text. Extend unit tests. |
| `tests/team_object_shape.rs` (new) | Integration test: object-shape team field resolves through to display. |

## Out of scope

- Consuming the `name` field embedded in the object shape (would avoid a cache lookup; can be added later).
- Cache format changes (no change to team cache).
- `--verbose` / `warning:` tag reform across the codebase (tracked separately).
- Write-path changes (the write path already sends a scalar UUID string, which Atlassian accepts for both shapes).

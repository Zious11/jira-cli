# `jr issue move --resolution` — atomic status + resolution transitions

**Issue:** [#263](https://github.com/Zious11/jira-cli/issues/263)

## Problem

`jr issue move` calls `POST /rest/api/3/issue/{key}/transitions` with only a transition ID. On JSM workflows (and classic Jira workflows where resolution lives on the "Done" screen), this leaves tickets in a half-resolved limbo:

- Status → "Resolved" / "Closed" / "Done"
- `resolution` field → null
- `resolutionDate` → null

Consequences: JQL filter `resolution IS EMPTY` still matches, SLAs don't stop, "recently resolved" automations miss the ticket, and reporters see it as open on the JSM portal even though agents see it as done.

The two-step workaround (transition then PUT resolution) is broken: a direct `PUT` of the `resolution` field does NOT trigger `resolutionDate`, so downstream time-based automations stay broken.

## Atlassian constraint (validated)

Confirmed via Atlassian's developer docs:

- `POST /rest/api/3/issue/{key}/transitions` accepts both `transition` AND `fields` in the request body.
- Passing `{"transition":{"id":"..."},"fields":{"resolution":{"name":"Done"}}}` atomically transitions the status, sets `resolution`, and fires the `resolutionDate` timestamp.
- `GET /rest/api/3/resolution` returns the instance-scoped resolution list (company-managed classic projects; team-managed projects don't use resolution). Each entry has `id`, `name`, `description`.

## Design

### New CLI surface

```
jr issue move <KEY> <STATUS> --resolution <NAME>
jr issue resolutions [--refresh]
```

- `--resolution <name>` on `jr issue move` is **optional**. When present, the name is matched **case-insensitively against the exact name** in the cached resolution list. Prefix / substring / partial matches do NOT auto-resolve — they surface the candidate list via `JrError::UserError` (exit 64), same convention as other `--no-input`-first jr resolvers. This is a deliberate UX choice: the flag is a machine-facing setter where silent fuzziness would be surprising. Unset = today's behavior, no resolution sent.
- `jr issue resolutions` lists the cached resolutions. Table output by default, `--output json` for scripts.
- `--refresh` busts the cache.

### API changes

- `src/api/jira/resolutions.rs` (new): `JiraClient::get_resolutions() -> Result<Vec<Resolution>>` calling `GET /rest/api/3/resolution`.
- `src/api/jira/issues.rs`: `transition_issue` gains an optional `fields: Option<&serde_json::Value>` argument. When `Some`, it merges into the request body alongside `transition`. Call sites that don't care pass `None`.
- `src/types/jira/` or inline: `Resolution { id, name, description }`.

### Cache

`~/.cache/jr/resolutions.json`, 7-day TTL matching existing team / workspace / cmdb_fields caches. `read_resolutions_cache()` / `write_resolutions_cache()` follow the existing `read_cache` / `write_cache` helpers in `src/cache.rs`.

### Partial-match

`resolve_resolution_by_name(&[Resolution], query) -> Result<Resolution>` using the existing `partial_match` crate (`src/partial_match.rs`). Exact > prefix > substring. `Ambiguous` and `ExactMultiple` branches surface the candidate list via `JrError::UserError` (exit 64) to match existing `jr issue move` status-disambiguation UX.

### Error path when Atlassian rejects a transition for missing resolution

When `jr issue move` succeeds at matching a transition but Atlassian returns a 400 on the transition POST containing "resolution is required" (or variants — different workflow configs word it differently), catch in the error path and transform to:

```
error: the "Done" transition requires a resolution.

Try:
    jr issue move <KEY> Done --resolution <name>

Run `jr issue resolutions` to see available values.
```

Heuristic: look for "resolution" (case-insensitive) and "required" in the Atlassian error body. If both present, transform. Otherwise pass through the original error. This is a hint, not a hard dependency.

### Testing

- `src/api/jira/resolutions.rs`: wiremock integration test for the endpoint wrapper.
- `src/cache.rs`: round-trip + missing-file tests for `ResolutionsCache`. TTL expiry is covered generically by the shared `read_cache` path (exercised by other whole-file caches like `TeamCache` / `WorkspaceCache`).
- `src/cli/issue/workflow.rs`: handler tests on `resolve_resolution_by_name` — exact match, case-insensitive exact, ambiguous (substring) returns exit 64, no match returns exit 64 with candidate list, multiple exact duplicates lists only the colliding entries (with ids for disambiguation).
- `tests/`: integration test for the missing-resolution error-path transformation — mount a wiremock transition endpoint that 400s with "Field 'resolution' is required", assert jr's exit 1 message mentions `--resolution` and `jr issue resolutions`.
- `jr issue resolutions`: table output test, JSON output test. `--refresh` behavior is exercised via the shared `load_resolutions(client, refresh)` helper; both `handle_move` (refresh=false) and `handle_resolutions` (refresh passed from the flag) use the same code path.

### Out of scope

- Generic `--field key=value` pass-through — explicitly deferred per the issue's "Suggested fix scope". Resolution is the 95% case; the long tail (fix version, component, custom workflow fields) can land in a follow-up if anyone hits it.
- Per-project resolution discovery — Atlassian's API only exposes instance-scoped resolution listing. A transition-screen-specific list isn't available publicly. Acceptable — most instances have 5–10 resolutions max.

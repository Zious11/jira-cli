# `jr issue create --output json` — return full issue shape

**Issue:** [#253](https://github.com/Zious11/jira-cli/issues/253)

## Problem

`jr issue create --output json` returns a minimal body with only `key` and `url`. `jr issue view --output json` on the same key returns a full `Issue` payload with `fields` (summary, status, assignee, labels, parent, description, story points, team, CMDB fields). Scripts that pipe `create` output into `jq '.fields.summary'` silently get empty strings.

The user-reported impact: "empty summary appeared in a status report we generated."

## Root cause

Validated against Atlassian REST API v3 docs (Perplexity + `developer.atlassian.com/cloud/jira/platform/rest/v3`):

- `POST /rest/api/3/issue` returns only `{id, key, self}` — the full issue is **not** included in the POST response.
- There is **no `expand` or `fields` query parameter** on POST. The only way to get the full shape is a follow-up `GET /rest/api/3/issue/{key}`.

`src/cli/issue/create.rs:138-156` currently serializes only `CreateIssueResponse { key }` and tacks on `url`. It does not follow up with a GET.

## Design

After a successful POST, perform a follow-up GET that mirrors `handle_view`'s behaviour, then merge `url` into the resulting JSON and print.

### User-visible change

Before (create):
```json
{ "key": "PROJ-123", "url": "https://…/browse/PROJ-123" }
```

After (create):
```json
{
  "key": "PROJ-123",
  "fields": { "summary": "…", "status": {…}, "assignee": {…}, … },
  "url": "https://…/browse/PROJ-123"
}
```

Shape matches `serde_json::to_value(&Issue { key, fields })` from
`src/types/jira/issue.rs` — the `Issue` struct only carries `key` and
`fields`, so `id` and `self` from the raw API response don't appear in
the serialized output.

If the follow-up GET fails, the fallback shape is:
```json
{
  "key": "PROJ-123",
  "url": "https://…/browse/PROJ-123",
  "fetch_error": "…error message…"
}
```

The top-level `fetch_error` string is a machine-readable sentinel so
scripts piping through `jq` can distinguish a degraded fallback from
success without parsing stderr.

### Which extra fields to fetch

The follow-up GET uses the same `extra_fields` composition as `handle_view` (`src/cli/issue/list.rs:759-770`):
- `config.global.fields.story_points_field_id`
- `config.global.fields.team_field_id`
- discovered CMDB fields (via cached `get_or_fetch_cmdb_fields`)

### CMDB linked-asset enrichment

**Out of scope.** `handle_view` also enriches linked CMDB assets via a batch lookup after the GET. For create, skip enrichment — the user just issued a create, the asset fields are rarely populated at creation time, and enrichment would double the latency of every `create` call. The raw CMDB field IDs in the response are enough for scripts that need them. If a future issue wants enriched assets on create, that's additive.

### Degraded path: GET fails after POST succeeds

The issue has already been created when the follow-up GET runs — we must **not** treat a GET failure as a create failure. Behaviour:

- Emit a stderr warning: `warning: issue created ({key}) but follow-up fetch failed: {err}`
- Fall back to the old minimal shape: `{ "key": "...", "url": "..." }`
- Exit 0 (the create succeeded; the user's automation still gets the key)

### Table output

Unchanged. Table already prints only `Created issue PROJ-123` + browse URL on stderr. No extra GET is performed on the table path (saves a round trip when the user is reading on a terminal).

### Command: `edit`

Out of scope. `edit` already has its own `edit_response` shape; this issue is specifically about `create`.

## Testing

All tests use wiremock (`JR_BASE_URL` override, existing pattern in `tests/`):

1. **Happy path** — POST returns `{id, key, self}`, GET returns full `Issue`. Assert stdout JSON has `.fields.summary`, `.key`, `.url`, and `.fields.status.name`.
2. **Degraded path** — POST returns 201, GET returns 500. Assert stdout JSON is the minimal `{key, url}` shape, stderr contains `warning:`, exit code 0.
3. **Table path** — POST returns 201, no GET is made (wiremock expectation: zero GET requests). Assert stdout/stderr match existing table output.

Unit tests: none needed — the handler already has integration coverage; the change is purely orchestration.

Snapshot: none — JSON output is already matched by integration tests, no insta snapshot change needed.

## Files touched

| Path | Change |
|---|---|
| `src/cli/issue/create.rs` | `handle_create` JSON branch: after POST, call `get_issue(&key, &extra)`, merge `url`, print. On GET failure, warn + fall back. |
| `src/cli/issue/create.rs` or `helpers.rs` | Factor `extra_fields_for_view(config, cmdb_fields)` helper shared with `handle_view` (light refactor). |
| `src/cli/issue/list.rs` | `handle_view` uses the new shared helper for extra_fields composition. |
| `tests/issue_create_json.rs` | New file: 3 wiremock tests (happy, degraded, table-unchanged). |

## Out of scope

- Asset enrichment on create (future issue if needed).
- `issue edit` JSON shape.
- `expand=renderedFields` on the follow-up GET (not part of `handle_view` either; keep parity).

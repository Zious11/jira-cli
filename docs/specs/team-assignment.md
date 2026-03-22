# Team Assignment Feature Spec

## Goal

Enable users to assign, filter, and list Jira teams via `jr`, resolving human-readable team names to UUIDs transparently.

## Background

Jira Cloud's Team field (`customfield_10001`, type `atlassian-team`) stores teams as UUIDs. Setting the field requires a plain UUID string — not a `{"value": "name"}` object. Listing available teams requires the Atlassian Teams REST API, which needs an organization ID obtained through the GraphQL gateway. JQL filtering by team also requires the UUID, not the display name.

The team field ID (`customfield_XXXXX`) varies per Jira instance. `FieldsConfig.team_field_id` already exists in `config.rs` and is already discovered during `jr init`. `cloud_id` already exists in `InstanceConfig`. Only `org_id` is new.

## Architecture

### Discovery Chain

Discovery endpoints (`/gateway/api/graphql`, `/gateway/api/public/teams/...`) are served at the **instance URL** (e.g., `https://myorg.atlassian.net`), not the API proxy. This matters because for OAuth users, `JiraClient.base_url` returns `https://api.atlassian.com/ex/jira/{cloud_id}` — which is a different host.

**Solution:** Add an `instance_url()` method to `JiraClient` that always returns the raw instance URL from `config.global.instance.url`, independent of `base_url()`. The teams API module uses `instance_url()` for discovery endpoints, while all standard Jira REST API calls continue using `base_url()`.

```
POST {instance_url}/gateway/api/graphql → cloudId + orgId (single call)
GET {instance_url}/gateway/api/public/teams/v1/org/{orgId}/teams → team list
GET {base_url}/rest/api/3/field → team_field_id (already implemented, uses base_url)
```

#### GraphQL Query for cloudId and orgId

Uses the `hostNames` parameter (extracted from the configured instance URL). Both `cloudId` and `orgId` are returned in a single call — no need for a separate `/_edge/tenant_info` call.

```
POST {instance_url}/gateway/api/graphql
Content-Type: application/json
Authorization: Basic <auth>

{
  "query": "query getOrgId { tenantContexts(hostNames: [\"myorg.atlassian.net\"]) { orgId cloudId } }"
}
```

Response:
```json
{
  "data": {
    "tenantContexts": [
      {
        "orgId": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
        "cloudId": "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy"
      }
    ]
  }
}
```

Note: Both `cloudIds` and `hostNames` parameters work. We use `hostNames` because we already know the hostname from config and it avoids a separate API call.

#### Teams REST API

```
GET {instance_url}/gateway/api/public/teams/v1/org/{orgId}/teams
Authorization: Basic <auth>
```

Response (verified against live API):
```json
{
  "entities": [
    {"teamId": "uuid-here", "displayName": "Platform Engineering", "organizationId": "...", "state": "ACTIVE", "teamType": "..."},
    ...
  ],
  "cursor": null
}
```

Pagination: Uses `cursor` field. When `cursor` is non-null, pass it as `?cursor=<value>` on the next request. When `cursor` is null, all teams have been returned.

### Storage

- **Config** (`~/.config/jr/config.toml`): `org_id` under `[instance]` (new); `cloud_id` under `[instance]` (already exists); `team_field_id` under `[fields]` (already exists).
- **Cache** (`~/.cache/jr/teams.json`): team list with timestamp — disposable, 7-day TTL. Uses `XDG_CACHE_HOME` if set, otherwise `~/.cache/jr/`. Cache directory is created on first write if it doesn't exist.

### Cache File Format

```json
{
  "fetched_at": "2026-03-22T05:30:00Z",
  "teams": [
    {"id": "2b42e90e-8c65-4370-ae22-0a12bf13b47b", "name": "Platform Engineering"},
    {"id": "84335611-434f-4636-a7aa-7079443f3ea7", "name": "Security Engineering"}
  ]
}
```

TTL: 7 days. Cache is safe to delete — next `--team` usage or `jr team list` repopulates it.

## New Command

### `jr team list`

```
jr team list [--refresh] [--output json|table]
```

- **Default**: reads from cache if available and within 7-day TTL. If cache is missing or expired, fetches fresh and updates cache.
- **`--refresh`**: forces a fresh API fetch regardless of TTL, updates cache.
- **Table output**: two columns — `Name` and `ID`.
- **JSON output**: array of `{"name": "...", "id": "..."}`.
- **`--no-input`**: no effect (this command doesn't prompt). Included for consistency — all commands support it.

## Modified Commands

### `jr issue create --team <name>` / `jr issue edit --team <name>`

1. Resolve team name to UUID via `resolve_team_field()` (cache + partial match).
2. Set `customfield_{team_field_id}` = UUID string (plain string, not wrapped in object).

### `jr issue edit --team ""`

Passing an empty string clears the team assignment (sets field to `null`). Out of scope for v1 — document as future work.

### `jr issue list --team <name>`

1. Resolve team name to UUID via `resolve_team_field()` (same resolution function used by create/edit).
2. Use resolved values in JQL: `{team_field_id} = "uuid-here"` (uses the dynamic custom field ID, not the display name — JQL requires UUIDs for team fields).

**Important:** All three JQL construction paths in `handle_list` (scrum, kanban, fallback) must call `resolve_team_field()` *before* building JQL, using the returned `(field_id, uuid)` tuple. The current code directly interpolates the raw team name into JQL with `"Team" = "{name}"` — this must be replaced in all three paths.

### `jr issue view`

Displaying the team field in `jr issue view` is out of scope for this feature. The `get_issue` API call would need to request the custom field and deserialize the team object. Future work.

## Team Name Resolution

`resolve_team_field()` in `src/cli/issue.rs` is the single resolution function used by all commands. It currently passes the raw team name through (`Ok((field_id, team_name.to_string()))`). It must be rewritten to:

1. Resolve `org_id` from config (lazy-fetch if missing — see Lazy Fallback).
2. Resolve `team_field_id` from config (lazy-fetch via `/rest/api/3/field` if missing).
3. Load team list from cache (fetch if missing or expired).
4. Run partial match (existing `partial_match.rs`) against team display names.
5. **Exact match** (1 result): return `(team_field_id, team_uuid)`.
6. **Ambiguous** (2+ results): prompt user to pick via `dialoguer::Select`. If `no_input` is true, error listing the ambiguous matches.
7. **No match**: error suggesting `jr team list --refresh`.

Returns `(String, String)` — `(team_field_id, team_uuid)`. Same shape as today, but the second value is now a UUID instead of the raw name.

**`no_input` threading:** `resolve_team_field()` must accept a `no_input: bool` parameter. `handle_create` already has access to this. `handle_edit` currently does not receive `no_input` — it must be threaded through. `handle_list` also needs it threaded for the team resolution prompt.

## `jr init` Changes

After URL + auth verification, prefetch (best-effort, warn on failure):

1. `cloud_id` + `org_id` via GraphQL `tenantContexts(hostNames: [...])` → save both to `config.toml [instance]` (single API call)
3. `team_field_id` via `/rest/api/3/field` → save to `config.toml [fields]` (already implemented in init — no change needed)
4. Team list via Teams REST API → write to `~/.cache/jr/teams.json` (new)

## Lazy Fallback

For users with existing configs (pre-team-support), if `org_id` or `team_field_id` is missing when `--team` is first used:

1. Discover `cloud_id` and `org_id` automatically (if missing).
2. Discover `team_field_id` automatically (if missing).
3. Save all discovered values to `config.toml`.
4. Proceed with team resolution.

This adds ~1s latency on first use only.

## New Files

| File | Purpose |
|------|---------|
| `src/api/jira/teams.rs` | API calls: `get_org_metadata(hostname)` (returns cloudId + orgId via GraphQL), `list_teams(org_id)`. Uses `instance_url()` not `base_url()` for discovery endpoints. |
| `src/types/jira/team.rs` | Serde structs: `GraphqlOrgResponse`, `TenantContext { org_id, cloud_id }`, `TeamsResponse { entities: [TeamEntry], cursor }`, `TeamEntry { team_id, display_name }` |
| `src/cli/team.rs` | `jr team list` command handler |
| `src/cache.rs` | Cache read/write/TTL logic for `~/.cache/jr/` (XDG_CACHE_HOME aware) |

## Modified Files

| File | Change |
|------|--------|
| `src/api/jira/mod.rs` | Add `pub mod teams` |
| `src/types/jira/mod.rs` | Add `pub mod team` |
| `src/api/client.rs` | Add `instance_url()` method returning raw instance URL from config (not the OAuth proxy URL) |
| `src/cli/mod.rs` | Add `TeamCommand` enum, `team` subcommand |
| `src/cli/issue.rs` | Rewrite `resolve_team_field()` to accept `no_input`, load cache, partial match, resolve to UUID, prompt on ambiguity. Update all three JQL paths (scrum, kanban, fallback) to call `resolve_team_field()` before building JQL and use `{field_id} = "uuid"` instead of `"Team" = "name"`. Thread `no_input` into `handle_edit` and `handle_list`. |
| `src/config.rs` | Add `org_id` to `InstanceConfig` |
| `src/cli/init.rs` | Add prefetch of cloud_id, org_id, and team cache (team_field_id discovery already exists) |
| `src/main.rs` | Add dispatch for `team` command |

## Error Handling

| Scenario | HTTP | Message |
|----------|------|---------|
| No auth / expired token | 401 | Existing `JiraClient` handling → `"Not authenticated. Run jr auth login."` |
| Wrong org_id / no access | 403 | `"Cannot access organization teams. Your org_id may be stale — run jr init to reconfigure."` |
| Cache missing + API unreachable | Network | `"Could not fetch team list. Try again or run jr team list --refresh when online."` |
| `--team` no match | — | `No team matching "foo". Run "jr team list --refresh" to update.` |
| `--team` ambiguous + `--no-input` | — | `Multiple teams match "eng": "Security Engineering", "Application Engineering". Use a more specific name.` |
| Team field doesn't exist | — | `No "Team" field found on this Jira instance.` |
| org_id discovery fails | — | `"Could not resolve organization ID. Check your Jira URL and permissions, or run jr init."` |
| team_field_id missing + discovery fails | — | `No "Team" field found on this Jira instance. This instance may not have the Team field configured.` |

## Bug Fix (already applied)

JQL `ORDER BY` clause was being joined with `AND`, producing invalid JQL like `"Team" = "x" AND ORDER BY rank ASC`. Already fixed in `src/cli/issue.rs` with unit tests for `build_fallback_jql`. This fix is part of the current working tree and should be committed with this feature.

## Testing

- **Unit tests**: cache TTL logic (expired/valid/missing), team name resolution via partial match, `build_fallback_jql` (already added)
- **Integration tests**: wiremock mocks for Teams REST API, `/_edge/tenant_info`, GraphQL orgId query
- **Manual**: live test against 1898andco.atlassian.net

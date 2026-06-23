# ADR-0009: handle_open Must Use instance_url(), Not base_url()

## Status
Accepted

## Context

A HIGH-severity UX bug was discovered in `handle_open` in
`src/cli/issue/workflow.rs`. When a user runs `jr issue open FOO-123` on an
OAuth-authenticated profile, the browser opens a 404 page instead of the issue.

`JiraClient` exposes two URL accessors:

| Accessor | Returns | Valid for |
|----------|---------|-----------|
| `base_url()` | For API-token profiles: `https://<site>.atlassian.net`; for OAuth profiles: `https://api.atlassian.com/ex/jira/<cloud_id>` | Making API calls — the REST API accepts both forms |
| `instance_url()` | Always: `https://<site>.atlassian.net` | Human-facing URLs (browse links, the Jira web UI) |

`handle_open` was using `base_url()` to compose the browse URL:

```
// broken for OAuth profiles
format!("{}/browse/{}", client.base_url(), key)
```

For OAuth profiles, `base_url()` returns `https://api.atlassian.com/ex/jira/<cloud_id>`.
The browser sends a GET to `https://api.atlassian.com/ex/jira/<cloud_id>/browse/FOO-123`,
which is not a valid Jira issue URL. The user sees a 404 or a JSON error body.

For API-token profiles, `base_url()` and `instance_url()` return the same value, so the
bug was invisible to API-token users.

## Decision

Replace `client.base_url()` with `client.instance_url()` at the browse-URL composition
site in `src/cli/issue/workflow.rs::handle_open`:

```
// fixed
format!("{}/browse/{}", client.instance_url(), key)
```

## Rationale

- `instance_url()` is explicitly designed for human-facing URLs. `base_url()` is the
  API-call URL, which may be an OAuth proxy endpoint not accessible via browser.
- This is a one-line fix with no structural changes required.
- The correct URL form (`<site>.atlassian.net/browse/<key>`) is the standard Jira Cloud
  browse URL, stable across all instances.

## Consequences

- **Fix scope:** 1 line changed in `src/cli/issue/workflow.rs::handle_open`.
- **Regression risk:** NONE for API-token profiles (both accessors return the same
  value). FIXED for OAuth profiles.
- **Test requirement:** Integration test with an OAuth profile fixture verifying that the
  `--open` URL is composed with the instance base URL, not the OAuth proxy URL.
- All other `client.base_url()` call sites in CLI handlers are making API calls and are
  correct as-is — this fix is scoped to the browser-URL composition.
- **BC anchor:** BC-3.4.001.

## See Also

- `src/cli/issue/workflow.rs::handle_open` — browse URL composition
- `src/api/client.rs` — `base_url()` and `instance_url()` accessor definitions
- ADR-0006 — Embedded OAuth app (explains the two-URL regime)

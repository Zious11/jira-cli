# ADR-0009: handle_open Must Use instance_url(), Not base_url()

## Status
Accepted

## Context

A HIGH-severity UX bug was discovered in `handle_open` (NFR-R-B, BC-3.4.001, NEW-INV-56). When a user runs `jr issue open FOO-123` on an OAuth-authenticated profile, the browser opens a 404 page instead of the issue.

**Bug anatomy (`cli/issue/workflow.rs:636`):**

`JiraClient` exposes two URL accessors:

| Accessor | Returns | Valid for |
|----------|---------|-----------|
| `base_url()` | For API-token profiles: `https://<site>.atlassian.net`; for OAuth profiles: `https://api.atlassian.com/ex/jira/<cloud_id>` | Making API calls — the REST API accepts both forms |
| `instance_url()` | Always: `https://<site>.atlassian.net` | Human-facing URLs (browse links, the Jira web UI) |

`handle_open` currently uses `base_url()` to compose the browse URL:

```
// current (broken for OAuth profiles)
format!("{}/browse/{}", client.base_url(), key)
```

For OAuth profiles, `base_url()` returns `https://api.atlassian.com/ex/jira/<cloud_id>`. The browser sends a GET to `https://api.atlassian.com/ex/jira/<cloud_id>/browse/FOO-123` — which is not a valid Jira issue URL. The user sees a 404 or a JSON error body.

For API-token profiles, `base_url()` and `instance_url()` return the same value, so the bug is invisible to API-token users.

## Decision

Replace `client.base_url()` with `client.instance_url()` at `cli/issue/workflow.rs:636`.

```
// fixed
format!("{}/browse/{}", client.instance_url(), key)
```

## Rationale

- `instance_url()` is explicitly designed for human-facing URLs. The `base_url()` accessor is the API-call URL, which may be an OAuth proxy endpoint that is not accessible via browser.
- This is a one-line fix. No structural changes are needed.
- The correct URL form (`<site>.atlassian.net/browse/<key>`) is the Jira standard browse URL, stable across all Jira Cloud instances.

## Consequences

- **Fix scope:** 1 line changed in `cli/issue/workflow.rs:636`.
- **Regression risk:** NONE for API-token profiles (both accessors return the same value). FIXED for OAuth profiles.
- **Test requirement:** Add an integration test with an OAuth profile fixture that verifies the `--open` URL is composed with the instance base URL, not the OAuth proxy base URL. The test may use a mock that captures the URL without actually opening a browser.
- **BC anchor:** BC-3.4.001 (MUST-FIX forward-looking spec).

## References

- NFR-R-B (nfr-catalog.md)
- BC-3.4.001 (bc-3-issue-write.md)
- Pass 3 R4 finding (jira-cli-pass-3-deep-r4.md)
- risk-register.md §R-H4

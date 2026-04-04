# Design: Add browse URL to `issue create` output

**Issue:** [#112](https://github.com/Zious11/jira-cli/issues/112)
**Date:** 2026-04-03
**Status:** Draft

## Problem

`jr issue create` table output prints `Created issue PROJ-123` but does not include the Jira browse URL. The `gh` CLI prints the full URL after creating an issue, which is useful for both humans (clickable link) and AI agents (direct URL without constructing it from config).

The original issue reported the key wasn't visible, but investigation confirmed the key is already displayed. The real gap is the missing browse URL.

JSON output currently returns only `{"key": "PROJ-123"}` — also missing the URL.

## Approach

Construct the browse URL from `client.instance_url()` + `/browse/` + key, and add it to both table and JSON output. This is a minimal change to `handle_create` in `src/cli/issue/create.rs`.

`instance_url()` is used instead of `base_url()` because the browse URL must point to the real Jira instance, not the OAuth proxy (which `base_url()` may point to for OAuth users).

## Design

### Change 1: Table output — add URL on second line

Current:
```
Created issue PROJ-123
```

After:
```
Created issue PROJ-123
https://mycompany.atlassian.net/browse/PROJ-123
```

The URL is printed as plain text (not green) so terminal link detection works and it's clickable.

In `src/cli/issue/create.rs`, the `OutputFormat::Table` arm changes from:

```rust
output::print_success(&format!("Created issue {}", response.key));
```

To:

```rust
let url = format!(
    "{}/browse/{}",
    client.instance_url().trim_end_matches('/'),
    response.key
);
output::print_success(&format!("Created issue {}", response.key));
println!("{}", url);
```

### Change 2: JSON output — add `url` field

Current:
```json
{
  "key": "PROJ-123"
}
```

After:
```json
{
  "key": "PROJ-123",
  "url": "https://mycompany.atlassian.net/browse/PROJ-123"
}
```

In `src/cli/issue/create.rs`, the `OutputFormat::Json` arm changes from:

```rust
println!("{}", serde_json::to_string_pretty(&response)?);
```

To:

```rust
let url = format!(
    "{}/browse/{}",
    client.instance_url().trim_end_matches('/'),
    response.key
);
let json_response = serde_json::json!({
    "key": response.key,
    "url": url,
});
println!("{}", serde_json::to_string_pretty(&json_response)?);
```

## What stays the same

- `handle_edit` output — unchanged (user already knows the issue key when editing)
- `CreateIssueResponse` struct — unchanged (URL is constructed, not deserialized)
- `output::print_success` — unchanged
- All other commands — unchanged

## Testing

One integration test added to `tests/issue_commands.rs` that:
1. Mounts a wiremock mock for `POST /rest/api/3/issue` returning `{"key": "TEST-1"}`
2. Calls `handle_create` with table output and verifies stdout contains both `Created issue TEST-1` and the browse URL
3. Calls with JSON output and verifies the response includes both `key` and `url` fields

## Files modified

- `src/cli/issue/create.rs` — table and JSON output in `handle_create` (~6 lines changed)

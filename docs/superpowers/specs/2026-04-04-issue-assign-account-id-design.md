# Design: Accept accountId as fallback for issue assign and create

**Issue:** [#115](https://github.com/Zious11/jira-cli/issues/115)
**Date:** 2026-04-04
**Status:** Draft

## Problem

When `--to` name search fails (ambiguous names, deactivated users, API quirks), there is no way to pass a known Jira accountId directly. The error messages already say "Specify the accountId directly" but no flag exists to do so.

This is especially painful for AI agents and scripts that already have an accountId from another API response (e.g., extracted from an issue's assignee field) and want to bypass name search entirely.

## Approach

Add `--account-id` as a mutually exclusive alternative to `--to` on both `issue assign` and `issue create`. When provided, skip all user search/disambiguation logic and pass the accountId straight to the Jira API.

**Why mutually exclusive flags instead of auto-detection:** Jira accountIds are opaque strings with no documented stable format. Auto-detecting whether a `--to` value is a name or an accountId would be fragile and could misinterpret display names as IDs. Popular CLIs (kubectl, docker) auto-detect because their ID formats are well-defined; Jira's are not. A separate flag is explicit and unambiguous.

**Why not client-side validation:** The Jira API returns a clear 404 error ("The user with account ID '...' does not exist") for invalid accountIds. Client-side format validation would be redundant and could reject valid IDs if Atlassian changes the format.

## Design

### Change 1: Add `--account-id` flag to `Assign` variant

In `src/cli/mod.rs`, add to the `Assign` variant:

```rust
Assign {
    /// Issue key
    key: String,
    /// Assign to this user (omit to assign to self)
    #[arg(long, conflicts_with = "account_id")]
    to: Option<String>,
    /// Assign to this Jira accountId directly (bypasses name search)
    #[arg(long, conflicts_with_all = ["to", "unassign"])]
    account_id: Option<String>,
    /// Remove assignee
    #[arg(long)]
    unassign: bool,
},
```

### Change 2: Add `--account-id` flag to `Create` variant

In `src/cli/mod.rs`, add to the `Create` variant:

```rust
/// Assign to user (name/email, or "me" for self)
#[arg(long, conflicts_with = "account_id")]
to: Option<String>,
/// Assign to this Jira accountId directly (bypasses name search)
#[arg(long, conflicts_with = "to")]
account_id: Option<String>,
```

### Change 3: Branch on `account_id` in `handle_assign`

In `src/cli/issue/workflow.rs`, the assignee resolution block changes from:

```rust
let (account_id, display_name) = if let Some(ref user_query) = to {
    helpers::resolve_assignee(client, user_query, &key, no_input).await?
} else {
    let me = client.get_myself().await?;
    (me.account_id, me.display_name)
};
```

To:

```rust
let (account_id, display_name) = if let Some(ref id) = account_id {
    (id.clone(), id.clone())
} else if let Some(ref user_query) = to {
    helpers::resolve_assignee(client, user_query, &key, no_input).await?
} else {
    let me = client.get_myself().await?;
    (me.account_id, me.display_name)
};
```

When `--account-id` is used, the display name is set to the accountId string itself since no user search is performed and we don't have a display name.

### Change 4: Branch on `account_id` in `handle_create`

In `src/cli/issue/create.rs`, the assignee block changes from:

```rust
if let Some(ref user_query) = to {
    let (account_id, _display_name) =
        helpers::resolve_assignee_by_project(client, user_query, &project_key, no_input)
            .await?;
    fields["assignee"] = json!({"id": account_id});
}
```

To:

```rust
if let Some(ref id) = account_id {
    fields["assignee"] = json!({"accountId": id});
} else if let Some(ref user_query) = to {
    let (acct_id, _display_name) =
        helpers::resolve_assignee_by_project(client, user_query, &project_key, no_input)
            .await?;
    fields["assignee"] = json!({"accountId": acct_id});
}
```

Note: This also fixes the existing `--to` path from `{"id": account_id}` to `{"accountId": acct_id}`, which is the documented Jira Cloud REST API v3 format (confirmed via Perplexity from Atlassian community sources).

### Output format

**Table (assign with `--account-id`):**
```
Assigned FOO-123 to 6279395793111000689f87d2
```

**JSON (assign with `--account-id`):**
```json
{
  "key": "FOO-123",
  "assignee": "6279395793111000689f87d2",
  "assignee_account_id": "6279395793111000689f87d2",
  "changed": true
}
```

Both `assignee` and `assignee_account_id` contain the accountId since no display name is available. This keeps the JSON schema consistent with the existing `--to` output shape.

**Create output** is unchanged — it shows the issue key and browse URL, not the assignee.

### Idempotent behavior

The existing idempotent check in `handle_assign` compares `assignee.account_id == account_id`. This works identically whether the accountId came from `--to` resolution or `--account-id` — no changes needed.

## What stays the same

- `--to` behavior on both commands — unchanged
- `--unassign` behavior — unchanged
- Self-assign (no flags on `assign`) — unchanged
- `resolve_assignee`, `resolve_assignee_by_project` functions — unchanged
- `resolve_user` (used by `issue list --user`) — unchanged, separate concern
- `issue list --user` — unchanged (no `--account-id` for JQL filtering)
- Error messages — unchanged (Jira API provides clear 404 for invalid accountIds)

## Testing

Two integration tests added to `tests/issue_commands.rs`:

1. **`test_assign_issue_with_account_id`**: Mount wiremock mocks for GET issue (to check current assignee) and PUT assignee. Call `handle_assign` with `account_id = Some("abc123")`. Verify the PUT request body contains `{"accountId": "abc123"}` and the output contains the accountId.

2. **`test_create_issue_with_account_id`**: Mount wiremock mock for POST create issue. Call `handle_create` with `account_id = Some("abc123")`. Verify the request body's `fields.assignee` is `{"accountId": "abc123"}`.

## Files modified

- `src/cli/mod.rs` — Add `account_id` field to `Assign` and `Create` variants (~4 lines each)
- `src/cli/issue/workflow.rs` — Branch on `account_id` in `handle_assign` (~3 lines added)
- `src/cli/issue/create.rs` — Branch on `account_id` in `handle_create`, fix `id` to `accountId` (~5 lines changed)
- `tests/issue_commands.rs` — Two integration tests (~60 lines total)

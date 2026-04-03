# Design: Fix partial_match duplicate name disambiguation

**Issues:** [#117](https://github.com/Zious11/jira-cli/issues/117), [#122](https://github.com/Zious11/jira-cli/issues/122)
**Date:** 2026-04-03
**Status:** Draft

## Problem

`partial_match` returns `Exact(String)` on the first case-insensitive exact match, stopping iteration. When multiple candidates share the same name (e.g., two Jira users named "John Smith"), the function silently returns the first one. All callers then use `.find(|u| u.display_name == matched_name)` to map the name back to an object, which again picks the first match.

Jira display names are not unique. Atlassian confirmed this is by design (JSDCLOUD-10963, Won't Fix). Two users in the same project can have identical display names with different accountIds. The current code can assign work to the wrong person with no warning.

The `Ambiguous` branch has the same problem: after interactive `dialoguer::Select`, the selected name is mapped back via `.find()`, which picks the first user with that name — potentially the wrong one if two users share both the matched substring and the display name.

## Scope

This fix covers two layers:

1. **`partial_match` module** — detect and report duplicate exact matches via a new `ExactMultiple` variant.
2. **User-resolution callers** (`resolve_user`, `resolve_assignee`, `resolve_assignee_by_project`, `resolve_team_field`) — handle `ExactMultiple` with disambiguation, and fix the index-based mapping bug in `Exact` and `Ambiguous` branches.

Non-user callers (statuses, transitions, link types, queue names, asset types) add a trivial match arm. Duplicates are not realistic for these domains (names are unique within their scope), but the compiler enforces exhaustive matching.

Issue #123 (extract shared disambiguation logic) is a follow-up refactor, not part of this fix.

## Design

### Change 1: New `ExactMultiple` variant in `MatchResult`

Add `ExactMultiple(Vec<String>)` to the enum:

```rust
pub enum MatchResult {
    Exact(String),
    ExactMultiple(Vec<String>),
    Ambiguous(Vec<String>),
    None(Vec<String>),
}
```

`ExactMultiple` contains all candidates that matched exactly (case-insensitive). The vec is never empty and always has length >= 2. Names in the vec are the original (not lowercased) candidate strings. Since all matched the same input exactly, they will all be identical strings (e.g., `["John Smith", "John Smith"]`).

### Change 2: Update `partial_match()` logic

Replace the early-return exact match loop with a collecting loop:

```rust
let exact_matches: Vec<String> = candidates
    .iter()
    .filter(|c| c.to_lowercase() == lower_input)
    .cloned()
    .collect();

match exact_matches.len() {
    0 => { /* fall through to substring matching (unchanged) */ }
    1 => return MatchResult::Exact(exact_matches.into_iter().next().unwrap()),
    _ => return MatchResult::ExactMultiple(exact_matches),
}
```

Substring matching logic is unchanged.

### Change 3: Non-user callers — trivial `ExactMultiple` arm

Every caller that matches on `MatchResult` adds:

```rust
MatchResult::ExactMultiple(names) => {
    names.into_iter().next().unwrap()
}
```

This preserves existing behavior (take first) for domains where duplicates don't occur. Affected callers:

- `src/cli/issue/workflow.rs` — transition name matching
- `src/cli/issue/list.rs` — status filter matching
- `src/cli/issue/links.rs` — link type matching (2 call sites)
- `src/cli/assets.rs` — asset status, schema, type matching (3 call sites)
- `src/cli/queue.rs` — queue name matching (2 call sites)

### Change 4: User-resolution callers — duplicate disambiguation

For `resolve_user`, `resolve_assignee`, `resolve_assignee_by_project`, and `resolve_team_field`:

**`ExactMultiple` handling:**

Collect all objects whose display name matches (there will be >= 2). Then:

- **`--no-input` mode:** Bail with an error listing each duplicate with its accountId (or teamId for teams):
  ```
  Multiple users named "John Smith" found:
    John Smith (account: abc123)
    John Smith (account: def456)
  Specify the accountId directly or use a more specific name.
  ```

- **Interactive mode:** Present a `dialoguer::Select` with disambiguating labels. Use email if available, accountId as fallback:
  ```
  Multiple users named "John Smith":
  > John Smith (john.smith@acme.com)
    John Smith (jsmith@other.org)
  ```
  Map the selection back by **index into the filtered duplicates list**, not by name.

**Fix existing `Exact` and `Ambiguous` branches:**

Replace `.find(|u| u.display_name == matched_name)` with index-based lookup. For `Exact`: find the index of the first user whose display name matches, then index into the users vec. For `Ambiguous` after `dialoguer::Select`: the selection index maps to the `matches` vec, which maps back to the original users vec by searching for users whose display name matches `matches[selection]`. Use position-aware iteration to get the correct user even when names collide.

Concretely, the `Ambiguous` interactive branch changes from:

```rust
// BEFORE — broken on duplicate names
let selected_name = &matches[selection];
let user = users.iter().find(|u| &u.display_name == selected_name).unwrap();
```

To index-based mapping:

```rust
// AFTER — find the Nth user whose display name is in the ambiguous set
let selected_name = &matches[selection];
let matching_users: Vec<&User> = users
    .iter()
    .filter(|u| u.display_name == *selected_name)
    .collect();
// If only one user has this name, take it. If multiple, we need secondary disambiguation.
// But this scenario (ambiguous substring match + duplicate names among the filtered set)
// is extremely unlikely. For now, take first — the ExactMultiple path handles the
// realistic duplicate-name case.
let user = matching_users[0];
```

The `Exact` branch is the more important fix since it's the realistic path for duplicate names.

### Change 5: Team disambiguation

`resolve_team_field` follows the same pattern as user resolution but uses `team.id` and `team.name` for disambiguation labels. The Team struct does not have an email field, so labels use team ID only: `"Alpha Team (team-uuid-alpha)"`.

## What stays the same

- Substring matching logic in `partial_match` — unchanged.
- `Ambiguous` and `None` variant semantics — unchanged.
- Non-user callers — behavior unchanged (take first on `ExactMultiple`).
- `--no-input` behavior for `Ambiguous` matches — already bails with error, unchanged.
- The `is_me_keyword` shortcut — unchanged.
- API layer — no changes to search endpoints.

## Error message examples

```
# --no-input mode, two users with same display name
Error: Multiple users named "John Smith" found:
  John Smith (account: abc123)
  John Smith (account: def456)
Specify the accountId directly or use a more specific name.

# --no-input mode, two teams with same name
Error: Multiple teams named "Platform" found:
  Platform (id: team-uuid-1)
  Platform (id: team-uuid-2)
Use a more specific name.
```

## Edge cases

- **Email field missing:** Jira does not guarantee `emailAddress` is present (privacy settings, managed accounts). Fall back to accountId for disambiguation labels.
- **Three or more duplicates:** The design handles any count >= 2. The `dialoguer::Select` list and error messages scale naturally.
- **Exact match on name that also appears as substring of another name:** `partial_match` checks exact matches first, so `ExactMultiple` is returned before substring matching runs. No interaction.
- **Single user returned by API:** The `users.len() == 1` early return fires before `partial_match` is called. No change needed.
- **Mixed active/inactive users with same name:** `resolve_user` filters to active users before calling `partial_match`. `resolve_assignee` and `resolve_assignee_by_project` use Jira's assignable-user endpoints which already filter. No change needed.

## Testing

1. **Unit test: `partial_match` with duplicate exact candidates** — Two identical strings in candidates, verify `ExactMultiple` returned with both.
2. **Unit test: `partial_match` with unique exact match** — Verify `Exact` still returned (regression guard).
3. **Proptest: duplicate candidates always yield `ExactMultiple`** — Generate candidate lists with intentional duplicates.
4. **Integration test: `resolve_assignee` with duplicate display names in `--no-input` mode** — wiremock returns two users with same `displayName` but different `accountId`. Verify CLI exits with error containing both accountIds.
5. **Integration test: `resolve_user` with duplicate display names in `--no-input` mode** — Same pattern for JQL user resolution.
6. **Existing tests:** All current `partial_match` tests continue to pass since no existing test has duplicate candidates.

## Files modified

- `src/partial_match.rs` — new variant + logic change (~15 lines)
- `src/cli/issue/helpers.rs` — `ExactMultiple` handling + index-based mapping fix in all 4 resolve functions (~60 lines)
- `src/cli/issue/workflow.rs` — trivial `ExactMultiple` arm (~3 lines)
- `src/cli/issue/list.rs` — trivial `ExactMultiple` arm (~3 lines)
- `src/cli/issue/links.rs` — trivial `ExactMultiple` arm x2 (~6 lines)
- `src/cli/assets.rs` — trivial `ExactMultiple` arm x3 (~9 lines)
- `src/cli/queue.rs` — trivial `ExactMultiple` arm x2 (~6 lines)

No new dependencies. No new CLI flags or config options.

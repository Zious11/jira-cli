# Issue Move: Accept Target Status Name

**GitHub Issue:** #108

**Goal:** Allow `jr issue move KEY "Completed"` to work when "Completed" is the target status name, not just the transition name.

## Problem

`jr issue move` currently matches user input only against transition names. Many users think in terms of target statuses ("move to Done") rather than transitions ("execute the Done transition"). When transition and status names differ (e.g., transition "Complete" → status "Completed"), users get a confusing error:

```
Error: No transition matching "Completed". Available: Review, Cancel, Complete
```

## Design

### Unified candidate pool

Replace the current single-pass transition-name matching with a unified candidate pool that includes both transition names and target status names.

1. For each transition, collect two candidate strings: the transition `name` and the target status `to.name`.
2. Each candidate maps back to its source transition.
3. Deduplicate by candidate string (case-insensitive). If a transition name equals its status name (e.g., `"Done"` → `"Done"`), keep one entry. This is the common case in default Jira workflows — behavior is unchanged.
4. Run `partial_match` once against the deduplicated candidate list.
5. If match resolves to a single transition → use it.
6. If match resolves to multiple transitions → treat as ambiguous (existing interactive prompt / `--no-input` error).

### Matching priority within partial_match

The existing `partial_match` function already handles priority correctly:

- Exact match (case-insensitive) takes precedence over substring match.
- If user types `"Complete"` and both `"Complete"` (transition) and `"Completed"` (status) are in the pool, `"Complete"` is an exact match and wins.
- If user types `"Completed"` and only `"Completed"` (status) is in the pool as an exact match, it wins.

No changes to `partial_match` are needed.

### Ambiguity handling

Ambiguity arises when user input partially matches multiple candidates in the unified pool. For example, typing `"Re"` when transitions include `"Reopen"` and `"Review"`.

Note: If two transitions lead to the same status (e.g., "Reopen" → "Open" and "Restart" → "Open"), deduplication means `"Open"` only appears once in the pool (mapped to the first transition). Typing `"Open"` is an exact match, not ambiguous. This is acceptable — both transitions reach the same status. If the user needs a specific transition's post-functions, they can type the transition name directly.

When ambiguity does occur:

- **Interactive mode:** Show disambiguation prompt listing the matching candidates.
- **`--no-input` mode:** Error with the list of matches.

This is consistent with existing ambiguous-transition-name handling.

### Idempotency check

The previous idempotency check compared user input only against the issue's current status name (case-insensitive). That correctly handled status-name input — e.g., if the issue is already in `"Completed"` and the user types `"Completed"`, the early-return fires before matching is attempted.

However, once `jr issue move` accepts both transition names and target status names, idempotency should also apply when the user types a transition name whose target status is the issue's current status. For example, if transition `"Complete"` leads to `"Completed"` and the issue is already in `"Completed"`, typing `"Complete"` should be treated as a no-op rather than attempting the transition.

Implementation change: treat user input as idempotent if either (a) the raw input matches the current status name, or (b) the input matches a transition whose `to.name` matches the current status name (case-insensitive). This preserves the existing early-return for status names and extends the same behavior to equivalent transition-name input.

### Error message improvement

When no match is found, show both transition and status names:

```
No transition matching "Foo". Available: Complete (→ Completed), Review (→ In Review), Cancel (→ Cancelled)
```

The `to` field is always present in the Jira Cloud transitions API response (confirmed via API docs and Perplexity), so the `→ Status` annotation is always available. Our Rust type keeps `to: Option<Status>` for defensive parsing; if `to` is `None`, fall back to just the transition name.

### Interactive prompt

The existing interactive prompt already shows `"Name -> Status"` format. No change needed.

## Scope

### Files changed

- `src/cli/issue/workflow.rs` — `handle_move` function: replace transition-name-only matching with unified pool, update error message format.

### Files NOT changed

- `src/partial_match.rs` — no changes needed.
- `src/types/jira/issue.rs` — `Transition` struct unchanged.
- `src/api/jira/issues.rs` — API calls unchanged.
- `src/cli/mod.rs` — CLI args unchanged.

### Out of scope

- Changing the `transitions` subcommand output format.
- Changing how numeric selection (typing `"1"`, `"2"`) works.
- Adding `--by-status` or `--by-transition` flags to force one matching mode.

## Testing

### Unit tests

None needed — `partial_match` is already well-tested. The logic change is in candidate list construction, which is covered by integration tests.

### Integration tests

1. **Match by transition name** (existing behavior preserved): `"In Progress"` matches transition name directly.
2. **Match by status name** (new behavior): `"Completed"` matches target status name when transition name is `"Complete"`.
3. **Deduplication**: When transition name equals status name (e.g., `"Done"` → `"Done"`), no duplicate candidates — single match.
4. **Shared target status name**: Two transitions leading to the same status produce one deduplicated status-name candidate, so an exact status-name match is not ambiguous solely for that reason.
5. **Error message format**: No match → error shows `"Name (→ Status)"` format.
6. **Idempotent with status name input**: Issue already in target status → exit 0 with "already in status" message.
7. **Idempotent with transition name input**: Issue already in target status, user types transition name → exit 0 with "already in status" message.

## API Validation

- **`to` field always present:** Confirmed by Jira Cloud REST API docs and Perplexity. Every transition has a target status object with `name`, `id`, `self`, and `description` fields.
- **Multiple transitions to same status:** Confirmed possible. Different transitions may have different post-functions. Under this design, the shared status-name candidate is deduplicated, so an exact status-name match is not ambiguous solely because multiple transitions reach that status.
- **Default workflows:** Transition names match status names in 3 of 4 transitions (`To Do`, `In Progress`, `Done`). The 4th (`Create` → `To Do`) is an INITIAL transition. Deduplication makes this a no-op for default workflow users.

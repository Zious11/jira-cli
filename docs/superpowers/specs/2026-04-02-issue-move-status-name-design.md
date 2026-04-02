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

### Ambiguity when multiple transitions share a status name

If two transitions lead to the same status (e.g., "Reopen" → "Open" and "Restart" → "Open"), and the user types "Open", both transitions match via status name. This is treated as ambiguous:

- **Interactive mode:** Show disambiguation prompt listing the matching transitions.
- **`--no-input` mode:** Error with the list of matches.

This is consistent with existing ambiguous-transition-name handling and is correct because different transitions to the same status may have different post-functions/side effects.

### Idempotency check

The existing idempotency check compares user input against the issue's current status name (case-insensitive). This already works for both transition names and status names — if the issue is in "Completed" and the user types `"Completed"`, the early-return fires before matching is attempted. No change needed.

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
4. **Ambiguous status name**: Two transitions leading to same status — treated as ambiguous in `--no-input` mode.
5. **Error message format**: No match → error shows `"Name (→ Status)"` format.
6. **Idempotent with status name input**: Issue already in target status → exit 0 with "already in status" message.

## API Validation

- **`to` field always present:** Confirmed by Jira Cloud REST API docs and Perplexity. Every transition has a target status object with `name`, `id`, `self`, and `description` fields.
- **Multiple transitions to same status:** Confirmed possible. Different transitions may have different post-functions. Treating as ambiguous is correct.
- **Default workflows:** Transition names match status names in 3 of 4 transitions (`To Do`, `In Progress`, `Done`). The 4th (`Create` → `To Do`) is an INITIAL transition. Deduplication makes this a no-op for default workflow users.

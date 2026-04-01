# Assets Tickets Status Filtering

**Issue:** #89
**Date:** 2026-04-01
**Status:** Design

## Problem

`jr assets tickets OBJ-1` returns all tickets linked to an asset with no way to filter by status. For assets with many linked tickets, the only option is to fetch everything and filter externally.

## Solution

Add `--open` and `--status` client-side filtering flags to `assets tickets`.

```
jr assets tickets CUST-5 --open
jr assets tickets CUST-5 --status "In Progress"
jr assets tickets CUST-5 --open --limit 10
```

## CLI Flags

Add to `AssetsCommand::Tickets`:

- `--open` — exclude tickets in the Done status category
- `--status <NAME>` — filter to a specific status (case-insensitive substring match)
- `--open` and `--status` conflict with each other (same pattern as `issue list`)
- Filtering applies **before** `--limit` truncation

## Filter Logic

The Assets connected-tickets API (`GET /objectconnectedtickets/{objectId}/tickets`) has no server-side filtering. All filtering is client-side on the fetched response.

### `--open`

Retain tickets where `status.colorName` is not `"green"`. The `colorName` field maps to Jira's fixed status categories: `"green"` = Done, `"yellow"` = In Progress, `"blue-gray"` = To Do. These mappings are fixed across all Jira Cloud instances.

Tickets with `status: None` are included (unknown status is not assumed to be Done).

### `--status`

Case-insensitive substring match on `status.name` using the existing `partial_match` module. Statuses are extracted from the fetched tickets for disambiguation.

If ambiguous: error listing matching statuses. If no match: error listing available statuses.

### Ordering

1. Fetch all tickets from API
2. Apply `--open` or `--status` filter
3. Apply `--limit` truncation
4. Display

## Error Handling

- `--status "xyz"` with no match: `No status matching "xyz". Available: In Progress, Done, To Do`
- `--status` ambiguous: `Ambiguous status "in". Matches: In Progress, In Review`
- Zero tickets after filtering: normal empty output, not an error
- Tickets with missing `status` field: included by `--open`, excluded by `--status` (no status name to match)

## Changes by File

| File | Change |
|------|--------|
| `src/cli/mod.rs` | Add `--open` and `--status` to `AssetsCommand::Tickets` with `conflicts_with` |
| `src/cli/assets.rs` | Filter tickets in `handle_tickets` before display/limit |

## Testing Strategy

### Unit tests

- Filter function: `--open` excludes green, keeps yellow/blue-gray/None
- Filter function: `--status` partial match
- Filter applies before `--limit`

### CLI smoke tests

- `--open` and `--status` conflict produces clap error

## API Constraints (Validated)

- Connected-tickets endpoint has no query parameters for filtering (confirmed via Context7 API docs)
- `status.colorName` maps to fixed Jira status categories: `"green"` = Done, `"yellow"` = In Progress, `"blue-gray"` = To Do (confirmed via Perplexity + Context7 status category API)
- These category-color mappings are fixed across all Jira Cloud instances (confirmed)

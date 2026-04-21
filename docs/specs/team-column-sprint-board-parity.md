# Team Column Parity for `sprint current` and `board view`

**Issue:** #246 — deferred follow-up from #191 (which closed #247 adding Team column to `issue list`).

## Problem

`jr issue list` shows a Team column when:

1. `team_field_id` is configured in `config.global.fields`
2. At least one returned issue has a populated team UUID under `fields.extra[team_field_id]`
3. Output is in `Table` mode (skipped for JSON since JSON consumers see the raw UUID under `fields.extra`)

Two other list-like commands show the same issues but omit the Team column, breaking UX consistency:

- `jr sprint current` (`src/cli/sprint.rs::handle_current`, around line 279–288)
- `jr board view` (`src/cli/board.rs::handle_view`, around line 213)

## Scope

**In scope:** thread the existing `list.rs` team-resolution pattern into both handlers. Gating rules remain identical — show only when configured AND populated AND Table mode.

**Out of scope:**

- Changing the team-resolution pattern itself. Cache layout, fallback behavior (UUID when name missing), and O(1) `HashMap<String, String>` lookup stay exactly as #191 established them.
- Auto-refreshing the team cache on miss. That's covered by #190 (shipped) and applies to `issue edit --team`, not display-only read paths. Display falls back to the UUID silently when the cache doesn't have the name.
- `board list` output. Lists boards, not issues, so there's no per-issue Team column to show.
- `sprint list`. Same — shows sprints, not issues.

## Design

Each handler gains the same local pattern, with a small structural variation driven by existing code shape (see per-file notes below):

```rust
// Request the team field in the API call's `extra` slice so the raw UUID
// is returned under `issue.fields.extra[team_field_id]`.
let team_field_id = config.global.fields.team_field_id.as_deref();
let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
if let Some(t) = team_field_id {
    extra.push(t);
}

// ... fetch issues via get_sprint_issues / search_issues with &extra ...

// Table-only: resolve UUIDs → display strings once. The Table-mode gate
// is structurally enforced either via a chained `matches!()` guard
// (list.rs, board.rs) or by being nested inside the `OutputFormat::Table`
// match arm (sprint.rs). Both achieve the same effect: no cache I/O for
// JSON consumers.
let team_displays: Vec<String> = if let Some(field_id) = team_field_id {
    let uuids: Vec<Option<String>> = issues
        .iter()
        .map(|i| i.fields.team_id(field_id, client.verbose()))
        .collect();
    if uuids.iter().any(|u| u.is_some()) {
        let team_map: HashMap<String, String> = crate::cache::read_team_cache()
            .ok()
            .flatten()
            .map(|c| c.teams.into_iter().map(|t| (t.id, t.name)).collect())
            .unwrap_or_default();
        uuids
            .iter()
            .map(|u| match u {
                Some(uuid) => team_map.get(uuid).cloned().unwrap_or_else(|| uuid.clone()),
                None => "-".to_string(),
            })
            .collect()
    } else {
        Vec::new()
    }
} else {
    Vec::new()
};
let show_team_col = !team_displays.is_empty();

// Per-row: feed team display into format_issue_row.
let rows: Vec<Vec<String>> = issues
    .iter()
    .enumerate()
    .map(|(i, issue)| {
        let team = if show_team_col {
            Some(team_displays[i].as_str())
        } else {
            None
        };
        format_issue_row(issue, sp_field_id, /* assets */ None, team)
    })
    .collect();
let headers = issue_table_headers(sp_field_id.is_some(), /* show_assets */ false, show_team_col);
```

### `sprint.rs` specifics

- `sp_field_id` is already threaded. Just add `team_field_id`, update the `extra` slice, and replace the current `None, None` tail on `format_issue_row` with team resolution.
- Headers currently passed as `issue_table_headers(sp_field_id.is_some(), false, false)`. Third arg becomes `show_team_col`.

### `board.rs` specifics

- Currently calls `format_issue_rows_public(&issues)` which hardcodes `team: None`. That helper stays as-is (it's still used for assets-free path in issue subcommands).
- Replace the `format_issue_rows_public` call + the hardcoded `&["Key", "Type", ...]` header array with an inline loop using `format_issue_row` and `issue_table_headers`, matching the sprint shape.
- Two API call sites (scrum `get_sprint_issues`, kanban `search_issues`) both need the `extra` slice updated to include `team_field_id`. `board view` does not use story points, so `extra` is just the optional team-field ID.

## Tests

Integration tests in new file `tests/team_column_parity.rs`:

- `sprint_current_shows_team_column_when_populated` — mock a sprint with 2 issues carrying team UUIDs, configure `team_field_id`, pre-populate the team cache, and assert Table-mode stdout contains both the "Team" header and resolved team names.
- `sprint_current_omits_team_column_when_field_id_not_configured` — no `team_field_id` in config → no Team header in stdout.
- `sprint_current_omits_team_column_when_no_issue_has_team` — `team_field_id` configured but zero issues have populated team → no Team header.
- `board_view_kanban_shows_team_column_when_populated` — same as sprint but via `jr board view` with a kanban board mock.
- `board_view_scrum_shows_team_column_when_populated` — scrum path (delegates to `get_sprint_issues`).
- `board_view_omits_team_column_when_field_id_not_configured`.

Unit tests: none needed. The pattern is already covered by `list.rs` unit tests; the handlers are thin glue.

## Backwards Compatibility

- Users without `team_field_id` configured: no visible change. Team column skipped as before.
- Users with `team_field_id` configured but no team-bearing issues in the query: no visible change. Column skipped.
- Users with `team_field_id` configured AND team-bearing issues: Team column now appears in sprint/board output (matching `issue list`).
- JSON output: unchanged in all cases. `fields.extra[team_field_id]` continues to surface the raw UUID.

No flag changes, no API surface changes.

## Risks & Mitigations

- **Sprint/board issue sets are typically smaller than generic issue list** (one sprint = ~10–50 issues), so the added cache read is a one-time <1ms cost. Same filesystem I/O pattern as `list.rs`. Negligible.
- **Team cache miss for populated issue** falls back to showing the raw UUID. Already the established behavior per #191. If a team name was renamed in Jira without refreshing the local cache, the output will show the old name or UUID — acceptable since `jr team list --refresh` covers the refresh case.

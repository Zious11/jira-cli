# Story Points Support

## Overview

Add full CRUD support for story points in `jr`, with configurable custom field ID discovery, sprint point summaries, and per-instance field resolution.

Story points in Jira are custom fields — there is no standard field ID. Common variants:

| Field ID | Name | Schema Custom Type |
|----------|------|--------------------|
| `customfield_10031` | Story Points | `customfieldtypes:float` (classic) |
| `customfield_10016` | Story point estimate | `greenhopper.jira:jsw-story-points` (JSW/next-gen) |

Different projects on the same instance may use different fields. The field ID must be discovered or configured.

## Field Discovery & Configuration

### Discovery Flow

1. Check `config.global.fields.story_points_field_id` — if set, use it immediately
2. If not set, call `GET /rest/api/3/field` and filter for fields where:
   - `custom == true`
   - `schema.type == "number"`
   - `name` case-insensitively matches "Story Points" or "Story point estimate"
3. **Single match** → persist to `~/.config/jr/config.toml` under `[fields]`, return the ID
4. **Multiple matches** → prompt user to select (in `--no-input` mode, error listing the matches)
5. **No match** → error: `"No story points field found. Set story_points_field_id under [fields] in ~/.config/jr/config.toml"`

### Config Shape

```toml
[fields]
team_field_id = "customfield_10001"         # existing
story_points_field_id = "customfield_10031"  # new
```

### `jr init` Integration

After the existing org metadata and team prefetch steps, run story points field discovery and persist the result. If multiple matches, prompt during init.

## Data Model

### Value Type

Story points are `f64` (float). The Jira API accepts both integer and decimal values — `5`, `5.0`, and `3.5` are all valid. The API always returns float (`13` → `13.0`).

### IssueFields Changes

Add a `#[serde(flatten)]` catch-all to `IssueFields` for dynamic custom fields:

```rust
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct IssueFields {
    pub summary: String,
    pub description: Option<Value>,
    pub status: Option<Status>,
    #[serde(rename = "issuetype")]
    pub issue_type: Option<IssueType>,
    pub priority: Option<Priority>,
    pub assignee: Option<User>,
    pub project: Option<IssueProject>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl IssueFields {
    pub fn story_points(&self, field_id: &str) -> Option<f64> {
        self.extra.get(field_id)?.as_f64()
    }
}
```

The configured `story_points_field_id` tells callers which key to pass to `story_points()`.

### Field Struct Update

Add `schema` to `Field` in `src/api/jira/fields.rs`:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct Field {
    pub id: String,
    pub name: String,
    pub custom: Option<bool>,
    pub schema: Option<FieldSchema>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FieldSchema {
    #[serde(rename = "type")]
    pub field_type: String,
    pub custom: Option<String>,
}
```

## CLI Interface

### `jr issue create` — New `--points` Flag

```bash
jr issue create -p FOO -t Story -s "Add login" --points 5
```

- Flag: `--points <f64>` (optional)
- Sets the configured custom field ID in the create payload: `fields[customfield_XXXXX] = json!(5.0)`

### `jr issue edit` — New `--points` Flag

```bash
jr issue edit KEY-123 --points 8
```

- Same pattern as other edit flags
- Added to the "no fields specified" error message

### `jr issue view` — Story Points Row

Display story points in the detail view when the field is configured:

- Display as integer when whole number (`5` not `5.0`), decimal when fractional (`3.5`)
- Show `(none)` when unset, same pattern as labels
- If no story points field configured/discoverable, omit the row silently

### `jr issue list` — Optional `--points` Flag

```bash
jr issue list --points
```

- Default table: Key, Type, Status, Priority, Assignee, Summary (unchanged)
- With `--points`: Key, Type, Status, Priority, Points, Assignee, Summary
- Points column shows `-` when unset
- `--output json` always includes story points in the `extra` map regardless of `--points` flag
- If no story points field configured, `--points` flag is silently ignored

### Field Resolution

All commands needing story points resolve the field ID once per invocation:

1. Config → return immediately
2. Discover → single match → persist → return
3. Discover → multiple matches → prompt or error
4. Discover → no match → error

## API Integration

### Validated Request/Response Formats

All formats validated against live Jira Cloud instance.

**Create issue with story points:**
```
POST /rest/api/3/issue
{"fields": {"project": {"key": "FOO"}, "issuetype": {"name": "Story"}, "summary": "...", "customfield_10031": 5.0}}
→ 201
```

**Update story points:**
```
PUT /rest/api/3/issue/{key}
{"fields": {"customfield_10031": 8.0}}
→ 204
```

**Search with story points:**
```
POST /rest/api/3/search/jql
{"jql": "...", "fields": ["summary", "status", "issuetype", "priority", "assignee", "project", "description", "customfield_10031"]}
→ customfield_10031 appears as plain f64 or null in response
```

**Get single issue:**
```
GET /rest/api/3/issue/{key}?fields=summary,status,...,customfield_10031
→ Same — plain f64 or null
```

## Sprint Points Summary

### Display

When running `jr sprint current`, always show the Points column and a summary line:

```
Sprint: Sprint 42  |  Points: 5/8 completed  (2 unestimated)
```

### Behavior

- Always show the Points column in sprint output (sprint planning is the primary use case for points)
- Summary line: `completed / total` — "completed" = issues where `statusCategory.key == "done"`
- If any issues lack story points, append `(N unestimated)` so the user knows the total is incomplete
- `--output json` includes a `sprint_summary` object: `{"completed_points": 5.0, "total_points": 8.0, "unestimated_count": 1}`
- If no story points field is configured/discoverable, skip the Points column and summary line silently

### Computation

Iterate sprint issues after fetch:
- `total_points`: sum of `story_points()` for issues that have a value
- `completed_points`: sum of `story_points()` for issues where `statusCategory.key == "done"`
- `unestimated_count`: count of issues where `story_points()` returns `None`

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Field not found on instance | Error: "No story points field found. Set `story_points_field_id` under `[fields]` in `~/.config/jr/config.toml`" |
| Field not on edit screen (Jira 400) | Error: "Cannot set story points on this issue type. The field may not be on the edit screen. Check your Jira project settings." |
| Invalid `--points` value | Clap handles — not a valid float |
| `--points` used but no field configured | Error before API call: "Story points field not configured. Run `jr init` or set `story_points_field_id` in config." |
| Sprint with no field configured | Silently skip Points column and summary line |
| Multiple fields found, `--no-input` | Error listing all matches |

## Display Formatting

- Whole numbers display without decimal: `5.0` → `"5"`, `13.0` → `"13"`
- Fractional numbers keep decimal: `3.5` → `"3.5"`
- Unset in view: `"(none)"`
- Unset in list/sprint table: `"-"`

Helper function:

```rust
fn format_points(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{}", value)
    }
}
```

## Files Touched

| File | Change |
|------|--------|
| `src/types/jira/issue.rs` | Add `extra: HashMap<String, Value>` with `#[serde(flatten)]`, add `story_points()` helper |
| `src/api/jira/fields.rs` | Add `FieldSchema` struct, add `find_story_points_field_id()` |
| `src/api/jira/issues.rs` | Add configured custom field ID to `fields` arrays in `search_issues()` and `get_issue()` |
| `src/cli/mod.rs` | Add `--points` flag to `Create`, `Edit`, `List` commands |
| `src/cli/issue.rs` | Update create/edit/view/list handlers, add `resolve_story_points_field_id()` |
| `src/cli/sprint.rs` | Add Points column to sprint output, add summary line computation |
| `src/config.rs` | Add `story_points_field_id` to `FieldsConfig` |
| `src/cli/init.rs` | Add story points field discovery during init |

## Testing Strategy

### Unit Tests

- `IssueFields::story_points()` — returns `Some(f64)` when present, `None` when missing/wrong type
- `find_story_points_field_id()` filtering — mock field lists with various names
- `format_points()` — whole numbers vs decimals
- Sprint summary computation — totals, completed, unestimated counts
- Existing `build_fallback_jql` tests — no regression

### Integration Tests (wiremock)

- `GET /rest/api/3/field` — discovery with 0, 1, and 2 matches
- `POST /rest/api/3/issue` — verify custom field in create payload
- `PUT /rest/api/3/issue/{key}` — verify custom field in update payload
- `POST /rest/api/3/search/jql` — verify custom field in `fields` array, verify `extra` map captures value

### Edge Cases

- Multiple story point fields found + `--no-input` → error
- Config has field ID set → skips discovery
- `--points 0` → valid, sets to 0.0
- Issue type doesn't support story points → surface Jira's 400 error

# Include Standard Fields in Issue View — Design Spec

**Issue:** #59

**Goal:** Add `created`, `updated`, `reporter`, `resolution`, `components`, and `fixVersions` to `jr issue view` JSON output, and show `created`, `updated`, `reporter` in the table view.

## Problem

`jr issue view <KEY> --output json` omits several standard Jira fields commonly needed for scripting and automation:

- `created` — issue creation timestamp
- `updated` — last update timestamp
- `reporter` — who created the issue
- `resolution` — resolution status (e.g., "Fixed", "Won't Do")
- `components` — project components
- `fixVersions` — target fix versions

These fields are available from the Jira API but are not requested by `get_issue()` or `search_issues()`, so they never appear in the output.

**Current `get_issue` field list:**
```
summary,status,issuetype,priority,assignee,project,description,labels,parent,issuelinks
```

Missing: `created`, `updated`, `reporter`, `resolution`, `components`, `fixVersions`.

## Scope

| Output | Fields added |
|--------|-------------|
| `issue view --output json` | All 6: `created`, `updated`, `reporter`, `resolution`, `components`, `fixVersions` |
| `issue view` (table) | 3: `created`, `updated`, `reporter` |
| `issue list --output json` | All 6 (via `search_issues` field list) |
| `issue list` (table) | None — already crowded |

## API Field Structures

From the Jira REST API v3:

- **`created`** / **`updated`** — ISO 8601 strings (e.g., `"2026-03-20T14:32:00.000+0000"`)
- **`reporter`** — Simplified User object with `accountId`, `displayName`, `active`. Same shape as `assignee`. The existing `User` type handles this — `emailAddress: Option<String>` naturally handles its absence.
- **`resolution`** — Object with `name` when resolved (e.g., `{"name": "Fixed"}`), `null` when unresolved
- **`components`** — Array of objects with `name` (e.g., `[{"name": "Backend"}]`). Always an array, even when empty (`[]`).
- **`fixVersions`** — Array of version objects with `name`, `released`, `releaseDate`

## Fix

### 1. New Types (`src/types/jira/issue.rs`)

Three small structs following existing patterns (`Status`, `Priority`, `IssueType` all capture just `name`):

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct Resolution {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Component {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Version {
    pub name: String,
    pub released: Option<bool>,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
}
```

Only fields needed for display are captured. Additional API fields (`id`, `description`, `self`) are ignored during deserialization — serde skips unknown fields by default.

### 2. `IssueFields` Changes (`src/types/jira/issue.rs`)

Add 6 fields to the existing struct. These were previously captured (if requested) in the `#[serde(flatten)] extra: HashMap<String, Value>` catch-all. With typed fields, serde routes them to the typed field first — confirmed as the correct serde behavior.

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
    pub reporter: Option<User>,
    pub project: Option<IssueProject>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub resolution: Option<Resolution>,
    #[serde(default)]
    pub components: Option<Vec<Component>>,
    #[serde(rename = "fixVersions", default)]
    pub fix_versions: Option<Vec<Version>>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    pub parent: Option<ParentIssue>,
    pub issuelinks: Option<Vec<IssueLink>>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
```

The `Default` derive requires all new types to either implement `Default` or be wrapped in `Option`. All 6 new fields are `Option`, so no `Default` impl needed on the new types.

### 3. API Field Lists (`src/api/jira/issues.rs`)

**`get_issue()`** — add to the hardcoded field string:

```rust
let mut fields =
    "summary,status,issuetype,priority,assignee,reporter,project,description,labels,parent,issuelinks,created,updated,resolution,components,fixVersions".to_string();
```

**`search_issues()`** — add to the fields vec:

```rust
let mut fields = vec![
    "summary",
    "status",
    "issuetype",
    "priority",
    "assignee",
    "reporter",
    "project",
    "description",
    "created",
    "updated",
    "resolution",
    "components",
    "fixVersions",
];
```

### 4. Table View (`src/cli/issue/list.rs` — `handle_view`)

Add 3 rows after the Assignee row, before Project:

```rust
rows.push(vec![
    "Reporter".into(),
    issue
        .fields
        .reporter
        .as_ref()
        .map(|r| r.display_name.clone())
        .unwrap_or_else(|| "Unassigned".into()),
]);
rows.push(vec![
    "Created".into(),
    issue
        .fields
        .created
        .as_deref()
        .map(format_comment_date)
        .unwrap_or_default(),
]);
rows.push(vec![
    "Updated".into(),
    issue
        .fields
        .updated
        .as_deref()
        .map(format_comment_date)
        .unwrap_or_default(),
]);
```

The existing `format_comment_date()` function already handles Jira's ISO 8601 format and produces `YYYY-MM-DD HH:MM` output — reused here.

### 5. JSON Output

No changes needed. `IssueFields` derives `Serialize`, so the 6 new typed fields automatically appear in `--output json`. The existing `render_json(&issue)` call handles it.

## Expected Output

### Table view (`jr issue view PROJ-123`):

```
┌─────────────┬─────────────────────────────────┐
│ Field       │ Value                           │
╞═════════════╪═════════════════════════════════╡
│ Key         │ PROJ-123                        │
│ Summary     │ Fix login redirect              │
│ Type        │ Bug                             │
│ Status      │ In Progress                     │
│ Priority    │ High                            │
│ Assignee    │ John Doe                        │
│ Reporter    │ Jane Smith                      │  ← NEW
│ Created     │ 2026-03-20 14:32                │  ← NEW
│ Updated     │ 2026-03-25 09:15                │  ← NEW
│ Project     │ My Project (PROJ)               │
│ Labels      │ bug, frontend                   │
│ Parent      │ PROJ-100 (Login Epic)           │
│ Links       │ blocks PROJ-456 (Deploy)        │
│ Points      │ 5                               │
│ Description │ Users are redirected to...       │
└─────────────┴─────────────────────────────────┘
```

### JSON view (`jr issue view PROJ-123 --output json`):

```json
{
  "key": "PROJ-123",
  "fields": {
    "summary": "Fix login redirect",
    "status": { "name": "In Progress", "statusCategory": { "name": "In Progress", "key": "indeterminate" } },
    "issuetype": { "name": "Bug" },
    "priority": { "name": "High" },
    "assignee": { "accountId": "5b10a284...", "displayName": "John Doe", "active": true },
    "reporter": { "accountId": "5b10a2844c20...", "displayName": "Jane Smith", "active": true },
    "created": "2026-03-20T14:32:00.000+0000",
    "updated": "2026-03-25T09:15:22.000+0000",
    "resolution": { "name": "Fixed" },
    "components": [{ "name": "Backend" }],
    "fixVersions": [{ "name": "v2.0", "released": false, "releaseDate": "2026-04-01" }],
    "project": { "key": "PROJ", "name": "My Project" },
    "labels": ["bug", "frontend"],
    "description": { "type": "doc", "version": 1, "content": [...] }
  }
}
```

## Files Changed

| File | Change |
|------|--------|
| `src/types/jira/issue.rs` | Add `Resolution`, `Component`, `Version` types; add 6 fields to `IssueFields` |
| `src/api/jira/issues.rs` | Add field names to `get_issue()` and `search_issues()` field lists |
| `src/cli/issue/list.rs` | Add Reporter, Created, Updated rows to `handle_view` table |

## What Doesn't Change

- `issue list` table output — unchanged (columns already crowded)
- `issue create` / `issue edit` — no changes
- `Comment` type — already has `created: Option<String>`, no conflict
- `handle_search()`, `handle_comments()`, `handle_list()` table rendering — unchanged
- `extra` HashMap — still captures custom fields; the 6 new fields are now typed and won't land in `extra`

## Testing

### Unit Tests (`src/types/jira/issue.rs`)

- Deserialize `IssueFields` with all 6 new fields present — verify typed access
- Deserialize with all 6 fields absent/null — verify all default to `None`
- Deserialize `Resolution` with `name` field
- Deserialize `Component` with `name` field
- Deserialize `Version` with `name`, `released`, `releaseDate` (and with optional fields absent)
- Verify `fixVersions` JSON key maps to `fix_versions` Rust field via serde rename
- Verify new fields don't appear in `extra` HashMap when typed field is present

### Integration Tests (`tests/`)

- Wiremock `GET /rest/api/3/issue/{key}` returning all 6 fields — verify JSON output contains them
- Wiremock returning null/empty for all 6 fields — verify graceful handling
- Verify table output contains Reporter, Created, Updated rows with formatted values

### Live Verification

```bash
jr issue view <KEY> --output json | jq '.fields | {created, updated, reporter, resolution, components, fixVersions}'
jr issue view <KEY>  # table should show Reporter, Created, Updated
```

## Backward Compatibility

**JSON output:** Additive only — 6 new fields appear that weren't previously present. No existing fields change. Consumers using `jq .fields.summary` are unaffected.

**Table output:** 3 new rows added (Reporter, Created, Updated) after Assignee. No existing rows change position or content.

**`extra` HashMap:** Fields that previously landed in `extra` (if someone was requesting them via a custom integration) now have typed fields. The JSON serialization output is identical — the field appears at the same path with the same structure. The only difference is serde routing during deserialization.

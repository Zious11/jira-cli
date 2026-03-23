# Issue Linking Support

## Overview

Add parent/child relationships and issue linking to `jr`. This enables AI agents to build ticket hierarchies (epic → stories → subtasks) and create peer relationships (blocks, relates to, duplicates) programmatically.

## Parent/Child Relationships

### Set Parent on Create

```bash
jr issue create -p FOO -t Story -s "Login page" --parent FOO-42
```

Sets `"parent": {"key": "FOO-42"}` in the create payload. Works for subtasks under tasks and stories under epics (both classic and next-gen projects — the `parent` field handles both in modern Jira Cloud).

### Reparent on Edit

```bash
jr issue edit FOO-123 --parent FOO-42
```

Sets `"parent": {"key": "FOO-42"}` via `PUT /rest/api/3/issue/{key}`.

### Display in View

```
│ Parent    ┆ FOO-42 (Auth Epic)         │
```

Shows `KEY (summary)` or `(none)`. Requires adding `parent` to the API `fields` request. Jira returns:

```json
"parent": {
  "key": "FOO-42",
  "fields": {
    "summary": "Auth Epic"
  }
}
```

### API Format (Validated)

**Create:** `POST /rest/api/3/issue` with `{"fields": {"parent": {"key": "FOO-42"}, ...}}`
**Edit:** `PUT /rest/api/3/issue/{key}` with `{"fields": {"parent": {"key": "FOO-42"}}}`
**Read:** `GET /rest/api/3/issue/{key}?fields=...,parent` — returns parent with key and summary

Live-validated: stories under epics use the `parent` field on the configured Jira instance.

## Issue Links

### Create Link

```bash
jr issue link FOO-1 FOO-2 --type blocks
jr issue link FOO-1 FOO-2 --type "relates to"
jr issue link FOO-1 FOO-2                      # defaults to "Relates"
```

Reading as "FOO-1 blocks FOO-2". Calls `POST /rest/api/3/issueLink` with:

```json
{
  "outwardIssue": {"key": "FOO-1"},
  "inwardIssue": {"key": "FOO-2"},
  "type": {"name": "Blocks"}
}
```

In Jira's link model, `outwardIssue` gets the "outward" label of the link type. For "Blocks": outward = "blocks", inward = "is blocked by". So `outwardIssue = FOO-1` means "FOO-1 blocks", and `inwardIssue = FOO-2` means "FOO-2 is blocked by". Validated against live instance link type definitions.

**Response:** Returns nothing on success. Use `post_no_content` (same pattern as `transition_issue`). To get the link ID, fetch the issue's `issuelinks` field afterward.

**Idempotent:** The Jira API itself is idempotent for link creation — duplicate link requests silently succeed (per official Atlassian docs: "If the link request duplicates a link, the response indicates that the issue link was created"). No client-side duplicate check needed.

**Type matching:** Link type names are title case in Jira ("Blocks", "Duplicate", "Relates"). Use case-insensitive partial matching (same `partial_match` module used for transitions). If ambiguous, prompt or error in `--no-input` mode.

### Remove Link

```bash
jr issue unlink FOO-1 FOO-2                    # removes all links between the two
jr issue unlink FOO-1 FOO-2 --type blocks      # removes only "Blocks" link
```

**Flow:**
1. `GET /rest/api/3/issue/{key}?fields=issuelinks` to find matching links
2. Filter by the other issue's key (and optionally by link type)
3. `DELETE /rest/api/3/issueLink/{linkId}` for each match

**Idempotent:** If no matching link found, exit 0 with "no link found" message.

**Response format:**
- Table: `Removed 2 link(s) between FOO-1 and FOO-2`
- JSON: `{"unlinked": true, "count": 2}`

### List Link Types

```bash
jr issue link-types
```

Calls `GET /rest/api/3/issueLinkType`. Response wraps in `{"issueLinkTypes": [...]}`.

Each link type has:
```json
{
  "id": "1010",
  "name": "Blocks",
  "inward": "is blocked by",
  "outward": "blocks"
}
```

**Table output:**

```
┌──────┬───────────┬─────────────────┬─────────────┐
│ ID   ┆ Name      ┆ Outward         ┆ Inward      │
├──────┼───────────┼─────────────────┼─────────────┤
│ 1010 ┆ Blocks    ┆ blocks          ┆ is blocked by │
│ 1000 ┆ Duplicate ┆ duplicates      ┆ is duplicated by │
│ 1020 ┆ Relates   ┆ relates to      ┆ relates to  │
└──────┴───────────┴─────────────────┴─────────────┘
```

### Display Links in View

```
│ Links     ┆ blocks FOO-2 (Fix login)           │
│           ┆ is blocked by FOO-3 (Deploy cert)   │
│           ┆ relates to FOO-5 (Auth redesign)    │
```

Requires adding `issuelinks` to the API `fields` request. The `issuelinks` array in the issue response contains objects with:
- `id` — the link ID (needed for delete)
- `type.name`, `type.inward`, `type.outward` — link type descriptions
- `inwardIssue` or `outwardIssue` — the linked issue with `key` and `fields.summary`

For each link, display the directional description (outward or inward depending on which side this issue is on) followed by the linked issue key and summary.

If no links exist, show `(none)`.

## CLI Interface

### New/Modified Flags

**`IssueCommand::Create`** — add:
```rust
/// Parent issue key
#[arg(long)]
parent: Option<String>,
```

**`IssueCommand::Edit`** — add:
```rust
/// Parent issue key
#[arg(long)]
parent: Option<String>,
```

Update the "no fields specified" error message to include `--parent`.

### New Subcommands

**`IssueCommand::Link`:**
```rust
/// Link two issues
Link {
    /// First issue key (outward)
    key1: String,
    /// Second issue key (inward)
    key2: String,
    /// Link type (partial match, default: "Relates")
    #[arg(long, default_value = "Relates")]
    r#type: String,
},
```

**`IssueCommand::Unlink`:**
```rust
/// Remove link(s) between two issues
Unlink {
    /// First issue key
    key1: String,
    /// Second issue key
    key2: String,
    /// Only remove links of this type (removes all if omitted)
    #[arg(long)]
    r#type: Option<String>,
},
```

**`IssueCommand::LinkTypes`:**
```rust
/// List available link types
LinkTypes,
```

## API Layer

### New File: `src/api/jira/links.rs`

```rust
impl JiraClient {
    /// Create a link between two issues.
    pub async fn create_issue_link(
        &self, outward_key: &str, inward_key: &str, link_type: &str
    ) -> Result<()>;

    /// Delete an issue link by ID.
    pub async fn delete_issue_link(&self, link_id: &str) -> Result<()>;

    /// List all available issue link types.
    pub async fn list_link_types(&self) -> Result<Vec<IssueLinkType>>;
}
```

### Types: `src/types/jira/issue.rs` additions

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct IssueLink {
    pub id: String,
    #[serde(rename = "type")]
    pub link_type: IssueLinkType,
    #[serde(rename = "inwardIssue")]
    pub inward_issue: Option<LinkedIssue>,
    #[serde(rename = "outwardIssue")]
    pub outward_issue: Option<LinkedIssue>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LinkedIssue {
    pub key: String,
    pub fields: Option<LinkedIssueFields>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LinkedIssueFields {
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueLinkType {
    pub id: Option<String>,
    pub name: String,
    pub inward: Option<String>,
    pub outward: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueLinkTypesResponse {
    #[serde(rename = "issueLinkTypes")]
    pub issue_link_types: Vec<IssueLinkType>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ParentIssue {
    pub key: String,
    pub fields: Option<LinkedIssueFields>,
}
```

Add to `IssueFields`:
```rust
    pub parent: Option<ParentIssue>,
    pub issuelinks: Option<Vec<IssueLink>>,
```

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Parent issue not found | Surface Jira's 404: "Issue FOO-999 not found" |
| Link type not found | Error: "Unknown link type 'foo'. Run 'jr issue link-types' to see available types." |
| Ambiguous link type match | Prompt to select (or error listing matches in `--no-input`) |
| Link already exists | API silently succeeds — exit 0 (idempotent by design) |
| No link to remove | Exit 0: "No link found between FOO-1 and FOO-2" |
| Self-linking | Client-side error before API call: "Cannot link an issue to itself." |
| Permission denied | Surface Jira's 401/403 error |

## Output Formats

All commands support `--output json`:

- **`link`:** `{"key1": "FOO-1", "key2": "FOO-2", "type": "Blocks", "linked": true}`
- **`unlink`:** `{"unlinked": true, "count": 2}` or `{"unlinked": false, "count": 0}`
- **`link-types`:** the array from the API response
- **`view`:** parent and links included in the existing JSON output

## Files Touched

| File | Change |
|------|--------|
| `src/cli/mod.rs` | Add `--parent` to Create/Edit, add Link/Unlink/LinkTypes variants |
| `src/cli/issue.rs` | Add handlers for link/unlink/link-types, wire parent into create/edit, update view |
| `src/api/jira/mod.rs` | Add `pub mod links;` |
| `src/api/jira/links.rs` | New — create_issue_link, delete_issue_link, list_link_types |
| `src/api/client.rs` | Add `pub async fn delete(&self, path: &str) -> Result<()>` method |
| `src/api/jira/issues.rs` | Add `issuelinks,parent` to `get_issue()` field request only (not `search_issues`) |
| `src/types/jira/issue.rs` | Add IssueLink, LinkedIssue, IssueLinkType, ParentIssue types, add fields to IssueFields |
| `tests/issue_commands.rs` | Add integration tests for link creation and link type listing |
| `tests/common/fixtures.rs` | Add link-related fixtures |

## Testing Strategy

### Unit Tests
- Partial match on link type names (case-insensitive)
- Display formatting for parent and links rows

### Integration Tests (wiremock)
- `POST /rest/api/3/issueLink` — verify request body format
- `DELETE /rest/api/3/issueLink/{linkId}` — verify correct ID passed
- `GET /rest/api/3/issueLinkType` — verify response parsing
- `GET /rest/api/3/issue/{key}` with `issuelinks,parent` — verify deserialization

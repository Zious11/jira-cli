# Internal vs External Comments for JSM Tickets — Design Spec

**Issue:** #103
**Status:** Draft
**Date:** 2026-04-05

## Problem

`jr issue comment` has no way to specify whether a comment should be internal (agent-only) or external (customer-visible). On JSM service desk projects, this distinction is critical — internal comments are used for analyst notes that should never be exposed to customers.

Currently all comments created via jr use the standard comment API with no visibility control, which defaults to external (customer-visible) on JSM projects.

## Solution

Add an `--internal` flag to `jr issue comment` and display visibility status in `jr issue comments` output.

```
# External (default, customer-visible)
jr issue comment HELP-42 "Customer update"

# Internal (agent-only)
jr issue comment HELP-42 "Investigation notes" --internal
```

## API Approach

Use the standard Jira REST API v3 for both create and list — no JSM-specific API needed.

**Validated against live Jira Cloud instance (2026-04-05):**

- `POST /rest/api/3/issue/{key}/comment` accepts a `properties` array with `sd.public.comment` entity property
- `GET /rest/api/3/issue/{key}/comment?expand=properties` returns `sd.public.comment` on each comment for JSM projects
- Non-JSM projects return empty `properties: []` — clean, no special handling needed

The JSM-specific API (`/rest/servicedeskapi/request/{key}/comment`) was evaluated and rejected:
- Only works for "service request" types, not all JSM issue types (404 on Problem, Task, etc.)
- Uses plain text body instead of ADF (loses markdown support)
- No benefit over the standard API approach

### Why Not the JSM API

| Criteria | Standard API | JSM API |
|----------|-------------|---------|
| Works on all issue types | Yes | Only service requests |
| Body format | ADF (existing support) | Plain text |
| Visibility in response | Via `expand=properties` | `public` boolean |
| Non-JSM projects | Property ignored, empty properties | 404 error |

## Create: `--internal` Flag

When `--internal` is passed, add the `sd.public.comment` entity property to the POST payload:

```json
{
  "body": { "type": "doc", "version": 1, "content": [...] },
  "properties": [
    {
      "key": "sd.public.comment",
      "value": { "internal": true }
    }
  ]
}
```

When `--internal` is NOT passed, omit the `properties` field entirely. Jira's default is external (customer-visible) — confirmed by live testing: comments without `sd.public.comment` have empty `properties: []` and are treated as external.

### Non-JSM Projects

The `sd.public.comment` property is silently accepted and stored on non-JSM projects but has no effect — there is no customer portal to restrict visibility on. The `--internal` flag works on any project without error.

## List: Conditional Visibility Column

Add `?expand=properties` to the GET comments request. For each comment, check for the `sd.public.comment` property.

**Display logic:**
- If ANY comment in the result has `sd.public.comment` → show "Visibility" column
- If NO comments have `sd.public.comment` (non-JSM projects) → omit column entirely

**Per-comment mapping:**
- `sd.public.comment: {internal: true}` → "Internal"
- `sd.public.comment: {internal: false}` → "External"
- No `sd.public.comment` property (but column is shown because other comments have it) → "External" (Jira's default)

### Table Output

```
# JSM project with mixed visibility
┌────────┬──────────┬────────────┬──────────────────────────────┐
│ Author ┆ Date     ┆ Visibility ┆ Body                         │
╞════════╪══════════╪════════════╪══════════════════════════════╡
│ Agent  ┆ 04-05    ┆ Internal   ┆ Investigation notes...       │
│ Agent  ┆ 04-05    ┆ External   ┆ Customer update...           │

# Non-JSM project (no visibility column)
┌────────┬──────────┬──────────────────────────────┐
│ Author ┆ Date     ┆ Body                         │
╞════════╪══════════╪══════════════════════════════╡
│ Dev    ┆ 04-05    ┆ Fixed in commit abc123...    │
```

### JSON Output

Include the raw `properties` array on each comment. Consumers can inspect it directly:

```json
{
  "id": "10042",
  "author": { "displayName": "Agent" },
  "body": { ... },
  "properties": [
    { "key": "sd.public.comment", "value": { "internal": true } }
  ]
}
```

Non-JSM comments have `"properties": []`.

## Implementation

### Files Changed

| File | Change | Description |
|------|--------|-------------|
| `src/cli/mod.rs` | Modify | Add `--internal` flag to `Comment` variant |
| `src/cli/issue/workflow.rs` | Modify | Pass `internal` flag to `add_comment` |
| `src/api/jira/issues.rs` | Modify | Accept `internal` param in `add_comment`, add `expand=properties` to `list_comments` query |
| `src/types/jira/issue.rs` | Modify | Add `properties` field to `Comment` struct, add `EntityProperty` type |
| `src/cli/issue/list.rs` | Modify | Conditional "Visibility" column in `handle_comments` |

### Type Changes

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityProperty {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Comment {
    pub id: Option<String>,
    pub body: Option<Value>,
    pub author: Option<User>,
    pub created: Option<String>,
    #[serde(default)]
    pub properties: Vec<EntityProperty>,
}
```

### API Changes

`add_comment` signature change:

```rust
// Before:
pub async fn add_comment(&self, key: &str, body: Value) -> Result<Comment>

// After:
pub async fn add_comment(&self, key: &str, body: Value, internal: bool) -> Result<Comment>
```

`list_comments` — add `expand=properties` to the query string (no signature change).

## Error Messages

| Scenario | Message |
|----------|---------|
| `--internal` on non-JSM project | No error — property silently accepted |
| Comment creation failure | Existing error handling unchanged |

## Testing

### Handler Tests (wiremock)

1. **`--internal` flag adds property to POST**: Mock `/rest/api/3/issue/{key}/comment`, verify request body contains `properties` array with `sd.public.comment: {internal: true}`
2. **No `--internal` omits property**: Mock same endpoint, verify request body has no `properties` field
3. **Comment listing with visibility**: Mock GET with `expand=properties`, return comments with `sd.public.comment` properties, verify "Visibility" column appears in output
4. **Comment listing without visibility**: Mock GET returning empty properties, verify no "Visibility" column

### Unit Tests

1. **`Comment` deserialization with properties**: Parse JSON with `sd.public.comment` property
2. **`Comment` deserialization without properties**: Parse JSON with empty/missing properties array
3. **`EntityProperty` deserialization**: Parse the `{key, value}` format

## Caveats

- `?expand=properties` on the comment list endpoint works in practice but is not explicitly documented by Atlassian as a supported expansion value. The `expand` parameter itself is documented, and `properties` is a documented field on the Comment model. If this ever breaks, a per-comment fallback exists via `GET /rest/api/3/comment/{id}/properties`.
- The `--internal` flag has no effect on non-JSM projects but is silently accepted. This is intentional — flagging an error would require JSM project detection, adding complexity with no user benefit.

## Out of Scope

- Interactive prompt for visibility selection — `--internal` flag is sufficient
- `--external` flag — omitting `--internal` already defaults to external
- JSM API integration — standard API handles both create and list
- Editing comment visibility after creation — separate feature
- Filtering comments by visibility — YAGNI

# Issue Edit Description Support

**Date:** 2026-04-01
**Issue:** #82
**Status:** Design

## Summary

Add `--description`, `--description-stdin`, and `--markdown` flags to `jr issue edit`, mirroring the existing flags on `jr issue create`. Description updates fully replace the existing description (no append mode).

## Motivation

`jr issue edit` supports editing summary, type, priority, labels, team, points, and parent — but not the description. Users must fall back to raw `curl` calls to update descriptions, losing `jr`'s auth handling, ADF conversion, and output formatting.

Use cases from #82:
- Enriching tickets after creation (e.g., appending implementation notes via scripted workflows)
- Correcting or updating descriptions without opening the browser
- Batch description updates via JQL-driven scripts
- Two-step creation workflows (create with minimal fields, add description later)

## CLI Surface

```
jr issue edit PROJ-123 --description "New description text"
jr issue edit PROJ-123 -d "Short description"
jr issue edit PROJ-123 --description-stdin < file.txt
echo "piped text" | jr issue edit PROJ-123 --description-stdin
jr issue edit PROJ-123 --description-stdin --markdown < notes.md
jr issue edit PROJ-123 --description "**bold text**" --markdown
```

### Flag Definitions

| Flag | Short | Type | Description |
|------|-------|------|-------------|
| `--description` | `-d` | `Option<String>` | Description text (conflicts with `--description-stdin`) |
| `--description-stdin` | — | `bool` | Read description from stdin (conflicts with `--description`) |
| `--markdown` | — | `bool` | Interpret description as Markdown instead of plain text |

### Conflict Rules

- `--description` and `--description-stdin` are mutually exclusive (enforced by clap `conflicts_with`).
- `--markdown` without a description source is silently ignored (matches `create` behavior).
- `--description ""` sets a description with an empty paragraph (valid — Jira stores this as a non-null ADF document with no visible text). To fully clear a description, a future `--no-description` flag would send `"description": null`.

## API Behavior

Uses `PUT /rest/api/3/issue/{key}` with the description in the `fields` object as ADF. This is the same pattern used by all other `edit` fields today.

```json
{
  "fields": {
    "description": {
      "type": "doc",
      "version": 1,
      "content": [
        {
          "type": "paragraph",
          "content": [
            { "type": "text", "text": "Updated description" }
          ]
        }
      ]
    }
  }
}
```

Key behaviors confirmed via Jira REST API v3 documentation:
- **Replace semantics:** The description field is a "set" operation — the entire description is replaced.
- **Atomic updates:** When combined with other fields (summary, priority, etc.), all updates succeed or all fail.
- **ADF required:** The description field in v3 must be Atlassian Document Format, not plain text or HTML.

## Implementation

### CLI Definition (`src/cli/mod.rs`)

Add three fields to `IssueCommand::Edit`:

```rust
Edit {
    // ... existing fields ...

    /// Description
    #[arg(short, long, conflicts_with = "description_stdin")]
    description: Option<String>,
    /// Read description from stdin (for piping)
    #[arg(long, conflicts_with = "description")]
    description_stdin: bool,
    /// Interpret description as Markdown
    #[arg(long)]
    markdown: bool,
}
```

### Handler Logic (`src/cli/issue/create.rs`)

In `handle_edit`, after destructuring the new fields:

1. Resolve description text:
   - If `description_stdin` → read stdin into `String`
   - Else → use `description` as-is (`Option<String>`)
2. If description text is `Some`:
   - Convert to ADF via `adf::markdown_to_adf` (if `--markdown`) or `adf::text_to_adf`
   - Set `fields["description"] = adf_body`
   - Set `has_updates = true`

This slots in alongside the existing field updates, before the label-handling block. No changes to `edit_issue` API method or `JiraClient`.

### Error Message Update

Update the "no fields specified" bail message to include `--description` and `--description-stdin` in the list of available flags.

## Testing

### Unit Tests (inline in `create.rs`)

- `--description` and `--description-stdin` conflict at parse time (clap validation)
- `--markdown` without description produces no description field

### Integration Tests (`tests/`)

| Test | Input | Assertion |
|------|-------|-----------|
| Plain text description | `--description "hello"` | PUT body contains ADF paragraph with "hello" |
| Markdown description | `--description "**bold**" --markdown` | PUT body contains ADF with bold markup |
| Stdin description | `--description-stdin` with piped input | PUT body contains ADF from stdin text |
| Combined fields | `--description "text" --summary "new"` | PUT body contains both description ADF and summary |
| Markdown flag alone | `--markdown --summary "new"` | PUT body has summary but no description field |
| JSON output | `--description "text" --output json` | Outputs `{"key": "...", "updated": true}` |

Integration tests use wiremock to mock the Jira API and verify PUT request bodies. Tests use `body_json` matcher for exact matching or `received_requests()` for partial body inspection.

## Out of Scope

- **Append mode** — Would require GET (fetch current description) + ADF content array merge + PUT. Meaningful feature but adds complexity. Can be a follow-up.
- **Interactive editor** — Opening `$EDITOR` for description editing. Separate feature request.
- **Description clearing** (`--no-description`) — Not requested. Can be added later following the `--no-points` pattern.

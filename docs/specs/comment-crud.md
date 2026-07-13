# Comment CRUD — `jr issue comment` subcommand group

**Story:** S-577-1 (subcommand refactor) + S-577-3/4/5/6 (delete/edit/view implementations)
**Status:** S-577-1 merged (add subcommand); S-577-3/4/5/6 pending.

## Background

`jr issue comment` was originally a leaf command (flat form):

```
jr issue comment KEY "message text"
```

As of S-577-1 (`feat/comment-subcommand-refactor`, issue #577), it is a subcommand group
with four subcommands: `add`, `delete`, `edit`, `view`. The flat form now exits 2 with a
migration hint.

## Subcommands

### `jr issue comment add KEY [MESSAGE] [--stdin] [--file PATH] [--markdown] [--internal]`

Add a comment. Replaces the old flat form.

- `KEY` — Jira issue key (e.g. `FOO-123`).
- `MESSAGE` — positional; `allow_hyphen_values = true` (supports leading-dash GFM list items).
- `--stdin` — read body from stdin (conflicts with MESSAGE and `--file`).
- `--file PATH` — read body from file (conflicts with MESSAGE and `--stdin`).
- `--markdown` — interpret the body as Markdown and convert to ADF before posting.
- `--internal` — JSM agent-only visibility (`sd.public.comment.internal = true`).
- `--output json` — returns the raw Jira Comment object passthrough.

Exit codes: 0 success, 1 API error, 64 empty body.

### `jr issue comment delete KEY --id ID [--yes]` *(stub, S-577-3)*

Delete a comment by numeric ID. Requires `--yes` or interactive confirmation.

- `--id ID` — comment ID (numeric string from `jr issue comments --output json`).
- `--yes` — skip confirmation prompt (non-interactive usage).
- `--output json` — `{"deleted": true, "id": str}`.

### `jr issue comment edit KEY [TEXT] --id ID [--file PATH] [--stdin] [--markdown] [--internal|--public] [--yes]` *(stub, S-577-4/5)*

Edit a comment body and/or visibility.

- `TEXT` positional — new body; `allow_hyphen_values = true`. Mutually exclusive with `--file` and `--stdin`.
- `--id ID` — comment ID to edit.
- `--file PATH` — new body from file. Mutually exclusive with TEXT and `--stdin`.
- `--stdin` — new body from stdin. Mutually exclusive with TEXT and `--file`.
- `--markdown` — convert body to ADF.
- `--internal` — set JSM internal visibility. Mutually exclusive with `--public`.
- `--public` — set JSM public visibility. Mutually exclusive with `--internal`.
- `--yes` — skip confirmation when changing visibility from public to internal.
- `--output json` — `{"id": str, "updated": true}`.

### `jr issue comment view KEY --id ID` *(stub, S-577-6)*

View a single comment by ID.

- `--id ID` — comment ID.
- `--output json` — raw Jira Comment object passthrough.

## Migration hint

When `jr issue comment <TOKEN> …` is invoked with an unrecognized TOKEN (e.g., `FOO-1`,
`list`, `ls`), `main.rs::try_parse` intercepts the `InvalidSubcommand` error and emits:

```
error: use `jr issue comment add` instead
       `jr issue comment KEY "text"` is no longer valid; the comment command is now a subcommand group.
```

For `list`/`ls` tokens specifically:

```
error: to list all comments, use `jr issue comments` (plural)
```

The intercept uses `ContextKind::Usage` (not argv positional scanning) so it works correctly
when global flags precede the subcommand (e.g., `jr --output json issue comment KEY "text"`).
Detail: AC-013 / BC-3.5.012.

## Behavioral contracts

- BC-3.5.012 — `jr issue comment add` accepts leading-dash positional body.
- BC-3.5.009 — `comment edit` mutual-exclusion pairs (text/file, text/stdin, file/stdin, internal/public).

## JSON output shapes

See `docs/specs/json-output-shapes.md` for the canonical shapes.

## Implementation files

- `src/cli/mod.rs` — `IssueCommand::Comment { command: CommentSubcommand }`, `CommentSubcommand` enum.
- `src/cli/issue/interactions.rs` — `handle_comment_add` (implemented); stubs for delete/edit/view.
- `src/cli/issue/mod.rs` — dispatch to interactions handlers.
- `src/main.rs` — `try_parse` intercept for flat-form migration hint.

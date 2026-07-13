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

Exit codes: 0 success, 1 API error, 1 empty body (legacy `anyhow::bail!`, not `JrError::UserError`; the future edit subcommand exits 64 for empty body per BC-3.5.009 EC-3.5.009-5 — add's behavior is preserved as-is per EC-3.5.012-2).

### `jr issue comment delete KEY --id ID [--yes]` *(stub, S-577-3)*

Delete a comment by numeric ID. Requires `--yes` or interactive confirmation.

- `--id ID` — comment ID (numeric string from `jr issue comments --output json`).
- `--yes` — skip confirmation prompt (non-interactive usage).
- `--output json` — `{"deleted": true, "id": str, "key": str}`.

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
- `--output json` — `{"changed_fields": {...}, "id": str, "key": str, "updated": true}`.

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

## Body-only PUT preservation guarantee

When `jr issue comment edit` updates only the body (no `--internal` / `--public` flag
supplied), the existing comment visibility must be preserved — not reset. This means the
`PUT /rest/api/3/issue/{key}/comment/{id}` request body MUST NOT include the `visibility`
field when no visibility change was requested.

The Jira Cloud API treats a `PUT` that omits `visibility` as a MERGE/PRESERVED verdict:
the server keeps the prior visibility value. Including `visibility: null` or `visibility: {}`
would overwrite it to public, so omission is the only correct path.

Three behavioral contracts govern this:

- **BC-3.5.005** — body-only edit (no visibility flag): `PUT` omits `visibility`; Jira
  preserves the existing setting (MERGE verdict).
- **BC-3.5.006** — visibility-change edit (`--internal` or `--public`): `PUT` includes
  the appropriate `visibility` object; Jira overwrites the prior setting (OVERWRITE
  verdict).
- **BC-3.5.007** — deferred EJ probe: JSM (EJ project) comments have portal-vs-agent
  visibility semantics that may differ from the Jira Cloud standard comment API. A live
  round-trip probe against the EJ project is deferred to S-577-5; until then, the
  standard MERGE behaviour is assumed and documented as a known open question.

Acceptance criterion AC-009(i) in S-577-1 enumerates these verdicts in the story's
test-coverage table. The MERGE/PRESERVED path is covered by the body-only test variant;
the OVERWRITE path is covered by the visibility-change test variant.

## Behavioral contracts

- BC-3.5.005 — body-only `PUT` omits `visibility`; existing setting preserved (MERGE).
- BC-3.5.006 — visibility-change `PUT` includes `visibility`; prior setting overwritten.
- BC-3.5.007 — EJ/JSM visibility probe deferred to S-577-5.
- BC-3.5.009 — `comment edit` mutual-exclusion pairs (text/file, text/stdin, file/stdin, internal/public).
- BC-3.5.012 — `jr issue comment add` accepts leading-dash positional body.

## JSON output shapes

See `docs/specs/json-output-shapes.md` for the canonical shapes.

## Implementation files

- `src/cli/mod.rs` — `IssueCommand::Comment { command: CommentSubcommand }`, `CommentSubcommand` enum.
- `src/cli/issue/interactions.rs` — `handle_comment_add` (implemented); stubs for delete/edit/view.
- `src/cli/issue/mod.rs` — dispatch to interactions handlers.
- `src/main.rs` — `try_parse` intercept for flat-form migration hint.

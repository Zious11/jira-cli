# Comment CRUD — `jr issue comment` subcommand group

**Story:** S-577-1 (subcommand refactor) + S-577-2 (API methods) + S-577-3/4/5/6 (delete/edit/view CLI implementations)
**Status:** S-577-1 + S-577-2 + S-577-3 + S-577-4 + S-577-6 merged; S-577-5 (visibility flags) pending.

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

### `jr issue comment delete KEY --id ID [--yes]`

Delete a comment by numeric ID. Requires `--yes` or interactive confirmation.

- `--id ID` — comment ID (numeric string from `jr issue comments --output json`).
- `--yes` — skip confirmation prompt (non-interactive usage).
- `--output json` — `{"deleted": true, "id": str, "key": str}`.

### `jr issue comment edit KEY [TEXT] --id ID [--file PATH] [--stdin] [--markdown] [--internal|--public] [--yes]` *(body edit shipped, S-577-4; --internal/--public/--yes deferred to S-577-5)*

Edit a comment body and/or visibility.

- `TEXT` positional — new body; `allow_hyphen_values = true`. Mutually exclusive with `--file` and `--stdin`.
- `--id ID` — comment ID to edit.
- `--file PATH` — new body from file. Mutually exclusive with TEXT and `--stdin`.
- `--stdin` — new body from stdin. Mutually exclusive with TEXT and `--file`.
- `--markdown` — convert body to ADF.
- `--internal` — set JSM internal visibility. Mutually exclusive with `--public`.
- `--public` — set JSM public visibility. Mutually exclusive with `--internal`.
- `--yes` — skip the confirmation prompt when making a comment public (`--public`); no effect on other paths (EC-3.5.008-1/-4).
- `--output json` — `{"changed_fields": {...}, "id": str, "key": str, "updated": true}`.

### `jr issue comment view KEY --id ID`

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
supplied), the existing JSM comment visibility must be preserved — not reset. The
`PUT /rest/api/3/issue/{key}/comment/{id}` request body achieves this by omitting the
`"properties"` key entirely when no visibility change was requested.

The Jira Cloud API treats a `PUT` that omits `"properties"` as a MERGE/PRESERVED verdict
for each entity property: the server keeps all prior property values, including
`sd.public.comment`. The dangerous path is explicitly sending a `properties` array the
caller does not fully control. Body-only PUT is therefore the safe default (DEC-168
ruling 1; research Claim 1 REFUTED-footgun).

`jr` NEVER sends a `"visibility"` key on any `comment edit` path this cycle. The role/group
visibility restriction field (a separate Jira Cloud mechanism, distinct from the JSM
`sd.public.comment` property) has a PRESERVED verdict: a body-only PUT does not clear
an existing role/group restriction. `jr` exposes no surface for changing role/group
restrictions this cycle — the deferred EJ probe is a BC-3.5.006 delivery obligation.

Three behavioral contracts govern this:

- **BC-3.5.005** — body-only edit (no `--internal` / `--public` flag): `PUT` body
  key-set is exactly `{"body"}`; the `"properties"` key MUST NOT be present. Jira
  preserves the existing `sd.public.comment` property (MERGE verdict).
- **BC-3.5.006** — `--internal` flag: `PUT` body key-set is `{"body","properties"}`,
  where `properties` is `[{"key":"sd.public.comment","value":{"internal":true}}]`.
  The deferred EJ probe (verifying MERGE semantics for other properties and the
  PRESERVED verdict for the role/group `visibility` restriction) is a delivery
  obligation of this BC.
- **BC-3.5.007** — `--public` flag (always requires confirmation): `PUT` body
  key-set is `{"body","properties"}`, where `properties` is
  `[{"key":"sd.public.comment","value":{"internal":false}}]`.

These wire-shape verdicts are pinned by the API-method tests in `tests/comment_crud_api.rs`
(S-577-2): body-only → key-set `{"body"}`; `--internal`/`--public` → key-set
`{"body","properties"}`. CLI-level edit coverage lands with S-577-4/5.

## Behavioral contracts

- BC-3.5.005 — body-only `PUT` key-set exactly `{"body"}`; `"properties"` key absent; Jira preserves existing `sd.public.comment` (MERGE).
- BC-3.5.006 — `--internal` `PUT` key-set `{"body","properties"}`; `properties: [{key:"sd.public.comment",value:{internal:true}}]`; deferred EJ probe is a delivery obligation of this BC.
- BC-3.5.007 — `--public` `PUT` key-set `{"body","properties"}`; `properties: [{key:"sd.public.comment",value:{internal:false}}]`; always requires confirmation.
- BC-3.5.009 — `comment edit` mutual-exclusion pairs (text/file, text/stdin, file/stdin, internal/public).
- BC-3.5.012 — `jr issue comment add` accepts leading-dash positional body.

## JSON output shapes

See `docs/specs/json-output-shapes.md` for the canonical shapes.

## Implementation files

- `src/cli/mod.rs` — `IssueCommand::Comment { command: CommentSubcommand }`, `CommentSubcommand` enum.
- `src/cli/issue/interactions.rs` — all four handlers implemented: `handle_comment_add`, `handle_comment_delete` (S-577-3), `handle_comment_edit` (body-only, S-577-4), `handle_comment_view` (S-577-6).
- `src/cli/issue/mod.rs` — dispatch to interactions handlers.
- `src/main.rs` — `try_parse` intercept for flat-form migration hint.

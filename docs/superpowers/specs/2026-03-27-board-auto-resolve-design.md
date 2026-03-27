# Board Auto-Resolve — Design Spec

## Problem

`jr sprint list --project PROJ` fails with a generic error telling the user to run `jr board list` — which returns all boards across all projects with no filtering. An AI agent or human must:

1. Call `jr board list` to get all boards (unfiltered)
2. Guess which board corresponds to the target project
3. Filter to scrum boards (sprint commands reject kanban)
4. Retry with `--board <ID>`

This multi-step guessing process is fragile and error-prone. Additionally, the global `--project` flag is not threaded through to sprint or board handlers, so `jr board view --project FOO` ignores the flag entirely.

**Issue:** #70

## Solution

Three changes that work together:

1. **Auto-resolve**: When `--board` is not set and no `board_id` is configured, automatically discover the board via the Jira API using the project key. If exactly one board matches, use it. If zero or multiple, error with specific guidance.
2. **Board list filters**: Add `--type` flag to `board list` and thread the global `--project` through, so `jr board list --project PROJ --type scrum` filters server-side.
3. **Thread `--project` to sprint/board**: Pass the global `--project` override to sprint and board handlers (matching how issue, project, and queue already work).

### CLI Interface

**Board list gains `--type` filter:**

```
jr board list [--project PROJ] [--type scrum|kanban]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--project` | global flag | from config | Filter boards by project key (existing global flag, now threaded through) |
| `--type` | `Option<String>` | none | Filter boards by type, constrained to `scrum` or `kanban` via clap `value_parser` |

**No new flags on `sprint list/current` or `board view`.** They already accept `--board`. Auto-resolve uses the existing `--project` global flag + `.jr.toml` config fallback.

### API Layer: `list_boards()` Changes

Current signature:

```rust
pub async fn list_boards(&self) -> Result<Vec<Board>>
```

Changes to:

```rust
pub async fn list_boards(
    &self,
    project_key: Option<&str>,
    board_type: Option<&str>,
) -> Result<Vec<Board>>
```

When `project_key` is `Some`, appends `projectKeyOrId=PROJ` to the URL. When `board_type` is `Some`, appends `type=scrum` (or `kanban`). Same pagination loop, but filtered server-side — fewer pages fetched. Typically returns in a single API call when filtering by project (most projects have 1-3 boards).

The Jira Agile API returns 200 OK with an empty `values` array when no boards match — even if the project key doesn't exist. No 404.

### `Board` Struct Gains `location`

The API response includes a `location` object on every board (mandatory since 2018 in Jira Cloud). We need it for the auto-resolve stderr hint.

```rust
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Board {
    pub id: u64,
    pub name: String,
    #[serde(rename = "type")]
    pub board_type: String,
    #[serde(default)]
    pub location: Option<BoardLocation>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct BoardLocation {
    #[serde(default, rename = "projectKey")]
    pub project_key: Option<String>,
    #[serde(default, rename = "projectName")]
    pub project_name: Option<String>,
}
```

`location` is `Option` with `#[serde(default)]` as a defensive measure, even though it's always present in Jira Cloud. The inner fields use `Option` because cross-project boards may have varying field presence.

### Auto-Resolve Helper

New `pub(crate)` function in `src/cli/board.rs`:

```rust
pub(crate) async fn resolve_board_id(
    config: &Config,
    client: &JiraClient,
    board_override: Option<u64>,
    project_override: Option<&str>,
    require_scrum: bool,
) -> Result<u64>
```

**Resolution order:**

1. CLI `--board` flag → return immediately
2. Config `board_id` from `.jr.toml` → return immediately
3. Auto-discover via API:
   - Get project key from `config.project_key(project_override)` — if none available, return a `ConfigError` suggesting `--board`, `board_id` in config, or `--project`
   - Call `client.list_boards(Some(project_key), type_filter)` where `type_filter` is `Some("scrum")` when `require_scrum` is true, `None` otherwise
   - If exactly 1 board → print hint to stderr (`Using board 42 — My Board (scrum)`), return its ID
   - If 0 boards → return error with project key and suggestion to check board list
   - If 2+ boards → return error listing candidate boards with IDs and names

**Why in `board.rs`?** The function is board-resolution logic. `sprint.rs` imports it as `crate::cli::board::resolve_board_id`. Keeps the helper close to where boards are managed, consistent with `resolve_effective_limit` living in `cli/mod.rs` near the limit-related logic.

### Error Messages

| Scenario | Message | Exit |
|----------|---------|------|
| No `--board`, no config, no project key | `No board configured and no project specified. Use --board <ID>, set board_id in .jr.toml, or specify --project to auto-discover.` | 78 |
| 0 boards found (sprint) | `No scrum boards found for project PROJ. Verify the project key is correct, then try "jr board list --project PROJ".` | 1 |
| 0 boards found (board view) | `No boards found for project PROJ. Verify the project key is correct, then try "jr board list --project PROJ".` | 1 |
| 2+ boards found | `Multiple scrum boards found for project PROJ:\n  42  My Board\n  99  Other Board\nUse --board <ID> to select one, or set board_id in .jr.toml.` | 1 |
| Explicit `--board` is kanban (sprint) | Unchanged: `Sprint commands are only available for scrum boards. Board 42 is a kanban board.` | 1 |

### Stderr Hint on Auto-Select

When auto-resolve picks a board, print to stderr:

```
Using board 42 — My Board (scrum)
```

This uses stderr (not stdout) so it doesn't pollute `--output json` or piped output. Matches the existing pattern of `eprintln!` for hints (truncation hints, kanban project warnings).

### Call Site Updates

**`main.rs`:** Both sprint and board dispatch arms gain `cli.project.as_deref()`:

| File | Current | New |
|------|---------|-----|
| `main.rs` Board arm | `cli::board::handle(command, &config, &client, &cli.output)` | `cli::board::handle(command, &config, &client, &cli.output, cli.project.as_deref())` |
| `main.rs` Sprint arm | `cli::sprint::handle(command, &config, &client, &cli.output)` | `cli::sprint::handle(command, &config, &client, &cli.output, cli.project.as_deref())` |

**Handler signatures:** Both `board::handle` and `sprint::handle` gain `project_override: Option<&str>`.

**Board handler changes:**

| Function | Change |
|----------|--------|
| `handle` | Gains `project_override`, passes to `handle_view` and `handle_list` |
| `handle_list` | Gains `project_override` and `board_type_filter: Option<&str>`, passes both to `client.list_boards()` |
| `handle_view` | Replaces `config.board_id()` block with `resolve_board_id(config, client, board, project_override, false)` |

**Sprint handler changes:**

| Function | Change |
|----------|--------|
| `handle` | Gains `project_override`, replaces `config.board_id()` block with `resolve_board_id(config, client, board_override, project_override, true)` |

**Sprint scrum guard:** The existing scrum-type check (lines 31-39 in `sprint.rs`) remains. It fires when `--board` or config provides a board directly (resolution steps 1-2), where auto-resolve was skipped. When auto-resolve discovers the board (step 3 with `require_scrum=true`), the guard is redundant but harmless — it confirms the board is scrum via `get_board_config()`.

**Existing callers of `list_boards()`:** The only existing caller is `handle_list` in `board.rs`. Update to pass `(project_key, board_type_filter)` instead of `()`.

## What Changes

- `list_boards()` gains `project_key` and `board_type` parameters
- `Board` struct gains `location: Option<BoardLocation>` field
- New `BoardLocation` struct in `types/jira/board.rs`
- New `resolve_board_id()` helper in `cli/board.rs`
- `BoardCommand::List` gains `board_type` field with clap `value_parser`
- `main.rs` threads `cli.project` to board and sprint handlers
- Both `board::handle` and `sprint::handle` gain `project_override` parameter
- Error messages updated for missing board scenarios

## What Doesn't Change

- Board view output formatting (table/JSON)
- Sprint list/current behavior (when `--board` is explicit)
- Kanban JQL generation (`build_kanban_jql`)
- The scrum-type guard in `sprint.rs` (still needed for explicit `--board`)
- Exit codes for existing error scenarios
- Any command other than board and sprint

## Testing

### Unit Tests

- `BoardLocation` deserialization: verify serde parses the location object correctly
- `list_boards` URL construction: verify query params are appended when present, omitted when `None`

### Integration Tests

Using wiremock to mock Jira API responses:

1. **Auto-resolve success (sprint)**: Mock `list_boards?projectKeyOrId=PROJ&type=scrum` returning 1 board. Mock board config + sprint list. Run sprint handler. Assert sprint list endpoint called with correct board ID.
2. **Auto-resolve ambiguous**: Mock `list_boards` returning 2 scrum boards. Assert error contains both board IDs and names.
3. **Auto-resolve no boards**: Mock `list_boards` returning empty array. Assert error mentions project key and suggests checking board list.
4. **Board list with filters**: Mock `list_boards`. Call `list_boards(Some("PROJ"), Some("scrum"))`. Assert wiremock received `?projectKeyOrId=PROJ&type=scrum`.
5. **Explicit --board skips auto-resolve**: Call with `board_override=Some(42)`. Assert `list_boards` endpoint is NOT called.
6. **Global --project threads through**: Call board view handler with `project_override=Some("PROJ")`, no config board_id. Assert auto-resolve uses PROJ.

### Existing Tests

- `build_kanban_jql` tests: unchanged
- `compute_sprint_summary` tests: unchanged
- `board_commands.rs` integration tests (from PR #73): `list_boards()` call in `handle_list` gains `None, None` params — trivial update
- `missing_board_id_returns_config_error` unit test: replaced by `resolve_board_id` integration tests

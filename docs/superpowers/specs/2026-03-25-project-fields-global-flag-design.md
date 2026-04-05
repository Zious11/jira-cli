# Fix `project fields` to Use Global `--project` Flag — Design Spec

**Issue:** #56 — `project fields` rejects global `--project` flag, only accepts positional arg

**Goal:** Remove the positional `[PROJECT]` argument from `project fields` so it uses the global `--project` flag and `.jr.toml` default, consistent with every other subcommand.

## Problem

`jr project fields --project PROJ` fails with:
```
error: unexpected argument '--project' found
```

`ProjectCommand::Fields` defines a positional `project: Option<String>` argument that creates a naming collision with the global `--project` flag on `Cli`. Clap cannot resolve the global flag at the nested subcommand level when a positional argument with the same name exists. This is the only subcommand with this inconsistency — all others (`issue list`, `sprint list`, `board view`, etc.) use the global `--project` flag.

**Root cause:** The positional `project` field in the `Fields` variant shadows the global `--project` flag, preventing clap from recognizing it at the `project fields` subcommand level. Confirmed empirically: `jr issue list --project FOO` works (no naming conflict), `jr project fields --project FOO` fails (naming conflict). Also confirmed via clap issues [#2053](https://github.com/clap-rs/clap/issues/2053) and [#3428](https://github.com/clap-rs/clap/issues/3428).

## Design

### Clap definition (`src/cli/mod.rs`)

Remove the `project` field from the `Fields` variant, converting it from a struct variant to a unit variant:

```rust
// Before
Fields {
    /// Project key (uses configured project if omitted)
    project: Option<String>,
}

// After
/// Show valid issue types, priorities, and statuses
Fields,
```

Unit variants are already used throughout the codebase (`BoardCommand::List`, `BoardCommand::View`, `SprintCommand::List`, `SprintCommand::Current`, `AuthCommand::Status`, `IssueCommand::LinkTypes`).

### Handler (`src/cli/project.rs`)

Update the dispatch and handler to remove the `project` parameter:

**Dispatch** — `ProjectCommand::Fields { project }` becomes `ProjectCommand::Fields`:

```rust
ProjectCommand::Fields => {
    handle_fields(config, client, output_format, project_override).await
}
```

**Handler** — `handle_fields` drops the `project` parameter and resolves the project key solely from `config.project_key(project_override)`:

```rust
async fn handle_fields(
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    let project_key = config.project_key(project_override).ok_or_else(|| {
        anyhow::anyhow!(
            "No project specified. Run \"jr project list\" to see available projects."
        )
    })?;
    // ... rest unchanged
}
```

`config.project_key(project_override)` resolves in this order:
1. `--project` CLI flag (passed as `project_override` from `cli.project` in `main.rs`)
2. `.jr.toml` project config (per-project default)

This is the same resolution used by `issue list`, `sprint list`, `board view`, and every other project-scoped command.

### Help text

After the fix, `jr project fields --help` will show the global `--project` flag in its options list (currently hidden by the naming conflict):

```
Show valid issue types, priorities, and statuses

Usage: jr project fields [OPTIONS]

Options:
      --project <PROJECT>    Override project key
      --output <OUTPUT>      Output format [default: table] [possible values: table, json]
      ...
```

This matches the help output of all other subcommands.

### Shell completions

Shell completions are generated on demand via `jr completion <shell>`. They will automatically reflect the removal of the positional argument — no manual update needed.

## Error handling

No change. When no project is specified via `--project`, `.jr.toml`, or global config:
```
Error: No project specified. Run "jr project list" to see available projects.
```

## Backward compatibility

`jr project fields PROJ` (the positional form) will no longer work. This is acceptable per project owner decision — the positional was the source of the bug and was inconsistent with the rest of the CLI.

The following files reference the old positional form and should be updated to use `--project`:
- `README.md` (line ~123: `jr project fields FOO`)
- `docs/superpowers/specs/2026-03-21-jr-jira-cli-design.md` (v1 spec examples)
- `docs/superpowers/plans/2026-03-21-jr-implementation.md` (error message text)

## Testing

- Existing integration tests for `get_project_issue_types` and `get_priorities` continue to verify API layer correctness
- The clap parsing fix is verified by building and running:
  - `jr project fields --project PROJ` (must succeed)
  - `jr project fields` with `.jr.toml` default (must succeed)
  - `jr project fields` with no project configured (must show error)

## Files changed

| File | Change |
|------|--------|
| `src/cli/mod.rs` | Remove `project` field from `Fields` variant (struct → unit) |
| `src/cli/project.rs` | Remove `project` param from dispatch and handler |
| `README.md` | Update `project fields` example to use `--project` |

## Non-goals

- Adding integration tests for clap parsing behavior (not done for any other subcommand)
- Changing how other subcommands resolve the project key
- Adding a local `--project` flag to `Fields` (unnecessary — the global flag covers this use case. While `issue create` has a local `--project` for interactive prompt fallback, `Fields` has no such need.)

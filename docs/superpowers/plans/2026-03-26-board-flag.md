# Add `--board` Flag — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `--board <ID>` CLI flag to `sprint list`, `sprint current`, and `board view` so users can specify a board without editing `.jr.toml`.

**Architecture:** Convert three unit variants in `SprintCommand` and `BoardCommand` to struct variants with `board: Option<u64>`. Add `Config::board_id()` resolver that prioritizes CLI override over config fallback. Update handlers and error messages.

**Tech Stack:** Rust, clap 4 (derive API)

**Spec:** `docs/superpowers/specs/2026-03-26-board-flag-design.md`

---

### Task 1: Add `Config::board_id()` resolver with tests

**Files:**
- Modify: `src/config.rs:109-113` (add method after `project_key`)
- Test: `src/config.rs` (inline `#[cfg(test)]` module, after `test_project_key_cli_override`)

- [ ] **Step 1: Write the failing test**

In `src/config.rs`, add this test after `test_project_key_cli_override` (after line 236):

```rust
#[test]
fn test_board_id_cli_override() {
    let config = Config {
        global: GlobalConfig::default(),
        project: ProjectConfig {
            project: None,
            board_id: Some(42),
        },
    };
    // CLI override wins
    assert_eq!(config.board_id(Some(99)), Some(99));
    // Config fallback
    assert_eq!(config.board_id(None), Some(42));
    // Neither set
    let empty = Config::default();
    assert_eq!(empty.board_id(None), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_board_id_cli_override`
Expected: FAIL with "no method named `board_id` found"

- [ ] **Step 3: Implement `board_id()` method**

In `src/config.rs`, add this method after `project_key` (after line 113):

```rust
pub fn board_id(&self, cli_override: Option<u64>) -> Option<u64> {
    cli_override.or(self.project.board_id)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_board_id_cli_override`
Expected: PASS

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/config.rs
git commit -m "feat: add Config::board_id() resolver for CLI override (#57)"
```

---

### Task 2: Convert enum variants and update handlers

**Files:**
- Modify: `src/cli/mod.rs:363-377` (BoardCommand and SprintCommand enums)
- Modify: `src/cli/sprint.rs:10-34` (handle function)
- Modify: `src/cli/board.rs:9-19,34-41` (handle and handle_view functions)

- [ ] **Step 1: Convert `BoardCommand::View` to struct variant**

In `src/cli/mod.rs`, replace lines 363-369:

```rust
#[derive(Subcommand)]
pub enum BoardCommand {
    /// List boards
    List,
    /// View current board issues
    View,
}
```

With:

```rust
#[derive(Subcommand)]
pub enum BoardCommand {
    /// List boards
    List,
    /// View current board issues
    View {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
    },
}
```

- [ ] **Step 2: Convert `SprintCommand` variants to struct variants**

In `src/cli/mod.rs`, replace lines 371-377:

```rust
#[derive(Subcommand)]
pub enum SprintCommand {
    /// List sprints
    List,
    /// Show current sprint issues
    Current,
}
```

With:

```rust
#[derive(Subcommand)]
pub enum SprintCommand {
    /// List sprints
    List {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
    },
    /// Show current sprint issues
    Current {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
    },
}
```

- [ ] **Step 3: Update `board.rs` handler dispatch**

In `src/cli/board.rs`, replace lines 15-18:

```rust
    match command {
        BoardCommand::List => handle_list(client, output_format).await,
        BoardCommand::View => handle_view(config, client, output_format).await,
    }
```

With:

```rust
    match command {
        BoardCommand::List => handle_list(client, output_format).await,
        BoardCommand::View { board } => {
            handle_view(config, client, output_format, board).await
        }
    }
```

- [ ] **Step 4: Update `handle_view` signature and board_id resolution**

In `src/cli/board.rs`, replace lines 34-41:

```rust
async fn handle_view(
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    let board_id = config.project.board_id.ok_or_else(|| {
        anyhow::anyhow!("No board_id configured. Set board_id in .jr.toml or run \"jr init\".")
    })?;
```

With:

```rust
async fn handle_view(
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    board_override: Option<u64>,
) -> Result<()> {
    let board_id = config.board_id(board_override).ok_or_else(|| {
        anyhow::anyhow!(
            "No board configured. Use --board <ID> or set board_id in .jr.toml.\n\
             Run \"jr board list\" to see available boards."
        )
    })?;
```

The rest of `handle_view` (lines 43-82) remains unchanged.

- [ ] **Step 5: Update `sprint.rs` handler — extract board from variants and update error message**

In `src/cli/sprint.rs`, replace lines 10-35:

```rust
/// Handle all sprint subcommands.
pub async fn handle(
    command: SprintCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    let board_id = config.project.board_id.ok_or_else(|| {
        anyhow::anyhow!("No board_id configured. Set board_id in .jr.toml or run \"jr init\".")
    })?;

    // Guard: sprints only make sense for scrum boards
    let board_config = client.get_board_config(board_id).await?;
    let board_type = board_config.board_type.to_lowercase();
    if board_type != "scrum" {
        bail!(
            "Sprint commands are only available for scrum boards. Board {} is a {} board.",
            board_id,
            board_config.board_type
        );
    }

    match command {
        SprintCommand::List => handle_list(board_id, client, output_format).await,
        SprintCommand::Current => handle_current(board_id, client, output_format, config).await,
    }
}
```

With:

```rust
/// Handle all sprint subcommands.
pub async fn handle(
    command: SprintCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    let board_override = match &command {
        SprintCommand::List { board } => *board,
        SprintCommand::Current { board } => *board,
    };

    let board_id = config.board_id(board_override).ok_or_else(|| {
        anyhow::anyhow!(
            "No board configured. Use --board <ID> or set board_id in .jr.toml.\n\
             Run \"jr board list\" to see available boards."
        )
    })?;

    // Guard: sprints only make sense for scrum boards
    let board_config = client.get_board_config(board_id).await?;
    let board_type = board_config.board_type.to_lowercase();
    if board_type != "scrum" {
        bail!(
            "Sprint commands are only available for scrum boards. Board {} is a {} board.",
            board_id,
            board_config.board_type
        );
    }

    match command {
        SprintCommand::List { .. } => handle_list(board_id, client, output_format).await,
        SprintCommand::Current { .. } => {
            handle_current(board_id, client, output_format, config).await
        }
    }
}
```

Note: This extracts `board` from each variant at the top using separate match arms, then uses `{ .. }` in the dispatch match to ignore the already-consumed field. This avoids duplicating the board_id resolution in each dispatch arm.

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All tests PASS (existing `compute_sprint_summary` tests are unaffected).

- [ ] **Step 7: Run clippy and format**

Run: `cargo fmt --all && cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, no format issues.

- [ ] **Step 8: Commit**

```bash
git add src/cli/mod.rs src/cli/sprint.rs src/cli/board.rs
git commit -m "feat: add --board flag to sprint and board commands (#57)"
```

---

### Task 3: Update documentation

**Files:**
- Modify: `README.md` (lines 110-113)

- [ ] **Step 1: Update README command table**

In `README.md`, replace lines 110-113:

```markdown
| `jr board list` | List boards |
| `jr board view` | Show current board issues |
| `jr sprint list` | List sprints (scrum only) |
| `jr sprint current` | Show current sprint issues (with points summary) |
```

With:

```markdown
| `jr board list` | List boards |
| `jr board view --board 42` | Show current board issues (`--board` or config) |
| `jr sprint list --board 42` | List sprints (`--board` or config, scrum only) |
| `jr sprint current --board 42` | Show current sprint issues (with points summary) |
```

- [ ] **Step 2: Run clippy and format**

Run: `cargo fmt --all && cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: update README with --board flag examples (#57)"
```

---

### Task 4: Final verification

**Files:**
- All modified files from Tasks 1-3

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Zero warnings.

- [ ] **Step 3: Run formatter**

Run: `cargo fmt --all && cargo fmt --all -- --check`
Expected: No format issues.

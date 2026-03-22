# Issue Handler Parameter Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor all 9 issue handlers to receive the owned `IssueCommand` enum instead of destructured fields, eliminating all 4 `#[allow(clippy::too_many_arguments)]` annotations.

**Architecture:** Pass the whole `IssueCommand` enum variant (owned, moved) into each handler function. Each handler destructures with `let IssueCommand::Variant { .. } = command else { unreachable!() }` at the top. The `handle()` dispatch uses `{ .. }` patterns to check variants without binding, allowing the move.

**Tech Stack:** Rust, clap (CLI enum definitions)

**Spec:** `docs/superpowers/specs/2026-03-22-issue-handler-refactor-design.md`

---

### Task 1: Refactor `handle()` dispatch and `handle_list`

**Files:**
- Modify: `src/cli/issue.rs:49-178` (handle dispatch + handle_list signature)

- [ ] **Step 1: Rewrite the `handle()` match block**

Replace the entire match block (lines 58-161) with the new dispatch pattern. The `handle()` signature stays the same. Each arm uses `{ .. }` to check the variant, then passes the owned `command` to the handler:

```rust
    match command {
        IssueCommand::List { .. } => {
            handle_list(command, output_format, config, client, project_override, no_input).await
        }
        IssueCommand::View { .. } => {
            handle_view(command, output_format, config, client).await
        }
        IssueCommand::Create { .. } => {
            handle_create(command, output_format, config, client, project_override, no_input)
                .await
        }
        IssueCommand::Edit { .. } => {
            handle_edit(command, output_format, config, client, no_input).await
        }
        IssueCommand::Move { .. } => {
            handle_move(command, output_format, client, no_input).await
        }
        IssueCommand::Transitions { .. } => {
            handle_transitions(command, output_format, client).await
        }
        IssueCommand::Assign { .. } => {
            handle_assign(command, output_format, client).await
        }
        IssueCommand::Comment { .. } => {
            handle_comment(command, output_format, client).await
        }
        IssueCommand::Open { .. } => handle_open(command, client).await,
    }
```

- [ ] **Step 2: Refactor `handle_list` signature and destructure inside**

Remove the `#[allow(clippy::too_many_arguments)]` annotation. Replace the signature and add destructuring at the top:

```rust
async fn handle_list(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::List {
        jql,
        status,
        team,
        limit,
        points: show_points,
    } = command
    else {
        unreachable!()
    };
```

Note: `points` is renamed to `show_points` in the destructure to match the variable name used in the function body.

The rest of the function body is unchanged.

- [ ] **Step 3: Verify it compiles and tests pass**

Run: `cargo test`
Expected: All tests pass

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings (one `#[allow]` removed)

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue.rs
git commit -m "refactor: pass IssueCommand to handle_list instead of individual params"
```

---

### Task 2: Refactor `handle_view` and `handle_create`

**Files:**
- Modify: `src/cli/issue.rs` (handle_view ~line 342, handle_create ~line 445)

- [ ] **Step 1: Refactor `handle_view`**

Replace signature and add destructuring:

```rust
async fn handle_view(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::View { key } = command else {
        unreachable!()
    };
```

The function body currently uses `key` as `&str` (it was passed as `key: &str`). Now `key` is an owned `String`. Update any usage of `key` in the body: replace `key` with `&key` where a `&str` is needed (e.g., `client.get_issue(&key, &extra)`). Check all occurrences.

- [ ] **Step 2: Refactor `handle_create`**

Remove the `#[allow(clippy::too_many_arguments)]` annotation. Replace signature and add destructuring:

```rust
async fn handle_create(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Create {
        project,
        issue_type,
        summary,
        description,
        description_stdin,
        priority,
        label: labels,
        team,
        points,
        markdown,
    } = command
    else {
        unreachable!()
    };
```

Note: `label` is renamed to `labels` in destructure to match variable name used in the body.

The rest of the function body is unchanged — it already uses owned values.

- [ ] **Step 3: Verify**

Run: `cargo test`
Expected: All tests pass

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings (two `#[allow]` removed total)

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue.rs
git commit -m "refactor: pass IssueCommand to handle_view and handle_create"
```

---

### Task 3: Refactor `handle_edit` and `handle_move`

**Files:**
- Modify: `src/cli/issue.rs` (handle_edit ~line 557, handle_move ~line 673)

- [ ] **Step 1: Refactor `handle_edit`**

Remove the `#[allow(clippy::too_many_arguments)]` annotation. Replace signature and add destructuring:

```rust
async fn handle_edit(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Edit {
        key,
        summary,
        issue_type,
        priority,
        label: labels,
        team,
        points,
        no_points,
    } = command
    else {
        unreachable!()
    };
```

Note: `label` renamed to `labels`. The body currently uses `key` as `&str` — now it's owned `String`. Update `key` references to `&key` where `&str` is needed.

- [ ] **Step 2: Refactor `handle_move`**

Replace signature and add destructuring:

```rust
async fn handle_move(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Move { key, status } = command else {
        unreachable!()
    };
```

The body currently uses `key` as `&str` — update references to `&key`.

- [ ] **Step 3: Verify**

Run: `cargo test`
Expected: All tests pass

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings (three `#[allow]` removed total)

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue.rs
git commit -m "refactor: pass IssueCommand to handle_edit and handle_move"
```

---

### Task 4: Refactor `handle_transitions`, `handle_assign`, `handle_comment`, `handle_open`

**Files:**
- Modify: `src/cli/issue.rs` (4 remaining handlers)

- [ ] **Step 1: Refactor `handle_transitions`**

```rust
async fn handle_transitions(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Transitions { key } = command else {
        unreachable!()
    };
```

Update `key` references to `&key` where `&str` is needed.

- [ ] **Step 2: Refactor `handle_assign`**

```rust
async fn handle_assign(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Assign { key, to, unassign } = command else {
        unreachable!()
    };
```

Update `key` references to `&key`.

- [ ] **Step 3: Refactor `handle_comment`**

Remove the `#[allow(clippy::too_many_arguments)]` annotation. Replace:

```rust
async fn handle_comment(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Comment {
        key,
        message,
        markdown,
        file,
        stdin,
    } = command
    else {
        unreachable!()
    };
```

Update `key` references to `&key`.

- [ ] **Step 4: Refactor `handle_open`**

```rust
async fn handle_open(command: IssueCommand, client: &JiraClient) -> Result<()> {
    let IssueCommand::Open { key, url_only } = command else {
        unreachable!()
    };
```

Update `key` references to `&key`.

- [ ] **Step 5: Verify**

Run: `cargo test`
Expected: All tests pass

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings (all 4 `#[allow]` removed)

Run: `cargo fmt --all -- --check`
Expected: No formatting issues

- [ ] **Step 6: Commit**

```bash
git add src/cli/issue.rs
git commit -m "refactor: pass IssueCommand to remaining handlers (transitions, assign, comment, open)"
```

---

### Task 5: Final verification

- [ ] **Step 1: Verify no `#[allow(clippy::too_many_arguments)]` remain**

Run: `grep -n "too_many_arguments" src/cli/issue.rs`
Expected: No matches

- [ ] **Step 2: Run full CI equivalent**

```bash
cargo fmt --all -- --check
cargo clippy --all --all-features --tests -- -D warnings
cargo test --all-features
```

Expected: All pass

- [ ] **Step 3: Verify no behavior changes**

Run: `cargo test --test issue_commands`
Run: `cargo test --test team_commands`
Expected: All pass — integration tests verify behavior is unchanged

- [ ] **Step 4: Build release binary and verify clap dispatch**

```bash
cargo build --release
./target/release/jr --help
./target/release/jr issue --help
./target/release/jr issue list --help
./target/release/jr issue create --help
./target/release/jr issue edit --help
```

Expected: All help outputs render correctly with no panics. This proves the enum dispatch wiring is correct.

- [ ] **Step 5: Live smoke test against Jira instance**

```bash
./target/release/jr issue list --limit 2
./target/release/jr issue list --points --limit 2
./target/release/jr issue view <any-issue-key>
./target/release/jr sprint current
```

Expected: All commands return data without errors. This proves the handler functions receive and destructure the enum correctly at runtime.

- [ ] **Step 6: Commit formatting if needed**

```bash
cargo fmt --all
git add src/cli/issue.rs
git commit -m "chore: format"
```

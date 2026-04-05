# Fix `project fields` Global `--project` Flag — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix bug #56 — make `project fields` use the global `--project` flag instead of a positional argument, consistent with all other subcommands.

**Architecture:** Remove the positional `project` field from `ProjectCommand::Fields` (clap derive enum), update the handler to resolve the project key solely from `config.project_key(project_override)`, and update the README example.

**Tech Stack:** Rust, clap 4 (derive API)

**Spec:** `docs/superpowers/specs/2026-03-25-project-fields-global-flag-design.md`

---

### Task 1: Remove positional arg and update handler

**Files:**
- Modify: `src/cli/mod.rs` (the `ProjectCommand` enum, around line 356-361)
- Modify: `src/cli/project.rs` (the `handle` dispatch and `handle_fields` function)

- [ ] **Step 1: Change `Fields` from struct variant to unit variant**

In `src/cli/mod.rs`, replace the `Fields` variant in the `ProjectCommand` enum:

```rust
// Before (lines 356-360):
    /// Show valid issue types, priorities, and statuses
    Fields {
        /// Project key (uses configured project if omitted)
        project: Option<String>,
    },

// After:
    /// Show valid issue types, priorities, and statuses
    Fields,
```

- [ ] **Step 2: Update the dispatch in `handle`**

In `src/cli/project.rs`, replace the `Fields` match arm in the `handle` function:

```rust
// Before (lines 21-23):
        ProjectCommand::Fields { project } => {
            handle_fields(project, config, client, output_format, project_override).await
        }

// After:
        ProjectCommand::Fields => {
            handle_fields(config, client, output_format, project_override).await
        }
```

- [ ] **Step 3: Update `handle_fields` signature and project resolution**

In `src/cli/project.rs`, replace the `handle_fields` function signature and project resolution:

```rust
// Before (lines 60-73):
async fn handle_fields(
    project: Option<String>,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    let project_key = project
        .or_else(|| config.project_key(project_override))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No project specified. Run \"jr project list\" to see available projects."
            )
        })?;

// After:
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
```

The rest of `handle_fields` (lines 75-107) remains unchanged.

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests PASS (no regressions — no existing tests depend on the positional arg).

- [ ] **Step 5: Run clippy and format**

Run: `cargo fmt --all && cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, no format issues.

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs src/cli/project.rs
git commit -m "fix: use global --project flag for project fields (#56)"
```

---

### Task 2: Update documentation

**Files:**
- Modify: `README.md` (line ~123)
- Modify: `docs/superpowers/specs/2026-03-21-jr-jira-cli-design.md` (lines 37 and 409)
- Modify: `docs/superpowers/plans/2026-03-21-jr-implementation.md` (line 3870)

- [ ] **Step 1: Update the `project fields` example in README**

In `README.md`, find line ~123:

```markdown
| `jr project fields FOO` | Show valid issue types and priorities |
```

Replace with:

```markdown
| `jr project fields --project FOO` | Show valid issue types, priorities, and statuses |
```

Note: The description is also updated to include "and statuses" since PR #61 added statuses support.

- [ ] **Step 2: Update v1 design spec examples**

In `docs/superpowers/specs/2026-03-21-jr-jira-cli-design.md`, update two references:

Line 37 — replace:
```
jr project fields FOO                # List valid issue types, priorities, statuses for a project
```
with:
```
jr project fields --project FOO      # List valid issue types, priorities, statuses for a project
```

Line 409 — replace:
```
`jr project fields FOO --output json` returns valid issue types, priorities, and statuses for a project.
```
with:
```
`jr project fields --project FOO --output json` returns valid issue types, priorities, and statuses for a project.
```

- [ ] **Step 3: Update v1 implementation plan error message**

In `docs/superpowers/plans/2026-03-21-jr-implementation.md`, line 3870 — replace:
```
.ok_or_else(|| anyhow::anyhow!("No project specified. Use 'jr project fields FOO' or configure .jr.toml"))?;
```
with:
```
.ok_or_else(|| anyhow::anyhow!("No project specified. Use 'jr project fields --project FOO' or configure .jr.toml"))?;
```

- [ ] **Step 4: Commit**

```bash
git add README.md docs/superpowers/specs/2026-03-21-jr-jira-cli-design.md docs/superpowers/plans/2026-03-21-jr-implementation.md
git commit -m "docs: update project fields examples to use --project flag (#56)"
```

---

### Task 3: Final verification

**Files:**
- All modified files from Tasks 1-2

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Zero warnings.

- [ ] **Step 3: Run formatter**

Run: `cargo fmt --all && cargo fmt --all -- --check`
Expected: No format issues.

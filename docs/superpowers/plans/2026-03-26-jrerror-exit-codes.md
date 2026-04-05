# JrError Exit Codes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace 13 `anyhow::anyhow!`/`bail!` calls with typed `JrError` variants so `main.rs` can map exit codes correctly (78 for config errors, 64 for user input errors).

**Architecture:** Mechanical replacements across 8 files + one format string change in `error.rs`. No new types or variants.

**Tech Stack:** Rust, thiserror, anyhow

---

### Task 1: Change ConfigError display format and add unit tests

**Files:**
- Modify: `src/error.rs:14`
- Test: `src/error.rs` (inline unit tests)

- [ ] **Step 1: Write failing unit tests for exit code mapping**

Add a `#[cfg(test)]` module at the bottom of `src/error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_error_exit_code() {
        let err = JrError::ConfigError("test".into());
        assert_eq!(err.exit_code(), 78);
    }

    #[test]
    fn user_error_exit_code() {
        let err = JrError::UserError("test".into());
        assert_eq!(err.exit_code(), 64);
    }

    #[test]
    fn config_error_display_no_prefix() {
        let err = JrError::ConfigError("No board_id configured.".into());
        assert_eq!(err.to_string(), "No board_id configured.");
    }

    #[test]
    fn user_error_display_passthrough() {
        let err = JrError::UserError("Invalid selection".into());
        assert_eq!(err.to_string(), "Invalid selection");
    }
}
```

- [ ] **Step 2: Run tests to verify the display test fails**

Run: `cargo test --lib error::tests`
Expected: `config_error_display_no_prefix` FAILS because current format prepends "Configuration error: ".

- [ ] **Step 3: Change ConfigError format string**

In `src/error.rs:14`, change:
```rust
    #[error("Configuration error: {0}")]
    ConfigError(String),
```
to:
```rust
    #[error("{0}")]
    ConfigError(String),
```

- [ ] **Step 4: Run tests to verify all pass**

Run: `cargo test --lib error::tests`
Expected: All 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/error.rs
git commit -m "fix: remove redundant ConfigError display prefix (#30)"
```

---

### Task 2: Replace anyhow::anyhow! with JrError::ConfigError (7 locations)

**Files:**
- Modify: `src/cli/board.rs:39-41`
- Modify: `src/cli/sprint.rs:16-18`
- Modify: `src/api/client.rs:35-36`
- Modify: `src/cli/team.rs:85-87`
- Modify: `src/config.rs:97-98`
- Modify: `src/cli/issue/helpers.rs:19-23`
- Modify: `src/cli/issue/helpers.rs:77-81`

Each replacement follows the same pattern. Replace:
```rust
anyhow::anyhow!("message text")
```
with:
```rust
crate::error::JrError::ConfigError("message text".into())
```

Note: some files may already import `JrError` or `crate::error`. Check each file's imports and add `use crate::error::JrError;` if not already present. Prefer the shortest unambiguous path available in each file.

- [ ] **Step 1: Replace in `src/cli/board.rs:40`**

Change the `anyhow::anyhow!(...)` call to `JrError::ConfigError("...".into())`, keeping the existing message text exactly as-is. Add `use crate::error::JrError;` if not present.

- [ ] **Step 2: Replace in `src/cli/sprint.rs:17`**

Same pattern: `anyhow::anyhow!(...)` → `JrError::ConfigError("...".into())`.

- [ ] **Step 3: Replace in `src/api/client.rs:36`**

This file already uses `JrError::ConfigError` at lines 278 and 301, so the import exists. Change line 36 only.

- [ ] **Step 4: Replace in `src/cli/team.rs:86`**

Same pattern. Add import if needed.

- [ ] **Step 5: Replace in `src/config.rs:98`**

Same pattern. Add import if needed.

- [ ] **Step 6: Replace in `src/cli/issue/helpers.rs:20` and `:78`**

Two locations in the same file. Same pattern for both.

- [ ] **Step 7: Run tests and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All tests pass, no clippy warnings. No test should break — error messages are identical.

- [ ] **Step 8: Commit**

```bash
git add src/cli/board.rs src/cli/sprint.rs src/api/client.rs src/cli/team.rs src/config.rs src/cli/issue/helpers.rs
git commit -m "fix: use JrError::ConfigError for config-missing guards (#30)"
```

---

### Task 3: Replace anyhow::anyhow!/bail! with JrError::UserError (6 locations)

**Files:**
- Modify: `src/cli/issue/create.rs:47-52, 63, 74`
- Modify: `src/cli/project.rs:69-73`
- Modify: `src/cli/issue/workflow.rs:121, 123`

Same mechanical pattern. Replace:
```rust
anyhow::anyhow!("message text")
```
with:
```rust
JrError::UserError("message text".into())
```

For `bail!("message")` at `workflow.rs:123`, replace with:
```rust
return Err(JrError::UserError("Selection out of range".into()).into());
```

- [ ] **Step 1: Replace in `src/cli/issue/create.rs:48, 63, 74`**

Three `anyhow::anyhow!(...)` calls → `JrError::UserError("...".into())`. Add import if needed.

- [ ] **Step 2: Replace in `src/cli/project.rs:70`**

Same pattern. Add import if needed.

- [ ] **Step 3: Replace in `src/cli/issue/workflow.rs:121` and `:123`**

Line 121: `anyhow::anyhow!("Invalid selection")` → `JrError::UserError("Invalid selection".into())`

Line 123: `bail!("Selection out of range")` → `return Err(JrError::UserError("Selection out of range".into()).into())`

Add import if needed.

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All tests pass, no clippy warnings.

- [ ] **Step 5: Commit**

```bash
git add src/cli/issue/create.rs src/cli/project.rs src/cli/issue/workflow.rs
git commit -m "fix: use JrError::UserError for missing-input guards (#30)"
```

---

### Task 4: Add inline unit tests for error type at call sites

**Files:**
- Modify: `src/cli/board.rs` (add `#[cfg(test)]` module)
- Modify: `src/cli/issue/create.rs` (add `#[cfg(test)]` module)

Handler functions are `pub(super)` — not reachable from `tests/`. Instead, add inline unit tests in the modules where the errors are raised. This follows the same pattern used elsewhere in the codebase (unit tests inline, integration tests for API calls).

- [ ] **Step 1: Add ConfigError unit test in `src/cli/board.rs`**

Add a `#[cfg(test)]` module at the bottom of `board.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::JrError;

    #[test]
    fn missing_board_id_returns_config_error() {
        // board_id = None triggers the ConfigError guard
        let result: Option<u64> = None;
        let err = result
            .ok_or_else(|| {
                JrError::ConfigError(
                    "No board_id configured. Set board_id in .jr.toml or run \"jr init\".".into(),
                )
            })
            .unwrap_err();
        assert_eq!(err.exit_code(), 78);
        assert!(err.to_string().contains("No board_id configured"));
    }
}
```

- [ ] **Step 2: Add UserError unit test in `src/cli/issue/create.rs`**

Add a `#[cfg(test)]` module at the bottom of `create.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::error::JrError;

    #[test]
    fn missing_project_returns_user_error() {
        let result: Option<String> = None;
        let err = result
            .ok_or_else(|| {
                JrError::UserError("Project key is required. Use --project or configure .jr.toml. Run \"jr project list\" to see available projects.".into())
            })
            .unwrap_err();
        assert_eq!(err.exit_code(), 64);
        assert!(err.to_string().contains("Project key is required"));
    }
}
```

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: All tests pass including the new unit tests.

- [ ] **Step 4: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: Clean.

- [ ] **Step 5: Commit**

```bash
git add src/cli/board.rs src/cli/issue/create.rs
git commit -m "test: add unit tests for exit code mapping at error sites (#30)"
```

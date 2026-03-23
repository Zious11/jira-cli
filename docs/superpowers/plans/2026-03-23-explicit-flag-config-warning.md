# Explicit Flag Config Warning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Warn on stderr when `--points` flag is used but `story_points_field_id` is not configured, instead of silently skipping (closes GitHub issue #18).

**Architecture:** Extract a helper function `resolve_show_points` that encapsulates the flag+config resolution and emits the warning to stderr. Replace the inline ternary in `handle_list`. Add 3 unit tests covering all branches.

**Tech Stack:** Rust, anyhow

**Spec:** `docs/superpowers/specs/2026-03-23-explicit-flag-config-warning-design.md`

---

## File Structure

| File | Responsibility | Change |
|------|---------------|--------|
| `src/cli/issue/list.rs` | List/view handlers, JQL construction, unit tests | Add `resolve_show_points` function, update `handle_list`, add 3 tests |

---

### Task 1: Add `resolve_show_points` with tests and update `handle_list`

**Files:**
- Modify: `src/cli/issue/list.rs:114` (replace inline ternary)
- Modify: `src/cli/issue/list.rs:127-128` (add new function before `build_fallback_jql`)
- Modify: `src/cli/issue/list.rs:322-386` (add tests to existing test module)

- [ ] **Step 1: Write three failing tests**

Add these tests to the existing `#[cfg(test)] mod tests` block in `src/cli/issue/list.rs`, after the last existing test (`fallback_jql_with_status_only` at line 382-384):

```rust
    #[test]
    fn resolve_show_points_flag_false() {
        assert_eq!(resolve_show_points(false, Some("customfield_10031")), None);
        assert_eq!(resolve_show_points(false, None), None);
    }

    #[test]
    fn resolve_show_points_flag_true_config_present() {
        assert_eq!(
            resolve_show_points(true, Some("customfield_10031")),
            Some("customfield_10031")
        );
    }

    #[test]
    fn resolve_show_points_flag_true_config_missing() {
        // Warning emitted to stderr (not captured), but function returns None without error
        assert_eq!(resolve_show_points(true, None), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --lib resolve_show_points
```
Expected: FAIL — `resolve_show_points` is not defined.

- [ ] **Step 3: Add `resolve_show_points` function**

In `src/cli/issue/list.rs`, add this function between the end of `handle_list` (line 127 `}`) and `fn build_fallback_jql` (line 129):

```rust
/// Resolve whether to show story points. Returns the field ID if points should
/// be shown, or None. Emits a warning to stderr if --points was requested but
/// config is missing.
fn resolve_show_points<'a>(show_points: bool, sp_field_id: Option<&'a str>) -> Option<&'a str> {
    if show_points {
        match sp_field_id {
            Some(id) => Some(id),
            None => {
                eprintln!(
                    "warning: --points ignored. Story points field not configured. \
                     Run \"jr init\" or set [fields].story_points_field_id in ~/.config/jr/config.toml"
                );
                None
            }
        }
    } else {
        None
    }
}
```

- [ ] **Step 4: Update `handle_list` to call the new function**

In `src/cli/issue/list.rs`, replace line 114:

```rust
    let effective_sp = if show_points { sp_field_id } else { None };
```

with:

```rust
    let effective_sp = resolve_show_points(show_points, sp_field_id);
```

- [ ] **Step 5: Run all tests**

```bash
cargo test --lib
```
Expected: All tests pass, including the 3 new `resolve_show_points` tests.

- [ ] **Step 6: Run clippy and fmt**

```bash
cargo clippy --all --all-features --tests -- -D warnings
cargo fmt --all
```
Expected: Zero warnings, formatting clean.

- [ ] **Step 7: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "fix: warn when --points flag lacks story points config

Extract resolve_show_points helper that emits a stderr warning when
--points is requested but story_points_field_id is not configured.
Command still succeeds (exit 0) with degraded output.

Closes #18"
```

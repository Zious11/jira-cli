# JQL String Escaping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Escape user-supplied values in JQL string interpolation to prevent injection (closes #28).

**Architecture:** Add a `src/jql.rs` module with a single `escape_value` function that escapes `\` then `"`. Apply it to all 9 string interpolation sites in `list.rs` and `board.rs`. TDD with 5 unit tests + 1 proptest.

**Tech Stack:** Rust, proptest

**Spec:** `docs/superpowers/specs/2026-03-23-jql-string-escaping-design.md`

---

## File Structure

| File | Responsibility | Change |
|------|---------------|--------|
| `src/jql.rs` | JQL value escaping utility | Create: `escape_value` function + unit tests + proptest |
| `src/lib.rs` | Crate module declarations | Add `pub mod jql;` |
| `src/cli/issue/list.rs` | Issue list/view handlers, JQL construction | Apply `crate::jql::escape_value()` at 8 interpolation sites |
| `src/cli/board.rs` | Board handlers, kanban JQL construction | Apply `crate::jql::escape_value()` at 1 interpolation site |

---

### Task 1: Create `src/jql.rs` with tests and implementation

**Files:**
- Create: `src/jql.rs`
- Modify: `src/lib.rs:1-10` (add module declaration)

- [ ] **Step 1: Write 5 failing unit tests + 1 proptest**

Create `src/jql.rs` with tests only (no implementation yet):

```rust
/// Escape a value for interpolation into a JQL double-quoted string literal.
///
/// Backslashes are escaped first, then double quotes. Order matters: escaping
/// quotes first would introduce backslashes that the second pass re-escapes,
/// leaving the quote exposed (escape neutralization attack).
pub fn escape_value(s: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_special_characters() {
        assert_eq!(escape_value("In Progress"), "In Progress");
    }

    #[test]
    fn double_quotes_escaped() {
        assert_eq!(
            escape_value(r#"He said "hello""#),
            r#"He said \"hello\""#
        );
    }

    #[test]
    fn backslashes_escaped() {
        assert_eq!(escape_value(r"path\to\file"), r"path\\to\\file");
    }

    #[test]
    fn escape_neutralization_prevented() {
        // Input: foo\"bar (backslash then quote)
        // Must escape both: foo\\\"bar
        // JQL parser sees: \\ (literal \) + \" (literal ") + bar
        assert_eq!(escape_value(r#"foo\"bar"#), r#"foo\\\"bar"#);
    }

    #[test]
    fn trailing_backslash() {
        // foo\ must become foo\\ so the closing " of the JQL string is not consumed
        assert_eq!(escape_value(r"foo\"), r"foo\\");
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Check that no unescaped double-quote remains after escaping.
    /// An unescaped quote is one preceded by an even number of backslashes
    /// (including zero), because pairs of backslashes cancel out.
    fn has_unescaped_quote(s: &str) -> bool {
        let chars: Vec<char> = s.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            if c == '"' {
                let mut backslash_count = 0;
                let mut j = i;
                while j > 0 && chars[j - 1] == '\\' {
                    backslash_count += 1;
                    j -= 1;
                }
                // Even number of preceding backslashes means the quote is unescaped
                if backslash_count % 2 == 0 {
                    return true;
                }
            }
        }
        false
    }

    proptest! {
        #[test]
        fn escaped_value_never_has_unescaped_quote(s in "\\PC{0,100}") {
            let escaped = escape_value(&s);
            prop_assert!(
                !has_unescaped_quote(&escaped),
                "Found unescaped quote in escaped output: {:?} -> {:?}",
                s,
                escaped
            );
        }
    }
}
```

- [ ] **Step 2: Register the module in `src/lib.rs`**

In `src/lib.rs`, add `pub mod jql;` in alphabetical order — between the `pub mod error;` line (line 7) and `pub mod output;` line (line 8):

```rust
pub mod jql;
```

The full file becomes:

```rust
pub mod adf;
pub mod api;
pub mod cache;
pub mod cli;
pub mod config;
pub mod duration;
pub mod error;
pub mod jql;
pub mod output;
pub mod partial_match;
pub mod types;
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo test --lib jql
```

Expected: FAIL — `todo!()` panics at runtime. All 6 tests fail.

- [ ] **Step 4: Implement `escape_value`**

In `src/jql.rs`, replace the `todo!()` body:

```rust
pub fn escape_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test --lib jql
```

Expected: All 6 tests pass (5 unit + 1 proptest).

- [ ] **Step 6: Run clippy and fmt**

```bash
cargo clippy --all --all-features --tests -- -D warnings && cargo fmt --all
```

Expected: Zero warnings, formatting clean.

- [ ] **Step 7: Commit**

```bash
git add src/jql.rs src/lib.rs
git commit -m "feat: add jql::escape_value for JQL string injection prevention

Add src/jql.rs module with escape_value function that escapes backslash
then double-quote in user-supplied values before JQL interpolation.
Includes 5 unit tests and 1 proptest covering escape neutralization.

Part of #28"
```

---

### Task 2: Apply escaping to `list.rs` and `board.rs`

**Files:**
- Modify: `src/cli/issue/list.rs:65,68,84,88,91,144,147,150`
- Modify: `src/cli/board.rs:64`

- [ ] **Step 1: Update `src/cli/issue/list.rs` — scrum path (lines 65, 68)**

Replace line 65:

```rust
                                    jql_parts.push(format!("status = \"{}\"", s));
```

with:

```rust
                                    jql_parts.push(format!("status = \"{}\"", crate::jql::escape_value(s)));
```

Replace line 68:

```rust
                                    jql_parts.push(format!("{} = \"{}\"", field_id, team_uuid));
```

with:

```rust
                                    jql_parts.push(format!("{} = \"{}\"", field_id, crate::jql::escape_value(team_uuid)));
```

- [ ] **Step 2: Update `src/cli/issue/list.rs` — kanban path (lines 84, 88, 91)**

Replace line 84:

```rust
                            jql_parts.push(format!("project = \"{}\"", pk));
```

with:

```rust
                            jql_parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
```

Replace line 88:

```rust
                            jql_parts.push(format!("status = \"{}\"", s));
```

with:

```rust
                            jql_parts.push(format!("status = \"{}\"", crate::jql::escape_value(s)));
```

Replace line 91:

```rust
                            jql_parts.push(format!("{} = \"{}\"", field_id, team_uuid));
```

with:

```rust
                            jql_parts.push(format!("{} = \"{}\"", field_id, crate::jql::escape_value(team_uuid)));
```

- [ ] **Step 3: Update `src/cli/issue/list.rs` — `build_fallback_jql` (lines 144, 147, 150)**

Replace line 144:

```rust
        parts.push(format!("project = \"{}\"", pk));
```

with:

```rust
        parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
```

Replace line 147:

```rust
        parts.push(format!("status = \"{}\"", s));
```

with:

```rust
        parts.push(format!("status = \"{}\"", crate::jql::escape_value(s)));
```

Replace line 150:

```rust
        parts.push(format!("{} = \"{}\"", field_id, team_uuid));
```

with:

```rust
        parts.push(format!("{} = \"{}\"", field_id, crate::jql::escape_value(team_uuid)));
```

- [ ] **Step 4: Update `src/cli/board.rs` — kanban path (line 64)**

Replace line 64:

```rust
            jql_parts.push(format!("project = \"{}\"", pk));
```

with:

```rust
            jql_parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
```

- [ ] **Step 5: Run all tests**

```bash
cargo test --lib
```

Expected: All tests pass. The existing `build_fallback_jql` tests use clean values (e.g., `"PROJ"`, `"In Progress"`, `"uuid-456"`) that contain no `\` or `"`, so `escape_value` returns them unchanged.

- [ ] **Step 6: Run clippy and fmt**

```bash
cargo clippy --all --all-features --tests -- -D warnings && cargo fmt --all
```

Expected: Zero warnings, formatting clean.

- [ ] **Step 7: Commit**

```bash
git add src/cli/issue/list.rs src/cli/board.rs
git commit -m "fix: escape user-supplied values in JQL string interpolation

Apply crate::jql::escape_value() to all 9 string interpolation sites
in list.rs (8) and board.rs (1). Prevents JQL injection when --status
or --project values come from untrusted sources.

Closes #28"
```

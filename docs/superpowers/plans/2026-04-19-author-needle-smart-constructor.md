# AuthorNeedle Smart-Constructor Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the lowercase invariant on `AuthorNeedle::NameSubstring` from convention to the type system via a `LoweredStr` newtype, and rename the classifier to a smart constructor `AuthorNeedle::from_raw`.

**Architecture:** Add a `LoweredStr(String)` newtype inside a nested private submodule `mod lowered_str` in `src/cli/issue/changelog.rs`, so the tuple field is unreachable from the parent module and `::new` is the only construction path the compiler will accept. Change `NameSubstring(String)` to `NameSubstring(LoweredStr)` so every `NameSubstring` value is lowercased by construction. Rename `classify_author` free function to the associated function `AuthorNeedle::from_raw`. Pure internal refactor — no user-facing behavior changes, no new integration tests; the existing 11 unit tests are the safety net.

**Tech Stack:** Rust 2024 edition, stdlib only (`std::ops::Deref`). Existing test harness (unit tests inline in `changelog.rs`).

---

## Spec reference

`docs/specs/author-needle-smart-constructor.md` — read before starting.

## Task 1: Swap to `LoweredStr` newtype + rename to `from_raw`

Land the full refactor in one atomic commit. Attempting to split — e.g. add `LoweredStr` first, then change the variant later — would either leave `LoweredStr` unused (clippy `dead_code`, which CLAUDE.md forbids suppressing) or require a broken-build intermediate commit.

**Files:**
- Modify: `src/cli/issue/changelog.rs`

- [ ] **Step 1: Read the current state of the file**

Open `src/cli/issue/changelog.rs` and read from top to bottom. The critical regions are:

- The `handle` function branch that maps raw author input into an `AuthorNeedle` (the `Some(raw) => Some(classify_author(raw))` path on the pre-refactor tree).
- The `AuthorNeedle` enum definition.
- The `classify_author` free function, which will become the associated smart constructor `AuthorNeedle::from_raw`, along with its doc comment.
- The `author_matches` function — note where the `NameSubstring` payload is passed into `contains(...)`.
- The `#[cfg(test)] mod tests` block near the end of the file containing the 11 heuristic unit tests (pre-refactor named `classify_author_*`, renamed in Step 9 to `from_raw_*`) and the `author_matches_*` tests.

Identify every test that destructures `AuthorNeedle::NameSubstring(s)` and asserts on `s` — they will change from `assert_eq!(s, "alice")` (which relies on `String` equality with `&str`) to `assert_eq!(s.as_str(), "alice")`.

- [ ] **Step 2: Write a failing unit test pinning the `LoweredStr` invariant**

Append this test to the end of the existing `#[cfg(test)] mod tests { ... }` block in `src/cli/issue/changelog.rs` (find the last test before the closing brace of `mod tests`):

```rust
#[test]
fn lowered_str_normalizes_input_on_construction() {
    let lowered = LoweredStr::new("MixedCase-Name");
    assert_eq!(lowered.as_str(), "mixedcase-name");
}
```

- [ ] **Step 3: Run the test suite and verify the new test fails to compile**

Run: `cargo test --lib -p jr -- changelog::tests::lowered_str_normalizes_input_on_construction`

Expected: compile error `cannot find type 'LoweredStr' in this scope` or similar. This confirms the type does not yet exist — the RED state.

Do not proceed until the error is `LoweredStr`-related (not some other compile issue in the file).

- [ ] **Step 4: Add the `LoweredStr` newtype inside a private submodule**

Insert this block in `src/cli/issue/changelog.rs` immediately before the `enum AuthorNeedle` definition. Keeping `LoweredStr` inside a nested `mod lowered_str` makes the tuple-struct field unreachable from the parent (`changelog.rs`), so `LoweredStr::new` is the only construction path even for same-file code — the compiler enforces the lowercase invariant fully.

```rust
/// Private submodule so the `LoweredStr` tuple field is not reachable
/// from the rest of `changelog.rs`. `::new` therefore becomes the only
/// construction path, and the compiler enforces the lowercase invariant
/// that `author_matches` relies on: needle is lowercased at
/// construction, haystack is lowercased at match time, so `contains`
/// is sound without re-normalizing the needle.
mod lowered_str {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(super) struct LoweredStr(String);

    impl LoweredStr {
        pub(super) fn new(s: &str) -> Self {
            Self(s.to_lowercase())
        }

        pub(super) fn as_str(&self) -> &str {
            &self.0
        }
    }

    impl std::ops::Deref for LoweredStr {
        type Target = str;
        fn deref(&self) -> &str {
            &self.0
        }
    }
}

use lowered_str::LoweredStr;
```

- [ ] **Step 5: Change the `NameSubstring` variant to carry `LoweredStr`**

Replace the `AuthorNeedle` enum definition:

```rust
#[derive(Debug, Clone)]
enum AuthorNeedle {
    /// Exact accountId match (literal input or resolved from "me").
    AccountId(String),
    /// Case-insensitive substring match against `displayName` or `accountId`.
    NameSubstring(String),
}
```

With:

```rust
#[derive(Debug, Clone)]
enum AuthorNeedle {
    /// Exact accountId match (literal input or resolved from "me").
    /// Case-sensitive — Jira accountIds are opaque identifiers.
    AccountId(String),
    /// Case-insensitive substring match against `displayName` or `accountId`.
    /// The inner `LoweredStr` is lowercased at construction time, so
    /// `author_matches` lowercases the haystack at match time and compares
    /// directly without also re-normalizing the needle.
    NameSubstring(LoweredStr),
}
```

- [ ] **Step 6: Convert `classify_author` to the associated function `AuthorNeedle::from_raw`**

Replace the current free function:

```rust
/// Classify a user-supplied `--author` value. ...
fn classify_author(raw: &str) -> AuthorNeedle {
    let trimmed = raw.trim();
    let looks_like_account_id = trimmed.contains(':')
        || (trimmed.len() >= 12
            && trimmed.chars().any(|c| c.is_ascii_digit())
            && trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    if looks_like_account_id {
        AuthorNeedle::AccountId(trimmed.to_string())
    } else {
        AuthorNeedle::NameSubstring(trimmed.to_lowercase())
    }
}
```

With:

```rust
impl AuthorNeedle {
    /// Classify a user-supplied `--author` value. A value is treated as an
    /// accountId if it either contains a colon, or is ≥12 chars of
    /// `[A-Za-z0-9_-]` containing at least one digit. Otherwise it is a
    /// name substring.
    ///
    /// The API's accountId format varies (`public cloud` uses
    /// `557058:...`-style strings; older formats are opaque 24+ char
    /// hex-like blobs). Both documented formats guarantee digits, so the
    /// digit requirement distinguishes them from long digit-free display
    /// names like `AlexanderGreene` or `jean-pierre-dupont`. Residual
    /// edge: a 12+ char single-word name that incidentally contains a
    /// digit (e.g. `User12345Name`) still classifies as accountId; see
    /// issue #213 for the rationale.
    fn from_raw(raw: &str) -> Self {
        let trimmed = raw.trim();
        let looks_like_account_id = trimmed.contains(':')
            || (trimmed.len() >= 12
                && trimmed.chars().any(|c| c.is_ascii_digit())
                && trimmed
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        if looks_like_account_id {
            Self::AccountId(trimmed.to_string())
        } else {
            Self::NameSubstring(LoweredStr::new(trimmed))
        }
    }
}
```

Key differences from the old version:

- It is now an `impl AuthorNeedle` associated function named `from_raw`, not a free function.
- Variant constructors use `Self::` prefix.
- The `NameSubstring` arm constructs via `LoweredStr::new(trimmed)` (which lowercases) instead of the bare `trimmed.to_lowercase()` string.

- [ ] **Step 7: Update the single production caller of `classify_author`**

In `src/cli/issue/changelog.rs`, the `handle` function has:

```rust
        Some(raw) => Some(classify_author(raw)),
```

Change it to:

```rust
        Some(raw) => Some(AuthorNeedle::from_raw(raw)),
```

- [ ] **Step 8: Update `author_matches` to call `.as_str()` on the needle**

The current function ends with:

```rust
        AuthorNeedle::NameSubstring(n) => {
            a.display_name.to_lowercase().contains(n) || a.account_id.to_lowercase().contains(n)
        }
```

where `n: &String`. Since `n` is now `&LoweredStr`, `contains` would require deref-coercion chains. Replace with the explicit form:

```rust
        AuthorNeedle::NameSubstring(n) => {
            a.display_name.to_lowercase().contains(n.as_str())
                || a.account_id.to_lowercase().contains(n.as_str())
        }
```

- [ ] **Step 9: Update every existing unit test that calls `classify_author`**

Find every occurrence of `classify_author(` in the `#[cfg(test)]` block and replace with `AuthorNeedle::from_raw(`. There are 11 such tests as of the current tree (names matching `from_raw_*`).

Example — the `classify_author_treats_short_name_as_substring` test currently reads:

```rust
    #[test]
    fn from_raw_treats_short_name_as_substring() {
        match classify_author("alice") {
            AuthorNeedle::NameSubstring(s) => assert_eq!(s, "alice"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }
```

Update it to:

```rust
    #[test]
    fn from_raw_treats_short_name_as_substring() {
        match AuthorNeedle::from_raw("alice") {
            AuthorNeedle::NameSubstring(s) => assert_eq!(s.as_str(), "alice"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }
```

Two mechanical changes per `NameSubstring`-asserting test:

1. `classify_author(...)` → `AuthorNeedle::from_raw(...)`
2. `assert_eq!(s, "alice")` → `assert_eq!(s.as_str(), "alice")`

Tests that destructure `AuthorNeedle::AccountId(s)` and assert on `s` do **not** need the `.as_str()` change — `AccountId` still holds a `String`, and `String == &str` comparison still works. Only the `classify_author` → `AuthorNeedle::from_raw` rename applies to them.

The full list of tests to update (all in `src/cli/issue/changelog.rs`'s `mod tests`):

- `from_raw_treats_short_name_as_substring` (NameSubstring — both changes)
- `from_raw_treats_colon_string_as_accountid` (AccountId — rename only)
- `from_raw_treats_long_hex_blob_as_accountid` (AccountId — rename only)
- `from_raw_long_alpha_only_name_is_substring` (NameSubstring — both changes)
- `from_raw_long_compound_name_is_substring` (NameSubstring — both changes)
- `from_raw_long_hyphenated_name_is_substring` (NameSubstring — both changes)
- `from_raw_old_hex_accountid_is_accountid` (AccountId — rename only)
- `from_raw_colon_forces_accountid_regardless_of_heuristics` (AccountId — rename only)
- `from_raw_long_name_with_digit_is_accountid` (AccountId — rename only)
- `from_raw_short_hyphenated_name_is_substring` (NameSubstring — both changes)
- `from_raw_unknown_placeholder_is_substring` (NameSubstring — both changes)

- [ ] **Step 10: Update the `author_matches_null_author_always_false` test**

This test constructs an `AuthorNeedle::NameSubstring` inline:

```rust
    #[test]
    fn author_matches_null_author_always_false() {
        assert!(!author_matches(
            None,
            &AuthorNeedle::NameSubstring("alice".into())
        ));
    }
```

`"alice".into()` previously produced a `String`. It must now produce a `LoweredStr`. The cleanest update is to go through the constructor:

```rust
    #[test]
    fn author_matches_null_author_always_false() {
        assert!(!author_matches(
            None,
            &AuthorNeedle::NameSubstring(LoweredStr::new("alice"))
        ));
    }
```

Any other test in the file that constructs `AuthorNeedle::NameSubstring(...)` directly needs the same treatment. After the edit, grep for `NameSubstring(` to confirm no raw-string constructions remain:

```bash
grep -n 'NameSubstring(' src/cli/issue/changelog.rs
```

Every match should either destructure in a pattern (`NameSubstring(s) =>`), use `LoweredStr::new(...)`, or be produced by `AuthorNeedle::from_raw(...)`.

- [ ] **Step 11: Run `cargo fmt` and verify formatting**

Run:

```bash
cargo fmt --all
cargo fmt --all -- --check
```

Expected: second command exits 0 with no output.

- [ ] **Step 12: Run `cargo clippy --all-targets -- -D warnings`**

Run:

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: exit 0 with no warnings. If clippy flags `as_str` as unused or `Deref` impl as unused, investigate before suppressing — CLAUDE.md forbids `#[allow]` lint suppression without explicit approval. `as_str` should be used by `author_matches` and the new unit test; `Deref` impls never trigger `dead_code`.

If the target author_matches call site uses deref coercion naturally (e.g. `contains(&*n)`), clippy may or may not flag it depending on version; the plan commits to `n.as_str()` to sidestep that entirely.

- [ ] **Step 13: Run the full test suite**

Run:

```bash
cargo test
```

Expected: all tests pass (780+ as of the baseline). Pay specific attention to:

- `lowered_str_normalizes_input_on_construction` — PASS (new test, the RED → GREEN transition)
- All 11 `from_raw_*` tests — PASS
- `author_matches_respects_account_id_exact` — PASS
- `author_matches_null_author_always_false` — PASS

If any test fails, read the failure output carefully. The most likely cause is a missed spot in Step 9 or Step 10 where a test still references `classify_author(...)` or constructs `NameSubstring("...".into())` expecting `String`.

- [ ] **Step 14: Commit**

```bash
git add src/cli/issue/changelog.rs
git commit -m "$(cat <<'EOF'
refactor(changelog): LoweredStr newtype + AuthorNeedle::from_raw (#215)

Move the lowercase invariant on AuthorNeedle::NameSubstring from
convention to the type system via a module-private LoweredStr(String)
newtype whose only constructor lowercases. Rename the classifier free
function classify_author to the smart-constructor associated function
AuthorNeedle::from_raw.

Pure internal refactor — no user-facing behavior change. All 11
existing from_raw_* unit tests still pass unchanged in intent
(mechanically updated to the new type and name).
EOF
)"
```

---

## Self-review checklist (controller runs this before dispatching Task 1)

**Spec coverage:**

- ✅ `LoweredStr(String)` newtype with private constructor — Step 4.
- ✅ `Deref<Target = str>` and `as_str(&self) -> &str` — Step 4.
- ✅ `#[derive(Debug, Clone, PartialEq, Eq)]` — Step 4.
- ✅ `NameSubstring(String)` → `NameSubstring(LoweredStr)` — Step 5.
- ✅ `classify_author` renamed to `AuthorNeedle::from_raw` — Step 6.
- ✅ Heuristic body unchanged — Step 6.
- ✅ Single production caller updated — Step 7.
- ✅ `author_matches` uses `n.as_str()` — Step 8.
- ✅ All 11 unit tests updated — Steps 9 & 10.
- ✅ One new test for `LoweredStr::new` lowercase invariant — Step 2.
- ✅ No new integration tests (spec says not needed).
- ✅ `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test` all run — Steps 11–13.

**Type consistency:**

- `LoweredStr::new(&str) -> Self` used in Step 4 definition and Steps 6, 10 constructors.
- `LoweredStr::as_str(&self) -> &str` used in Step 8 and Step 9.
- `AuthorNeedle::from_raw(raw: &str) -> Self` signature consistent across Steps 6, 7, 9.

**Placeholder scan:** No TBD, TODO, or "implement later" markers present.

**Scope:** Single task because variant type + all callers + all tests must update atomically to keep the build green. Splitting creates either unused-code lint violations (forbidden) or broken-build intermediate commits.

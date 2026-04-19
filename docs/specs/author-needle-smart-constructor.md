# AuthorNeedle smart-constructor refactor

**Issue:** [#215](https://github.com/Zious11/jira-cli/issues/215) (discovered during #200 / #213 review)

**Status:** Approved 2026-04-19

## Problem

`AuthorNeedle::NameSubstring(String)` in `src/cli/issue/changelog.rs` is documented as holding an already-lowercased string. `author_matches` relies on that invariant — it lowercases the haystack (`display_name`, `account_id`) and calls `contains(n)` without re-lowercasing the needle.

Today the invariant is upheld only by convention: `classify_author` is the sole constructor of the `NameSubstring` variant, and it always lowercases. Nothing stops a future contributor from writing `AuthorNeedle::NameSubstring(raw.to_string())` elsewhere in the module; the compiler will not object, and the resulting case-mismatch bug would be silent (matches would simply fail for mixed-case names).

## Goal

Move the lowercase invariant from convention to the type system so the compiler rejects any construction that does not lowercase.

## Non-goals

- Changing the classification heuristic (that was the scope of #213)
- Changing user-visible `--author` behavior
- Adding new integration tests — this is a pure internal refactor with existing unit-test coverage

## Design

### `LoweredStr` newtype

Define the wrapper inside a nested private module so that the inner tuple field is not reachable from `changelog.rs` itself. That makes `LoweredStr::new` the *only* construction path — not just for external callers, but for code inside this same file — and the compiler therefore enforces the lowercase invariant fully.

```rust
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

Both `Deref<Target = str>` and an explicit `as_str(&self) -> &str` are implemented — the standard Rust newtype pattern, mirroring `String::as_str` and `OsStr::as_str`. `Deref` enables coercion in generic/method-call contexts (e.g. `lowered.len()`); `as_str()` is used at explicit-conversion call sites where clarity matters more than brevity.

### Enum and smart constructor

```rust
#[derive(Debug, Clone)]
enum AuthorNeedle {
    AccountId(String),          // case-sensitive; no invariant to enforce
    NameSubstring(LoweredStr),  // compiler-enforced lowercase
}

impl AuthorNeedle {
    /// Classify a user-supplied `--author` value. Heuristic body identical to
    /// the previous `classify_author` free function (unchanged): accountId iff
    /// the value contains ':' or is ≥12 chars of `[A-Za-z0-9_-]` containing at
    /// least one digit; otherwise `NameSubstring`.
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

The existing free function `classify_author` becomes the associated function `AuthorNeedle::from_raw`; its body is unchanged.

The explicit `AuthorNeedle::AccountId(...)` construction in `handle` (used for the resolved "me" accountId) stays as-is — accountIds are case-sensitive, so there is no invariant to enforce for that variant.

### Why A+B hybrid, not A alone or B alone

- **B alone (rename only)** does not solve the problem. Rust enum variants inherit the enum's visibility, so nothing prevents in-module code from constructing `NameSubstring("MixedCase".into())`. Confirmed via the Rust reference and the 2024-edition RFC discussion.
- **A alone (newtype without rename)** gives type enforcement but leaves the awkwardly-named `classify_author` free function. Renaming it to `AuthorNeedle::from_raw` costs one line and makes the API self-explanatory.
- **A+B hybrid** gets both wins for ~15 LoC more than either option alone.

The newtype is further encapsulated inside a nested private submodule (`mod lowered_str`) so that the tuple field is not accessible from `changelog.rs`. Without that nesting, same-module code — including tests — could still call `LoweredStr("UPPER".into())` directly and bypass `::new`, leaving the invariant enforced only by in-file convention. The submodule closes that gap at compile time.

## Files touched

| File | Change |
|------|--------|
| `src/cli/issue/changelog.rs` | Add `mod lowered_str { struct LoweredStr ... }` submodule with `pub(super)` surface; change `NameSubstring(String)` → `NameSubstring(LoweredStr)`; rename `classify_author` → `AuthorNeedle::from_raw`; update `author_matches` to call `contains(n.as_str())`; update unit tests to compare via `s.as_str()`. |

No other files change. The enum and all related items remain module-private to `changelog.rs`.

## Test strategy

The 10 existing unit tests in `src/cli/issue/changelog.rs` (`classify_author_*`) already cover the heuristic's behavior:

- Short name → substring
- Colon → accountId
- Long hex blob → accountId
- Long alpha-only single word → substring
- Long compound name → substring
- Long hyphenated name → substring
- Old 24-char hex accountId → accountId
- Long name with digit → accountId
- Short hyphenated name → substring
- Unknown placeholder → substring

They need mechanical updates:

- Call `AuthorNeedle::from_raw(...)` instead of `classify_author(...)`
- Compare the `NameSubstring` inner via `s.as_str()` instead of direct `String` equality

Behavior is unchanged, so no new behavior-oriented test cases are needed. Two small unit tests are added to pin the newtype's type-level contract:

- `lowered_str_normalizes_input_on_construction` — asserts `LoweredStr::new("MixedCase-Name").as_str() == "mixedcase-name"`.
- `lowered_str_equality_is_case_invariant` — asserts `LoweredStr::new("Alice") == LoweredStr::new("alice")`, exercising the derived `PartialEq`/`Eq`.

## Rollout

Single commit, single file. No migration concerns — the type is confined to a nested private submodule and has no public API surface.

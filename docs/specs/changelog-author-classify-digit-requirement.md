# `jr issue changelog --author` classification fix

**Issue:** [#213](https://github.com/Zious11/jira-cli/issues/213)

## Problem

`classify_author()` in `src/cli/issue/changelog.rs` misclassifies long
single-word display names as accountIds. The current heuristic treats any
≥12-char string composed of `[A-Za-z0-9_-]` as an accountId candidate,
which then requires an exact-equality match in `author_matches()`.

Concrete regressions:

| Input | Current classification | Expected |
|---|---|---|
| `AlexanderGreene` (15 chars, no digits) | AccountId → exact match → 0 hits | NameSubstring |
| `JoseMariaRodriguez` (18 chars, no digits) | AccountId → 0 hits | NameSubstring |
| `jean-pierre-dupont` (18 chars, no digits) | AccountId → 0 hits | NameSubstring |

A user passing their own (or a teammate's) long-but-digit-free name gets
an empty result with no hint that the classifier misrouted them.

## Approach

**Tighten the heuristic: require a colon OR at least one digit** (in
addition to the existing ≥12-char and alphanumeric/`-`/`_` gates).

Both documented Jira Cloud accountId formats guarantee digits:

- Old 24-char hex: `5b10ac8d82e05b22cc7d4ef5` (contains `0-9` alongside
  `a-f`).
- New prefixed: `557058:f58131cb-b67d-43c7-b30d-6b58d40bd077` (colon
  plus 6-digit numeric prefix).
- Service accounts, bot accounts, Forge apps, Connect addons, and
  Marketplace apps all use the same two formats.
- Deleted/migrated users surface as the literal `"unknown"` (7 chars,
  below the length gate — already handled by NameSubstring).

Validated twice with Perplexity (accountId formats + edge-case account
types). Context7 on clap was unavailable (quota) but the change is
pure-logic and does not touch clap surface.

### Rejected alternatives

- **Option B (explicit `--author-name` / `--author-id` flags)** — breaks
  the single-flag `gh`-style UX; no dominant CLI convention to align
  with (Perplexity inconclusive); adds deprecation-path surface.
- **Option C (stderr hint on zero-match + AccountId)** — cheap but
  script-invisible, and its value shrinks once A closes the common
  miss. Can be revisited if a real user hits the residual edge.

## Algorithm change

In `src/cli/issue/changelog.rs::classify_author`:

```rust
// Before
let looks_like_account_id = trimmed.contains(':')
    || (trimmed.len() >= 12
        && trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));

// After
let looks_like_account_id = trimmed.contains(':')
    || (trimmed.len() >= 12
        && trimmed.chars().any(|c| c.is_ascii_digit())
        && trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
```

One new predicate: `.any(|c| c.is_ascii_digit())`.

## Residual edge

A 12+ char single-word name that incidentally contains a digit
(e.g. `User12345Name`) still misclassifies as AccountId. Documented in
tests. Rationale: further tightening (e.g. checking digit ratio) adds
complexity without a measured need. If users hit this in practice,
Option C (stderr hint) is the cheapest follow-up.

## Help text

Update `src/cli/mod.rs` Changelog `--author` long-help to reflect the
new rule. Replace:

> AccountIds (values containing ':' or ≥12 characters of letters,
> digits, '-', or '_') are matched exactly; other values match as a
> case-insensitive substring of displayName or accountId.

With:

> AccountIds (values containing ':' or ≥12 characters with at least one
> digit plus letters, digits, '-', or '_') are matched exactly; other
> values match as a case-insensitive substring of displayName or
> accountId.

Short help (`-h` one-liner) unchanged.

Also update the `///` docstring on `classify_author` to match.

## Tests

Add unit tests in `src/cli/issue/changelog.rs` `#[cfg(test)] mod tests`:

| Input | Expected | Purpose |
|---|---|---|
| `AlexanderGreene` | NameSubstring | regression guard: 15 chars no digit |
| `JoseMariaRodriguez` | NameSubstring | regression guard: 18 chars no digit |
| `jean-pierre-dupont` | NameSubstring | regression guard: dashed 18 chars no digit |
| `5b10ac8d82e05b22cc7d4ef5` | AccountId | old hex format (has digits) |
| `557058:f58131cb-b67d-43c7` | AccountId | colon-prefixed format (colon branch) |
| `User12345Name` | AccountId | documented residual edge (digit + 13 chars) |
| `jean-pierre` | NameSubstring | short (11 < 12) unchanged |
| `unknown` | NameSubstring | deleted-user accountId stub; 7 < 12 ⇒ substring |

If `classify_author` is currently `fn` (private, no `pub`), no visibility
change needed — tests live in the same module.

## Out of scope

- Option C stderr hint.
- Any change to `author_matches` semantics.
- Any change to `helpers::is_me_keyword` or the `"me"` resolution path.
- Behavior for `ChangelogEntry.author == None` (still filtered out as
  before via `let Some(a) = author else { return false }`).

## Files touched

- `src/cli/issue/changelog.rs` — 1-line predicate addition, docstring
  update, 8 new unit tests.
- `src/cli/mod.rs` — 1 line of `--author` long-help text.

## Exit criteria

- All existing tests pass.
- 8 new unit tests pass (TDD: write failing, tighten heuristic, pass).
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` green.
- Help text renders new wording in `jr issue changelog --help`.

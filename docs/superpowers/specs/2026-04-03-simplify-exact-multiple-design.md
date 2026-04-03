# Design: Simplify ExactMultiple variant and replace unreachable arms

**Issues:** [#126](https://github.com/Zious11/jira-cli/issues/126), [#127](https://github.com/Zious11/jira-cli/issues/127)
**Date:** 2026-04-03
**Status:** Draft

## Problem

`MatchResult::ExactMultiple(Vec<String>)` carries a Vec that no caller actually needs:

- 4 user-resolution callers (`helpers.rs`) ignore the Vec with `_` and re-query the original data source by lowercased name.
- 6 non-user callers have provably unreachable `ExactMultiple` arms (candidates are pre-deduplicated upstream) but silently take first with `names.into_iter().next().unwrap()`.
- 2 callers (`queue.rs`, `resolve_schema` in `assets.rs`) use `names.contains()` for filtering, but since the Vec only holds duplicate copies of the same case-insensitive string, this is equivalent to a lowercased comparison.

The Vec contains near-zero unique information — just duplicate copies of the matched name, possibly differing in casing. A single `String` is sufficient. The silent take-first fallback in 6 unreachable arms hides invariant violations.

Validated with Perplexity: simplifying to `ExactMultiple(String)` aligns with Rust's principle of minimal payloads matching actual usage. `unreachable!()` is idiomatic for provably impossible match arms.

## Scope

Two changes, one PR:

1. **Simplify variant** — `ExactMultiple(Vec<String>)` to `ExactMultiple(String)` (#127)
2. **Replace unreachable arms** — 6 silent take-first arms become `unreachable!()` (#126)

Plus cleanup of one dead code path in `queue.rs`.

## Design

### Change 1: Simplify `ExactMultiple` variant

```rust
// Before
ExactMultiple(Vec<String>),

// After
ExactMultiple(String),
```

The `String` is one representative matched name (the first exact match found, preserving original casing).

Construction in `partial_match()` changes from:

```rust
n if n > 1 => return MatchResult::ExactMultiple(exact_matches),
```

To:

```rust
n if n > 1 => return MatchResult::ExactMultiple(exact_matches.into_iter().next().unwrap()),
```

### Change 2: Replace 6 unreachable arms

All sites where candidates are provably pre-deduplicated upstream. Each arm becomes:

```rust
MatchResult::ExactMultiple(_) => {
    unreachable!("ExactMultiple should not occur: candidates are deduplicated")
}
```

Affected sites:

| File | Line | Dedup mechanism |
|------|------|-----------------|
| `src/cli/issue/workflow.rs` | ~143 | `HashSet` collects unique `(name, idx)` pairs |
| `src/cli/issue/list.rs` | ~181 | Statuses from project endpoint (unique by definition) |
| `src/cli/issue/links.rs` | ~64 | Link types unique in Jira by design |
| `src/cli/issue/links.rs` | ~136 | Link types unique in Jira by design |
| `src/cli/assets.rs` | ~334 | `HashSet` dedup on ticket statuses |
| `src/cli/assets.rs` | ~668 | `.sort(); .dedup()` on object type names |

### Change 3: Update 2 filtering callers

**`resolve_schema` (`assets.rs:~459`):** Change from `names.contains(&s.name)` to `s.name.to_lowercase() == input.to_lowercase()`. The `input` variable is already in scope (function parameter).

Before:
```rust
MatchResult::ExactMultiple(names) => {
    let duplicates: Vec<String> = schemas
        .iter()
        .filter(|s| names.contains(&s.name))
        .map(|s| format!("{} (id: {})", s.name, s.id))
        .collect();
    // ...error with duplicates
}
```

After:
```rust
MatchResult::ExactMultiple(name) => {
    let input_lower = input.to_lowercase();
    let duplicates: Vec<String> = schemas
        .iter()
        .filter(|s| s.name.to_lowercase() == input_lower)
        .map(|s| format!("{} (id: {})", s.name, s.id))
        .collect();
    // ...error with duplicates (unchanged message format)
}
```

**`resolve_queue_id` (`queue.rs:~165`):** Same pattern.

Before:
```rust
MatchResult::ExactMultiple(names) => {
    let matching: Vec<&Queue> =
        queues.iter().filter(|q| names.contains(&q.name)).collect();
    let ids: Vec<String> = matching.iter().map(|q| q.id.clone()).collect();
    Err(JrError::UserError(format!(
        "Multiple queues named \"{}\" found (IDs: {}). Use --id {} to specify.",
        names[0], ids.join(", "), ids[0]
    )).into())
}
```

After:
```rust
MatchResult::ExactMultiple(matched_name) => {
    let name_lower = name.to_lowercase();
    let matching: Vec<&Queue> =
        queues.iter().filter(|q| q.name.to_lowercase() == name_lower).collect();
    let ids: Vec<String> = matching.iter().map(|q| q.id.clone()).collect();
    Err(JrError::UserError(format!(
        "Multiple queues named \"{}\" found (IDs: {}). Use --id {} to specify.",
        matched_name, ids.join(", "), ids[0]
    )).into())
}
```

Note: `name` (the function parameter / original user input) is used for lowercased comparison. `matched_name` (from the variant) is used for display.

### Change 4: Remove dead code in queue.rs Exact branch

The `matching.len() > 1` check in the `Exact` branch (`queue.rs:~152`) is now dead — `ExactMultiple` catches duplicate candidate strings before `Exact` fires. Remove the branch, leaving just `Ok(matching[0].id.clone())`.

This also applies to the `find_queue_id` test helper which mirrors the production code.

### Change 5: Simplify helpers.rs arms

The 4 user-resolution callers already use `ExactMultiple(_)` (ignoring the Vec). The destructuring pattern stays the same — no functional change. They continue filtering from the original data source using `name.to_lowercase()`.

### Change 6: Update tests

**`partial_match.rs` unit tests:** Assert on `String` instead of `Vec`:

```rust
// Before
MatchResult::ExactMultiple(names) => {
    assert_eq!(names.len(), 2);
    assert!(names.iter().all(|n| n == "John Smith"));
}

// After
MatchResult::ExactMultiple(name) => {
    assert_eq!(name, "John Smith");
}
```

The `test_exact_match_duplicate_case_insensitive` test changes to verify the representative name (first match), not all casing variants.

The `test_exact_match_three_duplicates` test simplifies — it can no longer assert on count (the count is not in the variant). It verifies that `ExactMultiple` fires and contains a representative name.

**`partial_match.rs` proptest:** `duplicate_candidates_yield_exact_multiple` simplifies — assert the name matches case-insensitively instead of checking Vec length.

**`queue.rs` test helper:** Update `ExactMultiple` arm from `names.len()` to a fixed error string.

**Integration tests (`tests/duplicate_user_disambiguation.rs`):** No changes — they test CLI output (error messages), not variant internals.

## What stays the same

- All user-facing behavior: error messages, disambiguation prompts, exit codes
- `Exact`, `Ambiguous`, `None` variant semantics
- Substring matching logic in `partial_match`
- The 4 user-resolution callers in `helpers.rs` (functional behavior unchanged)
- Integration tests
- Unicode handling: `to_lowercase()` is used consistently with `partial_match` itself (Unicode case folding is a pre-existing concern, not introduced by this change)

## Edge cases

- **`unreachable!()` fires in production:** Means an upstream dedup was removed or broken. The panic message identifies the cause. This is strictly better than silently picking the wrong item.
- **`resolve_schema` with case-differing duplicates:** Two schemas named "Assets" and "assets" would both match. The error message lists both with IDs — same behavior as before.
- **Queue with case-differing duplicate names:** Same as schema — error lists all matching queues with IDs.

## Files modified

- `src/partial_match.rs` — variant change + construction + test updates (~20 lines)
- `src/cli/issue/workflow.rs` — `unreachable!()` (~8 lines removed, 3 added)
- `src/cli/issue/list.rs` — `unreachable!()` (~3 lines changed)
- `src/cli/issue/links.rs` — `unreachable!()` x2 (~6 lines changed)
- `src/cli/assets.rs` — `unreachable!()` x2 + `resolve_schema` filter change (~10 lines changed)
- `src/cli/queue.rs` — filter change + dead code removal (~15 lines changed)
- `src/cli/issue/helpers.rs` — no functional change (destructuring stays `_`)

No new dependencies. No new CLI flags or config options.

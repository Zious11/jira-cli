# Simplify ExactMultiple Variant Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Simplify `ExactMultiple(Vec<String>)` to `ExactMultiple(String)` and replace 6 provably-unreachable match arms with `unreachable!()`.

**Architecture:** Change the enum variant in `partial_match.rs`, then update all 12 call sites across 7 files. Six arms become `unreachable!()`, two filtering callers switch from `names.contains()` to lowercased comparison, four user-resolution callers stay unchanged (already ignore payload).

**Tech Stack:** Rust, no new dependencies

**Spec:** `docs/superpowers/specs/2026-04-03-simplify-exact-multiple-design.md`

---

## File Map

| File | Change type | What changes |
|------|-------------|--------------|
| `src/partial_match.rs` | Modify | Variant `Vec<String>` → `String`, construction, 3 unit tests, 1 proptest |
| `src/cli/issue/workflow.rs` | Modify | ExactMultiple arm → `unreachable!()` |
| `src/cli/issue/list.rs` | Modify | ExactMultiple arm → `unreachable!()` |
| `src/cli/issue/links.rs` | Modify | 2 ExactMultiple arms → `unreachable!()` |
| `src/cli/assets.rs` | Modify | 2 ExactMultiple arms → `unreachable!()`, 1 ExactMultiple filtering caller updated |
| `src/cli/queue.rs` | Modify | ExactMultiple filtering caller updated, dead code removed from Exact branch, test helper updated |
| `src/cli/issue/helpers.rs` | No change | Already uses `ExactMultiple(_)` — compiles as-is with `String` |

---

### Task 1: Change ExactMultiple variant and update tests in partial_match.rs

**Files:**
- Modify: `src/partial_match.rs:1-200`

- [ ] **Step 1: Update the enum variant from `Vec<String>` to `String`**

In `src/partial_match.rs`, change line 7 from:

```rust
    /// Multiple candidates share the same exact (case-insensitive) name
    ExactMultiple(Vec<String>),
```

To:

```rust
    /// Multiple candidates share the same exact (case-insensitive) name — carries one representative
    ExactMultiple(String),
```

- [ ] **Step 2: Update the construction in `partial_match()`**

In `src/partial_match.rs`, change line 27 from:

```rust
        n if n > 1 => return MatchResult::ExactMultiple(exact_matches),
```

To:

```rust
        n if n > 1 => return MatchResult::ExactMultiple(exact_matches.into_iter().next().unwrap()),
```

- [ ] **Step 3: Update `test_exact_match_duplicate_returns_exact_multiple`**

In `src/partial_match.rs`, replace the test (lines 103-112) with:

```rust
    #[test]
    fn test_exact_match_duplicate_returns_exact_multiple() {
        let candidates = vec!["John Smith".into(), "Jane Doe".into(), "John Smith".into()];
        match partial_match("John Smith", &candidates) {
            MatchResult::ExactMultiple(name) => {
                assert_eq!(name, "John Smith");
            }
            other => panic!("Expected ExactMultiple, got {:?}", other),
        }
    }
```

- [ ] **Step 4: Update `test_exact_match_duplicate_case_insensitive`**

In `src/partial_match.rs`, replace the test (lines 114-126) with:

```rust
    #[test]
    fn test_exact_match_duplicate_case_insensitive() {
        let candidates = vec!["John Smith".into(), "john smith".into()];
        match partial_match("john smith", &candidates) {
            MatchResult::ExactMultiple(name) => {
                // Preserves casing of the first match
                assert_eq!(name, "John Smith");
            }
            other => panic!("Expected ExactMultiple, got {:?}", other),
        }
    }
```

- [ ] **Step 5: Update `test_exact_match_three_duplicates`**

In `src/partial_match.rs`, replace the test (lines 128-142) with:

```rust
    #[test]
    fn test_exact_match_three_duplicates() {
        let candidates = vec![
            "John Smith".into(),
            "Jane Doe".into(),
            "John Smith".into(),
            "John Smith".into(),
        ];
        match partial_match("John Smith", &candidates) {
            MatchResult::ExactMultiple(name) => {
                assert_eq!(name, "John Smith");
            }
            other => panic!("Expected ExactMultiple, got {:?}", other),
        }
    }
```

- [ ] **Step 6: Update the `duplicate_candidates_yield_exact_multiple` proptest**

In `src/partial_match.rs`, replace the proptest (lines 180-198) with:

```rust
        #[test]
        fn duplicate_candidates_yield_exact_multiple(idx in 0usize..4) {
            let base: Vec<String> = vec![
                "In Progress".into(), "In Review".into(),
                "Blocked".into(), "Done".into(),
            ];
            // Duplicate one candidate
            let mut candidates = base.clone();
            candidates.push(base[idx].clone());
            let input = base[idx].clone();
            match partial_match(&input, &candidates) {
                MatchResult::ExactMultiple(name) => {
                    prop_assert_eq!(name.to_lowercase(), input.to_lowercase());
                }
                _ => prop_assert!(false, "Expected ExactMultiple for duplicated '{}'", input),
            }
        }
```

- [ ] **Step 7: Run partial_match tests to verify**

Run: `cargo test --lib partial_match`

Expected: All tests pass (unit tests + proptests).

- [ ] **Step 8: Commit**

```bash
git add src/partial_match.rs
git commit -m "refactor: simplify ExactMultiple variant from Vec<String> to String (#127)"
```

---

### Task 2: Replace 6 unreachable ExactMultiple arms and update 2 filtering callers

> **Implementation note:** During PR review, it was identified that 5 of the 6 dedup
> sites use case-sensitive dedup while `partial_match` operates case-insensitively.
> Only `workflow.rs` (which uses `to_lowercase()` keys in its HashSet) is truly
> unreachable. The other 5 sites were changed to graceful fallback
> (`MatchResult::ExactMultiple(name) => name`) instead of `unreachable!()`.
> Steps 2-5 and 7 below show the original plan; the actual implementation differs.

**Files:**
- Modify: `src/cli/issue/workflow.rs:143-157`
- Modify: `src/cli/issue/list.rs:181-184`
- Modify: `src/cli/issue/links.rs:64-67, 136-139`
- Modify: `src/cli/assets.rs:334-337, 459-471, 668-671`
- Modify: `src/cli/queue.rs:148-176, 213-232`

- [ ] **Step 1: Replace ExactMultiple arm in `workflow.rs`**

In `src/cli/issue/workflow.rs`, replace lines 143-157:

```rust
            MatchResult::ExactMultiple(names) => {
                // Duplicate transition names not expected; take first
                let name = names.into_iter().next().unwrap();
                let idx = candidates
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, i)| *i)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Internal error: matched candidate \"{}\" not found. Please report this as a bug.",
                            name
                        )
                    })?;
                &transitions[idx]
            }
```

With:

```rust
            MatchResult::ExactMultiple(_) => {
                unreachable!("ExactMultiple should not occur: candidates are deduplicated")
            }
```

- [ ] **Step 2: Replace ExactMultiple arm in `list.rs`**

In `src/cli/issue/list.rs`, replace lines 181-184:

```rust
            MatchResult::ExactMultiple(names) => {
                // Duplicate status names not expected; take first
                Some(names.into_iter().next().unwrap())
            }
```

With:

```rust
            MatchResult::ExactMultiple(_) => {
                unreachable!("ExactMultiple should not occur: statuses are unique")
            }
```

- [ ] **Step 3: Replace first ExactMultiple arm in `links.rs` (handle_link)**

In `src/cli/issue/links.rs`, replace lines 64-67:

```rust
        MatchResult::ExactMultiple(names) => {
            // Duplicate link type names not expected; take first
            names.into_iter().next().unwrap()
        }
```

With:

```rust
        MatchResult::ExactMultiple(_) => {
            unreachable!("ExactMultiple should not occur: link types are unique")
        }
```

- [ ] **Step 4: Replace second ExactMultiple arm in `links.rs` (handle_unlink)**

In `src/cli/issue/links.rs`, replace lines 136-139:

```rust
            MatchResult::ExactMultiple(names) => {
                // Duplicate link type names not expected; take first
                names.into_iter().next().unwrap()
            }
```

With:

```rust
            MatchResult::ExactMultiple(_) => {
                unreachable!("ExactMultiple should not occur: link types are unique")
            }
```

- [ ] **Step 5: Replace ExactMultiple arm in `assets.rs` ticket status filter**

In `src/cli/assets.rs`, replace lines 334-337:

```rust
            MatchResult::ExactMultiple(names) => {
                // Duplicate status names not expected; take first
                names.into_iter().next().unwrap()
            }
```

With:

```rust
            MatchResult::ExactMultiple(_) => {
                unreachable!("ExactMultiple should not occur: statuses are deduplicated")
            }
```

- [ ] **Step 6: Update ExactMultiple arm in `assets.rs` resolve_schema**

In `src/cli/assets.rs`, replace lines 459-471:

```rust
        MatchResult::ExactMultiple(names) => {
            let duplicates: Vec<String> = schemas
                .iter()
                .filter(|s| names.contains(&s.name))
                .map(|s| format!("{} (id: {})", s.name, s.id))
                .collect();
            Err(JrError::UserError(format!(
                "Multiple schemas named \"{}\": {}. Use the schema ID instead.",
                input,
                duplicates.join(", ")
            ))
            .into())
        }
```

With:

```rust
        MatchResult::ExactMultiple(_) => {
            let input_lower = input.to_lowercase();
            let duplicates: Vec<String> = schemas
                .iter()
                .filter(|s| s.name.to_lowercase() == input_lower)
                .map(|s| format!("{} (id: {})", s.name, s.id))
                .collect();
            Err(JrError::UserError(format!(
                "Multiple schemas named \"{}\": {}. Use the schema ID instead.",
                input,
                duplicates.join(", ")
            ))
            .into())
        }
```

- [ ] **Step 7: Replace ExactMultiple arm in `assets.rs` object type**

In `src/cli/assets.rs`, replace lines 668-671:

```rust
        MatchResult::ExactMultiple(names) => {
            // Duplicate type names not expected after dedup; take first
            names.into_iter().next().unwrap()
        }
```

With:

```rust
        MatchResult::ExactMultiple(_) => {
            unreachable!("ExactMultiple should not occur: type names are deduplicated")
        }
```

- [ ] **Step 8: Update ExactMultiple arm and remove dead code in `queue.rs` production code**

In `src/cli/queue.rs`, replace the entire `Exact` and `ExactMultiple` arms (lines 148-177):

```rust
        MatchResult::Exact(matched_name) => {
            let matching: Vec<&crate::types::jsm::Queue> =
                queues.iter().filter(|q| q.name == matched_name).collect();

            if matching.len() > 1 {
                let ids: Vec<String> = matching.iter().map(|q| q.id.clone()).collect();
                Err(JrError::UserError(format!(
                    "Multiple queues named \"{}\" found (IDs: {}). Use --id {} to specify.",
                    matched_name,
                    ids.join(", "),
                    ids[0]
                ))
                .into())
            } else {
                Ok(matching[0].id.clone())
            }
        }
        MatchResult::ExactMultiple(names) => {
            // ExactMultiple means partial_match found duplicate candidate strings.
            let matching: Vec<&crate::types::jsm::Queue> =
                queues.iter().filter(|q| names.contains(&q.name)).collect();
            let ids: Vec<String> = matching.iter().map(|q| q.id.clone()).collect();
            Err(JrError::UserError(format!(
                "Multiple queues named \"{}\" found (IDs: {}). Use --id {} to specify.",
                names[0],
                ids.join(", "),
                ids[0]
            ))
            .into())
        }
```

With:

```rust
        MatchResult::Exact(matched_name) => {
            Ok(queues
                .iter()
                .find(|q| q.name == matched_name)
                .expect("matched name must exist in queues")
                .id
                .clone())
        }
        MatchResult::ExactMultiple(matched_name) => {
            let name_lower = name.to_lowercase();
            let matching: Vec<&crate::types::jsm::Queue> = queues
                .iter()
                .filter(|q| q.name.to_lowercase() == name_lower)
                .collect();
            let ids: Vec<String> = matching.iter().map(|q| q.id.clone()).collect();
            Err(JrError::UserError(format!(
                "Multiple queues named \"{}\" found (IDs: {}). Use --id {} to specify.",
                matched_name,
                ids.join(", "),
                ids[0]
            ))
            .into())
        }
```

- [ ] **Step 9: Update the `find_queue_id` test helper in `queue.rs`**

In `src/cli/queue.rs`, replace the test helper (lines 213-233):

```rust
    fn find_queue_id(name: &str, queues: &[Queue]) -> Result<String, String> {
        let names: Vec<String> = queues.iter().map(|q| q.name.clone()).collect();
        match crate::partial_match::partial_match(name, &names) {
            crate::partial_match::MatchResult::Exact(matched_name) => {
                let matching: Vec<&Queue> =
                    queues.iter().filter(|q| q.name == matched_name).collect();
                if matching.len() > 1 {
                    Err(format!("duplicate: {}", matching.len()))
                } else {
                    Ok(matching[0].id.clone())
                }
            }
            crate::partial_match::MatchResult::ExactMultiple(names) => {
                Err(format!("duplicate: {}", names.len()))
            }
            crate::partial_match::MatchResult::Ambiguous(m) => {
                Err(format!("ambiguous: {}", m.len()))
            }
            crate::partial_match::MatchResult::None(_) => Err("none".into()),
        }
    }
```

With:

```rust
    fn find_queue_id(name: &str, queues: &[Queue]) -> Result<String, String> {
        let names: Vec<String> = queues.iter().map(|q| q.name.clone()).collect();
        match crate::partial_match::partial_match(name, &names) {
            crate::partial_match::MatchResult::Exact(matched_name) => {
                Ok(queues
                    .iter()
                    .find(|q| q.name == matched_name)
                    .expect("matched name must exist in queues")
                    .id
                    .clone())
            }
            crate::partial_match::MatchResult::ExactMultiple(_) => {
                Err("duplicate".into())
            }
            crate::partial_match::MatchResult::Ambiguous(m) => {
                Err(format!("ambiguous: {}", m.len()))
            }
            crate::partial_match::MatchResult::None(_) => Err("none".into()),
        }
    }
```

- [ ] **Step 10: Run all tests to verify**

Run: `cargo test`

Expected: All tests pass. The 4 helpers.rs callers use `ExactMultiple(_)` which compiles with both `Vec<String>` and `String`.

- [ ] **Step 11: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`

Expected: No warnings, no format violations.

- [ ] **Step 12: Commit**

```bash
git add src/cli/issue/workflow.rs src/cli/issue/list.rs src/cli/issue/links.rs src/cli/assets.rs src/cli/queue.rs
git commit -m "refactor: replace unreachable ExactMultiple arms and update filtering callers (#126)"
```

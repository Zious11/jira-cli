# S-3.10 Red Gate Log

Story: S-3.10 — Rewrite `format_roundtrip` proptest + delete deprecated 3-arg `parse_duration` calculator + retire H-018
Step: 3 (Test Writer)
Date: 2026-05-08

---

## Red Gate Classification

**REFACTOR-ONLY** — no new behavior introduced.

This story replaces an existing proptest body with a more robust structural assertion pattern. The new proptest passes immediately after being written because `format_duration`'s output behavior is completely unchanged. Traditional Red Gate ("write a failing test that passes only after implementation") does not apply here: the test *is* the deliverable for step 3, and the production function under test is unmodified.

---

## Verification Performed

### format_roundtrip proptest

```
cargo test --lib -- duration::proptests::format_roundtrip
```

Result: **PASS** — 1 test passed, 0 failed (proptest default 256 cases run, all green, finished in 0.01s).

Proptest strategy: `(1u64..1440u64).prop_map(|m| m * 60)` — generates 1439 distinct inputs (minutes 1..1439, converted to seconds). Zero rejection overhead vs. the previous `prop_filter` approach which rejected ~98.3% of inputs in the original `1u64..86400` range.

### parse_duration 3-arg signature still present

```
rg -n "parse_duration\b" src/duration.rs
```

Result: 3-arg `pub fn parse_duration` signature found at **line 98** (function definition intact). Existing unit test callers at lines 164–200 are present and untouched. The `format_roundtrip` proptest body (lines 282–304) contains **zero** calls to `parse_duration`. The other proptests at lines 263, 272, 278 (`valid_single_units_always_parse`, `combined_units_always_parse`, `garbage_input_never_panics`) still call the 3-arg function — those are in scope for step 4 deletion, not step 3.

Deletion of the 3-arg function itself is step 4 of the delivery workflow.

---

## Why Traditional Red Gate Does Not Apply

AC-001 rewrites an existing test, not a test for new behavior. The standard TDD Red Gate pattern is:

1. Write a test for behavior that does not yet exist → test fails (Red)
2. Write minimum implementation → test passes (Green)
3. Refactor

Here, `format_duration` already exists and is correct. The old `format_roundtrip` proptest also passed — but it used the 3-arg `parse_duration` calculator as its inverse, which is the "code pollution" anti-pattern (production code kept alive only for test infrastructure, not for user-observable behavior).

The step 3 deliverable is the *restructured test pattern* — a structural assertion that reconstructs seconds from the formatted token string without calling any inverse function. The "failing first" gate is replaced by "verify new structural assertion passes against the unchanged behavior" — which proves the test correctly exercises `format_duration`'s output contract.

The true deletion-gate ("all callers gone") is verified via `cargo build --all-targets` in step 4, when the 3-arg function is removed and the compiler confirms zero remaining references.

---

## Step 4 Pre-conditions

Before step 4 (Implementer) can safely delete `parse_duration` (3-arg), the following must be true:

1. `cargo test --lib -- duration::proptests::format_roundtrip` passes — CONFIRMED (step 3).
2. The `format_roundtrip` proptest body contains zero calls to `parse_duration` — CONFIRMED (step 3).
3. The `parse_duration_validate` unit tests (lines 205–238) still pass — these test the validator function which is unmodified and must remain passing.
4. The remaining callers of 3-arg `parse_duration` in `src/duration.rs` are:
   - Lines 164–200: unit tests `test_minutes`, `test_hours`, `test_hours_and_minutes`, `test_day`, `test_week`, `test_complex`, `test_empty_fails`, `test_number_without_unit_fails`, `test_invalid_unit_fails` — step 4 must delete these along with the function.
   - Lines 263–279: proptests `valid_single_units_always_parse`, `combined_units_always_parse`, `garbage_input_never_panics` — step 4 must delete these along with the function.
5. After deletion, `cargo build --all-targets` must exit 0 (definitive zero-caller gate; plain `cargo build` does NOT compile test targets and is insufficient).
6. After deletion, `rg -n "parse_duration\b" --type rust src/` must return ONLY `parse_duration_validate` hits — zero matches for the 3-arg variant.

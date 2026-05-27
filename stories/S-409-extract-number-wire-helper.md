---
document_type: story
story_id: "S-409"
title: "Extract parsed_number_to_wire_value helper + replace tautological test 38 with real unit tests (refs #409)"
wave: feature-followup
status: ready
intent: enhancement
feature_type: refactor
scope: trivial
severity: low
trivial_scope: true
issue: 409
points: 1
priority: low
tdd_mode: strict
estimated_effort: xsmall
depends_on: []
bc_anchors:
  - BC-3.4.015   # EC-3.4.015-4a — wire-form invariant (existing; this story only relocates its coverage)
# BC status: coverage relocation only — BC-3.4.015 is not modified, only re-covered by
# real unit tests instead of a tautological integration-test re-implementation.
# Status=ready is justified because no new BC is being authored; the story relocates
# existing BC-3.4.015 coverage from a tautological test to real unit tests.
verification_properties: []
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: "github.com/Zious11/jira-cli/issues/409"
implementation_strategy: tdd
module_criticality: LOW
files_modified:
  - src/cli/issue/field_resolve.rs   # MODIFIED — pub(crate) helper added; callsite at lines 286-292 replaced; 6 inline unit tests added
  - tests/issue_edit_field.rs        # MODIFIED — test 38 removed; tombstone comment block added
files_created: []
breaking_change: false
assumption_validations: []
risk_mitigations: []
---

# S-409 — Extract `parsed_number_to_wire_value` Helper + Replace Tautological Test 38

## Source of Truth

GitHub issue: https://github.com/Zious11/jira-cli/issues/409

**No new BCs. No new VPs. No new ADR. Production behavior byte-identical.**

## Goal

Extract the 7-line inline number-to-wire-value conversion in `src/cli/issue/field_resolve.rs`
into a named `pub(crate)` helper, replace the callsite with the helper, and swap the
tautological integration test 38 (which re-implements the same logic from first principles)
with 6 focused inline unit tests that call the actual production helper.

## Problem Statement

Copilot review finding R2-C4 on PR #401 (deferred during the S-396 cycle; re-validated in
`.factory/research/follow-up-validation-396.md`):

The `"number"` branch in `field_resolve.rs` contains an inline conversion (lines 286-292)
that decides whether to emit a parsed `f64` as an `i64` wire value (whole numbers) or as a
JSON number (fractional). Integration test 38 in `tests/issue_edit_field.rs` (lines
2425-2477) tests this same decision by re-implementing the predicate from scratch in the
test body — if the implementation ever changes, the test stays green because it does not
call the actual code.

This is a textbook tautological test: it validates the test's own logic, not the
production logic. Extracting the helper and writing inline unit tests against it closes
the gap without any behavior change.

## Behavioral Contracts

BC-3.4.015 EC-3.4.015-4a anchors the wire-form invariant (whole numbers must be emitted
as i64, not f64). This story does NOT modify BC-3.4.015; it only relocates the coverage
of the invariant from a tautological integration test to real unit tests that call the
helper directly.

## Acceptance Criteria

### AC-001 — Helper extracted as `pub(crate)`

`parsed_number_to_wire_value(parsed: f64) -> serde_json::Value` exists as a
`pub(crate)` function in `src/cli/issue/field_resolve.rs`.

The function signature must be exactly:
```rust
pub(crate) fn parsed_number_to_wire_value(parsed: f64) -> serde_json::Value
```

Verification: `grep -n "pub(crate) fn parsed_number_to_wire_value" src/cli/issue/field_resolve.rs`
returns exactly one match.

### AC-002 — Callsite replaced; behavior byte-identical

The inline conversion block at `field_resolve.rs` lines 286-292 is replaced with a single
call to `parsed_number_to_wire_value(parsed)`. The resulting `serde_json::Value` is
assigned to `wire_value`.

Production behavior is byte-identical: for a whole number in `[i64::MIN, i64::MAX]`,
`serde_json::Number::from(parsed as i64)` is emitted; for any other finite value,
`serde_json::json!(parsed)` is emitted. The predicate (`fract() == 0.0 && range check`)
is unchanged.

Verification: The replaced lines contain no direct `serde_json::Number::from` or
`serde_json::json!` usage — those live only inside the helper. The callsite is
`wire_value = parsed_number_to_wire_value(parsed);`.

### AC-003 — 6 inline unit tests in `field_resolve.rs::tests`

The `#[cfg(test)]` module in `src/cli/issue/field_resolve.rs` gains 6 new unit tests,
each calling `parsed_number_to_wire_value` directly:

| Test name | Input | Expected output |
|-----------|-------|-----------------|
| `test_parsed_number_to_wire_value_whole_integer` | `42.0` | `serde_json::Value::Number` with i64 value 42 |
| `test_parsed_number_to_wire_value_scientific_notation_whole` | `1e3` (i.e. `1000.0`) | `serde_json::Value::Number` with i64 value 1000 |
| `test_parsed_number_to_wire_value_fractional` | `3.14` | `serde_json::Value::Number` with f64 representation (not i64) |
| `test_parsed_number_to_wire_value_zero` | `0.0` | `serde_json::Value::Number` with i64 value 0 |
| `test_parsed_number_to_wire_value_negative_whole` | `-7.0` | `serde_json::Value::Number` with i64 value -7 |
| `test_parsed_number_to_wire_value_out_of_i64_range` | `1e19` (exceeds i64::MAX) | `serde_json::Value::Number` with f64 representation (not i64) |

All 6 tests use `assert_eq!` or `assert!(matches!(...))` against the actual helper return value —
no re-implementation of the predicate in test code.

### AC-004 — Test 38 removed; tombstone comment added

`test_bc_3_4_015_number_resolver_integer_is_i64_not_f64` (test 38, currently at
`tests/issue_edit_field.rs` lines 2425-2477) is removed from the integration test file.

A tombstone comment block is added at the removal site:

```rust
// test_bc_3_4_015_number_resolver_integer_is_i64_not_f64 — REMOVED (S-409)
// This test re-implemented the wire-value predicate from first principles (tautological).
// Coverage relocated to field_resolve.rs::tests (6 unit tests calling the actual helper)
// and confirmed end-to-end by integration tests 26/27:
//   test_bc_3_4_015_number_field_integer_wire_form
//   test_bc_3_4_015_number_field_scientific_notation_wire_form
// See: https://github.com/Zious11/jira-cli/issues/409
```

Verification: `grep -n "test_bc_3_4_015_number_resolver_integer_is_i64_not_f64" tests/issue_edit_field.rs`
returns zero function-definition matches (the tombstone comment line is acceptable).

### AC-005 — Integration tests 26/27 unchanged

`test_bc_3_4_015_number_field_integer_wire_form` (test 26) and
`test_bc_3_4_015_number_field_scientific_notation_wire_form` (test 27) in
`tests/issue_edit_field.rs` remain unchanged and continue to pin the wire-form contract
end-to-end via wiremock `NumericMode::Strict`.

Verification: Both test names exist and are unmodified after the S-409 changes.

### AC-006 — `cargo test` exits 0; suite count net +5

`cargo test` exits 0. Full-suite test count changes by approximately +5:
- 6 new unit tests added (in `field_resolve.rs::tests`)
- 1 integration test removed (test 38)
- Net: +5

Verification: `cargo test 2>&1 | tail -5` shows `test result: ok`.

### AC-007 — Lint and format clean

`cargo fmt --all -- --check` exits 0.
`cargo clippy --all-targets -- -D warnings` exits 0.

No `#[allow(...)]` attributes are added. If clippy warns on the extracted helper or its
call site, the code is refactored to fix the root cause.

## Implementation Strategy

This story requires a small, self-contained refactor. No new dependencies, no behavior
change, no public API change.

**Ordered sequence:**

1. **Create branch** `refactor/S-409-extract-number-wire-helper` from `develop`.

2. **Read `src/cli/issue/field_resolve.rs`** around lines 280-295 to confirm the exact
   current text of the inline conversion before editing. Do NOT use line numbers from
   this story as ground truth for the edit — verify against the actual file.

3. **Extract the helper** in `field_resolve.rs`:
   - Add `pub(crate) fn parsed_number_to_wire_value(parsed: f64) -> serde_json::Value`
     above or below the calling context (before the `resolve_edit_fields` function, or
     as a module-level free function — placement must satisfy Rust's forward-declaration
     rules; any position within the module is fine).
   - Move the predicate and both branches into the helper body.
   - Replace the original 7-line inline block with `wire_value = parsed_number_to_wire_value(parsed);`.

4. **Add 6 unit tests** in the existing `#[cfg(test)] mod tests` block at the bottom of
   `field_resolve.rs`. If no `mod tests` block exists, create one. Call
   `parsed_number_to_wire_value` directly in each test. Do NOT re-implement the predicate
   in test code.

5. **Edit `tests/issue_edit_field.rs`**:
   - Locate test 38 (`test_bc_3_4_015_number_resolver_integer_is_i64_not_f64`).
   - Delete the entire `#[tokio::test]` function.
   - Insert the tombstone comment block at the deletion site (AC-004).

6. **Run `cargo test`** — must exit 0.

7. **Run `cargo fmt --all -- --check`** — must exit 0.

8. **Run `cargo clippy --all-targets -- -D warnings`** — must exit 0.

9. **Run `bash scripts/check-spec-counts.sh`** — must exit 0 (no BC frontmatter changed).

10. **Run `bash scripts/check-bc-cumulative-counts.sh`** — must exit 0 (no count surfaces
    touched).

11. **Commit** with:
    `refactor(field_resolve): extract parsed_number_to_wire_value helper + replace tautological test 38 (closes #409)`

12. **Open PR** targeting `develop`.

## Out of Scope

- **Any change to the predicate logic** (`fract() == 0.0 && range check`). Production
  behavior must be byte-identical. If a correctness issue is discovered during
  implementation, stop and escalate — do not fix it in this story.
- **Other inline conversions in `field_resolve.rs`** outside the `"number"` branch.
- **Adding or modifying BCs, VPs, or holdouts.** This story touches no spec count surfaces.
- **Modifying integration tests 26/27.** They pin the end-to-end wire-form contract and
  must remain unchanged.

## Test Coverage Strategy

| Test type | Count | Location | What it tests |
|-----------|-------|----------|---------------|
| Unit tests (NEW) | 6 | `field_resolve.rs::tests` | Helper directly: whole, scientific-notation whole, fractional, zero, negative-whole, out-of-i64-range |
| Integration tests (UNCHANGED) | 2 | `tests/issue_edit_field.rs` (tests 26, 27) | End-to-end wire form via wiremock NumericMode::Strict |
| Tautological test (DELETED) | 1 | `tests/issue_edit_field.rs` (test 38) | Was re-implementing the predicate; no production code exercised |

Net suite delta: +5 (add 6, remove 1).

## Quality Gate Self-Check

| Criterion | Required | Notes |
|-----------|----------|-------|
| `cargo test` exits 0 | MUST | Full suite green |
| `cargo fmt --all -- --check` exits 0 | MUST | AC-007 |
| `cargo clippy --all-targets -- -D warnings` exits 0 | MUST | AC-007; no `#[allow]` additions |
| AC-004 tombstone grep returns 0 fn-definition matches | MUST | Verify test 38 removed |
| AC-005 tests 26/27 still present and unmodified | MUST | Regression pin |
| `bash scripts/check-spec-counts.sh` exits 0 | invariant | No BC frontmatter touched |
| `bash scripts/check-bc-cumulative-counts.sh` exits 0 | invariant | No count surfaces touched |
| `bash scripts/check-bc-no-numeric-test-counts.sh` exits 0 | invariant | No BC bodies with numeric test counts changed |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~3 k |
| `src/cli/issue/field_resolve.rs` (full file, 430 LOC) | ~6 k |
| `tests/issue_edit_field.rs` (scan for test 38 + tests 26/27 — targeted reads ~100 LOC each) | ~4 k |
| Tool outputs (cargo test, fmt, clippy, script exits) | ~2 k |
| **Total** | **~15 k** |

Well within a single-agent context window. No split required.
LOC delta: `field_resolve.rs` +~25 LOC (helper ~8 LOC + 6 unit tests ~17 LOC, inline block -7 → callsite +1);
`tests/issue_edit_field.rs` -~50 LOC net (delete ~53 LOC fn, add ~8 LOC tombstone comment).

## Tasks

- [ ] Create branch `refactor/S-409-extract-number-wire-helper` from `develop`
- [ ] Read `src/cli/issue/field_resolve.rs` lines 280-295 — verify exact text of inline conversion before editing
- [ ] Add `pub(crate) fn parsed_number_to_wire_value(parsed: f64) -> serde_json::Value` to `field_resolve.rs`
- [ ] Replace lines 286-292 callsite with `wire_value = parsed_number_to_wire_value(parsed);`
- [ ] Add 6 unit tests in `field_resolve.rs::tests` calling the helper directly (AC-003)
- [ ] Read `tests/issue_edit_field.rs` — locate test 38 (`test_bc_3_4_015_number_resolver_integer_is_i64_not_f64`)
- [ ] Delete test 38 function body; insert tombstone comment block (AC-004)
- [ ] Verify tests 26/27 (`test_bc_3_4_015_number_field_integer_wire_form`, `test_bc_3_4_015_number_field_scientific_notation_wire_form`) are unmodified (AC-005)
- [ ] Run `cargo test` — exits 0; net suite count +5 (AC-006)
- [ ] Run `cargo fmt --all -- --check` — exits 0 (AC-007)
- [ ] Run `cargo clippy --all-targets -- -D warnings` — exits 0 (AC-007)
- [ ] Run `bash scripts/check-spec-counts.sh` — exits 0
- [ ] Run `bash scripts/check-bc-cumulative-counts.sh` — exits 0
- [ ] Run `bash scripts/check-bc-no-numeric-test-counts.sh` — exits 0
- [ ] Commit with `refactor(field_resolve):` prefix; open PR targeting `develop`; `Closes #409`

## Previous Story Intelligence

Direct predecessor in the same area: S-396 (`issue edit --field NAME=VALUE` — arbitrary
custom field editing via editmeta). S-396 introduced `field_resolve.rs` and the `"number"`
branch that contains the inline conversion. The tautological test 38 was added during S-396
as an initial coverage placeholder; issue #409 was opened immediately after (Copilot R2-C4
finding) to replace it with a real unit test once the helper was extracted.

Key lesson from S-396: when a function's logic branch is non-trivial enough to warrant a
test, extract it as a named helper first so the test can call the actual code. Inline
anonymous logic blocks cannot be unit-tested without re-implementing them.

Key lesson from S-407 (`--label` conflict block coverage): tautological tests that
mirror production logic are worse than no test at all — they hide bugs by passing even
when the production code is wrong. The fix is always to test the production code directly,
not to test the test's own copy of the logic.

## Architecture Compliance Rules

1. **`pub(crate)` visibility only.** The extracted helper must be `pub(crate)`, not `pub`.
   It is an internal implementation detail of the `cli/issue` module — no external callers
   exist or are expected.

2. **No new module files.** The helper lives in `field_resolve.rs` alongside its only
   callsite. Do not create a `number_wire.rs` or similar.

3. **No behavior change.** The predicate (`fract() == 0.0 && parsed >= i64::MIN as f64
   && parsed <= i64::MAX as f64`) must be copied verbatim. If a clippy lint fires on the
   range-check expression (e.g., suggesting `as_f64()` or a different cast), fix the
   expression in a way that preserves identical semantics — do not use `#[allow]`.

4. **Unit tests must call the helper, not re-implement the predicate.** Any test that
   uses `fract()`, `i64::MIN`, or `i64::MAX` in its own logic is a tautological
   re-implementation and must be rewritten to use `assert_eq!` on the helper's output
   directly.

5. **Tombstone comment must reference both the new unit tests and the unchanged
   integration tests 26/27.** This ensures future readers understand the coverage chain
   without having to search.

## Library & Framework Requirements

No new dependencies. No version changes. The helper uses only `serde_json` (already a
direct dependency pinned in `Cargo.toml`).

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `src/cli/issue/field_resolve.rs` | Modify | Add `pub(crate)` helper + replace callsite + add 6 unit tests in `#[cfg(test)] mod tests` |
| `tests/issue_edit_field.rs` | Modify | Remove test 38 (~53 LOC); insert tombstone comment (~8 LOC) |

**Files NOT to create:** No new source files, no new spec files, no new test files.

**Files NOT to touch:** All other `src/` files, `Cargo.toml`, `deny.toml`, `STORY-INDEX.md`
(state-manager updates that), all BC count surfaces (frontmatter, BC-INDEX.md,
CANONICAL-COUNTS.md).

## Branch / PR Plan

- Branch: `refactor/S-409-extract-number-wire-helper`
- Target: `develop`
- Commit style: `refactor(field_resolve): extract parsed_number_to_wire_value helper + replace tautological test 38 (closes #409)`
- PR closes: `Closes #409`
- CHANGELOG entry: not required (internal refactor; no user-visible behavior change)

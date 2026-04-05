# Design: Add case-insensitive duplicate queue name integration test

**Issue:** [#131](https://github.com/Zious11/jira-cli/issues/131)
**Date:** 2026-04-03
**Status:** Draft

## Problem

The integration test `resolve_queue_duplicate_names_error_message` in `tests/queue.rs` (added in #129) mocks two queues both named `"Triage"` — identical casing. It exercises the `ExactMultiple` path in `partial_match` but does not exercise the case-insensitive `to_lowercase()` filter in `resolve_queue_by_name` at `src/cli/queue.rs:155-158`.

A developer could remove the `to_lowercase()` call and the existing test would still pass, since both mock queue names are already identical.

## Approach

Add one integration test with mixed-case queue names and a lowercase user input to exercise both sides of the `to_lowercase()` comparison.

Perplexity and Context7 searches found no authoritative documentation on whether JSM enforces case-insensitive queue name uniqueness. The `to_lowercase()` logic already exists in production as defensive code — this test ensures it stays working.

## Design

### Test: `resolve_queue_mixed_case_duplicate_names_error_message`

Added to `tests/queue.rs` alongside the existing same-casing test.

**Mock setup:** Two queues with different casing — `"Triage"` (ID `"30"`) and `"TRIAGE"` (ID `"40"`) — in service desk `"15"`.

**Call:** `resolve_queue_by_name("15", "triage", &client)` — lowercase input that matches neither stored name exactly, forcing both sides of `to_lowercase()` to do work.

**Assertions:**
1. The call returns an error (not a successful queue ID)
2. Error message contains `Multiple queues named` with the matched name
3. Error message contains both queue IDs (`30, 40`)
4. Error message contains the `--id` suggestion

This mirrors the structure of the existing `resolve_queue_duplicate_names_error_message` test but with mixed casing.

## What stays the same

- No production code changes — `resolve_queue_by_name` already handles this correctly
- Existing `resolve_queue_duplicate_names_error_message` test unchanged (covers same-casing path)
- `partial_match` module unchanged (has its own unit-level case-insensitive tests)
- `resolve_queue_by_name` visibility unchanged (`pub(crate)` since #129)

## Testing

One integration test added to `tests/queue.rs`. No other test changes.

## Files modified

- `tests/queue.rs` — append one test function (~30 lines)

# Evidence Report — issue-110-pr1

**Issue:** #110 — Multi-key bulk operations via Atlassian Bulk API (PR 1 of N)
**Branch:** `feat/issue-110-pr1-multi-key-bulk-api`
**Base SHA:** `b164c6b` (develop)
**Commits above base:**
- `15fc2a0` test-writer: 9 failing tests + OpenAPI-verified schema notes
- `fc9da51` impl: bulk module (api + types) + multi-key clap signature for edit/move
- `9868496` rustfmt fix

**Recorded:** 2026-05-09

---

## Demo Coverage

| Demo | File | Path | Criterion |
|------|------|------|-----------|
| D-001 | `D-001-bulk-tests-all-green` | [gif](D-001-bulk-tests-all-green.gif) / [webm](D-001-bulk-tests-all-green.webm) | 9 integration tests for bulk ops all pass |
| D-002 | `D-002-no-regression` | [gif](D-002-no-regression.gif) / [webm](D-002-no-regression.webm) | 612 lib unit tests all pass (no regression) |
| D-003 | `D-003-edit-help-multi-key` | [gif](D-003-edit-help-multi-key.gif) / [webm](D-003-edit-help-multi-key.webm) | `jr issue edit --help` shows `<KEYS>...` positional (multi-key) |
| D-004 | `D-004-move-help-with-to-flag` | [gif](D-004-move-help-with-to-flag.gif) / [webm](D-004-move-help-with-to-flag.webm) | `jr issue move --help` shows new `--to <STATUS>` flag |
| D-005 | `D-005-cap-1001-keys-exit-64` | [gif](D-005-cap-1001-keys-exit-64.gif) / [webm](D-005-cap-1001-keys-exit-64.webm) | 1001 keys → exit 64 + "Split into batches" hint, no HTTP call |

---

## Success Paths

| AC | Command | Expected | Demonstrated by |
|----|---------|----------|----------------|
| AC-001 | `jr issue edit KEY1 KEY2 KEY3 --label add:foo` | POST /bulk/issues/fields + poll → exit 0 | D-001 (test `test_edit_multi_key_issues_one_bulk_post_then_polls_to_complete`) |
| AC-002 | `jr issue move KEY1 KEY2 KEY3 --to Done` | POST /bulk/issues/transition + poll → exit 0 | D-001 (test `test_move_multi_key_issues_one_bulk_transition_post_then_polls`) |
| AC-003 | `--no-input` flag skips confirmation prompt | No hang, bulk POST fires | D-001 (test `test_edit_multi_key_with_no_input_skips_confirmation_prompt`) |
| AC-004 | Single-key edit routes via bulk API | exit 0, `--output json` returns `{key}` or `{results}` | D-001 (test `test_edit_single_key_routes_via_bulk_api_backward_compatible`) |
| AC-005 | `--label remove:X` sends REMOVE action | body contains "REMOVE" string | D-001 (test `test_edit_label_remove_sends_remove_action_in_bulk_payload`) |
| AC-006 | Multi-key `--output json` returns results array | `{results:[{key,status}]}` with correct count | D-001 (test `test_edit_multi_key_output_json_returns_results_array`) |
| CLI-003 | `jr issue edit --help` shows `<KEYS>...` positional | Help text shows Vec positional | D-003 |
| CLI-004 | `jr issue move --help` shows `--to <STATUS>` flag | Help text shows `--to` option | D-004 |

## Error Paths

| AC | Command | Expected | Demonstrated by |
|----|---------|----------|----------------|
| AC-007 | Partial failure (some keys fail) | exit 1 + per-key breakdown with error details | D-001 (test `test_edit_partial_failure_exits_one_with_per_key_breakdown`) |
| AC-008 | Poll receives HTTP 429 Retry-After | Retries after delay, exits 0 | D-001 (test `test_polling_respects_retry_after_on_429`) |
| Cap check | 1001 keys → exit 64 | "Too many issue keys: 1001 provided, maximum is 1000. Split into batches of 1000 or fewer." | D-005 |

---

## Reproduction Commands

```bash
# D-001: 9 bulk integration tests
cd .worktrees/issue-110-pr1
cargo test --test issue_bulk 2>&1 | tail -15

# D-002: lib unit tests (no regression)
cargo test --lib 2>&1 | tail -5

# D-003: edit help showing multi-key signature
./target/release/jr issue edit --help 2>&1 | head -20

# D-004: move help showing --to flag
./target/release/jr issue move --help 2>&1 | head -25

# D-005: cap enforcement (debug build, env override bypasses keychain)
export JR_BASE_URL=http://127.0.0.1:9
export JR_AUTH_HEADER="Basic dGVzdA=="
keys=(); for i in $(seq 1 1001); do keys+=("FOO-$i"); done
cargo run --quiet -- --no-input issue edit "${keys[@]}" --label add:foo
# → exit 64 + "Too many issue keys: 1001 provided, maximum is 1000."
```

---

## Schema Notes (API Contracts)

From `src/api/jira/bulk.rs` preamble and `tests/issue_bulk.rs` (OpenAPI-verified 2026-05-09):

### POST /rest/api/3/bulk/issues/fields
- **CONFIRMED:** `selectedIssueIdsOrKeys` (string array, up to 1,000)
- **BEST-GUESS:** `editedFieldsInput.labels.labelsAction` — casing unverified ("ADD"/"REMOVE" assumed). Empirical verification against live Jira recommended.
- **CONFIRMED:** HTTP 200 response with `taskId` in body (`BulkOperationProgress` shape)

### POST /rest/api/3/bulk/issues/transition
- **CONFIRMED:** `selectedIssueIdsOrKeys` + top-level `transitionId` (not nested)
- NOT `issueIds` (early secondary source, refuted)
- **CONFIRMED:** HTTP 200 with `BulkOperationProgress`

### GET /rest/api/3/bulk/queue/{taskId}
- **CONFIRMED status enum:** `ENQUEUED | RUNNING | COMPLETE | FAILED | CANCEL_REQUESTED | CANCELLED | DEAD`
- **NOTE:** Value is `"COMPLETE"` not `"COMPLETED"` (OpenAPI spec). Live API unverified — flagged for empirical check.
- Poll fields: `taskId`, `status`, `processedAccessibleIssues[]`, `failedAccessibleIssues{}`, `totalIssueCount`, `progressPercent`

---

## PR1 Scope Boundaries

The following are **known limitations in PR1** — tracked as PR2 scope:

1. **All-bulk routing for label edits (even single-key):** `jr issue edit FOO-1 --label add:foo` now routes through `POST /bulk/issues/fields` instead of the old single-key `PUT /issue/{key}`. This is a slight latency cost for single-key operations (async poll vs synchronous PUT). Behavior is correct; tradeoff accepted to unify code paths.

2. **Mixed ADD+REMOVE label payloads emit 2 bulk calls:** `--label add:foo --label remove:bar` currently sends one bulk POST for the ADD and one for the REMOVE (two round trips). PR2 candidate for single-call optimization.

3. **Multi-key non-label fields blocked:** `jr issue edit KEY1 KEY2 --summary "New title"` returns a "not yet supported" error. Only `--label` is bulk-enabled in PR1. PR2 scope: `--summary`, `--priority`, `--type`, `--parent`, `--points`, `--team`.

4. **`editedFieldsInput` label schema is best-guess:** The exact casing of `labelsAction` ("ADD" vs "add" vs "Add") and the nesting structure under `editedFieldsInput` is from community sources / partial OpenAPI extract. Empirical verification against live Jira Cloud is required before relying on this in production.

---

## Pre-existing Issues (Unrelated to PR1)

- **macOS keychain access dialog:** Running `jr` commands without `JR_AUTH_HEADER` in the debug build may trigger a macOS keychain access prompt. This is pre-existing behavior, not introduced by PR1. Demo D-005 uses `JR_AUTH_HEADER` env override to avoid this.
- **Release binary keychain-only:** `JR_AUTH_HEADER` is gated behind `#[cfg(debug_assertions)]` per SD-002 security policy. The release binary always loads from keychain. Demo D-003/D-004 use the release binary for help output (no network call needed); D-005 uses cargo run (debug build) for the cap test.

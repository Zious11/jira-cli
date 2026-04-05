# Handler-Level Tests for --to me Keyword and Idempotent Name Resolution

**Issue:** #148
**Date:** 2026-04-05

## Goal

Add handler-level integration tests to `tests/cli_handler.rs` covering three untested sub-paths in the assign and create command handlers:

1. `--to me` in assign (resolves via `get_myself()` through `resolve_assignee`)
2. `--to me` in create (resolves via `get_myself()` through `resolve_assignee_by_project`)
3. Idempotent assign when account ID comes from name search (not `--account-id`)

## Background

The existing handler-level tests (added in the PR for #139) cover the main branching logic but miss these sub-paths. Each exercises a distinct code path that could regress independently:

- `test_handler_assign_self` tests the **no-flag default** path: handler calls `get_myself()` directly. But `--to me` takes a **different path**: handler passes "me" to `resolve_assignee`, which checks `is_me_keyword`, then calls `get_myself()`. Same result, different code path.
- `test_handler_assign_idempotent` tests idempotency with `--account-id` (no user resolution). But idempotency with `--to Jane` exercises the search→resolve→idempotent-check sequence, which could fail if the resolved account ID doesn't match correctly.

## Tests

### Test 1: `test_handler_assign_to_me`

**Code path:** `handle_assign` → `--to` present → `resolve_assignee("me", "HDL-6")` → `is_me_keyword` returns true → `client.get_myself()` → idempotent check → `assign_issue` → `assign_changed_response`

**Mocks:**
- `GET /rest/api/3/myself` → `user_response()` (accountId "abc123", displayName "Test User")
- `GET /rest/api/3/issue/HDL-6` → unassigned issue
- `PUT /rest/api/3/issue/HDL-6/assignee` with `{"accountId": "abc123"}` → 204

**Command:** `issue assign HDL-6 --to me`

**Assertions:**
- Exit success
- `"changed": true`
- `"assignee": "Test User"`
- `"assignee_account_id": "abc123"`

### Test 2: `test_handler_create_to_me`

**Code path:** `handle_create` → `--to` present → `resolve_assignee_by_project("me", "HDL")` → `is_me_keyword` returns true → `client.get_myself()` → sets `fields["assignee"]` → POST create

**Mocks:**
- `GET /rest/api/3/myself` → `user_response()` (accountId "abc123")
- `POST /rest/api/3/issue` with `body_partial_json` verifying `fields.assignee.accountId == "abc123"` → 201 with `create_issue_response("HDL-200")`

**Command:** `issue create -p HDL -t Task -s "Created with --to me" --to me`

**Assertions:**
- Exit success
- `"key": "HDL-200"` in output

### Test 3: `test_handler_assign_idempotent_with_name_search`

**Code path:** `handle_assign` → `--to` present → `resolve_assignee("Jane", "HDL-7")` → search → single result (acc-jane-456) → idempotent check → issue already assigned to acc-jane-456 → early return with `changed: false`

**Mocks:**
- `GET /rest/api/3/user/assignable/search` with `query=Jane`, `issueKey=HDL-7` → single result `[{accountId: "acc-jane-456", displayName: "Jane Doe"}]`
- `GET /rest/api/3/issue/HDL-7` → assigned to `("acc-jane-456", "Jane Doe")`
- `PUT /rest/api/3/issue/HDL-7/assignee` with `.expect(0)` — must NOT be called

**Command:** `issue assign HDL-7 --to Jane`

**Assertions:**
- Exit success
- `"changed": false`

## Fixtures

No new fixtures needed. All tests reuse existing helpers:
- `user_response()` — for `/myself` endpoint
- `issue_response_with_assignee(key, summary, assignee_opt)` — for issue GET
- `user_search_response(vec![...])` — for assignable search
- `create_issue_response(key)` — for create POST

## Issue Keys

Continue the existing HDL sequence: HDL-6, HDL-7, HDL-200 (200 for create to avoid confusion with assign keys).

## Out of Scope

- Negative assertions on bypassed search endpoints for `--to me` tests (Perplexity validated: over-testing implementation details)
- Testing `is_me_keyword` unit behavior (already covered by unit tests in `helpers.rs`)
- Create command idempotency (create always creates a new issue — no idempotent check exists)

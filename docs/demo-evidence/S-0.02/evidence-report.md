# Demo Evidence Report — S-0.02

**Story:** Paginate `list_worklogs` to prevent silent truncation
**Story ID:** S-0.02
**Branch:** fix/paginate-list-worklogs
**Date recorded:** 2026-05-07
**Recording medium:** VHS (terminal recordings — CLI product)
**Font:** Menlo (system default on macOS)

---

## Coverage Summary

| AC | Description | Test | Recording | Result |
|----|-------------|------|-----------|--------|
| AC-001 | Two-page result returns all 80 items — no silent truncation | `test_bc_x_5_002_two_page_result_returns_all_80_items` | AC-001-two-page-result.gif | PASS |
| AC-002 | Both pages fetched — wiremock expect(2) satisfied | `test_bc_x_5_002_both_pages_fetched` | AC-002-both-pages-fetched.gif | PASS |
| AC-003 | Single-page no extra fetch — regression guard | `test_bc_x_5_002_single_page_no_extra_fetch` | AC-003-single-page-no-extra-fetch.gif | PASS |
| AC-004 | Empty issue returns zero items — loop-termination guard | `test_bc_x_5_002_empty_issue_returns_zero_items` | AC-004-empty-issue-zero-items.gif | PASS |
| Combined | All 4 new ACs + 5 existing worklog_commands tests pass | all 9 | AC-combined-all-four-pass.gif | 9/9 PASS |

---

## AC-001: Two-Page Result Returns All 80 Items

**Acceptance Criterion:** Given wiremock returns page 1 with 50 worklogs (`total: 80, startAt: 0,
maxResults: 50`) and page 2 with 30 worklogs (`total: 80, startAt: 50, maxResults: 50`), when
`jr worklog list PROJ-1 --output json` is executed, the JSON array length is 80. Fixes silent
truncation at 50 items in the prior implementation.

**Test:** `test_bc_x_5_002_two_page_result_returns_all_80_items` in `tests/worklog_commands.rs`

**Recordings:**
- `AC-001-two-page-result.gif` (134 KB)
- `AC-001-two-page-result.webm` (354 KB)
- `AC-001-two-page-result.tape` (VHS script source)

**Evidence (captured test output):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/worklog_commands.rs (target/debug/deps/worklog_commands-59d710c1dadd39c9)

running 1 test
test test_bc_x_5_002_two_page_result_returns_all_80_items ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.00s
```

---

## AC-002: Both Pages Fetched

**Acceptance Criterion:** Both pages are fetched from the server. The wiremock mock uses
`expect(2)` for the worklog endpoint, verifying that `list_worklogs` issues exactly two HTTP
requests for an 80-item issue. (Error path: wiremock panics if expectation is not met.)

**Test:** `test_bc_x_5_002_both_pages_fetched` in `tests/worklog_commands.rs`

**Recordings:**
- `AC-002-both-pages-fetched.gif` (122 KB)
- `AC-002-both-pages-fetched.webm` (288 KB)
- `AC-002-both-pages-fetched.tape` (VHS script source)

**Evidence (captured test output):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/worklog_commands.rs (target/debug/deps/worklog_commands-59d710c1dadd39c9)

running 1 test
test test_bc_x_5_002_both_pages_fetched ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.00s
```

---

## AC-003: Single-Page No Extra Fetch

**Acceptance Criterion:** For a single-page result (`total=20`, one page), only one HTTP request
is made. Regression guard: the pagination loop must terminate immediately when all items fit in
the first page, not issue a spurious second-page request.

**Test:** `test_bc_x_5_002_single_page_no_extra_fetch` in `tests/worklog_commands.rs`

**Recordings:**
- `AC-003-single-page-no-extra-fetch.gif` (129 KB)
- `AC-003-single-page-no-extra-fetch.webm` (343 KB)
- `AC-003-single-page-no-extra-fetch.tape` (VHS script source)

**Evidence (captured test output):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/worklog_commands.rs (target/debug/deps/worklog_commands-59d710c1dadd39c9)

running 1 test
test test_bc_x_5_002_single_page_no_extra_fetch ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.00s
```

---

## AC-004: Empty Issue Returns Zero Items

**Acceptance Criterion:** For an empty issue (`total=0`), zero items are returned with no HTTP
errors. Loop-termination guard: the pagination loop handles the empty case cleanly — it must not
loop infinitely or panic when the server returns an empty worklog list.

**Test:** `test_bc_x_5_002_empty_issue_returns_zero_items` in `tests/worklog_commands.rs`

**Recordings:**
- `AC-004-empty-issue-zero-items.gif` (124 KB)
- `AC-004-empty-issue-zero-items.webm` (340 KB)
- `AC-004-empty-issue-zero-items.tape` (VHS script source)

**Evidence (captured test output):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/worklog_commands.rs (target/debug/deps/worklog_commands-59d710c1dadd39c9)

running 1 test
test test_bc_x_5_002_empty_issue_returns_zero_items ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.00s
```

---

## Combined: All Four ACs Pass

**Recordings:**
- `AC-combined-all-four-pass.gif` (173 KB)
- `AC-combined-all-four-pass.webm` (856 KB)
- `AC-combined-all-four-pass.tape` (VHS script source)

**Evidence (captured test output):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/worklog_commands.rs (target/debug/deps/worklog_commands-59d710c1dadd39c9)

running 9 tests
test test_add_worklog ... ok
test test_bc_x_5_002_both_pages_fetched ... ok
test test_bc_x_5_002_empty_issue_returns_zero_items ... ok
test test_bc_x_5_002_single_page_no_extra_fetch ... ok
test test_bc_x_5_002_two_page_result_returns_all_80_items ... ok
test test_list_worklogs ... ok
test worklog_list_network_drop_surfaces_reach_error ... ok
test worklog_list_server_error_surfaces_friendly_message ... ok
test worklog_list_unauthorized_dispatches_reauth_message ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s
```

---

## Quality Gates

All gates verified clean after demo recordings (no source changes):

| Gate | Result |
|------|--------|
| `cargo build` | clean |
| `cargo test` | green (9 passed, 0 failed) |
| `cargo clippy -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |

---

## Traceability

| Recording | AC | BC Anchor | H-045 Outcome |
|-----------|----|-----------|---------------|
| AC-001 | AC-001 | BC-X.5.002 | MUST-PASS (was MUST-FAIL at dea1664) |
| AC-002 | AC-002 | BC-X.5.002 | MUST-PASS |
| AC-003 | AC-003 | BC-X.5.002 | MUST-PASS (regression guard) |
| AC-004 | AC-004 | BC-X.5.002 | MUST-PASS (loop-termination guard) |

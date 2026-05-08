# Demo Evidence Report — S-2.01

**Story:** BC-2 Issue-Read Regression Holdout Suite
**Branch:** `test/bc-2-issue-read-holdout-suite`
**Commit at recording:** `8ef3e55`
**Recorded:** 2026-05-07
**Tool:** VHS 0.11.0

---

## Coverage Summary

| AC | Holdout IDs | BC Contract | Tests | Result | Recording |
|----|-------------|-------------|-------|--------|-----------|
| AC-001 | H-030 | BC-7.3.001 | 1 | PASS | [AC-001-400-empty-body-sentinel](#ac-001) |
| AC-002 | H-031 | BC-X.2.005 | 1 | PASS | [AC-002-short-page-advances-by-page-size](#ac-002) |
| AC-003 | H-032 | BC-X.2.006 | 1 | PASS | [AC-003-safety-cap-1500-users](#ac-003) |
| AC-004 | H-033 | BC-3.7.004 | 1 | PASS | [AC-004-ftp-url-rejected-before-network](#ac-004) |
| AC-005 | H-034 | BC-3.7.001 | 1 | PASS | [AC-005-bare-host-url-normalized](#ac-005) |
| AC-006 | H-035 | BC-2.1.001 | 1 | PASS | [AC-006-all-filters-combined-no-panic](#ac-006) |
| AC-007 | H-021 | BC-2.1.007 | 1 | PASS | [AC-007-ambiguous-status-exits-64](#ac-007) |
| **Combined** | All | All | **7** | **7/7 PASS** | [COMBINED-all-7-tests-green](#combined) |

---

## AC-001: 400 + Empty Body — Sentinel in stderr (H-030) {#ac-001}

**Pins:** When a Jira API endpoint returns HTTP 400 with an empty response body,
`extract_error_message` produces the literal sentinel `"<empty response body>"` and the binary
writes it to stderr. Guards `src/api/client.rs::extract_error_message` branch 5 (empty body).

**Test function:**
- `test_s_2_01_h_030_bc_7_3_001_400_empty_body_shows_sentinel_in_stderr`

**Command recorded:**
```
cargo test --test issue_read_holdouts test_s_2_01_h_030 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-001-400-empty-body-sentinel.gif`
- `AC-001-400-empty-body-sentinel.webm`
- `AC-001-400-empty-body-sentinel.tape`

---

## AC-002: Short Page Advances by PAGE_SIZE Not Returned Count (H-031) {#ac-002}

**Pins:** When page 2 of a user search returns 35 users (fewer than PAGE_SIZE=100),
`search_users_all` advances `startAt` by PAGE_SIZE (100), not by 35. Page 3 starts at 200,
total = 235 (100 + 35 + 100). Guards JRACLOUD-71293 fixed-window pagination contract in
`src/api/jira/users.rs::search_users_all`.

**Test function:**
- `test_s_2_01_h_031_bc_x_2_005_short_page_advances_by_page_size_not_returned_count`

**Command recorded:**
```
cargo test --test issue_read_holdouts test_s_2_01_h_031 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-002-short-page-advances-by-page-size.gif`
- `AC-002-short-page-advances-by-page-size.webm`
- `AC-002-short-page-advances-by-page-size.tape`

---

## AC-003: Safety Cap Fires at 1500 Users With Warning (H-032) {#ac-003}

**Pins:** When the user-search endpoint never returns an empty page (infinite mock),
`search_users_all` terminates after exactly USER_PAGINATION_SAFETY_CAP=15 iterations, returns
1500 users (15 × PAGE_SIZE=100), and emits a stderr warning. Guards `USER_PAGINATION_SAFETY_CAP`
in `src/api/jira/users.rs`. Wiremock `.expect(15)` confirms exact loop count.

**Test function:**
- `test_s_2_01_h_032_bc_x_2_006_safety_cap_fires_at_1500_with_warning`

**Command recorded:**
```
cargo test --test issue_read_holdouts test_s_2_01_h_032 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed (safety cap warning visible in --nocapture output)

**Recordings:**
- `AC-003-safety-cap-1500-users.gif`
- `AC-003-safety-cap-1500-users.webm`
- `AC-003-safety-cap-1500-users.tape`

---

## AC-004: ftp:// URL Rejected Before Network — Exit 64 (H-033) {#ac-004}

**Pins:** `jr issue remote-link --url ftp://example.com` exits 64 (UserError), includes both
"http or https" and "ftp" in stderr, and makes ZERO HTTP calls. Uses an unreachable base URL
(127.0.0.1:1) so any network dial fails with connection-refused (exit ≠ 64), making zero-HTTP
implicit in the exit-code assertion. Guards scheme validation gate in
`src/cli/issue/links.rs::handle_remote_link`.

**Test function:**
- `test_s_2_01_h_033_bc_3_7_004_ftp_url_rejected_before_network_with_exit_64`

**Command recorded:**
```
cargo test --test issue_read_holdouts test_s_2_01_h_033 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-004-ftp-url-rejected-before-network.gif`
- `AC-004-ftp-url-rejected-before-network.webm`
- `AC-004-ftp-url-rejected-before-network.tape`

---

## AC-005: Bare Host URL Normalized With Trailing Slash (H-034) {#ac-005}

**Pins:** `jr issue remote-link --url https://example.com` POSTs to Jira remotelink with the
url field set to `https://example.com/` (trailing slash added by `url::Url::parse`). Mock uses
`body_partial_json` so a non-normalized URL causes a 404, failing at the exit-code assertion.
Guards url normalization contract in `src/cli/issue/links.rs::handle_remote_link`.

**Test function:**
- `test_s_2_01_h_034_bc_3_7_001_bare_host_url_normalized_with_trailing_slash`

**Command recorded:**
```
cargo test --test issue_read_holdouts test_s_2_01_h_034 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-005-bare-host-url-normalized.gif`
- `AC-005-bare-host-url-normalized.webm`
- `AC-005-bare-host-url-normalized.tape`

---

## AC-006: All Filters Combined Compose JQL Without Panic (H-035) {#ac-006}

**Pins:** `--assignee`, `--created-after`, `--status`, and `--team` may be combined in a
single `jr issue list` invocation. The handler composes them into a single JQL query without
panicking or returning an error. Returns a 5-element JSON array. Guards filter composition path
in `src/cli/issue/list.rs::handle_list`. Team resolved via pre-populated cache (no GraphQL call).

**Note:** `--open` and `--status` are `conflicts_with` at the clap layer; this test uses
`--status "In Progress"` instead. The BC-2.1.001 contract is about handler-level composition.

**Test function:**
- `test_s_2_01_h_035_bc_2_1_001_all_filters_combined_compose_jql_without_panic`

**Command recorded:**
```
cargo test --test issue_read_holdouts test_s_2_01_h_035 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-006-all-filters-combined-no-panic.gif`
- `AC-006-all-filters-combined-no-panic.webm`
- `AC-006-all-filters-combined-no-panic.tape`

---

## AC-007: Ambiguous Status Exits 64 — JQL Not Invoked (H-021) {#ac-007}

**Pins:** `jr issue list --status prog` exits 64 (UserError) and includes "Ambiguous status"
in stderr when the substring "prog" matches multiple status candidates. JQL search endpoint must
NOT be invoked (wiremock `.expect(0)` enforces this). Guards `partial_match` disambiguation in
`src/partial_match.rs` and `src/cli/issue/list.rs`. Anchors the pre-existing coverage in
`tests/issue_list_errors.rs:369` with a named holdout following the `test_s_2_01_*` convention.

**Test function:**
- `test_s_2_01_h_021_bc_2_1_007_ambiguous_status_exits_64_no_jql_call`

**Command recorded:**
```
cargo test --test issue_read_holdouts test_s_2_01_h_021 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-007-ambiguous-status-exits-64.gif`
- `AC-007-ambiguous-status-exits-64.webm`
- `AC-007-ambiguous-status-exits-64.tape`

---

## Combined: Full Suite 7/7 Green {#combined}

**Command recorded:**
```
cargo test --test issue_read_holdouts -- --nocapture --test-threads=1 2>&1
```

**Result:** 7 passed; 0 failed; 0 ignored; finished in ~0.78s

**Recordings:**
- `COMBINED-all-7-tests-green.gif`
- `COMBINED-all-7-tests-green.webm`
- `COMBINED-all-7-tests-green.tape`

---

## Quality Gates

| Gate | Status |
|------|--------|
| `cargo build` | CLEAN |
| `cargo test --test issue_read_holdouts` | 7/7 PASS |
| `cargo clippy -- -D warnings` | CLEAN (0 warnings) |
| `cargo fmt --all -- --check` | CLEAN |

# Evidence Report — S-2.06: worklog timeSpent server-side parsing (NFR-R-C) and CMDB cache tuple format pin

**Story:** S-2.06 v2.0.0
**Branch:** `feat/S-2.06-worklog-time-spent-passthrough`
**Test file:** `tests/worklog_duration_holdouts.rs`
**Activation HEAD:** `b3d2500` / `15f509c`
**Evidence recorded:** 2026-05-08
**VHS available:** Yes (`/opt/homebrew/bin/vhs`)

---

## Behavior Delta — Pre vs Post S-2.06

**Before S-2.06 (old behavior):**
`src/cli/worklog.rs::handle_add` called `duration::parse_duration(dur, 8, 5)` — a
client-side calculator that converted duration strings to seconds using hardcoded
constants of 8 hours/day and 5 days/week. The result was sent to the Jira API as
`"timeSpentSeconds": <integer>`. For example, `"1d"` produced `{"timeSpentSeconds": 28800}`.
This was silently wrong on any Jira instance with non-standard working hours (e.g., 7.5h/day
or 4-day weeks).

**After S-2.06 (new behavior):**
`parse_duration` is refactored to `parse_duration_validate` — a pure syntactic validator
that confirms the input matches Jira's accepted duration format (`Nw Nd Nh Nm`) and rejects
garbage before any network call. When input is valid, the user's original string is forwarded
verbatim to the worklog POST as `"timeSpent": "<string>"`. Jira parses the string server-side
using the instance's own configured `workingHoursPerDay` and `workingDaysPerWeek`. No client-side
arithmetic, no admin-permission dependency, no hardcoded constants.

**Verification source:**
A Perplexity verification pass on 2026-05-08 (`.factory/research/S-2.06-jira-timetracking-verification.md`)
confirmed three blocking errors in the v1.0.0 approach: (1) the `GET /rest/api/3/configuration/timetracking`
endpoint returns the active time-tracking provider name, not working-hours config — the correct
endpoint is `/rest/api/3/configuration/timetracking/options`; (2) the documented field names are
`workingHoursPerDay`/`workingDaysPerWeek` (floats), not the originally-assumed `hoursPerDay`/`daysPerWeek`
(integers); (3) both endpoints require "Administer Jira" global permission — typical `jr` users
receive 403 regardless of OAuth scope. The chosen resolution matches the `ankitpokhrel/jira-cli`
(canonical Go Jira CLI) pattern of forwarding the user's duration string directly as `timeSpent`.

---

## Coverage Summary

| AC | BC Anchor | Test Name | Status | Evidence Type |
|----|-----------|-----------|--------|---------------|
| AC-001 | BC-X.5.009 | `test_s_2_06_ac_001_bc_x_5_009_worklog_post_body_contains_timespent_string` | PASS | Transcript |
| AC-002 | BC-X.5.009 | `test_s_2_06_ac_002_bc_x_5_009_worklog_post_preserves_complex_string` | PASS | Transcript |
| AC-003 | BC-X.5.009 | `test_s_2_06_ac_003_bc_x_5_009_invalid_duration_rejected_before_network` | PASS | Transcript + VHS |
| AC-004 | BC-X.5.009 | `test_s_2_06_ac_004_bc_x_5_009_parse_duration_validator_unit` | PASS | Transcript |
| AC-005 | BC-6.2.013 | `test_s_2_06_ac_005_bc_6_2_013_legacy_id_only_cmdb_cache_graceful_miss` | PASS | Transcript |
| AC-006 | BC-6.2.013 | `test_s_2_06_ac_006_bc_6_2_013_valid_tuple_cache_hits_no_api_call` | PASS | Transcript |

---

## AC-001 / BC-X.5.009 — POST body contains `timeSpent` string, not `timeSpentSeconds`

**What this implements:** The worklog POST body must contain `"timeSpent": "1d"` (string field) and
must NOT contain `timeSpentSeconds`. This pins the API field name change in
`src/api/jira/worklogs.rs::add_worklog` from the old `{"timeSpentSeconds": <integer>}` to the
new `{"timeSpent": "<string>"}` passthrough.

**Test:** `test_s_2_06_ac_001_bc_x_5_009_worklog_post_body_contains_timespent_string`

**Verification command:**
```
cargo test --test worklog_duration_holdouts test_s_2_06_ac_001_bc_x_5_009_worklog_post_body_contains_timespent_string -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/worklog_duration_holdouts.rs (target/debug/deps/worklog_duration_holdouts-925c4740017d798c)

running 1 test
test test_s_2_06_ac_001_bc_x_5_009_worklog_post_body_contains_timespent_string ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.71s
```

---

## AC-002 / BC-X.5.009 — complex duration string preserved verbatim in POST body

**What this implements:** When the user passes `"2d 3h 30m"`, the POST body contains
`"timeSpent": "2d 3h 30m"` — spaces and mixed units preserved exactly as entered. This pins
the verbatim passthrough in `src/api/jira/worklogs.rs::add_worklog` and the multi-token
parsing acceptance in `parse_duration_validate`.

**Test:** `test_s_2_06_ac_002_bc_x_5_009_worklog_post_preserves_complex_string`

**Verification command:**
```
cargo test --test worklog_duration_holdouts test_s_2_06_ac_002_bc_x_5_009_worklog_post_preserves_complex_string -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/worklog_duration_holdouts.rs (target/debug/deps/worklog_duration_holdouts-925c4740017d798c)

running 1 test
test test_s_2_06_ac_002_bc_x_5_009_worklog_post_preserves_complex_string ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.68s
```

---

## AC-003 / BC-X.5.009 — invalid duration rejected before any network call

**What this implements:** When the user passes an invalid duration (`"1z"` — unknown unit `z`),
the command exits with code 64, stderr contains the syntax hint `Nw Nd Nh Nm`, and zero POST
requests are made to the worklog endpoint. This pins the early-exit validator in
`src/cli/worklog.rs::handle_add` and the `parse_duration_validate` error message format.

**Test:** `test_s_2_06_ac_003_bc_x_5_009_invalid_duration_rejected_before_network`

**Verification command:**
```
cargo test --test worklog_duration_holdouts test_s_2_06_ac_003_bc_x_5_009_invalid_duration_rejected_before_network -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/worklog_duration_holdouts.rs (target/debug/deps/worklog_duration_holdouts-925c4740017d798c)

running 1 test
test test_s_2_06_ac_003_bc_x_5_009_invalid_duration_rejected_before_network ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.68s
```

**VHS recording:**
- `AC-003-invalid-duration-rejected.tape` — VHS script source
- `AC-003-invalid-duration-rejected.gif` — terminal recording (191 KB)
- `AC-003-invalid-duration-rejected.webm` — archival recording (446 KB)

The recording runs the AC-003 test via `cargo test --test worklog_duration_holdouts ... --nocapture`,
demonstrating the invalid-duration rejection path with exit 64 and syntax hint.

---

## AC-004 / BC-X.5.009 — parse_duration_validate unit contract

**What this implements:** The refactored `parse_duration_validate(input: &str) -> Result<()>`
function (no `hours_per_day` / `days_per_week` parameters) accepts valid Jira duration strings
(`1d`, `2h`, `30m`, `1w`, `1w 2d 3h 4m`, `1w2d3h30m`) and rejects invalid ones (`"1z"`, `""`,
`"30"` (no unit), `"   "` (whitespace-only)). This pins the pure syntactic validator contract
in `src/duration.rs` — no calculator arithmetic, no hardcoded hour/day constants.

**Test:** `test_s_2_06_ac_004_bc_x_5_009_parse_duration_validator_unit`

**Verification command:**
```
cargo test --test worklog_duration_holdouts test_s_2_06_ac_004_bc_x_5_009_parse_duration_validator_unit -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/worklog_duration_holdouts.rs (target/debug/deps/worklog_duration_holdouts-925c4740017d798c)

running 1 test
test test_s_2_06_ac_004_bc_x_5_009_parse_duration_validator_unit ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s
```

---

## AC-005 / BC-6.2.013 — legacy ID-only CMDB cache format produces graceful miss

**What this pins:** When `cmdb_fields.json` contains the legacy ID-only format
`["customfield_10191"]` (a bare JSON array of strings, not the current `CmdbFieldsCache`
struct shape), `read_cmdb_fields_cache("default")` returns `Ok(None)` rather than panicking.
The stderr warning `"cache file cmdb_fields.json unreadable (invalid type: string "customfield_10191",
expected a sequence at line 1 column 20); will refetch"` is emitted (captured in --nocapture output
below). This pins the graceful-degradation path in `src/cache.rs::read_cache` so a future
"simplification" that replaces the match on deserialization failure with an unwrap would break
this test.

**Test:** `test_s_2_06_ac_005_bc_6_2_013_legacy_id_only_cmdb_cache_graceful_miss`

**Verification command:**
```
cargo test --test worklog_duration_holdouts test_s_2_06_ac_005_bc_6_2_013_legacy_id_only_cmdb_cache_graceful_miss -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/worklog_duration_holdouts.rs (target/debug/deps/worklog_duration_holdouts-925c4740017d798c)

running 1 test
warning: cache file cmdb_fields.json unreadable (invalid type: string "customfield_10191", expected a sequence at line 1 column 20); will refetch
test test_s_2_06_ac_005_bc_6_2_013_legacy_id_only_cmdb_cache_graceful_miss ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s
```

Note: the `warning:` line is the expected stderr output from `read_cache`'s graceful-degradation
path — it is the observable proof that the code hit the mismatch branch and logged it rather
than panicking.

---

## AC-006 / BC-6.2.013 — valid tuple-format CMDB cache hits without API call

**What this pins:** When `cmdb_fields.json` contains the current `CmdbFieldsCache` tuple
format (`Vec<(String, String)>`), `read_cmdb_fields_cache` returns `Ok(Some(cache))` with
the correct data. The "no API call" aspect is verified implicitly — no mock server is started
in this test, so any attempt to fetch a real network endpoint would fail with a connection
error. This pins the forward-compatible happy path in `src/cache.rs` so the tuple format
is not accidentally broken by future refactors (e.g., changing `(String, String)` to a
named struct without updating serde).

**Test:** `test_s_2_06_ac_006_bc_6_2_013_valid_tuple_cache_hits_no_api_call`

**Verification command:**
```
cargo test --test worklog_duration_holdouts test_s_2_06_ac_006_bc_6_2_013_valid_tuple_cache_hits_no_api_call -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/worklog_duration_holdouts.rs (target/debug/deps/worklog_duration_holdouts-925c4740017d798c)

running 1 test
test test_s_2_06_ac_006_bc_6_2_013_valid_tuple_cache_hits_no_api_call ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s
```

---

## Combined Run

**Verification command:**
```
cargo test --test worklog_duration_holdouts
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/worklog_duration_holdouts.rs (target/debug/deps/worklog_duration_holdouts-925c4740017d798c)

running 6 tests
test test_s_2_06_ac_004_bc_x_5_009_parse_duration_validator_unit ... ok
test test_s_2_06_ac_005_bc_6_2_013_legacy_id_only_cmdb_cache_graceful_miss ... ok
test test_s_2_06_ac_006_bc_6_2_013_valid_tuple_cache_hits_no_api_call ... ok
test test_s_2_06_ac_003_bc_x_5_009_invalid_duration_rejected_before_network ... ok
test test_s_2_06_ac_002_bc_x_5_009_worklog_post_preserves_complex_string ... ok
test test_s_2_06_ac_001_bc_x_5_009_worklog_post_body_contains_timespent_string ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.76s
```

Full transcript also available at: `docs/demo-evidence/S-2.06/combined-transcript.txt`

---

## Files in This Directory

| File | Description |
|------|-------------|
| `evidence-report.md` | This report — AC coverage, behavior delta, transcripts, rationale |
| `combined-transcript.txt` | Verbatim `cargo test --test worklog_duration_holdouts` output |
| `AC-003-invalid-duration-rejected.tape` | VHS script for AC-003 recording |
| `AC-003-invalid-duration-rejected.gif` | VHS-generated terminal recording (191 KB, PR embed) |
| `AC-003-invalid-duration-rejected.webm` | VHS-generated terminal recording (446 KB, archival) |

Note: AC-001, AC-002, AC-004, AC-005, and AC-006 use transcript-only evidence. These are
wiremock integration tests and library-level unit tests whose primary observable is test
pass/fail, not interactive CLI surface. AC-003 (user-facing stderr error with syntax hint
and non-zero exit code) is additionally evidenced with a VHS recording.

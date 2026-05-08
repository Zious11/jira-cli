# Evidence Report — S-2.04: BC-5/7 Boards, Sprints, and ADF Rendering Holdout Suite

**Story:** S-2.04
**Branch:** `test/S-2.04-bc-5-boards-sprints-holdout-suite`
**Test file:** `tests/boards_sprints_holdouts.rs`
**Activation HEAD:** `e9c2ba8`
**Evidence recorded:** 2026-05-08
**VHS available:** Yes (`/opt/homebrew/bin/vhs`)

---

## Coverage Summary

| AC | Holdout | BC Anchor | Test Name | Status | Evidence Type |
|----|---------|-----------|-----------|--------|---------------|
| AC-001a | H-040 | BC-5.2.005 | `test_s_2_04_h_040_bc_5_2_005_truncates_to_30_with_hint_when_35_issues` | PASS | Transcript |
| AC-001b | H-040 | BC-5.2.005 | `test_s_2_04_h_040_bc_5_2_005_all_flag_shows_all_35_no_hint` | PASS | Transcript |
| AC-001c | H-040 | BC-5.2.005 | `test_s_2_04_h_040_bc_5_2_005_under_limit_shows_all_no_hint` | PASS | Transcript |
| AC-002 | H-041 | BC-5.2.007 | `test_s_2_04_h_041_bc_5_2_007_sprint_add_json_has_sprint_id` | PASS | Transcript |
| AC-003 | H-041 | BC-5.2.008 | `test_s_2_04_h_041_bc_5_2_008_sprint_remove_json_has_no_sprint_id` | PASS | Transcript |
| AC-004 | H-042 | BC-5.2.001 | `test_s_2_04_h_042_bc_5_2_001_kanban_board_errors_on_sprint_list` | PASS | Transcript + VHS |
| AC-005 | H-043 | BC-5.3.001 | `test_s_2_04_h_043_bc_5_3_001_team_column_present_when_field_and_uuid_set` | PASS | Transcript |
| AC-006 | H-043 | BC-5.3.002 | `test_s_2_04_h_043_bc_5_3_002_team_column_absent_when_no_uuid_set` | PASS | Transcript |
| AC-007 | H-044 | BC-7.2.001 | `test_s_2_04_h_044_bc_7_2_001_adf_renders_heading_paragraph_drops_mention` | PASS | Transcript + VHS |

---

## AC-001a / H-040 / BC-5.2.005 — sprint current truncates to 30 with hint when 35 issues

**Behavioral contract:** `sprint current` shows at most 30 issues by default. When the
sprint has >30 issues, stderr contains a truncation hint ("Showing 30 results" or "~").

**Test:** `test_s_2_04_h_040_bc_5_2_005_truncates_to_30_with_hint_when_35_issues`

**Verification command:**
```
cargo test --test boards_sprints_holdouts test_s_2_04_h_040_bc_5_2_005_truncates_to_30_with_hint_when_35_issues -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/boards_sprints_holdouts.rs (target/debug/deps/boards_sprints_holdouts-8142f97468925142)

running 1 test
test test_s_2_04_h_040_bc_5_2_005_truncates_to_30_with_hint_when_35_issues ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.69s
```

**What behavior this pins:** The 30-issue truncation ceiling in `src/cli/sprint.rs` — both
the row count limit and the stderr hint emission. Raising the ceiling silently (or
removing the hint) would break this holdout.

**Why it passes against unmodified code:** The wiremock sprint endpoint returns 35 issue
keys; the code truncates the display list to 30 and emits the hint to stderr. The test
asserts `stdout` contains exactly 30 rows and `stderr` contains the hint substring.

---

## AC-001b / H-040 / BC-5.2.005 — sprint current --all shows all 35, no hint

**Behavioral contract:** When `--all` is passed, all issues are shown and no truncation
hint appears in stderr.

**Test:** `test_s_2_04_h_040_bc_5_2_005_all_flag_shows_all_35_no_hint`

**Verification command:**
```
cargo test --test boards_sprints_holdouts test_s_2_04_h_040_bc_5_2_005_all_flag_shows_all_35_no_hint -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/boards_sprints_holdouts.rs (target/debug/deps/boards_sprints_holdouts-8142f97468925142)

running 1 test
test test_s_2_04_h_040_bc_5_2_005_all_flag_shows_all_35_no_hint ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.68s
```

**What behavior this pins:** The `--all` flag bypass of the truncation ceiling in
`src/cli/sprint.rs`. Silently ignoring `--all` (always truncating) would fail this holdout.

**Why it passes against unmodified code:** With `--all`, the code skips the 30-row cap
entirely. The test asserts 35 rows in stdout and no hint substring in stderr.

---

## AC-001c / H-040 / BC-5.2.005 — sprint current under limit shows all, no hint

**Behavioral contract:** When the sprint has ≤30 issues (10 in this case), all issues
are shown and no truncation hint appears in stderr.

**Test:** `test_s_2_04_h_040_bc_5_2_005_under_limit_shows_all_no_hint`

**Verification command:**
```
cargo test --test boards_sprints_holdouts test_s_2_04_h_040_bc_5_2_005_under_limit_shows_all_no_hint -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/boards_sprints_holdouts.rs (target/debug/deps/boards_sprints_holdouts-8142f97468925142)

running 1 test
test test_s_2_04_h_040_bc_5_2_005_under_limit_shows_all_no_hint ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.69s
```

**What behavior this pins:** The no-op path through truncation when result count is below
the ceiling. Emitting a spurious hint for sub-limit result sets would fail this holdout.

**Why it passes against unmodified code:** With 10 issues (below the 30 ceiling), no
truncation occurs. The test asserts 10 rows in stdout and no hint substring in stderr.

---

## AC-002 / H-041 / BC-5.2.007 — sprint add JSON has sprint_id

**Behavioral contract:** `jr sprint add --sprint N KEY... --output json` returns JSON
with `sprint_id` present: `{"added": true, "issues": [...], "sprint_id": N}`.

**Test:** `test_s_2_04_h_041_bc_5_2_007_sprint_add_json_has_sprint_id`

**Verification command:**
```
cargo test --test boards_sprints_holdouts test_s_2_04_h_041_bc_5_2_007_sprint_add_json_has_sprint_id -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/boards_sprints_holdouts.rs (target/debug/deps/boards_sprints_holdouts-8142f97468925142)

running 1 test
test test_s_2_04_h_041_bc_5_2_007_sprint_add_json_has_sprint_id ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.71s
```

**What behavior this pins:** The `sprint_id` field in the JSON output of `sprint add`.
Any "harmonization" that removes `sprint_id` from the add response (to match remove)
would fail this holdout.

**Why it passes against unmodified code:** The `sprint add` handler serializes its
response struct that includes `sprint_id: u64`. The test parses stdout JSON with
`serde_json` and asserts `json["sprint_id"] == 100`.

---

## AC-003 / H-041 / BC-5.2.008 — sprint remove JSON has no sprint_id

**Behavioral contract:** `jr sprint remove --sprint N KEY... --output json` returns JSON
WITHOUT `sprint_id`: `{"issues": [...], "removed": true}`. The asymmetry with `sprint add`
is intentional and pinned.

**Test:** `test_s_2_04_h_041_bc_5_2_008_sprint_remove_json_has_no_sprint_id`

**Verification command:**
```
cargo test --test boards_sprints_holdouts test_s_2_04_h_041_bc_5_2_008_sprint_remove_json_has_no_sprint_id -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/boards_sprints_holdouts.rs (target/debug/deps/boards_sprints_holdouts-8142f97468925142)

running 1 test
test test_s_2_04_h_041_bc_5_2_008_sprint_remove_json_has_no_sprint_id ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.69s
```

**What behavior this pins:** The intentional absence of `sprint_id` from the `sprint remove`
JSON response. Adding `sprint_id` to the remove response (to match add) would fail this
holdout, catching unintended "harmonization".

**Why it passes against unmodified code:** The `sprint remove` handler serializes a
response struct that omits `sprint_id`. The test parses stdout JSON and asserts that
`json.get("sprint_id")` returns `None`.

---

## AC-004 / H-042 / BC-5.2.001 — kanban board errors on sprint list

**Behavioral contract:** `jr sprint list --board N` on a kanban board exits non-zero with
the exact literal message `Sprint commands are only available for scrum boards` in stderr.

**Test:** `test_s_2_04_h_042_bc_5_2_001_kanban_board_errors_on_sprint_list`

**Verification command:**
```
cargo test --test boards_sprints_holdouts test_s_2_04_h_042_bc_5_2_001_kanban_board_errors_on_sprint_list -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/boards_sprints_holdouts.rs (target/debug/deps/boards_sprints_holdouts-8142f97468925142)

running 1 test
test test_s_2_04_h_042_bc_5_2_001_kanban_board_errors_on_sprint_list ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.68s
```

**What behavior this pins:** The exact error message wording and non-zero exit code for
sprint commands invoked against kanban boards. Changing the error message wording,
downgrading to a warning, or allowing kanban boards to proceed would all fail this holdout.

**Why it passes against unmodified code:** The wiremock board config endpoint returns
`"type": "kanban"`. The sprint list handler fetches board config, checks the type, and
returns a `JrError` containing the exact string. The test asserts non-zero exit code and
that stderr contains the exact literal.

**VHS recording:**
- `AC-004-kanban-error.tape` — VHS script source
- `AC-004-kanban-error.gif` — terminal recording (192 KB)
- `AC-004-kanban-error.webm` — archival recording (439 KB)

The recording runs the AC-004 test via `cargo test --test boards_sprints_holdouts ... --nocapture`,
demonstrating the kanban board rejection path end-to-end.

---

## AC-005 / H-043 / BC-5.3.001 — team column present when field and UUID set

**Behavioral contract:** The team column in `sprint current` output appears when BOTH
`team_field_id` is configured in the active profile AND at least one issue has a non-null
team UUID that resolves to a name from the team cache.

**Test:** `test_s_2_04_h_043_bc_5_3_001_team_column_present_when_field_and_uuid_set`

**Verification command:**
```
cargo test --test boards_sprints_holdouts test_s_2_04_h_043_bc_5_3_001_team_column_present_when_field_and_uuid_set -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/boards_sprints_holdouts.rs (target/debug/deps/boards_sprints_holdouts-8142f97468925142)

running 1 test
test test_s_2_04_h_043_bc_5_3_001_team_column_present_when_field_and_uuid_set ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.69s
```

**What behavior this pins:** The conjunctive gate in `src/cli/issue/format.rs` that
shows the team column only when both conditions are met. Removing the UUID check
(showing team column even for null team fields) would fail this holdout.

**Why it passes against unmodified code:** The test writes a `teams.json` cache to an
isolated `XDG_CACHE_HOME` with a team UUID matching the issue fixture's custom field
value. The `jr sprint current` output is then asserted to contain "Team" as a column header.

---

## AC-006 / H-043 / BC-5.3.002 — team column absent when no UUID set

**Behavioral contract:** The team column is absent from `sprint current` output when all
issues have null team UUID, even if `team_field_id` is configured.

**Test:** `test_s_2_04_h_043_bc_5_3_002_team_column_absent_when_no_uuid_set`

**Verification command:**
```
cargo test --test boards_sprints_holdouts test_s_2_04_h_043_bc_5_3_002_team_column_absent_when_no_uuid_set -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/boards_sprints_holdouts.rs (target/debug/deps/boards_sprints_holdouts-8142f97468925142)

running 1 test
test test_s_2_04_h_043_bc_5_3_002_team_column_absent_when_no_uuid_set ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.71s
```

**What behavior this pins:** The second half of the conjunctive gate — `team_field_id`
configured is a necessary but not sufficient condition for the team column to appear.
Removing the null-UUID check (showing team column for all issues when `team_field_id`
is set) would fail this holdout.

**Why it passes against unmodified code:** The test configures `team_field_id` but all
issues in the mock sprint return `null` for the team custom field. The `jr sprint current`
output is asserted NOT to contain "Team" as a column header.

---

## AC-007 / H-044 / BC-7.2.001 — ADF renders heading and paragraph, drops mention

**Behavioral contract:** `jr issue view` on an issue with ADF description renders the
text of heading, paragraph, and codeBlock nodes. Mention nodes are silently dropped
(current behavior per issue #202). No panic occurs on any ADF node type.

**Test:** `test_s_2_04_h_044_bc_7_2_001_adf_renders_heading_paragraph_drops_mention`

**Verification command:**
```
cargo test --test boards_sprints_holdouts test_s_2_04_h_044_bc_7_2_001_adf_renders_heading_paragraph_drops_mention -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/boards_sprints_holdouts.rs (target/debug/deps/boards_sprints_holdouts-8142f97468925142)

running 1 test
test test_s_2_04_h_044_bc_7_2_001_adf_renders_heading_paragraph_drops_mention ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.67s
```

**What behavior this pins:** The ADF-to-text conversion in `src/adf.rs` — specifically
the heading, paragraph, and codeBlock rendering paths, and the intentional silent drop
of mention nodes. The KNOWN-GAP comment in the test marks this as a Wave 3 flip point:
when NFR-O-I is implemented, mention nodes should render as `@<displayName>` and this
assertion must be inverted.

**Why it passes against unmodified code:** The test spawns `jr issue view PROJ-1` against
a wiremock issue endpoint with a fully-structured ADF description. The stdout is asserted
to contain "My Heading" (from the heading node) and "Some text" (from the paragraph node);
it is asserted NOT to contain the mention node text. Exit code is asserted to be 0.

**VHS recording:**
- `AC-007-adf-rendering.tape` — VHS script source
- `AC-007-adf-rendering.gif` — terminal recording (186 KB)
- `AC-007-adf-rendering.webm` — archival recording (446 KB)

The recording runs the AC-007 test via `cargo test --test boards_sprints_holdouts ... --nocapture`,
demonstrating the ADF rendering path end-to-end.

---

## Combined Run

**Verification command:**
```
cargo test --test boards_sprints_holdouts
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/boards_sprints_holdouts.rs (target/debug/deps/boards_sprints_holdouts-8142f97468925142)

running 9 tests
test test_s_2_04_h_041_bc_5_2_008_sprint_remove_json_has_no_sprint_id ... ok
test test_s_2_04_h_042_bc_5_2_001_kanban_board_errors_on_sprint_list ... ok
test test_s_2_04_h_041_bc_5_2_007_sprint_add_json_has_sprint_id ... ok
test test_s_2_04_h_040_bc_5_2_005_under_limit_shows_all_no_hint ... ok
test test_s_2_04_h_043_bc_5_3_002_team_column_absent_when_no_uuid_set ... ok
test test_s_2_04_h_040_bc_5_2_005_all_flag_shows_all_35_no_hint ... ok
test test_s_2_04_h_040_bc_5_2_005_truncates_to_30_with_hint_when_35_issues ... ok
test test_s_2_04_h_044_bc_7_2_001_adf_renders_heading_paragraph_drops_mention ... ok
test test_s_2_04_h_043_bc_5_3_001_team_column_present_when_field_and_uuid_set ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.76s
```

Full transcript also available at: `docs/demo-evidence/S-2.04/combined-transcript.txt`

---

## Files in This Directory

| File | Description |
|------|-------------|
| `evidence-report.md` | This report — AC coverage, transcripts, rationale |
| `combined-transcript.txt` | Verbatim `cargo test --test boards_sprints_holdouts` output |
| `AC-004-kanban-error.tape` | VHS script for AC-004 recording |
| `AC-004-kanban-error.gif` | VHS-generated terminal recording (PR embed) |
| `AC-004-kanban-error.webm` | VHS-generated terminal recording (archival) |
| `AC-007-adf-rendering.tape` | VHS script for AC-007 recording |
| `AC-007-adf-rendering.gif` | VHS-generated terminal recording (PR embed) |
| `AC-007-adf-rendering.webm` | VHS-generated terminal recording (archival) |

Note: AC-001 (three cases), AC-002, AC-003, AC-005, and AC-006 use transcript-only
evidence. These are wiremock integration tests whose primary observable is test pass/fail,
not interactive CLI surface. AC-004 (exact error message string) and AC-007 (ADF text
rendering) have meaningful user-facing terminal output and are additionally evidenced
with VHS recordings.

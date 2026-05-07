# Demo Evidence Report — S-0.04

**Story:** Fix multi-profile field routing: active profile field IDs at all 14 read sites  
**Story ID:** S-0.04  
**Branch:** fix/multi-profile-fields-active  
**Commits:** `52bb668` (failing tests), `b304442` (fix)  
**Date recorded:** 2026-05-07  
**Recording medium:** VHS (terminal recordings — CLI product)  
**Font:** Menlo (system default on macOS)

---

## Coverage Summary

| AC | Description | Test(s) | Recording | Result |
|----|-------------|---------|-----------|--------|
| AC-001 | `--profile sandbox` uses sandbox story-points field ID | `test_bc_6_3_001_sandbox_profile_uses_sandbox_story_points_field_id` | AC-001-sandbox-profile-fields.gif | PASS |
| AC-002 | `--points` column present after save round-trip (no legacy `[fields]` block) | `test_bc_6_3_001_points_column_present_after_save_round_trip` | AC-002-points-column-after-save.gif | PASS |
| AC-003 | `sprint current` shows Team and Points after save round-trip | `test_bc_6_3_001_sprint_current_shows_team_and_points_after_save_round_trip` | AC-003-sprint-team-points-after-save.gif | PASS |
| AC-004 | `board view` shows Team column after save round-trip | `test_bc_6_3_001_board_view_shows_team_after_save_round_trip` | AC-004-board-team-after-save.gif | PASS |
| AC-005 | Error message references `[profiles.<name>]` not deprecated `[fields]` | `test_bc_6_3_001_error_message_references_profiles_section_not_fields` | AC-005-error-references-profiles.gif | PASS |
| AC-006 | List warning references `[profiles.<name>]` not deprecated `[fields]` | `test_bc_6_3_001_list_points_warning_references_profiles_section` | AC-006-list-warning-references-profiles.gif | PASS |
| Combined | All 8 BC-6.3.001 tests (6 integration + 2 contract unit) | all 8 | AC-combined-all-bc-6-3-001-pass.gif | 8/8 PASS |

---

## AC-001: `--profile sandbox` Uses Sandbox Story-Points Field ID

**Acceptance Criterion:** Given two profiles where `prod` has
`story_points_field_id = "customfield_10005"` and `sandbox` has
`story_points_field_id = "customfield_10099"`, running
`jr --profile sandbox issue create --points 5 ...` MUST produce a POST body
containing `"customfield_10099": 5` and NOT `"customfield_10005"`.

**Test:** `test_bc_6_3_001_sandbox_profile_uses_sandbox_story_points_field_id` in `tests/multi_profile_fields.rs`

**Recordings:**
- `AC-001-sandbox-profile-fields.gif` (161 KB)
- `AC-001-sandbox-profile-fields.webm` (621 KB)
- `AC-001-sandbox-profile-fields.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test test_bc_6_3_001_sandbox_profile_uses_sandbox_story_points_field_id ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.15s
```

---

## AC-002: `--points` Column Present After Save Round-Trip

**Acceptance Criterion:** After a save round-trip (legacy `[fields]` block absent,
field IDs only in `[profiles.default]`), `jr issue list --points` must still show
the Points column. The field ID must be requested in the search API call.

**Test:** `test_bc_6_3_001_points_column_present_after_save_round_trip` in `tests/multi_profile_fields.rs`

**Recordings:**
- `AC-002-points-column-after-save.gif` (160 KB)
- `AC-002-points-column-after-save.webm` (621 KB)
- `AC-002-points-column-after-save.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test test_bc_6_3_001_points_column_present_after_save_round_trip ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.15s
```

---

## AC-003: `sprint current` Shows Team and Points After Save Round-Trip

**Acceptance Criterion:** `jr sprint current` must include both Team and Points
columns when field IDs live only in `[profiles.default]` (no `[fields]` block).
Both `customfield_10031` (SP) and `customfield_10100` (team) must appear in the
sprint issues API query.

**Test:** `test_bc_6_3_001_sprint_current_shows_team_and_points_after_save_round_trip` in `tests/multi_profile_fields.rs`

**Recordings:**
- `AC-003-sprint-team-points-after-save.gif` (167 KB)
- `AC-003-sprint-team-points-after-save.webm` (633 KB)
- `AC-003-sprint-team-points-after-save.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test test_bc_6_3_001_sprint_current_shows_team_and_points_after_save_round_trip ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.18s
```

---

## AC-004: `board view` Shows Team Column After Save Round-Trip

**Acceptance Criterion:** `jr board view` must include a Team column (with resolved
team name) when `team_field_id` lives only in `[profiles.default]`. The team custom
field must be requested in the search API call.

**Test:** `test_bc_6_3_001_board_view_shows_team_after_save_round_trip` in `tests/multi_profile_fields.rs`

**Recordings:**
- `AC-004-board-team-after-save.gif` (154 KB)
- `AC-004-board-team-after-save.webm` (603 KB)
- `AC-004-board-team-after-save.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test test_bc_6_3_001_board_view_shows_team_after_save_round_trip ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.19s
```

---

## AC-005: Error Message References `[profiles.<name>]` Not `[fields]`

**Acceptance Criterion:** When `story_points_field_id` is missing for the active
profile, the error message must say
`"set story_points_field_id under [profiles.<name>]"` and must NOT contain
`"[fields]"`.

**Test:** `test_bc_6_3_001_error_message_references_profiles_section_not_fields` in `tests/multi_profile_fields.rs`

**Recordings:**
- `AC-005-error-references-profiles.gif` (160 KB)
- `AC-005-error-references-profiles.webm` (635 KB)
- `AC-005-error-references-profiles.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test test_bc_6_3_001_error_message_references_profiles_section_not_fields ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.06s
```

---

## AC-006: List Warning References `[profiles.<name>]` Not `[fields]`

**Acceptance Criterion:** When `--points` is passed but `story_points_field_id` is
missing from the active profile, the warning on stderr must say
`"set story_points_field_id under [profiles.<name>]"` and must NOT contain `"[fields]"`.

**Test:** `test_bc_6_3_001_list_points_warning_references_profiles_section` in `tests/multi_profile_fields.rs`

**Recordings:**
- `AC-006-list-warning-references-profiles.gif` (161 KB)
- `AC-006-list-warning-references-profiles.webm` (627 KB)
- `AC-006-list-warning-references-profiles.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test test_bc_6_3_001_list_points_warning_references_profiles_section ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.15s
```

---

## Combined: All 8 BC-6.3.001 Tests Pass

**Recordings:**
- `AC-combined-all-bc-6-3-001-pass.gif` (182 KB)
- `AC-combined-all-bc-6-3-001-pass.webm` (851 KB)
- `AC-combined-all-bc-6-3-001-pass.tape` (VHS script source)

**Evidence (captured test output):**
```
running 8 tests
test test_bc_6_3_001_active_profile_returns_per_profile_field_ids ... ok
test test_bc_6_3_001_board_view_shows_team_after_save_round_trip ... ok
test test_bc_6_3_001_error_message_references_profiles_section_not_fields ... ok
test test_bc_6_3_001_field_ids_survive_toml_save_round_trip ... ok
test test_bc_6_3_001_list_points_warning_references_profiles_section ... ok
test test_bc_6_3_001_points_column_present_after_save_round_trip ... ok
test test_bc_6_3_001_sandbox_profile_uses_sandbox_story_points_field_id ... ok
test test_bc_6_3_001_sprint_current_shows_team_and_points_after_save_round_trip ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s
```

---

## Quality Gates

All gates verified clean after demo recordings (no source changes):

| Gate | Result |
|------|--------|
| `cargo build` | clean |
| `cargo test --test multi_profile_fields` | green (8/8 BC-6.3.001 tests passed) |
| `cargo clippy -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |

---

## Traceability

| Recording | AC | BC Anchor | H-NEW-MP-001 Outcome |
|-----------|----|-----------|----------------------|
| AC-001-sandbox-profile-fields | AC-001 | BC-6.3.001 | MUST-PASS (was MUST-FAIL at pre-fix HEAD) |
| AC-002-points-column-after-save | AC-002 | BC-6.3.001 | MUST-PASS |
| AC-003-sprint-team-points-after-save | AC-003 | BC-6.3.001 | MUST-PASS |
| AC-004-board-team-after-save | AC-004 | BC-6.3.001 | MUST-PASS |
| AC-005-error-references-profiles | AC-005 | BC-6.3.001 | MUST-PASS |
| AC-006-list-warning-references-profiles | AC-006 | BC-6.3.001 | MUST-PASS |
| AC-combined-all-bc-6-3-001-pass | AC-001..006 | BC-6.3.001 | 8/8 MUST-PASS |

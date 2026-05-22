---
document_type: verification-delta
issue: "#398"
title: "Verification properties for issue edit/create field-echo"
date: "2026-05-21"
phase: F2
new_vps:
  - VP-398-001
  - VP-398-002
  - VP-398-003
  - VP-398-004
  - VP-398-005
related_bcs:
  - BC-3.4.012
  - BC-3.4.013
  - BC-3.4.014
---

# Verification Delta — Issue #398: Field-echo Verification Properties

## New Verification Properties

### VP-398-001: Resolved team name is the display name (not a UUID) in all echo surfaces

**Description**: When `--team <partial_name>` resolves to a team via `partial_match`,
the echoed name in ALL echo surfaces must be the team's display name, never the
internal UUID or the user's raw partial-match query.

**Applies to**:
- BC-3.4.012: `issue edit` table-mode — `  team → <display_name>` on stderr
- BC-3.4.013: `issue edit` JSON-mode — `changed_fields["team"]` value
- BC-3.4.014: `issue create` table-mode — `  team → <display_name>` on stderr

**Test strategy**: Integration tests using wiremock. Mock the team list API to return a
team with `id: "some-uuid-string"` and `name: "Platform Core"`. Run `jr issue edit KEY
--team plat` and `jr issue create --team plat`. Assert:
- Table-mode stderr does NOT contain the substring `"some-uuid-string"`.
- Table-mode stderr DOES contain `"Platform Core"`.
- JSON-mode `changed_fields.team` value does NOT contain `"some-uuid-string"`.
- JSON-mode `changed_fields.team` value IS exactly `"Platform Core"`.

**UUID-bypass exception**: When the caller passes a raw UUID directly (e.g., `--team
some-uuid-string`), the echo value IS the UUID. This is correct because no name lookup
occurred. Tests for this exception should assert the UUID is echoed as-is, not a
different string.

**UUID-bypass predicate** (`is_team_uuid` in `src/cli/issue/helpers.rs`): exactly 36
characters, in 8-4-4-4-12 hyphen-separated groups of ASCII hex digits (case-insensitive).
Any string that does not satisfy this predicate — including strings that are 35 chars, 37
chars, or contain non-hex characters — routes through partial-match, not the UUID bypass.

**Negative test case (DECISION LOCKED — round 5 F-1)**: The negative case MUST be a **direct unit-level assertion on `is_team_uuid`**, not an integration test routing a probe string through `partial_match`. Rationale: routing a 35-char boundary probe through `partial_match` proves nothing about `is_team_uuid`; the predicate boundary is better isolated at the unit level. The existing unit test `is_team_uuid_rejects_wrong_length` in `src/cli/issue/helpers.rs` (~line 617) already covers this boundary directly — reuse or cite it.

**Required unit-level negative assertion**: call `is_team_uuid("36885b3c-1bf0-4f85-a357-c5b858c31de")` (35 chars, one short of UUID length) and assert the return value is `false`. This directly proves that a string one char short of 36 chars is rejected by the predicate. A true 36-char UUID (e.g., `"36885b3c-1bf0-4f85-a357-c5b858c31def"`) would return `true`. **This assertion already exists at `src/cli/issue/helpers.rs:617` (`is_team_uuid_rejects_wrong_length`) — cite it, no new test needed.**

Do NOT write an integration test that routes this probe through `partial_match` and asserts a "No team matching" error — that tests `partial_match` fallback behavior, not the `is_team_uuid` predicate boundary.

**Test placement (DECISION LOCKED — round 7 F-1)**: `is_team_uuid` in `src/cli/issue/helpers.rs` has no `pub` visibility — it is module-private and is not exported via `lib.rs`. The `is_team_uuid` negative-case assertion is a UNIT test that MUST be placed in the `#[cfg(test)] mod tests` block INSIDE `src/cli/issue/helpers.rs`. Do NOT place it in `tests/`. The team-echo positive cases (verifying that a resolved display name, not a UUID, appears in stderr or JSON `changed_fields`) remain wiremock integration tests in `tests/`.

**Suggested test name**: `test_BC_3_4_012_team_echo_is_resolved_name_not_uuid` (table),
`test_BC_3_4_013_team_echo_is_resolved_name_not_uuid` (JSON),
`test_BC_3_4_014_create_team_echo_is_resolved_name_not_uuid` (create table),
`test_is_team_uuid_rejects_35_char_hex_string` (negative unit test — cite or alias the existing `is_team_uuid_rejects_wrong_length` test).

---

### VP-398-002: Description echo asymmetry is pinned (table = `(updated)` marker; JSON = raw input string)

**Description**: The `description` field is deliberately represented differently in the
two output channels. This asymmetry must NOT silently collapse in either direction:
- Table/human channel (BC-3.4.012): description echo is exactly `(updated)` — never the content.
- JSON channel (BC-3.4.013): `changed_fields["description"]` is the **raw user-supplied
  input string** (the literal value from `--description` or `--description-stdin`) — never
  `"(updated)"`, never an ADF→text round-trip. For a plain-text description, this is the
  original string exactly as typed. For a Markdown description (passed to `markdown_to_adf`),
  this is the original Markdown string — not the ADF output.

**Applies to**:
- BC-3.4.012: table-mode description echo
- BC-3.4.013: JSON-mode description echo

**Test strategy (table mode)**:
1. Run `jr issue edit KEY --description "Some longer description text"` in table mode.
2. Assert stderr CONTAINS the substring `description → (updated)`.
3. Assert stderr does NOT contain `"Some longer description text"` (content must not appear).
4. Assert stderr does NOT contain a truncated preview (no first-N-chars of content).

**Test strategy (JSON mode)**:
1. Run `jr issue edit KEY --description "Some longer description text" --output json`.
2. Parse the JSON output. Assert `changed_fields.description` is NOT `"(updated)"`.
3. Assert `changed_fields.description` IS exactly `"Some longer description text"` (the raw input).
4. Assert `changed_fields.description` does NOT contain raw ADF JSON structure.

**Suggested test names**: `test_BC_3_4_012_description_echo_is_updated_marker_not_content`,
`test_BC_3_4_013_description_echo_is_raw_input_string_not_marker`.

**VP-398-002 sub-case — stdin trailing-newline not normalized**:

EC-3.4.012-2 and EC-3.4.013-3 assert that `--description-stdin` captures input verbatim, including any trailing newline, with no normalization before the ADF conversion. This "no normalization" claim must be a tested AC, not an aspirational comment.

**Test strategy (stdin trailing-newline sub-case)**:
1. Pipe content that ends with a trailing newline via `--description-stdin`:
   `printf 'My description\n' | jr issue edit KEY --description-stdin --output json`
2. Parse the JSON output.
3. Assert `changed_fields.description` is exactly `"My description\n"` — the trailing `\n` must be present.
4. Assert `changed_fields.description` is NOT `"My description"` (no silent strip).

This test isolates the specific claim in EC-3.4.013-3: the raw bytes from stdin are stored in `changed_fields["description"]` before any processing. The same raw value is what gets passed to `markdown_to_adf` (or treated as plain text) for the PUT body — if stdin normalization were added, both the PUT body and the `changed_fields` value would be affected.

**Applies to**: BC-3.4.013 (JSON mode verifies the exact value; BC-3.4.012 table mode is unaffected since it always shows `(updated)` regardless of content).

**Suggested test name**: `test_BC_3_4_013_description_stdin_trailing_newline_preserved_in_changed_fields`.

**Durability note**: The description table/JSON asymmetry tested by VP-398-002 depends on the CLAUDE.md Gotcha entry mandated for the implementation phase (see prd-delta-398.md §6 "Description asymmetry documentation"). Without that entry, a future maintainer may "fix" the asymmetry by making both channels consistent, silently breaking this VP. The VP-398-002 test suite is necessary but not sufficient — the Gotcha entry is the institutional guard that keeps the asymmetry intentional and documented. The exact Gotcha text is pinned in prd-delta-398.md §6 (DECISION LOCKED — round 5 F-4). Adding that Gotcha entry is a tracked acceptance criterion of the F3 implementation story — not aspirational.

---

### VP-398-003: `updated: true` invariant preserved after `changed_fields` extension

**Description**: The `"updated": true` field in `edit_response` JSON payload must be
present in ALL cases after the `changed_fields` extension. Its removal or absence under
any condition is a breaking change.

**Note**: A zero-flag edit (`jr issue edit KEY --output json` with no field flags) is
NOT a valid test vehicle for this VP. `handle_edit` bails with exit 1 before reaching
the PUT when no field flags are given — the success path (and therefore `edit_response`)
is never reached. All VP-398-003 tests MUST use at least one valid field flag.

**Applies to**: BC-3.4.013

**Test strategy**:
1. **Non-empty changed_fields**: Run `jr issue edit KEY --summary "New title" --output json`.
   Parse JSON. Assert `output["updated"] == true`. Assert `output["changed_fields"]` is
   present and non-empty (contains at least `{"summary": "New title"}`).
2. **Single-field edit, different field**: Run `jr issue edit KEY --priority "High" --output json`.
   Parse JSON. Assert `output["updated"] == true`. Assert `output["changed_fields"]["priority"] == "High"`.
3. **Snapshot test**: The updated insta snapshot `jr__cli__issue__json_output__tests__edit.snap`
   must include `"updated": true` in the pinned shape alongside a non-empty `changed_fields`.

**Snapshot test split (DECISION LOCKED — round 7 F-3)**:
- The existing `test_edit` unit test in `src/cli/issue/json_output.rs` MUST be updated to pass a **non-empty** `BTreeMap` to `edit_response` for `changed_fields`. The existing insta snapshot file `jr__cli__issue__json_output__tests__edit.snap` MUST be regenerated to reflect this non-empty shape. The snapshot file keeps its current name — the "existing no-prefix tests are not renamed" convention applies (`test_edit` has no prefix and is not renamed).
- A **new** test `test_edit_response_empty_changed_fields` MUST be added (applying the new-test naming convention) to cover the empty-`BTreeMap` case. This test calls `edit_response` with an empty `BTreeMap<String, String>` and asserts that the resulting JSON has `"updated": true` and `"changed_fields": {}`. This new test does NOT use an insta snapshot — it asserts the shape directly to avoid snapshot churn for a trivial edge case.

**Suggested test names**: `test_BC_3_4_013_updated_true_present_with_summary_changed_fields`,
`test_BC_3_4_013_updated_true_present_with_priority_changed_fields`,
`test_edit_response_empty_changed_fields` (new test for empty-map case).

---

### VP-398-004: Cleared-field single-key model — no `no_parent` / `no_points` keys ever present

**Description**: When `--no-parent` is used, the `changed_fields` map (and the table-mode
echo) MUST contain exactly one key named `parent` with value `(cleared)`. No separate
`no_parent` key must appear. Identically for `--no-points`: exactly one key `points` with
value `(cleared)`; no `no_points` key.

**Applies to**:
- BC-3.4.012: table-mode echo (`  parent → (cleared)`, `  points → (cleared)`)
- BC-3.4.013: JSON-mode `changed_fields` (`"parent": "(cleared)"`, `"points": "(cleared)"`)

**Test strategy**:
1. Run `jr issue edit KEY --no-parent --output json`. Parse JSON. Assert `changed_fields`
   contains `"parent": "(cleared)"`. Assert `changed_fields` does NOT contain a key named
   `"no_parent"`. Assert `changed_fields` contains exactly one key (only `parent`, since
   only `--no-parent` was supplied).
2. Run `jr issue edit KEY --no-points --output json`. Parse JSON. Assert `changed_fields`
   contains `"points": "(cleared)"`. Assert `changed_fields` does NOT contain a key named
   `"no_points"`.
3. Table-mode: Run `jr issue edit KEY --no-parent` (no `--output json`). Assert stderr
   CONTAINS `  parent → (cleared)`. Assert stderr does NOT contain `no_parent` as a
   field label.

**Note**: This VP is a verification artifact only — it does not affect BC count surfaces
(total_bcs, definitional_count, BC-INDEX, CANONICAL-COUNTS). It verifies the MED-1 locked
decision from prd-delta-398.md §6: single map keys for cleared-field cases.

**Suggested test names**: `test_BC_3_4_013_no_parent_key_is_parent_not_no_parent`,
`test_BC_3_4_013_no_points_key_is_points_not_no_points`,
`test_BC_3_4_012_no_parent_table_echo_uses_parent_key`.

---

### VP-398-005: Create-path team-resolution error exits with `JrError::UserError` (IMP-5)

**Description**: When `jr issue create --team <ambiguous_or_missing_name> --no-input` is
run, `resolve_team_field` returns `Err(JrError::UserError)` before the POST. No team echo
is emitted, the create does not proceed, and the exit code matches
`JrError::UserError.exit_code()` as defined in `src/error.rs` (currently 64). This VP
adds a test to verify the create-path exit code so it is no longer an unverified pin.

**Applies to**:
- BC-3.4.014: `issue create` table-mode team echo — EC-3.4.014-3, EC-3.4.014-5

**Test strategy**:
1. Mock the team-list API to return a team list containing no team matching the given
   partial name (or return an ambiguous/none match).
2. Run `jr issue create --summary "Test" --type Task --team <unresolvable_name> --no-input`
   via wiremock integration test.
3. Assert the process exit code is 64.
4. Assert stdout is empty (no JSON, no issue key).
5. Assert stderr contains the stable substring `No team matching` (or the ambiguous-team
   error text, depending on the mock topology chosen).
6. Assert no POST to `/rest/api/3/issue` was issued.

**Note**: The exit code 64 is derived from `JrError::UserError.exit_code()` in
`src/error.rs`. If `exit_code()` is ever changed for `UserError`, this VP will catch the
regression. The test must assert the numeric exit code (64), not just non-zero.

**Suggested test name**: `test_BC_3_4_014_create_unresolvable_team_no_input_exits_64`.

---

## VP to BC Mapping Summary

| VP ID | BC(s) Covered | Key Invariant |
|-------|---------------|---------------|
| VP-398-001 | BC-3.4.012, BC-3.4.013, BC-3.4.014 | Team echo is display name, not UUID |
| VP-398-002 | BC-3.4.012, BC-3.4.013 | Description echo asymmetry: table=marker, JSON=raw input string |
| VP-398-003 | BC-3.4.013 | `"updated": true` present after `changed_fields` extension |
| VP-398-004 | BC-3.4.012, BC-3.4.013 | Cleared-field uses single key (`parent`/`points`), no `no_parent`/`no_points` keys |
| VP-398-005 | BC-3.4.014 | Create-path team-resolution error exits with code 64 (`JrError::UserError`) |

## Project Convention Note

This project inlines Verification Properties directly in BC body files rather than
maintaining separate VP-INDEX, verification-architecture.md, or
verification-coverage-matrix.md files (those files do not exist in this repository).

VP-398-001, VP-398-002, VP-398-003, VP-398-004, and VP-398-005 are recorded as **Verification Properties
subsections within the BC bodies** in `.factory/specs/prd/bc-3-issue-write.md`:
- VP-398-001: present in BC-3.4.012 §Verification Properties and BC-3.4.013 §Verification Properties and BC-3.4.014 §Verification Properties.
- VP-398-002: present in BC-3.4.012 §Verification Properties and BC-3.4.013 §Verification Properties.
- VP-398-003: present in BC-3.4.013 §Verification Properties.
- VP-398-004: present in BC-3.4.012 §Verification Properties and BC-3.4.013 §Verification Properties.
- VP-398-005: present in BC-3.4.014 §Verification Properties (added round 8).

No separate index propagation is required. VP-398-004 and VP-398-005 are verification artifacts only — they do not affect BC count surfaces (total_bcs, definitional_count, BC-INDEX, CANONICAL-COUNTS).

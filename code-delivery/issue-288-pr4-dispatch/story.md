---
document_type: story
story_id: issue-288-pr4-dispatch
title: "jr issue create --request-type dispatch fork to POST /rest/servicedeskapi/request"
cycle: 3-feature-jsm-request-types-288
wave: 3
status: ready-for-implementation
priority: P0
estimated_effort: large
tdd_mode: strict
version: "1.0.0"
date: 2026-05-18
producer: story-writer
bc_anchors:
  - BC-3.8.001
  - BC-3.8.002
  - BC-3.8.003
  - BC-3.8.004
  - BC-3.8.005
  - BC-3.8.006
  - BC-3.8.007
  - BC-3.8.008
  - BC-3.8.009
  - BC-3.8.010
  - BC-3.8.011
  - BC-3.3.001
  - BC-1.3.023
  - BC-X.3.005
holdout_anchors:
  - H-NEW-JSM-RT-001
  - H-NEW-JSM-RT-002
  - H-NEW-JSM-RT-003
  - H-NEW-JSM-RT-004
nfr_anchors: []
adr_refs:
  - ADR-0014
sd_refs: []
files_modified:
  - src/api/auth.rs
  - src/cli/auth/tests/mod.rs
  - src/cli/mod.rs
  - src/cli/issue/create.rs
  - src/cli/issue/mod.rs
  - tests/issue_create_jsm.rs
  - CLAUDE.md
  - CHANGELOG.md
breaking_change: false
depends_on:
  - issue-288-pr1-api
  - issue-288-pr2-cli
blocks: []
issue: 288
---

# issue-288-pr4-dispatch: `jr issue create --request-type` Dispatch Fork

## Context

This is the final PR for issue #288: it adds the user-facing `--request-type <NAME|ID>`,
`--field NAME=VALUE`, and `--on-behalf-of <accountId>` flags to `jr issue create` and wires
the conditional dispatch fork to `POST /rest/servicedeskapi/request`.

**This PR also absorbs the OAuth scope addition formerly planned as pr3-scope.** Research at
`.factory/research/issue-288-oauth-scope-coordination.md` validated that a separate PR for
the scope change (with a PR-template release-gate checklist) was disproportionate overhead.
The scope addition is a two-line code change (one in `src/api/auth.rs`, one in the pin test)
and lands atomically with the dispatch work that depends on it.

**Both prerequisites must be merged before this PR starts:**
- pr1-api: `JiraClient::create_jsm_request`, `list_request_types`, `get_request_type_fields`,
  and all serde types
- pr2-cli: `jr requesttype` commands and cache functions (both cache families needed in `handle_jsm_create`)

**Platform path is the regression baseline.** The existing `issue create` behavior (without
`--request-type`) MUST be byte-for-byte unchanged. The dispatch fork is gated solely on
`request_type.is_some()`. When `--request-type` is absent, the code path is identical to
the pre-#288 state. Existing integration tests (`tests/issue_create_json.rs`,
`tests/issue_commands.rs`, `tests/issue_write_holdouts.rs`) are the regression guards —
they must pass without modification.

**`handle_create` size warning:** `src/cli/issue/create.rs` is ~1,601 LOC. This PR adds
~120–160 LOC (the `handle_jsm_create` helper). The file will reach ~1,760 LOC. ADR-0014
documents this is acceptable; the shard rule (ADR-0012) is NOT triggered because the new
branch is structurally separate. Revisit at F7 if the file exceeds 2,000 LOC.

## Behavioral Contracts

| BC ID | Summary | Coverage |
|-------|---------|---------|
| BC-3.8.001 | `--request-type` dispatches to servicedeskapi; platform path unchanged when absent | H-NEW-JSM-RT-001 happy path + platform non-interference |
| BC-3.8.002 | Body uses `requestFieldValues`; `serviceDeskId` via `require_service_desk` | service desk resolution + non-JSM error (H-NEW-JSM-RT-002) |
| BC-3.8.003 | Name resolution via `partial_match`; errors clean on Ambiguous/None | name resolution + ambiguity tests |
| BC-3.8.004 | Numeric `--request-type <ID>` bypasses name resolution | numeric ID bypass test |
| BC-3.8.005 | `--summary` → `requestFieldValues.summary` (required) | summary mapping |
| BC-3.8.006 | `--description` → ADF; `isAdfRequest: true` | ADF description tests |
| BC-3.8.007 | `--priority`/`--label` → `requestFieldValues` | priority/label mapping |
| BC-3.8.008 | `--field NAME=VALUE`; first `=` splits; duplicate last-wins | proptest A.1–A.4 + integration |
| BC-3.8.009 | `--on-behalf-of <accountId>` → `raiseOnBehalfOf`; pass-through | OBO injection + absence omission |
| BC-3.8.010 | `--type` ignored with stderr warning when `--request-type` set | H-NEW-JSM-RT-004 |
| BC-3.8.011 | Platform-only flags (`--team`, `--points`, `--parent`, `--to`, `--account-id`) emit stderr warnings on JSM path | 5 new warning-emission tests, one per flag |
| BC-3.3.001 | Platform path unchanged when `--request-type` absent | all existing tests stay green |
| BC-1.3.023 | 401 scope-mismatch hint surfaces `write:servicedesk-request` | H-NEW-JSM-RT-003 |
| BC-X.3.005 | `InsufficientScope` dispatch on 401 | scope-mismatch integration test |

## Approved Scope

**New files:**
- `tests/issue_create_jsm.rs` — comprehensive integration tests for all BC-3.8.001..011

**Modified files:**
- `src/api/auth.rs` — add `write:servicedesk-request` to `DEFAULT_OAUTH_SCOPES` (absorbed from former pr3-scope)
- `src/cli/auth/tests/mod.rs` — update `default_oauth_scopes_pins_the_full_set_with_offline_access` pin test to include `write:servicedesk-request` (absorbed from former pr3-scope)
- `src/cli/mod.rs` — add `--request-type <NAME|ID>` (`Option<String>`), `--field NAME=VALUE`
  (`Vec<String>`, repeatable), `--on-behalf-of <accountId>` (`Option<String>`) to
  `IssueCommand::Create` variant
- `src/cli/issue/create.rs` — (1) update `handle_create` let-destructure for new fields;
  (2) add early branch: `if request_type.is_some() { return handle_jsm_create(...).await; }`;
  (3) add `handle_jsm_create` helper function (or extracted module if it grows large);
  (4) add `parse_field_kv` pure helper + `mod proptests` block (7 proptest properties)
- `src/cli/issue/mod.rs` — update pattern destructure to include new fields
- `CLAUDE.md` — document new flags, `--type` interaction warning, proptest properties added; add OAuth scope-change guidance note
- `CHANGELOG.md` — add entry noting `write:servicedesk-request` scope addition and re-consent prompt

**Existing tests that must remain green without modification:**
- `tests/issue_create_json.rs`
- `tests/issue_commands.rs`
- `tests/issue_write_holdouts.rs`
- `tests/requesttype_commands.rs` (from pr2)
- `tests/queue.rs` (from pr2)

## Acceptance Criteria

**AC-001** (traces to BC-3.8.001 + H-NEW-JSM-RT-001 — happy path dispatches to servicedeskapi).
`jr issue create --project HELP --request-type "Password Reset" --summary "My issue"` fires
exactly ONE POST to `/rest/servicedeskapi/request` and ZERO POSTs to `/rest/api/3/issue`.
Output: `{"key": "HELP-42"}` (or table equivalent). Exit 0. Pinned by:
`tests/issue_create_jsm.rs::test_jsm_create_happy_path_routes_to_servicedeskapi`
(holdout H-NEW-JSM-RT-001 — uses `expect(0)` on the platform endpoint mock)

**AC-002** (traces to BC-3.3.001 — platform path byte-for-byte unchanged when flag absent).
`jr issue create --project PROJ --type Task --summary "Platform issue"` (NO `--request-type`)
fires exactly ONE POST to `/rest/api/3/issue` and ZERO POSTs to `/rest/servicedeskapi/request`.
Pinned by: all existing tests in `tests/issue_create_json.rs` and `tests/issue_commands.rs`
remaining green without modification.

**AC-003** (traces to BC-3.8.002 + H-NEW-JSM-RT-002 — non-JSM project exits 64, zero HTTP POST).
`jr issue create --project SW --request-type "Bug Report" --summary "test"` on a project
that is not a JSM service desk exits 64 with stderr containing
'`--request-type` requires a Jira Service Management project'. ZERO POSTs to either endpoint.
Pinned by: `tests/issue_create_jsm.rs::test_jsm_create_non_jsm_project_exits_64_zero_http`
(holdout H-NEW-JSM-RT-002 completion)

**AC-004** (traces to BC-3.8.003 — ambiguous request type exits 64 with hint).
`jr issue create --project HELP --request-type "Bug" --summary "test"` when "Bug" matches
two request types exits 64 with "Ambiguous request type" + candidate names + hint
`jr requesttype list --project HELP`. Pinned by:
`tests/issue_create_jsm.rs::test_jsm_create_ambiguous_request_type_exits_64`

**AC-005** (traces to BC-3.8.004 — numeric ID bypasses name resolution).
`jr issue create --project HELP --request-type 5 --summary "test"` uses `requestTypeId: "5"`
directly in the body without calling `GET .../requesttype` list endpoint. Pinned by:
`tests/issue_create_jsm.rs::test_jsm_create_numeric_id_bypasses_name_lookup`

**AC-006** (traces to BC-3.8.005 — summary required in requestFieldValues).
Body sent to `/rest/servicedeskapi/request` contains `requestFieldValues.summary = "<text>"`.
If `--summary` is absent and `--no-input` is set, exits 64 "summary is required for JSM request submission".
Pinned by: `tests/issue_create_jsm.rs::test_jsm_create_summary_in_requestfieldvalues`

**AC-007** (traces to BC-3.8.006 — description → ADF, isAdfRequest: true).
When `--description "my text"` is set, body contains `isAdfRequest: true` and
`requestFieldValues.description` is a JSON object (ADF root, NOT a bare string).
When `--description` is absent, body does NOT contain `requestFieldValues.description`
and does NOT contain `isAdfRequest` (or contains `isAdfRequest: false`). Pinned by:
`tests/issue_create_jsm.rs::test_jsm_create_description_is_adf_with_is_adf_request_true`

**AC-008** (traces to BC-3.8.007 — priority and labels in requestFieldValues).
`--priority High` → body `requestFieldValues.priority = {"name": "High"}`.
`--label alpha --label beta` → body `requestFieldValues.labels = ["alpha", "beta"]` (plain string array, NOT object array).
Pinned by: `tests/issue_create_jsm.rs::test_jsm_create_priority_and_labels_mapped`

**AC-009** (traces to BC-3.8.008 — `--field` parsing; first-equals split; duplicate last-wins).
`--field "customfield_10200=foo" --field "desc=bar=baz"` results in body:
`requestFieldValues = {customfield_10200: "foo", desc: "bar=baz", ...}` (first `=` splits;
value preserves remaining `=` chars). Duplicate `--field summary=override` overwrites `--summary`
value in `requestFieldValues`. Missing `=` → exit 64. Pinned by:
`tests/issue_create_jsm.rs::test_jsm_create_field_first_equals_split_and_duplicate_last_wins`
AND proptest properties A.1–A.4 in `src/cli/issue/create.rs::mod proptests`.

**AC-010** (traces to BC-3.8.009 — `--on-behalf-of` → `raiseOnBehalfOf` top-level).
`--on-behalf-of "557058:abc123"` → body contains `raiseOnBehalfOf: "557058:abc123"` at the
root level (NOT inside `requestFieldValues`). When `--on-behalf-of` is absent, `raiseOnBehalfOf`
key is completely absent from body (NOT null). Pinned by:
`tests/issue_create_jsm.rs::test_jsm_create_on_behalf_of_injected_at_top_level`
AND proptest C.3 in `src/api/jsm/requests.rs::mod proptests`.

**AC-011** (traces to BC-3.8.010 + H-NEW-JSM-RT-004 — `--type` warning emitted before dispatch).
`jr issue create --project HELP --request-type "Bug" --type Task --summary "test"` emits
a single line to stderr: "warning: --type is ignored when --request-type is set; request type
encodes the issue type". Exit 0. JSON output shape unchanged. The warning is emitted before
the dispatch is issued; per BC-3.8.010, the warning may or may not fire if the command
early-exits on a downstream validation error (`--summary` missing, ambiguous type, etc.).
Pinned by:
`tests/issue_create_jsm.rs::test_jsm_create_type_flag_ignored_with_warning`
(holdout H-NEW-JSM-RT-004)

[UPDATED 2026-05-19 issue #288 pr4 adversary-pass-06 O-01] AC-011 wording
aligned to BC-3.8.010 permissive "need not" language. Implementation fires
warnings pre-dispatch (so they appear even on early-exit paths), which is
BC-compliant. Wave 2 pass-02 M-2 precedent applied: align AC to BC+impl
when impl is already correct per BC.

**AC-012** (traces to BC-1.3.023 + BC-X.3.005 + H-NEW-JSM-RT-003 — 401 scope-mismatch hint).
When `POST /rest/servicedeskapi/request` returns 401, the error path surfaces a hint
containing `write:servicedesk-request` (via `BC-X.3.005` `InsufficientScope` dispatch +
`BC-1.6.042` 401 substring match). Pinned by:
`tests/issue_create_jsm.rs::test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`
(repurposed in place (renamed from its pre-#384 name by S-384 F4); at S-288 delivery time this test asserted `write:servicedesk-request`; holdout H-NEW-JSM-RT-003 was subsequently re-bound to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` by S-384 adversary-pass-9 C-01)

**AC-013** (traces to BC-3.8.008 + verification-delta proptest A.1–A.4 — `parse_field_kv` extracted).
`parse_field_kv(args: &[String]) -> Result<HashMap<String, serde_json::Value>, JrError>` is
a standalone, unit-testable pure function (NOT inlined inside `handle_create` body).
Four proptest properties in `src/cli/issue/create.rs::mod proptests` covering A.1 (first-`=`
delimiter), A.2 (empty value allowed), A.3 (duplicate last-wins), A.4 (no panic on arbitrary
input). Pinned by: inline proptest block in `src/cli/issue/create.rs`.

**AC-014** (traces to BC-3.8.001 + BC-3.8.009 + verification-delta proptest C.1–C.3 — `build_jsm_request_body` extracted).
`build_jsm_request_body(...)` is a pure body-construction helper callable WITHOUT a `JiraClient`.
Three proptest properties in `src/api/jsm/requests.rs::mod proptests` covering C.1 (summary
always present), C.2 (description → ADF + `isAdfRequest: true` when Some), C.3 (`raiseOnBehalfOf`
presence/absence). Pinned by: inline proptest block in `src/api/jsm/requests.rs`.

**AC-015** (traces to BC-3.8.001 — `--output json` shape consistent with platform create).
`jr issue create --request-type "Bug" --project HELP --summary "test" --output json` emits
`{"key": "HELP-42"}` — identical shape to platform create. No additional fields. Pinned by:
`tests/issue_create_jsm.rs::test_jsm_create_output_json_shape_matches_platform`

**AC-016** (traces to BC-1.3.023 — OAuth scope addition absorbed from former pr3-scope).
`write:servicedesk-request` is present in `DEFAULT_OAUTH_SCOPES` in `src/api/auth.rs`.
The pin test `default_oauth_scopes_pins_the_full_set_with_offline_access` in
`src/cli/auth/tests/mod.rs` includes `write:servicedesk-request` in its expected set and
passes without modification. `CHANGELOG.md` contains an entry under the unreleased section
noting the scope addition and re-consent prompt (issue #288). `CLAUDE.md` Gotchas section
contains guidance on `DEFAULT_OAUTH_SCOPES` changes (Developer Console update + CHANGELOG entry).
Pinned by: `src/cli/auth/tests/mod.rs::default_oauth_scopes_pins_the_full_set_with_offline_access`.

**AC-017** (release-gate). `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`,
`scripts/check-spec-counts.sh` all pass. No new `unsafe` blocks. No new clippy allows.
Existing tests `tests/issue_create_json.rs`, `tests/issue_commands.rs`,
`tests/issue_write_holdouts.rs` all pass without modification.

**AC-018** (traces to BC-3.8.001 — mutation testing scope updated).
`.cargo/mutants.toml` `examine_globs` is extended to add:
`"src/api/jsm/requests.rs"`, `"src/api/jsm/request_types.rs"`, `"src/cli/requesttype.rs"`.
`docs/specs/cargo-mutants-policy.md` "Scope" section updated to list the three new files.
Pinned by: updated `mutants.toml` in this PR.
(Formerly AC-017 before absorption of OAuth scope work from pr3-scope.)

**AC-019** (traces to BC-3.8.011 — platform-only flag warnings on JSM path). When
`--request-type <NAME|ID>` is set on `jr issue create`, each of `--team`, `--points`,
`--parent`, `--to`, `--account-id` (if set) emits exactly ONE stderr warning line
with the verbatim BC-3.8.011 wording for that flag; JSM dispatch proceeds normally
to exit 0 on success. Generalizes the BC-3.8.010 `--type` warning pattern to all
platform-only flags. Idempotent — passing the same flag twice emits ONE warning.
Pinned by `tests/issue_create_jsm.rs::test_jsm_create_team_flag_emits_warning_with_request_type`,
`test_jsm_create_points_flag_emits_warning_with_request_type`,
`test_jsm_create_parent_flag_emits_warning_with_request_type`,
`test_jsm_create_to_flag_emits_warning_with_request_type`,
`test_jsm_create_account_id_flag_emits_warning_with_request_type` (5 tests).

[NEW 2026-05-19 issue #288 pr4 adversary-pass-03 M-01] AC-019 added to fill the
gap between BC-3.8.011 (codified in pass-01 C-02) and the story's AC list.
Frontmatter bc_anchors and BC table already referenced BC-3.8.011; only the
AC anchor was missing.

## Implementation Tasks

- [ ] **Prerequisites check:** Confirm pr1 and pr2 merged (pr3-scope dropped; scope addition lands in this PR).

- [ ] **OAuth scope addition (absorbed from former S-288-pr3-scope):**
  - Add `write:servicedesk-request ` to `DEFAULT_OAUTH_SCOPES` in `src/api/auth.rs` (line ~59).
  - Update `default_oauth_scopes_pins_the_full_set_with_offline_access` test in
    `src/cli/auth/tests/mod.rs` to include `write:servicedesk-request` in the expected set.
  - Add the following sentence to the Gotchas section of `CLAUDE.md` near the embedded OAuth
    entry: "When changing `DEFAULT_OAUTH_SCOPES`: (1) update the embedded `jr` OAuth app's
    permissions in the Atlassian Developer Console at
    https://developer.atlassian.com/console/myapps/ before tagging the release, and (2) add a
    CHANGELOG entry mentioning the re-consent prompt so users aren't surprised. Existing access
    tokens continue working with old scopes until expiry; new logins and refresh-token mints
    trigger re-consent."
  - Add CHANGELOG entry under the unreleased section: "OAuth: added
    `write:servicedesk-request` scope for JSM request creation (issue #288). Existing users
    will be prompted to re-consent on next login."
  - **Before tagging the release:** update the embedded `jr` OAuth app's permissions in the
    Atlassian Developer Console at https://developer.atlassian.com/console/myapps/ to include
    `write:servicedesk-request`. This is a manual step outside the PR — confirm in the PR
    description before merging.

- [ ] Add `--request-type`, `--field`, `--on-behalf-of` fields to `IssueCommand::Create`
      in `src/cli/mod.rs`. Use `Option<String>`, `Vec<String>` (repeatable), `Option<String>`
      types respectively with appropriate clap `#[arg(...)]` annotations.
- [ ] Update `src/cli/issue/mod.rs` pattern destructure to include the three new fields.
- [ ] Write RED tests in `tests/issue_create_jsm.rs` covering AC-001..AC-015 before
      any implementation (TDD Red Gate — all tests should fail at this point).
- [ ] Extract `parse_field_kv(args: &[String]) -> Result<HashMap<String, Value>, JrError>`
      as a standalone function in `src/cli/issue/create.rs`. Add `mod proptests` block with
      A.1–A.4 proptest properties. `cargo test --lib` must show the proptests red (they can't
      pass until `parse_field_kv` is implemented).
- [ ] Extract `build_jsm_request_body(...)` as a pure helper in `src/api/jsm/requests.rs`
      (no `JiraClient` dependency). Add `mod proptests` block with C.1–C.3.
- [ ] Implement `handle_jsm_create` helper in `src/cli/issue/create.rs`:
      1. `require_service_desk(client, project, "--request-type")` with call-site label
      2. Resolve `request_type` arg: if numeric → use as-is; else → cache read / API call / `partial_match`
      3. Build `requestFieldValues` from `--summary`, `--description` (ADF), `--priority`, `--label`, `--field`
      4. If `--type` also set → emit stderr warning (BC-3.8.010); do NOT return error
      5. Build body with `build_jsm_request_body(...)`
      6. POST via `client.create_jsm_request(body)`
      7. Emit `{"key": "<issue_key>"}` on stdout
- [ ] Add early dispatch branch in `handle_create`:
      ```rust
      if request_type.is_some() {
          return handle_jsm_create(client, config, /* args */).await;
      }
      ```
      The branch must come AFTER flag parsing but BEFORE any platform-path code executes.
- [ ] Implement proptest properties A.1–A.4 and C.1–C.3. Verify all 7 proptests green.
- [ ] Update `.cargo/mutants.toml` examine_globs to add 3 new files (AC-017).
- [ ] Update `docs/specs/cargo-mutants-policy.md` to list the 3 new files.
- [ ] Update `CLAUDE.md` with new flags documentation and the OAuth scope-change guidance
      note (per the OAuth scope addition task above).
- [ ] Run `cargo test` — all tests green including regression guards.
- [ ] Run `cargo clippy -- -D warnings` and `cargo fmt --check`.
- [ ] Run `scripts/check-spec-counts.sh` — must exit 0.

## Testing Strategy

**Primary test file:** `tests/issue_create_jsm.rs` — wiremock integration tests.

Key wiremock setup for each test:
1. Mount service desk resolution mock (`GET /rest/servicedeskapi/servicedesk`)
2. Mount request type list mock (`GET /rest/servicedeskapi/servicedesk/{id}/requesttype`)
   only for name-resolution tests; bypass for numeric ID tests
3. Mount JSM create mock (`POST /rest/servicedeskapi/request`) with `expect(1)`
4. Mount platform create mock (`POST /rest/api/3/issue`) with `expect(0)` to verify
   the platform endpoint is never called (AC-001 holdout guard)

**Proptest blocks (inline, not separate file):**
- `src/cli/issue/create.rs::mod proptests` — A.1–A.4 (parse_field_kv)
- `src/api/jsm/requests.rs::mod proptests` — C.1–C.3 (build_jsm_request_body)

Use `prop_assert!` / `prop_assert_eq!` (NOT plain `assert!`) — matching existing convention.

**Subprocess tests for stderr/exit-code verification:**
- H-NEW-JSM-RT-004 (--type warning to stderr): use the subprocess/binary test pattern if
  the test needs to verify exact stderr output
- H-NEW-JSM-RT-003 (401 hint): library-level test is sufficient (mock returns 401; assert
  error contains `write:servicedesk-request`)

**Regression guard:**
- `tests/issue_create_json.rs` — zero changes, must pass
- `tests/issue_commands.rs` — zero changes, must pass
- `tests/issue_write_holdouts.rs` — zero changes, must pass

## Architecture Compliance Rules

- **The dispatch fork gate MUST be `request_type.is_some()`** — no project-type detection,
  no additional HTTP round-trips at the gate (ADR-0014 Option C).
- **`handle_create` platform path MUST be structurally unchanged** when `--request-type` is
  absent. The early-return branch ensures this: `if request_type.is_some() { return ... }`.
  Any modification to the existing variable bindings, defaults, or order of operations in
  the platform path is a regression.
- **`parse_field_kv` MUST be extracted** as a standalone function — not inlined. This is
  a F6 prerequisite per the verification delta (F6 handoff checklist item 1).
- **`build_jsm_request_body` MUST be extracted** — no `JiraClient` dependency on the
  body-building function. Required for proptest C.1–C.3.
- **`partial_match::partial_match` MUST be reused** for name→ID resolution (not bespoke).
  F5 adversarial review will explicitly check for non-reuse.
- **`--no-input` parity:** `MatchResult::Ambiguous` → `JrError::UserError` (exit 64 clean,
  no prompt). Mirror `cli/queue.rs` `Ambiguous` arm exactly.
- No `unsafe` blocks. No `#[allow(...)]` suppressions.

**Forbidden behaviors for the platform path:**
- Platform path MUST NOT call `require_service_desk`
- Platform path MUST NOT call `list_request_types` or `get_request_type_fields`
- Platform path MUST NOT emit the `--type` warning (warning is JSM-path-only)

## Library and Framework Requirements

All versions already pinned in `Cargo.toml`. No new crate dependencies.

| Library | Purpose in this PR |
|---------|-------------------|
| `proptest` (dev, already used) | A.1–A.4, C.1–C.3 proptest properties |
| `wiremock` (dev, already used) | Integration test mock server |
| `serde_json` | Body construction + output |

Version pins from `Cargo.toml` are canonical — do NOT update any dependency versions in
this PR.

## File Structure Requirements

```
src/cli/mod.rs               MODIFIED — add --request-type, --field, --on-behalf-of to Create variant
src/cli/issue/mod.rs          MODIFIED — update Create pattern destructure for 3 new fields
src/cli/issue/create.rs       MODIFIED — early dispatch branch; handle_jsm_create helper;
                                          parse_field_kv pure function; mod proptests (A.1–A.4)
src/api/jsm/requests.rs       MODIFIED — build_jsm_request_body pure helper; mod proptests (C.1–C.3)

tests/issue_create_jsm.rs     NEW — comprehensive JSM create integration tests (AC-001..AC-015)

.cargo/mutants.toml           MODIFIED — add 3 new files to examine_globs
docs/specs/cargo-mutants-policy.md  MODIFIED — list new files in Scope section
.github/PULL_REQUEST_TEMPLATE.md    NEW (or MODIFIED) — BC-1.3.023 Developer Console checklist item
CLAUDE.md                     MODIFIED — new flags, --type interaction, proptest properties
```

## Token Budget Estimate

| Context item | Estimated tokens |
|---|---|
| This story | ~2,800 |
| `src/cli/issue/create.rs` (full file — platform path context required) | ~5,000 |
| `src/cli/mod.rs` (IssueCommand::Create variant to extend) | ~800 |
| `src/cli/issue/mod.rs` (pattern destructure) | ~400 |
| `src/api/jsm/requests.rs` (build_jsm_request_body to add) | ~600 |
| BC-3.8.001..010 (bc-3-issue-write.md §3.8) | ~600 |
| BC-3.3.001 (bc-3-issue-write.md — platform path regression baseline) | ~300 |
| BC-1.3.023 and BC-X.3.005 (scope mismatch path) | ~400 |
| ADR-0014 (dispatch fork rationale) | ~400 |
| `src/cli/queue.rs` (partial_match + require_service_desk pattern precedent) | ~800 |
| pr1, pr2 stories (context for what was built) | ~700 |
| `src/api/auth.rs` DEFAULT_OAUTH_SCOPES constant + pin test | ~300 |
| verification-delta (proptest A.1–A.4, C.1–C.3 specs) | ~800 |
| **Total** | **~13,900** |

This is the largest story in the cycle but within 20-30% of agent context window.
If the implementer finds `create.rs` context too heavy, consider loading only the
`handle_create` function body (lines ~350-end) rather than the full file.

## Previous Story Intelligence

pr1 (API layer) established:
1. `JiraClient::create_jsm_request(body: serde_json::Value)` — takes a pre-built body
2. `JsmRequestCreated.issue_key` — use this field to emit `{"key": "<issue_key>"}`
3. `build_jsm_request_body` — you're adding this pure helper to `api/jsm/requests.rs`

pr2 (CLI + cache) established:
1. Cache family 1: `read_request_type_cache` / `write_request_type_cache` — use these
   for the name→ID resolution cache read in `handle_jsm_create`
2. `require_service_desk` now takes `context_label: &'static str` — pass `"--request-type"`
   as the label (BC-3.8.002 specifies this call-site message)
3. `partial_match` wiring pattern: `MatchResult::Exact(rt)` → proceed; `Ambiguous` → exit 64;
   `None` → exit 64 — use the same arm structure as `cli/requesttype.rs`

**Note: pr3-scope was dropped.** The OAuth scope addition is performed in this PR (see
Implementation Tasks above). The Developer Console update is a manual release-gate step
confirmed in the PR description before merge. Research at
`.factory/research/issue-288-oauth-scope-coordination.md` validated that a separate PR
was disproportionate for a two-line code change.

Key lesson from `handle_create` review (F1 delta analysis):
- Adding fields to `IssueCommand::Create` requires updating ALL exhaustive pattern matches
  on that variant in the codebase. The Rust compiler enforces this — but be systematic:
  `src/cli/issue/mod.rs` and any other match sites (search codebase for `IssueCommand::Create`).
- The early-return branch goes at the TOP of `handle_create`, before any platform-specific
  code, to ensure the platform path is structurally unmodified.

## Risks / Notes

- **OAuth re-consent prompt for existing users on next login.** Adding
  `write:servicedesk-request` to `DEFAULT_OAUTH_SCOPES` triggers a re-consent dialog the
  next time a user authenticates or mints a new token. Atlassian handles the UX; existing
  access tokens continue working with old scopes until expiry. Mitigated by the CHANGELOG
  entry (AC-016) and the manual Developer Console update gate in the PR description.
- **`handle_create` is 1,601 LOC.** Read the full function context before modifying.
  Pay particular attention to where `request_type` is destructured and where the early
  branch should be inserted. The existing `--type` (issue type) variable binding must NOT
  be altered for the platform path.
- **`--type` vs. `--request-type` naming:** `--type` is the existing flag (issue type for
  platform create). `--request-type` is the new flag (JSM request type). Do NOT confuse
  these in variable names or comments.
- **`--type` warning semantics (BC-3.8.010):** The warning is emitted before the dispatch is
  issued; per BC-3.8.010, the warning may or may not fire if the command early-exits on a
  downstream validation error (`--summary` missing, ambiguous type, etc.). The implementation
  fires warnings pre-dispatch, which is BC-compliant.
- **F5 adversarial will check:** (1) Does `--type` flag silently swallow without warning?
  (2) Does `--field NAME=VALUE` correctly build `requestFieldValues` for multi-value fields?
  (3) Is `raiseOnBehalfOf` at the top level (NOT in `requestFieldValues`)?
- **`raiseOnBehalfOf` placement:** It is a TOP-LEVEL field in the JSM request body, NOT
  inside `requestFieldValues`. If placed inside `requestFieldValues`, the JSM API will
  ignore or error on it.
- **labels wire shape:** BC-3.8.007 confirmed `labels` is a plain string array `["alpha","beta"]`
  — NOT `[{"name":"alpha"}]`. The object-array concern was RESOLVED in F2.

## Out of Scope

- `jr requesttype` commands (pr2 — prerequisite)
- JSM SLA management, approval workflows (separate issues)
- Multi-value `requestFieldValues` array vs. scalar disambiguation beyond first `=` split
  (implementation detail deferred to a follow-up issue if complex)
- JSON output `_meta: {version: N}` envelope (NFR-O-P: deferred to v2)

## Done When

- `cargo test --test issue_create_jsm` exits 0 (all ACs green)
- `cargo test --test issue_create_json` exits 0 (platform path regression guard)
- `cargo test --test issue_commands` exits 0 (platform path regression guard)
- `cargo test` exits 0 (full suite including proptests)
- `cargo clippy -- -D warnings` exits 0
- `cargo fmt --all -- --check` exits 0
- `scripts/check-spec-counts.sh` exits 0
- `cargo mutants --in-diff` on the PR diff achieves ≥90% kill rate OR surviving mutants
  are individually whitelisted with justification comments
- AC-016 green: `write:servicedesk-request` in `DEFAULT_OAUTH_SCOPES`; pin test passes; CHANGELOG entry present
- All 4 holdout scenarios (H-NEW-JSM-RT-001..004, H-NEW-JSM-RT-005 via pr2) are green
- PR closes GitHub issue #288
- PR labels: `feat`, `jsm`
- Target branch: `develop`

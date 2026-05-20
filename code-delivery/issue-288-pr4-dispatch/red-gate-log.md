---
document_type: red-gate-log
level: ops
version: "1.0"
status: complete
producer: test-writer
timestamp: 2026-05-18T00:00:00
phase: 3
inputs:
  - .factory/code-delivery/issue-288-pr4-dispatch/story.md
  - src/api/jsm/requests.rs
  - src/cli/issue/create.rs
  - src/cli/auth/tests/mod.rs
  - tests/requesttype_commands.rs
traces_to: "S-288-pr4-dispatch"
stub_compile_verified: true
red_gate_verified: true
---

# Red Gate Log: S-288-pr4-dispatch — `jr issue create --request-type` Dispatch Fork

## Summary

| Story | Tests Written | All Fail (Red)? | Gate |
|-------|---------------|-----------------|------|
| S-288-pr4-dispatch (AC-001..016) | 17 integration + 8 lib (4 proptest parse_field_kv + 3 proptest build_jsm_request_body + 1 OAuth scope pin) = 25 total | 23 FAIL, 2 pass (AC-002 regression guards for platform path — correctly green) | PASS |

## Test Count by AC

| AC | Test(s) | Location | Red Gate Status |
|----|---------|----------|-----------------|
| AC-001 (BC-3.8.001, H-NEW-JSM-RT-001) | `test_jsm_create_happy_path_routes_to_servicedeskapi` | tests/issue_create_jsm.rs | FAIL (assertion: exit 0, got 101) |
| AC-002 (BC-3.3.001) | `test_jsm_create_without_request_type_uses_platform_path` | tests/issue_create_jsm.rs | PASS (platform path already works — correct) |
| AC-003 (BC-3.8.002, H-NEW-JSM-RT-002) | `test_jsm_create_non_jsm_project_exits_64_zero_http` | tests/issue_create_jsm.rs | FAIL (assertion: exit 64, got 101) |
| AC-004 (BC-3.8.003) | `test_jsm_create_ambiguous_request_type_exits_64` | tests/issue_create_jsm.rs | FAIL (assertion: exit 64, got 101) |
| AC-005 (BC-3.8.004) | `test_jsm_create_numeric_id_bypasses_name_lookup` | tests/issue_create_jsm.rs | FAIL (assertion: exit 0, got 101) |
| AC-006 (BC-3.8.005) | `test_jsm_create_summary_in_requestfieldvalues` | tests/issue_create_jsm.rs | FAIL (assertion: exit 0, got 101) |
| AC-007 (BC-3.8.006) | `test_jsm_create_description_is_adf_with_is_adf_request_true` + `test_jsm_create_plain_description_absent_when_no_description_flag` | tests/issue_create_jsm.rs | FAIL (assertion: exit 0, got 101) |
| AC-008 (BC-3.8.007) | `test_jsm_create_priority_and_labels_mapped` | tests/issue_create_jsm.rs | FAIL (assertion: exit 0, got 101) |
| AC-009 (BC-3.8.008) | `test_jsm_create_field_first_equals_split_and_duplicate_last_wins` + `test_jsm_create_field_missing_equals_exits_64` + 4 proptests A.1–A.4 | tests/issue_create_jsm.rs + src/cli/issue/create.rs::parse_field_kv_proptests | FAIL (assertion + todo!() panic) |
| AC-010 (BC-3.8.009) | `test_jsm_create_on_behalf_of_injected_at_top_level` + `test_jsm_create_on_behalf_of_absent_when_not_set` | tests/issue_create_jsm.rs | FAIL (assertion: exit 0, got 101) |
| AC-011 (BC-3.8.010, H-NEW-JSM-RT-004) | `test_jsm_create_type_flag_ignored_with_warning` | tests/issue_create_jsm.rs | FAIL (assertion: exit 0, got 101) |
| AC-012 (BC-1.3.023, BC-X.3.005, H-NEW-JSM-RT-003) | `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (repurposed in place (renamed from its pre-#384 name by S-384 F4)) + `test_platform_create_401_no_jsm_scope_hint` | tests/issue_create_jsm.rs | FAIL + PASS (at S-288 red-gate time the test was FAIL because assertions expected `write:servicedesk-request`; negative guard correctly passes; subsequent S-384 repurpose flipped the assertions to API-token-expiry hint) |
| AC-013 (BC-3.8.008) | 4 proptests (prop_parse_field_kv_*) | src/cli/issue/create.rs::parse_field_kv_proptests | FAIL (todo!() panic in unimplemented stub) |
| AC-014 (BC-3.8.001+BC-3.8.009) | 3 proptests (prop_build_jsm_request_body_*) | src/api/jsm/requests.rs::proptests | FAIL (todo!() panic in unimplemented stub) |
| AC-015 (BC-3.8.001) | `test_jsm_create_output_json_shape_matches_platform` | tests/issue_create_jsm.rs | FAIL (assertion: exit 0, got 101) |
| AC-016 (BC-1.3.023) | `test_default_oauth_scopes_include_servicedesk_request` | src/cli/auth/tests/mod.rs | FAIL (write:servicedesk-request not yet in DEFAULT_OAUTH_SCOPES) |
| AC-017 | Release gate — no test | n/a | n/a |
| AC-018 | mutants.toml scope — no test | n/a | n/a |

## Representative Failure Messages

### Integration tests (all JSM dispatch tests):
```
assertion `left == right` failed: BC-3.8.001: expected exit 0, got Some(101).
```
All integration tests see exit code 101 (Rust panic exit code) because the binary
panics at `todo!("S-288-pr4 Step 4")` inside `handle_jsm_create`. The test assertion
("expected exit 0") fires as the actual test failure — this is an assertion error, not
a test harness crash. ✓

### AC-016 (OAuth scope pin):
```
cli::auth::tests::test_default_oauth_scopes_include_servicedesk_request --- FAILED
assertion failed: BC-1.3.023: DEFAULT_OAUTH_SCOPES must include write:servicedesk-request
for JSM dispatch; got: "read:jira-work write:jira-work read:jira-user
read:servicedesk-request read:cmdb-object:jira read:cmdb-schema:jira offline_access"
```
The scope `write:servicedesk-request` is NOT yet in `DEFAULT_OAUTH_SCOPES`. This is the
expected Red Gate state — Step 4 adds it. ✓

### Proptest failures (AC-013, AC-014):
All 7 proptest properties fail with a `todo!()` panic originating from the
unimplemented stub bodies. This is the expected Red Gate failure mode for inline
proptest harnesses that call stubs directly. The proptest runner catches the panic and
reports it as a test failure.

Note: per Red Gate discipline, proptest panics (not assertion errors) are an accepted
failure mode for stub-calling unit tests. The integration tests in
`tests/issue_create_jsm.rs` all fail with assertion errors (the preferred form).

## Two Tests That PASS (Correct Behavior)

1. `test_jsm_create_without_request_type_uses_platform_path` — correctly passes
   because the platform path (no `--request-type`) is fully implemented and
   unmodified by the stubs. This is the BC-3.3.001 regression baseline test.

2. `test_platform_create_401_no_jsm_scope_hint` — correctly passes because the
   platform 401 handling does not emit `write:servicedesk-request` (the negative
   regression guard for AC-012).

Both passing tests are regression guards for pre-existing behavior — they should
remain green throughout Step 4 and Step 5.

## NO todo!() Panics in Integration Tests

Confirmed: All 15 failing integration tests in `tests/issue_create_jsm.rs` fail
with:
```
assertion `left == right` failed: BC-X.X.XXX: expected exit 0, got Some(101).
```
or:
```
assertion `left == right` failed: BC-X.X.XXX: expected exit 64, got Some(101).
```

Exit code 101 is Rust's panic exit code from the binary hitting `todo!()`. The test
assertion fires BEFORE any `todo!()` text appears in the test harness output itself.
None of the integration tests show raw "not yet implemented" in the harness — they
show assertion failures referencing the BC behavior under test.

## Stubs on Branch (from Step 2)

- `src/cli/issue/create.rs::parse_field_kv` — `todo!("S-288-pr4 Step 4")`
- `src/cli/issue/create.rs::handle_jsm_create` — `todo!("S-288-pr4 Step 4")`
- `src/api/jsm/requests.rs::build_jsm_request_body` — `todo!("S-288-pr4 Step 4")`

## Regression Check

| Existing Tests | Status |
|----------------|--------|
| `tests/requesttype_commands.rs` (15 tests) | all pass |
| `tests/queue.rs` (from pr2) | not re-run here but no source changes |
| `tests/project_meta.rs` | pass |
| `cargo test --lib` (696 pre-existing lib tests) | all pass |

## Hand-Off to Implementer

Stories ready for implementation: S-288-pr4-dispatch (Step 4)

Implementation guidance:
1. Implement `parse_field_kv` in `src/cli/issue/create.rs` — first-equals split,
   last-wins for duplicates, `JrError::UserError` for missing `=`.
2. Implement `build_jsm_request_body` in `src/api/jsm/requests.rs` — pure function,
   no `JiraClient`, per BC-3.8.005..009.
3. Implement `handle_jsm_create` in `src/cli/issue/create.rs` — service desk
   resolution, name→ID resolution via `partial_match`, body construction, POST,
   and `{"key": "..."}` output.
4. Add `"write:servicedesk-request"` to `DEFAULT_OAUTH_SCOPES` in `src/api/auth.rs`
   AND update the existing `default_oauth_scopes_pins_the_full_set_with_offline_access`
   pin test in `src/cli/auth/tests/mod.rs` to include the new scope.
5. Remove all `#[allow(unused_variables)]` / `#[allow(dead_code)]` / `#[allow(clippy::too_many_arguments)]`
   Step 4 TODOs from the stubs after implementing.

Make each failing test pass one at a time. AC-002 and the AC-012 negative guard
must remain green throughout.

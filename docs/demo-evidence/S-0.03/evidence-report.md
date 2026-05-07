# Demo Evidence Report — S-0.03

**Story:** Fix multi-workspace asset HashMap to use composite `(workspace_id, oid)` key  
**Story ID:** S-0.03  
**Branch:** fix/multi-workspace-asset-hashmap-key  
**Date recorded:** 2026-05-07  
**Recording medium:** VHS (terminal recordings — CLI product)  
**Font:** Menlo (system default on macOS)

---

## Coverage Summary

| AC | Description | Test(s) | Recording | Result |
|----|-------------|---------|-----------|--------|
| AC-001 | Two-workspace OID collision — distinct labels, no last-write-wins (integration) | `test_bc_4_3_001_multi_workspace_no_collision` | AC-001-multi-workspace-no-collision.gif | PASS |
| AC-001 | Composite key preserves both workspace entries (unit) | `test_bc_4_3_001_bare_oid_key_collides_on_shared_oid` | AC-001-unit-composite-key-verification.gif | PASS |
| AC-002 | Single-workspace regression guard — output unchanged by fix | `test_bc_4_3_001_single_workspace_regression_guard` | AC-002-single-workspace-regression-guard.gif | PASS |
| AC-003 | `to_enrich` composite key (line 398) preserved and unchanged | `test_bc_4_3_001_to_enrich_composite_key_unchanged` | AC-003-to-enrich-composite-key-unchanged.gif | PASS |
| Combined | All 5 BC-4.3.001 tests (3 unit + 2 integration) | all 5 | AC-combined-all-bc-4-3-001-pass.gif | 5/5 PASS |

---

## AC-001: Two-Workspace OID Collision (Integration)

**Acceptance Criterion:** Given wiremock setup with two workspaces `ws-A` and `ws-B`, both
containing an asset with `oid = "OBJ-88"` but different names (`"Acme Corp"` for ws-A,
`"Widgets Inc"` for ws-B), when `jr issue list --project PROJ --output json` is executed with
issues linked to both workspace assets, each issue's asset field must show the correct
workspace-specific name (no last-write-wins collision).

**Test:** `test_bc_4_3_001_multi_workspace_no_collision` in `tests/issue_list_assets.rs`

**Recordings:**
- `AC-001-multi-workspace-no-collision.gif` (127 KB)
- `AC-001-multi-workspace-no-collision.webm` (348 KB)
- `AC-001-multi-workspace-no-collision.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test test_bc_4_3_001_multi_workspace_no_collision ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.76s
```

---

## AC-001: Composite Key Unit Verification

**Acceptance Criterion:** A `HashMap<String, _>` (bare OID key) loses data when two workspaces
share the same OID, exhibiting last-write-wins. A `HashMap<(String, String), _>` (composite
workspace_id + oid key) preserves both workspace entries.

**Test:** `test_bc_4_3_001_bare_oid_key_collides_on_shared_oid` in `src/cli/issue/list.rs` (unit)

**Recordings:**
- `AC-001-unit-composite-key-verification.gif` (130 KB)
- `AC-001-unit-composite-key-verification.webm` (357 KB)
- `AC-001-unit-composite-key-verification.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test cli::issue::list::tests::test_bc_4_3_001_bare_oid_key_collides_on_shared_oid ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 609 filtered out; finished in 0.00s
```

---

## AC-002: Single-Workspace Regression Guard

**Acceptance Criterion:** Single-workspace tenants are unaffected: when all assets are from one
workspace, the output is identical to pre-fix behavior. The composite key change is transparent
to tenants with one workspace.

**Test:** `test_bc_4_3_001_single_workspace_regression_guard` in `tests/issue_list_assets.rs`

**Recordings:**
- `AC-002-single-workspace-regression-guard.gif` (136 KB)
- `AC-002-single-workspace-regression-guard.webm` (361 KB)
- `AC-002-single-workspace-regression-guard.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test test_bc_4_3_001_single_workspace_regression_guard ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.63s
```

---

## AC-003: `to_enrich` HashMap Preserved

**Acceptance Criterion:** The `to_enrich: HashMap<(String, String), ()>` at `list.rs:398` is
already correct (uses composite key) and must not be modified by this fix. This guard confirms
the fix targeted only the `resolved` HashMap at lines 446/449/456.

**Test:** `test_bc_4_3_001_to_enrich_composite_key_unchanged` in `src/cli/issue/list.rs` (unit)

**Recordings:**
- `AC-003-to-enrich-composite-key-unchanged.gif` (130 KB)
- `AC-003-to-enrich-composite-key-unchanged.webm` (352 KB)
- `AC-003-to-enrich-composite-key-unchanged.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test cli::issue::list::tests::test_bc_4_3_001_to_enrich_composite_key_unchanged ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 609 filtered out; finished in 0.00s
```

---

## Combined: All 5 BC-4.3.001 Tests Pass

**Recordings:**
- `AC-combined-all-bc-4-3-001-pass.gif` (690 KB)
- `AC-combined-all-bc-4-3-001-pass.webm` (1.2 MB)
- `AC-combined-all-bc-4-3-001-pass.tape` (VHS script source)

**Evidence (captured test output):**
```
running 3 tests
test cli::issue::list::tests::test_bc_4_3_001_bare_oid_key_collides_on_shared_oid ... ok
test cli::issue::list::tests::test_bc_4_3_001_composite_key_preserves_both_workspaces ... ok
test cli::issue::list::tests::test_bc_4_3_001_to_enrich_composite_key_unchanged ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 607 filtered out; finished in 0.00s

running 2 tests
test test_bc_4_3_001_multi_workspace_no_collision ... ok
test test_bc_4_3_001_single_workspace_regression_guard ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s
```

---

## Quality Gates

All gates verified clean after demo recordings (no source changes):

| Gate | Result |
|------|--------|
| `cargo build` | clean |
| `cargo test` | green (5 BC-4.3.001 tests passed, 0 failed) |
| `cargo clippy -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |

---

## Traceability

| Recording | AC | BC Anchor | H-036 Outcome |
|-----------|----|-----------|---------------|
| AC-001-multi-workspace-no-collision | AC-001 | BC-4.3.001 | MUST-PASS (was MUST-FAIL at dea1664) |
| AC-001-unit-composite-key-verification | AC-001 | BC-4.3.001 | MUST-PASS |
| AC-002-single-workspace-regression-guard | AC-002 | BC-4.3.001 | MUST-PASS |
| AC-003-to-enrich-composite-key-unchanged | AC-003 | BC-4.3.001 | MUST-PASS |
| AC-combined-all-bc-4-3-001-pass | AC-001/002/003 | BC-4.3.001 | 5/5 MUST-PASS |

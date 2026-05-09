# S-3.10 Demo Evidence Report

Story: Rewrite format_roundtrip proptest + delete deprecated 3-arg parse_duration + retire H-018
Branch: refactor/s-3-10-cleanup-deprecated-parse-duration
Type: REFACTOR + CLEANUP (no user-visible behavior change)

## Per-AC Evidence

---

### AC-001 — Rewritten format_roundtrip proptest passes structurally

- Demo: `AC001_format_roundtrip_passes.gif` (VHS recording of cargo test)
- Webm: `AC001_format_roundtrip_passes.webm`
- Tape: `AC-001-format-roundtrip-passes.tape`
- Command: `cargo test --lib -- duration::proptests::format_roundtrip`
- Expected output: `test duration::proptests::format_roundtrip ... ok` / `test result: ok. 1 passed`
- Verdict: **PASS**

---

### AC-002 — 3-arg parse_duration calculator deleted

- Demo: `AC002_parse_duration_deletion_verified.gif` (VHS recording of rg search)
- Webm: `AC002_parse_duration_deletion_verified.webm`
- Tape: `AC-002-parse-duration-deletion-verified.tape`
- Command: `rg -n 'parse_duration' src/`
- Captured output (no 3-arg calculator references — only validator):

```
src/api/jira/worklogs.rs:13:    /// function (use `duration::parse_duration_validate`).
src/duration.rs:5:/// Maximum byte length accepted by `parse_duration_validate`.
src/duration.rs:21:pub fn parse_duration_validate(input: &str) -> Result<()> {
src/duration.rs:108:    // WV2-SEC-01 regression pins: input length cap on parse_duration_validate
src/duration.rs:111:    fn test_parse_duration_validate_rejects_input_longer_than_max() {
src/duration.rs:113:        let result = parse_duration_validate(&too_long);
src/duration.rs:129:    fn test_parse_duration_validate_accepts_input_at_max_boundary() {
src/duration.rs:137:        let result = parse_duration_validate(&just_under_cap);
src/cli/worklog.rs:32:    duration::parse_duration_validate(dur)?;
```

All hits are `parse_duration_validate` (the production validator). The 3-arg `parse_duration(input, hours_per_day, days_per_week)` calculator is absent from `src/`.
- Verdict: **PASS**

---

### AC-003 — Rewritten proptest still passes; existing validator tests still pass (18 total)

- Demo: `AC003_all_duration_tests_pass.gif` (VHS recording of cargo test)
- Webm: `AC003_all_duration_tests_pass.webm`
- Tape: `AC-003-all-duration-tests-pass.tape`
- Command: `cargo test --lib -- duration`
- Expected output: 18 tests pass (5 duration module tests + 12 jql duration tests + 1 proptest)
- Actual output:

```
running 18 tests
test duration::tests::test_format_minutes ... ok
test duration::tests::test_format_hours_and_minutes ... ok
test duration::tests::test_format_hours ... ok
test duration::tests::test_parse_duration_validate_accepts_input_at_max_boundary ... ok
test jql::tests::validate_duration_empty ... ok
test jql::tests::validate_duration_combined_units ... ok
test jql::tests::validate_duration_invalid_unit ... ok
test jql::tests::validate_duration_no_digits ... ok
test jql::tests::validate_duration_reversed ... ok
test duration::tests::test_parse_duration_validate_rejects_input_longer_than_max ... ok
test jql::tests::validate_duration_valid_days ... ok
test jql::tests::validate_duration_valid_hours ... ok
test jql::tests::validate_duration_valid_minutes ... ok
test jql::tests::validate_duration_valid_months_uppercase ... ok
test jql::tests::validate_duration_valid_weeks ... ok
test jql::tests::validate_duration_valid_years ... ok
test jql::tests::validate_duration_valid_zero ... ok
test duration::proptests::format_roundtrip ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 602 filtered out; finished in 0.01s
```

- Verdict: **PASS**

---

### AC-004 — cargo test --all-targets passes

- Demo: `AC004_all_targets_pass.gif` (VHS recording of full test suite tail)
- Webm: `AC004_all_targets_pass.webm`
- Tape: `AC-004-all-targets-pass.tape`
- Command: `cargo test --all-targets 2>&1 | tail -20`
- Final lines from full run:

```
running 9 tests
test test_bc_x_5_002_empty_issue_returns_zero_items ... ok
test test_add_worklog ... ok
test test_list_worklogs ... ok
test test_bc_x_5_002_single_page_no_extra_fetch ... ok
test test_bc_x_5_002_two_page_result_returns_all_80_items ... ok
test test_bc_x_5_002_both_pages_fetched ... ok
test worklog_list_network_drop_surfaces_reach_error ... ok
test worklog_list_server_error_surfaces_friendly_message ... ok
test worklog_list_unauthorized_dispatches_reauth_message ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/worklog_duration_holdouts.rs (...)

running 6 tests
test test_s_2_06_ac_004_bc_x_5_009_parse_duration_validator_unit ... ok
...
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

- Verdict: **PASS**

---

### AC-005 — H-018 deleted; total_holdouts 51 → 50

- Type: Static evidence (factory-artifacts commit `4250e2c`)
- Command: `git -C .factory show 4250e2c -- specs/prd/holdout-scenarios.md`
- Relevant diff hunk:

```diff
--- a/specs/prd/holdout-scenarios.md
+++ b/specs/prd/holdout-scenarios.md
@@ -1,7 +1,7 @@
 ---
 context: holdout-scenarios
 title: "Holdout Scenarios"
-total_holdouts: 51
+total_holdouts: 50
 ...
 
-51 holdout scenarios for Phase 4 evaluation.
+50 holdout scenarios for Phase 4 evaluation.

@@ -190,18 +192,6 @@ Setup uses:
-### H-018: Duration validator accepts combined units
-**Setup**: none.
-**BC**: BC-X.5.005 (post-S-2.06 v2.0.0)
-**Difficulty**: easy
-**Action**: invoke `duration::parse_duration_validate("1w2d3h30m")` ...
-**Expected**: `parse_duration_validate("1w2d3h30m")` → `Ok(())` ...
```

H-018 block (18 lines) fully deleted. `total_holdouts` decremented from 51 to 50.
- Verdict: **PASS**

---

### AC-006 — BC-INDEX.md BC-X.5.005 row updated

- Type: Static evidence (factory-artifacts commit `4250e2c`)
- Command: `git -C .factory show 4250e2c -- specs/prd/BC-INDEX.md`
- Relevant diff hunk:

```diff
--- a/specs/prd/BC-INDEX.md
+++ b/specs/prd/BC-INDEX.md
@@ -558,7 +558,7 @@ R1/R4 prefix = deepening round that introduced it.
-| BC-X.5.005 | Calculator (deprecated post-S-2.06 v2.0.0; kept only for `format_duration`
-  round-trip proptest) AND validator `parse_duration_validate("1w2d3h30m")` accept
-  combined units. Validator is the production path; old calculator has no production
-  caller. See `src/duration.rs` and DEC-010. | BC-505 | src/duration.rs::tests | HIGH |
+| BC-X.5.005 | Validator `parse_duration_validate("1w2d3h30m")` accepts combined units
+  — production path only. Note: the 3-arg parse_duration calculator was deleted in
+  S-3.10 (was used only by tests post-S-2.06). See `src/duration.rs` and DEC-010.
+  | BC-505 | src/duration.rs::tests | HIGH |
```

BC-X.5.005 row updated to remove calculator reference. Validator remains the sole documented path.
- Verdict: **PASS**

---

### AC-007 — cross-cutting.md BC-X.5.005 prose updated (both PRD + domain-spec)

- Type: Static evidence (factory-artifacts commit `4250e2c`)
- Command: `git -C .factory show 4250e2c -- specs/prd/cross-cutting.md specs/domain-spec/cross-cutting.md`

PRD diff (specs/prd/cross-cutting.md):

```diff
-#### BC-X.5.005: `parse_duration_validate("1w2d3h30m")` accepts combined units (validator);
-  `parse_duration("1w2d3h30m", 8, 5)` calculator preserved deprecated for `format_duration`
-  round-trip proptest
+#### BC-X.5.005: `parse_duration_validate("1w2d3h30m")` accepts combined units
+  (validator — production path only)

 **Behavior**: ... `parse_duration_validate("1w2d3h30m")` is the sole production path.
-  Note: ... calculator `parse_duration(s, 8, 5)` is deprecated, retained only for
-  `format_duration` round-trip proptest.
+  Note: the 3-arg `parse_duration(s, hours_per_day, days_per_week)` calculator was
+  deleted in S-3.10 — it had no production caller after S-2.06 v2.0.0 and was retained
+  only for the `format_duration` round-trip proptest, which has been rewritten to not
+  depend on it.
```

Domain-spec diff (specs/domain-spec/cross-cutting.md):

```diff
-**`parse_duration(input: &str, hours_per_day: u64, days_per_week: u64) -> Result<u64>`**:
-  DEPRECATED post-S-2.06 v2.0.0; kept only for `format_duration` round-trip proptest.
-  No production caller. Returns total seconds. Accepts combined units `1w2d3h30m`.
-  Case-insensitive (input is lowercased first). Callers previously hardcoded `8, 5`.
+(Note: the 3-arg `parse_duration(input, hours_per_day, days_per_week) -> Result<u64>`
+calculator was deleted in S-3.10. It had no production caller after S-2.06 v2.0.0
+... That proptest has been rewritten in S-3.10 to use `format_duration` directly
+without the calculator.)

-| INV-DUR-001 | `parse_duration` accepts combined units (`1w2d3h30m`), unlike `validate_duration`...
+| INV-DUR-001 | ~~`parse_duration` accepts combined units...~~ **DELETED S-3.10**: ...
-| INV-DUR-002 | `parse_duration` is case-insensitive...
+| INV-DUR-002 | ~~`parse_duration` is case-insensitive...~~ **DELETED S-3.10**: ...
-| INV-DUR-003 | `parse_duration` u64 overflow potential for pathological inputs...
+| INV-DUR-003 | ~~`parse_duration` u64 overflow potential...~~ **DELETED S-3.10**: ...
```

Both PRD and domain-spec cross-cutting.md updated. Calculator entity removed from domain-spec entity list. INV-DUR-001/002/003 marked DELETED.
- Verdict: **PASS**

---

### AC-008 — cargo build/test/clippy/fmt all exit 0

- Demo: `AC008_toolchain_gates_green.gif` (VHS recording of combined gate chain)
- Webm: `AC008_toolchain_gates_green.webm`
- Tape: `AC-008-toolchain-gates-green.tape`
- Command: `cargo build --all-targets 2>&1 | tail -1 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo fmt --all -- --check && echo ALL GREEN`
- Expected: Terminal output ends with `ALL GREEN`
- Individual gate results (verified in CI dry-run):
  - `cargo build --all-targets`: `Finished 'dev' profile ... target(s) in 0.12s`
  - `cargo clippy --all-targets -- -D warnings`: `Finished 'dev' profile ... target(s) in 0.13s` (0 warnings)
  - `cargo fmt --all -- --check`: exits 0 (no output = clean)
  - Combined chain: exits 0, prints `ALL GREEN`
- Verdict: **PASS**

---

### AC-009 — Holdout Retirement Policy clause added to preamble

- Type: Static evidence (factory-artifacts commit `4250e2c`)
- Command: `grep -A 2 "Holdout Retirement Policy" .factory/specs/prd/holdout-scenarios.md`
- Captured output:

```
**Holdout Retirement Policy (S-3.10):** Holdouts pin user-observable behavior. If the target
of a holdout becomes an internal helper with no production caller (i.e., no longer
user-observable), the holdout must be rewritten or retired in the same story that introduces
the deprecation, not deferred. This rule was codified after S-2.06 v1→v2 pivoted away from
the client-side parse_duration calculator without retiring H-018 in the same wave (gap closed
in S-3.10).

---
```

Policy clause present in preamble, before Group 1 holdouts. Rationale traces to S-2.06 gap.
- Verdict: **PASS**

---

## Summary

All 9 ACs have at least one demo artifact.

| AC | Type | Artifact | Verdict |
|----|------|----------|---------|
| AC-001 | VHS recording | `AC001_format_roundtrip_passes.gif` | PASS |
| AC-002 | VHS recording | `AC002_parse_duration_deletion_verified.gif` | PASS |
| AC-003 | VHS recording | `AC003_all_duration_tests_pass.gif` | PASS |
| AC-004 | VHS recording | `AC004_all_targets_pass.gif` | PASS |
| AC-005 | Static (git diff) | factory-artifacts@4250e2c holdout-scenarios.md | PASS |
| AC-006 | Static (git diff) | factory-artifacts@4250e2c BC-INDEX.md | PASS |
| AC-007 | Static (git diff) | factory-artifacts@4250e2c cross-cutting.md (x2) | PASS |
| AC-008 | VHS recording | `AC008_toolchain_gates_green.gif` | PASS |
| AC-009 | Static (grep) | factory-artifacts@4250e2c holdout-scenarios.md preamble | PASS |

Story delivers:
- 1 test refactor: `format_roundtrip` proptest rewritten without inverse function (structural assertion)
- 1 function deletion: 3-arg `parse_duration` calculator + 9 inline tests + 3 proptests = ~117 LOC deleted
- 4 spec file updates (factory-artifacts@4250e2c): holdout-scenarios.md, BC-INDEX.md, prd/cross-cutting.md, domain-spec/cross-cutting.md
- 1 process improvement: Holdout Retirement Policy clause codified

Total: 2 develop-branch commits + 1 factory-artifacts commit. Zero user-visible behavior change. All toolchain gates green.

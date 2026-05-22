# Phase F6 — Mutation Testing Results

**Feature:** S-388 / issue #388 — cross-hierarchy `edit --type` 400 enrichment + `--no-parent` fake-endpoint hint fix
**Delta commit:** `e0ea24b` (merged to `develop`)
**Date:** 2026-05-20
**Tool:** cargo-mutants 27.0.0
**Verdict:** PASS — 100% kill rate (target ≥ 90%)

## Run Configuration

```
cargo mutants --no-config --in-diff /tmp/s388.diff --jobs 2 \
  --test-tool=cargo --test-workspace=false --timeout-multiplier 3.0 \
  -- --lib --test issue_edit_type_errors --test issue_edit_no_parent
```

### Scope rationale

Because the S-388 delta is **already merged** to `develop` (commit `e0ea24b`),
the CLAUDE.md canonical command `--in-diff $(git diff origin/develop...HEAD)`
would yield an empty diff. The delta diff was reconstructed against the
pre-merge parent:

```
git diff a66d664..e0ea24b -- src/cli/issue/create.rs src/types/jira/issue.rs > /tmp/s388.diff
```

This is the policy-correct `--in-diff` scope (`docs/specs/cargo-mutants-policy.md`):
mutation testing narrowed to **lines changed by the S-388 delta only**, not the
whole 2,300-line `create.rs` file. cargo-mutants identified **8 mutants** within
the delta line ranges.

`--no-config` was passed deliberately to bypass `.cargo/mutants.toml::examine_globs`
(which targets the bulk + JSM modules from earlier hardening cycles, not S-388).
Without `--no-config`, the run picks up out-of-delta mutants in `src/cli/requesttype.rs`
and `src/api/jsm/*` that the S-388 test scope does not cover — contaminating the
denominator. The `--in-diff` filter is the precise S-388 scope.

The test invocation runs `--lib` (which includes the inline
`is_cross_hierarchy_type_error_proptests` proptest and the `mod tests` unit
tests in `create.rs`) plus the two S-388 integration test binaries
(`issue_edit_type_errors`, `issue_edit_no_parent`). The `multi_cloudid_disambiguation`
integration test is deliberately excluded — it fails under cargo-mutants' parallel
execution due to system-keychain contention (`"item already exists in the keychain"`),
a known environmental flake unrelated to S-388. Confirmed: that test passes 12/12 in
isolation.

## Baseline

```
ok  Unmutated baseline — 713 lib tests + 9 issue_edit_no_parent + 10 issue_edit_type_errors — all pass
```

The inline proptest `cli::issue::create::is_cross_hierarchy_type_error_proptests::prop_cross_hierarchy_decided_by_subtask_flag_mismatch`
ran in the baseline and passed.

## Results

| Outcome        | Count |
|----------------|-------|
| Caught         | 7     |
| **Missed**     | **0** |
| Unviable       | 1     |
| Timeout        | 0     |
| Total mutants  | 8     |

### Kill rate

**7 caught / 7 viable mutants = 100%** (≥ 90% target — PASS, ≥ 95% — PASS).

Unviable mutants are excluded from the kill-rate denominator per cargo-mutants
convention (they fail to compile, so they are not a test-quality signal).

## Caught Mutants (7/7)

| Location | Mutation | In-delta region |
|----------|----------|-----------------|
| `create.rs:274:5`  | `replace handle_edit -> Result<()> with Ok(())` | `handle_edit` signature (delta touches its error path) |
| `create.rs:864:67` | `replace == with != in handle_edit` | case-insensitive type-name match in the `--type` dispatch block |
| `create.rs:898:22` | `replace && with || in handle_edit` | `no_parent && is_subtask_parent_error(e)` dual-gate |
| `create.rs:1270:31`| `replace match guard a != b with true` | classifier `is_cross_hierarchy_type_error` |
| `create.rs:1270:31`| `replace match guard a != b with false` | classifier `is_cross_hierarchy_type_error` |
| `create.rs:1270:33`| `replace != with == in is_cross_hierarchy_type_error` | classifier guard operator |
| `create.rs:1271:9` | `delete match arm (Some(_), Some(_))` | classifier `SameCategory` arm |

**Notable:** the `&&`→`||` mutant at `create.rs:898:22` is **CAUGHT** — this is
the mutant PR #397's CI flagged, killed by the kill-test
`test_no_parent_non_subtask_400_does_not_surface_cross_hierarchy_hint` in
`tests/issue_edit_no_parent.rs` (which pins `no_parent=true` + non-subtask 400).
This run independently confirms PR #397's CI mutation result.

The four classifier mutants (`1270`/`1271`) are all killed by the inline
proptest `is_cross_hierarchy_type_error_proptests`, which exhaustively covers
the 9-state `Option<bool> × Option<bool>` domain — any guard inversion or arm
deletion changes the verdict for at least one of the 9 states the proptest
asserts.

## Unviable Mutant (1, excluded from denominator)

| Location | Mutation | Why unviable |
|----------|----------|--------------|
| `create.rs:1269:5` | `replace is_cross_hierarchy_type_error -> Classification with Default::default()` | `Classification` derives `Debug + PartialEq` only — it has **no `Default` impl**. The mutant does not compile, so cargo-mutants classifies it Unviable. This is correct and expected: a `Default::default()` substitution is only viable when the return type implements `Default`. Not a test-quality gap. |

## Surviving Mutants

**None.** Zero missed mutants. No new test-suite gaps in the S-388 delta. No
findings to route back to Phase F4.

## Conclusion

PASS. 100% kill rate on the 7 viable in-delta mutants (target ≥ 90%, and the
≥ 95% security-critical bar — although this delta is not security-critical —
is also cleared). The S-388 test suite (11 new/strengthened tests + the inline
proptest) kills every viable mutation of the delta code. The PR #397 CI
mutation result is independently reconfirmed.

# Phase F6 — Kani Formal Verification Results

**Feature:** S-388 / issue #388 — cross-hierarchy `edit --type` 400 enrichment + `--no-parent` fake-endpoint hint fix
**Delta commit:** `e0ea24b` (merged to `develop`)
**Date:** 2026-05-20
**Verdict:** JUSTIFIED SKIP

## Decision: Kani is not applicable to this delta

### 1. The project does not use Kani

`CLAUDE.md` "Build & Test" and "Conventions" sections enumerate the verification
stack: `cargo test` (unit + integration), `proptest` (property tests), `insta`
(snapshot tests), and `cargo-mutants` (mutation testing). Kani is not installed
and not part of the toolchain (`cargo kani --version` → not found). Introducing
Kani for a 181-line CLI error-classification delta is disproportionate.

### 2. The only new pure function has a finite, exhaustively-tested domain

The delta introduces exactly one pure function:

```rust
fn is_cross_hierarchy_type_error(
    src_subtask: Option<bool>,
    tgt_subtask: Option<bool>,
    _err: &str,
) -> Classification
```

The decision domain is `Option<bool> × Option<bool>` = **9 discrete states**.
The `_err: &str` parameter is provably unused for the verdict — the underscore
prefix and the rustdoc contract ("The `err` argument MUST NOT influence the
return value") make this structural, and the function body never references it.
There is no arithmetic, no array indexing, no unsafe code, no overflow surface —
i.e. none of the property classes Kani specializes in.

### 3. The existing inline proptest already verifies the domain exhaustively

`is_cross_hierarchy_type_error_proptests::prop_cross_hierarchy_decided_by_subtask_flag_mismatch`
generates each flag from `prop_oneof![Just(None), Just(Some(true)), Just(Some(false))]`.
With 3 values per flag the strategy enumerates all 9 states; default proptest
runs (256 cases) cover every state many times over. The proptest asserts:

- **P1** `CrossHierarchy` when `Some(a) != Some(b)`
- **P2** `SameCategory` when `Some(a) == Some(b)`
- **P3** `Indeterminate` when either flag is `None`
- **P4** err-independence — re-runs with a contrasting `err` (including the
  locale-fragile substring `"issue type selected is invalid"`) and asserts the
  verdict is unchanged.

An exhaustive proptest over a 9-state finite domain provides the same total
coverage a Kani `kani::any()` harness would, plus the P4 path-independence
property. Kani would add zero verification value here.

## Conclusion

Skipping Kani is justified on three independent grounds: the project has no
Kani toolchain, the only pure function has a finite 9-state domain with no
overflow/bounds/unsafe surface, and that domain is already exhaustively
verified by an inline proptest. No formal-verification gap remains.

---
document_type: adversarial-review-pass
feature: issue-396 (jr issue edit --field NAME=VALUE)
phase: F5 Scoped Adversarial Review
pass: 1
date: 2026-05-25
verdict: NOT-CLEAN
reviewer: adversary
---

# F5 Pass 1 — VERDICT: NOT-CLEAN

## Summary

1 HIGH finding (data-loss class). Several LOW observations.

---

## HIGH Findings

### HIGH-1: Silent-drop of `--label` + `--field` on platform non-JSM path

**Description:** When `jr issue edit KEY --label add:foo --field Severity=Critical` is
executed on a standard (non-JSM) platform path, the `--label` flag causes early dispatch
to `handle_edit_bulk_labels`, which does not accept `field_pairs`. The `--label` conflict
block at `create.rs:445-489` lists 11 sibling flags but does not list `--field`.

The block's own comment states: "Combining them would silently drop the non-label fields
(exit 0, data loss). Reject the combination HERE." The comment explicitly names the data-
loss risk and documents that the guard block is the correct rejection site — yet `--field`
is absent from the block's forbidden-sibling list. A user issuing both flags receives exit
0 with the label change applied and the `--field` value silently discarded.

**Class:** Data-loss (silent drop). BC-3.4.017 EC coverage gap.

**Resolution path:** Add `--field` to the conflict block's sibling list; add EC-3.4.017-13
to `bc-3-issue-write.md`; add integration test pinning exit 64 for the combination.

**Routing:** FIX-F5-001. Branch: `fix/F5-001-label-field-silent-drop`.

---

## LOW Observations

1. **L-1 — `--field` empty-value error UX:** When `--field NAME=` (empty value after `=`)
   is passed, the error message does not distinguish between "empty value" and "malformed
   format" (`NAME=`). Minor UX gap; does not affect correctness.

2. **L-2 — Missing negative regression tests for `--label` conflict-block siblings:** The
   conflict block lists 12 flags as forbidden co-passengers with `--label`; at time of
   review only 2 of those 12 pairs have explicit rejection tests. The 10 untested entries
   remain correctness-tested only by the guard's presence in code, not by a pinned test
   that would survive a future refactor.

3. **L-3 — Meta-test gap:** No structural test enforces that every flag listed in the
   `--label` conflict block is also represented in the test suite. If a flag is added to
   the conflict block without a corresponding test, the regression coverage gap is
   undetectable by CI.

4. **L-4 — EC-3.4.017-13 spec amendment required:** BC-3.4.017 has no EC for the
   `--label` + `--field` conflict. Per VSDD convention, every user-facing constraint must
   have a corresponding EC in the spec before the implementation fix is merged.

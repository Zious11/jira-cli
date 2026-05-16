# S-345 Demo Evidence Report

**Story:** S-345 — Extract label-coalesce JSON builder into pure function + proptest
**Issue:** #345
**Worktree branch:** feature/S-345-label-coalesce-extract
**HEAD:** 283fde88be1042fc5d63c6753442237fbc7e5fff
**Recorded:** 2026-05-15

## Type
REFACTOR + PROPTEST — extracts pure helper from handle_edit_bulk_labels; production behavior unchanged.

## AC-1 — Pure function exists
**Evidence:** ac1-function-exists.txt, ac1-function-body.txt
**Verdict:** PASS — signature `fn build_labels_edited_fields(adds: &[String], removes: &[String]) -> serde_json::Value`; private; sync; no async, no I/O, no client refs.

## AC-2 — Call-site uses it
**Evidence:** ac2-call-site.txt
**Verdict:** PASS — handle_edit_bulk_labels calls build_labels_edited_fields(&adds, &removes) at line 922 and assigns to edited_fields.

## AC-3 — Existing integration tests pass byte-for-byte
**Evidence:** ac3-integration-pr2-passes.txt (40 passed), ac3-integration-bulk-passes.txt (9 passed)
**Verdict:** PASS — refactor is byte-for-byte equivalent.

## AC-4 — Inline proptest covers BC-3.4.006
**Evidence:** ac4-proptest-passes.txt (1 passed, ~256 cases per run; 701 other lib tests filtered out)
**Verdict:** PASS — proptest covers all 5 BC-3.4.006 invariants (top-level "labels" sole key, ADD iff adds non-empty, REMOVE iff removes non-empty, both-action array-form with ADD-at-0/REMOVE-at-1, single-action object-form).

## AC-5 — fmt + clippy + tests all green
**Evidence:** ac5a-fmt-check.txt (exit 0, clean), ac5b-clippy.txt (no warnings, clean finish)
**Verdict:** PASS

## Red Gate mutation evidence
**Mutation:** `"labelsAction": "ADD"` → `"labelsAction": "WRONG_ADD"` in build_labels_edited_fields (production path only, not doc comments)
**Evidence:** red-gate-mutation-fails.txt
**Verdict:** FAILS WITH BC-3.4.006 assertion — panic message: `BC-3.4.006: single-ADD MUST set labelsAction=ADD`. The proptest correctly discriminates the contract. Production code reverted; `git diff src/cli/issue/create.rs` is empty after revert; post-revert proptest green (1 passed).

## F5 Adversarial Convergence Summary
6 fresh-context adversary passes; trajectory 0/1/6 → 0/2/3 → 0/2/2 → 0/0/0 → 0/0/0 → 0/0/0.
3 consecutive CLEAN passes (Pass 4, 5, 6) — convergence achieved.

## Files
Application source changes:
- src/cli/issue/create.rs (+127, -32) — extract function + add proptest + 3 nit/concern fix passes

Supporting artifacts added in this PR (not application source):
- docs/demo-evidence/S-345/ (10 files: AC-1..AC-5 evidence captures + Red Gate + this report)
- .gitignore (+3 lines: add proptest-regressions/ exclusion)

## Trace
- BC-3.4.006 pinned (HIGH confidence after this PR)
- Story S-345 AC-1 through AC-5 all satisfied
- Mutation Red Gate substituted for green-on-first-run pattern (see `.factory/cycles/cycle-001/S-345/implementation/red-gate-log.md` — note: `.factory/` is gitignored and not committed; the committed evidence is `docs/demo-evidence/S-345/red-gate-mutation-fails.txt`)

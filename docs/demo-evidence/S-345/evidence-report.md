# S-345 Demo Evidence Report

**Story:** S-345 — Extract label-coalesce JSON builder into pure function + proptest
**Issue:** #345
**Worktree branch:** feature/S-345-label-coalesce-extract
**HEAD:** (updated after convergence-completing commit — see git log)
**Recorded:** 2026-05-16

Note: All AC evidence files captured at or after the convergence-completing commit
(Copilot final round — Class A tighten + Class B regen). The prior evidence cycles
(097f5cd through e97bf62) are superseded by this regeneration.

## Type
REFACTOR + PROPTEST — extracts pure helper from handle_edit_bulk_labels; production behavior unchanged.

## AC-1 — Pure function exists
**Evidence:** ac1-function-exists.txt, ac1-function-body.txt
**Verdict:** PASS — signature `fn build_labels_edited_fields(adds: &[String], removes: &[String]) -> serde_json::Value`; private; sync; no async, no I/O, no client refs.

## AC-2 — Call-site uses it
**Evidence:** ac2-call-site.txt
**Verdict:** PASS — handle_edit_bulk_labels calls build_labels_edited_fields(&adds, &removes) and assigns to edited_fields.

## AC-3 — Existing integration tests pass byte-for-byte
**Evidence:** ac3-integration-pr2-passes.txt (40 passed), ac3-integration-bulk-passes.txt (9 passed)
**Verdict:** PASS — refactor is byte-for-byte equivalent.

## AC-4 — Inline proptest covers BC-3.4.006
**Evidence:** ac4-proptest-passes.txt (1 passed, ~256 cases per run; 701 other lib tests filtered out)
**Verdict:** PASS — proptest covers all 5 BC-3.4.006 invariants (top-level "labels" sole key, ADD iff adds non-empty, REMOVE iff removes non-empty, both-action array-form with ADD-at-0/REMOVE-at-1, single-action object-form).

Copilot final round (Class A): proptest helper replaced — strict shape-pinning via
extract_action_and_names; iter().map() not filter_map(); each action entry asserted to
have EXACTLY 2 keys; each label entry asserted to have EXACTLY 1 key (name).

## AC-5 — fmt + clippy + tests all green
**Evidence:** ac5a-fmt-check.txt (exit 0, no output), ac5b-clippy.txt (no warnings, clean finish)
**Verdict:** PASS

## Red Gate mutation evidence
**Mutation:** `"labelsAction": "ADD"` → `"labelsAction": "WRONG_ADD"` in build_labels_edited_fields (production path only, not doc comments)
**Evidence:** red-gate-mutation-fails.txt
**Verdict:** FAILS WITH BC-3.4.006 assertion — test failed (proptest replays minimal input `adds = ["a"], removes = []`). The tightened proptest correctly discriminates the contract at the labelsAction string level. Production code reverted; `git diff src/cli/issue/create.rs` shows only Fix 1 changes (no WRONG_ADD); post-revert proptest green (1 passed).

## F5 Adversarial Convergence Summary
6 fresh-context adversary passes; trajectory 0/1/6 → 0/2/3 → 0/2/2 → 0/0/0 → 0/0/0 → 0/0/0.
3 consecutive CLEAN passes (Pass 4, 5, 6) — convergence achieved.

## Files
Application source changes:
- src/cli/issue/create.rs (+186, -41) — extract function + add proptest + fix/tighten passes

Supporting artifacts added in this PR (not application source):
- docs/demo-evidence/S-345/ac1-function-exists.txt
- docs/demo-evidence/S-345/ac1-function-body.txt
- docs/demo-evidence/S-345/ac2-call-site.txt
- docs/demo-evidence/S-345/ac3-integration-pr2-passes.txt
- docs/demo-evidence/S-345/ac3-integration-bulk-passes.txt
- docs/demo-evidence/S-345/ac4-proptest-passes.txt
- docs/demo-evidence/S-345/ac5a-fmt-check.txt
- docs/demo-evidence/S-345/ac5b-clippy.txt
- docs/demo-evidence/S-345/red-gate-mutation-fails.txt
- docs/demo-evidence/S-345/evidence-report.md
- .gitignore (+3 lines: add proptest-regressions/ exclusion)

## Trace
- BC-3.4.006 pinned (HIGH confidence after this PR)
- Story S-345 AC-1 through AC-5 all satisfied
- Mutation Red Gate substituted for green-on-first-run pattern (see `.factory/cycles/cycle-001/S-345/implementation/red-gate-log.md` — note: `.factory/` is gitignored and not committed; the committed evidence is `docs/demo-evidence/S-345/red-gate-mutation-fails.txt`)

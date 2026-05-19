---
document_type: lessons
story: S-382
issue: "#382"
pr: "#389"
merge_sha: b1c863e
merged_at: 2026-05-19
producer: state-manager
---

# Lessons Learned — Issue #382 (S-382 Quick-Dev)

Refactor JrError::InsufficientScope Display to use structured required_scope field.

---

## L-382-01 — Step-2 Stub Overreach (Variant-Widening Refactors)

**What happened:** When refactoring an existing enum variant, the stub agent inadvertently applied the implementation change (template parameterization of the Display format string) along with the structural change (adding the `required_scope` field). The stub commit therefore contained both "additive field" (which belongs in Step 2) and "behavior change" (which belongs in Step 4 implementation).

**Impact:** Red Gate was broken — the failing AC-3 test could not be written because the implementation was already present in the stub. The orchestrator caught this and reverted the implementation portion (commit `950aefb` revert → `bab5b4b` failing AC-3 test → `1fd3b23` implement Display change → AC-3 passes).

**Codify:** For variant-widening refactors, the stub-architect agent prompt must explicitly distinguish:
- **Additive field stub** — add the new field to the variant; leave Display/Serialize/etc. implementations unchanged. The old behavior must still compile and pass.
- **Behavior change** — updating Display, Serialize, or other trait impls to use the new field belongs in Step 4 (implementation), not Step 2 (stub).

The distinction is: "does this change break any existing test?" If yes, it belongs in Step 4 (Red Gate trigger), not Step 2 (structural scaffold).

---

## L-382-02 — Red Gate Sequencing for Pure-Refactor Stories

**What happened:** For pure-refactor stories (no new behavior, only renaming or signature widening with backward-compat fallback), Red Gate enforcement requires careful sequencing. The new tests must fail at the intermediate stub stage where the field exists but the Display impl has not been updated.

**Correct sequence demonstrated:**
1. `950aefb` — Revert template parameterization from stub (restore Red state)
2. `bab5b4b` — Write failing AC-3 test (asserts new Display format; fails because old format still active)
3. `1fd3b23` — Implement template parameterization — AC-3 now passes; all prior tests still green

**Codify:** In variant-widening refactors, the orchestrator must verify after Step 2 that the AC test targeting the Display/format change actually fails (cargo test exits non-zero on the new test). If all tests pass after Step 2, the stub overreached into Step 4 territory — revert the excess and re-test.

---

## L-382-03 — F1d Convergence Cost vs. Scope Classification

**What happened:** F1d convergence took 8 passes for a trivial-scope story (target is a 3-pass minimum for STANDARD/TRIVIAL). Each pass surfaced 2-8 findings. Most were S-7.01 partial-fix-propagation regressions — cross-artifact label drift and doc-surface count mismatches that were fixed in one document but not propagated to sibling documents (BC body, BC index, STATE.md, story spec, etc.).

**Pattern:** TRIVIAL classification was applied based on implementation scope (single enum field addition, ~50 LOC). But the spec corpus surface area was MEDIUM — BC-1.6.042 had references across bc-1 body, BC-INDEX, holdout-scenarios, STATE.md, STORY-INDEX, story spec — each requiring synchronized updates.

**Codify:** STANDARD/TRIVIAL classification should consider F1d _expected pass count_ as a factor alongside implementation LOC. If a story touches a BC that has references in 4+ files, the expected F1d passes is likely 4-6, not 1-3. When actual passes exceed 2x the expected count (8 vs. expected 3), retroactively reclassify the scope as MEDIUM and note the classification error in lessons.

---

## L-382-04 — Pre-Existing Test Flakes: Document, Don't Investigate

**What happened:** CI showed a keychain contention failure in `tests/multi_cloudid_disambiguation.rs` during S-382 delivery. Investigation confirmed the flake reproduces on `develop` without S-382 changes — it is a pre-existing macOS keychain "specified item already exists" error from concurrent test execution.

**Impact on S-382:** None. The PR was unblocked after confirming the flake was pre-existing.

**Codify:** Pre-existing test flakes (not caused by the current story's changes) should be:
1. Documented in the PR body with explicit "pre-existing, not a regression" note.
2. Tracked as a Drift Item in STATE.md (S-382-FLAKE-01).
3. NOT investigated as part of the current story unless they block CI for >2 retries.

The correct fix target for keychain flakes is: gate keychain tests behind `JR_RUN_KEYRING_TESTS=1 + #[ignore]` per existing CLAUDE.md convention, or add per-test keychain namespacing. This is future test-infrastructure work, not part of any feature story.

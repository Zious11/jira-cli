---
document_type: business-analyst-input
phase: F1
issue: 345
producer: business-analyst
inputs:
  - ".factory/specs/prd/BC-INDEX.md"
  - ".factory/specs/prd/bc-3-issue-write.md"
  - ".factory/specs/prd/cross-cutting.md"
  - ".factory/stories/STORY-INDEX.md"
  - ".factory/STATE.md"
  - "src/cli/issue/create.rs"
  - "tests/issue_bulk.rs"
  - "tests/issue_bulk_pr2.rs"
input-hash: "[pending]"
status: draft
timestamp: 2026-05-15
---

# F1 Business-Analyst Input — Issue #345

## BC Mapping

The label JSON-builder logic in `handle_edit_bulk_labels` is partially contracted
by BC-3.4.006 and the coalesce invariant tested in `test_label_add_remove_coalesce_emits_one_bulk_call`.
Neither BC explicitly contracts the two output shapes (object-form vs array-form) as
named invariants, nor pins the internal pure-function boundary. The proptest coverage
requested by #345 does not yet exist.

| BC ID | Title | Status | Notes |
|-------|-------|--------|-------|
| BC-3.4.006 | `issue edit --label add:foo --label remove:bar` interprets prefix and merges with existing | UNCHANGED | Contracts label-prefix semantics at the CLI layer; does NOT pin the JSON shapes or the single-call invariant as a formal BC body. The integration test in `test_label_add_remove_coalesce_emits_one_bulk_call` covers the coalesce side-effect (.expect(1)) but the BC body itself is thin (MEDIUM confidence, no explicit JSON shape pin). |
| BC-3.4.001 through BC-3.4.009 (excl. 006) | Edit, Open, Bulk deadline | UNCHANGED | No other 3.4 BC references label coalesce, `editedFieldsInput.labels`, or the JSON builder shape. |
| (no existing BC) | Object-form JSON shape when only ADD or only REMOVE | GAP | The `len() == 1` branch that unwraps to `{"labels": {labelsAction, labels}}` is not pinned in any BC body. |
| (no existing BC) | Array-form JSON shape when ADD + REMOVE coalesced | GAP | The `len() > 1` branch that produces `{"labels": [{labelsAction: "ADD", ...}, {labelsAction: "REMOVE", ...}]}` is not pinned in any BC body. |
| (no existing BC) | `build_labels_edited_fields` pure-function single-call invariant | GAP | The proptest requirement ("single-call invariant") has no BC anchor. This invariant — that the pure function produces deterministic output for any (adds, removes) pair — is currently implicit. |

**Key finding:** BC-3.4.006 exists and covers the prefix-parsing semantic (`add:` / `remove:`
/ bare = add). It does NOT contract the JSON wire shapes or the pure-function boundary.
The issue's proptest coverage request requires at minimum one new BC to anchor the
shape-correctness and single-call invariants that proptest will verify.

**Justification for gap assessment:** The `handle_edit_bulk_labels` source
(`src/cli/issue/create.rs:873-898`) builds both JSON shapes inline. The integration
tests use `body_string_contains` loose matchers (documented in the SCHEMA NOTES
at the top of `tests/issue_bulk.rs` and `tests/issue_bulk_pr2.rs`). No BC body
currently states "given adds=[X] removes=[], the wire body MUST be `{labels: {labelsAction: 'ADD', labels: [{name: X}]}}`"
or pins the symmetric array-form. The proptest requested by #345 would be
the first thing to assert these shapes deterministically — it needs a BC anchor
to be meaningful.

## NFR / VP Mapping

| NFR-ID / VP-ID | Title | Coverage Note |
|----------------|-------|---------------|
| BC-X.9.001 | `escape_value` proptest pattern | UNCHANGED — reference pattern for how inline proptests are structured in this codebase; not directly affected |
| BC-X.5.010 | Duration proptest pattern | UNCHANGED — reference pattern; `src/duration.rs` shows inline `mod proptests` with `proptest!` macro |
| BC-X.10.002 | `partial_match` proptest pattern | UNCHANGED — reference pattern; `src/partial_match.rs` has inline proptest |
| (no existing VP for label JSON builder) | Label JSON shape determinism | NO VP exists. The proptest for `build_labels_edited_fields` would be the first formal property check on this function. No VP directory exists in `.factory/specs/`; the pattern is inline `#[cfg(test)] mod proptests` as in `src/jql.rs` and `src/duration.rs`. |

No NFR in `nfr-catalog.md` covers label JSON builder determinism. The change
is too narrow to warrant a new NFR. The proptest pins property-level correctness
as a BC-level concern, not an NFR-level one.

## Story Risk Zone

| Story / PR | Why in risk zone |
|-----------|------------------|
| PR #348 (Feature Mode #110-pr2, closes #110) | Delivered `handle_edit_bulk_labels` and both JSON shapes. Any refactor of lines 873-898 is in the blast radius of every test that exercises the bulk label path. |
| PR #325 (#110 pr1) | Delivered the single-action `{"labels": {labelsAction: ...}}` object form. BC-3.4.006 was first documented here. |
| `tests/issue_bulk.rs` — `test_edit_label_remove_sends_remove_action_in_bulk_payload` | Directly pins the REMOVE path through `handle_edit_bulk_labels`. Must remain green post-refactor. |
| `tests/issue_bulk_pr2.rs` — `test_label_add_remove_coalesce_emits_one_bulk_call` | Pins the `.expect(1)` coalesce invariant (one bulk POST for ADD+REMOVE combined). Must remain green. |
| `tests/issue_bulk_pr2.rs` — `test_label_with_summary_rejected_before_search`, `test_label_with_priority_rejected_before_search`, and related rejection tests | Indirectly validate that the routing to `handle_edit_bulk_labels` only fires when label flags are the sole operation. No JSON-builder change should touch this path, but the shared dispatch means any signature change to `handle_edit_bulk_labels` appears here. |
| `tests/issue_bulk.rs` — `test_edit_multi_key_issues_one_bulk_post_then_polls_to_complete` | Happy-path bulk edit. Exercises polling but not label shapes specifically. Regression baseline. |

No Wave 0–3 story modifies the label JSON builder post-PR #348 closure.
Risk zone is bounded to the eight tests above plus any inline tests in
`src/cli/issue/create.rs` (current inline `mod tests` at line 1085 has two
tests: `missing_project_returns_user_error` and the field-categorization meta-test;
neither touches the label builder).

## Tests in Neighborhood

| File | Test | Relation to #345 |
|------|------|-----------------|
| `tests/issue_bulk_pr2.rs` | `test_label_add_remove_coalesce_emits_one_bulk_call` | Pins coalesce → 1 bulk POST via `.expect(1)`. Uses `body_string_contains("selectedActions")` loose matcher. Does NOT assert the exact JSON shape of `editedFieldsInput.labels`. Primary regression guard. |
| `tests/issue_bulk.rs` | `test_edit_label_remove_sends_remove_action_in_bulk_payload` | Pins the REMOVE-only path; asserts body contains `"REMOVE"`. Loose matcher only. |
| `tests/issue_bulk.rs` | `test_edit_multi_key_issues_one_bulk_post_then_polls_to_complete` | ADD-only path happy-path. |
| `tests/issue_bulk_pr2.rs` | Multiple `test_label_with_*_rejected_before_search` tests | Route-rejection tests; unaffected by the JSON-builder refactor itself. |
| `src/cli/issue/create.rs` | `missing_project_returns_user_error` (inline) | Unrelated to label builder. |
| `src/cli/issue/create.rs` | field-categorization meta-test (inline) | Verifies `IssueCommand::Edit` field enum coverage; unrelated to label builder JSON shapes. |
| (none yet) | proptest for `build_labels_edited_fields` | To be added by this issue. Will be inline `mod proptests` in `src/cli/issue/create.rs`, matching the `src/jql.rs` and `src/duration.rs` pattern. |

**No existing proptest covers the label JSON builder.** The proptest infrastructure
(`proptest = "1"` in Cargo.toml) is already present.

## Feature Type

`backend` — the change is entirely within `src/cli/issue/create.rs`. No UI,
no API schema change, no new external dependency, no configuration format change.
The extracted function is a pure transformation with no I/O.

## Intent Classification

`refactor`

**Reasoning:** The issue is labeled `refactor` + `test` + `audit-followup` by the
author. The behavior of `handle_edit_bulk_labels` is unchanged — the JSON shapes
must be preserved byte-for-byte (as stated in the issue). The deliverable is:
(1) a named pure function wrapping the inline JSON-builder logic, and (2) a
proptest covering the function's deterministic output contract. No new user-visible
behavior is added. No bug is fixed. The existing integration tests continue to
pass unchanged.

**Not `enhancement`:** No new user-facing capability is added. The only consumer
of the label JSON builder remains `handle_edit_bulk_labels`, which calls the bulk
API exactly as before.

**Not `bug-fix`:** The inline builder produces correct output today; the issue
does not claim otherwise. This is code-quality work: testability via extraction,
confidence via proptest.

**Not `feature`:** No new command, flag, or API interaction is introduced.

## Trivial-Scope Verdict

TRIVIAL

Criterion-by-criterion:

- **Single module / single file:** YES — the extraction targets `src/cli/issue/create.rs`
  exclusively. The proptest lives inline in the same file (following the `jql.rs` /
  `duration.rs` / `partial_match.rs` pattern). No new file is created; no other module
  is modified.
- **No new BCs:** CONDITIONAL — see recommendation below. The change is a refactor
  that preserves behavior; the question is whether the new proptest's invariants
  require a new BC body as a spec anchor. Analysis: BC-3.4.006 already exists and
  names the add/remove prefix semantic. Extending BC-3.4.006 with a note about the
  two JSON shapes and the pure-function proptest coverage is sufficient — no new BC
  ID is strictly required. However, the JSON shapes are currently uncontracted at the
  BC level; an extension to BC-3.4.006 or a new BC-3.4.010 would close that gap
  cleanly. Decision: TRIVIAL if BC-3.4.006 is extended in-place (no new BC ID);
  STANDARD if a new BC-3.4.010 is minted. Recommended path: BC-3.4.006 extension
  (in-place update, no ID increment).
- **No architecture change:** YES — extracting a private function within one file
  is not an architecture change. No module boundary is crossed. No new abstraction
  layer is introduced.
- **No new external deps:** YES — `proptest = "1"` is already in `[dev-dependencies]`.
  No new crate is added.
- **LOW regression risk:** YES — the refactor preserves byte-for-byte output.
  The six existing integration tests covering the label path (`.expect(1)` coalesce,
  REMOVE-path body check, happy-path polling) are the regression guard. If the
  extraction is done correctly, all pass unchanged. The proptest provides forward
  protection against accidental shape drift.

All five criteria pass (with the BC-3.4.006 in-place extension interpretation).
Verdict: **TRIVIAL**.

## Recommendation

**REFACTOR-ONLY-NO-NEW-BC** — with BC-3.4.006 extended in-place to note the two
JSON shapes and the proptest coverage.

Rationale:

1. The refactor is fully contained in `src/cli/issue/create.rs`. No other file changes.
2. BC-3.4.006 already exists with MEDIUM confidence and a thin body. F2 spec-evolution
   should extend its Behavior section to name the two output shapes (object-form for
   single-action, array-form for ADD+REMOVE) and cite the new proptest as the
   high-confidence source. This is an in-place amendment, not a new BC ID.
3. If the F2 owner prefers a clean separation (not mutating an existing BC body),
   minting BC-3.4.010 for the pure-function JSON shape invariant is also acceptable.
   That would shift the verdict to REFACTOR-WITH-NEW-BC-FOR-INVARIANT and scope to
   STANDARD. The business analyst defers to the F2 spec-evolution pass for that call.
4. No new story file is needed. This fits the feature-followup pattern established
   by S-340: a `code-delivery/issue-345/story.md` is sufficient.
5. Existing integration tests are the regression guard. The proptest is the new
   property-level guard. No additional integration test is required beyond what
   issue #345 calls for.

Total estimated scope: 1 inline proptest module (~30-50 lines in `src/cli/issue/create.rs`),
1 private function extraction (~30 lines reorganized, no net new logic),
1 BC-3.4.006 in-place extension (5-10 lines in `bc-3-issue-write.md` + BC-INDEX.md
confidence bump from MEDIUM to HIGH). Copilot review surface is minimal.

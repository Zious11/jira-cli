---
document_type: f1-delta-analysis
phase: phase-f1-delta-analysis
producer: state-manager
issue: 345
status: orchestrator-approved
timestamp: 2026-05-16
project: jira-cli
mode: BROWNFIELD
intent: refactor
feature_type: backend
trivial_scope: true
regression_risk: low
inputs:
  - ".factory/phase-f1-delta-analysis/architect-input-345.md"
  - ".factory/phase-f1-delta-analysis/business-analyst-input-345.md"
---

# F1 Delta Analysis — Issue #345

## Issue Summary

Issue #345 ("refactor(bulk): extract label-coalesce JSON builder into pure function with proptest coverage") is an audit-followup from the F6 hardening review of PR #110-pr2 (2026-05-10). The inline 26-line JSON-builder block inside `handle_edit_bulk_labels` (lines 873-898 of `src/cli/issue/create.rs`) is to be extracted into a named, private, synchronous pure function `build_labels_edited_fields(adds: &[String], removes: &[String]) -> serde_json::Value`, with an inline `mod proptests` block added in the same file. No behavioral change is introduced; the existing integration tests in `tests/issue_bulk_pr2.rs` serve as the regression baseline.

## Approved Scope

REFACTOR-ONLY. Extract `build_labels_edited_fields(adds: &[String], removes: &[String]) -> serde_json::Value` from `src/cli/issue/create.rs::handle_edit_bulk_labels` (lines 873-898). Add inline `#[cfg(test)] mod proptests` covering:
1. Top-level `"labels"` key is always present in the returned Value.
2. ADD entries appear in output if and only if `adds` is non-empty.
3. REMOVE entries appear in output if and only if `removes` is non-empty.
4. Single-operand path returns object-form; dual-operand path returns array-form.

BC strategy: EXTEND existing BC-3.4.006 in-place (currently MEDIUM confidence) to pin the two JSON wire shapes (object-form for single action, array-form for ADD+REMOVE coalesced) and cite the new proptest as coverage source. NO new BC ID is created.

## Impact Assessment Table

| Artifact | Change |
|----------|--------|
| PRD | BC-3.4.006 extended in-place (MEDIUM → HIGH confidence; two JSON wire shapes pinned; proptest cited). No new BC ID. |
| Architecture | UNCHANGED — private function extraction within one file; no module boundary crossed; no interface change |
| UX | n/a — backend-only refactor; no user-visible behavior change |
| Stories | +1 (S-345; feature-followup; code-delivery/issue-345/story.md) |
| Tests | 1 inline `mod proptests` block added in `src/cli/issue/create.rs`; ~20-30 lines; existing integration tests UNCHANGED |
| VPs | n/a — no VP directory exists; property correctness is anchored at BC level via proptest |

## Files Likely Changed

- `src/cli/issue/create.rs` — MODIFIED: new private function `build_labels_edited_fields` (~15 lines), call-site replacement in `handle_edit_bulk_labels` (26 inline lines → 1 call), new `mod proptests` block (~20-30 lines appended at end of file following `src/jql.rs` / `src/duration.rs` pattern). Estimated net growth: ~1,456 → ~1,490 lines.
- `.factory/specs/prd/bc-3-issue-write.md` — MODIFIED (F2, not F4): BC-3.4.006 body extended to pin object-form and array-form JSON wire shapes; confidence bumped MEDIUM → HIGH; proptest cited. (DO NOT TOUCH in F4 — product-owner owns F2.)
- `.factory/code-delivery/issue-345/story.md` — NEW: S-345 story file (feature-followup pattern from S-340)

## Files NOT Changed (regression baseline)

- `tests/issue_bulk_pr2.rs` — UNCHANGED; `test_label_add_remove_coalesce_emits_one_bulk_call` (`.expect(1)` coalesce guard), dry-run variants, and rejection tests remain untouched; they are the regression baseline for byte-for-byte JSON shape preservation
- `tests/issue_bulk.rs` — UNCHANGED; `test_edit_label_remove_sends_remove_action_in_bulk_payload` and `test_edit_multi_key_issues_one_bulk_post_then_polls_to_complete` remain untouched
- `src/cli/issue/workflow.rs` — UNCHANGED; `handle_move_bulk` does not use label coalescing
- `src/api/jira/bulk.rs` — UNCHANGED; `bulk_edit_fields` and `await_bulk_task` signatures are unmodified; the extracted function merely constructs the `edited_fields` value passed to `bulk_edit_fields`
- `src/cli/issue/create.rs::handle_edit_bulk_fields` — UNCHANGED; handles `--summary`, `--priority`, `--type`; separate from label builder
- `CLAUDE.md` — UNCHANGED; no new `JR_*` env var; no documented behavior change
- `.factory/specs/prd/BC-INDEX.md` — UNCHANGED (in-place BC extension, no new BC ID; BC-INDEX entry for BC-3.4.006 may receive a confidence-bump note at F2 discretion)
- `.factory/stories/STORY-INDEX.md` — MODIFIED (F3): S-345 registered at next available ID

## Risk Assessment

- **Regression risk: LOW** — The refactor preserves byte-for-byte `serde_json::Value` output. The only regression vector is if the extracted function diverges from the inline block logic. The PR2 `body_string_contains` / `body_partial_json` matchers and `.expect(1)` wiremock guards catch any divergence at the HTTP body level. Extraction by literal copy with no logic change makes tests go green immediately.
- **Architecture risk: NONE** — Pure internal refactor; no module boundary crossed; no trait or interface change; no change to the async boundary or the caller chain from `handle_edit` down
- **Security risk: NONE** — `build_labels_edited_fields` performs pure JSON construction from caller-supplied `Vec<String>` values already parsed and validated upstream in the label-prefix loop; no I/O, no credential handling, no external input deserialisation

## Recommended Scope for Subsequent Phases

- **F2:** Product-owner extends BC-3.4.006 in-place in `.factory/specs/prd/bc-3-issue-write.md` to pin the two JSON wire shapes (object-form for single action, array-form for ADD+REMOVE coalesced) and cite the new proptest; bumps confidence MEDIUM → HIGH. No new BC ID created. No BC-INDEX or CANONICAL-COUNTS change required.
- **F3:** Story-writer creates S-345 as a feature-followup story (`code-delivery/issue-345/story.md`); bc_anchors=[BC-3.4.006]; registers in STORY-INDEX.md.
- **F4:** Test-writer writes inline proptest first (Red Gate: proptest should fail before extraction is committed because `build_labels_edited_fields` does not yet exist as a named symbol). Implementer then extracts the function and replaces the call site. Red Gate passes when all tests green. Sequence: proptest added → `cargo test --lib` fails (undefined symbol) → function extracted → `cargo test --lib` passes → `cargo test` (integration suite) passes → `cargo clippy -- -D warnings` passes.
- **F5:** Scoped adversarial on the diff (3 clean passes); surface is one-file diff (~35 lines net change). No adversary needs access to integration tests as they are unchanged.
- **F6:** Minimal — the proptest IS the hardening artifact for this refactor. Security scan optional (no new I/O surface, no new attack surface per architect input). Copilot review expected to be 1-2 rounds.
- **F7:** PR via pr-manager; target develop; label `refactor`, `audit-followup`; closes #345.

## Deferred

None. This is a clean trivial refactor. All options from #331 (schema-correctness work) are a different issue and remain sandbox-blocked; they are explicitly out of scope for #345.

## Quality Gate

Orchestrator approved 2026-05-16.

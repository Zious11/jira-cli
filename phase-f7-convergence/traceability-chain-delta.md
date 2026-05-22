---
document_type: f7-traceability-chain-delta
feature: issue-388 / S-388
spec_version: v1.3.0
pr: "#397"
pr_sha: e0ea24b
date: 2026-05-21
producer: state-manager
---

# Traceability Chain — S-388 Delta

This document records the end-to-end traceability for the S-388 delta, linking
behavioral contracts through implementation artifacts to test coverage and adversarial
verification.

---

## BC → Implementation → Test → Verification

### BC-3.4.010 — Cross-hierarchy type-change 400 enrichment

| Link | Artifact |
|------|----------|
| **Behavioral Contract** | `BC-3.4.010` in `.factory/specs/prd/bc-3-issue-write.md` |
| **Spec anchor** | When `edit --type` returns HTTP 400 and `is_cross_hierarchy_type_error` evaluates CONFIRMED, the CLI MUST emit `CROSS_HIERARCHY_HINT` and exit 1. |
| **Implementation** | `src/cli/issue/create.rs` — `is_cross_hierarchy_type_error(issue_type: &IssueType, parent: &Option<Parent>) -> Classification` pure classifier; `Classification` enum (CONFIRMED / INDETERMINATE / NOT_CROSS_HIERARCHY); `CROSS_HIERARCHY_HINT` const; `handle_edit` error-path dispatch block routing 400 responses through the classifier. |
| **Supporting type** | `src/types/jira/issue.rs` — `IssueType.subtask: Option<bool>` field (`#[serde(default)]`), additive with no breaking change. |
| **Integration tests** | `tests/issue_edit_type_errors.rs` — tests #1 (story→subtask CONFIRMED hint emitted), #2 (subtask→story CONFIRMED hint emitted), #5 (indeterminate path: no hint emitted for unrecognized 400). |
| **Regression guard** | `tests/issue_edit_type_errors.rs::test_no_parent_non_subtask_400_does_not_surface_cross_hierarchy_hint` (test #10, added after CI mutation-gap detection at 85% kill rate). |
| **T-06 strengthening** | Existing `T-06` in `tests/issue_edit_no_parent.rs` — assertions tightened to ensure cross-hierarchy hint is NOT emitted on `--no-parent` path (BC-3.4.010 / BC-3.4.011 boundary guard). |
| **Inline proptest** | `src/cli/issue/create.rs` — exhaustive 9-state `Option<bool> × Option<bool>` proptest for `is_cross_hierarchy_type_error`; 256 runs; `err: &str` independence property verified. |
| **Adversary verification** | Per-story adversarial review: 4 passes (pass 1: 1 MAJOR found — `--no-parent` arm fabricated English error, fixed `fd0cdd5`; passes 2/3/4 CLEAN). F5 scoped adversarial: 2 clean passes. |
| **Merged SHA** | `e0ea24b` on `develop` (PR #397, 2026-05-21) |

---

### BC-3.4.011 — `--no-parent` non-subtask accurate error path

| Link | Artifact |
|------|----------|
| **Behavioral Contract** | `BC-3.4.011` in `.factory/specs/prd/bc-3-issue-write.md` |
| **Spec anchor** | When `--no-parent` is supplied for a non-subtask issue and Jira returns HTTP 400, the CLI MUST surface the accurate "cannot remove parent" message and NOT emit a fabricated English error or cross-hierarchy hint. |
| **Implementation** | `src/cli/issue/create.rs` — classifier typo/indeterminate paths in `is_cross_hierarchy_type_error`; `handle_edit` dispatch correctly routes non-subtask `--no-parent` 400 responses away from `CROSS_HIERARCHY_HINT`. MAJOR defect fixed at `fd0cdd5`: `--no-parent` arm previously fabricated an English error string instead of surfacing the real Jira error. |
| **Integration tests** | `tests/issue_edit_type_errors.rs` — tests #3 (no-parent non-subtask: no hint), #4 (no-parent with subtask flag None: no hint), #6 (typo path: INDETERMINATE, no hint), #7 (indeterminate: parent Some but subtask None), #8 (parent None: NOT_CROSS_HIERARCHY routing), #9 (boundary: parent + subtask=Some(false)). `tests/issue_edit_no_parent.rs` — T-06 strengthened (`&&`→`||` kill-test added). |
| **Inline proptest** | `src/cli/issue/create.rs` — same 9-state proptest as BC-3.4.010; confirms `err` arg has no influence on classification verdict. |
| **Adversary verification** | Per-story adversarial review: passes 2/3/4 CLEAN (after `fd0cdd5` fix). F5 scoped adversarial: 2 clean passes. |
| **Merged SHA** | `e0ea24b` on `develop` (PR #397, 2026-05-21) |

---

### BC-3.4.003 — Annotation-only cross-reference

| Link | Artifact |
|------|----------|
| **Behavioral Contract** | `BC-3.4.003` in `.factory/specs/prd/bc-3-issue-write.md` |
| **Change** | Annotation-only: added cross-reference to BC-3.4.010 and BC-3.4.011 in the Errors section. No behavioral change to BC-3.4.003 itself. |
| **Test coverage** | Existing tests for BC-3.4.003 unaffected; no new tests required. |
| **Adversary verification** | F2 adversarial review (10 passes, 3 consecutive clean): cross-ref annotation confirmed correctly placed and not contradicting BC-3.4.003 contract. |

---

### S-388 Story → BC Anchors

| Story | BCs Implemented | ACs |
|-------|----------------|-----|
| S-388 | BC-3.4.010, BC-3.4.011 | 7 ACs (all verified by integration tests + VHS demo evidence) |
| S-388 | BC-3.4.003 | annotation-only cross-ref (no new AC required) |

---

## Verification Chain

| Verification Type | Result | Evidence |
|------------------|--------|----------|
| Kani formal proof | JUSTIFIED SKIP | No Kani in project; 9-state finite domain exhaustively proptested. `.factory/phase-f6-hardening/kani-results.md` |
| Fuzz testing | JUSTIFIED SKIP | No new untrusted-input parser. `.factory/phase-f6-hardening/fuzz-results.md` |
| Mutation testing | 100% kill rate (7/7 viable) | `.factory/phase-f6-hardening/mutation-results.md` |
| cargo-deny | PASS | 340 crates; advisories ok, bans ok, licenses ok, sources ok |
| cargo-audit | PASS | 1096 advisories; clean |
| Full regression suite | 1398/0/18 | `.factory/phase-f6-hardening/summary.md` |
| Per-story adversarial | 3/3 CLEAN (passes 2/3/4) | Adversary convergence evidence (burst-log) |
| F5 scoped adversarial | 2 clean passes | F5 post-merge review |

---

## Traceability Append Note

There is no pre-existing `convergence/traceability-matrix.md` or sibling file under
`.factory/cycles/cycle-001/`. This document is the authoritative traceability artifact
for the S-388 delta. For the main BC traceability matrix covering the full v0.5 spec
corpus, see `.factory/specs/` and `BC-INDEX.md` in the behavioral-contracts directory.

If a project-level traceability matrix is created in a future cycle, the S-388 entries
from this document should be appended to that file with the following merge key:
`bc_ids: [BC-3.4.010, BC-3.4.011, BC-3.4.003]`, `story: S-388`, `pr: #397`, `sha: e0ea24b`.

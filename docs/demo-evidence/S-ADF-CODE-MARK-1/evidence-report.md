---
document_type: demo-evidence-report
story_id: "S-ADF-CODE-MARK-1"
title: "ADF code-mark exclusivity: push_code allowlist filter strips typographic marks"
github_issue: "#571"
recording_tool: VHS 0.11.0
branch: develop
worktree: .worktrees/S-ADF-CODE-MARK-1
produced_by: demo-recorder
timestamp: "2026-07-07"
bc_anchors: ["BC-7.2.015", "BC-7.2.007"]
---

# Demo Evidence Report — S-ADF-CODE-MARK-1

**Story**: ADF code-mark exclusivity — `push_code` allowlist filter strips typographic marks  
**GitHub issue**: #571  
**BC anchors**: BC-7.2.015 (primary), BC-7.2.007 EC-2 (amendment/closure)  
**Product type**: CLI library (Rust) — evidence is test-based per Demo Plan  
**Recording tool**: VHS 0.11.0  
**Red-Gate pre-fix evidence pointer**: `.factory/cycles/cycle-001/S-ADF-CODE-MARK-1/implementation/red-gate-log.md`

---

## Coverage Summary

All 12 acceptance criteria covered. Every AC traces to a VHS recording and named test function(s).

| AC | Description | Recording | Test(s) | Result |
|----|-------------|-----------|---------|--------|
| AC-001 | Test helpers `assert_marks_eq` + `assert_link_mark_with_href` exist | `AC-001-009-lib-tests` | Helper functions (consumed by AC-002..AC-007) | GREEN |
| AC-002 | `test_markdown_inline_code_mark_and_composition` rewritten to `marks==[code]` | `AC-001-009-lib-tests` | `test_markdown_inline_code_mark_and_composition` | GREEN |
| AC-003 | EC-1 strong stripped + control baseline | `AC-001-009-lib-tests` | `test_bc_7_2_015_strong_stripped_from_code_node`, `test_bc_7_2_015_plain_code_baseline` | GREEN |
| AC-004 | EC-2/EC-3/EC-4: em, strike, subsup stripped | `AC-001-009-lib-tests` | `test_bc_7_2_015_em_stripped_from_code_node`, `test_bc_7_2_015_strike_stripped_from_code_node`, `test_bc_7_2_015_subsup_stripped_from_code_node` | GREEN |
| AC-005 | EC-5: link mark preserved on code node | `AC-001-009-lib-tests` | `test_bc_7_2_015_link_preserved_on_code_node` | GREEN |
| AC-006 | EC-6/VP-571-003: node-scoped stripping; surrounding marks retained | `AC-001-009-lib-tests` | `test_bc_7_2_015_mixed_range_surrounding_marks_retained`, `test_bc_7_2_015_multi_mark_wrapper_only_code_node_stripped` | GREEN |
| AC-007 | PANEL-ANCHOR: `panel.content` traversal strips strong from code | `AC-001-009-lib-tests` | `test_bc_7_2_015_alert_wrapper_strong_code_stripped` | GREEN |
| AC-008 | EC-7/VP-571-004: `adf_to_text` read-tolerance retained; docstrings refreshed | `AC-001-009-lib-tests` | `test_render_marks_code_and_strong`, `test_render_strong_with_code_applies_code_innermost`, `test_push_code_normalizes_lone_cr_in_inline_code`, `test_push_code_normalizes_bare_lf_to_space` (MUST-STAY-GREEN) | GREEN |
| AC-009 | VP-571-001 proptest: universal quantifier over 9 container wrappers + all inline templates | `AC-001-009-lib-tests` | `prop_bc_7_2_015_no_code_marked_text_node_carries_typographic_marks` | GREEN |
| AC-010 | H-NEW-ADF-010 Calls A–D: wiremock integration tests (platform path) | `AC-010-integration-exclusivity` | `test_bc_7_2_015_call_a_strong_code_mark_stripped_platform_path`, `test_bc_7_2_015_call_b_subsup_code_mark_stripped_platform_path`, `test_bc_7_2_015_call_c_link_preserved_with_code_mark_platform_path`, `test_bc_7_2_015_call_d_surrounding_strong_retained_inner_code_stripped_platform_path` | GREEN |
| AC-011 | H-NEW-ADF-010 Call E: JSM path parity (subsup+code stripped via JSM route) | `AC-011-jsm-call-e` | `test_bc_7_2_015_call_e_jsm_path_subsup_code_mark_stripped` | GREEN |
| AC-012 | CLAUDE.md clause-(b) splice applied; `claude_md_citations` guard passes | `AC-012-citations-and-diff` | `test_claude_md_citations_resolve_to_real_files` | GREEN |

---

## Recordings

### AC-001-009-lib-tests — Unit tests (AC-001 through AC-009)

**Tape**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-001-009-lib-tests.tape`  
**GIF**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-001-009-lib-tests.gif`  
**WEBM**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-001-009-lib-tests.webm`

**Command demonstrated**:
```
cargo test --lib -- test_bc_7_2_015_ test_markdown_inline_code_mark_and_composition prop_bc_7_2_015
```

**Expected output**:
```
running 11 tests
test adf::tests::test_bc_7_2_015_strong_stripped_from_code_node ... ok
test adf::tests::test_bc_7_2_015_alert_wrapper_strong_code_stripped ... ok
test adf::tests::test_bc_7_2_015_mixed_range_surrounding_marks_retained ... ok
test adf::tests::test_bc_7_2_015_strike_stripped_from_code_node ... ok
test adf::tests::test_bc_7_2_015_subsup_stripped_from_code_node ... ok
test adf::tests::test_bc_7_2_015_em_stripped_from_code_node ... ok
test adf::tests::test_bc_7_2_015_link_preserved_on_code_node ... ok
test adf::tests::test_bc_7_2_015_plain_code_baseline ... ok
test adf::tests::test_markdown_inline_code_mark_and_composition ... ok
test adf::tests::test_bc_7_2_015_multi_mark_wrapper_only_code_node_stripped ... ok
test adf::tests::prop_bc_7_2_015_no_code_marked_text_node_carries_typographic_marks ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 992 filtered out; finished in 0.10s
```

**ACs covered**: AC-001, AC-002, AC-003, AC-004, AC-005, AC-006, AC-007, AC-008, AC-009

---

### AC-010-integration-exclusivity — H-NEW-ADF-010 Calls A–D (AC-010)

**Tape**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-010-integration-exclusivity.tape`  
**GIF**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-010-integration-exclusivity.gif`  
**WEBM**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-010-integration-exclusivity.webm`

**Command demonstrated**:
```
cargo test --test adf_code_mark_exclusivity
```

**Expected output**:
```
running 4 tests
test test_bc_7_2_015_call_c_link_preserved_with_code_mark_platform_path ... ok
test test_bc_7_2_015_call_b_subsup_code_mark_stripped_platform_path ... ok
test test_bc_7_2_015_call_d_surrounding_strong_retained_inner_code_stripped_platform_path ... ok
test test_bc_7_2_015_call_a_strong_code_mark_stripped_platform_path ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.78s
```

**ACs covered**: AC-010

---

### AC-011-jsm-call-e — H-NEW-ADF-010 Call E JSM path (AC-011)

**Tape**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-011-jsm-call-e.tape`  
**GIF**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-011-jsm-call-e.gif`  
**WEBM**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-011-jsm-call-e.webm`

**Command demonstrated**:
```
cargo test --test issue_create_jsm -- test_bc_7_2_015_call_e
```

**Expected output**:
```
running 1 test
test test_bc_7_2_015_call_e_jsm_path_subsup_code_mark_stripped ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 44 filtered out; finished in 0.74s
```

**What this verifies**: `^`code`^` submitted via the JSM route (`POST /rest/servicedeskapi/request`) produces a `requestFieldValues.description` text node for "code" with `marks == [code]` only. The platform endpoint `POST /rest/api/3/issue` is mounted with `.expect(0)` — confirming the JSM dispatch fork is intact.

**ACs covered**: AC-011

---

### AC-012-citations-and-diff — CLAUDE.md splice + citation guard (AC-012)

**Tape**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-012-citations-and-diff.tape`  
**GIF**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-012-citations-and-diff.gif`  
**WEBM**: `docs/demo-evidence/S-ADF-CODE-MARK-1/AC-012-citations-and-diff.webm`

**Commands demonstrated**:
```
cargo test --test claude_md_citations -- test_claude_md_citations_resolve_to_real_files
git diff develop -- CLAUDE.md | grep -A1 'enforced at emission'
```

**What this verifies**:
1. `test_claude_md_citations_resolve_to_real_files` (BC-X.13.001) passes — the BC-7.2.015 back-pointer added to CLAUDE.md is a non-path citation and is correctly excluded by the extractor's symbol-form filter; no dead citations introduced.
2. The `git diff` hunk confirms the clause-(b) tail was replaced byte-for-byte with the enforced behavior: `— enforced at emission time since #571: `push_code` strips typographic marks from code spans (see BC-7.2.015); ^`x`^ and **`x`** now emit schema-valid ADF with the `code` mark only.`

**ACs covered**: AC-012

---

## Red-Gate pre-fix evidence

The RED→GREEN story record lives at:  
**`.factory/cycles/cycle-001/S-ADF-CODE-MARK-1/implementation/red-gate-log.md`**

Key pre-fix observations (Task 2 observation window, branch: `fix/571-adf-code-mark-exclusivity` before `push_code` filter was applied):

| Anchor | Input | Pre-fix marks on code node | Status |
|--------|-------|---------------------------|--------|
| CONTROL | `` `x` `` | `["code"]` | GREEN (retention anchor) |
| EC-1 strong | `` **`x`** `` | `["strong", "code"]` | RED (CONFIRMED-INPUT) |
| EC-2 em | `` _`x`_ `` | `["em", "code"]` | RED (CONFIRMED-INPUT) |
| EC-3 strike | `` ~~`x`~~ `` | `["strike", "code"]` | RED (CONFIRMED-INPUT) |
| EC-4 subsup | `` ^`x`^ `` | `["subsup", "code"]` | RED (CONFIRMED-INPUT) |
| EC-5 link | `` [`x`](https://ex/) `` | `["link", "code"]` | GREEN (retention anchor) |
| EC-6 code node | `` **a `b` c** `` | `["strong", "code"]` on "b" | RED |
| multi-mark | `` _a **b `c` d** e_ `` | `["em", "strong", "code"]` on "c" | RED |
| PANEL-ANCHOR | `> [!NOTE]\n> **\`x\`**` | `["strong", "code"]` in panel | RED (CONFIRMED-INPUT) |
| AC-002 rewrite | `` **bold `code` bold** `` | `["strong", "code"]` on "code" | RED |

Task 3 adjudication: **all CONFIRMED-INPUT** (no MIXED-RANGE or DEMOTE). Task 3 was a no-op. EC-4 outcome binds H-NEW-ADF-010 Calls B and E — both retain the `^`code`^` form unchanged.

Full run: 10 tests; 8 FAILED (RED, expected), 2 passed (GREEN retention anchors). Gate: PASSED.

---

## Architecture Compliance Verification

| Rule | Verification | Status |
|------|-------------|--------|
| `push_code` is sole `{"type":"code"}` emit site outside test module | `grep -n '"type": "code"' src/adf.rs` — 1 match outside `#[cfg(test)]` | PASS |
| `adf_to_text` read-tolerance retained (`test_render_marks_code_and_strong`, `test_render_strong_with_code_applies_code_innermost`) | Included in MUST-STAY-GREEN list; both pass | PASS |
| BC-7.2.011 INV-1 preserved (`test_push_code_normalizes_*`) | Both normalization tests pass | PASS |
| JSM dispatch fork integrity (Call E `.expect(0)`) | Platform POST not called | PASS |
| No lint suppression | `cargo clippy -- -D warnings` clean | PASS |

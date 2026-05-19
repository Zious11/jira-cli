---
document_type: story
story_id: "S-382"
title: "Refactor JrError::InsufficientScope Display to use structured required_scope field (closes #382)"
wave: feature-followup
status: ready
intent: enhancement
feature_type: backend
scope: trivial
issue: 382
points: 2
priority: low
tdd_mode: strict
estimated_effort: small
depends_on: []
bc_anchors:
  - BC-1.6.042
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F4-quick-dev
spec_source: ".factory/phase-f1-delta-analysis/issue-382/delta-analysis.md"
implementation_strategy: tdd
files_modified:
  - src/error.rs
  - src/api/client.rs
  - src/cli/issue/create.rs
  - .factory/specs/prd/bc-1-auth-identity.md
test_files:
  - src/error.rs (inline unit tests — 2 new tests for required_scope Display branches)
breaking_change: false
# BC status: BC-1.6.042 anchored; F1d converged 3/3 at pass-08 (2026-05-19)
---

# S-382 — JrError::InsufficientScope Refactor

## Source of Truth

Full F1 delta analysis: `.factory/phase-f1-delta-analysis/issue-382/delta-analysis.md`
(F1d CONVERGED 3/3 at pass-08, 2026-05-19).

This story spec is a thin wrapper. The authoritative ACs, design template, lookup table,
and impact map live in delta-analysis.md and its sibling artifacts:
- `impact-boundary.md`
- `affected-artifacts.md`
- `design-validation.md`
- `po-decision-bc-parameterization.md`

## Problem Statement

`JrError::InsufficientScope` Display contains a hardcoded `"write:jira-work"` literal.
After PR #381 added JSM support requiring `write:servicedesk-request`, the generic error
message is stale: it names `write:jira-work` as the only scope workaround regardless of
which command failed. This refactor makes the scope reference structured and dynamic.

## Behavioral Contracts

| BC ID | Title | Clause |
|-------|-------|--------|
| BC-1.6.042 | InsufficientScope Display — parameterized scope hint with Empty-Some fallback | postcondition 1–3 + Empty-Some invariant |

## Acceptance Criteria

- **AC-1** (traces to BC-1.6.042 postcondition 1): Variant signature is
  `JrError::InsufficientScope { message: String, required_scope: Option<String> }`.
- **AC-2** (traces to BC-1.6.042 postcondition 2): Display uses thiserror
  expression-argument with `.filter(|s| !s.is_empty())`; preserves all existing
  Display text byte-for-byte except substituting `"write:jira-work"` with
  `{scope_hint}` derived from `required_scope`.
- **AC-3** (traces to BC-1.6.042 postcondition 3): New unit test
  `test_insufficient_scope_display_uses_required_scope_when_some` — Display contains
  `write:servicedesk-request` AND does NOT contain `write:jira-work` for
  `Some("write:servicedesk-request")`.
- **AC-4** (traces to BC-1.6.042 Empty-Some invariant): New unit test
  `test_insufficient_scope_display_empty_some_falls_back` — `Some("")` falls back;
  Display contains `write:jira-work`.
- **AC-5** (traces to BC-1.6.042 postcondition 2): Existing
  `tests/api_client.rs:136` (T-2) passes UNCHANGED — `None` fallback preserves the
  literal `"write:jira-work"`.
- **AC-6** (traces to BC-1.6.042 postcondition 1): 3 production construction sites
  updated per lookup table — `client.rs:700` and `client.rs:969` add
  `required_scope: None`; `create.rs:1983` adds
  `required_scope: Some("write:servicedesk-request".to_string())`.
- **AC-7** (traces to BC-1.6.042 postcondition 1): Destructure pattern at
  `src/cli/issue/create.rs:1982` updated from `{ message }` to `{ message, .. }`
  (E0027 prevention).

## Files to Touch

| File | Change |
|------|--------|
| `src/error.rs` | Variant struct fields + Display template + 2 new unit tests + 2 construction-site updates in existing unit tests |
| `src/api/client.rs` | 2 construction sites: add `required_scope: None` |
| `src/cli/issue/create.rs` | Destructure pattern fix (AC-7) + 1 construction site add `required_scope: Some(...)` |
| `.factory/specs/prd/bc-1-auth-identity.md` | BC-1.6.042 already updated in F1d; verify only — do not re-edit |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~1 k |
| delta-analysis.md (authoritative spec) | ~8 k |
| `src/error.rs` (read) | ~4 k |
| `src/api/client.rs` (read, 2 sites) | ~6 k |
| `src/cli/issue/create.rs` (read, 1 site + destructure) | ~5 k |
| `tests/api_client.rs` (verify T-2 unchanged) | ~3 k |
| Tool outputs + cargo test | ~4 k |
| **Total** | **~31 k** |

Well within single-agent context. No split required.

## Tasks

- [ ] Read delta-analysis.md (authoritative spec) before touching any source file
- [ ] Update `JrError::InsufficientScope` variant in `src/error.rs` to struct form
- [ ] Update Display impl to use `required_scope` with `.filter(|s| !s.is_empty())` fallback
- [ ] Add `test_insufficient_scope_display_uses_required_scope_when_some` (AC-3)
- [ ] Add `test_insufficient_scope_display_empty_some_falls_back` (AC-4)
- [ ] Update existing unit-test construction sites in `src/error.rs` (add `required_scope: None`)
- [ ] Update `src/api/client.rs` line ~700 — add `required_scope: None`
- [ ] Update `src/api/client.rs` line ~969 — add `required_scope: None`
- [ ] Update `src/cli/issue/create.rs` line ~1982 — fix destructure pattern to `{ message, .. }`
- [ ] Update `src/cli/issue/create.rs` line ~1983 — add `required_scope: Some("write:servicedesk-request".to_string())`
- [ ] Run `cargo test` — verify T-2 at `tests/api_client.rs:136` passes unchanged
- [ ] Run `cargo clippy -- -D warnings` — zero warnings
- [ ] Verify `cargo build --release` succeeds
- [ ] Per-story adversary 3/3 CLEAN before push (BC-5.39.001)

## Previous Story Intelligence

N/A — standalone quick-dev story; not part of an epic chain. Deferred from PR #381
(issue #288 JSM request-type support). No predecessor lessons to carry forward.

## Architecture Compliance Rules

- No new public API surface — this is an internal struct field addition only
- `JrError` lives in `src/error.rs`; do not move or split it
- thiserror `#[error(...)]` macro must remain the Display implementation mechanism
- No `impl fmt::Display` manual block — the thiserror derive handles it
- Construction sites use struct update syntax or named fields (not positional)
- No `#[allow]` suppressions without refactor justification (per CLAUDE.md)

## Library & Framework Requirements

- `thiserror` — version already pinned in `Cargo.toml` (do not upgrade)
- No new dependencies introduced by this story

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `src/error.rs` | Modify | Variant + Display + 2 new tests |
| `src/api/client.rs` | Modify | 2 construction sites only |
| `src/cli/issue/create.rs` | Modify | Destructure fix + 1 construction site |
| `tests/api_client.rs` | Read-only verify | T-2 must pass unchanged |

## Branch / PR Plan

- Branch: `chore/error-insufficient-scope-refactor` (or `feature/issue-382-error-scope`)
- Target: `develop`
- Commit style: conventional commits per CLAUDE.md (e.g., `refactor(error): parameterize InsufficientScope required_scope field`)
- PR closes #382

## Per-Story Delivery Notes

- Demos (Step 5) are LOCAL-ONLY per `docs/demo-evidence/` gitignore convention
- Per-story adversary 3/3 CLEAN required before push (BC-5.39.001)
- Research-agent validated design at F1d gate (pass-08); per-impl adversary handles code-side validation
- BC-1.6.042 was updated during F1d; only verify the BC file — do not re-edit unless the adversary finds a discrepancy

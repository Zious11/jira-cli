---
issue: 382
step: F1-Step-3
title: Impact Boundary Identification — JrError::InsufficientScope Display
status: complete
---

# Impact Boundary — Issue #382

`JrError::InsufficientScope` Display surfaces stale `write:jira-work` legacy text.
The shared error type was not updated when JSM scope work landed in PR #381.

---

## 1. Impact Boundary Table

| File | Role | Classification |
|------|------|----------------|
| `src/error.rs` | Variant definition + `#[error(...)]` Display macro | **MODIFIED** |
| `src/api/client.rs` | Two construction sites (line 700 `send()` first-401-body block, line 969 `parse_error()` helper) | **DEPENDENT** — no structural change; `message` field stays |
| `src/cli/issue/create.rs` | One construction site (line 1983) + one match arm (line 1982) | **DEPENDENT** — no structural change; behavior correct post-fix |
| `src/error.rs` unit test `insufficient_scope_display_includes_workarounds` (line 170) | Asserts Display contains `"write:jira-work"` | **MODIFIED** — assertion must be updated |
| `tests/api_client.rs` `test_401_scope_mismatch_returns_insufficient_scope` (line 100) | Asserts Display contains `"write:jira-work"` (line 136) | **DEPENDENT** — assertion passes unmodified via None-fallback at C-2 (verified) |
| `tests/oauth_flow_holdouts.rs` | AC-005 tests (lines 418, 450, 486) | **DEPENDENT** — only asserts `"Insufficient token scope"` prefix; no `write:jira-work` pin; no change needed |
| `tests/issue_create_jsm.rs` | C-01 test (line 1522) | **DEPENDENT** — asserts `write:servicedesk-request`, `jr auth refresh`, `jr auth login`; no `write:jira-work` pin; no change needed |

---

## 2. Construction Sites

These are the sites that call `JrError::InsufficientScope { message: ... }` to construct the variant:

| # | File | Line | Context |
|---|------|------|---------|
| C-1 | `src/api/client.rs` | 700 (`send()` first-401-body block) | Blanket 401 auto-refresh path: early-exit when body contains "scope does not match" |
| C-2 | `src/api/client.rs` | 969 (`parse_error()` helper) | `parse_error()` helper: same substring guard, different call path (non-retry) |
| C-3 | `src/cli/issue/create.rs` | 1983 | JSM request handler: re-constructs the variant with an enriched `message` that appends `write:servicedesk-request` context |

All three pass only a `message: String`. No structural change is required at construction sites under either design option.

---

## 3. Consumer Sites (match/downcast)

| # | File | Line | Context |
|---|------|------|---------|
| M-1 | `src/error.rs` | 75 | `exit_code()` arm `JrError::InsufficientScope { .. } => 2` — wildcard; no change |
| M-2 | `src/cli/issue/create.rs` | 1982 | `Ok(JrError::InsufficientScope { message }) =>` — destructures `message` field to re-wrap it; no change needed as long as field stays named `message` |

---

## 4. Test Sites with Display Assertions

Tests that assert specific text in `InsufficientScope`'s Display output:

| # | File | Test Function | Line(s) | Pins `write:jira-work`? | Needs Update? |
|---|------|---------------|---------|------------------------|---------------|
| T-1 | `src/error.rs` | `insufficient_scope_exit_code` | 129–137 (construction at **131**) | No — only asserts `exit_code() == 2` | **YES** — needs `required_scope: None` when variant gains second field (option a) |
| T-2 | `src/error.rs` | `insufficient_scope_display_includes_workarounds` | 170–186 (construction at **171**) | YES (line 180) | **YES** — assertion must be updated + `required_scope: None` needed |
| T-3 | `tests/api_client.rs` | `test_401_scope_mismatch_returns_insufficient_scope` | 100–144 | YES (line 136) | No — None-fallback at C-2 preserves `write:jira-work` literal in Display; assertion at line 136 still satisfied |
| T-4 | `tests/oauth_flow_holdouts.rs` | `test_s_1_06_h_022_scope_mismatch_lowercase_dispatches_insufficient_scope` | 418–444 | No — pins `"Insufficient token scope"` only | No |
| T-5 | `tests/oauth_flow_holdouts.rs` | `test_s_1_06_h_022_scope_mismatch_mixed_case_dispatches_insufficient_scope` | 450–479 | No — pins `"Insufficient token scope"` only | No |
| T-6 | `tests/oauth_flow_holdouts.rs` | `test_s_1_06_h_022_non_scope_401_and_403_do_not_dispatch_insufficient_scope` | 486–557 | No — negation tests only | No |
| T-7 | `tests/issue_create_jsm.rs` | `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` | 1522–1583 | No — pins `write:servicedesk-request` hint injected at C-3 | No |
| T-8 | `tests/api_client.rs` | `test_401_without_scope_mismatch_falls_through_to_not_authenticated` | 146–181 | No — negation test | No |
| T-9 | `tests/api_client.rs` | `test_401_scope_mismatch_matches_case_insensitively` | 183–216 | No — pins `"Insufficient token scope"` only | No |
| T-10 | `tests/api_client.rs` | `test_non_401_with_scope_substring_does_not_dispatch_to_insufficient_scope` | 218–255 | No — negation test | No |

T-1 and T-2 require updates. Both are construction sites in `src/error.rs` that need `required_scope: None` added when the variant signature widens under option (a): **2 test construction-call updates needed in `src/error.rs`** (lines 131 + 171). No assertion updates needed elsewhere — T-3 is satisfied by the None-fallback at C-2 which preserves the `write:jira-work` literal in Display.

---

## 5. External Dependencies / Re-exports

`src/lib.rs` re-exports `pub mod error` (line 7), making `jr::error::JrError` visible to integration tests. The integration tests reference `JrError` by string-matching Display output, not by importing the type directly (no `use jr::error::JrError` found in test files). The variant's public `message: String` field is accessed only in `create.rs` (M-2). As long as the field name does not change, no integration test import paths break.

---

## 5b. Dependent Doc/Spec Surfaces (verify-only)

These spec and doc files reference `InsufficientScope` behavior or BC-1.6.042. They are **verify-only** — no edits required under option (a) parameterization, but they must be checked after implementation to confirm they remain accurate.

| File | Location | Reference | Verify Action |
|------|----------|-----------|---------------|
| `.factory/specs/prd/edge-case-catalog.md` | Line 78 | `Covered by BC-1.6.042; holdout H-012` | Confirm BC-1.6.042 coverage assertion remains accurate post-fix; no text change needed |
| `.factory/specs/prd/BC-INDEX.md` | Line 122 | Source cell for BC-1.6.042 cites `tests/api_client.rs:99-144` | Confirm line reference remains accurate; no text change needed |
| `.factory/specs/prd/holdout-scenarios.md` | Lines 138–145 | H-012 — `InsufficientScope` scope-mismatch holdout | Passes under option (a) design; verify-only |
| `.factory/specs/prd/holdout-scenarios.md` | Lines 658–682 | H-NEW-JSM-RT-003 — JSM OAuth scope hint holdout | Passes under option (a) design; verify-only |
| `CLAUDE.md` | Gotchas section | General CLI behavior, auth flows, env-var test seams | No test-seam env-var or hidden behavior change introduced by #382; no update needed |
| `.factory/specs/prd/CANONICAL-COUNTS.md` | BC counts | BC count source of truth | BC count stable under option (a); no propagation update needed |

---

## 6. Architectural Change Assessment

**No new modules required. No new interfaces required. No module boundaries cross.**

`error.rs` is pure (no I/O, no side effects). The change is entirely internal to the `#[error(...)]` Display macro string embedded in the variant definition. The purity boundary is unaffected. The `InsufficientScope` variant shape (`{ message: String }`) does not need to change under either design option below; if option (a) is chosen, a second field is added but existing construction sites receive a default value for it.

---

## 7. Recommended Design Options

### Option (a): Add `required_scope: Option<String>` field

Add a second field to the variant. Construction sites that lack scope context pass `None`; the JSM path in `create.rs` passes `Some("write:servicedesk-request")`. The `#[error]` macro renders a scope-aware hint line when `required_scope` is `Some`.

**Rationale:** Encodes structured context in the type, making the Display self-contained and compile-time-checkable. Construction site C-3 already knows the required scope — it just discards it today. Downside: three construction sites need a second field (two pass `None`; one passes `Some`), and the `message` field at M-2 destructure is unchanged.

### Option (b): Construct Display dynamically from a broader hint enum

Replace the hard-coded workaround string with a `hint: ScopeHint` enum (`JiraWork` | `ServiceDeskRequest`), letting the Display implementation select the appropriate human-readable workaround text per variant. The enum is defined inside `error.rs` (no new file needed).

**Rationale:** Makes the type extensible if a third scope surface (e.g., Confluence) arises. More verbose but avoids `Option<String>` looseness. Construction site C-3 passes `ScopeHint::ServiceDeskRequest`; C-1 and C-2 pass `ScopeHint::JiraWork` (preserving today's existing text for the legacy case).

### Recommendation: Option (a)

Option (a) is narrower, requires fewer changes, and directly addresses the issue: the JSM path already has the required scope string at C-3 — it just needs a way to pass it through to the Display. `Option<String>` is idiomatic Rust for an optional enrichment field. Option (b) adds an enum type for two variants where `Option<String>` suffices. Choose option (b) only if a third scope surface is anticipated imminently.

---

## 8. Summary of Files to Touch

| File | Change |
|------|--------|
| `src/error.rs` | Update `#[error(...)]` Display text (remove/update `write:jira-work`); add `required_scope: Option<String>` if option (a); update unit test `insufficient_scope_display_includes_workarounds` |
| `tests/api_client.rs` | No change — None-fallback preserves `write:jira-work`; assertion at line 136 still passes |

All other files: no change.

---

## Change Log

- [REVISED 2026-05-19 per F1d adversary-pass-02 H-01 + M-01 + L-01]
  - H-01: Corrected cross-artifact contradiction on T-3 / `tests/api_client.rs` line 136. Section 1 table cell reclassified from MODIFIED to DEPENDENT. Section 4 T-3 "Needs Update?" cell changed to No with None-fallback rationale. Section 4 summary paragraph revised to remove the "+1 Display assertion update" claim and state no assertion updates needed. Section 8 `tests/api_client.rs` row changed from "Update assertion" to "No change — None-fallback preserves `write:jira-work`". Siblings delta-analysis.md:84 and design-validation.md Refinement 3 were correct; this file now matches.
  - M-01: Added 2 missing rows to Section 5b: `CLAUDE.md` (no test-seam env-var or hidden behavior change) and `.factory/specs/prd/CANONICAL-COUNTS.md` (BC count stable under option (a); no propagation update needed). Section previously had 4 rows; now has 6, matching delta-analysis.md and affected-artifacts.md enumeration.
  - L-01: Added symbol-name anchors to `src/api/client.rs` line references in Section 1 Impact Boundary Table and Section 2 Construction Sites table — `:700` annotated as `send()` first-401-body block; `:969` annotated as `parse_error()` helper — for forward-stability against line-number drift from concurrent PRs.
- [REVISED 2026-05-19 per F1d adversary-pass-01 F-01 + F-03]
  - F-01: Added `src/error.rs:131` (`insufficient_scope_exit_code` test) as a second construction-site in the test sites table (T-1). Updated summary to state "2 test construction-call updates needed in `src/error.rs` (lines 131 + 171)". Prior version enumerated only line 171.
  - F-03: Added section 5b "Dependent Doc/Spec Surfaces (verify-only)" enumerating `edge-case-catalog.md:78` (BC-1.6.042 reference), `BC-INDEX.md:122` (source cell), and `holdout-scenarios.md` lines 138–145 (H-012) and 658–682 (H-NEW-JSM-RT-003). All four entries are verify-only; no text changes to those files required under option (a).

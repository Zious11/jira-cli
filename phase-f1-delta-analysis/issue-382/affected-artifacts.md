---
document_type: phase-f1-affected-artifacts
issue: 382
title: "JrError::InsufficientScope Display — structured/dynamic scope reference"
phase: phase-f1-delta-analysis
step: step-4-affected-artifact-mapping
producer: business-analyst
date: 2026-05-19
intent: enhancement
feature_type: backend
trivial_classification: TRIVIAL
status: complete
---

# Affected Artifact Map — Issue #382

## Issue Summary

`JrError::InsufficientScope` Display contains a hardcoded `"write:jira-work"` literal in
`src/error.rs`. After JSM request-type support added `write:servicedesk-request` as a
required scope (issue #288, PR #381), the generic error message is stale: it names
`write:jira-work` as the only scope workaround regardless of which command failed. The
issue requests that the scope reference be structured/dynamic so the error surface is
accurate at the call-site.

---

## 1. BC Table

| BC ID | Title / Key Behavior | Classification | Reason |
|---|---|---|---|
| BC-1.6.042 — MODIFY (option-a parameterize in-place; see po-decision-bc-parameterization.md) | 401 + `scope does not match` body → InsufficientScope with 5 required substrings | **MODIFY** (option-a parameterize in-place; see `po-decision-bc-parameterization.md`) | PO decision (adversary-pass-01): parameterize BC-1.6.042 in-place. Behavior line updated to replace the hardcoded `write:jira-work` assertion with a parameterized-field contract (`None` falls back to `write:jira-work`; `Some("write:servicedesk-request")` for JSM path). No BC split, no new ID, no BC-INDEX or CANONICAL-COUNTS change. |
| BC-X.3.005 | 401 + scope-mismatch → InsufficientScope; 403 NOT dispatched | **UNCHANGED** | The dispatch logic (401 status gate + substring match) is not being changed. Only the Display output changes. |
| BC-1.3.023 | DEFAULT_OAUTH_SCOPES includes `write:jira-work` and `write:servicedesk-request` | **UNCHANGED** | Scope constant itself is not changing. However, this BC is the root motivation: having two required scopes exposed why the hardcoded hint is stale. No modification needed — it already documents both scopes. |
| BC-1.6.043 | 401 without scope-mismatch substring → NotAuthenticated, NOT InsufficientScope | **UNCHANGED** | Dispatch boundary unaffected. |
| BC-1.6.044 | 401 scope-mismatch match is case-insensitive | **UNCHANGED** | Matching logic unaffected. |
| BC-1.6.045 | Non-401 with scope-mismatch substring does NOT dispatch to InsufficientScope | **UNCHANGED** | Status gate unaffected. |
| BC-3.8.009 (via H-NEW-JSM-RT-003) | JSM 401 scope-mismatch surfaces `write:servicedesk-request` hint | **UNCHANGED** | The JSM create handler already re-constructs `InsufficientScope { message }` with an appended `write:servicedesk-request` hint (call-site injection). This BC is satisfied by the call-site, not by the error type's Display. No change needed — but the test `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` is a regression-risk-zone test (see Section 3). |

### New BCs Required

| New BC ID (proposed) | Description | Rationale |
|---|---|---|
| BC-1.6.047 (candidate — WITHDRAWN) | ~~InsufficientScope Display surfaces the runtime-resolved scope name, not a hardcoded `write:jira-work` literal~~ | **Not needed.** PO decision (adversary-pass-01 F-02): option (a) parameterize BC-1.6.042 in-place. Both paths (`write:jira-work` fallback + `write:servicedesk-request` JSM) are instantiations of one parameterized behavior. Splitting into two BCs overstates the distinction and inflates BC count. BC-INDEX, CANONICAL-COUNTS.md, and story bodies require no changes. See `po-decision-bc-parameterization.md`. |

---

## 2. VP (Verification Property) Table

No `ARCH-INDEX.md` was found at `.factory/specs/verification-architecture/` — that path is empty (the directory does not exist in this repo at this time). Verification properties are encoded in BCs and holdout scenarios for this project. The relevant verification anchors are:

| Verification Anchor | Type | Touches Issue #382 | Notes |
|---|---|---|---|
| H-012 | Holdout scenario | YES — REGRESSION RISK | Asserts `write:jira-work` substring in InsufficientScope Display. If the Display is made dynamic, this holdout's assertion must be updated to match the new contract. |
| H-NEW-JSM-RT-003 | Holdout scenario | YES — REGRESSION RISK | Asserts `write:servicedesk-request` in stderr for JSM 401. This is satisfied at the call-site (create.rs), not in error.rs Display, so it should survive unchanged — but must be verified. |
| H-022 (via S-1.06) | Holdout scenario | INDIRECT | Pins case-insensitive dispatch. Dispatch logic unchanged; no modification expected. |

---

## 3. Story Regression-Risk-Zone

Stories whose shipped implementation tests assert directly against InsufficientScope Display
output. Any change to `src/error.rs::JrError::InsufficientScope` Display format may cause
these to fail.

| Story ID | Title | Test File(s) | InsufficientScope Assertion | Risk Level |
|---|---|---|---|---|
| S-1.06 | OAuth flow holdout suite | `tests/oauth_flow_holdouts.rs` | Lines ~418-550: asserts `InsufficientScope` dispatch (substring match on error message); also asserts `NOT InsufficientScope` for non-scope 401s | MEDIUM — dispatch dispatch asserted, not exact Display text; Display text change may not break unless `to_string()` is called |
| H-012 anchor | Pre-existing (no story owner, GAP-H-004) | `tests/api_client.rs:100-144` | Line 136: `s.contains("write:jira-work")` — DIRECT assertion on hardcoded literal | HIGH — will break if `write:jira-work` is removed from Display |
| issue-288-pr4-dispatch | JSM dispatch story | `tests/issue_create_jsm.rs:1303-1580` | Lines ~1363, 1519-1547: asserts `write:servicedesk-request` in stderr via call-site re-wrap | LOW — satisfied by call-site injection, not by error.rs Display; should survive |
| (inline unit tests) | N/A | `src/error.rs:170-185` | `insufficient_scope_display_includes_workarounds`: asserts `write:jira-work` literal | HIGH — direct unit test of Display; will break if literal is removed |
| (inline unit tests) | N/A | `src/error.rs:129-136` | `insufficient_scope_exit_code`: asserts exit code 2 | NONE — exit code not changing |

---

## 4. Tests In Scope (Confirmed by Grep)

All test locations asserting on InsufficientScope Display or dispatch:

| File | Lines | Assertion Type | Change Required? |
|---|---|---|---|
| `src/error.rs` | 129-136 | Exit code = 2 | No |
| `src/error.rs` | 170-185 | Display contains `write:jira-work` (literal) | YES — if scope becomes dynamic |
| `tests/api_client.rs` | 100-144 | Display contains `write:jira-work` | No — None-fallback at C-2 preserves `write:jira-work` Display literal verbatim; assertion at `tests/api_client.rs:136` still satisfied |
| `tests/api_client.rs` | 146-181 | NOT InsufficientScope for session-expired 401 | No |
| `tests/api_client.rs` | 183-216 | InsufficientScope on mixed-case substring | No |
| `tests/api_client.rs` | 219-255 | NOT InsufficientScope for 403 | No |
| `tests/oauth_flow_holdouts.rs` | 403-550 | Dispatch assertions (not Display literal) | Likely no |
| `tests/issue_create_jsm.rs` | 1303-1580 | `write:servicedesk-request` in stderr | No (call-site injection) |
| `src/cli/auth/tests/mod.rs` | 339, 355 | Scope string constant (DEFAULT_OAUTH_SCOPES) | No |

---

## 5. Intent Classification

| Attribute | Value |
|---|---|
| Intent | `enhancement` |
| Classification signals | "refactor", "stale text", "hardcoded" — code functions correctly today; message is contextually inaccurate |
| Not a bug-fix | The error path works; no wrong behavior, only stale scope name in Display |
| Severity | N/A (enhancement) |

---

## 6. Trivial vs Standard Scope Classification

**Decision: TRIVIAL (revised pass-02; quick-dev route)**

**Reasoning:**

Revised to TRIVIAL by F1d adversary-pass-02. The implementation narrows to a single
semantic concept: add `scope_hint: Option<String>` as an additive optional field to
`JrError::InsufficientScope`, extend Display conditionally, and propagate via 3
production call-sites + 2 test sites. Specifically:

1. Add `scope_hint: Option<String>` to `JrError::InsufficientScope` (additive; existing
   construction sites gain `scope_hint: None` with no behavioral change).
2. Extend Display: when `scope_hint` is `Some(s)`, append ` Use a classic token with
   "{s}" scope.` — the `None` path is byte-for-byte identical to the current Display.
3. Update 3 production call-sites (`src/api/client.rs` ×2, `src/cli/issue/create.rs` ×1).
4. Update 2 test assertions that require the extended Display on the JSM (C-3) path.
5. No arch change; no new module; no new external dependency.

Regression risk is LOW under the None-fallback: exhaustive Rust match catches missed
construction sites at compile time; the C-1/C-2 paths (None) remain unchanged; only the
C-3 JSM path gets the `Some("write:servicedesk-request")` value. Per-story adversary
3/3 CLEAN remains the F4 gate.

**Known cosmetic — accepted for #382:** Post-refactor, the JSM C-3 path renders the scope
name twice in Display (once in the C-3-enriched message already present in `message`,
once in the new `scope_hint` workaround line). This is cosmetically suboptimal but
functionally harmless — the duplication reinforces the scope name. Removing the C-3
enrichment is out of scope for #382 (separate refactor with its own AC surface). If user
feedback flags the duplication, file a follow-up issue.

---

## 7. Feature Type Classification

**Feature type: `backend`**

- No CLI surface changes (flags, subcommands, command names)
- No UX changes beyond error message text in stderr
- No external API contract changes
- No new external dependencies
- All changes confined to `src/error.rs`, `src/api/client.rs`, `src/cli/issue/create.rs`
  (call-site updates), and corresponding test files

---

## 8. Files NOT Changed (Regression Baseline)

These files must not be modified by this issue's implementation. Any diff touching them
is a scope violation and must be flagged in review.

| File | Reason Excluded |
|---|---|
| `src/api/auth.rs` | Scope constant (DEFAULT_OAUTH_SCOPES) is not changing |
| `src/api/pagination.rs` | Unrelated |
| `src/cli/issue/list.rs` | Unrelated |
| `src/cli/issue/view.rs` | Unrelated |
| `src/cli/issue/workflow.rs` | Unrelated |
| `src/cli/auth/` (all files) | Auth flow not changing |
| `src/cli/assets.rs` | Unrelated |
| `src/cli/board.rs` | Unrelated |
| `src/cli/sprint.rs` | Unrelated |
| `src/cli/worklog.rs` | Unrelated |
| `src/cache.rs` | Unrelated |
| `src/config.rs` | Unrelated |
| `src/jql.rs` | Unrelated |
| `src/adf.rs` | Unrelated |
| `src/duration.rs` | Unrelated |
| `tests/issue_list_errors.rs` | Unrelated |
| `tests/bulk_*.rs` | Unrelated |
| `tests/search_*.rs` | Unrelated |
| `tests/migration_*.rs` | Unrelated |
| `.factory/specs/prd/bc-3-issue-write.md` | BC-3.8.009 satisfied at call-site; no modification needed unless F2 spec evolution determines otherwise |
| `.factory/specs/prd/cross-cutting.md` | BC-X.3.005 dispatch logic unchanged |
| `.factory/specs/prd/bc-1-auth-identity.md` | Only BC-1.6.042 needs modification (see Section 1); 1.6.043-045 and 1.3.023 are unchanged |

### Docs/Index Surfaces Verified Unchanged

These files reference `InsufficientScope` behavior or BC-1.6.042 but require no edits under option (a) parameterization. They are verify-only surfaces — implementation must confirm each reference remains accurate after the change lands.

| File | Reference / Location | Why Unchanged | Verify Action |
|------|----------------------|---------------|---------------|
| `CLAUDE.md` (Gotchas section) | No test-seam env-var or hidden behavior introduced by this change | No new `JR_*` env-var, no architectural edge case, no dispatch behavior change — Gotchas section has nothing to add | Confirm no `JR_*` or behavioral gotcha was introduced during F4 |
| `.factory/specs/prd/BC-INDEX.md` (line 122) | Source cell for BC-1.6.042 cites `tests/api_client.rs:99-144` | BC count is stable under option (a); BC-1.6.042 ID and title are unchanged; the Source cell citation remains accurate because T-2 (`tests/api_client.rs:100-144`) still passes unmodified (None-fallback preserves the `write:jira-work` assertion byte-for-byte) | Confirm `tests/api_client.rs:99-144` citation still resolves to the correct test after F4 changes |
| `.factory/specs/prd/CANONICAL-COUNTS.md` | BC cumulative count (57 in bc-1) | No new BC added (BC-1.6.047 candidate withdrawn); count is stable | Confirm count unchanged post-F4 |
| `.factory/specs/prd/edge-case-catalog.md` (line 78) | `Covered by BC-1.6.042; holdout H-012` | BC-1.6.042 still covers this edge case under parameterization; the coverage assertion remains accurate | Confirm edge-case description still aligns with updated BC-1.6.042 Behavior line |
| `.factory/specs/prd/holdout-scenarios.md` (lines 138–145, H-012) | `InsufficientScope` scope-mismatch holdout; asserts `write:jira-work` substring | Under option (a), the `None`→`write:jira-work` fallback path preserves the assertion; H-012 passes unmodified | Run H-012 in validation; confirm `write:jira-work` still present in Display for None path |
| `.factory/specs/prd/holdout-scenarios.md` (lines 658–682, H-NEW-JSM-RT-003) | JSM OAuth scope hint holdout; asserts `write:servicedesk-request` in stderr | Satisfied at call-site injection (C-3 in `create.rs`); `required_scope: Some("write:servicedesk-request")` on C-3 reinforces this; holdout passes | Run H-NEW-JSM-RT-003 in validation; confirm `write:servicedesk-request` still present |
| `docs/superpowers/specs/2026-04-17-insufficient-scope-error-design.md` | Historical v1 design record; deliberately frozen at 2026-04-17 implementation state. Stale `{ message: String }` references (lines 23, 63, 90) reflect v1 variant signature, not post-#382. | Not a living doc; intentionally not updated. | None — frozen record. |
| `docs/superpowers/plans/2026-04-17-insufficient-scope-error.md` | Historical v1 plan record; deliberately frozen at 2026-04-17 implementation state. Stale `{ message: String }` references (lines 24, 47, 57, 111, 193, 416, 448) reflect v1 variant signature, not post-#382. | Not a living doc; intentionally not updated. Same rationale as spec above. | None — frozen record. |

> **BC-INDEX.md summary terseness (L-02 intent — accepted):** BC-INDEX.md line 122 summary text says "InsufficientScope with 5 required substrings" — still accurate (count is 5) but does not hint that substring #3 is now parameterized. Summary text retained as-is (terse pointer convention); BC body is the authoritative description. BC body line 472 has the full parameterization description. Readers wanting depth follow the link.

---

## 9. Summary

| Attribute | Value |
|---|---|
| BCs to MODIFY | BC-1.6.042 (option-a parameterize in-place; see `po-decision-bc-parameterization.md`) |
| BCs NEW | None — BC-1.6.047 candidate withdrawn (PO decision, adversary-pass-01 F-02) |
| BCs UNCHANGED | BC-X.3.005, BC-1.3.023, BC-1.6.043, BC-1.6.044, BC-1.6.045, BC-3.8.009 |
| VPs touched | H-012 (update required), H-NEW-JSM-RT-003 (verify only) |
| Regression-risk-zone stories | S-1.06, issue-288-pr4-dispatch |
| High-risk tests | `src/error.rs:170-185`, `tests/api_client.rs:100-144` |
| Intent | enhancement |
| Trivial / Standard | TRIVIAL (revised pass-02) |
| Feature type | backend |
| Architect impact | Single module + 2 call-sites + 1 destructure fix; no architecture change. `src/cli/issue/create.rs:1982` destructure pattern `{ message }` → `{ message, .. }` required (compile-break E0027 per F1d adversary-pass-04 L-02). |

---

## Change Log

- [REVISED 2026-05-19 per F1d adversary-pass-04 L-02] — M-2 pattern reclassified MODIFIED; F4 must add `..` to destructure. `src/cli/issue/create.rs:1982` destructure `Ok(JrError::InsufficientScope { message }) => ...` will fail E0027 when `required_scope: Option<String>` is added; fix is `{ message, .. }`. Section 9 "Architect impact" row updated to record the additional destructure fix. Previous classification as DEPENDENT (no change needed at line 1982) was incorrect — the struct-widening at `src/error.rs` propagates a compile-break to every exhaustive destructure of `InsufficientScope`.
- [REVISED 2026-05-19 per F1d adversary-pass-03 M-01 + M-03 + L-01 + L-02 intent]
  - M-01: Propagated TRIVIAL reclassification from delta-analysis.md to 3 sites: frontmatter `trivial_classification`, Section 6 heading, Section 9 summary table row. Section 6 rationale paragraph rewritten to match delta-analysis.md line 54 TRIVIAL rationale (additive `scope_hint: Option<String>`, 3 production + 2 test sites, no arch change, LOW regression risk under None-fallback, per-story adversary 3/3 CLEAN as F4 gate).
  - M-03: Two historical superpowers docs added to Section 8 "Docs/Index Surfaces Verified Unchanged": `docs/superpowers/specs/2026-04-17-insufficient-scope-error-design.md` and `docs/superpowers/plans/2026-04-17-insufficient-scope-error.md`. Both are frozen v1 records; stale `{ message: String }` references are intentional. Verified-unchanged doc-surface count now 8 (was 6).
  - L-01: Known cosmetic (JSM C-3 path renders scope name twice in Display) documented as accepted wart in Section 6. Duplication is functionally harmless; removing C-3 enrichment is out of scope for #382.
  - L-02: BC-INDEX.md summary terseness accepted. Note added to Section 8 BC-INDEX row: terse pointer convention retained; BC body line 472 has full parameterization description.
- [REVISED 2026-05-19 per F1d adversary-pass-02 H-01 + L-03]
  - H-01: Section 4 row for `tests/api_client.rs:100-144` "Change Required?" cell corrected from "YES — if scope becomes dynamic" to "No — None-fallback at C-2 preserves `write:jira-work` Display literal verbatim; assertion at `tests/api_client.rs:136` still satisfied". The conditional was inconsistent with the rest of the artifact set's UNCHANGED classification for that test.
  - L-03: Section 1 BC table row for BC-1.6.042 pruned. Implementation detail `(runtime-parameterized via JrError::InsufficientScope { required_scope: Option<String> })` removed from Title/Key Behavior cell — that detail belongs in the BC body, not the summary cell. BC ID cell updated to include the MODIFY classification inline per the prescribed format.
- [REVISED 2026-05-19 per F1d adversary-pass-01 F-02 + F-04 + F-06 + F-07]
  - F-02: BC-1.6.047 candidate withdrawn. BC-1.6.042 classification updated to "MODIFY (option-a parameterize in-place; see po-decision-bc-parameterization.md)". PO decision confirms single-BC-in-place is correct; no BC-INDEX or CANONICAL-COUNTS change.
  - F-04: Added `edge-case-catalog.md:78` to "Docs/Index Surfaces Verified Unchanged" — references BC-1.6.042; coverage assertion remains accurate under parameterization (verify-only).
  - F-06: Added `BC-INDEX.md:122` Source cell to "Docs/Index Surfaces Verified Unchanged" — Source cell cites `tests/api_client.rs:99-144` which passes unmodified under option (a) None-fallback; no second-test citation required at this time.
  - F-07: Added `CLAUDE.md` Gotchas section to "Docs/Index Surfaces Verified Unchanged" — no new test-seam env-var or hidden behavior introduced by this change; Gotchas section requires no update.

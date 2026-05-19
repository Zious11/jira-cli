---
document_type: delta-analysis-report
feature_name: "JrError::InsufficientScope Display refactor"
issue: 382
created: 2026-05-19
spec_version_at_analysis: "N/A"
status: under-review
intent: "enhancement"
feature_type: "backend"
scope: "trivial"
severity: "N/A"
sources:
  - impact-boundary.md (architect, F1-Step-3; revised F1d adversary-pass-01 F-01 + F-03; revised F1d adversary-pass-02 H-01 + M-01 + L-01; revised F1d adversary-pass-03 M-02; revised F1d adversary-pass-04 L-02 — M-2 reclassified MODIFIED; revised F1d adversary-pass-05 M-01 + L-03)
  - affected-artifacts.md (business-analyst, F1-Step-4; revised F1d adversary-pass-01 F-02 + F-04 + F-06 + F-07; revised F1d adversary-pass-02 H-01 + L-03; revised F1d adversary-pass-03 M-01 + M-03 + L-01 + L-02 intent)
  - design-validation.md (research-agent, pre-F2 gate; revised F1d adversary-pass-01 F-01 + F-05; revised F1d adversary-pass-02 H-02 + M-05 + L-02; revised F1d adversary-pass-03 L-03 + AC-4 added; revised F1d adversary-pass-05 M-02 + L-02)
  - po-decision-bc-parameterization.md (product-owner, F1d adversary-pass-01 F-02)
  - bc-1-auth-identity.md BC-1.6.042 (Empty-Some policy added in pass-02)
---

# Delta Analysis Report: JrError::InsufficientScope Display Refactor

## Feature Request

- **Issue:** [#382 — JrError::InsufficientScope Display refactor](https://github.com/Zious11/jira-cli/issues/382)
- **Context:** Deferred from PR #381 / issue #288 (JSM request-type support)
- **Date:** 2026-05-19

`JrError::InsufficientScope` Display contains a hardcoded `"write:jira-work"` literal. After PR #381 added JSM support requiring `write:servicedesk-request`, the generic error message is stale: it names `write:jira-work` as the only scope workaround regardless of which command failed. This refactor makes the scope reference structured and dynamic.

---

## Classifications

### Intent Classification

**Classified intent:** `enhancement`

**Rationale:** The code path functions correctly today — the 401 scope-mismatch error is caught and surfaced. The message content is contextually inaccurate for JSM calls, not broken. No wrong behavior, only stale Display text. Signals: "refactor", "stale text", "hardcoded" — characteristic of an enhancement, not a bug-fix.

**Status: APPROVED by human.**

### Feature Type Classification

**Classified type:** `backend`

**Rationale:** No CLI surface changes (flags, subcommands). No UX changes beyond error message text in stderr. No external API contract changes. No new external dependencies. All changes confined to `src/error.rs`, `src/api/client.rs`, `src/cli/issue/create.rs`, and their corresponding test files.

**Status: APPROVED by human.**

### Trivial Scope Classification

**Classified scope:** `TRIVIAL (quick-dev route)`

**Rationale:** Per phase-f1-delta-analysis trivial criteria, this change is a single semantic concept: one BC modification with parameterization. Impact surface: 3 production construction call-sites + 2 test call-sites (small, bounded). No architecture change. No new external dependencies. Regression risk is LOW: the `None`-fallback at C-1 and C-2 preserves existing `"write:jira-work"` Display text byte-for-byte; Rust exhaustive-match catches any missed construction sites at compile time; T-2 (`tests/api_client.rs:136`) assertion passes unmodified. The prior STANDARD classification (from affected-artifacts.md Section 6) was predicated on wide test-assertion surface and the possibility of replacing the hardcoded literal; under the additive `Option<String>` design with None-fallback, that concern does not apply. Per-story-delivery adversary 3/3 CLEAN remains the gate during F4.

**Status: APPROVED by human.**

### Severity Classification

**Classified severity:** `N/A` (enhancement, not bug-fix)

---

## BC Decision

**BC-1.6.042:** Parameterized in-place under option (a) — PO decision (adversary-pass-01 F-02). See `po-decision-bc-parameterization.md`.

- Behavior line updated to replace the hardcoded `write:jira-work` assertion with a runtime-parameterized-field contract: `None` falls back to `write:jira-work` (preserves all existing test pins); `Some("write:servicedesk-request")` for the JSM path.
- **Empty-Some policy added (pass-02):** BC-1.6.042 now includes an explicit Empty-Some policy. Construction sites MUST pass either `None` or `Some(s)` where `s` is a non-empty ASCII scope name. The Display impl treats `Some("")` identically to `None` via `.filter(|s| !s.is_empty())` in the thiserror template. A new unit test (AC-4) MUST pin `Some("")` → `write:jira-work` fallback behavior.
- BC count is stable (57 cumulative in bc-1). BC-INDEX title, row, and Source cell are unchanged. CANONICAL-COUNTS.md is unchanged.
- BC-1.6.047 candidate **withdrawn** — both paths are instantiations of one parameterized behavior; splitting overstates the distinction and inflates BC count with no analytical gain.

---

## Impact Assessment

### Component Impact Table

| Component | Change Type | Notes |
|-----------|-------------|-------|
| `src/error.rs` | MODIFIED | `InsufficientScope` variant gains `required_scope: Option<String>` field; `#[error]` uses expression-argument form with `.filter(|s| !s.is_empty())` per BC-1.6.042 Empty-Some policy |
| `src/api/client.rs` | MODIFIED | 2 construction sites (lines 700 `send()` first-401-body block, 969 `parse_error()` helper) gain `required_scope: None` (back-compat fallback; preserves `"write:jira-work"` Display text for platform-write path) |
| `src/cli/issue/create.rs` | MODIFIED | (1) Destructure pattern at line 1982: `Ok(JrError::InsufficientScope { message }) => ...` → `Ok(JrError::InsufficientScope { message, .. }) => ...` — compile-break (E0027) without this fix when `required_scope: Option<String>` is added (F1d adversary-pass-04 L-02). (2) Construction site at line 1983 gains `required_scope: Some("write:servicedesk-request".to_string())`. |
| `src/error.rs` unit test T-1b (line 131) | MODIFIED | `insufficient_scope_exit_code`: construction call updated to add `required_scope: None`; assertion (exit_code == 2) UNCHANGED |
| `src/error.rs` unit test T-1 (line 171) | MODIFIED | `insufficient_scope_display_includes_workarounds`: construction call updated to add `required_scope: None`; assertion text UNCHANGED (None-fallback preserves `write:jira-work` literal in Display) |
| `tests/api_client.rs` T-2 (line 136) | UNCHANGED | Assertion passes unmodified via None-fallback at C-2; `write:jira-work` literal preserved byte-for-byte |
| `src/error.rs` unit test (NEW, AC-4) | NEW | `test_insufficient_scope_display_empty_some_falls_back`: constructs with `required_scope: Some("".into())`; asserts Display contains `write:jira-work` (fallback). Pins Empty-Some policy from BC-1.6.042 pass-02. No T-N label assigned — T-3 through T-9 are reserved for the harmonized scheme per impact-boundary.md Section 4 pass-05. |
| `BC-1.6.042` | MODIFY (option-a, in-place) | Behavior line parameterized + Empty-Some policy added; no new BC added; no BC-INDEX or CANONICAL-COUNTS change |
| `BC-1.6.047` (candidate) | WITHDRAWN | PO decision: not needed |

### Construction-Site Lookup Table (locked, with symbol anchors)

Per-construction-site `required_scope` values confirmed against Atlassian OAuth scopes docs (Jira platform + JSM) in design-validation.md Q-2. Values are locked for #382. Additional endpoints (Jira platform write by path, JSM read, Agile, CMDB) are OUT OF SCOPE for #382 — see Sub-table B below.

| Site | Path | Symbol anchor | `required_scope` Value |
|------|------|---------------|------------------------|
| `src/api/client.rs:700` | (any) — blanket-401 early-exit in `send()` first-401-body block | `send()` | `None` |
| `src/api/client.rs:969` | (any) — `parse_error()` helper | `parse_error()` | `None` |
| `src/cli/issue/create.rs:1983` | `POST /rest/servicedeskapi/request` — JSM re-wrap | JSM create re-wrap | `Some("write:servicedesk-request")` |

`None` at C-1 and C-2 is correct: these are endpoint-agnostic paths; conservative fallback to `"write:jira-work"` is the right behavior and preserves test pins T-1, T-1b, and T-2. Path-based endpoint inference in `parse_error()` (which has access to `response.url().path()`) is deferred — fragile, maintenance-heavy, and the None-fallback is sufficient. Q-4 closed; see Open Questions below.

### Future Endpoint Reference (Sub-table B — OUT OF SCOPE for #382)

| Endpoint family | Path prefix | Likely scope | Notes |
|-----------------|-------------|--------------|-------|
| Jira platform write | `/rest/api/3/` POST/PUT | `write:jira-work` | Already falls back to this via `None` at C-1/C-2 |
| JSM read | `/rest/servicedeskapi/` GET | `read:jira-work` + `read:servicedesk-request` | See issue #384 |
| Agile | `/rest/agile/1.0/` | `read:jira-work` / `write:jira-work` | Future PR |
| CMDB | `/jsm/assets/` | `read:cmdb-object` / `write:cmdb-object` | Future PR |

Future call-sites wanting endpoint-specific hints can apply the per-construction-site re-wrap pattern (match arm on `JrError::InsufficientScope`) without modifying the central client — C-3 is the established precedent.

### Validation Refinements (from design-validation.md)

Three refinements established by the research-agent validation gate (L-288-pr4-06) before F2 proceeds:

**Refinement 1 — thiserror idiom (Q-1):** Use the expression-argument form, NOT naive Option interpolation. Naive `{required_scope:?}` renders `Some("x")` / `None` literals to end-users. Correct template (incorporating pass-02 Empty-Some policy):

```rust
#[error(
    "Insufficient token scope: {message}\n\n\
     The Atlassian API gateway rejects granular-scoped personal tokens on POST \
     requests (while PUT/GET succeed). Workarounds:\n  \
     • Use a classic token with \"{scope_hint}\" scope instead of granular scopes, or\n  \
     • Try OAuth 2.0 (run \"jr auth login --oauth\") — may avoid this bug, not verified\n\n\
     See https://github.com/Zious11/jira-cli/issues/185 for details.",
    scope_hint = required_scope.as_deref().filter(|s| !s.is_empty()).unwrap_or("write:jira-work")
)]
InsufficientScope {
    message: String,
    required_scope: Option<String>,
},
```

Note the `.filter(|s| !s.is_empty())` between `as_deref()` and `unwrap_or`. This is required by BC-1.6.042's Empty-Some policy (added pass-02): `Some("")` must fall back to `"write:jira-work"` identically to `None`. The parenthetical `(while PUT/GET succeed)` is load-bearing context — preserved verbatim from the existing Display block, byte-for-byte, per the design-validation.md "byte-for-byte preserved" claim (M-02 finding). In-project precedent: `JrError::NotAuthenticated { hint: String }` (same structured-hint-field pattern). External precedent: gh CLI #9117 desired-pattern (runtime-resolved scope name + actionable recovery command).

**Refinement 2 — scope-name lookup table (Q-2):** Per-construction-site values confirmed against Atlassian OAuth scopes docs. Note: `parse_error()` in `client.rs:969` has access to `response.url().path()` but we do NOT thread endpoint inference here — path-based mapping is fragile and maintenance-heavy. `None`-fallback preserves existing behavior cheaply. Additional endpoints deferred.

**Refinement 3 — narrowed test-change scope (Q-5):** Exactly 2 test construction-call updates needed in `src/error.rs` (lines 131 + 171), NOT three. The architect's F1 impact-boundary entry for `tests/api_client.rs:100` (T-2) was corrected in pass-02: under the `None`→`"write:jira-work"` fallback design, T-2 passes unmodified. The assertion at `tests/api_client.rs:136` is satisfied byte-for-byte by the None-fallback. No assertion update needed at any site.

---

## Acceptance Criteria

| AC | Description | Status |
|----|-------------|--------|
| AC-1 | Variant signature `JrError::InsufficientScope { message: String, required_scope: Option<String> }` | Required for F4 |
| AC-2 | Display uses thiserror expression-argument form `scope_hint = required_scope.as_deref().filter(|s| !s.is_empty()).unwrap_or("write:jira-work")` | Required for F4 |
| AC-3 | New unit test `test_insufficient_scope_display_uses_required_scope_when_some` constructs with `required_scope: Some("write:servicedesk-request".into())`, asserts Display contains `write:servicedesk-request` AND does NOT contain `write:jira-work` (negation pins that the Some-branch suppresses the None-fallback) | Required for F4 |
| AC-4 | New unit test pins `Some("")` → Display contains `write:jira-work` (fallback, per Empty-Some policy from BC-1.6.042 pass-02) | Required for F4 — NEW from pass-02 |
| AC-5 | T-2 (`tests/api_client.rs:136`) still passes unmodified — `None`→`"write:jira-work"` fallback preserves assertion byte-for-byte | Verified by design |
| AC-6 | All 3 production construction sites updated per lookup table: `client.rs:700` → `None`, `client.rs:969` → `None`, `create.rs:1983` → `Some("write:servicedesk-request")` | Required for F4 |
| AC-7 | Destructure pattern at `src/cli/issue/create.rs:1982` updated from `Ok(JrError::InsufficientScope { message }) => ...` to `Ok(JrError::InsufficientScope { message, .. }) => ...` to remain compile-compatible (E0027 prevention) after struct-widening. Subsumes the line-1982 portion of AC-6's create.rs entry. | Required for F4 — NEW from pass-04 L-02 |

### Known Cosmetic — Accepted for #382

**C-3 dual-rendering:** Post-refactor, the JSM C-3 path (`src/cli/issue/create.rs:1983`) renders the scope name twice in the error Display output: once via the C-3-enriched `message` string already present in the `message` field (which the existing code appends at call-site), and once via the new `scope_hint` workaround line generated from `required_scope: Some("write:servicedesk-request")`. This duplication is cosmetically suboptimal but functionally harmless — the scope name is reinforced rather than contradicted, and the user receives actionable information in either occurrence.

**Decision:** Accept for #382. Removing the C-3 enrichment from the `message` field is a separate refactor with its own AC surface and is OUT OF SCOPE for this issue. If user feedback flags the duplication in stderr, file a follow-up issue. See affected-artifacts.md Section 6 for full rationale (L-01 cosmetic accept note).

---

## Files Changed

### New Files

| File Path | Purpose |
|-----------|---------|
| _(none required)_ | All changes are in-place modifications |
| New unit tests (inline in `src/error.rs`) | `test_insufficient_scope_display_uses_required_scope_when_some` — constructs with `required_scope: Some("write:servicedesk-request".into())`, asserts Display contains `write:servicedesk-request` AND does NOT contain `write:jira-work` (two-part; AC-3); `test_insufficient_scope_display_empty_some_falls_back` — pins `Some("")` → `write:jira-work` fallback (AC-4, Empty-Some policy) |

### Modified Files

| File Path | Change Type | Risk |
|-----------|-------------|------|
| `src/error.rs` | Variant struct-widening + `#[error]` template update with `.filter(|s| !s.is_empty())` + 2 construction call updates (T-1b at line 131 + T-1 at line 171) | LOW (compile-time exhaustive-match catches all missed sites) |
| `src/api/client.rs` | Two construction sites add `required_scope: None` (lines 700, 969) | LOW (additive back-compat field) |
| `src/cli/issue/create.rs` | One construction site adds `required_scope: Some("write:servicedesk-request".to_string())` (line 1983) | LOW |
| `.factory/specs/prd/bc-1-auth-identity.md` | BC-1.6.042 Behavior line parameterized in-place + Empty-Some policy added (option a; see `po-decision-bc-parameterization.md` and pass-02 BC-1.6.042 change note) | LOW |

### Dependent Files (unchanged; depend on modified files)

| File Path | Depends On | Regression Risk |
|-----------|-----------|----------------|
| `tests/api_client.rs` | `src/error.rs` (via Display output match) | LOW — T-2 passes unmodified (None-fallback); T-7, T-8, T-9 are negation/non-literal tests |
| `tests/oauth_flow_holdouts.rs` | `src/error.rs` (dispatch, not Display literal) | LOW — T-3/T-4/T-5 pin `"Insufficient token scope"` prefix only, no `write:jira-work` pin |
| `tests/issue_create_jsm.rs` | `src/cli/issue/create.rs` (call-site enriched message) | LOW — T-6 pins `write:servicedesk-request` injected at C-3; still passes with `Some(...)` |
| `src/lib.rs` | `src/error.rs` (re-export) | NONE — no import path changes |

---

## Files NOT Changed (Regression Baseline)

These files must not be modified during implementation. All their tests must continue to pass.

- `src/api/auth.rs` — `DEFAULT_OAUTH_SCOPES` constant is not changing; scope list is unaffected
- `src/cli/auth/` (all files) — auth flow not changing
- `src/cli/issue/list.rs`, `view.rs`, `workflow.rs` — unrelated command handlers
- `src/cli/assets.rs`, `board.rs`, `sprint.rs`, `worklog.rs` — unrelated
- `src/cache.rs`, `src/config.rs`, `src/jql.rs`, `src/adf.rs`, `src/duration.rs` — unrelated
- `tests/oauth_flow_holdouts.rs` — dispatch assertions only; no Display literal pins that would break
- `tests/issue_create_jsm.rs` — satisfied by call-site injection at C-3; no change needed
- `tests/bulk_*.rs`, `tests/search_*.rs`, `tests/migration_*.rs` — unrelated
- `.factory/specs/prd/bc-3-issue-write.md` — BC-3.8.009 satisfied at call-site; no modification needed
- `.factory/specs/prd/cross-cutting.md` — BC-X.3.005 dispatch logic unchanged
- `src/error.rs:129-136` (exit_code test) — wildcard `JrError::InsufficientScope { .. } => 2` arm; no change

### Docs/Index Surfaces Verified Unchanged

These spec and doc files reference `InsufficientScope` behavior or BC-1.6.042. They require no edits under option (a) parameterization but must be verified after implementation confirms accuracy. 8 surfaces enumerated (matching impact-boundary.md Section 5b and affected-artifacts.md Section 8 Docs/Index count; 2 rows added in pass-03 M-03; sibling count now genuinely matches at 8 — architect added the 2 frozen superpowers docs to Section 5b in pass-03, confirmed in pass-04 L-01 propagation).

| File | Reference | Why Unchanged | Verify Action |
|------|-----------|---------------|---------------|
| `CLAUDE.md` (Gotchas section) | No test-seam env-var or hidden behavior introduced | No new `JR_*` env-var; no architectural edge case; no dispatch behavior change | Confirm no `JR_*` or behavioral gotcha introduced during F4 |
| `.factory/specs/prd/BC-INDEX.md` (line 122) | Source cell cites `tests/api_client.rs:99-144` | BC count stable (57); BC-1.6.042 ID and title unchanged; T-2 passes unmodified via None-fallback | Confirm `tests/api_client.rs:99-144` citation still resolves correctly post-F4 |
| `.factory/specs/prd/CANONICAL-COUNTS.md` | BC cumulative count (57 in bc-1) | No new BC added; BC-1.6.047 candidate withdrawn | Confirm count unchanged post-F4 |
| `.factory/specs/prd/edge-case-catalog.md` (line 78) | `Covered by BC-1.6.042; holdout H-012` | BC-1.6.042 still covers this edge case under parameterization | Confirm edge-case description aligns with updated BC-1.6.042 Behavior line |
| `.factory/specs/prd/holdout-scenarios.md` (lines 138–145, H-012) | `write:jira-work` substring assertion | None-fallback preserves `write:jira-work` in Display; H-012 passes unmodified | Run H-012 in validation; confirm `write:jira-work` present in None-path Display |
| `.factory/specs/prd/holdout-scenarios.md` (lines 658–682, H-NEW-JSM-RT-003) | `write:servicedesk-request` in stderr | Satisfied by call-site injection at C-3; `Some("write:servicedesk-request")` reinforces this | Run H-NEW-JSM-RT-003; confirm `write:servicedesk-request` present |
| `docs/superpowers/specs/2026-04-17-insufficient-scope-error-design.md` | Historical v1 design record; stale `{ message: String }` references (lines 23, 63, 90) reflect v1 variant signature, not post-#382 | Not a living doc; deliberately frozen at 2026-04-17 implementation state. Stale references are intentional — post-#382 variant shape is documented in the live spec, not here. | None — frozen record; no verify action required |
| `docs/superpowers/plans/2026-04-17-insufficient-scope-error.md` | Historical v1 plan record; stale `{ message: String }` references (lines 24, 47, 57, 111, 193, 416, 448) reflect v1 variant signature, not post-#382 | Not a living doc; deliberately frozen at 2026-04-17 implementation state. Same rationale as spec above. | None — frozen record; no verify action required |

---

## Risk Assessment

| Risk Type | Level | Rationale |
|-----------|-------|-----------|
| Regression | LOW | `None` fallback preserves all existing Display text for platform-write 401 paths. Only T-1b and T-1 unit tests need construction-call updates (adding a field, assertions unchanged). T-2 (integration test) passes unmodified. Rust exhaustive match catches missed construction sites at compile time. M-2 compile-break risk (E0027 at `src/cli/issue/create.rs:1982`) is **mitigated** by AC-7: changing `{ message }` → `{ message, .. }` in the destructure pattern before any other construction site is touched. |
| Architecture | ZERO | `error.rs` is pure-core (no I/O, no side effects). No module boundaries change. No new dependencies. Variant field widening with back-compat `None` path. |
| Security | ZERO | No auth flow change. No secret handling. No trust boundary change. The scope name in Display is a user-facing hint, not a token or credential. |
| Performance | ZERO | `Option<String>` allocation only on `InsufficientScope` error paths (cold path; no performance impact). |

---

## Regression Baseline

- **Tests in regression risk zone (asserting on InsufficientScope Display or dispatch):** 10 total (T-1, T-1b, T-2 through T-9)
- **Tests requiring source change:** 2 (T-1b at `src/error.rs:131` — construction-call adds `required_scope: None`; assertion (exit_code == 2) UNCHANGED. T-1 at `src/error.rs:171` — construction-call adds `required_scope: None`; assertion text UNCHANGED.)
- **Tests unaffected despite Display change:** 8 (T-2 through T-9 — all pass via `None` fallback or pin non-literal assertions)
- **T-2 specifically:** `tests/api_client.rs:136` — UNCHANGED; assertion passes via None-fallback at C-2; `write:jira-work` literal preserved byte-for-byte
- **New unit tests required:** 2 (AC-3 — `test_insufficient_scope_display_uses_required_scope_when_some`: two-part assertion — Display contains `write:servicedesk-request` AND does NOT contain `write:jira-work`; AC-4 — `test_insufficient_scope_display_empty_some_falls_back` pins `Some("")` → `write:jira-work` fallback per BC-1.6.042 Empty-Some policy)
- **MODIFIED construction-call sites:** 2 (T-1 + T-1b; both in `src/error.rs`; assertions in both are UNCHANGED)
- **Risk zone test files:** `src/error.rs` (inline), `tests/api_client.rs`, `tests/oauth_flow_holdouts.rs`, `tests/issue_create_jsm.rs`

---

## Scope Recommendation

- **Mode:** Quick-dev route (TRIVIAL scope)
- **F2:** Modify BC-1.6.042 in-place (option a; already done by PO, Empty-Some policy added in pass-02); no new BC. Spec version: PATCH bump.
- **F3:** ONE story, ~2 story points. File: `S-X.YY-error-scope-refactor.md`.
- **F4:** Per-story delivery (worktree → stubs → failing tests → TDD → adversary 3/3 CLEAN → demos LOCAL ONLY → push → pr-manager 9-step).
- **F5/F6/F7:** Single-story scope — per-story adversarial review likely sufficient; F6 mutation testing in PR-scope CI.
- **Can parallelize:** No — single story, single author.

---

## Open Questions

All questions resolved. Status recorded below.

**Q-1: Approve thiserror expression-argument idiom?**
**Decision: APPROVED by human.** Expression-arg form `scope_hint = required_scope.as_deref().filter(|s| !s.is_empty()).unwrap_or("write:jira-work")` — includes Empty-Some defensive filter per pass-02 BC-1.6.042 policy. NOT naive `{required_scope:?}`. Cite `NotAuthenticated { hint: String }` as in-project precedent.

**Q-2: Approve Option (a) structured-field design?**
**Decision: APPROVED by human.** `required_scope: Option<String>` field on `InsufficientScope`. Scope-name lookup table locked per Sub-table A above. Confirmed against Atlassian OAuth scopes docs (classic scope names match `DEFAULT_OAUTH_SCOPES` in `src/api/auth.rs`).

**Q-3: Approve scope-name lookup table?**
**Decision: APPROVED by human.** `None` for C-1 and C-2 platform-write paths; `Some("write:servicedesk-request")` for C-3 JSM path. Confirmed against Atlassian OAuth scopes docs (developer.atlassian.com).

**Q-4: Any other endpoints to surface `Some(...)` for now?**
**Decision: Minimal — 3 sites only; future endpoints deferred to incremental PRs.** `parse_error()` has access to `response.url().path()` but path-based endpoint inference is fragile and maintenance-heavy. The `None`-fallback preserves existing behavior cheaply. If a fourth scope surface (e.g., JSM read per issue #384, Confluence write, or Agile) arises, the per-construction-site re-wrap pattern (match arm on `JrError::InsufficientScope`) can be applied without modifying the central client. Additional endpoint reference documented in Sub-table B. No further scope expansion for #382.

---

## Change Log

- [REVISED 2026-05-19 per F1d adversary-pass-05 — 2 MED (M-01 T-N harmonized, M-02 Display preserved verbatim) + 3 LOW (L-01 AC-3 tightened with negation, L-02 AC-3→AC-4, L-03 Section 1 T-1b row) addressed; status remains under-review until pass-06+ CLEAN.]
  - **M-01 (T-N harmonization):** Sources frontmatter updated to cite pass-05 M-01 + L-03 for impact-boundary.md; pass-05 M-02 + L-02 for design-validation.md. Component Impact Table: "T-3 (NEW, AC-4)" label on the Empty-Some row renamed to "(NEW, AC-4)" with a note that no T-N label is assigned to this row — T-3 through T-9 are reserved for the harmonized scheme per impact-boundary.md Section 4 pass-05. Change-log pass-03 entry updated in kind.
  - **M-02 (Display byte-for-byte):** Refinement 1 thiserror template updated to include `(while PUT/GET succeed)` parenthetical, matching the design-validation.md Q-1/Refinement-1 template restored in pass-05. Sentence added to the Note paragraph explicitly citing the M-02 finding and "byte-for-byte preserved" claim. The omission of the parenthetical in prior passes was a silent drop; the claim is now genuinely true.
  - **L-01 (AC-3 tightened with negation):** AC-3 in Acceptance Criteria table updated to specify the two-part assertion: asserts Display contains `write:servicedesk-request` AND does NOT contain `write:jira-work`. New Files table description for `test_insufficient_scope_display_uses_required_scope_when_some` updated to match. Regression Baseline AC-3 description updated to "two-part assertion." This tightening aligns with design-validation.md Q-5 Refinement 3 recommended assertion form.
  - **L-02 (AC-3→AC-4 stale citations):** Not applicable in delta-analysis.md (those citations were in design-validation.md); no additional changes needed here beyond the M-02/L-01 propagation above.
  - **L-03 (Section 1 T-1b row):** Already present in delta-analysis.md Component Impact Table (line 84 — T-1b row was added in pass-03/04). No further change needed.
  - **Frontmatter preserved:** `status: under-review` and `scope: trivial` unchanged.
- [REVISED 2026-05-19 per F1d adversary-pass-04 L-01 + L-02 propagation] — M-2 reclassified MODIFIED; Section 5b sibling count now genuinely matches at 8.
  - **L-02 (M-2 destructure fix):** Component Impact Table row for `src/cli/issue/create.rs` updated to document two distinct modifications: (1) destructure pattern at line 1982 changed from `{ message }` to `{ message, .. }` to prevent E0027 compile-break after struct-widening; (2) construction site at line 1983 gains `required_scope: Some("write:servicedesk-request".to_string())` (unchanged from prior analysis). AC-7 added to pin the destructure fix as a separate acceptance criterion. Risk Assessment updated: M-2 compile-break risk is now mitigated by AC-7 (fix must be applied before other construction sites are touched). Previous DEPENDENT classification of line 1982 was incorrect — exhaustive struct destructure without `..` becomes a hard compile error (E0027) the moment the struct gains any new field.
  - **L-01 (Section 5b count):** Docs/Index Surfaces Verified Unchanged prose updated to confirm that the "8 surfaces" count now genuinely matches impact-boundary.md Section 5b. The architect added the 2 frozen superpowers docs to Section 5b in pass-03; this pass verifies the cross-artifact count is consistent.
  - **Sources frontmatter:** impact-boundary.md source entry updated to cite pass-04 L-02 revision.
  - **Frontmatter preserved:** `status: under-review` and `scope: trivial` unchanged.
- [REVISED 2026-05-19 per F1d adversary-pass-03 — all 3 MED + 3 LOW findings addressed; status remains under-review until pass-04+ CLEAN]
  - **Docs/Index count updated 6 → 8.** Two historical superpowers docs added to "Docs/Index Surfaces Verified Unchanged": `docs/superpowers/specs/2026-04-17-insufficient-scope-error-design.md` and `docs/superpowers/plans/2026-04-17-insufficient-scope-error.md`. Both are frozen v1 records; stale `{ message: String }` references are intentional; no verify action required. Propagated from affected-artifacts.md Section 8 (M-03 finding).
  - **Component Impact Table corrected.** T-1 and T-1b row notes tightened to state construction-call-only updates; assertions explicitly noted as UNCHANGED. T-2 row notes updated to cite `write:jira-work` preserved byte-for-byte. NEW AC-4 row added for `test_insufficient_scope_display_empty_some_falls_back` pinning Empty-Some policy (no T-N label in this row — T-3 through T-9 are reserved for harmonized scheme per impact-boundary.md Section 4).
  - **Known cosmetic subsection added.** "Known Cosmetic — Accepted for #382" documents C-3 dual-rendering wart (scope name appears twice in Display for JSM path). Decision: accept for #382; follow-up if user feedback flags. Cites affected-artifacts.md Section 6 for rationale (L-01 intent).
  - **Regression Baseline counts updated.** NEW unit tests = 2 (AC-3 + AC-4); MODIFIED construction-call sites = 2 (T-1 + T-1b); UNCHANGED assertions = confirmed at all sites including T-2.
  - **Sources frontmatter updated** with pass-03 revision entries for impact-boundary.md, affected-artifacts.md, and design-validation.md.
  - **AC list verified complete:** AC-1 (variant signature), AC-2 (Display thiserror template with `.filter(|s| !s.is_empty())`), AC-3 (test `test_insufficient_scope_display_uses_required_scope_when_some`), AC-4 (test `test_insufficient_scope_display_empty_some_falls_back`), AC-5 (T-2 passes unmodified), AC-6 (all 3 production sites per lookup table). All ACs confirmed present and correctly worded per CLAUDE.md `test_<verb>_<subject>_<expected_outcome>` naming convention.
  - **PG-01 sibling-propagation note:** Pass-02 remediation fixed several sibling artifacts in isolation but missed propagating consistent T-1/T-2 row-level corrections across all sibling cells (cells in impact-boundary.md were corrected in pass-03 M-02 sweep). This is a known process gap in the F1d adversarial cycle: sibling-cell propagation relies on each agent re-reading the full artifact set at each pass. Pass-03 caught the drift. Acknowledged as partial-propagation regression class per S-7.01 discipline; mitigated by the explicit sweep step now codified in pass-03 M-02 methodology.
- [REVISED 2026-05-19 per F1d adversary-pass-02 remediation — all 8 findings addressed; status remains under-review until pass-03 CLEAN]
  - **Scope reclassified STANDARD → TRIVIAL (quick-dev route).** Rationale: single semantic concept (one BC modification with parameterization), 3 production + 2 test construction-call sites (small, bounded surface), no architecture change, no new external deps, LOW regression risk (None-fallback preserves byte-for-byte behavior). "Scope Recommendation" section updated from "Feature Mode / Full F1-F7 (STANDARD scope)" to "Quick-dev route (TRIVIAL scope)".
  - **AC list expanded.** AC-4 added: new unit test pins `Some("")` → `write:jira-work` fallback (Empty-Some policy from BC-1.6.042 pass-02). AC-2 updated to include `.filter(|s| !s.is_empty())` in the thiserror expression. New unit test count updated from 1 to 2 in Regression Baseline.
  - **Empty-Some policy subsection added** in BC Decision. Cites BC-1.6.042 Empty-Some policy (added in pass-02): construction sites MUST NOT pass `Some("")`; Display impl treats it as `None` via `.filter(|s| !s.is_empty())` between `as_deref()` and `unwrap_or`. AC-4 gates this during F4.
  - **thiserror template updated** in Refinement 1 to include `.filter(|s| !s.is_empty())` — consistent with pass-02 BC-1.6.042 change note and design-validation.md Q-1 final template.
  - **Construction-site lookup table reformatted** with symbol anchors (`:700` annotated as `send()` first-401-body block; `:969` annotated as `parse_error()` helper) for forward-stability against line-number drift.
  - **Sub-table B (future endpoints)** added inline citing design-validation.md Q-2 reference table; issue #384 cited for JSM-read case.
  - **Test construction-call site label corrected:** T-1 is `insufficient_scope_display_includes_workarounds` (line 171); T-1b is `insufficient_scope_exit_code` (line 131). Labels were previously swapped in the Component Impact Table. Corrected to match impact-boundary.md Section 4 labeling.
  - **Docs/Index Surfaces Verified Unchanged count confirmed:** 6 rows, matching impact-boundary.md Section 5b and affected-artifacts.md Section 8.
  - **Open Questions all closed:** Q-4 now reads "Minimal — 3 sites only; future endpoints in Sub-table B reference, deferred to incremental PRs." All four questions marked APPROVED or DECIDED.
  - **Sources frontmatter updated** to include pass-02 revision entries for all four sibling artifacts.
- [REVISED 2026-05-19 per F1d adversary-pass-01 — all 7 findings addressed]
  - F-01 (architect): `src/error.rs:131` added as 2nd test construction site. Regression baseline updated to 2 test construction-call updates needed (lines 131 + 171), not 1. Risk Assessment row updated to reflect corrected count.
  - F-02 (PO): BC-1.6.042 decision finalized as option (a) parameterize in-place. BC-1.6.047 candidate withdrawn. BC-INDEX and CANONICAL-COUNTS confirmed unchanged. Component Impact Table updated. `po-decision-bc-parameterization.md` added as source.
  - F-03 (architect): "Docs/Index Surfaces Verified Unchanged" subsection added to "Files NOT Changed" — enumerates `CLAUDE.md`, `BC-INDEX.md`, `CANONICAL-COUNTS.md`, `edge-case-catalog.md:78`, `holdout-scenarios.md:138-145` (H-012), `holdout-scenarios.md:658-682` (H-NEW-JSM-RT-003).
  - F-04 (business-analyst): `edge-case-catalog.md:78` added to "Docs/Index Surfaces Verified Unchanged" table with verify action.
  - F-05 (research-agent): design-validation.md Refinement 3 updated to cite 2 test construction sites; Q-2 rephrased to be accurate about `parse_error()` endpoint-access capability vs. deferral decision.
  - F-06 (business-analyst): `BC-INDEX.md:122` Source cell added to "Docs/Index Surfaces Verified Unchanged" — Source citation remains accurate under option (a).
  - F-07 (business-analyst): `CLAUDE.md` Gotchas section added to "Docs/Index Surfaces Verified Unchanged" — no new test-seam env-var or hidden behavior introduced.
  - Open Questions: Q-1, Q-2, Q-3 marked APPROVED by human. Q-4 collapsed to "Minimal (3 sites only); additional endpoints deferred to incremental PRs."

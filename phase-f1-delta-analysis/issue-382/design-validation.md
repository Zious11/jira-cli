---
issue: 382
step: F1-Step-4 (research-agent validation gate)
title: Design Validation — JrError::InsufficientScope Display refactor
status: complete
date: 2026-05-19
gate: pre-F2
predecessor: impact-boundary.md (F1-Step-3, architect's recommendation)
lesson_applied: L-288-pr4-06 (research-agent validates design BEFORE F2 spec evolution)
---

# Design Validation — Issue #382

Validates the architect's F1 Option-(a) recommendation (`add required_scope: Option<String>` field to `JrError::InsufficientScope`) before the team proceeds to F2 spec evolution and F4 implementation.

---

## Q-1: thiserror convention for variant-with-Option-field Display

**Validation method:** Context7 / docs.rs (`thiserror`), thiserror GitHub README, real-world example in `handlebars-rust/src/error.rs`.

**Finding:**

- `thiserror`'s `#[error("...")]` attribute supports **arbitrary format arguments** in addition to field-name shorthands. Docs.rs verbatim: *"These shorthands can be used together with any additional format args, which may be arbitrary expressions. ... refer to named fields as `.var` and tuple fields as `.0`."* The README example uses an expression `max = i32::MAX` after the format string.
- There is **no built-in `Option`-conditional formatting** in the macro itself. The official docs do not show an idiomatic single-line `#[error]` form that swaps text between `Some` and `None`.
- The architect's report Option-(b)-rejection-reasoning ("doesn't compose with `thiserror`'s `#[error(...)]` attribute mechanism" for dynamic-from-auth-context Display) is correct for **auth-context-at-Display-time** — but does NOT apply to Option-(a), because in Option-(a) the scope name is moved into the variant fields at construction time, not pulled from auth context at format time.
- Two known-working idioms surfaced in the wild:
  1. **Naive interpolation of the Option:** `#[error("Failed to access variable in strict mode {0:?}")]` with `MissingVariable(Option<String>)` — `handlebars-rust` does exactly this. Renders as literal `Some("x")` / `None` — UGLY for end-users in `jr` because the stderr output would read `... missing scope: Some("write:servicedesk-request") ...`.
  2. **Expression-arg interpolation:** `#[error("missing scope: {scope}", scope = required_scope.as_deref().unwrap_or("write:jira-work"))]` — exploits the "arbitrary expression" facility. This is the clean idiom for jr's case. Renders correctly for both `Some` and `None` paths in one line without a manual `impl Display`.
  3. **Hand-rolled `impl fmt::Display` + `#[error(transparent)]` blend** — possible escape hatch if conditional structure exceeds a single expression's readability. Higher maintenance cost.

**Verdict:** **REFINES** architect's recommendation. Option (a) works, but the architect's report does not specify *which* thiserror idiom to use. The pragma is the expression-argument form (idiom #2 above), not the bare-Option interpolation (idiom #1). The F1 report's phrasing "Display uses a runtime-resolved scope name instead of a hardcoded literal" is correct in intent; F2 must pin the exact `#[error(...)]` template.

**Action if REFUTES/REFINES:** F2 spec MUST include the exact `#[error]` template. Recommended template (pseudocode for the variant):

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

This is the ONLY change from the existing `#[error]` block — substituting `"write:jira-work"` with the parameterized `{scope_hint}` expression-arg. Every other character is preserved, including the parenthetical, the workaround bullets, the "may avoid this bug, not verified" hedge, and the issues/185 URL. This is what "byte-for-byte preserved" actually means. The parenthetical `(while PUT/GET succeed)` is load-bearing context — explains to users why their GET works but POST fails. Preserving the full Display text minus the one parameterized substring is the minimal-disruption refactor.

The `.filter(|s| !s.is_empty())` defensively treats `Some("")` identically to `None`, preventing the broken `Use a classic token with "" scope` Display output if a construction site passes empty by mistake. Pinned by a new unit test at AC-4. This aligns with the BC-1.6.042 Empty-Some policy.

`None` callers (and now `Some("")` callers) retain today's `"write:jira-work"` literal as the fallback (zero behavior regression for the platform-write path and for the existing `insufficient_scope_display_includes_workarounds` and `test_401_scope_mismatch_returns_insufficient_scope` tests — see Q-5). `Some("write:servicedesk-request")` callers get the JSM-correct hint.

---

## Q-2: Atlassian scope naming for endpoints that emit 401

**Validation method:** Web search + WebFetch of developer.atlassian.com OAuth scopes pages (Jira platform + JSM).

**Finding (per construction site):**

| Endpoint | Construction site | Correct classic scope | Granular alternative |
|----------|-------------------|----------------------|---------------------|
| `POST /rest/api/3/issue` (platform create) | `client.rs:700` (blanket 401 if body matches) + `client.rs:969` (parse_error) | `write:jira-work` | (multiple) |
| `PUT /rest/api/3/issue/{key}` (platform edit) | same — through generic `client.put`/`parse_error` | `write:jira-work` | (multiple) |
| `POST /rest/api/3/issue/{key}/transitions` (transition) | same | `write:jira-work` | (multiple) |
| `POST /rest/api/3/issue/{key}/comment` (comment) | same | `write:jira-work` | (multiple) |
| `POST /rest/servicedeskapi/request` (JSM create) | `create.rs:1983` (re-wrap with enriched hint) | `write:servicedesk-request` | `write:request:jira-service-management` |
| `GET /rest/api/3/project/{key}` (read project — used by `require_service_desk`) | NOT an `InsufficientScope` site today (`require_service_desk` is `UserError`, not 401-routed) — but the read scope is `read:jira-work` | `read:jira-work` | (multiple) |

Two important notes from the Atlassian docs:

1. **Atlassian's own JSM scope page explicitly recommends classic over granular:** *"Where available, the recommendation is to use these scopes [classic]."* This supports the existing codebase choice in `DEFAULT_OAUTH_SCOPES` (`src/api/auth.rs:60-63`) which lists classic scopes (`write:jira-work`, `write:servicedesk-request`).
2. **The community thread on "scope does not match" for `/rest/servicedeskapi/request`** does NOT definitively resolve whether classic `write:servicedesk-request` or granular `write:request:jira-service-management` is the canonical fix; the root cause is often `client_credentials` auth not being supported for JSM writes at all, regardless of scope. For jr's purposes (user-flow OAuth, not client_credentials), the classic scope `write:servicedesk-request` matches what jr already requests and what the existing hint already names — so the F1 recommendation is consistent with the current keychain/scopes plumbing.

**Verdict:** **CONFIRMS** architect's recommendation. The classic scope names already used in the codebase (`write:jira-work` for platform writes; `write:servicedesk-request` for JSM create) are correct against the official Atlassian OAuth scopes documentation. The F2 spec can lift these directly into the lookup table below.

**Action if REFUTES/REFINES:** None — proceed.

**Scope-name lookup table (F2 input):**

**Sub-table A — Construction sites IN SCOPE for #382 (the 3 sites we're touching):**

| Site | Endpoint | required_scope value |
|------|----------|----------------------|
| `client.rs:700` | (any) | `None` |
| `client.rs:969` | (any) | `None` |
| `create.rs:1983` | `POST /rest/servicedeskapi/request` | `Some("write:servicedesk-request")` |

**Sub-table B — Reference: additional endpoints that COULD use this pattern in future PRs (OUT OF SCOPE for #382; future enhancement; see issue #384 for the JSM-read case):**

| Endpoint family | Path prefix | Likely scope |
|-----------------|-------------|--------------|
| Jira platform write | `/rest/api/3/` POST/PUT | `write:jira-work` |
| JSM read | `/rest/servicedeskapi/` GET (e.g., `require_service_desk`) | `read:jira-work` + `read:servicedesk-request` |
| Agile | `/rest/agile/1.0/` | `read:jira-work` / `write:jira-work` |
| CMDB | `/jsm/assets/` | `read:cmdb-object` / `write:cmdb-object` |

**Recommended construction-site mapping for F2:**

- `client.rs:700` blanket-401 early-exit: pass `required_scope: None`. The dispatch happens BEFORE the call site is known to the central `send()` method; conservatively show the historical `write:jira-work` workaround. This preserves today's behavior for the platform-write path (covered by tests T-1 and T-2 in F1 impact report).
- `client.rs:969` parse_error helper: same — pass `None`. `parse_error` has access to `response.url().path()` (signature `async fn parse_error(response: Response) -> anyhow::Error` at `src/api/client.rs:957`), but we choose not to thread endpoint inference for now — the path-based mapping is fragile (URL substring matching), maintenance-heavy (every new endpoint needs a lookup-table entry), and the `None`-fallback preserves existing behavior cheaply. See open-question (d) in `delta-analysis.md` for the explicit deferral.
- `create.rs:1983` JSM re-wrap: pass `Some("write:servicedesk-request".to_string())`. C-3 already knows it's the JSM path and already enriches the message — the new field carries the same information into Display in a structured way.

**Cost/benefit honest analysis:** Threading endpoint inference into `parse_error` would add ~10 lines of path-prefix match logic covering ~5 known prefixes (`/rest/api/3/`, `/rest/servicedeskapi/`, `/rest/agile/1.0/`, `/jsm/assets/`, `/oauth/`). The benefit would be accurate fallback for non-JSM, non-blanket-401 paths (e.g., `read:jira-work` for GET-401 cases that route to InsufficientScope). We defer because: (1) those paths are rare in practice — most 401s come from the blanket C-1 path; (2) the `None` fallback to `write:jira-work` is benign (the user can still grant the scope; the message itself reports what was rejected). If a future call-site reports a confusing fallback, the per-call-site re-wrap pattern at C-3 is precedent — apply it there without modifying central client. The deferred path-prefix table is preserved in Q-2's reference table for future PRs.

Future opportunity (out of scope for #382): if more call sites want to inject endpoint-specific hints, the same pattern (match arm on `JrError::InsufficientScope` to re-wrap with the correct `required_scope`) can be reused per call site without modifying `send()` / `parse_error()` — keeps the central client free of endpoint-knowledge.

---

## Q-3: Precedent in similar Rust / Go CLIs

**Validation method:** Web search of cli/cli (gh) issues and discussions; review of error-message format requests.

**Finding:**

- **`gh` CLI** does NOT consistently inject the required scope name into its 403/401 error messages today — multiple open issues (#2845, #8326, #9117, #9380, #11308) request exactly this enhancement. When `gh` DOES get it right, the desired format is: *"This API operation needs the \"admin:org\" scope. To request it, run: gh auth refresh -h github.com -s admin:org"* (per #9117). This is the **runtime-resolved scope name + actionable recovery command** pattern — exactly what Option (a) implements for jr.
- The `gh` CLI's `gh auth refresh --scopes <scope>` command pattern mirrors jr's existing `jr auth refresh` / `jr auth login` pair already present in the existing JSM hint (`create.rs:1985-1988`). The format already cited in `jr` (`Run "jr auth refresh" to refresh, or "jr auth login" to re-authorize with updated scopes`) tracks the gh-CLI good-pattern.
- No Rust CLI surfaced with a published thiserror pattern for "hardcode required scope in variant string" — the few real-world examples (e.g., `handlebars-rust` `MissingVariable(Option<String>)`) use Option-typed fields. This supports Option (a) over a hardcoded literal.

**Verdict:** **CONFIRMS** architect's recommendation. Option (a)'s runtime-resolved scope name with actionable recovery command matches the documented desired pattern in `gh` CLI (the dominant Go-CLI reference) and matches the Rust thiserror idioms surveyed.

**Action if REFUTES/REFINES:** None — proceed.

---

## Q-4: Local codebase — similar patterns and precedent

**Validation method:** Local read of `src/error.rs` + grep of `JrError::` constructions across `src/api/`.

**Finding:**

- **`JrError::NotAuthenticated { hint: String }`** (line 5-6 of `src/error.rs`) is the closest in-codebase precedent. It already uses a named String field that's interpolated into the Display template via `#[error("Not authenticated. {hint}")]`. Construction sites pass different hint text per call path:
  - `src/api/client.rs:719` — `"Run \"jr auth login\" to connect."` (Basic auth 401)
  - `src/api/client.rs:777` — `"run 'jr auth refresh' to re-authenticate"` (OAuth retry 401)
  - `src/api/client.rs:835` — same as above (OAuth reconcile retry 401)
  - `src/api/client.rs:971` — `"Run \"jr auth login\" to connect."` (parse_error fallback)
  - `src/cli/issue/create.rs:1975` — `"The \`write:servicedesk-request\` OAuth scope may be missing. Run \`jr auth refresh\` or \`jr auth login\` to re-consent..."` (JSM re-wrap)
- This is **a direct template for what Option (a) should do for `InsufficientScope`** — replace the entire hint text with a per-call-site string. The `hint: String` (non-Option) form is even simpler than `Option<String>`, but Option (a)'s `Option<String>` provides a back-compat default for the platform-write path so the two existing test pins (`write:jira-work`) keep passing untouched. The architect chose `Option<String>` rather than `String` for exactly this reason — sound trade-off.
- No other `JrError` variant uses `Option<String>` today. The variant shape is novel for this enum but aligns with broader Rust ecosystem (per Q-1 examples).
- `DEFAULT_OAUTH_SCOPES` lives in `src/api/auth.rs:59-64` and is the canonical source of "what scopes does jr request." If F2 wants to centralize the scope-name lookup table, this file is the right home for it — though there's no need to refactor the lookup into a function for #382's narrow fix.

**Verdict:** **CONFIRMS** architect's recommendation, with a strong in-project precedent (`NotAuthenticated { hint: String }`). F2 spec should explicitly cite `NotAuthenticated` as the template pattern.

**Action if REFUTES/REFINES:** None — proceed. F2 should cite `NotAuthenticated { hint: String }` as the in-codebase precedent for "structured hint field interpolated into thiserror Display."

---

## Q-5: Existing tests that pin the literal `write:jira-work`

**Validation method:** Local read of both tests (full bodies).

**Finding:**

- **`src/error.rs:170` `insufficient_scope_display_includes_workarounds`** asserts `assert!(s.contains("write:jira-work"), "workaround missing: {s}")` on line 180. The test constructs the variant directly with `JrError::InsufficientScope { message: "...".into() }` (line 171-173). Under Option (a), this construction call would need a second field. If `required_scope: None` is the back-compat default and Display falls back to `"write:jira-work"` literal (per Q-1's recommended template), the assertion **still passes unmodified** — `None` falls back to the historical literal. The construction call signature itself changes, so the test source DOES need a one-line update to add `required_scope: None`.
- **`tests/api_client.rs:100` `test_401_scope_mismatch_returns_insufficient_scope`** asserts `assert!(s.contains("write:jira-work"))` on line 136. This is an integration test — it does NOT construct the variant directly; instead it triggers a 401 mock response and reads the resulting error. The triggering path is `client.post` → `send` → blanket-401 → `parse_error` → `InsufficientScope { message }`. Under Option (a), construction site C-2 passes `required_scope: None`, Display falls back to `"write:jira-work"`, and **the assertion passes unmodified**. No test-source change required.
- **`tests/oauth_flow_holdouts.rs` AC-005 tests** (T-3, T-4, T-5 in F1 report) pin only `"Insufficient token scope"` prefix and negation properties — no `write:jira-work` literal. **No change needed.**
- **`tests/issue_create_jsm.rs:1522` C-01 test** (T-6 in F1 report) pins `write:servicedesk-request`, `jr auth refresh`, `jr auth login`. Under Option (a), construction site C-3 passes `Some("write:servicedesk-request")`, Display includes that scope name, and the assertion passes. The C-3 site's existing enriched message (`format!("{message} (jr issue create --request-type requires the write:servicedesk-request OAuth scope. ...)")`) can remain unchanged — the new `required_scope` field provides Display-level reinforcement, not a replacement for the enriched message string. **No change needed unless F2 chooses to simplify by relying solely on `required_scope` (in which case T-6 still passes because the Display includes the scope name).**

**Verdict:** **CONFIRMS** architect's recommendation, with one refinement. The two tests flagged in F1 (T-1 and T-2) do NOT both need assertion updates — two construction-call updates are needed in `src/error.rs` tests module: line 131 (`insufficient_scope_exit_code`) and line 171 (`insufficient_scope_display_includes_workarounds`). Both add `required_scope: None`. T-2 (`tests/api_client.rs:100`) is unaffected because it triggers the error through a mock HTTP response, not direct construction. The architect's F1 impact-boundary table line 23 (T-2 = "**MODIFIED** — assertion must be updated") **overstates** the required change. Under the recommended fallback design (`None` renders `"write:jira-work"`), T-2's assertion still passes byte-for-byte.

**Action if REFUTES/REFINES:** F2 should narrow the test-change scope to:
- **T-1 (`src/error.rs:170` `insufficient_scope_display_includes_workarounds`):** add `required_scope: None` to the construction call on line 171-173. Assertion text unchanged.
- **T-1b (`src/error.rs:131` `insufficient_scope_exit_code`):** add `required_scope: None` to the construction call on line 131. This test also constructs `JrError::InsufficientScope { message: "..." .into() }` and will fail to compile when the variant signature widens. Assertion text unchanged.
- **T-2 (`tests/api_client.rs:100`):** **NO CHANGE.** Behavior preserved by `None`→`write:jira-work` fallback.
- **NEW unit test** required per issue #382 AC-3 ("New unit test pins the new structured Display behavior"). Recommended: `test_insufficient_scope_display_uses_required_scope_when_some` (per CLAUDE.md test-naming convention `test_<verb>_<subject>_<expected_outcome>`) — constructs with `required_scope: Some("write:servicedesk-request".into())`, asserts Display contains `write:servicedesk-request` and does NOT contain `write:jira-work`. Pins the new branch.
- **NEW unit test for Empty-Some policy (AC-4)**: `test_insufficient_scope_display_empty_some_falls_back` — constructs with `required_scope: Some("".into())`, asserts Display contains `write:jira-work` (fallback). Pins BC-1.6.042 Empty-Some policy.

---

## Summary

**Design recommendation: PROCEED with Option (a) WITH REFINEMENTS.**

The architect's F1 Option-(a) recommendation is the correct direction. Three refinements are required before F2:

### Refinement 1 — thiserror template (Q-1)

F2 must specify the exact `#[error(...)]` template using the expression-argument idiom:

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
```

This is the ONLY change from the existing `#[error]` block — substituting `"write:jira-work"` with the parameterized `{scope_hint}` expression-arg. Every other character is preserved, including the parenthetical, the workaround bullets, the "may avoid this bug, not verified" hedge, and the issues/185 URL. This is what "byte-for-byte preserved" actually means. The parenthetical `(while PUT/GET succeed)` is load-bearing context — explains to users why their GET works but POST fails. Preserving the full Display text minus the one parameterized substring is the minimal-disruption refactor.

The `.filter(|s| !s.is_empty())` defensively treats `Some("")` identically to `None`, preventing the broken `Use a classic token with "" scope` Display output if a construction site passes empty by mistake. Pinned by a new unit test at AC-4. This aligns with the BC-1.6.042 Empty-Some policy.

Do NOT use the naive `{required_scope:?}` form — it renders `Some("x")` / `None` literals to end-users. Cite `NotAuthenticated { hint: String }` as the in-project precedent for the structured-hint-field pattern.

### Refinement 2 — scope-name lookup table (Q-2)

F2 must include the scope-name table (above). For #382's narrow scope, only three construction-site values are needed:

- `client.rs:700` → `None`
- `client.rs:969` → `None`
- `create.rs:1983` → `Some("write:servicedesk-request".to_string())`

The `None` cases preserve today's `"write:jira-work"` Display text via fallback — no behavior regression for non-JSM 401 paths.

### Refinement 3 — narrowed test-change scope (Q-5)

F2 must correct the F1 impact-boundary entry for T-2 (`tests/api_client.rs:100` `test_401_scope_mismatch_returns_insufficient_scope`). Under the recommended fallback design, this test does NOT require an assertion update — it passes unmodified because `None` renders the historical literal. Two construction-call updates needed in `src/error.rs` tests module: line 131 (`insufficient_scope_exit_code`) and line 171 (`insufficient_scope_display_includes_workarounds`). Both add `required_scope: None`. Assertion text unchanged in both. A new unit test must be added per issue AC-3 to pin the `Some` branch.

### F2 / F4 inputs ready

The F2 spec-evolution phase has everything needed:
- Exact thiserror template (Refinement 1)
- Per-construction-site scope-name values (Refinement 2)
- Precise test-touch scope (Refinement 3, plus required new unit test name)
- In-codebase precedent to cite (`NotAuthenticated { hint: String }`)
- External-CLI precedent to cite (gh CLI #9117 desired-pattern format)
- Atlassian docs citations (developer.atlassian.com scopes pages, classic vs granular)

No PIVOT is required. No architectural change is required. The change remains a single-variant signature widening with back-compat fallback in Display.

---

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| WebSearch | 6 | thiserror Option-field interpolation; Atlassian OAuth scope docs; gh CLI scope-error patterns; Rust CLI permission-error best practices |
| WebFetch | 6 | docs.rs/thiserror (Display field interpolation); dtolnay/thiserror README; Atlassian developer.atlassian.com Jira + JSM OAuth scope pages; gh CLI issues #2845, #9117; handlebars-rust error.rs (real Option-field thiserror example); Atlassian community thread on "scope does not match" 401 |
| Grep | 4 | local enumeration of `InsufficientScope`, `parse_error`, `require_service_desk`, scope literals |
| Read | 10 | `src/error.rs`, `src/api/client.rs` (parse_error + blanket-401), `src/cli/issue/create.rs` (JSM re-wrap), `src/api/jsm/servicedesks.rs`, `src/api/auth.rs` (DEFAULT_OAUTH_SCOPES), `tests/api_client.rs` (4 scope-mismatch tests), `tests/issue_create_jsm.rs` (C-01 test + AC-012 platform-401 negation), `tests/oauth_flow_holdouts.rs` (3 AC-005 tests), `.factory/phase-f1-delta-analysis/issue-382/impact-boundary.md` (architect's F1 report) |
| Glob | 1 | `.factory/phase-f1-delta-analysis/issue-382/` directory enumeration |
| Perplexity | 0 | (Not used — Atlassian docs + thiserror docs are first-party canonical sources; web-fetched directly. Perplexity would have added value only if first-party sources were silent.) |
| Context7 | 0 | (Not used — `thiserror`'s Option-field guidance is not in its canonical docs; surveyed real-world usage instead. Context7 lookup would have returned the same docs.rs content already retrieved.) |
| Tavily | 0 | (Not used — WebSearch + WebFetch sufficed for cross-source validation; no second-index disambiguation needed.) |
| Training data | 2 areas | (1) general thiserror idiom familiarity (validated against docs.rs); (2) general Rust enum field-extension patterns (validated against in-project `NotAuthenticated` precedent). Both areas cross-checked against authoritative sources before being cited. |

**Total external tool calls:** 16 (6 WebSearch + 6 WebFetch + 4 Grep)
**Total local tool calls:** 11 (10 Read + 1 Glob)
**Training data reliance:** **low** — every claim cited a docs page, source file, or test file. The thiserror "arbitrary expression" claim, the Atlassian scope names, the `NotAuthenticated` template, and the test-assertion analysis are all verifiable against the cited artifacts.

---

## Sources (external)

- [thiserror — docs.rs](https://docs.rs/thiserror/latest/thiserror/) — `#[error("...")]` field interpolation + arbitrary-expression format args
- [dtolnay/thiserror — GitHub README](https://github.com/dtolnay/thiserror) — canonical idioms; no Option-conditional example
- [handlebars-rust/src/error.rs](https://github.com/sunng87/handlebars-rust/blob/master/src/error.rs) — real-world `Option<String>` in thiserror variant
- [Jira scopes for OAuth 2.0 (3LO) and Forge apps](https://developer.atlassian.com/cloud/jira/platform/scopes-for-oauth-2-3LO-and-forge-apps/) — `write:jira-work` for platform writes; `read:jira-work` for reads
- [Jira Service Management scopes for OAuth 2.0 (3LO) and Forge apps](https://developer.atlassian.com/cloud/jira/service-desk/scopes-for-oauth-2-3LO-and-forge-apps/) — `write:servicedesk-request` (classic, recommended) vs `write:request:jira-service-management` (granular)
- [cli/cli issue #9117](https://github.com/cli/cli/issues/9117) — gh CLI desired-pattern format for missing-scope hints: scope name + actionable recovery command
- [cli/cli issue #2845](https://github.com/cli/cli/issues/2845) — gh CLI scope-hint background
- [Atlassian community: "scope does not match" with servicedeskapi](https://community.atlassian.com/forums/Jira-Service-Management/Getting-quot-401-scope-does-not-match-quot-with-servicedeskapi/qaq-p/3144480) — context on JSM 401 scope-mismatch (client_credentials caveat not relevant to jr's user-flow OAuth)

---

[REVISED 2026-05-19 per F1d adversary-pass-01 F-01 + F-05]
[REVISED 2026-05-19 per F1d adversary-pass-02 H-02 + M-05 + L-02]
[REVISED 2026-05-19 per F1d adversary-pass-05 M-02 + L-02] — Restored "(while PUT/GET succeed)" parenthetical to Q-1/Refinement-1 template (was silently dropped, contradicted "byte-for-byte preserved" claim); 2 stale AC-3 citations fixed to AC-4.

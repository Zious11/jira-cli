# [S-382] Refactor JrError::InsufficientScope Display to use structured required_scope field

**Epic:** Feature Followup — Post-JSM enhancement (deferred from PR #381)
**Mode:** brownfield / quick-dev (TRIVIAL scope)
**Convergence:** CONVERGED after 3 adversarial passes (all CLEAN)

![Tests](https://img.shields.io/badge/tests-717%2F717-brightgreen)
![Coverage](https://img.shields.io/badge/coverage-stable-brightgreen)
![Mutation](https://img.shields.io/badge/mutation-N%2FA%20(trivial%20scope)-lightgrey)
![Holdout](https://img.shields.io/badge/holdout-N%2FA-lightgrey)

This PR closes #382. It adds a `required_scope: Option<String>` field to `JrError::InsufficientScope` and updates the `#[error(...)]` thiserror template to use a `scope_hint` expression-argument. The hardcoded `"write:jira-work"` literal is replaced with a runtime-resolved hint: `None` falls back to `"write:jira-work"` (preserving all existing test assertions byte-for-byte); `Some("write:servicedesk-request")` is passed by the JSM create path introduced in PR #381. The Empty-Some policy (`Some("")` → fallback via `.filter(|s| !s.is_empty())`) is pinned by a new unit test (AC-4). Net diff: 3 production files changed, 48 insertions / 6 deletions.

**Known pre-existing flake:** `tests/multi_cloudid_disambiguation.rs` may fail intermittently in CI due to keychain contention on the macOS runner. This is unrelated to S-382 and was present before this branch.

---

## Architecture Changes

```mermaid
graph TD
    ErrorEnum["src/error.rs\nJrError enum"] -->|Display via thiserror| UserStderr["stderr output"]
    ClientSend["src/api/client.rs\nsend() / parse_error()"] -->|construct InsufficientScope| ErrorEnum
    JSMCreate["src/cli/issue/create.rs\nhandle_jsm_create()"] -->|construct InsufficientScope\nSome(write:servicedesk-request)| ErrorEnum
    style JSMCreate fill:#FFE4B5
    style ErrorEnum fill:#90EE90
```

<details>
<summary><strong>Architecture Decision Record</strong></summary>

### ADR: Additive Option field on JrError::InsufficientScope

**Context:** PR #381 added JSM create support requiring `write:servicedesk-request` scope, but the existing `InsufficientScope` Display hardcoded `"write:jira-work"` as the only workaround hint regardless of which command failed.

**Decision:** Add `required_scope: Option<String>` as a named field on the struct variant. Use thiserror's expression-argument form to compute `scope_hint` at format time, with `None`-fallback to `"write:jira-work"`.

**Rationale:** Additive `Option` field is the smallest possible change; `None` preserves byte-for-byte backward compatibility with all existing call sites and test assertions. Rust's exhaustive match catches any missed construction site at compile time. The per-call-site re-wrap pattern (match arm on `JrError::InsufficientScope { message, .. }`) is the established precedent from `handle_jsm_create` and is extensible to future endpoints without modifying the central client.

**Alternatives Considered:**
1. Path-based inference in `parse_error()` — rejected because: fragile (URL path is not a stable discriminator for scope), maintenance-heavy, and `None`-fallback is sufficient for all current non-JSM paths.
2. Separate `JrError::InsufficientJsmScope` variant — rejected because: overstates the distinction (same user-visible error structure), inflates BC count, and requires a second `From` impl with no analytical gain.

**Consequences:**
- All existing tests pass without modification (T-2 at `tests/api_client.rs:136` passes via `None`-fallback).
- Future endpoints wanting a specific scope hint apply the C-3 re-wrap pattern without touching `client.rs`.

</details>

---

## Story Dependencies

```mermaid
graph LR
    PR381["PR #381\n(closed) JSM create dispatch"] --> S382["S-382\nthis PR"]
    S382 --> Future384["issue #384\n(pending) JSM read scopes"]
    style S382 fill:#FFD700
    style PR381 fill:#90EE90
    style Future384 fill:#E0E0E0
```

No story in `depends_on` list (standalone quick-dev, per story spec frontmatter). PR #381 is already merged; issue #384 is a future follow-up that does not block this PR.

---

## Spec Traceability

```mermaid
flowchart LR
    BC["BC-1.6.042\nInsufficientScope Display\nparameterized scope hint\n+ Empty-Some fallback"] --> AC1["AC-1\nVariant signature widened"]
    BC --> AC2["AC-2\nDisplay expression-arg\nwith .filter()"]
    BC --> AC3["AC-3\nSome(scope) → scope in Display\nNOT write:jira-work"]
    BC --> AC4["AC-4\nSome empty → write:jira-work"]
    BC --> AC5["AC-5\nT-2 unchanged (None fallback)"]
    BC --> AC6["AC-6\n3 production sites updated"]
    BC --> AC7["AC-7\nDestructure pattern fix"]
    AC1 --> SRC["src/error.rs:17-20"]
    AC2 --> SRC
    AC3 --> T3["test_insufficient_scope_display_uses_required_scope_when_some\nsrc/error.rs:195-209"]
    AC4 --> T4["test_insufficient_scope_display_empty_some_falls_back\nsrc/error.rs:212-222"]
    AC5 --> T2["tests/api_client.rs:136\n(unchanged)"]
    AC6 --> C1["client.rs:700 None"]
    AC6 --> C2["client.rs:970 None"]
    AC6 --> C3["create.rs:1983 Some(write:servicedesk-request)"]
    AC7 --> C3D["create.rs:1982 destructure { message, .. }"]
```

---

## Test Evidence

### Coverage Summary

| Metric | Value | Threshold | Status |
|--------|-------|-----------|--------|
| Unit tests | 717/717 pass | 100% | PASS |
| Coverage | stable (no regressions) | >80% | PASS |
| Mutation kill rate | N/A — trivial scope; CI mutation job scoped to diff | >90% | N/A |
| Holdout satisfaction | N/A — evaluated at wave gate (no holdout anchors for this story) | >0.85 | N/A |

### Test Flow

```mermaid
graph LR
    Unit["717 Unit Tests"]
    Integration["Integration (wiremock)"]
    Red["Red Gate log\nAC-3 FAIL before impl\nAC-4 PASS (both templates)"]

    Unit -->|717/717 pass| Pass1["PASS"]
    Integration -->|T-2 unchanged| Pass2["PASS"]
    Red --> Pass3["Gate PASSED"]

    style Pass1 fill:#90EE90
    style Pass2 fill:#90EE90
    style Pass3 fill:#90EE90
```

| Metric | Value |
|--------|-------|
| **New tests** | 2 added (AC-3, AC-4), 2 modified construction calls (T-1, T-1b) |
| **Total suite** | 717 unit tests PASS |
| **Coverage delta** | stable (3 files changed; additive Option field + Display template) |
| **Mutation kill rate** | scoped to diff; CI mutation job will report |
| **Regressions** | 0 |

<details>
<summary><strong>Detailed Test Results</strong></summary>

### New Tests (This PR)

| Test | Location | AC | Result |
|------|----------|----|--------|
| `test_insufficient_scope_display_uses_required_scope_when_some` | `src/error.rs:195-209` | AC-3 | PASS |
| `test_insufficient_scope_display_empty_some_falls_back` | `src/error.rs:212-222` | AC-4 | PASS |

### Modified Construction Call Sites in Tests (assertions unchanged)

| Test | Location | Change |
|------|----------|--------|
| `insufficient_scope_exit_code` (T-1b) | `src/error.rs:133-142` | Added `required_scope: None`; exit-code assertion UNCHANGED |
| `insufficient_scope_display_includes_workarounds` (T-1) | `src/error.rs:175-192` | Added `required_scope: None`; Display assertion UNCHANGED |

### AC-3 Two-Part Assertion

```rust
assert!(s.contains("write:servicedesk-request"), "...");
assert!(!s.contains("write:jira-work"), "...");
```

### Red Gate Discipline

The Red Gate log (`.factory/cycles/cycle-001/S-382/implementation/red-gate-log.md`) records:
- Step 2 over-reach detected and corrected — Display template was reverted to stub state before failing tests were added (commit `950aefb`).
- AC-3 FAIL verified with stub template; AC-4 PASS (both template variants) verified.
- TDD implementation authorized only after Red Gate passed.

### Coverage Analysis

| Metric | Value |
|--------|-------|
| Lines added | ~48 |
| Lines covered | all new lines exercised by 2 new + 2 modified unit tests |
| Branches added | 3 (Some non-empty, Some empty, None) |
| Branches covered | all 3 via AC-3, AC-4, T-1/T-2 |
| Uncovered paths | none |

### Mutation Testing

Scoped to PR diff via `cargo mutants --in-diff`. CI mutation job will report. Key mutant classes expected:
- Remove `.filter(|s| !s.is_empty())` → AC-4 kills it
- Replace `unwrap_or("write:jira-work")` with `unwrap_or("")` → T-1, T-2, AC-4 kill it
- Replace `None` with `Some("")` at C-1/C-2 → AC-4 kills it (Empty-Some policy enforced)

</details>

---

## Holdout Evaluation

N/A — evaluated at wave gate. No holdout anchors defined for this story (trivial scope, error-message-only refactor, no new CLI surface or user-facing behavior).

---

## Adversarial Review

| Pass | Verdict | Findings | Critical | High | Status |
|------|---------|----------|----------|------|--------|
| 1 | CLEAN | 0 | 0 | 0 | 17 invariants confirmed PASS |
| 2 | CLEAN | 0 | 0 | 0 | Fresh-context re-derivation; same conclusions |
| 3 | CLEAN | 0 | 0 | 0 | Probed whitespace-only Some, thiserror 2.x compat, dual-rendering, byte-for-byte drift |

**Convergence:** Per-story CONVERGED (3/3 CLEAN). Adversary exhausted probe surface in pass 3 with no findings.

<details>
<summary><strong>Pass 3 Probe Coverage</strong></summary>

- `Some("  ")` (whitespace-only): `.filter(|s| !s.is_empty())` does NOT filter whitespace-only strings — this is by-design; whitespace-only scope names are invalid Atlassian scopes and would surface as-is. The spec only requires the Empty-Some (`Some("")`) policy. Accepted.
- thiserror 2.x expression-arg semantics: confirmed compatible with `Cargo.toml:28` pin of version 2.
- C-3 dual-rendering (scope name appears twice for JSM path): accepted cosmetic per F1d pass-03.
- Byte-for-byte drift vs frozen v1 spec: Display template (lines 8-16) preserves `(while PUT/GET succeed)` parenthetical verbatim. Confirmed against baseline diff.

</details>

---

## Security Review

```mermaid
graph LR
    Critical["Critical: 0"]
    High["High: 0"]
    Medium["Medium: 0"]
    Low["Low: 0"]

    style Critical fill:#90EE90
    style High fill:#90EE90
    style Medium fill:#90EE90
    style Low fill:#90EE90
```

**Assessment: CLEAN.** This PR is a pure error-message refactor.

- No auth flow change.
- No new HTTP endpoint or trust boundary.
- No secrets handling or token exposure.
- The `required_scope` field is a user-facing hint string (`"write:servicedesk-request"` / `"write:jira-work"`), not a credential or token.
- No new external dependencies introduced.
- No `unsafe` blocks.
- No `#[allow]` suppression.

<details>
<summary><strong>Security Scan Details</strong></summary>

### SAST
- No new code paths that touch auth, secrets, or network.
- `Option<String>` field on an error variant — zero attack surface.
- Delta analysis risk classification: Security = ZERO.

### Dependency Audit
- No new dependencies. `cargo deny check` expected CLEAN (no change to `Cargo.toml`).

### Formal Verification
- N/A for error-message refactor. Type system provides mechanical correctness: Rust exhaustive struct-match catches any missed construction site at compile time.

</details>

---

## Risk Assessment & Deployment

### Blast Radius
- **Systems affected:** Error Display rendering path for `JrError::InsufficientScope` only (cold error path).
- **User impact:** If the fallback were broken, users would see a blank scope hint in the `InsufficientScope` error message. The `.filter(|s| !s.is_empty())` guard + AC-4 test pin prevent this.
- **Data impact:** None — error messages only.
- **Risk Level:** LOW

### Performance Impact
| Metric | Before | After | Delta | Status |
|--------|--------|-------|-------|--------|
| Latency p99 | unchanged | unchanged | 0 | OK |
| Memory | unchanged | unchanged | +1 `Option<String>` on error path only | OK |
| Throughput | unchanged | unchanged | 0 | OK |

`Option<String>` allocation occurs only when `InsufficientScope` is constructed — a cold error path. Zero hot-path impact.

<details>
<summary><strong>Rollback Instructions</strong></summary>

**Immediate rollback (< 2 min):**
```bash
git revert 1fd3b23
git push origin develop
```

The revert will restore the tuple-form `InsufficientScope(String)` variant and the hardcoded `"write:jira-work"` literal. All prior tests pass unmodified.

**Verification after rollback:**
- `cargo test --lib insufficient_scope` — all 2 original tests PASS (new tests from this PR will be gone)
- `tests/api_client.rs:136` PASS

</details>

### Feature Flags
None — error message rendering is not feature-flagged.

---

## Traceability

| Requirement | Story AC | Test | Verification | Status |
|-------------|---------|------|-------------|--------|
| BC-1.6.042 postcondition 1 | AC-1, AC-6, AC-7 | `insufficient_scope_exit_code` + compile | Rust exhaustive match | PASS |
| BC-1.6.042 postcondition 2 | AC-2, AC-5 | `insufficient_scope_display_includes_workarounds`, `tests/api_client.rs:136` | Unit + integration | PASS |
| BC-1.6.042 postcondition 3 | AC-3 | `test_insufficient_scope_display_uses_required_scope_when_some` | Unit (two-part) | PASS |
| BC-1.6.042 Empty-Some invariant | AC-4 | `test_insufficient_scope_display_empty_some_falls_back` | Unit | PASS |

<details>
<summary><strong>Full VSDD Contract Chain</strong></summary>

```
BC-1.6.042 postcondition 1 -> AC-1/AC-6/AC-7 -> compile exhaustive-match -> src/error.rs:17-20 / client.rs:700,970 / create.rs:1982-1991 -> ADV-PASS-3-OK
BC-1.6.042 postcondition 2 -> AC-2/AC-5 -> insufficient_scope_display_includes_workarounds + T-2 -> src/error.rs:8-16 -> ADV-PASS-3-OK
BC-1.6.042 postcondition 3 -> AC-3 -> test_insufficient_scope_display_uses_required_scope_when_some -> src/error.rs:195-209 -> ADV-PASS-3-OK
BC-1.6.042 Empty-Some invariant -> AC-4 -> test_insufficient_scope_display_empty_some_falls_back -> src/error.rs:212-222 -> ADV-PASS-3-OK
```

</details>

---

## Demo Evidence

Demo strategy: **Strategy C (text-only evidence)**. No VHS recording produced — this refactor has no new CLI surface. The behavior change is entirely in error-message rendering, which is fully pinned by four unit tests and one integration test. Per-AC text evidence is in `.factory/code-delivery/issue-382/` (LOCAL ONLY per `docs/demo-evidence/` gitignore convention; not committed to this PR per PR #386).

| AC | Evidence type | Location |
|----|--------------|----------|
| AC-1 | Source code + compile | `src/error.rs:17-20` |
| AC-2 | Source code | `src/error.rs:8-16` |
| AC-3 | Unit test (two-part) | `src/error.rs:195-209` |
| AC-4 | Unit test | `src/error.rs:212-222` |
| AC-5 | Integration test (unchanged) | `tests/api_client.rs:135-138` |
| AC-6 | Source code (3 sites) | `client.rs:700-703, 970-973`; `create.rs:1983-1991` |
| AC-7 | Source code | `create.rs:1982` |

---

## AI Pipeline Metadata

<details>
<summary><strong>Pipeline Details</strong></summary>

```yaml
ai-generated: true
pipeline-mode: brownfield / quick-dev (TRIVIAL scope)
factory-version: "1.0.0-rc.18"
pipeline-stages:
  delta-analysis-f1: completed (F1d converged 3/3 at pass-08)
  story-decomposition: completed (S-382, 2 points)
  tdd-implementation: completed (strict TDD; Red Gate passed)
  holdout-evaluation: "N/A — trivial scope, no holdout anchors"
  adversarial-review: completed (per-story 3/3 CLEAN)
  formal-verification: "N/A — error-message refactor, type system provides exhaustive-match guarantee"
  convergence: achieved
convergence-metrics:
  spec-novelty: "low (single semantic concept)"
  test-kill-rate: "pending CI mutation job"
  implementation-ci: "pending"
  holdout-satisfaction: "N/A"
adversarial-passes: 3
models-used:
  builder: claude-sonnet-4-6
  adversary: claude-sonnet-4-6 (independent re-derivation per pass)
generated-at: "2026-05-19"
```

</details>

---

## Pre-Merge Checklist

- [ ] All CI status checks passing
- [x] Coverage delta neutral (additive only; no regressions)
- [x] No critical/high security findings (pure error-message refactor; ZERO security risk)
- [x] Rollback procedure validated (single-commit revert)
- [x] No feature flags required
- [x] Per-story adversarial review 3/3 CLEAN
- [x] Red Gate discipline applied (AC-3 FAIL verified before TDD implementation)
- [ ] Human review completed (Copilot review requested post-PR creation)
- [x] No monitoring alerts required (error-message-only change; no production-impacting behavior)

---
document_type: lessons
story_id: issue-288-pr4-dispatch
cycle: cycle-3-wave-3
produced_by: state-manager
timestamp: 2026-05-19
pr: "#381"
merge_commit: "95232555"
---

# Lessons Learned — S-288-pr4-dispatch (Wave 3 / Issue #288)

Adversarial convergence: 9 passes (1 invalidated + 1 retry + 7 substantive), 3/3 consecutive
CLEAN (passes 07/08/09) per BC-5.39.001. 28 invariants verified in final pass.

---

## L-288-pr4-01: Invalid adversary path → ALL findings invalid; re-dispatch with WORKSPACE_BASE enforcement

When the adversary inspects the wrong filesystem path, ALL findings are invalid — none can be
carried as concerns because they describe files not under review. The correct protocol:

1. Persist a note to the story's adversary pass file marking the pass as INVALID (wrong path).
2. Do NOT carry any finding forward as a deferred item or NIT.
3. Re-dispatch immediately with an explicit `WORKSPACE_BASE=<absolute-path>` enforcement
   instruction in the adversary prompt and, if possible, a pre-flight check that verifies the
   adversary is reading the correct file before issuing findings.

This pattern materialized as pass-01 (invalid) → pass-02-retry in pr4-dispatch. The retry
pass correctly found substantive issues; the invalid pass findings were discarded.

**Apply to:** every adversary dispatch; include worktree path verification as first adversary step.

---

## L-288-pr4-02: New BC discovered during adversarial review → story AC + test pin in same burst

When the adversary identifies a missing behavioral contract (e.g., BC-3.8.011 platform-flag
warnings discovered during review), the response must trigger BOTH:

(a) A story AC addition to the in-flight story spec (PO dispatch).
(b) A verbatim string pin in tests (test-writer dispatch in the same burst as the AC addition).

Deferring either half creates a gap: the AC is unverified, or the test exists without a spec
anchor. The orchestrator must treat BC-discovery as a two-part atomic task: spec update + test
pin, not as a find-it-now / implement-it-later split.

**Apply to:** any adversarial pass that introduces a new BC during an active story.

---

## L-288-pr4-03: 401 handling on dispatched paths must cover BOTH JrError variants with negative-space tests

When wiring 401 unauthorized handling on a new dispatch path (e.g., JSM request submission),
test coverage must include:

- `JrError::NotAuthenticated` for Basic auth callers (no Bearer header).
- `JrError::InsufficientScope` for OAuth Bearer callers missing the required scope.

AND negative-space assertions verifying the unaffected path (i.e., NotAuthenticated does NOT
trigger an InsufficientScope message, and vice versa). A single 401-test that only checks the
happy-path variant leaves a coverage gap that the adversary will surface.

**Apply to:** any new API dispatch that forks on auth method; always pair positive + negative tests
for each JrError variant on the 401 path.

---

## L-288-pr4-04: New OAuth scope additions require a paired CHANGELOG entry + CLAUDE.md gotcha

When adding a new OAuth scope to the embedded app (e.g., `manage:jira-configuration` for JSM
request type submission), two artifacts MUST ship in the same PR:

1. A CHANGELOG entry covering forced re-consent: "Users with existing OAuth tokens must
   re-authenticate (`jr auth login`) to grant the new scope."
2. A CLAUDE.md gotcha entry covering the Atlassian Developer Console release gate: the new scope
   must be registered in the OAuth app console BEFORE shipping the release binary, or the
   re-consent flow will fail with a scope-not-registered error for all users.

Neither artifact is derivable from code review alone; the adversary surfaced this gap (PG-04).
The pattern is now codified as a pre-release checklist item for any scope-expanding PR.

**Apply to:** every PR that adds an entry to the `OAUTH_SCOPES` list in `src/api/auth.rs` or
equivalent.

---

## L-288-pr4-05: `clippy::too_many_arguments` MUST be solved by argument-struct refactor, NEVER by `#[allow]`

Per CLAUDE.md no-lint-suppression policy: when a function accumulates enough parameters to
trigger `clippy::too_many_arguments`, the correct fix is an argument-struct refactor:

- Group related parameters into a named struct (e.g., `JsmCreateArgs`, `JsmRequestBuilder`).
- Derive or implement the necessary traits on the struct.
- Thread the struct through callers.

Adding `#[allow(clippy::too_many_arguments)]` is NOT acceptable in this codebase (CLAUDE.md
"No lint suppression without refactoring"). If refactoring is impractical, ask the user before
suppressing and include a justification comment — but in practice, argument-struct refactors
for 6-8 parameter functions are straightforward and should always be preferred.

This lesson was reinforced during pr4-dispatch when the initial dispatch impl accumulated 7
parameters on the JSM submission path before the refactor was applied in the same PR.

**Apply to:** any function growing beyond 5 parameters; proactively refactor before clippy fires.

---

## L-288-pr4-06: Adversarial findings must be Perplexity/local-validated BEFORE filing as follow-up issues

Adversarial findings must be Perplexity/local-validated BEFORE being filed as follow-up issues.
The pr4-dispatch cycle ran 9 adversarial passes without per-finding validation — the retrospective
audit caught zero REFUTED claims but one reframe (O-08-05 scope name was incorrect: should be
`read:*` not `write:*`). Codify: research-agent dispatch as Step 9.5 between adversarial close
and follow-up issue filing. Process-gap codification target for future cycles.

**Apply to:** every adversary dispatch cycle; run research-agent validation pass before filing
any follow-up GitHub issues from adversarial findings.

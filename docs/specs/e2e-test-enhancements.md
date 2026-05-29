# Feature Spec: Live-Jira E2E Test Enhancements

**Status:** Draft
**Author:** jr maintainers (brainstormed 2026-05-29)
**Supersedes:** nothing — extends `docs/specs/e2e-live-jira-testing.md` (the original E2E suite spec)
**Research:** `.factory/research/e2e-enhancement-best-practices.md` (Perplexity-primary, cited)
**Tracking:** TBD (GitHub issue to be filed)

## 1. Overview

The live-Jira E2E suite (`tests/e2e_live.rs`, nightly `.github/workflows/e2e.yml`) shipped in
S-E2E-1/S-E2E-2 (PRs #433/#434). It established excellent *gating* discipline (a pure-function
gate test plus a source meta-guard) but its **assertions are shallow**: most tests assert only
`status.success()` or `is_array()`/`is_object()`, and the one write flow never reads back to
confirm its mutations actually took effect. Several high-value command families and all error
paths have **zero** live coverage.

This spec hardens the suite for **regression safety** while holding a hard line on
**portability**: every test must pass against *any* Jira Cloud instance, not just the one
provisioned for CI. It is organized as one spec with a shared test-helper foundation plus three
milestones (M1–M3). JSM expansion is explicitly deferred to a future spec.

## 2. Goals / Non-Goals

### Goals
- Catch regressions in `jr`'s **JSON output contract** (the AI-agent-facing surface) and in its
  **behavioral invariants** (round-trip read-after-write, idempotency, status-category transitions).
- Add live coverage for untested-but-high-value commands and **error/exit-code paths**.
- Harden the suite against live-API **flakiness** (eventual consistency, rate limits) and bound
  test-data **leakage** from interrupted runs.
- Keep the suite **non-blocking** and **serial** (unchanged posture from the original spec).

### Non-Goals
- **No JSM expansion in this pass.** The two existing JSM read tests (`queue list`,
  `requesttype list`, behind `JR_E2E_JSM_PROJECT`) stay as-is. Deeper JSM coverage
  (`requesttype fields`, `queue view`, `issue create --request-type` JSM-create path) is a
  **separate future spec**.
- **No new `jr issue delete` command.** Teardown remains **close-only** (see §7). Done-issues
  accumulating on the throwaway CI site is accepted (original spec §5).
- **No `reconcileIssues` CLI feature.** The Atlassian vendor read-after-write escape hatch
  (≤50 issue IDs) is a *product feature*, out of scope here. The test-side mitigation is
  `poll_jql` (§4). Recorded as a future candidate (§10).
- **No `issue edit --field` live test.** `--field` depends on a field being on the project's
  *Edit screen*, which is instance-specific (overfit risk). It stays covered by mocked tests
  (`tests/issue_edit_field.rs`); live coverage would have to clean-skip, providing no signal.
- **No exhaustive flag-combination coverage.** Smoke-level happy paths + contract assertions +
  the documented error paths — not a combinatorial matrix.

## 3. Guiding Principle — Portability (no overfitting)

This is the spine of the design. Every assertion pins a **contract or invariant**, never
instance-specific data. The rule, made explicit so reviewers can enforce it:

| Assert (portable) | Never assert (overfit) |
|---|---|
| `statusCategory` is one of the 3 fixed Jira categories (`To Do` / `In Progress` / `Done`; hardcoded green/yellow/blue-gray across all instances) | Workflow status **names** (`In Progress`, `Done`, `Closed`, …) — already env-configurable via `JR_E2E_STATUS_*` |
| `key` matches `^[A-Z][A-Z0-9]+-\d+$`; `id` is a numeric string | A specific key/ID value |
| "**If non-empty, every element conforms**" to the shape | That a list is non-empty (kills the S-398 over-fit class) |
| Required JSON keys are **present** with the right **type/format** | Exact field values from seed data |
| Error cases: process **exit code** + JSON error envelope **shape** | Error **message** substrings (locale/wording-fragile; cf. JRACLOUD-95368 lesson) |
| Custom-field behavior only where the field is guaranteed by the Jira platform | A field that depends on a project's screen/scheme config |

Sources: contract-testing literature (Pact flexible matchers, Fowler), and the repo's own
anti-substring-matching lessons. See research report §3, §6.

## 4. Foundation — shared test helpers (built first)

All new helpers live in `tests/e2e_live.rs` alongside the existing `poll_view`. They are
debug-only test code; no production-code change is required for the foundation.

- **`poll_jql(jql, predicate) -> Value`** — sibling to `poll_view` for **search-path**
  assertions. JQL search is *explicitly not read-after-write consistent* (Atlassian
  Search-and-Reconcile doc; JRACLOUD-97427: lag seconds→minutes→occasionally hours). Therefore:
  - Treats **"0 results" as retryable**, not a failure.
  - Bounded exponential backoff (mirrors `poll_view`'s schedule), conservative wall-clock cap.
  - **On budget exceedance: clean-skip (return/`eprintln!` skip), NOT hard-fail** — an
    unindexed-yet result is an environment signal, not a `jr` regression.
  - Emits elapsed poll time on exit so a CI log reader can tell lag from a real bug.
- **Shape matchers** (pure helpers, type/format only):
  - `assert_key_format(&str)` — `^[A-Z][A-Z0-9]+-\d+$`.
  - `assert_status_category(&Value, expected: &str)` — reads the `statusCategory` (key/name),
    asserts it equals one of the 3 fixed categories. Never touches status name.
  - `assert_issue_shape(&Value)` — `key` (format), `fields`/`summary` present, `status` object
    with a `statusCategory`.
  - `assert_array_of_objects_with_keys(&Value, &[&str])` — "if non-empty, every element has keys".
- **Transient classifier** — a helper that decides retry-vs-fail: retry on 429 / 503 /
  connection-reset / empty-index; **never** retry a 4xx in a positive test (that hides bugs).
- **Poll-budget env seam** — `JR_E2E_POLL_*` (debug-only, `#[cfg(debug_assertions)]`, mirroring
  the existing `JR_BULK_*` seams; add to the CLAUDE.md `JR_*` table in the same commit per the
  codified doc-fallout rule). Lets CI tune the ceiling and local runs use a short budget.

**Verification ordering rule (applies everywhere):** to confirm a write landed, prefer
**direct GET-by-key read-back** (`poll_view`) — it is read-after-write consistent. Use `poll_jql`
**only** for assertions specifically *about search behavior* (e.g. the `issue list --jql` command
itself). Prove the write via GET first; test search visibility second.

## 5. Milestone M1 — Assertion depth on the existing tests

Convert exit-code-only / shape-only checks into **contract + round-trip** assertions. No new
test functions required (mostly), just deeper bodies.

### 5.1 Read tests — element shape
- `issue list` (by project, and the summary-filter variant): when the array is non-empty, assert
  every element has `key` (format) + a `status` object with a `statusCategory`.
- `board list`: if non-empty, each element has `id` + `name` + `type`.
- `sprint list` / `sprint current`: if present, each sprint object has `id` + `state`.
- `user search`: if non-empty, each element has `accountId` + `displayName` (presence/type only).
- `worklog list`: if non-empty, each entry has `timeSpentSeconds` (numeric) + `id`.
- `project fields`: assert **all 5** documented keys present: `project`, `issue_types`,
  `priorities`, `statuses_by_issue_type`, `asset_fields` (currently only 2 are checked).

### 5.2 Write flow — round-trip every mutation
`test_e2e_write_flow_*` becomes a read-back flow:
1. **create** → assert `key` format; then `poll_view` and assert echoed `summary == summary_create`,
   issue type name == `Task`, and the run label is present in `labels`.
2. **edit** (summary) → `poll_view`, assert `summary == summary_edit`; assert the edit command's
   own JSON `changed_fields` shape (honor the #398/#396 echo-asymmetry contracts: human channel
   prints `(updated)` marker, JSON carries raw input — assert each channel **distinctly**).
3. **comment** → `issue comments <key> --output json`, assert the posted body text is present in
   some comment (GET-consistent; no JQL).
4. **worklog add 5m** → `worklog list <key> --output json`, assert an entry with
   `timeSpentSeconds == 300`.
5. **move → In Progress** → `poll_view`, assert `statusCategory` is the In-Progress category
   (**by category, not name**); then **re-issue the same move and assert exit 0** (single-key
   idempotency contract).
6. **move → Done** → `poll_view`, assert `statusCategory` is the Done category.

## 6. Milestone M2 — New regression coverage (portability-safe)

New gated tests (all `#[ignore]` + `e2e_enabled()` guard + run-label + teardown-eligible). Each
self-seeds its own data (no inter-test dependency).

### 6.1 Read / discovery
- `issue transitions <key>` → array of transition objects (shape: `id` + `name` present).
- `issue changelog <key>` → shape (histories array).
- `issue comments <key>` → array shape (standalone, beyond the M1 write-flow read-back).
- `board view <board_id>` → object shape (gated on `JR_E2E_BOARD_ID`, clean-skip otherwise).
- `team list` → array shape (clean-skip if org has no teams / endpoint unavailable — portability).
- `user view <self-accountId>` → object with `accountId` (resolve self via `user search` seed).
- `issue link-types` → array shape (`id` + `name` + `inward` + `outward`).

### 6.2 Write / behavioral contracts
- **assign**: `issue assign <key> --me` → `poll_view`, assert `assignee.accountId` is set;
  round-trip. (Self-assignment is permission-safe on any instance.)
- **link / unlink**: create two issues, `issue link A B <type>` → view A, assert the link to B
  present; `issue unlink` → view A, assert it's gone. Use a link type discovered from
  `link-types` (don't hardcode "Blocks" — pick the first available → portable).
- **edit --dry-run** (+ `--output json`): run against a seeded issue, assert **no mutation**
  (subsequent `poll_view` shows the field unchanged) + the dry-run JSON shape. Fully portable,
  no write.
- **bulk move** (multi-key positional / `--to`): create 2 issues, bulk-move both, assert the
  per-key bulk-result shape and that each transitioned (`poll_view` per key). Pins the
  documented **non-idempotent** bulk contract (distinct from single-key idempotency).
- **pagination dedup**: create 3 issues under one unique label, `issue list --jql "labels=<L>"
  --all --output json`, assert the returned keys are **duplicate-free** and include all 3. Pins
  the JRACLOUD-95368 client-side dedup contract without needing to trigger the upstream bug.
  (Use `poll_jql` — this is a search-path assertion.)

### 6.3 Error / exit-code paths (no mutation)
Assert **mapped exit code** (`src/error.rs` contract) + JSON error envelope shape; **never**
message substrings. The **exact** expected code per case is taken from `src/error.rs` and the
existing mocked error tests (`tests/issue_view_errors.rs`, `tests/issue_list_errors.rs`) — the
live tests must assert the **same** mapping those tests pin, so the live and mocked contracts
cannot drift. (Implementation step: read those tests first and reuse their expected codes; do
not invent a number.)
- **404** — `issue view E2E-99999999 --output json` → assert the not-found exit code from the
  contract above + presence of a JSON error field.
- **400** — `issue list --jql "this is not valid ("` --output json → assert the
  malformed-query exit code from the contract above + presence of a JSON error field.
- **401** — a command run with a deliberately bad `JR_AUTH_HEADER` → exit `2`
  (`JrError::NotAuthenticated`, confirmed in `error.rs`) + `errorMessages` present (assert
  presence, not Atlassian's sentence; honor the documented "no machine-readable `code`,
  no RFC-6750 header" shape).

## 7. Milestone M3 — Robustness & ops

### 7.1 Suite-side
- Adopt **`poll_jql`** for the create-then-search assertion(s) (the current summary-filter list
  test does not retry → latent flake on a cold index).
- **Secret-leak guard test**: run a normal `--output json` command and assert that neither stdout
  nor stderr contains the base64 token or the service-account email. Cheap, high-value, portable.
- **Leak-detection log** at suite start: count pre-existing `e2e-*`-labeled **open** issues and
  `eprintln!` the number (warn-only, never fails) — visible drift signals a broken teardown.
- (Optional, low priority) per-test wall-clock guard mapping a hung call toward exit 124.

### 7.2 CI / ops (`.github/workflows/`)
- **New `e2e-sweeper.yml`** — scheduled daily, non-blocking, `concurrency: jira-e2e` (shares the
  serialization group so it never interleaves with the main run). Closes
  `project=$JR_E2E_PROJECT AND labels ~ "e2e-" AND statusCategory != Done AND created <= -1d`.
  This is the backstop for **hard-cancelled** runs, where GitHub may skip even the `if: always()`
  teardown step (research §4). Close-only (no delete), best-effort (`|| true`), idempotent.
- **`e2e.yml` failure classification**: distinguish a first-call **401** (expired/revoked token →
  "rotate token") from a **connection/site-not-found** error (possible inactivity deactivation →
  "reactivate site within the grace window"). Different remediation, different log message.
  Non-blocking failure signal so an expired token is noticed before the reactivation window closes.
- The existing teardown (close-by-current-run-label) stays; the sweeper covers the cross-run /
  cancelled-run orphan gap that the per-run label cannot.

## 8. Out-of-scope items recorded for the future
- **JSM coverage expansion** (own spec): `requesttype fields`, `queue view`,
  `issue create --request-type` JSM-create path — all behind `JR_E2E_JSM_PROJECT`.
- **`reconcileIssues` CLI support** (≤50 IDs) — the vendor read-after-write escape hatch; a
  stronger create-then-search story than client-side polling. Candidate feature/flag.
- **`jr issue delete`** — would enable delete-based teardown/sweeper; deliberately not added.
- **Token-expiry early-warning** step (~30 days before expiry) — minor; loud 401 is the baseline.

## 9. Testing strategy
- The E2E tests *are* the verification artifact. New always-run unit tests cover any pure helper
  added (e.g. the shape matchers, the transient classifier) — mirroring how `extract_fn_body`
  already has always-run unit tests, so the foundation is verified even when `JR_RUN_E2E` is unset.
- The source meta-guard (`test_every_ignored_test_has_gate_guard`) automatically enforces that
  every new gated test early-returns via `e2e_enabled()` before any live call — no new gate
  plumbing needed; new tests inherit the guarantee.
- Local dry-run: `JR_RUN_E2E=1 JR_E2E_BASE_URL=… JR_AUTH_HEADER=… JR_E2E_PROJECT=E2E cargo test
  --test e2e_live -- --include-ignored --test-threads=1`.
- `ci.yml` is untouched and never passes `--include-ignored`, so the gated suite stays inert there.

## 10. References
- Original suite spec: `docs/specs/e2e-live-jira-testing.md`.
- Research report (Perplexity-primary, cited): `.factory/research/e2e-enhancement-best-practices.md`.
- Atlassian — Search and Reconcile (`reconcileIssues`, "search API doesn't provide
  read-after-write consistency by default", ≤50 IDs):
  <https://developer.atlassian.com/cloud/jira/platform/search-and-reconcile/>
- Atlassian — REST API rate limiting (429, `Retry-After`, `X-RateLimit-*`):
  <https://developer.atlassian.com/cloud/jira/platform/rate-limiting/>
- JRACLOUD-97427 — JQL indexing lag (seconds→minutes→hours):
  <https://jira.atlassian.com/browse/JRACLOUD-97427>
- Pact matching (flexible/type matchers) & Fowler contract testing:
  <https://docs.pact.io/getting_started/matching> ·
  <https://martinfowler.com/bliki/ContractTest.html>
- Repo-internal: `src/error.rs` (exit-code contract); CLAUDE.md notes on JRACLOUD-95368 dedup,
  #398/#396 echo asymmetry, `JR_*` debug-only seams, status-category color mapping.

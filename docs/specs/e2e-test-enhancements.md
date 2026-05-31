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
  - Bounded exponential backoff. To avoid drift, `poll_jql` and the existing `poll_view` SHOULD
    share one backoff schedule derived from `JR_E2E_POLL_MAX_ATTEMPTS` / `JR_E2E_POLL_INITIAL_MS`
    (refactor `poll_view`'s hardcoded `[250,500,1000,2000]` to read the same seam, or document why
    only `poll_jql` is configurable). Conservative wall-clock cap.
  - **Default on budget exceedance: clean-skip (return/`eprintln!` skip), NOT hard-fail** — an
    unindexed-yet result is an environment signal, not a `jr` regression. **Exception:** when the
    caller *created* the issues it is searching for (e.g. the pagination-dedup test §6.2), a
    persistent short-but-nonzero count after full budget is a REGRESSION and must fail loud — the
    skip default applies only to a 0-result (pure-lag) state. Expose this via a `poll_jql` mode
    parameter (skip-on-empty vs fail-on-short).
  - Emits elapsed poll time on exit so a CI log reader can tell lag from a real bug.
- **Shape matchers** (pure helpers, type/format only):
  - `assert_key_format(&str)` — `^[A-Z][A-Z0-9]+-\d+$`.
  - `assert_status_category(&Value, expected)` — asserts against the locale-**invariant**
    `statusCategory.key` (`new` / `indeterminate` / `done`), NOT the localized `name`
    (`To Do` / `In Progress` / `Done`). Take `expected` as an enum {ToDo, InProgress, Done} mapping
    to those stable keys — never a free status-name `&str`. This is what makes the move-transition
    assertions portable across instances and locales.
  - `assert_issue_shape(&Value)` — `key` (format), `fields`/`summary` present, `status` object
    with a `statusCategory`.
  - `assert_array_of_objects_with_keys(&Value, &[&str])` — "if non-empty, every element has keys".
- **Transient classifier** — a helper that decides retry-vs-fail: retry on 429 / 503 /
  connection-reset / empty-index; **never** retry a 4xx in a positive test (that hides bugs).
- **Poll-budget env seam** — two concrete vars: **`JR_E2E_POLL_MAX_ATTEMPTS`** (default mirrors
  `poll_view`'s 5) and **`JR_E2E_POLL_INITIAL_MS`** (default 250). Read via `std::env::var` **in
  test code only** — these govern the test-side poll loop, not the `jr` binary, so unlike
  `JR_BULK_*` they do NOT need a `#[cfg(debug_assertions)]` src/ read site (F1 verdict: zero src/
  change). Still add both names to the CLAUDE.md `JR_*` table in the same commit per the codified
  doc-fallout rule (#335/#357). Lets CI tune the ceiling and local runs use a short budget.

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
- `sprint list --board <id>`: JSON is a **bare array** of sprint objects, each with `id` + `state`
  (`state` is `Option` — assert if present).
- `sprint current --board <id>`: JSON is an **object** `{sprint, issues, sprint_summary?}` — the
  sprint is nested under `["sprint"]` and `issues` is an array of issues, NOT a top-level array of
  sprints (M-1; verified `src/cli/sprint.rs` `json!({"sprint":…, "issues":…})`). Assert
  `v["sprint"]["id"]` / `v["sprint"]["state"]`; if asserting issues, use `assert_issue_shape` on
  `v["issues"][]`.
- Both `sprint` commands **require** `--board` (a flag, from `JR_E2E_BOARD_ID`). **Preserve all
  three existing clean-skip conditions** when deepening: (a) `JR_E2E_BOARD_ID` unset; (b) non-scrum
  board → stderr "only available for scrum boards"; (c) no active sprint → stderr "No active
  sprint". The existing tests already handle (b) and (c) — do not drop them.
- `user search`: if non-empty, each element has `accountId` + `displayName` (presence/type only).
- `worklog list`: if non-empty, each entry's `timeSpentSeconds` **if present is numeric** (the
  field is `Option<u64>` in the `Worklog` type — do NOT require it non-null; F-07). Reserve the
  exact `== 300` value check for the just-written entry only (§5.2 step 4).
- `project fields`: assert **all 5** documented keys are **present** (never non-empty): `project`,
  `issue_types`, `priorities`, `statuses_by_issue_type`, `asset_fields` (currently only 2 are
  checked). **Trap (F-08):** `asset_fields` is `[]` on any non-CMDB instance and `priorities` /
  `statuses_by_issue_type` may be empty — assert key-presence only; do NOT strengthen to non-empty.

### 5.2 Write flow — round-trip every mutation
`test_e2e_write_flow_*` becomes a read-back flow.

> **Create-JSON contract (F-05, CORRECTED):** `issue create --output json` does a follow-up GET
> and emits the **full `Issue` object plus a top-level `url`** (verified `handle_create`,
> `src/cli/issue/create.rs`; corroborated by `docs/specs/issue-create-json-full-shape.md` / #253).
> On GET failure it degrades to `{key, url, fetch_error}`. It does NOT return a bare `{"key": …}`.
> Assert `key` (format/presence) **and `url` presence** on the create output (this full-shape +
> `url` + `fetch_error`-sentinel is itself a regression-worthy contract). The `poll_view` body
> remains the canonical source for field *values*.

1. **create** → assert `key` format **and `url` present**; then `poll_view` and assert echoed
   `summary == summary_create`, the issue type name **equals the value passed to `--type`**
   (read from `JR_E2E_ISSUE_TYPE`, defaulting to `"Task"`; not a hardcoded literal — implemented
   via `issue_type()` helper, S-E2E-3), and the run label is present in
   `labels`.
2. **edit summary** → `poll_view`, assert `summary == summary_edit`; assert the edit command's own
   JSON has `changed_fields` containing `summary` + `updated: true` (`edit_response` shape). NOTE:
   the #398/#396 echo-asymmetry (human prints `description → (updated)` marker vs JSON carrying the
   raw `--description` input) is **description-specific** — it does NOT apply to a `--summary` edit
   (summary echoes its value in both channels). To exercise the asymmetry, add an **edit-description
   sub-step**: `issue edit <key> --description <text> --output json` → assert JSON
   `changed_fields.description == <raw text>` and the human/table channel prints the `(updated)`
   marker — assert each channel **distinctly**. Keep the summary and description assertions separate.
3. **comment** → write with `issue comment <key> <text>`; read back via `issue comments <key>
   --output json` (GET-consistent; no JQL). **ADF caveat (M-comment):** `Comment.body` is an
   **Atlassian Document Format object** (`Option<Value>`), NOT a flat string — do NOT assert
   `body == "<text>"`. Assert the posted text appears as a **substring of the serialized comment
   JSON** (or extract plain text via the ADF→text path); the literal is embedded inside the ADF
   structure.
4. **worklog add 5m** → `worklog list <key> --output json`, assert an entry with
   `timeSpentSeconds == 300`.
5. **move → In Progress** → `poll_view`, assert `statusCategory` is the In-Progress category
   (**by category, not name**); then **re-issue the same move and assert exit 0** (single-key
   idempotency contract). Single-key move JSON is `{key, status, changed}` — the idempotent
   re-issue returns `changed: false` (F-10; distinct from the bulk shape in §6.2).
6. **move → Done** → `poll_view`, assert `statusCategory` is the Done category.

## 6. Milestone M2 — New regression coverage (portability-safe)

New gated tests (all `#[ignore]` + `e2e_enabled()` guard + run-label + teardown-eligible). Each
self-seeds its own data (no inter-test dependency).

### 6.1 Read / discovery
- `issue transitions <key>` → **bare JSON array** of `Transition` objects, each
  `{id, name, to}` where `to` is an **optional** object `{name, statusCategory:{name, key}}`
  (C-2, verified `Transition`/`Status`/`StatusCategory` in `src/types/jira/issue.rs` — there is NO
  `to_category` field anywhere in source). Assert `v.is_array()` and, if non-empty, each element
  has `id` + `name` (both `String`, guaranteed); treat `to` as present-or-absent. For a portable
  category assertion use `element["to"]["statusCategory"]["key"] ∈ {new, indeterminate, done}`.
- `issue changelog <key>` → **object** shape `{key, entries}`; assert `v.is_object()` and
  `v["entries"].is_array()` (F-03: the JSON is `ChangelogOutput { key, entries }`, NOT a bare
  array and NOT a `histories` key).
- `issue comments <key>` → array shape (standalone, beyond the M1 write-flow read-back).
- `board view --board <board_id>` → **bare JSON array of issue objects** (same shape as
  `issue list`; `handle_view` serializes `Vec<Issue>` via `print_output` — it is NOT an object;
  H-1). `--board` is a **flag**. Assert `v.is_array()` + per-element `assert_issue_shape` (if
  non-empty). **Clean-skip caveat (L-board):** a scrum board with no active sprint **bails
  non-zero** ("No active sprint found for board …") — do NOT treat that as failure; gate to a board
  known to have an active sprint, or tolerate that specific stderr. Gated on `JR_E2E_BOARD_ID`.
- `team list` → **array shape, BUT the empty-org path is a trap**: when the org has no teams,
  `handle_list` prints `"No teams found."` to **stderr** and exits **0** with **empty stdout**
  (returns before the JSON branch). A naive `serde_json::from_slice(stdout)` panics on empty input.
  Clean-skip condition is **empty stdout + exit 0** (do NOT key off exit code, which is 0). Parse +
  assert array only when stdout is non-empty.
- `user view <accountId>` → object with `accountId`. `account_id` is a positional (accountId or
  email). Resolve self via the `user search` seed — but **clean-skip if self-resolution yields
  nothing** (Browse Users permission may make `user search` return empty, per the existing
  `test_e2e_user_search_returns_array` note; no accountId → skip, don't fail).
- `issue link-types` → array shape; assert only `name` present (F-06: `id`/`inward`/`outward` are
  `Option` in `IssueLinkType` and serialize as null — only `name` is guaranteed). `--output json`
  IS supported (global flag).

### 6.2 Write / behavioral contracts
- **assign**: `issue assign <key>` with the assignee **omitted** → self-assignment (there is NO
  `--me` flag; F-01 — `handle_assign` falls to the `client.get_myself()` branch when no assignee
  is given; `--to me` is the equivalent explicit form). Then `poll_view` and assert
  `fields.assignee.accountId` is set on the **view body** (round-trip). NOTE: the assign command's
  *own* JSON is flat `{key, assignee, assignee_account_id, changed}` — read the accountId from the
  `poll_view` view body (`fields.assignee.accountId`), not from the assign output. (Self-assignment
  is permission-safe on any instance.)
- **link / unlink**: create two issues, `issue link A B` — **omit `--type`** to use the built-in
  default `Relates` (present on essentially all instances; more portable than picking the
  first `link-types` entry, whose ordering is instance-dependent). View A and assert the link to B
  is present by traversing `fields.issuelinks[]` and matching B's key under **either**
  `inwardIssue.key` **or** `outwardIssue.key` (F-09: the GET-render side that carries the partner
  key is not contractually fixed — check both). Assert link **presence by key**, not by
  `type.name` (avoids coupling to the link-type label). `issue unlink A B` → view A, assert no
  `issuelinks` entry references B.
- **edit --dry-run** (+ `--output json`): run `issue edit <key> --summary <new> --dry-run
  --output json` against a seeded issue. Assert (a) the output is **valid JSON**, and (b) **no
  mutation** occurred — a subsequent `poll_view` shows the summary **unchanged** (the load-bearing
  assertion; does not depend on exact dry-run key names). Do NOT hard-pin dry-run JSON key names
  the spec hasn't verified — the no-mutation round-trip is the portable contract. No write.
- **bulk move** — **DEFERRED (not delivered in S-E2E-4; tracked for a future pass).** The intended
  test: create 2 issues, bulk-move both, assert the bulk-result shape and that each transitioned
  (`poll_view` per key). The bulk-move JSON is `{taskId, results:[{key, status (, error)}]}` and
  the operation is **async/polled** — assert `results[].status ∈ {success, error, inaccessible}`,
  NOT `changed` (F-10: that is the single-key shape `{key, status, changed}`). `inaccessible` can
  occur transiently for an eventually-consistent just-created issue — treat it as retryable, not a
  failure. Pins the documented **non-idempotent** bulk contract (distinct from single-key
  idempotency). **Deferral rationale:** the async-poll bulk path adds significant test complexity
  and the single-key `move` idempotency contract (delivered in M1 §5.2 step 5) already exercises
  the transition machinery; the non-idempotent bulk shape is lower-risk and is recorded here as a
  follow-up rather than a blocker. The 11 other M2 tests are delivered.
- **pagination dedup**: create 3 issues under one **per-test-unique** label — embed `run_label()`
  plus `run_attempt`/a random nonce (e.g. `e2e-<run_id>-<attempt>-pgN`), NOT a shared/static label
  and NOT just `run_id` (a re-run reuses `GITHUB_RUN_ID`, only `run_attempt` differs — a bare
  run_id collides and inflates the count; M-2). Then `issue list --jql "labels=<unique-L>" --all
  --output json` (exact `labels=` match is valid JQL), assert the returned keys are
  **duplicate-free** and are a **superset of** the 3 created keys (NOT "exactly 3" — the dedup
  contract is what's under test; an exact-count adds a flake vector). Pins the JRACLOUD-95368
  client-side dedup contract without triggering the upstream bug.
  **Skip-vs-fail carve-out:** all 3 issues are *known created*, so `poll_jql`'s default
  clean-skip-on-budget-exceedance MUST NOT apply — that would mask the very dedup/pagination
  regression this test exists to catch. Use `poll_jql` to absorb index lag toward **≥3** results;
  treat only a **0-result** early state as pure lag. If the budget is exhausted with **1 or 2**
  results (some-but-not-all), **FAIL loudly** — a persistent short count after full budget is a
  regression, not lag.

### 6.3 Error / exit-code paths (no mutation)
Assert **mapped exit code** (`src/error.rs::JrError::exit_code()` contract) only; **never** message
substrings.

> **No JSON error envelope (H-2, CORRECTED):** `jr` does NOT emit a machine-readable JSON error
> envelope on these failure paths. `JrError` renders to **stderr** as a plain string
> (`print_error`; `ApiError` formats as `"API error ({status}): {message}"`), and stdout is empty
> on the error path — there is no `{errorMessages}` / `{error,code}` object on stdout for these
> commands (consistent with CLAUDE.md NFR-O-P: no `_meta` envelope; and the mocked error tests
> assert on stderr, never stdout JSON). Therefore: **assert exit code only** (plus "stdout is
> empty / process did not panic"). Do NOT assert a "JSON error field" — that contract does not
> exist and a `from_slice(stdout)` would fail on empty input.
>
> **Exit-code source of truth (F-04):** the mocked error tests pin **500→1** and **401→2**
> (`tests/issue_view_errors.rs`) and **500→1, 401→2, board-not-found→64** (`tests/issue_list_errors.rs`).
> No mocked pin exists for 400 or 404. Both *raw API* 400/404 map to exit **1** via the
> `JrError::ApiError` catch-all (`error.rs`: `_ => 1`), BUT a handler may wrap an API error in
> `UserError` → exit **64** (the board-not-found→64 remap proves list-path remapping is real).
> So accept exit **∈ {1, 64}** for 400/404 unless the implementer reads the specific handler
> branch and confirms a deterministic single code.

- **404** — `issue view E2E-99999999 --output json` → assert exit **∈ {1, 64}** (most likely 1 via
  `ApiError`; `issue view` *may* remap to `UserError`→64, cf. `user view`'s 404 handling). Stdout
  empty; error text on stderr (do not assert its wording).
- **400** — `issue list --jql "this is not valid ("` --output json → assert exit **∈ {1, 64}**
  (`handle_list` may wrap the JQL 400 in `UserError`→64 with a hint; raw → `ApiError`→1). Stdout
  empty; error on stderr.
- **401** — a command run with a deliberately bad-but-syntactically-valid `JR_AUTH_HEADER`
  (a well-formed `Basic <base64>` with wrong credentials, not a malformed string) → exit `2`
  (`JrError::NotAuthenticated`). Assert exit 2 + "no panic"; the auth error renders to stderr (do
  not assert Atlassian's sentence). **Debug-build-only by construction (F-11):** `JR_AUTH_HEADER`
  is gated behind `#[cfg(debug_assertions)]` (SD-002); the E2E harness runs the debug binary, so
  this is consistent with the rest of the suite — not a release-binary behavior.

## 7. Milestone M3 — Robustness & ops

### 7.1 Suite-side
- Adopt **`poll_jql`** for the create-then-search assertion(s) (the current summary-filter list
  test does not retry → latent flake on a cold index).
- **Secret-leak guard test**: run a normal `--output json` command and assert that neither stdout
  nor stderr contains the base64 token or the service-account email. Cheap, high-value, portable.
- **Leak-detection log** at suite start: count pre-existing **open** E2E issues via the
  best-effort `summary ~ "e2e"` predicate (NOT a label prefix — see F-02/M-1 in §7.2) and
  `eprintln!` the number (warn-only, never fails) — visible drift signals a broken teardown.
- (Optional, low priority) per-test wall-clock guard mapping a hung call toward exit 124.

### 7.2 CI / ops (`.github/workflows/`)
- **New `e2e-sweeper.yml`** — scheduled daily, non-blocking, `concurrency: jira-e2e` (shares the
  serialization group so it never interleaves with the main run). Closes
  `project=$JR_E2E_PROJECT AND summary ~ "e2e" AND statusCategory != Done AND created <= -1d`.
  **JQL correctness (F-02):** the cross-run sweeper matches on **`summary ~ "e2e"`**, NOT on
  labels. The `labels` field does not support the `~` (CONTAINS) operator and JQL has no
  label prefix/wildcard match, so `labels ~ "e2e-"` is invalid (HTTP 400). Every seeded issue
  carries the summary prefix `[e2e <run_label>]` (see existing tests).
  **Best-effort matching (M-1):** `~` is a **tokenized full-text** match, not substring/prefix —
  Jira strips punctuation like `[` and matches the *term* `e2e`. This is intentionally a
  best-effort cleanup backstop, not a precise selector: within the **dedicated, disposable** E2E
  project, over-matching is acceptable (everything there is throwaway) and under-matching only
  delays cleanup to the next sweep. Do NOT rely on it for precise selection — the per-run
  teardown in `e2e.yml` (exact `labels=e2e-$GITHUB_RUN_ID`) remains the precise path; the sweeper
  only catches what hard-cancellation left behind (research §4: cancelled runs skip even
  `if: always()`). Close-only (no delete), best-effort (`|| true`), idempotent.
  **Note:** the leak-detection log (§7.1) uses the same best-effort `summary ~ "e2e"` predicate.
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

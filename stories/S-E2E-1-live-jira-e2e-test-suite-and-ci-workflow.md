---
document_type: story
story_id: "S-E2E-1"
title: "Live-Jira E2E test suite + CI workflow (tests/e2e_live.rs + .github/workflows/e2e.yml)"
wave: feature-followup
status: draft
intent: feature
feature_type: infrastructure
scope: non-trivial
severity: medium
trivial_scope: false
issue: TBD
points: 8
priority: medium
tdd_mode: strict
estimated_effort: medium
depends_on: []
bc_anchors: []
# BC delta: EMPTY — this feature adds test infrastructure and a CI workflow only.
# No new product behavioral contracts. Zero src/ changes required.
# The JR_RUN_E2E env var is a test-harness gate operative only under
# #[cfg(debug_assertions)]; it is not a product behavior seam.
# BC status: no BC authorship required.
# Status=draft: no BCs anchor this story. Will remain draft until dispatched;
# the spec-first gate (S-7.01) does not block dispatch for infrastructure-only
# stories with explicit justification above.
verification_properties: []
holdout_anchors: []
nfr_anchors: [NFR-T-E2E-1]
adr_refs: []
sd_refs: [SD-002]
parent_phase: F3-story-decomposition
spec_source: "docs/specs/e2e-live-jira-testing.md"
implementation_strategy: tdd
module_criticality: MEDIUM
traceability_note: >
  BC delta is EMPTY (infrastructure feature; no product behavioral contracts).
  ACs trace to NFR-T-E2E-1 and to the canonical design spec sections
  (docs/specs/e2e-live-jira-testing.md §§ 3–8). Do not invent BC-S.SS.NNN
  identifiers — none exist for this story.
files_modified:
  - CLAUDE.md   # MODIFIED — add JR_RUN_E2E + JR_E2E_* env vars in AI Agent Notes; add E2E suite/workflow entry; add annual rotation runbook note
files_created:
  - tests/e2e_live.rs               # NEW — gated live-Jira E2E integration test suite
  - .github/workflows/e2e.yml       # NEW — CI workflow (push develop/main + nightly + workflow_dispatch; non-blocking; jira-e2e Environment)
breaking_change: false
assumption_validations: []
risk_mitigations: [R-NEW-1, R-NEW-2, R-NEW-3]
last_updated: "2026-05-29"
changelog:
  - date: "2026-05-29"
    phase: F5-adversarial-review
    author: story-writer
    summary: >
      AC-004-v2: stricken auth-status --output-json row (unsatisfiable — no JSON arm,
      no API call in src/cli/auth/status.rs); designated issue-list as the auth-seam
      validator. Added reconciliation note under AC-004. Updated Implementation Strategy
      step 3 and Tasks checklist to remove the auth-status test reference. Added OQ-5
      (OUT-OF-SCOPE) to Open Items recording the CLAUDE.md NFR-O-N doc/code drift for
      separate follow-up.
  - date: "2026-05-29"
    phase: F5-adversarial-review-pass-3
    author: story-writer
    summary: >
      H-2 fix: AC-004 board-list and user-search rows relaxed from "JSON array with at
      least one element" to "JSON array (shape-only; element count is site-dependent —
      do not over-fit)". The tests already assert is_array() only; the AC text was the
      inaccurate artifact (per S-398 lesson: do not over-fit to seed data). Amendment
      note added below the AC-004 table.
      M-2 fix: AC-007 step 1 corrected from --label e2e-<run_label> to --label <run_label>
      (single e2e- prefix). run_label() already returns e2e-<GITHUB_RUN_ID>, so the
      original wording implied e2e-e2e-<run_id> (double prefix), which would not match
      teardown JQL. Rationale sentence updated with clarifying note that run_label() is
      already prefixed. Implementation is correct; AC text was the inaccurate artifact.
---

# S-E2E-1 — Live-Jira E2E Test Suite + CI Workflow

## Source of Truth

Design spec: `/Users/zious/Documents/GITHUB/jira-cli/docs/specs/e2e-live-jira-testing.md`
F1 delta analysis: `/Users/zious/Documents/GITHUB/jira-cli/.factory/phase-f1-delta-analysis/e2e-live-jira-testing/F1-delta-analysis.md`
NFR anchor: `NFR-T-E2E-1` (added in F2, 2026-05-29; see `nfr-catalog.md` row 148)
GitHub issue: TBD (to be filed before F4 dispatch; provisioning tracking is R-NEW-1)

**No new BCs. No src/ changes. No new product CLI surface.**

## ID Convention Note

The project uses two story-ID schemes: sequential `S-W.NN` for wave stories and flat `S-NNN`
for feature-followup stories. This feature's F1 delta analysis explicitly proposed `S-E2E-1`
as a semantic label (mirroring the style used for feature cycles). The semantic ID is used
here because (a) the F1 recommendation was explicit, (b) the feature-followup group already
has non-numeric IDs (`issue-288-pr1-api`, etc.), and (c) `S-E2E-1` communicates purpose at a
glance without conflicting with any existing numeric ID.

## Goal

Implement a gated Rust integration test file (`tests/e2e_live.rs`) and a companion GitHub
Actions workflow (`.github/workflows/e2e.yml`) that exercise the compiled `jr` binary against
a real Jira Cloud instance. Update `CLAUDE.md` to document the new test-seam env vars.

This closes the testing coverage gap identified in the design spec §1: all 64 existing
integration tests are hermetic (wiremock / `JR_BASE_URL` mocks); nothing verifies `jr`
against real Jira API response shapes, pagination, or eventual consistency.

## Traceability

Because the BC delta for this feature is EMPTY (infrastructure only), acceptance criteria
trace to **NFR-T-E2E-1** and to the **design spec sections** instead of BC-S.SS.NNN clauses.
This is explicit and justified — see `traceability_note` in frontmatter and F1 §3.

| Traceability target | Type | Description |
|--------------------|----|-------------|
| NFR-T-E2E-1 | NFR (MEDIUM) | Obligation to keep the E2E suite runnable and wired into CI |
| Design spec §3 | Seam reference | Authentication via JR_BASE_URL + JR_AUTH_HEADER debug-only seams |
| Design spec §4 | Test structure | Gating, harness helper, read coverage, write flow |
| Design spec §5 | CI workflow | e2e.yml triggers, environment, teardown |
| Design spec §6 | Secret safety | PR guard, Environment branch policy |
| Design spec §8 | Configuration | JR_E2E_* env vars inventory |
| CLAUDE.md convention | Doc obligation | New JR_* env var entries per existing doc-fallout rule |

## Behavioral Contracts

None — infrastructure feature. BC delta is EMPTY. See F1 delta analysis §3.

## Acceptance Criteria

### AC-001 — Gate no-op invariant (traces to NFR-T-E2E-1; spec §4 Gating)

`cargo test --test e2e_live` without `JR_RUN_E2E=1` exits 0 and executes zero live Jira
calls. All tests in `tests/e2e_live.rs` are marked `#[ignore]` AND contain an early-return
guard (`if std::env::var("JR_RUN_E2E").as_deref() != Ok("1") { return; }`) to prevent any
test from reaching a Jira endpoint when the gate is unset.

A **non-gated assertion** (not `#[ignore]`, not early-return guarded) verifies this
invariant directly:

```rust
#[test]
fn test_suite_is_noop_without_jr_run_e2e() {
    // This test runs in ci.yml's plain `cargo test` and asserts that
    // JR_RUN_E2E is NOT set (which would mean ci.yml is unintentionally
    // setting it, causing live tests to run without the --include-ignored flag).
    assert_ne!(
        std::env::var("JR_RUN_E2E").as_deref(),
        Ok("1"),
        "JR_RUN_E2E=1 must not be set in a normal cargo test run"
    );
}
```

This regression-pins the gate: if `ci.yml` accidentally sets `JR_RUN_E2E=1`, this test
fails loudly in CI before any live call can be made.

Verification: `cargo test --test e2e_live` (without env) exits 0; the gate assertion test
is the only test that runs.

### AC-002 — `--include-ignored` without gate env returns early without contacting Jira (traces to NFR-T-E2E-1; spec §4)

`cargo test --test e2e_live -- --include-ignored` without `JR_RUN_E2E=1` exits 0. All
`#[ignore]` tests return early via the `JR_RUN_E2E` guard before constructing `e2e_cmd()`
or invoking `jr`. No HTTP connection is attempted.

Verification: run the above command locally without any `JR_E2E_*` vars set; it exits 0
with all gated tests marked as "ignored" or "ok" (depending on the early-return path).

### AC-003 — `e2e_cmd()` harness helper (traces to spec §4 Harness helper)

`tests/e2e_live.rs` defines a function `e2e_cmd() -> assert_cmd::Command` (or equivalent
`std::process::Command`) that constructs a `jr` binary invocation with:

- `JR_BASE_URL` set to the value of the `JR_E2E_BASE_URL` environment variable
- `JR_AUTH_HEADER` set to the value of the `JR_AUTH_HEADER` environment variable
- `XDG_CONFIG_HOME` set to a per-test `tempfile::TempDir` path (isolated config)
- `XDG_CACHE_HOME` set to a per-test `tempfile::TempDir` path (isolated cache)
- `--no-input` flag appended before any subcommand-specific args

The `TempDir` instances are kept alive for the duration of the test (not dropped early).

Helper functions also defined alongside `e2e_cmd()`:
- `run_label() -> String` — returns `e2e-<GITHUB_RUN_ID>` if set, otherwise `e2e-<unix_timestamp_ms>` for local runs
- `project() -> String` — returns `env::var("JR_E2E_PROJECT").expect("JR_E2E_PROJECT must be set")`
- `poll_view(key: &str) -> serde_json::Value` — see AC-005

Verification: `grep -n "fn e2e_cmd" tests/e2e_live.rs` returns exactly one match.

### AC-004 — Read command coverage (traces to NFR-T-E2E-1; spec §4 Read coverage)

**AC-004-v2 (supersedes AC-004-v1 row 1; amended 2026-05-29, F5 adversarial review):**

> **Reconciliation note (F-2/F-3, HIGH, 2026-05-29):**
> The original AC-004-v1 table included a row:
> `auth status --output json` → "exits 0; JSON object contains `accountId` or `emailAddress` field"
> This row is **unsatisfiable** against the real implementation. Verified at
> `src/cli/auth/status.rs::status`: the function accepts no `OutputFormat` parameter,
> emits only `println!` plaintext to stdout (no JSON arm, no `--output` flag), and makes
> **no Jira API call** — it probes only the local keychain/config. It cannot emit
> `accountId`/`emailAddress` (fields that require a `/rest/api/3/myself` call) and it
> cannot validate the auth seam against the live instance.
>
> **Resolution:** The auth seam is validated by the **first real network call** —
> `issue list --jql "project=<E2E>" --output json`. A 401 response from that call
> definitively indicates a broken `JR_AUTH_HEADER` seam or expired credential.
> The `auth status` command verifies only local keychain presence; it is not useful
> as a live-Jira E2E auth validator and is removed from this table.
>
> The `auth status --output json` row is stricken from AC-004. The `issue list` row
> (unchanged below) is the designated auth-seam validator.

When `JR_RUN_E2E=1` and the required env vars are set, the following read commands are
each exercised by at least one `#[ignore]`-gated test that asserts exit 0 and validates
the JSON output shape (presence checks, not value equality):

| Command | Minimum assertion |
|---------|-------------------|
| ~~`auth status --output json`~~ | ~~exits 0; JSON object contains `"accountId"` or `"emailAddress"` field~~ _(stricken 2026-05-29 F5: auth status emits no JSON and makes no API call — unsatisfiable; see reconciliation note above)_ |
| `issue list --jql "project=<E2E>" --output json` | exits 0; output is a JSON array (may be empty) — **also serves as the auth-seam validator**: a 401 here means the `JR_AUTH_HEADER` seam or credential is broken |
| `issue list --jql "project=<E2E> AND summary ~ e2e" --output json` | exits 0; output is a JSON array |
| `issue view <SEED_OR_CREATED_KEY> --output json` | exits 0; JSON object contains `"key"` field |
| `board list --output json` | exits 0; output is a JSON array _(shape-only; element count is site-dependent — do not over-fit)_ |
| `sprint list --board <BOARD_ID> --output json` (or equivalent) | exits 0; output is a JSON array (only if `JR_E2E_BOARD_ID` is set; skipped cleanly if unset) |
| `sprint current --board <BOARD_ID> --output json` (or equivalent) | exits 0; output is a JSON object or array (only if `JR_E2E_BOARD_ID` is set) |
| `worklog list <KEY> --output json` | exits 0; output is a JSON array (may be empty) |
| `user search <SELF_NAME_OR_EMAIL> --output json` | exits 0; output is a JSON array _(shape-only; element count is site-dependent — do not over-fit)_ |
| `project fields --project <E2E> --output json` (or equivalent) | exits 0; output is a JSON **object** with `issue_types` + `statuses_by_issue_type` keys (corrected 2026-05-29, F5 pass-5: the handler emits an object, not an array; test asserts `is_object()` + key presence) |

**JSM optional:** if `JR_E2E_JSM_PROJECT` is set, run `jr queue list --project <JSM>` and
`jr requesttype list --project <JSM>`; skip cleanly (not a test failure) if the env var is unset.

> **AC-004 amendment note (2026-05-29, F5 pass-3):** The `board list` and `user search` rows
> previously stated "JSON array with at least one element." That phrasing over-specifies:
> board and user counts are site-dependent (the sandbox may have zero boards until provisioned,
> or zero user-search results for a novel query term). Per the S-398 lesson ("do not over-fit
> to seed data"), the assertion is relaxed to shape-only (`is_array()` — zero or more elements).
> The tests already assert shape only; the AC text was the inaccurate artifact and has been
> corrected above.

Verification: `grep -c "#\[ignore\]" tests/e2e_live.rs` returns at least 10 (one gate per
read command family, plus the write flow tests).

### AC-005 — `poll_view` bounded retry (traces to spec §4 poll_view; spec §7 read-after-write)

`poll_view(key: &str) -> serde_json::Value` in `tests/e2e_live.rs` implements bounded retry:

- Attempts at most **5** iterations
- Sleeps a short backoff between attempts (e.g., 500ms initial, doubling or fixed)
- On each attempt: runs `e2e_cmd()` with `["issue", "view", key, "--output", "json"]`
- Returns the parsed `serde_json::Value` on first exit 0 + valid JSON
- After exhausting all attempts, panics with a clear message: `"poll_view({key}): timed out after N attempts — GET-by-key not consistent"`

The function does NOT retry indefinitely. It does NOT use `std::thread::sleep` with an
unbounded loop. The attempt count and backoff duration are compile-time constants (not
controlled by env vars).

Rationale: GET-by-key is assumed read-after-write consistent (per design spec §4 and §7),
but the bounded retry is the real guarantee.

Verification: `grep -n "fn poll_view" tests/e2e_live.rs` returns exactly one match.
`grep -n "loop\b\|while true\|loop {" tests/e2e_live.rs` returns ZERO matches (no unbounded loops).

### AC-006 — Configurable status names via env vars (traces to spec §8; F2 resolution of Open Item 2)

The write flow and any status-based assertions use these env vars for status names:

- `JR_E2E_STATUS_DONE` — status name for "closed/done"; default value `"Done"` if unset
- `JR_E2E_STATUS_IN_PROGRESS` — status name for "in progress"; default value `"In Progress"` if unset

Helper functions in `tests/e2e_live.rs`:

```rust
fn status_done() -> String {
    std::env::var("JR_E2E_STATUS_DONE").unwrap_or_else(|_| "Done".to_string())
}
fn status_in_progress() -> String {
    std::env::var("JR_E2E_STATUS_IN_PROGRESS").unwrap_or_else(|_| "In Progress".to_string())
}
```

These functions are used in the write flow (AC-007) and MUST NOT use hard-coded status
strings anywhere else in `tests/e2e_live.rs`.

Verification: `grep -n '"Done"\|"In Progress"' tests/e2e_live.rs` returns ZERO matches
outside of the `status_done()` and `status_in_progress()` function bodies.

### AC-007 — Write flow happy path (traces to NFR-T-E2E-1; spec §4 Write flow)

A single `#[ignore]`-gated test exercises the full write flow against the live site, all
steps in sequence:

1. `issue create --project <E2E> --type Task --summary "[e2e <run_label>] smoke test" --label <run_label> --output json` — exits 0; captures `key` from JSON `{"key": "..."}` response.
2. `poll_view(key)` — returns without panicking; response JSON contains `"key": key`.
3. `issue edit <key> --summary "[e2e <run_label>] smoke test (edited)" --output json` — exits 0.
4. `issue comment <key> "E2E smoke comment" --output json` — exits 0. (comment text is a POSITIONAL `message` arg — there is no `--body` flag; corrected 2026-05-29, F5 pass-6.)
5. `worklog add <key> 5m --output json` — exits 0.
6. `issue move <key> <status_in_progress()>` — exits 0; single-key `move` is idempotent if already in that status.
7. `issue move <key> <status_done()>` — exits 0.

Each step's exit code is asserted. The key captured in step 1 is the run-scoped key used
for all subsequent steps — no cross-test state sharing.

The label `<run_label>` (format: `e2e-<GITHUB_RUN_ID>` or `e2e-<timestamp>`, produced by
`run_label()`) ensures the teardown step in the workflow can select exactly this run's issues.
`run_label()` already includes the `e2e-` prefix — callers pass it directly to `--label`
without an extra `e2e-` prefix.

Verification: the write flow test function (identified by name containing "write_flow" or
"create_edit_comment_worklog_close") exercises all 7 sub-steps.

### AC-008 — Workflow `e2e.yml`: triggers, environment, concurrency, timeout, permissions (traces to spec §5)

`.github/workflows/e2e.yml` exists with the following structure:

```yaml
on:
  push:
    branches: [develop, main]
  schedule:
    - cron: "0 6 * * *"        # 06:00 UTC nightly
  workflow_dispatch:

concurrency:
  group: jira-e2e
  cancel-in-progress: false    # serialize on shared Jira site — do not cancel

jobs:
  e2e:
    runs-on: ubuntu-latest
    environment: jira-e2e      # secrets gated to this Environment + branch policy
    timeout-minutes: 20
    permissions:
      contents: read
    steps:
      # ... (see AC-009 for if: guard; AC-010 for teardown)
```

Verified field by field:
- `on.push.branches` includes both `develop` AND `main`
- `on.schedule.cron` is present (nightly schedule)
- `on.workflow_dispatch` is present
- `concurrency.group: jira-e2e` AND `cancel-in-progress: false` (both required)
- `environment: jira-e2e` present on the `e2e` job
- `timeout-minutes: 20` (or a lower value; NOT absent or higher than 30)
- `permissions.contents: read` (minimal permissions)

Verification: `grep -n "jira-e2e\|cancel-in-progress\|timeout-minutes\|contents: read" .github/workflows/e2e.yml` returns matches for all four patterns.

### AC-009 — Secret safety: PR guard + no pull_request_target (traces to spec §6)

`.github/workflows/e2e.yml` contains:

1. `if: github.event_name != 'pull_request'` on the `e2e` job (belt-and-suspenders; forks
   already cannot read the `jira-e2e` Environment secrets, but the guard prevents any
   future path that could reintroduce exposure).
2. The `on:` block does NOT contain `pull_request_target` as a trigger. Never.

Verification:
- `grep -n "pull_request_target" .github/workflows/e2e.yml` returns ZERO matches.
- `grep -n "github.event_name != 'pull_request'" .github/workflows/e2e.yml` returns exactly one match (on the job-level `if:`).

### AC-010 — `if: always()` teardown step (traces to spec §5 Teardown)

`.github/workflows/e2e.yml` contains a teardown step with these properties:

- `if: always()` — runs even when earlier steps fail, to guarantee issue cleanup
- Selects all issues labeled `e2e-${GITHUB_RUN_ID}` in the E2E project with
  `statusCategory != Done` via `jr issue list --jql ... --output json | jq -r '.[].key'`
- Pipes each key to `jr issue move "$KEY" "$STATUS_DONE"` where `STATUS_DONE` defaults
  to the value of `JR_E2E_STATUS_DONE` env var, falling back to `"Done"` if unset
- Uses `|| true` to prevent a single move failure from aborting the teardown loop

The teardown is close-only — it transitions issues to Done, it does NOT delete them.
Accumulated closed issues on the dedicated E2E project are acceptable (no DELETE command
exists in `jr` v1).

Verification: `grep -n "if: always()" .github/workflows/e2e.yml` returns at least one match.

### AC-011 — Non-blocking: e2e.yml is separate from ci.yml, not a required check (traces to spec §5; spec §2 Goals)

`.github/workflows/e2e.yml` is a standalone workflow file, entirely separate from
`.github/workflows/ci.yml`. The `ci.yml` file is NOT modified by this story. The `e2e.yml`
workflow is NOT added to any branch protection rule's required checks.

The test suite in `tests/e2e_live.rs` compiles and the gate-invariant test (AC-001) runs
as part of the normal `cargo test` in `ci.yml` — without contacting Jira.

Verification:
- `git diff --name-only HEAD` does NOT include `.github/workflows/ci.yml` (ci.yml unmodified).
- `grep -rn "e2e" .github/workflows/ci.yml` returns ZERO matches that would indicate the E2E workflow was wired into required checks.
- `cargo test --test e2e_live` exits 0 in the normal `ci.yml` run (gate invariant).

### AC-012 — CLAUDE.md documentation: new JR_* env var entries (traces to CLAUDE.md convention; F1 §2 Modified Files)

`CLAUDE.md` AI Agent Notes section is updated in the same commit as the test and workflow
files (per the project's "When adding a new `JR_*` test-seam env var" doc-fallout rule) to
document:

1. `JR_RUN_E2E` — gating env var; parallel entry to `JR_RUN_KEYRING_TESTS` and
   `JR_RUN_OAUTH_INTEGRATION` in the same bullet group.
2. `JR_E2E_BASE_URL` — real Jira Cloud site URL (secret in `jira-e2e` Environment)
3. `JR_AUTH_HEADER` — pre-composed `Basic <base64>` auth header (composed in-workflow from `JR_E2E_EMAIL` + `JR_E2E_API_TOKEN` secrets); note the **debug-only** seam gate
4. `JR_E2E_PROJECT` — Scrum project key for E2E
5. `JR_E2E_BOARD_ID` — optional; enables sprint mutation tests
6. `JR_E2E_JSM_PROJECT` — optional; enables JSM read tests
7. `JR_E2E_STATUS_DONE` — optional; default `"Done"`; configurable for sites with different status names
8. `JR_E2E_STATUS_IN_PROGRESS` — optional; default `"In Progress"`

The CLAUDE.md update must also include:
- A reference to the annual API-token rotation obligation (from spec §9; point to
  `docs/specs/e2e-live-jira-testing.md` §9 or add a brief runbook note inline).
- A note that the nightly schedule is a **data-retention guard** (not just latency), per
  spec §9 (free site deactivation after ~120 idle days → 15–60 day deletion window).

Verification: `grep -n "JR_RUN_E2E\|JR_E2E_" CLAUDE.md` returns at least 8 matches
(one per env var listed above).

## Out of Scope

- **Exact per-command JSON shape assertions** (Open Item 1, spec §12): finalized during
  implementation against the provisioned free site. Use field-presence checks rather than
  value equality where values depend on site state. Do not over-fit to seed data.
- **Token-expiry early-warning step** (Open Item 4, spec §12.4): deferred. The nightly
  job failing with 401 is the baseline signal. Loud, no data loss.
- **JSM write tests** (spec §2 Non-Goals): JSM read tests are gated behind `JR_E2E_JSM_PROJECT`; JSM write tests are out of scope for v1.
- **Sprint mutation tests** (spec §2): gated behind `JR_E2E_BOARD_ID`; only exercised if the variable is set.
- **Assets/CMDB E2E tests** (spec §2 Non-Goals): Assets requires paid plan (Standard+); not available on free tier. All CMDB tests remain wiremock-only.
- **OAuth 3LO / keychain login flows** (spec §2 Non-Goals): interactive/local; cannot run in CI. E2E authenticates via `JR_AUTH_HEADER` Basic seam.
- **jr issue delete command** (spec §2 Non-Goals): teardown is close-only; no DELETE.
- **Provisioning the Jira site** (R-NEW-1, F1 §9): one-time manual step documented in spec §10; tracked by a separate GitHub issue to be filed. The code and workflow can be merged before the site exists — the gated tests are no-ops without `JR_RUN_E2E=1`.
- **Any src/ changes** (F1 §2): zero source changes required; the feature reuses existing `JR_BASE_URL` and `JR_AUTH_HEADER` debug-only seams.

## Open Items (carry-forward from design spec §12 and F1 §9)

| # | Item | Risk | Resolution path |
|---|------|------|----------------|
| OQ-1 | Exact read assertions — field-presence checks stable across a fresh free site | LOW | Finalize during provisioning; use `"field_exists"` style assertions |
| OQ-3 | JSM free-tier coverage — confirm which JSM read commands work on free | LOW | Keep behind `JR_E2E_JSM_PROJECT` flag; skip cleanly if unset |
| OQ-4 | Token-expiry early warning (optional) | LOW | Defer; 401 on expiry is the baseline signal |
| R-NEW-1 | Site provisioning — if not done, workflow silently fails every run | MEDIUM | AC-008 and AC-010 are verifiable in dry-run before provisioning; file tracking issue at F4 dispatch |
| R-NEW-2 | Free-site idle deactivation data loss (15–60 day post-deactivation deletion window) | LOW | Nightly cron is the primary mitigation; runbook note in CLAUDE.md (AC-012) |
| R-NEW-3 | Concurrent runs clobber each other | LOW | `cancel-in-progress: false` serializes; run-scoped labels prevent cross-run confusion |
| OQ-5 (OUT-OF-SCOPE — doc drift, separate follow-up) | `CLAUDE.md` NFR-O-N line states: "auth status --output json covers single-profile JSON" — this contradicts `src/cli/auth/status.rs`, which has **no JSON arm and makes no API call**. The CLAUDE.md entry is pre-existing doc/code drift. This story does not fix `auth status` behavior (zero `src/` changes). **Recommended action: file a separate follow-up issue** to either (a) implement a JSON arm in `auth status` that calls `/rest/api/3/myself` and emits `accountId`/`emailAddress`, or (b) remove the NFR-O-N claim as inaccurate. Not fixed here. | LOW | File follow-up issue at F5 close; note in CLAUDE.md PR review. |

## Implementation Strategy

**TDD order (recommended per F1 §8 Recommended F4 order):**

1. **Gate no-op test first (AC-001)** — write the non-gated `test_suite_is_noop_without_jr_run_e2e` test. This runs in `ci.yml` immediately and prevents accidental live calls. This test must pass before any other test is written.

2. **Harness helpers (AC-003, AC-005, AC-006)** — write `e2e_cmd()`, `run_label()`, `project()`, `poll_view()`, `status_done()`, `status_in_progress()`. These are the foundation for all other tests. Compile-check without running live.

3. **Read tests (AC-004)** — write `#[ignore]`-gated tests for each read command family in the table. Start with `issue list --jql "project=<E2E>" --output json` (cheapest network call; also serves as the auth-seam validator — a 401 here means the seam/credential is broken), then `issue view`, then `board list`. Do NOT write a test for `auth status --output json` — that command emits no JSON and makes no API call (see AC-004-v2 reconciliation note). Write one test at a time; run each locally with `JR_RUN_E2E=1` if credentials are available, or accept compile-check only until the site is provisioned.

4. **Write flow (AC-007)** — write the `#[ignore]`-gated write flow test. Run locally with `JR_RUN_E2E=1` to confirm the full flow.

5. **Workflow file (AC-008, AC-009, AC-010, AC-011)** — write `.github/workflows/e2e.yml`. Verify field by field against the AC assertions. The file can be written and merged before the site is provisioned (secrets absent = workflow fails at the auth step, not at a code bug).

6. **CLAUDE.md update (AC-012)** — update in the same commit as the first commit that introduces `tests/e2e_live.rs` (per the project's atomic doc-fallout rule for `JR_*` env var additions).

**Branch:** `feat/S-E2E-1-live-jira-e2e` from `feat/e2e-live-jira-testing` (the branch this spec lives on, already checked out).

**Commit message:**
```
feat(e2e): add live-Jira E2E test suite + CI workflow (tests/e2e_live.rs, e2e.yml)
```

**PR target:** `develop` (standard feature-followup path).

## Test Coverage Strategy

| Test type | Count | Location | What it tests |
|-----------|-------|----------|---------------|
| Always-run gate invariant | 1 | `tests/e2e_live.rs` | Gate is not set in normal `cargo test`; prevents accidental live calls |
| `#[ignore]`-gated read tests | 10+ | `tests/e2e_live.rs` | Auth, issue list/search/view, board, sprint, worklog, user, project fields |
| `#[ignore]`-gated write flow | 1 | `tests/e2e_live.rs` | Full create → poll_view → edit → comment → worklog → move flow |
| CI workflow file | 1 file | `.github/workflows/e2e.yml` | Trigger config, Environment binding, concurrency, teardown |
| CLAUDE.md doc entries | 8+ lines | `CLAUDE.md` | JR_RUN_E2E + JR_E2E_* env vars |

**Test command (E2E suite when site is provisioned):**
```bash
JR_RUN_E2E=1 \
JR_BASE_URL=https://<site>.atlassian.net \
JR_AUTH_HEADER="Basic $(printf '%s:%s' "$EMAIL" "$TOKEN" | base64 -w0)" \
JR_E2E_PROJECT=E2E \
cargo test --test e2e_live -- --include-ignored --test-threads=1
```

`--test-threads=1` is required (tests share the live Jira project; parallelism causes
rate-limit pressure and non-deterministic ordering in the write flow).

## Quality Gate Self-Check

| Criterion | AC | Notes |
|-----------|-----|-------|
| `cargo test --test e2e_live` (no env) exits 0 | AC-001 | Gate invariant test passes |
| `grep -n "fn e2e_cmd" tests/e2e_live.rs` → 1 match | AC-003 | Harness helper present |
| `grep -n "fn poll_view" tests/e2e_live.rs` → 1 match | AC-005 | Bounded retry present |
| `grep -n '"Done"\|"In Progress"' tests/e2e_live.rs` → 0 matches outside helper bodies | AC-006 | No hard-coded status strings |
| `grep -n "pull_request_target" .github/workflows/e2e.yml` → 0 matches | AC-009 | No PR-target trigger |
| `grep -n "github.event_name != 'pull_request'" .github/workflows/e2e.yml` → 1 match | AC-009 | PR guard present |
| `grep -n "if: always()" .github/workflows/e2e.yml` → ≥1 match | AC-010 | Teardown guaranteed |
| `grep -n "jira-e2e" .github/workflows/e2e.yml` → ≥2 matches (group + environment) | AC-008 | Concurrency group + Environment |
| `grep -n "cancel-in-progress: false" .github/workflows/e2e.yml` → 1 match | AC-008 | Serialize on shared site |
| `git diff --name-only HEAD` does NOT include `ci.yml` | AC-011 | ci.yml unmodified |
| `grep -n "JR_RUN_E2E\|JR_E2E_" CLAUDE.md` → ≥8 matches | AC-012 | All env vars documented |
| `cargo test` exits 0 | smoke | Rust codebase unaffected; existing tests green |
| `cargo fmt --all -- --check` exits 0 | lint | No format drift |
| `cargo clippy --all-targets -- -D warnings` exits 0 | lint | No warnings |
| `bash scripts/check-spec-counts.sh` exits 0 | invariant | No BC frontmatter changed |
| `bash scripts/check-bc-cumulative-counts.sh` exits 0 | invariant | No count surfaces touched |
| `bash scripts/check-bc-no-numeric-test-counts.sh` exits 0 | invariant | No BC bodies with numeric counts |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~6 k |
| Design spec `docs/specs/e2e-live-jira-testing.md` | ~5 k |
| F1 delta analysis (targeted: §2 impact boundary, §5 regression surface) | ~4 k |
| `tests/oauth_embedded_login.rs` (model for `#[ignore]` + early-return gating pattern) | ~3 k |
| `CLAUDE.md` (AI Agent Notes section, ~lines 280-400) | ~5 k |
| New `tests/e2e_live.rs` (to write: ~300-400 LOC) | ~5 k |
| New `.github/workflows/e2e.yml` (to write: ~50-80 LOC) | ~2 k |
| Tool outputs (`cargo test`, `cargo clippy`, grep verifications, script exits) | ~3 k |
| BC files: 0 (none loaded — BC delta empty) | 0 |
| **Total** | **~33 k** |

Well within a single-agent context window (~200 k). No split required.
LOC delta: `tests/e2e_live.rs` +~350 LOC; `.github/workflows/e2e.yml` +~65 LOC;
`CLAUDE.md` +~15 LOC. No `src/` LOC changes.

## Tasks

- [ ] Create branch `feat/S-E2E-1-live-jira-e2e` from `feat/e2e-live-jira-testing`
- [ ] Read `tests/oauth_embedded_login.rs` — understand the `#[ignore]` + early-return gating pattern to replicate in `tests/e2e_live.rs`
- [ ] Read `CLAUDE.md` AI Agent Notes section — locate `JR_RUN_KEYRING_TESTS` and `JR_RUN_OAUTH_INTEGRATION` bullet positions for the new `JR_RUN_E2E` insertion point
- [ ] Create `tests/e2e_live.rs` skeleton: `#![allow(dead_code)]` gate, non-gated `test_suite_is_noop_without_jr_run_e2e` assertion (AC-001), module-level early-return macro or helper
- [ ] `cargo test --test e2e_live` — exits 0 with the gate test passing (Red Gate: 1 always-run test, 0 gated tests yet)
- [ ] Add harness helpers: `e2e_cmd()`, `run_label()`, `project()`, `status_done()`, `status_in_progress()` (AC-003, AC-006)
- [ ] Add `poll_view(key: &str) -> serde_json::Value` with bounded 5-attempt retry + backoff (AC-005)
- [ ] Add `#[ignore]`-gated read tests: `issue list` (auth-seam validator; a 401 = broken seam), `issue list with JQL filter`, `issue view`, `board list` — do NOT add an `auth status --output json` test (no JSON arm, no API call; see AC-004-v2) (AC-004)
- [ ] Add `#[ignore]`-gated optional tests: `sprint list`/`sprint current` (behind `JR_E2E_BOARD_ID` check), `worklog list`, `user search`, `project fields` (AC-004)
- [ ] Add optional JSM read tests: `queue list`, `requesttype list` (behind `JR_E2E_JSM_PROJECT` check; skip cleanly if unset) (AC-004)
- [ ] Add `#[ignore]`-gated write flow test: all 7 sub-steps in AC-007 (issue create → poll_view → edit → comment → worklog add → move In Progress → move Done)
- [ ] Verify `grep -n '"Done"\|"In Progress"' tests/e2e_live.rs` → 0 matches outside helper bodies (AC-006)
- [ ] Update `CLAUDE.md`: add `JR_RUN_E2E` entry in AI Agent Notes alongside `JR_RUN_KEYRING_TESTS`; add all `JR_E2E_*` env var entries; add rotation runbook note; add nightly keep-warm data-retention note (AC-012)
- [ ] `cargo test` — exits 0 (gate test passes; all gated tests skip without env)
- [ ] `cargo fmt --all -- --check` — exits 0
- [ ] `cargo clippy --all-targets -- -D warnings` — exits 0
- [ ] Create `.github/workflows/e2e.yml` per spec §5: triggers, concurrency group, jira-e2e Environment, timeout-minutes, permissions:contents:read, JR_AUTH_HEADER composition step, cargo test step, if:always() teardown (AC-008, AC-009, AC-010, AC-011)
- [ ] Verify all AC-008 grep checks pass against the written e2e.yml
- [ ] `grep -n "pull_request_target" .github/workflows/e2e.yml` → 0 matches (AC-009)
- [ ] Run `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh && bash scripts/check-bc-no-numeric-test-counts.sh` — all exit 0
- [ ] Commit all three files atomically (`tests/e2e_live.rs`, `.github/workflows/e2e.yml`, `CLAUDE.md`): `feat(e2e): add live-Jira E2E test suite + CI workflow`
- [ ] Open PR targeting `develop`; body references the design spec and NFR-T-E2E-1
- [ ] File GitHub issue for provisioning runbook (R-NEW-1 tracking issue)

## Previous Story Intelligence

**Structural predecessor: S-410** (gate keychain-transitive tests behind `JR_RUN_KEYRING_TESTS=1`,
PR #416). This story follows the same gating pattern at a larger scale: every live test is
`#[ignore]` + early-returns without the env var. The key lesson from S-410 (confirmed by
S-428 follow-up): the gate invariant must be enforced by BOTH `#[ignore]` AND an early-return
guard inside each gated function, because `--include-ignored` bypasses `#[ignore]`.

**Structural model: `tests/oauth_embedded_login.rs`** — the existing design pattern for this
exact scenario. Read it before writing `tests/e2e_live.rs`.

**Lesson from S-428 (L-421-4):** when classifying tests as "does not touch external state",
consider the full subprocess lifecycle, not just the code path to the critical operation.
For the E2E suite, the early-return guard must fire BEFORE `e2e_cmd()` is constructed —
the `TempDir` creation itself is harmless, but any `jr` subprocess spawn must be prevented.

**Lesson from S-2.05 / S-408:** CLAUDE.md updates that describe test infrastructure MUST be
in the same commit as the code they describe. No separate "docs" commit.

**Lesson from S-398 (description echo asymmetry):** when writing E2E assertions, use
field-presence checks (JSON key exists) rather than value equality where values depend on
site state. The E2E site is a live, potentially dirty environment — assertions must be
stable across re-runs.

## Architecture Compliance Rules

1. **Zero `src/` changes.** The feature reuses `JR_BASE_URL` and `JR_AUTH_HEADER` debug-only
   seams already gated by `#[cfg(debug_assertions)]` in `src/config.rs` and
   `src/api/client.rs`. If any `src/` change is needed, STOP and escalate — this is a scope
   violation per F1 §2 ("ZERO src/ changes required for v1").

2. **`#[ignore]` + early-return, not just one.** Every gated test MUST have both:
   - `#[ignore]` attribute (prevents the test from running under normal `cargo test`)
   - Early-return guard: `if std::env::var("JR_RUN_E2E").as_deref() != Ok("1") { return; }`
   The guard catches the `--include-ignored` path. See S-410 pattern.

3. **`--test-threads=1` is REQUIRED in the workflow.** The test suite shares a single live
   Jira project. Parallel test execution causes rate-limit pressure and non-deterministic
   write-flow ordering. The `cargo test` step in `e2e.yml` MUST pass `-- --test-threads=1`.

4. **No `pull_request_target` in the workflow.** This trigger bypasses GitHub's fork PR
   secret withholding and would expose Jira credentials to untrusted code. It is explicitly
   banned per design spec §6. Fail the review if it appears.

5. **Teardown uses `|| true` per step.** Individual `jr issue move` failures in teardown
   must not abort the teardown loop (a partially-closed set is better than an aborted
   teardown). Use `|| true` or equivalent.

6. **No hard-coded status strings.** All status name references use `status_done()` and
   `status_in_progress()` helper functions that fall back to defaults (AC-006). See
   Architecture Rule #1 in the Quality Gate Self-Check table.

7. **Concurrency `cancel-in-progress: false`.** The E2E project is shared; cancelling
   a run mid-flight leaves orphaned in-progress issues. Serialization (not cancellation)
   is the correct model.

8. **`JR_AUTH_HEADER` is composed in-workflow, not stored as a combined secret.** The
   workflow composes the header from `JR_E2E_EMAIL` + `JR_E2E_API_TOKEN` secrets:
   `JR_AUTH_HEADER="Basic $(printf '%s:%s' "$EMAIL" "$TOKEN" | base64 -w0)"`. This
   allows rotating email and token independently and avoids storing a pre-composed
   base64 blob (which could silently become invalid after a token rotation).

## Library & Framework Requirements

No new `Cargo.toml` dependencies. The test file uses only crates already present:

| Crate | Already in Cargo.toml | Usage |
|-------|----------------------|-------|
| `assert_cmd` | Yes (dev-dep) | `Command::cargo_bin("jr")` for E2E subprocess invocation |
| `serde_json` | Yes (dev-dep) | Parse `--output json` responses in assertions |
| `tempfile` | Yes (dev-dep) | `TempDir` for isolated `XDG_CONFIG_HOME`/`XDG_CACHE_HOME` per test |

If `assert_cmd` or `serde_json` or `tempfile` is NOT present in `Cargo.toml`
dev-dependencies, add them before writing test code. Verify with `grep -n "assert_cmd\|serde_json\|tempfile" Cargo.toml` before proceeding.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `tests/e2e_live.rs` | CREATE | Gated live-Jira E2E test suite (~350 LOC) |
| `.github/workflows/e2e.yml` | CREATE | CI workflow (~65 LOC) |
| `CLAUDE.md` | MODIFY | Add JR_RUN_E2E + JR_E2E_* env var entries + rotation runbook note (~+15 LOC) |

**Files NOT to create:** No new `src/` files, no new spec files, no new ADR, no new BC files.

**Files NOT to touch:** All of `src/`, `Cargo.toml`, `deny.toml`, `.github/workflows/ci.yml`,
`.github/workflows/release.yml`, `tests/common/`, all snapshot files (`tests/snapshots/`),
all existing `tests/*.rs` files, `STORY-INDEX.md` (state-manager updates that), all BC
count surfaces (frontmatter, `BC-INDEX.md`, `CANONICAL-COUNTS.md`).

## Branch / PR Plan

- Branch: `feat/S-E2E-1-live-jira-e2e` (from `feat/e2e-live-jira-testing`)
- Target: `develop`
- Commit: `feat(e2e): add live-Jira E2E test suite + CI workflow (tests/e2e_live.rs, e2e.yml)` (single atomic commit: all three files)
- PR body: reference design spec, NFR-T-E2E-1, and note that tests are no-ops until provisioned
- CHANGELOG entry: Add under `[Unreleased]` — "Added live-Jira E2E test suite (`tests/e2e_live.rs`) and CI workflow (`.github/workflows/e2e.yml`); guarded by `JR_RUN_E2E=1`; non-blocking in CI."

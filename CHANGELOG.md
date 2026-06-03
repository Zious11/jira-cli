# Changelog

All notable changes to jr will be documented here.

## [Unreleased]

### Breaking Changes

- **`jr issue move <key> <done-status>` now requires an explicit resolution on
  done-category transitions** (BC-3.2.013, ADR-0015, S-JSM-RESOLUTION-REQUIRED).
  When the target transition is done-category AND offers a resolution field (or has
  `isConditional=true`), the command now enforces resolution upfront:
  - Non-interactive (`--no-input` or no TTY): exits 64 unless `--resolution <name>`
    or `--no-resolution` is supplied.
  - Interactive (TTY): prompts for a resolution via `dialoguer::Select`.
  - `--no-resolution`: explicit opt-out for intentional null-resolution closes (e.g.,
    "Won't Do" automation paths). Mutually exclusive with `--resolution` (exit 2).
  - Scripts relying on the silent bypass must add `--resolution <name>` (recommended)
    or `--no-resolution` (explicit opt-out). Bulk `jr issue move` (multi-key) is NOT
    affected — only single-key move is subject to proactive enforcement.
  - `jr issue transitions --output json` output is byte-identical to pre-feature
    (`skip_serializing` on `Transition.fields` and `Transition.is_conditional`).

### Added

- **`--no-resolution` flag on `jr issue move`:** explicit opt-out from proactive
  resolution enforcement (BC-3.2.013). Use when closing on done-category transitions
  where a null resolution is genuinely intentional. Mutually exclusive with
  `--resolution`. No effect on non-done-category transitions. (S-JSM-RESOLUTION-REQUIRED)
- **`jr issue move` proactive resolution detection:** single-key path now calls
  `GET .../transitions?expand=transitions.fields` to detect whether the target
  transition offers a resolution field — no additional round-trip (replaces the plain
  `GET .../transitions` call in `handle_move`). `jr issue transitions` read command
  unchanged. (BC-3.2.013, ADR-0015, S-JSM-RESOLUTION-REQUIRED)

- **JSM live-E2E coverage expansion (S-JSM-E2E-1):** replaces 2 shallow JSM smoke tests
  with 7 shape-asserting / round-trip live tests — queue list/view (by-name + `--id`),
  requesttype list/fields (numeric-bypass pin), internal vs external comment visibility
  round-trip, `issue create --request-type` write round-trip (ADR-0014 dispatch fork), and
  the non-JSM `require_service_desk` guard (exit-64 + message assertions). Also adds a
  SURFACE flag-subset guard (`test_e2e_live_flags_are_subset_of_surface_table`) that closed
  a pre-existing gap (`--priority` missing from the `issue edit` SURFACE row). Zero `src/`
  change. Gated on `JR_E2E_JSM_PROJECT`; set to `EJ` in the `jira-e2e` GitHub Environment
  to activate. (S-JSM-E2E-1)
- **Fork-safe E2E CI gate:** `e2e.yml` and `e2e-sweeper.yml` are now gated by a repository
  variable `JR_E2E_ENABLED`. Both workflow jobs skip cleanly on forks and any repo where the
  variable is not set (empty string `!= 'true'`). A preflight step in `e2e.yml` asserts all
  required secrets/variables are present before consuming runner minutes building Rust.
  **Maintainers:** after merging, create a repository variable `JR_E2E_ENABLED=true` at
  Settings → Secrets and variables → Actions → Variables (repository scope, NOT
  environment scope) to re-enable nightly E2E on the canonical repo. Without this step both
  workflows skip on every trigger. See `docs/specs/e2e-fork-safe-ci-enablement.md §5.1`.
- **README E2E status badge:** `[![E2E](...e2e.yml/badge.svg?branch=develop)]` added as the
  second badge in the badge row. Shows green for passing or skipped runs (skipped = no
  `JR_E2E_ENABLED`); shows red when tests fail. Badge is pinned to the canonical repo.

### Fixed

- **JSM E2E self-close teardown (S-JSM-E2E-2):** the comment-visibility and
  create-request live tests now self-close their EJ tickets by dynamically discovering a
  closing transition (`statusCategory.key == "done"`, preferring Resolved/Closed/Done)
  instead of the hardcoded `"Done"` status name, which the EJ JSM workflow rejects —
  created EJ tickets were being left open on every nightly run. Best-effort teardown
  preserved (warn-and-return on failure, never fails the test). Zero `src/` change.
  (S-JSM-E2E-2)

### Changed

## [0.5.0-dev.13] - 2026-06-01

### Fixed

- **`jr issue edit --priority` (bulk / multi-key) now sends the correct `{"priorityId":"<id>"}` schema**,
  resolving the priority name to its id via `GET /rest/api/3/priority` and validating against real
  Jira Cloud. Adds live E2E coverage for priority (single + multi-key bulk), `worklog add`, and
  `issue` unassign. (#452, E2E-PG-4)
- **`jr issue edit --type` (bulk / multi-key) now uses the verified Jira Bulk Ops schema** — camelCase
  `issueType` in `editedFieldsInput`, project-scoped name→`issueTypeId` resolution via createmeta, and a
  cross-project exit-64 guard before any API call. (#331, #453)
- **`createmeta` issue-types response is parsed correctly** — the deserializer now reads the `issueTypes`
  field (not `values`) with offset-based pagination, fixing live issueType bulk-edit resolution that the
  mock-only tests had masked. (#331, #455)

### Changed

- Wired `JR_E2E_ISSUE_TYPE_ALT` into the live E2E workflow so the issueType bulk round-trip test runs in
  CI (`jira-e2e` environment). (#331, #454)
- Compacted `CLAUDE.md` gotchas / AI-agent-notes (~36% smaller) with no loss of load-bearing guidance. (#456)
- Dependency bumps (each cleared a 7-day soak measured from the dependency's version publish date):
  - `serde_json` 1.0.149 → 1.0.150 (#404)
  - `ossf/scorecard-action` 2.4.0 → 2.4.3 (#424)
  - `actions/dependency-review-action` 4.9.0 → 5.0.0 (#422)
  - `github/codeql-action` 3.35.5 → 4.35.5 (#423)
  - `actions/upload-artifact` 4.6.2 → 7.0.1 (#426)
  - `actions/checkout` 4.3.1 → 6.0.2 (#425)
  These four GitHub Actions majors move all workflows onto the Node.js 24 runtime; GitHub-hosted runners
  satisfy the new minimum runner requirement.

## [0.5.0-dev.12] - 2026-06-01

### Added

- Live-Jira E2E test suite (`tests/e2e_live.rs`) plus a non-blocking CI workflow
  (`.github/workflows/e2e.yml`) that exercises `jr` against a real Jira Cloud site.
  Gated behind `JR_RUN_E2E=1` (a complete no-op in normal `cargo test`); runs on push
  to `develop`/`main`, nightly, and on demand, inside a branch-restricted `jira-e2e`
  GitHub Environment. Covers read paths (issue/board/sprint/worklog/user/project, JSM
  optional) and a create→verify→edit→comment→worklog→transition write flow on a dedicated
  `E2E` project, with run-scoped labels and guaranteed close-only teardown. No `src/`
  changes; auth via the existing debug-only `JR_AUTH_HEADER`/`JR_BASE_URL` test seams.
  Includes enhancements from follow-up rounds: deeper assertions, new coverage (label
  add/remove, typed issue link/unlink, remote-link), error-path and robustness/ops
  hardening, an orphan-cleanup sweeper, and first-live-run fixes (empty-status default,
  sprint non-scrum skip). (S-E2E-1..5, #433, #434, #440, #441, #442)
- Offline CLI-surface guard (`tests/e2e_cli_surface_guard.rs`) that validates every `jr`
  subcommand path and flag referenced in `tests/e2e_live.rs` against `jr --help` at CI
  time, without requiring `JR_RUN_E2E` or any network access. Catches assumed-surface
  defects before live runs. (E2E-PG-1, #443)
- Live E2E coverage for label add/remove, `issue link/unlink --type`, and
  `issue remote-link`. (E2E-PG-4, #445)

### Fixed

- **`jr issue edit --label add:X / remove:Y` now works against real Jira Cloud.**
  Both single-key and multi-key label editing were previously broken end-to-end
  (returning HTTP 400 / failing to parse responses) and had only mock-test coverage.
  Single-key now uses `PUT /rest/api/3/issue/{key}` with the `update.labels` payload
  (bare string values; synchronous 204); multi-key now uses the correct `labelsFields`
  schema for the bulk endpoint, with `{"name":"<label>"}` objects per action element.
  Bulk poll responses also now tolerate Jira returning `taskId` and issue IDs as JSON
  integers rather than strings. (#447, #448, #449, #450; closes #446, BUG-LABEL-400)

## [0.5.0-dev.10] - 2026-05-26

### Added

- `issue edit`: new `--field NAME=VALUE` flag (repeatable) for setting arbitrary custom
  fields on an existing issue. Supports string, number, single-select (option), date,
  datetime, and user field types. Single-select options are resolved from `editmeta`
  `allowedValues` by human label (case-insensitive). Unsupported types (array, CMDB/any)
  exit 64 with an actionable hint. Field-name resolution uses case-insensitive substring
  match against `list_fields()`; supply `customfield_NNNNN` directly to bypass name
  resolution. Multi-key bulk path rejects `--field` (exit 64). (Issue #396)
- `jr issue edit` and `jr issue create` now echo changed/set fields on success.
  Table mode prints one `  field → value` line per field to stderr (alphabetical
  order; resolved team display name; `(updated)` marker for description; `(cleared)`
  for `--no-parent` / `--no-points`). `jr issue edit --output json` gains a
  `changed_fields` object in the response body with raw field values (description
  carries the raw user-supplied string, not the `(updated)` marker). (Issue #398)
- JSM request type support in `jr issue create` via `--request-type <NAME|ID>`,
  `--field NAME=VALUE`, `--on-behalf-of <accountId>` flags. When `--request-type`
  is set, the command dispatches to `POST /rest/servicedeskapi/request` instead of
  the platform `POST /rest/api/3/issue` endpoint; platform path is byte-for-byte
  unchanged otherwise. (Issue #288)
- `write:servicedesk-request` added to `DEFAULT_OAUTH_SCOPES`. Existing OAuth users
  **MUST re-consent** (`jr auth refresh` or `jr auth login`) to gain the new scope
  before JSM request creation will work. Existing access tokens continue working with
  old scopes until expiry; re-consent is triggered on the next token mint. (Issue #288)
- `jr issue create --request-type` and `jr issue create` (JSM path) now emit
  auth-aware 401 error hints. When a 401 occurs against `/rest/servicedeskapi/*`,
  the error message distinguishes between OAuth scope gaps (`write:servicedesk-request`
  missing) and API-token expiry, with actionable next-step guidance. (Issue #384)
- JSM input validation and UX polish for `jr issue create --request-type`: empty
  `--request-type` value is rejected at parse time (exit 64); combining
  `--markdown` with `--field description=` is rejected with a conflict error;
  using platform-only flags (`--type`, `--team`, `--sprint`, etc.) on the JSM
  path now emits a per-flag warning to stderr listing the ignored flags. (Issue #385)
- `jr issue edit --type` now emits an enriched error message when the transition
  is rejected with HTTP 400, including the target type name, the current hierarchy
  level, and a hint that cross-hierarchy type changes are not supported by Jira
  Cloud. `--no-parent` with a non-existent parent ID now surfaces a clear
  fake-endpoint hint instead of a raw 404. (Issue #388)

### Fixed

- `jr issue edit --label ... --field ...` combination on a single key now exits 64 with a
  clear conflict error instead of silently dropping the `--field` write and exiting 0. The
  `--label` routing fork calls a labels-only handler that does not accept custom-field pairs;
  the `--label` mutual-exclusion block now rejects this combination before any HTTP call.
  (FIX-F5-001, follow-up to issue #396)

### Dependencies

- `rand` bumped from 0.9.4 to 0.10.1. No user-visible behavior change; `jr` uses only
  the OS CSPRNG path (unaffected by the soundness fix in GHSA-cq8v-f236-94qc /
  RUSTSEC-2026-0097, which applies to `ThreadRng` with the `log` feature — neither of
  which `jr` enables). Dependency hygiene update. (Issue #413)

### BREAKING CHANGE (v0.6)

- `--verbose` no longer prints HTTP request/response bodies by default. Use `--verbose-bodies` for full body inspection. The new flag emits a PII warning.
- Rationale: prevents accidental PII leakage in shared terminals, debug log files, and AI-agent context windows. See [SD-003](.factory/architecture/security-decisions/SD-003-verbose-pii-redaction.md) for details.
- Migration: replace `jr ... --verbose` with `jr ... --verbose --verbose-bodies` if you relied on body inspection.

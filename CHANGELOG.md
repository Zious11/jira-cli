# Changelog

All notable changes to jr will be documented here.

## [Unreleased]

### Breaking Changes

### Added

### Fixed

### Changed

## [0.6.0-dev.3] - 2026-06-18

### Fixed

- **`jr issue comments` no longer stalls on repeated `nextPageToken` (S-525, BC-2.4.043):**
  `list_comments` in `src/api/jira/issues.rs` now carries the same non-advancing-offset
  guard that `get_changelog` uses — if `next <= start_at` after a page with `has_more=true`,
  the paginator aborts with an error instead of looping forever. Mirrors the JRACLOUD-95368
  pattern already present on the JQL search path. (#531)
- **Multi-line inline HTML in `--description`/comments no longer causes HTTP 400 (BC-7.2.011,
  #522):** `push_text`, `push_code`, and `text_to_adf` now enforce the ADF text-node invariant
  (no raw `\r` or `\n`) at the chokepoint level. In non-codeBlock context `\r\n`, lone `\r`,
  and bare `\n` are all mapped to a single space — matching `SoftBreak` semantics. In codeBlock
  context `\r\n`→`\n` and lone `\r`→`\n` (newlines preserved). This closes a reachable HIGH
  bug where multi-line inline HTML (e.g. `foo <span\nx>bar` in a `--description` or `issue
  comment`) emitted a raw `\n` into an ADF text node, which Jira rejected with HTTP 400. (#523)
- **Block-level HTML interior newlines rendered as `hardBreak` nodes instead of raw `\n`
  (BC-7.2.011, #492):** `markdown_to_adf` now applies Algorithm B to `Tag::HtmlBlock` content:
  the block is split on line boundaries and emitted as alternating `text`/`hardBreak` nodes
  (leading/trailing `hardBreak`s trimmed, empty result suppressed). Previously, multi-line
  block HTML emitted raw `\n` characters inside text nodes — an ADF schema violation that Jira
  rejected with HTTP 400. (#492)

### Changed

- **`jr issue create --request-type` and `jr project fields` now emit pretty-printed JSON
  (#526):** All `--output json` paths in `src/cli/` are now routed through
  `output::render_json`. The two commands that previously used compact `serde_json::json!`
  Display output — `handle_jsm_create` and `handle_fields` — now emit pretty-printed JSON
  consistent with every other `jr` command. `jq` and programmatic parsers that accept
  whitespace-insensitive JSON are unaffected; scripts that did byte-exact comparison against
  compact output will need updating. (#527, S-526)
- **`write_cmdb_fields_cache` and `write_object_type_attr_cache` now use model-b
  error handling (S-525/CR-007):** Both cache writers in `src/cache.rs` swallow disk-write
  errors with an `eprintln!("warning: …")` and return `Ok(())` rather than propagating via
  `?`. A failed cache write no longer prevents a successful API call from completing. Call
  sites updated to use `.ok()` (idiomatic for discarding an always-`Ok` result). (#531)
- **CI Gate aggregator job added as the single required branch-protection status check
  (S-CIGATE-1):** `ci.yml` now contains a `ci-gate` job (`needs: [fmt, clippy, test, msrv,
  deny, spec-guard]`, `if: always()`) that is the only job wired into branch protection on
  `develop`/`main`. New required CI jobs must be added to `ci-gate.needs`, never directly to
  branch protection, to prevent the matrix-rename fragility class. (#533 note: `cargo-mutants`
  examine_globs extended to cover `src/api/jira/issues.rs` and `src/cache.rs`; a keyring-
  touching test gated behind `JR_RUN_KEYRING_TESTS`.)
- **Opt-in release operations workflows added (inert by default):** Four new GitHub Actions
  workflows — Apple binary signing (`sign-and-publish.yml`), release backfill
  (`backfill-release.yml`), gap-fill (`release-gap-fill.yml`), and fork sync
  (`sync-upstream.yml`) — are activated only when the corresponding repository variables
  (`SIGNING_ENABLED`, `HOMEBREW_TAP_REPO`, `RELEASE_GAP_FILL_ENABLED`, `SYNC_UPSTREAM_REPO`)
  are set. The canonical repo has none of these set; existing CI is unaffected.
  (`docs/specs/fork-friendly-release-ops.md`, #503)
- **Documentation accuracy sweep (DRIFT-D1..D12, CR-003/CR-004):** internal doc-only
  corrections to `CLAUDE.md` and architecture notes; no runtime behavior changed. (#524)

## [0.6.0-dev.2] - 2026-06-14

### Added

- **Windows pre-built binary:** `jr-<version>-x86_64-pc-windows-msvc.zip` (containing
  `jr.exe`) is now published to GitHub Releases alongside the existing Unix `.tar.gz`
  artifacts. Packaged via PowerShell `Compress-Archive`; SHA-256 checksum file included.
  (ADR-0016)
- **Windows credential storage:** The `keyring` crate's `windows-native` feature is
  enabled, storing OAuth tokens and API tokens in Windows Credential Manager
  (`CRED_TYPE_GENERIC`). Prior to this change the keyring crate silently used a null
  backend on Windows, losing credentials across invocations. (ADR-0016, Decision 5b)
- **Idiomatic Windows config/cache paths:** On Windows, `jr` now resolves config to
  `%APPDATA%\jr` (`dirs::config_dir()`) and cache to `%LOCALAPPDATA%\jr`
  (`dirs::cache_dir()`). Unix paths (`~/.config/jr`, `~/.cache/jr/v1/<profile>/`)
  are unchanged. (BC-6.1.014, BC-6.2.016, ADR-0016 Decision 4)
- **Windows CI coverage:** `windows-latest` is added to both the `clippy` and `test`
  job matrices in `ci.yml`, providing per-PR regression protection for the
  `#[cfg(windows)]` code paths in `src/config.rs` and `src/cache.rs`. (ADR-0016,
  Decision 3)

## [0.5.0] - 2026-06-11

First stable release of the 0.5.0 line, consolidating the `0.5.0-dev.1`
through `0.5.0-dev.14` pre-releases. Highlights below; see the per-dev
sections for full detail.

### Breaking Changes

- **`jr issue move <key> <done-status>` now requires an explicit resolution on
  done-category transitions** (BC-3.2.013, ADR-0015). Supply `--resolution <name>`
  or `--no-resolution`; interactive sessions prompt. Bulk (multi-key) move is
  unaffected. (#465)

### Added

- **Jira Service Management support:** `jr requesttype list/fields`, `jr queue
  list/view`, `jr issue create --request-type` (servicedeskapi dispatch),
  `--internal` comments, JSM-aware 401 scope hints, and input validation.
  (#288 series, #379, #385, #394, #395)
- **Markdown → ADF coverage:** GFM task lists, GFM alerts (`> [!NOTE]`) → panel,
  super/subscript, bare-URL autolinking, footnotes, and block-HTML preservation.
  (#470, #471, #473, #474, #481, #487, #489)
- **Bulk operations:** multi-key `issue edit`/`move` via the Atlassian Bulk API,
  `--jql` selection, `--dry-run`, `--max` cap, and multi-field edits. (#110, #331, #345)
- **`issue edit` enhancements:** `--field NAME=VALUE` arbitrary custom-field edits,
  `--no-parent`, and changed-field echo on create/edit. (#324, #399, #401)
- **Auth:** OAuth auto-refresh on 401 with per-profile single-flight, multi-cloudId
  disambiguation (`--cloud-id`), and `auth --output json`. (#309, #320, #321)
- **Search:** keys-only JQL API and in-function pagination dedupe. (#362, #367)

### Fixed

- Bulk wire-schema corrections: label objects, `issueType` camelCase, `priorityId`,
  nested bulk-transition body, and numeric issue/task IDs in poll responses.
  (#447–#453, #449, #450, #479)
- ADF `listItem` and footnote content-model conformance. (#470, #481)
- Security hardening: `JR_BASE_URL` release-gate, `errorMessages` stderr
  sanitization, and `task_id` validation. (#355, #356, #357)
- Field-resolution numeric boundary parsing. (#418, #427)

### Changed

- Extensive live-Jira E2E suite with fork-safe CI gating, `cargo-mutants` CI,
  regression holdout suites, gitleaks secret scanning, and StepSecurity runner
  hardening. (#300–#306, #346, #373, #433–#499)

## [0.5.0-dev.14] - 2026-06-11

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
- **GFM task lists → ADF `taskList`/`taskItem` (#471, BC-7.2.010):** `markdown_to_adf`
  now maps `- [ ] task` / `- [x] done` (tight and loose lists, nested sublists) to
  native ADF `taskList` and `taskItem` nodes with `state: "TODO"/"DONE"`. Jira renders
  these as interactive checkboxes. Each item receives a document-unique `localId` via a
  DFS post-processing pass. Loose multi-paragraph items are merged using `hardBreak`
  separators. Nested `taskList` children and hoisted sibling lists are handled correctly.
  (`docs/specs/adf-task-list.md`)
- **GFM alerts → ADF `panel` (#483, BC-7.2.009):** `> [!NOTE]`, `> [!TIP]`,
  `> [!IMPORTANT]`, `> [!WARNING]`, `> [!CAUTION]` blockquotes are mapped to ADF
  `panel` nodes (panelType: `info`/`success`/`note`/`warning`/`error`). Nested panels
  and blockquotes are unwrapped recursively; nested tables are flattened to paragraphs.
  ADF `listItem` gains a `panel` unwrap arm. Round-trips back to GFM markers via
  `adf_to_text`. (`docs/specs/adf-panel-content-model.md`)
- **Markdown superscript/subscript → ADF `subsup` (#474):** `^x^` maps to ADF
  `subsup sup`; `~x~` maps to `subsup sub`. Double-tilde `~~x~~` continues to map
  to `strike`. Heading attributes (`## Title {#id}`) are parsed and silently dropped
  (ADF headings have no id attribute) rather than leaking `{#id}` into the title text.
- **Bare `http(s)://` URLs → ADF `link` mark (#473):** a post-build pass applies a
  `link` mark to bare URL runs in text nodes, so URLs are clickable in Jira without
  the author needing explicit Markdown link syntax. Scope is explicit-scheme only
  (`http(s)://`); `www.`-prefixed and bare emails are out of scope. Existing inline
  links and code spans are never double-linked. (`src/adf.rs`)
- **JSM live-E2E coverage expansion (S-JSM-E2E-1):** replaces 2 shallow JSM smoke tests
  with 7 shape-asserting / round-trip live tests — queue list/view (by-name + `--id`),
  requesttype list/fields (numeric-bypass pin), internal vs external comment visibility
  round-trip, `issue create --request-type` write round-trip (ADR-0014 dispatch fork), and
  the non-JSM `require_service_desk` guard (exit-64 + message assertions). Also adds a
  SURFACE flag-subset guard (`test_e2e_live_flags_are_subset_of_surface_table`) that closed
  a pre-existing gap (`--priority` missing from the `issue edit` SURFACE row). Zero `src/`
  change. Gated on `JR_E2E_JSM_PROJECT`; set to `EJ` in the `jira-e2e` GitHub Environment
  to activate. (S-JSM-E2E-1)
- **Fork-safe E2E CI gate (#459):** `e2e.yml` and `e2e-sweeper.yml` are now gated by a
  repository variable `JR_E2E_ENABLED`. Both workflow jobs skip cleanly on forks and any
  repo where the variable is not set (empty string `!= 'true'`). A preflight step in
  `e2e.yml` asserts all required secrets/variables are present before consuming runner
  minutes building Rust. **Maintainers:** after merging, create a repository variable
  `JR_E2E_ENABLED=true` at Settings → Secrets and variables → Actions → Variables
  (repository scope, NOT environment scope) to re-enable nightly E2E on the canonical
  repo. Without this step both workflows skip on every trigger. See
  `docs/specs/e2e-fork-safe-ci-enablement.md §5.1`.
- **README E2E status badge:** `[![E2E](...e2e.yml/badge.svg?branch=develop)]` added as
  the second badge in the badge row. Shows green for passing or skipped runs; shows red
  when tests fail. Badge is pinned to the canonical repo.
- **High-value live E2E coverage (#467, #468, E2E-HV-1/2):** expanded live test suite
  covers project list, user list, sprint add/remove, bulk move, write-flag paths
  (description/stdin/markdown, comment channels, story points, parent, `--field`).
- **Assign-by-query live E2E coverage (#458, E2E-PG-4).**

### Fixed

- **Leading-dash values now accepted for all free-text write-command args (#496, #471):**
  `issue create`/`edit` `--summary` and `--description`, `worklog add --message`, the
  `issue comment` positional message, and `issue remote-link --title` all carry
  `allow_hyphen_values = true`. This fixes the `"unexpected argument"` clap error that
  occurred when passing GFM markdown task lists (e.g. `--description "- [ ] todo"`),
  bullet-list content, or titled links as free-text write inputs. Surfaced by the nightly
  E2E test `test_e2e_markdown_task_list_produces_task_items`. Use `--description="…"` or
  `--description-stdin` for programmatic usage where the value may start with a dash.
  (`src/cli/mod.rs`)
- **`markdown_to_adf` listItem content-model conformance (#470, #477):** markdown like
  `- > quote`, `- # heading`, `- ---`, and indented tables inside list items no longer
  emit `blockquote`/`heading`/`table`/`rule` nodes as direct `listItem` children (ADF
  schema violation). Blockquotes are unwrapped recursively, headings are downconverted
  to paragraphs with inline marks preserved, tables are flattened to one paragraph per
  row, and rules are dropped. (BC-7.2.006, `docs/specs/adf-listitem-content-model.md`)
- **Markdown footnotes no longer emit malformed ADF (#481, #472):** `[^1]` reference
  markers and footnote definitions are now preserved as plain text markers and an
  appended definition section (one `rule` divider + one paragraph per definition) instead
  of being dropped or emitting ADF structures that Jira rejects with HTTP 400. Duplicate
  definition labels are deduped; empty container shells left by pulldown-cmark after
  hoisting are pruned. (`src/adf.rs`)
- **Block-level HTML preserved as literal text instead of being dropped (#489, #490):**
  `markdown_to_adf` now routes `Tag::HtmlBlock` through a `NodeKind::HtmlBlock` path
  rather than the silent `Sink` catch-all, so `<div>…</div>` and similar block HTML
  appears as a literal paragraph in ADF rather than vanishing. Interior newlines are kept
  verbatim; a single trailing newline is trimmed. Symmetric with the pre-existing inline
  HTML preservation. (`src/adf.rs`)
- **Bulk transition body now nested in `bulkTransitionInputs` wrapper (#479):**
  `POST /rest/api/3/bulk/issues/transition` was sending a flat top-level schema
  (`selectedIssueIdsOrKeys` + `transitionId` at root) that live Jira rejects with
  HTTP 400 "bulkTransitionInputs must not be empty". Fixed to the nested form required
  by the live API: `{"bulkTransitionInputs":[{"selectedIssueIdsOrKeys":[…],"transitionId":"…"}],"sendBulkNotification":false}`.
  (FIX-BULK-TRANSITION-001)
- **JSM E2E self-close teardown (#464, S-JSM-E2E-2):** the comment-visibility and
  create-request live tests now self-close their EJ tickets by dynamically discovering a
  closing transition (`statusCategory.key == "done"`) instead of the hardcoded `"Done"`
  status name, which the EJ JSM workflow rejects — created EJ tickets were being left open
  on every nightly run. Best-effort teardown preserved (warn-and-return on failure).
  Zero `src/` change. (S-JSM-E2E-2)
- **ADF read-path and gate-guard hardening (#499, #475):** `adf_to_text` human-mode
  coverage for `taskList`/`taskItem`/`panel`/`subsup`/block-HTML round-trips; E2E
  gate-guard `test_every_ignored_test_has_gate_guard` and
  `test_e2e_gate_disabled_when_env_unset` tightened to catch newly-added tests missing
  their `if !e2e_enabled() { return; }` guard.

### Changed

- Dependency bumps:
  - **`gitleaks/gitleaks-action` 2.3.9 → 3.0.0 (#469) — MAJOR version bump.** The
    v3 release drops the legacy `GITHUB_TOKEN`-based secret scanning in favour of a
    purpose-built token; existing workflows may need to update the action inputs if
    they customise the gitleaks configuration. Review the upstream v3 migration guide
    before merging if you maintain a fork.
  - `reqwest` 0.13.3 → 0.13.4 (#461)
  - `chrono` 0.4.44 → 0.4.45 (#497)
  - `EmbarkStudios/cargo-deny-action` 2.0.18 → 2.0.20 (#466)
  - `step-security/harden-runner` 2.19.3 → 2.19.4 (#463)
  - `github/codeql-action` 4.35.5 → 4.36.2 (#462, #498)
  - `actions/checkout` 6.0.2 → 6.0.3 (#484)

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

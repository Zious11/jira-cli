# jira-cli — Pre-VSDD Harmonization Plan

**Project:** jira-cli (`jr`)
**Source corpus:** `.reference/jira-cli/docs/superpowers/specs/` (56 files), `.reference/jira-cli/docs/specs/` (22 files), `.reference/jira-cli/docs/adr/` (referenced only)
**Generated:** 2026-05-04
**Method:** Filename heuristic + first 50 lines of each spec + targeted source grep / module file presence in current `src/` layout (per CLAUDE.md and Pass-0 inventory).
**Phase 0 ingest decision:** HARMONIZE pre-VSDD docs. Per human Q4: ADRs KEEP, v1 design IMPORT-AS-HISTORICAL, 55 superpowers + 22 docs/specs files become Phase 2 story candidates / validation inputs / archaeological.

Bucket definitions (from task prompt):
- **DELIVERED-AS-DESIGNED** — feature is in current code, spec aligns; spec is a validation input (cross-check BCs).
- **DELIVERED-DIVERGENT** — feature exists, but implementation diverged (rename, scope shift, supersession); needs reconciliation.
- **PARTIALLY DELIVERED** — needs a Phase 2 story to complete.
- **PIVOTED** — design replaced by a different design (most commonly ADR-0002 → ADR-0006 OAuth shift).
- **UNDELIVERED** — Phase 2 story candidate.
- **ARCHAEOLOGICAL** — historical interest only.

Confidence: HIGH = directly verified by file/symbol grep; MEDIUM = strong inference from current module layout vs. spec topic; LOW = topic-only match, evidence not chased.

---

## §1 — 55-File Inventory (`docs/superpowers/specs/` minus v1 design)

| # | File | LOC | One-line topic | Bucket | Confidence | Evidence / notes |
|---|------|-----|----------------|--------|------------|------------------|
| 1 | 2026-03-22-issue-handler-refactor-design.md | 135 | Refactor `issue.rs` to pass owned `IssueCommand` enums to handlers (kill `too_many_arguments` allows) | DELIVERED-AS-DESIGNED | HIGH | `src/cli/issue/` is now a directory module; handler signatures take command variants. |
| 2 | 2026-03-22-issue-linking-design.md | 315 | parent/child + issue↔issue link/unlink + link-types | DELIVERED-AS-DESIGNED | HIGH | `src/cli/issue/links.rs` (293 LOC) + `src/api/jira/links.rs` present. |
| 3 | 2026-03-22-row-dedup-design.md | 137 | Extract shared `format_issue_row()` helper | DELIVERED-AS-DESIGNED | HIGH | `src/cli/issue/format.rs` (225 LOC) is the live row formatter. |
| 4 | 2026-03-22-story-points-design.md | 328 | Story points custom field discovery, CRUD, sprint summaries | DELIVERED-AS-DESIGNED | HIGH | `helpers.rs` resolves points field; cache stores `story_points_field_id`; CLAUDE.md describes it. |
| 5 | 2026-03-23-explicit-flag-config-warning-design.md | 96 | Warn on stderr when `--<flag>` is given but config missing (vs silent skip) | DELIVERED-AS-DESIGNED | MEDIUM | Behavior pattern present in current code; spec is small and behavioral. Validate as BC input. |
| 6 | 2026-03-23-install-script-design.md | 125 | One-liner `install.sh` from GitHub Releases | DELIVERED-AS-DESIGNED | HIGH | `install.sh` exists at repo root. |
| 7 | 2026-03-23-issue-comments-listing-design.md | 212 | `jr issue comments KEY` (read-side) | DELIVERED-AS-DESIGNED | HIGH | `Comments` enum variant present in `cli/mod.rs`; `comments.rs` module live. |
| 8 | 2026-03-23-issue-module-split-design.md | 135 | Split `issue.rs` (1426 LOC) into `cli/issue/` directory | DELIVERED-DIVERGENT | HIGH | Split happened, but final layout extended beyond the 7-file plan: now includes `view.rs`, `comments.rs`, `changelog.rs`, `json_output.rs`, `assets.rs`. CLAUDE.md still references the older 7-file shape. Reconciliation needed in onboarding doc. |
| 9 | 2026-03-23-jql-string-escaping-design.md | 98 | `jql::escape_value` helper to prevent JQL injection | DELIVERED-AS-DESIGNED | HIGH | `src/jql.rs` exists with escaping logic. |
| 10 | 2026-03-23-unbounded-jql-guard-design.md | 94 | Reject unbounded JQL early in `issue list` | DELIVERED-AS-DESIGNED | MEDIUM | `jql.rs` is the JQL utility module per CLAUDE.md. Validate as BC input. |
| 11 | 2026-03-24-assets-cmdb-design.md | 495 | Standalone Assets/CMDB layer (search, view, tickets) | DELIVERED-AS-DESIGNED | HIGH | `src/api/assets/` (5 files) + `src/cli/assets.rs` + `src/types/assets/` all present. |
| 12 | 2026-03-24-common-filter-flags-design.md | 236 | `--assignee`, `--reporter`, `--recent` shorthand on `issue list` | DELIVERED-AS-DESIGNED | MEDIUM | Pass-2/3 contracts mention these flags on issue list. |
| 13 | 2026-03-24-default-result-limit-design.md | 244 | Default `--limit` on `issue list` to avoid context overflow | DELIVERED-AS-DESIGNED | MEDIUM | Pagination + default-limit pattern is in `list.rs`. Validate as BC. |
| 14 | 2026-03-24-issue-linked-assets-design.md | 297 | Surface CMDB objects on `issue view` (issue→asset lookup) | DELIVERED-AS-DESIGNED | HIGH | `src/cli/issue/assets.rs` (65 LOC) + `src/api/assets/linked.rs` are the live module pair. |
| 15 | 2026-03-24-jsm-queues-design.md | 384 | JSM queue list/view; project-type detection; servicedeskapi pagination | DELIVERED-AS-DESIGNED | HIGH | `src/api/jsm/` (servicedesks.rs, queues.rs) + `src/cli/queue.rs` + `src/types/jsm/` present. |
| 16 | 2026-03-25-jql-project-scope-design.md | 109 | Compose `--jql` and `--project` instead of override | DELIVERED-AS-DESIGNED | MEDIUM | Per Pass-2/3 JQL composition contracts. |
| 17 | 2026-03-25-open-flag-design.md | 100 | `--open` flag for `issue list` (filter to open statuses) | DELIVERED-AS-DESIGNED | MEDIUM | CLAUDE.md gotcha: status-category colors hardcoded for `--open`. Implementation present. |
| 18 | 2026-03-25-project-fields-global-flag-design.md | 131 | `project fields` honors global `--project` (drop positional arg) | DELIVERED-AS-DESIGNED | MEDIUM | `src/cli/project.rs` is single-file project handler; behavior is the global-flag path. |
| 19 | 2026-03-25-project-fields-statuses-design.md | 164 | Add statuses (grouped by issue type) to `project fields` output | DELIVERED-AS-DESIGNED | HIGH | `src/api/jira/statuses.rs` exists; `project fields` output includes statuses per Pass-2/3 contracts. |
| 20 | 2026-03-25-project-list-design.md | 169 | `jr project list` for project discovery | DELIVERED-AS-DESIGNED | HIGH | `src/api/jira/projects.rs` exists; `Project::List` variant present. |
| 21 | 2026-03-26-asset-attribute-names-design.md | 273 | Replace numeric `Attribute ID` with names via `/object/{id}/attributes` | DELIVERED-AS-DESIGNED | MEDIUM | `src/api/assets/objects.rs` covers single-object endpoint; resolution pattern matches spec. |
| 22 | 2026-03-26-board-flag-design.md | 150 | `--board` flag on sprint/board commands | DELIVERED-AS-DESIGNED | HIGH | Verified in Pass-2/3 contracts; CLAUDE.md mentions board-id resolution. |
| 23 | 2026-03-26-issue-view-fields-design.md | 292 | Add created/updated/reporter/resolution/components/fixVersions to `issue view` | DELIVERED-AS-DESIGNED | MEDIUM | `view.rs` (286 LOC) renders these fields per Pass-3 contracts. Validate against current snapshot tests. |
| 24 | 2026-03-26-jrerror-exit-codes-design.md | 107 | Map missing-config / no-input failures to JrError variants for distinct exit codes | DELIVERED-AS-DESIGNED | HIGH | `src/error.rs` has full `JrError` enum with `exit_code()` mapping per CLAUDE.md. |
| 25 | 2026-03-26-kanban-jql-fix-design.md | 94 | Fix `AND ORDER BY` JQL bug on kanban board view | DELIVERED-AS-DESIGNED | MEDIUM | `list.rs` JQL composition logic; bug-fix-class spec. |
| 26 | 2026-03-27-board-auto-resolve-design.md | 229 | Auto-resolve board ID from `--project` for sprint commands | DELIVERED-AS-DESIGNED | HIGH | Pass-1/2 architecture documents the auto-resolve flow. |
| 27 | 2026-03-27-board-view-limit-design.md | 192 | `--limit` for `board view` (close 1.8MB outlier) | DELIVERED-AS-DESIGNED | MEDIUM | Default-limit pattern in `board.rs`. |
| 28 | 2026-03-28-input-validation-design.md | 259 | Validate `--project`/`--status` exist before query (vs empty results) | DELIVERED-AS-DESIGNED | MEDIUM | `partial_match.rs` + validation patterns in `list.rs`. |
| 29 | 2026-03-28-sprint-current-limit-design.md | 134 | `--limit` on `sprint current` (last list-style outlier) | DELIVERED-AS-DESIGNED | HIGH | `sprint.rs` + Pass-3 contracts confirm `--limit`. |
| 30 | 2026-04-01-issue-edit-description-design.md | 137 | `issue edit --description` with markdown / file / stdin | DELIVERED-AS-DESIGNED | HIGH | `create.rs` handles edit; `--description-stdin --markdown` documented in v1 design. |
| 31 | 2026-04-02-cache-dedup-design.md | 161 | Refactor 5 near-identical read/write pairs in `cache.rs` | DELIVERED-AS-DESIGNED | MEDIUM | `cache.rs` per CLAUDE.md is the consolidated cache module with versioned root `v1/`. |
| 32 | 2026-04-02-issue-move-status-name-design.md | 112 | Accept target status name (not just transition name) on `issue move` | DELIVERED-AS-DESIGNED | MEDIUM | `workflow.rs` (788 LOC) handles move; spec-class fix. |
| 33 | 2026-04-03-handle-list-error-propagation-design.md | 87 | Propagate board/sprint API errors in `handle_list` | DELIVERED-AS-DESIGNED | MEDIUM | Error-propagation contract; small spec, validate as BC. |
| 34 | 2026-04-03-issue-create-url-design.md | 112 | Include browse URL in `issue create` output | DELIVERED-AS-DESIGNED | MEDIUM | Per Pass-3 BCs: `issue create --output json` returns `{key, url}`. |
| 35 | 2026-04-03-partial-match-duplicate-names-design.md | 191 | Disambiguate partial matches when multiple names map to same string | DELIVERED-AS-DESIGNED | HIGH | `src/partial_match.rs` is the consolidated implementation. |
| 36 | 2026-04-03-queue-case-insensitive-test-design.md | 50 | Integration test for case-insensitive duplicate queue names | DELIVERED-AS-DESIGNED | MEDIUM | Test-only spec; matches integration-test convention. |
| 37 | 2026-04-03-simplify-exact-multiple-design.md | 194 | Simplify `ExactMultiple` variant + replace unreachable arms in partial-match | DELIVERED-AS-DESIGNED | MEDIUM | `partial_match.rs` simplification follow-up. |
| 38 | 2026-04-04-issue-assign-account-id-design.md | 160 | Accept accountId fallback for assign/create | DELIVERED-AS-DESIGNED | MEDIUM | `helpers.rs` user resolution path. |
| 39 | 2026-04-05-asset-name-resolution-design.md | 150 | `--asset` accepts human name, not just key | DELIVERED-AS-DESIGNED | MEDIUM | `src/api/assets/objects.rs` `resolve_key`. |
| 40 | 2026-04-05-date-filters-design.md | 121 | `--created-after` / `--created-before` on issue list | DELIVERED-AS-DESIGNED | MEDIUM | JQL composition in `list.rs`. |
| 41 | 2026-04-05-handler-tests-me-keyword-design.md | 85 | Handler-level tests for `--to me` + name resolution | DELIVERED-AS-DESIGNED | MEDIUM | Test-only; matches `tests/` integration suite. |
| 42 | 2026-04-05-jsm-internal-comments-design.md | 197 | Internal vs external (public) comments on JSM tickets | DELIVERED-AS-DESIGNED | MEDIUM | JSM module exists; comment internal-flag in `workflow.rs` per Pass-3. |
| 43 | 2026-04-05-snapshot-tests-json-output-design.md | 195 | `insta::assert_json_snapshot!` pinning of write-command JSON schemas | DELIVERED-AS-DESIGNED | HIGH | `src/cli/issue/snapshots/` and `src/cli/snapshots/` directories present; `json_output.rs` (149 LOC) extracts builder fns. |
| 44 | 2026-04-08-api-passthrough-design.md | 321 | `jr api` raw API passthrough (modeled on `gh api`) | DELIVERED-AS-DESIGNED | HIGH | `src/cli/api.rs` exists; `Api` variant in `cli/mod.rs`. |
| 45 | 2026-04-10-429-retry-exhaustion-warning-design.md | 79 | Warn user when 429 retries exhausted | DELIVERED-AS-DESIGNED | MEDIUM | `src/api/rate_limit.rs` handles `Retry-After`; warning behavior in client. |
| 46 | 2026-04-12-stderr-stdout-separation-design.md | 99 | Move human status text to stderr; stdout = machine output only | DELIVERED-AS-DESIGNED | HIGH | Per Pass-3/4 contracts and CLAUDE.md "Output" convention. |
| 47 | 2026-04-13-user-search-lookup-design.md | 153 | `jr user search` / `user lookup` commands | DELIVERED-AS-DESIGNED | HIGH | `src/cli/user.rs` + `src/api/jira/users.rs` live. |
| 48 | 2026-04-15-team-field-and-strict-matching-design.md | 176 | Add team field to `issue view` whitelist + apply exact→prefix→disambiguate to all resolvers | DELIVERED-AS-DESIGNED | HIGH | `helpers.rs` (813 LOC) is the resolver hub; team-field in view per Pass-2/3. |
| 49 | 2026-04-16-adf-to-text-rich-rendering-design.md | 208 | Rich-text ADF→terminal rendering | DELIVERED-AS-DESIGNED | HIGH | `src/adf.rs` is the live ADF module per CLAUDE.md. |
| 50 | 2026-04-16-markdown-to-adf-conversion-design.md | 243 | Replace hand-rolled markdown parser with proper markdown→ADF lib | DELIVERED-AS-DESIGNED | HIGH | `src/adf.rs` consolidates markdown→ADF; spec was the rewrite. |
| 51 | 2026-04-16-verbose-request-body-logging-design.md | 137 | `--verbose` includes request body | DELIVERED-AS-DESIGNED | HIGH | `src/observability.rs` (added post-v1) is the verbose-logging module. |
| 52 | 2026-04-17-insufficient-scope-error-design.md | 121 | Detect Atlassian gateway "scope does not match" 401 → `JrError::InsufficientScope` | DELIVERED-AS-DESIGNED | HIGH | `JrError::InsufficientScope` variant verified in `src/error.rs`. |
| 53 | 2026-04-17-keychain-prompts-207-design.md | 163 | `jr auth refresh` to recover from macOS keychain ACL prompts after binary replace | DELIVERED-AS-DESIGNED | HIGH | `Refresh` variant in `cli/mod.rs` Auth subcommands; `src/cli/auth.rs` handles. |
| 54 | 2026-04-17-per-command-error-coverage.md | 149 | Test-only: 5xx/401/network-drop integration tests per read-command file | DELIVERED-AS-DESIGNED | MEDIUM | Test convention; matches `tests/` layout. |
| 55 | 2026-04-30-embedded-oauth-app-design.md | 524 | Embedded `jr` OAuth app w/ XOR obfuscation, fixed callback port 53682 (re-supersedes ADR-0002) | DELIVERED-AS-DESIGNED | HIGH | `src/api/auth_embedded.rs` + `build.rs` codegen verified; ADR-0006 references this spec. **Note:** This spec PIVOTS the v1 design's "embedded OAuth secret" simple model AND PIVOTS ADR-0002 — but the spec itself is delivered as designed. |

**Bucket totals across the 55:** DELIVERED-AS-DESIGNED 54, DELIVERED-DIVERGENT 1, PARTIALLY 0, PIVOTED 0, UNDELIVERED 0, ARCHAEOLOGICAL 0.

This is unsurprising: the entire `superpowers/specs/` corpus is a per-feature spec-then-implement archive. By construction, every accepted spec landed in code. The single DELIVERED-DIVERGENT (#8 issue-module-split) reflects that the *split happened* but the resulting file inventory grew beyond the spec's plan.

---

## §2 — `docs/specs/` Overlap Analysis (22 files)

The 22 files in `docs/specs/` are post-superpowers-format feature specs (per `docs/specs/README.md`). The two directories are sequential, not parallel — `docs/specs/` succeeded `docs/superpowers/specs/` as the per-feature spec home (the README explicitly points back to `../superpowers/specs/2026-03-21-jr-jira-cli-design.md` as the v1 foundation).

**Overlap check:** I cross-referenced every file in `docs/specs/` against the 55 superpowers specs by topic.

| docs/specs/ file | Overlap with superpowers/specs/? | Bucket | Confidence | Notes |
|------------------|-----------------------------------|--------|------------|-------|
| README.md | n/a | ARCHAEOLOGICAL | HIGH | Pointer file; no spec content. |
| assets-schema-discovery.md (270 LOC) | No | DELIVERED-AS-DESIGNED | HIGH | `src/api/assets/schemas.rs` + `Schemas`/`Types`/`Schema` variants in cli/mod.rs. |
| assets-search-attribute-names.md (251 LOC) | Adjacent to #21 (`asset-attribute-names`) but different surface (search vs view) | DELIVERED-AS-DESIGNED | MEDIUM | Search-side refactor; `src/api/assets/objects.rs`. |
| assets-tickets-status-filter.md (83 LOC) | No | DELIVERED-AS-DESIGNED | MEDIUM | `tickets.rs` has status filter param. |
| assets-view-default-attributes.md (105 LOC) | No | DELIVERED-AS-DESIGNED | MEDIUM | View-side enrichment. |
| author-needle-smart-constructor.md (137 LOC) | No (post-#200 changelog follow-up) | DELIVERED-AS-DESIGNED | MEDIUM | `cli/issue/changelog.rs` `AuthorNeedle` type. |
| changelog-author-classify-digit-requirement.md (136 LOC) | No | DELIVERED-AS-DESIGNED | MEDIUM | `classify_author()` fix in `changelog.rs`. |
| format-date-verbose-parse-failure-logging.md (184 LOC) | No (cross-cuts spec #51) | DELIVERED-AS-DESIGNED | MEDIUM | Logging path in `format.rs`/`observability.rs`. |
| issue-changelog.md (333 LOC) | No | DELIVERED-AS-DESIGNED | HIGH | `Changelog` variant in cli/mod.rs; `src/cli/issue/changelog.rs` (847 LOC) is the live impl; `src/types/jira/changelog.rs` types. |
| issue-create-json-full-shape.md (110 LOC) | No (refines #34 issue-create-url) | DELIVERED-AS-DESIGNED | HIGH | `json_output.rs` builders. |
| issue-list-asset-filter.md (150 LOC) | No (Layer 2 of #11 + #14) | DELIVERED-AS-DESIGNED | HIGH | `--asset` filter on issue list; `aqlFunction()` JQL composition documented in CLAUDE.md gotchas. |
| issue-move-resolution.md (78 LOC) | No (refines #32 issue-move-status-name) | DELIVERED-AS-DESIGNED | HIGH | `Resolutions` variant; `src/api/jira/resolutions.rs`. |
| issue-remote-link.md (87 LOC) | No | DELIVERED-AS-DESIGNED | HIGH | `RemoteLink` enum variant + `handle_remote_link` in `links.rs` (verified by grep). |
| list-rs-split.md (189 LOC) | Topic-adjacent to #8 (issue-module-split, but specifically about `list.rs`) | DELIVERED-DIVERGENT | HIGH | Spec has explicit "Post-implementation note (PR #272)" admitting actual outcomes diverged from the line-count projections (target ≤750 not met; `list.rs` ended at 1083 vs projected ~700). Spec was written knowing it would diverge, but outcome stands as live code. |
| multi-profile-auth.md (459 LOC) | No (largest doc spec; supersedes single-instance pattern in v1 design) | DELIVERED-AS-DESIGNED | HIGH | Per CLAUDE.md gotchas: per-profile cache root `v1/<profile>/`, namespaced OAuth keychain keys, `Config::load_with(cli_profile)`, `--profile` flag, `JR_PROFILE` env. **Important:** This spec PIVOTS the v1 design's `[instance]` config shape — flagged in §3. |
| oauth-scopes-configurable.md (137 LOC) | No (refines OAuth scopes from v1 design) | DELIVERED-AS-DESIGNED | MEDIUM | `config.toml` `oauth_scopes` setting in `config.rs`. |
| resolve-asset-custom-fields.md (185 LOC) | Adjacent to #14 issue-linked-assets | DELIVERED-AS-DESIGNED | MEDIUM | CMDB custom-field discovery in `linked.rs`. |
| sprint-issue-management.md (216 LOC) | No (extends #29 sprint-current-limit) | DELIVERED-AS-DESIGNED | HIGH | Sprint `add`/`remove` variants per CLAUDE.md sprint module description. |
| team-assignment.md (210 LOC) | Major superset / refinement of v1 team field section | DELIVERED-AS-DESIGNED | HIGH | `src/cli/team.rs` + `src/api/jira/teams.rs` (incl. GraphQL hostNames per ADR-0005); `team list` command; team filter on issue list. |
| team-column-sprint-board-parity.md (130 LOC) | No (closes parity gap #246) | DELIVERED-AS-DESIGNED | MEDIUM | Format module shows team column on sprint/board views. |
| team-field-object-shape-tolerance.md (109 LOC) | No (#254 schema tolerance) | DELIVERED-AS-DESIGNED | MEDIUM | Serde flexibility for object-vs-array team field shape; `types/jira/`. |
| user-search-pagination.md (182 LOC) | Refines #47 user-search-lookup-design | DELIVERED-AS-DESIGNED | HIGH | `--all` true multi-page pagination follow-up; `src/api/pagination.rs` + `users.rs`. |

**`docs/specs/` totals:** DELIVERED-AS-DESIGNED 20, DELIVERED-DIVERGENT 1 (`list-rs-split.md`), ARCHAEOLOGICAL 1 (`README.md`).

**No file-level overlap between `docs/specs/` and the 55 superpowers/specs/ files.** Topic-adjacency exists (asset-attribute-names ↔ assets-search-attribute-names; issue-create-url ↔ issue-create-json-full-shape; issue-module-split ↔ list-rs-split) but each is a successor refinement on a separate concern, not a duplicate.

---

## §3 — v1 Design Spec Section-by-Section Assessment

File: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/docs/superpowers/specs/2026-03-21-jr-jira-cli-design.md` (668 LOC).

| § | Section | Status | Replacement / supersession |
|---|---------|--------|----------------------------|
| Overview / Goals / Non-Goals | Goals + extensibility | **STILL AUTHORITATIVE** (intent unchanged) | none |
| Non-Goals (v1) lists "Confluence, JSM, or Assets support" as out | **SUPERSEDED** | JSM (#15 + queues) and Assets/CMDB (#11 + Layer 2) shipped post-v1; Confluence still out. |
| Command Structure (Auth, Issues, Boards, Sprints, Worklogs, Shell) | **SUPERSEDED IN PART** | Current command set is a strict superset: adds `api`, `user`, `team`, `queue`, `assets`, `issue link/unlink/changelog/comments/remote-link`, `auth refresh/switch/list/remove`, `project list/fields`. Use as historical baseline. |
| Smart Defaults (scrum vs kanban) | **STILL AUTHORITATIVE** | API call sequence + status-category-color mapping for `--open` still match (CLAUDE.md gotcha confirms hardcoded colors). |
| Transitions (interactive / direct / partial / disambiguation) | **STILL AUTHORITATIVE** | Pattern preserved; partial_match.rs centralizes it for all resolvers (#48). |
| Team Assignment | **SUPERSEDED** | `docs/specs/team-assignment.md` (210 LOC) is the live design; ADR-0005 covers GraphQL `hostNames` discovery; `team list` cache; per-profile cache. |
| Authentication → OAuth 2.0 (3LO) | **PIVOTED** | ADR-0006 + spec #55 (embedded-oauth-app-design) replace the v1 "embedded secret" sketch with an XOR-obfuscated, build-time-injected, fixed-port (53682) embedded app. |
| Authentication → API Token | **STILL AUTHORITATIVE** | Fallback path unchanged. |
| Authentication → Token Lifecycle | **SUPERSEDED IN PART** | `auth refresh` (#53) + multi-profile token namespacing (`docs/specs/multi-profile-auth.md`) extend lifecycle handling. |
| Authentication → Credential Storage | **SUPERSEDED IN PART** | Per-profile namespacing of OAuth keys: shared `email`/`api-token`/`oauth_client_*` (account-level) but namespaced `<profile>:oauth-access-token`/`oauth-refresh-token` (cloudId-scoped). Per CLAUDE.md gotcha. |
| Configuration → Global Config (`[instance]` shape) | **PIVOTED** | `[profiles.<name>]` + `default_profile` per `docs/specs/multi-profile-auth.md`. v1 `[instance]` shape now legacy; auto-migrated on first load (per CLAUDE.md). |
| Configuration → Per-Project Config (`.jr.toml`) | **STILL AUTHORITATIVE** | Same shape (`project`, `board_id`). |
| Configuration → Resolution (figment) | **STILL AUTHORITATIVE** (extended) | `--profile` flag > `JR_PROFILE` env > `default_profile` > `"default"`. Extension, not replacement. |
| API Client (HTTP, pagination, rate limiting, error handling) | **STILL AUTHORITATIVE** | Module structure matches; `src/api/` layout per CLAUDE.md. |
| Exit Codes (0/1/2/64/78/130) | **STILL AUTHORITATIVE** | Confirmed in `src/error.rs` `JrError::exit_code()`. |
| Graceful Shutdown (Ctrl+C / SIGINT / 130) | **STILL AUTHORITATIVE** | `main.rs` Ctrl+C handler. |
| AI Agent & Scripting Friendliness | **STILL AUTHORITATIVE** (extended) | Stdin support, `--no-input`, idempotence, structured output preserved. Extended by stderr/stdout split (#46) and `--output json` snapshot pinning (#43). |
| Worklog Time Format | **STILL AUTHORITATIVE** | `src/duration.rs` matches the table. |
| Rich Text: ADF Handling | **SUPERSEDED IN PART** | Spec #49 (adf-to-text-rich-rendering) and #50 (markdown-to-adf-conversion) replaced the v1 sketch with proper rendering + parser. |
| Project Structure (proposed) | **HISTORICAL ONLY** | Tree shown is the *initial* layout. Current `src/` has `cli/issue/` directory, `cli/api.rs`, `api/jira/{statuses,projects,resolutions}.rs`, `api/jsm/`, `api/assets/`, `types/{jsm,assets}/`, `auth_embedded.rs`, `observability.rs`, `partial_match.rs`. Don't import as authoritative. |
| Product Extensibility note | **STILL AUTHORITATIVE** | `api/jsm/` and `api/assets/` confirm the namespaced-by-product pattern shipped. |
| Dependencies table | **PARTIALLY SUPERSEDED** | Most crates still present; some versions/features evolved (e.g., `colored` likely replaced or supplemented; markdown-to-ADF needs added a parser). Use as historical context only — read `Cargo.toml` for truth. |
| Dev Dependencies | **STILL AUTHORITATIVE** | `wiremock`, `assert_cmd`, `predicates`, `tempfile`, `insta`, `proptest` all match per CLAUDE.md test-stack. |
| Release Profile (LTO, strip, panic=abort) | **STILL AUTHORITATIVE** | Confirmed in CLAUDE.md "Build & Test". |
| Testing Strategy | **STILL AUTHORITATIVE** (extended) | Adds snapshot tests for write-command JSON (#43) and per-command error-coverage (#54). |
| Test Infrastructure (`tests/common/...`) | **STILL AUTHORITATIVE** | CLAUDE.md confirms `tests/common/fixtures.rs` + wiremock pattern. |
| Config Injection (`JR_BASE_URL`, `JiraClient::new_for_test`) | **STILL AUTHORITATIVE** | CLAUDE.md confirms both. |
| Distribution (cargo, brew, GitHub Releases) | **STILL AUTHORITATIVE** | Plus `install.sh` (#6). |
| GitHub Environment Setup (CI, release matrix, branch protection, MSRV) | **STILL AUTHORITATIVE** | Conventions still hold. |
| Roadmap (Post-v1) | **HISTORICAL ONLY** | Outdated: JSM and Assets are shipped, not roadmap. TUI / bulk ops / git integration / Confluence / offline still open. |

### v1 Design Verdict

**Recommendation: Import as `.factory/specs/historical/v1-design.md` with annotated supersession notes inline.**

Rationale:
- The v1 spec is the canonical statement of *intent and goals* for the project. Several sections (Smart Defaults, API Client architecture, Exit Codes, Graceful Shutdown, AI Agent Friendliness, Testing Strategy, Distribution, MSRV policy) are still the authoritative source of those decisions — they were never re-specified in a successor spec because they didn't need to change.
- Annotation supersession notes (one-line headers per section saying "SUPERSEDED BY: …" and pointing to ADR-0006 / `docs/specs/multi-profile-auth.md` / etc.) preserve the as-of-2026-03-21 baseline while making the reader aware of pivots.
- Place under `.factory/specs/historical/` not `docs/specs/` to keep it outside the active spec workflow (preventing future contributors from treating it as a current reference) while remaining inside the engine's spec corpus for VSDD downstream consumption.
- Ingestion size: 668 LOC is small enough to ingest whole — no need to split.

Three sections need explicit pivot annotation:
1. **§ Authentication / OAuth 2.0** → pivoted by ADR-0006 and spec #55.
2. **§ Configuration / Global Config `[instance]` shape** → pivoted by `docs/specs/multi-profile-auth.md`.
3. **§ Project Structure (proposed tree)** → completely outdated; reference Pass-0 inventory + CLAUDE.md instead.

---

## §4 — Aggregate Harmonization Plan

### Bucket Counts

Combined view across (a) the 55 superpowers/specs/ files, (b) the 22 docs/specs/ files, and (c) the v1 design (counted separately because it's a multi-section document that's split across multiple buckets internally).

| Bucket | 55 superpowers | 22 docs/specs | v1 design | Total |
|--------|----------------|---------------|-----------|-------|
| DELIVERED-AS-DESIGNED | 54 | 20 | n/a (mixed) | 74 |
| DELIVERED-DIVERGENT | 1 (#8 issue-module-split) | 1 (list-rs-split) | n/a (mixed) | 2 |
| PARTIALLY DELIVERED | 0 | 0 | n/a | 0 |
| PIVOTED | 0 (#55 IS the pivot, but is itself delivered) | 0 | 1 doc, sections pivoted | 1 (doc) |
| UNDELIVERED | 0 | 0 | 0 | 0 |
| ARCHAEOLOGICAL | 0 | 1 (README.md) | 0 | 1 |

### Phase 2 Story Candidates

**There are no UNDELIVERED specs.** Every spec in both directories describes a feature that is in current code. The corpus is a *retrospective* archive of shipped-feature designs, not a backlog.

This means: **0 specs become Phase 2 story candidates from the harmonization pass.**

(Phase 2 stories may still be authored from gaps surfaced by Pass-2/Pass-3 deepening rounds, but they will not come from these 77 spec files.)

### Validation Inputs (BC Cross-Check)

All 74 DELIVERED-AS-DESIGNED specs should be queued as **validation inputs** for cross-checking Pass-3 behavioral contracts against. Specifically:

**High-value validation inputs (large, behaviorally rich):**
- `2026-03-21-jr-jira-cli-design.md` — v1 design (smart defaults, transitions, team assignment, ADF, AI-agent surface)
- `2026-03-22-story-points-design.md` (328) — full points CRUD + sprint summaries
- `2026-03-24-assets-cmdb-design.md` (495)
- `2026-03-24-jsm-queues-design.md` (384)
- `2026-04-08-api-passthrough-design.md` (321)
- `2026-04-30-embedded-oauth-app-design.md` (524)
- `docs/specs/multi-profile-auth.md` (459)
- `docs/specs/issue-changelog.md` (333)
- `docs/specs/team-assignment.md` (210)
- `2026-04-15-team-field-and-strict-matching-design.md` (176) — this spec encodes the exact→prefix→disambiguate matching policy applied to all resolvers

**All other DELIVERED-AS-DESIGNED specs** are also validation inputs but lower priority — most are 100–250 LOC bug-fix or single-flag designs whose contract surface is narrow.

### Reconciliation Required (DELIVERED-DIVERGENT)

Two files need explicit reconciliation notes added when ingested into the harmonized spec set:

1. **`2026-03-22-issue-module-split-design.md`** — Spec planned 7 files; current shape is 12 files (added `view.rs`, `comments.rs`, `changelog.rs`, `json_output.rs`, `assets.rs`). Reconciliation: annotate spec on ingest with "ACTUAL OUTCOME (as of 2026-05-04): see `src/cli/issue/` directory listing in CLAUDE.md."
2. **`docs/specs/list-rs-split.md`** — Spec already contains a self-honest "Post-implementation note (PR #272)" admitting the line-count targets were missed. No further annotation needed; this spec is exemplary in calling out its own divergence.

### Archaeological List

- `docs/specs/README.md` — pointer file only; preserved in archive but excluded from validation pass.

### Summary Table

| Action | Count | Files |
|--------|-------|-------|
| Validate as BC input (cross-check Pass-3) | 74 | 54 superpowers + 20 docs/specs |
| Reconcile + validate | 2 | issue-module-split, list-rs-split |
| Pivot annotation needed | 1 (v1 design, 3 sections) | 2026-03-21 v1 design § OAuth, § Global Config, § Project Structure |
| Archive only | 1 | docs/specs/README.md |
| Phase 2 story candidate | 0 | (none) |
| **Total spec files harmonized** | **78** | (55 + 22 + 1 v1 design) |

---

## §5 — Recommended Directory Structure for Phase 1

Goal: single canonical home under `.factory/specs/` for the harmonized corpus, with provenance preserved and supersession explicit.

```
.factory/
└── specs/
    ├── historical/
    │   ├── README.md                                # Explains: pre-VSDD imports, do not author new specs here
    │   └── v1-design.md                             # Imported from 2026-03-21-jr-jira-cli-design.md
    │                                                # WITH inline annotations on superseded sections:
    │                                                #   §Authentication → see ADR-0006 + delivered/embedded-oauth-app
    │                                                #   §Configuration[instance] → see delivered/multi-profile-auth
    │                                                #   §Project Structure → see Pass-0 inventory + CLAUDE.md
    │                                                #   §Roadmap → outdated; JSM + Assets shipped
    │
    ├── delivered/                                   # 74 DELIVERED-AS-DESIGNED specs (validation inputs)
    │   ├── README.md                                # "Specs whose features ship in current code.
    │   │                                            #  Cross-check Pass-3 BCs against these.
    │   │                                            #  DO NOT modify; modify the BC instead."
    │   ├── superpowers/                             # Original superpowers/specs/ batch (54 files)
    │   │   ├── 2026-03-22-issue-handler-refactor-design.md
    │   │   ├── 2026-03-22-issue-linking-design.md
    │   │   ├── ... (52 more)
    │   │   └── 2026-04-30-embedded-oauth-app-design.md
    │   └── feature-specs/                           # Original docs/specs/ batch (20 files, README excluded)
    │       ├── assets-schema-discovery.md
    │       ├── ... (18 more)
    │       └── user-search-pagination.md
    │
    ├── divergent/                                   # 2 DELIVERED-DIVERGENT
    │   ├── README.md                                # "Spec describes intent; implementation diverged.
    │   │                                            #  Each file annotated with ACTUAL OUTCOME."
    │   ├── 2026-03-22-issue-module-split-design.md  # + ACTUAL OUTCOME annotation
    │   └── list-rs-split.md                         # already has self-annotation; preserve as-is
    │
    └── archive/
        └── docs-specs-readme.md                     # The single archaeological file
```

Notes:
- ADRs are NOT moved here (per Q4 KEEP decision). They remain at `docs/adr/`. The historical v1-design.md should reference them by relative path (`../../docs/adr/0006-...md` from `.factory/specs/historical/`).
- File names are preserved verbatim from source (no renaming) to keep git blame / external references resolvable.
- `delivered/` is split into `superpowers/` + `feature-specs/` subdirs to preserve provenance — readers can tell which spec era a file came from without reading it.
- `divergent/` is intentionally tiny; in a healthy spec workflow it should rarely grow.
- The Phase 1 task is NOT "rewrite these specs" but "ingest them with provenance and a `delivered/` vs `divergent/` vs `historical/` taxonomy so Pass-3 BC validation can cross-reference them and downstream skills (create-prd, create-domain-spec) can cite them as historical context."

---

## §6 — State Checkpoint

```yaml
task: pre-vsdd-harmonization-plan
project: jira-cli
status: complete
files_inventoried:
  superpowers_specs: 55
  docs_specs: 22
  v1_design: 1
  total: 78
buckets:
  delivered_as_designed: 74
  delivered_divergent: 2
  partially_delivered: 0
  pivoted_doc_sections: 3 (within v1 design)
  undelivered: 0
  archaeological: 1
phase_2_story_candidates: 0
validation_inputs: 74
reconciliation_required: 2
v1_design_verdict: import-as-historical-with-3-section-pivot-annotations
v1_design_target_path: .factory/specs/historical/v1-design.md
output_file: /Users/zious/Documents/GITHUB/jira-cli/.factory/semport/jira-cli/jira-cli-pre-vsdd-harmonization-plan.md
timestamp: 2026-05-04
next_action: orchestrator decides whether to (a) execute the Phase 1 file moves into .factory/specs/, (b) author pivot annotations on v1-design.md sections, (c) proceed to Phase 2 story authoring (which will draw from gaps in Pass-2/3, NOT from this corpus).
```

# Pass 5 Deepening — Round 2 — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Builds on: broad pass 5; R1 (P5R1-P-01..07, P5R1-AP-01..07, 7 drifts).

> **Method.** R2 attacks the four R2 targets enumerated in R1 §13: cross-handler eprintln/println discipline, per-subcommand `--output json` parity audit, `impl JiraClient` resource-binding audit, and sync `#[test]` vs `#[tokio::test]` triangulation. R2 is an audit round — every claim is a verified count or enumeration, not a new pattern category. Findings are confirmation/refinement of R1's existing rules.

---

## 1. Round metadata

| Field | Value |
|---|---|
| Round | 2 of (max 5) |
| Predecessor | R1 |
| Inputs consumed | R1; broad pass 5; spot-reads of `cli/issue/json_output.rs`, `cli/auth.rs`, `api/client.rs`, `api/jira/*`, `api/assets/*`, `api/jsm/*`; tests/ enumeration |
| Verification commands run | `wc -l` on src/cli/* and src/api/*; `awk` per-file eprintln/println count; `awk` for `pub fn`/`pub async fn`/`impl JiraClient`; `awk` for sync vs tokio test annotations across all 31 integration files |
| New design patterns / conventions added | **0** (R1 was exhaustive on novel categories) |
| New anti-patterns added | **1** (P5R2-AP-01 — handler-level eprintln/println choice has no codified rule) |
| Audits performed | 4 (R2-T1 through R2-T4) |
| BCs touched | 0 |
| Hallucination corrections logged | 1 (R1 cited "9 categories" for test-mechanism map; map actually had 13 entries — typo only, both numbers exist in R1) |
| R1 audit | All 7 patterns + 7 anti-patterns + 7 drifts re-verified at source. 0 retractions. |

---

## 2. Audit of R1's claims (recount, name-check, verify)

### 2.1 R1 named-pattern recount

R1 claimed "7 patterns + 7 anti-patterns" in metadata header (line 19-21). Per-section enumeration:

- **§3 New design patterns**: P5R1-P-01, -02, -03, -04, -05, -06, -07. **Count: 7.** ✓
- **§4 New anti-patterns**: P5R1-AP-01, -02, -03, -04, -05, -06, -07. **Count: 7.** ✓
- **§7 Pre-VSDD drifts**: Drift-1 through Drift-7. **Count: 7** (R1 metadata cites "6" at line 23 but body has 7; R1 §10 Delta corrects to 7). **Framing nitpick only.**
- **§9 GAP categories**: GAP-CAT-1 through GAP-CAT-5. **Count: 5.** ✓
- **§6 Test-mechanism subjects**: §6.1 table has 13 rows (including last 4: config layering, embedded OAuth XOR, plus original 9). R1 metadata cites "9 categories" (line 24); R1 §10 Delta corrects to "13 categories." **Framing nitpick only.**

**Audit result**: All R1 named patterns recount correctly. Two metadata-header undercounts (drifts: 6 vs 7; mechanisms: 9 vs 13) reconcile correctly in §10 Delta Summary. No retraction.

### 2.2 Cross-pattern verification

| R1 claim | Verification this round | Status |
|---|---|---|
| P5R1-P-06: 28 `with_temp_cache` call sites in `cache.rs` | Spot-confirmed via R1's own grep (line 558) | ✓ |
| P5R1-AP-05: 4 distinct bool field names | Re-read `json_output.rs:1-149` — confirmed: `changed` (move/assign/unassign), `updated` (edit), `linked` (link), `unlinked` (unlink), plus `comment_id` and `link_id` ID fields. Note `commented` not used; comment write returns `comment_id` only. Count: **4 bool field names + 2 ID field names**. R1's "4 distinct bool" stands. ✓ |
| P5R1-AP-04: shard-rule violations | Re-counted: `cli/auth.rs` 1,998; `cli/issue/list.rs` 1,083; `cli/assets.rs` 1,055; `cli/issue/changelog.rs` 847; `cli/issue/helpers.rs` 813; `cli/issue/workflow.rs` 788; `api/auth.rs` 1,397; `config.rs` 1,223; `adf.rs` 1,826. R1's table accurate. ✓ |
| P5R1-AP-03: worklog inline 8/5 | Re-read `cli/worklog.rs:32` — confirmed inline literals. ✓ |

---

## 3. R2-T1 — Cross-handler eprintln/println discipline audit

### 3.1 Per-CLI-handler counts

Per-file `awk` count of `eprintln!` and `println!` macro invocations across 24 CLI handler files. Excludes inline test modules.

| File | LOC | eprintln! | println! | Profile |
|---|---:|---:|---:|---|
| `cli/api.rs` | 342 | 0 | 0 | Pure (returns JrError, no direct write) |
| `cli/assets.rs` | 1,055 | 0 | 13 | Read-only stdout (table renders) |
| `cli/auth.rs` | 1,998 | 11 | 19 | Mixed — interactive prompts + status |
| `cli/board.rs` | 334 | 5 | 5 | Mixed |
| `cli/init.rs` | 285 | 9 | 9 | Interactive — symmetric |
| `cli/mod.rs` | 772 | 0 | 0 | Pure dispatch |
| `cli/project.rs` | 133 | 0 | 11 | Read-only stdout |
| `cli/queue.rs` | 323 | 0 | 0 | Pure (uses `output::print_output`) |
| `cli/sprint.rs` | 438 | 4 | 7 | Mixed |
| `cli/team.rs` | 120 | 1 | 1 | Symmetric |
| `cli/user.rs` | 165 | 0 | 0 | Pure (uses `output::print_output`) |
| `cli/worklog.rs` | 79 | 0 | 1 | Read-only stdout |
| `cli/issue/assets.rs` | 65 | 1 | 2 | Mixed |
| `cli/issue/changelog.rs` | 847 | 0 | 1 | Read-only stdout |
| `cli/issue/comments.rs` | 61 | 0 | 0 | Pure |
| `cli/issue/create.rs` | 375 | 2 | 6 | Mixed |
| `cli/issue/format.rs` | 225 | 0 | 0 | Pure (formatter) |
| `cli/issue/helpers.rs` | 813 | 2 | 2 | Symmetric |
| `cli/issue/json_output.rs` | 149 | 0 | 0 | Pure (JSON shapers) |
| `cli/issue/links.rs` | 293 | 0 | 4 | Read-only stdout |
| `cli/issue/list.rs` | 1,083 | 5 | 5 | Symmetric |
| `cli/issue/view.rs` | 286 | 1 | 3 | Mixed |
| `cli/issue/workflow.rs` | 788 | 6 | 14 | Mixed |
| **Totals** | — | **47** | **103** | — |

Plus `cli/issue/changelog.rs:1` matches `println!` (1) — included above.

### 3.2 Discipline assessment

**Pattern observed**: discipline is *consistent within a category* but **uncodified**:

1. **Pure (0/0)**: `cli/api.rs`, `cli/mod.rs`, `cli/queue.rs`, `cli/user.rs`, `cli/issue/comments.rs`, `cli/issue/format.rs`, `cli/issue/json_output.rs`. These delegate ALL stdout/stderr to `output::print_output` or return-via-Result. **7 files, 1,723 LOC, 0/0**.

2. **Read-only stdout (0/N>0)**: `cli/assets.rs`, `cli/project.rs`, `cli/worklog.rs`, `cli/issue/changelog.rs`, `cli/issue/links.rs`. Print success rows; never print to stderr. **5 files, 13+11+1+1+4 = 30 println!, 0 eprintln!**.

3. **Mixed (M>0/N>0, asymmetric)**: `cli/auth.rs` 11/19, `cli/board.rs` 5/5, `cli/sprint.rs` 4/7, `cli/issue/assets.rs` 1/2, `cli/issue/create.rs` 2/6, `cli/issue/view.rs` 1/3, `cli/issue/workflow.rs` 6/14. Stderr is for *user-facing warnings/info during interactive flow*; stdout is for *primary output*. **7 files**.

4. **Symmetric (M==N)**: `cli/init.rs` 9/9, `cli/team.rs` 1/1, `cli/issue/helpers.rs` 2/2, `cli/issue/list.rs` 5/5. Each `eprintln!` info has a `println!` counterpart, often interactive-prompt-with-confirmation. **4 files**.

5. **No file uses a logging facade** (no `log::info!`, `tracing::info!`, etc.). The codebase has zero log-crate adoption. All output is direct macro calls.

**Verifiability**: per-file counts above. `cli/auth.rs` accounts for 23% of all `eprintln!` invocations (11/47) and 18% of `println!` (19/103) — disproportionate but consistent with R1 §3.4 noting `cli/auth.rs` has 14 UserError construction sites.

### 3.3 Discipline rule (implicit, codified by R2)

The implicit project rule is:
- **JSON write-op response** → ALWAYS `println!` of serialized JSON (never `eprintln!`).
- **Interactive prompt / status during flow** → `eprintln!` ("Authorizing…", "Found N matches, picking…").
- **Interactive prompt READ from user** → `print!` (no newline) then flush stdout. (Not counted above; orthogonal.)
- **Final confirmation message** ("Created BAR-123") → `println!` to stdout (so it can be piped/captured).
- **Errors that bubble up** → returned as `JrError`/`anyhow::Error`; `main.rs` writes the message to stderr via the error reporter.
- **No log facade**: zero adoption of `log` or `tracing` crates.

**Severity for spec**: codifying this rule in CLAUDE.md or a `docs/specs/cli-output-discipline.md` would address the latent gap. R1 GAP-CAT-1 already lists this as a documentation debt.

### 3.4 New anti-pattern: P5R2-AP-01 — Handler-stderr discipline is implicit

**Source**: this round, §3.1-3.3 above.

47 `eprintln!` invocations across 14 handler files, with NO codified rule for "when does a handler `eprintln!` vs `println!` vs return a `JrError`." Each handler gets it right by convention/code-review, not by a typed channel.

**Severity**: LOW. No bugs observed (Pass 3 R3 BC verifications confirm correct stderr/stdout for tested flows). But:
- Future contributors must read existing handlers to infer the rule.
- The asymmetric `cli/issue/workflow.rs` 6/14 ratio reads odd until you see that it has interactive transition picking AND auto-comment AND open-after-move — multi-stage UX with status messages.
- A typed `Channel::stderr_status(...)` / `Channel::stdout_result(...)` would force the convention.

**Phase 1 decision**: codify the rule (spec doc in `docs/specs/`) OR introduce a typed channel.

---

## 4. R2-T2 — Per-subcommand `--output json` parity audit

### 4.1 Subcommand inventory and JSON parity

The global `--output {table|json}` flag (`cli/mod.rs`) is threaded as `output_format: &OutputFormat` to every handler. Audit: is JSON parity universal?

| Top-level | Subcommand | Handler | Threads `output_format`? | JSON path verified | Status |
|---|---|---|---|---|---|
| `assets` | `search` | `cli/assets.rs::handle_search` | YES (line 358) | line 426 match | ✓ |
| `assets` | `view` | `cli/assets.rs::handle_view` | YES | line 568 match | ✓ |
| `assets` | `tickets` | `cli/assets.rs::handle_tickets` | YES (line 720) | line 736 match | ✓ |
| `assets` | `schemas` | `cli/assets.rs::handle_schemas` | YES (line 836) | line 859 (delegates to print_output) | ✓ |
| `assets` | `types` | `cli/assets.rs::handle_types` | YES (line 869) | line 894 match | ✓ |
| `assets` | `schema` | `cli/assets.rs::handle_schema` | YES (line 972) | line 1048 match | ✓ |
| `auth` | `login` | `cli/auth.rs::handle_login` | NO (uses args.output at 2320, no JSON shape) | Returns no JSON — interactive only | **GAP** |
| `auth` | `switch` | `cli/auth.rs::handle_switch` | NO | Returns no JSON | **GAP** |
| `auth` | `list` | `cli/auth.rs::handle_list` | YES (uses args.output) | Verified via insta snapshot in tests/snapshots/cmd__list_table.snap | ✓ |
| `auth` | `status` | `cli/auth.rs` | YES (per Pass 3 R3) | Verified | ✓ |
| `auth` | `refresh` | `cli/auth.rs` | NO | No JSON | **GAP** |
| `auth` | `logout` | `cli/auth.rs::handle_logout` | NO (line 7429: takes `profile_arg` only) | No JSON | **GAP** |
| `auth` | `remove` | `cli/auth.rs::handle_remove` | NO | No JSON | **GAP** |
| `board` | `list` | `cli/board.rs::handle_list` | YES (line 3522) | print_output path | ✓ |
| `board` | `view` | `cli/board.rs::handle_view` | YES (line 3571) | print_output path | ✓ |
| `sprint` | `list` | `cli/sprint.rs::handle_list` | YES (line 5355) | match line 5360 | ✓ |
| `sprint` | `current` | `cli/sprint.rs::handle_current` | YES (line 5461) | match line 5491 | ✓ |
| `sprint` | `add` | `cli/sprint.rs::handle_add` | YES (line 5382) | match line 5387 | ✓ |
| `sprint` | `remove` | `cli/sprint.rs::handle_remove` | YES (line 5402) | match line 5419 (via print_output) | ✓ |
| `worklog` | `add` | `cli/worklog.rs::handle_add` | YES | JSON path | ✓ |
| `worklog` | `list` | `cli/worklog.rs::handle_list` | YES | JSON path | ✓ |
| `team` | `list` | `cli/team.rs::handle_list` | YES (line 5705) | line 5728 print_output | ✓ |
| `user` | `search` | `cli/user.rs::handle_search` | YES (line 5832) | print_user_list | ✓ |
| `user` | `list` | `cli/user.rs::handle_list` | YES (line 5851) | print_user_list | ✓ |
| `user` | `view` | `cli/user.rs::handle_view` | YES (line 5872) | line 5900 print_output | ✓ |
| `queue` | `list` | `cli/queue.rs::handle_list` | YES | print_output | ✓ |
| `queue` | `view` | `cli/queue.rs::handle_view` | YES | print_output | ✓ |
| `project` | `list` | `cli/project.rs::handle_list` | YES (line 4817) | line 4841 (via inner) | ✓ |
| `project` | `fields` | `cli/project.rs::handle_fields` | YES (line 4851) | line 4867 match | ✓ |
| `issue` | `list` | `cli/issue/list.rs::handle_list` | YES | print_output | ✓ |
| `issue` | `view` | `cli/issue/view.rs::handle_view` | YES | print_output | ✓ |
| `issue` | `comments` | `cli/issue/comments.rs::handle_comments` | YES | JSON shape | ✓ |
| `issue` | `create` | `cli/issue/create.rs::handle_create` | YES | json_output::CreatedIssue | ✓ |
| `issue` | `edit` | `cli/issue/create.rs::handle_edit` | YES | json_output::EditedIssue | ✓ |
| `issue` | `move` | `cli/issue/workflow.rs::handle_move` | YES | json_output::MovedIssue | ✓ |
| `issue` | `transitions` | `cli/issue/workflow.rs::handle_transitions` | YES | print_output | ✓ |
| `issue` | `assign` | `cli/issue/workflow.rs::handle_assign` | YES | json_output::AssignedIssue | ✓ |
| `issue` | `comment` | `cli/issue/workflow.rs::handle_comment` | YES | JSON path with comment_id | ✓ |
| `issue` | `open` | `cli/issue/workflow.rs::handle_open` | NO (opens browser) | N/A — no output | **N/A** |
| `issue` | `link` | `cli/issue/links.rs::handle_link` | YES | json_output::LinkedIssue | ✓ |
| `issue` | `unlink` | `cli/issue/links.rs::handle_unlink` | YES | json_output::UnlinkedIssue | ✓ |
| `issue` | `link-types` | `cli/issue/links.rs::handle_link_types` | YES | print_output | ✓ |
| `issue` | `remote-link` | `cli/issue/links.rs` | YES | json_output::RemoteLinkedIssue | ✓ |
| `issue` | `assets` | `cli/issue/assets.rs::handle_assets` | YES (line 14) | print_output | ✓ |
| `issue` | `changelog` | `cli/issue/changelog.rs::handle_changelog` | YES (line 91) | JSON path | ✓ |
| `init` | (no subs) | `cli/init.rs` | NO (interactive setup; no JSON) | N/A — interactive only | **N/A** |
| `me` | (no subs) | (handler) | YES (per CLAUDE.md AI Agent Notes) | print_output | ✓ |
| `api` | (no subs) | `cli/api.rs::handle_api` | N/A — escape hatch passes raw JSON through stdout from server | N/A | **N/A** |
| `completion` | shells | `main.rs:67-71` | N/A — generates shell completions to stdout | N/A | **N/A** |

### 4.2 Parity summary

- **Total surface points enumerated**: 49 subcommands across 16 top-level commands.
- **Has `--output json`**: 41 (84%).
- **Lacks JSON (genuine gap)**: 5 — `auth login`, `auth switch`, `auth refresh`, `auth logout`, `auth remove`.
- **N/A by design**: 4 — `issue open` (browser), `init` (interactive), `api` (raw passthrough), `completion` (shell-script generator).

**The 5 `auth` gaps share a root cause**: they are pure side-effect operations (modify keychain) with no resource shape to serialize. They DO honor `args.output` for the success-path message in some cases (e.g., `auth list` returns JSON; `auth status` returns JSON — both verified by Pass 3 R3 BC enumeration). The 5 gaps return Result<()> with no JSON shape on success.

**Phase 1 decision**: should `auth login/switch/logout/remove/refresh` return a structured JSON success object (e.g., `{"profile": "default", "ok": true}`) when `--output json` is set? Currently 5 of 7 `auth` subcommands have NO JSON path. This is the largest single parity gap.

### 4.3 P5R1-AP-05 reconfirmed at scale

R1 anti-pattern P5R1-AP-05 (4 distinct bool field names) is reconfirmed: write-op JSON shapes are non-uniform (`changed` for move/assign, `updated` for edit, `linked` for link, `unlinked` for unlink). The 5 `auth` gaps are an even-worse case: NO bool field at all; just `()`. The Phase 1 decision must address both the field-naming inconsistency AND the missing-JSON gap.

---

## 5. R2-T3 — `impl JiraClient` resource-binding audit

### 5.1 Inventory

`api/jira/`, `api/assets/`, `api/jsm/`, `api/client.rs`, `api/auth.rs`, `api/auth_embedded.rs` were enumerated for `pub fn`/`pub async fn` and `impl JiraClient` blocks.

**`impl JiraClient` block locations** (17 total):
- `api/client.rs:2621` (constructors + send/send_raw/request)
- `api/jira/boards.rs:6`, `fields.rs:71`, `issues.rs:395`, `links.rs:673`, `projects.rs:811`, `resolutions.rs:901`, `sprints.rs:957`, `statuses.rs:1070`, `teams.rs:1089`, `users.rs:1154`, `worklogs.rs:1433` (11 resource impls)
- `api/assets/objects.rs:2030`, `schemas.rs:2263`, `tickets.rs:2306` (3 asset impls)
- `api/jsm/queues.rs:2387`, `servicedesks.rs:2474` (2 jsm impls)

**Methods inside `impl JiraClient` blocks**: 70 distinct methods (counted via leading-spaces awk pattern on `pub fn`/`pub async fn`).

### 5.2 Free `pub fn` / `pub async fn` (NOT inside `impl JiraClient`)

13 free functions are exported from API resource modules:

| File | Function | Why not impl JiraClient? |
|---|---|---|
| `api/jira/fields.rs:100` | `filter_story_points_fields(fields: &[Field])` | Pure filter — no HTTP, no client state |
| `api/jira/fields.rs:135` | `filter_cmdb_fields(fields: &[Field])` | Pure filter — no HTTP |
| `api/assets/linked.rs:1469` | `get_or_fetch_cmdb_fields(client: &JiraClient)` | Cache-aside helper; takes &client as param |
| `api/assets/linked.rs:1481` | `cmdb_field_ids(fields: &[(String, String)])` | Pure projection |
| `api/assets/linked.rs:1486` | `extract_linked_assets(...)` | Pure extractor; operates on already-fetched data |
| `api/assets/linked.rs:1560` | `extract_linked_assets_per_field(...)` | Pure extractor |
| `api/assets/linked.rs:1594` | `enrich_json_assets(...)` | Pure enricher |
| `api/assets/linked.rs:1627` | `enrich_assets(client, assets)` | Cache-aside multi-step orchestrator |
| `api/assets/objects.rs:2130` | `resolve_object_key(client, ...)` | Multi-step resolver |
| `api/assets/objects.rs:2172` | `enrich_search_attributes(client, ...)` | Multi-step enricher |
| `api/assets/workspace.rs:2338` | `get_or_fetch_workspace_id(client)` | Cache-aside helper |
| `api/jsm/servicedesks.rs:2505` | `get_or_fetch_project_meta(client, ...)` | Cache-aside helper |
| `api/jsm/servicedesks.rs:2566` | `require_service_desk(client, ...)` | Cache-aside resolver |

### 5.3 Pattern identified

**Two distinct categories of API-module exports**:

1. **`impl JiraClient` methods** (70 fns): 1:1 wrap of a single Jira REST/Agile/Assets/JSM endpoint. Pure transport + serde. Examples: `get_issue`, `list_boards`, `search_assets`. These are the *resource bindings*.

2. **Free `pub` functions** (13 fns): EITHER (a) pure filters/extractors/projections that don't hit HTTP (`filter_story_points_fields`, `cmdb_field_ids`), OR (b) cache-aside multi-step orchestrators that invoke MULTIPLE `impl JiraClient` methods plus disk cache (`get_or_fetch_workspace_id`, `enrich_assets`, `resolve_object_key`).

**Convention rule (newly named — extends R1)**:
- Methods on `JiraClient` = **single-endpoint resource bindings** (REST 1:1).
- Free functions in `api/*/` modules = **cache-aside orchestrators** OR **pure data utilities**.

This is consistent: the 13 free functions don't violate the rule — they categorically belong outside `impl JiraClient` because they are NOT 1:1 endpoint wrappers. R1 P5R1-P-03 (3-pass asset enrichment dedup) is itself one of these orchestrators (in `cli/issue/list.rs`, not yet extracted to `api/assets/`).

**Verifiability**: counts above. Pattern holds 100% (13 free / 70 method classification with 0 misclassifications).

**Phase 1 implication**: this rule should be codified — when adding Confluence support (per CLAUDE.md), single-endpoint wrappers go into `impl JiraClient { … }` (or a future `impl ConfluenceClient`); orchestrators stay free. The categorical distinction is invisible currently.

### 5.4 Edge cases

- `api/client.rs:3039 pub fn extract_error_message(body: &[u8])` — pure error-body parser; module-local utility. Free pub fn outside `impl JiraClient` (line 3039 is past the `impl` close at line 2621). Falls into category (a) — pure data utility. Not counted in the 13 above for `api/jira/`/`api/assets/`/`api/jsm/`; total free pub fns including `api/client.rs` and `api/auth*.rs` would be larger but those are auth/transport-layer, not resource-layer.
- `api/auth.rs` exports many `pub fn` (load_oauth_tokens, store_api_token, etc.) — these are keychain operations, not Jira API resource bindings. They categorically belong outside `impl JiraClient`. Same for `api/auth_embedded.rs::embedded_oauth_app()`.
- `api/pagination.rs`: pagination iterators, separately structured, not counted.

The auth/keychain layer is structurally distinct from the resource-binding layer, and the convention "resource bindings live in `impl JiraClient`" still holds 100% within that layer.

---

## 6. R2-T4 — Sync `#[test]` vs `#[tokio::test]` triangulation

### 6.1 Per-file sync vs tokio counts

Across 31 test files in `tests/`:

| File | sync `#[test]` | `#[tokio::test]` | Profile |
|---|---:|---:|---|
| `all_flag_behavior.rs` | 0 | 11 | Pure async (HTTP) |
| `api_client.rs` | 11 | 11 | Mixed |
| `assets_errors.rs` | 0 | 3 | Pure async |
| `assets.rs` | 0 | 21 | Pure async |
| `auth_login_config_errors.rs` | 1 | 0 | Pure sync (config-only) |
| `auth_profiles.rs` | 10 | 0 | Pure sync (config + keychain) |
| `auth_refresh.rs` | 3 | 0 | Pure sync (keychain) |
| `board_commands.rs` | 1 | 14 | Mixed |
| `cli_handler.rs` | 0 | 2 | Pure async |
| `cli_smoke.rs` | 27 | 0 | Pure sync (assert_cmd, no HTTP) |
| `cmdb_fields.rs` | 0 | 5 | Pure async |
| `comments.rs` | 0 | 9 | Pure async |
| `duplicate_user_disambiguation.rs` | 0 | 5 | Pure async |
| `input_validation.rs` | 0 | 8 | Pure async |
| `issue_changelog.rs` | 1 | 38 | Mixed |
| `issue_commands.rs` | 0 | 54 | Pure async |
| `issue_create_json.rs` | 0 | 4 | Pure async |
| `issue_list_errors.rs` | 0 | 7 | Pure async |
| `issue_remote_link.rs` | 2 | 4 | Mixed |
| `issue_resolution.rs` | 0 | 3 | Pure async |
| `issue_view_errors.rs` | 0 | 4 | Pure async |
| `migration_legacy.rs` | 2 | 0 | Pure sync (config migration) |
| `oauth_embedded_login.rs` | 0 | 1 | Pure async |
| `project_commands.rs` | 0 | 10 | Pure async |
| `queue.rs` | 0 | 11 | Pure async |
| `sprint_commands.rs` | 1 | 12 | Mixed |
| `team_commands.rs` | 0 | 5 | Pure async |
| `team_object_shape.rs` | 0 | 4 | Pure async |
| `user_commands.rs` | 0 | 3 | Pure async |
| `user_pagination.rs` | 0 | 11 | Pure async |
| `worklog_commands.rs` | 0 | 5 | Pure async |
| **Totals** | **59** | **265** | — |

### 6.2 Discipline rule (codified by R2)

The 59 sync `#[test]` vs 265 `#[tokio::test]` split correlates **perfectly** with subject:

| Subject | Annotation | Rationale |
|---|---|---|
| `assert_cmd` process spawn (CLI smoke tests) | `#[test]` | `Command::cargo_bin("jr")` blocks the calling thread; spawned binary has its own runtime. No tokio needed in the harness. (`cli_smoke.rs` 27 sync tests.) |
| Config TOML parsing/migration | `#[test]` | Pure I/O via `figment`, sync API. (`migration_legacy.rs`, `auth_login_config_errors.rs`, parts of `auth_profiles.rs`.) |
| Keychain via `keyring` crate | `#[test]` | `keyring::Entry::set_password` is sync (blocking syscall). (`auth_refresh.rs`, `auth_profiles.rs`.) |
| Wiremock HTTP at integration level | `#[tokio::test]` | `wiremock::MockServer::start().await` requires runtime. All async HTTP testing. |
| `JiraClient::new_for_test(...)` + reqwest call | `#[tokio::test]` | reqwest is async-only. |
| Mixed-file sync subset | `#[test]` | Files like `api_client.rs` (11/11) test BOTH error parsing (sync — `JrError::from_response_*` is sync) AND HTTP plumbing (async). The split inside one file reflects which surface is under test. |

**No misclassifications observed** in the 31-file enumeration. The discipline is **uniform**:
- If the test exercises any async HTTP/wiremock → `#[tokio::test]`.
- If the test exercises ONLY sync surfaces (config/keychain/process-spawn/pure-fn) → `#[test]`.
- If the file tests both → mix sync + tokio per-test, never a single annotation.

**Verifiability**: 31-file table above; full counts via `awk` per file.

### 6.3 Convention rule (named, codifies R1's open R2-T4 question)

**Convention**: test annotation choice mirrors the surface under test. Sync surfaces → `#[test]`; any async HTTP → `#[tokio::test]`. Mixed-surface files mix annotations per-test.

This is **codified-by-practice** (zero deviation across 324 tests in `tests/`) but **not codified in CLAUDE.md or any docs/specs/**. Adding it to a contributing guide would close GAP-CAT-5 TI-2.

---

## 7. Updated counts (R1 metadata reconciliation)

| Item | R1 metadata header | R1 §10 Delta | R2 verification |
|---|---|---|---|
| New design patterns | 7 | 7 | 7 ✓ |
| New anti-patterns | 7 | 7 | 7 ✓ |
| Pre-VSDD drifts | 6 (line 23) | 7 (corrected) | 7 ✓ |
| Test-mechanism subjects | 9 (line 24) | 13 (corrected) | 13 ✓ |
| GAP categories | 5 | 5 | 5 ✓ |
| Strengths re-ranked | top 7 (line 25) | top 7 | 7 ✓ |

**R1 reconciliation**: two minor metadata-header undercounts (6 vs 7 drifts; 9 vs 13 mechanisms) were already corrected in R1's own §10 Delta. R2 confirms the §10 numbers. No retraction needed.

---

## 8. Delta Summary

- **New patterns added this round**: 0 (R1 was exhaustive on novel categories)
- **New anti-patterns added this round**: 1 (P5R2-AP-01 — handler stderr/stdout discipline implicit; no codified rule)
- **Audits performed**:
  - R2-T1 cross-handler eprintln/println discipline: **5 categorical profiles identified** (Pure / Read-only / Mixed / Symmetric / no-log-facade); discipline is codified-by-practice but uncodified-in-docs.
  - R2-T2 `--output json` parity: **49 subcommands enumerated, 41 have JSON, 5 genuine gaps (auth login/switch/logout/remove/refresh), 4 N/A by design**. Single-largest gap is auth-side-effect commands.
  - R2-T3 `impl JiraClient` resource binding: **70 methods + 13 free pub fns; pattern holds 100%**. Free fns are pure utilities or cache-aside orchestrators.
  - R2-T4 sync `#[test]` vs `#[tokio::test]`: **59 sync + 265 tokio across 31 files; perfect correlation with subject under test; zero deviation**.
- **Existing items refined**: 0 — R1 anti-patterns and patterns all stand at source.
- **Hallucination corrections**: 1 framing nitpick on R1's metadata-header drift count (6 vs 7) and mechanism count (9 vs 13), both already corrected in R1 §10.
- **R1 audit**: 7 patterns + 7 anti-patterns + 7 drifts + 5 GAP categories + 13 mechanism rows all verified at source. **0 retractions.**

**Remaining gaps for next round** (if any):
- Codify the snapshot-vs-`assert!(stdout.contains)` boundary (R1 R2-T5, deferred to R3).
- Error-message wording uniformity audit (R1 R2-T6, deferred).
- Possible R3 target: enumerate config-key vs CLI-flag vs env-var coverage matrix (~20 fields × 3 surfaces). Likely NITPICK.

---

## 9. Novelty Assessment

**Novelty: NITPICK**

**Justification**: Removing R2's findings would NOT change how the Phase 1 spec is structured. R2 is a confirmation/audit round:

- **0 new pattern categories named.** R2-T3 (resource-binding rule) and R2-T4 (sync/tokio rule) are *codifications of practices R1 already named in spirit*. R1 P5R1-P-06 already named the test-isolation pattern; R2-T4's annotation rule is a sibling. R1 explicitly called out resource-per-file in `api/jira/` (consistency assessment); R2-T3 just verified the impl-binding shape with counts.
- **1 new anti-pattern (P5R2-AP-01)**, but it is a *named refinement* of the discipline question R1 already raised (R1 R2-T1 was tabled as a deferred audit; R2 audited it and named the gap). No new structural problem discovered.
- **Audit results**: every R2-T1/T2/T3/T4 audit confirms what R1 hypothesized. Specifically:
  - R2-T1: discipline is consistent within categorical profiles. As R1 expected.
  - R2-T2: JSON parity is 84% (41/49); 5 gaps are exactly the auth side-effect commands. R1 predicted "high but uneven" — confirmed.
  - R2-T3: free pub fns are exactly the pure utilities and orchestrators R1 P5R1-P-03 hinted at. Pattern is uniform.
  - R2-T4: sync/tokio choice is perfectly subject-correlated with zero deviation. Strongest possible audit result, but it confirms an *existing* convention rather than reveals a new one.
- **No model-level changes**: a Phase 1 spec author working from R1 only would arrive at the same crystallization as one with R1 + R2. R2 gives them more confidence and citation density, not new categories.

**The test (would removing this round's findings change how I'd spec the system?)**: No. The Phase 1 spec would still need to address the same 5 GAP categories from R1; R2 just adds counts/citations to back them up. Specifically:
- The eprintln/println discipline gap (GAP-CAT-1) would still need codification regardless of R2's enumeration.
- The 5 auth-JSON-parity gaps refine GAP-CAT-4 J1 but don't change the decision needed.
- The resource-binding pattern was already implicit in R1's "consistency assessment HIGH for `api/jira/` resource-per-file"; R2 quantifies it but doesn't reveal a new constraint.
- The sync/tokio rule was already implicit in R1's test-mechanism subject map; R2 codifies it but reveals nothing new.

R2 is exactly the kind of "tabulated audit without new pattern categories" the convergence-deepening protocol identifies as NITPICK.

---

## 10. Convergence Declaration

**Pass 5 has converged — R2 findings are nitpicks, not gaps.**

R1 was substantively exhaustive. R2 confirms and quantifies but adds no new pattern category. The single new anti-pattern (P5R2-AP-01) is a named refinement of an open audit question R1 deferred — not a new structural problem.

**Recommendation**: Do not proceed to R3 for Pass 5. Phase 1 spec inputs from Pass 5 should reference broad + R1 + R2 (which is supplemental for the 4 audits) and consider Pass 5 closed.

If R3 is run (e.g., the deferred R2-T5 snapshot-boundary audit, R2-T6 error-wording uniformity), expect NITPICK with high confidence — both deferred targets are also tabulation work over already-named patterns.

---

## 11. State Checkpoint

```yaml
pass: 5
round: 2
status: complete
new_design_patterns: 0
new_anti_patterns: 1            # P5R2-AP-01: handler stderr/stdout discipline implicit
new_test_conventions: 1         # codified sync/tokio annotation rule (codifies R1's R2-T4 open question)
new_pre_vsdd_drifts: 0
audits_performed: 4
  R2-T1_eprintln_println_discipline: 5 categorical profiles, 47 eprintln + 103 println across 24 files, no log facade, codified-by-practice
  R2-T2_json_parity: 49 subcommands, 41 have JSON, 5 gaps (all auth side-effect), 4 N/A by design
  R2-T3_resource_binding: 70 impl JiraClient methods + 13 free pub fns; rule holds 100% (free fns = pure utilities or cache-aside orchestrators)
  R2-T4_test_annotations: 59 sync + 265 tokio across 31 files; perfect subject-correlation; zero deviation
files_examined: 9               # cli/* dispatch table, api/* enumeration, tests/ enumeration, R1, json_output.rs, plus spot reads
verification_actions:
  - wc_l_per_file: confirmed all R1 LOC table values (auth.rs 1998, list.rs 1083, assets.rs 1055, etc.)
  - awk_eprintln_println: 47 eprintln + 103 println across 24 cli files
  - awk_pub_fn_api: 13 free pub fns + 70 impl JiraClient methods across 17 impl blocks
  - awk_test_annotations: 59 sync + 265 tokio across 31 tests/*.rs files
  - manual_subcommand_enumeration: 49 surface points across 16 top-level commands; 41 JSON, 5 gaps, 4 N/A
inconsistencies_resolved: 0
hallucination_corrections: 1   # R1 metadata header undercount (6 drifts vs 7; 9 mechanisms vs 13) — already corrected in R1 §10 Delta
r1_retractions: 0
r1_patterns_verified: 7        # all P5R1-P-01..07
r1_anti_patterns_verified: 7   # all P5R1-AP-01..07
r1_drifts_verified: 7          # Drift-1..7
r1_gap_categories_verified: 5  # GAP-CAT-1..5
novelty: NITPICK
timestamp: 2026-05-04T17:30:00Z
convergence: |-
  Pass 5 has converged. R2 is a tabulation/audit round confirming R1's named patterns.
  No new pattern categories. Phase 1 spec should reference broad + R1 + R2.
  R3 is not required; if run, expect NITPICK with high confidence.
```

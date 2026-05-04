# Pass 6: Broad-Sweep Synthesis — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Inputs: Pass 0 (inventory), Pass 1 (architecture), Pass 2 (domain model), Pass 3 (behavioral contracts), Pass 4 (NFR catalog), Pass 5 (conventions), CLAUDE.md.

> This is the **broad-sweep synthesis** for Phase A. It cross-references the five prior passes, surfaces inconsistencies, identifies gaps, and lists Phase B deepening targets. It is **not** the final synthesis (Phase C / Pass 8) — that one will be produced after convergence rounds finish.

---

## 1. Executive summary

`jr` (package `jr`, binary `jr`) is a Rust 2024 / MSRV-1.85 single-crate CLI for automating Atlassian Jira Cloud workflows. It is a **thin client** (ADR-0001) wrapping Jira Core REST v3, Agile REST, JSM REST, the Atlassian Teams API, GraphQL `tenantContexts`, and the Assets/CMDB API directly with `reqwest 0.13` (rustls, no native-tls). The architecture is a clean 5-layer stack (`L0 main → L1 Cli derive → L2 cli handlers → L3 api client + auth + pagination + rate_limit → L4 resource impls → L5 types`) with cross-cutting utilities (cache, config, error, output, adf, jql, duration, partial_match, observability). `jr` ships as a single static binary — no daemon, no IPC — and persists state to `~/.config/jr/config.toml`, `~/.cache/jr/v1/<profile>/`, and the OS keychain (`jr-jira-cli` service).

The most consequential findings from this ingest are:

1. **Scale and test density.** 80 source files / 23,334 LOC + 36 integration test files / 16,958 LOC + 931 total test functions (607 inline unit + 324 integration) + 17 insta snapshots + property tests on JQL/duration/partial_match. The test-to-source LOC ratio is **0.73** and the project has 6 ADRs, 22 post-v1 feature specs in `docs/specs/`, plus 56 pre-VSDD design docs and 75 pre-VSDD plans in `docs/superpowers/`.
2. **Architectural distinction.** Multi-profile correctness is enforced at three signature boundaries (every cache reader/writer takes `profile: &str` first; keychain has shared-vs-namespaced key namespaces with default-only legacy migration; config-active-profile resolution has a deterministic flag > env > config > "default" precedence threaded as a parameter, never via env-var seam). Combined with the ADR-0006 build-time XOR-obfuscated embedded OAuth app (per-build random key, fixed callback port 53682, TOCTOU-closed listener binding), the project encodes more correctness in signatures than in compile-time fences — a deliberate trade-off.
3. **Biggest unresolved risk.** Two large CLI handler files (`cli/auth.rs` 1,998 LOC and `cli/assets.rs` 1,055 LOC) violate the implicit "shard at ~1,000 LOC" rule that produced `cli/issue/`. Combined with `cli/issue/list.rs` having grown past CLAUDE.md's stated ~970 LOC to **1,083 LOC** even *after* the `list-rs-split.md` refactor, the project has visible drift between architectural rule and current state. No `docs/specs/auth-rs-split.md` or `docs/specs/assets-rs-split.md` exists.
4. **Pre-VSDD docs treatment is the explicit Phase 0 → Phase 1 decision pending.** The orchestrator must choose how to handle `docs/superpowers/specs/` (56 files, 10,727 LOC), `docs/superpowers/plans/` (75 files, 56,572 LOC), `docs/specs/` (22 post-v1 feature specs, 3,778 LOC), and `docs/adr/` (6 ADRs, 169 LOC) — **harmonize**, **reference-only**, or **supersede**. My default proposal is HARMONIZE (see §7).

---

## 2. Confidence assessment

| Pass | Scope | Coverage (0-3) | Confidence (0-3) | Notes / largest gap |
|---|---|---:|---:|---|
| 0 — Inventory | LOC counts, file manifest, dep graph, test counts, doc inventory, prioritization | 3 | 3 | Every numeric claim has a shell command alongside. No gap. |
| 1 — Architecture | Layer boundaries, dependency graph, cross-cutting concerns, deployment topology, 4 mermaid diagrams, 10 risks, 8 deviations | 3 | 3 | Only `adf.rs` (1,826 LOC) was not read in full — its public surface is captured but its internal node DSL is not characterized. |
| 2 — Domain Model | 51 entities, 19 value objects, 25 invariants, 7 bounded contexts, 4 state-machine diagrams, 6 cross-cutting flows | 3 | 3 | `cli/auth.rs` (1,998 LOC), `cli/issue/changelog.rs` (847 LOC), `api/auth.rs` (1,397 LOC) and `cli/issue/helpers.rs` (813 LOC) read at head only — domain operations enumerated but not exhaustively characterized at the function-by-function level. |
| 3 — Behavioral Contracts | 188 BCs (134 HIGH / 45 MEDIUM / 9 LOW); 20 holdout candidates; 7 untested invariants categorized | 2 | 3 | LOW-confidence BCs (n=9) need strengthening. ADF (69 unit tests), changelog (38 unit tests), and full keychain-gated tests not yet enumerated as discrete BCs — coverage of unit-test surface is partial. Keyring round-trip BCs are `#[ignore]`-gated. |
| 4 — NFR Catalog | 27 named config values cataloged; 23 NFR gaps across 5 dimensions (perf, security, reliability, observability, scalability) | 3 | 3 | `Cargo.lock` (332 transitive deps) inspected only via Pass 0 grep; specific advisory exposure not audited (would require running `cargo audit`). |
| 5 — Conventions | 10 naming conventions, 12 design patterns, 11 anti-patterns, top 5 strengths + top 5 gaps; consistency rated per axis | 3 | 3 | Test-name pattern split (108 prefix vs 212 no-prefix) quantified; ADF and changelog test conventions only sampled, not fully enumerated. |

**Interpretation:** Areas with coverage <3 or confidence <3 are the priority targets for Phase B deepening. Pass 3 (coverage 2/3) is the dominant Phase B target — both because BC extraction is highest value for the spec-crystallization phase, and because three of the largest files in the codebase (`cli/auth.rs`, `cli/issue/list.rs`, `cli/assets.rs`) need function-level BC enumeration that the broad sweep could not cover.

---

## 3. Cross-pass inconsistencies

### INC-01: `JrError` variant count
**Claims**:
- Pass 1 §3a said: "A single `enum JrError` with 10 variants (verified read)."
- Pass 2 §2a.2 said: "Pass 1 listed 10 variants; the actual count is **11** (Pass 1 missed `Json`)."
- Pass 5 §3.1 said: "**11 variants** confirmed (Pass 1 §3a missed `Json` and listed 10; Pass 2 §2a.2 corrects to 11)."
**Reconciliation**: **11 variants is correct.** Verified by reading `src/error.rs:3-49` directly during this pass — variants are: `NotAuthenticated`, `InsufficientScope`, `NetworkError`, `ApiError`, `ConfigError`, `UserError`, `Internal`, `Interrupted`, `Http(#[from])`, `Io(#[from])`, `Json(#[from])`. Pass 1 was a counting error; Passes 2 and 5 are accurate.
**Source**: `src/error.rs:1-49` (full read during Pass 6).

### INC-02: `EMBEDDED_CALLBACK_PORT` location
**Claims**:
- CLAUDE.md "Gotchas" said: "`src/api/auth_embedded.rs` is a thin sibling module to `auth.rs`. Keep obfuscation plumbing there; keep keychain/OAuth flow plumbing in `auth.rs`." (implies port plumbing — strategy / listener — lives with auth.rs, but the constant is unspecified)
- Pass 1 §8 deviation #3 said: "The fixed callback port constant `EMBEDDED_CALLBACK_PORT` (53682) is in `api::auth`, not `api::auth_embedded`. … verified at `api/auth.rs:384` it is in `api/auth.rs`."
- Pass 2 §2a.3 said: "`EMBEDDED_CALLBACK_PORT` (`api/auth.rs:384`, per Pass 1 §3d): the literal `53682`. Owned by `api::auth`, not `api::auth_embedded`."
**Reconciliation**: **Constant lives in `src/api/auth.rs:384`** as `pub const EMBEDDED_CALLBACK_PORT: u16 = 53682;`. Verified directly during Pass 6 via awk-search across all `.rs` files. The CLAUDE.md narrative is consistent with this when read carefully ("OAuth flow plumbing in auth.rs"), but a casual reader would expect the constant to be co-located with the embedded credential plumbing — this is an editorial-clarity issue in CLAUDE.md, not a code-level inconsistency.
**Source**: `src/api/auth.rs:384` (`pub const EMBEDDED_CALLBACK_PORT: u16 = 53682;`); also referenced from `src/cli/auth.rs:448`, `src/api/auth.rs:404, 930, 945`.

### INC-03: `cli/issue/list.rs` line count
**Claims**:
- CLAUDE.md "Gotchas" said: "`list.rs` is large (~970 lines)."
- Pass 0 §3a said: 1,083 LOC.
- Pass 0 §3a explicit comment: "Note: `list.rs` is documented in CLAUDE.md as ~970 lines. Actual is **1,083** — it has grown."
- Pass 5 §8.2 said: "Module-size growth on `cli/issue/list.rs` (now 1,083 LOC, was ~970 per CLAUDE.md) — already split once via `docs/specs/list-rs-split.md`."
**Reconciliation**: **1,083 LOC is correct.** Verified directly during Pass 6: `wc -l src/cli/issue/list.rs` → 1083. CLAUDE.md is stale by ~115 LOC. **Action:** the eventual VSDD doc should either pin the current LOC or use an aspirational "≤1000 LOC after split" target with `docs/specs/list-rs-split-round-2.md` (does not yet exist).
**Source**: `src/cli/issue/list.rs` (1083 LOC, verified).

### INC-04: Issue subcommand count
**Claims**:
- CLAUDE.md `cli/issue/` block lists 8 submodules: `mod.rs`, `format.rs`, `list.rs`, `create.rs`, `workflow.rs`, `links.rs`, `helpers.rs`, `assets.rs`.
- Pass 0 §3a said: 12 files in `cli/issue/`.
- Pass 1 §1a said: "split issue commands ... 12 + 11 modules" and listed `view, list, comments, changelog, create, workflow, links, format, helpers, json_output, assets`.
- Pass 2 §2b.1 said: "12 + 1 = 13 issue subcommands" (the +1 being `remote-link`, which is a top-level `IssueCommand::RemoteLink` variant; the file inventory still shows 12 files).
**Reconciliation**: **12 files in `cli/issue/`.** CLAUDE.md is missing `view.rs`, `comments.rs`, `changelog.rs`, `json_output.rs`. The narrative claim "list.rs contains list + view + comments" is also stale — `view` and `comments` were extracted into sibling files per `docs/specs/list-rs-split.md`. Counted CLI subcommands: 17 (`list`, `view`, `create`, `edit`, `move`, `transitions`, `resolutions`, `assign`, `comment`, `comments`, `changelog`, `open`, `link`, `unlink`, `link-types`, `remote-link`, `assets`).
**Source**: Pass 0 §3a; Pass 1 §1a; Pass 2 §2b.1 issue-subsystem table.

### INC-05: Status category color filtering mechanism
**Claims**:
- CLAUDE.md "Gotchas" said: "Status category colors are fixed: `green` = Done, `yellow` = In Progress, `blue-gray` = To Do. … Used by `--open` filtering."
- Pass 1 §3a referenced "blue-gray" colors as the mapping CLAUDE.md cites.
- Pass 2 §2b.2 #4 said: "**`--open` filtering** uses two distinct mechanisms depending on context: For Jira issues (`cli/issue/list.rs:303, 308, 625`): JQL clause `statusCategory != Done`. … For connected tickets (`cli/assets.rs:303-321`): client-side filter on `status.color_name != "green"`."
- Pass 3 BC-314 said: assets tickets `--open` filters `colorName != "green"`.
- Pass 3 INV-21 said: issue list `--open` uses JQL `statusCategory != Done`.
**Reconciliation**: **Both are correct — there are two mechanisms.** CLAUDE.md's "Used by `--open` filtering" implies one mechanism, but the codebase uses **JQL `statusCategory != Done`** for issues (because `cli/issue/list.rs` composes JQL server-side and `Issue::status` from search results carries `StatusCategory.key`, not `colorName`) and **client-side `colorName != "green"`** for assets connected tickets (because the `/connectedTickets` JSM endpoint returns `TicketStatus { name, colorName }` — confirmed in `types/assets/ticket.rs:25-30`). The mechanism choice is forced by the API shape, not by design preference. **Action:** the eventual VSDD doc should describe both filters as a single semantic ("filter to non-Done") with two implementations.
**Source**: `cli/issue/list.rs:303, 308, 625`; `cli/assets.rs:303-321`; `types/assets/ticket.rs:25-30`.

### INC-06: Top-level command surface
**Claims**:
- CLAUDE.md `cli/` block does not list `api.rs` or a `Completion` subcommand.
- Pass 0 §4 listed top-level commands as: `Init, Assets, Auth, Me, Project, Issue, Board, Sprint, Worklog, Team, User, Queue, Api, Completion`.
- Pass 1 §8 deviation #1: "Top-level `Api` and `Completion` subcommands are not in CLAUDE.md."
- Pass 2 §2b.1 confirms 14 top-level commands, including `Api` and `Completion`.
**Reconciliation**: **14 top-level commands; CLAUDE.md is missing `Api` and `Completion`.** `api.rs` is at 342 LOC; `Completion` is dispatched inline in `main.rs:67-71`.
**Source**: `src/cli/mod.rs:54-133`; `src/main.rs:67-71`; `src/cli/api.rs` (342 LOC).

### INC-07: `refresh_oauth_token` callers
**Claims**:
- CLAUDE.md Gotchas said: "`refresh_oauth_token` resolves credentials internally (keychain → embedded) — callers pass only `profile`."
- Pass 1 §8 deviation #4 said: "`refresh_oauth_token` is undocumented as having no production callers. The `pub` function exists … but the docstring (line 700) says: 'Currently has no production callers — it exists for a future 401 auto-refresh integration. `jr auth refresh` (the user-facing CLI command) uses the clear-and-relogin flow at `cli/auth.rs::refresh_credentials`, not this helper.'"
- Pass 4 §2.5 said: "`refresh_oauth_token` has NO production callers."
**Reconciliation**: **No contradiction; CLAUDE.md is silent on the production-caller question.** The function exists `pub` for a future 401-auto-refresh integration; `jr auth refresh` uses clear-and-relogin instead. CLAUDE.md describes the *signature contract* (no `client_id`/`client_secret` parameters) but not the *call-graph fact*. **Action:** if the eventual VSDD doc retains `refresh_oauth_token`, document the deferred-integration intent.
**Source**: `src/api/auth.rs:700-770` (function + docstring).

### INC-08: Lib.rs `observability` visibility
**Claims**:
- CLAUDE.md `lib.rs` description says: "Crate root (re-exports for integration tests)."
- Pass 1 §1c said: "`observability` is `pub(crate)` — it is intentionally NOT part of the integration-test public API. … This is a substantive deviation from CLAUDE.md."
**Reconciliation**: CLAUDE.md elides the visibility detail. **Verified at `src/lib.rs:1-12`:** all modules are `pub` except `pub(crate) mod observability;`. This is a deliberate design choice (the once-flag plumbing is implementation detail) and a minor CLAUDE.md gap, not a code inconsistency.
**Source**: `src/lib.rs:1-12`.

### INC-09: `Interrupted` variant vs Ctrl+C exit path
**Claims**:
- Pass 1 §3a said: "Exit code 130 is also used by main.rs for direct Ctrl+C (the `Interrupted` variant exists but isn't reached in practice — main.rs prints 'Interrupted' and `process::exit(130)` directly from the `tokio::select!` arm)."
- Pass 2 §2a.2 said: "`Interrupted` (`error.rs:38-39`) — `—` — 130 — Reserved variant; the actual Ctrl+C path in `main.rs:264` exits 130 directly without constructing this variant."
- Pass 4 §4.4 confirms abrupt `process::exit(130)`.
**Reconciliation**: **Consistent across passes.** The `Interrupted` variant is a reserved entry in `JrError` for completeness/future use; the Ctrl+C path bypasses it. This is documented honestly in all three passes. No inconsistency, but the eventual VSDD doc should call this out (or remove the variant if no caller is planned).
**Source**: `src/error.rs:38-39`; `src/main.rs:261-267`.

### INC-10: 12 vs 13 vs 17 issue commands count discrepancies
**Claims**:
- Pass 1 §1a counted "12 + 11 modules" in cli/, then "12 issue subcommands" in §1c.
- Pass 2 §2b.1 counted "12 + 1 = 13 issue subcommands per `cli/mod.rs:278-561`" and listed 17 commands in the §2b.1 issue table.
- Pass 6 cross-check: counting `IssueCommand` variants in `cli/mod.rs:54-738`: List, View, Create, Edit, Move, Transitions, Resolutions, Assign, Comment, Comments, Changelog, Open, Link, Unlink, LinkTypes, RemoteLink, Assets = **17 subcommands**.
**Reconciliation**: **17 subcommands.** Pass 2's "+1" was an attempt to reconcile mod-file count (12 files) with subcommand count (17 commands); the reconciliation is messy. **Action:** Phase B should produce the canonical 17-row issue subcommand table, fully BC'd.
**Source**: `src/cli/mod.rs` IssueCommand variants; Pass 2 §2b.1.

### INC-11: Library-vs-process tests "28 of 36" reuse pattern
**Claims**:
- Pass 5 §4.3 said: "Library-level tests… Used by 28 of 36 integration test files."
- Pass 5 §4.3 also said: "Process-level tests… Used for end-to-end exit-code/stderr/stdout validation."
- Pass 5 §4.7 said: "Pattern verified in 28 of 36 integration test files."
- Pass 5 §4.8 said: "Used in 36 integration tests" — implying all 36 use assert_cmd.
**Reconciliation**: The "28 of 36" applies to **library-level** (using `JiraClient::new_for_test`), and the remaining 8 use process-level invocation via `assert_cmd`. Pass 5 §4.8 is loose ("Used in 36 integration tests" — likely meant "36 integration test FILES exist; assert_cmd is used in some of them" but reads as a count claim). **Resolution: the boundary is fuzzy.** Many tests use both — they import `JiraClient::new_for_test` for library calls AND `Command::cargo_bin("jr")` for binary calls in different functions of the same file. **Action:** Phase B should produce a clean per-file test-style enumeration.
**Source**: `tests/*.rs` headers; Pass 5 §4.3, §4.7, §4.8.

### INC-12: Asset enrichment "N+1" framing
**Claims**:
- Pass 4 §1.5 said: "Asset enrichment is serialized, not concurrent: Per-field `client.get_assets(workspace_id, "object/{key}")` calls in `cli/issue/view.rs` and per-row enrichment in `cli/issue/list.rs` are awaited one-at-a-time."
- Pass 4 §5.2 said: "Asset enrichment N+1 risk: Per-row enrichment in `cli/issue/list.rs` issues one `client.get_assets(workspace_id, "object/{key}")` per linked asset per issue. For a list of 50 issues with 3 CMDB fields each, this can be 150+ extra GETs — all serial, all fully buffered."
- Pass 2 §2b.6 Flow 3 confirmed the per-asset GET pattern.
**Reconciliation**: **No inconsistency; Pass 4 makes the same point twice.** The N+1 risk is real, the broad sweep characterized it correctly. **Action:** Phase B target — verify whether `extract_linked_assets_per_field` already deduplicates by asset key before issuing per-asset GETs (would mitigate N+1 in practice). The Pass 4 framing assumes no dedup.
**Source**: `cli/issue/list.rs`; `api/assets/linked.rs`.

**Total inconsistencies found: 12.** Of these, 5 are confirmed factual errors that this pass resolved (INC-01, INC-02, INC-03, INC-04, INC-06) and 7 are framing/clarity issues (INC-05, INC-07, INC-08, INC-09, INC-10, INC-11, INC-12). Most of the factual errors trace to CLAUDE.md being a stale snapshot that the codebase has grown past.

---

## 4. Unified knowledge map

The codebase decomposes cleanly into 14 bounded contexts (Pass 2 listed 7 at the top level; the granular CLI surface adds 7 more behavioral subcontexts driven by command-family semantics). Risk level is composite: HIGH = >1500 LOC OR cross-cutting + many BCs OR many NFR concerns; MEDIUM = >800 LOC OR several NFR concerns; LOW = small, cohesive, well-bounded.

| Bounded context | Module path(s) | LOC | BCs | NFR concerns | Convention adherence | Risk level |
|---|---|---:|---:|---|---|---|
| **Auth & Identity** | `cli/auth.rs` (1,998), `api/auth.rs` (1,397), `api/auth_embedded.rs` (250) | 3,645 | ~24 (BC-001..024) | OAuth client lacks 30s timeout (gap §7.1.1), no PKCE (gap §7.2.6), no auto-refresh (gap §7.3.15), keychain partial-state, mixed kebab/snake key naming | mixed — file size violates implicit shard rule; key naming wart; otherwise high | **HIGH** |
| **Issue read** (list/view/comments/changelog) | `cli/issue/list.rs` (1,083), `cli/issue/view.rs` (TBD), `cli/issue/comments.rs` (TBD), `cli/issue/changelog.rs` (847), `cli/issue/format.rs` (TBD) | ~3,000 | ~25 (BC-101..124, plus changelog/comments) | full-buffer rendering, N+1 asset enrichment risk, JQL composition correctness | high — well-tested, JQL property-tested | **HIGH** |
| **Issue write** (create/edit/move/assign/comment/link/open/remote-link) | `cli/issue/create.rs` (375), `cli/issue/workflow.rs` (788), `cli/issue/links.rs` (TBD), `cli/issue/helpers.rs` (813), `cli/issue/json_output.rs` (TBD) | ~2,500 | ~25 (BC-201..225) | idempotency on move/assign (BC-207, BC-204), markdown→ADF correctness, label add/remove prefixes | high — clap conflicts pinned, JSON shape pinned via insta | **HIGH** |
| **Issue assets / CMDB** | `cli/issue/assets.rs` (TBD), `api/assets/linked.rs` (557), `api/assets/objects.rs`, `api/assets/workspace.rs` | ~1,200 | ~15 (BC-301..315) | N+1 GET pattern, AQL escaping, workspace ID resolution edge cases (404/403 → "JSM Premium required") | high — `aqlFunction` + `Key` capitalization correctly enforced | **HIGH** |
| **Assets standalone** (`jr assets *`) | `cli/assets.rs` (1,055), `api/assets/{schemas,tickets}.rs` | 1,400+ | ~12 (subset BC-301..315 + tickets) | colorName != green tolerance (tickets), bool-or-string `is_last`, schema discovery cache | high — `cli/assets.rs` violates shard rule (gap §9.2) | **MEDIUM-HIGH** |
| **Boards & Sprints** | `cli/board.rs`, `cli/sprint.rs` (438), `api/jira/boards.rs`, `api/jira/sprints.rs` | ~700 | ~10 (BC-401..410) | `MAX_SPRINT_ISSUES = 50` cap, scrum-only check (Pass 2 INV-23), board auto-resolve | high — small, cohesive | **MEDIUM** |
| **Worklogs** | `cli/worklog.rs`, `api/jira/worklogs.rs`, `duration.rs` (159) | ~500 | ~8 (BC-501..508) | `parse_duration` accepts combined units (vs JQL `validate_duration` which rejects — Pass 2 INV-5/INV-6 confusion vector) | high — property-tested, two duration parsers documented | **LOW-MEDIUM** |
| **Teams** | `cli/team.rs`, `api/jira/teams.rs` (56), `cache::TeamCache` | ~300 | ~6 (BC-601..606) | GraphQL `tenantContexts` (ADR-0005), org-id discovery, corrupt cache tolerance (BC-115) | high | **LOW-MEDIUM** |
| **Users** | `cli/user.rs`, `api/jira/users.rs`, `partial_match.rs` (200) | ~600 | ~9 (BC-701..709) | `--all` startAt advances by REQUESTED maxResults (regression-pinned), 1500-user safety cap (`USER_PAGINATION_SAFETY_CAP`), duplicate-disambiguation under `--no-input` | high — pagination semantics regression-pinned | **MEDIUM** |
| **Projects & Queues** | `cli/project.rs`, `cli/queue.rs` (323), `api/jira/projects.rs`, `api/jsm/{servicedesks,queues}.rs` (214) | ~700 | ~8 (BC-801..808) | service-desk discovery, `require_service_desk` for queue commands | high | **LOW-MEDIUM** |
| **Configuration** | `config.rs` (1,223) + `main.rs` profile threading | 1,300+ | ~12 (BC-901..911) | figment layering, profile resolution precedence, legacy migration write-back uses file-only baseline (Pass 2 INV-21), profile-name validation at three boundaries | high — 37 inline tests pin validation | **HIGH** (large file, central) |
| **Cache** | `cache.rs` (899) | 899 | ~10 (BC-1001..1010) | per-profile signature soft fence, TTL behavior, cross-profile leak risk, corruption recovery, non-atomic writes | high — per-profile signature universally followed | **MEDIUM-HIGH** |
| **Output / Rendering** | `output.rs` (76), `adf.rs` (1,826), `cli/issue/format.rs`, `cli/issue/json_output.rs` | ~2,000 | ~11 (BC-1101..1111) | JSON output shape stability, ADF round-trip correctness, stderr/stdout discipline, `--no-color` / `NO_COLOR` | high — `adf.rs` is the second-largest file in the codebase but cohesive; insta snapshots cover write-op shapes | **MEDIUM** |
| **Error handling & Runtime** | `error.rs` (137), `main.rs` (268), `api/client.rs` (490), `api/rate_limit.rs` (56), `api/pagination.rs` (374), `observability.rs` (39), `jql.rs` (395) | ~1,800 | ~22 (BC-1201..1214 + BC-1401..1411) | 11 JrError variants + exit-code mapping, 6-level `extract_error_message` precedence chain, 429 retry MAX_RETRIES=3, `JR_BASE_URL` test override | high — `extract_error_message` 6-level chain is a load-bearing detail not in CLAUDE.md | **MEDIUM-HIGH** |

**14 bounded contexts.** Of these, 5 are HIGH risk (Auth, Issue read, Issue write, Issue assets, Configuration), 4 are MEDIUM-HIGH or MEDIUM, and 5 are LOW-MEDIUM. The HIGH-risk contexts are also the largest by LOC and by BC count — Phase B deepening should prioritize them in that order, with Auth first (because the OAuth state machine, refresh path, and multi-profile coupling are the biggest single subsystem).

---

## 5. Gap report — Orphans and under-covered subsystems

### 5.1 Orphaned modules (>100 LOC, not substantially analyzed in Passes 1-3)

Files that received only a head-read or grep-level treatment in passes 1-3 and merit Phase B function-level deepening:

| File | LOC | Why orphan | Pass that should cover |
|---|---:|---|---|
| `src/adf.rs` | 1,826 | Read at head only (Pass 1 §3h says "I did not read this file in full"); 69 inline unit tests catalogued in Pass 3 §1.2 but not enumerated as discrete BCs | Pass 2 (entity model for ADF nodes), Pass 3 (per-node BCs) |
| `src/cli/auth.rs` | 1,998 | Only ~60 LOC read in Pass 1 (head); 44 unit tests in `cli::auth::tests` not enumerated; OAuth state machine not characterized at function level | Pass 2/3 |
| `src/api/auth.rs` | 1,397 | ~900 LOC read in Pass 1 (head + middle + tail); 22 unit tests catalogued; legacy migration partial-state branches not exhaustively characterized | Pass 2/3 |
| `src/cache.rs` | 899 | ~150 LOC read in Pass 1 (head); 27 unit tests covering TTL/per-profile/corruption — only 7 enumerated as BCs | Pass 3 |
| `src/cli/issue/changelog.rs` | 847 | Pass 0 lists 38 unit tests; Pass 3 BC-119..121 cover only 3 of those; AuthorNeedle smart constructor characterized in Pass 5 §5.12 but BCs not enumerated | Pass 3 |
| `src/cli/issue/helpers.rs` | 813 | 21 unit tests; team UUID resolution, story-points assignment, user resolution functions not BC'd at function level | Pass 2/3 |
| `src/cli/issue/workflow.rs` | 788 | 6 inline tests + integration coverage; idempotent-move state machine characterized but resolution-resolver and per-helper logic not | Pass 3 |
| `src/cli/issue/list.rs` | 1,083 | Pass 1 read first ~115 LOC; JQL composition rules + sprint expansion + status auto-inference + 4 date filters + `--open` filter not function-level BC'd | Pass 3 |
| `src/cli/assets.rs` | 1,055 | 21 unit tests; AQL building, `--open` colorName filter, `--status` partial-match — only 3 enumerated as BCs | Pass 3 |
| `src/config.rs` | 1,223 | ~450 LOC read; 37 unit tests; figment layering, validate_profile_name, migration are characterized but not all branches | Pass 2/3 |

### 5.2 Under-documented bounded contexts (low BC count relative to LOC)

- **ADF rendering subsystem.** 1,826 LOC, 69 unit tests, only 3 BCs (BC-1104..1106) — ratio ~25:1 LOC/BC, far higher than the 8:1 average. Round-trip invariants, table-render edge cases, code-block / heading / list parser semantics not BC'd.
- **Changelog filter pipeline.** 847 LOC, 38 unit tests, only 3 BCs (BC-119..121) — ratio ~14:1. AuthorNeedle classification (`:`, 12+ chars + digit), `--field` substring filter, `--reverse`, format-date observability gating not BC'd.
- **`extract_error_message` 6-level chain.** Pass 1 §8 deviation #7 flagged this as a load-bearing detail not in CLAUDE.md. Pass 3 BC-1201..1203 captures the HIGH-confidence version (12 unit tests). Adequate for now but worth a deepening round to capture all 6 fallback levels' edge cases.
- **`--no-input` TTY auto-detect logic.** Pass 3 BC-1103 marks this MEDIUM ("hard to test from `assert_cmd`, always non-TTY"). Behavior is universally relied upon but not directly tested.

### 5.3 Untested invariants (Pass 3 §3.5 — categorized for Phase B routing)

Pass 3 catalogued 7 untested invariants. Categorization:

| Invariant | Category | Why untested | Phase B route |
|---|---|---|---|
| INV-10 — `clear_profile_cache(name)` no-op for nonexistent dir | **not-yet-tested** | Has unit test but no integration test asserting it fires during `auth remove` flow | Add integration test in Phase B Pass 3 deepening |
| INV-11 — per-profile keychain key namespacing | **gated** | Most BCs `#[ignore]`-gated by `JR_RUN_KEYRING_TESTS=1` — don't fire by default in CI | Document gating in spec; add CI matrix that runs keyring tests on Linux/macOS where available |
| INV-12 — non-default profiles never inherit legacy keychain | **hard-to-test** | Asserted by absence (no `default` literal in fallback branches when `profile != "default"`); brittle | Add positive-side keyring integration test under `JR_RUN_KEYRING_TESTS` |
| INV-21 — `--open` issue list `statusCategory != Done` literal | **not-yet-tested** | Asserted only via JQL composition tests; no integration test does wiremock body-match for the literal fragment | Add integration test in Pass 3 deepening |
| INV-22 — `MAX_SPRINT_ISSUES = 50` cap | **not-yet-tested** | Has unit tests but no integration test passing 51+ keys | Add integration test |
| INV-24 — date validators run pre-HTTP | **hard-to-test** | Asserted only by clap-level rejection; no test passes valid command with bad date and counts HTTP requests | Add integration test using `Mock::expect(0)` |
| INV-25 — `--no-input` auto-set when stdin not TTY | **hard-to-test** | `assert_cmd` is always non-TTY | Document as expected-only; possibly defer to property-style or skip from holdouts |

### 5.4 NFR weaknesses without BC coverage (spec-level gaps)

These are NOT implementation bugs — they are intentional design gaps where no test exists because no test *should* exist (the absence is the spec):

1. **No 30s timeout on OAuth token-endpoint clients** (`api/auth.rs:607, 708`). Pass 4 §1.7 / §7.1.1.
2. **No PKCE in OAuth flow** (Pass 4 §7.2.6).
3. **No SBOM in CI** (Pass 4 §7.2.7).
4. **No release binary signing** (Pass 4 §7.2.8).
5. **No 401 auto-refresh in production paths** (Pass 4 §7.3.15) — the `refresh_oauth_token` function exists but has no callers; users hit 401 → must manually `jr auth refresh`.
6. **No upper bound on `Retry-After`** (Pass 4 §7.1.3) — a `Retry-After: 86400` would sleep 24h × 3 attempts.
7. **No HTTP-date format support in Retry-After** (Pass 4 §7.1.4).
8. **No FIPS-validated TLS** (rustls without `aws-lc-rs` feature; Pass 4 §7.2.10).
9. **`reqwest` default redirect policy is implicit** "up to 10 redirects" (Pass 4 §7.2.11) — not asserted, not configured.
10. **`cargo-deny multiple-versions = "warn"`** — duplicate transitive crate versions don't fail the build (Pass 4 §7.2.12).

These should be captured in the eventual Phase 1 spec as **explicit NFR decisions with rationale** ("we accept N+1 because…", "we defer PKCE because…"), not as implicit absences.

### 5.5 Pre-VSDD documentation drift candidates

Spotting inconsistencies between `docs/specs/` / `docs/superpowers/specs/` / `docs/adr/` and current code requires reading individual spec files (Pass 1-5 only inventoried them). Candidates flagged so far:

- **`docs/specs/list-rs-split.md`** — was acted on (view + comments split out), but `list.rs` has since grown to 1,083 LOC. The spec is "delivered" but the underlying design rule (shard at ~1000 LOC) is being violated again. **Drift type:** post-implementation regression on the design rule.
- **`docs/superpowers/specs/2026-04-30-embedded-oauth-app-design.md`** + **ADR-0006** — both reference the embedded XOR-obfuscation flow. Both should be cross-checked for whether they document `embedded_oauth_app_present()` (the no-decode presence check) and the `RedirectUriStrategyRequest::bind() → ResolvedRedirect` TOCTOU closure. (Pass B would verify.)
- **`docs/specs/multi-profile-auth.md`** — should be cross-checked against the actual lazy-migration logic in `api/auth.rs:111-169`. The convention "default lazy-migrates; non-default does NOT inherit legacy" is documented across passes but I have not personally verified it appears in the spec.
- **ADR-0002 (Superseded)** — should explicitly point to ADR-0006. Pass 1 §6.5 confirms supersession. Worth a re-read.
- **`docs/superpowers/plans/2026-03-21-jr-implementation.md`** (4,951 LOC) — the v1 implementation plan. Likely 90% delivered + 10% deferred or pivoted. Status field should be marked SUPERSEDED for Phase 1.

These are candidates only — Phase 1 (if user picks "harmonize") should run a doc-vs-code drift check more rigorously.

---

## 6. Phase B target list (priority-ordered)

Eight HIGH-priority targets, three MEDIUM, three LOW = **14 targets** total. Ordered by priority. The orchestrator can dispatch these in parallel within a tier.

### Phase B target T-01: Deepen `cli/auth.rs` (1,998 LOC) for OAuth state machine, refresh path, multi-profile entity coupling
**Priority**: HIGH
**Pass(es) to deepen**: 2, 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/cli/auth.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/auth_profiles.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/auth_refresh.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/auth_login_config_errors.rs`
**Why deepen**: The largest single subsystem; broad pass cataloged surface but not deep semantics. 44 inline unit tests not enumerated as BCs; OAuth credential resolver chain (flag → env → keychain → embedded → prompt for login; keychain → embedded for refresh) not characterized at function level; profile lifecycle state machine partially diagrammed.
**Expected output**: `pass-2-deep-auth.md` and `pass-3-deep-auth.md` — full BC enumeration covering all 44 unit tests + all keychain partial-state branches; per-function OAuth state machine; LoginArgs/RefreshArgs flow contract.

### Phase B target T-02: Deepen `cli/issue/list.rs` (1,083 LOC) for JQL composition rules
**Priority**: HIGH
**Pass(es) to deepen**: 2, 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/cli/issue/list.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/issue_commands.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/issue_list_errors.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/all_flag_behavior.rs`
**Why deepen**: The largest single CLI command file. Read first 115 LOC only in Pass 1. The unified JQL composition (project scope + `--open` → `statusCategory != Done` + asset clause + 4 date filters + `--status` partial-match + `--assignee`/`--reporter` me-resolution + `--team` resolution + `--recent` + ORDER BY) is the hottest correctness surface in the codebase.
**Expected output**: `pass-3-deep-issue-list.md` — JQL composition contract per branch; truncation hint behavior; story points / assets / team column conditional rendering; full BC enumeration.

### Phase B target T-03: Deepen the Assets/CMDB context (`cli/assets.rs` 1,055 LOC + `api/assets/*` 920 LOC + `types/assets/*` 779 LOC)
**Priority**: HIGH
**Pass(es) to deepen**: 2, 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/cli/assets.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/api/assets/`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/assets.rs` (1,799 LOC), `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/assets_errors.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/cmdb_fields.rs`
**Why deepen**: The N+1 enrichment risk (Pass 4 §1.5 / INC-12) needs verification — does extraction dedup by asset key? AQL escaping correctness needs property-style coverage. Workspace-id resolution corner cases (404 → "JSM Premium required" message; 403; partial workspace response) need full BC. `cli/assets.rs` is a candidate for the second sharding refactor (alongside `cli/auth.rs`).
**Expected output**: `pass-3-deep-assets.md` — full enumeration of `assets.rs` 21 unit tests + `linked.rs` 20 unit tests + workspace cache flow + AQL builder edge cases + ticket colorName filter.

### Phase B target T-04: Deepen all 324 integration tests for cross-cutting BC extraction
**Priority**: HIGH
**Pass(es) to deepen**: 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/cli_handler.rs` (2,134), `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/issue_commands.rs` (1,920), `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/api_client.rs` (444), all 36 test files
**Why deepen**: Pass 3 enumerated 188 BCs but the test corpus has 931 functions. Coverage ratio is ~20%. Full BC sweep across all 324 integration tests will surface BCs missed by the broad pass and tighten LOW-confidence ones. Particular focus: `cli_handler.rs` (the largest single test file at 2,134 LOC), `api_client.rs` (the cross-cutting HTTP-layer test surface), and `issue_changelog.rs` (1,722 LOC; only 3 BCs in Pass 3).
**Expected output**: `pass-3-deep-tests.md` — BC count target ≥300 HIGH (up from 134); LOW count → 0 (all promoted to HIGH/MEDIUM with tests); explicit gap list of behaviors with no test coverage.

### Phase B target T-05: Deepen the cache layer (`cache.rs` 899 LOC + per-profile boundary contracts)
**Priority**: HIGH
**Pass(es) to deepen**: 2, 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/cache.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/migration_legacy.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/issue_view_errors.rs` (corrupt teams.json case)
**Why deepen**: 27 inline unit tests, 7 BCs in Pass 3 (BC-1001..1010). Per-profile signature is a "soft fence" (Pass 1 §7 risk #6) — needs cross-profile leakage test enumeration. Cache miss policy (NotFound, expired, corrupt → all `Ok(None)`) is universal but not exhaustively BC'd. `clear_profile_cache` no-op invariant has unit test only.
**Expected output**: `pass-3-deep-cache.md` — full BC enumeration of all 6 cache categories + corruption-recovery scenarios + cross-profile boundary tests.

### Phase B target T-06: Deepen the rate-limit + 429 retry mechanism with property-test-style scenarios
**Priority**: HIGH
**Pass(es) to deepen**: 3, 4
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/api/client.rs` (lines 184-320), `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/api/rate_limit.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/api_client.rs:344-444`
**Why deepen**: 429 retry semantics are critical (BC-1401..1405) but Pass 3 noted 429-exhausted warning is MEDIUM confidence. The two send paths (`send` errors on 4xx/5xx; `send_raw` returns raw response) have subtle differences. `Retry-After` integer parsing has no upper bound (NFR gap §7.1.3). `try_clone()` "unreachable" panic should be verified.
**Expected output**: `pass-3-deep-rate-limit.md` — exhaustive scenario matrix (200/404/429+200/429×N/429+500/network-drop) for both `send` and `send_raw`; `Retry-After` boundary behavior.

### Phase B target T-07: Deepen `config.rs` (1,223 LOC) figment layering + profile resolution + migration
**Priority**: HIGH
**Pass(es) to deepen**: 2, 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/config.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/migration_legacy.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/auth_login_config_errors.rs`
**Why deepen**: Read at head only in Pass 1 (~450 LOC of 1,223). 37 inline unit tests. Profile-name validation runs at three boundaries (Pass 2 §2a.4) but each boundary's failure mode not exhaustively BC'd. Legacy migration write-back uses file-only baseline (Pass 2 INV-21) — no test passes JR_* env during migration to verify it doesn't bleed in.
**Expected output**: `pass-3-deep-config.md` — full enumeration of figment layer precedence + migration round-trip + validate_profile_name + Config::load_lenient_with vs Config::load_with.

### Phase B target T-08: Deepen `extract_error_message` 6-level chain + `JrError` dispatch
**Priority**: HIGH
**Pass(es) to deepen**: 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/api/client.rs:448-490`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/error.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/api_client.rs:257-342`
**Why deepen**: BC-1201..1203 cover the precedence chain at HIGH confidence but corner cases (mixed types in `errors{}`, nested `errors.field.messages[]`, empty body, raw text) have 12 unit tests in `tests/api_client.rs` not all enumerated. The chain is load-bearing — every Jira error message a user sees flows through it.
**Expected output**: `pass-3-deep-error-extraction.md` — 6-level chain diagram with all branch paths + every test scenario as a discrete BC.

### Phase B target T-09: Deepen `adf.rs` (1,826 LOC) — text↔ADF, markdown→ADF
**Priority**: MEDIUM
**Pass(es) to deepen**: 2, 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/adf.rs`
**Why deepen**: Second-largest source file. 69 inline unit tests, only 3 BCs in Pass 3 (BC-1104..1106). The ADF ↔ text/markdown engine is a hand-written DSL — its node types, paragraph/heading/list/code-block/table renderers, and link handling each merit BC enumeration.
**Expected output**: `pass-2-deep-adf.md` (entity model for ADF nodes) + `pass-3-deep-adf.md` (per-node rendering BC).

### Phase B target T-10: Deepen `cli/issue/changelog.rs` (847 LOC) — AuthorNeedle smart constructor + filter pipeline
**Priority**: MEDIUM
**Pass(es) to deepen**: 2, 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/cli/issue/changelog.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/issue_changelog.rs` (1,722 LOC)
**Why deepen**: 38 inline unit tests, ~38 integration tests in `issue_changelog.rs` (1,722 LOC), only 3 BCs in Pass 3. The `AuthorNeedle::classify` heuristic (`:` or 12+ chars with digit → AccountId; else NameSubstring) is uniquely complex — pinned by `docs/specs/author-needle-smart-constructor.md`.
**Expected output**: `pass-3-deep-changelog.md` — AuthorNeedle classification table; --field substring filter; --reverse semantics; format-date observability gating.

### Phase B target T-11: Deepen the OAuth state machine + 401 auto-refresh deferred-integration scope
**Priority**: MEDIUM
**Pass(es) to deepen**: 2, 4
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/api/auth.rs:545-895`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/api/auth.rs:700-770`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/tests/oauth_embedded_login.rs`
**Why deepen**: `refresh_oauth_token` exists `pub` but has no production callers (INC-07). The "clear-and-relogin" `auth refresh` flow vs the unused refresh-token grant is a non-trivial design choice. The deferred-integration scope (auto-refresh on 401) needs formal characterization for the eventual VSDD doc.
**Expected output**: `pass-2-deep-oauth-state.md` — full OAuth state machine + the gap between current and target (auto-refresh) behavior.

### Phase B target T-12: Deepen `partial_match.rs` + property tests
**Priority**: LOW
**Pass(es) to deepen**: 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/partial_match.rs`
**Why deepen**: 200 LOC, 12 unit tests including property tests. Single-substring → Ambiguous (not Exact) is a key invariant (INV-2). Resolver convention used by every "find by name" path. Worth promoting from convention-level (Pass 5) to discrete BC enumeration.
**Expected output**: Inline addendum to `pass-3-deep-utilities.md`.

### Phase B target T-13: Deepen `jql.rs` escaping + AQL building + property tests
**Priority**: LOW
**Pass(es) to deepen**: 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/jql.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/proptest-regressions/jql.txt`
**Why deepen**: 43 unit tests including property tests for `escape_value`, `validate_duration`, `validate_asset_key`, `validate_date`, `build_asset_clause`, `strip_order_by`. Pass 3 has BC-306..309 covering build_asset_clause; the rest are convention-level only. Property test corpus warrants explicit BC enumeration.
**Expected output**: Inline addendum to `pass-3-deep-utilities.md`.

### Phase B target T-14: Deepen `duration.rs` parser + property tests + cross-context comparison with JQL `validate_duration`
**Priority**: LOW
**Pass(es) to deepen**: 3
**Specific files / dirs**: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/duration.rs`, `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/src/jql.rs:16-33`
**Why deepen**: 16 unit tests + property tests. Two duration parsers with overlapping syntax and DIFFERENT acceptance (Pass 3 H-018) — easy footgun. Worth a discrete BC table contrasting them.
**Expected output**: Inline addendum to `pass-3-deep-utilities.md`.

**Total: 14 Phase B targets — 8 HIGH, 3 MEDIUM, 3 LOW.** The HIGH tier should converge first; MEDIUM and LOW can run later or in parallel after the HIGH tier produces no SUBSTANTIVE novelty. Auth and Issue contexts dominate the HIGH tier (T-01, T-02, T-04 directly; T-08 indirectly).

---

## 7. Recommendations for downstream skills

### 7.1 BC promotion strategy

**Promote directly into Phase 1** (high-confidence, evaluator-friendly, observable from outside the binary):
- All BC-1201..1214 (error handling, exit codes, remediation strings) — universal CLI contract.
- BC-007 (profile precedence flag > env > config > "default") — multi-profile contract.
- BC-012 (malformed config errors with exit 78 + does NOT overwrite file) — destructive-recovery safety.
- BC-103/104 (`--all` vs `--limit` cap; truncation hint) — pagination contract.
- BC-207 (`issue move` idempotent when current==target) — idempotency contract per CLAUDE.md.
- BC-1204 (exit code mapping) — sysexits.h contract.
- BC-606 (`IssueFields::team_id` accepts string-UUID and `{id: <uuid>}` object) — Atlas Teams shape tolerance.
- BC-306..308 (AQL clause uses field NAME + capital `Key`) — confirms two CLAUDE.md gotchas.

**Revisit in Phase B before promotion**: BC-013, BC-014, BC-022..024 (auth-related, mostly behind `#[ignore]` keyring tests). BC-118 (`--internal` JSM property; needs JSM-instance test).

### 7.2 Most evaluator-friendly holdout candidates (Phase 4)

From Pass 3's 20 holdout candidates, the most evaluator-friendly ones (clear setup, deterministic expectation, exit-code or stderr-text observable):

1. **H-001** (`auth status` first-run gives helpful guidance, not error) — empty XDG_CONFIG_HOME, exit 0, stderr `No profiles configured`.
2. **H-005** (malformed config TOML errors with exit 78 and does NOT overwrite the file) — bytewise file comparison, captures destructive-recovery safety.
3. **H-006** (`issue move FOO-1 "In Progress"` is idempotent when already in target) — wiremock POST `expect(0)`.
4. **H-008** (single-substring `--status` errors without firing JQL search) — wiremock JQL search `expect(0)`.
5. **H-013** (429 retry — `send_raw` returns 429 to caller after MAX_RETRIES=3) — `expect(4)` mock count.
6. **H-017** (AQL clause uses field NAME + capital `Key`) — pure unit test, no mock needed.
7. **H-019** (profile name `foo:bar` rejected at three boundaries) — security-boundary gate.

These have crisp setup, single-line expected output, and pin a contract that would silently break under common refactors.

### 7.3 ADRs the eventual VSDD architecture document should cross-reference

All 6 ADRs are still **architecturally authoritative**:
- **ADR-0001** (thin client) — drives the L4 resource-per-file convention.
- **ADR-0003** (reqwest + rustls-tls) — drives transport NFR + supply-chain shape.
- **ADR-0004** (per-feature specs) — drives the `docs/specs/` workflow that the eventual VSDD process should harmonize with.
- **ADR-0005** (GraphQL hostNames for org discovery) — drives the team subsystem.
- **ADR-0006** (embedded jr OAuth app + XOR obfuscation, re-supersedes ADR-0002) — drives the build-time codegen flow + per-build random key + threat model.

ADR-0002 (Superseded) should be retained for historical context but not used as a current decision input.

### 7.4 Specific architectural decisions that need to be made first

In priority order:
1. **Shard `cli/auth.rs` and `cli/assets.rs`.** Both violate the implicit module-size convention. Without a `docs/specs/auth-rs-split.md` and `docs/specs/assets-rs-split.md`, the eventual VSDD doc will inherit two ~1k-LOC handler monoliths. The Phase 1 spec should either codify the shard rule or codify the exception.
2. **Decide on the `refresh_oauth_token` future.** Either remove (clear-and-relogin is the only flow) or commit to integrating auto-refresh on 401 (NFR gap §7.3.15). Currently in limbo.
3. **Decide on PKCE for OAuth.** Defense-in-depth that's missing for an installed-app threat model. NFR gap §7.2.6.
4. **Decide on phantom-typed profile wrapper.** Pass 5 §9.2.4 flagged the soft-fence for per-profile cache signature. A `Profile(String)` newtype or `Cache<P>` phantom type would compile-time-enforce what's currently signature-discipline. Trade-off: more code; type-system-enforced correctness.
5. **Decide on the changelog observability layer.** `observability.rs` is 39 LOC; explicit deferral to "when there is cross-subsystem need" (Pass 4 §3.1). For AI-agent integration, a structured `tracing` layer would be valuable. Currently deferred — Phase 1 should make the deferral explicit.

### 7.5 Recommended Phase 1 entry treatment for pre-VSDD docs

**Default proposal: HARMONIZE.** Specifically:

- **`docs/adr/`** (6 files, 169 LOC): KEEP. ADRs 1, 3, 4, 5, 6 are still authoritative architecture decisions. ADR-0002 retained as Superseded historical record. The eventual VSDD doc cross-references ADR numbers without rewriting the ADR content.
- **`docs/superpowers/specs/2026-03-21-jr-jira-cli-design.md`** (668 LOC, v1 design spec): IMPORT AS HISTORICAL CONTEXT. This was the v1 design; the codebase has grown beyond it. Do not regenerate it; do not treat it as authoritative for current state; mark its status field as "Implemented in v1; see Phase 1 VSDD for current state."
- **`docs/superpowers/plans/`** (75 files, 56,572 LOC, mostly v1 implementation plans): SUPERSEDE. These are TDD-style red/green/refactor checklists for v1 features. The features are delivered. Mark the directory README (if added) as "Pre-VSDD plans, retained for archaeological reference. Current planning lives in TBD." Do not import as input to Phase 2 stories.
- **`docs/specs/`** (22 files, 3,778 LOC, post-v1 feature specs): TREAT AS ADDITIONAL INPUT CANDIDATES FOR PHASE 2 STORIES. ADR-0004 codifies this directory's role; the post-v1 specs should be checked one-by-one for whether they describe delivered behavior (then become validation inputs for the BC catalog) or planned behavior (then become Phase 2 story candidates). The `list-rs-split.md` is a good test case — it describes a delivered refactor, but the underlying "shard at ~1000 LOC" rule has regressed (INC-03), so it has dual value.

User may override with REFERENCE-ONLY (treat all four directories as historical, fully supersede with Phase 1 VSDD docs) or SUPERSEDE (delete pre-VSDD; rebuild from scratch). I recommend HARMONIZE because the ADRs and post-v1 specs encode real domain knowledge that would otherwise need to be rederived.

---

## 8. State Checkpoint

```yaml
pass: 6
status: complete
inconsistencies_found: 12
inconsistencies_resolved_now: 5      # INC-01, INC-02, INC-03, INC-04, INC-06 verified directly
inconsistencies_punted_to_phase_b: 0 # all framing-level INCs documented; nothing punted
phase_b_targets: 14
phase_b_high_priority: 8
phase_b_medium_priority: 3
phase_b_low_priority: 3
files_examined: 7                     # the 6 prior pass docs + CLAUDE.md + spot-verify in src/error.rs, src/cli/issue/list.rs, src/api/auth.rs
verification_actions:
  - awk_for_embedded_callback_port: confirmed in src/api/auth.rs:384 (not auth_embedded.rs)
  - wc_l_for_list_rs: confirmed 1083 LOC (CLAUDE.md says ~970)
  - read_full_error_rs: confirmed 11 JrError variants (Pass 1 said 10)
bounded_contexts_mapped: 14
nfr_gaps_unresolved: 10               # spec-level, no BC because no test
untested_invariants_categorized: 7
recommendations_to_orchestrator: 5    # BC promotion, holdouts, ADR cross-ref, decisions, pre-VSDD docs
default_pre_vsdd_treatment: HARMONIZE
timestamp: 2026-05-04T14:30:00Z
next_phase: B
```

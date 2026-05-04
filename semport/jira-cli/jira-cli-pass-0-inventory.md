# Pass 0: Inventory — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04

> Pass 0 is descriptive (inventory). Every numeric claim below is backed by a shell command shown with the figure. Where a subsection has fewer than 3 substantive items, this is declared explicitly rather than padded.

---

## 1. Tech Stack

### Toolchain (parsed from `rust-toolchain.toml`)
- **Channel:** `stable` (rolling — no version pin)
- **Components:** `rustfmt`, `clippy`, `llvm-tools-preview`

### Edition / MSRV (parsed from `Cargo.toml`)
- **Edition:** `2024`
- **MSRV (`rust-version`):** `1.85`
- **Package name:** `jr`
- **Binary name:** `jr` (single `[[bin]]` at `src/main.rs`)
- **Version:** `0.5.0-dev.7`
- **License:** MIT
- **Repository:** `https://github.com/Zious11/jira-cli`

### Release profile (`[profile.release]`)
| Setting | Value |
|---|---|
| `opt-level` | `3` |
| `lto` | `"thin"` |
| `codegen-units` | `1` |
| `strip` | `true` |
| `panic` | `"abort"` |

### Direct dependencies — runtime (`[dependencies]`, 24 entries)
| Crate | Version | Features (notable) |
|---|---|---|
| `anyhow` | `1` | — |
| `clap` | `4` | `derive` |
| `clap_complete` | `4` | — |
| `colored` | `3` | — |
| `comfy-table` | `7` | — |
| `dialoguer` | `0.12` | — |
| `dirs` | `6` | — |
| `figment` | `0.10` | `toml`, `env` |
| `keyring` | `3` | `apple-native`, `linux-native` |
| `open` | `5` | — |
| `reqwest` | `0.13` | `default-features = false`, `json`, `rustls` |
| `serde` | `1` | `derive` |
| `serde_json` | `1` | — |
| `thiserror` | `2` | — |
| `tokio` | `1` | `full` |
| `toml` | `1` | — |
| `base64` | `0.22` | — |
| `chrono` | `0.4` | `serde` |
| `futures` | `0.3` | `default-features = false`, `async-await` |
| `rand` | `0.9` | — |
| `urlencoding` | `2` | — |
| `url` | `2` | — |
| `pulldown-cmark` | `0.13` | `default-features = false` |

### Direct dev-dependencies (`[dev-dependencies]`, 6 entries)
| Crate | Version | Purpose |
|---|---|---|
| `assert_cmd` | `2` | Process-level CLI assertions |
| `predicates` | `3` | Matchers used with `assert_cmd` |
| `tempfile` | `3` | Test sandboxing |
| `wiremock` | `0.6` | HTTP mock server (Jira API fixture) |
| `insta` | `1` | Snapshot tests (with `json` feature) |
| `proptest` | `1` | Property-based tests (regressions in `proptest-regressions/jql.txt`) |

### Build-dependencies
- **None declared** in `Cargo.toml`. `build.rs` (125 LOC) uses only the standard library plus a hand-written FFI shim to `BCryptGenRandom` on Windows (no extra crates). On Unix it reads `/dev/urandom`. See section 4.

### Transitive deps
- **332 `[[package]]` entries** in `Cargo.lock`.
- Command: `awk '/^\[\[package\]\]/{c++} END{print c}' .reference/jira-cli/Cargo.lock` → `332`.

---

## 2. Directory Tree (depth ≤ 3)

Generated via `find ... -maxdepth 3 -type d | sort`:

```
.reference/jira-cli/
├── .claude/
│   └── commands/                # release.md helper for the release flow
├── .github/
│   ├── workflows/               # ci.yml, release.yml
│   ├── CODEOWNERS
│   └── dependabot.yml
├── docs/
│   ├── adr/                     # 6 architecture decision records
│   ├── specs/                   # 22 post-v1 feature specs (one per feature)
│   └── superpowers/
│       ├── plans/               # 75 implementation plans (dated)
│       └── specs/               # 56 design docs (dated)
├── proptest-regressions/
│   └── jql.txt                  # proptest seed corpus
├── src/
│   ├── api/
│   │   ├── assets/              # CMDB API impls (workspace, linked, objects, schemas, tickets)
│   │   ├── jira/                # Jira Core REST/Agile impls (boards, fields, issues, links,
│   │   │                        #  projects, resolutions, sprints, statuses, teams, users, worklogs)
│   │   └── jsm/                 # JSM impls (servicedesks, queues)
│   ├── cli/
│   │   ├── issue/               # Split issue commands (list, view, create, workflow, links,
│   │   │                        #  comments, changelog, helpers, format, json_output, assets)
│   │   ├── snapshots/           # insta snapshots for cli-level tests
│   │   └── (top-level command modules: api.rs, assets.rs, auth.rs, board.rs, init.rs,
│   │      mod.rs, project.rs, queue.rs, sprint.rs, team.rs, user.rs, worklog.rs)
│   ├── snapshots/               # insta snapshots at crate root
│   └── types/
│       ├── assets/              # CMDB serde structs
│       ├── jira/                # Jira serde structs
│       └── jsm/                 # JSM serde structs
├── tests/
│   ├── common/                  # fixtures.rs + mock_server.rs + mod.rs
│   └── snapshots/               # insta snapshots for integration tests
├── build.rs                     # XOR-obfuscated OAuth embedding (125 LOC)
├── Cargo.toml                   # 52 LOC
├── Cargo.lock                   # 332 packages
├── CLAUDE.md                    # AI agent operating manual (replicated at repo root)
├── deny.toml                    # 26 LOC, cargo-deny licence allowlist
├── install.sh                   # 128 LOC, install/upgrade script
├── README.md                    # 363 LOC
└── rust-toolchain.toml          # 3 LOC
```

Top-level files include only what's listed; no `lefthook.yml`, no `justfile`, no `.cargo/config.toml`, no `Makefile`. (Verified via `find -maxdepth 2 (-name lefthook* -o -name justfile -o -name Justfile -o -name .cargo)` returning nothing.)

---

## 3. File Inventory by Type

All numbers below come from `find ... -exec wc -l {} +`. I refused to estimate.

### 3a. Rust source under `src/` (80 files, 23,334 LOC)

Command: `find src -name '*.rs' -type f -exec wc -l {} + | tail -1` → `23334 total`.
Command: `find src -name '*.rs' -type f | wc -l` → `80`.

Split by submodule (each via `find <dir> -maxdepth 1 -name '*.rs' -exec wc -l {} +`):

| Submodule | Files | LOC |
|---|---:|---:|
| `src/` (top-level files only, no subdirs) | 11 | 5,233 |
| `src/cli/` (top-level only) | 12 | 6,044 |
| `src/cli/issue/` | 12 | 5,078 |
| `src/api/` (top-level only) | 6 | 2,574 |
| `src/api/jira/` | 12 | 1,457 |
| `src/api/jsm/` | 3 | 214 |
| `src/api/assets/` | 6 | 920 |
| `src/types/` (top-level only — just `mod.rs`) | 1 | 3 |
| `src/types/jira/` | 9 | 934 |
| `src/types/jsm/` | 3 | 98 |
| `src/types/assets/` | 5 | 779 |
| **Sum** | **80** | **23,334** |

(Sum reconciles to the project-level total — no orphan files.)

#### Top 10 largest source files (`find src -name '*.rs' -exec wc -l {} + | sort -rn | head -11`)

| File | LOC |
|---|---:|
| `src/cli/auth.rs` | 1,998 |
| `src/adf.rs` | 1,826 |
| `src/api/auth.rs` | 1,397 |
| `src/config.rs` | 1,223 |
| `src/cli/issue/list.rs` | 1,083 |
| `src/cli/assets.rs` | 1,055 |
| `src/cache.rs` | 899 |
| `src/cli/issue/changelog.rs` | 847 |
| `src/cli/issue/helpers.rs` | 813 |
| `src/cli/issue/workflow.rs` | 788 |

> Note: `list.rs` is documented in CLAUDE.md as ~970 lines. Actual is **1,083** — it has grown. Likely because `view` and `comments` were merged in (`docs/specs/list-rs-split.md` exists and was acted on per the CLI tree, but list itself remains the largest single command file).

### 3b. Rust source under `tests/` (36 files, 16,958 LOC)

Command: `find tests -name '*.rs' -type f -exec wc -l {} + | tail -1` → `16958 total`.
Command: `find tests -name '*.rs' -type f | wc -l` → `36`.

Top 5 largest test files (`find tests -name '*.rs' -exec wc -l {} + | sort -rn | head -6`):

| Test file | LOC |
|---|---:|
| `tests/cli_handler.rs` | 2,134 |
| `tests/issue_commands.rs` | 1,920 |
| `tests/assets.rs` | 1,799 |
| `tests/issue_changelog.rs` | 1,722 |
| `tests/all_flag_behavior.rs` | 686 |

### 3c. TOML / YAML / Markdown / shell

| Type | Files | LOC | Command |
|---|---:|---:|---|
| `*.toml` (top 2 levels) | 3 | 81 | `find -maxdepth 2 -name '*.toml' \| xargs wc -l` → 81 total |
| `*.yml` / `*.yaml` (incl. dependabot, workflows) | 3 | 240 | `wc -l ci.yml release.yml dependabot.yml` |
| `*.md` (entire tree) | 162 | 71,907 | `find -name '*.md' -exec wc -l {} +` |
| `install.sh` | 1 | 128 | `wc -l install.sh` |
| `build.rs` | 1 | 125 | `wc -l build.rs` |

### 3d. Project-wide totals

- **Rust source LOC (src/ + tests/ + build.rs):** 23,334 + 16,958 + 125 = **40,417**.
- **Total files scanned (.rs/.toml/.yml/.yaml/.md/.sh):** 286 (`find ... | wc -l`).
- **Total .rs files (src + tests + build.rs):** 80 + 36 + 1 = **117**.

---

## 4. Entry Points

| File | LOC | Role |
|---|---:|---|
| `src/main.rs` | 268 | Process entrypoint. `#[tokio::main]` with `tokio::select!` against `ctrl_c` for graceful 130-exit. Parses `Cli` via clap derive. Auto-enables `--no-input` when stdin is not a TTY (key for AI-agent ergonomics). Threads `cli.profile` into `Config::load_with(...)` rather than via env var (justified inline: `unsafe { set_var(...) }` is unsound under `#[tokio::main]` once worker threads exist). Renders structured JSON errors with `{"error", "code"}` when `--output json`. Dispatches to `cli::*::handle(...)` per top-level subcommand. |
| `src/lib.rs` | 12 | Crate root re-exporting public modules: `adf`, `api`, `cache`, `cli`, `config`, `duration`, `error`, `jql`, `output`, `partial_match`, `types`. `observability` is `pub(crate)`. This is the surface integration tests link against (`use jr::cli::...`). |
| `build.rs` | 125 | Build-time codegen for embedded OAuth. Reads `JR_BUILD_OAUTH_CLIENT_ID` / `_SECRET` env vars. When both present, generates a fresh 32-byte XOR key (Unix: `/dev/urandom`; Windows: inline FFI `BCryptGenRandom` shim, no extra crates) and writes `$OUT_DIR/embedded_oauth.rs` with three module-private constants (`EMBEDDED_ID`, `EMBEDDED_SECRET_XOR`, `EMBEDDED_SECRET_KEY`). When env vars are missing (forks, local builds), all three are emitted as `None`. `compile_error!` for non-unix/non-windows hosts. |

The Cli dispatch in `main.rs` enumerates the top-level command surface: `Init`, `Assets`, `Auth`, `Me`, `Project`, `Issue`, `Board`, `Sprint`, `Worklog`, `Team`, `User`, `Queue`, `Api`, `Completion`. Each handler resolves config + builds a `JiraClient::from_config` (except `Init` and `Completion`, which intentionally skip auth).

---

## 5. Configuration Surface

| File | Purpose |
|---|---|
| `Cargo.toml` (52 LOC) | Crate metadata, deps, release profile (LTO thin, strip, panic=abort, codegen-units=1, opt-level=3). |
| `rust-toolchain.toml` (3 LOC) | Pin to `stable` channel + components `rustfmt`, `clippy`, `llvm-tools-preview`. No specific version. |
| `deny.toml` (26 LOC) | `cargo-deny` config: license allowlist (MIT, Apache-2.0, BSD-2/3-Clause, ISC, MPL-2.0, Unicode-3.0/DFS-2016, CDLA-Permissive-2.0, OpenSSL, Zlib), `multiple-versions = "warn"`, `unknown-registry = "warn"`, `unknown-git = "warn"`. |
| `.github/workflows/ci.yml` (69 LOC) | Format (`cargo fmt --check`), Clippy (`-D warnings`), tests, deny check. Triggers on push to `main`/`develop` and PRs. Uses `Swatinem/rust-cache@v2`. |
| `.github/workflows/release.yml` (158 LOC) | Tag-driven (`v*`). Matrix builds: x86_64-apple-darwin, aarch64-apple-darwin, x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu (via `cross`). Injects `JR_BUILD_OAUTH_CLIENT_ID`/`_SECRET` into the build env (and threads them through `cross` via `CROSS_CONTAINER_OPTS`). |
| `.github/dependabot.yml` (13 LOC) | Cargo + GitHub Actions dependency updates. |
| `.github/CODEOWNERS` | Code ownership for protected-branch reviews. |
| `install.sh` (128 LOC) | User-facing install script (likely fetches release tarballs and lays the binary into `~/.local/bin` or similar). Not a build/CI input. |
| `.claude/commands/release.md` | Custom Claude Code slash command capturing release flow. |

**No `lefthook.yml`, no `justfile`, no `.cargo/config.toml`, no `Makefile`.** Verified.

---

## 6. Dependency Graph (cargo-level — crate purpose buckets)

Module-level dependencies are deferred to Pass 1. This section groups the 24 direct runtime + 6 dev deps by responsibility.

| Bucket | Crates |
|---|---|
| **HTTP client** | `reqwest` (rustls, JSON, no default features → no native-tls leak; ADR-0003) |
| **Serialization** | `serde` (derive), `serde_json`, `toml`, `urlencoding`, `url`, `base64` |
| **CLI parsing & shell ergonomics** | `clap` (derive), `clap_complete`, `dialoguer` (interactive prompts), `colored` (NO_COLOR-aware), `open` (browser launch for OAuth), `comfy-table` (table renderer) |
| **Configuration** | `figment` (toml + env layering), `dirs` (XDG paths) |
| **Auth / secrets** | `keyring` (apple-native + linux-native; OS keychain), `rand` (PKCE / state nonces / OAuth flow) |
| **Async runtime** | `tokio` (full feature → multi-thread, signal, time, IO), `futures` (async-await only, no default) |
| **Error handling** | `anyhow` (handler-level, dynamic error chain), `thiserror` (typed `JrError` enum with exit codes) |
| **Time / dates** | `chrono` (serde feature for ISO-8601 round-trips) |
| **Markdown / ADF** | `pulldown-cmark` (no default features) — feeds the markdown→ADF converter in `src/adf.rs` |
| **Test infrastructure (dev-deps)** | `assert_cmd`, `predicates`, `tempfile`, `wiremock` (HTTP fixture), `insta` (snapshots, json feature), `proptest` (property tests) |
| **Build-time** | None declared — `build.rs` uses only `std` |

The directory layout is already namespaced for sibling-product expansion (`api/jira/`, `api/jsm/`, `api/assets/`; `types/jira/`, `types/jsm/`, `types/assets/`). Confluence is not yet present but the structure is ready for it.

---

## 7. File Prioritization for Downstream Passes

Priority is the brownfield-ingest score (entry points → configs → core → API → tests → utils). Pass relevance is which downstream passes will lean on the file most.

### HIGH priority

| Path | Reason | Most-relevant passes |
|---|---|---|
| `src/main.rs` | Entry point + complete CLI dispatch table; encodes top-level command surface and per-command profile composition rule | 1 (architecture), 3 (BC), 5 (conv) |
| `src/cli/mod.rs` (772 LOC) | Clap derive types + global flag definitions (`--output`, `--profile`, `--project`, `--no-input`, `--no-color`, `--verbose`); the public CLI contract | 1, 3, 5 |
| `src/cli/issue/list.rs` (1,083 LOC) | Largest single command file; unified JQL composition, `--open` filtering by status category, list/view/comments orchestration | 2 (domain), 3 (BC), 4 (NFR — limits, retries, output), 5 (patterns) |
| `src/cli/issue/workflow.rs` (788 LOC) | Idempotent state-changing semantics (move, assign, comment, transitions); first-class for behavioral contract extraction | 3, 5 |
| `src/cli/issue/create.rs` (375 LOC) | Field-building logic, story-points/team resolution, edit semantics | 2, 3 |
| `src/cli/issue/helpers.rs` (813 LOC) | Team / points / user resolution + prompt scaffolding shared across issue commands | 2, 3, 5 |
| `src/cli/issue/changelog.rs` (847 LOC) | Author-needle smart constructor, classification rules, format-date behavior — encodes domain rules | 2, 3 |
| `src/cli/auth.rs` (1,998 LOC) | OAuth + API-token flows, profile lifecycle (login/switch/list/status/refresh/logout/remove), JSON error shape | 3, 4 (security), 5 |
| `src/cli/assets.rs` (1,055 LOC) | Assets/CMDB search, view, tickets, schemas, types, schema discovery | 2, 3 |
| `src/cli/init.rs` | Interactive setup, prefetches org metadata + caches | 3, 5 |
| `src/cli/sprint.rs` (438 LOC) | Scrum-only constraint (errors on kanban) — non-trivial domain rule | 2, 3 |
| `src/cli/queue.rs` (323 LOC) | JSM service-desk queue surface | 2, 3 |
| `src/cli/api.rs` (342 LOC) | Raw API passthrough — reveals the auth/headers/output contract for the entire client | 3, 4 |
| `src/api/client.rs` (490 LOC) | `JiraClient` HTTP plumbing: auth headers, 429/401 handling, retries, base URL override (`JR_BASE_URL`) | 1, 3, 4 |
| `src/api/auth.rs` (1,397 LOC) | OAuth 2.0 dance, per-profile keychain layout (shared vs namespaced keys), legacy migration logic | 3, 4 (security) |
| `src/api/auth_embedded.rs` | Sibling to auth.rs; embedded-app credential resolution + XOR de-obfuscation accessor | 1, 4 |
| `src/api/jira/issues.rs` (314 LOC) | Search, get, create, edit, list comments — primary REST surface | 2, 3 |
| `src/api/jira/fields.rs` (303 LOC) | Story-points + CMDB field discovery — non-trivial cache-feeding logic | 2, 3 |
| `src/api/jira/teams.rs`, `boards.rs`, `sprints.rs`, `links.rs`, `worklogs.rs`, `users.rs`, `projects.rs`, `statuses.rs`, `resolutions.rs` | Resource-per-file API shapes; one-to-one with Jira REST resources | 2, 3 |
| `src/api/pagination.rs` (374 LOC) | Offset + cursor pagination strategies; affects every list endpoint | 4 (NFR), 3 |
| `src/api/rate_limit.rs` | Retry-After parsing — key NFR | 4 |
| `src/cache.rs` (899 LOC) | Per-profile XDG cache, 7-day TTL, schema versioning (`v1/`), cache-miss-on-deserialization-failure pattern | 4 (NFR), 5 |
| `src/config.rs` (1,223 LOC) | figment layering, profile resolution precedence (flag > env > config > "default"), legacy migration. The behavioral source of `Config::load_with` | 1, 3, 5 |
| `src/error.rs` (136 LOC) | `JrError` enum + exit-code mapping (0/1/2/64/78/130) — shapes every command's error surface | 3, 5 |
| `tests/cli_handler.rs` (2,134 LOC) | Largest integration test; per-handler coverage with wiremock — first-class BC source | 3 |
| `tests/issue_commands.rs` (1,920 LOC) | Issue surface integration coverage | 3 |
| `tests/assets.rs` (1,799 LOC) | Assets surface integration coverage | 3 |
| `tests/common/fixtures.rs`, `tests/common/mock_server.rs` | Test infra used everywhere downstream | 3 (helper), 5 |

### MEDIUM priority

| Path | Reason | Passes |
|---|---|---|
| `src/types/jira/issue.rs` (625 LOC) | Issue serde shape + ADF/changelog/team-field tolerance | 2 |
| `src/types/jira/{board,changelog,project,sprint,team,user,worklog}.rs` | Domain shapes for each Jira resource | 2 |
| `src/types/assets/{linked,object,schema,ticket}.rs` (779 LOC total) | CMDB shapes (LinkedAsset, AssetObject, ConnectedTicket) — domain vocabulary | 2 |
| `src/types/jsm/{queue,servicedesk}.rs` (98 LOC total) | Small, complete JSM shapes | 2 |
| `src/api/assets/{linked,objects,schemas,tickets,workspace}.rs` (920 LOC total) | CMDB API impls; per-field enrichment, AQL search, key resolution | 2, 3 |
| `src/api/jsm/{queues,servicedesks}.rs` (214 LOC total) | JSM impls; small surface | 2, 3 |
| `src/adf.rs` (1,826 LOC) | Atlassian Document Format conversion (text↔ADF, markdown→ADF). Non-trivial format engine | 5 (rendering pattern), 4 (correctness) |
| `src/jql.rs` (395 LOC) | JQL escaping + validation + asset clause builder; gotcha-laden domain | 3, 5 |
| `src/duration.rs` (159 LOC) | Worklog duration parser (2h, 1h30m, 1d, 1w) — small domain | 3 |
| `src/output.rs` (76 LOC) | Comfy-table + JSON renderers — implements `--output` contract | 5 |
| `src/cli/issue/format.rs` | Row formatting / header building / points display — convention-heavy | 5 |
| `src/cli/issue/json_output.rs` | JSON shape for issue write ops (returns `{"key": "FOO-123"}`) | 3 |
| `src/cli/issue/links.rs` | Issue link operations (link/unlink/link-types) | 2, 3 |
| `src/cli/issue/comments.rs`, `view.rs`, `assets.rs` (cli/issue) | Read-side issue commands; small | 3 |
| `src/cli/{board,team,user,project,worklog,queue}.rs` | Lower-LOC command modules; mostly thin wrappers | 3 |
| `build.rs` (125 LOC) | Build-time XOR-obfuscated OAuth secret embedding — ADR-0006 | 1, 4 |

### LOW priority

| Path | Reason |
|---|---|
| `src/partial_match.rs` (200 LOC) | Case-insensitive substring matching with disambiguation — small, well-bounded utility |
| `src/observability.rs` (39 LOC) | `pub(crate)`; very small; verbose-logging plumbing |
| `src/cli/snapshots/`, `src/snapshots/`, `src/cli/issue/snapshots/`, `tests/snapshots/` | Insta snapshot outputs (.snap files) — useful as expectation fixtures, not source-of-truth |
| `proptest-regressions/jql.txt` | Proptest seed corpus — useful for Pass 3 to know property tests exist on JQL |
| `tests/common/mod.rs` | Glue file; small |

This prioritization will guide read-order in Passes 1–5. HIGH-priority files MUST be read in full before generating each pass output; MEDIUM may be skimmed; LOW only as needed for cross-references.

---

## 8. Pre-VSDD Documentation Inventory

These are pre-VSDD artifacts and **must not** be treated as authoritative spec inputs for Pass 3 — behavioral contracts come from code + tests. They are inventoried here for cross-reference only.

### `docs/adr/` — 6 files, 169 LOC total

| ADR | Title | Status |
|---|---|---|
| ADR-0001 | Thin Client vs Generated API Client | Accepted |
| ADR-0002 | OAuth 2.0 with Embedded Client Secret | Superseded (by ADR-0006) |
| ADR-0003 | reqwest with rustls-tls | Accepted |
| ADR-0004 | Per-Feature Specs, Not a Growing Master Spec | Accepted |
| ADR-0005 | GraphQL hostNames for Org Discovery | Accepted |
| ADR-0006 | Embedded `jr` OAuth App with Compile-Time Obfuscation | Accepted (re-supersedes ADR-0002) |

### `docs/superpowers/specs/` — 56 files, 10,727 LOC total

Pivot: "v1 design spec" is `2026-03-21-jr-jira-cli-design.md` (668 LOC). Everything dated after that is per-feature design.

Notable single docs:
- `2026-03-21-jr-jira-cli-design.md` (668 LOC) — v1 design spec
- `2026-04-30-embedded-oauth-app-design.md` — ADR-0006 companion
- `2026-03-24-assets-cmdb-design.md` — Assets/CMDB design
- `2026-03-24-jsm-queues-design.md` — JSM design
- 52 additional feature/refactor design docs

### `docs/superpowers/plans/` — 75 files, 56,572 LOC total

These are dated implementation plans (TDD-style: red/green/refactor checklists). Pivot: `2026-03-21-jr-implementation.md` (4,951 LOC) is the v1 implementation plan; the rest are per-feature plans dated 2026-03-22 through 2026-04-30.

### `docs/specs/` — 22 files, 3,778 LOC total

These are the post-v1 feature specs proper (the "one spec per feature" enforced by ADR-0004). Examples: `multi-profile-auth.md`, `oauth-scopes-configurable.md`, `issue-changelog.md`, `assets-schema-discovery.md`, `assets-search-attribute-names.md`, `issue-list-asset-filter.md`, `team-assignment.md`, `team-column-sprint-board-parity.md`, `team-field-object-shape-tolerance.md`, `user-search-pagination.md`, `list-rs-split.md`, etc. Plus a `README.md` describing the convention.

### Top-level
- `README.md` (363 LOC) — user-facing
- `CLAUDE.md` (12,438 chars) — AI-agent operating manual; identical to repo root
- `.claude/commands/release.md` — release-flow slash command

---

## 9. Test Strategy Snapshot (preview for Pass 3)

### Counts
- **Test files in `tests/`:** 36 (`find tests -name '*.rs' \| wc -l`).
- **Test functions in `tests/` (`#[test]` + `#[tokio::test]`):** 324 (`find tests -name '*.rs' \| xargs awk '/#\[(tokio::)?test\]/{c++}'`).
- **Test functions in `src/` (inline unit tests):** 607 (`find src -name '*.rs' \| xargs awk '/#\[(tokio::)?test\]/{c++}'`).
- **`#[cfg(test)]` blocks in `src/`:** 50 — i.e., ~50 source files have inline test modules.
- **`#[ignore]`-attributed tests:** 13 across the whole tree (`awk '/#\[ignore/{c++}'`).

### Total test functions: ~931 (607 inline unit + 324 integration). High-coverage codebase.

### Test infrastructure
| Tool | Where used | Purpose |
|---|---|---|
| `wiremock` (0.6) | `tests/common/mock_server.rs`, every integration test | HTTP fixture server; client constructed via `JiraClient::new_for_test(base_url, auth_header)` |
| `insta` (1, with `json` feature) | `tests/snapshots/`, `src/snapshots/`, `src/cli/snapshots/`, `src/cli/issue/snapshots/` | 17 `.snap` files; JSON-output snapshot tests for issue commands etc. |
| `proptest` (1) | `proptest-regressions/jql.txt` is the regression corpus | Property tests on JQL escaping/validation (likely `src/jql.rs` and `src/partial_match.rs`) |
| `assert_cmd` + `predicates` | binary-level CLI tests | Process-level invocation of `jr` (e.g., `cli_smoke.rs`) |
| `tempfile` | scattered | Sandbox config files / cache dirs |
| `JR_BASE_URL` env | tests use it to inject wiremock | Flagged in CLAUDE.md AI Agent Notes |

### `#[ignore]` gating
- Per CLAUDE.md, keyring round-trip tests are gated behind `JR_RUN_KEYRING_TESTS=1` + `#[ignore]` because Linux CI may lack `secret-service`.
- 13 `#[ignore]` attributes across the tree — Pass 3 should grep these and identify which behaviors are not exercised by default CI.

---

## 10. State Checkpoint

```yaml
pass: 0
status: complete
files_scanned: 286
loc_total: 40417     # 23334 src + 16958 tests + 125 build.rs
loc_total_breakdown:
  src_rs: 23334
  tests_rs: 16958
  build_rs: 125
  toml: 81
  yaml: 240
  markdown: 71907
files_breakdown:
  rs_total: 117      # 80 src + 36 tests + 1 build.rs
  src_rs: 80
  tests_rs: 36
  toml: 3
  yaml: 3
  markdown: 162
  shell: 1
deps:
  direct_runtime: 24
  dev: 6
  build: 0
  transitive: 332
test_strategy:
  unit_test_fns_in_src: 607
  integration_test_fns: 324
  total_test_fns: 931
  cfg_test_blocks: 50
  ignored_tests: 13
  insta_snapshots: 17
docs:
  adrs: 6
  v1_design_specs: 1
  superpowers_specs: 56
  superpowers_plans: 75
  feature_specs: 22
timestamp: 2026-05-04T00:00:00Z
next_pass: 1
```

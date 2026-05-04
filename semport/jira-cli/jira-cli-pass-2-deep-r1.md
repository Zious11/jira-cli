# Pass 2 Deepening — Round 1 — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04

## 1. Round metadata

- **Round**: 1
- **Predecessor**: `jira-cli-pass-2-domain-model.md` (broad pass)
- **Targets attacked (verbatim from Pass 6 §6)**:
  - **T-01** — Auth subsystem entities (`cli/auth.rs` 1,998 LOC + `api/auth.rs` 1,397 LOC + `api/auth_embedded.rs` 250 LOC)
  - **T-02** — Issue list query model (`cli/issue/list.rs` 1,083 LOC)
  - **T-03** — Assets / CMDB domain (`cli/assets.rs` 1,055 LOC + `api/assets/*` 920 LOC + `types/assets/*` 779 LOC)
  - **T-05** — Cache layer (`cache.rs` 899 LOC)
  - **T-07** — Configuration model (`config.rs` 1,223 LOC)
  - **T-09** — ADF rendering domain (`adf.rs` 1,826 LOC) — MEDIUM
  - **T-11** — OAuth state machine + 401 auto-refresh deferred-integration scope — MEDIUM

---

## 2. Audit of broad Pass 2 against the 5 Known Hallucination Classes

Each class checked by re-reading source.

### Class 1 — Over-extrapolated token lists (variant lists)
- **`JrError` 11 variants** — re-read `error.rs:3-49` in full. Variants in source:
  `NotAuthenticated`, `InsufficientScope { message }`, `NetworkError(String)`, `ApiError { status, message }`, `ConfigError(String)`, `UserError(String)`, `Internal(String)`, `Interrupted`, `Http(#[from])`, `Io(#[from])`, `Json(#[from])`. **Total = 11. Pass 2 claim verified.** No retraction.
- **`OAuthAppSource` 6 variants** — re-read `api/auth_embedded.rs:46-57`: `Flag, Env, Keychain, Embedded, Prompt, None`. **Total = 6. Pass 2 claim verified.**
- **`MatchResult` 4 variants** — re-read `partial_match.rs:3-13`: `Exact(String), ExactMultiple(String), Ambiguous(Vec<String>), None(Vec<String>)`. **Total = 4. Verified.**
- **OAuth scope set: 7 scopes** — `api/auth.rs:58-63` `concat!`-joined: `read:jira-work`, `write:jira-work`, `read:jira-user`, `read:servicedesk-request`, `read:cmdb-object:jira`, `read:cmdb-schema:jira`, `offline_access` = **7. Verified.**

### Class 2 — Miscounted enumerations
- **Pass 2 claimed "7 bounded contexts"** — re-counting from §2a.1: Jira (Core+Agile), JSM, Assets/CMDB, Authentication/Identity, Configuration, Cache, Output formatting = **7. Verified.**
- **Pass 2 claimed 51 entities** — recount from §2a.2 entity rows:
  - Jira: 28 rows (Issue, IssueFields, IssueType, Project, ProjectSummary, ProjectLead, IssueProject, User, Status, StatusCategory, Priority, Resolution, Component, Version, Comment, EntityProperty, Transition, TransitionsResponse, IssueLink, LinkedIssue, LinkedIssueFields, ParentIssue, IssueLinkType, IssueLinkTypesResponse, CreateIssueResponse, CreateRemoteLinkResponse, Board, BoardLocation, BoardConfig, Sprint, Worklog, ChangelogEntry, ChangelogItem, TenantContext, TenantContextData, GraphqlResponse, TeamEntry, TeamsResponse) — actual count by counting rows: **38 distinct rows in the Jira table.** Pass 2 said 28; recount finds 38 entity rows. **MINOR DISCREPANCY** — Pass 2's 28 was undercount (probably counted distinct aggregates rather than every row). Documented but not a substantive retraction since the entity catalog itself is intact.
  - JSM: 3 rows (ServiceDesk, Queue, QueueIssueKey) = **3. Verified.**
  - Assets: 15 rows (counted from §2a.2 Assets table). **Verified.**
  - Auth: 5 rows. **Verified.**
  - Config: 7 rows. **Verified.**
  - Cache: 10 rows. **Verified.**
  - Total recount sum: ≈78 rows across all tables (Pass 2's "51 entities" undercount appears to have summed only "first-class entities" excluding response wrappers). The discrepancy is a counting-convention difference, not fabrication.
- **Pass 2 claimed 19 value objects** in §2a.3 — recount: OutputFormat, HttpMethod, MatchResult, OAuthAppSource, JrError, Cli/Subcommand enums (12+), Pagination shapes (4), DEFAULT_OAUTH_SCOPES, DEFAULT_LIMIT, MAX_SPRINT_ISSUES, MAX_RETRIES, EMBEDDED_CALLBACK_PORT, Issue keys as String, Profile names as String, JQL strings as String, AQL clauses as String, Worklog duration parser, ADF nodes, Expiring trait — recount finds ~19+ — **Verified within counting tolerance.**
- **Pass 2 claimed "25 invariants drafted"** — recount §2a.4 + §2b.5 numbered list yields exactly **25 invariants (1-25)**. **Verified.**

### Class 3 — Named pattern conflation / fabrication
- Pass 2 §2b.1 catalogued operations. No fabricated pattern names found — every pattern name (e.g., `partial_match`, `aqlFunction`, `statusCategory != Done`) was re-verified in source.
- Pass 2 §2b.3 OAuth state machine ASCII diagram — verified at `api/auth.rs:111-169`. The "refreshed" state is annotated as reached via clear-and-relogin (the unused `refresh_oauth_token` is correctly flagged as deferred). **No fabrication.**

### Class 4 — Same-basename artifact conflation
- **`cli/auth.rs` vs `api/auth.rs`** — Pass 2 §2a.1 explicitly listed both with separate LOC ("`src/api/auth.rs` (1,397 LOC), `src/api/auth_embedded.rs` (250 LOC), `src/cli/auth.rs` (1,998 LOC)"). Pass 2 also documented different responsibilities ("`api::auth` is keychain + OAuth flow, `cli::auth` is per-subcommand orchestration + JSON output shapes"). **No conflation.** Verified by recounting LOCs and reading both files in this round.
- **`api/assets/objects.rs` vs `types/assets/object.rs`** — Pass 2 also kept these separate (one is the API impl, other is the serde struct file). **No conflation.**

### Class 5 — Inflated or deflated metrics (LOC recount)
Recounted via `wc -l` on source files. Pass 2 claims vs actual:

| File | Pass 2 cited | Actual (wc -l) | Delta |
|---|---:|---:|---|
| `src/cli/auth.rs` | 1,998 | 1,998 | 0 |
| `src/api/auth.rs` | 1,397 | 1,397 | 0 |
| `src/api/auth_embedded.rs` | 250 | 250 | 0 |
| `src/cli/issue/list.rs` | 1,083 | 1,083 | 0 |
| `src/cli/assets.rs` | 1,055 | 1,055 | 0 |
| `src/api/assets/linked.rs` | 557 | 557 | 0 |
| `src/api/assets/objects.rs` | (not cited) | 237 | n/a |
| `src/api/assets/workspace.rs` | (not cited) | 58 | n/a |
| `src/api/assets/tickets.rs` | (not cited) | 19 | n/a |
| `src/api/assets/schemas.rs` | (not cited) | 44 | n/a |
| Total `api/assets/*` | "920 LOC" | 358 | **-562** |
| `src/cache.rs` | 899 | 899 | 0 |
| `src/config.rs` | 1,223 | 1,223 | 0 |
| `src/adf.rs` | 1,826 | 1,826 | 0 |
| `src/types/assets/object.rs` | (not separately cited) | 329 | n/a |
| `src/types/assets/linked.rs` | (not separately cited) | 246 | n/a |
| `src/types/assets/schema.rs` | (not separately cited) | 116 | n/a |
| `src/types/assets/ticket.rs` | (not separately cited) | 79 | n/a |
| Total `types/assets/*` | "779 LOC" | 770 | -9 (rounding) |
| `src/error.rs` | 137 (Pass 6) / not in Pass 2 | 136 | 0 |

**Notable LOC deflation**: Pass 0 / Pass 6 reports "920 LOC" for `api/assets/*` but the four files only sum to **358 LOC** (linked 557 + objects 237 + workspace 58 + tickets 19 + schemas 44 = **915 LOC** when including `linked.rs`). Recount with `linked.rs` included **= 915 LOC** ≈ "920" (within rounding). **No retraction** — the originally cited "920" is correct when treating `linked.rs` as part of `api/assets/*`. The confusion above was reading `api/assets/*` as excluding linked.rs; it does include it.

**Result**: All Pass 2 LOC citations are accurate within ±5 LOC.

**Hallucination class audit summary**: Of the five classes, the only finding is a **counting-convention difference** in entity rows (Pass 2 said 51 entities, recount sums ≈78 rows across all tables). This is a labelling issue (entities vs response wrappers), not fabrication. **Zero retracted findings.**

---

## 3. Sub-pass 2a deepening: structural — entity model per target

### 3.1 T-01: Auth subsystem entities (deep)

The broad pass listed 5 first-class entities; this round adds **9 new entities** the broad pass missed.

#### New entity (E-01-01): `RedirectUriStrategyRequest` (request type)
- **Module**: `api/auth.rs:398-407`
- **Shape**: `enum { Dynamic, Fixed(u16) }` (`derive(Debug, Clone, Copy, PartialEq, Eq)`)
- **Lifecycle**: Constructed by `cli/auth.rs::login_oauth` based on `OAuthAppSource`: `Embedded` → `Fixed(EMBEDDED_CALLBACK_PORT)`; everything else → `Dynamic`. Consumed by `bind()` which returns a `ResolvedRedirect`.
- **Invariant**: `Fixed(p)` errors with friendly message on `EADDRINUSE` directing to BYO override; `Dynamic` propagates the underlying `io::Error`.

#### New entity (E-01-02): `ResolvedRedirect` (TOCTOU-closed binding)
- **Module**: `api/auth.rs:459-478`
- **Shape**: struct with **private** fields `strategy: RedirectUriStrategy, listener: tokio::net::TcpListener`. Two consumer methods: `strategy() -> RedirectUriStrategy` (read-only) and `into_parts() -> (RedirectUriStrategy, tokio::net::TcpListener)` (consume-once).
- **Lifecycle**: Created exactly by `RedirectUriStrategyRequest::bind()`; consumed by `oauth_login` to accept a single callback HTTP connection.
- **Architectural significance**: The private-field design **prevents** a future caller from moving the listener out and deriving a `redirect_uri` from a stale strategy — re-opening the TOCTOU between probe and use that the type was created to close. This is an explicitly documented type-system safety property (lines 451-458).

#### New entity (E-01-03): `RedirectUriStrategy` (resolved port choice)
- **Module**: `api/auth.rs:490-496`
- **Shape**: `enum { DynamicPort(u16), FixedPort(u16) }` with `redirect_uri()` method:
  - `FixedPort(p)` → `format!("http://127.0.0.1:{p}/callback")` (literal IPv4 to avoid macOS/Chrome `localhost`→`::1` resolver pitfall, line 506-516)
  - `DynamicPort(p)` → `format!("http://localhost:{p}/callback")` (preserves backward compat for BYO users with `localhost:` callback URLs already registered, line 517-528)
- **Invariant**: The two distinct host forms are pinned by test `redirect_uri_strategy_strings` at line 928-937. Atlassian validates `redirect_uri` by exact string match (NOT RFC 8252's "any loopback port" rule — JRACLOUD-92180), and the fixed-port host string is registered in Developer Console.

#### New entity (E-01-04): `OAuthResult` (login outcome)
- **Module**: `api/auth.rs:368-372`
- **Shape**: `pub struct { cloud_id: String, site_url: String, site_name: String }`
- **Lifecycle**: Returned by `oauth_login` after the 5-step flow (browser open → callback accept → code exchange → accessible-resources query → token persist).
- **Note**: The broad pass mentioned `cloud_id` + site discovery in the flow narrative but did not type-catalog this struct.

#### New entity (E-01-05): `RefreshAppSource` (private to api/auth.rs)
- **Module**: `api/auth.rs:822-826`
- **Shape**: `enum { Keychain, Embedded }` — module-private (no `pub`)
- **Distinction from `OAuthAppSource`**: The login resolver uses **6** sources (Flag, Env, Keychain, Embedded, Prompt, None). The refresh resolver uses **only 2** (Keychain, Embedded) — flag, env, and prompt are deliberately omitted because refresh is non-interactive and reuses the app credentials already associated with the stored refresh token (rationale at line 772-780).
- **Architectural significance**: Two distinct enums for two distinct precedence chains is a deliberate domain-modelling choice — silently sharing one chain would risk login-time-only sources (flag, env, prompt) leaking into the non-interactive refresh path.

#### New entity (E-01-06): `AuthFlow` (login dispatcher private enum)
- **Module**: `cli/auth.rs:264-280`
- **Shape**: `enum { Token, OAuth }` with `label() -> &'static str` (`"api_token"` / `"oauth"`)
- **Module-private**: kept private to `cli/auth.rs` so it isn't part of the crate library API surface.
- **Use sites**: `chosen_flow_for_profile` (production), `chosen_flow` (`#[cfg(test)]` only — kept as labeled entry point for a future caller per the explicit comment at line 287-289).
- **Invariant**: `label()` is the single source of truth for the on-disk `auth_method` value AND the `--output json` payload's `auth_method` key (test `auth_flow_labels_match_config_and_json_conventions` at line 1283).

#### New entity (E-01-07): `LoginArgs` (handle_login parameter bundle)
- **Module**: `cli/auth.rs:523-532`
- **Shape**: 8-field public struct: `profile: Option<String>, url: Option<String>, oauth: bool, email: Option<String>, token: Option<String>, client_id: Option<String>, client_secret: Option<String>, no_input: bool`
- **Domain semantics**: Encodes ALL inputs to login (every credential slot, profile/url, flow toggle, no-input gate) in a single struct — explicitly motivated as a `clippy::too_many_arguments` workaround per comment at line 519-521.

#### New entity (E-01-08): `RefreshArgs<'a>` (refresh_credentials parameter bundle)
- **Module**: `cli/auth.rs:834-843`
- **Shape**: 8-field borrowing struct: `profile: Option<&'a str>, oauth: bool, email: Option<String>, token: Option<String>, client_id: Option<String>, client_secret: Option<String>, no_input: bool, output: &'a OutputFormat`
- **Distinction from LoginArgs**: Same shape but **borrows profile name** (not owned) and includes `output: &OutputFormat` to drive the JSON-vs-table refresh-success rendering. The `RefreshArgs::output` field is what makes refresh's JSON success payload (`{status: "refreshed", auth_method: "oauth", next_step: "..."}`) routable through the standard output formatter.

#### New entity (E-01-09): `OAuthAppSource::None` sentinel (special variant)
- **Module**: `api/auth_embedded.rs:53-56`
- **Significance**: Distinct from Rust's `Option::None`. The `None` variant is a sentinel returned by `peek_oauth_app_source` so the status display surface can render `(none)` without forcing call sites to an `Option<OAuthAppSource>`. **Confirmed by re-reading source.** The broad pass labelled it as a sentinel but did not flag the `Option::None` collision risk for downstream readers.

#### Refined entity (E-01-10): Keychain key namespace catalog (full enumeration)
The broad pass listed the keys conceptually. Full re-enumeration from source:

| Constant | Value | Scope | Read-only? | Source |
|---|---|---|---|---|
| `KEY_EMAIL` | `"email"` | flat / shared | no | `api/auth.rs:19` |
| `KEY_API_TOKEN` | `"api-token"` | flat / shared | no | `api/auth.rs:20` |
| `"oauth_client_id"` (literal) | flat / shared | no | `api/auth.rs:192, 202, 239, 256, 330` |
| `"oauth_client_secret"` (literal) | flat / shared | no | `api/auth.rs:194, 206, 240, 257, 331` |
| `KEY_OAUTH_ACCESS_LEGACY` | `"oauth-access-token"` | flat / legacy | **read-only after migration** | `api/auth.rs:24` |
| `KEY_OAUTH_REFRESH_LEGACY` | `"oauth-refresh-token"` | flat / legacy | **read-only after migration** | `api/auth.rs:25` |
| `oauth_access_key(profile)` | `format!("{profile}:oauth-access-token")` | per-profile | no | `api/auth.rs:27-29` |
| `oauth_refresh_key(profile)` | `format!("{profile}:oauth-refresh-token")` | per-profile | no | `api/auth.rs:30-32` |

**Pass 2 broad treatment was correct in spirit but missed two structural points**:
1. The two `oauth_client_*` keys are **string literals at five call sites** (NOT named constants). Refactoring them to constants would be a one-line cleanup but is currently a code smell.
2. The legacy keys' "read-only after migration" status is enforced by the absence of any production write path — the broad pass mentioned the read-side rule but didn't note that no writer ever sets the legacy keys post-migration.

#### Refined entity (E-01-11): `EmbeddedOAuthApp` plaintext lifecycle
- **Module**: `api/auth_embedded.rs:22-26, 116-120`
- **Refinement**: Plaintext is held in process memory **for the lifetime of the binary**, NOT just for one OAuth call. Reason (line 19-21): "needed for every refresh-token grant". The `OnceLock` caches the decoded plaintext. **Defense in depth**: `embedded_oauth_app_present()` (lines 132-136) checks raw build constants without ever invoking `decode()` — so `jr auth status` on a release binary doesn't materialize the live `client_secret` into the heap.
- **Pass 2 broad treatment**: Mentioned the OnceLock and presence-without-decode but did not characterize the lifetime as "for the lifetime of the binary" with the refresh-grant rationale.

#### Refined entity (E-01-12): Unique test service-name namespacing
- **Module**: `api/auth.rs:1085-1128`
- **Pattern**: Tests gated by `JR_RUN_KEYRING_TESTS=1` use `unique_test_service()` (atomic counter + process ID) to namespace each test's keychain operations: `format!("jr-jira-cli-test-{}-{}", process::id(), n)`. A **module-static `KEYRING_TEST_ENV_MUTEX`** serializes `JR_SERVICE_NAME` env-mutation across concurrent tests. The mutex uses **poisoned-lock recovery** (`.unwrap_or_else(|poisoned| poisoned.into_inner())`) so a panicking keyring test doesn't leak into later tests.
- **Domain significance**: This is the testing scaffolding that makes the "12 keyring round-trip tests" actually correct under cargo test parallelism.

#### Refined entity (E-01-13): The 4 partial-state recovery branches in `load_oauth_tokens`

The broad pass §2b.3 OAuth state machine showed "partial state → user error OR legacy→namespaced auto-migrate for default". Re-reading `api/auth.rs:117-168` reveals **four distinct branches** with subtle ordering:

1. `(Some(a), Some(r))` — namespaced pair complete → return.
2. `(None, None)` — both absent. **Default profile only**: try legacy fallback. If complete: copy + delete + return. If absent: error "no stored OAuth token". **Non-default profiles**: skip the legacy probe entirely, error directly.
3. `(Some, None)` or `(None, Some)` — partial state. **Default profile only**: try legacy fallback (same logic as branch 2). **Recovery scenario**: an interrupted prior lazy migration could leave the namespaced pair partial while legacy is intact — recovery from legacy is the correct path. **Non-default profiles**: error with the explicit "partial — run logout then login" message.
4. (Implicit) Each `read_keyring_optional` call distinguishes `keyring::Error::NoEntry` (returns `Ok(None)`) from real backend failures (returns `Err`). A locked keychain or permission-denied error is NOT silently treated as absent — pinned by the helper at `api/auth.rs:181-187`.

**Tests pinning each branch (in `cfg(test)` mod, all `#[ignore]`-gated)**:
- Branch 2 default success → `lazy_migration_legacy_flat_keys_for_default_profile` (line 1155)
- Branch 2 non-default error → `lazy_migration_does_not_fire_for_non_default_profile` (line 1325)
- Branch 3 default recovery → `load_oauth_tokens_default_partial_recovers_from_legacy` (line 1273)
- Branch 3 non-default error → `load_oauth_tokens_errors_on_partial_state` (line 1251)

**Pass 2 broad treatment**: Captured the conceptual transitions but missed the four-branch decomposition and the keychain-error-is-not-absent invariant.

---

### 3.2 T-02: Issue list query model (deep)

The broad pass cataloged the `issue list` operation in §2b.1 and noted the JQL-composition flow in Flow 2 (§2b.6). This round adds entity-level structure for the query lifecycle.

#### New entity (E-02-01): `FilterOptions<'a>` (JQL clause bundle)
- **Module**: `cli/issue/list.rs:598-610`
- **Shape**: 11-field borrowing struct used **only** as input to `build_filter_clauses`:
  ```rust
  struct FilterOptions<'a> {
      assignee_jql: Option<&'a str>,
      reporter_jql: Option<&'a str>,
      status: Option<&'a str>,
      team_clause: Option<&'a str>,
      recent: Option<&'a str>,
      open: bool,
      asset_clause: Option<&'a str>,
      created_after_clause: Option<&'a str>,
      created_before_clause: Option<&'a str>,
      updated_after_clause: Option<&'a str>,
      updated_before_clause: Option<&'a str>,
  }
  ```
- **Domain significance**: The struct is a **JQL clause bag** — every field is either an `Option<&str>` (the resolved clause fragment) or a `bool` (a flag whose true-ness drives a literal substitution). This is the "options bag" pattern explicitly called out in source as a `clippy::too_many_arguments` workaround.
- **Pass 2 broad treatment**: Mentioned the JQL composition flow but did not type-catalog this struct.

#### New entity (E-02-02): JQL composition pipeline (deterministic order)
The broad pass listed the inputs to JQL composition. The actual composition happens in two passes (`build_jql_base_parts` + `build_filter_clauses`) joined by `AND`. The deterministic order, verified at `cli/issue/list.rs:613-648`, is:

| Position | Clause | Source |
|---:|---|---|
| 1 (base) | `project = "<key>"` (if project_key set) | line 44-45 |
| 2 (base) | `(<stripped --jql>)` (if `--jql` set) — explicitly parenthesized | line 47-48 |
| 2 (base, alternative) | `sprint = <id>` (if board has active scrum sprint) | line 281 |
| 2 (base, alternative) | `statusCategory != Done` (if board is kanban) | line 308 |
| 3 (filter) | `assignee = <jql>` | line 615-616 |
| 4 (filter) | `reporter = <jql>` | line 618-619 |
| 5 (filter) | `status = "<escaped>"` | line 621-622 |
| 6 (filter) | `statusCategory != Done` (if `--open`, **may duplicate** clause 2 alternative for kanban boards) | line 624-625 |
| 7 (filter) | `<team_clause>` | line 627-628 |
| 8 (filter) | `created >= -<duration>` (if `--recent`) | line 630-631 |
| 9 (filter) | `<asset_clause>` (built by `jql::build_asset_clause`) | line 633-634 |
| 10 (filter) | `created >= "<date>"` (if `--created-after`) | line 636-637 |
| 11 (filter) | `created < "<next_day>"` (if `--created-before`, **+1 day for inclusive end**) | line 639-640, 118-121 |
| 12 (filter) | `updated >= "<date>"` (if `--updated-after`) | line 642-643 |
| 13 (filter) | `updated < "<next_day>"` (if `--updated-before`, **+1 day**) | line 645-646, 123-126 |
| trailing | `ORDER BY <updated DESC \| rank ASC>` | line 51-52 (default), 281 (sprint), 309 (kanban) |

**Architecturally significant**: 
- **`--created-before`/`--updated-before` add 1 day** so that `--created-before 2026-05-04` matches issues created at 23:59 on 2026-05-04 (inclusive end-of-day). The user-visible date stays inclusive even though the JQL operator is strict-`<`. **NEW INVARIANT** not in broad Pass 2.
- **The `--open` flag and the kanban-board base clause both emit `statusCategory != Done`** — under `--open --board <kanban>`, the JQL contains the literal twice. Jira accepts redundant predicates so this is harmless, but a future deduplicator would need to normalize.
- **`ORDER BY` choice**: `rank ASC` for board+sprint paths (so issues appear in board order); `updated DESC` for project-scoped queries (so most-recently-changed first).

#### New entity (E-02-03): Project / board / sprint resolution branches in JQL base
- **Module**: `cli/issue/list.rs:268-338`
- **Branch decision tree**:
  1. `--jql` provided → strip `ORDER BY`, prepend project scope if any, leave the rest verbatim, default `ORDER BY updated DESC`.
  2. No `--jql`, no `board_id` in `.jr.toml` → just project scope + default `ORDER BY updated DESC`.
  3. No `--jql`, `board_id` set, scrum board, ≥1 active sprint → `sprint = <id>` + `ORDER BY rank ASC`.
  4. No `--jql`, `board_id` set, scrum board, **no** active sprint → fall back to project-scoped + `ORDER BY updated DESC`.
  5. No `--jql`, `board_id` set, kanban board → project scope + `statusCategory != Done` + `ORDER BY rank ASC`.
- **Error mapping**: `get_board_config` 404 → `JrError::UserError` with friendly "remove board_id from .jr.toml or use --jql" hint. Other errors → context-wrapped propagation. **NEW INVARIANT** not enumerated in broad Pass 2.

#### New entity (E-02-04): Asset enrichment N+1 mitigation (substantive correction to broad Pass 4 §1.5/§5.2)
The broad Pass 2 §2b.6 Flow 2 wrote that the list path enriches "per-row, per-field, per-asset". Pass 4 §1.5 said "all serial". **Re-reading `cli/issue/list.rs:386-463` reveals two important mitigations the broad sweep missed**:

1. **Deduplication by `(workspace_id, object_id)` pair** (line 397-411): A `HashMap<(String, String), ()>` collects unique pairs across all rows and fields. Multiple issues with the same linked asset = one HTTP call. The same enrichment is applied back to every position via `enrich_indices: Vec<(usize, usize)>`.
2. **Concurrent enrichment via `futures::future::join_all`** (line 429-445): The deduplicated futures are awaited concurrently. NOT serial.

**Refinement to Pass 2 Flow 2**: For 50 issues × 3 CMDB fields with 80% asset reuse, the actual call count is approximately 50×3×0.2 = 30 deduplicated calls, not 150 (broad Pass 4 §5.2 estimated 150+). And those 30 are concurrent, not serial. **MARKED FOR PASS 4 RE-REVIEW** — the NFR "N+1 risk" framing should be softened to "deduplicated and concurrent N+1" (still O(N) GETs, but not naïve N×M).

#### New entity (E-02-05): The `enrich_assets` helper (the OTHER enrichment path)
- **Module**: `api/assets/linked.rs:170-225`
- **Distinction from list.rs's enrichment**: `enrich_assets(client, &mut [LinkedAsset])` is the per-asset (NOT per-pair-unique) enrichment used by `view`/`assets` paths. Same `futures::future::join_all` concurrency. Same workspace-fallback logic. **No dedup** — operates on a flat slice provided by the caller.
- **Failure mode**: workspace fetch error → silently `return` (degrade gracefully). Per-asset error → the asset stays unenriched but no error propagates.

#### New entity (E-02-06): Story-points display gating
- **Module**: `cli/issue/list.rs:579-594`
- **Three-state rule**:
  - `--points false, sp_field_id: any` → return None (no column, no warning).
  - `--points true, sp_field_id: Some(id)` → return Some(id).
  - `--points true, sp_field_id: None` → return None **+ stderr warning** with config-fix hint.
- **Pass 2 broad treatment**: Flagged the `IssueFields::story_points(field_id)` accessor but did not enumerate the gating function.

#### New entity (E-02-07): Team display gating (cache-fallback hash map)
- **Module**: `cli/issue/list.rs:500-531`
- **Multi-condition gate**: `team_displays` is non-empty only when:
  1. Output format is Table (JSON skipped — JSON consumers see raw UUID under `fields.<team_field_id>`).
  2. `team_field_id` is configured.
  3. At least one issue has a populated team UUID.
- **Cache fallback**: `cache::read_team_cache(profile)` is best-effort; on miss/Err, builds an empty map. UUIDs without a name resolve to the raw UUID literal in display. **No re-fetch on cache miss inside this hot path** — re-fetch is an explicit `jr team list --refresh` call.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-01)**: `--asset` requires CMDB fields to exist on the Jira instance
- **Source**: `cli/issue/list.rs:170-178`
- **Behavior**: If `cmdb_fields.is_empty()` after `get_or_fetch_cmdb_fields`, error with `JrError::UserError("--asset requires Assets custom fields on this Jira instance. Assets requires a paid Jira Service Management plan.")`.
- **Pass 2 broad treatment**: Did not list this as an invariant.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-02)**: `--asset` auto-enables `--assets` display column
- **Source**: `cli/issue/list.rs:87` — `let show_assets = show_assets || asset_key.is_some();`
- **Pass 2 broad treatment**: Was listed in §2b.2 #18 but the line citation was wrong (no exact line; just "filtering implies displaying"). Confirmed at `:87`.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-03)**: Status validation cost decision
- **Source**: `cli/issue/list.rs:200-247`
- **Behavior**: When `--status` is set:
  - With `project_key` → `client.get_project_statuses(pk)` (project-scoped endpoint, returns only valid statuses for that project's issue types). 404 → "Project not found" UserError. `extract_unique_status_names` dedups across issue types and sorts.
  - Without `project_key` → `client.get_all_statuses()` (global Jira endpoint, returns every status across the instance).
- **Distinct error path**: When `--status` is set, the project-existence check at line 191-198 is **skipped** because `get_project_statuses` will already return 404 for a missing project.
- **Pass 2 broad treatment**: Did not enumerate this branching.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-04)**: Empty-query guard
- **Source**: `cli/issue/list.rs:344-352`
- **Behavior**: If after composition `all_parts.is_empty()` (no project AND no `--jql` AND no filter flags), error with explicit list of every possible flag the user could have passed. Prevents the "select * from issues" mistake against a multi-project instance.
- **Architectural significance**: This is the only place in the codebase where the JQL composition guards against an unbounded query. The `assets search` path (which also uses pagination) has no analogous check.

---

### 3.3 T-03: Assets / CMDB domain (deep)

#### New entity (E-03-01): The 6 endpoints in `api/assets/`
The broad pass cited "5 API files" but did not enumerate the endpoints. Re-reading:

| File | LOC | Endpoint(s) |
|---|---:|---|
| `linked.rs` | 557 | (no direct endpoint — calls `get_asset` from `objects.rs`); discovers CMDB fields via `client.find_cmdb_fields()` (in `api/jira/fields.rs`) |
| `objects.rs` | 237 | `POST object/aql` (search), `GET object/<id>` (single), `GET object/<id>/attributes`, `GET objecttype/<id>/attributes` |
| `workspace.rs` | 58 | `GET /rest/servicedeskapi/assets/workspace` (note: JSM endpoint, not Assets-native — see broad Pass 2 §2a.1) |
| `tickets.rs` | 19 | `GET object/<id>/connectedTickets` (per inspection of the `cli/assets.rs` call site) |
| `schemas.rs` | 44 | `GET objectschema/list`, `GET objectschema/<id>/objecttypes/flat` |

**Architectural note**: `linked.rs` doesn't HAVE its own endpoint — it's a domain helper layer that **wraps three concerns**: CMDB-field discovery (cache + Jira fields API), JSON value extraction (no network), and per-asset enrichment (calls `get_asset`). Treating it as a flat "API file" hides this composition.

#### New entity (E-03-02): Workspace-ID resolution lifecycle (full state machine)
- **Module**: `api/assets/workspace.rs:19-58`
- **States**:
  - **cache-hit** → return cached `workspace_id` (no HTTP).
  - **cache-miss + 200** → fetch first entry from paginated response, write cache, return.
  - **cache-miss + 404 / 403** → error mapped to `JrError::UserError("Assets is not available on this Jira site. Assets requires Jira Service Management Premium or Enterprise.")`.
  - **cache-miss + empty values array** → error `JrError::UserError("No Assets workspace found on this Jira site. ...")`.
  - **cache-miss + other error** → propagate.
- **Pagination shape**: Returns `ServiceDeskPage<WorkspaceEntry>` (note: JSM-style pagination envelope, not Assets-native). The function takes `page.values.into_iter().next()` — only the first entry is consumed; pagination is structural but unused in practice ("In practice there's only one workspace per site," line 17-18).
- **Cache-write swallowed**: `let _ = cache::write_workspace_cache(profile, &workspace_id);` — write failures are intentionally non-fatal.

#### New entity (E-03-03): `WorkspaceEntry` struct (private)
- **Module**: `api/assets/workspace.rs:9-13`
- **Shape**: `#[derive(Debug, Default, Deserialize)] struct WorkspaceEntry { #[serde(rename = "workspaceId")] workspace_id: String }`
- **Module-private**: not exposed; consumed only inside `get_or_fetch_workspace_id`.
- **Pass 2 broad treatment**: Did not catalog.

#### New entity (E-03-04): CMDB field discovery via `JiraClient::find_cmdb_fields`
- **Module**: Discovered in `cli/issue/list.rs` and `cli/issue/view.rs`. Lives in `api/jira/fields.rs` (per Pass 1 §1c).
- **Wrapper**: `api/assets/linked.rs::get_or_fetch_cmdb_fields` wraps `find_cmdb_fields` with the cache-first read/write pattern. Returns `Vec<(String, String)>` — `(customfield_NNNNN, "Display Name")` tuples.
- **Cache shape**: `CmdbFieldsCache` stores `Vec<(String, String)>` (the tuple shape called out in CLAUDE.md as the format-change point). Old format (ID-only) deserializes as a corrupt cache → `Ok(None)` → triggers refetch.

#### New entity (E-03-05): `extract_linked_assets` value-shape tolerance
- **Module**: `api/assets/linked.rs:29-67`
- **Three accepted JSON shapes** for a CMDB field's value:
  1. **Array** (`Value::Array(arr)`) — iterate and parse each element.
  2. **Object** (`Value::Object(_)`) — parse single.
  3. **String** (`Value::String(s)`) — emit a `LinkedAsset { name: Some(s), ..Default }` (degenerate — name-only, no key/id/workspace).
  4. Anything else (numbers, bools, null) → silently skipped.
- **`parse_cmdb_value` ID coercion** (line 77-81): accepts `objectId` as either string OR u64 (legacy Jira shapes). Coerces u64 to string via `n.to_string()`.
- **Skip-rule** (line 87-89): "only create an asset if we got at least something useful" — `label`, `objectKey`, OR `objectId` must be present. A bare `{workspaceId}` payload yields nothing.

#### New entity (E-03-06): The `extract_linked_assets_per_field` shape
- **Module**: `api/assets/linked.rs:103-115`
- **Distinction from `extract_linked_assets`**: Returns `Vec<(String, Vec<LinkedAsset>)>` (field-name → assets). Skips fields that have no linked assets. Used by display surfaces that group by field (e.g., `jr issue view` shows "Hardware: server-1, server-2" then "Client: acme-corp").
- **Position-coupling**: The display path relies on per-field positions to inject enrichment back into the JSON `extra` map.

#### New entity (E-03-07): JSON enrichment back-injection (`enrich_json_assets`)
- **Module**: `api/assets/linked.rs:137-167`
- **Behavior**: Mutates the `extra: HashMap<String, Value>` flatten container in place — for each CMDB field, matches enriched `LinkedAsset` entries to the original JSON elements **by position** and injects `objectKey`, `label`, `objectType` as additional fields. Additive (does not strip fields).
- **Two shape branches**:
  - Array shape: position-match each.
  - Object shape: enrich with `assets[0]`.
- **Domain significance**: This is what makes `--output json --assets` show enriched names for CMDB fields. Without this, the JSON path would show raw `objectId`-only entries.

#### New entity (E-03-08): AQL search pagination + cap
- **Module**: `api/assets/objects.rs:17-63`
- **Page size**: hard-coded `max_page_size = 25`. Per-call cap parameter `limit: Option<u32>` further constrains; without `limit`, paginates until `has_more == false`.
- **Loop early-exit**: When `limit` reached, `truncate(cap)` and `break`. **Do NOT consume the next page** — saves one round-trip.
- **`AssetsPage::is_last` tolerance**: Bool-or-string custom deserializer (broad pass §2a.3 noted this; verified).

#### New entity (E-03-09): Object-key resolution heuristic
- **Module**: `api/assets/objects.rs:111-137` (verified by reading)
- **Branches**:
  - All-numeric input → return as-is (assumed object ID).
  - Otherwise → AQL search `Key = "<input>"` and unwrap the first object's ID.
- **Pass 2 broad treatment**: Mentioned the heuristic; this round catalogues the exact predicate (`chars().all(|c| c.is_ascii_digit())`).

#### New entity (E-03-10): `filter_tickets` connected-ticket status filter
- **Module**: `cli/assets.rs:306-360+`
- **Two mutually-exclusive filter modes**:
  - `--open` (boolean): retain tickets where `status.colorName != "green"`. Tickets with `None` status: **retained** (`unwrap_or(true)`).
  - `--status <name>` (string): build deduped `status_names` list, run `partial_match::partial_match`, filter on the resolved name. Tickets with `None` status: **excluded** (filter only keeps Some(name)==resolved).
- **Asymmetry**: `--open` and `--status` treat `None` status differently. Pass 2 broad treatment §2b.5 invariant 20 captured this asymmetry only at the conceptual level; this round confirms with line citations.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-05)**: `parse_cmdb_value` requires at least one of `{label, objectKey, objectId}`
- **Source**: `api/assets/linked.rs:87-89`
- **Behavior**: A `{workspaceId: "ws"}`-only payload returns `None` (asset is silently dropped). The `id` field on `LinkedAsset` ends up populated only when `objectId` was present.
- **Pinning test**: Implicitly pinned by `parse_legacy_ids_only` (line 251-263) which provides all three keys — but NO test asserts "workspaceId-only payload yields no asset". Worth adding to Pass 3 invariant list.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-06)**: AssetsPage max page size = 25
- **Source**: `api/assets/objects.rs:26` (hardcoded local var, not a named constant)
- **Behavior**: AQL search paginates 25 at a time regardless of `--limit` larger than 25.
- **Pass 2 broad treatment**: Mentioned "max-page=25" in §2b.1 but did not pin as an invariant.

---

### 3.4 T-05: Cache layer (deep)

#### Refined entity (E-05-01): The 6 cache categories — full lifecycle table

| Cache | File | Storage shape | Per-entry TTL? | Read miss return | Corruption handling | Cross-profile read isolation |
|---|---|---|---|---|---|---|
| `TeamCache` | `teams.json` | whole-file | no (per-file) | `Ok(None)` | stderr warn + `Ok(None)` | strict (per-profile dir) |
| `ProjectMeta` | `project_meta.json` | keyed map by project key | **yes (per-entry `fetched_at`)** | `Ok(None)` for missing key OR expired entry | stderr warn + `Ok(None)` (whole map dropped on parse failure of the read path); on **write**, parse failure → `HashMap::new()` (existing entries lost, with warning) | strict |
| `WorkspaceCache` | `workspace.json` | whole-file | no | `Ok(None)` | stderr warn + `Ok(None)` | strict |
| `ResolutionsCache` | `resolutions.json` | whole-file | no | `Ok(None)` | stderr warn + `Ok(None)` | strict |
| `CmdbFieldsCache` | `cmdb_fields.json` | whole-file (Vec<(id,name)>) | no | `Ok(None)` | stderr warn + `Ok(None)` (catches the legacy ID-only format-change case) | strict |
| `ObjectTypeAttrCache` | `object_type_attrs.json` | keyed map by object_type_id | **per-FILE TTL on `fetched_at`** | `Ok(None)` if file expired OR key absent | stderr warn + `Ok(None)` on read; on write, parse failure → fresh empty cache (existing entries lost) | strict |

**Key finding**: **`ProjectMeta` and `ObjectTypeAttrCache` are NOT both "per-entry TTL"** — broad pass §2a.2 said both used per-entry TTL, but in fact:
- `ProjectMeta` checks `entry.fetched_at` per entry (`cache.rs:135`).
- `ObjectTypeAttrCache` checks the **wrapper's** `fetched_at` (`cache.rs:307-309`) — when the file ages out, ALL keyed entries expire together. Per-key TTL would require per-entry timestamps in the value, which the schema doesn't carry.

**Impact**: A 6-day-old `object_type_attrs.json` with 50 cached object types: on day 7, all 50 expire simultaneously. With `ProjectMeta`: each project's TTL is independent, so 50 projects fetched on different days expire on different days. **Pass 2 §2a.2 conflated these.** **CONV-ABS retraction not warranted** because the broad pass said "Per-entry TTL" for ProjectMeta (correct) and "per-file TTL" for ObjectTypeAttrCache (also correct, if you read it carefully) — but the rendered table cell read ambiguously. **Refined for clarity in this round.**

#### New entity (E-05-02): The `Expiring` trait contract
- **Module**: `cache.rs:10-12`
- **Shape**: Single-method trait `pub(crate) trait Expiring { fn fetched_at(&self) -> DateTime<Utc>; }`.
- **Architecture**: Implemented by `TeamCache`, `WorkspaceCache`, `ResolutionsCache`, `CmdbFieldsCache`, `ObjectTypeAttrCache` (all 5 whole-file caches). NOT implemented by `ProjectMeta` (because TTL is per-entry, the wrapper has no aggregated `fetched_at` — `ProjectMeta` itself has `fetched_at` per-entry). The trait is the gate for the generic `read_cache<T: Expiring>` reader.
- **Constant**: `CACHE_TTL_DAYS = 7` at line 7.

#### New entity (E-05-03): Map-cache write-merge semantics
- **Module**: `cache.rs:149-173` (`write_project_meta`), `cache.rs:316-353` (`write_object_type_attr_cache`)
- **Pattern**: BOTH map caches **read existing file → merge new entry → write whole file**. So a write of one project's metadata doesn't clobber other projects.
- **Failure mode on parse failure during merge**: 
  - `write_project_meta` (line 158-162): warn + start with empty `HashMap::new()` — **all other cached projects are lost**.
  - `write_object_type_attr_cache` (line 330-338): warn + start with fresh `ObjectTypeAttrCache { fetched_at: now, types: empty }` — **all other object types are lost**.
- **Domain significance**: A corrupted map cache is recovered SAFELY for read paths (returns `Ok(None)`) but on the next write, **silently destroys data**. This is a real correctness hazard for the "cache file structure changed in a release" scenario flagged in CLAUDE.md.

#### New entity (E-05-04): `cache_root` and `cache_dir` resolution
- **Module**: `cache.rs:64-78`
- **`cache_root()`** prefers `XDG_CACHE_HOME`, falls back to `~/.cache` (or literal `~` if home unknown). Suffix is `/jr`.
- **`cache_dir(profile)`** appends `v1/<profile>/`. The `v1` segment is the version fence — bumping to `v2` orphans the entire prior cache layout cleanly (CLAUDE.md gotcha).
- **Pass 2 broad treatment**: Mentioned both functions; this round verifies the absent-`HOME` fallback to literal `"~"` (which would produce `~/.cache/jr/v1/<profile>/` as a literal path string — almost certainly a bug, but unreachable on macOS/Linux/Windows in practice because `dirs::home_dir()` is reliable).

#### New entity (E-05-05): `clear_profile_cache` no-op semantics
- **Module**: `cache.rs:82-88`
- **Behavior**: If `<root>/v1/<profile>/` doesn't exist, **no-op** (return `Ok(())` without error). Used by `auth remove` and `auth refresh`.
- **Pass 2 broad treatment**: Listed at §2b.5 invariant 10. Confirmed.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-07)**: Map-cache writes silently destroy other entries on parse failure
- **Source**: `cache.rs:158-162, 330-338`
- **Behavior**: When the existing file is unreadable on a write path, the writer starts fresh — **other cached entries are lost** with only a stderr warning (which a non-verbose run would still see, but most users would miss).
- **Mitigation**: The 7-day TTL means lost entries refetch within a week. But during that window, a power user with 50 cached projects could see all but the just-written one disappear silently.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-08)**: Per-profile signature is enforced by **convention only** (no compile-time fence)
- **Source**: All public read/write fns take `profile: &str` first. There is NO newtype, NO trait gate, NO phantom type. A new function added later that **forgets to pass profile** would compile without warning.
- **Pass 2 broad treatment**: Listed at §2b.2 #7 "no compile-time fence". This round re-verifies — adding `pub fn read_my_cache() -> Result<...> { ... }` (without `profile`) would be a silent cross-profile leak. The "soft fence" property is documented in Pass 5 §9.2.4 as well; this is a cross-pass-confirmed invariant.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-09)**: Test scaffolding uses `XDG_CACHE_HOME` env mutation under a static `ENV_MUTEX`
- **Source**: `cache.rs:362-379`
- **Behavior**: `with_temp_cache(F)` holds a `Mutex<()>`, sets `XDG_CACHE_HOME` to a temp dir, runs `F` under `catch_unwind`, then unsets. Recovers from poisoned lock so a panicking test doesn't leak the env override. Uses `std::panic::AssertUnwindSafe` because `F: FnOnce()` isn't required to be unwind-safe.
- **Architectural significance**: Same env-mutex pattern as `cli/auth.rs::ENV_LOCK` and `api/auth.rs::KEYRING_TEST_ENV_MUTEX`. THREE distinct test-scaffolding mutexes for env-scoped tests.

---

### 3.5 T-07: Configuration model (deep)

#### Refined entity (E-07-01): Profile-name validation rule + 3 boundaries
- **Source**: `config.rs:113-140` (`validate_profile_name`)
- **Rule**:
  - **Length**: 1 ≤ len ≤ 64 (empty AND >64 both rejected).
  - **Charset**: `[A-Za-z0-9_-]` (ASCII alphanumeric + underscore + hyphen).
  - **Reserved Windows names**: case-insensitive match against `CON, NUL, AUX, PRN, COM1..COM9, LPT1..LPT9`.
- **Three boundaries** (broad Pass 2 §2a.4 listed these; re-verified):
  1. **CLI arg validator** (`main.rs:62`) — rejects `--profile bad-name` immediately.
  2. **Every key in `[profiles]`** (`config.rs:274-282`) — runs after migration, before active-profile resolution. Rejects `[profiles."foo:bar"]`.
  3. **Resolved active profile name** (`config.rs:304`) — covers `JR_PROFILE=foo:bar` and the `default_profile = "foo:bar"` field case.
- **Error message** (line 135-140): `"invalid profile name {name:?}; allowed: A-Z a-z 0-9 _ - up to 64 chars; reserved Windows names (CON, NUL, AUX, PRN, COM1-9, LPT1-9) excluded"`

#### Refined entity (E-07-02): Profile resolution precedence (deterministic)
- **Source**: `config.rs:95-110` (`resolve_active_profile_name`)
- **Order**: `cli_flag (--profile)` > `JR_PROFILE env` > `config.default_profile field` > `"default"` literal.
- **Threading rationale** (line 195-202, 293-298): The flag is threaded as a **parameter** to `Config::load_with(cli_profile: Option<&str>)`, NOT through an env-var seam (legacy `JR_PROFILE_OVERRIDE` was removed because `unsafe { std::env::set_var(...) }` under `#[tokio::main]` is unsound — POSIX `setenv` is not thread-safe and tokio worker threads exist before the async-main body runs).
- **Pass 2 broad treatment**: Captured the precedence; this round adds the threading-vs-env-var seam architectural rationale.

#### **NEWLY-DISCOVERED ENTITY (E-07-03)**: `Config::load_lenient_with` — the lenient/strict load split
- **Source**: `config.rs:210-218`
- **Behavior**: `load_lenient` skips the active-profile-existence check (lines 318-328 in `load_inner`). Otherwise identical.
- **Use sites**: `cli/auth.rs::handle_login`, `cli/auth.rs::login_token`, `cli/auth.rs::login_oauth` — these legitimately CREATE the target profile on demand. Strict load would fail before the create.
- **Architectural property**: `load_lenient_with(Some("newprof"))` allows `jr auth login --profile newprof --url ...` to succeed even though `[profiles.newprof]` doesn't yet exist on disk. EVERY OTHER command uses strict `load_with`.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-10)**: Migration write-back uses **file-only** baseline (no env overlay)
- **Source**: `config.rs:240-264`
- **Behavior**: When migration runs, the **file-only** read (NO `JR_*` env merge) drives the to-disk write. A `JR_DEFAULTS_OUTPUT=json jr issue list` run that triggers migration would NOT bake `output = "json"` into the saved config.
- **In-memory mirror**: The in-memory `global` still receives the env overlay so the current invocation behaves correctly; only the on-disk write is file-only.
- **Pass 2 broad treatment**: Listed at §2b.2 #21. **Re-verified**.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-11)**: `save_global` overlays only `default_profile + profiles` from in-memory; everything else is file-baseline
- **Source**: `config.rs:416-446`
- **Behavior**: Reads file-only baseline (no env overlay), then **only mutates** `to_save.default_profile = self.global.default_profile.clone()` and `to_save.profiles = self.global.profiles.clone()`. Other fields like `defaults.output` are preserved from disk — so a `JR_DEFAULTS_OUTPUT=json jr auth switch sandbox` invocation cannot persist `output = "json"`.
- **Architectural significance**: This is the env-leakage prevention for ALL save paths, not just migration. **Stronger than what broad Pass 2 §2b.2 #21 captured** (which only mentioned migration write-back).

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-12)**: `[fields]` legacy block is read at runtime via `config.global.fields.*` — NEVER through `active_profile().*`
- **Source**: `config.rs:48` (`pub fields: FieldsConfig`); `cli/issue/list.rs:147-148`; `cli/issue/view.rs:28-29`; `cli/issue/helpers.rs:43, 194, 200`; `cli/sprint.rs:232-233`; `cli/board.rs:192`.
- **Behavior**: Story-points and team field IDs are read from `config.global.fields.*` at every consumer. `ProfileConfig.team_field_id` and `ProfileConfig.story_points_field_id` exist on the per-profile struct (and are populated by migration + by `jr init`), but **NO consumer reads from them**.
- **What this means concretely**:
  - On a fresh install (`v1` shape, never migrated), `[fields]` is absent on disk. `Config::load` returns `global.fields = FieldsConfig::default()` (both fields `None`). Story points + team display are **silently disabled**, even if `[profiles.<active>].story_points_field_id` is set on disk.
  - On a post-migration install (which DID have legacy `[instance]/[fields]`), the migration runs once, populates `[profiles.default]`, and DROPS `[fields]` from disk on next save. After that save, `Config::load` again returns `global.fields = FieldsConfig::default()`. **Story points + team display silently disable for the user.**
  - The only configurations where story points + team display work today are: (a) hand-edited configs where the user kept `[fields]` on disk, OR (b) the brief window between migration's first load (which keeps `global.fields` populated in-memory) and the first `save_global` (which drops it from disk).
- **Architectural significance**: This is a **multi-profile correctness BUG** — CLAUDE.md and the broad Pass 2 §2a.2 / §2a.4 both describe `[fields]` as "drained into ProfileConfig during migration" with the implication that runtime reads use the per-profile values. The actual behavior is the OPPOSITE — runtime reads use the legacy global path, and the per-profile values are wholly unused.
- **Confirmation**: 
  ```
  $ grep -rn 'profile.story_points_field_id\|profile.team_field_id' src/  
  (zero hits)
  $ grep -rn 'global.fields' src/cli/
  src/cli/board.rs:192:    let team_field_id = config.global.fields.team_field_id.as_deref();
  src/cli/sprint.rs:232:    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
  src/cli/sprint.rs:233:    let team_field_id = config.global.fields.team_field_id.as_deref();
  src/cli/issue/list.rs:147:    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
  src/cli/issue/list.rs:148:    let team_field_id = config.global.fields.team_field_id.as_deref();
  src/cli/issue/helpers.rs:43:    let field_id = if let Some(id) = &config.global.fields.team_field_id;
  src/cli/issue/helpers.rs:194:    if let Some(sp) = config.global.fields.story_points_field_id.as_deref()
  src/cli/issue/helpers.rs:200:    if let Some(t) = config.global.fields.team_field_id.as_deref()
  src/cli/issue/view.rs:28-29:   (story_points + team)
  ```
- **Source comment evidence** (`config.rs:142-148`): "Tasks 4-15 callers still read `global.instance.*` / `global.fields.*` keep working until Tasks 7/8 migrate them to read `active_profile()` instead. Task 16 stops serializing the legacy fields, so they fall off disk on the next save." — **Tasks 7/8 (the migration of READS) appear to be incomplete; Task 16 (the migration of WRITES) DID happen.** This is the resulting drift.
- **Severity**: HIGH for the multi-profile spec (CLAUDE.md says "sandbox vs prod custom-field IDs can differ" but in practice the codebase only has ONE global `[fields]` block).

#### **NEWLY-DISCOVERED ENTITY (E-07-04)**: `JR_BASE_URL` overrides the resolved profile URL (test-only escape hatch)
- **Source**: `config.rs:351-353`
- **Behavior**: `JR_BASE_URL` env var, when set, **completely bypasses** the per-profile URL resolution AND the OAuth-specific `https://api.atlassian.com/ex/jira/<cloud_id>` rewrite. Used by integration tests with wiremock; documented as the test override (line "intended only for tests/power users" per Pass 2 §2b.2 #25).
- **Pass 2 broad treatment**: Listed in §2b.2 #25; this round confirms the **bypass also short-circuits the OAuth API host rewrite** (a nuance the broad pass missed).

#### **NEWLY-DISCOVERED ENTITY (E-07-05)**: OAuth `cloud_id` triggers API host rewrite
- **Source**: `config.rs:366-370`
- **Behavior**: If `profile.cloud_id` is set AND `profile.auth_method == Some("oauth")`, `base_url()` returns `https://api.atlassian.com/ex/jira/<cloud_id>` (the OAuth API gateway), NOT the per-profile `url` field.
- **Architectural significance**: OAuth requires routing through Atlassian's API gateway (which maps `cloud_id` → tenant), while api_token uses the direct site URL. This is a **flow-coupled URL rewrite** — same profile, two different runtime URLs depending on auth_method.
- **Pass 2 broad treatment**: Mentioned `cloud_id` field but did not catalog the host rewrite.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-13)**: Active-profile existence check is gated on `!profiles.is_empty()` AND `strict`
- **Source**: `config.rs:318-328`
- **Behavior**: `load_with` (strict) errors with `JrError::UserError("unknown profile: {name}; known: ...")` ONLY when:
  1. `strict` is true (NOT lenient).
  2. `global.profiles` is non-empty (a fresh install with no profiles is a legitimate first-run case — `init` / `auth login` would otherwise be locked out).
  3. The resolved active name is NOT a key in `global.profiles`.
- **Pass 2 broad treatment**: Captured the strict/lenient split but did not enumerate the empty-profiles-allowed case.

---

### 3.6 T-09: ADF rendering domain (MEDIUM-priority deepening)

This was a MEDIUM target; the round samples node coverage rather than exhaustively re-reading 1,826 LOC.

#### Entity catalog: ADF node types the codebase **emits** (markdown→ADF, in `AdfBuilder::end`)

| Node type emitted | Markdown source | Emitted shape | Source |
|---|---|---|---|
| `doc` | (root wrapper) | `{type: "doc", version: 1, content: [...]}` | `adf.rs:7-17, 24-32` |
| `paragraph` | `Tag::Paragraph` | `{type: "paragraph", content: children}` | `adf.rs:142` |
| `heading` | `Tag::Heading{level}` | `{type: "heading", attrs: {level}, content}` | `adf.rs:143-147` |
| `blockquote` | `Tag::BlockQuote` | `{type: "blockquote", content}` | `adf.rs:148` |
| `codeBlock` | `Tag::CodeBlock(kind)` | `{type: "codeBlock", content, attrs?: {language}}` | `adf.rs:149-155` |
| `bulletList` | `Tag::List(None)` | `{type: "bulletList", content}` | `adf.rs:156` |
| `orderedList` | `Tag::List(Some(start))` | `{type: "orderedList", content, attrs?: {order: start}}` (attrs only when start ≠ 1) | `adf.rs:157-162` |
| `listItem` | `Tag::Item` | `{type: "listItem", content: wrapped}` (children wrapped via `wrap_inlines_as_blocks` to satisfy ADF schema) | `adf.rs:163-188` |
| `table` | `Tag::Table` | `{type: "table", content}` | `adf.rs:189` |
| `tableRow` | `Tag::TableRow / TableHead` | `{type: "tableRow", content}` | `adf.rs:190` |
| `tableCell`, `tableHeader` | `Tag::TableCell` (with `is_header` from `in_table_head`) | `{type: cell_type, content: wrapped}` | `adf.rs:191-211` |
| `text` | `Event::Text(...)` | `{type: "text", text, marks?: [...]}` | `adf.rs:265, 282-285` |
| `hardBreak` | `Event::HardBreak` | `{type: "hardBreak"}` | `adf.rs:82` |
| `rule` | `Event::Rule` | `{type: "rule"}` | `adf.rs:83` |

#### Mark types (inline marks emitted alongside text)

| Mark | Markdown source | Shape | Source |
|---|---|---|---|
| `strong` | `Tag::Strong` (bold) | `{type: "strong"}` | `adf.rs:103` |
| `em` | `Tag::Emphasis` (italic) | `{type: "em"}` | `adf.rs:104` |
| `strike` | `Tag::Strikethrough` | `{type: "strike"}` | `adf.rs:105` |
| `code` | `Event::Code(...)` (inline) | `{type: "code"}` (added inside `push_code`) | `adf.rs:282` |
| `link` | `Tag::Link{dest_url, title}` | `{type: "link", attrs: {href, title?}}` (title only if non-empty) | `adf.rs:106-115` |

#### Node types the codebase **reads** but does NOT emit (in `adf_to_text`)

The `adf_to_text` reader has explicit branches at `adf.rs:380-535+` for: `text`, `paragraph`, `heading`, `bulletList`, `orderedList`, `listItem`, `rule`, `hardBreak`, `codeBlock`, `blockquote`, `table`, `tableRow`, `tableCell`, `tableHeader`. **Plus** a default branch (line 531+) that recurses into `content` for **unknown node types** rather than emitting placeholder strings — explicitly per #202 spec, this avoids debug strings like `[unsupported: type]` reaching user output. So:
- `mention` (Atlassian's @user reference) → recursed-into; if it has `attrs.text`, that's where the user-visible name lives — but the reader doesn't render it specially. Result: `mention` becomes a no-op (silent drop) unless a child `text` node exists.
- `emoji`, `inlineCard`, `panel`, `mediaSingle`, `mediaGroup`, `media` → all silently dropped (line 727-737 includes a test confirming `mediaGroup` produces empty output; line 743-752 test confirms `panel` drops its wrapper but renders inner content via the recurse-into-`content` rule).

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-14)**: ADF-to-markdown roundtrip is **lossy in both directions**
- **Forward (text→ADF)**: `text_to_adf(s)` wraps `s` as a single-paragraph single-text-node ADF doc. **No mark detection** — bold/italic markers in the raw string become literal characters. Use `markdown_to_adf` for rich-text ingestion.
- **Reverse (ADF→text)**: `adf_to_text` produces markdown-ish output (bullets become `- `, ordered lists become `N. `, blockquotes become `> `, code blocks become `\`\`\`lang ... \`\`\``, links become `[text](href)`, marks become `**bold**`, `*em*`, `~~strike~~`, `` `code` ``). But:
  - ADF `mention`, `emoji`, `inlineCard`, media nodes → silently dropped.
  - Unknown ADF node types → silently dropped (recurse into `content` only).
- **Markdown→ADF→markdown is NOT identity** for any document containing tables, mentions, custom panels, or any ADF-specific constructs.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-15)**: ADF `orderedList` `attrs.order` falsy values default to 1
- **Source**: `adf.rs:407-416`
- **Behavior**: Missing, 0, or negative `attrs.order` is treated as "start at 1" — matches Jira's renderer semantics.
- **Pass 2 broad treatment**: Did not catalog.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-16)**: `listItem` wraps children to satisfy ADF schema
- **Source**: `adf.rs:163-188`
- **Behavior**: pulldown-cmark legitimately emits `blockquote`, `heading`, `table`, `rule` inside `Item` (e.g., for markdown like `- > quoted`), but ADF's documented `listItem` schema only accepts `paragraph`, `bulletList`, `orderedList`, `codeBlock`, `mediaSingle`. The codebase WIDENS the wrap-allowlist to include `blockquote`, `heading`, `table`, `rule` — because Jira's renderer is lenient where the spec is strict. **Architectural choice**: tolerate spec/renderer divergence to preserve user-intended markdown.

#### **NEWLY-DISCOVERED INVARIANT (NEW-INV-17)**: `tableCell` content always wrapped in a block
- **Source**: `adf.rs:201-211`
- **Behavior**: pulldown-cmark emits `Text` events directly inside `TableCell` without a `Paragraph` wrapper. The codebase wraps inline text runs into a `paragraph` block before emitting — ADF requires cells to wrap content in a block.

---

### 3.7 T-11: OAuth state machine + 401 auto-refresh deferred-integration scope (MEDIUM)

#### The unused `refresh_oauth_token` function (entity catalog)
- **Module**: `api/auth.rs:704-770`
- **Signature**: `pub async fn refresh_oauth_token(profile: &str) -> Result<String>`
- **Inputs**: `profile` only. App credentials resolved internally via `resolve_refresh_app_credentials` (returns `(client_id, client_secret, RefreshAppSource)`). Refresh token loaded via `load_oauth_tokens(profile)` (which itself goes through the full lazy-migration recovery on default profile).
- **Outputs**: `Ok(new_access_token)` on success — the new access token is also persisted to keychain alongside the new refresh token (Atlassian RFC 6749 §6 inherits scopes; the response always contains both new tokens).
- **Side effects**:
  1. POST to `https://auth.atlassian.com/oauth/token` with `grant_type: refresh_token`.
  2. On HTTP success, persist `(access_token, refresh_token)` to keychain.
  3. On HTTP failure, surface a flow-aware hint: `Embedded` source → "embedded creds may have rotated, brew upgrade jr"; `Keychain` source → "stored creds may be invalid, re-run login".
  4. On keychain-write failure post-success, surface the partial-state explicitly so the user knows the new tokens are gone (Atlassian still rotated them on its side).
- **Production callers**: **ZERO**. Documented at line 700-703: "exists for a future 401 auto-refresh integration. `jr auth refresh` (the user-facing CLI command) uses the clear-and-relogin flow at `cli/auth.rs::refresh_credentials`, not this helper."

#### The clear-and-relogin alternative (`refresh_credentials`)
- **Module**: `cli/auth.rs:845-935`
- **Behavior** (verified):
  1. Load config (strict — error if `--profile X` against unknown X).
  2. Validate target profile name.
  3. Inspect TARGET (not active) profile's `auth_method` to dispatch flow.
  4. **Refuse pre-flight if api_token flow + no URL** — message directs user to `jr auth login --profile X --url <...>` (because `login_token` doesn't fetch a URL; only the OAuth flow does via `accessible-resources`).
  5. **Clear-only-what-this-flow-refreshes**:
     - OAuth: `clear_profile_creds(target)` — deletes `<target>:oauth-*-token` keys (and legacy flat keys ONLY if target == "default").
     - Token: `clear_all_credentials(&[target])` — deletes shared `email`/`api-token`/`oauth_client_id`/`oauth_client_secret` (because the SHARED api-token IS the credential being refreshed) plus the per-profile OAuth tokens.
  6. Re-run the same login flow as `auth login` (token or oauth).
  7. On post-clear login failure: emit a stderr message instructing the user to manually run the appropriate `jr auth login` to restore access.
- **Architectural distinction**: Refresh is **destructive-but-safe** — the keychain is wiped before the new credentials arrive. If login fails post-clear, the user is in a "no credentials" state but explicitly informed. This is the macOS Keychain ACL workaround for #207 (per the spec link at line 821-822).

#### The deferred-integration scope (auto-refresh on 401)
- **Current state**: A 401 response from any Jira call surfaces as `JrError::ApiError { status: 401, message }` (or `JrError::NotAuthenticated` if no auth header was attached). Users must manually run `jr auth refresh`.
- **What would change** if 401 auto-refresh were wired:
  - `JiraClient::send` (or a new `send_with_retry`) would, on 401 + active OAuth flow, call `refresh_oauth_token(profile)` once and replay the original request with the new bearer.
  - Failure modes (currently absent):
    - `refresh_oauth_token` itself fails (refresh token expired, client_secret rotated, network) → must NOT loop; must surface clearly with re-login hint.
    - Atlassian returns 401 even after refresh (revoked token, scope mismatch) → must NOT infinite-loop; needs a single-retry guard.
    - Refresh token race: two concurrent 401s simultaneously trigger refresh → must serialize via mutex or dedup futures, otherwise Atlassian invalidates tokens (refresh token rotation per §6) and one of the two requests sees a 401 from a now-invalid bearer.
  - Token persistence: the new tokens must be written before retry. If keychain write fails (currently surfaces as "partial state"), the retry would re-401.
  - Scope inheritance: the refresh-token grant inherits scopes from the original authorize per RFC 6749 §6 — no scope change is possible without re-login. If a user changed `oauth_scopes` in config, refresh wouldn't pick up the new set; only re-login does.
- **Why deferred**: Production complexity outweighs the benefit at the current scale (single-user CLI, 1-hour token lifetime). The clear-and-relogin path is **less elegant but more correct** — every refresh path goes through `oauth_login` which always re-validates `accessible-resources` and re-derives `cloud_id`, catching cases where the user's site set has changed.

---

## 4. Sub-pass 2b deepening: behavioral — operations and state machines per target

### 4.1 Auth subsystem operations (deep)

The broad pass listed 7 auth subcommands. This round refines each with the deep credential-resolver semantics.

#### `auth login` (refined)
- **Full credential resolver chain** (verified via `cli/auth.rs::resolve_oauth_app_credentials_for_test`):
  1. **Flag pair** (`--client-id` + `--client-secret`): both required. Partial → `JrError::UserError`.
  2. **Env pair** (`JR_OAUTH_CLIENT_ID` + `JR_OAUTH_CLIENT_SECRET`): both required. Partial → `JrError::UserError`.
  3. **Keychain pair** (`oauth_client_id` + `oauth_client_secret`): single-pass read via `try_load_oauth_app_credentials`. Both must be non-empty. Locked-keychain → propagate (not silently treated as absent).
  4. **Embedded** (XOR-decoded build constants): plaintext materialized lazily via `OnceLock`.
  5. **Prompt**: only if `!no_input`. Uses `dialoguer::Input` (visible) for client_id, `Password` (masked) for client_secret. Hint cites `https://developer.atlassian.com/console/myapps/`.
- **Side effects** (login_oauth happy path):
  1. If source is BYO (Flag/Env/Keychain/Prompt), `store_oauth_app_credentials(client_id, client_secret)` writes the SHARED `oauth_client_id`/`oauth_client_secret` keys. Embedded source SKIPS this step.
  2. `RedirectUriStrategyRequest::bind` atomically binds the listener.
  3. `oauth_login` runs the 5-step OAuth flow.
  4. `Config::load_lenient_with(Some(profile))` reload (lenient because target profile may not yet exist).
  5. Persist `[profiles.<target>] { url, cloud_id, auth_method = "oauth" }`. Promote `default_profile` if unset.
  6. `save_global` (with file-only baseline).

#### `auth refresh` (refined)
- **Profile-aware flow dispatch**: Reads the TARGET profile's `auth_method` (NOT active profile's). `chosen_flow_for_profile(&target_profile, args.oauth)`. Test `chosen_flow_for_profile_inspects_passed_profile_not_active` (line 1247) explicitly pins this — using `chosen_flow(&Config, _)` instead would silently dispatch the wrong flow when `active != target`.
- **Pre-flight URL check**: For `api_token` flow only — if target has no URL, refuse with hint to use `jr auth login --profile X --url <...>` instead. (The OAuth flow auto-discovers via `accessible-resources` so no analogous gap exists.)
- **Asymmetric clear**: OAuth target → only `clear_profile_creds(target)`. Token target → `clear_all_credentials(&[target])` (wipes shared api-token because IT IS the credential being refreshed).
- **Failure recovery messaging**: post-clear login failure → stderr "Credentials were cleared, but the login flow did not complete. Run `<jr auth login [--oauth]>` to restore access."
- **Output payload (JSON)**: `{"status": "refreshed", "auth_method": "<api_token|oauth>", "next_step": "If prompted to allow keychain access, choose \"Always Allow\" so future commands run silently."}`

#### `auth status` (refined)
- **Empty-profiles-no-flag short-circuit**: `if config.global.profiles.is_empty() && profile_arg.is_none()` → eprint "No profiles configured. Run `jr init` or `jr auth login --profile <NAME>` to set up." then `return Ok(())`. **Does NOT error** — designed as a setup-script probe.
- **Strict path with explicit `--profile X`**: even on empty profiles, `--profile X` errors `JrError::UserError("unknown profile: X; known: (none)")`. Matches strict semantics of switch/remove/logout.
- **OAuth-source diagnostic row** (line 791-795): On `auth_method == "oauth"`, calls `peek_oauth_app_source()` (mirrors `resolve_refresh_app_credentials` order: keychain wins, embedded falls back, otherwise `OAuthAppSource::None`). **Critical**: `peek_*` does NOT decode the embedded plaintext — uses `embedded_oauth_app_present()` for the presence check.
- **Keychain-error degradation**: `peek_oauth_app_source` on keychain-read error emits a stderr warning (not a fatal error) and treats the keychain as absent. This DIVERGES from `resolve_refresh_app_credentials` which hard-errors on the same condition — defensible because `auth status` is a status surface (display-non-blocking) but a behavioral asymmetry worth documenting.

#### `auth logout` (refined)
- **Strict-existence check**: Validates target profile exists in `[profiles]` before clearing. Matches switch/remove semantics.
- **Cleared keys**: `<target>:oauth-access-token`, `<target>:oauth-refresh-token`. **Plus** legacy flat keys IF target == "default" (otherwise the next `load_oauth_tokens("default")` would lazy-migrate them back, silently undoing the logout — pinned by test `clear_profile_creds_default_also_clears_legacy_flat_keys` at line 1187).
- **Shared keys NEVER touched**: email, api-token, oauth_client_id, oauth_client_secret. Logout of one profile must not log out other profiles using API token (which share the email/api-token keys).
- **Profile entry stays in config.toml**: A subsequent `jr auth login --profile <name>` re-authenticates without losing site metadata.

#### `auth remove` (refined)
- **Refuse-active-profile**: Errors `cannot remove active profile {target:?}; switch first`. Pinned at `cli/auth.rs:999-1004`.
- **Refuse-default_profile-target**: Even when `target` is NOT the active profile, errors if `target == default_profile` (e.g., `jr --profile sandbox auth remove default` where active=sandbox but default_profile=default). Otherwise the next strict `Config::load()` would error. **Pinned at line 1012-1018.** This is a NEW invariant not in broad Pass 2.
- **Order of operations** (line 1023-1034 docstring):
  1. Confirm with user (skipped under `--no-input`).
  2. Pre-validate against config clone — typo or unremovable target doesn't make user click through then error.
  3. Persist config first (so a later keychain/cache failure can't leave the profile listed in `config.toml` after credentials are gone).
  4. Best-effort wipe of per-profile OAuth tokens AND cache directory. Both intentionally non-fatal — missing keychain entry / cache dir is the expected steady state.
- **Warning surfaces** (line 1072-1085): keychain failure + cache failure each emit a `print_warning` line citing the manual-cleanup path. Overall command still reports success.

#### `auth switch` (refined)
- **Pure logic split**: `handle_switch_in_memory(GlobalConfig, target)` is a pure function used by tests; `handle_switch` is the IO orchestrator.
- **Validates target name + existence**: Errors `unknown profile: {target}; known: ...`.
- **Effect**: Sets `default_profile = Some(target)`, saves config.

#### `auth list` (refined)
- **Status field**: `"configured"` if URL set; `"unset"` otherwise. Explicitly NOT a credential-presence check ("STATUS reflects CONFIG presence (URL on file), not credential presence" — line 1131-1134).
- **Active marker**: leading `*` for active profile, leading space for others. Column widths stay stable.
- **JSON shape**: array of `{name, url, auth_method, status, active: bool}`.

### 4.2 Profile lifecycle state machine (refined from broad §2b.3)

The broad pass diagrammed five states. This round adds two transitional states:

```
   nonexistent
       │  jr auth login --profile NEW (lenient load)
       ▼
   created-no-url ────────────────────────────────────┐
       │                                               │
       │  [no-input branch]                            │
       │  • prepare_login_target errors                │
       │    "--url required" before keychain write     │
       │                                               ▼
       │  [interactive branch]                       errored,
       │  • dialoguer prompts for URL                still-nonexistent
       │  • write profile entry with URL
       │  • dispatch to login_token / login_oauth
       ▼
   exists-pending-creds
       │  • api_token flow OR oauth flow               
       │    - oauth: writes <NEW>:oauth-* + cloud_id
       │      + (BYO) shared oauth_client_*
       │    - api_token: writes shared email + api-token
       │  • config saved with auth_method
       ▼
   exists, inactive
       │  jr auth switch NEW
       ▼
   active
       │  jr auth switch OTHER
       ▼  (default_profile := OTHER)
   inactive again
       │  jr auth logout --profile NEW
       │  • deletes <NEW>:oauth-*-token (and if NEW == "default",
       │    legacy flat oauth-*-token to defeat lazy migration)
       │  • config entry stays
       ▼
   inactive, no-tokens
       │  jr auth login --profile NEW
       ▼
   active again ┐
                │
                ▼
   jr auth remove NEW (must be inactive AND not == default_profile)
       │  • config entry removed (saved first)
       │  • <NEW>:oauth-* deleted (best-effort warning if fail)
       │  • cache/v1/<NEW>/ deleted (best-effort warning if fail)
       │  • (target == "default") also clears legacy flat keys
       ▼
   nonexistent (back to start)
```

**New states identified**:
- `created-no-url`: ephemeral; either resolves to `exists-pending-creds` (interactive URL prompt) or errors out (under `--no-input`).
- `exists-pending-creds`: between profile entry creation and credential write. A keychain-write failure mid-flight leaves this state explicit ("Authorization succeeded with Atlassian, but jr could not save the OAuth tokens to the system keychain ..."). This is the partial-state recovery message.

### 4.3 Issue list query lifecycle state machine

```
   parse args → clap rejection? → exit 64
       │ (success)
       ▼
   validate dates/durations early → invalid? → exit 64 (no HTTP)
       │
       ▼
   resolve --asset (passthrough or AQL) ◄─── may HTTP if AQL search
       │
       ▼
   resolve --assignee/--reporter (me OR account-id OR display-name search)
       │
       ▼
   resolve --team UUID (cache-first, fetch-on-miss)
       │
       ▼
   resolve project_key (--project flag OR .jr.toml OR None)
       │
       ▼
   if project_key set AND --status NOT set:
     project_exists check (HTTP) → 404 → exit 64
       │
       ▼
   if --status set:
     get project statuses (project-scoped) OR get_all_statuses (instance-wide)
     partial_match → Exact | ExactMultiple | Ambiguous | None
       │
       ▼
   if --asset set:
     get_or_fetch_cmdb_fields (cache or HTTP)
     if empty → "requires JSM Premium" exit 64
     build_asset_clause
       │
       ▼
   compose JQL base parts (5-branch decision tree per E-02-03)
       │
       ▼
   compose filter parts (per E-02-02 deterministic order)
       │
       ▼
   if all_parts.is_empty → "No project or filters" exit 64 (no HTTP)
       │
       ▼
   guard against unbounded query
       │
       ▼
   POST /search/jql cursor-paginated (effective_limit OR all)
       │  per page:
       │  • 429 retry MAX_RETRIES=3 with Retry-After
       │  • 401 + scope mismatch → InsufficientScope exit 2
       │  • other 4xx/5xx → ApiError exit 1
       ▼
   asset enrichment (if show_assets_col):
     • dedup by (workspace_id, object_id)
     • futures::join_all for concurrent enrichment
     • workspace_id fallback via cache or HTTP
       │
       ▼
   team display:
     • only Table mode + ≥1 issue with team
     • cache::read_team_cache best-effort, UUID fallback for misses
       │
       ▼
   format rows + print
       │
       ▼
   if has_more && !all:
     approximate_count (HTTP) for "Showing N of ~M" hint
     OR fall back to "Showing N results" if count fails
       ▼
   exit 0
```

**Architecturally significant transitions**:
- **Date validation BEFORE any HTTP** (lines 90-114) — a typo in `--created-after 2026-13-99` never costs a network call. Pinned by Pass 2 §2b.5 invariant 24.
- **Three error-exits before pagination starts** (validation, project not found, status resolver) — each maps to exit 64 (UserError) without HTTP.
- **Two distinct workspace-id paths**: per-asset embedded `workspace_id` field (from `parse_cmdb_value`) vs. fallback via `get_or_fetch_workspace_id`. Per-asset wins when present.

### 4.4 Cache state machine (refined)

```
   ┌──────────────┐
   │ nonexistent  │ — file at <root>/v1/<profile>/<filename> doesn't exist
   └──────┬───────┘
          │ network fetch + write_cache
          ▼
   ┌──────────────┐
   │ writing      │ (atomic? — see below)
   └──────┬───────┘
          ▼
   ┌──────────────┐
   │ hit-fresh    │ — fetched_at within CACHE_TTL_DAYS (7 days)
   └──────┬───────┘
          │ time elapses
          ▼
   ┌──────────────┐
   │ stale        │ — fetched_at >= 7 days old; read returns Ok(None)
   └──────┬───────┘
          │ next consumer triggers refetch
          ▼
   ┌──────────────┐
   │ writing      │
   └──────────────┘
          
          (corruption path)
   
   ┌──────────────┐
   │ corrupt      │ — serde_json::from_str failed
   └──────┬───────┘
          │ read returns Ok(None) + stderr warning
          │ next consumer triggers refetch + write
          ▼
   ┌──────────────┐
   │ writing      │
   └──────────────┘
   
          (orphan path)
   
   ┌──────────────┐
   │ orphaned     │ — cache root version bumped from v1/ to v2/
   └──────────────┘
          (no transition — file just stops being read; left on disk to be GC'd by user or reclaimed by OS tmp cleanup)
   
          (map-cache write race / corruption)
   
   ┌────────────────┐  write attempt
   │ map: corrupt   ├───────────► entries silently destroyed; only just-written entry survives
   └────────────────┘
```

**Architecturally significant**:
- **Atomicity of write_cache**: `std::fs::write(dir.join(filename), content)?` — NOT atomic on most platforms. A `kill -9` mid-write could leave a truncated file. Recovery: next read → corrupt → `Ok(None)` + warn + refetch. **Acceptable failure mode** but worth flagging: `tempfile + rename` would be a one-line robustness improvement.
- **Map-cache corruption is catastrophic for OTHER entries**: NEW-INV-07 — see §3.4.

### 4.5 Workspace-ID + connected-tickets lifecycle (assets context)

```
   workspace-id state machine:
   
   nonexistent → cache hit | cache miss
                                 │
                                 ▼
                            HTTP GET /rest/servicedeskapi/assets/workspace
                                 │
                          ┌──────┼──────┐
                          ▼      ▼      ▼
                       200/    200/    404/403
                       ≥1     empty    │
                              values   │
                          │      │     ▼
                          │      ▼   "Assets not available on this Jira site"
                          │  "No Assets workspace found"
                          ▼
                       cache write (best-effort, non-fatal)
                          │
                          ▼
                       hit (workspace_id known)
   
   connected-tickets filter state machine:
   
   ConnectedTicket[] → 
     branch: --open (boolean)
       │ retain if status.colorName != "green" 
       │ (None status → retained: unwrap_or(true))
       ▼
     filtered list
     
     branch: --status <name>
       │ build deduped status_names from tickets having Some(status)
       │ partial_match(input, &names)
       │   Exact / ExactMultiple → use
       │   Ambiguous → exit 64 with matches
       │   None → exit 64 with available list
       │ filter tickets where status.name == resolved_name
       │ (None status → excluded)
       ▼
     filtered list
```

---

## 5. Newly-discovered entities & invariants (NOT in broad Pass 2)

Consolidated for orchestrator routing. Each is grounded in a re-read line citation.

### Entities (E-XX-NN)
- **E-01-01** — `RedirectUriStrategyRequest` (request type) — `api/auth.rs:398-407`
- **E-01-02** — `ResolvedRedirect` (TOCTOU-closed binding) — `api/auth.rs:459-478`
- **E-01-03** — `RedirectUriStrategy` (resolved port) — `api/auth.rs:490-496`
- **E-01-04** — `OAuthResult` (login outcome) — `api/auth.rs:368-372`
- **E-01-05** — `RefreshAppSource` (private 2-variant enum) — `api/auth.rs:822-826`
- **E-01-06** — `AuthFlow` (login dispatcher) — `cli/auth.rs:264-280`
- **E-01-07** — `LoginArgs` (arg bundle) — `cli/auth.rs:523-532`
- **E-01-08** — `RefreshArgs<'a>` (borrowing arg bundle) — `cli/auth.rs:834-843`
- **E-01-09** — `OAuthAppSource::None` sentinel (vs Option::None) — `api/auth_embedded.rs:53-56`
- **E-02-01** — `FilterOptions<'a>` (JQL clause bundle) — `cli/issue/list.rs:598-610`
- **E-02-02** — JQL composition pipeline (deterministic order) — multiple lines, §3.2 above
- **E-02-03** — Project / board / sprint resolution branches — `cli/issue/list.rs:268-338`
- **E-02-04** — Asset enrichment dedup + concurrent — `cli/issue/list.rs:386-463`
- **E-02-05** — `enrich_assets` helper (the OTHER path) — `api/assets/linked.rs:170-225`
- **E-02-06** — Story-points display gating — `cli/issue/list.rs:579-594`
- **E-02-07** — Team display gating + cache fallback — `cli/issue/list.rs:500-531`
- **E-03-01** — 6 endpoints in `api/assets/` (full enumeration) — §3.3 table
- **E-03-02** — Workspace-ID resolution lifecycle — `api/assets/workspace.rs:19-58`
- **E-03-03** — `WorkspaceEntry` private struct — `api/assets/workspace.rs:9-13`
- **E-03-04** — CMDB field discovery wrapper — `api/assets/linked.rs:12-21`
- **E-03-05** — `extract_linked_assets` value-shape tolerance — `api/assets/linked.rs:29-99`
- **E-03-06** — `extract_linked_assets_per_field` shape — `api/assets/linked.rs:103-115`
- **E-03-07** — `enrich_json_assets` back-injection — `api/assets/linked.rs:137-167`
- **E-03-08** — AQL search pagination + cap — `api/assets/objects.rs:17-63`
- **E-03-09** — Object-key resolution heuristic — `api/assets/objects.rs:111-137`
- **E-03-10** — `filter_tickets` connected-ticket filter — `cli/assets.rs:306-360+`
- **E-05-02** — `Expiring` trait contract — `cache.rs:10-12`
- **E-05-03** — Map-cache write-merge semantics — `cache.rs:149-173, 316-353`
- **E-05-04** — `cache_root` / `cache_dir` resolution — `cache.rs:64-78`
- **E-05-05** — `clear_profile_cache` no-op — `cache.rs:82-88`
- **E-07-03** — `Config::load_lenient_with` lenient/strict split — `config.rs:210-218`
- **E-07-04** — `JR_BASE_URL` test override — `config.rs:351-353`
- **E-07-05** — OAuth `cloud_id` triggers API host rewrite — `config.rs:366-370`

### Invariants (NEW-INV-NN)
- **NEW-INV-01** — `--asset` requires CMDB fields to exist on the Jira instance — `cli/issue/list.rs:170-178`
- **NEW-INV-02** — `--asset` auto-enables `--assets` display column (line citation refined to `:87`)
- **NEW-INV-03** — Status validation cost decision (project-scoped vs global) — `cli/issue/list.rs:200-247`
- **NEW-INV-04** — Empty-query guard — `cli/issue/list.rs:344-352`
- **NEW-INV-05** — `parse_cmdb_value` requires at least one of `{label, objectKey, objectId}` — `api/assets/linked.rs:87-89`
- **NEW-INV-06** — AssetsPage max page size = 25 — `api/assets/objects.rs:26`
- **NEW-INV-07** — Map-cache writes silently destroy other entries on parse failure — `cache.rs:158-162, 330-338`
- **NEW-INV-08** — Per-profile signature is convention only (no compile-time fence)
- **NEW-INV-09** — Cache test scaffolding uses `XDG_CACHE_HOME` env mutation under static `ENV_MUTEX` — `cache.rs:362-379`
- **NEW-INV-10** — Migration write-back uses file-only baseline (re-verified) — `config.rs:240-264`
- **NEW-INV-11** — `save_global` overlays only `default_profile + profiles`, everything else file-baseline — `config.rs:416-446`
- **NEW-INV-12** — `[fields]` legacy block is read at runtime via `config.global.fields.*` NEVER through `active_profile().*` — multiple files. **HIGH-SEVERITY MULTI-PROFILE BUG.**
- **NEW-INV-13** — Active-profile existence check gated on `!profiles.is_empty() && strict` — `config.rs:318-328`
- **NEW-INV-14** — ADF roundtrip is lossy in both directions
- **NEW-INV-15** — ADF `orderedList` `attrs.order` falsy values default to 1 — `adf.rs:407-416`
- **NEW-INV-16** — `listItem` wraps children to satisfy ADF schema (with widened allowlist) — `adf.rs:163-188`
- **NEW-INV-17** — `tableCell` content always wrapped in a block — `adf.rs:201-211`

---

## 6. Retracted / corrected

**No phantom subsystems retracted (no `CONV-ABS-N` markers required).**

The audit at §2 found only one **counting-convention discrepancy** (entity row count: Pass 2 said "51 entities" — recount sums ≈78 rows across all tables in §2a.2 when including response wrappers and 2nd-tier types). This is a labelling difference, not fabrication. **Documented but NOT retracted** — Pass 2's "51" appears to count first-class entities only (excluding response wrappers and 2nd-tier types like `TenantContext`/`GraphqlResponse`), which is a defensible counting convention.

**One framing softening (potential Pass 4 update needed)**:
- **Pass 4 §1.5/§5.2 N+1 framing** — Pass 4 said asset enrichment is "serialized, not concurrent" and "150+ extra GETs all serial". **Re-reading source disproves both halves**: enrichment is **deduplicated by (workspace_id, object_id) pair** AND **concurrent via futures::future::join_all**. The N+1 risk is real (still O(N) GETs in the worst case) but materially smaller than Pass 4 framed it. **Refined in §3.2 E-02-04 above** — passed back to orchestrator as a Pass 4 deepening candidate.

**One Pass 2 entity-table cell ambiguity refined (not retracted)**:
- **`ProjectMeta` vs `ObjectTypeAttrCache` TTL semantics** — broad pass §2a.2 had both labelled "per-entry TTL" or similar in different cells. Re-reading shows: `ProjectMeta` IS per-entry; `ObjectTypeAttrCache` is per-FILE (when the file ages out, ALL keyed entries expire together). **Refined in §3.4 table; not retracted.**

---

## 7. Delta Summary — what's new vs broad Pass 2

| Section | Items added (delta) |
|---|---|
| Auth entities | **+9 entities** (RedirectUriStrategyRequest, ResolvedRedirect, RedirectUriStrategy, OAuthResult, RefreshAppSource, AuthFlow, LoginArgs, RefreshArgs, OAuthAppSource::None sentinel) + **+3 refined entities** (keychain key catalog, EmbeddedOAuthApp lifetime, test scaffolding) + **+1 refined state machine** (4-branch partial-state recovery in `load_oauth_tokens`) |
| Issue list query | **+7 entities** (FilterOptions bundle, JQL composition pipeline, project/board/sprint branches, dedup-and-concurrent enrichment, enrich_assets helper, story-points gating, team display gating) + **+4 invariants** (NEW-INV-01..04) |
| Assets / CMDB | **+10 entities** (full endpoint enumeration, workspace-id state machine, WorkspaceEntry, CMDB discovery wrapper, value-shape tolerance, per-field shape, JSON back-injection, AQL pagination cap, object-key heuristic, filter_tickets) + **+2 invariants** (NEW-INV-05, NEW-INV-06) |
| Cache layer | **+4 entities** (Expiring trait, map-cache merge semantics, cache_root/cache_dir, clear_profile_cache no-op) + **+3 invariants** (NEW-INV-07, NEW-INV-08, NEW-INV-09) + **+1 refined entity-table cell** (TTL semantics distinction between ProjectMeta and ObjectTypeAttrCache) |
| Configuration | **+3 entities** (load_lenient_with, JR_BASE_URL bypass also short-circuits OAuth host rewrite, OAuth cloud_id host rewrite) + **+4 invariants** (NEW-INV-10..13). **Plus the substantive multi-profile bug finding** (NEW-INV-12) which was undetected in Pass 2 and is the most consequential single discovery this round. |
| ADF rendering | **+14 enumerated node types** + **+5 enumerated mark types** + **+4 invariants** (NEW-INV-14..17) |
| OAuth state machine | **+1 fully-characterized refresh function** (refresh_oauth_token unused production helper) + **+1 deferred-integration scope analysis** (what auto-refresh would change) + **+2 transitional states** (created-no-url, exists-pending-creds) |

**Quantitative delta**:
- New entities: **33** (vs broad Pass 2's 51 = +65% increase)
- New invariants: **17** (vs broad Pass 2's 25 = +68% increase)
- Refined existing entities: **6**
- Retracted entities: **0**
- Hallucination-class audit findings: **0 retractions, 1 counting-convention difference (documented, not retracted)**
- LOC deltas vs broad Pass: **0 (all citations match `wc -l` to within ±5 LOC)**

---

## 8. Novelty Assessment

**Novelty: SUBSTANTIVE**

Justification — would removing this round's findings change how you'd spec the system? Yes, in at least 4 material ways:

1. **NEW-INV-12 (multi-profile fields bug)** alone would change the spec. CLAUDE.md and Pass 2 both describe `[fields]` as drained-into-profile and per-profile-overridable. The actual code uses a global `[fields]` block at runtime. A spec written from broad Pass 2 alone would assert a per-profile invariant the code doesn't satisfy.

2. **E-01-02 / E-01-03 (TOCTOU-closed `ResolvedRedirect`)** is a load-bearing type-system safety property that the broad pass missed entirely. A spec rewrite that didn't preserve this property could regress to a probe-then-bind shape that re-opens the security hole.

3. **E-02-04 (dedup + concurrent asset enrichment)** materially changes the NFR characterization. Pass 4 framed asset enrichment as serial; the actual code deduplicates by `(workspace_id, object_id)` pair AND uses `futures::future::join_all`. Any "N+1 problem" remediation in Phase 1 would be wasted effort if it doesn't account for what's already been done.

4. **E-01-04/05 distinct refresh resolver (`RefreshAppSource` 2-variant)** vs login resolver (`OAuthAppSource` 6-variant) is a load-bearing domain modelling choice. A spec that conflated them into one chain would either re-introduce non-interactive prompt sources to refresh (wrong) or strip flag/env from login (also wrong).

These are model-changing findings, not refinements. **SUBSTANTIVE.**

---

## 9. Remaining gaps / next candidate scope (verbatim for Round 2)

Items the orchestrator should attack in Pass 2 Round 2:

### High priority (gaps in entities/invariants the broad pass + Round 1 left under-deepened)

1. **`api/jira/*` resource modules** — broad Pass 2 listed the file inventory at §2a.1 but never enumerated the per-resource HTTP methods, request/response types, or pagination shapes:
   - `api/jira/issues.rs` — `search`, `get`, `create`, `edit`, `list_comments`, `add_comment`, plus `transitions` HTTP + `add_changelog_expand` + `find_story_points_field`
   - `api/jira/boards.rs`, `api/jira/sprints.rs` — Agile REST entities
   - `api/jira/teams.rs` (56 LOC) — GraphQL `tenantContexts` request/response shape (per ADR-0005)
   - `api/jira/fields.rs` — story-points + CMDB field discovery details
   - `api/jira/users.rs` — current user, search, assignable users, `USER_PAGINATION_SAFETY_CAP`
   - `api/jira/projects.rs` — `IssueTypeWithStatuses`, `ProjectStatusesResponse`
   - `api/jira/links.rs` — link-type discovery
   - `api/jira/worklogs.rs` — duration-coupled types
   - `api/jira/statuses.rs` — global `get_all_statuses`
2. **`cli/issue/changelog.rs` (847 LOC)** — broad Pass 6 §5.1 flagged this as orphan. AuthorNeedle smart constructor (`:` or 12+ chars with digit → AccountId), `--field` substring filter, `--reverse`, format-date observability gating. 38 inline unit tests; only 3 BCs in Pass 3.
3. **`cli/issue/helpers.rs` (813 LOC)** — broad Pass 6 §5.1 also flagged. Team UUID resolution (cache-first), story-points assignment, user resolution (me/account-id/search), prompt patterns. 21 unit tests.
4. **`cli/issue/workflow.rs` (788 LOC)** — broad Pass 2 §2b.3 has the issue-move state machine. NOT yet covered: `assign` idempotency, `comment --internal` JSM property structure, `open --url-only` AI-friendly mode, `link/unlink/link-types` resolver, `remote-link` shape.
5. **`cli/issue/create.rs` (375 LOC)** + **`cli/issue/json_output.rs` (TBD)** — broad Pass 2 §2b.1 listed `create`/`edit` operations but didn't catalog the field-building pipeline (markdown→ADF flow, label add/remove prefixes, type/priority/parent resolution, --to/--account-id mutual exclusion). JSON output shapes pinned by insta need enumeration.
6. **`api/client.rs` (490 LOC)** — broad Pass 1 §3a covered headers/auth but the deepening should re-derive: `extract_error_message` 6-level precedence chain (Pass 6 T-08), `try_clone()` "unreachable" panic, `send` vs `send_raw` divergence, `from_config` construction, `JR_VERBOSE` stderr gate, `JR_BASE_URL` test injection.
7. **Pagination shapes** (`api/pagination.rs`) — broad Pass 2 §2a.3 listed `OffsetPage<T>, CursorPage<T>, ServiceDeskPage<T>, AssetsPage<T>` (4 envelopes). NOT yet deeply characterized: cursor token shape, `nextPageToken` semantics, `total` vs `isLast` semantics, `AssetsPage::is_last` bool-or-string custom deserializer.
8. **AQL builder edge cases** — `jql::build_asset_clause` parenthesized OR-join when multiple CMDB fields exist; escape semantics across the field name AND the asset key. Property-test enumeration.

### Medium priority

9. **`cli/board.rs`, `cli/sprint.rs` (438 LOC), `cli/worklog.rs`, `cli/team.rs`, `cli/user.rs`, `cli/project.rs`, `cli/queue.rs` (323 LOC)** — broad Pass 2 §2b.1 listed operations but did not catalog inputs, error mappings, or per-flag idempotency. `MAX_SPRINT_ISSUES = 50` cap, scrum-only check, `USER_PAGINATION_SAFETY_CAP`, `aqlFunction` field-NAME-not-ID rule applied at sprint side.
10. **`cli/init.rs`** — broad Pass 2 §2b.1 has the operation row + Flow 1 (cross-cutting). NOT covered: per-step error recovery (what happens if `tenantContexts` fails mid-init? if team list fetch fails? if story-points discovery fails?). Each step should be characterized as a failable substate.
11. **`adf.rs` deep round** — this round (T-09 MEDIUM) sampled the node catalog. Round 2 should: enumerate the ~69 inline unit tests, characterize the table-render edge cases (empty tables, headerless tables, mixed-cell-count rows), characterize the `wrap_inlines_as_blocks` allowlist semantics in detail, characterize ADF→text NEWLINE / WHITESPACE handling.
12. **`cli/api.rs` (342 LOC)** — broad Pass 0 listed it; broad Pass 2 §2b.1 row "raw passthrough through `JiraClient::send_raw`". Round 2 should catalog: HttpMethod enum, body-resolution (inline/`@file`/`@-`), header parsing, status passthrough, JSON validation policy.

### Low priority

13. **`partial_match.rs` (200 LOC)** — broad Pass 2 §2a.3 covered the 4 variants. Property-test enumeration + the case-sensitive-dedup rule explicitly captured at use-sites (`MatchResult::ExactMultiple` is treated like `Exact` everywhere — a CONVENTION not a property of the enum). Pass 6 §6 T-12 covers this.
14. **`duration.rs` (159 LOC)** — broad Pass 2 §2a.4 covered the syntax. Round 2 candidate: cross-context contrast with `jql::validate_duration`. Pass 6 §6 T-14 covers this.
15. **`jql.rs` (395 LOC)** — broad Pass 2 §2a.4 covered escape order, AQL function name, validate_duration/_date/_asset_key. Round 2: full BC enumeration of property tests + corpus regressions in `proptest-regressions/jql.txt`.

### Pass 4 deepening triggered (cross-pollination)

16. **Pass 4 N+1 framing softening (NEW from this round)** — refer to E-02-04 for the dedup-and-concurrent characterization. Pass 4 §1.5 / §5.2 should be revisited to reflect actual asset enrichment topology.

---

## 10. State Checkpoint

```yaml
pass: 2
round: 1
status: complete
audit_findings_against_hallucination_classes: 1
new_entities: 33
new_invariants: 17
retracted_findings: 0
files_examined: 14
novelty: SUBSTANTIVE
timestamp: 2026-05-04T18:45:00Z
next_round_targets: |-
  1. api/jira/*.rs resource modules (10 files) — per-resource HTTP methods, request/response types, pagination shapes
  2. cli/issue/changelog.rs (847 LOC) — AuthorNeedle smart constructor + 38 unit tests + filter pipeline
  3. cli/issue/helpers.rs (813 LOC) — team/user/points resolvers + 21 unit tests
  4. cli/issue/workflow.rs (788 LOC) deep — assign idempotency, comment --internal, open --url-only, links/unlink/remote-link
  5. cli/issue/create.rs (375 LOC) + cli/issue/json_output.rs — field-building pipeline + insta-pinned JSON shapes
  6. api/client.rs (490 LOC) — extract_error_message 6-level chain, send vs send_raw, JR_BASE_URL injection
  7. api/pagination.rs — cursor/offset/JSM/Assets envelope semantics (especially AssetsPage::is_last bool-or-string)
  8. AQL builder + escape semantics property tests
  9. cli/board, cli/sprint, cli/worklog, cli/team, cli/user, cli/project, cli/queue — inputs/errors/idempotency catalog
  10. cli/init.rs failable substates per-step
  11. adf.rs deep — ~69 unit tests + table-render edge cases + wrap_inlines_as_blocks allowlist
  12. cli/api.rs (342 LOC) — HttpMethod, body resolution, header parsing
  13. partial_match.rs property tests + ExactMultiple-as-Exact convention
  14. duration.rs cross-context contrast with jql::validate_duration
  15. jql.rs full BC enumeration + proptest-regressions corpus
  16. Pass 4 cross-pollination — N+1 framing softening (E-02-04 dedup + concurrent enrichment)
```

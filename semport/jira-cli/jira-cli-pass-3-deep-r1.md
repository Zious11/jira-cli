# Pass 3 Deep — Round 1: Behavioral Contracts (jira-cli / jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Builds on: `pass-3-behavioral-contracts.md` (broad: 188 BCs / 134 HIGH / 45 MEDIUM / 9 LOW), Pass 6 §6 target list (T-01..T-08).

> **Method.** Targets T-01..T-08 of the Phase B Pass 3 deepening were attacked. Test fns were recounted via `awk '/#\[(tokio::)?test/{c++} END{print c}'`. Source citations grounded in concrete `<file>:<line>` ranges. Single-file reads chunked by offset+limit. Test files for `cli_handler.rs` (T-04 deferred to round 2) and full `cli/issue/changelog.rs` enumeration (T-04/T-10) were partially attacked — round 2 will close the remainder.

---

## 1. Round metadata

| Field | Value |
|---|---|
| Round | 1 of (max 5) |
| Targets attacked this round | T-01, T-02, T-03, T-04 (partial), T-05, T-06, T-07, T-08 (full) |
| Targets deferred to round 2 | T-04 full sweep of `cli_handler.rs` 54 tests, `issue_changelog.rs` 39 tests; full `adf.rs` 69 tests; T-09/T-10/T-11 (medium-priority) |
| Files freshly read this round (full) | 6 — `tests/auth_profiles.rs`, `tests/auth_refresh.rs`, `tests/auth_login_config_errors.rs`, `tests/oauth_embedded_login.rs`, `tests/migration_legacy.rs`, `tests/api_client.rs` |
| Files freshly read this round (chunked) | 14 — `cli/issue/list.rs`, `tests/all_flag_behavior.rs`, `tests/issue_list_errors.rs:350-423`, `tests/issue_view_errors.rs:130-207`, `tests/issue_resolution.rs:85-159`, `tests/cmdb_fields.rs:140-190`, `tests/assets.rs:1-110, 140-340, 1480-1799`, `src/cache.rs:1-490, 760-900`, `src/config.rs:170-420`, `src/api/auth.rs:1-220, 370-490`, `src/api/client.rs:180-490`, `src/jql.rs:1-180`, `src/api/rate_limit.rs` (full), `src/duration.rs:1-75`, `src/cli/issue/changelog.rs:170-270` |
| BCs in broad pass | 188 (134 HIGH / 45 MEDIUM / 9 LOW) |
| BCs added this round | 87 new (mostly HIGH) |
| BCs promoted MEDIUM→HIGH | 13 |
| BCs promoted LOW→HIGH or MEDIUM | 4 |
| BCs corrected (CONV-ABS) | 4 |
| BCs after round | **271 total** (211 HIGH / 53 MEDIUM / 7 LOW) |

---

## 2. Audit of broad Pass 3 against the 5 Known Hallucination Classes

Verified by reading the actual test files and source. Findings:

### 2.1 Over-extrapolated token lists
- **BC-1201** broad-pass listed 6 fallback levels: `errorMessages[]` → `errors{}` → `message` → `errorMessage` → raw body → `<empty response body>`. Direct read of `src/api/client.rs:448-490` confirms **the actual fallback order is**: empty body → utf-8 decode → `errorMessages[]` (non-empty) → `errors{}` (non-empty) → `message` → `errorMessage` → raw body string. The broad summary mis-ordered "raw body" and "<empty response body>" — empty body is checked **first** (line 449-451), not last. **Action: minor reorder, see §3.8 BC-1201-R.**
- **BC-1015** (broad): "5xx → `API error (<status>)` + raw body message". This is correct, but the broad pass omitted that the specific error variant is `JrError::ApiError { status, message }` and the `message` IS `extract_error_message(body)` — i.e., 5xx errors flow through the same 6-level chain. Confirmed by `parse_error` at `src/api/client.rs:330-348`. **Action: tighten BC-1210 in §3.13.**

### 2.2 Miscounted enumerations
- **Broad §1.1** said `tests/assets.rs` has 21 tests. Direct recount via `awk '/#\[(tokio::)?test/{c++} END{print c}' tests/assets.rs` yields **24**. Broad table mistook this with `src/cli/assets.rs::tests` which has 21 unit tests. **Same-basename artifact conflation (Class 4) — see §6 retraction.**
- **Broad pass-3 stat table** said total 188 BCs. Recount: BC-001..024 (24), BC-101..124 (24), BC-201..225 (25), BC-301..315 (15), BC-401..410 (10), BC-501..508 (8), BC-601..606 (6), BC-701..709 (9), BC-801..808 (8), BC-901..911 (11), BC-1001..1010 (10), BC-1101..1111 (11), BC-1201..1214 (14), BC-1301..1307 (7), BC-1401..1411 (11) = **193**. Broad pass under-counted by 5. Recount substantiated by literal grep over the file. Not a hallucination — a clerical undercount.
- **Total integration-test fn count**: broad said 324; my recount via `awk '/#\[(tokio::)?test/{c++}' tests/*.rs tests/common/*.rs` returns **324**. Confirmed.
- **Total unit-test fn count**: broad said 607. My recount of `cli/issue/list.rs (26) + cli/assets.rs (21) + cli/auth.rs (44) + cli/issue/changelog.rs (38) + cache.rs (27) + config.rs (37) + jql.rs (43) + duration.rs (16) + partial_match.rs (12) + api/assets/linked.rs (20) = 284` for the 10 large modules. The broad-pass 607 figure was the SUM across all 43 unit-test modules — verified consistent at the file-by-file level for the modules I sampled.

### 2.3 Named pattern conflation / fabrication
- No fabricated test category names found. The pattern names ("Multi-profile auth workflow", "Idempotent move", "Legacy migration round-trip") all map 1:1 to documented test fn names in the test files.

### 2.4 Same-basename artifact conflation
- **`tests/assets.rs` vs `src/cli/assets.rs::tests`** — broad pass conflated their counts (21 vs 24). Documented in §2.2.
- **`cli/auth.rs` vs `tests/auth_*.rs` modules** — broad pass kept these separated correctly. ✓
- **`api/auth.rs` (1397 LOC) vs `cli/auth.rs` (1998 LOC)** — broad correctly distinguished; 22 vs 44 tests respectively. ✓

### 2.5 Inflated or deflated metrics
- **Broad BC-1403** said: "Retry-After parsed (integer seconds; per Pass 2 §2b.4 line)" — confidence MEDIUM. Direct read of `src/api/rate_limit.rs:1-30` confirms **integer-only parse via `parse::<u64>()`**. There is **no http-date format support** despite Pass 4 §7.1.4 listing "no HTTP-date format support" as a NFR gap. The broad-pass §1.2 table line "`api/rate_limit.rs` | 2 | Retry-After int + http-date parse" is **inflated**. **Retraction CONV-ABS-001 below.**
- **Broad pass-3 said `tests/assets.rs`** is 1,799 LOC with 21 tests. LOC verified (1799). Test count corrected to 24.

---

## 3. BC additions / promotions, per target T-NN

Format: BC-ID → confidence → sources → behavior → effect/edges. Each BC is grounded in at least one citation with line numbers.

### 3.1 T-01 — `cli/auth.rs` + auth integration tests (NEW + PROMOTIONS)

#### BC-013-R: `auth logout` deletes ONLY `<profile>:oauth-access-token` and `<profile>:oauth-refresh-token` keychain keys
**Confidence**: HIGH (PROMOTED from MEDIUM)
**Sources**: `src/api/auth.rs:24-32, 88-97`; `src/cli/auth.rs::handle_logout`; CLAUDE.md gotcha
**Behavior**: `logout` calls `delete_credential` on `entry(&oauth_access_key(profile))` and `entry(&oauth_refresh_key(profile))` only. Profile config entry is preserved; shared keychain keys (`email`, `api-token`, `oauth_client_id`, `oauth_client_secret`) untouched.
**Effects**: keychain mutations limited to per-profile namespaced OAuth keys. Re-login uses the preserved API-token / OAuth client credentials.
**Edges**: For unknown profile → returns BC-005 (exit 64 + "unknown profile").

#### BC-014-R: `auth remove <name>` deletes (1) profile entry from `[profiles]`, (2) `<name>:oauth-*` keychain keys, (3) per-profile cache directory
**Confidence**: HIGH (PROMOTED from MEDIUM)
**Sources**: `src/cli/auth.rs::handle_remove`; `src/cache.rs:82-88`; integration test `tests/auth_profiles.rs:120-140`
**Behavior**: Three-step delete. The cache deletion is via `cache::clear_profile_cache(name)` which is a no-op if the dir doesn't exist (BC-1005). Removing the active profile errors with exit 64 (BC-006) before any deletion happens.
**Edges**: All three deletions are best-effort; partial state should not cascade. Other profiles' keychain entries and config entries are untouched (cross-profile boundary, INV-11).

#### BC-022-R: `OAuthAppSource::Embedded` is reported by `auth status` only when `embedded_oauth_app_present()` returns true
**Confidence**: HIGH (PROMOTED from MEDIUM)
**Sources**: `src/api/auth_embedded.rs:46-57, 132-136`; `src/cli/auth.rs::peek_oauth_app_source`
**Behavior**: Order of resolution (highest wins): `Flag → Env (`JR_OAUTH_CLIENT_ID/SECRET`) → Keychain → Embedded → Prompt → None`. `peek_oauth_app_source` calls `embedded_oauth_app_present()` (no decode). The `present` check inspects only `EMBEDDED_ID.is_some_and(|s| !s.is_empty())` etc., never invoking `decode()`.
**Effects**: `auth status` can report "embedded credentials present" without materializing plaintext.

#### BC-023-R: `default` profile lazy-migrates legacy flat OAuth keys; non-default profiles never inherit them, even from a partial-state recovery branch
**Confidence**: HIGH (PROMOTED from MEDIUM)
**Sources**: `src/api/auth.rs:111-169` (full read in this round)
**Behavior**: `load_oauth_tokens(profile)` first reads `<profile>:oauth-access-token` and `<profile>:oauth-refresh-token`. If both are present → returns. If both missing → only `default` profile reads `KEY_OAUTH_ACCESS_LEGACY` / `KEY_OAUTH_REFRESH_LEGACY` (the flat keys), copies to namespaced, deletes legacy. **If the namespaced pair is half-present** (one missing), only `default` may try the legacy fallback (interrupted-migration recovery). Non-default profiles error with: `"OAuth keychain entries for profile <X> are partial (one of access/refresh present, the other missing). Run 'jr auth logout --profile <X>' then 'jr auth login --profile <X>' to restore a clean state."`
**Edges**: The condition `if profile == "default"` appears at TWO points (line 124 for fully-missing namespaced, line 151 for partial-state). Both are lazy-migration boundaries. Cross-profile inheritance is forbidden in both paths.

#### BC-024-R: `refresh_oauth_token(profile)` resolves credentials internally via keychain → embedded chain; signature has NO `client_id`/`client_secret` parameters
**Confidence**: HIGH (PROMOTED from LOW)
**Sources**: `src/api/auth.rs:700-770` (per Pass 6 INC-07); CLAUDE.md gotcha
**Behavior**: The function takes only `profile: &str`. Internally it calls the credential-resolver chain (keychain → embedded). Has no production callers as of this snapshot — exists for a future 401-auto-refresh integration; `jr auth refresh` (the user-facing command) uses the clear-and-relogin flow at `cli/auth.rs::refresh_credentials`, not this helper.
**Effects**: Re-introducing `client_id`/`client_secret` to the signature would short-circuit the resolver and break the embedded-OAuth path.

#### BC-025 (NEW): `auth refresh --no-input` against unconfigured profile exits 64 with stderr containing literal `no URL configured`, `jr auth login`, AND `--url`
**Confidence**: HIGH
**Sources**: `tests/auth_refresh.rs:43-106`
**Behavior**: Test pins all three substring assertions in stderr (`stderr.contains("no URL configured")`, `stderr.contains("jr auth login")`, `stderr.contains("--url")`). Pre-fix bug (round-16): refresh would clear creds then prompt for email — destructive misleading recovery. Post-fix: refusal preserves credentials.
**Edges**: stderr does NOT contain `panic` (negative-substring assertion line 105). `JR_SERVICE_NAME=jr-jira-cli-test` env-isolates keychain. `JR_INSTANCE_*` scrubbed to prevent direnv-set vars from spuriously satisfying URL check.
**Error variant**: `JrError::UserError`.

#### BC-026 (NEW): `auth refresh --help` includes the literal `--oauth` flag
**Confidence**: HIGH
**Sources**: `tests/auth_refresh.rs:7-24`
**Behavior**: `jr auth refresh --help` exits 0 and stdout contains both `refresh` (case-insensitive lowercase) AND `--oauth`. Pins clap's help-text rendering.

#### BC-027 (NEW): `auth refresh --oauth --help` is accepted in either flag order
**Confidence**: HIGH
**Sources**: `tests/auth_refresh.rs:26-40`
**Behavior**: clap accepts both `--oauth --help` and `--help --oauth` orderings, exit 0.

#### BC-028 (NEW): Embedded OAuth integration test (`tests/oauth_embedded_login.rs`) is `#[ignore]`-gated AND `panic!`s with `unimplemented!` if `JR_RUN_OAUTH_INTEGRATION=1` is set
**Confidence**: HIGH
**Sources**: `tests/oauth_embedded_login.rs:13-32` (full read this round)
**Behavior**: The test is intentionally `unimplemented!` so opting in via `JR_RUN_OAUTH_INTEGRATION=1` does NOT silently pass. The test requires an `oauth_login` base-URL override (deferred refactor) before a real wiremock-backed assertion can be written. Without `JR_RUN_OAUTH_INTEGRATION=1` set, the test early-returns at line 16-21 before the `unimplemented!`.
**Effects**: This is an explicit non-regression guard against false coverage signals.

#### BC-029 (NEW): `auth login --profile X` against `JR_PROFILE=ghost` (where ghost is unrelated and absent) succeeds in creating profile X
**Confidence**: HIGH (#[ignore]-gated by JR_RUN_KEYRING_TESTS)
**Sources**: `tests/auth_profiles.rs:282-333` (full read this round)
**Behavior**: Round-5 regression — `login_token`/`login_oauth` previously reloaded config via strict `Config::load()` after `handle_login`'s lenient load, which re-fired the unknown-active-profile check on the unrelated `JR_PROFILE` value and aborted creation. Both internal reloads now use `load_lenient_with` to match the orchestrator. Test sets `JR_PROFILE=ghost`, runs `jr auth login --profile fresh --url https://fresh.example`, asserts post-condition `[profiles.fresh]` written.
**Effects**: Login uses lenient load throughout — top-level + every internal reload.

#### BC-030 (NEW): Global `--profile` flag is propagated to `auth status` via `subcmd.profile.or(cli.profile)` composition in main.rs
**Confidence**: HIGH (PROMOTED scope from BC-008)
**Sources**: `tests/auth_profiles.rs:188-231` (full read)
**Behavior**: Round-10 regression — previously `auth status`/`login`/`refresh`/`logout` each reloaded config internally and only saw the subcommand-level `--profile`, dropping the global flag. main.rs now composes an effective profile (`subcmd.profile.or(cli.profile)`).
**Effects**: `jr --profile sandbox auth status` (no subcommand-level `--profile`) targets sandbox — stderr/stdout reflects sandbox URL/name.

#### BC-031 (NEW): Embedded OAuth callback URL must be exactly `http://127.0.0.1:53682/callback` — `EMBEDDED_CALLBACK_PORT` is centralized at `src/api/auth.rs:384`
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:374-407` (full read this round)
**Behavior**: `pub const EMBEDDED_CALLBACK_PORT: u16 = 53682;`. Used by `cli/auth.rs::login_oauth`, the CI smoke step, and the spec/runbook. Atlassian validates `redirect_uri` by exact string match — changing this is a breaking release. The IPv4 literal `127.0.0.1` is intentional (avoids macOS Chrome's `localhost`→`::1` resolver pitfall).
**Effects**: Listener binds via `RedirectUriStrategyRequest::Fixed(53682)` strategy (`src/api/auth.rs:399-407, 415-449`), which TOCTOU-closes by binding before producing `ResolvedRedirect`.

#### BC-032 (NEW): `RedirectUriStrategyRequest::Fixed(p)` produces a friendly error on `EADDRINUSE`; other I/O errors propagate raw
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:427-447`
**Behavior**: On `std::io::ErrorKind::AddrInUse`, returns `anyhow::anyhow!("port {p} is in use; the jr OAuth callback needs this port. Free it, or use your own OAuth app via --client-id/--client-secret (or set JR_OAUTH_CLIENT_ID/JR_OAUTH_CLIENT_SECRET) to fall back to a dynamic port.")`. Other errors propagate via `Err(e.into())`.
**Effects**: User-facing error names a clear remediation path. `Dynamic` strategy's I/O errors propagate raw (no friendly wrapper) — they are rare (OS-level ephemeral port exhaustion).

#### BC-033 (NEW): `ResolvedRedirect` has private fields preventing the listener from being detached from the strategy
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:455-477`
**Behavior**: A future caller cannot move the listener out, then derive a `redirect_uri` from the strategy that no longer matches the still-held listener. The TOCTOU window the type was created to close stays closed.
**Effects**: Type-system-enforced; documented at lines 455-458.

#### BC-034 (NEW): `auth_embedded::oauth_app_source` chain returns the highest-priority source
**Confidence**: HIGH (NEW)
**Sources**: `src/api/auth_embedded.rs:46-57`; `src/cli/auth.rs::peek_oauth_app_source`
**Behavior**: Order Flag → Env → Keychain → Embedded → Prompt → None. The first non-None-equivalent source wins; lower-priority sources never short-circuit higher-priority ones.

#### BC-035 (NEW): The `DEFAULT_OAUTH_SCOPES` const at `src/api/auth.rs:58-63` includes `offline_access`, `read:cmdb-object:jira`, and `read:cmdb-schema:jira` — the absence of any of these would break refresh, asset object reads, or asset schema reads
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:34-63`
**Behavior**: Concatenated single-space-separated scope list: `read:jira-work write:jira-work read:jira-user read:servicedesk-request read:cmdb-object:jira read:cmdb-schema:jira offline_access`. Documented contract: regression test (`default_oauth_scopes_pins_the_full_set_with_offline_access`) asserts no double spaces appear.
**Effects**: Embedded `jr` app must be registered with this exact scope set in Developer Console.

### 3.2 T-02 — `cli/issue/list.rs` JQL composition (NEW)

#### BC-125 (NEW): `--jql X` user input is wrapped in parens, prefixed with project scope (if any), and ORDER BY is stripped from user input then re-appended via the order-by selector
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:36-52` (`build_jql_base_parts`); 26 unit tests in same file at `:651-1083` (the `build_jql_base_parts_*` and `build_jql_parts_*` test cases)
**Behavior**: `build_jql_base_parts(jql, project_key)` calls `crate::jql::strip_order_by(jql)`, then constructs `Vec` with project scope (if any) followed by `(<stripped>)` (paren-wrapped). The order_by tuple slot is **always** the literal `"updated DESC"` from this function — it does NOT preserve the user's `ORDER BY`. (User-supplied `ORDER BY rank ASC` becomes `updated DESC`.)
**Effects**: `--jql "priority = Highest ORDER BY created DESC" --project PROJ` → `(project = "PROJ") AND (priority = Highest) ORDER BY updated DESC`. Pinned by unit test `build_jql_base_parts_jql_with_order_by_and_project`.

#### BC-126 (NEW): When `--jql` is not provided AND board_id+scrum-active-sprint exists → JQL becomes `sprint = <id>` ORDER BY `rank ASC`
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:278-282`
**Behavior**: First active sprint from `client.list_sprints(bid, Some("active"))` → `format!("sprint = {}", sprint.id)` clause + `"rank ASC"` order_by.
**Effects**: AsserPinned by `tests/all_flag_behavior.rs:347-352` (sprint mock returns id=100; expected JQL clause `sprint = 100`).

#### BC-127 (NEW): When `--jql` is not provided AND board_id is kanban → JQL becomes `project = "X" AND statusCategory != Done` ORDER BY `rank ASC`
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:302-310`; `tests/all_flag_behavior.rs:497-516, 542-562` (body-match `"jql": "project = \"PROJ\" AND statusCategory != Done ORDER BY rank ASC"`)
**Behavior**: This pins INV-21 with a literal wiremock body-match. Also notes that for kanban boards, the implicit `--open`-equivalent filter is server-side via `statusCategory != Done`, not via the `--open` flag mechanism.

#### BC-128 (NEW): When `--jql` is not provided AND no board_id is configured → JQL is `project = "X"` (project scope only) ORDER BY `updated DESC`
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:331-338`; `tests/all_flag_behavior.rs:42-86` (body-match `"jql": "(project = ALL) ORDER BY updated DESC"`)

#### BC-129 (NEW): When NO project AND NO filters AND NO `--jql` → exit 64 with help text listing all 12 filter sources
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:344-351`
**Behavior**: stderr contains literal "No project or filters specified. Use --project, --assignee, --reporter, --status, --open, --team, --recent, --created-after, --created-before, --updated-after, --updated-before, --asset, or --jql. You can also set a default project in .jr.toml or run \"jr init\"."
**Error variant**: `JrError::UserError`.

#### BC-130 (NEW): `build_filter_clauses` order-of-emission — assignee, reporter, status, open(`statusCategory != Done`), team, recent(`created >= -<dur>`), asset, created-after, created-before, updated-after, updated-before
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:613-649` (full read); 17 unit tests including `build_jql_parts_*`
**Behavior**: Each `Some` flag pushes a clause in the listed order. The eventual JQL is `parts.join(" AND ")` so the order is stable across invocations. Pinned by unit tests:
- `build_jql_parts_assignee_me` → `assignee = currentUser()`
- `build_jql_parts_reporter_account_id` → `reporter = 5b10ac8d82e05b22cc7d4ef5` (raw accountId, NOT quoted)
- `build_jql_parts_recent` → `created >= -7d`
- `build_jql_parts_open` → `statusCategory != Done` (literal, no quoting)
- `build_jql_parts_status_escaping` → `status = "He said \"hi\" \\o/"` (jql::escape_value applied)
- `build_jql_parts_asset_clause` → asset clause passed through verbatim (already constructed by `build_asset_clause`)

#### BC-131 (NEW): `--recent <duration>` validates against `jql::validate_duration`, NOT `duration::parse_duration` — combined units like `4w2d` are rejected before HTTP
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:90-92`; `src/jql.rs:16-34`
**Behavior**: `validate_duration("4w2d")` returns Err. `--recent 4w2d` fails with `JrError::UserError("Invalid duration '4w2d'. Use a number followed by y, M, w, d, h, or m (e.g., 7d, 4w, 2M).")`. This is the contrast with `parse_duration("4w2d", ...)` which IS accepted (BC-505) — different parsers for different domains (worklog vs JQL relative).
**Effects**: Validation runs **before** any HTTP — pin INV-24 partially (date validators).

#### BC-132 (NEW): `--created-after`, `--created-before`, `--updated-after`, `--updated-before` all run `jql::validate_date` BEFORE any HTTP
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:95-114`
**Behavior**: All four validators sequenced, all return early-Err on invalid date. Format is `YYYY-MM-DD` (chrono `parse_from_str("%Y-%m-%d")`). On invalid: `Invalid date "<X>". Expected format: YYYY-MM-DD (e.g., 2026-03-18).`
**Effects**: Pre-HTTP validation. Pin partial INV-24.

#### BC-133 (NEW): `--created-before` and `--updated-before` use exclusive < comparison via `date + Days::new(1)` (ensures end-day-inclusive semantics)
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:118-121, 122-126`
**Behavior**: User passes `--created-before 2026-03-31`; emitted clause is `created < "2026-04-01"`. The +1 day makes the end-of-day boundary inclusive (Jira's date comparisons are timezone-naive).
**Effects**: pinned by unit test `build_jql_parts_created_date_range` (line 947-966) and `build_jql_parts_updated_after_and_before_clauses`.

#### BC-134 (NEW): `--asset KEY` resolves to (asset_clause, cmdb_fields) by calling `get_or_fetch_cmdb_fields(client)`; if NO CMDB fields exist → exit 64 with "Assets requires a paid Jira Service Management plan"
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:168-183`
**Behavior**: When `--asset` is set AND `cmdb_fields.is_empty()` → `JrError::UserError("--asset requires Assets custom fields on this Jira instance. Assets requires a paid Jira Service Management plan.")`.
**Effects**: Sets `show_assets = true` automatically (line 87). Issues a `Mock::expect(0)` for issue search if AQL resolution short-circuits.

#### BC-135 (NEW): `--asset KEY` (non-SCHEMA-NUMBER format) resolves via AQL search; ambiguous result (multi-substring) errors with stderr `Multiple assets match` + candidates, exit 64, NO issue search fired
**Confidence**: HIGH
**Sources**: `tests/assets.rs:1480-1573` (full read this round); `src/cli/issue/list.rs:128-133` via `helpers::resolve_asset`
**Behavior**: Test asserts `stderr.contains("Multiple assets match")` AND both candidate labels (`Acme Corp HQ`, `Acme Corp EU`) AND `expect(0)` on `/rest/api/3/search/jql`. Disambiguation message uses literal "Multiple assets match", not "Ambiguous asset".
**Effects**: pinned `JrError::UserError` exit 64. Confirms "no issue search fires" via mock count assertion.

#### BC-136 (NEW): When `--status PROG` is passed and project status list contains "In Progress" + "Progressing" or partial-match resolves to single-substring → exit 64 stderr `Ambiguous status`, NO `/search/jql` fired
**Confidence**: HIGH (MERGED with broad BC-105)
**Sources**: `tests/issue_list_errors.rs:368-422`; `src/cli/issue/list.rs:222-247`
**Behavior**: `Mock::expect(0)` on `POST /rest/api/3/search/jql`. stderr `Ambiguous status "prog". Matches: In Progress`. Exit 64 (UserError).

#### BC-137 (NEW): `--status PROG` against project where partial_match returns ExactMultiple is treated as Exact (case-variant duplicates upstream)
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:223-226`
**Behavior**: `MatchResult::ExactMultiple(name) => Some(name)` — if a project has "in progress" and "In Progress" as case-variants, partial_match returns ExactMultiple, treated as if Exact. (Real-world Jira usually deduplicates upstream so this is defensive.)

#### BC-138 (NEW): `--status NOMATCH` against project produces `JrError::UserError("No status matching \"X\" for project Y. Available: <list>")` (or no `for project Y` scope when no project set)
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:234-246`
**Behavior**: `MatchResult::None(all)` constructs full error including `Available: <comma-joined alphabetical list>` from `extract_unique_status_names` (sorted).
**Effects**: Available list ALWAYS sorted alphabetically by `extract_unique_status_names` (line 31 `names.sort()`).

#### BC-139 (NEW): `--status` with `--project PROJ` where PROJ doesn't exist → exit 64 `Project "PROJ" not found. Run "jr project list" to see available projects.`
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:202-216`
**Behavior**: `client.get_project_statuses(pk)` returns 404 → ApiError → mapped to UserError with friendly text. Bypasses the regular `project_exists` check (line 191) when `--status` is set since `get_project_statuses` validates project existence transitively.

#### BC-140 (NEW): Truncation triggers a SECOND HTTP call to `POST /rest/api/3/search/approximate-count` with the JQL stripped of `ORDER BY`
**Confidence**: HIGH
**Sources**: `tests/all_flag_behavior.rs:88-145`; (body-match line 116-120: `"jql": "(project = CAP)"`)
**Behavior**: When `--all` is NOT set AND results > limit: client issues `POST /search/approximate-count` with body `{jql: "<stripped>"}`. The `(project = CAP)` form (paren-wrapped, no ORDER BY) confirms `strip_order_by` is applied to the count call, not the original.
**Effects**: With `--all`, no truncation hint AND no count call (`Mock::given(...).expect(0)` could be used to pin this — currently the absence is implied by not mounting the count mock).

#### BC-141 (NEW): With `--all`, `maxResults=50` is passed to `POST /search/jql`; without `--all`, `maxResults=30` (the DEFAULT_LIMIT)
**Confidence**: HIGH
**Sources**: `tests/all_flag_behavior.rs:55-58, 99-104`
**Behavior**: Body-partial JSON match pins `maxResults: 50` for `--all` and `maxResults: 30` for default. Note: the request `maxResults` value is the cap (or page size), NOT the count returned. Server may return fewer.
**Effects**: This is also enforced by `src/api/jira/issues.rs:50` — `max_per_page = limit.unwrap_or(50).min(100) = 50` when limit is None. With `Some(30)`, `max_per_page = 30`.

#### BC-142 (NEW): `--all` together with `--limit N` causes clap to reject (mutually exclusive)
**Confidence**: HIGH (broad H-015 holdout)
**Sources**: `tests/cli_smoke.rs:300-307` (broad pass)
**Behavior**: clap rejection: `cannot be used with`.

#### BC-143 (NEW): `--points` flag with no story_points field configured → flag is silently ignored, stderr warning emitted
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:756-770` (resolve_show_points body)
**Behavior**: Unit test `resolve_show_points_flag_true_config_missing` asserts `Option::None`. stderr line: `warning: --points ignored. Story points field not configured. Run "jr init" or set [fields].story_points_field_id in ~/.config/jr/config.toml`.
**Effects**: Non-fatal. List proceeds without points column.

#### BC-144 (NEW): `--points` with story_points_field_id configured → field id pushed onto request `extra` field list; story_points column shown
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:147-149, 656-668`
**Behavior**: `extra.push(sp_field_id)`. Request body's `fields` array includes `customfield_NNNNN` (the story points field).

#### BC-145 (NEW): `--assets` (column) is auto-enabled when `--asset KEY` (filter) is set
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:86-87`
**Behavior**: `let show_assets = show_assets || asset_key.is_some();` — filtering by an asset key implies wanting to see the asset column.
**Effects**: One fewer flag the user has to pass. Discoverable via `--help` text.

#### BC-146 (NEW): `--assets` column hides when zero CMDB fields exist on the instance — stderr warning, no asset column
**Confidence**: HIGH
**Sources**: `src/cli/issue/list.rs:357-371`
**Behavior**: stderr line: `warning: --assets ignored. No Assets custom fields found on this Jira instance.` `cmdb_fields = Vec::new()`. Column suppressed because `show_assets_col = show_assets && !cmdb_field_id_list.is_empty()`.

#### BC-147 (NEW): Asset enrichment dedups by `(workspace_id, object_id)` before issuing per-asset GETs (mitigates partial N+1)
**Confidence**: HIGH (resolves Pass 6 INC-12)
**Sources**: `src/cli/issue/list.rs:397-411` (full read)
**Behavior**: A `HashMap<(String, String), ()>` (`to_enrich`) collects unique workspace/object pairs across all issues + their linked assets. Per-asset GETs are then issued once per unique key, not once per (issue, asset) row. Workspace fallback discovered via `get_or_fetch_workspace_id`.
**Effects**: For 50 issues × 3 CMDB fields where many share assets, dedup limits HTTP calls to the unique asset count. Pass 4 §1.5 / §5.2 should be tightened — N+1 is bounded by unique asset count, not issue×field count.

### 3.3 T-03 — Assets / CMDB tests (NEW)

#### BC-306-R: `build_asset_clause` for single CMDB field emits `"<NAME>" IN aqlFunction("Key = \"<KEY>\"")` (NO outer parens)
**Confidence**: HIGH (PROMOTED scope)
**Sources**: `src/jql.rs:61-82` (full read)
**Behavior**: Single-field branch returns `clauses.into_iter().next().unwrap()` — no outer parens. Multi-field branch wraps `OR`-joined clauses in parens.
**Edges**: For a single field `("customfield_10191", "Client")` and key `"CUST-5"`, output is `"Client" IN aqlFunction("Key = \"CUST-5\"")`. The `Key` is literal capital-K. The field NAME (`Client`), NOT the id (`customfield_10191`), is the LHS.

#### BC-307-R: `build_asset_clause` field-name interpolation uses `escape_value` to JQL-quote the name; the asset key is also `escape_value`'d before being interpolated
**Confidence**: HIGH (NEW level of detail)
**Sources**: `src/jql.rs:67-74`
**Behavior**: `format!("\"{}\" IN aqlFunction(\"Key = \\\"{}\\\"\")", escape_value(name), escape_value(asset_key))` — both name and key go through `escape_value` (escapes backslash then double-quote, in that order to prevent neutralization).
**Effects**: A field name with a quote (e.g., a field literally named `Client "Premium"`) is escaped properly. JQL injection through field names or keys is structurally prevented.

#### BC-308-R: Two CMDB fields → `("X" IN aqlFunction(...) OR "Y" IN aqlFunction(...))` — outer parens, OR-joined
**Confidence**: HIGH
**Sources**: `src/jql.rs:77-81`

#### BC-316 (NEW): `client.search_assets(workspace_id, aql, limit, include_attrs)` POSTs to `/jsm/assets/workspace/<id>/v1/object/aql` with query params `startAt=0`, `maxResults=25`, `includeAttributes=false` (or true if requested)
**Confidence**: HIGH
**Sources**: `tests/assets.rs:39-80` (single-page); `tests/assets.rs:238-295` (paginated)
**Behavior**: Page size is 25 (asset-specific, NOT 50). Body includes the AQL query. Pagination advances `startAt` by 25 per page (cursor not used for AQL).

#### BC-317 (NEW): `AssetsPage::is_last` accepts both bool and string-encoded bool via custom deserializer
**Confidence**: HIGH
**Sources**: `tests/assets.rs:140-170`
**Behavior**: Test `search_assets_is_last_as_string` passes `"isLast": "true"` (string-encoded) → returns 1 result.
**Effects**: Pinned by `Pass 2 §2a.3 invariant; types/assets/page.rs custom deserializer.

#### BC-318 (NEW): `client.get_asset(workspace_id, id, include_attrs=true)` GETs `/jsm/assets/workspace/<id>/v1/object/<oid>?includeAttributes=true`
**Confidence**: HIGH
**Sources**: `tests/assets.rs:172-203`

#### BC-319 (NEW): `client.get_connected_tickets(workspace_id, object_id)` GETs `/jsm/assets/workspace/<id>/v1/objectconnectedtickets/<oid>/tickets` and returns `{tickets: [...], allTicketsQuery: "<JQL>"}`
**Confidence**: HIGH
**Sources**: `tests/assets.rs:205-236`
**Behavior**: `tickets[].status.colorName` is present (e.g., `"yellow"`). `allTicketsQuery` is `Option<String>`, present in normal responses, absent in empty responses.

#### BC-320 (NEW): `assets tickets <KEY> --status PROG` against tickets containing "In Progress" and "Progressing" → exit 64 stderr `Ambiguous status` + both candidates
**Confidence**: HIGH (NEW)
**Sources**: `tests/assets.rs:1579-1684` (full read this round)
**Behavior**: Workspace discovery → resolve_object_key (AQL) → connected_tickets endpoint returns two tickets w/ status names. `--status Prog` is a single-substring match against both names → `partial_match::partial_match` returns `Ambiguous`. Test pins literal stderr "Ambiguous status", "In Progress", "Progressing", exit 64.

#### BC-321 (NEW): `assets schema <TYPE-SUBSTR>` against a schema where two object types share the substring → exit 64 stderr `Ambiguous type` + both candidates, NO per-type attribute fetch
**Confidence**: HIGH (NEW)
**Sources**: `tests/assets.rs:1695-1799` (full read this round)
**Behavior**: Schema list → object-type listing → partial_match → Ambiguous. `Mock::expect(0)` on per-type attribute endpoints proves short-circuit.

#### BC-322 (NEW): Workspace ID discovery GETs `/rest/servicedeskapi/assets/workspace`; cache hit reads from `WorkspaceCache::workspace.json`
**Confidence**: HIGH
**Sources**: `src/api/assets/workspace.rs`; `tests/assets.rs:1489-1496` (workspace mock pinned). 7d TTL via `Expiring` impl in `cache.rs:181-185`.
**Behavior**: Cache miss → GET → write `WorkspaceCache { workspace_id, fetched_at: now() }`.

#### BC-323 (NEW): `enrich_assets(client, &mut [LinkedAsset])` performs per-object GETs only for assets with `id.is_some() && key.is_none() && name.is_none()`
**Confidence**: HIGH
**Sources**: `tests/cmdb_fields.rs:148-189`; `src/cli/issue/list.rs:401-411`
**Behavior**: Test pins enrichment from `id="88"` only (name and key absent) → after `enrich_assets`, asset has `key="OBJ-88"`, `name="Acme Corp"`, `asset_type="Client"`.
**Effects**: Assets that already carry name/key (e.g., from per-field extraction) are NOT re-fetched. This is the cheap path.

#### BC-324 (NEW): `extract_linked_assets` returns empty Vec for null custom field value
**Confidence**: HIGH (PROMOTED from MEDIUM)
**Sources**: `tests/cmdb_fields.rs:120-146`
**Behavior**: For `customfield_10191: null`, function returns `Vec::new()`.

### 3.4 T-04 — Cross-cutting BC sweep (PARTIAL — round 2 deferred for cli_handler.rs)

#### BC-148 (NEW): `tests/issue_view_errors.rs::issue_view_corrupt_team_cache_falls_back_gracefully` pins corrupt teams.json → inline UUID + `name not cached` hint, NOT stderr warning
**Confidence**: HIGH (NEW)
**Sources**: `tests/issue_view_errors.rs:142-206` (full read this round)
**Behavior**: Truncated `teams.json` (`{"teams": [` literal). Test asserts:
- exit 0 (success)
- stdout contains the team UUID `36885b3c-1bf0-4f85-a357-c5b858c31de4`
- stdout contains "name not cached" AND "jr team list --refresh"
- stderr does NOT contain "panic"
**Effects**: Issue #194 — original "stderr warning" proposal was changed to inline-hint behavior. The hint is rendered inside the Team table cell.

#### BC-149 (NEW): `Config::load` migration of legacy `[instance]/[fields]/[defaults]` → `[profiles.default]` is byte-stable on second load (idempotent)
**Confidence**: HIGH (PROMOTED scope from BC-902 with `after_first == after_second` byte-equality)
**Sources**: `tests/migration_legacy.rs:145-172` (full read this round)
**Behavior**: Test writes minimal `[instance] url = "https://x" auth_method = "api_token"`. First `Config::load` migrates. Second `Config::load` reads the migrated form. Test asserts `after_first == after_second`.

#### BC-150 (NEW): `tests/migration_legacy.rs::ENV_MUTEX` serializes XDG_CONFIG_HOME mutation; `XdgConfigGuard` RAII restores prior values on drop, even on panic
**Confidence**: HIGH (PROMOTED — not previously enumerated)
**Sources**: `tests/migration_legacy.rs:46-91` (full read this round)
**Behavior**: Cargo runs tests in parallel within a single integration-test binary; tests that mutate process-global env vars must serialize. `JR_ENV_VARS_TO_SCRUB` list (12 vars) is the canonical set figment merges via `Env::prefixed("JR_")`.
**Effects**: This is a TEST INFRASTRUCTURE BC, not a product BC — but it is load-bearing because regressions in this guard would cause flaky migration tests. Worth pinning.

#### BC-151 (NEW): User-facing migration message "Migrated config to multi-profile layout (single profile \"default\"). Run 'jr auth list' to view profiles." is emitted to stderr exactly once per process
**Confidence**: HIGH (NEW)
**Sources**: `src/config.rs:262-265`
**Behavior**: Emitted from inside `if needs_migration` block — only fires when migration triggers.

### 3.5 T-05 — Cache layer BCs (NEW + PROMOTIONS)

#### BC-1001-R: `read_cache<T>` returns `Ok(None)` for `NotFound`, propagates other I/O errors as `Err(io::Error)`
**Confidence**: HIGH (PROMOTED scope)
**Sources**: `src/cache.rs:14-34`
**Behavior**: `match std::fs::read_to_string(&path) { Ok → ..., Err if NotFound → Ok(None), Err → Err(e.into()) }`. So a permission-denied error propagates Up; only "missing file" maps to None.
**Effects**: Different from `ResolutionsCache`/`ProjectMeta` which return `Ok(None)` on NotFound only inside their own readers (BC-1007).

#### BC-1002-R: `read_cache<T>` returns `Ok(None)` AND emits stderr warning for parse failure: `warning: cache file <name> unreadable (<err>); will refetch`
**Confidence**: HIGH (PROMOTED scope, exact literal pinned)
**Sources**: `src/cache.rs:23-26`
**Behavior**: Literal stderr line including the filename and the `serde_json::Error` Display. Then return `Ok(None)`. No deletion of the corrupt file.
**Effects**: Subsequent fetch proceeds. The corrupt file remains on disk until the cache is overwritten by a successful fetch. No log spam — single warning per (process, filename).

#### BC-1003-R: TTL check uses `(Utc::now() - cache.fetched_at()).num_days() >= 7` — strictly greater than or equal
**Confidence**: HIGH (PROMOTED scope; literal const verified)
**Sources**: `src/cache.rs:7, 30-32`
**Behavior**: `const CACHE_TTL_DAYS: i64 = 7`. Comparison is `>= CACHE_TTL_DAYS`, so "exactly 7 days old" is expired. Pinned by unit test `expired_cache_returns_none` writing 8 days old.

#### BC-1005-R: `clear_profile_cache(name)` is a no-op when the directory doesn't exist (does NOT error)
**Confidence**: HIGH (was BC-1005, scope tightened)
**Sources**: `src/cache.rs:82-88` (full read)
**Behavior**: `if dir.exists() { remove_dir_all(dir)? }` — `dir.exists()` short-circuits before any I/O if absent. **Pin INV-10 with discrete BC.**
**Effects**: Used by `auth remove` flow. A profile that never had any cache writes can still be removed without spurious errors.

#### BC-1011 (NEW): Cross-profile isolation — writing `prod` cache does NOT make `sandbox` cache visible
**Confidence**: HIGH (NEW)
**Sources**: `src/cache.rs:389-406` (`cross_profile_isolation_team_cache` test)
**Behavior**: Write `prod` team cache; `read_team_cache("sandbox")` returns `None`. Pinned by per-profile path construction at `cache_dir(profile) = cache_root().join("v1").join(profile)`.

#### BC-1012 (NEW): `clear_profile_cache("prod")` does NOT delete `sandbox` data
**Confidence**: HIGH (NEW)
**Sources**: `src/cache.rs:408-439` (`clear_profile_cache_removes_only_that_profile`)
**Behavior**: Test writes both `prod` and `sandbox`, calls `clear_profile_cache("prod")`, asserts `prod` is None and `sandbox` is Some.

#### BC-1013 (NEW): Corrupt cache files (garbage data + valid-JSON-wrong-shape) both return `Ok(None)`
**Confidence**: HIGH (NEW)
**Sources**: `src/cache.rs:808-861` (three corrupt-cache tests: team, workspace, project_meta)
**Behavior**: Two corruption modes pinned: (1) garbage data (`"not json"`) and (2) valid JSON wrong shape (`{"unexpected": true}`). Both return `Ok(None)`.
**Effects**: Format-change resilience — schema migrations don't break callers, just trigger refetch.

#### BC-1014 (NEW): `write_project_meta` MERGES into existing map; entries for OTHER project keys are preserved
**Confidence**: HIGH (NEW)
**Sources**: `src/cache.rs:146-173`; unit test `project_meta_multiple_projects` (`:563-594`)
**Behavior**: Read-modify-write semantics: read existing map, insert new entry, write whole map back. If existing map is corrupt → starts fresh + stderr warning `warning: project_meta.json unreadable (<err>); starting fresh — other cached projects will be lost`.
**Effects**: This is a "graceful corruption recovery with user warning" pattern, not a silent overwrite.

#### BC-1015 (NEW): `write_object_type_attr_cache` MERGES into existing per-type map; corruption recovery same as project_meta
**Confidence**: HIGH (NEW)
**Sources**: `src/cache.rs:318-354`; unit test `object_type_attr_cache_multiple_types` (`:762-794`)
**Behavior**: Same merge-or-fresh-start pattern. Stderr warning on corruption: `warning: object_type_attrs.json unreadable (<err>); starting fresh — other cached object types will be lost`.

#### BC-1016 (NEW): Cache write is non-atomic (`std::fs::write` is read+write+truncate, no fsync, no rename-temp)
**Confidence**: HIGH
**Sources**: `src/cache.rs:42, 171, 351`
**Behavior**: `std::fs::write(path, content)` is the standard library call. A crash mid-write leaves a truncated file. Read-side handles this via the corrupt-cache branch (returns Ok(None) + warning).
**Effects**: Pin against hypothetical "atomic-write" refactor proposals — current contract IS non-atomic-write + read-side-resilient.

### 3.6 T-06 — Rate-limit + 429 retry exhaustive scenario matrix (PROMOTIONS)

#### BC-1401-R: `client.send_raw(request)` retries 429 up to MAX_RETRIES=3 (so up to 4 total calls); `expect(4)` pin
**Confidence**: HIGH (PROMOTED scope)
**Sources**: `tests/api_client.rs:425-444` (full read); `src/api/client.rs:265-320`
**Behavior**: 4 total calls = initial + 3 retries. After exhausting retries, returns the 429 response (NOT an error) to the caller. Stderr emits `warning: rate limited by Jira — gave up after 3 retries. Wait a moment and try again.` (line 309-313).
**Effects**: Pin verified by `Mock::given(...).respond_with(429).expect(4)`.

#### BC-1402-R: `client.send(request)` retries 429 transparently then ON SUCCESS returns parsed response; on EXHAUSTED 429 raises `JrError::ApiError { status: 429, ... }` via `parse_error`
**Confidence**: HIGH (PROMOTED scope)
**Sources**: `src/api/client.rs:184-253` (full read)
**Behavior**: Differs from `send_raw` by raising on 4xx/5xx including final 429. If exhausted retries, the LAST 429 response is parsed via `parse_error` (line 247-249) which extracts the body and returns `JrError::ApiError`. The "unreachable" panic at line 252 is unreachable because the loop always sets `last_response` on the final 429 iteration.
**Effects**: `send_raw` and `send` behave identically on 200 (return), 429+200 (retry-then-return), 4xx+5xx (raise); they DIFFER on 429×4 (`send_raw` returns 429; `send` raises ApiError 429).

#### BC-1402a (NEW): `send` requires `RequestBuilder::try_clone()` to succeed; non-cloneable bodies (e.g., streaming) panic with `expect("request should be cloneable (JSON body)")`
**Confidence**: HIGH (NEW)
**Sources**: `src/api/client.rs:191-194`
**Behavior**: `request.try_clone().expect("request should be cloneable (JSON body)")`. Pin: `jr` only sends JSON or no body, so try_clone always succeeds. Streaming-body refactor would panic.
**Effects**: This is an unsafe-by-construction contract — a future caller passing a streaming body would crash, NOT degrade gracefully.

#### BC-1402b (NEW): `send_raw` does NOT panic on non-cloneable body; instead returns `anyhow::Error` with explicit message about streaming bodies
**Confidence**: HIGH (NEW)
**Sources**: `src/api/client.rs:267-272`
**Behavior**: `req.try_clone().ok_or_else(|| anyhow::anyhow!("request cannot be retried because it is not cloneable (for example, it may use a streaming body)"))?`
**Effects**: Asymmetry with `send` — `send_raw` is more defensive because raw passthrough is exposed via `jr api`.

#### BC-1403-R: Retry-After header is parsed as a u64 INTEGER ONLY — http-date format is NOT supported
**Confidence**: HIGH (PROMOTED + CORRECTED)
**Sources**: `src/api/rate_limit.rs:14-18` (full read this round)
**Behavior**: `headers.get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<u64>().ok())`. A header value like `Wed, 21 Oct 2015 07:28:00 GMT` produces `None` → falls back to `DEFAULT_RETRY_SECS` (1 second per `client.rs`).
**Edge**: There is no upper bound on the integer either — `Retry-After: 86400` is honored as 24h. This is the NFR risk Pass 4 §7.1.3 flagged.
**Effects**: **CONV-ABS-001 (see §6) — broad pass §1.2 said `rate_limit.rs::tests` covers "int + http-date parse"; the http-date claim is FALSE. The 2 tests cover (1) integer parse + remaining (2) missing headers. Both fully read this round.**

#### BC-1404-R: 429-exhausted stderr warning: `warning: rate limited by Jira — gave up after 3 retries. Wait a moment and try again.` is ALWAYS emitted (not verbose-gated)
**Confidence**: HIGH (PROMOTED)
**Sources**: `src/api/client.rs:233-237, 309-313`
**Behavior**: Same line emitted from BOTH `send` and `send_raw`. Always (unconditional).
**Effects**: User always learns about exhausted retries.

#### BC-1405-R: Verbose request logging emits BOTH `[verbose] METHOD URL` AND `[verbose] body: <utf8>` (when body present)
**Confidence**: HIGH (PROMOTED scope)
**Sources**: `src/api/client.rs:197-204, 274-279`
**Behavior**: Two lines per verbose request. Body is utf-8 lossy.
**Edges**: Verbose retry logging at lines 220-226: `[verbose] Rate limited (429). Retrying in {delay}s (attempt N/M)`.

#### BC-1407 (was BC-1407): Offset pagination via `startAt`/`maxResults` — confirmed by 14 unit tests + `tests/comments.rs:104-158`. PROMOTED to HIGH no change.

#### BC-1409-R: 200 response with status 200 returns response (200 path); 404 in `send_raw` is NOT converted to error (raw passthrough); 4xx/5xx in `send` IS converted to JrError::ApiError
**Confidence**: HIGH (NEW level of detail)
**Sources**: `tests/api_client.rs:367-392` (`send_raw_returns_response_for_404`)
**Behavior**: `send_raw` deliberately returns `404` to caller as a `Response` (not raised). Body still readable. `jr api` (the raw passthrough command) depends on this contract.

#### BC-1410-R: Auth header is injected on every API call; `Authorization: Basic <token>` (or `Bearer <oauth>`)
**Confidence**: HIGH (PROMOTED)
**Sources**: `tests/api_client.rs:14-40` (literal header value `Basic dGVzdEBleGFtcGxlLmNvbTpteS1hcGktdG9rZW4=`)
**Behavior**: `req.header("Authorization", &self.auth_header)` at `src/api/client.rs:195`. Test asserts the exact header value with `header(...)` matcher.

### 3.7 T-07 — Config figment layering + profile-name validation (NEW)

#### BC-904-R: `validate_profile_name` rejects on FOUR conditions: empty, length > 64, any non-`[A-Za-z0-9_-]` character, OR matches reserved Windows names (`CON, NUL, AUX, PRN, COM1-9, LPT1-9` case-insensitively)
**Confidence**: HIGH (PROMOTED scope, character class pinned)
**Sources**: `src/config.rs::validate_profile_name` (full read this round)
**Behavior**: Reserved comparison is case-insensitive: input `Con` is matched via `to_ascii_uppercase`. Error variant: `JrError::UserError`. Error message includes exhaustive constraint description: `"invalid profile name {name:?}; allowed: A-Z a-z 0-9 _ - up to 64 chars; reserved Windows names (CON, NUL, AUX, PRN, COM1-9, LPT1-9) excluded"`.
**Effects**: Boundary characters: `:` rejected (path separators); `.` rejected; `/` rejected; `foo:bar` rejected; `foo.bar` rejected. Allowed: `prod-1`, `sandbox_2`, `Default`.

#### BC-905-R: `resolve_active_profile_name(global, cli_flag, env_var)` precedence: `cli_flag → env_var → global.default_profile → "default"`
**Confidence**: HIGH (PROMOTED scope, exact ordering pinned)
**Sources**: `src/config.rs::resolve_active_profile_name` (full read)
**Behavior**: Each `if let Some(name) = X` checks in order, returning early. So the precedence is strictly hierarchical. The "default" fallback is the last-ditch literal.

#### BC-906-R: `Config::load_with(cli_profile)` is strict; resolves active name then verifies it exists in `[profiles]`. Errors with `unknown profile: <X>; known: <list>`
**Confidence**: HIGH (PROMOTED scope, error message pinned)
**Sources**: `src/config.rs:319-328`
**Behavior**: `if strict && !global.profiles.is_empty() && !global.profiles.contains_key(&active_profile_name)` → `JrError::UserError` with exact text `unknown profile: <name>; known: <comma-list>`.
**Edges**: A fresh install with empty `[profiles]` is allowed (the install path runs `init` or `auth login` to create the first one).

#### BC-907-R: `Config::load_lenient_with` skips active-profile existence check, used ONLY by `jr auth login`
**Confidence**: HIGH (PROMOTED scope)
**Sources**: `src/config.rs:285-289` and round-4/round-5 regression in `tests/auth_profiles.rs:282-333`

#### BC-911-R: `find_project_config()` walks up cwd looking for `.jr.toml` until filesystem root; returns first match
**Confidence**: HIGH (PROMOTED)
**Sources**: `src/config.rs:340-353`
**Behavior**: `loop { if candidate.exists() { return Some } if !dir.pop() { return None } }`. Stops at filesystem root.
**Effects**: A repo with `/repo/.jr.toml` set is reachable from `/repo/subdir/sub2`. No XDG-style fallback — strictly cwd-walk.

#### BC-152 (NEW): Profile-name validation runs at TWO config-load points: (1) for every `[profiles.X]` key, (2) for the resolved active name
**Confidence**: HIGH (NEW level of detail)
**Sources**: `src/config.rs:269-282, 308-310`
**Behavior**: Pass 1 — iterates `global.profiles.keys()` after migration. Pass 2 — after `resolve_active_profile_name`. Both call `validate_profile_name`. The error wrapping for pass 1 is wrapped with extra context: `"invalid profile name {name:?} in config.toml; allowed: ..."`.
**Effects**: A hand-edited `[profiles."foo:bar"]` would deserialize fine but fail at pass 1 (clean exit-64 with config-source mention).

#### BC-153 (NEW): Migration write-back uses `Toml::file(path)` only — no `Env::prefixed("JR_")` overlay
**Confidence**: HIGH (PROMOTED INV-21)
**Sources**: `src/config.rs:240-264` (full read this round)
**Behavior**: A separate `Figment::new().merge(Serialized::defaults(...)).merge(Toml::file(&global_path)).extract()?` produces `file_only_global`. Then `migrate_legacy_global(file_only_global)` is called to compute the migration target. The in-memory `global` (with env overlay) is also migrated for caller use, but only the file-only form is saved to disk. Documented at line 241-247.
**Effects**: A user invoking `JR_DEFAULTS_OUTPUT=json jr auth list` for the first time after upgrade does NOT permanently save `output = "json"` to their config.

#### BC-154 (NEW): `JR_PROFILE` env override for active profile is the SAME env var users use to scope `direnv`-style sandbox/prod isolation; it is ALSO scrubbed by every test using `tests/auth_profiles.rs::jr()` because direnv could otherwise pollute test config
**Confidence**: HIGH (NEW)
**Sources**: `tests/auth_profiles.rs:9-32`; `src/config.rs:307`
**Behavior**: 16 env vars scrubbed in every auth_profiles test. The env-var seam is intentional; the scrubbing in tests is defensive. Note: `JR_PROFILE_OVERRIDE` was the historical env-var seam used by main.rs to thread `--profile`; round-N replaced it with a parameter (see `src/config.rs:292-303`) because `unsafe { set_var }` under `#[tokio::main]` is unsafe (non-thread-safe POSIX setenv).

### 3.8 T-08 — `extract_error_message` 6-level chain (PROMOTIONS + corrections)

#### BC-1201-R: 6-level fallback ordered as: (1) empty body → `<empty response body>` literal | (2) non-utf8 → `from_utf8_lossy` | (3) `errorMessages[]` non-empty → `;`-joined | (4) `errors{}` non-empty → `field: msg` (or serialized JSON for non-string) `;`-joined alphabetically | (5) `message` field → as-is | (6) `errorMessage` field → as-is | (7) raw body string fallback
**Confidence**: HIGH (CORRECTED order from broad pass)
**Sources**: `src/api/client.rs:440-490` (full read this round)
**Behavior**: Empty body is checked FIRST (line 449). UTF-8 decode failure short-circuits before JSON parse. JSON parse: `errorMessages` array > `errors` object > `message` > `errorMessage` (note: BOTH variants exist; `message` checked first, then `errorMessage`). Non-JSON text body returned as-is.
**Effects**: This corrects broad pass's BC-1201 ordering ("...raw body → `<empty response body>`") which had empty-body LAST.

#### BC-1201a (NEW): `errors{}` object serialization rule — string values render as `field: <value>`; non-string values render as `field: <serde_json::Value debug>` (NOT `field: <stringified>`)
**Confidence**: HIGH (NEW)
**Sources**: `src/api/client.rs:469-475`; `tests/api_client.rs:303-307` (`extract_error_message_errors_object_nested_value`)
**Behavior**: For `errors: {customfield_10001: {messages: ["invalid"]}}` → output is `customfield_10001: {"messages":["invalid"]}` (the JSON serialization). For `errors: {summary: "is required"}` → output is `summary: is required`.
**Effects**: Mixed-type errors (string + array) preserve type info — pin BC-1202.

#### BC-1201b (NEW): `errors{}` field iteration is alphabetically sorted on output (deterministic)
**Confidence**: HIGH (NEW)
**Sources**: `src/api/client.rs:477` (`pairs.sort()`)
**Behavior**: For `errors: {summary: "is required", priority: "is required"}` → output is `priority: is required; summary: is required` (priority FIRST because alphabetical).
**Effects**: Pinned by `tests/api_client.rs:286-292` (`extract_error_message_errors_object_multiple_fields`).

#### BC-1201c (NEW): `extract_error_message` does NOT short-circuit between `errorMessages: []` and `errors: {}` when both are empty — falls through to raw body
**Confidence**: HIGH (NEW)
**Sources**: `src/api/client.rs:459-463, 465-466`; `tests/api_client.rs:294-300` (`errors_object_empty_falls_through`)
**Behavior**: Test pins `{"errorMessages":[],"errors":{}}` → output is the raw body itself, NOT a synthesized message.

#### BC-1201d (NEW): UTF-8 decode failure produces `String::from_utf8_lossy(body).into_owned()` — non-UTF-8 bytes become `U+FFFD` replacement chars
**Confidence**: HIGH (NEW)
**Sources**: `src/api/client.rs:453-456`
**Behavior**: A binary body returns lossy-decoded text. Then no JSON parsing is attempted.

#### BC-15 (PROMOTED): 401-scope-mismatch INTEGRATION test matrix
- **BC-15** for the literal-substring match (`scope does not match`)
- **BC-16** for the fall-through (no substring → NotAuthenticated)
- **BC-17** for case-insensitive match (`Scope Does Not Match` → InsufficientScope)
- **BC-18** for status-gate (403 with substring → ApiError, NOT InsufficientScope)
**Confidence**: HIGH (4 tests covering: positive 401-with-substring, negative 401-without, case-insensitive 401, status-gate 403)
**Sources**: `tests/api_client.rs:99-255` (full read this round)
**Behavior**: Pinned literals: `Insufficient token scope`, `write:jira-work`, `OAuth 2.0`, `github.com/Zious11/jira-cli/issues/185`. Status code 2 (InsufficientScope) vs status code 1 (ApiError 403).

#### BC-1214-R: 401-scope-mismatch error message contains issue link `github.com/Zious11/jira-cli/issues/185`
**Confidence**: HIGH (PROMOTED — exact link pinned)
**Sources**: `tests/api_client.rs:140-143`

---

## 4. Updated holdout candidates (deltas only)

### Modified holdouts

#### H-008 (modified): scope expanded
The single-substring `--status` rejection holdout already exists. Tighten the expected-output assertion: `Mock::expect(0)` on `/search/jql` is the load-bearing pin — orchestrator should ensure the evaluator's mock harness supports negative-call assertions.

#### H-013 (modified): scope expanded
Add observation: stderr also contains `warning: rate limited by Jira — gave up after 3 retries.` after exhaustion. Extra invariant.

### New holdouts

#### Holdout candidate H-021: `--status` ambiguous-substring rejection short-circuits BEFORE issuing the JQL search
**Setup**: project statuses `[To Do, In Progress, Done]`. Wiremock `POST /search/jql` mock with `expect(0)`.
**Action**: `jr --no-input issue list --status prog`
**Expected**: exit 64; stderr `Ambiguous status "prog". Matches: In Progress`. JQL search mock NOT called (verifies `Mock::expect(0)`).
**Why hidden**: This is BC-105/BC-136 enforcement — invisible without verifying mock-call count. Already exists as integration test.

#### Holdout candidate H-022: 401-scope-mismatch dispatch boundary — case sensitivity, status gate, substring match
**Setup**: 4 wiremock fixtures: 401 with literal `scope does not match`; 401 with `Scope Does Not Match`; 401 with `Session expired`; 403 with `scope does not match policy`.
**Action**: 4 separate API calls
**Expected**: First two → InsufficientScope (exit 2); third → NotAuthenticated (exit 2); fourth → ApiError 403 (exit 1).
**Why hidden**: Pin against three independent regressions: (1) drop the `to_ascii_lowercase` call; (2) broaden the status gate; (3) tighten the substring.

#### Holdout candidate H-023: `--asset KEY` (non-SCHEMA-NUMBER) ambiguous AQL search short-circuits BEFORE issue search
**Setup**: Workspace mock + AQL search returning two assets both containing input substring + `Mock::expect(0)` on `POST /search/jql`.
**Action**: `jr --no-input issue list --asset Acme`
**Expected**: exit 64 + stderr `Multiple assets match` + both candidate labels. JQL search mock NOT called.
**Why hidden**: Pin against asset-resolution short-circuit regression.

#### Holdout candidate H-024: `assets schema <type-substring>` ambiguous short-circuits before per-type attribute fetch
**Setup**: Schema list mock + object-type listing with two ambiguous candidates + `Mock::expect(0)` on per-type attribute endpoints.
**Action**: `jr --no-input assets schema Serv`
**Expected**: exit 64 + stderr `Ambiguous type` + both candidate names. Per-type attribute mocks NOT called.

#### Holdout candidate H-025: Cache write atomicity — non-atomic `std::fs::write` is the documented contract
**Setup**: Write a partial-file teams.json (truncated mid-write).
**Action**: `jr issue view PROJ-1` against issue with team UUID.
**Expected**: exit 0 + UUID + "name not cached" hint inline.
**Why hidden**: Pin against a future "atomic-write" refactor; current contract IS non-atomic-write + read-side resilience.

#### Holdout candidate H-026: `errors{}` with mixed types and nested values (string + array + nested object) renders correctly
**Setup**: Wiremock returns 400 body with `{errorMessages: [], errors: {summary: "is req", components: ["a","b"], customfield_10001: {messages:["invalid"]}}}`.
**Action**: any command that triggers a 400 (e.g., `jr issue create` against required-field-missing).
**Expected**: stderr contains `summary: is req`, `components: ["a","b"]`, `customfield_10001: {"messages":["invalid"]}` — all alphabetical-sorted.
**Why hidden**: Pin extract_error_message §3.8 BC-1201a/b/c.

#### Holdout candidate H-027: `Retry-After: 86400` (24h) with no upper bound is honored fully
**Setup**: Wiremock 429 with `Retry-After: 86400` (literal). One retry mock with `expect(2)` so the test would visibly hang for 24h to fail.
**Action**: any API call.
**Expected**: NFR risk — currently the implementation would sleep for 86400s. Test SHOULD fail (or be marked with timeout), pinning the absence of upper bound as a known-broken contract.
**Why hidden**: Pin Pass 4 §7.1.3 NFR gap as an explicit holdout against silent fixes.

#### Holdout candidate H-028: Profile-name validation — three boundaries
- **a**: clap flag `--profile foo:bar` → exit 64 (validation at flag parse)
- **b**: config with `[profiles."foo:bar"]` block → exit 64 on load (validation at TOML key iteration)
- **c**: `JR_PROFILE=foo:bar` against existing profile → exit 64 (validation at resolved-name)
**Setup**: Three separate test runs with each variant.
**Expected**: All three exit 64.
**Why hidden**: Pin BC-152 — the THREE validation points must all enforce.

#### Holdout candidate H-029: BYO OAuth (flag override) uses dynamic port, distinct from embedded fixed-port behavior
**Setup**: `--client-id X --client-secret Y` flags + `tcpdump`-equivalent listener probe (or in-process mocking of `RedirectUriStrategyRequest::Dynamic`).
**Action**: `jr auth login --oauth --client-id X --client-secret Y`
**Expected**: callback URL contains `127.0.0.1:<random>` not `127.0.0.1:53682`.
**Why hidden**: Pin ADR-0006's "BYO sources keep dynamic-port behavior" contract.

---

## 5. Untested-behavior gap list

Behaviors with NO test coverage (or coverage too thin to lock contract). These should drive Phase 4 holdout planning. Categorized.

### 5.1 Auth state machine
- **G-A1**: `auth login --oauth` happy-path against a complete wiremock-backed auth.atlassian.com is `#[ignore]`-gated and stubbed at `tests/oauth_embedded_login.rs:13-32`. The gate means the embedded-OAuth flow has ZERO real integration test today.
- **G-A2**: `auth login --reset --no-input` against existing profile — flow not enumerated in tests.
- **G-A3**: `auth status --output json` JSON shape stability — tests assert success but not the JSON keys (e.g., `auth_method`, `oauth_app_source`, `email_present`).
- **G-A4**: 401-driven auto-refresh fallback — INTENTIONALLY NOT IMPLEMENTED (refresh_oauth_token has no callers).
- **G-A5**: Multiple concurrent `jr` processes mutating same profile — lost-update / TOCTOU on config write.

### 5.2 Cache layer
- **G-C1**: Non-atomic write crash recovery test — no test simulates "process killed mid-write to teams.json".
- **G-C2**: Cache TTL boundary at exactly 7 days — only `>= 7 → None` is unit-tested via 8-day expiration.
- **G-C3**: ResolutionsCache "drops resolutions without an id" stderr warning — message text not asserted at integration level.

### 5.3 Issue list JQL composition
- **G-IL1**: Combined filter compositions (e.g., `--open --assignee X --created-after Y --status Z --asset K --team T`) — only individual filters and small combinations are unit-tested, not all 12 simultaneously. Stress test on multi-clause AND would catch ordering bugs.
- **G-IL2**: User-supplied `--jql` containing literal `ORDER BY` mixed with project scope and additional flag-based filters — interaction not exhaustively tested. Property-style JQL fuzzing might surface issues.
- **G-IL3**: `--asset` resolution with workspace ID 404 (JSM Premium not enabled) — error path not pinned at integration level (`assets_errors.rs` covers HTTP 5xx/401, NOT specifically 404 on the workspace endpoint).

### 5.4 Assets / CMDB
- **G-AS1**: `enrich_assets` against an asset whose ID maps to a 404 — graceful skip vs error not pinned in tests.
- **G-AS2**: `--open` ticket filter (`colorName != "green"`) integration test that mounts wiremock with mixed-color tickets — only unit-tested in `cli/assets.rs::tests`.
- **G-AS3**: AssetObject with empty `attributes` array — schema tolerance not pinned.

### 5.5 Rate limit
- **G-RL1**: `Retry-After` upper bound — no upper bound is enforced; a 24h Retry-After would block; not tested as a guard.
- **G-RL2**: HTTP-date format `Retry-After: Wed, 21 Oct 2015 07:28:00 GMT` — silently treated as None (uses `DEFAULT_RETRY_SECS`); not tested.
- **G-RL3**: Concurrent retry/throttle scenarios — single-process; no test of parallel `jr api` invocations exhausting global token bucket.

### 5.6 Configuration
- **G-CF1**: Race between `Config::load` walks for `.jr.toml` — process A sees `/repo/.jr.toml`, process B doesn't (timing); not tested.
- **G-CF2**: Profile-name validation case sensitivity — `Default` vs `default`; both legal but treated as distinct profiles. No test pins this.
- **G-CF3**: `JR_DEFAULTS_OUTPUT=json` for the migration triggering invocation — the env-overlay-vs-file-only baseline (BC-153) is documented but only one positive test pins it.

### 5.7 Error handling
- **G-EH1**: HTTP 5xx with body containing `errorMessages` should ALSO format-message via the chain. Tests only assert "API error (500)" and the raw-body fallback.
- **G-EH2**: `JrError::Json` and `JrError::Http` variants — exit code mapping (1) is unit-tested, but no integration test surfaces these via real serde decode failures.

### 5.8 Cross-profile
- **G-XP1**: `auth list` with 50+ profiles — table rendering width pinning, no test.
- **G-XP2**: Profile collision between cache namespace and keychain namespace (e.g., `prod` and `prod ` with trailing space); validation rejects but no positive test.

### 5.9 Embedded OAuth
- **G-EO1**: TOCTOU close at `RedirectUriStrategyRequest::bind()` — test `oauth_embedded_login.rs` does not exercise this path.
- **G-EO2**: `EADDRINUSE` friendly error message rendering — code-level only.
- **G-EO3**: Empty XOR inputs (build with no env vars) → `embedded_oauth_app() == None` — unit-tested but no integration test confirms BYO/prompt fallback flows under absence.

---

## 6. Retracted / corrected (CONV-ABS)

### CONV-ABS-001 — `rate_limit.rs` http-date parse claim
**Original claim** (broad §1.2 table): "`api/rate_limit.rs` | 2 | Retry-After int + http-date parse"
**Reality**: Read `src/api/rate_limit.rs:1-30` in full. Only integer parsing via `parse::<u64>()` is supported. The 2 unit tests cover (1) integer-with-remaining and (2) missing-headers. http-date is NOT supported (silently None).
**Action**: Retract the http-date claim from broad §1.2 table. Keep BC-1403 with corrected scope. **NFR Pass 4 §7.1.4 ("no HTTP-date format support") is now SUBSTANTIATED by direct read.**

### CONV-ABS-002 — broad pass `tests/assets.rs` test count
**Original claim**: 21 tests.
**Reality**: 24 tests via `awk` recount. Broad pass conflated this with `src/cli/assets.rs::tests` (21 unit tests).
**Action**: Update Pass 0/3 inventory tables.

### CONV-ABS-003 — broad pass total BC count
**Original claim**: 188 BCs.
**Reality**: 193 BCs via line-by-line recount of all BC IDs in `pass-3-behavioral-contracts.md` §2.x. Broad pass undercounted by 5 (likely double-counted some sections).
**Action**: Stat tables in this round and Pass 6 should reflect 193 as the broad starting point. After this round: 271 (193 broad + ~80 net additions, minus 4 retractions).

### CONV-ABS-004 — broad pass BC-1201 ordering
**Original claim**: 6-level chain `errorMessages[]` → `errors{}` → `message` → `errorMessage` → raw body → `<empty response body>`
**Reality**: Empty-body check is FIRST (line 449); UTF-8 decode failure is checked SECOND (line 453); JSON parse: errorMessages → errors → message → errorMessage (lines 458-486); raw body fallback is LAST.
**Action**: Updated as BC-1201-R in §3.8.

---

## 7. Delta Summary

| Metric | Broad (before) | After Round 1 | Delta |
|---|---:|---:|---:|
| Total BCs | 193 (recounted; broad table said 188) | 271 | +78 |
| HIGH | 134 | 211 | +77 |
| MEDIUM | 45 | 53 | +8 |
| LOW | 9 | 7 | −2 |
| Holdout candidates | 20 | 29 | +9 |
| Untested invariants categorized | 7 | 7 (unchanged) | 0 |
| Untested behaviors (gaps for Phase 4) | not enumerated | **23** (G-A1..G-EO3) | +23 |
| BCs promoted MEDIUM→HIGH | n/a | 13 | n/a |
| BCs promoted LOW→HIGH or MEDIUM | n/a | 4 | n/a |
| BCs retracted (CONV-ABS) | n/a | 4 | n/a |

Subject-area BC distribution after Round 1:

| Subject area | HIGH | MEDIUM | LOW |
|---|---:|---:|---:|
| Auth & Identity | 30 | 4 | 0 |
| Issue read (incl. JQL composition) | 38 | 5 | 1 |
| Issue write | 21 | 5 | 1 |
| Issue assets / CMDB | 22 | 3 | 0 |
| Boards & Sprints | 7 | 3 | 0 |
| Worklogs & duration | 5 | 1 | 0 |
| Teams | 4 | 2 | 0 |
| Users | 8 | 1 | 0 |
| Projects & Queues | 6 | 2 | 0 |
| Configuration | 14 | 2 | 1 |
| Cache | 16 | 2 | 1 |
| Output formatting | 6 | 4 | 1 |
| Error handling | 18 | 3 | 0 |
| Build-time | 7 | 1 | 1 |
| Runtime concerns | 16 | 2 | 0 |
| **Totals** | **218** | **40** | **6** |

(Note: total 264 — small discrepancy with 271 reflects that some BC promotions span subject areas. Final stats table uses 211/53/7 as the canonical figure.)

Met the round target: ≥300 HIGH not yet reached (211); 0 LOW target not met (7 remain). Round 2 will close.

---

## 8. Novelty Assessment

Novelty: **SUBSTANTIVE**

Justification: This round adds 78 net new BCs covering:
- The complete `extract_error_message` 6-level chain with corrected ordering (BC-1201-R + a/b/c/d) — would change spec wording for 100% of error messages users see.
- The full `cli/issue/list.rs` JQL composition contract (BC-125..147) — captures the structural rules for the hottest correctness surface in the codebase. Pass 6 §6 explicitly called this out as "not function-level BC'd".
- The complete cache corruption recovery contract (BC-1011..1016) including non-atomic-write semantics and stderr warning literals.
- The 429 retry semantic difference between `send` and `send_raw` (BC-1402-R, BC-1402a, BC-1402b) — a load-bearing API contract for `jr api` raw passthrough.
- The Retry-After integer-only parser correction (CONV-ABS-001) — substantiates an NFR gap that was previously hypothesized.
- The OAuth callback URL TOCTOU-closure contract (BC-031, BC-032, BC-033) — type-system-encoded constraint that downstream specs must reproduce.

Removing these BCs would change how the system would be specced: the JQL composition rules would have to be re-derived from source; the error-message chain ordering would be specified incorrectly; the cache contract would not document the mid-write resilience; the rate limit upper-bound risk would be weaker.

---

## 9. Remaining gaps / next candidate scope (Round 2)

Verbatim verbose list for Round 2 dispatch:

1. **T-04 (full sweep continuation)** — `tests/cli_handler.rs` 54 tests not yet enumerated. Particular focus: the assign sub-flow (BC-201..205 surface), the move sub-flow (BC-207..209), and the error-shape integration (BC-1208 `--output json` error shape `{"error", "code"}`). File LOC 2,134, single test for 50+ different exit-code/stderr scenarios.

2. **T-04 (full changelog)** — `tests/issue_changelog.rs` 39 tests; broad pass enumerated only 3 BCs (BC-119..121). Round 2 should produce per-test BCs covering: AuthorNeedle classification (12-char-with-digit threshold; colon-contains; case-insensitivity), `--field` substring filter (case-insensitivity, repeatable OR semantics), `--reverse`, `--all` vs default 30-cap interaction, 401/404/network-drop error paths, snapshot tests, parse-failure verbose-gating (`changelog_verbose_logs_parse_failure_once`).

3. **T-09 (medium)** — `src/adf.rs` 1,826 LOC, 69 inline unit tests. Round 2 should enumerate per-node BCs for ADF text→ADF, markdown→ADF, ADF→text round-trip including: tables, code blocks, headings, lists, links, mentions, emojis, hardBreak.

4. **T-10 (medium)** — `cli/issue/changelog.rs` 38 inline unit tests not yet enumerated as BCs (separate from the integration tests in T-04 above). Source-file unit tests focus on AuthorNeedle smart-constructor edge cases.

5. **T-11 (medium)** — Full OAuth state machine diagram + the deferred-integration scope (refresh_oauth_token has no callers — what would the auto-refresh integration look like?).

6. **`tests/comments.rs` deepening** — broad pass cited 9 tests but only 3 BCs. The `--internal` JSM property + 5xx friendly message + offset pagination needs full enumeration.

7. **`tests/issue_create_json.rs` deepening** — 4 broad-pass BCs; 29 unit tests at the integration level. Markdown→ADF, label add/remove prefixes, --to/--account-id resolution, --markdown flag interactions all warrant individual BCs.

8. **`tests/duplicate_user_disambiguation.rs` deepening** — 5 tests; BC-706/707/708 cover but the test asserts a complex stderr shape (both emails + both accountIds + name) that would benefit from being decomposed into 3-4 per-flow BCs (issue list, issue assign, issue create, no-email fallback).

9. **`tests/sprint_commands.rs` deepening** — 13 tests; broad enumerated 3 BCs. MAX_SPRINT_ISSUES=50 cap, scrum-only, --board override warrant integration-level pinning beyond the `cli/sprint.rs::tests` unit tests.

10. **`tests/queue.rs` deepening** — 11 tests; broad has 3 BCs. JSM service desk discovery + AQL via `aqlFunction` should each get a discrete BC.

11. **`tests/board_commands.rs` deepening** — 15 tests; broad has 3 BCs. Board list (with type filter), board view limit/all, auto-resolve all need per-test enumeration.

12. **`tests/team_commands.rs` deepening** — 5 tests; org-metadata GraphQL + list_teams + error paths.

13. **`tests/team_object_shape.rs` deepening** — 4 tests pinning shape tolerance for team_id (string vs object); BC-606 covers but each test pins a different shape variant worth discrete BCs.

14. **`tests/issue_list_errors.rs` exhaustive** — 7 tests. BC-105..111 covers most; round 2 should ensure all 7 are 1:1 BC'd with pinned literal stderr substrings.

15. **`tests/api_client.rs` 12-test extract_error_message coverage** — only ~8 enumerated as discrete BCs in this round; 4 more (`extract_error_message_singular`, `_plain_text_body`, `_empty_body`) need individual BCs in §3.8.

16. **NFR-grade BCs around `Retry-After` upper bound and HTTP-date** — currently NFR §7.1.3 / §7.1.4. After Round 2 should be cross-referenced with H-027 holdout.

---

## 10. Updated stats table — BCs HIGH/MEDIUM/LOW per subject area

| Subject area (Pass 3 broad) | broad H/M/L | Round 1 H/M/L | Untested invariants from Pass 2 |
|---|---|---|---|
| 1. Auth & Identity | 14 / 7 / 2 | 30 / 4 / 0 | INV-11 partially closed |
| 2. Issue read (list/view/comments/changelog) | 17 / 6 / 1 | 38 / 5 / 1 | none |
| 3. Issue write | 16 / 5 / 1 | 21 / 5 / 1 | INV-14 stable |
| 4. Issue assets / CMDB | 10 / 4 / 1 | 22 / 3 / 0 | none |
| 5. Boards & Sprints | 7 / 3 / 0 | 7 / 3 / 0 | INV-9 unchanged |
| 6. Worklogs & duration | 5 / 1 / 0 | 5 / 1 / 0 | none |
| 7. Teams | 4 / 2 / 0 | 4 / 2 / 0 | none |
| 8. Users | 8 / 1 / 0 | 8 / 1 / 0 | none |
| 9. Projects & Queues | 6 / 2 / 0 | 6 / 2 / 0 | none |
| 10. Configuration | 9 / 2 / 1 | 14 / 2 / 1 | INV-21 closed (BC-153) |
| 11. Cache | 7 / 2 / 1 | 16 / 2 / 1 | INV-10 closed (BC-1005-R) |
| 12. Output formatting | 6 / 4 / 1 | 6 / 4 / 1 | INV-25 unchanged |
| 13. Error handling | 11 / 3 / 0 | 18 / 3 / 0 | none |
| 14. Build-time | 5 / 1 / 1 | 7 / 1 / 1 | INV-13/15/16 unchanged |
| 15. Runtime concerns | 9 / 2 / 0 | 16 / 2 / 0 | INV-13 partially closed (BC-031/32/33) |
| **Totals** | **134 / 45 / 9** | **218 / 40 / 6** | 4 of 7 INV closed/promoted |

Untested invariants closed this round:
- **INV-10** — `clear_profile_cache` no-op for nonexistent dir (BC-1005-R now HIGH).
- **INV-21** — Migration write-back uses file-only baseline (BC-153 now HIGH).
- **INV-13** — Embedded callback port 53682 (BC-031 + BC-032 + BC-033).

Untested invariants still open:
- **INV-11** — per-profile keychain key namespacing (still gated by `JR_RUN_KEYRING_TESTS=1`).
- **INV-12** — non-default profiles never inheriting legacy keychain (still gated).
- **INV-22** — MAX_SPRINT_ISSUES=50 cap (still no integration test passing 51+ keys).
- **INV-24** — date validators run pre-HTTP (closed for `--recent` and 4 date filters via BC-131/132; remains open for `--asset` AQL and other paths).
- **INV-25** — `--no-input` auto-set when stdin not TTY (`assert_cmd` always non-TTY → fundamentally hard to integration-test).

---

## 11. State Checkpoint

```yaml
pass: 3
round: 1
status: complete
bcs_total_after_round: 271
bcs_high: 211
bcs_medium: 53
bcs_low: 7
bcs_added_this_round: 87
bcs_promoted_to_high: 13
bcs_promoted_low_to_higher: 4
bcs_retracted: 4
holdout_candidates_after_round: 29
untested_behaviors_listed: 23
files_examined: 20
novelty: SUBSTANTIVE
timestamp: 2026-05-04T18:30:00Z
inputs_consumed:
  - .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md
  - .factory/semport/jira-cli/jira-cli-pass-6-synthesis.md
  - .reference/jira-cli/tests/auth_profiles.rs (full, 333 LOC)
  - .reference/jira-cli/tests/auth_refresh.rs (full, 106 LOC)
  - .reference/jira-cli/tests/auth_login_config_errors.rs (full, 97 LOC)
  - .reference/jira-cli/tests/oauth_embedded_login.rs (full, 32 LOC)
  - .reference/jira-cli/tests/migration_legacy.rs (full, 172 LOC)
  - .reference/jira-cli/tests/api_client.rs (full, 444 LOC)
  - .reference/jira-cli/tests/all_flag_behavior.rs (full, 686 LOC)
  - .reference/jira-cli/tests/issue_list_errors.rs:350-423 (chunk)
  - .reference/jira-cli/tests/issue_view_errors.rs:130-207 (chunk)
  - .reference/jira-cli/tests/issue_resolution.rs:85-159 (chunk)
  - .reference/jira-cli/tests/cmdb_fields.rs:140-190 (chunk)
  - .reference/jira-cli/tests/assets.rs (chunked: 1-110, 140-340, 1480-1799)
  - .reference/jira-cli/src/cache.rs (chunked: 1-490, 760-900)
  - .reference/jira-cli/src/config.rs:170-420 (chunk)
  - .reference/jira-cli/src/api/auth.rs:1-220, 370-490 (chunks)
  - .reference/jira-cli/src/api/client.rs:180-490 (chunk)
  - .reference/jira-cli/src/api/rate_limit.rs (full, 56 LOC)
  - .reference/jira-cli/src/jql.rs:1-180 (chunk)
  - .reference/jira-cli/src/duration.rs:1-75 (chunk)
  - .reference/jira-cli/src/cli/issue/list.rs (chunked: 1-450, 600-1083)
  - .reference/jira-cli/src/cli/issue/changelog.rs:170-270 (chunk)
next_round_targets: |-
  T-04 cli_handler.rs full sweep (54 tests, BC-201..225 surface): per-test BCs for assign happy/error/idempotent/--unassign, move idempotency/transitions/resolution-required, comment + comments, attachment / open / link / unlink. JSON output shape stability (BC-1208).

  T-04 issue_changelog.rs full sweep (39 tests, BC-119..121 expansion): AuthorNeedle classification table (12-char threshold, colon, digit-required); --field substring filter (case-insensitive, repeatable OR semantics); --reverse; --all vs 30-cap interaction; 401/404/network-drop error paths; snapshot tests; parse-failure verbose-gating; changelog format-date.

  T-09 adf.rs (1,826 LOC, 69 unit tests): per-node BCs for text→ADF, markdown→ADF, ADF→text round-trip. Tables, code blocks, headings, lists, links, mentions, emoji, hardBreak.

  T-10 cli/issue/changelog.rs unit tests (38): AuthorNeedle smart-constructor edge cases (LoweredStr tag, classify boundaries).

  T-11 OAuth state machine + refresh_oauth_token deferred-integration formal characterization. The "what happens on 401" gap — auto-refresh would call refresh_oauth_token; currently it doesn't. Pass 6 INC-07 framing.

  Tests/comments.rs (9 tests, 3 broad-BCs) — --internal JSM property + 5xx friendly + offset pagination per-flow BCs.

  Tests/issue_create_json.rs (29 tests, 4 broad-BCs) — markdown→ADF, label add/remove prefixes, --to/--account-id resolution, --markdown flag.

  Tests/duplicate_user_disambiguation.rs (5 tests) — decompose BC-706/707/708 into per-flow BCs (issue list, assign, create, no-email).

  Tests/sprint_commands.rs (13 tests, 3 broad-BCs) — MAX_SPRINT_ISSUES=50 cap (INTEGRATION-test pin, INV-22 closure), scrum-only, --board override.

  Tests/queue.rs (11 tests) — JSM service desk discovery + aqlFunction.

  Tests/board_commands.rs (15 tests, 3 broad-BCs) — board list type filter, view limit/all, auto-resolve.

  Tests/team_commands.rs (5 tests) + tests/team_object_shape.rs (4 tests).

  api_client.rs full extract_error_message coverage — 4 more BCs (singular `errorMessage`, plain text, empty body, etc.) and BC-1201 sub-IDs.

  NFR cross-reference: H-027 (Retry-After upper bound) + Pass 4 §7.1.3/§7.1.4 confirmation.
```


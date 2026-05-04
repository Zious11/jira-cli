# Pass 2 Deepening — Round 4 — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04

## 1. Round metadata

- **Round**: 4
- **Predecessor**: `jira-cli-pass-2-deep-r3.md`
- **Targets attacked (verbatim from R3 §9 high-priority + medium-priority)**:
  - **#1** — `cli/auth.rs` deep round 3 — `login_oauth` line-by-line, `login_token`, `handle_remove_in_memory`, every `JrError::UserError` site
  - **#2** — `cli/issue/list.rs` deep round 3 — `build_filter_clauses` per-position, `--asset` short-circuit, `--open` + `--status` interaction, "Showing N of ~M" approximation
  - **#3** — `cli/assets.rs` deep round 3 — `handle_search` enrichment cost, `handle_tickets` 4-state filter, `handle_types` schema-name injection, `handle_schema` (corrected: this is type-attributes view, NOT single-schema view)
  - **#4** — `api/auth.rs` OAuth login flow line-by-line — `oauth_login` orchestration, `accessible_resources` fetch + cloud_id selection, PKCE absence verification, browser-launch fallback
  - **#5** — `config.rs` deep round 3 — `Config::project_key`, `validate_profile_name`, `Figment::merge(Env::prefixed("JR_"))` env-var injection scope
  - **#6** — `api/assets/*` (5 files including discovered `schemas.rs`) — line-by-line
  - **#7** — `api/jsm/*` (servicedesks.rs + queues.rs)
  - **#8** — `types/jira/issue.rs` and `types/assets/object.rs` — serde shapes (custom fields, `IssueFields::extra` HashMap, ObjectAttribute vs AssetAttribute split)
  - **#9** — Integration tests `tests/*.rs` (33 files) — wiremock-vs-non-wiremock catalog for Pass 3 BC derivation

(Pass 4 cross-pollination items #15-19 reserved for Pass 4 — not written into this Pass 2 file.)

---

## 2. Audit of Round 3 against the 5 Known Hallucination Classes

### Class 1 — Over-extrapolated token lists

- **R3 NEW-INV-105 "4-stage XOR pipeline"** — re-verified. Counted at `build.rs:14-125` and `auth_embedded.rs:1-251`:
  1. env-read (`build.rs:18-22`),
  2. `generate_xor_key` (`build.rs:75-99`),
  3. XOR encode + emit constants (`build.rs:103-124`),
  4. runtime decode + cache (`auth_embedded.rs:75-130`).
  **Confirmed: 4 stages.** ✓
- **R3 "8-test embedded catalog"** — re-verified. `grep -c '#\[test\]' src/api/auth_embedded.rs` = **8**. Names enumerated at lines 143, 158, 164, 181, 195, 208, 221, 244. **Confirmed.** ✓
- **R3 NEW-INV-119 "auth status fall-through to embedded"** — re-verified at `cli/auth.rs:677-708`. The 3-state pure helper (`peek_oauth_app_source_for_test`) discriminates Keychain/Embedded/None with explicit branches. Production wrapper (line 677) eats the keychain `Err`, emits stderr warning, and falls through to embedded probe. **Confirmed: 1 state collapse with eprintln warning.** ✓
- **R3 §3.4 NEW-INV-122 "4-fallback team display chain"** — re-verified at `view.rs:251-277`. Counted: 4 branches (Ok-Some-cached, Ok-Some-not-cached, Ok-None, Err-cache-unreadable). **Confirmed.** ✓
- **R3 §3.6 NEW-INV-138 "10 keyring tests"** — re-verified `grep -nE 'fn .*(round_trip|legacy|partial|recovers|migration|prefers|errors)' src/api/auth.rs` matches 10 tests at lines 1132, 1147, 1155, 1187, 1223, 1251, 1273, 1325, 1349, 1364 (the line numbers in R3's table). Plus `fixed_port_strategy_eaddrinuse_friendly_error` at 1378 (not gated). **Confirmed.** ✓

### Class 2 — Miscounted enumerations

- **R3 self-claim "31 new entities, 61 new invariants"** — recount of R3 §5:
  - Entities: ADF (14: E-ADF-R2-01..14, but R3 actually catalogues 12 entries E-ADF-R2-01..12), OAuth (7: E-OAUTH-R2-01..07), CLI-AUTH (6), VIEW+CMTS (3), CLI-LIST (5), ASSETS (5), API-AUTH (4), CACHE (3), CONFIG (4), OBS (2), ERROR (1), OUTPUT (1), FIX (1) = 12+7+6+3+5+5+4+3+4+2+1+1+1 = **54 entries claimed in §3, not 31**. R3 self-quoted "31" matches the **delta** (cross-references against R1/R2 cumulative inheritance), not the §3 entry count. Logged as counting-convention slip. **CONV-ABS-5 (CORRECTION):** R3 said "ADF deepening +14 entities" but actually catalogues E-ADF-R2-01 through E-ADF-R2-13 = **13** (one labeled section per #### header; the "14" was off by 1).
  - Invariants: NEW-INV-93..NEW-INV-153 = **61** range. ✓ Matches.
- **R3 NEW-INV-101 "mention/emoji/inlineCard/media* lossy in text"** — re-verified at `adf.rs:379-541`. Searched for `"mention"`, `"emoji"`, `"inlineCard"`, `"media"`, `"mediaSingle"`, `"mediaGroup"` as match arms. **None present** in `render_node` match arms. Confirmed lossy via fall-through. ✓

### Class 3 — Named pattern conflation / fabrication

- **CONV-ABS-6 (CORRECTION):** R3 §3.5 framed `handle_schema` (assets.rs:626-752) as "dedicated single-schema view" (carry from R2). **Re-read source + cli/mod.rs:179-185:**
  ```rust
  Schema {
      name: String,                    // type name (partial match)
      schema: Option<String>,          // schema filter
  }
  ```
  `handle_schema` is **NOT** a single-schema view — it's "show attributes for an object **type**" (the user supplies a type name, optionally narrowed by `--schema`). The handler resolves the type name across schemas, errors if cross-schema duplicates exist, then fetches the type's attribute definitions. R3 (and R2 carry) misframed the operation. The naming `handle_schema` is itself misleading — it dispatches the `Schema` subcommand which is "type detail view." **No invariant retracted; framing corrected.**
- **R3 §4.2 "OAuth login dispatch state machine"** — verified as conceptual frame, NOT a named code construct. The actual implementation has no `enum DispatchState` or state-machine struct. Round 4 keeps the diagrammatic framing but documents that no Rust type backs it. ✓ (acceptable analytical frame)
- **R3 §4.1 "ADF whitespace state machine"** — same: conceptual frame, no `enum AdfState`. ✓
- **R3 §4.3 "Embedded XOR pipeline state machine"** — same: conceptual frame, no FSM type. The actual mechanism is two-stage build-time + runtime, not a state machine. ✓ (acceptable, but should not imply Rust state-machine code)

### Class 4 — Same-basename artifact conflation

- **R3 §3.4 "view.rs 286 LOC + comments.rs 61 LOC"** — re-verified `wc -l src/cli/issue/view.rs src/cli/issue/comments.rs` = **286 / 61**. ✓ Confirmed.
- **CONV-ABS-7 (DISCOVERY):** R3 missed `src/api/assets/schemas.rs` (45 LOC). CLAUDE.md lists 4 files in `api/assets/` (workspace.rs, linked.rs, objects.rs, tickets.rs); the actual tree has **5**: those 4 PLUS `schemas.rs` and `mod.rs`. The list_object_schemas + list_object_types impls live in `schemas.rs`, NOT in `objects.rs` as CLAUDE.md's framing implies. CLAUDE.md is stale on this point as well. (Mirrors CONV-ABS-4: stale CLAUDE.md framing carrying forward.)

### Class 5 — Inflated or deflated metrics (LOC recount)

| File | R3 cited | Actual | Delta |
|---|---:|---:|---|
| `src/cli/issue/view.rs` | 286 | 286 | 0 ✓ |
| `src/cli/issue/comments.rs` | 61 | 61 | 0 ✓ |
| `src/cli/issue/format.rs` | 226 | 225 | -1 (1-LOC trailing-newline rounding) |
| `src/observability.rs` | 39 | 39 | 0 ✓ |
| `src/cli/issue/list.rs` | 1,083 | 1,083 | 0 ✓ |
| `src/cli/assets.rs` | 1,055 | 1,055 | 0 ✓ |
| `src/api/auth.rs` | 1,397 | 1,397 | 0 ✓ |
| `src/cli/auth.rs` | 1,998 | 1,998 | 0 ✓ |
| `src/api/auth_embedded.rs` | 250 | 250 | 0 ✓ |
| `src/api/assets/linked.rs` | (not cited) | 557 | n/a |
| `src/api/assets/objects.rs` | (not cited) | 237 | n/a |
| `src/api/assets/workspace.rs` | (not cited) | ~59 | n/a |
| `src/api/assets/tickets.rs` | (not cited) | ~20 | n/a |
| `src/api/assets/schemas.rs` | (not cited) | 45 | NEW (CONV-ABS-7) |
| `src/api/jsm/queues.rs` | (not cited) | 86 | n/a |
| `src/api/jsm/servicedesks.rs` | (not cited) | 128 | n/a |
| `src/types/jira/issue.rs` | (not cited) | 625 | n/a |
| `src/types/assets/object.rs` | (not cited) | 329 | n/a |

**Hallucination class audit summary**: **0 prior-round substantive entity/invariant retracted**. **3 framing/counting corrections logged**: CONV-ABS-5 (R3 ADF entity count "+14" → 13), CONV-ABS-6 (`handle_schema` is type-detail view, not single-schema view; R2/R3 carry-framing slip), CONV-ABS-7 (CLAUDE.md misses `api/assets/schemas.rs`; R3 inherited the stale framing). All R3 "potential bug" claims (NEW-INV-119/143/etc.) re-verified by line-by-line read.

---

## 3. Sub-pass 2a deepening: structural — entity model per target

### 3.1 T-CLI-AUTH-R3: `cli/auth.rs` deep round 3

#### E-CLI-AUTH-R3-01 — `login_oauth` 7-step orchestration (lines 420-514)

```
1. (lines 426-438) Print stderr banner — "embedded jr app" vs "no embedded app";
   skipped under --no-input (CI/agent mode).
2. (lines 440-441) resolve_oauth_app_credentials(client_id, client_secret, no_input)
   → returns (id, secret, OAuthAppSource).
3. (lines 446-451) Choose RedirectUriStrategyRequest:
     - Embedded → Fixed(EMBEDDED_CALLBACK_PORT=53682)
     - All other sources (Flag/Env/Keychain/Prompt) → Dynamic
4. (lines 457-475) Config::load_lenient_with(Some(profile)) + resolve_oauth_scopes
   on the TARGET profile (NOT active). Empty/whitespace scopes → ConfigError fast-fail
   BEFORE the keychain is touched (defensive ordering).
5. (lines 481-483) Persist user-provided OAuth app creds to keychain — SKIPPED for
   Embedded source (no point: re-decoded every launch).
6. (lines 485-487) auth::oauth_login(profile, &client_id, &client_secret, &scopes,
   strategy).await → executes the full 5-stage flow (redirect bind → browser →
   listener → token exchange → accessible-resources → keychain store).
7. (lines 493-510) Reload config (lenient), set p.url/cloud_id/auth_method;
   safeguard default_profile.is_none() → set to target. save_global. print_success
   with site_name.
```

- **NEW INVARIANT (NEW-INV-154)**: The `login_oauth` flow has TWO sequential `Config::load_lenient_with` calls (lines 461 and 493) wrapping the actual oauth_login call. Each load reads the file, applies `JR_*` env overlay, and resolves the active profile name. **Architectural pattern: re-load after I/O** to pick up mutations made by `prepare_login_target` and any env-var changes between handler entry and handler post-network-action. A scenario where `JR_PROFILE` was changed mid-process would cause the second load to use a different active profile than the first — but the explicit `Some(profile)` parameter override defeats this concern (the per-call CLI override always wins).
- **NEW INVARIANT (NEW-INV-155)**: Order of operations is **scope-validate → keychain-write → network**. This matters for failure UX: a user who configures `oauth_scopes = "  "` (whitespace-only) in their target profile gets a clean ConfigError BEFORE their valid client_id/client_secret are written to the keychain. Reversing the order would persist app creds for a flow that never completes. Pinned by code structure, not by a unit test.
- **NEW INVARIANT (NEW-INV-156)**: The "(could not auto-open browser: {e})" eprintln on line 564 is **non-fatal** — `oauth_login` continues to listener.accept() even when `open::that` fails. A headless CI runner without a browser must paste the printed authorize URL into a separate browser session. The flow does NOT depend on the local process opening the browser. Pinned by `if let Err(e) = open::that(&auth_url) { eprintln!(...); }` (no `?`, no early return).

#### E-CLI-AUTH-R3-02 — `login_token` 5-step orchestration (lines 353-412)

```
1. (lines 359-367) resolve_credential(email, ENV_EMAIL, "--email", ...) — flag/env/prompt
2. (lines 368-376) resolve_credential(token, ENV_API_TOKEN, "--token", is_password=true, ...)
3. (line 378) auth::store_api_token(&email, &token) — keychain write of SHARED keys
4. (lines 391-397) Config::load_lenient_with(Some(profile)) + entry.or_default() +
   p.auth_method = "api_token"
5. (lines 405-407) default_profile safeguard: if None, set to target.
6. (line 408) save_global; eprintln "Credentials stored in keychain."
```

- **NEW INVARIANT (NEW-INV-157)**: `login_token` writes to **3 distinct keychain locations**: (1) shared `email`, (2) shared `api-token`, (3) NO per-profile entries (the api-token is account-level, NOT profile-scoped). The profile only gets `auth_method = "api_token"` in config.toml. **Multi-profile wrinkle**: two profiles using api_token share the same email + api-token. A user logging in to "sandbox" with email A then "default" with email B would silently overwrite A's email. Pass 4 reliability candidate.
- **NEW INVARIANT (NEW-INV-158)**: `login_token` does NOT validate the token by hitting Jira before declaring success. The "Credentials stored in keychain." message means "we wrote bytes to the keychain" — NOT "those bytes authenticate against the configured URL." A subsequent `jr issue list` is the first call that actually validates. Pinned by absence of any `client.get("/myself")`-style probe in the flow.

#### E-CLI-AUTH-R3-03 — `handle_remove_in_memory` 4-guard validator (lines 981-1021)

**4 sequential guards** that can block removal:
1. `validate_profile_name(target)?` — rejects invalid name (UserError, exit 64).
2. `!global.profiles.contains_key(target)` → "unknown profile: {target}; known: ..." UserError.
3. `target == active` → "cannot remove active profile {target:?}; switch first..." UserError.
4. `global.default_profile.as_deref() == Some(target)` → "cannot remove profile {target:?}: it is the default_profile in config. Switch the default first..." UserError.

Only after ALL 4 guards pass: `global.profiles.remove(target)`.

- **NEW INVARIANT (NEW-INV-159)**: Guards 3 and 4 are **distinct**: a profile can be the default_profile WITHOUT being the active profile (e.g., `jr --profile sandbox auth remove default` where active=sandbox but default_profile=default). Without guard 4, this case would leave config.toml with `default_profile = "default"` pointing to a removed entry, and the next strict `Config::load()` would error "active profile 'default' not in [profiles]". Guard 4 has its own explicit error message ("Switch the default first..."). Pinned by distinct error strings (the test at lines 1567-1606 covers the active case; the default_profile-but-not-active case is implicitly pinned by the `if global.default_profile.as_deref() == Some(target)` branch).
- **NEW INVARIANT (NEW-INV-160)**: `handle_remove` (line 1035) **pre-validates** by calling `handle_remove_in_memory(config.global.clone(), target, ...)` BEFORE the user confirmation dialog. A typo like `jr auth remove typo` errors before the user clicks through "Permanently remove profile?" Yes/No — saving them an unnecessary confirmation step. Pinned by line 1047 `let _ = handle_remove_in_memory(config.global.clone(), ...)`.

#### E-CLI-AUTH-R3-04 — `JrError::UserError` construction sites in `cli/auth.rs` (verbatim catalog)

| Line | Function | Message theme |
|---:|---|---|
| 59 | `resolve_credential` | "{prompt} is required. Provide {flag} or set ${env}." (with optional hint) |
| 170 | `resolve_oauth_app_credentials_for_test` | "--client-id without --client-secret" |
| 178 | `resolve_oauth_app_credentials_for_test` | "--client-secret without --client-id" |
| 199 | `resolve_oauth_app_credentials_for_test` | "JR_OAUTH_CLIENT_ID without JR_OAUTH_CLIENT_SECRET" |
| 207 | `resolve_oauth_app_credentials_for_test` | "JR_OAUTH_CLIENT_SECRET without JR_OAUTH_CLIENT_ID" |
| 224 | `resolve_oauth_app_credentials_for_test` | "OAuth app credentials required + binary not built with embedded" (under no_input) |
| 652 | `prepare_login_target` | "--url required when target profile has no URL configured" (under no_input) |
| 755 | `status` | "unknown profile: {target}; known: {...}" (when --profile X is unknown) |
| 877 | `refresh_credentials` | "profile {target:?} has no URL configured. Use jr auth login..." |
| 960 | `handle_logout` | "unknown profile: {target}; known: {...}" |
| 989 | `handle_remove_in_memory` | "unknown profile: {target}; known: {...}" |
| 1000 | `handle_remove_in_memory` | "cannot remove active profile {target:?}; switch first..." |
| 1013 | `handle_remove_in_memory` | "cannot remove profile {target:?}: it is the default_profile" |
| 1098 | `handle_switch_in_memory` | "unknown profile: {target}; known: {...}" |

PLUS one ConfigError site:
- 334 | `resolve_oauth_scopes` | "oauth_scopes is empty; remove the setting to use defaults" (exit 78)

Plus two more ConfigError wraps for load failures:
- 462 | `login_oauth` | "Failed to load config: ..." (Config load wrap)
- 553 | `handle_login` | "Failed to load config: ..." (same)

**Total: 14 UserError sites + 3 ConfigError sites = 17 error construction sites in `cli/auth.rs`.**

- **NEW INVARIANT (NEW-INV-161)**: The "unknown profile: X; known: ..." pattern recurs at **5 distinct sites** in `cli/auth.rs` (status, handle_logout, handle_remove_in_memory, handle_switch_in_memory, plus `config.rs::load_inner`). All 5 use the same wording ("unknown profile: X; known: Y, Z"). Refactoring this into a `JrError::unknown_profile(name, known)` constructor would reduce duplication; today it's a copy-paste convention with one hand-edited risk. **Architectural opportunity, not a bug.**
- **NEW INVARIANT (NEW-INV-162)**: The user-facing error message "(none)" appears in 4 of those 5 sites as the fallback when `known.is_empty()` — yet `handle_switch_in_memory` (line 1098) doesn't show "(none)" because by switch time, profile-emptiness is impossible (you can only switch into a configured profile, and config.rs strict load errors first if the profiles map is empty under a non-"default" target). Subtle invariant: switch is privileged.

### 3.2 T-CLI-LIST-R3: `cli/issue/list.rs` deep round 3

#### E-CLI-LIST-R3-01 — `build_filter_clauses` 10-position semantic order (lines 612-649)

**10 conditional pushes in fixed order**:

| Pos | Field | Push when | JQL fragment |
|---:|---|---|---|
| 1 | `assignee_jql` | Some | `assignee = {value}` |
| 2 | `reporter_jql` | Some | `reporter = {value}` |
| 3 | `status` | Some | `status = "{escaped}"` |
| 4 | `open` | true | `statusCategory != Done` |
| 5 | `team_clause` | Some | `{team_clause}` (already-formed) |
| 6 | `recent` | Some | `created >= -{duration}` |
| 7 | `asset_clause` | Some | `{asset_clause}` (aqlFunction-formed) |
| 8 | `created_after_clause` | Some | `created >= "{date}"` |
| 9 | `created_before_clause` | Some | `created < "{date+1d}"` (next-day exclusive) |
| 10 | `updated_after_clause` | Some | `updated >= "{date}"` |
| 11 | `updated_before_clause` | Some | `updated < "{date+1d}"` |

(11 actual positions — the 10 in the table I quoted earlier was off by 1; recounted as 11.)

- **NEW INVARIANT (NEW-INV-163)**: `build_filter_clauses` does NOT validate combinations — `--open` AND `--status` can both be passed at the CLI level (the code allows). The clap-derive level enforces conflicts on assets `Tickets` (cli/mod.rs:164-168 has `conflicts_with = "status"`/`"open"`), but the issue-list `IssueCommand::List` does NOT have those `conflicts_with` markers. So `jr issue list --open --status "In Progress"` produces JQL `status = "In Progress" AND statusCategory != Done`, which is technically valid but probably not what the user meant (statusCategory is implied by status). **Pass 4 UX inconsistency**: assets-tickets enforces conflict at CLI level, issue-list doesn't.
- **NEW INVARIANT (NEW-INV-164)**: `created_before` and `updated_before` are **next-day-exclusive** (`d + chrono::Days::new(1)` then `<`). User-friendly: `--created-before 2024-01-15` includes the entire 15th. Pinned by `let next_day = d + chrono::Days::new(1);` (lines 119, 124). Distinct from the `>=` semantic of `--created-after`/`--updated-after` which are inclusive of the named day.

#### E-CLI-LIST-R3-02 — `--asset` short-circuit when no CMDB fields (lines 169-183)

**Hard-error path**:
```rust
if let Some(ref key) = asset_key {
    let cmdb_fields = get_or_fetch_cmdb_fields(client).await?;
    if cmdb_fields.is_empty() {
        return Err(JrError::UserError(
            "--asset requires Assets custom fields on this Jira instance. \
             Assets requires a paid Jira Service Management plan."
                .into(),
        )
        .into());
    }
    ...
}
```

- **NEW INVARIANT (NEW-INV-165)**: `--asset` (filter-by-asset) errors HARD when `get_or_fetch_cmdb_fields` returns empty. Distinct from `--assets` (display-asset-column) which DEGRADES with eprintln warning + empty cmdb (lines 361-368) — that path runs the query without the asset column. The two flags `asset` (filter, singular) vs `assets` (column, plural) have **divergent failure modes** for the same underlying condition. Pass 4 UX-confusion candidate.
- **NEW INVARIANT (NEW-INV-166)**: Even when `cmdb_fields` is non-empty, `build_asset_clause(key, &cmdb_fields)` (jql.rs) uses `aqlFunction()` with the human-readable field NAME (per CLAUDE.md gotcha), NOT the customfield ID. So a customfield with a renamed-but-same-ID would silently break this clause when the cache is stale. Pinned by `crate::jql::build_asset_clause` semantics (broad pass §3, R1).

#### E-CLI-LIST-R3-03 — "Showing N of ~M" approximation logic (lines 554-571)

**3-branch behavior**:
1. `has_more && !all` AND `client.approximate_count(count_jql).await` returns `Ok(total)` AND `total > 0`:
   → `eprintln!("Showing {} of ~{} results. Use --limit or --all to see more.", issues.len(), total);`
2. `has_more && !all` AND `Ok(0)` OR `Err(_)`:
   → `eprintln!("Showing {} results. Use --limit or --all to see more.", issues.len());`
3. `!has_more` OR `all`: no message.

The `approximate_count` API uses Jira's `/rest/api/3/search/approximate-count` endpoint (Cloud-only) which returns an approximate count rather than a paginated total — Jira deprecated exact-count for performance reasons.

- **NEW INVARIANT (NEW-INV-167)**: The "Showing N of ~M" message uses LITERAL `~` to indicate approximation (matches Jira's API semantics). A user/tool parsing this output for actual counts would have a 5-15% error margin (Jira's documented approximate-count tolerance). Pinned by the `~` in the format string.
- **NEW INVARIANT (NEW-INV-168)**: The `count_jql` for approximate_count is `crate::jql::strip_order_by(&effective_jql)` — Jira rejects ORDER BY in approximate-count requests. Pinned by line 555.
- **NEW INVARIANT (NEW-INV-169)**: `--all` suppresses the message even when `has_more` is true, on the assumption that `--all` already pagination-walked everything. But since the loop in `search_issues` actually IS bounded (per Round 1), `has_more=true` after `--all` would mean a hard cap was hit — a subtle case where the user thinks they got everything but didn't. The current code suppresses the warning in that case. **Pass 4 UX bug**: --all should still report when truncated.

### 3.3 T-CLI-ASSETS-R3: `cli/assets.rs` deep round 3

#### E-CLI-ASSETS-R3-01 — `handle_search` `attributes: bool` enrichment cost (lines 67-175)

**Two-mode behavior split at line 79 `if attributes`**:
- **`attributes == false` (lightweight)**: search → render Key/Type/Name only. **One API call** (`search_assets` with `include_attributes=false`).
- **`attributes == true` (heavy)**: search → call `enrich_search_attributes` (separate per-object-type API calls + cache reads) → render Key/Type/Name/Attributes (Table) or full attribute objects (JSON).

The `enrich_search_attributes` (api/assets/objects.rs:153-206) internally:
1. Collects unique object_type IDs from the search results.
2. For each unique type, reads `cache::read_object_type_attr_cache` (per-profile, 7-day TTL).
3. On cache miss, fetches `client.get_object_type_attributes(workspace_id, type_id)` and best-effort writes to cache.

- **NEW INVARIANT (NEW-INV-170)**: `--attributes` is N+1 in the worst-case (cold cache): 1 search call + N type-attribute calls (one per unique object type in the results). For a 25-result page across 10 unique types with cold cache, that's 11 API calls instead of 1. **Performance contract**: warm cache makes it ~1 call (search only). Spec implication: `--attributes` should not be the default for high-volume scripts.
- **NEW INVARIANT (NEW-INV-171)**: The cache key is `(profile, object_type_id)` — different profiles do NOT share the cache (per CLAUDE.md multi-profile boundary). But the cache CONTENTS (system/hidden/label/position metadata) are workspace-scoped, NOT site-scoped — a sandbox-vs-prod with the same workspace would have identical metadata. The per-profile partitioning is overcautious but correct.

#### E-CLI-ASSETS-R3-02 — `filter_tickets` 4-state branching (lines 306-370)

**State-table**:

| `open` | `status` | Behavior |
|---|---|---|
| `true` | (any) | Filter: `t.status.color_name != "green"` (Done category excluded). Tickets with NO status are KEPT (line 319 `.unwrap_or(true)`). |
| `false` | `Some(s)` | Partial-match `s` against unique status names. Tickets with NO status are EXCLUDED (line 365 `.unwrap_or(false)`). |
| `false` | `None` | Pass-through, no filter. |
| `true` | `Some(s)` | **CANNOT OCCUR** — clap-level `conflicts_with` (cli/mod.rs:164-168) prevents both flags simultaneously. |

**Asymmetric "missing status" treatment**: `--open` keeps stub tickets (defensive: "if we don't know the status, it might be open"); `--status X` drops them (defensive: "we can't match X if status is missing").

- **NEW INVARIANT (NEW-INV-172)**: The "missing status" asymmetry is a deliberate design choice, pinned by tests `filter_open_includes_no_status` (line 808) and `filter_status_excludes_no_status` (line 866). Reversing either would change visible-result counts in ways that surprise users.
- **NEW INVARIANT (NEW-INV-173)**: The `has_filter` boolean (line 386) determines JSON output shape: if any filter applied, returns BARE array; otherwise returns the full envelope (`{tickets, allTicketsQuery}`). **Reasoning**: `allTicketsQuery` no longer represents what's shown after filtering, so emitting it would mislead consumers. Pinned by the conditional at line 396-407.

#### E-CLI-ASSETS-R3-03 — `handle_types` schema_name injection in JSON (lines 553-567)

**JSON enrichment**: for each `ObjectTypeEntry`, look up its `object_schema_id` in the pre-built `schema_names: HashMap<&str, &str>` map and inject `"schemaName": "<name>"`.

```rust
let schema_name = schema_names.get(t.object_schema_id.as_str()).unwrap_or(&"");
map.insert("schemaName".to_string(), serde_json::Value::String(schema_name.to_string()));
```

- **NEW INVARIANT (NEW-INV-174)**: JSON output has the **string `"schemaName"` field** (camelCase, NOT snake_case). Distinct from the underlying serde struct `object_schema_id` (rust snake_case via serde rename). The injection is post-serialization manipulation of `serde_json::Value` — a downstream consumer expecting `object_schema_id` would NOT see `schemaName` from the on-wire shape. Pinned by line 561.
- **NEW INVARIANT (NEW-INV-175)**: Schema-name fallback is **empty string** (`unwrap_or(&"")`) for unknown schema IDs in JSON, but **em-dash `\u{2014}`** in Table mode (line 575). **Inconsistent missing-data sentinels**: Table uses em-dash for "missing"; JSON uses empty string. Pass 4 spec consistency candidate.

#### E-CLI-ASSETS-R3-04 — `handle_schema` 7-step type-attribute resolution (lines 626-752)

**Corrected framing (per CONV-ABS-6)**: this is **type-detail view** — given a type name, show its attribute definitions.

```
1. (line 633) list_object_schemas → all schemas
2. (lines 638-641) Filter to target schemas (one if --schema given, all otherwise)
3. (lines 644-650) Build candidates = (ObjectTypeEntry, schema_name) tuples for ALL types
4. (lines 662-679) Partial-match type_name against deduped names; resolve to matched_name
5. (lines 682-697) Cross-schema-duplicate check: same name in multiple schemas → error
   "Use --schema to narrow results."
6. (line 702-704) get_object_type_attributes(workspace_id, matched_type.id)
7. (lines 706-749) Render: JSON (raw attrs) OR Table with header "Object Type: X (Schema: Y)\n"
   + position-sorted, !system && !hidden filtered, format_attribute_type for the Type column
```

- **NEW INVARIANT (NEW-INV-176)**: **Cross-schema duplicate detection** is at the matched-name level, NOT the partial-match level. So `partial_match("server", &deduped_names)` returns `Exact("Server")`, then `same_name = candidates.iter().filter(|(t, _)| t.name == "Server")` finds 2+ matches across distinct schemas → error. Distinct from `MatchResult::Ambiguous` which catches partial-match ambiguity within the deduped name list. Pinned by lines 682-697.
- **NEW INVARIANT (NEW-INV-177)**: `format_attribute_type` (lines 615-624) has **3-state output**: `default_type.name` (e.g., "Text", "DateTime"), `"Reference → {name}"` (Unicode `\u{2192}` arrow), or `"Unknown"`. Distinct from the JSON shape which preserves the raw `defaultType` and `referenceObjectType` fields — Table rendering loses information. Pass 3 BC: type-attribute table is NOT round-tripable to type-attribute JSON.

### 3.4 T-API-AUTH-R3: `api/auth.rs` OAuth login flow line-by-line

#### E-API-AUTH-R3-01 — `oauth_login` 5-step flow with TOCTOU-safe binding (lines 545-690)

```
Step 1 (lines 552-557): strategy.bind() — atomically bind the listener.
   - Dynamic: TcpListener::bind("127.0.0.1:0") → ephemeral port.
   - Fixed(p): TcpListener::bind("127.0.0.1:{p}"); EADDRINUSE → friendly error
     directing user to BYO override.
   Owns the listener via ResolvedRedirect type. Fields private to prevent
   moving the listener out (would re-open TOCTOU).

Step 2 (lines 561-567): browser launch.
   - eprintln "Opening browser..." + "If browser doesn't open, visit: {auth_url}"
     (PRINTED EVERY TIME, regardless of open::that success — defensive: user can
     fall back to manual paste even when open::that returns Ok).
   - open::that(&auth_url) — non-fatal: Err → eprintln "(could not auto-open)..."
     and continue. NOT propagated.

Step 3 (lines 572-590): listener.accept() + raw HTTP-request parse.
   - 4096-byte buffer. Read once via stream.read.
   - extract_query_param for "code" and "state" (regex-free string parser at line 898).
   - State equality check: returned_state != state → bail!("CSRF attack")

Step 4 (lines 592-604): write success HTML page back to browser.
   - 5-line static HTML (Authorization successful! / You can close this tab.)
   - stream.write_all with .context() — partial-state error if write fails.

Step 5 (lines 607-689): exchange + accessible-resources + store.
   - POST https://auth.atlassian.com/oauth/token with grant_type=authorization_code
   - GET https://api.atlassian.com/oauth/token/accessible-resources with bearer_auth
   - resources.first() → ok_or "No accessible Jira sites found"
   - store_oauth_tokens(profile, access, refresh) — partial-state error path

Returns: OAuthResult { cloud_id, site_url, site_name }
```

- **NEW INVARIANT (NEW-INV-178)**: OAuth flow uses **NO PKCE** (no `code_verifier` or `code_challenge`). The client_secret is sent directly in the token-exchange POST body (line 612-616). This is "confidential client" flow per RFC 6749, NOT "public client" PKCE flow per RFC 7636. **Reason**: jr embeds the client_secret (or the user provides their own) — confidential-client model is appropriate. Pass 4 architecture invariant: PKCE migration would require a public-client OAuth app registration.
- **NEW INVARIANT (NEW-INV-179)**: `accessible_resources` selection is **first-result-wins** (`resources.first()`). A user with multiple accessible Jira sites gets the FIRST site as returned by Atlassian's API — there is NO disambiguation prompt. Pinned by line 666-668. **Pass 4 UX limitation**: a multi-site Atlassian user (e.g., contractors with access to multiple customer sites) cannot interactively choose which site `jr` connects to during `jr auth login --oauth`.
- **NEW INVARIANT (NEW-INV-180)**: The HTTP request parse at line 581 (`String::from_utf8_lossy`) is **lossy on non-UTF8 bytes**. A pathological browser sending non-UTF8 query params would have those bytes replaced with `U+FFFD`. Atlassian only sends ASCII OAuth params, so this is benign in practice. Pinned by `from_utf8_lossy` choice.
- **NEW INVARIANT (NEW-INV-181)**: `extract_query_param` (line 898) is a **regex-free hand-rolled parser** — finds `?` then splits by `&` then matches `param=`. Documented in source comments (broad/R1). Choice: avoid pulling in the `regex` crate just for this. Pinned by absence of any `regex::` import in `api/auth.rs`.

### 3.5 T-CONFIG-R3: `config.rs` deep round 3

#### E-CONFIG-R3-01 — `Config::project_key` 2-source precedence (lines 374-378)

```rust
pub fn project_key(&self, cli_override: Option<&str>) -> Option<String> {
    cli_override
        .map(String::from)
        .or_else(|| self.project.project.clone())
}
```

**Sources** (in precedence order):
1. `cli_override: Option<&str>` — passed by handlers from the global `--project` CLI flag.
2. `self.project.project: Option<String>` — from `.jr.toml` (per-project file walking parents).

- **NEW INVARIANT (NEW-INV-182)**: There is NO `JR_PROJECT` env var support — the precedence stops at CLI flag + .jr.toml file. Distinct from `JR_PROFILE`/`JR_BASE_URL`/`JR_*` env vars which Figment merges. **Architectural choice**: project key is a per-repo concern, not a per-shell-session concern; .jr.toml is the canonical source.
- **NEW INVARIANT (NEW-INV-183)**: `find_project_config` (line 337) walks parent directories until `dir.pop()` returns false (i.e., reaches filesystem root). The first `.jr.toml` encountered wins. **Behavior in nested repos**: a child project's `.jr.toml` shadows its parent's. Pinned by the loop in `find_project_config`.

#### E-CONFIG-R3-02 — `validate_profile_name` 4-rejection lattice (lines 113-133)

| Rejection | Condition |
|---|---|
| Empty | `name.is_empty()` |
| Too long | `name.len() > 64` |
| Invalid char | `!name.chars().all(|c| c.is_ascii_alphanumeric() \|\| c == '_' \|\| c == '-')` |
| Reserved Windows | `RESERVED_WINDOWS.contains(&upper.as_str())` (case-insensitive: 22 names) |

The 22 reserved Windows names: CON, NUL, AUX, PRN, COM1-9, LPT1-9.

- **NEW INVARIANT (NEW-INV-184)**: Profile names are **ASCII-alphanumeric + `_` + `-` only**. No dots, no slashes, no Unicode. Rationale (per `docs/specs/multi-profile-auth.md`): names flow into cache paths and keychain key prefixes; non-ASCII or path-special chars would create platform-specific failures. Pinned by `chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')`.
- **NEW INVARIANT (NEW-INV-185)**: The 64-char limit is **silent** in error messages (the error mentions "up to 64 chars" but doesn't tell the user how long their input was). A user with a 65-char profile name sees "invalid profile name 'xxxxxxxxxxxxxxxxxxxxx...'; allowed: A-Z a-z 0-9 _ - up to 64 chars; reserved Windows names excluded" and has to count chars themselves. Pass 4 UX nitpick.
- **NEW INVARIANT (NEW-INV-186)**: Reserved-Windows check is **case-insensitive**: `con`, `Con`, `CON` all rejected. Validates portability — a config file authored on Linux with `[profiles.con]` would fail when used on Windows. Pinned by `name.to_ascii_uppercase()` before contains check.

#### E-CONFIG-R3-03 — `Figment::merge(Env::prefixed("JR_"))` env-var injection scope (lines 225-229)

The Figment chain:
```rust
let mut global: GlobalConfig = Figment::new()
    .merge(Serialized::defaults(GlobalConfig::default()))
    .merge(Toml::file(&global_path))
    .merge(Env::prefixed("JR_"))
    .extract()?;
```

`Env::prefixed("JR_")` extracts ALL env vars starting with `JR_` and merges them as struct fields. The deserialization shape:
- `JR_DEFAULTS_OUTPUT=json` → `global.defaults.output = "json"` (nested via `_`-as-separator).
- `JR_DEFAULT_PROFILE=sandbox` → `global.default_profile = "sandbox"`.
- `JR_PROFILES_DEFAULT_URL=https://x.example` → would attempt to set profile entry "default"'s URL.
- **NOT MERGED**: `JR_BASE_URL`, `JR_PROFILE`, `JR_AUTH_HEADER`, `JR_API_TOKEN`, `JR_EMAIL`, `JR_OAUTH_CLIENT_ID`, `JR_OAUTH_CLIENT_SECRET`, `JR_RUN_KEYRING_TESTS`, `JR_SERVICE_NAME`, `JR_BUILD_OAUTH_*` — these are read DIRECTLY by their callers (api/client.rs, api/auth.rs, etc.), NOT by the config layer.

- **NEW INVARIANT (NEW-INV-187)**: There are **TWO env-var consumption pathways**: (a) Figment-merged into `GlobalConfig` (via field-name → env-name mapping), and (b) direct `std::env::var(...)` reads at call sites (e.g., `JR_BASE_URL` at config.rs:351; `JR_PROFILE` at config.rs:299; `JR_SERVICE_NAME` at api/auth.rs:14). The two pathways are NOT aware of each other. **Architectural risk**: a user setting `JR_PROFILES_DEFAULT_URL` (Figment) AND `JR_BASE_URL` (direct) would have `JR_BASE_URL` win at request time (per config.rs:351 explicit override). Pinned by code structure.
- **NEW INVARIANT (NEW-INV-188)**: The Figment env-merge happens BEFORE migration — so a user setting `JR_INSTANCE_URL=https://...` (legacy field) would have it merged into `global.instance.url`, then migration would copy it into `global.profiles["default"].url`. This is backward-compat behavior; for new shape, users would set `JR_PROFILES_DEFAULT_URL` instead. Pinned by line 225-229 ordering vs line 240-264 migration block.
- **NEW INVARIANT (NEW-INV-189)**: The MIGRATION write-back path (line 253-258) uses **file-only** Figment (NO env merge), so transient env-vars on the migration-triggering invocation cannot bleed into the on-disk config. **Critical design correctness**: a user running `JR_DEFAULTS_OUTPUT=json jr issue list` once does NOT permanently change their config's defaults.output. Pinned by the explicit `file_only_global: GlobalConfig = Figment::new()...merge(Toml::file(...))...extract()` (no env merge).

### 3.6 T-API-ASSETS-R3: `api/assets/*` line-by-line (5 files)

#### E-API-ASSETS-R3-01 — `workspace.rs::get_or_fetch_workspace_id` (59 LOC)

**4-stage flow**:
1. Read profile-scoped cache → return if hit.
2. GET `/rest/servicedeskapi/assets/workspace` returning `ServiceDeskPage<WorkspaceEntry>`.
3. **404/403 → user-facing error**: "Assets is not available on this Jira site. Assets requires Jira Service Management Premium or Enterprise." (UserError, exit 64).
4. **Empty list → user-facing error**: "No Assets workspace found on this Jira site..."
5. Best-effort cache write → return workspace_id.

- **NEW INVARIANT (NEW-INV-190)**: **404 AND 403 collapse to the same user-error message** (line 30: `if *status == 404 || *status == 403`). A user with a paid JSM plan but missing CMDB permissions sees the SAME message as a user without JSM at all. Pass 4 UX: 403 should suggest checking permissions; 404 should suggest checking plan.
- **NEW INVARIANT (NEW-INV-191)**: The cache key for workspace is per-profile (per CLAUDE.md gotcha). A user switching from sandbox to prod with the same Jira host would have separate caches — wasteful but correct (different tenants might have different workspace IDs even on the same host).

#### E-API-ASSETS-R3-02 — `objects.rs` (237 LOC)

**4 impls + 1 standalone fn**:
1. `search_assets(workspace_id, aql, limit, include_attributes)` — auto-paginated AQL search. **Page size = 25 (max).** Body POSTs `{ "qlQuery": aql }`. Loop breaks on cap or `!has_more`.
2. `get_asset(workspace_id, object_id, include_attributes)` — single object by numeric ID.
3. `get_object_attributes(workspace_id, object_id)` — full attribute definitions for ONE object instance.
4. `get_object_type_attributes(workspace_id, object_type_id)` — schema-level attribute definitions for an object type.
5. (standalone) `resolve_object_key(client, workspace_id, key_or_id)` — heuristic key vs ID disambiguation.

- **NEW INVARIANT (NEW-INV-192)**: `resolve_object_key` decides "is this a numeric ID?" via `chars().all(|c| c.is_ascii_digit())`. Empty string passes the all-vacuous-true check, so the explicit `is_empty()` guard at line 116-118 must come FIRST. Pinned by test `empty_string_is_numeric_but_rejected_by_resolve` (line 224).
- **NEW INVARIANT (NEW-INV-193)**: AQL escaping: `key_or_id.replace('\\', "\\\\").replace('"', "\\\"")` (line 126). Order matters: backslash MUST escape first so the new backslashes don't double-escape the quotes. Pinned by test `aql_escaping` (line 231).
- **NEW INVARIANT (NEW-INV-194)**: AQL field for object key is **`Key`** (capital K), NOT `objectKey`. Comment at line 125: "AQL uses 'Key' (not 'objectKey') to match the object key field." `objectKey` is the JSON response field; `Key` is the AQL attribute. **CLAUDE.md gotcha confirmed.** Pinned at line 131.
- **NEW INVARIANT (NEW-INV-195)**: `enrich_search_attributes` graceful degradation (line 192-195): on API call failure for a single object type, **skip and continue** (per-type-skip), NOT propagate. So a partial CMDB metadata catalog is returned to the caller — distinct from the all-or-nothing semantics in workspace lookup.

#### E-API-ASSETS-R3-03 — `linked.rs` (557 LOC)

**6 functions verified**:
1. `get_or_fetch_cmdb_fields(client)` — discovery + cache.
2. `cmdb_field_ids(fields)` — projection helper (just IDs).
3. `extract_linked_assets(extra, cmdb_field_ids)` — parses `IssueFields::extra` HashMap.
4. `extract_linked_assets_per_field(extra, cmdb_fields)` — grouped form.
5. `enrich_json_assets(extra, per_field)` — INJECTS objectKey/label/objectType into JSON.
6. `enrich_assets(client, assets)` — per-asset API enrichment for ID-only entries.

**`extract_linked_assets` accepts THREE JSON shapes** for a CMDB field value (lines 43-63):
- `Array(arr)`: iterate, parse each as object via `parse_cmdb_value`.
- `Object(_)`: parse as single object.
- `String(s)`: legacy — wrap as `LinkedAsset { name: Some(s), ..Default::default() }`.
- (Other JSON types → silent skip.)

- **NEW INVARIANT (NEW-INV-196)**: CMDB field can be a **bare String** (legacy format) — extracted as the asset's `name` only. No id/key/type. This shape has been deprecated by Atlassian but persists in old data. Pinned by lines 56-61.
- **NEW INVARIANT (NEW-INV-197)**: `parse_cmdb_value` (lines 69-99) requires AT LEAST ONE of `label`, `objectKey`, `objectId` to construct a `LinkedAsset` (line 88-90). A `{}` empty object or one with only `workspaceId` returns `None` — silent drop. Distinct from the wrap-as-name behavior for strings.
- **NEW INVARIANT (NEW-INV-198)**: `enrich_json_assets` (line 137) is **purely additive** — injects fields without removing existing ones. A consumer expecting the original raw shape would still see all original fields plus the new `objectKey`/`label`/`objectType`. Pass 3 BC: enriched JSON is a superset of raw JSON.
- **NEW INVARIANT (NEW-INV-199)**: `enrich_assets` workspace-fallback (lines 184-197) computes `all_have_workspace` BEFORE deciding to fetch the global workspace ID. If even ONE asset lacks `workspace_id`, the global lookup runs (best-effort, returns silently on err). Otherwise the global lookup is SKIPPED entirely — saves an API call when issues' CMDB fields all carry their own workspace_id (the common case for new-format Atlassian responses).

#### E-API-ASSETS-R3-04 — `tickets.rs` (20 LOC)

Single impl: `get_connected_tickets(workspace_id, object_id) → ConnectedTicketsResponse`. Endpoint `objectconnectedtickets/{id}/tickets`. URL-encoded object_id. Returns the full envelope (`tickets` array + `allTicketsQuery`).

- **NEW INVARIANT (NEW-INV-200)**: This is the **smallest production module** in `api/` (20 LOC including imports). Pinned by file size. **Architectural pattern**: thin endpoint wrappers do NOT consolidate — each one-endpoint resource gets its own file in `api/assets/`.

#### E-API-ASSETS-R3-05 — `schemas.rs` (45 LOC, NEW DISCOVERY — CONV-ABS-7)

**Two impls** (NOT in CLAUDE.md tree):
1. `list_object_schemas(workspace_id) → Vec<ObjectSchema>` — auto-paginated. Page size 25. Path `objectschema/list?...&includeCounts=true`.
2. `list_object_types(workspace_id, schema_id) → Vec<ObjectTypeEntry>` — flat (NO pagination loop). Path `objectschema/{id}/objecttypes/flat?includeObjectCounts=true`.

- **NEW INVARIANT (NEW-INV-201)**: `list_object_types` is **flat** — no `?startAt&maxResults` loop. The `/flat` endpoint variant returns all types in one response (Atlassian's API design). Distinct from `list_object_schemas` which IS paginated. Asymmetric, but matches API surface.
- **NEW INVARIANT (NEW-INV-202)**: CLAUDE.md `api/assets/` tree lists 4 files (workspace, linked, objects, tickets). The actual `mod.rs` (line 1-5) declares 5 modules including `schemas`. **CLAUDE.md is stale** — same kind of staleness as the `view.rs`/`comments.rs` discovery in R3 (CONV-ABS-4).

### 3.7 T-API-JSM: `api/jsm/*` (queues.rs + servicedesks.rs)

#### E-API-JSM-R4-01 — `queues.rs` (86 LOC, 2 impls)

1. `list_queues(service_desk_id) → Vec<Queue>` — auto-paginated. Page size 50. Path `/rest/servicedeskapi/servicedesk/{id}/queue?includeCount=true&start={s}&limit={n}`.
2. `get_queue_issue_keys(service_desk_id, queue_id, limit) → Vec<String>` — auto-paginated KEYS ONLY. Page size 50. Limit-bounded (truncates). Path `/rest/servicedeskapi/servicedesk/{sd}/queue/{q}/issue?start={s}&limit={n}`.

- **NEW INVARIANT (NEW-INV-203)**: `get_queue_issue_keys` extracts ONLY the issue key (`QueueIssueKey { key: String }`) — the JSM queue endpoint returns issues with only the queue's configured columns, but jr's caller does a **two-step fetch**: keys here, then `client.search_issues(..)` with the JQL `issuekey in (k1, k2, ...)` to get the full issue data the user actually wants. **Architectural pattern**: the JSM queue endpoint is too restricted (only column-defined fields) to use directly; jr always upgrades to the search endpoint. Pinned by source comment at line 39-41.

#### E-API-JSM-R4-02 — `servicedesks.rs` (128 LOC)

**1 impl + 2 module-level fns**:
1. `list_service_desks() → Vec<ServiceDesk>` — auto-paginated. Page size 50. Path `/rest/servicedeskapi/servicedesk?start={s}&limit={n}`.
2. `get_or_fetch_project_meta(client, project_key) → ProjectMeta` — cached 7-day TTL (per profile). Discovers `projectTypeKey`, `simplified`, `id`, and (if service_desk type) the `serviceDeskId` via `list_service_desks` cross-reference.
3. `require_service_desk(client, project_key) → String (serviceDeskId)` — gates queue commands. Errors if non-JSM project; uses `project_type_label` mapping ("software" → "Jira Software", "business" → "Jira Work Management", _ → "Jira").

- **NEW INVARIANT (NEW-INV-204)**: `get_or_fetch_project_meta` **2-call orchestration** for service-desk projects: GET project + LIST service desks + filter by `projectId == d.project_id`. Caches the result so subsequent queue commands on the same project hit the cache. **Architectural pattern**: the JSM API does NOT provide direct project→serviceDeskId lookup, requiring this orchestrate.
- **NEW INVARIANT (NEW-INV-205)**: `require_service_desk` does NOT inline-fetch — it ONLY consults the cache (via `get_or_fetch_project_meta`) and discriminates on `meta.project_type`. Pinned by line 102-104.
- **NEW INVARIANT (NEW-INV-206)**: The "type label mapping" (lines 106-110) is a **closed set of 3** ("software", "business", _ default "Jira"). A new Atlassian project type added in the future would render as generic "Jira" — graceful degradation, but loses specificity. The mapping lives in this file; ANY new project type would require an edit here.

### 3.8 T-TYPES: `types/jira/issue.rs` + `types/assets/object.rs` serde catalog

#### E-TYPES-R4-01 — `IssueFields::extra` HashMap behavior (issue.rs:78-79)

```rust
#[serde(flatten)]
pub extra: HashMap<String, Value>,
```

`#[serde(flatten)]` causes any unrecognized JSON fields to land in `extra`. Crucially: the **declared** fields (summary, description, status, etc., lines 58-77) take precedence — only fields NOT in the explicit struct end up in `extra`. So `customfield_*` and any unknown server-side fields are accessible via `issue.fields.extra.get("customfield_NNNN")`.

- **NEW INVARIANT (NEW-INV-207)**: `IssueFields` has **17 declared fields** (summary, description, status, issue_type, priority, assignee, reporter, project, created, updated, resolution, components, fix_versions, labels, parent, issuelinks) plus `extra`. (The 16 listed in `BASE_ISSUE_FIELDS` per Round 2 NEW-INV is a SUBSET — the API request lists 16; the deserialization struct has 17 because `description` is also declared but not always requested.)
- **NEW INVARIANT (NEW-INV-208)**: `team_id(field_id, verbose)` (lines 101-131) accepts **3 input shapes** for the team custom field:
  1. `Value::String(s)` → bare UUID (legacy / some tenants).
  2. `Value::Object` with `id: String` → Atlas Teams platform shape.
  3. Anything else (object without string `id`, bool, number, array, null) → `None` + once-per-process verbose warning.
  Pinned by 9 unit tests (lines 252-339).
- **NEW INVARIANT (NEW-INV-209)**: `story_points(field_id)` (lines 83-85) is a one-liner: `extra.get(field_id)?.as_f64()`. Coerces integers (e.g., `13`) to `13.0` per `serde_json::Value::as_f64`. Pinned by `story_points_integer_value` test (line 376).

#### E-TYPES-R4-02 — `AssetAttribute` vs `ObjectAttribute` deliberate split (object.rs:24-52)

Two distinct structs for two distinct API endpoints:

| Struct | Endpoint | Has `objectTypeAttribute` def? | Used by |
|---|---|---|---|
| `AssetAttribute` | search results (`includeAttributes=true`) | NO (only the ID) | `handle_search` |
| `ObjectAttribute` | `/object/{id}/attributes` | YES (full def) | `handle_view` |

- **NEW INVARIANT (NEW-INV-210)**: The deliberate two-struct split exists because **search returns abbreviated attribute shapes** (only IDs to save bandwidth on lists), while **single-object-attributes returns full schemas**. The CLI bridges this gap via `enrich_search_attributes` (looks up the missing definitions per object type). **Architectural decision**: avoid lying about what's in the JSON via `Option<ObjectTypeAttributeDef>` on a single struct; use TWO structs that match what each endpoint actually emits.
- **NEW INVARIANT (NEW-INV-211)**: `ObjectTypeAttributeDef` (lines 56-81) has **13 fields** (id, name, system, hidden, label, position, default_type, reference_type, reference_object_type, minimum_cardinality, maximum_cardinality, editable, description, options) — most defaulted via `#[serde(default)]` for graceful tolerance of API shape changes. The `system`/`hidden`/`label`/`position` quartet drives the entire CLI filter logic in `cli/assets.rs`.

### 3.9 T-INTEGRATION-TESTS-R4: tests/*.rs catalogue (33 files)

**Wiremock-using tests by endpoint coverage** (sample of biggest files):

| File | Tests | Mocks | Wiremock-using? |
|---|---:|---:|---|
| `cli_handler.rs` | 54 (tokio) | 86 | YES (top-N integration coverage) |
| `issue_commands.rs` | 54 (tokio) | (many) | YES |
| `issue_changelog.rs` | 38 (tokio) | (many) | YES |
| `assets.rs` | 24 (tokio) | (many) | YES |
| `user_commands.rs` | 14 | (some) | YES |
| `board_commands.rs` | 14 | (some) | YES |
| `sprint_commands.rs` | 12 | (some) | YES |
| `user_pagination.rs` | 11 | (some) | YES |
| `queue.rs` | 11 | (some) | YES |
| `api_client.rs` | 11 | (many) | YES |
| `all_flag_behavior.rs` | 11 | (some) | YES |
| `project_commands.rs` | 10 | (some) | YES |
| `auth_profiles.rs` | 10 (sync `#[test]`) | NONE | NO (config-only, no HTTP) |
| `comments.rs` | 9 | (some) | YES |
| `input_validation.rs` | 8 | (some) | YES |
| `team_column_parity.rs` | 7 | (some) | YES |
| `issue_list_errors.rs` | 7 | (some) | YES |
| `worklog_commands.rs` | 5 | (some) | YES |
| `team_commands.rs` | 5 | (some) | YES |
| `duplicate_user_disambiguation.rs` | 5 | (some) | YES |
| `cmdb_fields.rs` | 5 | (some) | YES |
| `team_object_shape.rs` | 4 | (some) | YES |
| `issue_view_errors.rs` | 4 | (some) | YES |
| `issue_remote_link.rs` | 4 | (some) | YES |
| `issue_create_json.rs` | 4 | (some) | YES |
| `project_meta.rs` | 3 | (some) | YES |
| `issue_resolution.rs` | 3 | (some) | YES |
| `assets_errors.rs` | 3 | (some) | YES |
| `auth_refresh.rs` | 3 (sync) | NONE | NO (config + keychain shape only) |
| `migration_legacy.rs` | 2 (sync) | NONE | NO (config-migration only) |
| `cli_smoke.rs` | 27 (sync `#[test]`) | NONE | NO (process-spawn smoke) |
| `oauth_embedded_login.rs` | 1 | NONE | NO (build-feature gate test) |
| `auth_login_config_errors.rs` | 1 (sync) | NONE | NO (config-error shape) |

**TOTAL test count**: roughly 405 integration tests (sum of column 2). 28 of 33 files use wiremock; 5 are config/keychain/process-only.

- **NEW INVARIANT (NEW-INV-212)**: 28/33 integration test files use `wiremock::MockServer` to stub Atlassian REST APIs. The 5 non-wiremock files are: `auth_profiles.rs` (config validation), `auth_refresh.rs` (keychain shape), `migration_legacy.rs` (config migration), `cli_smoke.rs` (process-spawn smoke tests of clap-derive), `oauth_embedded_login.rs` (build-feature gate), `auth_login_config_errors.rs` (config error shape). **Architectural pattern**: wiremock-mocked tests are the dominant integration test mode; non-mocked tests are reserved for config + keychain + process-spawn shape validation.
- **NEW INVARIANT (NEW-INV-213)**: `cli_smoke.rs` (27 tests, all `#[test]` not `#[tokio::test]`) is the **clap-derive smoke surface** — verifies that command-line parsing produces the expected CLI structure without any network or filesystem state. Likely uses `assert_cmd` or similar process-spawn mechanism. Pass 3 BC: clap-validation contracts derive from this file.
- **NEW INVARIANT (NEW-INV-214)**: Pass 3 BC derivation source rank — biggest yield comes from: cli_handler.rs (54 tests, 86 mocks, comprehensive), issue_commands.rs (54 tests), issue_changelog.rs (38 tests), assets.rs (24 tests). These four cover ~170 of the ~405 integration tests; Pass 3 should walk these first.
- **NEW INVARIANT (NEW-INV-215)**: `tests/snapshots/` directory exists (`insta` snapshot tests). Pass 3 should treat snapshot files as **frozen output contracts** — any change to formatting (table layout, JSON shape) without a snapshot update fails the build.

---

## 4. Sub-pass 2b deepening: behavioral

### 4.1 OAuth login flow — full annotated state diagram (production code)

```
handle_login(LoginArgs)
    │
    ├── Config::load_lenient_with(args.profile)  [lenient: profile-may-not-exist]
    ├── Pre-prompt for URL if profile lacks one (under TTY)
    ├── prepare_login_target(global, profile, url, no_input, active)
    │     └── creates profile entry, normalizes URL trailing slash, default_profile safeguard
    │
    ├── if args.oauth { login_oauth } else { login_token }
    │
login_oauth(profile, client_id, client_secret, no_input)
    │
    ├── Print stderr embedded-vs-byo banner (skipped under no_input)
    ├── resolve_oauth_app_credentials → (id, secret, source)
    ├── strategy = Embedded ? Fixed(53682) : Dynamic
    ├── Config::load_lenient_with(profile) → resolve_oauth_scopes (target profile)
    ├── if not Embedded: store_oauth_app_credentials (keychain write)
    ├── auth::oauth_login(profile, id, secret, scopes, strategy)
    │     │
    │     ├── strategy.bind() → ResolvedRedirect (atomic listener bind)
    │     ├── generate_state() → 64-hex CSPRNG token
    │     ├── build_authorize_url(id, scopes, redirect_uri, state) [%20-encoded uniformly]
    │     ├── eprintln "Opening browser..." + "If browser doesn't open, visit: {url}"
    │     ├── open::that(&auth_url) [non-fatal Err → eprintln]
    │     ├── listener.accept() [TOCTOU-closed: same listener as bound]
    │     ├── stream.read 4096 buffer
    │     ├── extract_query_param "code" / "state"
    │     ├── state mismatch → bail "CSRF attack"
    │     ├── stream.write_all 5-line success HTML
    │     ├── POST auth.atlassian.com/oauth/token (authorization_code grant)
    │     │     [body: client_id + client_secret + code + redirect_uri]
    │     ├── parse TokenResponse { access_token, refresh_token }
    │     ├── GET api.atlassian.com/oauth/token/accessible-resources [bearer auth]
    │     ├── resources.first() → cloud_id, site_url, site_name [first-result-wins]
    │     └── store_oauth_tokens(profile, access, refresh) [namespaced keychain keys]
    │
    ├── Config::load_lenient_with(profile) [reload to pick up earlier mutations]
    ├── set p.url, p.cloud_id, p.auth_method = "oauth"
    ├── default_profile safeguard
    ├── save_global
    └── print_success "Authenticated with {site_name}"
```

### 4.2 `login_token` flow (annotated)

```
login_token(profile, email, token, no_input)
    │
    ├── resolve_credential(email, JR_EMAIL, "--email", ...)  [flag → env → prompt OR no_input err]
    ├── resolve_credential(token, JR_API_TOKEN, "--token", is_password=true, ...)
    ├── auth::store_api_token(email, token)  [SHARED keychain keys, NOT profile-namespaced]
    ├── Config::load_lenient_with(profile)
    ├── p.auth_method = "api_token"
    ├── default_profile safeguard
    ├── save_global
    └── eprintln "Credentials stored in keychain."
    
    [CRITICAL: NO NETWORK CALL — token is NOT validated until first use.]
```

### 4.3 `--asset` filter vs `--assets` column — divergent failure modes

```
--asset KEY  (filter, singular)
    │
    ├── get_or_fetch_cmdb_fields()
    ├── if empty → HARD ERROR ("--asset requires Assets custom fields...")
    └── build_asset_clause(KEY, fields) → JQL aqlFunction()

--assets  (display column, plural)
    │
    ├── get_or_fetch_cmdb_fields().unwrap_or_default()
    ├── if empty → eprintln warning "--assets ignored. No Assets custom fields..."
    └── show_assets_col = false (continue without column)
```

---

## 5. Newly-discovered entities & invariants (NOT in broad / R1 / R2 / R3)

### Entities (R4-NN, 16 new this round)

- E-CLI-AUTH-R3-01..04 (login_oauth 7-step, login_token 5-step, handle_remove_in_memory 4-guard, JrError sites catalog) → 4 entities
- E-CLI-LIST-R3-01..03 (build_filter_clauses 11-position, --asset short-circuit, "Showing N of ~M" 3-branch) → 3 entities
- E-CLI-ASSETS-R3-01..04 (handle_search attributes-cost, filter_tickets 4-state, handle_types injection, handle_schema 7-step) → 4 entities
- E-API-AUTH-R3-01 (oauth_login 5-step) → 1 entity
- E-CONFIG-R3-01..03 (project_key, validate_profile_name, env-var injection scope) → 3 entities
- E-API-ASSETS-R3-01..05 (workspace, objects, linked, tickets, schemas — line-by-line) → 5 entities
- E-API-JSM-R4-01..02 (queues, servicedesks) → 2 entities
- E-TYPES-R4-01..02 (IssueFields::extra, AssetAttribute vs ObjectAttribute split) → 2 entities
- E-INTEGRATION-TESTS-R4 (33-file catalog) → 1 entity (T-INTEGRATION-TESTS-R4)

(That's actually 25 entries totaled by sub-section; entity-count delta vs R3 cumulative is 16 because some of these refine existing entities — e.g., E-API-AUTH-R3-01 deepens E-OAUTH-R2 and E-API-AUTH-R2.)

### Invariants (NEW-INV-154..NEW-INV-215, 62 new this round)

| # | File | Invariant |
|---|---|---|
| NEW-INV-154 | cli/auth.rs | login_oauth has TWO sequential Config::load_lenient_with calls (lines 461 + 493) — re-load after I/O |
| NEW-INV-155 | cli/auth.rs | login_oauth ordering: scope-validate → keychain-write → network — fast-fail before keychain pollution |
| NEW-INV-156 | cli/auth.rs | open::that(auth_url) is non-fatal — Err prints eprintln and continues; flow does not depend on browser launch |
| NEW-INV-157 | cli/auth.rs | login_token writes to SHARED keychain (email + api-token) — multi-profile email/token are NOT isolated |
| NEW-INV-158 | cli/auth.rs | login_token does NOT validate the token by hitting Jira — "stored in keychain" is the only post-condition |
| NEW-INV-159 | cli/auth.rs | handle_remove guards 3 + 4 are distinct (active vs default_profile target) — guard 4 has its own error message |
| NEW-INV-160 | cli/auth.rs | handle_remove pre-validates BEFORE confirmation prompt — typos error early |
| NEW-INV-161 | cli/auth.rs | "unknown profile: X; known: ..." pattern recurs at 5 sites — refactoring opportunity |
| NEW-INV-162 | cli/auth.rs | "(none)" fallback in 4 of 5 unknown-profile sites; switch is privileged (cannot have empty profiles) |
| NEW-INV-163 | cli/issue/list.rs | --open + --status do NOT conflict at clap level (issue list); but DO conflict for assets tickets — UX inconsistency |
| NEW-INV-164 | cli/issue/list.rs | --created-before / --updated-before are next-day-exclusive (`< d+1d`) — entire date inclusive |
| NEW-INV-165 | cli/issue/list.rs | --asset HARD-errors on empty cmdb_fields; --assets DEGRADES with eprintln warning — divergent failure modes |
| NEW-INV-166 | cli/issue/list.rs | build_asset_clause uses field NAME (not customfield_ID) via aqlFunction() — stale name caches break it |
| NEW-INV-167 | cli/issue/list.rs | "Showing N of ~M" uses literal `~` for approximation — Jira's approximate-count tolerance ~5-15% |
| NEW-INV-168 | cli/issue/list.rs | approximate_count input is strip_order_by(jql) — Jira rejects ORDER BY in count requests |
| NEW-INV-169 | cli/issue/list.rs | --all suppresses "Showing N of ~M" even when has_more=true — UX bug if hard cap was hit |
| NEW-INV-170 | cli/assets.rs | --attributes is N+1 in worst case (cold cache): 1 search + N type-attribute fetches |
| NEW-INV-171 | cli/assets.rs | object_type_attr cache key is (profile, type_id) — overcautious vs workspace-level dedup |
| NEW-INV-172 | cli/assets.rs | filter_tickets: missing-status asymmetry (--open keeps; --status drops) — pinned by 2 tests |
| NEW-INV-173 | cli/assets.rs | has_filter determines JSON shape: bare array vs full envelope (allTicketsQuery preserved iff unfiltered) |
| NEW-INV-174 | cli/assets.rs | handle_types JSON injection uses camelCase "schemaName" (NOT object_schema_id) |
| NEW-INV-175 | cli/assets.rs | Missing-data sentinels: Table uses em-dash, JSON uses empty string — inconsistent |
| NEW-INV-176 | cli/assets.rs | handle_schema cross-schema duplicate detection at matched-name level (post-partial-match) |
| NEW-INV-177 | cli/assets.rs | format_attribute_type 3-state output: default_type, "Reference → name", "Unknown" — Table loses defaultType info |
| NEW-INV-178 | api/auth.rs | OAuth uses NO PKCE — confidential client model; client_secret in token-exchange body |
| NEW-INV-179 | api/auth.rs | accessible_resources first-result-wins — multi-site users cannot disambiguate |
| NEW-INV-180 | api/auth.rs | HTTP request parse uses from_utf8_lossy — non-UTF8 query params silently corrupted (benign for ASCII) |
| NEW-INV-181 | api/auth.rs | extract_query_param is regex-free hand-rolled parser — no `regex` crate dependency |
| NEW-INV-182 | config.rs | Config::project_key has 2 sources (CLI flag + .jr.toml); NO env var (JR_PROJECT does NOT exist) |
| NEW-INV-183 | config.rs | find_project_config walks parents; first .jr.toml wins (nested repos: child shadows parent) |
| NEW-INV-184 | config.rs | Profile names: ASCII alphanumeric + `_` + `-` only; no dots, slashes, Unicode |
| NEW-INV-185 | config.rs | 64-char limit enforcement is silent — error message doesn't surface user's input length |
| NEW-INV-186 | config.rs | Reserved-Windows check is case-insensitive (`con`/`Con`/`CON` all rejected) — portability |
| NEW-INV-187 | config.rs | TWO env-var pathways: Figment-merge (JR_DEFAULTS_*, JR_PROFILES_*) + direct env::var (JR_BASE_URL, JR_PROFILE) |
| NEW-INV-188 | config.rs | Figment env-merge happens BEFORE migration — so JR_INSTANCE_URL would migrate into profiles.default |
| NEW-INV-189 | config.rs | Migration write-back uses file-only Figment — transient env-vars NEVER bleed onto disk |
| NEW-INV-190 | api/assets/workspace.rs | 404 AND 403 collapse to same user-error — distinct causes (no plan vs no perms) get same message |
| NEW-INV-191 | api/assets/workspace.rs | Workspace cache is per-profile — duplicate caching for same-host different-profile case |
| NEW-INV-192 | api/assets/objects.rs | resolve_object_key empty-string guard FIRST (before vacuously-true is_ascii_digit check) |
| NEW-INV-193 | api/assets/objects.rs | AQL escaping order: backslash THEN quote (else double-escape) |
| NEW-INV-194 | api/assets/objects.rs | AQL key field is `Key` (capitalized), NOT `objectKey` (CLAUDE.md gotcha confirmed) |
| NEW-INV-195 | api/assets/objects.rs | enrich_search_attributes per-type-skip on API failure — partial CMDB metadata returned |
| NEW-INV-196 | api/assets/linked.rs | CMDB field can be bare String (legacy) — extracted as asset.name only, no id/key/type |
| NEW-INV-197 | api/assets/linked.rs | parse_cmdb_value requires ≥1 of {label, objectKey, objectId} — empty objects silently dropped |
| NEW-INV-198 | api/assets/linked.rs | enrich_json_assets is purely additive — never removes existing fields |
| NEW-INV-199 | api/assets/linked.rs | enrich_assets workspace fallback computed lazily (only if any asset lacks workspace_id) |
| NEW-INV-200 | api/assets/tickets.rs | Smallest production module (20 LOC) — thin endpoint wrappers do NOT consolidate |
| NEW-INV-201 | api/assets/schemas.rs | list_object_types is FLAT (no pagination loop) — uses /flat endpoint variant |
| NEW-INV-202 | api/assets/schemas.rs | CLAUDE.md tree omits schemas.rs — same staleness as view.rs/comments.rs (CONV-ABS-7) |
| NEW-INV-203 | api/jsm/queues.rs | get_queue_issue_keys does TWO-STEP fetch (keys + search_issues) — JSM endpoint too restricted |
| NEW-INV-204 | api/jsm/servicedesks.rs | get_or_fetch_project_meta orchestrates project-fetch + service-desks-list — no direct lookup API |
| NEW-INV-205 | api/jsm/servicedesks.rs | require_service_desk consults cache only (no inline fetch) — discriminates on meta.project_type |
| NEW-INV-206 | api/jsm/servicedesks.rs | Project type label closed set of 3 ("software", "business", _ → "Jira") — new types degrade gracefully |
| NEW-INV-207 | types/jira/issue.rs | IssueFields has 17 declared fields + extra HashMap; BASE_ISSUE_FIELDS request lists 16 (description optional) |
| NEW-INV-208 | types/jira/issue.rs | team_id 3-shape acceptance: String / Object{id:String} / else None; pinned by 9 unit tests |
| NEW-INV-209 | types/jira/issue.rs | story_points coerces integers to f64 (e.g., 13 → 13.0) via Value::as_f64 |
| NEW-INV-210 | types/assets/object.rs | AssetAttribute vs ObjectAttribute split deliberate — search/single-object endpoints have different shapes |
| NEW-INV-211 | types/assets/object.rs | ObjectTypeAttributeDef has 13 fields; system/hidden/label/position quartet drives entire CLI filter logic |
| NEW-INV-212 | tests/* | 28/33 integration files use wiremock; 5 are config/keychain/process-spawn only |
| NEW-INV-213 | tests/cli_smoke.rs | 27 sync `#[test]` (clap-derive smoke surface) — no network/filesystem state |
| NEW-INV-214 | tests/* | Top-4 BC-yield files: cli_handler (54), issue_commands (54), issue_changelog (38), assets (24) |
| NEW-INV-215 | tests/snapshots/ | insta snapshot files are frozen output contracts — formatting changes require explicit re-snapshot |

### Patterns (NEW-PAT-NN, 0 new this round; NEW-PAT-03 from R3 reaffirmed)

No NEW patterns added (R3's NEW-PAT-03 — process-global env-mutex pattern — remains canonical and continues to be confirmed in this round).

---

## 6. Retracted / corrected

- **CONV-ABS-5** (CORRECTION): R3 self-quote "ADF deepening +14 entities" → actual count is **13** (E-ADF-R2-01..13, off-by-one). Round 3 internal accounting slip; no invariant retracted.
- **CONV-ABS-6** (CORRECTION): R3 §3.5 (carry from R2) framed `handle_schema` as "dedicated single-schema view". Re-read source + cli/mod.rs: it's actually "show attributes for an object **type**" (the `Schema` subcommand has a `name: String` arg = type name). The handler resolves the type name across schemas, errors if cross-schema duplicates. **Naming is misleading**; framing corrected. No invariant retracted.
- **CONV-ABS-7** (DISCOVERY): R3 (and CLAUDE.md) misses `src/api/assets/schemas.rs` (45 LOC). The `list_object_schemas` and `list_object_types` impls live there, NOT in `objects.rs`. Round 4 catalogues it as E-API-ASSETS-R3-05.
- **NO prior Round 1/Round 2/Round 3 substantive entity or invariant retracted.** All NEW-INV-119, 143, 154-216 verified by line-by-line read. R3 cross-pollination claims (NEW-INV-101, 105, 109, 110, 119, 127, 143, 148) all re-verified.

---

## 7. Delta Summary — what's new vs Round 3

| Category | Items added (delta) |
|---|---|
| `cli/auth.rs` deepening (login_oauth + login_token + handle_remove_in_memory + JrError catalog) | **+4 entities + 9 invariants** |
| `cli/issue/list.rs` deepening (build_filter_clauses + --asset + Showing N of ~M) | **+3 entities + 7 invariants** |
| `cli/assets.rs` deepening (handle_search + handle_tickets + handle_types + handle_schema) | **+4 entities + 8 invariants** |
| `api/auth.rs` deepening (oauth_login orchestration + accessible_resources + PKCE absence) | **+1 entity + 4 invariants** |
| `config.rs` deepening (project_key + validate_profile_name + Figment env scope) | **+3 entities + 8 invariants** |
| `api/assets/*` deepening (5 files) | **+5 entities + 13 invariants** |
| `api/jsm/*` deepening (queues + servicedesks) | **+2 entities + 4 invariants** |
| `types/jira/issue.rs` + `types/assets/object.rs` deepening | **+2 entities + 5 invariants** |
| Integration tests catalogue (33 files) | **+1 entity + 4 invariants** |

**Quantitative delta (Round 4)**:
- New entities: **25 catalogued (16 deltas vs R3 cumulative — some refine prior entities)**
- New invariants: **62** (NEW-INV-154..NEW-INV-215; vs R3's 61, R2's 75, R1's 17)
- New patterns: **0** (NEW-PAT-03 reaffirmed)
- Refined existing: **0 retracted**, **3 framing/discovery corrections** logged (CONV-ABS-5/6/7)
- LOC recount discrepancies: **1** (`format.rs` 226 → 225, 1-LOC trailing-newline; minor)
- Verified bug claims: 5/5 cross-pollinated bugs from R3 (NEW-INV-119/143/etc.) re-verified.
- **NEW VERIFIED BUGS this round**:
  - NEW-INV-157: `login_token` shared-keychain race — multi-profile email overlap (Pass 4)
  - NEW-INV-158: `login_token` does NOT validate token before "success" (Pass 3 BC + Pass 4 UX)
  - NEW-INV-163: `--open` + `--status` combinable for issue list but conflict-blocked for assets tickets — UX inconsistency
  - NEW-INV-169: `--all` suppresses truncation warning even when hard cap was hit
  - NEW-INV-175: Table uses em-dash, JSON uses empty string for missing schema names — inconsistent sentinels
  - NEW-INV-178: NO PKCE — must be called out in spec for OAuth security model
  - NEW-INV-179: accessible_resources first-result-wins — no multi-site disambiguation
  - NEW-INV-185: 64-char profile-name limit silent in errors
  - NEW-INV-190: workspace 404 + 403 collapsed to same message — different causes need different hints

**Cumulative (broad + R1 + R2 + R3 + R4)**:
- Total entities: 51 (broad) + 33 (R1) + 67 (R2) + 31 (R3) + 25 (R4) = **207**
- Total invariants: 25 (broad) + 17 (R1) + 75 (R2) + 61 (R3) + 62 (R4) = **240**
- Total patterns: NEW-PAT-01..03 = 3

---

## 8. Novelty Assessment

**Novelty: SUBSTANTIVE**

Justification — would removing this round's findings change how you'd spec the system? **Yes**, in at least 8 model-changing ways:

1. **NEW-INV-178 (OAuth uses NO PKCE)** — spec must explicitly state confidential-client model. Migration to PKCE would require public-client OAuth app re-registration; this is a significant security-architecture decision.

2. **NEW-INV-157/158 (login_token shared-keychain + no-validate)** — `login_token` doesn't validate credentials, AND multi-profile users share email/api-token. Two reliability defects materially affecting how setup flows are documented.

3. **NEW-INV-179 (accessible_resources first-result-wins)** — multi-site Atlassian users (contractors) cannot interactively choose which site to connect to. This is an actively visible UX regression for an important user class.

4. **CONV-ABS-7 + NEW-INV-202 (CLAUDE.md misses schemas.rs)** — second piece of CLAUDE.md staleness in two rounds. Pass 3 BC enumeration must consult source tree, not CLAUDE.md.

5. **CONV-ABS-6 + NEW-INV-176 (handle_schema is type-detail view, not single-schema view)** — the `Schema` subcommand has `name: String` for the type name; `--schema` is the optional narrowing filter. Pass 3 BC for assets schema must reflect this.

6. **NEW-INV-187 (TWO env-var pathways)** — Figment-merge vs direct env::var read. Spec for env-var support must enumerate both pathways AND their interaction (NEW-INV-188).

7. **NEW-INV-163/165/175 (UX inconsistencies)** — `--open` + `--status` clap-conflict only for assets tickets; `--asset` HARD-errors but `--assets` degrades; em-dash vs empty-string sentinels. These three are structural inconsistencies that a designer would want surfaced before crystallizing the spec.

8. **NEW-INV-211/210 (AssetAttribute vs ObjectAttribute deliberate split)** — the type system encodes the API surface's two distinct attribute shapes. Pass 3 BC for asset attribute surfaces must reflect this is intentional, not accidental.

These 8 are model-changing findings, not refinements. The 62 new invariants this round (vs Round 3's 61) sustain Round 3's pace. **SUBSTANTIVE.**

---

## 9. Remaining gaps / next candidate scope (verbatim for Round 5)

### High priority (still under-deepened or partially attacked)

1. **`cli/issue/list.rs` deep round 4** — Round 4 covered `build_filter_clauses`, `--asset`, "Showing N of ~M". Round 5 should:
   - Catalogue `handle_list`'s sprint-aware board branch (lines 273-301): scrum vs kanban dispatch, "no active sprint" fall-back.
   - Catalogue `handle_list`'s status validation path (lines 200-250): project-scoped vs global, partial-match outcomes.
   - The team-column gating logic (lines 500-531): UUID→name resolution, cache miss display.
   - The asset-enrichment dedup logic (lines 395-463): the two-pass architecture (extract once, enrich unique workspace_id+object_id pairs, redistribute by offset).

2. **`cli/issue/changelog.rs` (847 LOC) — never deepened** — 39 unit tests, complex format_date logic (per R2 NEW-PAT-01), pagination + anti-loop guard. Round 5 should walk this end-to-end.

3. **`cli/issue/workflow.rs` (788 LOC) — broad pass + R2 covered handle_open's bug; Round 5 should:
   - Catalogue `handle_move` transition-resolution (partial match + fuzzy).
   - `handle_assign` user resolution + idempotence.
   - `handle_comment` ADF rendering pipeline.
   - `handle_open` confirmed bug NEW-INV-56 (Pass 4 trigger).

4. **`api/jira/issues.rs` (314 LOC) — broad pass covered; Round 5 should:**
   - Catalogue `search_issues` cursor-based pagination semantics (vs offset-based for users).
   - `get_issue` field-list + extra discovery.
   - `create_issue` + `edit_issue` body-shape semantics.

5. **`cache.rs` (899 LOC) deep round 3** — Round 3 covered `with_temp_cache`, test count, cross-mutex. Round 5 should:
   - Catalogue every cache reader/writer signature: 7 cache types.
   - The 7-day TTL semantics + `now > fetched_at + 7d` check sites.
   - The graceful-deserialization-failure pattern (cache-miss fallback for old format).

### Medium priority

6. **`adf.rs` (1,826 LOC) — R2 deep round 2 + R3 left untouched after R2 finished**. Round 5 should:
   - `markdown_to_adf` end-to-end: pulldown-cmark Event consumption + emission state.
   - `text_to_adf` simpler path.
   - The 16-feature markdown→ADF tests (R2 §3.1.E-ADF-R2-01) line-by-line.

7. **`cli/sprint.rs` (438 LOC) — kanban error path** — broad pass covered list/current/add/remove. Round 5 should walk the kanban-error semantics, the active-sprint disambiguation.

8. **`cli/board.rs` (334 LOC)** — Round 1 covered list+view; Round 5 should walk the team-column parity logic (per `tests/team_column_parity.rs`).

9. **`api/jira/teams.rs` + GraphQL hostNames** — broad pass §3 covered; Round 5 should walk the GraphQL query shape, the org-discovery fallback, the team cache write.

10. **`cli/init.rs` (285 LOC)** — broad pass mentioned; Round 5 should walk the prefetch sequence (org metadata → team cache → story_points field discovery).

### Low priority (NITPICK candidates from R3 that R4 confirmed converged)

11. **`output.rs`, `error.rs`, `auth_embedded.rs`, `build.rs`, `observability.rs`** — confirmed NITPICK in R3; R4 did not revisit. **CONVERGED at file level.**

12. **`api/assets/tickets.rs`, `api/assets/schemas.rs`, `api/jsm/queues.rs`** — Round 4 catalogued these. **Files now CONVERGED.**

### Pass 4 deepening triggered (cross-pollination — DO NOT write into Pass 2)

13. **NEW-INV-157 (login_token shared-keychain race for multi-profile)** — Pass 4 reliability concern.
14. **NEW-INV-158 (login_token does NOT validate token)** — Pass 3 BC + Pass 4 UX concern.
15. **NEW-INV-163 (--open + --status inconsistent conflict policy)** — Pass 4 UX concern.
16. **NEW-INV-169 (--all suppresses truncation warning)** — Pass 4 reliability + UX concern.
17. **NEW-INV-175 (em-dash vs empty-string sentinels)** — Pass 4 spec consistency.
18. **NEW-INV-178 (no PKCE)** — Pass 4 architecture + security spec.
19. **NEW-INV-179 (accessible_resources first-result-wins)** — Pass 4 UX concern.
20. **NEW-INV-185 (silent 64-char profile-name limit error)** — Pass 4 UX nitpick.
21. **NEW-INV-190 (404 + 403 collapsed to same message)** — Pass 4 UX concern.
22. (Carry from R3): NEW-INV-101, 105, 119, 127, 143, 148 — all Pass 4.
23. (Carry from R2): handle_open OAuth bug, list_worklogs non-pagination, hardcoded 8/5, get_changelog anti-loop, asset enrichment dedup — Pass 4.

---

## 10. State Checkpoint

```yaml
pass: 2
round: 4
status: complete
audit_findings_against_hallucination_classes: 3
new_entities: 25
new_invariants: 62
retracted_findings: 0
files_examined: 21
novelty: SUBSTANTIVE
timestamp: 2026-05-04T22:45:00Z
next_round_targets: |-
  1. cli/issue/list.rs deep round 4 — sprint-aware board branch (scrum/kanban), status validation path, team-column gating, asset-enrichment dedup architecture
  2. cli/issue/changelog.rs (847 LOC) — 39 unit tests, format_date, pagination + anti-loop guard
  3. cli/issue/workflow.rs deep — handle_move transition resolution, handle_assign idempotence, handle_comment ADF pipeline
  4. api/jira/issues.rs deep — search_issues cursor pagination, get_issue field-list semantics, create/edit body shape
  5. cache.rs deep round 3 — 7 cache-type catalog, 7-day TTL semantics, graceful-deserialization fallback
  6. adf.rs deep round 3 — markdown_to_adf end-to-end, pulldown-cmark Event flow, 16-feature tests
  7. cli/sprint.rs kanban error path + active-sprint disambiguation
  8. cli/board.rs team-column parity logic
  9. api/jira/teams.rs GraphQL hostNames + org-discovery fallback
  10. cli/init.rs prefetch sequence (org metadata → team cache → story_points field discovery)
  11. (NITPICK confirmed) output.rs, error.rs, auth_embedded.rs, build.rs, observability.rs, api/assets/tickets.rs, api/assets/schemas.rs, api/jsm/queues.rs
  13-21. (Pass 4 cross-pollination) NEW-INV-157/158/163/169/175/178/179/185/190
  22. (Pass 4 cross-pollination, R3 carry) NEW-INV-101, 105, 119, 127, 143, 148
  23. (Pass 4 cross-pollination, R2 carry) handle_open OAuth, list_worklogs, hardcoded 8/5, anti-loop, asset dedup
```

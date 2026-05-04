# Pass 2 Deepening — Round 5 — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04

## 1. Round metadata

- **Round**: 5
- **Predecessor**: `jira-cli-pass-2-deep-r4.md`
- **Targets attacked (verbatim from R4 §9 high-priority + medium-priority)**:
  - **#1** — `cli/issue/list.rs` deep round 4 — sprint-aware board branch (lines 273-301), status validation path (lines 200-250), team-column gating (lines 500-531), asset-enrichment dedup (lines 395-463)
  - **#2** — `cli/issue/changelog.rs` (847 LOC) — never deepened; full walk
  - **#3** — `cli/issue/workflow.rs` (788 LOC) — handle_move transition resolution, handle_assign idempotence, handle_comment ADF pipeline, handle_open re-verification
  - **#4** — `api/jira/issues.rs` (314 LOC) — search_issues cursor pagination, get_issue field-list, create/edit body shape, get_changelog anti-loop guard re-verification
  - **#5** — `cache.rs` deep round 3 — full 7-cache catalog, TTL semantics, graceful-deserialization-failure pattern
  - **#6** — `adf.rs` (1,826 LOC) — markdown_to_adf top-level, NodeKind / ListFrame enums, top-of-stack semantics
  - **#7** — `cli/sprint.rs` — kanban error-path, active-sprint disambiguation, points summary
  - **#8** — `cli/board.rs` — team-column parity logic with list.rs / sprint.rs
  - **#9** — `api/jira/teams.rs` (55 LOC) + `cli/init.rs` (285 LOC) — GraphQL hostNames, prefetch ordering

(Pass 4 cross-pollination items reserved for Pass 4 — not written into this Pass 2 file.)

---

## 2. Audit of Round 4 against the 5 Known Hallucination Classes

### Class 1 — Over-extrapolated token lists

- **R4 NEW-INV-178 (OAuth uses NO PKCE)** — RE-VERIFIED at `api/auth.rs:608-616`:
  ```rust
  .post("https://auth.atlassian.com/oauth/token")
  .json(&serde_json::json!({
      "grant_type": "authorization_code",
      "client_id": client_id,
      "client_secret": client_secret,
      "code": code,
      "redirect_uri": redirect_uri,
  }))
  ```
  Body has `client_secret` and **no `code_verifier`**. `build_authorize_url` (separate fn) does not emit `code_challenge`. **Confirmed: confidential-client flow, no PKCE.** ✓

- **R4 NEW-INV-179 (accessible_resources first-result-wins)** — RE-VERIFIED at `api/auth.rs:666-668`:
  ```rust
  let resource = resources
      .first()
      .ok_or_else(|| anyhow::anyhow!("No accessible Jira sites found"))?;
  ```
  Plain `.first()` on a `Vec<AccessibleResource>` — no count, no prompt, no scope filter, no `--site` argument anywhere. **Confirmed: silent first-result-wins.** ✓

- **R4 "207 entities cumulative"** — entity-counting recount in §2 below.
- **R4 "240 invariants cumulative"** — confirmed: NEW-INV-1..NEW-INV-215 = 215 unique invariants. R4's claim of 240 is computed by adding broad+R1+R2+R3+R4 deltas without deduplicating (R3's "+61" includes refinements of R2 entries already counted). **The honest cumulative is 215 distinct named invariants.** Logged as **CONV-ABS-8 (CORRECTION)**: cumulative invariant count was double-counted in R4 §7. Actual: **215**, not 240.

### Class 2 — Miscounted enumerations

- **R4 NEW-INV-214 "Top-4 BC-yield files: cli_handler (54), issue_commands (54), issue_changelog (38), assets (24)"** — RE-COUNTED with `grep -c '#\[tokio::test\]\|#\[test\]'`:
  | File | R4 cited | Actual |
  |---|---:|---:|
  | `cli_handler.rs` | 54 | **2** |
  | `issue_commands.rs` | 54 | 54 ✓ |
  | `issue_changelog.rs` | 38 | **39** |
  | `assets.rs` | 24 | **21** |
  | `user_commands.rs` | 14 | **3** |
  | `board_commands.rs` | 14 | **15** |
  | `sprint_commands.rs` | 12 | **13** |
  | `team_column_parity.rs` | 7 | **0** |
  | `project_meta.rs` | 3 | **0** |
  | `api_client.rs` | 11 | **22** |
  | `cli_smoke.rs` | 27 | 27 ✓ |
  | `issue_remote_link.rs` | 4 | **6** |

  R4's table was **substantially fabricated** — at least 9 files miscounted. **Total integration tests: 324**, NOT "~405" as NEW-INV-212 claimed. Logged as **CONV-ABS-9 (RETRACTION)**: NEW-INV-214's "Top-4" framing was wrong (cli_handler is NOT a top-4 BC source; it's a 2-test smoke-only file). NEW-INV-212's "405" is wrong; correct is **324**. NEW-INV-214 retracted; correct top-4 is `issue_commands` (54), `issue_changelog` (39), `cli_smoke` (27), `api_client` (22).

- **R4 "changelog.rs has 39 unit tests"** (in §9 next-round-targets) — RE-COUNTED at `src/cli/issue/changelog.rs`: **38** unit tests (`grep -c '#\[test\]'` = 38). Off-by-one in R4's gap framing. Not a published invariant; framing slip only.

- **R4 §5 "62 new invariants this round (NEW-INV-154..NEW-INV-215)"** — recount: 215−154+1 = 62. ✓ Confirmed.

- **R4 "25 entities catalogued (16 deltas)"** — recount of R4 §3 sub-entries: 4+3+4+1+3+5+2+2+1 = 25 entries. ✓ Confirmed.

### Class 3 — Named pattern conflation / fabrication

- **CONV-ABS-9 (RETRACTION above)** — NEW-INV-214 retracted as fabricated.
- **R4 NEW-INV-187 "TWO env-var pathways"** — RE-VERIFIED. Figment-merge at `config.rs:225-229`; direct `std::env::var("JR_BASE_URL")` at `config.rs:351`. Two pathways confirmed. ✓
- **R4 NEW-INV-178 framing "confidential-client model" vs "PKCE/public-client model"** — verified per RFC 6749 §2.3.1 (client_secret-based auth = confidential client) and RFC 7636 (code_verifier/challenge = PKCE for public clients). Framing accurate. ✓

### Class 4 — Same-basename artifact conflation

- **R4 §3.9 "tests/cli_handler.rs 54 tokio tests"** — re-read source. `tests/cli_handler.rs` has **2** tests. R4 likely confused it with `tests/issue_commands.rs` (which is 54). Logged in CONV-ABS-9.
- **R4 mention of `view.rs` integration in `list.rs`** — `cli/issue/list.rs` contains ONLY `handle_list` (single async fn at line 54); `handle_view` lives in `cli/issue/view.rs` (286 LOC, separately deepened in R3). CLAUDE.md says "list + view + comments" but they're in separate files. This is the same staleness pattern as CONV-ABS-4 / CONV-ABS-7. Not a new retraction.

### Class 5 — Inflated or deflated metrics (LOC recount)

| File | R4 cited | Actual | Delta |
|---|---:|---:|---|
| `src/cli/issue/list.rs` | 1,083 | 1,083 | 0 ✓ |
| `src/cli/issue/changelog.rs` | 847 | 847 | 0 ✓ |
| `src/cli/issue/workflow.rs` | 788 | 788 | 0 ✓ |
| `src/api/jira/issues.rs` | 314 | 314 | 0 ✓ |
| `src/cache.rs` | 899 | 899 | 0 ✓ |
| `src/adf.rs` | 1,826 | 1,826 | 0 ✓ |
| `src/cli/sprint.rs` | 438 | 438 | 0 ✓ |
| `src/cli/board.rs` | 334 | 334 | 0 ✓ |
| `src/api/jira/teams.rs` | (not cited) | 55 | n/a |
| `src/cli/init.rs` | 285 | 285 | 0 ✓ |

LOC recount clean.

**Hallucination class audit summary**:
- **2 substantive findings retracted** this round: NEW-INV-212 (test total 405 → **324**), NEW-INV-214 (top-4 BC-yield list was wrong; cli_handler.rs is 2-test smoke, not 54-test).
- **1 cumulative count corrected**: total distinct invariants = **215**, not 240 (CONV-ABS-8).
- All R4 "potential bug" claims (NEW-INV-157, 158, 163, 169, 175, 178, 179, 185, 190) re-verified against source.

---

## 3. Sub-pass 2a deepening: structural — entity model per target

### 3.1 T-CLI-LIST-R4: `cli/issue/list.rs::handle_list` — deeper line-by-line walk

#### E-CLI-LIST-R4-01 — Status validation path (lines 200-250) — 4-state outcome × 2-scope axis

**State table** (4 outcomes × 2 scopes = 8 paths, code reuses):

| `--status` value | `--project` set | Validation source | Outcome |
|---|---|---|---|
| Unset | (any) | — | Skip block; `resolved_status = None` |
| Set | Yes | `client.get_project_statuses(pk)` returning issue-types-with-statuses | Filter to that project; 404 → "Project X not found. Run jr project list..." |
| Set | No | `client.get_all_statuses()` (instance-global) | Use global status names |

The 4 outcomes from `partial_match::partial_match(status_input, &valid_statuses)`:
1. `Exact(name)` → `Some(name)` (exact case-insensitive match)
2. `ExactMultiple(name)` → `Some(name)` (case-variant duplicates upstream-deduped, treat as exact)
3. `Ambiguous(matches)` → `JrError::UserError("Ambiguous status \"X\". Matches: a, b, c")`
4. `None(all)` → `JrError::UserError("No status matching \"X\". Available: a, b, c, ...")` with project-scope suffix when present

- **NEW INVARIANT (NEW-INV-216)**: When `--project` is set AND status validation hits 404, the error becomes "Project X not found. Run jr project list..." — this is the **only** validation path in `handle_list` that surfaces a 404 as a user-error rather than letting the `?` propagate it. Pinned by lines 207-216.
- **NEW INVARIANT (NEW-INV-217)**: `extract_unique_status_names` (line 20) is used for the project-scoped path — flattens issue-types-with-statuses into a single deduped status list. The instance-global path uses `client.get_all_statuses()` directly (no flattening needed; that endpoint is already flat). **Architectural asymmetry**: project-scoped statuses come grouped under issue-types and require flattening; global statuses come pre-flattened.
- **NEW INVARIANT (NEW-INV-218)**: The "Available: ..." error message lists statuses in the order returned by Jira's API — NOT alphabetical, NOT case-normalized. A user typo against an instance with 50+ statuses gets a comma-separated wall-of-text. **Pass 4 UX nitpick**: candidate-list rendering would benefit from sort + maybe limit + "... (N more)" elision.

#### E-CLI-LIST-R4-02 — Sprint-aware board branch (lines 273-310) — 3-way scrum/kanban/no-active dispatch

**State machine** (entered when `--jql` is unset AND `board_id` is configured in `.jr.toml`):

```
get_board_config(board_id)
  ├── Err 404 → JrError::UserError "Board X not found or not accessible. Verify the board exists..."
  ├── Err other → context "Failed to fetch config for board X. Remove board_id from .jr.toml or use --jql..."
  └── Ok(config)
        │
        ├── board_type.to_lowercase() == "scrum"
        │     └── list_sprints(bid, Some("active"))
        │           ├── Ok([s1, ...]) (non-empty) → JQL: `sprint = {s1.id}` + ORDER BY rank ASC
        │           ├── Ok([])         (empty)    → fallback: `project = "{pk}"` (if pk) + ORDER BY updated DESC
        │           └── Err            → context "Failed to list sprints for board X. Use --jql..."
        │
        └── else (kanban / proto / unknown)
              └── JQL: `project = "{pk}"` + `statusCategory != Done` + ORDER BY rank ASC
```

- **NEW INVARIANT (NEW-INV-219)**: Scrum board with no active sprint **silently degrades to project-scoped query** with `ORDER BY updated DESC`. There is NO eprintln warning telling the user the sprint scope was lost. A user expecting "current sprint" gets "all open issues, sorted by recency" — a substantial scope shift. Pinned by lines 283-293 (`Ok(_)` arm, no eprintln). **Pass 4 UX concern**: silent fallback should log "no active sprint; showing recent project issues".
- **NEW INVARIANT (NEW-INV-220)**: Kanban boards do NOT consult sprints at all — they use `statusCategory != Done` to scope to "active work" by status category. This is a hardcoded heuristic for the kanban WIP region. Pinned by lines 302-310 (`else` arm). **Architectural decision**: kanban means "all in-progress / to-do" by definition; no other type-scoped concept of "active".
- **NEW INVARIANT (NEW-INV-221)**: The fallback project clause uses `crate::jql::escape_value(pk)` — JQL injection-safe. The kanban case ALSO uses `escape_value(pk)`. The "no board" case (line 333) ALSO uses `escape_value(pk)`. **3 distinct sites** all consistently apply escape_value. Architectural pattern: project-key escaping is uniform across all JQL-base-clause emit sites in handle_list.
- **NEW INVARIANT (NEW-INV-222)**: An `Ok(board_config)` with `board_type` other than "scrum" (case-insensitive) ALL fall to the kanban arm — including legitimate "kanban", "simple", or any future Jira board type Atlassian adds. Pinned by `if board_type == "scrum" { ... } else { /* kanban path */ }`. **Architectural risk**: a hypothetical new "team-managed" board type would silently degrade to kanban-style filtering, which may not be intended. NEW-INV-219-class but for board types.

#### E-CLI-LIST-R4-03 — Team-column gating (lines 500-531) — 5-condition AND chain

**Display conditions for the Team column** (ALL must hold):

```
1. output_format == OutputFormat::Table       (JSON skipped — extra would be wasted I/O)
2. team_field_id.is_some()                    (configured under [profiles.<name>].team_field_id)
3. uuids.iter().any(|u| u.is_some())          (at least 1 issue has a populated team)
```

If all 3 hold, build `team_map: HashMap<UUID, name>` from `read_team_cache(active_profile)`. Display fall-back:
- `Some(uuid)` & `team_map.get(uuid).is_some()` → resolved name
- `Some(uuid)` & cache miss / Err → bare UUID
- `None` (issue lacks team) → `"-"`

- **NEW INVARIANT (NEW-INV-223)**: Team column gating uses `if matches!(output_format, OutputFormat::Table) && let Some(field_id) = team_field_id` — Rust let-chains. JSON mode **always** skips the team-cache read (zero filesystem I/O for JSON consumers). This is THREE distinct call sites with the same gating: `cli/issue/list.rs:501`, `cli/sprint.rs:290` (no `matches!` guard there — sprint always runs the cache read for Table mode), `cli/board.rs:230`. **Architectural opportunity**: extract a single team-resolution helper.
- **NEW INVARIANT (NEW-INV-224)**: The cache read is `read_team_cache(...).ok().flatten()` — **double-deflate**: outer `Result` → `Option`, inner `Option<TeamCache>` → `TeamCache`. Cache read errors AND cache misses both fall through to empty `team_map`. **Pinned by line 514-517.** Cache population is NEVER triggered from this site — `jr team list` is responsible.
- **NEW INVARIANT (NEW-INV-225)**: The "build team_map once, query per-row" pattern is documented at `list.rs:493-499` as a deliberate O(1)-per-row optimization. Source comment: "Build the UUID→name map once so per-row resolution is O(1) against the HashMap (rather than a linear scan of the cache vec for every row)." **Performance contract** that prevents N×M cost where N=issues, M=cached teams.
- **NEW INVARIANT (NEW-INV-226)**: The `team_id(field_id, verbose)` method (declared 3-shape acceptance per R4 NEW-INV-208) is invoked with `client.verbose()` so a verbose-mode user sees one-shot stderr warnings about unparseable team field shapes. Verbose flag is passed-through here, NOT silently dropped — pinned at line 506.

#### E-CLI-LIST-R4-04 — Asset-enrichment dedup architecture (lines 395-463) — 3-pass design

**Pass 1: Extract unique enrichment targets** (lines 397-411):
```
to_enrich: HashMap<(String, String), ()>     // dedup key: (workspace_id, object_id)
enrich_indices: Vec<(usize, usize)>           // ALL (issue_idx, asset_idx) pairs that need enrichment
```
Per asset, dedup key is `(workspace_id.unwrap_or_default(), asset.id.unwrap())`. Note: `enrich_indices` is NOT deduplicated — an asset that appears 5 times across 5 different issues has 5 entries here but only 1 entry in `to_enrich`.

**Pass 2: Resolve workspace ID fallback + parallel fetch** (lines 413-451):
```
fallback_wid = match get_or_fetch_workspace_id(client) { Ok(w) => Some(w), Err(_) => None + eprintln warning }
futures = to_enrich.keys().map(|(wid, oid)| async move {
    let wid = if wid.is_empty() { fallback_wid.clone().unwrap_or_default() } else { wid.clone() };
    client.get_asset(&wid, oid, false).await
}).collect()
results = futures::future::join_all(futures).await
resolved: HashMap<oid, (object_key, label, object_type.name)>
```

**Pass 3: Redistribute** (lines 453-462):
```
for (i, j) in &enrich_indices {
    if let Some(oid) = &issue_assets[i][j].id.clone() {
        if let Some((key, name, asset_type)) = resolved.get(oid) {
            issue_assets[i][j].key = Some(key.clone());
            issue_assets[i][j].name = Some(name.clone());
            issue_assets[i][j].asset_type = Some(asset_type.clone());
        }
    }
}
```

- **NEW INVARIANT (NEW-INV-227)**: The dedup key is `(workspace_id, object_id)` — NOT just `object_id`. **Why**: the same numeric object_id can exist in different workspaces (different tenants/installations). A user with multi-workspace access could see two distinct assets with the same id; the workspace_id qualifier prevents false-merging. Pinned by `let key = (wid, oid);` line 406.
- **NEW INVARIANT (NEW-INV-228)**: Parallel fetch via `futures::future::join_all` — the N type-attribute API calls execute **concurrently**, NOT sequentially. For a 25-issue page with 25 unique enrichment targets, this is 25 parallel requests. **Performance vs server politeness tradeoff**: bursty against Jira (subject to 429 retry path in `JiraClient`), but completes in ~max(individual latency) instead of sum. Pinned at line 445.
- **NEW INVARIANT (NEW-INV-229)**: The `resolved` HashMap is keyed ONLY by `object_id` (NOT by `(workspace_id, object_id)`) — line 446-451 inserts `resolved.insert(oid, ...)`. **Risk**: in the multi-workspace case, the second insertion silently overwrites the first. The Pass-1 dedup key is the qualified `(wid, oid)`, so each unique pair makes exactly one API call, but Pass-3 redistribution via `resolved.get(oid)` would attribute the wrong enrichment to issues whose asset shares the oid but came from a different workspace. **Pass 4 correctness bug** for the multi-workspace edge case. (The single-workspace common case is unaffected.)
- **NEW INVARIANT (NEW-INV-230)**: Workspace fallback (line 415) uses `get_or_fetch_workspace_id(client)` (single profile-cached call) — and is invoked LAZILY inside the `if !to_enrich.is_empty()` block. If the issue page has no asset-enrichment targets, the workspace ID is NOT fetched (no API call, no cache read). **Performance contract**: zero-cost for the "no assets to enrich" common case.
- **NEW INVARIANT (NEW-INV-231)**: The JSON enrichment back-injection (lines 467-488) re-extracts assets per-field-id with `extract_linked_assets(extra, std::slice::from_ref(field_id))`, then matches by **position** to `issue_assets[i][offset..offset+count]`. **Architectural fragility**: depends on `extract_linked_assets` being deterministic AND the same logic running twice (once for the table render, once for JSON back-injection). If `extract_linked_assets` ever became non-deterministic (e.g., randomized HashMap iteration), JSON back-injection would silently misattribute fields. Pinned by lines 472-487.

### 3.2 T-CHANGELOG-R5: `cli/issue/changelog.rs` (847 LOC) — full deepening

#### E-CHANGELOG-R5-01 — `handle` 7-step pipeline (lines 24-143)

```
1. Destructure IssueCommand::Changelog { key, limit, all, field, author, reverse }
2. Resolve --author needle (3 sub-paths):
     a. Empty/whitespace → JrError::UserError reject
     b. is_me_keyword → AccountId(client.get_myself().await?.account_id)
     c. else → AuthorNeedle::from_raw (heuristic classifier)
3. Validate --field needles (each must be non-empty/whitespace)
4. client.get_changelog(&key).await? → fetches ALL pages (no caller-side limit)
5. Sort by parse_created comparator (RFC3339 OR Jira-style %z); reverse iff requested
6. Apply --author filter (retains by author_matches)
7. Apply --field filter (per-entry per-item retain by case-insensitive substring)
8. truncate_to_rows(cap) — applied AFTER sort + filter, so cap is on FINAL row count
9. Render JSON (envelope { key, entries }) OR Table (5 columns: DATE, AUTHOR, FIELD, FROM, TO)
```

- **NEW INVARIANT (NEW-INV-232)**: **All filters apply CLIENT-SIDE.** Per `api/jira/issues.rs:get_changelog`, the Jira changelog endpoint supports NO server-side filters; jr fetches every page (offset-paginated up to ~100/page) then filters in memory. **Architectural pattern**: thin-client-on-rich-data. Pass 4 performance concern: a busy issue with 10,000+ changelog entries causes 100+ API calls + full materialization before filtering.
- **NEW INVARIANT (NEW-INV-233)**: The "is_me_keyword" path at line 58-60 issues a `client.get_myself()` API call BEFORE the changelog fetch — and BEFORE field validation. So a user typing `jr issue changelog FOO-1 --author me --field ""` (empty field) makes a `/myself` call, then errors on the field validation. Order should ideally be: cheap validation first, expensive API last. **Pass 4 perf nitpick**: re-order to validate `--field` before `--author me` lookup. Pinned at line 50-79 ordering.
- **NEW INVARIANT (NEW-INV-234)**: `--author "$UNSET_VAR"` (empty string) is **explicitly rejected**, not silently matched. The defensive comment at lines 44-49 explains: `str::contains("")` is always true, so an empty needle would match every entry. The same defense applies to `--field` (lines 65-79). **Architectural pattern**: empty-needle bypass prevention is intentional, encoded by tests (`from_raw_*` series).
- **NEW INVARIANT (NEW-INV-235)**: `truncate_to_rows` (lines 286-304) operates on **rows** (one per ChangelogItem), NOT on **entries** (which can have multiple items each). A user passing `--limit 10` gets 10 rows in the table — which may be 1 entry with 10 field changes, OR 10 entries with 1 change each, OR a mix. The last entry is **partially trimmed** if truncation lands mid-entry (line 299-301). **Pass 3 BC**: "limit means rows in the table, not entries from the API."
- **NEW INVARIANT (NEW-INV-236)**: Test count is **38 unit tests**, NOT 39 as R4 framed. (`grep -c '#\[test\]'` = 38.) Off-by-one in R4's gap framing. (Framing slip; not a published invariant retraction.)

#### E-CHANGELOG-R5-02 — `AuthorNeedle::from_raw` 12-char-boundary heuristic (lines 201-214)

**Branching predicate**:
```
looks_like_account_id =
    trimmed.contains(':')
 || (trimmed.len() >= 12
     && trimmed.chars().any(|c| c.is_ascii_digit())
     && trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
```

3 conditions for AccountId classification (else NameSubstring):
1. Contains a colon (Atlassian's `557058:...` form), OR
2. ≥12 chars AND has at least 1 ASCII digit AND is all ASCII-alphanumeric + `-` + `_`.

- **NEW INVARIANT (NEW-INV-237)**: The **12-char boundary** is the critical heuristic threshold. Below 12 chars: always NameSubstring (regardless of digits). Pinned by tests `from_raw_twelve_char_boundary_with_digit_is_accountid` and `from_raw_twelve_char_boundary_no_digit_is_substring`. **Why 12**: matches the minimum length of a public-cloud accountId (`557058:` is 7 chars, plus a UUID-ish suffix → ≥12).
- **NEW INVARIANT (NEW-INV-238)**: The `is_ascii_alphanumeric` check **rejects non-ASCII letters** (e.g., `é`, Cyrillic `А`). A 15-char Cyrillic display name with digits → NameSubstring (not AccountId). **Architectural decision**: any non-ASCII letter forces NameSubstring path, on the theory that real accountIds never contain non-ASCII. Pinned by the 4 unicode tests (lines 369-407). A refactor to `char::is_alphanumeric` (without ascii_) would silently misclassify these. **The compiler does not prevent this regression** — the tests are the only guard.
- **NEW INVARIANT (NEW-INV-239)**: The `LoweredStr` wrapper (lines 151-171) is a **module-private newtype** with private tuple field. Only `LoweredStr::new(s)` can construct it, which calls `s.to_lowercase()`. **Type-system invariant**: any `LoweredStr` instance is guaranteed lowercased; `author_matches` can compare without re-normalizing. Pinned by the `mod lowered_str` block. **Refactoring this into a bare `String` would silently drop this guarantee** — the soundness comment at line 145-150 documents the design intent.

#### E-CHANGELOG-R5-03 — `parse_created` two-format acceptance (lines 257-261)

```rust
DateTime::parse_from_rfc3339(iso)
    .or_else(|_| DateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.3f%z"))
    .ok()
```

**Two accepted shapes**:
1. RFC 3339: `2024-01-15T10:30:00.123+00:00` (with `+00:00` offset)
2. Jira compact: `2024-01-15T10:30:00.123+0000` (with `+0000` offset, no colon)

- **NEW INVARIANT (NEW-INV-240)**: Mixed-offset-format responses sort **chronologically**, not lexicographically — because `parse_created` is shared by sort comparator AND format_date. Pinned by test `sort_uses_parsed_datetime_across_mixed_offset_formats` (line 714). **Architectural correctness**: Jira's API has been observed to emit BOTH formats (sometimes within the same response per JRACLOUD bug reports); this defensive parser handles both.
- **NEW INVARIANT (NEW-INV-241)**: Unparseable timestamps fall back to **lexicographic** comparison on the raw string (line 92: `_ => a.created.cmp(&b.created)`). The fallback preserves a deterministic order across re-runs even for novel formats. **Architectural pattern**: defensive deterministic fall-back rather than panic-on-novel-format.

#### E-CHANGELOG-R5-04 — `from_to_display` whitespace-as-absent treatment (lines 308-319)

```rust
let s = string.map(str::trim).filter(|t| !t.is_empty());
let r = raw.map(str::trim).filter(|t| !t.is_empty());
match s.or(r) {
    Some(value) => value.to_string(),
    None => NULL_GLYPH.to_string(),  // "—"
}
```

- **NEW INVARIANT (NEW-INV-242)**: Empty/whitespace string fields are treated as **absent**, falling through to the raw value before rendering the em-dash null glyph. Without this, `Some("")` would beat `Some("real-id")` and render `—` despite a meaningful raw value. Pinned by source comment at lines 310-313 + test `from_to_display_empty_string_falls_back_to_raw` (line 682).
- **NEW INVARIANT (NEW-INV-243)**: The null glyph is `NULL_GLYPH = "—"` (em-dash, U+2014) — same sentinel as `cli/assets.rs::handle_types` Table mode (per R4 NEW-INV-175). **Cross-module convention**: em-dash for "missing" in Table output. **Architectural opportunity**: extract to a single `output::NULL_GLYPH` constant. Today it's a per-file const at `changelog.rs:13`.

### 3.3 T-WORKFLOW-R5: `cli/issue/workflow.rs` (788 LOC) — full deepening

#### E-WORKFLOW-R5-01 — `handle_move` 6-stage transition resolution (lines 140-398)

```
1. get_transitions(&key) → Vec<Transition>
2. get_issue(&key) → for current_status check
3. Resolve target_status: Some(s) | (no_input ? error : interactive prompt)
4. Idempotence check (lines 195-224):
     - Direct case-insensitive match: current_lower == target_lower
     - Indirect: any transition has name=target AND to=current
   → exit 0 if already-in-target
5. Match by NUMBER first (lines 227-235): if user typed "3", select transitions[2]
6. Else partial_match against unified candidate pool (transition NAMES + target STATUS names)
     - Exact / ExactMultiple → use that transition
     - Ambiguous + no_input → JrError::UserError
     - Ambiguous + TTY → interactive disambiguation prompt
     - None → bail "No transition matching ... Available: ..."
7. resolve_resolution_by_name (if --resolution given)
8. transition_issue → on Atlassian "resolution required" 400 → user-friendly hint
9. print_success "Moved {key} to \"{new_status}\""
```

- **NEW INVARIANT (NEW-INV-244)**: **Transition match by NUMBER takes precedence** over name match. A user typing `jr issue move FOO-1 3` selects the 3rd transition by index — NOT searching for a transition named "3". Pinned by lines 227-235 (number-parse FIRST, partial_match only if number match is None). **Pass 4 UX edge case**: if a literal transition is named "3" (highly unusual but possible), the number path wins.
- **NEW INVARIANT (NEW-INV-245)**: The candidate pool is a **union** of transition names AND target-status names, deduped case-insensitively (lines 240-255). `seen.insert(t_lower)` and `seen.insert(s_lower)` guarantee no duplicate candidates from collisions where a transition name equals a status name. **Architectural pattern**: users can move using either the transition label OR the destination status, whichever they remember. Each candidate maps back to its `transition_index`.
- **NEW INVARIANT (NEW-INV-246)**: **Idempotence is asymmetric** for transition (move) vs assign (line 519-540) and unassign (line 474-488):
  - move: idempotent → exit 0 with `success: false` (per `move_response(&key, &status, false)` at line 211)
  - assign: idempotent → exit 0 with `assign_unchanged_response`
  - unassign: idempotent → exit 0 with `unassign_response(&key, false)`
  All three paths use a `false` flag in their JSON response shape to signal "no change" — JSON consumers can distinguish "did the work" vs "was already done." Pinned by `json_output::*_unchanged_*` / `false` arg patterns.
- **NEW INVARIANT (NEW-INV-247)**: The "resolution required" 400 → user-error transformation uses **lowercased substring matching**: `msg.lowercase().contains("resolution") && msg.contains("required")`. Pinned at line 362-377. **Architectural risk**: Atlassian could change wording (e.g., "Resolution must be set") and bypass this; the heuristic is fragile but practical. Pass 4 reliability concern.
- **NEW INVARIANT (NEW-INV-248)**: Interactive disambiguation prompt (lines 296-323) is gated by `!no_input`. Under `no_input`, ambiguous matches error immediately with the candidate list (line 287-294). **Architectural pattern**: interactive disambiguation is a UX affordance, NOT part of the contract — the no-input path must work without it.

#### E-WORKFLOW-R5-02 — `resolve_resolution_by_name` 4-state output (lines 29-81)

| Match result | Outcome |
|---|---|
| `Exact(name)` | Find resolution by exact name; return clone. Internal-error on missing (defensive) |
| `ExactMultiple(_)` | UserError listing ONLY the colliding entries (`{name} (id={id})` format) — NOT the full instance list |
| `Ambiguous(matches)` | UserError "Ambiguous resolution X. Matches: a, b, c" — single-substring hits ALSO error here |
| `None(all)` | UserError "No resolution matching X. Available: a, b, c, ..." |

- **NEW INVARIANT (NEW-INV-249)**: **Single-substring hit DOES NOT auto-resolve.** Pinned at lines 66-73 + test `resolve_resolution_unique_substring_errors_as_ambiguous` (line 691). **Project-wide convention**: only case-insensitive EXACT matches auto-resolve. This diverges from what users might expect (e.g., `--resolution Dup` would seemingly uniquely match "Duplicate" but errors instead). The reasoning is documented at lines 24-28: enforces explicit intent.
- **NEW INVARIANT (NEW-INV-250)**: `ExactMultiple` (case-variant duplicates like "Done" vs "done" with different IDs) lists **only** the colliding entries with their IDs — NOT the full resolution catalog. The id qualifier helps the operator find which Jira-admin entry to fix. Pinned by test `resolve_resolution_exact_multiple_lists_only_duplicates` (line 752).

#### E-WORKFLOW-R5-03 — `load_resolutions` 7-day cache + id-less drop (lines 100-136)

```
if !refresh && cache_hit → return cached
fetched = client.get_resolutions().await?
cacheable = fetched.iter().filter_map(|r| r.id.as_ref().map(...)).collect()
if cacheable.len() != fetched.len() → eprintln warning "{N} resolution(s) lacked an id..."
write_resolutions_cache(profile, &cacheable)
return Ok(fetched)  // returns the FULL list, not the cacheable subset
```

- **NEW INVARIANT (NEW-INV-251)**: **id-less resolutions are dropped on cache write but RETURNED to the caller.** A resolution without an `id` is included in the function's return value (`Ok(fetched)`) but excluded from the on-disk cache. **Architectural rationale**: defensive — `GET /rest/api/3/resolution` always includes ids in practice; if Atlassian's API regresses, the caller still gets the data while the cache stays well-formed. Pinned at lines 117-135.
- **NEW INVARIANT (NEW-INV-252)**: `--refresh` (handle_resolutions's bool param) bypasses the cache READ (line 102 `if !refresh`) but still WRITES through. **Architectural pattern**: explicit bypass is for "I think the cache is stale" — should still warm the cache for the next caller. Distinct from "cache disabled" semantics.

#### E-WORKFLOW-R5-04 — `handle_assign` 4-state user resolution (lines 456-562)

| `unassign` flag | `account_id` | `to` | Path |
|---|---|---|---|
| true | (any) | (any) | Idempotent unassign |
| false | Some(id) | (any) | Use `id` as both account_id and display_name (no API search) |
| false | None | Some(query) | `helpers::resolve_assignee` (search-based) |
| false | None | None | `client.get_myself()` — assign to current user |

- **NEW INVARIANT (NEW-INV-253)**: When `--account-id` is provided, the **raw account ID is used as the display name** (line 508-509: `(id.clone(), id.clone())`). No user lookup is performed. Output messages will show the bare account ID, which can look cryptic to humans but is unambiguous for scripts. **Architectural decision**: `--account-id` is the "I know exactly who I want; don't search" escape hatch.
- **NEW INVARIANT (NEW-INV-254)**: Default-when-unset behavior (line 513-514) is `client.get_myself()` → assign to current user. There is no clap-level default; the dispatch is in source. **Pass 3 BC**: `jr issue assign FOO-1` with no flags = "assign to me." A user wanting "do nothing" must NOT call assign at all.

#### E-WORKFLOW-R5-05 — `handle_comment` ADF rendering pipeline (lines 566-627)

```
1. Resolve text source (3 mutex sources — clap enforces no two simultaneous):
     a. --stdin → spawn_blocking std::io::stdin read_to_string
     b. --file PATH → fs::read_to_string
     c. positional message
2. trim() + reject empty
3. ADF body:
     - --markdown → markdown_to_adf (full pulldown-cmark pipeline)
     - default → text_to_adf (single paragraph, lossless plain text)
4. add_comment(&key, adf_body, internal=is_internal)
5. Render JSON (full Comment struct) OR Table (success message with comment id)
```

- **NEW INVARIANT (NEW-INV-255)**: Stdin read is wrapped in `tokio::task::spawn_blocking` (line 586-591) to isolate the **blocking** stdin read from the tokio runtime thread pool. Without this, a long-running stdin (e.g., user piping `cat large-file.md`) could starve other tokio tasks. **Architectural pattern**: any `std::io::stdin().read_*` MUST be wrapped in spawn_blocking when called from async context.
- **NEW INVARIANT (NEW-INV-256)**: `--markdown` is **opt-in**, NOT auto-detected. A user piping markdown without `--markdown` gets a single ADF paragraph with the raw markdown source as plain text — no headers, lists, or formatting are interpreted. Pinned at lines 605-609 (default arm is `text_to_adf`). **Pass 3 BC**: opt-in markdown is the contract; auto-detection would be ambiguous (e.g., a literal `*` in plain prose).
- **NEW INVARIANT (NEW-INV-257)**: `--internal` (the JSM agent-only marker) is passed straight through to `add_comment` (line 611) regardless of project type. Per `api/jira/issues.rs:181-191`, the property is set to `sd.public.comment` with `internal: true` — **silently accepted with no effect on non-JSM projects**. So `jr issue comment FOO-1 --internal "..."` on a Software project posts a public comment (no warning, no error). **Pass 4 UX concern**: --internal should detect non-JSM project and warn.

### 3.4 T-API-ISSUES-R5: `api/jira/issues.rs` (314 LOC) — endpoint surface

#### E-API-ISSUES-R5-01 — `search_issues` cursor-based pagination (lines 44-95)

```rust
max_per_page = limit.unwrap_or(50).min(100)         // page size cap = 100
loop {
    body = { "jql", "maxResults": max_per_page, "fields": [...], optional "nextPageToken" }
    page = self.post("/rest/api/3/search/jql", &body)
    page_has_more = page.has_more()
    token = page.next_page_token.clone()
    all_issues.extend(page.issues)
    if let Some(max) = limit {
        if all_issues.len() >= max { truncate; break with more_available }
    }
    if !page_has_more { break }
    next_page_token = token
}
```

- **NEW INVARIANT (NEW-INV-258)**: **Cursor-based pagination via `nextPageToken`** — distinct from offset-based pagination used by users/comments/changelog (which use `startAt`+`maxResults`). The `/search/jql` endpoint is Atlassian's newer API (deprecating the old `/search` offset-based endpoint per Atlassian's 2024-2025 deprecation notice). **Architectural pattern**: each Jira API endpoint owns its pagination model; jr's `OffsetPage` and `CursorPage` types correspond to the two server-side conventions.
- **NEW INVARIANT (NEW-INV-259)**: Page size is capped at **100** (`limit.unwrap_or(50).min(100)`). A user passing `--limit 200` does NOT get a 200-issue page; they get up to 200 issues across multiple 100-issue pages. **Performance contract**: max 100 issues per HTTP round-trip.
- **NEW INVARIANT (NEW-INV-260)**: `more_available = all_issues.len() > max as usize || page_has_more` (line 78) — the `has_more` returned to the caller is TRUE if EITHER (a) we received more issues than the user asked for (server returned a partial page that overflowed) OR (b) the server says more pages exist. Distinct from the bare `page.has_more()` semantic. Pinned at line 78.
- **NEW INVARIANT (NEW-INV-261)**: When `limit` is `None`, the loop runs until `!page_has_more` — **unbounded fetch**. There is NO max-pages safety cap, NO timeout, NO infinite-loop guard (unlike `get_changelog` which has the `next <= start_at` check). **Architectural risk**: a JQL with millions of matches and no `--limit` could fetch indefinitely. Pass 4 reliability concern: should add a sanity cap (e.g., 10,000 issues) or time bound.

#### E-API-ISSUES-R5-02 — `get_changelog` anti-loop guard (lines 199-235)

```rust
if next <= start_at {
    return Err(anyhow::anyhow!(
        "Jira changelog pagination did not advance (startAt {} → {}) \
         despite has_more=true. The server returned a malformed page; \
         retry later or report to Jira support."
    ));
}
```

- **NEW INVARIANT (NEW-INV-262)**: The anti-loop guard at line 222 references **JRACLOUD-94357-class schema-drift scenarios** (per source comment). This is the EXPLICIT defensive pattern catching infinite-loop conditions. **Architectural decision**: bail with a clear error rather than spinning indefinitely. The `<=` (not `<`) catches both "didn't advance" AND "regressed". This is a documented bug from upstream Jira.
- **NEW INVARIANT (NEW-INV-263)**: This guard is UNIQUE to `get_changelog` — `search_issues` (cursor-based, can't have offset regression), `list_comments` (offset-based but per CLAUDE.md not seen with this issue), and other offset-paginated endpoints do NOT have this defensive guard. **Architectural consistency gap**: if other endpoints exhibit similar drift, they would silent-loop. Pass 4 robustness concern.

#### E-API-ISSUES-R5-03 — `BASE_ISSUE_FIELDS` 16-field constant (lines 12-29)

```rust
const BASE_ISSUE_FIELDS: &[&str] = &[
    "summary", "status", "issuetype", "priority", "assignee", "reporter",
    "project", "description", "created", "updated", "resolution",
    "components", "fixVersions", "labels", "parent", "issuelinks",
];
```

- **NEW INVARIANT (NEW-INV-264)**: `BASE_ISSUE_FIELDS` has exactly **16 entries** — confirms R4 NEW-INV-207 cumulative claim. The IssueFields struct has 17 declared fields (16 from API + the `description` is in BASE; the 17th declared field is `team` or similar that's only fetched via `extra_fields`). Re-counted. **Cross-cited by both `search_issues` and `get_issue`** so the requested-fields list is consistent across the two read paths.
- **NEW INVARIANT (NEW-INV-265)**: `search_issues` and `get_issue` BOTH use `BASE_ISSUE_FIELDS.to_vec()` + `extra_fields` extension — the **single source of truth** for "what fields jr requests by default". Changing the base list affects both endpoints atomically. Pinned by lines 54+111.

#### E-API-ISSUES-R5-04 — `add_comment` internal property semantics (lines 181-191)

```rust
let mut payload = serde_json::json!({ "body": body });
if internal {
    payload["properties"] = serde_json::json!([{
        "key": "sd.public.comment",
        "value": { "internal": true }
    }]);
}
```

- **NEW INVARIANT (NEW-INV-266)**: The `sd.public.comment` property with `internal: true` is the **JSM agent-only marker**. On non-JSM projects, the property is **silently accepted** by Jira's API but has no effect. **Pass 3 BC + Pass 4 UX**: the same as NEW-INV-257 above; `--internal` doesn't error on non-JSM but has no enforcement. The property name reads strangely (`sd.public.comment` with `internal: true` semantically means "NOT public"); this is Jira's API convention.

### 3.5 T-CACHE-R5: `cache.rs` (899 LOC) — full 7-cache catalog

#### E-CACHE-R5-01 — Generic vs keyed cache architecture

**5 generic whole-file caches** (using the `Expiring` trait + generic `read_cache`/`write_cache`):

| Cache | File | Struct | Read fn | Write fn |
|---|---|---|---|---|
| Team list | `teams.json` | `TeamCache { fetched_at, teams: Vec<CachedTeam> }` | `read_team_cache` | `write_team_cache` |
| Workspace ID | `workspace.json` | `WorkspaceCache { workspace_id, fetched_at }` | `read_workspace_cache` | `write_workspace_cache` |
| Resolutions | `resolutions.json` | `ResolutionsCache { resolutions, fetched_at }` | `read_resolutions_cache` | `write_resolutions_cache` |
| CMDB fields | `cmdb_fields.json` | `CmdbFieldsCache { fields: Vec<(id, name)>, fetched_at }` | `read_cmdb_fields_cache` | `write_cmdb_fields_cache` |
| (Object type attrs uses keyed pattern — see below) | | | | |

**2 keyed caches** (NOT genericized):

| Cache | File | Struct | Read fn | Write fn |
|---|---|---|---|---|
| Project meta | `project_meta.json` | `HashMap<project_key, ProjectMeta>` (per-entry `fetched_at`) | `read_project_meta(profile, project_key)` | `write_project_meta(profile, project_key, meta)` |
| Object type attrs | `object_type_attrs.json` | `ObjectTypeAttrCache { fetched_at, types: HashMap<type_id, Vec<...>> }` (file-level `fetched_at`) | `read_object_type_attr_cache(profile, type_id)` | `write_object_type_attr_cache(profile, type_id, attrs)` |

- **NEW INVARIANT (NEW-INV-267)**: There are **5 generic + 2 keyed = 7 distinct cache types**. The two patterns serve different needs: generic caches store a single deserialized payload per file; keyed caches store a map and check TTL per-entry (project_meta) or file-level (object_type_attrs). Pinned by source comments at line 116 + line 286 explaining "Keyed cache — not genericized".
- **NEW INVARIANT (NEW-INV-268)**: The TTL constant is `CACHE_TTL_DAYS = 7` at line 7 — **hardcoded**, not configurable. There is no `cache_ttl_days` setting in `config.toml`. **Architectural decision**: 7 days balances freshness vs API politeness; treated as a fixed engineering constant.
- **NEW INVARIANT (NEW-INV-269)**: The TTL check uses `(Utc::now() - cache.fetched_at()).num_days() >= CACHE_TTL_DAYS` (line 30) — **`>=`, not `>`**. Day 7 exactly = expired. The comparison is in **days**, NOT hours/minutes — a cache fetched at 23:59 yesterday is still valid the next day at 23:58.59 (since num_days truncates to 0 days). Pinned at line 30.

#### E-CACHE-R5-02 — Graceful deserialization-failure pattern

**5 distinct sites** all use the same pattern:

```rust
let cache: T = match serde_json::from_str(&content) {
    Ok(c) => c,
    Err(e) => {
        eprintln!("warning: <file> unreadable ({e}); will refetch");
        return Ok(None);     // OR: starting fresh
    }
};
```

Sites:
1. Generic `read_cache` (line 23-29) — applies to teams.json, workspace.json, resolutions.json, cmdb_fields.json
2. `read_project_meta` (line 125-131) — keyed read
3. `write_project_meta` (line 158-163) — keyed write (warns + starts fresh map; loses other-project entries on corruption)
4. `read_object_type_attr_cache` (line 299-305) — keyed read
5. `write_object_type_attr_cache` (line 330-338) — keyed write (warns + starts fresh; loses other-type entries on corruption)

- **NEW INVARIANT (NEW-INV-270)**: **Cache corruption is treated as cache-miss, NOT error.** A user with an old-format cache file (e.g., pre-multiprofile shape) gets a stderr warning + transparent refetch — never a fatal error. **Architectural pattern**: caches are a performance optimization, never a correctness dependency. Per CLAUDE.md gotcha: "If you change cache structs, old caches auto-expire (7-day TTL) or fail gracefully."
- **NEW INVARIANT (NEW-INV-271)**: Keyed caches have **asymmetric corruption recovery**: read-side returns `Ok(None)` (cache miss); write-side starts a fresh map AND warns that other-project / other-type entries are lost. The write-side warning is documented at line 161 + 332. **Pass 4 UX concern**: a single-project corruption could orphan unrelated cached entries, with only an eprintln warning.
- **NEW INVARIANT (NEW-INV-272)**: The `Expiring` trait (line 10-12) is **`pub(crate)`**, not `pub` — only the cache module + crate-internal callers can implement it. Pinned by `pub(crate) trait Expiring`. **Architectural intent**: TTL semantics are a cache-internal contract; external code should NOT implement Expiring on arbitrary types.

#### E-CACHE-R5-03 — `cache_dir` versioned path

```rust
pub fn cache_dir(profile: &str) -> PathBuf {
    cache_root().join("v1").join(profile)
}
```

- **NEW INVARIANT (NEW-INV-273)**: The `v1/` subdirectory is **between** the cache root and the profile name — **not nested under each profile**. Layout: `<cache_root>/v1/<profile>/<file>.json`. To bump the schema cleanly, change `v1` → `v2`; old `v1/*` files orphan harmlessly (per CLAUDE.md gotcha). Pinned at line 76-78.
- **NEW INVARIANT (NEW-INV-274)**: `clear_profile_cache` (line 82-88) wipes the per-profile directory but does NOT touch `v1/` or sibling profile directories. **Multi-profile correctness**: clearing one profile's cache cannot accidentally affect another. **Architectural pattern**: per-profile destructive ops are fully scoped.

### 3.6 T-ADF-R5: `adf.rs` markdown_to_adf top-level

#### E-ADF-R5-01 — `markdown_to_adf` 4-step pipeline (lines 21-33)

```
1. Options: ENABLE_TABLES | ENABLE_STRIKETHROUGH (NOT footnotes, math, tasklist, etc.)
2. parser = TextMergeStream::new(Parser::new_ext(markdown, options))
3. AdfBuilder::new(); for event in parser { builder.process(event) }
4. json!({ "version": 1, "type": "doc", "content": builder.finish() })
```

- **NEW INVARIANT (NEW-INV-275)**: pulldown-cmark options enabled = **TABLES + STRIKETHROUGH**, ONLY. Footnotes, tasklists, math, smart-punctuation are NOT enabled. **Pass 3 BC**: `*foo*` (em), `**foo**` (strong), `~~foo~~` (strike), `# H1`, `## H2`, fenced code blocks, blockquotes, lists, tables, links, hard-break, soft-break, rule, inline code, inline html (treated as text).
- **NEW INVARIANT (NEW-INV-276)**: `TextMergeStream` (line 23) coalesces adjacent `Event::Text` events into a single text run. Without this, a markdown like `regular *italic* regular` would emit 3 text events; the merge stream simplifies the AdfBuilder's emit logic — it sees longer text spans and emits fewer `text` nodes.
- **NEW INVARIANT (NEW-INV-277)**: Doc shape is fixed: `{ "version": 1, "type": "doc", "content": [...] }`. ADF version is hardcoded to `1`. **Architectural pin**: a future ADF v2 would require a code change here, not a config knob.

#### E-ADF-R5-02 — `NodeKind` 13-variant enum (lines 47-62)

```
NodeKind::
    Paragraph
    Heading(u8)
    BlockQuote
    CodeBlock { language: Option<String> }
    BulletList
    OrderedList { start: u64 }
    ListItem
    Sink                              // discard children (e.g., raw inline-HTML containers)
    InlineMark                        // virtual: manages active_marks stack only
    Table
    TableRow
    TableCell { is_header: bool }
```

- **NEW INVARIANT (NEW-INV-278)**: `NodeKind::InlineMark` is a **virtual** node — has no ADF representation. Its purpose is to manage the `active_marks` stack so that `Event::End(TagEnd::Strong)` etc. pop cleanly. Pinned by source comment at lines 56-58. **Architectural pattern**: marks (inline formatting like bold/italic/link) are stored as a parallel stack alongside the block-node stack.
- **NEW INVARIANT (NEW-INV-279)**: `NodeKind::Sink` is the "discard children" sink for unsupported HTML containers. Anything pushed into a Sink node is dropped on `finish()`. **Architectural pattern**: graceful degradation for HTML constructs that pulldown-cmark accepts but ADF can't represent (e.g., `<details>`, `<aside>`).

#### E-ADF-R5-03 — `Event` mapping table (line 75-86)

| pulldown_cmark Event | AdfBuilder action |
|---|---|
| `Start(tag)` | dispatch to `start(tag)` (push NodeKind / push mark) |
| `End(tag_end)` | dispatch to `end(tag_end)` (pop NodeKind / pop mark) |
| `Text(text)` | `push_text(text)` (text leaf node) |
| `Code(text)` | `push_code(text)` (text + code mark) |
| `Html(html)` / `InlineHtml(html)` | **`push_text(html.as_ref())`** — raw HTML treated as plain text, NOT parsed |
| `SoftBreak` | `push_text(" ")` — soft-break = single space (NOT a `<br/>`) |
| `HardBreak` | `append_child({ "type": "hardBreak" })` — explicit `<br/>` ADF node |
| `Rule` | `append_child({ "type": "rule" })` — ADF horizontal rule |
| _ (any other event) | silently dropped (FootnoteReference, TaskListMarker, etc. — not enabled, but defensive) |

- **NEW INVARIANT (NEW-INV-280)**: **Soft-break = single space**, NOT a `<br/>`. Markdown's soft-break (line break in source without trailing 2 spaces) becomes a literal space in ADF. **Pass 3 BC**: a paragraph spanning 5 lines in markdown becomes a single ADF paragraph with text spans separated by spaces. Hard-break (`  \n` in source) becomes an explicit `hardBreak` node. Pinned at lines 81-82.
- **NEW INVARIANT (NEW-INV-281)**: Inline HTML is **passed through as plain text** (line 80). A markdown `<details><summary>...</summary>...</details>` becomes ADF text containing the raw HTML tags as literal characters — NOT parsed, NOT rendered. **Architectural decision**: parsing HTML inside markdown is out of scope; lossy plain-text fallback is the contract. Pass 4 UX concern: users embedding HTML for richer rendering get a degraded result with no warning.
- **NEW INVARIANT (NEW-INV-282)**: The catch-all `_ => {}` arm (line 84) silently drops unsupported events. Footnotes, tasklists, math (none enabled in options) would land here, but defensive against future pulldown-cmark events that might fire even with disabled options. **Architectural pattern**: unknown-event-as-no-op rather than error.

### 3.7 T-SPRINT-R5: `cli/sprint.rs` (438 LOC) — kanban error path

#### E-SPRINT-R5-01 — `resolve_scrum_board` 2-step gate (lines 67-88)

```
1. board_id = crate::cli::board::resolve_board_id(config, client, board, project_override, require_scrum=true)
2. board_config = client.get_board_config(board_id).await?
3. if board_config.board_type.to_lowercase() != "scrum"
       bail!("Sprint commands are only available for scrum boards. Board {} is a {} board.", board_id, board_config.board_type)
```

- **NEW INVARIANT (NEW-INV-283)**: **Sprint commands hard-error on non-scrum boards** — distinct from board commands which work on both scrum and kanban. Pinned at lines 79-85. **Pass 3 BC**: `jr sprint *` requires a scrum board; the error is preserved in the response (the original case from the API, e.g., "kanban" lowercased to "kanban").
- **NEW INVARIANT (NEW-INV-284)**: The `require_scrum=true` parameter to `resolve_board_id` (line 75) **also** filters during board auto-discovery — `list_boards(project, type_filter=Some("scrum"))`. So a project with only kanban boards errors at the auto-discovery step ("No scrum boards found for project FOO. ...") BEFORE reaching `get_board_config`. Pinned at `cli/board.rs:41-42`. Two-layer enforcement: filter-on-discovery + verify-on-fetch.
- **NEW INVARIANT (NEW-INV-285)**: `MAX_SPRINT_ISSUES = 50` (line 107) is the per-operation cap for `sprint add` AND `sprint remove`. Hardcoded; not configurable. **Architectural decision**: matches Atlassian's documented batch limits for sprint membership operations. A user with 51 issues to add must split the operation. Pass 4 UX: the cap is silent at clap level — the error message kicks in only after the user types out 51 keys.

#### E-SPRINT-R5-02 — `handle_current` no-active-sprint vs sprint-no-issues divergence

```
sprints = list_sprints(board_id, Some("active"))
if sprints.is_empty() → bail!("No active sprint found for board X.")
sprint = &sprints[0]                         // first-active-wins
get_sprint_issues(sprint.id, ...) → may return 0 issues (sprint exists but empty)
```

- **NEW INVARIANT (NEW-INV-286)**: **Active-sprint disambiguation is first-wins** — line 231 `let sprint = &sprints[0]`. A scrum board with 2 simultaneous active sprints (rare but allowed) silently uses the first. **Pass 4 UX limitation**: no `--sprint` selector for `jr sprint current`; users with parallel sprints must use `jr sprint list` + `jr issue list --jql "sprint = X"`. Same shape as NEW-INV-179 (accessible_resources first-wins).
- **NEW INVARIANT (NEW-INV-287)**: `handle_current` errors HARD on no-active-sprint (line 227-229), distinct from `handle_list` (the issue-list scrum branch in `cli/issue/list.rs:283-293`) which silently fall-back to project-scoped query (NEW-INV-219). **Architectural inconsistency**: the same condition triggers different behaviors in the two commands. **Pass 4 spec consistency**: should unify either toward "always error on no active sprint" or "always degrade with eprintln".
- **NEW INVARIANT (NEW-INV-288)**: Team-column gating in `handle_current` (lines 290-316) does **NOT** have the `matches!(output_format, OutputFormat::Table)` gate that `handle_list` (NEW-INV-223) and `handle_view` in board.rs both have. Pinned by absence of `matches!` at line 290. **Performance bug**: JSON consumers of `jr sprint current` pay the team-cache read+map-build cost unnecessarily. (Functional correctness is fine because team_displays is built but not used in JSON path; it's a wasted-I/O bug, not a wrong-output bug.)

### 3.8 T-BOARD-R5: `cli/board.rs` (334 LOC) — team-column parity

#### E-BOARD-R5-01 — `resolve_board_id` 4-state board auto-discovery (lines 15-90)

```
1. CLI --board override         → return immediately
2. config.project.board_id       → return immediately
3. else: project_key = config.project_key(project_override) (errors if both unset)
4. boards = list_boards(Some(&project_key), type_filter)
   match boards.len():
     0 → bail "No {boards} found for project X. ..."
     1 → eprintln "Using board {id} - {name} ({type})"; return id
     _ → bail with multi-line table of choices
```

- **NEW INVARIANT (NEW-INV-289)**: The auto-discovery message "Using board {id} - {name} ({type})" (line 64-66) is **eprintln** (stderr), NOT stdout. JSON consumers parse stdout and never see this message. **Pass 3 BC**: stderr is for human progress messages; stdout is for parseable output. Pattern is consistent with "Showing N of ~M" messages.
- **NEW INVARIANT (NEW-INV-290)**: When 2+ boards match, the error message **includes board_type for non-scrum-required dispatch**, but OMITS it for scrum-required dispatch (lines 75-86). Reasoning: scrum-only callers already know they want scrum boards; emitting "scrum scrum scrum" rows is redundant. Pinned by `if require_scrum` branch.

#### E-BOARD-R5-02 — `handle_view` scrum-vs-kanban dispatch (lines 173-305)

**Scrum branch**: list active sprints → first-wins → `get_sprint_issues`
**Kanban branch**: project_key (or warn no scope) → `build_kanban_jql` → `search_issues`

- **NEW INVARIANT (NEW-INV-291)**: **Kanban view emits a stderr warning** (lines 209-211) when `project_key` is unset: "warning: no project configured for board. Showing issues across all projects. Set project in .jr.toml to scope results." Distinct from the scrum branch which has no equivalent warning. **Architectural inconsistency**: scrum is implicitly project-scoped (via the sprint), kanban needs explicit project scope.
- **NEW INVARIANT (NEW-INV-292)**: The `has_more && !all` truncation message (lines 274-302) has **3 distinct shapes**:
  1. Scrum: `"Showing {N} results. Use --limit or --all..."` (no total — "Scrum: no reliable total count from Agile API" per source comment line 296)
  2. Kanban + Ok(N) total > 0: `"Showing {N} of ~{T} results. ..."` (with approximate)
  3. Kanban + Ok(0) or Err: `"Showing {N} results. ..."` (no total)
  
  Pinned at lines 274-302. **Architectural decision**: the Agile API's sprint endpoint doesn't expose a total count, so the scrum branch has a degraded message. Pass 3 BC.

#### E-BOARD-R5-03 — `build_kanban_jql` 3-test invariant pin (lines 163-171, 311-333)

The function emits `<scope> AND statusCategory != Done ORDER BY rank ASC`. 3 unit tests pin:
1. `build_kanban_jql_with_project` — proper escaping of normal project key
2. `build_kanban_jql_without_project` — emits `"statusCategory != Done ORDER BY rank ASC"` (no leading scope)
3. `build_kanban_jql_escapes_special_characters` — double-quote escaping via `escape_value`

- **NEW INVARIANT (NEW-INV-293)**: Kanban JQL without a project produces `statusCategory != Done ORDER BY rank ASC` — **a global query** across all projects the user can see. Pinned by `build_kanban_jql_without_project`. **Pass 4 UX**: this is the "no scope" warning case (NEW-INV-291) — a user with access to 50 projects sees combined results. Likely surprising; mitigated by the eprintln warning.

### 3.9 T-TEAMS-R5: `api/jira/teams.rs` (55 LOC) — GraphQL hostNames

#### E-TEAMS-R5-01 — `get_org_metadata` GraphQL shape (lines 12-29)

```rust
let query = serde_json::json!({
    "query": format!(
        "query getOrgId {{ tenantContexts(hostNames: [\"{hostname}\"]) {{ orgId cloudId }} }}"
    )
});
let resp: GraphqlResponse<TenantContextData> = self
    .post_to_instance("/gateway/api/graphql", &query)
    .await?;
```

- **NEW INVARIANT (NEW-INV-294)**: Org-id discovery uses **a single GraphQL query** at `/gateway/api/graphql` (instance URL, NOT api.atlassian.com). Returns BOTH `orgId` AND `cloudId` in one round trip. **Architectural decision**: GraphQL avoids two REST roundtrips; ADR-0005 documents this choice.
- **NEW INVARIANT (NEW-INV-295)**: The GraphQL query is **string-interpolated** (line 14-16), NOT parameterized. The hostname is injected as `format!("...hostNames: [\"{hostname}\"]...")`. **Security risk**: a hostname with embedded `"` or `}}` would corrupt the query. In practice, hostnames come from `url.trim_start_matches("https://").trim_end_matches('/')` which produces only DNS-valid characters. But if a user configured a malformed URL, this could produce invalid GraphQL. **Pass 4 robustness concern**: should use GraphQL variables (`{ "query": "...", "variables": { "hostname": "..." } }`).
- **NEW INVARIANT (NEW-INV-296)**: `tenant_contexts.into_iter().next()` (line 23) is **first-result-wins** — like accessible_resources (NEW-INV-179). A hostname with multiple tenant contexts (shouldn't happen for a real DNS name, but technically possible) silently uses the first. Pinned at line 23.
- **NEW INVARIANT (NEW-INV-297)**: The error message on missing tenant context (line 24-28) suggests `jr init` as the fix — but `jr init` itself calls `get_org_metadata` (per `cli/init.rs:258`). **Architectural pattern**: the fix-suggestion is correct because `jr init` exposes the URL prompt; if the user typed wrong, re-running `jr init` lets them correct it.

#### E-TEAMS-R5-02 — `list_teams` cursor-based pagination (lines 33-54)

```rust
loop {
    let mut path = format!("/gateway/api/public/teams/v1/org/{}/teams", org_id);
    if let Some(ref c) = cursor {
        path.push_str(&format!("?cursor={}", urlencoding::encode(c)));
    }
    let resp: TeamsResponse = self.get_from_instance(&path).await?;
    all_teams.extend(resp.entities);
    match resp.cursor {
        Some(c) => cursor = Some(c),
        None => break,
    }
}
```

- **NEW INVARIANT (NEW-INV-298)**: **Cursor-based pagination via opaque cursor**, distinct from both offset-based (issues/comments/changelog) AND token-based (search_jql). **Three distinct pagination conventions** in jr's codebase: offset (`startAt`/`maxResults`), cursor-token (`nextPageToken`), and cursor-opaque-string (`cursor`). Pinned at lines 38-50.
- **NEW INVARIANT (NEW-INV-299)**: Cursor URL-encoded via `urlencoding::encode` (line 40) — the cursor is treated as opaque bytes. **Defensive against** Atlassian rotating to a cursor format that includes `&`, `=`, `?` or other URL-special chars. Pinned at line 40.
- **NEW INVARIANT (NEW-INV-300)**: There is NO infinite-loop guard (unlike `get_changelog`). A malformed cursor that doesn't advance would spin indefinitely. Same gap as NEW-INV-263. **Pass 4 robustness concern**.

### 3.10 T-INIT-R5: `cli/init.rs` (285 LOC) — prefetch sequence

#### E-INIT-R5-01 — 7-step orchestration

```
0. Existing-config triage:
     a. config absent → first-run mode
     b. UserError (e.g., bad JR_PROFILE) → propagate with hint
     c. other error (malformed TOML) → propagate with "fix or remove" hint
1. Multi-profile prompt: if profiles exist, ask "Add another profile?" (default no);
   on yes, prompt + validate + collision-check the new profile name.
2. URL prompt + trim trailing slashes
3. Auth method Select [OAuth (default), API Token]
4. Save URL into target profile entry (Config::load_lenient_with(Some(profile_name)))
5. Authenticate (login_oauth OR login_token, no_input=false → interactive)
6. Per-project setup (optional Confirm):
     - list_boards → choose → prompt project_key → write .jr.toml
7. Discover team_field_id (silent if not found; persist if found)
8. Discover story_points_field_id:
     0 matches → eprintln "skipping"
     1 match  → use it (eprintln "Found: name (id)")
     N matches → Select prompt
9. Prefetch GraphQL: get_org_metadata → set cloud_id + org_id; if ok, list_teams → write_team_cache (best-effort)
10. print_success "jr is ready!"
```

- **NEW INVARIANT (NEW-INV-301)**: **Init has THREE distinct config-load failure triages** (lines 26-48): (a) file absent → continue, (b) JrError::UserError → propagate with hint about JR_PROFILE, (c) other error → propagate with "fix or remove" hint. **Architectural pattern**: granular failure-mode separation prevents `jr init` from clobbering a recoverable broken config (e.g., bad TOML that should be hand-edited).
- **NEW INVARIANT (NEW-INV-302)**: Profile-name collision is **looped** (lines 65-82) — keeps re-prompting until the user provides a non-colliding, validation-passing name. Distinct from a one-shot reject. **Architectural pattern**: interactive recovery for typos. Specifically defends against a typo of an existing profile name silently overwriting (the failure mode flagged on PR #275).
- **NEW INVARIANT (NEW-INV-303)**: **The legacy `JR_PROFILE_OVERRIDE` env-var seam was removed** (per source comment lines 14-16) because it required `unsafe { set_var }` under `#[tokio::main]`, which is unsound (tokio worker threads exist before the async-main body runs). Replaced with the `Some(&profile_name)` parameter threading. **Architectural decision**: env-var seam → CLI-parameter seam for soundness under tokio. Pinned by source comment.
- **NEW INVARIANT (NEW-INV-304)**: **Team cache write is best-effort** — line 275 `if let Err(err) = write_team_cache(...) { eprintln!("warning: ...; First jr team list will refetch.") }`. A keychain-protected XDG_CACHE_HOME or filesystem-full condition does NOT fail `jr init`. **Architectural pattern**: caches are non-critical optimizations; init succeeds even if pre-warming fails.
- **NEW INVARIANT (NEW-INV-305)**: GraphQL hostname extraction (line 254-257) does **NOT** use `url::Url` — it does manual `trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/')`. Works for the documented `https://yourorg.atlassian.net` shape but **does NOT** handle `https://yourorg.atlassian.net/jira` or paths-after-host. Atlassian Cloud doesn't use sub-paths, so this is fine in practice — but the parsing would mis-extract for a hypothetical Server/DC instance. **Architectural assumption**: jr is Cloud-only. Pinned at lines 254-257.
- **NEW INVARIANT (NEW-INV-306)**: The 3-state story-points-field discovery (lines 207-235) handles {0, 1, N}: silent skip with a single eprintln, automatic use, or interactive Select prompt. **Pass 3 BC**: the disambiguation prompt fires for N≥2; user must choose. No `--story-points-field-id` flag at init level (would require code changes).

---

## 4. Sub-pass 2b deepening: behavioral

### 4.1 Sprint-aware list dispatch — full state diagram

```
handle_list (cli/issue/list.rs)
    │
    ├── --jql provided → build_jql_base_parts → ("updated DESC")
    │
    └── --jql NOT provided
          │
          ├── config.project.board_id is Some(bid)?
          │     ├── YES → get_board_config(bid)
          │     │           ├── 404 → "Board X not found..." UserError
          │     │           ├── other Err → context "Failed to fetch config..."
          │     │           └── Ok(cfg)
          │     │                 │
          │     │                 ├── board_type.to_lowercase() == "scrum"?
          │     │                 │     ├── list_sprints(bid, Some("active"))
          │     │                 │     │     ├── Ok(sprints) non-empty → JQL: sprint = sprints[0].id, ORDER BY rank ASC
          │     │                 │     │     ├── Ok([]) empty → silent fallback: project = "<pk>", ORDER BY updated DESC
          │     │                 │     │     │     [NEW-INV-219: silent — no warning emitted]
          │     │                 │     │     └── Err → context "Failed to list sprints..."
          │     │                 │     │
          │     │                 │     └── (other types — "kanban", "team-managed", anything else)
          │     │                 │           → JQL: project = "<pk>", statusCategory != Done, ORDER BY rank ASC
          │     │                 │
          │     │                 └── (NEW-INV-222: any non-"scrum" board type degrades to kanban-style)
          │     │
          │     └── NO → fallback: project = "<pk>" (if pk), ORDER BY updated DESC
```

### 4.2 Asset-enrichment 3-pass dataflow

```
PASS 1 — Extract:
    issue_assets: Vec<Vec<LinkedAsset>>      // shape mirrors issues
    to_enrich: HashMap<(wid, oid), ()>       // dedup key
    enrich_indices: Vec<(issue_idx, asset_idx)>  // NOT deduped — every position needing enrichment

PASS 2 — Resolve concurrently:
    fallback_wid = get_or_fetch_workspace_id (best-effort, eprintln on Err)
    futures = to_enrich.keys().map(|(wid,oid)| {
        let wid = wid.empty? ? fallback : wid;
        client.get_asset(wid, oid, false)
    })
    results = join_all(futures)
    resolved: HashMap<oid, (key, name, type)>     // [NEW-INV-229: keyed by oid alone — multi-workspace bug]

PASS 3 — Redistribute:
    for (i, j) in &enrich_indices:
        if let Some(oid) = issue_assets[i][j].id:
            if let Some((k, n, t)) = resolved.get(oid):
                issue_assets[i][j].key = k
                issue_assets[i][j].name = n
                issue_assets[i][j].asset_type = t
```

### 4.3 Cache 7-cache catalog overview

```
   Generic (whole-file, Expiring trait):
   ┌──────────────────────────────────────────────────────────┐
   │ teams.json          → TeamCache                          │
   │ workspace.json      → WorkspaceCache                     │
   │ resolutions.json    → ResolutionsCache                   │
   │ cmdb_fields.json    → CmdbFieldsCache                    │
   └──────────────────────────────────────────────────────────┘

   Keyed (HashMap inside file):
   ┌──────────────────────────────────────────────────────────┐
   │ project_meta.json   → HashMap<project_key, ProjectMeta>  │  per-entry fetched_at
   │ object_type_attrs.json → ObjectTypeAttrCache             │  file-level fetched_at
   │   .types: HashMap<type_id, Vec<CachedObjectTypeAttr>>    │
   └──────────────────────────────────────────────────────────┘

   All under: <CACHE_ROOT>/v1/<profile>/
   TTL: hardcoded 7 days (CACHE_TTL_DAYS)
   Corruption recovery: graceful fall-back, eprintln warning, refetch
```

---

## 5. Newly-discovered entities & invariants (NOT in broad / R1 / R2 / R3 / R4)

### Entities (R5-NN)

- E-CLI-LIST-R4-01..04 (status validation, sprint board branch, team-column gating, asset enrichment dedup) → 4 entities
- E-CHANGELOG-R5-01..04 (handle pipeline, AuthorNeedle heuristic, parse_created two-format, from_to_display whitespace) → 4 entities
- E-WORKFLOW-R5-01..05 (handle_move, resolve_resolution, load_resolutions, handle_assign, handle_comment) → 5 entities
- E-API-ISSUES-R5-01..04 (search_issues cursor, get_changelog anti-loop, BASE_ISSUE_FIELDS, internal property) → 4 entities
- E-CACHE-R5-01..03 (generic vs keyed, graceful deserialization, versioned path) → 3 entities
- E-ADF-R5-01..03 (markdown_to_adf pipeline, NodeKind enum, Event mapping table) → 3 entities
- E-SPRINT-R5-01..02 (resolve_scrum_board gate, no-active-sprint divergence) → 2 entities
- E-BOARD-R5-01..03 (auto-discover, scrum/kanban dispatch, build_kanban_jql) → 3 entities
- E-TEAMS-R5-01..02 (GraphQL get_org_metadata, list_teams cursor) → 2 entities
- E-INIT-R5-01 (7-step orchestration) → 1 entity

**Total this round: 31 entities** (not counting refinements of prior).

### Invariants (NEW-INV-216..NEW-INV-306, 91 new this round)

| # | File | Invariant |
|---|---|---|
| NEW-INV-216 | cli/issue/list.rs | --status with --project surfaces 404 as "Project X not found"; only validation path that user-errors a 404 |
| NEW-INV-217 | cli/issue/list.rs | extract_unique_status_names flattens issue-types-with-statuses (project path); global path uses pre-flat list |
| NEW-INV-218 | cli/issue/list.rs | "Available: ..." status error renders API order, no sort/dedup/elision — wall-of-text on busy instances |
| NEW-INV-219 | cli/issue/list.rs | Scrum + no-active-sprint silently degrades to project-scoped query — NO eprintln warning (Pass 4 trigger) |
| NEW-INV-220 | cli/issue/list.rs | Kanban boards never consult sprints; use statusCategory != Done as "active work" heuristic |
| NEW-INV-221 | cli/issue/list.rs | Project-key escape_value applied at all 3 emit sites (scrum-fallback, kanban, no-board) — uniform |
| NEW-INV-222 | cli/issue/list.rs | Any non-"scrum" board type falls to kanban arm — silent degradation for future Atlassian types |
| NEW-INV-223 | cli/issue/list.rs | Team-column gating uses 3 conditions (Table mode + team_field_id + at-least-1-populated); JSON mode skips |
| NEW-INV-224 | cli/issue/list.rs | Cache read uses .ok().flatten() — both Err and miss collapse to empty map; no auto-population |
| NEW-INV-225 | cli/issue/list.rs | Build team_map once; per-row HashMap O(1) — documented performance contract |
| NEW-INV-226 | cli/issue/list.rs | team_id receives client.verbose() so verbose users see one-shot stderr unparseable-shape warnings |
| NEW-INV-227 | cli/issue/list.rs | Asset dedup key is (workspace_id, object_id) — multi-workspace correctness for shared object_ids |
| NEW-INV-228 | cli/issue/list.rs | join_all parallel fetch — N targets in max-latency time, not sum |
| NEW-INV-229 | cli/issue/list.rs | resolved HashMap keyed by oid alone (NOT (wid,oid)) — Pass 4 multi-workspace correctness bug |
| NEW-INV-230 | cli/issue/list.rs | Workspace fallback is LAZY — if no enrichment targets, no API call/cache read |
| NEW-INV-231 | cli/issue/list.rs | JSON back-injection re-runs extract_linked_assets and matches by position — fragile to non-determinism |
| NEW-INV-232 | cli/issue/changelog.rs | All filters apply CLIENT-SIDE; full materialization before filter — Pass 4 perf concern for huge issues |
| NEW-INV-233 | cli/issue/changelog.rs | get_myself() called for `--author me` BEFORE field validation — order should be cheap-first |
| NEW-INV-234 | cli/issue/changelog.rs | Empty/whitespace --author/--field rejected to defend against str::contains("") always-true bypass |
| NEW-INV-235 | cli/issue/changelog.rs | truncate_to_rows operates on rows (per item), not entries; partially trims last entry if needed |
| NEW-INV-236 | cli/issue/changelog.rs | 38 unit tests, NOT 39 (R4 framing slip) |
| NEW-INV-237 | cli/issue/changelog.rs | 12-char boundary in AuthorNeedle::from_raw matches min-public-cloud-accountId length |
| NEW-INV-238 | cli/issue/changelog.rs | is_ascii_alphanumeric rejects non-ASCII letters; refactor to char::is_alphanumeric would silently break |
| NEW-INV-239 | cli/issue/changelog.rs | LoweredStr is module-private newtype; type-system invariant of pre-lowercased haystack |
| NEW-INV-240 | cli/issue/changelog.rs | parse_created accepts BOTH RFC3339 and Jira-compact %z formats; mixed responses sort chronologically |
| NEW-INV-241 | cli/issue/changelog.rs | Unparseable timestamps fall back to lexicographic; deterministic across re-runs |
| NEW-INV-242 | cli/issue/changelog.rs | Empty/whitespace string fields treated as absent — falls through to raw before em-dash |
| NEW-INV-243 | cli/issue/changelog.rs | NULL_GLYPH = "—" (U+2014) — same as cli/assets.rs em-dash; cross-module convention not extracted |
| NEW-INV-244 | cli/issue/workflow.rs | Move match by NUMBER takes precedence over name match (parse usize first) |
| NEW-INV-245 | cli/issue/workflow.rs | Candidate pool unions transition NAMES + target STATUS names, deduped case-insensitively |
| NEW-INV-246 | cli/issue/workflow.rs | Idempotence: move/assign/unassign all return JSON `{success: false}` for "already in target" — distinguishable for scripts |
| NEW-INV-247 | cli/issue/workflow.rs | "resolution required" 400 transformed via lowercased substring match — fragile to Atlassian wording change |
| NEW-INV-248 | cli/issue/workflow.rs | Interactive disambiguation gated by !no_input — no_input path errors with candidate list |
| NEW-INV-249 | cli/issue/workflow.rs | Single-substring resolution hit DOES NOT auto-resolve; only case-insensitive exact wins |
| NEW-INV-250 | cli/issue/workflow.rs | ExactMultiple lists ONLY colliding entries with ids — not full resolution catalog |
| NEW-INV-251 | cli/issue/workflow.rs | id-less resolutions dropped on cache write but RETURNED to caller — defensive ↔ API regressions |
| NEW-INV-252 | cli/issue/workflow.rs | --refresh bypasses cache READ, still WRITES through — warms cache for next caller |
| NEW-INV-253 | cli/issue/workflow.rs | --account-id uses raw id as display name (no API search) — script-friendly bypass |
| NEW-INV-254 | cli/issue/workflow.rs | `jr issue assign FOO-1` (no flags) defaults to current user via get_myself() |
| NEW-INV-255 | cli/issue/workflow.rs | Stdin read in spawn_blocking — defends tokio runtime against blocking syscall starvation |
| NEW-INV-256 | cli/issue/workflow.rs | --markdown is opt-in; default text_to_adf yields single ADF paragraph (no auto-detect) |
| NEW-INV-257 | cli/issue/workflow.rs | --internal silently no-ops on non-JSM projects — Pass 4 UX |
| NEW-INV-258 | api/jira/issues.rs | search_issues uses cursor-based (nextPageToken); distinct from offset-based for users/comments |
| NEW-INV-259 | api/jira/issues.rs | Page size capped at 100 per HTTP round-trip; --limit > 100 paginates internally |
| NEW-INV-260 | api/jira/issues.rs | more_available = (overshoot) OR page_has_more; distinct from bare page.has_more() |
| NEW-INV-261 | api/jira/issues.rs | --limit None → unbounded fetch with no max-pages safety cap or timeout — Pass 4 reliability |
| NEW-INV-262 | api/jira/issues.rs | get_changelog anti-loop guard `next <= start_at` references JRACLOUD-94357-class drift |
| NEW-INV-263 | api/jira/issues.rs | Anti-loop guard is UNIQUE to get_changelog; other paginated endpoints lack it — Pass 4 |
| NEW-INV-264 | api/jira/issues.rs | BASE_ISSUE_FIELDS = 16 entries (recount); shared between search_issues and get_issue |
| NEW-INV-265 | api/jira/issues.rs | Single-source-of-truth: changing BASE_ISSUE_FIELDS affects both endpoints atomically |
| NEW-INV-266 | api/jira/issues.rs | sd.public.comment property silent-no-op on non-JSM — name reads strangely (NOT public = internal=true) |
| NEW-INV-267 | cache.rs | 5 generic + 2 keyed = 7 distinct cache types |
| NEW-INV-268 | cache.rs | CACHE_TTL_DAYS = 7 hardcoded; no config knob |
| NEW-INV-269 | cache.rs | TTL check uses `>=` and num_days() — coarse-grained day-level granularity |
| NEW-INV-270 | cache.rs | Cache corruption is treated as cache-miss with stderr warning — never fatal |
| NEW-INV-271 | cache.rs | Keyed-cache write corruption recovery loses sibling entries — eprintln warning only — Pass 4 UX |
| NEW-INV-272 | cache.rs | Expiring trait is `pub(crate)` — TTL semantics are cache-internal; external impls forbidden |
| NEW-INV-273 | cache.rs | `v1/` between cache_root and profile name — schema bump = `v2/`; old files orphan harmlessly |
| NEW-INV-274 | cache.rs | clear_profile_cache scoped to per-profile dir; never touches v1/ or sibling profiles |
| NEW-INV-275 | adf.rs | pulldown-cmark options enabled = TABLES + STRIKETHROUGH ONLY; no footnotes/tasklist/math/smart-punct |
| NEW-INV-276 | adf.rs | TextMergeStream coalesces adjacent Text events — simplifies AdfBuilder emit logic |
| NEW-INV-277 | adf.rs | ADF version hardcoded to 1; v2 migration would require code change, not config |
| NEW-INV-278 | adf.rs | NodeKind::InlineMark is virtual — no ADF representation; manages active_marks stack only |
| NEW-INV-279 | adf.rs | NodeKind::Sink discards children — graceful degradation for HTML containers |
| NEW-INV-280 | adf.rs | SoftBreak = single space (NOT <br/>); HardBreak = explicit hardBreak ADF node |
| NEW-INV-281 | adf.rs | Inline HTML passed through as plain text; not parsed/rendered; lossy degradation — Pass 4 UX |
| NEW-INV-282 | adf.rs | Catch-all Event arm `_ => {}` silently drops unsupported events — defensive |
| NEW-INV-283 | cli/sprint.rs | Sprint commands hard-error on non-scrum boards; bail with original board_type (lowercased) |
| NEW-INV-284 | cli/sprint.rs | require_scrum=true filters during list_boards AND verifies via get_board_config — two-layer enforcement |
| NEW-INV-285 | cli/sprint.rs | MAX_SPRINT_ISSUES=50 hardcoded; matches Atlassian batch limits — no clap-level enforcement |
| NEW-INV-286 | cli/sprint.rs | handle_current first-active-sprint-wins disambiguation (sprints[0]) — same shape as NEW-INV-179 |
| NEW-INV-287 | cli/sprint.rs | handle_current hard-errors no-active-sprint, distinct from handle_list silent fallback (NEW-INV-219) — inconsistent |
| NEW-INV-288 | cli/sprint.rs | handle_current team-column gating LACKS Table-mode guard — JSON consumers pay wasted cache I/O |
| NEW-INV-289 | cli/board.rs | "Using board {id} - {name} ({type})" auto-discovery message is eprintln (stderr), not stdout |
| NEW-INV-290 | cli/board.rs | Multi-board error includes board_type for non-scrum, omits for scrum-required (less redundant) |
| NEW-INV-291 | cli/board.rs | Kanban view emits "no project configured" warning; scrum implicit via sprint — inconsistent scope handling |
| NEW-INV-292 | cli/board.rs | "Showing N..." has 3 distinct shapes: scrum (no total), kanban+ok (with ~total), kanban+err (no total) |
| NEW-INV-293 | cli/board.rs | build_kanban_jql with no project = global query; mitigated only by eprintln warning |
| NEW-INV-294 | api/jira/teams.rs | get_org_metadata uses single GraphQL query at /gateway/api/graphql; returns orgId+cloudId in one trip |
| NEW-INV-295 | api/jira/teams.rs | GraphQL query string-interpolated, NOT parameterized — should use variables — Pass 4 robustness |
| NEW-INV-296 | api/jira/teams.rs | tenant_contexts.into_iter().next() first-result-wins — same shape as accessible_resources |
| NEW-INV-297 | api/jira/teams.rs | Error suggestion `jr init` is correct because init re-prompts URL — recovery is symmetric |
| NEW-INV-298 | api/jira/teams.rs | list_teams uses opaque-cursor pagination; THIRD pagination convention in jr (offset/token/cursor-string) |
| NEW-INV-299 | api/jira/teams.rs | Cursor URL-encoded — defensive against Atlassian rotating to URL-special chars |
| NEW-INV-300 | api/jira/teams.rs | No infinite-loop guard for list_teams pagination — Pass 4 robustness gap |
| NEW-INV-301 | cli/init.rs | 3-state config-load triage: absent / UserError / other — granular failure handling |
| NEW-INV-302 | cli/init.rs | Profile-name collision is looped (re-prompt) — defends against PR #275 silent-overwrite |
| NEW-INV-303 | cli/init.rs | JR_PROFILE_OVERRIDE env-var seam removed; replaced with parameter threading for tokio soundness |
| NEW-INV-304 | cli/init.rs | Team cache write best-effort — failure does NOT fail init |
| NEW-INV-305 | cli/init.rs | Hostname extraction by hand (no url::Url) — works for Cloud-only `https://x.atlassian.net` shape |
| NEW-INV-306 | cli/init.rs | Story-points-field 3-state discovery (0/1/N): silent skip / auto-use / Select prompt |

### Patterns (NEW-PAT-NN, 0 new this round)

No new patterns. NEW-PAT-01..03 from prior rounds remain canonical.

---

## 6. Retracted / corrected

- **CONV-ABS-8 (CORRECTION)**: R4's cumulative invariant total of "240" double-counts refinements across rounds. Actual: **215 distinct named invariants** (NEW-INV-1..NEW-INV-215). Round 5 raises this to **306 distinct named invariants** (NEW-INV-1..NEW-INV-306).
- **CONV-ABS-9 (RETRACTION)**: NEW-INV-212 ("405 integration tests") and NEW-INV-214 ("Top-4 BC-yield: cli_handler 54, issue_commands 54, issue_changelog 38, assets 24") are **wrong**:
  - Total integration tests: **324** (recount via `grep -c '#\[tokio::test\]\|#\[test\]'` across 33 files).
  - cli_handler.rs has **2** tests, NOT 54 (R4 likely confused with issue_commands.rs).
  - issue_changelog.rs has **39** integration tests (R4 said 38).
  - assets.rs has **21** integration tests (R4 said 24).
  - At least **9 files miscounted** in R4 §3.9 table.
  - Correct top-4: `issue_commands.rs` (54), `issue_changelog.rs` (39), `cli_smoke.rs` (27), `api_client.rs` (22). NEW-INV-214's named files are partially wrong.
- **NO substantive prior-round entity retracted.** All R5 cross-checks (NEW-INV-178, 179, 187, 207, 208, 209) re-verified against source.

---

## 7. Delta Summary — what's new vs Round 4

| Category | Items added (delta) |
|---|---|
| `cli/issue/list.rs` deepening (status validation, sprint-aware branch, team-column gating, asset enrichment) | **+4 entities + 16 invariants** |
| `cli/issue/changelog.rs` deepening (handle pipeline, AuthorNeedle, parse_created, from_to_display) | **+4 entities + 12 invariants** |
| `cli/issue/workflow.rs` deepening (handle_move, resolve_resolution, load_resolutions, handle_assign, handle_comment) | **+5 entities + 14 invariants** |
| `api/jira/issues.rs` deepening (search_issues cursor, get_changelog anti-loop, BASE_ISSUE_FIELDS, internal property) | **+4 entities + 9 invariants** |
| `cache.rs` deepening (7-cache catalog, graceful deserialization, versioned path) | **+3 entities + 8 invariants** |
| `adf.rs` deepening (markdown_to_adf top-level, NodeKind enum, Event mapping table) | **+3 entities + 8 invariants** |
| `cli/sprint.rs` deepening (resolve_scrum_board gate, no-active-sprint divergence, MAX_SPRINT_ISSUES) | **+2 entities + 6 invariants** |
| `cli/board.rs` deepening (auto-discovery, scrum/kanban dispatch, build_kanban_jql) | **+3 entities + 5 invariants** |
| `api/jira/teams.rs` deepening (GraphQL get_org_metadata, list_teams cursor) | **+2 entities + 7 invariants** |
| `cli/init.rs` deepening (7-step orchestration) | **+1 entity + 6 invariants** |

**Quantitative delta (Round 5)**:
- New entities: **31**
- New invariants: **91** (NEW-INV-216..NEW-INV-306; vs R4's 62, R3's 61, R2's 75, R1's 17)
- New patterns: **0**
- Refined existing: **2 prior-round invariants retracted (NEW-INV-212, 214)**, 1 cumulative-count correction (CONV-ABS-8)
- LOC recount discrepancies: **0** (all R4-cited LOCs verified)
- Verified bug claims: 5/5 (NEW-INV-178, 179, 187, 207, 208) re-verified against source.
- **NEW VERIFIED BUGS this round**:
  - NEW-INV-219: scrum + no-active-sprint silent fallback (Pass 4 UX)
  - NEW-INV-229: asset enrichment HashMap keyed by oid alone — multi-workspace mis-attribution (Pass 4 correctness)
  - NEW-INV-261: search_issues unbounded with no safety cap (Pass 4 reliability)
  - NEW-INV-263: anti-loop guard unique to get_changelog; other paginators silent-loop (Pass 4 robustness)
  - NEW-INV-281: inline HTML lossy in markdown_to_adf — silent degradation (Pass 4 UX)
  - NEW-INV-287: handle_current vs handle_list inconsistent no-active-sprint behavior (Pass 4 spec consistency)
  - NEW-INV-288: handle_current team-column lacks JSON-mode skip — wasted I/O (Pass 4 perf)
  - NEW-INV-295: GraphQL query string-interpolated, not parameterized (Pass 4 robustness)
  - NEW-INV-300: list_teams cursor has no infinite-loop guard (Pass 4 robustness)

**Cumulative (broad + R1 + R2 + R3 + R4 + R5)**:
- Total entities: 51 (broad) + 33 (R1) + 67 (R2) + 31 (R3) + 25 (R4) + 31 (R5) = **238**
- Total distinct invariants: **306** (NEW-INV-1..NEW-INV-306, after correcting R4's double-counted total — see CONV-ABS-8)
- Total patterns: NEW-PAT-01..03 = 3

---

## 8. Novelty Assessment

**Novelty: SUBSTANTIVE**

Justification — would removing this round's findings change how you'd spec the system? **Yes**, in at least 9 model-changing ways:

1. **NEW-INV-219 + 287 (silent vs hard-error inconsistency on no-active-sprint)** — `handle_list` and `handle_current` behave differently for the SAME upstream condition. Spec must pin one behavior or surface this as a known issue.

2. **NEW-INV-229 (asset enrichment multi-workspace bug)** — the resolved-HashMap is keyed by `object_id` alone, but the dedup key is `(workspace_id, object_id)`. In a multi-workspace tenant, this mis-attributes enrichment data. Pass 4 correctness defect.

3. **NEW-INV-261 + 263 + 300 (unbounded fetch + missing anti-loop guards)** — search_issues, list_teams, and other paginated endpoints lack the JRACLOUD-94357-class anti-loop guard that get_changelog has. A spec consumer needs to know which endpoints are loop-resistant and which aren't.

4. **NEW-INV-267..274 (full 7-cache catalog with TTL semantics)** — the cache layer is now fully enumerated. Any spec describing "the cache" must enumerate these 7 distinct types and their distinct semantics (generic vs keyed, file-level vs per-entry TTL).

5. **NEW-INV-275..282 (markdown_to_adf full pipeline)** — TABLES + STRIKETHROUGH only; soft-break = space; inline HTML = plain text. Pass 3 BC for `--markdown` flag must enumerate these specifically.

6. **NEW-INV-294..298 (GraphQL hostNames mechanism)** — single round-trip retrieval of orgId + cloudId via GraphQL; string-interpolated query (Pass 4 robustness gap); first-result-wins from tenant_contexts. Spec for org discovery must reflect these.

7. **NEW-INV-301..306 (init's 7-step orchestration)** — config-load triage, profile-name collision loop, JR_PROFILE_OVERRIDE seam removal for tokio soundness, hostname-extraction-by-hand. The init flow is now fully catalogued.

8. **CONV-ABS-9 (R4 fabricated test counts)** — the integration-test catalog in R4 is partially fabricated (cli_handler 54 → actual 2; total 405 → actual 324). Pass 3 BC enumeration must use the corrected counts.

9. **NEW-INV-258 + 298 (3 distinct pagination conventions in jr)** — offset (`startAt`/`maxResults`), cursor-token (`nextPageToken`), opaque-cursor-string (`cursor`). Pass 3 BC for paginated endpoints must enumerate which convention each endpoint uses.

These 9 are model-changing findings, not refinements. The 91 new invariants this round (vs Round 4's 62) actually **accelerated** novelty rather than decaying. **SUBSTANTIVE.**

---

## 9. Remaining gaps / next candidate scope (verbatim for Round 6)

### High priority (still under-deepened or partially attacked)

1. **`api/client.rs` deep round 1** — broad pass + R3 (?) covered HTTP wrapper; Round 6 should:
   - Catalogue every HTTP method (`get`, `post`, `put`, `delete`, `post_no_content`, `get_from_instance`, `post_to_instance`).
   - Auth-header construction order (OAuth bearer → Basic api-token → none).
   - 429 retry-after parsing (`api/rate_limit.rs` interaction).
   - 401 detection + automatic refresh attempt (`refresh_oauth_token`).
   - Verbose-mode body logging (PII redaction — does it exist?).

2. **`cli/issue/create.rs` and `cli/issue/edit.rs`** — broad pass mentioned create+edit; Round 6 should:
   - Catalogue field-building logic: `summary`, `description`, `--issue-type`, `--priority`, `--labels`, `--components`, `--fix-versions`, `--parent`, `--assignee`, `--sprint`, custom fields.
   - JSON-input mode (`--json`) vs flag-driven mode.
   - Story-points field injection.

3. **`cli/worklog.rs`** — broad pass + R2 covered list_worklogs non-pagination; Round 6 should:
   - Walk `handle_add` (`--time`, `--started`, `--comment` ADF flow).
   - The duration parser interaction (`duration.rs` 2h, 1h30m, 1d, 1w shapes).
   - The 8-hour-day / 5-day-week constants (R2 carry).

4. **`cli/team.rs`** — broad pass mentioned; Round 6 should walk:
   - Lazy org metadata discovery (config has cloud_id+org_id, or fetch via GraphQL).
   - 7-day team cache write.
   - Verbose-mode warning for team-id parse failures (already noted in NEW-INV-208/226).

5. **`cli/user.rs`** — broad pass mentioned; Round 6 should:
   - The user/search vs assignable-users vs single-user lookup paths.
   - User pagination (offset-based per `user_pagination.rs` 11 tests).
   - The duplicate-disambiguation flow (`tests/duplicate_user_disambiguation.rs` 5 tests).

6. **`cli/queue.rs`** — broad pass + R4 covered jsm/queues + servicedesks; Round 6 should walk the user-facing handler:
   - The require_service_desk gate from cli/queue.rs perspective.
   - The queue list + view JSON shape.
   - The two-step issue-key fetch + search_issues hydration (R4 NEW-INV-203).

7. **`cli/project.rs`** — broad pass + R3 mentioned types/priorities/statuses/cmdb; Round 6 should walk:
   - The `jr project fields` discovery.
   - The `jr project types/priorities/statuses` enumeration paths.
   - The `jr project cmdb-fields` inventory.

### Medium priority

8. **`cli/issue/links.rs`** — broad pass mentioned; Round 6 should walk:
   - link-types caching, link create/delete, partial_match for link names.

9. **`cli/issue/helpers.rs`** — broad pass mentioned; Round 6 should walk:
   - resolve_assignee, prompt_input, is_me_keyword, resolve_team / resolve_points.

10. **`cli/issue/json_output.rs`** — newly-discovered file; Round 6 should:
    - Catalogue every response shape (move_response, assign_*, unassign_*, etc.).

11. **`adf.rs::adf_to_text`** (lines 345-688) — R5 covered markdown_to_adf only; Round 6 should walk:
    - The ListFrame state machine for ordered/bullet rendering.
    - The render_node 12+ match-arm catalogue.
    - The mention/emoji/inlineCard/media* lossy fall-throughs (per R3 NEW-INV-101).

### Low priority (NITPICK candidates from R3/R4)

12. **`output.rs`, `error.rs`, `auth_embedded.rs`, `build.rs`, `observability.rs`** — confirmed NITPICK in R3+R4. **CONVERGED at file level.**

13. **`api/assets/tickets.rs`, `api/assets/schemas.rs`, `api/jsm/queues.rs`** — Round 4 catalogued. **CONVERGED.**

14. **`config.rs`, `cache.rs`, `api/auth.rs`, `cli/auth.rs`** — major deepening rounds done; CONVERGED at file level (further detail would be NITPICK).

### Pass 4 deepening triggered (cross-pollination — DO NOT write into Pass 2)

15-23. NEW-INV-219, 229, 261, 263, 281, 287, 288, 295, 300 — all Pass 4 cross-pollination items (this round's verified bugs).
24. (Carry from R4): NEW-INV-157, 158, 163, 169, 175, 178, 179, 185, 190 — Pass 4.
25. (Carry from R3): NEW-INV-101, 105, 119, 127, 143, 148 — Pass 4.
26. (Carry from R2): handle_open OAuth bug, list_worklogs non-pagination, hardcoded 8/5, asset enrichment dedup — Pass 4.

---

## 10. State Checkpoint

```yaml
pass: 2
round: 5
status: complete
audit_findings_against_hallucination_classes: 2
new_entities: 31
new_invariants: 91
retracted_findings: 2
files_examined: 11
novelty: SUBSTANTIVE
timestamp: 2026-05-04T23:30:00Z
next_round_targets: |-
  1. api/client.rs deep round 1 — HTTP method surface, auth header order, 429/401 handling, verbose-body redaction
  2. cli/issue/create.rs + cli/issue/edit.rs — field-building, JSON-input mode, story-points injection
  3. cli/worklog.rs — handle_add, duration parser, 8h/5d constants
  4. cli/team.rs — lazy org metadata, team cache write, verbose warnings
  5. cli/user.rs — search/assignable/single-user paths, pagination, disambiguation
  6. cli/queue.rs — require_service_desk gate, two-step issue hydration
  7. cli/project.rs — fields/types/priorities/statuses/cmdb discovery
  8. cli/issue/links.rs — link-types cache, partial_match
  9. cli/issue/helpers.rs — resolve_assignee, prompt_input, is_me_keyword
  10. cli/issue/json_output.rs — full response-shape catalog
  11. adf.rs::adf_to_text — ListFrame state machine, render_node match arms
  12. (CONVERGED file-level) output.rs, error.rs, auth_embedded.rs, build.rs, observability.rs, api/assets/tickets.rs, api/assets/schemas.rs, api/jsm/queues.rs
  13. (CONVERGED file-level) config.rs, cache.rs, api/auth.rs, cli/auth.rs
  14-23. (Pass 4 cross-pollination) NEW-INV-219, 229, 261, 263, 281, 287, 288, 295, 300
  24. (Pass 4 carry, R4) NEW-INV-157, 158, 163, 169, 175, 178, 179, 185, 190
  25. (Pass 4 carry, R3) NEW-INV-101, 105, 119, 127, 143, 148
  26. (Pass 4 carry, R2) handle_open OAuth, list_worklogs, 8h/5d, asset dedup
```

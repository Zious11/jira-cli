# `search_issue_keys` Lightweight API Spec

**Issue:** [#350](https://github.com/Zious11/jira-cli/issues/350) — add a lightweight `search_issue_keys` API method for JQL bulk-edit selection, avoiding the heavy `BASE_ISSUE_FIELDS` payload when only keys are needed downstream.

## Problem

`JiraClient::search_issues` always requests `BASE_ISSUE_FIELDS` (summary, status, description, issuelinks, labels, components, fixVersions, parent, etc. — see `src/api/jira/issues.rs:12-29`) regardless of how the caller intends to use the result. For a 100-issue match against a tenant where each issue has ~10 KB descriptions plus issuelinks, that is ~1 MB of unnecessary data per JQL search.

There is exactly one caller that throws away the body fields: `src/cli/issue/create.rs:386` in `handle_edit::effective_keys` (the JQL-driven bulk-edit selection path). It runs `.search_issues(jql_str, Some(effective_max + 1), &[])` and then immediately reduces to `Vec<String>` via `.into_iter().map(|i| i.key).collect()`. Every body byte that came back over the wire was wasted.

For `--max` values close to the Atlassian hard ceiling `BULK_MAX_KEYS = 1000` plus pagination, this measurably slows the bulk-edit flow and inflates the audit trail produced by `--verbose-bodies`. The fix is to give that caller a method that asks Jira for only the keys.

Sources reviewed for this spec: `.factory/research/issue-350-search-issue-keys-design.md` (full validation report + adversarial-pass addendum).

## Scope

**In scope:**

- New `JiraClient::search_issue_keys(jql, limit)` method that paginates `POST /rest/api/3/search/jql` with body `fields: ["key"]`.
- New public result struct `KeySearchResult { keys: Vec<String>, has_more: bool }`.
- New private deserialization helper `IssueKeyRow { key: String }` (does NOT reuse `Issue` — which forces the heavy `fields: IssueFields` body).
- Caller migration at `src/cli/issue/create.rs:386` only.
- New BC-2.6.050 in `.factory/specs/prd/bc-2-issue-read.md`, with the BC-INDEX count bumped and `scripts/check-spec-counts.sh` run as a verification.
- One-line update to `CLAUDE.md`'s `src/api/jira/issues.rs` blurb.

**Out of scope:**

- Changing the public signature or behavior of `search_issues` — purely additive at the time this spec was written. **Note (issue #361 follow-up, 2026-05-13):** under Copilot review on PR #364, `search_issues` was later updated to set `SearchResult.has_more = true` on repeated-cursor guard abort (matching the `KeySearchResult` contract this spec introduced). The signature is unchanged, but `has_more` will now report `true` in cases where it previously silently reported `false`. The three CLI readers (`cli/issue/list.rs`, `cli/board.rs`, `cli/sprint.rs`) consume this for truncation hints and will display the existing "Showing N of M" hint correctly on guard abort. No CLI flag, JSON shape, or error changes.
- Lifting the `.min(100)` per-page clamp; kept for hardening parity even though Atlassian docs say id/key-only requests can return more rows per page (see Validated API Facts §3).
- Tightening the repeated-cursor stderr warning text (the original warning overclaimed "server bug" by citing JRACLOUD-94632 even though live-data drift per JRACLOUD-95368 is also a legitimate trigger). **Resolved by issue #361:** follow-up research showed all three originally-cited tickets (JRACLOUD-94632 / -92049 / -85546) cover unrelated `/search/jql` defects; the warning was rebound to JRACLOUD-95368 ("nextPageToken pagination is not snapshot-stable under live mutation"), and the Atlassian-KB `key ASC` ORDER BY tie-breaker mitigation was added inline.
- Migrating other `search_issues` callers (`list.rs`, `view.rs`, `assets.rs`) — they consume the body fields. No keys-only callers exist elsewhere today.
- Adding a `JrError` variant — error surface is identical to `search_issues` (`anyhow::Result` from `self.post(...)`).
- Using `has_more` at the migrated caller. The existing `+1` lookahead + `matched_keys.len() > effective_max` truncation-error path is preserved verbatim; `has_more` is exposed for future callers.

## Validated API Facts

From the official Atlassian REST API v3 documentation (`api-group-issue-search`) and supporting community sources cited in `.factory/research/issue-350-search-issue-keys-design.md`:

1. **`fields: ["key"]` is the minimal valid body value for `POST /rest/api/3/search/jql`.** The `*none` magic value is supported on `/rest/api/3/issue/{id}` but NOT on `/search/jql`.
2. **Top-level `key` is always present** in each issue object regardless of the `fields` value. The minimal response per row is approximately `{"id":"...", "key":"...", "self":"..."}` plus a possibly-empty `fields: {}` body.
3. **`maxResults` is not API-capped at 100.** Atlassian explicitly documents that "the greatest number of items returned per page is achieved when requesting id or key only." The existing `.min(100)` clamp in `search_issues` is a conservative *client-side* choice. This spec keeps `.min(100)` for parity; lifting it is a separate decision.
4. **Pagination on `/search/jql` is cursor-only**: response carries `nextPageToken` (no `startAt`, no `total`). Same as `search_issues`.
5. **The `nextPageToken` repeated-cursor symptom is endpoint-level, not fields-level.** The mechanism documented in [JRACLOUD-95368](https://jira.atlassian.com/browse/JRACLOUD-95368) ("nextPageToken pagination is not snapshot-stable under live mutation") applies to keys-only requests just as to full-body requests. The existing anti-loop guard in `search_issues` (the "GUARD: detect repeated cursor token" block) MUST be mirrored verbatim. **Note (issue #361 follow-up, 2026-05-13):** this paragraph originally cited [JRACLOUD-94632](https://jira.atlassian.com/browse/JRACLOUD-94632), [JRACLOUD-92049](https://jira.atlassian.com/browse/JRACLOUD-92049), and [JRACLOUD-85546](https://jira.atlassian.com/browse/JRACLOUD-85546) as the evidence; subsequent research showed those three tickets cover unrelated `/search/jql` defects (initial-`nextPageToken=null` rejection; `startAt` offset behavior; missing `nextPage` field on `/field/search` respectively). The conclusion — that the guard must be mirrored at the endpoint level — stands; only the cited evidence changed.
6. **Inconclusive — `fields{}` body echo.** Some Perplexity sources suggested Jira may echo `key` inside `fields: {"key": "..."}` for `fields: ["key"]` requests. No empirical capture in published docs. Mitigation: deserialize only the top-level `key`. If Jira ever inverts this, the deserialization fails loudly rather than silently returning empty strings.
7. **April 2025 regression.** [community.developer.atlassian.com thread 88287](https://community.developer.atlassian.com/t/post-rest-api-3-search-jql-does-not-respect-maxresults-param/88287) reports a window where `maxResults` was disrespected and up to 10 000 issues were returned. Worth a regression-pinning test that the local truncate to `limit` still holds.

## Design

### Architecture

Follow the `search_users` / `search_users_all` precedent already established in this repo: keep the existing single method untouched; add a parallel concrete method + result struct with a domain-named field. Rust SDK precedent (octocrab, aws-sdk-rust, kube-rs) confirms duplicate concrete wrappers are idiomatic when domain semantics differ — `Issue` (rich object) vs `String` (primitive key) are not "uniformly similar items" per Rust API Guideline C-GEN-ITEMS. See research report §2.

Alternatives considered and rejected:

- **Genericizing `SearchResult<T>` with renamed field `items: Vec<T>`** — breaking-renames the `issues` field at every existing call site (`list.rs`, `view.rs`, `assets.rs`); large blast radius for one new caller; rejected as YAGNI.
- **Returning `Vec<String>` without a wrapper** — preserves the `+1` lookahead idiom but cannot signal `has_more` without forcing a magic protocol on every future caller. Violates the Rust principle of making impossible states unrepresentable.

### API shape

```rust
// src/api/jira/issues.rs

/// Result of a keys-only paginated issue search.
///
/// Parallel to `SearchResult`. Field name `keys` mirrors `issues` on
/// `SearchResult` (domain-named, not generic `items`) to match the
/// Rust-SDK precedent surveyed in
/// `.factory/research/issue-350-search-issue-keys-design.md`.
///
/// `has_more` is set to `true` in two cases:
///
/// 1. **Caller limit hit:** the caller supplied a `limit` and upstream still
///    had rows available beyond that limit. No guard-induced duplicates in
///    this case (within-page dedupe is the server's responsibility — Jira
///    does not document an explicit uniqueness guarantee, but does not
///    typically emit within-page duplicates absent live mutation).
/// 2. **Repeated-cursor guard abort:** the anti-loop guard fired (upstream
///    returned the same `nextPageToken` twice — typically live-data drift
///    per JRACLOUD-95368, "nextPageToken pagination is not snapshot-stable
///    under live mutation"), pagination was aborted with a stderr warning,
///    and the result set may be **incomplete AND may contain duplicate
///    keys**. `search_issue_keys` does not dedupe; under live-data drift the
///    server may have emitted the same key on multiple pages before the
///    cursor repeated. Callers that need strict uniqueness should re-issue
///    with `key ASC` as a stable secondary sort in the ORDER BY (append
///    `, key ASC` to an existing sort, or use `ORDER BY key ASC` if none —
///    JQL allows only one ORDER BY clause) — Atlassian's KB mitigation —
///    or dedupe locally.
///
/// When `limit` is set, callers cannot distinguish case 1 from case 2 from
/// `has_more` alone — the stderr warning fires only in case 2. When `limit`
/// is `None`, case 1 cannot trigger, so `has_more = true` unambiguously
/// signals case 2 (repeated-cursor guard abort). Today's sole caller
/// (`handle_edit::effective_keys` in `cli/issue/create.rs`) requests
/// `limit = effective_max + 1` and treats `keys.len() > effective_max` as a
/// truncation error. **This is NOT dup-tolerant**: a single drift-induced
/// duplicate inflates `keys.len()` by 1 and can spuriously trip the
/// truncation error when the true unique-key count is at the limit. Bulk
/// edits also do not dedupe — the same issue could receive the same field
/// edit twice (idempotent at the Jira API for most fields, but wasteful).
/// Both effects are user-visible-but-safe; not blocking. Future callers
/// that need a richer signal should track the deferred follow-up
/// (`feat(search): dedupe keys on JRACLOUD-95368 repeated-cursor guard abort`)
/// to surface "incomplete-and-possibly-duplicated" via the type system.
///
/// Traces to BC-2.6.050.
#[derive(Debug, Clone, PartialEq)]
pub struct KeySearchResult {
    pub keys: Vec<String>,
    pub has_more: bool,
}

impl JiraClient {
    // Existing — unchanged signature:
    pub async fn search_issues(
        &self,
        jql: &str,
        limit: Option<u32>,
        extra_fields: &[&str],
    ) -> Result<SearchResult> { ... }

    /// Search issues using JQL and return ONLY the matching issue keys.
    ///
    /// Lightweight variant of `search_issues` — requests `fields: ["key"]`
    /// in the POST body and deserializes only the top-level `key`, avoiding
    /// the ~10 KB-per-issue payload that `BASE_ISSUE_FIELDS` incurs.
    ///
    /// Use this when the caller only needs keys (e.g., JQL-driven bulk-edit
    /// selection at `cli/issue/create.rs::handle_edit`). For all other
    /// reads, use `search_issues`.
    ///
    /// Traces to BC-2.6.050.
    pub async fn search_issue_keys(
        &self,
        jql: &str,
        limit: Option<u32>,
    ) -> Result<KeySearchResult> { ... }
}
```

The new method derives `Debug`, `Clone`, `PartialEq` on `KeySearchResult` per Rust API Guideline C-COMMON-TRAITS. (The existing `SearchResult` is missing these derives; that is a latent gap, not a precedent to copy.)

### Wire shape and deserialization

Request body:

```json
{
  "jql": "<jql>",
  "maxResults": 100,
  "fields": ["key"],
  "nextPageToken": "<token>"
}
```

Response (Atlassian; with `fields: ["key"]`):

```json
{
  "issues": [
    { "id": "10001", "key": "FOO-1", "self": "https://.../10001", "fields": {} },
    { "id": "10002", "key": "FOO-2", "self": "https://.../10002", "fields": {} }
  ],
  "nextPageToken": "...",
  "isLast": false
}
```

Deserialization target (private):

```rust
#[derive(Deserialize)]
struct IssueKeyRow {
    key: String,
    // No `id`, no `self`, no `fields` — silently dropped via serde defaults.
    // No `#[serde(deny_unknown_fields)]`: Atlassian routinely adds top-level
    // fields and the SDK convention is to ignore unknowns silently.
}
```

`CursorPage<IssueKeyRow>` from `src/api/pagination.rs` is reused unchanged.

### Pagination loop

Logic mirrors the body of `search_issues` exactly, with two type substitutions: `Issue` → `IssueKeyRow` and `SearchResult` → `KeySearchResult`. Specifically:

- `maxResults` clamped to `.min(100)` (parity with `search_issues`, see Validated API Facts §3).
- `next_page_token` advance + `is_last` termination identical.
- **Repeated-cursor anti-loop guard verbatim**: track `prev_cursor`; on `next_cursor == prev_cursor`, abort with the same stderr warning text (citing the Atlassian ticket ID so users have a copy-pasteable search term). The warning text was inherited as-is from the original `search_issues` guard at the time this spec was written; the citation was later tightened by issue #361 (which rebound `JRACLOUD-94632` → `JRACLOUD-95368` after research showed the original ticket cited a different defect) — see Out of Scope above and Risks below.
- Local truncation: if `limit` is set and `keys.len() >= limit`, set `has_more = true` and truncate to `limit`.

### Caller migration

Single site: `src/cli/issue/create.rs:374-409` in `handle_edit::effective_keys`.

```diff
-        let search_result = client
-            .search_issues(jql_str, Some(effective_max + 1), &[])
-            .await?;
-        let matched = search_result.issues;
+        let search_result = client
+            .search_issue_keys(jql_str, Some(effective_max + 1))
+            .await?;
+        let matched_keys = search_result.keys;

         if matched_keys.is_empty() { return Err(... "matched 0 issues" ...); }

         if matched_keys.len() > effective_max as usize {
             return Err(... "matched at least N issues, exceeds --max M" ...);
         }

-        matched.into_iter().map(|i| i.key).collect()
+        matched_keys
```

The `+1` lookahead is preserved so the exact-count error message is unchanged. The new `has_more` field is exposed for future callers but unused here.

### Doc and spec fallout

- **CLAUDE.md** — one-line update to the `src/api/jira/issues.rs` blurb (currently "search, get, create, edit, list comments") to note `search_issue_keys` exists alongside `search_issues`.
- **`.factory/specs/prd/bc-2-issue-read.md`** — add BC-2.6.050 in subdomain 2.6 (API layer), after BC-2.6.049. Suggested text: *"`client.search_issue_keys(jql, limit)` posts `/rest/api/3/search/jql` with body `fields: ["key"]` and deserializes only the top-level `key` field; returns `KeySearchResult { keys, has_more }` where `has_more` is `true` on caller-side truncation OR repeated-cursor guard abort (typically JRACLOUD-95368 live-data drift), and `false` on pure cursor exhaustion."* Also bump the file's frontmatter `definitional_count` from 49 → 50.
- **`.factory/specs/prd/BC-INDEX.md`** — bump frontmatter `total_bcs` from 545 → 546 AND update the `sections:` line for `bc-2-issue-read.md` from `49 individually-bodied` → `50 individually-bodied`. Both counts must stay aligned with each other and with `bc-2-issue-read.md`'s `definitional_count`.
- **`scripts/check-spec-counts.sh`** — run as a verification (exit 0 required) before committing the spec changes. The script enforces frontmatter ↔ body count alignment per DRIFT-001 mitigation.

## Tests

### Integration tests (new file: `tests/search_issue_keys.rs`)

Wiremock-rs 0.6.5 against a test-only `JiraClient`. **Important matcher note:** `wiremock::matchers::body_partial_json` uses `assert_json_diff::assert_json_include` which is SUBSET-matching, NOT length-strict, for arrays — a matcher built from `["key"]` would also match a body whose `fields` array was `["key", "summary", "description"]`. The addendum §Q3 claim that it was length-strict is **retracted** (verified via Perplexity 2026-05-13 against wiremock 0.6.5 source). For true length-strict assertions on the `fields` array, tests capture the request via `MockServer::received_requests().await` and use `assert_eq!` on the resulting `serde_json::Value` (which IS length-strict for arrays).

1. `test_search_issue_keys_sends_fields_key_only` — calls the method, then asserts on the captured request body via `received_requests()` that `body["fields"] == json!(["key"])` exactly. Pins the perf goal: the request body asks for only `key`, never `BASE_ISSUE_FIELDS`.
2. `test_search_issue_keys_happy_path` — three rows return `{"key": "FOO-1", "fields": {}}` etc.; asserts `keys == ["FOO-1", "FOO-2", "FOO-3"]` and `has_more == false`.
3. `test_search_issue_keys_paginates_with_next_page_token` — page 1 returns `nextPageToken: "abc"`, page 2 returns `isLast: true`; asserts both keys arrays concatenated in order, `has_more == false`, both mocks `.expect(1)`.
4. `test_search_issue_keys_repeated_cursor_aborts_with_warning` — page 1 returns `nextPageToken: "loop"`, page 2 returns the SAME `nextPageToken: "loop"`; asserts loop aborts and returns only page 1's keys (stderr-literal assertion lives in test 11 — see below).
5. `test_search_issue_keys_truncates_at_limit_and_sets_has_more` — `limit = Some(2)`, server returns three rows; asserts `keys.len() == 2`, `has_more == true`. Pins the `has_more` semantics documented in the rustdoc of `KeySearchResult`.
6. `test_search_issue_keys_apr2025_regression_bound` — server returns 500 rows for `maxResults = 10`; caller passes `limit = Some(10)`; asserts exactly 10 keys returned and `has_more == true`. Defense-in-depth against the documented Atlassian regression.
7. `test_search_issue_keys_ignores_unknown_fields` — server returns rows with extra top-level fields (`id`, `self`, `expand`, and a future-hypothetical `securityLevel`); deserializer silently ignores. Confirms no `deny_unknown_fields`.
8. `test_search_issue_keys_returns_empty_for_no_matches` — server returns `{"issues": [], "isLast": true}`; asserts `keys.is_empty()` and `has_more == false`. *(Added per addendum §Q6.)*
9. `test_search_issue_keys_401_mid_pagination_propagates` — page 1 returns 200 with `nextPageToken`, page 2 returns 401; asserts the second `?` propagates and the method returns `Err`. Pins error-propagation contract across page boundaries (and validates interaction with the S-3.03 v2 auto-refresh path: refresh fires once, retry observes the original failure on the test seam). *(Added per addendum §Q6.)*
10. `test_search_issue_keys_malformed_json_returns_error` — page 1 returns 200 with corrupted body `{"issues": [{"key": ` (incomplete JSON); asserts `Err` propagates from serde. *(Added per addendum §Q6.)*
11. `test_search_issue_keys_stderr_emits_jracloud_95368_literal` — *(subprocess test)* — spawns `jr issue edit --jql ... --dry-run` against a stuck-cursor mock; captures stderr and asserts it contains the literal `"JRACLOUD-95368"`. Pairs with test 4 to satisfy AC-003's stderr-literal arm — library tests cannot capture `eprintln!` from inside the same process. *(Added during pass-01 F-02 fix; literal rebound from `"JRACLOUD-94632"` to `"JRACLOUD-95368"` under issue #361.)*
12. `test_search_issue_keys_clamps_max_results_to_100_per_page` — caller passes `limit = Some(200)` (> 100); the `.min(100)` clamp must reduce `maxResults` to 100 in the request body. Verified by capturing the request via `MockServer::received_requests()` and asserting `body["maxResults"] == 100`. Pins BC-2.6.050 §5. *(Added during pass-07 F-01 fix.)*
13. `test_search_issue_keys_repeated_cursor_abort_does_not_dedupe` — page 1 returns `["X-1"]` with `nextPageToken: "loop"`, page 2 returns `["X-1", "X-2"]` with the same `nextPageToken: "loop"` (simulating live-data drift mid-pagination); asserts `keys == ["X-1", "X-1", "X-2"]` and `has_more == true`. Pins the **no-dedupe contract** under JRACLOUD-95368 abort. *(Added by issue #361; if a future PR adds in-function dedupe, this test must be updated in lockstep with the caller-migration plan.)*

### Caller-level integration test (`tests/issue_bulk_pr2.rs`)

`test_handle_edit_jql_truncation_error_still_triggers_after_migration` — runs `jr issue edit --jql '<q>' --max 5 --label add:foo` with wiremock returning 7 keys; asserts the existing "JQL matched at least 6 issues, which exceeds --max 5" error path still fires after the migration. Pins regression invariant.

### Unit tests

None required. The new method is a thin wrapper over the HTTP surface; its semantics are exercised end-to-end via wiremock above. The private `IssueKeyRow` struct is implicitly covered by tests 2 and 7.

## Risks and Mitigations

- **Inconclusive: `fields{}` echo (Validated API Facts §6).** Mitigated by reading only top-level `key`. If Jira ever inverts this, tests 2/3/8 fail loudly with a serde "missing field `key`" deserialization error (NOT empty-string keys — IssueKeyRow.key has no #[serde(default)]).
- **JRACLOUD-94632 false positives (addendum §Q4).** The inherited warning text overclaimed "server bug" but the symptom-to-ticket mapping turned out to be even worse than the addendum noted: follow-up research under issue #361 (2026-05-13) showed JRACLOUD-94632 / -92049 / -85546 all cover *different* `/search/jql` defects — none of them describes "the server returns the same `nextPageToken` twice". The only correctly-attributed public source is **JRACLOUD-95368** ("nextPageToken pagination is not snapshot-stable under live mutation"), plus Atlassian's [KB article on inconsistent paginated JQL results](https://support.atlassian.com/jira/kb/inconsistent-paginated-api-search-result-while-using-jql/) which prescribes the `ORDER BY key ASC` mitigation. The warning text was rebound accordingly. Resolved.
- **Accepted trade-off: JRACLOUD-95368 names "duplicates/skips", not "repeated `nextPageToken`" specifically.** Per `.factory/research/issue-361-jra95368-scope.md` §3 (local, gitignored), the ticket's public Description enumerates duplicates and skips as symptoms; cursor-repetition is one inferential step from the same root-cause class ("snapshot instability under live mutation"). The user-visible warning cites only JRACLOUD-95368 (with `Likely cause:` hedging) rather than the longer "see Atlassian KB on inconsistent paginated JQL results" form. Reasoning: a user reading JRACLOUD-95368 will find the same root-cause story even if the cursor-repetition symptom is not literally enumerated, and the warning already names the actionable ORDER BY mitigation inline. The internal comment blocks (`src/api/jira/issues.rs` around lines 132-160 and 230-244) document the inferential step explicitly. Not blocking.
- **Possible duplicate keys on guard-abort under live-data drift (new finding from #361 research).** Because JRACLOUD-95368 is the typical cause, `search_issue_keys` *may* return duplicate keys in the abort path (live mutation can produce the same key on two pages before the cursor repeats). `search_issue_keys` does NOT dedupe today; the post-abort rustdoc and inline comment now warn callers explicitly and recommend `ORDER BY key ASC` as the upstream prevention. A separate follow-up issue (#365) tracks in-function dedupe. **Not blocking, but user-visible-but-safe** — the existing single caller (`handle_edit::effective_keys`) has two affected paths: (a) the `+1` lookahead truncation check (`keys.len() > effective_max`) is NOT dup-tolerant — a drift-induced duplicate inflates `keys.len()` by 1 and can spuriously trip the "exceeds --max" error when the true unique-key count is exactly at the limit; (b) bulk-edit calls would apply the same field edit to the same key twice. Both are safe: (a) is a recoverable user-visible error (re-run with `ORDER BY key ASC`), (b) is idempotent at the Jira API for most fields (labels server-side dedupe; assignee/priority/etc are point-set operations). An earlier draft of this PR called this "dup-tolerant by construction" — that overclaim was retracted under Copilot review on PR #364.
- **`has_more` semantic drift.** `has_more = true` has two distinct truthy triggers: caller-side truncation (limit hit) OR repeated-cursor guard abort (incomplete results due to live-data drift per JRACLOUD-95368). If a future caller assumes only one trigger is possible, they could mis-paginate or miss the guard-abort signal. Mitigated by explicit rustdoc on the struct (two numbered cases) + test 5 pinning the truncation contract + test 4 pinning the guard-abort path.
- **Length-strict array enforcement on `fields`.** `wiremock::body_partial_json` is subset-matching, so the strictness assertion lives in the test body (`received_requests()` + `assert_eq!`), not in the matcher. The addendum §Q3 claim that `body_partial_json` was length-strict is retracted in this spec — see Tests §1 note. A negative `.expect(0)` mock is not needed because the explicit `assert_eq!` is stronger.

## Backwards Compatibility

No public API or CLI behavior change for any consumer who does not call the new method:

- `search_issues(jql, limit, extra_fields)` — signature unchanged. Semantics unchanged at spec-write time, but **PR #364 (issue #361 follow-up)** added one carve-out: `SearchResult.has_more` is now set to `true` on repeated-cursor guard abort (was silently `false`), matching the `KeySearchResult` contract this spec introduced. Three CLI readers (`cli/issue/list.rs`, `cli/board.rs`, `cli/sprint.rs`) will display the existing "Showing N of M" truncation hint in that case where they previously silently reported a complete result. No CLI flag, JSON shape, or error changes; see the Out-of-Scope bullet above for the full rationale.
- `jr issue list --jql ...` — uses `search_issues`. Unchanged at spec-write time, with one corner-case carve-out under PR #364: in the rare repeated-cursor guard-abort scenario (`/rest/api/3/search/jql` returning the same `nextPageToken` twice — typically JRACLOUD-95368 live-data drift), the truncation hint at `cli/issue/list.rs` (the `has_more && !all` branch) will now print "Showing N of M, use --all to see more" where it previously silently reported a complete result. The new hint is correct behavior; no flag, JSON shape, or exit-code change.
- `jr board view` — uses `search_issues` for the kanban path. Same corner-case carve-out as `jr issue list` above (the `has_more` consumer at `cli/board.rs:206/216` will display the existing truncation hint on guard abort).
- `jr sprint current` — uses `get_sprint_issues` for the active-sprint path. **Not** affected by the `search_issues` carve-out (it consumes `OffsetPage`, not the `/search/jql` cursor). The `has_more` consumer at `cli/sprint.rs:245` is unchanged.
- `jr issue view <key>` — uses `get_issue`; unchanged.
- `jr issue edit --jql ... --max N` — was: full-body fetch for selection. **Now**: keys-only fetch for selection. Same external behavior (same error messages, same truncation logic), faster wire path. The `KeySearchResult.has_more` semantics described in the Out-of-Scope section apply: under guard abort, `effective_keys` may include duplicates that spuriously trip the `--max` truncation error or generate redundant bulk-edit calls (both safe-but-user-visible; tracked in #365).

## Out of Scope / Follow-ups

- ~~**Follow-up issue (file at same time as this PR):** `chore(search): tighten JRACLOUD-94632 stderr warning — repeated cursors can be live-data drift, not just server bug`. References JRACLOUD-95368.~~ **CLOSED by #361 (2026-05-13).** Follow-up research showed all three originally-cited Atlassian tickets (94632, 92049, 85546) cover unrelated defects; the warning was rebound to JRACLOUD-95368 + Atlassian KB `ORDER BY key ASC` mitigation.
- **New follow-up (deferred):** `feat(search): dedupe keys on repeated-cursor guard abort` — under JRACLOUD-95368 live-data drift, the abort path may have already collected duplicate keys in earlier pages. Not blocking for the existing single caller (`handle_edit` truncates via `+1` lookahead) but worth addressing before a second caller appears.
- Lifting the `.min(100)` per-page clamp for keys-only requests, if benchmarks ever justify it.
- A second keys-only caller — none exists today. If one appears, no API change needed.

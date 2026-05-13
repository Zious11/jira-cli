---
document_type: story
story_id: "S-350"
title: "Lightweight search_issue_keys API for JQL bulk-edit selection"
wave: feature-followup
status: draft
priority: medium
estimated_effort: small
tdd_mode: strict
bc_anchors:
  - BC-2.6.050
holdout_anchors: []
nfr_anchors: []
adr_refs:
  - ADR-0004
sd_refs: []
files_modified:
  - src/api/jira/issues.rs (add KeySearchResult struct, search_issue_keys method, IssueKeyRow private deserialization helper)
  - src/cli/issue/create.rs (migrate JQL bulk-edit selection at handle_edit::effective_keys; lines 374-409)
  - CLAUDE.md (one-line update to src/api/jira/issues.rs file blurb)
  - .factory/specs/prd/bc-2-issue-read.md (add BC-2.6.050)
  - .factory/specs/prd/BC-INDEX.md (frontmatter total_bcs bump)
test_files:
  - tests/search_issue_keys.rs (new wiremock integration test, 11 cases — 10 library tokio + 1 subprocess)
  - tests/issue_bulk_pr2.rs (add caller-level truncation-regression test alongside existing test_jql_* cases)
breaking_change: false
producer: orchestrator
version: "1.0.0"
last_updated: 2026-05-13
depends_on: []
blocks: []
issue: 350
public_spec: docs/specs/2026-05-13-search-issue-keys.md
research_report: .factory/research/issue-350-search-issue-keys-design.md
---

# S-350: Lightweight `search_issue_keys` API for JQL bulk-edit selection

## Context

`JiraClient::search_issues` always requests `BASE_ISSUE_FIELDS` (summary, status,
description, issuelinks, labels, components, fixVersions, parent, etc. — see
`src/api/jira/issues.rs:12-29`) regardless of how the caller uses the result.

`src/cli/issue/create.rs:386` in `handle_edit::effective_keys` (the JQL-driven
bulk-edit selection path) calls `.search_issues(jql_str, Some(effective_max + 1), &[])`
and immediately reduces to `Vec<String>` via `.into_iter().map(|i| i.key).collect()`.
Every body byte that came back is wasted. For a 100-issue match with ~10 KB
descriptions, that is ~1 MB of unnecessary data per JQL search.

The fix is a new lightweight method that asks Jira for only the keys.

Issue [#350](https://github.com/Zious11/jira-cli/issues/350) is the audit-followup
tracking this regression. Source: F5 adversarial review of PR #348 (issue #110 PR2),
2026-05-10, Copilot review round 7.

Full design rationale, validated API facts, and rejected alternatives live in
the public spec at `docs/specs/2026-05-13-search-issue-keys.md`. Research
validation lives at `.factory/research/issue-350-search-issue-keys-design.md`
(including a Q1–Q8 adversarial-pass addendum).

## Behavioral Contracts

**BC-2.6.050** (new — defined in this story; added to
`.factory/specs/prd/bc-2-issue-read.md` subdomain 2.6 after BC-2.6.049).

`client.search_issue_keys(jql, limit)` MUST:

1. POST `/rest/api/3/search/jql` with body `fields: ["key"]` and ONLY that
   value in the `fields` array. Never include `BASE_ISSUE_FIELDS` content.
2. Deserialize only the **top-level** `key` of each issue object — never
   read from `fields{}` body content. Tolerate unknown top-level fields
   silently (no `#[serde(deny_unknown_fields)]`).
3. Paginate via `nextPageToken` cursor identically to `search_issues`,
   including the JRACLOUD-94632 repeated-cursor anti-loop guard with the
   same stderr warning text.
4. Return `KeySearchResult { keys: Vec<String>, has_more: bool }` where
   `has_more = true` iff caller-side truncation occurred (the `limit`
   argument was hit AND the upstream API still had rows). Pure cursor
   exhaustion always returns `has_more = false`.
5. Clamp `maxResults` per page to `.min(100)` for parity with
   `search_issues` (NOT an API limit; conservative client-side choice).

**Regression invariant.** `search_issues`, `get_issue`, and every existing
caller of either MUST observe identical behavior to today. This story is
purely additive on the public API surface. No silent semantic shifts.

## Acceptance Criteria

**AC-001** (traces to BC-2.6.050 §1). Integration test
`tests/search_issue_keys.rs::test_search_issue_keys_sends_fields_key_only`:
calls `search_issue_keys`, then captures the request via
`MockServer::received_requests().await` and asserts
`body["fields"] == json!(["key"])` exactly. Length-strict because
`assert_eq!` on `serde_json::Value` is exact for arrays (unlike
`wiremock::body_partial_json` which uses `assert_json_include` subset
semantics — verified 2026-05-13 against wiremock 0.6.5). Test fails if
the request body's `fields` array contains anything other than exactly
`["key"]`. Pins the perf goal.

**AC-002** (traces to BC-2.6.050 §2). Integration tests
`test_search_issue_keys_happy_path` (3 keys round-trip),
`test_search_issue_keys_ignores_unknown_fields` (extra top-level fields
silently dropped), and `test_search_issue_keys_malformed_json_returns_error`
(corrupted body propagates `Err`). Pins the deserialization contract.

**AC-003** (traces to BC-2.6.050 §3). Four integration tests pin the
pagination contract:
1. `test_search_issue_keys_paginates_with_next_page_token` — two-page
   cursor walk; both mocks `.expect(1)`.
2. `test_search_issue_keys_repeated_cursor_aborts_with_warning` —
   JRACLOUD-94632 guard fires; loop aborts; only page 1's keys are returned.
3. `test_search_issue_keys_stderr_emits_jracloud_94632_literal` — subprocess
   test that spawns `jr` and asserts `stderr.contains("JRACLOUD-94632")`.
   Pairs with test 2 because `eprintln!` cannot be captured inside a
   library-level test.
4. `test_search_issue_keys_401_mid_pagination_propagates` — HTTP 401 on
   page 2 returns `Err`, not a partial-success.

**AC-004** (traces to BC-2.6.050 §4). Integration tests
`test_search_issue_keys_truncates_at_limit_and_sets_has_more`
(`limit = Some(2)`, server returns 3 → `keys.len() == 2`, `has_more == true`),
`test_search_issue_keys_returns_empty_for_no_matches`
(server returns `{"issues": []}` → `keys.is_empty()`, `has_more == false`),
and `test_search_issue_keys_apr2025_regression_bound` (server returns 500
rows for `maxResults = 10`, caller `limit = Some(10)` → exactly 10 returned,
`has_more == true`; defense-in-depth against the Atlassian Apr 2025
`maxResults` regression). Pins the `has_more` semantics and local-truncate
contract.

**AC-005** (traces to regression invariant). Caller-level integration test
added to `tests/issue_bulk_pr2.rs` (alongside the existing `test_jql_*`
cases), named
`test_handle_edit_jql_truncation_error_still_triggers_after_migration`:
runs `jr issue edit --jql '<q>' --max 5 --label add:foo` with wiremock
returning 7 keys; asserts the existing "JQL matched at least 6 issues,
which exceeds --max 5" error path still fires after the migration. Existing
`test_jql_default_max_50_caps_matched_issues`,
`test_jql_with_max_75_allows_75_matched`, and
`test_jql_max_above_1000_clamps_or_errors` continue to pass unchanged.

**AC-006** (traces to BC-2.6.050 — documentation arm). `KeySearchResult`
rustdoc:

1. References `BC-2.6.050` by name.
2. States the `has_more` semantics: `true` iff caller-side truncation only;
   pure exhaustion always returns `false`.
3. Documents the relationship to `SearchResult` (parallel concrete type,
   `keys` field mirrors `issues`).

`search_issue_keys` rustdoc:

1. References `BC-2.6.050` by name.
2. States the perf intent (avoids `BASE_ISSUE_FIELDS` payload).
3. Points callers to `search_issues` for body-bearing reads.

**AC-007** (release-gate). `cargo test`, `cargo clippy -- -D warnings`,
`cargo fmt --check`, and `scripts/check-spec-counts.sh` all pass.

## Implementation Sketch

```rust
// src/api/jira/issues.rs

#[derive(Debug, Clone, PartialEq)]
pub struct KeySearchResult {
    pub keys: Vec<String>,
    pub has_more: bool,
}

#[derive(Deserialize)]
struct IssueKeyRow {
    key: String,
    // No deny_unknown_fields — Atlassian routinely adds top-level fields.
}

impl JiraClient {
    /// Search issues using JQL and return ONLY the matching issue keys.
    ///
    /// Lightweight variant of `search_issues` — requests `fields: ["key"]`
    /// in the POST body and deserializes only the top-level `key`, avoiding
    /// the ~10 KB-per-issue payload that `BASE_ISSUE_FIELDS` incurs. Use
    /// this when the caller only needs keys; for body-bearing reads use
    /// `search_issues`.
    ///
    /// `has_more = true` iff the caller's `limit` was hit AND the upstream
    /// API still had rows; pure cursor exhaustion returns `has_more = false`.
    ///
    /// Traces to BC-2.6.050.
    pub async fn search_issue_keys(
        &self,
        jql: &str,
        limit: Option<u32>,
    ) -> Result<KeySearchResult> {
        let max_per_page = limit.unwrap_or(50).min(100);
        let mut all_keys: Vec<String> = Vec::new();
        let mut next_page_token: Option<String> = None;
        let mut more_available = false;
        let mut prev_cursor: Option<String> = None;

        loop {
            let mut body = serde_json::json!({
                "jql": jql,
                "maxResults": max_per_page,
                "fields": ["key"],
            });
            if let Some(ref token) = next_page_token {
                body["nextPageToken"] = serde_json::json!(token);
            }

            let page: CursorPage<IssueKeyRow> =
                self.post("/rest/api/3/search/jql", &body).await?;

            let page_has_more = page.has_more();
            let next_cursor = page.next_page_token.clone();
            all_keys.extend(page.issues.into_iter().map(|r| r.key));

            if let Some(max) = limit {
                if all_keys.len() >= max as usize {
                    more_available = all_keys.len() > max as usize || page_has_more;
                    all_keys.truncate(max as usize);
                    break;
                }
            }

            if !page_has_more { break; }

            // JRACLOUD-94632 anti-loop guard (verbatim from search_issues).
            if next_cursor.is_some() && next_cursor == prev_cursor {
                eprintln!(
                    "[jr] WARNING: Atlassian /rest/api/3/search/jql returned the same \
                     nextPageToken twice — aborting pagination loop. Some results may \
                     be missing. Upstream bug: JRACLOUD-94632."
                );
                break;
            }

            prev_cursor = next_cursor.clone();
            next_page_token = next_cursor;
        }

        Ok(KeySearchResult {
            keys: all_keys,
            has_more: more_available,
        })
    }
}
```

```rust
// src/cli/issue/create.rs — caller migration (lines 374-409)

let effective_keys: Vec<String> = if let Some(ref jql_str) = jql {
    if jql_str.trim().is_empty() {
        return Err(JrError::UserError(/* unchanged */).into());
    }

    let search_result = client
        .search_issue_keys(jql_str, Some(effective_max + 1))   // was: search_issues(..., &[])
        .await?;
    let matched_keys = search_result.keys;                      // was: search_result.issues

    if matched_keys.is_empty() {
        return Err(JrError::UserError(/* unchanged "matched 0 issues" */).into());
    }
    if matched_keys.len() > effective_max as usize {
        return Err(JrError::UserError(/* unchanged "matched at least N" */).into());
    }

    matched_keys                                                // was: matched.into_iter().map(|i| i.key).collect()
} else {
    /* positional keys path — unchanged */
};
```

## Tasks (TDD order)

1. **Red.** Write the 11 failing tests in `tests/search_issue_keys.rs` (AC-001..AC-004 + happy path + edge cases). Confirm all 11 fail with a clear "no such method" / "unresolved import" error before any impl lands.
2. **Red.** Write the failing caller-level test `test_handle_edit_jql_truncation_error_still_triggers_after_migration` in `tests/edit_bulk_jql.rs` (or the sibling that hosts existing JQL bulk-edit tests). It will currently pass because the migration hasn't happened — convert it to a regression-pin asserting the post-migration behavior.
3. **Green.** Add `KeySearchResult` struct with derives + `IssueKeyRow` private helper + `search_issue_keys` method body in `src/api/jira/issues.rs`. Tests 1–10 now pass.
4. **Green.** Migrate the caller in `src/cli/issue/create.rs:374-409`. Regression test from step 2 passes.
5. **Green / docs.** Add BC-2.6.050 to `.factory/specs/prd/bc-2-issue-read.md`
   after BC-2.6.049. Bump THREE counts in lockstep:
   - `bc-2-issue-read.md` frontmatter `definitional_count`: 49 → 50.
   - `BC-INDEX.md` frontmatter `total_bcs`: 545 → 546.
   - `BC-INDEX.md` `sections:` line for `bc-2-issue-read.md`:
     `49 individually-bodied` → `50 individually-bodied`.

   Run `scripts/check-spec-counts.sh` to verify no drift (per DRIFT-001
   mitigation).
6. **Green / docs.** One-line update to `CLAUDE.md`'s `src/api/jira/issues.rs` blurb.
7. **Refactor.** Polish rustdoc per AC-006 (cross-reference BC-2.6.050, document `has_more` semantics).
8. **Regress.** Run full test suite + clippy + fmt. AC-005 + AC-007.
9. **Follow-up issue.** File `chore(search): tighten JRACLOUD-94632 stderr warning — repeated cursors can be live-data drift, not just server bug` citing JRACLOUD-95368. This is documented in the public spec's "Out of Scope / Follow-ups" section. Step happens during PR creation, not in the diff.

## Files Modified

- `src/api/jira/issues.rs` — add `KeySearchResult`, `IssueKeyRow`, `search_issue_keys`.
- `src/cli/issue/create.rs` — migrate JQL bulk-edit selection at lines 374-409.
- `CLAUDE.md` — one-line update to `src/api/jira/issues.rs` blurb.
- `.factory/specs/prd/bc-2-issue-read.md` — add BC-2.6.050 in subdomain 2.6.
- `.factory/specs/prd/BC-INDEX.md` — frontmatter `total_bcs` count bump.

## Files NOT Modified (Regression Baseline)

- `search_issues`, `get_issue`, and every other public method on `JiraClient` — zero signature or behavioral change.
- `BASE_ISSUE_FIELDS` constant — kept as-is.
- `list.rs`, `view.rs`, `assets.rs` — they consume body fields; no migration.
- `CursorPage`, `src/api/pagination.rs` — reused unchanged.
- All other CLI subcommands — zero touch.

## Risk / Notes

- **Inconclusive `fields{}` echo.** Some sources suggested Jira may echo `key` inside `fields: {"key": "..."}` for `fields: ["key"]` requests. No empirical capture in published docs. Deserialization reads only the top-level `key` — if Jira ever inverts this, tests 2/3/8 fail loudly with empty-string keys. See public spec Validated API Facts §6.
- **JRACLOUD-94632 stderr text overclaims "server bug".** Live-data drift (JRACLOUD-95368) can also trigger the guard. Inherited verbatim by design §4; tightened in a follow-up PR. Not blocking.
- **`has_more` semantic drift risk.** A future caller could assume `has_more` reflects API state; rustdoc + AC-004 test pin the caller-side-truncation-only contract.
- **Refactor preservation.** Migration at create.rs:386 must preserve ALL existing error-message text exactly: "matched 0 issues" and "matched at least N issues" remain unchanged.
- **Length-strict `fields` enforcement at AC-001 (addendum §Q3 — RETRACTED).** `wiremock::matchers::body_partial_json` uses `assert_json_diff::assert_json_include` which has SUBSET semantics for arrays — a matcher built from `["key"]` would silently pass on a body with `["key", "summary", ...]`. Retracted via Perplexity verification 2026-05-13 against wiremock 0.6.5 source. AC-001 instead asserts on the captured request via `MockServer::received_requests().await` + `assert_eq!` on `serde_json::Value`, which IS length-strict for arrays.

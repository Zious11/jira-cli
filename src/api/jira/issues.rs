use crate::api::client::JiraClient;
use crate::api::pagination::{CursorPage, OffsetPage};
use crate::types::jira::{Comment, CreateIssueResponse, EditMeta, Issue, TransitionsResponse};
use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

/// Default fields requested when fetching issues (search and get).
///
/// Both `search_issues` and `get_issue` use this list so they stay in sync.
/// Callers can request additional fields via `extra_fields` parameters.
const BASE_ISSUE_FIELDS: &[&str] = &[
    "summary",
    "status",
    "issuetype",
    "priority",
    "assignee",
    "reporter",
    "project",
    "description",
    "created",
    "updated",
    "resolution",
    "components",
    "fixVersions",
    "labels",
    "parent",
    "issuelinks",
];

/// Result of a paginated issue search, including a flag indicating whether
/// the result set may be incomplete (caller-limit truncation OR
/// repeated-cursor guard abort).
///
/// **Invariant:** `issues` is always returned duplicate-free (first-occurrence
/// order preserved). `search_issues` applies per-iteration order-preserving
/// deduplication keyed on `issue.key` on every exit path: limit-truncation,
/// repeated-cursor guard abort, and clean cursor exhaustion (introduced in
/// issue #365). Callers can rely on this guarantee regardless of which exit
/// fires. To minimize live-data drift in the first place, use `key ASC` as a
/// stable secondary sort (append `, key ASC` to an existing sort, or use
/// `ORDER BY key ASC` if none — JQL allows only one ORDER BY clause) —
/// Atlassian's KB mitigation for snapshot instability per JRACLOUD-95368.
///
/// `has_more` is set to `true` in two cases (parallel to [`KeySearchResult`]):
///
/// 1. **Caller limit hit:** the caller supplied a `limit` and upstream still
///    had rows available beyond that limit.
/// 2. **Repeated-cursor guard abort:** the anti-loop guard fired (upstream
///    returned the same `nextPageToken` twice — typically live-data drift
///    per JRACLOUD-95368), pagination was aborted with a stderr warning,
///    and the result set may be **incomplete**.
///
/// Pure cursor exhaustion (no limit set, upstream returns no
/// `nextPageToken` — `CursorPage::has_more()` checks `next_page_token.is_some()`,
/// not the protocol-level `isLast` field which this client does not
/// deserialize) always returns `has_more = false`. When `limit` is set,
/// callers cannot distinguish case 1 from case 2 from `has_more` alone —
/// the stderr warning fires only in case 2. When `limit` is `None`, case 1
/// cannot trigger, so `has_more = true` unambiguously signals case 2
/// (repeated-cursor guard abort).
#[derive(Debug)]
pub struct SearchResult {
    pub issues: Vec<Issue>,
    pub has_more: bool,
}

/// Result of a keys-only paginated issue search.
///
/// Parallel to [`SearchResult`]. The field name `keys` mirrors the `issues`
/// field name on `SearchResult` (domain-named, not generic `items`) per the
/// Rust SDK precedent surveyed in
/// `.factory/research/issue-350-search-issue-keys-design.md`.
///
/// **Invariant:** `keys` is always returned duplicate-free (order-preserving,
/// first occurrence wins). `search_issue_keys` applies per-iteration
/// order-preserving deduplication on every exit path: limit-truncation,
/// repeated-cursor guard abort, and clean cursor exhaustion (introduced in
/// issue #365). Callers can rely on this guarantee regardless of which exit
/// fires. To minimize live-data drift in the first place, use `key ASC` as a
/// stable secondary sort (append `, key ASC` to an existing sort, or use
/// `ORDER BY key ASC` if none — JQL allows only one ORDER BY clause) —
/// Atlassian's KB mitigation for snapshot instability per JRACLOUD-95368.
///
/// `has_more` is set to `true` in two cases:
///
/// 1. **Caller limit hit:** the caller supplied a `limit` and upstream still
///    had rows available beyond that limit.
/// 2. **Repeated-cursor guard abort:** the anti-loop guard fired (upstream
///    returned the same `nextPageToken` twice — typically live-data drift
///    per JRACLOUD-95368, "nextPageToken pagination is not snapshot-stable
///    under live mutation"), pagination was aborted with a stderr warning,
///    and the result set may be **incomplete**.
///
/// When `limit` is set, callers cannot distinguish case 1 from case 2 from
/// `has_more` alone — the stderr warning fires only in case 2. When `limit`
/// is `None`, case 1 cannot trigger, so `has_more = true` unambiguously
/// signals case 2 (repeated-cursor guard abort).
///
/// Pure cursor exhaustion (no limit set, upstream returns no
/// `nextPageToken` — `CursorPage::has_more()` checks `next_page_token.is_some()`,
/// not the protocol-level `isLast` field which this client does not
/// deserialize) always returns `has_more = false`. Callers that need completeness
/// guarantees should treat `has_more = true` as "results may be truncated"
/// regardless of whether a `limit` was supplied.
///
/// Traces to BC-2.6.050.
#[derive(Debug, Clone, PartialEq)]
pub struct KeySearchResult {
    pub keys: Vec<String>,
    pub has_more: bool,
}

#[derive(Deserialize)]
struct ApproximateCountResponse {
    count: u64,
}

/// Minimal deserialization target for `search_issue_keys`.
///
/// Reads ONLY the top-level `key` field per issue. Atlassian guarantees
/// `key` is at the top level regardless of the request body's `fields`
/// value; the `fields` map itself is intentionally not deserialized here
/// to keep the payload contract narrow.
///
/// Does NOT use `#[serde(deny_unknown_fields)]` — Atlassian routinely
/// adds top-level response fields and the SDK convention is to ignore
/// unknowns silently.
///
/// `Default` is required here, but NOT for the fundamental reason one might
/// assume: `Vec<T>::default()` returns `vec![]` for ANY `T`, so the runtime
/// semantics of `#[serde(default)]` on `CursorPage::issues: Vec<T>` do NOT
/// actually require `T: Default`.
///
/// The requirement comes from serde-derive's CONSERVATIVE bound-inference
/// algorithm: when `#[serde(default)]` appears on a field of type `Vec<T>`,
/// the generated `Deserialize` impl adds `T: Default` to its where-clause
/// regardless of whether the bound is logically necessary. Removing `Default`
/// from `IssueKeyRow` produces:
///   error[E0277]: the trait bound `IssueKeyRow: Default` is not satisfied
///   required for `CursorPage<IssueKeyRow>` to implement `DeserializeOwned`
/// This is a known serde-derive limitation (conservative macro bounds), not a
/// fundamental requirement of `Vec` or serde's runtime logic. The `Default`
/// value is never used at runtime.
#[derive(Deserialize, Default)]
struct IssueKeyRow {
    key: String,
}

impl JiraClient {
    /// Search issues using JQL with cursor-based pagination.
    pub async fn search_issues(
        &self,
        jql: &str,
        limit: Option<u32>,
        extra_fields: &[&str],
    ) -> Result<SearchResult> {
        let max_per_page = limit.unwrap_or(50).min(100);
        let mut all_issues: Vec<Issue> = Vec::new();
        let mut next_page_token: Option<String> = None;

        let mut fields = BASE_ISSUE_FIELDS.to_vec();
        fields.extend_from_slice(extra_fields);

        let mut more_available = false;

        // Incremental order-preserving dedupe: JRACLOUD-95368 drift can emit the
        // same issue on multiple pages. Issue does not impl Hash so we key on
        // issue.key (String). Maintained outside the loop so each unique key is
        // stored once and we avoid rescanning the accumulated Vec on every page —
        // O(N) total vs the O(N²) that a per-iteration retain+rebuild would
        // incur on large result sets. Duplicates still get hashed on insert
        // attempts (HashSet::insert always hashes its argument) but are never
        // appended to all_issues. See #365.
        let mut seen_keys: HashSet<String> = HashSet::new();

        // Anti-loop guard: protect against Jira Cloud /rest/api/3/search/jql
        // returning the same nextPageToken twice, which would otherwise cause
        // an infinite pagination loop. The documented root cause class —
        // "snapshot instability under live mutation" — is described in
        // Atlassian's KB on inconsistent paginated JQL search results, with
        // related ticket JRACLOUD-95368 ("nextPageToken pagination is not
        // snapshot-stable under live mutation"). `nextPageToken` encodes a
        // position in the live result set rather than a snapshot, so drift in
        // the underlying data can land the server on a previously emitted
        // offset. JRACLOUD-95368's public Description names duplicates/skips
        // as the documented symptoms; cursor-repetition is one inferential
        // step (same root cause class, observed differently). The recommended
        // caller-side mitigation per Atlassian KB is a stable secondary sort —
        // append `, key ASC` to an existing ORDER BY, or use `ORDER BY key ASC`
        // if none (JQL allows only one ORDER BY clause). Symptoms have also been reported anecdotally
        // in atlassian/atlassian-mcp-server#118 and ankitpokhrel/jira-cli#898.
        // Follows the same defensive intent as the "did pagination advance?"
        // guard in `get_changelog` (which uses `next <= start_at` on
        // offset-based pagination); here the equivalent is `next_cursor ==
        // prev_cursor` on cursor-based pagination. When the guard fires we
        // emit a stderr warning citing JRACLOUD-95368 so users have a search
        // term and an actionable mitigation.
        let mut prev_cursor: Option<String> = None;

        loop {
            let mut body = serde_json::json!({
                "jql": jql,
                "maxResults": max_per_page,
                "fields": fields
            });

            if let Some(ref token) = next_page_token {
                body["nextPageToken"] = serde_json::json!(token);
            }

            let page: CursorPage<Issue> = self.post("/rest/api/3/search/jql", &body).await?;

            let page_has_more = page.has_more();
            let next_cursor = page.next_page_token.clone();

            // Append only issues not yet seen — first-occurrence order preserved.
            // `seen_keys` lives outside the loop; unique keys are stored once
            // and the accumulated Vec is never rescanned per-iteration.
            for issue in page.issues {
                if seen_keys.insert(issue.key.clone()) {
                    all_issues.push(issue);
                }
            }

            if let Some(max) = limit {
                if all_issues.len() >= max as usize {
                    more_available = all_issues.len() > max as usize || page_has_more;
                    all_issues.truncate(max as usize);
                    break;
                }
            }

            if !page_has_more {
                break;
            }

            // GUARD: detect repeated cursor token (next == prev) → abort + warn.
            // NFR-R-F: prevents infinite loop when the server returns the same
            // nextPageToken twice (typically JRACLOUD-95368 live-data drift).
            // Stderr-literal pin: tests/rate_limit_cap_tests.rs::ac_008_and_ac_new_d_search_jql_cursor_loop_terminates_with_jracloud_warning
            // Do NOT change the literal "JRACLOUD-95368" without updating that test.
            if next_cursor.is_some() && next_cursor == prev_cursor {
                eprintln!(
                    "[jr] WARNING: Atlassian /rest/api/3/search/jql returned the same \
                     nextPageToken twice — aborting pagination to prevent an infinite \
                     loop. Some results may be missing. Likely cause: live data \
                     mutation between page fetches (snapshot-instability, \
                     JRACLOUD-95368). Mitigation: end your JQL with `key ASC` in the \
                     ORDER BY (append `, key ASC` to an existing sort, or use \
                     `ORDER BY key ASC` if none)."
                );
                // Guard-aborted: signal incomplete results via has_more=true so
                // callers can distinguish "clean exhaustion" from
                // "repeated-cursor abort". Matches the `KeySearchResult`
                // contract for symmetry; otherwise `SearchResult.has_more`
                // would silently be `false` despite the explicit
                // "Some results may be missing" warning above. As of #365,
                // all_issues is already deduplicated (keyed on issue.key) by
                // the incremental seen_keys HashSet maintained outside the loop.
                // No additional dedupe call is needed here.
                more_available = true;
                break;
            }

            prev_cursor = next_cursor.clone();
            next_page_token = next_cursor;
        }

        Ok(SearchResult {
            issues: all_issues,
            has_more: more_available,
        })
    }

    /// Search issues using JQL and return ONLY the matching issue keys.
    ///
    /// Lightweight variant of [`Self::search_issues`] — requests
    /// `fields: ["key"]` in the POST body and deserializes only the
    /// top-level `key`, avoiding the ~10 KB-per-issue payload that
    /// `BASE_ISSUE_FIELDS` incurs.
    ///
    /// Use this when the caller only needs keys (e.g., JQL-driven
    /// bulk-edit selection at `cli/issue/create.rs::handle_edit`). For
    /// body-bearing reads use [`Self::search_issues`].
    ///
    /// `has_more` is set to `true` in two cases: (1) the caller's `limit`
    /// was hit AND upstream still had rows available (no guard-induced
    /// duplicates), or (2) the repeated-cursor anti-loop guard fired
    /// (upstream returned the same `nextPageToken` twice — typically
    /// live-data drift per JRACLOUD-95368), aborting pagination early with
    /// a stderr warning (keys may be **incomplete** — as of issue #365,
    /// duplicates are eliminated client-side on this path via per-iteration
    /// order-preserving deduplication; callers should still prefer
    /// `key ASC` in the ORDER BY (append `, key ASC` to an existing sort,
    /// or use `ORDER BY key ASC` if none — JQL allows only one ORDER BY
    /// clause) — Atlassian's KB mitigation). Pure cursor exhaustion returns
    /// `has_more = false`. See [`KeySearchResult`] for the full contract.
    ///
    /// The per-page clamp `.min(100)` is a conservative client-side choice
    /// for parity with `search_issues`; Atlassian docs note that id/key-only
    /// requests can return more rows per page than full-body requests.
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

        // Incremental order-preserving dedupe: JRACLOUD-95368 drift can emit the
        // same key on multiple pages. Maintained outside the loop so each unique
        // key is stored once and we avoid rescanning the accumulated Vec on every
        // page — O(N) total vs the O(N²) that a per-iteration retain+rebuild
        // would incur on large result sets. Duplicates still get hashed on insert
        // attempts (HashSet::insert always hashes its argument) but are never
        // appended to all_keys. Applied before the limit-truncation check so
        // `all_keys.len()` reflects the unique-key count when the truncation
        // sentinel fires. See #365.
        let mut seen_keys: HashSet<String> = HashSet::new();

        // Anti-loop guard: protect against Jira Cloud /rest/api/3/search/jql
        // returning the same nextPageToken twice (typically JRACLOUD-95368
        // live-data drift — `nextPageToken` encodes a non-snapshot position so
        // mutations between page fetches can land the server on a previously
        // emitted offset). Mirrors the guard in `search_issues` above — see
        // there for the full root-cause discussion and citations.
        let mut prev_cursor: Option<String> = None;

        loop {
            let mut body = serde_json::json!({
                "jql": jql,
                "maxResults": max_per_page,
                "fields": ["key"]
            });

            if let Some(ref token) = next_page_token {
                body["nextPageToken"] = serde_json::json!(token);
            }

            let page: CursorPage<IssueKeyRow> = self.post("/rest/api/3/search/jql", &body).await?;

            let page_has_more = page.has_more();
            let next_cursor = page.next_page_token.clone();

            // Append only keys not yet seen — first-occurrence order preserved.
            // `seen_keys` lives outside the loop; unique keys are stored once
            // and the accumulated Vec is never rescanned per-iteration.
            for key in page.issues.into_iter().map(|r| r.key) {
                if seen_keys.insert(key.clone()) {
                    all_keys.push(key);
                }
            }

            if let Some(max) = limit {
                if all_keys.len() >= max as usize {
                    // `all_keys.len() > max` handles the Apr 2025 regression
                    // (community.developer.atlassian.com thread 88287; see Validated
                    // API Facts §7 in docs/specs/2026-05-13-search-issue-keys.md)
                    // where the server overshoots maxResults AND sets isLast:true —
                    // the overshoot proves more data existed.
                    // `page_has_more` handles the normal "server said more pages" case.
                    // Do NOT simplify to `page_has_more` alone — that would miss the
                    // regression scenario.
                    more_available = all_keys.len() > max as usize || page_has_more;
                    all_keys.truncate(max as usize);
                    break;
                }
            }

            if !page_has_more {
                break;
            }

            // GUARD: detect repeated cursor token (next == prev) → abort + warn.
            // Mirrors the repeated-cursor guard block in search_issues above — same prev/next cursor check, same stderr warning text.
            // Stderr-literal pin: tests/search_issue_keys.rs::test_search_issue_keys_stderr_emits_jracloud_95368_literal
            // Do NOT change the literal "JRACLOUD-95368" without updating that test.
            if next_cursor.is_some() && next_cursor == prev_cursor {
                eprintln!(
                    "[jr] WARNING: Atlassian /rest/api/3/search/jql returned the same \
                     nextPageToken twice — aborting pagination to prevent an infinite \
                     loop. Some results may be missing. Likely cause: live data \
                     mutation between page fetches (snapshot-instability, \
                     JRACLOUD-95368). Mitigation: end your JQL with `key ASC` in the \
                     ORDER BY (append `, key ASC` to an existing sort, or use \
                     `ORDER BY key ASC` if none)."
                );
                // Guard-aborted: signal incomplete results via has_more=true. As of
                // #365, all_keys is already deduplicated by the incremental
                // seen_keys HashSet maintained outside the loop. No additional
                // dedupe call is needed here.
                more_available = true;
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

    /// Get an approximate count of issues matching a JQL query.
    ///
    /// Uses the dedicated count endpoint which is lightweight (no issue data fetched).
    /// The JQL should not include ORDER BY — use `jql::strip_order_by()` before calling.
    pub async fn approximate_count(&self, jql: &str) -> Result<u64> {
        let body = serde_json::json!({ "jql": jql });
        let resp: ApproximateCountResponse = self
            .post("/rest/api/3/search/approximate-count", &body)
            .await?;
        Ok(resp.count)
    }

    /// Get a single issue by key.
    pub async fn get_issue(&self, key: &str, extra_fields: &[&str]) -> Result<Issue> {
        let mut fields: Vec<&str> = BASE_ISSUE_FIELDS.to_vec();
        fields.extend_from_slice(extra_fields);
        let path = format!(
            "/rest/api/3/issue/{}?fields={}",
            urlencoding::encode(key),
            fields.join(",")
        );
        self.get(&path).await
    }

    /// Create a new issue.
    pub async fn create_issue(&self, fields: Value) -> Result<CreateIssueResponse> {
        let body = serde_json::json!({ "fields": fields });
        self.post("/rest/api/3/issue", &body).await
    }

    /// Edit an existing issue's fields.
    pub async fn edit_issue(&self, key: &str, fields: Value) -> Result<()> {
        let path = format!("/rest/api/3/issue/{}", urlencoding::encode(key));
        let body = serde_json::json!({ "fields": fields });
        self.put(&path, &body).await
    }

    /// Update a single issue's labels using the `update` verb (PUT issue).
    ///
    /// Sends `PUT /rest/api/3/issue/{key}` with body:
    /// ```json
    /// {"update": {"labels": [{"add": "foo"}, {"remove": "bar"}]}}
    /// ```
    ///
    /// Label values are **bare strings** — NOT `{"name":"foo"}` objects. The bare-string
    /// form is required by the Jira Cloud REST API v3 `update` verb; the `{"name":...}`
    /// form is only valid on the bulk fields endpoint. Source: Atlassian Cloud REST API v3
    /// `PUT /rest/api/3/issue/{issueIdOrKey}` "update" verb documentation
    /// (<https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/#api-rest-api-3-issue-issueidorkey-put>);
    /// empirically validated by live E2E run 26730687481 (the bulk-payload shape returned
    /// HTTP 400 on real Jira Cloud instances).
    ///
    /// Returns `Ok(())` on HTTP 204 No Content. Any other status propagates as an error.
    ///
    /// At least one of `adds` or `removes` must be non-empty (caller's responsibility).
    pub async fn update_issue_labels(
        &self,
        key: &str,
        adds: &[String],
        removes: &[String],
    ) -> Result<()> {
        debug_assert!(
            !adds.is_empty() || !removes.is_empty(),
            "update_issue_labels: at least one of adds or removes must be non-empty",
        );
        let mut label_ops: Vec<serde_json::Value> = Vec::new();
        for name in adds {
            label_ops.push(serde_json::json!({"add": name}));
        }
        for name in removes {
            label_ops.push(serde_json::json!({"remove": name}));
        }
        let path = format!("/rest/api/3/issue/{}", urlencoding::encode(key));
        let body = serde_json::json!({
            "update": {
                "labels": label_ops
            }
        });
        self.put(&path, &body).await
    }

    /// Fetch the editmeta for an issue.
    ///
    /// `GET /rest/api/3/issue/{key}/editmeta` returns the set of fields that
    /// are on the issue's agent Edit screen, including their schemas and
    /// `allowedValues`. Called by `resolve_edit_fields` (BC-3.4.015 Step 3)
    /// when `--field` is set; skipped entirely when `--field` is absent so
    /// existing `issue edit` invocations are byte-for-byte unchanged in latency.
    ///
    /// The response is NOT cached — it is issue-specific and mutable (an admin
    /// can change the Edit screen at any time), so caching risks stale
    /// `allowedValues` producing wrong option IDs on the wire.
    pub async fn get_editmeta(&self, key: &str) -> Result<EditMeta> {
        let path = format!("/rest/api/3/issue/{}/editmeta", urlencoding::encode(key));
        self.get(&path).await
    }

    /// Get available transitions for an issue.
    pub async fn get_transitions(&self, key: &str) -> Result<TransitionsResponse> {
        let path = format!("/rest/api/3/issue/{}/transitions", urlencoding::encode(key));
        self.get(&path).await
    }

    /// Get available transitions for an issue, with field metadata expanded.
    ///
    /// Calls `GET /rest/api/3/issue/{key}/transitions?expand=transitions.fields`.
    /// The expanded response populates `Transition.fields` (the screen field
    /// metadata map) and `Transition.is_conditional` for each transition.
    ///
    /// Called by `handle_move` (single-key path) to enable proactive resolution
    /// enforcement (BC-3.2.013). `handle_transitions` (the `jr issue transitions`
    /// read command) continues to call `get_transitions` — these are DISTINCT
    /// methods with DISTINCT call sites per ADR-0015 §4.
    ///
    /// `Transition.fields` carries `#[serde(skip_serializing)]` so JSON
    /// serialization of `TransitionsResponse` is byte-for-byte unchanged from
    /// the plain `get_transitions` path. ADR-0015 §3 read-command-stability contract.
    pub async fn get_transitions_with_fields(&self, key: &str) -> Result<TransitionsResponse> {
        let path = format!(
            "/rest/api/3/issue/{}/transitions?expand=transitions.fields",
            urlencoding::encode(key)
        );
        self.get(&path).await
    }

    /// Transition an issue to a new status, optionally setting extra fields
    /// in the same request (e.g. `resolution`). Passing `fields = None`
    /// preserves the pre-existing behaviour of sending only the transition id.
    ///
    /// When `fields` is `Some(&json)`, the value is merged as-is under the
    /// `fields` key of the request body — callers are responsible for shaping
    /// it correctly (Atlassian expects `{"resolution": {"name": "Done"}}` or
    /// `{"resolution": {"id": "10000"}}`).
    pub async fn transition_issue(
        &self,
        key: &str,
        transition_id: &str,
        fields: Option<&serde_json::Value>,
    ) -> Result<()> {
        let path = format!("/rest/api/3/issue/{}/transitions", urlencoding::encode(key));
        let body = match fields {
            Some(f) => serde_json::json!({
                "transition": { "id": transition_id },
                "fields": f,
            }),
            None => serde_json::json!({
                "transition": { "id": transition_id }
            }),
        };
        self.post_no_content(&path, &body).await
    }

    /// Assign an issue. Pass `None` for account_id to unassign.
    pub async fn assign_issue(&self, key: &str, account_id: Option<&str>) -> Result<()> {
        let path = format!("/rest/api/3/issue/{}/assignee", urlencoding::encode(key));
        let body = serde_json::json!({
            "accountId": account_id
        });
        self.put(&path, &body).await
    }

    /// Add a comment to an issue.
    ///
    /// When `internal` is true, sets the `sd.public.comment` entity property
    /// to mark the comment as internal (agent-only) on JSM projects.
    /// On non-JSM projects, the property is silently accepted with no effect.
    pub async fn add_comment(&self, key: &str, body: Value, internal: bool) -> Result<Comment> {
        let path = format!("/rest/api/3/issue/{}/comment", urlencoding::encode(key));
        let mut payload = serde_json::json!({ "body": body });
        if internal {
            payload["properties"] = serde_json::json!([{
                "key": "sd.public.comment",
                "value": { "internal": true }
            }]);
        }
        self.post(&path, &payload).await
    }

    /// Fetch the full audit changelog for an issue.
    ///
    /// Offset-paginated under `values[]`. Always fetches every page;
    /// sort/filter/truncate are the caller's responsibility — the Jira
    /// changelog endpoint supports no server-side filters and does not
    /// guarantee sort order.
    pub async fn get_changelog(
        &self,
        key: &str,
    ) -> Result<Vec<crate::types::jira::ChangelogEntry>> {
        let base = format!("/rest/api/3/issue/{}/changelog", urlencoding::encode(key));
        let mut all = Vec::new();
        let mut start_at = 0u32;
        let max_page_size: u32 = 100;

        loop {
            let path = format!("{}?startAt={}&maxResults={}", base, start_at, max_page_size);
            let page: OffsetPage<crate::types::jira::ChangelogEntry> = self.get(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values.unwrap_or_default());

            if !has_more {
                break;
            }
            // Guard against an API response that advertises more pages but
            // returns a page that wouldn't advance `startAt` — otherwise we'd
            // infinite-loop on a malformed/empty page (JRACLOUD-94357-class
            // schema-drift scenarios). Surface as an explicit error instead.
            if next <= start_at {
                return Err(anyhow::anyhow!(
                    "Jira changelog pagination did not advance (startAt {} → {}) \
                     despite has_more=true. The server returned a malformed page; \
                     retry later or report to Jira support.",
                    start_at,
                    next
                ));
            }
            start_at = next;
        }

        Ok(all)
    }

    /// List comments on an issue with auto-pagination.
    pub async fn list_comments(&self, key: &str, limit: Option<u32>) -> Result<Vec<Comment>> {
        let base = format!("/rest/api/3/issue/{}/comment", urlencoding::encode(key));
        let mut all = Vec::new();
        let mut start_at = 0u32;
        let max_page_size: u32 = 100;

        loop {
            let page_size = match limit {
                Some(cap) => {
                    let remaining = cap.saturating_sub(all.len() as u32);
                    if remaining == 0 {
                        break;
                    }
                    remaining.min(max_page_size)
                }
                None => max_page_size,
            };
            let path = format!(
                "{}?startAt={}&maxResults={}&expand=properties",
                base, start_at, page_size
            );
            let page: OffsetPage<Comment> = self.get(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.append(&mut page.comments.unwrap_or_default());

            if let Some(cap) = limit {
                if all.len() >= cap as usize {
                    all.truncate(cap as usize);
                    break;
                }
            }
            if !has_more {
                break;
            }
            start_at = next;
        }
        Ok(all)
    }

    /// Resolve the available issue types for a project via the createmeta issuetypes endpoint.
    ///
    /// Calls `GET /rest/api/3/issue/createmeta/{projectKey}/issuetypes` and paginates by
    /// offset until `startAt + page_len >= total` (or an empty page), returning all
    /// `IssueTypeEntry` values (id + name).
    ///
    /// `PageOfCreateMetaIssueTypes` uses OFFSET pagination (`startAt`/`maxResults`/`total`).
    /// There is NO `isLast` field — that belongs to the generic `PageBean<T>` family, not
    /// the specialized `PageOf...` types. Verified against the Atlassian OpenAPI-derived
    /// `jira.js` client (issue #331; see
    /// `.factory/research/issue-331-createmeta-response-schema.md`).
    ///
    /// # Usage
    /// - No cache — one or more HTTP calls per `--type` bulk invocation.
    ///   Most projects have ≤50 types (one page at `maxResults=200`); pagination
    ///   is a correctness guard for large enterprise type schemes.
    /// - Project-scoped: the same type name can have different IDs in different projects.
    /// - Call site: `handle_edit_bulk_fields` in `src/cli/issue/create.rs` only.
    pub(crate) async fn get_issue_types_for_project(
        &self,
        project_key: &str,
    ) -> Result<Vec<IssueTypeEntry>> {
        // Reuse IssueTypeMetadata from projects.rs is not possible here — that struct
        // lacks an `id` field (it has name/description/subtask only). We define a
        // separate IssueTypeEntry with id + name for createmeta resolution.
        let page_size: u32 = 200;
        let mut all: Vec<IssueTypeEntry> = Vec::new();
        let mut start_at: u32 = 0;
        loop {
            let response: CreatemetaIssueTypesResponse = self
                .get(&format!(
                    "/rest/api/3/issue/createmeta/{}/issuetypes?startAt={}&maxResults={}",
                    urlencoding::encode(project_key),
                    start_at,
                    page_size,
                ))
                .await?;
            let total = response.total;
            let page_len = response.issue_types.len() as u32;
            all.extend(response.issue_types);
            // Offset termination: stop on empty page or once we've consumed `total`.
            // (`PageOfCreateMetaIssueTypes` has no `isLast`; total drives the loop.)
            if page_len == 0 || start_at + page_len >= total {
                break;
            }
            start_at += page_len;
        }
        Ok(all)
    }
}

/// Issue type entry returned by `GET /rest/api/3/issue/createmeta/{projectKey}/issuetypes`.
///
/// Contains the `id` (string) and `name` fields needed for bulk issue-type resolution.
/// The `id` field is the value used as `issueTypeId` in the bulk edit payload.
///
/// Note: `IssueTypeMetadata` in `src/api/jira/projects.rs` lacks the `id` field
/// (it has name/description/subtask only); this struct is a separate, minimal type
/// scoped to the createmeta resolution path.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct IssueTypeEntry {
    pub id: String,
    pub name: String,
}

/// Response wrapper for `GET /rest/api/3/issue/createmeta/{projectKey}/issuetypes`.
///
/// `PageOfCreateMetaIssueTypes` uses OFFSET pagination: `{issueTypes, startAt,
/// maxResults, total}`. There is NO `values` field and NO `isLast` field — those
/// belong to the generic `PageBean<T>` family, NOT to the specialized `PageOf...`
/// types. Termination is `startAt + issueTypes.len() >= total` (or an empty page).
/// Verified against the Atlassian OpenAPI-derived `jira.js` client and
/// developer.atlassian.com (issue #331; see
/// `.factory/research/issue-331-createmeta-response-schema.md`).
#[derive(Debug, serde::Deserialize)]
struct CreatemetaIssueTypesResponse {
    #[serde(rename = "issueTypes", default)]
    pub issue_types: Vec<IssueTypeEntry>,
    /// Total number of issue types across all pages. Defaults to 0 if absent
    /// (defensive — drives the offset-termination check).
    #[serde(default)]
    pub total: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_result_has_more_false_when_no_truncation() {
        let result = SearchResult {
            issues: vec![],
            has_more: false,
        };
        assert!(!result.has_more);
    }

    #[test]
    fn search_result_has_more_true_when_truncated() {
        let result = SearchResult {
            issues: vec![],
            has_more: true,
        };
        assert!(result.has_more);
    }

    #[test]
    fn approximate_count_response_deserializes() {
        let json = r#"{"count": 1234}"#;
        let resp: ApproximateCountResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.count, 1234);
    }

    #[test]
    fn approximate_count_response_zero() {
        let json = r#"{"count": 0}"#;
        let resp: ApproximateCountResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.count, 0);
    }

    /// Regression-pin: `PageOfCreateMetaIssueTypes` uses `issueTypes` (NOT `values`),
    /// offset pagination (`startAt`/`maxResults`/`total`), and NO `isLast` field.
    ///
    /// This test MUST FAIL before the fix (current struct requires `values`) and
    /// MUST PASS after the fix (struct renames field to `issueTypes`).
    ///
    /// Bug source: live E2E `test_e2e_issue_edit_issuetype_multikey_bulk_roundtrip`
    /// failed with "missing field `values`". Verified against Atlassian OpenAPI-derived
    /// jira.js client and developer.atlassian.com. See issue #331 and
    /// `.factory/research/issue-331-createmeta-response-schema.md`.
    #[test]
    fn test_createmeta_response_deserializes_issuetypes_field() {
        // This is the REAL shape returned by
        // GET /rest/api/3/issue/createmeta/{projectKey}/issuetypes.
        // It uses the top-level key `issueTypes` — NOT `values`.
        // There is no `isLast` field (offset pagination only).
        let json = r#"{
            "startAt": 0,
            "maxResults": 200,
            "total": 2,
            "issueTypes": [
                {"id": "10001", "name": "Bug"},
                {"id": "10002", "name": "Story"}
            ]
        }"#;
        let resp: CreatemetaIssueTypesResponse = serde_json::from_str(json)
            .expect("CreatemetaIssueTypesResponse must deserialize from real Jira API shape");
        assert_eq!(resp.issue_types.len(), 2, "Expected 2 issue types");
        assert_eq!(resp.issue_types[0].id, "10001");
        assert_eq!(resp.issue_types[0].name, "Bug");
        assert_eq!(resp.issue_types[1].id, "10002");
        assert_eq!(resp.issue_types[1].name, "Story");
        assert_eq!(resp.total, 2, "Expected total=2");
    }
}

# Pass 4 R1 — NFR Catalog Deepening (Cross-Pollination Round)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Round: 1
Builds on: Pass 4 broad (27 config values, 23 NFR gaps, 5 dimensions); Pass 2 R1-R7 deepening (cross-pollination items); Pass 3 R4 BCs.

> **Method.** R1 incorporates ~30 cross-pollination items surfaced by Pass 2 R1-R7 into the NFR catalog. Each becomes a sub-section under the appropriate NFR dimension with `<file>:<line>` citation, severity classification, and Phase 1 spec recommendation. Original Pass 4 broad findings are re-verified — some are refined or partially superseded by Pass 2 discoveries.

---

## 0. R1 Scope and Method

R1 targets the **cross-pollination backlog** that accumulated across Pass 2 R1-R7. Each Pass 2 deepening round identified findings that are NFR-flavored (silent data loss, inconsistent semantics, security gaps, observability holes) rather than pure domain-model findings. R1 is the first pass that consolidates those into the NFR catalog with explicit severity and Phase 1 disposition.

R1 is intentionally NOT yet doing function-level depth on broad Pass 4's 5 dimensions — that is reserved for R2-R3. R1 is "intake" of the cross-pollination items + re-verification of broad Pass 4 against Pass 2's deeper findings.

---

## 1. Reliability NFRs — Cross-Pollination Additions

### NFR-R-A: `list_worklogs` non-paginated; silent truncation past page 1

**Source ref:** Pass 2 R1 NEW-INV-29
**Code citation:** `src/api/jira/worklogs.rs:25-30`
```rust
pub async fn list_worklogs(&self, key: &str) -> Result<Vec<Worklog>> {
    let path = format!("/rest/api/3/issue/{}/worklog", urlencoding::encode(key));
    let page: OffsetPage<Worklog> = self.get(&path).await?;
    Ok(page.items().to_vec())  // ← page 1 only; no loop
}
```
**NFR dimension:** Reliability (correctness — silent data loss)
**Severity:** **HIGH** — issues with >50 worklogs return only the first page; users see no warning. Atlassian's default page size for `/issue/{key}/worklog` is 50 per page; an issue under heavy time-tracking can easily exceed this.
**Recommendation:** Wrap in a `paginate_offset` loop (same pattern as `list_comments`). Alternatively, add explicit truncation warning on stderr if `page.total > page.items().len()`. Phase 1 must classify as MUST-FIX correctness bug.

### NFR-R-B: `handle_open` uses API gateway URL for OAuth profiles — opening fails

**Source ref:** Pass 2 R3 NEW-INV-56
**Code citation:** `src/cli/issue/workflow.rs:636` — `client.base_url()` returns `api.atlassian.com/ex/jira/<cloudId>` for OAuth profiles, not the user-facing `*.atlassian.net` instance URL.
**NFR dimension:** Reliability (functional correctness)
**Severity:** **HIGH** — `jr issue open KEY` for any OAuth profile opens a 404 in the browser. The user-facing browser URL must be `instance_url`, not `base_url`.
**Recommendation:** Use `client.instance_url()` or thread through the configured profile's `url` field. Phase 1 must classify as MUST-FIX correctness bug.

### NFR-R-C: Hardcoded `hours_per_day=8`, `days_per_week=5` ignores Jira instance time-tracking settings

**Source ref:** Pass 2 R5 NEW-INV-81
**Code citation:** `src/cli/worklog.rs handle_add` — uses literal `8` / `5` for `1d` / `1w` expansion; Jira instances configure these via `/rest/api/3/configuration/timetracking`.
**NFR dimension:** Reliability (semantic correctness — produces wrong worklog seconds for non-default-configured instances)
**Severity:** **MEDIUM** — most users keep defaults, but instances configured with 7.5h/day or 4-day weeks get wrong totals. Silent wrong answer.
**Recommendation:** Fetch and cache time-tracking config (7-day TTL alongside other caches); fall back to 8/5 only on cache miss. Phase 1: MUST-FIX or DOCUMENT-AS-IS depending on user base.

### NFR-R-D: Multi-profile fields bug — runtime reads legacy `config.global.fields.*`, ignores per-profile `ProfileConfig.{story_points,team}_field_id`

**Source ref:** Pass 2 R1 NEW-INV-12, R7 NEW-INV-143
**Code citation:** 12 sites read `config.global.fields.story_points_field_id` / `team_field_id`; migration writes per-profile fields but runtime never reads them. Silent regression after `jr init` re-runs against a different profile.
**NFR dimension:** Reliability (correctness — multi-profile data leak / wrong custom field IDs sent to API)
**Severity:** **CRITICAL** — sandbox vs prod custom-field IDs differ. Sending prod's `customfield_10005` to a sandbox API silently writes to the wrong field or fails with 400. Cross-profile correctness bug exactly of the class CLAUDE.md flags as "correctness, not UX."
**Recommendation:** Replace all 12 reads with profile-aware accessor `config.field_id(FieldKind::StoryPoints, profile)`. Phase 1: MUST-FIX correctness bug.

### NFR-R-E: Multi-workspace asset HashMap mis-attribution

**Source ref:** Pass 2 R6 NEW-INV-229
**Code citation:** `src/api/assets/linked.rs` — `to_enrich` keyed by `(workspace_id, object_id)`; `resolved` keyed by `object_id` alone. Two assets with same object_id from different workspaces get cross-attributed.
**NFR dimension:** Reliability (correctness — silent data corruption in display)
**Severity:** **HIGH** — affects only multi-workspace orgs but produces silently-wrong asset display. Assets/CMDB users with multiple connected workspaces are exposed.
**Recommendation:** Use `(workspace_id, object_id)` as key throughout. Phase 1: MUST-FIX correctness bug.

### NFR-R-F: `get_changelog` anti-loop guard

**Source ref:** Pass 2 R1 NEW-INV-18
**Code citation:** `src/api/jira/issues.rs:222-230` — defensive break if `nextPage` URL equals current URL.
**NFR dimension:** Reliability (defense in depth)
**Severity:** **LOW** — defensive pattern, not a known bug. Already implemented correctly.
**Recommendation:** DOCUMENT-AS-IS as a defensive pattern worth preserving in any rewrite.

### NFR-R-G: User pagination advances by REQUESTED `USER_PAGE_SIZE` not returned-count

**Source ref:** Pass 2 R1 NEW-INV-19
**Code citation:** `src/api/jira/users.rs` user-search loop — `start_at += USER_PAGE_SIZE` (deliberate, JRACLOUD-71293 workaround).
**NFR dimension:** Reliability (deliberate bug-tolerance behavior)
**Severity:** **LOW** — already pinned by regression test `tests/user_pagination.rs`. Documented gotcha.
**Recommendation:** DOCUMENT-AS-IS. Phase 1 must call this out as deliberate workaround for Atlassian bug, not a bug-to-fix.

---

## 2. Security NFRs — Cross-Pollination Additions

### NFR-S-A: No PKCE in OAuth flow (re-confirmation from Pass 2)

**Source ref:** Pass 2 R6 NEW-INV-178; previously Pass 4 broad §7.2.6
**Code citation:** `src/api/auth.rs:608-616` — authorize URL builder includes `client_id`, `scope`, `redirect_uri`, `state`; no `code_challenge` / `code_challenge_method`.
**NFR dimension:** Security (defense in depth)
**Severity:** **MEDIUM** — confidential-client model with embedded `client_secret` mitigates the public-client PKCE need, but RFC 8252 recommends PKCE for native apps regardless. Adding PKCE would be defense-in-depth against authorization-code interception on the loopback redirect.
**Recommendation:** SECURITY-DECIDE in Phase 1. Adding PKCE is low-effort (~30 LOC, no breaking changes); deferring it requires explicit threat-model justification.

### NFR-S-B: `JR_AUTH_HEADER` override has no test-only gate; production binary respects it

**Source ref:** Pass 2 R7 NEW-INV-310
**Code citation:** `src/api/client.rs:64-66`
```rust
let auth_header = if let Ok(header) = std::env::var("JR_AUTH_HEADER") {
    header  // ← prod binary accepts this
} else { /* keychain path */ };
```
**NFR dimension:** Security (privilege escalation via env var)
**Severity:** **HIGH** — any user (or malicious script) can set `JR_AUTH_HEADER=Bearer <stolen-token>` and `jr` will use it without checking `cfg(test)`. The variable is only documented in CLAUDE.md as a test seam, but production `jr` binaries respect it. Defense-in-depth gap.
**Recommendation:** SECURITY-DECIDE in Phase 1. Options: (a) gate behind `#[cfg(test)]`, (b) require simultaneous `JR_BASE_URL` to be set (de facto pairing), or (c) explicit feature flag at compile time. (b) is the lowest-risk migration since tests already pair them.

### NFR-S-C: `--verbose` mode dumps full request bodies including potential PII/secrets

**Source ref:** Pass 2 R7 NEW-INV-323; broad Pass 4 §7.2.9 (latent risk noted)
**Code citation:** `src/api/client.rs:200-203, 274-278` — `eprintln!("[verbose] body: {}", String::from_utf8_lossy(bytes))`. **Authorization header NOT logged** (Pass 4 §2.6 verified), but request body IS. Currently safe (no secret-bearing JSON bodies on JiraClient path), but assignee account IDs, PII in summaries/descriptions, and ADF-encoded comment text all flow through.
**NFR dimension:** Security (PII leakage / log scrub burden)
**Severity:** **MEDIUM** — for AI-agent usage where verbose output may be persisted to logs/transcripts. Atlassian account IDs are PII under GDPR.
**Recommendation:** SECURITY-DECIDE in Phase 1. Add redaction layer for known sensitive JSON fields (`assignee.accountId`, `description`, `comment.body`), or document `--verbose` as "not for production traces."

### NFR-S-D: Build-time XOR per-build random key (re-confirmation)

**Source ref:** Pass 2 R5 NEW-INV-105
**Code citation:** `build.rs:21-29` — fresh 32-byte random key per `cargo build`.
**NFR dimension:** Security (anti-static-scanning depth)
**Severity:** **LOW** — already implemented. Defense-in-depth strength. Documented in ADR-0006.
**Recommendation:** DOCUMENT-AS-IS — Phase 1 should preserve this exact behavior in any reimplementation.

---

## 3. UX / Observability NFRs — Cross-Pollination Additions

### NFR-O-A: ADF mention/emoji/inlineCard/media* nodes silently lossy in text mode

**Source ref:** Pass 2 R6 NEW-INV-101
**Code citation:** `src/adf.rs` — `render_text` collapses unsupported nodes to empty strings; `--output json` preserves them.
**NFR dimension:** UX / Observability (silent data loss in default output mode)
**Severity:** **MEDIUM** — comments containing @mentions render with the mention dropped. Power users can re-run with `--output json` but discoverability is poor.
**Recommendation:** UX-DECIDE in Phase 1. Options: (a) render `@<displayName>` placeholder, (b) emit stderr warning with `--verbose`, (c) DOCUMENT-AS-IS with `--output json` as workaround.

### NFR-O-B: `auth status` vs `auth refresh` inconsistent failure semantics

**Source ref:** Pass 2 R6 NEW-INV-119
**Code citation:** `src/cli/auth.rs` — `auth status` silently falls through to embedded if BYO keychain entries are absent; `auth refresh` errors. Inconsistent UX.
**NFR dimension:** Observability / UX
**Severity:** **MEDIUM** — confusing state inspection. User runs `auth status` (looks fine), then `auth refresh` (errors).
**Recommendation:** UX-DECIDE in Phase 1. Align both to either (a) silent fall-through (current `auth status` behavior) or (b) explicit error (current `auth refresh` behavior).

### NFR-O-C: `--no-color` two-trigger semantics

**Source ref:** Pass 2 R6 NEW-INV-127
**Code citation:** `src/main.rs:13-15` — flag OR `NO_COLOR` env. Both paths converge.
**NFR dimension:** UX (industry-standard convention)
**Severity:** **LOW** — already correct (no-color.org compliant).
**Recommendation:** DOCUMENT-AS-IS.

### NFR-O-D: No `tracing` crate — observability layer is 39 LOC

**Source ref:** Pass 2 R6 NEW-INV-148; broad Pass 4 §7.4.17
**Code citation:** `src/observability.rs` (39 LOC); `Cargo.toml` no `tracing`/`log`/`slog`.
**NFR dimension:** Observability
**Severity:** **MEDIUM** — for AI-agent integrators wanting parseable execution traces. For human users, `--verbose` is sufficient.
**Recommendation:** UX-DECIDE in Phase 1. Add structured tracing for AI-agent spans, or DOCUMENT-AS-IS deferral with explicit "not in scope" rationale.

### NFR-O-E: `login_token` shared-keychain race for multi-profile

**Source ref:** Pass 2 R6 NEW-INV-157
**Code citation:** `src/api/auth.rs` — api-token login writes to shared `email`/`api-token` keys; concurrent `jr auth login --profile X` and `jr auth login --profile Y` race on the shared keys.
**NFR dimension:** Reliability (concurrency)
**Severity:** **LOW** — single-user CLI; concurrent login is rare. Last-write-wins.
**Recommendation:** DOCUMENT-AS-IS as known limitation; advise users not to run concurrent logins.

### NFR-O-F: `login_token` does NOT validate token before storing

**Source ref:** Pass 2 R6 NEW-INV-158
**Code citation:** `src/api/auth.rs` — accepts user-provided email + token, stores immediately, no `GET /myself` validation.
**NFR dimension:** UX (silent misconfiguration)
**Severity:** **MEDIUM** — user typo'd token gets stored; first real command fails with confusing 401. A single `GET /myself` round-trip would catch this at login time.
**Recommendation:** UX-DECIDE in Phase 1. Add post-login validation with `GET /rest/api/3/myself`; on failure, do NOT store and prompt for retry. Trade-off: one extra API call per login.

### NFR-O-G: `--open` + `--status` inconsistent conflict policy

**Source ref:** Pass 2 R6 NEW-INV-163
**Code citation:** `src/cli/issue/list.rs` — `--open` and `--status Done` should logically conflict (`--open` excludes Done) but flow proceeds with empty result instead of erroring.
**NFR dimension:** UX
**Severity:** **LOW** — silent empty result is unintuitive but recoverable.
**Recommendation:** DOCUMENT-AS-IS or add clap conflict directive (`conflicts_with`) — Phase 1 decision.

### NFR-O-H: `--all` suppresses truncation warning

**Source ref:** Pass 2 R6 NEW-INV-169
**Code citation:** `src/cli/issue/list.rs` — truncation hint fires only when `--limit` exceeded; `--all` skips entirely (which is correct since no truncation occurred).
**NFR dimension:** UX (consistency)
**Severity:** **LOW** — current behavior is correct; flagged here only for documentation.
**Recommendation:** DOCUMENT-AS-IS.

### NFR-O-I: Em-dash vs empty-string sentinels

**Source ref:** Pass 2 R6 NEW-INV-175
**Code citation:** `src/cli/issue/format.rs` — sometimes `—` (em-dash) for missing fields, sometimes empty string. Inconsistent.
**NFR dimension:** UX (visual consistency)
**Severity:** **LOW** — purely cosmetic.
**Recommendation:** UX-DECIDE: pick one (em-dash recommended for column-alignment).

### NFR-O-J: `accessible_resources` first-result-wins blocks multi-site OAuth users

**Source ref:** Pass 2 R6 NEW-INV-179
**Code citation:** `src/api/auth.rs` — uses `resources[0]` from `accessible-resources` response; users with multiple Atlassian sites under one account silently get the first one.
**NFR dimension:** UX (multi-site support gap)
**Severity:** **MEDIUM** — affects users with multiple Atlassian sites under one identity (common for consultants, multi-org users).
**Recommendation:** UX-DECIDE in Phase 1. Add disambiguation prompt when `resources.len() > 1`, or `--site <name>` flag, or auto-create one profile per resource.

### NFR-O-K: Silent 64-char profile-name limit error

**Source ref:** Pass 2 R6 NEW-INV-185
**Code citation:** `src/config.rs:113-140` — `validate_profile_name` rejects >64 chars with same generic error as other validation failures; doesn't distinguish length-violation from charset-violation.
**NFR dimension:** UX (error message specificity)
**Severity:** **LOW** — error fires correctly, message is just generic.
**Recommendation:** UX-DECIDE: add length-specific message.

### NFR-O-L: 404 + 403 collapsed to same message

**Source ref:** Pass 2 R6 NEW-INV-190
**Code citation:** `src/api/client.rs` — `extract_error_message` returns same generic body for 404 and 403; user can't distinguish "issue doesn't exist" from "you don't have permission."
**NFR dimension:** UX (error specificity)
**Severity:** **MEDIUM** — common confusion for permission-restricted projects.
**Recommendation:** UX-DECIDE: status-code-specific message templates.

### NFR-O-M: `handle_list` silently degrades vs `handle_current` hard-errors (sprint resolution)

**Source ref:** Pass 2 R6 NEW-INV-219, NEW-INV-287
**Code citation:** `src/cli/sprint.rs` — `handle_list` for kanban board returns empty; `handle_current` errors. Inconsistent failure semantics for the same root cause.
**NFR dimension:** UX (consistency)
**Severity:** **MEDIUM** — confusing for users on kanban boards.
**Recommendation:** UX-DECIDE: align both to same behavior (recommend hard-error with helpful message about scrum-vs-kanban).

### NFR-O-N: `search_issues` unbounded fetch when `--all` is set

**Source ref:** Pass 2 R6 NEW-INV-261; broad Pass 4 §7.1.5
**Code citation:** `src/cli/issue/list.rs` — `--all` + huge project = OOM risk. Already noted in broad Pass 4.
**NFR dimension:** Performance / Reliability
**Severity:** **LOW** — synthetic; no real workflow hits 100k issues in a single command.
**Recommendation:** DOCUMENT-AS-IS with optional safety cap (e.g., `MAX_ISSUES_ALL = 10000`) for defense-in-depth.

### NFR-O-O: Silent 25-result asset cap

**Source ref:** Pass 2 R6 NEW-INV-263, NEW-INV-401
**Code citation:** `src/cli/assets.rs` — hardcoded `page_size = 25` for asset search; no truncation warning.
**NFR dimension:** UX (silent data limit)
**Severity:** **MEDIUM** — assets searches with >25 hits silently drop tail; user has no flag to expand.
**Recommendation:** UX-DECIDE: expose `--limit`/`--all` parity with issue list; emit truncation warning.

### NFR-O-P: `project_meta` cache invalidation by re-fetch

**Source ref:** Pass 2 R6 NEW-INV-281
**Code citation:** `src/cache.rs` — no explicit invalidation flag; relies on TTL or whole-cache deletion.
**NFR dimension:** Reliability (stale data)
**Severity:** **LOW** — 7-day TTL is short enough; refresh is `rm cache file` workaround.
**Recommendation:** DOCUMENT-AS-IS or add `--refresh` flag parity (resolutions cache already has this).

### NFR-O-Q: Scrum vs kanban silent fall-through (paired with NFR-O-M)

**Source ref:** Pass 2 R6 NEW-INV-288
**Code citation:** `src/cli/sprint.rs` — multiple handlers behave inconsistently on kanban boards.
**NFR dimension:** UX (consistency)
**Severity:** Folded into NFR-O-M.

### NFR-O-R: GraphQL string-interpolation gap (no escape function)

**Source ref:** Pass 2 R6 NEW-INV-295
**Code citation:** `src/api/jira/teams.rs` — GraphQL query for org metadata interpolates `cloudId` directly into the query string. No `escape_graphql_value` helper. cloudId is UUID-shaped so practical injection is hard, but the pattern is an injection-class smell.
**NFR dimension:** Security (defense in depth)
**Severity:** **LOW** — cloudId comes from Atlassian itself (not user input), and is UUID-validated. But coding pattern lacks the safety escape jql.rs uses.
**Recommendation:** SECURITY-DECIDE: add `escape_graphql_value` helper to match jql.rs convention even if practical injection is impossible today.

### NFR-O-S: 4 distinct bool field names in JSON output (changed/updated/linked/unlinked)

**Source ref:** Pass 2 R6 NEW-INV-300, NEW-INV-405
**Code citation:** `src/cli/issue/json_output.rs` — `move` returns `transitioned`, `assign` returns `changed`, `edit` returns `updated`, `link` returns `linked`/`unlinked`. Each command picked its own verb.
**NFR dimension:** UX (JSON contract consistency for AI-agent consumers)
**Severity:** **MEDIUM** — AI agents must know per-command which field to check. A single `applied: bool` would be reusable.
**Recommendation:** UX-DECIDE: keep as-is (per-command semantic verb is more readable), or unify to `applied`/`changed`. Phase 1 must explicitly choose since this is a JSON CONTRACT.

### NFR-O-T: u64 overflow risk on `Retry-After` parse

**Source ref:** Pass 2 R7 NEW-INV-351
**Code citation:** `src/api/rate_limit.rs:17-18` — `trim().parse::<u64>()` accepts up to `u64::MAX` seconds (~584 billion years).
**NFR dimension:** Reliability (defense in depth)
**Severity:** **LOW** — already noted in broad Pass 4 §7.1.3 as "no upper bound." `u64` overflow itself is impossible but practical bound (e.g., 5 minutes max) is missing.
**Recommendation:** SECURITY-DECIDE: cap `Retry-After` at 300s and emit warning when server requests more.

### NFR-O-U: Empty-teams JSON deserialization tolerance

**Source ref:** Pass 2 R7 NEW-INV-354
**Code citation:** `src/types/jira/team.rs` — accepts `[]` and `null` for teams response.
**NFR dimension:** Reliability (graceful degradation)
**Severity:** **LOW** — already correct.
**Recommendation:** DOCUMENT-AS-IS.

### NFR-O-V: Queue silent omission

**Source ref:** Pass 2 R7 NEW-INV-369
**Code citation:** `src/cli/queue.rs` / `src/api/jsm/queues.rs` — non-JSM projects silently return empty queue list rather than erroring.
**NFR dimension:** UX (silent failure)
**Severity:** **LOW** — empty result is logically correct (project has no queues).
**Recommendation:** DOCUMENT-AS-IS.

### NFR-O-W: CMDB codebase-wide silent-degrade on missing workspace

**Source ref:** Pass 2 R7 NEW-INV-374
**Code citation:** `src/api/assets/workspace.rs` — 404 → "JSM Premium required" message; 403 → same message; missing workspace silently returns no assets.
**NFR dimension:** UX (error specificity)
**Severity:** **MEDIUM** — confusing for users with JSM Premium but misconfigured permissions.
**Recommendation:** UX-DECIDE: distinguish 403 (permission) from 404 (no workspace) from 404 (no JSM Premium).

### NFR-O-X: `Retry-After` parsed only as integer seconds; HTTP-date silently falls back to 1s

**Source ref:** Pass 2 R7 NEW-INV-408; broad Pass 4 §7.1.4
**Code citation:** `src/api/rate_limit.rs:17-18`
**NFR dimension:** Reliability
**Severity:** **LOW** — Atlassian uses integer seconds; gap is theoretical.
**Recommendation:** DOCUMENT-AS-IS or add HTTP-date parser as defensive coding.

---

## 4. Performance NFRs — Corrections to Broad Pass 4

### NFR-P-CORRECTION-1: Asset enrichment topology

**Broad Pass 4 §1.5 / §5.2 stated:** "Asset enrichment is serialized, not concurrent: per-field calls awaited one-at-a-time. N+1 query pattern."

**Pass 2 E-02-04 corrected:** Asset enrichment is actually a **3-pass dedup-and-concurrent** pattern:
1. **Pass A — collect**: walk all rows, accumulate `Set<(workspace_id, object_id)>` of unique assets to enrich.
2. **Pass B — fetch**: dedup'd unique fetches, **NOT one-per-row**. (Concurrency level not yet quantified — needs R2 to verify whether `try_join_all` or similar is used; Pass 2 found no evidence of fan-out in `futures` usage.)
3. **Pass C — distribute**: hash-map lookup to populate every row.

**Severity:** correction note — broad Pass 4 OVER-STATED the N+1 risk. Per-row dedup mitigates the linear scaling for issues sharing CMDB assets.
**Recommendation:** R2 should verify pass-B concurrency primitive (sequential await loop vs `try_join_all`). If sequential, dedup-then-serial is still N+1 by *unique-asset* count, not by row × field count. NFR severity downgrades from MEDIUM to LOW for typical workloads.

### NFR-P-CORRECTION-2: Memory for `--all` lists

Broad Pass 4 §5.2 noted full-buffer rendering. Pass 2 R7 confirmed via `--all` semantics in `cli/mod.rs:740`. No revision needed; severity remains LOW.

---

## 5. Re-Verification of Broad Pass 4's 23 NFR Gaps

| Broad Pass 4 ref | Original claim | R1 verdict | Notes |
|---|---|---|---|
| §7.1.1 — OAuth no 30s timeout | OAuth `Client::new()` lacks timeout | **STILL TRUE** | `api/auth.rs:607, 708` — Pass 2 R6 reconfirms |
| §7.1.2 — N+1 asset enrichment | Serialized per-row | **REFINED** (see NFR-P-CORRECTION-1) | dedup-and-fetch, not strict N+1 |
| §7.1.3 — No `Retry-After` upper bound | uncapped sleep | **STILL TRUE** | NFR-O-T; severity LOW |
| §7.1.4 — No HTTP-date Retry-After | integer-only parse | **STILL TRUE** | NFR-O-X |
| §7.1.5 — Memory full-buffer | `--all` OOM theoretical | **STILL TRUE** | NFR-O-N |
| §7.2.6 — No PKCE | OAuth lacks PKCE | **STILL TRUE** | NFR-S-A; reconfirmed Pass 2 R6 |
| §7.2.7 — No SBOM | no cargo-cyclonedx in CI | **STILL TRUE** | unchanged |
| §7.2.8 — No release signing | SHA256 only | **STILL TRUE** | unchanged |
| §7.2.9 — Verbose body logging | latent risk | **REFINED → HIGHER SEVERITY** | NFR-S-C; account IDs are PII |
| §7.2.10 — No FIPS TLS | rustls without aws-lc-rs | **STILL TRUE** | very low severity |
| §7.2.11 — Implicit redirect policy | reqwest default 10 | **STILL TRUE** | unchanged |
| §7.2.12 — `multiple-versions = warn` | non-blocking | **STILL TRUE** | unchanged |
| §7.3.13 — Ctrl+C OAuth refresh | partial-state recovery via re-login | **STILL TRUE** | unchanged |
| §7.3.14 — Cache writes not atomic | torn write recoverable via miss | **STILL TRUE** | unchanged |
| §7.3.15 — No 401 auto-refresh | manual `auth refresh` only | **STILL TRUE** | medium severity |
| §7.3.16 — Config save not atomic | direct fs::write | **STILL TRUE** | very low |
| §7.4.17 — observability.rs 39 LOC | no tracing | **STILL TRUE** | NFR-O-D |
| §7.4.18 — No request-id propagation | no x-arequestid | **STILL TRUE** | low-medium |
| §7.4.19 — No metrics emission | no Prometheus/OTel | **STILL TRUE** | very low |
| §7.4.20 — `--verbose` binary | no per-module filter | **STILL TRUE** | very low |
| §7.5.21 — No file locking | concurrent writes race | **STILL TRUE** | very low |
| §7.5.22 — No bulk-create | single-issue per call | **STILL TRUE** | low (out of v1 scope) |
| §7.5.23 — No streaming output | full-buffer | **STILL TRUE** | very low |

**Verdict:** all 23 broad Pass 4 NFR gaps remain valid. 1 is REFINED (§7.1.2 — asset enrichment topology corrected); 1 is REFINED → HIGHER SEVERITY (§7.2.9 — verbose body logging now explicitly flagged for PII due to AI-agent usage patterns).

---

## 6. New Configuration Values Discovered (Deltas vs Broad's 27)

Pass 2 R1-R7 surfaced additional named constants. R1 catalog of these:

| Constant | Value | Where defined | NFR dimension | Source ref |
|---|---|---|---|---|
| `USER_PAGE_SIZE` | (numeric — Pass 2 cited) | `src/api/jira/users.rs` | Reliability (deliberate JRACLOUD-71293 workaround) | NEW-INV-19 |
| `USER_PAGINATION_SAFETY_CAP` | `1500` | `src/api/jira/users.rs` | Reliability (defensive bound) | Pass 2 R3 |
| Asset hardcoded page size | `25` | `src/cli/assets.rs` | UX (silent cap) | NFR-O-O |
| Hardcoded `hours_per_day` | `8` | `src/cli/worklog.rs` | Reliability (semantic correctness) | NFR-R-C |
| Hardcoded `days_per_week` | `5` | `src/cli/worklog.rs` | Reliability (semantic correctness) | NFR-R-C |
| Profile name length cap | `64` | `src/config.rs:113-140` | Security (path-traversal prevention) | NFR-O-K |

**Updated config-value catalog total: 27 (broad) + 6 (R1 deltas) = 33.**

---

## 7. NFR Severity Matrix (All Items)

Organized by severity descending. **Severity** = combined likelihood × impact for a real user.

| ID | Dimension | Severity | Source | Recommendation |
|---|---|---|---|---|
| NFR-R-D | Reliability | **CRITICAL** | NEW-INV-12/143 | MUST-FIX: per-profile fields lookup |
| NFR-R-A | Reliability | HIGH | NEW-INV-29 | MUST-FIX: paginate list_worklogs |
| NFR-R-B | Reliability | HIGH | NEW-INV-56 | MUST-FIX: handle_open use instance_url |
| NFR-R-E | Reliability | HIGH | NEW-INV-229 | MUST-FIX: (workspace_id, object_id) key |
| NFR-S-B | Security | HIGH | NEW-INV-310 | SECURITY-DECIDE: gate JR_AUTH_HEADER |
| NFR-R-C | Reliability | MEDIUM | NEW-INV-81 | UX-DECIDE: time-tracking config fetch |
| NFR-S-A | Security | MEDIUM | NEW-INV-178 | SECURITY-DECIDE: PKCE adoption |
| NFR-S-C | Security | MEDIUM | NEW-INV-323 | SECURITY-DECIDE: --verbose redaction |
| NFR-O-A | UX | MEDIUM | NEW-INV-101 | UX-DECIDE: ADF lossy nodes |
| NFR-O-B | UX | MEDIUM | NEW-INV-119 | UX-DECIDE: auth status/refresh align |
| NFR-O-D | Observability | MEDIUM | NEW-INV-148 | UX-DECIDE: tracing layer |
| NFR-O-F | UX | MEDIUM | NEW-INV-158 | UX-DECIDE: validate api-token at login |
| NFR-O-J | UX | MEDIUM | NEW-INV-179 | UX-DECIDE: accessible_resources disambiguation |
| NFR-O-L | UX | MEDIUM | NEW-INV-190 | UX-DECIDE: 404/403 message specificity |
| NFR-O-M | UX | MEDIUM | NEW-INV-219/287 | UX-DECIDE: scrum/kanban consistency |
| NFR-O-O | UX | MEDIUM | NEW-INV-263/401 | UX-DECIDE: asset --limit/--all parity |
| NFR-O-S | UX | MEDIUM | NEW-INV-300/405 | UX-DECIDE: JSON bool field unification |
| NFR-O-W | UX | MEDIUM | NEW-INV-374 | UX-DECIDE: CMDB error specificity |
| Pass 4 §7.2.9 | Security | MEDIUM (raised) | refined NFR-S-C | Verbose body PII redaction |
| Pass 4 §7.3.15 | Reliability | MEDIUM | broad | DECIDE: 401 auto-refresh integration |
| NFR-R-F | Reliability | LOW | NEW-INV-18 | DOCUMENT-AS-IS |
| NFR-R-G | Reliability | LOW | NEW-INV-19 | DOCUMENT-AS-IS |
| NFR-S-D | Security | LOW | NEW-INV-105 | DOCUMENT-AS-IS |
| NFR-O-C | UX | LOW | NEW-INV-127 | DOCUMENT-AS-IS |
| NFR-O-E | Reliability | LOW | NEW-INV-157 | DOCUMENT-AS-IS |
| NFR-O-G | UX | LOW | NEW-INV-163 | UX-DECIDE: clap conflict |
| NFR-O-H | UX | LOW | NEW-INV-169 | DOCUMENT-AS-IS |
| NFR-O-I | UX | LOW | NEW-INV-175 | UX-DECIDE: pick em-dash |
| NFR-O-K | UX | LOW | NEW-INV-185 | UX-DECIDE: length-specific msg |
| NFR-O-N | Performance | LOW | NEW-INV-261 | DOCUMENT-AS-IS |
| NFR-O-P | Reliability | LOW | NEW-INV-281 | DOCUMENT-AS-IS |
| NFR-O-R | Security | LOW | NEW-INV-295 | SECURITY-DECIDE: GraphQL escape helper |
| NFR-O-T | Reliability | LOW | NEW-INV-351 | SECURITY-DECIDE: cap Retry-After at 300s |
| NFR-O-U | Reliability | LOW | NEW-INV-354 | DOCUMENT-AS-IS |
| NFR-O-V | UX | LOW | NEW-INV-369 | DOCUMENT-AS-IS |
| NFR-O-X | Reliability | LOW | NEW-INV-408 | DOCUMENT-AS-IS |
| Pass 4 §7.1.x | various | LOW (×8) | broad | mostly DOCUMENT-AS-IS |
| Pass 4 §7.4.x | Observability | LOW-VL (×4) | broad | UX-DECIDE: tracing |
| Pass 4 §7.5.x | Scalability | VL (×3) | broad | DOCUMENT-AS-IS |

**Severity breakdown:**
- **Critical:** 1
- **High:** 4
- **Medium:** 14
- **Low:** ~22

**Total NFR items:** 41 (broad's 23 + 18 cross-pollination — net new). Some broad items mapped 1:1 to cross-pollination items (e.g., §7.1.3 / NFR-O-T for Retry-After cap), so the unique total is **~37 distinct NFR concerns**.

---

## 8. Phase 1 Spec Implications

### 8.1 MUST-FIX (correctness bugs requiring fix before Phase 1 spec freeze)

1. **NFR-R-D — Multi-profile fields bug** (CRITICAL). 12 sites read legacy `config.global.fields.*` instead of per-profile.
2. **NFR-R-B — `handle_open` OAuth URL bug** (HIGH). Browser opens 404 for OAuth profiles.
3. **NFR-R-A — `list_worklogs` truncation** (HIGH). Silent data loss past page 1.
4. **NFR-R-E — Multi-workspace asset HashMap mis-attribution** (HIGH). Cross-workspace asset display corruption.

### 8.2 SECURITY-DECIDE (explicit threat-model decision required)

1. **NFR-S-B — `JR_AUTH_HEADER` test-only gating** (HIGH). Choose: cfg(test) vs JR_BASE_URL pairing vs feature flag.
2. **NFR-S-A — PKCE adoption** (MEDIUM). Choose: add PKCE vs document deferral with threat model.
3. **NFR-S-C — `--verbose` body redaction** (MEDIUM). Choose: redact known PII fields vs document not-for-prod.

### 8.3 UX-DECIDE (UX consistency / contract decisions)

1. **NFR-R-C — Time-tracking config fetch** (MEDIUM). hardcoded 8/5 silently wrong on customized instances.
2. **NFR-O-A — ADF lossy nodes** (MEDIUM). Mention/emoji/inlineCard in text mode.
3. **NFR-O-B — `auth status` vs `auth refresh` semantics** (MEDIUM).
4. **NFR-O-F — Validate api-token at login** (MEDIUM). One extra round-trip, catches typos.
5. **NFR-O-J — `accessible_resources` multi-site disambiguation** (MEDIUM). Multi-org users.
6. **NFR-O-L — 404 vs 403 message specificity** (MEDIUM).
7. **NFR-O-M — Scrum vs kanban consistency** (MEDIUM).
8. **NFR-O-O — Asset --limit/--all parity** (MEDIUM). Silent 25-result cap.
9. **NFR-O-S — JSON bool field unification** (MEDIUM). 4 distinct verb names is a JSON contract issue.
10. **NFR-O-W — CMDB error specificity** (MEDIUM).
11. **NFR-O-D — `tracing` layer for AI-agent integration** (MEDIUM). Defer or commit.
12. **Pass 4 §7.3.15 — 401 auto-refresh** (MEDIUM). Remove `refresh_oauth_token` or wire it up.

### 8.4 DOCUMENT-AS-IS (preserve current behavior; cite rationale in spec)

- NFR-R-F, NFR-R-G, NFR-S-D (defensive patterns / deliberate workarounds)
- NFR-O-C (no-color is correct)
- NFR-O-E, NFR-O-H, NFR-O-N, NFR-O-P, NFR-O-U, NFR-O-V, NFR-O-X (low-impact edge cases)
- All Pass 4 §7.5.x (scalability is out of v1 scope)
- All Pass 4 §7.4.x except §7.4.17 (observability deferral is explicit; tracing decision is its own item)
- Most Pass 4 §7.1.x (perf gaps are documented theoretical risks)

---

## 9. Hallucination Audit (5 Classes vs Broad Pass 4)

Cross-checked broad Pass 4 against Pass 2 deepening findings:

### Class 1: Token lists — specific HTTP timeouts/limits

- **30s timeout**: verified `api/client.rs:84` — correct.
- **MAX_RETRIES = 3**: verified `api/client.rs:11` — correct.
- **DEFAULT_RETRY_SECS = 1**: verified `api/client.rs:14` — correct.
- **CACHE_TTL_DAYS = 7**: verified `cache.rs:7` — correct.
- **MAX_SPRINT_ISSUES = 50**: verified `cli/sprint.rs:107` — correct.
- **EMBEDDED_CALLBACK_PORT = 53682**: verified `api/auth.rs:384` — correct.
- **Profile name length 64**: verified `config.rs:113-140` — correct.

**Verdict:** No hallucinations in token lists.

### Class 2: Miscounted

- Broad claimed "27 config values cataloged." Pass 2 surfaced 6 additional named constants (USER_PAGE_SIZE, USER_PAGINATION_SAFETY_CAP, asset 25, hours_per_day 8, days_per_week 5, profile name 64). **Recount: 33.** Broad was UNDERCOUNTED, not overcounted. No hallucination — broad simply hadn't yet found these.
- Broad claimed "10 NFR gaps in §7." Actual count in §7 is **23** (5 perf + 7 security + 4 reliability + 4 observability + 3 scalability). The "10 NFR gaps" phrasing in the user's invocation appears to refer to high-level dimensions, not per-item. R1 disambiguates: 23 broad items + 18 cross-pollination = 41 unique items.

**Verdict:** Broad's "27 config values" and "23 NFR gaps in §7" are accurate to broad's scope; the "10" referenced in the prompt was a higher-level grouping.

### Class 3: Pattern fabrication — NFR pattern names

- "MAX_RETRIES" / "DEFAULT_RETRY_SECS" / "CACHE_TTL_DAYS" / "EMBEDDED_CALLBACK_PORT" / "MAX_SPRINT_ISSUES" / "DEFAULT_LIMIT" / "DEFAULT_OAUTH_SCOPES" / "DEFAULT_SERVICE_NAME" — all verified as named consts.
- "Cursor pagination" / "OffsetPage" / "ServiceDeskPage" / "AssetsPage" — all verified in `api/pagination.rs`.

**Verdict:** No fabricated pattern names.

### Class 4: Same-basename ambiguity

- `cli/assets.rs` (1,055 LOC standalone assets command) vs `cli/issue/assets.rs` (linked assets subcommand under issue) — both exist and broad correctly disambiguates.
- `api/auth.rs` (1,397 LOC OAuth/keychain) vs `api/auth_embedded.rs` (250 LOC obfuscation plumbing) — both exist; broad correctly cites both.

**Verdict:** No same-basename confusion.

### Class 5: Inflated metrics

- Broad cited "332 transitive deps in Cargo.lock." Pass 0 same number. **Spot-recheck recommended for R2** but no contradicting evidence in deepening rounds.
- Broad cited "24 runtime + 6 dev direct deps." Cross-checked with Pass 0 — consistent.

**Verdict:** No inflated metrics found in R1 audit. R2 should spot-verify the 332-deps claim with `cat Cargo.lock | grep '^name =' | wc -l`.

---

## 10. Delta Summary

- **New NFR items added:** 24 cross-pollination findings (NFR-R-A through NFR-O-X)
- **New configuration values cataloged:** 6 (USER_PAGE_SIZE, USER_PAGINATION_SAFETY_CAP, asset 25, hours_per_day 8, days_per_week 5, profile name 64)
- **Broad Pass 4 items refined:** 2 (asset enrichment topology corrected from N+1 to dedup-and-fetch; verbose body logging severity raised due to PII implications)
- **Broad Pass 4 items unchanged:** 21
- **Total NFR concerns:** 41 (some overlap; ~37 unique)
- **Severity escalations:** 1 (Pass 4 §7.2.9 → MEDIUM from latent)
- **Severity downgrades:** 1 (asset enrichment from MEDIUM to LOW for typical workloads)
- **MUST-FIX correctness bugs identified:** 4 (NFR-R-D, NFR-R-B, NFR-R-A, NFR-R-E)
- **SECURITY-DECIDE items:** 6 (NFR-S-A, NFR-S-B, NFR-S-C, NFR-O-R, NFR-O-T, plus broad PKCE/SBOM/signing)
- **UX-DECIDE items:** 12
- **DOCUMENT-AS-IS items:** ~18

---

## 11. Novelty Assessment

**Novelty: SUBSTANTIVE**

**Justification:** R1 introduces 24 new NFR items (NFR-R-A..G + NFR-S-A..D + NFR-O-A..X) sourced from Pass 2 R1-R7 cross-pollination. These were NOT in broad Pass 4 — they were domain-model findings whose NFR implications are surfaced for the first time here. R1 also CORRECTS broad Pass 4 §1.5/§5.2 on asset enrichment topology (N+1 → dedup-and-fetch) and ELEVATES verbose body logging severity. Removing R1's findings would change the spec — Phase 1 would lose the 4 MUST-FIX correctness bugs (NFR-R-A, NFR-R-B, NFR-R-D, NFR-R-E) entirely from the NFR catalog, and would inherit the wrong asset-enrichment topology.

**Test:** Would removing R1 change how the system is spec'd? **Yes** — Phase 1 would ship without the 4 MUST-FIX correctness bugs surfaced here, and would carry over an incorrect topology claim. SUBSTANTIVE.

---

## 12. Convergence Declaration

**Another round needed.** R2 should:

1. Function-level deepening of broad Pass 4's 5 dimensions (perf/security/reliability/observability/scalability) — NOT just intake. R1 was intake; R2 is depth.
2. Verify asset-enrichment concurrency primitive (sequential await loop vs `try_join_all`) to finalize NFR-P-CORRECTION-1 severity.
3. Spot-recheck "332 transitive deps" claim from broad and Pass 0 against current `Cargo.lock`.
4. Enumerate per-MUST-FIX bug the affected BCs (cross-reference with Pass 3 deepening) so Phase 1 can directly trace BC → NFR fix.
5. Examine `extract_error_message` 6-level chain (broad Pass 4 not in scope; surfaced as observability item in Pass 6 synthesis §5.2).
6. Review keychain partial-state recovery branches enumerated in Pass 2 R6/R7 — confirm Pass 4 broad's coverage is complete.

---

## 13. State Checkpoint

```yaml
pass: 4
round: 1
status: complete
new_nfr_findings_incorporated: 24
nfr_gaps_total: 41
must_fix_count: 4
security_decide_count: 6
ux_decide_count: 12
document_as_is_count: 18
config_values_total: 33
files_examined: 9
novelty: SUBSTANTIVE
timestamp: 2026-05-04T15:30:00Z
inputs_consumed:
  - .factory/semport/jira-cli/jira-cli-pass-4-nfr-catalog.md
  - .factory/semport/jira-cli/jira-cli-pass-2-deep-r1.md (cross-poll items)
  - .factory/semport/jira-cli/jira-cli-pass-2-deep-r2.md (cross-poll items)
  - .factory/semport/jira-cli/jira-cli-pass-2-deep-r3.md (cross-poll items)
  - .factory/semport/jira-cli/jira-cli-pass-2-deep-r4.md (cross-poll items)
  - .factory/semport/jira-cli/jira-cli-pass-2-deep-r5.md (cross-poll items)
  - .factory/semport/jira-cli/jira-cli-pass-2-deep-r6.md (cross-poll items)
  - .factory/semport/jira-cli/jira-cli-pass-2-deep-r7.md (cross-poll items)
  - .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md (BC cross-ref)
  - .factory/semport/jira-cli/jira-cli-pass-6-synthesis.md
verification_actions:
  - read_worklogs_rs: confirmed list_worklogs returns page 1 only
  - read_client_rs_head: confirmed JR_AUTH_HEADER no cfg(test) gate
hallucination_audit:
  - token_lists: clean
  - miscounts: broad undercounted config values (27 → 33); broad's "23 NFR gaps in §7" accurate
  - pattern_fabrication: clean
  - same_basename: clean
  - inflated_metrics: 332-transitive-deps claim flagged for R2 spot-verify
next_round_targets: |-
  R2 — function-level depth on 5 NFR dimensions
  R2 — verify asset-enrichment concurrency primitive
  R2 — spot-recheck Cargo.lock 332 transitive deps
  R2 — cross-ref MUST-FIX bugs to BCs from Pass 3 deepening
  R2 — extract_error_message 6-level chain coverage
  R2 — keychain partial-state recovery branch enumeration
```

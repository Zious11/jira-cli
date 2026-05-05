---
context: nfr-catalog
title: "NFR Catalog — Pass 4 Convergence"
total_nfrs: 41  # 42 − NFR-O-K (merged into NFR-S-D at ADV-P7-002)
last_updated: 2026-05-04
source_pass: 4
trace: |
  - L2: .factory/specs/domain-spec/
  - Source: .factory/semport/jira-cli/jira-cli-pass-4-deep-r4.md §2,§3,§4
  - Source: .factory/semport/jira-cli/jira-cli-pass-8-deep-synthesis.md §8
---

# NFR Catalog — jira-cli Pass 4 Convergence

42 individually-enumerated NFR rows. Pass 4 produced: broad (23) + R1 deepening (+18) + NFR-R-E re-promotion (0 net) + R4 NEW-2 (+1) = 42 entries then reconciled to 40 summary rows + NFR-R-NEW-1 added by ADV-P2-003 = 41 total. NFR-S-E severity promoted from LOW to HIGH per ADV-P2-004. NFR-S-F (cargo-deny supply chain) added per ADV-P3-007 = 42 total.

**Severity totals: 1 CRITICAL / 6 HIGH / 15 MEDIUM / 19 LOW = 41 total** (NFR-O-K merged into NFR-S-D at ADV-P7-002)

All four MUST-FIX items (NFR-R-D, NFR-R-A, NFR-R-B, NFR-R-E) have been crystallized as behavioral contracts in the L3 PRD:
- NFR-R-D → BC-6.3.001 (multi-profile fields bug)
- NFR-R-A → BC-X.5.002 (list_worklogs non-paginated)
- NFR-R-B → BC-3.4.001 (handle_open OAuth URL)
- NFR-R-E → BC-4.3.001 (multi-workspace asset HashMap)

---

## Dimension 1: Reliability (R-*)

### CRITICAL

| ID | Description | Severity | Site | Phase 3 Routing | BC Anchor |
|---|---|---|---|---|---|
| **NFR-R-D** | Multi-profile fields bug: all field reads use `config.global.fields.*` path; per-profile `story_points_field_id`/`team_field_id` in `ProfileConfig` are never read by handlers. Cross-profile correctness failure — sandbox vs prod custom-field IDs silently disagree. | CRITICAL | `src/cli/issue/list.rs:147-148`, `src/cli/issue/view.rs:28-29`, `src/cli/issue/helpers.rs:43,194,200,209`, `src/cli/sprint.rs:232-233`, `src/cli/board.rs:192`, `src/cli/issue/create.rs:128,277,283` (14 sites total) | **FIX-IN-PHASE-3**: Add `Config::field_id(FieldKind, profile)` accessor; replace all 14 `config.global.fields.*` reads; add integration test in `tests/auth_profiles.rs` | BC-6.3.001 |

### HIGH

| ID | Description | Severity | Site | Phase 3 Routing | BC Anchor |
|---|---|---|---|---|---|
| **NFR-R-A** | `list_worklogs` non-paginated: fetches `OffsetPage<Worklog>` and returns `.items().to_vec()` — no loop. Silent data loss past page 1 for issues with >50 worklogs (Atlassian's default page). | HIGH | `src/api/jira/worklogs.rs:25-30` | **FIX-IN-PHASE-3**: Refactor to use `paginate_offset` loop (same as `list_comments`); add 2-page worklog integration test | BC-X.5.002 |
| **NFR-R-B** | `handle_open` broken for OAuth profiles: constructs URL via `client.base_url()` which returns `https://api.atlassian.com/ex/jira/<cloud_id>` for OAuth profiles — browser opens 404. Should use `client.instance_url()`. | HIGH | `src/cli/issue/workflow.rs:636` | **FIX-IN-PHASE-3**: One-line fix `base_url()` → `instance_url()`; add OAuth profile integration test | BC-3.4.001 |
| **NFR-R-E** | Multi-workspace asset HashMap mis-attribution: `resolved: HashMap<String, _>` keyed by `oid` alone at line 446 loses workspace context; last-write-wins on `oid` collision. `to_enrich` (line 398) is correctly keyed `(wid, oid)`. Bug is in `cli/issue/list.rs` (NOT `api/assets/linked.rs::enrich_assets` which is correct). | HIGH | `src/cli/issue/list.rs:440,446,449,456` | **FIX-IN-PHASE-3**: Change `resolved` key to `(String, String)` at all 3 sites; add multi-workspace fixture integration test | BC-4.3.001 |

### MEDIUM

| ID | Description | Severity | Site | Phase 3 Routing |
|---|---|---|---|---|
| **NFR-R-C** | Worklog duration uses hardcoded `8h/day, 5d/week` constants. Jira instances can configure these via `/rest/api/3/configuration/timetracking`. Silent wrong-answer for 7.5h or 4-day setups. | MEDIUM | `src/cli/worklog.rs:32` | **FIX-IN-PHASE-3**: Fetch + cache timetracking config from Jira instance (7-day TTL); fall back to 8/5 on miss |
| **NFR-R-F** | `get_changelog` anti-loop guard present (breaks if nextPage URL == current URL). `search_issues` cursor loop has no analogous guard against cursor == cursor regression. | MEDIUM | `src/api/jira/issues.rs:222-230` | **DOCUMENT-AS-IS**: Add similar guard to `search_issues`; document pattern |
### LOW

| ID | Description | Severity | Site | Phase 3 Routing |
|---|---|---|---|---|
| **NFR-R-G** | Non-atomic cache writes: `cache.rs:36-43` uses direct `std::fs::write` — no temp-file + atomic rename. Crash/SIGKILL between start and end leaves indeterminate file state. Self-healing via deserialization-failure → cache-miss. | LOW | `src/cache.rs:36-43` | **DOCUMENT-AS-IS**: Self-healing already; LOW for single-user CLI. Optional: use temp-file + rename pattern. |
| **NFR-R-NEW-1** | `Retry-After` header has no upper-bound cap in current code. `Retry-After: 86400` causes the retry loop to sleep for 24 hours with no user escape (other than Ctrl+C). BC-X.4.009 proposes `MAX_RETRY_AFTER_SECS = 60` cap as Phase 3 fix. Current behavior: any valid u64 value is honored as-is. **Severity LOW (ADV-P3-009 reviewed, retained; section corrected ADV-P6-003):** Single-user CLI — user can Ctrl+C at any time; not a service-grade SLA concern. Atlassian does not send multi-hour `Retry-After` values in practice. | LOW | `src/api/rate_limit.rs:14-19` | **FIX-IN-PHASE-3**: Implement `MAX_RETRY_AFTER_SECS = 60` cap per BC-X.4.009. Print warning and abort retry when cap exceeded. H-027 pins current gap (to be updated when fix lands). |
| **NFR-R-NEW-2** | `parse_duration` silently wraps on multiplicative overflow for pathological inputs (e.g., `99999999999999w`). Release builds have `panic=abort` which disables debug overflow checks — silent wrapped value sent to Jira API. **Severity LOW (section corrected ADV-P6-003).** | LOW | `src/duration.rs:29-32` | **DOCUMENT-AS-IS**: Use `checked_mul`; bail with "duration too large" error. ~5 LOC fix. Acceptable for v1. |

---

## Dimension 2: Security (S-*)

### HIGH

| ID | Description | Severity | Site | Phase 3 Routing |
|---|---|---|---|---|
| **NFR-S-B** | `JR_AUTH_HEADER` env var read unconditionally in production binary (`client.rs:64-66`). Any process inheriting that env-var bypasses keychain auth. Privilege escalation risk in CI/CD environments where env vars leak between jobs. | HIGH | `src/api/client.rs:64-66` | **SECURITY-DECIDE**: Option (a) `#[cfg(test)]` gate; OR (b) require simultaneous `JR_BASE_URL` set (lowest-risk migration). Policy decision required. |
| **NFR-S-F** | Supply-chain: `cargo-deny` is wired in CI but `multiple-versions = "warn"` policy means version dupes don't fail the build. No SBOM published. 332 transitive Cargo deps for an OAuth-handling CLI. Cross-ref: risk register R-H5. | HIGH | `deny.toml`, `.github/workflows/ci.yml` | **FIX-IN-PHASE-3**: Enforce `multiple-versions = "deny"` in `deny.toml`; publish SBOM via `cargo cyclonedx`. See R-H5 in risk-register.md. |
| **NFR-S-E** | GitHub Actions workflows use floating action tags (e.g., `actions/checkout@v4`) instead of pinned SHA digests. A compromised tag could inject malicious code into the build/release pipeline without detection — specifically, the OAuth client secret injected at build time (`JR_BUILD_OAUTH_CLIENT_ID`/`_SECRET`) could be exfiltrated. CI/CD integrity gap. | HIGH | `.github/workflows/` | **FIX-IN-PHASE-3**: Pin all `uses: <action>@<tag>` lines to `uses: <action>@<sha256-digest>` (e.g., `actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683`). Use `pin-github-action` or Dependabot to automate. Severity promoted from LOW to HIGH per ADV-P2-004 (rare event but OAuth client secret exposure if exploited). |

### MEDIUM

| ID | Description | Severity | Site | Phase 3 Routing |
|---|---|---|---|---|
| **NFR-S-A** | PKCE not implemented in OAuth flow. `build_authorize_url` sends no `code_challenge`/`code_challenge_method`. `exchange_code_for_token` sends no `code_verifier`. RFC 8252 recommends PKCE for native apps regardless of confidential-client status. | MEDIUM | `src/api/auth.rs:608-616` | **SECURITY-DECIDE**: Add RFC 7636 PKCE (~30 LOC). Cross-reference ADR-0006. |
| **NFR-S-C** | `--verbose` logs full request body via `String::from_utf8_lossy`. Account IDs, ADF comment text, summaries, descriptions all flow through. Authorization header NOT logged (only path). PII leakage to AI-agent transcripts and incident logs. | MEDIUM | `src/api/client.rs:200-203,274-278` | **SECURITY-DECIDE**: Add `redact_body()` helper in `observability.rs`; or default verbose to header-only with `--verbose-bodies` opt-in. |

### LOW

| ID | Description | Severity | Site | Phase 3 Routing |
|---|---|---|---|---|
| **NFR-S-D** | Profile name validation regex does not distinguish length (>64) from charset violation — same error message for both. LOW impact. (formerly tracked separately as NFR-O-K — merged at adversary Pass 7) | LOW | `src/config.rs:113-140` | **DOCUMENT-AS-IS**: Improve error message precision. 2 LOC fix. |

---

## Dimension 3: Observability (O-*)

### MEDIUM (observability items classified as MEDIUM)

| ID | Description | Severity | Site | Phase 3 Routing |
|---|---|---|---|---|
| **NFR-O-A** | No structured logging (tracing crate). `tracing` is already a dep. 47 `eprintln!` + 103 `println!` across 24 CLI handlers with no structured spans. AI-agent integration hampered by unparseable strings. | MEDIUM | `src/observability.rs`, `src/main.rs` | **DEFER**: Adopt `tracing` crate + `tracing-subscriber` in `main.rs` per `--verbose`. Replace `eprintln!("[verbose]...")` with `tracing::debug!`. P2 priority. |
| **NFR-O-B** | `refresh_oauth_token` is public with zero production callers — limbo state. Exists at `src/api/auth.rs:704-770` for a future 401 auto-refresh integration per CLAUDE.md. | MEDIUM | `src/api/auth.rs:704-770` | **DEFER**: Wire into `JiraClient::send` (one-attempt refresh on 401 expired_token). P1 priority. |
| **NFR-O-D** | `cli/auth.rs` (1,998 LOC) and `cli/assets.rs` (1,055 LOC) violate implicit ~1000 LOC shard rule. Same split approach as `cli/issue/`. | MEDIUM | `src/cli/auth.rs`, `src/cli/assets.rs` | **DEFER**: Create `src/cli/auth/{login,switch,list,status,refresh,logout,remove,helpers}.rs`. P1 priority. |
| **NFR-O-F** | 5 auth subcommands (login/switch/logout/remove/refresh) have no `--output json` paths — only human-readable output. | MEDIUM | `src/cli/auth.rs` | **POLICY-DECISION**: Add JSON shapes for auth subcommands (e.g., `{"profile": "X", "ok": true}`). |
| **NFR-O-J** | JSON field naming inconsistency: write-ops use 4 distinct booleans (`changed`, `updated`, `linked`, `unlinked`). No single canonical field name. | MEDIUM | various CLI handlers | **POLICY-DECISION**: Adopt `success: bool` + `action: string` OR document 4-distinct-name vocabulary as deliberate verb-aligned. Snapshot-pinned so change is high-friction. |
| **NFR-O-L** | CLAUDE.md does not document `cli/issue/view.rs` (286 LOC), `cli/issue/comments.rs` (61), `observability.rs` (39), `api/assets/schemas.rs` (44) — 4 undocumented orphan modules. | MEDIUM | `CLAUDE.md` | **FIX-IN-PHASE-3**: Update CLAUDE.md to include 12 deviations (D1-D12 from Pass 1 R1 §3d). |
| **NFR-O-M** | `--open` filter uses two mechanisms: JQL `statusCategory != Done` for Jira issues (server-side); `status.colorName != "green"` for connected tickets (client-side). Not documented as intentional. | MEDIUM | `src/cli/issue/list.rs:303,308,625`, `src/cli/assets.rs:303-321` | **DOCUMENT-AS-IS**: Add to CLAUDE.md "Gotchas" explaining the two-mechanism single semantic. |
| **NFR-O-O** | User pagination advances by `USER_PAGE_SIZE` not returned-count — deliberate workaround for JRACLOUD-71293. Undocumented in source. A future contributor "fixing" this would regress the workaround. | MEDIUM | `src/api/jira/users.rs` | **DOCUMENT-AS-IS**: Add source comment + CLAUDE.md "Gotchas" entry. |
| **NFR-O-S** | `accessible_resources` uses `resources.first()` — silent first-result-wins for multi-site OAuth users. No disambiguation or `--cloud-id` flag. | MEDIUM | `src/api/auth.rs:666-668` | **DEFER**: Add `--cloud-id <ID>` flag + interactive prompt or `--no-input` error. P1 priority. |
| **NFR-O-W** | Mixed test prefix styles: 108 `test_<verb>_<subject>` + 212 `<subject>_<verb>_<expected>` no-prefix. No codified convention for new tests. | MEDIUM | tests/ | **POLICY-DECISION**: Codify single style for new tests. |

### LOW (observability items classified as LOW)

| ID | Description | Severity | Site | Phase 3 Routing |
|---|---|---|---|---|
| **NFR-O-C** | No `--dry-run` flag on state-changing commands. Already documented as out-of-scope in v1 design spec. | LOW | `src/cli/issue/workflow.rs` | **DOCUMENT-AS-IS**: Retain out-of-scope status. |
| **NFR-O-E** | No progress indicator for long-running operations (asset enrichment, team list with many pages). | LOW | `src/cli/assets.rs`, `src/cli/team.rs` | **DEFER**: Consider for v2 UX pass. |
| **NFR-O-G** | `cli/issue/list.rs` is 970 LOC (post-split); `view.rs` and `comments.rs` are already split out per `docs/specs/list-rs-split.md`. CLAUDE.md still describes undivided `list.rs`. | LOW | `CLAUDE.md` | **DOCUMENT-AS-IS**: CLAUDE.md update covers this (NFR-O-L fix). |
| **NFR-O-H** | `JR_RUN_OAUTH_INTEGRATION` env-var gates 1 ignored test but not documented in CLAUDE.md "AI Agent Notes". | LOW | `CLAUDE.md` | **FIX-IN-PHASE-3**: Add alongside `JR_RUN_KEYRING_TESTS`. |
| **NFR-O-I** | `ADF::to_text` silently drops mention/emoji/inlineCard/media nodes (`_` fall-through at `adf.rs:531-540`). Documented in source as deliberate per issue #202. | LOW | `src/adf.rs:531-540` | **DEFER**: Render `@<displayName>` for mentions; `:emoji:` for emojis; `[<title>](url)` for inlineCard. Medium-effort. |
| **NFR-O-N** | `5 auth subcommands` lack JSON output paths (mentioned in NFR-O-F); also no `--output json` test coverage for `auth status` with multiple profiles. | LOW | `src/cli/auth.rs` | **DEFER**: Cover in auth JSON shape work (NFR-O-F). |
| **NFR-O-P** | No API version field in JSON output. Downstream parsers cannot detect schema changes. | LOW | `src/output.rs` | **DEFER**: Consider `"_meta": {"version": "1"}` envelope for v2. |
| **NFR-O-R** | `eprintln!` for human hints and `println!` for data are implicit contracts; no typed channel enum. 5 categorical profiles (Pure/Read-only/Mixed/Symmetric/no-log-facade) emerge by code-review only. | LOW | 24 CLI handler files | **DOCUMENT-AS-IS**: Document the 5 profiles in a source comment or CLAUDE.md. |
| **NFR-O-T** | `worklog list` default page size undocumented (currently whatever `OffsetPage` returns from Atlassian default). | LOW | `src/api/jira/worklogs.rs` | **DOCUMENT-AS-IS**: After NFR-R-A fix, document max-results parameter. |
| **NFR-O-U** | `sprint list` does not show sprint start/end dates in table output. Present in API response. | LOW | `src/cli/sprint.rs` | **DEFER**: UX pass v2. |
| **NFR-O-V** | `board view` truncation hint emitted to stderr (consistent with `issue list`/`sprint current`). Not documented. | LOW | `src/cli/board.rs` | **DOCUMENT-AS-IS**: Add to CLAUDE.md convention list. |
| **NFR-O-X** | No `jr version --output json` path (exists only as human-readable). | LOW | `src/main.rs` | **DEFER**: Low priority. |

---

## Dimension 4: Performance (P-*)

### MEDIUM

| ID | Description | Severity | Site | Phase 3 Routing |
|---|---|---|---|---|
| **NFR-P-NEW-1** | Asset enrichment uses `futures::future::join_all` with no concurrency cap. K unique assets → K simultaneous HTTP calls. 429-storm risk for large issue lists with many CMDB assets. `MAX_RETRIES=3` mitigates but does not bound. | MEDIUM | `src/cli/issue/list.rs:445`, `src/api/assets/linked.rs:216` | **DEFER**: Replace `join_all` with `buffer_unordered(8)`; add `MAX_CONCURRENT_ASSET_FETCHES = 8` constant. P2 priority. |

---

## Dimension 5: Scalability / API Conformance

### LOW

| ID | Description | Severity | Site | Phase 3 Routing |
|---|---|---|---|---|
| **NFR-SCA-1** | Retry-After parsing accepts integer only; HTTP-date format (`Mon, 04 May 2026 00:00:00 GMT`) silently falls through to `DEFAULT_RETRY_SECS = 1`. Atlassian sends integers in practice. | LOW | `src/api/rate_limit.rs:14-19` | **DOCUMENT-AS-IS**: Add HTTP-date fallback via `chrono` when/if observed in production. |
| **NFR-SCA-2** | Soft-fence per-profile cache isolation: convention is "every cache fn takes `profile: &str` first" (100% conformance) but no compile-time enforcement. Future contributor could add profile-unaware reader. | LOW | `src/cache.rs` | **DEFER**: Introduce `Profile(String)` newtype to enforce at compile time. P1 priority. |
| **NFR-SCA-3** | `validate_asset_key` accepts ASCII alphanumeric prefix + `-` + ASCII digit suffix only. Unicode object keys would be rejected. Not a current use case. | LOW | `src/jql.rs:39-54` | **DOCUMENT-AS-IS**: Constraint is by design; AQL attribute names are ASCII. |

---

## NFR Summary Table

| ID | Dimension | Severity | Phase 3 Routing | BC Anchor |
|---|---|---|---|---|
| NFR-R-D | Reliability | CRITICAL | FIX-IN-PHASE-3 | BC-6.3.001 |
| NFR-R-A | Reliability | HIGH | FIX-IN-PHASE-3 | BC-X.5.002 |
| NFR-R-B | Reliability | HIGH | FIX-IN-PHASE-3 | BC-3.4.001 |
| NFR-R-E | Reliability | HIGH | FIX-IN-PHASE-3 | BC-4.3.001 |
| NFR-S-B | Security | HIGH | SECURITY-DECIDE | — |
| NFR-R-C | Reliability | MEDIUM | FIX-IN-PHASE-3 | — |
| NFR-R-F | Reliability | MEDIUM | DOCUMENT-AS-IS | — |
| NFR-S-A | Security | MEDIUM | SECURITY-DECIDE | — |
| NFR-S-C | Security | MEDIUM | SECURITY-DECIDE | — |
| NFR-O-A | Observability | MEDIUM | DEFER | — |
| NFR-O-B | Observability | MEDIUM | DEFER | — |
| NFR-O-D | Observability | MEDIUM | DEFER | — |
| NFR-O-F | Observability | MEDIUM | POLICY-DECISION | — |
| NFR-O-J | Observability | MEDIUM | POLICY-DECISION | — |
| NFR-O-L | Observability | MEDIUM | FIX-IN-PHASE-3 | — |
| NFR-O-M | Observability | MEDIUM | DOCUMENT-AS-IS | — |
| NFR-O-O | Observability | MEDIUM | DOCUMENT-AS-IS | — |
| NFR-O-S | Observability | MEDIUM | DEFER | — |
| NFR-O-W | Observability | MEDIUM | POLICY-DECISION | — |
| NFR-P-NEW-1 | Performance | MEDIUM | DEFER | — |
| NFR-R-G | Reliability | LOW | DOCUMENT-AS-IS | — |
| NFR-R-NEW-1 | Reliability | LOW | FIX-IN-PHASE-3 | BC-X.4.009 (proposed fix) |
| NFR-R-NEW-2 | Reliability | LOW | DOCUMENT-AS-IS | — |
| NFR-S-D | Security | LOW | DOCUMENT-AS-IS | — |
| NFR-S-E | Security | HIGH | FIX-IN-PHASE-3 | — |
| NFR-S-F | Security | HIGH | FIX-IN-PHASE-3 | — |
| NFR-O-C | Observability | LOW | DOCUMENT-AS-IS | — |
| NFR-O-E | Observability | LOW | DEFER | — |
| NFR-O-G | Observability | LOW | DOCUMENT-AS-IS | — |
| NFR-O-H | Observability | LOW | FIX-IN-PHASE-3 | — |
| NFR-O-I | Observability | LOW | DEFER | — |
| NFR-O-N | Observability | LOW | DEFER | — |
| NFR-O-P | Observability | LOW | DEFER | — |
| NFR-O-R | Observability | LOW | DOCUMENT-AS-IS | — |
| NFR-O-T | Observability | LOW | DOCUMENT-AS-IS | — |
| NFR-O-U | Observability | LOW | DEFER | — |
| NFR-O-V | Observability | LOW | DOCUMENT-AS-IS | — |
| NFR-O-X | Observability | LOW | DEFER | — |
| NFR-SCA-1 | Scalability | LOW | DOCUMENT-AS-IS | — |
| NFR-SCA-2 | Scalability | LOW | DEFER | — |
| NFR-SCA-3 | Scalability | LOW | DOCUMENT-AS-IS | — |

**Phase 3 routing summary:**
- FIX-IN-PHASE-3: 10 (1 CRITICAL, 5 HIGH, 2 MEDIUM, 2 LOW — includes NFR-R-NEW-1)
- SECURITY-DECIDE: 3 (1 HIGH, 2 MEDIUM)
- POLICY-DECISION: 3 (3 MEDIUM)
- DOCUMENT-AS-IS: 13 (LOW or MEDIUM; NFR-R-NEW-1 moved to FIX-IN-PHASE-3; NFR-O-K merged into NFR-S-D at Pass 7)
- DEFER: 12 (MEDIUM and LOW)

**Total: 41** (42 rows − NFR-O-K merged into NFR-S-D at adversary Pass 7. NFR-S-F added per ADV-P3-007. NFR-S-E severity promoted LOW→HIGH per ADV-P2-004.)

**Counting clarification** (ADV-P2-005 + ADV-P3-007 + ADV-P7-002 reconciliation): The NFR Summary Table contains 41 individually-enumerated rows. Severity breakdown: 1 CRITICAL / 6 HIGH / 15 MEDIUM / 19 LOW = 41. This is the canonical count. NFR-O-K was a duplicate of NFR-S-D (same site src/config.rs:113-140, same routing DOCUMENT-AS-IS, same fix) and was merged at adversary Pass 7. Prior counting clarifications referencing 39 rows, 41, 42, 44 cumulative, or 45 total were inconsistent; this count supersedes them. Every row in the Summary Table represents a distinct NFR concern in the dimension body tables above. No phantom rows exist.

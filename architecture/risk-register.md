# Risk Register — jr (jira-cli)

**traces_to:** README.md
**Source:** Pass 1 R1 §5 (26 risks) + R2 §7 (1 severity escalation) + Pass 2 ADV-P2-004 (1 new HIGH) + Pass 6 ADV-P6-004 (R-H3 demoted HIGH→MEDIUM) + Pass 8 ADV-P8-003 (R-M3 merged into R-L11 — Retry-After duplicate)
**Total risks:** 26 (11 R1-NEW + 14 broad-pass + 1 R1-NEW reclassified to CRITICAL + 1 Pass-2 addition; R-M3 merged into R-L11 at Pass 8)
**Severity distribution:** 1 CRITICAL / 6 HIGH / 8 MEDIUM / 11 LOW

> **Numbering note:** R1-NEW-10 (multi-profile fields silent regression, NFR-R-D) was elevated from MEDIUM to CRITICAL during Pass 4 R1 analysis and appears as R-C1 in the CRITICAL block below. The R1-NEW label is not repeated in the numbered sequence; the CRITICAL block carries it. Effective R1-NEW count in the MEDIUM/HIGH rows is 11 (NEW-1 through NEW-9, NEW-11, NEW-12).

---

## CRITICAL (1)

| # | Risk | NFR | BC Anchor | ADR | Phase 3 Action |
|---|------|-----|-----------|-----|----------------|
| **R-C1** (NFR-R-D) | Multi-profile fields silent regression: 14 read sites use `config.global.fields.*` path; per-profile `story_points_field_id`/`team_field_id` in `ProfileConfig` are never read by handlers. Cross-profile correctness failure — sandbox vs prod custom-field IDs silently disagree. Elevated from MEDIUM to CRITICAL by Pass 4 R1 NFR-R-D. | NFR-R-D | BC-6.3.001 | ADR-0007 | FIX-IN-PHASE-3: Add `Config::field_id(FieldKind, profile)` accessor; replace all 14 `config.global.fields.*` reads; add integration test |

---

## HIGH (6)

| # | Risk | NFR | BC Anchor | ADR | Phase 3 Action |
|---|------|-----|-----------|-----|----------------|
| **R-H1** (R1-NEW-1) | Multi-workspace asset HashMap mis-attribution: Pass 2 dedup key is `(wid, oid)` (correct), but Pass 2 result map is keyed by `oid` alone (`cli/issue/list.rs:446`). Second insertion silently wins on `oid` collision across workspaces. Single-workspace tenants unaffected. | NFR-R-E | BC-4.3.001 | ADR-0008 | FIX-IN-PHASE-3: Change key type to `(String, String)` at 3 sites |
| **R-H2** (R1-NEW-2) | `JR_AUTH_HEADER` env-var honored in production binary with no `#[cfg(test)]` gate (`api/client.rs:64-66`). Any process inheriting this env var bypasses keychain auth entirely. Privilege escalation risk in CI/CD environments. | NFR-S-B | — | — | SECURITY-DECIDE: Option (a) `#[cfg(test)]` gate; Option (b) require simultaneous `JR_BASE_URL` |
| **R-H3** (R1-NEW-7) | `handle_open` uses `client.base_url()` not `instance_url()` (`workflow.rs:636`). For OAuth profiles, `base_url()` returns `https://api.atlassian.com/ex/jira/<cloud_id>` — browser opens 404. One-line fix. | NFR-R-B | BC-3.4.001 | ADR-0009 | FIX-IN-PHASE-3: `base_url()` → `instance_url()` |
| **R-H4** (R1-NEW-8) | `list_worklogs` non-paginated: fetches `OffsetPage<Worklog>` and returns `.items().to_vec()` — no loop. Silent data loss past page 1 for issues with >50 worklogs. | NFR-R-A | BC-X.5.002 | ADR-0010 | FIX-IN-PHASE-3: Refactor to `paginate_offset` loop |
| **R-H5** | Supply-chain: 332 transitive Cargo deps for an OAuth-handling CLI. `cargo-deny` is wired in CI but `multiple-versions = "warn"` policy means version dupes don't fail the build. No SBOM published. | NFR-S-F | — | — | FIX-IN-PHASE-3: Enforce `multiple-versions = "deny"` in `deny.toml`; publish SBOM. See NFR-S-F in nfr-catalog.md. |
| **R-H6** (ADV-P2-004) | GitHub Actions floating-tag SHA pinning (NFR-S-E): all 8 action references in `ci.yml` + `release.yml` use mutable version tags (`@v6`, `@v2`, `@v7`, etc.) rather than full commit SHAs. A force-pushed tag can redirect to attacker-controlled code in the same pipeline that injects `JR_BUILD_OAUTH_CLIENT_ID`/`JR_BUILD_OAUTH_CLIENT_SECRET`. Rare event (requires tag-force-push on upstream action repos) but high impact (direct OAuth client secret exposure). Severity rebased to HIGH per Pass-2 ADV-P2-004 reconciliation (was CRITICAL in cicd-setup.md GAP-1). | NFR-S-E | — | — | FIX-IN-PHASE-3: Pin all 8 action references to full commit SHA in `ci.yml` + `release.yml`; enable dependabot github-actions ecosystem to keep SHAs current. See cicd-setup.md GAP-1. |

---

## MEDIUM (8)

| # | Risk | NFR | Phase 3 Action |
|---|------|-----|----------------|
| **R-M0** (R1-NEW-3; formerly R-H3 — reclassified MEDIUM per ADV-P6-004) | `--verbose` dumps full HTTP request bodies including user-typed content (comments, summaries, descriptions, accountIds, emails). Authorization header is NOT logged, but body is. Users piping `2>log.txt` for debugging leak payload bytes. **MEDIUM rationale:** `--verbose` is opt-in; Auth header already redacted; PII exposure is user-controlled. Matches NFR-S-C (MEDIUM). ID R-H3 retained in pass-6 notes for traceability; canonical ID here is R-M0. | NFR-S-C | SECURITY-DECIDE: Add `redact_body()` helper; or default verbose to header-only with `--verbose-bodies` opt-in |
| **R-M1** (R1-NEW-4) | OAuth flow uses NO PKCE (NEW-INV-178). `build_authorize_url` sends no `code_challenge`. `exchange_code_for_token` sends no `code_verifier`. RFC 8252 recommends PKCE for native apps. ADR-0006 accepts the confidential-client model with embedded secret; PKCE is an addendum question. | NFR-S-A | SECURITY-DECIDE: Add RFC 7636 PKCE (~30 LOC). Cross-reference ADR-0006 addendum. |
| **R-M2** (R1-NEW-5) | `accessible_resources` first-result-wins (`api/auth.rs:666-668`). No `--site` flag, no count, no disambiguation. User with multiple cloud sites may silently authenticate to the wrong one. | NFR-O-S | DEFER: Add `--cloud-id <ID>` flag + interactive prompt or `--no-input` error. P1 priority. |
| **R-M4** (R1-NEW-9) | `worklog add` hardcodes 8h/day, 5d/week constants. Jira instances can configure different values via `/rest/api/3/configuration/timetracking`. Silent wrong-answer for non-standard setups. | NFR-R-C | FIX-IN-PHASE-3: Fetch + cache timetracking config (7-day TTL); fall back to 8/5 on miss. |
| **R-M5** | `cli/issue/list.rs` at 1,083 LOC is past the ~1000 LOC shard threshold. High branch density (booleans, Options, mutually-exclusive flag pairs). Continued growth risks undocumented edge cases. | NFR-O-D | DEFER: Per ADR-0012, the shard rule is now codified; future additions to `list.rs` trigger a new shard spec. |
| **R-M6** | `cli/auth.rs` at 1,998 LOC is the largest single file. Contains: API-token login, OAuth flow orchestration, credential resolution, profile lifecycle (7 subcommands). Cohesive but expensive to evolve safely. | NFR-O-D | DEFER: Shard via `cli/auth/{login,switch,status,refresh,logout,remove,helpers}.rs`. P1 priority. |
| **R-M7** | ADF round-trip is lossy for `mention`, `emoji`, `inlineCard`, `media` nodes — silently dropped in `adf_to_text` via `_` fall-through. Users viewing issues with rich content see incomplete text. | NFR-O-I | DEFER: Render `@<displayName>` for mentions, `:emoji:` for emojis, `[<title>](url)` for inlineCard. Medium-effort. |
| **R-M8** | `--internal` flag silently no-ops on non-JSM projects (NEW-INV-257). UX surprise — user expects an error when the flag is inapplicable. | — | DOCUMENT-AS-IS: Add comment in `cli/queue.rs` and CLAUDE.md Gotchas. |

---

## LOW (11)

| # | Risk | NFR | Phase 3 Action |
|---|------|-----|----------------|
| **R-L1** (R1-NEW-11) | Per-profile cache signature is convention-only (no compile-time fence). A future free-function cache reader without `profile` param compiles successfully but leaks data across profiles. | NFR-SCA-2 | DEFER: ADR-0011 documents the `Profile(String)` newtype option (DEFERRED). |
| **R-L2** (R1-NEW-12) | `get_changelog` anti-loop guard exists at one site. `search_issues` cursor loop has no analogous guard against cursor-equals-cursor regression. | NFR-R-F | DOCUMENT-AS-IS: Add similar guard to `search_issues`. |
| **R-L3** | Embedded OAuth XOR obfuscation is reversible by design. ADR-0006 is explicit: XOR defeats automated scanners, not motivated attackers. Per-build random key is the mitigation. | — | DOCUMENT-AS-IS: Accepted threat model per ADR-0006. |
| **R-L4** | Ctrl+C cancellation is abrupt (`process::exit(130)` from `tokio::select!`). In-flight HTTP, OAuth callback listener, partial cache writes are all dropped. | — | DOCUMENT-AS-IS: Acceptable for CLI. User-recoverable for auth partial states via `jr auth refresh`. |
| **R-L5** | `observability.rs` is intentionally tiny (39 LOC, no tracing crate). Constrains debugging in production, limits AI-agent integration paths. | NFR-O-A | DEFER: Adopt `tracing` crate + `tracing-subscriber` in `main.rs`. P2 priority. |
| **R-L6** | Two CLI dispatch paths for `Auth` subcommands. Most subcommands flow `main.rs → cli::<cmd>::handle(...)`. `Auth` is special — main.rs composes effective profile before dispatching to per-variant handlers. | — | DOCUMENT-AS-IS: Intentional, documented in source comment (`main.rs:84-91`). |
| **R-L7** | Keychain prompt at `JiraClient` construction time (macOS). On new binary install, user gets "jr wants to access your keychain" prompt per command until ACL is approved. | — | DOCUMENT-AS-IS: Inherent, well-documented. Workaround: `jr auth refresh` rebinds ACL. |
| **R-L8** | Non-atomic cache writes (`std::fs::write`). Crash between start and end leaves indeterminate state. Mitigation: deserialization failure on next read → `Ok(None)` (self-healing). | NFR-R-G | DOCUMENT-AS-IS: Self-healing already. Optional: use temp-file + rename pattern. |
| **R-L9** | `parse_duration` silently wraps on multiplicative overflow for pathological inputs in release builds (`panic=abort`). Wrong duration value sent to Jira API. | NFR-R-NEW-2 | DOCUMENT-AS-IS: Use `checked_mul`; bail with error. ~5 LOC fix. |
| **R-L10** | 4 distinct JSON boolean field names in write-op output (`changed`, `updated`, `linked`, `unlinked`). AI-agent integrators must learn per-command semantics. Snapshot-pinned (change is high-friction). | NFR-O-J | POLICY-DECISION: Adopt `success: bool` + `action: string` canonical shape, OR document 4-name vocabulary as deliberate verb-aligned. |
| **R-L11** | `Retry-After` parser: integer-only (no HTTP-date). Atlassian sends integers in practice; low observed risk. (R-M3 merged here at adversary Pass 8 — same concern, NFR-SCA-1 LOW is authoritative.) | NFR-SCA-1 | DOCUMENT-AS-IS: Add HTTP-date fallback if observed in production. |

---

## Risk Summary

| Severity | Count | Top action |
|----------|------:|-----------|
| CRITICAL | 1 | FIX-IN-PHASE-3 (NFR-R-D multi-profile fields) |
| HIGH | 6 | 5× FIX-IN-PHASE-3, 1× SECURITY-DECIDE |
| MEDIUM | 8 | 4× DEFER, 1× DOCUMENT-AS-IS, 1× FIX-IN-PHASE-3, 2× SECURITY-DECIDE (R-M3 merged into R-L11 at Pass 8) |
| LOW | 11 | 8× DOCUMENT-AS-IS, 2× DEFER, 1× POLICY-DECISION |
| **Total** | **26** | |

---

## ADR-0006 Addendum Note (NFR-S-A / PKCE)

ADR-0006 (embedded OAuth) was written with a specific threat model: embedded `client_secret` is an Atlassian-accepted pattern (confirmed by `acli` reference). PKCE (NFR-S-A, NEW-INV-178) is a separate finding: the current code has no `code_challenge` in the authorize URL and no `code_verifier` in the token exchange. RFC 8252 recommends PKCE for native apps regardless of confidential-client status.

**Tension with ADR-0006:** ADR-0006 explicitly states "Atlassian OAuth 2.0 (3LO) requires a `client_secret` for the token exchange step as of 2026-04 — there is no PKCE / public-client flow." If Atlassian indeed requires `client_secret`, PKCE adds defense in depth without replacing the secret. The Phase 3 SECURITY-DECIDE must clarify whether Atlassian's current API now accepts PKCE-only (public client), or whether PKCE+secret is the correct combined approach. An ADR-0006 addendum should record the decision.

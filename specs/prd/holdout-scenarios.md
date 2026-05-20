---
context: holdout-scenarios
title: "Holdout Scenarios"
total_holdouts: 55
# H-NEW-AUTH-002 registered by S-0.07 (Phase 3, 2026-05-07). Wave 0 COMPLETE.
# H-NEW-VERBOSE-001 and H-NEW-VERBOSE-002 registered here per CV2-003 fix (authored_by: S-0.06).
version: "1.1.1"
last_updated: 2026-05-18
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/
  - Source broad P3: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §4 (H-001..H-020)
  - Source R1: .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md §4 (H-021..H-029)
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.9 (H-030..H-047)
  - Source BC-NFR-R-D: .factory/semport/jira-cli/jira-cli-bc-nfr-r-d-draft.md (H-NEW-MP-001)
---

# Holdout Scenarios — jira-cli

55 holdout scenarios for Phase 4 evaluation. Scenarios are numbered sequentially; evaluator gets binary + fixture data, NOT source code or this document. Expected outputs are precise.

Setup uses:
- `XDG_CONFIG_HOME` / `XDG_CACHE_HOME` pointing to temp directories
- `JR_BASE_URL` pointing to a local wiremock/mock server (Rust `wiremock` crate pattern)
- `JR_SERVICE_NAME=jr-jira-cli-test` to isolate keychain (where applicable)
- `assert_cmd` (process-spawn) or `JiraClient::new_for_test` (library-level) for invocation

**Note on H-NEW-* format**: Holdouts H-NEW-MP-001, H-NEW-VERBOSE-001, H-NEW-VERBOSE-002, and H-NEW-AUTH-002 use an extended format with explicit `**Status**`, `**Verification**`, and prepended NFR/BC fields. This is deliberate for net-new holdouts that anchor MUST-FIX BCs discovered post-corpus-lock. H-001..H-047 use the legacy compact format established during corpus creation. Phase 4 evaluators should parse both shapes.

**Holdout Retirement Policy (S-3.10):** Holdouts pin user-observable behavior. If the target of a holdout becomes an internal helper with no production caller (i.e., no longer user-observable), the holdout must be rewritten or retired in the same story that introduces the deprecation, not deferred. This rule was codified after S-2.06 v1→v2 pivoted away from the client-side parse_duration calculator without retiring H-018 in the same wave (gap closed in S-3.10).

---

## Group 1: Foundational / Mixed Edge Cases (H-001..H-029)

### H-001: `auth status` first-run gives helpful guidance, not error
**Setup**: empty `XDG_CONFIG_HOME`. No env vars.
**Action**: `jr auth status`
**Expected**: exit 0; stderr contains `No profiles configured`.
**Why hidden**: Setup scripts probe with this command. Regression here breaks every onboarding flow.
**BC refs**: BC-1.1.002

---

### H-002: `auth list --output json` returns `[]` for fresh install
**Setup**: empty `XDG_CONFIG_HOME`.
**Action**: `jr auth list --output json`
**Expected**: exit 0; stdout = `[]`.
**Why hidden**: JSON shape is the parsing contract for orchestrators.
**BC refs**: BC-1.1.001

---

### H-003: Profile precedence — flag > env > config > "default"
**Setup**: config.toml with three profiles `from-config / from-env / from-flag` + `default_profile = "from-config"`. Set `JR_PROFILE=from-env`.
**Action**: `jr --profile from-flag auth list --output json`
**Expected**: exit 0; exactly one element with `"active": true` and `"name": "from-flag"`.
**Why hidden**: Multi-source precedence is invisible from any single test.
**BC refs**: BC-1.1.007
**Holdout H-003 notes**: Must set all 3 simultaneously. Variation: remove `--profile from-flag` → `from-env` wins; remove both → `from-config` wins.

---

### H-004: `auth refresh --no-input` against unconfigured profile fails clearly
**Setup**: empty config. Set `JR_SERVICE_NAME=jr-jira-cli-test` to isolate keychain.
**Action**: `jr --no-input auth refresh`
**Expected**: exit 64; stderr contains `no URL configured`, `jr auth login`, `--url`. Stderr does NOT contain `panic`.
**Why hidden**: Pre-fix behavior was to clear creds then prompt for email — destructive misleading recovery.
**BC refs**: BC-1.1.011

---

### H-005: Malformed config TOML errors with exit 78 and does NOT overwrite the file
**Setup**: write malformed TOML (`[unclosed\nbad = \n`) at `XDG_CONFIG_HOME/jr/config.toml`. Capture file bytes.
**Action**: `jr auth login --oauth --client-id X --client-secret Y --no-input`
**Expected**: exit 78; stderr contains `toml` or `parse`; file bytes are unchanged.
**Why hidden**: Pre-fix bug silently overwrote with defaults — destroyed user settings.
**BC refs**: BC-1.1.012
**Source**: `tests/auth_login_config_errors.rs:18-97`

---

### H-006: `issue move FOO-1 "In Progress"` is idempotent when already in target
**Setup**: wiremock returns `GET /issue/FOO-1` with `status.name = "In Progress"`. Mock POST transitions with `expect(0)`.
**Action**: `jr issue move FOO-1 "In Progress" --output json`
**Expected**: exit 0; stdout JSON has `"changed": false`. POST mock not invoked. (v2026-05-08: corrected from `"transitioned"` to `"changed"` per S-2.07 v2.0.0; canonical at src/cli/issue/json_output.rs:4-10)
**Why hidden**: Idempotency is invisible in success-only tests.
**BC refs**: BC-3.2.001

---

### H-007: `issue move FOO-1 Done` against state requiring resolution surfaces `--resolution` hint
**Setup**: transitions list has Done; current status In Progress; POST transitions returns 400 `{errors: {resolution: "Field 'resolution' is required"}}`.
**Action**: `jr --no-input issue move FOO-1 Done`
**Expected**: exit non-zero; stderr contains both `--resolution` AND `jr issue resolutions`.
**Why hidden**: Atlassian's raw error wording is unfriendly. The remediation rewrite is the user-value.
**BC refs**: BC-3.2.009

---

### H-008: `issue list --status prog` (single-substring) errors without firing JQL search
**Setup**: project statuses `[To Do, In Progress, Done]`. Wiremock POST `/search/jql` `expect(0)`.
**Action**: `jr --no-input issue list --status prog` (in `.jr.toml::project="PROJ"` cwd)
**Expected**: exit 64; stderr `Ambiguous status` + `In Progress`. JQL search mock not called.
**Why hidden**: Pin from issue #193 — strict-matching rollout. Behavior boundary invisible without mock count.
**BC refs**: BC-2.1.013

---

### H-009: `issue list` with corrupt `teams.json` is non-fatal; UUID + cache hint shown
**Setup**: write `{"teams": [` (truncated) to `~/.cache/jr/v1/default/teams.json`. Mock issue with team UUID `<u>`.
**Action**: `jr issue view PROJ-1`
**Expected**: exit 0; stdout contains `<u>` AND `name not cached` AND `jr team list --refresh`. stderr no panic.
**Why hidden**: Format-change graceful degradation.
**BC refs**: BC-2.3.035
**Source**: `tests/issue_view_errors.rs:BC-1135d`

---

### H-010: `--all` issue list returns more than 30; default truncates with hint
**Setup**: wiremock returns 35 issues in one cursor page.
**Action**: `jr issue list --jql "project = X" --all --output json` then `jr issue list --jql "project = X" --output json`
**Expected**: first → JSON array length 35. Second → JSON array length 30 AND stderr contains `Showing 30 results` or `~`.
**Why hidden**: Pagination cap regulated by request body shape; invisible from output count alone.
**BC refs**: BC-2.2.018, BC-2.2.019

---

### H-011: Legacy `[instance]` config migrates to `[profiles.default]` on first load (idempotent)
**Setup**: write legacy `[instance] / [fields] / [defaults]` config to disk.
**Action**: load config twice (e.g., `jr auth list` twice).
**Expected**: After first load, on-disk file has `[profiles.default]`, no `[instance]`/`[fields]`. After second load, file is byte-identical to after first.
**Why hidden**: Migration is one-shot and silent; idempotency invisible without bytewise comparison.
**BC refs**: BC-6.1.001, BC-6.1.002

---

### H-012: 401 with `scope does not match` body produces InsufficientScope error with workaround docs
**Setup**: wiremock POST `/rest/api/3/issue` returns 401 body `{message: "Unauthorized; scope does not match"}`.
**Action**: any command that triggers post (e.g., `issue create`).
**Expected**: exit 2; stderr contains `Insufficient token scope`, `write:jira-work`, `OAuth 2.0`, `github.com/Zious11/jira-cli/issues/185`.
**Why hidden**: A future tightening of the substring match would silently break this.
**BC refs**: BC-1.6.042, BC-X.3.005
**Source**: `tests/api_client.rs:99-255`

---

### H-013: 429 retry — `send_raw` returns 429 to caller after MAX_RETRIES=3
**Setup**: wiremock GET responds 429 with `Retry-After: 0` for 4 calls (`expect(4)`).
**Action**: `client.send_raw(GET /myself)`.
**Expected**: response status = 429 (NOT an error). Exactly 4 calls fired. Stderr contains `warning: rate limited by Jira — gave up after 3 retries. Wait a moment and try again.`
**Why hidden**: Retry semantics for `jr api` raw passthrough must NOT raise.
**BC refs**: BC-X.1.005

---

### H-014: `assign --to <name>` against duplicate display names + `--no-input` errors with email/accountId disambiguation
**Setup**: assignable user search returns two users with same `displayName` `"John Smith"`.
**Action**: `jr issue assign FOO-1 --to "John Smith" --no-input`
**Expected**: exit non-zero; stderr contains both emails AND both accountIds.
**Why hidden**: AI-agent ergonomic — needs accountId to retry.
**BC refs**: BC-X.7.004
**Source**: `tests/duplicate_user_disambiguation.rs`

---

### H-015: clap mutual-exclusion: `--all` and `--limit` together fails fast
**Setup**: none.
**Action**: `jr issue list --all --limit 10`
**Expected**: exit non-zero; stderr contains `cannot be used with`.
**Why hidden**: Many subcommands have similar conflicts; checking one regression-detects refactor mistakes.
**BC refs**: BC-2.2.020

---

### H-016: `auth remove <active>` is rejected
**Setup**: config with `default_profile = "default"` and `[profiles.default]` set.
**Action**: `jr --no-input auth remove default`
**Expected**: exit 64; stderr contains `cannot remove active`. Config file unchanged.
**Why hidden**: Destructive operation safety; failure here would break invariants others depend on.
**BC refs**: BC-1.1.006

---

### H-017: AQL clause uses field NAME + capital `Key`
**Setup**: caller passes `cmdb_fields = [("customfield_10191", "Client")]`, asset_key = `CUST-5`.
**Action**: invoke `jql::build_asset_clause("CUST-5", &fields)`.
**Expected**: exact string `"Client" IN aqlFunction("Key = \"CUST-5\"")`. Not `customfield_10191`; capital `Key` not `objectKey`.
**Why hidden**: Two CLAUDE.md gotchas conflated in one helper.
**BC refs**: BC-4.1.002
**Source**: `src/jql.rs:278-308 (build_asset_clause_* unit tests)`

---

### H-019: Profile name `foo:bar` rejected at THREE boundaries
**Setup**: three variants — (a) `--profile foo:bar` flag; (b) config with `[profiles."foo:bar"]`; (c) `JR_PROFILE=foo:bar` against existing profile.
**Action**: any non-init `jr` command for each variant.
**Expected**: each → exit 64.
**Why hidden**: Validates the security boundary protecting cache paths and keychain-key namespaces.
**BC refs**: BC-6.1.004

---

### H-020: `--output json` error shape is structured `{"error", "code"}` to stderr
**Setup**: any command that errors (e.g., `jr --output json auth switch ghost` against config without `[profiles.ghost]`).
**Action**: above.
**Expected**: exit 64; stderr is parseable JSON with keys `error` (string) and `code` (number 64).
**Why hidden**: Programmatic consumers depend on this shape; not asserted by most unit tests.
**BC refs**: BC-7.3.005

---

### H-021: `--status prog` ambiguous rejection short-circuits BEFORE JQL search
**Setup**: project statuses `[To Do, In Progress, Done]`. Wiremock `POST /search/jql` mock with `expect(0)`.
**Action**: `jr --no-input issue list --status prog`
**Expected**: exit 64; stderr `Ambiguous status "prog". Matches: In Progress`. JQL search mock NOT called.
**Why hidden**: Invisible without verifying mock-call count.
**BC refs**: BC-2.1.007

---

### H-022: 401-scope-mismatch dispatch boundary — case sensitivity, status gate, substring match
**Setup**: 4 wiremock fixtures: 401 with `scope does not match`; 401 with `Scope Does Not Match`; 401 with `Session expired`; 403 with `scope does not match policy`.
**Action**: 4 separate API calls.
**Expected**: First two → InsufficientScope (exit 2); third → NotAuthenticated (exit 2); fourth → ApiError 403 (exit 1).
**Why hidden**: Pin against three independent regressions: drop `to_ascii_lowercase`, broaden status gate, tighten substring.
**BC refs**: BC-1.6.043, BC-1.6.044, BC-1.6.045

---

### H-023: `--asset KEY` ambiguous AQL search short-circuits BEFORE issue search
**Setup**: Workspace mock + AQL search returning two assets both containing input substring. `Mock::expect(0)` on `POST /search/jql`.
**Action**: `jr --no-input issue list --asset Acme`
**Expected**: exit 64 + stderr `Multiple assets match` + both candidate labels. JQL search mock NOT called.
**Why hidden**: Pin against asset-resolution short-circuit regression.
**BC refs**: BC-2.1.012

---

### H-024: `assets schema <type-substring>` ambiguous short-circuits before per-type attribute fetch
**Setup**: Schema list mock + object-type listing with two ambiguous candidates. `Mock::expect(0)` on per-type attribute endpoints.
**Action**: `jr --no-input assets schema Serv`
**Expected**: exit 64 + stderr `Ambiguous type` + both candidate names. Per-type attribute mocks NOT called.
**Why hidden**: Short-circuit before expensive fetch (BC-4.2.007).
**BC refs**: BC-4.2.007

---

### H-025: Cache write atomicity — non-atomic `std::fs::write` is the documented contract
**Setup**: Write a partial-file teams.json (truncated mid-write).
**Action**: `jr issue view PROJ-1` against issue with team UUID.
**Expected**: exit 0 + UUID + "name not cached" hint inline.
**Why hidden**: Pin against a future "atomic-write" refactor; current contract IS non-atomic-write + read-side resilience.
**BC refs**: BC-6.2.014

---

### H-026: `errors{}` with mixed types and nested values renders correctly
**Setup**: Wiremock returns 400 body with `{errorMessages: [], errors: {summary: "is req", components: ["a","b"], customfield_10001: {messages:["invalid"]}}}`.
**Action**: any command that triggers a 400 (e.g., `jr issue create`).
**Expected**: stderr contains `summary: is req`, `components: ["a","b"]`, `customfield_10001: {"messages":["invalid"]}` — all alphabetical-sorted.
**Why hidden**: Pin extract_error_message BC-1201a/b/c.
**BC refs**: BC-7.3.002

---

### H-027: `Retry-After: 86400` (24h) — parsed value preserved without upper bound (KNOWN-GAP pin)
**Setup**: Construct a `http::HeaderMap` containing `Retry-After: 86400`. Call `RateLimitInfo::from_headers(&headers)` directly (unit test — no Wiremock, no process spawn, no real-time clock dependency).
**Action**: Assert `rate_limit_info.retry_after_secs == 86400`.
**Expected**: With the MAX_RETRY_AFTER_SECS cap (S-3.07), `RateLimitInfo::from_headers` (or its replacement after S-3.07) returns an "abort" signal when retry_after_secs exceeds 60. The literal value 86400 is parsed without overflow but the abort signal is honored — no 24-hour sleep occurs. Test passes against post-S-3.07 code.
**Status**: MUST-PASS (S-3.07 added MAX_RETRY_AFTER_SECS=60 cap; verified by AC-001 + AC-002 + AC-003 in tests/rate_limit_cap_tests.rs and tests/rate_limit_cap_ac003.rs)
**Why hidden**: Pin Pass 4 §7.1.3 NFR gap as an explicit holdout against silent fixes that add an upper bound cap. Reframed from retry-loop test (ADV-P22-004: Mock::expect(2) + 5s window were internally contradictory with an 86400s delay).
**BC refs**: BC-X.4.002 (current behavior pinned — no cap); BC-X.4.009 (future MUST-FAIL when MAX_RETRY_AFTER_SECS=60 cap is implemented — flip assertion to `retry_after_secs == 60`)

---

### H-028: Hand-edited config with `[profiles."foo:bar"]` TOML key rejected at load (config-load boundary only)
**Note**: H-019 covers all three validation boundaries simultaneously. H-028 isolates the config-file parse path specifically — the scenario where a power user directly edits config.toml with an illegal profile name.
**Setup**: Write `~/.config/jr/config.toml` with `[profiles."foo:bar"]` block by hand. No flag or env-var involvement.
**Action**: `jr auth list`
**Expected**: exit 64; stderr contains `invalid profile name`; no profile data returned.
**Why hidden**: Config-file-load validation is independent from clap-flag validation and env-var validation (different code path in `Config::load_with`). This path (key iteration) is separate from pass-2 (resolved active name) and flag-level pass.
**BC refs**: BC-6.1.004, BC-6.1.005

---

### H-029: BYO OAuth uses dynamic port; embedded uses fixed port 53682
**Setup**: Two invocations: (a) `jr auth login --oauth` (embedded) and (b) `jr auth login --oauth --client-id X --client-secret Y` (BYO).
**Action**: Inspect callback URL in each case.
**Expected**: (a) callback URL = `http://127.0.0.1:53682/callback` (exact literal). (b) callback URL = `http://localhost:<random_port>/callback` (dynamic, NOT 53682, NOT IPv4).
**Why hidden**: Pin ADR-0006's "BYO sources keep dynamic-port behavior" contract.
**BC refs**: BC-1.5.034, BC-1.5.031

---

## Group 2: Issue Read, JQL, Filtering, and Error Extraction (H-030..H-035)

### H-030: `extract_error_message` empty-body precedence (FIRST not LAST)
**Setup**: Wiremock returns 400 with empty response body (byte length == 0).
**Action**: any command that triggers a 400.
**Expected**: stderr message contains the literal string `"<empty response body>"` — this IS the return value from `extract_error_message` for a zero-length body. There is no status-code-derived substitution.
**Why hidden**: CONV-ABS-004 — broad pass had empty-body LAST; corrected to FIRST. ADV-P2-001 corrected the expected behavior from "status-derived" to "literal string". Easy to regress on ordering changes.
**BC refs**: BC-7.3.001

---

### H-031: `user search --all` continues past short non-empty page (JRACLOUD-71293 workaround)
**Setup**: Wiremock pages: 100 users, then 35 users, then 100 users, then empty.
**Action**: `jr user search u --all --output json`
**Expected**: JSON array length = 235. No `"duplicates"` or `"missing"` users. `start_at` advances by `USER_PAGE_SIZE` (100), NOT by returned count.
**Why hidden**: A "fix" that advances by returned-count would produce duplicates per JRACLOUD-71293.
**BC refs**: BC-X.7.006, BC-X.2.005

---

### H-032: `user search --all` hits safety cap with warning
**Setup**: Wiremock returns 100 users per page indefinitely (unbounded responder).
**Action**: `jr user search u --all --output json`
**Expected**: exit 0; stderr contains `"hit pagination safety cap"` (user-visible warning). Array length = 1500 (`USER_PAGINATION_SAFETY_CAP`).
**Why hidden**: Pin against a refactor that removes the safety cap.
**BC refs**: BC-X.2.006

---

### H-033: `jr issue remote-link --url ftp://example.com` rejected pre-HTTP (scheme allowlist)
**Setup**: No wiremock needed; Wiremock `expect(0)` optional.
**Action**: `jr issue remote-link FOO-1 --url ftp://example.com`
**Expected**: exit 64; stderr contains `"http or https"` AND `"ftp"`. Zero HTTP calls.
**Why hidden**: Scheme allowlist is a user-safety contract; easy to regress.
**BC refs**: BC-3.7.004

---

### H-034: `jr issue remote-link` URL gains trailing slash from `url::Url::parse` normalization
**Setup**: Wiremock POST `/rest/api/3/issue/PROJ-123/remotelink` body contains `"url": "https://example.com/"` (WITH trailing slash).
**Action**: `jr issue remote-link PROJ-123 --url https://example.com --title "Example"`
**Expected**: stdout JSON has `"url": "https://example.com/"` (trailing slash added by normalization). Wiremock receives `url` with trailing slash in body.
**Why hidden**: `url::Url::parse` normalization is not obvious; easy to regress by changing URL handling.
**BC refs**: BC-3.7.001

---

### H-035: `issue list` combined filter — all filters and no panic
**Setup**: Wiremock with project statuses, team list, CMDB workspace. Mock JQL search returning 5 issues.
**Action**: `jr issue list --open --assignee "Jane" --created-after "2026-01-01" --status "In Progress" --team "engineering" --output json`
**Expected**: exit 0; stdout JSON array; all 5 issues present. No panic.
**Why hidden**: Combined multi-clause JQL composition is only individually tested; ordering bugs visible only with all clauses active.
**BC refs**: BC-2.1.001..BC-2.1.017

---

## Group 3: Assets / CMDB (H-036..H-039)

### H-036: Multi-workspace asset HashMap — `(wid, oid)` composite key (MUST-FIX pin)
**Setup**: Two workspaces `ws-A` and `ws-B` both return an asset with `oid = "OBJ-88"` but different names.
**Action**: `jr issue list --project PROJ --output json` with issues linked to both workspace assets.
**Expected (FIXED behavior)**: Each issue shows the correct asset name for its workspace. No last-write-wins collision.
**Status**: MUST-FIX (NFR-R-E). Current code fails this holdout — the holdout defines the target.
**BC refs**: BC-4.3.001

---

### H-037: `assets search` workspace discovery cached — second call fires no HTTP
**Setup**: First call populates workspace cache. Wipe HTTP mock server after first call.
**Action**: Second `jr assets search "Key = X"` invocation.
**Expected**: exit 0; no HTTP call to workspace endpoint; result from cache.
**Why hidden**: Cache hit is invisible from output alone.
**BC refs**: BC-4.2.001

---

### H-038: `enrich_assets` — already-resolved assets skip GET
**Setup**: `LinkedAsset` list with: (a) id-only, (b) id+key+name. Wiremock GET on asset endpoint with `expect(1)` (only asset-a fetched).
**Action**: invoke enrichment pipeline.
**Expected**: Only asset-a is fetched. Asset-b's key/name unchanged.
**Why hidden**: Skip-already-resolved invariant; invisible from output alone.
**BC refs**: BC-4.3.002

---

### H-039: `assets tickets --status PROG` ambiguous — exit 64 with candidates
**Setup**: Connected tickets with statuses `["In Progress", "Progressing"]`.
**Action**: `jr assets tickets OBJ-1 --status PROG`
**Expected**: exit 64; stderr `Ambiguous status`; stderr contains `In Progress` and `Progressing`.
**Why hidden**: Disambiguate against single partial-match accepting.
**BC refs**: BC-4.2.006

---

## Group 4: Sprint & Board (H-040..H-042)

### H-040: `sprint current` truncation — 30 default, --all bypasses, under-limit no hint
**Setup**: Sprint with 35 issues.
**Action**: (a) `jr sprint current` → (b) `jr sprint current --all` → (c) sprint with 10 issues, `jr sprint current`
**Expected**: (a) 30 results + stderr `Showing 30 results`. (b) 35 results + no hint. (c) 10 results + no hint.
**Why hidden**: Three-case truncation contract invisible from any single run.
**BC refs**: BC-5.2.005

---

### H-041: Sprint add JSON shape — sprint_id present; remove JSON shape — NO sprint_id
**Setup**: Sprint ID = 100. Issues `["TEST-1", "TEST-2"]`.
**Action**: `jr sprint add --sprint 100 TEST-1 TEST-2 --output json` and `jr sprint remove --sprint 100 TEST-1 TEST-2 --output json`
**Expected**: Add → `{"added": true, "issues": ["TEST-1", "TEST-2"], "sprint_id": 100}`. Remove → `{"issues": ["TEST-1", "TEST-2"], "removed": true}` (NO sprint_id).
**Why hidden**: Asymmetric add vs remove shapes — pin against "harmonization" that adds sprint_id to remove.
**BC refs**: BC-5.2.007, BC-5.2.008

---

### H-042: `sprint list` on kanban board — hard error with literal message
**Setup**: Board configured as kanban (`type = "kanban"`).
**Action**: `jr sprint list --board 1`
**Expected**: exit non-zero; stderr contains exact literal `Sprint commands are only available for scrum boards`.
**Why hidden**: Hard error (not silent degrade) is the documented asymmetry with `issue list`.
**BC refs**: BC-5.2.001

---

## Group 5: Output Rendering (H-043..H-044)

### H-043: Team column — conjunctive gate (configured AND populated)
**Setup**: Two issue lists: (a) `team_field_id` configured, one issue has team UUID; (b) `team_field_id` configured, NO issue has team UUID.
**Action**: `jr sprint current` for each.
**Expected**: (a) Team column appears. (b) Team column absent.
**Why hidden**: Conjunctive gate invisible from single-case tests.
**BC refs**: BC-5.3.001, BC-5.3.002

---

### H-044: `issue view` with ADF description — text output, no panic
**Setup**: Issue `PROJ-1` with ADF description containing heading, paragraph, code block, mention.
**Action**: `jr issue view PROJ-1`
**Expected**: exit 0; stdout contains the heading text and paragraph text (rendered). Mention node silently dropped (current behavior). No panic on any node type.
**Why hidden**: ADF node rendering is a large surface; easy to panic on unexpected node types.
**BC refs**: BC-7.2.001..BC-7.2.051

---

## Group 6: Reliability / MUST-FIX Pins (H-045..H-047, H-NEW-MP-001)

### H-045: `list_worklogs` pagination — all pages returned (MUST-FIX pin)
**Setup**: Wiremock: page 1 returns 50 worklogs (`total: 80, startAt: 0, maxResults: 50`); page 2 returns 30 worklogs (`total: 80, startAt: 50, maxResults: 50`).
**Action**: `jr worklog list PROJ-1 --output json`
**Expected (FIXED behavior)**: JSON array length = 80. Both pages fetched.
**Status**: MUST-FIX (NFR-R-A). Current code fails this holdout (returns 50, silently truncates).
**BC refs**: BC-X.5.002

---

### H-046: `jr issue open FOO-1` uses instance URL, not API gateway URL (MUST-FIX pin)
**Setup**: OAuth profile with `cloudId = "my-cloud-123"`. `client.base_url()` = `https://api.atlassian.com/ex/jira/my-cloud-123`. `client.instance_url()` = `https://mycompany.atlassian.net`.
**Fixture**: Use `JiraClient::new_for_test(base_url, auth_header)` constructor with OAuth-mode `Bearer` auth header. Wiremock at `JR_BASE_URL` simulates `https://api.atlassian.com/ex/jira/my-cloud-123`. Cross-reference H-029 for embedded OAuth login fixture pattern.
**Action**: `jr issue open FOO-1 --url-only` (print without opening browser)
**Expected (FIXED behavior)**: stdout contains `https://mycompany.atlassian.net/browse/FOO-1`. Does NOT contain `api.atlassian.com`.
**Status**: MUST-FIX (NFR-R-B). Current code fails this holdout for OAuth profiles.
**BC refs**: BC-3.4.001

---

### H-047: `accessible_resources` multi-cloudId disambiguation — MUST-PASS (elevated from KNOWN-GAP)
**Setup**: OAuth mock returns two cloud resources: `[{id: "cloud-A", name: "Company A", url: "https://company-a.atlassian.net"}, {id: "cloud-B", name: "Company B", url: "https://company-b.atlassian.net"}]`.
**Action**: `jr auth login --oauth --client-id X --client-secret Y --no-input`
**Expected**: exit 64; stderr contains an actionable listing of available cloud-ids (with name, URL, and cloudId for each org); user is instructed to re-run with `--cloud-id <id>`.
**Purpose**: NFR-O-S fulfilled. Disambiguates multi-org OAuth login. --cloud-id flag selects a specific org non-interactively; dialoguer::Select prompt activates on TTY without --no-input; --no-input + multi-org exits 64 with actionable listing.
**Status**: MUST-PASS (S-3.04 added --cloud-id flag + dialoguer::Select prompt + --no-input exit-64; elevated KNOWN-GAP → MUST-PASS by PR #320 / b6ab77c, 2026-05-09. Multi-cloudId disambiguation now implemented: --cloud-id flag for non-interactive scripts; dialoguer::Select prompt for TTY; exit 64 + actionable error for --no-input + multi-org. AC-006 of S-3.04 was the integration test that validates this closure.)
**BC refs**: BC-1.5.038, BC-1.1.007, BC-1.5.031

---

### H-NEW-MP-001: Multi-profile fields bug — profile B uses its own story-points field (MUST-FIX pin)
**NFR source**: NFR-R-D (CRITICAL)
**BC**: BC-6.3.001

**Setup**:
1. Config with two profiles:
   - Profile `prod`: `story_points_field_id = "customfield_10005"`
   - Profile `sandbox`: `story_points_field_id = "customfield_10099"`
2. Wiremock at `JR_BASE_URL` captures POST `/rest/api/3/issue` request body.

**Action**: `jr --profile sandbox issue create --summary "Test" --story-points 5 --type Story --project PROJ --no-input`

**Expected (FIXED behavior)**:
- POST body contains `"customfield_10099": 5` (profile `sandbox`'s field ID)
- POST body does NOT contain `"customfield_10005"` (profile `prod`'s field ID)
- exit 0

**Status**: MUST-FIX (NFR-R-D, CRITICAL). Current code fails this holdout — reads `config.global.fields.story_points_field_id` which returns `customfield_10005` regardless of profile.

**Verification**:
- Round-trip test: create profile `A` (field ID `customfield_A`) and `B` (field ID `customfield_B`). Assert each uses its own when `--profile A` or `--profile B` is set.
- Error message test: when `[profiles.sandbox]` has no `story_points_field_id`, error must reference `[profiles.sandbox]` not deprecated `[fields]`.

---

## Group 7: SD-003 Verbose-Bodies PII Safety (H-NEW-VERBOSE-001..H-NEW-VERBOSE-002)

### H-NEW-VERBOSE-001: `--verbose-bodies` emits PII warning to stderr (MUST-PASS)
**NFR source**: NFR-S-C
**BC**: BC-7.5.001
**SD anchor**: SD-003
**Authored by**: S-0.06

**Setup**:
1. Wiremock at `JR_BASE_URL` returns any valid 200 response for a simple GET (e.g., `GET /rest/api/3/myself`).
2. Config with a valid profile (real or mocked auth header via `JR_AUTH_HEADER` or test fixture).

**Action**: `jr --verbose-bodies auth status` (or any command that triggers at least one HTTP call)

**Expected (MUST-PASS)**:
- exit 0
- stderr contains ALL THREE of the following lines (in any order relative to body content, but before the first `[verbose] body:` line):
  1. `[jr] WARNING: --verbose-bodies prints request/response bodies to stderr.`
  2. `[jr] These bodies contain PII (accountId, emailAddress, ADF text content).`
  3. `[jr] Do not pipe to AI-agent contexts or shared logs without consent.`
- stderr also contains at least one `[verbose] body:` line (body content is printed)
- stderr does NOT contain the suppression hint `[verbose] body suppressed (use --verbose-bodies to inspect, will print PII)` (that hint is `--verbose`-only)

**Status**: MUST-PASS. Verifies SD-003 Option B postcondition: explicit opt-in body logging with mandatory PII warning.

**Verification**:
- Process-spawn test in `tests/verbose_bodies.rs`: assert stderr contains all three warning lines.
- Regression check: if a future change removes the warning or gates it behind another flag, this holdout fails.
- Cross-reference: SD-003 Resolution §3 lines 79-83; S-0.06 AC-003.

---

### H-NEW-VERBOSE-002: `--verbose` alone does NOT print body content (MUST-PASS + regression pin)
**NFR source**: NFR-S-C
**BC**: BC-7.5.001
**SD anchor**: SD-003
**Authored by**: S-0.06

**Setup**:
1. Wiremock at `JR_BASE_URL` returns a 200 response with a non-empty JSON body (e.g., `{"accountType": "atlassian", "emailAddress": "user@example.com"}`).
2. Config with a valid profile.

**Action**: `jr --verbose auth status` (without `--verbose-bodies`)

**Expected (MUST-PASS)**:
- exit 0
- stderr contains `[verbose] GET /rest/api/3/myself` (or equivalent method+URL line for the command)
- stderr contains the suppression hint: `[verbose] body suppressed (use --verbose-bodies to inspect, will print PII)`
- stderr does NOT contain `[verbose] body:` (no raw body bytes printed)
- stderr does NOT contain `emailAddress` or any PII field values from the response body
- stderr does NOT contain ANY of the three PII warning lines (`[jr] WARNING: --verbose-bodies...`, `[jr] These bodies contain PII...`, `[jr] Do not pipe...`) — those warnings appear ONLY with `--verbose-bodies`

**Status**: MUST-PASS. Regression pin: if a future change inadvertently re-enables body printing under `--verbose` alone (reverting SD-003 Option B), this holdout fails.

**Verification**:
- Process-spawn test in `tests/verbose_bodies.rs`: assert stderr contains suppression hint and does NOT contain `[verbose] body:`.
- Three-variant test: (a) `--verbose` alone → suppression hint, no body; (b) `--verbose-bodies` alone → warning + body, no suppression hint; (c) `--verbose --verbose-bodies` → warning + body + method/URL lines.
- Cross-reference: SD-003 Resolution §3 lines 68-76; S-0.06 AC-001, AC-002; H-NEW-VERBOSE-001.

---

## Group 8: SD-002 Release Binary Auth Gate (H-NEW-AUTH-002)

### H-NEW-AUTH-002: Release binary refuses `JR_AUTH_HEADER` auth bypass (MUST-PASS + regression pin)

**NFR source**: NFR-S-B
**BC**: BC-X.1.001
**SD anchor**: SD-002 (Option B-revised — `#[cfg(debug_assertions)]` compile-time gate, canonized 2026-05-07 during S-0.05)
**Authored by**: S-0.07
**gate_attribute**: `cfg(debug_assertions)`
**mode**: must-pass + regression

**Setup**:
1. Build jr in release mode: `cargo build --release`
2. Set `JR_AUTH_HEADER=Basic dGVzdEBleGFtcGxlLmNvbTpmYWtl` (a Base64-encoded fake credential) in the child process environment.
3. Empty `XDG_CONFIG_HOME` (no configured profiles, no keychain entries). Set `JR_SERVICE_NAME=jr-jira-cli-test` to isolate keychain.
4. No `JR_BASE_URL` set (or pointing to a non-listening address to ensure no real API call succeeds).

**Action**: `./target/release/jr auth status`

**Expected (MUST-PASS — post-S-0.05)**:
- Exit non-zero (64 — no profile configured, or 78 — config error); NOT exit 0
- `JR_AUTH_HEADER` is NOT used as the auth header; the binary behaves as if the env var were absent
- stderr does NOT contain any reference to `dGVzdEBleGFtcGxlLmNvbTpmYWtl` (the fake credential value)
- stderr does NOT contain `api.atlassian.com` (no successful API call against any server)
- The binary falls through to keychain lookup / config-error path, proving the env-var read compiled out

**Expected (MUST-FAIL — pre-S-0.05 at activation HEAD dea1664)**:
- The fake `JR_AUTH_HEADER` value is loaded into `JiraClient` unconditionally (src/api/client.rs:64-66 pre-fix)
- Combined with `JR_BASE_URL` pointing to a mock server, the fake header would be used for an API call — bypassing keychain auth entirely (security violation)
- Without a mock server, the command still exits early (URL not configured), but the env var IS present in the loaded client struct

**Verification**:
- Process-spawn test in `tests/auth_header_release_gate.rs`: gated behind `#[ignore]` and `JR_RUN_RELEASE_AUTH_GATE_TEST=1` to avoid requiring a release build in standard CI unit test runs.
- Assert exit code is 64 (no profile configured — NOT a fake-auth success).
- Assert stderr does not contain the fake credential string or any API server response.
- Regression check: if a future change re-introduces unconditional `JR_AUTH_HEADER` reading in a release build (by removing the `#[cfg(debug_assertions)]` gate), this holdout fails.

**Practical test note**: The gate is `#[cfg(debug_assertions)]`. Debug binaries (including `cargo_bin` subprocess binaries used in most integration tests) still honor `JR_AUTH_HEADER` — that is intentional to preserve ~151 subprocess integration tests. This holdout MUST therefore use a RELEASE binary (`./target/release/jr`) built with `cargo build --release`. A debug subprocess test (`assert_cmd::cargo::cargo_bin`) would NOT verify this holdout because `debug_assertions=true` in debug builds.

**Status**: MUST-PASS. Satisfies SD-002 Option B-revised postcondition. Pre-S-0.05: MUST-FAIL (holdout defines the target). Post-S-0.05 (at develop SHA d907504): MUST-PASS.
**BC refs**: BC-X.1.001, SD-002
**Added**: S-0.07, Phase 3 Wave 0 (2026-05-07)

---

## Group 9: JSM Request Types (issue #288)

### H-NEW-JSM-RT-001: JSM request creation via `issue create --request-type` routes to servicedeskapi endpoint (MUST-PASS)

**NFR source**: BC-3.8.001, BC-3.8.002
**BC**: BC-3.8.001, BC-3.8.002, BC-3.8.008
**Authored by**: F2 spec evolution (2026-05-18)

**Setup**:
1. Wiremock at `JR_BASE_URL`. Config: project `HELPDESK` with `typeKey = "service_desk"`.
2. Mock `GET /rest/servicedeskapi/servicedesk` returning `{values: [{id: "3", projectKey: "HELPDESK"}]}`.
3. Mock `GET /rest/servicedeskapi/servicedesk/3/requesttype` returning `{isLastPage: true, values: [{id: "5", name: "Get IT Help", description: "IT support"}]}`.
4. Mock `POST /rest/servicedeskapi/request` with `expect(1)` returning 201 `{issueId: "10042", issueKey: "HELP-42", currentStatus: {status: "Waiting for support"}, _links: {web: {href: "https://example.atlassian.net/browse/HELP-42"}}}`.
5. Mock `POST /rest/api/3/issue` with `expect(0)` (platform create must NOT be called).

**Action**: `jr issue create --project HELPDESK --request-type "Get IT Help" --summary "VPN broken" --no-input --output json`

**Expected (MUST-PASS)**:
- exit 0
- stdout JSON: `{"key": "HELP-42"}`
- `POST /rest/servicedeskapi/request` called exactly once (expect(1) satisfied)
- POST body contains: `"requestTypeId": "5"` AND `"serviceDeskId": "3"` AND `"requestFieldValues"` containing `"summary": "VPN broken"`
- `POST /rest/api/3/issue` NOT called (expect(0) satisfied)
- `--output json` payload: v1 emits minimal `{"key": "HELP-42"}` only; `.url` field from `_links.web.href` is NOT surfaced in v1 (browse URL exposure is deferred). Mock setup retains `_links` field for API fidelity but implementation does not map it to output. This assertion locks the v1 behavior (mirrors BC-3.8.001 output shape).

**Why hidden**: The routing branch decision between platform and JSM endpoints is invisible from output alone — mock call counts are required to pin which endpoint was invoked. A naive implementation could POST to both or route to the wrong one while still returning a key-shaped response.

**Status**: MUST-PASS. Core routing invariant for BC-3.8.001.

---

### H-NEW-JSM-RT-002: `issue create --request-type` on software project errors clean with JSM hint, zero POST (MUST-PASS)

**NFR source**: BC-3.8.002, BC-X.8.004
**BC**: BC-3.8.002, BC-X.8.004, BC-3.3.001 (modified — platform path NOT exercised)
**Authored by**: F2 spec evolution (2026-05-18); F1d adversary pass-01 (2026-05-18 — BC-3.3.001 annotation added)

**Setup**:
1. Wiremock at `JR_BASE_URL`. Config: project `PROJ` with `typeKey = "software"`.
2. Mock `GET /rest/servicedeskapi/servicedesk` returning `{values: []}` or returning project meta indicating software type (no service desk entry for PROJ).
3. Mock `POST /rest/servicedeskapi/request` with `expect(0)`.
4. Mock `POST /rest/api/3/issue` with `expect(0)`.

**Action**: `jr issue create --project PROJ --request-type "Get IT Help" --summary "VPN broken" --no-input`

**Expected (MUST-PASS)**:
- exit 64
- stderr contains `Jira Software project` AND actionable suggestion referencing JSM or queue commands
- `POST /rest/servicedeskapi/request` NOT called (expect(0) satisfied)
- `POST /rest/api/3/issue` NOT called (expect(0) satisfied)

**Why hidden**: The `require_service_desk` gate is a client-side check before any HTTP. Its correct invocation for `issue create --request-type` is invisible without mock-call verification. A regression where the dispatch branch bypasses the service-desk check would allow the JSM POST to attempt on a non-JSM project (returning an API error instead of a clean exit-64).

**Status**: MUST-PASS. Guards BC-3.8.002's non-JSM project fail-fast behavior.

---

### H-NEW-JSM-RT-003: `issue create --request-type` OAuth scope-mismatch 401 surfaces `write:servicedesk-request` recovery hint (MUST-PASS)

**NFR source**: BC-3.8.015, BC-X.3.005, BC-1.6.042
**BC**: BC-3.8.015, BC-X.3.005, BC-1.6.042, BC-1.3.023
**Note**: BC-X.8.006 and BC-X.8.007 are intentionally NOT in the BC list. BC-3.8.014 is also intentionally NOT in the BC list — this holdout exercises the OAuth InsufficientScope arm only (scope-mismatch body routes through client.rs:696-704 short-circuit); BC-3.8.014's positive path (Basic-auth 401 → API-token-expiry hint) is pinned by `test_jsm_create_basic_auth_401_surfaces_api_token_hint` and `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (repurposed in place by F4), so BC-3.8.014 is intentionally absent from the `BC:` list above.
**Authored by**: F2 spec evolution (2026-05-18)
**Test file**: `tests/issue_create_jsm.rs` — realized AS `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`. The holdout and this test are the SAME artifact; there is no separate file. This test is already GREEN on `develop` unmodified and MUST remain unmodified.

> **[REVISED 2026-05-19 issue #384 adversary-pass-9 C-01]** Re-bound from the pre-#384 Basic-auth 401 test (Basic + generic-expiry — renamed by F4 to `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`, a BC-3.8.014 pin asserting API-token-expiry hint with `write:servicedesk-request` ABSENT) to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (Bearer + scope-mismatch body — the ONLY deterministic OAuth→`JrError`→`write:servicedesk-request` path via the `JR_AUTH_HEADER` seam). All prior revision-note blockquotes referencing the earlier binding are superseded by this note. Title updated to reflect scope-mismatch framing.

**Setup** (faithfully describes the bound test `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`):
0. **Cache dir is empty** (isolated `tempfile::tempdir()` for `XDG_CACHE_HOME`) — all GET mocks are reached on a cold cache.
1. Wiremock at `JR_BASE_URL`. Auth: `JR_AUTH_HEADER=Bearer test-oauth-token` (OAuth/Bearer fixture).
2. Project-meta GET for `HELP` returns a service-desk-type project (via the `mount_project_meta_help` helper — project `HELP`, id `99`, service-desk type). The helper is authoritative for the exact mock body.
3. Service-desk list GET returns service desk matched to project `HELP` (via `mount_service_desk_list` helper). The `projectId` field must match the project `id` from step 2 for `require_service_desk` to succeed.
4. Request-type list GET for the service desk returns a **single-element list** via the `mount_request_types_password_reset` helper: `"Password Reset"` only (one entry, no ambiguity in partial_match resolution). NOTE: this helper is distinct from `mount_request_type_list` (two-element list used by the sibling `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` — repurposed and renamed by F4); do NOT consolidate the two helpers.
5. `POST /rest/servicedeskapi/request` returns HTTP 401 with a **scope-mismatch body**: `{"errorMessages": ["Unauthorized; scope does not match"]}`. This body triggers the short-circuit at `src/api/client.rs:696-704` BEFORE the Bearer guard and BEFORE the refresh coordinator, landing as `JrError::InsufficientScope` in `handle_jsm_create`'s `map_err`. The OAuth arm (`is_oauth_auth() == true`) preserves `InsufficientScope` and surfaces the `write:servicedesk-request` hint. **WHY scope-mismatch body is required:** a generic-expiry body on a Bearer client routes through the refresh coordinator (client.rs:727+), which deterministically fails with a raw anyhow error (not a `JrError`) via the `JR_AUTH_HEADER` seam — the `write:servicedesk-request` hint is never injected and the test would not be a valid pin.

**Action**: `jr issue create --project HELP --request-type "Password Reset" --summary "Reset my password" --no-input`

**Expected (MUST-PASS)** — exactly the four assertions made by `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (read from `tests/issue_create_jsm.rs` lines 1566-1582):
- exit non-zero
- stderr contains `write:servicedesk-request`
- stderr contains `jr auth refresh`
- stderr contains `jr auth login`

**Note**: The negative boundary "OAuth path does NOT leak the Basic-auth API-token hint" is NOT pinned by this holdout — it is covered positively by BC-3.8.014's dedicated Basic-auth tests (`test_jsm_create_basic_auth_401_surfaces_api_token_hint` and `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (repurposed and renamed by F4)), which assert the Basic path produces the API-token-expiry hint, making the negative boundary implicit and structurally enforced.

**Why hidden**: The OAuth `InsufficientScope` 401 path (scope-mismatch body → client.rs:696-704 short-circuit → `InsufficientScope` → `map_err` OAuth arm) must surface `write:servicedesk-request`. This is the only deterministic Bearer→`JrError`→hint path via the `JR_AUTH_HEADER` seam. A regression where the OAuth `InsufficientScope` arm loses the `write:servicedesk-request` hint would be invisible without this pin.

**Status**: MUST-PASS. Verifies that BC-3.8.015's `write:servicedesk-request` addition is surfaced in the user-facing error recovery path for OAuth auth via the `InsufficientScope` arm (the only deterministic testable path).

---

### H-NEW-JSM-RT-004: `--type` flag ignored with stderr warning when `--request-type` is set (MUST-PASS)

**NFR source**: BC-3.8.010
**BC**: BC-3.8.010, BC-3.8.001
**Authored by**: F1d adversary pass-01 (2026-05-18)

**Setup**:
1. Wiremock at `JR_BASE_URL`. Config: project `HELPDESK` with `typeKey = "service_desk"`.
2. Mock `GET /rest/servicedeskapi/servicedesk` returning `{values: [{id: "3", projectKey: "HELPDESK"}]}`.
3. Mock `GET /rest/servicedeskapi/servicedesk/3/requesttype` returning `{isLastPage: true, values: [{id: "5", name: "Get IT Help", description: "IT support"}]}`.
4. Mock `POST /rest/servicedeskapi/request` with `expect(1)` returning 201 `{issueId: "10042", issueKey: "HELP-42", currentStatus: {status: "Waiting for support"}}`.

**Action**: `jr issue create --project HELPDESK --request-type "Get IT Help" --type Bug --summary "foo" --no-input --output json`

**Expected (MUST-PASS)**:
- exit 0
- stdout JSON: `{"key": "HELP-42"}`
- stderr contains: `warning: --type is ignored when --request-type is set; request type encodes the issue type`
- `POST /rest/servicedeskapi/request` called exactly once (expect(1) satisfied)
- `--type Bug` value does NOT appear in the POST body (request type field uses resolved requestTypeId "5", not platform issue-type label)

**Why hidden**: The `--type` flag interaction at the JSM dispatch site is not visible from the JSON output alone — only the stderr line and mock body inspection reveal whether `--type` was silently dropped or incorrectly forwarded. A regression where `--type` causes an error (rather than a warning) or where the warning is omitted would be invisible without this pin.

**Status**: MUST-PASS. Pins BC-3.8.010 (--type ignored with warning).

---

### H-NEW-JSM-RT-005: `jr requesttype fields` uses cache on second call — no extra HTTP (SHOULD-PASS)

**NFR source**: BC-X.12.005
**BC**: BC-X.12.005, BC-X.12.008
**Authored by**: F1d adversary pass-02 (2026-05-18)

**Setup**:
1. Wiremock at `JR_BASE_URL`. Config: project `HELPDESK` with `typeKey = "service_desk"`.
2. Mock `GET /rest/servicedeskapi/servicedesk` returning `{values: [{id: "3", projectKey: "HELPDESK"}]}` with `expect(1..=2)` (service desk resolution happens on each `requesttype fields` call; caching behavior may reduce to 1).
3. Mock `GET /rest/servicedeskapi/servicedesk/3/requesttype` returning `{isLastPage: true, values: [{id: "5", name: "Get IT Help", description: "IT support"}]}` with `expect(1..=2)` (request type list for name resolution is cached; cache-warm second call should not hit this, but expect range accommodates both cache-miss and cache-hit paths).
4. Mock `GET /rest/servicedeskapi/servicedesk/3/requesttype/5/field` with `expect(1)` returning a minimal field response `{canRaiseOnBehalfOf: false, canAddRequestParticipants: false, requestTypeFields: [{fieldId: "summary", name: "Summary", required: true, jiraSchema: {type: "string"}}]}`.

**Action (two sequential calls)**:
1. `jr requesttype fields "Get IT Help" --project HELPDESK --no-input`
2. `jr requesttype fields "Get IT Help" --project HELPDESK --no-input`

**Expected (SHOULD-PASS)**:
- Both calls exit 0
- `GET /rest/servicedeskapi/servicedesk/3/requesttype/5/field` is called exactly once across both runs (expect(1) satisfied) — second call uses the per-request-type fields cache
- Both calls produce identical stdout output (table with "Summary", required=YES)

**Why hidden**: The per-request-type fields cache (`request_type_fields_<sid>_<rtId>.json`) is a separate cache layer from the request-type list cache. A regression where the fields cache is not populated or not read on the second call would result in two HTTP calls — visible only via wiremock `expect(1)` assertion failure.

**Status**: SHOULD-PASS. Pins BC-X.12.005 §Caching (fields cache hit/miss behavior).

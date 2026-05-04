---
context: holdout-scenarios
title: "Holdout Scenarios"
total_holdouts: 48
last_updated: 2026-05-04
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/
  - Source broad P3: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §4 (H-001..H-020)
  - Source R1: .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md §4 (H-021..H-029)
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.9 (H-030..H-047)
  - Source BC-NFR-R-D: .factory/semport/jira-cli/jira-cli-bc-nfr-r-d-draft.md (H-NEW-MP-001)
---

# Holdout Scenarios — jira-cli

48 holdout scenarios for Phase 4 evaluation. Scenarios are numbered sequentially; evaluator gets binary + fixture data, NOT source code or this document. Expected outputs are precise.

Setup uses:
- `XDG_CONFIG_HOME` / `XDG_CACHE_HOME` pointing to temp directories
- `JR_BASE_URL` pointing to a local wiremock/mock server (Rust `wiremock` crate pattern)
- `JR_SERVICE_NAME=jr-jira-cli-test` to isolate keychain (where applicable)
- `assert_cmd` (process-spawn) or `JiaClient::new_for_test` (library-level) for invocation

---

## Group 1: Auth & Profile Edge Cases (H-001..H-008, H-016, H-019, H-021..H-029)

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
**BC refs**: BC-1.6.046

---

### H-005: Malformed config TOML errors with exit 78 and does NOT overwrite the file
**Setup**: write malformed TOML (`[unclosed\nbad = \n`) at `XDG_CONFIG_HOME/jr/config.toml`. Capture file bytes.
**Action**: `jr auth login --oauth --client-id X --client-secret Y --no-input`
**Expected**: exit 78; stderr contains `toml` or `parse`; file bytes are unchanged.
**Why hidden**: Pre-fix bug silently overwrote with defaults — destroyed user settings.
**BC refs**: BC-6.1.002
**Source**: `tests/auth_login_config_errors.rs:18-97`

---

### H-006: `issue move FOO-1 "In Progress"` is idempotent when already in target
**Setup**: wiremock returns `GET /issue/FOO-1` with `status.name = "In Progress"`. Mock POST transitions with `expect(0)`.
**Action**: `jr issue move FOO-1 "In Progress" --output json`
**Expected**: exit 0; stdout JSON has `"transitioned": false`. POST mock not invoked.
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
**BC refs**: BC-1.6.044, BC-X.1.007
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
**Source**: `tests/h017_aql_clause`

---

### H-018: `parse_duration` vs `validate_duration` — different acceptance for combined units
**Setup**: none.
**Action**: invoke `duration::parse_duration("1w2d3h30m", 8, 5)` and `jql::validate_duration("4w2d")`.
**Expected**: parse_duration → Ok(seconds = 1×5×8×3600 + 2×8×3600 + 3×3600 + 30×60). validate_duration → Err.
**Why hidden**: Two parsers with overlapping syntax but DIFFERENT acceptance — easy to confuse.
**BC refs**: BC-X.9.002, BC-X.9.003

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
**BC refs**: BC-1.6.043, BC-1.6.044

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

### H-027: `Retry-After: 86400` (24h) honored without upper bound (KNOWN-GAP pin)
**Setup**: Wiremock 429 with `Retry-After: 86400` (literal). Expect call count = 2.
**Action**: any API call (library level, NOT process-spawn — avoid actual 24h sleep).
**Expected**: The parsed delay value is 86400 seconds (no upper bound applied). Evaluator waits 5s; if no second call is fired within that window, mark as `"honored unbounded" PASS` — the very absence of a retry proves the large delay is being honored. stderr MUST contain `"warning: rate limited by Jira"`.
**Why hidden**: Pin Pass 4 §7.1.3 NFR gap as an explicit holdout against silent fixes that add an upper bound.
**BC refs**: BC-X.4.002

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

## Group 2: Issue Read, JQL, and Filtering (H-030..H-035)

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
**BC refs**: BC-3.7.003

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
**BC refs**: BC-7.2.001..BC-7.2.054

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
**Action**: `jr issue open FOO-1 --url-only` (print without opening browser)
**Expected (FIXED behavior)**: stdout contains `https://mycompany.atlassian.net/browse/FOO-1`. Does NOT contain `api.atlassian.com`.
**Status**: MUST-FIX (NFR-R-B). Current code fails this holdout for OAuth profiles.
**BC refs**: BC-3.4.001

---

### H-047: `accessible_resources` first-result-wins for multi-site OAuth — pin as KNOWN GAP
**Setup**: OAuth mock returns two cloud resources: `[{id: "cloud-A", name: "Company A"}, {id: "cloud-B", name: "Company B"}]`.
**Action**: `jr auth login --oauth --client-id X --client-secret Y --no-input`
**Expected**: Authenticated to `cloud-A` (first result wins). No disambiguation or error.
**Purpose**: Pin the known-gap behavior; when NFR-O-S is fixed (add `--cloud-id` flag), this holdout becomes a MUST-FAIL that drives the fix.
**BC refs**: BC-1.5.038

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

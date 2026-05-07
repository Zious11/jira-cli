---
context: edge-case-catalog
title: "Edge Case Catalog"
last_updated: 2026-05-04
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §3 (cross-ref), §5
  - Source R1: .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md §3.3 (assets), §3.4 (auth)
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.7 (team parity)
  - Source P8: .factory/semport/jira-cli/jira-cli-pass-8-deep-synthesis.md §8
---

# Edge Case Catalog — jira-cli

Cross-cutting boundary conditions and untested behavior gaps cataloged from Pass 3.

Categories:
- **EC-AUTH**: Authentication & profile boundary conditions
- **EC-CFG**: Configuration & cache edge cases
- **EC-HTTP**: HTTP transport edge cases
- **EC-JQL**: JQL composition edge cases
- **EC-ASSET**: Assets/CMDB edge cases
- **EC-SPRINT**: Boards & sprints edge cases
- **EC-OUT**: Output rendering edge cases
- **EC-GAP**: Untested invariant gaps (from Pass 3 §3.5 and Pass 5 synthesis)

---

## EC-AUTH: Authentication Edge Cases

### EC-AUTH-001: Profile flag > env > config > "default" full override chain
**Boundary**: All three override sources set simultaneously.
**Expected**: `--profile from-flag` wins over `JR_PROFILE=from-env` wins over `default_profile = "from-config"`.
**Status**: Covered by BC-1.1.007; holdout H-003.
**Test gap**: Integration test must set all 3 simultaneously and assert `"active": true` on `from-flag`.

### EC-AUTH-002: `auth remove` active profile guard
**Boundary**: User attempts to remove the currently active profile.
**Expected**: Exit 64; error `"cannot remove active profile"`; config file unchanged.
**Status**: Covered by BC-1.1.006; holdout H-016.

### EC-AUTH-003: Legacy `[instance]` config auto-migration idempotency
**Boundary**: Legacy config loaded twice (or after a previous migration).
**Expected**: Second load produces byte-identical on-disk file. No double-migration side effects.
**Status**: Covered by BC-6.1.002; holdout H-011.

### EC-AUTH-004: Malformed TOML does NOT overwrite config
**Boundary**: Config file contains malformed TOML; any `jr` command invoked.
**Expected**: Exit 78; stderr contains parse error; config file bytes unchanged.
**Status**: Covered by BC-1.1.012; holdout H-005.

### EC-AUTH-005: Non-default profile never inherits legacy keychain keys
**Boundary**: Profile name is NOT `"default"`; legacy flat keychain keys exist.
**Expected**: Non-default profile does not read flat keys; only reads `<profile>:oauth-*` namespaced keys.
**Status**: Covered by BC-1.4.027 (PARTIALLY-TESTED — asserted by absence, INV-12).
**Test gap**: Positive-side keyring integration test.

### EC-AUTH-006: `JR_AUTH_HEADER` without `JR_BASE_URL` — bypass risk
**Boundary**: `JR_AUTH_HEADER` set but `JR_BASE_URL` not set.
**Expected**: After NFR-S-B fix — env var ignored; keychain used instead.
**Status**: MUST-FIX (NFR-S-B, SECURITY-DECIDE). Not currently guarded.
**Test gap**: Unit test asserting header ignored when base URL not set.

### EC-AUTH-007: Profile name with colon — security boundary
**Boundary**: Profile name `foo:bar` attempted via (a) `--profile foo:bar`, (b) TOML key `[profiles."foo:bar"]`, (c) `JR_PROFILE=foo:bar`.
**Expected**: Each → exit 64. Colon separator in keychain keys must not be leaked into profile names.
**Status**: Covered by BC-6.1.004; holdout H-019.

### EC-AUTH-008: OAuth callback loopback IPv4 vs IPv6
**Boundary**: System resolves `localhost` to `::1` (IPv6) on macOS; callback listener binds `127.0.0.1`.
**Expected**: Callback URL is literal `http://127.0.0.1:53682/callback` — forces IPv4 to match listener.
**Status**: Covered by BC-1.5.031 (ADR-0006 invariant). Not tested via actual HTTP — assertion is code-level.

### EC-AUTH-009: `InsufficientScope` substring match precision
**Boundary**: 401 body contains "scope does not match" (exact substring).
**Expected**: `InsufficientScope` variant raised (not generic `NotAuthenticated`).
**Status**: Covered by BC-1.6.044; holdout H-012.
**Test gap**: Any future tightening of the substring match would silently break this.

---

## EC-CFG: Configuration & Cache Edge Cases

### EC-CFG-001: Cache deserialization failure (corrupt file) — non-fatal
**Boundary**: Cache file contains truncated or invalid JSON (e.g., `{"teams": [`).
**Expected**: Cache miss (not error); application re-fetches from API; warns user with cache hint.
**Status**: Covered by BC-6.2.002; holdout H-009.

### EC-CFG-002: Cache TTL expiry — transparent re-fetch
**Boundary**: Cache file mtime is >7 days old.
**Expected**: Treated as miss; API re-fetched; file overwritten with fresh data.
**Status**: Covered by BC-6.2.003 (TTL check: `(Utc::now() - fetched_at).num_days() >= 7`).

### EC-CFG-003: `clear_profile_cache` on nonexistent directory — no-op
**Boundary**: Profile with no cache directory attempts cache clear (e.g., during `auth remove`).
**Expected**: No error; no-op.
**Status**: UNTESTED-INTEGRATION (INV-10). Unit test exists but no integration test asserts this during `auth remove` flow.

### EC-CFG-004: Cross-profile cache isolation
**Boundary**: Two profiles (`prod`, `sandbox`) with different `story_points_field_id` values.
**Expected**: Each profile's cache reads/writes use `~/.cache/jr/v1/<profile>/` independently.
**Status**: Soft fence (100% conformance by convention); no compile-time enforcement (NFR-SCA-2).

### EC-CFG-005: Multi-profile fields bug — MUST-FIX
**Boundary**: Profile `sandbox` has `story_points_field_id = "customfield_10099"`. Command run with `--profile sandbox`.
**Expected (FIXED behavior)**: Request body uses `customfield_10099`. Error message references `[profiles.sandbox]` not deprecated `[fields]`.
**Status**: MUST-FIX (NFR-R-D) → BC-6.3.001; holdout H-NEW-MP-001. Current behavior silently uses `config.global.fields.*`.

---

## EC-HTTP: HTTP Transport Edge Cases

### EC-HTTP-001: 429 retry — MAX_RETRIES=3 and then passthrough for send_raw
**Boundary**: Server returns 429 for 4 consecutive calls.
**Expected**: `send` raises error after 3 retries. `send_raw` returns 429 response to caller (NOT error).
**Status**: Covered by BC-X.1.005; holdout H-013.

### EC-HTTP-002: Retry-After integer-only parsing
**Boundary**: Server sends `Retry-After: Mon, 04 May 2026 00:00:00 GMT` (HTTP-date format).
**Expected**: Falls through to `DEFAULT_RETRY_SECS = 1`. NOT a parse error.
**Status**: Documented gap (NFR-SCA-1); Atlassian sends integers in practice.

### EC-HTTP-003: Network drop mid-stream
**Boundary**: TCP connection drops during response body read.
**Expected**: `NetworkError` (exit 1) + `"Could not reach <host>"`.
**Status**: Covered by asset error tests (BC-4.4.003). Similar tests for other subsystems.

### EC-HTTP-004: Auth header injected on every retry attempt
**Boundary**: Request retried after 429 backoff.
**Expected**: Auth header present on retry (injected at `client.rs:195` per retry loop).
**Status**: Covered by BC-X.1.005.

### EC-HTTP-005: Ctrl+C during API call — graceful exit 130
**Boundary**: User presses Ctrl+C while HTTP call in flight.
**Expected**: `tokio::select!` in main.rs wins; exit 130; no panic.
**Status**: Covered by BC-X.1.009. MEDIUM confidence (CLAUDE.md: medium-frequency path).

---

## EC-JQL: JQL Composition Edge Cases

### EC-JQL-001: JQL special characters escaped in `escape_value`
**Boundary**: Issue summary contains `"` or `\` characters.
**Expected**: `escape_value` produces `\"` and `\\` respectively. JQL injection structurally prevented.
**Status**: Covered by BC-X.9.001 proptest.

### EC-JQL-002: `validate_duration` rejects combined units
**Boundary**: `jr worklog add PROJ-1 4w2d` (combined units in JQL duration validation).
**Expected**: `validate_duration("4w2d")` → Err.
**Status**: Covered by BC-X.9.002; holdout H-018.

### EC-JQL-003: `parse_duration` accepts combined units
**Boundary**: `jr worklog add PROJ-1 1w2d3h30m` (combined units in worklog context).
**Expected**: `parse_duration("1w2d3h30m", 8, 5)` → Ok(seconds).
**Status**: Covered by BC-X.9.002; holdout H-018.

### EC-JQL-004: Date validator runs before HTTP
**Boundary**: `jr issue list --from 2026-13-01` (invalid date).
**Expected**: Exit 64 immediately; no HTTP call fired.
**Status**: PARTIALLY-TESTED (INV-24 gap). Clap-level rejection tested but no test asserts "no HTTP fired" via mock count.

### EC-JQL-005: `build_asset_clause` uses field NAME not ID
**Boundary**: CMDB field `("customfield_10191", "Client")` passed to JQL builder.
**Expected**: AQL clause uses `"Client" IN aqlFunction("Key = \"CUST-5\"")` — NOT `"customfield_10191"`.
**Status**: Covered by BC-4.1.002; holdout H-017.

### EC-JQL-006: AQL attribute is capital `Key` not `objectKey`
**Boundary**: Object key `CUST-5` referenced in AQL.
**Expected**: `Key = "CUST-5"` in AQL — NOT `objectKey = "CUST-5"`.
**Status**: CLAUDE.md gotcha; covered by BC-4.1.002; holdout H-017.

---

## EC-ASSET: Assets/CMDB Edge Cases

### EC-ASSET-001: `AssetsPage::is_last` accepts bool and string-encoded bool
**Boundary**: API returns `"isLast": "true"` (string) instead of `"isLast": true` (bool).
**Expected**: Custom deserializer handles both; pagination terminates correctly.
**Status**: Covered by BC-4.2.003.

### EC-ASSET-002: `extract_linked_assets` with null custom field
**Boundary**: Issue with `"customfield_10191": null`.
**Expected**: Returns empty `Vec<LinkedAsset>` (no error, no panic).
**Status**: Covered by BC-4.1.007.

### EC-ASSET-003: `enrich_assets` — id-only asset only
**Boundary**: `LinkedAsset` has `id` set but `key` and `name` are None.
**Expected**: GET fired to resolve; after enrichment: `key`, `name`, `asset_type` populated.
**Status**: Covered by BC-4.3.002.

### EC-ASSET-004: `enrich_assets` — already-resolved asset skipped
**Boundary**: `LinkedAsset` has both `key` and `name` populated.
**Expected**: No GET fired (skip).
**Status**: Covered by BC-4.3.002.

### EC-ASSET-005: `LinkedAsset::display()` id-fallback hint
**Boundary**: Asset has only `id` (no key, no name).
**Expected**: Display shows `#<id> (run 'jr init' to resolve asset names)`.
**Status**: Covered by BC-4.3.003.

### EC-ASSET-006: Multi-workspace `resolved` HashMap collision — MUST-FIX
**Boundary**: Two workspaces share `oid = "OBJ-88"` for different objects.
**Expected (FIXED behavior)**: `resolved` keyed by `(workspace_id, oid)` — no collision.
**Status**: MUST-FIX (NFR-R-E) → BC-4.3.001; holdout H-036. Current code uses `oid`-only key (last-write-wins).

### EC-ASSET-007: Workspace ID — cache hit vs miss
**Boundary**: Workspace cache exists with TTL <7d vs >7d.
**Expected**: Cache hit → no HTTP; cache miss → GET `rest/servicedeskapi/assets/workspace` → re-cache.
**Status**: Covered by BC-4.2.001.

---

## EC-SPRINT: Boards & Sprints Edge Cases

### EC-SPRINT-001: Kanban board sprint commands — hard error
**Boundary**: `jr sprint current` on a kanban board.
**Expected**: Exit non-zero; literal message `"Sprint commands are only available for scrum boards"`.
**Status**: Covered by BC-5.2.001.

### EC-SPRINT-002: `issue list` on kanban board — silent degrade
**Boundary**: `jr issue list` when no scrum board found for project.
**Expected**: Silent degrade — no error, just no sprint filter applied. (Asymmetry with sprint commands — DOCUMENTED.)
**Status**: Covered by BC-2.1.004 (kanban board uses project JQL, no error) and BC-2.2.027 (no active sprint fallback).

### EC-SPRINT-003: Sprint add/remove cap — MAX_SPRINT_ISSUES=50
**Boundary**: `jr sprint add --sprint 1 <51 issue keys>`.
**Expected**: At most 50 processed; remainder silently ignored.
**Status**: UNTESTED-INTEGRATION (INV-22). Unit tests exist; no integration test passes 51+ keys.

### EC-SPRINT-004: `sprint current` under-limit (10 issues)
**Boundary**: Sprint has 10 issues; default limit = 30.
**Expected**: All 10 returned; NO truncation hint on stderr.
**Status**: Covered by BC-5.2.005.

### EC-SPRINT-005: Sprint JSON `sprint_id` asymmetry
**Boundary**: `sprint add` vs `sprint remove` JSON output.
**Expected**: Add response includes `sprint_id`; remove response does NOT include `sprint_id`.
**Status**: Covered by BC-5.2.007 and BC-5.2.008 (insta snapshots).

---

## EC-OUT: Output Rendering Edge Cases

### EC-OUT-001: Team column — conjunctive gate
**Boundary**: `team_field_id` configured but no issue has team UUID.
**Expected**: Team column omitted (both conditions must be true: configured AND populated).
**Status**: Covered by BC-5.3.001 and BC-5.3.002.

### EC-OUT-002: Stale team cache — UUID fallback with hint
**Boundary**: Issue has team UUID but team name not in cache.
**Expected**: Shows `"UUID (name not cached — run 'jr team list --refresh')"`.
**Status**: Covered by BC-5.3.003.

### EC-OUT-003: Team column with `--output json` — raw UUID
**Boundary**: `jr sprint current --output json` with team UUID present.
**Expected**: JSON includes raw team UUID; no cache lookup performed.
**Status**: Covered by BC-5.3.004.

### EC-OUT-004: ADF empty-body edge case
**Boundary**: ADF node with empty content array.
**Expected**: No panic; returns empty string or minimal text.
**Status**: Covered by BC-7.2.* edge cases.

### EC-OUT-005: `extract_error_message` empty body (FIRST priority)
**Boundary**: API returns 4xx with empty response body.
**Expected**: Returns literal string `<empty response body>` (per BC-7.3.005); NOT attempts to parse `{}`.
**Status**: Covered by BC-7.3.005 (corrected from broad pass per CONV-ABS-004; further corrected per ADV-P5-002).

### EC-OUT-006: `--no-input` auto-set for non-TTY
**Boundary**: Command invoked from CI / piped context where stdin is not TTY.
**Expected**: `--no-input` behavior auto-enabled; no prompts fired.
**Status**: UNTESTED-DIRECT (INV-25). Hard to test from `assert_cmd` (always non-TTY).

### EC-OUT-007: `--output json` error format — structured to stderr
**Boundary**: Any command errors with `--output json` active.
**Expected**: stderr is parseable JSON `{"error": "<msg>", "code": <int>}`; stdout empty or absent.
**Status**: Covered by BC-7.3.005; holdout H-020. (BC-7.4.012 is user view hidden email — unrelated.)

---

## EC-GAP: Untested Invariant Gaps

These are invariants from Pass 3 §3.5 (INV-10..25) that have no integration test coverage. All are test-writing targets for Phase 3.

| Invariant | Description | Gap Type | Holdout |
|---|---|---|---|
| INV-10 | `clear_profile_cache(name)` no-op for nonexistent dir | UNTESTED-INTEGRATION | — |
| INV-11 | Per-profile keychain key namespacing | PARTIALLY-TESTED (most `#[ignore]`-gated) | — |
| INV-12 | Non-default profile never inherits legacy keys | PARTIALLY-TESTED (asserted by absence) | — |
| INV-21 | `--open` JQL fragment `statusCategory != Done` | INDIRECT (no wiremock body match) | — |
| INV-22 | `MAX_SPRINT_ISSUES=50` cap | UNTESTED-INTEGRATION | — |
| INV-24 | Date validators run before HTTP | PARTIALLY (no mock count assertion) | — |
| INV-25 | `--no-input` TTY autoset | UNTESTED-DIRECT | — |

### Additional gaps from Pass 8 synthesis

| Gap | Description | Priority |
|---|---|---|
| G-A1 | `refresh_oauth_token` has zero production callers; exists for future 401 auto-refresh | P1 — wire into `send` |
| G-B1 | 5 auth subcommands lack `--output json` paths | P2 — add JSON shapes |
| G-C1 | `cli/issue/list.rs` asset enrichment `join_all` unbounded concurrency | P2 — `buffer_unordered(8)` |
| G-D1 | Profile name 64-char error message doesn't distinguish length from charset violation | LOW — 2 LOC fix |
| G-E1 | `search_issues` cursor loop has no anti-loop guard (unlike `get_changelog`) | LOW — document/optional add |
| G-EO1 | `observability.rs` is 39 LOC with one function at 2 sites; no tracing crate integration; tracing crate not present in Cargo.toml | MEDIUM — Phase 3 |
| G-EO2 | CLAUDE.md missing `cli/issue/view.rs`, `cli/issue/comments.rs`, `observability.rs`, `api/assets/schemas.rs` | MEDIUM — Phase 3 doc |
| G-EO3 | User pagination fixed-advance by `USER_PAGE_SIZE` not returned-count (JRACLOUD-71293 workaround) — undocumented | LOW — add source comment |

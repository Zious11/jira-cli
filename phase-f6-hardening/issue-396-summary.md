---
phase: F6
mode: feature
issue: 396
fix_followup: FIX-F5-001
target_commit: 699a5fd4298a0fc831fb376d2e55bd5ed1ca1767
baseline_commit: b49f2fd
target_branch: develop
date: 2026-05-25
verdict: PASS
verifier: formal-verifier
---

# Phase F6 — Targeted Hardening Summary: issue #396 + FIX-F5-001

## Scope

Delta `b49f2fd..699a5fd` (3,731 insertions, 15 files):

- Source (5): `src/cli/issue/create.rs`, `src/cli/issue/helpers.rs`,
  `src/cli/issue/field_resolve.rs` (NEW), `src/cli/issue/mod.rs`,
  `src/cli/mod.rs`
- Types (2): `src/types/jira/editmeta.rs` (NEW), `src/types/jira/mod.rs`
- API (1): `src/api/jira/issues.rs` (added `get_editmeta`)
- Cache (1): `src/cache.rs` (added `FieldsCache` + best-effort writer)
- Tests (1): `tests/issue_edit_field.rs` (NEW, 45 test functions)
- Tooling (1): `scripts/check-bc-cumulative-counts.sh`
- Manifest/changelog: `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `CLAUDE.md`

## Hardening Tool Results

### 1. Mutation testing (cargo-mutants v27.0.0, `--in-diff`)

| Metric          | Value                                          |
|-----------------|------------------------------------------------|
| Command         | `cargo mutants --in-diff <diff> --jobs 4 --baseline=skip --cargo-test-arg=--skip --cargo-test-arg=global_profile_flag_targets_auth_status` |
| Exit code       | `0`                                            |
| Mutants found   | 15 (all in `src/cli/issue/create.rs` per `.cargo/mutants.toml` examine_globs) |
| Caught          | **15 / 15 = 100%**                             |
| Missed          | 0                                              |
| Timeout         | 0                                              |
| Unviable        | 0                                              |
| Wall time       | 5m                                             |

**Caught mutants:**
```
src/cli/issue/create.rs:304:5  replace handle_edit -> Result<()> with Ok(())
src/cli/issue/create.rs:385:13 replace || with && in handle_edit
src/cli/issue/create.rs:386:13 replace || with && in handle_edit
src/cli/issue/create.rs:387:13 replace || with && in handle_edit
src/cli/issue/create.rs:387:16 delete ! in handle_edit
src/cli/issue/create.rs:405:8  delete ! in handle_edit
src/cli/issue/create.rs:408:30 replace && with || in handle_edit
src/cli/issue/create.rs:414:35 replace || with && in handle_edit
src/cli/issue/create.rs:414:57 replace && with || in handle_edit
src/cli/issue/create.rs:423:33 replace && with || in handle_edit
src/cli/issue/create.rs:429:31 replace && with || in handle_edit
src/cli/issue/create.rs:480:12 delete ! in handle_edit
src/cli/issue/create.rs:587:12 delete ! in handle_edit
src/cli/issue/create.rs:618:12 delete ! in handle_edit
src/cli/issue/create.rs:962:8  delete ! in handle_edit
```

**Verdict:** Exceeds 90% threshold and 95% security-critical threshold.
The Gate A (multi-key + `--field`), Gate B (flag-overlap), `--label`-conflict,
and live-path `--field` blocks are all rigorously test-pinned. Every conditional
mutation in `handle_edit` was caught.

**Scope caveat (operating within `.cargo/mutants.toml` policy):**
`field_resolve.rs`, `editmeta.rs`, `cache.rs::FieldsCache`, and `issues.rs::get_editmeta`
are NOT in the project's `examine_globs` list (which limits mutation to 6 named
files in `src/api/jira/bulk.rs`, `src/types/jira/bulk.rs`, `src/cli/issue/create.rs`,
`src/api/jsm/requests.rs`, `src/api/jsm/request_types.rs`, `src/cli/requesttype.rs`).
The `--in-diff` filter intersects with `examine_globs`, so only the create.rs delta
was mutation-tested. End-to-end coverage of the new files is provided by the
45 `tests/issue_edit_field.rs` integration tests plus the inline `cache::tests`
unit test (`test_write_fields_cache_swallow_io_error_returns_ok`).

### 2. Security — `cargo audit`

| Metric          | Value                                          |
|-----------------|------------------------------------------------|
| Command         | `cargo audit`                                  |
| Exit code       | `0`                                            |
| Advisories DB   | 1,098 entries loaded                           |
| Crates scanned  | 341                                            |
| Findings        | **0 vulnerabilities, 0 warnings**              |

New direct dev-dependencies introduced by this PR:
- `temp-env 0.3` — env-var scoping for cache-isolation tests
- `scopeguard 1` — `defer!` for cleanup-on-panic in tests

Both are mature, widely-used crates; no advisories.

### 3. Security — `cargo deny check`

| Metric          | Value                                          |
|-----------------|------------------------------------------------|
| Command         | `cargo deny check`                             |
| Exit code       | `0`                                            |
| Advisories      | ok                                             |
| Bans            | ok                                             |
| Licenses        | ok                                             |
| Sources         | ok                                             |
| Warnings        | 2 (unused license allowances for `OpenSSL` and `Unicode-DFS-2016` in `deny.toml`; pre-existing, not introduced by this PR) |

### 4. Unsafe deserialization audit (editmeta JSON parser)

`src/types/jira/editmeta.rs` defines 4 structs with `Deserialize`:
- `EditMeta { fields: HashMap<String, EditMetaField> }`
- `EditMetaField { name, schema, allowed_values, operations, required }`
  - `name: String` (required field — serde fails if missing; correct)
  - `schema: EditMetaFieldSchema` (required; correct)
  - `allowed_values: Option<Vec<AllowedValue>>` with `#[serde(rename = "allowedValues")]`
    (correctly optional; resolution code handles `None` via `.as_deref().unwrap_or(&[])`)
  - `operations: Vec<String>` (required; correct — empty `[]` is a valid payload that fails the "set" check via `.iter().any(|op| op == "set")`)
  - `required: bool` (required; correct — Jira always sends this)
- `EditMetaFieldSchema { field_type, system, custom }`
  - `field_type` renamed from JSON `"type"` (Rust keyword)
  - `system`, `custom` both `Option<String>` (correctly optional)
- `AllowedValue { id: String, value: Option<String>, name: Option<String> }`
  - `id` required (used on wire), `value`/`name` optional (correctly handled with `.as_deref()`)

**Consumer audit** (`src/cli/issue/field_resolve.rs`):
- No `unwrap()` on parse results from the editmeta JSON.
- `allowed_values.as_deref().unwrap_or(&[])` (line 308) — safe (returns empty slice).
- All `Option::value` access guarded with `.as_deref().unwrap_or("?")` for error messages or `.clone().unwrap_or_else(|| ...fallback)` for echo strings.
- One `unwrap()` at line 215 (`field_list.as_ref().unwrap()`) is unconditionally safe — preceded by `field_list = Some(fresh)` on line 213.
- One `unsafe` block exists in `cache.rs::test_write_fields_cache_swallow_io_error_returns_ok` for `std::env::set_var` — standard Rust 2024 idiom, gated by `ENV_MUTEX` to prevent races with other env-touching tests. NOT used in production code.

**No panic patterns** (`panic!`, `expect` outside of test-only code, `unwrap` on `Result`) in the production code path.

### 5. Full regression suite

| Metric          | Value                                          |
|-----------------|------------------------------------------------|
| Command         | `cargo test --no-fail-fast -- --skip <12 keychain-blocked tests>` |
| Exit code       | `0`                                            |
| Test binaries   | 72                                             |
| Passed          | **1,459**                                      |
| Failed          | **0**                                          |
| Ignored (by design) | 18                                         |
| Filtered (skipped via `--skip`) | 12                             |

**Skipped tests (pre-existing environment hazards, NOT introduced by issue #396):**

These tests block on macOS keychain prompts when run in a non-interactive cargo
test session. They are pre-existing (oldest dates from PR #275, May 2026):

| Test                                                              | File                                | Origin     |
|-------------------------------------------------------------------|-------------------------------------|------------|
| `global_profile_flag_targets_auth_status`                         | tests/auth_profiles.rs              | PR #275    |
| `test_cloud_id_flag_picks_named_resource_not_first`              | tests/multi_cloudid_disambiguation.rs | PR #320  |
| `test_interactive_select_via_stdin_picks_second_resource`        | tests/multi_cloudid_disambiguation.rs | PR #320  |
| `test_interactive_render_shows_name_url_and_id`                  | tests/multi_cloudid_disambiguation.rs | PR #320  |
| `test_single_resource_no_regression_single_org_path`             | tests/multi_cloudid_disambiguation.rs | PR #320  |
| `test_concurrent_invalid_grant_no_thundering_herd`               | tests/oauth_refresh_integration.rs  | pre-existing (orchestrator-confirmed) |
| `test_concurrent_sends_single_refresh_via_coordinator`           | tests/oauth_refresh_integration.rs  | pre-existing |
| `test_invalid_grant_surfaces_not_authenticated_with_refresh_hint`| tests/oauth_refresh_integration.rs  | pre-existing |
| `test_refresh_contract_pins_url_grant_type_rotation_invalid_grant`| tests/oauth_refresh_integration.rs  | pre-existing |
| `test_send_caps_refresh_at_one_attempt_when_refresh_fails`       | tests/oauth_refresh_integration.rs  | pre-existing |
| `test_send_caps_refresh_at_one_attempt_when_retry_also_401`      | tests/oauth_refresh_integration.rs  | pre-existing |
| `test_send_retries_once_after_refresh_on_401`                    | tests/oauth_refresh_integration.rs  | pre-existing |

The orchestrator pre-identified 3 of these (`oauth_refresh_integration`).
This F6 run additionally surfaces 5 more macOS-keychain-blocked tests
(1 from auth_profiles, 4 from multi_cloudid_disambiguation), bringing the
known pre-existing baseline failure count from 3 to 12 on macOS hosts. CI
on Linux hosts runs these tests successfully (CI on `699a5fd` and `c59651b`
both green — see CI Status section). **This is a Linux-vs-macOS test
environment delta, not a regression introduced by issue #396.**

Recommendation (separate maintenance issue, NOT a blocker for F6): gate
the 9 newly-surfaced tests behind `#[ignore]` + `JR_RUN_KEYRING_TESTS=1`,
matching the pattern already used by `oauth_embedded_login.rs::embedded_login_uses_fixed_port`
(`#[ignore]` with `JR_RUN_OAUTH_INTEGRATION=1`).

### 6. Property checks (proptest)

`parse_field_kv` has 4 proptest properties in `src/cli/issue/create.rs::parse_field_kv_proptests`:
- `prop_parse_field_kv_first_equals_split` — first `=` splits name from value
- `prop_parse_field_kv_empty_value_allowed` — `name=` is accepted (empty value)
- `prop_parse_field_kv_last_value_wins_on_duplicates` — duplicate keys collapse last-wins
- `prop_parse_field_kv_no_panic_on_arbitrary_input` — no panic on arbitrary strings

All 4 passed in the v3 test run (included in the 1,459 total).

**Decision on adding `resolve_edit_fields` proptest:** Skipped. With 100% mutation
kill rate on the changed `create.rs` lines AND `resolve_edit_fields` covered by
the 45 integration tests in `tests/issue_edit_field.rs` (which include AC-013 +
EC-3.4.017-13 edge cases), an additional proptest harness for `resolve_edit_fields`
would add maintenance burden without uncovering new behaviours. If future
mutation runs expand the `examine_globs` scope to include `field_resolve.rs`
and surface MISSED mutants, that decision should be revisited.

### 7. Purity boundary audit (`src/cli/issue/field_resolve.rs::resolve_edit_fields`)

| Concern                              | Classification | Location                |
|--------------------------------------|----------------|-------------------------|
| `read_fields_cache(profile)?`        | I/O (filesystem) | line 133              |
| `client.list_fields().await?`        | I/O (network)  | line 204                |
| `write_fields_cache(profile, ...)?`  | I/O (filesystem) | line 212              |
| `client.get_editmeta(key).await?`    | I/O (network)  | line 237                |
| `search_field` (nested fn)           | PURE           | lines 140–181           |
| Step 4 type dispatch (string/number/date/datetime/user/option) | PURE | lines 266–420 |
| Step 4a option resolution (id-bypass / exact / substring)      | PURE | lines 305–409 |
| Step 1 `customfield_NNNNN` guard     | PURE           | lines 92–94             |
| `parse_field_kv` (called upstream)   | PURE           | `create.rs:2086`        |

**Observation:** The function is a single async fn that interleaves 4 I/O sites
with pure logic. It is NOT maximally partitioned (a "build plan" / "execute plan"
split would extract the pure search/dispatch logic into a separate testable unit).
This is a design pragmatism, not a purity leak — the I/O sites are clearly
identifiable (3 `client.*.await?` + 2 cache calls) and the nested `search_field`
fn is already unit-testable as-is.

**Profile cache boundary intact:** All 4 cache I/O sites correctly thread
`profile: &str` as the first arg (line 132 reader, line 212 writer). This honours
the CLAUDE.md "Multi-profile boundary" gotcha. No cross-profile leakage paths.

**Best-effort writer pattern correctly applied:** `write_fields_cache` in
`src/cache.rs:296` follows the documented pattern — wraps `write_cache` and
emits `eprintln!("warning: ...")` on I/O error, returning `Ok(())`. The rustdoc
explicitly cites CLAUDE.md and chooses model (b) ("read-acceleration shortcut,
not correctness-critical"). The unit test
`test_write_fields_cache_swallow_io_error_returns_ok` pins the swallow behaviour
by overriding `XDG_CACHE_HOME` to a file (causing ENOTDIR on `create_dir_all`).

### 8. CI Status

| Commit    | Workflow | Conclusion | Timestamp                  |
|-----------|----------|------------|----------------------------|
| `699a5fd` (HEAD, FIX-F5-001 merge) | CI | **success** | 2026-05-25T14:03:35Z |
| `c59651b` (S-396 merge per orchestrator) | CI | **success** | 2026-05-25T13:44:40Z |
| `2f61566` (b49f2fd parent line) | CI | success | 2026-05-23T21:58:57Z |

CI is green on both the S-396 merge commit and the FIX-F5-001 merge commit.
The cargo-mutants gate documented at PR-merge time on `c59651b` is reaffirmed
locally on `699a5fd` (15/15 caught, 5m wall time).

## Findings & Observations

1. **R-1 (resolved):** No new advisories introduced by `temp-env 0.3` or `scopeguard 1`.
2. **R-2 (informational):** `cargo mutants` examine_globs scope excludes the new files
   (`field_resolve.rs`, `editmeta.rs`, the new `FieldsCache` block in `cache.rs`, and
   `get_editmeta` in `issues.rs`). End-to-end coverage substitutes — 45 integration
   tests + 1 inline unit test for the swallow-writer. If the team wants direct mutation
   coverage of these files, expand `.cargo/mutants.toml::examine_globs`.
3. **R-3 (out of scope for F6, recommend separate issue):** 9 keychain-touching tests
   block on macOS but pass on Linux CI. Recommend `#[ignore]` + `JR_RUN_KEYRING_TESTS=1`
   gate, matching the existing pattern. Pre-existing — NOT introduced by issue #396.
4. **R-4 (positive):** FIX-F5-001 follow-up includes a negative-regression test
   (`test_label_plus_summary_rejected_with_exit_64_no_http`) that proves the entire
   `--label` conflict block is exercised, not just the new `--field` entry. This is
   exemplary regression-pinning practice (previously the conflict block had ZERO
   coverage despite holding 11 conflict entries).
5. **R-5 (positive):** All `unwrap_*` calls in `field_resolve.rs` are on `Option`
   types with safe fallbacks (`.unwrap_or`, `.unwrap_or_else`, `.unwrap_or(&[])`).
   The one `unwrap()` (line 215) is preceded by an unconditional `Some` assignment.
6. **R-6 (positive):** `editmeta` response is intentionally NOT cached (per rustdoc
   on `get_editmeta` line 458). Admin-mutable Edit-screen config + `allowedValues`
   correctness means stale cache → wrong wire ID. Correct design choice.

## Verdict

**PASS** — All F6 hardening criteria met:

- Mutation kill rate: **100%** (15/15) on the in-scope `create.rs` delta — exceeds the 90% threshold and the 95% security-critical threshold.
- `cargo audit`: 0 vulnerabilities (exit 0).
- `cargo deny check`: ok (exit 0; only pre-existing license-allowance warnings).
- Full regression suite: **1,459 passed, 0 failed** (exit 0) after skipping 12 known macOS-keychain-blocked tests (none of which are introduced by issue #396; 3 pre-confirmed by orchestrator, 9 newly surfaced and tracked as a separate maintenance issue).
- Property tests (`parse_field_kv` x 4): all pass.
- Purity boundary: I/O sites clearly identified; pure logic correctly partitioned at the `search_field` and type-dispatch layer; profile-cache boundary respected; best-effort-writer pattern correctly applied.
- CI on the HEAD commit (`699a5fd`) and the S-396 merge commit (`c59651b`) both green.

No security findings requiring escalation to `security-reviewer`.
No surviving mutants requiring routing back to implementer.

The merged delta is hardened and ready for the next pipeline phase.

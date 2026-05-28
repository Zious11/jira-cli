---
document_type: story
story_id: "S-428"
title: "Wiremock-only refactor: extract resolve_cloud_id + rewrite tests #4/#5/#6 in-process (closes #428)"
wave: feature-followup
status: ready
intent: bug-fix
feature_type: infrastructure
scope: non-trivial
severity: low
trivial_scope: false
issue: 428
points: 3
priority: medium
tdd_mode: strict
estimated_effort: small
depends_on: []
bc_anchors: []
# No BC anchor — this is a production refactor with byte-identical observable behavior.
# No new BCs. The existing OAuth multi-org disambiguation contract (BC-1.5.038, anchored
# by S-3.04) is unchanged. BC status: no BC authorship required.
# Status=ready because: (a) F1 human-approval gate passed 2026-05-28 with all four Open
# Questions resolved, (b) no new BCs gate this story, and (c) design decisions are locked.
verification_properties: []
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: ".factory/phase-f1-delta-analysis/issue-428/delta-analysis.md"
implementation_strategy: tdd
module_criticality: MEDIUM
traces_to:
  - L-410-1
  - L-421-4
  - issue-428
  - issue-429
files_modified:
  - src/api/auth.rs                      # MODIFIED — lift AccessibleResource to module scope (#[doc(hidden)] pub, Debug, PartialEq, Deserialize); extract #[doc(hidden)] pub resolve_cloud_id function; update call site in oauth_login
  - tests/multi_cloudid_disambiguation.rs # MODIFIED — rewrite bodies of tests #4, #5, #6 to call resolve_cloud_id in-process via Vec<AccessibleResource> struct literals; remove jr_isolated() from those three tests
  - CLAUDE.md                            # MODIFIED (same commit) — update JR_RUN_KEYRING_TESTS=1 bullet to describe in-process pattern for tests #4/#5/#6
files_created: []
breaking_change: false
assumption_validations: []
risk_mitigations: []
---

# S-428 — Wiremock-Only Refactor: Extract `resolve_cloud_id` + Rewrite Tests #4/#5/#6 In-Process

## Source of Truth

GitHub issue: https://github.com/Zious11/jira-cli/issues/428
F1 delta analysis: `/Users/zious/Documents/GITHUB/jira-cli/.factory/phase-f1-delta-analysis/issue-428/delta-analysis.md`
Immediate predecessor: S-410 (PR #416, merged 2026-05-27) — gated 6 KEYCHAIN-TRANSITIVE tests
Issue #429: related root-cause alternative (see References section)

## Goal

Extract the multi-org disambiguation block in `src/api/auth.rs::oauth_login` into a
`#[doc(hidden)] pub` function `resolve_cloud_id`, then rewrite the three flaking integration tests
(#4, #5, #6 in `tests/multi_cloudid_disambiguation.rs`) to call that function in-process
via wiremock — eliminating the keychain-race root cause at the test level without losing
always-run CI coverage of the exit-64 disambiguation contract.

## Background

S-410 (PR #416) gated 6 KEYCHAIN-TRANSITIVE tests in `tests/multi_cloudid_disambiguation.rs`
behind `JR_RUN_KEYRING_TESTS=1`. At F1 for S-410, the architect classified tests #4, #5, #6
as NO-KEYCHAIN on the basis that their subprocess exits 64 before `store_oauth_tokens` is
reached (exit-code reasoning).

That classification was incorrect per L-421-4: the architect only checked the explicit
code path to keychain write, not the full subprocess lifecycle. The `jr_isolated()` helper
sets `JR_SERVICE_NAME` at subprocess spawn time — before any exit-64 branch is reached —
which triggers macOS keychain interaction for a novel service name regardless of whether
the code path reaches `store_oauth_tokens`. Observed flakes confirmed the misclassification:
three occurrences of `SecItemAlreadyExists` keychain error on CI, each exiting 1 instead
of the expected 64.

The F1 v2 delta analysis considered two options: (a) gate tests #4/#5/#6 (simpler, closes
CI flakes, but accepts a coverage gap — always-run coverage of the exit-64 disambiguation
contract is lost) or (b) close the coverage gap via a wiremock-only refactor. The human
approval gate chose option (b). This story implements option (b).

**No new BCs.** The refactor does not alter observable behavior. The exit-64 /
`JrError::UserError` contract for OAuth multi-org disambiguation is governed by BC-1.5.038
(anchored by S-3.04, PR #320). The refactor is internal restructuring. The BC text is
unchanged. Existing test `src/error.rs::tests::user_error_exit_code` already pins
`JrError::UserError("test".into()).exit_code() == 64`, providing exit-code coverage
without requiring subprocess tests.

## Scope

The story covers five atomic changes, all delivered in a single commit:

### A. Production refactor — `src/api/auth.rs::oauth_login`

The three-branch disambiguation block (currently the inline `match resources.len()` block
starting at the `// Disambiguation: BC-1.5.038` comment in `oauth_login`) is extracted
into a new named function `resolve_cloud_id`. The extraction is a pure "lift into named
function" — no behavioral change.

Call site change in `oauth_login`:

```rust
// BEFORE:
let resource_id: String = match resources.len() { ... };

// AFTER:
let resource_id = resolve_cloud_id(&resources, cloud_id_override, no_input)
    .map_err(anyhow::Error::from)?;
```

The extracted function's signature (locked at F1 approval):

```rust
#[doc(hidden)]
pub fn resolve_cloud_id(
    resources: &[AccessibleResource],
    cloud_id_override: Option<&str>,
    no_input: bool,
) -> Result<String, crate::error::JrError>
```

`pub(crate)` is invisible to the integration-test crate (separate crate linkage); `pub` is required and `#[doc(hidden)]` signals not-a-supported-public-API.

Return type is `Result<String, crate::error::JrError>` (not `anyhow::Error`) so tests
can match on the error variant directly without downcasting.

The extracted function includes ALL THREE disambiguation branches verbatim:
- 0-arm: `Err(JrError::UserError(...))` (empty resources)
- 1-arm: `Ok(resources[0].id.clone())`
- multi-org arm: cloud_id_override search, no_input exit-64, interactive dialoguer/stdin

**Important:** the 0-arm changes from `return Err(...)` (inside an async fn that returns
`anyhow::Result`) to `Err(...)` (function returns `Result<String, JrError>` directly).
Semantics are identical — no behavioral change.

### B. Type promotion — `AccessibleResource` in `src/api/auth.rs`

`AccessibleResource` is currently defined as a function-local struct inside `oauth_login`.
It must be lifted to module scope with these attributes (locked at F1 approval):

```rust
#[doc(hidden)]
#[derive(Debug, PartialEq, serde::Deserialize)]
pub struct AccessibleResource {
    pub id: String,
    pub url: String,
    pub name: String,
}
```

`pub(crate)` is invisible to the integration-test crate (separate crate linkage); `pub` is required and `#[doc(hidden)]` signals not-a-supported-public-API. The visibility is unconditional — NOT gated behind `#[cfg(test)]`. The function may have future callers (e.g., `jr auth check`).

### C. Test rewrites — `tests/multi_cloudid_disambiguation.rs` tests #4, #5, #6

The three tests are rewritten to call `resolve_cloud_id` in-process. They retain wiremock
(`MockServer`) for any HTTP setup inherited from their current bodies, but they NO LONGER
spawn `jr_isolated()` and NO LONGER touch the keychain. They do NOT receive `#[ignore]`.

Tests to rewrite (by function name):

- `test_cloud_id_flag_value_not_in_response_exits_64` (test #4)
- `test_no_input_multi_org_exits_64_with_actionable_error` (test #5)
- `test_no_input_multi_org_lists_available_cloud_ids_in_error` (test #6)

Test fixture construction: tests use `Vec<AccessibleResource>` struct literals directly
(no serde JSON round-trip required). This is cleaner and faster than deserializing fixture
JSON. The existing wiremock mock-setup helpers in the test file may still be used for
any mounts needed, but the test assertions pivot from `exit_code == 64` (subprocess) to
`Err(JrError::UserError(_))` (in-process).

Expected assertion pattern for each test (per F1 delta analysis Section 13):

```rust
// test #4: --cloud-id not in response
let result = jr::api::auth::resolve_cloud_id(
    &resources,
    Some("cloud-NONEXISTENT"),
    true,
);
let err = result.unwrap_err();
assert!(matches!(err, jr::error::JrError::UserError(_)));
let msg = err.to_string();
assert!(msg.contains("cloud-NONEXISTENT"));
assert!(msg.contains("not found") || msg.contains("not found in accessible"));

// test #5: --no-input multi-org
let result = jr::api::auth::resolve_cloud_id(&resources, None, true);
let err = result.unwrap_err();
assert!(matches!(err, jr::error::JrError::UserError(_)));
let msg = err.to_string();
assert!(msg.contains("Multiple"));
assert!(msg.contains("--cloud-id"));

// test #6: --no-input lists available IDs in error
let result = jr::api::auth::resolve_cloud_id(&resources, None, true);
let err = result.unwrap_err();
let msg = err.to_string();
assert!(msg.contains(name1));
assert!(msg.contains(name2));
assert!(msg.contains(cloud_id1));
assert!(msg.contains(cloud_id2));
```

The 9 other tests in `tests/multi_cloudid_disambiguation.rs` (including the 6
already-gated KEYCHAIN-TRANSITIVE tests #2, #3, #7, #9, #10, #12 from S-410) are
NOT modified.

### D. CLAUDE.md update — keyring tests bullet (same commit)

The CLAUDE.md `JR_RUN_KEYRING_TESTS=1` bullet in the "AI Agent Notes" section currently
reads approximately:

```
Coverage includes inline unit tests in `src/api/auth.rs` and integration tests in
`tests/auth_profiles.rs`, `tests/multi_cloudid_disambiguation.rs` (6 KEYCHAIN-TRANSITIVE
tests touching `store_oauth_tokens`), and `tests/oauth_refresh_integration.rs`
(4 KEYCHAIN-DIRECT + 7 KEYCHAIN-TRANSITIVE tests touching `load_oauth_tokens`/`store_oauth_tokens`).
```

After this story, the description for `tests/multi_cloudid_disambiguation.rs` must be
updated to reflect that the file's 12 tests now split into three groups:

```
`tests/multi_cloudid_disambiguation.rs` (6 KEYCHAIN-TRANSITIVE tests touching
`store_oauth_tokens`; tests #4, #5, #6 in the same file are in-process tests (no subprocess, no MockServer — Vec<AccessibleResource> struct literals)
that call `resolve_cloud_id` directly — no keyring access, no JR_RUN_KEYRING_TESTS gate)
```

The count "6 KEYCHAIN-TRANSITIVE" STAYS AT 6. Tests #4/#5/#6 become in-process, not
keychain-transitive — they do NOT increment the count.

**Atomic-doc-fallout rule (per PR #356 R14-R18 lesson):** The CLAUDE.md update MUST be
in the SAME commit as the `src/api/auth.rs` production change and the test rewrites.
A separate "docs" commit is not acceptable.

### E. Followup linkage

Issue #429 was filed during the S-410/S-428 F1 cycle as an alternative root-cause fix:
a crypto-random `JR_SERVICE_NAME` suffix for `jr_isolated()` subprocess tests. This
story's in-process refactor is the chosen mitigation path for tests #4/#5/#6. If this
story merges, #429 may be superseded as WONTFIX. Decide at F7 review.

## Acceptance Criteria

### AC-001 — `AccessibleResource` is at module scope with correct attributes

`src/api/auth.rs` defines `AccessibleResource` at module scope (outside any function),
not as a function-local struct inside `oauth_login`.

The struct has exactly:
- `#[doc(hidden)]` attribute
- `#[derive(Debug, PartialEq, serde::Deserialize)]` (in addition to any existing derives)
- `pub` visibility on the struct and fields `id`, `url`, `name`

(`pub(crate)` is invisible to the integration-test crate (separate crate linkage); `pub` is required and `#[doc(hidden)]` signals not-a-supported-public-API.)

Verification: `grep -n "pub struct AccessibleResource" src/api/auth.rs` returns
exactly one match at module scope (not inside a function body).

### AC-002 — `resolve_cloud_id` is a `#[doc(hidden)] pub` function at module scope

`src/api/auth.rs` contains `#[doc(hidden)] pub fn resolve_cloud_id` with the locked signature:

```rust
#[doc(hidden)]
pub fn resolve_cloud_id(
    resources: &[AccessibleResource],
    cloud_id_override: Option<&str>,
    no_input: bool,
) -> Result<String, crate::error::JrError>
```

(`pub(crate)` is invisible to the integration-test crate (separate crate linkage); `pub` is required and `#[doc(hidden)]` signals not-a-supported-public-API.)

The function is NOT `async` — the disambiguation logic is pure (no I/O on non-interactive
paths). The interactive branch (dialoguer) is synchronous.

Verification: `grep -n "pub fn resolve_cloud_id" src/api/auth.rs` returns exactly
one match.

### AC-003 — All three disambiguation branches are present in `resolve_cloud_id`

The extracted function body contains all three cases:
1. `0` resources — returns `Err(JrError::UserError(...))`
2. `1` resource — returns `Ok(resources[0].id.clone())`
3. Multiple resources (the `_` arm) — includes:
   - `cloud_id_override` present: find-by-id or `Err(JrError::UserError(...))`
   - `no_input` true without override: `Err(JrError::UserError(...))`
   - Interactive branch: dialoguer or line-based fallback

Verification: `grep -c "=> Ok(resources\[0\].id.clone())" src/api/auth.rs` returns exactly 1
(the single-resource fast-path, now inside `resolve_cloud_id`). The old call site in
`oauth_login` must no longer contain the inline `match resources.len()` block.

### AC-004 — `oauth_login` delegates to `resolve_cloud_id`

The disambiguation block in `src/api/auth.rs::oauth_login` (previously the inline
`match resources.len() { ... }` block) is replaced by a call to `resolve_cloud_id`.

The call site matches the pattern:

```rust
let resource_id = resolve_cloud_id(&resources, cloud_id_override, no_input)
    .map_err(anyhow::Error::from)?;
```

Verification: `grep -n "resolve_cloud_id" src/api/auth.rs` returns at least two matches:
the function definition and the call site in `oauth_login`.

### AC-005 — Test #4 passes without `JR_RUN_KEYRING_TESTS=1`, without keychain

`test_cloud_id_flag_value_not_in_response_exits_64` in `tests/multi_cloudid_disambiguation.rs`:
- Does NOT call `jr_isolated()` in its body
- Does NOT have `#[ignore]` attribute
- Asserts `Err(JrError::UserError(_))` with `msg.contains("cloud-NONEXISTENT")` and a
  "not found" cue string
- Passes under `cargo test` without setting any env vars

Verification: `grep -n "jr_isolated" tests/multi_cloudid_disambiguation.rs | grep
"cloud_id_flag_value_not_in_response"` returns no matches.

### AC-006 — Test #5 passes without `JR_RUN_KEYRING_TESTS=1`, without keychain

`test_no_input_multi_org_exits_64_with_actionable_error` in `tests/multi_cloudid_disambiguation.rs`:
- Does NOT call `jr_isolated()` in its body
- Does NOT have `#[ignore]` attribute
- Asserts `Err(JrError::UserError(_))` with `msg.contains("Multiple")` AND
  `msg.contains("--cloud-id")`
- Passes under `cargo test` without setting any env vars

### AC-007 — Test #6 passes without `JR_RUN_KEYRING_TESTS=1`, without keychain

`test_no_input_multi_org_lists_available_cloud_ids_in_error` in `tests/multi_cloudid_disambiguation.rs`:
- Does NOT call `jr_isolated()` in its body
- Does NOT have `#[ignore]` attribute
- Asserts `Err(JrError::UserError(_))` with message containing both org names AND both
  cloud IDs from the fixture data
- Passes under `cargo test` without setting any env vars

### AC-008 — 6 KEYCHAIN-TRANSITIVE tests (#2, #3, #7, #9, #10, #12) remain unchanged

The six tests gated by S-410 (PR #416) are NOT modified:
- `test_cloud_id_flag_is_parsed_not_rejected_by_clap` (test #2)
- `test_cloud_id_flag_picks_named_resource_not_first` (test #3)
- `test_single_resource_no_regression_single_org_path` (test #7)
- `test_cloud_id_flag_does_not_change_redirect_uri_in_authorize_url` (test #9)
- `test_interactive_select_via_stdin_picks_second_resource` (test #10)
- `test_interactive_render_shows_name_url_and_id` (test #12)

Each retains `#[ignore = "..."]` (message form) and the `JR_RUN_KEYRING_TESTS=1` early-return guard.

Verification: `grep -c '#\[ignore' tests/multi_cloudid_disambiguation.rs` returns
exactly **6** (same as post-S-410; matches both bare `#[ignore]` and message-form `#[ignore = "..."]`).

### AC-009 — Observable behavior of `oauth_login` is byte-identical before and after

The integration test suite for `oauth_login` (tests #2, #3, #7, #9, #10, #12 — all
KEYCHAIN-TRANSITIVE) produces identical outcomes under `JR_RUN_KEYRING_TESTS=1`:

```
JR_RUN_KEYRING_TESTS=1 cargo test -- --include-ignored multi_cloudid_disambiguation
```

All 12 tests pass. No test changes outcome as a result of the extraction.

On CI (without the env var), `cargo test` exits 0 — all 6 always-run tests pass (tests
#1, #4, #5, #6, #8, #11) and the 6 gated tests are skipped.

### AC-010 — CLAUDE.md update in the same commit as code changes

The CLAUDE.md `JR_RUN_KEYRING_TESTS=1` bullet is updated to describe that:
- The count of KEYCHAIN-TRANSITIVE tests in `tests/multi_cloudid_disambiguation.rs`
  remains **6**.
- Tests #4, #5, #6 in the same file now exercise `resolve_cloud_id` in-process (no subprocess, no MockServer — Vec<AccessibleResource> struct literals)
  without keyring access and therefore do NOT require the `JR_RUN_KEYRING_TESTS`
  gate.

The update is in the SAME commit as the `src/api/auth.rs` and test changes. There is no
separate documentation commit.

Verification: `git show HEAD --stat` includes both `src/api/auth.rs`,
`tests/multi_cloudid_disambiguation.rs`, and `CLAUDE.md` in the diff.

### AC-011 — `cargo test` (no env vars) exits 0; `cargo clippy` and `cargo fmt` clean

`cargo test` exits 0 without any special env vars set.
`cargo fmt --all -- --check` exits 0.
`cargo clippy --all-targets -- -D warnings` exits 0.
No `#[allow(...)]` attributes added.

### AC-012 — Script invariants pass

`bash scripts/check-spec-counts.sh` exits 0.
`bash scripts/check-bc-cumulative-counts.sh` exits 0.
`bash scripts/check-bc-no-numeric-test-counts.sh` exits 0.

No BC files are modified by this story. All three scripts must pass with zero edits to
`.factory/specs/prd/` files.

## Out of Scope

- **Issue #429 root-cause fix** — adding a crypto-random `JR_SERVICE_NAME` suffix to
  `jr_isolated()` to prevent keychain contention across parallel subprocess tests. That
  fix addresses the underlying flake mechanism. This story's in-process refactor eliminates
  the need for tests #4/#5/#6 to spawn a subprocess at all; #429 may become WONTFIX. See
  References.
- **Any wider cleanup of `src/api/auth.rs`** — e.g., shard split, renaming functions,
  extracting further helpers. The only production code change is the specific extraction
  described in Scope A and B.
- **Changes to the 6 already-gated KEYCHAIN-TRANSITIVE tests** (#2, #3, #7, #9, #10, #12).
  Those tests are untouched.
- **Changes to `tests/oauth_refresh_integration.rs`** — not involved.
- **Changes to `tests/auth_profiles.rs`** — not involved.
- **Adding a new env var or test gate** — no `JR_RUN_CLOUDID_INTEGRATION` or similar.
- **Modifying the `oauth_login` function signature** — it is unchanged. Only the call site
  within `oauth_login` changes from inline-match to `resolve_cloud_id(...)` delegation.

## References

- **F1 delta analysis (v2):** `.factory/phase-f1-delta-analysis/issue-428/delta-analysis.md`
- **GitHub issue #428:** https://github.com/Zious11/jira-cli/issues/428
- **GitHub issue #429 (sequence note):** If this story's wiremock-only refactor is the chosen
  mitigation path (i.e., it merges), issue #429 may be superseded as WONTFIX. Decide at F7.
  #429 tracks adding a crypto-random suffix to `JR_SERVICE_NAME` in `jr_isolated()` as an
  alternative root-cause fix for subprocess keychain races.
- **L-410-1** (codified 2026-05-27): F1 per-test audit must cross-check table row count via
  grep. Applied in the F1 v2 analysis — row count 12 matched grep count 12.
- **L-421-4** (codified 2026-05-28): The architect's "follows exit path" reasoning for
  keychain classification was incomplete — it considered only the explicit code path to
  keychain write, not the full subprocess lifecycle. This is the root lesson motivating S-428.
- **Predecessor S-410:** PR #416 (merged 2026-05-27) — gated 6 KEYCHAIN-TRANSITIVE tests.
  This story builds on S-410's groundwork and closes the coverage gap S-410 left open.
- **BC-1.5.038 (S-3.04, PR #320):** The behavioral contract for OAuth multi-org disambiguation
  is unchanged by this story. The exit-64 / `JrError::UserError` contract is preserved.

## Implementation Strategy

**Ordered sequence:**

1. **Create branch** `fix/S-428-wiremock-only-disambiguation` from `develop`.

2. **Read `src/api/auth.rs` around the `AccessibleResource` definition and the
   `// Disambiguation: BC-1.5.038` comment** to confirm exact current text. Do NOT use
   line numbers from this story as ground truth — verify against the actual file at the
   time of implementation. The disambiguation block is currently the `match resources.len()`
   block inside `src/api/auth.rs::oauth_login`.

3. **Read `tests/multi_cloudid_disambiguation.rs` in full** to verify the 12 test functions
   and their current bodies for tests #4, #5, #6. Note the fixture helpers available (e.g.,
   `two_resources_b_first`) to understand what mock data already exists that can be repurposed
   as `Vec<AccessibleResource>` struct literals.

4. **Lift `AccessibleResource` to module scope** (Scope B): move the struct definition out
   of `oauth_login` to the module level; add `#[doc(hidden)]`, `#[derive(Debug, PartialEq, serde::Deserialize)]`,
   and `pub` visibility on the struct and fields.

5. **Extract `resolve_cloud_id`** (Scope A): lift the `match resources.len() { ... }` block
   verbatim into the new function with the locked signature. Update the call site in
   `oauth_login`. Verify the `0`-arm changes from `return Err(...).into()` to `Err(...)`.

6. **Compile check:** `cargo build` must exit 0 before touching test files.

7. **Rewrite test #4** (`test_cloud_id_flag_value_not_in_response_exits_64`): remove
   `jr_isolated()`, build a `Vec<AccessibleResource>` via struct literals matching the
   test's existing mock data, call `jr::api::auth::resolve_cloud_id(...)`, assert
   `Err(JrError::UserError(_))` with message content checks. Keep any wiremock mounts
   that the test still needs; remove any that are only needed for the subprocess path.

8. **Rewrite test #5** (`test_no_input_multi_org_exits_64_with_actionable_error`):
   same pattern — struct literals, in-process call, `Err` assertion.

9. **Rewrite test #6** (`test_no_input_multi_org_lists_available_cloud_ids_in_error`):
   same pattern — two-org fixture, `msg.contains(name1)` etc.

10. **Update CLAUDE.md** (Scope D): update the keyring-tests bullet as specified. This
    must be done before committing.

11. **Run `cargo test`** — exits 0 (AC-011).

12. **Verify #[ignore] count unchanged:** `grep -c '#\[ignore' tests/multi_cloudid_disambiguation.rs`
    returns 6 (AC-008; the pattern matches both bare `#[ignore]` and message-form `#[ignore = "..."]`).

13. **Run `cargo fmt --all -- --check`** and `cargo clippy --all-targets -- -D warnings`
    — both exit 0 (AC-011).

14. **Run script invariants** (AC-012).

15. **Commit all three file changes atomically** (per AC-010):
    ```
    fix(auth): extract resolve_cloud_id + in-process wiremock tests #4/#5/#6 (closes #428)
    ```

16. **Open PR** targeting `develop`; body includes `Closes #428`.

## Test Coverage Strategy

| Test type | Count | Location | What it tests |
|-----------|-------|----------|---------------|
| In-process unit-style (rewritten, previously subprocess) | 3 | `tests/multi_cloudid_disambiguation.rs` tests #4, #5, #6 | `resolve_cloud_id` error paths: not-found override, no-input multi-org exit-64, multi-org listing in error message |
| Always-run NO-KEYCHAIN (unchanged) | 3 | `tests/multi_cloudid_disambiguation.rs` tests #1, #8, #11 | Help text, callback URL, disambiguation help text |
| KEYCHAIN-TRANSITIVE gated (unchanged from S-410) | 6 | `tests/multi_cloudid_disambiguation.rs` tests #2, #3, #7, #9, #10, #12 | Full OAuth flow incl. store_oauth_tokens — run only with JR_RUN_KEYRING_TESTS=1 |
| Existing unit test (unchanged) | 1 | `src/error.rs::tests::user_error_exit_code` | `JrError::UserError(_) => 64` mapping (pins exit-code contract without subprocess) |

Net suite delta: 0 new test functions (rewrites only). The 3 rewritten tests move from
subprocess+keychain-touching to in-process+pure — they become more reliable, not fewer.

Note: the #1–#12 labels above are LOGICAL labels per the S-3.04 AC→test mapping, NOT strict
file-position order. In particular, the always-run `test_cloud_id_help_text_mentions_disambiguation_or_multiple_orgs`
and the gated `test_interactive_render_shows_name_url_and_id` are positionally swapped relative
to their logical labels. Do not renumber; the gated set is correctly identified by function name in AC-008.

## Quality Gate Self-Check

| Criterion | Required | Notes |
|-----------|----------|-------|
| `cargo test` exits 0 (no env vars) | AC-011 | All 6 always-run tests pass; 6 gated skipped |
| `grep -c '#\[ignore' tests/multi_cloudid_disambiguation.rs` → 6 | AC-008 | Same count as post-S-410; matches message-form `#[ignore = "..."]` |
| Tests #4, #5, #6 have no `jr_isolated()` call | AC-005/AC-006/AC-007 | Grep for absence |
| Tests #4, #5, #6 have no `#[ignore]` | AC-005/AC-006/AC-007 | grep confirm |
| `grep -n "pub struct AccessibleResource" src/api/auth.rs` → 1 match at module scope | AC-001 | Module-level, not function-local |
| `grep -n "pub fn resolve_cloud_id" src/api/auth.rs` → 1 match | AC-002 | Function present |
| `cargo fmt --all -- --check` exits 0 | AC-011 | No format drift |
| `cargo clippy --all-targets -- -D warnings` exits 0 | AC-011 | No `#[allow]` suppressions |
| `bash scripts/check-spec-counts.sh` exits 0 | AC-012 | No BC files touched |
| `bash scripts/check-bc-cumulative-counts.sh` exits 0 | AC-012 | No count drift |
| `bash scripts/check-bc-no-numeric-test-counts.sh` exits 0 | AC-012 | No BC bodies touched |
| CLAUDE.md in same commit as code | AC-010 | `git show HEAD --stat` includes all 3 files |
| (Optional, dev machine) `JR_RUN_KEYRING_TESTS=1 cargo test -- --include-ignored multi_cloudid_disambiguation` exits 0 | AC-009 | Confirms extraction is behavior-identical |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~5 k |
| `src/api/auth.rs` (targeted read: `oauth_login` function scope, ~lines 550-850) | ~8 k |
| `tests/multi_cloudid_disambiguation.rs` (full — 1100+ LOC) | ~14 k |
| `CLAUDE.md` (targeted: `JR_RUN_KEYRING_TESTS=1` section, ~lines 340-370) | ~2 k |
| F1 delta analysis (`.factory/phase-f1-delta-analysis/issue-428/delta-analysis.md`) | ~6 k |
| Tool outputs (`cargo build`, `cargo test`, `cargo clippy`, `grep` verifications) | ~3 k |
| **Total** | **~38 k** |

Well within a single-agent context window (~200 k). No split required.
LOC delta estimate: `src/api/auth.rs` +~60 LOC net (new function ~50 LOC + struct move
~5 LOC + derives ~3 LOC + rustdoc ~5 LOC; inline match block replaced by single call).
`tests/multi_cloudid_disambiguation.rs` net change: 0 or slightly negative (subprocess
boilerplate removed; in-process assertions are typically shorter).

## Tasks

- [ ] Create branch `fix/S-428-wiremock-only-disambiguation` from `develop`
- [ ] Read `src/api/auth.rs` from `oauth_login` entry through end of disambiguation block — confirm exact text of `match resources.len()` and current `AccessibleResource` struct definition (verify function-local scope)
- [ ] Read `tests/multi_cloudid_disambiguation.rs` in full — identify tests #4, #5, #6 function bodies; note available fixture helpers for `Vec<AccessibleResource>` construction
- [ ] Lift `AccessibleResource` to module scope: add `#[doc(hidden)]`, `#[derive(Debug, PartialEq, serde::Deserialize)]`, `pub` visibility on struct and fields (AC-001)
- [ ] Extract `resolve_cloud_id` function with locked signature (AC-002, AC-003); move inline `match` block verbatim; update 0-arm from `return Err(...).into()` to `Err(...)`
- [ ] Update call site in `oauth_login`: replace inline match with `resolve_cloud_id(&resources, cloud_id_override, no_input).map_err(anyhow::Error::from)?` (AC-004)
- [ ] `cargo build` — exits 0 before touching test files
- [ ] Rewrite test #4 (`test_cloud_id_flag_value_not_in_response_exits_64`): remove `jr_isolated()`, construct `Vec<AccessibleResource>` via struct literals, call `jr::api::auth::resolve_cloud_id(...)`, assert `Err(JrError::UserError(_))` + message content (AC-005)
- [ ] Rewrite test #5 (`test_no_input_multi_org_exits_64_with_actionable_error`): same pattern (AC-006)
- [ ] Rewrite test #6 (`test_no_input_multi_org_lists_available_cloud_ids_in_error`): same pattern, two-org fixture (AC-007)
- [ ] Verify `grep -c '#\[ignore' tests/multi_cloudid_disambiguation.rs` → 6 (AC-008)
- [ ] Verify tests #4, #5, #6 have no `jr_isolated()` call (AC-005/006/007)
- [ ] Update CLAUDE.md: JR_RUN_KEYRING_TESTS=1 bullet — count stays 6, add in-process description for tests #4/#5/#6 (AC-010)
- [ ] Run `cargo test` — exits 0 (AC-011)
- [ ] Run `cargo fmt --all -- --check` — exits 0 (AC-011)
- [ ] Run `cargo clippy --all-targets -- -D warnings` — exits 0 (AC-011)
- [ ] Run `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh && bash scripts/check-bc-no-numeric-test-counts.sh` — all exit 0 (AC-012)
- [ ] Commit atomically (all three files in one commit): `fix(auth): extract resolve_cloud_id + in-process wiremock tests #4/#5/#6 (closes #428)`
- [ ] Open PR targeting `develop`; body: `Closes #428`

## Previous Story Intelligence

**Direct predecessor: S-410** (PR #416, merged 2026-05-27 — gate keychain-transitive
tests). S-410 correctly gated 6 tests but misclassified tests #4, #5, #6 as NO-KEYCHAIN.
The misclassification was based on exit-code reasoning (per L-421-4, this is insufficient:
the subprocess lifecycle must be inspected, not just the code path to keychain write).

**Structural predecessor: S-421** (two-stage i64 parser refactor). As a small,
self-contained refactor of a production function with test rewrites, this story is
structurally similar to S-421. Follow the same quality gate self-check pattern.

**Key lesson from L-410-1:** always cross-check the per-test inspection table row count
against `grep -c "^async fn test_\|^fn test_"` on the actual file before beginning edits.
The delta analysis count is 12; verify it matches before writing any test changes.

**Key lesson from L-421-4 (the root cause of this story):** "The architect's 'follows
exit path' reasoning was incomplete — it considered only the explicit code path to keychain
write, not the full subprocess lifecycle." When classifying tests as keychain-touching vs
not, inspect whether the test spawns a subprocess that sets `JR_SERVICE_NAME` — regardless
of whether the code path reaches an explicit keychain call.

**Key lesson from PR #356 R14-R18:** Documentation changes (CLAUDE.md) that describe
test infrastructure must be in the same commit as the code/test changes they describe.
Separate "docs" commits allow the two to drift.

## Architecture Compliance Rules

1. **`#[doc(hidden)] pub` visibility, unconditional.** `resolve_cloud_id` and `AccessibleResource`
   are `#[doc(hidden)] pub`, NOT gated behind `#[cfg(test)]`. `pub(crate)` is invisible to the
   integration-test crate (separate crate linkage); `pub` is required and `#[doc(hidden)]`
   signals not-a-supported-public-API. The function may have future callers in production code.

2. **No production behavior change.** `oauth_login` must exhibit byte-identical observable
   behavior before and after the extraction. The extraction is pure "lift into named
   function" — no logic changes permitted. If any behavioral difference is noticed during
   implementation, stop and escalate.

3. **Function-local structs `TokenResponse` stays.** Only `AccessibleResource` is lifted.
   The `TokenResponse` struct (also currently function-local in `oauth_login`) is NOT moved
   — only `AccessibleResource` is in scope.

4. **`resolve_cloud_id` is NOT async.** The disambiguation logic contains no await points
   on the non-interactive paths (the two paths exercised by tests #4/#5/#6). The interactive
   dialoguer branch is synchronous. Do not add `async`.

5. **No `#[allow]` lint suppressions.** If clippy warns on any expression in the extracted
   function, refactor to satisfy the lint.

6. **9 unchanged tests must not be modified.** Tests #1, #2, #3, #7, #8, #9, #10, #11,
   #12 in `tests/multi_cloudid_disambiguation.rs` are NOT touched. Their line numbers and
   bodies must be identical to the post-S-410 state.

7. **`AccessibleResource` field visibility.** Fields `id`, `url`, `name` must be
   `pub` (with `#[doc(hidden)]` on the struct) for tests to construct `Vec<AccessibleResource>`
   via struct literals without `Default` or builder patterns. `pub(crate)` is invisible to the
   integration-test crate (separate crate linkage) and would make struct-literal construction
   fail to compile in `tests/`.

8. **No new module files.** All changes are within `src/api/auth.rs`,
   `tests/multi_cloudid_disambiguation.rs`, and `CLAUDE.md`.

## Library & Framework Requirements

No new dependencies. No version changes. The refactor uses only:
- Existing `dialoguer` (already in `Cargo.toml`) — interactive branch, unchanged
- `serde::Deserialize` (already in `Cargo.toml`) — derive on `AccessibleResource`
- Rust standard library `std::io::IsTerminal` (already used in the current function body)
- `wiremock` (already in dev-dependencies) — tests already use it; no new mounts required
  for the rewritten tests (they may not need any mounts at all, just struct literals)

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `src/api/auth.rs` | Modify | Lift `AccessibleResource` + extract `resolve_cloud_id`; update `oauth_login` call site |
| `tests/multi_cloudid_disambiguation.rs` | Modify | Rewrite bodies of tests #4, #5, #6 only |
| `CLAUDE.md` | Modify (same commit) | Update keyring-tests bullet per Scope D |

**Files NOT to create:** No new source files, no new spec files, no new test files.

**Files NOT to touch:** `tests/oauth_refresh_integration.rs`, `tests/auth_profiles.rs`,
all other `src/` files, `Cargo.toml`, `deny.toml`, `STORY-INDEX.md` (state-manager
updates that), all BC count surfaces (`.factory/specs/prd/bc-*.md` frontmatter,
`BC-INDEX.md`, `CANONICAL-COUNTS.md`), `CHANGELOG.md` (no user-visible behavior change).

## Branch / PR Plan

- Branch: `fix/S-428-wiremock-only-disambiguation`
- Target: `develop`
- Commit style: `fix(auth): extract resolve_cloud_id + in-process wiremock tests #4/#5/#6 (closes #428)`
- PR closes: `Closes #428`
- CHANGELOG entry: not required (test infrastructure + internal refactor; no user-visible
  behavior change)

---
document_type: story
story_id: "S-410"
title: "Gate keychain-transitive tests behind JR_RUN_KEYRING_TESTS=1 in multi_cloudid_disambiguation.rs and oauth_refresh_integration.rs (closes #410)"
wave: feature-followup
status: ready
intent: bug-fix
feature_type: infrastructure
scope: small
severity: low-medium
trivial_scope: false
issue: 410
points: 1
priority: medium
tdd_mode: strict
estimated_effort: small
depends_on: []
bc_anchors: []
# No BC anchor — this is test infrastructure only. No behavioral contract change.
# No production code path is affected. BC status: no BC authorship required.
# Status=ready is set because the fix is mechanical test annotation with no
# spec-gate prerequisite: the gate convention (JR_RUN_KEYRING_TESTS=1) is already
# codified in CLAUDE.md line 351; this story extends coverage to two additional files.
verification_properties: []
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: ".factory/phase-f1-delta-analysis/issue-410/delta-analysis.md"
implementation_strategy: tdd
module_criticality: MEDIUM
files_modified:
  - tests/multi_cloudid_disambiguation.rs    # MODIFIED — add #[ignore] + JR_RUN_KEYRING_TESTS=1 early-return guard to 5 KEYCHAIN-TRANSITIVE tests (tests #2, 3, 7, 9, 10 per delta-analysis classification tables)
  - tests/oauth_refresh_integration.rs       # MODIFIED — add #[ignore] + JR_RUN_KEYRING_TESTS=1 early-return guard to 7 KEYCHAIN-TRANSITIVE tests (tests #1, 3, 4, 5, 6, 7, 9 per delta-analysis classification tables)
  - CLAUDE.md                                # MODIFIED — extend JR_RUN_KEYRING_TESTS=1 documentation (line ~351) to explicitly name both test files
files_created: []
breaking_change: false
assumption_validations: []
risk_mitigations: []
---

# S-410 — Gate Keychain-Transitive Tests Behind `JR_RUN_KEYRING_TESTS=1`

## Source of Truth

F1 delta analysis: `.factory/phase-f1-delta-analysis/issue-410/delta-analysis.md`
GitHub issue: https://github.com/Zious11/jira-cli/issues/410
CI flake evidence: run 26477547114 — `test_cloud_id_flag_picks_named_resource_not_first` fails on macOS runner.

**No new BCs. No new VPs. No new ADR. No production code changes.**

## Goal

Gate keychain-touching tests behind the existing `JR_RUN_KEYRING_TESTS=1` convention to prevent
macOS keychain prompts during local dev and intermittent CI flakes on headless macOS runners.

## Problem Statement

Two integration test files contain tests that touch the system keychain transitively via
`store_oauth_tokens` / `load_oauth_tokens` in `src/api/auth.rs`. These tests run unconditionally
in CI (no `#[ignore]` gate), which causes intermittent failures on macOS GitHub runners when the
keychain security framework triggers a system prompt — halting the runner.

The existing canonical convention (CLAUDE.md line 351) requires `JR_RUN_KEYRING_TESTS=1` +
`#[ignore]` for all keychain-touching tests, but neither test file follows it:

- `tests/multi_cloudid_disambiguation.rs`: 5 KEYCHAIN-TRANSITIVE tests (#2, 3, 7, 9, 10), none gated
- `tests/oauth_refresh_integration.rs`: 7 KEYCHAIN-TRANSITIVE tests (#1, 3, 4, 5, 6, 7, 9), none gated

The confirmed flaky CI test is `test_cloud_id_flag_picks_named_resource_not_first` (test #3 in
`multi_cloudid_disambiguation.rs`), which runs a full OAuth login flow and calls `store_oauth_tokens`
via `jr_isolated()` — the macOS security framework still fires for novel service names even when
`JR_SERVICE_NAME` redirects to a test-scoped namespace.

## Behavioral Contracts

No BC anchor — this is test-infrastructure only. No production code path is affected.
No user-visible behavior changes. BC status: no BC authorship required.

## Acceptance Criteria

### AC-001 — 5 KEYCHAIN-TRANSITIVE tests in `multi_cloudid_disambiguation.rs` are gated

The following 5 tests have `#[ignore]` attribute AND an early-return guard as the FIRST
statement in the test body:

```rust
if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
    return; // requires JR_RUN_KEYRING_TESTS=1 and system keychain access
}
```

Tests that must be gated (by function name):
1. `test_cloud_id_flag_is_parsed_not_rejected_by_clap` (line 285) — post-implementation reaches `store_oauth_tokens`
2. `test_cloud_id_flag_picks_named_resource_not_first` (line 343) — confirmed CI flake (run 26477547114)
3. `test_single_resource_no_regression_single_org_path` (line 659) — single-org success path calls `store_oauth_tokens`
4. `test_cloud_id_flag_does_not_change_redirect_uri_in_authorize_url` (line 802) — full OAuth flow calls `store_oauth_tokens`
5. `test_interactive_select_via_stdin_picks_second_resource` (line 884) — interactive path calls `store_oauth_tokens`

The `JR_SERVICE_NAME` isolation via `jr_isolated()` must be preserved on all gated tests.
These tests use `#[tokio::test]`; the guard goes INSIDE the async body (cannot be a compile-time gate).

### AC-002 — 7 KEYCHAIN-TRANSITIVE tests in `oauth_refresh_integration.rs` are gated

The following 7 tests have `#[ignore]` attribute AND the same early-return guard:

Tests that must be gated (by function name):
1. `test_send_retries_once_after_refresh_on_401` (line 241) — post-implementation calls `load_oauth_tokens("default")`
2. `test_invalid_grant_surfaces_not_authenticated_with_refresh_hint` (line 428) — same
3. `test_send_caps_refresh_at_one_attempt_when_retry_also_401` (line 504) — same
4. `test_send_caps_refresh_at_one_attempt_when_refresh_fails` (line 562) — same
5. `test_concurrent_sends_single_refresh_via_coordinator` (line 627) — N=10 concurrent calls to `load_oauth_tokens`
6. `test_concurrent_invalid_grant_no_thundering_herd` (line 720) — same
7. `test_refresh_contract_pins_url_grant_type_rotation_invalid_grant` (line 860) — same

**Lock acquisition discipline:** The env lock (`harness::env_lock().lock().await`) must be
acquired AFTER the guard check to avoid holding the lock for skipped tests.

**Already-gated tests are NOT changed:** `test_refresh_persists_rotated_tokens_via_store_oauth_tokens`
(#[ignore] + guard at line 330), `test_waiters_use_in_memory_token_not_keychain` (line 996),
`test_inter_process_reconcile_after_invalid_grant` (line 1119), and
`test_persist_before_publish_fault_injection` (line 1307) are already correctly gated and
must not be disturbed.

### AC-003 — 6 NO-KEYCHAIN tests in `multi_cloudid_disambiguation.rs` remain always-run

The following tests must NOT receive `#[ignore]` — they are always-run red-gate signals for CI:

1. `test_cloud_id_flag_recognized_in_help` (line 249) — `jr auth login --help` only; no keychain
2. `test_cloud_id_flag_value_not_in_response_exits_64` (line 430) — expected exit 64; no keychain write
3. `test_no_input_multi_org_exits_64_with_actionable_error` (line 503) — expected exit 64; no keychain write
4. `test_no_input_multi_org_lists_available_cloud_ids_in_error` (line 575) — expected exit 64; no keychain write
5. `test_callback_url_contains_127_0_0_1_and_port_53682` (line 742) — `jr auth login --help` only; no keychain
6. `test_cloud_id_help_text_mentions_disambiguation_or_multiple_orgs` (line 1049) — `jr auth login --help` only; no keychain

Verification: `grep -n '#\[ignore\]' tests/multi_cloudid_disambiguation.rs` must show exactly
5 lines, one per AC-001 test function. No other `#[ignore]` annotations in this file.

### AC-004 — `test_manual_jr_auth_refresh_unchanged` remains always-run

`test_manual_jr_auth_refresh_unchanged` (line 814 in `oauth_refresh_integration.rs`) must NOT
receive `#[ignore]`. It runs `jr auth refresh --help` only — NO keychain access. It is a
regression guard for the manual refresh command and must continue to run in standard CI.

Verification: `grep -n '#\[ignore\]' tests/oauth_refresh_integration.rs` must show exactly
7 lines — 3 already-gated + 4 newly-gated (the already-gated `test_refresh_persists_rotated_tokens…`
is 1 existing; `test_waiters_use_in_memory_token_not_keychain` is 1 existing; `test_inter_process_reconcile…`
is 1 existing; `test_persist_before_publish_fault_injection` is 1 existing — so 4 pre-existing +
7 newly-gated = 11 total `#[ignore]` annotations in the file post-fix).

### AC-005 — `cargo test` passes without keychain prompts on a clean checkout

`cargo test` (no env vars set) exits 0 on a developer machine or CI runner without prompting
the macOS keychain. No `Security` framework dialog. No `SecItemCopyMatching` prompt in the
process output. The gated tests are skipped via `#[ignore]` and the early-return guard.

This is the convergence proof for the primary bug: the CI flake is gone.

### AC-006 — `JR_RUN_KEYRING_TESTS=1 cargo test -- --include-ignored` runs all tests

`JR_RUN_KEYRING_TESTS=1 cargo test -- --include-ignored` exits 0 (on a developer machine
with keychain access). The 12 newly-gated tests run and pass. No regression to the gated tests
themselves — their logic is unchanged, only the skip gate is added.

This is the convergence proof that the gated tests remain valid.

### AC-007 — CLAUDE.md documentation extended at line ~351

The existing `JR_RUN_KEYRING_TESTS=1` documentation in CLAUDE.md (line ~351) is extended to
explicitly state that the gate covers:
- Inline unit tests in `src/api/auth.rs` (pre-existing)
- `tests/multi_cloudid_disambiguation.rs` KEYCHAIN-TRANSITIVE tests (newly gated by this story)
- `tests/oauth_refresh_integration.rs` KEYCHAIN-TRANSITIVE always-run tests (newly gated by this story)

The update must not alter the meaning of the existing entry — only extend the "covered by" scope.
The `JR_RUN_OAUTH_INTEGRATION=1` entry (line ~352, for `tests/oauth_embedded_login.rs`) must not
be changed.

## Implementation Strategy

This story requires NO production code changes. All changes are test annotations and documentation.

**Ordered sequence:**

1. **Create branch** `fix/keychain-test-isolation` from `develop`.

2. **Read both test files in full** before editing. Verify the test function names and line numbers
   from the delta analysis match the actual file. The delta analysis was produced from a read-only
   audit; line numbers may drift if recent PRs landed after the audit.

3. **Edit `tests/multi_cloudid_disambiguation.rs`:** For each of the 5 KEYCHAIN-TRANSITIVE tests
   (AC-001), add `#[ignore]` before the `#[tokio::test]` attribute, then add the guard as the
   first statement inside the async body. Preserve `jr_isolated()` usage on all gated tests.

4. **Edit `tests/oauth_refresh_integration.rs`:** For each of the 7 KEYCHAIN-TRANSITIVE tests
   (AC-002), add `#[ignore]` before the `#[tokio::test]` attribute, then add the guard BEFORE
   any `harness::env_lock().lock().await` acquisition.

5. **Edit `CLAUDE.md`:** Extend the `JR_RUN_KEYRING_TESTS=1` documentation entry (AC-007).

6. **Run `cargo test`** — must exit 0 with no keychain prompt (AC-005 proxy in local env).

7. **Verify gated test counts:**
   - `grep -c '#\[ignore\]' tests/multi_cloudid_disambiguation.rs` → 5
   - `grep -c '#\[ignore\]' tests/oauth_refresh_integration.rs` → 11

8. **Run `cargo clippy -- -D warnings`** — must exit 0. No new warnings from guard addition.

9. **Run `cargo fmt --all -- --check`** — must exit 0. Format the guard idiom consistently
   with existing guard instances in `src/api/auth.rs` (e.g., the `with_test_keyring` wrapper pattern).

10. **Optionally verify gated tests still execute correctly:**
    `JR_RUN_KEYRING_TESTS=1 cargo test -- --include-ignored multi_cloudid_disambiguation oauth_refresh_integration`
    (on a dev machine with keychain access — not required in CI).

11. Per-story adversary 3/3 CLEAN before push.

## Out of Scope

- **Changing `JR_SERVICE_NAME` isolation in `jr_isolated()`.** The `jr_isolated()` helper correctly
  redirects the keychain service name to a test-scoped namespace. It does NOT eliminate macOS
  security framework interaction but prevents cross-test contamination. Preserving it as-is is
  intentional and correct.
- **Adding a new `JR_RUN_CLOUDID_INTEGRATION` env var.** The delta analysis recommends Option A
  (extend existing `JR_RUN_KEYRING_TESTS=1`) — do not introduce a new gate variable.
- **Changing module-level comments in `oauth_refresh_integration.rs`.** The comment at line 28
  says "All 11 tests" but the actual function count is 12 (off by one as documented in the delta
  analysis). Fixing that count is a cosmetic follow-up, not in scope.
- **Any production code change.** `src/api/auth.rs`, `src/api/`, and all other `src/` files are
  NOT modified.
- **Adding BC bodies or VPs.** Test infrastructure isolation is not a behavioral contract change.
- **Updating BC count surfaces.** `scripts/check-bc-cumulative-counts.sh` must exit 0 with zero
  edits to BC files.

## Regression Risk Mitigations

**Risk rating: LOW** (per F1 delta analysis — test-only changes, no production code paths affected).

- 6 always-run NO-KEYCHAIN tests in `multi_cloudid_disambiguation.rs` continue running in CI
  and provide red-gate signal for flag-registration and error-path ACs.
- `test_manual_jr_auth_refresh_unchanged` continues running in CI as a manual refresh command
  regression guard.
- The 12 newly-gated tests are unchanged in logic — only a skip gate is added. They remain
  executable under `JR_RUN_KEYRING_TESTS=1`.
- No behavior that reaches end users can regress from this change.

## Test Coverage Strategy

The change IS the test isolation fix. There are no new test functions to write.

**Meta-verification strategy (convergence proofs):**

| Verification | How | AC |
|-------------|-----|----|
| Standard CI no longer flakes | `cargo test` exits 0 without env vars (no keychain prompt) | AC-005 |
| Gated tests still valid | `JR_RUN_KEYRING_TESTS=1 cargo test -- --include-ignored` exits 0 | AC-006 |
| No inadvertent always-run gating | `grep -c '#[ignore]' tests/multi_cloudid_disambiguation.rs` → 5 | AC-003 |
| No inadvertent always-run gating | `grep -c '#[ignore]' tests/oauth_refresh_integration.rs` → 11 | AC-004 |
| CLAUDE.md updated | grep for new file names in updated entry | AC-007 |

## Quality Gate Self-Check

| Criterion | Required | Notes |
|-----------|----------|-------|
| `cargo test` exits 0 (no env vars) | AC-005 | Primary convergence proof; gated tests skipped |
| `grep -c '#\[ignore\]' tests/multi_cloudid_disambiguation.rs` → 5 | AC-001/AC-003 | Exactly 5; no accidental gating of NO-KEYCHAIN tests |
| `grep -c '#\[ignore\]' tests/oauth_refresh_integration.rs` → 11 | AC-002/AC-004 | 4 pre-existing + 7 newly-gated |
| `cargo clippy -- -D warnings` exits 0 | project convention | Zero warnings; no `#[allow]` suppressions |
| `cargo fmt --all -- --check` exits 0 | project convention | Guard idiom formatted consistently |
| `bash scripts/check-spec-counts.sh` exits 0 | invariant | No BC files touched; counts unchanged |
| `bash scripts/check-bc-cumulative-counts.sh` exits 0 | invariant | No cumulative count drift |
| `bash scripts/check-bc-no-numeric-test-counts.sh` exits 0 | invariant | No BC files touched |
| Per-story adversary 3/3 CLEAN | project convention | Required before push |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~4 k |
| F1 delta analysis (`.factory/phase-f1-delta-analysis/issue-410/delta-analysis.md`) | ~5 k |
| `tests/multi_cloudid_disambiguation.rs` (full — 1100+ LOC) | ~14 k |
| `tests/oauth_refresh_integration.rs` (full — 1300+ LOC) | ~16 k |
| CLAUDE.md (lines ~340-360 for the JR_RUN_KEYRING_TESTS section) | ~2 k |
| Tool outputs (`cargo test`, `cargo clippy`, `grep` counts) | ~2 k |
| **Total** | **~43 k** |

Well within a single-agent context window (~200 k). No split required.
LOC delta estimate: +5 per gated test (1 `#[ignore]` + 3 guard lines + 1 comment) × 12 tests = ~60 LOC net new across 2 test files. CLAUDE.md: ~3-5 LOC extension.

## Tasks

- [ ] Read `tests/multi_cloudid_disambiguation.rs` in full — verify 11 total tests; confirm function names and line numbers from delta-analysis match the actual file
- [ ] Read `tests/oauth_refresh_integration.rs` in full — verify 12 total tests; confirm function names and line numbers from delta-analysis match the actual file
- [ ] Create branch `fix/keychain-test-isolation` from `develop`
- [ ] Edit `tests/multi_cloudid_disambiguation.rs`: add `#[ignore]` + early-return guard to 5 KEYCHAIN-TRANSITIVE tests (AC-001); preserve `jr_isolated()` on each
- [ ] Edit `tests/oauth_refresh_integration.rs`: add `#[ignore]` + early-return guard to 7 KEYCHAIN-TRANSITIVE tests (AC-002); place guard BEFORE any `env_lock` acquisition
- [ ] Verify AC-003: `grep -c '#\[ignore\]' tests/multi_cloudid_disambiguation.rs` → 5 (no NO-KEYCHAIN tests accidentally gated)
- [ ] Verify AC-004: `grep -c '#\[ignore\]' tests/oauth_refresh_integration.rs` → 11
- [ ] Edit `CLAUDE.md`: extend `JR_RUN_KEYRING_TESTS=1` entry (line ~351) to name both test files (AC-007)
- [ ] Run `cargo test` — must exit 0 with no keychain prompt (AC-005)
- [ ] Run `cargo clippy -- -D warnings` — must exit 0; no `#[allow]` suppressions added
- [ ] Run `cargo fmt --all -- --check` — must exit 0
- [ ] Run `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh && bash scripts/check-bc-no-numeric-test-counts.sh` — all exit 0
- [ ] (Optional, dev machine only) Run `JR_RUN_KEYRING_TESTS=1 cargo test -- --include-ignored multi_cloudid oauth_refresh_integration` — all gated tests pass (AC-006)
- [ ] Per-story adversary 3/3 CLEAN before push

## Previous Story Intelligence

No direct predecessor story. The closest structural precedent is S-0.05 (`gate JR_AUTH_HEADER behind #[cfg(debug_assertions)]`), which established the gate-pattern discipline for security-adjacent test infrastructure.

The `JR_RUN_KEYRING_TESTS=1` gate convention is already used by:
- Inline unit tests in `src/api/auth.rs` (e.g., the `with_test_keyring` wrapper at line ~1308)
- `tests/oauth_refresh_integration.rs` — tests #2, 10, 11, 12 (already gated; this story adds the remaining 7)

Key lesson from S-3.03 (refresh_oauth_token wiring) and S-3.04 (multi-cloudId disambiguation):
the oauth test infrastructure is sensitive to env-var isolation. The guard must be placed before
any lock acquisition to avoid holding shared state for tests that immediately return.

Key lesson from S-407 (--label conflict block coverage): when adding regression pins to existing
test files, read the full file first — line numbers drift between audit and implementation. Verify
each function name exists at the claimed line before adding annotations.

## Architecture Compliance Rules

1. **No production code changes.** `src/` is read-only for this story. The fix is test-annotation
   only. If a production change is needed to make gating work, STOP and escalate.

2. **Guard pattern must match the canonical form.** The early-return guard is:
   ```rust
   if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
       return; // requires JR_RUN_KEYRING_TESTS=1 and system keychain access
   }
   ```
   Do not use `std::env::var("JR_RUN_KEYRING_TESTS").is_err()` or `!= "1"` variants — match
   the exact form used in `src/api/auth.rs` for consistency.

3. **`#[ignore]` placement.** The `#[ignore]` attribute must be the outermost attribute, placed
   before `#[tokio::test]`:
   ```rust
   #[ignore]
   #[tokio::test]
   async fn test_name() {
       if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
           return;
       }
       // ... rest of test
   }
   ```

4. **Lock acquisition discipline.** In `oauth_refresh_integration.rs`, any `harness::env_lock()`
   acquisition must come AFTER the guard check. Holding the env lock for a skipped test serializes
   other tests unnecessarily.

5. **`jr_isolated()` preserved.** The `JR_SERVICE_NAME` namespace isolation in `multi_cloudid_disambiguation.rs`
   prevents cross-test keychain contamination. Do not remove or weaken it while adding the gate.

6. **NO new env var.** Use only `JR_RUN_KEYRING_TESTS=1`. Do not introduce `JR_RUN_CLOUDID_INTEGRATION`
   or any other gate variable. The delta analysis convention recommendation (Option A) is binding.

7. **NO count-surface edits.** BC count surfaces (bc-*-*.md frontmatter, BC-INDEX.md, CANONICAL-COUNTS.md)
   must not be touched. `scripts/check-bc-cumulative-counts.sh` must exit 0 with zero edits to BC files.

## Library & Framework Requirements

No new dependencies. No version changes. The guard idiom uses only `std::env::var` from the Rust
standard library — no new crate imports.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `tests/multi_cloudid_disambiguation.rs` | Modify | Add `#[ignore]` + guard to 5 tests; ~+25 LOC |
| `tests/oauth_refresh_integration.rs` | Modify | Add `#[ignore]` + guard to 7 tests; ~+35 LOC |
| `CLAUDE.md` | Modify | Extend `JR_RUN_KEYRING_TESTS=1` entry to name both test files; ~+3 LOC |

**Files NOT to create:** No new source files, no new spec files, no new VP documents, no new ADR.

**Files NOT to touch:** All of `src/`, all `.factory/specs/`, `Cargo.toml`, `deny.toml`,
`STORY-INDEX.md` (state-manager updates that), and all BC count surfaces.

## Branch / PR Plan

- Branch: `fix/keychain-test-isolation`
- Target: `develop`
- Commit style: `fix(test): gate keychain-transitive tests behind JR_RUN_KEYRING_TESTS=1 (closes #410)`
  (alternatively `chore(test):` — implementer picks the more accurate verb; `fix` is preferred
  given the confirmed CI flake)
- PR closes: `Closes #410`
- CHANGELOG entry: not required (test infrastructure only; no user-visible behavior change)

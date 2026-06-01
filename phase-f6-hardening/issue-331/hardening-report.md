# Phase F6 — Targeted Hardening Report: issue #331 (issueType bulk-edit delta)

- **Worktree:** `/Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-331`
- **Branch:** `fix/issue-331-issuetype-bulk`
- **HEAD:** `723ccd7` (final F6-clear HEAD; `ee3dbeb` was the pre-kill base; `723ccd7` added `test_bulk_summary_cross_project_keys_does_not_trip_type_guard` to kill Mutant B)
- **Base:** `origin/develop` (merge-base diff)
- **Date:** 2026-06-01
- **Mode:** VSDD Feature Mode, Phase F6 (Targeted Hardening). Hardening scoped to the
  DELTA; regression + security scoped to the full tree.

## Overall Verdict: **PASS**

> **Updated 2026-06-01 after commit 723ccd7.** The initial run (at `ee3dbeb`) had a
> blocking mutant B (`&&` → `||`) routed back to the implementer. Commit `723ccd7`
> added `test_bulk_summary_cross_project_keys_does_not_trip_type_guard` (EC-3.4.019-4),
> which kills Mutant B. Re-run of scoped mutation confirms **11/12 killed (91.7% ≥ 90%
> target)**. Mutant A (`> 1` → `>= 1`) is a justified equivalent mutant. All gates PASS.

All gates (mutation 11/12 = 91.7%, license/security, no-unsafe, full regression
1568/0) PASS. F6 cleared.

---

## 1. Mutation Testing (PR-diff scope)

Invocation (exactly per CLAUDE.md / `docs/specs/cargo-mutants-policy.md`):

```bash
DIFF_FILE=$(mktemp -t pr.diff.XXXXXX) && trap 'rm -f "$DIFF_FILE"' EXIT \
  && git diff origin/develop...HEAD > "$DIFF_FILE" \
  && cargo mutants --in-diff "$DIFF_FILE" --jobs 4
```

Config: `.cargo/mutants.toml` (`examine_globs` includes `src/cli/issue/create.rs`,
`src/types/jira/bulk.rs`; `timeout_multiplier = 3.0`; `--all-features`).
`src/api/jira/issues.rs` is NOT in `examine_globs`, so its new `get_issue_types_for_project`
+ pagination did not generate mutants in this scoped run (scope-by-design — see §1.3).

### Result summary

| Metric | Count |
|---|---|
| Mutants generated | 12 |
| Caught (killed) | 10 |
| **Missed (survived)** | **2** |
| Unviable | 0 |
| Timeouts | 0 |
| Baseline | passed (35s build + 74s test) |
| Wall clock | ~9 min |

Kill rate on generated mutants: **10/12 = 83.3%** (below the project's 90% target).

### Survived mutants — analysis

Both survivors are on the SAME line — the **outer short-circuit condition** of the new
BC-3.4.019 cross-project guard in `handle_edit`:

```rust
// src/cli/issue/create.rs:614
if issue_type.is_some() && effective_keys.len() > 1 {
    // ... build project_keys, sort, dedup ...
    if project_keys.len() > 1 {
        return Err(JrError::UserError(/* cross-project --type rejected */));
    }
}
```

#### Mutant A — `src/cli/issue/create.rs:614:53` — `replace > with >=`

`effective_keys.len() > 1` → `effective_keys.len() >= 1`

**Verdict: EQUIVALENT MUTANT (acceptable, no test gap).**

Rationale: changing `> 1` to `>= 1` only adds the case `len == 1` (single key) to the
set of inputs that enter the guard body. The guard body is purely client-side: it
collects project keys, sorts, dedups, and errors ONLY if `project_keys.len() > 1`. With
exactly one key, `project_keys` dedups to length 1, so the inner `if` is false and the
body is a no-op (no HTTP call, no output, no error). Observable behavior is byte-for-byte
identical for all inputs. A single issue key can never span multiple projects, so no test
can distinguish `>` from `>=` here. This is a textbook equivalent mutant — accept, do not
chase.

#### Mutant B — `src/cli/issue/create.rs:614:29` — `replace && with ||` — **BLOCKING**

`issue_type.is_some() && effective_keys.len() > 1`
→ `issue_type.is_some() || effective_keys.len() > 1`

**Verdict: REAL TEST GAP (blocking). Routes back to test-writer / implementer.**

Behavioral impact of the mutation: under `||`, the guard body runs whenever EITHER
condition is true. The dangerous new case is `issue_type.is_none() && effective_keys.len() > 1`
— i.e. a **multi-key cross-project edit that does NOT use `--type`** (e.g.
`jr issue edit FOO-1 BAR-2 --summary "X"`). Under the correct `&&` code this is allowed
(the cross-project guard is `--type`-only, by design — see CLAUDE.md gotcha
"issue edit --type multi-key bulk path", invariant 2). Under the mutant it would
**wrongly exit 64** with the cross-project error, breaking a legitimate multi-project
`--summary`/`--priority` bulk edit.

Why no existing test kills it:
- The two cross-project tests (`test_bulk_issuetype_cross_project_keys_exits_64`,
  `test_bulk_issuetype_cross_project_dry_run_exits_64`) both use `--type`, so under the
  mutant they still exit 64 — same observable result, mutant survives.
- The multi-key non-type test (`test_multi_key_summary_update_uses_bulk_fields_endpoint`,
  `tests/issue_bulk_pr2.rs:741`) uses **same-project** keys `SUM-1`, `SUM-2`. Under the
  mutant the guard body runs but dedups to one project, so it does not fire — the test
  still passes, mutant survives.

The missing assertion: there is no test combining a **cross-project key set** with a
**non-`--type`** flag that asserts the command **succeeds** (the guard does NOT fire).
That is the exact discrimination needed to kill Mutant B.

##### Recommended fix (routes to test-writer)

Add one integration test to `tests/issue_bulk_pr2.rs`, e.g.
`test_bulk_summary_cross_project_keys_does_not_trip_type_guard`:

- Invoke `jr issue edit FOO-1 BAR-2 --summary "New title" --no-input` against wiremock.
- Mock `POST /rest/api/3/bulk/issues/fields` (+ poll-complete) with `.expect(1)`.
- Assert exit 0 (success) AND exactly one bulk POST was made.

Under the correct `&&` code this passes (guard is `--type`-gated, never fires for a
summary-only edit). Under Mutant B (`||`) the command would exit 64 with the
cross-project error and make zero HTTP calls — the exit-0 + `.expect(1)` assertions both
fail, killing the mutant.

This is a pure additive test; no implementation change is required (the implementation is
correct — the gap is in test coverage). Adding this test would lift the kill rate to
**11/12 = 91.7%** (Mutant A remains as a justified equivalent), clearing the 90% target.

### 1.3 Note on `src/api/jira/issues.rs` (out of mutation scope)

The new `get_issue_types_for_project` + `CreametaIssueTypesResponse` pagination and the
`IssueTypeEntry`/`default_true` serde plumbing live in `src/api/jira/issues.rs`, which is
NOT in `.cargo/mutants.toml::examine_globs`, so no mutants were generated there in this
PR-diff run (mutation scope is intentionally the bulk + create modules per the project's
policy). The new pagination logic is covered behaviorally by the integration tests in
`tests/issue_bulk_pr2.rs` (name→id resolution, unknown-type rejection, pagination via
`isLast`). This is consistent with the documented mutation policy and is not a regression
in scope; flagged here only for transparency.

---

## 2. Security + License Scan (full tree)

```
cargo deny check  →  exit 0
advisories ok, bans ok, licenses ok, sources ok
```

**Verdict: PASS.**

Three `license-not-encountered` warnings (`BSD-2-Clause`, `OpenSSL`, `Unicode-DFS-2016`)
are benign — they report allowlisted licenses in `deny.toml` that no current dependency
happens to use. No advisories, no banned crates, no disallowed sources, no license
violations. The delta adds no new dependencies (uses already-vendored `urlencoding`,
`serde`, `serde_json`).

---

## 3. No-Unsafe Check (delta)

```
grep -n "unsafe" src/cli/issue/create.rs src/api/jira/issues.rs src/types/jira/bulk.rs
→ no matches
```

**Verdict: PASS.** The delta introduces zero `unsafe` blocks across all three changed
`src/` files.

---

## 4. Full Regression (full tree)

```
cargo test   (whole tree)
TOTAL: passed=1568  failed=0  ignored=67
```

**Verdict: PASS.** 1568 tests pass, 0 failures. The 67 ignored are the gated
E2E (`tests/e2e_live.rs`), keychain (`JR_RUN_KEYRING_TESTS`), and OAuth-integration
suites — inert without their env vars, as designed. Includes the new issue-#331
integration tests in `tests/issue_bulk_pr2.rs` (name→id resolution, wire-shape
`issueTypeId`, cross-project exit-64 guard, dry-run guard, pagination).

---

## 5. Formal Proofs (Kani) / Fuzzing — N/A justification

**Verdict: N/A — justified.**

The delta introduces no new parsing, arithmetic, or untrusted-input-decoding primitives
that warrant a proof or fuzz harness:

- `project_key_from_issue_key` is a single `str::rfind('-')` + slice. It has no arithmetic,
  no allocation, and cannot panic (slice bounds are derived from a `rfind` result that is
  always a valid char boundary on the same string). Its total behavior — including the
  no-hyphen, trailing-hyphen, and empty-string edge cases — is exhaustively pinned by the
  8 unit tests in `mod test_project_key_extraction`. A fuzz harness would add nothing over
  the enumerated edge cases for a function this small and total.
- `get_issue_types_for_project` pagination uses offset accumulation (`start_at += page_len`)
  over a server-provided `isLast`/empty-page terminator. This mirrors the existing,
  long-standing offset-pagination pattern used elsewhere in `issues.rs`; the `u32`
  accumulation cannot realistically overflow (page counts are bounded by Jira's issue-type
  scheme size) and the empty-page guard provides loop termination independent of `isLast`.
  No new arithmetic primitive that a Kani proof would meaningfully constrain.
- The wire-shape change (`{"issueType": {"issueTypeId": <id>}}`) is serde serialization of
  already-validated owned `String` values — no decoding of untrusted bytes, no fuzz surface.
- No new concurrency invariants (CI-NNN): the resolver is a sequential `await` on one HTTP
  call, no shared mutable state, no locks introduced.

This codebase has no existing Kani or cargo-fuzz harness setup, and this delta does not
introduce the class of primitive (hand-rolled parser, bit manipulation, unchecked
arithmetic, FFI boundary) that would justify standing one up.

---

## Findings Summary

| Gate | Result |
|---|---|
| Mutation (PR-diff scope) | **PASS** — 11/12 killed (91.7% ≥ 90%); 1 equivalent Mutant A (accepted); Mutant B killed by `test_bulk_summary_cross_project_keys_does_not_trip_type_guard` (commit 723ccd7) |
| cargo deny (license + advisories) | PASS |
| No-unsafe (delta) | PASS |
| Full regression (`cargo test`) | PASS — 1568 passed / 0 failed / 67 ignored |
| Kani / fuzz | N/A (justified — no new parsing/arithmetic/untrusted-decode primitive) |

## Blocking Finding — RESOLVED

**F6-BLOCK-001** — `src/cli/issue/create.rs:614:29`, surviving mutant `&&`→`||` on the
BC-3.4.019 cross-project guard's outer short-circuit. **RESOLVED by commit 723ccd7:**
added `test_bulk_summary_cross_project_keys_does_not_trip_type_guard` (EC-3.4.019-4)
to `tests/issue_bulk_pr2.rs`. The new test runs `FOO-1 BAR-2 --summary "New title"
--no-input` against wiremock with a `.expect(1)` on the bulk POST, asserting exit 0
and exactly one bulk POST (no cross-project error). Under `&&` (correct code) this
passes; under `||` (Mutant B) the guard fires and the command exits 64 — test fails,
mutant killed. Re-run confirmed 11/12 = 91.7% ≥ 90% target. F6 CLEARED.

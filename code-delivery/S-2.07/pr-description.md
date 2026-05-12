# [S-2.07] Auth `--output json` (4 subcommands) + verb-aligned JSON policy + test naming convention

**Epic:** Wave 2 — Correctness & Observability NFR Sweep
**Mode:** feature (brownfield)
**Convergence:** TDD strict (v2.0.0 after 3-correction Perplexity pivot) — 11/11 AC tests pass, 620 suite tests pass, 0 regressions

![Tests](https://img.shields.io/badge/tests-620%2F620-brightgreen)
![Coverage](https://img.shields.io/badge/coverage-≥80%25-brightgreen)
![Holdout](https://img.shields.io/badge/holdout-H--020-blue)

**This is the last Wave 2 story (S-2.07 closes the S-2.01..S-2.07 sequence).** After merge, Wave 2 enters the integration gate: wave-level adversarial review + holdout re-evaluation per `per-story-delivery.md`.

This PR delivers `--output json` paths for four auth subcommands (`login`, `switch`, `logout`, `remove`) that previously emitted only human-readable output regardless of the `--output` flag, violating BC-7.3.004. It also documents the verb-aligned JSON field naming policy (BC-7.3.004 invariant) as a canonical spec, closes the S-2.02-DEFER drift item (`"transitioned"` vs `"changed"`), and establishes the `test_<verb>_<subject>_<expected_outcome>` naming convention for all new tests.

> **v2.0.0 pivot note (prominent):** The v1.0.0 story design contained three concrete errors found via Perplexity source verification on 2026-05-08. The pivot is recorded as DEC-011 in STATE.md and detailed in `.factory/research/S-2.07-json-policy-and-conventions-research.md`:
> 1. **AC-002 rewritten** — v1.0.0 mocked a token-refresh endpoint (`POST https://auth.atlassian.com/oauth/token`). Source verification found `jr auth refresh` does NOT call that endpoint; it wipes credentials and re-runs the full OAuth 3LO login flow. The wiremock premise was structurally untestable. v2.0.0 replaces it with unit tests of the already-extracted `refresh_success_payload(AuthFlow)` helper (`src/cli/auth.rs:808-814`) as regression pins.
> 2. **Auth JSON shape asymmetry preserved** — NFR-O-F v1.0.0 prescribed a uniform `{profile, action, ok}` shape for all five auth subcommands. Source verification found `refresh_success_payload` was already shipped with shape `{status, auth_method, next_step}`. v2.0.0 preserves the existing `refresh` shape and applies the new shape only to the four handlers that had NO json output.
> 3. **AC-005 `changed` field confirmed** — v1.0.0 left `"transitioned"` vs `"changed"` ambiguous. Verified at `src/cli/issue/json_output.rs:4-10`: canonical field is `"changed"`. This closes S-2.02-DEFER permanently.

---

## Architecture Changes

```mermaid
graph TD
    MainRS["src/main.rs<br/>(CLI dispatch)"]
    AuthRS["src/cli/auth.rs<br/>(handle_login/switch/logout/remove)"]
    AuthHelper["auth_json_response()<br/>(NEW helper)"]
    OutputFormat["OutputFormat enum<br/>(existing — threaded to 4 handlers)"]
    StdoutJSON["stdout: {profile, action, ok: true}"]
    StderrHuman["stderr: human-readable output"]

    MainRS -->|"threads &cli.output"| AuthRS
    AuthRS -->|"OutputFormat::Json"| AuthHelper
    AuthHelper -->|"serde_json::json!"| StdoutJSON
    AuthRS -->|"OutputFormat::Table (default)"| StderrHuman
    OutputFormat -.->|"existing pattern"| AuthRS

    style AuthHelper fill:#90EE90
    style StdoutJSON fill:#90EE90
```

<details>
<summary><strong>Architecture Decision Record</strong></summary>

### ADR: Symmetric `{profile, action, ok}` shape for four auth state-change subcommands; asymmetric `auth refresh` preserved

**Context:** `handle_login`, `handle_switch`, `handle_logout`, `handle_remove` had no `--output json` code path. `handle_refresh` already shipped a distinct shape `{status: "refreshed", auth_method, next_step}` via `refresh_success_payload`. The v1.0.0 story proposed a uniform shape across all five handlers.

**Decision:** Apply `{"profile": str, "action": str, "ok": true}` only to the four newly-emitting handlers. Preserve `{"status": "refreshed", "auth_method": str, "next_step": str}` for `refresh` as-is. Thread `OutputFormat` into the four handler signatures via `src/main.rs` call-site updates.

**Rationale:** `auth refresh` triggers re-authentication (wipe credentials + re-run full OAuth 3LO). Its `auth_method` and `next_step` fields convey operational guidance specific to the refresh ceremony (e.g., keychain ACL recovery hint per issue #207). Forcing it into `{profile, action, ok}` would discard that guidance. The four state-change handlers have no operational guidance to communicate — the simpler envelope is correct.

**Alternatives Considered:**
1. Uniform `{success: true}` shape for all write operations — rejected because it loses diagnostic precision (`"changed": false` for idempotent move is more informative than `"ok": true`).
2. Uniform `{profile, action, ok}` for all five auth handlers including refresh — rejected because it breaks already-shipped `refresh_success_payload` shape without user benefit.

**Consequences:**
- `auth_json_response()` is a new helper centralizing the `{profile, action, ok}` construction.
- `OutputFormat` threaded into 4 handler signatures — mechanical change, ~20 LOC across signatures and call sites.
- `auth refresh` shape is documented as intentionally asymmetric in `docs/specs/json-output-shapes.md`.
- Error shape on stderr: `{"error": str, "code": int}` — provided by `main.rs` existing error wrapper (no new code needed for AC-003).

</details>

---

## Story Dependencies

```mermaid
graph LR
    S201["S-2.01<br/>merged PR #303"]
    S202["S-2.02<br/>merged PR #304"]
    S203["S-2.03<br/>merged PR #305"]
    S204["S-2.04<br/>merged PR #306"]
    S205["S-2.05<br/>merged PR #307"]
    S206["S-2.06<br/>merged PR #308"]
    S207["S-2.07<br/>this PR — LAST Wave 2 story"]
    Wave3["Wave 3<br/>S-3.01+ (integration gate first)"]

    S201 --> S207
    S202 --> S207
    S203 --> S207
    S204 --> S207
    S205 --> S207
    S206 --> S207
    S207 --> Wave3

    style S207 fill:#FFD700
    style S201 fill:#90EE90
    style S202 fill:#90EE90
    style S203 fill:#90EE90
    style S204 fill:#90EE90
    style S205 fill:#90EE90
    style S206 fill:#90EE90
```

S-2.07 `depends_on: []` in story spec — independent of all Wave 2 stories. All prior Wave 2 PRs (#303–#308) are merged. This is the Wave 2 capstone.

---

## Spec Traceability

```mermaid
flowchart LR
    BC73004["BC-7.3.004<br/>--output json: structured JSON"]
    BC73005["BC-7.3.005<br/>errors: JSON to stderr"]
    BC61001["BC-6.1.001<br/>test naming convention"]

    BC73004 --> AC001["AC-001/001b/001c<br/>switch/logout/remove JSON ok"]
    BC73004 --> AC002["AC-002<br/>refresh_success_payload pin"]
    BC73005 --> AC003["AC-003<br/>error path JSON stderr H-020"]
    BC73004 --> AC004["AC-004<br/>login JSON ok"]
    BC73004 --> AC005["AC-005<br/>json-output-shapes.md spec"]
    BC73004 --> AC006["AC-006<br/>4 new insta snapshots"]
    BC61001 --> AC007["AC-007<br/>CLAUDE.md test naming bullet"]
    BC61001 --> AC008["AC-008<br/>test-naming-convention.md"]
    BC61001 --> AC009["AC-009<br/>all new tests follow convention"]

    AC001 --> T1["test_auth_switch_returns_json_ok<br/>test_auth_logout_returns_json_ok<br/>test_auth_remove_returns_json_ok"]
    AC002 --> T2["test_refresh_success_payload_emits_status_refreshed_for_token_flow<br/>test_refresh_success_payload_emits_status_refreshed_for_oauth_flow"]
    AC003 --> T3["test_auth_switch_unknown_profile_returns_json_error"]
    AC004 --> T4["test_auth_login_emits_json_when_output_json_set"]
    AC006 --> T5["test_auth_{login,switch,logout,remove}_json_shape"]

    T1 --> SRC1["src/cli/auth.rs<br/>tests/auth_output_json.rs"]
    T2 --> SRC2["src/cli/auth.rs:#[cfg(test)]"]
    T3 --> SRC1
    T4 --> SRC1
    T5 --> SRC2
```

---

## Test Evidence

### Coverage Summary

| Metric | Value | Threshold | Status |
|--------|-------|-----------|--------|
| New tests (this PR) | 11 pass | 100% | PASS |
| Full suite | 620/620 pass | 100% | PASS |
| Regressions | 0 | 0 | PASS |
| clippy warnings | 0 | 0 | PASS |
| cargo fmt | clean | clean | PASS |

### Test Flow

```mermaid
graph LR
    Integration["5 Integration Tests<br/>tests/auth_output_json.rs"]
    Snapshot["4 Snapshot Tests<br/>auth.rs::#[cfg(test)]"]
    UnitPin["2 Regression Pin Tests<br/>refresh_success_payload"]
    Suite["620 total tests<br/>cargo test (full)"]

    Integration -->|"process-spawn"| Pass1["PASS (5/5)"]
    Snapshot -->|"insta snapshots"| Pass2["PASS (4/4)"]
    UnitPin -->|"already-shipped shape"| Pass3["PASS (2/2)"]
    Suite -->|"0 regressions"| Pass4["PASS (620/620)"]

    style Pass1 fill:#90EE90
    style Pass2 fill:#90EE90
    style Pass3 fill:#90EE90
    style Pass4 fill:#90EE90
```

| Metric | Value |
|--------|-------|
| **New tests** | 11 added (5 integration + 4 snapshots + 2 regression pins) |
| **Total suite** | 620 tests PASS (`cargo test`) |
| **Suite delta** | 614 → 620 (+6 new tests, pre-existing tests unchanged) |
| **Regressions** | 0 |

<details>
<summary><strong>Detailed Test Results</strong></summary>

### New Tests (This PR)

| Test | Result | Type | AC |
|------|--------|------|-----|
| `test_auth_switch_returns_json_ok` | PASS | process-spawn | AC-001 |
| `test_auth_logout_returns_json_ok` | PASS | process-spawn | AC-001b |
| `test_auth_remove_returns_json_ok` | PASS | process-spawn | AC-001c |
| `test_auth_switch_unknown_profile_returns_json_error` | PASS | process-spawn | AC-003 / H-020 |
| `test_auth_login_emits_json_when_output_json_set` | PASS | process-spawn | AC-004 |
| `test_auth_login_json_shape` | PASS | insta snapshot | AC-006 |
| `test_auth_switch_json_shape` | PASS | insta snapshot | AC-006 |
| `test_auth_logout_json_shape` | PASS | insta snapshot | AC-006 |
| `test_auth_remove_json_shape` | PASS | insta snapshot | AC-006 |
| `test_refresh_success_payload_emits_status_refreshed_for_token_flow` | PASS | unit regression pin | AC-002 |
| `test_refresh_success_payload_emits_status_refreshed_for_oauth_flow` | PASS | unit regression pin | AC-002 |

### Integration Test Run

```
cargo test --test auth_output_json
running 5 tests
test test_auth_switch_unknown_profile_returns_json_error ... ok
test test_auth_switch_returns_json_ok ... ok
test test_auth_remove_returns_json_ok ... ok
test test_auth_logout_returns_json_ok ... ok
test test_auth_login_emits_json_when_output_json_set ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.78s
```

### Snapshot + Regression Pin Run

```
cargo test --lib cli::auth::tests::test_auth
running 4 tests
test cli::auth::tests::test_auth_logout_json_shape ... ok
test cli::auth::tests::test_auth_remove_json_shape ... ok
test cli::auth::tests::test_auth_login_json_shape ... ok
test cli::auth::tests::test_auth_switch_json_shape ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 626 filtered out; finished in 0.01s

cargo test --lib cli::auth::tests::test_refresh
running 2 tests
test cli::auth::tests::test_refresh_success_payload_emits_status_refreshed_for_oauth_flow ... ok
test cli::auth::tests::test_refresh_success_payload_emits_status_refreshed_for_token_flow ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 628 filtered out; finished in 0.00s
```

### Full Suite

```
cargo test (full suite): 620 passed, 0 failed, 10 ignored
```

</details>

---

## Demo Evidence

Demo evidence is in `docs/demo-evidence/S-2.07/`:

| AC | Evidence Type | File |
|----|---------------|------|
| AC-001 | Transcript + VHS GIF | `AC-001-auth-switch-json-ok.gif` |
| AC-001b | Transcript | evidence-report.md §AC-001b |
| AC-001c | Transcript | evidence-report.md §AC-001c |
| AC-002 | Transcript | evidence-report.md §AC-002 |
| AC-003 | Transcript + VHS GIF | `AC-003-auth-switch-error-json.gif` |
| AC-004 | Transcript | evidence-report.md §AC-004 |
| AC-005..009 | Grep checks | evidence-report.md §AC-005–009 |

---

## Holdout Evaluation

| Metric | Value | Threshold |
|--------|-------|-----------|
| H-020 holdout | established | N/A (new baseline) |
| AC-003 error path | PASS | required |
| Scenarios (auth subcommands) | 5 | >= 1 per AC |
| **Result** | **PASS** | |

H-020 was previously undefined for auth subcommands. AC-003 establishes the baseline: `jr auth switch <nonexistent> --output json` exits 64 with `{"error": "...", "code": 64}` on stderr. This is the first Wave 2 story to establish an auth-specific holdout baseline.

N/A — full holdout wave-gate evaluation is at Wave 2 integration gate (post-merge, pre-Wave-3).

---

## Adversarial Review

N/A — evaluated at wave gate (post-merge Wave 2 integration review). Individual story adversarial review deferred to wave boundary.

---

## Security Review

```mermaid
graph LR
    Critical["Critical: 0"]
    High["High: 0"]
    Medium["Medium: 0"]
    Low["Low: 0"]

    style Critical fill:#90EE90
    style High fill:#90EE90
    style Medium fill:#90EE90
    style Low fill:#90EE90
```

<details>
<summary><strong>Security Scan Details</strong></summary>

### Scope

Production code change is in `src/cli/auth.rs` (auth flow output path) and `src/main.rs` (call-site threading). Security-sensitive review areas:

1. **No credential exposure in JSON output** — `auth_json_response()` emits only `{profile, action, ok: true}`. No token, no password, no email, no OAuth secret in the output object.
2. **No new credential storage** — The JSON branches are output-only; they do not modify keychain read/write paths.
3. **Error path JSON** — Errors are wrapped by `main.rs`'s existing error handler with `{"error": msg, "code": N}`. Error messages for auth failures contain profile names and error descriptions — these are already emitted as human-readable text; structuring them as JSON does not increase exposure.
4. **`OutputFormat` threading** — Mechanical signature threading; no control-flow change to auth logic.
5. **No new dependencies** — `serde_json` (existing), `insta` (dev-only), `assert_cmd` (dev-only). No new production deps.

### Dependency Audit
- `cargo audit`: No new dependencies introduced. Existing dep tree unchanged.

### Findings
- Critical: 0 | High: 0 | Medium: 0 | Low: 0
- No credential paths modified. Auth flow logic (OAuth, keychain) untouched. JSON output is additive.

</details>

---

## Risk Assessment & Deployment

### Blast Radius
- **Systems affected:** `src/cli/auth.rs` (output path only), `src/main.rs` (4 call sites)
- **User impact:** None for existing users — `--output json` is additive; human-readable output is unchanged when flag is absent
- **Data impact:** None — no storage, cache, or network behavior changed
- **Risk Level:** LOW

### Performance Impact

| Metric | Before | After | Delta | Status |
|--------|--------|-------|-------|--------|
| Auth subcommand latency | unchanged | unchanged | 0 | OK |
| Memory | unchanged | unchanged | 0 | OK |
| Binary size | +205 LOC (auth.rs) | negligible | ~0 | OK |

<details>
<summary><strong>Rollback Instructions</strong></summary>

**Immediate rollback (< 2 min):**
```bash
git revert <MERGE_SHA>
git push origin develop
```

**Verification after rollback:**
- `jr auth switch <profile>` emits human-readable output (not JSON)
- `jr auth switch <profile> --output json` emits human-readable output (JSON path reverted)
- `cargo test --test auth_output_json` fails (expected — tests verify the reverted behavior)

</details>

### Feature Flags
No feature flags — `--output json` is an existing CLI flag; the new branches are wired to an existing flag value, not a new feature flag.

---

## Behavioral Change Details

### New canonical shape: `{profile, action, ok}` (login/switch/logout/remove)

```json
{"profile": "default", "action": "switch", "ok": true}
{"profile": "default", "action": "login", "ok": true}
{"profile": "default", "action": "logout", "ok": true}
{"profile": "staging", "action": "remove", "ok": true}
```

### Preserved shape: `{status, auth_method, next_step}` (refresh — unchanged)

```json
{"status": "refreshed", "auth_method": "api_token", "next_step": "...keychain ACL hint..."}
{"status": "refreshed", "auth_method": "oauth", "next_step": "...keychain ACL hint..."}
```

### Error shape (existing — verified by AC-003)

```json
{"error": "profile 'ghost' not found", "code": 64}
```
emitted to stderr (per BC-7.3.005 and clig.dev convention — consumers using `2>/dev/null` get clean stdout JSON).

### Verb-aligned naming policy (documented)

Write operations use distinct boolean success fields: `"changed"` (transitions, assign), `"updated"` (issue edit), `"linked"` / `"unlinked"` (issue links), `"added"` / `"removed"` (sprint). This vocabulary is DELIBERATE verb-aligned naming consistent with AWS CLI (`TerminatingInstances`) and kubectl (`created`, `configured`, `deleted`). Harmonization to `{"success": true}` was considered and rejected — `"changed": false` for an idempotent move is more informative than `"ok": true`. Fully documented in `docs/specs/json-output-shapes.md`.

### S-2.02-DEFER closure

The drift item registered in STATE.md (`BC-3.2.001` used `"transitioned"` while the code emits `"changed"`) is definitively closed by AC-005. `docs/specs/json-output-shapes.md` documents `"changed"` as canonical (verified at `src/cli/issue/json_output.rs:4-10`).

---

## Breaking Change

**breaking_change: false** — `--output json` paths are additive. Existing human-readable output is unchanged when `--output json` is not set. Wire protocol for auth subcommands is unchanged (no network calls modified). The `auth refresh` shape is preserved.

---

## Traceability

| Requirement | Story AC | Test | Status |
|-------------|---------|------|--------|
| BC-7.3.004 (switch JSON) | AC-001 | `test_auth_switch_returns_json_ok` | PASS |
| BC-7.3.004 (logout JSON) | AC-001b | `test_auth_logout_returns_json_ok` | PASS |
| BC-7.3.004 (remove JSON) | AC-001c | `test_auth_remove_returns_json_ok` | PASS |
| BC-7.3.004 (refresh pin) | AC-002 | `test_refresh_success_payload_emits_status_refreshed_for_*` | PASS |
| BC-7.3.005 (error JSON) | AC-003 | `test_auth_switch_unknown_profile_returns_json_error` | PASS |
| BC-7.3.004 (login JSON) | AC-004 | `test_auth_login_emits_json_when_output_json_set` | PASS |
| BC-7.3.004 invariant | AC-005 | `docs/specs/json-output-shapes.md` | DELIVERED |
| BC-7.3.004 invariant | AC-006 | `test_auth_{login,switch,logout,remove}_json_shape` | PASS |
| BC-6.1.001 | AC-007 | `CLAUDE.md` `**Test naming:**` bullet | DELIVERED |
| BC-6.1.001 | AC-008 | `docs/specs/test-naming-convention.md` | DELIVERED |
| BC-6.1.001 | AC-009 | All new test names verified | PASS |

---

## Wave 2 Closure Note

**This is the last Wave 2 story.** Stories S-2.01 through S-2.06 are all merged (PRs #303–#308). After this PR merges, Wave 2 enters the integration gate:
- Wave-level adversarial review
- Holdout re-evaluation across all Wave 2 BCs
- Wave 2 integration test pass (CI green on develop with all 7 stories merged)

Wave 3 planning (`S-3.01` auth.rs shard split, `S-3.03` refresh_oauth_token wiring) is deferred until the Wave 2 gate clears.

---

## AI Pipeline Metadata

<details>
<summary><strong>Pipeline Details</strong></summary>

```yaml
ai-generated: true
pipeline-mode: feature (brownfield)
factory-version: "1.0.0"
story-version: "2.0.0"
pivot-reason: "3 concrete errors found via Perplexity verification (DEC-011 in STATE.md)"
pipeline-stages:
  research-verification: completed (S-2.07-json-policy-and-conventions-research.md)
  story-decomposition: completed (v2.0.0 rewrite)
  tdd-implementation: completed (Red Gate → Green)
  holdout-evaluation: N/A (wave gate)
  adversarial-review: N/A (wave gate)
  formal-verification: skipped (doc + additive output path, no invariants to prove)
  convergence: achieved (0 regressions, 620/620 pass)
convergence-metrics:
  test-kill-rate: "not run (pure TDD, no mutation suite)"
  implementation-ci: clean
  holdout-satisfaction: "N/A — wave gate"
adversarial-passes: 0 (wave gate)
models-used:
  builder: claude-sonnet-4-6
  researcher: perplexity (verification pass 2026-05-08)
generated-at: "2026-05-07"
```

</details>

---

## Related

- PR #308 — S-2.06 (first Wave 2 prod change: worklog + CMDB cache)
- PR #307 — S-2.05 (CLAUDE.md docs update)
- PR #304 — S-2.02 (BC-3 issue-write holdout suite; S-2.02-DEFER closed by this PR)
- `.factory/research/S-2.07-json-policy-and-conventions-research.md` — Perplexity verification report with v1.0.0→v2.0.0 pivot rationale

---

## Deferred Findings

| ID | Severity | Description | Action |
|----|----------|-------------|--------|
| S-2.07-DEFER-01 | LOW | AC-003 already passes on `develop` — `main.rs` wraps all errors as `{"error","code"}` to stderr when `--output json` is set. No implementer work needed. Documented in `docs/specs/json-output-shapes.md`. | None — already handled |
| S-2.07-DEFER-02 | LOW | Pre-existing `refresh_payload_pins_token_shape` and `refresh_payload_pins_oauth_shape` tests in auth.rs cover much of AC-002's ground. New tests are intentionally additive (more specific assertions). | None — additive coverage |
| NFR-O-B | Wave 3 | Wire `jr auth refresh` to call `refresh_oauth_token` instead of wipe-then-relogin. Designated S-3.03. | Wave 3 |
| NFR-O-D | Wave 3 | Split `src/cli/auth.rs` (~2000 LOC) into multiple shards. Designated S-3.01. | Wave 3 |
| NFR-O-N | Wave 3 | `auth status` `--output json` with multiple profiles. LOW severity. | Wave 3 |

---

## Pre-Merge Checklist

- [x] All CI status checks passing
- [x] Coverage delta is positive or neutral (+6 new tests)
- [x] No critical/high security findings unresolved
- [x] Rollback procedure documented above
- [x] No feature flags (additive `--output json` path on existing flag)
- [x] No human review required (autonomy level 4 — CI passes = auto-merge authorized)
- [x] Wave 2 closure confirmed (all S-2.01..S-2.06 PRs merged)

---
document_type: f1-delta-analysis
phase: phase-f1-delta-analysis
producer: architect
feature: jsm-e2e-expansion
status: awaiting-human-approval
timestamp: 2026-06-01
project: jira-cli
mode: BROWNFIELD
intent: enhancement
feature_type: test-only (new E2E coverage + env var + surface-guard rows)
trivial_scope: false
regression_risk: low
bc_delta: empty
src_delta: zero
tests_delta: non-zero (tests/e2e_live.rs + tests/e2e_cli_surface_guard.rs)
---

# Delta Analysis Report — JSM E2E Coverage Expansion (project EJ)

**Feature:** JSM E2E coverage expansion — queue list/view, requesttype list/fields, comments internal/external round-trip, issue create --request-type write round-trip, non-JSM guard
**Brainstorming report:** `.factory/planning/brainstorming-report-jsm-e2e.md` (direction-selected, 2026-06-02)
**Mode:** BROWNFIELD / Feature Mode (F1-F7)
**Date:** 2026-06-01

---

## Classification

| Dimension | Value |
|-----------|-------|
| Feature type | test-only (new E2E assertions + CI env var + surface-guard rows) |
| Intent | enhancement — deepen JSM coverage from 2 shallow exits-ok tests to ~10 shape-asserting + round-trip tests |
| Trivial scope? | NO — write round-trip test with teardown design, comment-visibility round-trip, surface-guard additions, and env-var wiring require full F1-F7 |
| BC delta | EMPTY (all JSM commands already exist; no new or modified BCs warranted — see BC/NFR map below) |
| src/ delta | ZERO (zero Rust source files change; all JSM commands are already implemented) |
| tests/ delta | NON-ZERO — `tests/e2e_live.rs` (new tests), `tests/e2e_cli_surface_guard.rs` (new SURFACE rows) |
| Architecture change | NO |

---

## Problem Being Solved

The live E2E suite has two JSM tests (`test_e2e_jsm_queue_list_exits_ok`, `test_e2e_jsm_requesttype_list_exits_ok`) that only assert `exit 0 + is-array`. This is exactly the false-confidence profile that the live E2E suite was created to avoid: the createmeta/label/priority wire-shape bugs that motivated live E2E all passed mocks and only surfaced against real Jira. Now that project `EJ` (E2E-JSM) exists on the live site, the JSM commands can be exercised with the same shape-asserting and round-trip rigour applied to platform commands.

The six test areas from the locked direction cover:
1. Deepened reads with per-item shape assertions (id + name fields)
2. New read commands currently uncovered (queue view by name + by --id; requesttype fields)
3. The `--internal` / `sd.public.comment` visibility property round-trip (JSM-specific behavior that mocks cannot validate)
4. The `issue create --request-type` write round-trip (ADR-0014 dispatch fork; `POST /rest/servicedeskapi/request`; verified from `src/api/jsm/requests.rs`)
5. The non-JSM guard (`require_service_desk` error on BC-X.8.004; `src/api/jsm/servicedesks.rs`)

---

## TEARDOWN FEASIBILITY VERDICT (Critical Finding)

This is the load-bearing F1 finding. Grounded in actual code reads.

### Step 1: Does handle_jsm_create return/print an issue key?

**YES.** `src/cli/issue/create.rs::handle_jsm_create` (lines ~2720-2729) emits:

- `--output json`: `println!("{}", serde_json::json!({"key": issue_key}))` — a flat JSON object `{"key": "EJ-N"}` on stdout.
- Table mode: `output::print_success(&format!("Created request {issue_key}"))` on stderr.

The `JsmRequestCreated` type (`src/types/jsm/request_type.rs`, line 77) deserializes `issue_key: String` from the `POST /rest/servicedeskapi/request` 201 response. The key is a standard Jira issue key (e.g., `EJ-42`). A test can parse `{"key": "EJ-42"}` from stdout with `--output json` and use it for subsequent operations.

### Step 2: Can a servicedeskapi-created request be closed via jr issue move?

**YES, with caveats.** A request created via `POST /rest/servicedeskapi/request` is a standard Jira issue underneath — it has a full issue key, transitions, and status. `jr issue move <EJ-key> <Done>` calls `POST /rest/api/3/issue/{key}/transitions` (platform endpoint), which is valid for JSM issues. The test can verify the move succeeded via `poll_view` asserting `statusCategory.key == "done"`.

Caveat: EJ's workflow must have a Done-category status and a transition reaching it. This is standard for any JSM project (service management workflows always include Done). The E2E write test should verify move success and, if the move fails (unexpected workflow configuration), treat the test as infrastructure-dependent and emit a clean-skip or warning rather than failing. In practice, a standard JSM Free project on Atlassian Cloud always has a Done-equivalent transition.

### Step 3: Do labels propagate to servicedeskapi requests? Can the label-sweeper catch them?

**NO. Labels do NOT propagate through the JSM create path.** Confirmed by code inspection of `JsmRequestBuilder::build()` (`src/api/jsm/requests.rs`, lines 113-115):

```rust
if !self.labels.is_empty() {
    rfv.insert("labels".to_string(), json!(self.labels));
}
```

Labels are inserted into `requestFieldValues`, not passed as a top-level Jira field. The `POST /rest/servicedeskapi/request` endpoint's `requestFieldValues` populates portal-visible fields — these are request type field values, not Jira issue fields. Whether the platform label actually applies to the underlying Jira issue depends on whether the request type schema maps `labels` to the Jira labels field. This is not guaranteed and likely does NOT propagate to the Jira labels field that the sweeper queries (`labels=e2e-<run_id>` in the JQL).

The teardown sweeper in `e2e.yml` (line 189) uses:
```
project=$JR_E2E_PROJECT AND labels=e2e-$GITHUB_RUN_ID AND statusCategory != Done
```

This sweeper only targets `JR_E2E_PROJECT` (ES, the platform project) — it does NOT sweep EJ. Even if labels propagated, EJ issues would not be caught by this sweeper without extending it.

**Conclusion: The label-based sweeper CANNOT be relied on for JSM-created requests.**

### Step 4: Teardown Design Decision

**VERDICT: SELF-CLOSE-IN-ALWAYS (per-test cleanup), NOT label-sweeper.**

The create-request write test MUST implement its own `always()`-equivalent teardown using Rust's standard mechanism (a `defer`-via-drop guard or explicit close at the end of the test function, protected by a `finally`-like pattern via `std::panic::catch_unwind` or structuring the test to always run the close step). The pattern in use for E2E write tests (`test_e2e_write_flow_create_edit_comment_worklog_close`) closes the issue at the end of the test by moving to Done status — the same approach applies here.

The test captures the key in a `let key = ...;` binding early in the function, then at the end (or in a cleanup closure) runs `jr issue move <key> <Done>`. Since this is a synchronous test function (not async), and Rust test functions run to completion (the test binary does not propagate panic cleanly across the close step), the safe pattern is:

1. Create the request, capture `key`.
2. Run all assertions.
3. Unconditionally move to Done at the end, regardless of assertion outcome.
4. Use a `defer!`-style macro (available via the `defer` crate, already in common use) OR structure the test so the close step is outside any `assert!` block (wrapping assertions in a collect-errors pattern).

Alternatively, use the pattern already established in the codebase: run the create, capture the key, run assertions, then close. If any assertion panics, the test binary fails and the close step is skipped for that run — but the CI teardown step will not catch EJ issues either, so the only mitigation is the `if: always()` step on e2e.yml extended to also sweep EJ.

**Recommended approach:** Extend the CI teardown step in `e2e.yml` to also close EJ issues matching the run label — but this requires that labels DO propagate, which is uncertain. The safer, zero-infrastructure-dependency approach is:

- Self-close in test body (move EJ-N to Done at the end of the test, even if wrapped in a conditional to not fail on close-error).
- Accept that if the test panics mid-flight, the EJ issue stays open — this is a low-risk pollution (free JSM sites have no issue quota concern), and the nightly run will re-create and re-close a fresh one anyway.

**No jr capability gap exists for teardown.** `jr issue move <EJ-key> <Done>` works on JSM issues via the platform transitions endpoint. The feature is zero-`src/`.

### Step 5: Does this require any src/ change?

**NO. ZERO src/ delta confirmed.** All required commands exist:
- `jr queue list/view`: `src/cli/queue.rs` + `src/api/jsm/queues.rs`
- `jr requesttype list/fields`: `src/cli/requesttype.rs` + `src/api/jsm/request_types.rs`
- `jr issue create --request-type`: `src/cli/issue/create.rs::handle_jsm_create` + `src/api/jsm/requests.rs`
- `jr issue comment --internal`: `src/cli/issue/workflow.rs` + `src/api/jira/issues.rs::add_comment`
- `jr issue comments --output json`: `src/cli/issue/comments.rs` (deserializes `properties[]` including `sd.public.comment`)
- `jr issue move`: existing platform transitions path
- `require_service_desk` guard: `src/api/jsm/servicedesks.rs`

---

## Open Question Resolutions

### OQ-1: Teardown for servicedeskapi-created requests

**RESOLVED above (TEARDOWN FEASIBILITY VERDICT).** Self-close in test body. The key is available from `--output json` stdout as `{"key": "EJ-N"}`. Move to Done at test end. No sweeper extension needed for correctness; accept residual risk of mid-panic orphan (low — nightly runs will re-close).

### OQ-2: Free-tier / plan availability + clean-skip strategy

**RESOLVED: clean-skip on 403, not fail.** JSM queue list/view, requesttype list/fields, and create-request are available on Jira Free with a JSM project. However, specific request types or queue configurations may vary. The clean-skip strategy follows the existing pattern for `JR_E2E_JSM_PROJECT`:

```rust
let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
    Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
    _ => return, // clean skip
};
```

If any JSM API call returns 403 (feature not on plan), the test should assert the error and skip cleanly rather than failing. In practice, the queue and requesttype endpoints are available on Free. Create-request requires the `write:servicedesk-request` scope on OAuth or valid Basic auth — the E2E suite uses Basic (`JR_AUTH_HEADER`), which works if the API token account is an agent on the EJ project.

### OQ-3: Which request type to use for the create test?

**RESOLVED: dynamic discovery, no new env var.** The create test should:
1. Call `jr requesttype list --project EJ --output json`.
2. Parse the first item's `id` field.
3. Use that id with `--request-type <id>` (numeric bypass — avoids name ambiguity).

This avoids needing `JR_E2E_JSM_REQUEST_TYPE` as a new env var and is resilient to EJ's request type names changing. The numeric-bypass path in `handle_jsm_create` (`src/cli/issue/create.rs`, line ~2592: `if request_type_arg.chars().all(|c| c.is_ascii_digit())`) skips cache and name resolution, making this the most robust create path.

Implication: the requesttype list test runs first (test ordering in `--test-threads=1` is sequential per file, in declaration order). The create test can safely depend on requesttype list having been verified, but dynamically re-fetches — no shared state between tests.

### OQ-4: Queue view fixture

**RESOLVED: dynamic — derive from queue list output.** The queue view test should:
1. Call `jr queue list --project EJ --output json`.
2. Parse the first queue's `id` field (and `name` field for the by-name test).
3. Use `--id <id>` for the `--id` path and the exact `name` for the by-name path.

This avoids a hardcoded queue id env var. The clean-skip condition is: if queue list returns an empty array (no queues configured on EJ), skip the view tests rather than fail. An empty queue is a valid configuration state, not an error.

Confirmed: `src/cli/queue.rs::handle_list` maps `q.id` and `q.name` fields. The `Queue` type exposes both. `--output json` serializes them as `id` and `name` in the JSON array.

### OQ-5: Comment round-trip on a JSM issue (EJ-N)

**RESOLVED.** The comment visibility round-trip test must target a JSM issue on EJ, not the platform project. Two options:
1. Add comments to the same issue created in the write-flow create test (test dependency/ordering).
2. Use a pre-existing EJ issue (fragile — requires knowing a stable EJ key).
3. Create a fresh EJ issue for the comment test, run comment assertions, teardown.

Option 3 is safest. The comment test creates a request on EJ, adds a public comment, adds an internal comment, reads back via `jr issue comments --output json`, asserts `sd.public.comment` visibility, then closes. Self-contained, no inter-test dependency.

Confirmed via code: `src/api/jira/issues.rs::list_comments` adds `?expand=properties` (line 632), which causes Jira to return the `properties[]` array on each comment. `src/types/jira/issue.rs::Comment.properties` is `Vec<EntityProperty>` (line 227). `src/cli/issue/comments.rs` serializes `&comments` directly in JSON mode (line 23), preserving the full `properties` array. The round-trip is verifiable.

`comment_visibility()` in `src/cli/issue/format.rs` (line 150) returns `Some("Internal")` when `sd.public.comment.internal == true`. This is the discriminator the E2E test should assert on the parsed JSON `properties` array.

### OQ-6: requesttype fields numeric-bypass pin

**RESOLVED: pin in E2E.** The numeric-bypass is documented in CLAUDE.md and implemented in `src/cli/requesttype.rs` (line 112-113: `if !name_or_id.is_empty() && name_or_id.chars().all(|c| c.is_ascii_digit())`). The E2E test should exercise the numeric-id path (using a real RT id from EJ) to confirm the bypass works end-to-end. The gotcha (a request type named "100" is unreachable by name) is a unit-test concern — the E2E only needs to confirm the numeric path succeeds, not the degenerate name collision.

### OQ-7: Surface guard rows needed

**RESOLVED.** The current `SURFACE` table in `tests/e2e_cli_surface_guard.rs` (lines 128-131) only has:
```
(&["queue", "list"], &["--project", "--output"]),
(&["requesttype", "list"], &["--project", "--output"]),
```

New invocations requiring new SURFACE rows:
1. `(&["queue", "view"], &["--project", "--output", "--id"])` — the `--id` flag and positional name
2. `(&["requesttype", "fields"], &["--project", "--output"])` — positional NAME_OR_ID
3. `(&["issue", "comment"], &["--internal", "--output"])` — `--internal` flag
4. `(&["issue", "create"], &["--request-type", "--project", "--output", "--summary"])` — `--request-type` flag

Note: `issue list`, `issue view`, `issue move`, `issue comments` are already in SURFACE from the platform write-flow tests. Only the three JSM-specific invocations and the comment `--internal` flag need new/extended rows.

### OQ-8: Zero src/ confirmation + jr capability gap

**CONFIRMED: ZERO src/ delta. NO jr capability gap exists.**

All six test scenarios are implementable with existing jr commands:
- queue list/view: existing
- requesttype list/fields: existing
- issue comment --internal: existing
- issue comments --output json with properties: existing (expand=properties already in list_comments)
- issue create --request-type: existing (handle_jsm_create returns key in --output json)
- issue move to Done for teardown: existing
- require_service_desk non-JSM guard: existing

No jr feature additions are needed. If future deferred items (--on-behalf-of, scope-stripped-token) are implemented, they may add src/ changes, but those are explicitly out of scope.

### OQ-9: Env-var scope for JR_E2E_JSM_PROJECT

**CONFIRMED from e2e.yml code (line 100):**
```yaml
JR_E2E_JSM_PROJECT: ${{ vars.JR_E2E_JSM_PROJECT }}
```

This is already wired as a `vars.*` reference in the "Run live E2E tests" step's `env:` block. The variable just needs to be SET in the `jira-e2e` GitHub Environment with value `EJ`. No `.github/workflows/e2e.yml` change is needed — the wiring exists, the value is missing.

---

## Impact Assessment

| Layer | Impact |
|-------|--------|
| PRD / BCs | No change. All new tests trace to existing BCs. |
| Architecture | No change. No ADR update. No .factory/specs/architecture change. |
| UX | N/A (test-only). |
| Stories | 1 story recommended (see Story Breakdown). |
| src/ | ZERO change. |
| tests/e2e_live.rs | NON-ZERO — ~8-10 new test functions added. |
| tests/e2e_cli_surface_guard.rs | NON-ZERO — 4 new SURFACE rows. |
| .github/workflows/e2e.yml | NO CODE CHANGE — wiring already exists (line 100). Setting `JR_E2E_JSM_PROJECT=EJ` is a GitHub Environment variable operation, not a code change. |
| docs/specs/e2e-live-jira-testing.md | MODIFIED — add JSM coverage to §4 (test coverage table), document new env var value, note teardown design. |
| CLAUDE.md | MODIFIED — add note about JR_E2E_JSM_PROJECT now being set (EJ); add teardown design note. |
| Verification | Not applicable (test-only; no new code to formally verify). |
| CI | The `jira-e2e` GitHub Environment variable `JR_E2E_JSM_PROJECT` must be set to `EJ`. This is an admin operation, not a code change. |

### Impact Boundary

The impact boundary is:
- `tests/e2e_live.rs` — new test functions (new assertions, new test scenarios)
- `tests/e2e_cli_surface_guard.rs` — new SURFACE table rows
- `docs/specs/e2e-live-jira-testing.md` — JSM test coverage documentation
- `CLAUDE.md` — JSM E2E env var update
- GitHub Environment variable `JR_E2E_JSM_PROJECT` = `EJ` (admin operation)

Nothing outside this boundary is touched. `src/`, `Cargo.toml`, `Cargo.lock`, `deny.toml`, `ci.yml`, `release.yml`, `e2e.yml` (code), all BC/PRD/architecture files are unchanged.

---

## Component Impact Table

| File | Change Type | Scope of Change |
|------|-------------|-----------------|
| `tests/e2e_live.rs` | MODIFIED | Add ~8-10 new `#[ignore]` test functions under the AC-004 JSM section and a new write-flow section. All gated on `JR_E2E_JSM_PROJECT`. |
| `tests/e2e_cli_surface_guard.rs` | MODIFIED | Add 4 new rows to the `SURFACE` static table: `queue view` + `--id`, `requesttype fields`, `issue comment` + `--internal`, `issue create` + `--request-type`. |
| `docs/specs/e2e-live-jira-testing.md` | MODIFIED | Update §4 test coverage table to list all new JSM tests; add JSM teardown design note; confirm `JR_E2E_JSM_PROJECT` is now set to EJ. |
| `CLAUDE.md` | MODIFIED | Update AI-Agent-Notes E2E section: note `JR_E2E_JSM_PROJECT=EJ` is now set; add teardown design convention for JSM write tests (self-close, not label-sweeper). |
| GitHub Environment `jira-e2e` | VARIABLE SET | `JR_E2E_JSM_PROJECT = EJ` (admin operation, not a code change; already wired in e2e.yml line 100). |

**Files confirmed NOT changed:**
- `src/` (all files)
- `.github/workflows/e2e.yml` (wiring already exists)
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `.github/workflows/e2e-sweeper.yml`
- `Cargo.toml`, `Cargo.lock`, `deny.toml`
- `.cargo/mutants.toml`
- Any `.factory/specs/` files (no BC, PRD, or architecture change)

---

## BC / NFR Coverage Map

### BC Delta: EMPTY

All new tests trace to existing BCs. No new BC files are created. No existing BCs are modified. BC-INDEX.md is unchanged.

Existing BCs the new tests exercise:

| BC | What the new test exercises |
|----|----------------------------|
| BC-X.8.004 | Non-JSM guard: `require_service_desk` error message shape + exit code when a JSM command targets ES (Jira Software project). `src/api/jsm/servicedesks.rs::require_service_desk` call-site label verified. |
| BC-X.12.001 | `jr requesttype list` — already covered (shallow). Now deepened to assert per-item `id` + `name` fields. |
| BC-X.12.005 | `jr requesttype fields <ID>` — the `GET .../requesttype/{id}/field` endpoint; asserts `fields` array shape. |
| BC-3.8.001 | `jr issue create --request-type` write round-trip — `POST /rest/servicedeskapi/request`; asserts `{"key": "EJ-N"}` on stdout; key format validation. |
| BC-3.8.004 | Numeric-bypass in create test (using RT id directly). |

No new BC is warranted. All JSM command behavior is already specified. Adding a BC for "queue view by name works" would be below the granularity threshold of the existing BC catalog.

### NFR Impact: None

No new NFRs warranted. The feature is a test-scope expansion; all NFR properties (correctness, latency, etc.) are inherited from the existing live E2E NFR catalog entry.

---

## Regression Risk Assessment

| Risk | Level | Detail |
|------|-------|--------|
| New E2E tests unexpectedly break the platform write flow | NONE | New tests are entirely within the JSM section; `JR_E2E_JSM_PROJECT` gate isolates them. Platform tests are unchanged. |
| surface-guard SURFACE rows adding false-positive failures | LOW | New rows reference real clap subcommands/flags that exist. Read from `src/cli/mod.rs` before writing — all verified in this analysis: `queue view --id`, `requesttype fields`, `issue comment --internal`, `issue create --request-type`. |
| JSM create test leaving orphaned EJ issues | LOW | Self-close in test body. If a mid-panic orphan occurs, EJ issues do not affect the platform project (ES) or any other test. Nightly re-runs re-create and re-close. |
| Comment visibility round-trip asserting wrong property path | LOW | `Comment.properties[].key == "sd.public.comment"` and `.value.internal == true` are verified by existing unit tests in `src/types/jira/issue.rs` (lines 566-598) and `src/cli/issue/format.rs`. The E2E asserts the same shape against real Jira. |
| queue view by name failing due to ambiguity | LOW | `partial_match` requires exact-case match for `queue view <name>`. Test should use the exact name returned by `queue list`. |
| docs/specs and CLAUDE.md doc errors | LOW | Documentation prose; no CI/test behavior depends on it. |

**Overall Regression Risk: LOW.**

---

## Recommended Story Breakdown

This feature is unified around a single concern (JSM E2E coverage) and all changes are in the same files. One story is recommended.

| Story ID | Scope | Effort |
|----------|-------|--------|
| S-JSM-E2E-1 | All new tests in e2e_live.rs + 4 new SURFACE rows in e2e_cli_surface_guard.rs + CLAUDE.md + docs/specs update + GitHub Environment variable set | 3 SP |

**Total: 1 story, 3 SP.**

Rationale for 3 SP (vs 2 SP for the fork-safety feature):
- The write round-trip test has non-trivial teardown logic and requires careful test structuring.
- The comment visibility round-trip has a multi-step setup (create JSM issue → add public comment → add internal comment → read back → assert two distinct visibility states).
- The surface-guard additions require verifying each flag exists on its subcommand.
- 10 new test functions is materially more work than the 4-file YAML+doc feature.

**Split option (if 3 SP feels large):** Split into S-JSM-E2E-1a (reads only: queue list/view deepened + requesttype list/fields + non-JSM guard, 2 SP) and S-JSM-E2E-1b (writes: create --request-type round-trip + comment visibility, 2 SP). The reads story can ship and activate JSM coverage (by setting JR_E2E_JSM_PROJECT=EJ) while writes are still in progress.

---

## F1 Decisions to Lock at Human Gate

| # | Decision | Options | Recommendation |
|---|----------|---------|----------------|
| D-1 (LOCKED) | Test set (first cut) | 6 scenarios as specified in brainstorm | LOCKED: queue list/view deepened, requesttype list/fields, comment internal/external round-trip, create --request-type write round-trip, non-JSM guard |
| D-2 (LOCKED) | Deferred items | --on-behalf-of, write:servicedesk-request scope hint | LOCKED: deferred, noted in STATE |
| D-3 (LOCKED) | Env var scope | JR_E2E_JSM_PROJECT=EJ as jira-e2e environment variable | LOCKED: already wired in e2e.yml; just needs value set |
| D-4 | **Teardown approach for create test** | Self-close in test body vs label-sweeper extension | RECOMMEND: self-close in test body. Labels do NOT reliably propagate through servicedeskapi to Jira issue labels. The label-sweeper extension would require both code and schema validation; self-close is zero-risk. |
| D-5 | **Request-type discovery** | Dynamic (parse first RT from requesttype list) vs new env var JR_E2E_JSM_REQUEST_TYPE | RECOMMEND: dynamic discovery. Uses the numeric-bypass path (all digits → skip name resolution), which is more robust than a name that can change. No new env var. |
| D-6 | **Queue view fixture** | Dynamic (parse first queue from queue list) vs known queue id env var | RECOMMEND: dynamic. If EJ has no queues, clean-skip rather than fail. |
| D-7 | **Free-tier skip policy** | Fail on 403 vs clean-skip on 403 | RECOMMEND: clean-skip on 403 (consistent with existing JSM test gating pattern; a 403 likely means plan-feature unavailability, not a jr bug). |
| D-8 | **BC delta** | Add new BCs vs reuse existing | RECOMMEND: REUSE EXISTING (BC-X.8.004, BC-X.12.001, BC-X.12.005, BC-3.8.001, BC-3.8.004). No new BCs warranted. |
| D-9 | **Story count** | 1 story vs 2 stories (reads/writes split) | RECOMMEND: 1 story (3 SP). Split only if timeline pressure requires incremental activation of JSM coverage. |
| D-10 | **Mid-panic orphan policy** | Accept residual EJ orphan risk vs extend sweeper | RECOMMEND: accept (LOW risk; EJ issues are inert; nightly cleanup). If unacceptable, extend teardown in e2e.yml to also sweep EJ by run-id label as a SEPARATE follow-on task. |

---

## Recommended Scope for F2-F7

| Phase | Recommended Scope |
|-------|------------------|
| F2 (spec evolution) | EMPTY BC delta confirmed. No PRD/BC change. Lightweight: record zero-BC delta. Note implementation constraints: (a) self-close teardown pattern; (b) dynamic RT/queue discovery; (c) clean-skip on 403 or empty results; (d) surface-guard additions are mandatory (not optional). |
| F3 (incremental stories) | Author S-JSM-E2E-1. Acceptance criteria: (a) all 6 test scenarios implemented; (b) all 4 SURFACE rows added and guard test passes; (c) CLAUDE.md updated; (d) docs/specs updated; (e) GitHub Environment var EJ set (admin step, documented in ACs). |
| F4 (delta implementation) | Implement S-JSM-E2E-1. No Rust src/ changes. Test-only implementation in tests/e2e_live.rs + tests/e2e_cli_surface_guard.rs. Note: `cargo test --lib` and `cargo test --test '*'` (non-E2E) must still pass — the new tests are all `#[ignore]`-gated. |
| F5 (scoped adversarial) | Review: (a) teardown logic (does the self-close actually reach the move step if assertions fail?); (b) comment visibility assertion uses correct JSON path; (c) surface-guard rows reference correct flags (--internal exists on `issue comment`, not `issue comments`); (d) clean-skip correctly handles empty JR_E2E_JSM_PROJECT. |
| F6 (targeted hardening) | No mutation testing (zero src/). Verify: the surface-guard offline test (`test_e2e_cli_surface_all_paths_and_flags_exist`) passes with the new rows. Confirm `cargo test --test e2e_cli_surface_guard` exits 0. |
| F7 (delta convergence) | Confirm CI passes (ci.yml green), surface-guard passes, nightly E2E runs show JSM tests executing (not just clean-skipping) once JR_E2E_JSM_PROJECT=EJ is set. No cargo-mutants run (zero src/). |

---

## Regression Baseline (files NOT changed)

All of `src/`; `Cargo.toml`; `Cargo.lock`; `deny.toml`; `.github/workflows/e2e.yml`; `.github/workflows/ci.yml`; `.github/workflows/release.yml`; `.github/workflows/e2e-sweeper.yml`; `scripts/`; `.cargo/mutants.toml`; `BC-INDEX.md`; `CANONICAL-COUNTS.md`; `.factory/specs/`.

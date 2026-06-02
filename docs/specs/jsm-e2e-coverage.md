# Feature Spec: JSM E2E Coverage Expansion (project EJ)

**Status:** Approved (F1 human gate passed 2026-06-01)
**Author:** jr maintainers (F2 spec evolution 2026-06-01)
**Extends:** `docs/specs/e2e-live-jira-testing.md` — cross-reference, not duplication.
  Sections of that spec updated by this feature are called out explicitly in §10 below.
**Research:** `.factory/planning/brainstorming-report-jsm-e2e.md` + `.factory/phase-f1-delta-analysis/jsm-e2e-expansion/delta-analysis.md`
**Tracking:** GitHub issue TBD (filed during F4 delivery)

---

## 1. Problem and Context

The live E2E suite has exactly two JSM tests — `test_e2e_jsm_queue_list_exits_ok` and
`test_e2e_jsm_requesttype_list_exits_ok` — both gated on `JR_E2E_JSM_PROJECT` (currently
unset → clean-skip) and both asserting only `exit 0 + is-array`. This is the same
false-confidence profile that motivated live E2E testing in the first place: the
createmeta/label/priority wire-shape bugs that shipped undetected through mocks and only
surfaced against real Jira. Shallow JSM assertions repeat that risk verbatim.

The deeper structural problem: JSM has divergent code paths that mocks cannot exercise.
`issue create --request-type` routes to `POST /rest/servicedeskapi/request` (the ADR-0014
dispatch fork), not to `/rest/api/3/issue`. The `sd.public.comment` entity property
determining internal/external comment visibility is set via platform endpoints but only
readable as live Jira data — a mock returns whatever the test fixture says; real Jira
actually enforces the property. The `require_service_desk` guard calls
`get_or_fetch_project_meta` and fails in a project-type-specific way that requires a real
project record to discriminate.

Now that project `EJ` (E2E-JSM) exists on the live site, JSM commands can be exercised
with the same shape-asserting and round-trip rigour applied to platform commands.

**BC and NFR corpora unchanged.** All new tests trace to existing BCs. The BC corpus
(585 BCs) and NFR corpus (41 NFRs) are EXPLICITLY UNCHANGED by this spec. No new BC
file is created; no existing BC is modified; BC-INDEX.md is unchanged.

---

## 2. Scope and Constraints

### 2.1 Zero src/ Delta

This feature is zero-`src/`. All JSM commands already exist:

| Command | Implementation |
|---------|----------------|
| `jr queue list/view` | `src/cli/queue.rs` + `src/api/jsm/queues.rs` |
| `jr requesttype list/fields` | `src/cli/requesttype.rs` + `src/api/jsm/request_types.rs` |
| `jr issue create --request-type` | `src/cli/issue/create.rs::handle_jsm_create` + `src/api/jsm/requests.rs` |
| `jr issue comment --internal` | `src/cli/issue/workflow.rs` + `src/api/jira/issues.rs::add_comment` |
| `jr issue comments --output json` | `src/cli/issue/comments.rs` (serializes `properties[]` including `sd.public.comment`) |
| `jr issue move` | existing platform transitions path |
| `require_service_desk` guard | `src/api/jsm/servicedesks.rs` |

F1 confirmed no jr capability gap. F4 has no `src/` change, no Red Gate invocation, and no
demo story. The implementing story is pure test and documentation.

### 2.2 BC / NFR Coverage Map

New tests trace to existing BCs without modification:

| BC | What the new test exercises |
|----|----------------------------|
| BC-X.8.004 | Non-JSM guard: `require_service_desk` error message shape + exit code when a JSM command targets ES (a Jira Software project). (AC-007) |
| BC-X.12.001 | `jr requesttype list` — deepened to assert per-item `id` + `name` fields. (AC-002) |
| BC-X.12.005 | `jr requesttype fields <ID>` — GET `.../requesttype/{id}/field`; asserts `fields` array shape. (AC-004) |
| BC-3.8.001 | `jr issue create --request-type` write round-trip — `POST /rest/servicedeskapi/request`; asserts `{"key": "EJ-N"}` on stdout. (AC-006) |
| BC-3.8.004 | Numeric-bypass in create test (using RT id directly). (AC-004, AC-006) |
| BC-3.5.001 | `jr issue comment <key> --internal` write side: adds `sd.public.comment` property. (AC-005) |
| BC-2.4.041 | `jr issue comments --output json` read side: exposes `properties[]` array including `sd.public.comment`. (AC-005) |

**Orphan note — AC-001 and AC-003 (queue list / queue view):**
`jr queue list` and `jr queue view` shipped in an earlier delivery cycle with NO behavioral
contracts in the BC corpus. Anchoring these E2E tests to BC-X.12.001 (a `requesttype`
contract) would be a semantically-invalid traceability link — a recognized "false coverage"
anti-pattern. AC-001 and AC-003 are therefore explicitly logged as un-contracted (orphan)
acceptance criteria. This is a pre-existing corpus gap, not introduced by this story. The
resolution is tracked follow-up story S-QUEUE-BC-1, which will author document-as-is BCs
(proposed: BC-X.8.008 / BC-X.8.009) for the queue command family in section X.8 "Projects
& Queues", in parity with how requesttype commands got X.12.001-008. Research justification:
`.factory/research/jsm-e2e-queue-bc-anchoring-validation.md`.

---

## 3. Clean-Skip Policy

All JSM tests share a uniform clean-skip policy. A skip is loud (eprintln to stderr) and
MUST never cause a test failure.

### 3.1 Primary gate — JR_E2E_JSM_PROJECT

Every JSM test MUST begin with:

```rust
let jsm_project = match std::env::var("JR_E2E_JSM_PROJECT") {
    Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
    _ => {
        eprintln!("[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM test");
        return;
    }
};
```

No new env var is introduced. The variable is already wired in `e2e.yml` (line ~100) as
`JR_E2E_JSM_PROJECT: ${{ vars.JR_E2E_JSM_PROJECT }}`. Activation requires only setting the
value `EJ` in the `jira-e2e` GitHub Environment (see §9).

### 3.2 Dynamic-discovery skip — empty list

When a test derives a fixture from a list (queues, request types), it MUST skip cleanly if
the list is empty:

```rust
let queues: Vec<serde_json::Value> = serde_json::from_str(&stdout).expect("JSON");
if queues.is_empty() {
    eprintln!("[SKIP] No queues found on EJ — skipping queue view test");
    return;
}
```

An empty list is a valid configuration state, not an error.

### 3.3 403 / feature-unavailable skip

Any JSM API call returning HTTP 403 (feature not on plan, insufficient permission, or plan
tier) MUST be treated as a clean-skip condition, not a test failure. Emit an eprintln
message. Pattern: check `status.success()` and additionally inspect stderr for a `403`
substring before asserting.

---

## 4. Dynamic-Discovery Design

### 4.1 Queue Fixture

`queue view` requires a queue id and name. These are derived dynamically — no hardcoded
fixture env var is introduced.

**Output model correction (F5):** `jr queue view --output json` returns the queue's
ISSUES as a JSON array of issue objects (each with `"key"` and `"fields"`) — NOT a
queue identity object. `handle_view` in `src/cli/queue.rs` calls `output::print_output`
with `&issues` (`Vec<Issue>`). The test assertions must match this: exit 0 + parseable
issue array, with per-element `"key"`/`"fields"` check if non-empty. The routing value
(by-name vs by-id) is validated by both paths succeeding, not by comparing queue id/name.

Discovery pattern:
1. Run `jr queue list --project <EJ> --output json`.
2. Parse the JSON array; if empty → clean-skip (§3.2).
3. Take `queues[0]["id"]` (string or integer → stringify) for the `--id` path.
4. Take `queues[0]["name"]` (string) for the by-name path.
5. Use the exact name from step 4 — `partial_match` requires an unambiguous substring;
   using the full name from the API response guarantees uniqueness. If EJ contains two
   queues with identical names, `resolve_queue_by_name` returns `UserError` (exit 64)
   with a "Multiple queues" message; the by-name sub-path clean-skips in that case (the
   by-id path is unaffected and continues to provide routing-branch coverage).

### 4.2 Request-Type Fixture

`requesttype fields` and `issue create --request-type` both require a real request-type id.
These are derived dynamically.

Discovery pattern:
1. Run `jr requesttype list --project <EJ> --output json`.
2. Parse the JSON array; if empty → clean-skip (§3.2).
3. Take `rts[0]["id"]` as a string.
4. Confirm the value is numeric (all JSM request-type ids on the Atlassian platform are
   positive integers); if not numeric for any reason → skip with a warning.
5. Use this id string as the `--request-type <id>` argument. Because the string is
   all-ASCII-digit, `handle_jsm_create` takes the numeric-bypass path
   (`src/cli/issue/create.rs`: `if request_type_arg.chars().all(|c| c.is_ascii_digit())`)
   and skips name resolution — the most robust path.

The requesttype list test is declared before the create and fields tests in `e2e_live.rs`.
Because `--test-threads=1` serializes execution in declaration order, the create test can
depend on requesttype list having been exercised — but MUST re-fetch its own fixture
independently (no shared state between test functions).

---

## 5. Test Scenarios

Seven test scenarios are defined. All are `#[ignore]`-gated via the `e2e_enabled()` check
and additionally gated on `JR_E2E_JSM_PROJECT` per §3.1.

### Scenario 1 — Deepen queue list shape assertions

**Test function:** `test_e2e_jsm_queue_list_shape` (replaces or supplements the existing
shallow `test_e2e_jsm_queue_list_exits_ok`)

**Steps:**
1. Run `jr queue list --project <EJ> --output json`.
2. Assert exit 0.
3. Parse the response as a JSON array.
4. If the array is non-empty, assert that each item has an `"id"` field (non-null) and a
   `"name"` field (non-null, non-empty string). A single item assertion is sufficient to
   catch wire renames; iterating all items is preferred.

**Clean-skip condition:** None (an empty array is asserted as a valid state; the test
passes even with zero queues, since the per-item assertion only fires if items exist).

**BC traced:** NONE (explicitly logged orphan). `jr queue list` shipped without a behavioral
contract — see §2.2 orphan note. Tracked in follow-up story S-QUEUE-BC-1.

---

### Scenario 2 — Deepen requesttype list shape assertions

**Test function:** `test_e2e_jsm_requesttype_list_shape` (replaces or supplements the
existing shallow `test_e2e_jsm_requesttype_list_exits_ok`)

**Steps:**
1. Run `jr requesttype list --project <EJ> --output json`.
2. Assert exit 0.
3. Parse the response as a JSON array.
4. If the array is non-empty, assert that each item has an `"id"` field (non-null) and a
   `"name"` field (non-null, non-empty string).

**Clean-skip condition:** None (empty array is a valid state).

**BC traced:** BC-X.12.001 (requesttype read command output shape).

---

### Scenario 3 — Queue view by name AND by --id

**Test function:** `test_e2e_jsm_queue_view`

**Output model:** `jr queue view --output json` returns the queue's ISSUES as a JSON
array of issue objects — NOT a queue identity object. `handle_view` outputs `&issues`
(`Vec<Issue>`). The test validates both routing branches (name→id resolution vs direct
`--id`) by asserting exit 0 and a parseable issue array on each path.

**Steps:**
1. Run `jr queue list --project <EJ> --output json`; parse the array.
2. If the array is empty → clean-skip (§3.2).
3. Extract `first_id` (stringify) and `first_name` (exact string) from `queues[0]`.
4. **By-name path:** Run `jr queue view "<first_name>" --project <EJ> --output json`.
   Assert exit 0; parse stdout as a JSON array; if non-empty, assert each element has
   `"key"` and `"fields"` fields (issue objects). An empty array is a valid pass
   (a queue with zero issues exists and is accessible).
5. **By-id path:** Run `jr queue view --id <first_id> --project <EJ> --output json`.
   Assert exit 0; parse stdout as a JSON array; same per-element shape assertion.
   An empty array is valid.

**DO NOT** assert `"id"` or `"name"` equality in the view response — those fields
belong to the queue list endpoint, not the queue view endpoint.

**Clean-skip condition:** Skip when the queue list is empty (§3.2). Skip on 403 (§3.3).

**BC traced:** NONE (explicitly logged orphan). `jr queue view` shipped without a behavioral
contract — see §2.2 orphan note. Tracked in follow-up story S-QUEUE-BC-1. The `--id` flag
path exercises the distinct routing branch in `src/cli/queue.rs`; this behavior will be
contracted in S-QUEUE-BC-1.

---

### Scenario 4 — requesttype fields shape + numeric-bypass pin

**Test function:** `test_e2e_jsm_requesttype_fields`

**Steps:**
1. Run `jr requesttype list --project <EJ> --output json`; parse the array.
2. If the array is empty → clean-skip (§3.2).
3. Extract `first_rt_id` (stringify) from `rts[0]["id"]`; confirm it is all-ASCII-digit.
4. Run `jr requesttype fields <first_rt_id> --project <EJ> --output json`.
5. Assert exit 0.
6. Parse the response; assert the top-level shape contains a `"fields"` key (array, possibly
   empty) — the array validates the endpoint contract and deserialization.

**Numeric-bypass pin:** Because `first_rt_id` is all-ASCII-digit, `src/cli/requesttype.rs`
(`if !name_or_id.is_empty() && name_or_id.chars().all(|c| c.is_ascii_digit())`) takes the
numeric-id path, bypassing `partial_match` and cache name resolution. This test pins that
the numeric path succeeds end-to-end against real Jira. The degenerate case (a request type
named exactly "100" is unreachable by name) is documented in CLAUDE.md and not exercised
here (unit-test concern).

**Clean-skip condition:** Skip when the requesttype list is empty (§3.2). Skip on 403 (§3.3).

**BC traced:** BC-X.12.005 (requesttype fields output shape). BC-3.8.004 (numeric bypass).

---

### Scenario 5 — Internal vs external comment visibility round-trip

**Test function:** `test_e2e_jsm_comment_visibility`

This is the most complex JSM test. It creates a fresh JSM request, exercises the
`--internal` flag, reads back the `sd.public.comment` property from the comments JSON, and
self-closes.

**Steps:**
1. Run `jr requesttype list --project <EJ> --output json`; parse; if empty → clean-skip.
2. Extract `first_rt_id` from `rts[0]["id"]`.
3. Create a fresh JSM request; capture `key` from `{"key": "<key>"}` stdout.
4. Add a **public** comment: `"public comment from e2e run <run_id>"` (no `--internal`).
   FIX 3a: ANY non-success (not just 403) → best-effort close + clean-skip. This ensures
   no comment-add failure can panic-orphan the created issue.
5. Add an **internal** comment: `"internal comment from e2e run <run_id>" --internal`.
   Same any-failure clean-skip as step 4.
6. **Self-close FIRST (F-2b + FIX 3a close-always-runs):** `jr issue move <key> <Done>`
   executes BEFORE the read-back and all property assertions. No-orphan invariant: once a
   valid key is captured, every exit path either performs a best-effort close in the
   comment-add failure handlers (step 4/5) or reaches this unconditional close. Close is
   best-effort (warn on failure, never fail the test).
7. **FIX 2 + F-3 predicate-driven retry:** read back `jr issue comments <key> --output json`
   in a retry loop (max 5 attempts, 250 ms → 2 000 ms backoff) and retry while the FULL
   success predicate is false — i.e., retry when either (a) a comment body has not yet
   appeared, OR (b) the `sd.public.comment` property has not yet expanded (body visible but
   property still absent). Breaking as soon as bodies appear but before property expansion
   is confirmed can hard-fail on a property-lag, defeating the retry purpose. On budget
   exhaustion: if bodies never appeared → `[SKIP]` (environmental lag). If bodies appeared
   but property state is wrong → real regression, assert and fail loudly.
8. **F-1 body-matched assertions:** locate each comment by serialized-ADF-body substring.
   - The comment matching `"internal comment from e2e run <run_id>"` MUST have
     `properties[].key == "sd.public.comment"` with `.value.internal == true`.
   - The comment matching `"public comment from e2e run <run_id>"` MUST NOT have that
     property set to true. This prevents system/journal comments from satisfying the check.

**sd.public.comment property detail:** `src/api/jira/issues.rs::list_comments` adds
`?expand=properties`. The E2E assertion traverses the raw JSON `properties` array —
not the table-display discriminator — for precision.

**Teardown:** Self-close executed before assertions (step 6). See §6 for orphan-risk
documentation.

**Clean-skip conditions:** requesttype list empty (§3.2); any failure on comment-add steps
(FIX 3a — close-then-skip on any non-success, not just 403); create returns non-zero
(skip); comment read-back budget exhausted with bodies absent (FIX 2 / F-3 skip). If
bodies appeared but property wrong at budget exhaustion → hard-fail (genuine regression).

**BC traced:** BC-3.5.001 — `jr issue comment --internal` adds `sd.public.comment` property
(write side). BC-2.4.041 — `jr issue comments --output json` exposes `properties[]` array
including `sd.public.comment` (read side). `jr issue create --request-type` is used for
fixture setup only (create path traced to BC-3.8.001 in Scenario 6).

---

### Scenario 6 — issue create --request-type write round-trip

**Test function:** `test_e2e_jsm_create_request_roundtrip`

**Steps:**
1. Run `jr requesttype list --project <EJ> --output json`; parse; if empty → clean-skip.
2. Extract `first_rt_id` from `rts[0]["id"]`; confirm all-ASCII-digit.
3. Create a request; assert exit 0; parse `{"key": "<key>"}` stdout; capture `key`;
   assert `key` is non-empty.
4. **Non-fatal bounded poll (F-2b):** attempt `jr issue view <key> --output json` in a
   local retry loop (max 5 attempts, 250 ms → 2 000 ms backoff), returning
   `Option<Value>` — NEVER panicking. On budget exhaustion, capture `None` and continue.
5. **Self-close FIRST (F-2b close-always-runs):** `jr issue move <key> <Done>` executes
   BEFORE any remaining assertions. This guarantees that poll exhaustion or a prefix-
   assertion failure cannot orphan the created EJ issue. Close is best-effort (warn, never
   fail the test).
6. Assert key prefix matches the JSM project (e.g. starts with `"EJ-"`). This is a
   purely in-memory check after the close.
7. Assert `poll_view` result: if `Option<Some(v)>`, assert `v["key"] == key`. If `None`,
   emit `[WARN]` and skip the view assertion (GET-by-key lag on free-tier site is
   environmental — the creation was already confirmed by step 3).

**F-2b rationale:** the original `poll_view()` call panics after MAX_ATTEMPTS, which fires
BEFORE the self-close and leaves the EJ issue orphaned. The non-fatal local poll + close-
first ordering eliminates this orphan-on-transient-GET-lag path entirely.

**ADR-0014 dispatch fork pin:** this test exercises `handle_jsm_create` which dispatches to
`POST /rest/servicedeskapi/request` — a completely different endpoint from the platform
`POST /rest/api/3/issue`. The response type `JsmRequestCreated` deserializes `issue_key:
String` and emits `{"key": issue_key}` on stdout. This path cannot be validated by mocks.

**Teardown:** Self-close executed before post-create assertions (step 5). See §6 for
orphan-risk documentation.

**Clean-skip conditions:** requesttype list empty (§3.2); 403 (§3.3); create returns
non-zero exit (skip with eprintln, not fail).

**BC traced:** BC-3.8.001 (`issue create --request-type` write path). BC-3.8.004 (numeric
bypass used implicitly via all-digit id).

---

### Scenario 7 — Non-JSM guard (require_service_desk)

**Test function:** `test_e2e_jsm_non_jsm_guard`

**Steps:**
1. Run `jr queue list --project <ES> --output json` where `<ES>` is the standard Scrum
   project (`JR_E2E_PROJECT`, typically `ES`).
2. Assert exit code is non-zero (specifically 64, `UserError`, per `JrError::exit_code()`).
3. Assert stderr contains the `require_service_desk` error message. The error message
   format is `<call_site_label> a Jira Service Management project.` where `call_site_label`
   is a noun phrase passed by the queue list handler. Assert that stderr contains the
   substring `"Jira Service Management project"` (locale-stable substring of the
   call-site-labeled message, per BC-X.8.004).

**Note on BC-X.8.004 call-site label contract:** `require_service_desk` takes a
`call_site_label: &'static str` that is prepended before `" a Jira Service Management
project."`. The exact phrase depends on the call site. The E2E test asserts a stable
substring (`"Jira Service Management project"`) rather than the full verbatim string to
remain resilient to call-site-label wording changes that don't alter the core contract.

**Clean-skip condition (FIX 1):** Auth failures must be clean-skipped before any exit-64
assertion fires. The skip is keyed on exit code 2 (`JrError::NotAuthenticated`, the
definitive auth-failure signal) OR on the combined stdout+stderr containing "401", "403",
or "Not authenticated" when exit code is not 64. Rationale: `require_service_desk`
(`src/api/jsm/servicedesks.rs`) rewrites a live-Jira 401 into `JrError::NotAuthenticated`
(exit 2, message "Not authenticated. Your API token may be expired or revoked…"). The raw
"401" string does NOT appear in `jr` output in this path — keying the skip on a bare "401"
stderr substring is insufficient and will miss an expired-token scenario entirely.

**Message channel (OBS-1):** The `"Jira Service Management project"` substring assertion
checks the COMBINED stdout+stderr. In this codebase `jr` always emits error text to stderr
in both human and json modes (`src/main.rs` uses `eprintln!` for the JSON error envelope),
so in practice the message is always on stderr. The combined check is a defensive superset:
it can never miss a stderr-only message and remains robust to any future channel change
without requiring a test update.

**Harness precondition (OBS-2):** `JR_E2E_PROJECT` must name a live, reachable, NON-JSM
(Jira Software/Work) project. A missing or unreachable project hard-fails this test by
design. Do not broaden the auth-skip to cover 404 or network errors — that would mask a
guard regression where the wrong error (not exit-64 + JSM-guard message) is returned.

**BC traced:** BC-X.8.004 (`require_service_desk` guard error + exit code).

---

## 6. Teardown Design and Orphan-Risk Documentation

### 6.1 Self-Close in Test Body

Scenarios 5 and 6 create JSM requests on EJ and MUST self-close them in the test body.

The pattern is: capture `key` from `--output json` stdout immediately after create; run
all assertions; unconditionally attempt `jr issue move <key> <JR_E2E_STATUS_DONE:-Done>`
at the end of the function. If the move fails (e.g. workflow does not have a transition to
Done, or the issue is already Done), emit a warning (`eprintln!`) but do NOT fail the test
on close failure — the close step is best-effort cleanup, not an assertion.

```rust
// Always attempt close at end of test
let close_out = e2e_cmd()
    .args(["issue", "move", &key, &status_done])
    .output()
    .expect("close command");
if !close_out.status.success() {
    eprintln!("[WARN] Failed to close EJ issue {key}: {:?}", close_out.status);
}
```

`jr issue move <EJ-key> <Done>` calls `POST /rest/api/3/issue/{key}/transitions` (the
platform transitions endpoint), which is valid for JSM issues — they are standard Jira
issues underneath the service management layer.

### 6.2 Labels Do NOT Propagate to JSM Requests — Sweeper Cannot Cover EJ

F1 code analysis confirmed that labels inserted into `requestFieldValues` via
`JsmRequestBuilder::build()` (`src/api/jsm/requests.rs`) do NOT reliably propagate to
the Jira issue's `labels` field. The existing e2e.yml teardown sweeper (line ~189) queries:

```
project=$JR_E2E_PROJECT AND labels=e2e-$GITHUB_RUN_ID AND statusCategory != Done
```

This sweeper targets `JR_E2E_PROJECT` (ES) only. Even if labels propagated, EJ issues
would not be caught without extending the sweeper.

**Conclusion: The label-based sweeper CANNOT be relied on for EJ-created requests.**

### 6.3 Residual Orphan Risk

If a test panics mid-flight (between `create` and the self-close step), the EJ issue stays
open. This is LOW risk:

- EJ issues do not affect the platform project ES or any other test.
- Free Jira Cloud sites have no issue quota concern.
- Nightly runs will create and close fresh issues — orphans accumulate but are inert.
- The CI sweeper does NOT cover EJ and labels do not propagate from the JSM create path —
  this is an explicit accepted gap. If orphan accumulation becomes a problem, extend the
  sweeper as a separate maintenance task outside this feature scope.

This orphan risk is documented explicitly here and must be noted in the implementing story's
ACs.

---

## 7. Deferred Sub-Gaps (Out of Scope)

The following JSM behaviors are explicitly out of scope for this feature. They are
documented here to prevent re-discovery and to enable future targeting.

| Sub-gap | Reason deferred | Prerequisite |
|---------|----------------|--------------|
| `--on-behalf-of` flag on `issue create --request-type` | Requires a second customer account on the EJ site. The CI service account is the only account; creating a request on behalf of a second user requires that account to exist. | Provision a second Atlassian account on the E2E site. |
| `write:servicedesk-request` 401 scope hint | Requires a scope-stripped OAuth token to trigger the `InsufficientScope` path (BC-3.8.015). The E2E suite authenticates via Basic auth (`JR_AUTH_HEADER`); a 401 scope error would require a Bearer token with the `write:servicedesk-request` scope absent. | Provision a scope-stripped OAuth token as a CI secret. |
| JSM queue/requesttype pagination | EJ has a small number of queues/request types; pagination is not exercised at free-tier scale. | A larger JSM project with paginated results. |
| Service desk `requesttype` creation/deletion | No `jr requesttype create/delete` command exists. | New jr feature (separate feature scope). |

---

## 8. Verification Properties

These properties have no formal proof strategy (the feature is test-only; no new Rust code
is introduced). They are empirical CI-run checks verified during Phase F6 (targeted
hardening) by inspecting CI run output after `JR_E2E_JSM_PROJECT=EJ` is activated.

### VER-JSM-E2E-1: Queue list shape

**Scenario:** 1 (§5, Scenario 1)
**BC anchor:** NONE — `jr queue list` is currently un-contracted (explicitly logged orphan;
see §2.2 orphan note). This verification property verifies empirical behavior; contract
authoring is tracked in follow-up story S-QUEUE-BC-1.
**Condition:** `jr queue list --project EJ --output json` exits 0 and returns a JSON array
where every item has non-null `"id"` and `"name"` fields.
**Verification method (F6):** Inspect the CI E2E run log entry for
`test_e2e_jsm_queue_list_shape`. Confirm the test passes (not skipped) and the assertion
on per-item field presence succeeds.
**Expected outcome:** test passes; no shape assertion fires.

---

### VER-JSM-E2E-2: Requesttype list shape

**Scenario:** 2 (§5, Scenario 2)
**Condition:** `jr requesttype list --project EJ --output json` exits 0 and returns a JSON
array where every item has non-null `"id"` and `"name"` fields.
**Verification method (F6):** Inspect the CI E2E run log for
`test_e2e_jsm_requesttype_list_shape`. Confirm the test passes.
**Expected outcome:** test passes; no shape assertion fires.

---

### VER-JSM-E2E-3: Queue view by name and by --id (issues array)

**Scenario:** 3 (§5, Scenario 3)
**BC anchor:** NONE — `jr queue view` is currently un-contracted (explicitly logged orphan;
see §2.2 orphan note). This verification property verifies empirical behavior including both
the by-name and `--id` routing branches; contract authoring is tracked in follow-up story
S-QUEUE-BC-1.
**Condition:** `jr queue view "<name>" --project EJ --output json` exits 0 and returns a
JSON array of issue objects (each with `"key"` and `"fields"` if non-empty);
`jr queue view --id <id> --project EJ --output json` exits 0 and returns the same shape.
An empty array on either path is a valid pass. `"id"` and `"name"` fields from the queue
list are NOT present in the view response — do not assert them.
**Verification method (F6):** Inspect the CI E2E run log for `test_e2e_jsm_queue_view`.
Confirm both the by-name and by-id sub-paths are exercised (not skipped) and the
issue-array shape assertions pass.
**Expected outcome:** test passes; both routing branches produce a parseable issue array.

---

### VER-JSM-E2E-4: Requesttype fields shape and numeric-bypass

**Scenario:** 4 (§5, Scenario 4)
**Condition:** `jr requesttype fields <numeric_id> --project EJ --output json` exits 0
and the response contains a top-level `"fields"` key.
**Verification method (F6):** Inspect the CI E2E run log for
`test_e2e_jsm_requesttype_fields`. Confirm the test passes and the numeric-bypass path
was taken (the id used is all-digit).
**Expected outcome:** test passes; `"fields"` key present in response.

---

### VER-JSM-E2E-5: Comment internal vs external visibility round-trip

**Scenario:** 5 (§5, Scenario 5)
**BC anchor:** BC-3.5.001 (write side — `jr issue comment --internal` adds `sd.public.comment`
property) + BC-2.4.041 (read side — `jr issue comments --output json` exposes `properties[]`
array including `sd.public.comment`).
**Condition:** After adding a public comment and an `--internal` comment to a fresh EJ
request, `jr issue comments <key> --output json` returns an array where:
- At least one item has `properties[].key == "sd.public.comment"` with
  `value.internal == true` (the internal comment).
- At least one item does NOT have that property (the public comment).
**Verification method (F6):** Inspect the CI E2E run log for
`test_e2e_jsm_comment_visibility`. Confirm the test passes, the EJ issue was created and
closed, and both visibility assertions succeed.
**Expected outcome:** test passes; `sd.public.comment` property correctly present on the
internal comment and absent (or not set to `true`) on the public comment.

---

### VER-JSM-E2E-6: issue create --request-type write round-trip

**Scenario:** 6 (§5, Scenario 6)
**Condition:** `jr issue create --project EJ --request-type <id> --summary "..." --output
json` exits 0, returns `{"key": "EJ-N"}`, `poll_view(key)` resolves, and `jr issue move
<key> Done` succeeds.
**Verification method (F6):** Inspect the CI E2E run log for
`test_e2e_jsm_create_request_roundtrip`. Confirm the ADR-0014 dispatch fork is exercised
(POST to servicedeskapi endpoint, not to `/rest/api/3/issue`), the key is returned, and
the issue is closed.
**Expected outcome:** test passes; key captured, poll_view resolves, self-close succeeds.

---

### VER-JSM-E2E-7: Non-JSM guard exits with correct error

**Scenario:** 7 (§5, Scenario 7)
**Condition:** `jr queue list --project ES --output json` exits non-zero (exit 64) and the
combined stdout+stderr contains the substring `"Jira Service Management project"` (error
text is always on stderr in this codebase; combined check is a defensive superset per
OBS-1). If the exit code is 2 (NotAuthenticated) or if the combined output contains "401",
"403", or "Not authenticated" with a non-64 exit code, the test clean-skips (auth failure,
not a guard-contract violation). A missing/unreachable JR_E2E_PROJECT hard-fails by design
(OBS-2 — do not broaden skip). See §5 Scenario 7 for the exact logic.
**Verification method (F6):** Inspect the CI E2E run log for `test_e2e_jsm_non_jsm_guard`.
Confirm the test passes and the exit-code-64 + message assertions succeed.
**Expected outcome:** test passes; exit 64 confirmed; `require_service_desk` error message
confirmed against BC-X.8.004.

---

## 9. Rollout

No workflow code change is needed. The variable `JR_E2E_JSM_PROJECT` is already wired in
`e2e.yml` (line ~100) as `JR_E2E_JSM_PROJECT: ${{ vars.JR_E2E_JSM_PROJECT }}` inside the
"Run live E2E tests" step `env:` block. Activation requires a single admin operation:

**Set `JR_E2E_JSM_PROJECT=EJ` in the `jira-e2e` GitHub Environment.**

Steps:
1. Navigate to `https://github.com/Zious11/jira-cli/settings/environments/jira-e2e`.
2. Under "Environment variables", click "Add variable".
3. Name: `JR_E2E_JSM_PROJECT`, Value: `EJ`.
4. Save.
5. Verify: trigger a `workflow_dispatch` run on `develop`. Confirm the JSM tests appear in
   the run log as executing (not as `[SKIP]`) and the new test functions are present.

This is an environment variable (environment-scoped to `jira-e2e`), NOT a repository
variable. Environment-scoped variables are passed to the runner after the job starts — the
`JR_E2E_JSM_PROJECT` variable is consumed inside the running Rust test binary, not in a
`jobs.<id>.if:` expression, so environment-scoping is correct for this use case.

---

## 10. F4 Implementation Touch-Point List

The following files are modified in F4. This list is normative for the implementing story.
No other files are touched.

| File | Change | Spec reference |
|------|--------|---------------|
| `tests/e2e_live.rs` | Add 7 new `#[ignore]`-gated test functions (Scenarios 1-7, §5), all gated on `JR_E2E_JSM_PROJECT` per §3.1 (except Scenario 7 which uses `JR_E2E_PROJECT`). Scenarios 5 and 6 include self-close teardown per §6. Replace or supplement the existing 2 shallow JSM tests. | §3, §5, §6 |
| `tests/e2e_cli_surface_guard.rs` | Add 4 new rows to the `SURFACE` static table: `(&["queue", "view"], &["--project", "--output", "--id"])`, `(&["requesttype", "fields"], &["--project", "--output"])`, `(&["issue", "comment"], &["--internal", "--output"])`, `(&["issue", "create"], &["--request-type", "--project", "--output", "--summary"])`. | §5, Scenarios 3-6 |
| `docs/specs/e2e-live-jira-testing.md` | §4 "Optional / feature-flagged" JSM entry: update to list all 7 new test function names and note that queue view, requesttype fields, comment visibility, and create round-trip are now covered. §8 Configuration inventory: update `JR_E2E_JSM_PROJECT` row notes to reflect `EJ` as the value and document the teardown design note (self-close, not sweeper). | §4, §9 |
| `CLAUDE.md` | In the AI Agent Notes E2E section: update `JR_E2E_JSM_PROJECT` entry to note `EJ` is now set; add teardown design convention for JSM write tests (`self-close in test body, not label-sweeper, because labels do not propagate through servicedeskapi to Jira issue labels`). | §6.2 |

**Files confirmed NOT changed:**
- `src/` (all files — zero Rust source changes)
- `.github/workflows/e2e.yml` (wiring already exists; no code change)
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `.github/workflows/e2e-sweeper.yml`
- `Cargo.toml`, `Cargo.lock`, `deny.toml`, `.cargo/mutants.toml`
- `scripts/`, `.factory/specs/` (no BC, PRD, or architecture change)
- `BC-INDEX.md`, `CANONICAL-COUNTS.md`

**F4 delivery notes:**
- Zero Rust compilation required for the delta (tests are `--test-threads=1` serialized;
  all new tests are `#[ignore]`-gated and never run in `ci.yml`).
- `cargo test` (non-E2E) and `cargo test --test e2e_cli_surface_guard` MUST pass with the
  new SURFACE rows. This is the F6 offline verification step.
- No Red Gate invocation; no demo story; no mutation testing run (zero `src/` change).
- Delivery is a single story (`S-JSM-E2E-1`, 3 SP) touching tests, SURFACE guard, and
  documentation.

---

## 11. F1 Decisions Encoded in This Spec

| Decision | Encoded in | Value |
|----------|-----------|-------|
| Test set (7 scenarios) | §5 | Scenarios 1-7 as specified |
| Dynamic discovery of queue/RT fixtures | §4 | No new env var; parse list output |
| Self-close teardown (not sweeper) | §6 | Capture key from `--output json`; `jr issue move` at test end |
| Sweeper does NOT cover EJ (labels don't propagate) | §6.2, §6.3 | Explicit accepted gap |
| Residual orphan risk accepted | §6.3 | LOW; inert EJ issues; documented |
| Clean-skip on unset JR_E2E_JSM_PROJECT | §3.1 | Loud eprintln, return |
| Clean-skip on empty list | §3.2 | Loud eprintln, return |
| Clean-skip on 403 | §3.3 | Loud eprintln, not fail |
| Numeric-bypass for RT id | §4.2 | All-ASCII-digit id → `handle_jsm_create` bypass |
| No new BC, no new NFR | §2.2 | BC corpus 585, NFR corpus 41, unchanged |
| Deferred: --on-behalf-of, scope-stripped-token 401 | §7 | Documented sub-gaps |
| Rollout: set JR_E2E_JSM_PROJECT=EJ in jira-e2e env | §9 | Environment variable (env-scoped, not repo-level) |
| Zero src/ delta | §2.1 | All JSM commands exist; F4 is test-only |

---

## 12. References

- F1 delta analysis: `.factory/phase-f1-delta-analysis/jsm-e2e-expansion/delta-analysis.md`
- Brainstorming report: `.factory/planning/brainstorming-report-jsm-e2e.md`
- Existing E2E spec: `docs/specs/e2e-live-jira-testing.md`
- Fork-safe CI spec: `docs/specs/e2e-fork-safe-ci-enablement.md`
- ADR-0004: Per-feature specs: `docs/adr/0004-per-feature-specs.md`
- ADR-0014: JSM create dispatch fork: `docs/adr/0014-jsm-request-creation.md`
- BC-X.8.004: `require_service_desk` guard: `.factory/specs/prd/bc-x-cross-cutting.md`

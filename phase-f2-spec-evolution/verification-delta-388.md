---
document_type: f2-verification-delta
phase: phase-f2-spec-evolution
producer: formal-verifier
issue: 388
status: draft
timestamp: 2026-05-20
project: jira-cli
mode: BROWNFIELD
intent: enhancement
inputs:
  - ".factory/phase-f1-delta-analysis/issue-388/delta-analysis.md"
  - "src/cli/issue/create.rs"
  - "src/types/jira/issue.rs"
---

# F2 Verification Delta — Issue #388

Verification approach for the #388 delta (cross-hierarchy type-change error +
fake-endpoint hint fix). Brief and actionable; consumed by the F4 implementer.

Filename note: the unsuffixed `verification-delta.md` in this directory belongs
to the #288 cycle. This document follows the issue-suffixed convention already
used by `prd-delta-384.md` / `prd-delta-385.md`.

## 1. No VP-NNN Artifacts Required — Confirmed

This project has **no formal verification-property layer**. Verified directly:

- No `.factory/specs/verification-*` directory exists.
- No `VP-NNN` document exists anywhere under `.factory/`.
- `.factory/specs/` contains only `domain-spec/`, `prd/`, and consistency-audit
  files — verification is anchored at the **BC level** (BC-3.4.010, BC-3.4.011)
  plus inline/integration tests, which is the established project standard.

F1 already reached this conclusion (delta-analysis.md §F6/F7); this step
confirms it independently. **No VP-NNN document is to be created for #388.**

## 2. Verification Artifact for the #388 Delta — Inline Proptest

The verification-architecture artifact for this feature is a single **inline
proptest** co-located with the pure classifier in
`src/cli/issue/create.rs` under a dedicated submodule
`mod is_cross_hierarchy_type_error_proptests`, mirroring the existing
`build_labels_proptests` / `parse_field_kv_proptests` sub-modules.

### Function under test

```rust
fn is_cross_hierarchy_type_error(
    src_subtask: Option<bool>,
    tgt_subtask: Option<bool>,
    err: &str,
) -> Classification

enum Classification { CrossHierarchy, SameCategory, Indeterminate }
```

The subtask flag is `Option<bool>`, not `bool`: the live field is
`IssueTypeMetadata.subtask: Option<bool>` (`src/api/jira/projects.rs:12`) and F1
mandates `IssueType.subtask: Option<bool>` with `#[serde(default)]`, so `None`
is genuinely reachable when Atlassian omits the field. The classifier is
therefore pure over the full `Option<bool> × Option<bool>` domain and returns
**all three** `Classification` variants:

- `Some(a), Some(b)` with `a != b` ⟹ `CrossHierarchy`
- `Some(a), Some(b)` with `a == b` ⟹ `SameCategory`
- either argument `None` ⟹ `Indeterminate`

No variant is dead. The `Indeterminate` path has a traceable pure-function
source: it is produced whenever a subtask flag was absent from the API response.
There are two distinct sub-causes of `Indeterminate`, both with named
integration coverage:

- **Cause 1 — fetch failure:** a *fetch failure* in the `handle_edit` caller
  (source issue or project-types call returns 5xx) yields `Indeterminate` — the
  caller either passes `None` for the unresolved flag or short-circuits to
  `Indeterminate` directly. Integration test #4 (project-types 5xx) pins this
  caller-side fetch-failure route.
- **Cause 2 — 200 + absent `subtask` flag:** a successful `HTTP 200` fetch
  whose response omits the `subtask` field deserializes (via `#[serde(default)]`)
  to `None`, which the pure classifier maps to `Indeterminate`. This sub-cause
  has two sides, each with its own named integration test:
  - **Source-side absent flag** — the *source* issue's type metadata omits
    `subtask`; pinned by integration test #6
    `test_edit_type_indeterminate_absent_subtask_flag_surfaces_raw_error`.
  - **Target-side absent flag** — the *target* issue type (resolved from
    `get_project_issue_types`) omits `subtask`; pinned by the new integration
    test #7 `test_edit_type_indeterminate_absent_target_subtask_flag_surfaces_raw_error`.

Either way every variant is reachable from the pure function itself, and the
proptest below covers all nine input states. Both `Indeterminate` sub-causes —
and, within Cause 2, both the source-side and target-side absent-`subtask`-flag
sub-causes — now have named integration coverage (tests #4, #6, and #7).

Separately, the **unresolvable target-type-name → typo-hint** routing outcome
(table row R3 in §3) is pinned by the new integration test #8
`test_edit_type_unresolved_type_name_surfaces_typo_hint`.

Two further integration tests pin the previously proof-by-analysis-only routing
rows R1 and R0b: test #9
`test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error` (row R1 —
`get_issue` itself fails) and test #10
`test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment` (row R0b —
`edit_issue` fails with a non-400 error). The integration suite in
`tests/issue_edit_type_errors.rs` therefore comprises **10 functions**.

### Properties to assert

| # | Property | Rationale |
|---|----------|-----------|
| P1 | `Some(a), Some(b)` with `a != b` ⟹ result == `CrossHierarchy` | Cross-hierarchy is decided purely by the subtask-flag mismatch — the primary, locale-independent gate. |
| P2 | `Some(a), Some(b)` with `a == b` ⟹ result == `SameCategory` | Matching flags = same hierarchy level; never a cross-hierarchy error. |
| P3 | either argument `None` ⟹ result == `Indeterminate` | An absent subtask flag means the hierarchy level is unknown; the classifier must not guess. |
| P4 | For any `err` string, the result depends ONLY on `(src_subtask, tgt_subtask)` — the `err` argument never changes the classification. | **Load-bearing.** Pins the architectural constraint (delta-analysis.md, research addendum A1/A2): the English substring `"issue type selected is invalid"` MUST NOT be a classifier. The `err` param exists only for hint composition, not branching — this prevents the false-positive regression flagged MEDIUM in F1. |

P1–P3 together form a total spec over the 9-state `Option<bool> × Option<bool>`
domain. P4 is the load-bearing property: it is the mechanism that prevents the
locale-fragility false-positive regression.

### Proptest strategy (precise — F4 implementer writes verbatim)

```rust
use proptest::prelude::*;

fn opt_bool() -> impl Strategy<Value = Option<bool>> {
    prop_oneof![Just(None), Just(Some(true)), Just(Some(false))]
}

proptest! {
    #[test]
    fn prop_cross_hierarchy_decided_by_subtask_flag_mismatch(
        src in opt_bool(),
        tgt in opt_bool(),
        // Arbitrary message; includes the locale-fragile substring with
        // non-zero probability so P4 actively exercises the no-influence claim.
        err in prop_oneof![
            ".*",
            Just("issue type selected is invalid".to_string()),
            Just(String::new()),
        ],
    ) {
        let result = is_cross_hierarchy_type_error(src, tgt, &err);

        match (src, tgt) {
            (Some(a), Some(b)) if a != b => {
                prop_assert_eq!(result, Classification::CrossHierarchy);  // P1
            }
            (Some(a), Some(b)) => {
                let _ = (a, b);
                prop_assert_eq!(result, Classification::SameCategory);    // P2
            }
            _ => {
                prop_assert_eq!(result, Classification::Indeterminate);   // P3
            }
        }

        // P4: err must not change the verdict — re-run with a fixed
        // contrasting message and assert equality.
        let baseline = is_cross_hierarchy_type_error(src, tgt, "");
        prop_assert_eq!(
            is_cross_hierarchy_type_error(src, tgt, &err),
            baseline,
        );
    }
}
```

Strategy notes for F4:
- `opt_bool()` enumerates the full `Option<bool>` domain (`None`, `Some(true)`,
  `Some(false)`); the `src × tgt` product is 9 states, which proptest covers
  trivially. A non-proptest exhaustive 9-row table test is an acceptable
  *addition* but not a substitute — the proptest is required to exercise P4
  against arbitrary `err` strings.
- `Classification` must derive `PartialEq` + `Debug` for `prop_assert_eq!`.
- The `prop_oneof!` for `err` deliberately injects the locale-fragile substring
  so P4 is a real test, not a vacuous one.

## 3. Caller-Routing Totality — `handle_edit` (O-4)

The §2 proptest proves the **pure classifier** `is_cross_hierarchy_type_error`
is total over `Option<bool> × Option<bool>` (P1–P3). That proof does **not**
cover totality of the **caller-side routing** in `handle_edit` — i.e. the
mapping from the caller's full input space to a single user-facing outcome.
This section closes that gap (adversarial Pass-3 finding O-4, process-gap) by
analysis; no proof code is required.

**Scope of the totality claim.** The claim below covers `handle_edit`'s
**complete error-path input space**, beginning with `edit_issue`'s own outcome
— not just the post-400 enrichment sub-path. The enrichment routing
(`get_issue` / `get_project_issue_types` / classifier) is entered **only after**
`edit_issue` itself fails with `HTTP 400`; that gate is the first decision point
of the argument.

The enrichment routing is well-defined by an **explicit call ordering** (now
stated in the BCs): once the HTTP-400 gate opens, `handle_edit` calls `get_issue`
**first**; `get_project_issue_types` is called **only if** `get_issue` succeeds.

**Live-code note on `get_project_issue_types` (CRITICAL-2).** Unlike `get_issue`,
`get_project_issue_types` (`src/api/jira/projects.rs:47-51`) does **not** surface
deserialization or missing-key failures as an `Err`. It applies
`.ok()).unwrap_or_default()`, so a `HTTP 200` whose body is malformed/unparseable
or omits the `issueTypes` key yields `Ok(vec![])` — an **empty list**, not an
error. A *fetch failure* from `get_project_issue_types` (→ Indeterminate, row R2)
therefore arises **only** from an HTTP/network error on the underlying GET. A
`200` with an unparseable body is **not** a fetch failure: it produces an empty
list, the target name is consequently "not in list", and the routing lands on the
typo-hint (unresolvable-name) outcome — row R3 — **not** Indeterminate. (This is
specific to `get_project_issue_types`; `get_issue` deserialization failure *does*
surface as an error and keeps its R1 fetch-failure routing.)

**`Option<IssueType>` flatten step (CRITICAL-1).** The classifier's first
argument `src_subtask: Option<bool>` is not read directly off the wire — the
caller must **flatten** an `Option<IssueType>` through `.subtask` to obtain it.
The source subtask flag originates from `issue.fields.issuetype`, which is
`Option<IssueType>` (`src/types/jira/issue.rs:62`), and `IssueType.subtask` is
itself `Option<bool>`. The caller computes `src_subtask` as
`issue.fields.issuetype.as_ref().and_then(|t| t.subtask)`. This flatten collapses
**two** distinct absences into the same `src_subtask: None`:

- **Outer `None`** — the `issuetype` object is wholly absent from the `get_issue`
  response (`issue.fields.issuetype` is `None`).
- **Inner `None`** — the `issuetype` object is present but omits the `subtask`
  field (`IssueType.subtask` is `None` via `#[serde(default)]`).

Both paths feed `src_subtask: None` into the classifier, and the classifier maps
`None` on either argument to `Indeterminate` (P3). The totality argument's step 5
below therefore operates on the **post-flatten** `Option<bool> × Option<bool>`
domain; this flatten is the explicit bridge from the `get_issue` response's
`Option<IssueType>` layer to the classifier's `Option<bool>` domain.

The classifier runs **only if** both calls return `HTTP 200` **and** the target
type name is found in the returned list. The **target name lookup** is a
**case-insensitive exact match on the issue-type `name`** against the
`get_project_issue_types` results (not a substring or prefix match, and not a
match on the issue-type `id`); this makes the "Target name in list?" partition
well-defined. The table below enumerates the caller's complete error-path input
space — starting with `edit_issue`'s outcome — and the exactly-one user-facing
outcome each input produces.

| # | `edit_issue` | `get_issue` | `get_project_issue_types` | Target name in list?<br>(case-insensitive exact `name` match) | Classifier `(src_subtask, tgt_subtask)` | User-facing outcome |
|---|--------------|-------------|---------------------------|----------------------|------------------------------------------|---------------------|
| R0a | succeeds | *not called* | *not called* | — | *not run* | **happy path** — edit applied (no enrichment; out of scope of this delta) |
| R0b | fails, NON-400 (401/403/5xx/network) | *not called* | *not called* | — | *not run* | **raw error surfaced unchanged** — no enrichment; the original `JrError` is propagated as-is (pinned by integration test #10) |
| R1 | fails, HTTP 400 | fails (`Result::is_err()`, any `Err` variant) | *not called* | — | *not run* | **Indeterminate** — the `extract_error_message`-processed 400 message text (`JrError::ApiError.message`) surfaced (pinned by integration test #9) |
| R2 | fails, HTTP 400 | 200 | HTTP/network failure (underlying GET errors) | — | *not run* | **Indeterminate** — the `extract_error_message`-processed 400 message text (`JrError::ApiError.message`) surfaced |
| R3 | fails, HTTP 400 | 200 | 200 (incl. malformed/unparseable body → empty list)<br>OR target name not in list | NO | *not run* | **typo hint** (unresolvable-name outcome; pinned by integration test #8) |
| R4 | fails, HTTP 400 | 200 | 200 | YES | `None` on either argument | **Indeterminate** — the `extract_error_message`-processed 400 message text (`JrError::ApiError.message`) surfaced |
| R5 | fails, HTTP 400 | 200 | 200 | YES | `Some/Some`, values differ | **CrossHierarchy hint** |
| R6 | fails, HTTP 400 | 200 | 200 | YES | `Some/Some`, values equal | **typo hint** |

**Note on the target-name lookup (m-2).** The "Target name in list?" decision
point (step 4 of the totality argument) is a **case-insensitive exact match on
the issue-type `name`** field. The caller compares the user-supplied target type
name against each `name` returned by `get_project_issue_types`, lower-casing both
sides; a row matches iff one entry is exactly equal under that fold. There is no
substring/prefix fallback and no `id`-based match on this path. This makes the
{not in list, in list} partition total and unambiguous: every target name lands
in exactly one branch.

**Note on the surfaced 400 text (M-2).** Rows R1, R2, and R4 surface the
`extract_error_message`-processed 400 message text — **not** the raw Atlassian
JSON error envelope. `JiraClient::parse_error` (`src/api/client.rs`) runs
`extract_error_message()` over the response bytes, so `JrError::ApiError.message`
carries only the extracted human-readable message, not the raw
`{"errors": ...}` / `{"errorMessages": [...]}` JSON body. "Indeterminate"
outcomes therefore present that processed message verbatim, with no
cross-hierarchy / typo enrichment appended.

`extract_error_message` is itself the composition
`sanitize_for_stderr(extract_error_message_raw(body))` — the raw extractor
followed by a stderr-sanitization pass (MAJOR-1). `sanitize_for_stderr` is
effectively a **no-op for plain-ASCII content**: it strips/escapes only
control and non-printable characters, leaving printable ASCII untouched.
Consequently, where this verification delta and the integration tests reason
about the *surfaced message text*, any plain-ASCII test substring is **stable**
through `extract_error_message` — the sanitization pass does not perturb it, so
substring assertions on plain-ASCII expected text are sound.

**Totality argument.** `handle_edit`'s complete error-path input space is
partitioned by four sequential, mutually exclusive decision points:

1. **`edit_issue` outcome** — exactly one of {succeeds, fails NON-400, fails
   HTTP 400}. "Succeeds" is the happy path R0a (no enrichment; out of scope of
   this delta). "Fails NON-400" (401/403/5xx/network) is R0b — the raw error is
   surfaced unchanged with no enrichment. Only "fails HTTP 400" proceeds to the
   enrichment routing. The HTTP-400 gate is evaluated by downcasting
   `edit_issue`'s `anyhow::Error` to `JrError::ApiError { status: 400, .. }`;
   any error that does not downcast to a 400 falls in R0b. (`get_issue` is
   provably never reached unless `edit_issue` fails with HTTP 400 — the explicit
   gate guarantees it.)
2. **`get_issue` outcome** — exactly one of {fails, 200}. The "fails" branch is
   R1; the "200" branch proceeds. **The fetch-failure gate here is
   `Result::is_err()` — any `Err` variant — NOT a status-code downcast (MAJOR-1).**
   This is deliberately a *different mechanism* from the step-1 HTTP-400 gate,
   which downcasts to `JrError::ApiError { status: 400 }`. The reason: `parse_error`
   (`src/api/client.rs:980-995`) does **not** produce `JrError::ApiError` for a 401
   — it produces `NotAuthenticated` / `InsufficientScope` — so a status-code
   downcast would miss auth failures. The Indeterminate fetch-failure detection on
   `get_issue` must therefore treat *every* `Err` (network error, deserialization
   failure, 401 `NotAuthenticated`, 403 `InsufficientScope`, 5xx `ApiError`, etc.)
   as a fetch failure → R1. (`get_project_issue_types` is provably never reached
   when `get_issue` fails — the explicit call ordering guarantees it.)
3. **`get_project_issue_types` outcome** — exactly one of {HTTP/network failure,
   200}. The "HTTP/network failure" branch is R2; the "200" branch proceeds. As in
   step 2, this fetch-failure detection is `Result::is_err()` (any `Err` variant),
   **not** a status-code downcast (MAJOR-1).
   Because `get_project_issue_types` does `.ok()).unwrap_or_default()`
   (`src/api/jira/projects.rs:47-51`), a `200` with a malformed/unparseable body
   or absent `issueTypes` key is **not** a fetch failure — it yields an empty
   list and flows into step 4 as the "not in list" branch (R3), **not** R2. Only
   an HTTP/network error on the underlying GET reaches R2.
4. **target name lookup** — exactly one of {not in list, in list}. "Not in
   list" is R3 — this branch is reached both when the list is non-empty but
   lacks the target name **and** when the list is empty (e.g. a `200` with an
   unparseable body, per step 3). "In list" proceeds to the classifier.
5. **`Option<IssueType>` flatten + classifier verdict** — before the classifier
   runs, the caller flattens `issue.fields.issuetype: Option<IssueType>` (CRITICAL-1)
   through `.subtask` via `issue.fields.issuetype.as_ref().and_then(|t| t.subtask)`
   to produce `src_subtask: Option<bool>`. Both a wholly-absent `issuetype` object
   (outer `None`) and a present-but-`subtask`-omitted object (inner `None`)
   collapse to `src_subtask: None`. With `src_subtask` (and `tgt_subtask`,
   flattened the same way from the resolved target issue type) thus in the
   `Option<bool>` domain, `is_cross_hierarchy_type_error` is itself total (P1–P3)
   over `Option<bool> × Option<bool>`, partitioning into exactly
   {Indeterminate (R4), CrossHierarchy (R5), SameCategory (R6)}.

Each decision point is a total partition of its branch's input subspace, and
the branches are nested disjointly. Therefore:

- **No input is unrouted** — every (`edit_issue` outcome, `get_issue` outcome,
  `get_project_issue_types` outcome, lookup result, classifier verdict) tuple
  lands in exactly one of R0a–R6 (the table is exhaustive: rows R0a–R6 cover the
  full Cartesian product once the HTTP-400, call-ordering, and lookup gates
  prune the unreachable combinations).
- **No input is double-routed** — the gate at each decision point is mutually
  exclusive, so no tuple satisfies the precondition of two rows.

**Dual-gate precedence — `--type` and `--no-parent` both set (CRITICAL-4).**
When both `--type` and `--no-parent` are supplied and `edit_issue` 400s,
`handle_edit`'s error block contains two independent enrichment arms: the
`--type` cross-hierarchy enrichment arm and the `--no-parent`
(`is_subtask_parent_error`) enrichment arm. These are evaluated in a **fixed
order**: the `--type` cross-hierarchy arm is evaluated **first**; the
`--no-parent` arm is evaluated **only if** the `--type` arm emits no hint
(i.e. the `--type` arm's outcome is an unenriched/Indeterminate result). The
two arms never both append text — first-hint-wins. The R0a–R6 table above
describes the `--type` arm; when `--no-parent` is also set, an
unenriched-error row (R0b/R1/R2/R4) is the precondition that hands control to
the `--no-parent` arm, and the R3/R5/R6 hint rows short-circuit it. This fixed
ordering keeps the routing deterministic with both flags set: each input still
maps to exactly one user-facing outcome.

`handle_edit`'s complete error-path routing is therefore **total and
deterministic**: every input maps to exactly one user-facing outcome (happy
path / unenriched error / typo hint / cross-hierarchy hint). R0b, R1, R2, and R4
all surface an *unenriched* error — i.e. no cross-hierarchy / typo text is
appended — but the two cases differ in what exactly is surfaced: R0b propagates
the original non-400 `JrError` as-is (no enrichment attempted at all, because
`edit_issue` failed with a non-400 error), whereas R1, R2, and R4 surface the
`extract_error_message`-processed 400 message text (`JrError::ApiError.message`,
**not** the raw Atlassian JSON error body — see the M-2 note above). R1, R2, and
R4 correspond to the two `Indeterminate` sub-causes of §2 (R1/R2 = Cause 1 fetch
failure; R4 = Cause 2, 200 + absent `subtask` flag — source- or target-side).

## 4. Fuzz / Kani Candidates — None

No fuzz target and no Kani harness are warranted for the #388 delta.

- **Kani:** The classifier's input domain is `Option<bool> × Option<bool>`
  (9 states) plus a `&str` that, per P4, has zero influence on the output. A
  9-row exhaustive unit test is a complete proof of the classification logic —
  Kani would add tooling cost with no coverage gain. There is no arithmetic, no
  array indexing, no unsafe code, no state machine in this delta.
- **Fuzz:** The only string input (`err`) is not parsed, indexed, or used for
  control flow (P4). There is no untrusted-input parser in the delta. The new
  HTTP calls (`get_issue`, `get_project_issue_types`) go through existing,
  already-exercised serde paths; the additive `IssueType.subtask:
  Option<bool>` field with `#[serde(default)]` cannot panic on absent or
  malformed input. Nothing in the delta presents a fuzzing surface.

Standard F6 hardening (full-tree `cargo test`, `cargo clippy -D warnings`,
`cargo deny check`, and `cargo mutants --in-diff` on the PR diff per
`docs/specs/cargo-mutants-policy.md`) remains in force — but no *new*
fuzz/Kani harness is added.

## 5. Consistency With the Sibling Helper `is_subtask_parent_error`

The new classifier is verified the same way as its sibling
`is_subtask_parent_error` (`src/cli/issue/create.rs:1159`), confirming the
approach is consistent with the existing project pattern:

- **Pure-function shape:** `is_subtask_parent_error` is a pure predicate (no
  I/O, no async, no client refs) tested via the inline `#[cfg(test)] mod tests`
  block. `is_cross_hierarchy_type_error` is likewise pure and gets the same
  inline treatment — with the addition of a proptest, placed in the dedicated
  `mod is_cross_hierarchy_type_error_proptests` submodule, because its input
  domain is a small total function ideal for property coverage (the file
  already hosts `build_labels_proptests` and `parse_field_kv_proptests`, so an
  inline proptest module is the established convention here, not a new one).
- **Behavioral pinning lives in integration tests:** `is_subtask_parent_error`'s
  user-visible behavior is pinned by `tests/issue_edit_no_parent.rs` (the T-06
  hint test). `is_cross_hierarchy_type_error`'s user-visible behavior is pinned
  by the new `tests/issue_edit_type_errors.rs` (10 functions) plus the
  strengthened T-06. Same two-layer model: pure logic = inline unit/proptest;
  wired behavior = wiremock integration test.
- **Kept distinct:** Per delta-analysis.md §Open Questions 3, the two helpers
  address different errors and MUST NOT be merged. The verification artifacts
  stay separate accordingly — the new proptest covers only the new function.

## 6. Pass-2 Adversarial Findings — Verification Impact

Two adversarial Pass-2 findings (F-2, F-5) touch this delta. Neither changes the
proptest strategy.

- **F-2 — unresolved target type name ⟹ typo hint is CALLER-side routing.**
  The F-2 fix lives entirely in `handle_edit` caller routing: when the target
  type name does not resolve to a known issue type, the caller emits a typo
  hint. It is **NOT** a change to the pure classifier
  `is_cross_hierarchy_type_error(src_subtask: Option<bool>, tgt_subtask: Option<bool>, err)`.
  The pure classifier's signature, its three-variant output, and its 9-state
  proptest (P1–P4 in §2 above) are **UNCHANGED** by the Pass-2 findings. The
  proptest strategy stands exactly as written — F-2 is verified by integration
  coverage of the caller route, not by the proptest.
- **F-5 — full `CROSS_HIERARCHY_HINT` string pinned by integration assertion.**
  The complete `CROSS_HIERARCHY_HINT` text is now pinned verbatim by a
  full-string integration assertion in `tests/issue_edit_type_errors.rs` (not
  merely the `JRACLOUD-27893` substring). This integration assertion is the
  authority for hint-text correctness. The proptest is unaffected: P4 still
  asserts that the `err` argument never influences classification, and the
  proptest still does **NOT** touch hint text — hint composition is a
  caller-side concern verified solely by the integration layer.

## 7. Pass-3 Adversarial Findings — Verification Impact

One adversarial Pass-3 finding (O-4) touches this delta. It does not change the
proptest strategy or the pure classifier.

- **O-4 — caller-routing totality (process-gap).** The §2 proptest proves the
  pure classifier total, but caller-side routing totality in `handle_edit` was
  not verified. Addressed by the new **§3 caller-routing totality table**,
  which enumerates `handle_edit`'s complete error-path input space — starting
  with `edit_issue`'s own outcome (succeeds / fails NON-400 / fails HTTP 400),
  then the post-400 enrichment routing (HTTP outcomes + classifier verdict) —
  and shows every input maps to exactly one user-facing outcome — no input
  unrouted, none double-routed — given the HTTP-400 gate and the explicit
  `get_issue`-then-`get_project_issue_types` call ordering. This is a
  documentation/analysis addition; no proof code is added. The pure classifier
  `is_cross_hierarchy_type_error`, its signature, and its 9-state P1–P4 proptest
  are **UNCHANGED** by O-4 — the gap and its remedy are entirely caller-side.

## 8. Pass-6 Adversarial Findings — Verification Impact

Four adversarial Pass-6 findings touch this delta. All are caller-routing /
coverage / live-code-accuracy matters; **none** change the proptest strategy or
the pure classifier.

- **CRITICAL-2 — `get_project_issue_types` does not error on a malformed body.**
  Live code (`src/api/jira/projects.rs:47-51`) applies `.ok()).unwrap_or_default()`,
  so a `200` with a malformed/unparseable body or absent `issueTypes` key yields
  `Ok(vec![])`, not `Err`. The §3 routing table and totality argument now
  classify a `get_project_issue_types` *fetch failure* (→ Indeterminate, row R2)
  as an HTTP/network error on the underlying GET **only**; a `200` with an
  unparseable body produces an empty list → "not in list" → typo-hint
  (unresolvable-name) row R3, not Indeterminate. `get_issue` deserialization
  failure is unaffected — it still surfaces as an error and keeps its R1 routing.
- **CRITICAL-4 — dual-gate precedence.** §3 now states the fixed evaluation
  order for `handle_edit`'s error block when both `--type` and `--no-parent` are
  set: the `--type` cross-hierarchy arm evaluates first, the `--no-parent` arm
  evaluates only if the `--type` arm emits no hint. First-hint-wins keeps the
  routing deterministic with both flags set.
- **MAJOR-3 — 8th integration test + citation accuracy.** The integration-test
  count is corrected 7 → 8 throughout; the new test #8
  `test_edit_type_unresolved_type_name_surfaces_typo_hint` covers the
  unresolvable-name → typo-hint outcome (table row R3), the routing-table row
  that previously had no named test. Row R3 now cites test #8.
- **MAJOR-1 — `extract_error_message` composition.** §3 now documents that
  `extract_error_message` = `sanitize_for_stderr(extract_error_message_raw(body))`
  and that `sanitize_for_stderr` is a no-op for plain-ASCII content, so
  plain-ASCII test substrings are stable through it.

The pure classifier `is_cross_hierarchy_type_error`, its
`Option<bool> × Option<bool>` signature, its three-variant `Classification`
output, and the 9-state P1–P4 proptest are **UNCHANGED** by all four Pass-6
findings — every remedy is caller-routing / coverage / live-code-accuracy.

## 9. Pass-7 Adversarial Findings — Verification Impact

Three adversarial Pass-7 findings touch this delta. All are caller-routing /
coverage / live-code-accuracy matters; **none** change the proptest strategy or
the pure classifier.

- **CRITICAL-1 — `Option<IssueType>` flatten step.** The §3 totality argument
  previously jumped straight to the classifier's `Option<bool> × Option<bool>`
  domain without showing how `src_subtask` is derived. §3 now states the explicit
  flatten: the source subtask flag comes from `issue.fields.issuetype`, which is
  `Option<IssueType>` (`src/types/jira/issue.rs:62`), and `IssueType.subtask` is
  `Option<bool>`. The caller flattens via
  `issue.fields.issuetype.as_ref().and_then(|t| t.subtask)`; both a wholly-absent
  `issuetype` object (outer `None`) and a present-but-`subtask`-omitted object
  (inner `None`) collapse to `src_subtask: None`. This flatten is now an explicit
  step of the totality argument (decision point 5), so the argument covers the
  `Option<IssueType>` outer layer, not only the classifier's `Option<bool>` domain.
- **MAJOR-1 — Indeterminate fetch-failure gate is `is_err()`, not a downcast.**
  §3 decision points 2 and 3 now state that the Indeterminate fetch-failure
  detection on `get_issue` / `get_project_issue_types` is `Result::is_err()` (any
  `Err` variant), **not** a status-code downcast. This is deliberately distinct
  from the step-1 HTTP-400 gate, which downcasts to
  `JrError::ApiError { status: 400 }`. The reason is that `parse_error`
  (`src/api/client.rs:980-995`) does not produce `JrError::ApiError` for a 401 —
  it produces `NotAuthenticated` / `InsufficientScope` — so a status-code downcast
  would miss auth failures on the fetch calls.
- **Test count 8 → 10.** Two new integration tests pin previously
  proof-by-analysis-only routing rows: test #9
  `test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error` (row R1 —
  `get_issue` itself fails) and test #10
  `test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment` (row R0b —
  `edit_issue` fails non-400). The integration-test count is corrected 8 → 10
  throughout, and rows R0b and R1 in the §3 routing table now each cite their own
  named test (R1 → #9, R0b → #10).

The pure classifier `is_cross_hierarchy_type_error`, its
`Option<bool> × Option<bool>` signature, its three-variant `Classification`
output, and the 9-state P1–P4 proptest are **UNCHANGED** by all three Pass-7
findings — every remedy is caller-routing / coverage / live-code-accuracy.

## Summary for F4

1. No VP-NNN document — BC-level anchoring (BC-3.4.010 / BC-3.4.011) only.
2. `is_cross_hierarchy_type_error` is pure over `Option<bool> × Option<bool>`
   and returns all three `Classification` variants (`None` ⟹ `Indeterminate`).
   Add ONE inline proptest asserting P1–P4; strategy `src/tgt in opt_bool()`
   (`prop_oneof![Just(None), Just(Some(true)), Just(Some(false))]`, 9 states),
   `err in prop_oneof![".*", <locale substring>, ""]`. P4 (err never changes the
   verdict) is the load-bearing property.
3. Caller-routing totality (§3): `handle_edit`'s complete error-path input
   space — `edit_issue` outcome (succeeds / fails NON-400 / fails HTTP 400)
   first, then the post-400 enrichment routing (HTTP outcomes + classifier
   verdict) — maps to exactly one user-facing outcome via the R0a–R6 table —
   total and deterministic, given the HTTP-400 gate and the explicit
   `get_issue`-first / `get_project_issue_types`-second call ordering.
4. No new fuzz target, no Kani harness — pure classification logic over 9
   states, no parser surface.
5. Mirror the `is_subtask_parent_error` pattern: inline pure-function tests +
   `tests/issue_edit_type_errors.rs` integration pins (10 functions). Both
   `Indeterminate` sub-causes have named integration coverage: Cause 1 (fetch
   5xx) by test #4; Cause 2 (200 + absent `subtask` flag) by test #6
   `test_edit_type_indeterminate_absent_subtask_flag_surfaces_raw_error`
   (source-side) and test #7
   `test_edit_type_indeterminate_absent_target_subtask_flag_surfaces_raw_error`
   (target-side). The unresolvable-name → typo-hint outcome (table row R3) is
   pinned by the new test #8
   `test_edit_type_unresolved_type_name_surfaces_typo_hint`. Routing rows R1 and
   R0b are pinned by new tests #9
   `test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error` and #10
   `test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment`
   respectively.
6. Pass-2 findings F-2 and F-5 do not alter the proptest: F-2 is CALLER-side
   `handle_edit` routing (the pure classifier and its P1–P4 proptest are
   unchanged); F-5's full-`CROSS_HIERARCHY_HINT` verbatim pin is an integration
   assertion — the proptest still does not touch hint text.
7. Pass-3 finding O-4 (caller-routing totality) is addressed by the §3 analysis
   table — documentation only; the pure classifier and its P1–P4 proptest are
   unchanged.
8. Pass-7 findings: §3 now shows the explicit `Option<IssueType>` flatten
   (`issue.fields.issuetype.as_ref().and_then(|t| t.subtask)`) bridging the
   `get_issue` response to the classifier's `Option<bool>` domain (CRITICAL-1);
   the Indeterminate fetch-failure gate on `get_issue` / `get_project_issue_types`
   is `Result::is_err()`, not a status-code downcast (MAJOR-1); the
   integration-test count is 10, with rows R1 and R0b now pinned by named tests
   #9 and #10. The pure classifier and its P1–P4 proptest remain unchanged.

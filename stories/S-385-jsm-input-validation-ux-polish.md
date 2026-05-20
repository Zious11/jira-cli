---
document_type: story
story_id: "S-385"
title: "JSM input validation UX polish: harmonize project-required error, guard empty --request-type, reject --markdown+--field description= conflict, move platform-flag warnings post-require_service_desk (closes #385)"
wave: feature-followup
status: ready
intent: enhancement
feature_type: backend
scope: standard
issue: 385
points: 5
priority: medium
tdd_mode: strict
estimated_effort: medium
depends_on: [S-384]  # S-384 (PR #394) modifies the same handle_jsm_create locus and tests/issue_create_jsm.rs; S-385 must branch from a develop HEAD that includes PR #394
bc_anchors:
  - BC-3.8.016
  - BC-3.8.017
  - BC-3.8.002
  - BC-3.8.010
  - BC-3.8.011
  - BC-3.8.003  # regression-pin only — not implemented or modified by this story; item-6 test (test_jsm_create_ambiguous_request_type_exits_64) must remain green unchanged
holdout_anchors:
  - H-NEW-JSM-RT-006
  - H-NEW-JSM-RT-007
nfr_anchors: []
adr_refs:
  - ADR-0014
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: ".factory/phase-f2-spec-evolution/prd-delta-385.md"
implementation_strategy: tdd
module_criticality: HIGH  # src/cli/issue/create.rs — core JSM create command path; all 4 fixes share handle_jsm_create locus
files_modified:
  - src/cli/issue/create.rs        # MODIFIED — all 4 fixes: harmonize error string (O-08-02), add empty-RT guard (O-08-04), add --markdown+--field description= conflict guard (O-08-06), remove pre-dispatch block ~64-96 + move 6 platform-flag warnings post-require_service_desk (O-08-07); update stale JsmCreateArgs rustdoc (~1812-1819) AND handle_jsm_create Steps doc-comment (~1839-1855) (AC-7)
  - tests/issue_create_jsm.rs      # MODIFIED — update 1 assertion (O-08-02) + 4 new test functions (O-08-04/06/07) + 1 double-emission pin (F-02)
  - src/cli/issue/helpers.rs       # OPTIONAL/CONDITIONAL — add pub(super) fn project_key_required_error(context: &str) -> JrError helper only if implementer chooses to deduplicate platform and JSM error strings via shared helper; NOT required if string is inlined at line 1891
  # Factory-artifact bookkeeping files (already updated by F3 story-writer; F4 implementer verifies only):
  # .factory/stories/STORY-INDEX.md  — S-385 already registered; verify total_stories=43; update last_updated only
  # .factory/sprint-state.yaml       — append S-385 entry under feature_followup_standalone block
breaking_change: false
# BC status: BC-3.8.016 and BC-3.8.017 produced in F2 (2026-05-20). BC-3.8.002, BC-3.8.010, BC-3.8.011 modified in F2 (2026-05-20). All BCs sealed.
# F3 story produced after F2 convergence confirmed complete (guard scripts all exit 0).
---

# S-385 — JSM Input Validation UX Polish

## Source of Truth

F1 delta analysis: `.factory/phase-f1-delta-analysis/delta-analysis-385.md` (approved).
F2 PRD delta: `.factory/phase-f2-spec-evolution/prd-delta-385.md` (COMPLETE, 2026-05-20).
Canonical Guard Ordering: `.factory/specs/prd/bc-3-issue-write.md` §`Canonical Guard Ordering — handle_jsm_create` (SINGLE SOURCE OF TRUTH for step ordering).

This story delivers all 4 fixes from GitHub issue #385 as a single atomic commit. All 4 fixes share
the `handle_jsm_create` code locus — they cannot and must not be split into separate stories.

## Problem Statement

Four LOW/UX observations from adversary pass-08, all validated against `develop` HEAD (PR #381 merged),
are bundled into a single polish story:

**O-08-02:** `handle_jsm_create` at `create.rs:1891` returns `"project is required for JSM request
creation"` — 6 words, no actionable affordances. The platform sibling at line 146-148 returns a
17-word string mentioning `--project`, `.jr.toml`, and `jr project list`. The JSM path must be
harmonized to the richer form (BC-3.8.002, modified in F2).

**O-08-04:** `--request-type ""` (empty string) falls through to `resolve_jsm_request_type_id` and
`partial_match("", &candidates)`, which returns `Ambiguous` for any non-empty candidate list —
producing "Ambiguous request type — N matches" instead of a clear "request type cannot be empty".
A new guard at step 1 of the Canonical Guard Ordering fixes this (BC-3.8.016, new in F2).

**O-08-06:** `--markdown` + `--field description=<value>` produces a desync in
`JsmRequestBuilder::build()`: `is_adf_request = true` is set, then `extra_fields["description"]`
overwrites the ADF value with a plain string. This may produce a JSM 400 or silently drop ADF
formatting. A parse-time rejection guard at step 2 of the Canonical Guard Ordering fixes this
(BC-3.8.017, new in F2).

**O-08-07:** The 6 platform-only flag warnings (`--type`, `--team`, `--points`, `--parent`, `--to`,
`--account-id`) currently fire in `handle_create` at lines ~64-96 — BEFORE `handle_jsm_create` is
called. Lines ~64-66 are a leading explanatory comment (`// Emit stderr warnings for platform-only
flags ... Warnings fire BEFORE dispatch so they appear / even if dispatch errors out later...`) whose
"Warnings fire BEFORE dispatch" claim becomes FALSE after O-08-07; lines ~67-96 are the six warning
`eprintln!` blocks. On a non-JSM project the user sees both the warning and the non-JSM project error.
BC-3.8.010 and BC-3.8.011 (both modified in F2) require the warnings to fire at step 5 of the
Canonical Guard Ordering — inside `handle_jsm_create` AFTER `require_service_desk` returns `Ok`.
The entire block at lines ~64-96 (leading comment + warning blocks) MUST be removed (single-site
requirement F-02).

### Canonical Guard Ordering (from `bc-3-issue-write.md` SINGLE SOURCE OF TRUTH)

```
Step 0: Project-key resolution (BC-3.8.002) — may exit 64, no HTTP
Step 1: BC-3.8.016 — empty/whitespace-only --request-type guard — exit 64, no HTTP
Step 2: BC-3.8.017 — --markdown + --field description= conflict guard — exit 64, no HTTP
Step 3: --markdown requires --description guard (pre-existing) — exit 64, no HTTP
Step 4: require_service_desk (BC-3.8.002) — exit 64 on non-JSM project, no HTTP to servicedeskapi
Step 5: BC-3.8.010/011 platform-only flag warnings (--type, --team, --points, --parent, --to,
        --account-id) — fire ONLY after require_service_desk returns Ok
Step 6: Request-type resolution (numeric bypass, resolve_jsm_request_type_id, partial_match),
        parse_field_kv, POST /rest/servicedeskapi/request
```

## Behavioral Contracts

| BC ID | File | Title | Clause(s) |
|-------|------|-------|-----------|
| BC-3.8.016 | `bc-3-issue-write.md` | `--request-type ""` or whitespace-only exits 64 before `require_service_desk` with explicit message | postconditions 1-4 |
| BC-3.8.017 | `bc-3-issue-write.md` | `--markdown` + `--field description=<value>` rejected at top of `handle_jsm_create`; exit 64 | postconditions 1-5 |
| BC-3.8.002 | `bc-3-issue-write.md` | JSM body uses `requestFieldValues` map; project-required error harmonized | Errors clause (O-08-02 harmonized string) |
| BC-3.8.010 | `bc-3-issue-write.md` | `--type` is IGNORED with stderr warning when `--request-type` is set | Warning position postcondition (O-08-07) |
| BC-3.8.011 | `bc-3-issue-write.md` | Platform-only flags ignored on JSM path emit stderr warnings | Warning position postcondition (O-08-07) |
| BC-3.8.003 | `bc-3-issue-write.md` | Ambiguous request-type partial match → exit 64 with disambiguation hint | **Regression-pin only** — not implemented or modified by this story; item-6 test (`test_jsm_create_ambiguous_request_type_exits_64`) must remain green unchanged |

## Acceptance Criteria

### AC-1 — Harmonized JSM project-required error string (O-08-02)
(traces to BC-3.8.002 Errors clause, harmonized string postcondition)

The error string at `create.rs:1891` (and at the corresponding helper site if extracted) MUST be
updated from `"project is required for JSM request creation"` to the harmonized form:

```
Project key is required for JSM request creation. Use --project or configure .jr.toml. Run "jr project list" to see available JSM projects.
```

The updated string carries the `--project` / `.jr.toml` / `jr project list` affordances and
sentence-cases the opening, while preserving "for JSM request creation" as context. Exit code
remains 64. No HTTP is issued before the error.

The existing test `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` in
`tests/issue_create_jsm.rs` (Required Test Deliverable item 4) MUST have its assertion updated to
assert the new harmonized string, NOT the old terse string.

**Test:** `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` (UPDATED assertion — BC-3.8.002).

### AC-2 — Empty / whitespace-only `--request-type` exits 64 before `require_service_desk` (O-08-04)
(traces to BC-3.8.016 postconditions 1-4 / H-NEW-JSM-RT-006)

When `--request-type` is the empty string `""` or a whitespace-only string such as `"   "`:

- Exit 64.
- stderr contains (assert via `contains`): `request type cannot be empty`
- stdout is empty.
- No HTTP calls issued. The guard fires at Canonical Guard Ordering step 1, before `require_service_desk`
  (step 4). The numeric-bypass check and `partial_match` (both in step 6) are never reached.

The guard MUST use `request_type_arg.trim().is_empty()` to cover both the empty-string case and the
whitespace-only case (EC-3.8.016-1). A guard using `.is_empty()` alone (without `.trim()`) would
pass the `""` primary case but fail the whitespace-only boundary case.

**Tests (both cases MANDATORY — Required Test Deliverable item 1):**
`test_jsm_create_empty_request_type_exits_64` in `tests/issue_create_jsm.rs`. MUST test BOTH
`--request-type ""` and `--request-type "   "` (whitespace-only) within the same test function or
as parameterized sub-cases. Holdout H-NEW-JSM-RT-006.

### AC-3 — `--markdown` + `--field description=` conflict rejected before `require_service_desk` (O-08-06)
(traces to BC-3.8.017 postconditions 1-5 / H-NEW-JSM-RT-007)

When `--markdown` is set AND any `--field` arg token's raw key (substring before first `=`, NO
trimming, NO case-folding) is EXACTLY `"description"`:

- Exit 64.
- stderr is ONE canonical single-sentence message. **The implementation's `eprintln!` MUST emit the
  FULL canonical single-sentence conflict message — copy it BYTE-FOR-BYTE from the CANONICAL SOURCE
  in `bc-3-issue-write.md` BC-3.8.017 body (look for the line marked CANONICAL SOURCE in that BC's
  postconditions). The F2 PRD delta does NOT contain the full verbatim string — only the BC body
  does.** The three `contains` fragments below are TEST-ASSERTION slices of that one sentence, NOT
  the implementation string — do not assemble the implementation string from the fragments.
  Three `contains` assertions required for the test:
  - (a) `` `--field description=...` cannot be combined with `--markdown` ``
  - (b) `may result in a JSM 400 error or silently dropped ADF formatting`
  - (c) `` Pass `--description` with `--markdown`, or omit `--markdown` `` (remediation clause; pins
    "errors always suggest what to do next" convention)
- stdout is empty.
- No HTTP calls issued. Guard fires at Canonical Guard Ordering step 2, before `require_service_desk`
  (step 4).

The guard uses an EXACT, case-SENSITIVE, no-trim key match: `raw_key == "description"`. `--field
Description=X` (capital D) does NOT trigger the guard (EC-3.8.017-3). A `--field` token with no `=`
character does NOT trigger the guard (EC-3.8.017-5).

Note on guard ordering vs step 3: this guard (step 2) fires BEFORE the existing `--markdown`-requires-
`--description` guard (step 3). Therefore `--markdown --field description=X` with NO `--description`
flag correctly triggers this guard's conflict message — NOT the "requires --description" message.

**Test (Required Test Deliverable item 2):**
`test_jsm_create_markdown_field_description_conflict_exits_64` in `tests/issue_create_jsm.rs`.
Must assert all three `contains` slices. Holdout H-NEW-JSM-RT-007.

### AC-4 — Platform-flag warnings suppressed on non-JSM project (O-08-07)
(traces to BC-3.8.010 warning-position postcondition / BC-3.8.011 warning-position postcondition)

When a non-JSM project is used with `--request-type <non-empty>` and any of the platform-only flags
(`--type`, `--team`, `--points`, `--parent`, `--to`, `--account-id`):

- Only the non-JSM project error (from `require_service_desk`) appears on stderr.
- The platform-flag warning MUST NOT appear on stderr.
- Exit code 64 (from `require_service_desk`).

Consequence of this change: the entire pre-dispatch block in `handle_create` (lines ~64-96 in the
pre-#385 code — the leading explanatory comment at ~64-66 AND the six warning `eprintln!` blocks at
~67-96) MUST be removed. The leading comment's "Warnings fire BEFORE dispatch" claim is false after
O-08-07 and must not be left stranded. Warnings exist at exactly ONE site: step 5 of the Canonical
Guard Ordering inside `handle_jsm_create`, AFTER `require_service_desk` returns `Ok`.

The existing test `test_jsm_create_type_flag_ignored_with_warning` MUST remain GREEN unchanged — it
uses a JSM project (HELP) and pins that the warning fires on the JSM success path.

**Test (Required Test Deliverable item 3):**
`test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project` in `tests/issue_create_jsm.rs`.

**Mock topology (non-JSM project mocks required — NOT zero-mock):** Item 3 is a MOCKED-HTTP test
(unlike zero-mock AC-2/AC-3). Reaching `require_service_desk` (step 4) requires issuing its project
meta HTTP call. Use the H-NEW-JSM-RT-002 mock topology (see `holdout-scenarios.md §H-NEW-JSM-RT-002`):
  - `GET /rest/api/3/project/PROJ` → project meta with `typeKey = "software"` (non-JSM project)
  - `GET /rest/servicedeskapi/servicedesk` → `{values: []}` (no service desk entry for PROJ)
  - `POST /rest/servicedeskapi/request` with `expect(0)` — MUST NOT be called
  - `POST /rest/api/3/issue` with `expect(0)` — MUST NOT be called

**Exit-code precondition:** assert `exit 64` (from `require_service_desk`). This is a required
precondition before asserting warning absence — a test that exits at step 1 (empty `--request-type`)
or step 2/3 instead of step 4 would reach step 5 never having been a possibility, silently voiding
the warning-suppression pin.

**Assertion mechanism:**
- `exit 64` FIRST (verifies the test reached `require_service_desk` and failed there)
- stderr `!contains` (or `count == 0`) for `"warning: --type is ignored"` — the absence check
- stderr `contains` the non-JSM-project error (verifies the correct failure site)
Use a non-empty `--request-type` value (e.g. `--request-type "Get IT Help"`) so the test exercises
the step-4 `require_service_desk` path; an empty `--request-type` would exit at step 1 per
BC-3.8.016 before reaching step 4, and the warning-suppression assertion would be trivially true
for the wrong reason.

### AC-5 — Existing green tests remain green (regression baseline)
(traces to BC-3.8.010 JSM-path pin / BC-3.8.003 genuine-ambiguity pin)

These two tests MUST remain GREEN and UNMODIFIED:
- `test_jsm_create_type_flag_ignored_with_warning` (Required Test Deliverable item 5) — warning still
  fires on JSM project path; O-08-07 does not suppress it on the success path.
- `test_jsm_create_ambiguous_request_type_exits_64` (Required Test Deliverable item 6) — uses
  `--request-type "Bug"` (non-empty prefix matching multiple candidates); BC-3.8.016's empty guard
  does not affect this test.

### AC-6 — Single-site warning emission: no double-emission on JSM success path (F-02)
(traces to BC-3.8.010 single-site requirement / BC-3.8.011 single-site requirement)

On a successful JSM create path with all 6 platform-only flags set across two invocations (required
due to `--to`/`--account-id` clap mutual exclusion), each of the 6 warning strings MUST appear
EXACTLY ONCE on stderr. Double-emission from two code sites is a defect pinned by this test.

**Mock topology (FULL JSM success-path mocks required — NOT zero-mock):** This test is the
OPPOSITE of AC-2/AC-3. Both invocations MUST reach canonical step 5 (warnings fire), which requires
`require_service_desk` to succeed (step 4) and the POST to succeed (step 6). Mount the FULL JSM
happy-path mock set for each invocation — use the same helper pattern as the existing
`test_jsm_create_type_flag_ignored_with_warning` (or H-NEW-JSM-RT-004's topology):
  - `GET /rest/api/3/project/{KEY}` → service-desk-type project meta
  - `GET /rest/servicedeskapi/servicedesk` → service desk list with matching project
  - `GET /rest/servicedeskapi/servicedesk/{id}/requesttype` → request-type list containing the used name
  - `POST /rest/servicedeskapi/request` with `expect(1)` → HTTP 201 success response

**Exit-code precondition:** BOTH invocations MUST assert `exit 0` BEFORE asserting warning counts.
A non-zero exit means step 5 was never reached — the warning-count assertions would silently void
the double-emission pin. An inert test that exits 64 and then asserts `count == 0` passes trivially.

**Assertion mechanism — occurrence count, NOT `contains`:** A plain `contains` assertion is
FUNCTIONALLY INERT for this test — it passes whether a warning appears once OR twice, making it
unable to detect the double-emission defect it exists to catch. MUST use an occurrence-count
assertion, e.g.:
```rust
let count = stderr.matches("warning: --type is ignored").count();
assert_eq!(count, 1, "expected exactly one --type warning; got {count}");
```
Apply the same `count == 1` check for EVERY warning substring in each invocation's stderr. A
`contains`-only assertion here is a test quality defect equivalent to having no test at all for
the double-emission property.

Implementation note: `--to` and `--account-id` are clap-mutually-exclusive on `issue create`
(confirmed via `src/cli/mod.rs`: `conflicts_with = "account_id"` / `conflicts_with = "to"`), so all
six flags cannot appear in a single invocation. This test MUST use TWO invocations:
- Invocation A: carries `--type <T> --team <id> --points <n> --parent <key> --to <id>` (5 flags);
  assert exit 0, then assert each of the 5 warning substrings appears EXACTLY ONCE via count.
- Invocation B: carries `--account-id <id>` (1 flag); assert exit 0, then assert the
  `--account-id`-ignored warning substring appears EXACTLY ONCE via count.

This is distinct from BC-3.8.011's idempotency contract (one warning per repeated logical flag
occurrence) — that covers duplicate flag instances, not duplicate code sites.

**Test (Required Test Deliverable item 7):**
`test_jsm_create_platform_flag_warnings_emit_once_on_success` in `tests/issue_create_jsm.rs`.

### AC-7 — Update ALL stale rustdoc/doc-comment in `create.rs` that describe pre-#385 warning behavior
(originates from adversary pass-17 observation O-1; supports the O-08-07 single-site warning invariant but is a documentation-hygiene task — no BC postcondition governs rustdoc text; BC-3.8.010/011 are listed for context only, not as the tracing clause)

AC-7 covers **two** stale doc blocks in `src/cli/issue/create.rs`. The implementer MUST read both
full blocks before editing — `~`-hedged line numbers are approximate; the actual positions may shift
as O-08-04/06/07 code is inserted above them. Read the source, locate both blocks by content, update
both.

**Block (a) — `JsmCreateArgs` rustdoc (~lines 1812-1819):**
Currently states that the platform-flag warnings are "handled BEFORE dispatch in `handle_create`".
After O-08-07 this is false. MUST be corrected to state that warnings fire at step 5 of the
Canonical Guard Ordering inside `handle_jsm_create` — AFTER `require_service_desk` returns `Ok`,
before request-type resolution.

**Block (b) — `handle_jsm_create` function doc-comment Steps block (~lines 1839-1855):**
Currently has a line `/// Steps (BC-3.8.001..010):` and a step that reads approximately
`/// 4. If --type is also set → emit stderr warning (BC-3.8.010). Do NOT error.`
Two updates required:
- The `Steps (BC-3.8.001..010)` range must be updated to `BC-3.8.001..017` (or equivalent) to
  reflect the two new BCs added by this story.
- The new steps 1 and 2 (empty-`--request-type` guard / BC-3.8.016, and `--markdown`+`--field
  description=` conflict guard / BC-3.8.017) must be added to the Steps list.
- The step describing the `--type` warning MUST be updated to reflect the O-08-07 single-site
  placement: the warning now fires at canonical step 5 (after `require_service_desk` returns `Ok`),
  NOT at step 4 as the pre-#385 doc states.

This is a documentation-only change within the implementation commit; no test is required for either
rustdoc update itself. However, the O-08-07 implementation (AC-4 + AC-6) is the gate: the rustdoc
must not be updated to reflect the new behavior unless O-08-07 is actually implemented.

## Required Test Deliverables Summary

| # | Test Function | Type | BC Pin | AC | Mocks | Exit precondition | Assertion mechanism |
|---|--------------|------|--------|----|-------|-------------------|---------------------|
| 1 | `test_jsm_create_empty_request_type_exits_64` | NEW | BC-3.8.016 / H-NEW-JSM-RT-006 | AC-2 | ZERO mocks (guard fires at step 1 before any HTTP; any call = test failure) | assert exit 64 | `contains` "request type cannot be empty"; MUST cover BOTH `""` and `"   "` inputs |
| 2 | `test_jsm_create_markdown_field_description_conflict_exits_64` | NEW | BC-3.8.017 / H-NEW-JSM-RT-007 | AC-3 | ZERO mocks (guard fires at step 2 before any HTTP; any call = test failure) | assert exit 64 | 3 × `contains` slices of ONE canonical sentence (see AC-3); `contains` is correct here (pinning presence, not count) |
| 3 | `test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project` | NEW | BC-3.8.010 / BC-3.8.011 | AC-4 | MOCKED — H-NEW-JSM-RT-002 topology: non-JSM project meta (`typeKey="software"`), service-desk list returning `{values:[]}`, `POST` endpoints `expect(0)` | assert exit 64 FIRST (verifies step-4 was reached) | `!contains` (or `count==0`) for warning string; `contains` for non-JSM-project error |
| 4 | `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` | UPDATED assertion | BC-3.8.002 | AC-1 | Existing mock set unchanged (do NOT change test structure) | assert exit 64 (unchanged) | Update one `contains` assertion to harmonized string; all other assertions unchanged |
| 5 | `test_jsm_create_type_flag_ignored_with_warning` | MUST REMAIN GREEN | BC-3.8.010 (JSM path) | AC-5 | Existing full JSM success-path mocks (JSM project HELP) — unchanged | assert exit 0 (unchanged) | Existing `contains` for warning string — unchanged; `contains` is correct (pinning presence on success path) |
| 6 | `test_jsm_create_ambiguous_request_type_exits_64` | MUST REMAIN GREEN | BC-3.8.003 | AC-5 | Existing mocks (service-desk project + request-type list with ≥2 entries matching "Bug") — unchanged | assert exit 64 (unchanged) | Existing assertions unchanged; BC-3.8.016 guard not triggered (non-empty input) |
| 7 | `test_jsm_create_platform_flag_warnings_emit_once_on_success` | NEW | BC-3.8.010 / BC-3.8.011 (single-site) | AC-6 | FULL JSM success-path mocks for BOTH invocations (see AC-6; same topology as H-NEW-JSM-RT-004 / `test_jsm_create_type_flag_ignored_with_warning`); `POST` `expect(1)` per invocation | assert exit 0 FIRST per invocation (verifies step 5 was reached) | occurrence COUNT (`stderr.matches(substr).count() == 1`) for each warning substring — NOT `contains`; a plain `contains` is inert for double-emission detection |

All 7 are MANDATORY acceptance-gate deliverables. Items 1 and 2 are also Holdout scenarios
H-NEW-JSM-RT-006 and H-NEW-JSM-RT-007 respectively. Items 5 and 6 are regression pins that must
not be modified.

**Note: AC-7 is a documentation-only change (rustdoc correction) with no test deliverable — it is
intentionally absent from this table. It is tracked via the Tasks checklist and Files-to-Touch list.
A TDD implementer scanning this table for all testable ACs has the complete set at items 1–7 above;
AC-7 requires no Red-Gate stub or test function.**

## O-08-07 Threading Requirement (Implementation Note H-03)

Moving all six platform-flag warnings to step 5 inside `handle_jsm_create` requires the
warning-triggering flag values to be in scope at that site. As of the pre-#385 baseline,
`JsmCreateArgs` destructures only: `project`, `request_type`, `summary`, `description`,
`description_stdin`, `priority`, `labels`, `markdown`, `on_behalf_of`, `field_pairs`. It does
NOT carry `issue_type` (`--type`), `team` (`--team`), `points` (`--points`), `parent` (`--parent`),
`to` (`--to`), or `account_id` (`--account-id`).

The behavioral contracts (BC-3.8.010/BC-3.8.011) constrain WHEN warnings fire (step 5: after
`require_service_desk` returns `Ok`, before request-type resolution). They do NOT constrain the
threading mechanism. Acceptable mechanisms include:
- Extending `JsmCreateArgs` with the six missing fields
- Passing them as additional parameters to `handle_jsm_create`
- Extracting them from a broader `CreateArgs` reference passed alongside `JsmCreateArgs`

The implementer MUST choose a mechanism that makes all six values visible at the step-5 warning
site. The spec does not prescribe which mechanism to use. The choice must NOT introduce a clippy
warning — if a mechanism causes `too_many_arguments` or similar, refactor rather than suppress.

**Single-site requirement (F-02):** The entire pre-dispatch block in `handle_create` (lines ~64-96
in the pre-#385 code — the leading explanatory comment at ~64-66 AND the six warning `eprintln!`
blocks at ~67-96) MUST be REMOVED. The leading comment's "Warnings fire BEFORE dispatch" claim is
false after O-08-07; removing only lines 67-96 leaves a stranded incorrect comment. The warnings
must exist at exactly ONE site: canonical step 5 inside `handle_jsm_create`. Double-emission is a
defect (pinned by Required Test Deliverable item 7).

## Files to Touch

| File | Action | Risk |
|------|--------|------|
| `src/cli/issue/create.rs` | MODIFY — (1) harmonize JSM project-required error string at ~line 1891 (O-08-02); (2) add empty-RT guard at step 1 before `require_service_desk` (O-08-04); (3) add `--markdown` + `--field description=` conflict guard at step 2 (O-08-06); (4) remove entire pre-dispatch block from `handle_create` lines ~64-96 (leading comment at ~64-66 + six warning `eprintln!` blocks at ~67-96) and re-emit all 6 warnings at step 5 inside `handle_jsm_create` after `require_service_desk` returns `Ok` (O-08-07); (5) update ALL stale rustdoc/doc-comment in `JsmCreateArgs` (~lines 1812-1819) and `handle_jsm_create` Steps block (~lines 1839-1855) — see AC-7 | LOW-MEDIUM — O-08-02 changes a pinned verbatim string (test must be updated); O-08-04/06 are new guards; O-08-07 reorders code but preserves all BC behaviors on JSM path |
| `tests/issue_create_jsm.rs` | MODIFY — (1) update assertion in `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` (AC-1); (2) add `test_jsm_create_empty_request_type_exits_64` (AC-2); (3) add `test_jsm_create_markdown_field_description_conflict_exits_64` (AC-3); (4) add `test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project` (AC-4); (5) add `test_jsm_create_platform_flag_warnings_emit_once_on_success` (AC-6) | LOW — one assertion update; 4 new tests with no modification to existing passing logic |
| `src/cli/issue/helpers.rs` | OPTIONAL/CONDITIONAL — add `pub(super) fn project_key_required_error(context: &str) -> JrError` only if implementer chooses to deduplicate the platform and JSM project-required error strings. If inlined at line 1891, do not touch this file. | LOW if extracted (additive only) |

**Files NOT to touch:** `src/api/jsm/requests.rs` (conflict guard is in `handle_jsm_create`, not
`JsmRequestBuilder::build()`), `src/partial_match.rs` (empty-string guard is in the caller, not
`partial_match`), `src/error.rs` (no new error variant or constant needed), BC spec files (sealed
in F2 — do not re-edit unless adversary finds implementation-BC discrepancy).

## Regression Baseline

Tests that MUST remain green unmodified after this delivery:

- `test_jsm_create_type_flag_ignored_with_warning` — JSM project + `--type` + success; warning present
- `test_jsm_create_ambiguous_request_type_exits_64` — non-empty partial match; genuine ambiguity
- All other tests in `tests/issue_create_jsm.rs` (39 total pre-#385)
- `tests/issue_write_holdouts.rs` — H-NEW-JSM-RT-004 (BC-3.8.010 warning on JSM path) must stay green
- `tests/jsm_request_api.rs` — `JsmRequestBuilder` proptest C.2 (BC-3.8.006 description/ADF presence); `build()` is unchanged
- `tests/issue_commands.rs` — platform create coverage; platform create path is byte-for-byte unchanged
- `tests/issue_create_json.rs` — platform create JSON shape; unaffected
- `tests/queue.rs` — `require_service_desk` interface is unchanged; must stay green
- `tests/requesttype_commands.rs` — `require_service_desk` interface is unchanged; must stay green

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~5 k |
| F2 PRD delta (`.factory/phase-f2-spec-evolution/prd-delta-385.md`) | ~8 k |
| BC files (BC-3.8.002, BC-3.8.010, BC-3.8.011, BC-3.8.016, BC-3.8.017 sections in `bc-3-issue-write.md`) | ~6 k |
| Holdout scenarios §H-NEW-JSM-RT-006 and §H-NEW-JSM-RT-007 | ~2 k |
| `src/cli/issue/create.rs` (read `handle_create` lines 60-110 + `JsmCreateArgs` ~lines 1812-1875 + `handle_jsm_create` ~lines 1876-2010) | ~8 k |
| `src/cli/issue/helpers.rs` (scan for insertion site if helper is extracted) | ~2 k |
| `tests/issue_create_jsm.rs` (read existing test structure: full file for insertion site context, existing `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` assertion location) | ~15 k |
| Tool outputs + `cargo test` + `cargo clippy` output | ~5 k |
| **Total** | **~51 k** |

Well within single-agent context. No split required. LOC delta estimate: ~30 lines in `create.rs`
(4 new guards + reordering + rustdoc update), ~250 lines in `tests/issue_create_jsm.rs` (1 assertion
update + 4 new test functions), optionally ~8 lines in `helpers.rs`.

## Tasks

- [ ] Read F2 PRD delta (`.factory/phase-f2-spec-evolution/prd-delta-385.md`) — capture Canonical Guard Ordering, 7 Required Test Deliverables, and canonical verbatim strings for BC-3.8.002, BC-3.8.016, BC-3.8.017
- [ ] Read `bc-3-issue-write.md` §`Canonical Guard Ordering — handle_jsm_create` and §BC-3.8.002/010/011/016/017 — extract exact postcondition text and message strings
- [ ] Read `src/cli/issue/create.rs` lines 60-110 (`handle_create` warning block) — identify lines to remove (O-08-07 single-site requirement)
- [ ] Read `src/cli/issue/create.rs` `JsmCreateArgs` (~lines 1812-1875) + `handle_jsm_create` (~lines 1876-2010) — understand destructured fields, project-key resolution site (~line 1891), and existing guard/flow structure
- [ ] Read `tests/issue_create_jsm.rs` — locate `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` assertion (O-08-02 update target); locate insertion sites for 4 new tests
- [ ] Decide O-08-07 threading mechanism: extend `JsmCreateArgs`, pass additional params, or pass broader `CreateArgs` ref — choose the approach that avoids clippy warnings
- [ ] Decide O-08-02 helper extraction: inline harmonized string at ~line 1891, OR extract `project_key_required_error(context: &str)` into `helpers.rs` if deduplication benefit is worth it
- [ ] Add step-1 empty-RT guard (`request_type_arg.trim().is_empty()`) inside `handle_jsm_create` AFTER project-key resolution, BEFORE `require_service_desk` — exit 64, stderr "request type cannot be empty", no HTTP (O-08-04 / BC-3.8.016)
- [ ] Add step-2 `--markdown` + `--field description=` conflict guard inside `handle_jsm_create` AFTER step 1, BEFORE `require_service_desk` — exit 64, `eprintln!` emits the FULL canonical single-sentence conflict message copied BYTE-FOR-BYTE from `bc-3-issue-write.md` BC-3.8.017 body (CANONICAL SOURCE); the 3 `contains` fragments in AC-3 are test-assertion slices only, NOT the implementation string (O-08-06 / BC-3.8.017)
- [ ] Update project-key resolution error string at ~line 1891 to harmonized form: `"Project key is required for JSM request creation. Use --project or configure .jr.toml. Run \"jr project list\" to see available JSM projects."` (O-08-02 / BC-3.8.002)
- [ ] Remove entire pre-dispatch block from `handle_create` (lines ~64-96 — leading explanatory comment at ~64-66 AND six warning `eprintln!` blocks at ~67-96); re-emit all 6 warnings at step 5 inside `handle_jsm_create` after `require_service_desk` returns `Ok`, before step-6 request-type resolution; do NOT leave the ~64-66 comment stranded (O-08-07 / BC-3.8.010/011)
- [ ] Update ALL stale rustdoc/doc-comment in `create.rs` that describe pre-#385 warning behavior (AC-7): (a) `JsmCreateArgs` rustdoc (~lines 1812-1819) — correct "handled BEFORE dispatch in `handle_create`" to reflect step-5 single-site placement; (b) `handle_jsm_create` Steps doc-comment (~lines 1839-1855) — update `Steps (BC-3.8.001..010)` BC range, add steps for BC-3.8.016 and BC-3.8.017, update the `--type`-warning step to reflect canonical step-5 post-`require_service_desk` placement; read both full blocks in source before editing
- [ ] Update assertion in `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` to assert the harmonized error string (Required Test Deliverable item 4)
- [ ] Add `test_jsm_create_empty_request_type_exits_64` covering BOTH `--request-type ""` and `--request-type "   "` inputs; zero HTTP mocks mounted; assert exit 64 + stderr contains "request type cannot be empty" (Required Test Deliverable item 1)
- [ ] Add `test_jsm_create_markdown_field_description_conflict_exits_64` with `--request-type 17 --markdown --field description="plain text override"`; zero HTTP mocks; assert exit 64 + 3 stderr `contains` checks (Required Test Deliverable item 2)
- [ ] Add `test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project`: mount H-NEW-JSM-RT-002 mock topology (non-JSM project meta, service-desk list `{values:[]}`, POST endpoints `expect(0)`); assert exit 64 FIRST; then assert warning `!contains` (or `count==0`) AND non-JSM error `contains` (Required Test Deliverable item 3)
- [ ] Add `test_jsm_create_platform_flag_warnings_emit_once_on_success`: mount FULL JSM success-path mocks for BOTH invocations (same pattern as `test_jsm_create_type_flag_ignored_with_warning` / H-NEW-JSM-RT-004); assert exit 0 FIRST per invocation; then assert each warning substring count == 1 via `stderr.matches(substr).count()` — NOT plain `contains` (Required Test Deliverable item 7)
- [ ] Verify `test_jsm_create_type_flag_ignored_with_warning` remains GREEN unchanged (Required Test Deliverable item 5)
- [ ] Verify `test_jsm_create_ambiguous_request_type_exits_64` remains GREEN unchanged (Required Test Deliverable item 6)
- [ ] Run `cargo test --test issue_create_jsm` — all 7 required tests pass; no regressions
- [ ] Run `cargo test` — full suite green; `tests/issue_write_holdouts.rs` H-NEW-JSM-RT-004 still green; `tests/queue.rs` and `tests/requesttype_commands.rs` still green
- [ ] Run `cargo clippy -- -D warnings` — zero warnings; no `#[allow]` suppressions; refactor if needed
- [ ] Run `cargo build --release` — succeeds
- [ ] Run `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh` — both exit 0
- [ ] Per-story adversary 3/3 CLEAN before push

## Previous Story Intelligence

This story immediately follows S-384 (JSM 401 auth-aware hints, merged PR #394) and is adjacent to
issue-288-pr4-dispatch (JSM dispatch fork, PR #381) and S-383 (platform-inverse warnings, PR #390).

Key lessons carried forward:

- **From issue-288-pr4-dispatch (PR #381):** `handle_jsm_create` in `src/cli/issue/create.rs` is
  the primary change target. The function is complex (~130 LOC) — read the full function before
  making any changes, not just the lines surrounding the insertion points. Context matters.

- **From S-383 (platform-inverse warnings):** The warning pattern (`eprintln!("warning: ...")`) in
  `handle_create` is idiomatic in this codebase. The O-08-07 fix is a MOVE operation (from
  `handle_create` to `handle_jsm_create`), not a deletion — the warning strings and their conditions
  are unchanged; only the site changes.

- **From S-384:** Threading flag values through function boundaries is a common pattern here. If
  extending `JsmCreateArgs`, add the six fields as `Option<T>` to avoid breaking existing call sites.
  If passing as additional parameters, prefer a small struct over a long parameter list to avoid
  the `too_many_arguments` clippy warning.

- **Verbatim string discipline (from multiple prior stories):** Copy canonical error strings
  BYTE-FOR-BYTE from the authoritative source. Any character deviation causes adversarial failures.
  Source mapping for this story:
  - BC-3.8.002 harmonized project-required string → F2 PRD delta §BC-3.8.002 "New verbatim" block
    (also in `bc-3-issue-write.md` BC-3.8.002 Errors clause). Contains an escaped inner double-quote
    (`\"jr project list\"`) — ensure the Rust string literal escapes it correctly.
  - BC-3.8.016 "request type cannot be empty" message → `bc-3-issue-write.md` BC-3.8.016 body
    (CANONICAL SOURCE). Also quoted in F2 PRD delta §BC-3.8.016 postconditions.
  - BC-3.8.017 conflict message → `bc-3-issue-write.md` BC-3.8.017 body ONLY (CANONICAL SOURCE).
    The F2 PRD delta does NOT contain the full verbatim sentence — only the BC body does. The three
    `contains` fragments in AC-3 are test-assertion slices, NOT the full implementation string.
    Read the BC-3.8.017 body and copy the full sentence from there.

- **Zero-HTTP-mock tests (from S-384 and H-NEW-JSM-RT-006/007):** For AC-2 (step-1 guard) and
  AC-3 (step-2 guard), do NOT mount any wiremock stubs. A regression moving either guard below
  `require_service_desk` (step 4) would cause the test to fail with "unexpected HTTP call" — the
  zero-mock setup is the regression detector.

- **`JR_AUTH_HEADER` seam (from SD-002):** Not directly needed for this story (no auth-path
  changes), but confirm existing tests' `JR_AUTH_HEADER` usage is not disturbed by the warning
  reordering.

## Architecture Compliance Rules

Extracted from `bc-3-issue-write.md` §`Canonical Guard Ordering` and architecture conventions:

1. **Guard ordering is non-negotiable.** Steps 0→1→2→3→4→5→6 must be preserved in code. Reordering
   guards changes observable behavior and violates the holdout setup assumptions (H-NEW-JSM-RT-006
   expects step-1 guard with zero HTTP; H-NEW-JSM-RT-007 expects step-2 guard with zero HTTP).

2. **Conflict guard placement in `handle_jsm_create`, not `JsmRequestBuilder::build()`.**
   The O-08-06 conflict guard MUST live in `handle_jsm_create` after `parse_field_kv` scan.
   Moving it into `build()` would require extending the `JsmRequestBuilder` proptest suite
   (`tests/jsm_request_api.rs`) which is explicitly NOT in scope for this story.

3. **Single-site warning emission.** The entire pre-dispatch block in `handle_create` (lines ~64-96
   — the leading explanatory comment at ~64-66 AND the six warning `eprintln!` blocks at ~67-96)
   MUST be completely removed. Removing only lines 67-96 leaves a stranded comment whose "Warnings
   fire BEFORE dispatch" claim is false after O-08-07. The warnings exist at exactly ONE site:
   step 5 in `handle_jsm_create`. Partial removal (e.g., only removing `--type`) creates an
   asymmetry violation.

4. **No new modules.** All changes are additive guards and reordering within existing functions.
   The optional `project_key_required_error` helper is the ONLY new function permitted, and only in
   the already-existing `src/cli/issue/helpers.rs`.

5. **No `#[allow]` suppressions.** Per CLAUDE.md convention: if clippy warns, refactor to fix the
   root cause. The most likely clippy concern is `too_many_arguments` if `handle_jsm_create`
   gains additional parameters for the threading mechanism — refactor to a helper struct if needed.

6. **Exit code 64 for all new guards.** Both BC-3.8.016 (empty RT) and BC-3.8.017 (markdown conflict)
   exit 64 via the existing `JrError::UserError` variant. No new error variant is needed.

## Library & Framework Requirements

No new dependencies. All changes use stdlib + existing project types.

- `wiremock` in `tests/issue_create_jsm.rs` is already a dev-dependency — use the same import
  pattern as existing tests. For AC-2 and AC-3 (zero-HTTP tests), do NOT mount any stubs; use
  the same wiremock server setup pattern but with no `.register().await` calls.
- `assert_cmd` + `predicates` are already present for binary-level assertions.
- No version pins change; no `Cargo.toml` edits.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `src/cli/issue/create.rs` | Modify | 4 guard/reorder changes + rustdoc update; ~30 LOC net; all changes within `handle_create` and `handle_jsm_create` |
| `tests/issue_create_jsm.rs` | Modify | 1 assertion update + 4 new test functions; ~250 LOC net |
| `src/cli/issue/helpers.rs` | Modify (conditional) | Add `pub(super) fn project_key_required_error(context: &str) -> JrError` if deduplication chosen; ~8 LOC net |
| `.factory/stories/STORY-INDEX.md` | Already updated by F3 story-writer | S-385 row already registered in Story Manifest + Feature Followup table. `total_stories` is already 43 (corrected from a prior overcount per PG-385-6). The F4 implementer MUST NOT increment `total_stories` again — S-385 is already counted. Verify `total_stories: 43` and that the S-385 manifest row is present; update `last_updated` only. |
| `.factory/sprint-state.yaml` | Modify | Append S-385 entry under `feature_followup_standalone` block |

## Branch / PR Plan

- Branch: `feat/issue-385-jsm-input-validation-ux-polish`
- Target: `develop`
- Commit style: `feat(jsm): input validation UX polish — harmonize project-required error, guard empty --request-type, reject --markdown+--field description= conflict, move platform-flag warnings post-require_service_desk (#385)`
- PR closes #385
- CHANGELOG entry recommended: error-message improvements and new input validation guards are
  user-visible changes for JSM users

**Why `breaking_change: false` despite a CHANGELOG entry:** No previously-successful
`jr issue create` invocation changes outcome. Specifically:
- BC-3.8.016 (`--request-type ""` guard) adds exit-64 on an input that previously produced
  the misleading "Ambiguous request type — N matches" error — still an error, different message.
- BC-3.8.017 (`--markdown` + `--field description=` guard) adds exit-64 on an input that
  previously produced a desynced/malformed request body (may have resulted in a JSM 400 or
  silently dropped ADF formatting) — the rejection is additive-on-an-already-broken path.
- O-08-02 improves the wording of an existing exit-64 error message — same exit code, richer text.
- O-08-07 deduplicates and relocates platform-flag warnings — warnings still fire on the JSM
  success path; the only change is suppression on the non-JSM error path (where they were
  misleading). No success-path outcome changes.

The `breaking_change: false` field and the CHANGELOG recommendation are therefore consistent:
the changes are user-visible (improved messages, new clear rejections) but additive-on-error-paths
only. A CHANGELOG entry is warranted for discoverability; a breaking-change marker is not.

## Per-Story Delivery Notes

- Demos (Step 5) are LOCAL-ONLY per `docs/demo-evidence/` gitignore convention.
- Per-story adversary 3/3 CLEAN required before push.
- F2 is COMPLETE (2026-05-20) — guard scripts all exit 0. BC files are sealed. Do NOT re-edit
  BC files unless the adversary finds a discrepancy between the BC body and the implementation.
- The `check-bc-cumulative-counts.sh` guard must exit 0 post-edit. The BC count delta from F2
  is already committed (+2 definitional: BC-3.8.016 and BC-3.8.017; total 573→575). Both guard
  scripts should already exit 0 without additional spec-file edits.
- Test 1 (`test_jsm_create_empty_request_type_exits_64`) requires BOTH `""` and `"   "` inputs
  in the same test — the whitespace-only case pins the `.trim()` call per EC-3.8.016-1.
- Test 7 (`test_jsm_create_platform_flag_warnings_emit_once_on_success`) requires TWO invocations
  due to `--to`/`--account-id` clap mutual exclusion. Verify each warning appears exactly once, not
  zero times (that would be a regression) and not twice (that would be the double-emission bug).

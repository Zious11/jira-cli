# Demo Evidence Report — S-606-1

**Story:** S-606-1 — `jr issue list --component` filter (bare / `not:` / `none` / `all:` forms)
**Branch:** `feature/S-606-1-issue-list-component-filter` (worktree `.worktrees/S-606-1`)
**HEAD at recording time:** `3408d16d` (test(issue): S-606-1 pin OR input-order + verbatim no-project message + tidy AC-003 negatives)
**Tool:** VHS (terminal recordings)
**Build:** `cargo build` debug — `jr 0.7.0-dev.1`
**Seams used:** `JR_BASE_URL`, `JR_AUTH_HEADER`, `JR_CONFIG_DIR`, `JR_CACHE_DIR`
**Acceptance criteria count:** 17

---

## Why this story's evidence is test-heavy

`--component`'s *happy-path* forms (bare OR-composition, `not:` OR-EMPTY, `all:` AND-composition,
resolver zero-match/ambiguous-match failures) all require a real project-existence check and a
real `GET /rest/api/3/project/{key}/components` resolver round-trip before any JQL is composed.
Recording those live would mean either (a) fabricating a live Jira response, which this task's
brief explicitly forbids, or (b) standing up a mock HTTP server as part of the demo artifact,
which this repo's existing demo-evidence convention (S-604-1, S-604-3, S-576-*) already treats as
**test-derived evidence**, not a VHS recording — the wiremock harness *is* the fixture-backed
demonstration, and the passing integration test *is* the proof.

What **is** genuinely offline and independent of any backend is the **pre-flight guard layer**:
five of `--component`'s ACs (`none` combined with another value, repeated `all:`, `all:` mixed
with a bare value, `none` with no project scope, a bare/`not:`/`all:` value with no project scope)
are evaluated purely from the CLI-supplied argument list, before any HTTP call — including the
project-existence check — is even attempted (BC-2.1.020/021/022, `VP-COMPONENT-013`/`-015`). These
are recorded live below with `JR_BASE_URL` pointed at a closed local port: if the guard did NOT
fire pre-flight, the command would hang or fail with a connection-refused error instead of
returning the documented message and exit code. The recordings prove the zero-HTTP guarantee
end-to-end, not just via the wiremock `.expect(0)` assertion.

Every AC below is either (a) a live VHS recording of real `jr` binary output, or (b) cited to a
specific, currently-passing integration test in `tests/issue_commands.rs` with the exact JQL
substring or stderr message the test asserts. No live Jira output is fabricated anywhere in this
report.

---

## Recordings

| File | GIF | WEBM | ACs Covered |
|------|-----|------|-------------|
| `AC-HELP-component-flag-surface.tape` | `AC-HELP-component-flag-surface.gif` | `AC-HELP-component-flag-surface.webm` | CLI surface for all four `--component` forms (bare/`not:`/`none`/`all:`) |
| `AC-007-010-011-combination-rejections.tape` | `AC-007-010-011-combination-rejections.gif` | `AC-007-010-011-combination-rejections.webm` | AC-007, AC-010, AC-011 |
| `AC-008-015-project-scope-required.tape` | `AC-008-015-project-scope-required.gif` | `AC-008-015-project-scope-required.webm` | AC-008, AC-015 |

All three recordings run the real debug `jr` binary with `JR_BASE_URL` pointed at a closed port
(`http://127.0.0.1:9999`, nothing listening) and a well-formed `JR_AUTH_HEADER`. Every command
shown exits 64 with the documented message **without hanging or producing a connection error**,
which is itself the live proof that these five guards fire before any HTTP attempt.

---

## AC Coverage Map

### AC-001 — Bare `--component` repeated → single OR-composed clause (BC-2.1.018 postcondition 1)

**Evidence type:** Integration test (passing)
**Test:** `test_bc_2_1_018_issue_list_component_repeated_or_composed_single_clause`

`jr issue list --project FOO --component Backend --component Frontend` composes ONE clause
`component in (10001, 10002)` (input order preserved) — never two separate `component in (10001)
AND component in (10002)` clauses. A companion regression test,
`test_bc_2_1_018_issue_list_component_or_preserves_input_order_not_sorted`, inverts the input
order (`Frontend` before `Backend`) and asserts the clause reads `component in (10002, 10001)` —
proving the order is genuinely input-preserved, not id-sorted.

**Test result:** PASS

---

### AC-002 — Single bare value stays `IN`, not rewritten to `=` (BC-2.1.018 EC-2.1.018-1)

**Evidence type:** Integration test (passing)
**Test:** `test_bc_2_1_018_issue_list_component_single_value_stays_in_clause`

`--component Backend` alone → `component in (10001)`. Test asserts the clause is present AND
asserts `component = 10001` is absent from the composed JQL.

**Test result:** PASS

---

### AC-003 — `not:<NAME>` composes the full OR-EMPTY form (BC-2.1.019 Postcondition 1)

**Evidence type:** Integration test (passing)
**Test:** `test_bc_2_1_019_issue_list_component_not_composes_or_empty_form`

`--component not:Frontend` → `(component not in (10002) OR component is EMPTY)` — the full
parenthesized form, never a bare `not in`. The test's final assertion counts occurrences of the
bare inner clause vs. the fully-wrapped form and asserts they're equal, so a regression that
emitted an unwrapped `not in` alongside the wrapped one would be caught.

**Test result:** PASS

---

### AC-004 — Multiple `not:` values group into ONE clause (BC-2.1.019 EC-2.1.019-1)

**Evidence type:** Integration test (passing)
**Test:** `test_bc_2_1_019_issue_list_component_multiple_not_single_group`

`--component not:Backend --component not:Frontend` → single clause `(component not in (10001,
10002) OR component is EMPTY)`. Test asserts exactly one `OR component is EMPTY` occurrence in the
composed JQL (not two).

**Test result:** PASS

---

### AC-005 — Bare + `not:` coexist, AND-joined, bare first (BC-2.1.018 Precondition 3 / BC-2.1.019 Postcondition 2)

**Evidence type:** Integration test (passing)
**Test:** `test_bc_2_1_018_issue_list_component_bare_and_not_coexist_bare_first`

`--component Backend --component not:Frontend` → `component in (10001) AND (component not in
(10002) OR component is EMPTY)` — both clauses present, AND-joined, bare clause ordered first.

**Test result:** PASS

---

### AC-006 — `none` composes `component is EMPTY` with ZERO resolver HTTP (BC-2.1.020 Postcondition 1)

**Evidence type:** Integration test (passing)
**Test:** `test_bc_2_1_020_issue_list_component_none_zero_resolver_http`

`jr issue list --project FOO --component none` → `component is EMPTY`. The test mounts a
`GET /rest/api/3/project/FOO/components` mock with `.expect(0)` — proving the resolver GET never
fires for `none` specifically (VP-COMPONENT-015), even though the project-existence check still
runs (project scope IS still required — see AC-008).

A companion case-insensitivity test, `test_bc_2_1_020_issue_list_component_none_case_insensitive`,
pins `--component NONE` and `--component None` against the identical zero-resolver-HTTP behavior.

**Test result:** PASS (both tests)

---

### AC-007 — `none` rejects combination with any other `--component` value (BC-2.1.020 Behavior)

**Evidence type:** VHS recording + integration test

**Recording:** `AC-007-010-011-combination-rejections.gif` (segment 1)

```
$ jr --no-input issue list --project FOO --component none --component Backend
Error: --component none cannot be combined with other --component values.
```
Exit code: 64. `JR_BASE_URL` points at a closed port with nothing listening — the command returns
this message immediately rather than hanging or reporting a connection failure, proving the
rejection fires before any HTTP attempt (zero HTTP, per BC-2.1.020).

**Test:** `test_bc_2_1_020_issue_list_component_none_combination_rejected` — asserts exit 64,
zero HTTP (via `s606_1_expect_zero_http`'s catch-all `.expect(0)` mocks on every GET/POST), and
the exact message above.

**Test result:** PASS

---

### AC-008 — `none` still requires project scope despite zero resolver HTTP (BC-2.1.020 Precondition 2 / EC-2.1.020-3)

**Evidence type:** VHS recording + integration test

**Recording:** `AC-008-015-project-scope-required.gif` (segment 1)

```
$ jr --no-input issue list --component none
Error: --component none requires --project (or a configured default project) to avoid an unrestricted org-wide search.
```
Exit code: 64. Run from a directory with no `.jr.toml`, no `--project`. `none` is NOT exempt from
project-scoping despite needing zero resolver HTTP — this is the load-bearing distinction the
story's Previous-Story-Intelligence table calls out explicitly.

**Test:** `test_bc_2_1_020_issue_list_component_none_requires_project_scope` — asserts exit 64,
zero HTTP, and the exact message above.

**Test result:** PASS

---

### AC-009 — `all:<N1>,<N2>` composes AND-joined repeated equality, not `IN` (BC-2.1.021 Postcondition 1)

**Evidence type:** Integration test (passing)
**Test:** `test_bc_2_1_021_issue_list_component_all_and_composed_repeated_equality`

`--component all:Backend,Frontend` → `component = 10001 AND component = 10002`. Test asserts both
the repeated-equality form is present AND that `component in (` is absent from the composed JQL —
confirming `all:` never degrades to the OR-semantic `IN` operator.

**Test result:** PASS

---

### AC-010 — Repeated `all:` occurrences rejected (BC-2.1.021 Precondition 1)

**Evidence type:** VHS recording + integration test

**Recording:** `AC-007-010-011-combination-rejections.gif` (segment 2)

```
$ jr --no-input issue list --project FOO --component all:X --component all:Y
Error: --component all: may only be specified once; comma-separate multiple names within one all: value.
```
Exit code: 64, zero HTTP (closed-port proof as above).

**Test:** `test_bc_2_1_021_issue_list_component_repeated_all_prefix_rejected` — asserts exit 64,
zero HTTP, and the exact message above.

**Test result:** PASS

---

### AC-011 — `all:` mixed with a bare value rejected (BC-2.1.021 Precondition 2 / EC-2.1.021-2)

**Evidence type:** VHS recording + integration test

**Recording:** `AC-007-010-011-combination-rejections.gif` (segment 3)

```
$ jr --no-input issue list --project FOO --component all:Backend --component Frontend
Error: --component all: cannot be combined with other --component values.
```
Exit code: 64, zero HTTP (closed-port proof as above).

**Test:** `test_bc_2_1_021_issue_list_component_all_mixed_with_bare_rejected` — asserts exit 64,
non-empty stderr, zero HTTP, and the exact message above.

**Test result:** PASS

---

### AC-012 — `all:` with a single name degenerates to one-term equality (BC-2.1.021 EC-2.1.021-1)

**Evidence type:** Integration test (passing)
**Test:** `test_bc_2_1_021_issue_list_component_all_single_name_degenerates`

`--component all:Backend` (single name, no comma) → `component = 10001`. Test asserts this AND
that `component in (10001)` (the bare-form clause shape) is absent — proving `all:Backend` and
`--component Backend` are genuinely different code paths that happen to be functionally similar,
not the same code path aliased.

**Test result:** PASS

---

### AC-013 — Zero-match resolver failure → exit 64, alphabetically-sorted candidate list (BC-2.1.022 Behavior)

**Evidence type:** Integration test (passing) — requires a live resolver round-trip (project
existence + component list GET), out of scope for an offline VHS recording per this task's brief.
**Test:** `test_bc_2_1_022_issue_list_component_unknown_name_zero_search`

`--component BadName` against a project whose only components are Frontend/Backend (deliberately
fixture-ordered as Frontend-then-Backend, to prove the "Available:" list is genuinely sorted by
the implementation rather than passed through as-is) → exit 64:
```
Component 'BadName' not found in project FOO. Available: Backend, Frontend.
```
The test mounts `POST /rest/api/3/search/jql` with `.expect(0)` — proving the JQL search is NEVER
called (VP-COMPONENT-013).

**Test result:** PASS

---

### AC-014 — Ambiguous resolver failure → exit 64, sorted candidate list (BC-2.1.022 Behavior)

**Evidence type:** Integration test (passing) — same resolver-round-trip scoping note as AC-013.
**Test:** `test_bc_2_1_022_issue_list_component_ambiguous_name_zero_search`

`--component Amb` against fixture components "Ambition" (20002) and "Amber" (20001), deliberately
mounted in reverse-alphabetical order → exit 64:
```
Ambiguous component 'Amb'. Matches: Amber, Ambition.
```
Zero JQL search calls (`.expect(0)` on `POST /rest/api/3/search/jql`).

**Test result:** PASS

---

### AC-015 — No project scope (bare/`not:`/`all:`) → exit 64 before the resolver GET (BC-2.1.022 EC-2.1.022-1)

**Evidence type:** VHS recording + integration test

**Recording:** `AC-008-015-project-scope-required.gif` (segment 2)

```
$ jr --no-input issue list --component Backend
Error: --component requires --project (or a configured default project) to resolve component names.
```
Exit code: 64. Run from a directory with no `.jr.toml`, no `--project`. `JR_BASE_URL` points at a
closed port — the immediate, correct-message response (rather than a hang or connection-refused
error) proves the guard fires BEFORE the resolver GET is even attempted.

**Test:** `test_bc_2_1_022_issue_list_component_no_project_scope_exits_64_before_get` — asserts
exit 64, zero HTTP, and the exact message above.

**Test result:** PASS

---

### AC-016 — Clause ordering: `component` after `asset`, before date-range clauses (BC-2.1.007 amendment)

**Evidence type:** Integration test (passing)
**Test:** `test_bc_2_1_007_issue_list_component_clause_ordering_after_asset_before_dates`

`jr issue list --project FOO --assignee me --component Backend --created-after 2026-01-01` composes
`assignee = currentUser()`, `component in (10001)`, and `created >= "2026-01-01"` in that relative
order. Test locates each clause's substring index in the composed JQL and asserts
`assignee_idx < component_idx < created_idx` — a structural ordering proof, not a fragile exact
string match, so the test survives unrelated clause-text changes elsewhere in the query.

**Test result:** PASS

---

### AC-017 — Reserved-syntax collisions short-circuit before name resolution (BC-2.1.019/020/021 documentation)

**Evidence type:** Integration test (passing) — three sub-scenarios in one test function.
**Test:** `test_bc_2_1_019_020_021_reserved_syntax_collisions_short_circuit_documented`

Proves, structurally, that a component literally named `"none"`, `"not:Deprecated"`, or
`"all:Backend"` is unreachable via the corresponding `--component` form:

- (a) A component literally named `"none"` exists in the fixture (id `40001`) with a resolver GET
  mounted `.expect(0)`; `--component none` still composes `component is EMPTY` — the keyword
  short-circuits before name resolution could ever see the literal `"none"` component.
- (b) Fixture contains both `"Deprecated"` (30001) and `"not:Deprecated"` (30002);
  `--component not:Deprecated` resolves to `30001` (the prefix strips and resolves `"Deprecated"`),
  never `30002`.
- (c) Analogous proof for `all:Backend` vs. a literal component named `"all:Backend"`.

This documents the limitation per BC-2.1.019 EC-2.1.019-3 / BC-2.1.020 EC-2.1.020-4 / BC-2.1.021
EC-2.1.021-3 — not a bug, a reserved-prefix collision with a documented workaround via raw
`--jql`.

**Test result:** PASS

---

## CLI Surface Recording

### `jr issue list --help` — `--component` flag documents all four forms

**Recording:** `AC-HELP-component-flag-surface.gif`

```
      --component <COMPONENT>
          Filter by component name (repeatable, OR-combined). Prefix forms: `not:<NAME>`
          excludes (issues with no component are still included), `none` matches issues with
          zero components (must be the only occurrence), `all:<N1>,<N2>` requires every listed
          component (AND-combined; at most one `all:` occurrence). See BC-2.1.018..022
```
Confirms the flag is wired into `jr issue list` and its help text documents bare/`not:`/`none`/
`all:` forms exactly as specified in BC-2.1.018 through BC-2.1.022.

---

## Artifact Index

| File | Type | ACs Covered |
|------|------|-------------|
| `AC-HELP-component-flag-surface.tape` | VHS script | CLI surface (all four forms) |
| `AC-HELP-component-flag-surface.gif` / `.webm` | VHS recording | CLI surface |
| `AC-007-010-011-combination-rejections.tape` | VHS script | AC-007, AC-010, AC-011 |
| `AC-007-010-011-combination-rejections.gif` / `.webm` | VHS recording | AC-007, AC-010, AC-011 |
| `AC-008-015-project-scope-required.tape` | VHS script | AC-008, AC-015 |
| `AC-008-015-project-scope-required.gif` / `.webm` | VHS recording | AC-008, AC-015 |
| `evidence-report.md` | This report | All 17 ACs |

**Integration tests in `tests/issue_commands.rs` (component-filter subset):** 19 tests
(`test_bc_2_1_018_*`, `test_bc_2_1_019_*`, `test_bc_2_1_020_*`, `test_bc_2_1_021_*`,
`test_bc_2_1_022_*`, `test_bc_2_1_007_issue_list_component_*`), all PASS — cover AC-001 through
AC-017 plus two Step-4.5 hardening regressions (input-order preservation, `none` case
insensitivity) not individually numbered as ACs.

---

## Summary

| AC | Description | Evidence | Status |
|----|-------------|----------|--------|
| AC-001 | Bare repeated → single OR clause, input order preserved | Test | PASS |
| AC-002 | Single bare value stays `IN` | Test | PASS |
| AC-003 | `not:` full OR-EMPTY form | Test | PASS |
| AC-004 | Multiple `not:` → one group | Test | PASS |
| AC-005 | Bare + `not:` coexist, bare first | Test | PASS |
| AC-006 | `none` → `IS EMPTY`, zero resolver HTTP | Test | PASS |
| AC-007 | `none` combination rejected | VHS + Test | PASS |
| AC-008 | `none` requires project scope | VHS + Test | PASS |
| AC-009 | `all:` AND-composed repeated equality | Test | PASS |
| AC-010 | Repeated `all:` rejected | VHS + Test | PASS |
| AC-011 | `all:` + bare mixed rejected | VHS + Test | PASS |
| AC-012 | `all:` single-name degenerates | Test | PASS |
| AC-013 | Zero-match → exit 64, sorted list, zero search | Test | PASS |
| AC-014 | Ambiguous → exit 64, sorted candidates, zero search | Test | PASS |
| AC-015 | No project scope (bare/`not:`/`all:`) → exit 64 | VHS + Test | PASS |
| AC-016 | Clause ordering: after `asset`, before date-range | Test | PASS |
| AC-017 | Reserved-syntax collisions short-circuit | Test | PASS |

**Coverage: 17/17 ACs demonstrated (5 via live VHS recording of real `jr` binary output; 17 via
citation to a currently-passing integration test — several ACs carry both).**

## Test Run Summary

```
cargo test --test issue_commands
test result: ok. 112 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

(112 = full `tests/issue_commands.rs` suite, including this story's 19 component-filter tests and
all pre-existing `issue list` tests — confirming no regression to prior filter behavior.)

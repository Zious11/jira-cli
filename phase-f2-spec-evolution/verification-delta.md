---
document_type: f2-verification-delta
phase: phase-f2-spec-evolution
producer: formal-verifier
issue: 288
status: draft
timestamp: 2026-05-18
project: jira-cli
mode: BROWNFIELD
intent: feature
inputs:
  - ".factory/phase-f1-delta-analysis/delta-analysis-288.md"
  - ".factory/phase-f1-delta-analysis/business-analyst-input-288.md"
  - ".factory/specs/"
  - ".cargo/mutants.toml"
  - "docs/specs/cargo-mutants-policy.md"
  - "src/partial_match.rs"
  - "src/jql.rs"
  - "src/duration.rs"
  - "src/api/pagination.rs"
  - "src/api/jsm/queues.rs"
  - "src/api/jira/users.rs"
---

# F2 Verification Delta — Issue #288

## 1. VP-Directory Confirmation

**Result: CONFIRMED — no VP directory exists.** A direct inspection of
`.factory/specs/` shows only two subdirectories: `prd/` and `domain-spec/`.
There is no `verification-architecture/`, no `verification-properties/`, no
`VP-INDEX.md`, and no per-property VP-NNN files. The F1 BA input was correct:
"No VP directory exists; property correctness is anchored at BC level."

`.factory/specs/` actual contents:

```
.factory/specs/
├── .gitkeep
├── domain-spec/
├── phase-1-consistency-audit-r2.md
├── phase-1-consistency-audit.md
└── prd/
```

Property-level correctness on this project is anchored by:

- **BC bodies** in `.factory/specs/prd/bc-*.md` (functional contracts).
- **Integration tests** under `tests/` (BC-level end-to-end).
- **Inline `#[cfg(test)] mod proptests` blocks** in `src/*.rs` for pure
  helpers. Confirmed precedent (3 modules):
  - `src/duration.rs:160` — `mod proptests` (worklog duration parser).
  - `src/jql.rs:365` — `mod proptests` (`escape_value` no-unescaped-quote
    invariant).
  - `src/partial_match.rs:149` — `mod proptests` (`partial_match` never
    panics, exact-match always found, etc.).
- **Mutation testing** via `.cargo/mutants.toml` + the `mutants` CI job
  enforcing a 90% kill-rate on PR-diff scope (see `docs/specs/cargo-mutants-policy.md`).

This F2 verification delta therefore catalogues proptest and mutation
coverage to be produced in F6, scoped to the 18 new BCs (BC-3.8.001..010 +
BC-X.12.001..008) and the two modified BCs (BC-1.3.023, BC-3.3.001). No
"VP" artifacts are minted; F6 hardening enforces the catalogue below
against the in-tree code.

---

## 2. Proptest Catalogue

Each entry below specifies (a) the function under test, (b) the file where
the inline `#[cfg(test)] mod proptests` block will live (alongside the
unit tests for the same module), (c) the property invariant, (d) the BC
references it covers, and (e) the proptest strategy.

The proptest pattern is the one already established by `src/jql.rs` and
`src/partial_match.rs`: a single `mod proptests` block at the bottom of
the module, gated by `#[cfg(test)]`, importing `proptest::prelude::*`,
using `prop_assert!` / `prop_assert_eq!` for assertions, and `proptest!`
macro blocks for the properties.

### Candidate Set (5 proptest properties across 2 files)

#### Group A — `--field NAME=VALUE` parser (`src/cli/issue/create.rs`)

The `--field` flag is **net-new** on `issue create` (grep confirms zero
prior occurrences). The parser is a string-split helper that converts
`Vec<String>` of `NAME=VALUE` items into a `HashMap<String, serde_json::Value>`
suitable for the `requestFieldValues` body field of `POST /rest/servicedeskapi/request`
(BC-3.8.007). Implementation will live in `cli/issue/create.rs` (or in a
new private `parse_field_kv` helper there); the inline `mod proptests`
block goes in the same file. Existing test conventions in `create.rs`
already use `#[cfg(test)] mod tests`, so adding `mod proptests` alongside
is a no-friction extension.

| # | Function | Property | BC refs | Strategy |
|---|----------|----------|---------|----------|
| A.1 | `parse_field_kv(args: &[String]) -> Result<HashMap<String, Value>, _>` | **First-`=` is the only delimiter.** For any input `KEY=REST_WITH_ANY_EQUALS`, the resulting map contains exactly one entry whose value's string form preserves every byte after the first `=` literally (including additional `=` characters). | BC-3.8.007 | `(key in "[A-Za-z0-9_ ]{1,20}", value in "\\PC{0,80}").prop_map(|(k, v)| format!("{}={}", k, v))` then assert the round-trip. |
| A.2 | `parse_field_kv` | **Empty value is allowed.** `KEY=` (zero-length suffix) produces a map entry with an empty-string value, NOT an error and NOT a missing key. | BC-3.8.007 | `key in "[A-Za-z][A-Za-z0-9_]{0,20}"` then assert `parse_field_kv(&[format!("{}=", k)])` returns `{k: ""}`. |
| A.3 | `parse_field_kv` | **Duplicate-key last-write-wins (or: error — F4 to decide, but F1 BA already pinned "last wins" in BC-3.8.007).** Given two `--field` entries with identical key but different values, the second value is the one in the final map. The proptest must align with whatever F4 implements — if F4 chooses "error on duplicate" instead, this proptest is the regression guard for the chosen semantics. | BC-3.8.007 | `(key in "[A-Za-z]{1,10}", v1 in "[a-z]{1,20}", v2 in "[a-z]{1,20}").prop_filter("distinct", |(_, a, b)| a != b)`; assert resulting map value == `v2`. |
| A.4 | `parse_field_kv` | **No panic on arbitrary input.** Mirror of `partial_match::never_panics_on_arbitrary_input`. For any UTF-8 string slice (including no `=`, leading `=`, only `=`, only whitespace), the parser either returns `Ok` or returns `Err` — it never panics, never overflows, never trips a `debug_assert`. | BC-3.8.007 (negative path) | `s in "\\PC{0,100}"`; assert `let _ = parse_field_kv(&[s]);` does not panic. |

**Note on A.3:** The BC body in F1 BA input pins "duplicate NAME → last
wins". F2 spec evolution must preserve that wording in the final BC body;
if architecture review changes the semantics in F2, this proptest spec
must be updated in lockstep before F6.

**Note on unicode keys:** Property A.1's `key` strategy is ASCII-only by
default. F4 should decide whether Jira request type field names are
restricted to ASCII (in which case A.1 is sufficient) or accept Unicode
(in which case A.1 should be widened to `\\PC{1,20}` minus `=`). The
deferred Atlassian-side validation rule for field-name character set is
the gating question; see F6 handoff checklist.

#### Group B — `cli/requesttype.rs` partial-name resolution

**No new proptest needed.** The partial-name resolution path reuses
`src/partial_match.rs::partial_match` directly, exactly as `cli/queue.rs`
does today (confirmed at `cli/queue.rs:147` and `cli/queue.rs:206`).
`partial_match` already has 4 proptest properties (`exact_match_always_found`,
`never_panics_on_arbitrary_input`, `empty_candidates_always_returns_none`,
`duplicate_candidates_yield_exact_multiple`) at `src/partial_match.rs:149`.

The new caller adds no new logic to test at the property level: it only
wires `MatchResult` arms to `JrError::UserError`, which is straightforward
match-arm coverage handled by integration tests in `tests/requesttype_commands.rs`.
This covers BC-X.12.007 and BC-3.8.004 without a duplicate proptest.

**Verification debt avoidance note:** if `cli/requesttype.rs` introduces
its own bespoke matching helper instead of reusing `partial_match`,
this assessment is invalid and a new proptest property is required.
F5 adversarial review should explicitly check for non-reuse.

#### Group C — `api/jsm/requests.rs` `requestFieldValues` body construction

| # | Function | Property | BC refs | Strategy |
|---|----------|----------|---------|----------|
| C.1 | `build_jsm_request_body(summary: &str, description: Option<&str>, fields: &HashMap<String, Value>, on_behalf_of: Option<&str>, request_type_id: &str, service_desk_id: &str) -> serde_json::Value` (or whatever shape F4 produces) | **Summary is always present** in the resulting `requestFieldValues` map under key `"summary"`, regardless of what else is in the user-supplied `fields` map. If the user supplies `--field summary=X`, the user value MUST win (last-write-wins between CLI arg and `--field` override is a deliberate design choice — F4 to decide and pin). | BC-3.8.001, BC-3.8.007 | `(summary in "[A-Za-z0-9 ]{1,80}", extra_fields in prop::collection::hash_map("[A-Za-z]{1,10}", "[a-z]{1,30}", 0..5))`; build body; assert `body["requestFieldValues"]["summary"]` is non-null. |
| C.2 | `build_jsm_request_body` | **Description is ADF when `isAdfRequest: true`.** If `description` is `Some(_)`, the body MUST contain `"isAdfRequest": true` AND `requestFieldValues.description` MUST be a JSON object (ADF root), not a bare string. If `description` is `None`, the field is absent (NOT `null`). | BC-3.8.001 (ADF clause) | `desc in proptest::option::of("[A-Za-z .]{1,100}")`; build body; assert `body["isAdfRequest"]` and `body["requestFieldValues"]["description"]` shape per `desc.is_some()`. |
| C.3 | `build_jsm_request_body` | **`raiseOnBehalfOf` absence vs. presence.** If `on_behalf_of` is `None`, the body MUST NOT contain a `raiseOnBehalfOf` key at all (NOT `null`, NOT empty-string — absent). If `Some(account_id)`, the key MUST be present at the top level (NOT under `requestFieldValues`) with the exact account-id string. | BC-3.8.009 | `obo in proptest::option::of("[a-zA-Z0-9:-]{10,40}")`; build body; assert presence/absence via `serde_json::Value::get` checks. |

**File:** new proptest module in `src/api/jsm/requests.rs` (alongside the
new API client function), pattern: `#[cfg(test)] mod proptests` at the
bottom of the file. This is a pure construction helper — no HTTP, no I/O —
making it an ideal proptest target.

#### Group D — `api/jsm/request_types.rs` pagination

**Assessment: no new proptest required.** The new
`list_request_types` function will reuse `ServiceDeskPage<RequestType>`
from `src/api/pagination.rs` (confirmed at `pagination.rs:87-113`). The
pagination loop will mirror `src/api/jsm/queues.rs::list_queues` exactly:
loop on `page.has_more()`, advance `start` via `page.next_start()` (which
is `self.start + self.size` — advance by returned-count, NOT by `limit`).
This is the **opposite** convention from `src/api/jira/users.rs`, which
advances by `USER_PAGE_SIZE` due to JRACLOUD-71293's fixed-window
permission filtering. The JSM API does NOT exhibit JRACLOUD-71293 (it is
a standard list endpoint without per-page permission filtering at fixed
windows), so the `ServiceDeskPage::next_start()` semantic is correct for
request types.

**Coverage already present** for `ServiceDeskPage`: `src/api/pagination.rs:255-261`
has `test_service_desk_page_has_more` unit tests. These already cover the
`is_last_page` invariant. No proptest property is warranted unless F4
introduces a divergent pagination loop in `request_types.rs` (e.g., a
hand-rolled offset advance that does not call `next_start()`). F5
adversarial review should flag any such divergence as a regression
against the established JSM pagination pattern.

**Integration coverage** for the new endpoint:
`tests/requesttype_commands.rs::requesttype_list_returns_types_from_servicedesk`
will mock a paginated response and assert all values are returned.

---

### Proptest Summary

- **New proptest files:** 0 (all properties live in existing or new module
  files via the inline `#[cfg(test)] mod proptests` pattern).
- **New proptest properties:** 7 total — 4 in `cli/issue/create.rs`
  (Group A.1–A.4), 3 in `api/jsm/requests.rs` (Group C.1–C.3).
- **Reused proptest coverage:** all 4 `partial_match` proptests inherited
  by `cli/requesttype.rs` via direct call.
- **No-new-property modules:** `api/jsm/request_types.rs` (pagination
  pattern-reuse), `cli/requesttype.rs` (partial-match reuse),
  `types/jsm/request_type.rs` (serde-derive; runtime errors not
  silent-regression risks).
- **BC-3.8.010 (--type ignored with warning):** integration-test-only — no proptest
  candidate. The behavior is a stderr warning side-effect with no pure-function
  property to fuzz; it is fully covered by H-NEW-JSM-RT-004 and the integration
  test `tests/issue_create_jsm.rs::type_flag_ignored_with_warning_when_request_type_set`.

---

## 3. Mutation Testing Scope Additions

### Current scope (`.cargo/mutants.toml`)

```toml
examine_globs = [
    "src/api/jira/bulk.rs",
    "src/types/jira/bulk.rs",
    "src/cli/issue/create.rs",
]
```

`src/cli/issue/create.rs` is **already in scope.** The new
`--request-type` dispatch branch in `handle_create` is automatically
covered by the existing glob and will be subjected to mutation testing
on the PR-diff scope (`--in-diff` in CI).

### Required additions for #288

F6 must expand `examine_globs` to add the four new modules. The expansion
should be made in the same PR as the implementation (matching the
established pattern: `docs/specs/cargo-mutants-policy.md` already documents
that scope is set by `examine_globs`, not by CLI flags).

**Proposed updated `examine_globs`:**

```toml
examine_globs = [
    "src/api/jira/bulk.rs",
    "src/types/jira/bulk.rs",
    "src/cli/issue/create.rs",
    "src/api/jsm/requests.rs",       # NEW for #288
    "src/api/jsm/request_types.rs",  # NEW for #288
    "src/cli/requesttype.rs",        # NEW for #288
    "src/types/jsm/request_type.rs", # NEW for #288 (serde structs — see note)
]
```

**Note on serde-only module:** `types/jsm/request_type.rs` contains
`#[derive(Deserialize)]` structs with minimal hand-written code. Mutation
testing on derive-generated code is mostly noise. F6 may choose to:
(a) include the file and whitelist any surviving derive-internal mutants
with `// mutants::skip: serde derive — not user code` justifications, or
(b) exclude it from `examine_globs` and rely on serde's own test coverage
plus the integration tests in `tests/requesttype_commands.rs`. The
recommended path is **(b) — exclude** unless hand-written `impl` blocks
are added (e.g., custom `Deserialize`, builder methods).

**Final recommendation for `examine_globs`:**

```toml
examine_globs = [
    "src/api/jira/bulk.rs",
    "src/types/jira/bulk.rs",
    "src/cli/issue/create.rs",
    "src/api/jsm/requests.rs",       # NEW: body construction + HTTP dispatch
    "src/api/jsm/request_types.rs",  # NEW: pagination + lookup
    "src/cli/requesttype.rs",        # NEW: handler dispatch + match arms
]
```

### Whitelist additions (anticipated)

F6 should expect to apply `#[mutants::skip]` with justification on:

- The `is_some()` branch gate in `handle_create` (e.g.,
  `if request_type.is_some()`) is the dispatch fork. The mutant
  `if request_type.is_none()` (negate condition) must be killed; this is
  the primary kill target, NOT a whitelist candidate. Any failure to kill
  it indicates a missing integration test in `tests/issue_create_jsm.rs`
  (likely the `Mock::expect(0)` test on the platform endpoint when
  `--request-type` is set, or vice versa).
- Defensive `require_service_desk` early-return guards (impossible state)
  may need whitelisting with `// mutants::skip: defensive guard for
  pre-checked invariant` — but only if the integration test already
  proves the guard is never reached in practice.

### Kill-rate target

Per existing policy (`docs/specs/cargo-mutants-policy.md`): **90% on the
PR-diff scope.** No exception requested for #288 — the new modules are
new code without legacy debt, so 90% is achievable.

### CI integration

The existing `cargo mutants --in-diff ...` invocation from CLAUDE.md
(`DIFF_FILE=$(mktemp ...) && ... && cargo mutants --in-diff "$DIFF_FILE" --jobs 4`)
will automatically pick up the new files on the PR diff. No CI YAML
change required beyond the `examine_globs` update.

---

## 4. Fuzz Target Additions

**None expected.** Rationale:

1. **`--field NAME=VALUE` parser:** Small surface area (string-split on
   first `=`); proptest A.1–A.4 with `\\PC{0,100}` arbitrary-input strategy
   covers the input domain adequately. Fuzzing would not surface anything
   proptest cannot. The historical precedent on this project is that
   pure-string helpers (`escape_value` in `jql.rs`, `partial_match`,
   `Duration::parse`) use proptest, not fuzz.
2. **HTTP response parsing:** `api/jsm/requests.rs` and
   `api/jsm/request_types.rs` reuse the existing `JiraClient::get_from_instance`
   / `post_to_instance` infrastructure plus `serde_json` deserialization
   into `RequestType` / `JsmRequestCreated` structs. The serde layer is
   not project-fuzzed today; introducing fuzz here would expand the
   verification surface unjustifiably for one feature.
3. **No new binary or unsafe code:** No new `unsafe` blocks, no new
   crates with native code, no new parser combinators. The fuzz-target
   cost/benefit ratio is poor.

**If F6 disagrees,** the candidate fuzz target would be `parse_field_kv`
under `cargo fuzz` with a libFuzzer harness in a new `fuzz/`
directory — but this would be the first such directory on the project
and would require a separate spec.

---

## 5. F6 Handoff Checklist

Before F6 (Targeted Hardening) can sign off, the following must be in
place. F6 is responsible for verifying each item.

### Code prerequisites (produced in F4)

- [ ] `src/cli/issue/create.rs` contains a `parse_field_kv` function (or
      equivalent named helper) that is unit-testable in isolation. If
      the parser logic is inlined inside `handle_create`, **F4 must
      extract it** so proptest A.1–A.4 can target it directly. This is a
      non-negotiable prerequisite — proptest on a 1,601-LOC handler is
      impractical.
- [ ] `src/api/jsm/requests.rs` exposes `build_jsm_request_body`
      (or equivalent pure body-construction helper) separately from the
      HTTP-firing function. The body-builder must be callable without a
      `JiraClient`. Required for proptest C.1–C.3.
- [ ] `src/cli/requesttype.rs` uses `crate::partial_match::partial_match`
      directly (not a bespoke matcher). If a bespoke matcher is
      introduced, F4 must add proptest properties for it before F6.
- [ ] `src/api/jsm/request_types.rs` uses `ServiceDeskPage::next_start()`
      for pagination advance (NOT a hand-rolled offset). If
      hand-rolled, F4 must justify in code comments AND F6 must add
      regression-pinning unit tests.

### Spec/doc prerequisites (produced in F2)

- [ ] BC-3.8.007 body in `bc-3-issue-write.md` pins one of:
      "duplicate NAME → last wins" OR "duplicate NAME → exit 64 with
      error". Proptest A.3 must align.
- [ ] BC-3.8.001 body pins the JSON output shape: `{"key": "<KEY>"}` for
      `--output json`. Integration test in `tests/issue_create_jsm.rs`
      asserts this; mutation testing on the JSON-formatting branch in
      `handle_create` enforces it.

### Mutation testing prerequisites

- [ ] `.cargo/mutants.toml` `examine_globs` extended to include the three
      new files listed in §3.
- [ ] `docs/specs/cargo-mutants-policy.md` "Scope" section updated to
      list the new files. (One-line addition under the bullet list.)
- [ ] CI `mutants` job re-baselined: first run on the #288 branch may
      surface a flood of mutants; per the policy's "Deferral Policy"
      section, file one GitHub follow-up issue per uncovered-region
      cluster rather than blocking the PR.

### Proptest prerequisites

- [ ] 4 proptest properties added to `src/cli/issue/create.rs::mod proptests`
      (Group A.1–A.4).
- [ ] 3 proptest properties added to `src/api/jsm/requests.rs::mod proptests`
      (Group C.1–C.3).
- [ ] All proptests use `prop_assert!`/`prop_assert_eq!` (NOT plain
      `assert!`) — matching the existing convention.

### Verification gaps (F6 cannot cover; recommend defer)

1. **Atlassian Developer Console scope registration.** The HIGH-risk item
   from F1 risk assessment (BC-1.3.023 expansion to include
   `write:servicedesk-request`). No automated verification can confirm
   the scope is registered in the production OAuth app. **Recommendation:
   defer to F7 release-gate manual checklist** (already noted in F1
   "Recommended Scope for Subsequent Phases" under F7). NOT a blocker for
   F6.
2. **JSM API live-response shape drift.** Mock-based integration tests
   pin the response shape we expect. If Atlassian changes the
   `/rest/servicedeskapi/request` response (e.g., renames `issueKey` to
   `requestKey`), no proptest or mutation test will catch it. **Recommendation:
   defer to a contract-test phase outside F6.** NOT a blocker for F6.
3. **`raiseOnBehalfOf` accountId validation.** F1 risk assessment flagged
   F5 adversary check: "Is the `raiseOnBehalfOf` accountId validated
   before being passed through?" This is a *behavioral* property
   (rejection of malformed input), not a *structural* property
   (presence/absence in body). Proptest C.3 covers the structural side;
   the behavioral side is BC-3.8.009 integration test coverage.
   **Recommendation: handle as adversarial finding in F5, not F6 gap.**
   NOT a blocker for F6.

---

## 6. F6 Sign-off Criteria

F6 may sign off when:

1. All 7 proptest properties from §2 are in tree and pass.
2. `cargo mutants --in-diff` on the PR diff reports ≥90% kill-rate, OR
   the surviving mutants are individually whitelisted with justification
   comments AND a follow-up GitHub issue is filed per the deferral policy.
3. The four items in §5 "Code prerequisites" are confirmed by F6 file
   inspection.
4. No `unsafe` blocks introduced (CLAUDE.md "Conventions" — no unsafe
   without explicit justification).
5. `cargo clippy -- -D warnings` and `cargo fmt --all -- --check` remain
   green on the #288 branch.

Items §5.4 "Verification gaps" are explicitly OUT of F6 scope and routed
elsewhere (F5, F7, or new follow-up issue).

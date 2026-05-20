---
document_type: prd-delta
phase: phase-f2-spec-evolution
producer: product-owner
issue: 385
status: complete
created: 2026-05-20
spec_version: "1.2.0"
bc_delta: "+2 new (BC-3.8.016, BC-3.8.017); 3 modified (BC-3.8.002, BC-3.8.010, BC-3.8.011)"
grand_total_before: 573
grand_total_after: 575
holdout_delta: "+2 new (H-NEW-JSM-RT-006, H-NEW-JSM-RT-007)"
---

# PRD Delta — Issue #385

> **SCOPE BANNER (C-01):** This delta specifies the TARGET contract to be implemented in Phase F3/F4. `develop` HEAD currently reflects pre-#385 behavior; the BC postconditions below are the work to be done, not a description of merged code. `status: complete` in the frontmatter means Phase F2 spec evolution is complete, not that #385 is implemented.

## Feature Request

- **Brief:** JSM create UX polish: harmonize project-required error (O-08-02), guard empty `--request-type` (O-08-04), reject `--markdown` + `--field description=` conflict (O-08-06), move `--type` warning post-`require_service_desk` (O-08-07)
- **Issue link:** https://github.com/Zious11/jira-cli/issues/385
- **F1 source:** `.factory/phase-f1-delta-analysis/delta-analysis-385.md`
- **Spec version after:** 1.2.0

---

## New Behavioral Contracts

### Canonical Guard Ordering in `handle_jsm_create`

The authoritative 6-step guard ordering for `handle_jsm_create` is defined in `bc-3-issue-write.md` under `#### Canonical Guard Ordering — handle_jsm_create` (subdomain 3.8, immediately before BC-3.8.016). This document is a delta spec — see that block for the single source of truth. When changing any step, update ONLY that block.

---

### BC-3.8.016: `--request-type ""` (empty string after trim) exits 64 before `require_service_desk`

**File**: `.factory/specs/prd/bc-3-issue-write.md`
**Closes**: O-08-04

**Preconditions**:
- `--request-type` is set to the empty string (`""`) or a whitespace-only string

**Postconditions**:
- exit 64
- stderr contains (assert via `contains`): `"request type cannot be empty"` <!-- duplicated from BC-3.8.016 body in bc-3-issue-write.md (CANONICAL) — update both together -->
- stdout is empty
- No HTTP calls issued — guard fires at ordering step 1, before `require_service_desk` (step 4); numeric-bypass check and `partial_match` both occur at step 6

**Rationale**: Without the guard, `partial_match("", &candidates)` returns `Ambiguous` for any NON-EMPTY candidate list (because `"anything".contains("")` is always `true` in Rust, so every name matches) and `None` for an empty candidate list. Both outcomes produce misleading messages when the user explicitly passed an empty string — the empty-string guard prevents either from surfacing.

---

### BC-3.8.017: `--markdown` + `--field description=<value>` combination rejected at the top of `handle_jsm_create`; exit 64

**File**: `.factory/specs/prd/bc-3-issue-write.md`
**Closes**: O-08-06

**Preconditions**:
- `--markdown` flag is set AND any `--field` arg token has a raw key (substring before first `=`, NO trimming, NO case-folding) that is EXACTLY `"description"` — case-SENSITIVE, no-trim match, identical to `parse_field_kv`'s key extraction. `--field Description=X` (key `Description`) does NOT satisfy this precondition.

**Postconditions**:
- exit 64
- stderr is the ONE canonical single-sentence message defined in BC-3.8.017 body (bc-3-issue-write.md CANONICAL SOURCE). The implementation MUST emit a single contiguous stderr sentence — NOT two separate `eprintln!` calls. The two `contains` checks below are substring slices of that one sentence, provided for test-assertion convenience only:
  - assert via `contains`: `` `--field description=...` cannot be combined with `--markdown` `` <!-- duplicated from BC-3.8.017 body in bc-3-issue-write.md (CANONICAL) — update both together -->
  - assert via `contains`: `may result in a JSM 400 error or silently dropped ADF formatting` <!-- duplicated from BC-3.8.017 body in bc-3-issue-write.md (CANONICAL) — update both together -->
- stdout is empty
- No HTTP calls issued — guard fires at ordering step 2 (see Canonical Guard Ordering above), before `require_service_desk`

**Guard position relative to other guards**: This guard (step 2) sits BEFORE the existing `--markdown`-requires-`--description` guard (step 3). Therefore `--markdown --field description=X` with NO `--description` flag correctly triggers THIS guard's conflict message — NOT the "requires --description" message.

**Rationale**: `JsmRequestBuilder::build()` populates `requestFieldValues["description"]` with the ADF object during description handling and computes `is_adf_request = true`; it then iterates `extra_fields`, and an `extra_fields` entry keyed exactly `"description"` overwrites the ADF value with a plain string; `isAdfRequest: true` is still emitted in the final body — producing the desync. The exact Atlassian behavior (400 vs silent drop) is undocumented; this spec DOES NOT assert "Atlassian returns 400" per CLAUDE.md citation discipline.

**Non-triggering cases**:
- `--markdown` alone (with `--description`), no `--field description=`: guard does NOT fire
- `--field description=value` WITHOUT `--markdown`: guard does NOT fire

**F1 divergence note (Open Question #3)**: The F1 delta-analysis Open Question #3 recommended a conflict message containing the phrase "producing a malformed request". F2 deliberately refined the canonical message to say "desyncing `isAdfRequest: true` with a plain-string description value (may result in a JSM 400 error or silently dropped ADF formatting)" for technical precision — the desync is the root cause; "malformed request" understated the ambiguity of Atlassian's server behavior (which is undocumented). The F1 OQ-3 wording is intentionally superseded.

---

## Modified Behavioral Contracts

### BC-3.8.002 — UPDATED (O-08-02: harmonized project-required error string)

**Change**: The error string emitted when no project is resolvable AND `no_input` is effective (set explicitly via `--no-input` OR auto-enabled on non-TTY stdin per CLAUDE.md) OR `helpers::prompt_input` itself errors — updated to match platform path affordances. The code site (`create.rs:1882-1891`) checks `no_input` only; non-TTY is not a separate code-level check but causes `no_input` to be auto-set before the handler runs.

**Previous verbatim**:
```
project is required for JSM request creation
```

**New verbatim**:
```
Project key is required for JSM request creation. Use --project or configure .jr.toml. Run "jr project list" to see available JSM projects.
```

**Rationale**: Platform path (`handle_create`) returns: `"Project key is required. Use --project or configure .jr.toml. Run \"jr project list\" to see available projects."` — 17+ words with three actionable affordances. JSM path returned 6 words with none. New string adds all three affordances, sentence-cases the opening, and preserves the "for JSM request creation" context label.

**Test impact**: `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` in `tests/issue_create_jsm.rs` asserts the verbatim string and MUST be updated to the new string.

**Holdout coverage**: O-08-02 is DELIBERATELY holdout-exempt. Unlike O-08-04 (→H-NEW-JSM-RT-006) and O-08-06 (→H-NEW-JSM-RT-007), this change is a string-only error-message update with no control-flow impact. The existing unit test `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` (updated to assert the new verbatim string) provides complete regression coverage. No holdout is needed or added for this change.

---

### BC-3.8.010 — UPDATED (O-08-07: warning position clarification)

**Change**: The `--type` warning position is now explicitly specified: it fires INSIDE `handle_jsm_create` AFTER `require_service_desk` returns `Ok`, not in `handle_create` before `handle_jsm_create` is called.

**Consequence**: On a non-JSM project where `require_service_desk` fails (this assumes `--request-type` is non-empty; an empty/whitespace-only `--request-type` exits at step 1 per BC-3.8.016 before reaching `require_service_desk`), the user sees ONLY the non-JSM project error. The previous behavior emitted both the warning AND the error on non-JSM projects, which was misleading.

**JSM success path**: Unchanged — warning still fires at step 5 (after `require_service_desk` returns `Ok`, BEFORE request-type resolution at step 6) when the project is a valid JSM service desk. Consequence: on a JSM project with an unresolvable `--request-type` name, the `--type` warning WILL have fired and the partial-match error (BC-3.8.003) follows at step 6 — both appear on stderr; this is acceptable because the project IS a service desk.

**New test required**: `test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project` — non-JSM project + `--request-type X` (non-empty) + `--type Bug` → exit 64 with non-JSM error, stderr does NOT contain `"warning: --type is ignored"`. (An empty `--request-type` exits at step 1 per BC-3.8.016 before reaching `require_service_desk`; this test uses a non-empty value to exercise step 4.)

---

### BC-3.8.011 — UPDATED (O-08-07: same warning-position constraint)

**Change**: All six warnings (the `--type` warning of BC-3.8.010 plus the five platform-only flag warnings of BC-3.8.011: `--team`, `--points`, `--parent`, `--to`, `--account-id`) receive the same post-`require_service_desk` position constraint. Moving all six warnings together avoids an asymmetry where `--type` fires post-`require_service_desk` but the other five still fire pre-`require_service_desk`.

**Existing tests**: All per-flag warning-emission integration tests must remain green — warnings still fire on the JSM success path.

---

### O-08-07 Implementation Note: Threading Flag Values into `handle_jsm_create`

**IMPLEMENTATION NOTE (H-03)**: Placing all six warnings at step 5 inside `handle_jsm_create` requires the warning-triggering flag values to be in scope at that site. As of the pre-#385 baseline, `JsmCreateArgs` (`create.rs:1864-1875`) destructures only: `project`, `request_type`, `summary`, `description`, `description_stdin`, `priority`, `labels`, `markdown`, `on_behalf_of`, `field_pairs`. It does NOT carry `issue_type` (`--type`), `team` (`--team`), `points` (`--points`), `parent` (`--parent`), `to` (`--to`), or `account_id` (`--account-id`).

The behavioral contracts (BC-3.8.010/BC-3.8.011) constrain WHEN the warnings fire (step 5: after `require_service_desk` returns `Ok`, before request-type resolution at step 6). They do NOT constrain the threading mechanism — that is an F3/F4 implementation choice. Acceptable mechanisms include but are not limited to:
- Extending `JsmCreateArgs` with the six missing fields
- Passing them as additional parameters to `handle_jsm_create`
- Extracting them from a broader `CreateArgs` reference passed alongside `JsmCreateArgs`

Implementers MUST choose a mechanism that makes all six values visible at the step-5 warning site. The spec does not prescribe which mechanism to use.

**Single-site requirement (F-02)**: The existing pre-dispatch warning emission block in `handle_create` (which currently fires these warnings before `handle_jsm_create` is called) MUST be REMOVED as part of this change — the warnings must exist at exactly ONE site, canonical step 5 inside `handle_jsm_create`. Double-emission from two code sites is a defect. The new required test `test_jsm_create_platform_flag_warnings_emit_once_on_success` (item 7 in Required Test Deliverables) pins this: it asserts each warning appears EXACTLY ONCE on stderr on the JSM success path, catching any double-emission regression. This is distinct from BC-3.8.011's existing idempotency contract (one warning per logical flag regardless of how many times the flag is repeated), which concerns duplicate flags — not duplicate code sites.

---

## Edge Cases Added (embedded in new BCs)

### EC-3.8.016-1: Whitespace-only `--request-type`

`--request-type "   "` (spaces only) is treated equivalently to `--request-type ""` after `trim()`. Same exit 64, same message: `"request type cannot be empty"`.

### EC-3.8.017-1: `--field description=` (empty value) + `--markdown`

Even `--field description=` (empty-value form) triggers the conflict guard if `--markdown` is set, because the key `"description"` is present in the raw `--field` arg list regardless of value. Note: the guard fires even without `--description` being set; that case correctly surfaces the BC-3.8.017 conflict message (not the "requires --description" message), because this guard sits at step 2 in the canonical ordering, BEFORE the existing `--markdown`-requires-`--description` guard at step 3.

### EC-3.8.017-2: Multiple `--field` with `description` key

`--field summary=foo --field description=bar --markdown` triggers the guard. The raw-split check scans each `--field` token in order; as soon as any token's raw key is exactly `"description"`, the guard fires — order of `--field` args is irrelevant.

### EC-3.8.017-3: `--field Description=value` (capitalized) + `--markdown`

`--field Description=X` (capital D) does NOT trigger the guard. The guard uses an EXACT, case-SENSITIVE, no-trim key match (`raw_key == "description"`), mirroring `parse_field_kv`'s raw extraction. The raw key `Description` does not equal `"description"`, and crucially does not produce the desync — `extra_fields["Description"]` does not overwrite `requestFieldValues["description"]` in the HashMap. The command proceeds to step 6 and `Description` is passed as a normal extra field to the JSM request body.

[UPDATED adversary pass-3 H-02] REVERSED (pass-11 H-1): Previously stated that capitalized keys DID trigger the guard (case-insensitive match after pass-3). That was wrong: the premise that a differently-cased key produces the desync is incorrect. HashMap overwrite is exact-match; `Description` ≠ `description` as a key. Guard reverted to exact/case-sensitive/no-trim match.

### EC-3.8.017-4: `--markdown --description-stdin --field description=X`

`--description-stdin` (reading description from stdin) combined with `--markdown` and `--field description=X` → guard fires, exit 64, conflict message. The guard does not inspect whether the description comes from `--description`, `--description-stdin`, or any other source — it fires whenever `--markdown` is set AND a `--field` whose raw key is exactly `"description"` is present, regardless of the description source.

### EC-3.8.017-5: `--field description` (no `=`, malformed token) + `--markdown`

A `--field` token with NO `=` character at all (e.g. `--field description`) does NOT trigger the BC-3.8.017 guard — the step-2 raw-split check requires a `=`-present form to extract a key; a no-`=` token has no extractable key and therefore never satisfies the conflict condition. BC-3.8.017's guard does not fire.

The downstream outcome depends on other flags present:

- **With a description source** (e.g. `--markdown --description "X" --field description`): step 3 (`--markdown`-requires-`--description`) is satisfied, so the no-`=` token reaches `parse_field_kv` at step 6, which surfaces the existing malformed-pair error (exit 64, pre-#385 path, out of this BC's scope).
- **Without a description source** (e.g. `--markdown --field description` with no `--description` / `--description-stdin`): the step-3 `--markdown`-requires-`--description` guard fires first ("--markdown requires --description…"), before step 6 is reached.

In both cases, BC-3.8.017's step-2 guard does not fire. The EC's core assertion is that a no-`=` token cannot produce the ADF desync BC-3.8.017 guards against — a valid `key=value` token where the raw key is exactly `"description"` is required for the desync to occur.

---

## New Holdout Scenarios

### H-NEW-JSM-RT-006 (pins BC-3.8.016)

`jr issue create --project HELP --request-type "" --summary "Test" --no-input` → exit 64, stderr "request type cannot be empty", no HTTP.

See `holdout-scenarios.md §H-NEW-JSM-RT-006` for full setup and expected assertions.

### H-NEW-JSM-RT-007 (pins BC-3.8.017)

`jr issue create --project HELP --request-type 17 --summary "Reset please" --markdown --field description="plain text override" --no-input` → exit 64, stderr conflict message, no HTTP. (The step-2 conflict guard does not inspect the `--request-type` value; numeric 17 is used only to keep the action concrete. The zero-mock property holds because the step-2 guard precedes `require_service_desk` (step 4) regardless of the request-type value. A regression moving this guard below step 4 would be caught by the zero-mock setup.)

See `holdout-scenarios.md §H-NEW-JSM-RT-007` for full setup, expected assertions, and boundary cases.

---

## Count Impact

| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| `bc-3-issue-write.md` `total_bcs` (frontmatter) | 93 | 95 | +2 |
| `bc-3-issue-write.md` `definitional_count` (frontmatter) | 64 | 66 | +2 |
| `bc-3-issue-write.md` body preamble | 93 | 95 | +2 |
| BC-INDEX.md `sections:` bc-3 line | 93 / 64 | 95 / 66 | +2/+2 |
| BC-INDEX.md Section 3 header | 93 / 64 | 95 / 66 | +2/+2 |
| BC-INDEX.md subdomain 3.8 header | 15 BCs ..015 | 17 BCs ..017 | +2 |
| BC-INDEX.md Coverage Statistics bc-3 row | 93 / 64 | 95 / 66 | +2/+2 |
| BC-INDEX.md Coverage Statistics Total row | 573 / 341 | 575 / 343 | +2/+2 |
| BC-INDEX.md `total_bcs` (frontmatter) | 573 | 575 | +2 |
| CANONICAL-COUNTS.md bc-3 definitional row | 64 / 64 | 66 / 66 | +2/+2 |
| CANONICAL-COUNTS.md Total individually-bodied | 341 | 343 | +2 |
| CANONICAL-COUNTS.md bc-3 total_bcs row | 93 | 95 | +2 |
| CANONICAL-COUNTS.md Sum row | 573 | 575 | +2 |
| CANONICAL-COUNTS.md grand total prose | 573 | 575 | +2 |
| CANONICAL-COUNTS.md Breakdown line | 573 / 341 | 575 / 343 | +2/+2 |
| holdout-scenarios.md `total_holdouts` | 55 | 57 | +2 |
| CANONICAL-COUNTS.md holdout total | 55 | 57 | +2 |

Note: guard-script output is the authoritative verification; see §Guard Script Results below.

---

## Guard Script Results (post-edit)

Verbatim output from the three guard scripts run after all adversary pass-3 fixes were applied:

```
$ bash scripts/check-spec-counts.sh
OK: all spec counts verified.
exit: 0

$ bash scripts/check-bc-cumulative-counts.sh
OK: all cumulative BC counts verified (575 total across 8 files).
exit: 0

$ bash scripts/check-bc-no-numeric-test-counts.sh
OK: no numeric test counts in BC Trace/Source fields.
exit: 0
```

---

## Required Test Deliverables

The implementing story MUST include these named test functions as discrete ACs:

1. `test_jsm_create_empty_request_type_exits_64` (NEW — BC-3.8.016 pin; O-08-04; MUST cover BOTH `--request-type ""` and a whitespace-only input such as `--request-type "   "` — the whitespace-only boundary pins the `.trim()` in the guard implementation per EC-3.8.016-1)
2. `test_jsm_create_markdown_field_description_conflict_exits_64` (NEW — BC-3.8.017 pin; O-08-06; stderr assertions: three `contains` slices of the ONE canonical sentence — (a) `` `--field description=...` cannot be combined with `--markdown` ``; (b) `may result in a JSM 400 error or silently dropped ADF formatting`; (c) `Pass \`--description\` with \`--markdown\`, or omit \`--markdown\`` — remediation clause, pins "errors always suggest what to do next" convention)
3. `test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project` (NEW — BC-3.8.010 O-08-07 pin; asserts warning ABSENT on non-JSM project, non-JSM error IS present)
4. `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` (UPDATED assertion — BC-3.8.002; assert harmonized error string, NOT old terse string)
5. `test_jsm_create_type_flag_ignored_with_warning` (MUST remain green unchanged — BC-3.8.010 JSM-path pin)
6. `test_jsm_create_ambiguous_request_type_exits_64` (MUST remain green unchanged — BC-3.8.003; genuine ambiguity, not empty-string; confirmed: uses `--request-type "Bug"` which matches both "Bug Report" and "Bug Fix Request" — a non-empty prefix, verified by reading `tests/issue_create_jsm.rs::test_jsm_create_ambiguous_request_type_exits_64`. BC-3.8.016's empty guard does not affect this test.)
7. `test_jsm_create_platform_flag_warnings_emit_once_on_success` (NEW — F-02 single-site pin; asserts each of the six warning-flag warnings appears EXACTLY ONCE on stderr on a successful JSM create path. Catches double-emission if the old pre-dispatch warning block in `handle_create` is not removed. Note: `--to` and `--account-id` are clap-mutually-exclusive on `issue create` (confirmed via `src/cli/mod.rs` lines 383/386: `conflicts_with = "account_id"` / `conflicts_with = "to"`), so all six flags cannot appear in a single invocation. This test MUST use TWO invocations: Invocation A carries `--type <T> --team <id> --points <n> --parent <key> --to <id>` (five flags); Invocation B carries `--account-id <id>` (one flag, sufficient to reach the success path). Each warning string must appear exactly once in the relevant invocation's stderr. Distinct from BC-3.8.011's idempotency contract (one warning per repeated logical flag), which concerns duplicate flag occurrences — not duplicate code sites.)

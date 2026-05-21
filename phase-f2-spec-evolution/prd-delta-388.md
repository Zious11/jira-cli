---
document_type: prd-delta
phase: phase-f2-spec-evolution
issue: 388
title: "Accurate cross-hierarchy type-change error + fix fake-endpoint hint (Option A)"
created: 2026-05-20
spec_version_before: "1.2.0"
spec_version_after: "1.3.0"
new_bcs: [BC-3.4.010, BC-3.4.011]
modified_bcs: [BC-3.4.003]
bc_count_delta: +2
total_bcs_before: 575
total_bcs_after: 577
inputs:
  - ".factory/phase-f1-delta-analysis/issue-388/delta-analysis.md"
  - ".factory/research/issue-388-subtask-conversion.md"
---

# PRD Delta — Issue #388 F2 Spec Evolution

## Summary

`jr issue edit KEY --type X` returns HTTP 400 with no actionable guidance when the
requested type change crosses the standard ↔ sub-task hierarchy boundary. The Jira
Cloud REST API does not support cross-hierarchy type conversion (JRACLOUD-27893, open
since 2012, unresolved). This delta defines two new behavioral contracts to govern the
error-enrichment logic.

A bundled bug is also addressed: `src/cli/issue/create.rs:834` previously emitted a
hint referencing `PUT /rest/api/3/issue/{key}/convert` — a non-existent endpoint.
BC-3.4.010 mandates that the shared `CROSS_HIERARCHY_HINT` constant replaces this fake
endpoint reference.

## New Behavioral Contracts

### BC-3.4.010: Cross-hierarchy `edit --type` 400 → CROSS_HIERARCHY_HINT

**File**: `bc-3-issue-write.md`, Section 3.4

When `edit_issue` returns HTTP 400 AND `src_subtask != tgt_subtask` (the source
issue's `issuetype.subtask` flag differs from the target type's `subtask` flag),
the CLI exits 1 and emits the pinned `CROSS_HIERARCHY_HINT` on stderr.

**Verbatim hint (locale-resilient, no exact UI label):**

```
The Jira Cloud REST API does not support changing the standard / sub-task hierarchy level via this endpoint (see JRACLOUD-27893). To convert it, open the issue in the Jira web UI and use the action menu to find the Convert option.
```

The neutral framing ("does not support changing the...hierarchy level via this endpoint") is accurate for BOTH call sites that share `CROSS_HIERARCHY_HINT`: the `edit --type` cross-hierarchy path (user requested a type change) and the `--no-parent` subtask-bound path (user did not request a type change). The prior wording "Converting a work item across..." would have mis-described the `--no-parent` case. The constant is ONE shared string. The `--no-parent` path MUST prepend the following verbatim context sentence (PINNED STRING) before the shared constant:

```
Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue.
```

The `edit --type` path emits the constant directly with no prepended sentence. The pinned sentence deliberately avoids the `...` glyph and quoted label references found in F1 draft wording, using plain ASCII for locale-resilience.

**Classifier design:** `is_cross_hierarchy_type_error(src_subtask: Option<bool>, tgt_subtask: Option<bool>, err: &str) -> Classification` is a pure function. It returns `CrossHierarchy` only when both arguments are `Some(_)` and the inner values differ. `SameCategory` when both are `Some(_)` and equal. `Indeterminate` when either argument is `None`. The `handle_edit` caller additionally yields `Indeterminate` when an enrichment fetch returns an `Err` (HTTP error or network error only — see note below). An unresolvable target name (200 response with the name absent from the list) is handled by the caller BEFORE invoking the classifier — emitting the typo hint directly (SameCategory outcome at the caller level). The English substring `"issue type selected is invalid"` is NOT the sole gate — research addendum A1 confirmed it fires on both boundary-cross errors AND plain typos (locale-fragile; addendum A2).

**`get_project_issue_types` deserialization behavior (live code, CRITICAL-2):** The type-name lookup against `get_project_issue_types` is **net-new F4 logic** built inside `handle_edit`'s error path — it does not pre-exist in the codebase. `get_project_issue_types` at `src/api/jira/projects.rs:47-51` uses `.and_then(|v| from_value::<Vec<IssueTypeMetadata>>(v).ok()).unwrap_or_default()`. A 200 response with a malformed or missing `issueTypes` key returns `Ok(vec![])` — NOT an `Err`. Therefore a 200 with malformed body is NOT an Indeterminate-trigger; it routes to the unresolvable-name sub-path (typo hint). Only an actual `Err` (HTTP error or network error) triggers Indeterminate via Cause-1. This graceful outcome is acceptable: a malformed project-metadata response is rare and the typo hint is not harmful. Do NOT modify `projects.rs` to change this behavior (F1 baseline: "Files NOT Changed").

**Net-new lookup note (CRITICAL-1):** The client-side lookup of the `--type` value against `get_project_issue_types` results is **net-new logic F4 must build inside `handle_edit`'s error path**. The live `edit --type` success path (`src/cli/issue/create.rs:782-785`) passes `--type` raw to Jira with no client-side resolution. The enrichment lookup is deliberately isolated to the error path. The client-side name match uses **case-insensitive exact match** — a deliberate choice for the error-enrichment path that may not perfectly mirror Jira's server-side resolution, but divergence only affects which hint is shown, never edit correctness.

**Bug fix anchor — `--no-parent` hint replacement scope (CRITICAL-3):** The same `CROSS_HIERARCHY_HINT` constant replaces the **ENTIRE** prior `--no-parent` hint block at `src/cli/issue/create.rs:830-837`. The multi-line `format!` that composed the prior hint spans lines 830-836; line 837 is the separate `bail!` statement. The prior `format!` (lines 830-836) contained FOUR sentences: "Tip: subtasks are structurally bound…", "To clear the parent, first convert…", the fake `jr api /rest/api/3/issue/{key}/convert -X put -d '{"type":{"name":"Task"}}'` line, and "(then re-run with --no-parent if needed.)". NONE of these four old sentences are retained. The new block is exactly: the verbatim context sentence below (prepended first), followed immediately by `CROSS_HIERARCHY_HINT` — nothing else. F4 must delete the entire old multi-line hint block and replace it with the new two-part output to avoid stale sentences surviving the edit.

On the `--no-parent` path, the caller MUST prepend the verbatim pinned context sentence before the shared constant:

```
Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue.
```

On the `edit --type` path, the constant is emitted directly with no prepended sentence.
*(O-3 note: The pinned sentence deliberately softens F1-recommended wording — drops the `...` glyph and quoted UI label references — for locale-resilience.)*

**HTTP-400 gate:** The error-enrichment block (BC-3.4.010 and BC-3.4.011) is entered only when `edit_issue`'s `anyhow::Error` downcasts to `JrError::ApiError { status: 400, .. }` (constructed at `src/api/client.rs::parse_error` ~lines 973-997; defined in `src/error.rs`). If `edit_issue` fails with a non-400 error (401, 403, 5xx, network error, or any other non-400 outcome), NO enrichment occurs — the raw error is surfaced unchanged and neither BC-3.4.010 nor BC-3.4.011 enrichment applies. The input space of `handle_edit`'s 400-error-block is total: only `status == 400` enters; all other failures exit before enrichment. The R0b routing row (non-400 `edit_issue` error) is tested by test #10.

**`Option<IssueType>` outer-layer flatten (CRITICAL):** `issue.fields.issuetype` in `src/types/jira/issue.rs:62` is `Option<IssueType>`. `IssueType.subtask` is itself `Option<bool>`. The caller MUST read `src_subtask` via `issue.fields.issuetype.as_ref().and_then(|t| t.subtask)`. Two distinct sources of `src_subtask: None` exist: (a) the `issuetype` object is wholly absent (`Option<IssueType>` is `None`); (b) `issuetype` is present but its `subtask` key is omitted (`IssueType.subtask` is `None`). Both collapse to `None` → Indeterminate via the `and_then` flatten. Phrase the BC-3.4.010 precondition prose about `IssueType.subtask` as an F4-future additive change (the field does not yet exist in the codebase at F2 spec time).

**Call-ordering precondition:** `handle_edit` calls `get_issue` FIRST (supplying both
the source `issuetype.subtask` flag and `fields.project.key`). Only if `get_issue`
succeeds (HTTP 200) is `get_project_issue_types(project_key)` called. A `get_issue`
failure → Indeterminate immediately (the second call never executes). The
unresolvable-name sub-path is reachable only when `get_issue` already succeeded. This
ordering ensures caller-side routing is provably total with no input matching two
branches simultaneously.

**`--type` + `--no-parent` dual-gate precedence (CRITICAL-4):** `--type` and `--no-parent` are NOT mutually exclusive in clap. Confirmed in `src/cli/mod.rs` lines 437-459: `issue_type: Option<String>` at line 438 has NO `conflicts_with` annotation; `no_parent: bool` at line 459 has `#[arg(long, conflicts_with = "parent")]` only — no conflict with `issue_type`. Both flags can be supplied simultaneously. If both are set and `edit_issue` returns HTTP 400, the evaluation order in `handle_edit`'s `if let Err(ref e) = edit_result` block MUST be: the `--type` cross-hierarchy enrichment is evaluated FIRST (invoking `get_issue` → `get_project_issue_types` → `is_cross_hierarchy_type_error`); only if it does NOT emit a hint (classification is SameCategory or Indeterminate and no hint was shown) does the `--no-parent` arm evaluate. This ordering ensures the more-specific cross-hierarchy diagnosis takes precedence over the legacy string-match gate.

**Known limitation — `--no-parent` locale-fragile gate:** The `--no-parent` arm's hint
emission is gated by `is_subtask_parent_error`, a disjunctive English-substring matcher:
`msg.contains("subtask") || (msg.contains("parent") && msg.contains("400"))`. The
locale-fragility risk differs by disjunct: the first disjunct (`"subtask"`) is an
English word and will miss the error on non-English Jira instances; the second disjunct
(`"parent"` + `"400"`) is partially locale-robust because `"400"` is a locale-independent
HTTP status token, but `"parent"` is still English and may not appear in non-English
error messages. Both disjuncts are inherited from the pre-#388 implementation and are a
deliberate scope boundary for #388 — modifying `is_subtask_parent_error`'s locale
resilience is not in scope and is not a regression introduced here.

**Postcondition pins:**
- Stderr contains the verbatim full string of `CROSS_HIERARCHY_HINT` (the entire fenced-block text above, exact byte content). Tests MUST assert the full string, not just the `JRACLOUD-27893` substring.
- Stderr contains the literal `JRACLOUD-27893`.
- Stderr does NOT contain the substring `jr api /rest/api/3/issue` (regression pin unique to the removed fake-endpoint hint at `src/cli/issue/create.rs:834`; the exact prior hint text was `jr api /rest/api/3/issue/{key}/convert -X put -d '{"type":{"name":"Task"}}'`; the pin substring `jr api /rest/api/3/issue` uniquely identifies this removed text without over-matching the broader `/rest/api/3/issue/` path fragment which may appear in legitimate diagnostics; see T-06 note below for why the broader form was rejected).
- Exit code 1.

### BC-3.4.011: Same-hierarchy or indeterminate `edit --type` 400 → typo hint or raw error

**File**: `bc-3-issue-write.md`, Section 3.4

When `edit_issue` returns HTTP 400 AND the classification is NOT cross-hierarchy:

**SameCategory sub-path** (`is_cross_hierarchy_type_error` returns `SameCategory`: both `subtask` fields are `Some(_)` and equal — covers typos AND workflow-incompatibility 400s). The enrichment lookup that determines whether the target name IS found uses **case-insensitive exact match on the issue-type `name` field** (so the enrichment verdict agrees with how Jira server-side resolves the type name; partial_match substring matching MUST NOT be used, which could mis-resolve ambiguous type names):
- Emit pinned hint:

```
Jira rejected the type change. If the type name is wrong, run `jr project types` to list valid types; the change may also be blocked by workflow or scheme constraints.
```

- Surface the `extract_error_message`-processed 400 message text carried in `JrError::ApiError.message` (this is the extracted message only — e.g., `issuetype: The issue type selected is invalid.`; the raw JSON envelope such as `{"errors": {...}}` is NOT surfaced because `JiraClient::parse_error` in `src/api/client.rs` runs `extract_error_message()` on the response bytes. Note: `extract_error_message` is `sanitize_for_stderr(extract_error_message_raw(body))` per `src/api/client.rs:1481` — for plain-ASCII message text, `sanitize_for_stderr` is effectively a no-op, so test substrings from plain-ASCII extracted text are safe; test assertions MUST use plain-ASCII substrings only, not control characters or multibyte sequences). When asserting in tests (#3, #4), choose a substring from the EXTRACTED message (e.g., `The issue type selected is invalid` survives extraction; `{"errors"` or `"issuetype":` as raw JSON keys do not).
- `CROSS_HIERARCHY_HINT` (containing `JRACLOUD-27893`) MUST NOT appear.

**Indeterminate sub-path** — occurs in two distinct ways (distinct from the unresolvable-name case, which is caller-side and routes to the typo hint):
1. (Cause-1) `handle_edit` enrichment fetch (`get_issue` or `get_project_issue_types`) returns `Err` — detected by `Result::is_err()` on the call, NOT by downcasting to a specific error variant. ANY `Err` variant triggers Indeterminate: `JrError::NotAuthenticated` (e.g., a `get_issue` 401 — note: `parse_error` at `src/api/client.rs` ~lines 973-997 maps HTTP 401 to `NotAuthenticated`/`InsufficientScope`, NOT to `ApiError{401}`), `JrError::ApiError { status: 5xx, .. }`, network errors, etc. A successful 200 response with the name absent from the list does NOT route here — that is the unresolvable-name sub-path (typo hint). Tested by test #4 (R2: `get_issue` succeeds, project-types 5xx) and test #9 (R1: `get_issue` itself fails).
2. (Cause-2) A fetch returns HTTP 200 but the `subtask` field is absent (`None`) after deserialization → `is_cross_hierarchy_type_error(None, _, _)` or `is_cross_hierarchy_type_error(_, None, _)` returns `Indeterminate`. For the source-issue side, this covers both: (a) the `issuetype` object wholly absent from the response (`Option<IssueType>` is `None`), and (b) `issuetype` present but `subtask` key omitted — both collapse to `src_subtask: None` via `issue.fields.issuetype.as_ref().and_then(|t| t.subtask)`. Tested by test #6 (source-side) and test #7 (target-side).

On either cause:
- Surface the `extract_error_message`-processed 400 message text carried in `JrError::ApiError.message` only. When asserting in tests (#6, #7, #9), choose a substring from the extracted message, not raw JSON envelope keys.
- No enrichment hint of any kind.
- Exit code 1.

**Negative constraint (CRITICAL):** `JRACLOUD-27893` MUST NOT appear on stderr on
either sub-path. This prevents the cross-hierarchy hint from misleading users who
made a plain type-name typo or whose type change is rejected by workflow constraints
(research addendum A4).

## Modified Behavioral Contract

### BC-3.4.003: Errors cross-reference annotation (no behavioral change)

Added an **Errors** field cross-referencing BC-3.4.010 and BC-3.4.011 for the
HTTP 400 `--type` error paths. The primary success path (PUT 204) and ADF description
behavior are unchanged. This is an annotation-only update.

## Count Impact

| Surface | Before | After | Delta |
|---------|--------|-------|-------|
| bc-3-issue-write.md `definitional_count` | 66 | 68 | +2 |
| bc-3-issue-write.md `total_bcs` | 95 | 97 | +2 |
| BC-INDEX.md `total_bcs` | 575 | 577 | +2 |
| BC-INDEX.md Section 3.4 header | 9 BCs | 11 BCs | +2 |
| BC-INDEX.md Section 3 header | 95 BCs cumulative; 66 individually-bodied | 97 BCs cumulative; 68 individually-bodied | +2 |
| BC-INDEX.md summary table Section 3 row | 95 / 66 | 97 / 68 | +2 |
| BC-INDEX.md grand total row | 575 / 343 | 577 / 345 | +2 |
| CANONICAL-COUNTS.md per-file bc-3 | 95 | 97 | +2 |
| CANONICAL-COUNTS.md per-file definitional bc-3 | 66 | 68 | +2 |
| CANONICAL-COUNTS.md Sum | 575 | 577 | +2 |
| CANONICAL-COUNTS.md individually-bodied total | 343 | 345 | +2 |
| Spec version | 1.2.0 | 1.3.0 | MINOR bump |

## Count Verification (guard script expected output)

After all edits, the CI guards must produce exit 0. The implementing story must verify:

```
$ bash scripts/check-spec-counts.sh
OK
$ bash scripts/check-bc-cumulative-counts.sh
OK
$ bash scripts/check-bc-no-numeric-test-counts.sh
OK
```

All three must exit 0 before committing.

## Required Test Deliverables (F3/F4)

> **AUTHORITATIVE COUNT: TEN (10) integration tests.**
> The delta-analysis.md figure of five (5) tests is SUPERSEDED by this F2 spec delta.
> Tests #6 and #7 were added during F2 adversarial review (Pass-3, finding M-2).
> Test #8 was added during Pass-6 adversarial review (finding MAJOR-3): the unresolvable-name
> sub-path (200 response, name absent from list) previously had no named test.
> Tests #9 and #10 were added during Pass-7 adversarial review (findings O-1 and O-2):
> test #9 covers the R1 routing row (`get_issue` itself fails, distinct from R2 covered by test #4);
> test #10 covers the R0b routing row (non-400 `edit_issue` error, no enrichment).
> The F3 story-writer and F4 implementer MUST use TEN as the authoritative count.
> Do not revert to eight.

The implementing story MUST include all ten named integration tests in a new file
`tests/issue_edit_type_errors.rs`:

1. `test_edit_type_cross_hierarchy_std_to_subtask_surfaces_conversion_hint`
   — standard→subtask: GET issue (`subtask: Some(false)`), GET project types (target `subtask: Some(true)`),
   PUT 400 → exit 1, stderr contains `JRACLOUD-27893`,
   stderr does NOT contain `jr api /rest/api/3/issue` (regression pin on removed fake hint)

2. `test_edit_type_cross_hierarchy_subtask_to_std_surfaces_conversion_hint`
   — subtask→standard: reverse direction (`subtask: Some(true)` → `Some(false)`),
   same assertions (contains `JRACLOUD-27893`; does NOT contain `jr api /rest/api/3/issue`)

3. `test_edit_type_same_hierarchy_400_surfaces_typo_hint`
   — both flags `subtask: Some(false)` → exit 1,
   stderr contains `jr project types`,
   stderr contains a known substring from the `extract_error_message`-processed 400 message (e.g., `The issue type selected is invalid` — a substring that survives extraction; do NOT assert raw JSON envelope keys such as `{"errors"` or `"issuetype":`),
   stderr does NOT contain `JRACLOUD-27893` (negative pin),
   stderr does NOT contain `jr api /rest/api/3/issue` (fake-endpoint regression pin — uniform across all `handle_edit` 400-path tests)

4. `test_edit_type_indeterminate_project_types_5xx_surfaces_raw_error`
   — GET issue succeeds (returns `subtask: Some(false)`), GET project types returns 5xx
   → exit 1, `extract_error_message`-processed 400 message present in stderr (assert a known extracted substring), no hint,
   stderr does NOT contain `JRACLOUD-27893` (negative pin, matching test #3),
   stderr does NOT contain `jr api /rest/api/3/issue` (fake-endpoint regression pin — uniform across all `handle_edit` 400-path tests)

5. `test_edit_type_cross_hierarchy_hint_no_fake_endpoint_literal`
   — regression pin: CrossHierarchy 400 path → stderr does NOT contain `jr api /rest/api/3/issue`
   (literal-pin unique to the removed fake `PUT /rest/api/3/issue/{key}/convert` hint;
   mirrors `JRACLOUD-95368` literal-pin pattern in CLAUDE.md)

6. `test_edit_type_indeterminate_absent_subtask_flag_surfaces_raw_error`
   — wiremock `get_issue` returns HTTP 200 with the `subtask` key OMITTED from the issuetype object
   (i.e. the JSON response has `{"issuetype": {"name": "Task"}}` — no `"subtask"` field at all) →
   deserialization produces `subtask: None`; `is_cross_hierarchy_type_error(None, _, _)` returns
   `Indeterminate` → exit 1; `extract_error_message`-processed 400 message present on stderr (assert
   a known extracted substring, not raw JSON envelope keys); no enrichment hint of any kind;
   `JRACLOUD-27893` MUST NOT appear on stderr;
   stderr does NOT contain `jr api /rest/api/3/issue` (fake-endpoint regression pin — uniform across all `handle_edit` 400-path tests).
   This tests Indeterminate Cause-2 (field absent at the source-issue level; corresponds to EC-3.4.011-5).

7. `test_edit_type_indeterminate_absent_target_subtask_flag_surfaces_raw_error`
   — wiremock `get_issue` returns HTTP 200 with source `subtask` field PRESENT (`subtask: Some(false)`);
   wiremock `get_project_issue_types` returns HTTP 200 with the matched target type's `subtask` key
   OMITTED from the response object (i.e., `{"name": "Task"}` — no `"subtask"` field) →
   deserialization produces `tgt_subtask: None`; `is_cross_hierarchy_type_error(Some(false), None, _)`
   returns `Indeterminate` → exit 1; `extract_error_message`-processed 400 message present on stderr
   (assert a known extracted substring, not raw JSON envelope keys); no enrichment hint of any kind;
   `JRACLOUD-27893` MUST NOT appear on stderr;
   stderr does NOT contain `jr api /rest/api/3/issue` (fake-endpoint regression pin — uniform across all `handle_edit` 400-path tests).
   This tests Indeterminate Cause-2 at the target-type level (symmetric to test #6; corresponds to EC-3.4.011-6).

8. `test_edit_type_unresolved_type_name_surfaces_typo_hint`
   — wiremock `get_issue` returns HTTP 200 with source `subtask: Some(false)`;
   wiremock `get_project_issue_types` returns HTTP 200 with a non-empty type list that does NOT
   contain the `--type` value (e.g., list contains `["Story", "Bug", "Task"]` but `--type "Taks"`)
   → unresolvable-name sub-path → typo hint emitted; exit 1;
   stderr contains `jr project types` (from the pinned typo hint);
   `JRACLOUD-27893` MUST NOT appear on stderr (negative pin);
   stderr does NOT contain `jr api /rest/api/3/issue` (fake-endpoint regression pin — uniform across all `handle_edit` 400-path tests).
   This covers the previously-untested unresolvable-name sub-path (EC-3.4.011-3, EC-3.4.011-7).
   Note: the classifier is NOT invoked on this path — the typo hint is emitted by the caller directly before calling `is_cross_hierarchy_type_error`.

9. `test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error`
   — wiremock `edit_issue` returns HTTP 400 (entering the enrichment block);
   wiremock `get_issue` returns HTTP 5xx (or any error, e.g., 503) → `Result::is_err()` is
   true on the `get_issue` call → Indeterminate immediately (R1 routing row; `get_project_issue_types`
   is NOT called — do NOT mount a wiremock for it);
   exit nonzero; raw error on stderr (extract_error_message-processed 400 message);
   NEITHER the cross-hierarchy hint NOR the typo hint appears; `JRACLOUD-27893` MUST NOT appear
   on stderr; `jr api /rest/api/3/issue` absent.
   Distinct wiremock topology from test #4 (R2): test #4 has `get_issue` succeed (HTTP 200) then
   `get_project_issue_types` fail (5xx); test #9 has `get_issue` itself fail (5xx), so
   `get_project_issue_types` is never reached. The two tests exercise different topology branches
   even though both route to Indeterminate. Exercises EC-3.4.011-4.

10. `test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment`
    — wiremock `edit_issue` returns HTTP 403 (a non-400 error — the R0b routing row);
    exit nonzero; raw error on stderr; NEITHER the cross-hierarchy hint NOR the typo hint appears;
    `JRACLOUD-27893` MUST NOT appear on stderr; `jr api /rest/api/3/issue` absent.
    Do NOT mount wiremock stubs for `get_issue` or `get_project_issue_types` — verify (via
    `expect(0)` mock pattern) that no enrichment fetch occurs. The enrichment block is entered
    ONLY when `edit_issue` downcasts to `JrError::ApiError { status: 400, .. }`; a 403 bypasses
    the block entirely. Exercises BC-3.4.010 and BC-3.4.011 negative constraint on the HTTP-400 gate.

Additionally, strengthen T-06 in `tests/issue_edit_no_parent.rs`:
- `test_subtask_parent_clear_surfaces_400_with_convert_hint`: add
  `assert!(stderr.contains("JRACLOUD-27893"))`,
  `assert!(!stderr.contains("jr api /rest/api/3/issue"))`, and
  `assert!(stderr.contains("Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue."))` (pins the full verbatim normative context sentence that MUST be prepended before `CROSS_HIERARCHY_HINT` on the `--no-parent` path; this sentence is a PINNED STRING — the full text is authoritative and the test MUST assert this substring, not a paraphrase).

**Note on T-06 negative-assertion substring:** The pin `jr api /rest/api/3/issue` intentionally supersedes the broader form `/rest/api/3/issue/` proposed in the F1 delta-analysis. The broader form is over-broad and false-positive-prone: the new `CROSS_HIERARCHY_HINT` text itself contains the phrase "To convert it, open the issue in the Jira web UI", and other diagnostic messages may reference REST paths beginning with `/rest/api/3/issue/`. The substring `jr api /rest/api/3/issue` uniquely identifies the removed fake-endpoint hint (`jr api /rest/api/3/issue/{key}/convert -X put -d '{"type":{"name":"Task"}}'`) without matching legitimate content.

Also required: add inline proptest module `#[cfg(test)] mod is_cross_hierarchy_type_error_proptests` in `src/cli/issue/create.rs`
(mirroring the existing `build_labels_proptests` and `parse_field_kv_proptests` precedent — NOT the bare `mod tests`, which already exists in that file and would collide) for the pure
`is_cross_hierarchy_type_error(src_subtask: Option<bool>, tgt_subtask: Option<bool>, err: &str)` function.
This proptest is the primary verification artifact for the classifier's three-branch totality
and is cited in both BC-3.4.010 and BC-3.4.011 Source and Trace fields.
Properties:
- For any `(src: bool, tgt: bool, msg: String)` where `src != tgt`: `is_cross_hierarchy_type_error(Some(src), Some(tgt), &msg)` returns `CrossHierarchy`.
- For any `(v: bool, msg: String)`: `is_cross_hierarchy_type_error(Some(v), Some(v), &msg)` returns `SameCategory`.
- For any `(tgt: Option<bool>, msg: String)`: `is_cross_hierarchy_type_error(None, tgt, &msg)` returns `Indeterminate`.
- For any `(src: Option<bool>, msg: String)`: `is_cross_hierarchy_type_error(src, None, &msg)` returns `Indeterminate`.

## Architecture Notes

- No new module. No new public `JiraClient` method.
- `CROSS_HIERARCHY_HINT: &str` — new shared constant in `src/cli/issue/create.rs`
  (co-located with `is_subtask_parent_error`; referenced by both the `edit --type`
  error block and the `--no-parent` path at `create.rs:834`).
- `is_cross_hierarchy_type_error` — new pure function (sibling of `is_subtask_parent_error`
  at approx. `create.rs:1159`). NOT a replacement of the sibling; both remain.
- `IssueType.subtask: Option<bool>` — additive field with `#[serde(default)]` in
  `src/types/jira/issue.rs`; backward-compatible. `IssueType` is the struct that carries
  the SOURCE issue's type information (from `Issue.fields.issuetype` in the `get_issue`
  response) — this is the struct that GAINS the new `subtask: Option<bool>` field in
  issue #388. `None` is a reachable runtime state (Jira may omit the field), handled as
  `Indeterminate` by the classifier. The TARGET type's `subtask` flag comes from
  `IssueTypeMetadata` (the struct returned by `get_project_issue_types`) — this struct
  ALREADY has `subtask: Option<bool>` and does NOT need a new field for issue #388.
  All struct-literal constructions of `IssueType` in `tests/common/fixtures.rs` must
  add `subtask: None`.
- **BC-authoring checklist addition:** For every `Option`/nullable field a BC branches
  on, confirm the `None`/null branch is assigned a classification. For this feature:
  `subtask: None` on either source or target → `Indeterminate` (not an error, not
  ignored). Apply this check to any future BC that dispatches on `Option<T>`.
- Both API calls (`get_issue`, `get_project_issue_types`) already exist; no new
  signatures required.
- The bulk `--type` path (`handle_edit_bulk_fields`) is NOT affected and must not
  receive this enrichment.

## Open Items for F3/F4

- Verify `tests/common/fixtures.rs` for `IssueType { name: "..." }` struct-literal
  constructions and add `subtask: None` to each.
- Decide whether to add a CLAUDE.md gotcha entry for `is_cross_hierarchy_type_error`
  (out of F2 scope; implementer decides at F4 if warranted).
- **[RESOLVED — m-1]** The `get_issue` call uses the full `BASE_ISSUE_FIELDS` projection (confirmed in `src/api/jira/issues.rs:13-25`), which includes `"issuetype"`. The Atlassian Jira Cloud REST API v3 returns the complete `IssueType` object — including the `subtask` boolean and `hierarchyLevel` — as a nested field within any projected `issuetype` field. The `fields=` query parameter on `/rest/api/3/issue/{key}` filters top-level issue fields only; it does NOT filter nested properties within a returned field. Therefore `get_issue` (with `issuetype` in `BASE_ISSUE_FIELDS`) reliably returns the `subtask` sub-field. F4 wiremock fixtures MUST include `"subtask": true/false` in the `issuetype` object for tests #1, #2, #3, #4, #5 — and OMIT `"subtask"` for tests #6 (source-side) and #7 (target-side). No open item remains. (Note: The prior open-item description referenced `?fields=issuetype` as the actual query — this was incorrect. The actual query uses the full `BASE_ISSUE_FIELDS` list. This correction is informational only; it does not change the confirmed response shape.)

## F3 Authoritative Test Count Note (c-2)

The authoritative count of **ten (10)** integration tests for `tests/issue_edit_type_errors.rs` is
specified in this F2 delta (revised from 7 during Pass-6 adversarial review, finding MAJOR-3; revised
from 8 during Pass-7 adversarial review, findings O-1 and O-2). BC bodies deliberately avoid numeric
test counts per project convention (enforced by `scripts/check-bc-no-numeric-test-counts.sh`). The F3
story is responsible for carrying the ten-test count into the durable story spec as the canonical
implementation reference. The BC bodies cite tests by name only; the count lives in this delta
document and the F3 story.

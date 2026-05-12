---
document_type: copilot-review-progress
level: ops
version: "1.0"
status: in-progress
producer: state-manager
pr: 358
issue: 343
branch: chore/edit-field-categorization-test-343
head_sha: 925da89
created: 2026-05-12
---

# PR #358 Copilot Review Progress — edit-field-categorization-test (#343)

## PR Summary

**Title:** chore(test): unit test asserting every Edit field is categorized (#343)
**Branch:** chore/edit-field-categorization-test-343
**Head SHA:** 29608b8
**Labels:** test, audit-followup
**Source:** F5 adversarial review process-gap finding from issue #110 part 2

**Change description:** Test-only PR. Adds `test_343_every_edit_field_is_categorized` in
`src/cli/issue/create.rs::tests` module. Helper `extract_edit_field_names` parses
`src/cli/mod.rs` via `include_str!` and extracts `IssueCommand::Edit` fields. Three
hand-maintained sets:
- `SELECTORS` (5): fields that select by name/list
- `BULK_SUPPORTED` (4): fields allowed in `issue edit --jql` bulk mode
- `REJECTED_IN_BULK` (8): fields rejected with an error in bulk mode

Total: 17 fields; assertions verify union completeness + pairwise disjoint + non-empty.
255 lines added; zero source-code paths touched.

**Test results at PR open:**
- 1 new test passes
- Full cargo test: 61 groups, 1249 passed, 0 failed
- cargo fmt --check: CLEAN
- cargo clippy --all-targets -- -D warnings: CLEAN

**Perplexity skip justification:** Test mechanics only; no external behavior, library API,
or Atlassian API contract to validate per Lesson 1 boundary.

---

## Copilot Review Rounds

### Round 1 — R1 COMPLETE 2026-05-12

| Field | Value |
|-------|-------|
| Status | COMPLETE |
| Requested at | 2026-05-12 |
| Review ID | 4268914353 |
| Findings | 1 |
| Perplexity validations | n/a (test mechanics only; Lesson 1 boundary — no external behavior to validate) |
| Fix commits | 9ca690e (chore(test): use BTreeSet for deterministic test failure diffs) |
| Threads resolved | 1/1 — PRRT_kwDORs-xfc6BSISi |
| Reply comment ID | 3223525042 |
| CI on fix commit | 8/8 green |
| Trajectory | 1→R2 |

**Finding C1** (comment 3223512008): Doc comment on `extract_edit_field_names` claimed it
returned an "alphabetically-stable HashSet." HashSet iteration order in Rust is
hash-seed-dependent, so the claim was inaccurate AND assertion failure messages would
produce nondeterministic field orderings in CI output.

**Fix:** Switched all set types in the test to BTreeSet:
- Function return type: `HashSet<String>` → `BTreeSet<String>`
- Internal accumulator: `HashSet::new()` → `BTreeSet::new()`
- Caller-side sets: `selectors`, `bulk_supported`, `rejected_in_bulk`
- Pairwise intersection temporaries: `s_b`, `s_r`, `b_r`
- `categorized` union set
- Doc comment updated to explain the BTreeSet choice (deterministic iteration order
  ensures stable failure messages when sets diverge).

**Perplexity validation:** skipped per Lesson 1 boundary — finding concerns Rust
`std::collections` semantics (HashSet vs BTreeSet ordering), a well-established language
fact not requiring external API or library validation.

### Round 2 — R2 COMPLETE 2026-05-12

| Field | Value |
|-------|-------|
| Status | COMPLETE |
| Requested at | 2026-05-12 |
| Review ID | 4268937977 |
| Findings | 1 |
| Perplexity validations | n/a (test mechanics only; Lesson 1 boundary — no external behavior to validate) |
| Fix commits | c708211 (chore(test): tolerate formatting variants in extract_edit_field_names matcher) |
| Threads resolved | 1/1 — PRRT_kwDORs-xfc6BSMuX |
| Reply comment ID | 3223556249 |
| CI on fix commit | 8/8 green |
| Trajectory | 1→1→R3 |

**Finding C1** (comment 3223535825): `extract_edit_field_names` closing-brace detection used
the exact string `"    },"` — formatting-fragile under three real-world source variants:
(a) last enum variant `}` with no trailing comma, (b) `},  // comment` trailing inline
comment, (c) trailing whitespace after the brace or comma.

**Fix:** Introduced `is_matching_closing_brace` closure that accepts any line at the same
indentation depth whose trimmed content starts with `}` and is optionally followed by `,`
and/or whitespace/comment. Three new unit tests added to exercise the closure:
- `test_extract_edit_field_names_no_trailing_comma` — last-variant brace without comma
- `test_extract_edit_field_names_trailing_comment` — `},  // comment` form
- `test_extract_edit_field_names_trailing_whitespace` — brace with trailing spaces

All 4 original #343 tests still pass. Full cargo test: 1252 passed (+3), 0 failed.

**Perplexity validation:** skipped per Lesson 1 boundary — finding concerns string-matching
logic in a test helper, a Rust code-quality issue not requiring external API or library
validation.

### Round 3 — R3 COMPLETE 2026-05-12

| Field | Value |
|-------|-------|
| Status | COMPLETE |
| Requested at | 2026-05-12 |
| Fix commit | 925da89 (chore(test): align doc + remove dead-code check in field extractor) |
| Findings | 2 |
| Perplexity validations | n/a (test mechanics only; Lesson 1 boundary — no external behavior to validate) |
| Threads resolved | 2/2 — PRRT_kwDORs-xfc6BSS3f, PRRT_kwDORs-xfc6BSS3r |
| Reply comment IDs | 3223583146, 3223583216 |
| CI on fix commit | 8/8 green |
| Trajectory | 1→1→2→R4 |

**Finding C1** (comment 3223569286 / thread PRRT_kwDORs-xfc6BSS3f): The strategy doc
comment on `extract_edit_field_names` still described the pre-R2 matching behavior:
"8-space indent + `},` exact close." After R2 introduced `is_matching_closing_brace`,
the strategy doc was not updated. The surrounding inline `Logic:` block also referenced
"8-space indent (clap variant fields use 8-space indent)," which described a hardcoded
assumption the code no longer makes.

**Fix:** Updated strategy doc to describe the actual `trim_start` + tolerant matcher
behavior. Updated the `Logic:` block to explain the real byte-positioning mechanism:
indent depth is computed by searching for the `Edit {` opening line and measuring its
leading whitespace — no hardcoded assumption about 8 spaces. Reply 3223583146.

**Finding C2** (comment 3223569301 / thread PRRT_kwDORs-xfc6BSS3r): Redundant
`rest.starts_with(' ')` check in the `is_matching_closing_brace` closure. After
`strip_prefix('}')` succeeds, `rest` contains whatever follows `}` in the line —
which for a valid closer is always `,`, `//`, whitespace-then-comment, or nothing;
never a space. The space-check can never be true. Dead code.

**Fix:** Removed the dead `rest.starts_with(' ')` branch. Updated the adjacent comment
to explain that deeper-indent closers are rejected via the byte-positioning mechanism:
`strip_prefix` fails when the line has more leading whitespace than expected, so any
line at a deeper indent level will not match the expected-indent prefix and the closure
returns false before any content check. Reply 3223583216.

**Process observation:** R3 is a doc-fallout cluster from R2. The codified doc-fallout
lesson (PR #356 R14-R18, 2026-05-12) was not applied when c708211 was pushed — the
strategy doc and `Logic:` block were ~15 lines above the changed `is_matching_closing_brace`
closure, and were not audited together with it. Sub-lesson added to lessons.md:
"grep narration-style comments (Strategy:, Logic:, etc.) before pushing a behavior-expanding commit."

**Perplexity validation:** skipped per Lesson 1 boundary — findings concern doc-comment
accuracy for internal test helper logic; no external API, library, or language behavior
to validate.

---

## Trajectory

| Round | Findings | Delta | Notes |
|-------|----------|-------|-------|
| R1 | 1 | — | Review 4268914353; BTreeSet fix 9ca690e; 1/1 threads resolved; CI 8/8 green |
| R2 | 1 | 0 | Review 4268937977; tolerant brace matcher c708211; 3 new edge-case tests; 1/1 threads resolved; CI 8/8 green |
| R3 | 2 | +1 | Fix commit 925da89; 2 doc-fallout findings from R2 tolerant-matcher (stale strategy doc + dead-code space-check); 2/2 threads resolved; CI 8/8 green |
| R4 | pending | — | Pending |

---

## Resolution Status

| Status | Value |
|--------|-------|
| Overall | IN_PROGRESS |
| Converged | no |
| Merged | no |
| Merge SHA | — |
| Closes issue | #343 |

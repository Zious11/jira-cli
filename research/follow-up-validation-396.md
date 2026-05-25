# Follow-Up Validation — Issue #396 VSDD Cycle

**Date:** 2026-05-25
**Worktree HEAD:** main repo, `develop` @ `699a5fd` (post FIX-F5-001 merge)
**Validator:** Research agent (Claude)
**Source authority:** Local code inspection (no Perplexity used except for one
external library question — see Item 5).

---

## Tooling Note

The agent's `Grep` and `Glob` tools were unavailable in this session
(ripgrep binary missing — `ENOENT: no such file or directory, posix_spawn
'rg'`). All validation was performed via sequential `Read` calls against
known-path files. Coverage may have minor blind spots for "is this string
also referenced anywhere else?" questions — for those I have flagged
"based on the test files I directly inspected" rather than claim
exhaustive negative coverage.

---

## Item 1 (DI-396-F5-1) — `--label` conflict block coverage gap

**Verdict:** CONFIRMED.

**Severity:** MED (coverage gap, not a correctness defect).

**Evidence:**

- The conflict block at `src/cli/issue/create.rs:445-492` enumerates
  exactly the 12 entries claimed:
  `--summary` (448), `--priority` (451), `--type` (454), `--team` (457),
  `--points` (460), `--no-points` (463), `--parent` (466),
  `--no-parent` (469), `--description` (472),
  `--description-stdin` (475), `--markdown` (478), `--field` (481).
- The error message format at `src/cli/issue/create.rs:484-489` is:
  `"--label cannot be combined with {joined} in the same call. …"`
- In `tests/issue_edit_field.rs` (file ends at line 2910) only two
  tests exercise this block:
  - `test_label_plus_field_rejected_with_exit_64_no_http` at
    `tests/issue_edit_field.rs:2817-2856`
  - `test_label_plus_summary_rejected_with_exit_64_no_http` at
    `tests/issue_edit_field.rs:2868-2909`
- The in-file comment at `tests/issue_edit_field.rs:2862-2865` is
  candid: *"Pre-existing coverage gap: there was ZERO test coverage
  for the entire --label conflict block before FIX-F5-001."* So
  FIX-F5-001 brought coverage from 0/12 to 2/12; the other 10 entries
  remain uncovered.

**Mutation-escape reasoning (Item 1 ask #3):** Deleting any of the 10
uncovered `push("--xxx")` lines would compile cleanly and pass the
existing meta-test (`test_343_every_edit_field_is_categorized` at
`src/cli/issue/create.rs:1523-1637`) because the meta-test only
enforces SELECTORS/BULK_SUPPORTED/REJECTED_IN_BULK partition
correctness (lines 1571-1636) — it never inspects the conflict
block. So mutation testing would tag the deletion as a viable
mutant.

**Recommendation:** FILE — but MERGE-WITH-OTHER (combine with Item 2,
since the structural fix is the same family of work). One
parameterized table-driven test would cover all 11 cases in
~30 LOC. Alternatively, the meta-test in Item 2 makes per-flag
positive-regression tests redundant; pick one or the other but not
both.

---

## Item 2 (DI-396-F5-2 process-gap) — no structural meta-test for conflict-block completeness

**Verdict:** CONFIRMED.

**Severity:** MED.

**Evidence:**

- The meta-test at `src/cli/issue/create.rs:1523-1637`
  (`test_343_every_edit_field_is_categorized`) does exactly what its
  doc comment says (lines 1486-1521): asserts the union of three
  hard-coded sets equals the extracted Edit-variant field set, and
  asserts pairwise disjointness. It does NOT touch the
  `--label` conflict block at all.
- No other test in `tests/issue_edit_field.rs` enforces conflict-block
  completeness (I read every test in that file via direct page reads;
  the only conflict-block tests are the two from Item 1).
- I could not exhaustively grep the entire codebase due to the
  rg-tool outage. Confidence on "no test anywhere enforces this" is
  HIGH but not 100 percent.

**Mechanizability check (Item 2 ask #3):** The proposed meta-test IS
mechanizable using the same `include_str!` + source-text pattern as
the existing meta-test:

```rust
// Pseudo-code: extract every `conflicting.push("--<flag>")` line
// from src/cli/issue/create.rs::handle_edit's --label block.
let create_rs = include_str!("create.rs");
let pushes = extract_label_conflict_pushes(create_rs);
// Expected set = (BULK_SUPPORTED \ {"label"}) ∪ REJECTED_IN_BULK
// mapped from snake_case test-side names to clap kebab-case flag names.
```

The existing extractor (`extract_edit_field_names` at
`src/cli/issue/create.rs:1713-1779`) is a robust precedent and copes
with rustfmt drift; a parallel extractor scoped to the
`if !labels.is_empty() { … }` block at lines 445-492 is the same
shape of work.

**Recommendation:** FILE — MERGE WITH ITEM 1. The structural
meta-test is the durable fix; the per-flag positive tests are
incidental coverage. Filing both as one issue keeps the scope
coherent ("`--label` conflict block: structural enforcement + the
gap that motivated it").

---

## Item 3 (DI-396-F5-3) — clap `--field` help text doesn't mention `--label`

**Verdict:** PARTIAL.

**Severity:** COSMETIC.

**Evidence:**

- `src/cli/mod.rs:469-474` shows the `--field` help verbatim:

  ```rust
  /// Arbitrary custom field values as NAME=VALUE pairs (repeatable).
  /// The first '=' splits name from value; subsequent '=' are part of the value.
  /// Duplicate keys use the last value provided. Single-key path only (rejected
  /// in bulk-edit context). See also: CLAUDE.md Gotchas — `--field` on issue edit.
  ```

  It mentions bulk-edit rejection and points at CLAUDE.md; it does
  NOT mention `--label`.
- CLAUDE.md DOES document the `--label` + `--field` conflict —
  `/Users/zious/Documents/GITHUB/jira-cli/CLAUDE.md:333-338` (gotcha
  (6) in the `issue edit --field` block). So the "See also"
  cross-reference does land the user on the right text.
- For the other Edit flags at `src/cli/mod.rs:460-475`:
  `--description` (461) and `--description-stdin` (464) use clap's
  `conflicts_with = "description_stdin"` / `"description"` attribute
  pair — that's clap-native conflict detection, not help-text prose.
  `--markdown` (467) has no conflicts noted in its help. No other
  Edit-mod flag's help text enumerates its prose-level conflicts.

**So:** the codebase convention is NOT "every flag's help text
enumerates its conflicts." The convention is "clap-native
`conflicts_with` for parser-level pairs; CLAUDE.md for runtime
mutual-exclusion that crosses arg-group boundaries." The `--field`
help follows that convention by pointing at CLAUDE.md.

**Recommendation:** DON'T FILE. The "See also: CLAUDE.md Gotchas"
cross-reference is the documented affordance, and the codebase
convention does not require enumerating runtime conflicts in help
text. If the user-experience desire is "make every conflict
discoverable from `--help` alone" that's a separate, larger UX
discussion that should be filed once for ALL Edit flags, not
selectively for `--field`.

---

## Item 4 (DI-396-F5-4) — EC-3.4.017-13 line-anchor citation drift

**Verdict:** CONFIRMED, and the surrounding BC text has additional
drift that the original claim did not enumerate.

**Severity:** LOW (documentation accuracy, not behavior).

**Evidence (from `.factory/specs/prd/bc-3-issue-write.md`):**

- EC-3.4.017-13 at line 1529-1537 cites:
  - `src/cli/issue/create.rs:~835` for the `--label` short-circuit.
    Actual location: line 838-841. The `~` softens the citation but
    the off-by-3 is still inaccurate.
  - "lines 445-489" for the `--label` mutual-exclusion block. Actual
    range: 445-492 (the block ends at the closing `}` on line 492;
    line 489 is the `.join(", ")` argument, not the block end).

**Additional drift uncovered in the same file:**

- EC-3.4.017 invariant 2 at line 1469: refers to
  "`create.rs:1435+`" for the partition meta-test. Actual location
  of `test_343_every_edit_field_is_categorized`: 1523-1637. Line
  1435 is inside `is_cross_hierarchy_type_error` — unrelated code.
- EC-3.4.017-10 at line 1507: cites `parse_field_kv` at
  "`src/cli/issue/create.rs:1982-1997`". Actual location: 2086-2101.
  Line 1982-1997 is inside the `parse_field_kv_proptests` module,
  not the function itself.
- CLAUDE.md `--field` gotcha (6) at `CLAUDE.md:334`: refers to
  "`src/cli/issue/create.rs:~445 (the --label mutual-exclusion
  block)`" (the `~` softener is present; benign). It also says
  `create.rs:~835` for the routing fork — same off-by-3 as
  EC-3.4.017-13.

**Recurring-class assessment:** Yes — at least 4 stale line-anchor
citations exist in just the two BC entries adjacent to FIX-F5-001
(EC-3.4.017-10, EC-3.4.017-13, and invariant 2). I did not survey
the rest of `bc-3-issue-write.md` (2152 lines) or the other 11+ BC
files in `.factory/specs/prd/`. Given that I found 4 stale citations
in approximately 100 adjacent lines, the codebase-wide tally is
plausibly in the dozens.

**Recommendation:** FILE — but as a SEPARATE issue from Items 1+2,
because:
- The class of fix is different (citation maintenance vs.
  test/meta-test code).
- The right long-term remedy is process-level (e.g., switch
  citations to symbol form like
  `create.rs::handle_edit::label_conflict_block` instead of line
  numbers, OR add a CI check that resolves line ranges and warns on
  drift). That's an infrastructure conversation, not a per-citation
  patch.
- Several of the cited stale ranges are unrelated to FIX-F5-001
  (invariant 2 cites line 1435 — pre-FIX-F5-001 drift; the function
  citation 1982-1997 has nothing to do with FIX-F5-001 either).
  Lumping them into a FIX-F5-001 follow-up confuses the
  archaeology.

The right framing for the issue: "Replace line-anchor citations in
bc-3-issue-write.md and CLAUDE.md `--field` gotcha with symbol-form
or rustdoc-anchored citations; OR add a CI guard that resolves
cited line ranges against current HEAD and flags drift."

---

## Item 5 (R2-C4 carry from F4) — test 38 reimplements wire-serialization inline

**Verdict:** REFUTED. The carry-forward recommendation was based on
an incorrect assumption about how `wiremock::matchers::body_partial_json`
handles JSON number types.

**Severity:** N/A (the original claim doesn't hold).

**Evidence:**

- Test 38 at `tests/issue_edit_field.rs:2433-2477` does indeed
  reimplement the `f64 → serde_json::Value` decision logic inline
  (lines 2436-2440, 2449-2455, 2465-2470).
- Tests 26 and 27 at `tests/issue_edit_field.rs:1660-1714` (test 26)
  and 1722 onward (test 27) DO use `body_partial_json` matchers
  pinning the wire shape:
  - Test 26 matches `{"fields": {"customfield_20001": 5}}` (integer
    literal, line 1687-1689).
  - Test 27 (continues past line 1750, similar structure) matches
    integer `5000` for the `"5e3"` input.
- The load-bearing question is whether `body_partial_json`
  distinguishes `5` from `5.0`. From the wiremock-rs source and
  `assert-json-diff` docs:
  - `BodyPartialJsonMatcher` delegates to
    `assert_json_matches_no_panic(&body, &self.0, Config::new(CompareMode::Inclusive))`.
  - `assert_json_diff::Config::new(CompareMode::Inclusive)` defaults
    `numeric_mode` to `NumericMode::Strict` (verified against the
    crate docs).
  - `NumericMode::Strict` documentation: *"Different numeric types
    aren't considered equal."*
- Therefore tests 26 and 27 DO catch a regression that emits float
  `5.0` instead of integer `5` on the wire — `body_partial_json` will
  fail the match, the `.expect(1)` mock will register zero matching
  hits, and the `MockServer` drop assertion fires.
- That means test 38's coverage IS redundant with tests 26 and 27 at
  the wire-form level. But the duplicate logic is a maintenance
  cost: if anyone "fixes" the f64→Number conversion in production
  code, test 38 won't fail (it has its own copy of the logic), but
  tests 26 and 27 will. So test 38 acts as a frozen-spec snapshot
  of the algorithm rather than a live invariant — confusing.

So Item 5's claim "removing test 38 would lose no coverage" is
technically true at the wire-shape level (tests 26/27 cover it), but
the original F4 R2 recommendation (delete OR extract a helper)
remains correct. The carry-forward CAN be re-validated — but the
substantive ask is "what's the right disposition for test 38?" not
"is the wire-form actually pinned?"

**Recommendation:** DON'T FILE as-is, but DO file a different,
sharper issue: "Test 38 duplicates production logic inline; either
extract `parsed_number_to_wire_value(f64) -> serde_json::Value` and
have test 38 call it (so a fix to the production logic also
exercises the test), OR delete test 38 entirely (since tests 26/27
cover the wire form via wiremock's strict `body_partial_json`
matcher)." This sharpens the F4 R2 recommendation, which was
ambiguous about which option was preferred.

If choosing between the two: EXTRACT THE HELPER. Pure-function unit
tests are cheaper to maintain than wiremock integration tests, and
the f64→i64-or-f64 branch logic has 4-5 edge cases (NaN, infinity,
i64::MAX boundary, denormals) that aren't worth standing up
wiremock for. Test 38 becomes a useful pure-function regression
suite once it calls a shared helper.

---

## Item 6 (F6 surface — pre-existing) — 9 macOS-keychain-blocking tests

**Verdict:** PARTIAL / mostly REFUTED for the specific numbers, but
the underlying concern (some tests trigger keychain prompts on
macOS dev machines post-implementation) has merit.

**Severity:** LOW (developer-machine ergonomics; CI is unaffected
because CI runs Linux).

**Evidence per file:**

### `tests/auth_profiles.rs` (claim: 1 blocking test)

REFUTED. The two keychain-touching tests in this file are ALREADY
gated:
- `auth_login_creates_new_profile_with_url` at line 242-243:
  `#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]`
- `auth_login_with_jr_profile_pointing_to_unrelated_profile_still_creates_target`
  at line 290-291: same `#[ignore]` annotation.
- The remaining tests (`auth_switch_unknown_profile_exits_64`,
  `auth_list_shows_no_profiles_for_fresh_install`,
  `auth_status_fresh_install_no_profiles_succeeds`,
  `auth_status_unknown_profile_exits_64`,
  `auth_logout_unknown_profile_exits_64`,
  `auth_remove_active_profile_exits_64`,
  `precedence_flag_overrides_env_overrides_config`,
  `global_profile_flag_targets_auth_status`) do not touch the
  keychain — they only manipulate `XDG_CONFIG_HOME`.

### `tests/multi_cloudid_disambiguation.rs` (claim: 4 blocking tests)

PARTIAL. None of the 12 tests in this file is marked `#[ignore]`,
but the harness at `tests/multi_cloudid_disambiguation.rs:155-216`
uses `JR_SERVICE_NAME=jr-test-mc-<pid>-<tid>-<nanos>` (a unique
per-invocation service name) to isolate keychain entries.

The macOS keychain prompt-on-read behavior:
- A unique `JR_SERVICE_NAME` per invocation means each test creates
  fresh credentials in the keychain under a unique service name. The
  first WRITE creates the entry and macOS prompts for permission to
  store it.
- macOS's "Always Allow" choice is keyed to the
  `(executable, service)` tuple. Since each invocation has a fresh
  service name, no "Always Allow" can be remembered — every test
  invocation triggers a fresh prompt on developer macOS machines.
- CI doesn't see this because:
  - Linux CI uses secret-service which has no per-app-confirmation
    requirement, AND
  - Even if Linux CI lacks secret-service (per CLAUDE.md note at
    line 351), these tests either fail-fast on the `keyring` crate
    error or skip the keychain write entirely depending on the OS
    Keyring backend.

So the practical blocking count on a developer Mac is **all 10
tests that go through `jr_isolated`** (lines 248, 284-285, 342-343,
429-430, 502-503, 574-575, 658-659, 801-802, 883-884, 969-970), not
4 — the original claim under-counted. Two `--help`-only tests
(`test_cloud_id_flag_recognized_in_help` at 248 and
`test_cloud_id_help_text_mentions_disambiguation_or_multiple_orgs`
at 1048) don't touch the keychain.

### `tests/oauth_refresh_integration.rs` (claim: 4 blocking tests
beyond the 3 the orchestrator already knew about)

REFUTED for the count. The 4 keychain-gated tests are ALREADY
properly gated:
- `test_refresh_persists_rotated_tokens_via_store_oauth_tokens` at
  line 327-328: `#[tokio::test] #[ignore]` + early-return
  `if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1")`
  at line 330.
- `test_waiters_use_in_memory_token_not_keychain` at line 993-994:
  same pattern.
- `test_inter_process_reconcile_after_invalid_grant` at line
  1116-1117: same pattern.
- `test_persist_before_publish_fault_injection` at line 1303-1304:
  same pattern.

CONFIRMED concern: the "always-run" tests (AC-001, AC-003, AC-004v1,
AC-004v2, AC-005, AC-006, AC-008) do NOT set `JR_SERVICE_NAME`. They
use `JiraClient::new_for_test(...)` (profile="default"), and once
the auto-refresh wiring lands, those tests will call
`refresh_oauth_token("default")` which reads the REAL `jr-jira-cli`
keychain service. On developer macOS that DOES trigger a prompt.
But these tests are currently labelled "pre-implementation RED" and
their failure mode is documented (line 99-103 of the file). Once
implementation lands, they'll prompt — and at that point they need
isolation, not necessarily `#[ignore]`.

### Convention confirmation (Item 6 ask #4)

CLAUDE.md `:351` documents:
*"Keyring round-trip tests are gated behind `JR_RUN_KEYRING_TESTS=1`
+ `#[ignore]` because Linux CI may lack secret-service"*

CLAUDE.md `:352` documents the
`tests/oauth_embedded_login.rs::embedded_login_uses_fixed_port`
pattern (gated by `JR_RUN_OAUTH_INTEGRATION=1` + `#[ignore]`),
which is the canonical model.

So yes, the project does have a documented convention. The
keychain-touching tests in `oauth_refresh_integration.rs` already
follow it. The ones in `multi_cloudid_disambiguation.rs` use a
DIFFERENT isolation strategy (unique-per-invocation
`JR_SERVICE_NAME`) and are NOT gated, which is the actual gap.

### Pre-existing vs. introduced-by-#396 (Item 6 ask #2)

CONFIRMED pre-existing. None of `tests/auth_profiles.rs`,
`tests/multi_cloudid_disambiguation.rs`, or
`tests/oauth_refresh_integration.rs` was touched by S-396 or
FIX-F5-001. S-396 added/touched
`tests/issue_edit_field.rs::test_label_plus_field_rejected_...` and
the existing-coverage test alongside; FIX-F5-001 added the
conflict-block line in `src/cli/issue/create.rs:480-482` and the
two label-conflict tests in `tests/issue_edit_field.rs`. Neither
touched OAuth/auth-profile tests.

**Recommendation:** FILE — but reshape the scope.

The correct ask is NOT "9 tests need `#[ignore]`". The correct asks
are TWO things:
1. `tests/multi_cloudid_disambiguation.rs` — 10 tests that go
   through `jr_isolated()` use a unique-per-invocation
   `JR_SERVICE_NAME` but no `#[ignore]`. On developer macOS each
   invocation triggers a fresh keychain prompt. Either (a) `#[ignore]`
   + `JR_RUN_KEYRING_TESTS=1` gate them like
   `oauth_refresh_integration.rs`, OR (b) document that they're
   expected to prompt on macOS and add a setup note.
2. `tests/oauth_refresh_integration.rs` — the 7 "always-run" tests
   (AC-001, AC-003..AC-008) will start hitting the developer's REAL
   keychain service (`jr-jira-cli`) once the auto-refresh
   implementation lands. They need `JR_SERVICE_NAME` set to a test
   namespace OR a `#[ignore]` gate. Currently they fail-fast
   pre-implementation, but that's not durable.

The right framing for the issue is: "Auth-touching integration
tests need a uniform isolation strategy on developer macOS — pick
ONE convention (unique-service-name OR
`#[ignore]`+`JR_RUN_KEYRING_TESTS`) and apply it consistently across
`tests/auth_profiles.rs`, `tests/multi_cloudid_disambiguation.rs`,
and `tests/oauth_refresh_integration.rs`. Document the chosen
convention in CLAUDE.md."

This is a single MED-severity infrastructure issue, not 9 separate
test patches.

---

## Final Summary Table

| Item | Verdict | Severity | File/Don't | Group |
|------|---------|----------|------------|-------|
| 1 — `--label` block coverage | CONFIRMED | MED | FILE | Group A |
| 2 — meta-test for conflict block | CONFIRMED | MED | FILE | Group A |
| 3 — clap help mentions `--label` | PARTIAL | COSMETIC | DON'T FILE | — |
| 4 — EC line-anchor drift | CONFIRMED++ | LOW | FILE | Group B |
| 5 — test 38 wire-serialization | REFUTED-as-stated | N/A | FILE-REFRAMED | Group C |
| 6 — 9 keychain-blocking tests | PARTIAL | LOW-MED | FILE-RESHAPED | Group D |

**Recommended GitHub-issue scoping:**

1. **One issue, Group A (Items 1 + 2):** "Add structural meta-test
   for `--label` mutual-exclusion block in `handle_edit`."
   - The meta-test (Item 2) is the durable fix; the per-flag tests
     (Item 1) become redundant once it lands.
   - One PR, scope ~50 LOC of test code, no production code changes.

2. **One issue, Group B (Item 4):** "Replace stale line-anchor
   citations in `bc-3-issue-write.md` and CLAUDE.md gotchas; add CI
   guard against drift."
   - Don't try to fix individual citations; pick a symbol-form
     convention (or a `scripts/check-line-anchors.sh` CI guard
     that resolves cited ranges against current HEAD).
   - Likely needs design discussion before implementation — file as
     a discussion issue first.

3. **One issue, Group C (Item 5 reframed):** "Extract
   `parsed_number_to_wire_value(f64) -> serde_json::Value` helper;
   refactor test 38 to call it (or delete test 38 since tests 26/27
   pin the wire form via wiremock strict number matching)."
   - Small scope (~20 LOC); pick the helper option for cleanliness.

4. **One issue, Group D (Item 6 reshaped):** "Unify
   auth-touching-test isolation convention on developer macOS;
   apply consistently across `auth_profiles.rs`,
   `multi_cloudid_disambiguation.rs`,
   `oauth_refresh_integration.rs`; document in CLAUDE.md."
   - This is process work, not 9 test patches.

5. **DON'T FILE Item 3.** It's CLI-UX preference, not a defect; the
   `See also: CLAUDE.md` cross-reference is the documented
   affordance and matches the codebase convention.

**Net:** 4 GitHub issues, not 6. Save reviewer cycles by combining
1+2 (same fix family) and skipping 3 (no-op).

---

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity perplexity_ask | 0 | — |
| Perplexity perplexity_search | 0 | — |
| Perplexity perplexity_research | 0 | — |
| Perplexity perplexity_reason | 0 | — |
| Context7 | 0 | — |
| Tavily tavily_search | 0 | — |
| Tavily tavily_research | 0 | — |
| Tavily tavily_extract | 0 | — |
| Tavily tavily_crawl | 0 | — |
| Tavily tavily_map | 0 | — |
| WebFetch | 4 | wiremock-rs `BodyPartialJsonMatcher` source on GitHub; `assert-json-diff` `NumericMode` and `Config` docs on docs.rs — needed to confirm Item 5's "strict number matching" claim |
| WebSearch | 1 | wiremock-rs body_partial_json integer vs float — backup search for Item 5 |
| Local Read | ~22 | `src/cli/issue/create.rs` (multiple regions), `src/cli/mod.rs` (Edit variant flags), `src/api/auth.rs`, `tests/issue_edit_field.rs`, `tests/auth_profiles.rs`, `tests/multi_cloudid_disambiguation.rs`, `tests/oauth_refresh_integration.rs`, `tests/oauth_embedded_login.rs`, `tests/issue_commands.rs`, `.factory/specs/prd/bc-3-issue-write.md`, `CLAUDE.md` |
| Training data | 0 areas | Not relied on; all claims grounded in local file reads or web-fetched docs |

**Total external tool calls:** 5 (1 WebSearch + 4 WebFetch); the
remainder are local reads.

**Training data reliance:** Low. The only training-data inference
was "macOS `SecKeychainFindGenericPassword` prompts on
unrecognized-service first-write" (Item 6 analysis). That's stable
macOS behavior documented in Apple's Security framework reference
and unchanged since macOS 10.6; I have not re-validated it against
docs in this session.

**Tool-availability caveat:** `Grep` and `Glob` were unavailable
(ripgrep binary missing). Negative-existence claims ("no other
test in the codebase covers X") are stated based on directly-read
files only; a true exhaustive grep would marginally strengthen
those claims but is unlikely to change the verdicts.

---

## Sources (Item 5 external validation)

- [wiremock-rs `matchers.rs` source — `BodyPartialJsonMatcher`](https://github.com/LukeMathWalker/wiremock-rs/blob/main/src/matchers.rs)
- [`assert-json-diff::NumericMode` docs](https://docs.rs/assert-json-diff/latest/assert_json_diff/enum.NumericMode.html)
- [`assert-json-diff::Config` docs](https://docs.rs/assert-json-diff/latest/assert_json_diff/struct.Config.html)
- [`wiremock::matchers::body_partial_json` docs](https://docs.rs/wiremock/latest/wiremock/matchers/fn.body_partial_json.html)

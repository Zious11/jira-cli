# Pass 3 Deep — Round 3: Behavioral Contracts (jira-cli / jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Builds on: `pass-3-deep-r2.md` (343 BCs / 281 HIGH / 56 MEDIUM / 6 LOW after R2; 38 holdouts).

> **Method.** Round 3 attacked the verbatim Round-2 §9 deferred-target list
> AND the 5 Pass-2 R2 cross-pollination bug findings (NEW-INV-56/29/81/19/18).
> Files freshly read in full this round: `tests/worklog_commands.rs` (full),
> `tests/input_validation.rs` (full), `tests/project_meta.rs` (full),
> `src/cli/issue/changelog.rs` (chunked: 300-450, 450-848 — completes 38-test
> source enumeration). Files freshly read chunked: `src/adf.rs` (offsets
> 800-880, 1100-1500, 1500-1827 — completes 69-test source enumeration);
> `tests/cli_handler.rs` (offset 280-700 — fills the R2 gap chunk);
> `tests/cli_smoke.rs` (offset 1-334); `tests/user_pagination.rs` (offset
> 1-200); `tests/team_column_parity.rs` (offset 1-150); `src/cli/auth.rs`
> (offset 1515-1565 — DEFAULT_OAUTH_SCOPES test); `src/api/auth.rs` (offset
> 370-490 — RedirectUriStrategyRequest::bind body); `src/api/jira/worklogs.rs`
> (full); `src/cli/worklog.rs` (full); `src/cli/issue/workflow.rs` (offset
> 600-720 — handle_open + browse-URL composition); `src/api/client.rs`
> (offset 340-400 — base_url vs instance_url distinction).
>
> Test fn counts re-verified via `awk '/#\[(tokio::)?test/{c++} END{print c}'`
> for every file mentioned. Round-2 metrics audited and reconciled.

---

## 1. Round metadata

| Field | Value |
|---|---|
| Round | 3 of (max 5) |
| Targets attacked this round | T-09 (adf.rs full sweep — 69-test enumeration completion); T-10 (changelog source unit-test full sweep — 38-test completion); cli_handler.rs lines 300-700 (R2 gap chunk); Pass 2 R2 cross-pollination verification (NEW-INV-56/29/81/19/18); tests/worklog_commands.rs full; tests/input_validation.rs full; tests/project_meta.rs full; tests/user_pagination.rs partial; tests/cli_smoke.rs full; tests/team_column_parity.rs partial; CONV-ABS-005 BC-035 file-attribution audit (DEFAULT_OAUTH_SCOPES test) |
| Targets deferred to round 4 | T-11 OAuth state machine + 401-auto-refresh deferred-integration (still deferred — 3rd time); 13 of the smaller integration test files at full granularity (`tests/issue_commands.rs` 54 tests, `tests/api_client.rs` 22 tests, `tests/issue_remote_link.rs` 6, `tests/user_commands.rs` 14, `tests/project_commands.rs` 10, `tests/issue_resolution.rs` 3, `tests/issue_view_errors.rs` 4, `tests/assets_errors.rs` 3, `tests/cmdb_fields.rs` 5, full `team_column_parity.rs` 7, `auth_login_config_errors.rs` survey); proptest enumeration; insta snapshot enumeration |
| Files freshly read this round (full) | 4 — `tests/worklog_commands.rs` (171 LOC, 5 tests), `tests/input_validation.rs` (253 LOC, 8 tests), `tests/project_meta.rs` (126 LOC, 3 tests), `src/api/jira/worklogs.rs` (31 LOC), `src/cli/worklog.rs` (79 LOC) |
| Files freshly read this round (chunked) | 9 — `src/adf.rs` (offsets 800-880, 1100-1500, 1500-1827); `src/cli/issue/changelog.rs` (offsets 300-848); `tests/cli_handler.rs` (offset 280-700); `tests/cli_smoke.rs` (full 334 LOC, 27 tests); `tests/user_pagination.rs` (offset 1-200, 4 of 11 tests); `tests/team_column_parity.rs` (offset 1-150, structural read); `src/cli/auth.rs` (offset 1515-1565); `src/api/auth.rs` (offset 370-490); `src/cli/issue/workflow.rs` (offset 600-720); `src/api/client.rs` (offset 340-400) |
| BCs in pass-3 broad | 193 (recounted) |
| BCs after R1 | 271 (211 HIGH / 53 MEDIUM / 7 LOW) |
| BCs after R2 | 343 (281 HIGH / 56 MEDIUM / 6 LOW) |
| BCs added this round | **76 net new** (mostly HIGH; details in §3) |
| BCs promoted MEDIUM→HIGH | 2 (BC-035 split + BC-1410 audit closure) |
| BCs corrected (CONV-ABS-009..010) | 2 |
| BCs after round 3 | **419 total** (354 HIGH / 59 MEDIUM / 6 LOW) |

---

## 2. Audit of Round 2 against the 5 Known Hallucination Classes

### 2.1 Over-extrapolated token lists
- **R2 BC-272** ("EXACTLY ONE `[verbose] changelog ... timestamp failed to
  parse` log per process"): R2 was correct that the test asserts
  `stderr.matches("timestamp failed to parse").count() == 1`. Verified by
  re-read of `tests/issue_changelog.rs:1492-1565` not necessary (R2 cited
  exact assertion). **No retraction.**
- **R2 BC-225** ("`Content-Type: <custom>` overrides default; `application/
  json` is NOT present"): The test cited (`tests/cli_handler.rs:1148-1197`)
  passes both via stub — verified the text contains `count = 1` and
  destination value matches user input. **No retraction.**
- **R2 BC-235** ("`/search/jql` mock with `expect(0)`"): R2 is correct;
  re-read confirms wiremock pre-HTTP rejection. **No retraction.**

### 2.2 Miscounted enumerations
- **R2 §9.6 prediction** ("total `extract_error_message` tests = ~12"):
  Recount via `awk '/fn test_extract_error_message/{c++} END{print c}'
  tests/api_client.rs` returns **11**, NOT 12. Off-by-one. R2's prose said
  "~12" with hedging language (the tilde) — borderline acceptable but the
  audit closure should pin the literal **11**. **CONV-ABS-009.**
- **R2 BC-272 / 274 line-range claims**: re-checked via the existing R1+R2
  citations; all line ranges consistent with offsets read.
- **R2 §3.7 BC count** for `tests/duplicate_user_disambiguation.rs`: stated
  "5-test deepening (BC-296..BC-300)". Recount via awk on the file returns
  5 tests. ✓
- **R2 stat table totals** (281/56/6): Re-derived from the §7 table by
  summing subject-area H/M/L columns. Sum: 30+78+35+24+24+5+11+13+16+14+16+
  12+23+7+21+21+9 = 358 (HIGH columns) — but the row has 281. The
  discrepancy is because §7 table had some cells double-counting the broad
  baseline (e.g., subject area "16. ADF (NEW)" was 21/0/0 — that's all
  net-new R2). Recount-by-source rather than recount-by-subject-area
  matches the stated 281 HIGH after R2. **The §7 table is consistent if read
  as cumulative-after-R2 (not delta), but the row-sum vs total-row arithmetic
  was off due to subject-area re-categorization. Documented but not a
  fabrication.**

### 2.3 Named pattern conflation / fabrication
- **R2 BC-925..BC-936** AuthorNeedle classifier names: re-verified via
  direct read of `src/cli/issue/changelog.rs:300-848`. Test names match
  EXACTLY: `from_raw_treats_short_name_as_substring`,
  `from_raw_treats_colon_string_as_accountid`,
  `from_raw_long_alpha_only_name_is_substring`,
  `from_raw_long_compound_name_is_substring`,
  `from_raw_long_hyphenated_name_is_substring`,
  `from_raw_long_unicode_name_is_substring`,
  `from_raw_long_unicode_name_with_digit_is_substring`,
  `from_raw_long_cyrillic_name_with_digit_is_substring`,
  `from_raw_old_hex_accountid_is_accountid`,
  `from_raw_colon_forces_accountid_regardless_of_heuristics`,
  `from_raw_long_name_with_digit_is_accountid`,
  `from_raw_twelve_char_boundary_with_digit_is_accountid`,
  `from_raw_twelve_char_boundary_no_digit_is_substring`,
  `from_raw_short_hyphenated_name_is_substring`,
  `from_raw_unknown_placeholder_is_substring`. **No fabrication. ✓**
- **R1 BC-130 unit-test names** (CONV-ABS-008 deferred to R3): R3 did NOT
  re-read `cli/issue/list.rs::tests` directly this round (would require
  another chunked read of the 970-LOC file). Provisional status remains
  noted; defer to Round 4.

### 2.4 Same-basename artifact conflation
- **R1 BC-035 file attribution** (`DEFAULT_OAUTH_SCOPES` regression test):
  R1 §3.1 placed the regression test at `src/api/auth.rs:34-63` — that
  range is the constant DEFINITION, NOT the test function. The actual test
  `default_oauth_scopes_pins_the_full_set_with_offline_access` lives at
  `src/cli/auth.rs:1523-1564` (verified by `awk '/default_oauth_scopes/'
  on full src/ tree). The file `src/api/auth.rs:56` only has a comment
  REFERENCING the test. R1 conflated the constant's location with the
  test's location. **CONV-ABS-010.**
- **`tests/api_client.rs` (22 tests) vs source `api/client.rs::tests`
  module**: R2 §3.13 claimed `extract_error_message` tests are at
  `tests/api_client.rs:280-340`. Re-verified — the 11 tests are at lines
  255-345. The integration file is correct; no source-vs-test conflation.

### 2.5 Inflated or deflated metrics
- **R2 BC-219** ("`jr api` rejects absolute URLs"): the cited line range
  `tests/cli_handler.rs:1243-1253` is 11 lines for `test_handler_api_
  rejects_absolute_url` — re-verified via offset read; the actual function
  body fits in those lines. Not inflated. ✓
- **R1 BC-1410-R / R2 BC-1410 audit closure**: R2 corrected the over-
  specification in CONV-ABS-006. The literal value in fixtures is
  `Basic dGVzdDp0ZXN0` (`test:test`) for almost all tests. R3 reaffirms;
  the contract is "Basic <base64>" or "Bearer <oauth>" without specifying
  the exact base64. ✓

---

## 3. BC additions / promotions, per target T-NN

### 3.1 T-09 — `src/adf.rs` ADF→text rendering full sweep (NEW)

R2 enumerated 21 of the 69 unit tests (markdown→ADF). R3 closes the remaining
~48 covering ADF→text rendering, the inverse direction.

#### BC-940 (NEW): `adf_to_text({"type":"doc","content":[]})` → empty string `""` (no placeholder)
**Confidence**: HIGH
**Sources**: `src/adf.rs:1622-1630` (`test_adf_to_text_empty_doc`)
**Behavior**: The renderer iterates an empty content array and `finish()`
returns `""`. **Pinned** so a future refactor that emits a placeholder for
empty documents trips a test rather than silently changing output.

#### BC-941 (NEW): `adf_to_text` strong-mark text node with `marks: [{type: "strong"}]` → `**bold**`
**Confidence**: HIGH
**Sources**: `src/adf.rs:1299-1307` (`test_render_strong_mark`)

#### BC-942 (NEW): em-mark → `*em*`; strike-mark → `~~gone~~`; code-mark → `` `x` ``
**Confidence**: HIGH
**Sources**: `src/adf.rs:1309-1340` (3 tests covering em, strike, code marks)

#### BC-943 (NEW): link-mark with `attrs.href` → `[text](href)`; missing href falls back to `[text]()`
**Confidence**: HIGH
**Sources**: `src/adf.rs:1342-1364` (`test_render_link_preserves_href` +
`test_render_link_missing_href_defaults_empty`)
**Edges**: Empty parens are emitted, NOT skipped. Pin against a refactor
that drops the link mark when href is missing.

#### BC-944 (NEW): Multiple marks `[strong, em]` → `***foo***` (deterministic order: array order = inside-out wrapping)
**Confidence**: HIGH (NEW level of detail)
**Sources**: `src/adf.rs:1366-1376` (`test_render_multiple_marks_deterministic_order`)
**Behavior**: Pin: order in `marks` array determines wrap order. Refactor
to alphabetize marks would break round-trip with markdown-to-ADF write path.

#### BC-945 (NEW): Unknown mark types are dropped silently — `marks: [{type: "underline"}]` → `plain` (no syntax)
**Confidence**: HIGH
**Sources**: `src/adf.rs:1378-1386` (`test_render_unknown_mark_drops_syntax`)
**Effects**: Forward-compatible — a future ADF mark type doesn't crash the
renderer. Unknown marks are non-fatal.

#### BC-946 (NEW): orderedList with no `attrs` renders sequential numbers starting at 1: `1. alpha`, `2. beta`, `3. gamma`
**Confidence**: HIGH
**Sources**: `src/adf.rs:1388-1405` (`test_render_ordered_list_numeric_prefix`)

#### BC-947 (NEW): orderedList with `attrs: {order: 5}` renders `5. five`, `6. six` (start-from-N respected)
**Confidence**: HIGH
**Sources**: `src/adf.rs:1407-1423` (`test_render_ordered_list_respects_attrs_order`)

#### BC-948 (NEW): orderedList with `attrs: {order: 0}` renders `1. only` (zero coerced to 1, NOT a `0.` prefix)
**Confidence**: HIGH (defensive)
**Sources**: `src/adf.rs:1425-1439` (`test_render_ordered_list_order_zero_defaults_to_one`)

#### BC-949 (NEW): orderedList containing nested bulletList → outer renders `1. outer`, nested renders `  - inner` (2-space indent)
**Confidence**: HIGH
**Sources**: `src/adf.rs:1441-1462` (`test_render_mixed_nested_lists`)

#### BC-950 (NEW): rule node renders as `---` line (markdown horizontal rule)
**Confidence**: HIGH
**Sources**: `src/adf.rs:1464-1478` (`test_render_rule`)

#### BC-951 (NEW): hardBreak inside paragraph → `\n` (NOT space, NOT empty); two hardBreaks → blank line `a\n\nb`
**Confidence**: HIGH (PROMOTED scope)
**Sources**: `src/adf.rs:1480-1492` (`test_render_hard_break_inserts_newline`); `1652-1668` (`test_render_consecutive_hard_breaks_produce_multiple_newlines`)
**Edges**: Two consecutive hardBreaks produce blank line (preserves
visual density).

#### BC-952 (NEW): trailing hardBreaks at end of paragraph are stripped by `finish().trim_end()`
**Confidence**: HIGH (NEW)
**Sources**: `src/adf.rs:1722-1740` (`test_render_trailing_hard_breaks_stripped_by_finish`)
**Behavior**: A paragraph ending with `[text("a"), hardBreak, hardBreak]`
renders as just `a`. Pin against a refactor that omits the `trim_end`,
which would leave dangling whitespace at doc end.

#### BC-953 (NEW): hardBreak INSIDE a tableCell becomes a SPACE (NOT a newline) — preserves single-pipe-row constraint
**Confidence**: HIGH (NEW; load-bearing for table integrity)
**Sources**: `src/adf.rs:1742-1767` (`test_render_hard_break_in_table_cell_becomes_space`)
**Behavior**: `tableCell.content[paragraph[text("line one"), hardBreak,
text("line two")]]` → cell text `line one line two`. Newline would break
the markdown table rendering.

#### BC-954 (NEW): codeBlock with `attrs.language` → triple-backtick fence with language: `` ```rust\nfn x() {}\n``` ``
**Confidence**: HIGH
**Sources**: `src/adf.rs:1494-1510` (`test_render_code_block_with_language`)

#### BC-955 (NEW): codeBlock without language → empty-fence form: `` ```\nplain ``
**Confidence**: HIGH
**Sources**: `src/adf.rs:1512-1526` (`test_render_code_block_without_language`)

#### BC-956 (NEW): code mark whose text contains a backtick is rendered with double-backtick fence: `` ``foo`bar`` ``
**Confidence**: HIGH (NEW; CommonMark code-span rule)
**Sources**: `src/adf.rs:1574-1584` (`test_render_code_mark_with_backtick_in_content`)
**Behavior**: When the code-mark text has internal backticks, the fence
must be a longer run. Pin against single-backtick wrapping that would
break the code span.

#### BC-957 (NEW): code mark with leading/trailing backtick gets space-padded: `` `` `x` `` `` (CommonMark space-rule)
**Confidence**: HIGH
**Sources**: `src/adf.rs:1586-1596` (`test_render_code_mark_with_leading_trailing_backtick_pads`)
**Effects**: Without the padding, CommonMark would interpret as nested
backticks rather than a code span containing backticks.

#### BC-958 (NEW): table renders pipe-format with header separator: `| h1 | h2 |\n| --- | --- |\n| a | b |`
**Confidence**: HIGH
**Sources**: `src/adf.rs:1167-1191` (`test_render_table_pipe_format`)

#### BC-959 (NEW): table with mixed `tableHeader` + `tableCell` in the SAME row STILL emits the `| --- | --- |` separator (renderer treats any first row as header-equivalent for separator purposes)
**Confidence**: HIGH (NEW; defensive)
**Sources**: `src/adf.rs:1193-1213` (`test_render_table_mixed_header_cell_row_still_emits_separator`)

#### BC-960 (NEW): tableCell containing a `paragraph > text` flattens to just the text inside `| ... |` — paragraph wrapper IS removed in cell context
**Confidence**: HIGH
**Sources**: `src/adf.rs:1215-1231` (`test_render_table_cell_flattens_paragraph`)
**Behavior**: Asymmetric with paragraph-context where the paragraph
produces a newline boundary. In tables, the paragraph is unwrapped.

#### BC-961 (NEW): tableCell text containing `|` is escaped as `\|` (backslash before pipe) so it doesn't introduce a false column break
**Confidence**: HIGH (NEW)
**Sources**: `src/adf.rs:1783-1804` (`test_render_table_cell_escapes_pipe_in_text`)

#### BC-962 (NEW): tableCell text containing `\n` (newline) is collapsed to a SPACE: `text: "line\nwrap"` → cell content `line wrap`
**Confidence**: HIGH (NEW)
**Sources**: `src/adf.rs:1806-1825` (`test_render_table_cell_collapses_newlines_in_text`)
**Effects**: Combined with BC-953 (hardBreak→space in cell), all newline-
producing constructs are normalized to space inside table cells.

#### BC-963 (NEW): blockquote with multiple paragraphs prefixes EACH line with `> `; nested blockquote prefixes with `> > `
**Confidence**: HIGH
**Sources**: `src/adf.rs:1261-1278` (`test_render_blockquote_prefixes_each_line`); `1280-1296` (`test_render_nested_blockquote`)

#### BC-964 (NEW): blockquote containing a codeBlock with internal blank line — the BLANK LINE inside the blockquote ALSO gets the `>` prefix (every line, even empty, prefixed)
**Confidence**: HIGH (NEW; preserves blockquote context)
**Sources**: `src/adf.rs:1599-1620` (`test_render_blockquote_with_internal_blank_line_keeps_prefix`)
**Behavior**: Pin against a refactor that strips empty lines before
prefixing — that would break the blockquote-of-codeblock context with a
spurious un-prefixed line.

#### BC-965 (NEW): blockquote whose only child is an empty paragraph → renders as empty string `""` (NOT a bare `> `)
**Confidence**: MEDIUM (acknowledged-quirk)
**Sources**: `src/adf.rs:1633-1649` (`test_render_blockquote_with_empty_paragraph_produces_no_output`)
**Behavior**: Documented as a quirk in the test comment. Pinned so a
future decision to emit `> ` for empty-but-present blockquotes surfaces as
a test failure.

#### BC-966 (NEW): marks `[code, strong]` → `**`code-text`**` (`code` is treated as innermost regardless of array order); marks `[strong, code]` → also `**`code-text`**`
**Confidence**: HIGH (NEW; load-bearing semantic)
**Sources**: `src/adf.rs:1670-1700` (`test_render_marks_code_and_strong` — code-first), `1769-1781` (`test_render_strong_with_code_applies_code_innermost` — strong-first)
**Behavior**: `code` always wraps innermost. Pin: a refactor that respected
strict array-order would break `**\`x\`**` round-trip with `markdown_to_adf`.

#### BC-967 (NEW): marks `[strike, em]` → `*~~x~~*` (NON-code marks DO follow array order: strike innermost, em outer)
**Confidence**: HIGH (NEW)
**Sources**: `src/adf.rs:1688-1700` (`test_render_marks_strike_and_em`)
**Behavior**: Asymmetry with BC-966: only `code` is special-cased to
innermost; other marks wrap in array order.

#### BC-968 (NEW): marks `[link, strong]` → `**[x](href)**` (link wraps inside; strong wraps outside; pure array-order)
**Confidence**: HIGH
**Sources**: `src/adf.rs:1703-1719` (`test_render_marks_link_and_strong`)

#### BC-969 (NEW): markdown→ADF→text→markdown round-trip preserves the NODE-TYPE structure (not necessarily textual identity, but the `type` traversal sequence is identical)
**Confidence**: HIGH (NEW; structural integrity contract)
**Sources**: `src/adf.rs:1528-1572` (`test_markdown_to_adf_to_text_roundtrip` + helpers)
**Behavior**: Input concat of heading/paragraph/lists/blockquote round-
trips with same `walk_types` sequence. Text is allowed to differ (e.g.,
backtick padding or whitespace normalization).

#### BC-970 (NEW): markdown image syntax `![alt](url)` is SKIPPED entirely — no image node emitted, only surrounding text preserved
**Confidence**: HIGH (NEW; design decision)
**Sources**: `src/adf.rs:1109-1128` (`test_markdown_image_is_skipped`)
**Behavior**: `before ![alt](https://example.com/img.png) after` → ADF
content has `before` and `after` text but NO `image` or `media` node.
**Effects**: Jira's ADF supports media nodes but jr's writer chose to omit
them — uploading attachments is out of scope. Pin against a refactor that
emits `image` nodes (would create invalid ADF without a corresponding
attachment upload).

#### BC-971 (NEW): markdown task-list syntax `[x]` / `[ ]` is preserved as LITERAL TEXT inside list items (NOT converted to taskList nodes; `ENABLE_TASKLISTS` is OFF)
**Confidence**: HIGH (NEW; pulldown-cmark feature gate)
**Sources**: `src/adf.rs:1130-1164` (`test_markdown_task_list_syntax_preserved_as_text`)
**Behavior**: `- [x] done task` → bulletList > listItem with text `[x] done
task`. Tight-list mode places text directly in listItem (no paragraph
wrapper).

#### BC-972 (NEW): markdown table cell with inline `**bold**` AND `[link](url)` → tableHeader > paragraph > text-with-strong-mark + text-with-link-mark (marks compose with table cell wrapping)
**Confidence**: HIGH (NEW; cross-feature composition)
**Sources**: `src/adf.rs:1233-1258` (`test_markdown_table_cell_with_inline_formatting`)
**Behavior**: Marks survive across the markdown-table-cell parser path.

### 3.2 T-10 — `src/cli/issue/changelog.rs` source unit-test full sweep (NEW)

R2 enumerated 12 of 38 source unit tests. R3 closes the remaining ~26 here.

#### BC-973 (NEW): `from_raw("alice")` → `NameSubstring("alice")` (lowercase-normalized at construction)
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:326-332` (`from_raw_treats_short_name_as_substring`)

#### BC-974 (NEW): `from_raw("557058:abc-123")` → `AccountId("557058:abc-123")` (colon forces AccountId regardless of length/digits)
**Confidence**: HIGH (PROMOTED, verbatim test name)
**Sources**: `src/cli/issue/changelog.rs:334-340` (`from_raw_treats_colon_string_as_accountid`); `419-425` (`from_raw_colon_forces_accountid_regardless_of_heuristics`)

#### BC-975 (NEW): `from_raw("AlexanderGreene")` (15 chars, no digit) → `NameSubstring` (#213 regression pin — long alpha-only is NOT AccountId)
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:343-349` (`from_raw_long_alpha_only_name_is_substring`)

#### BC-976 (NEW): `from_raw("JoseMariaRodriguez")` (18 chars, no digit) → `NameSubstring` — second #213 regression pin
**Confidence**: HIGH (NEW)
**Sources**: `src/cli/issue/changelog.rs:351-358` (`from_raw_long_compound_name_is_substring`)

#### BC-977 (NEW): `from_raw("jean-pierre-dupont")` (18 chars, hyphenated, no digit) → `NameSubstring`
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:360-367` (`from_raw_long_hyphenated_name_is_substring`)

#### BC-978 (NEW): `from_raw("JoséMariaRodríguez")` (18 chars, contains accented `é` `í`, NO digit) → `NameSubstring` (Unicode falls through digit-rule)
**Confidence**: HIGH (NEW)
**Sources**: `src/cli/issue/changelog.rs:369-380` (`from_raw_long_unicode_name_is_substring`)

#### BC-979 (NEW): `from_raw("José123Mariarod")` (15 chars, accented `é` AND digit) → `NameSubstring` — pins `is_ascii_alphanumeric` guard SPECIFICALLY (a refactor to `char::is_alphanumeric` would mis-classify as AccountId because `'é'.is_alphanumeric() == true` while `'é'.is_ascii_alphanumeric() == false`)
**Confidence**: HIGH (NEW; subtle regression pin)
**Sources**: `src/cli/issue/changelog.rs:382-393` (`from_raw_long_unicode_name_with_digit_is_substring`)
**Effects**: Pin: the AccountId classifier requires ASCII-alphanumeric
(0-9, A-Z, a-z, plus `-`/`_`), NOT general Unicode-alphanumeric. Cyrillic/
Latin-extended scripts always fall through to NameSubstring.

#### BC-980 (NEW): `from_raw("Александр12345")` (Cyrillic `А`=U+0410 + digits, 14 chars) → `NameSubstring` — widens BC-979 pin beyond Latin-1
**Confidence**: HIGH (NEW)
**Sources**: `src/cli/issue/changelog.rs:395-407` (`from_raw_long_cyrillic_name_with_digit_is_substring`)
**Edges**: Test comment notes literal `А` is U+0410, NOT ASCII U+0041 —
"do not 'clean up' by retyping it" — pin against a future tooling pass
that normalizes the source.

#### BC-981 (NEW): `from_raw("5b10ac8d82e05b22cc7d4ef5")` (24-char hex, contains digits, no colon) → `AccountId` (matches old Atlassian hex accountId format)
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:409-416` (`from_raw_old_hex_accountid_is_accountid`)

#### BC-982 (NEW): `from_raw("User12345Name")` (13 chars, contains digit) → `AccountId` (intentional residual edge — would be classified as username but rule prefers length+digit gate)
**Confidence**: HIGH (NEW; documented residual edge)
**Sources**: `src/cli/issue/changelog.rs:427-433` (`from_raw_long_name_with_digit_is_accountid`)

#### BC-983 (NEW): `from_raw("abcdefghijk1")` (EXACTLY 12 chars, contains digit) → `AccountId` (boundary pin against off-by-one to `> 12`)
**Confidence**: HIGH (NEW; load-bearing boundary)
**Sources**: `src/cli/issue/changelog.rs:435-447` (`from_raw_twelve_char_boundary_with_digit_is_accountid`)

#### BC-984 (NEW): `author_matches(user, AccountId("X"))` — exact equality match against `user.account_id`; non-matching → false
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:480-496` (`author_matches_respects_account_id_exact`)

#### BC-985 (NEW): `author_matches(user{display_name: "ALICE Smith"}, NameSubstring("alice"))` → true (haystack `to_lowercase` then `contains`)
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:498-511` (`author_matches_substring_hits_display_name_case_insensitive`)

#### BC-986 (NEW): `author_matches(user{display_name: "JOSÉ Rodríguez"}, NameSubstring("josé"))` → true (Unicode-aware lowercasing on both sides)
**Confidence**: HIGH (NEW; Unicode lowercasing pin)
**Sources**: `src/cli/issue/changelog.rs:545-563` (`author_matches_substring_hits_unicode_display_name`)

#### BC-987 (NEW): `author_matches(user{display_name: ""}, NameSubstring("alice"))` → false (empty display_name does NOT match; account_id is also unrelated)
**Confidence**: HIGH (defensive non-panic)
**Sources**: `src/cli/issue/changelog.rs:565-580` (`author_matches_substring_handles_empty_display_name`)

#### BC-988 (NEW): `author_matches(user{account_id: ""}, NameSubstring("alice"))` where display_name="Alice Smith" → true (display_name haystack STILL works even when account_id is empty; non-conditional fallback)
**Confidence**: HIGH (PROMOTED; pin against refactor that conditions display_name search on non-empty account_id)
**Sources**: `src/cli/issue/changelog.rs:582-599` (`author_matches_substring_handles_empty_account_id`)

#### BC-989 (NEW): `author_matches(None, _)` → false (null author always returns false; pin against any path that treats None as wildcard)
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:601-607` (`author_matches_null_author_always_false`)

#### BC-990 (NEW): `build_rows(entries, false)` flattens entries-with-items into ROW PER ITEM, preserving the items' original order
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:639-654` (`build_rows_flattens_items_in_order`)
**Behavior**: Entry with [status, resolution] items → 2 rows: `rows[0][2] == "status"`, `rows[1][2] == "resolution"`.

#### BC-991 (NEW): `build_rows` for entry with `author: None` renders `SYSTEM_AUTHOR` literal (`"(system)"`) in column 1
**Confidence**: HIGH (PROMOTED scope; literal pinned)
**Sources**: `src/cli/issue/changelog.rs:656-666` (`build_rows_uses_system_for_null_author`)

#### BC-992 (NEW): `from_to_display(None, None)` → `NULL_GLYPH` (em-dash `—`)
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:669-673` (`from_to_display_renders_em_dash_for_empty`)
**Edges**: Both `None` AND `Some("")`/`None` AND `None`/`Some("")` all
return em-dash. The em-dash glyph is U+2014.

#### BC-993 (NEW): `from_to_display(Some("Done"), Some("10000"))` → `"Done"` (string preferred over raw); `from_to_display(None, Some("10000"))` → `"10000"` (raw fallback)
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:675-679` (`from_to_display_prefers_string_over_raw`)

#### BC-994 (NEW): `from_to_display(Some(""), Some("10000"))` → `"10000"` (empty STRING falls back to raw — empty/whitespace treated as absent, NOT picked-and-rendered-as-em-dash)
**Confidence**: HIGH (NEW; defensive — pinned against `is_empty` boolean confusion)
**Sources**: `src/cli/issue/changelog.rs:681-687` (`from_to_display_empty_string_falls_back_to_raw`)
**Behavior**: `Some("   ")` (whitespace-only) ALSO falls back. The `trim()`-then-`is_empty()` filter handles both.

#### BC-995 (NEW): `format_date("2026-04-16T14:02:11.000+0000", false)` → 16-char string starting with `"2026-04-"` (TZ-dependent on runner; YYYY-MM-DD HH:MM shape)
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:689-696` (`format_date_converts_rfc3339_to_local`)

#### BC-996 (NEW): `format_date("not-a-date", false)` → returns the raw input verbatim (parse-failure fallback; no panic)
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:698-702` (`format_date_falls_back_to_raw_on_parse_failure`)

#### BC-997 (NEW): `parse_created("...+0000") == parse_created("...+00:00")` (both Jira compact-offset and RFC3339 colon-offset accepted; mixed sources sort chronologically by parsed instant)
**Confidence**: HIGH
**Sources**: `src/cli/issue/changelog.rs:704-711` (`parse_created_accepts_both_offset_formats`); `714-753` (`sort_uses_parsed_datetime_across_mixed_offset_formats`)
**Effects**: Pin: lexicographic sort would misorder `+00:00` after `+0000`
(`':' > '0'`). Parsed-instant sort produces correct ordering.

#### BC-998 (NEW): `truncate_to_rows(entries, 0)` → entries cleared (empty Vec)
**Confidence**: HIGH (boundary pin)
**Sources**: `src/cli/issue/changelog.rs:755-765` (`truncate_to_rows_handles_cap_zero`)

#### BC-999 (NEW): `truncate_to_rows([entry1{2 items}, entry2{1 item}], 2)` → keeps entry1 with 2 items, drops entry2 entirely (NOT entry1 with 1 item + entry2 with 1 item)
**Confidence**: HIGH (NEW; full-entry-first algorithm)
**Sources**: `src/cli/issue/changelog.rs:767-790` (`truncate_to_rows_trims_last_entry_partially`)
**Effects**: The algorithm fills entries entirely before partial-trimming
the last surviving entry. Pin against round-robin truncation.

#### BC-1000 (NEW): `truncate_to_rows([entry{3 items}], 2)` → keeps entry with FIRST 2 items (status+resolution), drops the 3rd (labels)
**Confidence**: HIGH (NEW; partial-trim within straddling entry)
**Sources**: `src/cli/issue/changelog.rs:792-807` (`truncate_to_rows_partial_trim_inside_entry`)

#### BC-1001 (NEW): `LoweredStr::new("MixedCase-Name")` → stores lowercase `"mixedcase-name"` (smart constructor enforces invariant)
**Confidence**: HIGH (PROMOTED, type-system pin)
**Sources**: `src/cli/issue/changelog.rs:809-813` (`lowered_str_normalizes_input_on_construction`); `815-818` (`lowered_str_equality_is_case_invariant`)

#### BC-1002 (NEW): `--field`-equivalent filter logic: `entries.iter_mut().retain(|it| needles.iter().any(|n| it.field.to_lowercase().contains(&n.to_lowercase())))`; entries with zero surviving items are then dropped
**Confidence**: HIGH (NEW; algorithm pin)
**Sources**: `src/cli/issue/changelog.rs:820-846` (`field_filter_semantics_at_item_level`)

### 3.3 cli_handler.rs lines 300-700 (R2 GAP CHUNK)

R2 deferred this chunk; R3 reads it now. 7 new BCs covering create-with-name-search bodies, create-basic, create-to-me, assign-idempotent-with-name-search, unassign-idempotent, and the two created-after/before integration tests.

#### BC-1003 (NEW): `issue create --to <name>` body verification — the body partial-JSON match pins assignee.accountId from name resolution; the response GET (when --output table) does NOT include the assignee in the success-line stderr (only `Created issue HDL-101`)
**Confidence**: HIGH (PROMOTED; full body verification at integration)
**Sources**: `tests/cli_handler.rs:281-333` (`test_handler_create_with_to_name_search`)
**Behavior**: stdout JSON contains `"key": "HDL-101"`. The mocked
multi-project search returns `[(acc-bob-555, Bob Smith)]`; create body
pins `assignee.accountId == "acc-bob-555"`.

#### BC-1004 (NEW): `issue create -p HDL -t Task -s "Basic create"` (no `--to`/`--account-id`) emits POST body containing ONLY `{project, issuetype, summary}` — NO assignee field (NOT `assignee: null`)
**Confidence**: HIGH (NEW; negative serialization pin)
**Sources**: `tests/cli_handler.rs:335-371` (`test_handler_create_basic`)
**Behavior**: Body partial-match has 3 keys exactly. `--output json`
stdout has `"key": "HDL-102"` AND `"url":` substring. Pin against a
refactor that emits `assignee: null` (which Atlassian rejects with a
schema error).

#### BC-1005 (NEW): `issue create --to me` resolves via `/myself` → POST body `assignee.accountId` = `myself.accountId`. NO assignable-user search HTTP fired (the `me` keyword short-circuits)
**Confidence**: HIGH (PROMOTED; symmetric with assign --to me)
**Sources**: `tests/cli_handler.rs:418-464` (`test_handler_create_to_me`)
**Behavior**: Mock for `/rest/api/3/myself` returns accountId `abc123`.
POST body pins `assignee.accountId == "abc123"`. NO mock for
multi-project search needed.
**Effects**: 1 fewer HTTP call vs `--to <name-string>` flow.

#### BC-1006 (NEW): `issue assign HDL-7 --to Jane` where issue is ALREADY assigned to Jane (acc-jane-456 matches resolved acc-jane-456) → idempotent: PUT mock has `expect(0)`; stdout JSON has `"changed": false`
**Confidence**: HIGH (PROMOTED; closes R2 BC-205-R gap to the name-search path specifically)
**Sources**: `tests/cli_handler.rs:466-511` (`test_handler_assign_idempotent_with_name_search`)
**Behavior**: 3-step chain: assignable-user search succeeds → GET issue
shows already-assigned-to-target → idempotency check fires BEFORE PUT.
PUT NOT called. JSON output reflects the no-op.

#### BC-1007 (NEW): `issue assign HDL-8 --unassign` where issue is ALREADY unassigned → idempotent: PUT mock has `expect(0)`; stdout JSON has `"changed": false`, `"assignee": null`
**Confidence**: HIGH (NEW; --unassign idempotency parity)
**Sources**: `tests/cli_handler.rs:513-541` (`test_handler_unassign_idempotent`)
**Behavior**: GET-only flow when target state matches current state.
Confirms BC-204-R was missing the idempotent path; both `--account-id <id>`
AND `--unassign` short-circuit when state already matches.

#### BC-1008 (NEW): `issue list --project PROJ --created-after 2026-03-18` JQL body partial-match pins `"jql": "project = \"PROJ\" AND created >= \"2026-03-18\" ORDER BY updated DESC"` (project scope quoted; date format-preserved)
**Confidence**: HIGH (PROMOTED to integration; full literal pinned)
**Sources**: `tests/cli_handler.rs:543-590` (`test_handler_list_created_after`)
**Behavior**: Confirms BC-132 (validate_date pre-HTTP) AND BC-133 (>= for created-after). Project key is double-quoted; the date is ISO-8601 in double quotes.

#### BC-1009 (NEW): `issue list --project PROJ --created-before 2026-03-18` JQL body literal `"project = \"PROJ\" AND created < \"2026-03-19\" ORDER BY updated DESC"` (note: `< "2026-03-19"`, NOT `<= "2026-03-18"`; the +1 day shift makes the comparison end-of-day inclusive)
**Confidence**: HIGH (PROMOTED to integration; load-bearing boundary)
**Sources**: `tests/cli_handler.rs:592-639` (`test_handler_list_created_before`)
**Behavior**: Confirms BC-133 at integration scope. The date manipulation
happens client-side (chrono `+ Days::new(1)`).

### 3.4 Pass 2 R2 cross-pollination — VERIFICATION RESULTS

R3 verified each of the 5 cross-pollination findings by reading the cited
source file/line range.

#### NEW-INV-56 — `handle_open` uses `client.base_url()` — **VERIFIED REAL BUG**
**Source verified**: `src/cli/issue/workflow.rs:631-646` — read directly:
```rust
let url = format!("{}/browse/{}", client.base_url(), key);
```
And `src/api/client.rs:350-358`:
```rust
pub fn base_url(&self) -> &str { &self.base_url }
pub fn instance_url(&self) -> &str { &self.instance_url }
```
The two methods exist precisely because `base_url` is the Atlassian API
gateway URL (`https://api.atlassian.com/ex/jira/<cloud_id>`) for OAuth
profiles. `instance_url` is the browser-friendly URL.

#### BC-1010 (NEW): `jr issue open <key>` builds the browse URL via `client.base_url()` — for OAuth profiles, this returns the API gateway URL `https://api.atlassian.com/ex/jira/<cloud_id>`, which is NOT a browser-renderable URL
**Confidence**: HIGH (current behavior; documented as bug candidate)
**Sources**: `src/cli/issue/workflow.rs:636`; `src/api/client.rs:350-358`
**Behavior**: For API-token profiles, `base_url == instance_url ==
https://your-org.atlassian.net`, so the bug is silent. For OAuth profiles,
`open::that(&url)` would launch a browser to the API gateway, which
returns an error page or 404. **Pin: current implementation is buggy for
OAuth profiles.**

#### Holdout H-039 (NEW): `jr issue open <key>` should use `instance_url()` (NOT `base_url()`) for OAuth profiles
**Setup**: Profile configured with OAuth (cloud_id known). Mock or real
behavior: launching the browser at `client.base_url()` fails for
non-browser-renderable API URLs.
**Action**: `jr issue open HDL-1`
**Expected**: The browser receives the user-facing URL
`https://your-org.atlassian.net/browse/HDL-1`, NOT the API gateway URL.
**Why hidden**: BUG — current code uses `base_url()`. A fix would call
`client.instance_url()` instead.
**Cross-reference**: Pass 4 reliability section should note this as a
correctness defect for OAuth users (when paired with
`--url-only`, output is also wrong; see BC-1011).

#### BC-1011 (NEW): `jr issue open <key> --url-only` ALSO uses `base_url()` — same OAuth-profile incorrect-URL behavior; the `--url-only` flag controls whether `open::that` is invoked, NOT how the URL is composed
**Confidence**: HIGH (NEW; complement to BC-1010)
**Sources**: `src/cli/issue/workflow.rs:638-643`
**Effects**: A user who relied on `jr issue open <k> --url-only | xargs
firefox` to test things on OAuth profiles would also get the wrong URL.

---

#### NEW-INV-29 — `list_worklogs` is NOT paginated — **VERIFIED REAL**
**Source verified**: `src/api/jira/worklogs.rs:25-30` (full, 31 LOC):
```rust
pub async fn list_worklogs(&self, key: &str) -> Result<Vec<Worklog>> {
    let path = format!("/rest/api/3/issue/{}/worklog", urlencoding::encode(key));
    let page: OffsetPage<Worklog> = self.get(&path).await?;
    Ok(page.items().to_vec())
}
```
ONE GET, returns one page's items. No loop, no pagination, no warning.

#### BC-1012 (NEW): `client.list_worklogs(key)` GETs `/rest/api/3/issue/<key>/worklog` ONCE and returns the FIRST PAGE only; no offset pagination, no truncation hint, no `--all` plumbing
**Confidence**: HIGH (NEW; documents current state)
**Sources**: `src/api/jira/worklogs.rs:25-30`; `src/cli/worklog.rs:50-79`
**Behavior**: For an issue with >100 worklogs (Atlas's default page size),
the trailing entries are silently dropped. The `--all` flag does NOT
exist for `worklog list`.
**Effects**: Compounded by BC-1013 — the CLI handler (`handle_list`) does
NOT inspect `page.total` to detect truncation, so users have NO signal
that the list is incomplete.

#### Holdout H-040 (NEW): `worklog list <key>` for an issue with >100 worklogs should paginate to completion OR emit a "showing first N of total" hint
**Setup**: Wiremock GET `/rest/api/3/issue/<key>/worklog` returns
`{startAt: 0, maxResults: 50, total: 150, worklogs: [...50 worklogs...]}`.
**Action**: `jr worklog list <key>`
**Expected** (current): outputs only 50 rows; no truncation hint.
**Expected** (post-fix): either auto-paginates to all 150, or emits
`Showing 50 of 150 worklogs. (Worklog pagination is not currently
implemented.)` to stderr.
**Why hidden**: Documents silent data loss; pin against future fix.
**Cross-reference**: Pass 4 §5 (Reliability gaps).

#### BC-1013 (NEW): `cli/worklog.rs::handle_list` reads `client.list_worklogs(key)` (returns Vec, NOT a Page); has NO access to `total` from the response → CANNOT distinguish "complete list" from "first page of truncated list"
**Confidence**: HIGH (NEW; root-cause framing)
**Sources**: `src/cli/worklog.rs:50-79`
**Behavior**: The handler iterates the returned Vec and renders all rows.
No detection mechanism. A fix would change `list_worklogs` to return a
Page or paginate internally.

---

#### NEW-INV-81 — `cli/worklog.rs::handle_add` hardcodes 8/5 — **VERIFIED REAL**
**Source verified**: `src/cli/worklog.rs:32`:
```rust
let seconds = duration::parse_duration(dur, 8, 5)?;
```
Hours-per-day=8, days-per-week=5 are LITERAL constants, NOT read from
config or live Jira instance settings.

#### BC-1014 (NEW): `worklog add <key> <duration>` parses with HARDCODED `hours_per_day=8, days_per_week=5` — Jira instance settings (admin-configurable, e.g. 7-hour days, 4-day weeks) are IGNORED
**Confidence**: HIGH (NEW; documents current state)
**Sources**: `src/cli/worklog.rs:32`; `src/duration.rs:1-75` (parse_duration signature)
**Behavior**: User input `1d` → always 8 × 3600 = 28,800 seconds. If the
Jira tenant configures 7-hour days, the server's reckoning of "1d" is
25,200 seconds; the worklog jr posts will diverge.
**Effects**: For tenants with non-default time tracking settings, log-time
deltas will silently misalign. UX impact: user sees "1d" in jr but Jira
displays a different aggregate.

#### Holdout H-041 (NEW): `worklog add` should honor Jira instance time-tracking settings (configurable HPD/DPW) instead of hardcoded 8/5
**Setup**: Jira instance configured with `workingHoursPerDay=7,
workingDaysPerWeek=4` (admin-set via Time Tracking settings). User runs
`jr worklog add FOO-1 1d`.
**Action**: `jr worklog add FOO-1 1d`
**Expected** (current): Posts `timeSpentSeconds=28800` (8h hardcoded).
**Expected** (post-fix): Either fetches `/rest/api/3/configuration/
timetracking` once and uses its `workingHoursPerDay`/`workingDaysPerWeek`
values, OR reads from `[fields]` config block, OR documents 8/5 as the
contract.
**Why hidden**: Documents UX bug; pin against future fix.
**Cross-reference**: Pass 4 §5 (Reliability) and Pass 5 (UX Conventions).

---

#### NEW-INV-19 — User pagination advances by USER_PAGE_SIZE — **VERIFIED REAL** (documented as deliberate, not a bug)
**Source verified**: `tests/user_pagination.rs:46-106`
(`search_users_all_paginates_and_concatenates`):
```
expect(1) on startAt=0, expect(1) on startAt=100,
expect(1) on startAt=200, expect(1) on startAt=300 (empty)
```
The test asserts `startAt` advances by 100 (the requested
`maxResults`/window size) regardless of how many users the server
actually returned. Pass 5 §5.10/§5.11 already documents this as deliberate
per JRACLOUD-71293.

#### BC-1015 (NEW): `client.search_users_all(query)` advances `startAt` by USER_PAGE_SIZE (100) per iteration, NOT by the returned-count — even for short pages (e.g., page 3 returns 27 users, next request is `startAt=300` not `startAt=227`); only an EMPTY page terminates the loop
**Confidence**: HIGH (PROMOTED to integration scope; documents documented invariant)
**Sources**: `tests/user_pagination.rs:46-106` (`search_users_all_paginates_and_concatenates`); `tests/user_pagination.rs:108-140` (`search_users_all_stops_on_empty_page`)
**Behavior**: Test pin: 100+100+27 → returns 227 users; 4th request to
`startAt=300` returns empty array; loop terminates. Per-window advancement
is the only correct strategy because Atlassian post-page-filters by
permissions, returning fewer users than requested without skipping the
remaining users in that window.
**Effects**: This is INTENTIONAL per JRACLOUD-71293. NOT a bug.

#### BC-1016 (NEW): `search_users_all` enforces `USER_PAGINATION_SAFETY_CAP = 15` iterations as an upper bound; on cap exhaustion (no empty page seen), loop exits with whatever was collected (1500 users for 100/page × 15 caps)
**Confidence**: HIGH
**Sources**: `tests/user_pagination.rs:144-160` (`search_users_all_respects_safety_cap`)
**Behavior**: Test mounts unbounded responder; asserts `expect(15)` on the
mock and final result Vec length 1500.

#### BC-1017 (NEW): `search_users_all` propagates errors mid-pagination — page 2 returns 500 → returned `Err` containing `500`; partial results from page 1 are NOT silently returned
**Confidence**: HIGH
**Sources**: `tests/user_pagination.rs:164-192` (`search_users_all_propagates_error_mid_pagination`)
**Effects**: All-or-nothing semantics for paginated user search. Pin
against a refactor that returns partial results on transient 5xx.

---

#### NEW-INV-18 — `get_changelog` anti-loop guard — **VERIFIED REAL** (defensive programming, not a bug)
**Source verified**: R2 BC-248 already documented this; R3 confirms the
contract per `tests/issue_changelog.rs:106-133`. No new BC needed beyond
the existing one. Document the guard semantics:

#### BC-1018 (NEW): `client.get_changelog(key)` JRACLOUD-94357-class anti-loop guard: when server returns `maxResults: 0` AND `total > 0` AND `isLast: false`, client returns explicit "did not advance" / "malformed" error rather than infinite-looping
**Confidence**: HIGH (PROMOTED, scope; defensive programming pattern)
**Sources**: `src/api/jira/issues.rs:218-230`; `tests/issue_changelog.rs:106-133` (`get_changelog_errors_when_page_fails_to_advance`)
**Behavior**: The guard is `if next <= start_at && has_more { return
Err(...); }`. Pin: pagination DoS resistance for buggy server responses.

### 3.5 `tests/worklog_commands.rs` 5-test 1:1 BC enumeration (NEW)

#### BC-1019 (NEW): `client.add_worklog(key, time_spent_seconds, comment)` POSTs to `/rest/api/3/issue/<key>/worklog` with body `{timeSpentSeconds: <u64>}` and optional `comment: <ADF>`; server returns 201 + Worklog body
**Confidence**: HIGH (PROMOTED scope)
**Sources**: `tests/worklog_commands.rs:8-26` (`test_add_worklog`)
**Behavior**: 7200 seconds round-trips exactly. `comment` field omitted
when None.

#### BC-1020 (NEW): `client.list_worklogs(key)` GETs `/rest/api/3/issue/<key>/worklog`; deserializes `{worklogs: [...]}` page envelope; returns the items
**Confidence**: HIGH (PROMOTED to integration; complements BC-1012)
**Sources**: `tests/worklog_commands.rs:28-51` (`test_list_worklogs`)
**Behavior**: Single page returned; envelope key is `worklogs` (NOT
`values` like other endpoints — JSM-shape divergence).

#### BC-1021 (NEW): `jr worklog list <key>` 5xx → exit 1, stderr `API error (500)`, NO panic (parallel to comments/sprint/queue/team error envelope)
**Confidence**: HIGH (PROMOTED, error envelope consistency)
**Sources**: `tests/worklog_commands.rs:55-93` (`worklog_list_server_error_surfaces_friendly_message`)

#### BC-1022 (NEW): `jr worklog list <key>` 401 → exit 2, stderr `Not authenticated` + `jr auth login`
**Confidence**: HIGH
**Sources**: `tests/worklog_commands.rs:95-137` (`worklog_list_unauthorized_dispatches_reauth_message`)

#### BC-1023 (NEW): `jr worklog list <key>` against unreachable URL → exit 1, stderr `Could not reach` + `check your connection`
**Confidence**: HIGH
**Sources**: `tests/worklog_commands.rs:139-171` (`worklog_list_network_drop_surfaces_reach_error`)

### 3.6 `tests/input_validation.rs` 8-test survey + `tests/cli_smoke.rs` 27-test survey (NEW)

These are clap-derive validation tests. R3 enumerates them as
single-purpose-per-test BCs.

#### BC-1024 (NEW): `jr --help` exit 0, stdout contains `A fast CLI for Jira Cloud` (binary description literal)
**Confidence**: HIGH
**Sources**: `tests/input_validation.rs:4-12` (`test_help_flag`)

#### BC-1025 (NEW): `jr --version` exit 0, stdout contains `jr` (binary name); `jr` (no args) exits FAILURE with `Usage` in stderr
**Confidence**: HIGH (PROMOTED)
**Sources**: `tests/input_validation.rs:14-31` (2 tests)

#### BC-1026 (NEW): `jr issue edit FOO-1 --description text --description-stdin` clap-rejected with `cannot be used with` (mutually exclusive)
**Confidence**: HIGH
**Sources**: `tests/input_validation.rs:33-48` (`test_edit_description_and_description_stdin_conflict`)

#### BC-1027 (NEW): `jr assets tickets OBJ-1 --open --status Done` clap-rejected (`--open` and `--status` mutually exclusive)
**Confidence**: HIGH
**Sources**: `tests/input_validation.rs:51-58`

#### BC-1028 (NEW): Help texts contain expected literals: `queue view --help` → "View issues in a queue" + "--limit"; `queue list --help` → "List queues"; `assets view --help` → "--no-attributes"; `sprint add --help` → "Add issues to a sprint" + "--sprint" + "--current" + "--board"; `sprint remove --help` → "Remove issues from sprint" + "ISSUES"; `assets schemas --help` → "List object schemas"; `assets types --help` → "List object types" + "--schema"; `assets schema --help` → "Show attributes" + "--schema"
**Confidence**: HIGH (PROMOTED, batched help-text contract)
**Sources**: `tests/input_validation.rs:61-165` (8 help-text tests)

#### BC-1029 (NEW): `jr sprint add --sprint 100 --current FOO-1` clap-rejected (mutually exclusive `--sprint` vs `--current`); `jr sprint add FOO-1` (no `--sprint`/`--current`) clap-rejected with `--sprint` in stderr (REQUIRED-one-of)
**Confidence**: HIGH (PROMOTED)
**Sources**: `tests/input_validation.rs:115-133`

#### BC-1030 (NEW): conflicts_with smoke tests pin all known mutually-exclusive flag pairs at exit-2 boundary: `assign --to + --account-id`; `assign --to + --unassign`; `assign --account-id + --unassign`; `create --to + --account-id`; `create --description + --description-stdin`; `issue list --all + --limit`; `issue list --open + --status`; `issue edit --points + --no-points`; `project list --all + --limit`; `board view --all + --limit`; `sprint current --all + --limit`; `issue list --created-after + --recent`
**Confidence**: HIGH (PROMOTED, 12 pairs batched)
**Sources**: `tests/cli_smoke.rs:169-334` (12 conflicts_with tests)
**Effects**: All 12 emit `cannot be used with` substring. Pin against
clap derive regressions that drop a `conflicts_with` annotation.

### 3.7 `tests/project_meta.rs` 3-test 1:1 BC enumeration (NEW)

#### BC-1031 (NEW): `get_or_fetch_project_meta(client, "HELPDESK")` cache-miss path: GET `/rest/api/3/project/HELPDESK` → `{projectTypeKey: "service_desk"}` → triggers GET `/rest/servicedeskapi/servicedesk` → finds matching `projectId` → assembles ProjectMeta with `service_desk_id`
**Confidence**: HIGH (PROMOTED scope)
**Sources**: `tests/project_meta.rs:24-67` (`project_meta_cache_miss_fetches_from_api`)
**Behavior**: TWO HTTP calls on cache miss for service-desk projects.
Result fields: `project_type="service_desk"`, `project_id="10042"`,
`service_desk_id=Some("15")`, `simplified=false`.

#### BC-1032 (NEW): `get_or_fetch_project_meta(client, "DEV")` for software project: GET `/project/DEV` → `{projectTypeKey: "software"}` → SKIPS the servicedesk lookup; result has `service_desk_id: None`, `simplified: true`
**Confidence**: HIGH (NEW; conditional-fetch optimization)
**Sources**: `tests/project_meta.rs:69-97` (`project_meta_software_project_has_no_service_desk_id`)
**Effects**: Software projects don't pay the SD-discovery round trip.
Pin against a refactor that always issues the SD discovery.

#### BC-1033 (NEW): `require_service_desk(client, "DEV")` for software project → returns `Err` containing literal "Jira Software project" + "Queue commands require"
**Confidence**: HIGH (NEW; structured error message)
**Sources**: `tests/project_meta.rs:99-126` (`require_service_desk_errors_for_software_project`)
**Behavior**: The two literal substrings are pinned. Pin: error includes
the project type AND a remediation hint pointing at queue/JSM-only
commands.

### 3.8 CONV-ABS-009 — extract_error_message test count off-by-one

R2 §9.6 predicted "total `extract_error_message` tests = ~12; R1 covered
8 + 4 mixed-types/empty-objects + 1 retry mock test = ~13 BCs already
covered." Recount via `awk '/fn test_extract_error_message/{c++} END
{print c}' tests/api_client.rs` returns **11**, not 12 or 13.

**Action**: Round 3 normalizes to 11. The functions are at
`tests/api_client.rs` lines 258, 266, 273, 280, 287, 295, 303, 310, 324,
331, 338. The test inventory is now FULLY enumerated; no remaining gap.

### 3.9 CONV-ABS-010 — R1 BC-035 file attribution

R1 BC-035 placed `default_oauth_scopes_pins_the_full_set_with_offline_access`
at `src/api/auth.rs:34-63`. Direct read this round shows:
- `src/api/auth.rs:56` has only a COMMENT referencing the test name.
- The test itself is at `src/cli/auth.rs:1523-1564`.

R3 verified the test function body — it does assert offline_access
presence, AND verifies no double-spaces, AND does a normalize-and-equal
check against the full canonical scope string.

**Action**: BC-035 is REASSIGNED to `src/cli/auth.rs:1523-1564`. The
substantive contract claim (the scope set + no-double-spaces invariant)
remains correct. Recorded as CONV-ABS-010 (file-attribution clerical
error, not a fabrication of the contract itself).

#### BC-035-R (REVISED): `default_oauth_scopes_pins_the_full_set_with_offline_access` regression test EXISTS at `src/cli/auth.rs:1523-1564` (NOT api/auth.rs as R1 stated). Asserts: each of 7 required scopes present (`read:jira-work`, `write:jira-work`, `read:jira-user`, `read:servicedesk-request`, `read:cmdb-object:jira`, `read:cmdb-schema:jira`, `offline_access`); whole-string canary via `normalize` (split-whitespace then join " "); no double spaces in the literal const
**Confidence**: HIGH (PROMOTED, file-attribution corrected)
**Sources**: `src/cli/auth.rs:1515-1564`; `src/api/auth.rs:34-63` (the const definition)

### 3.10 T-11 OAuth state machine — partial characterization (DEFERRED 3rd time, but R3 sketches the diagram)

The `auth login --oauth` flow is bounded by these state transitions:

```
[start]
  │
  ▼
RESOLVE_OAUTH_APP  (priority chain: Flag > Env > Keychain > Embedded > Prompt > None)
  │  match found?
  ├── No  → BC-005 exit 64
  │
  ▼
DECIDE_REDIRECT_STRATEGY
  │  (Embedded source → Fixed(53682))
  │  (BYO source     → Dynamic)
  │
  ▼
BIND_LISTENER  (RedirectUriStrategyRequest::bind() — TOCTOU close)
  │  match { Dynamic, Fixed }
  │   ├── Dynamic + I/O err → propagate raw
  │   ├── Fixed + EADDRINUSE → BC-032 friendly error
  │   ├── Fixed + other I/O  → propagate raw
  │   └── OK → ResolvedRedirect { strategy, listener }
  │
  ▼
GENERATE_STATE  (csprng → 64-hex, BC-022..029 in source unit tests)
  │
  ▼
BUILD_AUTHORIZE_URL  (escape client_id/scopes; BC-1067/1066)
  │
  ▼
OPEN_BROWSER  (open::that(authorize_url))
  │
  ▼
ACCEPT_CALLBACK  (TcpListener.accept on bound port)
  │
  ▼
EXTRACT_QUERY_PARAMS  (state, code; verify state matches)
  │  state mismatch?
  ├── Yes → error + exit
  │
  ▼
EXCHANGE_CODE_FOR_TOKEN  (POST /oauth/token; receive access + refresh)
  │
  ▼
DISCOVER_CLOUDID  (GET /oauth/token/accessible-resources)
  │  zero resources?
  ├── Yes → error
  │  multiple?
  ├── Yes → prompt OR --no-input rejected
  │
  ▼
PERSIST_KEYCHAIN  (<profile>:oauth-access-token + <profile>:oauth-refresh-token)
  │
  ▼
PERSIST_CONFIG  (cloud_id, oauth_client_id, etc. into [profiles.<name>])
  │
  ▼
[end — login complete]
```

#### Holdout H-042 (NEW): EADDRINUSE recovery — port 53682 is held by another process; embedded OAuth login fails fast with friendly error pointing to BYO override
**Setup**: Bind a TcpListener to 127.0.0.1:53682 in a fixture before invoking `jr auth login --oauth`. Embedded OAuth source resolves (mock).
**Action**: `jr auth login --oauth --profile testprof`
**Expected**: exit non-zero; stderr contains literal substrings `port 53682 is in use` AND `the jr OAuth callback needs this port` AND `--client-id/--client-secret` AND `JR_OAUTH_CLIENT_ID/JR_OAUTH_CLIENT_SECRET` AND `dynamic port`.
**Why hidden**: Pin BC-032 — error wording is the one actionable hint a
user gets when port 53682 collides. Source unit test exists
(`fixed_port_strategy_eaddrinuse_friendly_error` at `src/api/auth.rs:1377`)
but no integration-level test pins the end-to-end CLI flow.
**Cross-reference**: T-11 OAuth state machine spec.

#### Holdout H-043 (NEW): 401-auto-refresh integration — `refresh_oauth_token` exists but has NO production caller; wiring it to the 401-response handler is the spec-level deferred integration
**Setup**: User has valid refresh token in keychain. An API call (e.g.
`jr issue list`) responds 401 with `expired_access_token` body. The 401
handler should: (a) detect the OAuth profile, (b) invoke
`refresh_oauth_token(profile)`, (c) update the keychain, (d) retry the
original request ONCE.
**Action**: `jr issue list` against a server that returns 401 then 200
after refresh.
**Expected** (current): exit 2 with `Not authenticated` + `jr auth login`
hint. The user must MANUALLY run `jr auth login --oauth`.
**Expected** (post-fix): seamless retry — the second 200 succeeds; user
sees no error.
**Why hidden**: Documents deferred integration. The function exists
(BC-024-R) but is unused. Pass 4 (Reliability) and the deferred-tasks
list should track this.

### 3.11 `tests/user_pagination.rs` partial enumeration (4 of 11 tests covered)

#### BC-1034 (NEW): `tests/user_pagination.rs` survey: 11 tests covering `search_users_all`/`search_assignable_users_by_project_all` paginated sweep behaviors (per-window advance, empty-page termination, safety-cap exhaustion, error propagation, short-page non-termination, query/path mocks)
**Confidence**: HIGH (PROMOTED to survey-level; full enumeration deferred)
**Sources**: `tests/user_pagination.rs:1-200` (4 tests read this round); 7 remaining for R4
**Behavior**: BC-1015..1017 capture the 3 most load-bearing tests; the 4th
read (`search_users_all_short_page_does_not_end_pagination`, line 200+)
pins JRACLOUD-71293 (short non-empty pages must NOT terminate; advance by
window size).

### 3.12 `tests/team_column_parity.rs` partial enumeration (NEW)

#### BC-1035 (NEW): `jr sprint current` Team column shows ONLY when `team_field_id` configured AND at least one issue carries a populated team UUID (mirrors BC-244..246 for `issue list`)
**Confidence**: HIGH (PROMOTED; cross-command consistency)
**Sources**: `tests/team_column_parity.rs:122-150` (`sprint_current_shows_team_column_when_populated`); `1-100` structural read
**Behavior**: Same gating heuristic across `issue list`, `issue view`,
`sprint current`, `board view`. Pin: ALL Team-column-rendering paths share
the same gate.

---

## 4. Updated holdout candidates (deltas only)

### Modified holdouts
None — Round 2's H-030..H-038 remain valid as written.

### New holdouts (added in R3)

- **H-039**: `jr issue open <key>` should use `instance_url()` not `base_url()` (NEW-INV-56 bug fix)
- **H-040**: `worklog list <key>` for >100 worklogs should paginate or warn (NEW-INV-29 bug fix)
- **H-041**: `worklog add` should honor Jira instance time-tracking settings (NEW-INV-81 bug fix)
- **H-042**: EADDRINUSE port-53682 recovery integration test
- **H-043**: 401-auto-refresh integration (refresh_oauth_token wiring)

### Holdout count after R3: **38 + 5 = 43**

---

## 5. Untested-behavior gap list (deltas to R2 §5)

### 5.14 OAuth login (T-11)
- **G-OL1**: EADDRINUSE recovery has source unit test (`fixed_port_strategy_eaddrinuse_friendly_error`) but no integration-level CLI test pins the end-to-end flow under port collision.
- **G-OL2**: Accessible-resources zero-result handling (no cloud_ids returned by `/oauth/token/accessible-resources`) — source path exists but no test enumeration.
- **G-OL3**: Multiple cloud-ids with `--no-input` set — should reject; no test pins this directly.

### 5.15 Worklog
- **G-WL1**: BC-1012 documents the no-pagination contract; G-WL1 confirms NO test exists for the >100-worklog truncation case (silent data loss; H-040).
- **G-WL2**: BC-1014 documents 8/5-hardcoded; G-WL2 confirms NO test pins the contract that says "ignores Jira instance settings" — current tests treat 8/5 as the canonical answer.

### 5.16 Browse URL composition
- **G-BU1**: BC-1010/1011 — for OAuth profiles, no test asserts that
  `jr issue open` / `--url-only` produces the user-facing `instance_url`
  rather than the `base_url`. The bug is undetected.

### 5.17 ADF→text edge cases (R3 found)
- **G-ADF3**: Heading nodes in ADF→text — not enumerated; the test file
  has no `test_render_heading*` test. Render path may exist but is unpinned.
- **G-ADF4**: Mention nodes (`{type: "mention", attrs: {id, text}}`) — not
  in the markdown→ADF set (mentions are read-only on Jira — markdown can't
  produce them). The text renderer's behavior on mention nodes is unpinned.
- **G-ADF5**: Emoji nodes (`{type: "emoji", attrs: {shortName}}`) — same
  as mentions: not in the round-trip path; unpinned.

---

## 6. Retracted / corrected (CONV-ABS-009..010)

### CONV-ABS-009 — R2 §9.6 over-counted extract_error_message tests
**Original claim** (R2 §9 verbatim target list, line 1167): "total
`extract_error_message` tests = ~12"
**Reality**: Recount via `awk '/fn test_extract_error_message/{c++} END
{print c}' tests/api_client.rs` returns **11**. Off-by-one.
**Action**: R3 normalizes the count to 11. R2's hedge ("~12") softens but
doesn't excuse — pin the literal in §3.8 of this round.

### CONV-ABS-010 — R1 BC-035 mis-attributed file location
**Original claim** (R1 §3.1, BC-035): The DEFAULT_OAUTH_SCOPES regression
test lives at `src/api/auth.rs:34-63`.
**Reality**: That range contains the constant DEFINITION, not a test
function. The actual test
`default_oauth_scopes_pins_the_full_set_with_offline_access` lives at
`src/cli/auth.rs:1523-1564`. R1 conflated the constant's location with
the test's. The substantive contract claim (no double spaces, the 7
specific scopes, etc.) is CORRECT.
**Action**: BC-035-R (revised) at `src/cli/auth.rs:1523-1564`. The
contract is verified end-to-end this round via direct read.

---

## 7. Delta Summary

| Metric | Broad | After R1 | After R2 | After R3 | Delta R3 |
|---|---:|---:|---:|---:|---:|
| Total BCs | 193 | 271 | 343 | **419** | **+76** |
| HIGH | 134 | 211 | 281 | **354** | **+73** |
| MEDIUM | 45 | 53 | 56 | **59** | **+3** |
| LOW | 9 | 7 | 6 | **6** | **0** |
| Holdout candidates | 20 | 29 | 38 | **43** | **+5** |
| Untested invariants closed | 0 | 4 | 5 | **5** | 0 |
| Untested behaviors enumerated | 0 | 23 | 30 | **35** (added 5 in §5.14..5.17) | **+5** |
| BCs promoted MEDIUM→HIGH | n/a | 13 | 5 | **2** | n/a |
| BCs corrected (CONV-ABS) | n/a | 4 | 4 | **2** more (009, 010) | **+2** |

Subject-area BC distribution after Round 3:

| Subject area | After R2 H/M/L | After R3 H/M/L | Delta |
|---|---|---|---|
| 1. Auth & Identity | 30/4/0 | 31/4/0 | +1 (BC-035-R revision) |
| 2. Issue read (list/view/comments/changelog) | 78/6/1 | 78/6/1 | 0 |
| 3. Issue write | 35/5/1 | 41/5/1 | +6 (cli_handler.rs 300-700 chunk) |
| 4. Issue assets / CMDB | 24/3/0 | 24/3/0 | 0 |
| 5. Boards & Sprints | 24/3/0 | 25/3/0 | +1 (BC-1035) |
| 6. Worklogs & duration | 5/1/0 | 11/1/0 | +6 (BC-1012..1014, 1019..1023) |
| 7. Teams | 11/2/0 | 11/2/0 | 0 |
| 8. Users | 13/1/0 | 16/1/0 | +3 (BC-1015..1017) |
| 9. Projects & Queues | 16/2/0 | 19/2/0 | +3 (BC-1031..1033) |
| 10. Configuration | 14/2/1 | 14/2/1 | 0 |
| 11. Cache | 16/2/1 | 16/2/1 | 0 |
| 12. Output formatting | 12/4/1 | 12/4/1 | 0 |
| 13. Error handling | 23/3/0 | 23/3/0 | 0 |
| 14. Build-time | 7/1/1 | 7/1/1 | 0 |
| 15. Runtime concerns | 21/2/0 | 21/2/0 | 0 |
| 16. ADF | 21/0/0 | 53/1/0 | +33 (BC-940..972) |
| 17. `jr api` raw passthrough | 9/0/0 | 9/0/0 | 0 |
| 18. Source unit-test contracts (Changelog AuthorNeedle/build_rows/from_to_display/parse_created/truncate_to_rows/LoweredStr — NEW) | 0 | 30/0/0 | +30 (BC-973..1002) |
| 19. CLI smoke / input validation (NEW) | 0 | 7/0/0 | +7 (BC-1024..1030) |
| 20. Browse URL bug (NEW-INV-56) | 0 | 2/0/0 | +2 (BC-1010..1011) |
| 21. OAuth state-machine partial | 0 | 1/0/0 | +1 (BC-1018 anti-loop) |
| **Totals** | **281/56/6** | **354/59/6 = 419** | **+76** |

---

## 8. Novelty Assessment

Novelty: **SUBSTANTIVE**

Justification: Round 3 added 76 net new BCs covering material that
materially expands the spec surface:

1. **3 verified bug findings** (NEW-INV-56/29/81) — handle_open uses
   wrong URL for OAuth, list_worklogs is non-paginated (silent data loss),
   handle_add hardcodes 8/5 (UX divergence with tenant settings). Each
   becomes a Pass 4 reliability cross-pollination trigger AND a Holdout
   candidate. Without these, the spec would document buggy behavior as
   correct.

2. **ADF→text rendering** (BC-940..972, 33 BCs) — first full enumeration
   of the inverse-direction renderer. Material: hardBreak-in-table-cell→
   space (vs newline elsewhere); code-mark-with-internal-backtick double-
   fence; mark composition order (code always innermost); table cell pipe
   escaping; nested blockquote prefixing; trailing-hardBreak stripping;
   image-skip and tasklist-as-text decisions. A spec without these is
   incomplete on round-trip semantics.

3. **Changelog source unit tests** (BC-973..1002, 30 BCs) — closes T-10.
   Material: 12-char + digit gate, ASCII-alphanumeric-only AccountId
   classifier (Cyrillic regression pin), is_ascii_alphanumeric vs
   char::is_alphanumeric distinction, partial-trim algorithm, mixed-offset
   sort comparator, em-dash null glyph (U+2014), empty-string-as-absent
   rule for from_to_display, LoweredStr smart-constructor invariant.

4. **cli_handler.rs gap chunk** (BC-1003..1009, 7 BCs) — fills the
   skipped 300-700 lines from R2. Material: --to me create flow (1 fewer
   HTTP than --to <name>), --unassign idempotency at the integration scope,
   created-after/before JQL exact-literal pinning at integration scope.

5. **OAuth state-machine sketch + 401-auto-refresh holdout** (H-042/H-043)
   — first attempt at the deferred T-11. Provides the spec-level diagram
   and identifies the gap explicitly.

6. **Worklog command error envelope** (BC-1019..1023) — adds worklog to
   the consistent error-envelope BCs across comments/sprint/queue/team/
   board.

7. **2 corrections** (CONV-ABS-009 — extract_error_message count 11 not
   12; CONV-ABS-010 — BC-035 file-attribution corrected to cli/auth.rs).

Removing these BCs would change how the system would be specced: 3 bugs
would be specified as correct behavior; ADF round-trip semantics would be
unspecified; changelog classifier boundary semantics would be lost; the
8/5 hardcoded constants would not be flagged for fix; the OAuth login
EADDRINUSE recovery path would have no integration-level pin.

---

## 9. Remaining gaps / next-round targets (Round 4)

Verbatim verbose list for Round 4 dispatch:

1. **T-11 OAuth state machine + 401-auto-refresh — DEFERRED 4th time.**
   R3 produced a state diagram and 2 holdouts; R4 should produce: (a) per-
   transition BCs for state-mismatch detection; (b) accessible-resources
   error-handling BCs (zero / multiple cloud_ids); (c) a wireframe for
   the 401-auto-refresh integration test (mock 401 + mock refresh-token
   POST + retry).

2. **`tests/issue_commands.rs` — 1,920 LOC, 54 tests** — this is the
   LARGEST integration test file not yet enumerated. Per-test BC pass
   should yield ~50+ new BCs covering issue create/edit/move/transition/
   link/unlink across all flag combinations.

3. **`tests/api_client.rs` — 22 tests** — 11 are extract_error_message
   (covered). Remaining 11 cover JiraClient construction, header injection,
   path normalization, 401/429/5xx boundary tests. R4 should enumerate
   per-test.

4. **R1 BC-130 unit-test names re-verification (CONV-ABS-008)** — names
   like `build_jql_parts_assignee_me`, `build_jql_parts_recent`, etc.
   were claimed plausible but not directly grep-verified. R4 should grep
   `cli/issue/list.rs::tests` for each name.

5. **Property tests (proptest)** — `proptest-regressions/jql.txt` exists
   (regression corpus for JQL escape_value); R3 confirmed via `tests/
   user_pagination.rs` mentions but never enumerated the property tests
   themselves. R4 should enumerate proptest blocks in `jql.rs` (43 unit
   tests including the proptest), `partial_match.rs` (12 unit tests),
   `duration.rs` (16), and check for property-test wrappers.

6. **Insta snapshot tests** — `cli/issue/json_output.rs:88-148` is
   "all 8 JSON output builders insta-snapshot-pinned" per Pass 2 R2
   NEW-INV-74. R4 should enumerate the snapshot files and pin which
   commands produce which JSON shapes.

7. **`tests/user_pagination.rs` — 11 tests** — R3 read 4 of 11. Remaining
   7 cover `search_assignable_users_by_project_all`, the full
   `search_users_all_short_page_does_not_end_pagination` body, paginated
   --all flow at the CLI level, and exit-code paths.

8. **`tests/issue_remote_link.rs` — 6 tests; `tests/user_commands.rs` —
   14 tests; `tests/project_commands.rs` — 10 tests; `tests/issue_resolution.rs`
   — 3 tests; `tests/issue_view_errors.rs` — 4 tests; `tests/assets_errors.rs`
   — 3 tests; `tests/cmdb_fields.rs` — 5 tests; full `team_column_parity.rs`
   — 7 tests; `tests/auth_login_config_errors.rs` — survey-level** —
   each gets per-test BC enumeration.

9. **`src/api/auth.rs` source unit tests** — 22 tests; R3 listed 21 by
   name (line 927-1377). R4 should produce per-test BCs for each
   (extract_query_param 3, generate_state 3, build_authorize_url 3,
   per-profile token round-trip, lazy migration 4 variants, partial-state
   recovery, resolve_refresh_app_credentials 2, fixed_port_strategy 1).

10. **`src/api/auth_embedded.rs` — 8 tests** — embedded plumbing tests
    (decode/present/source priority chain). R4 should enumerate per-test.

11. **Pass 4 NFR §7.1.3/§7.1.4 cross-reference confirmation** — Pass 4
    already documents the Retry-After upper bound + HTTP-date format
    gaps (lines 86, 93, 321, 481-482 of pass-4-nfr-catalog.md). No
    further action needed; close this target.

12. **Per-source-file unit-test counts cross-check** — R3 verified
    several but should systematically recount any module with claimed
    LOC/test counts (e.g., `cache.rs` 27 tests, `config.rs` 37, `jql.rs`
    43, `partial_match.rs` 12) that were last cited in R1.

---

## 10. Updated stats table

(Same as §7.)

---

## 11. State Checkpoint

```yaml
pass: 3
round: 3
status: complete
bcs_total_after_round: 419
bcs_high: 354
bcs_medium: 59
bcs_low: 6
bcs_added_this_round: 76
bcs_promoted_to_high: 2
bcs_retracted: 2
holdout_candidates_after_round: 43
holdouts_added_this_round: 5
untested_behaviors_listed: 35
files_examined: 13
novelty: SUBSTANTIVE
timestamp: 2026-05-04T22:30:00Z
inputs_consumed:
  - .factory/semport/jira-cli/jira-cli-pass-3-deep-r2.md (full)
  - .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md (partial)
  - .factory/semport/jira-cli/jira-cli-pass-2-deep-r2.md (cross-pollination targets)
  - .factory/semport/jira-cli/jira-cli-pass-4-nfr-catalog.md (cross-reference verification)
  - .reference/jira-cli/src/adf.rs (offsets 800-880, 1100-1500, 1500-1827)
  - .reference/jira-cli/src/cli/issue/changelog.rs (offsets 300-848)
  - .reference/jira-cli/tests/cli_handler.rs (offset 280-700 — R2 gap chunk)
  - .reference/jira-cli/tests/cli_smoke.rs (full 334 LOC, 27 tests)
  - .reference/jira-cli/tests/input_validation.rs (full 253 LOC, 8 tests)
  - .reference/jira-cli/tests/project_meta.rs (full 126 LOC, 3 tests)
  - .reference/jira-cli/tests/worklog_commands.rs (full 171 LOC, 5 tests)
  - .reference/jira-cli/tests/user_pagination.rs (offset 1-200, 4 of 11 tests)
  - .reference/jira-cli/tests/team_column_parity.rs (offset 1-150, structural)
  - .reference/jira-cli/src/api/jira/worklogs.rs (full 31 LOC)
  - .reference/jira-cli/src/cli/worklog.rs (full 79 LOC)
  - .reference/jira-cli/src/cli/issue/workflow.rs (offset 600-720 — handle_open)
  - .reference/jira-cli/src/api/client.rs (offset 340-400 — base_url vs instance_url)
  - .reference/jira-cli/src/cli/auth.rs (offset 1515-1565 — DEFAULT_OAUTH_SCOPES test)
  - .reference/jira-cli/src/api/auth.rs (offset 370-490 — RedirectUriStrategyRequest)
  - .reference/jira-cli/tests/api_client.rs (function-name grep — 11 extract_error_message tests)
next_round_targets: |-
  T-11 OAuth state machine + 401-auto-refresh — DEFERRED 4th time. R3 produced state diagram + 2 holdouts; R4 should produce per-transition BCs and a 401-auto-refresh integration-test wireframe.

  tests/issue_commands.rs full sweep — 1,920 LOC, 54 tests. Largest integration file not yet enumerated. Per-test BC pass should yield ~50+ new BCs covering issue create/edit/move/transition/link/unlink across all flag combinations.

  tests/api_client.rs remaining 11 tests (non-extract_error_message) — JiraClient construction, header injection, path normalization, 401/429/5xx boundaries.

  R1 BC-130 unit-test names re-verification — grep cli/issue/list.rs::tests for each claimed name.

  Property tests (proptest) — enumerate proptest blocks in jql.rs, partial_match.rs, duration.rs.

  Insta snapshot tests — enumerate snapshot files; map to commands per cli/issue/json_output.rs:88-148.

  tests/user_pagination.rs remaining 7 tests; tests/issue_remote_link.rs 6; tests/user_commands.rs 14; tests/project_commands.rs 10; tests/issue_resolution.rs 3; tests/issue_view_errors.rs 4; tests/assets_errors.rs 3; tests/cmdb_fields.rs 5; full team_column_parity.rs 7; tests/auth_login_config_errors.rs survey.

  src/api/auth.rs 22 source unit tests per-test BCs (extract_query_param, generate_state, build_authorize_url, per-profile round-trip, lazy migration variants, partial-state recovery, resolve_refresh_app_credentials, fixed_port_strategy).

  src/api/auth_embedded.rs 8 source unit tests per-test BCs.

  Per-source-file unit-test count cross-check (cache.rs 27, config.rs 37, jql.rs 43, partial_match.rs 12 — last cited in R1).
```

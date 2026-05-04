# Pass 2 Deepening — Round 3 — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04

## 1. Round metadata

- **Round**: 3
- **Predecessor**: `jira-cli-pass-2-deep-r2.md`
- **Targets attacked (verbatim from Round 2 §9)**:
  - **#1** — `adf.rs` deep round 2 (1,826 LOC) — 69 unit tests, table-render edge cases, `wrap_inlines_as_blocks` allowlist, ADF→text whitespace rules, silent-drop nodes
  - **#2** — OAuth state machine deep round 2 — full `build.rs` XOR pipeline + every OAuth-related test
  - **#3** — `cli/auth.rs` deep round 2 — `chosen_flow_for_profile` test pinning + JSON output shapes + keyring round-trip tests + `prepare_login_target`
  - **#4** — `cli/issue/list.rs` deep round 2 — `format_issue_row` exact column ordering + `compose_extra_fields` (actually in `helpers.rs`) + `handle_view` (actually in `view.rs`) + `--no-color` mechanism
  - **#5** — `cli/assets.rs` deep round 2 — `resolve_schema` partial-match + `attribute` table-render display_value/value coalesce
  - **#6** — `cache.rs` deep round 2 — `with_temp_cache` scaffolding + cross-mutex interaction
  - **#7** — `config.rs` deep round 2 — migration field-by-field + NEW-INV-12 (multi-profile fields bug) verification
  - **#8** — `api/auth.rs` line-by-line — `clear_profile_creds` vs `clear_all_credentials` + `read_keyring_optional` + `KEYRING_TEST_ENV_MUTEX`
  - **#10** — `output.rs` (76 LOC) — NITPICK candidate
  - **#11** — `error.rs` JrError construction sites
  - **#12** — `build.rs` + `embedded_oauth.rs` codegen XOR pipeline
  - (Additionally surfaced: `cli/issue/view.rs`, `cli/issue/comments.rs`, `observability.rs` — none catalogued by broad/R1/R2)

(Pass 4 cross-pollination items #13-17 reserved for Pass 4 — not written into this Pass 2 file.)

---

## 2. Audit of Round 2 against the 5 Known Hallucination Classes

### Class 1 — Over-extrapolated token lists
- **R2 NEW-INV-19 USER_PAGE_SIZE = 100, USER_PAGINATION_SAFETY_CAP = 15** — re-verified at `api/jira/users.rs:8, 16`. Confirmed. ✓
- **R2 E-API-09 `list_worklogs` is NOT paginated** — re-read `api/jira/worklogs.rs:25-30`. **VERIFIED** — single `get` call, no loop, returns `page.items().to_vec()`. **NEW-INV-29 is a real, confirmed defect.** ✓
- **R2 E-API-01 `BASE_ISSUE_FIELDS` 16 fields** — re-read `api/jira/issues.rs:12-29`. Counted: `summary, status, issuetype, priority, assignee, reporter, project, description, created, updated, resolution, components, fixVersions, labels, parent, issuelinks` = 16. ✓
- **R2 NEW-INV-22 "Story-points name allowlist (only 2 names)"** — re-read `api/jira/fields.rs:45-48` (`KNOWN_SP_SCHEMA_TYPES`) + name filter (`api/jira/fields.rs:50-81`). Two **schema** types AND two **names** ("story points", "story point estimate"). Confirmed. ✓
- **R2 §3.7 NEW-INV-60 / NEW-INV-61 — `JR_BASE_URL` and `JR_AUTH_HEADER` test seams** — re-verified `api/client.rs:37, 65-67`. ✓
- **R2 §3.6 E-LK-04 scheme allowlist `http`/`https`** — verified.

### Class 2 — Miscounted enumerations

- **CONV-ABS-2 (RETRACTION):** R2 §9 carryover #3 framed `cli/auth.rs` deep round 2 as "**12** keyring round-trip tests". **Recount**: The keyring round-trip tests (gated by `JR_RUN_KEYRING_TESTS=1`, `#[ignore]`, with `KEYRING_TEST_ENV_MUTEX`) live in `api/auth.rs` (NOT `cli/auth.rs`), and there are exactly **10** of them at lines 1131, 1146, 1154, 1186, 1222, 1250, 1272, 1324, 1348, 1363. R2's "12" overcounted by 2. Both error directions: file misattributed (cli vs api) AND count overstated. **Round 3 catalogues 10 keyring tests with line numbers in §3.6.**

- **R2 self-claim "39 new entities, 75 new invariants"** — recount of R2 §5:
  - Entities: E-API-01..11 (11) + E-PAG-01..05 (5, includes the deserializer) + E-CL-01..07 (7) + E-HE-01..07 (7) + E-WF-01..07 (7) + E-LK-01..04 (4) + E-CLI-01..05 (5) + E-API-CLI-01..05 (5) + E-CR-01..05 (5) + E-JO-01..02 (2) + E-CMD-01..06 (6) + E-INIT-01..03 (3) = **67**, not 39. R2's "39 new entities" is a delta count vs Round 1's prior coverage; the listed E-XX entries (post-Round-1) sum to 67, but R2's table-row delta count of 39 was structured to highlight ONLY the genuinely new categories. Not a fabrication; a counting-convention slip. **R2's text "+18% Round 2 vs Round 1" makes the convention clear retrospectively.** No retraction (this is internal accounting, not a load-bearing claim).
  - Invariants: NEW-INV-18..92 → range = 75. Verified by direct count. ✓

- **R2 NEW-PAT-01 "3 sites: ... an unverified third site"** — Round 3 verifies. There are exactly 3 call-sites for `log_parse_failure_once` (with their own `static LOGGED: AtomicBool`):
  1. `types/jira/issue.rs:103` (team_id parse-failure log)
  2. `cli/issue/format.rs:119` (format_comment_date)
  3. `cli/issue/changelog.rs:269` (format_date for changelog)
  
  Plus the helper at `observability.rs:16` itself (which R2 did not catalogue). R2's "3 sites" claim is correct; "an unverified third site" was the changelog one — verified now. ✓

### Class 3 — Named pattern conflation / fabrication

- **CONV-ABS-3 (CORRECTION):** R2 §3.11 E-CMD-06 said `cli/user.rs --no-color` toggles "ANSI emission via `colored`'s env var override (`NO_COLOR`)". **Re-reading `main.rs:13-15`:** the actual mechanism is `colored::control::set_override(false)` — a direct API call on the `colored` crate, NOT setting the `NO_COLOR` env var. The env var IS read at line 13 (`std::env::var("NO_COLOR").is_ok()`) as a TRIGGER for the same `set_override(false)` call. R2's "via `colored`'s env var override" framing collapses two distinct mechanisms (CLI flag + env trigger) into one ("env var override"). **Corrected:** the `--no-color` CLI flag and the `NO_COLOR` env var are TWO independent triggers, both routing through `colored::control::set_override(false)`. Logged as Round 2 framing slip; no Round 1 substantive claim retracted.

- **R2 §3.5 NEW-INV-56 "`handle_open` uses `base_url`" potential bug** — re-verified at `cli/issue/workflow.rs:636`: `let url = format!("{}/browse/{}", client.base_url(), key);`. Cross-referenced `api/client.rs:351-358` for `base_url()` vs `instance_url()`. **CONFIRMED REAL BUG** for OAuth profiles where `base_url()` returns the API gateway (`https://api.atlassian.com/ex/jira/<cloud_id>`). The browser would receive a JSON response or 404 instead of the issue page. ✓ Pass 4 (reliability) and Pass 3 (BC) candidate.

- **R2 §3.11 NEW-INV-81 "worklog hardcoded 8/5"** — re-verified at `cli/worklog.rs:32`: `let seconds = duration::parse_duration(dur, 8, 5)?;`. **CONFIRMED REAL DEFECT.** The `duration::parse_duration` signature (per Round 1 NEW-INV-04) requires HPD/DPW parameters, but the worklog handler passes literal `8, 5` regardless of Jira instance setting. ✓ Pass 4 candidate.

- **R2 §3.1 NEW-INV-18 "`get_changelog` anti-loop guard"** — re-verified at `api/jira/issues.rs:218-230`. The guard `if next <= start_at { return Err(...) }` fires when `has_more=true` but `start_at` wouldn't advance. Error message references "JRACLOUD-94357-class". ✓ Confirmed real defensive code.

- **R2 §3.1 NEW-INV-19 "fixed-window pagination"** — re-verified at `api/jira/users.rs:98`: `start_at = start_at.saturating_add(USER_PAGE_SIZE);`. Source comment at lines 72-83 explicitly cites JRACLOUD-71293 and the rationale (advance by window, not returned count, to avoid post-permission-filter overlap). ✓

### Class 4 — Same-basename artifact conflation

- **CONV-ABS-4 (CORRECTION — propagation of CLAUDE.md framing):** Both broad pass §2a.1 AND R2 (and CLAUDE.md itself) frame `list.rs` as "list + view + comments". **Re-reading `cli/issue/`:** the `view` and `comments` operations live in dedicated sibling modules:
  - `cli/issue/list.rs` (1,083 LOC) — `handle_list` ONLY (per `super::view`/`super::comments` re-imports)
  - `cli/issue/view.rs` (286 LOC) — `handle_view` (entirely new — never catalogued)
  - `cli/issue/comments.rs` (61 LOC) — `handle_comments` (entirely new — never catalogued)
  
  CLAUDE.md is stale on this point. Round 3 catalogues `view.rs` and `comments.rs` as new entities (E-VIEW-01, E-CMTS-01).

- **R2 §3.1 E-API-08 `api/jira/links.rs (97 LOC)` distinct from `cli/issue/links.rs (293 LOC)`** — re-verified by `wc -l`. Two separate files. ✓
- **`cli/auth.rs` (1,998 LOC) vs `api/auth.rs` (1,397 LOC)** — re-verified. Distinct files (Round 1 §3.1 covered the latter; R2 carryover #3 conflated test residence: see CONV-ABS-2 above).

### Class 5 — Inflated or deflated metrics (LOC recount)

| File | R2 cited | Actual | Delta |
|---|---:|---:|---|
| `src/adf.rs` | 1,826 | 1,826 | 0 ✓ |
| `src/cli/auth.rs` | 1,998 | 1,998 | 0 ✓ |
| `src/api/auth.rs` | 1,397 | 1,397 | 0 ✓ |
| `src/cli/issue/list.rs` | 1,083 | 1,083 | 0 ✓ |
| `src/cli/assets.rs` | 1,055 | 1,055 | 0 ✓ |
| `src/cache.rs` | 899 | 899 | 0 ✓ |
| `src/config.rs` | 1,223 | 1,223 | 0 ✓ |
| `src/cli/issue/workflow.rs` | 788 | 788 | 0 ✓ |
| `src/api/jira/worklogs.rs` | 31 | 31 | 0 ✓ |
| `src/cli/worklog.rs` | 79 | 79 | 0 ✓ |
| `src/api/jira/users.rs` | 290 | 290 | 0 ✓ |
| `src/api/jira/issues.rs` | 314 | 314 | 0 ✓ |
| `src/error.rs` | 137 | 136 | -1 (1-LOC trailing-newline rounding) |
| `src/output.rs` | 76 | 76 | 0 ✓ |
| `src/api/auth_embedded.rs` | (not cited) | 250 | n/a |
| `build.rs` | (not cited) | 125 | n/a |
| `src/cli/issue/view.rs` | (not cited; mis-claimed inside list.rs) | 286 | n/a (NEW) |
| `src/cli/issue/comments.rs` | (not cited; mis-claimed inside list.rs) | 61 | n/a (NEW) |
| `src/cli/issue/format.rs` | (not cited) | 226 | n/a |
| `src/observability.rs` | (not cited; not in CLAUDE.md tree) | 39 | n/a (NEW) |
| `tests/common/fixtures.rs` | (not cited) | 446 | n/a |

**Hallucination class audit summary**: **0 prior-round substantive entity/invariant retracted**. **3 framing/counting corrections logged**: CONV-ABS-2 (12 keyring tests → 10), CONV-ABS-3 (`--no-color` mechanism mis-collapsed), CONV-ABS-4 (CLAUDE.md `list.rs = list+view+comments` framing propagation). All five "potential bug" claims (NEW-INV-18, 19, 29, 56, 81) **verified real** by line-by-line source re-read.

---

## 3. Sub-pass 2a deepening: structural — entity model per target

### 3.1 T-ADF-R2: `adf.rs` deepened catalog (1,826 LOC, 69 unit tests)

#### E-ADF-R2-01 — Test-corpus enumeration
`grep -c '#\[test\]' src/adf.rs` → **69** unit tests exactly. Range `lines 686..1825`. Categories (verified):
- 5 markdown→ADF basic shape tests (688-721)
- 1 markdown→ADF complex snapshot (758, `insta::assert_json_snapshot!`)
- 16 markdown→ADF feature tests (792-1163)
- 1 ADF→text complex snapshot (1055, `insta::assert_snapshot!`)
- 5 markdown→ADF→ADF roundtrip + edge tests
- 41 ADF→text rendering tests (1167-1825) covering all node types, marks, table edge cases, blockquote nesting, code-mark interactions

#### E-ADF-R2-02 — `wrap_inlines_as_blocks` allowlist (lines 173-211)
**Two distinct allowlists** at separate call sites:
- **listItem allowlist (lines 175-185)** — 9 block types: `paragraph, bulletList, orderedList, blockquote, codeBlock, heading, table, rule, mediaSingle`.
- **tableCell/tableHeader allowlist (lines 202-209)** — 6 block types: `paragraph, bulletList, orderedList, blockquote, codeBlock, heading`. **table, rule, mediaSingle are NOT allowed inside cells** — would violate ADF schema (no nested tables).
- **NEW INVARIANT (NEW-INV-93)**: Two allowlists differ by 3 entries; cells cannot nest tables, rules, or media. Adding `table` to the cell allowlist would emit valid ADF JSON but Jira's renderer rejects nested tables.

#### E-ADF-R2-03 — `wrap_inlines_as_blocks` empty-input contract (lines 309-311)
**Empty children produce a single empty paragraph** `[json!({"type":"paragraph","content":[]})]`. Documented as "ADF requires at least one block in listItem/tableCell". **NEW INVARIANT (NEW-INV-94)**: A markdown source `- ` (bullet with nothing) emits a `listItem > paragraph[]` rather than `listItem[]`. Pinned by lookup pattern (no test directly hits the empty-input branch but it's the only emission that satisfies ADF schema requirement).

#### E-ADF-R2-04 — `tableRow` separator emission rule (lines 496-525)
**Separator row emitted when `has_header == true`** — `has_header` is set if ANY cell in the row is `tableHeader` (lines 509-510). **NEW INVARIANT (NEW-INV-95)**: A row mixing `tableHeader` + `tableCell` (e.g., one header cell + one body cell in the same row) STILL emits the separator `| --- | --- |`. Pinned by `test_render_table_mixed_header_cell_row_still_emits_separator` (line 1194-1213). The separator is per-row, not per-table, so a malformed multi-header-row table emits multiple separators.

#### E-ADF-R2-05 — Table-cell text sanitization (`sanitize_table_cell_text`, lines 609-611)
**Two replacements applied in order**:
1. `\r` and `\n` → ` ` (collapse newlines).
2. `|` → `\|` (escape pipes).
- **Order matters**: a literal `|\n` becomes `| ` (after newline-replace) and then escapes the now-orphan pipe. **NEW INVARIANT (NEW-INV-96)**: Newline-replace runs BEFORE pipe-escape; reversing would produce `\|\n` (literal newline preserved as escape) and break the row structure. Pinned by `test_render_table_cell_collapses_newlines_in_text` (1807-1825) and `test_render_table_cell_escapes_pipe_in_text` (1784-1804).

#### E-ADF-R2-06 — `wrap_code_span` delimiter-length adaptation (lines 618-636)
**CommonMark rule**: inline-code delimiter must have MORE backticks than the longest run inside. Algorithm:
1. Scan for longest backtick run (`current` counter, reset on non-backtick).
2. Use `longest_run + 1` backticks as delimiter.
3. **Pad with single space if content begins/ends with backtick** (avoids "glue" — CommonMark would parse `` ``\`x\``` `` as `\\`x\\` ` rather than the intended literal `` `x` ``).
- **NEW INVARIANT (NEW-INV-97)**: Pinned by `test_render_code_mark_with_backtick_in_content` (1574-1584): `"foo\`bar"` → `` ``foo`bar`` `` (2 ticks, no padding because content doesn't begin/end with backtick). And `test_render_code_mark_with_leading_trailing_backtick_pads` (1586-1596): `"\`x\`"` → `` `` `x` `` `` (single tick + padding spaces).

#### E-ADF-R2-07 — `apply_marks` code-innermost rule (lines 651-682)
**Code is ALWAYS applied innermost regardless of marks-array order**:
- If `code` is anywhere in `marks`, it's applied first (lines 655-662).
- Other marks then wrap in array order.
- **Rationale (per source comment)**: write-path emits `[strong, code]` for `**\`x\`**` because `push_code` appends `{type: "code"}` after active marks; applying strictly in order would produce `\`**x**\`` (code outermost), losing bold. **NEW INVARIANT (NEW-INV-98)**: Pinned by `test_render_strong_with_code_applies_code_innermost` (1769-1781) and `test_render_marks_code_and_strong` (1670-1686). The reversed-array case `[code, strong]` STILL applies code innermost.

#### E-ADF-R2-08 — Unknown-mark drop semantics (line 678)
**`apply_marks` `_ => result`** — unknown mark types pass through with NO added syntax (e.g., `{"type": "underline"}` produces just the text without `<u>x</u>` markup). **NEW INVARIANT (NEW-INV-99)**: Pinned by `test_render_unknown_mark_drops_syntax` (1378-1386). A future ADF mark type added by Atlassian would render its TEXT but lose its STYLE — failsafe degradation rather than emitting unknown markdown.

#### E-ADF-R2-09 — Unknown-leaf vs unknown-container divergence (lines 531-540)
**Two distinct paths for unknown node types**:
- If the node has NO `content` field → **silent drop** (renders empty string).
- If the node HAS a `content` array → **recurse into children**, salvaging text.
- **Rationale (source comment)**: avoids debug strings like `[unsupported: type]` reaching users; salvages container nodes (`panel`, `nestedExpand`, future Atlassian types) without dropping their text.
- **NEW INVARIANT (NEW-INV-100)**: Pinned by `test_render_unknown_leaf_drops_silently` (732-738) and `test_render_unknown_container_recurses` (740-755). The two-branch design means `mediaSingle`-class leaves with no `content` are silent; `panel`-class containers preserve text.

#### E-ADF-R2-10 — `mention`/`emoji`/`inlineCard`/`media*` silent-drop catalog
**These ADF leaf node types have NO renderer match arm** in `render_node` (lines 379-541). They fall through to `_` → `if node.get("content").is_some()` → recurse if container, else drop. Specifically:
- `mention` (leaf in ADF: `{type: "mention", attrs: {...}}`, no `content`) → **silent drop**, even the user's display name is lost.
- `emoji` (leaf: `{type: "emoji", attrs: {shortName, text}}`, no `content`) → **silent drop**, even the literal `:emoji:` shortname is lost.
- `inlineCard` (leaf: `{type: "inlineCard", attrs: {url}}`, no `content`) → **silent drop**, even the URL is lost.
- `media`/`mediaSingle`/`mediaGroup` (`mediaSingle` and `mediaGroup` are containers; `media` is a leaf) → leaf media: silent drop. mediaSingle: recurses into content (but its only child is a `media` leaf, so net effect is silent).
- **NEW INVARIANT (NEW-INV-101)**: ADF round-trip is LOSSY for `mention`/`emoji`/`inlineCard`/`media*`. A markdown→ADF→text roundtrip preserves text + standard marks; ADF documents authored in the Jira UI containing mentions/emojis/cards LOSE that data on `jr issue view`. **Spec implication**: AI-agent consumption of `jr issue view --output table` will silently miss inlined references; `--output json` preserves them in the raw `description` ADF field.

#### E-ADF-R2-11 — `finish()` trailing-whitespace strip (line 601)
**`output.trim_end().to_string()`** — the rendered output's trailing whitespace (newlines from paragraphs, blank lines from hardBreaks) is stripped before return. **NEW INVARIANT (NEW-INV-102)**: A document ending with `[text("a"), hardBreak, hardBreak]` renders as `"a"` (NOT `"a\n\n"`). Pinned by `test_render_trailing_hard_breaks_stripped_by_finish` (1722-1740). Distinct from interior hardBreaks: `[text("a"), hardBreak, hardBreak, text("b")]` → `"a\n\nb"` (pinned by `test_render_consecutive_hard_breaks_produce_multiple_newlines`, 1652-1668).

#### E-ADF-R2-12 — Blockquote internal-blank-line `>` continuity (lines 478-486)
**Blank line inside blockquote → emit just `>` (no trailing space)**. The standard prefix is `> `; for empty lines it becomes bare `>`. Pinned by `test_render_blockquote_with_internal_blank_line_keeps_prefix` (1599-1620). **NEW INVARIANT (NEW-INV-103)**: A multi-paragraph blockquote (or codeBlock-with-blank-line inside blockquote) preserves blockquote context with the bare `>` marker between content lines. Without this, a blank line would visually break out of the quote.

#### E-ADF-R2-13 — `orderedList` `attrs.order` validation (lines 411-416, 157-160)
**Read-side**: `unwrap_or(1).filter(|&n| n >= 1)` — `0` and negative coerce to 1 (matches Jira's HTML `<ol start>` lenient handling).
**Write-side**: `if start != 1 { node["attrs"] = json!({"order": start}); }` — `start == 1` OMITS the `attrs` field entirely (saves 17 bytes per default-start list).
- **NEW INVARIANT (NEW-INV-104)**: ADF emission for default-start ordered lists has NO `attrs` field — pinned by `test_markdown_ordered_list_omits_order_when_start_is_one` (799-803). A future schema check that asserts presence of `attrs` would regress.

### 3.2 T-OAUTH-R2: OAuth state machine deep round 2

#### E-OAUTH-R2-01 — Full XOR pipeline (build.rs + auth_embedded.rs)
**4-stage pipeline** (verified at `build.rs:14-125` and `src/api/auth_embedded.rs:1-251`):
1. **build.rs reads env**: `JR_BUILD_OAUTH_CLIENT_ID` + `JR_BUILD_OAUTH_CLIENT_SECRET` (both `.filter(|s| !s.is_empty())`). Both required; missing → emits `None`-typed constants (fork/local-build path).
2. **build.rs `generate_xor_key()`**: 32 bytes from OS entropy. `#[cfg(unix)]` reads `/dev/urandom`; `#[cfg(windows)]` calls `BCryptGenRandom` via inline FFI shim with `BCRYPT_USE_SYSTEM_PREFERRED_RNG = 0x00000002`. Other targets: `compile_error!` (jr's release matrix is macOS/Linux/Windows only).
3. **build.rs XOR**: `secret_bytes[i] ^ key[i % 32]`. Writes `EMBEDDED_ID: Option<&str>`, `EMBEDDED_SECRET_XOR: Option<&[u8]>`, `EMBEDDED_SECRET_KEY: Option<&[u8; 32]>` to `$OUT_DIR/embedded_oauth.rs`. **All three module-private (no `pub`)** — only the `embedded_oauth_app()` accessor is public.
4. **runtime decode**: `auth_embedded::decode()` reverses the XOR; `embedded_oauth_app()` uses `OnceLock` to cache the result for the process lifetime (refresh tokens are needed for every `refresh_oauth_token` call).
- **NEW INVARIANT (NEW-INV-105)**: Generation of the XOR key is **per-build, not per-binary**. Two `cargo build` invocations on the same secret produce different ciphertexts — defeats automated diff-based scanners.
- **NEW INVARIANT (NEW-INV-106)**: `build_embedded_app` REJECTS empty inputs (`id.is_empty() || xor.is_empty()` at line 100; empty decoded secret at line 104) — a misconfigured CI pipeline emitting `JR_BUILD_OAUTH_CLIENT_SECRET=""` cannot ship an empty client_secret. Pinned by `build_embedded_app_rejects_empty_inputs` (auth_embedded.rs:194-201).

#### E-OAUTH-R2-02 — `EmbeddedOAuthApp::Debug` redaction (auth_embedded.rs:34-41)
**Manual `Debug` impl** redacts `client_secret` as `<redacted>` literal; `client_id` rendered verbatim (treated as non-secret — identifies the OAuth app to Atlassian, not the user session). **NEW INVARIANT (NEW-INV-107)**: A stray `dbg!(app)` or `tracing::debug!("{app:?}")` cannot leak `client_secret`. Pinned by `embedded_oauth_app_debug_redacts_secret` (220-239).

#### E-OAUTH-R2-03 — `embedded_oauth_app_present()` no-decode probe (auth_embedded.rs:132-136)
**Cheap probe**: checks only `EMBEDDED_ID.is_some_and(|s| !s.is_empty()) && EMBEDDED_SECRET_XOR.is_some_and(|x| !x.is_empty()) && EMBEDDED_SECRET_KEY.is_some()`. **Does NOT decode** — defense in depth so `jr auth status` doesn't materialize the live `client_secret` in process heap. **NEW INVARIANT (NEW-INV-108)**: A user running read-only `jr auth status` does NOT cause the embedded `client_secret` to be decoded. Distinct from `embedded_oauth_app()` which forces the decode.

#### E-OAUTH-R2-04 — `redirect_uri_strategy_strings` test pinning (api/auth.rs:927-937)
**Two distinct redirect URIs** based on strategy:
- `RedirectUriStrategy::FixedPort(53682)` → `"http://127.0.0.1:53682/callback"` (literal IPv4, NOT `localhost`).
- `RedirectUriStrategy::DynamicPort(N)` → `"http://localhost:N/callback"`.
- **Rationale (lines 920-926 doc)**: Atlassian validates `redirect_uri` by exact string match. The embedded app registers `127.0.0.1:53682` to FORCE IPv4 (avoids macOS/Chrome's `localhost`→`::1` resolver pitfall). BYO apps registered with `localhost` keep `localhost` for backward compatibility.
- **NEW INVARIANT (NEW-INV-109)**: The embedded app uses `127.0.0.1` literal (NOT `localhost`); changing to `localhost` would silently break embedded auth on IPv6-preferring systems. Pinned by `redirect_uri_strategy_strings`.
- **NEW INVARIANT (NEW-INV-110)**: `embedded_callback_port_is_53682` test (944-946) is a **type-system lock** — `EMBEDDED_CALLBACK_PORT` constant must equal 53682. Atlassian Developer Console registers exactly this port; changing it is a breaking release. Pinned at compile-time-style assertion.

#### E-OAUTH-R2-05 — `build_authorize_url` adversarial-encoding tests (api/auth.rs:1018-1083)
**3 tests pin URL-encoding correctness**:
1. `test_build_authorize_url_happy_path` (1018-1037): asserts `audience=api.atlassian.com`, `client_id=`, `scope=` is `%20`-encoded (NOT `+`-encoded; Atlassian rejects `+`-encoded space), `redirect_uri=` percent-encoded, `state=`, `response_type=code`, `prompt=consent`.
2. `test_build_authorize_url_escapes_hostile_client_id` (1043-1060): pathological `client_id` `"real_id&redirect_uri=evil.example#frag"` must be fully percent-encoded. **Without uniform encoding, an attacker-controlled client_id could inject a redirect_uri override.**
3. `test_build_authorize_url_escapes_plus_in_scope` (1067-1083): `+` in scope name encodes to `%2B` (form-urlencoded `+` means "space", which would silently corrupt the scope list).
- **NEW INVARIANT (NEW-INV-111)**: The 3 adversarial tests pin `build_authorize_url` against **scope-corruption** (`+` ambiguity), **`redirect_uri` injection** (special char escape), and **encoding-form drift** (`%20` vs `+`).

#### E-OAUTH-R2-06 — `generate_state` entropy tests (api/auth.rs:973-1012)
**3 tests pin CSPRNG quality**:
- `test_generate_state_is_hex` (973-978): all chars are `is_ascii_hexdigit()`.
- `test_generate_state_is_64_hex_chars` (984-992): exactly 64 chars (32 bytes hex). Guards against regression to lower-entropy sources (timestamp-hex, truncated UUIDs).
- `test_generate_state_is_not_deterministic` (1001-1012): 8 calls produce 8 distinct values. Birthday-bound collision probability across 8 samples of 256-bit entropy is ≈ 2^-253 — rigorously not a flake source.
- **NEW INVARIANT (NEW-INV-112)**: OAuth `state` is exactly 256 bits of entropy (32 bytes, 64 hex chars). The 3-test combination ensures any regression toward weaker entropy fails the suite.

#### E-OAUTH-R2-07 — `KEYRING_TEST_ENV_MUTEX` poisoned-lock recovery details (api/auth.rs:1095-1128)
**6 specific behaviors verified**:
1. **Mutex declaration**: `static KEYRING_TEST_ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());` (line 1095).
2. **Poisoned-lock recovery**: `.lock().unwrap_or_else(|poisoned| poisoned.into_inner())` (line 1109-1111). A panicking test does NOT block subsequent tests.
3. **Opt-in gate**: `if std::env::var("JR_RUN_KEYRING_TESTS").is_err() { return; }` — non-keyring CI runs skip silently.
4. **Per-call unique service name**: `unique_test_service()` uses `AtomicU64::fetch_add` + process ID — even if `JR_SERVICE_NAME` set/unset races (which the mutex prevents), keychain entries don't collide.
5. **`unsafe { std::env::set_var(...) }` justification**: documented inline (lines 1114-1117) — mutex held for full scope; opt-in gate keeps tests off default path.
6. **Cleanup ordering**: `clear_all_credentials(&["default", "sandbox"])` BEFORE restoring `JR_SERVICE_NAME` (lines 1120, 1122-1127). If a test creates entries and panics, the cleanup still runs (panic propagation happens after `with_test_keyring` returns to the caller via `panic::resume_unwind`).
- **NEW INVARIANT (NEW-INV-113)**: Keyring tests are isolated by THREE compounding mechanisms: process-level mutex on env, per-call unique service-name namespace, and post-test entry cleanup. Removing any one would risk cross-test contamination.

### 3.3 T-CLI-AUTH-R2: `cli/auth.rs` deep round 2

#### E-CLI-AUTH-R2-01 — `chosen_flow` vs `chosen_flow_for_profile` test pinning
**`chosen_flow` is `#[cfg(test)]`** (line 297) — production code uses ONLY `chosen_flow_for_profile`. Justification (lines 285-290 doc): `chosen_flow(&Config, _)` always reads the active profile, but production callers like `refresh_credentials` need the TARGET profile, not the active one. The test-only wrapper exists so the unit tests pinning the precedence rules don't have to construct a `ProfileConfig` directly — they pass a `&Config`.
- **5 tests pin behavior**:
  1. `chosen_flow_defaults_to_token_when_unset` (1217-1220): `auth_method = None` → Token.
  2. `chosen_flow_uses_token_for_explicit_api_token` (1223-1226): explicit `"api_token"` → Token.
  3. `chosen_flow_uses_oauth_when_config_says_so` (1228-1232): `"oauth"` → OAuth.
  4. `chosen_flow_oauth_override_wins_over_config` (1234-1238): override `oauth=true` flips Token to OAuth.
  5. `chosen_flow_for_profile_inspects_passed_profile_not_active` (1247-1280): **regression test** — active=Token + sandbox=OAuth, then verify `chosen_flow_for_profile(&sandbox, false) == OAuth` even though `chosen_flow(&config, false) == Token`. **Pins the BUG fix** for refreshing a non-active profile.
- **NEW INVARIANT (NEW-INV-114)**: The 5-test set **forms a precedence lattice**: Token (default) ← Override (oauth=true wins). The 5th test pins the architectural choice to look at the TARGET profile, not the active one — without it, `jr auth refresh --profile sandbox` against an active=api_token user would silently dispatch the wrong flow.

#### E-CLI-AUTH-R2-02 — `prepare_login_target` validator (lines 629-663)
**4-step contract**:
1. **Resolve target**: `Some(name) → validate_profile_name(name)? then name; None → active_profile_name`.
2. **Get-or-default profile entry**: `global.profiles.entry(target.clone()).or_default()` — creates an empty `ProfileConfig` if profile is brand-new.
3. **URL handling**:
   - `Some(url)` → `entry.url = Some(url.trim_end_matches('/').to_string());` (trailing-slash normalization).
   - `None && entry.url.is_none() && no_input` → **error**: `"--url required when the target profile has no URL configured"` (JrError::UserError, exit 64).
   - `None && entry.url.is_some()` → leave URL alone (re-login on existing profile).
   - `None && entry.url.is_none() && !no_input` → leave URL `None` (the prompt happens later in `login_oauth`/`login_token`).
4. **Default-profile auto-set**: if `global.default_profile.is_none()`, sets it to the new target. **First profile ever created becomes the default.**
- **NEW INVARIANT (NEW-INV-115)**: `prepare_login_target` is the SINGLE place where trailing-slash normalization happens for URLs — `entry.url = Some(url.trim_end_matches('/').to_string())`. A profile created via direct config-file edit with `https://x.example/` (trailing slash) would NOT be normalized.
- **NEW INVARIANT (NEW-INV-116)**: The first-profile auto-default is silent — no message printed. A user creating their first profile via `jr auth login --profile sandbox` would have `default_profile = "sandbox"` set. Subsequent `jr` commands (no `--profile`) would target "sandbox".
- **3 tests pin behavior** (lines 1708-1782): unknown profile name validates; URL trim; env-vs-active precedence.

#### E-CLI-AUTH-R2-03 — `refresh_success_payload` JSON shape (lines 805-814)
**Single-source-of-truth payload builder**:
```json
{
  "status": "refreshed",
  "auth_method": "<api_token|oauth>",
  "next_step": "<REFRESH_HELP_LINE>"
}
```
- **`REFRESH_HELP_LINE`** (line 803) — constant string: `"If prompted to allow keychain access, choose \"Always Allow\" so future commands run silently."`
- **NEW INVARIANT (NEW-INV-117)**: The `next_step` text is **literally the same** in JSON output and stderr eprintln — pinned by `refresh_payload_pins_token_shape` (1289-1301) which asserts `payload["next_step"].contains("Always Allow")`. JSON consumers + Table-mode users see identical guidance.

#### E-CLI-AUTH-R2-04 — `render_list_json` JSON shape (lines 1150-1170)
**Per-profile object**:
```json
{
  "name": "<name>",
  "url": "<url-or-null>",
  "auth_method": "<method-or-null>",
  "status": "<configured|unset>",
  "active": <bool>
}
```
- **`status`** is computed from `p.url.is_some()` — "configured" if URL present, "unset" otherwise.
- **`active: true`** appears on EXACTLY ONE entry (the active profile).
- **NEW INVARIANT (NEW-INV-118)**: `auth_method` is the RAW config value (`Option<String>`), passed through unmodified. A custom profile with `auth_method = "future_method"` would render `"auth_method": "future_method"` — no validation at JSON-output time.

#### E-CLI-AUTH-R2-05 — `peek_oauth_app_source` 3-state pure helper (lines 697-708)
**Pure function `peek_oauth_app_source_for_test(keychain_present, embedded_present) -> OAuthAppSource`**:
- `keychain_present == true` → `Keychain` (always wins).
- `embedded_present == true` → `Embedded` (fallback).
- Else → `None` (the explicit sentinel variant — NOT Rust's `Option::None`).
- **`peek_oauth_app_source` (production, line 677)**: calls `try_load_oauth_app_credentials()`. On `Err(_)` (locked keychain, permission denied), emits stderr warning AND falls through to `embedded_present` check. **Diverges from `resolve_refresh_app_credentials`**, which hard-errors on the same condition. Documented (lines 670-676) as defensible "non-blocking status surface" decision.
- **NEW INVARIANT (NEW-INV-119)**: `jr auth status` may display `embedded` when the keychain is merely temporarily inaccessible. A subsequent `jr auth refresh` (which uses `resolve_refresh_app_credentials`) would error with the keychain-failure detail — the status display and refresh have INCONSISTENT failure semantics by design.

#### E-CLI-AUTH-R2-06 — `auth status` strict-vs-permissive profile-existence check (lines 715-764)
**Two-mode strictness**:
- **Permissive**: empty `profiles` AND no `--profile` flag → eprintln "No profiles configured. Run \`jr init\`..." then `Ok(())`. Exit 0. Used by setup scripts / first-run probes.
- **Strict**: when user passes `--profile X` against unknown profile → `JrError::UserError("unknown profile: X; known: ...")` (exit 64). Matches strict behavior of switch/remove/logout.
- **NEW INVARIANT (NEW-INV-120)**: `jr auth status` is the ONLY auth subcommand with mode-conditional strictness. A first-run user can probe `jr auth status` without errors; an explicit `--profile typo` always errors.

### 3.4 T-CLI-LIST-R2: `cli/issue/list.rs` + `view.rs` + `comments.rs` deep round 2

#### E-CLI-LIST-R2-01 — Discovery: `view.rs` and `comments.rs` are sibling modules (NOT inside list.rs)
**Per CONV-ABS-4**: `cli/issue/list.rs` (1,083 LOC) contains `handle_list` only. The view/comments handlers are dedicated modules:
- `cli/issue/view.rs` (286 LOC) — `pub(super) async fn handle_view`. Dispatched via `mod.rs:42-43`: `IssueCommand::View {..} => view::handle_view(...)`.
- `cli/issue/comments.rs` (61 LOC) — `pub(super) async fn handle_comments`.
- **CLAUDE.md framing "list + view + comments"** is stale (or describes the spec, not the file).

#### E-VIEW-01 — `view.rs::handle_view` 4-stage flow (286 LOC)
1. **Field discovery**: `sp_field_id` + `team_field_id` + `cmdb_fields` (`get_or_fetch_cmdb_fields(client).await.unwrap_or_default()` — cmdb failures degrade to empty, not error).
2. **Fetch**: `client.get_issue(&key, &extra)` with `extra` composed via `helpers::compose_extra_fields(config, &cmdb_fields)`.
3. **Per-field asset enrichment** (lines 38-67): for each `(field_id, field_name)` in cmdb_fields, extract the assets, batch-enrich via `enrich_assets`, then redistribute by offset. **Two-pass to amortize a single `get_assets` call across N fields.**
4. **Output dispatch**: JSON injects enriched assets into `issue.fields.extra`; Table builds 13-row spec (Key, Summary, Type, Status, Priority, Assignee, Reporter, Created, Updated, Project, Labels, Parent, Links) + per-CMDB-field rows + Points + Team + Description.
- **NEW INVARIANT (NEW-INV-121)**: The view-table row order is **deterministic and field-set-driven** — Key/Summary first, Description last. CMDB field rows appear BETWEEN Links and Points; Team appears AFTER Points. Adding a new column to view requires deciding its position relative to this order.

#### E-VIEW-02 — `view::handle_view` Team-display fallback chain (lines 251-277)
**3 fallback paths** when team UUID resolves but cache lookup misses:
1. `Ok(Some(c))` AND `team` found in `c.teams` → name display.
2. `Ok(Some(c))` AND team NOT in cache → `"<uuid> (name not cached — run 'jr team list --refresh')"`.
3. `Ok(None)` (cache absent) → same message.
4. `Err(e)` → eprintln warning + `"<uuid> (team cache unreadable)"` display.
- **NEW INVARIANT (NEW-INV-122)**: A team UUID that resolves on the issue but is missing from cache renders a SPECIFIC fallback message pointing the user at `jr team list --refresh`. Generic "(unknown)" would be a regression — the actionable hint is the contract.

#### E-CMTS-01 — `comments.rs::handle_comments` conditional Visibility column (lines 16-58)
**3-state behavior**:
1. `has_visibility = comments.iter().any(|c| comment_visibility(c).is_some())` — whether ANY comment has `sd.public.comment` property.
2. `true` → 4-column table: `Author | Date | Visibility | Body`. Per-comment, `comment_visibility(c).unwrap_or("External")` (so a comment with property renders the value; a comment WITHOUT property among others WITH property defaults to "External").
3. `false` → 3-column: `Author | Date | Body`.
- **NEW INVARIANT (NEW-INV-123)**: The Visibility column is **conditional on the entire comment set**. A single internal comment in a 100-comment thread shows the column for ALL 100 (with "External" default for the others). Adds visual noise in proportion to "any internal comment exists" rather than "this comment is internal".

#### E-CLI-LIST-R2-02 — `format_issue_row` exact column ordering (`cli/issue/format.rs:21-82`)
**Fixed positions, conditional inclusion**:
| Position | Column | Always? | Source |
|---|---|---|---|
| 1 | `Key` | yes | `issue.key` |
| 2 | `Type` | yes | `issue.fields.issue_type.name` (default empty) |
| 3 | `Status` | yes | `issue.fields.status.name` |
| 4 | `Priority` | yes | `issue.fields.priority.name` |
| 5 | `Points` | only if `sp_field_id.is_some()` | `format_points` or `"-"` |
| 6 | `Assignee` | yes | `issue.fields.assignee.display_name` or `"Unassigned"` |
| 7 | `Team` | only if `team` arg passed | caller-resolved string |
| 8 | `Assets` | only if `assets.is_some()` | `format_linked_assets_short` |
| 9 | `Summary` | yes | `issue.fields.summary` |
- **`issue_table_headers` (lines 86-104)** mirrors this exactly with three booleans: `show_points, show_assets, show_team`. **Caller MUST pass matching values to row + headers** — mismatched booleans produce off-by-one columns.
- **NEW INVARIANT (NEW-INV-124)**: `format_issue_row` and `issue_table_headers` are an **invariant pair** — both must use the same 3-boolean signature in the same order. Refactoring one requires refactoring the other; there's no compile-time enforcement (both return `Vec<String>` / `Vec<&str>`).

#### E-CLI-LIST-R2-03 — `format_issue_rows_public` is a 4-arg-zeroed shim (lines 7-12)
**Always passes `(issue, None, None, None)`** — the public-API caller (used by integration tests + library consumers) gets the 6-column form (no Points/Team/Assets). **NEW INVARIANT (NEW-INV-125)**: The shim function is a STABLE-API FACADE for non-CLI consumers. CLI internals use `format_issue_row` directly; library consumers use `format_issue_rows_public`. Adding a column to the row would NOT break library consumers (they get the 6-column subset).

#### E-CLI-LIST-R2-04 — `compose_extra_fields` lives in `helpers.rs:189-204` (NOT list.rs)
**3-position composition**:
1. `config.global.fields.story_points_field_id` → push (if Some).
2. For each `(id, _)` in `cmdb_fields` → push.
3. `config.global.fields.team_field_id` → push (if Some).
- **Result is `Vec<String>`** (allocation-owned by caller; `view.rs` wraps as `Vec<&str>` via `iter().map(String::as_str)` for the `get_issue` API).
- **NEW INVARIANT (NEW-INV-126)**: The composed-fields ordering (story_points → cmdb_fields → team) is **stable** — callers that filter or post-process by index would be sensitive to reorders. **Note**: `compose_extra_fields` reads from `config.global.fields.*` (legacy shape) — see NEW-INV-12 verification below.

#### E-CLI-LIST-R2-05 — `--no-color` mechanism (CORRECTION TO R2 NEW-INV E-CMD-06)
**Two independent triggers, single sink** (per `main.rs:13-15`):
- CLI flag: `cli.no_color: bool` (clap-parsed from `--no-color`, see `cli/mod.rs:32`).
- Env var: `std::env::var("NO_COLOR").is_ok()`.
- Either → `colored::control::set_override(false)`.
- **NEW INVARIANT (NEW-INV-127)** (corrects R2's CONV-ABS-3): `--no-color` is a startup-time globally-applied override; the `colored` crate's `Colorize` methods (`.green()`, `.red()`, `.bold()`, `.dimmed()`) all check the global override. There is NO per-command color toggling — the entire process is colorless or color-enabled.

### 3.5 T-CLI-ASSETS-R2: `cli/assets.rs` deep round 2

#### E-CLI-ASSETS-R2-01 — Operation catalog (6 sub-commands, dispatched at line 14-65)
- `Search { query, limit, attributes }` → `handle_search` (line 67).
- `View { key, no_attributes }` → `handle_view` (line 216).
- `Tickets { key, limit, open, status }` → `handle_tickets` (line 372).
- `Schemas` → `handle_schemas` (line 492).
- `Types { schema }` → `handle_types` (line 524, with optional `--schema` partial-match).
- `Schema { schema }` → handle_schema (calls `resolve_schema` for the partial-match — verifies the requested schema EXISTS).
- All 6 share `workspace_id` resolved from `client.get_workspace_id().await?` at the dispatcher (top of `handle`).

#### E-CLI-ASSETS-R2-02 — `resolve_schema` 4-branch fallback (lines 444-490)
**Try ID exact match FIRST, then partial-match by name**:
1. `schemas.iter().find(|s| s.id == input)` → return immediately. **NEW**: ID is treated as case-SENSITIVE exact match (assets schema IDs are numeric strings). 
2. Otherwise: `partial_match(input, &names)` against schema NAMES.
3. Branches: `Exact(name)` → find-by-name; `ExactMultiple(_)` → emit duplicates with IDs `(id: ...)`, suggest using the schema ID. `Ambiguous(matches)` → list partial matches. `None(all)` → list all available.
- **NEW INVARIANT (NEW-INV-128)**: `resolve_schema` is the ONLY resolver in the codebase that accepts BOTH IDs AND names with this fallback. Issue keys are checked separately (key-format validation); asset object keys use `resolve_object_key`; team names check UUID format separately. The schema resolver is uniquely lenient.
- **NEW INVARIANT (NEW-INV-129)**: `ExactMultiple` for schemas emits `(id: <id>)` in the duplicates list — the same convention as `resolve_resolution_by_name` (Round 2 NEW-INV-50). Two resolvers use this pattern; team/user/asset/link-type resolvers do NOT.

#### E-CLI-ASSETS-R2-03 — `handle_view` attribute-filter divergence (lines 226-298)
**Two filter-set choices based on output mode**:
- **JSON mode** (lines 233-234): `retain(|a| !system && !hidden)`. **Keeps `label` attributes** for programmatic consumers.
- **Table mode** (lines 267-271): `retain(|a| !system && !hidden && !label)`. **Drops `label` attributes** because the `Name` row already shows the label, so showing it again would duplicate.
- **Sort**: both modes `sort_by_key(|a| a.object_type_attribute.position)`.
- **NEW INVARIANT (NEW-INV-130)**: JSON output is INTENTIONALLY a superset of table output for asset attributes. A consumer scripting against `jr assets view FOO-1 --output json` sees the label attributes; a human running `jr assets view FOO-1` does not. This is a deliberate consumer-vs-human contract divergence.

#### E-CLI-ASSETS-R2-04 — Attribute display_value vs value coalesce (lines 282-285)
**Per-value emission**: `v.display_value.clone().or_else(|| v.value.clone()).unwrap_or_default()`.
- `display_value` is the human-readable form (e.g., for a User attribute: the user's display name).
- `value` is the raw form (e.g., the user's accountId).
- **`unwrap_or_default()` → empty string** when both are None.
- **One row per value**: a multi-value attribute (e.g., a User-list) emits N rows with the same Attribute name, distinct values.
- **NEW INVARIANT (NEW-INV-131)**: The CLI prefers `display_value` over `value`; a custom attribute renderer that returns `value=null, display_value=null` shows an EMPTY string in the table cell (NOT a placeholder dash). Distinct from the `format_issue_row` `"-"` fallback for missing fields.

#### E-CLI-ASSETS-R2-05 — `format_inline_attributes` 3-layer fallback (lines 182-216)
**Inline format for search results**: `Name: Value, Name: Value, ...`.
- **Pair construction** (line 187-196): each attribute paired with its `objectTypeAttribute` definition (or `None` for unknown — graceful degradation).
- **Position sort**: known attributes by `position`; unknown appended at end.
- **Per-attribute name**: from `objectTypeAttribute.name` if present, else `object_type_attribute_id` (the raw ID surfaces if metadata lookup failed).
- **NEW INVARIANT (NEW-INV-132)**: When CMDB metadata fails to load, the search results gracefully fall back to showing raw attribute IDs as labels rather than dropping the attributes — the user sees `customfield_12345: <value>` instead of nothing.

### 3.6 T-API-AUTH-R2: `api/auth.rs` line-by-line deepening

#### E-API-AUTH-R2-01 — `read_keyring_optional` 3-state discrimination (lines 181-187)
**Pin point**: `entry(key)?.get_password()` matches:
- `Ok(v)` → `Some(v)` (entry exists, password retrieved).
- `Err(keyring::Error::NoEntry)` → `None` (genuinely absent — the only "not present" path).
- `Err(e)` → propagate (locked keychain, permission denied, platform error).
- **NEW INVARIANT (NEW-INV-133)**: This helper EXISTS specifically because the naive `entry.get_password().ok()` collapses ALL errors to absent. Doc comment (lines 173-180) explains: collapsing would silently trigger fallbacks (legacy migration, generic "no token") and hide real problems. **Architectural pattern**: error-discrimination helpers are first-class entities.

#### E-API-AUTH-R2-02 — `clear_profile_creds` vs `clear_all_credentials` semantic split

| Aspect | `clear_profile_creds` | `clear_all_credentials` |
|---|---|---|
| Signature | `(profile: &str)` | `(profiles: &[&str])` |
| OAuth tokens cleared | ONE profile's `<profile>:oauth-*-token` | EVERY listed profile's `<profile>:oauth-*-token` |
| Shared keys cleared | NEVER (email/api-token preserved) | ALWAYS (email/api-token wiped) |
| OAuth app creds cleared | NEVER | ALWAYS (oauth_client_id/secret wiped) |
| Legacy flat OAuth keys | ONLY if `profile == "default"` | ONLY if `profiles.contains("default")` |
| Used by | `jr auth logout`, `jr auth refresh --oauth` | `jr auth refresh --token` (and historically the keychain-ACL recovery path) |

- **NEW INVARIANT (NEW-INV-134)**: `clear_profile_creds("default")` deletes legacy flat keys to prevent lazy-migration resurrection (Round 1 NEW-INV recurrence avoidance). Pinned by `clear_profile_creds_default_also_clears_legacy_flat_keys` test (line 1187-1216).
- **NEW INVARIANT (NEW-INV-135)**: `clear_all_credentials` only wipes legacy flat keys when **explicitly clearing "default"** — protects a user mid-migration whose `default` profile has unmigrated tokens that "sandbox" refresh shouldn't touch. Pinned by `clear_profile_creds_non_default_leaves_legacy_keys_alone` (1223-1240) and the `clear_all_credentials` `if profiles.contains(&"default")` guard (line 339).
- **NEW INVARIANT (NEW-INV-136)**: Both functions aggregate failures (`Vec<String>`) rather than fail-fast — an `Err` from `delete_credential` on one key doesn't stop the loop. **Architecturally significant**: partial-clear failures are reported as a single aggregated error message so the user sees ALL stale entries, not just the first.

#### E-API-AUTH-R2-03 — `try_load_oauth_app_credentials` single-pass design (lines 255-262)
**Motivation (lines 244-254 doc)**: combines probe + load into one keychain read. The naive `probe_oauth_app_credentials()? .then(load_oauth_app_credentials())` issues 4 keychain reads (2 each); on macOS this can multiply the user-visible "Allow Access" prompts. The single-pass version does 2 reads (one each for id and secret), de-duplicating prompts.
- **NEW INVARIANT (NEW-INV-137)**: Resolver-chain callers MUST use `try_load_oauth_app_credentials`, NOT `probe + load`. Documented at the resolver site. **Performance-correctness invariant**: violation regresses keychain prompt UX.

#### E-API-AUTH-R2-04 — Keyring round-trip test catalog (10 tests, line numbers)

| # | Line | Test name | Scenario |
|---|---:|---|---|
| 1 | 1132 | `store_and_load_per_profile_oauth_tokens_round_trip` | Two profiles store + load distinct token pairs |
| 2 | 1147 | `load_oauth_tokens_returns_err_for_missing_profile` | Missing profile → Err |
| 3 | 1155 | `lazy_migration_legacy_flat_keys_for_default_profile` | Legacy flat keys → migrate on `load_oauth_tokens("default")` |
| 4 | 1187 | `clear_profile_creds_default_also_clears_legacy_flat_keys` | Logout-default wipes legacy keys |
| 5 | 1223 | `clear_profile_creds_non_default_leaves_legacy_keys_alone` | Logout-sandbox preserves legacy keys |
| 6 | 1251 | `load_oauth_tokens_errors_on_partial_state` | Partial (Some, None) state → explicit Err (NOT collapse to "absent") |
| 7 | 1273 | `load_oauth_tokens_default_partial_recovers_from_legacy` | Interrupted-migration recovery: partial namespaced + complete legacy → load legacy |
| 8 | 1325 | `lazy_migration_does_not_fire_for_non_default_profile` | Non-default profile NEVER inherits legacy keys |
| 9 | 1349 | `resolve_refresh_app_credentials_prefers_keychain` | BYO user keeps their app creds across refresh |
| 10 | 1364 | `resolve_refresh_app_credentials_errors_when_both_absent` | No keychain + no embedded → Err with "embedded" hint |

Plus 1 non-keyring test that DOESN'T require the gate: `fixed_port_strategy_eaddrinuse_friendly_error` (1378).

- **NEW INVARIANT (NEW-INV-138)**: The 10 keyring tests EACH wrap their body in `with_test_keyring(|| { ... })` so the global `JR_SERVICE_NAME` mutex serializes parallel test execution AND each test gets a unique service-name namespace. Removing the `with_test_keyring` wrapper from any one test would race-corrupt the others.

### 3.7 T-CACHE-R2: `cache.rs` deep round 2

#### E-CACHE-R2-01 — `with_temp_cache` test scaffolding (lines 364-379)
**5-step orchestration**:
1. **Lock**: `ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner())` (poisoned-recovery; differs from `KEYRING_TEST_ENV_MUTEX` only in service-name semantics).
2. **TempDir**: `TempDir::new().unwrap()` (auto-cleanup on drop).
3. **Set env**: `unsafe { std::env::set_var("XDG_CACHE_HOME", dir.path()) }`. SAFETY justification (lines 369-371): "ENV_MUTEX serialises all tests that touch XDG_CACHE_HOME; the variable is only read inside cache functions called within this lock, so no concurrent env access occurs."
4. **Run**: `std::panic::catch_unwind(AssertUnwindSafe(f))`.
5. **Cleanup + propagate**: `unsafe { std::env::remove_var("XDG_CACHE_HOME") }`; drop guard; `panic::resume_unwind` if test panicked.
- **NEW INVARIANT (NEW-INV-139)**: `cache.rs` `ENV_MUTEX` is a SEPARATE mutex from `cli/auth.rs::ENV_LOCK` and `api/auth.rs::KEYRING_TEST_ENV_MUTEX`. **Three independent process-global mutexes** in the codebase, each guarding a different env-var namespace (`XDG_CACHE_HOME`, generic test env, `JR_SERVICE_NAME`). They don't deadlock because no test holds two simultaneously.

#### E-CACHE-R2-02 — `with_temp_cache` test count: 27 #[test] in `cache.rs`
**`grep -c '#[test]'` = 27**, of which 20 wrap in `with_temp_cache(|| { ... })` (per `grep with_temp_cache | head -20`). The remaining 7 are pure-function tests (no filesystem access). **NEW INVARIANT (NEW-INV-140)**: The pattern "wrap any test that calls a cache_dir-dependent function in `with_temp_cache`" is enforced by code review only — there's no compiler check. A new test that forgets the wrapper would race with parallel tests and intermittently fail.

#### E-CACHE-R2-03 — Cross-mutex non-interaction
- `cache.rs` `ENV_MUTEX` (`XDG_CACHE_HOME`).
- `api/auth.rs` `KEYRING_TEST_ENV_MUTEX` (`JR_SERVICE_NAME`).
- `cli/auth.rs` `ENV_LOCK` (generic env mutation in `EnvGuard`).
- `config.rs` `ENV_MUTEX` (per `config.rs:474` — `JR_PROFILE`, `JR_BASE_URL`, etc.).
- **NEW INVARIANT (NEW-INV-141)**: Tests that need TWO env namespaces (e.g., a cache test that ALSO sets `JR_PROFILE`) cannot trivially compose mutexes — would require nested locking with consistent ordering to avoid deadlocks. The codebase AVOIDS this by NOT having such cross-namespace tests; if one is needed, the design pattern would require careful ordering.

### 3.8 T-CONFIG-R2: `config.rs` migration field-by-field + NEW-INV-12 verification

#### E-CONFIG-R2-01 — `migrate_legacy_global` 7-field copy (lines 150-178)
**Migration is no-op IF**:
- `global.profiles` is non-empty (already in new shape) — line 151.
- ALL 7 legacy fields are None (nothing to migrate) — lines 155-162.

**Otherwise constructs a `ProfileConfig` from these 7 fields** (lines 166-174):
1. `url ← global.instance.url`
2. `auth_method ← global.instance.auth_method`
3. `cloud_id ← global.instance.cloud_id`
4. `org_id ← global.instance.org_id`
5. `oauth_scopes ← global.instance.oauth_scopes`
6. `team_field_id ← global.fields.team_field_id`
7. `story_points_field_id ← global.fields.story_points_field_id`

**Inserts into `global.profiles["default"]`** (line 175); sets `default_profile = Some("default")` (line 176).

- **NEW INVARIANT (NEW-INV-142)**: Migration COPIES (not moves) — legacy `global.instance.*` and `global.fields.*` are LEFT POPULATED in memory. They will still be SERIALIZED to disk if the save path emits them, BUT the save path (lines 442-443) ONLY copies `default_profile` and `profiles` — so legacy fields fall off disk on the next `save_global` call. **Two-phase migration**: in-memory still backward-compatible, on-disk new-shape-only.

#### E-CONFIG-R2-02 — VERIFIED: NEW-INV-12 (multi-profile fields bug) is a real defect
**All `config.global.fields.*` reads** found via grep (12 read sites):

| File | Line | Field | Code |
|---|---:|---|---|
| `cli/board.rs` | 192 | team | `let team_field_id = config.global.fields.team_field_id.as_deref();` |
| `cli/sprint.rs` | 232 | story_points | `let sp_field_id = config.global.fields.story_points_field_id.as_deref();` |
| `cli/sprint.rs` | 233 | team | `let team_field_id = config.global.fields.team_field_id.as_deref();` |
| `cli/issue/list.rs` | 147 | story_points | (read) |
| `cli/issue/list.rs` | 148 | team | (read) |
| `cli/issue/helpers.rs` | 43 | team | `if let Some(id) = &config.global.fields.team_field_id { ... }` |
| `cli/issue/helpers.rs` | 194 | story_points | `if let Some(sp) = config.global.fields.story_points_field_id.as_deref() { ... }` (compose_extra_fields) |
| `cli/issue/helpers.rs` | 200 | team | (compose_extra_fields) |
| `cli/issue/view.rs` | 28 | story_points | (read) |
| `cli/issue/view.rs` | 29 | team | (read) |
| `cli/issue/helpers.rs` | 777-778 | both | (TEST setup writes only) |

**ALL production reads** target `config.global.fields.*` (the LEGACY shape), NOT `config.active_profile().team_field_id` etc. After migration, the legacy shape is still in-memory (per E-CONFIG-R2-01) AND the new-shape `[profiles.default]` has the fields too — but if a SECOND profile is added, its custom `team_field_id` is IGNORED because all readers go to the legacy shape, which only has the DEFAULT profile's values.

- **NEW INVARIANT (NEW-INV-143)** (verifies R1's NEW-INV-12 with stronger evidence): A multi-profile setup where `[profiles.sandbox] team_field_id = "Y"` differs from `[profiles.default] team_field_id = "X"` will incorrectly use `"X"` (or the legacy `[fields].team_field_id` value) for ALL profiles. **POTENTIAL BUG / Design Limitation**: there is no migration path for sandbox-specific custom-field IDs. The CLAUDE.md "Multi-profile boundary" gotcha says cache reads MUST take `profile: &str` — but custom-field IDs do NOT yet take profile. **Pass 4 deepening trigger**: this is a correctness boundary that broad/R1/R2 didn't surface.

#### E-CONFIG-R2-03 — `Config::load` vs `load_with` vs `load_lenient` vs `load_lenient_with`
**4-method facade pattern** (lines 192-218):
- `load() → load_with(None)` — strict, no CLI override.
- `load_with(cli_profile) → load_inner(cli_profile, true)` — strict, CLI override.
- `load_lenient() → load_lenient_with(None)` — lenient (skips active-profile-existence check).
- `load_lenient_with(cli_profile) → load_inner(cli_profile, false)` — lenient + CLI override.
- **`load_lenient` is used by EXACTLY ONE caller**: `jr auth login`. Justification (lines 207-209 doc): login legitimately creates profiles on demand.
- **NEW INVARIANT (NEW-INV-144)**: Outside `jr auth login`, every config load is STRICT — typos in `--profile` surface as `JrError::UserError("unknown profile: X; known: ...")` rather than silent fallback to active. The 4-method facade is a **policy enforcement boundary**.

#### E-CONFIG-R2-04 — `save_global` over-write semantics (lines 425-446)
**3-step pattern**:
1. **Read disk fresh** (file-only, NO env overlay) into `to_save: GlobalConfig`. If file doesn't exist, `GlobalConfig::default()`.
2. **Overlay only multi-profile fields**: `to_save.default_profile = self.global.default_profile.clone(); to_save.profiles = self.global.profiles.clone();`. Other fields (e.g., `defaults.output`) preserve from disk.
3. **Write `to_save`**.
- **NEW INVARIANT (NEW-INV-145)**: `save_global` is a **selective** write — `defaults` is preserved from disk; `instance` and `fields` (legacy shape) are NOT propagated from `self.global` to disk. So in-memory mutations to `global.fields.*` are NOT persisted by `save_global`. (See test setup writes at `helpers.rs:777-778` which only affect in-memory state.)

### 3.9 T-EMBEDDED: `auth_embedded.rs` test catalog (lines 138-249)

**6 tests pin embedded-OAuth invariants**:
1. `decode_round_trip_known_plaintext` (143-155): XOR encode → XOR decode → original plaintext.
2. `build_embedded_app_none_when_constants_unset` (157-161): all-None inputs → None output.
3. `build_embedded_app_returns_decoded_when_all_set` (163-178): all-Some → decoded `EmbeddedOAuthApp`.
4. `build_embedded_app_none_when_any_constant_missing` (180-189): individual None → None output (3 sub-cases).
5. `build_embedded_app_rejects_empty_inputs` (194-201): empty id or empty xor → None.
6. `embedded_oauth_app_is_none_in_default_test_build` (207-215): test builds without env vars → None.
7. `embedded_oauth_app_debug_redacts_secret` (220-239): client_secret never in Debug output.
8. `embedded_oauth_app_present_is_false_in_default_test_build` (243-249): presence-check false in test build.
- (8 tests total — corrects spot-count.)
- **NEW INVARIANT (NEW-INV-146)**: The 8-test set is a **defense-in-depth contract** — tests #6 and #8 specifically pin the "test builds must NOT have embedded credentials" property. If CI ever leaks `JR_BUILD_OAUTH_CLIENT_*` into test runs (e.g., misconfigured workflow), tests #6/#8 fire BEFORE any test would attempt to use them.

### 3.10 T-OBS: `observability.rs` (NEW — neither broad nor R1/R2 catalogued)

#### E-OBS-01 — `log_parse_failure_once` shared helper (39 LOC)
**Single function**: `pub(crate) fn log_parse_failure_once(flag: &AtomicBool, site: &str, iso: &str, verbose: bool)`.
- **Behavior**: `if verbose && !flag.swap(true, Ordering::Relaxed) { eprintln!(...); }`.
- **Short-circuit ordering** (test at line 26-38 pins): `verbose` check FIRST, `flag.swap` SECOND. Reversing would burn the gate flag on non-verbose runs and suppress the first verbose log later.
- **Used by 3 sites** (formal NEW-PAT-01 verification): `types/jira/issue.rs:103`, `cli/issue/format.rs:119`, `cli/issue/changelog.rs:269`.
- **NEW INVARIANT (NEW-INV-147)**: A `static LOGGED: AtomicBool = AtomicBool::new(false);` per call-site (NOT shared across sites). Each formatter has its OWN once-per-process gate. So a malformed changelog timestamp + a malformed comment timestamp produce two separate verbose logs, not one.

#### E-OBS-02 — Module is OUT OF CLAUDE.md tree (39 LOC)
**`observability.rs`** is at `/src/observability.rs` but NOT listed in CLAUDE.md's tree diagram. Doc comment (lines 1-7): "Lightweight observability primitives shared across commands. Intentionally tiny: the project has no tracing/log crate, and a single `--verbose`-gated `eprintln!` is the established pattern (see `src/api/client.rs` for HTTP-request logging). Expand to a real tracing layer when there is cross-subsystem need."
- **NEW INVARIANT (NEW-INV-148)**: The codebase has **NO** tracing/structured logging crate (no `tracing`, no `log`). All observability is `eprintln!` gated by `verbose: bool` on `JiraClient`. Adding structured logging would be a significant infrastructure decision; the `observability.rs` module exists as a stub for that future expansion.

### 3.11 T-ERROR: `error.rs` JrError construction sites (broad pass §2a.2)

#### E-ERROR-01 — 11 variants confirmed, exit codes pinned
**Verified 11 variants** at `error.rs:4-49`:
1. `NotAuthenticated` → exit 2
2. `InsufficientScope { message }` → exit 2
3. `NetworkError(String)` → exit 1 (default)
4. `ApiError { status: u16, message: String }` → exit 1 (default)
5. `ConfigError(String)` → exit 78
6. `UserError(String)` → exit 64
7. `Internal(String)` → exit 1 (default)
8. `Interrupted` → exit 130
9. `Http(reqwest::Error)` (transparent #[from]) → exit 1
10. `Io(std::io::Error)` (transparent #[from]) → exit 1
11. `Json(serde_json::Error)` (transparent #[from]) → exit 1

- **8 unit tests** in `error.rs:64-136` pin: ConfigError=78, UserError=64, Internal=1 (with display passthrough), InsufficientScope=2 with display containing "Insufficient token scope", "write:jira-work", "OAuth 2.0", and the issue-185 link.
- **NEW INVARIANT (NEW-INV-149)**: `Internal` was added with explicit exit code 1 (NOT 64 / 78) so callers downcasting to `JrError` can distinguish "we have a bug" from "user did something wrong" or "config is broken". The doc comment (lines 30-34) is normative: prefix call-site messages with "Internal error:".
- **NEW INVARIANT (NEW-INV-150)**: Variants 9-11 (Http, Io, Json) are `#[error(transparent)]` — display-passes-through-to-source-error AND `#[from]`-conversion lets `?` operate on these external error types. **Code-organization invariant**: all `?` propagation from `reqwest::Error`, `io::Error`, `serde_json::Error` arrives in `JrError` via these three.

### 3.12 T-OUTPUT: `output.rs` (76 LOC) — NITPICK confirmation

#### E-OUTPUT-R3-01 — 5 functions, 2 unit tests
- `render_table(headers, rows)` — comfy-table `UTF8_FULL_CONDENSED` preset with `ContentArrangement::Dynamic`.
- `render_json(data)` — `serde_json::to_string_pretty`.
- `print_output(format, headers, rows, json_data)` — dispatcher that handles empty-rows specially: `"No results found."` (dimmed).
- `print_success(msg)` — `eprintln!("{}", msg.green())`.
- `print_warning(msg)` — `eprintln!("warning: {msg}")` (NOT colored).
- `print_error(msg)` — `eprintln!("{}: {}", "Error".red().bold(), msg)`.
- **NEW INVARIANT (NEW-INV-151)**: `print_warning` is the ONLY of the 3 print-helpers that does NOT use color. Inconsistency vs `print_success` (green) and `print_error` (red bold). **Could be a UX inconsistency**, but it's pinned in tests effectively (tests don't assert on color). Round 3 NITPICK candidate.
- **NEW INVARIANT (NEW-INV-152)**: `print_output` short-circuits empty rows BEFORE any header rendering — so `jr issue list` with zero matches prints `"No results found."` (dimmed) instead of an empty table with headers. Distinct from JSON mode which prints `[]`.

### 3.13 T-INTEGRATION-TESTS: `tests/common/fixtures.rs` (446 LOC) — partial catalog

#### E-FIX-01 — Test fixture builder catalog (32+ functions identified by grep)
**Categories**:
- **User**: `user_response`, `user_search_response`.
- **Issue**: 8+ builders (`issue_response`, `issue_response_with_points`, `issue_response_with_assignee`, `issue_response_with_standard_fields`, `issue_response_with_labels_parent_links`, `issue_response_with_team`, `issue_with_links_response`, `create_issue_response`).
- **Issue search**: `issue_search_response`, `issue_search_response_with_next_page`, `approximate_count_response`.
- **Transitions**: `transitions_response`, `transitions_response_with_status`.
- **Errors**: `error_response`.
- **GraphQL teams**: `graphql_org_metadata_json`, `teams_list_json`.
- **Projects**: `project_search_response`, `project_response`, `project_statuses_response`.
- **Boards**: `board_response`, `board_list_response`, `board_config_response`.
- **Sprints**: `sprint`, `sprint_list_response`, `sprint_issues_response`.
- **Multi-project assignee**: `multi_project_user_search_response`.
- **Fields**: `fields_response_with_story_points`.
- **Links**: `link_types_response`.
- **NEW INVARIANT (NEW-INV-153)**: Every Atlassian REST endpoint shape used by the codebase has a corresponding fixture builder in `tests/common/fixtures.rs`. The fixtures are the **specification-by-example** for the Atlassian API surface — Pass 3 BCs can derive contracts from these directly.

---

## 4. Sub-pass 2b deepening: behavioral

### 4.1 ADF text-rendering whitespace state machine

```
adf_to_text(adf)
    │
    ├── render_doc(adf)
    │     for child in content: render_node(child)
    │
    ├── render_node(paragraph) → render_children + push '\n' (one trailing newline per paragraph)
    ├── render_node(heading) → "# ... " + render_children + '\n'
    ├── render_node(rule) → "---\n" (3 dashes + newline)
    ├── render_node(hardBreak) → '\n'
    ├── render_node(table) → render_children + '\n'
    │
    ├── render_node(blockquote):
    │     ├── start = output.len()
    │     ├── render_children → temp output
    │     ├── split_off + line-by-line prefix:
    │     │     ├── trailing-empty lines: TRIM (would produce dangling "> ")
    │     │     ├── empty middle line: emit ">" (no trailing space)
    │     │     └── non-empty: emit "> " + line
    │     └── final '\n' appended
    │
    ├── render_node(codeBlock) → "```<lang>\n" + render_children + "\n```\n"
    │
    ├── render_node(table):
    │     for tableRow:
    │       ├── "| " + cells joined by " | " + " |\n"
    │       └── if any tableHeader: "| --- | --- ... |\n"
    │
    └── finish() → output.trim_end() (strip trailing \n)
```

**Whitespace-collapse rules**:
- `paragraph` always emits `'\n'` trailer.
- `hardBreak` emits `'\n'`.
- Two consecutive `hardBreak` → blank line: `\n\n`.
- Trailing whitespace stripped at `finish()`.
- Tables: cells use ' ' join (single space), pipe-escape pipes, newline-collapse newlines.
- Blockquote: `> ` prefix per line; bare `>` for blank lines; trailing-empty trimmed.

### 4.2 OAuth login dispatch state machine

```
handle_login(LoginArgs)
    │
    ├── Config::load_lenient_with(args.profile)
    ├── prepare_login_target(global, args.profile, args.url, no_input, active)
    │     ├── target = args.profile.unwrap_or(active)
    │     ├── validate_profile_name(target)
    │     ├── entry = global.profiles.entry(target).or_default()
    │     ├── if args.url is Some: entry.url = trim_end_matches('/')
    │     ├── if no_input AND no url AND no entry.url: ERROR
    │     └── if global.default_profile is None: set to target
    │
    ├── chosen_flow_for_profile(&target_profile, args.oauth)
    │     ├── if oauth_override: AuthFlow::OAuth
    │     ├── auth_method == "oauth": AuthFlow::OAuth
    │     └── else: AuthFlow::Token
    │
    ├── match flow:
    │     ├── Token → login_token(target, email, token, no_input)
    │     │     └── stores api-token + email in shared keychain
    │     │
    │     └── OAuth → login_oauth(target, client_id, client_secret, no_input)
    │           ├── resolve OAuth app:
    │           │     ├── flag (--client-id/--client-secret)
    │           │     ├── env (JR_OAUTH_CLIENT_*)
    │           │     ├── keychain (BYO stored creds)
    │           │     ├── embedded (build-time XOR)
    │           │     └── prompt (interactive fallback)
    │           ├── browser-redirect to authorize URL (with %20-escaped scopes, exact-match redirect_uri)
    │           ├── local listener (fixed-port 53682 OR dynamic-port for BYO)
    │           ├── exchange code for tokens
    │           ├── fetch accessible-resources → cloud_id, site_url, site_name
    │           └── store namespaced <profile>:oauth-* tokens
    │
    └── output: Table → success message; JSON → minimal success payload
```

### 4.3 Embedded XOR pipeline state machine

```
[BUILD TIME (build.rs)]
    │
    ├── env JR_BUILD_OAUTH_CLIENT_ID
    ├── env JR_BUILD_OAUTH_CLIENT_SECRET
    │
    ├── if either missing/empty:
    │     emit:
    │       const EMBEDDED_ID: Option<&str> = None;
    │       const EMBEDDED_SECRET_XOR: Option<&[u8]> = None;
    │       const EMBEDDED_SECRET_KEY: Option<&[u8; 32]> = None;
    │     (this is the FORK / LOCAL-BUILD branch)
    │
    └── if both present:
          ├── generate_xor_key() → 32 bytes
          │     ├── unix: /dev/urandom
          │     └── windows: BCryptGenRandom (BCRYPT_USE_SYSTEM_PREFERRED_RNG)
          ├── XOR each secret byte with key[i % 32]
          └── emit constants with Some(...) values

[RUNTIME (auth_embedded.rs)]
    │
    ├── embedded_oauth_app_present() → cheap probe; NO decode
    │     (used by jr auth status — defense in depth)
    │
    └── embedded_oauth_app() → OnceLock-cached
          ├── build_embedded_app(EMBEDDED_ID, EMBEDDED_SECRET_XOR, EMBEDDED_SECRET_KEY)
          │     ├── all None → return None
          │     ├── empty id or empty xor → return None
          │     ├── decode xor → secret_bytes ^ key[i % 32]
          │     ├── if decoded String is empty → return None
          │     └── return Some(EmbeddedOAuthApp { client_id, client_secret })
          └── cache forever in OnceLock (refresh tokens need it)
```

---

## 5. Newly-discovered entities & invariants (NOT in broad / R1 / R2)

### Entities (R3-NN, 31 new this round)

**ADF deepening**:
- E-ADF-R2-01..14 (table tests, allowlists, sanitization, code wrapping, mark composition, drop semantics, hardBreak, ordered list attrs)

**OAuth deepening**:
- E-OAUTH-R2-01..07 (XOR pipeline, debug redaction, no-decode probe, redirect URI tests, authorize URL, state CSPRNG, mutex semantics)

**`cli/auth.rs` deepening**:
- E-CLI-AUTH-R2-01..06 (chosen_flow tests, prepare_login_target, refresh_success_payload, render_list_json, peek_oauth_app_source, status strict-vs-permissive)

**`cli/issue/list.rs` + `view.rs` + `comments.rs` deepening**:
- E-VIEW-01..02 (handle_view 4-stage, team-display fallback)
- E-CMTS-01 (handle_comments conditional Visibility column)
- E-CLI-LIST-R2-01..05 (file split discovery, format_issue_row, format_issue_rows_public, compose_extra_fields location, no-color mechanism)

**`cli/assets.rs` deepening**:
- E-CLI-ASSETS-R2-01..05 (operation catalog, resolve_schema, attribute filter divergence, value coalesce, format_inline_attributes)

**`api/auth.rs` deepening**:
- E-API-AUTH-R2-01..04 (read_keyring_optional 3-state, clear_* split, try_load_oauth_app_credentials, 10-test catalog)

**`cache.rs` deepening**:
- E-CACHE-R2-01..03 (with_temp_cache, test count, cross-mutex)

**`config.rs` deepening**:
- E-CONFIG-R2-01..04 (migrate_legacy_global field-by-field, NEW-INV-12 verification, 4-load facade, save_global semantics)

**Embedded OAuth tests**:
- (covered by E-OAUTH-R2-01..03)

**`observability.rs` (orphan module)**:
- E-OBS-01..02 (log_parse_failure_once helper, out-of-CLAUDE.md tree)

**`error.rs`**:
- E-ERROR-01 (11 variants, 8 tests, exit code policy)

**`output.rs`**:
- E-OUTPUT-R3-01 (5 functions, color inconsistency)

**Integration-test fixtures**:
- E-FIX-01 (32+ fixture builders across 12 categories)

### Invariants (NEW-INV-93..NEW-INV-153, 61 new this round)

| # | File | Invariant |
|---|---|---|
| NEW-INV-93 | adf.rs | listItem allowlist (9 types) ≠ tableCell allowlist (6 types); cells cannot nest tables |
| NEW-INV-94 | adf.rs | wrap_inlines_as_blocks empty-input → single empty paragraph (ADF schema requirement) |
| NEW-INV-95 | adf.rs | Table separator emits per-row when ANY cell is tableHeader; mixed rows still emit separator |
| NEW-INV-96 | adf.rs | sanitize_table_cell_text: newline-replace BEFORE pipe-escape (order matters) |
| NEW-INV-97 | adf.rs | wrap_code_span: longest_run+1 backticks; pad with spaces if content begins/ends with backtick |
| NEW-INV-98 | adf.rs | apply_marks: code mark ALWAYS innermost regardless of array order |
| NEW-INV-99 | adf.rs | Unknown mark types pass through with text only (no syntax) |
| NEW-INV-100 | adf.rs | Unknown leaf node: silent drop. Unknown container: recurse into content |
| NEW-INV-101 | adf.rs | mention/emoji/inlineCard/media* are LOSSY in ADF→text — JSON output preserves, table drops |
| NEW-INV-102 | adf.rs | finish() trim_end strips trailing whitespace (incl. trailing hardBreaks) |
| NEW-INV-103 | adf.rs | Blockquote internal blank line emits bare ">" (no trailing space) |
| NEW-INV-104 | adf.rs | orderedList omits attrs.order when start==1 (saves bytes; avoids redundant attrs) |
| NEW-INV-105 | build.rs | XOR key generated per-build, not per-binary — different ciphertexts across builds |
| NEW-INV-106 | auth_embedded.rs | build_embedded_app rejects empty id/xor/secret — empty CI vars don't ship |
| NEW-INV-107 | auth_embedded.rs | EmbeddedOAuthApp::Debug redacts client_secret as `<redacted>` literal |
| NEW-INV-108 | auth_embedded.rs | embedded_oauth_app_present() does NOT decode — read-only probe |
| NEW-INV-109 | api/auth.rs | RedirectUriStrategy::FixedPort uses literal "127.0.0.1" (NOT "localhost") for IPv4-force |
| NEW-INV-110 | api/auth.rs | EMBEDDED_CALLBACK_PORT = 53682 type-system-locked; changing breaks Atlassian registration |
| NEW-INV-111 | api/auth.rs | build_authorize_url 3 adversarial tests pin: scope-corruption, redirect_uri injection, encoding drift |
| NEW-INV-112 | api/auth.rs | OAuth state is exactly 256 bits (32 bytes hex); 3 tests pin entropy contract |
| NEW-INV-113 | api/auth.rs | KEYRING_TEST_ENV_MUTEX: 6-mechanism isolation (mutex + opt-in + unique service-name + cleanup) |
| NEW-INV-114 | cli/auth.rs | 5-test set forms precedence lattice for chosen_flow_for_profile (active vs target) |
| NEW-INV-115 | cli/auth.rs | prepare_login_target trims trailing slash from URL (only normalization site) |
| NEW-INV-116 | cli/auth.rs | First-profile auto-default is silent — no message printed |
| NEW-INV-117 | cli/auth.rs | refresh next_step text is identical in JSON output and stderr eprintln |
| NEW-INV-118 | cli/auth.rs | render_list_json passes auth_method through unmodified (no validation at output) |
| NEW-INV-119 | cli/auth.rs | jr auth status may show 'embedded' when keychain temporarily inaccessible — diverges from refresh |
| NEW-INV-120 | cli/auth.rs | jr auth status is the ONLY auth subcmd with mode-conditional strictness (permissive on empty profiles) |
| NEW-INV-121 | view.rs | view-table row order is deterministic 13-row spec + CMDB rows + Points + Team + Description |
| NEW-INV-122 | view.rs | Team UUID with cache-miss renders specific "(name not cached — run jr team list --refresh)" message |
| NEW-INV-123 | comments.rs | Visibility column conditional on entire comment set (any internal → show for all) |
| NEW-INV-124 | format.rs | format_issue_row + issue_table_headers must use same 3-boolean signature; no compile enforcement |
| NEW-INV-125 | format.rs | format_issue_rows_public is stable-API facade — always passes (None,None,None) |
| NEW-INV-126 | helpers.rs | compose_extra_fields ordering: story_points → cmdb → team (stable) |
| NEW-INV-127 | main.rs | --no-color and NO_COLOR env are TWO triggers, single sink: colored::control::set_override(false) |
| NEW-INV-128 | assets.rs | resolve_schema is the ONLY resolver accepting BOTH IDs AND names with fallback |
| NEW-INV-129 | assets.rs | ExactMultiple for schemas emits "(id: <id>)" in dup list — same convention as resolve_resolution_by_name |
| NEW-INV-130 | assets.rs | JSON keeps label attributes; Table drops them — intentional consumer-vs-human divergence |
| NEW-INV-131 | assets.rs | Attribute display: display_value > value > "" (empty fallback, NOT "-") |
| NEW-INV-132 | assets.rs | format_inline_attributes falls back to raw attribute IDs as labels when CMDB metadata missing |
| NEW-INV-133 | api/auth.rs | read_keyring_optional 3-state discrimination: NoEntry → None; other Err → propagate |
| NEW-INV-134 | api/auth.rs | clear_profile_creds("default") deletes legacy flat keys to prevent lazy-migration resurrection |
| NEW-INV-135 | api/auth.rs | clear_all_credentials wipes legacy keys ONLY when "default" is in profiles list |
| NEW-INV-136 | api/auth.rs | Both clear_* functions aggregate failures (Vec<String>) — partial-clear reports all |
| NEW-INV-137 | api/auth.rs | Resolver chain MUST use try_load_oauth_app_credentials (single-pass) — not probe + load |
| NEW-INV-138 | api/auth.rs | 10 keyring tests EACH wrap in with_test_keyring — race-corruption guard |
| NEW-INV-139 | cache.rs | 3 process-global env mutexes: ENV_MUTEX (cache), ENV_LOCK (cli/auth), KEYRING_TEST_ENV_MUTEX (api/auth) |
| NEW-INV-140 | cache.rs | ~20 of 27 cache tests wrap in with_temp_cache; pattern enforced by review only |
| NEW-INV-141 | cache.rs | No tests cross multiple env namespaces (deadlock-avoidance design choice) |
| NEW-INV-142 | config.rs | migrate_legacy_global COPIES (not moves) legacy fields; save_global doesn't propagate them — two-phase |
| NEW-INV-143 | config.rs | **VERIFIED MULTI-PROFILE FIELDS BUG**: ALL config.global.fields.* reads target legacy shape; sandbox profile's team_field_id is IGNORED. Pass 4 trigger. |
| NEW-INV-144 | config.rs | 4-method load facade enforces strict-vs-lenient policy (load_lenient ONLY for jr auth login) |
| NEW-INV-145 | config.rs | save_global is selective — preserves disk-state for `defaults`, NOT for `instance` or `fields` |
| NEW-INV-146 | auth_embedded.rs | 8-test set defends "test builds must NOT have embedded credentials" — CI-leak guard |
| NEW-INV-147 | observability.rs | log_parse_failure_once: one static AtomicBool per call-site (NOT shared); each formatter has own gate |
| NEW-INV-148 | observability.rs | Codebase has NO tracing/structured logging crate; observability.rs is a stub for future expansion |
| NEW-INV-149 | error.rs | Internal variant exits 1 (NOT 64/78) so "we have a bug" distinguishable from UserError/ConfigError |
| NEW-INV-150 | error.rs | 3 transparent #[from] variants (Http, Io, Json) are the `?` propagation arrival points |
| NEW-INV-151 | output.rs | print_warning is the ONLY of 3 print-helpers without color (NITPICK candidate) |
| NEW-INV-152 | output.rs | print_output short-circuits empty rows BEFORE header rendering: "No results found." (dimmed) |
| NEW-INV-153 | tests/common/fixtures.rs | Every Atlassian REST shape has a fixture builder — specification-by-example for Pass 3 BCs |

### Patterns (NEW-PAT-NN, 1 new this round)
- **NEW-PAT-03** — "process-global env-mutex with poisoned-lock recovery + per-call unique-namespace": the same pattern at `cache.rs::ENV_MUTEX`, `api/auth.rs::KEYRING_TEST_ENV_MUTEX`, `cli/auth.rs::ENV_LOCK`, `config.rs::ENV_MUTEX`. **4 sites; one architectural primitive.** Test isolation strategy worth lifting to the spec.

---

## 6. Retracted / corrected

- **CONV-ABS-2** (RETRACTION): R2 §9 carryover #3 said "12 keyring round-trip tests in `cli/auth.rs`". **Recount**: 10 keyring round-trip tests, located in `api/auth.rs` (NOT `cli/auth.rs`). Round 3 catalogues 10 with line numbers in §3.6.
- **CONV-ABS-3** (CORRECTION): R2 §3.11 NEW-INV E-CMD-06 said `--no-color` "toggles ANSI emission via `colored`'s env var override (`NO_COLOR`)". **Corrected**: `--no-color` flag and `NO_COLOR` env var are TWO independent triggers, both routing through `colored::control::set_override(false)` at startup. The mechanism is direct API call, not env-var override. Logged as Round 2 framing slip.
- **CONV-ABS-4** (CORRECTION): Both broad pass §2a.1 AND R2 (and CLAUDE.md) frame `cli/issue/list.rs` as "list + view + comments". **Re-reading the source tree:** `view.rs` (286 LOC) and `comments.rs` (61 LOC) are dedicated sibling modules; `list.rs` (1,083 LOC) contains `handle_list` only. CLAUDE.md is stale. Round 3 catalogues `view.rs` and `comments.rs` as new entities (E-VIEW-01, E-CMTS-01).
- **NO prior Round 1/Round 2 substantive entity or invariant retracted.** All five "potential bug" claims (NEW-INV-18, 19, 29, 56, 81) re-verified by line-by-line source read.
- **R2 self-claim "39 new entities"** — internal accounting, not a load-bearing claim. The 67 listed entities (per R2 §5) are the cumulative new catalogue; "39" is the delta-by-category count. Counting-convention is internally consistent. No retraction.

---

## 7. Delta Summary — what's new vs Round 2

| Category | Items added (delta) |
|---|---|
| ADF deepening (T-ADF-R2) | **+14 entities + 12 invariants** |
| OAuth deepening (T-OAUTH-R2) | **+7 entities + 9 invariants** |
| `cli/auth.rs` deepening | **+6 entities + 7 invariants** |
| `view.rs` + `comments.rs` (new modules) | **+3 entities + 3 invariants** |
| `cli/issue/list.rs` + `format.rs` deepening | **+5 entities + 4 invariants** (incl. CONV-ABS-3 correction) |
| `cli/assets.rs` deepening | **+5 entities + 5 invariants** |
| `api/auth.rs` deepening | **+4 entities + 6 invariants** |
| `cache.rs` deepening | **+3 entities + 3 invariants** |
| `config.rs` deepening (incl. NEW-INV-143 bug verification) | **+4 entities + 4 invariants** |
| `observability.rs` (orphan) | **+2 entities + 2 invariants** |
| `error.rs` JrError | **+1 entity + 2 invariants** |
| `output.rs` | **+1 entity + 2 invariants** |
| `tests/common/fixtures.rs` | **+1 entity + 1 invariant** |
| Patterns | **+1 pattern** (NEW-PAT-03) |

**Quantitative delta (Round 3)**:
- New entities: **31** (vs R2's 39, R1's 33)
- New invariants: **61** (NEW-INV-93..153; vs R2's 75, R1's 17)
- New patterns: **1** (NEW-PAT-03)
- Refined existing: **0 retracted**, **3 framing/counting corrections** logged (CONV-ABS-2/3/4)
- LOC recount discrepancies: **0** (1-LOC rounding for `error.rs`, same as R2)
- **Verified bug claims**: 5/5 propagated from R2 (NEW-INV-18, 19, 29, 56, 81). All real, all surfaced as Pass 3/4 candidates.
- **NEW VERIFIED bug**: NEW-INV-143 (multi-profile fields bug) — strengthens R1's NEW-INV-12 with line-by-line evidence (12 read sites all targeting legacy shape).

**Cumulative (broad + R1 + R2 + R3)**:
- Total entities: 51 (broad) + 33 (R1) + 67 (R2 catalogued) + 31 (R3) = **182**
- Total invariants: 25 (broad) + 17 (R1) + 75 (R2) + 61 (R3) = **178**
- Total patterns: NEW-PAT-01..03 = 3

---

## 8. Novelty Assessment

**Novelty: SUBSTANTIVE**

Justification — would removing this round's findings change how you'd spec the system? **Yes**, in at least 7 model-changing ways:

1. **NEW-INV-143 (verified multi-profile fields bug, escalated from R1's NEW-INV-12)** — pins the architectural defect: ALL `config.global.fields.*` reads use the LEGACY shape, not `active_profile().{team,story_points}_field_id`. Spec must explicitly call out this design limitation OR commit to the schema migration that fixes it. This is a Pass 4 reliability finding AND a Pass 3 BC candidate.

2. **NEW-INV-101 (ADF mention/emoji/inlineCard/media* lossy in text mode)** — spec must either preserve these in `--output text` or explicitly document that JSON is lossless; AI agents reading `jr issue view` against tickets with mentions will silently miss data.

3. **CONV-ABS-4 (view.rs and comments.rs are real sibling modules, NOT inside list.rs)** — CLAUDE.md framing is stale. Pass 3 BC enumeration must reference the correct file split (3 separate handlers in 3 separate files), not the CLAUDE.md "list + view + comments" abstraction.

4. **NEW-INV-105 (XOR key per-build, not per-binary)** — every `cargo build` with the secret produces a different ciphertext. Spec must call this out for release-build determinism / reproducibility planning.

5. **NEW-INV-110 (EMBEDDED_CALLBACK_PORT = 53682, type-system-locked)** — port change is a breaking release because Atlassian Developer Console registers exact port. Spec must capture this as a non-negotiable.

6. **NEW-INV-127 (--no-color mechanism: 2 triggers, 1 sink via `colored::control::set_override`)** — corrects R2's framing of "env var override". Spec for color handling must reflect both flag + env paths.

7. **NEW-INV-148 (codebase has NO tracing/structured logging crate)** — observability is `eprintln!`-only. Spec for adding structured logging would be an architectural leap; the codebase is currently positioned for `tracing` adoption via `observability.rs` stub.

These are model-changing findings, not refinements. The 61 new invariants this round (vs Round 2's 75) materially expand coverage. **SUBSTANTIVE.**

---

## 9. Remaining gaps / next candidate scope (verbatim for Round 4)

### High priority (still under-deepened)

1. **`cli/auth.rs` (1,998 LOC) deep round 3** — Round 3 covered chosen_flow tests, prepare_login_target, refresh_success_payload, render_list_json, peek_oauth_app_source, status semantics. Round 4 should:
   - Catalogue `login_oauth` line-by-line: scope resolution, browser launch, listener strategies, error paths.
   - Catalogue `login_token` line-by-line: prompt fallbacks, env-var precedence.
   - Catalogue `handle_remove_in_memory` + cache directory cleanup post-removal.
   - Catalogue every `JrError::UserError` construction site in `cli/auth.rs` (broad pass §2a said 11 variants but didn't enumerate per-file usage).

2. **`cli/issue/list.rs` (1,083 LOC) deep round 3** — Round 3 only covered `format_issue_row` + the file-split discovery. The query lifecycle (Round 1 §3.2's 13 positions) and `--asset` JQL composition need a per-line walk:
   - `compose_filter_clauses` per-position semantics.
   - `--open` filtering interaction with `--status`.
   - `--asset` short-circuit when no CMDB fields.
   - The "Showing N of ~M" approximation logic.

3. **`cli/assets.rs` (1,055 LOC) deep round 3** — Round 3 covered 5 sub-commands at high level. Round 4 should:
   - `handle_search` — `attributes: bool` flag's enrichment cost (separate API call).
   - `handle_tickets` — `filter_tickets` 4-state branching (open + status combinations).
   - `handle_types` — schema_name injection into JSON entries.
   - `handle_schema` — dedicated single-schema view.

4. **OAuth login flow line-by-line (`api/auth.rs`)** — Round 1/R2/R3 covered redirect URIs, state generation, refresh_oauth_token, embedded resolver. Still missing:
   - `oauth_login` orchestration (the actual login flow).
   - `accessible_resources` fetch + `cloud_id` selection logic.
   - PKCE? code_verifier? (do we have any?) — verify.
   - Browser-launch fallback strategy.

5. **`config.rs` deep round 3** — Round 3 covered migration + load facade + save semantics + NEW-INV-143. Round 4 should:
   - `Config::project_key` + per-project `.jr.toml` precedence.
   - `validate_profile_name` + every error case.
   - The `Figment::merge(Env::prefixed("JR_"))` env-var injection scope.

### Medium priority

6. **`api/assets/*` (4 files, ~600 LOC total)** — broad pass and R1 §3.3 covered the 6 endpoints + filter_tickets + key resolution. Round 4 line-by-line:
   - `workspace.rs` — workspace ID discovery + cache.
   - `linked.rs` — CMDB field discovery, asset extraction, `enrich_assets`/`enrich_json_assets`.
   - `objects.rs` — AQL search, get_object, resolve_object_key.
   - `tickets.rs` — connected tickets endpoint.

7. **`api/jsm/*` (servicedesks.rs + queues.rs)** — broad pass listed; Round 4 line-by-line.

8. **`types/jira/*` and `types/assets/*`** — none covered. Round 4 should catalogue serde struct shapes, custom field handling, the `IssueFields::extra` HashMap behavior.

9. **Integration tests `tests/*.rs` (33 files)** — Round 3 only catalogued `tests/common/fixtures.rs`. Round 4 should enumerate test files + their wiremock setups for Pass 3 BC derivation.

### Low priority (NITPICK candidates)

10. **`output.rs`** — covered. NITPICK.
11. **`error.rs`** — covered. NITPICK.
12. **`auth_embedded.rs`** — 8-test catalog covered. NITPICK.
13. **`build.rs`** — 4-stage XOR pipeline covered. NITPICK.
14. **`observability.rs`** — covered. NITPICK.

### Pass 4 deepening triggered (cross-pollination — DO NOT write into Pass 2)

15. **NEW-INV-143 (multi-profile fields bug)** — Pass 4 reliability concern. NEW finding this round; Pass 4 round 2 must incorporate.
16. **NEW-INV-101 (ADF lossy node types)** — Pass 4 UX concern (AI agents miss data); Pass 3 BC concern (text rendering contract).
17. **NEW-INV-105/127/148 (build-time XOR per-build, --no-color two-trigger, no-tracing)** — Pass 4 architecture concerns.
18. **NEW-INV-119 (auth status vs refresh inconsistent failure semantics)** — Pass 4 reliability concern (silent fall-through to embedded).
19. (Carry from R2): handle_open OAuth bug, list_worklogs non-pagination, hardcoded 8/5 worklog, get_changelog anti-loop, asset enrichment dedup — all confirmed real this round; Pass 4 round 2 must incorporate.

---

## 10. State Checkpoint

```yaml
pass: 2
round: 3
status: complete
audit_findings_against_hallucination_classes: 3
new_entities: 31
new_invariants: 61
retracted_findings: 0
files_examined: 21
novelty: SUBSTANTIVE
timestamp: 2026-05-04T20:30:00Z
next_round_targets: |-
  1. cli/auth.rs deep round 3 — login_oauth + login_token line-by-line, handle_remove_in_memory, JrError construction sites
  2. cli/issue/list.rs deep round 3 — compose_filter_clauses per-position, --asset short-circuit, --open + --status interactions, "Showing N of ~M" approximation
  3. cli/assets.rs deep round 3 — handle_search attributes-cost, handle_tickets 4-state filter, handle_types schema-name injection, handle_schema
  4. api/auth.rs OAuth login flow line-by-line — oauth_login orchestration, accessible_resources, cloud_id selection, PKCE? browser launch
  5. config.rs deep round 3 — Config::project_key precedence, validate_profile_name, JR_* env injection scope
  6. api/assets/* line-by-line (workspace.rs, linked.rs, objects.rs, tickets.rs)
  7. api/jsm/* line-by-line (servicedesks.rs, queues.rs)
  8. types/jira/* + types/assets/* serde struct catalog (custom fields, IssueFields::extra HashMap)
  9. integration tests tests/*.rs (33 files) — Pass 3 BC enumeration prep
  10. (NITPICK candidates) output.rs, error.rs, auth_embedded.rs, build.rs, observability.rs
  15. (Pass 4 cross-pollination) NEW-INV-143 multi-profile fields bug — verified this round
  16. (Pass 4 cross-pollination) NEW-INV-101 ADF lossy node types
  17. (Pass 4 cross-pollination) NEW-INV-105/127/148 architecture concerns
  18. (Pass 4 cross-pollination) NEW-INV-119 status-vs-refresh inconsistent failure
  19. (Pass 4 cross-pollination, carry from R2) handle_open / list_worklogs / hardcoded 8/5 / anti-loop / asset enrichment
```

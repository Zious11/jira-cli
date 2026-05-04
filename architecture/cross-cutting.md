# Cross-Cutting Concerns — jr (jira-cli)

**traces_to:** README.md
**Source:** Pass 1 broad §3a-k, R1 §6a-6d, R2 §4-5

---

## 1. Error Handling (`src/error.rs`)

### JrError variants and exit codes

| Variant | Exit code | When |
|---------|----------:|------|
| `NotAuthenticated` | 2 | 401 from Jira (not scope-related) |
| `InsufficientScope { message }` | 2 | 401 with body matching "scope does not match" (issue #185 — granular tokens reject POST) |
| `ConfigError(String)` | 78 | Missing config, profile unconfigured, bad TOML |
| `UserError(String)` | 64 | Bad CLI input — unknown profile, ambiguous match, empty selection, missing required arg |
| `Interrupted` | 130 | Reserved for Ctrl+C (see note below) |
| `Internal(String)` | 1 | "Should never happen" — invariant violations |
| `NetworkError(String)` | 1 | reqwest reachability failure (DNS, connect timeout) |
| `ApiError { status, message }` | 1 | Any 4xx/5xx not specialized into above |
| `Http(reqwest::Error)` | 1 | `#[from]` transparent |
| `Io(std::io::Error)` | 1 | `#[from]` transparent |
| `Json(serde_json::Error)` | 1 | `#[from]` transparent |

**Total: 11 variants** (R1 corrected broad pass's count of 10).

**Ctrl+C note:** `main.rs` uses `tokio::select! { _ = ctrl_c() => { eprintln!("\nInterrupted"); process::exit(130) } }`. The `Interrupted` variant is never constructed in practice — abrupt exit is the implementation.

**Handler pattern:** handlers return `anyhow::Result<()>`. `main.rs` walks `e.chain()` for the first `JrError` to extract exit code. If no `JrError` found: exit 1. JSON output mode wraps as `{"error": ..., "code": ...}`.

### `extract_error_message` — 7-level precedence chain

This function (`api/client.rs:448-490`) is the load-bearing error message extractor for all API errors. Users see its output for every `JrError::ApiError`.

**Canonical source of truth:** PRD `error-taxonomy.md §2` (corrected by CONV-ABS-004 — empty body is FIRST, not last).

```
Priority 1 (HIGHEST): response body is empty → return None; caller uses status-code-derived message
Priority 2: response["errorMessages"] — array; join non-empty elements with ";"
Priority 3: response["errors"] — object; "field: msg" pairs (alphabetically sorted; non-string values JSON-serialized)
Priority 4: response["errors"][field]["messages"] — nested messages array; first element
Priority 5: response["message"] — single string (most REST v3 error shape)
Priority 6: response["errorDescription"] — singular form (alt field name, JSM/OAuth error shape)
Priority 7 (FALLBACK): raw response body via from_utf8_lossy — if none of the above match
```

Every error message a user sees from `JrError::ApiError` is shaped by this chain. Phase 3 tests must exercise all 7 levels.

**Key invariant:** Empty body check is FIRST — prevents treating `{}` or `""` as an error message source.

---

## 2. Output Discipline (`src/output.rs`)

### stdout vs stderr contract

| Channel | Contents |
|---------|---------|
| **stdout** | Table output (comfy-table), JSON output (`--output json`), parseable success data |
| **stderr** | `print_success/warning/error`, `eprintln!` verbose logs, rate-limit retry warnings, cache-corruption warnings, browse URLs (Table mode), "no teams found", browser-launch failure (non-fatal) |

**Invariant:** `--output json` consumers may pipe `2>/dev/null` to get clean JSON with all human-noise suppressed.

### Output format selection

`print_output(format, headers, rows, json_data)` is the unified call shape. Every handler builds parallel table rows + serializable struct.

### Color control

`colored::control::set_override(false)` is called when `--no-color` flag is set OR when `NO_COLOR` env var is present. Both triggers apply; single mechanism.

### `--verbose` mode

When `--verbose` is set:
- `[verbose] {METHOD} {URL}` printed to stderr before each request
- Full request body printed via `String::from_utf8_lossy` (**including comments, summaries, descriptions, accountIds, emails**)
- Rate-limit retry timing printed to stderr
- Parse-failure one-shot logs from `observability.rs` printed

**Security gap (NFR-S-C):** Authorization header is NOT logged, but request body is. Users piping `jr ... 2>log.txt` for debugging leak payload bytes. Policy decision required for Phase 3 (see nfr-catalog.md NFR-S-C).

---

## 3. Rate Limiting and Retry (`api/client.rs`, `api/rate_limit.rs`)

| Constant | Value | Location |
|----------|-------|---------|
| `MAX_RETRIES` | 3 | `api/client.rs:11` |
| `DEFAULT_RETRY_SECS` | 1 | `api/client.rs:14` |

**Retry logic:** on 429, parse `Retry-After` header as integer seconds (via `api/rate_limit.rs:14-19`). Sleep and retry up to 3 times. After exhausting retries: `"warning: rate limited by Jira — gave up after 3 retries."` to stderr.

**Retry-After format support:** integer seconds only. HTTP-date format (`Mon, 04 May 2026 00:00:00 GMT`) silently falls through to `DEFAULT_RETRY_SECS = 1`. Atlassian sends integers in practice (NFR-SCA-1, LOW).

`X-RateLimit-Remaining` is parsed but only stored diagnostically — not used to proactively throttle.

---

## 4. Pagination (`api/pagination.rs`)

Four pagination shapes for four distinct Atlassian API families:

| Type | Strategy | Used by | Field shape |
|------|----------|---------|-------------|
| `OffsetPage<T>` | offset (`startAt` + `maxResults` + `total`) | Most Jira REST v3 endpoints | Items under `values` / `issues` / `worklogs` / `comments` — `items()` returns whichever is populated, in priority order |
| `CursorPage<T>` | cursor (`nextPageToken`) | `/rest/api/3/search/jql` ONLY | `nextPageToken` is `None` when last page |
| `ServiceDeskPage<T>` | offset, JSM shape (`size`/`start`/`limit`/`isLastPage`) | `/rest/servicedeskapi/*` | `is_last_page` boolean instead of computed from offsets |
| `AssetsPage<T>` | offset, Assets shape (`startAt`/`maxResults`/`total`/`isLast`) | Assets `POST /object/aql` | `isLast` may be bool OR string — `deserialize_bool_or_string` handles both |

**NFR-R-A gap:** `list_worklogs` uses `OffsetPage<Worklog>` but returns `.items().to_vec()` with no pagination loop — silently truncates at first page for issues with >50 worklogs. Fix: refactor to `paginate_offset` loop (same pattern as `list_comments`). See ADR-0010.

---

## 5. ADF Translation (`src/adf.rs`, 1,826 LOC)

Atlassian Document Format (ADF) is the native storage format for Jira Cloud rich text (issue descriptions, comments). `adf.rs` provides:

| Function | Direction | Notes |
|----------|-----------|-------|
| `text_to_adf(s)` | plain text → ADF | Wraps in paragraph node |
| `markdown_to_adf(s)` | Markdown → ADF | Via `pulldown-cmark` |
| `adf_to_text(node)` | ADF → plain text | Used for table/terminal rendering |

**Known lossy conversions (NFR-O-I, LOW):** `adf_to_text` silently drops `mention`, `emoji`, `inlineCard`, and `media` nodes via `_` fall-through at `adf.rs:531-540`. Documented as intentional per issue #202.

Used by: issue create/edit (`--description-stdin`, `--markdown`), `comment add`, issue view renderer.

---

## 6. JQL Utilities (`src/jql.rs`, 395 LOC)

| Function | Purpose | Notes |
|----------|---------|-------|
| `escape_value(s)` | Escape for JQL string literals | Backslashes first, then quotes — order is critical |
| `validate_duration(s)` | Validate JQL duration syntax | Units `y/M/w/d/h/m`; combined units rejected; `M` = months, `m` = minutes |
| `validate_date(s)` | Validate JQL date literal | |
| `validate_asset_key(key)` | Validate CMDB asset key format | Must match `<alphanumeric>-<digits>` (e.g., CUST-5) |
| `build_asset_clause(key, fields)` | Build AQL filter JQL clause | Uses field **name** (human-readable), NOT `customfield_NNNNN`. AQL attribute: `Key` (not `objectKey`). |
| `strip_order_by(jql)` | Remove ORDER BY from user JQL | Called before re-appending canonical ORDER BY |

**Property-test coverage:** `proptest-regressions/jql.txt` — the brittle escaping logic has fuzz coverage.

**Critical gotcha:** `aqlFunction()` not `assetsQuery()`. Field name (human-readable) not field ID. AQL attribute `Key` not `objectKey`. `build_asset_clause` enforces this boundary.

---

## 7. Partial Match (`src/partial_match.rs`, 200 LOC)

Case-insensitive substring matching with explicit disambiguation semantics.

```
MatchResult::Exact(T)        — exactly one item whose name matches exactly
MatchResult::Unique(T)       — exactly one item whose name contains the needle
MatchResult::Ambiguous(Vec)  — multiple items contain the needle
MatchResult::None            — no items match
```

**Fail-closed contract:** on `Ambiguous`, commands refuse the operation with exit 64 (`UserError`) and list the candidates. Never guess. Under `--no-input`, disambiguation prompt is suppressed and the `Ambiguous` result is an immediate error.

Property tests cover this module (`proptest`).

---

## 8. Duration Parser (`src/duration.rs`, 159 LOC)

Parses worklog duration strings: `2h`, `1h30m`, `1d`, `1w`, etc. Converts to seconds using hardcoded constants: 8h/day, 5d/week.

**NFR-R-C gap (MEDIUM):** Jira instances can configure different day/week lengths via `/rest/api/3/configuration/timetracking`. The hardcoded constants silently produce wrong values for non-standard configurations (7.5h/day, 4-day weeks).

**NFR-R-NEW-2 gap (LOW):** `parse_duration` silently wraps on multiplicative overflow for pathological inputs (e.g., `99999999999999w`) in release builds (`panic=abort` disables debug overflow checks). Fix: use `checked_mul`; bail with "duration too large" error.

---

## 9. Observability (`src/observability.rs`, 39 LOC — `pub(crate)` only)

The entire observability surface is a single function:

```rust
pub(crate) fn log_parse_failure_once(flag: &AtomicBool, site: &str, iso: &str, verbose: bool)
```

Used at exactly 2 call sites (Pass 1 R2 verified):
- `cli/issue/format.rs:127`
- `cli/issue/changelog.rs:276`

`pub(crate)` — not in the integration-test-visible public API surface.

**Pattern:** each call site has a `static LOGGED: AtomicBool = AtomicBool::new(false)`. On first parse failure, logs `[verbose] parse failure at {site}: "{iso}"` to stderr. Subsequent failures at the same site are silent. Prevents log flooding for chronic parse failures.

**No tracing crate.** No structured logging. All diagnostic output is `--verbose`-gated `eprintln!`. NFR-O-A (MEDIUM, DEFERRED) tracks adoption of the `tracing` crate (already a dependency — `tracing` is in `[dependencies]` — but not wired to any subscriber in production).

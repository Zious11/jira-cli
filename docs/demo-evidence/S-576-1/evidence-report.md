# Evidence Report — S-576-1: `jr issue attachment list`

**Story:** S-576-1  
**Feature:** `jr issue attachment list` — table + JSON output + client-side filters  
**BC anchors:** BC-2.7.001, BC-2.7.002, BC-2.7.003, BC-2.7.004, BC-2.7.005, BC-2.7.006  
**Recorded:** 2026-07-19

---

## Recording Setup

All CLI demos use a minimal Python mock server (`mock-server.py`) serving canned
Jira API fixture responses at `http://127.0.0.1:19876`.  The binary is the
debug build at `.worktrees/S-576-1/target/debug/jr`.  Auth is bypassed via
`JR_AUTH_HEADER="Basic dGVzdDp0ZXN0"` (debug seam); API URL is overridden via
`JR_BASE_URL` (debug seam).  No real Jira instance is involved.

Mock fixtures:
| Key   | Mock response |
|-------|---------------|
| DEMO-1 | 3 attachments (PDF, PNG, CSV) |
| DEMO-2 | 0 attachments |
| DEMO-3 | 404 Not Found |
| DEMO-4 | 401 Unauthorized |
| DEMO-5 | 403 Forbidden |
| DEMO-6 | 500 Internal Server Error |
| DEMO-7 | 6 mixed attachments (PDF ×3, JPEG, PNG, CSV) for filter demos |

---

## AC Coverage Map

### AC-001 — Table output (BC-2.7.001)
**Artifact:** `AC-001-002-table-zero-attachments.gif` / `.webm`

Demonstrates `jr issue attachment list DEMO-1` rendering a six-column table
(ID | Filename | Type | Size | Created | Author) with human-readable sizes
(42.0 KB, 200.0 KB, 8.0 KB).  `display_sanitize_filename` applied to every
Filename cell.

---

### AC-002 — Zero attachments (BC-2.7.001)
**Artifact:** `AC-001-002-table-zero-attachments.gif` / `.webm`  
*(same recording as AC-001; second command in tape)*

Demonstrates `jr issue attachment list DEMO-2` (zero attachments): exits 0,
empty stdout, stderr prints `"No attachments on DEMO-2."`.

---

### AC-003 — JSON curated shape (BC-2.7.002)
**Artifact:** `AC-003-004-json-filter-hint.gif` / `.webm`

Demonstrates `jr issue attachment list DEMO-1 --output json`:
- Pretty-printed JSON array routed through `output::render_json` (#526 invariant)
- Each element: keys in BTreeMap order `author, contentUrl, created, filename, id, mimeType, size`
- `"self"` absent from all elements (VP-576-004 assertion 1)
- `"content"` renamed to `"contentUrl"` (VP-576-004 assertion 2)
- `author` curated to `{accountId, displayName}` only — no `self`, `avatarUrls`, `accountType`
- `size` is a raw `u64` integer

---

### AC-004 — Filter-count hint (BC-2.7.001 EC-2.7.001-2)
**Artifact:** `AC-003-004-json-filter-hint.gif` / `.webm`  
*(same recording as AC-003; second command in tape)*

Demonstrates `jr issue attachment list DEMO-1 --filter mime=application/pdf`:
- Filter reduces display from 3 to 1 row (N < M)
- Hint fires on stderr: `"Showing 1 of 3 attachments."`
- Hint appears in table mode; same hint fires in JSON mode (deliberate asymmetry BC-2.7.001 EC-2.7.001-2)

---

### AC-005 — `--filter mime=<glob>` (BC-2.7.003)
**Artifact:** `AC-005-006-007-filters.gif` / `.webm`  
*(first command in tape)*

Demonstrates `jr issue attachment list DEMO-7 --filter mime=image*`:
- Glob `image*` crosses `/`: matches `image/jpeg` and `image/png` (BC-2.7.003 star-crosses-slash)
- `application/pdf` excluded (3 of 6 matched)
- Hint: `"Showing 2 of 6 attachments."`

---

### AC-006 — `--filter name=<glob>` + AND composition (BC-2.7.004)
**Artifact:** `AC-005-006-007-filters.gif` / `.webm`  
*(second command in tape)*

Demonstrates `jr issue attachment list DEMO-7 --filter name=report-?.pdf`:
- Glob `report-?.pdf` with `?` matches exactly one character
- Matches `report-A.pdf` and `report-1.pdf`; excludes `report-10.pdf` (two chars after dash)
- Hint: `"Showing 2 of 6 attachments."`

---

### AC-007 — `--filter size-max=<bytes>` (BC-2.7.005)
**Artifact:** `AC-005-006-007-filters.gif` / `.webm`  
*(third command in tape)*

Demonstrates `jr issue attachment list DEMO-7 --filter size-max=2048`:
- Retains only files ≤ 2048 bytes (3 of 6: report-A.pdf 1 KB, report-1.pdf 2 KB, data.csv 512 B)
- Hint: `"Showing 3 of 6 attachments."`

---

### AC-008 — Invalid filter → exit 64 (BC-2.7.003 EC-2.7.003-2)
**Artifact:** `AC-008-invalid-filter.gif` / `.webm`

Two error paths:

1. `jr issue attachment list DEMO-1 --filter mime` (missing `=`):  
   Exit 64, message: `"Invalid filter 'mime': expected key=value form. Accepted keys: mime=, name=, size-max=."`  
   No HTTP call made.

2. `jr issue attachment list DEMO-1 --filter type=image` (unknown key):  
   Exit 64, message: `"Unknown filter key 'type'. Accepted keys: mime=, name=, size-max=."`  
   No HTTP call made.

---

### AC-009 — Error taxonomy (BC-2.7.006)
**Artifact:** `AC-009-error-taxonomy.gif` / `.webm`

Five error paths:

| Demo command | HTTP status | Exit | Stderr |
|---|---|---|---|
| `jr issue attachment list DEMO-3` | 404 | 64 | `Issue DEMO-3 not found or not accessible.` |
| `jr issue attachment list DEMO-4` | 401 | 2 | `Not authenticated. Run "jr auth login" to connect.` |
| `jr issue attachment list DEMO-5` | 403 | 1 | `Permission denied: cannot access issue DEMO-5.` |
| `jr issue attachment list DEMO-6` | 500 | 1 | `API error (500): Internal server error` |
| `JR_BASE_URL=http://127.0.0.1:19999 jr issue attachment list DEMO-1` | network | 1 | `Could not reach 127.0.0.1 — check your connection` |

---

### AC-010 — CLI surface guard (BC-2.7.001 precondition)
**Artifact:** `AC-010-surface-guard.gif` / `.webm`

Demonstrates `cargo test --test e2e_cli_surface_guard 2>&1 | tail -8`:
- All 10 surface guard tests pass (GREEN)
- `test_e2e_cli_surface_all_paths_and_flags_exist` verifies `attachment list` flags
  (`<KEY>`, `--filter`, `--output`, `--no-input`, `--profile`, `--no-color`) exist in CLI help
- `test_parser_paths_are_subset_of_surface_table` confirms SURFACE table entries are registered

---

### AC-011 — Documentation obligations (BC-2.7.001 postcondition)
**Artifact:** `AC-011-docs-obligations.gif` / `.webm`

Demonstrates:
1. `grep -n 'attachment list' README.md` → line 272: command-table row present
2. `grep -n 'attachment list' CHANGELOG.md` → lines 21–32: CHANGELOG entry present
   (`feat(issue): attachment list subcommand + JSON output + filters (#576)`)
3. `cargo test --test claude_md_citations 2>&1 | tail -5` → all 61 citations pass
   (validates `src/cli/issue/attachments.rs` and `src/api/jira/attachments.rs` entries
   in CLAUDE.md `src/cli/issue/` and `src/api/jira/` architecture listings)

---

## Coverage Summary

| AC | BC | Artifact (GIF + WebM) | Coverage |
|----|----|----------------------|----------|
| AC-001 | BC-2.7.001 | AC-001-002-table-zero-attachments | success path |
| AC-002 | BC-2.7.001 | AC-001-002-table-zero-attachments | zero-attachment path |
| AC-003 | BC-2.7.002 | AC-003-004-json-filter-hint | JSON curated shape |
| AC-004 | BC-2.7.001 EC-2.7.001-2 | AC-003-004-json-filter-hint | filter-count hint |
| AC-005 | BC-2.7.003 | AC-005-006-007-filters | mime glob (star crosses /) |
| AC-006 | BC-2.7.004 | AC-005-006-007-filters | name glob (? metacharacter) |
| AC-007 | BC-2.7.005 | AC-005-006-007-filters | size-max filter |
| AC-008 | BC-2.7.003 EC-2.7.003-2 | AC-008-invalid-filter | invalid filter (2 paths) |
| AC-009 | BC-2.7.006 | AC-009-error-taxonomy | 404 / 401 / 403 / 5xx / network |
| AC-010 | BC-2.7.001 precondition | AC-010-surface-guard | e2e_cli_surface_guard 10/10 GREEN |
| AC-011 | BC-2.7.001 postcondition | AC-011-docs-obligations | README + CHANGELOG + claude_md_citations |

**All 11 acceptance criteria covered. Every recording links to a specific AC.**

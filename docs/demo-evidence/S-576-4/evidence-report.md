# Demo Evidence Report — S-576-4

**Story:** S-576-4 — `jr issue attachment delete` single AID + bulk + `--older-than` + `--dry-run`  
**Branch:** `feat/S-576-4-attachment-delete` (worktree `.worktrees/S-576-4`)  
**Tool:** VHS 0.11.0 (terminal recordings)  
**Mock backend:** `mock-server.py` (port 19879, Python 3.14 HTTPServer)  
**Build:** `cargo build` debug — `jr 0.6.0-dev.10`  
**Seams:** `JR_BASE_URL`, `JR_AUTH_HEADER`, `JR_CONFIG_DIR`, `JR_CACHE_DIR`, `JR_STDIN_IS_TTY`  
**Acceptance criteria count:** 16  

---

## Recordings

| File | GIF | WEBM | ACs Covered |
|------|-----|------|-------------|
| `AC-001-002-003-010-single-gate.tape` | `AC-001-002-003-010-single-gate.gif` | `AC-001-002-003-010-single-gate.webm` | AC-001, AC-002, AC-003, AC-010 |
| `AC-001-013-dec168-targeted-404.tape` | `AC-001-013-dec168-targeted-404.gif` | `AC-001-013-dec168-targeted-404.webm` | AC-001, AC-013 |
| `AC-004-005-bulk-failsoft.tape` | `AC-004-005-bulk-failsoft.gif` | `AC-004-005-bulk-failsoft.webm` | AC-004, AC-005 |
| `AC-006-007-011-issue-older-than.tape` | `AC-006-007-011-issue-older-than.gif` | `AC-006-007-011-issue-older-than.webm` | AC-006, AC-007, AC-011 |
| `AC-008-009-dry-run.tape` | `AC-008-009-dry-run.gif` | `AC-008-009-dry-run.webm` | AC-008, AC-009 |
| `AC-007-016-duration-errors.tape` | `AC-007-016-duration-errors.gif` | `AC-007-016-duration-errors.webm` | AC-007, AC-016 |
| `AC-012-014-015-clap-tests.tape` | `AC-012-014-015-clap-tests.gif` | `AC-012-014-015-clap-tests.webm` | AC-012, AC-013, AC-014, AC-015 |

---

## AC Coverage Map

### AC-001 — DELETE endpoint; AID validation; 204/200 success; 404→DEC-168

**Recording 1:** `AC-001-002-003-010-single-gate.gif` — successful delete with `--yes`  
**Recording 2:** `AC-001-013-dec168-targeted-404.gif` — DEC-168 targeted 404 + AID validation

Evidence shown:
- `jr issue attachment delete 99001 --yes` → `"Deleted attachment 99001."` (human, stderr)
- `jr issue attachment delete 99404 --yes` → exit 64; stderr begins with `"Attachment 99404 not found or not accessible."` followed by Jira error body (DEC-168 canonical prefix+body order)
- `jr issue attachment delete abc --yes` → exit 64: `"invalid attachment id: 'abc' (must be numeric)"` (AID validation before HTTP)

---

### AC-002 — Single-AID confirmation gate; pre-prompt metadata GET; VP-576-002

**Recording:** `AC-001-002-003-010-single-gate.gif`

Evidence shown:
- Interactive gate: `jr issue attachment delete 99001` → shows prompt `"Delete attachment report.pdf (99001)? [y/N] "` (pre-prompt GET fetches filename) → typing `y` proceeds to DELETE (VP-576-002 confirm)
- Typing `n` → `"Deletion cancelled."` exit 0 (VP-576-002 cancel)

---

### AC-003 — Single-AID JSON response shape

**Recording:** `AC-001-002-003-010-single-gate.gif`

Evidence shown:
- `jr issue attachment delete 99001 --yes --output json` → `{"deleted": true, "id": "99001"}` (BTreeMap-alphabetical; output via `render_json`)
- Human mode: `"Deleted attachment 99001."` to stderr (Profile 4 Symmetric)

---

### AC-004 — `--yes` required for bulk; fail-soft 404; non-404 abort; all-404 benign hint

**Recording:** `AC-004-005-bulk-failsoft.gif`

Evidence shown:
- Missing `--yes` on `<AID1> <AID2>` → exit 64: `"--yes is required to delete multiple attachments without a confirmation prompt."` (EC-3.9.016-8)
- Partial 404 benign skip: `delete 99001 99998 --yes` → `{"count":1,"deleted":true,"ids":["99001"]}` (99998 benign-skipped)
- Non-404 abort: `delete 99001 99403 --yes` → first DELETE completes, 403 aborts sequence → exit 1 (EC-3.9.010-4)

---

### AC-005 — Bulk JSON response shape

**Recording:** `AC-004-005-bulk-failsoft.gif`

Evidence shown:
- `jr issue attachment delete 99001 99002 --yes --output json` → `{"count": 2, "deleted": true, "ids": ["99001", "99002"]}` (BTreeMap-alphabetical; IDs in request order)

---

### AC-006 — `--issue KEY + --older-than`; age filter; BC-3.9.019 canonical strings

**Recording:** `AC-006-007-011-issue-older-than.gif`

Evidence shown:
- Human mode: `Deleting 2 attachment(s) older than 30d from DEMO-1.` (pre-DELETE HINT) + `Deleted 2 attachment(s) older than 30d from DEMO-1.` (success summary) on stderr
- JSON mode (`--output json`): NEITHER hint string emitted to stderr (BC-3.9.019 JSON-suppressed)
- Zero-match (DEMO-2 empty): `"No attachments older than 30d found on DEMO-2."` → exit 0

---

### AC-007 — `--older-than` duration parsing; invalid duration; P1-001/P2-001 overflow guards

**Recording 1:** `AC-006-007-011-issue-older-than.gif` — chrono filter behavior  
**Recording 2:** `AC-007-016-duration-errors.gif` — invalid duration error paths

Evidence shown:
- Chrono filter: DEMO-10 has 99001 (2025-12-01, old) + 99003 (2026-07-20, recent); `--older-than 30d --dry-run` shows only 99001 in table (99003 filtered out by age)
- Invalid `bad` → exit 64: `"invalid duration: 'bad'. Use formats like 30m, 2h, 1d, 7d, 2w."` (EC-3.9.019-3)
- Multibyte `5€` (P1-001a) → exit 64 (not panic)
- Overflow `1000000000000d` (P2-001 chrono-band guard) → exit 64 (not overflow-panic)

---

### AC-008 — `--dry-run` single-AID (EC-3.9.020-3); AID validation fires; no gate; no DELETE

**Recording:** `AC-008-009-dry-run.gif`

Evidence shown:
- `jr issue attachment delete 99001 --dry-run` → stderr: `"--dry-run has no effect on single-ID delete; omit the flag."` exit 0 (no gate, no DELETE)
- JSON mode: `{"attachments":[{"id":"99001"}],"dryRun":true,"ids":["99001"]}` (no filename — no metadata fetch)

---

### AC-009 — `--dry-run` bulk (EC-3.9.020-1/2); `[ID,Filename,Size,Created]` table; metadata fan-out

**Recording:** `AC-008-009-dry-run.gif`

Evidence shown:
- `--issue DEMO-1 --older-than 30d --dry-run`: `[ID,Filename,Size,Created]` table with 2 rows; final line: `"2 attachment(s) would be deleted. Run without --dry-run to confirm."`
- JSON mode: `{"attachments":[{"filename":"report.pdf","id":"99001"},{"filename":"notes.txt","id":"99002"}],"dryRun":true,"ids":["99001","99002"]}`
- Multi-AID dry-run (`99001 99002 --dry-run`): per-AID metadata fan-out via GET populates Filename column

---

### AC-010 — Non-interactive without `--yes` exits 64 (DEC-174)

**Recording:** `AC-001-002-003-010-single-gate.gif`

Evidence shown:
- `jr issue attachment delete 99001 --no-input` → exit 64: `"Use --yes to confirm deletion without a prompt."` (BC-3.9.015 non-interactive canonical message)

---

### AC-011 — `--issue + --older-than` combined; `display_sanitize_filename` in dry-run preview

**Recording:** `AC-006-007-011-issue-older-than.gif`

Evidence shown:
- Combined flow: `--issue DEMO-1 --older-than 30d --yes` runs list resolve + age filter + bulk delete with BC-3.9.016 definitive ruling (no interactive gate on bulk path — `--yes` required upfront)
- No interactive gate offered; `--yes` was required (confirmed by EC-3.9.016 enforcement test in clap-tests tape)
- Dry-run preview filenames are display-sanitized (CWE-116 via `display_sanitize_filename`)

---

### AC-012 — e2e surface guard; CLAUDE.md delivery; docs/specs/ delivery

**Recording:** `AC-012-014-015-clap-tests.gif`

Evidence shown:
- `jr issue attachment delete --help | grep -E 'issue|older-than|--yes|--dry-run'` → all flags visible (e2e surface guard entries confirmed)
- `cargo test --test attachment_delete` → 25 tests green (covers surface guard + CLAUDE.md citations)

---

### AC-013 — VP-576-002 wiremock; DEC-168 body surfacing; mutants.toml

**Recording 1:** `AC-001-002-003-010-single-gate.gif` — VP-576-002 confirm/cancel  
**Recording 2:** `AC-001-013-dec168-targeted-404.gif` — DEC-168 prefix+body order  
**Recording 3:** `AC-012-014-015-clap-tests.gif` — cargo test covers VP-576-002 wiremock tests

Evidence shown:
- VP-576-002 confirm: gate confirm (`y`) → DELETE issued (wiremock asserts one DELETE)
- VP-576-002 cancel: gate cancel (`n`) → `"Deletion cancelled."` exit 0, zero DELETEs issued
- DEC-168: stderr BEGINS with `"Attachment 99404 not found or not accessible."` then Jira body (prefix before body, NOT body-only)
- cargo test green confirms `test_bc_3_9_008_404_body_surfaced_to_stderr` assertion passes

---

### AC-014 — bare `--issue` without `--older-than` → exit 2 (EC-3.9.016-9)

**Recording:** `AC-012-014-015-clap-tests.gif`

Evidence shown:
- `jr issue attachment delete --issue DEMO-1` → exit 2 + clap error: `the following required arguments were not provided: --older-than`

---

### AC-015 — Clap mutual-exclusion and required-group constraints (EC-3.9.016-4/5/9/10)

**Recording:** `AC-012-014-015-clap-tests.gif`

Evidence shown:
- `--issue without --older-than` → exit 2 (EC-3.9.016-9)
- `--older-than without --issue` → exit 2 (EC-3.9.016-5)
- positional AID + `--issue` conflict → exit 2 (EC-3.9.016-4)

---

### AC-016 — Delete error taxonomy (BC-3.9.013)

**Recording:** `AC-007-016-duration-errors.gif`

Evidence shown:
- 403 targeted delete → exit 1: `"API error (403): Permission denied."`
- AID validation (non-numeric `abc`) → exit 64: `"invalid attachment id: 'abc' (must be numeric)"` (before any HTTP)
- Invalid duration → exit 64 (taxonomy row: invalid input)

---

## Summary

| AC | Description | Recording | Status |
|----|-------------|-----------|--------|
| AC-001 | DELETE endpoint; AID validation; 204 success; 404 DEC-168 | AC-001-002 + AC-001-013 | PASS |
| AC-002 | Single-AID gate; pre-prompt GET; VP-576-002 y/n | AC-001-002-003-010 | PASS |
| AC-003 | Single-AID JSON `{deleted,id}` + human stderr | AC-001-002-003-010 | PASS |
| AC-004 | --yes required bulk; fail-soft 404; non-404 abort | AC-004-005 | PASS |
| AC-005 | Bulk JSON `{count,deleted,ids}` | AC-004-005 | PASS |
| AC-006 | --issue+--older-than; BC-3.9.019 hint strings; zero-match | AC-006-007-011 | PASS |
| AC-007 | parse_age_duration; invalid/multibyte/overflow → exit 64 | AC-006-007-011 + AC-007-016 | PASS |
| AC-008 | --dry-run single-AID (EC-3.9.020-3); no gate/DELETE | AC-008-009 | PASS |
| AC-009 | --dry-run bulk; [ID,Filename,Size,Created] table; metadata fan-out | AC-008-009 | PASS |
| AC-010 | Non-interactive without --yes → exit 64 | AC-001-002-003-010 | PASS |
| AC-011 | --issue+--older-than combined; display_sanitize in dry-run | AC-006-007-011 | PASS |
| AC-012 | e2e surface guard; CLI flags in --help | AC-012-014-015 | PASS |
| AC-013 | VP-576-002 wiremock; DEC-168 body order; mutants.toml | AC-001-002 + AC-001-013 + AC-012-014-015 | PASS |
| AC-014 | --issue without --older-than → exit 2 | AC-012-014-015 | PASS |
| AC-015 | Clap mutual-exclusion constraints exit 2 | AC-012-014-015 | PASS |
| AC-016 | Delete error taxonomy: 403/AID-validation/durations | AC-007-016 | PASS |

**Coverage: 16/16 ACs demonstrated.**

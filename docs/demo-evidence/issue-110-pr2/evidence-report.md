# Evidence Report — issue-110-pr2

**Issue:** #110 — Multi-key bulk operations via Atlassian Bulk API (PR 2 of N)
**Branch:** `develop` (worktree: `.worktrees/issue-110-pr2`)
**Recorded:** 2026-05-10

---

## Demo Coverage

| Demo | File | Path | Criterion |
|------|------|------|-----------|
| D-001 | `D-001-edit-help-new-flags` | [gif](D-001-edit-help-new-flags.gif) / [webm](D-001-edit-help-new-flags.webm) | `jr issue edit --help` documents `--jql`, `--max`, `--yes`, `--dry-run` |
| D-002 | `D-002-dry-run-table` | [gif](D-002-dry-run-table.gif) / [webm](D-002-dry-run-table.webm) | `--dry-run` table output: lists affected keys + planned changes, no HTTP |
| D-003 | `D-003-dry-run-json` | [gif](D-003-dry-run-json.gif) / [webm](D-003-dry-run-json.webm) | `--dry-run --output json`: `{"dryRun":true,"issues":[...],"plannedChanges":{...}}` |
| D-004 | `D-004-no-parent-multi-key-rejected` | [gif](D-004-no-parent-multi-key-rejected.gif) / [webm](D-004-no-parent-multi-key-rejected.webm) | C-1 audit fix: `--no-parent` with multi-key → friendly error + exit 64 |
| D-005 | `D-005-empty-jql-rejected` | [gif](D-005-empty-jql-rejected.gif) / [webm](D-005-empty-jql-rejected.webm) | F1 guard: `--jql ""` → "cannot be empty" error + exit 64 |
| D-006 | `D-006-pr2-tests-all-green` | [gif](D-006-pr2-tests-all-green.gif) / [webm](D-006-pr2-tests-all-green.webm) | 24 PR2 integration tests: `test result: ok. 24 passed` |
| D-007 | `D-007-no-regression` | [gif](D-007-no-regression.gif) / [webm](D-007-no-regression.webm) | 612 lib unit tests: `test result: ok. 612 passed; 0 failed` |

---

## Success Paths

| AC | Command | Expected | Demonstrated by |
|----|---------|----------|----------------|
| Help flags | `jr issue edit --help` | Shows `--jql`, `--max`, `--yes`, `--dry-run` with descriptions | D-001 |
| Dry-run table | `jr issue edit FOO-1 FOO-2 --label add:demo --dry-run --no-input` | `DRY RUN` header + issue list + planned changes, exit 0, no HTTP | D-002 |
| Dry-run JSON | same + `--output json` | `{"dryRun":true,"issues":["FOO-1","FOO-2"],"plannedChanges":{...}}` piped through `jq .` | D-003 |
| PR2 tests | `cargo test --test issue_bulk_pr2 -- --test-threads=1` | `test result: ok. 24 passed; 0 failed` | D-006 |
| No regression | `cargo test --lib` | `test result: ok. 612 passed; 0 failed` | D-007 |

## Error Paths

| AC | Command | Expected | Demonstrated by |
|----|---------|----------|----------------|
| C-1 multi-key `--no-parent` | `jr issue edit FOO-1 FOO-2 --no-parent --no-input` | "Multi-key bulk edit doesn't yet support: --no-parent" + exit 64 | D-004 |
| F1 empty `--jql` | `jr issue edit --jql "" --label add:foo --no-input` | "--jql query cannot be empty" + exit 64 | D-005 |

---

## Reproduction Commands

```bash
# D-001: help text showing new PR2 flags
./target/release/jr issue edit --help 2>&1 | head -35

# D-002: dry-run table (no backend needed — positional keys skip HTTP)
export JR_BASE_URL=http://127.0.0.1:9
export JR_AUTH_HEADER="Basic dGVzdA=="
./target/release/jr issue edit FOO-1 FOO-2 --label add:demo --dry-run --no-input

# D-003: dry-run JSON piped through jq
./target/release/jr issue edit FOO-1 FOO-2 --label add:demo --dry-run --no-input --output json | jq .

# D-004: C-1 audit fix — --no-parent rejected for multi-key
./target/release/jr issue edit FOO-1 FOO-2 --no-parent --no-input; echo "exit $?"

# D-005: F1 guard — empty --jql rejected before any HTTP
./target/release/jr issue edit --jql "" --label add:foo --no-input; echo "exit $?"

# D-006: 24 PR2 integration tests
cargo test --test issue_bulk_pr2 -- --test-threads=1 2>&1 | tail -20

# D-007: lib unit tests (regression gate)
cargo test --lib 2>&1 | tail -5
```

---

## PR2 Scope Summary

New acceptance criteria covered by this demo set:

- **`--jql` flag:** Select issues for bulk edit via JQL query instead of positional keys
- **`--max` flag:** Caps JQL match count (default 50, ceiling 1000); errors without mutation if exceeded
- **`--yes` flag:** Skips interactive confirmation for large JQL match sets
- **`--dry-run` flag:** Preview mode — shows planned changes without executing HTTP mutations
- **C-1 audit fix:** Unsupported flags (`--no-parent`, `--no-points`, `--description-stdin`, `--markdown`) are rejected with a friendly error when more than one key is provided
- **F1 guard:** Empty `--jql ""` is caught early with a descriptive error before any network activity

---

## Notes

- All demos except D-006/D-007 require no live Jira backend — they error or exit clean before any HTTP call.
- D-002/D-003 use `JR_BASE_URL=http://127.0.0.1:9` + `JR_AUTH_HEADER` to satisfy config loading, but `--dry-run` with positional keys makes zero HTTP calls by design.
- D-004/D-005 fail at argument validation, also before any HTTP.
- **Prerequisites for reproduction:** `JR_AUTH_HEADER` is a debug-build-only env seam
  (`#[cfg(debug_assertions)]`). Release builds (`./target/release/jr`) ignore
  `JR_AUTH_HEADER` entirely — `JiraClient::from_config` will consult the macOS keychain
  and fail with a credential-not-found error if `jr auth login` has not been run first.
  For demos that exercise pre-HTTP validation (e.g., `--max 0` rejection, empty `--jql`
  rejection, multi-key flag conflicts), use the **debug build** (`cargo run -- ...`) to
  avoid the keychain prerequisite. D-002/D-003 and D-006/D-007 require either a prior
  `jr auth login` (release) or the debug build with `JR_AUTH_HEADER` set.

# Demo Evidence Report — issue-284

**Issue:** [#284](https://github.com/jaredbrichards/jira-cli/issues/284) — Add `--no-parent` flag to `jr issue edit`
**Branch:** `feat/issue-284-no-parent-flag`
**Base SHA:** `ff00061`
**Recorded:** 2026-05-09

---

## Coverage Map

| Demo | Claim demonstrated | Artifact | Confirmation |
|------|--------------------|----------|--------------|
| D-001 | `--no-parent` flag is registered and described in `--help` output | `D-001-no-parent-in-help.gif` / `.webm` | `grep` extracts `--no-parent  Clear the issue's parent` + 2 context lines |
| D-002 | `--no-parent` and `--parent` conflict (clap `conflicts_with`) — exit 2, error message | `D-002-conflicts-with-parent.gif` / `.webm` | `error: the argument '--no-parent' cannot be used with '--parent <PARENT>'` visible |
| D-003 | All 8 acceptance-criterion integration tests pass | `D-003-all-tests-green.gif` / `.webm` | `test result: ok. 8 passed; 0 failed` shown in tail output |
| D-004 | No regression — all 612 lib unit tests preserved | `D-004-no-regression.gif` / `.webm` | `test result: ok. 612 passed; 0 failed` shown in tail output |

---

## Reproduction Commands

```bash
# Prerequisites: worktree at .worktrees/issue-284/, release binary pre-built
cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-284
cargo build --release

# D-001: flag in help
PATH="./target/release:$PATH" jr issue edit --help 2>&1 | grep -A 2 "no-parent"

# D-002: conflicts_with error (exit 2)
PATH="./target/release:$PATH" jr issue edit FOO-100 --no-parent --parent BAR-200 2>&1
echo "Exit: $?"

# D-003: 8 integration tests green
cargo test --test issue_edit_no_parent 2>&1 | tail -5

# D-004: 612 lib tests no regression
cargo test --lib 2>&1 | tail -5
```

---

## Implementation Notes

**(a) `conflicts_with` declared bidirectionally.** Both `--parent` and `--no-parent` clap args carry
`conflicts_with` pointing at each other, so the error fires regardless of argument order on the command
line.

**(b) Subtask convert-hint via case-insensitive substring match.** When the Jira API returns HTTP 400
and the error string contains the substring `subtask` (case-insensitive), the handler surfaces a
human-readable hint suggesting the user convert the issue type first before clearing the parent.

**(c) `serde_json::Value::Null` flows through existing edit path without struct changes.** The
`--no-parent` flag inserts `{"parent": null}` into the update payload using the existing
`serde_json::Value` map, avoiding any changes to the `EditIssueRequest` struct or the
`edit_issue` API function signature.

---

## Artifacts

| File | Size |
|------|------|
| `D-001-no-parent-in-help.tape` | 505 B |
| `D-001-no-parent-in-help.gif` | 31 KB |
| `D-001-no-parent-in-help.webm` | 30 KB |
| `D-002-conflicts-with-parent.tape` | 519 B |
| `D-002-conflicts-with-parent.gif` | 33 KB |
| `D-002-conflicts-with-parent.webm` | 32 KB |
| `D-003-all-tests-green.tape` | 400 B |
| `D-003-all-tests-green.gif` | 43 KB |
| `D-003-all-tests-green.webm` | 64 KB |
| `D-004-no-regression.tape` | 374 B |
| `D-004-no-regression.gif` | 43 KB |
| `D-004-no-regression.webm` | 81 KB |

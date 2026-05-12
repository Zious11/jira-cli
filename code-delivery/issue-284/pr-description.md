## Summary

Closes #284. Adds `jr issue edit --no-parent` to clear an issue's parent (mirrors the existing `--no-points` precedent). Replaces the documented workaround `jr api /rest/api/3/issue/{key} -X put -d '{"fields":{"parent":null}}'` with first-class CLI ergonomics.

**Closes:** #284

## Architecture Changes

```mermaid
graph TD
    A["cli/mod.rs<br/>Add no_parent: bool flag<br/>conflicts_with = parent"] --> B["cli/issue/create.rs<br/>handle_edit — destructure no_parent<br/>inject Value::Null into fields[parent]<br/>subtask 400 hint"]
    B --> C["api/jira/issues.rs<br/>edit_issue — unchanged<br/>Value::Null flows through existing path"]
    D["clap conflicts_with<br/>bidirectional<br/>parent ↔ no_parent"] --> A
    E["tests/issue_edit_no_parent.rs<br/>8 new wiremock-backed tests"] --> B
```

## Story Dependencies

```mermaid
graph LR
    I284["issue-284<br/>--no-parent flag"] --> NONE["No upstream dependencies<br/>Cargo.lock: minor version bump only"]
```

## Spec Traceability

```mermaid
flowchart LR
    BC["BC: Users need to<br/>clear parent in CLI"] --> AC1["AC-1: --no-parent registered<br/>in jr issue edit --help"]
    BC --> AC2["AC-2: --no-parent + --parent<br/>conflict → exit 2"]
    BC --> AC3["AC-3: PUT sends parent:null<br/>→ 204 No Content"]
    BC --> AC4["AC-4: Subtask 400 surfaces<br/>convert hint"]
    AC1 --> T1["test_no_parent_flag_in_help"]
    AC2 --> T2["test_no_parent_conflicts_with_parent<br/>test_parent_conflicts_with_no_parent"]
    AC3 --> T3["test_no_parent_sends_null<br/>test_no_parent_with_other_fields"]
    AC4 --> T4["test_no_parent_subtask_400_hint"]
    T1 --> D001["D-001-no-parent-in-help.gif"]
    T2 --> D002["D-002-conflicts-with-parent.gif"]
    T3 --> D003["D-003-all-tests-green.gif"]
    T4 --> D003
```

## Changes

| File | Change |
|------|--------|
| `src/cli/mod.rs` | Add `no_parent: bool` to `Edit` variant; bidirectional `conflicts_with` between `parent` and `no_parent` |
| `src/cli/issue/create.rs` | Destructure `no_parent` in `handle_edit`; inject `serde_json::Value::Null` into `fields["parent"]` when set; update "no fields specified" bail message; subtask 400 detection with convert hint |
| `tests/issue_edit_no_parent.rs` | NEW — 8 wiremock-backed integration tests covering help text, JSON null body, mutual exclusion, subtask hint, multi-field combo, and `--output json` success |

## Demo Evidence

| Demo | Claim | Artifact |
|------|-------|----------|
| D-001 | `--no-parent` registered with helpful description | [D-001-no-parent-in-help.gif](../../docs/demo-evidence/issue-284/D-001-no-parent-in-help.gif) |
| D-002 | `--no-parent` + `--parent` conflict yields exit 2 with "cannot be used with" message | [D-002-conflicts-with-parent.gif](../../docs/demo-evidence/issue-284/D-002-conflicts-with-parent.gif) |
| D-003 | All 8 new tests green | [D-003-all-tests-green.gif](../../docs/demo-evidence/issue-284/D-003-all-tests-green.gif) |
| D-004 | No regression on 612 lib tests | [D-004-no-regression.gif](../../docs/demo-evidence/issue-284/D-004-no-regression.gif) |

## Test Evidence

| Metric | Value |
|--------|-------|
| New integration tests | 8 / 8 passed |
| Lib unit tests | 612 / 612 passed (no regression) |
| Clippy | Zero warnings (zero-warnings policy) |
| Stable rustfmt | Clean |
| Nightly rustfmt | Clean |
| Release build | Green |

## Security Review

No new auth surface, no new dependencies, no network endpoints added. This PR adds a single CLI flag that passes `Value::Null` through the existing `edit_issue` API path. Security review: NONE required (flag-only change within existing authenticated API call).

## Risk Assessment

**Blast radius:** LOW. Single new boolean flag with bidirectional `conflicts_with`. Existing `--parent` behavior is unchanged (regression-pinned via existing tests). The `serde_json::Value::Null` path flows through the existing `edit_issue` function without struct changes.

**Performance impact:** NONE. No new API calls, no new cache reads, no new async paths.

**Breaking changes:** NONE. Additive flag only.

## Out of Scope

Subtask conversion (`POST /rest/api/3/issue/{key}/convert`) — separate future story. This PR only detects subtask attempts and surfaces a human-readable hint pointing the user to the conversion endpoint.

## Pre-existing Known Flake

`tests/auth_login_json_test.rs::test_auth_login_emits_json_when_output_json_set` — macOS keychain `item already exists` error. Pre-existing, unrelated to this PR. CI Linux is not expected to reproduce this.

## Research Backing

`.factory/research/issue-284-no-parent-flag.md` — Perplexity-verified claims:
- PUT with `parent: null` returns 204 (verified)
- Subtask parent clear fails 400 (verified)
- Legacy Epic Link replaced by parent field in Feb 2024 (verified)

## AI Pipeline Metadata

| Field | Value |
|-------|-------|
| Pipeline mode | Feature delta (issue-284) |
| Models used | claude-sonnet-4-6 |
| Branch | `feat/issue-284-no-parent-flag` |
| Base SHA | `ff00061` |
| Commits | 3 (test-writer → implementer → demos) |

## Pre-Merge Checklist

- [x] PR description matches actual diff
- [x] All 4 ACs covered by demo evidence (1 recording per AC minimum)
- [x] Traceability chain complete (BC → AC → Test → Demo)
- [x] 8 new integration tests pass
- [x] 612 lib tests preserved (no regression)
- [x] Clippy clean (zero warnings)
- [x] `cargo fmt --all -- --check` passes
- [x] Release build green
- [x] Security review: no new attack surface
- [x] No upstream story dependencies to wait for
- [ ] CI checks green (populated in step 6)
- [ ] Squash-merged (populated in step 8)

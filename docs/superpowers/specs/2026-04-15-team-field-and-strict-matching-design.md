# Strict Name Matching + Team Field on `issue view`

> **Issues:** #181 — `issue create/edit: --team silently dropped when value not in cache`
> **Issues:** #182 — `issue view: team field missing from fetch whitelist`
> **Also closes:** #192 — `refactor: apply exact→prefix→disambiguate matching to user/status/asset/queue resolvers`

## Problem

### #181 — silent mis-resolution (root cause)

`partial_match` currently returns `MatchResult::Exact(first)` when exactly one **substring** match is found (`src/partial_match.rs:40`). This means `--team "Ops"` silently resolves to `"Platform Ops"` if that's the only team containing "Ops" in the cache. The user intends to reference a different team that isn't cached, but the CLI applies the wrong value without warning.

The issue title says "silently dropped" but the actual behavior is silent **mis-resolution** — the PUT succeeds with the wrong team ID. From the user's perspective it reads as dropped because `jr issue view` can't show the team value (issue #182), and filtering `jr issue list --team "<intended>"` returns the same empty result as if the team were never set.

This footgun exists across all eight `partial_match` call sites: team, user, status (move + list filter), link type (link + unlink), queue, asset. Every one of them silently picks a substring match when there's exactly one hit.

### #182 — team field not visible on view

`BASE_ISSUE_FIELDS` (`src/api/jira/issues.rs:12-29`) does not include the team custom field. `handle_view` (`src/cli/issue/list.rs:699`) only adds story points + CMDB fields to `extra_fields`. Even when a team is set correctly, `jr issue view` has no way to display it — the user cannot verify their own `--team` assignments.

## Design

### Core Change — `partial_match` rejects silent substring hits

Change the substring-fallback arm in `src/partial_match.rs`:

```rust
// before
match matches.len() {
    0 => MatchResult::None(candidates.to_vec()),
    1 => MatchResult::Exact(matches.into_iter().next().unwrap()),
    _ => MatchResult::Ambiguous(matches),
}

// after
match matches.len() {
    0 => MatchResult::None(candidates.to_vec()),
    _ => MatchResult::Ambiguous(matches),
}
```

This collapses `1` and `n>1` into the same `Ambiguous` branch. Callers already route `Ambiguous` to an interactive prompt (TTY) or a bail with candidate list (`--no-input`). No new disambiguation code needed.

`MatchResult::Exact` is now only produced by the case-insensitive **exact** match path (`src/partial_match.rs:19-29`). This is the invariant the name is supposed to carry.

Validation: `gh` and `kubectl` both use exact-only resource resolution — silent substring matching is an anti-pattern (Perplexity/Context7 validation, 2026-04-15).

### Impact on call sites

All eight call sites already handle `Ambiguous` correctly — they were designed for the `n>1` case and the same code path handles `n==1` without modification:

| File | Behavior (TTY) | Behavior (`--no-input`) |
|---|---|---|
| `src/cli/issue/helpers.rs` (team) | `dialoguer::Select` prompt | `bail!` with candidate list |
| `src/cli/issue/helpers.rs` (user via `disambiguate_user`) | `dialoguer::Select` prompt | `bail!` with candidate list |
| `src/cli/issue/workflow.rs` (status transition) | numbered `prompt_input` | `bail!` with candidate list |
| `src/cli/issue/list.rs` (status filter) | n/a — no TTY prompt for list filter | `JrError::UserError` with candidate list |
| `src/cli/issue/links.rs` (link + unlink) | `dialoguer::Select` prompt | `bail!` with candidate list |
| `src/cli/queue.rs` (queue resolve x2) | `dialoguer::Select` via existing list-match path | error with candidates |

`list.rs`'s status filter currently errors in all modes (no TTY prompt) — this is existing behavior and we keep it. A status filter is pipe-friendly and prompting there would block scripts.

### User-visible behavior change

Before this change:
```
$ jr issue move FOO-1 "prog"         # TTY or --no-input: silently resolves to "In Progress"
Moved FOO-1 to In Progress
```

After:
```
$ jr issue move FOO-1 "prog" --no-input
Error: Ambiguous transition "prog". Matches: In Progress

$ jr issue move FOO-1 "prog"          # TTY
Ambiguous match for "prog". Did you mean one of:
  1. In Progress
Select (number): 1
Moved FOO-1 to In Progress
```

Single-item prompts are slightly awkward UX but acceptable — the existing `Ambiguous` path renders them without crashing. This is a follow-up refinement candidate, not a blocker.

### Team Field on `issue view` (#182)

Four ordered changes in `src/cli/issue/list.rs` (inside `handle_view`):

1. **Extend `extra_fields`** — after the existing `sp_field_id` and `cmdb_field_id_list` collection, append `config.global.fields.team_field_id` when set.
2. **Read the field value** — after `client.get_issue(&key, &extra).await?`, extract the UUID from `issue.fields.extra.get(team_field_id)` (the `extra: HashMap<String, Value>` field is `#[serde(flatten)]` and already captures all custom fields; the existing `story_points(field_id)` helper uses the same map). Add a sibling helper `team_id(field_id: &str) -> Option<String>` that returns `self.extra.get(field_id)?.as_str().map(String::from)`. Atlassian Teams field returns a bare UUID string per Perplexity validation against `developer.atlassian.com/platform/teams/components/team-field-in-jira-rest-api/`. When the field has never been set, Jira **omits the key entirely** from the response (it is not returned as explicit `null`); `extra.get(field_id)` returns `None` in that case. The rare explicit-`null` case (e.g. from a changelog or webhook expansion) also yields `None` via `.as_str()`, so both are handled by a single helper that returns `Option<String>`.
3. **Resolve UUID → name** via `cache::read_team_cache()` (already populated by `jr team list` / `jr init`). The cache lookup is synchronous and adds no API calls.
4. **Render a row** in the table:

| Scenario | Row |
|---|---|
| Field unconfigured (`team_field_id` is `None`) | row omitted |
| Field absent from response (never set) or explicit `null` | row omitted |
| UUID in cache | `Team: <Name>` |
| UUID not in cache | `Team: <uuid> (name not cached — run 'jr team list --refresh')` |

The row is inserted in the same block that handles story points (around `list.rs:908-915`) for consistency.

JSON output is unchanged — the raw `customfield_<id>` passes through `serde_json::to_value` as before. Agents already parse it.

### Out of scope (tracked separately)

| Item | Issue |
|---|---|
| Auto-refresh team cache on miss + UUID pass-through | #190 |
| Team column in `issue list` output | #191 |
| Refine single-item `Ambiguous` prompt into y/N confirmation | not filed — minor UX polish |

## Files changed

| File | Change |
|---|---|
| `src/partial_match.rs` | Collapse substring `1 => Exact` arm into `Ambiguous` |
| `src/partial_match.rs` tests | Update `test_partial_match_unique` to expect `Ambiguous` (rename → `test_partial_match_single_substring_is_ambiguous`) |
| `src/cli/issue/list.rs` | Add `team_field_id` to `extra_fields`; extract + render team row in `handle_view` |
| `src/types/jira/issue.rs` | Add `team_id(field_id: &str) -> Option<String>` helper to `IssueFields` (mirrors the existing `story_points` helper) |
| `tests/input_validation.rs` | Update `valid_status_partial_match_resolves` to expect `Ambiguous` |
| `tests/cli_handler.rs` or new `tests/issue_view.rs` | Add handler tests: view renders team name when cached, fallback when uncached, row omitted when unconfigured |
| `tests/issue_commands.rs` (if exists) or similar | Add test: `--team "partial-name"` under `--no-input` errors with candidate list; exact match still succeeds |

Anything using substring-match behavior in existing tests surfaces during `cargo test` and updates in the same commit as `partial_match.rs`.

## Error handling

| Scenario | Behavior | Exit |
|---|---|---|
| `partial_match` returns `Ambiguous` with 1 hit, `--no-input` | `bail!` with candidate list, existing message per call site | 1 (anyhow) or 64 (`JrError::UserError`) — unchanged per call site |
| `partial_match` returns `Ambiguous` with 1 hit, TTY | Existing `dialoguer::Select` or `prompt_input` — user picks or aborts | 0 on pick, 130 on Ctrl-C |
| `view` with unconfigured team field | Row omitted, no error | 0 |
| `view` with team UUID not in cache | Row renders with fallback text | 0 |
| `view` JSON output | Raw customfield value in response — unchanged | 0 |

## Testing

**Unit tests** (`src/partial_match.rs`):
- Exact CI match → `Exact` (unchanged)
- Exact CI multiple → `ExactMultiple` (unchanged)
- Substring 0 hits → `None` (unchanged)
- Substring 1 hit → **`Ambiguous([hit])`** (changed)
- Substring n>1 hits → `Ambiguous([hits])` (unchanged)

**Integration tests** (`tests/input_validation.rs`):
- `valid_status_partial_match_resolves` → updated to expect `Ambiguous`
- `ambiguous_status_returns_multiple_matches` → unchanged (already tests n>1)

**Handler tests** (`tests/cli_handler.rs` or dedicated file):
- `issue view` with team field configured + UUID in cache → table row `Team: <Name>` appears, JSON has raw customfield
- `issue view` with team field configured + UUID not in cache → row renders with fallback text
- `issue view` with team field not configured → no team row, no error
- `issue edit --team "<substring>" --no-input` → errors with candidate list, exit 1
- `issue edit --team "<exact-name>" --no-input` → succeeds

**Snapshot tests** (`insta`) — extend existing `issue view` snapshot if present to cover team row.

## Alignment with project conventions

- **Thin client, no abstraction layer** — change is in matching logic and an existing extra_fields extension pattern. No new abstraction.
- **Machine-output-first** — `--output json` unchanged. Team field passes through raw.
- **Non-interactive by default** — strict matching under `--no-input` is the whole point. TTY users get a one-item prompt (acceptable).
- **Idempotent read operations** — `view` remains a pure GET.
- **Breaking change, pre-1.0 tool** — matches the stderr/stdout migration (#134) and fits the documented project vision of being a tighter agentic CLI. Users relying on substring matching in scripts must switch to exact names; the migration path is already documented in error messages.

## Validation

- **Perplexity (2026-04-15):** `gh` uses exact API-level match; `kubectl` uses exact match (the `kubectl get pod nginx` call is literally a GET to `/api/v1/.../pods/nginx` — no client-side fuzzing); silent substring matching flagged as anti-pattern by both.
- **Perplexity (2026-04-15):** Atlassian Teams custom field returns a bare UUID string on `GET /rest/api/3/issue/{key}?fields=customfield_NNNNN`. Read and write shapes are symmetric (same bare string). When the field has never been set, Jira **omits the key entirely** from the response rather than returning `null` — both cases converge to `None` in the helper. Source: `developer.atlassian.com/platform/teams/components/team-field-in-jira-rest-api/`.
- **Perplexity (2026-04-15):** `#[serde(flatten)] HashMap<String, Value>` preserves the distinction — explicit `null` in JSON becomes `Value::Null` in the map; absent keys have no entry. Spec uses `.get(field_id).and_then(|v| v.as_str())` which collapses both cases safely.
- **Perplexity (2026-04-15):** `dialoguer::Select` with a single-item list renders a normal prompt requiring Enter (no auto-select, no error). UX is functional; single-item `Ambiguous` prompts remain acceptable in TTY mode.
- **Perplexity (2026-04-15):** Jira Cloud v3 returns custom field keys in response bodies verbatim in `customfield_NNNNN` form — no lowercase conversion, renaming, or aliasing. `HashMap::get("customfield_10001")` is safe without case-insensitive lookup. Confirmed by Context7-indexed example at `developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-fields/` (field creation response echoes `"id": "customfield_10101"` / `"key": "customfield_10101"` — same form used in issue GET responses).
- **Perplexity (2026-04-15):** CLI convention for single-candidate "did you mean" disambiguation favors scriptable suggestion + bail under `--no-input`, prompt-if-you-can in TTY — which is exactly the existing `Ambiguous` branch behavior this spec routes single-hit substrings through. No new interaction pattern needed.
- **Codebase audit:** all 8 `partial_match` call sites already route `Ambiguous` through the correct TTY/`--no-input` branches. No new wiring needed.
- **Test audit:** only `test_partial_match_unique` (unit) and `valid_status_partial_match_resolves` (integration) assert the old `1 => Exact` behavior. Both update in the same commit as the core fix.

# `jr issue move --resolution` — atomic status + resolution transitions

**Issue:** [#263](https://github.com/Zious11/jira-cli/issues/263)
**ADR:** [ADR-0015](../adr/0015-proactive-resolution-enforcement.md)

## Problem

`jr issue move` calls `POST /rest/api/3/issue/{key}/transitions` with only a transition ID. On JSM workflows (and classic Jira workflows where resolution lives on the "Done" screen), this leaves tickets in a half-resolved limbo:

- Status → "Resolved" / "Closed" / "Done"
- `resolution` field → null
- `resolutionDate` → null

Consequences: JQL filter `resolution IS EMPTY` still matches, SLAs don't stop, "recently resolved" automations miss the ticket, and reporters see it as open on the JSM portal even though agents see it as done.

The two-step workaround (transition then PUT resolution) is broken: a direct `PUT` of the `resolution` field does NOT trigger `resolutionDate`, so downstream time-based automations stay broken.

## Atlassian constraint (validated)

Confirmed via Atlassian's developer docs:

- `POST /rest/api/3/issue/{key}/transitions` accepts both `transition` AND `fields` in the request body.
- Passing `{"transition":{"id":"..."},"fields":{"resolution":{"name":"Done"}}}` atomically transitions the status, sets `resolution`, and fires the `resolutionDate` timestamp.
- `GET /rest/api/3/resolution` returns the instance-scoped resolution list (company-managed classic projects; team-managed projects don't use resolution). Each entry has `id`, `name`, `description`.
- Atlassian's KB explicitly states: "Issue transition API is not expected to respect the screens." Screen-only resolution requirements are silently bypassed by the API — a 400 never fires unless a workflow **validator** (not merely a screen field) backs the requirement. See `.factory/research/jsm-resolution-required-api-validation.md` (Claim 3, CONFIRMED).

## Design

### CLI surface

```
jr issue move <KEY> <STATUS> --resolution <NAME>
jr issue move <KEY> <STATUS> --no-resolution
jr issue resolutions [--refresh]
```

- `--resolution <name>` on `jr issue move` sets the resolution atomically in the transition POST body. Name is matched case-insensitively via `resolve_resolution_by_name` (exact > prefix > substring). Prefix/substring ambiguity exits 64 with the candidate list — same convention as other `--no-input`-first resolvers. This flag is now required (or `--no-resolution` must be passed) for done-category transitions that offer a resolution field. See "Proactive enforcement" below.
- `--no-resolution` is the explicit opt-out. When passed, proactive enforcement is skipped and the transition is sent without a resolution field — identical to the previous default behavior. Mutually exclusive with `--resolution`. Required for scripts that intentionally close issues without a resolution (e.g., "Won't Do" paths where no resolution is desired). This flag is the **breaking-change migration path** for existing scripts.
- `jr issue resolutions` lists the cached resolutions. Table output by default, `--output json` for scripts.
- `--refresh` busts the resolution cache.

### Breaking change (proactive enforcement)

**This is a breaking change introduced in the feature cycle following initial `--resolution` support.**

Previously: `jr issue move KEY Done` on a workflow where resolution lives on the Done screen
succeeded silently with `resolution=null`.

Now: `jr issue move KEY Done` on a done-category transition that offers a resolution field (or
has `isConditional=true`) will:

- **Interactive (TTY, not `--no-input`):** prompt for a resolution via `dialoguer::Select`, then
  proceed with the chosen resolution set atomically in the transition body.
- **Non-interactive (`--no-input` or stdin not a TTY):** exit 64 with:
  ```
  The transition to "<to_label>" requires a resolution.

  Try:
      jr issue move KEY <to_label> --resolution <name>

  Run `jr issue resolutions` to see available values.
  ```

Scripts that previously relied on the silent bypass must add either `--resolution <name>` or
`--no-resolution`. The `--no-resolution` flag is the zero-friction migration path when a
resolution is genuinely not desired.

### Proactive enforcement design

Enforcement happens via `?expand=transitions.fields` on the existing transitions GET — no
additional round-trip. The flow in `handle_move` (single-key only; bulk is excluded):

```
1. get_transitions_with_fields(key)  →  resolve selected_transition
   [replaces get_transitions; same round-trip count, ?expand=transitions.fields added]
2. get_issue                          →  idempotency check  [unchanged]
3. Detect:
     is_done_category    = to.statusCategory.key == "done"
     offers_resolution   = fields.contains_key("resolution")
     is_conditional      = transition.is_conditional == Some(true)
     needs_resolution    = is_done_category && (offers_resolution || is_conditional)
4a. if needs_resolution && --resolution absent && --no-resolution absent:
       no_input → exit 64 with hint  (BC-3.2.013)
       TTY      → dialoguer::Select prompt, use chosen value  (BC-3.2.013)
4b. if --resolution provided → resolve_resolution_by_name → set in body  (BC-3.2.011)
4c. if --no-resolution provided → skip enforcement, send no resolution field  [opt-out]
5. transition_issue(fields)           [unchanged]
6. 400 "resolution required" handler  [BC-3.2.009 backstop, preserved]
```

**Detection trigger:** `to.statusCategory.key == "done"` is the stable, instance-independent,
lowercase machine key for the Done category. NOT the status name. The `isConditional` OR-clause
catches validator-only requirements that do not appear on the transition screen.

**Conservative fallback:** If `to.statusCategory` is absent from the response (defensive
deserialization returns `None`), `is_done_category` is `false` and enforcement is skipped.
The reactive BC-3.2.009 path handles any resulting 400.

**BC-3.2.013:** The full behavioral contract (exit codes, error messages, interactive prompt
shape, allowedValues validation) is specified in `bc-3-issue-write.md` § BC-3.2.013.

### Read-command stability contract

`jr issue transitions --output json` output is byte-for-byte unchanged by this feature.

The `Transition` struct gains:
```rust
#[serde(default, skip_serializing)]
pub fields: Option<std::collections::HashMap<String, serde_json::Value>>,
```

`skip_serializing` means the field is populated when the API returns it (via
`get_transitions_with_fields`) but is never emitted in JSON serialization. The read command
(`handle_transitions`) continues to call `get_transitions` (no expand) and serializes the same
output shape as today. See ADR-0015 §3 for the full rationale and the future-extension rule.

### API changes

- `src/api/jira/resolutions.rs` (new): `JiraClient::get_resolutions() -> Result<Vec<Resolution>>` calling `GET /rest/api/3/resolution`.
- `src/api/client.rs` (or `src/api/jira/issues.rs`): `get_transitions_with_fields(key) -> Result<TransitionsResponse>` calling `GET .../transitions?expand=transitions.fields`. Called by `handle_move` only.
- `src/api/jira/issues.rs`: `transition_issue` gains an optional `fields: Option<&serde_json::Value>` argument. When `Some`, merges into the request body alongside `transition`. Call sites that don't need resolution pass `None`.
- `src/types/jira/issue.rs`: `Transition` gains `#[serde(default, skip_serializing)] pub fields: Option<HashMap<String, serde_json::Value>>` and `pub is_conditional: Option<bool>`.
- `src/types/jira/` or inline: `Resolution { id, name, description }`.
- `src/cli/issue/workflow.rs`: `handle_move` uses `get_transitions_with_fields`; detection + enforcement block inserted between transition resolution and the POST.

### Cache

`~/.cache/jr/resolutions.json`, 7-day TTL matching existing team / workspace / cmdb_fields caches. `read_resolutions_cache()` / `write_resolutions_cache()` follow the existing `read_cache` / `write_cache` helpers in `src/cache.rs`. Resolution list is fetched by the interactive prompt path via `load_resolutions(client, false)` — same call as the `--resolution` flag path.

### Partial-match

`resolve_resolution_by_name(&[Resolution], query) -> Result<Resolution>` using the existing `partial_match` crate (`src/partial_match.rs`). Exact > prefix > substring. `Ambiguous` and `ExactMultiple` branches surface the candidate list via `JrError::UserError` (exit 64) to match existing `jr issue move` status-disambiguation UX. The interactive prompt path uses `dialoguer::Select` directly (no partial-match ambiguity — user picks from a pre-listed set).

### Error paths

**Proactive enforcement (primary path — BC-3.2.013):**
Before the POST, when `needs_resolution` is true and neither `--resolution` nor `--no-resolution`
is supplied:
- Non-interactive: exit 64, stderr: `The transition to "<to_label>" requires a resolution.` followed by a `Try:` hint line and `Run \`jr issue resolutions\`` suggestion (exact wording from `workflow.rs::handle_move` REQUIRED branch — load-bearing substrings: `"requires a resolution"`, `"--resolution"`).
- Interactive: `dialoguer::Select` prompt listing resolutions from `load_resolutions(client, false)`. On Ctrl+C / prompt error, exit non-zero.

**Reactive backstop (BC-3.2.009 — preserved):**
When Atlassian returns 400 on the transition POST containing "resolution is required" (validator-enforced workflows, or any case the proactive gate missed):
```
error: the "Done" transition requires a resolution.

Try:
    jr issue move <KEY> Done --resolution <name>

Run `jr issue resolutions` to see available values.
```
Heuristic: look for "resolution" (case-insensitive) and "required" in the Atlassian error body. If both present, transform. Otherwise pass through the original error.

**`--no-resolution` with wrong transition type:**
`--no-resolution` is accepted silently on non-done-category transitions (no-op). It only changes
behavior when the enforcement gate would have fired.

### Testing

- `src/api/jira/resolutions.rs`: wiremock integration test for the endpoint wrapper.
- `src/cache.rs`: round-trip + missing-file tests for `ResolutionsCache`. TTL expiry is covered generically by the shared `read_cache` path.
- `src/cli/issue/workflow.rs`: handler tests on `resolve_resolution_by_name` — exact match, case-insensitive exact, ambiguous (substring) returns exit 64, no match returns exit 64 with candidate list, multiple exact duplicates lists only the colliding entries (with ids for disambiguation).
- `tests/issue_resolution.rs` or `tests/issue_move_resolution_enforce.rs` (new — F4):
  - `test_move_refuses_done_category_with_resolution_field_no_input` — mock GET with expanded transitions returning done-category + `fields.resolution`; no `--resolution`; `--no-input`; assert exit 64 + `"--resolution"` in stderr; assert POST was NOT called (expect(0)).
  - `test_move_proceeds_no_enforcement_when_no_status_category` — mock without `statusCategory` on `to`; assert 204 POST.
  - `test_move_proceeds_no_enforcement_when_not_done_category` — mock with `statusCategory.key == "indeterminate"`; assert 204 POST.
  - `test_move_proceeds_no_enforcement_when_fields_absent` — done-category target, `fields` map absent; assert 204 POST.
  - `test_move_no_resolution_flag_skips_enforcement` — done-category + `fields.resolution`; `--no-resolution` passed; assert 204 POST with no resolution in body.
- `tests/`: integration test for the reactive BC-3.2.009 path — mount a wiremock transition endpoint that 400s with "Field 'resolution' is required", assert jr's exit 1 message mentions `--resolution` and `jr issue resolutions`.
- `jr issue resolutions`: table output test, JSON output test. `--refresh` behavior is exercised via `load_resolutions(client, refresh)`.
- E2E (`tests/e2e_live.rs`): `test_e2e_jsm_resolution_enforcement` inverts the bypass-demo branch — `jr issue move KEY <done> --no-input` asserts exit 64 + stderr contains `"--resolution"` (BC-3.2.013 gate fired), not exit 0.

### Out of scope

- **Bulk `jr issue move`:** proactive enforcement applies to single-key `handle_move` only. Bulk path retains BC-3.2.009 reactive backstop. Users running bulk done-category moves should supply `--resolution` upfront. See ADR-0015 §6.
- Generic `--field key=value` pass-through — explicitly deferred. Resolution is the 95% case; the long tail (fix version, component, custom workflow fields) can land in a follow-up.
- Per-project resolution discovery — Atlassian's API only exposes instance-scoped resolution listing. A transition-screen-specific list isn't available publicly. Acceptable — most instances have 5–10 resolutions max.

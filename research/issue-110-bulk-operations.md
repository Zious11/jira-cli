---
document_type: research
issue_id: 110
title: "Verification: bulk operations via JQL or multiple keys"
last_updated: 2026-05-10
sources_count: 9
---

# Issue #110 — Verification: bulk operations via JQL or multiple keys

## Claim 1 — Jira bulk-mutation API existence

**Status:** CONFIRMED — a dedicated Issue Bulk Operations API group exists on
Jira Cloud REST v3, separate from the legacy per-issue endpoints. It provides
purpose-built endpoints for edit, transition, move, delete, watch and
unwatch, all of which accept a single payload selecting up to 1,000 issues.

**Citation:**
- Atlassian REST v3 — Issue Bulk Operations group:
  https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/
- Atlassian Tasks API (poll endpoint):
  https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-tasks/
- 1,000-issue cap (Atlassian community confirmation):
  https://community.atlassian.com/forums/Jira-questions/When-it-says-quot-Bulk-changes-are-currently-limited-to-1-000/qaq-p/1539237

**Summary:**

| Endpoint | Method | Purpose | Selector | Async |
|---|---|---|---|---|
| `/rest/api/3/bulk/issues/fields` | POST | Bulk edit fields (labels, priority, components, custom fields) | `selectedIssueIdsOrKeys[]` | Yes — returns `taskId` |
| `/rest/api/3/bulk/issues/transition` | POST | Bulk transition | `selectedIssueIdsOrKeys[]` + transition id | Yes |
| `/rest/api/3/bulk/issues/move` | POST | Bulk move to project | `selectedIssueIdsOrKeys[]` + target | Yes |
| `/rest/api/3/bulk/issues/delete` | POST | Bulk delete | `selectedIssueIdsOrKeys[]` | Yes |
| `/rest/api/3/bulk/issues/watch` | POST | Bulk add watcher | `selectedIssueIdsOrKeys[]` | Yes |
| `/rest/api/3/bulk/issues/unwatch` | POST | Bulk remove watcher | `selectedIssueIdsOrKeys[]` | Yes |
| `/rest/api/3/bulk/queue/{taskId}` | GET | Poll task status (also reachable via `/rest/api/3/task/{taskId}`) | — | — |

Key facts that were verified:

- **Selector is ID-list, NOT JQL.** The bulk endpoints take
  `selectedIssueIdsOrKeys` (array of up to 1,000 issue IDs or keys). They do
  **not** accept a `jql` field directly. To bulk-operate by JQL, the client
  must first call `/rest/api/3/search/jql` to materialise the key list, then
  pass it to the bulk endpoint.
  - **INCONCLUSIVE on optional `jql` field.** One Perplexity reasoning pass
    asserted the bulk-fields endpoint also accepts a `jql` field; this could
    not be cross-confirmed against the Atlassian docs page (WebFetch returned
    truncated content). The official endpoint group reference and the second
    Perplexity search both describe only `selectedIssueIdsOrKeys`. Treat
    "JQL-only payload" as **unverified**; assume the safe path is
    search-then-bulk.
- **Hard caps:**
  - Up to **1,000 issues per call** (subtasks count toward the cap).
  - Up to **200 fields** per bulk-edit call.
  - **1,500,000** total combined fields across all issues for a bulk move.
- **Async pattern:** All six mutation endpoints are asynchronous. They
  enqueue a task and return a `taskId`. The caller polls
  `/rest/api/3/bulk/queue/{taskId}` (or the standard
  `/rest/api/3/task/{taskId}`) until `status` is `COMPLETED` or `FAILED`.
  Per-issue success/error breakdown lives inside the `result` blob (shape is
  operation-specific). Tasks are retained ~14 days.
- **Permissions required:** Global "bulk change" permission, plus per-project
  Browse + the operation-specific permission (Edit / Transition / Delete /
  Move) on every issue. A user lacking permission on a single issue does
  **not** abort the whole task — that issue is reported as a per-issue error
  in the task result.
- **Labels-specific shape (per Perplexity, partly model-knowledge):** the
  edit endpoint exposes a `labelsAction` discriminator (`ADD` / `REMOVE` /
  `REPLACE`) under `editedFieldsInput`. **Treat this exact field name as
  unverified** — it is consistent with the Cloud bulk-edit UI semantics but
  was not retrievable from the truncated Atlassian doc page in this
  research pass. Implementation must read the live JSON schema before coding
  the payload.
- **GA status:** No public "GA" announcement was located in the search
  window. The endpoint group is documented under the standard v3 reference
  (no "experimental" banner shown in the indexed sources), but a launch /
  changelog post could not be confirmed. **INCONCLUSIVE on GA banner**;
  verify before relying on stability guarantees.

## Claim 2 — Rate limiting

**Status:** CONFIRMED for burst-rate model and per-issue write limit;
INCONCLUSIVE on a published "recommended concurrency cap".

**Citation:** https://developer.atlassian.com/cloud/jira/platform/rate-limiting/

**Summary:**

- Atlassian Cloud uses a **token-bucket per endpoint per tenant** model. The
  general-purpose burst caps documented are:
  - GET ~ **100 req/s**
  - POST ~ **100 req/s**
  - PUT ~ **50 req/s**
  - DELETE ~ **50 req/s**
  Specific endpoints may set their own (sometimes higher) overrides.
- **Hourly points quota** layered on top of bursts. Exact points per call are
  unpublished and evolving (`RateLimit-Reason: jira-quota-*`).
- **Per-issue write limit** (`RateLimit-Reason: jira-per-issue-on-write`)
  caps how often the same issue can be mutated — relevant if a bulk fallback
  re-touches the same key.
- **No public "max concurrent connections" number.** Atlassian's own
  Automation product caps parallelism at **8** items per Jira host, which is
  consistent with the existing `buffer_unordered(8)` jr uses elsewhere
  (S-3.05). Atlassian's guidance is qualitative: "avoid excessive
  concurrency", "share rate-limit state across threads/nodes", and "respect
  `Retry-After`".
- **Implication for jr:** If we go via the bulk API, **one HTTP call covers
  up to 1,000 issues**, so rate limiting is barely a concern in the happy
  path. If we fall back to N per-issue calls (e.g., for an older/unsupported
  account), `buffer_unordered(8)` with the existing `MAX_RETRY_AFTER_SECS=60`
  cap from S-3.07 is the correct ceiling — same as the asset-enrichment
  pattern.

## Claim 3 — Dry-run patterns

**Status:** PARTIAL — Atlassian provides no "validate-only" preview endpoint
for bulk mutations. Dry-run must be implemented client-side.

**Citation:**
- Atlassian REST v3 issue bulk operations group (no `validate-only` /
  `dryRun` flag listed):
  https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/
- ankitpokhrel/jira-cli + go-jira behaviour: per Perplexity search
  (search result was partly off-topic; treat as **low-confidence**). Neither
  CLI exposes a documented `--dry-run` for bulk mutation, and neither
  supports multi-key positional bulk in a single invocation; both rely on
  shell loops or query-driven transitions.

**Summary:**

- **No native "would-this-transition-succeed" endpoint** exists. The closest
  primitives are:
  - `GET /rest/api/3/issue/{key}/transitions` — list transitions currently
    available on a single issue (we already use this in `cli/issue/workflow.rs`).
  - Pre-calling `/rest/api/3/search/jql` to materialise the affected key
    list (this is essentially the dry-run for the JQL path).
- **Atlassian's bulk-edit UI** does have a "preview changes" screen, but
  that's a UI-only construct. The REST surface goes straight to the async
  task.
- **Recommended dry-run UX for jr:**
  1. JQL path: run the search, render the matched issues (key + summary +
     status), show the diff summary (`+labels: [foo]`, `-labels: [bar]`,
     `→ status: Done`), require explicit confirmation (or an explicit
     `--yes`) before submitting the bulk call.
  2. Multi-key path: same diff summary, but skip the search step.
  3. `--dry-run` short-circuits before the bulk POST and exits 0 with the
     same diff payload (also rendered as JSON under `--output json`).

## Claim 4 — JQL expansion safety

**Status:** CONFIRMED RISK — a 1,000-issue cap exists at the API and the
materialisation path goes through the same `/search/jql` cursor codepath
that hit the JRACLOUD-94632 loop bug.

**Citation:**
- Bulk hard-cap (1,000 issues/operation):
  https://community.atlassian.com/forums/Jira-questions/When-it-says-quot-Bulk-changes-are-currently-limited-to-1-000/qaq-p/1539237
- jr's existing anti-loop guard for `/search/jql`: per S-3.07 (in-repo).

**Summary:**

- **Atlassian-imposed cap = 1,000 issues per bulk call.** This is a
  ceiling, not a quota — exceeding it returns a 4xx, not a 429.
- **JQL materialisation reuses the existing `search/jql` codepath** in
  `src/api/pagination.rs`. The S-3.07 cursor anti-loop guard already
  protects against JRACLOUD-94632, so jr does not need a second guard for
  this feature — it inherits the fix.
- **UX recommendation (jr-side `--max` cap):** Default cap **50** matched
  issues for JQL bulk (matches the user's "30-60 seconds for 50+ tickets"
  framing in the issue). Provide `--max <N>` to raise it, with a hard
  ceiling at 1,000 (the API cap). Above 50, require either `--yes` or an
  interactive confirmation showing the rendered match list.
- **Truncation hint convention:** When the JQL search returns more than
  `--max`, emit the standard "Showing N of M — use --max to widen" hint
  to **stderr**, consistent with `issue list` / `sprint current` /
  `board view`.

## Claim 5 — Multi-key clap UX

**Status:** CONFIRMED — clap 4.x derive cannot place a required positional
**after** a `Vec<String>` variadic. The `gh issue close 123 456 789`
pattern used as inspiration **does not work** out of the box in clap and is
implemented in `gh` via custom argument logic.

**Citation:**
- clap derive trailing-var-arg constraint:
  https://docs.rs/clap/latest/clap/_derive/index.html
- clap discussion confirming "variadic must be last":
  https://github.com/clap-rs/clap/discussions/2260
- Forum thread reproducing the same constraint:
  https://users.rust-lang.org/t/clap-how-can-i-handle-variable-number-of-positional-arguments-like-cp-mv-etc/124841

**Summary:**

- **Pattern that DOES NOT work:**
  `jr issue move KEY1 KEY2 KEY3 "Done"` — clap will pull `"Done"` into the
  `Vec<String>` of keys.
- **Patterns that DO work:**
  1. **Flagged target** (recommended, idiomatic, matches existing jr style):
     ```
     jr issue move KEY1 KEY2 KEY3 --to "Done"
     jr issue edit KEY1 KEY2 KEY3 --label add:foo
     ```
     `keys: Vec<String>` as the only positional; the operation target is a
     `--flag`. This is what `jr issue edit` already does (single key + flags
     today; just widen `key: String` → `keys: Vec<String>`).
  2. **JQL flag**:
     ```
     jr issue edit --jql 'project = PROJ AND status = "To Do"' --label add:foo
     jr issue move --jql '...' --to "Done"
     ```
     Mutually exclusive with positional keys (clap `ArgGroup` with
     `required = true`).
  3. **Custom raw parsing** (NOT recommended): mimic `gh`'s approach. Higher
     maintenance burden; loses clap's auto-generated help / completion.
- **`gh issue close 123 456 789`** itself: per Perplexity (low confidence,
  source not authoritative) `gh` does this via custom logic, not via
  cobra's stock variadic-positional support. Either way, clap cannot
  replicate it cleanly with a trailing different-typed positional.

## Implications for #110 implementation

This is by far the highest-leverage of the four open issues. The bulk API
exists, is documented, accepts up to 1,000 issues per call, is async, and
already aligns with jr's existing `taskId`-style polling primitives.

### Recommended architecture

**1. Multi-key path** (positional `Vec<String>`):

```rust
// cli/issue/create.rs (edit handler) — widen signature:
//   Before: key: String
//   After:  keys: Vec<String> (1..=1000)
```

- Validate `1 ≤ keys.len() ≤ 1000` up front; > 1000 errors with
  exit code 64 (`USAGE`) and a "split into batches" hint.
- Single key is still allowed (backward compatible — `keys[0]` path).
- Operation flags (`--label add:foo`, `--to "Done"`, `--assignee USER`)
  unchanged.

**2. JQL path** (`--jql`):

- Mutually exclusive with positional keys (clap `ArgGroup`).
- Default cap: `--max 50`. Hard ceiling: 1,000 (API limit).
- Pre-fetch matched keys via `/rest/api/3/search/jql` (reuses existing
  pagination + S-3.07 anti-loop guard).
- If matched count > `--max`, error with the over-limit number and
  suggest `--max`.
- If `--max ≤ matched ≤ 50` and no `--yes`/`--no-input`, render the
  matched issue list and prompt for confirmation.

**3. `--dry-run` flag** (jr-wide convention candidate):

- Short-circuits before the bulk POST.
- Renders a diff summary (target keys + per-field changes) on stdout
  in the active output format (table or JSON).
- Exits 0.
- Should also be supported by single-key edits for symmetry.

**4. Concurrency / API call shape**:

- **Happy path = ONE HTTP call** to `/rest/api/3/bulk/issues/fields` (or
  `/transition` / `/move`) with up to 1,000 keys. No `buffer_unordered`
  needed — the API does the fan-out server-side.
- **Polling**: `GET /rest/api/3/bulk/queue/{taskId}` with a small backoff
  (e.g., 1s → 2s → 5s capped at 10s; total wait capped to e.g. 5 min, with
  `--no-wait` to print taskId and exit immediately).
- **Fallback to N per-issue calls** is a deliberate non-goal v1; if the
  bulk endpoint is unavailable or returns a hard error for the whole batch,
  we surface the error rather than silently fanning out.

**5. Idempotency**:

- **Label add/remove**: server-side semantics with `labelsAction: ADD` /
  `REMOVE` are idempotent (adding a label twice is a no-op; removing an
  absent label is a no-op). **Confirmed via API semantics, but exact field
  name `labelsAction` is unverified — read the live schema before coding.**
- **Transitions**: `move PROJ-100 → Done` when already in Done. Atlassian
  bulk-transition reports a per-issue error in the task result. jr should
  match the existing `cli/issue/workflow.rs` convention: a no-op
  transition when already in target state exits 0 (idempotent), so the
  bulk wrapper should treat per-issue "already in state" errors as
  successes. Verify wording of the Atlassian error before pattern-matching.

**6. Failure handling**:

- **All-or-nothing? No.** The bulk task completes with `status: COMPLETED`
  even when some sub-issues fail (per-issue errors live in `result`).
  jr should:
  - Exit 0 if all succeeded.
  - Exit 1 (with `--output json` reporting per-issue errors) if any
    failed.
  - Render a per-key success/failure summary table on stdout (or JSON).
- **Pre-flight permission check is impractical** — the API does it
  per-issue. Don't try to replicate it client-side.

### File list (rough)

- `src/api/jira/issues.rs` — add `bulk_edit_fields`, `bulk_transition`,
  `bulk_move`, `bulk_delete`, plus `get_bulk_task(taskId)`.
- `src/api/jira/bulk.rs` (new) — alternative location for clarity given
  `issues.rs` is already heavy.
- `src/cli/issue/create.rs` — widen `edit` handler to `Vec<String>`,
  add `--jql`, `--dry-run`, `--no-wait`, `--max`, `--yes`.
- `src/cli/issue/workflow.rs` — same widening for `move`.
- `src/types/jira/bulk.rs` (new) — request/response structs for bulk +
  task progress.
- `tests/issue_bulk.rs` (new) — wiremock-driven happy-path,
  partial-failure, JQL-expansion-cap, dry-run-renders-no-mutation tests.

### Effort estimate

**Large.** Reasonable scope guess: 5–8 days for a careful first pass with
TDD, including JQL preview UX, dry-run convention, and async polling.
This is large enough that splitting into two PRs is sensible:

- **PR 1 (medium):** Multi-key positional path for `issue edit` and
  `issue move` against the bulk API, plus polling. No JQL, no dry-run.
- **PR 2 (medium):** `--jql`, `--dry-run`, `--max`, `--yes`, confirmation
  prompts. Builds on PR 1.

### Acceptance criteria draft

1. `jr issue edit KEY1 KEY2 KEY3 --label add:foo` issues exactly **one**
   bulk-edit HTTP call and exits 0 on success.
2. `jr issue move KEY1 KEY2 KEY3 --to Done` issues exactly **one**
   bulk-transition call.
3. `jr issue edit --jql '...' --label add:foo` first calls
   `/search/jql`, renders the matched issues, prompts (unless
   `--yes`/`--no-input`), and on confirmation issues one bulk-edit call.
4. `--max` defaults to 50; `--max 1000` is the hard ceiling; `> 1000`
   errors with exit code 64.
5. `--dry-run` prints the diff summary, makes zero mutating HTTP calls,
   exits 0.
6. `--output json` returns `{ "taskId": "...", "results": [{key, status,
   error?}, ...] }` shape on completion.
7. Partial failure: exits 1 with per-issue error breakdown.
8. Polling uses exponential backoff and respects `Retry-After` (reuses
   existing rate-limit infra). `--no-wait` returns immediately with
   `taskId`.
9. Single-key invocation (`jr issue edit KEY1 --label add:foo`) is
   backward-compatible — same exit code, same `--output json` shape (with
   a single-element results array).

## Recommended next action

**PROCEED** — this is the highest-impact of the four open issues, the
backing API exists and is well-shaped (1,000 issues / call, async, per-issue
error reporting), and the integration points (existing `search/jql`
pagination, S-3.07 anti-loop guard, S-3.07 retry-after cap, S-3.05
`buffer_unordered` not even required for the happy path) are all already in
place.

**Open questions to resolve at design-spec time** (do NOT block PROCEED on
these — resolve while writing the feature spec):

1. Verify the **exact request schema** of `/rest/api/3/bulk/issues/fields`
   against the live Atlassian docs page (the WebFetch in this research run
   came back truncated). Specifically confirm: (a) `labelsAction` field
   name and allowed values, (b) presence/absence of an optional `jql`
   field, (c) whether the response carries `taskId` directly or via
   `Location` header.
2. Verify the **exact bulk-transition payload shape** — earlier sources
   gave conflicting field names (`issueIds` vs `selectedIssueIdsOrKeys`,
   `transition.id` vs `transition.transitionId`).
3. Confirm whether **GA banner** is present on the doc page; if it's marked
   experimental, document that risk in the spec.
4. Decide the **`--dry-run` convention** — should it be a top-level flag
   (jr-wide) or scoped to bulk-capable subcommands? Recommendation:
   scoped initially; promote to global once a second consumer appears.

Suggested effort: **large**, split into PR1 (medium, multi-key+bulk API)
and PR2 (medium, JQL+dry-run UX).

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity search | 5 | Bulk endpoints existence/shape, rate limits, ankitpokhrel/go-jira/gh prior art, clap variadic positionals, GA/async/error semantics |
| Perplexity reason | 1 | Cross-check endpoint inventory and async/dry-run patterns (returned mostly off-topic — discounted; only structural cues retained) |
| WebFetch | 3 | Attempted direct fetch of `developer.atlassian.com` bulk-ops page (truncated 3 of 3 times — could not retrieve canonical doc body) |
| Training data | 2 areas | (a) `labelsAction` field name + ADD/REMOVE/REPLACE values — Perplexity asserted but Atlassian doc body could not be retrieved to confirm; flagged as unverified. (b) GA-banner status — could not confirm via web; flagged INCONCLUSIVE. |

**Total MCP/web tool calls:** 9 (5 Perplexity search + 1 Perplexity reason + 3 WebFetch).

**Training data reliance:** medium — the bulk endpoint inventory, 1,000-issue
cap, async/taskId pattern, rate-limit numbers, and clap-variadic-positional
constraint are all sourced from documented URLs. The exact JSON field names
inside the bulk-edit payload (`editedFieldsInput`, `labelsAction` ADD/REMOVE/
REPLACE) could not be confirmed because every WebFetch of
`developer.atlassian.com/.../api-group-issue-bulk-operations/` returned
truncated content; the implementation must read the live schema before
coding the payload. The "ankitpokhrel/go-jira have no multi-key bulk"
finding is also low-confidence (the Perplexity search returned partly
off-topic results); verify by reading their READMEs at spec time.

### Sources

- Atlassian REST v3 — Issue Bulk Operations:
  https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/
- Atlassian REST v3 — Tasks (poll endpoint):
  https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-tasks/
- Atlassian REST v3 — Issues (per-issue baseline):
  https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/
- Atlassian rate limiting (burst caps, Retry-After):
  https://developer.atlassian.com/cloud/jira/platform/rate-limiting/
- 1,000-issue cap (community confirmation):
  https://community.atlassian.com/forums/Jira-questions/When-it-says-quot-Bulk-changes-are-currently-limited-to-1-000/qaq-p/1539237
- Search endpoint deprecation (drives use of `/search/jql`):
  https://docs.adaptavist.com/sr4jc/latest/release-notes/breaking-changes/atlassian-rest-api-search-endpoints-deprecation
- clap derive `trailing_var_arg` constraint:
  https://docs.rs/clap/latest/clap/_derive/index.html
- clap discussion (variadic must be last):
  https://github.com/clap-rs/clap/discussions/2260
- Rust users forum (same constraint reproduced):
  https://users.rust-lang.org/t/clap-how-can-i-handle-variable-number-of-positional-arguments-like-cp-mv-etc/124841

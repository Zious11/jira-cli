# ADR-0015: Proactive Resolution Enforcement on Done-Category Transitions

## Status
Accepted

## Context
`jr issue move KEY <target>` calls `POST /rest/api/3/issue/{key}/transitions`. On JSM workflows
and classic Jira workflows where the Resolution field lives on the transition's Done screen, this
previously left the issue in a half-resolved state: status set, but `resolution=null` and
`resolutionDate=null`. JQL filter `resolution IS EMPTY` still matched, SLAs did not stop, and
reporters saw the issue as open on the JSM portal.

The existing reactive path (BC-3.2.009) catches `400 Bad Request` responses that contain
"resolution is required" and turns them into a user-facing hint. However, this only fires when
the Atlassian workflow has a **validator** ("Field Required Validator" or "Resolution required").
Many JSM workflows put Resolution on the transition screen without a validator — in that
configuration, the API bypasses the screen requirement silently and the 400 path never fires.

Atlassian's own Knowledge Base documents this explicitly:

> "It is still possible to transition the issue to Resolve without any resolution using Jira
> Cloud API directly."
> "Issue transition API is not expected to respect the screens."
> — support.atlassian.com, "Best practices on using the Resolution field in Jira Cloud"

The user's EJ project (JSM) reproduces this exactly: Done transitions via API succeed with
`resolution=null` because the workflow uses a screen-only requirement, not a validator.
This is documented, instance-independent Atlassian behavior — not an EJ quirk.

Research reference: `.factory/research/jsm-resolution-required-api-validation.md`

## Decisions

### 1. Proactive (expand-based) enforcement, not reactive-400-only

**Decision:** `handle_move` calls `GET /rest/api/3/issue/{key}/transitions?expand=transitions.fields`
instead of the plain `GET .../transitions`. When the resolved transition targets a done-category
status AND the expanded `fields` map contains a `"resolution"` key (or `isConditional` is true),
`jr` intercepts BEFORE the POST and either prompts the user (interactive) or exits 64
(non-interactive).

**Justification:** The reactive-400 path (BC-3.2.009) only fires when Atlassian enforces
resolution at the API level. For screen-only resolution requirements (the dominant JSM pattern),
the API never returns a 400 — it silently produces a `resolution=null` closed issue. No amount of
error handling after the POST can fix a silent success. Proactive enforcement using
`expand=transitions.fields` is the only reliable client-side defense for this class of workflow.

The expand is necessary but not sufficient: `transitions.fields` reflects screen configuration
only, not workflow validators. A validator-enforced requirement can exist without the field
appearing on the screen. To catch this validator-only / not-on-screen false-negative, the trigger
condition also includes `isConditional == true` — Atlassian uses `isConditional` to signal that
hidden conditions or validators may require fields not listed in the screen `fields` map. Together
the two conditions cover the dominant cases:

- Screen-only (no validator): detected via `fields` containing `"resolution"`.
- Validator-only (no screen): detected via `isConditional == true`.
- Both screen and validator: detected via `fields` containing `"resolution"` (sufficient).

The reactive BC-3.2.009 backstop is **preserved** for any residual cases that slip through
(e.g., the `statusCategory` field was absent from the expanded response, disabling the proactive
gate) and for forward-compatibility with future Atlassian behavioral changes.

### 2. Detection rule: `to.statusCategory.key == "done"` AND (`fields` has `"resolution"` OR `isConditional`)

**Decision:** The enforcement trigger condition is:

```
to.statusCategory.key == "done"
  AND
(transition.fields.contains_key("resolution") OR transition.is_conditional == Some(true))
```

`to.statusCategory.key == "done"` is the stable, instance-independent, lowercase machine
identifier for the green Done category. Atlassian hardcodes exactly four category keys
(`undefined`, `new`, `indeterminate`, `done`) — these are immutable across all cloud instances
and remain English/lowercase regardless of UI locale. Using the machine key is correct; using the
status name ("Done", "Resolved", "Closed") would be fragile.

**Required vs optional (`fields.resolution.required`):** When `fields` contains `"resolution"`,
the `required` boolean distinguishes hard-required (screen marks it required) from optional.
`jr` enforces in both cases when the target is done-category — an optional resolution on a Done
transition is still a resolution that should be explicitly chosen or explicitly declined. Users
who genuinely want to close without a resolution (e.g., "Won't Do" paths) pass `--no-resolution`
as an explicit opt-out.

**Conservative gate:** If `to.statusCategory` is absent from the expanded response (defensive
deserialization returns `None`), `is_done_category` evaluates to `false` and the proactive
enforcement is skipped. The issue falls through to the existing reactive path. This preserves
backward-compatibility for any instance returning non-standard transition shapes.

**BC cross-reference:** BC-3.2.013 (defined in `bc-3-issue-write.md`) specifies the full
behavioral contract and test coverage for this enforcement rule.

### 3. Read-command stability: `#[serde(default, skip_serializing)]` on `Transition.fields`

**Decision:** Add the `fields` field to the shared `Transition` struct with both `serde(default)`
and `serde(skip_serializing)`:

```rust
#[serde(default, skip_serializing)]
pub fields: Option<std::collections::HashMap<String, serde_json::Value>>,
```

**This is the governing type-level stability contract for `jr issue transitions`.**

Two options were evaluated:

**Option A (chosen): `#[serde(skip_serializing)]` on the shared `Transition` struct.**
`skip_serializing` means the field is populated during deserialization (from the expanded
response) but never emitted in JSON serialization. The `jr issue transitions --output json`
output is byte-for-byte identical to today. `get_transitions` and `get_transitions_with_fields`
share the single `Transition` type — when the plain `GET .../transitions` is used (no expand),
Atlassian omits the `fields` key entirely, and `serde(default)` materializes it as `None`.
No BC break to the read surface.

**Option B (rejected): Separate `TransitionWithFields` struct.**
A parallel struct eliminates ambiguity about serialization intent at the type level, but
introduces struct duplication for a single field difference and complicates shared helpers that
currently accept `&Transition`.

Option A is correct here because: (a) the output stability constraint is enforced by
`skip_serializing` — it is mechanically impossible to accidentally serialize the field; (b) the
single struct is simpler and the `skip_serializing` annotation is self-documenting; (c) the
existing BC-3.2 read tests pin the exact JSON shape and will fail CI immediately if
`skip_serializing` is removed.

**Future extension rule:** Any future field added to `Transition` that IS intended to appear in
`jr issue transitions --output json` MUST NOT carry `skip_serializing`. The annotation on
`fields` is scope-specific, not a blanket convention. Code reviewers must verify this distinction
when modifying `Transition`.

### 4. Two separate `JiraClient` methods — `get_transitions` and `get_transitions_with_fields`

**Decision:** Add `get_transitions_with_fields(key) -> Result<TransitionsResponse>` as a new
method on `JiraClient`. Do NOT change `get_transitions`.

**Justification:** `get_transitions` is called by `handle_transitions` (the `jr issue transitions`
read command). That command has no need for field metadata and should not pay the expanded
payload cost. The intent difference between the two call sites (read-only listing vs
enforcement-aware move) is captured at the method name level. Two methods, one for each intent,
preserve the principle of least surprise per call site.

`handle_move` calls `get_transitions_with_fields`. `handle_transitions` keeps calling
`get_transitions`. This substitution does NOT add a round-trip — `handle_move` already fetches
transitions once to resolve the transition ID. The URL changes from `../transitions` to
`../transitions?expand=transitions.fields`. Payload size increases modestly (field metadata per
transition, typical 2–10 KB for normal workflows) but round-trip count remains 1 GET + 1 POST.

### 5. Resolution value shape and `allowedValues` validation

**Decision:** Resolution is always sent as an object (`{"id": "..."}` or `{"name": "..."}`),
never as a bare string. When `transitions.fields.resolution.allowedValues` is present in the
expanded response, `jr` validates the user-supplied `--resolution` value against it and exits 64
with the candidate list on mismatch. When `allowedValues` is absent, `jr` falls back to the
global resolution list from `GET /rest/api/3/resolution` (as in BC-3.2.011) and accepts the
user's choice if it matches an entry there.

The `allowedValues` validation is performed on the ID or case-insensitive name — not on
localized display strings. This matches the existing `resolve_resolution_by_name` behavior
(exact > prefix > substring).

### 6. Bulk exclusion: single-key `handle_move` only

**Decision:** The proactive enforcement applies ONLY to the single-key `jr issue move` path
(`handle_move`). The bulk path (`handle_move_bulk`) is explicitly excluded.

**Justification:** Atlassian's bulk transition endpoint does not support `expand=transitions.fields`
equivalently. Enforcing resolution across N keys spanning multiple projects would require
one `get_transitions_with_fields` call per key (or at minimum per unique project), with the
complexity of collecting N resolution decisions upfront. This is significant implementation
complexity for a rare operation. The bulk path retains the reactive BC-3.2.009 error path —
if Atlassian enforces resolution at the API level (validator-backed workflow), each failing key
returns a 400 with the existing hint. Bulk users who need resolution should supply `--resolution`
explicitly.

**Documented limitation:** The CHANGELOG entry and docs must note that bulk `jr issue move`
does not get proactive enforcement. Users moving multiple issues to Done on validator-enforced
workflows should supply `--resolution` upfront to avoid per-key 400 errors.

### 7. Breaking change declaration and `--no-resolution` opt-out

**Decision:** This is a **breaking change** to `jr issue move` default behavior.

Previously: `jr issue move KEY Done` on a JSM workflow with a screen-only resolution requirement
succeeded silently with `resolution=null`.

After this change: the same command either prompts for a resolution (interactive TTY) or exits 64
with a hint (non-interactive / `--no-input`).

**Migration:** Scripts that transition to done-category status without `--resolution` MUST be
updated. Two migration paths:
1. Add `--resolution <name>` (recommended).
2. Add `--no-resolution` to explicitly opt out of enforcement for this invocation (escape hatch
   for workflows where `resolution=null` is genuinely intentional).

`--no-resolution` is mutually exclusive with `--resolution`. When `--no-resolution` is passed,
proactive enforcement is skipped and the transition is sent without a resolution field in the
body — identical to the previous default behavior. This flag is deliberately named to force
conscious acknowledgment of the opt-out.

**CHANGELOG:** Next minor version MUST include a Breaking Changes entry covering:
- The new proactive enforcement behavior.
- The `--no-resolution` escape hatch.
- The migration note for scripts that move issues to done-category status.

## Consequences

- `jr issue move KEY Done` on resolution-offering Done-category transitions now enforces a
  resolution upfront. Silent `resolution=null` closes are no longer possible in the common case.
- `jr issue transitions --output json` output is byte-for-byte unchanged. No BC break to the
  read surface.
- `get_transitions_with_fields` replaces `get_transitions` in `handle_move` — same round-trip
  count, slightly larger payload. No latency regression for interactive use.
- The proactive gate fires even when the workflow does NOT have a validator. For workflows where
  `resolution=null` is genuinely acceptable, `--no-resolution` provides the explicit escape.
- F5 adversarial review and F6 mutation hardening should specifically target the detection
  condition (the `is_done_category && (offers_resolution_field || is_conditional)` expression)
  and the `skip_serializing` contract enforcement.
- `test_e2e_jsm_resolution_enforcement` in the E2E suite inverted its bypass-demo branch (done,
  S-JSM-RESOLUTION-REQUIRED): the no-resolution done-category move with `--no-input` now hard-asserts
  exit 64 + stderr contains `"--resolution"` (BC-3.2.013 gate). The silent exit-0 / null-resolution
  acceptance path is removed.

## See Also

- BC-3.2.013 in `bc-3-issue-write.md` — full behavioral spec for the proactive enforcement gate
- BC-3.2.009 — reactive 400 backstop (preserved alongside BC-3.2.013)
- BC-3.2.011 — `--resolution` body shape (unchanged; interactive prompt produces the same shape)
- `docs/specs/issue-move-resolution.md` — CLI surface design (updated to reflect proactive path)
- `.factory/phase-f1-delta-analysis/jsm-resolution-required/delta-analysis.md` — F1 scope analysis
- `.factory/research/jsm-resolution-required-api-validation.md` — API behavior validation (Claims 1–4)
- ADR-0014 — JSM request creation path (parallel JSM context)

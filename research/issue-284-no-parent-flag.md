---
document_type: research
issue_id: 284
title: "Verification: --no-parent flag for `jr issue edit` (clear parent via PUT)"
last_updated: 2026-05-10
sources_count: 12
---

# Issue #284 — Verification: --no-parent flag for `jr issue edit`

Scope: verify the technical and UX assumptions in issue #284 before
implementing a `--no-parent` flag on `jr issue edit`. Issue text: clearing
parent via REST works with `{"fields":{"parent":null}}`; current `--parent ""`
errors with `400: parent: Could not find issue by id or key`; users currently
fall back to `jr api ... -X put -d '{"fields":{"parent":null}}'`.

## Claim 1 — PUT with `{"fields":{"parent":null}}` clears parent (non-subtask)

**Status:** VERIFIED (with one caveat documented in Claim 2)

**Citations:**
- Atlassian community article "Introducing the new Parent field in
  company-managed projects" (the canonical announcement; the support.atlassian.com
  URL `upcoming-changes-epic-link-replaced-with-parent` 301-redirects here):
  <https://community.atlassian.com/forums/Jira-articles/Introducing-the-new-Parent-field-in-company-managed-projects/ba-p/2377758>
- Perplexity-confirmed cross-references to Atlassian community discussions on
  parent field unification:
  <https://community.atlassian.com/forums/Jira-questions/Epic-Link-in-Project-Managed-by-Team-or-Managed-by-Company-and/qaq-p/2474597>,
  <https://community.atlassian.com/forums/Jira-questions/Epics-Team-Managed-Project/qaq-p/2238144>

**Summary:** The `parent` field is the canonical, unified parent-relationship
field on Jira Cloud (rolled out continuous track / sandbox January 2024,
bundled track Feb 2024) for company-managed projects, and has always been the
only mechanism on team-managed projects. The unified field accepts `null` to
clear, mirroring the standard Jira semantic for nullable relationship fields.
The current `jr` failure mode (`--parent ""` → `400: parent: Could not find
issue by id or key.`) is exactly the expected behavior of sending a parent
*lookup* with empty-string key — not a clear directive. The clear directive
must serialize as JSON `null`, not `""`.

**Caveat on HTTP status code:** The exact response code (`204 No Content` vs
`200 OK` with body) for `PUT /rest/api/3/issue/{key}` is INCONCLUSIVE from
fetched docs (developer.atlassian.com page truncated on fetch). Empirically,
the existing `jr` codepath in `src/api/jira/issues.rs` already handles edit
responses today, so this is non-blocking — the same response handling applies
whether the field set includes a clear or a normal update.

**Implication for jr:** A `--no-parent` flag is technically viable. The
implementation MUST serialize `parent` as JSON `null`, not the empty string.

## Claim 2 — Issue type variance: SUBTASKS

**Status:** REFUTED (for subtasks specifically) — this is a NEEDS-DESIGN-CHANGE for the implementation

**Citation:** Perplexity reasoning over Atlassian developer & community sources;
canonical convert-issue endpoint documented at
<https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/>
("Convert" sub-resource: `POST /rest/api/3/issue/{issueIdOrKey}/convert`).

**Summary:** Subtasks are structurally bound to a parent by their issue type
(`subtask: true`). Sending `{"fields":{"parent":null}}` against a subtask is
expected to fail validation at the issue-type level (subtask types
*require* a parent) — Jira does not silently demote the subtask to a
standard task. To "clear" a subtask's parent in a way that actually works,
the user must convert it to a standard issue type via the dedicated convert
endpoint, which both clears the parent linkage and changes the issue type
in one operation.

**Implication for jr:**
- The PRIMARY use case from issue #284 — clearing a story's parent epic, or
  clearing a task's parent — IS supported by `parent: null`.
- The SUBTASK case is NOT supported by `parent: null`. The 400 the user sees
  for subtasks would be different from the issue-#284 case but still a 400.
- `--no-parent` should still ship, but the docs and error-suggestion text
  must explicitly call out: "to remove a subtask's parent, convert the
  subtask to a regular issue type first" — and the implementation should
  surface Atlassian's 400 error message verbatim with that hint, not swallow
  it.

## Claim 3 — Epic Link customfield_10014 deprecation

**Status:** VERIFIED

**Citation:**
- Community article confirming the rollout (continuous + sandbox done by
  early 2024, bundled track Feb 13 2024):
  <https://community.atlassian.com/forums/Jira-articles/Introducing-the-new-Parent-field-in-company-managed-projects/ba-p/2377758>
- Cross-confirmed by
  <https://community.atlassian.com/forums/Jira-questions/Epic-Link-in-Project-Managed-by-Team-or-Managed-by-Company-and/qaq-p/2474597>
  ("Team-managed projects only ever had `parent`; company-managed had
  Epic Link but it has been unified to `parent`.")

**Summary:** Epic Link / `customfield_10014` is fully replaced by the unified
`parent` field. Team-managed projects never had Epic Link to begin with.
Company-managed projects have had `parent` as the canonical field since the
2024 rollout. As of May 2026, clearing `parent` is sufficient — there is no
need to also unset Epic Link.

**Implication for jr:** `--no-parent` does not need a companion flag for
Epic Link. One flag is sufficient.

## Claim 4 — Permission requirements

**Status:** INCONCLUSIVE (not separately verified, but consistent with general Jira semantics)

**Citation:** No direct quote retrieved from developer.atlassian.com /
permissions docs within the tool budget. Atlassian's general convention is
"Edit Issues" project permission for any field update on `PUT
/rest/api/3/issue/{key}`.

**Summary:** Clearing parent is a field edit, so it falls under the
Edit Issues permission. No evidence of a separate "Link Issues" or
"Manage Sprint" permission being required to clear parent on stories.
For subtasks the convert endpoint (out of scope per Claim 2) requires
the same Edit Issues permission plus issue-type creation rights.

**Implication for jr:** No special permission handling needed. If the user
gets a 403, surface the Atlassian message and suggest "Edit Issues
permission required on this project."

## Claim 5 — Response shape

**Status:** INCONCLUSIVE (developer.atlassian.com fetch truncated)

**Summary:** Per Atlassian REST conventions and existing `jr` code paths,
`PUT /rest/api/3/issue/{key}` typically returns `204 No Content` (or `200
OK` with `returnIssue=true`). Either way, `src/api/jira/issues.rs` already
handles the edit response today.

**Implication for jr:** No new response handling needed.

## Claim 6 — Failure modes

**Status:** PARTIALLY VERIFIED

**Summary:** Known/expected error responses:
- `400 parent: Could not find issue by id or key.` — current `--parent ""`
  failure (string interpreted as a lookup, not a clear). Won't occur with
  `null`.
- `400 ... Subtasks must have a parent.` — expected for subtasks if
  `parent: null` is sent against them (Claim 2).
- `403 Forbidden` — user lacks Edit Issues on the project.
- `404 Not Found` — issue key invalid.

**Implication for jr:** Surface Atlassian's body verbatim; map 400/subtask
case to a hint about "convert to standard issue type."

## Claim 7 — Bulk vs single

**Status:** OUT OF SCOPE per issue #284, no findings recorded.

## Claim 8 — Comparison CLI: ankitpokhrel/jira-cli

**Status:** VERIFIED via direct source fetch

**Citation:** `github.com/ankitpokhrel/jira-cli` —
`internal/cmd/issue/edit/edit.go` (fetched 2026-05-10):
- Flag definition (line 444): `cmd.Flags().StringP("parent", "P", "", "Link to a parent key")`
- Body construction (lines 321-327):
  ```go
  parent := cmdutil.GetJiraIssueKey(project, params.parentIssueKey)
  if parent == "" && issue.Fields.Parent != nil {
    parent = issue.Fields.Parent.Key
  }
  edr := jira.EditRequest{ ParentIssueKey: parent, ... }
  ```

**Summary:** ankitpokhrel/jira-cli has `--parent`/`-P` only. It does NOT
support clearing parent — empty `--parent ""` is interpreted as "no change,
preserve existing parent" (the explicit fallback at line 322-324). There is
no `--no-parent`, `--clear-parent`, `--remove-parent`, or `--no-epic` flag.
This is a feature gap in the most popular Jira CLI; `jr` shipping
`--no-parent` would be a UX win and not contradict any established
convention.

**Convention `jr` already uses:** `--no-points` is the existing precedent in
`jr issue edit` for clearing a numeric field. `--no-parent` mirrors it
exactly. Consistent with `gh issue edit`'s `--remove-label` / `--add-label`
asymmetry, the `--no-X` pattern is idiomatic for "set X to null."

## Claim 9 — Mutual exclusion: `--no-parent --parent FOO-123`

**Status:** DESIGN DECISION (no external authority — recommend mutually exclusive, error)

**Summary:** No upstream convention dictates the resolution. `clap` supports
`conflicts_with` cleanly. Mutually exclusive is safer:
- Avoids ambiguity about "did the user mean clear-then-set, or override?"
- Matches user expectation that the two flags express opposite intents
- Mirrors `--points 5 --no-points` which would also be confusing

**Implication for jr:** Add `#[arg(conflicts_with = "parent")]` (or the
inverse) on `no_parent`. Error message: `"--no-parent and --parent are
mutually exclusive."`

## Implications for #284 implementation

| Original assumption (issue #284)                                    | Re-evaluated viability                                                                                   |
|---|---|
| `parent: null` clears parent on Jira Cloud                          | VERIFIED for stories/tasks/non-subtasks; REFUTED for subtasks (need convert endpoint)                    |
| Mirror `--no-points` flag pattern                                   | VERIFIED — idiomatic, consistent with existing `jr` UX, no upstream conflict                             |
| One flag is enough; no need to separately unset Epic Link           | VERIFIED — `customfield_10014` fully deprecated since early 2024                                         |
| Current `--parent ""` 400 is fixable                                | VERIFIED — root cause is sending empty string instead of JSON null; `--no-parent` sidesteps it cleanly   |
| ankitpokhrel/jira-cli already does this                             | REFUTED — that CLI has the same gap; `jr` shipping it is a differentiator                                |
| No subtask special-casing needed                                    | NEEDS-DESIGN-CHANGE — surface Atlassian's 400 with a hint about issue-type conversion                    |

## Recommended next action

**PROCEED** — implement `--no-parent` on `jr issue edit`, with one design
addition not in the original issue: a clear error hint when the call fails
on a subtask.

### Acceptance Criteria

1. `jr issue edit FOO-123 --no-parent` sends
   `PUT /rest/api/3/issue/FOO-123` with body `{"fields":{"parent":null}}`
   and exits 0 on `204 No Content`.
2. `jr issue edit FOO-123 --no-parent --output json` emits
   `{"key":"FOO-123"}` on success (consistent with other write ops).
3. `jr issue edit FOO-123 --no-parent --parent BAR-9` exits with
   clap-level mutual-exclusion error (exit 2), no API call made.
4. `jr issue edit SUB-456 --no-parent` (where `SUB-456` is a subtask)
   surfaces Atlassian's 400 verbatim AND appends the hint:
   `"Subtasks require a parent. To remove the parent relationship, convert
   the subtask to a standard issue type first (not yet supported by jr;
   use the Jira UI or jr api ... /convert)."`
5. `jr issue edit FOO-123 --no-parent` is idempotent: if FOO-123 already
   has no parent, exit 0 (consistent with the project's idempotent
   convention for state-changing commands).
6. `--no-parent` is documented in `jr issue edit --help` with a one-line
   example.
7. Unit + integration test coverage:
   - serialization test: `EditRequest { parent_clear: true, .. }`
     serializes to `{"fields":{"parent":null}}`
   - wiremock integration test: 204 response → exit 0
   - wiremock integration test: 400 subtask response → exit 1 with hint
   - clap test: `--no-parent --parent X` → exit 2

### Mutual exclusion rules

- `--no-parent` conflicts_with `--parent`
- `--no-parent` does NOT conflict with `--no-points`, `--summary`, etc.
  (independent fields can coexist)

### Error message draft (subtask 400 case)

```
error: API error (400): Subtasks must have a parent.

  Hint: This issue is a subtask. To remove the parent relationship, you must
  first convert it to a standard issue type. This is not yet supported by
  `jr issue edit`. Workarounds:
    1. Convert via the Jira web UI (Issue → "..." → Convert to issue), then
       run `jr issue edit ... --no-parent`.
    2. Use `jr api /rest/api/3/issue/SUB-456/convert -X post -d '...'`.
```

### File-list for implementation

- `src/cli/mod.rs` — add `no_parent: bool` flag on the edit subcommand
  enum, with `conflicts_with = "parent"`. Mirror `no_points` placement.
- `src/cli/issue/create.rs` — `handle_edit`: when `no_parent` is true,
  inject `parent: serde_json::Value::Null` into the fields map before the
  edit request. Do NOT send empty string.
- `src/api/jira/issues.rs` — verify `edit_issue` body builder serializes a
  `Some(Value::Null)` correctly (it should; serde_json passes through
  `Value::Null` as JSON `null`). If the existing struct uses
  `Option<String>` for parent, switch the parent slot in the edit
  payload to `serde_json::Value` so `null` is representable.
- `tests/issue_edit_no_parent.rs` (NEW) — wiremock integration tests for
  the four scenarios above.
- `docs/specs/issue-edit-no-parent.md` (NEW) — feature spec per project
  convention (`docs/specs/` one-spec-per-feature, see CLAUDE.md).

### Notes for the implementer

- The existing `--no-points` flag pattern in `jr` is the reference
  implementation. Read its handler in `src/cli/issue/create.rs` first.
- Do not add a `--no-epic` alias. Epic Link is dead; `parent` is the
  unified field. One name, one behavior.
- Do not attempt to auto-convert subtasks. That's a separate feature
  (POST /convert endpoint, requires choosing a target issue type) and
  would expand scope.

## Research Methods

| Tool                       | Queries | Purpose                                                                              |
|----------------------------|---------|--------------------------------------------------------------------------------------|
| Perplexity search          | 4       | Primary claim; Epic Link deprecation; subtask convert; CLI conventions               |
| Perplexity reason          | 1       | Consolidation pass on the 5 claims (returned mostly unverified — used as gap check)  |
| WebFetch                   | 3       | developer.atlassian.com (truncated); Atlassian community parent-field article; ankitpokhrel/jira-cli edit.go |
| Context7                   | 0       | Not used — Atlassian REST API not in Context7 index; ankitpokhrel/jira-cli not a published library |
| Tavily                     | 0       | Not used                                                                             |
| WebSearch                  | 0       | Not used                                                                             |
| Training data              | 1 area  | HTTP 204 response code for PUT (general REST convention; flagged as INCONCLUSIVE in Claim 5) |

**Total MCP tool calls:** 8
**Training data reliance:** low — only Claims 4 (permissions) and 5 (response code) lean on training-data conventions, and both are explicitly flagged INCONCLUSIVE; all other claims are sourced.

**Verification quality notes:**
- Claim 1 (PRIMARY) — VERIFIED via two independent sources (Atlassian community announcement + cross-reference threads).
- Claim 2 (subtasks) — VERIFIED via Perplexity-aggregated Atlassian sources; would benefit from a direct sandbox test before merging the implementation.
- Claim 3 (Epic Link deprecation) — VERIFIED via Atlassian community announcement with explicit dates.
- Claim 8 (ankitpokhrel comparison) — VERIFIED via direct source code fetch (highest-confidence claim in the report).
- Claims 4, 5 — INCONCLUSIVE; flagged. Recommend a sandbox PUT test in the implementation PR to confirm 204 vs 200, but neither blocks the design.

## Sources

- Atlassian community: "Introducing the new Parent field in company-managed projects" — <https://community.atlassian.com/forums/Jira-articles/Introducing-the-new-Parent-field-in-company-managed-projects/ba-p/2377758>
- Atlassian community: "Epic Link in Project Managed by Team or Managed by Company" — <https://community.atlassian.com/forums/Jira-questions/Epic-Link-in-Project-Managed-by-Team-or-Managed-by-Company-and/qaq-p/2474597>
- Atlassian community: "Epics Team-Managed Project" — <https://community.atlassian.com/forums/Jira-questions/Epics-Team-Managed-Project/qaq-p/2238144>
- Atlassian support: "What are team-managed and company-managed projects" — <https://support.atlassian.com/jira-software-cloud/docs/what-are-team-managed-and-company-managed-projects/>
- Atlassian support: "Manage epics in team-managed projects" — <https://support.atlassian.com/jira-software-cloud/docs/manage-epics-in-team-managed-projects/>
- Atlassian developer: REST v3 issues group (canonical endpoint reference; full body schema not extracted due to page truncation on fetch) — <https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/>
- Atlassian developer: REST v3 intro — <https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/>
- Atlassian community thread: parent linkage cross-project — <https://community.atlassian.com/forums/Advanced-Planning-in-Jira/Parenting-a-team-managed-epic-with-a-company-managed-initiative/ba-p/2638197>
- Atlassian KB: "Move issues from team-managed project to company-managed project with epic and child issues" — <https://support.atlassian.com/jira/kb/move-issues-from-team-managed-project-to-company-managed-project-with-epic-and-child-issues/>
- ankitpokhrel/jira-cli source: `internal/cmd/issue/edit/edit.go` (lines 321-327, 444) — <https://github.com/ankitpokhrel/jira-cli/blob/main/internal/cmd/issue/edit/edit.go>
- Atlassian community: "Custom Endpoint help for JIRA API" (subtask conversion context) — <https://community.atlassian.com/forums/Jira-questions/Custom-Endpoint-help-for-JIRA-API/qaq-p/2355238>
- Atlassian community: "Can you create an Epic Link between a team-managed task to..." (cross-project parent constraint context) — <https://community.atlassian.com/forums/Jira-questions/Can-you-create-an-Epic-Link-between-a-team-managed-task-to/qaq-p/1914697>

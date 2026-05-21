---
document_type: research
issue_id: 388
title: "Sub-task ↔ standard issue-type conversion via Jira Cloud REST API"
last_updated: 2026-05-20
sources_count: 18
---

# Issue #388 — Sub-task ↔ standard issue-type conversion via Jira Cloud REST API

## Scope

GitHub issue #388 asks `jr` to support converting an issue across the
**standard ↔ sub-task** boundary (Sub-task→Task, Task→Sub-task), either
transparently through `jr issue edit --type X` or via a dedicated
`jr issue convert` subcommand. Today `edit --type` works for same-category
type changes (Task↔Story↔Bug) but fails with
`HTTP 400: "issuetype: The issue type selected is invalid."` when crossing
the boundary.

This report answers six research questions with cited Atlassian sources,
distinguishing officially documented behavior from undocumented behavior.

---

## Q1 — Is there ANY supported public Jira Cloud REST API v3 / Agile endpoint that converts an issue between sub-task and standard categories?

**Answer: NO.** As of May 2026, no public Jira Cloud platform REST API v3
endpoint and no Jira Agile (Software) REST API endpoint converts an issue
across the standard ↔ sub-task hierarchy boundary.

The relevant API groups were reviewed:

- **Issues API group** (`/rest/api/3/issue/...`): the sub-resources are
  `assignee`, `comment`, `editmeta`, `notify`, `properties`, `remotelink`,
  `transitions`, `votes`, `watchers`, `worklog`, `changelog`, `archive`,
  `restore`. There is **no `convert` sub-resource.** Editing the `issuetype`
  field via `PUT /rest/api/3/issue/{key}` is the only type-change mechanism,
  and Jira's validator rejects a sub-task→standard or standard→sub-task type
  on that endpoint with `400 "issue type selected is invalid"` — the exact
  symptom in issue #388. Source:
  <https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/>
  and developer-community confirmation
  <https://community.developer.atlassian.com/t/how-can-i-convert-a-sub-task-to-task-with-rest-api/71133>
  (user reports the identical 400; no resolution; no Atlassian staff answer).

- **Issue bulk operations API group**
  (`POST /rest/api/3/bulk/issues/move`, `/bulk/issues/fields`, etc.): see Q3.
  Bulk move does **not** cross the hierarchy boundary.

- **Issue types API group** (`/rest/api/3/issuetype...`): CRUD on issue-type
  *definitions* (create/update/delete an issue type, manage avatars/schemes).
  Nothing here mutates an *existing issue's* type or hierarchy level.

- **Agile (Software) REST API** (`/rest/agile/1.0/...`): board, sprint,
  backlog, epic operations. No issue-type-conversion endpoint.

**Documented vs undocumented:** The *absence* is officially documented by
omission — none of the four API groups lists a conversion operation. The
*existence* of the limitation is corroborated by the long-standing
unresolved feature request JRACLOUD-27893 (see Q5).

---

## Q2 — Confirm or refute: `PUT /rest/api/3/issue/{key}/convert` does not exist in Jira Cloud

**CONFIRMED — the endpoint does not exist.** Neither `PUT` nor `POST`
`/rest/api/3/issue/{issueIdOrKey}/convert` is a real Jira Cloud REST API v3
operation. The Issues API group reference (see Q1) enumerates every
sub-resource of `/rest/api/3/issue/{key}` and `convert` is not among them.

> **Direct consequence for `jr` — a bug to fix.** `src/cli/issue/create.rs`
> line 834 emits a hint that tells users to run:
> ```
> jr api /rest/api/3/issue/{key}/convert -X put -d '{"type":{"name":"Task"}}'
> ```
> This references a **non-existent endpoint** and will itself produce a
> `404`. The hint was introduced for issue #284's `--no-parent` subtask
> case. The prior research file `.factory/research/issue-284-no-parent-flag.md`
> (Claim 2) cited a "canonical convert-issue endpoint
> `POST /rest/api/3/issue/{issueIdOrKey}/convert`" — **that citation was
> wrong.** No such endpoint exists. The #284 research was a misattribution,
> in the same family as the JRACLOUD misattributions documented in CLAUDE.md
> under issue #361. This must be corrected regardless of which option is
> chosen for #388.

---

## Q3 — Confirm or refute: the Bulk Move API cannot change issue-type *category*, only same-category type

**CONFIRMED — Bulk Move (`POST /rest/api/3/bulk/issues/move`) cannot cross
the standard ↔ sub-task hierarchy boundary.**

The Bulk Move API moves issues to a single target project, issue type, and
parent. It *does* support remapping issue types — but only **within the
same hierarchy level**:

- Standard parent of type A → standard issue of type B: **SUPPORTED**.
- Sub-task of type X → sub-task of type Y (`issueTypeMapping` /
  `inferSubtaskTypeDefault`): **SUPPORTED** — the `inferSubtaskTypeDefault`
  and per-subtask `targetToSourcesMapping` parameters remap *sub-task types
  to other sub-task types*, never sub-task → standard.
- Standard issue → sub-task, or sub-task → standard: **NOT SUPPORTED**.

The underlying constraint is structural, not an API gap. Atlassian Community
Champion Trudy Claspill states the rule plainly:

> "Jira does not allow an issue to have a parent that is at the same level.
> The parent issue must be in the level above the child."
> — <https://community.atlassian.com/forums/Jira-questions/Is-it-possible-to-Migrate-sub-task-to-task-issue-type-while/qaq-p/2726506>

A standard task and a sub-task occupy different hierarchy levels; converting
between them changes the issue's level, which the bulk-move type-mapping
machinery does not do. The Atlassian KB "Bulk move work items with the Jira
Cloud REST API" lists the supported scenarios as moving standard items *with*
their sub-tasks and re-parenting sub-tasks — never level conversion. Source:
<https://support.atlassian.com/atlassian-cloud/kb/bulk-move-work-items-with-the-jira-cloud-rest-api/>
and <https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/>.

**Documented vs undocumented:** The supported scenarios are officially
documented. The *inability* to cross the boundary is documented by omission
plus the explicit hierarchy-level rule from Atlassian community guidance
(Community Champion, not Atlassian staff — but the hierarchy rule itself is
a documented product behavior).

---

## Q4 — What does the Jira Cloud web UI "Convert to issue / Convert to sub-task" wizard call?

**The web UI conversion is a multi-step server-rendered wizard backed by
internal, undocumented endpoints — not the public REST API.**

- In the UI: **More → Convert to subtask** / **Convert to issue** launches a
  3–4 step wizard (select target type / select parent / map status / confirm
  field changes). Sources:
  <https://www.tutorialspoint.com/jira/jira_convert_issue_to_subtask.htm>,
  <https://www.tutorialspoint.com/jira/jira_convert_subtask_to_issue.htm>.
- On Jira Data Center / Server, the feature is implemented as UI plugin
  modules ("View Issue Ops Bar Convert to Sub-Task Link" / "...Convert to
  Issue Link") — confirming it is a *UI action*, not a REST resource.
  Source: <https://support.atlassian.com/jira/kb/the-convert-to-subtask-option-is-not-available-on-the-more-drop-down/>.
- The 2024–2026 Jira Cloud Platform changelog references convert-to-subtask
  only as an *extension point*: the `jira:actionValidator` /
  `jira:workflowValidator` Forge/Connect modules now fire on a
  `workItemTypeChanged` action when a standard work item is converted to a
  sub-task type, with new Jira-expression context variables
  (`newIssueType`, `newIssueTypeData`). This lets *apps validate* a
  UI-driven conversion — it is **not** a REST endpoint that *performs* one.
  Source: <https://developer.atlassian.com/cloud/jira/platform/changelog/>.

**Are the internal endpoints usable by third-party tools?** **No.** They
are `/rest/internal/*` (or `/secure/ConvertIssue*.jspa`-style servlet)
endpoints with no published contract, no stability guarantee, no OAuth scope
coverage, and they can change without notice. Using them would directly
violate `jr`'s ADR-0001 (thin client over the *public* API only) and is the
class of dependency Atlassian explicitly does not support for integrations.

**Documented vs undocumented:** The wizard's *existence* is documented; the
*endpoints behind it* are entirely undocumented and unsupported.

---

## Q5 — Has Atlassian added or committed to adding such an endpoint (2024–2026)?

**No — and the request is 14 years old and still not scheduled.**

- **JRACLOUD-27893 "Enable conversion of issue to subtask via REST"** —
  Created 16 Apr 2012, last updated 11 May 2026, **Status: Gathering
  Interest, Resolution: Unresolved**, 78 votes, 40 watchers, 7 support
  references. "Gathering Interest" is Atlassian's lowest pre-roadmap state —
  it "needs more unique domain votes and comments before being reviewed."
  No Atlassian staff commitment, no timeline, no linked implementation.
  Source: <https://jira.atlassian.com/browse/JRACLOUD-27893>
  (Server/DC twin: <https://jira.atlassian.com/browse/JRASERVER-27893>).
- The Atlassian developer changelog (2024–2026) contains **no announcement**
  of a conversion endpoint. The only adjacent activity is the
  `workItemTypeChanged` validator-module work described in Q4 — an
  app-extension hook, not a conversion API.

**Conclusion:** There is no public commitment. Treating #388 as
"blocked-by-upstream" is factually accurate; the upstream blocker has been
open since 2012 with no movement.

---

## Q6 — How does `ankitpokhrel/jira-cli` handle this (their issue #805)?

**Open, unimplemented.** `ankitpokhrel/jira-cli` issue #805 ("convert
subtask to a regular task / re-parent it") was opened **20 Dec 2024**. As of
May 2026 it is **still open**, has **no assignee**, **no linked PR or
branch**, and **no maintainer comment**. The most-popular Jira CLI has not
solved this — there is no implementation to copy, and no upstream
convention to align with. Source:
<https://github.com/ankitpokhrel/jira-cli/issues/805>.

This matches the prior `jr` finding for issue #284 (research file
`issue-284-no-parent-flag.md`, Claim 8): `ankitpokhrel/jira-cli` has the
same gap. `jr` is not behind a competitor here — the whole ecosystem is
blocked by the missing Atlassian endpoint.

---

## Synthesis

| Question | Verdict | Confidence |
|---|---|---|
| Q1 — any supported conversion endpoint | NO | High |
| Q2 — `/issue/{key}/convert` exists | REFUTED (does not exist) | High |
| Q3 — Bulk Move can cross category | REFUTED (cannot) | High |
| Q4 — UI wizard uses public API | NO (internal/unsupported) | High |
| Q5 — Atlassian added/committed | NO (JRACLOUD-27893 open since 2012) | High |
| Q6 — upstream CLI implemented it | NO (issue #805 open, no PR) | High |

**Inconclusive items (flagged):**

- The exact internal endpoint path/verb behind the Cloud UI wizard could not
  be retrieved (Q4). It does not matter for the recommendation — internal
  endpoints are out of scope under ADR-0001 regardless of their exact shape.
- Whether a *clone-and-close* approximation (option B) can faithfully copy
  all custom fields, attachments, comments, worklogs, and history was not
  exhaustively tested. General Jira API knowledge says comments/worklogs/
  attachments/history are **not** copyable as a unit and the issue **key
  changes** — but the precise fidelity ceiling would need a sandbox spike.

---

## Recommendation

**Option A — Reshape: drop the conversion feature; make `edit --type` emit an
accurate, helpful error on the cross-boundary 400, and fix the fake-endpoint
hint bug.**

### Why A

1. **There is no supported path to do this correctly (Q1–Q5).** A true
   conversion *must* preserve the issue key, history, comments, worklogs,
   and attachments — that is the entire point of "convert" vs "recreate."
   No public Jira Cloud endpoint does this. Options B and D both fail the
   core requirement.

2. **Option D (internal endpoints) is a hard no.** It violates ADR-0001
   (public API only), depends on an undocumented, unversioned contract that
   Atlassian can break without notice, has no OAuth scope coverage, and
   would make `jr` fragile in exactly the way the thin-client architecture
   was designed to avoid. The CLAUDE.md citation-discipline notes (issue
   #361) show the project already treats unverified API claims as a
   correctness hazard; shipping against `/rest/internal/*` is the inverse of
   that discipline.

3. **Option B (clone-and-close) is a feature trap.** It does NOT preserve
   the key/history (explicitly noted in #388), silently produces a *new*
   issue, breaks every external reference to the old key, and cannot copy
   comments/worklogs/attachments faithfully. Presenting that as
   `jr issue convert` would mislead users — they would reasonably expect a
   real conversion. If anyone ever wants this, it should be an explicit,
   differently-named command (e.g. `jr issue recreate --as-type`) with loud
   warnings — *not* a response to #388, and not under the verb "convert."

4. **Option C (close as blocked-by-upstream) is correct on the facts but
   leaves the user worse off.** Issue #388's user hits a real, confusing
   `400 "issue type selected is invalid"` today with no guidance. Closing
   the issue without improving that error abandons the actual pain point.
   The fake-endpoint hint bug at `create.rs:834` would still need a separate
   fix anyway — so C is strictly a subset of A with worse UX.

5. **Option A delivers real value within architectural constraints.** The
   deliverable is a precise, actionable error whenever `edit --type` (or the
   `--no-parent` subtask path) hits the cross-boundary 400:
   - Detect the boundary-crossing 400 (sub-task→standard or
     standard→sub-task) — distinguishable by inspecting the source issue's
     `issuetype.subtask` flag against the requested target type.
   - Emit a hint that (a) explains *why* it failed (hierarchy-level change
     is not supported by the Jira Cloud REST API), (b) points to the web-UI
     wizard ("More → Convert to issue / Convert to sub-task"), and (c) cites
     JRACLOUD-27893 so users can add their vote.
   - **Fix the bug:** remove the fake `/rest/api/3/issue/{key}/convert`
     reference at `src/cli/issue/create.rs:834` and replace it with the
     web-UI workaround. (Same fix the prior #284 research's wrong citation
     would have required.)

### Concrete scope for the #388 PR

- Add a `is_cross_hierarchy_type_error()` detector (sibling to the existing
  `is_subtask_parent_error()` at `create.rs:1159`) keyed on the
  `"issue type selected is invalid"` 400 body **plus** a source-vs-target
  subtask-flag mismatch, to avoid false positives on genuinely invalid type
  names.
- Replace the `create.rs:834` hint text. New wording (no fake endpoint):
  ```
  This issue cannot change between standard and sub-task types via the API.
  Jira Cloud's REST API does not support cross-hierarchy issue-type
  conversion (see JRACLOUD-27893). To convert it, use the Jira web UI:
  open the issue, then "More → Convert to issue" (or "Convert to sub-task").
  ```
- Apply the same hint on the `edit --type` cross-boundary 400 path.
- Tests: wiremock integration test for each direction (sub-task→standard,
  standard→sub-task) asserting exit 1 + the hint on stderr; a regression
  test asserting the literal string `/rest/api/3/issue/` no longer appears
  in the hint (mirrors the stderr-literal pinning pattern in CLAUDE.md).
- Update `.factory/research/issue-284-no-parent-flag.md` is not required,
  but the #388 PR should note the #284 Claim-2 citation was wrong.

### Disposition of the upstream blocker

Reference JRACLOUD-27893 in the #388 resolution. The conversion feature
itself is genuinely blocked-by-upstream; option A ships the *reachable*
portion (good errors) now and leaves a clean re-entry point if Atlassian
ever ships the endpoint.

---

## Addendum (2026-05-20) — Validation of the Option A implementation design

The Option A design was reviewed against Jira Cloud reality. Verdicts:

### A1 — `"issue type selected is invalid"` is NOT a boundary-cross-specific message

**REFINE — the substring is reliable as a *match*, but it is NOT a reliable
*classifier*.** The error `{"issuetype":"The issue type selected is invalid."}`
is the SAME message Jira Cloud returns for a mistyped/wrong-ID issue type,
not just for a boundary cross. Confirmed:
<https://community.developer.atlassian.com/t/issuetype-the-issue-type-selected-is-invalid/65007>
and <https://community.developer.atlassian.com/t/unable-to-change-issue-type-via-rest-api/64003>
— both threads show this exact message produced by referencing an issue-type
ID not present on the project (a plain mistake, not a hierarchy cross).

Implication: a detector that returns `true` on the bare string ALONE will
over-claim "this is a conversion" on every typo. The design MUST gate on a
**source-vs-target subtask-flag mismatch** (compare the source issue's
`issuetype.subtask` boolean against the requested target type's `subtask`
boolean) before emitting the conversion-specific hint. The dual-interpretation
tail in the proposed hint partially mitigates this but is not sufficient on
its own — see the verdict table in the reply.

### A2 — i18n / localization risk

**FLAG.** Jira Cloud REST validation messages are subject to the requesting
user's locale. `"The issue type selected is invalid."` is the en-US string;
a non-English Jira site can return a translated phrase. Pure substring
matching on English text is locale-fragile — the same class of fragility
CLAUDE.md already calls out for the auto-refresh 401 trigger (which
deliberately uses a blanket-status trigger, not substring match). The
detector should therefore treat the subtask-flag mismatch as the PRIMARY
signal and the English substring as a secondary/best-effort confirmation,
degrading gracefully (still surface Atlassian's raw body) when neither the
locale string nor the flag comparison is conclusive.

### A3 — Current Jira Cloud UI wording

**REFINE — the proposed hint wording `"More → Convert to issue"` is stale.**
Current Jira Cloud (2025–2026 issue view) uses the **ellipsis `...`** action
menu in the top-right of the issue, not a labelled "More" button. Atlassian
is also mid-migration from "issue" to "work item" terminology, so instances
may show either `Convert to issue` / `Convert to sub-task` OR
`Convert to work item`. Sources:
<https://www.tutorialspoint.com/jira/jira_convert_subtask_to_issue.htm>,
<https://community.atlassian.com/forums/Jira-questions/Convert-sub-task-to-task-type/qaq-p/977033>.
Recommended locale-and-version-resilient wording avoids quoting an exact
label — see the reply.

### A4 — Workflow/scheme-incompatibility also produces 400s on type change

**FLAG.** A type change can ALSO be rejected when the target type's workflow
does not contain the issue's current status (the `editmeta` `allowedValues`
constraint). That failure is a different, legitimate non-conversion 400.
Whether it surfaces the *identical* `"issue type selected is invalid"`
string or a different message is INCONCLUSIVE from public sources. This
reinforces A1: the subtask-flag mismatch must be the gating signal so the
conversion hint never fires on a workflow-incompatibility 400.

### A5 — JRACLOUD-27893 citation scope

**CONFIRMED appropriate for both cases.** JRACLOUD-27893 ("Enable conversion
of issue to subtask via REST") covers conversion in both directions and is
the canonical public ticket for the missing capability. It is the correct
citation for the `--type` cross-boundary hint AND the `--no-parent` subtask
hint (un-parenting a subtask requires the same missing conversion). No
better-scoped public ticket exists.

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity perplexity_ask | 0 | Not used (Perplexity MCP unavailable in session; WebSearch/WebFetch substituted) |
| Perplexity perplexity_search | 0 | Not used |
| Perplexity perplexity_research | 0 | Not used |
| Perplexity perplexity_reason | 0 | Not used |
| Context7 | 0 | Not applicable — Atlassian REST API not in Context7 index |
| Tavily | 0 | Not used |
| WebFetch | 7 | Developer-community sub-task-convert thread; JRACLOUD-27893 tracker; ankitpokhrel #805; Bulk Operation FAQ; bulk-move issuetype thread; sub-task-migration community thread; bulk-move KB; convert-to-subtask UI KB; Issues API group reference |
| WebSearch | 6 | Convert endpoint existence; 400 symptom corroboration; upstream #805; bulk-move category limits; internal endpoints; Atlassian changelog |
| Training data | 2 areas | (1) General REST conventions for what a faithful "convert" must preserve — flagged in Q-synthesis "inconclusive". (2) That `/rest/internal/*` endpoints are unversioned/unsupported — well-established and consistent with Atlassian's published integration guidance. |

**Total external tool calls:** 13 (7 WebFetch + 6 WebSearch)
**Training data reliance:** low — every factual claim about endpoint
existence, tracker status, and upstream issue status is sourced to a fetched
Atlassian or GitHub page; training data is used only for two clearly-flagged
general-knowledge points.

## Sources

- Jira Cloud REST API v3 — Issues API group — <https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/>
- Jira Cloud REST API v3 — Issue bulk operations API group — <https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/>
- Jira Cloud Platform changelog — <https://developer.atlassian.com/cloud/jira/platform/changelog/>
- Bulk Operation APIs: Additional Examples and FAQs — <https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/>
- Atlassian KB: Bulk move work items with the Jira Cloud REST API — <https://support.atlassian.com/atlassian-cloud/kb/bulk-move-work-items-with-the-jira-cloud-rest-api/>
- Atlassian KB: The convert to-subtask option is not available on the More drop-down — <https://support.atlassian.com/jira/kb/the-convert-to-subtask-option-is-not-available-on-the-more-drop-down/>
- Developer Community: How can I convert a sub task to task with REST API? — <https://community.developer.atlassian.com/t/how-can-i-convert-a-sub-task-to-task-with-rest-api/71133>
- Atlassian Community: How to change issuetype with the Bulk move API — <https://community.atlassian.com/forums/Jira-questions/How-to-change-issuetype-with-the-Bulk-move-API/qaq-p/2984553>
- Atlassian Community: Is it possible to migrate sub-task to task issue type while maintaining parent link — <https://community.atlassian.com/forums/Jira-questions/Is-it-possible-to-Migrate-sub-task-to-task-issue-type-while/qaq-p/2726506>
- JRACLOUD-27893: Enable conversion of issue to subtask via REST — <https://jira.atlassian.com/browse/JRACLOUD-27893>
- JRASERVER-27893 (Server/DC twin) — <https://jira.atlassian.com/browse/JRASERVER-27893>
- ankitpokhrel/jira-cli issue #805 — <https://github.com/ankitpokhrel/jira-cli/issues/805>
- ankitpokhrel/jira-cli repository — <https://github.com/ankitpokhrel/jira-cli>
- Tutorialspoint: JIRA - Convert Issue To Subtask — <https://www.tutorialspoint.com/jira/jira_convert_issue_to_subtask.htm>
- Tutorialspoint: JIRA - Convert Subtask to Issue — <https://www.tutorialspoint.com/jira/jira_convert_subtask_to_issue.htm>
- Prior `jr` research: issue #284 --no-parent flag (contains the misattributed convert-endpoint citation corrected by this report) — `.factory/research/issue-284-no-parent-flag.md`
- Atlassian Community: Bulk move subtasks' issue type while keeping their parent tickets — <https://community.atlassian.com/forums/Jira-questions/Bulk-move-subtasks-issue-type-while-keeping-their-parent-tickets/qaq-p/3151391>
- Developer Community: Convert to Sub-Task with REST API (Server context) — <https://community.developer.atlassian.com/t/convert-to-sub-task-with-reset-api/56858>
</content>
</invoke>

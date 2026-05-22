# Research: Field-change echo conventions for `jr issue edit` (issue #398)

**Date:** 2026-05-21
**Type:** general (technology / CLI-convention research)
**Topic:** How agentic-style CLIs confirm field changes after edit/update, and how they echo large / multi-line (description / body) fields.

---

## Summary of question

`jr issue edit` currently prints only `Updated FOO-123`. We are adding a "changed fields" echo
(table mode → stderr; JSON mode → `changed_fields` map in the payload). Scalar fields are
trivial. The open question is the **description** field: long, multi-line, ADF-encoded —
echoing verbatim floods the confirmation line. This report surveys prevailing CLI behavior
and ends with a concrete recommendation.

---

## 1. How `gh` confirms changes after `gh issue edit` / `gh pr edit`

**Verified from source — `gh` does NOT echo changed fields at all.** It prints only the
resource URL to stdout.

- `gh issue edit` — on success, prints the **issue URL(s)** to stdout, one per line, sorted;
  failures go to stderr as `failed to update {URL}: {error}`. No field echo.
  Source: [`pkg/cmd/issue/edit/edit.go`](https://github.com/cli/cli/blob/trunk/pkg/cmd/issue/edit/edit.go) — `fmt.Fprintln(opts.IO.Out, editedIssueURL)`.
- `gh pr edit` — on success, prints only `pr.URL` to stdout. No field echo, no summary.
  Source: [`pkg/cmd/pr/edit/edit.go`](https://github.com/cli/cli/blob/trunk/pkg/cmd/pr/edit/edit.go) — `fmt.Fprintln(opts.IO.Out, pr.URL)`.

**Observed behavior:** `$ gh issue edit 23 --body "..."` → prints `https://github.com/owner/repo/issues/23`
and nothing else. `gh` deliberately keeps state-change output to the single most useful
machine-consumable token (the URL) so it pipes cleanly into the next command. It does NOT
echo the new body — not even truncated, not even a marker.

This is the dominant pattern in the tool `jr` is most explicitly modeled on.

## 2. How other tools represent a changed long-text / body field

| Tool | Edit command | Success output | Long-text (body/description) handling |
|------|--------------|----------------|----------------------------------------|
| **`gh`** | `gh issue edit`, `gh pr edit` | Resource URL only | **Not echoed at all** — no marker, no preview. ([source](https://github.com/cli/cli/blob/trunk/pkg/cmd/issue/edit/edit.go)) |
| **`glab`** | `glab issue update` | `✓ Updated issue ...` + issue URL | Not echoed; confirmation is a fixed success line + URL. ([docs](https://docs.gitlab.com/cli/issue/update/)) |
| **`jira-cli` (ankitpokhrel)** | `jira issue edit` | `✓ Issue updated\n<browse-URL>` | **Not echoed.** Source: `cmdutil.Success("Issue updated\n%s", GenerateServerBrowseURL(...))` in [`internal/cmd/issue/edit/edit.go`](https://github.com/ankitpokhrel/jira-cli/blob/main/internal/cmd/issue/edit/edit.go). If nothing changed → `✗ Nothing to update`. |
| **`stripe` CLI** | `stripe <resource> update` | Full updated object as **JSON** | The *entire* updated object, including long text, is dumped as JSON to stdout. There is no human "confirmation line" mode — the object IS the confirmation. Pipe-to-`jq` is the intended workflow. ([docs](https://docs.stripe.com/stripe-cli/use-cli)) |
| **`kubectl`** | `kubectl apply` / `kubectl edit` | `<kind>/<name> configured` (or `unchanged`) | **Not echoed.** kubectl reports only the resource identity + an action verb (`created`/`configured`/`unchanged`). The body/spec is never reflected back. To see values you re-`get -o yaml`. ([docs](https://kubernetes.io/docs/tasks/manage-kubernetes-objects/declarative-config/)) |
| **`aws` CLI** | `aws <svc> update-*` | Full updated object as JSON (default) | The whole response object is printed as JSON; long-text attributes appear verbatim. There is no separate "human confirmation" — same machine-output-first model as Stripe. |

### What this tells us

There are exactly **two camps**, and almost nobody echoes a *truncated preview* or a
*per-field human summary* of a long-text change:

- **Camp A — "identity + verb" (gh, glab, jira-cli, kubectl):** Confirmation is a fixed
  success token: the resource URL/ID, optionally with an action verb. Changed fields are
  **never** itemized. Long text is **never** echoed in any form. The user/agent re-fetches
  if they want to see values.
- **Camp B — "echo the whole object as JSON" (stripe, aws):** No human confirmation line;
  the success output IS the full updated object as structured JSON. Long text appears
  verbatim because JSON string-encoding makes multi-line content safe to emit.

**Nobody surveyed truncates a body to a preview snippet in a confirmation line.** The
closest thing to a "marker" is kubectl's `configured` verb, which says *that* a change
happened without saying *what* the new value is. No surveyed tool emits a diff in default
confirmation output. No surveyed tool emits a character/line count for a changed field.

This is a meaningful negative result: the truncated-preview idea, while intuitive, has no
precedent among mature agentic CLIs — because a preview is neither reliably machine-parseable
nor reliably faithful (it can mislead an agent into thinking it saw the full value).

## 3. Prevailing convention and pipe-friendliness best practice

From the Command Line Interface Guidelines ([clig.dev](https://clig.dev/)) and corroborating
sources:

- **"If you change state, tell the user."** A state-changing command should explain what
  happened so the user can model system state. This argues *for* a changed-fields echo —
  but the canonical examples (e.g. `git push`) report *identity and counts*, not field
  *contents*.
- **stdout for data, stderr for messaging.** "Log messages, errors, and so on should all
  be sent to `stderr` so that when commands are piped together, these messages are
  displayed to the user and not fed into the next command." → A human confirmation echo
  belongs on **stderr** (which matches `jr`'s existing "Mixed" / profile-3 channel model).
- **Large output → pager, not inline.** clig.dev recommends paging large text rather than
  flooding a line. A confirmation line is the *wrong place* for a multi-line body by
  construction.
- **Machine output should be deterministic and stable.** Field names in JSON shouldn't
  shift between releases except on SemVer-major; additive changes are safe. A
  `changed_fields` map is a clean additive change.
- **A truncated/elided value is a hazard for scripts and agents.** Any preview is
  lossy; if an agent reads it as the authoritative value it will be wrong. Best practice
  is: machine output is either *complete and faithful* or *absent* — never *partial and
  ambiguous*.

**Net convention:** confirmation output should report **what changed (the field names)**
and the resource identity, and should keep **scalar new-values** where they are short and
safe. Long-text fields should be reported as *changed* (a marker), not *shown* (a value),
in any line-oriented/human/table context.

## 4. Long-text treatment: JSON payload vs human/table output

These two channels have different constraints and should be treated differently:

**Human / table (stderr):**
- A multi-line ADF/description value will break any table-ish or line-oriented rendering.
- A truncated preview is lossy and (per §2) has no precedent.
- → Emit a **`(updated)` marker** for the description: the user learns *that* it changed
  without the line being flooded. This matches kubectl's `configured` philosophy and
  `jr`'s own existing "hints to stderr" convention (board-view truncation hint,
  `sprint current`, `issue list`).

**JSON (`--output json`, `changed_fields` map):**
- JSON string-encoding makes multi-line / arbitrary content **safe** — newlines become
  `\n`, no delimiter ambiguity, `jq` consumes it losslessly. This is exactly why
  Stripe/AWS can dump whole objects.
- A machine-output-first CLI should be **complete and faithful** in JSON. An agent that
  parses `changed_fields` should get the *real* value, not a preview.
- **However:** `jr`'s description is **ADF** (Atlassian Document Format — a nested JSON
  document), not plain text. Echoing raw ADF into `changed_fields.description` would be
  technically faithful but practically hostile: agents expect a string there, and raw ADF
  is verbose and instance-coupled. `jr` already owns an `ADF→text` converter (`src/adf.rs`).
- → In JSON, set `changed_fields.description` to the **plain-text rendering of the new
  description** (via the existing `adf.rs` ADF→text path), not raw ADF and not a marker.
  This is complete, faithful, machine-parseable, and pipe-safe. If full-fidelity ADF is
  ever needed, that is a separate `--output json` object-echo decision (see Stripe model),
  not part of the `changed_fields` confirmation contract.

---

## RECOMMENDATION

For `jr issue edit`, echo a `--description` change as follows:

### (a) Table / human output (stderr)

Use a **`(updated)` marker** — do NOT print the value, do NOT print a truncated preview.

```
Updated FOO-123
  summary:     New summary text
  priority:    High
  description: (updated)
```

- Scalar fields show their new value inline (short, safe, useful).
- `description` shows the literal token `(updated)` — no content.
- All of this goes to **stderr** (profile-3 "Mixed" — keeps it out of piped stdout and out
  of `--output json`).

Rationale: matches `gh` / `glab` / `jira-cli` / `kubectl` (none echo body content; kubectl's
`configured` verb is the direct analogue of `(updated)`). A preview has zero precedent among
agentic CLIs and is lossy/misleading. The marker satisfies clig.dev's "tell the user what
changed" without flooding the line or breaking table-ish rendering.

### (b) JSON output (`changed_fields` map)

Set `changed_fields.description` to the **plain-text rendering of the new description**
(reuse `src/adf.rs` ADF→text), NOT a `(updated)` marker and NOT raw ADF.

```json
{
  "key": "FOO-123",
  "changed_fields": {
    "summary": "New summary text",
    "priority": "High",
    "description": "First line of the new body\nSecond line\n..."
  }
}
```

Rationale: JSON string-encoding makes multi-line content safe and pipe-friendly (the whole
reason Stripe/AWS can echo full objects). A machine-output-first CLI must be **complete and
faithful** in its machine channel — an agent parsing `changed_fields.description` must get
the real new value, not an ambiguous `(updated)` sentinel it cannot act on. Plain-text (not
raw ADF) keeps the value agent-consumable and instance-decoupled, and `jr` already owns the
converter.

### Why this split is correct for `jr`'s "agentic, scriptable, machine-output-first" vision

- **Pipe-friendly:** the long value never touches stdout in table mode (it is a marker on
  stderr); in JSON mode it is string-encoded so it cannot break parsing.
- **Machine-output-first:** the JSON channel is complete and faithful — no lossy preview,
  no sentinel the agent can't use.
- **Human-respectful:** the table channel stays compact and scannable; kubectl's decade of
  `configured` proves a verb/marker is enough for humans.
- **Consistent with `jr`'s own conventions:** hints/markers to stderr (profile-3), JSON
  payload is the authoritative machine surface, additive `changed_fields` key is a
  SemVer-minor-safe schema change.

### Asymmetry note (intentional, document it)

The description field is deliberately represented **differently** in the two channels —
`(updated)` marker in human/table, full plain-text value in JSON. This asymmetry is
correct (human channel optimizes for scannability, machine channel for fidelity) but is a
gotcha for anyone reading the code later. Recommend a one-line code comment plus a CLAUDE.md
"Gotchas" entry when implementing, so a future maintainer does not "fix" the asymmetry by
making them match.

---

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| WebSearch | 6 | `gh`/`glab`/`jira-cli`/`stripe`/`kubectl` edit-output behavior; CLI confirmation best practice |
| WebFetch | 5 | Verified `gh issue edit` + `gh pr edit` source; `ankitpokhrel/jira-cli` edit source; glab issue update docs; clig.dev guidelines |
| Perplexity | 0 | MCP not invoked this run — primary claims verified directly against source repos |
| Context7 | 0 | Not applicable (no library API in scope) |
| Tavily | 0 | Not invoked |
| Training data | 1 area | `aws` CLI default-JSON object echo — flagged as model knowledge, consistent with Stripe pattern but NOT source-verified this run |

**Total external tool calls:** 11
**Training data reliance:** low — the load-bearing claims (`gh`, `jira-cli`, glab, kubectl,
clig.dev) are verified against primary sources (GitHub source files / official docs). Only
the `aws` CLI characterization rests on training data and is non-load-bearing for the
recommendation.

### Confidence

- **High:** `gh` does not echo changed fields (source-verified, both `issue` and `pr`).
- **High:** `jira-cli` (ankitpokhrel) does not echo body (source-verified).
- **High:** No surveyed agentic CLI emits a truncated body preview in confirmation output.
- **Medium:** `aws` CLI full-object-JSON behavior (training data; consistent with Stripe).
- **High:** clig.dev guidance on stdout/stderr split and JSON stability (source-verified).

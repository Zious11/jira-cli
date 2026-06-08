# `markdown_to_adf` — `listItem` content-model conformance

**Issue:** [#470](https://github.com/Zious11/jira-cli/issues/470)
**Module:** `src/adf.rs`
**Behavioral contract:** BC-7.2.006 (`.factory/specs/prd/bc-7-output-render.md`)

## Problem

`markdown_to_adf` emits ADF `listItem` nodes whose `content` may contain
`blockquote`, `heading`, `table`, and `rule` child nodes. Per the official
Atlassian Document Format spec, the `listItem` content model permits **only**:

- `paragraph` (no marks)
- `bulletList`
- `orderedList`
- `codeBlock` (no marks)
- `mediaSingle`

`blockquote`, `heading`, `table`, and `rule` are valid **top-level** block
nodes but are **not** valid children of `listItem`. The current code's existing
comment acknowledges this ("Jira's renderer handles the latter shape
leniently") — but renderer leniency is not contractual. A renderer tightening,
an editor round-trip, or reuse in another Atlassian surface (e.g. Confluence)
can drop, reflow, or reject the content.

pulldown-cmark legitimately emits all four node kinds inside `Item` for inputs
such as `- > quoted`, `- # heading`, `- item\n\n  ---`, and a loose list item
containing an indented GFM table. All four shapes were confirmed reachable.

## Atlassian constraint (validated)

Confirmed via the Atlassian `listItem` node reference page
(`developer.atlassian.com/cloud/jira/platform/apis/document/nodes/listItem/`,
verified 2026-06-08, Perplexity-cited): `listItem.content` is restricted to the
five node types above. Note the asymmetry that caused the original bug —
`tableCell`/`tableHeader` *do* permit rich block content (`blockquote`,
`heading`, `table`, etc.); `listItem` does not. The code generalized the cell
content model to list items.

## Design

When assembling a `listItem`, run a normalization pass over its children that
transforms every disallowed block into the permitted set. The pass runs
**before** `wrap_inlines_as_blocks`, because simply removing the disallowed
types from that function's allowlist would cause loose inline runs adjacent to
them to be mis-grouped and the disallowed blocks themselves to be wrapped into a
`paragraph` (producing `paragraph > blockquote`, the *other* invalid shape).

### Normalization rules

| Disallowed child | Normalization | Rationale |
|---|---|---|
| `blockquote` | **Unwrap** — splice the blockquote's own child blocks directly into the listItem, **recursively** normalized | Lossless; preserves paragraph structure and inline marks. `blockquote`'s children are themselves block-level (paragraphs etc.), all listItem-legal after recursion. |
| `heading` | **Convert to `paragraph`**, preserving the heading's inline `content` (text nodes + marks) verbatim; drop the `level` attr | In-spec; text and inline emphasis preserved. No added `strong` mark (predictable, lossless). |
| `table` | **Flatten** — emit **one `paragraph` per `tableRow`**, joining cells in `\| a \| b \|` form by splicing each cell's inline nodes in directly (`flatten_table_to_paragraphs`) | Cell text and inline **marks are preserved** (real ADF `strong`/`em`/`link` nodes — NOT routed through `adf_to_text`, which would emit literal `**`/`[]()` markdown that Jira shows verbatim). The table **grid structure is lost** — no ADF node nests a table inside a listItem. Extreme edge case (requires loose list + indented table). |
| `rule` | **Drop** | A `rule` is an empty leaf with no content and no meaning inside a list item. |

Permitted children (`paragraph`, `bulletList`, `orderedList`, `codeBlock`,
`mediaSingle`) and any loose inline nodes pass through untouched; the existing
`wrap_inlines_as_blocks` then groups remaining inline runs into paragraphs using
the **five-type allowlist**. Nested `bulletList`/`orderedList` items are
normalized independently at their own `listItem` boundary, so the invariant holds
recursively.

`flatten_table_to_paragraphs` splices each cell's **inline** children into the
row paragraph. For markdown-reachable tables every cell block is a `paragraph`;
defensively, a non-`paragraph` cell block (which the ADF `tableCell` schema
permits but pulldown-cmark never emits in GFM cells) is rendered to a
newline-free plain-text node rather than spliced as a block, keeping the output
valid. An all-empty source row collapses to bare `| | |` separators — valid ADF,
emitted as-is.

An empty resulting listItem still yields a single empty `paragraph`, preserving
ADF's "at least one block" requirement (existing `wrap_inlines_as_blocks`
behavior).

### Recursion

Normalization is recursive only through `blockquote` unwrapping (e.g.
`- > # heading` → unwrap blockquote → heading → paragraph). Nested
`bulletList`/`orderedList` children are already produced by recursive
`listItem` assembly and need no re-processing.

## Behavior changes

- `- > quoted text` now yields `listItem > paragraph` (was `listItem >
  blockquote > paragraph`). The existing test
  `test_markdown_blockquote_inside_list_item_is_nested_not_paragraph_wrapped`
  asserted the old out-of-spec shape and is updated to assert the in-spec shape
  (requirements changed to spec compliance, per CLAUDE.md test-modification
  policy).
- `- # heading` now yields `listItem > paragraph` (was `listItem > heading`).
- `- item\n\n  ---` now yields `listItem > paragraph` only (rule dropped).
- A table inside a list item now yields `listItem > paragraph(+)` (flattened
  rows) instead of `listItem > table`.

No change to top-level (non-listItem) `blockquote`, `heading`, `table`, `rule`
handling. No change to `tableCell`/`tableHeader` content (which legitimately
allows rich blocks).

## Out of scope

- Task lists, footnotes, bare-URL autolinks, and other missing markdown
  constructs (tracked separately: #471, #472, #473, #474).
- E2E coverage of the read/render path (#475).

## Test plan

Unit tests in `src/adf.rs` (`cargo test --lib adf`):

- [x] `- > quoted text` → `listItem > paragraph[text "quoted text"]`, no `blockquote` node anywhere
- [x] `- # Heading text` → `listItem > paragraph[text "Heading text"]`, no `heading` node, no `level` attr
- [x] `- ### deep **bold** head` → `listItem > paragraph` preserving the `strong` mark on "bold"
- [x] `- > # quoted heading` (recursive) → `listItem > paragraph`, no `blockquote`/`heading`
- [x] `- item\n\n  ---` → `listItem` with the paragraph kept and **no** `rule` node
- [x] rule-only item (`-   \n\n    ---`) → `listItem` with a single empty `paragraph`, no `rule` node
- [x] table inside a list item → `listItem` with one `paragraph` per row, no `table` node, cell text preserved, and a marked cell keeps its ADF mark (e.g. `strong`)
- [x] `codeBlock` and nested `orderedList` inside a list item pass through unmodified (permitted children)
- [x] regression: a plain `- a\n- b` bullet list and nested `- a\n  - b` still produce correct `paragraph`/`bulletList` shapes
- [x] regression: top-level `> quote`, `# heading`, `---`, and a standalone table are unchanged

Negative assertions use a structural `contains_node_type()` walk over the ADF
tree (not serialized-string `contains`), so a text node whose literal text
contains a type name cannot false-positive.

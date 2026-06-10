# ADF Task List Mapping Spec (issue #471)

## Summary

GFM task lists (`- [ ] unchecked` / `- [x] checked`) map to ADF `taskList` /
`taskItem` nodes. This document describes the implementation strategy,
normalization obligations, and known edge cases.

## ADF Schema (from @atlaskit/adf-schema full.json v40.9.2)

```json
{
  "taskList": {
    "attrs": { "localId": { "type": "string", "minLength": 1 } },
    "content": ["taskItem", "taskList"],
    "minItems": 1
  },
```

**taskList TUPLE LEAD rule:** `taskList.content` is a TUPLE where the FIRST
element MUST be a `taskItem`. Subsequent elements may be `taskItem` or nested
`taskList`. This is the canonical `@atlaskit/adf-schema` rule (verified in
`.factory/research/issue-471-adf-tasknode-shape.md`). The validator enforces
this strictly at `i == 0` — it is correct and must not be relaxed.

```json
  "taskItem": {
    "attrs": {
      "localId": { "type": "string", "minLength": 1 },
      "state": { "enum": ["TODO", "DONE"] }
    },
    "content": ["inline_node"],
    "minItems": 1
  }
}
```

Key constraints:
- `taskList.attrs.localId`: required, non-empty string
- `taskItem.attrs.state`: uppercase `"TODO"` or `"DONE"` only
- `taskItem.content`: **inline-only** — no paragraph wrapper
- `taskList.content`: TUPLE — **first element MUST be `taskItem`**; subsequent
  elements may be `taskItem` or nested `taskList`. An empty `taskList` is
  invalid ADF (`minItems: 1`). A `taskList` leading with a `taskList` violates
  the tuple-lead rule. (Validator enforces `i == 0` → must be `taskItem`.)
- `additionalProperties: false` — extra keys cause Jira HTTP 400

## Implementation Strategy: Approach B (post-hoc reclassification)

pulldown-cmark 0.13.3 emits `Event::TaskListMarker(bool)` after
`Start(Tag::Item)`. The builder uses retroactive stack mutation:

1. `Start(List(None))` → push `NodeKind::BulletList`
2. `Start(Item)` → push `NodeKind::ListItem`
3. In a **tight list**: `TaskListMarker(checked)` fires directly, mutating
   stack top from `ListItem` to `TaskItem { checked }`
4. In a **loose list**: `Start(Paragraph)` fires first, then
   `TaskListMarker(checked)` fires inside the Paragraph context, mutating
   `Paragraph` → `TaskItem { checked }`
5. At `End(Tag::List)` (`NodeKind::BulletList` arm): inspect children for
   `taskItem` nodes. If found, reclassify the BulletList to a `taskList`.

## EC-16: Loose Multi-Paragraph Task Items

For loose lists (`- [ ] line1\n\n  line2`), pulldown-cmark wraps each
paragraph in `Tag::Paragraph`. The retroactive mutation converts the FIRST
paragraph to a `TaskItem`. At `End(Item)` (ListItem finalization), if the
first direct child is a `taskItem` JSON node, the ListItem arm merges all
content:

- Extract inline content from the `taskItem` child
- Extract inline content from each subsequent `paragraph` child
- Inject `hardBreak` separator between non-empty parts
- Apply trim: remove leading/trailing `hardBreak` nodes
- Return as a merged `taskItem` (not a `listItem`)

## EC-13: Nested Task Lists (sibling semantics)

ADF `taskList.content` permits nested `taskList` as a SIBLING of `taskItem`
nodes (not nested inside `taskItem.content`). pulldown-cmark emits nested
sublists inside the parent item, so:

1. `TaskItem` finalization (`NodeKind::TaskItem` arm in `end()`): block children
   (`taskList`, `bulletList`, `orderedList`, and any other non-inline type) are
   separated from inline children (`text`/`hardBreak`). Inline children become
   `taskItem.content` directly. Block children are returned via
   `EndResult::WithHoists { node: taskItem, hoists: block_siblings }` — a typed
   channel that carries both the task item node and its hoisted siblings without
   any JSON side-channel field.
2. `BulletList` reclassification: sees `WithHoists`-originated block siblings
   as plain children of the BulletList (appended by the `WithHoists` dispatch at
   `end()`). Nested `taskList` children become part of `task_children` (siblings
   in the final `taskList.content`). Nested `bulletList`/`orderedList` go into
   the `hoisted` set for EC-15.

## EC-15: Plain BulletList Nested Inside Task Item (hoist to grandparent)

A plain bullet list nested inside a task item is invalid in `taskList.content`
and in `taskItem.content`. The hoist path:

1. `TaskItem` finalization returns `EndResult::WithHoists { node: taskItem,
   hoists: [bulletList, …] }` (typed channel — no JSON `_pending_hoists` field).
2. The `end()` dispatch appends `taskItem` first, then each hoist, so all hoists
   become siblings in the BulletList's children.
3. `BulletList` reclassification puts plain `bulletList`/`orderedList` in the
   `hoisted` set and returns `EndResult::WithHoists { node: taskList, hoists }`.
4. The outer `end()` append appends the `taskList` first, then the hoisted nodes
   — yielding correct parent-level ordering: `[taskList{outer}, bulletList{inner}]`.
   No JSON `_post_hoists` field is used; the typed `EndResult::WithHoists` carries
   both the primary node and its siblings cleanly.

## EC-5: `listItem > taskList` normalization

ADF `listItem.content` does not permit `taskList`. The
`normalize_list_item_content` function unwraps `taskList` children:
each `taskItem`'s inline content is wrapped in `paragraph`, wrapped in
`listItem`, collected into a new `bulletList`.

## EC-6: `blockquote > taskList` normalization

ADF `blockquote.content` forbids `taskList`. pulldown-cmark 0.13.3 DOES emit
`blockquote > taskList` for `> - [ ] item` (container-agnostic task-marker
scan). `normalize_blockquote_content` unwraps `taskList` → each `taskItem`'s
inline content becomes a `paragraph` inside the blockquote. Lossy (checkbox
state dropped).

## EC-8: Empty and Structurally-Empty Task Items

`is_empty_block_container` treats `taskItem` as prunable when:
- `content` array is empty, OR
- All content nodes are `hardBreak` OR whitespace-only text OR
  backslash-only text (the failed-escape artifact from `\\\n` in a tight list)

The backslash-only prune is a deliberate product choice matching the
hardBreak-only prune: a lone `\` from a failed GFM backslash-escape carries
no semantic task content.

## EC-9: Empty `taskList` Pruning

If all `taskItem` children of a `taskList` are pruned, the empty `taskList`
is subsequently pruned by the `"taskList"` entry in `REQUIRES_CONTENT`.

## LocalId Assignment

`assign_local_ids` runs as a post-finish() pass on the built ADF tree.
DFS pre-order, 1-based counter, assigns to both `taskList.attrs.localId` and
`taskItem.attrs.localId`. Counter is document-wide and unique. No uuid crate —
counter-based strings (`"1"`, `"2"`, …).

## Rendering (adf_to_text)

`ListFrame::Task` variant added. `taskList` → push `ListFrame::Task`, recurse.
`taskItem` → emit `- [x] ` (DONE) or `- [ ] ` (TODO) prefix, render inline
content, append `\n`. Nested task lists render with 2-space indentation per
level.

## Known Lossy Transforms

- EC-3 mixed-list promotion: plain items promoted to TODO, identity lost
- EC-5 listItem normalization: checkbox state dropped (sub-ref EC-10(b))
- EC-6 blockquote unwrapping: checkbox state dropped (sub-ref EC-10(c))
- EC-10: checkbox state is dropped in several normalization paths:
  - EC-10(b) — `listItem > taskList` unwrap (EC-5): TODO/DONE state discarded
  - EC-10(c) — `blockquote > taskList` unwrap (EC-6): TODO/DONE state discarded
  - EC-10(f) — casing of externally-provided state normalized on text render
    (adf_to_text uses case-insensitive match; uppercase emitted by markdown_to_adf)
- EC-13 nested hoist: inner taskList becomes sibling (lost nesting semantics
  if the outer container is not a taskList)
- EC-15 hoist to grandparent: visual association between task and sub-list broken
- EC-16 multi-paragraph flatten: paragraph breaks become line breaks
- hardBreak separator round-trip loss: hardBreaks injected by EC-16 do not
  survive the text round-trip

## Behavioral Contract Reference

BC-7.2.010 in `.factory/specs/prd/bc-7-output-render.md`

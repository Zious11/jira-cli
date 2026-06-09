# `markdown_to_adf` — GFM alerts → ADF `panel` with content-model normalization

**Issue:** [#483](https://github.com/Zious11/jira-cli/issues/483) (split from [#474](https://github.com/Zious11/jira-cli/issues/474))
**Module:** `src/adf.rs`
**Behavioral contract:** BC-7.2.009 (`.factory/specs/prd/bc-7-output-render.md`)

## Problem

GitHub-Flavored Markdown **alerts** are blockquotes tagged with a kind marker:

```markdown
> [!NOTE]
> Useful information.

> [!WARNING]
> Critical content demanding attention.
```

ADF has a first-class node for this — `panel` — but `markdown_to_adf` currently
does **not** enable alert parsing, so `> [!NOTE]` is rendered as a plain
`blockquote` with the literal text `[!NOTE]` surviving as the first line (the
deferral documented in the source comment and CLAUDE.md from #474).

Mapping alerts to `panel` is not a one-line change: it opens a content-model
normalization problem of the same class #470 solved for `listItem`. The ADF
`panel` content model **forbids** several nodes that pulldown-cmark can
legitimately emit inside an alert blockquote, and the `listItem` content model
forbids `panel` entirely. A naive mapping produces Jira **HTTP 400** on legal
markdown.

## Atlassian constraint (validated)

Verified against the authoritative `@atlaskit/adf-schema` 52.9.5 generated node
spec (the validator Jira's backend actually enforces — broader than the
simplified `developer.atlassian.com` docs). Research: Perplexity + primary-source
WebFetch, 2026-06-09.

### `panel.content` (verbatim content expression)

```
(paragraph | heading | bulletList | orderedList | blockCard | mediaGroup
 | mediaSingle | codeBlock | taskList | rule | decisionList
 | unsupportedBlock | extension)+
```

- **Allowed:** `paragraph` (no marks), `heading` (no marks), `bulletList`,
  `orderedList`, `codeBlock`, `taskList`, `rule`, `mediaGroup`, `mediaSingle`,
  `blockCard`, `decisionList`.
- **NOT allowed:** nested `panel`, `table`, `blockquote`.

### `listItem.content` (verbatim content expression)

```
(paragraph | bulletList | orderedList | taskList | mediaSingle | codeBlock
 | unsupportedBlock | extension)+
```

- **NOT allowed:** `panel`. So an alert appearing inside a list item cannot be a
  `panel` in place.

### `panelType` attribute

The schema accepts `info | note | tip | warning | error | success | custom`.
**This spec emits only the five portable types** `info | note | warning | error
| success`. `custom` requires extra `panelIcon`/`panelColor` attrs and an editor
feature flag; `tip` is under-documented and renders inconsistently across Jira
Cloud surfaces. Both are avoided for REST portability.

## pulldown-cmark API (validated)

Confirmed against pulldown-cmark 0.13.0 docs (CONFIRMED, all claims):

- `Options::ENABLE_GFM` enables alert parsing. In 0.13 it gates **only** the
  blockquote alert tags — it does **not** double-enable or conflict with the
  individually-set `ENABLE_TABLES` / `ENABLE_STRIKETHROUGH` / `ENABLE_FOOTNOTES`
  / `ENABLE_SUPERSCRIPT` / `ENABLE_SUBSCRIPT` / `ENABLE_HEADING_ATTRIBUTES`
  flags. Available with `default-features = false` (all `ENABLE_*` are runtime
  bitflags, not cargo features).
- A tagged alert emits `Tag::BlockQuote(Some(BlockQuoteKind))`.
  `BlockQuoteKind` variants are exactly `Note`, `Tip`, `Important`, `Warning`,
  `Caution`.
- A plain blockquote emits `Tag::BlockQuote(None)`.
- A disqualified marker falls back to `BlockQuote(None)` with `[!NOTE]`
  surviving as literal text. The renderer therefore keys off `Some(kind)` only
  and **never string-matches the text** for the marker.
- **Parser leniency (empirically verified against 0.13, not GitHub's stricter
  spec):** pulldown recognizes the marker with or without the leading space
  (`>[!NOTE]` works) and is case-insensitive (`[!note]`, `[!Note]` all map to
  the same kind). The only disqualifier observed is **trailing text on the
  marker line** (`> [!NOTE] extra` → plain blockquote) and an **unknown kind**
  (`[!FOO]` → plain blockquote).

## Design

### 1. Parser: enable GFM, carry the alert kind

Add `| Options::ENABLE_GFM` to the `markdown_to_adf` options block and replace
the deferral comment with a pointer to this spec.

The `AdfBuilder` `BlockQuote` handling becomes kind-aware:

- `Tag::BlockQuote(None)` → `NodeKind::BlockQuote` (unchanged — plain blockquote).
- `Tag::BlockQuote(Some(kind))` → a new `NodeKind::Panel { panel_type }` carrying
  the mapped panelType string.

`NodeKind::Panel` on `end()` emits `{ "type": "panel", "attrs": { "panelType":
<type> }, "content": normalize_panel_content(children) }`.

### GFM kind → panelType mapping

| `BlockQuoteKind` | `panelType` | Rationale |
|------------------|-------------|-----------|
| `Note`           | `info`      | Neutral/informational (blue). |
| `Tip`            | `success`   | Positive guidance (green); avoids risky `tip` type. |
| `Important`      | `note`      | "Take note" (purple); keeps `warning` slot unambiguous. |
| `Warning`        | `warning`   | Exact 1:1 (yellow). |
| `Caution`        | `error`     | Danger/red — matches GitHub's Caution styling. |

A single pure helper `panel_type_for(kind: BlockQuoteKind) -> &'static str`
owns this mapping (exhaustive match, no `_` arm — a future pulldown variant
forces a compile error rather than a silent default).

### 2. Panel content normalization (`normalize_panel_content`)

Mirrors `normalize_list_item_content`. Runs over a panel's children, transforming
every disallowed block into the permitted set, then the caller wraps loose inline
runs via `wrap_inlines_as_blocks` against the panel allowlist. Rules:

- **nested `panel`** → unwrapped: its children are spliced in and recursively
  normalized (handles `> [!NOTE]\n> > [!TIP]`, which pulldown nests as
  `panel > panel`). The inner panel's `panelType` is discarded; ADF cannot
  represent a panel-in-panel and there is no lossless alternative.
- **`blockquote`** → unwrapped: child blocks spliced in and recursively
  normalized (matches the `listItem` treatment; a `blockquote` inside a panel is
  invalid).
- **`table`** → flattened to one `paragraph` per row via the existing
  `flatten_table_to_paragraphs` (marks preserved; grid structure necessarily
  lost — identical to the `listItem` path).
- **`heading`** → **kept** (panel permits `heading`) but **stripped of node-level
  marks** so it satisfies `heading (no marks)`. See §4.
- **permitted blocks** (`paragraph`, `bulletList`, `orderedList`, `codeBlock`,
  `rule`, `taskList`, …) and loose inline nodes pass through untouched.

The panel allowlist passed to `wrap_inlines_as_blocks` is:
`["paragraph", "heading", "bulletList", "orderedList", "codeBlock", "rule"]`
(the subset reachable from markdown; media/card/decision/task nodes are never
produced by `markdown_to_adf` today).

### 3. `listItem` rejects `panel` (extend `normalize_list_item_content`)

A `panel` can land inside a `listItem` for markdown like:

```markdown
- item text
  > [!NOTE]
  > nested alert
```

`normalize_list_item_content` gains a `panel` arm: **unwrap** the panel — splice
its already-panel-normalized children in and recursively normalize them through
the listItem rules. Unwrapping (rather than degrading to a blockquote) is correct
because `blockquote` is itself disallowed inside `listItem`; unwrapping yields
permitted blocks directly and matches the existing blockquote arm's behavior.

### 4. No node-level marks leak into panel `paragraph`/`heading`

`panel.content` requires `paragraph (no marks)` and `heading (no marks)`. ADF
block nodes never carry a top-level `marks` array in this codebase's output (only
text leaf nodes do), so paragraphs are already compliant. As defense-in-depth and
to satisfy the contract explicitly, `normalize_panel_content` strips any
`marks` key found directly on a child `paragraph`/`heading` node (a no-op on
current output, but pins the invariant against future regressions and is asserted
by a unit test).

### 5. Reverse path (`adf_to_text`): render `panel` → `> [!KIND]`

`adf_to_text` currently has no `panel` arm — it falls through to the `_`
catch-all, which recurses into `content` and renders the inner blocks as bare
text (the `[!NOTE]` marker is lost). Add an explicit `panel` arm that:

1. Reads `attrs.panelType`, maps it back to a GFM kind label via
   `gfm_label_for_panel_type` (inverse of the forward table).
2. Renders children into a buffer, then prefixes the first line with
   `[!KIND]\n` and quotes every line with `> ` — reusing the exact
   line-prefixing logic already proven in the `blockquote` arm.

Inverse mapping (panelType → GFM label):

| `panelType` | label      |
|-------------|------------|
| `info`      | `NOTE`     |
| `success`   | `TIP`      |
| `note`      | `IMPORTANT`|
| `warning`   | `WARNING`  |
| `error`     | `CAUTION`  |
| other/unknown | (no marker — render as plain blockquote) |

**Round-trip:** `markdown_to_adf` → `adf_to_text` is **stable** for the five
alert kinds (`> [!NOTE]` → panel `info` → `> [!NOTE]`). It is **not** a perfect
inverse for content that was normalized (a table inside an alert flattens and
does not reconstitute) — identical to the documented `listItem`/table limitation.

### 6. `is_empty_block_container` — re-add `panel` to the prune set

An empty `panel` (`content: []`) is invalid ADF and would draw a Jira 400.
Pulldown can leave an empty shell after footnote-def hoisting (the #472 class).
Add `"panel"` to the `REQUIRES_CONTENT` array so empty panels are pruned, exactly
like `blockquote`/`heading`/etc.

## Edge cases

| Input | Expected output |
|-------|-----------------|
| `> [!NOTE]\n> text` | `panel` `info`, one `paragraph`. |
| `>[!NOTE]` (no space) | `panel` `info` (pulldown leniency — space optional). |
| `> [!note]` / `> [!Note]` (any case) | `panel` `info` (pulldown is case-insensitive). |
| `> [!NOTE] extra` (trailing text on marker line) | plain `blockquote`, `[!NOTE]` survives as text. |
| `> [!FOO]` (unknown kind) | plain `blockquote`, `[!FOO]` survives as text. |
| `> plain quote` | `blockquote` (unchanged). |
| `> [!NOTE]\n> > [!TIP]\n> > inner` | `panel` `info`; inner panel unwrapped, its paragraphs spliced in. |
| alert wrapping a table | `panel` with the table flattened to per-row paragraphs. |
| `- x\n  > [!NOTE]\n  > y` | `listItem` with the panel unwrapped to a `paragraph` (no `panel` child). |
| alert containing only a heading | `panel` with a mark-free `heading` child. |
| empty alert (`> [!NOTE]` then nothing) | pruned by `is_empty_block_container`. |
| `panel` `info` → `adf_to_text` | `> [!NOTE]\n> …`. |
| `panel` with unmapped `panelType` → `adf_to_text` | plain `> …` blockquote, no marker. |

## Test plan (TDD — tests first)

Unit tests in `src/adf.rs::tests` (prefix `test_` per the new naming
convention):

**Forward (markdown → ADF):**
- `test_markdown_alert_note_maps_to_panel_info`
- `test_markdown_alert_tip_maps_to_panel_success`
- `test_markdown_alert_important_maps_to_panel_note`
- `test_markdown_alert_warning_maps_to_panel_warning`
- `test_markdown_alert_caution_maps_to_panel_error`
- `test_markdown_alert_marker_with_trailing_text_stays_literal_blockquote`
- `test_markdown_alert_marker_without_leading_space_still_panel`
- `test_markdown_alert_marker_case_insensitive_still_panel`
- `test_markdown_unknown_alert_kind_stays_literal_blockquote`
- `test_markdown_plain_blockquote_unchanged`
- `test_markdown_nested_alert_unwraps_inner_panel`
- `test_markdown_alert_with_table_flattens_to_paragraphs`
- `test_markdown_alert_in_listitem_unwraps_panel`
- `test_markdown_alert_heading_child_has_no_marks`
- `test_markdown_empty_alert_pruned`
- `test_panel_content_only_permitted_node_types` (invariant scan over output)

**Reverse (ADF → text):**
- `test_render_panel_info_to_note_alert` (asserts the marker line is quoted)
- `test_render_panel_multiline_body_quotes_every_line`
- `test_render_panel_unknown_type_to_plain_blockquote`
- `test_render_panel_tip_type_renders_no_marker`

**Unit (direct helper coverage):**
- `test_normalize_panel_content_strips_paragraph_marks`
- `test_is_empty_block_container_membership` (extended with `panel`)

**Round-trip:**
- `test_alert_markdown_to_text_roundtrip_all_kinds`

**Live-Jira verification (needs-sandbox, gated `JR_RUN_E2E`):** POST each of the
five produced panel shapes plus the three normalized shapes (nested, table,
in-list) via `jr issue create --description-stdin` against the E2E sandbox; assert
no 400. Tracked under the issue's `needs-sandbox` item; add to `tests/e2e_live.rs`
if a sandbox surface is available, else document as a manual verification step.

## Out of scope

- `panelType: "custom"` / `"tip"` (portability — see Atlassian constraint).
- `mediaGroup`/`mediaSingle`/`blockCard`/`decisionList`/`taskList` inside panels:
  `markdown_to_adf` does not produce these nodes from any markdown input today.
- GFM task lists (`- [ ]`) → `taskList` — separate issue #471.

## References

- pulldown-cmark 0.13.0: `Options`, `Tag::BlockQuote`, `BlockQuoteKind`
  (docs.rs).
- `@atlaskit/adf-schema` 52.9.5 generated `panel` / `listItem` content
  expressions; `panel.js` panelType constants.
- `docs/specs/adf-listitem-content-model.md` (#470 — the normalization-pass
  precedent this spec mirrors).
- Prior ADF work: #470 (listItem), #472 (footnotes + empty-container pruning),
  #474 (subsup + heading attrs).

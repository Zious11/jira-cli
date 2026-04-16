# Markdown-to-ADF Conversion (Replace Hand-Rolled Parser)

> **Issue:** #197 — `bug: issue comment --markdown and wiki markup both render as plain text — no ADF conversion`

## Problem

`jr` accepts a `--markdown` flag on three commands that produce ADF bodies for Jira:

| Command | Call site |
|---|---|
| `jr issue comment --markdown` | `cli/issue/workflow.rs` (`handle_comment`) |
| `jr issue create --markdown` (description) | `cli/issue/create.rs` (`handle_create`) |
| `jr issue edit --markdown` (description) | `cli/issue/create.rs` (`handle_edit`) |

All three routed through `adf::markdown_to_adf()`. Before this change, that function was a hand-rolled line-by-line parser in `src/adf.rs` that handled only a partial subset of markdown: headings, bullet lists, code blocks, bold (`**`), inline code (`` ` ``).

**What is silently dropped:**

- `*italic*` / `_italic_` — emphasis
- `[text](url)` — inline links
- `1. item` — ordered lists
- `| a | b |` — tables
- `~~strike~~` — strikethrough
- `> blockquote` — blockquotes
- `---` — horizontal rules
- Hard breaks (two trailing spaces)
- Nested lists
- Escape sequences (`\*not italic\*`)
- Interleaved inline marks (`**bold _italic_ bold**`)

Each of these fell through to literal text inside a single `paragraph > text` node, which Jira rendered as raw source syntax. The issue's "wiki markup" half is out of scope — `jr` has never claimed wiki markup support, and the `--markdown` flag only covers markdown.

The bug's blast radius was all three commands simultaneously. A user running `jr issue create --markdown` got a broken description stored on the issue exactly as badly as `jr issue comment --markdown` produced a broken comment body.

## Design

### Core Change

Replace the internals of `adf::markdown_to_adf()` with a `pulldown_cmark::Parser` event stream and a stateful `AdfBuilder`. The public signature stays `fn markdown_to_adf(markdown: &str) -> serde_json::Value` — the three callers in `workflow.rs` and `create.rs` don't change.

**Dependency:** `pulldown-cmark = { version = "0.13", default-features = false }` added to `Cargo.toml`. Default features (`simd`, `html`, `serde`) are unused by our event-iteration path; disabling them keeps the dep light.

**Why pulldown-cmark vs extending the hand-rolled parser:** the old parser had correctness gaps even in the features it claimed to support (its `parse_inline` helper could only match the first delimiter of each type, had no escape handling, and its emphasis-vs-bold disambiguation collided on `*` / `**`). Adding each missing feature (links, italics, tables, nested lists, escape sequences, left/right flanking rules) would have meant re-implementing CommonMark by hand — the spec pulldown-cmark already passes. Used by rustdoc and mdbook; de facto Rust CommonMark parser.

**Why pulldown-cmark vs comrak:** comrak bundles an HTML renderer we'd never use and is heavier. markdown-rs (mdast) is less mature. pulldown-cmark's event-driven API maps cleanly to building an ADF tree.

### Data Flow

```
&str
  ↓ Parser::new_ext(input, Options::ENABLE_TABLES | ENABLE_STRIKETHROUGH)
  ↓ TextMergeStream::new(...)        // merges adjacent Event::Text runs so
                                      // marks and escape sequences coalesce
                                      // into a single text node
  ↓ Iterator<Event<'_>>
  ↓ AdfBuilder.process(event) for each event
  ↓ Vec<Value> (root block-level nodes)
  ↓ wrap in {"version":1,"type":"doc","content":[...]}
serde_json::Value
```

### Builder Architecture

Stack-based builder — the idiomatic pattern for event-stream → tree conversion (used internally by pulldown-cmark's HTML renderer; documented in pulldown-cmark's dev guide):

```rust
struct AdfBuilder {
    root: Vec<Value>,              // Top-level block nodes
    stack: Vec<PartialNode>,       // Open containers (Heading, Paragraph, List, Item, BlockQuote, Table, Row, Cell, ...)
    active_marks: Vec<Value>,      // Currently-open inline marks (strong, em, code, strike, link)
    in_table_head: bool,           // For distinguishing tableHeader from tableCell
}

struct PartialNode {
    kind: NodeKind,                // Which ADF node this will become
    children: Vec<Value>,
}
```

Attributes (heading level, code block language, ordered-list `order`, tableCell `is_header`) are carried on the `NodeKind` variant itself (e.g. `Heading(u8)`, `CodeBlock { language }`, `OrderedList { start }`, `TableCell { is_header }`) rather than on a separate `attrs` field, so the emission logic in `end()` can destructure the kind and build the final JSON object in one step.

`Event::Start(Tag::X)` pushes a `PartialNode`. `Event::End(TagEnd::X)` pops the stack, wraps `children` into the partial's content, and appends the completed `Value` to the parent's children (or to `root` if the stack is empty). Text / inline events append leaves to the current top-of-stack's children.

Inline marks use a parallel `active_marks` list. When `Text(s)` fires while marks are active, the emitted `{"type":"text","text":s}` node gets `"marks": [...active_marks]` attached.

### Feature Mapping

**Block nodes** (emit via `Event::Start(Tag::_) → Event::End(TagEnd::_)` pair):

| pulldown-cmark `Tag` | ADF node |
|---|---|
| `Heading { level, .. }` | `heading` with `attrs.level` (1-6) |
| `Paragraph` | `paragraph` |
| `BlockQuote(_)` | `blockquote` |
| `CodeBlock(CodeBlockKind::Fenced(lang))` | `codeBlock` with `attrs.language` if non-empty |
| `CodeBlock(CodeBlockKind::Indented)` | `codeBlock` with no language attr |
| `List(None)` | `bulletList` |
| `List(Some(1))` | `orderedList` (no `order` attr — defaults to 1) |
| `List(Some(n))` where n ≠ 1 | `orderedList` with `attrs.order = n` |
| `Item` | `listItem` |
| `Table(_)` | `table` (no attrs; alignment Vec ignored — ADF has no per-column alignment) |
| `TableHead` / `TableRow` | `tableRow` |
| `TableCell` (inside `TableHead`) | `tableHeader` |
| `TableCell` (elsewhere) | `tableCell` |

**Inline marks** (applied to every `Text` event emitted while the mark is active):

| pulldown-cmark `Tag` | ADF mark |
|---|---|
| `Strong` | `{"type": "strong"}` |
| `Emphasis` | `{"type": "em"}` |
| `Strikethrough` | `{"type": "strike"}` |
| `Link { dest_url, title, .. }` | `{"type": "link", "attrs": {"href": <dest_url>, "title"?: <title>}}` |

**Leaf events:**

| Event | ADF handling |
|---|---|
| `Text(s)` | Append `{"type":"text","text":s, "marks"?: [...active_marks]}` to current container |
| `Code(s)` | Append `{"type":"text","text":s,"marks":[{"type":"code"}, ...active_marks]}` |
| `SoftBreak` | Emit a single space — ADF has no soft-break node |
| `HardBreak` | Emit `{"type":"hardBreak"}` |
| `Rule` | Emit `{"type":"rule"}` at block level |

**Explicit non-mappings** (events we intentionally drop or render as fallback text):

| Event | Behavior | Reason |
|---|---|---|
| `Start(Tag::Image { .. })` through `End(TagEnd::Image)` | Skip — suppress events between until end | Jira Cloud `media` nodes require pre-upload to the media API with a returned media ID; external URLs aren't reliably supported in comments |
| `Html(s)` / `InlineHtml(s)` | Emit as literal text via `Text(s)` handling | Preserves user intent without attempting unsafe HTML-to-ADF mapping |
| `FootnoteReference(_)` / `Start(Tag::FootnoteDefinition(_))` | Skip | ADF has no native footnote nodes; would require embedding as inline links, low ROI |
| `- [x] task` / `- [ ] task` (GFM task list syntax) | `ENABLE_TASKLISTS` left unset — pulldown-cmark parses `[x]` and `[ ]` as literal text inside the list item, producing a regular `bulletList` with the brackets preserved in-text | ADF's `taskList`/`taskItem` / `blockTaskItem` schema support for Jira (vs Confluence) is unclear per available docs. Letting the brackets render as text is equivalent to manual prefixing but requires no extra event handling |
| `Tag::DefinitionList` / `DefinitionListTitle` / `DefinitionListDefinition` | Not enabled via Options — events won't fire | Out of scope for scope (b) |
| `Start(Tag::MetadataBlock(_))` | Not enabled via Options — events won't fire | Out of scope |

### Alignment with Atlassian's Own Behavior

Per-column table alignment (`|:---:|` center, `|---:|` right) is silently dropped. ADF's documented `tableCell` / `tableHeader` schema has no `align` or `text-align` attribute, and the only table-level `layout` values (`'center'`, `'align-start'`) control whole-table page alignment, not columns. Atlassian's own Jira web-UI markdown editor drops per-column alignment for the same reason. This is a graceful degradation consistent with the platform, not a bug in the converter.

### What Does NOT Change

| Item | Reason |
|---|---|
| Public signature of `markdown_to_adf` | Drop-in replacement; three call sites stay identical |
| `text_to_adf()` | Separate function for plain-text bodies (no markdown flag). Untouched |
| `adf_to_text()` | Reverse direction (ADF → text for `issue view`, `issue comments`). Separate concern; table/link render-back would be a follow-up issue if users ask |
| `Cargo.toml` feature flags for `pulldown-cmark` | Default-features-off. No SIMD (tiny inputs, unneeded), no HTML renderer, no event-level serde |
| Worklog comment path | `cli/worklog.rs:33` always uses `text_to_adf` (no `--markdown` flag exists there). Adding one is out of scope |
| Wiki markup (Atlassian wiki syntax) | Never supported; would be a separate `--wiki` flag if ever requested. Out of scope for #197 |
| `--verbose` body logging | PR #198 already ships request-body logging; users can diff ADF output via `jr --verbose ... 2>trace.log` |
| HTTP client, auth, retry logic | ADF is the payload; transport is unaffected |

## Files Changed

| File | Change |
|---|---|
| `Cargo.toml` | Add `pulldown-cmark = { version = "0.13", default-features = false }` under `[dependencies]` |
| `src/adf.rs` | Rewrite `markdown_to_adf()` body; keep `text_to_adf` and `adf_to_text` as-is; existing `parse_heading`, `parse_inline`, `flush_list` helpers deleted (replaced by builder) |
| `src/adf.rs` | New private `AdfBuilder` struct + `impl` for event processing (~300 lines) |
| `src/adf.rs` (tests) | Update existing `test_markdown_to_adf_snapshot` snapshot (richer output); add ~15 new feature-focused unit tests |
| `src/snapshots/jr__adf__tests__markdown_complex_to_adf.snap` | Regenerated to reflect structurally-correct ADF from pulldown-cmark |

## Testing

**Unit tests** in `src/adf.rs #[cfg(test)] mod tests` — one focused test per feature, asserting top-level ADF shape (not every byte). Matches the existing convention in `adf.rs` and pulldown-cmark's own `specs/*.txt` test pattern.

New tests:
- `test_markdown_italic_to_em_mark`
- `test_markdown_link_preserves_href_and_title`
- `test_markdown_ordered_list_omits_order_when_start_is_one`
- `test_markdown_ordered_list_sets_order_when_start_is_not_one`
- `test_markdown_strikethrough_to_strike_mark`
- `test_markdown_blockquote_wraps_children`
- `test_markdown_horizontal_rule`
- `test_markdown_hard_break`
- `test_markdown_nested_bullet_list`
- `test_markdown_table_cells_and_headers` — verify first row uses `tableHeader`, subsequent rows `tableCell`
- `test_markdown_image_is_skipped`
- `test_markdown_soft_break_becomes_space`
- `test_markdown_task_list_syntax_preserved_as_text` — `- [x] foo` ends up as a bullet item containing literal `[x] foo`
- `test_markdown_escape_literal_asterisk`
- `test_markdown_mixed_marks` — `**bold _italic_ bold**` preserves nested marks

Kept tests: the existing `test_markdown_heading`, `test_markdown_list`, `test_markdown_code_block`, `test_adf_roundtrip_heading`, `test_text_to_adf`, `test_adf_to_text_paragraph`, `test_adf_to_text_unsupported`, `test_adf_to_text_snapshot` — still valid, no rewrite.

**Snapshot test:** `test_markdown_to_adf_snapshot` (existing) — updated to exercise the union of features in one realistic prose block. Catches regressions from pulldown-cmark version bumps and builder refactors. Snapshot file regenerates on first run.

**Integration tests:** `tests/issue_commands.rs:631` uses `jr::adf::markdown_to_adf()` as the wiremock `body_partial_json` matcher source-of-truth for the `--markdown` write path. (Sibling tests at `:589` and `:671` exercise `text_to_adf` and are unaffected.) The markdown test auto-tracks whatever the new implementation emits — no test code change needed. It verifies the full CLI → client → HTTP path ships the new ADF shape.

**No proptest.** The codebase uses proptest only for pure symbolic round-trip functions (`partial_match`, `jql`, `duration`). Markdown→ADF is stateful event-driven; pulldown-cmark already passes the CommonMark spec suite upstream. Our responsibility is the mapping, not the parser.

## Error Handling

Infallible signature — `markdown_to_adf(&str) → Value` does not return `Result`. Same as today.

| Scenario | Behavior |
|---|---|
| Empty or whitespace-only input | Emit `{"version":1,"type":"doc","content":[]}` (same as today) |
| Malformed markdown (unclosed fences, unmatched emphasis) | pulldown-cmark treats as literal text; we consume events as-is. No error |
| Invalid UTF-8 | Not possible — `&str` input is UTF-8 by type |
| Adversarial / deeply nested input | pulldown-cmark has built-in DoS protections: `LINK_MAX_NESTED_PARENS = 32`, `MAX_AUTOCOMPLETED_CELLS = 2^18`, `link_ref_expansion_limit = max(text.len(), 100_000)`, brace-nesting switches from tracking to counting at 25 levels. Inherited for free |
| Malformed table (header/delimiter column mismatch) | pulldown-cmark emits paragraph events with literal `\|` text — we never see `Tag::Table`. No error path needed |
| `Parser::new_ext` | Never panics or returns `Result`. Always yields events, even for adversarial input. Confirmed: no known CVEs or DoS reports for pulldown-cmark |

## Alignment with Project Conventions

- **Thin client, no abstraction layer** — change is confined to `adf.rs`; no new module, no wrapper trait, no conversion pipeline
- **Single-responsibility module** — `adf.rs` grows from ~200 LOC to ~500 LOC but remains focused on ADF conversions (both directions)
- **Machine-output-first** — output is still deterministic JSON `Value`; no user-visible text changes from `jr` itself
- **Non-interactive** — no new prompts, flags, or interactive paths
- **TDD** — every new feature ships with a focused unit test; snapshot test pins end-to-end ADF shape

## Validation

- **Perplexity (2026-04-16):** pulldown-cmark is the de facto Rust CommonMark parser (used by rustdoc, mdbook). Alternatives (comrak, markdown-rs) are heavier or less mature. Stack-based builder is the idiomatic event→tree conversion pattern.
- **Perplexity + Atlassian dev docs (2026-04-16):**
  - `orderedList.attrs.order` (integer ≥ 0) IS supported — authoritative docs at `developer.atlassian.com/cloud/jira/platform/apis/document/nodes/orderedList/` confirm, overriding an earlier-less-reliable Perplexity answer
  - `table` node attrs are `layout` (`'center' | 'align-start'`), `isNumberColumnEnabled`, `width`, `displayMode` — per-column alignment not in schema
  - Header rows distinguished by `tableHeader` vs `tableCell` node type per cell
  - `link` mark: `href` required, `title` optional
  - Jira `media` nodes require pre-upload; external URLs unreliable — justifies skipping markdown images
- **Context7 `/pulldown-cmark/pulldown-cmark`:**
  - `Event::End(TagEnd)` (not `Event::End(Tag)`) — current API in 0.12+
  - `CowStr<'a>` carries event text (owned via `.to_string()` when building `Value`)
  - Stack-based builder matches pulldown-cmark's own HTML renderer approach
  - Built-in DoS protections documented (`LINK_MAX_NESTED_PARENS`, `MAX_AUTOCOMPLETED_CELLS`)
  - Malformed tables revert to paragraph at the parser level — no `Tag::Table` event emitted
- **Local verification (2026-04-16):** `cargo search pulldown-cmark` confirms latest `0.13.3`; project MSRV `rust-version = "1.85"` (`Cargo.toml:7`) well above pulldown-cmark's 1.74 requirement
- **Codebase audit:**
  - All three `--markdown` call sites (in `cli/issue/workflow.rs` and `cli/issue/create.rs`) use the identical `if markdown { markdown_to_adf } else { text_to_adf }` pattern — one rewrite fixes all three
  - The `adf_to_text` read-path (invoked from `cli/issue/list.rs` when displaying comments and descriptions) operates on ADF `Value`s; no coupling to the write-path parser shape
  - The insta snapshot at `src/snapshots/jr__adf__tests__markdown_complex_to_adf.snap` is regenerated on first test run per the codebase convention

## Out of Scope (Follow-Ups)

| Item | Why deferred |
|---|---|
| `adf_to_text` support for tables, links, strikethrough | Reverse direction; display-side concern. File as separate issue if `jr issue view` readers report raw ADF artifacts |
| Task list native `taskList`/`taskItem` mapping | ADF schema support for Jira (vs Confluence) is unclear per available docs. Text prefix fallback is safe; native mapping can land if/when Atlassian documents it for Jira |
| Markdown images via media-upload flow | Requires attachment upload, media ID lookup, error handling for auth/size — substantial scope on its own |
| Wiki markup (`h2. `, `*bold*`, etc.) → ADF | Atlassian-specific legacy format. Would need a new `--wiki` flag. File separately if requested |
| `--markdown` on `jr worklog add` | Not in issue scope; users have not requested worklog markdown. File separately if asked |
| Footnotes, definition lists, subscript, math | No clean ADF mapping; rare in Jira automation. Low ROI |

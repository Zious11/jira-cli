# ADF → Text Rich Rendering

**Status:** Proposed
**Issue:** [#202](https://github.com/Zious11/jira-cli/issues/202)
**Related:** [#197](https://github.com/Zious11/jira-cli/issues/197), [PR #201](https://github.com/Zious11/jira-cli/pull/201)

## Problem

`src/adf.rs::adf_to_text` is the read-path renderer used by `jr issue view` (description) and `jr issue comments` (body). After PR #201 replaced the write-path `markdown_to_adf` with a real `pulldown-cmark`-driven builder, the write path emits proper ADF for ordered lists, tables, inline marks, blockquotes, hard breaks, horizontal rules, and code blocks with language — but the read path is behind. Concretely:

- `orderedList` items render with `- ` bullet prefixes (same as `bulletList`) — numeric order is lost.
- `table` / `tableRow` / `tableCell` / `tableHeader` fall through a generic "render children" path, collapsing cells into a flat text stream with no visible structure.
- `link` marks render as their text only — the `href` attr is dropped.
- `strong` / `em` / `strike` / `code` marks are dropped — bold and italic are indistinguishable from plain.
- `blockquote` has no line marker — reads as a plain paragraph.
- `rule` is silently dropped.
- `hardBreak` is silently dropped.
- `codeBlock` ignores `attrs.language`.
- Unknown node types emit a debug string `[unsupported: <type>]` into user-facing output.

The renderer's consumers embed its output inside `comfy-table` cells with `ContentArrangement::Dynamic` (see `src/cli/issue/list.rs:651`, `666`, `756`). That context rules out ANSI escape codes (the cell wraps the string as-is) and rules out Unicode box-drawing (would compete with the outer table). The only signal available for distinguishing inline formatting is literal characters in the output.

## Design

### Core change

Replace the current free `render_node` / `render_children` pair with a stateful `AdfRenderer` struct living in `src/adf.rs` alongside `AdfBuilder`. The public API is unchanged:

```rust
pub fn adf_to_text(adf: &Value) -> String {
    let mut r = AdfRenderer::new();
    r.render_doc(adf);
    r.finish()
}
```

The struct carries the mutable state the new behavior needs — ordered-list counters, blockquote nesting depth — without having to thread those through function parameters on every recursive call. This mirrors the write-path `AdfBuilder` in the same file and matches the idiomatic Rust pattern used by `serde_json::PrettyFormatter`, `pulldown-cmark-to-cmark`, and `rustdoc` for tree-to-string rendering with nested state.

### Renderer shape

```rust
struct AdfRenderer {
    output: String,
    list_stack: Vec<ListFrame>,
    blockquote_depth: usize,
}

enum ListFrame {
    Bullet,
    Ordered { next_index: u64 },
}
```

- `list_stack.len()` replaces the old `depth: usize` parameter. Indent level for `listItem` is `list_stack.len() - 1`.
- Each `listItem` under `ListFrame::Ordered` increments `next_index` after rendering.
- `blockquote_depth` tracks nested `blockquote` nesting; prefix rendered into each line as `"> " * blockquote_depth`.

### Feature mapping

| ADF node | Text output |
|---|---|
| `text` | apply `marks` (see Marks below) |
| `paragraph` | children, then `\n` |
| `heading` (`attrs.level`) | `#` repeated `level` times, then space, then children, then `\n`. Required per schema; defaults to `level=1` defensively against corrupted input only. |
| `bulletList` | push `ListFrame::Bullet`, render children, pop |
| `orderedList` (`attrs.order`) | push `ListFrame::Ordered { next_index: order.unwrap_or(1) }`, render children, pop |
| `listItem` | `"  "` × `(list_stack.len() - 1)` + prefix (`- ` or `{n}. `) + children + `\n`. Increment counter if the enclosing frame is `Ordered`. |
| `blockquote` | increment `blockquote_depth`, render children, decrement. Every rendered line contributed while `blockquote_depth > 0` gets prefixed with `"> "` × depth. |
| `codeBlock` (`attrs.language`) | ` ```{lang}` fence line (empty lang = plain ` ``` `), code content, ` ``` ` close fence, `\n`. |
| `rule` | `---\n` |
| `hardBreak` | `\n`. Trailing-two-spaces (`  \n`) form is a markdown-source convention; plain text doesn't need it. |
| `table` | render each `tableRow`; between header row(s) and body rows, emit `\| --- \| --- \|\n` separator. `\n` after the whole table. |
| `tableRow` | `\| ` + cells joined by ` \| ` + ` \|\n`. After rendering, if any cell in this row was a `tableHeader`, emit the separator line with one dashed segment per cell in that row. |
| `tableHeader` / `tableCell` | render children flat — paragraphs inside cells collapse (no inner `\n`). |
| unknown **with** `content` array | recurse into `content`. Salvages `panel`, `nestedExpand`, and future additions without per-type code. |
| unknown **leaf** (no `content`) | emit nothing. Silently drops `mediaSingle`, `mention`, `status`, `emoji`, `date`, `inlineCard`, `taskList`. Explicit renderers for the user-visible ones are tracked as follow-up work. |

### Marks

| Mark | Wrap (innermost text → outermost) |
|---|---|
| `code` | `` `text` `` |
| `em` | `*text*` |
| `strong` | `**text**` |
| `strike` | `~~text~~` |
| `link` (`attrs.href`) | `[text](href)`. Required per schema; defaults to empty string defensively against corrupted input only. |
| unknown (`underline`, `textColor`, `subsup`, `backgroundColor`, …) | bare text, no syntax. |

Iteration order: walk `node.marks[]` in array order, applying each mark to wrap the accumulated text. Result: the last mark in the array ends up outermost in the output. This is deterministic for snapshot tests even when multiple marks coexist on one text node. CommonMark parses back such strings to an equivalent AST regardless of which mark is outermost on a single run, so the fixed order is semantically roundtrip-safe.

CLI convention context: tools like `gh`, `glow`, `mdcat`, and `bat` strip delimiters in plain-text mode because they also emit ANSI styling in colored mode. Our renderer never has ANSI available (cell context), so bare text would erase all distinction between bold, italic, strike, inline code, and plain. Markdown-syntax wrapping is the best signal available.

### Header-row detection (tables)

Per the ADF schema, header-ness lives on the cell (`tableHeader` vs `tableCell`), not the row — a `tableRow` may legally mix both. Jira Cloud's editor always emits the first row as all `tableHeader` in practice, but the renderer doesn't assume that. Rule:

> After rendering each `tableRow`, if any of its cells was a `tableHeader`, emit the `| --- | --- | ... |` separator with one dashed segment per cell.

The first all-header row produces the right markdown; the pathological "mixed header+body in one row" case produces valid output where the separator still appears after the row with any header.

### Data flow

```
adf_to_text(&Value)
    └─ AdfRenderer::new()
       └─ render_doc(&Value)           // iterates top-level content[]
          └─ render_block(&Value)       // dispatches on node.type
             ├─ paragraph → render_inline(&Value) + newline
             ├─ heading → # prefix + render_inline
             ├─ bulletList / orderedList → push frame, render_block on each listItem, pop
             ├─ listItem → indent + prefix + render_inline (+ nested block children)
             ├─ blockquote → ++depth, render children, --depth
             ├─ codeBlock → fence + raw text children + fence
             ├─ rule → ---
             ├─ table → render each tableRow; emit separator after any header row
             └─ unknown → recurse into content if present, else skip
          └─ render_inline(&Value)
             ├─ text → apply_marks(text, marks)
             ├─ hardBreak → newline
             └─ unknown → recurse into content if present, else skip
```

### Blockquote line-prefixing

Implementation: before rendering a blockquote's children, record `output.len()`. After rendering, split the appended segment on `\n` and prefix each line with `"> "` × `blockquote_depth`. This means the prefix applies uniformly to paragraph text, nested list items, code blocks, and anything else rendered within. Nesting compounds (`> > `) because each enclosing blockquote applies its own prefix on unwind.

## Testing

### Unit tests (one per AC item + edge cases)

| Test | Scenario |
|---|---|
| `test_render_ordered_list_numeric_prefix` | 3 items, no `attrs.order`, asserts `1.` / `2.` / `3.` |
| `test_render_ordered_list_respects_attrs_order` | `attrs.order: 5`, asserts `5.` / `6.` |
| `test_render_nested_bullet_lists_indent` | outer/inner bulletList, asserts `-\n  -` |
| `test_render_mixed_nested_lists` | orderedList containing bulletList, asserts `1.\n  -` |
| `test_render_link_preserves_href` | link mark, asserts `[text](url)` |
| `test_render_link_with_no_href` | malformed link, asserts `[text]()` |
| `test_render_strong_mark` | asserts `**text**` |
| `test_render_em_mark` | asserts `*text*` |
| `test_render_strike_mark` | asserts `~~text~~` |
| `test_render_code_mark` | asserts `` `text` `` |
| `test_render_unknown_mark_drops_syntax` | `underline` mark, asserts bare text |
| `test_render_multiple_marks_deterministic_order` | `[strong, em]`, asserts fixed output |
| `test_render_blockquote_prefixes_each_line` | multi-line paragraph, asserts every line starts `>` |
| `test_render_nested_blockquote` | doubly-nested, asserts `> >` |
| `test_render_rule` | asserts `---` on its own line |
| `test_render_hard_break_inserts_newline` | paragraph with hardBreak |
| `test_render_code_block_with_language` | asserts `` ```rust\n...\n``` `` |
| `test_render_code_block_without_language` | asserts empty fence |
| `test_render_table_pipe_format` | 2×2 table with header, asserts header row + `\| --- \|` separator + body rows |
| `test_render_table_mixed_header_cell_row` | row with both types, asserts separator still emitted |
| `test_render_unknown_container_recurses` | `{type:"panel", content:[...]}`, asserts inner text visible |
| `test_render_unknown_leaf_drops_silently` | `{type:"mediaSingle"}`, asserts empty |
| `test_render_malformed_heading_defaults_level_1` | no `attrs.level`, asserts `# text` |

### Snapshot test

Rewrite `test_adf_to_text_snapshot` (currently adf.rs:766) with a rich doc exercising: heading at level 2, paragraph with mixed marks, nested lists mixing bullet and ordered, blockquote, `rule`, `hardBreak`, `codeBlock` with `attrs.language`, and a 2-row table with headers. Stored as insta snapshot `adf_to_text_complex`.

### Roundtrip test

`test_markdown_to_adf_to_text_roundtrip` — feed a moderately rich markdown string through `markdown_to_adf`, then through `adf_to_text`, and assert that re-parsing the output via `markdown_to_adf` produces structurally-equivalent ADF. Structural equivalence compares node types in traversal order and depth, and treats `marks` as a set per text node (ignores array order — CommonMark renders both orderings of e.g. `{em, strong}` as `***text***`). Whitespace-only text differences are ignored. Catches regressions in mark emission and list-numbering, and tightens the contract between the two sides.

### Updated test

`test_adf_to_text_unsupported` (currently adf.rs:460) asserts `[unsupported: mediaGroup]`. Updated contract:
- unknown leaf → empty string
- unknown container (has `content` array) → recurse into children, text of recognized descendants appears

Two small tests replace the old one.

### No integration tests

`adf_to_text` is a pure function with no I/O, no config, no async. Existing `jr issue view` and `jr issue comments` integration tests that pass through it cover the wiring. No new integration tests needed.

## Error Handling

One UX: best-effort render; never panic, never return an error, never emit debug syntax.

- Use `serde_json::Value::get` + typed accessors (`.as_str`, `.as_array`, `.as_u64`) returning `Option`; chain with `.and_then` and default with `.unwrap_or` / `.unwrap_or_default`. No `.unwrap()` or `.expect()` anywhere in the renderer.
- Missing optional fields (`codeBlock.attrs.language`, link `attrs.title`) → sensible empty default.
- Missing required fields (`heading.attrs.level`, `link.attrs.href`) → defensive fallback (`level=1`, empty href). The ADF schema requires these; the fallback exists as defense-in-depth against corrupted input, not a normal path.
- Malformed value types (e.g., `text` field on text node is an object not string) → treat as empty text. Matches the write-path's tolerance for unexpected event shapes.

## Validation Summary

Decisions confirmed through Perplexity and Context7 during brainstorming:

- **Stateful `&mut self` visitor struct** is the idiomatic Rust pattern for tree-to-string renderers tracking nested state (`serde_json::PrettyFormatter`, `pulldown-cmark-to-cmark`, `rustdoc`).
- **Markdown-syntax wrapping for inline marks** diverges from the `gh`/`glow`/`bat` convention of "strip delimiters in plain-text mode" — defensible here because their plain-text mode is an ANSI-colored mode's fallback, while our output is consumed in a context (inside `comfy-table` cells) where ANSI is never available. Bare text would erase all inline distinctions.
- **ADF `link.attrs.href`** required and non-empty per schema; `heading.attrs.level` required per schema; `codeBlock.attrs.language` optional.
- **ADF `tableRow` can mix `tableHeader` and `tableCell`** per schema — header-ness is per-cell. Rule checks for any header in the row, not "first row is header."
- **CommonMark nested blockquote:** `> >` with spaces, per line, not `>>`. Our design matches.
- **Plain `\n` for hardBreak** in plain-text output — trailing-two-spaces is source-level syntax, not rendered-text convention.
- **Graceful fallback** (recurse into unknown containers, silently drop unknown leaves) precedented in the `flexydox/issue-pr-commenter-action` ADF→markdown converter and related npm tooling.

## Out of Scope (Follow-Ups)

These are realistic ADF content types a Jira ticket might contain, deferred to separate issues:

- **`mention`** → render as `@display_name`. Very common in real Jira content; worth doing but needs its own resolution plan (attrs contain `id` + `text`; prefer `text` if present).
- **`status` / `emoji` / `date` / `inlineCard`** → one-line text renderers per type.
- **`mediaSingle` / `mediaGroup` / `media`** → `[image: filename]` or similar. Jira native attachments; rendering them inline is ambiguous.
- **`taskList` / `taskItem`** → `- [x]` / `- [ ]` style. Requires a new list frame variant.
- **`panel`** (with type info/note/warning/error) → prefix with `> Note:` etc. Currently covered partially by the unknown-container fallback (content renders, panel type lost).
- **Nested mark types beyond the AC** — `underline`, `textColor`, `subsup`, `backgroundColor` — bare-text fallback is already correct; dedicated formatting could come later if needed.

Each of the above is a small, independent follow-up. Handling them here would inflate scope without serving the immediate fix of #202's acceptance criteria.

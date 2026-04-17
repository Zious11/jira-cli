# Markdown-to-ADF Conversion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hand-rolled `adf::markdown_to_adf()` with a `pulldown-cmark`-based event-to-ADF converter so `--markdown` produces structurally-correct ADF on `issue comment`, `issue create`, and `issue edit`.

**Architecture:** Single-file change to `src/adf.rs`. A new private `AdfBuilder` consumes `pulldown_cmark::Event`s via a node stack and active-marks list, producing `serde_json::Value`. Public signature `fn markdown_to_adf(&str) -> Value` unchanged — three call sites untouched. Full design in `docs/superpowers/specs/2026-04-16-markdown-to-adf-conversion-design.md`.

**Tech Stack:** Rust 2024 edition (MSRV 1.85), `pulldown-cmark = 0.13` (CommonMark parser), `serde_json::Value` (ADF carrier), `insta` (snapshot tests).

---

## File Structure

| File | Change |
|---|---|
| `Cargo.toml` | Add `pulldown-cmark = { version = "0.13", default-features = false }` |
| `src/adf.rs` | Rewrite `markdown_to_adf()` internals; add private `AdfBuilder`; keep `text_to_adf`, `adf_to_text` as-is; delete obsolete helpers `parse_heading`, `parse_inline`, `flush_list` |
| `src/adf.rs #[cfg(test)]` | Add ~13 new feature-focused unit tests; regenerate the existing `test_markdown_to_adf_snapshot` |
| `src/snapshots/jr__adf__tests__markdown_complex_to_adf.snap` | Regenerated (accepted via `cargo insta accept`) |

No new files, no new modules. `tests/issue_commands.rs` is unchanged — its markdown call site (`:631`) imports `jr::adf::markdown_to_adf` and uses it as the wiremock matcher's source-of-truth, so it auto-tracks whatever the new implementation emits. (Sibling tests at `:589` and `:671` use `text_to_adf` and are unaffected.)

## Task Decomposition

Four tasks, each a `RED → GREEN → COMMIT` cycle. Task 1 is the biggest (scaffolding + block nodes); Tasks 2–4 add incremental capabilities. All code edits happen inside `src/adf.rs`.

---

### Task 1: Add pulldown-cmark dep, scaffold `AdfBuilder`, implement block nodes and leaf events

**Scope:** Wire up the replacement, handle block-level Tags and leaf events. This task keeps all pre-existing passing tests green and adds tests for the new block-level features (ordered-list `order` attr, hard break, rule, nested lists, blockquote). Inline marks come in Task 2.

**Files:**
- Modify: `Cargo.toml` (dependencies section)
- Modify: `src/adf.rs` — replace `markdown_to_adf()` body (lines 18-75), delete `parse_heading` (86-98), `parse_inline` (100-144), `flush_list` (77-84); add new `AdfBuilder`
- Modify: `src/adf.rs #[cfg(test)]` — update the complex snapshot test input to exercise more features

- [ ] **Step 1.1: Add `pulldown-cmark` to `Cargo.toml`**

Locate the `[dependencies]` section of `Cargo.toml` and add:

```toml
pulldown-cmark = { version = "0.13", default-features = false }
```

Run `cargo check` to fetch and confirm the dep compiles.

Expected: `Compiling pulldown-cmark v0.13.x` followed by a clean compile.

- [ ] **Step 1.2: RED — add a failing test for ordered lists with non-1 start**

The current hand-rolled parser does not emit `orderedList` at all (it ignores `1.` prefixes). Add this test to the bottom of `src/adf.rs`'s `mod tests`:

```rust
#[test]
fn test_markdown_ordered_list_sets_order_when_start_is_not_one() {
    let adf = markdown_to_adf("5. first\n6. second");
    assert_eq!(adf["content"][0]["type"], "orderedList");
    assert_eq!(adf["content"][0]["attrs"]["order"], 5);
    assert_eq!(adf["content"][0]["content"][0]["type"], "listItem");
}
```

- [ ] **Step 1.3: Run the new test to verify it fails**

Run: `cargo test --lib adf:: test_markdown_ordered_list_sets_order_when_start_is_not_one`

Expected: FAIL. The current implementation has no concept of ordered lists; output is a `paragraph` with literal `"5. first\n6. second"` text.

- [ ] **Step 1.4: GREEN — rewrite `markdown_to_adf()` using `pulldown-cmark` + `AdfBuilder`**

Replace lines 18-144 of `src/adf.rs` (everything from `pub fn markdown_to_adf` through the end of `parse_inline` and `render_node`'s sibling helpers — keep `adf_to_text`, `render_node`, `render_children`, `text_to_adf` untouched) with:

```rust
use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser,
    Tag, TagEnd,
};

pub fn markdown_to_adf(markdown: &str) -> Value {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let parser = Parser::new_ext(markdown, options);
    let mut builder = AdfBuilder::new();
    for event in parser {
        builder.process(event);
    }
    json!({
        "version": 1,
        "type": "doc",
        "content": builder.finish(),
    })
}

struct AdfBuilder {
    // Top-level block nodes collected for the final `doc.content` array.
    root: Vec<Value>,
    // Open containers. Each holds the node-kind metadata needed to emit on close,
    // plus its accumulated children.
    stack: Vec<PartialNode>,
    // Active inline marks applied to every Text event while any are open.
    active_marks: Vec<Value>,
    // Tracks whether we're between TableHead start/end so cells emit as tableHeader.
    in_table_head: bool,
}

struct PartialNode {
    // Serialized kind + attrs for the node to emit on End.
    // We construct the Value at End time so `children` can be appended.
    kind: NodeKind,
    children: Vec<Value>,
}

enum NodeKind {
    Paragraph,
    Heading(u8),
    BlockQuote,
    CodeBlock { language: Option<String> },
    BulletList,
    OrderedList { start: u64 },
    ListItem,
    // For events we want to swallow entirely (e.g., Image contents, unhandled tags).
    Sink,
}

// Tasks 2 and 3 will extend this enum with:
// - `InlineMark` (Task 2) for Strong/Emphasis/Strikethrough/Link container tracking
// - `Table`, `TableRow`, `TableCell { is_header: bool }` (Task 3) for GFM tables

impl AdfBuilder {
    fn new() -> Self {
        Self {
            root: Vec::new(),
            stack: Vec::new(),
            active_marks: Vec::new(),
            in_table_head: false,
        }
    }

    fn process(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag_end) => self.end(tag_end),
            Event::Text(text) => self.push_text(text.as_ref()),
            Event::Code(text) => self.push_code(text.as_ref()),
            Event::Html(html) | Event::InlineHtml(html) => self.push_text(html.as_ref()),
            Event::SoftBreak => self.push_text(" "),
            Event::HardBreak => self.append_child(json!({ "type": "hardBreak" })),
            Event::Rule => self.append_child(json!({ "type": "rule" })),
            // Footnotes / math / task markers with unset options: ignored.
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        // Task 2 fills in inline-mark arms (Strong, Emphasis, Strikethrough, Link).
        // Task 3 fills in Table/TableHead/TableRow/TableCell arms.
        match tag {
            Tag::Paragraph => self.push(NodeKind::Paragraph),
            Tag::Heading { level, .. } => self.push(NodeKind::Heading(heading_level_to_u8(level))),
            Tag::BlockQuote(_) => self.push(NodeKind::BlockQuote),
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.into_string()),
                    _ => None,
                };
                self.push(NodeKind::CodeBlock { language });
            }
            Tag::List(None) => self.push(NodeKind::BulletList),
            Tag::List(Some(start)) => self.push(NodeKind::OrderedList { start }),
            Tag::Item => self.push(NodeKind::ListItem),
            Tag::Image { .. } => self.push(NodeKind::Sink),
            // Everything else (inline marks, tables, html block, footnotes, etc.)
            // handled in later tasks or ignored.
            _ => self.push(NodeKind::Sink),
        }
    }

    fn end(&mut self, _tag_end: TagEnd) {
        let Some(partial) = self.stack.pop() else {
            return;
        };
        let PartialNode { kind, children } = partial;
        let node = match kind {
            NodeKind::Paragraph => Some(json!({ "type": "paragraph", "content": children })),
            NodeKind::Heading(level) => Some(json!({
                "type": "heading",
                "attrs": { "level": level },
                "content": children,
            })),
            NodeKind::BlockQuote => Some(json!({ "type": "blockquote", "content": children })),
            NodeKind::CodeBlock { language } => {
                let mut node = json!({ "type": "codeBlock", "content": children });
                if let Some(lang) = language {
                    node["attrs"] = json!({ "language": lang });
                }
                Some(node)
            }
            NodeKind::BulletList => Some(json!({ "type": "bulletList", "content": children })),
            NodeKind::OrderedList { start } => {
                let mut node = json!({ "type": "orderedList", "content": children });
                if start != 1 {
                    node["attrs"] = json!({ "order": start });
                }
                Some(node)
            }
            NodeKind::ListItem => Some(json!({ "type": "listItem", "content": children })),
            NodeKind::Sink => None,
        };
        if let Some(node) = node {
            self.append_child(node);
        }
    }

    fn push(&mut self, kind: NodeKind) {
        self.stack.push(PartialNode {
            kind,
            children: Vec::new(),
        });
    }

    fn append_child(&mut self, node: Value) {
        if let Some(top) = self.stack.last_mut() {
            if !matches!(top.kind, NodeKind::Sink) {
                top.children.push(node);
            }
        } else {
            self.root.push(node);
        }
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // Sink swallows text (e.g., inside images).
        if let Some(top) = self.stack.last() {
            if matches!(top.kind, NodeKind::Sink) {
                return;
            }
        }
        let mut node = json!({ "type": "text", "text": text });
        if !self.active_marks.is_empty() {
            node["marks"] = json!(self.active_marks);
        }
        self.append_child(node);
    }

    fn push_code(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(top) = self.stack.last() {
            if matches!(top.kind, NodeKind::Sink) {
                return;
            }
        }
        // Code events get the `code` mark alongside any active marks.
        let mut marks = self.active_marks.clone();
        marks.push(json!({ "type": "code" }));
        self.append_child(json!({
            "type": "text",
            "text": text,
            "marks": marks,
        }));
    }

    fn finish(self) -> Vec<Value> {
        self.root
    }
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
```

Delete these unused items:
- `fn flush_list` (was lines 77-84)
- `fn parse_heading` (was lines 86-98)
- `fn parse_inline` (was lines 100-144)

Keep `text_to_adf`, `adf_to_text`, `render_node`, `render_children` intact (unchanged).

The unused-import warning suppression: if `Alignment`, `BlockQuoteKind`, `LinkType` show as unused after this task (Task 2/3 use them), prefix with `#[allow(unused_imports)]` on the use statement OR drop them and re-add in the task that uses them. Prefer the latter: start with only the imports this task actually uses.

**Minimal imports for Task 1:**

```rust
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
```

- [ ] **Step 1.5: Add remaining block-level tests**

Add these tests to `mod tests` (after the existing ones):

```rust
#[test]
fn test_markdown_ordered_list_omits_order_when_start_is_one() {
    let adf = markdown_to_adf("1. alpha\n2. beta");
    assert_eq!(adf["content"][0]["type"], "orderedList");
    assert!(adf["content"][0]["attrs"].is_null());
}

#[test]
fn test_markdown_hard_break() {
    // Two trailing spaces then newline = hard break.
    let adf = markdown_to_adf("line one  \nline two");
    // The paragraph should contain text, hardBreak, text.
    let para = &adf["content"][0];
    assert_eq!(para["type"], "paragraph");
    let contents = para["content"].as_array().unwrap();
    assert!(contents.iter().any(|n| n["type"] == "hardBreak"));
}

#[test]
fn test_markdown_horizontal_rule() {
    let adf = markdown_to_adf("above\n\n---\n\nbelow");
    let has_rule = adf["content"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["type"] == "rule");
    assert!(has_rule, "expected a rule node, got: {adf}");
}

#[test]
fn test_markdown_soft_break_becomes_space() {
    // Single newline inside a paragraph = soft break, which we render as a space.
    let adf = markdown_to_adf("first line\nsecond line");
    let para = &adf["content"][0];
    let text = para["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["text"].as_str())
        .collect::<String>();
    assert_eq!(text, "first line second line");
}

#[test]
fn test_markdown_nested_bullet_list() {
    let adf = markdown_to_adf("- outer\n  - inner");
    let outer_list = &adf["content"][0];
    assert_eq!(outer_list["type"], "bulletList");
    let outer_item = &outer_list["content"][0];
    assert_eq!(outer_item["type"], "listItem");
    // The inner list is a block-level child of the outer listItem.
    let has_inner = outer_item["content"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["type"] == "bulletList");
    assert!(has_inner, "expected nested bulletList, got: {outer_item}");
}

#[test]
fn test_markdown_blockquote_wraps_children() {
    let adf = markdown_to_adf("> quoted text");
    let bq = &adf["content"][0];
    assert_eq!(bq["type"], "blockquote");
    let para = &bq["content"][0];
    assert_eq!(para["type"], "paragraph");
    assert_eq!(para["content"][0]["text"], "quoted text");
}

#[test]
fn test_markdown_code_block_with_language() {
    let adf = markdown_to_adf("```rust\nfn x() {}\n```");
    let block = &adf["content"][0];
    assert_eq!(block["type"], "codeBlock");
    assert_eq!(block["attrs"]["language"], "rust");
    // Code content should be present as a text node.
    assert_eq!(block["content"][0]["text"], "fn x() {}\n");
}

#[test]
fn test_markdown_empty_input() {
    let adf = markdown_to_adf("");
    assert_eq!(adf["type"], "doc");
    assert_eq!(adf["content"], json!([]));
}
```

- [ ] **Step 1.6: Run the affected tests to confirm they pass**

Run: `cargo test --lib adf::`

Expected: All new tests pass. Pre-existing tests (`test_text_to_adf`, `test_adf_to_text_paragraph`, `test_markdown_heading`, `test_markdown_list`, `test_markdown_code_block`, `test_adf_roundtrip_heading`, `test_adf_to_text_unsupported`, `test_adf_to_text_snapshot`) also pass.

The `test_markdown_to_adf_snapshot` will fail with a snapshot mismatch — that's expected and will be addressed in Task 4.

- [ ] **Step 1.7: Commit**

```bash
git add Cargo.toml Cargo.lock src/adf.rs
git commit -m "$(cat <<'EOF'
feat: scaffold pulldown-cmark-based AdfBuilder for markdown_to_adf (#197)

Replaces the hand-rolled line-by-line parser with a pulldown-cmark Parser
event stream consumed by a stateful AdfBuilder. This task covers block
nodes (paragraph, heading, blockquote, lists, codeBlock) and leaf events
(hardBreak, softBreak, rule). Inline marks and tables follow in
subsequent tasks.

Existing tests continue to pass; snapshot test will be regenerated in
the cleanup task.
EOF
)"
```

---

### Task 2: Inline marks — Strong, Emphasis, Strikethrough, Link

**Scope:** Teach `AdfBuilder` about inline marks. Emphasis + Strong are core CommonMark and need no flag. Strikethrough is already enabled via `ENABLE_STRIKETHROUGH` set in Task 1. Links are core CommonMark.

**Files:**
- Modify: `src/adf.rs` — extend `NodeKind`, extend `start()` and `end()` match arms, add helpers for mark lifecycle

- [ ] **Step 2.1: RED — add tests for each mark**

Append to `mod tests`:

```rust
#[test]
fn test_markdown_italic_to_em_mark() {
    let adf = markdown_to_adf("*italic words*");
    let text_node = &adf["content"][0]["content"][0];
    assert_eq!(text_node["type"], "text");
    assert_eq!(text_node["text"], "italic words");
    assert_eq!(text_node["marks"][0]["type"], "em");
}

#[test]
fn test_markdown_bold_to_strong_mark() {
    let adf = markdown_to_adf("**bold words**");
    let text_node = &adf["content"][0]["content"][0];
    assert_eq!(text_node["text"], "bold words");
    assert_eq!(text_node["marks"][0]["type"], "strong");
}

#[test]
fn test_markdown_strikethrough_to_strike_mark() {
    let adf = markdown_to_adf("~~gone~~");
    let text_node = &adf["content"][0]["content"][0];
    assert_eq!(text_node["text"], "gone");
    assert_eq!(text_node["marks"][0]["type"], "strike");
}

#[test]
fn test_markdown_link_preserves_href_and_no_title() {
    let adf = markdown_to_adf("[jr](https://example.com/jr)");
    let text_node = &adf["content"][0]["content"][0];
    assert_eq!(text_node["text"], "jr");
    let mark = &text_node["marks"][0];
    assert_eq!(mark["type"], "link");
    assert_eq!(mark["attrs"]["href"], "https://example.com/jr");
    // Title is absent when not provided in markdown.
    assert!(mark["attrs"]["title"].is_null());
}

#[test]
fn test_markdown_link_preserves_href_and_title() {
    let adf = markdown_to_adf(r#"[jr](https://example.com/jr "JR docs")"#);
    let mark = &adf["content"][0]["content"][0]["marks"][0];
    assert_eq!(mark["type"], "link");
    assert_eq!(mark["attrs"]["href"], "https://example.com/jr");
    assert_eq!(mark["attrs"]["title"], "JR docs");
}

#[test]
fn test_markdown_mixed_marks() {
    // "**bold *italic* bold**" — nested emphasis should produce two separate
    // text nodes with overlapping/non-overlapping marks as pulldown-cmark parses them.
    let adf = markdown_to_adf("**bold _italic_ bold**");
    let content = adf["content"][0]["content"].as_array().unwrap();
    // At minimum, every text node here should have the `strong` mark.
    assert!(
        content
            .iter()
            .all(|n| n["marks"].as_array().is_some_and(
                |m| m.iter().any(|mk| mk["type"] == "strong")
            )),
        "every text node should carry strong, got: {content:?}"
    );
    // And at least one node should also carry `em`.
    assert!(
        content.iter().any(|n| n["marks"].as_array().is_some_and(
            |m| m.iter().any(|mk| mk["type"] == "em")
        )),
        "expected at least one node with em + strong, got: {content:?}"
    );
}

#[test]
fn test_markdown_escape_literal_asterisk() {
    let adf = markdown_to_adf(r"\*not italic\*");
    let text_node = &adf["content"][0]["content"][0];
    assert_eq!(text_node["text"], "*not italic*");
    // No em mark because backslash escapes the asterisks.
    assert!(text_node["marks"].is_null());
}
```

- [ ] **Step 2.2: Run tests to verify they fail**

Run: `cargo test --lib adf::test_markdown_italic adf::test_markdown_bold adf::test_markdown_strikethrough adf::test_markdown_link adf::test_markdown_mixed adf::test_markdown_escape`

Expected: FAIL — text nodes emit without marks (all current NodeKind::Sink).

- [ ] **Step 2.3: GREEN — implement inline-mark handling**

In `src/adf.rs`, update the `use pulldown_cmark::...` line to include the additional identifiers for this task:

```rust
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};
```

Add the `InlineMark` variant to `NodeKind`. Append to the `enum NodeKind` definition (after `Sink`):

```rust
    // Container for inline marks. Has no ADF node; just manages the active_marks stack
    // so End events pop cleanly.
    InlineMark,
```

Extend the `start()` match arms. Replace the existing `Tag::Image { .. } => self.push(NodeKind::Sink),` arm and append just before the final `_ => self.push(NodeKind::Sink),` catch-all:

```rust
            Tag::Strong => self.push_mark(json!({ "type": "strong" })),
            Tag::Emphasis => self.push_mark(json!({ "type": "em" })),
            Tag::Strikethrough => self.push_mark(json!({ "type": "strike" })),
            Tag::Link { dest_url, title, .. } => {
                let mut attrs = serde_json::Map::new();
                attrs.insert("href".to_string(), json!(dest_url.as_ref()));
                if !title.is_empty() {
                    attrs.insert("title".to_string(), json!(title.as_ref()));
                }
                self.push_mark(json!({ "type": "link", "attrs": attrs }));
            }
            Tag::Image { .. } => self.push(NodeKind::Sink),
```

Add `push_mark` and `pop_mark` methods inside `impl AdfBuilder`:

```rust
    fn push_mark(&mut self, mark: Value) {
        self.active_marks.push(mark);
        self.push(NodeKind::InlineMark);
    }

    fn pop_mark(&mut self) {
        self.active_marks.pop();
    }
```

Update the `end()` method's match so `InlineMark` pops the mark. Replace the existing `NodeKind::Sink => None,` arm with:

```rust
            NodeKind::InlineMark => {
                self.pop_mark();
                None
            }
            NodeKind::Sink => None,
```

Note: the `LinkType` import may look unused — it isn't referenced directly by our match (we destructure `Link { dest_url, title, .. }` ignoring `link_type`). Drop `LinkType` from the `use` statement; keep only what's named:

```rust
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
```

(That's identical to Task 1's import line — no change actually needed for Task 2.)

- [ ] **Step 2.4: Run the new tests to verify they pass**

Run: `cargo test --lib adf::test_markdown_italic adf::test_markdown_bold adf::test_markdown_strikethrough adf::test_markdown_link adf::test_markdown_mixed adf::test_markdown_escape`

Expected: all 7 pass.

- [ ] **Step 2.5: Re-run the full adf test module to check for regressions**

Run: `cargo test --lib adf::`

Expected: all tests except `test_markdown_to_adf_snapshot` pass (the snapshot is still pending Task 4).

- [ ] **Step 2.6: Commit**

```bash
git add src/adf.rs
git commit -m "feat: add inline-mark handling (strong, em, strike, link) to AdfBuilder (#197)"
```

---

### Task 3: Tables — `Table`, `TableHead`, `TableRow`, `TableCell` with header cell distinction

**Scope:** Handle the four table-related Tags. First row's cells emit as `tableHeader`; subsequent rows' cells emit as `tableCell`. We ignore column alignment (ADF has no per-column alignment attribute).

**Important event-shape note:** pulldown-cmark emits `Tag::TableHead` containing `Tag::TableCell` events directly — there is NO `Tag::TableRow` inside `TableHead`. Only body rows emit `Tag::TableRow`. That's why our `Tag::TableHead` arm pushes a synthetic `NodeKind::TableRow` (so the header row still becomes an ADF `tableRow` node containing `tableHeader` cells, which is what the ADF schema requires).

Event sequence for `| foo | bar |\n| --- | --- |\n| baz | qux |`:

```
Start(Table) → Start(TableHead) → Start(TableCell) → Text("foo") → End(TableCell) → ... → End(TableHead)
             → Start(TableRow)  → Start(TableCell) → Text("baz") → End(TableCell) → ... → End(TableRow)
             → End(Table)
```

**Files:**
- Modify: `src/adf.rs` — extend `start()` / `end()` match arms for table tags, wire up the `in_table_head` flag

- [ ] **Step 3.1: RED — add a table test**

Append to `mod tests`:

```rust
#[test]
fn test_markdown_table_cells_and_headers() {
    let input = "| foo | bar |\n| --- | --- |\n| baz | qux |";
    let adf = markdown_to_adf(input);
    let table = &adf["content"][0];
    assert_eq!(table["type"], "table");

    let rows = table["content"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "expected 2 tableRows (header + body)");

    // Header row's cells should be tableHeader.
    let header_cells = rows[0]["content"].as_array().unwrap();
    assert_eq!(header_cells[0]["type"], "tableHeader");
    assert_eq!(header_cells[1]["type"], "tableHeader");

    // Body row's cells should be tableCell.
    let body_cells = rows[1]["content"].as_array().unwrap();
    assert_eq!(body_cells[0]["type"], "tableCell");
    assert_eq!(body_cells[1]["type"], "tableCell");

    // Cells wrap their content in a paragraph, per ADF convention.
    let first_header_text = &header_cells[0]["content"][0];
    assert_eq!(first_header_text["type"], "paragraph");
    assert_eq!(first_header_text["content"][0]["text"], "foo");
}
```

- [ ] **Step 3.2: Run the table test to verify it fails**

Run: `cargo test --lib adf::test_markdown_table_cells_and_headers`

Expected: FAIL — tables currently produce nothing (Tag::Table falls through to `NodeKind::Sink`).

- [ ] **Step 3.3: GREEN — implement table handling**

Add the table `NodeKind` variants. Append to the `enum NodeKind` definition:

```rust
    Table,
    TableRow,
    TableCell { is_header: bool },
```

In `src/adf.rs`, extend the `start()` match arms. Replace the `_ => self.push(NodeKind::Sink),` catch-all with these arms followed by the catch-all:

```rust
            Tag::Table(_) => self.push(NodeKind::Table),
            Tag::TableHead => {
                self.in_table_head = true;
                self.push(NodeKind::TableRow);
            }
            Tag::TableRow => self.push(NodeKind::TableRow),
            Tag::TableCell => self.push(NodeKind::TableCell {
                is_header: self.in_table_head,
            }),
            _ => self.push(NodeKind::Sink),
```

Extend the `end()` match arms. Add these arms immediately before the existing `NodeKind::InlineMark` / `NodeKind::Sink` arms:

```rust
            NodeKind::Table => Some(json!({ "type": "table", "content": children })),
            NodeKind::TableRow => {
                Some(json!({ "type": "tableRow", "content": children }))
            }
            NodeKind::TableCell { is_header } => {
                // ADF requires cells to wrap content in a block (paragraph).
                // pulldown-cmark emits Text events directly inside TableCell
                // without a Paragraph wrapper, so we wrap here.
                let cell_type = if is_header { "tableHeader" } else { "tableCell" };
                let wrapped_content = if children
                    .iter()
                    .all(|n| n["type"].as_str().is_some_and(|t| matches!(t, "paragraph" | "bulletList" | "orderedList" | "blockquote" | "codeBlock" | "heading")))
                {
                    children
                } else {
                    vec![json!({ "type": "paragraph", "content": children })]
                };
                Some(json!({ "type": cell_type, "content": wrapped_content }))
            }
```

Add the `in_table_head` reset to `end()` when closing `TableHead`. The cleanest place is to detect the TagEnd variant. Update the start of `end()`:

```rust
    fn end(&mut self, tag_end: TagEnd) {
        if matches!(tag_end, TagEnd::TableHead) {
            self.in_table_head = false;
        }
        let Some(partial) = self.stack.pop() else {
            return;
        };
```

- [ ] **Step 3.4: Run the table test to verify it passes**

Run: `cargo test --lib adf::test_markdown_table_cells_and_headers`

Expected: PASS.

- [ ] **Step 3.5: Re-run the full adf test module**

Run: `cargo test --lib adf::`

Expected: all tests except `test_markdown_to_adf_snapshot` pass.

- [ ] **Step 3.6: Commit**

```bash
git add src/adf.rs
git commit -m "feat: convert GFM tables to ADF table/tableRow/tableHeader/tableCell nodes (#197)"
```

---

### Task 4: Fallbacks, snapshot regeneration, integration-test verification, cleanup

**Scope:** Add tests for the explicit non-mappings (images skipped, task list syntax preserved as text) and verify end-to-end flow. Regenerate the complex snapshot. Run the full CI-equivalent check set.

**Files:**
- Modify: `src/adf.rs #[cfg(test)]` — add tests, update the snapshot input to exercise union of features
- Regenerated: `src/snapshots/jr__adf__tests__markdown_complex_to_adf.snap`

- [ ] **Step 4.1: RED — add fallback tests and refresh the snapshot input**

Append to `mod tests`:

```rust
#[test]
fn test_markdown_image_is_skipped() {
    let adf = markdown_to_adf("before ![alt](https://example.com/img.png) after");
    let para_text: String = adf["content"][0]["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["text"].as_str())
        .collect::<String>();
    // Image should be omitted — only the surrounding text remains.
    // "before " and " after" become "before  after" (double space where image was).
    assert!(para_text.contains("before"), "got: {para_text:?}");
    assert!(para_text.contains("after"), "got: {para_text:?}");
    assert!(!para_text.contains("img.png"), "image URL should not leak: {para_text:?}");
    // No image nodes emitted.
    let has_image = adf.to_string().contains("\"image\"") || adf.to_string().contains("media");
    assert!(!has_image, "no image/media nodes should be emitted: {adf}");
}

#[test]
fn test_markdown_task_list_syntax_preserved_as_text() {
    // ENABLE_TASKLISTS is not set, so `[x]` renders as literal text inside a bullet item.
    let adf = markdown_to_adf("- [x] done task\n- [ ] pending task");
    let list = &adf["content"][0];
    assert_eq!(list["type"], "bulletList");
    let items = list["content"].as_array().unwrap();
    let text = |item: &Value| -> String {
        item["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["text"].as_str())
            .collect()
    };
    assert!(text(&items[0]).contains("[x]"), "got: {}", text(&items[0]));
    assert!(text(&items[0]).contains("done task"));
    assert!(text(&items[1]).contains("[ ]"));
    assert!(text(&items[1]).contains("pending task"));
}
```

Next, update the existing `test_markdown_to_adf_snapshot` input to exercise the union of features. Replace the current test body:

```rust
    #[test]
    fn test_markdown_to_adf_snapshot() {
        let input = "## Root cause\n\nThe auth module had a **critical bug** in `validate_token`.\n\n- Missing null check\n- Wrong error type\n\n```rust\nfn validate() -> bool {\n    true\n}\n```";
        let adf = markdown_to_adf(input);
        insta::assert_json_snapshot!("markdown_complex_to_adf", adf);
    }
```

with:

```rust
    #[test]
    fn test_markdown_to_adf_snapshot() {
        let input = concat!(
            "# Header 1\n",
            "\n",
            "Paragraph with **bold**, *italic*, ~~strike~~, `inline code`, and a ",
            "[link](https://example.com \"title\").\n",
            "\n",
            "## Header 2\n",
            "\n",
            "- bullet one\n",
            "- bullet two\n",
            "  - nested bullet\n",
            "\n",
            "1. ordered\n",
            "2. items\n",
            "\n",
            "> blockquoted text\n",
            "\n",
            "| Col A | Col B |\n",
            "| ----- | ----- |\n",
            "| a1    | b1    |\n",
            "| a2    | b2    |\n",
            "\n",
            "---\n",
            "\n",
            "```rust\n",
            "fn validate() -> bool { true }\n",
            "```\n",
        );
        let adf = markdown_to_adf(input);
        insta::assert_json_snapshot!("markdown_complex_to_adf", adf);
    }
```

- [ ] **Step 4.2: Run all adf tests**

Run: `cargo test --lib adf::`

Expected: the two new fallback tests pass; the snapshot test fails with "snapshot assertion" until we accept the new snap file.

- [ ] **Step 4.3: Review and accept the new snapshot**

Run: `cargo install cargo-insta 2>/dev/null || true` (idempotent — installs if missing).

Run: `cargo insta review`

In the TUI, inspect the new snapshot of `markdown_complex_to_adf`. Verify by eye:
- `heading` nodes with `attrs.level = 1` and `attrs.level = 2`
- `paragraph` with `strong`, `em`, `strike`, `code` marks on the text nodes
- `link` mark with `attrs.href = "https://example.com"` and `attrs.title = "title"`
- `bulletList` with nested `bulletList` inside one `listItem`
- `orderedList` with no `attrs.order` (since start = 1)
- `blockquote` wrapping a `paragraph`
- `table` with `tableRow` containing `tableHeader` first row and `tableCell` subsequent rows
- `rule` block at the `---` location
- `codeBlock` with `attrs.language = "rust"`

Press `a` to accept. If the structure is wrong, abort the review, fix the builder, and re-run.

Alternative non-interactive path if the TUI is unavailable: `cargo insta accept` after verifying the `.snap.new` file manually.

- [ ] **Step 4.4: Run the integration tests to confirm end-to-end**

Run: `cargo test --test issue_commands`

Expected: all tests pass, including the three that call `jr::adf::markdown_to_adf` at `:589`, `:631`, `:671`. These drive wiremock `body_partial_json` matchers with the output of the new builder — if the client is POSTing what the new builder emits, they pass.

- [ ] **Step 4.5: Full CI-equivalent check set**

Run each, in order, fixing failures before proceeding:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all three clean.

If `cargo fmt --check` fails, run `cargo fmt --all` and recommit. If `clippy` flags `unused_imports` or `too_many_arguments`, refactor per project convention (no `#[allow]` without justification — see CLAUDE.md "No lint suppression without refactoring").

- [ ] **Step 4.6: Commit**

```bash
git add src/adf.rs src/snapshots/
git commit -m "$(cat <<'EOF'
test: regenerate markdown-to-ADF snapshot covering full feature matrix (#197)

Replaces the previous simple snapshot with input exercising headings,
all inline marks (strong/em/strike/code/link), nested lists, ordered
lists, blockquote, table (with header and body rows), horizontal rule,
and fenced code block with language. Adds tests for explicit fallbacks:
images are skipped, task list syntax passes through as literal text.

Closes #197.
EOF
)"
```

---

## Self-Review Checklist (for the engineer)

Before marking the plan complete, verify:

**Spec coverage:**
- [ ] `markdown_to_adf(&str) -> Value` signature unchanged — call sites in `workflow.rs:417`, `create.rs:98`, `create.rs:199` untouched
- [ ] All block nodes from spec "Feature Mapping" are implemented: heading, paragraph, blockquote, codeBlock, bulletList, orderedList, listItem, table, tableRow, tableCell, tableHeader
- [ ] All inline marks: strong, em, strike, code, link (with href + title)
- [ ] All leaf events: Text, Code, SoftBreak (space), HardBreak, Rule
- [ ] All fallbacks: Image (skip), Html/InlineHtml (literal text), FootnoteReference (ignore), task list syntax (literal brackets in text)
- [ ] Per-column table alignment silently dropped (no `align` attr emitted; no warning)
- [ ] `orderedList.attrs.order` omitted when start = 1, emitted when start ≠ 1

**Test coverage:**
- [ ] Unit tests exist for every row in the "Feature Mapping" table
- [ ] Snapshot test covers the union of features
- [ ] Integration tests in `tests/issue_commands.rs` pass without modification

**Code quality:**
- [ ] `parse_heading`, `parse_inline`, `flush_list` removed (no dead code)
- [ ] `text_to_adf` and `adf_to_text` untouched
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] No `#[allow]` added without refactor (CLAUDE.md policy)

**Dependencies:**
- [ ] `Cargo.toml` adds `pulldown-cmark = { version = "0.13", default-features = false }` only
- [ ] No other dependencies added

---

## Out of Scope (Do Not Add)

- `adf_to_text` reverse-direction support for tables/links/marks — separate issue if needed
- `--markdown` flag on `jr worklog add` — out of #197 scope
- Wiki markup (`--wiki`) conversion — separate feature, not this PR
- Markdown images via media-upload — requires attachment flow, out of scope
- `--debug` verbose flag, response body logging — unrelated
- Refactoring `text_to_adf` or `adf_to_text` — unchanged in this PR

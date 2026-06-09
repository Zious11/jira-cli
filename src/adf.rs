use pulldown_cmark::{
    BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
    TextMergeStream,
};
use serde_json::{Value, json};

pub fn text_to_adf(text: &str) -> Value {
    json!({
        "version": 1,
        "type": "doc",
        "content": [
            {
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": text }
                ]
            }
        ]
    })
}

pub fn markdown_to_adf(markdown: &str) -> Value {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        // `^x^` / `~x~` -> ADF subsup mark. ENABLE_SUBSCRIPT reassigns single-tilde
        // `~x~` from strikethrough to subscript; double-tilde `~~x~~` stays strike.
        | Options::ENABLE_SUPERSCRIPT
        | Options::ENABLE_SUBSCRIPT
        // Consume `## Title {#id}` attribute syntax instead of leaking `{#id}` into
        // the heading text. ADF headings have no id attr, so the value is dropped.
        | Options::ENABLE_HEADING_ATTRIBUTES
        // GFM alert blockquotes (`> [!NOTE|TIP|IMPORTANT|WARNING|CAUTION]`) ->
        // ADF `panel`. In pulldown-cmark 0.13 ENABLE_GFM gates ONLY the alert
        // blockquote tags (it does not double-enable tables/strike/footnotes set
        // above). Tagged alerts arrive as `Tag::BlockQuote(Some(kind))`; plain
        // quotes as `BlockQuote(None)`. ADF's `panel` content model forbids
        // nested `panel`, `table`, and `blockquote`, and `listItem` forbids
        // `panel`, so the mapping runs the same content-model normalization as
        // listItem (#470). See docs/specs/adf-panel-content-model.md (#483).
        | Options::ENABLE_GFM;
    let parser = TextMergeStream::new(Parser::new_ext(markdown, options));
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

/// Map a GFM alert kind to a portable ADF `panelType`.
///
/// Only the five always-safe panelTypes are used (`info`/`note`/`warning`/
/// `error`/`success`); the schema also permits `tip`/`custom`, but those are
/// editor-feature-gated and render inconsistently across Jira Cloud surfaces, so
/// they are avoided for REST portability. The match is exhaustive (no `_` arm)
/// so a future pulldown-cmark `BlockQuoteKind` variant is a compile error rather
/// than a silent default. See docs/specs/adf-panel-content-model.md (#483).
fn panel_type_for(kind: BlockQuoteKind) -> &'static str {
    match kind {
        BlockQuoteKind::Note => "info",
        BlockQuoteKind::Tip => "success",
        BlockQuoteKind::Important => "note",
        BlockQuoteKind::Warning => "warning",
        BlockQuoteKind::Caution => "error",
    }
}

/// Inverse of [`panel_type_for`]: map an ADF `panelType` back to the GFM alert
/// label for the `> [!KIND]` reverse render. Unknown/unmapped types (e.g.
/// `tip`/`custom`, or a panel from another source) return `None`, so
/// `adf_to_text` falls back to a plain blockquote with no marker.
fn gfm_label_for_panel_type(panel_type: &str) -> Option<&'static str> {
    match panel_type {
        "info" => Some("NOTE"),
        "success" => Some("TIP"),
        "note" => Some("IMPORTANT"),
        "warning" => Some("WARNING"),
        "error" => Some("CAUTION"),
        _ => None,
    }
}

struct AdfBuilder {
    root: Vec<Value>,
    stack: Vec<PartialNode>,
    active_marks: Vec<Value>,
    in_table_head: bool,
    // ADF has no native footnote node. Definition bodies are collected here and
    // flushed at `finish()` into an appended footnotes section (a `rule` divider
    // followed by one labelled paragraph per definition), so authored content is
    // preserved rather than silently dropped (issue #472).
    footnote_defs: Vec<Value>,
    // Labels already collected, so a duplicate `[^1]: ...` definition line keeps
    // only the first occurrence instead of emitting two identically-labelled
    // paragraphs.
    footnote_labels_seen: std::collections::HashSet<String>,
}

struct PartialNode {
    kind: NodeKind,
    children: Vec<Value>,
}

enum NodeKind {
    Paragraph,
    Heading(u8),
    BlockQuote,
    // GFM alert (`> [!NOTE]` etc.) -> ADF `panel`. `panel_type` is the mapped,
    // portable panelType string (info/note/warning/error/success).
    Panel { panel_type: &'static str },
    CodeBlock { language: Option<String> },
    BulletList,
    OrderedList { start: u64 },
    ListItem,
    Sink,
    // Container for inline marks. Has no ADF node; just manages the active_marks stack
    // so End events pop cleanly.
    InlineMark,
    Table,
    TableRow,
    TableCell { is_header: bool },
    // Captures a footnote definition's block content. On End the content is
    // moved into `footnote_defs` (label-prefixed) instead of the parent flow.
    FootnoteDefinition { label: String },
}

impl AdfBuilder {
    fn new() -> Self {
        Self {
            root: Vec::new(),
            stack: Vec::new(),
            active_marks: Vec::new(),
            in_table_head: false,
            footnote_defs: Vec::new(),
            footnote_labels_seen: std::collections::HashSet::new(),
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
            // ADF has no footnote-reference node. Render the reference inline as a
            // plain `[label]` marker so the caret form `[^label]` never survives as
            // literal text (#472). The marker is a *structural* reference, not
            // content, so it deliberately does NOT inherit the active inline marks
            // (a `[^1]` inside `**bold**` must not produce a bold marker); this also
            // keeps it consistent with the unmarked definition-side marker.
            // Note: pulldown-cmark only emits this event for references that have a
            // matching definition; an undefined `[^x]` stays literal text upstream.
            Event::FootnoteReference(label) => self.push_footnote_marker(label.as_ref()),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.push(NodeKind::Paragraph),
            Tag::Heading { level, .. } => self.push(NodeKind::Heading(heading_level_to_u8(level))),
            Tag::BlockQuote(None) => self.push(NodeKind::BlockQuote),
            Tag::BlockQuote(Some(kind)) => self.push(NodeKind::Panel {
                panel_type: panel_type_for(kind),
            }),
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
            Tag::Strong => self.push_mark(json!({ "type": "strong" })),
            Tag::Emphasis => self.push_mark(json!({ "type": "em" })),
            Tag::Strikethrough => self.push_mark(json!({ "type": "strike" })),
            Tag::Superscript => {
                self.push_mark(json!({ "type": "subsup", "attrs": { "type": "sup" } }))
            }
            Tag::Subscript => {
                self.push_mark(json!({ "type": "subsup", "attrs": { "type": "sub" } }))
            }
            Tag::Link {
                dest_url, title, ..
            } => {
                let mut attrs = serde_json::Map::new();
                attrs.insert("href".to_string(), json!(dest_url.as_ref()));
                if !title.is_empty() {
                    attrs.insert("title".to_string(), json!(title.as_ref()));
                }
                self.push_mark(json!({ "type": "link", "attrs": attrs }));
            }
            Tag::Table(_) => self.push(NodeKind::Table),
            Tag::TableHead => {
                self.in_table_head = true;
                self.push(NodeKind::TableRow);
            }
            Tag::TableRow => self.push(NodeKind::TableRow),
            Tag::TableCell => self.push(NodeKind::TableCell {
                is_header: self.in_table_head,
            }),
            // Explicit for documentation; the final catch-all also handles this,
            // but images are visibly named as intentionally suppressed per the
            // spec's Feature Mapping (ADF `media` nodes require pre-upload).
            Tag::Image { .. } => self.push(NodeKind::Sink),
            Tag::FootnoteDefinition(label) => {
                self.push(NodeKind::FootnoteDefinition {
                    label: label.into_string(),
                });
            }
            _ => self.push(NodeKind::Sink),
        }
    }

    fn end(&mut self, tag_end: TagEnd) {
        if matches!(tag_end, TagEnd::TableHead) {
            self.in_table_head = false;
        }
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
            NodeKind::Panel { panel_type } => {
                // ADF `panel.content` forbids nested `panel`, `table`, and
                // `blockquote`; normalize_panel_content transforms each into the
                // permitted set BEFORE wrapping loose inline runs (mirrors the
                // listItem path #470). See docs/specs/adf-panel-content-model.md.
                let normalized = normalize_panel_content(children);
                // A body-less alert (`> [!NOTE]` with no content) emits empty
                // panel content so `is_empty_block_container` prunes the whole
                // panel below — an empty `panel` is invalid ADF (Jira 400).
                // Unlike `listItem` (which must stay non-empty, so it keeps a
                // placeholder paragraph), a top-level panel can be dropped
                // entirely; do NOT route empties through `wrap_inlines_as_blocks`,
                // which would inject a placeholder paragraph and defeat pruning.
                let content = if normalized.is_empty() {
                    Vec::new()
                } else {
                    wrap_inlines_as_blocks(
                        normalized,
                        &[
                            "paragraph",
                            "heading",
                            "bulletList",
                            "orderedList",
                            "codeBlock",
                            "rule",
                        ],
                    )
                };
                Some(json!({
                    "type": "panel",
                    "attrs": { "panelType": panel_type },
                    "content": content,
                }))
            }
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
            NodeKind::ListItem => {
                // ADF `listItem.content` permits ONLY paragraph, bulletList,
                // orderedList, codeBlock, and mediaSingle. pulldown-cmark
                // legitimately emits blockquote, heading, table, and rule inside
                // Item for markdown like `- > quoted` or `- # heading`; those are
                // NOT valid listItem children (issue #470,
                // docs/specs/adf-listitem-content-model.md). `normalize_list_item_content`
                // transforms each disallowed block into the permitted set BEFORE
                // wrapping loose inline runs — shrinking the allowlist alone would
                // instead wrap them into a paragraph, producing the equally-invalid
                // `paragraph > blockquote` shape.
                let normalized = normalize_list_item_content(children);
                let wrapped = wrap_inlines_as_blocks(
                    normalized,
                    &[
                        "paragraph",
                        "bulletList",
                        "orderedList",
                        "codeBlock",
                        "mediaSingle",
                    ],
                );
                Some(json!({ "type": "listItem", "content": wrapped }))
            }
            NodeKind::Table => Some(json!({ "type": "table", "content": children })),
            NodeKind::TableRow => Some(json!({ "type": "tableRow", "content": children })),
            NodeKind::TableCell { is_header } => {
                // ADF requires cells to wrap content in a block. pulldown-cmark
                // emits Text events directly inside TableCell without a Paragraph
                // wrapper, so we wrap here.
                let cell_type = if is_header {
                    "tableHeader"
                } else {
                    "tableCell"
                };
                let wrapped = wrap_inlines_as_blocks(
                    children,
                    &[
                        "paragraph",
                        "bulletList",
                        "orderedList",
                        "blockquote",
                        "codeBlock",
                        "heading",
                    ],
                );
                Some(json!({ "type": cell_type, "content": wrapped }))
            }
            NodeKind::InlineMark => {
                self.pop_mark();
                // InlineMark has no ADF node of its own. Splice children (already
                // tagged with marks at `push_text` time, plus any nested text or
                // hardBreak nodes from inner mark spans) into the parent.
                for child in children {
                    self.append_child(child);
                }
                None
            }
            NodeKind::FootnoteDefinition { label } => {
                // Keep only the first definition per label; a duplicate
                // `[^1]: ...` line is an authoring error and would otherwise emit
                // two identically-labelled paragraphs.
                if self.footnote_labels_seen.insert(label.clone()) {
                    // Move the definition's blocks into the deferred footnotes
                    // section, prefixing the body with a `[label] ` marker. The
                    // definition is almost always a single paragraph; if its first
                    // block is not a paragraph (e.g. a list body), prepend a
                    // standalone label paragraph instead of mutating it.
                    let mut blocks = children;
                    let marker = json!({ "type": "text", "text": format!("[{label}] ") });
                    match blocks.first_mut() {
                        Some(first) if first["type"] == "paragraph" => {
                            match first["content"].as_array_mut() {
                                Some(content) => content.insert(0, marker),
                                None => first["content"] = json!([marker]),
                            }
                        }
                        Some(_) => {
                            blocks.insert(0, json!({ "type": "paragraph", "content": [marker] }));
                        }
                        None => {
                            // Empty definition body — still emit the bare label.
                            blocks.push(json!({ "type": "paragraph", "content": [marker] }));
                        }
                    }
                    self.footnote_defs.extend(blocks);
                }
                None
            }
            NodeKind::Sink => None,
        };
        if let Some(node) = node {
            // Drop block containers left with empty `content` (invalid ADF that
            // Jira rejects with HTTP 400). Two ways this arises:
            //   * pulldown-cmark hoists a footnote definition out of an enclosing
            //     block, leaving an empty shell (`> [^1]: x` -> empty blockquote);
            //   * a contentless heading from a bare `#` line.
            // End events fire inner-first, so if a future transform emptied a
            // nested container it would be pruned before its parent finalizes.
            // (In practice today the only reachable empties are a direct
            // blockquote and a bare heading; the list path keeps a valid empty
            // placeholder paragraph and is never pruned — see is_empty_block_container.)
            if !is_empty_block_container(&node) {
                self.append_child(node);
            }
        }
    }

    fn push(&mut self, kind: NodeKind) {
        self.stack.push(PartialNode {
            kind,
            children: Vec::new(),
        });
    }

    fn push_mark(&mut self, mark: Value) {
        self.active_marks.push(mark);
        self.push(NodeKind::InlineMark);
    }

    fn pop_mark(&mut self) {
        self.active_marks.pop();
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

    /// Append a footnote reference marker `[label]` as a plain, unmarked text
    /// node. Unlike `push_text` it never applies `active_marks` (a footnote
    /// reference is structural, not styled content), but it still honors a Sink
    /// (e.g. inside image alt text) so the marker is dropped there like any other
    /// inline content.
    fn push_footnote_marker(&mut self, label: &str) {
        if let Some(top) = self.stack.last() {
            if matches!(top.kind, NodeKind::Sink) {
                return;
            }
        }
        self.append_child(json!({ "type": "text", "text": format!("[{label}]") }));
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(top) = self.stack.last() {
            if matches!(top.kind, NodeKind::Sink) {
                return;
            }
        }
        let mut node = json!({ "type": "text", "text": text });
        if !self.active_marks.is_empty() {
            node["marks"] = json!(dedup_marks_by_type(&self.active_marks));
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
        let mut marks = self.active_marks.clone();
        marks.push(json!({ "type": "code" }));
        self.append_child(json!({
            "type": "text",
            "text": text,
            "marks": dedup_marks_by_type(&marks),
        }));
    }

    fn finish(mut self) -> Vec<Value> {
        // Flush collected footnote definitions into an appended section,
        // separated from the body by a single `rule` divider. Only emitted when
        // at least one definition exists (a bare reference adds no section).
        if !self.footnote_defs.is_empty() {
            // Add the divider only when there is body content to divide from, and
            // never when the body already ends in a rule — otherwise a doc that is
            // footnote-only gets a leading rule, and a doc ending in `---` gets two
            // adjacent rules.
            let ends_with_rule = self
                .root
                .last()
                .and_then(|n| n.get("type"))
                .and_then(Value::as_str)
                == Some("rule");
            if !self.root.is_empty() && !ends_with_rule {
                self.root.push(json!({ "type": "rule" }));
            }
            self.root.append(&mut self.footnote_defs);
        }
        self.root
    }
}

/// Keep only the first mark of each `type`. ADF (ProseMirror) treats a text
/// node's marks as a set keyed by type, so two marks of the same type are
/// invalid. This arises with nested same-type spans — e.g. `^a ~b~ c^` puts both
/// a `subsup` sup and a `subsup` sub on the inner text; we keep the outer one.
fn dedup_marks_by_type(marks: &[Value]) -> Vec<Value> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out: Vec<Value> = Vec::new();
    for mark in marks {
        let ty = mark.get("type").and_then(Value::as_str).unwrap_or_default();
        if !seen.contains(&ty) {
            seen.push(ty);
            out.push(mark.clone());
        }
    }
    out
}

/// True for block container nodes left with an empty `content` array. ADF
/// rejects an empty `content` on these with HTTP 400, so they must be pruned
/// rather than emitted.
///
/// Excluded by design:
/// - `paragraph` / `codeBlock`: ADF permits them to be empty, and
///   `wrap_inlines_as_blocks` relies on an empty paragraph as a valid
///   placeholder inside otherwise-empty table cells and list items.
/// - `tableCell` / `tableHeader`: dropping a cell would break column alignment;
///   they always receive that placeholder paragraph instead, so they are never
///   actually empty.
///
/// `heading` IS included: unlike paragraph, an empty heading (e.g. a bare `#`
/// line) carries no content and the ADF schema treats heading content as
/// required.
fn is_empty_block_container(node: &Value) -> bool {
    const REQUIRES_CONTENT: [&str; 8] = [
        "blockquote",
        "panel",
        "heading",
        "listItem",
        "bulletList",
        "orderedList",
        "table",
        "tableRow",
    ];
    let is_required = node
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|t| REQUIRES_CONTENT.contains(&t));
    let is_empty = node
        .get("content")
        .and_then(Value::as_array)
        .is_some_and(|c| c.is_empty());
    is_required && is_empty
}

/// Group a mixed list of inline and block nodes into pure block-level output.
///
/// ADF requires `listItem`, `tableCell`, and `tableHeader` content to be
/// block-level (paragraph, lists, codeBlock, etc.). pulldown-cmark emits
/// inline events (Text, hardBreak) directly inside tight list items and
/// table cells without a paragraph wrapper, and nested block structures can
/// appear alongside inline content (e.g., a tight list item with a nested
/// bullet list: `[Text("outer"), bulletList]`).
///
/// Each run of consecutive inline nodes is wrapped in its own paragraph;
/// block nodes (matching `block_types`) pass through as siblings. An empty
/// input produces a single empty paragraph so the output always satisfies
/// ADF's "at least one block" requirement.
fn wrap_inlines_as_blocks(children: Vec<Value>, block_types: &[&str]) -> Vec<Value> {
    if children.is_empty() {
        return vec![json!({ "type": "paragraph", "content": [] })];
    }
    let is_block = |n: &Value| n["type"].as_str().is_some_and(|t| block_types.contains(&t));
    let mut result: Vec<Value> = Vec::new();
    let mut inline_run: Vec<Value> = Vec::new();
    for child in children {
        if is_block(&child) {
            if !inline_run.is_empty() {
                result.push(json!({
                    "type": "paragraph",
                    "content": std::mem::take(&mut inline_run),
                }));
            }
            result.push(child);
        } else {
            inline_run.push(child);
        }
    }
    if !inline_run.is_empty() {
        result.push(json!({ "type": "paragraph", "content": inline_run }));
    }
    result
}

/// Normalize the children of a `listItem` to the ADF-permitted content model.
///
/// ADF `listItem.content` permits only `paragraph`, `bulletList`, `orderedList`,
/// `codeBlock`, and `mediaSingle`. pulldown-cmark legitimately emits
/// `blockquote`, `heading`, `table`, and `rule` inside list items; this pass
/// transforms each into the permitted set (issue #470,
/// `docs/specs/adf-listitem-content-model.md`):
///
/// - `blockquote` → unwrapped: its child blocks are spliced in and recursively
///   normalized (handles e.g. `- > # heading`).
/// - `heading` → converted to `paragraph`, preserving inline content and dropping
///   the `level` attr.
/// - `table` → flattened to one `paragraph` per row, joining cells in `| a | b |`
///   form while preserving each cell's inline content and ADF marks
///   (`flatten_table_to_paragraphs`). The grid structure is not preserved.
/// - `rule` → dropped (empty leaf, meaningless inside a list item).
///
/// Permitted blocks and loose inline nodes (`text`, `hardBreak`) pass through
/// untouched; the caller's `wrap_inlines_as_blocks` then groups the inline runs
/// into paragraphs.
fn normalize_list_item_content(children: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for child in children {
        match child["type"].as_str() {
            Some("blockquote") | Some("panel") => {
                // `listItem.content` permits neither `blockquote` nor `panel`.
                // Unwrap and recursively normalize the inner blocks (the panel's
                // panelType is discarded — ADF cannot represent a panel inside a
                // listItem). #483.
                let inner = child
                    .get("content")
                    .and_then(|c| c.as_array())
                    .cloned()
                    .unwrap_or_default();
                out.extend(normalize_list_item_content(inner));
            }
            Some("heading") => {
                let content = child.get("content").cloned().unwrap_or_else(|| json!([]));
                out.push(json!({ "type": "paragraph", "content": content }));
            }
            Some("table") => out.extend(flatten_table_to_paragraphs(&child)),
            Some("rule") => { /* dropped — no content, invalid inside listItem */ }
            _ => out.push(child),
        }
    }
    out
}

/// Normalize the children of a `panel` to the ADF-permitted content model.
///
/// ADF `panel.content` permits `paragraph`/`heading` (both no-marks),
/// `bulletList`, `orderedList`, `codeBlock`, `rule`, `taskList`, and several
/// media/card/decision nodes — but **forbids** nested `panel`, `table`, and
/// `blockquote`. pulldown-cmark can emit all three inside an alert blockquote
/// (nested alerts, an alert wrapping a table, an alert wrapping a plain quote),
/// so this pass transforms each into the permitted set BEFORE the caller's
/// `wrap_inlines_as_blocks` groups loose inline runs (issue #483,
/// `docs/specs/adf-panel-content-model.md`):
///
/// - `panel` → unwrapped: child blocks spliced in and recursively normalized
///   (the inner panelType is discarded; `panel > panel` is invalid).
/// - `blockquote` → unwrapped: child blocks spliced in and recursively normalized.
/// - `table` → flattened to one `paragraph` per row via
///   `flatten_table_to_paragraphs` (marks preserved; grid structure lost).
/// - `heading` → kept, but stripped of any node-level `marks` to satisfy
///   `heading (no marks)`.
/// - `paragraph` → kept, with any node-level `marks` stripped (`paragraph
///   (no marks)`); defense-in-depth — block nodes don't carry marks today.
///
/// Permitted blocks and loose inline nodes pass through untouched.
fn normalize_panel_content(children: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for mut child in children {
        match child["type"].as_str() {
            Some("panel") | Some("blockquote") => {
                let inner = child
                    .get("content")
                    .and_then(|c| c.as_array())
                    .cloned()
                    .unwrap_or_default();
                out.extend(normalize_panel_content(inner));
            }
            Some("table") => out.extend(flatten_table_to_paragraphs(&child)),
            Some("heading") | Some("paragraph") => {
                // panel.content requires `paragraph (no marks)` / `heading (no
                // marks)`: strip any node-level marks array.
                if let Some(obj) = child.as_object_mut() {
                    obj.remove("marks");
                }
                out.push(child);
            }
            _ => out.push(child),
        }
    }
    out
}

/// Flatten an ADF `table` node into one `paragraph` per row, for embedding inside
/// a `listItem` (which the ADF content model forbids from containing a table).
///
/// Cells are joined in `| a | b |` form. Each cell's inline content is spliced in
/// as real ADF nodes, so marks (`strong`, `em`, `link`, …) are **preserved** — we
/// do NOT route through `adf_to_text`, which would render marks as literal
/// markdown syntax (`**bold**`, `[label](url)`) that Jira would then display
/// verbatim. The table's grid structure is necessarily lost (there is no ADF node
/// nesting a table inside a listItem); only the per-row pipe layout and cell
/// content survive.
fn flatten_table_to_paragraphs(table: &Value) -> Vec<Value> {
    let mut paragraphs: Vec<Value> = Vec::new();
    let Some(rows) = table.get("content").and_then(|c| c.as_array()) else {
        return paragraphs;
    };
    for row in rows {
        if row.get("type").and_then(Value::as_str) != Some("tableRow") {
            continue;
        }
        let Some(cells) = row.get("content").and_then(|c| c.as_array()) else {
            continue;
        };
        let mut content: Vec<Value> = Vec::new();
        for cell in cells {
            let sep = if content.is_empty() { "| " } else { " | " };
            content.push(json!({ "type": "text", "text": sep }));
            // For markdown tables a cell's content is always `paragraph` blocks,
            // so we splice their inline children in, preserving marks. The ADF
            // `tableCell` schema also permits richer blocks (bulletList, codeBlock,
            // …); a non-paragraph block must NOT be spliced as inline — that would
            // emit invalid `paragraph > <block>`. Render any such block to a
            // newline-free plain-text node instead. This branch is unreachable from
            // `markdown_to_adf` today (pulldown-cmark emits only inline events in
            // GFM cells) but keeps the function total and ADF-valid.
            if let Some(blocks) = cell.get("content").and_then(|c| c.as_array()) {
                for block in blocks {
                    if block.get("type").and_then(Value::as_str) == Some("paragraph") {
                        if let Some(inlines) = block.get("content").and_then(|c| c.as_array()) {
                            content.extend(inlines.iter().cloned());
                        }
                    } else {
                        let doc = json!({ "type": "doc", "version": 1, "content": [block] });
                        let text = adf_to_text(&doc).trim_end().replace(['\n', '\r'], " ");
                        if !text.is_empty() {
                            content.push(json!({ "type": "text", "text": text }));
                        }
                    }
                }
            }
        }
        // An all-empty row collapses to bare separators (`| | |`) — valid ADF, and
        // a faithful (if sparse) rendering of an empty source row; emitted as-is.
        if content.is_empty() {
            continue; // a row with no cells contributes nothing
        }
        content.push(json!({ "type": "text", "text": " |" }));
        paragraphs.push(json!({ "type": "paragraph", "content": content }));
    }
    paragraphs
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

pub fn adf_to_text(adf: &Value) -> String {
    let mut r = AdfRenderer::new();
    r.render_doc(adf);
    r.finish()
}

struct AdfRenderer {
    output: String,
    list_stack: Vec<ListFrame>,
}

enum ListFrame {
    Bullet,
    Ordered { next_index: u64 },
}

impl AdfRenderer {
    fn new() -> Self {
        Self {
            output: String::new(),
            list_stack: Vec::new(),
        }
    }

    fn render_doc(&mut self, adf: &Value) {
        if let Some(content) = adf.get("content").and_then(|c| c.as_array()) {
            for node in content {
                self.render_node(node);
            }
        }
    }

    fn render_node(&mut self, node: &Value) {
        let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match node_type {
            "text" => {
                let text = node.get("text").and_then(|t| t.as_str()).unwrap_or("");
                let marks = node.get("marks").and_then(|m| m.as_array());
                self.output.push_str(&apply_marks(text, marks));
            }
            "paragraph" => {
                self.render_children(node);
                self.output.push('\n');
            }
            "heading" => {
                let level = node
                    .get("attrs")
                    .and_then(|a| a.get("level"))
                    .and_then(|l| l.as_u64())
                    .unwrap_or(1) as usize;
                for _ in 0..level {
                    self.output.push('#');
                }
                self.output.push(' ');
                self.render_children(node);
                self.output.push('\n');
            }
            "bulletList" => {
                self.list_stack.push(ListFrame::Bullet);
                self.render_children(node);
                self.list_stack.pop();
            }
            "orderedList" => {
                // Treat missing, 0, or negative `attrs.order` as "start at 1" —
                // matches Jira's own renderer, which ignores invalid HTML
                // `<ol start>` values the same way.
                let start = node
                    .get("attrs")
                    .and_then(|a| a.get("order"))
                    .and_then(|o| o.as_u64())
                    .filter(|&n| n >= 1)
                    .unwrap_or(1);
                self.list_stack
                    .push(ListFrame::Ordered { next_index: start });
                self.render_children(node);
                self.list_stack.pop();
            }
            "listItem" => {
                let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));
                self.output.push_str(&indent);
                let prefix = match self.list_stack.last_mut() {
                    Some(ListFrame::Ordered { next_index }) => {
                        let n = *next_index;
                        *next_index += 1;
                        format!("{n}. ")
                    }
                    _ => "- ".to_string(),
                };
                self.output.push_str(&prefix);
                self.render_children(node);
            }
            "rule" => {
                self.output.push_str("---\n");
            }
            "hardBreak" => {
                self.output.push('\n');
            }
            "codeBlock" => {
                let lang = node
                    .get("attrs")
                    .and_then(|a| a.get("language"))
                    .and_then(|l| l.as_str())
                    .unwrap_or("");
                self.output.push_str("```");
                self.output.push_str(lang);
                self.output.push('\n');
                self.render_children(node);
                self.output.push_str("\n```\n");
            }
            "blockquote" => {
                let start = self.output.len();
                self.render_children(node);

                // Prefix every line in the just-rendered segment with "> ".
                // Nesting accumulates ("> > inner") because each level's prefix
                // pass runs on unwind, re-prefixing the output its children's
                // inner passes already produced — so a fixed "> " is correct at
                // every level; no depth counter is needed.
                let rendered = self.output.split_off(start);
                // Collect lines, trim trailing empties (they'd produce a dangling
                // "> " at the very end). Middle empty lines are preserved and
                // prefixed with "> " so the blockquote context isn't broken by
                // the blank-line-between-paragraphs pattern (e.g., a multi-line
                // code block inside a blockquote).
                let mut lines: Vec<&str> = rendered.split('\n').collect();
                while lines.last() == Some(&"") {
                    lines.pop();
                }
                let prefix = "> ";
                for (i, line) in lines.iter().enumerate() {
                    if i > 0 {
                        self.output.push('\n');
                    }
                    if line.is_empty() {
                        // Blank line inside the quote: emit just ">" (no trailing
                        // space) — matches the conventional `>\n` markdown form
                        // and preserves block-quote continuity.
                        self.output.push('>');
                    } else {
                        self.output.push_str(prefix);
                        self.output.push_str(line);
                    }
                }
                if !lines.is_empty() {
                    self.output.push('\n');
                }
            }
            "panel" => {
                // Render a panel back to a GFM alert `> [!KIND]\n> body`. A
                // known panelType emits the alert marker as the first quoted
                // line; an unmapped type (tip/custom/foreign) falls back to a
                // plain blockquote with no marker. Line-prefixing mirrors the
                // `blockquote` arm. #483.
                let label = node
                    .get("attrs")
                    .and_then(|a| a.get("panelType"))
                    .and_then(|p| p.as_str())
                    .and_then(gfm_label_for_panel_type);
                let start = self.output.len();
                self.render_children(node);
                let rendered = self.output.split_off(start);
                let mut lines: Vec<&str> = rendered.split('\n').collect();
                while lines.last() == Some(&"") {
                    lines.pop();
                }
                let marker = label.map(|l| format!("[!{l}]"));
                // First emit the marker line (if any), then the body lines, all
                // prefixed with "> ".
                let mut first = true;
                if let Some(marker) = &marker {
                    self.output.push_str("> ");
                    self.output.push_str(marker);
                    first = false;
                }
                for line in &lines {
                    if !first {
                        self.output.push('\n');
                    }
                    first = false;
                    if line.is_empty() {
                        self.output.push('>');
                    } else {
                        self.output.push_str("> ");
                        self.output.push_str(line);
                    }
                }
                if marker.is_some() || !lines.is_empty() {
                    self.output.push('\n');
                }
            }
            "table" => {
                self.render_children(node);
                self.output.push('\n');
            }
            "tableRow" => {
                let cells: &[Value] = node
                    .get("content")
                    .and_then(|c| c.as_array())
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                let cell_count = cells.len();
                let mut has_header = false;
                self.output.push_str("| ");
                for (i, cell) in cells.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(" | ");
                    }
                    if cell.get("type").and_then(|t| t.as_str()) == Some("tableHeader") {
                        has_header = true;
                    }
                    self.render_cell_inline(cell);
                }
                self.output.push_str(" |\n");
                if has_header {
                    self.output.push_str("| ");
                    for i in 0..cell_count {
                        if i > 0 {
                            self.output.push_str(" | ");
                        }
                        self.output.push_str("---");
                    }
                    self.output.push_str(" |\n");
                }
            }
            "tableCell" | "tableHeader" => {
                // Should not be reached directly — tableRow invokes render_cell_inline
                // on its cells. Fall through to flat rendering defensively.
                self.render_cell_inline(node);
            }
            _ => {
                // NFR-O-I: ADF inline nodes mention/emoji/inlineCard/media fall through to `_`
                // here. Canonical render hints (per developer.atlassian.com/cloud/jira/platform/
                // apis/document/nodes/, retrieved 2026-05):
                //   mention    -> attrs.text (already includes leading "@"); fallback "@?"
                //   emoji      -> attrs.text (unicode glyph) or attrs.shortName (e.g. ":smile:")
                //   inlineCard -> attrs.url (title not guaranteed; either url OR data, not both)
                //   media      -> "[media]" placeholder; fileName requires Media Services call
                // Not implemented in v0.5; tracked under issue #202.
                //
                // Unknown node: recurse into content if present, otherwise
                // drop silently. Per the #202 spec, this avoids debug strings
                // like "[unsupported: type]" reaching user output while still
                // salvaging the text content of container nodes like panel or
                // nestedExpand.
                if node.get("content").is_some() {
                    self.render_children(node);
                }
            }
        }
    }

    fn render_children(&mut self, node: &Value) {
        if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
            for child in content {
                self.render_node(child);
            }
        }
    }

    /// Render a tableCell/tableHeader's children in "flat" mode: a paragraph's
    /// inline content is emitted without its trailing newline (which would
    /// break the "| cell | cell |" row structure). Other block types inside
    /// a cell (rare but legal per the schema) fall back to normal rendering.
    fn render_cell_inline(&mut self, cell: &Value) {
        let Some(content) = cell.get("content").and_then(|c| c.as_array()) else {
            return;
        };
        for (i, child) in content.iter().enumerate() {
            if i > 0 {
                self.output.push(' ');
            }
            let child_type = child.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match child_type {
                "paragraph" => {
                    // Paragraph inside a cell: render its inline children
                    // directly. `hardBreak` becomes a space; text nodes are
                    // sanitized for cell-unsafe characters (pipes, newlines).
                    if let Some(cc) = child.get("content").and_then(|c| c.as_array()) {
                        for inline in cc {
                            self.render_inline_in_cell(inline);
                        }
                    }
                }
                "hardBreak" => self.output.push(' '),
                _ => self.render_inline_in_cell(child),
            }
        }
    }

    /// Render an inline node in cell mode: `hardBreak` becomes a space, and
    /// text nodes are sanitized so pipes don't introduce false column
    /// separators and embedded newlines don't break the row structure. Marks
    /// are applied to the sanitized text so the escape survives mark wrapping.
    fn render_inline_in_cell(&mut self, node: &Value) {
        let t = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match t {
            "hardBreak" => self.output.push(' '),
            "text" => {
                let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let sanitized = sanitize_table_cell_text(text);
                let marks = node.get("marks").and_then(|m| m.as_array());
                self.output.push_str(&apply_marks(&sanitized, marks));
            }
            _ => self.render_node(node),
        }
    }

    fn finish(self) -> String {
        self.output.trim_end().to_string()
    }
}

/// Escape pipe characters and collapse embedded newlines in text that will
/// be rendered inside a markdown table cell. Without this, a literal `|` in a
/// cell's text would be read as an extra column separator, and an embedded
/// `\n` would break the pipe row into multiple lines.
fn sanitize_table_cell_text(text: &str) -> String {
    text.replace(['\r', '\n'], " ").replace('|', r"\|")
}

/// Wrap an inline-code span using a delimiter long enough to contain any
/// backtick runs in `text` (CommonMark rule: delimiter must have more
/// backticks than the longest run inside). If the content begins or ends
/// with a backtick, a single space is padded on each side so the delimiter
/// can't "glue" to the content.
fn wrap_code_span(text: &str) -> String {
    let mut longest_run = 0usize;
    let mut current = 0usize;
    for ch in text.chars() {
        if ch == '`' {
            current += 1;
            longest_run = longest_run.max(current);
        } else {
            current = 0;
        }
    }
    let delim = "`".repeat(longest_run + 1);
    let needs_pad = text.starts_with('`') || text.ends_with('`');
    if needs_pad {
        format!("{delim} {text} {delim}")
    } else {
        format!("{delim}{text}{delim}")
    }
}

/// Wrap `text` with markdown-style syntax for each mark. `code` is always
/// applied innermost regardless of its position in the `marks` array, because
/// the content of an inline-code span is literal and cannot itself carry
/// other markdown syntax. The remaining marks then wrap the code span in
/// array order.
///
/// This matters because the write-path `AdfBuilder::push_code` appends
/// `{type: "code"}` to the active marks *after* any other marks, so on
/// roundtrip we see `marks: [strong, code]` for `**`\x`**`; applying marks
/// strictly in order would produce `` `**x**` `` (code outermost),
/// losing the bold semantics.
///
/// Unknown mark types pass through without added syntax.
fn apply_marks(text: &str, marks: Option<&Vec<Value>>) -> String {
    let Some(marks) = marks else {
        return text.to_string();
    };
    let has_code = marks
        .iter()
        .any(|m| m.get("type").and_then(|t| t.as_str()) == Some("code"));
    let mut result = if has_code {
        wrap_code_span(text)
    } else {
        text.to_string()
    };
    for mark in marks {
        let mark_type = mark.get("type").and_then(|t| t.as_str()).unwrap_or("");
        result = match mark_type {
            "code" => result, // handled above as innermost
            "em" => format!("*{result}*"),
            "strong" => format!("**{result}**"),
            "strike" => format!("~~{result}~~"),
            "subsup" => {
                // Reverse of the markdown mapping: `^x^` (sup) / `~x~` (sub).
                let sub = mark
                    .get("attrs")
                    .and_then(|a| a.get("type"))
                    .and_then(|t| t.as_str())
                    == Some("sub");
                if sub {
                    format!("~{result}~")
                } else {
                    format!("^{result}^")
                }
            }
            "link" => {
                let href = mark
                    .get("attrs")
                    .and_then(|a| a.get("href"))
                    .and_then(|h| h.as_str())
                    .unwrap_or("");
                format!("[{result}]({href})")
            }
            _ => result,
        };
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recursively check whether any node in an ADF value (or its `content`
    /// subtrees) has the given `type`. Structural — unlike a serialized-string
    /// substring search, it cannot false-positive on a text node whose literal
    /// text happens to contain the type name.
    fn contains_node_type(value: &Value, node_type: &str) -> bool {
        if value.get("type").and_then(Value::as_str) == Some(node_type) {
            return true;
        }
        match value {
            Value::Array(items) => items.iter().any(|v| contains_node_type(v, node_type)),
            Value::Object(map) => map.values().any(|v| contains_node_type(v, node_type)),
            _ => false,
        }
    }

    #[test]
    fn test_text_to_adf() {
        let adf = text_to_adf("Hello world");
        assert_eq!(adf["type"], "doc");
        assert_eq!(adf["content"][0]["type"], "paragraph");
        assert_eq!(adf["content"][0]["content"][0]["text"], "Hello world");
    }

    #[test]
    fn test_adf_to_text_paragraph() {
        let adf = text_to_adf("Hello world");
        assert_eq!(adf_to_text(&adf), "Hello world");
    }

    #[test]
    fn test_markdown_heading() {
        let adf = markdown_to_adf("## Root cause");
        assert_eq!(adf["content"][0]["type"], "heading");
        assert_eq!(adf["content"][0]["attrs"]["level"], 2);
    }

    #[test]
    fn test_markdown_list() {
        let adf = markdown_to_adf("- item one\n- item two");
        assert_eq!(adf["content"][0]["type"], "bulletList");
        let items = adf["content"][0]["content"].as_array().unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_markdown_code_block() {
        let adf = markdown_to_adf("```\nlet x = 1;\n```");
        assert_eq!(adf["content"][0]["type"], "codeBlock");
    }

    #[test]
    fn test_adf_roundtrip_heading() {
        let adf = markdown_to_adf("## Title\nSome text");
        let text = adf_to_text(&adf);
        assert!(text.contains("## Title"));
        assert!(text.contains("Some text"));
    }

    #[test]
    fn test_render_unknown_leaf_drops_silently() {
        let adf = json!({
            "type": "doc",
            "content": [{ "type": "mediaGroup" }]
        });
        assert_eq!(adf_to_text(&adf), "");
    }

    #[test]
    fn test_render_unknown_container_recurses() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "panel",
                "attrs": {"panelType": "info"},
                "content": [
                    {"type": "paragraph", "content": [{"type": "text", "text": "inside panel"}]}
                ]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("inside panel"), "got: {text:?}");
        assert!(!text.contains("[unsupported"), "no debug string: {text:?}");
    }

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

    #[test]
    fn test_markdown_ordered_list_sets_order_when_start_is_not_one() {
        let adf = markdown_to_adf("5. first\n6. second");
        assert_eq!(adf["content"][0]["type"], "orderedList");
        assert_eq!(adf["content"][0]["attrs"]["order"], 5);
        assert_eq!(adf["content"][0]["content"][0]["type"], "listItem");
    }

    #[test]
    fn test_markdown_ordered_list_omits_order_when_start_is_one() {
        let adf = markdown_to_adf("1. alpha\n2. beta");
        assert_eq!(adf["content"][0]["type"], "orderedList");
        assert!(adf["content"][0]["attrs"].is_null());
    }

    #[test]
    fn test_markdown_hard_break() {
        let adf = markdown_to_adf("line one  \nline two");
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
        assert_eq!(block["content"][0]["text"], "fn x() {}\n");
    }

    #[test]
    fn test_markdown_empty_input() {
        let adf = markdown_to_adf("");
        assert_eq!(adf["type"], "doc");
        assert_eq!(adf["content"], json!([]));
    }

    #[test]
    fn test_markdown_inline_code_mark_and_composition() {
        // Plain inline code: emits text with a `code` mark.
        let adf = markdown_to_adf("see `foo` here");
        let code_node = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "foo")
            .expect("expected a text node for 'foo'");
        assert_eq!(code_node["marks"][0]["type"], "code");

        // Inline code inside bold: composes both marks on the same text node.
        let adf = markdown_to_adf("**bold `code` bold**");
        let code_node = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "code")
            .expect("expected a text node for 'code'");
        let mark_types: Vec<&str> = code_node["marks"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m["type"].as_str())
            .collect();
        assert!(
            mark_types.contains(&"code") && mark_types.contains(&"strong"),
            "expected code + strong on the inline-code inside bold, got: {mark_types:?}"
        );
    }

    #[test]
    fn test_markdown_blockquote_inside_list_item_is_unwrapped_to_paragraph() {
        // `- > quoted` → pulldown-cmark emits blockquote inside Item. The ADF
        // `listItem` content model does NOT permit `blockquote` (only paragraph,
        // bulletList, orderedList, codeBlock, mediaSingle — see
        // docs/specs/adf-listitem-content-model.md, issue #470). We unwrap the
        // blockquote and splice its child paragraph(s) directly into the listItem.
        let adf = markdown_to_adf("- > quoted text");
        let item = &adf["content"][0]["content"][0];
        assert_eq!(item["type"], "listItem");
        let first_child = &item["content"][0];
        assert_eq!(first_child["type"], "paragraph");
        assert_eq!(first_child["content"][0]["text"], "quoted text");
        // No blockquote node anywhere in the document.
        assert!(
            !contains_node_type(&adf, "blockquote"),
            "blockquote must not appear inside listItem: {adf}"
        );
    }

    #[test]
    fn test_markdown_heading_inside_list_item_becomes_paragraph() {
        // ADF `listItem` does not permit `heading`. Convert to a paragraph,
        // preserving inline content (issue #470).
        let adf = markdown_to_adf("- # Heading text");
        let item = &adf["content"][0]["content"][0];
        assert_eq!(item["type"], "listItem");
        let first_child = &item["content"][0];
        assert_eq!(first_child["type"], "paragraph");
        assert_eq!(first_child["content"][0]["text"], "Heading text");
        assert!(
            first_child.get("attrs").is_none(),
            "downconverted paragraph must not carry the heading's level attr: {first_child}"
        );
        assert!(
            !contains_node_type(&adf, "heading"),
            "heading must not appear inside listItem: {adf}"
        );
    }

    #[test]
    fn test_markdown_heading_inside_list_item_preserves_inline_marks() {
        // `- ### deep **bold** head` → paragraph preserving the strong mark on
        // "bold" (inline content kept verbatim, only the heading wrapper dropped).
        let adf = markdown_to_adf("- ### deep **bold** head");
        let para = &adf["content"][0]["content"][0]["content"][0];
        assert_eq!(para["type"], "paragraph");
        let content = para["content"].as_array().unwrap();
        let bold = content
            .iter()
            .find(|n| n["text"] == "bold")
            .expect("expected a 'bold' text node");
        assert_eq!(bold["marks"][0]["type"], "strong");
    }

    #[test]
    fn test_markdown_blockquote_with_heading_inside_list_item_normalizes_recursively() {
        // `- > # quoted heading` → unwrap blockquote, then the inner heading is
        // itself downconverted to a paragraph (recursive normalization).
        let adf = markdown_to_adf("- > # quoted heading");
        let item = &adf["content"][0]["content"][0];
        assert_eq!(item["type"], "listItem");
        let first_child = &item["content"][0];
        assert_eq!(first_child["type"], "paragraph");
        assert_eq!(first_child["content"][0]["text"], "quoted heading");
        assert!(
            !contains_node_type(&adf, "blockquote") && !contains_node_type(&adf, "heading"),
            "neither blockquote nor heading may appear inside listItem: {adf}"
        );
    }

    #[test]
    fn test_markdown_rule_inside_list_item_is_dropped() {
        // ADF `listItem` does not permit `rule`. A horizontal rule inside a list
        // item carries no content and is dropped; the paragraph is kept.
        let adf = markdown_to_adf("- item\n\n  ---");
        let item = &adf["content"][0]["content"][0];
        assert_eq!(item["type"], "listItem");
        assert_eq!(item["content"][0]["type"], "paragraph");
        assert_eq!(item["content"][0]["content"][0]["text"], "item");
        assert!(
            !contains_node_type(&adf, "rule"),
            "rule must not appear inside listItem: {adf}"
        );
    }

    #[test]
    fn test_markdown_table_inside_list_item_flattens_to_paragraphs() {
        // ADF `listItem` does not permit `table`. The table is flattened to one
        // paragraph per row (`| a | b |` form); no `table`/`tableRow` node
        // survives. The header cell is bold to verify inline marks are preserved
        // as real ADF marks (NOT serialized to literal `**` markdown).
        let adf = markdown_to_adf("- intro\n\n  | **a** | b |\n  | - | - |\n  | 1 | 2 |");
        let item = &adf["content"][0]["content"][0];
        assert_eq!(item["type"], "listItem");
        let children = item["content"].as_array().unwrap();
        // Every child must be a permitted listItem block type.
        for child in children {
            let t = child["type"].as_str().unwrap();
            assert!(
                [
                    "paragraph",
                    "bulletList",
                    "orderedList",
                    "codeBlock",
                    "mediaSingle"
                ]
                .contains(&t),
                "unexpected listItem child type {t:?}: {item}"
            );
        }
        assert!(
            !contains_node_type(&adf, "table") && !contains_node_type(&adf, "tableRow"),
            "no table node may appear inside listItem: {adf}"
        );

        // Every text node across the flattened paragraphs is newline-free, and no
        // literal markdown mark syntax leaked (the bold cell must NOT render as
        // `**a**` — that would mean we routed through adf_to_text).
        let all_texts: Vec<String> = children
            .iter()
            .filter_map(|p| p["content"].as_array())
            .flatten()
            .filter_map(|n| n["text"].as_str().map(str::to_string))
            .collect();
        assert!(
            all_texts.iter().all(|t| !t.contains('\n')),
            "text nodes must be newline-free: {all_texts:?}"
        );
        let joined = all_texts.concat();
        assert!(
            !joined.contains("**"),
            "bold cell must be a real strong mark, not literal `**`: {joined:?}"
        );
        // The pipe layout and both rows' cell text survive.
        assert!(
            joined.contains("| a "),
            "header cell 'a' missing: {joined:?}"
        );
        assert!(
            joined.contains("| 1 ") && joined.contains("| 2 "),
            "data cells missing: {joined:?}"
        );

        // The bold header cell keeps its ADF `strong` mark.
        let bold_a = children
            .iter()
            .filter_map(|p| p["content"].as_array())
            .flatten()
            .find(|n| n["text"] == "a")
            .expect("expected an 'a' text node from the header cell");
        assert_eq!(
            bold_a["marks"][0]["type"], "strong",
            "bold cell text must retain its strong mark: {bold_a}"
        );
    }

    #[test]
    fn test_flatten_table_non_paragraph_cell_block_renders_as_plain_text() {
        // Defensive branch: the ADF tableCell schema permits non-paragraph blocks
        // (here a codeBlock with an embedded newline) even though markdown_to_adf
        // never produces them. flatten_table_to_paragraphs must NOT splice such a
        // block as inline (that would be invalid `paragraph > codeBlock`); it
        // renders to a single newline-free text node. Tests the private fn directly
        // since the branch is unreachable through the parser.
        let table = json!({
            "type": "table",
            "content": [{
                "type": "tableRow",
                "content": [{
                    "type": "tableCell",
                    "content": [{
                        "type": "codeBlock",
                        "content": [{ "type": "text", "text": "line1\nline2" }]
                    }]
                }]
            }]
        });
        let paras = flatten_table_to_paragraphs(&table);
        assert_eq!(paras.len(), 1, "one row → one paragraph: {paras:?}");
        let content = paras[0]["content"].as_array().unwrap();
        // No block node smuggled into the paragraph; every child is a text node.
        for node in content {
            assert_eq!(node["type"], "text", "paragraph child must be text: {node}");
            assert!(
                !node["text"].as_str().unwrap().contains(['\n', '\r']),
                "flattened text must be newline-free: {node}"
            );
        }
        let joined: String = content.iter().filter_map(|n| n["text"].as_str()).collect();
        assert!(
            joined.contains("line1 line2"),
            "code text must survive: {joined:?}"
        );
    }

    #[test]
    fn test_markdown_rule_only_list_item_yields_empty_paragraph() {
        // A list item whose only content is a rule: the rule is dropped, leaving
        // the item empty, so `wrap_inlines_as_blocks` supplies a single empty
        // paragraph to satisfy ADF's "at least one block" rule (BC-7.2.006 edge
        // case). No `rule` node survives.
        let adf = markdown_to_adf("-   \n\n    ---");
        let item = &adf["content"][0]["content"][0];
        assert_eq!(item["type"], "listItem");
        let children = item["content"].as_array().unwrap();
        assert_eq!(children.len(), 1, "expected exactly one child: {item}");
        assert_eq!(children[0]["type"], "paragraph");
        assert_eq!(
            children[0]["content"].as_array().map(Vec::len),
            Some(0),
            "the fallback paragraph must be empty: {item}"
        );
        assert!(
            !contains_node_type(&adf, "rule"),
            "rule must not survive inside listItem: {adf}"
        );
    }

    #[test]
    fn test_markdown_codeblock_inside_list_item_passes_through() {
        // `codeBlock` is a permitted listItem child and must pass through the
        // normalization untouched (BC-7.2.006).
        let adf = markdown_to_adf("- ```\n  let x = 1;\n  ```");
        let item = &adf["content"][0]["content"][0];
        assert_eq!(item["type"], "listItem");
        let code = &item["content"][0];
        assert_eq!(code["type"], "codeBlock");
        assert!(
            code["content"][0]["text"]
                .as_str()
                .is_some_and(|t| t.contains("let x = 1;")),
            "codeBlock content must be preserved verbatim: {item}"
        );
    }

    #[test]
    fn test_markdown_ordered_list_inside_list_item_passes_through() {
        // A nested `orderedList` is a permitted listItem child and passes through;
        // its own items are normalized at their own listItem boundary.
        let adf = markdown_to_adf("- outer\n  1. a\n  2. b");
        let item = &adf["content"][0]["content"][0];
        assert_eq!(item["type"], "listItem");
        let nested = item["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["type"] == "orderedList")
            .expect("expected a nested orderedList child");
        let inner_items = nested["content"].as_array().unwrap();
        assert_eq!(
            inner_items.len(),
            2,
            "nested ordered list must keep both items"
        );
        assert_eq!(inner_items[0]["type"], "listItem");
        assert_eq!(inner_items[0]["content"][0]["type"], "paragraph");
    }

    #[test]
    fn test_markdown_inline_html_becomes_literal_text() {
        // ENABLE_HTML is not set in Options; pulldown-cmark still emits Html/InlineHtml
        // events which we forward to push_text so the literal source is preserved.
        let adf = markdown_to_adf("before <span>x</span> after");
        let para_text: String = adf["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["text"].as_str())
            .collect();
        assert!(
            para_text.contains("<span>") && para_text.contains("</span>"),
            "HTML should pass through as literal text, got: {para_text:?}"
        );
    }

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
        let adf = markdown_to_adf("**bold _italic_ bold**");
        let content = adf["content"][0]["content"].as_array().unwrap();
        // Every text node in this paragraph should carry `strong` (outer).
        assert!(
            content.iter().all(|n| n["marks"]
                .as_array()
                .is_some_and(|m| m.iter().any(|mk| mk["type"] == "strong"))),
            "every text node should carry strong, got: {content:?}"
        );
        // The node containing "italic" should also carry `em`.
        let italic_node = content
            .iter()
            .find(|n| n["text"] == "italic")
            .expect("expected a text node for 'italic'");
        let italic_marks: Vec<&str> = italic_node["marks"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m["type"].as_str())
            .collect();
        assert!(
            italic_marks.contains(&"strong") && italic_marks.contains(&"em"),
            "expected strong + em on the italic node, got: {italic_marks:?}"
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

    #[test]
    fn test_adf_to_text_snapshot() {
        let adf = json!({
            "type": "doc",
            "version": 1,
            "content": [
                {"type": "heading", "attrs": {"level": 2}, "content": [
                    {"type": "text", "text": "Summary"}
                ]},
                {"type": "paragraph", "content": [
                    {"type": "text", "text": "A "},
                    {"type": "text", "text": "bold", "marks": [{"type": "strong"}]},
                    {"type": "text", "text": " word, an "},
                    {"type": "text", "text": "italic", "marks": [{"type": "em"}]},
                    {"type": "text", "text": " word, a "},
                    {"type": "text", "text": "link", "marks": [
                        {"type": "link", "attrs": {"href": "https://example.com"}}
                    ]},
                    {"type": "text", "text": ", and "},
                    {"type": "text", "text": "code", "marks": [{"type": "code"}]},
                    {"type": "text", "text": "."}
                ]},
                {"type": "bulletList", "content": [
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "first bullet"}]}]},
                    {"type": "listItem", "content": [
                        {"type": "paragraph", "content": [{"type": "text", "text": "second bullet"}]},
                        {"type": "orderedList", "attrs": {"order": 3}, "content": [
                            {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "three"}]}]},
                            {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "four"}]}]}
                        ]}
                    ]}
                ]},
                {"type": "blockquote", "content": [
                    {"type": "paragraph", "content": [{"type": "text", "text": "quoted thought"}]}
                ]},
                {"type": "rule"},
                {"type": "codeBlock", "attrs": {"language": "rust"}, "content": [
                    {"type": "text", "text": "fn main() { println!(\"hi\"); }"}
                ]},
                {"type": "table", "content": [
                    {"type": "tableRow", "content": [
                        {"type": "tableHeader", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "k"}]}]},
                        {"type": "tableHeader", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "v"}]}]}
                    ]},
                    {"type": "tableRow", "content": [
                        {"type": "tableCell", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "a"}]}]},
                        {"type": "tableCell", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "1"}]}]}
                    ]}
                ]}
            ]
        });
        let text = adf_to_text(&adf);
        insta::assert_snapshot!("adf_to_text_complex", text);
    }

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
        assert!(para_text.contains("before"), "got: {para_text:?}");
        assert!(para_text.contains("after"), "got: {para_text:?}");
        assert!(
            !para_text.contains("img.png"),
            "image URL should not leak: {para_text:?}"
        );
        // No image nodes emitted.
        let has_image = adf.to_string().contains("\"image\"") || adf.to_string().contains("media");
        assert!(!has_image, "no image/media nodes should be emitted: {adf}");
    }

    #[test]
    fn test_markdown_task_list_syntax_preserved_as_text() {
        // ENABLE_TASKLISTS is not set, so `[x]` renders as literal text inside a bullet item.
        // pulldown-cmark emits text directly inside the listItem (no paragraph wrapper
        // for tight lists), so we collect text nodes from the item's direct children.
        let adf = markdown_to_adf("- [x] done task\n- [ ] pending task");
        let list = &adf["content"][0];
        assert_eq!(list["type"], "bulletList");
        let items = list["content"].as_array().unwrap();
        let text = |item: &Value| -> String {
            item["content"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|n| {
                    // Tight list: text nodes sit directly inside listItem.
                    // Loose list: text nodes are wrapped in a paragraph.
                    if let Some(t) = n["text"].as_str() {
                        Some(t.to_string())
                    } else {
                        n["content"].as_array().map(|children| {
                            children
                                .iter()
                                .filter_map(|c| c["text"].as_str())
                                .collect::<String>()
                        })
                    }
                })
                .collect()
        };
        assert!(text(&items[0]).contains("[x]"), "got: {}", text(&items[0]));
        assert!(text(&items[0]).contains("done task"));
        assert!(text(&items[1]).contains("[ ]"));
        assert!(text(&items[1]).contains("pending task"));
    }

    // --- Footnotes (issue #472) -------------------------------------------
    // Before the fix, ENABLE_FOOTNOTES was off, so `[^1]` survived as literal
    // text and `[^1]: ...` became a stray paragraph — visibly broken output.
    // The fix parses footnotes and renders references as a plain `[label]`
    // marker, collecting definitions into an appended footnotes section
    // (a `rule` divider + one labelled paragraph per definition). This
    // preserves authored content rather than silently dropping it.

    /// Collect all `text` node strings from a paragraph node, in order.
    fn para_text_of(node: &Value) -> String {
        node["content"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|n| n["text"].as_str())
                    .collect::<String>()
            })
            .unwrap_or_default()
    }

    #[test]
    fn test_markdown_footnote_reference_renders_marker_not_literal_caret() {
        let adf = markdown_to_adf("See note.[^1]\n\n[^1]: The note body.");
        let s = adf.to_string();
        assert!(
            !s.contains("[^1]"),
            "literal footnote caret must not survive: {s}"
        );
        let first_para = &adf["content"][0];
        assert_eq!(first_para["type"], "paragraph");
        let para_text = para_text_of(first_para);
        assert!(para_text.contains("See note."), "got: {para_text:?}");
        assert!(
            para_text.contains("[1]"),
            "reference marker missing: {para_text:?}"
        );
    }

    #[test]
    fn test_markdown_footnote_definition_appended_after_rule_with_label() {
        let adf = markdown_to_adf("Body.[^1]\n\n[^1]: Definition text.");
        let content = adf["content"].as_array().unwrap();
        let rule_idx = content
            .iter()
            .position(|n| n["type"] == "rule")
            .expect("a rule divider must precede the footnotes section");
        let def = &content[rule_idx + 1];
        assert_eq!(def["type"], "paragraph");
        let def_text = para_text_of(def);
        assert!(
            def_text.starts_with("[1] "),
            "definition must be prefixed with its [label] marker: {def_text:?}"
        );
        assert!(
            def_text.contains("Definition text."),
            "definition body missing: {def_text:?}"
        );
    }

    #[test]
    fn test_markdown_footnote_definition_not_stray_broken_paragraph() {
        let adf = markdown_to_adf("Body.[^1]\n\n[^1]: Definition text.");
        let s = adf.to_string();
        assert!(
            !s.contains("[^1]:"),
            "stray `[^1]:` definition remnant must not survive: {s}"
        );
    }

    #[test]
    fn test_markdown_multiple_footnotes_share_single_divider() {
        let adf = markdown_to_adf("First.[^a] Second.[^b]\n\n[^a]: Alpha.\n[^b]: Beta.");
        let s = adf.to_string();
        assert!(!s.contains("[^"), "no literal carets: {s}");
        let content = adf["content"].as_array().unwrap();
        let rules = content.iter().filter(|n| n["type"] == "rule").count();
        assert_eq!(rules, 1, "exactly one footnotes divider expected: {s}");
        assert!(s.contains("Alpha."), "alpha def body missing: {s}");
        assert!(s.contains("Beta."), "beta def body missing: {s}");
        let first_para_text = para_text_of(&content[0]);
        assert!(
            first_para_text.contains("[a]"),
            "ref [a] missing: {first_para_text}"
        );
        assert!(
            first_para_text.contains("[b]"),
            "ref [b] missing: {first_para_text}"
        );
    }

    #[test]
    fn test_markdown_footnote_undefined_reference_stays_literal_no_section() {
        // pulldown-cmark only emits a `FootnoteReference` event for references
        // that have a matching definition. An *undefined* `[^x]` is never
        // recognized as a footnote — it remains the literal text the user typed
        // and produces no footnotes section. This is documented pulldown
        // behavior, not the #472 malformed-output bug (which required a
        // definition to manifest). Pinning it guards against a silent change.
        let adf = markdown_to_adf("Dangling.[^x]");
        let content = adf["content"].as_array().unwrap();
        assert!(
            content.iter().all(|n| n["type"] != "rule"),
            "no rule divider without any definition"
        );
        let para_text = para_text_of(&content[0]);
        assert_eq!(para_text, "Dangling.[^x]", "undefined ref left verbatim");
    }

    // Adversarial-review-driven edge cases (issue #472 hybrid review): the
    // dual (Claude + Gemini) adversary passes found that pulldown-cmark hoists
    // footnote definitions out of enclosing blocks (leaving empty containers
    // that ADF rejects with HTTP 400), and several rendering edge cases.

    /// Recursively assert no *required-content* container carries an empty
    /// `content` array — Jira rejects the whole payload with 400. `paragraph`,
    /// `heading`, and `codeBlock` MAY be empty in ADF and are excluded.
    fn assert_no_invalid_empty_container(adf: &Value) {
        const REQUIRES_CONTENT: &[&str] = &[
            "blockquote",
            "panel",
            "heading",
            "listItem",
            "bulletList",
            "orderedList",
            "table",
            "tableRow",
            "tableCell",
            "tableHeader",
        ];
        fn walk(n: &Value) {
            if let Some(t) = n["type"].as_str() {
                if REQUIRES_CONTENT.contains(&t) {
                    let empty = n["content"].as_array().is_some_and(|c| c.is_empty());
                    assert!(!empty, "invalid empty `{t}` content (Jira 400): {n}");
                }
            }
            if let Some(arr) = n["content"].as_array() {
                arr.iter().for_each(walk);
            }
        }
        walk(adf);
    }

    #[test]
    fn test_markdown_footnote_definition_in_blockquote_no_empty_container() {
        // `> [^1]: x` hoists the definition out, leaving an empty blockquote.
        let adf = markdown_to_adf("Body.[^1]\n\n> [^1]: quoted note");
        assert_no_invalid_empty_container(&adf);
        assert!(
            adf.to_string().contains("quoted note"),
            "def body preserved"
        );
    }

    #[test]
    fn test_markdown_footnote_definition_in_list_no_empty_container() {
        // `- [^1]: x` hoists the definition out, leaving the list item holding
        // only a placeholder *empty paragraph*. That paragraph is valid ADF (so
        // it is NOT pruned), which keeps the listItem/bulletList non-empty and
        // valid. The point of this test: the hoist must never yield an
        // empty-`content` listItem/bulletList (a 400), and the body survives.
        let adf = markdown_to_adf("Body.[^1]\n\n- [^1]: listed note");
        assert_no_invalid_empty_container(&adf);
        assert!(
            adf.to_string().contains("listed note"),
            "def body preserved"
        );
    }

    #[test]
    fn test_markdown_footnote_reference_marker_does_not_inherit_marks() {
        // A reference inside `**bold**` must render a PLAIN `[1]` marker — the
        // marker is structural, not styled content.
        let adf = markdown_to_adf("**bold[^1]**\n\n[^1]: note");
        let first_para = &adf["content"][0];
        let marker = first_para["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "[1]")
            .expect("reference marker [1] present");
        assert!(
            marker.get("marks").is_none(),
            "footnote marker must not inherit surrounding marks: {marker}"
        );
        // The neighbouring real content still carries its mark.
        let bold = first_para["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["text"] == "bold")
            .unwrap();
        assert_eq!(
            bold["marks"][0]["type"], "strong",
            "bold text keeps its mark"
        );
    }

    #[test]
    fn test_markdown_footnote_duplicate_definition_kept_once() {
        let adf = markdown_to_adf("Body.[^1]\n\n[^1]: first\n[^1]: second");
        let content = adf["content"].as_array().unwrap();
        let def_paras = content
            .iter()
            .filter(|n| n["type"] == "paragraph" && para_text_of(n).starts_with("[1] "))
            .count();
        assert_eq!(
            def_paras, 1,
            "duplicate definition must collapse to one: {adf}"
        );
        let s = adf.to_string();
        assert!(s.contains("first"), "first definition kept: {s}");
        assert!(
            !s.contains("second"),
            "duplicate (second) definition dropped: {s}"
        );
    }

    #[test]
    fn test_markdown_footnote_no_double_rule_when_body_ends_with_rule() {
        let adf = markdown_to_adf("Body.[^1]\n\n---\n\n[^1]: note");
        let rules = adf["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["type"] == "rule")
            .count();
        assert_eq!(rules, 1, "must not emit two adjacent rule dividers: {adf}");
    }

    #[test]
    fn test_markdown_footnote_only_document_has_no_leading_rule() {
        // A definition with no body content and no reference still preserves the
        // text but must not produce a leading rule divider.
        let adf = markdown_to_adf("[^1]: orphan definition");
        let content = adf["content"].as_array().unwrap();
        assert_ne!(content[0]["type"], "rule", "no leading rule: {adf}");
        assert!(
            adf.to_string().contains("orphan definition"),
            "orphan definition body preserved: {adf}"
        );
    }

    #[test]
    fn test_markdown_footnote_definition_body_list_preserved() {
        // A definition whose body is a list exercises the non-paragraph branch:
        // a standalone `[1]` label paragraph is prepended, then the list blocks.
        let adf = markdown_to_adf("Body.[^1]\n\n[^1]:\n    - alpha\n    - beta");
        assert_no_invalid_empty_container(&adf);
        let content = adf["content"].as_array().unwrap();
        let has_label = content
            .iter()
            .any(|n| n["type"] == "paragraph" && para_text_of(n).starts_with("[1]"));
        assert!(has_label, "label paragraph present: {adf}");
        let s = adf.to_string();
        assert!(
            s.contains("alpha") && s.contains("beta"),
            "list body preserved: {s}"
        );
    }

    #[test]
    fn test_is_empty_block_container_membership() {
        // Pin the REQUIRES_CONTENT set directly, so the prune coverage does not
        // rely on a particular markdown input reaching each container type.
        for ty in [
            "blockquote",
            "panel",
            "heading",
            "listItem",
            "bulletList",
            "orderedList",
            "table",
            "tableRow",
        ] {
            assert!(
                is_empty_block_container(&json!({ "type": ty, "content": [] })),
                "{ty} with empty content must be pruned"
            );
            assert!(
                !is_empty_block_container(
                    &json!({ "type": ty, "content": [{ "type": "text", "text": "x" }] })
                ),
                "{ty} with content must be kept"
            );
        }
        // Excluded types: empty is valid ADF (or pruning would break structure).
        for ty in ["paragraph", "codeBlock", "tableCell", "tableHeader"] {
            assert!(
                !is_empty_block_container(&json!({ "type": ty, "content": [] })),
                "{ty} with empty content must NOT be pruned"
            );
        }
    }

    #[test]
    fn test_markdown_bare_heading_pruned_no_empty_container() {
        // A contentless `#` line yields an empty heading; it must be pruned, not
        // emitted as an invalid empty-content node.
        let adf = markdown_to_adf("#\n\nbody text");
        assert_no_invalid_empty_container(&adf);
        let content = adf["content"].as_array().unwrap();
        assert!(
            content.iter().all(|n| n["type"] != "heading"),
            "empty heading must be pruned: {adf}"
        );
        assert!(
            adf.to_string().contains("body text"),
            "body preserved: {adf}"
        );
    }

    // --- Minor markdown constructs (issue #474) ---------------------------
    // subsup (^x^/~x~) and heading attribute stripping (## Title {#id}).
    // GFM alert blockquotes (> [!NOTE]) -> panel are descoped to #483 and stay
    // plain blockquotes here.

    /// First text node of the first paragraph, with its marks.
    fn first_para_first_text(adf: &Value) -> Value {
        adf["content"][0]["content"][0].clone()
    }

    #[test]
    fn test_markdown_superscript_to_subsup_sup() {
        let adf = markdown_to_adf("a ^sup^ b");
        let nodes = adf["content"][0]["content"].as_array().unwrap();
        let sup = nodes
            .iter()
            .find(|n| n["text"] == "sup")
            .expect("superscript text node present");
        assert_eq!(sup["marks"][0]["type"], "subsup");
        assert_eq!(sup["marks"][0]["attrs"]["type"], "sup");
    }

    #[test]
    fn test_markdown_subscript_to_subsup_sub() {
        let adf = markdown_to_adf("a ~sub~ b");
        let nodes = adf["content"][0]["content"].as_array().unwrap();
        let sub = nodes
            .iter()
            .find(|n| n["text"] == "sub")
            .expect("subscript text node present");
        assert_eq!(sub["marks"][0]["type"], "subsup");
        assert_eq!(sub["marks"][0]["attrs"]["type"], "sub");
    }

    #[test]
    fn test_markdown_intraword_superscript_stays_literal() {
        // pulldown-cmark does not open a superscript when the `^` is tight against
        // a preceding word char, so the common `mc^2^` exponent form is NOT
        // converted — it stays literal. Documented limitation (#474); use a
        // boundary like `mc ^2^` to get a subsup mark.
        let adf = markdown_to_adf("mc^2^");
        let t = adf["content"][0]["content"][0].clone();
        assert_eq!(t["text"], "mc^2^", "intraword caret stays literal: {t}");
        assert!(t["marks"].is_null(), "no subsup mark applied: {t}");
    }

    #[test]
    fn test_markdown_double_tilde_still_strikethrough_not_subscript() {
        // Enabling ENABLE_SUBSCRIPT must not steal `~~x~~` from strikethrough.
        let adf = markdown_to_adf("~~struck~~");
        let t = first_para_first_text(&adf);
        assert_eq!(t["text"], "struck");
        assert_eq!(t["marks"][0]["type"], "strike", "got: {t}");
    }

    #[test]
    fn test_render_subsup_mark_reverse_path() {
        // adf_to_text must render a subsup mark back to `^x^` / `~x~` so a fetched
        // Jira issue containing subsup is not silently flattened, and the
        // markdown -> ADF -> text round-trip is lossless.
        let sup = json!({
            "type": "doc",
            "content": [{ "type": "paragraph", "content": [
                { "type": "text", "text": "x", "marks": [{ "type": "subsup", "attrs": { "type": "sup" } }] }
            ]}]
        });
        assert_eq!(adf_to_text(&sup).trim(), "^x^");
        let sub = json!({
            "type": "doc",
            "content": [{ "type": "paragraph", "content": [
                { "type": "text", "text": "y", "marks": [{ "type": "subsup", "attrs": { "type": "sub" } }] }
            ]}]
        });
        assert_eq!(adf_to_text(&sub).trim(), "~y~");
    }

    #[test]
    fn test_subsup_markdown_to_text_roundtrip() {
        let text = adf_to_text(&markdown_to_adf("a ^sup^ and ~sub~ b"));
        assert!(text.contains("^sup^"), "sup round-trip: {text:?}");
        assert!(text.contains("~sub~"), "sub round-trip: {text:?}");
    }

    #[test]
    fn test_subsup_composes_with_strong() {
        // subsup must compose with another mark on the same span: `**^x^**` keeps
        // both `strong` and `subsup`, and a markdown -> ADF -> text -> ADF
        // round-trip preserves both (reverse path renders `^**x**^`).
        let marks_of = |adf: &Value| -> Vec<String> {
            adf["content"][0]["content"][0]["marks"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|m| m["type"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let adf = markdown_to_adf("**^x^**");
        let marks = marks_of(&adf);
        assert!(
            marks.contains(&"strong".to_string()),
            "strong present: {adf}"
        );
        assert!(
            marks.contains(&"subsup".to_string()),
            "subsup present: {adf}"
        );
        // Reverse + re-parse: both marks survive the text representation.
        let reparsed = markdown_to_adf(&adf_to_text(&adf));
        let rt = marks_of(&reparsed);
        assert!(
            rt.contains(&"strong".to_string()) && rt.contains(&"subsup".to_string()),
            "round-trip preserves both marks: {reparsed}"
        );
    }

    #[test]
    fn test_markdown_strike_sub_sup_coexist() {
        let adf = markdown_to_adf("~~s~~ ~b~ ^p^");
        let nodes = adf["content"][0]["content"].as_array().unwrap();
        let mark_of = |text: &str| -> String {
            nodes
                .iter()
                .find(|n| n["text"] == text)
                .and_then(|n| n["marks"][0]["type"].as_str().map(str::to_string))
                .unwrap_or_else(|| panic!("text {text:?} missing/markless in {nodes:?}"))
        };
        assert_eq!(mark_of("s"), "strike");
        assert_eq!(mark_of("b"), "subsup");
        assert_eq!(mark_of("p"), "subsup");
    }

    #[test]
    fn test_markdown_nested_sub_in_sup_dedupes_subsup_mark() {
        // `^ ~x~ ^` nests a subscript inside a superscript; the inner text would
        // otherwise carry two `subsup` marks, which ADF rejects (duplicate mark
        // type). dedup_marks_by_type keeps the first (outer sup).
        let adf = markdown_to_adf("a ^b ~c~ d^ e");
        let nodes = adf["content"][0]["content"].as_array().unwrap();
        let inner = nodes
            .iter()
            .find(|n| n["text"] == "c")
            .expect("inner text node present");
        let marks = inner["marks"].as_array().expect("inner has marks");
        let subsup_count = marks.iter().filter(|m| m["type"] == "subsup").count();
        assert_eq!(subsup_count, 1, "at most one subsup mark per node: {inner}");
    }

    #[test]
    fn test_markdown_nested_sub_in_sup_keeps_outer_sup() {
        // F-2 / BC-7.2.007 EC-3: dedup_marks_by_type is first-wins.
        // For input `a ^b ~c~ d^ e`, node `c` sits inside BOTH the outer `^…^`
        // (sup) and the inner `~…~` (sub), so it would otherwise carry two
        // `subsup` marks. The outer `^` opens `subsup { type: "sup" }` first;
        // the inner `~` opens `subsup { type: "sub" }` second. After dedup the
        // text node `c` must carry the outer (sup) mark, NOT the inner (sub).
        // Node `b` is inside only the outer `^…^` and never receives a duplicate,
        // so it is not the interesting target here. This test fails if dedup is
        // changed to last-wins.
        let adf = markdown_to_adf("a ^b ~c~ d^ e");
        let nodes = adf["content"][0]["content"].as_array().unwrap();
        let inner = nodes
            .iter()
            .find(|n| n["text"] == "c")
            .expect("inner text node 'c' must be present");
        let marks = inner["marks"]
            .as_array()
            .expect("inner node must have marks");
        let subsup_mark = marks
            .iter()
            .find(|m| m["type"] == "subsup")
            .expect("exactly one subsup mark must survive dedup");
        let survivor_type = subsup_mark["attrs"]["type"]
            .as_str()
            .expect("subsup mark must have attrs.type");
        assert_eq!(
            survivor_type, "sup",
            "outer sup must win dedup over inner sub: {inner}"
        );
    }

    #[test]
    fn test_markdown_superscript_no_mark_leak_to_trailing_text() {
        // Regression: Tag::Superscript / Tag::Subscript push a `subsup` mark via
        // push_mark (which pushes a NodeKind::InlineMark stack frame). The generic
        // end() handler must pop it via pop_mark when the closing tag fires.
        // If the pop is ever omitted the subsup mark bleeds onto nodes that follow
        // the closing `^`/`~`, so text after the span incorrectly inherits the mark.
        //
        // This test directly asserts the no-leak guarantee that previously rested
        // only on structural reasoning: both that the superscript span node carries
        // the correct mark AND that the trailing text node carries NO mark.
        // Covers superscript and subscript in a single test because the pop path is
        // identical for both (same push_mark / NodeKind::InlineMark mechanism).

        // --- Superscript: "a ^sup^ b" ---
        let adf_sup = markdown_to_adf("a ^sup^ b");
        let nodes_sup = adf_sup["content"][0]["content"]
            .as_array()
            .expect("paragraph must have content nodes");

        let sup_node = nodes_sup
            .iter()
            .find(|n| n["text"] == "sup")
            .expect("superscript text node 'sup' must be present");
        assert_eq!(
            sup_node["marks"][0]["type"], "subsup",
            "sup node must carry subsup mark: {sup_node}"
        );
        assert_eq!(
            sup_node["marks"][0]["attrs"]["type"], "sup",
            "subsup mark must be sup variant: {sup_node}"
        );

        // The trailing node must NOT carry any marks. The text after the closing `^`
        // is " b" (space + b). We find the node whose text comes after the "sup" node
        // in document order (i.e. the last node in the paragraph).
        let trailing_sup = nodes_sup
            .last()
            .expect("paragraph must have at least one trailing node after the span");
        assert_ne!(
            trailing_sup["text"], "sup",
            "last node must be the trailing text, not the span: {nodes_sup:?}"
        );
        assert!(
            trailing_sup["marks"].is_null()
                || trailing_sup["marks"]
                    .as_array()
                    .map(|a| a.is_empty())
                    .unwrap_or(false),
            "subsup mark must not leak onto trailing text after closing `^`: {trailing_sup}"
        );

        // --- Subscript: "a ~sub~ b" ---
        let adf_sub = markdown_to_adf("a ~sub~ b");
        let nodes_sub = adf_sub["content"][0]["content"]
            .as_array()
            .expect("paragraph must have content nodes");

        let sub_node = nodes_sub
            .iter()
            .find(|n| n["text"] == "sub")
            .expect("subscript text node 'sub' must be present");
        assert_eq!(
            sub_node["marks"][0]["type"], "subsup",
            "sub node must carry subsup mark: {sub_node}"
        );
        assert_eq!(
            sub_node["marks"][0]["attrs"]["type"], "sub",
            "subsup mark must be sub variant: {sub_node}"
        );

        let trailing_sub = nodes_sub
            .last()
            .expect("paragraph must have at least one trailing node after the span");
        assert_ne!(
            trailing_sub["text"], "sub",
            "last node must be the trailing text, not the span: {nodes_sub:?}"
        );
        assert!(
            trailing_sub["marks"].is_null()
                || trailing_sub["marks"]
                    .as_array()
                    .map(|a| a.is_empty())
                    .unwrap_or(false),
            "subsup mark must not leak onto trailing text after closing `~`: {trailing_sub}"
        );
    }

    #[test]
    fn test_markdown_heading_non_attribute_brace_stripped() {
        // BC-7.2.008 EC-2: `## Foo {bar}` (no `#`/`.`/`key=val` — a plain word
        // in braces) produces heading text "Foo", not "Foo {bar}".
        // pulldown-cmark with ENABLE_HEADING_ATTRIBUTES treats any `{…}` block at
        // end-of-heading as a potential attribute container and silently drops
        // unrecognised tokens inside it, so `{bar}` is stripped alongside valid
        // forms like `{#id}` and `{.cls}`.
        let adf = markdown_to_adf("## Foo {bar}");
        let heading = &adf["content"][0];
        assert_eq!(heading["type"], "heading", "must be a heading: {adf}");
        assert_eq!(heading["attrs"]["level"], 2, "must be level 2: {adf}");
        let text: String = heading["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["text"].as_str())
            .collect();
        // pulldown-cmark strips `{bar}` — the surviving text is "Foo", not "Foo {bar}".
        // Consistent with BC-7.2.008 EC-2 (corrected): a plain-word brace such as
        // `{bar}` at end-of-heading is silently dropped by the parser alongside valid
        // attribute syntax. This assertion pins that behavior so a future
        // pulldown-cmark upgrade that changes it is caught immediately.
        assert_eq!(
            text, "Foo",
            "pulldown-cmark strips {{bar}} even without valid attr syntax: {adf}"
        );
    }

    #[test]
    fn test_markdown_heading_attributes_stripped() {
        // id, class, key=value, and combined forms are all consumed by
        // ENABLE_HEADING_ATTRIBUTES — none leak into the heading text.
        for src in [
            "## Title {#myid}",
            "## Title {.cls}",
            "## Title {#id .cls}",
            "## Title {key=val}",
        ] {
            let adf = markdown_to_adf(src);
            let heading = &adf["content"][0];
            assert_eq!(heading["type"], "heading", "{src}: {adf}");
            assert_eq!(heading["attrs"]["level"], 2, "{src}: {adf}");
            let text: String = heading["content"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|n| n["text"].as_str())
                .collect();
            // Exact equality is the leak check: any `{#id}`/`{.cls}`/`{key=val}`
            // remnant would make `text` != "Title".
            assert_eq!(text, "Title", "{src}: attr must not leak into text");
        }
    }

    #[test]
    fn test_render_table_pipe_format() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [
                    {"type": "tableRow", "content": [
                        {"type": "tableHeader", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "h1"}]}]},
                        {"type": "tableHeader", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "h2"}]}]},
                    ]},
                    {"type": "tableRow", "content": [
                        {"type": "tableCell", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "a"}]}]},
                        {"type": "tableCell", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "b"}]}]},
                    ]},
                ]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("| h1 | h2 |"), "header row missing: {text:?}");
        assert!(
            text.contains("| --- | --- |"),
            "separator missing: {text:?}"
        );
        assert!(text.contains("| a | b |"), "body row missing: {text:?}");
    }

    #[test]
    fn test_render_table_mixed_header_cell_row_still_emits_separator() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [
                    {"type": "tableRow", "content": [
                        {"type": "tableHeader", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "h"}]}]},
                        {"type": "tableCell", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "c"}]}]},
                    ]},
                ]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("| h | c |"), "row missing: {text:?}");
        assert!(
            text.contains("| --- | --- |"),
            "separator missing: {text:?}"
        );
    }

    #[test]
    fn test_render_table_cell_flattens_paragraph() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [{
                    "type": "tableRow",
                    "content": [
                        {"type": "tableCell", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "just text"}]}]}
                    ]
                }]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("| just text |"), "cell not flat: {text:?}");
    }

    #[test]
    fn test_markdown_table_cell_with_inline_formatting() {
        // Verify marks (Task 2) compose correctly with table cells (Task 3).
        // Structure: doc > table > tableRow > tableHeader > paragraph > text.
        let adf = markdown_to_adf("| **bold** | [link](https://x) |\n| - | - |\n| a | b |");
        let header_row = &adf["content"][0]["content"][0];
        assert_eq!(header_row["type"], "tableRow");

        // First header cell -> paragraph -> text "bold" with strong mark.
        let first_header_cell = &header_row["content"][0];
        assert_eq!(first_header_cell["type"], "tableHeader");
        let first_header_para = &first_header_cell["content"][0];
        assert_eq!(first_header_para["type"], "paragraph");
        let bold_text = &first_header_para["content"][0];
        assert_eq!(bold_text["text"], "bold");
        assert_eq!(bold_text["marks"][0]["type"], "strong");

        // Second header cell -> paragraph -> text "link" with link mark.
        let second_header_cell = &header_row["content"][1];
        assert_eq!(second_header_cell["type"], "tableHeader");
        let second_header_para = &second_header_cell["content"][0];
        let link_text = &second_header_para["content"][0];
        assert_eq!(link_text["text"], "link");
        assert_eq!(link_text["marks"][0]["type"], "link");
        assert_eq!(link_text["marks"][0]["attrs"]["href"], "https://x");
    }

    #[test]
    fn test_render_blockquote_prefixes_each_line() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "blockquote",
                "content": [
                    {"type": "paragraph", "content": [{"type": "text", "text": "line one"}]},
                    {"type": "paragraph", "content": [{"type": "text", "text": "line two"}]}
                ]
            }]
        });
        let text = adf_to_text(&adf);
        for line in text.lines() {
            assert!(line.starts_with("> "), "line should be prefixed: {line:?}");
        }
        assert!(text.contains("> line one"));
        assert!(text.contains("> line two"));
    }

    #[test]
    fn test_render_nested_blockquote() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "blockquote",
                "content": [{
                    "type": "blockquote",
                    "content": [
                        {"type": "paragraph", "content": [{"type": "text", "text": "inner"}]}
                    ]
                }]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("> > inner"), "got: {text:?}");
    }

    #[test]
    fn test_render_strong_mark() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "bold", "marks": [{"type": "strong"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf), "**bold**");
    }

    #[test]
    fn test_render_em_mark() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "em", "marks": [{"type": "em"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf), "*em*");
    }

    #[test]
    fn test_render_strike_mark() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "gone", "marks": [{"type": "strike"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf), "~~gone~~");
    }

    #[test]
    fn test_render_code_mark() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "x", "marks": [{"type": "code"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf), "`x`");
    }

    #[test]
    fn test_render_link_preserves_href() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "jr", "marks": [
                    {"type": "link", "attrs": {"href": "https://example.com/jr"}}
                ]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf), "[jr](https://example.com/jr)");
    }

    #[test]
    fn test_render_link_missing_href_defaults_empty() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "jr", "marks": [{"type": "link"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf), "[jr]()");
    }

    #[test]
    fn test_render_multiple_marks_deterministic_order() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "foo", "marks": [{"type": "strong"}, {"type": "em"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf), "***foo***");
    }

    #[test]
    fn test_render_unknown_mark_drops_syntax() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "plain", "marks": [{"type": "underline"}]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf), "plain");
    }

    #[test]
    fn test_render_ordered_list_numeric_prefix() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "orderedList",
                "content": [
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "alpha"}]}]},
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "beta"}]}]},
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "gamma"}]}]},
                ]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("1. alpha"), "got: {text:?}");
        assert!(text.contains("2. beta"), "got: {text:?}");
        assert!(text.contains("3. gamma"), "got: {text:?}");
    }

    #[test]
    fn test_render_ordered_list_respects_attrs_order() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "orderedList",
                "attrs": {"order": 5},
                "content": [
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "five"}]}]},
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "six"}]}]},
                ]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("5. five"), "got: {text:?}");
        assert!(text.contains("6. six"), "got: {text:?}");
    }

    #[test]
    fn test_render_ordered_list_order_zero_defaults_to_one() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "orderedList",
                "attrs": {"order": 0},
                "content": [
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "only"}]}]},
                ]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("1. only"), "got: {text:?}");
    }

    #[test]
    fn test_render_mixed_nested_lists() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "orderedList",
                "content": [{
                    "type": "listItem",
                    "content": [
                        {"type": "paragraph", "content": [{"type": "text", "text": "outer"}]},
                        {"type": "bulletList", "content": [{
                            "type": "listItem",
                            "content": [{"type": "paragraph", "content": [{"type": "text", "text": "inner"}]}]
                        }]}
                    ]
                }]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("1. outer"), "got: {text:?}");
        assert!(text.contains("  - inner"), "got: {text:?}");
    }

    #[test]
    fn test_render_rule() {
        let adf = json!({
            "type": "doc",
            "content": [
                {"type": "paragraph", "content": [{"type": "text", "text": "above"}]},
                {"type": "rule"},
                {"type": "paragraph", "content": [{"type": "text", "text": "below"}]}
            ]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("---"), "expected rule line, got: {text:?}");
        assert!(text.contains("above"));
        assert!(text.contains("below"));
    }

    #[test]
    fn test_render_hard_break_inserts_newline() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "line one"},
                {"type": "hardBreak"},
                {"type": "text", "text": "line two"}
            ]}]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("line one\nline two"), "got: {text:?}");
    }

    #[test]
    fn test_render_code_block_with_language() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "codeBlock",
                "attrs": {"language": "rust"},
                "content": [{"type": "text", "text": "fn x() {}"}]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(
            text.contains("```rust"),
            "expected rust fence, got: {text:?}"
        );
        assert!(text.contains("fn x() {}"));
    }

    #[test]
    fn test_render_code_block_without_language() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "codeBlock",
                "content": [{"type": "text", "text": "plain"}]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(
            text.contains("```\nplain"),
            "expected empty fence, got: {text:?}"
        );
    }

    #[test]
    fn test_markdown_to_adf_to_text_roundtrip() {
        let input = concat!(
            "# Heading\n",
            "\n",
            "Paragraph with **bold** and *italic* and `code`.\n",
            "\n",
            "- a\n",
            "- b\n",
            "\n",
            "1. one\n",
            "2. two\n",
            "\n",
            "> quote\n",
        );
        let adf_original = markdown_to_adf(input);
        let text = adf_to_text(&adf_original);
        let adf_reparsed = markdown_to_adf(&text);

        let types_original = collect_node_types(&adf_original);
        let types_reparsed = collect_node_types(&adf_reparsed);
        assert_eq!(
            types_original, types_reparsed,
            "node-type structure should roundtrip"
        );
    }

    /// Walk the ADF tree depth-first and collect each node's `type` field.
    /// Used to assert structural (not textual) equivalence on roundtrip.
    fn collect_node_types(adf: &Value) -> Vec<String> {
        let mut types = Vec::new();
        walk_types(adf, &mut types);
        types
    }

    fn walk_types(node: &Value, out: &mut Vec<String>) {
        if let Some(t) = node.get("type").and_then(|t| t.as_str()) {
            out.push(t.to_string());
        }
        if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
            for child in content {
                walk_types(child, out);
            }
        }
    }

    #[test]
    fn test_render_code_mark_with_backtick_in_content() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "foo`bar", "marks": [{"type": "code"}]}
            ]}]
        });
        let text = adf_to_text(&adf);
        assert_eq!(text, "``foo`bar``");
    }

    #[test]
    fn test_render_code_mark_with_leading_trailing_backtick_pads() {
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "`x`", "marks": [{"type": "code"}]}
            ]}]
        });
        let text = adf_to_text(&adf);
        assert_eq!(text, "`` `x` ``");
    }

    #[test]
    fn test_render_blockquote_with_internal_blank_line_keeps_prefix() {
        // Blockquote containing a codeBlock whose content has a blank line.
        // The blank line inside the quote must get a ">" prefix so the
        // blockquote context isn't broken.
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "blockquote",
                "content": [{
                    "type": "codeBlock",
                    "content": [{"type": "text", "text": "line 1\n\nline 3"}]
                }]
            }]
        });
        let text = adf_to_text(&adf);
        for line in text.lines() {
            assert!(
                line.starts_with('>'),
                "every rendered line inside the blockquote should begin with '>': {line:?}"
            );
        }
    }

    #[test]
    fn test_adf_to_text_empty_doc() {
        // `{"type": "doc", "content": []}` has no children — the renderer
        // iterates an empty array and `finish()` returns an empty string.
        // Pinned here so a future refactor that starts emitting a placeholder
        // for empty documents trips a test instead of silently changing output.
        let adf = json!({"type": "doc", "content": []});
        assert_eq!(adf_to_text(&adf), "");
    }

    #[test]
    fn test_render_blockquote_with_empty_paragraph_produces_no_output() {
        // A blockquote whose only child is an empty paragraph currently
        // produces no output: the paragraph emits just its trailing newline,
        // and the blockquote's trailing-empty-lines trim pops both lines off,
        // leaving nothing to prefix. This is a quirk of the current contract
        // rather than a "correct" answer — the pinned behavior here lets a
        // future decision to instead emit a bare `> ` marker surface as a
        // test failure rather than a silent change.
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "blockquote",
                "content": [{"type": "paragraph", "content": []}]
            }]
        });
        assert_eq!(adf_to_text(&adf), "");
    }

    #[test]
    fn test_render_consecutive_hard_breaks_produce_multiple_newlines() {
        // Each `hardBreak` pushes a `\n`, so two consecutive ones inside a
        // paragraph leave a blank line between the surrounding text spans.
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [
                    {"type": "text", "text": "a"},
                    {"type": "hardBreak"},
                    {"type": "hardBreak"},
                    {"type": "text", "text": "b"}
                ]
            }]
        });
        assert_eq!(adf_to_text(&adf), "a\n\nb");
    }

    #[test]
    fn test_render_marks_code_and_strong() {
        // The write-path emits `[strong, code]` for `**`x`**` because
        // `push_code` appends `{type: "code"}` after active marks. This test
        // covers the reverse-order case: even when the array is
        // `[code, strong]`, the `code` mark is applied innermost, so bold
        // wraps the code span rather than the other way around.
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "x", "marks": [
                    {"type": "code"}, {"type": "strong"}
                ]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf), "**`x`**");
    }

    #[test]
    fn test_render_marks_strike_and_em() {
        // Non-code marks wrap in array order: `strike` first (innermost),
        // then `em` outside it.
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "x", "marks": [
                    {"type": "strike"}, {"type": "em"}
                ]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf), "*~~x~~*");
    }

    #[test]
    fn test_render_marks_link_and_strong() {
        // `link` precedes `strong` in the marks array, so `apply_marks` wraps
        // `link` first (producing `[x](href)`) and then `strong` around that.
        // `link` has no code-style special case, so the result is purely
        // order-driven.
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "x", "marks": [
                    {"type": "link", "attrs": {"href": "https://example.com/jr"}},
                    {"type": "strong"}
                ]}
            ]}]
        });
        assert_eq!(adf_to_text(&adf), "**[x](https://example.com/jr)**");
    }

    #[test]
    fn test_render_trailing_hard_breaks_stripped_by_finish() {
        // `finish()` calls `trim_end()` on the accumulated output. The
        // paragraph's own trailing `\n` and any trailing `hardBreak` newlines
        // are all whitespace, so they're stripped together. This pins the
        // "no stray blank lines at end of doc" contract — the more brittle
        // complement to the interior-hardBreak test above.
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [
                    {"type": "text", "text": "a"},
                    {"type": "hardBreak"},
                    {"type": "hardBreak"}
                ]
            }]
        });
        assert_eq!(adf_to_text(&adf), "a");
    }

    #[test]
    fn test_render_hard_break_in_table_cell_becomes_space() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [{
                    "type": "tableRow",
                    "content": [{
                        "type": "tableCell",
                        "content": [{
                            "type": "paragraph",
                            "content": [
                                {"type": "text", "text": "line one"},
                                {"type": "hardBreak"},
                                {"type": "text", "text": "line two"}
                            ]
                        }]
                    }]
                }]
            }]
        });
        let text = adf_to_text(&adf);
        // The cell content must stay on a single pipe row — no embedded newline.
        assert!(text.contains("| line one line two |"), "got: {text:?}");
    }

    #[test]
    fn test_render_strong_with_code_applies_code_innermost() {
        // Matches the write-path's marks ordering: strong + code produces
        // marks = [strong, code]. Output must be **`code`** not `**code**`.
        let adf = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "x", "marks": [{"type": "strong"}, {"type": "code"}]}
            ]}]
        });
        let text = adf_to_text(&adf);
        assert_eq!(text, "**`x`**");
    }

    #[test]
    fn test_render_table_cell_escapes_pipe_in_text() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [{
                    "type": "tableRow",
                    "content": [{
                        "type": "tableCell",
                        "content": [{"type": "paragraph", "content": [
                            {"type": "text", "text": "a|b"}
                        ]}]
                    }]
                }]
            }]
        });
        let text = adf_to_text(&adf);
        // Pipe inside the cell must be escaped so it doesn't introduce a
        // false column break.
        assert!(text.contains(r"| a\|b |"), "got: {text:?}");
    }

    #[test]
    fn test_render_table_cell_collapses_newlines_in_text() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [{
                    "type": "tableRow",
                    "content": [{
                        "type": "tableCell",
                        "content": [{"type": "paragraph", "content": [
                            {"type": "text", "text": "line\nwrap"}
                        ]}]
                    }]
                }]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("| line wrap |"), "got: {text:?}");
    }

    // --- GFM alerts -> ADF panel (issue #483) ----------------------------
    // `> [!NOTE|TIP|IMPORTANT|WARNING|CAUTION]` maps to an ADF `panel` with a
    // portable panelType, normalizing the panel content model (no nested panel,
    // table, or blockquote) the way #470 did for listItem.
    // See docs/specs/adf-panel-content-model.md.

    /// First top-level node of the document.
    fn first_block(adf: &Value) -> Value {
        adf["content"][0].clone()
    }

    /// Assert the document's first block is a `panel` with the given panelType,
    /// and return its `content` array.
    fn assert_panel(adf: &Value, panel_type: &str) -> Vec<Value> {
        let block = first_block(adf);
        assert_eq!(block["type"], "panel", "expected panel, got: {block}");
        assert_eq!(
            block["attrs"]["panelType"], panel_type,
            "panelType mismatch: {block}"
        );
        block["content"].as_array().cloned().unwrap_or_default()
    }

    /// Recursively collect every node `type` that appears inside any `panel` in
    /// the document (the panel's direct and transitive content).
    fn panel_descendant_types(adf: &Value) -> Vec<String> {
        fn walk(node: &Value, in_panel: bool, acc: &mut Vec<String>) {
            if in_panel {
                if let Some(t) = node.get("type").and_then(Value::as_str) {
                    acc.push(t.to_string());
                }
            }
            let now_in_panel =
                in_panel || node.get("type").and_then(Value::as_str) == Some("panel");
            if let Some(children) = node.get("content").and_then(Value::as_array) {
                for child in children {
                    walk(child, now_in_panel, acc);
                }
            }
        }
        let mut acc = Vec::new();
        if let Some(content) = adf.get("content").and_then(Value::as_array) {
            for node in content {
                walk(node, false, &mut acc);
            }
        }
        acc
    }

    #[test]
    fn test_markdown_alert_note_maps_to_panel_info() {
        let adf = markdown_to_adf("> [!NOTE]\n> useful info");
        let content = assert_panel(&adf, "info");
        assert_eq!(content[0]["type"], "paragraph", "got: {content:?}");
        assert!(
            adf.to_string().contains("useful info"),
            "body preserved: {adf}"
        );
        // The `[!NOTE]` marker must NOT survive as literal text.
        assert!(
            !adf.to_string().contains("[!NOTE]"),
            "marker leaked into content: {adf}"
        );
    }

    #[test]
    fn test_markdown_alert_tip_maps_to_panel_success() {
        let adf = markdown_to_adf("> [!TIP]\n> a tip");
        assert_panel(&adf, "success");
    }

    #[test]
    fn test_markdown_alert_important_maps_to_panel_note() {
        let adf = markdown_to_adf("> [!IMPORTANT]\n> key point");
        assert_panel(&adf, "note");
    }

    #[test]
    fn test_markdown_alert_warning_maps_to_panel_warning() {
        let adf = markdown_to_adf("> [!WARNING]\n> careful");
        assert_panel(&adf, "warning");
    }

    #[test]
    fn test_markdown_alert_caution_maps_to_panel_error() {
        let adf = markdown_to_adf("> [!CAUTION]\n> danger");
        assert_panel(&adf, "error");
    }

    #[test]
    fn test_markdown_alert_marker_with_trailing_text_stays_literal_blockquote() {
        // pulldown-cmark 0.13 requires the alert marker to be the SOLE content of
        // the first line. Trailing text on the marker line (`> [!NOTE] extra`)
        // disqualifies it -> stays a plain blockquote with the marker as literal
        // text. (Note: pulldown is otherwise lenient — a missing space `>[!NOTE]`
        // and any-case `[!note]`/`[!Note]` ARE still recognized as alerts.)
        let adf = markdown_to_adf("> [!NOTE] extra\n> text");
        let block = first_block(&adf);
        assert_eq!(block["type"], "blockquote", "got: {block}");
        assert!(
            adf.to_string().contains("[!NOTE]"),
            "marker stays literal: {adf}"
        );
    }

    #[test]
    fn test_markdown_unknown_alert_kind_stays_literal_blockquote() {
        let adf = markdown_to_adf("> [!FOO]\n> text");
        let block = first_block(&adf);
        assert_eq!(block["type"], "blockquote", "got: {block}");
        assert!(
            adf.to_string().contains("[!FOO]"),
            "unknown marker stays literal: {adf}"
        );
    }

    #[test]
    fn test_markdown_plain_blockquote_unchanged() {
        let adf = markdown_to_adf("> just a quote");
        let block = first_block(&adf);
        assert_eq!(block["type"], "blockquote", "got: {block}");
    }

    #[test]
    fn test_markdown_nested_alert_unwraps_inner_panel() {
        // `panel > panel` is invalid ADF; the inner alert is unwrapped, its blocks
        // spliced into the outer panel. No panel may contain a nested panel.
        let adf = markdown_to_adf("> [!NOTE]\n> outer\n> > [!TIP]\n> > inner");
        let content = assert_panel(&adf, "info");
        let types: Vec<&str> = content.iter().filter_map(|n| n["type"].as_str()).collect();
        assert!(
            !types.contains(&"panel"),
            "inner panel must be unwrapped: {content:?}"
        );
        assert!(
            !panel_descendant_types(&adf).contains(&"panel".to_string()),
            "no nested panel anywhere: {adf}"
        );
        assert!(
            adf.to_string().contains("inner"),
            "inner text preserved: {adf}"
        );
    }

    #[test]
    fn test_markdown_alert_with_table_flattens_to_paragraphs() {
        let md = "> [!NOTE]\n> | a | b |\n> | --- | --- |\n> | 1 | 2 |";
        let adf = markdown_to_adf(md);
        assert_panel(&adf, "info");
        let descendants = panel_descendant_types(&adf);
        assert!(
            !descendants.contains(&"table".to_string()),
            "table must be flattened out of panel: {adf}"
        );
        // Flattened, not dropped: the cell data must survive (one paragraph per
        // row in `| a | b |` form).
        let s = adf.to_string();
        for cell in ["a", "b", "1", "2"] {
            assert!(s.contains(cell), "cell `{cell}` lost on flatten: {adf}");
        }
        assert!(
            descendants.contains(&"paragraph".to_string()),
            "rows must become paragraphs: {adf}"
        );
    }

    #[test]
    fn test_markdown_alert_in_listitem_unwraps_panel() {
        // `listItem > panel` is invalid ADF; the panel is unwrapped inside the item.
        let md = "- item\n\n  > [!NOTE]\n  > nested";
        let adf = markdown_to_adf(md);
        let descendants_have_panel = {
            fn has_panel_in_listitem(node: &Value) -> bool {
                let is_li = node.get("type").and_then(Value::as_str) == Some("listItem");
                if is_li {
                    if let Some(c) = node.get("content").and_then(Value::as_array) {
                        if c.iter().any(|n| n["type"] == "panel") {
                            return true;
                        }
                    }
                }
                node.get("content")
                    .and_then(Value::as_array)
                    .is_some_and(|c| c.iter().any(has_panel_in_listitem))
            }
            adf.get("content")
                .and_then(Value::as_array)
                .is_some_and(|c| c.iter().any(has_panel_in_listitem))
        };
        assert!(
            !descendants_have_panel,
            "listItem must not contain a panel: {adf}"
        );
        assert!(adf.to_string().contains("nested"), "text preserved: {adf}");
    }

    #[test]
    fn test_markdown_alert_heading_child_has_no_marks() {
        // panel.content requires `heading (no marks)`. A heading inside an alert
        // must not carry a node-level `marks` array.
        let adf = markdown_to_adf("> [!NOTE]\n> # Title");
        let content = assert_panel(&adf, "info");
        let heading = content
            .iter()
            .find(|n| n["type"] == "heading")
            .expect("heading preserved in panel");
        assert!(
            heading.get("marks").is_none(),
            "heading must have no node-level marks: {heading}"
        );
    }

    #[test]
    fn test_markdown_empty_alert_pruned() {
        // An alert with no body would produce an empty panel shell (invalid ADF,
        // Jira 400). It must be pruned entirely by is_empty_block_container — NOT
        // kept as a panel holding a placeholder paragraph. Positively assert the
        // panel node is absent from the whole document (not vacuously true on a
        // non-empty-content panel).
        let adf = markdown_to_adf("> [!NOTE]");
        assert_no_invalid_empty_container(&adf);
        fn has_panel(n: &Value) -> bool {
            n["type"] == "panel"
                || n["content"]
                    .as_array()
                    .is_some_and(|c| c.iter().any(has_panel))
        }
        assert!(
            !has_panel(&adf),
            "empty panel must be pruned entirely: {adf}"
        );
    }

    #[test]
    fn test_panel_content_only_permitted_node_types() {
        // Invariant: no panel anywhere contains a disallowed child node type.
        let md = "> [!NOTE]\n> outer\n> > [!TIP]\n> > | a | b |\n> > | - | - |\n> > | 1 | 2 |";
        let adf = markdown_to_adf(md);
        const FORBIDDEN: [&str; 3] = ["panel", "table", "blockquote"];
        for t in panel_descendant_types(&adf) {
            assert!(
                !FORBIDDEN.contains(&t.as_str()),
                "forbidden node `{t}` inside panel: {adf}"
            );
        }
    }

    #[test]
    fn test_render_panel_info_to_note_alert() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "panel",
                "attrs": {"panelType": "info"},
                "content": [
                    {"type": "paragraph", "content": [{"type": "text", "text": "useful"}]}
                ]
            }]
        });
        let text = adf_to_text(&adf);
        // The marker line itself must be quoted (`> [!NOTE]`), not a bare
        // `[!NOTE]` (which would be a malformed alert), and the body line quoted.
        assert!(
            text.contains("> [!NOTE]"),
            "marker line must be quoted: {text:?}"
        );
        assert!(text.contains("> useful"), "body quoted: {text:?}");
    }

    #[test]
    fn test_render_panel_multiline_body_quotes_every_line() {
        // Each body line of a panel must get its own `> ` prefix (the per-line
        // logic shared with the blockquote arm).
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "panel",
                "attrs": {"panelType": "warning"},
                "content": [
                    {"type": "paragraph", "content": [{"type": "text", "text": "line one"}]},
                    {"type": "paragraph", "content": [{"type": "text", "text": "line two"}]}
                ]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(text.contains("> [!WARNING]"), "marker quoted: {text:?}");
        assert!(text.contains("> line one"), "first line quoted: {text:?}");
        assert!(text.contains("> line two"), "second line quoted: {text:?}");
    }

    #[test]
    fn test_render_panel_tip_type_renders_no_marker() {
        // `markdown_to_adf` never emits panelType `tip`, but `adf_to_text` may
        // receive it from another source; it has no GFM label, so it renders as
        // a plain quoted blockquote with no marker (same arm as `custom`).
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "panel",
                "attrs": {"panelType": "tip"},
                "content": [
                    {"type": "paragraph", "content": [{"type": "text", "text": "y"}]}
                ]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(!text.contains("[!"), "no alert marker for `tip`: {text:?}");
        assert!(text.contains("> y"), "still quoted: {text:?}");
    }

    #[test]
    fn test_render_panel_unknown_type_to_plain_blockquote() {
        let adf = json!({
            "type": "doc",
            "content": [{
                "type": "panel",
                "attrs": {"panelType": "custom"},
                "content": [
                    {"type": "paragraph", "content": [{"type": "text", "text": "x"}]}
                ]
            }]
        });
        let text = adf_to_text(&adf);
        assert!(
            !text.contains("[!"),
            "no alert marker for unknown type: {text:?}"
        );
        assert!(text.contains("> x"), "still quoted: {text:?}");
    }

    #[test]
    fn test_alert_markdown_to_text_roundtrip_all_kinds() {
        for (marker, body) in [
            ("NOTE", "n"),
            ("TIP", "t"),
            ("IMPORTANT", "i"),
            ("WARNING", "w"),
            ("CAUTION", "c"),
        ] {
            let md = format!("> [!{marker}]\n> {body}");
            let adf = markdown_to_adf(&md);
            let text = adf_to_text(&adf);
            assert!(
                text.contains(&format!("[!{marker}]")),
                "round-trip lost marker for {marker}: {text:?}"
            );
            assert!(
                text.contains(&format!("> {body}")),
                "round-trip lost body for {marker}: {text:?}"
            );
        }
    }

    #[test]
    fn test_markdown_alert_marker_without_leading_space_still_panel() {
        // pulldown-cmark 0.13 is lenient: a missing space after `>` (`>[!NOTE]`)
        // is still recognized as an alert. This pins an upstream-dependency
        // behavior the mapping relies on — a future bump narrowing recognition
        // would otherwise silently leak `[!NOTE]` as a plain blockquote.
        let adf = markdown_to_adf(">[!NOTE]\n>body");
        assert_panel(&adf, "info");
    }

    #[test]
    fn test_markdown_alert_marker_case_insensitive_still_panel() {
        // pulldown-cmark recognizes the marker case-insensitively (`[!note]`,
        // `[!Note]`). Pin it so a dependency bump can't silently regress it.
        for md in ["> [!note]\n> b", "> [!Note]\n> b", "> [!WaRnInG]\n> b"] {
            let adf = markdown_to_adf(md);
            let block = first_block(&adf);
            assert_eq!(
                block["type"], "panel",
                "case-insensitive marker must map to panel: {md:?} -> {block}"
            );
        }
    }

    // --- Missing path coverage (issue #476) ---------------------------------
    // Three characterization/pinning tests for code paths that markdown_to_adf
    // actively exercises but had no dedicated assertion.

    /// 1. Nested ordered list: outer `orderedList` → `listItem` containing both
    ///    a text paragraph and an inner `orderedList`.
    #[test]
    fn test_convert_nested_ordered_list_produces_inner_ordered_list() {
        // "1. a\n   1. b" — three-space indent is what pulldown-cmark requires
        // for a sub-list inside an ordered item (mirrors "- outer\n  - inner"
        // for bullets but with 3-space indent because the "1. " prefix is 3 chars).
        let adf = markdown_to_adf("1. a\n   1. b");

        // Outer list is an orderedList at doc root.
        let outer_list = &adf["content"][0];
        assert_eq!(
            outer_list["type"], "orderedList",
            "outer must be orderedList: {adf}"
        );

        // The outer listItem wraps a paragraph "a" and the nested orderedList.
        let outer_item = &outer_list["content"][0];
        assert_eq!(outer_item["type"], "listItem");

        // The inner orderedList must be a direct child of the outer listItem.
        let inner_list = outer_item["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["type"] == "orderedList")
            .expect("outer listItem must contain a nested orderedList");

        // The inner orderedList contains exactly one listItem with text "b".
        let inner_items = inner_list["content"].as_array().unwrap();
        assert_eq!(
            inner_items.len(),
            1,
            "inner orderedList has one item: {inner_list}"
        );
        assert_eq!(inner_items[0]["type"], "listItem");
        // pulldown-cmark wraps tight inner items in a paragraph via wrap_inlines_as_blocks.
        let inner_text = inner_items[0]["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| {
                panic!("inner listItem must have paragraph > text: {inner_items:?}")
            });
        assert_eq!(
            inner_text, "b",
            "inner item text must be 'b': {inner_items:?}"
        );
    }

    /// 2. Block-level HTML: `<div>x</div>` on its own line triggers
    ///    `Tag::HtmlBlock` (Start + End), which hits the `_ => push(NodeKind::Sink)`
    ///    catch-all in `start()`. The subsequent `Event::Html` text goes through
    ///    `push_text`, but `push_text`'s Sink guard discards it. The End pops the
    ///    Sink, returning `None` — the whole block is silently dropped.
    ///
    ///    Inline HTML (`Event::InlineHtml`) arrives via the identical
    ///    `Event::Html(html) | Event::InlineHtml(html) => self.push_text(...)` arm,
    ///    but WITHOUT a surrounding Start/End Sink wrapper, so `push_text` runs
    ///    normally and the literal HTML is preserved as text (see
    ///    `test_markdown_inline_html_becomes_literal_text`).
    #[test]
    fn test_convert_block_html_is_silently_dropped() {
        // `<div>x</div>` on its own line: pulldown-cmark emits
        //   Start(HtmlBlock) → Html("<div>x</div>") → End(HtmlBlock).
        // Start(HtmlBlock) → Sink catch-all; push_text Sink guard discards the
        // Html event; End pops Sink returning None. Result: empty doc.
        let adf = markdown_to_adf("<div>x</div>");
        let content = adf["content"].as_array().unwrap();
        assert!(
            content.is_empty(),
            "block HTML must be silently dropped (HtmlBlock wraps it in a Sink): {adf}"
        );
    }

    /// 3. `hardBreak` inside a mark span: `**line one  \nline two**` (two
    ///    trailing spaces before `\n` = markdown hard break, inside bold).
    ///
    ///    The `InlineMark` end handler splices children into the parent paragraph.
    ///    The produced paragraph content is:
    ///    text("line one", marks=[strong]), hardBreak, text("line two", marks=[strong])
    #[test]
    fn test_convert_hard_break_inside_mark_span_preserves_mark_and_break() {
        // Two trailing spaces before the newline produce a hard break within the
        // bold span. The InlineMark end handler (NodeKind::InlineMark branch in
        // `end()`) splices already-marked children — including the hardBreak node
        // pushed by `Event::HardBreak` — back into the parent paragraph.
        let adf = markdown_to_adf("**line one  \nline two**");

        let para = &adf["content"][0];
        assert_eq!(
            para["type"], "paragraph",
            "outer node must be paragraph: {adf}"
        );

        let children = para["content"].as_array().unwrap();
        // Must contain: text("line one"), hardBreak, text("line two").
        assert_eq!(
            children.len(),
            3,
            "paragraph must have exactly 3 children (text, hardBreak, text): {children:?}"
        );

        // First child: text "line one" with strong mark.
        assert_eq!(children[0]["type"], "text");
        assert_eq!(children[0]["text"], "line one");
        assert_eq!(
            children[0]["marks"][0]["type"], "strong",
            "first text must carry strong mark: {:?}",
            children[0]
        );

        // Second child: hardBreak (no marks — hardBreak nodes never carry marks
        // in ADF; they are emitted by Event::HardBreak → append_child directly).
        assert_eq!(
            children[1]["type"], "hardBreak",
            "middle child must be hardBreak: {:?}",
            children[1]
        );

        // Third child: text "line two" with strong mark.
        assert_eq!(children[2]["type"], "text");
        assert_eq!(children[2]["text"], "line two");
        assert_eq!(
            children[2]["marks"][0]["type"], "strong",
            "second text must carry strong mark: {:?}",
            children[2]
        );
    }

    #[test]
    fn test_normalize_panel_content_strips_paragraph_marks() {
        // panel.content requires `paragraph (no marks)`. markdown_to_adf does not
        // emit block-level marks today, so exercise the defense-in-depth strip
        // directly with a synthetic marked paragraph.
        let children = vec![json!({
            "type": "paragraph",
            "marks": [{ "type": "strong" }],
            "content": [{ "type": "text", "text": "x" }]
        })];
        let out = normalize_panel_content(children);
        assert_eq!(out.len(), 1, "paragraph kept: {out:?}");
        assert!(
            out[0].get("marks").is_none(),
            "node-level marks stripped from panel paragraph: {:?}",
            out[0]
        );
        // Inline content (and its marks) is untouched — only node-level marks go.
        assert_eq!(out[0]["content"][0]["text"], "x");
    }
}

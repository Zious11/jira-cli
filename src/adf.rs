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

use pulldown_cmark::{
    CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd, TextMergeStream,
};

pub fn markdown_to_adf(markdown: &str) -> Value {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
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

struct AdfBuilder {
    root: Vec<Value>,
    stack: Vec<PartialNode>,
    active_marks: Vec<Value>,
    in_table_head: bool,
}

struct PartialNode {
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
    Sink,
    // Container for inline marks. Has no ADF node; just manages the active_marks stack
    // so End events pop cleanly.
    InlineMark,
    Table,
    TableRow,
    TableCell { is_header: bool },
}

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
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
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
            Tag::Strong => self.push_mark(json!({ "type": "strong" })),
            Tag::Emphasis => self.push_mark(json!({ "type": "em" })),
            Tag::Strikethrough => self.push_mark(json!({ "type": "strike" })),
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
            NodeKind::Table => Some(json!({ "type": "table", "content": children })),
            NodeKind::TableRow => Some(json!({ "type": "tableRow", "content": children })),
            NodeKind::TableCell { is_header } => {
                // ADF requires cells to wrap content in a block (paragraph).
                // pulldown-cmark emits Text events directly inside TableCell
                // without a Paragraph wrapper, so we wrap here.
                let cell_type = if is_header {
                    "tableHeader"
                } else {
                    "tableCell"
                };
                let wrapped_content = if children.iter().all(|n| {
                    n["type"].as_str().is_some_and(|t| {
                        matches!(
                            t,
                            "paragraph"
                                | "bulletList"
                                | "orderedList"
                                | "blockquote"
                                | "codeBlock"
                                | "heading"
                        )
                    })
                }) {
                    children
                } else {
                    vec![json!({ "type": "paragraph", "content": children })]
                };
                Some(json!({ "type": cell_type, "content": wrapped_content }))
            }
            NodeKind::InlineMark => {
                self.pop_mark();
                // InlineMark is a transparent container: splice its collected text
                // nodes (already tagged with marks via active_marks at the time of
                // push_text) into the parent, rather than discarding them.
                for child in children {
                    self.append_child(child);
                }
                None
            }
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

pub fn adf_to_text(adf: &Value) -> String {
    let mut output = String::new();
    if let Some(content) = adf.get("content").and_then(|c| c.as_array()) {
        for node in content {
            render_node(node, &mut output, 0);
        }
    }
    output.trim_end().to_string()
}

fn render_node(node: &Value, output: &mut String, depth: usize) {
    let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match node_type {
        "text" => {
            if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
                output.push_str(text);
            }
        }
        "paragraph" => {
            render_children(node, output, depth);
            output.push('\n');
        }
        "heading" => {
            let level = node
                .get("attrs")
                .and_then(|a| a.get("level"))
                .and_then(|l| l.as_u64())
                .unwrap_or(1) as usize;
            for _ in 0..level {
                output.push('#');
            }
            output.push(' ');
            render_children(node, output, depth);
            output.push('\n');
        }
        "bulletList" | "orderedList" => {
            render_children(node, output, depth);
        }
        "listItem" => {
            let indent = "  ".repeat(depth);
            output.push_str(&indent);
            output.push_str("- ");
            render_children(node, output, depth + 1);
        }
        "codeBlock" => {
            output.push_str("```\n");
            render_children(node, output, depth);
            output.push_str("\n```\n");
        }
        _ => {
            if node.get("content").is_some() {
                render_children(node, output, depth);
            } else {
                output.push_str(&format!("[unsupported: {node_type}]"));
            }
        }
    }
}

fn render_children(node: &Value, output: &mut String, depth: usize) {
    if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
        for child in content {
            render_node(child, output, depth);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_adf_to_text_unsupported() {
        let adf = json!({
            "type": "doc",
            "content": [{ "type": "mediaGroup" }]
        });
        assert!(adf_to_text(&adf).contains("[unsupported: mediaGroup]"));
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
                {"type": "heading", "attrs": {"level": 2}, "content": [{"type": "text", "text": "Summary"}]},
                {"type": "paragraph", "content": [{"type": "text", "text": "This is a description."}]},
                {"type": "bulletList", "content": [
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Item one"}]}]},
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Item two"}]}]}
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
}
